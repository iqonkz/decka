//! The USB loop against a fake `adb` (a .cmd script), so no phone or Android SDK is needed.
#![cfg(windows)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::{Agent, AgentOptions};

async fn wait_until(secs: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    done()
}

async fn start(dir: &std::path::Path, adb: &str) -> Agent {
    let mut config = default_config();
    config.collectors.sys.enabled = false;
    config.usb.enabled = true;
    config.usb.adb_path = adb.to_string();
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

#[tokio::test]
async fn a_device_gets_reversed_and_an_unauthorized_one_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("adb.log");
    let script = dir.path().join("fake-adb.cmd");
    std::fs::write(
        &script,
        format!(
            "@echo off\r\necho %* >> \"{log}\"\r\nif \"%1\"==\"devices\" (\r\n echo List of devices attached\r\n echo AAA	device\r\n echo BBB	unauthorized\r\n)\r\nexit /b 0\r\n",
            log = log.display()
        ),
    )
    .unwrap();

    let agent = start(dir.path(), script.to_str().unwrap()).await;
    let port = agent.status().listening.unwrap().port;
    assert!(wait_until(8, || agent.status().usb.devices.iter().any(|d| d.serial == "AAA" && d.reversed)).await, "{:?}", agent.status().usb);
    let usb = agent.status().usb;
    assert!(usb.enabled && usb.adb_found);
    assert!(usb.devices.iter().any(|d| d.serial == "BBB" && !d.reversed));
    assert_eq!(usb.last_error.as_deref(), Some("device BBB is unauthorized: accept the USB debugging prompt on the phone"));

    let calls = std::fs::read_to_string(&log).unwrap();
    assert!(calls.contains(&format!("-s AAA reverse tcp:{port} tcp:{port}")), "{calls}");
    assert!(!calls.contains("-s BBB"), "{calls}");
    agent.shutdown().await;
}

#[tokio::test]
async fn a_missing_adb_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), "definitely-not-adb.exe").await;
    assert!(wait_until(5, || agent.status().usb.last_error.is_some()).await);
    let usb = agent.status().usb;
    assert!(usb.enabled && !usb.adb_found && usb.devices.is_empty());
    agent.shutdown().await;
}
