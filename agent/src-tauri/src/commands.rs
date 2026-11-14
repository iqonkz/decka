//! IPC commands of the settings window (PROTOCOL.md 9): names and argument/result shapes mirror
//! `IpcCommands` in shared/src/ipc.ts. They are reachable only from the window, never over the network.

use decka_core::model::{ActionsFile, ConfigFile, LayoutFile};
use decka_core::proto::NotifyReq;
use decka_core::agent::{AgentStatus, DeviceInfo, PairingInfo, TestActionResult};
use decka_core::model::NotifLevel;
use decka_core::{Agent, SaveResult};
use serde::Serialize;
use tauri::{AppHandle, Manager};

fn agent(app: &AppHandle) -> Result<Agent, String> {
    app.try_state::<Agent>().map(|s| s.inner().clone()).ok_or_else(|| "the Decka agent did not start".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResult {
    api_token: String,
}

#[tauri::command]
pub fn get_status(app: AppHandle) -> Result<AgentStatus, String> {
    Ok(agent(&app)?.status())
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<ConfigFile, String> {
    Ok(agent(&app)?.config())
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: ConfigFile) -> Result<SaveResult, String> {
    let autostart = config.autostart;
    let result = agent(&app)?.save_config(config);
    if result == SaveResult::Saved {
        crate::sync_autostart(&app, autostart);
    }
    Ok(result)
}

#[tauri::command]
pub async fn restart_server(app: AppHandle) -> Result<AgentStatus, String> {
    Ok(agent(&app)?.restart_server().await)
}

#[tauri::command]
pub fn regenerate_api_token(app: AppHandle) -> Result<TokenResult, String> {
    agent(&app)?.regenerate_api_token().map(|api_token| TokenResult { api_token })
}

#[tauri::command]
pub fn get_actions(app: AppHandle) -> Result<ActionsFile, String> {
    Ok(agent(&app)?.actions())
}

#[tauri::command]
pub fn save_actions(app: AppHandle, actions: ActionsFile) -> Result<SaveResult, String> {
    Ok(agent(&app)?.save_actions(actions))
}

#[tauri::command]
pub fn get_layout(app: AppHandle) -> Result<LayoutFile, String> {
    Ok(agent(&app)?.layout())
}

#[tauri::command]
pub fn save_layout(app: AppHandle, layout: LayoutFile) -> Result<SaveResult, String> {
    Ok(agent(&app)?.save_layout(layout))
}

#[tauri::command]
pub async fn test_action(app: AppHandle, action_id: String) -> Result<TestActionResult, String> {
    Ok(agent(&app)?.test_action(&action_id).await)
}

#[tauri::command]
pub fn start_pairing(app: AppHandle) -> Result<PairingInfo, String> {
    Ok(agent(&app)?.start_pairing())
}

#[tauri::command]
pub fn cancel_pairing(app: AppHandle) -> Result<(), String> {
    agent(&app)?.cancel_pairing();
    Ok(())
}

#[tauri::command]
pub fn list_devices(app: AppHandle) -> Result<Vec<DeviceInfo>, String> {
    Ok(agent(&app)?.list_devices())
}

#[tauri::command]
pub fn revoke_device(app: AppHandle, device_id: String) -> Result<(), String> {
    agent(&app)?.revoke_device(&device_id);
    Ok(())
}

#[tauri::command]
pub fn send_test_notification(app: AppHandle) -> Result<(), String> {
    agent(&app)?.push_notification(NotifyReq {
        title: "Decka test".to_string(),
        body: None,
        level: Some(NotifLevel::Info),
        source: Some("decka".to_string()),
    });
    Ok(())
}

/// Exe names of programs that currently have a visible window (suggestions for a page's apps).
#[tauri::command]
pub fn list_window_apps() -> Vec<String> {
    decka_core::collectors::focus::list_window_apps()
}

/// The built-in context page templates (DATA_MODEL.md 7).
#[tauri::command]
pub fn get_templates(app: AppHandle) -> Result<Vec<decka_core::templates::PageTemplate>, String> {
    Ok(agent(&app)?.get_templates())
}

/// Adds a template's actions and page and saves both files.
#[tauri::command]
pub fn add_template(app: AppHandle, template_id: String) -> Result<SaveResult, String> {
    Ok(agent(&app)?.add_template(&template_id))
}

#[tauri::command]
pub fn open_config_dir(app: AppHandle) -> Result<(), String> {
    let agent = agent(&app)?;
    std::process::Command::new("explorer.exe").arg(agent.config_dir()).spawn().map(drop).map_err(|e| e.to_string())
}
