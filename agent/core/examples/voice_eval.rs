//! Accuracy of voice understanding without audio (DATA_MODEL.md 12.8): every phrase of `shared/voice/eval/<lang>.json`
//! goes through the phrase table (fast path) and, when the table does not know it, through the local `llama-server` with
//! the very prompt and schema the agent uses (`voice::llm`). Prints one row per language.
//!
//!   cargo run -p decka-core --example voice_eval -- --port <p> --key <k> [--lang ru] [--no-ai]
//!
//! The server is started by hand (`llama-server -m <gguf> -c 4096 -ngl 99 --jinja --port <p> --api-key <k>`).

use std::sync::Arc;
use std::time::Instant;

use decka_core::ai::download::{parse_manifest, Downloader};
use decka_core::ai::service::{default_root, DetectProbe, ModelChoice, VoiceDeps, VoiceService, VoiceSetArgs};
use decka_core::voice::host::table_text;
use decka_core::voice::llm::ask_at;
use decka_core::voice::matcher::{Match, Table};
use decka_core::voice::tools::{validate, ActionRef, AppRef, ButtonRef, Context, PageRef, RawCall, Resolved};
use serde_json::Value;

const LANGS: [&str; 11] = ["en", "ru", "es", "de", "fr", "pt", "zh", "hi", "ar", "ja", "kk"];

fn context(lang: &str) -> Context {
    let app = |n: &str| AppRef { name: n.into(), program: format!("C:\\Apps\\{}.exe", n.to_lowercase().replace(' ', "")) };
    Context {
        lang: lang.into(),
        actions: vec![ActionRef { id: "mute-mic".into(), name: "Mute mic".into() }],
        buttons: vec![ButtonRef { label: "Mute mic".into(), action_id: "mute-mic".into() }],
        apps: ["Calculator", "Spotify", "Discord", "Google Chrome", "Notepad", "Steam", "Telegram"].iter().map(|n| app(n)).collect(),
        pages: ["Main", "Streaming", "Media"].iter().map(|n| PageRef { id: n.to_lowercase(), name: (*n).into() }).collect(),
        confirm_actions: vec![],
        vision_ready: false,
        memory: vec![],
        weather_place: None,
        guide_running: false,
    }
}

/// Every expected argument is there and equal (numbers as numbers). Text is compared without case and by containment either
/// way: the tools match names fuzzily (`chrome` finds `Google Chrome`) and a search query may carry an extra word.
fn args_match(expected: &Value, actual: &Value) -> bool {
    let Some(exp) = expected.as_object() else { return true };
    exp.iter().all(|(k, v)| match (v, actual.get(k)) {
        (Value::String(a), Some(Value::String(b))) => {
            let (a, b) = (a.to_lowercase(), b.to_lowercase());
            a == b || (!a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a)))
        }
        (a, Some(b)) => a == b,
        _ => false,
    })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (mut port, mut key) = (get("--port").and_then(|p| p.parse::<u16>().ok()).unwrap_or(8080), get("--key").unwrap_or_default());
    // `--serve standard|light --settings <dir>`: the agent's own supervisor starts the installed servers (in a Job Object, so
    // they end with this process) and the eval talks to them.
    let _service = match get("--serve") {
        Some(which) => {
            let model = if which == "light" { ModelChoice::Light } else { ModelChoice::Standard };
            let settings = std::path::PathBuf::from(get("--settings").expect("--settings <dir>"));
            std::fs::create_dir_all(&settings).expect("settings dir");
            let manifest = parse_manifest(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/ai/manifest.json")).expect("manifest")).expect("manifest parses");
            let service = VoiceService::new(
                tokio::runtime::Handle::current(),
                VoiceDeps { root: default_root(), settings_path: settings.join("voice.json"), manifest, hardware: Arc::new(DetectProbe), mics: Arc::new(Vec::new), downloader: Downloader::default(), layers: Arc::new(decka_core::ai::service::NoLayerMemory), emit: Arc::new(|_| {}) },
            )
            .expect("service");
            service.set(VoiceSetArgs { model: Some(model), keep_loaded: Some(true), ..Default::default() }).expect("model");
            let t = Instant::now();
            let endpoints = service.supervisor().ensure_started().await.expect("the servers start");
            eprintln!("servers ready in {:.1} s (model {which})", t.elapsed().as_secs_f64());
            (port, key) = (endpoints.llama_port, endpoints.llama_key);
            Some(service)
        }
        None => None,
    };
    let only = get("--lang");
    let no_ai = args.iter().any(|a| a == "--no-ai");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/voice/eval");

    let (mut fast_time, mut ai_time) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    let (mut fast_total, mut ai_total) = (0u32, 0u32);
    println!("| lang | phrases | table | AI | correct | accuracy |");
    println!("|---|---|---|---|---|---|");
    for lang in LANGS.iter().filter(|l| only.as_deref().is_none_or(|o| o == **l)) {
        let items: Vec<Value> = match std::fs::read_to_string(root.join(format!("{lang}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok()) {
            Some(i) => i,
            None => continue,
        };
        let table = Table::parse(table_text(lang)).expect("the phrase table parses");
        let ctx = context(lang);
        let (mut fast, mut ai, mut correct) = (0u32, 0u32, 0u32);
        for item in &items {
            let (say, tool) = (item["say"].as_str().unwrap_or_default(), item["tool"].as_str().unwrap_or_default());
            let t = Instant::now();
            // The pipeline's own rule: a table hit that the tools refuse goes to the model.
            let hit = match table.matches(say) {
                Match::Hit(i) => {
                    let raw = RawCall { tool: i.tool, args: Value::Object(i.args) };
                    (!matches!(validate(&raw, &ctx), Resolved::Reject(_))).then_some(raw)
                }
                _ => None,
            };
            let got: Option<RawCall> = match hit {
                Some(raw) => {
                    fast += 1;
                    fast_time += t.elapsed();
                    Some(raw)
                }
                None if no_ai => None,
                None => {
                    ai += 1;
                    let t = Instant::now();
                    let answer = ask_at(port, &key, say, &ctx, lang).await.map(|(c, _)| c);
                    ai_time += t.elapsed();
                    answer
                }
            };
            // As the agent does: the arguments are put in shape first (T27.0); a calc counts when it works out to the same number.
            let got = got.map(|mut c| {
                if let Value::Object(m) = &c.args {
                    c.args = Value::Object(decka_core::voice::tools::normalize(&c.tool, m, &decka_core::assistant::Local::now()));
                }
                c
            });
            let calc_same = |c: &RawCall| {
                let (a, b) = (item["args"]["expr"].as_str(), c.args["expr"].as_str());
                matches!((a.map(decka_core::voice::calc::eval), b.map(decka_core::voice::calc::eval)), (Some(Ok(x)), Some(Ok(y))) if (x - y).abs() < 1e-9)
            };
            if got.as_ref().is_some_and(|c| c.tool == tool && (args_match(&item["args"], &c.args) || (tool == "calc" && calc_same(c)))) {
                correct += 1;
            } else if args.iter().any(|a| a == "--verbose") {
                eprintln!("[{lang}] {say:?}: expected {tool} {}, got {got:?}", item["args"]);
            }
        }
        println!("| {lang} | {} | {fast} | {ai} | {correct} | {:.0}% |", items.len(), correct as f64 * 100.0 / items.len().max(1) as f64);
        (fast_total, ai_total) = (fast_total + fast, ai_total + ai);
    }
    let ms = |d: std::time::Duration, n: u32| if n == 0 { 0.0 } else { d.as_secs_f64() * 1000.0 / n as f64 };
    println!("\naverage understanding time: table {:.3} ms ({fast_total} phrases), model {:.0} ms ({ai_total} phrases)", ms(fast_time, fast_total), ms(ai_time, ai_total));
}
