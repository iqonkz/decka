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
