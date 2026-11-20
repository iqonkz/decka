use decka_core::config::{default_actions, default_layout, load_or_init, save_json_atomic};
use decka_core::model::*;
use decka_core::validate::{validate_actions, validate_config, validate_layout};
use serde_json::{json, Value};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/fixtures");

#[test]
fn empty_dir_creates_four_files() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = load_or_init(dir.path());
    assert!(loaded.issues.is_empty(), "{:?}", loaded.issues);
    for f in ["config.json", "actions.json", "layout.json", "devices.json"] {
        assert!(dir.path().join(f).is_file(), "{f} missing");
    }
    let t = &loaded.config.api_token;
    assert_eq!(t.len(), 46);
    assert!(t.starts_with("dk_"));
    assert!(t[3..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'));

    // A second start reads the same files back with no issues and the same token.
    let again = load_or_init(dir.path());
    assert!(again.issues.is_empty());
    assert_eq!(again.config.api_token, loaded.config.api_token);
}

#[test]
fn invalid_layout_fixtures_report_expected_path() {
    let actions = default_actions();
    for name in [
        "layout-overlap",
        "layout-out-of-bounds",
        "layout-dup-id",
        "layout-unknown-action",
        "layout-bad-navigate",
        "layout-bad-size",
    ] {
        let text = std::fs::read_to_string(format!("{FIXTURES}/invalid/{name}.json")).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        let layout: LayoutFile = serde_json::from_value(v["layout"].clone()).unwrap();
        let issues = validate_layout(&layout, &actions);
        assert!(!issues.is_empty(), "{name}: no issues");
        assert_eq!(issues[0].path, v["expectPath"].as_str().unwrap(), "{name}: {issues:?}");
    }
}

#[test]
fn broken_layout_json_falls_back_without_touching_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("layout.json");
    std::fs::write(&path, "{ this is not json").unwrap();
    let loaded = load_or_init(dir.path());
    assert_eq!(loaded.layout.pages.len(), 1);
    assert_eq!(loaded.layout.pages[0].id, "main");
    assert!(loaded.layout.pages[0].items.is_empty());
    assert!(loaded.issues.iter().any(|(f, _)| f == "layout.json"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ this is not json");
}

#[test]
fn action_validation_rules() {
    let mk = |v: Value| -> ActionsFile { serde_json::from_value(json!({ "version": 1, "actions": { "a": v } })).unwrap() };
    assert!(validate_actions(&mk(json!({ "name": "x", "type": "open_url", "url": "https://x" }))).is_empty());
    assert!(!validate_actions(&mk(json!({ "name": "x", "type": "open_url", "url": "ftp://x" }))).is_empty());
    assert!(!validate_actions(&mk(json!({ "name": "x", "type": "hotkey", "keys": ["f25"] }))).is_empty());
    assert!(validate_actions(&mk(json!({ "name": "x", "type": "hotkey", "keys": ["ctrl", "f24"] }))).is_empty());
    assert!(!validate_actions(&mk(json!({ "name": "x", "type": "hotkey", "keys": ["ctrl", "ctrl"] }))).is_empty());
}

#[test]
fn config_and_display_validation_rules() {
    let mut config = load_or_init(tempfile::tempdir().unwrap().path()).config;
    assert!(validate_config(&config).is_empty());
    config.port = 80;
    assert!(!validate_config(&config).is_empty());

    let actions = default_actions();
    let mut layout = default_layout();
    layout.display.sleep_after_sec = 30;
    layout.display.dim_after_sec = 60;
    assert!(!validate_layout(&layout, &actions).is_empty());
}

#[test]
fn atomic_save_leaves_no_tmp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.json");
    save_json_atomic(&path, &json!({ "a": 1 })).unwrap();
    save_json_atomic(&path, &json!({ "a": 2 })).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\n  \"a\": 2\n}\n");
    assert!(!dir.path().join("x.json.tmp").exists());
}

#[test]
fn files_written_with_a_utf8_bom_are_read() {
    // Windows PowerShell 5.1 `Set-Content -Encoding utf8` writes a BOM.
    let dir = tempfile::tempdir().unwrap();
    let actions = "\u{feff}{\"version\":1,\"actions\":{\"charmap\":{\"name\":\"Charmap\",\"type\":\"launch\",\"program\":\"charmap.exe\"}}}";
    std::fs::write(dir.path().join("actions.json"), actions).unwrap();
    let loaded = load_or_init(dir.path());
    assert!(loaded.issues.is_empty(), "{:?}", loaded.issues);
    assert!(loaded.actions.actions.contains_key("charmap"));
}

/// DATA_MODEL.md 11.6: passwords and sign-in tokens typed into config.json as plain text are encrypted at the first load and
/// written back; the original strings are gone from the file.
#[test]
fn plain_secrets_in_config_json_are_protected_at_load_and_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = decka_core::config::default_config();
    config.obs = Some(serde_json::from_value(json!({ "enabled": true, "port": 4455, "password": "plain-obs-pw" })).unwrap());
    config.mail = Some(serde_json::from_value(json!({ "accounts": [{ "id": "m1", "name": "Mail", "host": "imap.example.org", "port": 993, "user": "me", "password": "plain-mail-pw" }] })).unwrap());
    config.youtube = Some(serde_json::from_value(json!({ "enabled": true, "clientId": "cid", "secret": "plain-yt-secret", "tokens": "plain-yt-tokens" })).unwrap());
    config.twitch = Some(serde_json::from_value(json!({ "enabled": true, "clientId": "cid", "tokens": "plain-tw-tokens" })).unwrap());
    save_json_atomic(&dir.path().join("config.json"), &config).unwrap();

    let loaded = load_or_init(dir.path());
    assert!(loaded.issues.is_empty(), "{:?}", loaded.issues);
    let on_disk = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    for plain in ["plain-obs-pw", "plain-mail-pw", "plain-yt-secret", "plain-yt-tokens", "plain-tw-tokens"] {
        assert!(!on_disk.contains(plain), "{plain} is still in config.json");
    }
    assert!(loaded.config.obs.unwrap().password.unwrap().starts_with("dpapi:"));
    assert!(loaded.config.mail.unwrap().accounts[0].password.as_deref().unwrap().starts_with("dpapi:"));
    assert!(loaded.config.youtube.as_ref().unwrap().tokens.as_deref().unwrap().starts_with("dpapi:"));
    // The blobs read back as what was typed, and a second load changes nothing.
    assert_eq!(decka_core::dpapi::unprotect(loaded.config.youtube.unwrap().secret.as_deref().unwrap()).unwrap(), "plain-yt-secret");
    let again = load_or_init(dir.path());
    assert!(again.issues.is_empty());
    assert_eq!(std::fs::read_to_string(dir.path().join("config.json")).unwrap(), on_disk);
}
