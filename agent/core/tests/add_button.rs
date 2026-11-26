// T29.6: the files of a button made by voice go through the checked save and are what a phone gets.
use std::sync::Arc;

use decka_core::actions::testing::RecordingRunner;
use decka_core::model::LayoutItem;
use decka_core::voice::addbutton;
use decka_core::{Agent, AgentOptions, SaveResult};

#[tokio::test(flavor = "multi_thread")]
async fn a_built_button_is_saved_into_both_files() {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::start(AgentOptions { config_dir: Some(dir.path().to_path_buf()), port_override: Some(0), bind_override: Some("127.0.0.1".into()), client_dist: None, runner: Arc::new(RecordingRunner::default()), fixed_pair_code: None }).await.unwrap();
    let mut layout = agent.layout();
    layout.pages[0].items.clear();
    assert_eq!(agent.save_layout(layout), SaveResult::Saved);
    let page = agent.layout().pages[0].id.clone();

    let (actions, layout) = addbutton::build("ru", "win.lock", &page, &agent.actions(), &agent.layout()).unwrap();
    assert_eq!(agent.save_actions(actions), SaveResult::Saved);
    assert_eq!(agent.save_layout(layout), SaveResult::Saved);

    let on_disk = std::fs::read_to_string(dir.path().join("layout.json")).unwrap();
    assert!(on_disk.contains("v-win-lock"), "{on_disk}");
    let LayoutItem::Button(b) = &agent.layout().pages[0].items[0] else { panic!("a button") };
    assert!(agent.actions().actions.contains_key(b.action_id.as_ref().unwrap()));
    agent.shutdown().await;
}