//! The `reminders` and `lists` sections of the phone state (DATA_MODEL.md 13.7): they follow the files, are cut to 5 reminders
//! and 50 items, and a toggle from the phone changes only an item the agent issued.

use std::sync::Arc;
use std::time::Duration;

use decka_core::actions::DryRunner;
use decka_core::assistant::{Day, Local, When};
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::SwitchConfig;
use decka_core::{Agent, AgentOptions};
use serde_json::Value;

async fn start(dir: &std::path::Path) -> Agent {
    let mut config = default_config();
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()), config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None }).await.unwrap()
}

/// Waits until the section satisfies `ok` (the publisher runs in the background).
async fn until(agent: &Agent, section: &str, ok: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..100 {
        let v = agent.store().get(section);
        if ok(&v) {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("section {section} did not reach the expected value: {}", agent.store().get(section));
}

#[tokio::test]
async fn sections_start_empty_and_follow_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    assert_eq!(agent.store().get("reminders"), serde_json::json!({ "items": [] }));
    assert_eq!(agent.store().get("lists"), serde_json::json!({ "lists": [] }));

    agent.assistant().list_add("Shopping", "milk").unwrap();
    let v = until(&agent, "lists", |v| v["lists"].as_array().is_some_and(|l| !l.is_empty())).await;
    assert_eq!(v["lists"][0]["name"], "Shopping");
    assert_eq!(v["lists"][0]["items"][0]["text"], "milk");
    assert_eq!(v["lists"][0]["items"][0]["done"], false);
    let (list_id, item_id) = (v["lists"][0]["id"].as_str().unwrap().to_string(), v["lists"][0]["items"][0]["id"].as_str().unwrap().to_string());

    // A tick mark from the phone is a patch; an id the agent did not issue changes nothing.
    assert!(!agent.assistant().toggle(&list_id, "0000000000000000"));
    assert!(agent.assistant().toggle(&list_id, &item_id));
    until(&agent, "lists", |v| v["lists"][0]["items"][0]["done"] == true).await;
    assert!(agent.assistant().toggle(&list_id, &item_id));
    until(&agent, "lists", |v| v["lists"][0]["items"][0]["done"] == false).await;

    // The same files give the same sections after a restart.
    agent.shutdown().await;
    let again = start(dir.path()).await;
    let v = until(&again, "lists", |v| v["lists"].as_array().is_some_and(|l| !l.is_empty())).await;
    assert_eq!(v["lists"][0]["items"][0]["text"], "milk");
    again.shutdown().await;
}

#[tokio::test]
async fn the_phone_gets_at_most_five_reminders_and_fifty_items() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    let now = Local::now();
    for i in 0..8 {
        agent.assistant().add_reminder(&format!("reminder {i}"), &When::InMinutes(10 + i), &now).unwrap();
    }
    agent.assistant().add_reminder("tomorrow", &When::Day { day: Day::Tomorrow, hour: 9, minute: 0 }, &now).unwrap();
    let v = until(&agent, "reminders", |v| v["items"].as_array().is_some_and(|a| a.len() == 5)).await;
    let texts: Vec<&str> = v["items"].as_array().unwrap().iter().map(|r| r["text"].as_str().unwrap()).collect();
    assert_eq!(texts, ["reminder 0", "reminder 1", "reminder 2", "reminder 3", "reminder 4"], "soonest first");
    assert!(v["items"][0]["at"].as_str().unwrap().len() == 16, "local ISO, minutes");

    for i in 0..60 {
        agent.assistant().list_add("Big", &format!("item {i}")).unwrap();
    }
    let v = until(&agent, "lists", |v| v["lists"].as_array().is_some_and(|l| !l.is_empty()) && v["lists"][0]["items"].as_array().is_some_and(|i| i.len() == 50)).await;
    assert_eq!(v["lists"][0]["items"][49]["text"], "item 49");
    agent.shutdown().await;
}

#[test]
fn the_two_widgets_are_valid_layout_items_with_the_documented_sizes() {
    use decka_core::config::{default_actions, default_layout};
    use decka_core::model::LayoutItem;
    use decka_core::validate::validate_layout;
    let mut layout = default_layout();
    layout.pages[0].items.clear();
    let item = |json: serde_json::Value| serde_json::from_value::<LayoutItem>(json).unwrap();
    let id = "0123456789abcdef";
    let issues = |items: Vec<LayoutItem>| {
        let mut l = layout.clone();
        l.pages[0].items = items;
        validate_layout(&l, &default_actions()).iter().map(|i| i.message.clone()).collect::<Vec<_>>()
    };
    assert!(issues(vec![item(serde_json::json!({ "id": "a", "kind": "reminders", "x": 0, "y": 0, "w": 2, "h": 2 })), item(serde_json::json!({ "id": "b", "kind": "list", "x": 2, "y": 0, "w": 2, "h": 2, "listId": id }))]).is_empty());
    assert!(issues(vec![item(serde_json::json!({ "id": "a", "kind": "list", "x": 0, "y": 0, "w": 2, "h": 1 }))]).is_empty(), "2x1 is the smallest");
    assert!(!issues(vec![item(serde_json::json!({ "id": "a", "kind": "reminders", "x": 0, "y": 0, "w": 1, "h": 1 }))]).is_empty(), "1x1 is too small");
    assert!(!issues(vec![item(serde_json::json!({ "id": "a", "kind": "list", "x": 0, "y": 0, "w": 2, "h": 2, "listId": "../x" }))]).is_empty(), "the id is 16 hex characters");
    assert_eq!(item(serde_json::json!({ "id": "a", "kind": "list", "x": 0, "y": 0, "w": 2, "h": 2 })).kind_name(), "list");
}