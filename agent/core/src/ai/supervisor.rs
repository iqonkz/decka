//! Supervisor of the two local servers behind voice control (DATA_MODEL.md 12.3): `llama-server` and `whisper-server`
//! as hidden child processes inside a Job Object (they die with the agent), on free 127.0.0.1 ports, `llama-server`
//! behind a random key per launch. Lazy start, unload after idle, restart with back-off, stop after repeated crashes.
//!
//! The decisions (`on_crash`, `idle_expired`, argument lists) are pure functions; Windows and process calls are thin.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{watch, Mutex as AsyncMutex};

pub const RESTART_DELAYS_MS: [u64; 3] = [2_000, 5_000, 15_000];
pub const CRASH_WINDOW_MS: u64 = 10 * 60 * 1000;
/// More crashes than this inside `CRASH_WINDOW_MS` stop the servers until a manual retry.
pub const MAX_CRASHES: usize = 3;
pub const IDLE_MS: u64 = 10 * 60 * 1000;
pub const READY_TIMEOUT: Duration = Duration::from_secs(60);
const IDLE_CHECK: Duration = Duration::from_secs(30);
const MAX_LOG_LINES: usize = 50;

#[derive(Debug, Clone, PartialEq)]
pub enum CrashDecision {
    Restart { after_ms: u64 },
    GiveUp,
}

/// Records a crash at `now_ms` and decides: restart after 2, 5, 15 s, or give up on the 4th crash within 10 minutes.
pub fn on_crash(history: &mut Vec<u64>, now_ms: u64) -> CrashDecision {
    history.retain(|t| now_ms.saturating_sub(*t) < CRASH_WINDOW_MS);
    history.push(now_ms);
    if history.len() > MAX_CRASHES {
        CrashDecision::GiveUp
    } else {
        CrashDecision::Restart { after_ms: RESTART_DELAYS_MS[history.len() - 1] }
    }
}

/// True when the servers should be unloaded: running, not pinned by `Keep the model loaded`, idle for 10 minutes.
pub fn idle_expired(last_used_ms: u64, now_ms: u64, keep_loaded: bool, running: bool) -> bool {
    running && !keep_loaded && now_ms.saturating_sub(last_used_ms) >= IDLE_MS
}

pub fn llama_args(model: &Path, port: u16, key: &str) -> Vec<String> {
    let mut a: Vec<String> = ["-m"].iter().map(|s| s.to_string()).collect();
    a.push(model.display().to_string());
    for s in ["-c", "4096", "-ngl", "99", "--jinja", "--no-webui", "--host", "127.0.0.1", "--port"] {
        a.push(s.to_string());
    }
    a.push(port.to_string());
    a.push("--api-key".into());
    a.push(key.into());
    a
}

pub fn whisper_args(model: &Path, port: u16, cores: usize) -> Vec<String> {
    vec![
        "-m".into(),
        model.display().to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "-t".into(),
        cores.saturating_sub(1).max(1).to_string(),
    ]
}

/// A free loopback port (bind to 0, read it, release it).
pub fn free_port() -> std::io::Result<u16> {
    Ok(std::net::TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

/// A random key for one launch of `llama-server` (64 hex characters).
pub fn new_key() -> String {
    crate::config::random_bytes(32).iter().map(|b| format!("{b:02x}")).collect()
}

// ---- Job Object -------------------------------------------------------------------------------------------------

#[cfg(windows)]
pub use job::Job;

#[cfg(windows)]
mod job {
    use std::ffi::c_void;
    use std::os::windows::io::RawHandle;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// A Job Object with `KILL_ON_JOB_CLOSE`: every process assigned to it dies when this value (or the agent) goes away.
    pub struct Job(HANDLE);

    // SAFETY: a job handle is only passed to thread-safe kernel calls.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> std::io::Result<Job> {
            // SAFETY: plain Win32 calls with valid arguments; the handle is closed in `Drop`.
            unsafe {
                let handle = CreateJobObjectW(None, PCWSTR::null()).map_err(|e| std::io::Error::other(e.to_string()))?;
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let set = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if let Err(e) = set {
                    let _ = CloseHandle(handle);
                    return Err(std::io::Error::other(e.to_string()));
                }
                Ok(Job(handle))
            }
        }

        pub fn assign(&self, process: RawHandle) -> std::io::Result<()> {
            // SAFETY: `process` is a live process handle owned by the caller.
            unsafe { AssignProcessToJobObject(self.0, HANDLE(process)).map_err(|e| std::io::Error::other(e.to_string())) }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle came from `CreateJobObjectW` and is closed once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(not(windows))]
pub struct Job;

#[cfg(not(windows))]
impl Job {
    pub fn new() -> std::io::Result<Job> {
        Ok(Job)
    }
}

// ---- Supervisor -------------------------------------------------------------------------------------------------

/// What `ensure_started` hands to the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoints {
    pub llama_port: u16,
    pub llama_key: String,
    pub whisper_port: u16,
}

/// What the settings window shows: `Loading` -> `loading`, `Ready` -> `ready`, `Stopped` -> `error`.
#[derive(Debug, Clone, PartialEq)]
pub enum SupState {
    Idle,
    Loading,
    Ready,
    /// Gave up after repeated crashes or could not start; holds the last stderr line.
    Stopped(String),
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub llama_exe: PathBuf,
    pub llama_model: PathBuf,
    pub whisper_exe: PathBuf,
    pub whisper_model: PathBuf,
    pub keep_loaded: bool,
}

#[derive(Default)]
struct Tail {
    last: String,
    logged: usize,
}

struct Spec {
    name: &'static str,
    exe: PathBuf,
    args: Vec<String>,
    port: u16,
    health: &'static str,
}

struct Live {
    endpoints: Endpoints,
    /// Dropping the sender (or sending) tells the monitors to kill their child and exit.
    stop: watch::Sender<bool>,
}

struct Inner {
    job: Job,
    cfg: Mutex<Option<SupervisorConfig>>,
    live: AsyncMutex<Option<Live>>,
    state: watch::Sender<SupState>,
    crashes: Mutex<Vec<u64>>,
    t0: Instant,
    last_used_ms: Mutex<u64>,
}

#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<Inner>,
    /// Background work runs on this handle, never on a bare `tokio::spawn` (commands may run outside the runtime).
    handle: tokio::runtime::Handle,
}

impl Supervisor {
    pub fn new(handle: tokio::runtime::Handle) -> std::io::Result<Supervisor> {
        Ok(Supervisor {
            inner: Arc::new(Inner {
                job: Job::new()?,
                cfg: Mutex::new(None),
                live: AsyncMutex::new(None),
                state: watch::channel(SupState::Idle).0,
                crashes: Mutex::new(Vec::new()),
                t0: Instant::now(),
                last_used_ms: Mutex::new(0),
            }),
            handle,
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<SupState> {
        self.inner.state.subscribe()
    }

    pub fn state(&self) -> SupState {
        self.inner.state.borrow().clone()
    }

    /// Sets the files to run. A changed config takes effect at the next start.
    pub fn configure(&self, cfg: Option<SupervisorConfig>) {
        *self.inner.cfg.lock().unwrap() = cfg;
    }

    pub fn set_keep_loaded(&self, keep: bool) {
        if let Some(c) = self.inner.cfg.lock().unwrap().as_mut() {
            c.keep_loaded = keep;
        }
    }

    fn now_ms(&self) -> u64 {
        self.inner.t0.elapsed().as_millis() as u64
    }

    /// Marks a voice request: restarts the idle clock.
    pub fn touch(&self) {
        *self.inner.last_used_ms.lock().unwrap() = self.now_ms();
    }

    /// Lazy start: returns the endpoints, starting both servers on the first call and waiting until they are healthy.
    pub async fn ensure_started(&self) -> Result<Endpoints, String> {
        let mut live = self.inner.live.lock().await;
        self.touch();
        if let Some(l) = live.as_ref() {
            if let SupState::Stopped(msg) = self.state() {
                return Err(msg);
            }
            return Ok(l.endpoints.clone());
        }
        if let SupState::Stopped(msg) = self.state() {
            return Err(msg); // needs a manual `retry`
        }
        let cfg = self.inner.cfg.lock().unwrap().clone().ok_or_else(|| "Voice control is not installed".to_string())?;
        self.inner.state.send_replace(SupState::Loading);

        let endpoints = Endpoints {
            llama_port: free_port().map_err(|e| e.to_string())?,
            llama_key: new_key(),
            whisper_port: free_port().map_err(|e| e.to_string())?,
        };
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
        let llama = Spec {
            name: "llama-server",
            exe: cfg.llama_exe.clone(),
            args: llama_args(&cfg.llama_model, endpoints.llama_port, &endpoints.llama_key),
            port: endpoints.llama_port,
            health: "/health",
        };
        let whisper = Spec {
            name: "whisper-server",
            exe: cfg.whisper_exe.clone(),
            args: whisper_args(&cfg.whisper_model, endpoints.whisper_port, cores),
            port: endpoints.whisper_port,
            health: "/",
        };

        let (llama_run, whisper_run) = tokio::join!(self.start_server(&llama), self.start_server(&whisper));
        let (llama_run, whisper_run) = match (llama_run, whisper_run) {
            (Ok(a), Ok(b)) => (a, b),
            (a, b) => {
                // One failed: kill the other (dropping it kills the child) and report the first error.
                let msg = a.err().or(b.err()).unwrap_or_default();
                self.inner.state.send_replace(SupState::Stopped(msg.clone()));
                return Err(msg);
            }
        };

        let (stop_tx, stop_rx) = watch::channel(false);
        self.spawn_monitor(llama, llama_run, stop_rx.clone());
        self.spawn_monitor(whisper, whisper_run, stop_rx.clone());
        self.spawn_idle_watch(stop_rx);
        *live = Some(Live { endpoints: endpoints.clone(), stop: stop_tx });
        self.inner.state.send_replace(SupState::Ready);
        Ok(endpoints)
    }

    /// Unloads both servers (idle, `Remove`, shutdown). Does not clear a `Stopped` state.
    pub async fn stop(&self) {
        if let Some(live) = self.inner.live.lock().await.take() {
            let _ = live.stop.send(true);
        }
        if !matches!(self.state(), SupState::Stopped(_)) {
            self.inner.state.send_replace(SupState::Idle);
        }
    }

    /// `Retry` after the servers gave up: forgets the crash history; the next request starts them again.
    pub async fn retry(&self) {
        self.stop().await;
        self.inner.crashes.lock().unwrap().clear();
        self.inner.state.send_replace(SupState::Idle);
    }

    async fn start_server(&self, spec: &Spec) -> Result<Running, String> {
        let mut cmd = tokio::process::Command::new(&spec.exe);
        cmd.args(&spec.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", spec.name))?;
        #[cfg(windows)]
        if let Some(h) = child.raw_handle() {
            if let Err(e) = self.inner.job.assign(h) {
                tracing::warn!("{}: could not join the job object: {e}", spec.name);
            }
        }
        let tail = Arc::new(Mutex::new(Tail::default()));
        if let Some(stderr) = child.stderr.take() {
            let tail = tail.clone();
            let name = spec.name;
            self.handle.spawn(read_stderr(stderr, tail, name));
        }
        match wait_healthy(spec.port, spec.health, READY_TIMEOUT, &mut child).await {
            Ok(()) => Ok(Running { child, tail }),
            Err(why) => {
                let last = tail.lock().unwrap().last.clone();
                let _ = child.start_kill();
                Err(if last.is_empty() { format!("{}: {why}", spec.name) } else { last })
            }
        }
    }

    fn spawn_monitor(&self, spec: Spec, mut running: Running, mut stop: watch::Receiver<bool>) {
        let me = self.clone();
        self.handle.spawn(async move {
            loop {
                tokio::select! {
                    _ = running.child.wait() => {}
                    _ = stop.changed() => {
                        let _ = running.child.start_kill();
                        return;
                    }
                }
                // The child exited by itself: a crash.
                let last = running.tail.lock().unwrap().last.clone();
                let decision = on_crash(&mut me.inner.crashes.lock().unwrap(), me.now_ms());
                match decision {
                    CrashDecision::GiveUp => {
                        let msg = if last.is_empty() { format!("{} stopped", spec.name) } else { last };
                        me.inner.state.send_replace(SupState::Stopped(msg));
                        // Take the other server down too; `live` stays set so `ensure_started` reports the error.
                        if let Some(l) = me.inner.live.lock().await.as_ref() {
                            let _ = l.stop.send(true);
                        }
                        return;
                    }
                    CrashDecision::Restart { after_ms } => {
                        me.inner.state.send_replace(SupState::Loading);
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(after_ms)) => {}
                            _ = stop.changed() => return,
                        }
                        match me.start_server(&spec).await {
                            Ok(r) => {
                                running = r;
                                me.inner.state.send_replace(SupState::Ready);
                            }
                            Err(_) => continue_as_crash(&mut running),
                        }
                    }
                }
            }
        });
    }

    fn spawn_idle_watch(&self, mut stop: watch::Receiver<bool>) {
        let me = self.clone();
        self.handle.spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(IDLE_CHECK) => {}
                    _ = stop.changed() => return,
                }
                let keep = me.inner.cfg.lock().unwrap().as_ref().is_some_and(|c| c.keep_loaded);
                let last = *me.inner.last_used_ms.lock().unwrap();
                if idle_expired(last, me.now_ms(), keep, true) {
                    me.stop().await;
                    return;
                }
            }
        });
    }
}

struct Running {
    child: tokio::process::Child,
    tail: Arc<Mutex<Tail>>,
}

/// A failed restart counts as another crash of the same server: make the child "exit" at once so the loop decides again.
fn continue_as_crash(running: &mut Running) {
    let _ = running.child.start_kill();
}

async fn read_stderr(stderr: tokio::process::ChildStderr, tail: Arc<Mutex<Tail>>, name: &'static str) {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let mut t = tail.lock().unwrap();
        if t.logged < MAX_LOG_LINES {
            t.logged += 1;
            tracing::info!(target: "voice", "{name}: {line}");
        }
        t.last = line;
    }
}

/// Polls `GET /health` (llama) or `GET /` (whisper) until it answers 200, the child exits, or `timeout` passes.
async fn wait_healthy(port: u16, path: &str, timeout: Duration, child: &mut tokio::process::Child) -> Result<(), String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(3)).build().map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{port}{path}");
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("exited ({status})"));
        }
        if client.get(&url).send().await.is_ok_and(|r| r.status().is_success()) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("did not become ready in time".into());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

// ---- Speech recognition request ---------------------------------------------------------------------------------

/// A recognition of a 30 s phrase takes seconds on a GPU and under half a minute on a CPU.
pub const TRANSCRIBE_TIMEOUT: Duration = Duration::from_secs(15);

/// The `multipart/form-data` body of `POST /inference` (DATA_MODEL.md 12.4 item 2): `file` (WAV), the forced `language`,
/// `response_format=json`, `temperature=0`.
pub fn whisper_multipart(boundary: &str, wav: &[u8], language: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav.len() + 512);
    let field = |body: &mut Vec<u8>, name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    };
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"voice.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(b"\r\n");
    field(&mut body, "language", language);
    field(&mut body, "response_format", "json");
    field(&mut body, "temperature", "0");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// The text of a whisper answer (`{"text": " open calculator"}`). Silence comes back as an empty text or as a marker such as
/// `[BLANK_AUDIO]` / `(silence)`: both are an empty result.
pub fn parse_whisper_response(body: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "Speech recognition answered something unreadable".to_string())?;
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        return Err(format!("Speech recognition failed: {e}"));
    }
    let text = v.get("text").and_then(|t| t.as_str()).ok_or_else(|| "Speech recognition answered without text".to_string())?;
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let marker = (text.starts_with('[') && text.ends_with(']')) || (text.starts_with('(') && text.ends_with(')'));
    Ok(if marker { String::new() } else { text })
}

impl Supervisor {
    /// Recognises 16 kHz mono PCM16 with `whisper-server` (started lazily; the first call waits for the models).
    pub async fn transcribe(&self, pcm: &[u8], language: &str) -> Result<String, String> {
        let endpoints = self.ensure_started().await?;
        let samples: Vec<i16> = pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        let wav = crate::voice::audio::encode_wav_pcm16(&samples, 16_000);
        let boundary = format!("decka{}", new_key());
        let body = whisper_multipart(&boundary, &wav, language);
        let client = reqwest::Client::builder().timeout(TRANSCRIBE_TIMEOUT).build().map_err(|e| e.to_string())?;
        let url = format!("http://127.0.0.1:{}/inference", endpoints.whisper_port);
        let answer = client
            .post(url)
            .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Speech recognition did not answer: {e}"))?;
        if !answer.status().is_success() {
            return Err(format!("Speech recognition failed: HTTP {}", answer.status().as_u16()));
        }
        let text = answer.text().await.map_err(|e| e.to_string())?;
        self.touch();
        parse_whisper_response(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_multipart_body_has_the_file_and_the_fixed_fields_in_order() {
        let body = whisper_multipart("BND", b"RIFFdata", "ru");
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(text.starts_with("--BND\r\nContent-Disposition: form-data; name=\"file\"; filename=\"voice.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFFdata\r\n"));
        let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle} missing"));
        assert!(at("name=\"language\"\r\n\r\nru\r\n") < at("name=\"response_format\"\r\n\r\njson\r\n"));
        assert!(at("name=\"response_format\"") < at("name=\"temperature\"\r\n\r\n0\r\n"));
        assert!(text.ends_with("--BND--\r\n"));
        assert_eq!(text.matches("--BND\r\n").count(), 4);
    }

    #[test]
    fn a_whisper_answer_gives_clean_text_and_silence_gives_nothing() {
        assert_eq!(parse_whisper_response(r#"{"text":"  open   calculator\n"}"#).unwrap(), "open calculator");
        assert_eq!(parse_whisper_response(r#"{"text":" [BLANK_AUDIO]"}"#).unwrap(), "");
        assert_eq!(parse_whisper_response(r#"{"text":"(silence)"}"#).unwrap(), "");
        assert_eq!(parse_whisper_response(r#"{"text":""}"#).unwrap(), "");
        assert!(parse_whisper_response(r#"{"error":"bad audio"}"#).unwrap_err().contains("bad audio"));
        assert!(parse_whisper_response("<html>").is_err());
        assert!(parse_whisper_response(r#"{"other":1}"#).is_err());
    }

    #[test]
    fn crashes_restart_after_2_5_15_seconds_and_the_fourth_in_ten_minutes_gives_up() {
        let mut h = Vec::new();
        assert_eq!(on_crash(&mut h, 1_000), CrashDecision::Restart { after_ms: 2_000 });
        assert_eq!(on_crash(&mut h, 10_000), CrashDecision::Restart { after_ms: 5_000 });
        assert_eq!(on_crash(&mut h, 30_000), CrashDecision::Restart { after_ms: 15_000 });
        assert_eq!(on_crash(&mut h, 60_000), CrashDecision::GiveUp);
    }

    #[test]
    fn old_crashes_leave_the_window() {
        let mut h = Vec::new();
        for t in [0, 1_000, 2_000] {
            on_crash(&mut h, t);
        }
        // 10 minutes after the first three: they are forgotten, the back-off starts over.
        assert_eq!(on_crash(&mut h, 2_000 + CRASH_WINDOW_MS), CrashDecision::Restart { after_ms: 2_000 });
        assert_eq!(h.len(), 1);
        // Exactly at the window edge the first one still counts.
        let mut h = vec![0, 100, 200];
        assert_eq!(on_crash(&mut h, CRASH_WINDOW_MS - 1), CrashDecision::GiveUp);
    }

    #[test]
    fn idle_unload_needs_running_no_pin_and_ten_quiet_minutes() {
        assert!(idle_expired(0, IDLE_MS, false, true));
        assert!(!idle_expired(0, IDLE_MS - 1, false, true));
        assert!(!idle_expired(0, IDLE_MS * 5, true, true), "Keep the model loaded");
        assert!(!idle_expired(0, IDLE_MS * 5, false, false), "nothing to unload");
        assert!(!idle_expired(IDLE_MS, 0, false, true), "clock going backwards is not idle");
    }

    #[test]
    fn server_arguments_follow_section_12_3() {
        let a = llama_args(Path::new("m.gguf"), 5555, "k");
        assert_eq!(
            a.join(" "),
            "-m m.gguf -c 4096 -ngl 99 --jinja --no-webui --host 127.0.0.1 --port 5555 --api-key k"
        );
        assert_eq!(whisper_args(Path::new("w.bin"), 6000, 8).join(" "), "-m w.bin --host 127.0.0.1 --port 6000 -t 7");
        assert_eq!(whisper_args(Path::new("w.bin"), 6000, 1).last().unwrap(), "1", "at least one thread");
    }

    #[test]
    fn ports_are_free_and_keys_are_random_per_launch() {
        let p = free_port().unwrap();
        assert!(p > 0);
        std::net::TcpListener::bind(("127.0.0.1", p)).expect("the port was released");
        let (a, b) = (new_key(), new_key());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[cfg(windows)]
    #[test]
    fn a_process_in_the_job_dies_when_the_job_closes() {
        use std::os::windows::io::AsRawHandle;
        use std::os::windows::process::CommandExt;
        let mut child = std::process::Command::new("ping.exe")
            .args(["-n", "60", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .unwrap();
        let job = Job::new().unwrap();
        job.assign(child.as_raw_handle()).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        drop(job);
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "the child must be killed with the job");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[tokio::test]
    async fn without_files_the_start_is_refused_and_the_state_stays_idle() {
        let sup = Supervisor::new(tokio::runtime::Handle::current()).unwrap();
        assert!(sup.ensure_started().await.is_err());
        assert_eq!(sup.state(), SupState::Idle);
    }

    #[tokio::test]
    async fn a_missing_executable_stops_with_a_message_and_retry_clears_it() {
        let sup = Supervisor::new(tokio::runtime::Handle::current()).unwrap();
        sup.configure(Some(SupervisorConfig {
            llama_exe: PathBuf::from("Z:\\nope\\llama-server.exe"),
            llama_model: PathBuf::from("m"),
            whisper_exe: PathBuf::from("Z:\\nope\\whisper-server.exe"),
            whisper_model: PathBuf::from("w"),
            keep_loaded: false,
        }));
        let err = sup.ensure_started().await.unwrap_err();
        assert!(err.contains("server"), "{err}");
        assert!(matches!(sup.state(), SupState::Stopped(_)));
        assert!(sup.ensure_started().await.is_err(), "no new attempts before Retry");
        sup.retry().await;
        assert_eq!(sup.state(), SupState::Idle);
    }
}
