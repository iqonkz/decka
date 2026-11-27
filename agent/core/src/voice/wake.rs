//! T32.1: the wake word "Decka". This file is the policy: the settings (off by default), the sensitivity, the keywords, when
//! the microphone may listen, the energy gate in front of the model and the lines the worker prints. The model itself runs
//! in a process of its own (`wake_engine`). Audio is never written anywhere: the gate keeps 0.3 s in memory and that is all.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::ai::download::ManifestEntry;

/// The files of `shared/ai/manifest.json` the wake word needs: the runtime archive first, then the model.
pub const IDS: [&str; 5] = ["wake-runtime", "wake-encoder", "wake-decoder", "wake-joiner", "wake-tokens"];
/// The names of the model files on disk, in the order of `IDS[1..]`.
pub const MODEL_FILES: [&str; 4] = ["encoder.onnx", "decoder.onnx", "joiner.onnx", "tokens.txt"];
/// Written next to the unpacked runtime: the folder is complete.
pub const RUNTIME_DONE: &str = ".done";
/// A detection this soon after the last one is the same word.
pub const COOLDOWN: Duration = Duration::from_millis(2500);
/// "Say Decka" in the settings: detections are only counted for this long.
pub const TEST_FOR: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sensitivity {
    Low,
    #[default]
    Medium,
    High,
}

impl Sensitivity {
    /// The probability the keyword must reach: larger is more careful.
    pub fn threshold(self) -> f32 {
        match self {
            Sensitivity::Low => 0.35,
            Sensitivity::Medium => 0.22,
            Sensitivity::High => 0.12,
        }
    }

    /// The bonus of the keyword during the search.
    pub fn score(self) -> f32 {
        match self {
            Sensitivity::Low => 1.0,
            Sensitivity::Medium => 1.5,
            Sensitivity::High => 2.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sensitivity::Low => "low",
            Sensitivity::Medium => "medium",
            Sensitivity::High => "high",
        }
    }

    pub fn parse(s: &str) -> Option<Sensitivity> {
        [Sensitivity::Low, Sensitivity::Medium, Sensitivity::High].into_iter().find(|x| x.name() == s)
    }
}

/// `wake.json` of the configuration folder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sensitivity: Sensitivity,
}

impl WakeSettings {
    pub fn load(dir: &Path) -> WakeSettings {
        let path = dir.join("wake.json");
        let Ok(text) = std::fs::read_to_string(&path) else { return WakeSettings::default() };
        serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!("wake.json is damaged, the wake word stays off: {e}");
            WakeSettings::default()
        })
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("wake.json"), text).map_err(|e| format!("cannot save the wake word settings: {e}"))
    }
}

/// The keywords in the form of the spotter: the pieces of the model (its SentencePiece vocabulary), then the score, the
/// threshold and the name. "Hey Decka" and "Decka" contain the same word, so they need no line of their own.
pub fn keywords(s: Sensitivity) -> String {
    ["▁де к ка", "▁д э ка", "▁д э к ка", "▁де ка"]
        .iter()
        .enumerate()
        .map(|(i, pieces)| {
            // The last spelling is also a Russian word ("deka"): it has to be clearer.
            let threshold = if i == 3 { (s.threshold() + 0.15).min(0.9) } else { s.threshold() };
            format!("{pieces} :{:.2} #{threshold:.2} @decka{i}\n", s.score())
        })
        .collect()
}

/// Where the files of the wake word are inside the folder of the AI package.
#[derive(Debug, Clone, PartialEq)]
pub struct Files {
    pub archive: PathBuf,
    pub runtime: PathBuf,
    pub model: PathBuf,
}

impl Files {
    /// The paths for the entries of the manifest; `None` if the manifest has no complete set.
    pub fn of(root: &Path, manifest: &[ManifestEntry]) -> Option<Files> {
        let short = |id: &str| manifest.iter().find(|e| e.id == id).map(|e| e.sha256[..8.min(e.sha256.len())].to_ascii_lowercase());
        for id in IDS {
            short(id)?;
        }
        let wake = root.join("wake");
        let runtime = short(IDS[0])?;
        Some(Files { archive: wake.join(format!("runtime-{runtime}.tar.bz2")), runtime: wake.join(format!("runtime-{runtime}")), model: wake.join(format!("model-{}", short(IDS[1])?)) })
    }

    pub fn installed(&self) -> bool {
        self.runtime.join(RUNTIME_DONE).is_file() && MODEL_FILES.iter().all(|f| self.model.join(f).is_file())
    }

    /// The library inside the unpacked archive (`<name of the release>/lib/`).
    pub fn library(&self) -> Option<PathBuf> {
        let name = "sherpa-onnx-c-api.dll";
        let mut folders = vec![self.runtime.clone()];
        folders.extend(std::fs::read_dir(&self.runtime).ok()?.flatten().map(|e| e.path()));
        folders.iter().flat_map(|f| [f.join(name), f.join("lib").join(name)]).find(|p| p.is_file())
    }
}

/// What decides whether the microphone listens now.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Conditions {
    pub enabled: bool,
    /// Voice control is on: there is something the word can start.
    pub voice_on: bool,
    pub installed: bool,
    pub recording_note: bool,
    pub fullscreen: bool,
    pub checking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WakeState {
    Off,
    Downloading,
    Starting,
    Listening,
    Paused,
    Error,
}

/// The state without the worker: `Starting` means "the worker should run".
pub fn wanted(c: Conditions) -> WakeState {
    if !c.enabled || !c.voice_on || !c.installed {
        WakeState::Off
    } else if c.recording_note || c.fullscreen || c.checking {
        WakeState::Paused
    } else {
        WakeState::Starting
    }
}

/// Two detections close together are one.
#[derive(Debug, Default)]
pub struct Cooldown(Option<Instant>);

impl Cooldown {
    pub fn accept(&mut self, now: Instant) -> bool {
        if self.0.is_some_and(|last| now.saturating_duration_since(last) < COOLDOWN) {
            return false;
        }
        self.0 = Some(now);
        true
    }
}

/// One line of the worker's standard output.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Ready,
    Wake,
    Failed(String),
}

pub fn parse_line(line: &str) -> Option<Line> {
    match line.trim() {
        "ready" => Some(Line::Ready),
        "wake" => Some(Line::Wake),
        other => other.strip_prefix("error ").map(|e| Line::Failed(e.to_string())),
    }
}

/// The level under which nothing is speech, whatever the room sounds like.
pub const GATE_MIN: f32 = 0.006;
/// Chunks (100 ms) kept before the gate opens, so that the start of the word is not lost.
pub const PREROLL: usize = 3;
/// Quiet chunks after which the gate closes.
pub const HANGOVER: u32 = 15;

/// What to do with one chunk.
#[derive(Debug, PartialEq)]
pub enum Gated {
    /// Quiet: the model sleeps.
    Skip,
    /// These chunks go to the model (the first time with the pre-roll).
    Feed(Vec<Vec<i16>>),
    /// The sound ended: forget what was heard.
    Close,
}

/// The energy gate: in a quiet room the model does not run at all.
#[derive(Debug, Default)]
pub struct Gate {
    preroll: VecDeque<Vec<i16>>,
    levels: VecDeque<f32>,
    open: bool,
    quiet: u32,
}

impl Gate {
    fn threshold(&self) -> f32 {
        let mut sorted: Vec<f32> = self.levels.iter().copied().collect();
        sorted.sort_by(f32::total_cmp);
        let floor = sorted.get(sorted.len() / 5).copied().unwrap_or(0.0);
        // Constant music raises the floor; the cap keeps speech over it audible to the model.
        (floor * 2.0).clamp(GATE_MIN, 0.03)
    }

    pub fn feed(&mut self, chunk: Vec<i16>, rms: f32) -> Gated {
        let loud = rms >= self.threshold();
        self.levels.push_back(rms);
        while self.levels.len() > 50 {
            self.levels.pop_front();
        }
        if self.open {
            self.quiet = if loud { 0 } else { self.quiet + 1 };
            if self.quiet >= HANGOVER {
                self.open = false;
                self.quiet = 0;
                return Gated::Close;
            }
            return Gated::Feed(vec![chunk]);
        }
        if loud {
            self.open = true;
            let mut out: Vec<Vec<i16>> = self.preroll.drain(..).collect();
            out.push(chunk);
            return Gated::Feed(out);
        }
        self.preroll.push_back(chunk);
        while self.preroll.len() > PREROLL {
            self.preroll.pop_front();
        }
        Gated::Skip
    }
}

/// The tray icon with a dot of a microphone in the lower right corner (RGBA, row by row).
pub fn dot_icon(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let r = (width.min(height) as f32 * 0.24).max(2.0);
    let (cx, cy) = (width as f32 - r - 0.5, height as f32 - r - 0.5);
    for y in 0..height {
        for x in 0..width {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let i = ((y * width + x) * 4) as usize;
            if i + 3 >= out.len() {
                continue;
            }
            // A white ring around a red dot reads on a light and on a dark taskbar.
            if d <= r * 0.72 {
                out[i..i + 4].copy_from_slice(&[0xef, 0x44, 0x44, 0xff]);
            } else if d <= r {
                out[i..i + 4].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wake_word_is_off_until_the_user_turns_it_on() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!WakeSettings::load(dir.path()).enabled);
        let on = WakeSettings { enabled: true, sensitivity: Sensitivity::High };
        on.save(dir.path()).unwrap();
        assert_eq!(WakeSettings::load(dir.path()), on);
        std::fs::write(dir.path().join("wake.json"), "{broken").unwrap();
        assert!(!WakeSettings::load(dir.path()).enabled);
    }

    #[test]
    fn a_higher_sensitivity_lowers_the_threshold_of_every_spelling() {
        let (low, high) = (Sensitivity::Low, Sensitivity::High);
        assert!(low.threshold() > high.threshold() && low.score() < high.score());
        let lines: Vec<String> = keywords(Sensitivity::Medium).lines().map(str::to_string).collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "▁де к ка :1.50 #0.22 @decka0");
        assert!(lines[3].contains("#0.37"), "the ordinary word is stricter: {}", lines[3]);
        assert_eq!(Sensitivity::parse("high"), Some(high));
        assert_eq!(Sensitivity::parse("max"), None);
    }

    #[test]
    fn the_microphone_listens_only_when_nothing_is_in_the_way() {
        let ok = Conditions { enabled: true, voice_on: true, installed: true, ..Default::default() };
        assert_eq!(wanted(ok), WakeState::Starting);
        for c in [Conditions { enabled: false, ..ok }, Conditions { voice_on: false, ..ok }, Conditions { installed: false, ..ok }] {
            assert_eq!(wanted(c), WakeState::Off);
        }
        for c in [Conditions { recording_note: true, ..ok }, Conditions { fullscreen: true, ..ok }, Conditions { checking: true, ..ok }] {
            assert_eq!(wanted(c), WakeState::Paused);
        }
    }

    #[test]
    fn the_gate_sleeps_in_silence_and_opens_with_the_start_of_the_word() {
        let mut gate = Gate::default();
        for i in 0..10 {
            assert_eq!(gate.feed(vec![i], 0.001), Gated::Skip);
        }
        assert_eq!(gate.feed(vec![100], 0.05), Gated::Feed(vec![vec![7], vec![8], vec![9], vec![100]]));
        for _ in 0..HANGOVER - 1 {
            assert!(matches!(gate.feed(vec![0], 0.001), Gated::Feed(_)));
        }
        assert_eq!(gate.feed(vec![0], 0.001), Gated::Close);
        assert_eq!(gate.feed(vec![0], 0.001), Gated::Skip);
    }

    #[test]
    fn the_worker_lines_and_the_cooldown() {
        assert_eq!(parse_line("ready\r"), Some(Line::Ready));
        assert_eq!(parse_line("wake"), Some(Line::Wake));
        assert_eq!(parse_line("error no library"), Some(Line::Failed("no library".into())));
        assert_eq!(parse_line("noise"), None);
        let (mut c, t) = (Cooldown::default(), Instant::now());
        assert!(c.accept(t));
        assert!(!c.accept(t + Duration::from_secs(1)));
        assert!(c.accept(t + COOLDOWN));
    }

    #[test]
    fn the_real_manifest_has_the_wake_files_under_apache_and_the_voice_package_does_not_take_them() {
        let m = crate::ai::download::parse_manifest(include_str!("../../../../shared/ai/manifest.json")).unwrap();
        for id in IDS {
            let e = m.iter().find(|e| e.id == id).expect(id);
            assert!(e.license == "Apache-2.0" && e.kind == "wake" && e.size > 0, "{id}");
        }
        let plan = crate::ai::service::plan(&m, crate::ai::service::ModelChoice::Standard, false, false);
        assert!(plan.iter().all(|e| e.kind != "wake"));
        let dir = tempfile::tempdir().unwrap();
        let files = Files::of(dir.path(), &m).unwrap();
        assert!(!files.installed());
        assert!(Files::of(dir.path(), &m[..3]).is_none());
    }

    #[test]
    fn the_dot_changes_only_the_lower_right_corner() {
        let icon = dot_icon(&vec![0u8; 32 * 32 * 4], 32, 32);
        assert_eq!(&icon[..4], &[0, 0, 0, 0]);
        let centre = (24 * 32 + 24) * 4;
        assert_eq!(&icon[centre..centre + 4], &[0xef, 0x44, 0x44, 0xff]);
    }
}
