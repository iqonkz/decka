//! Wiring of the voice package (M21): builds the `VoiceService` and the `Host` (pipeline, microphone, hotkey) from decka-core,
//! forwards every change of the status to the settings window as the event `voice-status`, and owns the window of the pill
//! (created when a request of the PC starts, destroyed when it is idle again; DATA_MODEL.md 12.6). The IPC commands live in
//! `commands.rs`.

use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use decka_core::ai::download::{parse_manifest, Downloader, ManifestEntry};
use decka_core::ai::service::{default_root, DetectProbe, VoiceDeps, VoiceService};
use decka_core::voice::host::{Host, PillCmd};
use decka_core::Agent;
use serde_json::Value;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const PILL: &str = "voice-pill";
const PILL_WIDTH: f64 = 360.0;
/// 64 px for the pill, 160 px with the answer or the confirmation (DESIGN.md 4.12).
const PILL_HEIGHT: f64 = 64.0;
const PILL_HEIGHT_OPEN: f64 = 160.0;
/// From the top edge of the screen, in logical pixels.
const PILL_TOP: f64 = 24.0;

/// `shared/ai/manifest.json`: the bundled resource, or the repository copy when running from a checkout.
fn load_manifest(app: &AppHandle) -> Vec<ManifestEntry> {
    let bundled = app.path().resolve("ai/manifest.json", tauri::path::BaseDirectory::Resource).ok();
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("shared").join("ai").join("manifest.json");
    for path in bundled.into_iter().chain(std::iter::once(dev)) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        match parse_manifest(&text) {
            Ok(entries) => return entries,
            Err(e) => tracing::error!("voice manifest {} is invalid: {e}", path.display()),
        }
    }
    // Without the list `Turn on voice control` answers "The voice package list is incomplete".
    Vec::new()
}

/// The pill window: created on the monitor of the mouse cursor, centred at the top, never taking the focus.
fn create_pill(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, PILL, WebviewUrl::App("index.html?pill=1".into()))
        .title("Decka voice")
        .inner_size(PILL_WIDTH, PILL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|e| tracing::error!("cannot create the voice pill: {e}"))
        .ok()?;
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let scale = m.scale_factor();
        let x = m.position().x as f64 + (m.size().width as f64 - PILL_WIDTH * scale) / 2.0;
        let y = m.position().y as f64 + PILL_TOP * scale;
        let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
    let _ = window.show();
    Some(window)
}

fn show_pill(app: &AppHandle, payload: Value) {
    let (window, fresh) = match app.get_webview_window(PILL) {
        Some(w) => (w, false),
        None => match create_pill(app) {
            Some(w) => (w, true),
            None => return,
        },
    };
    let open = matches!(payload.get("state").and_then(Value::as_str), Some("confirm" | "done" | "error"));
    let _ = window.set_size(LogicalSize::new(PILL_WIDTH, if open { PILL_HEIGHT_OPEN } else { PILL_HEIGHT }));
    let _ = window.emit("voice-pill", payload.clone());
    if fresh {
        // The page needs a moment to start listening: the first states are repeated until it does.
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            for ms in [300u64, 700, 1500] {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                let Some(w) = app.get_webview_window(PILL) else { return };
                let _ = w.emit("voice-pill", payload.clone());
            }
        });
    }
}

fn pill(app: &AppHandle, cmd: PillCmd) {
    match cmd {
        PillCmd::Show(payload) => show_pill(app, payload),
        PillCmd::Destroy => {
            if let Some(w) = app.get_webview_window(PILL) {
                let _ = w.destroy();
            }
        }
    }
}

pub fn setup(app: &AppHandle, agent: &Agent) {
    let tauri::async_runtime::RuntimeHandle::Tokio(handle) = tauri::async_runtime::handle();
    let window = app.clone();
    let host_cell: Arc<OnceLock<Host>> = Arc::new(OnceLock::new());
    let cell = host_cell.clone();
    let deps = VoiceDeps {
        root: default_root(),
        settings_path: agent.config_dir().join("voice.json"),
        manifest: load_manifest(app),
        hardware: Arc::new(DetectProbe),
        mics: Arc::new(decka_core::voice::mic::list_mics),
        downloader: Downloader::default(),
        emit: Arc::new(move |status| {
            let _ = window.emit("voice-status", status.clone());
            if let Some(host) = cell.get() {
                host.on_status(&status);
            }
        }),
    };
    let service = match VoiceService::new(handle.clone(), deps) {
        Ok(service) => service,
        Err(e) => {
            tracing::error!("voice control did not start: {e}");
            return;
        }
    };
    let pill_app = app.clone();
    let host = Host::new(handle, agent.clone(), service.clone(), Arc::new(move |cmd| pill(&pill_app, cmd)));
    let _ = host_cell.set(host.clone());
    // Voice control may be on already (installed earlier): the hotkey and the phones need it from the start.
    host.on_status(&service.status());
    app.manage(service);
    app.manage(host);
}
