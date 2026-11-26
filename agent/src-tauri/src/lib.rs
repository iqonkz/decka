mod commands;
mod highlight;
mod island;
mod tray;
mod voice;

use std::sync::Arc;

use decka_core::actions::WindowsRunner;
use decka_core::{Agent, AgentOptions};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// Makes the OS autostart entry match `config.autostart` (only touches it when they differ).
pub fn sync_autostart(app: &AppHandle, wanted: bool) {
    let manager = app.autolaunch();
    let enabled = manager.is_enabled().unwrap_or(false);
    let result = if wanted && !enabled {
        manager.enable()
    } else if !wanted && enabled {
        manager.disable()
    } else {
        Ok(())
    };
    if let Err(e) = result {
        tracing::warn!("cannot update the autostart entry: {e}");
    }
}

pub fn run() {
    decka_core::logging::init_file_logging(&decka_core::paths::config_dir(None));

    let mut builder = tauri::Builder::default();
    // Registered first: a second launch just brings the first window forward. A test copy (`DECKA_CONFIG_DIR` set, a separate
    // configuration folder) does not take part, so that it can run next to the installed agent without taking its place.
    if std::env::var_os("DECKA_CONFIG_DIR").is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_settings(app)));
    }
    builder
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_dialog::init())
        // One-click update of the agent (T20.3): the plugin verifies the minisign signature against `plugins.updater.pubkey`.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .on_window_event(|window, event| {
            // Closing the window destroys it (its WebView2 processes go with it); the app lives in the tray until "Quit".
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.destroy();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_config,
            commands::save_config,
            commands::restart_server,
            commands::ai_remove_rule,
            commands::diagnose_connection,
            commands::fix_firewall,
            commands::regenerate_api_token,
            commands::get_actions,
            commands::save_actions,
            commands::get_layout,
            commands::save_layout,
            commands::test_action,
            commands::start_pairing,
            commands::cancel_pairing,
            commands::list_devices,
            commands::revoke_device,
            commands::set_can_approve,
            commands::send_test_notification,
            commands::get_update_status,
            commands::check_updates,
            commands::open_release,
            commands::search_place,
            commands::obs_status,
            commands::set_obs_password,
            commands::set_mail_password,
            commands::mail_check_now,
            commands::lists_get,
            commands::reminders_get,
            commands::reminder_upsert,
            commands::reminder_delete,
            commands::list_upsert,
            commands::list_delete,
            commands::memory_get,
            commands::memory_update,
            commands::memory_delete,
            commands::memory_clear,
            commands::voice_dictation_get,
            commands::voice_dictation_set,
            commands::lyrics_get,
            commands::lyrics_set,
        commands::lyrics_overlay_set,
            commands::windows_notifications_status,
            commands::network_status,
            commands::cc_status,
            commands::cc_install,
            commands::cc_uninstall,
            commands::twitch_status,
            commands::twitch_login_start,
            commands::twitch_logout,
            commands::youtube_status,
            commands::youtube_login_start,
            commands::youtube_logout,
            commands::set_youtube_secret,
            commands::refresh_weather,
            commands::open_config_dir,
            commands::open_logs_dir,
            commands::icon_from_program,
            commands::icon_from_image,
            commands::get_icon_preview,
            commands::icon_from_svg,
            commands::is_first_run,
            commands::list_installed_apps,
            commands::describe_program,
            commands::refresh_program_icons,
            commands::count_lowres_icons,
            commands::list_window_apps,
            commands::get_templates,
            commands::add_template,
            commands::voice_status,
            commands::voice_enable,
            commands::voice_cancel,
            commands::voice_remove,
            commands::voice_redownload,
            commands::voice_self_check,
            commands::voice_set,
            commands::voice_confirm,
            commands::voice_stop,
            commands::voice_action,
            commands::island_rect,
            commands::media_control,
            commands::voice_notices_get,
            commands::voice_notices_set,
            commands::guide_control,
            commands::guides_list,
            commands::guides_rename,
            commands::guides_delete,
            commands::modes_list,
            commands::modes_save,
            commands::modes_delete,
            commands::voice_set_model_tier,
            commands::voice_context_get,
            commands::voice_context_set,
            commands::island_config_get,
            commands::island_config_set,
            commands::voice_speak_reply,
            commands::voice_context_pack_status,
            commands::voice_context_pack_download,
            commands::voice_context_pack_remove,
            commands::voice_duck_get,
            commands::voice_duck_set,
            commands::voice_chip,
        ])
        .setup(|app| {
            let minimized = std::env::args().any(|a| a == "--minimized");
            let first_run = !decka_core::paths::config_dir(None).join("config.json").exists();

            // The bundled web client; without it the server still runs and `/` answers "client not bundled".
            let client_dist = app.path().resolve("client", BaseDirectory::Resource).ok().filter(|p| p.exists());
            let opts = AgentOptions {
        bind_override: None,
                config_dir: None,
                client_dist,
                port_override: None,
                runner: Arc::new(WindowsRunner::new()),
                fixed_pair_code: None,
            };
            // A failed start must not kill the app: the window opens and the problem is shown there.
            match tauri::async_runtime::block_on(Agent::start(opts)) {
                Ok(agent) => {
                    sync_autostart(app.handle(), agent.config().autostart);

                    // Forward every status change to the window as `decka://status-changed`.
                    let handle = app.handle().clone();
                    let mut status = agent.subscribe_status();
                    tauri::async_runtime::spawn(async move {
                        while status.changed().await.is_ok() {
                            let snapshot = status.borrow_and_update().clone();
                            let _ = handle.emit("decka://status-changed", snapshot);
                        }
                    });

                    // The daily check for a newer release; its answer goes to the window and to the tray.
                    let handle = app.handle().clone();
                    let mut updates = agent.subscribe_updates();
                    tauri::async_runtime::spawn(async move {
                        while updates.changed().await.is_ok() {
                            let snapshot = updates.borrow_and_update().clone();
                            let _ = handle.emit("decka://update-changed", snapshot.clone());
                            tray::show_update(&handle, snapshot.available.as_ref().map(|a| a.version.as_str()));
                        }
                    });
                    agent.start_update_loop();
                    voice::setup(app.handle(), &agent);
                    island::setup(app.handle(), &agent);
                    let highlighter = highlight::setup(app.handle());
                    if let Some(host) = app.try_state::<decka_core::voice::host::Host>() {
                        host.attach_screen(highlighter, std::sync::Arc::new(highlight::GuideEmitter(app.handle().clone())), Vec::new());
                        host.start_notices();
                        // T29.6: a button that asks for something is finished in the settings window.
                        let handle = app.handle().clone();
                        host.attach_settings(std::sync::Arc::new(move |target: &str| {
                            let (app, target) = (handle.clone(), target.to_string());
                            let _ = handle.run_on_main_thread(move || tray::show_settings_at(&app, &target));
                        }));
                    }

                    app.manage(agent);
                }
                Err(e) => tracing::error!("agent did not start: {e}"),
            }

            tray::setup(app.handle())?;

            // `--minimized` keeps the window hidden, except on the very first run.
            if !minimized || first_run {
                tray::show_settings(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Decka")
        .run(|app, event| {
            match event {
                // The last window going away must not end the app: it lives in the tray (`code` is set only by `exit`).
                tauri::RunEvent::ExitRequested { api, code, .. } => {
                    if code.is_none() {
                        api.prevent_exit();
                    }
                }
                // T29.3: sounds that were turned down come back before the process is gone.
                tauri::RunEvent::Exit => {
                    if let Some(host) = app.try_state::<decka_core::voice::host::Host>() {
                        host.quit();
                    }
                }
                _ => {}
            }
        });
}
