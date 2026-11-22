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

/// The answer of the last update check (ARCHITECTURE.md 8a).
#[tauri::command]
pub fn get_update_status(app: AppHandle) -> Result<decka_core::updates::UpdateStatus, String> {
    Ok(agent(&app)?.update_status())
}

/// `Check now`: asks GitHub at once and returns the answer.
#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Result<decka_core::updates::UpdateStatus, String> {
    Ok(agent(&app)?.check_updates().await)
}

/// The place search of `More в†’ Integrations в†’ Weather`: a list of places, or `{ error }`.
#[tauri::command]
pub async fn search_place(app: AppHandle, query: String) -> Result<serde_json::Value, String> {
    Ok(match agent(&app)?.search_place(&query).await {
        Ok(places) => serde_json::to_value(places).map_err(|e| e.to_string())?,
        Err(error) => serde_json::json!({ "error": error }),
    })
}

/// `Refresh`: asks Open-Meteo now; the weather, or `{ error }`.
#[tauri::command]
pub async fn refresh_weather(app: AppHandle) -> Result<serde_json::Value, String> {
    Ok(match agent(&app)?.refresh_weather().await {
        Ok(info) => serde_json::to_value(info).map_err(|e| e.to_string())?,
        Err(error) => serde_json::json!({ "error": error }),
    })
}

/// `More в†’ Integrations в†’ OBS Studio`: connection state, scenes and audio inputs.
#[tauri::command]
pub async fn obs_status(app: AppHandle) -> Result<decka_core::agent::ObsStatusInfo, String> {
    Ok(agent(&app)?.obs_status().await)
}

/// `Set passwordвЂ¦`: DPAPI-encrypted and saved; an empty password removes it.
#[tauri::command]
pub fn set_obs_password(app: AppHandle, password: String) -> Result<(), String> {
    agent(&app)?.set_obs_password(&password)
}

/// `More в†’ Integrations в†’ Mail`: the password of one mailbox, DPAPI-encrypted and saved; an empty password removes it.
#[tauri::command]
pub fn set_mail_password(app: AppHandle, account_id: String, password: String) -> Result<(), String> {
    agent(&app)?.set_mail_password(&account_id, &password)
}

/// `More в†’ Integrations в†’ Windows notifications`: the switch, the access Windows gives and the apps seen so far.
#[tauri::command]
pub async fn windows_notifications_status(app: AppHandle) -> Result<decka_core::agent::WinNotifStatus, String> {
    let agent = agent(&app)?;
    // The question to Windows is a WinRT call: it is asked off the UI thread.
    tauri::async_runtime::spawn_blocking(move || agent.windows_notifications_status()).await.map_err(|e| e.to_string())
}

/// `Phone` page: the kind of network the phones use (the question to Windows is asked off the UI thread).
#[tauri::command]
pub async fn network_status(app: AppHandle) -> Result<decka_core::agent::NetworkStatus, String> {
    let agent = agent(&app)?;
    tauri::async_runtime::spawn_blocking(move || agent.network_status()).await.map_err(|e| e.to_string())
}

/// `Check now`: looks at the mailboxes at once.
#[tauri::command]
pub fn mail_check_now(app: AppHandle) -> Result<(), String> {
    agent(&app)?.mail_check_now();
    Ok(())
}

/// `More в†’ Integrations в†’ AI coding`: what Claude Code and Codex look like from here.
#[tauri::command]
pub fn cc_status(app: AppHandle) -> Result<decka_core::agent::CcStatus, String> {
    Ok(agent(&app)?.cc_status())
}

/// `Install`: our status line and permission hook in the Claude Code settings (DATA_MODEL.md 6.8).
#[tauri::command]
pub fn cc_install(app: AppHandle) -> Result<decka_core::agent::CcStatus, String> {
    agent(&app)?.cc_install()
}

/// `Remove`: takes back only what `Install` wrote.
#[tauri::command]
pub fn cc_uninstall(app: AppHandle) -> Result<decka_core::agent::CcStatus, String> {
    agent(&app)?.cc_uninstall()
}

/// `More в†’ Integrations в†’ Twitch`: the sign-in and what the agent sees.
#[tauri::command]
pub fn twitch_status(app: AppHandle) -> Result<decka_core::agent::TwitchStatus, String> {
    Ok(agent(&app)?.twitch_status())
}

/// `Sign in`: the code to type on twitch.tv/activate.
#[tauri::command]
pub async fn twitch_login_start(app: AppHandle) -> Result<decka_core::twitch::TwitchLogin, String> {
    agent(&app)?.twitch_login_start().await
}

/// `Sign out`: the tokens are deleted.
#[tauri::command]
pub fn twitch_logout(app: AppHandle) -> Result<(), String> {
    agent(&app)?.twitch_logout()
}

/// `More в†’ Integrations в†’ YouTube`: the sign-in and what the agent sees.
#[tauri::command]
pub fn youtube_status(app: AppHandle) -> Result<decka_core::agent::YoutubeStatus, String> {
    Ok(agent(&app)?.youtube_status())
}

/// `Sign in`: the code to type on google.com/device.
#[tauri::command]
pub async fn youtube_login_start(app: AppHandle) -> Result<decka_core::youtube::YoutubeLogin, String> {
    agent(&app)?.youtube_login_start().await
}

/// `Sign out`: the tokens are deleted.
#[tauri::command]
pub fn youtube_logout(app: AppHandle) -> Result<(), String> {
    agent(&app)?.youtube_logout()
}

/// `Set secretвЂ¦`: the client secret of the Google application, encrypted with DPAPI; empty removes it.
#[tauri::command]
pub fn set_youtube_secret(app: AppHandle, secret: String) -> Result<(), String> {
    agent(&app)?.set_youtube_secret(&secret)
}

/// `Download`: opens a release page of the project in the browser (other addresses are refused).
#[tauri::command]
pub fn open_release(app: AppHandle, url: String) -> Result<(), String> {
    agent(&app)?.open_release(&url)
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<ConfigFile, String> {
    Ok(agent(&app)?.config_for_window())
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

/// Forgets one rule of "Always allow" (`id` given) or all of them.
#[tauri::command]
pub fn ai_remove_rule(app: AppHandle, id: Option<String>) -> Result<(), String> {
    agent(&app)?.ai_remove_rules(id.as_deref());
    Ok(())
}

/// The connection checks of the Phone tab.
#[tauri::command]
pub async fn diagnose_connection(app: AppHandle) -> Result<Vec<decka_core::diag::DiagCheck>, String> {
    Ok(agent(&app)?.diagnose().await)
}

/// Adds the Windows Firewall rule for Decka (Windows asks for administrator rights).
#[tauri::command]
pub async fn fix_firewall() -> Result<(), String> {
    decka_core::diag::fix_firewall().await
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
    let result = agent(&app)?.save_layout(layout);
    // The tray menu follows `display.language`.
    if matches!(result, SaveResult::Saved) {
        crate::tray::refresh_language(&app);
    }
    Ok(result)
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

/// `Can answer Claude Code` of one phone (DATA_MODEL.md 11.3).
#[tauri::command]
pub fn set_can_approve(app: AppHandle, device_id: String, allowed: bool) -> Result<bool, String> {
    Ok(agent(&app)?.set_can_approve(&device_id, allowed))
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

/// Sharpens the icons of program buttons that have a 96 px picture (M14); runs off the UI thread.
#[tauri::command]
pub async fn refresh_program_icons(app: AppHandle) -> Result<decka_core::agent::RefreshIcons, String> {
    let agent = agent(&app)?;
    tauri::async_runtime::spawn_blocking(move || agent.refresh_program_icons()).await.map_err(|e| e.to_string())
}

/// How many program buttons have a low-resolution icon (M14).
#[tauri::command]
pub fn count_lowres_icons(app: AppHandle) -> Result<usize, String> {
    Ok(agent(&app)?.count_lowres_icons())
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

// ---- M21: voice control (shared/src/ipc.ts `voice_*`; the event `voice-status` carries every change) ----

fn voice(app: &AppHandle) -> Result<decka_core::ai::service::VoiceService, String> {
    app.try_state::<decka_core::ai::service::VoiceService>().map(|s| s.inner().clone()).ok_or_else(|| "voice control did not start".to_string())
}

#[tauri::command]
pub fn voice_status(app: AppHandle) -> Result<decka_core::ai::service::VoiceStatus, String> {
    Ok(voice(&app)?.status())
}

/// `Turn on voice control` (download) or `Retry` (servers gave up).
#[tauri::command]
pub fn voice_enable(app: AppHandle) -> Result<decka_core::ai::service::VoiceStatus, String> {
    Ok(voice(&app)?.enable())
}

#[tauri::command]
pub fn voice_cancel(app: AppHandle) -> Result<decka_core::ai::service::VoiceStatus, String> {
    Ok(voice(&app)?.cancel())
}

#[tauri::command]
pub async fn voice_remove(app: AppHandle) -> Result<decka_core::ai::service::VoiceStatus, String> {
    Ok(voice(&app)?.remove().await)
}

#[tauri::command]
pub fn voice_set(
    app: AppHandle,
    model: Option<decka_core::ai::service::ModelChoice>,
    hotkey: Option<Vec<String>>,
    keep_loaded: Option<bool>,
    speak: Option<bool>,
    phone_button: Option<bool>,
    mic: Option<String>,
) -> Result<decka_core::ai::service::VoiceStatus, String> {
    // Flat camelCase keys like every other command (shared/src/ipc.ts `VoiceSetArgs`); tools/check-ipc.mjs guards the names.
    voice(&app)?.set(decka_core::ai::service::VoiceSetArgs { model, hotkey, keep_loaded, speak, phone_button, mic })
}

/// The Yes/No of the voice pill (oice_confirm of shared/src/ipc.ts); an unknown id is ignored by the pipeline.
#[tauri::command]
pub async fn voice_confirm(app: AppHandle, id: String, yes: bool) -> Result<(), String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    host.confirm(&id, yes);
    Ok(())
}

/// The click on the stop button (or the pill) while it listens: the recording ends and the command runs (DATA_MODEL.md 12.6a).
#[tauri::command]
pub async fn voice_stop(app: AppHandle) -> Result<(), String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    host.stop();
    Ok(())
}
