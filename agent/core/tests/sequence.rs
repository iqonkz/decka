//! `sequence` actions (DATA_MODEL.md 2.3): steps in order, pauses, stop on the first error, "Already running", `toggleVar` at the end.

use std::sync::Arc;
use std::time::Duration;

use decka_core::actions::testing::RecordingRunner;
use decka_core::config::{default_actions, default_config, save_json_atomic};
use decka_core::model::{Action, SequenceStep, SwitchConfig};
use decka_core::proto::ErrorCode;
use decka_core::{Agent, AgentOptions};
use serde_json::json;

fn run(id: &str) -> SequenceStep {
    SequenceStep::Run { action_id: id.to_string() }
}

fn wait(ms: i64) -> SequenceStep {
    SequenceStep::Wait { delay_ms: ms }
}

fn sequence(steps: Vec<SequenceStep>, toggle: Option<&str>) -> Action {
    Action::Sequence { name: "Seq".into(), toggle_var: toggle.map(str::to_string), steps }
}

async fn start(runner: Arc<RecordingRunner>, seq: Action) -> (Agent, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut actions = default_actions();
    actions.actions.insert("seq".into(), seq);
    save_json_atomic(&dir.path().join("actions.json"), &actions).unwrap();
    // The collectors would publish variables and sections of their own.
    let mut config = default_config();
    config.collectors.audio = Some(SwitchConfig { enabled: false });
    config.collectors.media = Some(SwitchConfig { enabled: false });
    config.collectors.focus = Some(SwitchConfig { enabled: false });
    save_json_atomic(&dir.path().join("config.json"), &config).unwrap();
    let agent = Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()), config_dir: Some(dir.path().to_path_buf()), client_dist: None, port_override: Some(0), runner, fixed_pair_code: None })
        .await
        .unwrap();
    (agent, dir)
}

#[tokio::test]
async fn steps_run_in_order_with_their_pauses() {
    let runner = Arc::new(RecordingRunner::new());
    let (agent, _dir) = start(runner.clone(), sequence(vec![run("media-play"), wait(200), run("media-next"), run("media-prev")], None)).await;

    let begun = std::time::Instant::now();
    agent.execute("seq").await.unwrap();
    let total = begun.elapsed();

    assert_eq!(runner.calls(), ["media-play", "media-next", "media-prev"]);
    let calls = runner.timed_calls();
    let gap = calls[1].1.duration_since(calls[0].1);
    // The pause is 200 ms, give or take the scheduler (±50 ms below, a little more above: CI machines are slow).
    assert!(gap >= Duration::from_millis(150) && gap < Duration::from_millis(600), "pause was {gap:?}");
    // No pause between the last two steps.
    assert!(calls[2].1.duration_since(calls[1].1) < Duration::from_millis(150));
    assert!(total >= Duration::from_millis(190), "finished in {total:?}");
    agent.shutdown().await;
}

#[tokio::test]
async fn the_first_error_stops_the_sequence_and_names_the_step() {
    let runner = Arc::new(RecordingRunner::failing_on("media-next", "boom"));
    let (agent, _dir) = start(runner.clone(), sequence(vec![run("media-play"), wait(50), run("media-next"), run("media-prev")], Some("test.flag"))).await;

    let err = agent.execute("seq").await.unwrap_err();
    assert_eq!(err, (ErrorCode::ActionFailed, "Step 3: boom".to_string()));
    assert_eq!(runner.calls(), ["media-play"], "nothing runs after the failed step");
    // `toggleVar` is for a sequence that went all the way.
    assert_eq!(agent.store().get("vars"), json!({}));
    agent.shutdown().await;
}

#[tokio::test]
async fn toggle_var_flips_after_all_steps() {
    let runner = Arc::new(RecordingRunner::new());
    let (agent, _dir) = start(runner.clone(), sequence(vec![run("media-play"), run("media-next")], Some("test.flag"))).await;
    agent.execute("seq").await.unwrap();
    assert_eq!(agent.store().get("vars"), json!({ "test.flag": true }));
    agent.execute("seq").await.unwrap();
    assert_eq!(agent.store().get("vars"), json!({ "test.flag": false }));
    agent.shutdown().await;
}

#[tokio::test]
async fn a_second_press_while_it_runs_is_refused_and_the_first_finishes() {
    let runner = Arc::new(RecordingRunner::slow(Duration::from_millis(250)));
    let (agent, _dir) = start(runner.clone(), sequence(vec![run("media-play"), run("media-next")], None)).await;

    let first = {
        let agent = agent.clone();
        tokio::spawn(async move { agent.execute("seq").await })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    let err = agent.execute("seq").await.unwrap_err();
    assert_eq!(err, (ErrorCode::ActionFailed, "Already running".to_string()));

    first.await.unwrap().unwrap();
    assert_eq!(runner.calls(), ["media-play", "media-next"], "the refused press ran nothing");
    // Once it is over the key works again.
    agent.execute("seq").await.unwrap();
    assert_eq!(runner.calls().len(), 4);
    agent.shutdown().await;
}

#[tokio::test]
async fn a_failed_run_frees_the_key() {
    let runner = Arc::new(RecordingRunner::failing_on("media-play", "nope"));
    let (agent, _dir) = start(runner, sequence(vec![run("media-play")], None)).await;
    for _ in 0..2 {
        let err = agent.execute("seq").await.unwrap_err();
        assert_eq!(err, (ErrorCode::ActionFailed, "Step 1: nope".to_string()), "not 'Already running'");
    }
    agent.shutdown().await;
}
