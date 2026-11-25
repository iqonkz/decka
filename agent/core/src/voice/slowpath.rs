//! The slow path (TASKS.md "Медленный путь", T26.4): whatever goes through the vision model or the 9B model says so before it
//! starts - "Looking at the screen... ~N s" with a progress bar. N is the moving average of the last five such calls on this
//! PC; before the first measurement it is 10 s for the 4B model and 20 s for the 9B. Longer than twice N: "A little longer...".

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many measurements the average uses.
pub const KEEP: usize = 5;
pub const DEFAULT_4B_SEC: u32 = 10;
pub const DEFAULT_9B_SEC: u32 = 20;
const FILE: &str = "voice_slow.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tracker {
    /// Durations of the last calls, oldest first, in milliseconds.
    samples_ms: Vec<u64>,
}

impl Tracker {
    pub fn record(&mut self, ms: u64) {
        self.samples_ms.push(ms);
        if self.samples_ms.len() > KEEP {
            self.samples_ms.remove(0);
        }
    }

    /// The expected duration in whole seconds (at least 1).
    pub fn eta_sec(&self, big_model: bool) -> u32 {
        if self.samples_ms.is_empty() {
            return if big_model { DEFAULT_9B_SEC } else { DEFAULT_4B_SEC };
        }
        let avg = self.samples_ms.iter().sum::<u64>() / self.samples_ms.len() as u64;
        (avg as f64 / 1000.0).round().max(1.0) as u32
    }

    pub fn load(dir: &Path) -> Tracker {
        std::fs::read_to_string(dir.join(FILE)).ok().and_then(|t| serde_json::from_str::<Tracker>(&t).ok()).map(Tracker::sane).unwrap_or_default()
    }

    /// Best effort: the average is only a hint, a failed write loses nothing important.
    pub fn save(&self, dir: &Path) {
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(PathBuf::from(dir).join(FILE), text);
        }
    }

    /// A hand-edited file cannot make the bar absurd: at most `KEEP` samples, none above 10 minutes.
    fn sane(mut self) -> Tracker {
        self.samples_ms.retain(|ms| *ms <= 600_000);
        let extra = self.samples_ms.len().saturating_sub(KEEP);
        self.samples_ms.drain(..extra);
        self
    }
}

/// What the island shows while a slow call runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    #[serde(rename = "etaSec")]
    pub eta_sec: u32,
    #[serde(rename = "elapsedSec")]
    pub elapsed_sec: u32,
}

impl Progress {
    /// Longer than twice the estimate: the label becomes "A little longer...".
    pub fn overdue(&self) -> bool {
        self.elapsed_sec > self.eta_sec * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_the_first_measurement_the_defaults_are_10_and_20_seconds() {
        let t = Tracker::default();
        assert_eq!((t.eta_sec(false), t.eta_sec(true)), (10, 20));
    }

    #[test]
    fn the_estimate_is_the_average_of_the_last_five() {
        let mut t = Tracker::default();
        for ms in [100_000, 4_000, 6_000, 8_000, 10_000, 12_000] {
            t.record(ms);
        }
        assert_eq!(t.eta_sec(false), 8, "the first (oldest) one fell out: (4+6+8+10+12)/5");
        let mut quick = Tracker::default();
        quick.record(100);
        assert_eq!(quick.eta_sec(false), 1, "never zero");
    }

    #[test]
    fn it_is_overdue_after_twice_the_estimate() {
        assert!(!Progress { eta_sec: 10, elapsed_sec: 20 }.overdue());
        assert!(Progress { eta_sec: 10, elapsed_sec: 21 }.overdue());
    }

    #[test]
    fn the_file_round_trips_and_a_bad_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = Tracker::default();
        t.record(7_000);
        t.save(dir.path());
        assert_eq!(Tracker::load(dir.path()), t);
        std::fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert_eq!(Tracker::load(dir.path()), Tracker::default());
        std::fs::write(dir.path().join(FILE), r#"{"samples_ms":[1,2,3,4,5,6,7,999999999]}"#).unwrap();
        assert_eq!(Tracker::load(dir.path()).samples_ms, vec![3, 4, 5, 6, 7]);
    }
}
