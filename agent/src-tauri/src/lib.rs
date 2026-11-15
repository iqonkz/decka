mod commands;
mod tray;

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

    tauri::Builder::default()
        // Registered first: a second launch just brings the first window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_settings(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            // Closing the window only hides it; the app lives in the tray until "Quit".
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_config,
            commands::save_config,
            commands::restart_server,
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
            commands::send_test_notification,
            commands::open_config_dir,
            commands::open_logs_dir,
            commands::icon_from_program,
            commands::icon_from_image,
            commands::get_icon_preview,
            commands::icon_from_svg,
            commands::is_first_run,
            commands::list_installed_apps,
            commands::list_window_apps,
            commands::get_templates,
            commands::add_template,
        ])
        .setup(|app| {
            let minimized = std::env::args().any(|a| a == "--minimized");
            let first_run = !decka_core::paths::config_dir(None).join("config.json").exists();

            // The bundled web client; without it the server still runs and `/` answers "client not bundled".
            let client_dist = app.path().resolve("client", BaseDirectory::Resource).ok().filter(|p| p.exists());
            let opts = AgentOptions {
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
        .run(tauri::generate_context!())
        .expect("error while running Decka");
}
