//! Runners for tests: public (no cfg) because integration tests in `tests/` need them.

use std::sync::Mutex;

use super::ActionRunner;
use crate::model::Action;

/// Records the ids of executed actions; `failing` always returns an error instead.
pub struct RecordingRunner {
    calls: Mutex<Vec<String>>,
    error: Option<String>,
}

impl RecordingRunner {
    pub fn new() -> RecordingRunner {
        RecordingRunner { calls: Mutex::new(Vec::new()), error: None }
    }

    pub fn failing(msg: &str) -> RecordingRunner {
        RecordingRunner { calls: Mutex::new(Vec::new()), error: Some(msg.to_string()) }
    }

    pub fn calls(&self) -> Vec<String> {
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
        if let Some(msg) = &self.error {
            return Err(msg.clone());
        }
        self.calls.lock().unwrap().push(id.to_string());
        Ok(())
    }
}
