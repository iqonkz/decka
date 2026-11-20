//! Limits for the dangerous system actions pressed on a phone (DATA_MODEL.md 11.4).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::model::SystemOp;

/// A phone must have been connected this long before it may shut the PC down.
pub const MIN_CONNECTED: Duration = Duration::from_secs(60);
/// At most one dangerous action in this time for the whole agent.
pub const MIN_GAP: Duration = Duration::from_secs(30);

/// Who presses (for the log) and how long this connection has been up.
pub struct PhoneCtx {
    pub device_name: String,
    pub connected_for: Duration,
}

/// `shutdown`, `restart`, `signout`, `hibernate`.
pub fn is_dangerous_system(op: SystemOp) -> bool {
    matches!(op, SystemOp::Shutdown | SystemOp::Restart | SystemOp::Signout | SystemOp::Hibernate)
}

/// Remembers when the last dangerous action was let through.
#[derive(Default)]
pub struct DangerGate {
    last: Mutex<Option<Instant>>,
}

impl DangerGate {
    pub fn check(&self, connected_for: Duration) -> Result<(), &'static str> {
        self.check_at(connected_for, Instant::now())
    }

    /// `Err` carries the text the phone shows. Only an action that passes starts the 30 s of waiting.
    pub fn check_at(&self, connected_for: Duration, now: Instant) -> Result<(), &'static str> {
        if connected_for < MIN_CONNECTED {
            return Err("Too soon after connecting");
        }
        let mut last = self.last.lock().unwrap();
        if last.is_some_and(|at| now.saturating_duration_since(at) < MIN_GAP) {
            return Err("Wait a little before trying again");
        }
        *last = Some(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shutdown_ten_seconds_after_connecting_is_refused() {
        let gate = DangerGate::default();
        assert_eq!(gate.check_at(Duration::from_secs(10), Instant::now()), Err("Too soon after connecting"));
        // The refusal does not start the waiting time: a legitimate press right after is fine.
        assert_eq!(gate.check_at(Duration::from_secs(61), Instant::now()), Ok(()));
    }

    #[test]
    fn a_second_one_five_seconds_later_is_refused_and_one_after_thirty_is_not() {
        let gate = DangerGate::default();
        let t0 = Instant::now();
        assert_eq!(gate.check_at(Duration::from_secs(120), t0), Ok(()));
        assert_eq!(gate.check_at(Duration::from_secs(125), t0 + Duration::from_secs(5)), Err("Wait a little before trying again"));
        // A refused press does not extend the wait.
        assert_eq!(gate.check_at(Duration::from_secs(150), t0 + Duration::from_secs(30)), Ok(()));
    }

    #[test]
    fn only_four_operations_are_dangerous() {
        for op in [SystemOp::Shutdown, SystemOp::Restart, SystemOp::Signout, SystemOp::Hibernate] {
            assert!(is_dangerous_system(op));
        }
        for op in [SystemOp::Lock, SystemOp::Sleep, SystemOp::TaskManager, SystemOp::DisplayOff] {
            assert!(!is_dangerous_system(op));
        }
    }
}
