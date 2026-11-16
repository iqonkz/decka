//! The update check against a local stand-in for GitHub (no network): a newer release is offered, a failed check keeps
//! what was known, and the answer reaches subscribers. ARCHITECTURE.md 8a.

use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::{Agent, AgentOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const RELEASE_FIXTURE: &str = include_str!("../../../shared/fixtures/github-release-latest.json");

/// The fixture with a version far ahead of any real one, so the tests do not break at every release.
fn release() -> &'static str {
    Box::leak(RELEASE_FIXTURE.replace("0.2.0", "9.9.0").into_boxed_str())
}

/// Answers every request with `status` and `body`; returns the address and the user agent of the last request.
async fn fake_github(status: u16, body: &'static str) -> (String, Arc<std::sync::Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/releases/latest", listener.local_addr().unwrap());
    let seen = Arc::new(std::sync::Mutex::new(String::new()));
    let for_task = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            *for_task.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).to_string();
            let reply = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    (url, seen)
}

async fn start(dir: &std::path::Path) -> Agent {
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None }).await.unwrap()
}

#[tokio::test]
async fn a_newer_release_is_offered_with_a_user_agent_and_reaches_subscribers() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    assert_eq!(agent.update_status().checked_at, None);
    let mut updates = agent.subscribe_updates();

    let (url, seen) = fake_github(200, release()).await;
    let status = agent.check_updates_at(&url).await;
    let found = status.available.clone().expect("9.9.0 is newer than the agent");
    assert_eq!((found.version.as_str(), found.url.as_str()), ("9.9.0", "https://github.com/iqonkz/decka/releases/tag/v9.9.0"));
    assert!(status.checked_at.is_some() && !status.failed);
    assert_eq!(agent.update_status(), status);
    assert!(updates.has_changed().unwrap());
    assert_eq!(updates.borrow_and_update().available.as_ref().map(|a| a.version.as_str()), Some("9.9.0"));

    let request = seen.lock().unwrap().to_lowercase();
    assert!(request.contains("user-agent: decka-agent/"), "{request}");
}

#[tokio::test]
async fn a_failed_check_keeps_the_known_release_and_says_it_failed() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    let (good, _) = fake_github(200, release()).await;
    agent.check_updates_at(&good).await;

    let (bad, _) = fake_github(500, "oops").await;
    let after = agent.check_updates_at(&bad).await;
    assert!(after.failed);
    assert_eq!(after.available.as_ref().map(|a| a.version.as_str()), Some("9.9.0"));

    // Nothing listens there at all: the same.
    let nowhere = agent.check_updates_at("http://127.0.0.1:1/").await;
    assert!(nowhere.failed);
}

#[tokio::test]
async fn the_same_or_an_older_release_offers_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    static SAME: &str = r#"{ "tag_name": "v0.1.0", "html_url": "https://github.com/iqonkz/decka/releases/tag/v0.1.0", "published_at": "2026-11-01T00:00:00Z", "prerelease": false }"#;
    let (url, _) = fake_github(200, SAME).await;
    let status = agent.check_updates_at(&url).await;
    assert_eq!(status.available, None);
    assert!(!status.failed);
}

#[tokio::test]
async fn only_release_pages_of_the_project_can_be_opened() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    assert!(agent.open_release("https://evil.example/").is_err());
    assert!(agent.open_release("https://github.com/iqonkz/decka-evil/releases/").is_err());
}
