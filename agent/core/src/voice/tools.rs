//! The tools voice control may use (DATA_MODEL.md 12.5) — all of its power. A call arrives as data (`RawCall`: from a
//! phrase table or from the model) and leaves as a `ToolCall` only when every argument passed this module's checks.
//! Nothing here lets a speaker or the model name a command, a path, a key or a URL: programs and actions come from the
//! lists the host passes in (`Context`), URLs are built here from fixed templates.

use std::collections::HashSet;

use serde_json::Value;

use super::matcher::{strip_suffix, tokens, trigram_sim, FUZZY_THRESHOLD};
use crate::model::{MediaOp, SystemOp};

/// The whole white list (12.5 without the later stages: write, notes, screen help, files, calendar, chains).
pub const TOOLS: &[&str] = &[
    "press_button", "run_action", "open_app", "switch_app", "close_app", "window_other_monitor", "which_monitor", "volume", "media", "go_page",
    "web_search", "open_site", "type_text", "dictate", "timer", "system", "text_transform", "screen_ask",
    "memory_add", "memory_remove", "day_brief", "remind", "reminders_show", "list_add", "list_show", "list_remove", "calc", "ask",
    "lyrics_window", "status", "reply_action",
    // M27: eyes and hands on the screen, reading aloud, work modes.
    "find_on_screen", "ui_act", "guide", "guide_record", "guide_control", "read_aloud", "mode_run", "mode_save",
    // M29: a ready-made button of the catalog, a picture of the screen.
    "add_button", "screenshot",
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
    /// The Screen help package (the projector of the model) is on the disk; without it `screen_ask` first asks to download it.
    pub vision_ready: bool,
    /// Facts the user asked to remember that fit the phrase (at most 20): data for the model, never an instruction.
    pub memory: Vec<String>,
    /// The place of the weather widget (T26.F-6): a phrase naming another city is not answered from it.
    pub weather_place: Option<String>,
    /// A guide or a recording is running: "next", "back", "stop", "done" mean its controls (T27.5).
    pub guide_running: bool,
    /// This request cut a spoken answer short (T29.8): "stop" then means only that.
    pub speech_interrupted: bool,
    /// The pages without a free cell, and the page the voice went to last (T29.6: a new button goes there).
    pub full_pages: Vec<String>,
    pub current_page: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideCtl {
    Next,
    Back,
    Stop,
    Done,
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
    /// T26.0: hide (`show: false`) or show the lyrics window of the PC; not stored in config.
    LyricsWindow { show: bool },
    /// T26.3: what to do with the answer that was just shown (copy, insert, read aloud, another variant); only within 30 s of it.
    ReplyAction { kind: super::followup::Kind },
    /// T26.1: a fact about this PC or its widgets, answered from the store by a fixed template (never from the internet).
    Status { topic: super::status::Topic },
    /// T27.3: highlight the element of the active window that fits `query`; `None` with `next` is "not that one" (the next candidate).
    FindOnScreen { query: Option<String> },
    /// T27.4: press, scroll or close a tab. `target: None` is "this" (the last highlight). A dangerous name is pressed only when
    /// `confirmed` (the pipeline sets it after the "yes").
    UiAct { target: Option<String>, action: super::screenhelp::UiAction, confirmed: bool },
    /// T27.5: a guide through the program, step by step.
    Guide { goal: String },
    /// T27.6: record the steps of the user as a recipe for `goal`.
    GuideRecord { goal: String },
    /// "Next", "back", "stop", "done" during a guide or a recording.
    GuideControl { kind: GuideCtl },
    /// T27.9: read the selection (or the element under the cursor) aloud.
    ReadAloud,
    /// T27.10: open and arrange the programs of a saved mode / save the present layout as a mode.
    ModeRun { name: String },
    ModeSave { name: String },
    Volume(VolumeOp),
    Media(MediaOp),
    GoPage { page_id: String, name: String },
    OpenUrl { url: String },
    TypeText { text: String },
    /// A long dictation, pasted through the clipboard. Never named by the model: only the pipeline makes it (DATA_MODEL.md 13.1).
    PasteText { text: String },
    /// `media {op: "play"}` (T29.2): start what is paused.
    MediaPlay,
    /// T29.6: a ready-made button of the catalog on a page (after a yes).
    AddButton { catalog_id: String, page_id: String },
    /// T29.6: a key that asks for something: the settings window opens on it.
    SetupButton { catalog_id: String },
    /// T29.7: a picture of the monitor, the window or a chosen area.
    Screenshot(crate::model::ShotArea),
    Dictate,
    Timer { minutes: u32, label: Option<String> },
    System(SystemOp),
    /// Work on the selected text (DATA_MODEL.md 13.2); `lang` is the target of `translate`.
    /// `text` (T26.2): said by the user instead of selected; then nothing is captured and the result is only shown.
    TextTransform { op: super::textops::Op, lang: Option<String>, text: Option<String> },
    /// A question about the active window (DATA_MODEL.md 13.3).
    ScreenAsk { question: String },
    /// Arithmetic worked out on the PC by `calc.rs` (DATA_MODEL.md 12.5): never by the model.
    Calc { expr: String },
    /// A short answer of the model to a question that is not a command (DATA_MODEL.md 12.5).
    Ask { question: String },
    /// A reminder (DATA_MODEL.md 13.4); the moment is worked out by the agent from `when`, never by the model.
    Remind { text: String, when: crate::assistant::When, repeat: Option<crate::assistant::Repeat> },
    RemindersShow,
    /// The brief of the day (DATA_MODEL.md 13.5).
    DayBrief,
    /// DATA_MODEL.md 13.6: only after an explicit "remember that ...".
    MemoryAdd { text: String },
    MemoryRemove { text: String },
    ListAdd { list: String, item: String },
    ListShow { list: String },
    /// `item: None` deletes the whole list (after a confirmation).
    ListRemove { list: String, item: Option<String> },
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
    let args = normalize(raw.tool.as_str(), args, &crate::assistant::Local::now());
    match run(raw.tool.as_str(), &args, ctx) {
        Ok(r) | Err(r) => r,
    }
}

/// Shapes of an argument the model gets wrong often are put right after parsing, not by the prompt (T27.0):
/// a `remind` with `at` as a clock time ("16:30") or with a `time` and no `day` becomes `day` + `time` (today when the time is still
/// ahead, tomorrow otherwise). Everything else stays as it came; a refusal still comes from `run`.
pub fn normalize(tool: &str, args: &serde_json::Map<String, Value>, now: &crate::assistant::Local) -> serde_json::Map<String, Value> {
    let mut out = args.clone();
    if tool != "remind" {
        return out;
    }
    let is_clock = |s: &str| s.trim().split_once(':').is_some_and(|(h, m)| h.parse::<u32>().is_ok_and(|h| h < 24) && m.parse::<u32>().is_ok_and(|m| m < 60));
    if let Some(Value::String(at)) = out.get("at") {
        if is_clock(at) && !out.contains_key("time") {
            let t = at.trim().to_string();
            out.remove("at");
            out.insert("time".into(), Value::String(t));
        }
    }
    if let Some(Value::String(t)) = out.get("time").cloned() {
        if !out.contains_key("day") && !out.contains_key("repeat") && !out.contains_key("inMinutes") && is_clock(&t) {
            let (h, m) = t.trim().split_once(':').map(|(h, m)| (h.parse::<u32>().unwrap_or(0), m.parse::<u32>().unwrap_or(0))).unwrap_or((0, 0));
            let ahead = (h, m) > (now.hour, now.minute);
            out.insert("day".into(), Value::String(if ahead { "today" } else { "tomorrow" }.into()));
        }
    }
    out
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
        "find_on_screen" => {
            only(args, &["query", "next"])?;
            if args.get("next").and_then(Value::as_bool) == Some(true) {
                if args.contains_key("query") {
                    return Err(reject("next takes no query"));
                }
                ready(ToolCall::FindOnScreen { query: None }, false, "Next candidate")
            } else {
                let q = text(args, "query", 100)?.to_string();
                ready(ToolCall::FindOnScreen { query: Some(q.clone()) }, false, format!("Find {q} on the screen"))
            }
        }
        "ui_act" => {
            only(args, &["target", "action"])?;
            let action = text(args, "action", 20).ok().and_then(super::screenhelp::UiAction::parse).ok_or_else(|| reject("unknown action"))?;
            let target = match args.get("target") {
                None | Some(Value::Null) => None,
                Some(_) => {
                    let t = text(args, "target", 100)?;
                    (!matches!(t.to_lowercase().as_str(), "this" | "it" | "это" | "это самое")).then(|| t.to_string())
                }
            };
            // A dangerous name that was said is confirmed before anything is pressed; what is only "this" is checked when it runs.
            let danger = action == super::screenhelp::UiAction::Click && target.as_deref().is_some_and(|t| super::screenhelp::is_dangerous(t, &super::screenhelp::all_danger_words().iter().map(String::as_str).collect::<Vec<_>>()));
            let summary = format!("Press {}", target.as_deref().unwrap_or("this"));
            ready(ToolCall::UiAct { target, action, confirmed: danger }, danger, summary)
        }
        "guide" => {
            only(args, &["goal"])?;
            let goal = text(args, "goal", 200)?.to_string();
            ready(ToolCall::Guide { goal: goal.clone() }, false, format!("Guide: {goal}"))
        }
        "guide_record" => {
            only(args, &["goal"])?;
            let goal = text(args, "goal", 200)?.to_string();
            ready(ToolCall::GuideRecord { goal: goal.clone() }, false, format!("Record: {goal}"))
        }
        "guide_control" => {
            only(args, &["kind"])?;
            let kind = match text(args, "kind", 10)? {
                "next" => GuideCtl::Next,
                "back" => GuideCtl::Back,
                "stop" => GuideCtl::Stop,
                "done" => GuideCtl::Done,
                _ => return Err(reject("unknown guide control")),
            };
            ready(ToolCall::GuideControl { kind }, false, "Guide control")
        }
        "read_aloud" => {
            only(args, &[])?;
            ready(ToolCall::ReadAloud, false, "Read aloud")
        }
        "mode_run" | "mode_save" => {
            only(args, &["name"])?;
            let name = text(args, "name", 60)?.to_string();
            if tool == "mode_run" {
                ready(ToolCall::ModeRun { name: name.clone() }, false, format!("Mode {name}"))
            } else {
                ready(ToolCall::ModeSave { name: name.clone() }, false, format!("Save the layout as {name}"))
            }
        }
        "which_monitor" => {
            only(args, &[])?;
            ready(ToolCall::WhichMonitor, false, "Which monitor")
        }
        "reply_action" => {
            only(args, &["kind"])?;
            // `lang:<code>` belongs to the window of the PC (IPC), not to a spoken phrase.
            match args.get("kind").and_then(Value::as_str).filter(|k| !k.starts_with("lang")).and_then(super::followup::Kind::parse) {
                Some(kind) => ready(ToolCall::ReplyAction { kind }, false, "Answer action"),
                None => reject("unknown answer action"),
            }
        }
        "status" => {
            only(args, &["topic"])?;
            match args.get("topic").and_then(Value::as_str).and_then(super::status::Topic::parse) {
                Some(topic) => ready(ToolCall::Status { topic }, false, "Status"),
                None => reject("unknown status topic"),
            }
        }
        "lyrics_window" => {
            only(args, &["show"])?;
            match args.get("show").and_then(Value::as_bool) {
                Some(true) => ready(ToolCall::LyricsWindow { show: true }, false, "Show the lyrics"),
                Some(false) => ready(ToolCall::LyricsWindow { show: false }, false, "Hide the lyrics"),
                None => reject("show must be true or false"),
            }
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
            // T29.2: "play music" only ever starts; it is not the play/pause toggle.
            if args.get("op").and_then(Value::as_str) == Some("play") {
                return Ok(ready(ToolCall::MediaPlay, false, "Media"));
            }
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
        "text_transform" => {
            only(args, &["op", "lang", "text"])?;
            let op = super::textops::Op::parse(args.get("op").and_then(Value::as_str).unwrap_or_default()).ok_or_else(|| reject("unknown text operation"))?;
            let said = match args.get("text") {
                None | Some(Value::Null) => None,
                Some(_) => Some(text(args, "text", super::textops::MAX_SPOKEN_TEXT)?.to_string()),
            };
            // T29.1: "translate this text" points at the selection; the words are not what is translated.
            let said = said.filter(|t| !super::textops::is_selection_reference(t));
            let lang = match args.get("lang") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_str().filter(|l| super::textops::LANGS.contains(l)).ok_or_else(|| reject("unknown language"))?.to_string()),
            };
            if op == super::textops::Op::Translate && lang.is_none() {
                return Err(reject("translate needs a language"));
            }
            if op != super::textops::Op::Translate && lang.is_some() {
                return Err(reject("only translate takes a language"));
            }
            ready(ToolCall::TextTransform { op, lang, text: said }, false, format!("Text: {}", op.name()))
        }
        "remind" => {
            use crate::assistant::{Day, Local, When};
            only(args, &["text", "inMinutes", "day", "time", "at", "repeat"])?;
            let what = text(args, "text", crate::assistant::MAX_TEXT)?.to_string();
            let clock = |s: &str| -> Option<(u32, u32)> {
                let (h, m) = s.trim().split_once(':')?;
                let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
                (h < 24 && m < 60).then_some((h, m))
            };
            let repeat = match args.get("repeat") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_str().and_then(crate::assistant::Repeat::parse).ok_or_else(|| reject("repeat must be daily, weekdays or weekly"))?),
            };
            // A series needs a time of the day; a weekly one a weekday; a daily one and one of weekdays no day at all.
            if let Some(r) = repeat {
                if args.get("inMinutes").is_some() || args.get("at").is_some() || args.get("time").is_none() {
                    return Err(reject("a repeating reminder needs a time of the day"));
                }
                let day = args.get("day").and_then(Value::as_str).and_then(Day::parse);
                match (r, day) {
                    (crate::assistant::Repeat::Weekly, Some(Day::Weekday(_))) => {}
                    (crate::assistant::Repeat::Weekly, _) => return Err(reject("a weekly reminder needs a weekday")),
                    (_, None) if args.get("day").is_none() => {}
                    _ => return Err(reject("only a weekly reminder takes a day")),
                }
            }
            let when = match (args.get("inMinutes"), args.get("day"), args.get("time"), args.get("at")) {
                (None, None, Some(t), None) if repeat.is_some() => {
                    let (hour, minute) = t.as_str().and_then(clock).ok_or_else(|| reject("time must be HH:MM"))?;
                    When::Day { day: Day::Today, hour, minute }
                }
                (Some(m), None, None, None) => {
                    let m = m.as_u64().filter(|m| (1..=crate::assistant::MAX_AHEAD_MINUTES as u64).contains(m)).ok_or_else(|| reject("inMinutes must be 1..525600"))?;
                    When::InMinutes(m as u32)
                }
                (None, Some(d), Some(t), None) => {
                    let day = d.as_str().and_then(Day::parse).ok_or_else(|| reject("unknown day"))?;
                    let (hour, minute) = t.as_str().and_then(clock).ok_or_else(|| reject("time must be HH:MM"))?;
                    When::Day { day, hour, minute }
                }
                (None, None, None, Some(a)) => When::At(a.as_str().and_then(Local::parse_iso).ok_or_else(|| reject("at must be a local time YYYY-MM-DDTHH:MM"))?),
                _ => return Err(reject("a reminder needs exactly one of: inMinutes, day with time, at")),
            };
            ready(ToolCall::Remind { text: what, when, repeat }, false, "Set a reminder")
        }
        "memory_add" => {
            only(args, &["text"])?;
            ready(ToolCall::MemoryAdd { text: text(args, "text", super::memory::MAX_FACT_CHARS)?.to_string() }, false, "Remember")
        }
        "memory_remove" => {
            only(args, &["text"])?;
            let t = text(args, "text", super::memory::MAX_FACT_CHARS)?.to_string();
            ready_key(ToolCall::MemoryRemove { text: t.clone() }, true, format!("Forget: {t}"), "voice.confirm.forget", &[("text", &t)])
        }
        "day_brief" => {
            only(args, &[])?;
            ready(ToolCall::DayBrief, false, "Brief of the day")
        }
        "reminders_show" => {
            only(args, &[])?;
            ready(ToolCall::RemindersShow, false, "Show reminders")
        }
        "list_add" => {
            only(args, &["list", "item"])?;
            let (list, item) = (text(args, "list", crate::assistant::MAX_TEXT)?.to_string(), text(args, "item", crate::assistant::MAX_TEXT)?.to_string());
            ready(ToolCall::ListAdd { list, item }, false, "Add to a list")
        }
        "list_show" => {
            only(args, &["list"])?;
            ready(ToolCall::ListShow { list: text(args, "list", crate::assistant::MAX_TEXT)?.to_string() }, false, "Show a list")
        }
        "list_remove" => {
            only(args, &["list", "item"])?;
            let list = text(args, "list", crate::assistant::MAX_TEXT)?.to_string();
            match args.get("item") {
                None | Some(Value::Null) => {
                    let summary = format!("Delete the list {list}");
                    ready_key(ToolCall::ListRemove { list: list.clone(), item: None }, true, summary, "voice.confirm.list_delete", &[("name", &list)])
                }
                Some(_) => ready(ToolCall::ListRemove { list, item: Some(text(args, "item", crate::assistant::MAX_TEXT)?.to_string()) }, false, "Remove from a list"),
            }
        }
        "calc" => {
            only(args, &["expr"])?;
            let expr = text(args, "expr", super::calc::MAX_EXPR)?.to_string();
            if !super::calc::is_expression(&expr) {
                return Err(reject("that is not arithmetic"));
            }
            ready(ToolCall::Calc { expr }, false, "Calculate")
        }
        "ask" => {
            only(args, &["question"])?;
            ready(ToolCall::Ask { question: text(args, "question", super::ask::MAX_QUESTION)?.to_string() }, false, "Answer a question")
        }
        "screen_ask" => {
            only(args, &["question"])?;
            let question = text(args, "question", super::screen::MAX_QUESTION)?.to_string();
            if ctx.vision_ready {
                ready(ToolCall::ScreenAsk { question }, false, "Look at the screen")
            } else {
                // The package is optional and big: the user agrees to the download first (the answer to the question follows it).
                ready_key(ToolCall::ScreenAsk { question }, true, "Download the Screen help package (about 0.7 GB)?", "voice.confirm.screen_pack", &[])
            }
        }
        "screenshot" => {
            use crate::model::ShotArea;
            only(args, &["area"])?;
            let area = match args.get("area") {
                None | Some(Value::Null) => ShotArea::Screen,
                Some(v) => match v.as_str() {
                    Some("screen") => ShotArea::Screen,
                    Some("window") => ShotArea::Window,
                    Some("region") => ShotArea::Region,
                    _ => return Err(reject("unknown area")),
                },
            };
            ready(ToolCall::Screenshot(area), false, "Screenshot")
        }
        "add_button" => {
            use super::addbutton;
            only(args, &["catalogId", "name", "page"])?;
            let entries = addbutton::catalog(&ctx.lang);
            let entry = match args.get("catalogId") {
                Some(Value::String(id)) => entries.iter().find(|e| e.id == *id && e.is_button()).ok_or_else(|| reject("not a button of the catalog"))?,
                _ => {
                    let spoken = text(args, "name", 100)?;
                    match addbutton::find(&ctx.lang, spoken, &entries).as_slice() {
                        [] => return Err(reject("no such button in the catalog")),
                        [one] => one,
                        many => return Ok(ask(many.iter().map(|e| e.name.clone()).collect())),
                    }
                }
            };
            // A key that needs a URL, keys or a name is finished by hand: voice brings none of these.
            if entry.asks() {
                return Ok(ready(ToolCall::SetupButton { catalog_id: entry.id.clone() }, false, format!("Set up {}", entry.name)));
            }
            let page = match args.get("page") {
                None | Some(Value::Null) => ctx.current_page.as_ref().and_then(|id| ctx.pages.iter().find(|p| p.id == *id)).or_else(|| ctx.pages.first()),
                Some(_) => {
                    let name = text(args, "page", 100)?;
                    best(&ctx.lang, name, &ctx.pages, |p| p.name.as_str(), |p| p.id.clone()).first().map(|i| &ctx.pages[*i])
                }
            };
            let page = page.ok_or_else(|| reject("no such page"))?;
            let call = ToolCall::AddButton { catalog_id: entry.id.clone(), page_id: page.id.clone() };
            // Nothing to ask when there is no place: the executor says so.
            if ctx.full_pages.contains(&page.id) {
                return Ok(ready(call, false, addbutton::NO_ROOM));
            }
            let icon = entry.icon();
            ready_key(call, true, format!("Add {} to the page {}?", entry.name, page.name), "voice.confirm.add_button", &[("name", &entry.name), ("page", &page.name), ("label", &entry.label), ("icon", &icon)])
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
    #[test]
    fn m27_tools_take_only_their_arguments() {
        use super::super::screenhelp::UiAction;
        assert_eq!(ready_of(call("find_on_screen", json!({ "query": "save" }))).call, ToolCall::FindOnScreen { query: Some("save".into()) });
        assert_eq!(ready_of(call("find_on_screen", json!({ "next": true }))).call, ToolCall::FindOnScreen { query: None });
        assert!(is_reject(call("find_on_screen", json!({ "next": true, "query": "x" }))));
        assert!(is_reject(call("find_on_screen", json!({}))));
        assert!(is_reject(call("find_on_screen", json!({ "query": "x", "x": 1 }))));
        let v = ready_of(call("ui_act", json!({ "action": "click", "target": "this" })));
        assert_eq!((v.call.clone(), v.confirm), (ToolCall::UiAct { target: None, action: UiAction::Click, confirmed: false }, false));
        assert!(matches!(ready_of(call("ui_act", json!({ "action": "scroll_down" }))).call, ToolCall::UiAct { target: None, action: UiAction::ScrollDown, .. }));
        for bad in [json!({ "action": "drag" }), json!({ "target": "x" }), json!({ "action": "click", "path": "C:\\x" })] {
            assert!(is_reject(call("ui_act", bad.clone())), "{bad}");
        }
        assert_eq!(ready_of(call("guide", json!({ "goal": "save as pdf" }))).call, ToolCall::Guide { goal: "save as pdf".into() });
        assert!(is_reject(call("guide", json!({}))));
        assert_eq!(ready_of(call("guide_control", json!({ "kind": "stop" }))).call, ToolCall::GuideControl { kind: GuideCtl::Stop });
        assert!(is_reject(call("guide_control", json!({ "kind": "rm" }))));
        assert_eq!(ready_of(call("read_aloud", json!({}))).call, ToolCall::ReadAloud);
        assert!(is_reject(call("read_aloud", json!({ "text": "x" }))));
        assert_eq!(ready_of(call("mode_run", json!({ "name": "work" }))).call, ToolCall::ModeRun { name: "work".into() });
    }

    #[test]
    fn pressing_a_dangerous_name_waits_for_a_yes_and_runs_only_after_it() {
        use super::super::screenhelp::UiAction;
        for name in ["delete all", "Удалить", "send message", "оплатить заказ"] {
            let v = ready_of(call("ui_act", json!({ "action": "click", "target": name })));
            assert!(v.confirm, "{name} asks first");
            assert_eq!(v.call, ToolCall::UiAct { target: Some(name.into()), action: UiAction::Click, confirmed: true }, "the call is marked confirmed only for the run after the yes");
        }
        assert!(!ready_of(call("ui_act", json!({ "action": "click", "target": "save" }))).confirm);
        assert!(!ready_of(call("ui_act", json!({ "action": "scroll_down", "target": "delete" }))).confirm, "scrolling presses nothing");
    }

    #[test]
    fn remind_arguments_are_put_in_shape_before_validation() {
        use crate::assistant::Local;
        let now = Local::new(2026, 11, 26, 14, 0).unwrap();
        let obj = |v: serde_json::Value| v.as_object().unwrap().clone();
        // "at" with a clock time and a time with no day: today when ahead, tomorrow when passed.
        let a = normalize("remind", &obj(json!({ "text": "x", "at": "16:30" })), &now);
        assert_eq!((a.get("time"), a.get("day"), a.get("at")), (Some(&json!("16:30")), Some(&json!("today")), None));
        let b = normalize("remind", &obj(json!({ "text": "x", "time": "09:00" })), &now);
        assert_eq!(b.get("day"), Some(&json!("tomorrow")));
        // Not touched: another tool, a full ISO "at", a series, a given day, minutes.
        assert_eq!(normalize("timer", &obj(json!({ "minutes": 5 })), &now), obj(json!({ "minutes": 5 })));
        let iso = obj(json!({ "text": "x", "at": "2026-11-27T10:00" }));
        assert_eq!(normalize("remind", &iso, &now), iso);
        let series = obj(json!({ "text": "x", "time": "08:30", "repeat": "daily" }));
        assert_eq!(normalize("remind", &series, &now), series);
        let dated = obj(json!({ "text": "x", "day": "fri", "time": "09:00" }));
        assert_eq!(normalize("remind", &dated, &now), dated);
        // And it goes through validation as a reminder.
        assert!(matches!(call("remind", json!({ "text": "call", "at": "23:59" })), Resolved::Ready(_)));
    }

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
            vision_ready: false,
            memory: vec![],
        weather_place: None,
        guide_running: false,
        speech_interrupted: false,
        full_pages: vec![],
        current_page: None,
        }
    }

    /// T28.4: an ambiguous name asks which one, a dangerous one waits for "yes", and no tool of voice creates or changes
    /// an action (nothing of voice writes `actions.json`).
    #[test]
    fn a_button_by_voice_asks_confirms_and_never_writes_actions() {
        match call("open_app", json!({ "name": "chrome" })) {
            Resolved::Ask { options, .. } => assert_eq!(options.len(), 2, "{options:?}"),
            other => panic!("{other:?}"),
        }
        for name in ["Delete", "Send message", "Pay now", "Удалить", "Отправить", "Оплатить"] {
            let v = ready_of(call("ui_act", json!({ "action": "click", "target": name })));
            assert!(v.confirm, "{name}");
        }
        assert!(!ready_of(call("ui_act", json!({ "action": "click", "target": "Save" }))).confirm);
        for tool in ["create_button", "add_action", "save_actions", "write_actions"] {
            assert!(is_reject(call(tool, json!({ "id": "x", "name": "x" }))), "{tool}");
        }
        assert!(TOOLS.iter().all(|t| !t.contains("action") || *t == "run_action" || *t == "reply_action"), "no tool changes actions");
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
        // The white list only grows with a task of the roadmap (DATA_MODEL.md 12.5, 13): never with a tool that runs a command.
        assert!(TOOLS.len() >= 16 && TOOLS.contains(&"text_transform"));
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
    fn a_reminder_states_its_time_in_exactly_one_way_and_the_model_does_no_date_arithmetic() {
        use crate::assistant::{Day, When};
        let r = |args: Value| call("remind", args);
        assert_eq!(ready_of(r(json!({ "text": "call mom", "inMinutes": 15 }))).call, ToolCall::Remind { text: "call mom".into(), when: When::InMinutes(15), repeat: None });
        assert_eq!(ready_of(r(json!({ "text": "x", "day": "fri", "time": "08:15" }))).call, ToolCall::Remind { text: "x".into(), when: When::Day { day: Day::Weekday(4), hour: 8, minute: 15 }, repeat: None });
        assert!(matches!(ready_of(r(json!({ "text": "x", "at": "2026-12-01T09:00" }))).call, ToolCall::Remind { when: When::At(_), .. }));
        for bad in [
            json!({ "text": "x" }),
            json!({ "text": "x", "inMinutes": 0 }),
            json!({ "text": "x", "inMinutes": 525_601 }),
            json!({ "text": "x", "inMinutes": 5, "day": "today", "time": "10:00" }),
            json!({ "text": "x", "day": "today" }),
            json!({ "text": "x", "day": "someday", "time": "10:00" }),
            json!({ "text": "x", "day": "today", "time": "25:00" }),
            json!({ "text": "x", "at": "tomorrow" }),
            json!({ "text": "x", "at": "2026-12-01T09:00Z" }),
            json!({ "text": "", "inMinutes": 5 }),
            json!({ "text": "x".repeat(201), "inMinutes": 5 }),
            json!({ "text": "x", "inMinutes": 5, "extra": 1 }),
        ] {
            assert!(is_reject(r(bad.clone())), "{bad}");
        }
    }

    #[test]
    fn memory_is_added_by_name_and_forgotten_only_after_a_question() {
        assert_eq!(ready_of(call("memory_add", json!({ "text": "Aigerim is my wife" }))).call, ToolCall::MemoryAdd { text: "Aigerim is my wife".into() });
        let v = ready_of(call("memory_remove", json!({ "text": "Aigerim" })));
        assert!(v.confirm && v.key == Some("voice.confirm.forget") && v.call == ToolCall::MemoryRemove { text: "Aigerim".into() });
        assert!(is_reject(call("memory_add", json!({ "text": "x".repeat(201) }))) && is_reject(call("memory_add", json!({ "text": "" }))));
        assert!(is_reject(call("memory_remove", json!({ "id": "1" }))));
    }

    #[test]
    fn calc_and_ask_take_one_text_and_calc_only_arithmetic() {
        assert_eq!(ready_of(call("calc", json!({ "expr": "54 times 114" }))).call, ToolCall::Calc { expr: "54 times 114".into() });
        assert_eq!(ready_of(call("calc", json!({ "expr": "5 / 0" }))).call, ToolCall::Calc { expr: "5 / 0".into() }, "the answer says what is wrong");
        assert!(is_reject(call("calc", json!({ "expr": "open the calculator" }))));
        assert!(is_reject(call("calc", json!({ "expr": "1+1", "x": 1 }))));
        assert!(is_reject(call("calc", json!({ "expr": "1+".repeat(101) }))));
        let v = ready_of(call("ask", json!({ "question": "why is the sky blue" })));
        assert!(!v.confirm && v.call == ToolCall::Ask { question: "why is the sky blue".into() });
        assert!(is_reject(call("ask", json!({ "question": "x".repeat(301) }))));
        assert!(is_reject(call("ask", json!({}))));
    }

    #[test]
    fn a_series_takes_a_time_of_the_day_and_a_weekly_one_a_weekday() {
        use crate::assistant::{Day, Repeat, When};
        let r = |v: serde_json::Value| call("remind", v);
        let daily = ready_of(r(json!({ "text": "pills", "repeat": "daily", "time": "09:00" })));
        assert_eq!(daily.call, ToolCall::Remind { text: "pills".into(), when: When::Day { day: Day::Today, hour: 9, minute: 0 }, repeat: Some(Repeat::Daily) });
        assert!(matches!(ready_of(r(json!({ "text": "x", "repeat": "weekdays", "time": "08:30" }))).call, ToolCall::Remind { repeat: Some(Repeat::Weekdays), .. }));
        assert_eq!(
            ready_of(r(json!({ "text": "gym", "repeat": "weekly", "day": "mon", "time": "07:00" }))).call,
            ToolCall::Remind { text: "gym".into(), when: When::Day { day: Day::Weekday(0), hour: 7, minute: 0 }, repeat: Some(Repeat::Weekly) }
        );
        for bad in [
            json!({ "text": "x", "repeat": "monthly", "time": "09:00" }),
            json!({ "text": "x", "repeat": "daily" }),
            json!({ "text": "x", "repeat": "daily", "inMinutes": 5 }),
            json!({ "text": "x", "repeat": "daily", "at": "2026-12-01T09:00" }),
            json!({ "text": "x", "repeat": "weekly", "time": "09:00" }),
            json!({ "text": "x", "repeat": "weekly", "day": "tomorrow", "time": "09:00" }),
            json!({ "text": "x", "repeat": "daily", "day": "mon", "time": "09:00" }),
        ] {
            assert!(is_reject(r(bad.clone())), "{bad}");
        }
    }

    #[test]
    fn the_day_brief_takes_nothing() {
        assert_eq!(ready_of(call("day_brief", json!({}))).call, ToolCall::DayBrief);
        assert!(is_reject(call("day_brief", json!({ "city": "Almaty" }))));
    }

    #[test]
    fn list_tools_take_names_and_only_deleting_a_whole_list_asks() {
        assert_eq!(ready_of(call("list_add", json!({ "list": "shopping", "item": "milk" }))).call, ToolCall::ListAdd { list: "shopping".into(), item: "milk".into() });
        assert_eq!(ready_of(call("list_show", json!({ "list": "shopping" }))).call, ToolCall::ListShow { list: "shopping".into() });
        let one = ready_of(call("list_remove", json!({ "list": "shopping", "item": "milk" })));
        assert!(!one.confirm && one.call == ToolCall::ListRemove { list: "shopping".into(), item: Some("milk".into()) });
        let all = ready_of(call("list_remove", json!({ "list": "shopping" })));
        assert!(all.confirm && all.key == Some("voice.confirm.list_delete") && all.call == ToolCall::ListRemove { list: "shopping".into(), item: None });
        assert_eq!(ready_of(call("reminders_show", json!({}))).call, ToolCall::RemindersShow);
        assert!(is_reject(call("list_add", json!({ "list": "shopping" }))));
        assert!(is_reject(call("list_show", json!({ "list": "shopping", "id": "1" }))), "ids are the agent's, never named by the model");
        assert!(is_reject(call("reminders_show", json!({ "all": true }))));
    }

    #[test]
    fn screen_ask_asks_for_the_package_first_and_looks_at_the_window_only_when_it_is_there() {
        let with = |ready: bool, args: Value| {
            let mut c = ctx();
            c.vision_ready = ready;
            validate(&RawCall { tool: "screen_ask".into(), args }, &c)
        };
        let v = ready_of(with(false, json!({ "question": "what is this error" })));
        assert!(v.confirm && v.key == Some("voice.confirm.screen_pack"), "the download needs the consent of the user");
        let v = ready_of(with(true, json!({ "question": "what is this error" })));
        assert!(!v.confirm);
        assert_eq!(v.call, ToolCall::ScreenAsk { question: "what is this error".into() });
        assert!(is_reject(with(true, json!({ "question": "" }))));
        assert!(is_reject(with(true, json!({ "question": "x".repeat(301) }))));
        assert!(is_reject(with(true, json!({ "question": "a", "window": "b" }))), "no field to choose a window");
    }

    /// T29.6: a catalog key becomes a draft that waits for "yes"; a key that asks goes to the settings; nothing else is made.
    #[test]
    fn add_button_takes_catalog_keys_only_and_waits_for_yes() {
        let c = Context { lang: "ru".into(), pages: vec![PageRef { id: "main".into(), name: "Главная".into() }, PageRef { id: "media".into(), name: "Медиа".into() }], ..Default::default() };
        let add = |args: serde_json::Value| validate(&RawCall { tool: "add_button".into(), args }, &c);
        let r = ready_of(add(json!({ "name": "блокировки" })));
        assert!(r.confirm);
        assert_eq!(r.call, ToolCall::AddButton { catalog_id: "win.lock".into(), page_id: "main".into() });
        assert_eq!((r.key, r.args.get("icon").map(String::as_str), r.args.get("page").map(String::as_str)), (Some("voice.confirm.add_button"), Some("lucide:lock"), Some("Главная")));
        let r = ready_of(add(json!({ "catalogId": "win.lock", "page": "медиа" })));
        assert_eq!(r.call, ToolCall::AddButton { catalog_id: "win.lock".into(), page_id: "media".into() });
        // A key that asks for a URL is finished in the settings, without a confirmation.
        let r = ready_of(add(json!({ "catalogId": "web.site" })));
        assert_eq!((r.confirm, r.call), (false, ToolCall::SetupButton { catalog_id: "web.site".into() }));
        for bad in [json!({ "catalogId": "nope" }), json!({ "catalogId": "widget.clock" }), json!({ "name": "формат диска" }), json!({ "catalogId": "win.lock", "url": "x" }), json!({ "catalogId": "win.lock", "page": "нет такой" })] {
            assert!(is_reject(add(bad.clone())), "{bad}");
        }
        // A full page: no question, the executor answers "No space on the page".
        let full = Context { full_pages: vec!["main".into()], ..c.clone() };
        let r = ready_of(validate(&RawCall { tool: "add_button".into(), args: json!({ "catalogId": "win.lock" }) }, &full));
        assert!(!r.confirm);
        // The page the voice went to last is the one.
        let there = Context { current_page: Some("media".into()), ..c };
        let r = ready_of(validate(&RawCall { tool: "add_button".into(), args: json!({ "catalogId": "win.lock" }) }, &there));
        assert_eq!(r.call, ToolCall::AddButton { catalog_id: "win.lock".into(), page_id: "media".into() });
    }

    #[test]
    fn text_transform_takes_one_of_eight_operations_and_a_language_only_for_translate() {
        use crate::voice::textops::Op;
        for op in ["polite", "shorter", "fix", "formal", "casual", "summarize", "explain"] {
            let r = ready_of(call("text_transform", json!({ "op": op })));
            assert!(matches!(r.call, ToolCall::TextTransform { lang: None, .. }) && !r.confirm, "{op}");
        }
        assert_eq!(ready_of(call("text_transform", json!({ "op": "translate", "lang": "de" }))).call, ToolCall::TextTransform { op: Op::Translate, lang: Some("de".into()), text: None });
        // T26.2: said text (<= 500 characters), and the reply operation.
        assert_eq!(
            ready_of(call("text_transform", json!({ "op": "translate", "lang": "zh", "text": "спасибо за помощь" }))).call,
            ToolCall::TextTransform { op: Op::Translate, lang: Some("zh".into()), text: Some("спасибо за помощь".into()) }
        );
        assert!(matches!(ready_of(call("text_transform", json!({ "op": "reply" }))).call, ToolCall::TextTransform { op: Op::Reply, .. }));
        assert!(is_reject(call("text_transform", json!({ "op": "translate", "lang": "zh", "text": "a".repeat(501) }))));
        assert!(is_reject(call("text_transform", json!({ "op": "reply", "lang": "de" }))));
        assert!(is_reject(call("text_transform", json!({ "op": "translate" }))), "translate needs a language");
        assert!(is_reject(call("text_transform", json!({ "op": "fix", "lang": "de" }))));
        assert!(is_reject(call("text_transform", json!({ "op": "translate", "lang": "xx" }))));
        assert!(is_reject(call("text_transform", json!({ "op": "delete" }))));
        assert!(is_reject(call("text_transform", json!({ "op": "fix", "extra": 1 }))));
    }

    #[test]
    fn monitor_and_dictation_tools_take_no_arguments() {
        assert_eq!(ready_of(call("which_monitor", Value::Null)).call, ToolCall::WhichMonitor);
        assert_eq!(ready_of(call("lyrics_window", json!({ "show": false }))).call, ToolCall::LyricsWindow { show: false });
        assert_eq!(ready_of(call("lyrics_window", json!({ "show": true }))).call, ToolCall::LyricsWindow { show: true });
        for bad in [json!({}), json!({ "show": "no" }), json!({ "show": false, "x": 1 })] {
            assert!(matches!(call("lyrics_window", bad.clone()), Resolved::Reject(_)), "{bad}");
        }
        for (k, want) in [("copy", super::super::followup::Kind::Copy), ("insert", super::super::followup::Kind::Insert), ("speak", super::super::followup::Kind::Speak), ("again", super::super::followup::Kind::Again)] {
            assert_eq!(ready_of(call("reply_action", json!({ "kind": k }))).call, ToolCall::ReplyAction { kind: want });
        }
        for bad in [json!({}), json!({ "kind": "run" }), json!({ "kind": "lang:de" }), json!({ "kind": "copy", "x": 1 })] {
            assert!(matches!(call("reply_action", bad.clone()), Resolved::Reject(_)), "{bad}");
        }
        assert_eq!(ready_of(call("status", json!({ "topic": "ram" }))).call, ToolCall::Status { topic: super::super::status::Topic::Ram });
        for bad in [json!({}), json!({ "topic": "internet" }), json!({ "topic": "cpu", "x": 1 })] {
            assert!(matches!(call("status", bad.clone()), Resolved::Reject(_)), "{bad}");
        }
        assert_eq!(ready_of(call("window_other_monitor", json!({}))).call, ToolCall::WindowOtherMonitor);
        assert_eq!(ready_of(call("dictate", json!({}))).call, ToolCall::Dictate);
    }
}
