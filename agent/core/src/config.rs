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
        usb: UsbConfig { enabled: false, adb_path: "adb".to_string() },
        collectors: Collectors {
            sys: SysCollectorConfig { enabled: true, interval_sec: 2 },
            checks: Vec::new(),
        },
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
    layout.pages = vec![Page { id: "main".to_string(), name: "Main".to_string(), items: Vec::new() }];
    layout
}

/// Write `<path>.tmp`, fsync, then rename over the target.
pub fn save_json_atomic<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    text.push('\n');
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

/// Generic per-file loading: missing -> create from defaults; invalid -> fallback, file untouched.
fn load_file<T: Serialize + DeserializeOwned>(
    dir: &Path,
    name: &str,
    default: impl FnOnce() -> T,
    fallback: impl FnOnce() -> T,
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
        Ok(Some(value)) => {
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

pub fn load_or_init(dir: &Path) -> Loaded {
    let mut issues = Vec::new();
    if let Err(e) = fs::create_dir_all(dir) {
        issues.push(("config.json".to_string(), ValidationIssue { path: String::new(), message: format!("cannot create config dir: {e}") }));
    }

    let config = load_file(dir, "config.json", default_config, default_config, validate_config, &mut issues);
    let actions = load_file(dir, "actions.json", default_actions, empty_actions, validate_actions, &mut issues);
    let layout = load_file(dir, "layout.json", default_layout, empty_layout, |l: &LayoutFile| validate_layout(l, &actions), &mut issues);
    let devices = load_file(dir, "devices.json", default_devices, default_devices, |d: &DevicesFile| {
        if d.version == 1 {
            Vec::new()
        } else {
            vec![ValidationIssue { path: "/version".to_string(), message: "version must be 1".to_string() }]
        }
    }, &mut issues);

    Loaded { config, actions, layout, devices, issues }
}
