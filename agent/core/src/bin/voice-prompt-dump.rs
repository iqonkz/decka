//! `voice-prompt-dump --lang <code>`: prints `{ system, tools, params }` of the voice request as JSON, built by the same
//! functions the agent uses (`voice::llm`). Input of `tools/voice-eval.mjs --prompt <file>`.

use decka_core::voice::llm::prompt_dump;
use decka_core::voice::tools::{ActionRef, AppRef, ButtonRef, Context, PageRef};

fn main() {
    let mut args = std::env::args().skip(1);
    let mut lang = "en".to_string();
    while let Some(a) = args.next() {
        if a == "--lang" {
            match args.next() {
                Some(v) => lang = v,
                None => {
                    eprintln!("usage: voice-prompt-dump --lang <code>");
                    std::process::exit(2);
                }
            }
        }
    }
    // A small, typical PC: enough for the prompt to carry its lists.
    let ctx = Context {
        lang: lang.clone(),
        actions: vec![ActionRef { id: "lock".into(), name: "Lock".into() }],
        buttons: vec![ButtonRef { label: "Mute mic".into(), action_id: "lock".into() }],
        apps: ["Calculator", "Notepad", "Chrome", "Telegram", "Spotify", "Explorer"].iter().map(|n| AppRef { name: (*n).into(), program: format!("C:\\{n}.exe") }).collect(),
        pages: vec![PageRef { id: "main".into(), name: "Main".into() }],
        ..Context::default()
    };
    println!("{}", serde_json::to_string_pretty(&prompt_dump(&ctx, &lang)).expect("serialize"));
}
