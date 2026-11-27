//! One voice request from the first word to the result (DATA_MODEL.md 12.4): record -> recognise -> match -> (confirm)
//! -> execute. `step` is a pure function over events; `Driver` runs its effects against `Ports` (whisper, the model, the
//! executors, the microphone), which tests replace with fakes.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;

use super::matcher::{Intent, Match, Table};
use super::tools::{validate, Context, RawCall, Resolved, ToolCall, Validated};

/// A risky call waits this long for `voice_confirm` (or a spoken yes/no), then it is cancelled.
pub const CONFIRM_TIMEOUT_MS: u64 = 10_000;
const MAX_TEXT: usize = 300;

/// Texts the user sees (PROTOCOL.md 3, DATA_MODEL.md 12.4).
pub mod msg {
    pub const DIDNT_CATCH: &str = "Didn't catch that";
    pub const DIDNT_GET: &str = "Didn't get that — try again";
    /// T28.2: why the model or the recognition does not work (the texts of `m28.err_*_short`).
    pub const FAIL_VRAM: &str = "Not enough GPU memory for the model";
    pub const FAIL_RAM: &str = "Not enough RAM for the model";
    pub const FAIL_MODEL_FILE: &str = "Voice model file is damaged";
    pub const FAIL_PORT: &str = "Voice server port is busy";
    pub const FAIL_CRASH: &str = "Voice server stopped unexpectedly";
    pub const FAIL_STT: &str = "Speech recognition failed to start";
    pub const CANCELLED: &str = "Cancelled";
    pub const DONE: &str = "Done";
    pub const NO_MIC: &str = "No microphone found";
    pub const MIC_UNAVAILABLE: &str = "Microphone is unavailable";
    pub const NO_SPEECH: &str = "Didn't hear anything";
    pub const NO_SOUND: &str = crate::voice::mic::NO_SOUND;
    pub const WRONG: &str = "Something went wrong, try again";
    pub const ASK_WEB: &str = "I can't know that offline, search the web?";
    /// T29.1: there was no field to type into; the text waits on the clipboard.
    pub const COPIED_PASTE: &str = "Copied: paste with Ctrl+V";
    pub use crate::voice::calc::{DIV_ZERO, NOT_NUMBER};
    pub use crate::voice::textops::{NO_ANSWER, NO_CONTEXT, SELECT_FIRST, TOO_LONG};
}

/// i18n keys of the fixed phrases (`shared/src/i18n`, `voice.reply.*`); the English text of `msg` is their fallback.
pub mod key {
    pub const DIDNT_CATCH: &str = "voice.reply.didnt_catch";
    pub const DIDNT_GET: &str = "voice.reply.didnt_get";
    pub const CANCELLED: &str = "voice.reply.cancelled";
    pub const DONE: &str = "voice.reply.done";
    pub const WHICH: &str = "voice.reply.which";
    pub const CANT: &str = "voice.reply.cant";
    pub const NO_MIC: &str = "voice.reply.no_mic";
    pub const MIC_UNAVAILABLE: &str = "voice.reply.mic_unavailable";
    pub const NO_SPEECH: &str = "voice.reply.no_speech";
    pub const NO_SOUND: &str = "voice.reply.no_sound";
    pub const WRONG: &str = "voice.reply.wrong";
    pub const SELECT_FIRST: &str = "voice.reply.select_text";
    pub const TOO_LONG: &str = "voice.reply.text_too_long";
    pub const NO_CONTEXT: &str = "voice.reply.no_context";
    pub const DIV_ZERO: &str = "voice.reply.div_zero";
    pub const NOT_NUMBER: &str = "voice.reply.calc_failed";
    /// A confirmation, not a reply: the question "search the web?" after an `ask` the model could not answer.
    pub const ASK_WEB: &str = "voice.confirm.ask_web";
    pub const COPIED_PASTE: &str = "voice.reply.copied_paste";
    pub const NO_ROOM: &str = "voice.reply.no_room";
    pub const FAIL_VRAM: &str = "m28.err_vram_short";
    pub const FAIL_RAM: &str = "m28.err_ram_short";
    pub const FAIL_MODEL_FILE: &str = "m28.err_model_file_short";
    pub const FAIL_PORT: &str = "m28.err_port_short";
    pub const FAIL_CRASH: &str = "m28.err_crash_short";
    pub const FAIL_STT: &str = "m28.err_stt_short";
}

/// The short text of a failure of the servers (T28.2); the island shows its key.
pub fn failure_msg(reason: crate::ai::failure::Reason) -> &'static str {
    use crate::ai::failure::Reason;
    match reason {
        Reason::Vram => msg::FAIL_VRAM,
        Reason::Ram => msg::FAIL_RAM,
        Reason::ModelFile => msg::FAIL_MODEL_FILE,
        Reason::Port => msg::FAIL_PORT,
        Reason::Crash => msg::FAIL_CRASH,
        Reason::Mic => msg::MIC_UNAVAILABLE,
        Reason::Stt => msg::FAIL_STT,
    }
}

/// "Understood: ..." (T28.3): what a call is going to do, as an i18n key with the English text as its fallback.
pub fn understood(v: &Validated) -> ConfirmInfo {
    let plain = |key: &'static str| (Some(key), BTreeMap::new());
    let with = |key: &'static str, name: &str, value: String| (Some(key), BTreeMap::from([(name.to_string(), value)]));
    let (key, args) = match &v.call {
        // The question of a screen request is about the download; what was understood is still a look at the screen.
        ToolCall::ScreenAsk { .. } => plain("voice.did.screen"),
        ToolCall::AddButton { .. } => plain("voice.did.button"),
        _ if v.confirm && v.key.is_some() => (v.key, v.args.clone()),
        ToolCall::OpenApp { name, .. } => with("voice.did.open", "name", name.clone()),
        ToolCall::SwitchApp { name, .. } => with("voice.did.switch", "name", name.clone()),
        ToolCall::CloseApp { name, .. } => with("voice.did.close", "name", name.clone()),
        ToolCall::RunAction { .. } => match v.args.get("name") {
            Some(name) => with("voice.did.run", "name", name.clone()),
            None => (None, BTreeMap::new()),
        },
        ToolCall::GoPage { name, .. } => with("voice.did.page", "name", name.clone()),
        ToolCall::SetupButton { .. } => plain("voice.did.button"),
        ToolCall::Screenshot(_) => plain("voice.did.screenshot"),
        ToolCall::SystemTheme(_) | ToolCall::AppearanceSettings => plain("voice.did.theme"),
        ToolCall::ModeRun { name } => with("voice.did.mode", "name", name.clone()),
        ToolCall::Timer { minutes, .. } => with("voice.did.timer", "minutes", minutes.to_string()),
        ToolCall::OpenUrl { .. } => plain("voice.did.web"),
        ToolCall::TypeText { .. } | ToolCall::PasteText { .. } | ToolCall::Dictate => plain("voice.did.type"),
        ToolCall::Volume(_) => plain("voice.did.volume"),
        ToolCall::Media(_) | ToolCall::MediaPlay => plain("voice.did.media"),
        ToolCall::FindOnScreen { .. } | ToolCall::UiAct { .. } | ToolCall::ReadAloud => plain("voice.did.screen"),
        ToolCall::Guide { .. } | ToolCall::GuideRecord { .. } | ToolCall::GuideControl { .. } => plain("voice.did.guide"),
        ToolCall::Ask { .. } | ToolCall::Calc { .. } | ToolCall::Status { .. } => plain("voice.did.question"),
        ToolCall::TextTransform { .. } | ToolCall::ReplyAction { .. } => plain("voice.did.text"),
        _ => (None, BTreeMap::new()),
    };
    ConfirmInfo { text: cut(&v.summary), key, args }
}

/// The i18n key of a fixed text of an executor (the English text is its fallback).
pub(crate) fn executor_key(text: &str) -> Option<&'static str> {
    match text {
        msg::COPIED_PASTE => Some(key::COPIED_PASTE),
        crate::voice::addbutton::NO_ROOM => Some(key::NO_ROOM),
        // T29.A: the fixed answers of the screen help, the memory and the window tools in the language of the user.
        "Did not find it on the screen" | "Not found on the screen" => Some("voice.reply.not_found_screen"),
        "No other candidates" => Some("voice.reply.no_candidates"),
        "Cannot read this window" | "No window to look at" => Some("voice.reply.cannot_read_window"),
        "Cannot see any buttons in this window" => Some("voice.reply.no_buttons"),
        "No guide is running" => Some("voice.reply.no_guide"),
        "A guide is already running" => Some("voice.reply.guide_busy"),
        "The assistant could not choose a step" | "The assistant did not answer" => Some("voice.reply.no_step"),
        "I do not remember that" => Some("voice.reply.no_memory"),
        "The memory is full: forget something first" => Some("voice.reply.memory_full"),
        "That fact is already there" => Some("voice.reply.known_fact"),
        "There is no active window" => Some("voice.reply.no_window"),
        "There is only one monitor" => Some("voice.reply.one_monitor"),
        "Too many timers are running" => Some("voice.reply.timers_full"),
        "That button cannot be added by voice" => Some("voice.reply.cannot_add_button"),
        crate::notes::ALREADY => Some("voice.reply.note_already"),
        crate::voice::heavy::NOT_ALLOWED => Some("voice.reply.close_not_allowed"),
        crate::voice::heavy::NOT_RUNNING => Some("voice.reply.close_not_running"),
        crate::voice::heavy::NOT_CLOSED => Some("voice.reply.close_failed"),
        crate::notes::NO_DISK => Some("voice.reply.note_no_disk"),
        msg::SELECT_FIRST => Some(key::SELECT_FIRST),
        msg::TOO_LONG => Some(key::TOO_LONG),
        msg::NO_CONTEXT => Some(key::NO_CONTEXT),
        msg::NO_ANSWER => Some(key::DIDNT_GET),
        msg::DIV_ZERO => Some(key::DIV_ZERO),
        msg::NOT_NUMBER => Some(key::NOT_NUMBER),
        msg::FAIL_VRAM => Some(key::FAIL_VRAM),
        msg::FAIL_RAM => Some(key::FAIL_RAM),
        msg::FAIL_MODEL_FILE => Some(key::FAIL_MODEL_FILE),
        msg::FAIL_PORT => Some(key::FAIL_PORT),
        msg::FAIL_CRASH => Some(key::FAIL_CRASH),
        msg::FAIL_STT => Some(key::FAIL_STT),
        msg::MIC_UNAVAILABLE => Some(key::MIC_UNAVAILABLE),
        _ => None,
    }
}

/// A fixed phrase as an i18n key with its arguments (PROTOCOL.md 3: `replyKey` / `replyArgs`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyKey {
    pub key: &'static str,
    pub args: BTreeMap<String, String>,
}

impl ReplyKey {
    pub fn plain(key: &'static str) -> ReplyKey {
        ReplyKey { key, args: BTreeMap::new() }
    }

    pub fn with(key: &'static str, name: &str, value: &str) -> ReplyKey {
        ReplyKey { key, args: BTreeMap::from([(name.to_string(), value.to_string())]) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Pc,
    Phone,
}

/// The microphone while a question waits for its "yes" or "no" (T29.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ear {
    /// Only the buttons and the keys answer (a phone, or the microphone failed).
    Off,
    /// The PC records the answer.
    Open,
    /// The recorded answer is being recognised.
    Busy,
}

/// The recognition of an answer that was said just before the deadline still gets this long.
const ANSWER_GRACE_MS: u64 = 5_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Idle,
    Listening,
    Transcribing,
    /// The servers are starting (the first request after a pause).
    Loading,
    /// Waiting for the model's answer.
    Thinking,
    Confirming { call: Validated, deadline_ms: u64, ear: Ear },
    Executing,
    Done(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub phase: Phase,
    pub id: Option<String>,
    pub source: Option<Source>,
    pub heard: Option<String>,
    /// The model's own short answer; replaces the standard result text.
    pub say: Option<String>,
    /// The key of the fixed phrase in `Done` / `Failed`; `None` for free text (the model's answer, an executor's result).
    pub reply_key: Option<ReplyKey>,
    /// What the request is, for the label of the pill (DATA_MODEL.md 13.7): `dictate` (a dictation) or `screen` (the screen is being looked at).
    pub activity: Option<&'static str>,
    /// T26.3: the shown answer can be continued; the window of the PC offers `followup::ACTIONS` under it.
    pub actions: bool,
    /// T28.3: what the phrase was understood as (`Heard` -> `Understood` -> doing it), set when a call starts.
    pub understood: Option<ConfirmInfo>,
    /// T29.P: a running call waits for the model to load first (a search on the screen): shown as "loading".
    pub model_wait: bool,
}

impl State {
    pub fn idle() -> State {
        State { phase: Phase::Idle, id: None, source: None, heard: None, say: None, reply_key: None, activity: None, actions: false, understood: None, model_wait: false }
    }

    /// Whether sound is being recorded now: the phrase, or the answer to a question.
    pub fn is_hearing(&self) -> bool {
        matches!(self.phase, Phase::Listening | Phase::Confirming { ear: Ear::Open, .. })
    }
}

/// The `voice` section of the state (PROTOCOL.md 3).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VoiceInfo {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
    #[serde(rename = "replyKey", skip_serializing_if = "Option::is_none")]
    pub reply_key: Option<&'static str>,
    #[serde(rename = "replyArgs", skip_serializing_if = "BTreeMap::is_empty")]
    pub reply_args: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm: Option<ConfirmInfo>,
    /// `dictate` or `screen` while the request runs: the pill says `Dictating...` or `Looking at the screen`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<&'static str>,
    /// T26.3: what can be done with the shown answer. Only for the window of the PC (`pill_payload`); the phones do not get it.
    #[serde(skip)]
    pub actions: Vec<&'static str>,
    /// T28.3: what was understood and whether it is being done now; for the window of the PC only.
    #[serde(skip)]
    pub understood: Option<ConfirmInfo>,
    #[serde(skip)]
    pub executing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfirmInfo {
    /// The English text: free text or the fallback of `key`.
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<&'static str>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
}

fn cut(s: &str) -> String {
    s.chars().take(MAX_TEXT).collect()
}

impl State {
    pub fn info(&self) -> VoiceInfo {
        let (state, reply, confirm) = match &self.phase {
            Phase::Idle => ("idle", None, None),
            Phase::Listening => ("listening", None, None),
            Phase::Transcribing => ("recognizing", None, None),
            Phase::Executing if self.model_wait => ("loading", None, None),
            Phase::Thinking | Phase::Executing => ("thinking", None, None),
            Phase::Loading => ("loading", None, None),
            Phase::Confirming { call, .. } => {
                ("confirm", None, Some(ConfirmInfo { text: cut(&call.summary), key: call.key, args: call.args.clone() }))
            }
            Phase::Done(r) => ("done", Some(cut(r)), None),
            Phase::Failed(r) => ("error", Some(cut(r)), None),
        };
        let idle = state == "idle";
        VoiceInfo {
            state,
            id: self.id.clone().filter(|_| !idle),
            source: self.source.filter(|_| !idle),
            heard: self.heard.as_deref().map(cut),
            reply,
            reply_key: self.reply_key.as_ref().filter(|_| matches!(state, "done" | "error")).map(|k| k.key),
            reply_args: self.reply_key.as_ref().filter(|_| matches!(state, "done" | "error")).map(|k| k.args.clone()).unwrap_or_default(),
            confirm,
            activity: self.activity.filter(|_| matches!(state, "listening" | "recognizing" | "thinking" | "loading")),
            actions: if self.actions && state == "done" { super::followup::ACTIONS.to_vec() } else { Vec::new() },
            understood: self.understood.clone().filter(|_| !idle),
            // While the call waits for the model the island says that, not "doing it".
            executing: self.phase == Phase::Executing && !self.model_wait,
        }
    }
}

/// What `voice.log` says about an execution: never the text of the result or of the error.
fn executed_line(r: &Result<String, String>) -> &'static str {
    match r {
        Ok(_) => "executed: ok",
        Err(_) => "executed: error",
    }
}

/// The variant of an event without its payload (for `voice.log`).
fn event_name(e: &Event) -> &'static str {
    match e {
        Event::Start { source: Source::Pc, .. } => "Start(pc)",
        Event::Start { .. } => "Start(phone)",
        Event::AudioEnd => "AudioEnd",
        Event::Cancel => "Cancel",
        Event::ModelLoading => "ModelLoading",
        Event::ModelReady => "ModelReady",
        Event::Transcript(_) => "Transcript",
        Event::CaptureFailed(_) => "CaptureFailed",
        Event::NoSpeech => "NoSpeech",
        Event::Watchdog => "Watchdog",
        Event::LlmAnswer(_) => "LlmAnswer",
        Event::Confirm { .. } => "Confirm",
        Event::Tick { .. } => "Tick",
        Event::Executed(_) => "Executed",
        Event::Reset => "Reset",
        #[allow(unreachable_patterns)]
        _ => "Other",
    }
}

fn phase_name(p: &Phase) -> &'static str {
    match p {
        Phase::Idle => "Idle",
        Phase::Listening => "Listening",
        Phase::Transcribing => "Transcribing",
        Phase::Loading => "Loading",
        Phase::Thinking => "Thinking",
        Phase::Confirming { .. } => "Confirming",
        Phase::Executing => "Executing",
        Phase::Done(_) => "Done",
        Phase::Failed(_) => "Failed",
        #[allow(unreachable_patterns)]
        _ => "Other",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Start { id: String, source: Source },
    /// The recording is over (release, second hotkey press, silence, 30 s).
    AudioEnd,
    Cancel,
    /// The servers are starting.
    ModelLoading,
    /// T29.P: the model a running call waited for has loaded (or will not).
    ModelReady,
    Transcript(Result<String, String>),
    /// The PC microphone could not be opened or died while recording.
    CaptureFailed(String),
    /// Nothing was said for 5 s (DATA_MODEL.md 12.6a): the request ends, no sound goes to recognition.
    NoSpeech,
    /// The state has not changed for 30 s (the watchdog).
    Watchdog,
    /// The model's answer: a tool call (with its short text) or nothing usable.
    LlmAnswer(Option<(RawCall, Option<String>)>),
    /// The model cannot answer at all (T28.2): the fixed text that says why.
    ModelFailed(&'static str),
    Confirm { id: String, yes: bool },
    Tick { now_ms: u64 },
    Executed(Result<String, String>),
    Reset,
    /// M30: the PC itself asks (a button of the island): the call waits for the "yes" like a spoken one.
    Propose { id: String, call: Validated },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    StartCapture,
    StopCapture,
    Transcribe,
    AskLlm(String),
    Execute(ToolCall),
    /// Send `Event::Tick` after this many milliseconds.
    ScheduleTick(u64),
}

pub struct Env<'a> {
    pub table: &'a Table,
    pub ctx: &'a Context,
    pub confirm_ms: u64,
    /// "Dictate" is waiting: the next phrase is typed as it is, not understood as a command.
    pub dictate: bool,
}

fn describe(i: &Intent) -> String {
    let arg = i.args.values().filter_map(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string()))).collect::<Vec<_>>().join(" ");
    if arg.is_empty() { i.tool.replace('_', " ") } else { format!("{} {arg}", i.tool.replace('_', " ")) }
}

/// The model gave nothing: say why the table's own reading was refused, when there was one, else `otherwise`.
fn no_answer(s: State, env: &Env, otherwise: &str) -> (State, Vec<Effect>) {
    let table_reject = s.heard.as_deref().and_then(|h| match env.table.matches(h) {
        Match::Hit(i) => match validate(&RawCall { tool: i.tool, args: serde_json::Value::Object(i.args) }, env.ctx) {
            Resolved::Reject(why) => Some(why),
            _ => None,
        },
        _ => None,
    });
    match table_reject {
        Some(why) => finish_key(s, Phase::Failed(format!("Can't do that: {why}")), ReplyKey::with(key::CANT, "why", &why)),
        None => finish_key(s, Phase::Failed(otherwise.to_string()), ReplyKey::plain(executor_key(otherwise).unwrap_or(key::DIDNT_GET))),
    }
}

/// Starts a call; the pill says what is going on while it runs.
fn executing(mut s: State, v: &Validated) -> (State, Vec<Effect>) {
    let call = v.call.clone();
    s.understood = Some(understood(v));
    s.phase = Phase::Executing;
    s.model_wait = false;
    if matches!(call, ToolCall::ScreenAsk { .. }) {
        s.activity = Some("screen");
    }
    // The result of arithmetic and of a question is the answer itself, not the few words the model said with the call.
    if matches!(call, ToolCall::Calc { .. } | ToolCall::Ask { .. } | ToolCall::Status { .. } | ToolCall::TextTransform { .. } | ToolCall::ReplyAction { .. }) {
        s.say = None;
    }
    // An answer that is shown (not pasted) can be continued; the flag only counts when the result is not empty.
    s.actions = matches!(call, ToolCall::TextTransform { .. } | ToolCall::ReplyAction { kind: super::followup::Kind::Again });
    (s, vec![Effect::Execute(call)])
}

fn finish(mut s: State, phase: Phase) -> (State, Vec<Effect>) {
    s.phase = phase;
    s.reply_key = None;
    if !matches!(s.phase, Phase::Done(ref t) if !t.is_empty()) {
        s.actions = false;
    }
    (s, Vec::new())
}

/// `finish` for a fixed phrase: the text is the English fallback of `key`.
fn finish_key(mut s: State, phase: Phase, key: ReplyKey) -> (State, Vec<Effect>) {
    s.phase = phase;
    s.reply_key = Some(key);
    s.actions = false;
    (s, Vec::new())
}

/// Runs a tool call through `validate` and moves to confirm, execute, a question or a failure.
fn resolve(s: State, raw: &RawCall, now_ms: u64, env: &Env, from_model: bool) -> (State, Vec<Effect>) {
    match validate(raw, env.ctx) {
        Resolved::Ready(v) if v.confirm => confirming(s, v, now_ms, env),
        Resolved::Ready(v) => executing(s, &v),
        Resolved::Ask { question, options } => finish_key(s, Phase::Done(question), ReplyKey::with(key::WHICH, "options", &options.iter().take(3).cloned().collect::<Vec<_>>().join(" / "))),
        Resolved::Reject(_) if from_model => finish_key(s, Phase::Failed(msg::DIDNT_GET.into()), ReplyKey::plain(key::DIDNT_GET)),
        Resolved::Reject(why) => finish_key(s, Phase::Failed(format!("Can't do that: {why}")), ReplyKey::with(key::CANT, "why", &why)),
    }
}

/// Asks the question. A request of the PC also listens for the spoken answer, and gets half as long again to say it.
fn confirming(mut s: State, call: Validated, now_ms: u64, env: &Env) -> (State, Vec<Effect>) {
    let pc = s.source == Some(Source::Pc);
    let window = if pc { env.confirm_ms + env.confirm_ms / 2 } else { env.confirm_ms };
    s.phase = Phase::Confirming { call, deadline_ms: now_ms + window, ear: if pc { Ear::Open } else { Ear::Off } };
    let mut effects = vec![Effect::ScheduleTick(window)];
    if pc {
        effects.insert(0, Effect::StartCapture);
    }
    (s, effects)
}

/// A "yes": the microphone closes and the call runs. A "no": the microphone closes.
fn answered(s: State, call: &Validated, yes: bool) -> (State, Vec<Effect>) {
    let pc = s.source == Some(Source::Pc);
    let (next, mut effects) = if yes { executing(s, call) } else { finish_key(s, Phase::Done(msg::CANCELLED.into()), ReplyKey::plain(key::CANCELLED)) };
    if pc {
        effects.insert(0, Effect::StopCapture);
    }
    (next, effects)
}

/// The same question with the microphone in another state.
fn hearing(mut s: State, ear: Ear) -> State {
    if let Phase::Confirming { ear: now, .. } = &mut s.phase {
        *now = ear;
    }
    s
}

/// The state machine. Events that make no sense in the current phase change nothing.
pub fn step(state: &State, event: Event, now_ms: u64, env: &Env) -> (State, Vec<Effect>) {
    let s = state.clone();
    match (&state.phase, event) {
        (Phase::Idle | Phase::Done(_) | Phase::Failed(_), Event::Start { id, source }) => {
            let effects = if source == Source::Pc { vec![Effect::StartCapture] } else { Vec::new() };
            (State { phase: Phase::Listening, id: Some(id), source: Some(source), activity: env.dictate.then_some("dictate"), ..State::idle() }, effects)
        }
        // M30: a button of the island asks for something that needs a "yes" (closing a program).
        (Phase::Idle | Phase::Done(_) | Phase::Failed(_), Event::Propose { id, call }) => {
            let s = State { id: Some(id), source: Some(Source::Pc), ..State::idle() };
            confirming(s, call, now_ms, env)
        }
        (Phase::Idle, Event::Cancel) => (s, Vec::new()),
        (Phase::Listening, Event::CaptureFailed(e)) if e == msg::NO_SOUND => {
            if !cfg!(test) {
                super::mic::log_line("capture failed: no sound from any microphone");
                super::mic::ring_dump("no sound");
            }
            let (mut next, _) = finish_key(s, Phase::Failed(msg::NO_SOUND.into()), ReplyKey::plain(key::NO_SOUND));
            next.heard = None;
            (next, vec![Effect::StopCapture])
        }
        (Phase::Listening, Event::CaptureFailed(e)) => {
            tracing::warn!("voice microphone: {e}");
            if !cfg!(test) {
                super::mic::log_line(&format!("capture failed: {e}"));
                super::mic::ring_dump("capture failed");
            }
            let (mut next, _) = finish_key(s, Phase::Failed(msg::MIC_UNAVAILABLE.into()), ReplyKey::plain(key::MIC_UNAVAILABLE));
            next.heard = None;
            (next, vec![Effect::StopCapture])
        }
        (Phase::Listening, Event::NoSpeech) => {
            if !cfg!(test) {
                super::mic::ring_dump("no speech");
            }
            let (mut next, _) = finish_key(s, Phase::Failed(msg::NO_SPEECH.into()), ReplyKey::plain(key::NO_SPEECH));
            next.heard = None;
            (next, vec![Effect::StopCapture])
        }
        (Phase::Listening | Phase::Transcribing | Phase::Loading | Phase::Thinking, Event::Watchdog) => {
            let (mut next, _) = finish_key(s, Phase::Failed(msg::WRONG.into()), ReplyKey::plain(key::WRONG));
            next.heard = None;
            (next, vec![Effect::StopCapture])
        }
        (_, Event::Cancel) => (State::idle(), vec![Effect::StopCapture]),
        (Phase::Done(_) | Phase::Failed(_), Event::Reset) => (State::idle(), Vec::new()),
        (Phase::Listening, Event::AudioEnd) => {
            let mut s = s;
            s.phase = Phase::Transcribing;
            (s, vec![Effect::StopCapture, Effect::Transcribe])
        }
        (Phase::Transcribing | Phase::Thinking, Event::ModelLoading) => finish(s, Phase::Loading),
        (Phase::Executing, Event::ModelLoading) => (State { model_wait: true, ..s }, Vec::new()),
        (Phase::Executing, Event::ModelReady) => (State { model_wait: false, ..s }, Vec::new()),
        (Phase::Transcribing | Phase::Loading, Event::Transcript(Err(e))) => match executor_key(&e) {
            Some(k) => finish_key(s, Phase::Failed(e), ReplyKey::plain(k)),
            None => finish(s, Phase::Failed(e)),
        },
        (Phase::Transcribing | Phase::Loading, Event::Transcript(Ok(text))) => {
            let text = text.trim().to_string();
            if text.is_empty() {
                return finish_key(s, Phase::Failed(msg::DIDNT_CATCH.into()), ReplyKey::plain(key::DIDNT_CATCH));
            }
            let mut s = s;
            s.heard = Some(text.clone());
            if env.dictate {
                // Long text goes through the clipboard (a call of the model could never name this: it is not in the white list).
                if let super::dictation::Insert::Paste(text) = super::dictation::insert_plan(&text) {
                    s.phase = Phase::Executing;
                    return (s, vec![Effect::Execute(ToolCall::PasteText { text })]);
                }
                let args = serde_json::json!({ "text": text });
                return resolve(s, &RawCall { tool: "type_text".into(), args }, now_ms, env, false);
            }
            // T29.8: "stop" said over a spoken answer: the answer is silent already, nothing else is stopped.
            if env.ctx.speech_interrupted && env.table.guide_control(&text) == Some("stop") {
                return finish_key(s, Phase::Done(msg::DONE.into()), ReplyKey::plain(key::DONE));
            }
            // During a guide the words "next", "back", "stop", "done" are its controls, before anything else (T27.5).
            if env.ctx.guide_running {
                if let Some(kind) = env.table.guide_control(&text) {
                    let raw = RawCall { tool: "guide_control".into(), args: serde_json::json!({ "kind": kind }) };
                    return resolve(s, &raw, now_ms, env, false);
                }
            }
            match env.table.matches(&text) {
                Match::Hit(intent) => {
                    // "weather in Paris" is not the weather of the widget: the model searches the web for it (T26.F-6).
                    if intent.tool == "status"
                        && intent.args.get("topic").and_then(|t| t.as_str()) == Some("weather")
                        && env.table.names_other_place(&text, env.ctx.weather_place.as_deref())
                    {
                        s.phase = Phase::Thinking;
                        return (s, vec![Effect::AskLlm(text)]);
                    }
                    let raw = RawCall { tool: intent.tool, args: serde_json::Value::Object(intent.args) };
                    // A template can swallow a phrase meant for another tool ("open main page" is no program): the
                    // model gets the phrase before it is given up on.
                    if let Resolved::Reject(_) = validate(&raw, env.ctx) {
                        if !cfg!(test) {
                            // The reason can quote the user's words: only the tool is written down.
                            super::mic::log_line(&format!("table hit {} but refused (apps known: {})", raw.tool, env.ctx.apps.len()));
                        }
                        s.phase = Phase::Thinking;
                        return (s, vec![Effect::AskLlm(text)]);
                    }
                    resolve(s, &raw, now_ms, env, false)
                }
                Match::Ambiguous(list) => {
                    let names: Vec<String> = list.iter().take(3).map(describe).collect();
                    let shown = names.join(" / ");
                    finish_key(s, Phase::Done(format!("Which one: {}?", names.join(" or "))), ReplyKey::with(key::WHICH, "options", &shown))
                }
                Match::None => {
                    s.phase = Phase::Thinking;
                    (s, vec![Effect::AskLlm(text)])
                }
            }
        }
        (Phase::Thinking | Phase::Loading | Phase::Transcribing, Event::LlmAnswer(answer)) => match answer {
            None => no_answer(s, env, msg::DIDNT_GET),
            // The model may remember something only when the user asked for it in so many words (13.6).
            Some((raw, _)) if raw.tool == "memory_add" && !s.heard.as_deref().is_some_and(super::memory::asks_to_remember) => {
                finish_key(s, Phase::Failed(msg::DIDNT_GET.into()), ReplyKey::plain(key::DIDNT_GET))
            }
            Some((raw, say)) => {
                let mut s = s;
                s.say = say.filter(|t| !t.trim().is_empty());
                // T30.7: the rules the model does not always keep.
                let raw = match s.heard.as_deref() {
                    Some(heard) => super::llm::adjust(raw, heard, env.table, env.ctx, &env.ctx.lang),
                    None => raw,
                };
                resolve(s, &raw, now_ms, env, true)
            }
        },
        (Phase::Thinking | Phase::Loading | Phase::Transcribing, Event::ModelFailed(why)) => no_answer(s, env, why),
        // The PC recorded an answer: recognise it; the deadline waits for the recognition.
        (Phase::Confirming { call, deadline_ms, ear: Ear::Open }, Event::AudioEnd) => {
            let deadline_ms = (*deadline_ms).max(now_ms + ANSWER_GRACE_MS);
            let mut s = s;
            s.phase = Phase::Confirming { call: call.clone(), deadline_ms, ear: Ear::Busy };
            (s, vec![Effect::StopCapture, Effect::Transcribe, Effect::ScheduleTick(deadline_ms - now_ms)])
        }
        // Silence: keep listening until the deadline.
        (Phase::Confirming { ear: Ear::Open, .. }, Event::NoSpeech) => (s, vec![Effect::StopCapture, Effect::StartCapture]),
        // Without a microphone the buttons and the keys still answer.
        (Phase::Confirming { ear: Ear::Open, .. }, Event::CaptureFailed(_)) => (hearing(s, Ear::Off), vec![Effect::StopCapture]),
        (Phase::Confirming { ear: Ear::Busy, .. }, Event::Transcript(Err(_)) | Event::ModelFailed(_)) => (hearing(s, Ear::Off), Vec::new()),
        // A spoken yes or no while a confirmation is open; anything else is not an answer.
        (Phase::Confirming { call, ear, .. }, Event::Transcript(Ok(text))) => match env.table.answer(&text) {
            Some(yes) => answered(s, call, yes),
            None if *ear == Ear::Busy => (hearing(s, Ear::Open), vec![Effect::StartCapture]),
            None => (s, Vec::new()),
        },
        (Phase::Confirming { call, .. }, Event::Confirm { id, yes }) if state.id.as_deref() == Some(id.as_str()) => answered(s, call, yes),
        (Phase::Confirming { call, deadline_ms, .. }, Event::Tick { now_ms }) if now_ms >= *deadline_ms => answered(s, call, false),
        // The model cannot know the answer offline: the pill asks, and only a yes searches the web (DATA_MODEL.md 12.5).
        (Phase::Executing, Event::Executed(Ok(reply))) if super::ask::web_question(&reply).is_some() => {
            let query = super::ask::web_question(&reply).unwrap_or_default();
            let raw = RawCall { tool: "web_search".into(), args: serde_json::json!({ "query": query, "site": "web" }) };
            match validate(&raw, env.ctx) {
                Resolved::Ready(mut v) => {
                    v.confirm = true;
                    v.summary = msg::ASK_WEB.into();
                    v.key = Some(key::ASK_WEB);
                    v.args.clear();
                    let mut s = s;
                    s.say = None;
                    confirming(s, v, now_ms, env)
                }
                _ => finish_key(s, Phase::Failed(msg::DIDNT_GET.into()), ReplyKey::plain(key::DIDNT_GET)),
            }
        }
        // M30 (T30.2): the program did not close by itself: force needs another "yes".
        (Phase::Executing, Event::Executed(Ok(reply))) if super::heavy::force_question(&reply).is_some() => {
            let (exe, name) = super::heavy::force_question(&reply).unwrap_or_default();
            let call = Validated {
                summary: format!("Close {name} by force?"),
                key: Some("voice.confirm.close_force"),
                args: [("name".to_string(), name.clone())].into(),
                call: ToolCall::CloseHeavy { exe, name, force: true },
                confirm: true,
            };
            let mut s = s;
            s.say = None;
            confirming(s, call, now_ms, env)
        }
        // A `status` answer: a template key with its numbers, the text is the English fallback.
        (Phase::Executing, Event::Executed(Ok(reply))) if super::status::decode(&reply).is_some() => {
            let (k, args, text) = super::status::decode(&reply).unwrap_or(("", Default::default(), String::new()));
            finish_key(s, Phase::Done(text), ReplyKey { key: k, args })
        }
        // T27.3: the element that was found, by its name.
        (Phase::Executing, Event::Executed(Ok(reply))) if reply.starts_with("Here: ") => {
            let name = reply.trim_start_matches("Here: ").to_string();
            finish_key(s, Phase::Done(reply), ReplyKey::with("voice.reply.found_here", "name", &name))
        }
        // T29.1: whatever the model said with the call, the user must learn that the text is on the clipboard.
        (Phase::Executing, Event::Executed(Ok(reply))) if reply == msg::COPIED_PASTE => finish_key(s, Phase::Done(reply), ReplyKey::plain(key::COPIED_PASTE)),
        (Phase::Executing, Event::Executed(Ok(reply))) => {
            match (s.say.clone(), reply.is_empty()) {
                (Some(say), _) => finish(s, Phase::Done(say)),
                (None, true) => finish_key(s, Phase::Done(msg::DONE.to_string()), ReplyKey::plain(key::DONE)),
                (None, false) => finish(s, Phase::Done(reply)),
            }
        }
        (Phase::Executing, Event::Executed(Err(e))) => match executor_key(&e) {
            Some(k) => finish_key(s, Phase::Failed(e), ReplyKey::plain(k)),
            None => finish(s, Phase::Failed(e)),
        },
        _ => (s, Vec::new()),
    }
}

// ---- Driver -----------------------------------------------------------------------------------------------------

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Everything the pipeline needs from outside: speech recognition, the model, the executors, the microphone, the screen.
pub trait Ports: Send + Sync + 'static {
    /// The phrase table and the things that exist on this PC, read fresh for every request.
    fn env(&self) -> (Arc<Table>, Context);
    /// "Dictate" is waiting for the phrase to type (default: never).
    fn dictating(&self) -> bool {
        false
    }
    /// 16 kHz mono PCM16 -> text.
    fn transcribe(&self, pcm: Vec<u8>) -> BoxFut<Result<String, String>>;
    /// The AI path; `None` = no usable answer.
    fn ask_llm(&self, text: String, ctx: Context) -> BoxFut<Option<(RawCall, Option<String>)>>;
    /// Why the model cannot answer at all, after `ask_llm` gave nothing (default: it can).
    fn model_failure(&self) -> Option<&'static str> {
        None
    }
    /// Runs a validated call with Decka's own executors; the text is the result for the user.
    fn execute(&self, call: ToolCall) -> BoxFut<Result<String, String>>;
    /// The PC microphone on (`true`) or off.
    fn capture(&self, on: bool);
    /// The new `voice` section.
    fn publish(&self, info: VoiceInfo);
}

struct Shared {
    state: State,
    audio: Vec<u8>,
    /// Speech the endpoint heard in this recording (`None`: not known, e.g. a phone).
    speech_ms: Option<u32>,
}

/// A recording of the PC without a single chunk of speech never goes to the recognizer: whisper invents text from silence.
pub fn audio_end_or_no_speech(event: Event, speech_ms: Option<u32>) -> Event {
    match (event, speech_ms) {
        (Event::AudioEnd, Some(0)) => Event::NoSpeech,
        (e, _) => e,
    }
}

/// Owns one pipeline: feeds events to `step` and runs the effects. Cheap to clone.
#[derive(Clone)]
pub struct Driver {
    ports: Arc<dyn Ports>,
    shared: Arc<Mutex<Shared>>,
    handle: tokio::runtime::Handle,
    t0: std::time::Instant,
    confirm_ms: u64,
}

impl Driver {
    pub fn new(handle: tokio::runtime::Handle, ports: Arc<dyn Ports>) -> Driver {
        Driver::with_confirm_timeout(handle, ports, CONFIRM_TIMEOUT_MS)
    }

    pub fn with_confirm_timeout(handle: tokio::runtime::Handle, ports: Arc<dyn Ports>, confirm_ms: u64) -> Driver {
        Driver { ports, shared: Arc::new(Mutex::new(Shared { state: State::idle(), audio: Vec::new(), speech_ms: None })), handle, t0: std::time::Instant::now(), confirm_ms }
    }

    pub fn state(&self) -> State {
        self.shared.lock().unwrap().state.clone()
    }

    /// The endpoint's count of speech so far in the running recording.
    pub fn note_speech(&self, ms: u32) {
        let mut sh = self.shared.lock().unwrap();
        if sh.state.is_hearing() {
            sh.speech_ms = Some(ms);
        }
    }

    pub fn push_audio(&self, pcm: &[u8]) {
        let mut sh = self.shared.lock().unwrap();
        if sh.state.is_hearing() {
            sh.audio.extend_from_slice(pcm);
        }
    }

    /// Feeds one event; effects run in the background and answer with further events.
    pub fn handle_event(&self, event: Event) {
        let (table, ctx) = self.ports.env();
        let env = Env { table: &table, ctx: &ctx, confirm_ms: self.confirm_ms, dictate: self.ports.dictating() };
        let now_ms = self.t0.elapsed().as_millis() as u64;
        let name = event_name(&event);
        if !cfg!(test) {
            match &event {
                Event::LlmAnswer(Some((call, _))) => super::mic::log_line(&format!("model chose tool {}", call.tool)),
                Event::LlmAnswer(None) => super::mic::log_line("model gave no usable answer"),
                // The outcome only: the text of a result can be a translation, a screen answer or a remembered fact.
                Event::Executed(r) => super::mic::log_line(executed_line(r)),
                _ => {}
            }
        }
        if let (Event::Transcript(Err(e)), false) = (&event, cfg!(test)) {
            // The error of the servers, not the words of the user.
            super::mic::log_line(&format!("transcript error: {e}"));
        }
        let (info, effects, pcm) = {
            let mut sh = self.shared.lock().unwrap();
            let before = phase_name(&sh.state.phase);
            let event = match event {
                // A "yes" is shorter than a phrase has to be: whatever speech there was in an answer is recognised.
                Event::NoSpeech if matches!(sh.state.phase, Phase::Confirming { ear: Ear::Open, .. }) && sh.speech_ms.is_some_and(|ms| ms > 0) => Event::AudioEnd,
                event if sh.state.is_hearing() => audio_end_or_no_speech(event, sh.speech_ms),
                event => event,
            };
            let (next, effects) = step(&sh.state, event, now_ms, &env);
            if name != "Tick" && !cfg!(test) {
                // Names only: never sound or text.
                super::mic::log_line(&format!("event {name} t={now_ms}ms {before} -> {}", phase_name(&next.phase)));
            }
            let changed = next != sh.state;
            // A new recording starts empty (a phone request has no capture of its own).
            if effects.contains(&Effect::StartCapture) || (next.is_hearing() && !sh.state.is_hearing()) {
                sh.audio.clear();
                sh.speech_ms = None;
            }
            let pcm = if effects.contains(&Effect::Transcribe) { std::mem::take(&mut sh.audio) } else { Vec::new() };
            sh.state = next;
            (changed.then(|| sh.state.info()), effects, pcm)
        };
        if let Some(info) = info {
            self.ports.publish(info);
        }
        for effect in effects {
            self.run(effect, &ctx, pcm.clone());
        }
    }

    fn run(&self, effect: Effect, ctx: &Context, pcm: Vec<u8>) {
        let me = self.clone();
        match effect {
            Effect::StartCapture => self.ports.capture(true),
            Effect::StopCapture => self.ports.capture(false),
            Effect::Transcribe => {
                self.handle.spawn(async move {
                    let r = me.ports.transcribe(pcm).await;
                    me.handle_event(Event::Transcript(r));
                });
            }
            Effect::AskLlm(text) => {
                let ctx = ctx.clone();
                self.handle.spawn(async move {
                    let a = me.ports.ask_llm(text, ctx).await;
                    let failed = if a.is_none() { me.ports.model_failure() } else { None };
                    me.handle_event(failed.map_or(Event::LlmAnswer(a), Event::ModelFailed));
                });
            }
            Effect::Execute(call) => {
                self.handle.spawn(async move {
                    let r = me.ports.execute(call).await;
                    me.handle_event(Event::Executed(r));
                });
            }
            Effect::ScheduleTick(ms) => {
                self.handle.spawn(async move {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    let now_ms = me.t0.elapsed().as_millis() as u64;
                    me.handle_event(Event::Tick { now_ms });
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_answers_of_the_tools_have_a_key_in_the_dictionary() {
        let english = include_str!("../../../../shared/src/i18n/en.ts");
        let found = [
            "Did not find it on the screen", "Not found on the screen", "No other candidates", "Cannot read this window", "No window to look at",
            "Cannot see any buttons in this window", "No guide is running", "A guide is already running", "The assistant could not choose a step",
            "The assistant did not answer", "I do not remember that", "The memory is full: forget something first", "That fact is already there",
            "There is no active window", "There is only one monitor", "Too many timers are running", "That button cannot be added by voice",
        ];
        for text in found {
            let key = executor_key(text).unwrap_or_else(|| panic!("no key: {text}"));
            assert!(english.contains(&format!("\"{key}\"")), "{key} is not in en.ts");
        }
        assert!(english.contains("\"voice.reply.found_here\""));
    }

    #[test]
    fn the_log_of_an_execution_never_holds_its_text() {
        for r in [Ok("My password is hunter2".to_string()), Err("no program called secret-plan".to_string())] {
            let line = executed_line(&r);
            assert!(line == "executed: ok" || line == "executed: error", "{line}");
        }
    }
    use crate::voice::tools::{ActionRef, AppRef};
    use serde_json::json;

    fn table() -> Table {
        Table::parse(include_str!("../../../../shared/voice/en.json")).unwrap()
    }

    fn ctx() -> Context {
        Context {
            actions: vec![ActionRef { id: "lock".into(), name: "Lock".into() }],
            apps: vec![AppRef { name: "Calculator".into(), program: "calc.exe".into() }],
            ..Default::default()
        }
    }

    /// Runs `events` from idle; returns the last state and every effect.
    fn run(events: Vec<Event>) -> (State, Vec<Effect>) {
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let mut state = State::idle();
        let mut all = Vec::new();
        for (i, e) in events.into_iter().enumerate() {
            let (s, fx) = step(&state, e, 1_000 + i as u64, &env);
            state = s;
            all.extend(fx);
        }
        (state, all)
    }

    fn start(source: Source) -> Event {
        Event::Start { id: "00112233aabbccdd".into(), source }
    }

    fn said(text: &str) -> Event {
        Event::Transcript(Ok(text.into()))
    }

    /// T27.5: while a guide runs, "stop" and "next" are its controls (without the model); otherwise they are ordinary words.
    #[test]
    fn during_a_guide_stop_and_next_are_its_controls() {
        let (t, mut c) = (table(), ctx());
        let go = |c: &Context, phrase: &str| {
            let env = Env { table: &t, ctx: c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
            let mut state = State::idle();
            let mut all = Vec::new();
            for (i, e) in vec![start(Source::Phone), Event::AudioEnd, said(phrase)].into_iter().enumerate() {
                let (s, fx) = step(&state, e, 1_000 + i as u64, &env);
                state = s;
                all.extend(fx);
            }
            all
        };
        c.guide_running = true;
        assert!(go(&c, "stop").contains(&Effect::Execute(ToolCall::GuideControl { kind: crate::voice::tools::GuideCtl::Stop })));
        assert!(go(&c, "next").contains(&Effect::Execute(ToolCall::GuideControl { kind: crate::voice::tools::GuideCtl::Next })));
        c.guide_running = false;
        assert!(!go(&c, "next").iter().any(|e| matches!(e, Effect::Execute(ToolCall::GuideControl { .. }))), "no guide: not a control");
    }

    /// T23.B: a question the model cannot answer offline asks "search the web?"; the search opens only after a yes.
    #[test]
    fn an_unknown_answer_asks_before_the_web_and_a_calc_reply_is_shown_as_it_is() {
        let ask = RawCall { tool: "ask".into(), args: serde_json::json!({ "question": "bitcoin price today" }) };
        let base = vec![start(Source::Phone), Event::AudioEnd, said("bitcoin price today"), Event::LlmAnswer(Some((ask.clone(), Some("hmm".into()))))];
        let (s, fx) = run(base.clone());
        assert_eq!((s.phase.clone(), fx.last().cloned()), (Phase::Executing, Some(Effect::Execute(ToolCall::Ask { question: "bitcoin price today".into() }))));
        assert_eq!(s.say, None, "the answer is the reply, not the words said with the call");

        let mut asked = base.clone();
        asked.push(Event::Executed(Ok(super::super::ask::web_reply("bitcoin price today"))));
        let (s, _) = run(asked.clone());
        let Phase::Confirming { call, .. } = &s.phase else { panic!("{:?}", s.phase) };
        assert!(call.confirm && call.key == Some("voice.confirm.ask_web"));
        assert!(matches!(&call.call, ToolCall::OpenUrl { url } if url.starts_with("https://www.google.com/search?q=bitcoin")));
        assert_eq!(s.info().state, "confirm");

        let mut no = asked.clone();
        no.push(Event::Confirm { id: "00112233aabbccdd".into(), yes: false });
        assert_eq!(run(no).0.phase, Phase::Done(msg::CANCELLED.into()), "no search without a yes");
        let mut yes = asked;
        yes.push(Event::Confirm { id: "00112233aabbccdd".into(), yes: true });
        let (s, fx) = run(yes);
        assert_eq!(s.phase, Phase::Executing);
        assert!(matches!(fx.last(), Some(Effect::Execute(ToolCall::OpenUrl { .. }))));

        // A status answer keeps its key and numbers, and the model's own words do not replace it (T26.1).
        let st = RawCall { tool: "status".into(), args: serde_json::json!({ "topic": "cpu" }) };
        let answer = crate::voice::status::answer_for(crate::voice::status::Topic::Cpu, &crate::voice::status::Sections { sys: serde_json::json!({ "cpu": 23 }), ..Default::default() }, "en", &crate::assistant::Local::now());
        let events = vec![start(Source::Phone), Event::AudioEnd, said("hm"), Event::LlmAnswer(Some((st, Some("it is fine".into())))), Event::Executed(Ok(crate::voice::status::encode(&answer)))];
        let (s, _) = run(events);
        let i = s.info();
        assert_eq!((s.phase.clone(), i.reply_key, i.reply_args.get("pct").map(String::as_str)), (Phase::Done("Processor: 23% busy".into()), Some("voice.status.cpu"), Some("23")));

        // T26.3: a shown rewrite offers the actions of the PC window; a pasted one (empty result) and a status answer do not.
        let tt = |op: &str| RawCall { tool: "text_transform".into(), args: serde_json::json!({ "op": op }) };
        let (shown, _) = run(vec![start(Source::Pc), Event::AudioEnd, said("hm"), Event::LlmAnswer(Some((tt("summarize"), Some("ok".into())))), Event::Executed(Ok("It is about cats.".into()))]);
        assert_eq!((shown.phase.clone(), shown.info().actions), (Phase::Done("It is about cats.".into()), crate::voice::followup::ACTIONS.to_vec()));
        let (pasted, _) = run(vec![start(Source::Pc), Event::AudioEnd, said("hm"), Event::LlmAnswer(Some((tt("polite"), None))), Event::Executed(Ok(String::new()))]);
        assert!(pasted.info().actions.is_empty());
        let (failed, _) = run(vec![start(Source::Pc), Event::AudioEnd, said("hm"), Event::LlmAnswer(Some((tt("summarize"), None))), Event::Executed(Err(msg::NO_CONTEXT.into()))]);
        assert_eq!((failed.info().reply_key, failed.info().actions.is_empty()), (Some("voice.reply.no_context"), true));

        // The result of arithmetic and a division by zero.
        let sum = RawCall { tool: "calc".into(), args: serde_json::json!({ "expr": "54 умножить на 114" }) };
        let events = vec![start(Source::Phone), Event::AudioEnd, said("hm"), Event::LlmAnswer(Some((sum, Some("hmm".into())))), Event::Executed(Ok("54 × 114 = 6156".into()))];
        assert_eq!(run(events).0.phase, Phase::Done("54 × 114 = 6156".into()));
        let zero = RawCall { tool: "calc".into(), args: serde_json::json!({ "expr": "5 / 0" }) };
        let events = vec![start(Source::Phone), Event::AudioEnd, said("hm"), Event::LlmAnswer(Some((zero, None))), Event::Executed(Err(msg::DIV_ZERO.into()))];
        let (s, _) = run(events);
        assert_eq!((s.phase.clone(), s.info().reply_key), (Phase::Failed(msg::DIV_ZERO.into()), Some("voice.reply.div_zero")));
    }

    #[test]
    fn a_recording_without_speech_is_not_sent_to_the_recognizer() {
        assert_eq!(audio_end_or_no_speech(Event::AudioEnd, Some(0)), Event::NoSpeech);
        assert_eq!(audio_end_or_no_speech(Event::AudioEnd, Some(300)), Event::AudioEnd);
        assert_eq!(audio_end_or_no_speech(Event::AudioEnd, None), Event::AudioEnd);
        let (s, fx) = run(vec![start(Source::Pc), audio_end_or_no_speech(Event::AudioEnd, Some(0))]);
        assert_eq!(s.phase, Phase::Failed(msg::NO_SPEECH.into()));
        assert!(!fx.contains(&Effect::Transcribe));
    }

    #[test]
    fn no_sound_from_any_microphone_fails_and_the_pill_goes_idle() {
        let (s, _) = run(vec![start(Source::Pc), Event::CaptureFailed(msg::NO_SOUND.into())]);
        assert_eq!(s.info().reply_key, Some("voice.reply.no_sound"));
        let mut events = vec![start(Source::Pc), Event::CaptureFailed(msg::NO_SOUND.into()), start(Source::Pc), audio_end_or_no_speech(Event::AudioEnd, Some(0)), Event::Reset];
        events.extend([start(Source::Pc), Event::CaptureFailed(msg::NO_SOUND.into()), Event::Reset]);
        assert_eq!(run(events).0.phase, Phase::Idle);
    }

    #[test]
    fn repeated_presses_always_end_in_idle() {
        // Failed -> Start during the result -> NoSpeech -> Reset -> Idle; five times in a row.
        let mut events = Vec::new();
        for _ in 0..5 {
            events.extend([start(Source::Pc), Event::NoSpeech, start(Source::Pc), Event::NoSpeech, Event::Reset]);
        }
        let (s, _) = run(events);
        assert_eq!(s.phase, Phase::Idle);
        assert_eq!(s.id, None);
        // A stale Reset after a new Start changes nothing; the session goes on.
        let (s, _) = run(vec![start(Source::Pc), Event::NoSpeech, start(Source::Pc), Event::Reset]);
        assert_eq!(s.phase, Phase::Listening);
    }

    #[test]
    fn a_running_call_that_waits_for_the_model_says_loading_and_goes_on() {
        let call = |call: ToolCall| Validated { call, confirm: false, summary: "x".into(), key: None, args: Default::default() };
        let (s, _) = executing(State::idle(), &call(ToolCall::FindOnScreen { query: Some("file".into()) }));
        assert_eq!(s.info().state, "thinking");
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let (s, fx) = step(&s, Event::ModelLoading, 1, &env);
        assert_eq!((s.phase.clone(), s.info().state, s.info().executing, fx.len()), (Phase::Executing, "loading", false, 0));
        // The long watchdog of a request does not end a call that is running.
        assert_eq!(step(&s, Event::Watchdog, 2, &env).0.phase, Phase::Executing);
        let (s, _) = step(&s, Event::ModelReady, 3, &env);
        assert_eq!(s.info().state, "thinking");
        let (s, _) = step(&s, Event::Executed(Ok("Here: File".into())), 4, &env);
        assert_eq!(s.info().reply_key, Some("voice.reply.found_here"));
        // A new call starts without the mark.
        let mut waiting = State::idle();
        waiting.model_wait = true;
        assert_eq!(executing(waiting, &call(ToolCall::DayBrief)).0.info().state, "thinking");
    }

    #[test]
    fn a_phone_request_walks_record_recognise_match_execute() {
        let (s, fx) = run(vec![start(Source::Phone), Event::AudioEnd, said("volume up")]);
        assert_eq!(s.phase, Phase::Executing);
        assert_eq!(s.heard.as_deref(), Some("volume up"));
        assert_eq!(fx, [Effect::StopCapture, Effect::Transcribe, Effect::Execute(ToolCall::Volume(crate::voice::tools::VolumeOp::Up))]);
        let (s, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("volume up"), Event::Executed(Ok("Done".into()))]);
        assert_eq!(s.phase, Phase::Done("Done".into()));
        let info = s.info();
        assert_eq!((info.state, info.reply.as_deref(), info.source), ("done", Some("Done"), Some(Source::Phone)));
    }

    #[test]
    fn a_dictated_phrase_is_typed_as_it_is_even_when_it_sounds_like_a_command() {
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: true };
        let (s, _) = step(&State::idle(), start(Source::Phone), 1, &env);
        let (s, _) = step(&s, Event::AudioEnd, 2, &env);
        let (s, fx) = step(&s, said("volume up"), 3, &env);
        assert_eq!(s.phase, Phase::Executing);
        assert_eq!(fx, [Effect::Execute(ToolCall::TypeText { text: "volume up".into() })]);
    }

    #[test]
    fn the_pill_knows_a_dictation_and_a_look_at_the_screen_while_they_run() {
        let (t, c) = (table(), ctx());
        let dictating = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: true };
        let (s, _) = step(&State::idle(), start(Source::Pc), 1, &dictating);
        assert_eq!(s.info().activity, Some("dictate"));
        let (s, _) = step(&s, Event::AudioEnd, 2, &dictating);
        let (s, _) = step(&s, said("hello there"), 3, &dictating);
        assert_eq!(s.phase, Phase::Executing, "typed");
        let (s, _) = step(&s, Event::Executed(Ok(String::new())), 4, &dictating);
        assert_eq!((s.info().state, s.info().activity), ("done", None), "the label goes with the end of the request");
        let ordinary = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        assert_eq!(step(&State::idle(), start(Source::Pc), 1, &ordinary).0.info().activity, None);
        // The screen: the call that looks at it marks the request, any other call does not.
        let call = |call: ToolCall| Validated { call, confirm: false, summary: "x".into(), key: None, args: Default::default() };
        let (s, fx) = executing(State::idle(), &call(ToolCall::ScreenAsk { question: "what is this".into() }));
        assert_eq!((s.phase.clone(), s.info().state, s.info().activity), (Phase::Executing, "thinking", Some("screen")));
        assert_eq!(fx.len(), 1);
        assert_eq!(executing(State::idle(), &call(ToolCall::DayBrief)).0.info().activity, None);
        // It is part of the section the phone gets only while it runs.
        assert_eq!(serde_json::to_value(s.info()).unwrap()["activity"], "screen");
        assert!(serde_json::to_value(State::idle().info()).unwrap().get("activity").is_none());
    }

    #[test]
    fn the_fixed_failures_of_the_text_tools_carry_their_i18n_keys() {
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let mut s = State::idle();
        s.phase = Phase::Executing;
        let (s, _) = step(&s, Event::Executed(Err(msg::SELECT_FIRST.into())), 5, &env);
        assert_eq!((s.phase.clone(), s.info().reply_key), (Phase::Failed("Select text first".into()), Some("voice.reply.select_text")));
        let mut s = State::idle();
        s.phase = Phase::Executing;
        let (s, _) = step(&s, Event::Executed(Err("something else".into())), 5, &env);
        assert_eq!(s.info().reply_key, None, "free text has no key");
    }

    #[test]
    fn the_model_may_remember_a_fact_only_after_an_explicit_phrase() {
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let ask = |heard: &str| {
            let mut s = State::idle();
            s.phase = Phase::Thinking;
            s.heard = Some(heard.into());
            let call = RawCall { tool: "memory_add".into(), args: serde_json::json!({ "text": "Aigerim is my wife" }) };
            step(&s, Event::LlmAnswer(Some((call, None))), 5, &env)
        };
        let (s, fx) = ask("remember that Aigerim is my wife");
        assert_eq!((s.phase, fx), (Phase::Executing, vec![Effect::Execute(ToolCall::MemoryAdd { text: "Aigerim is my wife".into() })]));
        let (s, fx) = ask("Aigerim called me yesterday");
        assert_eq!((s.phase, fx.is_empty()), (Phase::Failed(msg::DIDNT_GET.into()), true), "no cue: the model learns nothing by itself");
    }

    #[test]
    fn a_long_dictation_is_pasted_through_the_clipboard() {
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: true };
        let (s, _) = step(&State::idle(), start(Source::Pc), 1, &env);
        let (s, _) = step(&s, Event::AudioEnd, 2, &env);
        let long = "word ".repeat(80);
        let (s, fx) = step(&s, said(long.trim()), 3, &env);
        assert_eq!(s.phase, Phase::Executing);
        assert_eq!(fx, [Effect::Execute(ToolCall::PasteText { text: long.trim().to_string() })]);
    }

    #[test]
    fn the_pc_microphone_is_started_only_for_pc_requests() {
        assert_eq!(run(vec![start(Source::Pc)]).1, [Effect::StartCapture]);
        assert!(run(vec![start(Source::Phone)]).1.is_empty());
    }

    #[test]
    fn a_risky_tool_waits_for_confirmation_and_runs_only_after_yes() {
        let (s, fx) = run(vec![start(Source::Phone), Event::AudioEnd, said("lock computer")]);
        assert!(matches!(s.phase, Phase::Confirming { .. }));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))), "nothing runs before the answer");
        assert!(fx.contains(&Effect::ScheduleTick(CONFIRM_TIMEOUT_MS)));
        let info = s.info();
        assert_eq!((info.state, info.confirm.map(|c| c.text)), ("confirm", Some("Lock the computer".to_string())));

        let (s, fx) = run(vec![start(Source::Phone), Event::AudioEnd, said("lock computer"), Event::Confirm { id: "00112233aabbccdd".into(), yes: true }]);
        assert_eq!(s.phase, Phase::Executing);
        assert!(fx.iter().any(|e| matches!(e, Effect::Execute(ToolCall::System(_)))));
    }

    #[test]
    fn no_or_a_timeout_cancels_without_running() {
        let go = || vec![start(Source::Phone), Event::AudioEnd, said("lock computer")];
        let mut no = go();
        no.push(Event::Confirm { id: "00112233aabbccdd".into(), yes: false });
        let (s, fx) = run(no);
        assert_eq!(s.phase, Phase::Done(msg::CANCELLED.into()));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))));

        // Timeout: a tick before the deadline changes nothing, a tick after it cancels.
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: 10_000, dictate: false };
        let (s, _) = run(go());
        let Phase::Confirming { deadline_ms, .. } = s.phase.clone() else { panic!() };
        let (early, _) = step(&s, Event::Tick { now_ms: deadline_ms - 1 }, 0, &env);
        assert!(matches!(early.phase, Phase::Confirming { .. }));
        let (late, fx) = step(&s, Event::Tick { now_ms: deadline_ms }, 0, &env);
        assert_eq!(late.phase, Phase::Done(msg::CANCELLED.into()));
        assert!(fx.is_empty());
    }

    #[test]
    fn a_confirm_for_another_request_is_ignored_and_a_spoken_answer_works() {
        let go = || vec![start(Source::Phone), Event::AudioEnd, said("lock computer")];
        let mut wrong = go();
        wrong.push(Event::Confirm { id: "ffffffffffffffff".into(), yes: true });
        assert!(matches!(run(wrong).0.phase, Phase::Confirming { .. }));
        let mut spoken = go();
        spoken.push(said("yes please"));
        assert_eq!(run(spoken).0.phase, Phase::Executing);
        let mut spoken_no = go();
        spoken_no.push(said("no"));
        assert_eq!(run(spoken_no).0.phase, Phase::Done(msg::CANCELLED.into()));
        let mut mumble = go();
        mumble.push(said("hmm let me think"));
        assert!(matches!(run(mumble).0.phase, Phase::Confirming { .. }), "neither yes nor no: keep waiting");
    }

    /// T29.2: the PC listens for the answer; "yes" runs the call, silence and other words keep the question open.
    #[test]
    fn the_pc_hears_a_spoken_yes() {
        let ask = vec![start(Source::Pc), Event::AudioEnd, said("lock computer")];
        let (s, fx) = run(ask.clone());
        assert!(matches!(s.phase, Phase::Confirming { ear: Ear::Open, .. }) && s.is_hearing());
        assert!(fx.ends_with(&[Effect::StartCapture, Effect::ScheduleTick(CONFIRM_TIMEOUT_MS * 3 / 2)]), "{fx:?}");

        let mut yes = ask.clone();
        yes.extend([Event::AudioEnd, said("yes")]);
        let (s, fx) = run(yes);
        assert_eq!(s.phase, Phase::Executing);
        assert!(fx.ends_with(&[Effect::StopCapture, Effect::Execute(ToolCall::System(crate::model::SystemOp::Lock))]), "{fx:?}");

        let mut other = ask.clone();
        other.extend([Event::AudioEnd, said("what was that again")]);
        let (s, fx) = run(other);
        assert!(matches!(s.phase, Phase::Confirming { ear: Ear::Open, .. }), "listens again");
        assert_eq!(fx.last(), Some(&Effect::StartCapture));

        let mut silence = ask.clone();
        silence.push(Event::NoSpeech);
        let (s, fx) = run(silence);
        assert!(matches!(s.phase, Phase::Confirming { ear: Ear::Open, .. }));
        assert!(fx.ends_with(&[Effect::StopCapture, Effect::StartCapture]));

        // No microphone: the question stays for the buttons.
        let mut deaf = ask.clone();
        deaf.extend([Event::CaptureFailed("gone".into()), Event::Confirm { id: "00112233aabbccdd".into(), yes: true }]);
        assert_eq!(run(deaf).0.phase, Phase::Executing);

        // An answer said just before the deadline is still recognised.
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: 10_000, dictate: false };
        let (s, _) = run(ask);
        let Phase::Confirming { deadline_ms, .. } = s.phase.clone() else { panic!() };
        let (s, _) = step(&s, Event::AudioEnd, deadline_ms - 100, &env);
        let (s, _) = step(&s, Event::Tick { now_ms: deadline_ms }, deadline_ms, &env);
        assert!(matches!(s.phase, Phase::Confirming { ear: Ear::Busy, .. }));
        assert_eq!(step(&s, said("yes"), deadline_ms + 900, &env).0.phase, Phase::Executing);
    }

    /// T30.2: the "Close" button of the island only asks; the program is closed after the "yes", and one that stays is
    /// ended by force only after another "yes".
    #[test]
    fn a_program_is_closed_only_after_a_yes_and_forced_only_after_another() {
        let close = |force| ToolCall::CloseHeavy { exe: "C:\\g\\game.exe".into(), name: "Game".into(), force };
        let propose = Event::Propose { id: "00112233aabbccdd".into(), call: Validated { call: close(false), confirm: true, summary: "Close Game?".into(), key: Some("voice.confirm.close_heavy"), args: Default::default() } };
        let (s, fx) = run(vec![propose.clone()]);
        assert!(matches!(s.phase, Phase::Confirming { .. }) && s.source == Some(Source::Pc));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))), "the button alone closes nothing");
        let yes = Event::Confirm { id: "00112233aabbccdd".into(), yes: true };
        let (s, fx) = run(vec![propose.clone(), Event::Confirm { id: "00112233aabbccdd".into(), yes: false }]);
        assert_eq!(s.phase, Phase::Done(msg::CANCELLED.into()));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))));
        let (_, fx) = run(vec![propose.clone(), yes.clone()]);
        assert!(fx.contains(&Effect::Execute(close(false))), "{fx:?}");
        let stays = Event::Executed(Ok(super::super::heavy::force_reply("C:\\g\\game.exe", "Game")));
        let (s, fx) = run(vec![propose.clone(), yes.clone(), stays.clone()]);
        let Phase::Confirming { call, .. } = &s.phase else { panic!("{:?}", s.phase) };
        assert_eq!((call.call.clone(), call.key), (close(true), Some("voice.confirm.close_force")));
        assert!(!fx.contains(&Effect::Execute(close(true))), "never by force without the second yes");
        assert!(run(vec![propose, yes.clone(), stays, yes]).1.contains(&Effect::Execute(close(true))));
        // While something else is being asked or done, the button changes nothing.
        let busy = vec![start(Source::Pc), Event::Propose { id: "ffffffffffffffff".into(), call: Validated { call: close(false), confirm: true, summary: String::new(), key: None, args: Default::default() } }];
        assert_eq!(run(busy).0.phase, Phase::Listening);
    }

    /// T29.6: a new button is written only after a "yes".
    #[test]
    fn a_button_is_added_only_after_a_yes() {
        let add = RawCall { tool: "add_button".into(), args: json!({ "catalogId": "win.lock" }) };
        let (t, mut c) = (table(), ctx());
        c.pages = vec![crate::voice::tools::PageRef { id: "main".into(), name: "Main".into() }];
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let go = |last: Event| {
            let mut state = State::idle();
            let mut all = Vec::new();
            for e in [start(Source::Phone), Event::AudioEnd, said("make me one"), Event::LlmAnswer(Some((add.clone(), None))), last] {
                let (s, fx) = step(&state, e, 1_000, &env);
                state = s;
                all.extend(fx);
            }
            (state, all)
        };
        let adds = |fx: &[Effect]| fx.iter().any(|e| matches!(e, Effect::Execute(ToolCall::AddButton { .. })));
        let id = "00112233aabbccdd".to_string();
        assert!(adds(&go(Event::Confirm { id: id.clone(), yes: true }).1));
        let (s, fx) = go(Event::Confirm { id, yes: false });
        assert!(!adds(&fx) && s.phase == Phase::Done(msg::CANCELLED.into()));
        assert!(!adds(&go(Event::Tick { now_ms: u64::MAX }).1), "a timeout writes nothing");
    }

    /// T29.8: "stop" over a spoken answer does nothing else (it must not pause the music).
    #[test]
    fn stop_said_over_a_spoken_answer_only_stops_it() {
        let (t, mut c) = (table(), ctx());
        c.speech_interrupted = true;
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let mut state = State::idle();
        let mut all = Vec::new();
        for e in [start(Source::Pc), Event::AudioEnd, said("stop")] {
            let (s, fx) = step(&state, e, 1, &env);
            state = s;
            all.extend(fx);
        }
        assert!(matches!(state.phase, Phase::Done(_)));
        assert!(!all.iter().any(|e| matches!(e, Effect::Execute(_) | Effect::AskLlm(_))), "{all:?}");
    }

    #[test]
    fn silence_and_failures_end_with_the_standard_texts() {
        assert_eq!(run(vec![start(Source::Phone), Event::AudioEnd, said("   ")]).0.phase, Phase::Failed(msg::DIDNT_CATCH.into()));
        assert_eq!(run(vec![start(Source::Phone), Event::AudioEnd, Event::Transcript(Err("whisper is down".into()))]).0.phase, Phase::Failed("whisper is down".into()));
        // A known phrase for a program that is not installed: the model is asked first (the template may have swallowed a
        // phrase for another tool); with no answer the table's own refusal is what the user reads. No action either way.
        let (s, fx) = run(vec![start(Source::Phone), Event::AudioEnd, said("open powershell")]);
        assert_eq!(s.phase, Phase::Thinking);
        assert!(fx.contains(&Effect::AskLlm("open powershell".into())) && !fx.iter().any(|e| matches!(e, Effect::Execute(_))));
        let (s, fx) = run(vec![start(Source::Phone), Event::AudioEnd, said("open powershell"), Event::LlmAnswer(None)]);
        assert!(matches!(&s.phase, Phase::Failed(m) if m.starts_with("Can't do that")), "{:?}", s.phase);
        assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))));
        // The model may know better: "open main page" is a page, not a program.
        let (s, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("open powershell"), Event::LlmAnswer(Some((RawCall { tool: "open_app".into(), args: json!({ "name": "calculator" }) }, None)))]);
        assert_eq!(s.phase, Phase::Executing);
    }

    #[test]
    fn an_unknown_phrase_goes_to_the_model_and_its_answer_is_checked_again() {
        let (s, fx) = run(vec![start(Source::Phone), Event::AudioEnd, said("what a lovely day")]);
        assert_eq!(s.phase, Phase::Thinking);
        assert!(fx.contains(&Effect::AskLlm("what a lovely day".into())));
        let ask = |a| run(vec![start(Source::Phone), Event::AudioEnd, said("what a lovely day"), Event::LlmAnswer(a)]);
        let good = ask(Some((RawCall { tool: "open_app".into(), args: json!({ "name": "calculator" }) }, Some("Opening it".into()))));
        assert_eq!(good.0.phase, Phase::Executing);
        // The model's own words replace the standard result.
        let (done, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("what a lovely day"), Event::LlmAnswer(Some((RawCall { tool: "open_app".into(), args: json!({ "name": "calculator" }) }, Some("Opening it".into())))), Event::Executed(Ok("Opened Calculator".into()))]);
        assert_eq!(done.phase, Phase::Done("Opening it".into()));
        // Anything the rules refuse is "Didn't get that", with no action.
        for bad in [
            RawCall { tool: "launch".into(), args: json!({ "program": "cmd.exe" }) },
            RawCall { tool: "run_action".into(), args: json!({ "id": "format-c" }) },
            RawCall { tool: "open_app".into(), args: json!({ "name": "calculator", "args": "--evil" }) },
        ] {
            let (s, fx) = ask(Some((bad, None)));
            assert_eq!(s.phase, Phase::Failed(msg::DIDNT_GET.into()));
            assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))));
        }
        assert_eq!(ask(None).0.phase, Phase::Failed(msg::DIDNT_GET.into()));
    }

    #[test]
    fn an_ambiguous_phrase_asks_and_does_not_act() {
        let t = Table::parse(r#"{"lang":"en","intents":[{"tool":"volume","args":{"op":"mute"},"patterns":["sound off"]},{"tool":"system","args":{"op":"sleep"},"patterns":["sound off"]}]}"#).unwrap();
        let c = ctx();
        let env = Env { table: &t, ctx: &c, confirm_ms: 1, dictate: false };
        let mut st = State::idle();
        for e in [start(Source::Phone), Event::AudioEnd, said("sound off")] {
            let (s, fx) = step(&st, e, 0, &env);
            assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))));
            st = s;
        }
        let Phase::Done(q) = st.phase else { panic!() };
        assert!(q.starts_with("Which one:") && q.contains("volume mute") && q.contains("system sleep"), "{q}");
    }

    #[test]
    fn cancel_works_everywhere_and_a_busy_pipeline_ignores_a_second_start() {
        let (s, fx) = run(vec![start(Source::Pc), Event::Cancel]);
        assert_eq!(s.phase, Phase::Idle);
        assert!(fx.contains(&Effect::StopCapture));
        let (s, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("lock computer"), Event::Cancel]);
        assert_eq!(s.phase, Phase::Idle);
        let (s, _) = run(vec![start(Source::Phone), Event::Start { id: "ffffffffffffffff".into(), source: Source::Pc }]);
        assert_eq!((s.phase, s.id.as_deref()), (Phase::Listening, Some("00112233aabbccdd")));
        // After a result the next request may start at once.
        assert_eq!(run(vec![start(Source::Phone), Event::AudioEnd, said("   "), start(Source::Phone)]).0.phase, Phase::Listening);
    }

    #[test]
    fn the_section_follows_the_protocol_shape() {
        let (s, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("lock computer")]);
        let v = serde_json::to_value(s.info()).unwrap();
        assert_eq!(v["state"], "confirm");
        assert_eq!((v["id"].as_str(), v["source"].as_str(), v["heard"].as_str()), (Some("00112233aabbccdd"), Some("phone"), Some("lock computer")));
        assert_eq!(v["confirm"]["text"], "Lock the computer");
        assert!(v.get("reply").is_none());
        assert_eq!(serde_json::to_value(State::idle().info()).unwrap(), json!({ "state": "idle" }));
        // Long texts are cut to 300 characters.
        let mut long = State::idle();
        long.phase = Phase::Done("x".repeat(500));
        assert_eq!(long.info().reply.unwrap().chars().count(), 300);
    }

    /// T28.2: a failure of the servers is a fixed phrase with its key, never the raw line of the process.
    #[test]
    fn a_failed_server_says_why_with_a_key() {
        use crate::ai::failure::Reason;
        let thinking = vec![start(Source::Pc), Event::AudioEnd, said("hm")];
        for (reason, expected) in [
            (Reason::Vram, "m28.err_vram_short"),
            (Reason::Ram, "m28.err_ram_short"),
            (Reason::ModelFile, "m28.err_model_file_short"),
            (Reason::Port, "m28.err_port_short"),
            (Reason::Crash, "m28.err_crash_short"),
            (Reason::Stt, "m28.err_stt_short"),
            (Reason::Mic, "voice.reply.mic_unavailable"),
        ] {
            let mut events = thinking.clone();
            events.push(Event::ModelFailed(failure_msg(reason)));
            let (s, _) = run(events);
            assert_eq!((s.info().state, s.info().reply_key), ("error", Some(expected)), "{reason:?}");
        }
        // Also while the pill says "loading".
        let (s, _) = run(vec![start(Source::Pc), Event::AudioEnd, said("hm"), Event::ModelLoading, Event::ModelFailed(msg::FAIL_RAM)]);
        assert_eq!(s.info().reply.as_deref(), Some("Not enough RAM for the model"));
        // The recognition that could not start.
        let (s, _) = run(vec![start(Source::Pc), Event::AudioEnd, Event::Transcript(Err(msg::FAIL_STT.into()))]);
        assert_eq!(s.info().reply_key, Some("m28.err_stt_short"));
        // A phrase of the tables needs no model.
        assert_eq!(run(vec![start(Source::Pc), Event::AudioEnd, said("volume up")]).0.phase, Phase::Executing);
    }

    /// T28.3: "Heard" -> "Understood" -> doing it; the stages are for the window of the PC, not for the phones.
    #[test]
    fn a_command_shows_what_was_heard_understood_and_that_it_is_being_done() {
        let (s, _) = run(vec![start(Source::Pc), Event::AudioEnd]);
        assert_eq!((s.info().heard, s.info().understood, s.info().executing), (None, None, false));
        let (s, _) = run(vec![start(Source::Pc), Event::AudioEnd, said("volume up")]);
        let info = s.info();
        assert_eq!((info.heard.as_deref(), info.understood.as_ref().and_then(|u| u.key), info.executing), (Some("volume up"), Some("voice.did.volume"), true));
        let phone = serde_json::to_value(&info).unwrap();
        assert!(phone.get("understood").is_none() && phone.get("executing").is_none());
        let pill = crate::voice::host::pill_payload(&info, &[], 0.0);
        assert_eq!((pill["understood"]["key"].as_str(), pill["executing"].as_bool()), (Some("voice.did.volume"), Some(true)));
        // The result keeps what was understood; nothing is being done any more.
        let (done, _) = run(vec![start(Source::Pc), Event::AudioEnd, said("volume up"), Event::Executed(Ok(String::new()))]);
        assert_eq!((done.info().understood.is_some(), done.info().executing), (true, false));
        // After a "yes" the confirmed call is named by its own key.
        let (s, _) = run(vec![start(Source::Pc), Event::AudioEnd, said("lock computer"), Event::Confirm { id: "00112233aabbccdd".into(), yes: true }]);
        assert_eq!(s.info().understood.and_then(|u| u.key), Some("voice.confirm.lock"));
    }

    // ---- Driver on fakes ----

    #[derive(Default)]
    struct Fake {
        executed: Mutex<Vec<ToolCall>>,
        published: Mutex<Vec<VoiceInfo>>,
        heard: Mutex<Vec<usize>>,
        transcript: Mutex<String>,
        broken_model: Mutex<Option<&'static str>>,
    }

    impl Ports for Fake {
        fn env(&self) -> (Arc<Table>, Context) {
            (Arc::new(table()), ctx())
        }
        fn transcribe(&self, pcm: Vec<u8>) -> BoxFut<Result<String, String>> {
            self.heard.lock().unwrap().push(pcm.len());
            let t = self.transcript.lock().unwrap().clone();
            Box::pin(async move { Ok(t) })
        }
        fn ask_llm(&self, _: String, _: Context) -> BoxFut<Option<(RawCall, Option<String>)>> {
            Box::pin(async { None })
        }
        fn model_failure(&self) -> Option<&'static str> {
            *self.broken_model.lock().unwrap()
        }
        fn execute(&self, call: ToolCall) -> BoxFut<Result<String, String>> {
            self.executed.lock().unwrap().push(call);
            Box::pin(async { Ok("Done".to_string()) })
        }
        fn capture(&self, _: bool) {}
        fn publish(&self, info: VoiceInfo) {
            self.published.lock().unwrap().push(info);
        }
    }

    async fn settle(f: &Fake, want: &str) {
        for _ in 0..100 {
            if f.published.lock().unwrap().last().is_some_and(|i| i.state == want) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("never reached {want}: {:?}", f.published.lock().unwrap().iter().map(|i| i.state).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn the_driver_runs_a_whole_request_on_fakes() {
        let fake = Arc::new(Fake::default());
        *fake.transcript.lock().unwrap() = "volume up".into();
        let d = Driver::new(tokio::runtime::Handle::current(), fake.clone());
        d.handle_event(start(Source::Phone));
        d.push_audio(&[0u8; 3200]);
        d.push_audio(&[0u8; 3200]);
        d.handle_event(Event::AudioEnd);
        settle(&fake, "done").await;
        assert_eq!(fake.heard.lock().unwrap().as_slice(), [6400], "the recording reached recognition");
        assert_eq!(fake.executed.lock().unwrap().len(), 1);
        let states: Vec<_> = fake.published.lock().unwrap().iter().map(|i| i.state).collect();
        assert_eq!(states, ["listening", "recognizing", "thinking", "done"]);
    }

    #[tokio::test]
    async fn the_driver_asks_why_when_the_model_gives_nothing() {
        let fake = Arc::new(Fake::default());
        *fake.transcript.lock().unwrap() = "something the tables do not know".into();
        *fake.broken_model.lock().unwrap() = Some(msg::FAIL_VRAM);
        let d = Driver::new(tokio::runtime::Handle::current(), fake.clone());
        d.handle_event(start(Source::Phone));
        d.push_audio(&[0u8; 3200]);
        d.handle_event(Event::AudioEnd);
        settle(&fake, "error").await;
        assert_eq!(fake.published.lock().unwrap().last().unwrap().reply_key, Some("m28.err_vram_short"));
        assert!(fake.executed.lock().unwrap().is_empty());
    }

    /// T23.B: the pill hung in `Listening…` after several commands in a row. Ten requests with every kind of ending,
    /// one after another: the phase is never left in a working state and always returns to idle.
    #[tokio::test]
    async fn ten_requests_in_a_row_always_return_to_idle() {
        let fake = Arc::new(Fake::default());
        let d = Driver::new(tokio::runtime::Handle::current(), fake.clone());
        let wait = |d: Driver| async move {
            for _ in 0..200 {
                if !matches!(d.state().phase, Phase::Listening | Phase::Transcribing | Phase::Loading | Phase::Thinking | Phase::Executing) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("stuck in {:?}", d.state().phase);
        };
        for i in 0..10u32 {
            *fake.transcript.lock().unwrap() = if i % 4 == 3 { "   ".into() } else if i % 4 == 2 { "tell me a story".into() } else { "volume up".into() };
            d.handle_event(Event::Start { id: format!("{i:016x}"), source: Source::Pc });
            assert_eq!(d.state().phase, Phase::Listening, "request {i} starts");
            d.push_audio(&[1u8; 3200]);
            match i % 5 {
                // The click on the pill or the hotkey while speaking.
                0 => {
                    d.note_speech(600);
                    d.handle_event(Event::AudioEnd);
                }
                // Silence after speech ends the phrase.
                1 => {
                    d.note_speech(900);
                    d.handle_event(Event::AudioEnd);
                }
                // A click before any speech, and silence for 5 s.
                2 => {
                    d.note_speech(0);
                    d.handle_event(Event::AudioEnd);
                }
                3 => d.handle_event(Event::NoSpeech),
                // Esc.
                _ => d.handle_event(Event::Cancel),
            }
            wait(d.clone()).await;
            // The result stays for a while, then the reset of the host (or a new request) follows.
            if matches!(d.state().phase, Phase::Done(_) | Phase::Failed(_)) {
                // A new request may start over a finished one at once.
                if i % 3 == 0 {
                    d.handle_event(Event::Start { id: format!("{:016x}", 100 + i), source: Source::Pc });
                    assert_eq!(d.state().phase, Phase::Listening);
                    d.handle_event(Event::Watchdog);
                    // A reset from the earlier request is stale and changes nothing.
                }
                d.handle_event(Event::Reset);
            }
            assert_eq!(d.state().phase, Phase::Idle, "request {i} returns to idle");
            assert_eq!(d.state().id, None);
        }
        assert_eq!(fake.published.lock().unwrap().last().unwrap().state, "idle");
    }

    #[tokio::test]
    async fn the_driver_cancels_an_unanswered_confirmation_and_never_executes() {
        let fake = Arc::new(Fake::default());
        *fake.transcript.lock().unwrap() = "lock computer".into();
        let d = Driver::with_confirm_timeout(tokio::runtime::Handle::current(), fake.clone(), 60);
        d.handle_event(start(Source::Phone));
        d.handle_event(Event::AudioEnd);
        settle(&fake, "confirm").await;
        settle(&fake, "done").await;
        assert_eq!(fake.published.lock().unwrap().last().unwrap().reply.as_deref(), Some(msg::CANCELLED));
        assert!(fake.executed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_driver_runs_a_confirmed_call_once() {
        let fake = Arc::new(Fake::default());
        *fake.transcript.lock().unwrap() = "lock computer".into();
        let d = Driver::new(tokio::runtime::Handle::current(), fake.clone());
        d.handle_event(start(Source::Phone));
        d.handle_event(Event::AudioEnd);
        settle(&fake, "confirm").await;
        d.handle_event(Event::Confirm { id: "00112233aabbccdd".into(), yes: true });
        d.handle_event(Event::Confirm { id: "00112233aabbccdd".into(), yes: true });
        settle(&fake, "done").await;
        assert_eq!(fake.executed.lock().unwrap().len(), 1);
    }

    #[test]
    fn fixed_phrases_carry_i18n_keys_and_free_text_does_not() {
        let (s, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("   ")]);
        let i = s.info();
        assert_eq!((i.reply_key, i.reply.as_deref()), (Some("voice.reply.didnt_catch"), Some(msg::DIDNT_CATCH)));
        let (s, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("please lock my pc")]);
        let c = s.info().confirm.unwrap();
        assert_eq!((c.key, c.text.as_str()), (Some("voice.confirm.lock"), "Lock the computer"));
        let (s, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("please lock my pc"), Event::Confirm { id: "00112233aabbccdd".into(), yes: false }]);
        assert_eq!(s.info().reply_key, Some("voice.reply.cancelled"));
        // The model's own answer is free text: no key.
        let (done, _) = run(vec![start(Source::Phone), Event::AudioEnd, said("what a lovely day"), Event::LlmAnswer(Some((RawCall { tool: "open_app".into(), args: json!({ "name": "calculator" }) }, Some("Opening it".into())))), Event::Executed(Ok(String::new()))]);
        let i = done.info();
        assert_eq!((i.reply_key, i.reply.as_deref()), (None, Some("Opening it")));
    }

    #[test]
    fn a_microphone_that_fails_ends_the_request_and_stops_the_capture() {
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let (listening, _) = step(&State::idle(), Event::Start { id: "0011223344556677".into(), source: Source::Pc }, 0, &env);
        let (failed, effects) = step(&listening, Event::CaptureFailed(msg::NO_MIC.into()), 1, &env);
        assert_eq!(failed.phase, Phase::Failed(msg::MIC_UNAVAILABLE.into()));
        assert_eq!(failed.info().reply_key, Some("voice.reply.mic_unavailable"));
        assert_eq!(effects, [Effect::StopCapture]);
        let (other, _) = step(&listening, Event::CaptureFailed("Microphone stopped".into()), 1, &env);
        assert_eq!(other.info().reply_key, Some("voice.reply.mic_unavailable"));
        // Outside of a recording it changes nothing.
        assert_eq!(step(&State::idle(), Event::CaptureFailed("x".into()), 1, &env).0, State::idle());
    }

    #[test]
    fn nothing_said_and_the_watchdog_end_the_request_with_their_own_phrases() {
        let (t, c) = (table(), ctx());
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS, dictate: false };
        let (listening, _) = step(&State::idle(), Event::Start { id: "0011223344556677".into(), source: Source::Pc }, 0, &env);
        let (quiet, effects) = step(&listening, Event::NoSpeech, 1, &env);
        assert_eq!((quiet.phase.clone(), quiet.info().reply_key), (Phase::Failed(msg::NO_SPEECH.into()), Some("voice.reply.no_speech")));
        assert_eq!(effects, [Effect::StopCapture], "the sound does not go to recognition");
        for phase in [Phase::Listening, Phase::Transcribing, Phase::Loading, Phase::Thinking] {
            let mut s = listening.clone();
            s.phase = phase;
            let (w, effects) = step(&s, Event::Watchdog, 1, &env);
            assert_eq!((w.phase.clone(), w.info().reply_key, effects), (Phase::Failed(msg::WRONG.into()), Some("voice.reply.wrong"), vec![Effect::StopCapture]));
        }
        assert_eq!(step(&State::idle(), Event::Watchdog, 1, &env).0, State::idle());
        let (done, _) = step(&quiet, Event::Watchdog, 1, &env);
        assert_eq!(done, quiet, "a finished request is not touched");
    }
}
