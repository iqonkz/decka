use std::net::IpAddr;
use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::agent::CcDenied;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::{Agent, AgentOptions, SaveResult};
use serde_json::Value;

async fn start(dir: &std::path::Path) -> Agent {
    let mut config = default_config();
    config.collectors.sys.interval_sec = 1;
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()), config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None })
        .await
        .unwrap()
}

#[tokio::test]
async fn hook_token_guards_cc_endpoints_and_stays_out_of_the_window() {
    let cfg = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let agent = start(cfg.path()).await;
    let local: IpAddr = "127.0.0.1".parse().unwrap();
    let lan: IpAddr = "192.168.1.5".parse().unwrap();

    // Not installed: there is no hook token, so every call is 401 and the state is not installed.
    assert_eq!(agent.cc_authorize(Some(local), Some("dk_x")), Err(CcDenied::Unauthorized));
    assert!(!agent.cc_status_in(Some(home.path())).installed);

    let status = agent.cc_install_in(home.path()).unwrap();
    assert!(status.installed && !status.status_line_conflict && !status.codex_found);
    let token = agent.config().ai.unwrap().hook_token.unwrap();
    assert!(token.starts_with("dk_"));

    assert_eq!(agent.cc_authorize(Some(local), Some(&token)), Ok(()));
    assert_eq!(agent.cc_authorize(Some(lan), Some(&token)), Err(CcDenied::NotFound), "other machines see no endpoint");
    assert_eq!(agent.cc_authorize(None, Some(&token)), Err(CcDenied::NotFound));
    let api_token = agent.config().api_token;
    assert_eq!(agent.cc_authorize(Some(local), Some(&api_token)), Err(CcDenied::Unauthorized), "the API token does not open the hooks");
    assert_eq!(agent.cc_authorize(Some(local), None), Err(CcDenied::Unauthorized));

    // The window never gets the token, and saving the window's copy keeps the stored one.
    let for_window = agent.config_for_window();
    assert!(for_window.ai.as_ref().unwrap().hook_token.is_none());
    assert_eq!(agent.save_config(for_window), SaveResult::Saved);
    assert_eq!(agent.config().ai.unwrap().hook_token.as_deref(), Some(token.as_str()));

    // Remove: our entries and the token are gone, the section is null.
    let status = agent.cc_uninstall_in(home.path()).unwrap();
    assert!(!status.installed);
    assert!(agent.config().ai.unwrap().hook_token.is_none());
    assert_eq!(agent.store().get("ai"), Value::Null);
    assert_eq!(agent.cc_authorize(Some(local), Some(&token)), Err(CcDenied::Unauthorized));
    agent.shutdown().await;
}

#[tokio::test]
async fn a_statusline_call_fills_the_section() {
    let cfg = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let agent = start(cfg.path()).await;
    agent.cc_install_in(home.path()).unwrap();
    let line = agent.cc_statusline(br#"{"session_id":"abcdef123456","cwd":"C:\\p\\decka","rate_limits":{"five_hour":{"used_percentage":23,"resets_at":4102444800}}}"#);
    assert_eq!(line, "Decka · 5h 23%");
    // The section goes out at most every 2 seconds: the loop of the agent sends it.
    let mut ai = agent.store().get("ai");
    for _ in 0..80 {
        if ai["sessions"][0]["project"] == "decka" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        ai = agent.store().get("ai");
    }
    assert_eq!(ai["claude"]["windows"][0]["label"], "5h");
    assert_eq!(ai["sessions"][0]["project"], "decka");
    // No phone is connected: the permission question goes to the terminal at once.
    assert_eq!(agent.cc_permission(br#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#, false).await, None);
    assert_eq!(agent.cc_permission(br#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#, true).await, None);
    agent.shutdown().await;
}

#[tokio::test]
async fn codex_gets_its_own_hook_next_to_foreign_ones_and_it_goes_away_again() {
    let cfg = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    // Codex is "found" when its sessions folder exists; it already has a hook of its own.
    std::fs::create_dir_all(home.path().join(".codex").join("sessions")).unwrap();
    let hooks_path = home.path().join(".codex").join("hooks.json");
    let foreign = serde_json::json!({ "hooks": { "PermissionRequest": [ { "matcher": "Bash", "hooks": [ { "type": "command", "command": "mine.exe" } ] } ] } });
    std::fs::write(&hooks_path, serde_json::to_string(&foreign).unwrap()).unwrap();
    let agent = start(cfg.path()).await;

    let status = agent.cc_install_in(home.path()).unwrap();
    assert!(status.codex_found && status.codex_hook);
    let after: Value = serde_json::from_str(&std::fs::read_to_string(&hooks_path).unwrap()).unwrap();
    let list = after["hooks"]["PermissionRequest"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0], foreign["hooks"]["PermissionRequest"][0]);
    let command = list[1]["hooks"][0]["command"].as_str().unwrap();
    assert!(command.contains("/api/cc/permission?agent=codex") && command.contains("curl.exe") && command.contains("Bearer dk_"), "{command}");
    assert_eq!(list[1]["hooks"][0]["type"], "command");
    assert!(hooks_path.with_file_name("hooks.json.decka-bak").exists());

    let status = agent.cc_uninstall_in(home.path()).unwrap();
    assert!(!status.codex_hook);
    let removed: Value = serde_json::from_str(&std::fs::read_to_string(&hooks_path).unwrap()).unwrap();
    assert_eq!(removed, foreign);
    agent.shutdown().await;
}

#[tokio::test]
async fn a_hooks_file_made_only_for_decka_is_removed_with_it() {
    let cfg = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codex").join("sessions")).unwrap();
    let agent = start(cfg.path()).await;
    agent.cc_install_in(home.path()).unwrap();
    assert!(home.path().join(".codex").join("hooks.json").exists());
    agent.cc_uninstall_in(home.path()).unwrap();
    assert!(!home.path().join(".codex").join("hooks.json").exists());
    agent.shutdown().await;
}
