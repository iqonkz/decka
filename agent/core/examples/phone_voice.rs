//! T32.H.3: the agent with voice control for a phone, doing nothing for real: an action is only printed (`DRY <id>
//! <type>`). For the automatic check of the whole chain of a phone request.
//!
//!   $env:DECKA_CONFIG_DIR = "<empty folder>"      # required: the settings and the log of this copy
//!   $env:DECKA_VOICE_WAV = "<any 16 kHz wav>"     # stands in for the microphone of the PC, should it be asked for
//!   cargo run -p decka-core --example phone_voice -- --ai-root <folder of the voice package> --port 19252 --pair-code ABCD2345 --lang ru
//!   node tools/ws-smoke.mjs pair  127.0.0.1:19252 ABCD2345
//!   node tools/ws-smoke.mjs voice 127.0.0.1:19252 <token> say.wav
//!
//! Stop it with Ctrl+C or by its process id; its helpers end with it.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use decka_core::actions::DryRunner;
use decka_core::ai::download::{parse_manifest, Downloader};
use decka_core::ai::service::{DetectProbe, NoLayerMemory, VoiceDeps, VoiceService, VoiceSetArgs};
use decka_core::voice::dictation::{DictationSettings, Trigger};
use decka_core::voice::host::Host;
use decka_core::{Agent, AgentOptions};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let config = PathBuf::from(std::env::var_os("DECKA_CONFIG_DIR").expect("set DECKA_CONFIG_DIR to an empty folder"));
    let root = PathBuf::from(get("--ai-root").expect("--ai-root <folder of the voice package>"));
    let agent = Agent::start(AgentOptions {
        // Loopback only: no question of the firewall about this copy.
        bind_override: "127.0.0.1".parse().ok(),
        config_dir: Some(config.clone()),
        client_dist: None,
        port_override: Some(get("--port").and_then(|p| p.parse().ok()).unwrap_or(19252)),
        runner: Arc::new(DryRunner),
        fixed_pair_code: get("--pair-code"),
    })
    .await
    .expect("the agent starts");
    let mut layout = agent.layout();
    layout.display.language = get("--lang").or(layout.display.language);
    agent.save_layout(layout);

    let manifest = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/ai/manifest.json")).expect("manifest");
    let cell: Arc<OnceLock<Host>> = Arc::new(OnceLock::new());
    let listener = cell.clone();
    let handle = tokio::runtime::Handle::current();
    let service = VoiceService::new(
        handle.clone(),
        VoiceDeps {
            root,
            settings_path: config.join("voice.json"),
            manifest: parse_manifest(&manifest).expect("the manifest parses"),
            hardware: Arc::new(DetectProbe),
            mics: Arc::new(Vec::new),
            downloader: Downloader::default(),
            layers: Arc::new(NoLayerMemory),
            emit: Arc::new(move |status| {
                if let Some(host) = listener.get() {
                    host.on_status(&status);
                }
            }),
            screen_help: Arc::new(|| false),
        },
    )
    .expect("voice control starts");
    // Keys nobody presses by chance: this copy is driven by the phone only.
    let keys = ["ctrl", "alt", "shift", "f12"].map(String::from).to_vec();
    service.set(VoiceSetArgs { hotkey: Some(keys), ..Default::default() }).expect("settings");
    let host = Host::new(handle, agent.clone(), service.clone(), Arc::new(|_| {}));
    let _ = cell.set(host.clone());
    host.set_dictation_settings(DictationSettings { trigger: Trigger::Off, ..host.dictation_settings() }).expect("dictation off");
    host.on_status(&service.status());
    println!("READY {:?} {:?}", service.status().state, agent.status().listening.map(|l| l.port));

    let _ = tokio::signal::ctrl_c().await;
    host.stop_helpers().await;
    agent.shutdown().await;
}
