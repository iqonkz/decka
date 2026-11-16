//! Runners for tests: public (no cfg) because integration tests in `tests/` need them.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::ActionRunner;
use crate::model::Action;

/// Records the ids of executed actions (with the time of each); `failing` always returns an error instead,
/// `failing_on` only for one action id, `slow` takes its time for every action.
pub struct RecordingRunner {
    calls: Mutex<Vec<(String, Instant)>>,
    error: Option<String>,
    fail_on: Option<(String, String)>,
    pause: Duration,
}

impl RecordingRunner {
    pub fn new() -> RecordingRunner {
        RecordingRunner { calls: Mutex::new(Vec::new()), error: None, fail_on: None, pause: Duration::ZERO }
    }

    pub fn failing(msg: &str) -> RecordingRunner {
        RecordingRunner { error: Some(msg.to_string()), ..RecordingRunner::new() }
    }

    /// Fails with `msg` only for the action `id`; everything else succeeds and is recorded.
    pub fn failing_on(id: &str, msg: &str) -> RecordingRunner {
        RecordingRunner { fail_on: Some((id.to_string(), msg.to_string())), ..RecordingRunner::new() }
    }

    /// Every action takes `pause` to run.
    pub fn slow(pause: Duration) -> RecordingRunner {
        RecordingRunner { pause, ..RecordingRunner::new() }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().map(|(id, _)| id.clone()).collect()
    }

    /// The ids with the moment each action started.
    pub fn timed_calls(&self) -> Vec<(String, Instant)> {
        self.calls.lock().unwrap().clone()
    }
}

impl Default for RecordingRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionRunner for RecordingRunner {
    fn run(&self, id: &str, _action: &Action) -> Result<(), String> {
        let started = Instant::now();
        if !self.pause.is_zero() {
            std::thread::sleep(self.pause);
        }
        if let Some(msg) = &self.error {
            return Err(msg.clone());
        }
        if let Some((bad, msg)) = &self.fail_on {
            if bad == id {
                return Err(msg.clone());
            }
        }
        self.calls.lock().unwrap().push((id.to_string(), started));
        Ok(())
    }
}
