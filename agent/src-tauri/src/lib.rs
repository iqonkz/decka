use std::sync::Arc;

use decka_core::actions::WindowsRunner;
use decka_core::{Agent, AgentOptions};
use tauri::path::BaseDirectory;
use tauri::Manager;

fn log_level() -> tracing::Level {
    match std::env::var("DECKA_LOG").unwrap_or_default().to_ascii_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    }
}

pub fn run() {
    let _ = tracing_subscriber::fmt().with_max_level(log_level()).try_init();

    tauri::Builder::default()
        .setup(|app| {
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
                    app.manage(agent);
                }
                Err(e) => tracing::error!("agent did not start: {e}"),
            }

            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Decka");
}
