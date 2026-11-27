//! A live meeting note on the real devices and models (TASKS.md T30.R): the same agent, voice service and host as the
//! application, with a configuration folder of its own. Records the microphone and the PC sound, prints numbers only -
//! never the words that were heard - and removes everything it wrote.
//!
//! `cargo run --release -p decka-core --example note_live -- --dir <empty folder> --seconds 120 --expect <word,word,...>`

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::ai::download::{parse_manifest, Downloader};
use decka_core::ai::service::{default_root, DetectProbe, VoiceDeps, VoiceService};
use decka_core::notes::Event;
use decka_core::voice::host::Host;
use decka_core::{Agent, AgentOptions};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let dir = std::path::PathBuf::from(get("--dir").expect("--dir <empty folder>"));
    let seconds: u64 = get("--seconds").and_then(|s| s.parse().ok()).unwrap_or(120);
    let expect: Vec<String> = get("--expect").map(|s| s.split(',').map(str::to_lowercase).collect()).unwrap_or_default();
    std::fs::create_dir_all(&dir).expect("the folder");
    let agent = Agent::start(AgentOptions { config_dir: Some(dir.clone()), client_dist: None, port_override: Some(19333), bind_override: Some("127.0.0.1".into()), runner: Arc::new(DryRunner), fixed_pair_code: None })
        .await
        .expect("the agent starts");
    let manifest = parse_manifest(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/ai/manifest.json")).expect("manifest")).expect("manifest parses");
    let deps = VoiceDeps { root: default_root(), settings_path: dir.join("voice.json"), manifest, hardware: Arc::new(DetectProbe), mics: Arc::new(decka_core::voice::mic::list_mics), downloader: Downloader::default(), layers: Arc::new(agent.clone()), emit: Arc::new(|_| {}), screen_help: Arc::new(|| false) };
    let service = VoiceService::new(tokio::runtime::Handle::current(), deps).expect("voice service");
    let _host = Host::new(tokio::runtime::Handle::current(), agent.clone(), service.clone(), Arc::new(|_| {}));
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    agent.notes().listen(Arc::new(move |e| sink.lock().unwrap().push((Instant::now(), e))));
    let first = agent.note_start().expect("the note starts");
    let started = Instant::now();
    println!("recording for {seconds} s (first note: {first}, recognition on the video card: {})", service.recognition_on_gpu());
    let mut recording_var = false;
    while started.elapsed() < Duration::from_secs(seconds) {
        tokio::time::sleep(Duration::from_secs(1)).await;
        recording_var |= agent.store().get("vars").get("notes.recording").and_then(|v| v.as_bool()) == Some(true);
    }
    println!("status while recording: {:?}; variable notes.recording seen: {recording_var}", agent.notes().status());
    assert!(agent.notes().stop(), "it was recording");
    let stopped = Instant::now();
    while agent.notes().status().processing && stopped.elapsed() < Duration::from_secs(900) {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    println!("written down {:.1} s after the stop", stopped.elapsed().as_secs_f64());
    for (at, event) in events.lock().unwrap().iter() {
        let kind = match event {
            Event::Saved { summarized, .. } => format!("Saved (summarized: {summarized})"),
            other => format!("{other:?}"),
        };
        println!("  +{:.1} s {kind}", at.duration_since(started).as_secs_f64());
    }
    for note in agent.notes().list() {
        let body = note.markdown.to_lowercase();
        let found = expect.iter().filter(|w| body.contains(w.as_str())).count();
        let sections = body.lines().filter(|l| l.starts_with("## ")).count();
        println!("note: {} characters, {sections} sections, expected words found {found} of {}", note.markdown.chars().count(), expect.len());
        agent.notes().delete(&note.id).expect("delete");
    }
    let left: Vec<_> = std::fs::read_dir(dir.join("notes")).map(|d| d.flatten().map(|f| f.file_name()).collect()).unwrap_or_default();
    println!("left in the notes folder after deleting: {left:?}");
    service.supervisor().stop().await;
    let _ = std::fs::remove_dir_all(dir.join("notes"));
}
