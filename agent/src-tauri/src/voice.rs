//! Wiring of the voice package (M21): builds the `VoiceService` from decka-core and forwards every change to the
//! settings window as the event `voice-status`. The IPC commands live in `commands.rs`.

use std::path::Path;
use std::sync::Arc;

use decka_core::ai::download::{parse_manifest, Downloader, ManifestEntry};
use decka_core::ai::service::{default_root, DetectProbe, VoiceDeps, VoiceService};
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

pub fn setup(app: &AppHandle, config_dir: &Path) {
    let tauri::async_runtime::RuntimeHandle::Tokio(handle) = tauri::async_runtime::handle();
    let window = app.clone();
    let deps = VoiceDeps {
        root: default_root(),
        settings_path: config_dir.join("voice.json"),
        manifest: load_manifest(app),
        hardware: Arc::new(DetectProbe),
        // The microphone list comes with the capture code (C2); until then the window shows only the default device.
        mics: Arc::new(Vec::new),
        downloader: Downloader::default(),
        emit: Arc::new(move |status| {
            let _ = window.emit("voice-status", status);
        }),
    };
    match VoiceService::new(handle, deps) {
        Ok(service) => {
            app.manage(service);
        }
        Err(e) => tracing::error!("voice control did not start: {e}"),
    }
}
