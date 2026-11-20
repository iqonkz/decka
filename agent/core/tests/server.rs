use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::{Agent, AgentOptions};
use serde_json::{json, Value};

async fn start(dir: &std::path::Path, port: u16) -> Agent {
    Agent::start(AgentOptions {
        config_dir: Some(dir.to_path_buf()),
        client_dist: None,
        port_override: Some(port),
        runner: Arc::new(DryRunner),
        fixed_pair_code: None,
    })
    .await
    .unwrap()
}

fn port_of(agent: &Agent) -> u16 {
    agent.status().listening.expect("agent is listening").port
}

#[tokio::test]
async fn health_and_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), 0).await;
    let base = format!("http://127.0.0.1:{}", port_of(&agent));

    let res = reqwest::get(format!("{base}/api/health")).await.unwrap();
    assert_eq!(res.status(), 200);
    assert!(res.headers().get("access-control-allow-origin").is_none());
    assert_eq!(res.headers().get("x-content-type-options").unwrap(), "nosniff");
    let body: Value = serde_json::from_str(&res.text().await.unwrap()).unwrap();
    assert_eq!(body, json!({ "name": "decka", "version": decka_core::VERSION, "protocol": 2 }));

    let res = reqwest::get(format!("{base}/api/nope")).await.unwrap();
    assert_eq!(res.status(), 404);
    let body: Value = serde_json::from_str(&res.text().await.unwrap()).unwrap();
    assert_eq!(body["error"]["code"], "NOT_FOUND");

    // No client_dist: the root answers 404 with a plain text hint.
    let res = reqwest::get(format!("{base}/")).await.unwrap();
    assert_eq!(res.status(), 404);
    assert_eq!(res.text().await.unwrap(), "client not bundled");

    agent.shutdown().await;
}

#[tokio::test]
async fn static_client_is_served_with_spa_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let dist = tempfile::tempdir().unwrap();
    std::fs::write(dist.path().join("index.html"), "<html>decka</html>").unwrap();
    let agent = Agent::start(AgentOptions {
        config_dir: Some(dir.path().to_path_buf()),
        client_dist: Some(dist.path().to_path_buf()),
        port_override: Some(0),
        runner: Arc::new(DryRunner),
        fixed_pair_code: None,
    })
    .await
    .unwrap();
    let base = format!("http://127.0.0.1:{}", port_of(&agent));

    for path in ["/", "/index.html", "/some/route"] {
        let res = reqwest::get(format!("{base}{path}")).await.unwrap();
        assert_eq!(res.status(), 200, "{path}");
        assert_eq!(res.headers().get("cache-control").unwrap(), "no-cache", "{path}");
        assert_eq!(res.text().await.unwrap(), "<html>decka</html>");
    }
    agent.shutdown().await;
}

#[tokio::test]
async fn second_agent_on_a_busy_port_does_not_listen() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let first = start(dir1.path(), 0).await;
    let second = start(dir2.path(), port_of(&first)).await;
    let status = second.status();
    assert!(status.listening.is_none());
    assert!(status.listen_error.is_some());
    first.shutdown().await;
    second.shutdown().await;
}
