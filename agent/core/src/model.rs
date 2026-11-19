//! Serde types for on-disk files and state sections. JSON names match the TS types in /shared/src.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ===== actions.json =====

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionsFile {
    pub version: u32,
    pub actions: BTreeMap<String, Action>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaOp {
    PlayPause,
    Next,
    Prev,
    VolUp,
    VolDown,
    Mute,
    /// M15: through the media session only.
    Shuffle,
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemOp {
    Lock,
    Sleep,
    /// M17 (DATA_MODEL.md 10.8).
    Shutdown,
    Restart,
    Signout,
    Hibernate,
    DisplayOff,
    TaskManager,
    Settings,
    Explorer,
    EmptyRecycleBin,
}

/// M15: Core Audio toggles (DATA_MODEL.md 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioOp {
    MicToggle,
    OutMuteToggle,
}

/// M15: obs-websocket requests (DATA_MODEL.md 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObsOp {
    RecordToggle,
    StreamToggle,
    Scene,
    InputMuteToggle,
    SaveReplay,
}

/// Twitch actions (the stream of the signed-in channel): a clip of the last seconds, or an announcement in the chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwitchOp {
    Clip,
    Announce,
}

/// YouTube actions (the live stream of the signed-in channel): a message in the chat of the stream, go live with the stream that
/// was scheduled in YouTube Studio, end the stream that is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YoutubeOp {
    Chat,
    Start,
    End,
}

/// One step of a `sequence`: run an action, or wait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SequenceStep {
    Run {
        #[serde(rename = "actionId")]
        action_id: String,
    },
    Wait {
        #[serde(rename = "delayMs")]
        delay_ms: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Action {
    Hotkey {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        keys: Vec<String>,
    },
    Media {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        op: MediaOp,
    },
    Launch {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        program: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hidden: Option<bool>,
    },
    OpenUrl {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        url: String,
    },
    System {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        op: SystemOp,
    },
    Audio {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        op: AudioOp,
    },
    Obs {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        op: ObsOp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scene: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<String>,
    },
    Twitch {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        op: TwitchOp,
        /// The text of an `announce`, and only there.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Youtube {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        op: YoutubeOp,
        /// The text of the chat message (1..200).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Sequence {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toggle_var: Option<String>,
        steps: Vec<SequenceStep>,
    },
}

impl Action {
    pub fn name(&self) -> &str {
        match self {
            Action::Hotkey { name, .. }
            | Action::Media { name, .. }
            | Action::Launch { name, .. }
            | Action::OpenUrl { name, .. }
            | Action::System { name, .. }
            | Action::Audio { name, .. }
            | Action::Obs { name, .. }
            | Action::Twitch { name, .. }
            | Action::Youtube { name, .. }
            | Action::Sequence { name, .. } => name,
        }
    }

    pub fn toggle_var(&self) -> Option<&str> {
        match self {
            Action::Hotkey { toggle_var, .. }
            | Action::Media { toggle_var, .. }
            | Action::Launch { toggle_var, .. }
            | Action::OpenUrl { toggle_var, .. }
            | Action::System { toggle_var, .. }
            | Action::Audio { toggle_var, .. }
            | Action::Obs { toggle_var, .. }
            | Action::Twitch { toggle_var, .. }
            | Action::Youtube { toggle_var, .. }
            | Action::Sequence { toggle_var, .. } => toggle_var.as_deref(),
        }
    }

    /// Short type name, as it appears in JSON ("hotkey", "open_url", ...).
    pub fn type_name(&self) -> &'static str {
        match self {
            Action::Hotkey { .. } => "hotkey",
            Action::Media { .. } => "media",
            Action::Launch { .. } => "launch",
            Action::OpenUrl { .. } => "open_url",
            Action::System { .. } => "system",
            Action::Audio { .. } => "audio",
            Action::Obs { .. } => "obs",
            Action::Twitch { .. } => "twitch",
            Action::Youtube { .. } => "youtube",
            Action::Sequence { .. } => "sequence",
        }
    }
}

// ===== layout.json =====

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    /// 3..8 (DATA_MODEL.md 3.1); checked by `validate_layout`.
    pub cols: u8,
    /// 2..5.
    pub rows: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutFile {
    pub version: u32,
    pub grid: Grid,
    pub display: DisplaySettings,
    pub pages: Vec<Page>,
}

/// The layout section sent to the phone: LayoutFile without `version`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientLayout {
    pub grid: Grid,
    pub display: DisplaySettings,
    pub pages: Vec<Page>,
}

impl LayoutFile {
    pub fn to_client(&self) -> ClientLayout {
        ClientLayout {
            grid: self.grid.clone(),
            display: self.display.clone(),
            pages: self.pages.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub items: Vec<LayoutItem>,
    /// Context page (M11): shown while one of these apps (lower-case exe names) is active on the PC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apps: Option<Vec<String>>,
    /// M13: a popup group, opened over the current page by a button with `popup`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popup: Option<bool>,
    /// M13: popup pages only: keep the panel open after a key inside it was pressed.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "stayOpen")]
    pub stay_open: Option<bool>,
    /// Shown automatically between these two times of day on the phone (when no app page applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<PageTime>,
}

/// `from` and `to` are `HH:MM` on the phone's clock; `from` later than `to` crosses midnight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageTime {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettings {
    pub dim_after_sec: i64,
    pub dim_level: f64,
    pub sleep_after_sec: i64,
    pub pixel_shift: bool,
    pub night: Night,
    /// M11: switch to context pages automatically; absent = true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_pages: Option<bool>,
    /// M13: `amoled` | `studio` | `material` | `glass`; absent = amoled (checked by `validate_layout`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// M13: `#rrggbb`; absent = `#7c5cff`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// M13: `aurora` | `sunset` | `ocean` | `graphite` | `album` (Glass only); absent = aurora.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper: Option<String>,
    /// M14: `standard` | `full` (DESIGN.md 2.1); absent = standard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_face: Option<String>,
    /// M14: show the labels of keys by default; absent = true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_labels: Option<bool>,
    /// M14: interface language `auto` | `en` | `ru` (DESIGN.md 6); absent = auto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// M14: `landscape` | `portrait` (DATA_MODEL.md 3.7); absent = landscape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Night {
    pub enabled: bool,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ButtonLook {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bind {
    pub var: String,
    pub states: BTreeMap<String, ButtonLook>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigate: Option<String>,
    /// M13: id of a popup page opened over the current one (alone, or after `actionId` succeeded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<Bind>,
    /// M14: `standard` | `full`; overrides `display.keyFace`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<String>,
    /// M14: overrides `display.keyLabels`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_label: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SysItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// M15: a graph of the last 2 minutes (DATA_MODEL.md 6.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusesItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusTileItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub status_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationsItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// "Now playing" widget (M11); a missing action id means the key is not drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
}

/// M15: volume slider (DATA_MODEL.md 6.6); a missing action falls back to `vol-up` / `vol-down` / `vol-mute` when they exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub up_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute_action: Option<String>,
}

/// M15: timer (DATA_MODEL.md 6.7); lives on the phone only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Minutes, 1..4 values of 1..180; absent = [25, 5].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presets: Option<Vec<i64>>,
}

/// M15: weather (DATA_MODEL.md 6.3) and OBS (6.4) widgets carry no settings of their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlainItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutItem {
    Button(ButtonItem),
    Clock(ClockItem),
    Sys(SysItem),
    Statuses(StatusesItem),
    Status(StatusTileItem),
    Notifications(NotificationsItem),
    Media(MediaItem),
    Volume(VolumeItem),
    Timer(TimerItem),
    Weather(PlainItem),
    Obs(PlainItem),
    Ai(AiItem),
    Twitch(PlainItem),
    Youtube(PlainItem),
    /// The keys of the page of the app in front, shown inside a widget.
    Context(PlainItem),
}

impl LayoutItem {
    /// (id, x, y, w, h) of any item kind.
    pub fn rect(&self) -> (&str, i32, i32, i32, i32) {
        match self {
            LayoutItem::Button(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Clock(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Sys(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Statuses(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Status(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Notifications(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Media(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Volume(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Timer(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Weather(i) | LayoutItem::Obs(i) | LayoutItem::Twitch(i) | LayoutItem::Youtube(i) | LayoutItem::Context(i) => (&i.id, i.x, i.y, i.w, i.h),
            LayoutItem::Ai(i) => (&i.id, i.x, i.y, i.w, i.h),
        }
    }

    /// Moves any item kind to (x, y).
    pub fn set_pos(&mut self, x: i32, y: i32) {
        match self {
            LayoutItem::Button(i) => (i.x, i.y) = (x, y),
            LayoutItem::Clock(i) => (i.x, i.y) = (x, y),
            LayoutItem::Sys(i) => (i.x, i.y) = (x, y),
            LayoutItem::Statuses(i) => (i.x, i.y) = (x, y),
            LayoutItem::Status(i) => (i.x, i.y) = (x, y),
            LayoutItem::Notifications(i) => (i.x, i.y) = (x, y),
            LayoutItem::Media(i) => (i.x, i.y) = (x, y),
            LayoutItem::Volume(i) => (i.x, i.y) = (x, y),
            LayoutItem::Timer(i) => (i.x, i.y) = (x, y),
            LayoutItem::Weather(i) | LayoutItem::Obs(i) | LayoutItem::Twitch(i) | LayoutItem::Youtube(i) | LayoutItem::Context(i) => (i.x, i.y) = (x, y),
            LayoutItem::Ai(i) => (i.x, i.y) = (x, y),
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            LayoutItem::Button(_) => "button",
            LayoutItem::Clock(_) => "clock",
            LayoutItem::Sys(_) => "sys",
            LayoutItem::Statuses(_) => "statuses",
            LayoutItem::Status(_) => "status",
            LayoutItem::Notifications(_) => "notifications",
            LayoutItem::Media(_) => "media",
            LayoutItem::Volume(_) => "volume",
            LayoutItem::Timer(_) => "timer",
            LayoutItem::Weather(_) => "weather",
            LayoutItem::Obs(_) => "obs",
            LayoutItem::Ai(_) => "ai",
            LayoutItem::Twitch(_) => "twitch",
            LayoutItem::Youtube(_) => "youtube",
            LayoutItem::Context(_) => "context",
        }
    }
}

// ===== config.json / devices.json =====

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    pub version: u32,
    pub agent_name: String,
    pub port: u32,
    pub bind: String,
    pub api_token: String,
    pub autostart: bool,
    /// M14: ask GitHub once a day for a newer release (ARCHITECTURE.md 8a); absent = true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_check: Option<bool>,
    pub usb: UsbConfig,
    pub collectors: Collectors,
    /// M15 (DATA_MODEL.md 6.3); absent = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather: Option<WeatherConfig>,
    /// M15 (DATA_MODEL.md 6.4); absent = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obs: Option<ObsConfig>,
    /// M16 (DATA_MODEL.md 6.8); absent = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiConfig>,
    /// Twitch (stream numbers and keys of the signed-in channel); absent = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub twitch: Option<TwitchConfig>,
    /// YouTube (live numbers and a chat key of the signed-in channel); absent = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub youtube: Option<YoutubeConfig>,
}

/// `secret` (the client secret of the owner's Google application, which Google's device flow requires and which is not
/// confidential for this kind of client) and `tokens` are only ever `dpapi:<base64>` on disk; the IPC answer leaves them out
/// and sets `secretSet` / `tokensSet`. The secret is written by `set_youtube_secret`, the tokens by the sign-in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeConfig {
    pub enabled: bool,
    #[serde(default)]
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// The channel that signed in (its title); written by the sign-in only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_set: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_set: Option<bool>,
}

/// The `youtube` section: what the phone shows. No token ever appears here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeInfo {
    pub connected: bool,
    pub channel: String,
    pub live: bool,
    pub viewers: u64,
    /// Absent when the channel hides its subscriber count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscribers: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
}

/// `tokens` is only ever `dpapi:<base64>` on disk (the access and the refresh token together); the IPC answer leaves it out
/// and sets `tokensSet`. `client_id` is the id of a Twitch application of the owner (a public client: no secret exists).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchConfig {
    pub enabled: bool,
    #[serde(default)]
    pub client_id: String,
    /// The channel that signed in (`login`, `id`); written by the sign-in only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// Only in `get_config` answers; never stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_set: Option<bool>,
}

/// The `twitch` section: what the phone shows. No token ever appears here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchInfo {
    pub connected: bool,
    pub login: String,
    pub live: bool,
    pub viewers: u64,
    pub followers: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
}

/// M16: AI coding helpers. `hook_token` is created by `cc_install`, guards only `/api/cc/*`, and is never sent to the window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub claude: AiClaudeConfig,
    pub codex: AiCodexConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_token: Option<String>,
    /// What the user chose to allow always (the phone's "Always allow"); changed only by the agent and the rules list of the window.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<crate::ai::AiRule>,
    /// The ready answers of "Deny and say" (0..4 texts); absent = the built-in three.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replies: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiClaudeConfig {
    pub enabled: bool,
    pub approvals: bool,
    /// 60 | 120 | 300.
    pub timeout_sec: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiCodexConfig {
    pub enabled: bool,
    /// Phone approvals for Codex (its `PermissionRequest` hook); absent = on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvals: Option<bool>,
}

/// M16: the `ai` widget (DATA_MODEL.md 6.8); `show` is `both` (default) | `claude` | `codex`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherConfig {
    pub enabled: bool,
    pub place: String,
    pub lat: f64,
    pub lon: f64,
    /// `metric` | `imperial`.
    pub units: String,
}

/// `password` is only ever `dpapi:<base64>` on disk; the IPC answer leaves it out and sets `passwordSet` instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsConfig {
    pub enabled: bool,
    pub port: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Only in `get_config` answers; never stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_set: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbConfig {
    pub enabled: bool,
    pub adb_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Collectors {
    pub sys: SysCollectorConfig,
    pub checks: Vec<HttpCheck>,
    /// M11; absent = enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<SwitchConfig>,
    /// M11; absent = enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<SwitchConfig>,
    /// M15 (DATA_MODEL.md 6.6); absent = enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<SwitchConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchConfig {
    pub enabled: bool,
}

impl Collectors {
    pub fn audio_enabled(&self) -> bool {
        self.audio.as_ref().map_or(true, |c| c.enabled)
    }

    pub fn focus_enabled(&self) -> bool {
        self.focus.as_ref().map_or(true, |c| c.enabled)
    }

    pub fn media_enabled(&self) -> bool {
        self.media.as_ref().map_or(true, |c| c.enabled)
    }
}

/// One key of the app strip (DATA_MODEL.md 10.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StripKey {
    pub action_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// The `strip` section: the keys of the built-in template of the app in front, at most 16.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StripInfo {
    pub app: String,
    pub template: String,
    pub name: String,
    pub keys: Vec<StripKey>,
}

/// Active PC window: the executable name only, NEVER the window title (S16).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusInfo {
    pub app: Option<String>,
}

/// Current Windows media session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub source: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    pub playing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<u64>,
    pub position_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
    /// M15: GSMTC `IsShuffleActive`; absent when the source has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shuffle: Option<bool>,
    /// M15: `none` | `list` | `track`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<String>,
}

/// M15: weather section (DATA_MODEL.md 6.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherInfo {
    pub place: String,
    pub units: String,
    pub updated_at: u64,
    pub current: WeatherNow,
    pub daily: Vec<WeatherDay>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherNow {
    pub temp: i64,
    pub code: u32,
    pub is_day: bool,
    pub wind: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherDay {
    pub date: String,
    pub min: i64,
    pub max: i64,
    pub code: u32,
}

/// M15: OBS section (DATA_MODEL.md 6.4); the password never appears here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsInfo {
    pub connected: bool,
    pub recording: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_started_at: Option<u64>,
    pub streaming: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_started_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    pub replay_buffer: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysCollectorConfig {
    pub enabled: bool,
    pub interval_sec: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCheck {
    pub id: String,
    pub label: String,
    pub url: String,
    pub interval_sec: i64,
    pub timeout_sec: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect_status: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DevicesFile {
    pub version: u32,
    pub devices: Vec<DeviceRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub created_at: u64,
    pub last_seen_at: u64,
    /// M14: `client.version` of the last `pair` / `hello` (ARCHITECTURE.md 8a); absent for phones seen before it was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
}

// ===== State sections =====

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysStats {
    pub cpu: u32,
    pub mem: SysMem,
    pub disks: Vec<SysDisk>,
    pub net: SysNet,
    pub uptime_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysMem {
    pub used_mb: u64,
    pub total_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysDisk {
    pub mount: String,
    pub used_gb: u64,
    pub total_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysNet {
    pub rx_kbps: u64,
    pub tx_kbps: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusState {
    Ok,
    Warn,
    Error,
    Running,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusSource {
    Push,
    Check,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusItem {
    pub id: String,
    pub label: String,
    pub state: StatusState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub source: StatusSource,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub ts: u64,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub level: NotifLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VarValue {
    Bool(bool),
    Num(serde_json::Number),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
}
