//! The M15 contract (T15.1): new actions, widgets, sizes, reserved variables, the weather / OBS config.

use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::config::{default_actions, default_config, default_layout, save_json_atomic};
use decka_core::model::*;
use decka_core::validate::{is_reserved_var, validate_actions, validate_config, validate_layout};
use decka_core::{Agent, AgentOptions, SaveResult};
use serde_json::{json, Value};

fn actions_m15() -> ActionsFile {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/fixtures/actions.m15.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn with(id: &str, action: Value) -> ActionsFile {
    let mut file = actions_m15();
    file.actions.insert(id.to_string(), serde_json::from_value(action).unwrap());
    file
}

fn problems(file: &ActionsFile) -> Vec<(String, String)> {
    validate_actions(file).into_iter().map(|i| (i.path, i.message)).collect()
}

fn paths(file: &ActionsFile) -> Vec<String> {
    problems(file).into_iter().map(|p| p.0).collect()
}

fn layout_with(items: Value) -> LayoutFile {
    let mut layout = default_layout();
    layout.pages = serde_json::from_value(json!([{ "id": "main", "name": "Main", "items": items }])).unwrap();
    layout
}

fn layout_messages(items: Value, actions: &ActionsFile) -> Vec<String> {
    validate_layout(&layout_with(items), actions).into_iter().map(|i| i.message).collect()
}

#[test]
fn the_m15_actions_fixture_is_valid() {
    assert_eq!(problems(&actions_m15()), vec![]);
}

#[test]
fn the_default_actions_toggle_nothing_reserved() {
    assert_eq!(problems(&default_actions()), vec![]);
    assert!(default_actions().actions.values().all(|a| a.toggle_var().is_none_or(|v| !is_reserved_var(v))));
}

#[test]
fn the_old_default_vol_mute_is_migrated_silently_and_nothing_else_is() {
    use decka_core::config::{load_or_init, migrate_actions};
    let dir = tempfile::tempdir().unwrap();
    let mut old = default_actions();
    old.actions.insert("vol-mute".into(), Action::Media { name: "Mute".into(), toggle_var: Some("audio.muted".into()), op: MediaOp::Mute });
    save_json_atomic(&dir.path().join("actions.json"), &old).unwrap();
    let loaded = load_or_init(dir.path());
    assert!(loaded.issues.iter().all(|(file, _)| file != "actions.json"), "{:?}", loaded.issues);
    assert_eq!(loaded.actions, default_actions());

    // Another action with a reserved toggleVar, or another toggleVar on vol-mute, is still an error.
    let mut other = default_actions();
    other.actions.insert("m".into(), Action::Media { name: "M".into(), toggle_var: Some("audio.muted".into()), op: MediaOp::Mute });
    assert!(!migrate_actions(&mut other));
    assert_eq!(paths(&other), ["/actions/m/toggleVar"]);
    let mut renamed = default_actions();
    renamed.actions.insert("vol-mute".into(), Action::Media { name: "Mute".into(), toggle_var: Some("mic.muted".into()), op: MediaOp::Mute });
    assert!(!migrate_actions(&mut renamed));
    assert_eq!(paths(&renamed), ["/actions/vol-mute/toggleVar"]);
}

#[test]
fn sequence_limits() {
    let run = |n: usize| with("s", json!({ "name": "S", "type": "sequence", "steps": (0..n).map(|_| json!({ "actionId": "media-play" })).collect::<Vec<_>>() }));
    assert_eq!(problems(&run(10)), vec![]);
    assert_eq!(problems(&run(11)), vec![("/actions/s/steps".to_string(), "a sequence has 1..10 steps".to_string())]);
    assert_eq!(paths(&run(0)), ["/actions/s/steps"]);

    let nested = with("s", json!({ "name": "S", "type": "sequence", "steps": [{ "actionId": "go-live" }] }));
    assert_eq!(problems(&nested), vec![("/actions/s/steps/0".to_string(), "a sequence cannot contain another sequence".to_string())]);
    let itself = with("s", json!({ "name": "S", "type": "sequence", "steps": [{ "actionId": "s" }] }));
    assert_eq!(paths(&itself), ["/actions/s/steps/0"]);

    let missing = with("s", json!({ "name": "S", "type": "sequence", "steps": [{ "actionId": "nope" }] }));
    assert_eq!(problems(&missing), vec![("/actions/s/steps/0".to_string(), "action \"nope\" does not exist".to_string())]);
}

#[test]
fn sequence_pauses() {
    let pauses = |list: &[i64]| with("s", json!({ "name": "S", "type": "sequence", "steps": list.iter().map(|d| json!({ "delayMs": d })).collect::<Vec<_>>() }));
    assert_eq!(problems(&pauses(&[10000, 5000])), vec![]);
    assert_eq!(problems(&pauses(&[10000, 5001])), vec![("/actions/s/steps".to_string(), "the pauses add up to more than 15000 ms".to_string())]);
    assert_eq!(paths(&pauses(&[49, 10001])), ["/actions/s/steps/0", "/actions/s/steps/1"]);
}

#[test]
fn obs_scene_and_input_go_with_their_ops_only() {
    let obs = |extra: Value| {
        let mut a = json!({ "name": "O", "type": "obs" });
        a.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        with("o", a)
    };
    assert_eq!(problems(&obs(json!({ "op": "scene", "scene": "Gaming" }))), vec![]);
    assert_eq!(problems(&obs(json!({ "op": "input_mute_toggle", "input": "Mic" }))), vec![]);
    assert_eq!(problems(&obs(json!({ "op": "save_replay" }))), vec![]);
    assert_eq!(paths(&obs(json!({ "op": "scene" }))), ["/actions/o/scene"]);
    assert_eq!(paths(&obs(json!({ "op": "record_toggle", "scene": "Gaming" }))), ["/actions/o/scene"]);
    assert_eq!(paths(&obs(json!({ "op": "input_mute_toggle" }))), ["/actions/o/input"]);
    assert_eq!(paths(&obs(json!({ "op": "scene", "scene": "A", "input": "B" }))), ["/actions/o/input"]);
    assert_eq!(paths(&obs(json!({ "op": "scene", "scene": "x".repeat(101) }))), ["/actions/o/scene"]);
}

#[test]
fn a_reserved_variable_cannot_be_toggled() {
    let mute = with("m", json!({ "name": "Mute", "type": "media", "op": "mute", "toggleVar": "audio.muted" }));
    assert_eq!(problems(&mute), vec![("/actions/m/toggleVar".to_string(), "toggleVar: \"audio.muted\" is set by Decka itself — remove it".to_string())]);
    for name in ["mic.muted", "media.playing", "obs.recording"] {
        let a = with("m", json!({ "name": "M", "type": "media", "op": "mute", "toggleVar": name }));
        assert_eq!(paths(&a), ["/actions/m/toggleVar"], "{name}");
    }
    assert_eq!(problems(&with("m", json!({ "name": "M", "type": "media", "op": "mute", "toggleVar": "my.flag" }))), vec![]);
}

#[test]
fn the_new_keys_are_allowed() {
    for key in ["printscreen", "insert", "period", "comma", "minus", "equal", "slash", "semicolon"] {
        let a = with("k", json!({ "name": "K", "type": "hotkey", "keys": ["win", key] }));
        assert_eq!(problems(&a), vec![], "{key}");
    }
    assert_eq!(paths(&with("k", json!({ "name": "K", "type": "hotkey", "keys": ["capslock"] }))), ["/actions/k/keys/0"]);
}

#[test]
fn widget_sizes() {
    let actions = actions_m15();
    let cases = [
        ("volume", 3, 1, true), ("volume", 1, 4, true), ("volume", 1, 1, false), ("volume", 2, 2, false), ("volume", 5, 1, false),
        ("timer", 1, 1, true), ("timer", 2, 2, true), ("timer", 1, 2, false), ("timer", 3, 1, false),
        ("weather", 1, 1, true), ("weather", 2, 1, true), ("weather", 3, 2, true), ("weather", 4, 2, true), ("weather", 3, 1, false), ("weather", 2, 2, false),
        ("obs", 2, 1, true), ("obs", 4, 2, true), ("obs", 1, 1, false), ("obs", 5, 1, false),
    ];
    for (kind, w, h, allowed) in cases {
        let issues = layout_messages(json!([{ "id": "w", "kind": kind, "x": 0, "y": 0, "w": w, "h": h }]), &actions);
        assert_eq!(issues.is_empty(), allowed, "{kind} {w}x{h}: {issues:?}");
    }
    let sys = |w: i32, h: i32, graph: bool| layout_messages(json!([{ "id": "s", "kind": "sys", "x": 0, "y": 0, "w": w, "h": h, "graph": graph }]), &actions).is_empty();
    assert!(sys(4, 2, true) && sys(2, 1, true) && !sys(5, 2, true) && !sys(3, 1, false) && sys(2, 2, false));
}

#[test]
fn volume_actions_and_timer_presets() {
    let actions = actions_m15();
    let volume = |extra: Value| {
        let mut v = json!({ "id": "v", "kind": "volume", "x": 0, "y": 0, "w": 3, "h": 1 });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        layout_messages(json!([v]), &actions)
    };
    assert!(volume(json!({ "upAction": "vol-up", "downAction": "vol-down", "muteAction": "vol-mute" })).is_empty());
    assert_eq!(volume(json!({ "upAction": "nope" })), ["unknown upAction 'nope'"]);
    assert_eq!(volume(json!({ "muteAction": "nope" })), ["unknown muteAction 'nope'"]);

    let timer = |presets: Value| layout_messages(json!([{ "id": "t", "kind": "timer", "x": 0, "y": 0, "w": 2, "h": 1, "presets": presets }]), &actions);
    assert!(timer(json!([25, 5])).is_empty() && timer(json!([1, 2, 3, 4])).is_empty());
    for bad in [json!([]), json!([1, 2, 3, 4, 5]), json!([0]), json!([181])] {
        assert_eq!(timer(bad.clone()).len(), 1, "{bad}");
    }
}

#[test]
fn back_is_for_keys_of_normal_pages() {
    let actions = actions_m15();
    let back = json!({ "id": "b", "kind": "button", "x": 0, "y": 0, "w": 1, "h": 1, "navigate": "$back" });
    assert!(layout_messages(json!([back.clone()]), &actions).is_empty());

    let mut layout = layout_with(json!([{ "id": "o", "kind": "button", "x": 0, "y": 0, "w": 1, "h": 1, "popup": "grp" }]));
    layout.pages.push(serde_json::from_value(json!({ "id": "grp", "name": "Group", "popup": true, "items": [back.clone()] })).unwrap());
    let messages: Vec<String> = validate_layout(&layout, &actions).into_iter().map(|i| i.message).collect();
    assert_eq!(messages, ["a key inside a popup group cannot open a page or another group"]);

    let mut both = back;
    both["actionId"] = json!("media-play");
    assert_eq!(layout_messages(json!([both]), &actions), ["a button can either run an action or open a page, not both"]);
}

#[test]
fn weather_and_obs_config_is_checked() {
    let mut c = default_config();
    assert!(validate_config(&c).is_empty());
    c.weather = Some(WeatherConfig { enabled: true, place: "Almaty".into(), lat: 43.25, lon: 76.95, units: "metric".into() });
    c.obs = Some(ObsConfig { enabled: true, port: 4455, password: None, password_set: None });
    assert!(validate_config(&c).is_empty());

    let paths_of = |c: &ConfigFile| validate_config(c).into_iter().map(|i| i.path).collect::<Vec<_>>();
    let mut bad = c.clone();
    bad.weather = Some(WeatherConfig { enabled: true, place: String::new(), lat: 91.0, lon: -181.0, units: "kelvin".into() });
    bad.obs = Some(ObsConfig { enabled: true, port: 80, password: Some("hunter2".into()), password_set: None });
    assert_eq!(paths_of(&bad), ["/weather/place", "/weather/lat", "/weather/lon", "/weather/units", "/obs/port", "/obs/password"]);
}

#[test]
fn m15_item_kinds_round_trip_with_their_fields() {
    let items = json!([
        { "kind": "volume", "id": "v", "x": 0, "y": 0, "w": 3, "h": 1, "upAction": "vol-up", "downAction": "vol-down", "muteAction": "vol-mute" },
        { "kind": "timer", "id": "t", "x": 3, "y": 0, "w": 2, "h": 1, "presets": [25, 5] },
        { "kind": "weather", "id": "w", "x": 5, "y": 0, "w": 2, "h": 1 },
        { "kind": "obs", "id": "o", "x": 0, "y": 1, "w": 3, "h": 1 },
        { "kind": "sys", "id": "s", "x": 3, "y": 1, "w": 4, "h": 2, "graph": true }
    ]);
    let parsed: Vec<LayoutItem> = serde_json::from_value(items.clone()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), items);
}

// ----- the agent: HTTP, get_config / save_config, sections -----

async fn start(dir: &std::path::Path, mut config: ConfigFile) -> Agent {
    // The collectors would put variables and sections of their own into the state these tests look at.
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()), config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None })
        .await
        .unwrap()
}

#[tokio::test]
async fn http_refuses_to_write_or_delete_reserved_variables() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), default_config()).await;
    let base = format!("http://127.0.0.1:{}", agent.status().listening.unwrap().port);
    let token = agent.config().api_token;
    let client = reqwest::Client::new();

    for name in ["audio.volume", "mic.muted", "media.playing", "obs.scene"] {
        let put = client.put(format!("{base}/api/vars/{name}")).header("Authorization", format!("Bearer {token}")).body(r#"{"value":1}"#).send().await.unwrap();
        assert_eq!(put.status().as_u16(), 400, "PUT {name}");
        let body: Value = serde_json::from_str(&put.text().await.unwrap()).unwrap();
        assert_eq!((body["error"]["code"].as_str(), body["error"]["message"].as_str()), (Some("BAD_MESSAGE"), Some("reserved variable")), "{name}");
        let del = client.delete(format!("{base}/api/vars/{name}")).header("Authorization", format!("Bearer {token}")).send().await.unwrap();
        assert_eq!(del.status().as_u16(), 400, "DELETE {name}");
    }
    let ok = client.put(format!("{base}/api/vars/my.flag")).header("Authorization", format!("Bearer {token}")).body(r#"{"value":true}"#).send().await.unwrap();
    assert_eq!(ok.status().as_u16(), 200);
    assert_eq!(agent.store().get("vars"), json!({ "my.flag": true }));
    agent.shutdown().await;
}

#[tokio::test]
async fn the_obs_password_never_leaves_get_config_and_is_kept_by_save_config() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = default_config();
    config.obs = Some(ObsConfig { enabled: true, port: 4455, password: Some("dpapi:AAAA".into()), password_set: None });
    let agent = start(dir.path(), config).await;

    let shown = agent.config_for_window();
    let obs = shown.obs.clone().unwrap();
    assert_eq!((obs.password, obs.password_set), (None, Some(true)));
    assert!(!serde_json::to_string(&shown).unwrap().contains("dpapi"), "the window got the password");

    // The window sends the config back with its own idea of the password: it is ignored, the stored blob stays.
    let mut edited = shown;
    edited.obs.as_mut().unwrap().port = 4456;
    edited.obs.as_mut().unwrap().password = Some("dpapi:EVIL".into());
    assert_eq!(agent.save_config(edited), SaveResult::Saved);
    let stored = agent.config().obs.unwrap();
    assert_eq!((stored.port, stored.password.as_deref(), stored.password_set), (4456, Some("dpapi:AAAA"), None));
    let on_disk = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(on_disk.contains("dpapi:AAAA") && !on_disk.contains("EVIL") && !on_disk.contains("passwordSet"));
    agent.shutdown().await;
}

#[tokio::test]
async fn a_config_without_a_password_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = default_config();
    config.obs = Some(ObsConfig { enabled: false, port: 4455, password: None, password_set: None });
    let agent = start(dir.path(), config).await;
    assert_eq!(agent.config_for_window().obs.unwrap().password_set, Some(false));
    agent.shutdown().await;
}

#[tokio::test]
async fn the_state_has_weather_and_obs_sections_that_start_null() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), default_config()).await;
    let (_, state) = agent.store().snapshot();
    assert_eq!((&state["weather"], &state["obs"]), (&Value::Null, &Value::Null));
    agent.shutdown().await;
}
