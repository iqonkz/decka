//! `Agent`: owns config, state store and the HTTP/WS server (ARCHITECTURE.md 4.3).

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::actions::ActionRunner;
use crate::auth::{Devices, Pairing};
use crate::config::{self, Loaded};
use crate::model::*;
use crate::proto::NotifyReq;
use crate::state::{Store, MAX_NOTIFICATIONS};

/// Notification body limit (PROTOCOL.md 5).
const MAX_BODY_CHARS: usize = 500;

pub struct AgentOptions {
    pub config_dir: Option<PathBuf>,
    pub client_dist: Option<PathBuf>,
    pub port_override: Option<u16>,
    pub runner: Arc<dyn ActionRunner>,
    /// Headless/tests only: a pairing code that stays valid and is never consumed.
    pub fixed_pair_code: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("invalid bind address '{0}'")]
    BadBind(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listening {
    pub bind: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStatus {
    pub device_id: String,
    pub name: String,
    pub remote: String,
    pub connected_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbDevice {
    pub serial: String,
    pub reversed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbStatus {
    pub enabled: bool,
    pub adb_found: bool,
    pub devices: Vec<UsbDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Mirrors `AgentStatus` in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub version: String,
    pub listening: Option<Listening>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_error: Option<String>,
    pub lan_ips: Vec<String>,
    pub clients: Vec<ClientStatus>,
    pub usb: UsbStatus,
    pub config_dir: String,
}

/// Mirrors `PairingInfo` in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfo {
    pub code: String,
    pub url: String,
    pub qr_svg: String,
    pub expires_at: u64,
}

/// Mirrors `DeviceInfo` in shared/src/config.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub created_at: u64,
    pub last_seen_at: u64,
    pub connected: bool,
}

struct ServerHandle {
    shutdown: oneshot::Sender<()>,
    join: JoinHandle<()>,
}

struct Inner {
    opts: AgentOptions,
    config_dir: PathBuf,
    config: Mutex<ConfigFile>,
    actions: Mutex<ActionsFile>,
    layout: Mutex<LayoutFile>,
    store: Store,
    pairing: Pairing,
    devices: Devices,
    notif_counter: AtomicU64,
    listening: Mutex<Option<Listening>>,
    listen_error: Mutex<Option<String>>,
    server: tokio::sync::Mutex<Option<ServerHandle>>,
}

#[derive(Clone)]
pub struct Agent {
    inner: Arc<Inner>,
}

impl Agent {
    pub async fn start(opts: AgentOptions) -> Result<Agent, StartError> {
        let config_dir = crate::paths::config_dir(opts.config_dir.clone());
        let Loaded { config, actions, layout, devices, issues } = config::load_or_init(&config_dir);
        if config.bind.parse::<IpAddr>().is_err() {
            return Err(StartError::BadBind(config.bind));
        }

        let layout_value = serde_json::to_value(layout.to_client()).expect("layout serializes");
        let agent = Agent {
            inner: Arc::new(Inner {
                config_dir: config_dir.clone(),
                config: Mutex::new(config),
                actions: Mutex::new(actions),
                layout: Mutex::new(layout),
                store: Store::new(layout_value),
                pairing: Pairing::new(opts.fixed_pair_code.clone()),
                devices: Devices::new(config_dir.join("devices.json"), devices),
                notif_counter: AtomicU64::new(1),
                listening: Mutex::new(None),
                listen_error: Mutex::new(None),
                server: tokio::sync::Mutex::new(None),
                opts,
            }),
        };

        // One error notification per invalid file (DATA_MODEL.md 1): body = first problem.
        let mut reported = std::collections::HashSet::new();
        for (file, issue) in issues {
            if reported.insert(file.clone()) {
                agent.push_notification(NotifyReq {
                    title: format!("{file} invalid"),
                    body: Some(issue.message.chars().take(MAX_BODY_CHARS).collect()),
                    level: Some(NotifLevel::Error),
                    source: Some("decka".to_string()),
                });
            }
        }

        agent.start_server().await;
        Ok(agent)
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn runner(&self) -> &Arc<dyn ActionRunner> {
        &self.inner.opts.runner
    }

    pub fn fixed_pair_code(&self) -> Option<&str> {
        self.inner.opts.fixed_pair_code.as_deref()
    }

    pub fn pairing(&self) -> &Pairing {
        &self.inner.pairing
    }

    pub fn devices(&self) -> &Devices {
        &self.inner.devices
    }

    /// New pairing code (cancels the previous one). `url` and `qr_svg` are filled in T6.3.
    pub fn start_pairing(&self) -> PairingInfo {
        let (code, expires_at) = self.inner.pairing.start();
        PairingInfo { code, url: String::new(), qr_svg: String::new(), expires_at }
    }

    pub fn cancel_pairing(&self) {
        self.inner.pairing.cancel();
    }

    pub fn list_devices(&self) -> Vec<DeviceInfo> {
        self.inner
            .devices
            .list()
            .into_iter()
            .map(|d| DeviceInfo { id: d.id, name: d.name, created_at: d.created_at, last_seen_at: d.last_seen_at, connected: false })
            .collect()
    }

    pub fn revoke_device(&self, device_id: &str) {
        self.inner.devices.revoke(device_id);
    }

    pub fn config_dir(&self) -> &std::path::Path {
        &self.inner.config_dir
    }

    pub fn config(&self) -> ConfigFile {
        self.inner.config.lock().unwrap().clone()
    }

    pub fn actions(&self) -> ActionsFile {
        self.inner.actions.lock().unwrap().clone()
    }

    pub fn layout(&self) -> LayoutFile {
        self.inner.layout.lock().unwrap().clone()
    }

    /// Adds a notification at the front (newest first, max 20) and returns its id.
    pub fn push_notification(&self, req: NotifyReq) -> String {
        let id = self.inner.notif_counter.fetch_add(1, Ordering::Relaxed).to_string();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let notification = Notification {
            id: id.clone(),
            ts,
            title: req.title,
            body: req.body,
            level: req.level.unwrap_or(NotifLevel::Info),
            source: req.source,
        };
        self.inner.store.update_notifications(|list| {
            list.insert(0, notification);
            list.truncate(MAX_NOTIFICATIONS);
        });
        id
    }

    pub fn status(&self) -> AgentStatus {
        AgentStatus {
            version: crate::VERSION.to_string(),
            listening: self.inner.listening.lock().unwrap().clone(),
            listen_error: self.inner.listen_error.lock().unwrap().clone(),
            lan_ips: lan_ips(),
            clients: Vec::new(),
            usb: UsbStatus { enabled: self.config().usb.enabled, adb_found: false, devices: Vec::new(), last_error: None },
            config_dir: self.inner.config_dir.display().to_string(),
        }
    }

    pub async fn shutdown(&self) {
        self.stop_server().await;
    }

    pub async fn restart_server(&self) -> AgentStatus {
        self.stop_server().await;
        self.start_server().await;
        self.status()
    }

    async fn start_server(&self) {
        let config = self.config();
        let port = self.inner.opts.port_override.unwrap_or(config.port as u16);
        let ip: IpAddr = config.bind.parse().expect("bind validated at start");
        match TcpListener::bind(SocketAddr::new(ip, port)).await {
            Ok(listener) => {
                let local = listener.local_addr().expect("bound socket has an address");
                *self.inner.listening.lock().unwrap() = Some(Listening { bind: config.bind.clone(), port: local.port() });
                *self.inner.listen_error.lock().unwrap() = None;

                let app = crate::server::router(self.clone(), self.inner.opts.client_dist.clone());
                let (tx, rx) = oneshot::channel::<()>();
                let join = tokio::spawn(async move {
                    let serve = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
                        .with_graceful_shutdown(async move {
                            let _ = rx.await;
                        });
                    if let Err(e) = serve.await {
                        tracing::error!("server stopped: {e}");
                    }
                });
                *self.inner.server.lock().await = Some(ServerHandle { shutdown: tx, join });
                tracing::info!("listening on {}:{}", config.bind, local.port());
            }
            Err(e) => {
                *self.inner.listening.lock().unwrap() = None;
                *self.inner.listen_error.lock().unwrap() = Some(e.to_string());
                tracing::error!("cannot listen on {}:{}: {e}", config.bind, port);
            }
        }
    }

    async fn stop_server(&self) {
        if let Some(handle) = self.inner.server.lock().await.take() {
            let _ = handle.shutdown.send(());
            let _ = handle.join.await;
        }
        *self.inner.listening.lock().unwrap() = None;
    }
}

fn is_private_v4(ip: &IpAddr) -> bool {
    matches!(ip, IpAddr::V4(v4) if v4.is_private())
}

/// Private IPv4 addresses of this PC (10/8, 172.16/12, 192.168/16): primary first, at most 4.
pub fn lan_ips() -> Vec<String> {
    let mut ips: Vec<IpAddr> = Vec::new();
    if let Ok(primary) = local_ip_address::local_ip() {
        if is_private_v4(&primary) {
            ips.push(primary);
        }
    }
    if let Ok(list) = local_ip_address::list_afinet_netifas() {
        for (_, ip) in list {
            if is_private_v4(&ip) && !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }
    ips.into_iter().take(4).map(|ip| ip.to_string()).collect()
}
