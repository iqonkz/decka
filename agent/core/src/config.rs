//! Config directory: load with defaults, atomic save (DATA_MODEL.md sections 1-4).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::SysRng;
use rand::TryRng;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::model::*;
use crate::validate::{validate_actions, validate_config, validate_layout};

const DEFAULT_ACTIONS: &str = include_str!("../../../shared/fixtures/actions.default.json");
const DEFAULT_LAYOUT: &str = include_str!("../../../shared/fixtures/layout.default.json");

pub struct Loaded {
    pub config: ConfigFile,
    pub actions: ActionsFile,
    pub layout: LayoutFile,
    pub devices: DevicesFile,
    /// (file name, issue) for every file that could not be used as written.
    pub issues: Vec<(String, ValidationIssue)>,
}

/// Cryptographically secure random bytes from the OS.
pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    SysRng.try_fill_bytes(&mut buf).expect("OS random generator failed");
    buf
}

pub fn generate_api_token() -> String {
    format!("dk_{}", URL_SAFE_NO_PAD.encode(random_bytes(32)))
}

pub fn default_config() -> ConfigFile {
    let name = std::env::var("COMPUTERNAME").unwrap_or_default();
    let name: String = name.chars().take(40).collect();
    ConfigFile {
        version: 1,
        agent_name: if name.is_empty() { "PC".to_string() } else { name },
        port: 9247,
        bind: "0.0.0.0".to_string(),
        api_token: generate_api_token(),
        autostart: false,
        update_check: None,
        usb: UsbConfig { enabled: false, adb_path: "adb".to_string() },
        collectors: Collectors {
            sys: SysCollectorConfig { enabled: true, interval_sec: 2 },
            checks: Vec::new(),
            focus: None,
            media: None,
            audio: None,
        },
        weather: None,
        obs: None,
        ai: None,
        twitch: None,
        youtube: None,
        mail: None,
        windows_notifications: None,
        network: None,
        media: None,
    }
}

pub fn default_actions() -> ActionsFile {
    serde_json::from_str(DEFAULT_ACTIONS).expect("embedded default actions are valid")
}

pub fn default_layout() -> LayoutFile {
    serde_json::from_str(DEFAULT_LAYOUT).expect("embedded default layout is valid")
}

pub fn default_devices() -> DevicesFile {
    DevicesFile { version: 1, devices: Vec::new() }
}

/// Used when actions.json / layout.json is invalid at start-up.
pub fn empty_actions() -> ActionsFile {
    ActionsFile { version: 1, actions: Default::default() }
}

pub fn empty_layout() -> LayoutFile {
    let mut layout = default_layout();
    layout.pages = vec![Page { id: "main".to_string(), name: "Main".to_string(), items: Vec::new(), apps: None, popup: None, stay_open: None, time: None }];
    layout
}

/// Exact text written to disk: pretty JSON (2 spaces) plus a trailing newline.
pub fn json_text<T: Serialize>(value: &T) -> std::io::Result<String> {
    let mut text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    text.push('\n');
    Ok(text)
}

/// Write `<path>.tmp`, fsync, then rename over the target.
pub fn save_json_atomic<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let text = json_text(value)?;
    save_text_atomic(path, &text)
}

pub fn save_text_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Read one file. `Ok(None)` = missing; `Err` = unreadable or not valid JSON for the type.
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
            serde_json::from_str(text).map(Some).map_err(|e| format!("invalid JSON: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read file: {e}")),
    }
}

/// M15 migration (T15.2 step 4): the default `vol-mute` used to carry `toggleVar: "audio.muted"`, and the audio collector now
/// sets that variable itself. This one case is dropped silently (logged); any other `toggleVar` on a reserved name stays an
/// error. Returns whether something changed.
pub fn migrate_actions(actions: &mut ActionsFile) -> bool {
    let Some(Action::Media { op: MediaOp::Mute, toggle_var, .. }) = actions.actions.get_mut("vol-mute") else { return false };
    if toggle_var.as_deref() != Some("audio.muted") {
        return false;
    }
    *toggle_var = None;
    tracing::info!("actions.json: dropped toggleVar \"audio.muted\" from vol-mute (the audio collector sets it now)");
    true
}

/// Generic per-file loading: missing -> create from defaults; invalid -> fallback, file untouched.
/// `migrate` may rewrite a value that was read (it is not written back to the file until the next save).
fn load_file<T: Serialize + DeserializeOwned>(
    dir: &Path,
    name: &str,
    default: impl FnOnce() -> T,
    fallback: impl FnOnce() -> T,
    migrate: impl FnOnce(&mut T),
    validate: impl FnOnce(&T) -> Vec<ValidationIssue>,
    issues: &mut Vec<(String, ValidationIssue)>,
) -> T {
    let path = dir.join(name);
    match read_json::<T>(&path) {
        Ok(None) => {
            let value = default();
            if let Err(e) = save_json_atomic(&path, &value) {
                issues.push((name.to_string(), ValidationIssue { path: String::new(), message: format!("cannot create file: {e}") }));
            }
            value
        }
        Ok(Some(mut value)) => {
            migrate(&mut value);
            let found = validate(&value);
            if found.is_empty() {
                value
            } else {
                issues.extend(found.into_iter().map(|i| (name.to_string(), i)));
                fallback()
            }
        }
        Err(message) => {
            issues.push((name.to_string(), ValidationIssue { path: String::new(), message }));
            fallback()
        }
    }
}

/// Turns a secret that was typed or pasted into the file as plain text into `dpapi:<base64>` (DATA_MODEL.md 11.6). Anything that
/// is already a blob stays; without DPAPI (not Windows) the text is left alone. Returns whether it changed.
fn protect_plain(value: &mut Option<String>) -> bool {
    let Some(plain) = value.as_deref().filter(|v| !v.is_empty() && !v.starts_with(crate::dpapi::PREFIX)) else { return false };
    match crate::dpapi::protect(plain) {
        Ok(blob) => {
            *value = Some(blob);
            true
        }
        Err(e) => {
            tracing::warn!("cannot protect a secret of config.json: {e}");
            false
        }
    }
}

/// All passwords and sign-in tokens of config.json (OBS, mail, YouTube, Twitch) are kept only as DPAPI blobs. The setters of the
/// window already write blobs; this catches a value written by hand. `apiToken` and the hook token stay readable: scripts and the
/// Claude Code hooks read them from the file (see BLOCKERS.md). Returns whether anything was changed (the file must be rewritten).
pub fn protect_secrets(config: &mut ConfigFile) -> bool {
    let mut changed = false;
    if let Some(obs) = config.obs.as_mut() {
        changed |= protect_plain(&mut obs.password);
    }
    if let Some(mail) = config.mail.as_mut() {
        for account in &mut mail.accounts {
            changed |= protect_plain(&mut account.password);
        }
    }
    if let Some(yt) = config.youtube.as_mut() {
        changed |= protect_plain(&mut yt.secret);
        changed |= protect_plain(&mut yt.tokens);
    }
    if let Some(tw) = config.twitch.as_mut() {
        changed |= protect_plain(&mut tw.tokens);
    }
    changed
}

pub fn load_or_init(dir: &Path) -> Loaded {
    let mut issues = Vec::new();
    if let Err(e) = fs::create_dir_all(dir) {
        issues.push(("config.json".to_string(), ValidationIssue { path: String::new(), message: format!("cannot create config dir: {e}") }));
    }

    // Secrets written by hand are encrypted before the file is validated (the validator accepts blobs only), and written back;
    // a config.json that could not be used is never rewritten.
    let protected = std::cell::Cell::new(false);
    let config = load_file(dir, "config.json", default_config, default_config, |c| protected.set(protect_secrets(c)), validate_config, &mut issues);
    if protected.get() && !issues.iter().any(|(file, _)| file == "config.json") {
        if let Err(e) = save_json_atomic(&dir.join("config.json"), &config) {
            tracing::warn!("cannot rewrite config.json with protected secrets: {e}");
        }
    }
    let actions = load_file(dir, "actions.json", default_actions, empty_actions, |a| { migrate_actions(a); }, validate_actions, &mut issues);
    let layout = load_file(dir, "layout.json", default_layout, empty_layout, |_| {}, |l: &LayoutFile| validate_layout(l, &actions), &mut issues);
    let devices = load_file(dir, "devices.json", default_devices, default_devices, |_| {}, |d: &DevicesFile| {
        if d.version == 1 {
            Vec::new()
        } else {
            vec![ValidationIssue { path: "/version".to_string(), message: "version must be 1".to_string() }]
        }
    }, &mut issues);

    Loaded { config, actions, layout, devices, issues }
}
