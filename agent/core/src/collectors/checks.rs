//! HTTP checks: `GET url`, redirects not followed, body not read (DATA_MODEL.md 4).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use reqwest::redirect::Policy;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::model::{HttpCheck, StatusState};
use crate::statuses::Statuses;

/// Running check tasks; dropping aborts them all.
pub struct ChecksCollector {
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for ChecksCollector {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

/// Milliseconds rounded to the nearest 10, so jitter does not rebroadcast every check.
fn round_ms(ms: u128) -> u128 {
    (ms + 5) / 10 * 10
}

/// Turns an HTTP answer into (state, text).
pub fn judge(code: u16, expect: Option<u32>, elapsed_ms: u128) -> (StatusState, String) {
    let ok = match expect {
        Some(e) => u32::from(code) == e,
        None => (200..400).contains(&code),
    };
    if ok {
        (StatusState::Ok, format!("{code} · {} ms", round_ms(elapsed_ms)))
    } else {
        (StatusState::Error, code.to_string())
    }
}

async fn check_once(client: &reqwest::Client, check: &HttpCheck) -> (StatusState, String) {
    let started = Instant::now();
    match client.get(&check.url).timeout(Duration::from_secs(check.timeout_sec.max(1) as u64)).send().await {
        Ok(response) => judge(response.status().as_u16(), check.expect_status, started.elapsed().as_millis()),
        Err(e) if e.is_timeout() => (StatusState::Error, "timeout".to_string()),
        Err(_) => (StatusState::Error, "unreachable".to_string()),
    }
}

impl ChecksCollector {
    /// Removes statuses of checks that no longer exist, publishes the rest as `unknown`, then runs
    /// every check immediately and then every `intervalSec`.
    pub fn start(statuses: Statuses, checks: Vec<HttpCheck>) -> ChecksCollector {
        let keep: HashSet<String> = checks.iter().map(|c| c.id.clone()).collect();
        statuses.retain_checks(&keep);
        if checks.is_empty() {
            return ChecksCollector { tasks: Vec::new() };
        }
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .user_agent(format!("decka/{}", crate::VERSION))
            .build()
            .expect("HTTP client builds");

        let mut tasks = Vec::new();
        for check in checks {
            statuses.set_check(&check.id, &check.label, check.group.as_deref(), StatusState::Unknown, None);
            let (client, statuses) = (client.clone(), statuses.clone());
            tasks.push(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(check.interval_sec.max(1) as u64));
                interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                loop {
                    interval.tick().await; // the first tick fires immediately
                    let (state, text) = check_once(&client, &check).await;
                    statuses.set_check(&check.id, &check.label, check.group.as_deref(), state, Some(text));
                }
            }));
        }
        ChecksCollector { tasks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_judged_by_the_expected_status() {
        assert_eq!(judge(200, None, 84), (StatusState::Ok, "200 · 80 ms".to_string()));
        assert_eq!(judge(204, None, 86), (StatusState::Ok, "204 · 90 ms".to_string()));
        assert_eq!(judge(301, None, 10), (StatusState::Ok, "301 · 10 ms".to_string()));
        assert_eq!(judge(404, None, 10), (StatusState::Error, "404".to_string()));
        assert_eq!(judge(500, None, 10), (StatusState::Error, "500".to_string()));
        assert_eq!(judge(404, Some(404), 10).0, StatusState::Ok);
        assert_eq!(judge(200, Some(204), 10).0, StatusState::Error);
    }
}
