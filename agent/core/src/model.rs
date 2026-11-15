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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemOp {
    Lock,
    Sleep,
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
}

impl Action {
    pub fn name(&self) -> &str {
        match self {
            Action::Hotkey { name, .. }
            | Action::Media { name, .. }
            | Action::Launch { name, .. }
            | Action::OpenUrl { name, .. }
            | Action::System { name, .. } => name,
        }
    }

    pub fn toggle_var(&self) -> Option<&str> {
        match self {
            Action::Hotkey { toggle_var, .. }
            | Action::Media { toggle_var, .. }
            | Action::Launch { toggle_var, .. }
            | Action::OpenUrl { toggle_var, .. }
            | Action::System { toggle_var, .. } => toggle_var.as_deref(),
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
    pub usb: UsbConfig,
    pub collectors: Collectors,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchConfig {
    pub enabled: bool,
}

impl Collectors {
    pub fn focus_enabled(&self) -> bool {
        self.focus.as_ref().map_or(true, |c| c.enabled)
    }

    pub fn media_enabled(&self) -> bool {
        self.media.as_ref().map_or(true, |c| c.enabled)
    }
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
