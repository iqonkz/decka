//! `Agent`: owns config, state store and the HTTP/WS server (ARCHITECTURE.md 4.3).

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, watch};
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
    /// M11: the active app on the PC (exe name), `None` when unknown or the collector is off.
    pub focus_app: Option<String>,
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
    /// `client.version` of the last `pair` / `hello` (M14); absent until the phone connects with a version-aware agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
}

/// `refresh_program_icons` result (PROTOCOL.md 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefreshIcons {
    pub updated: usize,
    pub failed: usize,
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
        self.agent.publish_status();
    }
}

struct ServerHandle {
    shutdown: oneshot::Sender<()>,
    join: JoinHandle<()>,
}

/// Frees the "running" mark of a sequence however its run ends (also when the press is dropped halfway).
struct RunningGuard {
    inner: Arc<Inner>,
    id: String,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.inner.running.lock().unwrap().remove(&self.id);
    }
}

struct Inner {
    opts: AgentOptions,
    config_dir: PathBuf,
    config: Mutex<ConfigFile>,
    actions: Mutex<ActionsFile>,
    /// Ids of the sequences that are running now: a second press of the same key answers `Already running`.
    running: Mutex<std::collections::HashSet<String>>,
    layout: Mutex<LayoutFile>,
    store: Store,
    statuses: crate::statuses::Statuses,
    background: Mutex<Vec<JoinHandle<()>>>,
    sys_collector: Mutex<Option<crate::collectors::sys::SysCollector>>,
    checks_collector: Mutex<Option<crate::collectors::checks::ChecksCollector>>,
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
    status_tx: watch::Sender<AgentStatus>,
    usb_state: Mutex<crate::usb::UsbState>,
    usb_task: Mutex<Option<JoinHandle<()>>>,
    focus_collector: Mutex<Option<crate::collectors::focus::FocusCollector>>,
    media_collector: Mutex<Option<crate::collectors::media::MediaCollector>>,
    audio_collector: Mutex<Option<crate::audio::AudioCollector>>,
    /// The weather refresh loop (M15) and the last good answer (older than 3 h it is dropped).
    weather_task: Mutex<Option<JoinHandle<()>>>,
    weather_last: Mutex<Option<WeatherInfo>>,
    focus_app: Mutex<Option<String>>,
    /// The answer of the last update check (ARCHITECTURE.md 8a).
    update: Mutex<crate::updates::UpdateStatus>,
    /// Sends the new status to the window and the tray when a check finishes.
    update_tx: watch::Sender<crate::updates::UpdateStatus>,
    /// Runtime of `Agent::start`; lets synchronous callers (UI commands) start background tasks.
    runtime: tokio::runtime::Handle,
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
        store.set("icons", crate::icons::collect_icons(&config_dir, &layout));
        let agent = Agent {
            inner: Arc::new(Inner {
                config_dir: config_dir.clone(),
                config: Mutex::new(config),
                actions: Mutex::new(actions),
                running: Mutex::new(Default::default()),
                layout: Mutex::new(layout),
                statuses: crate::statuses::Statuses::new(store.clone()),
                store,
                background: Mutex::new(Vec::new()),
                sys_collector: Mutex::new(None),
                checks_collector: Mutex::new(None),
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
                status_tx: watch::channel(blank_status(&config_dir)).0,
                usb_state: Mutex::new(crate::usb::UsbState::default()),
                usb_task: Mutex::new(None),
                focus_collector: Mutex::new(None),
                media_collector: Mutex::new(None),
                audio_collector: Mutex::new(None),
                weather_task: Mutex::new(None),
                weather_last: Mutex::new(None),
                focus_app: Mutex::new(None),
                update: Mutex::new(crate::updates::UpdateStatus::default()),
                update_tx: watch::channel(crate::updates::UpdateStatus::default()).0,
                runtime: tokio::runtime::Handle::current(),
                opts,
            }),
        };
        agent.publish_status();

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
        agent.restart_usb();
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

        // "Now playing" collector (M11): off = the section is null.
        let old_media = self.inner.media_collector.lock().unwrap().take();
        drop(old_media);
        if config.collectors.media_enabled() {
            *self.inner.media_collector.lock().unwrap() = Some(crate::collectors::media::MediaCollector::start(self.inner.store.clone()));
        } else {
            self.inner.store.set("media", serde_json::Value::Null);
            self.inner.store.update_vars(crate::collectors::media::clear_vars);
        }

        // Sound collector (M15): volume and the mutes of the default devices as variables; off = the variables are gone.
        let old_audio = self.inner.audio_collector.lock().unwrap().take();
        drop(old_audio);
        if config.collectors.audio_enabled() {
            *self.inner.audio_collector.lock().unwrap() = Some(crate::audio::AudioCollector::start(self.inner.store.clone()));
        } else {
            self.inner.store.update_vars(crate::audio::clear_vars);
        }

        // Active-app collector (M11): off = the section is null and the status shows no app.
        let old_focus = self.inner.focus_collector.lock().unwrap().take();
        drop(old_focus);
        if config.collectors.focus_enabled() {
            let weak = Arc::downgrade(&self.inner);
            let on_change: crate::collectors::focus::OnChange = Arc::new(move |app| {
                if let Some(inner) = weak.upgrade() {
                    *inner.focus_app.lock().unwrap() = app;
                    Agent { inner }.publish_status();
                }
            });
            let collector = crate::collectors::focus::FocusCollector::start(self.inner.store.clone(), on_change);
            *self.inner.focus_collector.lock().unwrap() = Some(collector);
        } else {
            self.inner.store.set("focus", serde_json::Value::Null);
            *self.inner.focus_app.lock().unwrap() = None;
            self.publish_status();
        }

        // Weather (M15): a loop on the runtime; off = the section is null.
        self.start_weather();

        // HTTP checks: stop the old tasks (dropping aborts them), then start from the new config.
        let old_checks = self.inner.checks_collector.lock().unwrap().take();
        drop(old_checks);
        let checks = crate::collectors::checks::ChecksCollector::start(&self.inner.runtime, self.inner.statuses.clone(), config.collectors.checks);
        *self.inner.checks_collector.lock().unwrap() = Some(checks);
    }

    /// Stops the USB loop (forgetting every forwarded device) and, when `usb.enabled`, starts it again.
    /// Safe to call from any thread: the task is spawned on the stored runtime handle.
    pub fn restart_usb(&self) {
        if let Some(old) = self.inner.usb_task.lock().unwrap().take() {
            old.abort();
        }
        *self.inner.usb_state.lock().unwrap() = crate::usb::UsbState::default();
        if self.config().usb.enabled {
            let task = crate::usb::spawn(&self.inner.runtime, self.clone());
            *self.inner.usb_task.lock().unwrap() = Some(task);
        }
        self.publish_status();
    }

    pub(crate) fn listening_port(&self) -> Option<u16> {
        self.inner.listening.lock().unwrap().as_ref().map(|l| l.port)
    }

    pub(crate) fn set_usb_state(&self, state: crate::usb::UsbState) {
        *self.inner.usb_state.lock().unwrap() = state;
        self.publish_status();
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
        self.publish_status();
        (ConnGuard { agent: self.clone(), id }, kicked)
    }

    pub fn devices(&self) -> &Devices {
        &self.inner.devices
    }

    /// New pairing code (cancels the previous one) with the URL and QR code for it.
    pub fn start_pairing(&self) -> PairingInfo {
        let (code, expires_at) = self.inner.pairing.start();
        let config = self.config();
        // USB-only mode or no LAN address: the phone reaches the PC through 127.0.0.1 (adb reverse).
        let mut ips = if config.bind == "127.0.0.1" { Vec::new() } else { lan_ips() };
        if ips.is_empty() {
            ips.push("127.0.0.1".to_string());
        }
        let port = self.inner.listening.lock().unwrap().as_ref().map_or(config.port as u16, |l| l.port);
        let url = crate::qr::pair_url(&ips[0], &ips, port, &code, &config.agent_name);
        let qr_svg = crate::qr::qr_svg(&url);
        PairingInfo { code, url, qr_svg, expires_at }
    }

    /// The answer of the last update check (ARCHITECTURE.md 8a).
    pub fn update_status(&self) -> crate::updates::UpdateStatus {
        self.inner.update.lock().unwrap().clone()
    }

    /// Receives the new update status whenever a check finishes (the window and the tray listen).
    pub fn subscribe_updates(&self) -> watch::Receiver<crate::updates::UpdateStatus> {
        self.inner.update_tx.subscribe()
    }

    /// (Re)starts the weather loop from the config: at once, then every 30 minutes. Safe from any thread (the runtime handle is used).
    fn start_weather(&self) {
        if let Some(old) = self.inner.weather_task.lock().unwrap().take() {
            old.abort();
        }
        *self.inner.weather_last.lock().unwrap() = None;
        let enabled = self.config().weather.as_ref().is_some_and(|w| w.enabled);
        if !enabled {
            self.inner.store.set("weather", serde_json::Value::Null);
            return;
        }
        let me = self.clone();
        let task = self.inner.runtime.spawn(async move {
            loop {
                if let Err(e) = me.refresh_weather().await {
                    tracing::debug!("weather: {e}");
                    // A failed update keeps the old data, until it is 3 hours old.
                    let kept = crate::weather::fresh(me.inner.weather_last.lock().unwrap().clone(), crate::auth::now_ms());
                    if kept.is_none() {
                        me.inner.store.set("weather", serde_json::Value::Null);
                    }
                }
                tokio::time::sleep(crate::weather::REFRESH_EVERY).await;
            }
        });
        *self.inner.weather_task.lock().unwrap() = Some(task);
    }

    /// Asks Open-Meteo for the forecast of the place in the settings now (the `Refresh` button and the loop).
    pub async fn refresh_weather(&self) -> Result<WeatherInfo, String> {
        if !self.config().weather.is_some_and(|w| w.enabled) {
            return Err("Weather is off".to_string());
        }
        self.refresh_weather_at(crate::weather::FORECAST_URL).await
    }

    /// `refresh_weather` against another address (the tests use a local server); needs a place in the settings, on or off.
    pub async fn refresh_weather_at(&self, base: &str) -> Result<WeatherInfo, String> {
        let Some(cfg) = self.config().weather else { return Err("Weather is not set up".to_string()) };
        let body = crate::weather::fetch(&crate::weather::forecast_url(base, cfg.lat, cfg.lon, &cfg.units)).await?;
        let info = crate::weather::parse_forecast(&body, &cfg.place, &cfg.units, crate::auth::now_ms()).ok_or("Unexpected answer from Open-Meteo")?;
        *self.inner.weather_last.lock().unwrap() = Some(info.clone());
        self.inner.store.set("weather", serde_json::to_value(&info).expect("weather serializes"));
        Ok(info)
    }

    /// The place search of the settings (`search_place`): 2..80 characters, up to 5 results; the answer is in the language of the deck.
    pub async fn search_place(&self, query: &str) -> Result<Vec<crate::weather::Place>, String> {
        self.search_place_at(crate::weather::GEOCODING_URL, query).await
    }

    pub async fn search_place_at(&self, base: &str, query: &str) -> Result<Vec<crate::weather::Place>, String> {
        let query = query.trim();
        if !(2..=80).contains(&query.chars().count()) {
            return Err("Type 2 to 80 characters".to_string());
        }
        let language = if self.layout().display.language.as_deref() == Some("ru") { "ru" } else { "en" };
        let body = crate::weather::fetch(&crate::weather::geocoding_url(base, query, language)).await?;
        Ok(crate::weather::parse_places(&body))
    }

    /// Asks GitHub for the latest release once (on request: `Check now`, opening `About`) and keeps the answer.
    pub async fn check_updates(&self) -> crate::updates::UpdateStatus {
        self.check_updates_at(crate::updates::LATEST_URL).await
    }

    /// `check_updates` against another address (the tests use a local server).
    pub async fn check_updates_at(&self, url: &str) -> crate::updates::UpdateStatus {
        let result = match crate::updates::fetch_latest(url).await {
            Ok(body) => crate::updates::newer_release(&body, crate::VERSION).map_err(|_| ()),
            Err(()) => Err(()),
        };
        let status = match result {
            Ok(available) => crate::updates::UpdateStatus { available, checked_at: Some(crate::auth::now_ms()), failed: false },
            // A failed check keeps what was known before (a banner does not vanish because the network blinked).
            Err(()) => {
                let before = self.update_status();
                crate::updates::UpdateStatus { available: before.available, checked_at: Some(crate::auth::now_ms()), failed: true }
            }
        };
        *self.inner.update.lock().unwrap() = status.clone();
        self.inner.update_tx.send_replace(status.clone());
        status
    }

    /// The daily check: first one 45 s after the start, then every 24 h, only while `config.updateCheck` is not `false`.
    /// Runs on the runtime of `Agent::start`, so it is safe to call from a thread without one (a Tauri command or `setup`).
    pub fn start_update_loop(&self) {
        let for_task = self.clone();
        let task = self.inner.runtime.spawn(async move {
            tokio::time::sleep(Duration::from_secs(45)).await;
            loop {
                if for_task.config().update_check != Some(false) {
                    for_task.check_updates().await;
                }
                tokio::time::sleep(crate::updates::CHECK_EVERY).await;
            }
        });
        self.inner.background.lock().unwrap().push(task);
    }

    /// Opens the page of a release in the browser; only addresses under the project's releases are accepted.
    pub fn open_release(&self, url: &str) -> Result<(), String> {
        if !url.starts_with(crate::updates::RELEASES_PREFIX) {
            return Err("not a release page of Decka".to_string());
        }
        crate::actions::process::open_url(url)
    }

    /// Latest status; the receiver updates on connect/disconnect, server restart and config changes.
    pub fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.inner.status_tx.subscribe()
    }

    /// Recomputes the status and wakes subscribers if it changed.
    pub(crate) fn publish_status(&self) {
        let new = self.status();
        self.inner.status_tx.send_if_modified(|current| {
            if *current == new {
                false
            } else {
                *current = new;
                true
            }
        });
    }

    fn write_config(&self, config: ConfigFile) -> SaveResult {
        let rejected = |message: String| SaveResult::Rejected(vec![ValidationIssue { path: String::new(), message }]);
        let text = match json_text(&config) {
            Ok(t) => t,
            Err(e) => return rejected(e.to_string()),
        };
        if let Err(e) = save_text_atomic(&self.inner.config_dir.join("config.json"), &text) {
            return rejected(format!("cannot write config.json: {e}"));
        }
        *self.inner.config.lock().unwrap() = config;
        SaveResult::Saved
    }

    /// Validates and writes config.json, then restarts the collectors. `port` and `bind` take effect
    /// only after `restart_server`.
    pub fn save_config(&self, mut config: ConfigFile) -> SaveResult {
        // The OBS password is written only by `set_obs_password`: whatever the caller sends is ignored, the stored one stays.
        let stored = self.config().obs.and_then(|o| o.password);
        if let Some(obs) = config.obs.as_mut() {
            obs.password = stored;
            obs.password_set = None;
        }
        let issues = crate::validate::validate_config(&config);
        if !issues.is_empty() {
            return SaveResult::Rejected(issues);
        }
        let result = self.write_config(config);
        if result == SaveResult::Saved {
            self.restart_collectors();
            self.restart_usb();
            self.publish_status();
        }
        result
    }

    /// Replaces the API token; scripts that use the old one stop working immediately.
    pub fn regenerate_api_token(&self) -> Result<String, String> {
        let token = config::generate_api_token();
        let mut config = self.config();
        config.api_token = token.clone();
        match self.write_config(config) {
            SaveResult::Saved => Ok(token),
            SaveResult::Rejected(issues) => Err(issues.first().map(|i| i.message.clone()).unwrap_or_default()),
        }
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
                DeviceInfo { id: d.id, name: d.name, created_at: d.created_at, last_seen_at: d.last_seen_at, connected: is_connected, app_version: d.app_version }
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

    /// The config as the settings window sees it: the OBS password is left out and `passwordSet` says whether there is one.
    pub fn config_for_window(&self) -> ConfigFile {
        let mut config = self.config();
        if let Some(obs) = config.obs.as_mut() {
            obs.password_set = Some(obs.password.is_some());
            obs.password = None;
        }
        config
    }

    pub fn actions(&self) -> ActionsFile {
        self.inner.actions.lock().unwrap().clone()
    }

    pub fn layout(&self) -> LayoutFile {
        self.inner.layout.lock().unwrap().clone()
    }

    /// Runs an action by id: runner on the blocking pool, then `toggleVar` on success. A `sequence` runs its steps one by one
    /// (DATA_MODEL.md 2.3) and answers `Already running` to a second press while it is still going.
    pub async fn execute(&self, action_id: &str) -> Result<(), (ErrorCode, String)> {
        let action = self.inner.actions.lock().unwrap().actions.get(action_id).cloned();
        let Some(action) = action else {
            return Err((ErrorCode::UnknownAction, format!("action '{action_id}' is not defined")));
        };
        if let Action::Sequence { steps, .. } = &action {
            let toggle = action.toggle_var().map(str::to_string);
            if !self.inner.running.lock().unwrap().insert(action_id.to_string()) {
                return Err((ErrorCode::ActionFailed, "Already running".to_string()));
            }
            let _guard = RunningGuard { inner: self.inner.clone(), id: action_id.to_string() };
            for (index, step) in steps.iter().enumerate() {
                self.run_step(step).await.map_err(|(code, message)| (code, format!("Step {}: {message}", index + 1)))?;
            }
            self.toggle_var(toggle);
            return Ok(());
        }
        self.run_action(action_id, action).await
    }

    /// One step of a sequence: wait, or run the action and wait for its result.
    async fn run_step(&self, step: &SequenceStep) -> Result<(), (ErrorCode, String)> {
        match step {
            SequenceStep::Wait { delay_ms } => {
                tokio::time::sleep(std::time::Duration::from_millis((*delay_ms).max(0) as u64)).await;
                Ok(())
            }
            SequenceStep::Run { action_id } => {
                let action = self.inner.actions.lock().unwrap().actions.get(action_id).cloned();
                match action {
                    None => Err((ErrorCode::UnknownAction, format!("action '{action_id}' is not defined"))),
                    // The file was edited after it was validated; a sequence never starts another one.
                    Some(Action::Sequence { .. }) => Err((ErrorCode::ActionFailed, "a sequence cannot contain another sequence".to_string())),
                    Some(action) => self.run_action(action_id, action).await,
                }
            }
        }
    }

    /// The runner on the blocking pool, then `toggleVar` on success.
    async fn run_action(&self, action_id: &str, action: Action) -> Result<(), (ErrorCode, String)> {
        let runner = self.inner.opts.runner.clone();
        let id = action_id.to_string();
        let toggle = action.toggle_var().map(str::to_string);
        let result = tokio::task::spawn_blocking(move || runner.run(&id, &action))
            .await
            .map_err(|e| (ErrorCode::Internal, format!("action task failed: {e}")))?;
        result.map_err(|msg| (ErrorCode::ActionFailed, msg))?;
        self.toggle_var(toggle);
        Ok(())
    }

    fn toggle_var(&self, toggle: Option<String>) {
        if let Some(var) = toggle {
            self.inner.store.update_vars(|vars| {
                let next = !matches!(vars.get(&var), Some(VarValue::Bool(true)));
                vars.insert(var, VarValue::Bool(next));
            });
        }
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
        let icons = crate::icons::collect_icons(&self.inner.config_dir, &layout);
        *self.inner.layout.lock().unwrap() = layout;
        self.inner.store.set("layout", value);
        self.inner.store.set("icons", icons);
    }

    /// Layout issues of the first kind that need the disk: `img:` icons must have their files (DATA_MODEL.md 8.2).
    fn icon_issues(&self, layout: &LayoutFile) -> Vec<ValidationIssue> {
        crate::icons::missing_icon_issues(&self.inner.config_dir, layout)
    }

    /// Icon of a program (`.exe`) as `img:<hash16>`; blocks (Shell + imaging), call it from a blocking context.
    pub fn icon_from_program(&self, path: &str) -> Result<String, String> {
        crate::icons::icon_from_program(&self.inner.config_dir, path)
    }

    /// Icon from a picture file as `img:<hash16>`; blocks, call it from a blocking context.
    pub fn icon_from_image(&self, path: &str) -> Result<String, String> {
        crate::icons::icon_from_image(&self.inner.config_dir, path)
    }

    /// Buttons of programs whose icon is a PNG narrower than 256 px (the `Sharpen` line of the `Look` tab, DESIGN.md 2.1).
    pub fn count_lowres_icons(&self) -> usize {
        crate::icons::count_lowres_icons(&self.inner.config_dir, &self.layout(), &self.actions())
    }

    /// Extracts the icons of low-resolution program buttons again at 256 px and saves the layout when something changed
    /// (DATA_MODEL.md 8.1). Blocks (Shell calls): call it from a blocking context.
    pub fn refresh_program_icons(&self) -> RefreshIcons {
        let mut layout = self.layout();
        let (updated, mut failed) = crate::icons::refresh_program_icons(&self.inner.config_dir, &mut layout, &self.actions());
        if updated > 0 && !matches!(self.save_layout(layout), SaveResult::Saved) {
            failed += updated;
            return RefreshIcons { updated: 0, failed };
        }
        RefreshIcons { updated, failed }
    }

    /// Data URL of a stored `img:` icon for the settings window.
    pub fn icon_preview(&self, icon: &str) -> Option<String> {
        crate::icons::icon_preview(&self.inner.config_dir, icon)
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
                Ok(mut actions) => {
                    config::migrate_actions(&mut actions);
                    match crate::validate::validate_actions(&actions).first() {
                        Some(issue) => self.invalid_file_notification(name, &issue.message),
                        // A dangling button reference is allowed: the press answers UNKNOWN_ACTION.
                        None => *self.inner.actions.lock().unwrap() = actions,
                    }
                }
            },
            "layout.json" => match serde_json::from_str::<LayoutFile>(&text) {
                Err(e) => self.invalid_file_notification(name, &format!("invalid JSON: {e}")),
                Ok(layout) => match crate::validate::validate_layout(&layout, &self.actions()).into_iter().chain(self.icon_issues(&layout)).next().as_ref() {
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
        let mut issues = crate::validate::validate_layout(&layout, &self.actions());
        issues.extend(self.icon_issues(&layout));
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

    /// First run (DESIGN.md 4.4): no phone is paired yet and the layout is still the one Decka created.
    pub fn is_first_run(&self) -> bool {
        self.list_devices().is_empty() && self.layout() == crate::config::default_layout()
    }

    pub fn get_templates(&self) -> Vec<crate::templates::PageTemplate> {
        crate::templates::all()
    }

    /// Adds a template's actions (existing ids are kept) and its page, then saves actions.json and layout.json.
    pub fn add_template(&self, template_id: &str) -> SaveResult {
        let (actions, layout) = match crate::templates::build(template_id, &self.layout(), &self.actions()) {
            Ok(built) => built,
            Err(issues) => return SaveResult::Rejected(issues),
        };
        let saved = self.save_actions(actions);
        if saved != SaveResult::Saved {
            return saved;
        }
        self.save_layout(layout)
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
            usb: {
                let state = self.inner.usb_state.lock().unwrap().clone();
                UsbStatus { enabled: self.config().usb.enabled, adb_found: state.adb_found, devices: state.devices, last_error: state.last_error }
            },
            focus_app: self.inner.focus_app.lock().unwrap().clone(),
            config_dir: self.inner.config_dir.display().to_string(),
        }
    }

    pub async fn shutdown(&self) {
        let focus = self.inner.focus_collector.lock().unwrap().take();
        drop(focus); // joins the thread
        let media = self.inner.media_collector.lock().unwrap().take();
        drop(media);
        let audio = self.inner.audio_collector.lock().unwrap().take();
        drop(audio);
        if let Some(task) = self.inner.weather_task.lock().unwrap().take() {
            task.abort();
        }
        let watcher = self.inner.watcher.lock().unwrap().take();
        drop(watcher);
        if let Some(usb) = self.inner.usb_task.lock().unwrap().take() {
            usb.abort();
        }
        for task in self.inner.background.lock().unwrap().drain(..) {
            task.abort();
        }
        let collector = self.inner.sys_collector.lock().unwrap().take();
        drop(collector); // joins the thread
        let checks = self.inner.checks_collector.lock().unwrap().take();
        drop(checks);
        self.stop_server().await;
    }

    pub async fn restart_server(&self) -> AgentStatus {
        self.stop_server().await;
        self.start_server().await;
        self.restart_usb(); // the port may have changed
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
        self.publish_status();
    }

    async fn stop_server(&self) {
        if let Some(handle) = self.inner.server.lock().await.take() {
            let _ = handle.shutdown.send(());
            let _ = handle.join.await;
        }
        *self.inner.listening.lock().unwrap() = None;
        self.publish_status();
    }
}

/// Placeholder for the status channel's initial value; replaced right after construction.
fn blank_status(config_dir: &std::path::Path) -> AgentStatus {
    AgentStatus {
        version: crate::VERSION.to_string(),
        listening: None,
        listen_error: None,
        lan_ips: Vec::new(),
        clients: Vec::new(),
        usb: UsbStatus { enabled: false, adb_found: false, devices: Vec::new(), last_error: None },
        focus_app: None,
        config_dir: config_dir.display().to_string(),
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
