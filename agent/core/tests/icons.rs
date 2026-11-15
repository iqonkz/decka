//! M12: `img:` icons through the agent: rule 11 on save, and the `icons` section follows the layout.

use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::config::{default_actions, default_config, default_layout, save_json_atomic};
use decka_core::icons::{icons_dir, save_icon};
use decka_core::model::{LayoutItem, Page};
use decka_core::{Agent, AgentOptions, SaveResult};

async fn start(dir: &std::path::Path) -> Agent {
    let mut config = default_config();
    config.collectors.sys.enabled = false;
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    save_json_atomic(&dir.join("layout.json"), &default_layout()).unwrap();
    save_json_atomic(&dir.join("actions.json"), &default_actions()).unwrap();
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None })
        .await
        .unwrap()
}

fn set_first_button_icon(agent: &Agent, icon: &str) -> decka_core::model::LayoutFile {
    let mut layout = agent.layout();
    let page: &mut Page = &mut layout.pages[0];
    for item in &mut page.items {
        if let LayoutItem::Button(b) = item {
            b.icon = Some(icon.to_string());
            break;
        }
    }
    layout
}

#[tokio::test]
async fn a_missing_icon_file_is_rejected_with_a_clear_message() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    let layout = set_first_button_icon(&agent, "img:0123456789abcdef");
    match agent.save_layout(layout) {
        SaveResult::Rejected(issues) => {
            assert!(issues.iter().any(|i| i.message == "icon file not found — choose the icon again" && i.path.ends_with("/icon")), "{issues:?}");
        }
        SaveResult::Saved => panic!("expected a rejection"),
    }
    agent.shutdown().await;
}

#[tokio::test]
async fn the_icons_section_follows_the_layout() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    assert_eq!(agent.store().get("icons"), serde_json::json!({}));

    let icon = save_icon(dir.path(), b"\x89PNG pretend icon").unwrap();
    let hash = icon.strip_prefix("img:").unwrap().to_string();
    assert_eq!(agent.save_layout(set_first_button_icon(&agent, &icon)), SaveResult::Saved);
    let section = agent.store().get("icons");
    assert!(section[&hash].as_str().unwrap().starts_with("data:image/png;base64,"), "{section}");
    assert_eq!(agent.icon_preview(&icon), section[&hash].as_str().map(str::to_string));

    // The icon is no longer used: the section empties, the file stays (DATA_MODEL.md 8.1).
    assert_eq!(agent.save_layout(set_first_button_icon(&agent, "lucide:play")), SaveResult::Saved);
    assert_eq!(agent.store().get("icons"), serde_json::json!({}));
    assert!(icons_dir(dir.path()).join(format!("{hash}.png")).exists());
    agent.shutdown().await;
}

#[tokio::test]
async fn first_run_until_the_layout_changes() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path()).await;
    assert!(agent.is_first_run(), "a fresh install is a first run");
    let layout = set_first_button_icon(&agent, "lucide:star");
    assert_eq!(agent.save_layout(layout), SaveResult::Saved);
    assert!(!agent.is_first_run(), "a changed layout is not");
    agent.shutdown().await;
}

#[tokio::test]
async fn the_section_is_filled_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let icon = save_icon(dir.path(), b"\x89PNG start icon").unwrap();
    let hash = icon.strip_prefix("img:").unwrap().to_string();
    let mut config = default_config();
    config.collectors.sys.enabled = false;
    save_json_atomic(&dir.path().join("config.json"), &config).unwrap();
    let mut layout = default_layout();
    let button = layout.pages[0].items.iter_mut().find_map(|i| if let LayoutItem::Button(b) = i { Some(b) } else { None }).expect("a button");
    button.icon = Some(icon);
    save_json_atomic(&dir.path().join("layout.json"), &layout).unwrap();
    save_json_atomic(&dir.path().join("actions.json"), &default_actions()).unwrap();
    let agent = Agent::start(AgentOptions { config_dir: Some(dir.path().to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None })
        .await
        .unwrap();
    assert!(agent.store().get("icons").get(&hash).is_some());
    agent.shutdown().await;
}
