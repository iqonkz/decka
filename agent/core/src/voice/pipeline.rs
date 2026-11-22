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
    pub const CANCELLED: &str = "Cancelled";
    pub const DONE: &str = "Done";
    pub const NO_MIC: &str = "No microphone found";
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

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Idle,
    Listening,
    Transcribing,
    /// The servers are starting (the first request after a pause).
    Loading,
    /// Waiting for the model's answer.
    Thinking,
    Confirming { call: Validated, deadline_ms: u64 },
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
}

impl State {
    pub fn idle() -> State {
        State { phase: Phase::Idle, id: None, source: None, heard: None, say: None, reply_key: None }
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
            Phase::Transcribing | Phase::Thinking | Phase::Executing => ("thinking", None, None),
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
        }
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
    Transcript(Result<String, String>),
    /// The PC microphone could not be opened or died while recording.
    CaptureFailed(String),
    /// The model's answer: a tool call (with its short text) or nothing usable.
    LlmAnswer(Option<(RawCall, Option<String>)>),
    Confirm { id: String, yes: bool },
    Tick { now_ms: u64 },
    Executed(Result<String, String>),
    Reset,
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
}

fn describe(i: &Intent) -> String {
    let arg = i.args.values().filter_map(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string()))).collect::<Vec<_>>().join(" ");
    if arg.is_empty() { i.tool.replace('_', " ") } else { format!("{} {arg}", i.tool.replace('_', " ")) }
}

fn finish(mut s: State, phase: Phase) -> (State, Vec<Effect>) {
    s.phase = phase;
    s.reply_key = None;
    (s, Vec::new())
}

/// `finish` for a fixed phrase: the text is the English fallback of `key`.
fn finish_key(mut s: State, phase: Phase, key: ReplyKey) -> (State, Vec<Effect>) {
    s.phase = phase;
    s.reply_key = Some(key);
    (s, Vec::new())
}

/// Runs a tool call through `validate` and moves to confirm, execute, a question or a failure.
fn resolve(mut s: State, raw: &RawCall, now_ms: u64, env: &Env, from_model: bool) -> (State, Vec<Effect>) {
    match validate(raw, env.ctx) {
        Resolved::Ready(v) if v.confirm => {
            let deadline_ms = now_ms + env.confirm_ms;
            s.phase = Phase::Confirming { call: v, deadline_ms };
            (s, vec![Effect::ScheduleTick(env.confirm_ms)])
        }
        Resolved::Ready(v) => {
            s.phase = Phase::Executing;
            (s, vec![Effect::Execute(v.call)])
        }
        Resolved::Ask { question, options } => finish_key(s, Phase::Done(question), ReplyKey::with(key::WHICH, "options", &options.iter().take(3).cloned().collect::<Vec<_>>().join(" / "))),
        Resolved::Reject(_) if from_model => finish_key(s, Phase::Failed(msg::DIDNT_GET.into()), ReplyKey::plain(key::DIDNT_GET)),
        Resolved::Reject(why) => finish_key(s, Phase::Failed(format!("Can't do that: {why}")), ReplyKey::with(key::CANT, "why", &why)),
    }
}

/// The state machine. Events that make no sense in the current phase change nothing.
pub fn step(state: &State, event: Event, now_ms: u64, env: &Env) -> (State, Vec<Effect>) {
    let s = state.clone();
    match (&state.phase, event) {
        (Phase::Idle | Phase::Done(_) | Phase::Failed(_), Event::Start { id, source }) => {
            let effects = if source == Source::Pc { vec![Effect::StartCapture] } else { Vec::new() };
            (State { phase: Phase::Listening, id: Some(id), source: Some(source), heard: None, say: None, reply_key: None }, effects)
        }
        (Phase::Idle, Event::Cancel) => (s, Vec::new()),
        (Phase::Listening, Event::CaptureFailed(e)) => {
            let (mut next, _) = if e == msg::NO_MIC { finish_key(s, Phase::Failed(e), ReplyKey::plain(key::NO_MIC)) } else { finish(s, Phase::Failed(e)) };
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
        (Phase::Transcribing | Phase::Loading, Event::Transcript(Err(e))) => finish(s, Phase::Failed(e)),
        (Phase::Transcribing | Phase::Loading, Event::Transcript(Ok(text))) => {
            let text = text.trim().to_string();
            if text.is_empty() {
                return finish_key(s, Phase::Failed(msg::DIDNT_CATCH.into()), ReplyKey::plain(key::DIDNT_CATCH));
            }
            let mut s = s;
            s.heard = Some(text.clone());
            match env.table.matches(&text) {
                Match::Hit(intent) => resolve(s, &RawCall { tool: intent.tool, args: serde_json::Value::Object(intent.args) }, now_ms, env, false),
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
            None => finish_key(s, Phase::Failed(msg::DIDNT_GET.into()), ReplyKey::plain(key::DIDNT_GET)),
            Some((raw, say)) => {
                let mut s = s;
                s.say = say.filter(|t| !t.trim().is_empty());
                resolve(s, &raw, now_ms, env, true)
            }
        },
        // A spoken yes or no while a confirmation is open.
        (Phase::Confirming { call, .. }, Event::Transcript(Ok(text))) => match env.table.answer(&text) {
            Some(true) => {
                let call = call.call.clone();
                let mut s = s;
                s.phase = Phase::Executing;
                (s, vec![Effect::Execute(call)])
            }
            Some(false) => finish_key(s, Phase::Done(msg::CANCELLED.into()), ReplyKey::plain(key::CANCELLED)),
            None => (s, Vec::new()),
        },
        (Phase::Confirming { call, .. }, Event::Confirm { id, yes }) if state.id.as_deref() == Some(id.as_str()) => {
            if yes {
                let call = call.call.clone();
                let mut s = s;
                s.phase = Phase::Executing;
                (s, vec![Effect::Execute(call)])
            } else {
                finish_key(s, Phase::Done(msg::CANCELLED.into()), ReplyKey::plain(key::CANCELLED))
            }
        }
        (Phase::Confirming { deadline_ms, .. }, Event::Tick { now_ms }) if now_ms >= *deadline_ms => {
            finish_key(s, Phase::Done(msg::CANCELLED.into()), ReplyKey::plain(key::CANCELLED))
        }
        (Phase::Executing, Event::Executed(Ok(reply))) => {
            match (s.say.clone(), reply.is_empty()) {
                (Some(say), _) => finish(s, Phase::Done(say)),
                (None, true) => finish_key(s, Phase::Done(msg::DONE.to_string()), ReplyKey::plain(key::DONE)),
                (None, false) => finish(s, Phase::Done(reply)),
            }
        }
        (Phase::Executing, Event::Executed(Err(e))) => finish(s, Phase::Failed(e)),
        _ => (s, Vec::new()),
    }
}

// ---- Driver -----------------------------------------------------------------------------------------------------

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Everything the pipeline needs from outside: speech recognition, the model, the executors, the microphone, the screen.
pub trait Ports: Send + Sync + 'static {
    /// The phrase table and the things that exist on this PC, read fresh for every request.
    fn env(&self) -> (Arc<Table>, Context);
    /// 16 kHz mono PCM16 -> text.
    fn transcribe(&self, pcm: Vec<u8>) -> BoxFut<Result<String, String>>;
    /// The AI path; `None` = no usable answer.
    fn ask_llm(&self, text: String, ctx: Context) -> BoxFut<Option<(RawCall, Option<String>)>>;
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
        Driver { ports, shared: Arc::new(Mutex::new(Shared { state: State::idle(), audio: Vec::new() })), handle, t0: std::time::Instant::now(), confirm_ms }
    }

    pub fn state(&self) -> State {
        self.shared.lock().unwrap().state.clone()
    }

    pub fn push_audio(&self, pcm: &[u8]) {
        let mut sh = self.shared.lock().unwrap();
        if sh.state.phase == Phase::Listening {
            sh.audio.extend_from_slice(pcm);
        }
    }

    /// Feeds one event; effects run in the background and answer with further events.
    pub fn handle_event(&self, event: Event) {
        let (table, ctx) = self.ports.env();
        let env = Env { table: &table, ctx: &ctx, confirm_ms: self.confirm_ms };
        let now_ms = self.t0.elapsed().as_millis() as u64;
        let (info, effects, pcm) = {
            let mut sh = self.shared.lock().unwrap();
            let (next, effects) = step(&sh.state, event, now_ms, &env);
            let changed = next != sh.state;
            if matches!(next.phase, Phase::Listening) && !matches!(sh.state.phase, Phase::Listening) {
                sh.audio.clear();
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
                    me.handle_event(Event::LlmAnswer(a));
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
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS };
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
        let env = Env { table: &t, ctx: &c, confirm_ms: 10_000 };
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

    #[test]
    fn silence_and_failures_end_with_the_standard_texts() {
        assert_eq!(run(vec![start(Source::Phone), Event::AudioEnd, said("   ")]).0.phase, Phase::Failed(msg::DIDNT_CATCH.into()));
        assert_eq!(run(vec![start(Source::Phone), Event::AudioEnd, Event::Transcript(Err("whisper is down".into()))]).0.phase, Phase::Failed("whisper is down".into()));
        // A known phrase for a program that is not installed: no action.
        let (s, fx) = run(vec![start(Source::Phone), Event::AudioEnd, said("open powershell")]);
        assert!(matches!(s.phase, Phase::Failed(_)));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Execute(_))));
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
        let env = Env { table: &t, ctx: &c, confirm_ms: 1 };
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

    // ---- Driver on fakes ----

    #[derive(Default)]
    struct Fake {
        executed: Mutex<Vec<ToolCall>>,
        published: Mutex<Vec<VoiceInfo>>,
        heard: Mutex<Vec<usize>>,
        transcript: Mutex<String>,
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
        assert_eq!(states, ["listening", "thinking", "thinking", "done"]);
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
        let env = Env { table: &t, ctx: &c, confirm_ms: CONFIRM_TIMEOUT_MS };
        let (listening, _) = step(&State::idle(), Event::Start { id: "0011223344556677".into(), source: Source::Pc }, 0, &env);
        let (failed, effects) = step(&listening, Event::CaptureFailed(msg::NO_MIC.into()), 1, &env);
        assert_eq!(failed.phase, Phase::Failed(msg::NO_MIC.into()));
        assert_eq!(failed.info().reply_key, Some("voice.reply.no_mic"));
        assert_eq!(effects, [Effect::StopCapture]);
        let (other, _) = step(&listening, Event::CaptureFailed("Microphone stopped".into()), 1, &env);
        assert_eq!(other.info().reply_key, None);
        // Outside of a recording it changes nothing.
        assert_eq!(step(&State::idle(), Event::CaptureFailed("x".into()), 1, &env).0, State::idle());
    }}
