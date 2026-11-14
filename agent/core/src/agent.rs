//! `Agent`: owns config, state store and the HTTP/WS server (ARCHITECTURE.md 4.3).

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::actions::ActionRunner;
use crate::auth::{Devices, Pairing};
use crate::config::{self, json_text, save_text_atomic, Loaded};
use crate::model::*;
use crate::proto::{ErrorCode, NotifyReq};
use crate::ratelimit::RateLimiter;
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

/// Mirrors the `test_action` result in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestActionResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of `save_*`; serializes like `SaveResult` in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveResult {
    Saved,
    Rejected(Vec<ValidationIssue>),
}

impl Serialize for SaveResult {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            SaveResult::Saved => serde_json::json!({ "ok": true }).serialize(s),
            SaveResult::Rejected(issues) => serde_json::json!({ "ok": false, "issues": issues }).serialize(s),
        }
    }
}

/// A live authorized WebSocket connection.
struct Conn {
    device_id: String,
    name: String,
    remote: String,
    connected_at: u64,
    kick: Option<oneshot::Sender<()>>,
}

/// Removes the connection from the registry when dropped.
pub struct ConnGuard {
    agent: Agent,
    id: u64,
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.agent.inner.conns.lock().unwrap().remove(&self.id);
    }
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
    statuses: crate::statuses::Statuses,
    background: Mutex<Vec<JoinHandle<()>>>,
    sys_collector: Mutex<Option<crate::collectors::sys::SysCollector>>,
    pairing: Pairing,
    devices: Devices,
    notif_counter: AtomicU64,
    pair_limiter: RateLimiter,
    api_guard: crate::server::api_auth::ApiGuard,
    conns: Mutex<HashMap<u64, Conn>>,
    conn_counter: AtomicU64,
    listening: Mutex<Option<Listening>>,
    listen_error: Mutex<Option<String>>,
    server: tokio::sync::Mutex<Option<ServerHandle>>,
    /// Last content of each watched file that the agent applied or wrote itself.
    applied: Mutex<HashMap<&'static str, String>>,
    watcher: Mutex<Option<crate::watch::WatchHandle>>,
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
        let store = Store::new(layout_value);
        let agent = Agent {
            inner: Arc::new(Inner {
                config_dir: config_dir.clone(),
                config: Mutex::new(config),
                actions: Mutex::new(actions),
                layout: Mutex::new(layout),
                statuses: crate::statuses::Statuses::new(store.clone()),
                store,
                background: Mutex::new(Vec::new()),
                sys_collector: Mutex::new(None),
                pairing: Pairing::new(opts.fixed_pair_code.clone()),
                devices: Devices::new(config_dir.join("devices.json"), devices),
                notif_counter: AtomicU64::new(1),
                pair_limiter: RateLimiter::new(10, Duration::from_secs(60)),
                api_guard: crate::server::api_auth::ApiGuard::new(),
                conns: Mutex::new(HashMap::new()),
                conn_counter: AtomicU64::new(1),
                listening: Mutex::new(None),
                listen_error: Mutex::new(None),
                server: tokio::sync::Mutex::new(None),
                applied: Mutex::new(HashMap::new()),
                watcher: Mutex::new(None),
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

        {
            let mut applied = agent.inner.applied.lock().unwrap();
            for name in crate::watch::WATCHED {
                if let Ok(text) = std::fs::read_to_string(agent.inner.config_dir.join(name)) {
                    applied.insert(name, text.strip_prefix('\u{feff}').unwrap_or(&text).to_string());
                }
            }
        }
        // Once a second: statuses whose TTL ran out become "stale".
        {
            let for_task = agent.clone();
            let ticker = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    for_task.inner.statuses.expire(std::time::Instant::now());
                }
            });
            agent.inner.background.lock().unwrap().push(ticker);
        }
        agent.restart_collectors();
        agent.start_server().await;
        match crate::watch::spawn(&agent) {
            Ok(handle) => *agent.inner.watcher.lock().unwrap() = Some(handle),
            Err(e) => tracing::warn!("cannot watch the config directory: {e}"),
        }
        Ok(agent)
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    /// Stops the running collectors and starts them again from the current config.
    pub fn restart_collectors(&self) {
        let config = self.config();
        let old = self.inner.sys_collector.lock().unwrap().take();
        drop(old); // joins the old thread before a new one starts
        let new = if config.collectors.sys.enabled {
            Some(crate::collectors::sys::SysCollector::start(
                self.inner.store.clone(),
                config.collectors.sys.interval_sec.max(1) as u64,
            ))
        } else {
            self.inner.store.set("sys", serde_json::Value::Null);
            None
        };
        *self.inner.sys_collector.lock().unwrap() = new;
    }

    pub fn statuses(&self) -> &crate::statuses::Statuses {
        &self.inner.statuses
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

    pub fn api_guard(&self) -> &crate::server::api_auth::ApiGuard {
        &self.inner.api_guard
    }

    pub fn pair_limiter(&self) -> &RateLimiter {
        &self.inner.pair_limiter
    }

    /// Adds a live connection to the registry. The receiver fires when the device is revoked.
    pub(crate) fn register_connection(&self, device_id: &str, name: &str, remote: SocketAddr) -> (ConnGuard, oneshot::Receiver<()>) {
        let id = self.inner.conn_counter.fetch_add(1, Ordering::Relaxed);
        let (kick, kicked) = oneshot::channel();
        let conn = Conn {
            device_id: device_id.to_string(),
            name: name.to_string(),
            remote: remote.to_string(),
            connected_at: crate::auth::now_ms(),
            kick: Some(kick),
        };
        self.inner.conns.lock().unwrap().insert(id, conn);
        (ConnGuard { agent: self.clone(), id }, kicked)
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
        let connected: std::collections::HashSet<String> =
            self.inner.conns.lock().unwrap().values().map(|c| c.device_id.clone()).collect();
        self.inner
            .devices
            .list()
            .into_iter()
            .map(|d| {
                let is_connected = connected.contains(&d.id);
                DeviceInfo { id: d.id, name: d.name, created_at: d.created_at, last_seen_at: d.last_seen_at, connected: is_connected }
            })
            .collect()
    }

    /// Removes the device and closes its open sockets with DEVICE_REVOKED.
    pub fn revoke_device(&self, device_id: &str) {
        self.inner.devices.revoke(device_id);
        for conn in self.inner.conns.lock().unwrap().values_mut() {
            if conn.device_id == device_id {
                if let Some(kick) = conn.kick.take() {
                    let _ = kick.send(());
                }
            }
        }
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

    /// Runs an action by id: runner on the blocking pool, then `toggleVar` on success.
    pub async fn execute(&self, action_id: &str) -> Result<(), (ErrorCode, String)> {
        let action = self.inner.actions.lock().unwrap().actions.get(action_id).cloned();
        let Some(action) = action else {
            return Err((ErrorCode::UnknownAction, format!("action '{action_id}' is not defined")));
        };
        let runner = self.inner.opts.runner.clone();
        let id = action_id.to_string();
        let toggle = action.toggle_var().map(str::to_string);
        let result = tokio::task::spawn_blocking(move || runner.run(&id, &action))
            .await
            .map_err(|e| (ErrorCode::Internal, format!("action task failed: {e}")))?;
        result.map_err(|msg| (ErrorCode::ActionFailed, msg))?;
        if let Some(var) = toggle {
            self.inner.store.update_vars(|vars| {
                let next = !matches!(vars.get(&var), Some(VarValue::Bool(true)));
                vars.insert(var, VarValue::Bool(next));
            });
        }
        Ok(())
    }

    /// Same as `execute`, shaped like the `test_action` IPC result.
    pub async fn test_action(&self, action_id: &str) -> TestActionResult {
        match self.execute(action_id).await {
            Ok(()) => TestActionResult { ok: true, error: None },
            Err((_, message)) => TestActionResult { ok: false, error: Some(message) },
        }
    }

    fn invalid_file_notification(&self, file: &str, body: &str) {
        self.push_notification(NotifyReq {
            title: format!("{file} invalid"),
            body: Some(body.chars().take(MAX_BODY_CHARS).collect()),
            level: Some(NotifLevel::Error),
            source: Some("decka".to_string()),
        });
    }

    fn apply_layout(&self, layout: LayoutFile) {
        let value = serde_json::to_value(layout.to_client()).expect("layout serializes");
        *self.inner.layout.lock().unwrap() = layout;
        self.inner.store.set("layout", value);
    }

    /// Called by the file watcher after the debounce: re-reads one file and applies it if valid.
    pub(crate) fn reload_file(&self, name: &'static str) {
        let Ok(text) = std::fs::read_to_string(self.inner.config_dir.join(name)) else { return };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
        {
            let mut applied = self.inner.applied.lock().unwrap();
            if applied.get(name) == Some(&text) {
                return; // unchanged, or our own write
            }
            applied.insert(name, text.clone());
        }
        match name {
            "actions.json" => match serde_json::from_str::<ActionsFile>(&text) {
                Err(e) => self.invalid_file_notification(name, &format!("invalid JSON: {e}")),
                Ok(actions) => match crate::validate::validate_actions(&actions).first() {
                    Some(issue) => self.invalid_file_notification(name, &issue.message),
                    // A dangling button reference is allowed: the press answers UNKNOWN_ACTION.
                    None => *self.inner.actions.lock().unwrap() = actions,
                },
            },
            "layout.json" => match serde_json::from_str::<LayoutFile>(&text) {
                Err(e) => self.invalid_file_notification(name, &format!("invalid JSON: {e}")),
                Ok(layout) => match crate::validate::validate_layout(&layout, &self.actions()).first() {
                    Some(issue) => self.invalid_file_notification(name, &issue.message),
                    None => self.apply_layout(layout),
                },
            },
            _ => {}
        }
    }

    /// Writes a watched file atomically; the text is recorded first so the watcher ignores our own write.
    fn write_watched(&self, name: &'static str, text: &str) -> Result<(), SaveResult> {
        let previous = self.inner.applied.lock().unwrap().insert(name, text.to_string());
        save_text_atomic(&self.inner.config_dir.join(name), text).map_err(|e| {
            let mut applied = self.inner.applied.lock().unwrap();
            match previous {
                Some(p) => applied.insert(name, p),
                None => applied.remove(name),
            };
            SaveResult::Rejected(vec![ValidationIssue { path: String::new(), message: format!("cannot write {name}: {e}") }])
        })
    }

    /// Validates the actions themselves (not button references), writes actions.json, updates memory.
    pub fn save_actions(&self, actions: ActionsFile) -> SaveResult {
        let issues = crate::validate::validate_actions(&actions);
        if !issues.is_empty() {
            return SaveResult::Rejected(issues);
        }
        let text = match json_text(&actions) {
            Ok(t) => t,
            Err(e) => return SaveResult::Rejected(vec![ValidationIssue { path: String::new(), message: e.to_string() }]),
        };
        if let Err(rejected) = self.write_watched("actions.json", &text) {
            return rejected;
        }
        *self.inner.actions.lock().unwrap() = actions;
        SaveResult::Saved
    }

    /// Validates against the current actions, writes layout.json, updates memory and the layout section.
    pub fn save_layout(&self, layout: LayoutFile) -> SaveResult {
        let issues = crate::validate::validate_layout(&layout, &self.actions());
        if !issues.is_empty() {
            return SaveResult::Rejected(issues);
        }
        let text = match json_text(&layout) {
            Ok(t) => t,
            Err(e) => return SaveResult::Rejected(vec![ValidationIssue { path: String::new(), message: e.to_string() }]),
        };
        if let Err(rejected) = self.write_watched("layout.json", &text) {
            return rejected;
        }
        self.apply_layout(layout);
        SaveResult::Saved
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
            clients: self
                .inner
                .conns
                .lock()
                .unwrap()
                .values()
                .map(|c| ClientStatus { device_id: c.device_id.clone(), name: c.name.clone(), remote: c.remote.clone(), connected_at: c.connected_at })
                .collect(),
            usb: UsbStatus { enabled: self.config().usb.enabled, adb_found: false, devices: Vec::new(), last_error: None },
            config_dir: self.inner.config_dir.display().to_string(),
        }
    }

    pub async fn shutdown(&self) {
        let watcher = self.inner.watcher.lock().unwrap().take();
        drop(watcher);
        for task in self.inner.background.lock().unwrap().drain(..) {
            task.abort();
        }
        let collector = self.inner.sys_collector.lock().unwrap().take();
        drop(collector); // joins the thread
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
