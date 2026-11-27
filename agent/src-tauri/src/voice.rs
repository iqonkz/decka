//! Wiring of the voice package (M21): builds the `VoiceService` and the `Host` (pipeline, microphone, hotkey) from decka-core,
//! forwards every change of the status to the settings window as the event `voice-status`, and owns the window of the pill
//! (created when a request of the PC starts, destroyed when it is idle again; DATA_MODEL.md 12.6). The IPC commands live in
//! `commands.rs`.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use decka_core::ai::download::{parse_manifest, Downloader, ManifestEntry};
use decka_core::ai::service::{default_root, DetectProbe, VoiceDeps, VoiceService};
use decka_core::voice::host::{Host, PillCmd};
use decka_core::Agent;
use tauri::{AppHandle, Emitter, Manager};

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

/// The voice host shows a request of the PC: the island draws it (T26.4); `idle` takes it away.
fn pill(app: &AppHandle, cmd: PillCmd) {
    match cmd {
        PillCmd::Show(payload) => crate::island::on_voice(app, Some(payload)),
        PillCmd::Destroy => crate::island::on_voice(app, None),
    }
}
pub fn setup(app: &AppHandle, agent: &Agent) {
    let tauri::async_runtime::RuntimeHandle::Tokio(handle) = tauri::async_runtime::handle();
    let window = app.clone();
    let host_cell: Arc<OnceLock<Host>> = Arc::new(OnceLock::new());
    let cell = host_cell.clone();
    let screen_help = agent.clone();
    let deps = VoiceDeps {
        screen_help: Arc::new(move || screen_help.voice_context_enabled()),
        root: default_root(),
        settings_path: agent.config_dir().join("voice.json"),
        manifest: load_manifest(app),
        hardware: Arc::new(DetectProbe),
        mics: Arc::new(decka_core::voice::mic::list_mics),
        downloader: Downloader::default(),
        layers: Arc::new(agent.clone()),
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
    // T32.1: the settings window and the dot in the tray follow the wake word.
    let window = app.clone();
    host.wake().listen(Arc::new(move |status| {
        let _ = window.emit("wake-status", status.clone());
        crate::tray::show_wake(&window, status.enabled && status.installed);
    }));
    app.manage(service);
    app.manage(host);
}
