use std::sync::Arc;
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::HttpCheck;
use decka_core::{Agent, AgentOptions, SaveResult};
use serde_json::Value;

fn check(id: &str) -> HttpCheck {
    HttpCheck {
        id: id.to_string(),
        label: id.to_uppercase(),
        url: "http://127.0.0.1:1/".to_string(),
        interval_sec: 3600,
        timeout_sec: 1,
        expect_status: None,
        group: None,
    }
}

async fn wait_until(secs: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    done()
}

async fn start_with(dir: &std::path::Path, edit: impl FnOnce(&mut decka_core::model::ConfigFile)) -> Agent {
    let mut config = default_config();
    config.collectors.sys.interval_sec = 1;
    edit(&mut config);
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
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

fn status_ids(agent: &Agent) -> Vec<String> {
    agent.store().get("statuses").as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn replacing_a_check_swaps_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start_with(dir.path(), |c| {
        c.collectors.sys.enabled = false;
        c.collectors.checks = vec![check("a")];
    })
    .await;
    assert!(status_ids(&agent).contains(&"a".to_string()));

    let mut config = agent.config();
    config.collectors.checks = vec![check("b")];
    assert_eq!(agent.save_config(config), SaveResult::Saved);

    assert!(wait_until(2, || status_ids(&agent) == ["b"]).await, "statuses: {:?}", status_ids(&agent));
    agent.shutdown().await;
}

/// Regression: the settings window calls `save_config` from a thread that has no Tokio context;
/// starting the HTTP-check tasks used to panic there ("there is no reactor running").
#[tokio::test]
async fn save_config_works_from_a_thread_without_a_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start_with(dir.path(), |c| c.collectors.sys.enabled = false).await;
    let mut config = agent.config();
    config.collectors.checks = vec![check("from-thread")];
    let for_thread = agent.clone();
    let result = std::thread::spawn(move || for_thread.save_config(config)).join().expect("save_config panicked");
    assert_eq!(result, SaveResult::Saved);
    assert!(wait_until(2, || status_ids(&agent).contains(&"from-thread".to_string())).await);
    agent.shutdown().await;
}

#[tokio::test]
async fn disabling_the_sys_collector_sets_the_section_to_null() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start_with(dir.path(), |_| {}).await;

    // A subscriber must exist for the collector to sample at all.
    let _rx = agent.store().subscribe();
    assert!(wait_until(4, || agent.store().get("sys") != Value::Null).await, "sys never appeared");

    let mut config = agent.config();
    config.collectors.sys.enabled = false;
    assert_eq!(agent.save_config(config), SaveResult::Saved);
    assert!(wait_until(3, || agent.store().get("sys") == Value::Null).await, "sys is still {}", agent.store().get("sys"));

    // And it stays null: no thread is running any more.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(agent.store().get("sys"), Value::Null);
    agent.shutdown().await;
}

#[tokio::test]
async fn an_invalid_config_is_rejected_and_the_file_stays() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start_with(dir.path(), |c| c.collectors.sys.enabled = false).await;
    let before = std::fs::read_to_string(dir.path().join("config.json")).unwrap();

    let mut config = agent.config();
    config.port = 80;
    match agent.save_config(config) {
        SaveResult::Rejected(issues) => assert_eq!(issues[0].path, "/port"),
        SaveResult::Saved => panic!("port 80 was accepted"),
    }
    assert_eq!(std::fs::read_to_string(dir.path().join("config.json")).unwrap(), before);
    assert_ne!(agent.config().port, 80);
    agent.shutdown().await;
}

#[tokio::test]
async fn regenerating_the_token_changes_it_and_persists_it() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start_with(dir.path(), |c| c.collectors.sys.enabled = false).await;
    let old = agent.config().api_token;
    let new = agent.regenerate_api_token().unwrap();
    assert_ne!(old, new);
    assert_eq!(agent.config().api_token, new);
    let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join("config.json")).unwrap()).unwrap();
    assert_eq!(on_disk["apiToken"], new.as_str());
    agent.shutdown().await;
}

#[tokio::test]
async fn status_subscribers_hear_about_connections_and_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start_with(dir.path(), |c| c.collectors.sys.enabled = false).await;
    let mut rx = agent.subscribe_status();
    assert!(rx.borrow().listening.is_some());

    let status = agent.restart_server().await;
    assert!(status.listening.is_some());
    // A restart goes through "not listening" and back; the channel reports the changes.
    assert!(tokio::time::timeout(Duration::from_secs(1), rx.changed()).await.is_ok());
    agent.shutdown().await;
}

#[tokio::test]
async fn pairing_info_has_a_url_with_the_code_and_a_qr_svg() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start_with(dir.path(), |c| {
        c.collectors.sys.enabled = false;
        c.bind = "127.0.0.1".to_string();
    })
    .await;
    let info = agent.start_pairing();
    assert_eq!(info.code.len(), 8);
    assert!(info.url.starts_with("http://127.0.0.1:"), "{}", info.url);
    assert!(info.url.contains(&format!("#c={}&h=127.0.0.1&p=", info.code)), "{}", info.url);
    assert!(info.qr_svg.contains("<svg"));
    agent.shutdown().await;
}
