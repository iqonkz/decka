//! Running the screen help (T27.3-T27.6): the search, the press, the guide and the recording of a recipe. The world (the windows,
//! the clock, the mouse) and the model are traits, so every rule here is tested with fakes; `WinScreen` is the real world.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;

use super::guides::{Guides, Recipe, RecipeStep};
use super::highlight::{Highlight, Highlighter};
use super::screenhelp::{self, ActPlan, Finder, Guide, Step, StepError, UiAction};
use super::tools::GuideCtl;
use super::ui_snapshot::{Element, Rect, Snapshot};

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The windows, the mouse and the clock as the screen help sees them.
pub trait Screen: Send + Sync {
    fn snapshot(&self) -> Option<Snapshot>;
    /// Lower-case exe name of the active program.
    fn process(&self) -> String;
    fn foreground(&self) -> u64;
    fn perform(&self, rect: Rect, action: UiAction) -> Result<String, String>;
    /// A click in the middle of a block of text (T30.6): there is no element to invoke.
    fn click_text(&self, rect: Rect) -> Result<String, String> {
        self.perform(rect, UiAction::Click)
    }
    fn now_ms(&self) -> u64;
    /// A new left click since the last call, where it was.
    fn click(&self) -> Option<(i32, i32)>;
    /// Asks to be told on `events` that the window in front, the focus or the set of windows changed; the subscription
    /// lasts as long as what is returned. `None`: no such news here, the guide looks by itself.
    fn watch(&self, _events: mpsc::UnboundedSender<()>) -> Option<Box<dyn Send>> {
        None
    }
}

/// The model of the guide: a chat body in, the text of the answer out (`None`: no answer).
pub trait StepAsker: Send + Sync {
    fn ask(&self, body: Value) -> BoxFut<Option<String>>;
    /// How long a step is expected to take, in seconds: the island shows "~N s" while the model thinks (the slow path).
    fn eta_sec(&self) -> Option<u32> {
        None
    }
}

/// What the island shows of a guide (`GuideStepEvent`): the goal, the number of the step, the text, whether it goes on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GuideEvent {
    pub goal: String,
    pub step: u32,
    pub text: String,
    pub active: bool,
    /// The model is thinking about this step: the estimate in seconds (`~N s` in the island).
    #[serde(rename = "etaSec", skip_serializing_if = "Option::is_none")]
    pub eta_sec: Option<u32>,
}

pub trait GuideSink: Send + Sync {
    fn event(&self, e: GuideEvent);
}

pub struct NoGuideSink;

impl GuideSink for NoGuideSink {
    fn event(&self, _: GuideEvent) {}
}

pub struct ScreenRun {
    screen: Arc<dyn Screen>,
    highlighter: Highlighter,
    guides: Guides,
    events: Arc<dyn GuideSink>,
    danger: Vec<String>,
    finder: Mutex<Option<Finder>>,
    snapshot: Mutex<Snapshot>,
    last: Mutex<Option<Element>>,
    ctl: Mutex<Option<mpsc::UnboundedSender<GuideCtl>>>,
    running: AtomicBool,
    recording: AtomicBool,
    poll: Duration,
    settle: Duration,
    quiet_look: Duration,
    safety_look: Duration,
}

/// T30.5: the guide wakes this often while it waits for the person (cheap: the highlight and the news of UI Automation);
/// it is also the least time between two readings of the window.
const POLL: Duration = Duration::from_millis(400);
/// How long a click is let settle.
const SETTLE: Duration = Duration::from_millis(250);
/// Without news from UI Automation the window is read this often.
const QUIET_LOOK: Duration = Duration::from_secs(1);
/// With news arriving it is read when there is some, and this often anyway (a change nobody announced).
const SAFETY_LOOK: Duration = Duration::from_secs(5);

impl ScreenRun {
    pub fn new(screen: Arc<dyn Screen>, highlighter: Highlighter, guides: Guides, events: Arc<dyn GuideSink>, danger: Vec<String>) -> ScreenRun {
        ScreenRun {
            screen,
            highlighter,
            guides,
            events,
            danger,
            finder: Mutex::new(None),
            snapshot: Mutex::new(Snapshot::default()),
            last: Mutex::new(None),
            ctl: Mutex::new(None),
            running: AtomicBool::new(false),
            recording: AtomicBool::new(false),
            poll: POLL,
            settle: SETTLE,
            quiet_look: QUIET_LOOK,
            safety_look: SAFETY_LOOK,
        }
    }

    /// Other timing (tests).
    pub fn with_timing(mut self, poll: Duration, settle: Duration) -> ScreenRun {
        (self.poll, self.settle, self.quiet_look, self.safety_look) = (poll, settle, poll, poll);
        self
    }

    /// Other intervals of the readings (tests).
    pub fn with_looks(mut self, quiet: Duration, safety: Duration) -> ScreenRun {
        (self.quiet_look, self.safety_look) = (quiet, safety);
        self
    }

    fn danger_words(&self) -> Vec<&str> {
        screenhelp::all_danger_words().iter().map(String::as_str).chain(self.danger.iter().map(String::as_str)).collect()
    }

    pub fn guides(&self) -> &Guides {
        &self.guides
    }

    fn show(&self, e: &Element, text: &str, step: Option<u32>) {
        *self.last.lock().unwrap() = Some(e.clone());
        self.highlighter.show(Highlight { rect: e.rect, text: text.to_string(), step, total: None }, self.screen.now_ms(), self.screen.foreground());
    }

    // ---- T27.3 --------------------------------------------------------------------------------------------------

    /// "Where is ...?": highlights the best element; `None` is "not that one": the next candidate of the same search.
    pub fn find(&self, query: Option<&str>) -> Result<String, String> {
        let id = match query {
            Some(q) => {
                let snap = self.screen.snapshot().ok_or("Cannot read this window")?;
                let finder = Finder::new(&snap, q);
                let id = finder.current();
                *self.snapshot.lock().unwrap() = snap;
                *self.finder.lock().unwrap() = Some(finder);
                id.ok_or("Did not find it on the screen")?
            }
            None => self.finder.lock().unwrap().as_mut().and_then(Finder::advance).ok_or("No other candidates")?,
        };
        let e = self.snapshot.lock().unwrap().get(id).cloned().ok_or("Did not find it on the screen")?;
        self.show(&e, &e.name, None);
        Ok(format!("Here: {}", e.name))
    }

    // ---- T27.4 --------------------------------------------------------------------------------------------------

    /// Presses, scrolls or closes a tab. "This" is the element highlighted last. A dangerous name is not pressed unless `confirmed`.
    pub fn act(&self, target: Option<&str>, action: UiAction, confirmed: bool) -> Result<String, String> {
        if action == UiAction::CloseTab {
            return self.screen.perform(Rect::default(), action);
        }
        let snap = self.screen.snapshot().ok_or("Cannot read this window")?;
        let last_id = self.last.lock().unwrap().as_ref().and_then(|l| RecipeStep::of(l).find(&snap).map(|e| e.id));
        if matches!(action, UiAction::ScrollDown | UiAction::ScrollUp) && target.is_none() {
            // Scrolling needs a place: the last highlight, or the middle of what the window shows.
            let at = last_id.and_then(|i| snap.get(i)).map(|e| e.rect).or_else(|| snap.elements.first().map(|e| e.rect)).ok_or("Nothing to scroll")?;
            return self.screen.perform(at, action);
        }
        let danger = self.danger_words();
        match screenhelp::plan_act(&snap, target, last_id, confirmed, &danger) {
            ActPlan::Do(id) => {
                let e = snap.get(id).cloned().ok_or("Not found on the screen")?;
                self.highlighter.clear();
                let done = if e.role == "text" && action == UiAction::Click { self.screen.click_text(e.rect) } else { self.screen.perform(e.rect, action) };
                *self.last.lock().unwrap() = None;
                done
            }
            ActPlan::NeedsConfirm { id, name } => {
                if let Some(e) = snap.get(id) {
                    self.show(e, &name, None);
                }
                Err(format!("Not pressed: \"{name}\" needs a yes first"))
            }
            ActPlan::NotFound => Err("Did not find it on the screen".into()),
        }
    }

    // ---- T27.5 --------------------------------------------------------------------------------------------------

    /// "Next", "back", "stop", "done" for the guide or the recording that is running.
    pub fn control(&self, kind: GuideCtl) -> Result<String, String> {
        match self.ctl.lock().unwrap().as_ref() {
            Some(tx) if tx.send(kind).is_ok() => Ok("OK".into()),
            _ => Err("No guide is running".into()),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed) || self.recording.load(Ordering::Relaxed)
    }

    /// The guide: at most 15 steps. A saved recipe of this goal in this program goes first and without the model; a step that is not in
    /// the window hands over to the model. Returns the closing words.
    pub async fn guide(&self, goal: &str, lang_name: &str, asker: &dyn StepAsker) -> Result<String, String> {
        if self.is_running() {
            return Err("A guide is already running".into());
        }
        self.running.store(true, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel();
        *self.ctl.lock().unwrap() = Some(tx);
        let out = self.guide_loop(goal, lang_name, asker, &mut rx).await;
        *self.ctl.lock().unwrap() = None;
        self.running.store(false, Ordering::Relaxed);
        self.highlighter.clear();
        self.events.event(GuideEvent { goal: goal.into(), step: 0, text: String::new(), active: false, eta_sec: None });
        out
    }

    async fn guide_loop(&self, goal: &str, lang_name: &str, asker: &dyn StepAsker, rx: &mut mpsc::UnboundedReceiver<GuideCtl>) -> Result<String, String> {
        let mut guide = Guide::new(goal);
        let process = self.screen.process();
        let recipe: Option<Recipe> = self.guides.find(goal, &process);
        let mut from_recipe = recipe.as_ref().map(|r| (r.id.clone(), r.steps.clone(), 0usize));
        while !guide.finished() {
            let snap = self.screen.snapshot().ok_or("Cannot read this window")?;
            if snap.elements.is_empty() {
                return Err("Cannot see any buttons in this window".into());
            }
            // 1. The recipe, step by step, each checked in the live window.
            let mut next: Option<(Element, String)> = None;
            if let Some((id, steps, at)) = from_recipe.as_mut() {
                match steps.get(*at).and_then(|s| s.find(&snap)) {
                    Some(e) => {
                        next = Some((e.clone(), e.name.clone()));
                        *at += 1;
                        if *at == steps.len() {
                            self.guides.used(id);
                        }
                    }
                    None if *at < steps.len() => from_recipe = None,
                    None => {}
                }
            }
            // 2. The model: only an id of this snapshot.
            if next.is_none() {
                let body = screenhelp::step_request_body(goal, &snap, lang_name, &guide.done_names());
                // The model takes seconds: the card shows "~N s" for as long as it thinks.
                self.events.event(GuideEvent { goal: goal.into(), step: guide.step_number() as u32 + 1, text: String::new(), active: true, eta_sec: asker.eta_sec() });
                let mut tries = 0;
                loop {
                    let Some(content) = asker.ask(body.clone()).await else { return Err("The assistant did not answer".into()) };
                    match screenhelp::parse_step(&content, &snap) {
                        Ok(Step::Element { id, say }) => {
                            let e = snap.get(id).cloned().ok_or("Bad step")?;
                            next = Some((e, say));
                            break;
                        }
                        Ok(Step::Done { say }) => return Ok(if say.is_empty() { "Done".into() } else { say }),
                        Ok(Step::NotHere { say }) => return Err(if say.is_empty() { "Open the program you need".into() } else { say }),
                        Err(StepError::OutsideSnapshot | StepError::Unreadable) => {
                            tries += 1;
                            if tries >= 2 {
                                return Err("The assistant could not choose a step".into());
                            }
                        }
                    }
                }
            }
            let (e, say) = next.ok_or("No step")?;
            if !guide.push(e.clone(), say.clone()) {
                break;
            }
            self.events.event(GuideEvent { goal: goal.into(), step: guide.step_number() as u32, text: say.clone(), active: true, eta_sec: None });
            self.show(&e, &say, Some(guide.step_number() as u32));
            // 3. Wait for the person: the highlight ends (a click on it, another window), the window changes, "next", "back", "stop".
            let mut last_action = self.screen.now_ms();
            let (news, mut changes) = mpsc::unbounded_channel();
            let _watch = self.screen.watch(news);
            // `heard`: this program announces its changes. `pending`: an announced one has not been looked at.
            let (mut heard, mut pending, mut looked) = (false, false, tokio::time::Instant::now());
            // The tick runs on however much news arrives.
            let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + self.poll, self.poll);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    c = rx.recv() => match c {
                        Some(GuideCtl::Stop) | None => { guide.stop(); break; }
                        Some(GuideCtl::Done) => return Ok("Done".into()),
                        Some(GuideCtl::Next) => break,
                        Some(GuideCtl::Back) => {
                            if let Some((pe, ptext)) = guide.back().cloned() {
                                self.events.event(GuideEvent { goal: goal.into(), step: guide.step_number() as u32, text: ptext.clone(), active: true, eta_sec: None });
                                self.show(&pe, &ptext, Some(guide.step_number() as u32));
                                last_action = self.screen.now_ms();
                            }
                        }
                    },
                    Some(()) = changes.recv() => {
                        // A burst of events is one piece of news.
                        while changes.try_recv().is_ok() {}
                        (heard, pending) = (true, true);
                    }
                    _ = tick.tick() => {
                        let now = self.screen.now_ms();
                        if self.highlighter.current().is_none() {
                            break;
                        }
                        let every = if heard { self.safety_look } else { self.quiet_look };
                        // A window read from its picture (T30.6) is not read again while waiting: the text of a game or
                        // a video differs every time. The click on the highlight or "next" ends the step.
                        if !snap.text && (pending || looked.elapsed() >= every) {
                            (pending, looked) = (false, tokio::time::Instant::now());
                            if self.screen.snapshot().is_some_and(|s| changed(&snap, &s)) {
                                break;
                            }
                        }
                        if screenhelp::needs_repeat(last_action, now) {
                            self.show(&e, &say, Some(guide.step_number() as u32));
                            last_action = now;
                        }
                    }
                }
            }
            if guide.is_stopped() {
                return Ok("Stopped".into());
            }
            // The window needs a moment to show what the click opened.
            tokio::time::sleep(self.settle).await;
        }
        Ok(if guide.is_stopped() { "Stopped".into() } else { "That is as far as I can guide".into() })
    }

    // ---- T27.6: recording ---------------------------------------------------------------------------------------

    /// "Remember how to <goal>": the clicks of the person become the steps until "done"; then the recipe is saved.
    pub async fn record(&self, goal: &str) -> Result<String, String> {
        if self.is_running() {
            return Err("A guide is already running".into());
        }
        self.recording.store(true, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel();
        *self.ctl.lock().unwrap() = Some(tx);
        let process = self.screen.process();
        let mut steps: Vec<RecipeStep> = Vec::new();
        let result = loop {
            tokio::select! {
                c = rx.recv() => match c {
                    Some(GuideCtl::Done) => break self.guides.save(goal, &process, steps.clone(), "now").map(|_| format!("Remembered: {goal}")),
                    Some(GuideCtl::Stop) | None => break Err("Recording cancelled".to_string()),
                    _ => {}
                },
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if let Some((x, y)) = self.screen.click() {
                        if let Some(snap) = self.screen.snapshot() {
                            // The smallest element under the click.
                            let hit = snap.elements.iter().filter(|e| x >= e.rect.x && x < e.rect.x + e.rect.width && y >= e.rect.y && y < e.rect.y + e.rect.height).min_by_key(|e| e.rect.width * e.rect.height);
                            if let Some(e) = hit {
                                if steps.len() < super::guides::MAX_RECIPE_STEPS {
                                    steps.push(RecipeStep::of(e));
                                }
                            }
                        }
                    }
                }
            }
        };
        *self.ctl.lock().unwrap() = None;
        self.recording.store(false, Ordering::Relaxed);
        result
    }
}

/// Whether the window changed: the elements and their names, not their places. A reading that was cut (a big window and
/// the deadline) ends at another element every time: only what both readings reached is compared.
fn changed(before: &Snapshot, after: &Snapshot) -> bool {
    let same = |a: &Element, b: &Element| (&a.role, &a.name, &a.automation_id) == (&b.role, &b.name, &b.automation_id);
    let whole = !before.truncated && !after.truncated;
    (whole && before.elements.len() != after.elements.len()) || after.elements.is_empty() || before.elements.iter().zip(&after.elements).any(|(a, b)| !same(a, b))
}
/// The real windows.
pub struct WinScreen {
    started: std::time::Instant,
    was_down: AtomicBool,
}

impl WinScreen {
    pub fn new() -> WinScreen {
        WinScreen { started: std::time::Instant::now(), was_down: AtomicBool::new(false) }
    }
}

impl Default for WinScreen {
    fn default() -> Self {
        WinScreen::new()
    }
}

impl Screen for WinScreen {
    fn snapshot(&self) -> Option<Snapshot> {
        let found = super::ui_snapshot::active_window();
        // T30.6: a window that says almost nothing about itself is read from its picture.
        if found.as_ref().is_none_or(|s| s.elements.len() < super::ocr::FEW) {
            if let Some(read) = super::ocr::active_window(found.clone().unwrap_or_default()) {
                return Some(read);
            }
        }
        found
    }
    fn click_text(&self, rect: Rect) -> Result<String, String> {
        super::ui_snapshot::click_centre(rect)
    }
    fn process(&self) -> String {
        crate::collectors::focus::foreground_app().unwrap_or_default().to_lowercase()
    }
    fn foreground(&self) -> u64 {
        super::winops::foreground_handle()
    }
    fn perform(&self, rect: Rect, action: UiAction) -> Result<String, String> {
        super::ui_snapshot::perform(rect, action)
    }
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn click(&self) -> Option<(i32, i32)> {
        let (down, at) = super::winops::left_button_and_cursor();
        let was = self.was_down.swap(down, Ordering::Relaxed);
        (down && !was).then_some(at)
    }
    fn watch(&self, events: mpsc::UnboundedSender<()>) -> Option<Box<dyn Send>> {
        super::uia_events::watch(None, events).map(|w| Box::new(w) as Box<dyn Send>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::highlight::Recorder;
    use std::sync::atomic::AtomicUsize;

    fn el(id: u32, role: &str, name: &str, x: i32) -> Element {
        Element { id, role: role.into(), name: name.into(), automation_id: String::new(), rect: Rect { x, y: 0, width: 10, height: 10 }, parent_name: String::new() }
    }

    struct Fake {
        shots: Vec<Snapshot>,
        at: AtomicUsize,
        performed: Mutex<Vec<(Rect, UiAction)>>,
        clicks: Mutex<Vec<(i32, i32)>>,
        clock: AtomicUsize,
        fore: AtomicUsize,
        /// Whether the window announces its changes, whom it tells, and how often it was read.
        announces: AtomicBool,
        news: Mutex<Option<mpsc::UnboundedSender<()>>>,
        looks: AtomicUsize,
    }

    impl Fake {
        /// The person did something: the window shows the next snapshot.
        fn advance(&self) {
            self.at.fetch_add(1, Ordering::Relaxed);
            if let Some(tx) = self.news.lock().unwrap().as_ref() {
                let _ = tx.send(());
            }
        }

        fn new(shots: Vec<Snapshot>) -> Arc<Fake> {
            Arc::new(Fake { shots, at: AtomicUsize::new(0), performed: Mutex::new(vec![]), clicks: Mutex::new(vec![]), clock: AtomicUsize::new(0), fore: AtomicUsize::new(1), announces: AtomicBool::new(false), news: Mutex::new(None), looks: AtomicUsize::new(0) })
        }
    }

    impl Screen for Fake {
        fn watch(&self, events: mpsc::UnboundedSender<()>) -> Option<Box<dyn Send>> {
            if !self.announces.load(Ordering::Relaxed) {
                return None;
            }
            *self.news.lock().unwrap() = Some(events);
            Some(Box::new(()))
        }
        fn snapshot(&self) -> Option<Snapshot> {
            self.looks.fetch_add(1, Ordering::Relaxed);
            let i = self.at.load(Ordering::Relaxed).min(self.shots.len() - 1);
            self.shots.get(i).cloned()
        }
        fn process(&self) -> String {
            "winword.exe".into()
        }
        fn foreground(&self) -> u64 {
            self.fore.load(Ordering::Relaxed) as u64
        }
        fn perform(&self, rect: Rect, action: UiAction) -> Result<String, String> {
            self.performed.lock().unwrap().push((rect, action));
            Ok("Pressed".into())
        }
        fn now_ms(&self) -> u64 {
            self.clock.fetch_add(1, Ordering::Relaxed) as u64
        }
        fn click(&self) -> Option<(i32, i32)> {
            let mut c = self.clicks.lock().unwrap();
            if c.is_empty() { None } else { Some(c.remove(0)) }
        }
    }

    struct Script(Mutex<Vec<&'static str>>, AtomicUsize);

    impl StepAsker for Script {
        fn eta_sec(&self) -> Option<u32> {
            Some(7)
        }

        fn ask(&self, _body: Value) -> BoxFut<Option<String>> {
            self.1.fetch_add(1, Ordering::Relaxed);
            let next = { let mut s = self.0.lock().unwrap(); if s.is_empty() { None } else { Some(s.remove(0).to_string()) } };
            Box::pin(async move { next })
        }
    }

    fn run(shots: Vec<Snapshot>) -> (ScreenRun, Arc<Fake>, Arc<Recorder>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(shots);
        let rec = Arc::new(Recorder::default());
        let sr = ScreenRun::new(fake.clone(), Highlighter::new(rec.clone()), Guides::new(dir.path()), Arc::new(NoGuideSink), vec![]).with_timing(Duration::from_millis(5), Duration::from_millis(5));
        (sr, fake, rec, dir)
    }

    fn snap(names: &[&str]) -> Snapshot {
        Snapshot { elements: names.iter().enumerate().map(|(i, n)| el(i as u32 + 1, "button", n, i as i32 * 20)).collect(), truncated: false, text: false }
    }

    #[test]
    fn where_is_highlights_the_best_and_not_that_one_goes_to_the_next() {
        let (sr, _, rec, _d) = run(vec![snap(&["Save", "Save as PDF", "Help"])]);
        assert_eq!(sr.find(Some("save")).unwrap(), "Here: Save");
        assert_eq!(sr.find(None).unwrap(), "Here: Save as PDF");
        assert!(sr.find(None).is_err(), "no third candidate");
        assert_eq!(rec.shown.lock().unwrap().len(), 2);
        assert_eq!(sr.find(Some("zebra")).unwrap_err(), "Did not find it on the screen");
    }

    #[test]
    fn a_dangerous_element_is_never_pressed_without_the_yes() {
        let (sr, fake, rec, _d) = run(vec![snap(&["Cancel", "Delete all"])]);
        let err = sr.act(Some("delete all"), UiAction::Click, false).unwrap_err();
        assert!(err.contains("needs a yes"), "{err}");
        assert!(fake.performed.lock().unwrap().is_empty(), "nothing was pressed");
        assert_eq!(rec.shown.lock().unwrap().len(), 1, "but it is shown");
        assert!(sr.act(Some("delete all"), UiAction::Click, true).is_ok());
        assert_eq!(fake.performed.lock().unwrap().len(), 1);
    }

    #[test]
    fn this_presses_what_was_highlighted_last_and_a_safe_name_needs_no_yes() {
        let (sr, fake, _, _d) = run(vec![snap(&["Cancel", "Save"])]);
        assert!(sr.act(None, UiAction::Click, false).is_err(), "nothing highlighted yet");
        sr.find(Some("save")).unwrap();
        assert!(sr.act(None, UiAction::Click, false).is_ok());
        assert_eq!(fake.performed.lock().unwrap()[0].1, UiAction::Click);
        assert!(sr.act(Some("cancel"), UiAction::Click, false).is_ok());
    }

    #[test]
    fn closing_a_tab_and_scrolling_need_no_element() {
        let (sr, fake, _, _d) = run(vec![snap(&["Save"])]);
        assert!(sr.act(None, UiAction::CloseTab, false).is_ok());
        assert!(sr.act(None, UiAction::ScrollDown, false).is_ok());
        assert_eq!(fake.performed.lock().unwrap().iter().map(|p| p.1).collect::<Vec<_>>(), vec![UiAction::CloseTab, UiAction::ScrollDown]);
    }

    #[tokio::test]
    async fn a_guide_goes_three_steps_and_ends_on_done() {
        let shots = vec![snap(&["File", "Home"]), snap(&["Save As", "Open"]), snap(&["PDF", "DOCX"]), snap(&["Finished"])];
        let (sr, fake, rec, _d) = run(shots);
        let asker = Script(Mutex::new(vec![r#"{"next":1,"say":"Open File"}"#, r#"{"next":1,"say":"Choose Save As"}"#, r#"{"next":1,"say":"Pick PDF"}"#, r#"{"next":"done","say":"Saved"}"#]), AtomicUsize::new(0));
        let sr = Arc::new(sr);
        let runner = sr.clone();
        let handle = tokio::spawn(async move { runner.guide("save as pdf", "English", &asker).await });
        // The person does each step: "next" three times.
        for _ in 0..3 {
            tokio::time::sleep(Duration::from_millis(80)).await;
            fake.advance();
        }
        assert_eq!(handle.await.unwrap().unwrap(), "Saved");
        let texts: Vec<String> = rec.shown.lock().unwrap().iter().map(|h| h.text.clone()).collect();
        assert_eq!(texts, vec!["Open File", "Choose Save As", "Pick PDF"]);
        assert_eq!(rec.shown.lock().unwrap().iter().map(|h| h.step).collect::<Vec<_>>(), vec![Some(1), Some(2), Some(3)]);
        assert!(!sr.is_running());
    }

    #[tokio::test]
    async fn not_here_asks_to_open_the_program_and_stop_ends_the_guide() {
        let (sr, _, _, _d) = run(vec![snap(&["Start"])]);
        let asker = Script(Mutex::new(vec![r#"{"next":"not_here","say":"Open Word"}"#]), AtomicUsize::new(0));
        assert_eq!(sr.guide("save as pdf", "English", &asker).await.unwrap_err(), "Open Word");
        let (sr, _, _, _d) = run(vec![snap(&["File"])]);
        let sr = Arc::new(sr);
        let runner = sr.clone();
        let asker = Script(Mutex::new(vec![r#"{"next":1,"say":"Open File"}"#]), AtomicUsize::new(0));
        let handle = tokio::spawn(async move { runner.guide("x", "English", &asker).await });
        tokio::time::sleep(Duration::from_millis(80)).await;
        sr.control(GuideCtl::Stop).unwrap();
        assert_eq!(handle.await.unwrap().unwrap(), "Stopped");
        assert!(sr.control(GuideCtl::Next).is_err(), "no guide any more");
    }

    #[tokio::test]
    async fn an_id_the_model_invents_is_refused_twice_then_the_guide_gives_up() {
        let (sr, _, rec, _d) = run(vec![snap(&["File"])]);
        let asker = Script(Mutex::new(vec![r#"{"next":42,"say":"Press 42"}"#, r#"{"next":7,"say":"Press 7"}"#]), AtomicUsize::new(0));
        let err = sr.guide("x", "English", &asker).await.unwrap_err();
        assert_eq!(err, "The assistant could not choose a step");
        assert!(rec.shown.lock().unwrap().is_empty(), "nothing was highlighted for an invented element");
    }

    #[tokio::test]
    async fn a_saved_recipe_is_followed_without_the_model() {
        let (sr, fake, rec, _d) = run(vec![snap(&["File"]), snap(&["Save As"]), snap(&["Save As"])]);
        sr.guides().save("save as pdf", "winword.exe", vec![RecipeStep { role: "button".into(), name: "File".into(), automation_id: String::new() }, RecipeStep { role: "button".into(), name: "Save As".into(), automation_id: String::new() }], "1").unwrap();
        let asker = Script(Mutex::new(vec![r#"{"next":"done","say":"Done"}"#]), AtomicUsize::new(0));
        let sr = Arc::new(sr);
        let runner = sr.clone();
        let asker = Arc::new(asker);
        let a2 = asker.clone();
        let handle = tokio::spawn(async move { runner.guide("save as PDF", "English", &*a2).await });
        for _ in 0..2 {
            tokio::time::sleep(Duration::from_millis(80)).await;
            fake.advance();
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
        sr.control(GuideCtl::Stop).unwrap();
        let _ = handle.await;
        assert_eq!(rec.shown.lock().unwrap().iter().map(|h| h.text.clone()).collect::<Vec<_>>(), vec!["File", "Save As"]);
        assert_eq!(sr.guides().list()[0].uses, 1);
    }

    /// T30.5: with news of the window the next step comes at once and the window is not read while nothing happens;
    /// without news it is read at the quiet interval.
    #[tokio::test]
    async fn the_guide_looks_when_the_window_says_it_changed_and_rarely_otherwise() {
        let asker = || Script(Mutex::new(vec![r#"{"next":1,"say":"Open File"}"#, r#"{"next":"done","say":"Saved"}"#]), AtomicUsize::new(0));
        let make = |announces: bool, quiet: u64| {
            let (sr, fake, _, dir) = run(vec![snap(&["File"]), snap(&["Finished"])]);
            fake.announces.store(announces, Ordering::Relaxed);
            let sr = Arc::new(sr.with_looks(Duration::from_millis(quiet), Duration::from_secs(60)));
            (sr, fake, dir)
        };
        // Announced: the quiet interval is a minute and the step still follows at once.
        let (sr, fake, _d) = make(true, 60_000);
        let (runner, script) = (sr.clone(), asker());
        let handle = tokio::spawn(async move { runner.guide("x", "English", &script).await });
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(fake.looks.load(Ordering::Relaxed), 1, "nothing happened: the window was read once, for the step");
        fake.advance();
        assert_eq!(tokio::time::timeout(Duration::from_secs(3), handle).await.expect("at once").unwrap().unwrap(), "Saved");
        // Silent: the change is found by the quiet look.
        let (sr, fake, _d) = make(false, 150);
        let (runner, script) = (sr.clone(), asker());
        let handle = tokio::spawn(async move { runner.guide("x", "English", &script).await });
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(fake.looks.load(Ordering::Relaxed), 1);
        fake.advance();
        assert_eq!(tokio::time::timeout(Duration::from_secs(3), handle).await.expect("by the quiet look").unwrap().unwrap(), "Saved");
    }

    #[test]
    fn a_cut_reading_is_compared_only_as_far_as_both_reached() {
        let cut = |names: &[&str]| Snapshot { truncated: true, ..snap(names) };
        assert!(!changed(&cut(&["A", "B", "C"]), &cut(&["A", "B"])), "the same window, read less far");
        assert!(changed(&cut(&["A", "B"]), &cut(&["A", "X"])));
        assert!(changed(&snap(&["A", "B"]), &snap(&["A"])));
        assert!(changed(&cut(&["A"]), &cut(&[])), "nothing left");
        assert!(!changed(&snap(&["A"]), &snap(&["A"])));
    }

    #[derive(Default)]
    struct Events(Mutex<Vec<GuideEvent>>);

    impl GuideSink for Events {
        fn event(&self, e: GuideEvent) {
            self.0.lock().unwrap().push(e);
        }
    }

    #[tokio::test]
    async fn while_the_model_thinks_the_card_carries_the_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let events = Arc::new(Events::default());
        let sr = ScreenRun::new(Fake::new(vec![snap(&["Start"])]), Highlighter::new(Arc::new(Recorder::default())), Guides::new(dir.path()), events.clone(), vec![]).with_timing(Duration::from_millis(5), Duration::from_millis(5));
        let asker = Script(Mutex::new(vec![r#"{"next":"not_here","say":"Open Word"}"#]), AtomicUsize::new(0));
        let _ = sr.guide("x", "English", &asker).await;
        let seen = events.0.lock().unwrap();
        assert_eq!(seen[0].eta_sec, Some(7), "the first event is the wait for the model");
        assert!(seen[0].active && seen.last().is_some_and(|e| !e.active), "and the card is closed at the end");
    }

    #[tokio::test]
    async fn recording_turns_clicks_into_a_recipe_on_done() {
        let (sr, fake, _, _d) = run(vec![snap(&["File", "Save As"])]);
        fake.clicks.lock().unwrap().extend([(5, 5), (25, 5)]);
        let sr = Arc::new(sr);
        let runner = sr.clone();
        let handle = tokio::spawn(async move { runner.record("save as pdf").await });
        tokio::time::sleep(Duration::from_millis(400)).await;
        sr.control(GuideCtl::Done).unwrap();
        assert_eq!(handle.await.unwrap().unwrap(), "Remembered: save as pdf");
        let r = sr.guides().find("save as pdf", "winword.exe").unwrap();
        assert_eq!(r.steps.iter().map(|s| s.name.clone()).collect::<Vec<_>>(), vec!["File", "Save As"]);
    }
}
