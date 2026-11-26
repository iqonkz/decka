// The M29 settings are written to config.json and come back after the agent restarts.
use std::sync::Arc;

use decka_core::actions::testing::RecordingRunner;
use decka_core::island::{Align, Rest};
use decka_core::model::IslandConfig;
use decka_core::{Agent, AgentOptions};

async fn start(dir: &std::path::Path) -> Agent {
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), port_override: Some(0), bind_override: Some("127.0.0.1".into()), client_dist: None, runner: Arc::new(RecordingRunner::default()), fixed_pair_code: None }).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn ducking_and_the_island_settings_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    assert!(agent.voice_duck_enabled(), "on by default");
    agent.set_voice_duck_enabled(false).unwrap();
    agent.set_island_settings(IslandConfig { resting_mode: Some("lyrics".into()), ..Default::default() }).unwrap();
    agent.set_island_settings(IslandConfig { bar_height: Some(14), bar_position: Some("right".into()), ..Default::default() }).unwrap();
    assert!(agent.set_island_settings(IslandConfig { bar_height: Some(40), ..Default::default() }).is_err());
    agent.shutdown().await;

    let again = start(dir.path()).await;
    assert!(!again.voice_duck_enabled());
    let island = again.island_settings();
    assert_eq!((island.resting_mode, island.bar_height, island.bar_position), (Rest::Lyrics, 14, Align::Right));
    again.shutdown().await;
}