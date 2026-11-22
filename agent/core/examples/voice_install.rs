//! Installs the AI package of voice control (the same code as the button `Turn on voice control`) without the window:
//! the files go to `%LOCALAPPDATA%\Decka\ai`, the settings of this run to the folder given by `--settings`.
//!
//!   cargo run --release -p decka-core --example voice_install -- --model standard|light --settings <dir>
//!
//! Used once at a release to run `voice_eval` against the real models.

use std::sync::Arc;
use std::time::Duration;

use decka_core::ai::download::{parse_manifest, Downloader};
use decka_core::ai::service::{default_root, DetectProbe, ModelChoice, VoiceDeps, VoiceService, VoiceSetArgs, VoiceState};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let model = match get("--model").as_deref() {
        Some("light") => ModelChoice::Light,
        _ => ModelChoice::Standard,
    };
    let settings = std::path::PathBuf::from(get("--settings").expect("--settings <dir>"));
    std::fs::create_dir_all(&settings).expect("settings dir");
    let manifest = parse_manifest(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/ai/manifest.json")).expect("manifest")).expect("manifest parses");
    let service = VoiceService::new(
        tokio::runtime::Handle::current(),
        VoiceDeps {
            root: default_root(),
            settings_path: settings.join("voice.json"),
            manifest,
            hardware: Arc::new(DetectProbe),
            mics: Arc::new(Vec::new),
            downloader: Downloader::default(),
            emit: Arc::new(|_| {}),
        },
    )
    .expect("service");
    service.set(VoiceSetArgs { model: Some(model), ..Default::default() }).expect("model");
    let mut status = service.enable();
    let mut shown = 0u64;
    while matches!(status.state, VoiceState::Downloading) {
        tokio::time::sleep(Duration::from_secs(5)).await;
        status = service.status();
        if let Some(p) = &status.progress {
            let pct = p.done * 100 / p.total.max(1);
            if pct >= shown + 5 {
                shown = pct;
                println!("{pct}% ({:.2} of {:.2} GB)", p.done as f64 / 1e9, p.total as f64 / 1e9);
            }
        }
    }
    println!("state: {:?}, model: {:?}, error: {:?}", status.state, status.model, status.error);
    std::process::exit(if matches!(status.state, VoiceState::Ready) { 0 } else { 1 });
}
