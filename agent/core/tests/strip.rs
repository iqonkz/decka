//! The app strip (DATA_MODEL.md 10.5): `tpl-*` keys run only while the strip of their template is the one shown.

use std::sync::Arc;

use decka_core::actions::testing::RecordingRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::SwitchConfig;
use decka_core::proto::ErrorCode;
use decka_core::{Agent, AgentOptions};

async fn start(dir: &std::path::Path, runner: Arc<RecordingRunner>) -> Agent {
    let mut config = default_config();
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()), config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner, fixed_pair_code: None }).await.unwrap()
}

#[tokio::test]
async fn a_strip_key_runs_only_for_the_strip_shown() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Arc::new(RecordingRunner::new());
    let agent = start(dir.path(), runner.clone()).await;

    // No program in front: no strip, the template action is not known.
    assert_eq!(agent.execute("tpl-word-bold").await.unwrap_err().0, ErrorCode::UnknownAction);

    // Word in front: its keys run, the other template's keys do not.
    agent.set_focus_for_test(Some("winword.exe"));
    assert_eq!(agent.store().get("strip")["template"], "word");
    assert!(agent.execute("tpl-word-bold").await.is_ok());
    assert_eq!(runner.calls(), ["tpl-word-bold"]);
    assert_eq!(agent.execute("tpl-excel-bold").await.unwrap_err().0, ErrorCode::UnknownAction);

    // Another program in front: Word's key is unknown again, and nothing was written to actions.json.
    agent.set_focus_for_test(Some("notepad.exe"));
    assert!(agent.store().get("strip").is_null());
    assert_eq!(agent.execute("tpl-word-bold").await.unwrap_err().0, ErrorCode::UnknownAction);
    assert!(!agent.actions().actions.contains_key("tpl-word-bold"));
    assert_eq!(runner.calls().len(), 1);
}
