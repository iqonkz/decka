//! Voice control inside the running agent (M21 C2): connects the pipeline (`pipeline::Driver`) to the things that exist
//! only there — the microphone, the hotkey, whisper, the action runners, the `voice` section of the state and the pill.
//! The decisions (`press_event`, `pill_cmds`, `tool_action`, `build_context`, `pill_payload`) are pure functions with tests;
//! `Host` is the glue. The window of the pill is created by the application through the `PillHook`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::check::{CheckPorts, CheckStep, Recording, StepError};
use super::dictation::{self, DictationSettings, Mode, Trigger};
use super::target::{self, FocusMemory, Route};
use super::endpoint::Verdict;
use super::wake_host::{Surroundings, Wake, WakeDeps};
use super::hotkey::{self, Pressed, Plan, Tracker};
use super::matcher::Table;
use super::mic;
use super::pipeline::{key, msg, BoxFut, Driver, Ear, Event, Phase, Ports, Source, State, VoiceInfo};
use super::winops;
use super::tools::{ActionRef, AppRef, ButtonRef, Context, PageRef, RawCall, ToolCall, VolumeOp};
use crate::agent::Agent;
use crate::ai::failure::{Reason, Server};
use crate::ai::service::{VoiceService, VoiceSetArgs, VoiceState, VoiceStatus};
use crate::ai::supervisor::SupState;
use crate::installed::InstalledApp;
use crate::model::{Action, ActionsFile, LayoutFile, LayoutItem, MediaOp, VarValue};

/// A finished request stays on the screen this long, then the pipeline returns to idle and the pill goes away.
pub const RESULT_SHOWN_MS: u64 = 4_000;
/// A request that does not change its state for this long is cancelled (DATA_MODEL.md 12.6a).
pub const WATCHDOG_MS: u64 = 30_000;

/// How long one stage may stay unchanged before the watchdog gives up on it; the clock restarts at every change of stage.
pub fn watchdog_ms(phase: &Phase) -> u64 {
    match phase {
        Phase::Listening => 30_000,
        // Two starts of the model (on the card, then on the processor, T28.1) of up to `READY_TIMEOUT` each.
        Phase::Loading => 250_000,
        // The request has its own limit (15 s on a GPU, up to 60 s on a CPU, `supervisor::stt_timeout`): this is the backstop.
        Phase::Transcribing => 61_000,
        Phase::Thinking => 20_000,
        _ => WATCHDOG_MS,
    }
}
/// An opened microphone that gives no chunk for this long is unavailable.
pub const MIC_SILENT_MS: u64 = 1_000;
/// The decisions of this many requests after the start are written to `voice.log`.
const LOGGED_FIRST: usize = 5;
/// A press or a click that arrives sooner than this after the start of a PC recording cannot end it.
const MIN_RECORDING_MS: u128 = 400;
static PC_STARTED: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

/// The window of the pill belongs to the main thread and takes seconds to appear: the pipeline (and the microphone) must
/// not wait for it. One worker keeps ALL commands of the pill (states and waves) in order.
fn pill_worker(job: Box<dyn FnOnce() + Send>) {
    static WORKER: std::sync::OnceLock<std::sync::Mutex<std::sync::mpsc::Sender<Box<dyn FnOnce() + Send>>>> = std::sync::OnceLock::new();
    let tx = WORKER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Box<dyn FnOnce() + Send>>();
        std::thread::spawn(move || {
            while let Ok(job) = rx.recv() {
                job();
            }
        });
        std::sync::Mutex::new(tx)
    });
    let _ = tx.lock().unwrap().send(job);
}

/// A wave update waits in the queue of the pill while the window is created; one waiting update is enough.
static WAVE_PENDING: AtomicBool = AtomicBool::new(false);

/// Wave of the microphone for the pill. It goes through the same queue as the states and is checked when it RUNS: a wave
/// that overtook the `Destroy` of its request used to bring the pill back, stuck in `Listening…` (0.22.0).
fn send_wave(current: impl FnOnce() -> State + Send + 'static, pill: PillHook, cmd: PillCmd, id: Option<String>) {
    if WAVE_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    pill_worker(Box::new(move || {
        WAVE_PENDING.store(false, Ordering::SeqCst);
        let s = current();
        if s.phase == Phase::Listening && s.id == id {
            pill(cmd);
        }
    }));
}

fn too_early() -> bool {
    PC_STARTED.lock().unwrap().is_some_and(|t| t.elapsed().as_millis() < MIN_RECORDING_MS)
}
static LOGGED_REQUESTS: AtomicUsize = AtomicUsize::new(0);

// ---- Pure decisions ---------------------------------------------------------------------------------------------

/// What the application does with the pill window.
#[derive(Debug, Clone, PartialEq)]
pub enum PillCmd {
    /// Create the window if it is not there, then send the payload (event `voice-pill`).
    Show(Value),
    Destroy,
}

/// The payload of the event `voice-pill`: the `voice` section plus the level of the microphone and the chord.
pub fn pill_payload(info: &VoiceInfo, hotkey: &[String], level: f32) -> Value {
    let mut v = serde_json::to_value(info).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.insert("level".into(), json!((level as f64 * 100.0).round() / 100.0));
        o.insert("hotkey".into(), json!(hotkey::format_chord(hotkey)));
        // T26.3: the actions under a shown answer (the window of the PC only).
        o.insert("actions".into(), json!(info.actions));
        // T28.3: the stages of a command, "Heard" -> "Understood" -> "Doing it" (the window of the PC only).
        if let Some(u) = &info.understood {
            o.insert("understood".into(), json!(u));
        }
        if info.executing {
            o.insert("executing".into(), json!(true));
        }
    }
    v
}

/// The pill belongs to requests of the PC; a phone shows its own sheet. `idle` takes the window away.
pub fn pill_cmd(info: &VoiceInfo, hotkey: &[String], level: f32) -> Option<PillCmd> {
    match (info.state, info.source) {
        ("idle", _) => Some(PillCmd::Destroy),
        (_, Some(Source::Pc)) => Some(PillCmd::Show(pill_payload(info, hotkey, level))),
        _ => None,
    }
}

fn is_active(p: &Phase) -> bool {
    matches!(p, Phase::Listening | Phase::Transcribing | Phase::Loading | Phase::Thinking | Phase::Executing)
}

/// The hotkey: start, or end the recording, or cancel what is thinking. `id` is used for a new request.
pub fn press_event(state: &State, id: String) -> Option<Event> {
    match (&state.phase, state.source) {
        (Phase::Idle | Phase::Done(_) | Phase::Failed(_), _) => Some(Event::Start { id, source: Source::Pc }),
        (Phase::Listening | Phase::Confirming { ear: Ear::Open, .. }, Some(Source::Pc)) => Some(Event::AudioEnd),
        (Phase::Transcribing | Phase::Loading | Phase::Thinking, _) => Some(Event::Cancel),
        _ => None,
    }
}

/// Esc: `No` while a confirmation is open, else cancel.
pub fn esc_event(state: &State) -> Option<Event> {
    match (&state.phase, &state.id) {
        (Phase::Idle, _) => None,
        (Phase::Confirming { .. }, Some(id)) => Some(Event::Confirm { id: id.clone(), yes: false }),
        _ => Some(Event::Cancel),
    }
}

/// Enter: `Yes` while a confirmation is open.
pub fn enter_event(state: &State) -> Option<Event> {
    match (&state.phase, &state.id) {
        (Phase::Confirming { .. }, Some(id)) => Some(Event::Confirm { id: id.clone(), yes: true }),
        _ => None,
    }
}

/// `(esc, enter)` to catch globally now: only for a request of the PC (a phone has its own buttons).
pub fn aux_for(state: &State) -> (bool, bool) {
    if state.source != Some(Source::Pc) {
        return (false, false);
    }
    hotkey::aux_keys(is_active(&state.phase), matches!(state.phase, Phase::Confirming { .. }))
}

/// A validated call as an `Action` of the existing executors, with the text for the user. `RunAction` is not here: it goes
/// through `Agent::execute`. Tools without an executor yet are an error.
pub fn tool_action(call: &ToolCall) -> Result<(Action, String), String> {
    let media = |op: MediaOp, text: &str| Ok((Action::Media { name: "voice".into(), toggle_var: None, op }, text.to_string()));
    match call {
        ToolCall::OpenApp { name, program } => Ok((
            Action::Launch { name: name.clone(), toggle_var: None, program: program.clone(), args: None, cwd: None, hidden: None },
            format!("Opened {name}"),
        )),
        ToolCall::OpenUrl { url } => Ok((Action::OpenUrl { name: "voice".into(), toggle_var: None, url: url.clone() }, String::new())),
        ToolCall::System(op) => Ok((Action::System { name: "voice".into(), toggle_var: None, op: *op }, String::new())),
        ToolCall::Media(op) => media(*op, ""),
        ToolCall::Volume(VolumeOp::Up) => media(MediaOp::VolUp, ""),
        ToolCall::Volume(VolumeOp::Down) => media(MediaOp::VolDown, ""),
        ToolCall::Volume(VolumeOp::Mute) => media(MediaOp::Mute, ""),
        ToolCall::Screenshot(area) => {
            let (key, text) = match area {
                crate::model::ShotArea::Region => ("voice.reply.shot_area", "Select the area with the mouse"),
                _ => ("voice.reply.screenshot", "Screenshot copied and saved"),
            };
            Ok((Action::Screenshot { name: "voice".into(), toggle_var: None, area: *area }, super::status::encode(&super::status::Answer { key, args: Default::default(), text: text.into() })))
        }
        // The rest has no action of its own: `Ports::execute` runs it through `winops`, the timer and the host.
        _ => Err("not an action of its own".into()),
    }
}

/// How long "dictate" waits for the phrase to type; later the next phrase is an ordinary command again.
pub const DICTATE_WINDOW: Duration = Duration::from_secs(60);

/// Whether a dictation armed until `until` is still waiting at `now`.
pub fn dictation_waiting(until: Option<Instant>, now: Instant) -> bool {
    until.is_some_and(|u| now < u)
}

/// A long dictation goes in through the clipboard; the old text of the clipboard comes back after a second (13.1).
#[cfg(windows)]
fn paste_system(text: &str) -> Result<String, String> {
    use super::clipboard::{paste_text, SystemClipboard, SystemKeys};
    paste_text(&SystemClipboard, &SystemKeys, text, None, dictation::CLIPBOARD_RESTORE, &std::thread::sleep).map(|_| String::new())
}

#[cfg(not(windows))]
fn paste_system(_: &str) -> Result<String, String> {
    Err("unsupported platform".into())
}

/// T29.1: a dictated or inserted text goes where the user was; into the new place only when that is a field; else to the
/// clipboard, and the answer says so.
fn deliver(focus: &FocusMemory, text: &str, paste: bool) -> Result<String, String> {
    let before = focus.remembered();
    let now = focus.current();
    let route = target::route(before.as_ref(), now.as_ref());
    // The kind of the place only, never what is in it.
    mic::log_line(&format!(
        "deliver {route:?}: field={:?} same={} class={}",
        now.as_ref().map(|n| n.edit),
        before.as_ref().zip(now.as_ref()).is_some_and(|(b, n)| b.same(n)),
        now.as_ref().map(|n| target::window_class(n.window)).unwrap_or_default(),
    ));
    match route {
        Route::Clipboard => copy_to_clipboard(text).map(|_| super::pipeline::msg::COPIED_PASTE.to_string()),
        Route::Field if paste || now.is_some_and(|n| target::typing_is_lossy(&target::window_class(n.window))) => paste_system(text),
        Route::Field => winops::type_text(text).map(|_| String::new()),
    }
}

/// T30.2: the list the island shows, and when it was made.
struct HeavyList {
    at: Instant,
    list: Vec<super::heavy::Heavy>,
    /// The island still shows it (a button of it was not pressed).
    shown: bool,
}

/// The island shows the list this long; "close <name>" means one of its programs for longer.
const HEAVY_SHOWN: Duration = Duration::from_secs(20);
const HEAVY_FRESH: Duration = Duration::from_secs(120);

fn fresh_heavy(list: &Mutex<Option<HeavyList>>) -> Vec<super::heavy::Heavy> {
    list.lock().unwrap().as_ref().filter(|l| l.at.elapsed() < HEAVY_FRESH).map(|l| l.list.clone()).unwrap_or_default()
}

/// M30 (T30.1): what the meeting notes run on.
struct NoteModels {
    agent: Agent,
    service: VoiceService,
}

/// A summary of a piece of a meeting may take long on a processor.
const NOTE_TIMEOUT: Duration = Duration::from_secs(180);

impl crate::notes::Backend for NoteModels {
    fn available(&self) -> Result<(), String> {
        if is_on(&self.service.status()) { Ok(()) } else { Err(crate::notes::NOT_READY.into()) }
    }

    fn live(&self) -> bool {
        self.service.recognition_on_gpu()
    }

    fn language(&self) -> String {
        crate::tray_text::agent_language(self.agent.layout().display.language.as_deref()).to_string()
    }

    fn microphone(&self) -> Option<String> {
        self.service.microphone()
    }

    fn transcribe(&self, pcm: Vec<u8>, language: String) -> crate::notes::BoxFut<Result<String, String>> {
        let sup = self.service.supervisor().clone();
        Box::pin(async move { sup.transcribe_long(&pcm, &language).await })
    }

    fn model(&self, body: Value) -> crate::notes::BoxFut<Option<String>> {
        let sup = self.service.supervisor().clone();
        Box::pin(async move { super::assist::chat_text(&sup, &body, NOTE_TIMEOUT).await })
    }
}

fn answer(key: &'static str, name: Option<&str>, text: &str) -> super::status::Answer {
    super::status::Answer { key, args: name.map(|n| ("name".to_string(), n.to_string())).into_iter().collect(), text: text.into() }
}

const WARN_OTHERS: &str = "Recording. Tell the others that the meeting is being recorded";

/// What the island says about a note; a start by voice has its own answer (the island is busy then and shows nothing here).
fn note_notice(event: &crate::notes::Event, warned: bool) -> Option<super::status::Answer> {
    use crate::notes::{Event, StopReason};
    Some(match event {
        Event::Started if !warned => answer("voice.reply.note_first", None, WARN_OTHERS),
        Event::Started | Event::Stopped(StopReason::User) => return None,
        Event::Stopped(StopReason::TimeLimit) => answer("voice.reply.note_limit", None, "3 hours: the note stopped and is being saved"),
        Event::Stopped(StopReason::DiskFull) => answer("voice.reply.note_disk", None, "The disk is almost full: the note stopped and is being saved"),
        Event::Saved { title, summarized: true, .. } => answer("voice.reply.note_saved", Some(title), &format!("Note saved: {title}")),
        Event::Saved { title, .. } => answer("voice.reply.note_saved_plain", Some(title), &format!("Note saved without a summary: {title}")),
        Event::Empty => answer("voice.reply.note_empty", None, "Nothing was said: no note"),
        Event::Failed(_) => answer("voice.reply.note_failed", None, "The note could not be saved"),
    })
}

/// `day_brief` (13.5): the facts the agent already has, put into words by the model; the plain facts when the model fails the check.
async fn day_brief(agent: &Agent, sup: &crate::ai::supervisor::Supervisor, lang: &str) -> Result<String, String> {
    use super::brief;
    let store = agent.store();
    let (assistant, now) = (agent.assistant(), crate::assistant::Local::now());
    let lists = assistant.lists();
    let facts = brief::facts(&store.get("weather"), &store.get("mail"), &store.get("ai"), &assistant.upcoming(50), &lists, &now);
    let facts = brief::with_events(facts, &store.get("calendar"), &now, crate::auth::now_ms() as i64);
    let Some(facts) = facts else { return Ok(brief::NOTHING.into()) };
    let body = brief::request_body(&facts, lang);
    let answer = super::assist::chat_text(sup, &body, brief::BRIEF_TIMEOUT).await;
    Ok(answer.and_then(|a| brief::check_brief(&facts, &a)).unwrap_or_else(|| brief::facts_text(&facts, &lists)))
}

/// The Screen help package, downloaded once with the consent given in the confirmation; `download` holds the percent while
/// it comes, for the island (T29.2). The same "yes" switches Screen help on: without it the model runs without the
/// projector (T29.P2).
async fn screen_pack(agent: &Agent, service: &VoiceService, download: &Mutex<Option<u8>>) -> Result<(), String> {
    if !agent.voice_context_enabled() {
        agent.set_voice_context_enabled(true)?;
    }
    if !service.vision_installed() {
        let result = service.install_vision_with(&mut |p| *download.lock().unwrap() = Some(p)).await;
        *download.lock().unwrap() = None;
        result?;
    }
    service.apply_screen_help().await;
    Ok(())
}

/// `screen_ask` (13.3): a picture of the active window in memory, one question to the model on this PC.
async fn screen_ask(sup: &crate::ai::supervisor::Supervisor, lang: &str, question: &str) -> Result<String, String> {
    // The picture is taken after the download and the restart of the server, so that it is the window the user is looking at now.
    let data_url = tokio::task::spawn_blocking(super::screen::capture_data_url).await.map_err(|e| format!("The action did not finish: {e}"))??;
    let body = super::screen::request_body(question, lang, &data_url);
    drop(data_url);
    let answer = super::assist::chat_text(sup, &body, super::screen::ASK_TIMEOUT).await;
    answer.as_deref().and_then(super::screen::accept_answer).ok_or_else(|| super::textops::NO_ANSWER.to_string())
}

/// What a text operation made: the text to show (empty when the result replaced the selection) and the text it worked on.
struct Transformed {
    reply: String,
    source: String,
}

/// Where the text of an operation comes from.
enum TextSource<'a> {
    /// The selection of the focused program (UI Automation first, then `Ctrl+C`).
    Selection,
    /// A text that was said or carried over from the last answer: nothing is captured, the answer is only shown.
    Given(&'a str),
}

/// `text_transform` (13.2): the text is taken, rewritten by the model and pasted back (or shown).
#[cfg(windows)]
async fn transform_text(sup: &crate::ai::supervisor::Supervisor, focus: &FocusMemory, lang: &str, op: super::textops::Op, target_lang: Option<&str>, source: TextSource<'_>, temperature: f64) -> Result<Transformed, String> {
    use super::clipboard::{SystemClipboard, SystemKeys};
    use super::textops;
    // A text that was said (T26.2) or continued (T26.3): no capture, the answer is only shown.
    if let TextSource::Given(text) = source {
        let grabbed = textops::source_text(text).ok_or_else(|| textops::SELECT_FIRST.to_string())?;
        let body = textops::request_body_at(op, target_lang, lang, &grabbed.text, temperature);
        let answer = super::assist::chat_text(sup, &body, textops::TRANSFORM_TIMEOUT).await;
        return Ok(Transformed { reply: textops::finish_shown(answer.as_deref())?, source: grabbed.text });
    }
    // T29.1: from where the user was when the request began. UI Automation first: the clipboard stays untouched when the
    // program gives its selection that way.
    let memory = focus.clone();
    let taken = tokio::task::spawn_blocking(move || {
        let before = memory.remembered();
        target::take_selection(&memory, before.as_ref(), &SystemClipboard, &SystemKeys, &std::thread::sleep).map(|g| (g, before))
    })
    .await
    .map_err(|e| format!("The action did not finish: {e}"))?;
    // T32.H.1: every step says what it did, never the text.
    let (grabbed, before) = match taken {
        Ok(taken) => taken,
        Err(e) => {
            mic::log_line(&format!("transform {}: selection failed: {e}", op.name()));
            return Err(e);
        }
    };
    mic::log_line(&format!("transform {}: selection chars={} by={}", op.name(), grabbed.text.chars().count(), if grabbed.previous.is_some() { "copy" } else { "uia" }));
    let body = textops::request_body_at(op, target_lang, lang, &grabbed.text, temperature);
    let answer = super::assist::chat_text(sup, &body, textops::TRANSFORM_TIMEOUT).await;
    mic::log_line(&format!("transform {}: model answer chars={:?}", op.name(), answer.as_ref().map(|a| a.chars().count())));
    let source = grabbed.text.clone();
    let memory = focus.clone();
    let reply = tokio::task::spawn_blocking(move || {
        if !op.replaces() {
            return textops::finish(&SystemClipboard, &SystemKeys, op, &grabbed, answer.as_deref(), &std::thread::sleep);
        }
        let now = memory.current();
        let route = target::route_replace(before.as_ref(), now.as_ref());
        mic::log_line(&format!("transform {}: route={route:?} field={:?}", op.name(), now.as_ref().map(|n| n.edit)));
        if route == Route::Clipboard {
            return textops::leave_on_clipboard(&SystemClipboard, op, &grabbed, answer.as_deref()).map(|_| textops::PASTE_FAILED.to_string());
        }
        let Some(new) = answer.as_deref().and_then(|a| textops::accept_transform(op, &grabbed.text, a)) else {
            mic::log_line(&format!("transform {}: the answer of the model cannot be used", op.name()));
            super::clipboard::give_back(&SystemClipboard, &grabbed);
            return Err(textops::NO_ANSWER.to_string());
        };
        let done = textops::replace_selection(&SystemClipboard, &SystemKeys, &new, grabbed.previous.clone(), &super::uia::focused_text, &std::thread::sleep);
        mic::log_line(&format!("transform {}: paste {done:?}", op.name()));
        Ok(match done? {
            textops::Replaced::Yes => String::new(),
            textops::Replaced::No => textops::PASTE_FAILED.to_string(),
            textops::Replaced::Unknown => textops::PASTE_UNCHECKED.to_string(),
        })
    })
    .await
    .map_err(|e| format!("The action did not finish: {e}"))??;
    Ok(Transformed { reply, source })
}

#[cfg(not(windows))]
async fn transform_text(_: &crate::ai::supervisor::Supervisor, _: &FocusMemory, _: &str, _: super::textops::Op, _: Option<&str>, _: TextSource<'_>, _: f64) -> Result<Transformed, String> {
    Err("unsupported platform".into())
}

/// The temperature of "another variant": different words, same meaning.
const VARIANT_TEMPERATURE: f64 = 0.8;

type Answers = Arc<Mutex<super::followup::Memory>>;

/// A shown answer becomes the context of the next phrase; an answer that replaced the selection ends it.
fn remember(answers: &Answers, op: super::textops::Op, target: Option<&str>, ui_lang: &str, done: &Transformed) {
    let mut memory = answers.lock().unwrap();
    if done.reply.is_empty() || [super::pipeline::msg::COPIED_PASTE, super::textops::PASTE_FAILED, super::textops::PASTE_UNCHECKED].contains(&done.reply.as_str()) {
        memory.clear();
    } else {
        let answer = super::followup::Answer {
            tool: "text_transform",
            op,
            lang: target.map(str::to_string),
            source_text: done.source.clone(),
            result: done.reply.clone(),
            ui_lang: ui_lang.to_string(),
        };
        memory.set(answer, Instant::now());
    }
}

/// `text_transform` of a call: a rewrite right after an answer continues that answer (T26.3).
async fn run_transform(
    sup: &crate::ai::supervisor::Supervisor,
    focus: &FocusMemory,
    answers: &Answers,
    lang: &str,
    op: super::textops::Op,
    target: Option<&str>,
    said: Option<&str>,
) -> Result<String, String> {
    let carried = if said.is_none() { answers.lock().unwrap().fresh(Instant::now()).and_then(|a| super::followup::continued_text(a, op)) } else { None };
    let given = said.map(str::to_string).or(carried);
    let source = given.as_deref().map_or(TextSource::Selection, TextSource::Given);
    let done = transform_text(sup, focus, lang, op, target, source, 0.2).await?;
    remember(answers, op, target, lang, &done);
    Ok(done.reply)
}

/// What to do with the answer that was just shown (T26.3), for a phrase ("insert it") and for the buttons of the PC window.
async fn reply_action(sup: &crate::ai::supervisor::Supervisor, focus: &FocusMemory, answers: &Answers, kind: &super::followup::Kind) -> Result<String, String> {
    use super::followup::Kind;
    use super::textops;
    let Some(answer) = answers.lock().unwrap().fresh(Instant::now()).cloned() else {
        return Err(textops::NO_CONTEXT.to_string());
    };
    let blocking = |e: tokio::task::JoinError| format!("The action did not finish: {e}");
    match kind {
        Kind::Copy => {
            let text = answer.result;
            tokio::task::spawn_blocking(move || copy_to_clipboard(&text)).await.map_err(blocking)??;
            Ok(String::new())
        }
        Kind::Insert => {
            let (text, focus) = (answer.result, focus.clone());
            tokio::task::spawn_blocking(move || deliver(&focus, &text, true)).await.map_err(blocking)?
        }
        Kind::Speak => {
            let text = answer.result;
            tokio::task::spawn_blocking(move || super::speak::speak(&text)).await.map_err(blocking)??;
            Ok(String::new())
        }
        Kind::Again => {
            let done = transform_text(sup, focus, &answer.ui_lang, answer.op, answer.lang.as_deref(), TextSource::Given(&answer.source_text), VARIANT_TEMPERATURE).await?;
            remember(answers, answer.op, answer.lang.as_deref(), &answer.ui_lang, &done);
            Ok(done.reply)
        }
        Kind::Lang(code) => {
            let from = super::followup::continued_text(&answer, textops::Op::Translate).unwrap_or_else(|| answer.result.clone());
            let done = transform_text(sup, focus, &answer.ui_lang, textops::Op::Translate, Some(code), TextSource::Given(&from), 0.2).await?;
            remember(answers, textops::Op::Translate, Some(code), &answer.ui_lang, &done);
            Ok(done.reply)
        }
    }
}

#[cfg(windows)]
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use super::clipboard::{Clipboard, SystemClipboard};
    SystemClipboard.set(text)
}

#[cfg(not(windows))]
fn copy_to_clipboard(_: &str) -> Result<(), String> {
    Err("unsupported platform".into())
}
/// The value of the variable `voice.goto`: the page and a counter, so going to the same page twice is a new value.
/// The phones watch the variable and navigate (they never send the page back).
pub fn goto_value(page_id: &str, seq: u64) -> String {
    format!("{page_id}#{seq}")
}

/// True when a spoken word alone must not run this action: it ends the session or throws files away (itself or one of the
/// steps of a sequence). A misheard phrase must not switch the PC off.
fn ends_work(id: &str, actions: &ActionsFile, depth: u8) -> bool {
    use crate::model::{Action, SequenceStep, SystemOp};
    match actions.actions.get(id) {
        Some(Action::System { op, .. }) => matches!(op, SystemOp::Shutdown | SystemOp::Restart | SystemOp::Signout | SystemOp::Hibernate | SystemOp::Sleep | SystemOp::EmptyRecycleBin),
        Some(Action::Sequence { steps, .. }) if depth < 4 => steps.iter().any(|s| matches!(s, SequenceStep::Run { action_id } if ends_work(action_id, actions, depth + 1))),
        _ => false,
    }
}

/// What exists on this PC now (DATA_MODEL.md 12.5): the lists the tools check names against.
pub fn build_context(lang: &str, actions: &ActionsFile, layout: &LayoutFile, installed: &[InstalledApp]) -> Context {
    let mut buttons = Vec::new();
    let mut confirm_actions = Vec::new();
    for page in &layout.pages {
        for item in &page.items {
            if let LayoutItem::Button(b) = item {
                let Some(action_id) = &b.action_id else { continue };
                if let Some(label) = b.label.as_ref().filter(|l| !l.trim().is_empty()) {
                    buttons.push(ButtonRef { label: label.clone(), action_id: action_id.clone() });
                }
                if b.confirm == Some(true) && !confirm_actions.contains(action_id) {
                    confirm_actions.push(action_id.clone());
                }
            }
        }
    }
    // T29.A: shutting down and the like ask "yes" by voice also when the key itself is not marked.
    for id in actions.actions.keys() {
        if !confirm_actions.contains(id) && ends_work(id, actions, 0) {
            confirm_actions.push(id.clone());
        }
    }
    Context {
        vision_ready: false,
        vision_off: false,
        ha: vec![],
        no_reply: false,
        heavy: vec![],
        memory: Vec::new(),
        weather_place: None,
        guide_running: false,
        speech_interrupted: false,
        full_pages: layout.pages.iter().filter(|p| super::addbutton::free_cell(p, i32::from(layout.grid.cols), i32::from(layout.grid.rows)).is_none()).map(|p| p.id.clone()).collect(),
        current_page: None,
        lang: lang.to_string(),
        actions: actions.actions.iter().map(|(id, a)| ActionRef { id: id.clone(), name: a.name().to_string() }).collect(),
        buttons,
        apps: installed.iter().take(200).map(|a| AppRef { name: a.name.clone(), program: a.program.clone() }).collect(),
        pages: layout.pages.iter().filter(|p| p.popup != Some(true)).map(|p| PageRef { id: p.id.clone(), name: p.name.clone() }).collect(),
        confirm_actions,
    }
}

/// The phrase table of a language (embedded; English when the language has none).
pub fn table_text(lang: &str) -> &'static str {
    match lang {
        "ru" => include_str!("../../../../shared/voice/ru.json"),
        "zh" => include_str!("../../../../shared/voice/zh.json"),
        "hi" => include_str!("../../../../shared/voice/hi.json"),
        "es" => include_str!("../../../../shared/voice/es.json"),
        "ar" => include_str!("../../../../shared/voice/ar.json"),
        "fr" => include_str!("../../../../shared/voice/fr.json"),
        "pt" => include_str!("../../../../shared/voice/pt.json"),
        "de" => include_str!("../../../../shared/voice/de.json"),
        "ja" => include_str!("../../../../shared/voice/ja.json"),
        "kk" => include_str!("../../../../shared/voice/kk.json"),
        _ => include_str!("../../../../shared/voice/en.json"),
    }
}

/// Whether voice control is on for this PC: the models are there (the servers start with the first request). A server
/// that failed does not turn it off (T28.2): the hotkey still answers, the island says why, and the recognition and the
/// phrase tables work without the model.
pub fn is_on(status: &VoiceStatus) -> bool {
    matches!(status.state, VoiceState::Ready | VoiceState::Loading) || status.reason.is_some()
}

// ---- The glue ---------------------------------------------------------------------------------------------------

pub type PillHook = Arc<dyn Fn(PillCmd) + Send + Sync>;
/// Opens the settings window at a target (`keys:catalog:<id>`).
pub type SettingsHook = Arc<dyn Fn(&str) + Send + Sync>;

struct Inner {
    handle: tokio::runtime::Handle,
    /// "Dictate" waits for the next phrase until this moment.
    dictate_until: Arc<Mutex<Option<Instant>>>,
    /// T26.3: the answer that can be continued for 30 s (memory only).
    answers: Answers,
    /// T26.7: hint chips of the island that opened by voice (empty when the setting is off).
    chips: Arc<Mutex<Vec<Value>>>,
    /// The slow path: durations of the last calls, and when the running one started.
    slow: Arc<Mutex<super::slowpath::Tracker>>,
    slow_since: Arc<Mutex<Option<Instant>>>,
    /// Timers that are running now (at most `winops::MAX_TIMERS`).
    timers: Arc<AtomicUsize>,
    goto_seq: Arc<AtomicU64>,
    agent: Agent,
    service: VoiceService,
    pill: PillHook,
    driver: OnceLock<Driver>,
    tables: Mutex<HashMap<String, Arc<Table>>>,
    installed: Mutex<Vec<InstalledApp>>,
    sink_on: AtomicBool,
    tracker: Mutex<Tracker>,
    /// Dictation (DATA_MODEL.md 13.1): the settings and whether the running request is a dictation.
    dictation: Mutex<DictationSettings>,
    dictation_request: AtomicBool,
    /// The chord of dictation that is registered now.
    dictation_chord: Mutex<Option<hotkey::Chord>>,
    #[cfg(windows)]
    ctrl_hook: Mutex<Option<super::winkeys::CtrlHook>>,
    #[cfg(windows)]
    hotkeys: Mutex<Option<hotkey::Hotkeys>>,
    #[cfg(windows)]
    mic: Mutex<Option<mic::Mic>>,
    /// The last info of a request, to repeat it with a new level.
    last: Mutex<Option<VoiceInfo>>,
    /// Chunks the microphone has delivered to the current request.
    chunks: Arc<AtomicUsize>,
    /// Counts the changes of the state; the watchdog fires if the count did not move.
    changes: Arc<AtomicU64>,
    /// M27: the screen help (find, press, guide), once the application has given it a window to highlight in.
    screen: OnceLock<Arc<super::screenrun::ScreenRun>>,
    /// "Check voice" is running (T28.3).
    checking: AtomicBool,
    /// T29.1: the window and the element that had the focus when the request began.
    focus: FocusMemory,
    /// T29.2: the percent of the Screen help package while it is downloaded for a question.
    download: Arc<Mutex<Option<u8>>>,
    /// T29.3: turns other sounds down while the PC records.
    ducker: super::duck::Ducker,
    /// T29.4: the orb of a dictation.
    orb: Arc<Mutex<super::orb::Orb>>,
    /// T29.8: the request whose answer was spoken, and whether this request cut a spoken answer short.
    spoken: Mutex<Option<String>>,
    interrupted: AtomicBool,
    /// T29.6: the page the voice went to last, for when no phone has said which page it shows.
    current_page: Arc<Mutex<Option<String>>>,
    /// T30.2: the last answer to "what uses the memory".
    heavy: Arc<Mutex<Option<HeavyList>>>,
    /// T30.4: the focus timer of a work mode.
    focus_timer: Arc<super::modes::FocusTimer>,
    /// T29.6: shows the settings window on a catalog key; the application gives it.
    settings: OnceLock<SettingsHook>,
    /// T32.1: the wake word.
    wake: Wake,
}

#[derive(Clone)]
pub struct Host {
    inner: Arc<Inner>,
}

impl Host {
    /// While the model or the recognition is being loaded: the seconds that are probably left.
    pub fn loading_eta_sec(&self) -> Option<u32> {
        self.inner.service.supervisor().loading_eta_sec()
    }

    pub fn new(handle: tokio::runtime::Handle, agent: Agent, service: VoiceService, pill: PillHook) -> Host {
        if !cfg!(test) {
            mic::scrub_old_log();
        }
        let dictation = DictationSettings::load(agent.config_dir());
        let config_dir = agent.config_dir().to_path_buf();
        let (root, manifest, downloader) = service.package();
        let exe = std::env::current_exe().unwrap_or_else(|_| "decka-agent.exe".into());
        let wake = Wake::new(handle.clone(), WakeDeps { config_dir: config_dir.clone(), root, manifest, downloader, exe });
        let inner = Arc::new(Inner {
            handle: handle.clone(),
            dictate_until: Arc::new(Mutex::new(None)),
            answers: Arc::new(Mutex::new(Default::default())),
            chips: Arc::new(Mutex::new(Vec::new())),
            slow: Arc::new(Mutex::new(super::slowpath::Tracker::load(&config_dir))),
            slow_since: Arc::new(Mutex::new(None)),
            timers: Arc::new(AtomicUsize::new(0)),
            goto_seq: Arc::new(AtomicU64::new(0)),
            agent,
            service,
            pill,
            driver: OnceLock::new(),
            tables: Mutex::new(HashMap::new()),
            installed: Mutex::new(Vec::new()),
            sink_on: AtomicBool::new(false),
            tracker: Mutex::new(Tracker::default()),
            dictation: Mutex::new(dictation),
            dictation_request: AtomicBool::new(false),
            dictation_chord: Mutex::new(None),
            #[cfg(windows)]
            ctrl_hook: Mutex::new(None),
            #[cfg(windows)]
            hotkeys: Mutex::new(None),
            #[cfg(windows)]
            mic: Mutex::new(None),
            last: Mutex::new(None),
            chunks: Arc::new(AtomicUsize::new(0)),
            changes: Arc::new(AtomicU64::new(0)),
            screen: OnceLock::new(),
            checking: AtomicBool::new(false),
            focus: FocusMemory::default(),
            download: Arc::default(),
            ducker: super::duck::Ducker::start(&config_dir),
            orb: Arc::default(),
            spoken: Mutex::new(None),
            interrupted: AtomicBool::new(false),
            current_page: Arc::default(),
            heavy: Arc::default(),
            focus_timer: Arc::default(),
            settings: OnceLock::new(),
            wake,
        });
        let driver = Driver::new(handle.clone(), inner.clone());
        let _ = inner.driver.set(driver);
        let host = Host { inner };
        host.start_wake();
        // M30 (T30.1): the meeting notes use the same recognition and model; what happens to them is said on the island.
        let notes = host.inner.agent.notes();
        notes.set_backend(Arc::new(NoteModels { agent: host.inner.agent.clone(), service: host.inner.service.clone() }));
        let weak = Arc::downgrade(&host.inner);
        notes.listen(Arc::new(move |event| {
            let Some(inner) = weak.upgrade() else { return };
            let warned = inner.agent.config().notes.is_some_and(|n| n.warned);
            if let Some(answer) = note_notice(&event, warned) {
                inner.plate(&super::status::encode(&answer), false, if warned { 6 } else { 12 });
            }
        }));
        // The list of programs takes a moment to read: not on the first request.
        let weak = Arc::downgrade(&host.inner);
        handle.spawn_blocking(move || {
            let mut apps = crate::installed::list_installed_apps();
            for (name, path) in [("Calculator", r"C:\Windows\System32\calc.exe"), ("Notepad", r"C:\Windows\System32\notepad.exe")] {
                if std::path::Path::new(path).is_file() && !apps.iter().any(|a| a.name.eq_ignore_ascii_case(name)) {
                    apps.push(InstalledApp { name: name.into(), program: path.into(), args: vec![], cwd: None, icon: None });
                }
            }
            if let Some(inner) = weak.upgrade() {
                *inner.installed.lock().unwrap() = apps;
            }
        });
        // Reminders (13.4): what is due is announced every 20 s; what was missed while the PC was off, once at the start.
        let weak = Arc::downgrade(&host.inner);
        handle.spawn(async move {
            let mut on_start = true;
            loop {
                let Some(inner) = weak.upgrade() else { break };
                let due = inner.agent.assistant().take_due(&crate::assistant::Local::now());
                if let Some((title, body)) = crate::assistant::due_notice(&due, on_start) {
                    inner.remind_plate(&body);
                    inner.agent.push_notification(crate::proto::NotifyReq { title, body: Some(body), level: Some(crate::model::NotifLevel::Info), source: Some("reminder".into()) });
                }
                on_start = false;
                drop(inner);
                tokio::time::sleep(Duration::from_secs(20)).await;
            }
        });
        // "The model is loading" reaches the pipeline from the supervisor.
        let weak = Arc::downgrade(&host.inner);
        let mut rx = host.inner.service.supervisor().subscribe();
        handle.spawn(async move {
            while rx.changed().await.is_ok() {
                let loading = *rx.borrow_and_update() == SupState::Loading;
                let Some(inner) = weak.upgrade() else { break };
                if loading {
                    inner.driver().handle_event(Event::ModelLoading);
                }
            }
        });
        host
    }

    /// Every change of the status of voice control: connects or disconnects the phones and the hotkey, sets `voice.ready`.
    pub fn on_status(&self, status: &VoiceStatus) {
        let inner = &self.inner;
        let on = is_on(status);
        if inner.sink_on.swap(on, Ordering::SeqCst) != on {
            let sink: Option<Arc<dyn super::PhoneSink>> = on.then(|| Arc::new(inner.driver().clone()) as Arc<dyn super::PhoneSink>);
            inner.agent.voice().set_sink(sink);
            inner.agent.store().update_vars(|vars| {
                vars.insert("voice.ready".to_string(), VarValue::Bool(on));
            });
            if !on {
                inner.driver().handle_event(Event::Cancel);
            }
        }
        self.register_hotkey(on.then_some(status.hotkey.as_slice()));
        self.register_dictation(on);
    }

    /// The settings of dictation as they are now.
    pub fn dictation_settings(&self) -> DictationSettings {
        self.inner.dictation.lock().unwrap().clone()
    }

    /// Saves new settings of dictation and applies them (the trigger is registered or removed at once).
    pub fn set_dictation_settings(&self, mut settings: DictationSettings) -> Result<(), String> {
        settings.dictionary = dictation::normalize_dictionary(&settings.dictionary);
        if let Trigger::Chord { keys } = &settings.trigger {
            hotkey::parse_chord(keys)?;
        }
        settings.save(self.inner.agent.config_dir())?;
        *self.inner.dictation.lock().unwrap() = settings;
        self.register_dictation(is_on(&self.inner.service.status()));
        Ok(())
    }

    /// The trigger of dictation: the keyboard hook of the double Ctrl, or a chord, only while voice control is on.
    #[cfg(windows)]
    fn register_dictation(&self, on: bool) {
        let inner = &self.inner;
        let trigger = inner.dictation.lock().unwrap().trigger.clone();
        {
            let mut slot = inner.ctrl_hook.lock().unwrap();
            let want = on && trigger == Trigger::DoubleCtrl;
            if want && slot.is_none() {
                let weak: Weak<Inner> = Arc::downgrade(inner);
                match super::winkeys::CtrlHook::start(Arc::new(move |c| {
                    if let Some(inner) = weak.upgrade() {
                        inner.ctrl_pressed(c);
                    }
                })) {
                    Ok(h) => *slot = Some(h),
                    Err(e) => tracing::warn!("dictation keyboard hook: {e}"),
                }
            } else if !want && slot.is_some() {
                *slot = None;
            }
        }
        let chord = match (&trigger, on) {
            (Trigger::Chord { keys }, true) => hotkey::parse_chord(keys).ok(),
            _ => None,
        };
        let mut current = inner.dictation_chord.lock().unwrap();
        if *current != chord {
            let shown = match &trigger {
                Trigger::Chord { keys } => keys.clone(),
                _ => Vec::new(),
            };
            let result = self.hotkeys().and_then(|h| h.as_ref().map_or(Ok(()), |h| h.set_dictation(chord, &shown)));
            if let Err(e) = result {
                tracing::warn!("dictation hotkey: {e}");
            }
            *current = chord;
        }
    }

    #[cfg(not(windows))]
    fn register_dictation(&self, _on: bool) {}

    #[cfg(windows)]
    fn register_hotkey(&self, wanted: Option<&[String]>) {
        let inner = &self.inner;
        let plan = inner.tracker.lock().unwrap().plan(wanted);
        let result = match plan {
            Ok(Plan::Nothing) => return,
            Ok(Plan::Unregister) => self.hotkeys().and_then(|h| h.as_ref().map_or(Ok(()), |h| h.set_main(None, &[]))),
            Ok(Plan::Register(chord)) => self.hotkeys().and_then(|h| h.as_ref().map_or(Ok(()), |h| h.set_main(Some(chord), wanted.unwrap_or(&[])))),
            Err(e) => Err(e),
        };
        if let Err(e) = &result {
            tracing::warn!("voice hotkey: {e}");
        }
        inner.service.set_hotkey_error(result.err());
    }

    #[cfg(not(windows))]
    fn register_hotkey(&self, _wanted: Option<&[String]>) {}

    #[cfg(windows)]
    fn hotkeys(&self) -> Result<std::sync::MutexGuard<'_, Option<hotkey::Hotkeys>>, String> {
        let mut guard = self.inner.hotkeys.lock().unwrap();
        if guard.is_none() {
            let weak: Weak<Inner> = Arc::downgrade(&self.inner);
            *guard = Some(hotkey::Hotkeys::start(Arc::new(move |p| {
                if let Some(inner) = weak.upgrade() {
                    inner.pressed(p);
                }
            }))?);
        }
        Ok(guard)
    }

    /// T30.2: the programs the island lists now, as `{ pid, name, mb }`.
    pub fn memory_shown(&self) -> Option<Vec<Value>> {
        let list = self.inner.heavy.lock().unwrap();
        let l = list.as_ref().filter(|l| l.shown && l.at.elapsed() < HEAVY_SHOWN && !l.list.is_empty())?;
        Some(l.list.iter().map(|h| serde_json::json!({ "pid": h.pid, "name": h.name, "mb": h.mb })).collect())
    }

    /// `memory_close` of the island (IPC): the "Close" button of one program of the list. It asks "close it?" first.
    pub fn memory_close(&self, pid: u32) -> Result<(), String> {
        let program = {
            let mut list = self.inner.heavy.lock().unwrap();
            let l = list.as_mut().filter(|l| l.at.elapsed() < HEAVY_FRESH).ok_or("The list is out of date")?;
            let program = l.list.iter().find(|h| h.pid == pid).cloned().ok_or("Unknown program")?;
            l.shown = false;
            program
        };
        let super::tools::Resolved::Ready(call) = super::tools::close_question(&program) else { return Err("Unknown program".into()) };
        // A recording that is running is not needed any more.
        if self.inner.driver().state().phase == Phase::Listening {
            self.inner.driver().handle_event(Event::Cancel);
        }
        self.inner.driver().handle_event(Event::Propose { id: format!("{:016x}", rand::random::<u64>()), call });
        Ok(())
    }

    /// `voice_confirm` of the pill (IPC).
    pub fn confirm(&self, id: &str, yes: bool) {
        self.inner.driver().handle_event(Event::Confirm { id: id.to_string(), yes });
    }

    /// The hint chips of the island that is open now (T26.7), as `{ label, command }`; the first one starts or stops a
    /// meeting note (T30.1).
    pub fn context_chips(&self) -> Vec<Value> {
        let note = if self.inner.agent.notes().status().active { ("voice.chip.note_stop", "note_stop") } else { ("voice.chip.note", "note") };
        let mut chips = vec![serde_json::json!({ "label": note.0, "command": note.1 })];
        chips.extend(self.inner.chips.lock().unwrap().iter().cloned());
        chips
    }

    /// `voice_chip` of the PC window (IPC): a tap on a chip is the same request as saying it. The command is one of
    /// `context::COMMANDS`; what it does is decided here.
    pub fn chip(&self, command: &str) -> Result<(), String> {
        use super::textops::Op;
        let lang = self.inner.language();
        let call = match command {
            "translate" => ToolCall::TextTransform { op: Op::Translate, lang: Some(lang.to_string()), text: None },
            "explain" => ToolCall::TextTransform { op: Op::Explain, lang: None, text: None },
            "reply" => ToolCall::TextTransform { op: Op::Reply, lang: None, text: None },
            "error" => ToolCall::ScreenAsk { question: "What is this error and how can it be fixed?".into() },
            "note" => ToolCall::NotesRecord { start: Some(true) },
            "note_stop" => ToolCall::NotesRecord { start: Some(false) },
            _ => return Err("unknown chip".into()),
        };
        let inner = self.inner.clone();
        // The recording that is running is not needed any more.
        if self.inner.driver().state().phase == Phase::Listening {
            self.inner.driver().handle_event(Event::Cancel);
        }
        let shows_text = matches!(call, ToolCall::TextTransform { .. });
        self.inner.handle.spawn(async move {
            match inner.execute(call).await {
                Ok(text) if !text.is_empty() => inner.plate(&text, shows_text, 20),
                Ok(_) => {}
                Err(e) => inner.plate(&e, false, 5),
            }
        });
        Ok(())
    }

    /// `voice_action` of the PC window (IPC): `copy`, `insert`, `speak`, `again`, `lang:<code>` on the answer that was just shown.
    /// The result of `again` and `lang` is shown as a new answer. The window only names an action; what it does is decided here.
    pub fn action(&self, kind: &str) -> Result<(), String> {
        // T30.2: the button of the memory note.
        if kind == "memory" {
            let inner = self.inner.clone();
            self.inner.handle.spawn(async move {
                match inner.execute(ToolCall::MemoryTop).await {
                    Ok(text) => inner.plate(&text, false, 20),
                    Err(e) => inner.plate(&e, false, 5),
                }
            });
            return Ok(());
        }
        let kind = super::followup::Kind::parse(kind).ok_or_else(|| "unknown action".to_string())?;
        let inner = self.inner.clone();
        self.inner.handle.spawn(async move {
            let sup = inner.service.supervisor().clone();
            match reply_action(&sup, &inner.focus, &inner.answers, &kind).await {
                Ok(text) if !text.is_empty() => inner.plate(&text, true, 20),
                Ok(_) => {}
                Err(e) => inner.plate(&e, false, 5),
            }
        });
        Ok(())
    }

    /// `voice_self_check` (T28.3): microphone, recognition, model and a dry run of the action; `progress` gets every
    /// change. Nothing is executed. A server that had given up gets another try.
    pub async fn self_check(&self, progress: CheckProgress) -> Result<Vec<CheckStep>, String> {
        let inner = self.inner.clone();
        if !is_on(&inner.service.status()) {
            return Err("Voice control is not installed".into());
        }
        let phase = inner.driver().state().phase;
        if is_active(&phase) || matches!(phase, Phase::Confirming { .. }) {
            return Err("A voice request is running".into());
        }
        if inner.checking.swap(true, Ordering::SeqCst) {
            return Err("The check is running already".into());
        }
        let _running = Checking(inner.clone());
        let sup = inner.service.supervisor().clone();
        if sup.failure_of(Server::Llm).is_some() || sup.failure_of(Server::Stt).is_some() {
            sup.retry().await;
        }
        let lang = inner.language();
        Ok(super::check::run(&HostCheck { inner, progress }, lang).await)
    }

    /// Starts or ends a PC voice request, as if the hotkey were pressed.
    pub fn trigger(&self) {
        self.inner.pressed(Pressed::Main);
    }

    /// M27: gives the screen help its highlight (the window of the application) and the sink of the guide events. Once.
    pub fn attach_screen(&self, highlighter: super::highlight::Highlighter, events: Arc<dyn super::screenrun::GuideSink>, danger: Vec<String>) {
        let guides = super::guides::Guides::new(self.inner.agent.config_dir());
        let run = super::screenrun::ScreenRun::new(Arc::new(super::screenrun::WinScreen::new()), highlighter, guides, events, danger);
        let _ = self.inner.screen.set(Arc::new(run));
    }

    /// T27.8: every few seconds looks at the widgets and, when a rule says so, shows a short note on the island (no sound).
    pub fn start_notices(&self) {
        let inner = self.inner.clone();
        self.inner.handle.spawn(async move {
            let (mut notices, started) = (super::notices::Notices::default(), Instant::now());
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let store = inner.agent.store();
                let fullscreen = tokio::task::spawn_blocking(crate::lyrics_overlay::foreground_is_fullscreen).await.unwrap_or(false);
                let focus = inner.focus_timer.ended(Instant::now());
                let mut s = super::notices::sample(started.elapsed().as_millis() as u64, &store.get("sys"), &store.get("battery"), &store.get("ai"), focus, fullscreen);
                s.soon = super::notices::soon(&store.get("calendar"), crate::auth::now_ms() as i64);
                for n in notices.step(inner.agent.voice_notices_enabled(), &s) {
                    if n.rule == super::notices::Rule::Ram {
                        // T30.2: with the button "Show what uses it".
                        let pct = s.ram_pct.map_or_else(String::new, |v| format!("{}", v.round() as i64));
                        let said = super::status::Answer { key: "voice.reply.ram_notice", args: [("pct".to_string(), pct)].into(), text: n.text.clone() };
                        inner.plate_with(&super::status::encode(&said), vec!["memory"], 20);
                    } else if n.rule == super::notices::Rule::Phone {
                        let pct = s.phone_low.map_or_else(String::new, |v| format!("{}", v.round() as i64));
                        let said = super::status::Answer { key: "voice.reply.phone_battery_low", args: [("pct".to_string(), pct)].into(), text: n.text.clone() };
                        inner.plate(&super::status::encode(&said), false, 8);
                    } else if n.rule == super::notices::Rule::Event {
                        let args = n.event.map(|e| [("min".to_string(), e.minutes.to_string()), ("title".to_string(), e.title)].into()).unwrap_or_default();
                        let said = super::status::Answer { key: "voice.reply.calendar_soon", args, text: n.text.clone() };
                        inner.plate(&super::status::encode(&said), false, 12);
                    } else if n.rule == super::notices::Rule::Focus {
                        inner.focus_timer.told();
                        let said = super::status::Answer { key: "voice.reply.focus_over", args: Default::default(), text: n.text.clone() };
                        inner.plate(&super::status::encode(&said), false, 8);
                    } else {
                        inner.plate(&n.text, false, 6);
                    }
                }
            }
        });
    }

    pub fn screen(&self) -> Option<Arc<super::screenrun::ScreenRun>> {
        self.inner.screen.get().cloned()
    }

    /// The current state of the pipeline.
    pub fn state(&self) -> State {
        self.inner.driver().state()
    }

    /// The slow call that runs now (the vision model): the estimate and the time so far, for the island.
    pub fn slow_progress(&self) -> Option<super::slowpath::Progress> {
        let since = (*self.inner.slow_since.lock().unwrap())?;
        let eta_sec = self.inner.slow.lock().unwrap().eta_sec(false);
        Some(super::slowpath::Progress { eta_sec, elapsed_sec: since.elapsed().as_secs() as u32 })
    }

    /// T29.8: speaks the answer of the PC request that is on show, once, when "answer aloud" is on. The window of the island
    /// gives the text in the language of the interface; anything else than a short answer of the current request is ignored.
    pub fn speak_answer(&self, text: &str) {
        let inner = self.inner.clone();
        let state = inner.driver().state();
        if !inner.service.status().speak || state.source != Some(Source::Pc) || !matches!(state.phase, Phase::Done(_) | Phase::Failed(_)) {
            return;
        }
        {
            let mut spoken = inner.spoken.lock().unwrap();
            if *spoken == state.id {
                return;
            }
            *spoken = state.id;
        }
        let (text, lang) = (text.trim().to_string(), inner.language());
        if text.is_empty() {
            return;
        }
        std::thread::spawn(move || {
            inner.ducker.hold(super::duck::Who::Speech, inner.agent.voice_duck_enabled());
            if let Err(e) = super::speak::answer(&text, lang) {
                tracing::warn!("speaking the answer: {e}");
            }
            inner.ducker.hold(super::duck::Who::Speech, false);
        });
    }

    /// T29.6: gives the host a way to open the settings window. Once.
    pub fn attach_settings(&self, open: SettingsHook) {
        let _ = self.inner.settings.set(open);
    }

    /// T29.4: the orb of a dictation, while there is one: the island window shows it at the caret instead of the panel.
    pub fn orb(&self) -> Option<super::orb::View> {
        self.inner.orb.lock().unwrap().view(Instant::now())
    }

    /// "Quit": the helpers are asked to end (T32.H.4); what does not is taken by the job objects when the process is gone.
    pub async fn stop_helpers(&self) {
        self.inner.wake.stop();
        self.inner.service.supervisor().stop().await;
    }

    /// The program is ending: the volumes that were turned down come back first.
    pub fn quit(&self) {
        self.inner.wake.stop();
        self.inner.ducker.quit();
    }

    /// T32.1: the wake word, for the settings window and the tray.
    pub fn wake(&self) -> &Wake {
        &self.inner.wake
    }

    /// A detection is the same request as the voice key, but it never ends a recording that runs. Twice a second the
    /// wake word hears what is going on (a note, a full-screen game, "Check voice").
    fn start_wake(&self) {
        let weak = Arc::downgrade(&self.inner);
        self.inner.wake.on_wake(Arc::new(move || {
            let Some(inner) = weak.upgrade() else { return };
            if matches!(inner.driver().state().phase, Phase::Idle | Phase::Done(_) | Phase::Failed(_)) {
                mic::log_line("wake word");
                inner.dictation_request.store(false, Ordering::SeqCst);
                inner.pressed(Pressed::Main);
            }
        }));
        let weak = Arc::downgrade(&self.inner);
        self.inner.handle.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let Some(inner) = weak.upgrade() else { break };
                let around = if inner.wake.settings().enabled {
                    Surroundings {
                        voice_on: inner.sink_on.load(Ordering::SeqCst),
                        recording_note: inner.agent.notes().status().active,
                        fullscreen: tokio::task::spawn_blocking(crate::lyrics_overlay::foreground_is_fullscreen).await.unwrap_or(false),
                        checking: inner.checking.load(Ordering::SeqCst),
                        mic: inner.service.microphone(),
                    }
                } else {
                    Surroundings::default()
                };
                // Starting and stopping a process is not work for the async threads.
                let wake = inner.wake.clone();
                if let Err(e) = tokio::task::spawn_blocking(move || wake.tick(around)).await {
                    tracing::warn!("wake word: {e}");
                }
            }
        });
    }

    /// The download that a question waits for, in percent, for the island.
    pub fn download_progress(&self) -> Option<u8> {
        *self.inner.download.lock().unwrap()
    }

    /// `voice_stop` of the pill (IPC): the click on the stop button or on the pill ends the recording of the PC at once.
    pub fn stop(&self) {
        let driver = self.inner.driver();
        let s = driver.state();
        mic::log_line("voice_stop from the pill");
        if s.is_hearing() && s.source == Some(Source::Pc) && !too_early() {
            driver.handle_event(Event::AudioEnd);
        }
    }
}

/// The model of the guide: the chat body goes to the local server, the text of the answer comes back. While it thinks the island
/// shows the estimate (the slow path).
struct LlamaAsker {
    sup: crate::ai::supervisor::Supervisor,
    slow_since: Arc<Mutex<Option<Instant>>>,
    slow: Arc<Mutex<super::slowpath::Tracker>>,
    dir: std::path::PathBuf,
}

impl super::screenrun::StepAsker for LlamaAsker {
    fn eta_sec(&self) -> Option<u32> {
        Some(self.slow.lock().unwrap().eta_sec(false))
    }

    fn ask(&self, body: Value) -> BoxFut<Option<String>> {
        let (sup, since, slow, dir) = (self.sup.clone(), self.slow_since.clone(), self.slow.clone(), self.dir.clone());
        Box::pin(async move {
            *since.lock().unwrap() = Some(Instant::now());
            let started = Instant::now();
            let content = async {
                let (port, key) = sup.ensure_llm().await.ok()?;
                let client = reqwest::Client::builder().timeout(Duration::from_secs(60)).build().ok()?;
                let answer = client
                    .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
                    .bearer_auth(&key)
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_vec(&body).ok()?)
                    .send()
                    .await
                    .ok()?;
                if !answer.status().is_success() {
                    return None;
                }
                let v: Value = serde_json::from_str(&answer.text().await.ok()?).ok()?;
                sup.touch();
                v.pointer("/choices/0/message/content")?.as_str().map(str::to_string)
            }
            .await;
            *since.lock().unwrap() = None;
            if content.is_some() {
                let mut t = slow.lock().unwrap();
                t.record(started.elapsed().as_millis() as u64);
                t.save(&dir);
            }
            content
        })
    }
}

impl Inner {
    fn driver(&self) -> &Driver {
        self.driver.get().expect("the driver is set in Host::new")
    }

    fn language(&self) -> &'static str {
        crate::tray_text::agent_language(self.agent.layout().display.language.as_deref())
    }

    fn table(&self, lang: &str) -> Arc<Table> {
        let mut tables = self.tables.lock().unwrap();
        tables
            .entry(lang.to_string())
            .or_insert_with(|| Arc::new(Table::parse(table_text(lang)).or_else(|_| Table::parse(table_text("en"))).expect("the English table is valid")))
            .clone()
    }

    /// The double Ctrl starts a dictation; a single tap of Ctrl ends the one that is listening.
    #[cfg(windows)]
    fn ctrl_pressed(&self, c: super::winkeys::Ctrl) {
        use super::winkeys::Ctrl;
        let state = self.driver().state();
        let listening_dictation = state.phase == Phase::Listening && state.source == Some(Source::Pc) && self.dictation_request.load(Ordering::SeqCst);
        match c {
            Ctrl::Double if !listening_dictation => self.start_dictation(),
            Ctrl::Tap if listening_dictation && !too_early() => self.driver().handle_event(Event::AudioEnd),
            _ => {}
        }
    }

    /// Starts a request of the PC whose phrase is typed as it is (after the `clean` pass), whatever it sounds like.
    fn start_dictation(&self) {
        let state = self.driver().state();
        match (&state.phase, state.source) {
            (Phase::Idle | Phase::Done(_) | Phase::Failed(_), _) => {
                self.dictation_request.store(true, Ordering::SeqCst);
                self.pressed(Pressed::Main);
            }
            // A dictation that is listening ends with the next press of its chord.
            (Phase::Listening, Some(Source::Pc)) if self.dictation_request.load(Ordering::SeqCst) => self.pressed(Pressed::Main),
            _ => {}
        }
    }

    /// T26.7: looks at the active program and the selection when the island opens by voice - only if the user turned it on.
    fn refresh_chips(&self) {
        self.chips.lock().unwrap().clear();
        if !self.agent.voice_context_enabled() {
            return;
        }
        let (chips, vision) = (self.chips.clone(), self.service.vision_installed());
        self.handle.spawn(async move {
            let found = tokio::task::spawn_blocking(move || super::context::chips_json(&super::context::chips(&super::context::snapshot(vision)))).await.unwrap_or_default();
            *chips.lock().unwrap() = found;
        });
    }

    fn pressed(&self, p: Pressed) {
        // "Check voice" holds the microphone.
        if self.checking.load(Ordering::SeqCst) {
            return;
        }
        if p == Pressed::Dictate {
            return self.start_dictation();
        }
        let state = self.driver().state();
        mic::log_line(&format!("hotkey {p:?} phase={:?}", std::mem::discriminant(&state.phase)));
        let event = match p {
            Pressed::Main => press_event(&state, format!("{:016x}", rand::random::<u64>())),
            Pressed::Esc => esc_event(&state),
            Pressed::Enter => enter_event(&state),
            Pressed::Dictate => None,
        };
        if let Some(e) = event {
            if e == Event::AudioEnd && too_early() {
                mic::log_line("end ignored: too early");
                return;
            }
            if matches!(e, Event::Start { .. }) {
                *PC_STARTED.lock().unwrap() = Some(Instant::now());
                self.refresh_chips();
            }
            self.driver().handle_event(e);
        }
    }

    fn set_aux(&self, state: &State) {
        #[cfg(windows)]
        if let Some(h) = self.hotkeys.lock().unwrap().as_ref() {
            let (esc, enter) = aux_for(state);
            h.set_aux(esc, enter);
        }
        #[cfg(not(windows))]
        let _ = state;
    }

    /// The plate of a reminder (DATA_MODEL.md 13.7): the pill shows its text for 5 s, unless a voice request is running.
    fn remind_plate(self: &Arc<Self>, text: &str) {
        self.plate(text, false, 5);
    }

    /// A line on the pill for `secs` seconds; with `actions` the PC window offers what can be done with the answer (T26.3).
    fn plate(self: &Arc<Self>, text: &str, actions: bool, secs: u64) {
        self.plate_with(text, if actions { super::followup::ACTIONS.to_vec() } else { Vec::new() }, secs);
    }

    /// `plate` with the buttons of its own.
    fn plate_with(self: &Arc<Self>, text: &str, buttons: Vec<&'static str>, secs: u64) {
        if is_active(&self.driver().state().phase) {
            return;
        }
        // A fixed phrase is translated by the window ("Copied: paste with Ctrl+V"); a free text is shown as it is.
        let (text, key, args) = match super::status::decode(text) {
            Some((key, args, plain)) => (plain, Some(key), args),
            None => (text.to_string(), super::pipeline::executor_key(text), Default::default()),
        };
        let text = text.as_str();
        let info = move |state: &'static str, reply: Option<String>| VoiceInfo {
            state,
            id: reply.is_some().then(|| "0000000000000000".to_string()),
            source: reply.is_some().then_some(Source::Pc),
            heard: None,
            reply_key: key.filter(|_| reply.is_some()),
            reply_args: if reply.is_some() { args.clone() } else { Default::default() },
            reply,
            confirm: None,
            activity: None,
            actions: if state == "done" { buttons.clone() } else { Vec::new() },
            understood: None,
            executing: false,
        };
        self.show(&info("done", Some(text.chars().take(200).collect())), 0.0);
        let weak = Arc::downgrade(self);
        self.handle.spawn(async move {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            if let Some(inner) = weak.upgrade() {
                // A voice request that started meanwhile owns the pill now.
                if !is_active(&inner.driver().state().phase) {
                    inner.show(&info("idle", None), 0.0);
                }
            }
        });
    }

    fn show(&self, info: &VoiceInfo, level: f32) {
        let hotkey = self.service.status().hotkey;
        if let Some(cmd) = pill_cmd(info, &hotkey, level) {
            let pill = self.pill.clone();
            pill_worker(Box::new(move || pill(cmd)));
        }
    }
}

// ---- "Check voice" (T28.3) --------------------------------------------------------------------------------------

pub type CheckProgress = Arc<dyn Fn(&[CheckStep]) + Send + Sync>;

/// Clears `checking`, also when the check is abandoned.
struct Checking(Arc<Inner>);

impl Drop for Checking {
    fn drop(&mut self) {
        self.0.checking.store(false, Ordering::SeqCst);
    }
}

/// A recording of the PC microphone for the check: `max` long, or until the pause after speech when `to_the_pause`.
#[cfg(windows)]
fn record_for_check(wanted: Option<String>, max: Duration, to_the_pause: bool) -> Result<Recording, StepError> {
    // The test input of `capture` (T21.11) stands in for the microphone here too.
    if let Some(bytes) = std::env::var_os("DECKA_VOICE_WAV").and_then(|p| std::fs::read(p).ok()) {
        return Ok(Recording { pcm: bytes.get(44..).unwrap_or_default().to_vec(), level: 0.5, sound: true, speech: true });
    }
    let failed = |e: String| {
        let error_key = if e == msg::NO_MIC { key::NO_MIC } else if e == msg::NO_SOUND { key::NO_SOUND } else { key::MIC_UNAVAILABLE };
        StepError { reason: Some(Reason::Mic), error: e, error_key: Some(error_key) }
    };
    let got = Arc::new(Mutex::new(Recording::default()));
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let (sink, done, broke) = (got.clone(), tx.clone(), tx);
    let microphone = mic::start(
        wanted,
        false,
        move |fed| {
            let mut r = sink.lock().unwrap();
            r.pcm.extend_from_slice(&fed.pcm);
            r.level = r.level.max(fed.level);
            r.sound |= fed.decision.rms >= mic::DIGITAL_SILENCE;
            r.speech |= fed.decision.speech_ms > 0;
            if to_the_pause && fed.verdict != Verdict::Continue {
                let _ = done.send(Ok(()));
            }
        },
        move |e| {
            let _ = broke.send(Err(e));
        },
        |_| {},
    )
    .map_err(failed)?;
    let ended = rx.recv_timeout(max);
    drop(microphone);
    let recording = got.lock().unwrap().clone();
    match ended {
        Ok(Err(e)) if !recording.sound => Err(failed(e)),
        _ => Ok(recording),
    }
}

#[cfg(not(windows))]
fn record_for_check(_: Option<String>, _: Duration, _: bool) -> Result<Recording, StepError> {
    Err(StepError::reason(Reason::Mic))
}

struct HostCheck {
    inner: Arc<Inner>,
    progress: CheckProgress,
}

impl HostCheck {
    fn record(&self, max: Duration, to_the_pause: bool) -> BoxFut<Result<Recording, StepError>> {
        let wanted = self.inner.service.status().mic;
        Box::pin(async move { tokio::task::spawn_blocking(move || record_for_check(wanted, max, to_the_pause)).await.unwrap_or_else(|_| Err(StepError::reason(Reason::Mic))) })
    }
}

impl CheckPorts for HostCheck {
    fn env(&self) -> (Arc<Table>, Context) {
        Ports::env(&*self.inner)
    }

    fn record_level(&self) -> BoxFut<Result<Recording, StepError>> {
        self.record(Duration::from_secs(3), false)
    }

    fn start_stt(&self) -> BoxFut<Result<(), StepError>> {
        let sup = self.inner.service.supervisor().clone();
        Box::pin(async move { sup.ensure_stt().await.map(|_| ()).map_err(|f| StepError::reason(f.reason)) })
    }

    fn record_phrase(&self) -> BoxFut<Result<Recording, StepError>> {
        self.record(Duration::from_secs(10), true)
    }

    fn transcribe(&self, pcm: Vec<u8>) -> BoxFut<Result<String, StepError>> {
        let (sup, lang) = (self.inner.service.supervisor().clone(), self.inner.language());
        // The check hears a phrase the way a command is heard.
        let words = self.inner.table(lang).stt_prompt();
        Box::pin(async move { sup.transcribe_prompted(&pcm, lang, Some(&words)).await.map_err(|e| sup.failure_of(Server::Stt).map_or_else(|| StepError::text(e, None), |f| StepError::reason(f.reason))) })
    }

    fn ask_model(&self, phrase: String, ctx: Context) -> BoxFut<Result<RawCall, StepError>> {
        let (sup, lang) = (self.inner.service.supervisor().clone(), self.inner.language());
        Box::pin(async move {
            // Starting it waits for `/health`.
            let (port, secret) = sup.ensure_llm().await.map_err(|f| StepError::reason(f.reason))?;
            let answer = super::llm::ask_at(port, &secret, &phrase, &ctx, lang).await;
            sup.touch();
            answer.map(|(call, _)| call).ok_or_else(|| StepError::text(msg::DIDNT_GET, Some(key::DIDNT_GET)))
        })
    }

    fn progress(&self, steps: &[CheckStep]) {
        (self.progress)(steps);
    }
}

impl Ports for Inner {
    fn env(&self) -> (Arc<Table>, Context) {
        let lang = self.language();
        let mut ctx = build_context(lang, &self.agent.actions(), &self.agent.layout(), &self.installed.lock().unwrap());
        let (installed, on) = (self.service.vision_installed(), self.agent.voice_context_enabled());
        ctx.vision_ready = installed && on;
        ctx.vision_off = installed && !on;
        ctx.heavy = fresh_heavy(&self.heavy);
        ctx.ha = self.agent.ha_voice_entities();
        ctx.no_reply = self.answers.lock().unwrap().fresh(Instant::now()).is_none();
        ctx.weather_place = self.agent.store().get("weather").get("place").and_then(|p| p.as_str()).map(str::to_string);
        ctx.guide_running = self.screen.get().is_some_and(|s| s.is_running());
        // T29.P: the page a phone shows; without a phone, the page the voice went to last.
        ctx.current_page = self.agent.voice().shown_page().or_else(|| self.current_page.lock().unwrap().clone());
        ctx.speech_interrupted = self.interrupted.load(Ordering::SeqCst);
        (self.table(lang), ctx)
    }

    fn dictating(&self) -> bool {
        self.dictation_request.load(Ordering::SeqCst) || dictation_waiting(*self.dictate_until.lock().unwrap(), Instant::now())
    }

    fn transcribe(&self, pcm: Vec<u8>) -> BoxFut<Result<String, String>> {
        let (sup, lang, driver) = (self.service.supervisor().clone(), self.language(), self.driver().clone());
        // A dictation spells the words of the dictionary and, in `clean` mode, goes through the model (13.1).
        let dictating = self.dictating();
        let settings = self.dictation.lock().unwrap().clone();
        // T32.H.2: a command is recognised with the words of frequent commands to expect.
        let words = self.table(self.language()).stt_prompt();
        Box::pin(async move {
            // Less than a tenth of a second is a key click, not speech.
            if pcm.len() < super::MAX_CHUNK_BYTES / 2 {
                return Ok(String::new());
            }
            // The warm-up of the recognition has not finished by the end of the phrase: only now the pill says so.
            if sup.state() == SupState::Loading && !sup.is_up(Server::Stt) {
                driver.handle_event(Event::ModelLoading);
            }
            // A recognition that could not start has a fixed reason (T28.2); anything else keeps its own text.
            let reason = |e: String| sup.failure_of(Server::Stt).map_or(e, |f| super::pipeline::failure_msg(f.reason).to_string());
            // T29.2: a phrase that the recognition repeats is kept once.
            if !dictating {
                let heard = sup.transcribe_prompted(&pcm, lang, Some(&words)).await.map(|t| super::transcript::collapse_repeats(&t)).map_err(reason)?;
                return Ok(if super::transcript::echoes_prompt(&heard, &words) { String::new() } else { heard });
            }
            let prompt = dictation::whisper_prompt(&settings.dictionary);
            let raw = sup.transcribe_prompted(&pcm, lang, prompt.as_deref()).await.map(|t| super::transcript::collapse_repeats(&t)).map_err(reason)?;
            if settings.mode == Mode::Clean && !raw.is_empty() {
                Ok(super::assist::clean_dictation(&sup, lang, &raw, &settings.dictionary).await)
            } else {
                Ok(raw)
            }
        })
    }

    fn ask_llm(&self, text: String, mut ctx: Context) -> BoxFut<Option<(RawCall, Option<String>)>> {
        let (sup, lang) = (self.service.supervisor().clone(), self.language());
        // Up to 20 facts that fit the phrase go with it (13.6); the phrase itself is not stored.
        ctx.memory = self.agent.memory().for_phrase(&text);
        let driver = self.driver().clone();
        Box::pin(async move {
            // The recognition may be ready long before the model: the wait for the model has its own, longer watchdog.
            if !sup.is_up(Server::Llm) && sup.failure_of(Server::Llm).is_none() {
                driver.handle_event(Event::ModelLoading);
            }
            super::llm::ask(&sup, lang, &text, &ctx).await
        })
    }

    fn model_failure(&self) -> Option<&'static str> {
        self.service.supervisor().failure_of(Server::Llm).map(|f| super::pipeline::failure_msg(f.reason))
    }

    fn execute(&self, call: ToolCall) -> BoxFut<Result<String, String>> {
        let agent = self.agent.clone();
        let (dictate, timers, goto_seq) = (self.dictate_until.clone(), self.timers.clone(), self.goto_seq.clone());
        let answers = self.answers.clone();
        let (slow, slow_since, slow_dir) = (self.slow.clone(), self.slow_since.clone(), self.agent.config_dir().to_path_buf());
        let handle = self.handle.clone();
        let (sup, lang, service) = (self.service.supervisor().clone(), self.language(), self.service.clone());
        let screen = self.screen.get().cloned();
        let focus = self.focus.clone();
        let download = self.download.clone();
        let (current_page, settings) = (self.current_page.clone(), self.settings.get().cloned());
        let heavy = self.heavy.clone();
        let focus_timer = self.focus_timer.clone();
        let driver = self.driver().clone();
        Box::pin(async move {
            // T29.P: while the model loads, the PC is busy and a window is read only in part ("did not find it"): the
            // search on the screen waits for the model, and the island says so.
            if matches!(call, ToolCall::FindOnScreen { .. } | ToolCall::UiAct { .. }) && !sup.is_up(Server::Llm) && sup.failure_of(Server::Llm).is_none() {
                driver.handle_event(Event::ModelLoading);
                // A model that cannot start does not stop the search.
                let _ = sup.ensure_llm().await;
                driver.handle_event(Event::ModelReady);
            }
            if let ToolCall::RunAction { action_id } = &call {
                return agent.execute(action_id).await.map(|_| String::new()).map_err(|(_, m)| m);
            }
            // Any command but a continuation ends the context of the last answer (T26.3).
            if !matches!(call, ToolCall::TextTransform { .. } | ToolCall::ReplyAction { .. }) {
                answers.lock().unwrap().clear();
            }
            match &call {
                ToolCall::NotesRecord { start } => {
                    let start = start.unwrap_or(!agent.notes().status().active);
                    let said = tokio::task::spawn_blocking(move || {
                        if !start {
                            return Ok(if agent.notes().stop() { answer("voice.reply.note_stopped", None, "Note stopped, writing it down") } else { answer("voice.reply.note_none", None, "No note is being recorded") });
                        }
                        Ok::<_, String>(if agent.note_start()? { answer("voice.reply.note_first", None, WARN_OTHERS) } else { answer("voice.reply.note_started", None, "Recording a note") })
                    })
                    .await
                    .map_err(|e| format!("The action did not finish: {e}"))??;
                    return Ok(super::status::encode(&said));
                }
                ToolCall::Ha { entity_id, service, name } => {
                    // The choice is looked at again: it may have changed while the question was open.
                    if !agent.ha_voice_entities().iter().any(|e| e.0 == *entity_id) {
                        return Err("That device is not chosen for voice control".into());
                    }
                    agent.ha_call(entity_id, *service).await?;
                    return Ok(super::status::encode(&answer("voice.reply.ha_done", Some(name), &format!("Done: {name}"))));
                }
                ToolCall::MemoryTop => {
                    let list = tokio::task::spawn_blocking(super::heavy::look).await.map_err(|e| format!("The action did not finish: {e}"))?;
                    let said = if list.is_empty() {
                        answer("voice.reply.memory_none", None, "No program to show")
                    } else {
                        let text = super::heavy::list_text(&list);
                        super::status::Answer { key: "voice.reply.memory_top", args: [("list".to_string(), text.clone())].into(), text }
                    };
                    *heavy.lock().unwrap() = Some(HeavyList { at: Instant::now(), list, shown: true });
                    return Ok(super::status::encode(&said));
                }
                ToolCall::CloseHeavy { exe, name, force } => {
                    let (exe, name, force) = (exe.clone(), name.clone(), *force);
                    return tokio::task::spawn_blocking(move || {
                        use super::heavy::Closed;
                        Ok::<_, String>(match super::heavy::close(&super::heavy::WinOps, &exe, force)? {
                            Closed::Yes => super::status::encode(&answer("voice.reply.closed", Some(&name), &format!("Closed {name}"))),
                            Closed::Still => super::heavy::force_reply(&exe, &name),
                        })
                    })
                    .await
                    .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::Volume(VolumeOp::Set(v)) => {
                    let v = *v;
                    return tokio::task::spawn_blocking(move || crate::audio::set_volume(v).map(|_| winops::volume_text(v)))
                        .await
                        .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::SwitchApp { name, program } | ToolCall::CloseApp { name, program } => {
                    let (name, program, close) = (name.clone(), program.clone(), matches!(call, ToolCall::CloseApp { .. }));
                    return tokio::task::spawn_blocking(move || {
                        let done = if close { winops::close_app(&program, &name) } else { winops::switch_app(&program, &name) };
                        done.map(|_| if close { format!("Closing {name}") } else { format!("Switched to {name}") })
                    })
                    .await
                    .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::WindowOtherMonitor => {
                    return tokio::task::spawn_blocking(winops::move_to_other_monitor).await.map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::WhichMonitor => {
                    return tokio::task::spawn_blocking(winops::which_monitor).await.map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::Status { topic } => {
                    let store = agent.store();
                    let sections = super::status::Sections {
                        weather: store.get("weather"),
                        sys: store.get("sys"),
                        battery: store.get("battery"),
                        media: store.get("media"),
                        ai: store.get("ai"),
                        focus: store.get("focus"),
                        reminder: agent.assistant().upcoming(1).into_iter().next(),
                    };
                    return Ok(super::status::encode(&super::status::answer_for(*topic, &sections, lang, &crate::assistant::Local::now())));
                }
                ToolCall::FindOnScreen { .. } | ToolCall::UiAct { .. } | ToolCall::Guide { .. } | ToolCall::GuideRecord { .. } | ToolCall::GuideControl { .. } | ToolCall::ModeRun { .. } | ToolCall::ModeSave { .. } => {
                    let Some(screen) = screen else { return Err("The screen help is not ready".into()) };
                    return match call.clone() {
                        ToolCall::FindOnScreen { query } => tokio::task::spawn_blocking(move || screen.find(query.as_deref())).await.map_err(|e| format!("The action did not finish: {e}"))?,
                        ToolCall::UiAct { target, action, confirmed } => tokio::task::spawn_blocking(move || screen.act(target.as_deref(), action, confirmed)).await.map_err(|e| format!("The action did not finish: {e}"))?,
                        ToolCall::Guide { goal } => {
                            // The model reads the window and answers: the island shows "~N s" for as long as it thinks (slow path).
                            let asker = LlamaAsker { sup: sup.clone(), slow_since: slow_since.clone(), slow: slow.clone(), dir: slow_dir.clone() };
                            screen.guide(&goal, super::llm::language_name(lang), &asker).await
                        }
                        ToolCall::GuideRecord { goal } => screen.record(&goal).await,
                        ToolCall::GuideControl { kind } => {
                            if kind == super::tools::GuideCtl::Stop {
                                super::speak::stop();
                            }
                            screen.control(kind)
                        }
                        // T27.10 (modes) is run by the application: it knows the windows.
                        ToolCall::ModeRun { name } => {
                            let done = super::modes::run(&slow_dir, &name)?;
                            // T30.4: a mode with focus minutes starts the focus timer; its end is a note on the island and
                            // a notification (a toast on the phone).
                            let Some(minutes) = super::modes::focus_of(&super::modes::load(&slow_dir), &name) else { return Ok(done) };
                            let generation = focus_timer.arm();
                            let (agent, timer) = (agent.clone(), focus_timer.clone());
                            handle.spawn(async move {
                                tokio::time::sleep(Duration::from_secs(u64::from(minutes) * 60)).await;
                                if timer.finish(generation, Instant::now()) {
                                    let title = super::modes::focus_over(lang).to_string();
                                    agent.push_notification(crate::proto::NotifyReq { title, body: None, level: Some(crate::model::NotifLevel::Info), source: Some("focus".into()) });
                                }
                            });
                            Ok(format!("{done}. Focus: {minutes} min"))
                        }
                        ToolCall::ModeSave { name } => super::modes::save(&slow_dir, &name),
                        _ => unreachable!(),
                    };
                }
                ToolCall::Calendar { when } => {
                    return Ok(super::status::encode(&crate::calendar::answer(&agent.store().get("calendar"), *when, &crate::assistant::Local::now(), crate::auth::now_ms() as i64)));
                }
                ToolCall::ReadAloud => {
                    return tokio::task::spawn_blocking(move || {
                        // T29.1: the selection of the place the user was in, then whatever is selected or under the mouse now.
                        #[cfg(windows)]
                        let remembered = target::Desk::remembered_selection(&focus).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
                        #[cfg(not(windows))]
                        let remembered: Option<String> = None;
                        let text = remembered.or_else(super::uia::selected_text).or_else(super::uia::text_at_cursor).ok_or_else(|| "Nothing to read: select some text first".to_string())?;
                        super::speak::read_aloud(&text).map(|_| String::new())
                    })
                    .await
                    .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::MediaPlay => {
                    let runner = agent.runner().clone();
                    return tokio::task::spawn_blocking(move || {
                        if crate::media_control::resume() == crate::media_control::SessionOutcome::Done {
                            return Ok(String::new());
                        }
                        // No session, or it refused: the key, as for a button of the deck.
                        runner.run("voice", &Action::Media { name: "voice".into(), toggle_var: None, op: MediaOp::PlayPause }).map(|_| String::new())
                    })
                    .await
                    .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::LyricsWindow { show } => {
                    agent.set_lyrics_hidden(!*show);
                    return Ok(if *show { "Lyrics shown".into() } else { "Lyrics hidden".into() });
                }
                ToolCall::TypeText { text } => {
                    *dictate.lock().unwrap() = None;
                    let text = text.clone();
                    return tokio::task::spawn_blocking(move || deliver(&focus, &text, false))
                        .await
                        .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::MemoryAdd { text } => {
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
                    return agent.memory().add(text, now).map(|f| format!("Remembered: {}", f.text));
                }
                ToolCall::MemoryRemove { text } => {
                    return agent.memory().remove_matching(text).map(|f| format!("Forgot: {}", f.text));
                }
                ToolCall::DayBrief => {
                    return day_brief(&agent, &sup, lang).await;
                }
                ToolCall::Remind { text, when, repeat } => {
                    let now = crate::assistant::Local::now();
                    let r = agent.assistant().add_reminder_repeat(text, when, *repeat, &now)?;
                    return Ok(format!("Reminder set: {}", crate::assistant::reminders_text(&[r], &now)));
                }
                ToolCall::RemindersShow => {
                    let now = crate::assistant::Local::now();
                    return Ok(crate::assistant::reminders_text(&agent.assistant().upcoming(5), &now));
                }
                ToolCall::ListAdd { list, item } => {
                    return agent.assistant().list_add(list, item).map(|name| format!("Added to {name}"));
                }
                ToolCall::ListShow { list } => {
                    return agent.assistant().list_show(list).map(|l| crate::assistant::list_text(&l));
                }
                ToolCall::ListRemove { list, item: Some(item) } => {
                    return agent.assistant().list_remove_item(list, item).map(|(name, gone)| format!("Removed {gone} from {name}"));
                }
                ToolCall::ListRemove { list, item: None } => {
                    return agent.assistant().list_delete(list).map(|name| format!("Deleted the list {name}"));
                }
                ToolCall::Calc { expr } => {
                    return super::calc::answer(expr).map_err(|e| super::calc::error_text(&e).to_string());
                }
                ToolCall::Ask { question } => {
                    let body = super::ask::request_body(question, lang);
                    return match super::assist::chat_text(&sup, &body, super::ask::ASK_TIMEOUT).await.as_deref().and_then(super::ask::accept) {
                        Some(super::ask::Answer::Text(t)) => Ok(t),
                        Some(super::ask::Answer::Unknown) => Ok(super::ask::web_reply(question)),
                        None => Err(super::textops::NO_ANSWER.to_string()),
                    };
                }
                ToolCall::ScreenAsk { question } => {
                    screen_pack(&agent, &service, &download).await?;
                    // The slow path: the island shows "Looking at the screen..." with the estimate until this ends.
                    *slow_since.lock().unwrap() = Some(Instant::now());
                    let started = Instant::now();
                    let answer = screen_ask(&sup, lang, question).await;
                    *slow_since.lock().unwrap() = None;
                    if answer.is_ok() {
                        let mut tracker = slow.lock().unwrap();
                        tracker.record(started.elapsed().as_millis() as u64);
                        tracker.save(&slow_dir);
                    }
                    return answer;
                }
                ToolCall::TextTransform { op, lang: target, text } => {
                    return run_transform(&sup, &focus, &answers, lang, *op, target.as_deref(), text.as_deref()).await;
                }
                ToolCall::ReplyAction { kind } => {
                    return reply_action(&sup, &focus, &answers, kind).await;
                }
                ToolCall::PasteText { text } => {
                    *dictate.lock().unwrap() = None;
                    let text = text.clone();
                    return tokio::task::spawn_blocking(move || deliver(&focus, &text, true))
                        .await
                        .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::Dictate => {
                    *dictate.lock().unwrap() = Some(Instant::now() + DICTATE_WINDOW);
                    return Ok("Say the text to type".into());
                }
                ToolCall::Timer { minutes, label } => {
                    if timers.fetch_add(1, Ordering::SeqCst) >= winops::MAX_TIMERS {
                        timers.fetch_sub(1, Ordering::SeqCst);
                        return Err("Too many timers are running".into());
                    }
                    let (minutes, label) = (*minutes, label.clone());
                    handle.spawn(async move {
                        tokio::time::sleep(Duration::from_secs(u64::from(minutes) * 60)).await;
                        let (title, body) = winops::timer_notice(minutes, label.as_deref());
                        agent.push_notification(crate::proto::NotifyReq { title, body: Some(body), level: Some(crate::model::NotifLevel::Info), source: Some("voice".into()) });
                        timers.fetch_sub(1, Ordering::SeqCst);
                    });
                    return Ok(winops::timer_set_text(minutes));
                }
                ToolCall::AddButton { catalog_id, page_id } => {
                    let (new_actions, new_layout) = super::addbutton::build(lang, catalog_id, page_id, &agent.actions(), &agent.layout())?;
                    let name = super::addbutton::catalog(lang).into_iter().find(|e| e.id == *catalog_id).map(|e| e.name).unwrap_or_default();
                    // The checked saves: a file that is not right is refused as a whole. The action goes first, the button needs it.
                    return tokio::task::spawn_blocking(move || {
                        let refused = |r: crate::agent::SaveResult| match r {
                            crate::agent::SaveResult::Saved => Ok(()),
                            crate::agent::SaveResult::Rejected(issues) => Err(format!("Not saved: {}", issues.first().map_or_else(String::new, |i| i.message.clone()))),
                        };
                        refused(agent.save_actions(new_actions))?;
                        refused(agent.save_layout(new_layout))?;
                        Ok(super::status::encode(&super::status::Answer { key: "voice.reply.button_added", args: [("name".to_string(), name.clone())].into(), text: format!("Added the button {name}") }))
                    })
                    .await
                    .map_err(|e| format!("The action did not finish: {e}"))?;
                }
                ToolCall::SystemTheme(wanted) => {
                    let wanted = *wanted;
                    let light = tokio::task::spawn_blocking(move || crate::wintheme::apply(wanted)).await.map_err(|e| format!("The action did not finish: {e}"))??;
                    let (key, text) = if light { ("voice.reply.theme_light", "Switched Windows to the light theme") } else { ("voice.reply.theme_dark", "Switched Windows to the dark theme") };
                    return Ok(super::status::encode(&super::status::Answer { key, args: Default::default(), text: text.into() }));
                }
                ToolCall::AppearanceSettings => {
                    let Some(open) = settings else { return Err("Open Decka settings to change its theme".into()) };
                    open("appearance");
                    return Ok(super::status::encode(&super::status::Answer { key: "voice.reply.theme_decka", args: Default::default(), text: "The theme of Decka is chosen in its settings".into() }));
                }
                ToolCall::SetupButton { catalog_id } => {
                    let Some(open) = settings else { return Err("Open Decka settings to add this button".into()) };
                    let name = super::addbutton::catalog(lang).into_iter().find(|e| e.id == *catalog_id).map(|e| e.name).unwrap_or_default();
                    open(&format!("keys:catalog:{catalog_id}"));
                    return Ok(super::status::encode(&super::status::Answer { key: "voice.reply.setup_button", args: [("name".to_string(), name.clone())].into(), text: format!("Finish the button {name} in the settings") }));
                }
                ToolCall::GoPage { page_id, name } => {
                    *current_page.lock().unwrap() = Some(page_id.clone());
                    let value = goto_value(page_id, goto_seq.fetch_add(1, Ordering::SeqCst) + 1);
                    agent.store().update_vars(|vars| {
                        vars.insert("voice.goto".to_string(), VarValue::Str(value));
                    });
                    return Ok(format!("Page {name}"));
                }
                _ => {}
            }
            let (action, text) = tool_action(&call)?;
            let runner = agent.runner().clone();
            tokio::task::spawn_blocking(move || runner.run("voice", &action))
                .await
                .map_err(|e| format!("The action did not finish: {e}"))??;
            Ok(text)
        })
    }

    fn capture(&self, on: bool) {
        // T29.3: other sounds are quiet for as long as the PC listens.
        self.ducker.hold(super::duck::Who::Microphone, on && self.agent.voice_duck_enabled());
        #[cfg(windows)]
        {
            let mut slot = self.mic.lock().unwrap();
            if !on {
                // Closing the device waits for the audio thread; it must never happen on the thread that handles the event
                // (the main thread of the application in the case of `voice_stop`): a thread of its own does it.
                if let Some(m) = slot.take() {
                    std::thread::spawn(move || drop(m));
                }
                return;
            }
            // Test input (T21.11): `DECKA_VOICE_WAV` = a 16 kHz mono PCM16 WAV that is fed instead of the microphone, in real time.
            if let Some(path) = std::env::var_os("DECKA_VOICE_WAV") {
                let read = std::fs::read(&path);
                mic::log_line(&format!("test wav: {}", match &read { Ok(b) => format!("{} bytes", b.len()), Err(e) => e.to_string() }));
                if let Ok(bytes) = read {
                    let pcm = bytes.get(44..).unwrap_or_default().to_vec();
                    let (driver, id) = (self.driver().clone(), self.driver().state().id);
                    std::thread::spawn(move || {
                        for chunk in pcm.chunks(super::MAX_CHUNK_BYTES) {
                            std::thread::sleep(Duration::from_millis(100));
                            let s = driver.state();
                            if !s.is_hearing() || s.id != id {
                                return;
                            }
                            driver.push_audio(chunk);
                        }
                        std::thread::sleep(Duration::from_millis(300));
                        let s = driver.state();
                        if s.is_hearing() && s.id == id {
                            driver.handle_event(Event::AudioEnd);
                        }
                    });
                    return;
                }
            }
            let wanted = self.service.status().mic;
            let started = {
                let (feed, fail, service) = (self.driver().clone(), self.driver().clone(), self.service.clone());
                let pill_self = self.pill.clone();
                let hotkey = self.service.status().hotkey;
                let last_info = self.last.lock().unwrap().clone();
                self.chunks.store(0, Ordering::SeqCst);
                mic::ring_clear();
                let (counter, decided) = (self.chunks.clone(), Arc::new(AtomicBool::new(false)));
                // The decisions of the first requests after the start (or all of them with `DECKA_VOICE_DEBUG`) go to voice.log.
                let log = std::env::var_os("DECKA_VOICE_DEBUG").is_some() || LOGGED_REQUESTS.fetch_add(1, Ordering::SeqCst) < LOGGED_FIRST;
                mic::start(
                    wanted,
                    log,
                    move |fed| {
                        counter.fetch_add(1, Ordering::SeqCst);
                        let line = mic::decision_line(&fed.decision, fed.verdict);
                        if log {
                            mic::log_line(&line);
                        }
                        // The last 50 decisions are kept in memory and dumped to voice.log on NoSpeech / CaptureFailed.
                        mic::ring_push(line);
                        feed.note_speech(fed.decision.speech_ms);
                        feed.push_audio(&fed.pcm);
                        // The wave of the pill follows the voice (a chunk is 100 ms, so at most 10 times a second).
                        if let Some(info) = last_info.as_ref().filter(|i| i.state == "listening") {
                            if let Some(cmd) = pill_cmd(info, &hotkey, fed.level) {
                                // Windows of the pill belong to the main thread: never wait for it on the audio thread.
                                let driver = feed.clone();
                                send_wave(move || driver.state(), pill_self.clone(), cmd, info.id.clone());
                            }
                        }
                        let event = match fed.verdict {
                            Verdict::Continue => return,
                            Verdict::End => Event::AudioEnd,
                            Verdict::NoSpeech => Event::NoSpeech,
                        };
                        // Once, and not on the audio thread: the event stops the capture, which waits for this very thread.
                        if feed.state().is_hearing() && !decided.swap(true, Ordering::SeqCst) {
                            let driver = feed.clone();
                            std::thread::spawn(move || driver.handle_event(event));
                        }
                    },
                    move |e| fail.handle_event(Event::CaptureFailed(e)),
                    move |name| {
                        // The microphone that gave sound after the silence of the first one becomes the chosen one.
                        let _ = service.set(VoiceSetArgs { mic: Some(name), ..Default::default() });
                    },
                )
            };
            match started {
                Ok(m) => {
                    *slot = Some(m);
                    // No chunk in a second: the device opened but gives no sound.
                    let (driver, counter, id) = (self.driver().clone(), self.chunks.clone(), self.driver().state().id);
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(MIC_SILENT_MS));
                        let s = driver.state();
                        if s.phase == Phase::Listening && s.id == id && counter.load(Ordering::SeqCst) == 0 {
                            driver.handle_event(Event::CaptureFailed("no audio chunks".into()));
                        }
                    });
                }
                Err(e) => {
                    drop(slot);
                    let text = if e == msg::NO_MIC { e } else { format!("Microphone: {e}") };
                    // After the effects of the current event are done: the state is `Listening` by then.
                    let driver = self.driver().clone();
                    std::thread::spawn(move || driver.handle_event(Event::CaptureFailed(text)));
                }
            }
        }
        #[cfg(not(windows))]
        let _ = on;
    }

    fn publish(&self, info: VoiceInfo) {
        let state = self.driver().state();
        let was_listening = self.last.lock().unwrap().as_ref().is_some_and(|i| i.state == "listening");
        *self.last.lock().unwrap() = Some(info.clone());
        // T29.1: where the user is as the request begins: text is read from and typed into this place.
        if info.state == "listening" && !was_listening {
            self.focus.remember();
            // T29.8: a new request ends the answer that is being read.
            self.interrupted.store(super::speak::is_speaking(), Ordering::SeqCst);
            super::speak::stop();
        }
        if info.state == "idle" {
            super::speak::stop();
        }
        // Warm-up: the servers start while the phrase is still being recorded (a no-op when they run already).
        if info.state == "listening" && !was_listening {
            let sup = self.service.supervisor().clone();
            self.handle.spawn(async move {
                if let Err(e) = sup.ensure_started().await {
                    mic::log_line(&format!("warm-up failed: {e}"));
                }
            });
        }
        // T29.4: a dictation of the PC shows the orb at the caret; a result that must be read becomes its bubble.
        if state.source != Some(Source::Phone) {
            let bubble = match info.state {
                "error" => info.reply.clone().map(|text| super::orb::Bubble { text, key: info.reply_key }),
                "done" if info.reply_key == Some(key::COPIED_PASTE) => info.reply.clone().map(|text| super::orb::Bubble { text, key: info.reply_key }),
                _ => None,
            };
            let appeared = self.orb.lock().unwrap().on_state(info.state, info.activity == Some("dictate"), bubble, Instant::now());
            if appeared {
                let orb = self.orb.clone();
                std::thread::spawn(move || {
                    if let Some(at) = super::orb::caret_anchor() {
                        orb.lock().unwrap().set_anchor(at);
                    }
                });
            }
        }
        if matches!(info.state, "idle" | "done" | "error") {
            // The request that was a dictation is over.
            self.dictation_request.store(false, Ordering::SeqCst);
        }
        self.agent.store().set("voice", serde_json::to_value(&info).unwrap_or(Value::Null));
        let shown = Instant::now();
        self.show(&info, 0.0);
        self.set_aux(&state);
        // The watchdog: a request waits in one stage for as long as `watchdog_ms` says, then it is cancelled.
        let seen = self.changes.fetch_add(1, Ordering::SeqCst) + 1;
        if matches!(info.state, "listening" | "loading" | "recognizing" | "thinking") {
            let (driver, changes) = (self.driver().clone(), self.changes.clone());
            let wait = watchdog_ms(&state.phase);
            mic::log_line(&format!("stage {} watchdog={}s pill={}ms", info.state, wait / 1000, shown.elapsed().as_millis()));
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(wait));
                if changes.load(Ordering::SeqCst) == seen {
                    driver.handle_event(Event::Watchdog);
                }
            });
        }
        // Idle always takes the pill away; a second, idempotent Destroy catches a window that was still being created.
        if info.state == "idle" {
            let (driver, pill) = (self.driver().clone(), self.pill.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                // Through the queue of the pill: a Destroy that overtakes a Show would leave the window open.
                pill_worker(Box::new(move || {
                    if driver.state().phase == Phase::Idle {
                        pill(PillCmd::Destroy);
                    }
                }));
            });
        }
        // A result stays visible for a moment, then the pipeline is idle again.
        if matches!(info.state, "done" | "error") {
            let (driver, id) = (self.driver().clone(), info.id.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(RESULT_SHOWN_MS));
                let s = driver.state();
                if s.id == id && matches!(s.phase, Phase::Done(_) | Phase::Failed(_)) {
                    driver.handle_event(Event::Reset);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SystemOp;

    fn state(phase: Phase, source: Source) -> State {
        State { phase, id: Some("0011223344556677".into()), source: Some(source), ..State::idle() }
    }

    fn info(state: &'static str, source: Option<Source>) -> VoiceInfo {
        VoiceInfo { state, id: Some("x".into()), source, heard: None, reply: None, reply_key: None, reply_args: Default::default(), confirm: None, activity: None, actions: Vec::new(), understood: None, executing: false }
    }

    /// T23.B: a wave of request N queued after the `Destroy` of request N must not bring the pill back.
    #[test]
    fn a_wave_queued_after_destroy_does_not_create_the_pill() {
        use std::sync::Mutex;
        let log: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let pill: PillHook = {
            let log = log.clone();
            Arc::new(move |c| log.lock().unwrap().push(if matches!(c, PillCmd::Destroy) { "destroy" } else { "show" }))
        };
        let shared = Arc::new(Mutex::new(state(Phase::Listening, Source::Pc)));
        let id = shared.lock().unwrap().id.clone();
        // The request ends: the state becomes idle and the destroy goes into the queue of the pill.
        {
            let (shared, pill) = (shared.clone(), pill.clone());
            pill_worker(Box::new(move || {
                *shared.lock().unwrap() = State::idle();
                pill(PillCmd::Destroy);
            }));
        }
        // The late wave of the same request is queued behind it.
        let s2 = shared.clone();
        send_wave(move || s2.lock().unwrap().clone(), pill.clone(), PillCmd::Show(json!({})), id);
        // A marker job: when it runs, everything before it has run.
        let (tx, rx) = std::sync::mpsc::channel();
        pill_worker(Box::new(move || {
            let _ = tx.send(());
        }));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(*log.lock().unwrap(), ["destroy"]);
    }

    #[test]
    fn the_hotkey_starts_ends_and_cancels() {
        let id = || "aaaaaaaaaaaaaaaa".to_string();
        assert_eq!(press_event(&State::idle(), id()), Some(Event::Start { id: id(), source: Source::Pc }));
        assert_eq!(press_event(&state(Phase::Done("x".into()), Source::Pc), id()), Some(Event::Start { id: id(), source: Source::Pc }));
        assert_eq!(press_event(&state(Phase::Listening, Source::Pc), id()), Some(Event::AudioEnd), "the second press ends the recording");
        assert_eq!(press_event(&state(Phase::Listening, Source::Phone), id()), None, "a phone recording is not ended from the PC");
        assert_eq!(press_event(&state(Phase::Thinking, Source::Pc), id()), Some(Event::Cancel));
        assert_eq!(press_event(&state(Phase::Executing, Source::Pc), id()), None);
    }

    #[test]
    fn esc_cancels_or_says_no_and_enter_says_yes_only_in_a_confirmation() {
        let call = crate::voice::tools::Validated { call: ToolCall::Dictate, confirm: true, summary: "x".into(), key: None, args: Default::default() };
        let confirming = state(Phase::Confirming { call, deadline_ms: 1, ear: Ear::Open }, Source::Pc);
        assert_eq!(press_event(&confirming, "x".into()), Some(Event::AudioEnd), "the hotkey ends the spoken answer");
        assert_eq!(esc_event(&confirming), Some(Event::Confirm { id: "0011223344556677".into(), yes: false }));
        assert_eq!(enter_event(&confirming), Some(Event::Confirm { id: "0011223344556677".into(), yes: true }));
        assert_eq!(esc_event(&state(Phase::Listening, Source::Pc)), Some(Event::Cancel));
        assert_eq!(esc_event(&State::idle()), None);
        assert_eq!(enter_event(&state(Phase::Listening, Source::Pc)), None);
        assert_eq!(aux_for(&confirming), (true, true));
        assert_eq!(aux_for(&state(Phase::Listening, Source::Pc)), (true, false));
        assert_eq!(aux_for(&state(Phase::Listening, Source::Phone)), (false, false), "Esc is not taken for a phone request");
        assert_eq!(aux_for(&state(Phase::Done("x".into()), Source::Pc)), (false, false));
    }

    #[test]
    fn the_pill_follows_requests_of_the_pc_only() {
        let hk = vec!["ctrl".to_string(), "alt".to_string(), "space".to_string()];
        assert!(matches!(pill_cmd(&info("listening", Some(Source::Pc)), &hk, 0.5), Some(PillCmd::Show(_))));
        assert_eq!(pill_cmd(&info("listening", Some(Source::Phone)), &hk, 0.5), None);
        assert_eq!(pill_cmd(&info("idle", None), &hk, 0.0), Some(PillCmd::Destroy));
        let Some(PillCmd::Show(v)) = pill_cmd(&info("done", Some(Source::Pc)), &hk, 0.456) else { panic!() };
        assert_eq!((v["state"].as_str(), v["level"].as_f64(), v["hotkey"].as_str()), (Some("done"), Some(0.46), Some("Ctrl+Alt+Space")));
    }

    #[test]
    fn tools_map_to_the_existing_executors_and_the_rest_runs_elsewhere() {
        let (a, text) = tool_action(&ToolCall::OpenApp { name: "Calculator".into(), program: "C:\\calc.exe".into() }).unwrap();
        assert!(matches!(a, Action::Launch { ref program, args: None, .. } if program == "C:\\calc.exe"));
        assert_eq!(text, "Opened Calculator");
        assert!(matches!(tool_action(&ToolCall::System(SystemOp::Lock)).unwrap().0, Action::System { op: SystemOp::Lock, .. }));
        assert!(matches!(tool_action(&ToolCall::Volume(VolumeOp::Up)).unwrap().0, Action::Media { op: MediaOp::VolUp, .. }));
        assert!(matches!(tool_action(&ToolCall::OpenUrl { url: "https://a.b".into() }).unwrap().0, Action::OpenUrl { .. }));
        for stub in [ToolCall::Volume(VolumeOp::Set(30)), ToolCall::Dictate, ToolCall::WhichMonitor, ToolCall::Timer { minutes: 5, label: None }] {
            assert!(tool_action(&stub).is_err(), "{stub:?}");
        }
    }

    #[test]
    fn a_dictation_waits_for_a_minute_and_then_lapses() {
        let now = Instant::now();
        assert!(!dictation_waiting(None, now));
        assert!(dictation_waiting(Some(now + DICTATE_WINDOW), now));
        assert!(!dictation_waiting(Some(now), now), "the deadline itself is already over");
        assert!(!dictation_waiting(Some(now), now + Duration::from_secs(1)));
    }

    #[test]
    fn going_to_the_same_page_twice_is_a_new_value() {
        assert_ne!(goto_value("media", 1), goto_value("media", 2));
        assert_eq!(goto_value("media", 7), "media#7");
    }

    #[test]
    fn the_context_lists_buttons_confirmations_pages_and_programs() {
        let actions: ActionsFile = serde_json::from_str(r#"{"version":1,"actions":{"lock":{"type":"system","name":"Lock","op":"lock"}}}"#).unwrap();
        let layout: LayoutFile = serde_json::from_value(json!({
            "version": 1, "grid": {"cols": 7, "rows": 4},
            "display": {"dimAfterSec": 0, "dimLevel": 0.5, "sleepAfterSec": 0, "pixelShift": false, "night": {"enabled": false, "from": "22:00", "to": "07:00", "level": 0.3}},
            "pages": [
                {"id": "main", "name": "Main", "items": [
                    {"kind": "button", "id": "b1", "x": 0, "y": 0, "w": 1, "h": 1, "label": "Lock", "actionId": "lock", "confirm": true},
                    {"kind": "button", "id": "b2", "x": 1, "y": 0, "w": 1, "h": 1, "actionId": "lock"} ]},
                {"id": "pop", "name": "Popup", "popup": true, "items": []} ]
        }))
        .unwrap();
        let apps = vec![InstalledApp { name: "Calculator".into(), program: "C:\\calc.exe".into(), args: vec![], cwd: None, icon: None }];
        let c = build_context("kk", &actions, &layout, &apps);
        assert_eq!(c.lang, "kk");
        assert_eq!(c.buttons, [ButtonRef { label: "Lock".into(), action_id: "lock".into() }], "a button without a label cannot be named");
        assert_eq!(c.confirm_actions, ["lock"]);
        // T29.A: a key that ends the session asks by voice even when nobody marked it, also from inside a sequence.
        let risky: ActionsFile = serde_json::from_value(json!({ "version": 1, "actions": {
            "off": { "type": "system", "name": "Off", "op": "shutdown" },
            "bin": { "type": "system", "name": "Bin", "op": "empty_recycle_bin" },
            "night": { "type": "sequence", "name": "Night", "steps": [{ "delayMs": 10 }, { "actionId": "off" }] },
            "calm": { "type": "sequence", "name": "Calm", "steps": [{ "actionId": "tasks" }] },
            "tasks": { "type": "system", "name": "Tasks", "op": "task_manager" },
            "loop": { "type": "sequence", "name": "Loop", "steps": [{ "actionId": "loop" }] }
        } }))
        .unwrap();
        let mut asked = build_context("en", &risky, &layout, &[]).confirm_actions;
        asked.retain(|id| risky.actions.contains_key(id));
        asked.sort();
        assert_eq!(asked, ["bin", "night", "off"]);
        assert_eq!(c.pages, [PageRef { id: "main".into(), name: "Main".into() }], "popups are not pages to go to");
        assert_eq!((c.actions.len(), c.apps.len()), (1, 1));
    }

    #[test]
    fn every_language_has_a_table_that_parses() {
        for lang in crate::tray_text::LANGUAGES {
            let t = Table::parse(table_text(lang)).unwrap_or_else(|e| panic!("{lang}: {e}"));
            assert_eq!(&t.lang, lang);
        }
    }
}
