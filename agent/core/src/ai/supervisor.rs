//! Supervisor of the two local servers behind voice control (DATA_MODEL.md 12.3): `llama-server` and `whisper-server`
//! as hidden child processes inside a Job Object (they die with the agent), on free 127.0.0.1 ports, `llama-server`
//! behind a random key per launch. Lazy start, unload after idle, restart with back-off, stop after repeated crashes.
//! The two are independent (T28.3): the recognition works while the model cannot load.
//!
//! The decisions (`on_crash`, `idle_expired`, `with_cpu_fallback`, argument lists) are plain functions; Windows and process
//! calls are thin.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{watch, Mutex as AsyncMutex};

use super::failure::{after_cpu_retry, classify, stronger, Failure, Reason, Server};
use super::gpu::ALL_LAYERS;

pub const RESTART_DELAYS_MS: [u64; 3] = [2_000, 5_000, 15_000];
pub const CRASH_WINDOW_MS: u64 = 10 * 60 * 1000;
/// More crashes than this inside `CRASH_WINDOW_MS` stop the server until a manual retry.
pub const MAX_CRASHES: usize = 3;
pub const IDLE_MS: u64 = 10 * 60 * 1000;
pub const READY_TIMEOUT: Duration = Duration::from_secs(120);
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

/// True when the servers should be unloaded: running and idle for `after_ms` (0 = they are kept loaded).
pub fn idle_expired(last_used_ms: u64, now_ms: u64, after_ms: u64, running: bool) -> bool {
    running && after_ms > 0 && now_ms.saturating_sub(last_used_ms) >= after_ms
}

/// How long the start of a server probably takes, in seconds: the last one measured, else guessed from the size of its
/// model (about 300 MB a second from the disk and 3 s on top).
pub fn load_estimate_sec(last_ms: u64, model_bytes: u64) -> u64 {
    if last_ms > 0 {
        last_ms.div_ceil(1000)
    } else {
        model_bytes / 300_000_000 + 3
    }
}

/// `ngl` layers go to the video card (T28.1); with less than all of them the projector stays on the processor.
pub fn llama_args(model: &Path, mmproj: Option<&Path>, port: u16, key: &str, ngl: u32) -> Vec<String> {
    let mut a: Vec<String> = ["-m"].iter().map(|s| s.to_string()).collect();
    a.push(model.display().to_string());
    // The optional Screen help package (DATA_MODEL.md 13.3): the projector that lets the model read an image.
    if let Some(p) = mmproj {
        a.push("--mmproj".into());
        a.push(p.display().to_string());
        if ngl < ALL_LAYERS {
            a.push("--no-mmproj-offload".into());
        }
    }
    let ngl = ngl.to_string();
    // T29.A, memory: by default llama-server keeps up to 8 GB of old prompts in the RAM and 32 checkpoints per slot. No
    // prompt cache and 4 checkpoints are as fast for our few long prompts and take about half a gigabyte less (measured).
    for s in ["-c", "4096", "-ngl", ngl.as_str(), "--cache-ram", "0", "--ctx-checkpoints", "4", "--jinja", "--no-webui", "--host", "127.0.0.1", "--port"] {
        a.push(s.to_string());
    }
    a.push(port.to_string());
    a.push("--api-key".into());
    a.push(key.into());
    a
}

pub fn whisper_args(model: &Path, port: u16, threads: usize) -> Vec<String> {
    vec![
        "-m".into(),
        model.display().to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "-t".into(),
        threads.max(1).to_string(),
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
    /// The recognition runs on the video card (a longer first start, a short limit).
    pub whisper_gpu: bool,
}

/// What the settings window shows: `Loading` -> `loading`, `Ready` -> `ready`, `Stopped` -> `error`.
#[derive(Debug, Clone, PartialEq)]
pub enum SupState {
    Idle,
    Loading,
    Ready,
    /// A server gave up after repeated crashes or could not start. The other one may still work (the model's comes first).
    Stopped(Failure),
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub llama_exe: PathBuf,
    pub llama_model: PathBuf,
    /// The projector of the Screen help package, when it is installed.
    pub mmproj: Option<PathBuf>,
    /// Layers of the model on the video card (`-ngl`, `gpu::gpu_layers`).
    pub ngl: u32,
    pub whisper_exe: PathBuf,
    pub whisper_model: PathBuf,
    /// Quiet time after which both servers are unloaded (`voice.unloadAfterMin`); 0 = never.
    pub unload_after_ms: u64,
    /// whisper_exe is the GPU build.
    pub whisper_gpu: bool,
}

/// One attempt with `ngl` layers; if it fails and layers were on the card, one more entirely on the processor (T28.1).
/// Both attempts are logged. Returns what started and the layers it runs with.
pub async fn with_cpu_fallback<T, F, Fut>(ngl: u32, mut attempt: F) -> Result<(T, u32), Failure>
where
    F: FnMut(u32) -> Fut,
    Fut: std::future::Future<Output = Result<T, Failure>>,
{
    let first = match attempt(ngl).await {
        Ok(started) => return Ok((started, ngl)),
        Err(f) => f,
    };
    tracing::warn!(target: "voice", "model start with -ngl {ngl} failed ({:?}, exit {:?}): {}", first.reason, first.code, first.detail);
    if ngl == 0 {
        return Err(first);
    }
    match attempt(0).await {
        Ok(started) => {
            tracing::info!(target: "voice", "model start with -ngl 0 succeeded");
            Ok((started, 0))
        }
        Err(second) => {
            tracing::warn!(target: "voice", "model start with -ngl 0 failed ({:?}, exit {:?}): {}", second.reason, second.code, second.detail);
            Err(after_cpu_retry(&first, second))
        }
    }
}

#[derive(Default)]
struct Tail {
    last: String,
    logged: usize,
    /// The line that says the most about a failure.
    hint: Option<(Reason, String)>,
}

struct Spec {
    server: Server,
    exe: PathBuf,
    args: Vec<String>,
    port: u16,
    health: &'static str,
}

struct Live {
    port: u16,
    key: String,
    /// Dropping the sender (or sending) tells the monitor to kill its child and exit.
    stop: watch::Sender<bool>,
}

/// One of the two servers: it starts, fails and restarts on its own.
#[derive(Default)]
struct Slot {
    live: AsyncMutex<Option<Live>>,
    failure: Mutex<Option<Failure>>,
    crashes: Mutex<Vec<u64>>,
    starting: AtomicBool,
    up: AtomicBool,
    /// When the running start began, and how long the last successful one took (ms).
    since: Mutex<Option<Instant>>,
    last_load_ms: std::sync::atomic::AtomicU64,
}

/// `starting` of a slot, cleared also when the start is abandoned.
struct Starting<'a>(&'a AtomicBool);

impl<'a> Starting<'a> {
    fn new(flag: &'a AtomicBool) -> Starting<'a> {
        flag.store(true, Ordering::SeqCst);
        Starting(flag)
    }
}

impl Drop for Starting<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Told the model file that runs on the processor only because the card could not take it.
pub type CpuFallbackHook = Arc<dyn Fn(&Path) + Send + Sync>;

struct Inner {
    job: Job,
    cfg: Mutex<Option<SupervisorConfig>>,
    llm: Slot,
    stt: Slot,
    state: watch::Sender<SupState>,
    on_cpu_fallback: Mutex<Option<CpuFallbackHook>>,
    t0: Instant,
    last_used_ms: Mutex<u64>,
}

#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<Inner>,
    /// Background work runs on this handle, never on a bare `tokio::spawn` (commands may run outside the runtime).
    handle: tokio::runtime::Handle,
}

fn plain_failure(server: Server, detail: impl Into<String>) -> Failure {
    Failure { server, reason: if server == Server::Llm { Reason::Crash } else { Reason::Stt }, code: None, detail: detail.into() }
}

const NOT_INSTALLED: &str = "Voice control is not installed";

impl Supervisor {
    pub fn new(handle: tokio::runtime::Handle) -> std::io::Result<Supervisor> {
        let sup = Supervisor {
            inner: Arc::new(Inner {
                job: Job::new()?,
                cfg: Mutex::new(None),
                llm: Slot::default(),
                stt: Slot::default(),
                state: watch::channel(SupState::Idle).0,
                on_cpu_fallback: Mutex::new(None),
                t0: Instant::now(),
                last_used_ms: Mutex::new(0),
            }),
            handle,
        };
        sup.spawn_idle_watch();
        Ok(sup)
    }

    pub fn subscribe(&self) -> watch::Receiver<SupState> {
        self.inner.state.subscribe()
    }

    pub fn state(&self) -> SupState {
        self.inner.state.borrow().clone()
    }

    fn slot(&self, server: Server) -> &Slot {
        match server {
            Server::Llm => &self.inner.llm,
            Server::Stt => &self.inner.stt,
        }
    }

    /// Why this server is down until `retry`; `None` while it works or has not been asked for.
    pub fn failure_of(&self, server: Server) -> Option<Failure> {
        self.slot(server).failure.lock().unwrap().clone()
    }

    /// The server answers requests now.
    pub fn is_up(&self, server: Server) -> bool {
        self.slot(server).up.load(Ordering::SeqCst)
    }

    fn compute_state(&self) -> SupState {
        let slots = [&self.inner.llm, &self.inner.stt];
        if slots.iter().any(|s| s.starting.load(Ordering::SeqCst)) {
            SupState::Loading
        } else if let Some(f) = slots.iter().find_map(|s| s.failure.lock().unwrap().clone()) {
            SupState::Stopped(f)
        } else if slots.iter().any(|s| s.up.load(Ordering::SeqCst)) {
            SupState::Ready
        } else {
            SupState::Idle
        }
    }

    fn publish(&self) {
        self.inner.state.send_if_modified(|state| {
            let next = self.compute_state();
            let changed = *state != next;
            *state = next;
            changed
        });
    }

    /// Sets the files to run. A changed config takes effect at the next start and forgets the failure of what changed.
    pub fn configure(&self, cfg: Option<SupervisorConfig>) {
        let (llm_changed, stt_changed) = {
            let mut current = self.inner.cfg.lock().unwrap();
            let llm = |c: &SupervisorConfig| (c.llama_exe.clone(), c.llama_model.clone(), c.mmproj.clone(), c.ngl);
            let stt = |c: &SupervisorConfig| (c.whisper_exe.clone(), c.whisper_model.clone(), c.whisper_gpu);
            let changed = (current.as_ref().map(llm) != cfg.as_ref().map(llm), current.as_ref().map(stt) != cfg.as_ref().map(stt));
            *current = cfg;
            changed
        };
        for (changed, slot) in [(llm_changed, &self.inner.llm), (stt_changed, &self.inner.stt)] {
            if changed {
                *slot.failure.lock().unwrap() = None;
                slot.crashes.lock().unwrap().clear();
            }
        }
        self.publish();
    }

    pub fn set_cpu_fallback_hook(&self, hook: CpuFallbackHook) {
        *self.inner.on_cpu_fallback.lock().unwrap() = Some(hook);
    }

    pub fn set_unload_after(&self, ms: u64) {
        if let Some(c) = self.inner.cfg.lock().unwrap().as_mut() {
            c.unload_after_ms = ms;
        }
    }

    /// While a server starts: the seconds that are probably left (at least 1).
    pub fn loading_eta_sec(&self) -> Option<u32> {
        let cfg = self.inner.cfg.lock().unwrap().clone()?;
        [(&self.inner.llm, &cfg.llama_model), (&self.inner.stt, &cfg.whisper_model)]
            .into_iter()
            .filter(|(slot, _)| slot.starting.load(Ordering::SeqCst))
            .filter_map(|(slot, model)| {
                let since = (*slot.since.lock().unwrap())?;
                let size = std::fs::metadata(model).map_or(0, |m| m.len());
                let expected = load_estimate_sec(slot.last_load_ms.load(Ordering::SeqCst), size);
                Some(expected.saturating_sub(since.elapsed().as_secs()).max(1) as u32)
            })
            .max()
    }

    fn now_ms(&self) -> u64 {
        self.inner.t0.elapsed().as_millis() as u64
    }

    /// Marks a voice request: restarts the idle clock.
    pub fn touch(&self) {
        *self.inner.last_used_ms.lock().unwrap() = self.now_ms();
    }

    /// Lazy start of both servers; the error is the model's when both failed.
    pub async fn ensure_started(&self) -> Result<Endpoints, String> {
        let (llm, stt) = tokio::join!(self.ensure_llm(), self.ensure_stt());
        let (llama_port, llama_key) = llm.map_err(|f| f.detail)?;
        let (whisper_port, whisper_gpu) = stt.map_err(|f| f.detail)?;
        Ok(Endpoints { llama_port, llama_key, whisper_port, whisper_gpu })
    }

    /// The model server (port and key), started on the first call; waits until it is healthy.
    pub async fn ensure_llm(&self) -> Result<(u16, String), Failure> {
        self.ensure(Server::Llm).await
    }

    /// The recognition server (port, whether it runs on the card), started on the first call. It does not need the model.
    pub async fn ensure_stt(&self) -> Result<(u16, bool), Failure> {
        let (port, _) = self.ensure(Server::Stt).await?;
        let gpu = self.inner.cfg.lock().unwrap().as_ref().is_some_and(|c| c.whisper_gpu);
        Ok((port, gpu))
    }

    async fn ensure(&self, server: Server) -> Result<(u16, String), Failure> {
        let slot = self.slot(server);
        let mut live = slot.live.lock().await;
        self.touch();
        if let Some(f) = self.failure_of(server) {
            return Err(f); // needs a manual `retry`
        }
        if let Some(l) = live.as_ref() {
            return Ok((l.port, l.key.clone()));
        }
        if self.inner.cfg.lock().unwrap().is_none() {
            return Err(plain_failure(server, NOT_INSTALLED));
        }
        let port = free_port().map_err(|e| plain_failure(server, e.to_string()))?;
        let key = new_key();
        let began = Instant::now();
        let started = {
            *slot.since.lock().unwrap() = Some(began);
            let _starting = Starting::new(&slot.starting);
            self.publish();
            self.launch(server, port, &key).await
        };
        let result = match started {
            Ok(running) => {
                let (stop, stop_rx) = watch::channel(false);
                slot.up.store(true, Ordering::SeqCst);
                self.spawn_monitor(server, port, key.clone(), running, stop_rx);
                *live = Some(Live { port, key: key.clone(), stop });
                slot.last_load_ms.store(began.elapsed().as_millis().max(1) as u64, Ordering::SeqCst);
                // The quiet time counts from the moment the server can be asked, not from the start of a slow load.
                self.touch();
                Ok((port, key))
            }
            Err(f) => {
                *slot.failure.lock().unwrap() = Some(f.clone());
                Err(f)
            }
        };
        self.publish();
        result
    }

    /// Starts one server and waits until it answers.
    async fn launch(&self, server: Server, port: u16, key: &str) -> Result<Running, Failure> {
        let Some(cfg) = self.inner.cfg.lock().unwrap().clone() else { return Err(plain_failure(server, NOT_INSTALLED)) };
        match server {
            Server::Llm => {
                let (running, used) = with_cpu_fallback(cfg.ngl, |ngl| {
                    let spec = Spec { server, exe: cfg.llama_exe.clone(), args: llama_args(&cfg.llama_model, cfg.mmproj.as_deref(), port, key, ngl), port, health: "/health" };
                    async move { self.start_server(&spec).await }
                })
                .await?;
                if used != cfg.ngl {
                    self.remember_cpu(&cfg.llama_model);
                }
                Ok(running)
            }
            Server::Stt => {
                let spec = Spec { server, exe: cfg.whisper_exe.clone(), args: whisper_args(&cfg.whisper_model, port, physical_cores()), port, health: "/" };
                let running = self.start_server(&spec).await?;
                // The first run on a video card compiles its kernels (tens of seconds, once): a silent second here, so that
                // it happens while the recognition is "loading" and not inside the first phrase.
                if cfg.whisper_gpu {
                    warm_whisper(port).await;
                }
                Ok(running)
            }
        }
    }

    fn remember_cpu(&self, model: &Path) {
        if let Some(c) = self.inner.cfg.lock().unwrap().as_mut().filter(|c| c.llama_model == model) {
            c.ngl = 0;
        }
        let hook = self.inner.on_cpu_fallback.lock().unwrap().clone();
        if let Some(hook) = hook {
            hook(model);
        }
    }

    /// Unloads both servers (idle, `Remove`, shutdown). Does not forget a failure.
    pub async fn stop(&self) {
        for slot in [&self.inner.llm, &self.inner.stt] {
            Self::unload(slot).await;
        }
        self.publish();
    }

    /// Unloads the model server only (its set of files changed); recognition stays.
    pub async fn stop_llm(&self) {
        Self::unload(&self.inner.llm).await;
        self.publish();
    }

    async fn unload(slot: &Slot) {
        if let Some(live) = slot.live.lock().await.take() {
            let _ = live.stop.send(true);
        }
        slot.up.store(false, Ordering::SeqCst);
    }

    /// The projector the model server is configured with.
    pub fn projector(&self) -> Option<PathBuf> {
        self.inner.cfg.lock().unwrap().as_ref().and_then(|c| c.mmproj.clone())
    }

    /// `Retry` after a server gave up: forgets the failures and the crash history; the next request starts again.
    pub async fn retry(&self) {
        self.stop().await;
        for slot in [&self.inner.llm, &self.inner.stt] {
            *slot.failure.lock().unwrap() = None;
            slot.crashes.lock().unwrap().clear();
        }
        self.publish();
    }

    async fn start_server(&self, spec: &Spec) -> Result<Running, Failure> {
        let name = spec.server.name();
        let mut cmd = tokio::process::Command::new(&spec.exe);
        cmd.args(&spec.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = cmd.spawn().map_err(|e| plain_failure(spec.server, format!("{name}: {e}")))?;
        #[cfg(windows)]
        if let Some(h) = child.raw_handle() {
            if let Err(e) = self.inner.job.assign(h) {
                tracing::warn!("{name}: could not join the job object: {e}");
            }
        }
        let tail = Arc::new(Mutex::new(Tail::default()));
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            readers.push(self.handle.spawn(read_output(out, tail.clone(), name)));
        }
        if let Some(err) = child.stderr.take() {
            readers.push(self.handle.spawn(read_output(err, tail.clone(), name)));
        }
        let mut running = Running { child, tail, readers };
        match wait_healthy(spec.port, spec.health, READY_TIMEOUT, &mut running.child).await {
            Ok(()) => Ok(running),
            Err(why) => {
                let code = running.child.try_wait().ok().flatten().and_then(|s| s.code());
                let _ = running.child.start_kill();
                Err(running.failure(spec.server, code, &why).await)
            }
        }
    }

    fn spawn_monitor(&self, server: Server, port: u16, key: String, mut running: Running, mut stop: watch::Receiver<bool>) {
        let me = self.clone();
        self.handle.spawn(async move {
            loop {
                let status = tokio::select! {
                    status = running.child.wait() => status.ok(),
                    _ = stop.changed() => {
                        let _ = running.child.start_kill();
                        return;
                    }
                };
                // The child exited by itself: a crash.
                let slot = me.slot(server);
                slot.up.store(false, Ordering::SeqCst);
                let mut failure = running.failure(server, status.and_then(|s| s.code()), "stopped").await;
                tracing::warn!(target: "voice", "{} stopped (exit {:?}): {}", server.name(), failure.code, failure.detail);
                running = loop {
                    let decision = on_crash(&mut slot.crashes.lock().unwrap(), me.now_ms());
                    let CrashDecision::Restart { after_ms } = decision else {
                        *slot.failure.lock().unwrap() = Some(failure);
                        me.publish();
                        return;
                    };
                    let started = {
                        *slot.since.lock().unwrap() = Some(Instant::now());
                        let _starting = Starting::new(&slot.starting);
                        me.publish();
                        let stopped = tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(after_ms)) => false,
                            _ = stop.changed() => true,
                        };
                        if stopped { None } else { Some(me.launch(server, port, &key).await) }
                    };
                    match started {
                        // Stopped while waiting or starting: dropping the child kills it.
                        None => {
                            me.publish();
                            return;
                        }
                        Some(Ok(next)) => {
                            if stop.has_changed().unwrap_or(true) {
                                me.publish();
                                return;
                            }
                            break next;
                        }
                        Some(Err(f)) => failure = f,
                    }
                };
                slot.up.store(true, Ordering::SeqCst);
                me.publish();
            }
        });
    }

    /// One watch for the life of the supervisor: unloads the servers after the quiet time of the settings.
    fn spawn_idle_watch(&self) {
        let (inner, handle) = (Arc::downgrade(&self.inner), self.handle.clone());
        self.handle.spawn(async move {
            loop {
                tokio::time::sleep(IDLE_CHECK).await;
                let Some(inner) = inner.upgrade() else { return };
                let me = Supervisor { inner, handle: handle.clone() };
                let after = me.inner.cfg.lock().unwrap().as_ref().map_or(IDLE_MS, |c| c.unload_after_ms);
                let last = *me.inner.last_used_ms.lock().unwrap();
                // Never in the middle of a start: stopping waits for it and would throw the fresh server away.
                let starting = me.inner.llm.starting.load(Ordering::SeqCst) || me.inner.stt.starting.load(Ordering::SeqCst);
                if !starting && idle_expired(last, me.now_ms(), after, me.is_up(Server::Llm) || me.is_up(Server::Stt)) {
                    me.stop().await;
                }
            }
        });
    }
}

struct Running {
    child: tokio::process::Child,
    tail: Arc<Mutex<Tail>>,
    readers: Vec<tokio::task::JoinHandle<()>>,
}

impl Running {
    /// Why it failed, after its output has been read to the end.
    async fn failure(&mut self, server: Server, code: Option<i32>, why: &str) -> Failure {
        for reader in self.readers.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(2), reader).await;
        }
        let tail = self.tail.lock().unwrap();
        classify(server, tail.hint.clone(), code, &tail.last, why)
    }
}

async fn read_output(out: impl tokio::io::AsyncRead + Unpin, tail: Arc<Mutex<Tail>>, name: &'static str) {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut reader = BufReader::new(out);
    let mut raw = Vec::new();
    // Bytes, not `lines()`: a path in another code page must not end the reading.
    while reader.read_until(b'\n', &mut raw).await.is_ok_and(|n| n > 0) {
        let line = String::from_utf8_lossy(&raw).trim().to_string();
        raw.clear();
        if line.is_empty() {
            continue;
        }
        let mut t = tail.lock().unwrap();
        if t.logged < MAX_LOG_LINES {
            t.logged += 1;
            tracing::info!(target: "voice", "{name}: {line}");
        }
        t.hint = stronger(t.hint.take(), &line);
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

/// The limit of one recognition (architect, M21-4): 15 s on a GPU; 10 s + 3 x the length of the recording on a CPU, at most 60 s.
pub fn stt_timeout(gpu: bool, seconds: f64) -> Duration {
    if gpu {
        Duration::from_secs(15)
    } else {
        Duration::from_secs_f64((10.0 + 3.0 * seconds.max(0.0)).min(60.0))
    }
}

/// `audio_ctx` of whisper for a recording: 50 frames per second plus a margin, not the whole 30 s window (1500 frames).
pub fn audio_ctx(seconds: f64) -> u32 {
    ((seconds.max(0.0) * 50.0).ceil() as u32 + 150).clamp(300, 1500)
}

/// Physical cores (the threads of the recognition); the logical count halved when the system does not say.
pub fn physical_cores() -> usize {
    let logical = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    sysinfo::System::physical_core_count().unwrap_or((logical / 2).max(1)).max(1)
}

/// The `multipart/form-data` body of `POST /inference` (DATA_MODEL.md 12.4 item 2): `file` (WAV), the forced `language`,
/// `response_format=json`, `temperature=0`.
pub fn whisper_multipart(boundary: &str, wav: &[u8], language: &str, audio_ctx: u32) -> Vec<u8> {
    whisper_multipart_prompted(boundary, wav, language, audio_ctx, None)
}

/// `whisper_multipart` with the `prompt` of whisper (the personal dictionary, DATA_MODEL.md 13.1): words it should spell this way.
pub fn whisper_multipart_prompted(boundary: &str, wav: &[u8], language: &str, audio_ctx: u32, prompt: Option<&str>) -> Vec<u8> {
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
    // Short phrases: greedy decoding, no timestamps, only the part of the window that holds the recording.
    field(&mut body, "beam_size", "1");
    field(&mut body, "best_of", "1");
    field(&mut body, "no_timestamps", "true");
    field(&mut body, "audio_ctx", &audio_ctx.to_string());
    if let Some(p) = prompt.filter(|p| !p.trim().is_empty()) {
        field(&mut body, "prompt", p);
    }
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

/// One second of silence through `/inference`; failures are ignored (a real request reports its own).
async fn warm_whisper(port: u16) {
    let wav = crate::voice::audio::encode_wav_pcm16(&vec![0i16; 16_000], 16_000);
    let boundary = format!("decka{}", new_key());
    let body = whisper_multipart(&boundary, &wav, "en", audio_ctx(1.0));
    let Ok(client) = reqwest::Client::builder().timeout(Duration::from_secs(170)).build() else { return };
    let _ = client
        .post(format!("http://127.0.0.1:{port}/inference"))
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(body)
        .send()
        .await;
}

impl Supervisor {
    /// Recognises 16 kHz mono PCM16 with `whisper-server` (started lazily; the first call waits for it, not for the model).
    pub async fn transcribe(&self, pcm: &[u8], language: &str) -> Result<String, String> {
        self.transcribe_prompted(pcm, language, None).await
    }

    /// `transcribe` with a prompt for whisper (the dictionary of a dictation).
    pub async fn transcribe_prompted(&self, pcm: &[u8], language: &str, prompt: Option<&str>) -> Result<String, String> {
        let (port, gpu) = self.ensure_stt().await.map_err(|f| f.detail)?;
        let samples: Vec<i16> = pcm.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        let wav = crate::voice::audio::encode_wav_pcm16(&samples, 16_000);
        let boundary = format!("decka{}", new_key());
        let body = whisper_multipart_prompted(&boundary, &wav, language, audio_ctx(samples.len() as f64 / 16_000.0), prompt);
        let client = reqwest::Client::builder().timeout(stt_timeout(gpu, samples.len() as f64 / 16_000.0)).build().map_err(|e| e.to_string())?;
        let url = format!("http://127.0.0.1:{port}/inference");
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
        let body = whisper_multipart("BND", b"RIFFdata", "ru", 400);
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(text.starts_with("--BND\r\nContent-Disposition: form-data; name=\"file\"; filename=\"voice.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFFdata\r\n"));
        let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle} missing"));
        assert!(at("name=\"language\"\r\n\r\nru\r\n") < at("name=\"response_format\"\r\n\r\njson\r\n"));
        assert!(at("name=\"response_format\"") < at("name=\"temperature\"\r\n\r\n0\r\n"));
        assert!(text.ends_with("--BND--\r\n"));
        assert_eq!(text.matches("--BND\r\n").count(), 8);
        assert!(text.contains("name=\"audio_ctx\"\r\n\r\n400\r\n") && text.contains("name=\"beam_size\"\r\n\r\n1\r\n"));
        assert_eq!(stt_timeout(true, 20.0).as_secs(), 15);
        assert_eq!(stt_timeout(false, 2.0).as_secs(), 16);
        assert_eq!(stt_timeout(false, 30.0).as_secs(), 60);
        assert_eq!(audio_ctx(2.0), 300);
        assert_eq!(audio_ctx(30.0), 1500);
    }

    #[test]
    fn the_dictionary_goes_to_whisper_as_a_prompt_only_when_there_is_one() {
        let with = String::from_utf8_lossy(&whisper_multipart_prompted("B", b"W", "ru", 400, Some("Decka, Qwen"))).to_string();
        assert!(with.contains("name=\"prompt\"\r\n\r\nDecka, Qwen\r\n"));
        let without = String::from_utf8_lossy(&whisper_multipart_prompted("B", b"W", "ru", 400, Some("  "))).to_string();
        assert!(!without.contains("prompt"));
        assert_eq!(whisper_multipart("B", b"W", "ru", 400), whisper_multipart_prompted("B", b"W", "ru", 400, None));
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
    fn idle_unload_needs_a_running_server_and_the_quiet_time_of_the_settings() {
        assert!(idle_expired(0, IDLE_MS, IDLE_MS, true));
        assert!(!idle_expired(0, IDLE_MS - 1, IDLE_MS, true));
        assert!(idle_expired(0, 60_000, 60_000, true), "one minute");
        assert!(!idle_expired(0, IDLE_MS * 50, 0, true), "0 keeps the model loaded");
        assert!(!idle_expired(0, IDLE_MS * 5, IDLE_MS, false), "nothing to unload");
        assert!(!idle_expired(IDLE_MS, 0, IDLE_MS, true), "clock going backwards is not idle");
    }

    #[test]
    fn the_load_estimate_is_the_last_load_else_a_guess_from_the_size() {
        assert_eq!(load_estimate_sec(0, 0), 3);
        assert_eq!(load_estimate_sec(0, 3_000_000_000), 13);
        assert_eq!(load_estimate_sec(7_200, 3_000_000_000), 8);
    }

    #[test]
    fn server_arguments_follow_section_12_3() {
        let a = llama_args(Path::new("m.gguf"), None, 5555, "k", 99);
        assert_eq!(
            a.join(" "),
            "-m m.gguf -c 4096 -ngl 99 --cache-ram 0 --ctx-checkpoints 4 --jinja --no-webui --host 127.0.0.1 --port 5555 --api-key k"
        );
        let with = llama_args(Path::new("m.gguf"), Some(Path::new("p.gguf")), 5555, "k", 99);
        assert_eq!(with.join(" "), "-m m.gguf --mmproj p.gguf -c 4096 -ngl 99 --cache-ram 0 --ctx-checkpoints 4 --jinja --no-webui --host 127.0.0.1 --port 5555 --api-key k");
        // T28.1: a part of the layers (or none) puts the projector on the processor.
        let part = llama_args(Path::new("m.gguf"), Some(Path::new("p.gguf")), 5555, "k", 18);
        assert_eq!(part.join(" "), "-m m.gguf --mmproj p.gguf --no-mmproj-offload -c 4096 -ngl 18 --cache-ram 0 --ctx-checkpoints 4 --jinja --no-webui --host 127.0.0.1 --port 5555 --api-key k");
        assert!(!llama_args(Path::new("m.gguf"), None, 5555, "k", 0).contains(&"--no-mmproj-offload".to_string()));
        assert_eq!(whisper_args(Path::new("w.bin"), 6000, 4).join(" "), "-m w.bin --host 127.0.0.1 --port 6000 -t 4");
        assert_eq!(whisper_args(Path::new("w.bin"), 6000, 1).last().unwrap(), "1", "at least one thread");
    }

    fn failed(reason: Reason) -> Failure {
        Failure { server: Server::Llm, reason, code: Some(1), detail: format!("{reason:?}") }
    }

    /// Runs `with_cpu_fallback` over a script of results; returns the outcome and the `-ngl` of every attempt.
    async fn attempts(ngl: u32, script: Vec<Result<(), Failure>>) -> (Result<u32, Failure>, Vec<u32>) {
        let tried = std::cell::RefCell::new(Vec::new());
        let mut script = script.into_iter();
        let result = with_cpu_fallback(ngl, |n| {
            tried.borrow_mut().push(n);
            std::future::ready(script.next().expect("no further attempt"))
        })
        .await;
        (result.map(|(_, used)| used), tried.into_inner())
    }

    #[tokio::test]
    async fn a_failed_start_is_retried_once_on_the_processor() {
        assert_eq!(attempts(99, vec![Ok(())]).await, (Ok(99), vec![99]));
        assert_eq!(attempts(99, vec![Err(failed(Reason::Crash)), Ok(())]).await, (Ok(0), vec![99, 0]), "after a crash");
        assert_eq!(attempts(18, vec![Err(failed(Reason::Vram)), Ok(())]).await, (Ok(0), vec![18, 0]));
        let (result, tried) = attempts(99, vec![Err(failed(Reason::Vram)), Err(failed(Reason::Crash))]).await;
        assert_eq!((result.unwrap_err().reason, tried), (Reason::Vram, vec![99, 0]), "only one retry");
        let (result, tried) = attempts(0, vec![Err(failed(Reason::Ram))]).await;
        assert_eq!((result.unwrap_err().reason, tried), (Reason::Ram, vec![0]), "already on the processor");
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

    fn missing() -> SupervisorConfig {
        SupervisorConfig {
            llama_exe: PathBuf::from("Z:\\nope\\llama-server.exe"),
            llama_model: PathBuf::from("m"),
            mmproj: None,
            ngl: 99,
            whisper_exe: PathBuf::from("Z:\\nope\\whisper-server.exe"),
            whisper_model: PathBuf::from("w"),
            unload_after_ms: IDLE_MS,
            whisper_gpu: false,
        }
    }

    #[tokio::test]
    async fn a_missing_executable_stops_with_a_message_and_retry_clears_it() {
        let sup = Supervisor::new(tokio::runtime::Handle::current()).unwrap();
        sup.configure(Some(missing()));
        let err = sup.ensure_started().await.unwrap_err();
        assert!(err.contains("server"), "{err}");
        assert!(matches!(sup.state(), SupState::Stopped(_)));
        assert!(sup.ensure_started().await.is_err(), "no new attempts before Retry");
        assert_eq!(sup.failure_of(Server::Llm).map(|f| f.reason), Some(Reason::Crash));
        assert_eq!(sup.failure_of(Server::Stt).map(|f| f.reason), Some(Reason::Stt));
        sup.retry().await;
        assert_eq!(sup.state(), SupState::Idle);
    }

    #[tokio::test]
    async fn the_servers_fail_apart_and_another_model_forgets_the_failure_of_the_model() {
        let sup = Supervisor::new(tokio::runtime::Handle::current()).unwrap();
        sup.configure(Some(missing()));
        assert!(sup.ensure_llm().await.is_err());
        assert_eq!(sup.failure_of(Server::Stt), None, "the recognition was not asked for and is not blamed");
        sup.configure(Some(SupervisorConfig { llama_model: PathBuf::from("other"), ..missing() }));
        assert_eq!((sup.failure_of(Server::Llm), sup.state()), (None, SupState::Idle));
    }
}
