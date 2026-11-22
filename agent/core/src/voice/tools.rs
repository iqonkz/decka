//! The tools voice control may use (DATA_MODEL.md 12.5) — all of its power. A call arrives as data (`RawCall`: from a
//! phrase table or from the model) and leaves as a `ToolCall` only when every argument passed this module's checks.
//! Nothing here lets a speaker or the model name a command, a path, a key or a URL: programs and actions come from the
//! lists the host passes in (`Context`), URLs are built here from fixed templates.

use std::collections::HashSet;

use serde_json::Value;

use super::matcher::{strip_suffix, tokens, trigram_sim, FUZZY_THRESHOLD};
use crate::model::{MediaOp, SystemOp};

/// The whole white list (12.5 without the later stages: write, notes, screen help, files, calendar, chains).
pub const TOOLS: [&str; 16] = [
    "press_button", "run_action", "open_app", "switch_app", "close_app", "window_other_monitor", "which_monitor", "volume", "media", "go_page",
    "web_search", "open_site", "type_text", "dictate", "timer", "system",
];

pub const MAX_TEXT: usize = 2000;
pub const MAX_QUERY: usize = 300;
const MAX_LABEL: usize = 40;
/// Candidates whose scores differ by less than this are ambiguous.
const TIE_MARGIN: f64 = 0.05;

#[derive(Debug, Clone, PartialEq)]
pub struct RawCall {
    pub tool: String,
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ButtonRef {
    pub label: String,
    pub action_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppRef {
    pub name: String,
    /// What the host launches or finds a window of; never comes from the speaker.
    pub program: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageRef {
    pub id: String,
    pub name: String,
}

/// What exists on this PC right now; the host builds it from `actions.json`, `layout.json` and the installed programs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Language of the phrase table in use: picks the case suffixes cut from spoken names.
    pub lang: String,
    pub actions: Vec<ActionRef>,
    pub buttons: Vec<ButtonRef>,
    pub apps: Vec<AppRef>,
    pub pages: Vec<PageRef>,
    /// Action ids that need a confirmation (layout buttons with `confirm`).
    pub confirm_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeOp {
    Up,
    Down,
    Mute,
    Set(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolCall {
    RunAction { action_id: String },
    OpenApp { name: String, program: String },
    SwitchApp { name: String, program: String },
    CloseApp { name: String, program: String },
    WindowOtherMonitor,
    WhichMonitor,
    Volume(VolumeOp),
    Media(MediaOp),
    GoPage { page_id: String, name: String },
    OpenUrl { url: String },
    TypeText { text: String },
    Dictate,
    Timer { minutes: u32, label: Option<String> },
    System(SystemOp),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Validated {
    pub call: ToolCall,
    /// The call is risky: it waits for `voice_confirm` (12.4 item 5).
    pub confirm: bool,
    /// What the confirmation shows and the log keeps, e.g. `Lock the computer`.
    pub summary: String,
    /// i18n key of the confirmation (`voice.confirm.*`) with its arguments; `summary` is the English fallback.
    pub key: Option<&'static str>,
    pub args: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    Ready(Validated),
    /// Several buttons, programs or pages fit equally: ask, do nothing.
    Ask { question: String, options: Vec<String> },
    /// Nothing to do; the text says why.
    Reject(String),
}

fn reject(why: &str) -> Resolved {
    Resolved::Reject(why.to_string())
}

fn ready(call: ToolCall, confirm: bool, summary: impl Into<String>) -> Resolved {
    Resolved::Ready(Validated { call, confirm, summary: summary.into(), key: None, args: Default::default() })
}

/// `ready` for a confirmation with an i18n key.
fn ready_key(call: ToolCall, confirm: bool, summary: impl Into<String>, key: &'static str, args: &[(&str, &str)]) -> Resolved {
    let args = args.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    Resolved::Ready(Validated { call, confirm, summary: summary.into(), key: Some(key), args })
}

/// Only these fields may be present; anything else makes the whole call void.
fn only(args: &serde_json::Map<String, Value>, allowed: &[&str]) -> Result<(), Resolved> {
    match args.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(Resolved::Reject(format!("unexpected field: {k}"))),
        None => Ok(()),
    }
}

fn text<'a>(args: &'a serde_json::Map<String, Value>, key: &str, max: usize) -> Result<&'a str, Resolved> {
    let s = args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    match s {
        Some(s) if s.chars().count() <= max => Ok(s),
        Some(_) => Err(Resolved::Reject(format!("{key} is too long"))),
        None => Err(Resolved::Reject(format!("{key} is missing"))),
    }
}

// ---- Name resolution --------------------------------------------------------------------------------------------

/// Score of a spoken name against a known name: 1.0 equal, 0.9 all spoken words are words of the name, else trigrams.
fn name_score(spoken: &[String], known: &str) -> f64 {
    let known_toks = tokens(known);
    if known_toks == spoken {
        return 1.0;
    }
    if !spoken.is_empty() && spoken.iter().all(|w| known_toks.contains(w)) {
        return 0.9;
    }
    let s = trigram_sim(&spoken.join(" "), &known_toks.join(" "));
    if s >= FUZZY_THRESHOLD { s } else { 0.0 }
}

/// Indices of the best-fitting entries (several when the best scores are within the margin). `key` merges entries that
/// are the same target under different names.
fn best<T>(lang: &str, spoken: &str, items: &[T], name: impl Fn(&T) -> &str, key: impl Fn(&T) -> String) -> Vec<usize> {
    let mut spoken = tokens(spoken);
    let score = |spoken: &[String]| -> Vec<(f64, usize)> {
        items.iter().enumerate().map(|(i, t)| (name_score(spoken, name(t)), i)).filter(|s| s.0 > 0.0).collect()
    };
    let mut scored = score(&spoken);
    // A case suffix on the last word is cut only when the name as spoken matches nothing.
    if scored.is_empty() {
        if let Some(stem) = spoken.last().and_then(|w| strip_suffix(lang, w)) {
            *spoken.last_mut().unwrap() = stem;
            scored = score(&spoken);
        }
    }
    let Some(top) = scored.iter().map(|s| s.0).fold(None, |m: Option<f64>, x| Some(m.map_or(x, |y| y.max(x)))) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    scored.into_iter().filter(|s| top - s.0 < TIE_MARGIN).filter(|s| seen.insert(key(&items[s.1]))).map(|s| s.1).collect()
}

fn ask(options: Vec<String>) -> Resolved {
    let shown = options.iter().take(3).cloned().collect::<Vec<_>>().join(" or ");
    Resolved::Ask { question: format!("Which one: {shown}?"), options }
}

// ---- URLs -------------------------------------------------------------------------------------------------------

/// Percent-encodes every byte outside the unreserved set.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `^[a-z0-9-]+(\.[a-z0-9-]+)+$`, at most 253 characters.
pub fn is_domain(s: &str) -> bool {
    s.len() <= 253 && s.contains('.') && s.split('.').all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
}

// ---- Validation -------------------------------------------------------------------------------------------------

/// The single entrance of a voice request into Decka's executors.
pub fn validate(raw: &RawCall, ctx: &Context) -> Resolved {
    if !TOOLS.contains(&raw.tool.as_str()) {
        return Resolved::Reject(format!("unknown tool: {}", raw.tool));
    }
    let empty = serde_json::Map::new();
    let args = match &raw.args {
        Value::Object(m) => m,
        Value::Null => &empty,
        _ => return reject("arguments must be an object"),
    };
    match run(raw.tool.as_str(), args, ctx) {
        Ok(r) | Err(r) => r,
    }
}

fn run(tool: &str, args: &serde_json::Map<String, Value>, ctx: &Context) -> Result<Resolved, Resolved> {
    Ok(match tool {
        "run_action" => {
            only(args, &["id"])?;
            let id = text(args, "id", 64)?;
            match ctx.actions.iter().find(|a| a.id == id) {
                Some(a) => run_action(a, ctx),
                None => reject("no such action"),
            }
        }
        "press_button" => {
            only(args, &["label"])?;
            let label = text(args, "label", 100)?;
            // A button by its label, or any action by its name.
            let mut cands: Vec<(String, String)> = ctx.buttons.iter().map(|b| (b.label.clone(), b.action_id.clone())).collect();
            cands.extend(ctx.actions.iter().map(|a| (a.name.clone(), a.id.clone())));
            let found = best(&ctx.lang, label, &cands, |c| c.0.as_str(), |c| c.1.clone());
            match found.as_slice() {
                [] => reject("no such button"),
                [i] => match ctx.actions.iter().find(|a| a.id == cands[*i].1) {
                    Some(a) => run_action(a, ctx),
                    None => reject("no such action"),
                },
                many => ask(many.iter().map(|i| cands[*i].0.clone()).collect()),
            }
        }
        "open_app" | "switch_app" | "close_app" => {
            only(args, &["name"])?;
            let name = text(args, "name", 100)?;
            let found = best(&ctx.lang, name, &ctx.apps, |a| a.name.as_str(), |a| a.program.to_ascii_lowercase());
            match found.as_slice() {
                [] => reject("that program is not installed"),
                [i] => {
                    let app = &ctx.apps[*i];
                    let (n, p) = (app.name.clone(), app.program.clone());
                    match tool {
                        "open_app" => ready(ToolCall::OpenApp { name: n.clone(), program: p }, false, format!("Open {n}")),
                        "switch_app" => ready(ToolCall::SwitchApp { name: n.clone(), program: p }, false, format!("Switch to {n}")),
                        _ => ready(ToolCall::CloseApp { name: n.clone(), program: p }, false, format!("Close {n}")),
                    }
                }
                many => ask(many.iter().map(|i| ctx.apps[*i].name.clone()).collect()),
            }
        }
        "window_other_monitor" => {
            only(args, &[])?;
            ready(ToolCall::WindowOtherMonitor, false, "Move the window to the other monitor")
        }
        "which_monitor" => {
            only(args, &[])?;
            ready(ToolCall::WhichMonitor, false, "Which monitor")
        }
        "volume" => {
            only(args, &["op", "value"])?;
            let op = args.get("op").and_then(Value::as_str).unwrap_or_default();
            let value = match args.get("value") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_u64().filter(|n| *n <= 100).ok_or_else(|| reject("value must be 0..100"))? as u8),
            };
            match (op, value) {
                ("up", None) => ready(ToolCall::Volume(VolumeOp::Up), false, "Volume up"),
                ("down", None) => ready(ToolCall::Volume(VolumeOp::Down), false, "Volume down"),
                ("mute", None) => ready(ToolCall::Volume(VolumeOp::Mute), false, "Mute"),
                ("set", Some(v)) => ready(ToolCall::Volume(VolumeOp::Set(v)), false, format!("Volume {v}")),
                ("set", None) => reject("value is missing"),
                ("up" | "down" | "mute", Some(_)) => reject("unexpected field: value"),
                _ => reject("unknown volume op"),
            }
        }
        "media" => {
            only(args, &["op"])?;
            let op = match args.get("op").and_then(Value::as_str).unwrap_or_default() {
                "play_pause" => MediaOp::PlayPause,
                "next" => MediaOp::Next,
                "prev" => MediaOp::Prev,
                "shuffle" => MediaOp::Shuffle,
                "repeat" => MediaOp::Repeat,
                _ => return Err(reject("unknown media op")),
            };
            ready(ToolCall::Media(op), false, "Media")
        }
        "go_page" => {
            only(args, &["name"])?;
            let name = text(args, "name", 100)?;
            let found = best(&ctx.lang, name, &ctx.pages, |p| p.name.as_str(), |p| p.id.clone());
            match found.as_slice() {
                [] => reject("no such page"),
                [i] => ready(ToolCall::GoPage { page_id: ctx.pages[*i].id.clone(), name: ctx.pages[*i].name.clone() }, false, format!("Go to page {}", ctx.pages[*i].name)),
                many => ask(many.iter().map(|i| ctx.pages[*i].name.clone()).collect()),
            }
        }
        "web_search" => {
            only(args, &["query", "site"])?;
            let query = text(args, "query", MAX_QUERY)?;
            let base = match args.get("site").and_then(Value::as_str).unwrap_or("web") {
                "web" => "https://www.google.com/search?q=",
                "youtube" => "https://www.youtube.com/results?search_query=",
                "maps" => "https://www.google.com/maps/search/",
                _ => return Err(reject("unknown site")),
            };
            ready(ToolCall::OpenUrl { url: format!("{base}{}", percent_encode(query)) }, false, format!("Search for {query}"))
        }
        "open_site" => {
            only(args, &["domain"])?;
            let domain = text(args, "domain", 253)?.to_ascii_lowercase();
            if !is_domain(&domain) {
                return Err(reject("that is not a domain"));
            }
            ready(ToolCall::OpenUrl { url: format!("https://{domain}") }, false, format!("Open {domain}"))
        }
        "type_text" => {
            only(args, &["text"])?;
            let t = args.get("text").and_then(Value::as_str).filter(|s| !s.is_empty() && s.chars().count() <= MAX_TEXT);
            match t {
                Some(t) => ready(ToolCall::TypeText { text: t.to_string() }, false, "Type text"),
                None => reject("text is empty or too long"),
            }
        }
        "dictate" => {
            only(args, &[])?;
            ready(ToolCall::Dictate, false, "Dictate")
        }
        "timer" => {
            only(args, &["minutes", "label"])?;
            let minutes = args.get("minutes").and_then(Value::as_u64).filter(|m| (1..=180).contains(m)).ok_or_else(|| reject("minutes must be 1..180"))? as u32;
            let label = match args.get("label") {
                None | Some(Value::Null) => None,
                Some(_) => Some(text(args, "label", MAX_LABEL)?.to_string()),
            };
            ready(ToolCall::Timer { minutes, label }, false, format!("Timer for {minutes} min"))
        }
        "system" => {
            only(args, &["op"])?;
            // Only the harmless pair; shutdown, restart and the like are not voice tools.
            match args.get("op").and_then(Value::as_str).unwrap_or_default() {
                "lock" => ready_key(ToolCall::System(SystemOp::Lock), true, "Lock the computer", "voice.confirm.lock", &[]),
                "sleep" => ready_key(ToolCall::System(SystemOp::Sleep), true, "Put the computer to sleep", "voice.confirm.sleep", &[]),
                _ => reject("unknown system op"),
            }
        }
        _ => reject("unknown tool"),
    })
}

fn run_action(a: &ActionRef, ctx: &Context) -> Resolved {
    ready_key(ToolCall::RunAction { action_id: a.id.clone() }, ctx.confirm_actions.contains(&a.id), format!("Run {}", a.name), "voice.confirm.run", &[("name", &a.name)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> Context {
        Context {
            actions: vec![
                ActionRef { id: "lock".into(), name: "Lock".into() },
                ActionRef { id: "obs-rec".into(), name: "Start recording".into() },
                ActionRef { id: "shutdown".into(), name: "Shut down".into() },
            ],
            buttons: vec![
                ButtonRef { label: "Record".into(), action_id: "obs-rec".into() },
                ButtonRef { label: "Power off".into(), action_id: "shutdown".into() },
            ],
            apps: vec![
                AppRef { name: "Calculator".into(), program: "C:\\Windows\\calc.exe".into() },
                AppRef { name: "Google Chrome".into(), program: "C:\\chrome.exe".into() },
                AppRef { name: "Chrome Remote Desktop".into(), program: "C:\\crd.exe".into() },
                AppRef { name: "Notepad".into(), program: "C:\\notepad.exe".into() },
                AppRef { name: "Notepad (shortcut)".into(), program: "C:\\NOTEPAD.EXE".into() },
            ],
            pages: vec![PageRef { id: "main".into(), name: "Main".into() }, PageRef { id: "media".into(), name: "Media".into() }],
            confirm_actions: vec!["shutdown".into()],
            lang: "en".into(),
        }
    }

    #[test]
    fn a_kazakh_case_suffix_is_cut_only_when_the_raw_name_matches_nothing() {
        let mut c = ctx();
        c.lang = "kk".into();
        c.apps.push(AppRef { name: "Хром".into(), program: "C:\\hrom.exe".into() });
        c.apps.push(AppRef { name: "Тіркелді".into(), program: "C:\\reg.exe".into() });
        let open = |c: &Context, name: &str| validate(&RawCall { tool: "open_app".into(), args: json!({ "name": name }) }, c);
        match open(&c, "хромнан") {
            Resolved::Ready(v) => assert!(matches!(v.call, ToolCall::OpenApp { ref program, .. } if program == "C:\\hrom.exe")),
            other => panic!("{other:?}"),
        }
        // The raw name is itself a known name: nothing is cut.
        match open(&c, "тіркелді") {
            Resolved::Ready(v) => assert!(matches!(v.call, ToolCall::OpenApp { ref program, .. } if program == "C:\\reg.exe")),
            other => panic!("{other:?}"),
        }
        c.lang = "en".into();
        assert!(!matches!(open(&c, "хромнан"), Resolved::Ready(_)));
    }

    fn call(tool: &str, args: Value) -> Resolved {
        validate(&RawCall { tool: tool.into(), args }, &ctx())
    }

    fn ready_of(r: Resolved) -> Validated {
        match r {
            Resolved::Ready(v) => v,
            other => panic!("{other:?}"),
        }
    }

    fn is_reject(r: Resolved) -> bool {
        matches!(r, Resolved::Reject(_))
    }

    #[test]
    fn the_white_list_is_exactly_the_stage_one_tools() {
        assert_eq!(TOOLS.len(), 16);
        for t in ["write", "shell", "run", "exec", "open_url", "launch", "hotkey"] {
            assert!(is_reject(call(t, json!({}))), "{t}");
        }
    }

    #[test]
    fn an_unknown_action_id_or_an_extra_field_voids_the_call() {
        assert_eq!(ready_of(call("run_action", json!({ "id": "lock" }))).call, ToolCall::RunAction { action_id: "lock".into() });
        assert!(is_reject(call("run_action", json!({ "id": "rm-rf" }))));
        assert!(is_reject(call("run_action", json!({ "id": "lock", "args": ["/c", "calc"] }))));
        assert!(is_reject(call("run_action", json!({ "id": "lock", "program": "cmd.exe" }))));
        assert!(is_reject(call("run_action", json!({}))));
        assert!(is_reject(call("run_action", json!("lock"))));
        assert!(is_reject(call("media", json!({ "op": "play_pause", "program": "x" }))));
        assert!(is_reject(call("system", json!({ "op": "lock", "force": true }))));
        assert!(is_reject(call("dictate", json!({ "x": 1 }))));
        assert!(is_reject(call("web_search", json!({ "query": "a", "url": "http://evil" }))));
    }

    #[test]
    fn a_button_is_found_by_its_label_or_its_action_name_and_its_confirmation_is_kept() {
        let v = ready_of(call("press_button", json!({ "label": "record" })));
        assert_eq!(v.call, ToolCall::RunAction { action_id: "obs-rec".into() });
        assert!(!v.confirm);
        assert_eq!(ready_of(call("press_button", json!({ "label": "start recording" }))).call, ToolCall::RunAction { action_id: "obs-rec".into() });
        let v = ready_of(call("press_button", json!({ "label": "power off" })));
        assert!(v.confirm, "the layout marks this button for confirmation");
        assert!(is_reject(call("press_button", json!({ "label": "nothing like it" }))));
    }

    #[test]
    fn open_app_takes_only_installed_programs() {
        let v = ready_of(call("open_app", json!({ "name": "calculator" })));
        assert_eq!(v.call, ToolCall::OpenApp { name: "Calculator".into(), program: "C:\\Windows\\calc.exe".into() });
        assert!(is_reject(call("open_app", json!({ "name": "C:\\Windows\\System32\\cmd.exe" }))));
        assert!(is_reject(call("open_app", json!({ "name": "powershell" }))));
        assert!(is_reject(call("close_app", json!({ "name": "unknown program" }))));
        // The same program under two shortcuts is one candidate.
        assert!(matches!(ready_of(call("switch_app", json!({ "name": "notepad" }))).call, ToolCall::SwitchApp { .. }));
    }

    #[test]
    fn two_equally_good_programs_make_a_question_not_an_action() {
        match call("open_app", json!({ "name": "chrome" })) {
            Resolved::Ask { options, question } => {
                assert_eq!(options.len(), 2);
                assert!(question.contains("Google Chrome") && question.contains("Chrome Remote Desktop"), "{question}");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(ready_of(call("open_app", json!({ "name": "google chrome" }))).call, ToolCall::OpenApp { .. }));
    }

    #[test]
    fn pages_resolve_by_name() {
        assert_eq!(ready_of(call("go_page", json!({ "name": "media" }))).call, ToolCall::GoPage { page_id: "media".into(), name: "Media".into() });
        assert!(is_reject(call("go_page", json!({ "name": "secret" }))));
    }

    #[test]
    fn open_site_refuses_everything_that_is_not_a_plain_domain() {
        assert_eq!(ready_of(call("open_site", json!({ "domain": "Example.com" }))).call, ToolCall::OpenUrl { url: "https://example.com".into() });
        assert!(matches!(ready_of(call("open_site", json!({ "domain": "docs.rs" }))).call, ToolCall::OpenUrl { .. }));
        for bad in ["localhost", "a b.com", "http://x.com", "x.com/path", "x.com?q=1", "x..com", ".com", "x.com:8080", "javascript:alert(1)", "x_y.com", "", "ex ample.com"] {
            assert!(is_reject(call("open_site", json!({ "domain": bad }))), "{bad:?}");
        }
        assert!(!is_domain(&format!("{}.com", "a".repeat(260))));
    }

    #[test]
    fn web_search_encodes_the_query_into_a_fixed_template() {
        let u = |site: &str, q: &str| match ready_of(call("web_search", json!({ "query": q, "site": site }))).call {
            ToolCall::OpenUrl { url } => url,
            other => panic!("{other:?}"),
        };
        assert_eq!(u("web", "cats & dogs"), "https://www.google.com/search?q=cats%20%26%20dogs");
        assert_eq!(u("youtube", "lo-fi"), "https://www.youtube.com/results?search_query=lo-fi");
        assert_eq!(u("maps", "кафе"), "https://www.google.com/maps/search/%D0%BA%D0%B0%D1%84%D0%B5");
        assert_eq!(u("web", "a#b?c=d/../e"), "https://www.google.com/search?q=a%23b%3Fc%3Dd%2F..%2Fe");
        assert!(is_reject(call("web_search", json!({ "query": "x", "site": "bing" }))));
        assert!(is_reject(call("web_search", json!({ "query": "" }))));
        assert!(is_reject(call("web_search", json!({ "query": "x".repeat(MAX_QUERY + 1) }))));
        // The default site is the web.
        assert!(ready_of(call("web_search", json!({ "query": "x" }))).summary.contains('x'));
    }

    #[test]
    fn volume_timer_and_text_have_ranges() {
        assert_eq!(ready_of(call("volume", json!({ "op": "set", "value": 30 }))).call, ToolCall::Volume(VolumeOp::Set(30)));
        assert!(is_reject(call("volume", json!({ "op": "set", "value": 101 }))));
        assert!(is_reject(call("volume", json!({ "op": "set" }))));
        assert!(is_reject(call("volume", json!({ "op": "up", "value": 5 }))));
        assert!(is_reject(call("volume", json!({ "op": "louder" }))));
        assert_eq!(ready_of(call("timer", json!({ "minutes": 15 }))).call, ToolCall::Timer { minutes: 15, label: None });
        assert!(matches!(ready_of(call("timer", json!({ "minutes": 180, "label": "tea" }))).call, ToolCall::Timer { .. }));
        for bad in [json!({ "minutes": 0 }), json!({ "minutes": 181 }), json!({ "minutes": -3 }), json!({ "minutes": "5" }), json!({})] {
            assert!(is_reject(call("timer", bad)));
        }
        assert!(matches!(ready_of(call("type_text", json!({ "text": "hello\nworld" }))).call, ToolCall::TypeText { .. }));
        assert!(is_reject(call("type_text", json!({ "text": "x".repeat(MAX_TEXT + 1) }))));
        assert!(is_reject(call("type_text", json!({ "text": "" }))));
        assert!(is_reject(call("media", json!({ "op": "vol_up" }))), "volume belongs to the volume tool");
    }

    #[test]
    fn system_needs_a_confirmation_and_offers_only_lock_and_sleep() {
        let lock = ready_of(call("system", json!({ "op": "lock" })));
        assert!(lock.confirm);
        assert_eq!(lock.call, ToolCall::System(SystemOp::Lock));
        assert_eq!(lock.summary, "Lock the computer");
        assert!(ready_of(call("system", json!({ "op": "sleep" }))).confirm);
        for op in ["shutdown", "restart", "signout", "hibernate", "display_off", ""] {
            assert!(is_reject(call("system", json!({ "op": op }))), "{op}");
        }
    }

    #[test]
    fn monitor_and_dictation_tools_take_no_arguments() {
        assert_eq!(ready_of(call("which_monitor", Value::Null)).call, ToolCall::WhichMonitor);
        assert_eq!(ready_of(call("window_other_monitor", json!({}))).call, ToolCall::WindowOtherMonitor);
        assert_eq!(ready_of(call("dictate", json!({}))).call, ToolCall::Dictate);
    }
}
