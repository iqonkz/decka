//! A manual check of the screen guide against the real desktop (T27.R): reads the active window through UI Automation and asks
//! the local model for the first step of a goal. Prints what it saw and what it chose; presses nothing.
//!
//!   cargo run -p decka-core --example guide_demo -- --port 8089 --key <k> --goal "turn on dark mode" [--wait 4]

use decka_core::voice::screenhelp::{parse_step, step_request_body, Step};
use decka_core::voice::ui_snapshot::active_window;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
    let (port, key, goal) = (get("--port").unwrap_or("8089".into()), get("--key").unwrap_or_default(), get("--goal").unwrap_or("turn on dark mode".into()));
    let wait: u64 = get("--wait").and_then(|w| w.parse().ok()).unwrap_or(4);
    std::thread::sleep(std::time::Duration::from_secs(wait));
    println!("foreground: {:?} (handle {})", decka_core::voice::context::foreground_title(), decka_core::voice::winops::foreground_handle());
    let started = std::time::Instant::now();
    let Some(snap) = active_window() else { return println!("no snapshot") };
    println!("snapshot: {} elements in {} ms (truncated: {})", snap.elements.len(), started.elapsed().as_millis(), snap.truncated);
    for e in snap.elements.iter().take(12) {
        println!("  {:>3} {:<10} {:?} [{}]", e.id, e.role, e.name, e.automation_id);
    }
    let body = step_request_body(&goal, &snap, "English", &[]);
    let t = std::time::Instant::now();
    let answer = reqwest::Client::new().post(format!("http://127.0.0.1:{port}/v1/chat/completions")).bearer_auth(key).header("Content-Type", "application/json").body(serde_json::to_vec(&body).unwrap()).send().await.and_then(|r| r.error_for_status());
    let Ok(answer) = answer else { return println!("the model did not answer") };
    let text: serde_json::Value = serde_json::from_str(&answer.text().await.unwrap_or_default()).unwrap_or_default();
    let content = text.pointer("/choices/0/message/content").and_then(|c| c.as_str()).unwrap_or_default().to_string();
    println!("model said ({} ms): {content}", t.elapsed().as_millis());
    match parse_step(&content, &snap) {
        Ok(Step::Element { id, say }) => println!("STEP -> {} {:?}: {say}", id, snap.get(id).map(|e| e.name.clone())),
        Ok(other) => println!("{other:?}"),
        Err(e) => println!("refused: {e:?}"),
    }
}
