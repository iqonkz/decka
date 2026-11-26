//! IPC commands of the settings window (PROTOCOL.md 9): names and argument/result shapes mirror
//! `IpcCommands` in shared/src/ipc.ts. They are reachable only from the window, never over the network.

use decka_core::model::{ActionsFile, ConfigFile, LayoutFile};
use decka_core::proto::NotifyReq;
use decka_core::agent::{AgentStatus, DeviceInfo, PairingInfo, TestActionResult};
use decka_core::model::NotifLevel;
use decka_core::{Agent, SaveResult};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

fn agent(app: &AppHandle) -> Result<Agent, String> {
    app.try_state::<Agent>().map(|s| s.inner().clone()).ok_or_else(|| "the Decka agent did not start".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResult {
    api_token: String,
}

/// The switch of the lyrics of the current song (DATA_MODEL.md 14).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsStatus {
    enabled: bool,
    pc_overlay: &'static str,
}

fn lyrics_status(agent: &Agent) -> LyricsStatus {
    LyricsStatus { enabled: agent.lyrics_enabled(), pc_overlay: agent.lyrics_overlay_mode().as_str() }
}

/// `Lyrics` card: is the lookup on (off by default; on sends the artist, the title and the length of a song to lrclib.net)
/// and the mode of the window of the words on the PC.
#[tauri::command]
pub fn lyrics_get(app: AppHandle) -> Result<LyricsStatus, String> {
    Ok(lyrics_status(&agent(&app)?))
}

/// Switches the lyrics on or off; off clears them from the phones at once.
#[tauri::command]
pub fn lyrics_set(app: AppHandle, enabled: bool) -> Result<LyricsStatus, String> {
    let agent = agent(&app)?;
    agent.set_lyrics_enabled(enabled)?;
    Ok(lyrics_status(&agent))
}

/// The mode of the window of the words on the PC: `off`, `auto` or `always` (anything else is refused).
#[tauri::command]
pub fn lyrics_overlay_set(app: AppHandle, mode: String) -> Result<LyricsStatus, String> {
    let parsed = match mode.as_str() {
        "off" | "auto" | "always" => decka_core::lyrics_overlay::PcOverlay::parse(Some(&mode)),
        _ => return Err("unknown mode".into()),
    };
    let agent = agent(&app)?;
    agent.set_lyrics_overlay_mode(parsed)?;
    Ok(lyrics_status(&agent))
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

/// The place search of `More → Integrations → Weather`: a list of places, or `{ error }`.
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

/// `More → Integrations → OBS Studio`: connection state, scenes and audio inputs.
#[tauri::command]
pub async fn obs_status(app: AppHandle) -> Result<decka_core::agent::ObsStatusInfo, String> {
    Ok(agent(&app)?.obs_status().await)
}

/// `Set password…`: DPAPI-encrypted and saved; an empty password removes it.
#[tauri::command]
pub fn set_obs_password(app: AppHandle, password: String) -> Result<(), String> {
    agent(&app)?.set_obs_password(&password)
}

/// `More → Integrations → Mail`: the password of one mailbox, DPAPI-encrypted and saved; an empty password removes it.
#[tauri::command]
pub fn set_mail_password(app: AppHandle, account_id: String, password: String) -> Result<(), String> {
    agent(&app)?.set_mail_password(&account_id, &password)
}

/// `More → Integrations → Windows notifications`: the switch, the access Windows gives and the apps seen so far.
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

/// M22: every list with all its items (the `list` widget picks one of them by id).
#[tauri::command]
pub fn lists_get(app: AppHandle) -> Result<serde_json::Value, String> {
    Ok(agent(&app)?.assistant().lists_json())
}

/// One reminder as the Reminders window gets it.
fn reminder_json(r: &decka_core::assistant::Reminder) -> serde_json::Value {
    let mut v = serde_json::json!({ "id": r.id, "text": r.text, "at": r.at, "done": r.done });
    if let Some(repeat) = r.repeat {
        v["repeat"] = repeat.as_str().into();
    }
    v
}

/// M22 (DATA_MODEL.md 13.7): every reminder, finished ones too.
#[tauri::command]
pub fn reminders_get(app: AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::Value::Array(agent(&app)?.assistant().reminders_all().iter().map(reminder_json).collect()))
}

/// Adds a reminder (no `id`) or changes the text and the time of one; only ids the agent issued are accepted.
#[tauri::command]
/// `repeat`: `once` | `daily` | `weekdays` | `weekly`; absent keeps the series of an existing reminder (a new one is once).
pub fn reminder_upsert(app: AppHandle, id: Option<String>, text: String, at: String, repeat: Option<String>) -> Result<serde_json::Value, String> {
    use decka_core::assistant::Repeat;
    let repeat = match repeat.as_deref() {
        None => None,
        Some("once") => Some(None),
        Some(other) => Some(Some(Repeat::parse(other).ok_or("The repeat must be once, daily, weekdays or weekly")?)),
    };
    let r = agent(&app)?.assistant().reminder_upsert(id.as_deref(), &text, &at, repeat, &decka_core::assistant::Local::now())?;
    Ok(reminder_json(&r))
}

#[tauri::command]
pub fn reminder_delete(app: AppHandle, id: String) -> Result<(), String> {
    if agent(&app)?.assistant().reminder_delete(&id) {
        Ok(())
    } else {
        Err("There is no such reminder".into())
    }
}

/// One item of `list_upsert`: `id` is absent for a new item.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListItemArg {
    pub id: Option<String>,
    pub text: String,
    pub done: bool,
}

/// Creates a list (no `id`) or replaces the name and the items of one.
#[tauri::command]
pub fn list_upsert(app: AppHandle, id: Option<String>, name: String, items: Vec<ListItemArg>) -> Result<serde_json::Value, String> {
    let items: Vec<(Option<String>, String, bool)> = items.into_iter().map(|i| (i.id, i.text, i.done)).collect();
    let l = agent(&app)?.assistant().list_upsert(id.as_deref(), &name, &items)?;
    Ok(serde_json::json!({ "id": l.id, "name": l.name, "items": l.items.iter().map(|i| serde_json::json!({ "id": i.id, "text": i.text, "done": i.done })).collect::<Vec<_>>() }))
}

#[tauri::command]
pub fn list_delete(app: AppHandle, id: String) -> Result<(), String> {
    if agent(&app)?.assistant().list_delete_id(&id) {
        Ok(())
    } else {
        Err("There is no such list".into())
    }
}

/// The facts of `More -> Voice -> Memory`.
#[tauri::command]
pub fn memory_get(app: AppHandle) -> Result<Vec<decka_core::voice::memory::Fact>, String> {
    Ok(agent(&app)?.memory().all())
}

#[tauri::command]
pub fn memory_update(app: AppHandle, id: String, text: String) -> Result<decka_core::voice::memory::Fact, String> {
    agent(&app)?.memory().update(&id, &text)
}

#[tauri::command]
pub fn memory_delete(app: AppHandle, id: String) -> Result<(), String> {
    if agent(&app)?.memory().remove_id(&id) {
        Ok(())
    } else {
        Err("I do not remember that".into())
    }
}

/// `Forget all`.
#[tauri::command]
pub fn memory_clear(app: AppHandle) -> Result<(), String> {
    agent(&app)?.memory().clear()
}

/// The settings of dictation (`dictation.json`).
#[tauri::command]
pub fn voice_dictation_get(app: AppHandle) -> Result<decka_core::voice::dictation::DictationSettings, String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    Ok(host.dictation_settings())
}

/// Saves the settings of dictation; an invalid shortcut is an error string. The answer is what was saved (the dictionary is tidied).
#[tauri::command]
pub fn voice_dictation_set(
    app: AppHandle,
    trigger: decka_core::voice::dictation::Trigger,
    mode: decka_core::voice::dictation::Mode,
    dictionary: Vec<String>,
) -> Result<decka_core::voice::dictation::DictationSettings, String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    host.set_dictation_settings(decka_core::voice::dictation::DictationSettings { trigger, mode, dictionary })?;
    Ok(host.dictation_settings())
}

/// `Check now`: looks at the mailboxes at once.
#[tauri::command]
pub fn mail_check_now(app: AppHandle) -> Result<(), String> {
    agent(&app)?.mail_check_now();
    Ok(())
}

/// `More → Integrations → AI coding`: what Claude Code and Codex look like from here.
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

/// `More → Integrations → Twitch`: the sign-in and what the agent sees.
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

/// `More → Integrations → YouTube`: the sign-in and what the agent sees.
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

/// `Set secret…`: the client secret of the Google application, encrypted with DPAPI; empty removes it.
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
    let dir = decka_core::logging::logs_dir(agent(&app)?.config_dir());
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

/// `Download again` of a damaged model file (T28.2).
#[tauri::command]
pub async fn voice_redownload(app: AppHandle) -> Result<decka_core::ai::service::VoiceStatus, String> {
    voice(&app)?.redownload().await
}

/// `Check voice` (T28.3): the four steps; every change also goes to the window as the event `voice-check`. A dry run.
#[tauri::command]
pub async fn voice_self_check(app: AppHandle) -> Result<Vec<decka_core::voice::check::CheckStep>, String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?.inner().clone();
    let window = app.clone();
    host.self_check(std::sync::Arc::new(move |steps: &[decka_core::voice::check::CheckStep]| {
        let _ = window.emit("voice-check", steps.to_vec());
    }))
    .await
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

/// `voice.context.enabled` (T26.7): hints by the active program when the island opens.
#[tauri::command]
pub fn voice_context_get(app: AppHandle) -> Result<bool, String> {
    Ok(agent(&app)?.voice_context_enabled())
}

#[tauri::command]
pub fn voice_context_set(app: AppHandle, enabled: bool) -> Result<bool, String> {
    agent(&app)?.set_voice_context_enabled(enabled)
}

/// `config.island` (T29.5): how the island rests while a song plays.
#[tauri::command]
pub fn island_config_get(app: AppHandle) -> Result<decka_core::island::Settings, String> {
    Ok(agent(&app)?.island_settings())
}

#[tauri::command]
pub fn island_config_set(app: AppHandle, resting_mode: Option<String>, bar_height: Option<u32>, bar_position: Option<String>) -> Result<decka_core::island::Settings, String> {
    agent(&app)?.set_island_settings(decka_core::model::IslandConfig { resting_mode, bar_height, bar_position })
}

/// `voice.duckMusic` (T29.3).
#[tauri::command]
pub fn voice_duck_get(app: AppHandle) -> Result<bool, String> {
    Ok(agent(&app)?.voice_duck_enabled())
}

#[tauri::command]
pub fn voice_duck_set(app: AppHandle, enabled: bool) -> Result<bool, String> {
    agent(&app)?.set_voice_duck_enabled(enabled)
}

/// A tap on a hint chip of the island (T26.7): `translate`, `explain`, `reply` or `error`; the PC decides what it means.
#[tauri::command]
pub async fn voice_chip(app: AppHandle, command: String) -> Result<(), String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    host.chip(&command)
}

/// The visible panel of the island (T26.4): the rest of its window lets clicks through. CSS pixels from the top-left of the window.
#[tauri::command]
pub fn island_rect(app: AppHandle, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let finite = [x, y, width, height].iter().all(|v| v.is_finite());
    if !finite || width < 0.0 || height < 0.0 || width > 4000.0 || height > 4000.0 {
        return Err("bad rectangle".into());
    }
    if let Some(shared) = app.try_state::<std::sync::Arc<crate::island::Shared>>() {
        let rect = crate::island::Rect { x, y, width, height };
        shared.set_rect(rect);
        crate::island::fit(&app, rect);
    }
    Ok(())
}

/// `voice.notices.enabled` (T27.8).
#[tauri::command]
pub fn voice_notices_get(app: AppHandle) -> Result<bool, String> {
    Ok(agent(&app)?.voice_notices_enabled())
}

#[tauri::command]
pub fn voice_notices_set(app: AppHandle, enabled: bool) -> Result<bool, String> {
    agent(&app)?.set_voice_notices_enabled(enabled)
}

/// A button of the guide card in the island (T27.5): `next`, `back`, `stop`, `done`.
#[tauri::command]
pub fn guide_control(app: AppHandle, kind: String) -> Result<(), String> {
    use decka_core::voice::tools::GuideCtl;
    let kind = match kind.as_str() {
        "next" => GuideCtl::Next,
        "back" => GuideCtl::Back,
        "stop" => GuideCtl::Stop,
        "done" => GuideCtl::Done,
        _ => return Err("unknown guide control".into()),
    };
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    host.screen().ok_or_else(|| "The screen help is not ready".to_string())?.control(kind).map(|_| ())
}

/// The saved guides (T27.6): the files of `%APPDATA%/Decka/guides`.
#[tauri::command]
pub fn guides_list(app: AppHandle) -> Result<Vec<decka_core::voice::guides::Recipe>, String> {
    Ok(decka_core::voice::guides::Guides::new(agent(&app)?.config_dir()).list())
}

#[tauri::command]
pub fn guides_rename(app: AppHandle, id: String, goal: String) -> Result<(), String> {
    decka_core::voice::guides::Guides::new(agent(&app)?.config_dir()).rename(&id, &goal)
}

#[tauri::command]
pub fn guides_delete(app: AppHandle, id: String) -> Result<(), String> {
    decka_core::voice::guides::Guides::new(agent(&app)?.config_dir()).delete(&id)
}

/// The work modes (T27.10): `%APPDATA%/Decka/modes.json`.
#[tauri::command]
pub fn modes_list(app: AppHandle) -> Result<Vec<decka_core::voice::modes::Mode>, String> {
    Ok(decka_core::voice::modes::load(agent(&app)?.config_dir()))
}

#[tauri::command]
pub fn modes_save(app: AppHandle, mode: decka_core::voice::modes::Mode, original_name: Option<String>) -> Result<(), String> {
    decka_core::voice::modes::put(agent(&app)?.config_dir(), mode, original_name.as_deref())
}

#[tauri::command]
pub fn modes_delete(app: AppHandle, name: String) -> Result<(), String> {
    decka_core::voice::modes::delete(agent(&app)?.config_dir(), &name)
}

/// The model of the assistant (T27.7): light, standard or the extended 9B one. One model at a time; the server restarts.
#[tauri::command]
pub fn voice_set_model_tier(app: AppHandle, model: decka_core::ai::service::ModelChoice) -> Result<decka_core::ai::service::ModelChoice, String> {
    let service = app.try_state::<decka_core::ai::service::VoiceService>().ok_or_else(|| "voice control did not start".to_string())?;
    service.set(decka_core::ai::service::VoiceSetArgs { model: Some(model), ..Default::default() })?;
    Ok(model)
}

/// A media button of the island (T26.F.10): `prev`, `play_pause`, `next`, `mute`. The same operations as the deck buttons; any other
/// name is refused.
#[tauri::command]
pub async fn media_control(app: AppHandle, kind: String) -> Result<(), String> {
    let op = decka_core::island::media_op(&kind).ok_or_else(|| "unknown media button".to_string())?;
    let runner = agent(&app)?.runner().clone();
    let action = decka_core::model::Action::Media { name: "island".into(), toggle_var: None, op };
    tokio::task::spawn_blocking(move || runner.run("island", &action)).await.map_err(|e| format!("The action did not finish: {e}"))??;
    Ok(())
}

/// A button under the shown answer (T26.3): `copy`, `insert`, `speak`, `again`, `lang:<code>`. The window only names the action;
/// what it does is decided by the PC, and a name outside this list is refused.
#[tauri::command]
pub async fn voice_action(app: AppHandle, kind: String) -> Result<(), String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    host.action(&kind)
}

/// The click on the stop button (or the pill) while it listens: the recording ends and the command runs (DATA_MODEL.md 12.6a).
#[tauri::command]
pub async fn voice_stop(app: AppHandle) -> Result<(), String> {
    let host = app.try_state::<decka_core::voice::host::Host>().ok_or_else(|| "voice control did not start".to_string())?;
    host.stop();
    Ok(())
}
