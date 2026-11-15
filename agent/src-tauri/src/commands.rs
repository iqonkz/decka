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

/// `{ icon: "img:<hash16>" }` or `{ error }`, as in PROTOCOL.md 9.
#[derive(Serialize)]
#[serde(untagged)]
pub enum IconResult {
    Icon { icon: String },
    Error { error: String },
}

impl From<Result<String, String>> for IconResult {
    fn from(r: Result<String, String>) -> Self {
        match r {
            Ok(icon) => IconResult::Icon { icon },
            Err(error) => IconResult::Error { error },
        }
    }
}

/// Icon of a program (exe) from the Shell; runs off the UI thread.
#[tauri::command]
pub async fn icon_from_program(app: AppHandle, path: String) -> Result<IconResult, String> {
    let agent = agent(&app)?;
    tauri::async_runtime::spawn_blocking(move || agent.icon_from_program(&path).into()).await.map_err(|e| e.to_string())
}

/// Icon from a picture file (png, jpg, ico, bmp); runs off the UI thread.
#[tauri::command]
pub async fn icon_from_image(app: AppHandle, path: String) -> Result<IconResult, String> {
    let agent = agent(&app)?;
    tauri::async_runtime::spawn_blocking(move || agent.icon_from_image(&path).into()).await.map_err(|e| e.to_string())
}

/// True while nothing is paired and the layout is untouched: the settings window shows the first-run guide.
#[tauri::command]
pub fn is_first_run(app: AppHandle) -> Result<bool, String> {
    Ok(agent(&app)?.is_first_run())
}

/// Stores an SVG from the icon library as `img:<hash16>` (DATA_MODEL.md 8.4); `{ error: "Unsupported SVG" }` otherwise.
#[tauri::command]
pub fn icon_from_svg(app: AppHandle, svg: String) -> Result<IconResult, String> {
    Ok(decka_core::icons::icon_from_svg(agent(&app)?.config_dir(), &svg).into())
}

/// Start menu programs for the picker; takes seconds, so it runs off the UI thread.
#[tauri::command]
pub async fn list_installed_apps() -> Result<Vec<decka_core::installed::InstalledApp>, String> {
    tauri::async_runtime::spawn_blocking(decka_core::installed::list_installed_apps).await.map_err(|e| e.to_string())
}

/// One `.exe` or `.lnk` (dropped from Explorer or chosen with the dialog) as a program: name, target, arguments, icon (M14).
#[tauri::command]
pub async fn describe_program(path: String) -> Result<DescribeResult, String> {
    tauri::async_runtime::spawn_blocking(move || decka_core::installed::describe_program(&path).map(DescribeResult::App).unwrap_or_else(|error| DescribeResult::Error { error }))
        .await
        .map_err(|e| e.to_string())
}

/// `InstalledApp` or `{ error }`, as in PROTOCOL.md 9.
#[derive(Serialize)]
#[serde(untagged)]
pub enum DescribeResult {
    App(decka_core::installed::InstalledApp),
    Error { error: String },
}

/// Data URL of a stored `img:` icon (settings-window preview); `null` for other kinds or a missing file.
#[tauri::command]
pub fn get_icon_preview(app: AppHandle, icon: String) -> Result<Option<String>, String> {
    Ok(agent(&app)?.icon_preview(&icon))
}

/// Opens `<configDir>\logs` in Explorer (created when missing).
#[tauri::command]
pub fn open_logs_dir(app: AppHandle) -> Result<(), String> {
    let dir = decka_core::logging::logs_dir(&agent(&app)?.config_dir());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe").arg(dir).spawn().map(drop).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_config_dir(app: AppHandle) -> Result<(), String> {
    let agent = agent(&app)?;
    std::process::Command::new("explorer.exe").arg(agent.config_dir()).spawn().map(drop).map_err(|e| e.to_string())
}
