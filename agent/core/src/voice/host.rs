//! Voice control inside the running agent (M21 C2): connects the pipeline (`pipeline::Driver`) to the things that exist
//! only there — the microphone, the hotkey, whisper, the action runners, the `voice` section of the state and the pill.
//! The decisions (`press_event`, `pill_cmds`, `tool_action`, `build_context`, `pill_payload`) are pure functions with tests;
//! `Host` is the glue. The window of the pill is created by the application through the `PillHook`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::endpoint::Verdict;
use super::hotkey::{self, Pressed, Plan, Tracker};
use super::matcher::Table;
use super::mic;
use super::pipeline::{msg, BoxFut, Driver, Event, Phase, Ports, Source, State, VoiceInfo};
use super::winops;
use super::tools::{ActionRef, AppRef, ButtonRef, Context, PageRef, RawCall, ToolCall, VolumeOp};
use crate::agent::Agent;
use crate::ai::service::{VoiceService, VoiceState, VoiceStatus};
use crate::ai::supervisor::SupState;
use crate::installed::InstalledApp;
use crate::model::{Action, ActionsFile, LayoutFile, LayoutItem, MediaOp, VarValue};

/// A finished request stays on the screen this long, then the pipeline returns to idle and the pill goes away.
pub const RESULT_SHOWN_MS: u64 = 4_000;
/// A request that does not change its state for this long is cancelled (DATA_MODEL.md 12.6a).
pub const WATCHDOG_MS: u64 = 30_000;
/// An opened microphone that gives no chunk for this long is unavailable.
pub const MIC_SILENT_MS: u64 = 1_000;
/// The decisions of this many requests after the start are written to `voice.log`.
const LOGGED_FIRST: usize = 5;
/// A press or a click that arrives sooner than this after the start of a PC recording cannot end it.
const MIN_RECORDING_MS: u128 = 400;
static PC_STARTED: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

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
        (Phase::Listening, Some(Source::Pc)) => Some(Event::AudioEnd),
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

/// The value of the variable `voice.goto`: the page and a counter, so going to the same page twice is a new value.
/// The phones watch the variable and navigate (they never send the page back).
pub fn goto_value(page_id: &str, seq: u64) -> String {
    format!("{page_id}#{seq}")
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
    Context {
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

/// Whether voice control is on for this PC: the models are there (the servers start with the first request).
pub fn is_on(status: &VoiceStatus) -> bool {
    matches!(status.state, VoiceState::Ready | VoiceState::Loading)
}

// ---- The glue ---------------------------------------------------------------------------------------------------

pub type PillHook = Arc<dyn Fn(PillCmd) + Send + Sync>;

struct Inner {
    handle: tokio::runtime::Handle,
    /// "Dictate" waits for the next phrase until this moment.
    dictate_until: Arc<Mutex<Option<Instant>>>,
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
}

#[derive(Clone)]
pub struct Host {
    inner: Arc<Inner>,
}

impl Host {
    pub fn new(handle: tokio::runtime::Handle, agent: Agent, service: VoiceService, pill: PillHook) -> Host {
        let inner = Arc::new(Inner {
            handle: handle.clone(),
            dictate_until: Arc::new(Mutex::new(None)),
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
            #[cfg(windows)]
            hotkeys: Mutex::new(None),
            #[cfg(windows)]
            mic: Mutex::new(None),
            last: Mutex::new(None),
            chunks: Arc::new(AtomicUsize::new(0)),
            changes: Arc::new(AtomicU64::new(0)),
        });
        let driver = Driver::new(handle.clone(), inner.clone());
        let _ = inner.driver.set(driver);
        let host = Host { inner };
        // The list of programs takes a moment to read: not on the first request.
        let weak = Arc::downgrade(&host.inner);
        handle.spawn_blocking(move || {
            let apps = crate::installed::list_installed_apps();
            if let Some(inner) = weak.upgrade() {
                *inner.installed.lock().unwrap() = apps;
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
    }

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

    /// `voice_confirm` of the pill (IPC).
    pub fn confirm(&self, id: &str, yes: bool) {
        self.inner.driver().handle_event(Event::Confirm { id: id.to_string(), yes });
    }

    /// `voice_stop` of the pill (IPC): the click on the stop button or on the pill ends the recording of the PC at once.
    pub fn stop(&self) {
        let driver = self.inner.driver();
        let s = driver.state();
        mic::log_line("voice_stop from the pill");
        if s.phase == Phase::Listening && s.source == Some(Source::Pc) && !too_early() {
            driver.handle_event(Event::AudioEnd);
        }
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

    fn pressed(&self, p: Pressed) {
        let state = self.driver().state();
        mic::log_line(&format!("hotkey {p:?} phase={:?}", std::mem::discriminant(&state.phase)));
        let event = match p {
            Pressed::Main => press_event(&state, format!("{:016x}", rand::random::<u64>())),
            Pressed::Esc => esc_event(&state),
            Pressed::Enter => enter_event(&state),
        };
        if let Some(e) = event {
            if e == Event::AudioEnd && too_early() {
                mic::log_line("end ignored: too early");
                return;
            }
            if matches!(e, Event::Start { .. }) {
                *PC_STARTED.lock().unwrap() = Some(Instant::now());
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

    fn show(&self, info: &VoiceInfo, level: f32) {
        let hotkey = self.service.status().hotkey;
        if let Some(cmd) = pill_cmd(info, &hotkey, level) {
            (self.pill)(cmd);
        }
    }
}

impl Ports for Inner {
    fn env(&self) -> (Arc<Table>, Context) {
        let lang = self.language();
        let ctx = build_context(lang, &self.agent.actions(), &self.agent.layout(), &self.installed.lock().unwrap());
        (self.table(lang), ctx)
    }

    fn dictating(&self) -> bool {
        dictation_waiting(*self.dictate_until.lock().unwrap(), Instant::now())
    }

    fn transcribe(&self, pcm: Vec<u8>) -> BoxFut<Result<String, String>> {
        let (sup, lang) = (self.service.supervisor().clone(), self.language());
        Box::pin(async move {
            // Less than a tenth of a second is a key click, not speech.
            if pcm.len() < super::MAX_CHUNK_BYTES / 2 {
                return Ok(String::new());
            }
            sup.transcribe(&pcm, lang).await
        })
    }

    fn ask_llm(&self, text: String, ctx: Context) -> BoxFut<Option<(RawCall, Option<String>)>> {
        let (sup, lang) = (self.service.supervisor().clone(), self.language());
        Box::pin(async move { super::llm::ask(&sup, lang, &text, &ctx).await })
    }

    fn execute(&self, call: ToolCall) -> BoxFut<Result<String, String>> {
        let agent = self.agent.clone();
        let (dictate, timers, goto_seq) = (self.dictate_until.clone(), self.timers.clone(), self.goto_seq.clone());
        let handle = self.handle.clone();
        Box::pin(async move {
            if let ToolCall::RunAction { action_id } = &call {
                return agent.execute(action_id).await.map(|_| String::new()).map_err(|(_, m)| m);
            }
            match &call {
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
                ToolCall::TypeText { text } => {
                    *dictate.lock().unwrap() = None;
                    let text = text.clone();
                    return tokio::task::spawn_blocking(move || winops::type_text(&text).map(|_| String::new()))
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
                ToolCall::GoPage { page_id, name } => {
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
            let wanted = self.service.status().mic;
            let started = {
                let (feed, fail) = (self.driver().clone(), self.driver().clone());
                let pill_self = self.pill.clone();
                let hotkey = self.service.status().hotkey;
                let last_info = self.last.lock().unwrap().clone();
                self.chunks.store(0, Ordering::SeqCst);
                let (counter, decided) = (self.chunks.clone(), Arc::new(AtomicBool::new(false)));
                // The decisions of the first requests after the start (or all of them with `DECKA_VOICE_DEBUG`) go to voice.log.
                let log = std::env::var_os("DECKA_VOICE_DEBUG").is_some() || LOGGED_REQUESTS.fetch_add(1, Ordering::SeqCst) < LOGGED_FIRST;
                let handle = self.handle.clone();
                mic::start(
                    wanted,
                    log,
                    move |fed| {
                        counter.fetch_add(1, Ordering::SeqCst);
                        if log {
                            mic::log_line(&mic::decision_line(&fed.decision, fed.verdict));
                        }
                        feed.push_audio(&fed.pcm);
                        // The wave of the pill follows the voice (a chunk is 100 ms, so at most 10 times a second).
                        if let Some(info) = last_info.as_ref().filter(|i| i.state == "listening") {
                            if let Some(cmd) = pill_cmd(info, &hotkey, fed.level) {
                                // Windows of the pill belong to the main thread: never wait for it on the audio thread.
                                let pill_self = pill_self.clone();
                                handle.spawn_blocking(move || pill_self(cmd));
                            }
                        }
                        let event = match fed.verdict {
                            Verdict::Continue => return,
                            Verdict::End => Event::AudioEnd,
                            Verdict::NoSpeech => Event::NoSpeech,
                        };
                        // Once, and not on the audio thread: the event stops the capture, which waits for this very thread.
                        if feed.state().phase == Phase::Listening && !decided.swap(true, Ordering::SeqCst) {
                            let driver = feed.clone();
                            std::thread::spawn(move || driver.handle_event(event));
                        }
                    },
                    move |e| fail.handle_event(Event::CaptureFailed(e)),
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
        *self.last.lock().unwrap() = Some(info.clone());
        self.agent.store().set("voice", serde_json::to_value(&info).unwrap_or(Value::Null));
        self.show(&info, 0.0);
        self.set_aux(&state);
        // The watchdog: a request that waits in one state for 30 s is cancelled.
        let seen = self.changes.fetch_add(1, Ordering::SeqCst) + 1;
        if matches!(info.state, "listening" | "loading" | "thinking") {
            let (driver, changes) = (self.driver().clone(), self.changes.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(WATCHDOG_MS));
                if changes.load(Ordering::SeqCst) == seen {
                    driver.handle_event(Event::Watchdog);
                }
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
        State { phase, id: Some("0011223344556677".into()), source: Some(source), heard: None, say: None, reply_key: None }
    }

    fn info(state: &'static str, source: Option<Source>) -> VoiceInfo {
        VoiceInfo { state, id: Some("x".into()), source, heard: None, reply: None, reply_key: None, reply_args: Default::default(), confirm: None }
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
        let confirming = state(Phase::Confirming { call, deadline_ms: 1 }, Source::Pc);
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
