use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::config::{default_actions, default_config, default_layout, save_json_atomic};
use decka_core::model::{Action, ActionsFile, LayoutFile, LayoutItem, Page};
use decka_core::{Agent, AgentOptions, SaveResult};

async fn start(dir: &std::path::Path, edit: impl FnOnce(&mut LayoutFile, &mut ActionsFile)) -> Agent {
    let mut config = default_config();
    config.collectors.sys.enabled = false;
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    let (mut layout, mut actions) = (default_layout(), default_actions());
    edit(&mut layout, &mut actions);
    save_json_atomic(&dir.join("layout.json"), &layout).unwrap();
    save_json_atomic(&dir.join("actions.json"), &actions).unwrap();
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

fn rejected(result: SaveResult) -> Vec<(String, String)> {
    match result {
        SaveResult::Rejected(issues) => issues.into_iter().map(|i| (i.path, i.message)).collect(),
        SaveResult::Saved => panic!("expected a rejection"),
    }
}

#[tokio::test]
async fn each_template_is_added_on_the_default_7x4_grid() {
    for id in ["vscode", "browser", "zoom", "spotify"] {
        let dir = tempfile::tempdir().unwrap();
        let agent = start(dir.path(), |_, _| {}).await;
        let pages_before = agent.layout().pages.len();
        assert_eq!(agent.add_template(id), SaveResult::Saved, "{id}");

        let layout = agent.layout();
        assert_eq!(layout.pages.len(), pages_before + 1);
        let page = layout.pages.last().unwrap();
        assert_eq!(page.id, id);
        assert!(page.apps.as_ref().is_some_and(|a| !a.is_empty()));
        assert!(agent.actions().actions.keys().any(|k| k.starts_with(&format!("tpl-{id}-"))), "{id}: actions");

        // Both files and the layout section the phones get are up to date.
        let on_disk: LayoutFile = serde_json::from_str(&std::fs::read_to_string(dir.path().join("layout.json")).unwrap()).unwrap();
        assert_eq!(on_disk.pages.len(), pages_before + 1);
        let actions_on_disk: ActionsFile = serde_json::from_str(&std::fs::read_to_string(dir.path().join("actions.json")).unwrap()).unwrap();
        assert!(actions_on_disk.actions.keys().any(|k| k.starts_with(&format!("tpl-{id}-"))));
        assert_eq!(agent.store().get("layout")["pages"].as_array().unwrap().len(), pages_before + 1);
        agent.shutdown().await;
    }
}

#[tokio::test]
async fn home_is_replaced_by_the_first_page_id_and_item_ids_are_prefixed() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), |_, _| {}).await;
    assert_eq!(agent.add_template("vscode"), SaveResult::Saved);
    let layout = agent.layout();
    let home = layout.pages[0].id.clone();
    let page = layout.pages.last().unwrap();
    let home_button = page.items.iter().find_map(|i| match i {
        LayoutItem::Button(b) if b.id == "t-vscode-home" => Some(b),
        _ => None,
    });
    assert_eq!(home_button.expect("home button").navigate.as_deref(), Some(home.as_str()));
    assert!(page.items.iter().all(|i| i.rect().0.starts_with("t-vscode-")));
    agent.shutdown().await;
}

#[tokio::test]
async fn adding_vscode_twice_is_an_error_about_code_exe() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), |_, _| {}).await;
    assert_eq!(agent.add_template("vscode"), SaveResult::Saved);
    let issues = rejected(agent.add_template("vscode"));
    assert_eq!(issues.len(), 1);
    assert!(issues[0].0.ends_with("/apps"), "{issues:?}");
    assert_eq!(issues[0].1, "code.exe is already used by page VS Code");
    assert_eq!(agent.layout().pages.iter().filter(|p| p.id.starts_with("vscode")).count(), 1);
    agent.shutdown().await;
}

#[tokio::test]
async fn a_5x3_grid_is_too_small() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), |layout, _| {
        layout.grid.cols = 5;
        layout.grid.rows = 3;
        layout.pages = vec![Page { id: "main".into(), name: "Main".into(), items: Vec::new(), apps: None }];
    })
    .await;
    let issues = rejected(agent.add_template("browser"));
    assert_eq!(issues, [("/grid".to_string(), "Template needs a 7×4 grid or larger".to_string())]);
    agent.shutdown().await;
}

#[tokio::test]
async fn ten_pages_is_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), |layout, _| {
        layout.pages = (0..10).map(|n| Page { id: format!("p{n}"), name: format!("P{n}"), items: Vec::new(), apps: None }).collect();
    })
    .await;
    let issues = rejected(agent.add_template("zoom"));
    assert_eq!(issues, [("/pages".to_string(), "Page limit reached".to_string())]);
    agent.shutdown().await;
}

#[tokio::test]
async fn an_unknown_template_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), |_, _| {}).await;
    assert_eq!(rejected(agent.add_template("nope")), [("/templates".to_string(), "unknown template".to_string())]);
    agent.shutdown().await;
}

#[tokio::test]
async fn an_existing_action_with_the_same_id_is_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let mine = Action::Hotkey { name: "My save".into(), toggle_var: None, keys: vec!["ctrl".into(), "alt".into(), "s".into()] };
    let agent = start(dir.path(), |_, actions| {
        actions.actions.insert("tpl-vscode-save".to_string(), mine.clone());
    })
    .await;
    assert_eq!(agent.add_template("vscode"), SaveResult::Saved);
    assert_eq!(agent.actions().actions["tpl-vscode-save"], mine);
    assert!(agent.actions().actions.contains_key("tpl-vscode-palette"));
    agent.shutdown().await;
}

#[tokio::test]
async fn a_taken_page_id_gets_a_numeric_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), |layout, _| {
        layout.pages.push(Page { id: "spotify".into(), name: "Mine".into(), items: Vec::new(), apps: None });
    })
    .await;
    assert_eq!(agent.add_template("spotify"), SaveResult::Saved);
    assert_eq!(agent.layout().pages.last().unwrap().id, "spotify-2");
    agent.shutdown().await;
}
