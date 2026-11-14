use std::sync::Arc;
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::{Agent, AgentOptions, SaveResult};
use serde_json::Value;

async fn start(dir: &std::path::Path) -> Agent {
    Agent::start(AgentOptions {
        config_dir: Some(dir.to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        runner: Arc::new(DryRunner),
        fixed_pair_code: None,
    })
    .await
    .unwrap()
}

/// Polls `check` for up to `secs` seconds.
async fn wait_for(secs: u64, mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    check()
}

fn page_name(agent: &Agent) -> String {
    agent.store().get("layout")["pages"][0]["name"].as_str().unwrap().to_string()
}

fn read_layout(dir: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("layout.json")).unwrap()).unwrap()
}

#[tokio::test]
async fn valid_layout_edit_is_applied_within_two_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    assert_eq!(page_name(&agent), "Main");

    let mut layout = read_layout(dir.path());
    layout["pages"][0]["name"] = Value::from("Renamed");
    std::fs::write(dir.path().join("layout.json"), serde_json::to_string_pretty(&layout).unwrap()).unwrap();

    assert!(wait_for(2, || page_name(&agent) == "Renamed").await, "layout section did not change");
    assert_eq!(agent.layout().pages[0].name, "Renamed");
    agent.shutdown().await;
}

#[tokio::test]
async fn invalid_layout_edit_keeps_the_old_layout_and_notifies() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;

    let mut layout = read_layout(dir.path());
    layout["pages"][0]["items"][4]["x"] = Value::from(1); // b-prev now overlaps b-play
    std::fs::write(dir.path().join("layout.json"), serde_json::to_string_pretty(&layout).unwrap()).unwrap();

    let notified = wait_for(2, || {
        let list = agent.store().get("notifications");
        list.as_array().unwrap().iter().any(|n| n["level"] == "error" && n["title"] == "layout.json invalid")
    })
    .await;
    assert!(notified, "no error notification");
    assert_eq!(page_name(&agent), "Main");
    assert_eq!(agent.layout().pages[0].items.len(), 15);
    agent.shutdown().await;
}

#[tokio::test]
async fn save_layout_with_overlap_is_rejected_and_file_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    let before = std::fs::read_to_string(dir.path().join("layout.json")).unwrap();

    let mut layout = serde_json::to_value(agent.layout()).unwrap();
    layout["pages"][0]["items"][4]["x"] = Value::from(1); // b-prev onto b-play
    let result = agent.save_layout(serde_json::from_value(layout).unwrap());
    match result {
        SaveResult::Rejected(issues) => assert_eq!(issues[0].path, "/pages/0/items/5"),
        SaveResult::Saved => panic!("overlap was accepted"),
    }
    assert_eq!(std::fs::read_to_string(dir.path().join("layout.json")).unwrap(), before);
    agent.shutdown().await;
}

#[tokio::test]
async fn save_layout_writes_updates_the_section_and_is_not_reapplied_by_the_watcher() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;

    let mut layout = agent.layout();
    layout.pages[0].name = "Saved".to_string();
    assert_eq!(agent.save_layout(layout), SaveResult::Saved);
    assert_eq!(page_name(&agent), "Saved");
    assert_eq!(read_layout(dir.path())["pages"][0]["name"], "Saved");

    // The watcher must treat our own write as already applied: no further change, no notification.
    let rev = agent.store().rev();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(agent.store().rev(), rev);
    agent.shutdown().await;
}

#[tokio::test]
async fn removing_an_action_used_by_a_button_is_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;

    let mut actions = agent.actions();
    actions.actions.remove("media-play");
    assert_eq!(agent.save_actions(actions), SaveResult::Saved);
    assert!(!agent.actions().actions.contains_key("media-play"));
    // The layout keeps its dangling reference.
    assert!(agent.layout().pages[0].items.iter().any(|i| matches!(i, decka_core::model::LayoutItem::Button(b) if b.action_id.as_deref() == Some("media-play"))));
    agent.shutdown().await;
}
