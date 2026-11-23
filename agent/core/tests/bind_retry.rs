//! T21.16: a busy port is retried, and who holds it is reported. Loopback only (no firewall prompts).
use std::net::{SocketAddr, TcpListener as StdListener};
use std::sync::Arc;
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::agent::bind_retrying;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::{Agent, AgentOptions};

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

async fn start_on(dir: &std::path::Path, port: u16) -> Agent {
    let mut config = default_config();
    config.bind = "127.0.0.1".to_string();
    config.collectors.sys.enabled = false;
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
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

#[tokio::test]
async fn binding_succeeds_when_the_port_is_freed_within_the_time() {
    let holder = StdListener::bind("127.0.0.1:0").unwrap();
    let addr = holder.local_addr().unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        drop(holder);
    });
    let l = bind_retrying(addr, Duration::from_secs(10), Duration::from_millis(100)).await.expect("bound after the release");
    assert_eq!(l.local_addr().unwrap(), addr);
    release.join().unwrap();
}

#[tokio::test]
async fn binding_gives_up_after_the_time_with_addr_in_use() {
    let holder = StdListener::bind("127.0.0.1:0").unwrap();
    let addr: SocketAddr = holder.local_addr().unwrap();
    let started = Instant::now();
    let err = bind_retrying(addr, Duration::from_millis(600), Duration::from_millis(100)).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
    assert!(started.elapsed() >= Duration::from_millis(300) && started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn the_agent_listens_once_the_port_is_free_and_says_who_held_it_meanwhile() {
    let holder = StdListener::bind("127.0.0.1:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();
    let agent = start_on(dir.path(), port).await;

    // Busy: not listening, the status names the port (and the program when Windows says which).
    let status = agent.status();
    assert!(status.listening.is_none() && status.listen_error.is_some());
    assert_eq!(status.listen_busy.as_ref().map(|b| b.port), Some(port));
    #[cfg(windows)]
    assert!(status.listen_busy.unwrap().app.is_some_and(|a| a.to_lowercase().ends_with(".exe")));

    tokio::time::sleep(Duration::from_secs(1)).await;
    drop(holder);
    assert!(wait_until(10, || agent.status().listening.is_some()).await, "the agent listens after the port was freed");
    let status = agent.status();
    assert!(status.listen_error.is_none() && status.listen_busy.is_none());
    agent.shutdown().await;
}

#[tokio::test]
async fn try_again_binds_a_freed_port_without_restarting_the_agent() {
    let holder = StdListener::bind("127.0.0.1:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();
    let agent = start_on(dir.path(), port).await;
    assert!(agent.status().listening.is_none());
    drop(holder);
    // The "Try again" button of the Phone tab: `restart_server` on the same agent.
    let status = agent.restart_server().await;
    assert!(status.listening.is_some());
    agent.shutdown().await;
}
