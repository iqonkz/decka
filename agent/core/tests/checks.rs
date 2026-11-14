use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use decka_core::actions::DryRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::HttpCheck;
use decka_core::{Agent, AgentOptions};
use serde_json::Value;

/// Local test server: /ok -> 200, /fail -> 500, /slow -> 200 after 3 s, /moved -> 302.
async fn test_server() -> u16 {
    let app = Router::new()
        .route("/ok", get(|| async { "fine" }))
        .route("/fail", get(|| async { StatusCode::INTERNAL_SERVER_ERROR }))
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(3)).await;
                "late"
            }),
        )
        .route("/moved", get(|| async { (StatusCode::FOUND, [("location", "/ok")]) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

fn check(id: &str, url: String) -> HttpCheck {
    HttpCheck {
        id: id.to_string(),
        label: id.to_uppercase(),
        url,
        interval_sec: 10,
        timeout_sec: 1,
        expect_status: None,
        group: Some("services".to_string()),
    }
}

async fn agent_with(checks: Vec<HttpCheck>, dir: &std::path::Path) -> Agent {
    let mut config = default_config();
    config.collectors.sys.enabled = false;
    config.collectors.checks = checks;
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

fn status_of(agent: &Agent, id: &str) -> Option<Value> {
    agent.store().get("statuses").as_array().unwrap().iter().find(|s| s["id"] == id).cloned()
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

#[tokio::test]
async fn checks_publish_ok_error_timeout_and_unreachable() {
    let port = test_server().await;
    let dir = tempfile::tempdir().unwrap();
    let agent = agent_with(
        vec![
            check("up", format!("http://127.0.0.1:{port}/ok")),
            check("broken", format!("http://127.0.0.1:{port}/fail")),
            check("slow", format!("http://127.0.0.1:{port}/slow")),
            // Windows answers a closed local port only after ~2 s (SYN retries), so this one gets a longer timeout.
            HttpCheck { timeout_sec: 5, ..check("dead", "http://127.0.0.1:1/".to_string()) },
            check("redirect", format!("http://127.0.0.1:{port}/moved")),
        ],
        dir.path(),
    )
    .await;

    // Right after the start every check is published as "unknown".
    for id in ["up", "broken", "slow", "dead", "redirect"] {
        assert!(status_of(&agent, id).is_some(), "{id} missing");
    }

    let all_done = wait_until(8, || {
        ["up", "broken", "slow", "dead", "redirect"].iter().all(|id| status_of(&agent, id).is_some_and(|s| s["state"] != "unknown"))
    })
    .await;
    assert!(all_done, "checks did not finish in 8 s: {}", agent.store().get("statuses"));

    let up = status_of(&agent, "up").unwrap();
    assert_eq!(up["state"], "ok");
    assert!(up["text"].as_str().unwrap().starts_with("200 · "), "{up}");
    assert_eq!(up["source"], "check");
    assert_eq!(up["group"], "services");
    assert_eq!(up["label"], "UP");

    let broken = status_of(&agent, "broken").unwrap();
    assert_eq!((broken["state"].as_str(), broken["text"].as_str()), (Some("error"), Some("500")));

    let slow = status_of(&agent, "slow").unwrap();
    assert_eq!((slow["state"].as_str(), slow["text"].as_str()), (Some("error"), Some("timeout")));

    let dead = status_of(&agent, "dead").unwrap();
    assert_eq!((dead["state"].as_str(), dead["text"].as_str()), (Some("error"), Some("unreachable")));

    // Redirects are not followed: a 302 is itself an answer in 200..399.
    let redirect = status_of(&agent, "redirect").unwrap();
    assert_eq!(redirect["state"], "ok");
    assert!(redirect["text"].as_str().unwrap().starts_with("302 · "));
    agent.shutdown().await;
}
