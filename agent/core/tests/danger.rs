//! Dangerous system actions pressed on a phone (DATA_MODEL.md 11.4), with a recording runner: nothing is really shut down.

use std::sync::Arc;
use std::time::Duration;

use decka_core::actions::testing::RecordingRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::SwitchConfig;
use decka_core::proto::ErrorCode;
use decka_core::safety::PhoneCtx;
use decka_core::{Agent, AgentOptions};
use serde_json::json;

async fn start(dir: &std::path::Path, runner: Arc<RecordingRunner>) -> Agent {
    let mut config = default_config();
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    let actions = json!({ "version": 1, "actions": {
        "off": { "type": "system", "name": "Off", "op": "shutdown" },
        "lock": { "type": "system", "name": "Lock", "op": "lock" },
    } });
    std::fs::write(dir.join("actions.json"), actions.to_string()).unwrap();
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner, fixed_pair_code: None }).await.unwrap()
}

fn phone(seconds: u64) -> PhoneCtx {
    PhoneCtx { device_name: "Galaxy".to_string(), connected_for: Duration::from_secs(seconds) }
}

#[tokio::test]
async fn shutdown_from_a_phone_waits_for_the_connection_to_age_and_for_the_gap() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Arc::new(RecordingRunner::new());
    let agent = start(dir.path(), runner.clone()).await;

    // 10 s after connecting: refused, nothing runs.
    let err = agent.execute_for("off", Some(&phone(10))).await.unwrap_err();
    assert_eq!((err.0, err.1.as_str()), (ErrorCode::ActionFailed, "Too soon after connecting"));
    assert!(runner.calls().is_empty());

    // An old enough connection may do it once ...
    agent.execute_for("off", Some(&phone(120))).await.unwrap();
    assert_eq!(runner.calls(), ["off"]);
    // ... and a second one right after is refused.
    let err = agent.execute_for("off", Some(&phone(125))).await.unwrap_err();
    assert_eq!((err.0, err.1.as_str()), (ErrorCode::ActionFailed, "Wait a little before trying again"));
    assert_eq!(runner.calls(), ["off"]);

    // Harmless system actions and the PC itself are not limited.
    agent.execute_for("lock", Some(&phone(1))).await.unwrap();
    agent.execute("off").await.unwrap();
    assert_eq!(runner.calls(), ["off", "lock", "off"]);
    agent.shutdown().await;
}
