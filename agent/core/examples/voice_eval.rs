//! Accuracy of voice understanding without audio (DATA_MODEL.md 12.8): every phrase of `shared/voice/eval/<lang>.json`
//! goes through the phrase table (fast path) and, when the table does not know it, through the local `llama-server` with
//! the very prompt and schema the agent uses (`voice::llm`). Prints one row per language.
//!
//!   cargo run -p decka-core --example voice_eval -- --port <p> --key <k> [--lang ru] [--no-ai]
//!
//! The server is started by hand (`llama-server -m <gguf> -c 4096 -ngl 99 --jinja --port <p> --api-key <k>`).

use decka_core::voice::host::table_text;
use decka_core::voice::llm::ask_at;
use decka_core::voice::matcher::{Match, Table};
use decka_core::voice::tools::{ActionRef, AppRef, ButtonRef, Context, PageRef, RawCall};
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
    }
}

/// Every expected argument is there and equal (text without case, numbers as numbers).
fn args_match(expected: &Value, actual: &Value) -> bool {
    let Some(exp) = expected.as_object() else { return true };
    exp.iter().all(|(k, v)| match (v, actual.get(k)) {
        (Value::String(a), Some(Value::String(b))) => a.to_lowercase() == b.to_lowercase(),
        (a, Some(b)) => a == b,
        _ => false,
    })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (port, key) = (get("--port").and_then(|p| p.parse::<u16>().ok()).unwrap_or(8080), get("--key").unwrap_or_default());
    let only = get("--lang");
    let no_ai = args.iter().any(|a| a == "--no-ai");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/voice/eval");

    println!("| lang | phrases | table | AI | correct | accuracy |");
    println!("|---|---|---|---|---|---|");
    for lang in LANGS.iter().filter(|l| only.as_deref().is_none_or(|o| o == **l)) {
        let items: Vec<Value> = match std::fs::read_to_string(root.join(format!("{lang}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok()) {
            Some(i) => i,
            None => continue,
        };
        let table = Table::parse(table_text(lang)).expect("the phrase table parses");
        let ctx = context(lang);
        let (mut fast, mut ai, mut correct) = (0, 0, 0);
        for item in &items {
            let (say, tool) = (item["say"].as_str().unwrap_or_default(), item["tool"].as_str().unwrap_or_default());
            let got: Option<RawCall> = match table.matches(say) {
                Match::Hit(i) => {
                    fast += 1;
                    Some(RawCall { tool: i.tool, args: Value::Object(i.args) })
                }
                _ if no_ai => None,
                _ => {
                    ai += 1;
                    ask_at(port, &key, say, &ctx, lang).await.map(|(c, _)| c)
                }
            };
            if got.as_ref().is_some_and(|c| c.tool == tool && args_match(&item["args"], &c.args)) {
                correct += 1;
            } else if args.iter().any(|a| a == "--verbose") {
                eprintln!("[{lang}] {say:?}: expected {tool} {}, got {got:?}", item["args"]);
            }
        }
        println!("| {lang} | {} | {fast} | {ai} | {correct} | {:.0}% |", items.len(), correct as f64 * 100.0 / items.len().max(1) as f64);
    }
}
