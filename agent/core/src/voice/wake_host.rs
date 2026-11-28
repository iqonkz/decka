//! T32.1: runs the wake word. Downloads its files when the user turns it on, starts and stops the worker process as the
//! conditions say (so a pause really closes the microphone) and passes a detection on. The rules are in `wake`.

use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use serde::Serialize;

use super::wake::{self, Conditions, Cooldown, Files, Line, Sensitivity, WakeSettings, WakeState};
use super::wake_engine::WorkerArgs;
use crate::ai::download::{extract_archive, Downloader, ManifestEntry};
use crate::ai::supervisor::{on_crash, CrashDecision, Job};

/// How often a worker whose microphone keeps failing is started again.
const RETRY_MICROPHONE: std::time::Duration = std::time::Duration::from_secs(60);

/// What the settings window and the tray are told (event `wake-status`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeStatus {
    pub enabled: bool,
    pub sensitivity: Sensitivity,
    pub installed: bool,
    pub state: WakeState,
    pub reason: Option<String>,
    /// Of the download, while it runs.
    pub percent: Option<u8>,
    /// Detections since the start of the agent: the indicator of "Say Decka".
    pub heard: u32,
    pub testing: bool,
    /// Bytes the download takes.
    pub size: u64,
    /// It listens, but the microphone gave no sound at all in the last report (a noise gate, a muted headset).
    pub silent: bool,
}

/// What the host knows now.
#[derive(Debug, Clone, Default)]
pub struct Surroundings {
    pub voice_on: bool,
    pub recording_note: bool,
    pub fullscreen: bool,
    pub checking: bool,
    pub mic: Option<String>,
}

pub struct WakeDeps {
    pub config_dir: PathBuf,
    /// The folder of the AI package.
    pub root: PathBuf,
    pub manifest: Vec<ManifestEntry>,
    pub downloader: Downloader,
    /// The program that is started as the worker (the agent itself).
    pub exe: PathBuf,
}

type Listener = Arc<dyn Fn(&WakeStatus) + Send + Sync>;
type OnWake = Arc<dyn Fn() + Send + Sync>;

struct Worker {
    child: Child,
    /// Closes with the agent and takes the worker along.
    _job: Option<Job>,
    args: WorkerArgs,
    id: u64,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct Live {
    worker: Option<Worker>,
    next_id: u64,
    ready: bool,
    paused: bool,
    reason: Option<String>,
    percent: Option<u8>,
    heard: u32,
    test_until: Option<Instant>,
    crashes: Vec<u64>,
    /// No more restarts until the settings change.
    gave_up: bool,
    restart_at: Option<Instant>,
    cooldown: Cooldown,
    cancel: Option<Arc<AtomicBool>>,
    sent: Option<WakeStatus>,
    /// No sound in the last report of the worker.
    silent: bool,
    /// When `voice.log` last heard of a word that is on.
    noted: Option<Instant>,
}

struct Inner {
    handle: tokio::runtime::Handle,
    deps: WakeDeps,
    started: Instant,
    settings: Mutex<WakeSettings>,
    live: Mutex<Live>,
    listener: Mutex<Option<Listener>>,
    on_wake: Mutex<Option<OnWake>>,
}

#[derive(Clone)]
pub struct Wake {
    inner: Arc<Inner>,
}

impl Wake {
    pub fn new(handle: tokio::runtime::Handle, deps: WakeDeps) -> Wake {
        let settings = WakeSettings::load(&deps.config_dir);
        Wake { inner: Arc::new(Inner { handle, deps, started: Instant::now(), settings: Mutex::new(settings), live: Mutex::default(), listener: Mutex::new(None), on_wake: Mutex::new(None) }) }
    }

    pub fn listen(&self, listener: Listener) {
        *self.inner.listener.lock().unwrap() = Some(listener);
        self.inner.live.lock().unwrap().sent = None;
        self.inner.publish();
    }

    pub fn on_wake(&self, f: OnWake) {
        *self.inner.on_wake.lock().unwrap() = Some(f);
    }

    pub fn settings(&self) -> WakeSettings {
        *self.inner.settings.lock().unwrap()
    }

    pub fn status(&self) -> WakeStatus {
        self.inner.status()
    }

    /// The switch and the sensitivity of the settings. Turning it on downloads the files if they are not there.
    pub fn set(&self, enabled: bool, sensitivity: Sensitivity) -> Result<WakeStatus, String> {
        let next = WakeSettings { enabled, sensitivity };
        let files = self.inner.files();
        if enabled && files.is_none() {
            return Err("The wake word files are not in the package list".into());
        }
        next.save(&self.inner.deps.config_dir)?;
        *self.inner.settings.lock().unwrap() = next;
        {
            let mut live = self.inner.live.lock().unwrap();
            live.reason = None;
            live.gave_up = false;
            live.crashes.clear();
            live.restart_at = None;
            if !enabled {
                live.worker = None;
                live.ready = false;
                live.test_until = None;
                if let Some(cancel) = live.cancel.take() {
                    cancel.store(true, Ordering::SeqCst);
                }
            }
        }
        if enabled && files.is_some_and(|f| !f.installed()) {
            self.download();
        }
        self.inner.publish();
        Ok(self.status())
    }

    /// "Say Decka": for a short while a detection is only counted, it starts nothing.
    pub fn test(&self, on: bool) -> WakeStatus {
        self.inner.live.lock().unwrap().test_until = on.then(|| Instant::now() + wake::TEST_FOR);
        self.inner.publish();
        self.status()
    }

    fn download(&self) {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut live = self.inner.live.lock().unwrap();
            if live.cancel.is_some() {
                return;
            }
            live.cancel = Some(cancel.clone());
            live.percent = Some(0);
        }
        let weak = Arc::downgrade(&self.inner);
        self.inner.handle.spawn(async move {
            let result = fetch_all(&weak, &cancel).await;
            let Some(inner) = weak.upgrade() else { return };
            {
                let mut live = inner.live.lock().unwrap();
                live.percent = None;
                live.cancel = None;
                if let Err(e) = result {
                    if !cancel.load(Ordering::SeqCst) {
                        tracing::warn!("wake word download: {e}");
                        live.reason = Some(e);
                    }
                }
            }
            inner.publish();
        });
    }

    /// Called twice a second: starts, restarts or stops the worker.
    pub fn tick(&self, around: Surroundings) {
        let inner = &self.inner;
        let settings = self.settings();
        let files = inner.files();
        let conditions = Conditions {
            enabled: settings.enabled,
            voice_on: around.voice_on,
            installed: files.as_ref().is_some_and(Files::installed),
            recording_note: around.recording_note,
            fullscreen: around.fullscreen,
            checking: around.checking,
        };
        let wanted = wake::wanted(conditions);
        {
            let mut live = inner.live.lock().unwrap();
            live.paused = wanted == WakeState::Paused;
            if wanted != WakeState::Starting {
                live.worker = None;
                live.ready = false;
            } else if let Some(files) = files {
                match files.library() {
                    Some(library) => {
                        let args = WorkerArgs { library, model: files.model, sensitivity: settings.sensitivity, mic: around.mic };
                        if live.worker.as_ref().is_some_and(|w| w.args != args) {
                            live.worker = None;
                        }
                        let waiting = live.restart_at.is_some_and(|t| Instant::now() < t);
                        if live.worker.is_none() && !live.gave_up && !waiting {
                            live.ready = false;
                            live.next_id += 1;
                            let id = live.next_id;
                            match inner.start(args, id) {
                                Ok(worker) => live.worker = Some(worker),
                                Err(e) => {
                                    tracing::warn!("wake word: {e}");
                                    live.reason = Some(e);
                                    live.gave_up = true;
                                }
                            }
                        }
                    }
                    None => live.reason = Some("The wake word library is missing".into()),
                }
            }
        }
        inner.publish();
        if settings.enabled {
            inner.note_not_listening();
        }
    }

    /// The agent is leaving.
    pub fn stop(&self) {
        let mut live = self.inner.live.lock().unwrap();
        live.worker = None;
        live.ready = false;
    }
}

async fn fetch_all(weak: &Weak<Inner>, cancel: &AtomicBool) -> Result<(), String> {
    let (files, entries, downloader) = {
        let inner = weak.upgrade().ok_or("stopped")?;
        let files = inner.files().ok_or("The wake word files are not in the package list")?;
        let entries: Vec<ManifestEntry> = wake::IDS.iter().filter_map(|id| inner.deps.manifest.iter().find(|e| e.id == *id).cloned()).collect();
        (files, entries, inner.deps.downloader.clone())
    };
    let total: u64 = entries.iter().map(|e| e.size).sum::<u64>().max(1);
    let mut before = 0u64;
    for (i, entry) in entries.iter().enumerate() {
        let dest = if i == 0 { files.archive.clone() } else { files.model.join(wake::MODEL_FILES[i - 1]) };
        if !(i == 0 && files.runtime.join(wake::RUNTIME_DONE).is_file()) {
            let mut progress = |done: u64| {
                let percent = (((before + done) * 100) / total).min(99) as u8;
                if let Some(inner) = weak.upgrade() {
                    let changed = inner.live.lock().unwrap().percent.replace(percent) != Some(percent);
                    if changed {
                        inner.publish();
                    }
                }
            };
            downloader.fetch(entry, &dest, cancel, &mut progress).await.map_err(|e| e.to_string())?;
            if i == 0 {
                let (archive, runtime) = (files.archive.clone(), files.runtime.clone());
                tokio::task::spawn_blocking(move || -> Result<(), String> {
                    extract_archive(&archive, &runtime).map_err(|e| e.to_string())?;
                    std::fs::write(runtime.join(wake::RUNTIME_DONE), b"").map_err(|e| e.to_string())?;
                    // The archive is not needed once unpacked; a leftover only takes disk space.
                    if let Err(e) = std::fs::remove_file(&archive) {
                        tracing::warn!("wake word archive stays on the disk: {e}");
                    }
                    Ok(())
                })
                .await
                .map_err(|e| e.to_string())??;
            }
        }
        before += entry.size;
    }
    Ok(())
}

impl Inner {
    fn files(&self) -> Option<Files> {
        Files::of(&self.deps.root, &self.deps.manifest)
    }

    fn status(&self) -> WakeStatus {
        let settings = *self.settings.lock().unwrap();
        let installed = self.files().is_some_and(|f| f.installed());
        let size = wake::IDS.iter().filter_map(|id| self.deps.manifest.iter().find(|e| e.id == *id)).map(|e| e.size).sum();
        let live = self.live.lock().unwrap();
        let state = if !settings.enabled {
            WakeState::Off
        } else if live.percent.is_some() {
            WakeState::Downloading
        } else if live.reason.is_some() && live.worker.is_none() {
            WakeState::Error
        } else if live.paused {
            WakeState::Paused
        } else if live.worker.is_some() {
            if live.ready { WakeState::Listening } else { WakeState::Starting }
        } else {
            WakeState::Off
        };
        WakeStatus {
            enabled: settings.enabled,
            sensitivity: settings.sensitivity,
            installed,
            state,
            reason: live.reason.clone(),
            percent: live.percent,
            heard: live.heard,
            testing: live.test_until.is_some_and(|t| Instant::now() < t),
            size,
            silent: state == WakeState::Listening && live.silent,
        }
    }

    /// A word that is on but does not listen says so in `voice.log` (a listening worker reports by itself).
    fn note_not_listening(&self) {
        let status = self.status();
        if status.state == WakeState::Listening {
            return;
        }
        {
            let mut live = self.live.lock().unwrap();
            if live.noted.is_some_and(|t| t.elapsed() < wake::REPORT_EVERY) {
                return;
            }
            live.noted = Some(Instant::now());
        }
        let reason = status.reason.map(|r| format!(" reason={r}")).unwrap_or_default();
        super::mic::log_line(&format!("wake not listening: state={:?}{reason}", status.state));
    }

    /// Tells the listener, only when something changed.
    fn publish(&self) {
        let status = self.status();
        {
            let mut live = self.live.lock().unwrap();
            if live.sent.as_ref() == Some(&status) {
                return;
            }
            live.sent = Some(status.clone());
        }
        let listener = self.listener.lock().unwrap().clone();
        if let Some(l) = listener {
            l(&status);
        }
    }

    fn start(self: &Arc<Self>, args: WorkerArgs, id: u64) -> Result<Worker, String> {
        let mut command = Command::new(&self.deps.exe);
        command.args(args.to_args()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|e| format!("The wake word did not start: {e}"))?;
        #[cfg(windows)]
        let job = {
            use std::os::windows::io::AsRawHandle;
            Job::new().and_then(|job| job.assign(child.as_raw_handle()).map(|()| job)).map_err(|e| tracing::warn!("wake word job object: {e}")).ok()
        };
        #[cfg(not(windows))]
        let job: Option<Job> = None;
        let out = child.stdout.take().ok_or("The wake word did not start: no output")?;
        let weak = Arc::downgrade(self);
        std::thread::Builder::new()
            .name("decka-wake".into())
            .spawn(move || {
                for line in std::io::BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    let (Some(inner), Some(line)) = (weak.upgrade(), wake::parse_line(&line)) else { continue };
                    inner.line(id, line);
                }
                if let Some(inner) = weak.upgrade() {
                    inner.ended(id);
                }
            })
            .map_err(|e| format!("The wake word did not start: {e}"))?;
        Ok(Worker { child, _job: job, args, id })
    }

    fn line(&self, id: u64, line: Line) {
        let wake_now = {
            let mut live = self.live.lock().unwrap();
            if live.worker.as_ref().is_none_or(|w| w.id != id) {
                return;
            }
            match line {
                Line::Ready => {
                    live.ready = true;
                    live.silent = false;
                    live.reason = None;
                    false
                }
                Line::Level(m) => {
                    live.silent = m.no_signal();
                    super::mic::log_line(&format!("wake alive: chunks={} peak={:.4} silent={} model={} hits={}", m.chunks, m.peak, m.silent, m.model, m.hits));
                    false
                }
                Line::Failed(e) => {
                    tracing::warn!("wake word worker: {e}");
                    live.reason = Some(e);
                    false
                }
                Line::Wake => {
                    let now = Instant::now();
                    let new = live.cooldown.accept(now);
                    if new {
                        live.heard += 1;
                    }
                    new && !live.test_until.is_some_and(|t| now < t)
                }
            }
        };
        self.publish();
        if wake_now {
            let on_wake = self.on_wake.lock().unwrap().clone();
            if let Some(f) = on_wake {
                f();
            }
        }
    }

    /// The worker ended without being asked to.
    fn ended(&self, id: u64) {
        {
            let mut live = self.live.lock().unwrap();
            if live.worker.as_ref().is_none_or(|w| w.id != id) {
                return;
            }
            live.worker = None;
            live.ready = false;
            let now_ms = self.started.elapsed().as_millis() as u64;
            match on_crash(&mut live.crashes, now_ms) {
                CrashDecision::Restart { after_ms } => live.restart_at = Some(Instant::now() + std::time::Duration::from_millis(after_ms)),
                CrashDecision::GiveUp => {
                    // T34.1: a microphone that is gone for a while must not end the wake word until the next restart.
                    if live.reason.as_deref().is_some_and(wake::microphone_trouble) {
                        live.restart_at = Some(Instant::now() + RETRY_MICROPHONE);
                    } else {
                        live.gave_up = true;
                        live.reason.get_or_insert_with(|| "The wake word stopped unexpectedly".into());
                    }
                }
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    fn wake_in(dir: &std::path::Path, install: bool) -> Wake {
        let manifest = crate::ai::download::parse_manifest(include_str!("../../../../shared/ai/manifest.json")).unwrap();
        let files = Files::of(dir, &manifest).unwrap();
        if install {
            std::fs::create_dir_all(files.runtime.join("lib")).unwrap();
            std::fs::create_dir_all(&files.model).unwrap();
            std::fs::write(files.runtime.join(wake::RUNTIME_DONE), b"").unwrap();
            std::fs::write(files.runtime.join("lib").join("sherpa-onnx-c-api.dll"), b"x").unwrap();
            for f in wake::MODEL_FILES {
                std::fs::write(files.model.join(f), b"x").unwrap();
            }
        }
        let deps = WakeDeps { config_dir: dir.into(), root: dir.into(), manifest, downloader: Downloader::with_hosts(&[]), exe: dir.join("no-such-program.exe") };
        Wake::new(tokio::runtime::Handle::current(), deps)
    }

    #[tokio::test]
    async fn off_by_default_and_a_worker_that_cannot_start_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let wake = wake_in(dir.path(), true);
        let around = Surroundings { voice_on: true, ..Default::default() };
        wake.tick(around.clone());
        assert_eq!(wake.status().state, WakeState::Off);
        assert!(wake.status().installed && wake.status().size > 30_000_000);
        wake.set(true, Sensitivity::Medium).unwrap();
        wake.tick(Surroundings { fullscreen: true, ..around.clone() });
        assert_eq!(wake.status().state, WakeState::Paused);
        wake.tick(around);
        let s = wake.status();
        assert_eq!(s.state, WakeState::Error);
        assert!(s.reason.unwrap().contains("did not start"));
        assert_eq!(wake.set(false, Sensitivity::Low).unwrap().state, WakeState::Off);
    }

    #[tokio::test]
    async fn a_detection_starts_a_request_but_not_during_the_test_and_not_twice() {
        let dir = tempfile::tempdir().unwrap();
        let wake = wake_in(dir.path(), true);
        let started = Arc::new(AtomicUsize::new(0));
        let count = started.clone();
        wake.on_wake(Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }));
        // A stand-in worker that waits on its input.
        let child = Command::new("cmd.exe").args(["/c", "pause"]).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
        let args = WorkerArgs { library: "x".into(), model: "y".into(), sensitivity: Sensitivity::Medium, mic: None };
        wake.inner.live.lock().unwrap().worker = Some(Worker { child, _job: None, args, id: 7 });
        wake.test(true);
        wake.inner.line(7, Line::Wake);
        assert_eq!((wake.status().heard, started.load(Ordering::SeqCst)), (1, 0));
        wake.test(false);
        wake.inner.live.lock().unwrap().cooldown = Cooldown::default();
        wake.inner.line(7, Line::Wake);
        wake.inner.line(7, Line::Wake);
        assert_eq!((wake.status().heard, started.load(Ordering::SeqCst)), (2, 1));
        // A line of an older worker is ignored.
        wake.inner.line(6, Line::Wake);
        assert_eq!(wake.status().heard, 2);
        wake.stop();
    }

    /// Lets the stand-in worker fail with `reason` four times in a row.
    fn fail_four_times(wake: &Wake, reason: &str) {
        for id in 1..=4 {
            let child = Command::new("cmd.exe").args(["/c", "pause"]).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
            let args = WorkerArgs { library: "x".into(), model: "y".into(), sensitivity: Sensitivity::Medium, mic: None };
            wake.inner.live.lock().unwrap().worker = Some(Worker { child, _job: None, args, id });
            wake.inner.line(id, Line::Failed(reason.into()));
            wake.inner.ended(id);
        }
    }

    #[tokio::test]
    async fn a_failing_microphone_is_tried_again_and_a_failing_model_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let wake = wake_in(dir.path(), true);
        wake.set(true, Sensitivity::Medium).unwrap();
        fail_four_times(&wake, "Microphone stopped: the device is gone");
        {
            let live = wake.inner.live.lock().unwrap();
            assert!(!live.gave_up && live.restart_at.is_some_and(|t| t > Instant::now() + RETRY_MICROPHONE / 2));
        }
        let s = wake.status();
        assert_eq!((s.state, s.reason.as_deref()), (WakeState::Error, Some("Microphone stopped: the device is gone")));
        wake.set(true, Sensitivity::Medium).unwrap();
        fail_four_times(&wake, "the wake word model did not load");
        assert!(wake.inner.live.lock().unwrap().gave_up);
    }

    #[tokio::test]
    async fn a_listening_worker_that_gets_no_sound_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let wake = wake_in(dir.path(), true);
        wake.set(true, Sensitivity::Medium).unwrap();
        let child = Command::new("cmd.exe").args(["/c", "pause"]).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
        let args = WorkerArgs { library: "x".into(), model: "y".into(), sensitivity: Sensitivity::Medium, mic: None };
        wake.inner.live.lock().unwrap().worker = Some(Worker { child, _job: None, args, id: 1 });
        wake.inner.line(1, Line::Ready);
        wake.inner.line(1, Line::Level(wake::Meter { chunks: 50, silent: 50, ..Default::default() }));
        assert!(wake.status().silent);
        wake.inner.line(1, Line::Level(wake::Meter { chunks: 50, silent: 2, peak: 0.1, ..Default::default() }));
        assert!(!wake.status().silent && wake.status().state == WakeState::Listening);
        wake.stop();
    }

    #[tokio::test]
    async fn without_files_turning_it_on_starts_the_download_and_turning_off_cancels_it() {
        let dir = tempfile::tempdir().unwrap();
        let wake = wake_in(dir.path(), false);
        let s = wake.set(true, Sensitivity::Medium).unwrap();
        assert!(!s.installed);
        assert_eq!(s.state, WakeState::Downloading);
        assert_eq!(wake.set(false, Sensitivity::Medium).unwrap().state, WakeState::Off);
    }
}
