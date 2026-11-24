//! Voice control inside the running agent (M21 C2): connects the pipeline (`pipeline::Driver`) to the things that exist
//! only there — the microphone, the hotkey, whisper, the action runners, the `voice` section of the state and the pill.
//! The decisions (`press_event`, `pill_cmds`, `tool_action`, `build_context`, `pill_payload`) are pure functions with tests;
//! `Host` is the glue. The window of the pill is created by the application through the `PillHook`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::dictation::{self, DictationSettings, Mode, Trigger};
use super::endpoint::Verdict;
use super::hotkey::{self, Pressed, Plan, Tracker};
use super::matcher::Table;
use super::mic;
use super::pipeline::{msg, BoxFut, Driver, Event, Phase, Ports, Source, State, VoiceInfo};
use super::winops;
use super::tools::{ActionRef, AppRef, ButtonRef, Context, PageRef, RawCall, ToolCall, VolumeOp};
use crate::agent::Agent;
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
        Phase::Loading => 120_000,
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

/// `day_brief` (13.5): the facts the agent already has, put into words by the model; the plain facts when the model fails the check.
async fn day_brief(agent: &Agent, sup: &crate::ai::supervisor::Supervisor, lang: &str) -> Result<String, String> {
    use super::brief;
    let store = agent.store();
    let (assistant, now) = (agent.assistant(), crate::assistant::Local::now());
    let lists = assistant.lists();
    let facts = brief::facts(&store.get("weather"), &store.get("mail"), &store.get("ai"), &assistant.upcoming(50), &lists, &now);
    let Some(facts) = facts else { return Ok(brief::NOTHING.into()) };
    let body = brief::request_body(&facts, lang);
    let answer = super::assist::chat_text(sup, &body, brief::BRIEF_TIMEOUT).await;
    Ok(answer.and_then(|a| brief::check_brief(&facts, &a)).unwrap_or_else(|| brief::facts_text(&facts, &lists)))
}

/// `screen_ask` (13.3): the Screen help package (downloaded once, with the consent given in the confirmation), a picture of the
/// active window in memory, one question to the model on this PC.
async fn screen_ask(service: &VoiceService, sup: &crate::ai::supervisor::Supervisor, lang: &str, question: &str) -> Result<String, String> {
    if !service.vision_installed() {
        service.install_vision().await?;
    }
    // The picture is taken after the download and the restart of the server, so that it is the window the user is looking at now.
    let data_url = tokio::task::spawn_blocking(super::screen::capture_data_url).await.map_err(|e| format!("The action did not finish: {e}"))??;
    let body = super::screen::request_body(question, lang, &data_url);
    drop(data_url);
    let answer = super::assist::chat_text(sup, &body, super::screen::ASK_TIMEOUT).await;
    answer.as_deref().and_then(super::screen::accept_answer).ok_or_else(|| super::textops::NO_ANSWER.to_string())
}

/// `text_transform` (13.2): the selection of the focused program is taken, rewritten by the model and pasted back (or shown).
#[cfg(windows)]
async fn transform_selection(sup: &crate::ai::supervisor::Supervisor, lang: &str, op: super::textops::Op, target: Option<&str>) -> Result<String, String> {
    use super::clipboard::{SystemClipboard, SystemKeys};
    use super::textops;
    let grabbed = tokio::task::spawn_blocking(|| textops::take_selection(&SystemClipboard, &SystemKeys, &std::thread::sleep))
        .await
        .map_err(|e| format!("The action did not finish: {e}"))??;
    let body = textops::request_body(op, target, lang, &grabbed.text);
    let answer = super::assist::chat_text(sup, &body, textops::TRANSFORM_TIMEOUT).await;
    tokio::task::spawn_blocking(move || textops::finish(&SystemClipboard, &SystemKeys, op, &grabbed, answer.as_deref(), &std::thread::sleep))
        .await
        .map_err(|e| format!("The action did not finish: {e}"))?
}

#[cfg(not(windows))]
async fn transform_selection(_: &crate::ai::supervisor::Supervisor, _: &str, _: super::textops::Op, _: Option<&str>) -> Result<String, String> {
    Err("unsupported platform".into())
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
        vision_ready: false,
        memory: Vec::new(),
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
}

#[derive(Clone)]
pub struct Host {
    inner: Arc<Inner>,
}

impl Host {
    pub fn new(handle: tokio::runtime::Handle, agent: Agent, service: VoiceService, pill: PillHook) -> Host {
        let dictation = DictationSettings::load(agent.config_dir());
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
        });
        let driver = Driver::new(handle.clone(), inner.clone());
        let _ = inner.driver.set(driver);
        let host = Host { inner };
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

    /// `voice_confirm` of the pill (IPC).
    pub fn confirm(&self, id: &str, yes: bool) {
        self.inner.driver().handle_event(Event::Confirm { id: id.to_string(), yes });
    }

    /// Starts or ends a PC voice request, as if the hotkey were pressed.
    pub fn trigger(&self) {
        self.inner.pressed(Pressed::Main);
    }

    /// The current state of the pipeline.
    pub fn state(&self) -> State {
        self.inner.driver().state()
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

    fn pressed(&self, p: Pressed) {
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
        if self.driver().state().phase != Phase::Idle {
            return;
        }
        let info = |state: &'static str, reply: Option<String>| VoiceInfo {
            state,
            id: reply.is_some().then(|| "0000000000000000".to_string()),
            source: reply.is_some().then_some(Source::Pc),
            heard: None,
            reply,
            reply_key: None,
            reply_args: Default::default(),
            confirm: None,
            activity: None,
        };
        self.show(&info("done", Some(text.chars().take(200).collect())), 0.0);
        let weak = Arc::downgrade(self);
        self.handle.spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if let Some(inner) = weak.upgrade() {
                // A voice request that started meanwhile owns the pill now.
                if inner.driver().state().phase == Phase::Idle {
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

impl Ports for Inner {
    fn env(&self) -> (Arc<Table>, Context) {
        let lang = self.language();
        let mut ctx = build_context(lang, &self.agent.actions(), &self.agent.layout(), &self.installed.lock().unwrap());
        ctx.vision_ready = self.service.vision_installed();
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
        Box::pin(async move {
            // Less than a tenth of a second is a key click, not speech.
            if pcm.len() < super::MAX_CHUNK_BYTES / 2 {
                return Ok(String::new());
            }
            // The warm-up has not finished by the end of the phrase: only now the pill says so.
            if sup.state() == SupState::Loading {
                driver.handle_event(Event::ModelLoading);
            }
            if !dictating {
                return sup.transcribe(&pcm, lang).await;
            }
            let prompt = dictation::whisper_prompt(&settings.dictionary);
            let raw = sup.transcribe_prompted(&pcm, lang, prompt.as_deref()).await?;
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
        Box::pin(async move { super::llm::ask(&sup, lang, &text, &ctx).await })
    }

    fn execute(&self, call: ToolCall) -> BoxFut<Result<String, String>> {
        let agent = self.agent.clone();
        let (dictate, timers, goto_seq) = (self.dictate_until.clone(), self.timers.clone(), self.goto_seq.clone());
        let handle = self.handle.clone();
        let (sup, lang, service) = (self.service.supervisor().clone(), self.language(), self.service.clone());
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
                ToolCall::Remind { text, when } => {
                    let now = crate::assistant::Local::now();
                    let r = agent.assistant().add_reminder(text, when, &now)?;
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
                    return screen_ask(&service, &sup, lang, question).await;
                }
                ToolCall::TextTransform { op, lang: target } => {
                    return transform_selection(&sup, lang, *op, target.as_deref()).await;
                }
                ToolCall::PasteText { text } => {
                    *dictate.lock().unwrap() = None;
                    let text = text.clone();
                    return tokio::task::spawn_blocking(move || paste_system(&text))
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
                            if s.phase != Phase::Listening || s.id != id {
                                return;
                            }
                            driver.push_audio(chunk);
                        }
                        std::thread::sleep(Duration::from_millis(300));
                        let s = driver.state();
                        if s.phase == Phase::Listening && s.id == id {
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
                        if feed.state().phase == Phase::Listening && !decided.swap(true, Ordering::SeqCst) {
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
        // Warm-up: the servers start while the phrase is still being recorded (a no-op when they run already).
        if info.state == "listening" && !was_listening {
            let sup = self.service.supervisor().clone();
            self.handle.spawn(async move {
                if let Err(e) = sup.ensure_started().await {
                    mic::log_line(&format!("warm-up failed: {e}"));
                }
            });
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
        State { phase, id: Some("0011223344556677".into()), source: Some(source), heard: None, say: None, reply_key: None, activity: None }
    }

    fn info(state: &'static str, source: Option<Source>) -> VoiceInfo {
        VoiceInfo { state, id: Some("x".into()), source, heard: None, reply: None, reply_key: None, reply_args: Default::default(), confirm: None, activity: None }
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
