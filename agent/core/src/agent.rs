//! `Agent`: owns config, state store and the HTTP/WS server (ARCHITECTURE.md 4.3).

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::actions::ActionRunner;
use crate::auth::{Devices, Pairing};
use crate::safety::{DangerGate, PhoneCtx};
use crate::config::{self, json_text, save_text_atomic, Loaded};
use crate::model::*;
use crate::proto::{ErrorCode, NotifyReq};
use crate::ratelimit::RateLimiter;
use crate::state::{Store, MAX_NOTIFICATIONS};

/// Notification body limit (PROTOCOL.md 5).
const MAX_BODY_CHARS: usize = 500;

/// The `ai` section of a PC where the coding helpers were never turned on: it only carries what the agent notes there.
fn ai_off() -> AiConfig {
    AiConfig {
        claude: AiClaudeConfig { enabled: false, approvals: true, timeout_sec: 120, account_limits: false },
        codex: AiCodexConfig { enabled: false, approvals: None },
        hook_token: None,
        rules: Vec::new(),
        replies: None,
        gpu_layers: None,
    }
}

impl crate::ai::service::LayerMemory for Agent {
    fn load(&self) -> Option<GpuLayers> {
        self.config().ai.and_then(|a| a.gpu_layers)
    }
    fn save(&self, value: Option<GpuLayers>) {
        self.set_gpu_layers(value);
    }
}

pub struct AgentOptions {
    pub config_dir: Option<PathBuf>,
    pub client_dist: Option<PathBuf>,
    pub port_override: Option<u16>,
    pub runner: Arc<dyn ActionRunner>,
    /// Headless/tests only: a pairing code that stays valid and is never consumed.
    pub fixed_pair_code: Option<String>,
    /// Tests only: listen on this address instead of the `bind` of the configuration (the tests use `127.0.0.1`, so that
    /// Windows does not ask the owner to allow every test program through the firewall).
    pub bind_override: Option<String>,
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

/// Who keeps the port of the agent busy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenBusy {
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
}

/// The port is retried for this long when it is busy (the old copy of the agent may still be closing after an update).
pub const BIND_RETRY_TOTAL: Duration = Duration::from_secs(15);

/// Binds `addr`; while the address is in use, tries again after `first`, twice as long, ... (at most 2 s) until `total` has passed.
pub async fn bind_retrying(addr: SocketAddr, total: Duration, first: Duration) -> std::io::Result<TcpListener> {
    let started = std::time::Instant::now();
    let mut delay = first;
    loop {
        match TcpListener::bind(addr).await {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && started.elapsed() + delay <= total => {
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(2));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Mirrors `AgentStatus` in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub version: String,
    pub listening: Option<Listening>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_error: Option<String>,
    /// The port is held by another program (T21.16): the name of its exe when it is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_busy: Option<ListenBusy>,
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
    /// M18: may this phone answer Claude Code questions (DATA_MODEL.md 11.3).
    pub can_approve: bool,
}

/// `refresh_program_icons` result (PROTOCOL.md 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefreshIcons {
    pub updated: usize,
    pub failed: usize,
}

/// How many phone notifications can still be opened by `notif_open`.
const WIN_TARGETS: usize = 100;

/// Mirrors `NetworkStatus` in shared/src/ipc.ts (DATA_MODEL.md 11.2): what kind of network the phones use now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    /// `usb` (only this PC is reachable), `private` or `public`.
    pub kind: String,
    pub allow_public: bool,
}

/// Mirrors `WindowsNotificationsStatus` in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WinNotifStatus {
    pub enabled: bool,
    /// `allowed`, `denied` (blocked in Windows settings) or `unknown`.
    pub access: String,
    pub apps: Vec<String>,
}

/// Mirrors `ObsStatus` in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObsStatusInfo {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub scenes: Vec<String>,
    pub inputs: Vec<String>,
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

/// What the window shows about YouTube.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeStatus {
    pub enabled: bool,
    pub signed_in: bool,
    /// The client secret is stored (its value never leaves the agent).
    pub secret_set: bool,
    pub channel: Option<String>,
    pub error: Option<String>,
    /// A sign-in in progress: the code to type on google.com/device.
    pub pending: Option<crate::youtube::YoutubeLogin>,
}

/// What the window shows about Twitch.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchStatus {
    pub enabled: bool,
    pub signed_in: bool,
    pub login: Option<String>,
    /// The error of the sign-in, or of the last look at the channel.
    pub error: Option<String>,
    /// A sign-in in progress: the code to type on twitch.tv/activate.
    pub pending: Option<crate::twitch::TwitchLogin>,
}

/// What the window shows about Claude Code and Codex (PROTOCOL.md 9, `CcStatus`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcStatus {
    pub installed: bool,
    pub status_line_conflict: bool,
    pub last_seen_at: Option<u64>,
    pub codex_found: bool,
    /// Our hook is in `%USERPROFILE%\.codex\hooks.json` (Codex asks for a trust review of it with `/hooks`).
    pub codex_hook: bool,
}

/// Why `/api/cc/*` refused a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcDenied {
    /// Not from this PC.
    NotFound,
    Unauthorized,
    RateLimited,
}

/// What the connection to Home Assistant runs with: the address, the token as stored and the watched entities.
type HaApplied = (String, String, std::collections::BTreeSet<String>);

/// Home Assistant as the settings window shows it (`ha_get`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HaStatus {
    pub url: String,
    pub has_token: bool,
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub entities: Vec<HaChoice>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HaChoice {
    pub id: String,
    pub name: String,
    /// Chosen for voice control and the state of buttons.
    pub enabled: bool,
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
        let gone = {
            let mut conns = self.agent.inner.conns.lock().unwrap();
            let gone = conns.remove(&self.id).map(|c| c.device_id);
            gone.filter(|d| !conns.values().any(|c| c.device_id == *d))
        };
        // T30.3: the charge of a phone that left is not shown any more.
        if let Some(device) = gone {
            self.agent.inner.battery.forget(&device);
        }
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
    /// The collectors the layout needed when the collectors were last (re)started; `None` before the first start.
    needed: Mutex<Option<crate::collectors::needed::Needed>>,
    /// The app strip now (DATA_MODEL.md 10.5) and the counter that lets a newer focus change cancel a pending recount.
    strip: Mutex<Option<crate::model::StripInfo>>,
    strip_gen: std::sync::atomic::AtomicU64,
    /// The installed programs (read once, without their list icons) and the icon found per app for the strip.
    installed: Mutex<Option<Vec<crate::installed::InstalledApp>>>,
    strip_icons: Mutex<std::collections::HashMap<String, Option<String>>>,
    battery_collector: Mutex<Option<crate::battery::BatteryCollector>>,
    mail_task: Mutex<Option<JoinHandle<()>>>,
    /// `Check now` in the window wakes the mail loop.
    mail_check: Arc<tokio::sync::Notify>,
    /// How the last round went for every mailbox (`mail_status`, T31.2).
    mail_status: Mutex<Vec<crate::mail::MailAccountStatus>>,
    winnotif_collector: Mutex<Option<crate::winnotif::WinNotifCollector>>,
    /// Phone notification id -> (id Windows gave the toast, AUMID of the program); at most `WIN_TARGETS` entries.
    win_targets: Mutex<std::collections::VecDeque<(String, u32, String)>>,
    /// The apps that sent toasts since the start: the list of exceptions in the window offers them.
    win_apps: Mutex<std::collections::BTreeSet<String>>,
    pairing: Pairing,
    devices: Devices,
    notif_counter: AtomicU64,
    pair_limiter: RateLimiter,
    /// Failed v2 `auth` proofs per IP: 5 in a minute block the IP for the rest of it.
    auth_fail_limiter: RateLimiter,
    /// Handshakes from public networks: 5 per minute and IP (DATA_MODEL.md 11.2).
    public_limiter: RateLimiter,
    /// Dangerous system actions from phones: one per 30 s (DATA_MODEL.md 11.4).
    danger_gate: DangerGate,
    /// Network category of the local address a connection arrived on (cached 30 s).
    networks: crate::netpolicy::Networks,
    api_guard: crate::server::api_auth::ApiGuard,
    conns: Mutex<HashMap<u64, Conn>>,
    conn_counter: AtomicU64,
    listening: Mutex<Option<Listening>>,
    listen_error: Mutex<Option<String>>,
    listen_busy: Mutex<Option<ListenBusy>>,
    /// The task that keeps trying to bind a busy port.
    bind_task: Mutex<Option<JoinHandle<()>>>,
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
    /// The connection to OBS Studio (M15), while the integration is on.
    obs_client: Mutex<Option<Arc<crate::obs::ObsClient>>>,
    weather_last: Mutex<Option<WeatherInfo>>,
    /// AI coding helpers (M16): the limits, the sessions and the permission questions.
    ai: Arc<crate::ai::AiHub>,
    /// M21: the entrance of the `voice_*` messages of the phones (a pipeline is attached when voice control is on).
    voice: crate::voice::VoiceHub,
    /// Reminders and lists of the voice assistant (DATA_MODEL.md 13.4).
    assistant: crate::assistant::Assistant,
    /// What the user asked to remember (DATA_MODEL.md 13.6).
    memory: crate::voice::memory::Memory,
    /// M24: lyrics of the current track (off by default).
    lyrics: crate::lyrics::Lyrics,
    /// T26.0: the lyrics window is hidden by voice until shown again or the agent restarts (never written to config).
    lyrics_hidden: AtomicBool,
    /// T29.5: the application draws an island, which shows what plays whether or not a phone has a media widget.
    island: AtomicBool,
    ai_task: Mutex<Option<JoinHandle<()>>>,
    /// Twitch: the polling loop, the sign-in in progress (and its error), the last error and the last good numbers.
    twitch_task: Mutex<Option<JoinHandle<()>>>,
    twitch_login_task: Mutex<Option<JoinHandle<()>>>,
    twitch_login: Mutex<Option<(crate::twitch::TwitchLogin, Option<String>)>>,
    twitch_error: Mutex<Option<String>>,
    twitch_last: Mutex<Option<TwitchInfo>>,
    /// The time of the newest follower seen so far (ms); `None` until the first look.
    twitch_seen: Mutex<Option<u64>>,
    twitch_api: Mutex<crate::twitch::Api>,
    /// YouTube: the same pieces as for Twitch.
    youtube_task: Mutex<Option<JoinHandle<()>>>,
    youtube_login_task: Mutex<Option<JoinHandle<()>>>,
    youtube_login: Mutex<Option<(crate::youtube::YoutubeLogin, Option<String>)>>,
    youtube_error: Mutex<Option<String>>,
    youtube_last: Mutex<Option<YoutubeInfo>>,
    youtube_api: Mutex<crate::youtube::Api>,
    focus_app: Mutex<Option<String>>,
    /// The answer of the last update check (ARCHITECTURE.md 8a).
    update: Mutex<crate::updates::UpdateStatus>,
    /// Sends the new status to the window and the tray when a check finishes.
    update_tx: watch::Sender<crate::updates::UpdateStatus>,
    /// T20.4: the APK of the newer release (downloaded and verified), the folder it lives in and the one-download-per-device gate.
    app_update: Mutex<Option<crate::apk_update::AppUpdate>>,
    updates_dir: PathBuf,
    apk_gate: Mutex<crate::apk_update::DownloadGate>,
    /// M30: the section `battery`, with the charge of the phones (T30.3).
    battery: crate::battery::Hub,
    /// M30: meeting notes (T30.1).
    notes: crate::notes::Notes,
    /// M30: the connection to Home Assistant (T30.8) and what it was started with.
    ha: crate::homeassistant::Hub,
    ha_applied: Mutex<Option<HaApplied>>,
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
        let Loaded { mut config, actions, layout, devices, issues } = config::load_or_init(&config_dir);
        if let Some(b) = &opts.bind_override {
            config.bind = b.clone();
        }
        if config.bind.parse::<IpAddr>().is_err() {
            return Err(StartError::BadBind(config.bind));
        }

        let layout_value = serde_json::to_value(layout.to_client()).expect("layout serializes");
        let store = Store::new(layout_value);
        store.set("icons", crate::icons::collect_icons(&config_dir, &layout));
        let ha_store = store.clone();
        let agent = Agent {
            inner: Arc::new(Inner {
                config_dir: config_dir.clone(),
                config: Mutex::new(config),
                actions: Mutex::new(actions),
                running: Mutex::new(Default::default()),
                layout: Mutex::new(layout),
                statuses: crate::statuses::Statuses::new(store.clone()),
                ai: crate::ai::AiHub::new(store.clone()),
                battery: crate::battery::Hub::new(store.clone()),
                voice: crate::voice::VoiceHub::new(),
                assistant: crate::assistant::Assistant::load(&config_dir),
                memory: crate::voice::memory::Memory::load(&config_dir),
                lyrics: crate::lyrics::Lyrics::new(&config_dir, lyrics_dir(&config_dir, opts.config_dir.is_some())),
                lyrics_hidden: AtomicBool::new(false),
                island: AtomicBool::new(false),
                ai_task: Mutex::new(None),
                twitch_task: Mutex::new(None),
                twitch_login_task: Mutex::new(None),
                twitch_login: Mutex::new(None),
                twitch_error: Mutex::new(None),
                twitch_last: Mutex::new(None),
                twitch_seen: Mutex::new(None),
                twitch_api: Mutex::new(crate::twitch::Api::real()),
                youtube_task: Mutex::new(None),
                youtube_login_task: Mutex::new(None),
                youtube_login: Mutex::new(None),
                youtube_error: Mutex::new(None),
                youtube_last: Mutex::new(None),
                youtube_api: Mutex::new(crate::youtube::Api::real()),
                store,
                background: Mutex::new(Vec::new()),
                sys_collector: Mutex::new(None),
                checks_collector: Mutex::new(None),
                needed: Mutex::new(None),
                strip: Mutex::new(None),
                strip_gen: std::sync::atomic::AtomicU64::new(0),
                installed: Mutex::new(None),
                strip_icons: Mutex::new(std::collections::HashMap::new()),
                battery_collector: Mutex::new(None),
                mail_task: Mutex::new(None),
                mail_check: Arc::new(tokio::sync::Notify::new()),
                mail_status: Mutex::new(Vec::new()),
                winnotif_collector: Mutex::new(None),
                win_targets: Mutex::new(std::collections::VecDeque::new()),
                win_apps: Mutex::new(std::collections::BTreeSet::new()),
                pairing: Pairing::new(opts.fixed_pair_code.clone()),
                devices: Devices::new(config_dir.join("devices.json"), devices),
                notif_counter: AtomicU64::new(1),
                pair_limiter: RateLimiter::new(10, Duration::from_secs(60)),
                auth_fail_limiter: RateLimiter::new(5, Duration::from_secs(60)),
                public_limiter: RateLimiter::new(5, Duration::from_secs(60)),
                danger_gate: DangerGate::default(),
                networks: crate::netpolicy::Networks::default(),
                api_guard: crate::server::api_auth::ApiGuard::new(),
                conns: Mutex::new(HashMap::new()),
                conn_counter: AtomicU64::new(1),
                listening: Mutex::new(None),
                listen_error: Mutex::new(None),
                listen_busy: Mutex::new(None),
                bind_task: Mutex::new(None),
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
                obs_client: Mutex::new(None),
                weather_last: Mutex::new(None),
                focus_app: Mutex::new(None),
                update: Mutex::new(crate::updates::UpdateStatus::default()),
                update_tx: watch::channel(crate::updates::UpdateStatus::default()).0,
                app_update: Mutex::new(None),
                updates_dir: updates_dir(&config_dir, opts.config_dir.is_some()),
                apk_gate: Mutex::new(crate::apk_update::DownloadGate::default()),
                runtime: tokio::runtime::Handle::current(),
                notes: crate::notes::Notes::new(config_dir.join("notes"), tokio::runtime::Handle::current()),
                ha: crate::homeassistant::Hub::new(ha_store),
                ha_applied: Mutex::new(None),
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
                    for_task.inner.battery.expire(std::time::Instant::now());
                }
            });
            agent.inner.background.lock().unwrap().push(ticker);
        }
        // DATA_MODEL.md 13.7: the phone sections of reminders and lists follow the files.
        {
            let for_task = agent.clone();
            let mut rx = agent.inner.assistant.subscribe();
            let publisher = tokio::spawn(async move {
                loop {
                    let (reminders, lists) = for_task.inner.assistant.sections();
                    for_task.inner.store.set("reminders", reminders);
                    for_task.inner.store.set("lists", lists);
                    if rx.changed().await.is_err() {
                        break;
                    }
                }
            });
            agent.inner.background.lock().unwrap().push(publisher);
        }
        // M24 (DATA_MODEL.md 14): the lyrics follow the media session; off = nothing is asked.
        {
            let for_task = agent.clone();
            let mut rx = agent.inner.store.subscribe();
            let task = tokio::spawn(async move {
                for_task.sync_lyrics().await;
                loop {
                    match rx.recv().await {
                        Ok(patch) if patch.section == "media" => for_task.sync_lyrics().await,
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            agent.inner.background.lock().unwrap().push(task);
        }
        // A deck button can follow the recording (`notes.recording`).
        {
            let weak = Arc::downgrade(&agent.inner);
            agent.inner.notes.listen(Arc::new(move |event| {
                use crate::notes::Event;
                let Some(inner) = weak.upgrade() else { return };
                let recording = match event {
                    Event::Started => true,
                    Event::Stopped(_) => false,
                    _ => return,
                };
                inner.store.update_vars(|vars| {
                    vars.insert("notes.recording".to_string(), VarValue::Bool(recording));
                });
            }));
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

    /// M21: where `voice_*` messages of the phones arrive; `set_sink` connects the pipeline.
    pub fn memory(&self) -> &crate::voice::memory::Memory {
        &self.inner.memory
    }

    pub fn lyrics(&self) -> &crate::lyrics::Lyrics {
        &self.inner.lyrics
    }

    /// Whether the lyrics of the current song are looked up (config.media.lyrics.enabled, off by default).
    pub fn lyrics_enabled(&self) -> bool {
        self.config().media.and_then(|m| m.lyrics).is_some_and(|l| l.enabled)
    }

    /// A search for the lyrics of the current track is running (T26.F.13).
    pub fn lyrics_loading(&self) -> bool {
        self.inner.lyrics.is_loading()
    }

    /// Brings the lyrics section in line with the media session and the switch.
    pub async fn sync_lyrics(&self) {
        let media: Option<crate::model::MediaInfo> = serde_json::from_value(self.inner.store.get("media")).ok();
        let value = self.inner.lyrics.section(media.as_ref(), self.lyrics_enabled()).await;
        self.inner.store.set("lyrics", value);
    }

    /// lyrics_set: the switch of the lyrics. Off clears the section at once and nothing more is asked.
    pub fn set_lyrics_enabled(&self, enabled: bool) -> Result<bool, String> {
        let mut config = self.config();
        config.media.get_or_insert_with(Default::default).lyrics.get_or_insert_with(Default::default).enabled = enabled;
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        let me = self.clone();
        self.inner.runtime.spawn(async move { me.sync_lyrics().await });
        Ok(enabled)
    }

    /// `media.lyrics.pcOverlay` (DESIGN.md 4.17): the window of the words on the PC; absent or unknown = `auto`.
    /// The entities whose state is followed: the chosen ones and those of the `ha` actions.
    fn ha_watched(&self, chosen: &[String]) -> std::collections::BTreeSet<String> {
        let mut watched: std::collections::BTreeSet<String> = chosen.iter().cloned().collect();
        watched.extend(self.inner.actions.lock().unwrap().actions.values().filter_map(|a| match a {
            Action::Ha { entity_id, .. } => Some(entity_id.clone()),
            _ => None,
        }));
        watched
    }

    /// Starts, restarts or stops the connection to Home Assistant when the settings or the actions ask for it.
    fn refresh_ha(&self) {
        let wanted: Option<HaApplied> = self.config().ha.map(|ha| (ha.url.clone(), ha.token.clone(), self.ha_watched(&ha.entities)));
        let mut applied = self.inner.ha_applied.lock().unwrap();
        if *applied == wanted {
            return;
        }
        *applied = wanted.clone();
        let Some((url, stored, watched)) = wanted else { return self.inner.ha.stop() };
        match crate::dpapi::unprotect(&stored) {
            Ok(token) => self.inner.ha.start(&self.inner.runtime, url, token, watched),
            Err(e) => {
                tracing::warn!("home assistant: the stored token cannot be read: {e}");
                self.inner.ha.stop();
            }
        }
    }

    fn ha_token(&self) -> Option<(String, String)> {
        let ha = self.config().ha?;
        crate::dpapi::unprotect(&ha.token).ok().map(|t| (ha.url, t))
    }

    /// Runs a service of Home Assistant on one entity (a `ha` action, or the voice).
    pub async fn ha_call(&self, entity: &str, service: crate::model::HaService) -> Result<(), String> {
        let (url, token) = self.ha_token().ok_or(crate::homeassistant::NOT_CONFIGURED)?;
        let entity = entity.to_string();
        tokio::task::spawn_blocking(move || crate::homeassistant::call(&url, &token, &entity, service)).await.map_err(|_| crate::homeassistant::NO_ANSWER.to_string())?
    }

    /// The entities voice control may name: the chosen ones, by the names Home Assistant gives them.
    pub fn ha_voice_entities(&self) -> Vec<(String, String)> {
        let Some(ha) = self.config().ha else { return Vec::new() };
        let known = self.inner.ha.entities();
        ha.entities.iter().map(|id| (id.clone(), known.iter().find(|e| e.id == *id).map_or_else(|| id.clone(), |e| e.name.clone()))).collect()
    }

    /// `ha_get`: the settings with the entities of the server (what it listed last, else asked now).
    pub async fn ha_status(&self) -> HaStatus {
        let config = self.config().ha;
        let (connected, mut error) = self.inner.ha.status();
        let mut listed = self.inner.ha.entities();
        if listed.is_empty() {
            if let Some((url, token)) = self.ha_token() {
                match crate::homeassistant::states(&url, &token).await {
                    Ok(found) => listed = found,
                    Err(e) => error = Some(e),
                }
            }
        }
        let chosen = config.as_ref().map(|h| h.entities.clone()).unwrap_or_default();
        let mut entities: Vec<HaChoice> = listed.iter().map(|e| HaChoice { id: e.id.clone(), name: e.name.clone(), enabled: chosen.contains(&e.id) }).collect();
        // A chosen entity the server does not list stays visible, so that it can be unchosen.
        entities.extend(chosen.iter().filter(|c| !listed.iter().any(|e| e.id == **c)).map(|c| HaChoice { id: c.clone(), name: c.clone(), enabled: true }));
        HaStatus { url: config.as_ref().map(|h| h.url.clone()).unwrap_or_default(), has_token: config.is_some(), connected, error, entities }
    }

    /// `ha_set_config`: an empty token keeps the stored one while the address stays the same (a changed address never gets
    /// the old token); an empty address removes the integration.
    pub fn ha_set_config(&self, url: &str, token: &str, entities: Vec<String>) -> Result<(), String> {
        let mut config = self.config();
        config.ha = if url.trim().is_empty() {
            None
        } else {
            let url = crate::homeassistant::base(url)?;
            let token = match token.trim() {
                "" => config.ha.as_ref().filter(|h| h.url == url).map(|h| h.token.clone()).ok_or("Enter the token of Home Assistant")?,
                new => crate::dpapi::protect(new)?,
            };
            let mut entities = entities;
            entities.sort();
            entities.dedup();
            Some(crate::model::HaConfig { url, token, entities })
        };
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        self.refresh_ha();
        Ok(())
    }

    /// `ha_test`: asks the server for its entities; the number of them. An empty token means the stored one, and only for
    /// the stored address.
    pub async fn ha_test(&self, url: &str, token: &str) -> Result<usize, String> {
        let url = crate::homeassistant::base(url)?;
        let token = match token.trim() {
            "" => self.ha_token().filter(|(stored, _)| *stored == url).map(|(_, t)| t).ok_or("Enter the token of Home Assistant")?,
            given => given.to_string(),
        };
        crate::homeassistant::states(&url, &token).await.map(|found| found.len())
    }

    /// M30 (T30.3): `phone_status` of a paired phone. Data only: shown in the battery section, answered by voice.
    pub fn phone_status(&self, device_id: &str, name: &str, pct: u32, charging: bool) {
        let phone = crate::battery::Phone { name: name.to_string(), pct, charging };
        self.inner.battery.phone(device_id, phone, std::time::Instant::now());
    }

    /// M30 (T30.1): the meeting notes.
    pub fn notes(&self) -> &crate::notes::Notes {
        &self.inner.notes
    }

    /// `config.notes.keepAudio`; off by default.
    pub fn notes_keep_audio(&self) -> bool {
        self.config().notes.is_some_and(|n| n.keep_audio)
    }

    pub fn set_notes_keep_audio(&self, keep: bool) -> Result<bool, String> {
        let mut config = self.config();
        config.notes.get_or_insert_with(Default::default).keep_audio = keep;
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        Ok(keep)
    }

    /// Starts a meeting note. `true`: the first one ever, the user is to be told to warn the others.
    pub fn note_start(&self) -> Result<bool, String> {
        self.inner.notes.start(self.notes_keep_audio())?;
        let mut config = self.config();
        let first = !config.notes.as_ref().is_some_and(|n| n.warned);
        if first {
            config.notes.get_or_insert_with(Default::default).warned = true;
            if let SaveResult::Rejected(issues) = self.write_config(config) {
                tracing::warn!("notes.warned was not saved: {issues:?}");
            }
        }
        Ok(first)
    }

    /// The deck button and the island: stops a running note, else starts one.
    pub fn notes_toggle(&self) -> Result<(), String> {
        if self.inner.notes.stop() {
            return Ok(());
        }
        self.note_start().map(|_| ())
    }

    /// `voice.context.enabled` (T26.7): hints by the active program when the island opens; off by default.
    pub fn voice_context_enabled(&self) -> bool {
        self.config().voice.and_then(|v| v.context).is_some_and(|c| c.enabled)
    }

    /// `voice.notices.enabled` (T27.8): short notes about memory, processor, battery, disk; on by default.
    pub fn voice_notices_enabled(&self) -> bool {
        self.config().voice.and_then(|v| v.notices).is_none_or(|n| n.enabled)
    }

    pub fn set_voice_notices_enabled(&self, enabled: bool) -> Result<bool, String> {
        let mut config = self.config();
        config.voice.get_or_insert_with(Default::default).notices = Some(crate::model::VoiceNoticesConfig { enabled });
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        Ok(enabled)
    }

    /// `config.island` (T29.5) with the defaults filled in.
    pub fn island_settings(&self) -> crate::island::Settings {
        crate::island::Settings::of(self.config().island.as_ref())
    }

    /// Changes what is given and keeps the rest; a word or a height outside the allowed ones is refused.
    pub fn set_island_settings(&self, change: crate::model::IslandConfig) -> Result<crate::island::Settings, String> {
        if change.resting_mode.as_deref().is_some_and(|m| !["bar", "lyrics"].contains(&m))
            || change.bar_position.as_deref().is_some_and(|p| !["left", "center", "right"].contains(&p))
            || change.bar_height.is_some_and(|h| !crate::island::BAR_HEIGHTS.contains(&h))
        {
            return Err("Unknown island setting".into());
        }
        let mut config = self.config();
        let island = config.island.get_or_insert_with(Default::default);
        island.resting_mode = change.resting_mode.or_else(|| island.resting_mode.take());
        island.bar_height = change.bar_height.or(island.bar_height);
        island.bar_position = change.bar_position.or_else(|| island.bar_position.take());
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        Ok(self.island_settings())
    }

    /// `voice.duckMusic` (T29.3): other sounds are turned down while the microphone records; on by default.
    pub fn voice_duck_enabled(&self) -> bool {
        self.config().voice.and_then(|v| v.duck_music).unwrap_or(true)
    }

    pub fn set_voice_duck_enabled(&self, enabled: bool) -> Result<bool, String> {
        let mut config = self.config();
        config.voice.get_or_insert_with(Default::default).duck_music = Some(enabled);
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        Ok(enabled)
    }

    pub fn set_voice_context_enabled(&self, enabled: bool) -> Result<bool, String> {
        let mut config = self.config();
        config.voice.get_or_insert_with(Default::default).context.get_or_insert_with(Default::default).enabled = enabled;
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        Ok(enabled)
    }

    /// The voice hid the lyrics window (T26.0); it beats `auto` and `always`.
    pub fn lyrics_hidden(&self) -> bool {
        self.inner.lyrics_hidden.load(Ordering::SeqCst)
    }

    pub fn set_lyrics_hidden(&self, hidden: bool) {
        self.inner.lyrics_hidden.store(hidden, Ordering::SeqCst);
    }

    pub fn lyrics_overlay_mode(&self) -> crate::lyrics_overlay::PcOverlay {
        let config = self.config();
        let raw = config.media.as_ref().and_then(|m| m.lyrics.as_ref()).and_then(|l| l.pc_overlay.as_deref());
        crate::lyrics_overlay::PcOverlay::parse(raw)
    }

    /// lyrics_overlay_set: stores the mode; the switch of the lookup stays as it is.
    pub fn set_lyrics_overlay_mode(&self, mode: crate::lyrics_overlay::PcOverlay) -> Result<crate::lyrics_overlay::PcOverlay, String> {
        let mut config = self.config();
        config.media.get_or_insert_with(Default::default).lyrics.get_or_insert_with(Default::default).pc_overlay = Some(mode.as_str().to_string());
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        self.restart_if_needed_changed();
        Ok(mode)
    }

    /// lyrics_offset of a phone: the shift of the current track moves by one step; any other key changes nothing.
    pub fn lyrics_offset(&self, track_key: &str, delta_ms: i64) -> bool {
        match self.inner.lyrics.nudge(track_key, delta_ms) {
            Some(value) => {
                self.inner.store.set("lyrics", value);
                true
            }
            None => false,
        }
    }

    pub fn assistant(&self) -> &crate::assistant::Assistant {
        &self.inner.assistant
    }

    pub fn voice(&self) -> &crate::voice::VoiceHub {
        &self.inner.voice
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    /// Stops the running collectors and starts them again from the current config.
    pub fn restart_collectors(&self) {
        let config = self.config();
        let needed = self.needed_now();
        *self.inner.needed.lock().unwrap() = Some(needed);
        let old = self.inner.sys_collector.lock().unwrap().take();
        drop(old); // joins the old thread before a new one starts
        let new = if config.collectors.sys.enabled && needed.sys {
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
        if config.collectors.media_enabled() && needed.media {
            *self.inner.media_collector.lock().unwrap() = Some(crate::collectors::media::MediaCollector::start(self.inner.store.clone()));
        } else {
            self.inner.store.set("media", serde_json::Value::Null);
            self.inner.store.update_vars(crate::collectors::media::clear_vars);
        }

        // Sound collector (M15): volume and the mutes of the default devices as variables; off = the variables are gone.
        let old_audio = self.inner.audio_collector.lock().unwrap().take();
        drop(old_audio);
        if config.collectors.audio_enabled() && needed.audio {
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
                    let agent = Agent { inner };
                    agent.publish_status();
                    agent.schedule_strip();
                }
            });
            let collector = crate::collectors::focus::FocusCollector::start(self.inner.store.clone(), on_change);
            *self.inner.focus_collector.lock().unwrap() = Some(collector);
        } else {
            self.inner.store.set("focus", serde_json::Value::Null);
            *self.inner.focus_app.lock().unwrap() = None;
            self.publish_status();
            self.schedule_strip();
        }

        // Charge of the PC and of Bluetooth devices (M17): only while a widget or a variable needs it.
        let old_battery = self.inner.battery_collector.lock().unwrap().take();
        drop(old_battery);
        if needed.battery {
            *self.inner.battery_collector.lock().unwrap() = Some(crate::battery::BatteryCollector::start(self.inner.battery.clone()));
        } else {
            self.inner.battery.pc_off();
        }
        // Unread mail (M17): IMAP once in two minutes while the widget or a variable needs it.
        self.start_mail(needed.mail);
        // Windows notifications (M17): only while the switch in the window is on.
        self.start_winnotif();

        // Weather (M15): a loop on the runtime; off = the section is null.
        self.start_weather();
        // Home Assistant (T30.8): the states of the chosen entities; off = no variables.
        self.refresh_ha();
        // OBS (M15): the connection to obs-websocket; off = no section, no variables.
        self.start_obs();
        // AI coding helpers (M16): limits of Claude Code and Codex, permission questions.
        self.start_ai();
        // Twitch and YouTube: the numbers of the signed-in channel.
        self.start_twitch();
        self.start_youtube();

        // HTTP checks: stop the old tasks (dropping aborts them), then start from the new config.
        let old_checks = self.inner.checks_collector.lock().unwrap().take();
        drop(old_checks);
        let checks = crate::collectors::checks::ChecksCollector::start(&self.inner.runtime, self.inner.statuses.clone(), config.collectors.checks);
        *self.inner.checks_collector.lock().unwrap() = Some(checks);
    }

    /// Recounts the `strip` section 700 ms from now; a newer call cancels this one, so a quick Alt+Tab settles first.
    /// Safe from any thread: the task runs on the stored runtime handle.
    fn schedule_strip(&self) {
        use std::sync::atomic::Ordering;
        let generation = self.inner.strip_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let me = self.clone();
        self.inner.runtime.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            if me.inner.strip_gen.load(Ordering::SeqCst) == generation {
                // The Start menu and the icon come from Windows: blocking work, off the runtime threads.
                let _ = tokio::task::spawn_blocking(move || me.recount_strip(true)).await;
            }
        });
    }

    /// (Re)starts the reader of Windows notifications when `windowsNotifications.enabled`.
    fn start_winnotif(&self) {
        let old = self.inner.winnotif_collector.lock().unwrap().take();
        drop(old); // joins the thread
        let enabled = self.config().windows_notifications.is_some_and(|w| w.enabled);
        // `notif.windows` = true while the feature is on; absent = off (the empty widget then explains how to turn it on).
        self.inner.store.update_vars(|vars| {
            if enabled {
                vars.insert("notif.windows".to_string(), VarValue::Bool(true));
            } else {
                vars.remove("notif.windows");
            }
        });
        if !enabled {
            return;
        }
        let (added_inner, removed_inner, seen_inner) = (Arc::downgrade(&self.inner), Arc::downgrade(&self.inner), Arc::downgrade(&self.inner));
        let handlers = crate::winnotif::Handlers {
            added: Box::new(move |toast| {
                if let Some(inner) = added_inner.upgrade() {
                    Agent { inner }.on_windows_toast(toast);
                }
            }),
            removed: Box::new(move |win_id| {
                if let Some(inner) = removed_inner.upgrade() {
                    Agent { inner }.on_windows_toast_removed(win_id);
                }
            }),
            seen_app: Box::new(move |app| {
                if let Some(inner) = seen_inner.upgrade() {
                    crate::winnotif::remember_app(&mut inner.win_apps.lock().unwrap(), app);
                }
            }),
        };
        *self.inner.winnotif_collector.lock().unwrap() = Some(crate::winnotif::WinNotifCollector::start(handlers));
    }

    /// A new toast in Windows: it goes to the phone as a notification, unless its app is in the list of exceptions.
    fn on_windows_toast(&self, toast: crate::winnotif::WinToast) {
        crate::winnotif::remember_app(&mut self.inner.win_apps.lock().unwrap(), &toast.app);
        let settings = self.config().windows_notifications;
        let exclude = settings.as_ref().map(|w| w.exclude.clone()).unwrap_or_default();
        if crate::winnotif::is_excluded(&toast.app, &exclude) {
            return;
        }
        let Some((title, body)) = crate::winnotif::title_and_body(&toast.texts) else { return };
        // Privacy (DATA_MODEL.md 11.5): in the mode `app` the words of the notification never leave the PC.
        let app_only = crate::winnotif::privacy_for(&toast.app, settings.as_ref().and_then(|w| w.privacy.as_ref())) == crate::model::PrivacyMode::App;
        let (title, body) = if app_only { (if toast.app.is_empty() { "Windows".to_string() } else { toast.app.clone() }, None) } else { (title, body) };
        let openable = crate::winnotif::valid_aumid(&toast.aumid);
        let id = self.push_notification_with(
            NotifyReq { title, body, level: Some(NotifLevel::Info), source: Some("windows".to_string()) },
            Some(toast.app.clone()).filter(|a| !a.is_empty()),
            openable.then_some(true),
            app_only.then_some(true),
        );
        if openable {
            let mut targets = self.inner.win_targets.lock().unwrap();
            targets.push_back((id, toast.id, toast.aumid));
            while targets.len() > WIN_TARGETS {
                targets.pop_front();
            }
        }
    }

    /// The toast went away in Windows (read, dismissed): it goes from the phone as well.
    fn on_windows_toast_removed(&self, win_id: u32) {
        let gone: Vec<String> = {
            let mut targets = self.inner.win_targets.lock().unwrap();
            let gone = targets.iter().filter(|(_, w, _)| *w == win_id).map(|(id, _, _)| id.clone()).collect();
            targets.retain(|(_, w, _)| *w != win_id);
            gone
        };
        if !gone.is_empty() {
            self.inner.store.update_notifications(|list| list.retain(|n| !gone.contains(&n.id)));
        }
    }

    /// `notif_open` from the phone: opens the program the notification came from. Only the id of the notification is taken from the
    /// phone; the program comes from what Windows told us. An unknown id (or one without a program) launches nothing.
    pub async fn notif_open(&self, notif_id: &str) -> bool {
        let aumid = self.inner.win_targets.lock().unwrap().iter().find(|(id, _, _)| id == notif_id).map(|(_, _, a)| a.clone());
        let Some(action) = aumid.as_deref().and_then(crate::winnotif::open_action) else {
            tracing::warn!("notif_open: no program for notification {notif_id}");
            return false;
        };
        let runner = self.inner.opts.runner.clone();
        match tokio::task::spawn_blocking(move || runner.run("notif-open", &action)).await {
            Ok(Ok(())) => true,
            Ok(Err(e)) => {
                tracing::warn!("notif_open: {e}");
                false
            }
            Err(e) => {
                tracing::warn!("notif_open: {e}");
                false
            }
        }
    }

    /// For the window: is the switch on, what Windows says about the access, and the apps seen so far.
    pub fn windows_notifications_status(&self) -> WinNotifStatus {
        WinNotifStatus {
            enabled: self.config().windows_notifications.is_some_and(|w| w.enabled),
            access: crate::winnotif::access_status().to_string(),
            apps: self.inner.win_apps.lock().unwrap().iter().cloned().collect(),
        }
    }

    /// For tests: a toast as if Windows had shown it.
    #[doc(hidden)]
    pub fn windows_toast_for_test(&self, toast: crate::winnotif::WinToast) {
        self.on_windows_toast(toast);
    }

    /// (Re)starts the mail loop: every two minutes, and at once on `Check now`, the unseen count of every mailbox of `config.mail`.
    /// An account whose login was refused is not tried again until its password changes. `on` false = no section, no variables.
    fn start_mail(&self, on: bool) {
        if let Some(old) = self.inner.mail_task.lock().unwrap().take() {
            old.abort();
        }
        let has_accounts = self.config().mail.is_some_and(|m| !m.accounts.is_empty());
        if !on || !has_accounts {
            self.inner.mail_status.lock().unwrap().clear();
            self.inner.store.set("mail", serde_json::Value::Null);
            self.inner.store.update_vars(crate::mail::clear_vars);
            return;
        }
        let me = self.clone();
        let wake = self.inner.mail_check.clone();
        let task = self.inner.runtime.spawn(async move {
            // Account id -> the password blob that was refused, and what the server said.
            let mut refused: std::collections::HashMap<String, (Option<String>, crate::mail::MailFailure)> = std::collections::HashMap::new();
            loop {
                let accounts = me.config().mail.map(|m| m.accounts).unwrap_or_default();
                let mut results = Vec::new();
                for (index, account) in accounts.into_iter().enumerate() {
                    let result = match refused.get(&account.id) {
                        Some((blob, failure)) if *blob == account.password => Err(failure.clone()),
                        _ => {
                            let outcome = probe_mailbox(&account).await.outcome;
                            if let Err(f) = &outcome {
                                // Said once while it stays the same. The number of the box, not its name: the log holds no
                                // user text, address or password.
                                let repeated = me.inner.mail_status.lock().unwrap().iter().any(|s| s.account_id == account.id && s.reason == Some(f.reason) && s.step == Some(f.step));
                                if !repeated {
                                    tracing::warn!("mailbox #{}: {} at the {:?} step", index + 1, f.reason.as_str(), f.step);
                                }
                            }
                            match &outcome {
                                Err(failure) if failure.reason.is_refusal() => {
                                    refused.insert(account.id.clone(), (account.password.clone(), failure.clone()));
                                }
                                _ => {
                                    refused.remove(&account.id);
                                }
                            }
                            outcome
                        }
                    };
                    results.push((account, result));
                }
                *me.inner.mail_status.lock().unwrap() = results.iter().map(|(a, r)| crate::mail::MailAccountStatus::new(&a.id, r)).collect();
                let info = crate::mail::build_info(&results, crate::auth::now_ms());
                me.inner.store.update_vars(|v| crate::mail::apply_vars(v, Some(&info)));
                me.inner.store.set("mail", serde_json::to_value(&info).expect("mail info serializes"));
                tokio::select! {
                    () = tokio::time::sleep(crate::mail::INTERVAL) => {}
                    () = wake.notified() => {}
                }
            }
        });
        *self.inner.mail_task.lock().unwrap() = Some(task);
    }

    /// `Check now` in the window: looks at the mailboxes at once.
    pub fn mail_check_now(&self) {
        self.inner.mail_check.notify_one();
    }

    /// `mail_status`: how the last round went; empty while nothing needs the mail.
    pub fn mail_status(&self) -> Vec<crate::mail::MailAccountStatus> {
        self.inner.mail_status.lock().unwrap().clone()
    }

    /// `mail_test` (T31.2): one check of a saved mailbox, step by step. A success also forgets an earlier refusal.
    pub async fn mail_test(&self, account_id: &str) -> Result<crate::mail::MailTestResult, String> {
        let account = self.config().mail.and_then(|m| m.accounts.into_iter().find(|a| a.id == account_id)).ok_or("no such mailbox")?;
        let probe = self.inner.runtime.spawn(async move { probe_mailbox(&account).await }).await.map_err(|e| e.to_string())?;
        if probe.outcome.is_ok() {
            self.start_mail(self.needed_now().mail);
        }
        Ok(probe.into())
    }

    /// `set_mail_password`: encrypts with DPAPI and saves it for the mailbox `account_id`; an empty password removes it. The
    /// refusal of an earlier login is forgotten with the old password.
    pub fn set_mail_password(&self, account_id: &str, password: &str) -> Result<(), String> {
        let mut config = self.config();
        let account = config.mail.as_mut().and_then(|m| m.accounts.iter_mut().find(|a| a.id == account_id)).ok_or("no such mailbox")?;
        account.password = if password.is_empty() { None } else { Some(crate::dpapi::protect(password)?) };
        account.password_set = None;
        match self.write_config(config) {
            SaveResult::Saved => {
                self.start_mail(self.needed_now().mail);
                Ok(())
            }
            SaveResult::Rejected(issues) => Err(issues.first().map(|i| i.message.clone()).unwrap_or_default()),
        }
    }

    /// For tests: pretends `app` is in front and recounts the strip at once (no collector, no 700 ms wait).
    #[doc(hidden)]
    pub fn set_focus_for_test(&self, app: Option<&str>) {
        *self.inner.focus_app.lock().unwrap() = app.map(str::to_string);
        self.recount_strip(false);
    }

    /// Recounts the strip; `with_icon` also looks up the program's own icon (the Start menu is read once, blocking).
    fn recount_strip(&self, with_icon: bool) {
        let layout = self.layout();
        let lang = crate::tray_text::agent_language(layout.display.language.as_deref());
        let focus = self.inner.focus_app.lock().unwrap().clone();
        let mut strip = crate::strip::strip_for(focus.as_deref(), &layout, &crate::templates::all(), lang);
        if let (true, Some(s)) = (with_icon, strip.as_mut()) {
            s.icon = self.strip_icon(&s.app);
        }
        let value = strip.as_ref().map_or(serde_json::Value::Null, |s| serde_json::to_value(s).expect("strip serializes"));
        *self.inner.strip.lock().unwrap() = strip;
        // The icon of the strip travels in the `icons` section next to the icons of the layout.
        self.store_icons(&layout);
        self.inner.store.set("strip", value);
    }

    /// `img:<hash16>` of the installed program whose exe is `app`, `None` when it is not installed (remembered per app).
    fn strip_icon(&self, app: &str) -> Option<String> {
        if let Some(known) = self.inner.strip_icons.lock().unwrap().get(app) {
            return known.clone();
        }
        let installed = {
            let mut cache = self.inner.installed.lock().unwrap();
            cache.get_or_insert_with(|| crate::installed::list_installed_apps().into_iter().map(|a| crate::installed::InstalledApp { icon: None, ..a }).collect()).clone()
        };
        let strip = StripInfo { app: app.to_string(), template: String::new(), name: String::new(), keys: Vec::new(), icon: None };
        let icon = crate::strip::with_icon(strip, &installed, |program| crate::icons::icon_from_program(&self.inner.config_dir, program).ok()).icon;
        self.inner.strip_icons.lock().unwrap().insert(app.to_string(), icon.clone());
        icon
    }

    /// The `icons` section: the icons the layout uses and the one of the strip.
    fn store_icons(&self, layout: &LayoutFile) {
        let mut icons = crate::icons::collect_icons(&self.inner.config_dir, layout);
        let strip_icon = self.inner.strip.lock().unwrap().as_ref().and_then(|s| s.icon.clone());
        if let (Some(map), Some(icon)) = (icons.as_object_mut(), strip_icon) {
            if let Some(hash) = crate::icons::img_hash(&icon) {
                if let Some(url) = crate::icons::icon_data_url(&self.inner.config_dir, hash) {
                    map.insert(hash.to_string(), serde_json::Value::String(url));
                }
            }
        }
        self.inner.store.set("icons", icons);
    }

    /// The template action of a `tpl-*` id, only while it is a key of the strip shown now and `actions.json` has no action
    /// with that id (DATA_MODEL.md 10.5). Nothing is written to `actions.json`.
    fn strip_action(&self, action_id: &str) -> Option<Action> {
        let strip = self.inner.strip.lock().unwrap().clone()?;
        if !strip.keys.iter().any(|k| k.action_id == action_id) {
            return None;
        }
        let template = crate::templates::all().into_iter().find(|t| t.id == strip.template)?;
        template.actions.get(action_id).filter(|a| !matches!(a, Action::Sequence { .. })).cloned()
    }

    /// The collectors the current layout and actions need (DATA_MODEL.md 10.1).
    fn needed_now(&self) -> crate::collectors::needed::Needed {
        let mut needed = crate::collectors::needed::needed_collectors(&self.layout(), &self.actions());
        needed.media |= self.inner.island.load(Ordering::SeqCst) && self.lyrics_overlay_mode() != crate::lyrics_overlay::PcOverlay::Off;
        needed
    }

    /// The application shows the island of the PC: what plays is read for it (unless its overlay is off).
    pub fn set_island_present(&self) {
        self.inner.island.store(true, Ordering::SeqCst);
        self.restart_if_needed_changed();
    }

    /// After the layout or the actions changed: restarts the collectors only when the set they need is not the same.
    fn restart_if_needed_changed(&self) {
        // A new `ha` button may follow an entity that is not watched yet.
        self.refresh_ha();
        let before = *self.inner.needed.lock().unwrap();
        if before.is_some() && before != Some(self.needed_now()) {
            self.restart_collectors();
        }
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

    pub fn auth_fail_limiter(&self) -> &RateLimiter {
        &self.inner.auth_fail_limiter
    }

    /// The handshake limiter that applies to a network category: 10 a minute, 5 on public networks.
    pub fn handshake_limiter(&self, category: crate::netpolicy::Category) -> &RateLimiter {
        if category == crate::netpolicy::Category::Public {
            &self.inner.public_limiter
        } else {
            &self.inner.pair_limiter
        }
    }

    pub fn networks(&self) -> &crate::netpolicy::Networks {
        &self.inner.networks
    }

    /// `config.network.allowPublic` (default true).
    pub fn allow_public(&self) -> bool {
        self.config().network.is_none_or(|n| n.allow_public)
    }

    /// The kind of network the first LAN address of this PC is on (blocks for the Windows lookup on a cache miss).
    pub fn network_status(&self) -> NetworkStatus {
        use crate::netpolicy::Category;
        let kind = if self.config().bind == "127.0.0.1" {
            "usb"
        } else {
            match lan_ips().first().and_then(|ip| ip.parse::<IpAddr>().ok()).map(|ip| self.inner.networks.category(ip)) {
                Some(Category::Private) => "private",
                Some(Category::Public) => "public",
                Some(Category::Local) | None => "usb",
            }
        };
        NetworkStatus { kind: kind.to_string(), allow_public: self.allow_public() }
    }

    /// The decision of the network policy for one connection (DATA_MODEL.md 11.2). May block on the first Windows lookup.
    pub fn network_verdict(&self, conn: &crate::netpolicy::ConnInfo, request: crate::netpolicy::Request) -> crate::netpolicy::Verdict {
        crate::netpolicy::policy(self.inner.networks.category(conn.local.ip()), request, self.allow_public())
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
        let (code, s_qr, expires_at) = self.inner.pairing.start_with_secret();
        let config = self.config();
        // USB-only mode or no LAN address: the phone reaches the PC through 127.0.0.1 (adb reverse).
        let mut ips = if config.bind == "127.0.0.1" { Vec::new() } else { lan_ips() };
        if ips.is_empty() {
            ips.push("127.0.0.1".to_string());
        }
        let port = self.inner.listening.lock().unwrap().as_ref().map_or(config.port as u16, |l| l.port);
        let s_qr = base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, s_qr);
        let url = crate::qr::pair_url(&ips[0], &ips, port, &code, &config.agent_name, &s_qr);
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

    /// (Re)starts the OBS connection from the config; safe from any thread (the runtime handle is used).
    fn start_obs(&self) {
        drop(self.inner.obs_client.lock().unwrap().take()); // aborts the old task
        let Some(cfg) = self.config().obs.filter(|o| o.enabled && self.needed_now().obs) else {
            crate::obs::clear(&self.inner.store);
            return;
        };
        // The password is read from its DPAPI blob here and goes nowhere but into the handshake.
        let password = cfg.password.as_deref().and_then(|stored| match crate::dpapi::unprotect(stored) {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("obs: the stored password cannot be read: {e}");
                None
            }
        });
        let me = self.clone();
        let on_wrong: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            me.push_notification(NotifyReq { title: "OBS: wrong password".to_string(), body: None, level: Some(NotifLevel::Warning), source: Some("decka".to_string()) });
        });
        let client = crate::obs::ObsClient::start(&self.inner.runtime, u16::try_from(cfg.port).unwrap_or(4455), password, self.inner.store.clone(), on_wrong);
        *self.inner.obs_client.lock().unwrap() = Some(Arc::new(client));
    }

    /// The `obs` actions (DATA_MODEL.md 2.3): one request to OBS. `OBS is not connected`, or the comment of OBS, as the error.
    async fn obs_action(&self, op: ObsOp, scene: Option<&str>, input: Option<&str>) -> Result<(), String> {
        let client = self.inner.obs_client.lock().unwrap().clone().ok_or("OBS is not connected")?;
        let (kind, data) = crate::obs::request_for(op, scene, input)?;
        client.request(kind, data).await.map(|_| ())
    }

    /// For the window (`obs_status`): is OBS connected, why not, and its scenes and audio inputs.
    pub async fn obs_status(&self) -> ObsStatusInfo {
        let Some(client) = self.inner.obs_client.lock().unwrap().clone() else {
            return ObsStatusInfo { connected: false, error: Some("OBS is off".to_string()), scenes: Vec::new(), inputs: Vec::new() };
        };
        let state = client.state();
        if !state.connected {
            return ObsStatusInfo { connected: false, error: client.error(), scenes: Vec::new(), inputs: Vec::new() };
        }
        let names = |data: &serde_json::Value, list: &str, key: &str| -> Vec<String> {
            data.get(list).and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.get(key).and_then(|s| s.as_str())).map(str::to_string).collect()).unwrap_or_default()
        };
        let scenes = match client.request("GetSceneList", serde_json::Value::Null).await {
            Ok(d) => names(&d, "scenes", "sceneName"),
            Err(_) => state.scenes.clone(),
        };
        let inputs = match client.request("GetInputList", serde_json::Value::Null).await {
            Ok(d) => names(&d, "inputs", "inputName"),
            Err(_) => state.inputs.clone(),
        };
        ObsStatusInfo { connected: true, error: None, scenes, inputs }
    }

    /// `set_obs_password`: encrypts with DPAPI and saves; an empty password removes it. The connection starts again with it.
    pub fn set_obs_password(&self, password: &str) -> Result<(), String> {
        let mut config = self.config();
        let obs = config.obs.get_or_insert(ObsConfig { enabled: false, port: 4455, password: None, password_set: None });
        obs.password = if password.is_empty() { None } else { Some(crate::dpapi::protect(password)?) };
        obs.password_set = None;
        match self.write_config(config) {
            SaveResult::Saved => {
                self.start_obs();
                Ok(())
            }
            SaveResult::Rejected(issues) => Err(issues.first().map(|i| i.message.clone()).unwrap_or_default()),
        }
    }

    /// (Re)starts the `ai` loop: once a second it sends what the 2-second rule held back, once a minute it drops old sessions
    /// and reset windows and looks at the Codex files (when that is on). Both off = no section, no variables, no task.
    fn start_ai(&self) {
        if let Some(old) = self.inner.ai_task.lock().unwrap().take() {
            old.abort();
        }
        let ai = self.config().ai;
        let claude_on = ai.as_ref().is_some_and(|a| a.claude.enabled);
        let codex_on = ai.as_ref().is_some_and(|a| a.codex.enabled);
        self.inner.ai.configure(claude_on, codex_on);
        self.inner.ai.set_rules(ai.as_ref().map(|a| a.rules.clone()).unwrap_or_default());
        if !claude_on && !codex_on {
            return;
        }
        let hub = self.inner.ai.clone();
        let account_on = claude_on && ai.as_ref().is_some_and(|a| a.claude.account_limits);
        let task = self.inner.runtime.spawn(async move {
            let mut ticks = 0u64;
            loop {
                if account_on && ticks.is_multiple_of(crate::ai::ACCOUNT_POLL.as_secs()) {
                    if let Some(home) = crate::ai::home_dir() {
                        let now = crate::auth::now_ms();
                        match crate::ai::fetch_account_limits(&crate::ai::credentials_path(&home), crate::ai::ACCOUNT_USAGE_URL, now).await {
                            Ok(Some(windows)) => hub.set_account_claude(windows, now),
                            Ok(None) => {}
                            Err(e) => tracing::debug!("claude account limits: {e:?}"),
                        }
                    }
                }
                if ticks.is_multiple_of(60) {
                    hub.prune(crate::auth::now_ms());
                    if codex_on {
                        let found = tokio::task::spawn_blocking(|| crate::ai::home_dir().and_then(|h| crate::ai::read_codex(&h))).await.ok().flatten();
                        hub.set_codex(found);
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                hub.tick();
                ticks += 1;
            }
        });
        *self.inner.ai_task.lock().unwrap() = Some(task);
    }

    // ===== Twitch =====

    /// Tests only: the two hosts of Twitch are replaced by a local server.
    pub fn set_twitch_hosts(&self, id: &str, api: &str) {
        *self.inner.twitch_api.lock().unwrap() = crate::twitch::Api { id: id.to_string(), api: api.to_string() };
    }

    fn twitch_api(&self) -> crate::twitch::Api {
        self.inner.twitch_api.lock().unwrap().clone()
    }

    /// The `twitch.*` variables follow the section; no section, no variables.
    fn apply_twitch(&self, info: Option<&TwitchInfo>) {
        self.inner.store.set("twitch", info.map_or(serde_json::Value::Null, |i| serde_json::to_value(i).expect("twitch serializes")));
        self.inner.store.update_vars(|vars| {
            vars.retain(|k, _| !k.starts_with("twitch."));
            if let Some(i) = info {
                vars.insert("twitch.live".to_string(), VarValue::Bool(i.live));
                vars.insert("twitch.viewers".to_string(), VarValue::Num(serde_json::Number::from(i.viewers)));
                vars.insert("twitch.followers".to_string(), VarValue::Num(serde_json::Number::from(i.followers)));
            }
        });
    }

    fn twitch_tokens(&self) -> Result<crate::twitch::Tokens, String> {
        let stored = self.config().twitch.and_then(|t| t.tokens).ok_or("Sign in to Twitch first")?;
        let json = crate::dpapi::unprotect(&stored).map_err(|e| format!("the Twitch sign-in cannot be read: {e}"))?;
        serde_json::from_str(&json).map_err(|_| "the Twitch sign-in cannot be read".to_string())
    }

    fn save_twitch_tokens(&self, tokens: &crate::twitch::Tokens) -> Result<(), String> {
        let blob = crate::dpapi::protect(&serde_json::to_string(tokens).map_err(|e| e.to_string())?)?;
        let mut config = self.config();
        let twitch = config.twitch.as_mut().ok_or("Twitch is not set up")?;
        twitch.tokens = Some(blob);
        twitch.tokens_set = None;
        match self.write_config(config) {
            SaveResult::Saved => Ok(()),
            SaveResult::Rejected(issues) => Err(issues.first().map(|i| i.message.clone()).unwrap_or_default()),
        }
    }

    /// One call to Twitch with the stored tokens; an expired access token is refreshed once and the call repeated.
    async fn twitch_call<T, F, Fut>(&self, f: F) -> Result<T, String>
    where
        F: Fn(crate::twitch::Api, String, String, String) -> Fut,
        Fut: std::future::Future<Output = Result<T, crate::twitch::ApiError>>,
    {
        use crate::twitch::ApiError;
        let cfg = self.config().twitch.filter(|t| t.enabled).ok_or("Twitch is off")?;
        let user_id = cfg.user_id.clone().ok_or("Sign in to Twitch first")?;
        let mut tokens = self.twitch_tokens()?;
        let api = self.twitch_api();
        for attempt in 0..2 {
            match f(api.clone(), cfg.client_id.clone(), tokens.access.clone(), user_id.clone()).await {
                Ok(v) => return Ok(v),
                Err(ApiError::Failed(m)) => return Err(m),
                Err(ApiError::Unauthorized) if attempt == 0 => {
                    tokens = api.refresh(&cfg.client_id, &tokens.refresh).await.map_err(|_| "Twitch asks to sign in again".to_string())?;
                    self.save_twitch_tokens(&tokens)?;
                }
                Err(ApiError::Unauthorized) => return Err("Twitch asks to sign in again".to_string()),
            }
        }
        Err("Twitch asks to sign in again".to_string())
    }

    /// A notification for every follower that came since the last look (at most 3; more become one line).
    fn twitch_follower_alerts(&self, latest: &[(String, u64)]) {
        let names = {
            let mut seen = self.inner.twitch_seen.lock().unwrap();
            let result = crate::twitch::new_followers(latest, *seen);
            *seen = result.1;
            result.0
        };
        let source = Some("twitch".to_string());
        if names.len() > 3 {
            self.push_notification(NotifyReq { title: format!("★ +{}", names.len()), body: Some("Twitch".to_string()), level: Some(NotifLevel::Info), source });
            return;
        }
        for name in names {
            self.push_notification(NotifyReq { title: format!("★ {name}"), body: Some("Twitch".to_string()), level: Some(NotifLevel::Info), source: source.clone() });
        }
    }

    /// One look at the channel: live, viewers, followers.
    async fn twitch_poll_once(&self) -> Result<TwitchInfo, String> {
        let login = self.config().twitch.and_then(|t| t.login).unwrap_or_default();
        let (stream, page) = self
            .twitch_call(|api, cid, token, uid| async move {
                let stream = api.stream(&cid, &token, &uid).await?;
                let page = api.followers(&cid, &token, &uid).await?;
                Ok((stream, page))
            })
            .await?;
        self.twitch_follower_alerts(&page.latest);
        Ok(crate::twitch::build_info(&login, stream, page.total))
    }

    /// (Re)starts the loop from the config: off or signed out = no section and no variables.
    fn start_twitch(&self) {
        if let Some(old) = self.inner.twitch_task.lock().unwrap().take() {
            old.abort();
        }
        *self.inner.twitch_last.lock().unwrap() = None;
        *self.inner.twitch_seen.lock().unwrap() = None;
        let on = self.config().twitch.is_some_and(|t| t.enabled && t.tokens.is_some() && t.user_id.is_some());
        if !on {
            self.apply_twitch(None);
            return;
        }
        let me = self.clone();
        let task = self.inner.runtime.spawn(async move {
            loop {
                match me.twitch_poll_once().await {
                    Ok(info) => {
                        *me.inner.twitch_error.lock().unwrap() = None;
                        *me.inner.twitch_last.lock().unwrap() = Some(info.clone());
                        me.apply_twitch(Some(&info));
                    }
                    Err(e) => {
                        tracing::debug!("twitch: {e}");
                        *me.inner.twitch_error.lock().unwrap() = Some(e);
                        // The last numbers stay on the phone, marked as not connected.
                        let last = me.inner.twitch_last.lock().unwrap().clone();
                        if let Some(mut info) = last {
                            info.connected = false;
                            me.apply_twitch(Some(&info));
                        }
                    }
                }
                tokio::time::sleep(crate::twitch::POLL_EVERY).await;
            }
        });
        *self.inner.twitch_task.lock().unwrap() = Some(task);
    }

    /// Step 1 of the sign-in: needs the client id of the owner's Twitch application. The window shows `user_code`; the agent
    /// asks Twitch every few seconds until the code has been typed, then stores the tokens and starts the loop.
    pub async fn twitch_login_start(&self) -> Result<crate::twitch::TwitchLogin, String> {
        let cfg = self.config().twitch.filter(|t| !t.client_id.is_empty()).ok_or("Enter the client id of your Twitch application first")?;
        let api = self.twitch_api();
        let code = api.device_start(&cfg.client_id).await?;
        let login = crate::twitch::TwitchLogin { user_code: code.user_code.clone(), verification_uri: code.verification_uri.clone(), expires_at: crate::auth::now_ms() + code.expires_in * 1000 };
        *self.inner.twitch_login.lock().unwrap() = Some((login.clone(), None));
        if let Some(old) = self.inner.twitch_login_task.lock().unwrap().take() {
            old.abort();
        }
        let me = self.clone();
        let client_id = cfg.client_id.clone();
        let expires_at = login.expires_at;
        let task = self.inner.runtime.spawn(async move {
            let mut interval = code.interval;
            let failed = |me: &Agent, e: String| {
                if let Some((_, error)) = me.inner.twitch_login.lock().unwrap().as_mut() {
                    *error = Some(e);
                }
            };
            loop {
                tokio::time::sleep(Duration::from_secs(interval)).await;
                if crate::auth::now_ms() > expires_at {
                    return failed(&me, "The code has expired".to_string());
                }
                match api.device_poll(&client_id, &code.device_code).await {
                    Ok(crate::twitch::Poll::Pending) => {}
                    Ok(crate::twitch::Poll::SlowDown) => interval += 5,
                    Ok(crate::twitch::Poll::Done(tokens)) => {
                        return match me.finish_twitch_login(&api, &client_id, tokens).await {
                            Ok(()) => *me.inner.twitch_login.lock().unwrap() = None,
                            Err(e) => failed(&me, e),
                        };
                    }
                    Err(e) => return failed(&me, e),
                }
            }
        });
        *self.inner.twitch_login_task.lock().unwrap() = Some(task);
        Ok(login)
    }

    async fn finish_twitch_login(&self, api: &crate::twitch::Api, client_id: &str, tokens: crate::twitch::Tokens) -> Result<(), String> {
        let (user_id, login) = api.user(client_id, &tokens.access).await.map_err(|e| match e {
            crate::twitch::ApiError::Unauthorized => "Twitch refused the sign-in".to_string(),
            crate::twitch::ApiError::Failed(m) => m,
        })?;
        let blob = crate::dpapi::protect(&serde_json::to_string(&tokens).map_err(|e| e.to_string())?)?;
        let mut config = self.config();
        let twitch = config.twitch.as_mut().ok_or("Twitch is not set up")?;
        (twitch.tokens, twitch.login, twitch.user_id, twitch.enabled) = (Some(blob), Some(login), Some(user_id), true);
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        self.start_twitch();
        Ok(())
    }

    /// Forgets the channel: the tokens are deleted, the loop stops, the section is `null`.
    pub fn twitch_logout(&self) -> Result<(), String> {
        if let Some(old) = self.inner.twitch_login_task.lock().unwrap().take() {
            old.abort();
        }
        *self.inner.twitch_login.lock().unwrap() = None;
        *self.inner.twitch_error.lock().unwrap() = None;
        let mut config = self.config();
        if let Some(t) = config.twitch.as_mut() {
            (t.tokens, t.login, t.user_id, t.tokens_set) = (None, None, None, None);
        }
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        self.start_twitch();
        Ok(())
    }

    /// For the window.
    pub fn twitch_status(&self) -> TwitchStatus {
        let cfg = self.config().twitch;
        let pending = self.inner.twitch_login.lock().unwrap().clone();
        TwitchStatus {
            enabled: cfg.as_ref().is_some_and(|t| t.enabled),
            signed_in: cfg.as_ref().is_some_and(|t| t.tokens.is_some()),
            login: cfg.and_then(|t| t.login),
            error: pending.as_ref().and_then(|(_, e)| e.clone()).or_else(|| self.inner.twitch_error.lock().unwrap().clone()),
            pending: pending.map(|(p, _)| p),
        }
    }

    /// The `twitch` actions: a clip of the live stream, or an announcement in the chat.
    async fn twitch_action(&self, op: TwitchOp, message: Option<&str>) -> Result<(), String> {
        match op {
            TwitchOp::Clip => self.twitch_call(|api, cid, token, uid| async move { api.clip(&cid, &token, &uid).await }).await.map(|_| ()),
            TwitchOp::Announce => {
                let text = message.ok_or("The announcement has no text")?.to_string();
                self.twitch_call(|api, cid, token, uid| {
                    let text = text.clone();
                    async move { api.announce(&cid, &token, &uid, &text).await }
                })
                .await
            }
        }
    }

    // ===== YouTube =====

    /// Tests only: the two hosts of Google are replaced by a local server.
    pub fn set_youtube_hosts(&self, auth: &str, api: &str) {
        *self.inner.youtube_api.lock().unwrap() = crate::youtube::Api { auth: auth.to_string(), api: api.to_string() };
    }

    fn youtube_api(&self) -> crate::youtube::Api {
        self.inner.youtube_api.lock().unwrap().clone()
    }

    fn apply_youtube(&self, info: Option<&YoutubeInfo>) {
        self.inner.store.set("youtube", info.map_or(serde_json::Value::Null, |i| serde_json::to_value(i).expect("youtube serializes")));
        self.inner.store.update_vars(|vars| {
            vars.retain(|k, _| !k.starts_with("youtube."));
            if let Some(i) = info {
                vars.insert("youtube.live".to_string(), VarValue::Bool(i.live));
                vars.insert("youtube.viewers".to_string(), VarValue::Num(serde_json::Number::from(i.viewers)));
                if let Some(s) = i.subscribers {
                    vars.insert("youtube.subscribers".to_string(), VarValue::Num(serde_json::Number::from(s)));
                }
            }
        });
    }

    fn write_youtube(&self, edit: impl FnOnce(&mut YoutubeConfig)) -> Result<(), String> {
        let mut config = self.config();
        edit(config.youtube.get_or_insert_with(|| YoutubeConfig { enabled: false, client_id: String::new(), secret: None, channel: None, tokens: None, secret_set: None, tokens_set: None }));
        match self.write_config(config) {
            SaveResult::Saved => Ok(()),
            SaveResult::Rejected(issues) => Err(issues.first().map(|i| i.message.clone()).unwrap_or_default()),
        }
    }

    /// `set_youtube_secret`: the client secret of the owner's Google application, encrypted with DPAPI; empty removes it.
    pub fn set_youtube_secret(&self, secret: &str) -> Result<(), String> {
        let blob = if secret.trim().is_empty() { None } else { Some(crate::dpapi::protect(secret.trim())?) };
        self.write_youtube(|y| {
            y.secret = blob;
            y.secret_set = None;
        })
    }

    fn youtube_secret(&self) -> Result<String, String> {
        let stored = self.config().youtube.and_then(|y| y.secret).ok_or("Enter the client secret of your Google application first")?;
        crate::dpapi::unprotect(&stored).map_err(|e| format!("the client secret cannot be read: {e}"))
    }

    fn youtube_tokens(&self) -> Result<crate::youtube::Tokens, String> {
        let stored = self.config().youtube.and_then(|y| y.tokens).ok_or("Sign in to YouTube first")?;
        let json = crate::dpapi::unprotect(&stored).map_err(|e| format!("the YouTube sign-in cannot be read: {e}"))?;
        serde_json::from_str(&json).map_err(|_| "the YouTube sign-in cannot be read".to_string())
    }

    fn save_youtube_tokens(&self, tokens: &crate::youtube::Tokens) -> Result<(), String> {
        let blob = crate::dpapi::protect(&serde_json::to_string(tokens).map_err(|e| e.to_string())?)?;
        self.write_youtube(|y| {
            y.tokens = Some(blob);
            y.tokens_set = None;
        })
    }

    /// One call to YouTube with the stored tokens; an expired access token is refreshed once and the call repeated.
    async fn youtube_call<T, F, Fut>(&self, f: F) -> Result<T, String>
    where
        F: Fn(crate::youtube::Api, String) -> Fut,
        Fut: std::future::Future<Output = Result<T, crate::youtube::ApiError>>,
    {
        use crate::youtube::ApiError;
        let cfg = self.config().youtube.filter(|y| y.enabled).ok_or("YouTube is off")?;
        let mut tokens = self.youtube_tokens()?;
        let api = self.youtube_api();
        for attempt in 0..2 {
            match f(api.clone(), tokens.access.clone()).await {
                Ok(v) => return Ok(v),
                Err(ApiError::Failed(m)) => return Err(m),
                Err(ApiError::Unauthorized) if attempt == 0 => {
                    let secret = self.youtube_secret()?;
                    tokens = api.refresh(&cfg.client_id, &secret, &tokens.refresh).await.map_err(|_| "YouTube asks to sign in again".to_string())?;
                    self.save_youtube_tokens(&tokens)?;
                }
                Err(ApiError::Unauthorized) => return Err("YouTube asks to sign in again".to_string()),
            }
        }
        Err("YouTube asks to sign in again".to_string())
    }

    /// One look at the channel: live, viewers, subscribers.
    async fn youtube_poll_once(&self) -> Result<YoutubeInfo, String> {
        let (channel, subscribers, live) = self
            .youtube_call(|api, token| async move {
                let (channel, subscribers) = api.channel(&token).await?;
                let live = match api.broadcast(&token).await? {
                    Some(b) => {
                        let viewers = api.viewers(&token, &b.video_id).await?;
                        Some((b, viewers))
                    }
                    None => None,
                };
                Ok((channel, subscribers, live))
            })
            .await?;
        Ok(crate::youtube::build_info(&channel, subscribers, live))
    }

    /// (Re)starts the loop from the config: off or signed out = no section and no variables.
    fn start_youtube(&self) {
        if let Some(old) = self.inner.youtube_task.lock().unwrap().take() {
            old.abort();
        }
        *self.inner.youtube_last.lock().unwrap() = None;
        let on = self.config().youtube.is_some_and(|y| y.enabled && y.tokens.is_some() && y.secret.is_some());
        if !on {
            self.apply_youtube(None);
            return;
        }
        let me = self.clone();
        let task = self.inner.runtime.spawn(async move {
            loop {
                match me.youtube_poll_once().await {
                    Ok(info) => {
                        *me.inner.youtube_error.lock().unwrap() = None;
                        *me.inner.youtube_last.lock().unwrap() = Some(info.clone());
                        me.apply_youtube(Some(&info));
                    }
                    Err(e) => {
                        tracing::debug!("youtube: {e}");
                        *me.inner.youtube_error.lock().unwrap() = Some(e);
                        let last = me.inner.youtube_last.lock().unwrap().clone();
                        if let Some(mut info) = last {
                            info.connected = false;
                            me.apply_youtube(Some(&info));
                        }
                    }
                }
                tokio::time::sleep(crate::youtube::POLL_EVERY).await;
            }
        });
        *self.inner.youtube_task.lock().unwrap() = Some(task);
    }

    /// Step 1 of the sign-in: needs the client id and the secret of the owner's Google application.
    pub async fn youtube_login_start(&self) -> Result<crate::youtube::YoutubeLogin, String> {
        let cfg = self.config().youtube.filter(|y| !y.client_id.is_empty()).ok_or("Enter the client id of your Google application first")?;
        let secret = self.youtube_secret()?;
        let api = self.youtube_api();
        let code = api.device_start(&cfg.client_id).await?;
        let login = crate::youtube::YoutubeLogin { user_code: code.user_code.clone(), verification_uri: code.verification_uri.clone(), expires_at: crate::auth::now_ms() + code.expires_in * 1000 };
        *self.inner.youtube_login.lock().unwrap() = Some((login.clone(), None));
        if let Some(old) = self.inner.youtube_login_task.lock().unwrap().take() {
            old.abort();
        }
        let me = self.clone();
        let client_id = cfg.client_id.clone();
        let expires_at = login.expires_at;
        let task = self.inner.runtime.spawn(async move {
            let mut interval = code.interval;
            let failed = |me: &Agent, e: String| {
                if let Some((_, error)) = me.inner.youtube_login.lock().unwrap().as_mut() {
                    *error = Some(e);
                }
            };
            loop {
                tokio::time::sleep(Duration::from_secs(interval)).await;
                if crate::auth::now_ms() > expires_at {
                    return failed(&me, "The code has expired".to_string());
                }
                match api.device_poll(&client_id, &secret, &code.device_code).await {
                    Ok(crate::youtube::Poll::Pending) => {}
                    Ok(crate::youtube::Poll::SlowDown) => interval += 5,
                    Ok(crate::youtube::Poll::Done(tokens)) => {
                        return match me.finish_youtube_login(&api, tokens).await {
                            Ok(()) => *me.inner.youtube_login.lock().unwrap() = None,
                            Err(e) => failed(&me, e),
                        };
                    }
                    Err(e) => return failed(&me, e),
                }
            }
        });
        *self.inner.youtube_login_task.lock().unwrap() = Some(task);
        Ok(login)
    }

    async fn finish_youtube_login(&self, api: &crate::youtube::Api, tokens: crate::youtube::Tokens) -> Result<(), String> {
        let (channel, _) = api.channel(&tokens.access).await.map_err(|e| match e {
            crate::youtube::ApiError::Unauthorized => "YouTube refused the sign-in".to_string(),
            crate::youtube::ApiError::Failed(m) => m,
        })?;
        let blob = crate::dpapi::protect(&serde_json::to_string(&tokens).map_err(|e| e.to_string())?)?;
        self.write_youtube(|y| {
            (y.tokens, y.channel, y.enabled) = (Some(blob), Some(channel), true);
        })?;
        self.start_youtube();
        Ok(())
    }

    /// Forgets the channel: the tokens are deleted, the loop stops, the section is `null`.
    pub fn youtube_logout(&self) -> Result<(), String> {
        if let Some(old) = self.inner.youtube_login_task.lock().unwrap().take() {
            old.abort();
        }
        *self.inner.youtube_login.lock().unwrap() = None;
        *self.inner.youtube_error.lock().unwrap() = None;
        self.write_youtube(|y| (y.tokens, y.channel, y.tokens_set) = (None, None, None))?;
        self.start_youtube();
        Ok(())
    }

    /// For the window.
    pub fn youtube_status(&self) -> YoutubeStatus {
        let cfg = self.config().youtube;
        let pending = self.inner.youtube_login.lock().unwrap().clone();
        YoutubeStatus {
            enabled: cfg.as_ref().is_some_and(|y| y.enabled),
            signed_in: cfg.as_ref().is_some_and(|y| y.tokens.is_some()),
            secret_set: cfg.as_ref().is_some_and(|y| y.secret.is_some()),
            channel: cfg.and_then(|y| y.channel),
            error: pending.as_ref().and_then(|(_, e)| e.clone()).or_else(|| self.inner.youtube_error.lock().unwrap().clone()),
            pending: pending.map(|(p, _)| p),
        }
    }

    /// The `youtube` actions: a message in the chat of the live stream.
    async fn youtube_action(&self, op: YoutubeOp, message: Option<&str>) -> Result<(), String> {
        match op {
            YoutubeOp::Start => {
                self.youtube_call(|api, token| async move {
                    let Some(next) = api.broadcast_in(&token, "upcoming").await? else {
                        return Err(crate::youtube::ApiError::Failed("YouTube: no stream is scheduled (create one in YouTube Studio)".to_string()));
                    };
                    api.transition(&token, &next.video_id, "live").await
                })
                .await
            }
            YoutubeOp::End => {
                self.youtube_call(|api, token| async move {
                    let Some(now) = api.broadcast(&token).await? else { return Err(crate::youtube::ApiError::Failed("YouTube: you are not live".to_string())) };
                    api.transition(&token, &now.video_id, "complete").await
                })
                .await
            }
            YoutubeOp::Chat => {
                let text = message.ok_or("The message has no text")?.to_string();
                self.youtube_call(|api, token| {
                    let text = text.clone();
                    async move {
                        let Some(broadcast) = api.broadcast(&token).await? else { return Err(crate::youtube::ApiError::Failed("YouTube: you are not live".to_string())) };
                        let Some(chat) = broadcast.live_chat_id else { return Err(crate::youtube::ApiError::Failed("YouTube: the stream has no chat".to_string())) };
                        api.chat(&token, &chat, &text).await
                    }
                })
                .await
            }
        }
    }

    pub fn ai_hub(&self) -> &Arc<crate::ai::AiHub> {
        &self.inner.ai
    }

    /// Who may call `/api/cc/*`: this PC only, with the `hookToken` (never the `apiToken`), 20 calls a second.
    pub fn cc_authorize(&self, ip: Option<IpAddr>, token: Option<&str>) -> Result<(), CcDenied> {
        if !ip.is_some_and(|i| i.to_canonical().is_loopback()) {
            return Err(CcDenied::NotFound);
        }
        let stored = self.config().ai.and_then(|a| a.hook_token);
        let valid = match (stored.as_deref(), token) {
            (Some(expected), Some(given)) if !expected.is_empty() => crate::auth::ct_eq(expected.as_bytes(), given.as_bytes()),
            _ => false,
        };
        if !valid {
            return Err(CcDenied::Unauthorized);
        }
        if self.inner.ai.allow_request() {
            Ok(())
        } else {
            Err(CcDenied::RateLimited)
        }
    }

    /// `POST /api/cc/statusline`: the line for the terminal.
    pub fn cc_statusline(&self, body: &[u8]) -> String {
        self.inner.ai.statusline(crate::ai::parse_statusline(body), crate::auth::now_ms())
    }

    /// `POST /api/cc/permission`: `None` = an empty answer, the tool asks in its terminal. `codex` = the call came from the
    /// hook of Codex (`?agent=codex`), otherwise from Claude Code. Both wait the time set for Claude.
    pub async fn cc_permission(&self, body: &[u8], codex: bool) -> Option<String> {
        let ai = self.config().ai?;
        let approvals = if codex { ai.codex.enabled && ai.codex.approvals != Some(false) } else { ai.claude.enabled && ai.claude.approvals };
        let opts = crate::ai::PermOpts {
            provider: if codex { "codex" } else { "claude" },
            replies: ai.replies.clone().unwrap_or_else(crate::ai::default_replies),
            approvals,
            // Only phones that may answer count: without one the question goes straight to the terminal (DATA_MODEL.md 11.3).
            phones: self.inner.conns.lock().unwrap().values().filter(|c| self.inner.devices.can_approve(&c.device_id)).count(),
            timeout: Duration::from_secs(u64::from(ai.claude.timeout_sec)),
        };
        self.inner.ai.permission(opts, body).await
    }

    /// `perm_answer` from the phone `device_id`. A phone without the right (`canApprove`) is ignored and logged; returns whether
    /// the answer was taken.
    pub fn answer_permission(&self, device_id: &str, req_id: &str, decision: crate::proto::PermDecision, reply: Option<usize>) -> bool {
        if !self.inner.devices.can_approve(device_id) {
            tracing::warn!("perm_answer ignored: device {device_id} may not answer Claude Code");
            return false;
        }
        let name = self.inner.devices.name_of(device_id).unwrap_or_default();
        let taken = self.inner.ai.answer_from_phone(req_id, decision, reply, &format!("Phone {name}"));
        self.save_ai_rules();
        taken
    }

    /// Writes the rules of "Always allow" to config.json when they changed since the last save.
    fn save_ai_rules(&self) {
        if !self.inner.ai.take_rules_dirty() {
            return;
        }
        let mut config = self.config();
        let Some(ai) = config.ai.as_mut() else { return };
        ai.rules = self.inner.ai.rules();
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            tracing::warn!("cannot save the rules of Always allow: {}", issues.first().map(|i| i.message.as_str()).unwrap_or(""));
        }
    }

    /// The rules list of the window: forget one rule (`Some(id)`) or all of them (`None`).
    pub fn ai_remove_rules(&self, id: Option<&str>) {
        self.inner.ai.remove_rules(id);
        self.save_ai_rules();
    }

    pub fn cc_status(&self) -> CcStatus {
        self.cc_status_in(crate::ai::home_dir().as_deref())
    }

    pub fn cc_status_in(&self, home: Option<&std::path::Path>) -> CcStatus {
        let (installed, conflict) = home.map_or((false, false), crate::ai::state_of);
        CcStatus {
            installed,
            status_line_conflict: conflict,
            last_seen_at: self.inner.ai.last_seen(),
            codex_found: home.is_some_and(|h| crate::ai::codex_dir(h).is_dir()),
            codex_hook: home.is_some_and(crate::ai::codex_hook_installed),
        }
    }

    pub fn cc_install(&self) -> Result<CcStatus, String> {
        let home = crate::ai::home_dir().ok_or("cannot find the home folder")?;
        self.cc_install_in(&home)
    }

    /// Writes our entries into `<home>\.claude\settings.json` (DATA_MODEL.md 6.8) and turns the integration on.
    pub fn cc_install_in(&self, home: &std::path::Path) -> Result<CcStatus, String> {
        let mut config = self.config();
        let codex_found = crate::ai::codex_dir(home).is_dir();
        let ai = config.ai.get_or_insert_with(|| AiConfig {
            claude: AiClaudeConfig { enabled: true, approvals: true, timeout_sec: 120, account_limits: false },
            codex: AiCodexConfig { enabled: codex_found, approvals: None },
            hook_token: None,
            rules: Vec::new(),
            replies: None,
            gpu_layers: None,
        });
        let token = ai.hook_token.get_or_insert_with(config::generate_api_token).clone();
        ai.claude.enabled = true;
        let timeout = ai.claude.timeout_sec;
        let codex_hook = codex_found && ai.codex.enabled && ai.codex.approvals != Some(false);
        crate::ai::install_into(home, &token, config.port, timeout)?;
        if codex_hook {
            crate::ai::install_codex_into(home, &token, config.port, timeout)?;
        } else {
            crate::ai::uninstall_codex_from(home)?;
        }
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        self.start_ai();
        Ok(self.cc_status_in(Some(home)))
    }

    pub fn cc_uninstall(&self) -> Result<CcStatus, String> {
        let home = crate::ai::home_dir().ok_or("cannot find the home folder")?;
        self.cc_uninstall_in(&home)
    }

    pub fn cc_uninstall_in(&self, home: &std::path::Path) -> Result<CcStatus, String> {
        crate::ai::uninstall_from(home)?;
        crate::ai::uninstall_codex_from(home)?;
        let mut config = self.config();
        if let Some(ai) = config.ai.as_mut() {
            ai.hook_token = None;
            ai.claude.enabled = false;
        }
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            return Err(issues.first().map(|i| i.message.clone()).unwrap_or_default());
        }
        self.start_ai();
        Ok(self.cc_status_in(Some(home)))
    }

    /// (Re)starts the weather loop from the config: at once, then every 30 minutes. Safe from any thread (the runtime handle is used).
    fn start_weather(&self) {
        if let Some(old) = self.inner.weather_task.lock().unwrap().take() {
            old.abort();
        }
        *self.inner.weather_last.lock().unwrap() = None;
        let enabled = self.config().weather.as_ref().is_some_and(|w| w.enabled) && self.needed_now().weather;
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
        let language = crate::tray_text::agent_language(self.layout().display.language.as_deref());
        let body = crate::weather::fetch(&crate::weather::geocoding_url(base, query, language)).await?;
        Ok(crate::weather::parse_places(&body))
    }

    /// Asks GitHub for the latest release once (on request: `Check now`, opening `About`) and keeps the answer.
    /// A newer release also gets its APK prepared for the phones (T20.4) in the background.
    pub async fn check_updates(&self) -> crate::updates::UpdateStatus {
        let status = self.check_updates_at(crate::updates::LATEST_URL).await;
        if let Some(found) = status.available.as_ref() {
            let me = self.clone();
            let version = found.version.clone();
            self.inner.runtime.spawn(async move {
                me.prepare_app_update(&crate::apk_update::release_base(&version), &version, crate::apk_update::UPDATE_PUBKEY).await;
            });
        }
        status
    }

    /// Downloads and verifies the APK of `version` from `base`, and on success offers it to the phones (section `update`).
    /// A failed check leaves nothing offered. Returns whether the APK is ready.
    pub async fn prepare_app_update(&self, base: &str, version: &str, pubkey: &str) -> bool {
        if self.inner.app_update.lock().unwrap().as_ref().is_some_and(|a| a.version == version) {
            return true;
        }
        match crate::apk_update::prepare(base, version, &self.inner.updates_dir, pubkey).await {
            Ok(update) => {
                let info = serde_json::json!({ "app": { "version": update.version, "size": update.size, "sha256": update.sha256 } });
                *self.inner.app_update.lock().unwrap() = Some(update);
                self.inner.store.set("update", info);
                true
            }
            Err(e) => {
                tracing::warn!("app update {version}: not offered ({e:?})");
                false
            }
        }
    }

    /// The APK the phones may fetch now.
    pub fn app_update(&self) -> Option<crate::apk_update::AppUpdate> {
        self.inner.app_update.lock().unwrap().clone()
    }

    /// `apk_get` of an authorized phone: the chunk at `offset`. Only for the offered APK, offsets that are multiples of 3000
    /// inside the file, and one connection per device at a time.
    pub fn apk_chunk(&self, device_id: &str, conn: &str, offset: u64) -> Result<crate::proto::ServerMsg, ApkGetError> {
        let Some(update) = self.app_update() else { return Err(ApkGetError::NoUpdate) };
        if !self.inner.apk_gate.lock().unwrap().allow(device_id, conn, crate::auth::now_ms()) {
            return Err(ApkGetError::Busy);
        }
        let Some(bytes) = crate::apk_update::read_chunk(&update, offset) else { return Err(ApkGetError::BadOffset) };
        use base64::Engine as _;
        Ok(crate::proto::ServerMsg::ApkChunk {
            offset,
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
            total: update.size,
            sha256: update.sha256,
        })
    }

    /// The phone's connection ended: its download place is free again.
    pub fn apk_release(&self, device_id: &str, conn: &str) {
        self.inner.apk_gate.lock().unwrap().release(device_id, conn);
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

    fn write_config(&self, mut config: ConfigFile) -> SaveResult {
        crate::config::protect_secrets(&mut config);
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
        // The hook token is created by `cc_install` only; the window never sees it and cannot change it.
        let before = self.config();
        // `server` is written in the file by hand: a saved form that does not carry it keeps it.
        if config.server.is_none() {
            config.server = before.server.clone();
        }
        // `ai.gpuLayers` is the agent's own note (T28.1): a saved form neither changes nor drops it.
        match (config.ai.as_mut(), before.ai.as_ref().and_then(|a| a.gpu_layers.clone())) {
            (Some(ai), kept) => ai.gpu_layers = kept,
            (None, Some(kept)) => config.ai = Some(AiConfig { gpu_layers: Some(kept), ..ai_off() }),
            (None, None) => {}
        }
        if let Some(ai) = config.ai.as_mut() {
            ai.hook_token = before.ai.as_ref().and_then(|a| a.hook_token.clone());
            // The rules of "Always allow" change only through the phone and the rules list, never through a saved form.
            ai.rules = before.ai.as_ref().map(|a| a.rules.clone()).unwrap_or_default();
        }
        // The YouTube secret and sign-in are written by Decka only; a new client id is another application, so they go.
        if let Some(yt) = config.youtube.as_mut() {
            let kept = before.youtube.as_ref().filter(|b| b.client_id == yt.client_id);
            (yt.secret, yt.tokens, yt.channel, yt.secret_set, yt.tokens_set) = (kept.and_then(|b| b.secret.clone()), kept.and_then(|b| b.tokens.clone()), kept.and_then(|b| b.channel.clone()), None, None);
        }
        // Mail passwords are written by `set_mail_password` only; a mailbox keeps its password while host and user stay the same,
        // so that a changed host can never receive the old password.
        if let Some(mail) = config.mail.as_mut() {
            for account in &mut mail.accounts {
                let kept = before.mail.as_ref().and_then(|m| m.accounts.iter().find(|b| b.id == account.id && b.host == account.host && b.user == account.user));
                account.password = kept.and_then(|b| b.password.clone());
                account.password_set = None;
            }
        }
        // Home Assistant is written by `ha_set_config` only.
        config.ha = before.ha.clone();
        // The Twitch sign-in is written by the sign-in only; a new client id is another application, so the old sign-in goes.
        if let Some(tw) = config.twitch.as_mut() {
            let kept = before.twitch.as_ref().filter(|b| b.client_id == tw.client_id);
            (tw.tokens, tw.login, tw.user_id, tw.tokens_set) = (kept.and_then(|b| b.tokens.clone()), kept.and_then(|b| b.login.clone()), kept.and_then(|b| b.user_id.clone()), None);
        }
        self.commit_config(&before, config)
    }

    /// The settings of a restored backup (T31.4). Unlike `save_config` it may bring passwords and tokens: `backup` puts in
    /// only the ones this PC already has.
    pub(crate) fn import_config(&self, config: ConfigFile) -> SaveResult {
        let result = self.commit_config(&self.config(), config);
        if result == SaveResult::Saved {
            self.refresh_ha();
        }
        result
    }

    fn commit_config(&self, before: &ConfigFile, config: ConfigFile) -> SaveResult {
        let issues = crate::validate::validate_config(&config);
        if !issues.is_empty() {
            return SaveResult::Rejected(issues);
        }
        // What the hooks in the files depend on: the port, the wait time and whether Codex asks the phone.
        let hooks_of = |c: &ConfigFile| c.ai.as_ref().map(|a| (a.claude.timeout_sec, a.codex.enabled, a.codex.approvals != Some(false)));
        let reinstall = (before.port != config.port || hooks_of(before) != hooks_of(&config)) && config.ai.as_ref().is_some_and(|a| a.hook_token.is_some());
        let result = self.write_config(config);
        if result == SaveResult::Saved {
            // A new port or wait time must reach the hook that Claude Code calls (DATA_MODEL.md 6.8, step 6).
            if reinstall {
                if let Some(home) = crate::ai::home_dir().filter(|h| crate::ai::state_of(h).0) {
                    if let Err(e) = self.cc_install_in(&home) {
                        tracing::warn!("cc_install after a settings change: {e}");
                    }
                }
            }
            self.restart_collectors();
            self.restart_usb();
            self.publish_status();
        }
        result
    }

    /// T29.P: `server.allowedHosts` - names, besides those of this PC, that requests may carry in `Host`.
    pub(crate) fn allowed_hosts(&self) -> Vec<String> {
        self.inner.config.lock().unwrap().server.as_ref().map(|s| s.allowed_hosts.clone()).unwrap_or_default()
    }

    /// T28.1: writes or removes `ai.gpuLayers`; nothing else of the settings changes.
    pub fn set_gpu_layers(&self, value: Option<GpuLayers>) {
        let mut config = self.config();
        match (config.ai.as_mut(), value) {
            (Some(ai), value) => ai.gpu_layers = value,
            (None, Some(value)) => config.ai = Some(AiConfig { gpu_layers: Some(value), ..ai_off() }),
            (None, None) => return,
        }
        if let SaveResult::Rejected(issues) = self.write_config(config) {
            tracing::warn!("ai.gpuLayers was not saved: {issues:?}");
        }
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
                let can_approve = d.can_approve.unwrap_or(true);
                DeviceInfo { id: d.id, name: d.name, created_at: d.created_at, last_seen_at: d.last_seen_at, connected: is_connected, app_version: d.app_version, can_approve }
            })
            .collect()
    }

    /// The switch `Can answer Claude Code` of the window; false for an unknown device. Answers that arrive from a phone that
    /// just lost the right are ignored from the next message on.
    pub fn set_can_approve(&self, device_id: &str, allowed: bool) -> bool {
        self.inner.devices.set_can_approve(device_id, allowed)
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
        if let Some(ai) = config.ai.as_mut() {
            ai.hook_token = None;
        }
        if let Some(tw) = config.twitch.as_mut() {
            tw.tokens_set = Some(tw.tokens.is_some());
            tw.tokens = None;
        }
        if let Some(yt) = config.youtube.as_mut() {
            (yt.secret_set, yt.tokens_set) = (Some(yt.secret.is_some()), Some(yt.tokens.is_some()));
            (yt.secret, yt.tokens) = (None, None);
        }
        if let Some(mail) = config.mail.as_mut() {
            for account in &mut mail.accounts {
                account.password_set = Some(account.password.is_some());
                account.password = None;
            }
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
        self.execute_for(action_id, None).await
    }

    /// `execute` for a press from a phone: the dangerous system actions are held back right after connecting and rate limited
    /// (DATA_MODEL.md 11.4). `None` = the PC itself (the window's test button, HTTP), without those limits.
    pub async fn execute_for(&self, action_id: &str, phone: Option<&PhoneCtx>) -> Result<(), (ErrorCode, String)> {
        let action = self.inner.actions.lock().unwrap().actions.get(action_id).cloned();
        let Some(action) = action else {
            // Not in actions.json: a key of the app strip shown now still runs (it comes from the built-in template).
            return match action_id.starts_with("tpl-").then(|| self.strip_action(action_id)).flatten() {
                Some(from_template) => self.run_action(action_id, from_template, phone).await,
                None => Err((ErrorCode::UnknownAction, format!("action '{action_id}' is not defined"))),
            };
        };
        if let Action::Sequence { steps, .. } = &action {
            let toggle = action.toggle_var().map(str::to_string);
            if !self.inner.running.lock().unwrap().insert(action_id.to_string()) {
                return Err((ErrorCode::ActionFailed, "Already running".to_string()));
            }
            let _guard = RunningGuard { inner: self.inner.clone(), id: action_id.to_string() };
            for (index, step) in steps.iter().enumerate() {
                self.run_step(step, phone).await.map_err(|(code, message)| (code, format!("Step {}: {message}", index + 1)))?;
            }
            self.toggle_var(toggle);
            return Ok(());
        }
        self.run_action(action_id, action, phone).await
    }

    /// One step of a sequence: wait, or run the action and wait for its result.
    async fn run_step(&self, step: &SequenceStep, phone: Option<&PhoneCtx>) -> Result<(), (ErrorCode, String)> {
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
                    Some(action) => self.run_action(action_id, action, phone).await,
                }
            }
        }
    }

    /// The runner on the blocking pool, then `toggleVar` on success.
    async fn run_action(&self, action_id: &str, action: Action, phone: Option<&PhoneCtx>) -> Result<(), (ErrorCode, String)> {
        // Shutdown, restart, sign out and hibernate pressed on a phone: not right after connecting, not twice in a row (11.4).
        if let (Action::System { op, .. }, Some(phone)) = (&action, phone) {
            if crate::safety::is_dangerous_system(*op) {
                self.inner.danger_gate.check(phone.connected_for).map_err(|m| (ErrorCode::ActionFailed, m.to_string()))?;
                tracing::info!("system {} by {}", format!("{op:?}").to_lowercase(), phone.device_name);
            }
        }
        // YouTube and Twitch are spoken to by the agent itself (their tokens never reach a runner).
        if let Action::Youtube { op, message, toggle_var, .. } = &action {
            self.youtube_action(*op, message.as_deref()).await.map_err(|m| (ErrorCode::ActionFailed, m))?;
            self.toggle_var(toggle_var.clone());
            return Ok(());
        }
        if let Action::Twitch { op, message, toggle_var, .. } = &action {
            self.twitch_action(*op, message.as_deref()).await.map_err(|m| (ErrorCode::ActionFailed, m))?;
            self.toggle_var(toggle_var.clone());
            return Ok(());
        }
        // Home Assistant is spoken to by the agent itself: its token never reaches a runner.
        if let Action::Ha { entity_id, service, toggle_var, .. } = &action {
            self.ha_call(entity_id, *service).await.map_err(|m| (ErrorCode::ActionFailed, m))?;
            self.toggle_var(toggle_var.clone());
            return Ok(());
        }
        // The recorder of the meeting notes belongs to the agent.
        if let Action::NotesRecord { toggle_var, .. } = &action {
            // Opening the sound devices takes a moment: not on a thread of the runtime.
            let me = self.clone();
            tokio::task::spawn_blocking(move || me.notes_toggle()).await.map_err(|e| (ErrorCode::Internal, format!("action task failed: {e}")))?.map_err(|m| (ErrorCode::ActionFailed, m))?;
            self.toggle_var(toggle_var.clone());
            return Ok(());
        }
        // OBS is spoken to over a connection the agent holds, not by a runner.
        if let Action::Obs { op, scene, input, toggle_var, .. } = &action {
            self.obs_action(*op, scene.as_deref(), input.as_deref()).await.map_err(|m| (ErrorCode::ActionFailed, m))?;
            self.toggle_var(toggle_var.clone());
            return Ok(());
        }
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
        self.store_icons(&layout);
        *self.inner.layout.lock().unwrap() = layout;
        self.inner.store.set("layout", value);
        self.restart_if_needed_changed();
        self.schedule_strip(); // a page for the app may have appeared or gone
    }

    /// Layout issues of the first kind that need the disk: `img:` icons must have their files (DATA_MODEL.md 8.2).
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
                        None => {
                            // T29.P: a header written by hand in plain text is protected and the file written back.
                            if crate::webhook::protect_headers(&mut actions) {
                                if let Ok(text) = json_text(&actions) {
                                    if self.write_watched("actions.json", &text).is_err() {
                                        tracing::warn!("cannot rewrite actions.json with protected headers");
                                    }
                                }
                            }
                            *self.inner.actions.lock().unwrap() = actions;
                        }
                    }
                }
            },
            "layout.json" => match serde_json::from_str::<LayoutFile>(&text) {
                Err(e) => self.invalid_file_notification(name, &format!("invalid JSON: {e}")),
                Ok(mut layout) => match crate::validate::validate_layout(&layout, &self.actions()).first() {
                    Some(issue) => self.invalid_file_notification(name, &issue.message),
                    None => {
                        // The file of the user stays as it is; the default icon stands in while a picture is missing.
                        crate::icons::replace_missing_icons(&self.inner.config_dir, &mut layout);
                        self.apply_layout(layout);
                    }
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
    pub fn save_actions(&self, mut actions: ActionsFile) -> SaveResult {
        let issues = crate::validate::validate_actions(&actions);
        if !issues.is_empty() {
            return SaveResult::Rejected(issues);
        }
        // T29.P: the headers of a webhook go to the file as DPAPI blobs.
        crate::webhook::protect_headers(&mut actions);
        let text = match json_text(&actions) {
            Ok(t) => t,
            Err(e) => return SaveResult::Rejected(vec![ValidationIssue { path: String::new(), message: e.to_string() }]),
        };
        if let Err(rejected) = self.write_watched("actions.json", &text) {
            return rejected;
        }
        *self.inner.actions.lock().unwrap() = actions;
        self.restart_if_needed_changed();
        SaveResult::Saved
    }

    /// Validates against the current actions, writes layout.json, updates memory and the layout section.
    pub fn save_layout(&self, mut layout: LayoutFile) -> SaveResult {
        crate::icons::replace_missing_icons(&self.inner.config_dir, &mut layout);
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
        self.push_notification_with(req, None, None, None)
    }

    /// `push_notification` with the M17 fields: the display name of the Windows app and whether a tap can open it.
    fn push_notification_with(&self, req: NotifyReq, app: Option<String>, openable: Option<bool>, hidden: Option<bool>) -> String {
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
            app,
            openable,
            hidden,
        };
        self.inner.store.update_notifications(|list| {
            list.insert(0, notification);
            list.truncate(MAX_NOTIFICATIONS);
        });
        id
    }

    /// The connection checks of the Phone tab (`diag.rs`).
    pub async fn diagnose(&self) -> Vec<crate::diag::DiagCheck> {
        crate::diag::run(&self.status()).await
    }

    pub fn status(&self) -> AgentStatus {
        AgentStatus {
            version: crate::VERSION.to_string(),
            listening: self.inner.listening.lock().unwrap().clone(),
            listen_error: self.inner.listen_error.lock().unwrap().clone(),
            listen_busy: self.inner.listen_busy.lock().unwrap().clone(),
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
        let battery = self.inner.battery_collector.lock().unwrap().take();
        drop(battery);
        let winnotif = self.inner.winnotif_collector.lock().unwrap().take();
        drop(winnotif);
        if let Some(task) = self.inner.mail_task.lock().unwrap().take() {
            task.abort();
        }
        if let Some(task) = self.inner.weather_task.lock().unwrap().take() {
            task.abort();
        }
        if let Some(task) = self.inner.ai_task.lock().unwrap().take() {
            task.abort();
        }
        for task in [&self.inner.twitch_task, &self.inner.twitch_login_task, &self.inner.youtube_task, &self.inner.youtube_login_task] {
            if let Some(task) = task.lock().unwrap().take() {
                task.abort();
            }
        }
        drop(self.inner.obs_client.lock().unwrap().take());
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
        let addr = SocketAddr::new(ip, port);
        match TcpListener::bind(addr).await {
            Ok(listener) => self.serve(listener, &config.bind),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                // The port is busy: say who holds it and keep trying for a while (after an update the old copy may still be closing).
                tracing::warn!("port {port} is busy, trying again for {} s", BIND_RETRY_TOTAL.as_secs());
                *self.inner.listening.lock().unwrap() = None;
                *self.inner.listen_error.lock().unwrap() = Some(e.to_string());
                *self.inner.listen_busy.lock().unwrap() = Some(ListenBusy { port, app: crate::portowner::port_owner(port) });
                let (agent, bind) = (self.clone(), config.bind.clone());
                let task = tokio::spawn(async move {
                    match bind_retrying(addr, BIND_RETRY_TOTAL, Duration::from_millis(500)).await {
                        Ok(listener) => agent.serve(listener, &bind),
                        Err(e) => {
                            tracing::error!("cannot listen on {bind}:{port}: {e}");
                            *agent.inner.listen_error.lock().unwrap() = Some(e.to_string());
                            *agent.inner.listen_busy.lock().unwrap() =
                                (e.kind() == std::io::ErrorKind::AddrInUse).then(|| ListenBusy { port, app: crate::portowner::port_owner(port) });
                        }
                    }
                    agent.publish_status();
                });
                *self.inner.bind_task.lock().unwrap() = Some(task);
            }
            Err(e) => {
                *self.inner.listening.lock().unwrap() = None;
                *self.inner.listen_error.lock().unwrap() = Some(e.to_string());
                *self.inner.listen_busy.lock().unwrap() = None;
                tracing::error!("cannot listen on {}:{}: {e}", config.bind, port);
            }
        }
        self.publish_status();
    }

    /// Starts serving on a bound listener.
    fn serve(&self, listener: TcpListener, bind: &str) {
        let local = listener.local_addr().expect("bound socket has an address");
        *self.inner.listening.lock().unwrap() = Some(Listening { bind: bind.to_string(), port: local.port() });
        *self.inner.listen_error.lock().unwrap() = None;
        *self.inner.listen_busy.lock().unwrap() = None;

        let app = crate::server::router(self.clone(), self.inner.opts.client_dist.clone());
        let agent = self.clone();
        let (tx, rx) = oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            let listener = crate::netpolicy::PolicyListener { inner: listener, agent };
            let serve = axum::serve(listener, app.into_make_service_with_connect_info::<crate::netpolicy::ConnInfo>())
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                });
            if let Err(e) = serve.await {
                tracing::error!("server stopped: {e}");
            }
        });
        *self.inner.server.try_lock().expect("the server slot is free while it starts") = Some(ServerHandle { shutdown: tx, join });
        tracing::info!("listening on {bind}:{}", local.port());
    }
    async fn stop_server(&self) {
        if let Some(task) = self.inner.bind_task.lock().unwrap().take() {
            task.abort();
        }
        if let Some(handle) = self.inner.server.lock().await.take() {
            let _ = handle.shutdown.send(());
            let _ = handle.join.await;
        }
        *self.inner.listening.lock().unwrap() = None;
        self.publish_status();
    }
}

/// Placeholder for the status channel's initial value; replaced right after construction.
/// Why an `apk_get` was not served (the connection stays open).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApkGetError {
    /// No verified APK is offered.
    NoUpdate,
    /// Another connection of this device is downloading.
    Busy,
    /// Not a multiple of 3000 or outside the file.
    BadOffset,
}

/// `%LOCALAPPDATA%\Decka\lyrics`; a test copy keeps it inside its own folder.
/// One check of a saved mailbox. Without a password (none yet, or one this Windows user cannot read) it ends at the sign-in.
async fn probe_mailbox(account: &crate::model::MailAccount) -> crate::mail::Probe {
    let password = account.password.as_deref().and_then(|blob| crate::dpapi::unprotect(blob).ok());
    // The configuration is validated to 1..65535; anything else cannot be connected to.
    let port = u16::try_from(account.port).unwrap_or(0);
    crate::mail::probe(&account.host, port, &account.user, password.as_deref(), crate::mail::TIMEOUT).await
}

fn lyrics_dir(config_dir: &std::path::Path, explicit: bool) -> PathBuf {
    if explicit || std::env::var_os("DECKA_CONFIG_DIR").is_some_and(|d| !d.is_empty()) {
        return config_dir.join("lyrics");
    }
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Decka").join("lyrics")
}

/// `%LOCALAPPDATA%\Decka\updates`; a test copy (explicit or env configuration folder) keeps it inside its own folder.
fn updates_dir(config_dir: &std::path::Path, explicit: bool) -> PathBuf {
    if explicit || std::env::var_os("DECKA_CONFIG_DIR").is_some_and(|d| !d.is_empty()) {
        return config_dir.join("updates");
    }
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Decka").join("updates")
}

fn blank_status(config_dir: &std::path::Path) -> AgentStatus {
    AgentStatus {
        version: crate::VERSION.to_string(),
        listening: None,
        listen_error: None,
        listen_busy: None,
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
