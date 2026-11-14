//! Bearer-token authorization and per-IP limits for /api/* (PROTOCOL.md 5).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::Response;

use crate::agent::Agent;
use crate::auth::ct_eq;
use crate::proto::ErrorCode;
use crate::ratelimit::RateLimiter;

use super::http::error_response;

const REQUESTS_PER_MINUTE: u32 = 60;
const MAX_UNAUTHORIZED: u32 = 10;
const WINDOW: Duration = Duration::from_secs(60);

struct Failures {
    window_start: Instant,
    count: u32,
    blocked_until: Option<Instant>,
}

/// 60 authorized requests/min per IP; 10 answers of 401 within a minute block the IP (429) for 60 s.
pub struct ApiGuard {
    requests: RateLimiter,
    failures: Mutex<HashMap<String, Failures>>,
}

impl ApiGuard {
    pub fn new() -> ApiGuard {
        ApiGuard { requests: RateLimiter::new(REQUESTS_PER_MINUTE, WINDOW), failures: Mutex::new(HashMap::new()) }
    }

    pub fn authorize(&self, ip: &str, token: Option<&str>, expected: &str) -> Result<(), Response> {
        self.authorize_at(ip, token, expected, Instant::now())
    }

    fn authorize_at(&self, ip: &str, token: Option<&str>, expected: &str, now: Instant) -> Result<(), Response> {
        let too_many = || error_response(StatusCode::TOO_MANY_REQUESTS, ErrorCode::RateLimited, "too many requests");
        let mut failures = self.failures.lock().unwrap();
        failures.retain(|_, f| f.blocked_until.is_some_and(|b| b > now) || now.saturating_duration_since(f.window_start) < WINDOW);

        if failures.get(ip).and_then(|f| f.blocked_until).is_some_and(|b| b > now) {
            return Err(too_many());
        }
        let valid = token.is_some_and(|t| ct_eq(t.as_bytes(), expected.as_bytes()));
        if !valid {
            let f = failures.entry(ip.to_string()).or_insert(Failures { window_start: now, count: 0, blocked_until: None });
            f.count += 1;
            if f.count >= MAX_UNAUTHORIZED {
                f.blocked_until = Some(now + WINDOW);
            }
            return Err(error_response(StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized, "missing or invalid API token"));
        }
        drop(failures);
        if self.requests.check(ip) {
            Ok(())
        } else {
            Err(too_many())
        }
    }
}

impl Default for ApiGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Extractor: succeeds only for requests with a valid `Authorization: Bearer <apiToken>` header.
/// The token is never read from the query string.
pub struct ApiAuth;

impl FromRequestParts<Agent> for ApiAuth {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, agent: &Agent) -> Result<Self, Self::Rejection> {
        let ip = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip().to_string())
            .unwrap_or_default();
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, t)| t.trim());
        agent.api_guard().authorize(&ip, token, &agent.config().api_token)?;
        Ok(ApiAuth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_unauthorized_answers_block_the_ip_for_a_minute() {
        let guard = ApiGuard::new();
        let t0 = Instant::now();
        for _ in 0..10 {
            let r = guard.authorize_at("1.1.1.1", Some("bad"), "good", t0).unwrap_err();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }
        // Even a correct token is refused while blocked.
        let r = guard.authorize_at("1.1.1.1", Some("good"), "good", t0 + Duration::from_secs(30)).unwrap_err();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(guard.authorize_at("1.1.1.1", Some("good"), "good", t0 + Duration::from_secs(61)).is_ok());
        // Another IP is unaffected.
        assert!(guard.authorize_at("2.2.2.2", Some("good"), "good", t0).is_ok());
    }
}
