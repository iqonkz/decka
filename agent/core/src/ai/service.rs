//! The voice package as the settings window sees it (IPC contract in shared/src/ipc.ts): install by one button, progress,
//! settings, removal. It owns the downloader and the supervisor; it never touches the network channel to the phone.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::download::{extract_archive, Downloader, ManifestEntry};
use super::hardware::{detect_hardware, HardwareInfo};
use super::supervisor::{SupState, Supervisor, SupervisorConfig};

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
pub fn plan<'a>(manifest: &'a [ManifestEntry], model: ModelChoice) -> Vec<&'a ManifestEntry> {
    let llm = match model {
        ModelChoice::Standard => "llm-standard",
        ModelChoice::Light => "llm-light",
    };
    ["llama-runtime", "whisper-runtime", llm, "stt"].iter().filter_map(|id| manifest.iter().find(|e| e.id == *id)).collect()
}

fn url_file_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or("file").split('?').next().unwrap_or("file").to_string()
}

fn is_archive(e: &ManifestEntry) -> bool {
    e.kind == "runtime"
}

/// Where a finished entry lives: a runtime folder or a model file.
fn target_of(root: &Path, e: &ManifestEntry) -> PathBuf {
    if is_archive(e) {
        root.join("runtime").join(format!("{}-{}", e.id, &e.sha256[..8.min(e.sha256.len())].to_ascii_lowercase()))
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
}

struct Inner {
    root: PathBuf,
    settings_path: PathBuf,
    manifest: Vec<ManifestEntry>,
    hardware: Arc<dyn HardwareProbe>,
    mics: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    downloader: Downloader,
    supervisor: Supervisor,
    emit: Emit,
    state: Mutex<State>,
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
                mics: deps.mics,
                downloader: deps.downloader,
                supervisor,
                emit: deps.emit,
                state: Mutex::new(State { settings, download: None, last_error: None }),
            }),
            handle,
        };
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

    fn model(&self) -> ModelChoice {
        let chosen = self.inner.state.lock().unwrap().settings.model;
        chosen.unwrap_or_else(|| recommend(&self.inner.hardware.probe()))
    }

    fn installed(&self) -> bool {
        let files = plan(&self.inner.manifest, self.model());
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
        let files = plan(&self.inner.manifest, self.model());
        let root = &self.inner.root;
        let llama_exe = find_file(&target_of(root, files[0]), "llama-server.exe")?;
        let whisper_exe = find_file(&target_of(root, files[1]), "whisper-server.exe")?;
        let keep_loaded = self.inner.state.lock().unwrap().settings.keep_loaded;
        Some(SupervisorConfig {
            llama_exe,
            llama_model: target_of(root, files[2]),
            whisper_exe,
            whisper_model: target_of(root, files[3]),
            keep_loaded,
        })
    }

    pub fn status(&self) -> VoiceStatus {
        let hw = self.inner.hardware.probe();
        let model = self.model();
        let installed = self.installed();
        let sup = self.inner.supervisor.state();
        let st = self.inner.state.lock().unwrap();
        let total: u64 = plan(&self.inner.manifest, model).iter().map(|e| e.size).sum();
        let (state, progress, error) = if let Some(d) = &st.download {
            (VoiceState::Downloading, Some(d.progress.clone()), None)
        } else if let Some(msg) = &st.last_error {
            (VoiceState::Error, None, Some(msg.clone()))
        } else if !installed {
            (VoiceState::Off, None, None)
        } else {
            match sup {
                SupState::Loading => (VoiceState::Loading, None, None),
                SupState::Stopped(msg) => (VoiceState::Error, None, Some(format!("Voice control stopped: {msg}"))),
                SupState::Idle | SupState::Ready => (VoiceState::Ready, None, None),
            }
        };
        VoiceStatus {
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
            error,
        }
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
        let files: Vec<ManifestEntry> = plan(&self.inner.manifest, model).into_iter().cloned().collect();
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
        let other = match keep {
            ModelChoice::Standard => "llm-light",
            ModelChoice::Light => "llm-standard",
        };
        if let Some(e) = self.inner.manifest.iter().find(|e| e.id == other) {
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
        let files = plan(&self.inner.manifest, self.model());
        files.len() == 4 && [0usize, 1, 3].iter().all(|i| is_present(&self.inner.root, files[*i]))
    }
}

#[cfg(test)]
mod tests {
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
        let ids = |c| plan(&m, c).iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(ModelChoice::Standard), ["llama-runtime", "whisper-runtime", "llm-standard", "stt"]);
        assert_eq!(ids(ModelChoice::Light), ["llama-runtime", "whisper-runtime", "llm-light", "stt"]);
    }

    #[test]
    fn settings_default_to_ctrl_alt_space_and_a_visible_phone_button() {
        let s = VoiceSettings::default();
        assert_eq!(s.hotkey, ["ctrl", "alt", "space"]);
        assert!(s.phone_button && !s.keep_loaded && !s.speak && s.model.is_none());
        let partial: VoiceSettings = serde_json::from_str(r#"{"speak":true}"#).unwrap();
        assert!(partial.speak && partial.phone_button);
    }

    fn service(dir: &Path, hardware: HardwareInfo) -> (VoiceService, Arc<Mutex<Vec<VoiceStatus>>>) {
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
