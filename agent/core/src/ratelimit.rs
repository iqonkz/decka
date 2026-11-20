//! Fixed-window rate limiter keyed by string (IP address, socket id, ...).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    max: u32,
    window: Duration,
    entries: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(max: u32, window: Duration) -> RateLimiter {
        RateLimiter { max, window, entries: Mutex::new(HashMap::new()) }
    }

    /// Returns true if the call is within the limit (and counts it).
    pub fn check(&self, key: &str) -> bool {
        self.check_at(key, Instant::now())
    }

    /// True if the key already used up its allowance in the current window (does not count).
    pub fn exhausted(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, (start, _)| now.saturating_duration_since(*start) < self.window);
        entries.get(key).is_some_and(|(_, n)| *n >= self.max)
    }

    fn check_at(&self, key: &str, now: Instant) -> bool {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, (start, _)| now.saturating_duration_since(*start) < self.window);
        let entry = entries.entry(key.to_string()).or_insert((now, 0));
        if entry.1 < self.max {
            entry.1 += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_max_then_blocks_then_resets_after_window() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(limiter.check_at("ip", t0));
        assert!(limiter.check_at("ip", t0));
        assert!(limiter.check_at("ip", t0));
        assert!(!limiter.check_at("ip", t0 + Duration::from_secs(59)));
        assert!(limiter.check_at("ip", t0 + Duration::from_secs(60)));
    }

    #[test]
    fn keys_are_independent() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(limiter.check_at("a", t0));
        assert!(!limiter.check_at("a", t0));
        assert!(limiter.check_at("b", t0));
    }
}
