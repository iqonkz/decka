//! The voice package as the settings window sees it (IPC contract in shared/src/ipc.ts): install by one button, progress,
//! settings, removal. It owns the downloader and the supervisor; it never touches the network channel to the phone.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::download::{extract_archive, Downloader, ManifestEntry};
use super::failure::{Failure, Reason, Server};
use super::gpu;
use super::hardware::{detect_hardware, HardwareInfo};
use super::supervisor::{SupState, Supervisor, SupervisorConfig};
use crate::model::GpuLayers;

/// With less memory than this the 4B model is a poor fit: the settings offer the light 2B one (T28.1).
pub const LIGHT_UNDER_RAM_GB: f64 = 12.0;

// ---- Contract types (mirror `VoiceStatus` in shared/src/ipc.ts) --------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceState {
    Off,
    Downloading,
    Ready,
    Loading,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelChoice {
    Standard,
    Light,
    /// Qwen3.5 9B (T27.7): offered only on a PC with 16 GB of RAM or 8 GB of video memory; chosen by hand, never recommended.
    #[serde(rename = "9b")]
    Extended,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    pub state: VoiceState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
    pub hardware: HardwareInfo,
    pub model: ModelChoice,
    pub download_gb: f64,
    pub hotkey: Vec<String>,
    pub keep_loaded: bool,
    pub speak: bool,
    pub phone_button: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mic: Option<String>,
    pub mics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// T28.2: why a server does not work; `error` keeps the raw line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// The exit code of the process that stopped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// T28.1: the 4B model on a PC with little memory — offer the light one (never switched by itself).
    pub suggest_light: bool,
}

/// Arguments of `voice_set`; every field is optional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSetArgs {
    pub model: Option<ModelChoice>,
    pub hotkey: Option<Vec<String>>,
    pub keep_loaded: Option<bool>,
    pub speak: Option<bool>,
    pub phone_button: Option<bool>,
    pub mic: Option<String>,
}

/// Persisted in `<config dir>/voice.json`. `model: None` = the recommendation for this PC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceSettings {
    pub model: Option<ModelChoice>,
    pub hotkey: Vec<String>,
    pub keep_loaded: bool,
    pub speak: bool,
    pub phone_button: bool,
    pub mic: Option<String>,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        VoiceSettings {
            model: None,
            hotkey: vec!["ctrl".into(), "alt".into(), "space".into()],
            keep_loaded: false,
            speak: false,
            phone_button: true,
            mic: None,
        }
    }
}

// ---- Hardware (detection and the 12.2 rule live in `hardware.rs`) ------------------------------------------------

/// Where the PC's hardware comes from; tests give a fixed answer.
pub trait HardwareProbe: Send + Sync {
    fn probe(&self) -> HardwareInfo;
}

/// The real thing: DXGI for the video card, `sysinfo` for the RAM.
pub struct DetectProbe;

impl HardwareProbe for DetectProbe {
    fn probe(&self) -> HardwareInfo {
        detect_hardware()
    }
}

/// DATA_MODEL.md 12.2 through `hardware::recommend_model`.
pub fn recommend(hw: &HardwareInfo) -> ModelChoice {
    match hw.recommended_model() {
        "standard" => ModelChoice::Standard,
        _ => ModelChoice::Light,
    }
}
// ---- Plan of files ----------------------------------------------------------------------------------------------

/// The manifest entries one install needs: both runtimes, the chosen model, the speech model.
pub fn plan(manifest: &[ManifestEntry], model: ModelChoice, gpu: bool) -> Vec<&ManifestEntry> {
    let llm = match model {
        ModelChoice::Standard => "llm-standard",
        ModelChoice::Light => "llm-light",
        ModelChoice::Extended => "llm-extended",
    };
    ["llama-runtime", if gpu { "whisper-runtime-cuda" } else { "whisper-runtime" }, llm, "stt"].iter().filter_map(|id| manifest.iter().find(|e| e.id == *id)).collect()
}

fn url_file_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or("file").split('?').next().unwrap_or("file").to_string()
}

fn is_archive(e: &ManifestEntry) -> bool {
    e.kind == "runtime"
}

/// Where a finished entry lives: a runtime folder or a model file.
fn target_of(root: &Path, e: &ManifestEntry) -> PathBuf {
    if e.kind == "vision" {
        // Both projectors are called `mmproj-F16.gguf` upstream: the id keeps them apart.
        root.join("models").join(format!("{}-{}", e.id, url_file_name(&e.url)))
    } else if is_archive(e) {
        root.join("runtime").join(format!("{}-{}", e.id, e.sha256[..8.min(e.sha256.len())].to_ascii_lowercase()))
    } else {
        root.join("models").join(url_file_name(&e.url))
    }
}

fn is_present(root: &Path, e: &ManifestEntry) -> bool {
    let t = target_of(root, e);
    if is_archive(e) {
        t.is_dir()
    } else {
        std::fs::metadata(&t).map(|m| m.len() == e.size).unwrap_or(false)
    }
}

/// Finds `name` inside `dir` (archives may keep the executables in a sub-folder).
pub fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(f) = find_file(&p, name) {
                return Some(f);
            }
        } else if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(name)) {
            return Some(p);
        }
    }
    None
}

pub fn default_root() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Decka").join("ai")
}

// ---- Layers on the video card ------------------------------------------------------------------------------------

/// Where "this model runs on the processor on this card" is kept (`config.json`, `ai.gpuLayers`).
pub trait LayerMemory: Send + Sync {
    fn load(&self) -> Option<GpuLayers>;
    fn save(&self, value: Option<GpuLayers>);
}

/// Remembers nothing (the examples).
pub struct NoLayerMemory;

impl LayerMemory for NoLayerMemory {
    fn load(&self) -> Option<GpuLayers> {
        None
    }
    fn save(&self, _: Option<GpuLayers>) {}
}

/// The video card as the memory of the layers knows it: another card (or another amount of memory) forgets it.
pub fn gpu_key(hw: &HardwareInfo) -> String {
    format!("{}|{:.1}", hw.gpu.as_deref().unwrap_or(""), hw.vram_gb.unwrap_or(0.0))
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

// ---- Service ----------------------------------------------------------------------------------------------------

type Emit = Arc<dyn Fn(VoiceStatus) + Send + Sync>;

struct Download {
    cancel: Arc<AtomicBool>,
    progress: Progress,
    join: Option<tokio::task::JoinHandle<()>>,
}

struct State {
    settings: VoiceSettings,
    download: Option<Download>,
    last_error: Option<String>,
    /// The hotkey could not be registered (taken by another program): shown in the status, voice control still works.
    hotkey_error: Option<String>,
}

struct Inner {
    root: PathBuf,
    settings_path: PathBuf,
    manifest: Vec<ManifestEntry>,
    hardware: Arc<dyn HardwareProbe>,
    /// The video card does not change while the agent runs: asked once (the DXGI query is slow on a busy card).
    hardware_cache: std::sync::OnceLock<HardwareInfo>,
    mics: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    downloader: Downloader,
    supervisor: Supervisor,
    layers: Arc<dyn LayerMemory>,
    emit: Emit,
    state: Mutex<State>,
    /// The percent of a running download of the Screen help package, `IDLE` when none runs.
    vision_percent: AtomicU8,
}

const IDLE: u8 = u8::MAX;

/// The Screen help package as the settings page shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct VisionPack {
    pub installed: bool,
    pub size: u64,
    pub percent: Option<u8>,
}

/// Ends the "a download runs" mark, also when the download is abandoned.
struct Downloading<'a>(&'a AtomicU8);

impl Drop for Downloading<'_> {
    fn drop(&mut self) {
        self.0.store(IDLE, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct VoiceService {
    inner: Arc<Inner>,
    handle: tokio::runtime::Handle,
}

pub struct VoiceDeps {
    pub root: PathBuf,
    /// `voice.json` in the config directory.
    pub settings_path: PathBuf,
    pub manifest: Vec<ManifestEntry>,
    pub hardware: Arc<dyn HardwareProbe>,
    pub mics: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    pub downloader: Downloader,
    pub layers: Arc<dyn LayerMemory>,
    /// Called after every change (the window event `voice-status`).
    pub emit: Emit,
}

impl VoiceService {
    pub fn new(handle: tokio::runtime::Handle, deps: VoiceDeps) -> std::io::Result<VoiceService> {
        let settings: VoiceSettings =
            std::fs::read_to_string(&deps.settings_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let supervisor = Supervisor::new(handle.clone())?;
        let svc = VoiceService {
            inner: Arc::new(Inner {
                root: deps.root,
                settings_path: deps.settings_path,
                manifest: deps.manifest,
                hardware: deps.hardware,
                hardware_cache: std::sync::OnceLock::new(),
                mics: deps.mics,
                downloader: deps.downloader,
                supervisor,
                layers: deps.layers,
                emit: deps.emit,
                vision_percent: AtomicU8::new(IDLE),
                state: Mutex::new(State { settings, download: None, last_error: None, hotkey_error: None }),
            }),
            handle,
        };
        // The card could not take the model: remembered, so that the next start goes to the processor at once.
        let weak = Arc::downgrade(&svc.inner);
        svc.inner.supervisor.set_cpu_fallback_hook(Arc::new(move |model: &Path| {
            let Some(inner) = weak.upgrade() else { return };
            let hw = inner.hardware_cache.get_or_init(|| inner.hardware.probe());
            inner.layers.save(Some(GpuLayers { model: file_name(model), gpu: gpu_key(hw), layers: 0 }));
        }));
        svc.configure_supervisor();
        // Every supervisor state change reaches the window.
        let watcher = svc.clone();
        let mut rx = svc.inner.supervisor.subscribe();
        svc.handle.spawn(async move {
            while rx.changed().await.is_ok() {
                watcher.emit();
            }
        });
        Ok(svc)
    }

    pub fn supervisor(&self) -> &Supervisor {
        &self.inner.supervisor
    }

    fn hardware(&self) -> HardwareInfo {
        self.inner.hardware_cache.get_or_init(|| self.inner.hardware.probe()).clone()
    }

    fn model(&self) -> ModelChoice {
        let chosen = self.inner.state.lock().unwrap().settings.model;
        chosen.unwrap_or_else(|| recommend(&self.hardware()))
    }

    /// The GPU build of `whisper-server` (cuBLAS) for an NVIDIA card with at least 2 GB; otherwise the CPU build.
    fn whisper_gpu(&self) -> bool {
        self.inner.manifest.iter().any(|e| e.id == "whisper-runtime-cuda") && self.hardware().whisper_gpu()
    }

    fn installed(&self) -> bool {
        let files = plan(&self.inner.manifest, self.model(), self.whisper_gpu());
        files.len() == 4 && files.iter().all(|e| is_present(&self.inner.root, e))
    }

    /// Points the supervisor at the installed files (or at nothing).
    fn configure_supervisor(&self) {
        let cfg = self.supervisor_config();
        self.inner.supervisor.configure(cfg);
    }

    fn supervisor_config(&self) -> Option<SupervisorConfig> {
        if !self.installed() {
            return None;
        }
        let files = plan(&self.inner.manifest, self.model(), self.whisper_gpu());
        let root = &self.inner.root;
        let llama_exe = find_file(&target_of(root, files[0]), "llama-server.exe")?;
        let whisper_exe = find_file(&target_of(root, files[1]), "whisper-server.exe")?;
        let keep_loaded = self.inner.state.lock().unwrap().settings.keep_loaded;
        let llama_model = target_of(root, files[2]);
        let mmproj = self.vision_entry().filter(|e| is_present(root, e)).map(|e| target_of(root, e));
        let whisper_gpu = files[1].id == "whisper-runtime-cuda";
        let ngl = self.gpu_layers(&llama_model, mmproj.as_deref(), whisper_gpu);
        Some(SupervisorConfig {
            llama_exe,
            llama_model,
            mmproj,
            ngl,
            whisper_exe,
            whisper_model: target_of(root, files[3]),
            keep_loaded,
            whisper_gpu,
        })
    }

    /// `-ngl` for this model on this card: what was remembered after a failed start, else the share that fits (T28.1).
    fn gpu_layers(&self, model: &Path, mmproj: Option<&Path>, whisper_gpu: bool) -> u32 {
        let hw = self.hardware();
        if let Some(kept) = self.inner.layers.load() {
            if kept.model == file_name(model) && kept.gpu == gpu_key(&hw) {
                return kept.layers;
            }
            // Another model or another video card: try the card again.
            self.inner.layers.save(None);
        }
        let size = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
        let count = gpu::gguf_layer_count(model).unwrap_or(gpu::DEFAULT_LAYER_COUNT);
        gpu::gpu_layers(hw.vram_gb, whisper_gpu, size(model), mmproj.map_or(0, size), count)
    }

    /// `Download again` of a damaged model file (T28.2): removes what the failure names and fetches it once more.
    pub async fn redownload(&self) -> Result<VoiceStatus, String> {
        let broken: Vec<Failure> = [Server::Llm, Server::Stt]
            .iter()
            .filter_map(|s| self.inner.supervisor.failure_of(*s))
            .filter(|f| f.reason == Reason::ModelFile)
            .collect();
        if broken.is_empty() {
            return Err("No damaged model file".into());
        }
        self.inner.supervisor.stop().await;
        let files: Vec<ManifestEntry> = plan(&self.inner.manifest, self.model(), self.whisper_gpu()).into_iter().cloned().collect();
        if files.len() != 4 {
            return Err("The voice package list is incomplete".into());
        }
        let mut vision = false;
        for failure in &broken {
            let line = failure.detail.to_ascii_lowercase();
            let entry = match failure.server {
                // The projector of Screen help names itself in the line; anything else is the model.
                Server::Llm if ["mmproj", "multimodal", "clip"].iter().any(|n| line.contains(n)) => {
                    vision = true;
                    self.vision_entry().cloned()
                }
                Server::Llm => Some(files[2].clone()),
                Server::Stt => Some(files[3].clone()),
            };
            if let Some(e) = entry {
                std::fs::remove_file(target_of(&self.inner.root, &e)).or_else(|err| if err.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(err.to_string()) })?;
            }
        }
        self.inner.supervisor.retry().await;
        if vision {
            self.install_vision().await?;
        }
        Ok(self.enable())
    }

    /// The projector of the optional Screen help package for the chosen model (DATA_MODEL.md 13.3).
    pub fn vision_entry(&self) -> Option<&ManifestEntry> {
        let id = match self.model() {
            ModelChoice::Standard => "mmproj-standard",
            ModelChoice::Light => "mmproj-light",
            ModelChoice::Extended => "mmproj-extended",
        };
        self.inner.manifest.iter().find(|e| e.id == id)
    }

    /// The package for the settings page: on the disk, its size, and how far a running download is.
    pub fn vision_pack(&self) -> VisionPack {
        let percent = self.inner.vision_percent.load(Ordering::SeqCst);
        VisionPack { installed: self.vision_installed(), size: self.vision_entry().map_or(0, |e| e.size), percent: (percent != IDLE).then_some(percent) }
    }

    /// Deletes the package (the model file only; voice control stays). The server restarts without it at the next request.
    pub async fn remove_vision(&self) -> Result<(), String> {
        if self.inner.vision_percent.load(Ordering::SeqCst) != IDLE {
            return Err("The Screen help package is being downloaded".into());
        }
        let entry = self.vision_entry().cloned().ok_or("The Screen help package is not available")?;
        // The running server holds the file.
        self.inner.supervisor.stop().await;
        let path = target_of(&self.inner.root, &entry);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Cannot delete the package: {e}")),
        }
        self.configure_supervisor();
        Ok(())
    }

    /// Whether the Screen help package is on the disk.
    pub fn vision_installed(&self) -> bool {
        self.vision_entry().is_some_and(|e| is_present(&self.inner.root, e))
    }

    /// Downloads the Screen help package (after the user agreed to it) and restarts the model server with it.
    pub async fn install_vision(&self) -> Result<(), String> {
        self.install_vision_with(&mut |_| {}).await
    }

    /// `install_vision` that reports how far the download is, in percent (T29.2: the island shows it).
    pub async fn install_vision_with(&self, percent: &mut (dyn FnMut(u8) + Send)) -> Result<(), String> {
        let entry = self.vision_entry().cloned().ok_or("The Screen help package is not available")?;
        if !is_present(&self.inner.root, &entry) {
            // One download at a time: the settings page and a spoken "yes" would write the same file.
            if self.inner.vision_percent.compare_exchange(IDLE, 0, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                return Err("The Screen help package is being downloaded already".into());
            }
            let _running = Downloading(&self.inner.vision_percent);
            let dest = target_of(&self.inner.root, &entry);
            let cancel = AtomicBool::new(false);
            let total = entry.size.max(1);
            percent(0);
            let shared = &self.inner.vision_percent;
            self.inner
                .downloader
                .fetch(&entry, &dest, &cancel, &mut |done| {
                    let p = (done.saturating_mul(100) / total).min(100) as u8;
                    shared.store(p, Ordering::SeqCst);
                    percent(p)
                })
                .await
                .map_err(|e| e.to_string())?;
        }
        // The server loads the projector at its start: the running one is stopped, the next request starts it again.
        self.configure_supervisor();
        self.inner.supervisor.stop().await;
        Ok(())
    }

    pub fn status(&self) -> VoiceStatus {
        let hw = self.hardware();
        let model = self.model();
        let installed = self.installed();
        let sup = self.inner.supervisor.state();
        let st = self.inner.state.lock().unwrap();
        let total: u64 = plan(&self.inner.manifest, model, self.whisper_gpu()).iter().map(|e| e.size).sum();
        let mut failure = None;
        let (state, progress, error) = if let Some(d) = &st.download {
            (VoiceState::Downloading, Some(d.progress.clone()), None)
        } else if let Some(msg) = &st.last_error {
            (VoiceState::Error, None, Some(msg.clone()))
        } else if !installed {
            (VoiceState::Off, None, None)
        } else {
            match sup {
                SupState::Loading => (VoiceState::Loading, None, None),
                SupState::Stopped(f) => {
                    let detail = format!("Voice control stopped: {}", f.detail);
                    failure = Some(f);
                    (VoiceState::Error, None, Some(detail))
                }
                SupState::Idle | SupState::Ready => (VoiceState::Ready, None, None),
            }
        };
        VoiceStatus {
            reason: failure.as_ref().map(|f| f.reason),
            exit_code: failure.as_ref().and_then(|f| f.code),
            suggest_light: model == ModelChoice::Standard && hw.ram_gb < LIGHT_UNDER_RAM_GB,
            state,
            progress,
            hardware: hw,
            model,
            download_gb: total as f64 / 1e9,
            hotkey: st.settings.hotkey.clone(),
            keep_loaded: st.settings.keep_loaded,
            speak: st.settings.speak,
            phone_button: st.settings.phone_button,
            mic: st.settings.mic.clone(),
            mics: (self.inner.mics)(),
            error: error.or_else(|| st.hotkey_error.clone()),
        }
    }

    /// The result of registering the hotkey: `Some(text)` shows it as `VoiceStatus.error`, `None` clears it.
    pub fn set_hotkey_error(&self, error: Option<String>) {
        {
            let mut st = self.inner.state.lock().unwrap();
            if st.hotkey_error == error {
                return;
            }
            st.hotkey_error = error;
        }
        self.emit();
    }

    fn emit(&self) {
        (self.inner.emit)(self.status());
    }

    fn save_settings(&self) {
        let s = self.inner.state.lock().unwrap().settings.clone();
        if let Err(e) = crate::config::save_json_atomic(&self.inner.settings_path, &s) {
            tracing::warn!("voice settings were not saved: {e}");
        }
    }

    /// `Turn on voice control` / `Retry`: downloads what is missing, then the servers can start on the first request.
    pub fn enable(&self) -> VoiceStatus {
        // Installed and the servers gave up: this is the manual Retry.
        if self.installed() {
            self.inner.state.lock().unwrap().last_error = None;
            let sup = self.inner.supervisor.clone();
            self.handle.spawn(async move { sup.retry().await });
            self.configure_supervisor();
            return self.status();
        }
        {
            let mut st = self.inner.state.lock().unwrap();
            if st.download.is_some() {
                drop(st);
                return self.status();
            }
            st.last_error = None;
            let cancel = Arc::new(AtomicBool::new(false));
            st.download = Some(Download { cancel: cancel.clone(), progress: Progress { done: 0, total: 0 }, join: None });
            drop(st);
            let me = self.clone();
            let join = self.handle.spawn(async move { me.run_download(cancel).await });
            if let Some(d) = self.inner.state.lock().unwrap().download.as_mut() {
                d.join = Some(join);
            }
        }
        self.emit();
        self.status()
    }

    async fn run_download(&self, cancel: Arc<AtomicBool>) {
        let model = self.model();
        let files: Vec<ManifestEntry> = plan(&self.inner.manifest, model, self.whisper_gpu()).into_iter().cloned().collect();
        let result = if files.len() != 4 {
            Err("The voice package list is incomplete".to_string())
        } else {
            self.download_all(&files, &cancel).await
        };
        {
            let mut st = self.inner.state.lock().unwrap();
            st.download = None;
            st.last_error = match &result {
                Ok(()) => None,
                Err(msg) if cancel.load(Ordering::Relaxed) => {
                    let _ = msg;
                    None
                }
                Err(msg) => Some(msg.clone()),
            };
        }
        if result.is_ok() {
            self.drop_other_model(model);
            self.configure_supervisor();
        }
        self.emit();
    }

    async fn download_all(&self, files: &[ManifestEntry], cancel: &Arc<AtomicBool>) -> Result<(), String> {
        let total: u64 = files.iter().map(|e| e.size).sum();
        let mut finished = 0u64;
        let last_emit = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
        for e in files {
            if is_present(&self.inner.root, e) {
                finished += e.size;
                continue;
            }
            let dir = if is_archive(e) { "downloads" } else { "models" };
            let dest = self.inner.root.join(dir).join(url_file_name(&e.url));
            let me = self.clone();
            let last_emit = last_emit.clone();
            let base = finished;
            let mut on_progress = move |done: u64| {
                if let Some(d) = me.inner.state.lock().unwrap().download.as_mut() {
                    d.progress = Progress { done: base + done, total };
                }
                let mut last = last_emit.lock().unwrap();
                if last.elapsed() >= Duration::from_millis(250) {
                    *last = Instant::now();
                    drop(last);
                    me.emit();
                }
            };
            self.inner.downloader.fetch(e, &dest, cancel, &mut on_progress).await.map_err(|err| err.to_string())?;
            if is_archive(e) {
                let (archive, target) = (dest.clone(), target_of(&self.inner.root, e));
                tokio::task::spawn_blocking(move || extract_archive(&archive, &target))
                    .await
                    .map_err(|err| err.to_string())?
                    .map_err(|err| err.to_string())?;
                let _ = std::fs::remove_file(&dest);
            }
            finished += e.size;
        }
        Ok(())
    }

    /// A model switch downloads the other model, then deletes the previous one (12.2).
    fn drop_other_model(&self, keep: ModelChoice) {
        let keep_id = match keep {
            ModelChoice::Standard => "llm-standard",
            ModelChoice::Light => "llm-light",
            ModelChoice::Extended => "llm-extended",
        };
        // One model at a time: every other model file goes.
        for e in self.inner.manifest.iter().filter(|e| e.kind == "llm" && e.id != keep_id) {
            let _ = std::fs::remove_file(target_of(&self.inner.root, e));
        }
    }

    /// `Cancel`: stops the download; the partial files stay for a later resume.
    pub fn cancel(&self) -> VoiceStatus {
        if let Some(d) = self.inner.state.lock().unwrap().download.as_ref() {
            d.cancel.store(true, Ordering::Relaxed);
        }
        self.status()
    }

    /// `Remove voice control`: stops the servers and erases everything in `ai\`.
    pub async fn remove(&self) -> VoiceStatus {
        let join = {
            let mut st = self.inner.state.lock().unwrap();
            st.last_error = None;
            st.download.as_mut().and_then(|d| {
                d.cancel.store(true, Ordering::Relaxed);
                d.join.take()
            })
        };
        if let Some(j) = join {
            let _ = j.await;
        }
        self.inner.supervisor.stop().await;
        self.inner.supervisor.configure(None);
        let root = self.inner.root.clone();
        for _ in 0..10 {
            if !root.exists() || std::fs::remove_dir_all(&root).is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await; // the servers are still releasing their files
        }
        self.inner.supervisor.retry().await;
        self.emit();
        self.status()
    }

    pub fn set(&self, args: VoiceSetArgs) -> Result<VoiceStatus, String> {
        if let Some(h) = &args.hotkey {
            if h.is_empty() || h.len() > 4 || h.iter().any(|k| k.is_empty() || k.len() > 12 || !k.bytes().all(|b| b.is_ascii_alphanumeric())) {
                return Err("Invalid hotkey".into());
            }
        }
        let mut switch_model = false;
        {
            let mut st = self.inner.state.lock().unwrap();
            if let Some(h) = args.hotkey {
                st.settings.hotkey = h.iter().map(|k| k.to_ascii_lowercase()).collect();
            }
            if let Some(v) = args.keep_loaded {
                st.settings.keep_loaded = v;
            }
            if let Some(v) = args.speak {
                st.settings.speak = v;
            }
            if let Some(v) = args.phone_button {
                st.settings.phone_button = v;
            }
            if let Some(m) = args.mic {
                st.settings.mic = if m.is_empty() { None } else { Some(m) };
            }
            // The extended model only on a PC that can carry it.
            if args.model == Some(ModelChoice::Extended) && !self.hardware().extended_ok() {
                return Err("The extended model needs 16 GB of memory or 8 GB of video memory".into());
            }
            // A model change while a download runs is ignored: the running install keeps its model.
            if let Some(m) = args.model {
                if st.download.is_none() && st.settings.model != Some(m) {
                    st.settings.model = Some(m);
                    switch_model = true;
                }
            }
        }
        self.save_settings();
        self.inner.supervisor.set_keep_loaded(self.inner.state.lock().unwrap().settings.keep_loaded);
        if switch_model {
            let sup = self.inner.supervisor.clone();
            let me = self.clone();
            self.handle.spawn(async move {
                sup.stop().await; // unload the old model before the next start
                me.configure_supervisor();
                // Already installed with another model: fetch the new one (and drop the old one afterwards).
                if me.inner.state.lock().unwrap().last_error.is_none() && !me.installed() && me.was_installed_before_switch() {
                    me.enable();
                } else {
                    me.emit();
                }
            });
        }
        self.emit();
        Ok(self.status())
    }

    /// True when the runtimes and the speech model are there, i.e. the user had voice control turned on.
    fn was_installed_before_switch(&self) -> bool {
        let files = plan(&self.inner.manifest, self.model(), self.whisper_gpu());
        files.len() == 4 && [0usize, 1, 3].iter().all(|i| is_present(&self.inner.root, files[*i]))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_extended_model_is_in_the_manifest_with_its_projector_and_needs_a_strong_pc() {
        let manifest = crate::ai::download::parse_manifest(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/ai/manifest.json")).unwrap()).unwrap();
        let llm = manifest.iter().find(|e| e.id == "llm-extended").expect("the 9B model");
        let vision = manifest.iter().find(|e| e.id == "mmproj-extended").expect("its own projector");
        assert!(llm.url.contains("Qwen3.5-9B-GGUF") && llm.url.ends_with("Q4_K_M.gguf") && llm.license == "Apache-2.0" && llm.sha256.len() == 64);
        assert!(vision.url.contains("Qwen3.5-9B-GGUF") && vision.kind == "vision" && vision.sha256 != manifest.iter().find(|e| e.id == "mmproj-standard").unwrap().sha256);
        let ids: Vec<_> = plan(&manifest, ModelChoice::Extended, false).iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids, ["llama-runtime", "whisper-runtime", "llm-extended", "stt"]);
        let hw = |vram: Option<f64>, ram: f64| HardwareInfo { gpu: None, vram_gb: vram, ram_gb: ram };
        assert!(hw(None, 16.0).extended_ok() && hw(Some(8.0), 8.0).extended_ok());
        assert!(!hw(Some(6.0), 8.0).extended_ok() && !hw(None, 15.9).extended_ok());
        assert_eq!(serde_json::to_string(&ModelChoice::Extended).unwrap(), "\"9b\"");
        assert_ne!(recommend(&hw(Some(24.0), 64.0)), ModelChoice::Extended, "never recommended");
    }

    use super::*;
    use sha2::{Digest, Sha256};

    struct Fixed(HardwareInfo);
    impl HardwareProbe for Fixed {
        fn probe(&self) -> HardwareInfo {
            self.0.clone()
        }
    }

    fn hw(vram: Option<f64>, ram: f64) -> HardwareInfo {
        HardwareInfo { gpu: vram.map(|_| "GPU".into()), vram_gb: vram, ram_gb: ram }
    }

    fn entry(id: &str, kind: &str, url: &str, data: &[u8]) -> ManifestEntry {
        let sha256 = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
        ManifestEntry { id: id.into(), kind: kind.into(), url: url.into(), sha256, size: data.len() as u64, license: "MIT".into() }
    }

    fn manifest() -> Vec<ManifestEntry> {
        vec![
            entry("llama-runtime", "runtime", "http://127.0.0.1:1/llama.zip", b"a"),
            entry("whisper-runtime", "runtime", "http://127.0.0.1:1/whisper.zip", b"bb"),
            entry("llm-standard", "llm", "http://127.0.0.1:1/Qwen3.5-4B-Q4_K_M.gguf", b"cccc"),
            entry("llm-light", "llm", "http://127.0.0.1:1/Qwen3.5-2B-Q4_K_M.gguf", b"dd"),
            entry("stt", "stt", "http://127.0.0.1:1/ggml-large-v3-turbo-q5_0.bin", b"eee"),
        ]
    }

    #[test]
    fn recommendation_follows_the_hardware_table() {
        assert_eq!(recommend(&hw(Some(8.0), 8.0)), ModelChoice::Standard);
        assert_eq!(recommend(&hw(Some(6.0), 8.0)), ModelChoice::Standard);
        assert_eq!(recommend(&hw(Some(4.0), 8.0)), ModelChoice::Light);
        assert_eq!(recommend(&hw(None, 16.0)), ModelChoice::Standard);
        assert_eq!(recommend(&hw(None, 15.9)), ModelChoice::Light);
    }

    #[test]
    fn the_plan_has_both_runtimes_the_chosen_model_and_speech() {
        let m = manifest();
        let ids = |c| plan(&m, c, false).iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(ModelChoice::Standard), ["llama-runtime", "whisper-runtime", "llm-standard", "stt"]);
        assert_eq!(ids(ModelChoice::Light), ["llama-runtime", "whisper-runtime", "llm-light", "stt"]);
        let mut with_cuda = manifest();
        with_cuda.push(entry("whisper-runtime-cuda", "runtime", "http://127.0.0.1:1/whisper-cuda.zip", b"ff"));
        let gpu_ids: Vec<_> = plan(&with_cuda, ModelChoice::Standard, true).iter().map(|e| e.id.clone()).collect();
        assert_eq!(gpu_ids, ["llama-runtime", "whisper-runtime-cuda", "llm-standard", "stt"]);
        let nvidia = |vram| HardwareInfo { gpu: Some("NVIDIA GeForce RTX 2060 SUPER".into()), vram_gb: Some(vram), ram_gb: 16.0 };
        assert!(nvidia(8.0).whisper_gpu() && nvidia(2.0).whisper_gpu() && !nvidia(1.0).whisper_gpu());
        assert!(!hw(Some(8.0), 16.0).whisper_gpu(), "only NVIDIA has a ready GPU build");
        assert!(!hw(None, 16.0).whisper_gpu());
    }

    #[test]
    fn the_real_manifest_has_the_two_optional_projectors_under_apache_and_the_base_plan_does_not_take_them() {
        let m = crate::ai::download::parse_manifest(include_str!("../../../../shared/ai/manifest.json")).unwrap();
        let root = Path::new("root");
        let targets: Vec<PathBuf> = ["mmproj-standard", "mmproj-light"]
            .iter()
            .map(|id| {
                let e = m.iter().find(|e| e.id == *id).expect("projector in the manifest");
                assert!((e.kind.as_str(), e.license.as_str(), e.sha256.len()) == ("vision", "Apache-2.0", 64), "{id}");
                target_of(root, e)
            })
            .collect();
        assert_ne!(targets[0], targets[1], "same upstream file name, different files");
        for choice in [ModelChoice::Standard, ModelChoice::Light] {
            assert!(plan(&m, choice, false).iter().all(|e| e.kind != "vision"), "the Screen help package is never part of the base install");
        }
    }

    #[test]
    fn settings_default_to_ctrl_alt_space_and_a_visible_phone_button() {
        let s = VoiceSettings::default();
        assert_eq!(s.hotkey, ["ctrl", "alt", "space"]);
        assert!(s.phone_button && !s.keep_loaded && !s.speak && s.model.is_none());
        let partial: VoiceSettings = serde_json::from_str(r#"{"speak":true}"#).unwrap();
        assert!(partial.speak && partial.phone_button);
    }

    #[derive(Default)]
    struct Kept(Mutex<Option<GpuLayers>>);
    impl LayerMemory for Kept {
        fn load(&self) -> Option<GpuLayers> {
            self.0.lock().unwrap().clone()
        }
        fn save(&self, value: Option<GpuLayers>) {
            *self.0.lock().unwrap() = value;
        }
    }

    #[tokio::test]
    async fn the_processor_is_remembered_for_one_model_on_one_card() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(Kept::default());
        let (svc, _) = service_with(dir.path(), hw(Some(8.0), 16.0), memory.clone());
        let model = dir.path().join("m.gguf");
        std::fs::write(&model, b"x").unwrap();
        assert_eq!(svc.gpu_layers(&model, None, false), gpu::ALL_LAYERS);
        let kept = GpuLayers { model: "m.gguf".into(), gpu: gpu_key(&hw(Some(8.0), 16.0)), layers: 0 };
        memory.save(Some(kept.clone()));
        assert_eq!(svc.gpu_layers(&model, None, false), 0);
        assert_eq!(memory.load(), Some(kept.clone()), "it stays");
        // Another model: forgotten.
        let other = dir.path().join("other.gguf");
        std::fs::write(&other, b"x").unwrap();
        assert_eq!(svc.gpu_layers(&other, None, false), gpu::ALL_LAYERS);
        assert_eq!(memory.load(), None);
        // Another card: forgotten.
        memory.save(Some(GpuLayers { gpu: "old card|2.0".into(), ..kept }));
        assert_eq!(svc.gpu_layers(&model, None, false), gpu::ALL_LAYERS);
        assert_eq!(memory.load(), None);
    }

    #[tokio::test]
    async fn the_light_model_is_offered_only_for_the_4b_model_on_a_pc_with_little_memory() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path(), hw(Some(8.0), 7.9));
        assert_eq!((svc.status().model, svc.status().suggest_light), (ModelChoice::Standard, true));
        svc.set(VoiceSetArgs { model: Some(ModelChoice::Light), ..Default::default() }).unwrap();
        assert!(!svc.status().suggest_light);
        let (roomy, _) = service(&dir.path().join("b"), hw(Some(8.0), 15.8));
        assert!(!roomy.status().suggest_light);
        assert!(roomy.redownload().await.is_err(), "nothing is damaged");
    }

    fn service(dir: &Path, hardware: HardwareInfo) -> (VoiceService, Arc<Mutex<Vec<VoiceStatus>>>) {
        service_with(dir, hardware, Arc::new(Kept::default()))
    }

    fn service_with(dir: &Path, hardware: HardwareInfo, layers: Arc<dyn LayerMemory>) -> (VoiceService, Arc<Mutex<Vec<VoiceStatus>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let svc = VoiceService::new(
            tokio::runtime::Handle::current(),
            VoiceDeps {
                root: dir.join("ai"),
                settings_path: dir.join("voice.json"),
                manifest: manifest(),
                hardware: Arc::new(Fixed(hardware)),
                mics: Arc::new(|| vec!["Mic A".to_string()]),
                downloader: Downloader::with_hosts(&["127.0.0.1"]),
                layers,
                emit: Arc::new(move |s| sink.lock().unwrap().push(s)),
            },
        )
        .unwrap();
        (svc, seen)
    }

    #[tokio::test]
    async fn a_fresh_install_is_off_and_reports_the_size_of_the_recommended_model() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path(), hw(Some(8.0), 16.0));
        let s = svc.status();
        assert_eq!(s.state, VoiceState::Off);
        assert_eq!(s.model, ModelChoice::Standard);
        assert!((s.download_gb - 10.0 / 1e9).abs() < 1e-12); // 1 + 2 + 4 + 3 bytes of test data
        assert_eq!(s.mics, ["Mic A"]);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["state"], "off");
        assert!(json.get("progress").is_none() && json.get("error").is_none(), "optional fields are omitted");
        assert!(json["hardware"].get("ramGb").is_some() && json.get("downloadGb").is_some() && json.get("phoneButton").is_some());
    }

    #[tokio::test]
    async fn set_saves_settings_validates_the_hotkey_and_emits() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, seen) = service(dir.path(), hw(None, 8.0));
        let s = svc.set(VoiceSetArgs { speak: Some(true), keep_loaded: Some(true), hotkey: Some(vec!["Ctrl".into(), "F9".into()]), ..Default::default() }).unwrap();
        assert!(s.speak && s.keep_loaded);
        assert_eq!(s.hotkey, ["ctrl", "f9"]);
        assert!(!seen.lock().unwrap().is_empty(), "voice-status was emitted");
        let saved: VoiceSettings = serde_json::from_str(&std::fs::read_to_string(dir.path().join("voice.json")).unwrap()).unwrap();
        assert_eq!(saved.hotkey, ["ctrl", "f9"]);
        assert!(svc.set(VoiceSetArgs { hotkey: Some(vec![]), ..Default::default() }).is_err());
        assert!(svc.set(VoiceSetArgs { hotkey: Some(vec!["a b".into()]), ..Default::default() }).is_err());
        // A second service on the same files reads the settings back.
        let (again, _) = service(dir.path(), hw(None, 8.0));
        assert!(again.status().speak);
    }

    #[tokio::test]
    async fn set_saves_every_field_of_the_flat_camel_case_payload() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path(), hw(None, 8.0));
        // What the settings window sends: flat camelCase keys.
        let args: VoiceSetArgs = serde_json::from_value(serde_json::json!({
            "model": "light", "hotkey": ["Ctrl", "F8"], "keepLoaded": true, "speak": true, "phoneButton": false, "mic": "Headset"
        }))
        .unwrap();
        svc.set(args).unwrap();
        let (again, _) = service(dir.path(), hw(None, 8.0));
        let s = again.status();
        assert_eq!(s.model, ModelChoice::Light);
        assert_eq!(s.hotkey, ["ctrl", "f8"]);
        assert!(s.keep_loaded && s.speak && !s.phone_button);
        assert_eq!(s.mic.as_deref(), Some("Headset"));
    }

    #[tokio::test]
    async fn the_model_choice_switches_the_planned_download_and_the_size() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path(), hw(Some(8.0), 16.0));
        let before = svc.status().download_gb;
        let s = svc.set(VoiceSetArgs { model: Some(ModelChoice::Light), ..Default::default() }).unwrap();
        assert_eq!(s.model, ModelChoice::Light);
        assert!(s.download_gb < before);
    }

    #[tokio::test]
    async fn remove_erases_the_ai_folder_and_returns_to_off() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path(), hw(None, 8.0));
        std::fs::create_dir_all(dir.path().join("ai").join("models")).unwrap();
        std::fs::write(dir.path().join("ai").join("models").join("x.gguf"), b"x").unwrap();
        let s = svc.remove().await;
        assert_eq!(s.state, VoiceState::Off);
        assert!(!dir.path().join("ai").exists());
    }

    #[tokio::test]
    async fn an_unreachable_host_ends_in_an_error_state_and_cancel_returns_to_off() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path(), hw(None, 8.0));
        // Nothing listens on 127.0.0.1:1: the download fails at once (or is cancelled); no real network is used.
        let started = svc.enable();
        assert_eq!(started.state, VoiceState::Downloading);
        svc.cancel();
        for _ in 0..100 {
            if svc.status().state != VoiceState::Downloading {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(matches!(svc.status().state, VoiceState::Off | VoiceState::Error));
    }
}
