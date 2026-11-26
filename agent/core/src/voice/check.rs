//! `Check voice` (TASKS.md T28.3): the chain step by step — microphone, recognition, model, what would run. The recognition
//! is checked without the model. A dry run: `CheckPorts` has no way to execute anything.

use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;

use super::matcher::{Match, Table};
use super::pipeline::{failure_msg, key, msg, understood, BoxFut, ConfirmInfo};
use super::tools::{validate, Context, RawCall, Resolved};
use crate::ai::failure::Reason;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepId {
    Mic,
    Stt,
    Model,
    Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepState {
    Pending,
    Running,
    Ok,
    Error,
}

/// One row of the panel (`voice_self_check` and the event `voice-check`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckStep {
    pub id: StepId,
    pub state: StepState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// The result: the level, the recognised text, the tool, what would be done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// The English text of a failure; `error_key` translates it when it is a fixed phrase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_key: Option<&'static str>,
    /// What the action step understood, for the window to translate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub understood: Option<ConfirmInfo>,
    /// The action would wait for a "yes".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm: Option<bool>,
    /// The microphone is open for the phrase of the recognition step: speak now.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub listening: bool,
}

impl CheckStep {
    fn pending(id: StepId) -> CheckStep {
        CheckStep { id, state: StepState::Pending, duration_ms: None, detail: None, reason: None, error: None, error_key: None, understood: None, confirm: None, listening: false }
    }
}

/// A short recording of the microphone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recording {
    /// 16 kHz mono PCM16.
    pub pcm: Vec<u8>,
    /// The loudest smoothed level, 0..1.
    pub level: f32,
    /// Some sound came in (not digital silence).
    pub sound: bool,
    /// The end-of-speech decision heard speech.
    pub speech: bool,
}

/// A step that failed.
#[derive(Debug, Clone, PartialEq)]
pub struct StepError {
    pub reason: Option<Reason>,
    pub error: String,
    pub error_key: Option<&'static str>,
}

impl StepError {
    pub fn reason(reason: Reason) -> StepError {
        let error_key = match reason {
            Reason::Mic => key::MIC_UNAVAILABLE,
            Reason::Stt => key::FAIL_STT,
            Reason::Vram => key::FAIL_VRAM,
            Reason::Ram => key::FAIL_RAM,
            Reason::ModelFile => key::FAIL_MODEL_FILE,
            Reason::Port => key::FAIL_PORT,
            Reason::Crash => key::FAIL_CRASH,
        };
        StepError { reason: Some(reason), error: failure_msg(reason).into(), error_key: Some(error_key) }
    }

    pub fn text(error: impl Into<String>, error_key: Option<&'static str>) -> StepError {
        StepError { reason: None, error: error.into(), error_key }
    }
}

/// What the check needs from the PC. Deliberately no way to execute a call.
pub trait CheckPorts: Send + Sync {
    fn env(&self) -> (Arc<Table>, Context);
    /// The level of the microphone for 3 s.
    fn record_level(&self) -> BoxFut<Result<Recording, StepError>>;
    /// Starts the recognition (without the model).
    fn start_stt(&self) -> BoxFut<Result<(), StepError>>;
    /// One phrase: ends at the pause after speech, or after 10 s.
    fn record_phrase(&self) -> BoxFut<Result<Recording, StepError>>;
    fn transcribe(&self, pcm: Vec<u8>) -> BoxFut<Result<String, StepError>>;
    /// `/health` of the model, then one request: the tool it chose for `phrase`.
    fn ask_model(&self, phrase: String, ctx: Context) -> BoxFut<Result<RawCall, StepError>>;
    /// Every change of the steps, for the window.
    fn progress(&self, steps: &[CheckStep]);
}

/// The test phrase "what time is it" in the language of the request.
pub fn time_phrase(lang: &str) -> &'static str {
    match lang {
        "ru" => "который час",
        "zh" => "现在几点",
        "hi" => "अभी कितने बजे हैं",
        "es" => "qué hora es",
        "ar" => "كم الساعة",
        "fr" => "quelle heure est-il",
        "pt" => "que horas são",
        "de" => "wie spät ist es",
        "ja" => "今何時ですか",
        "kk" => "сағат неше",
        _ => "what time is it",
    }
}

struct Run<'a> {
    ports: &'a dyn CheckPorts,
    steps: Vec<CheckStep>,
}

impl Run<'_> {
    fn step(&mut self, id: StepId) -> &mut CheckStep {
        self.steps.iter_mut().find(|s| s.id == id).expect("the four steps")
    }

    fn begin(&mut self, id: StepId) -> Instant {
        self.step(id).state = StepState::Running;
        self.ports.progress(&self.steps);
        Instant::now()
    }

    fn end(&mut self, id: StepId, since: Instant, result: Result<String, StepError>) {
        let step = self.step(id);
        step.duration_ms = Some(since.elapsed().as_millis() as u64);
        step.listening = false;
        match result {
            Ok(detail) => {
                step.state = StepState::Ok;
                step.detail = Some(detail).filter(|d| !d.is_empty());
            }
            Err(e) => {
                step.state = StepState::Error;
                (step.reason, step.error, step.error_key) = (e.reason, Some(e.error), e.error_key);
            }
        }
        self.ports.progress(&self.steps);
    }
}

async fn microphone(ports: &dyn CheckPorts) -> Result<String, StepError> {
    let r = ports.record_level().await?;
    if r.sound {
        Ok(format!("{} %", (r.level.clamp(0.0, 1.0) * 100.0).round() as u32))
    } else {
        Err(StepError { reason: Some(Reason::Mic), error: msg::NO_SOUND.into(), error_key: Some(key::NO_SOUND) })
    }
}

async fn recognition(run: &mut Run<'_>) -> Result<String, StepError> {
    run.ports.start_stt().await?;
    run.step(StepId::Stt).listening = true;
    run.ports.progress(&run.steps);
    let r = run.ports.record_phrase().await?;
    run.step(StepId::Stt).listening = false;
    run.ports.progress(&run.steps);
    if !r.speech {
        return Err(StepError::text(msg::NO_SPEECH, Some(key::NO_SPEECH)));
    }
    let text = run.ports.transcribe(r.pcm).await?;
    if text.trim().is_empty() {
        Err(StepError::text(msg::DIDNT_CATCH, Some(key::DIDNT_CATCH)))
    } else {
        Ok(text.trim().to_string())
    }
}

/// What the phrase would do: the tables first, the model when they do not know it. Returns the step's text and the call.
async fn dry_run(ports: &dyn CheckPorts, phrase: &str, model_error: Option<StepError>) -> Result<(ConfirmInfo, bool), StepError> {
    let (table, ctx) = ports.env();
    let raw = match table.matches(phrase) {
        Match::Hit(i) => RawCall { tool: i.tool, args: serde_json::Value::Object(i.args) },
        Match::Ambiguous(list) => {
            let tools: Vec<String> = list.iter().take(3).map(|i| i.tool.replace('_', " ")).collect();
            return Err(StepError::text(format!("Which one: {}?", tools.join(" or ")), None));
        }
        Match::None => match model_error {
            Some(e) => return Err(e),
            None => ports.ask_model(phrase.to_string(), ctx.clone()).await?,
        },
    };
    match validate(&raw, &ctx) {
        Resolved::Ready(v) => Ok((understood(&v), v.confirm)),
        Resolved::Ask { question, .. } => Err(StepError::text(question, None)),
        Resolved::Reject(why) => Err(StepError::text(format!("Can't do that: {why}"), None)),
    }
}

/// The whole chain. A failed microphone leaves the recognition unchecked; the model is checked in any case; the action
/// uses the recognised phrase, or the test phrase when there is none.
pub async fn run(ports: &dyn CheckPorts, lang: &str) -> Vec<CheckStep> {
    let mut run = Run { ports, steps: [StepId::Mic, StepId::Stt, StepId::Model, StepId::Action].map(CheckStep::pending).to_vec() };

    let since = run.begin(StepId::Mic);
    let mic = microphone(ports).await;
    let mic_ok = mic.is_ok();
    run.end(StepId::Mic, since, mic);

    let mut heard = None;
    if mic_ok {
        let since = run.begin(StepId::Stt);
        let result = recognition(&mut run).await;
        heard = result.as_ref().ok().cloned();
        run.end(StepId::Stt, since, result);
    }

    let since = run.begin(StepId::Model);
    let (_, ctx) = ports.env();
    let model = ports.ask_model(time_phrase(lang).to_string(), ctx).await;
    let model_error = model.as_ref().err().cloned();
    run.end(StepId::Model, since, model.map(|call| call.tool));

    let since = run.begin(StepId::Action);
    let phrase = heard.unwrap_or_else(|| time_phrase(lang).to_string());
    let action = dry_run(ports, &phrase, model_error).await;
    let result = action.map(|(info, confirm)| {
        let text = info.text.clone();
        let step = run.step(StepId::Action);
        (step.understood, step.confirm) = (Some(info), Some(confirm));
        text
    });
    run.end(StepId::Action, since, result);
    run.steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::tools::{ActionRef, AppRef};
    use std::sync::Mutex;

    struct Fake {
        level: Result<Recording, StepError>,
        stt: Result<(), StepError>,
        phrase: &'static str,
        model: Result<RawCall, StepError>,
        asked: Mutex<Vec<String>>,
        progress: Mutex<Vec<Vec<(StepState, bool)>>>,
    }

    fn sound() -> Recording {
        Recording { pcm: vec![0; 3200], level: 0.42, sound: true, speech: true }
    }

    fn fake() -> Fake {
        Fake { level: Ok(sound()), stt: Ok(()), phrase: "volume up", model: Ok(RawCall { tool: "status".into(), args: serde_json::json!({ "topic": "time" }) }), asked: Mutex::default(), progress: Mutex::default() }
    }

    impl CheckPorts for Fake {
        fn env(&self) -> (Arc<Table>, Context) {
            let ctx = Context { actions: vec![ActionRef { id: "lock".into(), name: "Lock".into() }], apps: vec![AppRef { name: "Calculator".into(), program: "calc.exe".into() }], ..Default::default() };
            (Arc::new(Table::parse(include_str!("../../../../shared/voice/en.json")).unwrap()), ctx)
        }
        fn record_level(&self) -> BoxFut<Result<Recording, StepError>> {
            let r = self.level.clone();
            Box::pin(async move { r })
        }
        fn start_stt(&self) -> BoxFut<Result<(), StepError>> {
            let r = self.stt.clone();
            Box::pin(async move { r })
        }
        fn record_phrase(&self) -> BoxFut<Result<Recording, StepError>> {
            Box::pin(async { Ok(sound()) })
        }
        fn transcribe(&self, _: Vec<u8>) -> BoxFut<Result<String, StepError>> {
            let text = self.phrase.to_string();
            Box::pin(async move { Ok(text) })
        }
        fn ask_model(&self, phrase: String, _: Context) -> BoxFut<Result<RawCall, StepError>> {
            self.asked.lock().unwrap().push(phrase);
            let r = self.model.clone();
            Box::pin(async move { r })
        }
        fn progress(&self, steps: &[CheckStep]) {
            self.progress.lock().unwrap().push(steps.iter().map(|s| (s.state, s.listening)).collect());
        }
    }

    fn states(steps: &[CheckStep]) -> Vec<StepState> {
        steps.iter().map(|s| s.state).collect()
    }

    #[tokio::test]
    async fn the_whole_chain_reports_each_step_with_its_time() {
        let f = fake();
        let steps = run(&f, "en").await;
        assert_eq!(states(&steps), [StepState::Ok; 4]);
        assert!(steps.iter().all(|s| s.duration_ms.is_some()));
        assert_eq!(steps[0].detail.as_deref(), Some("42 %"));
        assert_eq!(steps[1].detail.as_deref(), Some("volume up"));
        assert_eq!(steps[2].detail.as_deref(), Some("status"));
        assert_eq!((steps[3].understood.as_ref().and_then(|u| u.key), steps[3].confirm), (Some("voice.did.volume"), Some(false)));
        assert_eq!(f.asked.lock().unwrap().as_slice(), ["what time is it"], "a phrase of the tables does not go to the model again");
        let progress = f.progress.lock().unwrap();
        assert!(progress.iter().any(|p| p[1] == (StepState::Running, true)), "the window learns when to speak");
        assert_eq!(progress.first().unwrap()[0].0, StepState::Running);
    }

    #[tokio::test]
    async fn the_recognition_is_checked_without_the_model() {
        let f = Fake { model: Err(StepError::reason(Reason::Vram)), ..fake() };
        let steps = run(&f, "en").await;
        assert_eq!(states(&steps), [StepState::Ok, StepState::Ok, StepState::Error, StepState::Ok]);
        assert_eq!((steps[2].reason, steps[2].error_key), (Some(Reason::Vram), Some("m28.err_vram_short")));
        // A phrase only the model knows: the action step says why the model cannot answer.
        let f = Fake { phrase: "something the tables do not know", model: Err(StepError::reason(Reason::Ram)), ..fake() };
        let steps = run(&f, "en").await;
        assert_eq!((steps[1].state, steps[3].state, steps[3].reason), (StepState::Ok, StepState::Error, Some(Reason::Ram)));
    }

    #[tokio::test]
    async fn a_silent_microphone_skips_the_recognition_and_a_dangerous_phrase_would_ask() {
        let f = Fake { level: Ok(Recording { sound: false, ..sound() }), ..fake() };
        let steps = run(&f, "en").await;
        assert_eq!(states(&steps)[..3], [StepState::Error, StepState::Pending, StepState::Ok]);
        assert_eq!((steps[0].reason, steps[0].error_key), (Some(Reason::Mic), Some("voice.reply.no_sound")));
        let f = Fake { phrase: "lock computer", ..fake() };
        let steps = run(&f, "en").await;
        assert_eq!((steps[3].state, steps[3].confirm), (StepState::Ok, Some(true)));
        let f = Fake { stt: Err(StepError::reason(Reason::Stt)), ..fake() };
        let steps = run(&f, "en").await;
        assert_eq!((steps[1].reason, steps[2].state), (Some(Reason::Stt), StepState::Ok));
    }

    #[test]
    fn every_language_has_its_time_phrase() {
        let all: Vec<&str> = ["en", "ru", "zh", "hi", "es", "ar", "fr", "pt", "de", "ja", "kk"].iter().map(|l| time_phrase(l)).collect();
        assert_eq!(all.iter().collect::<std::collections::HashSet<_>>().len(), 11);
    }
}
