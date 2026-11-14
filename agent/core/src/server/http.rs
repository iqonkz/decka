use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use super::api_auth::ApiAuth;
use super::MAX_BODY_BYTES;
use crate::agent::Agent;
use crate::proto::{ErrorBody, ErrorCode, HealthRes, HttpError, NotifyReq, NotifyRes};

const MAX_TITLE: usize = 120;
const MAX_BODY: usize = 500;
const MAX_SOURCE: usize = 40;

pub async fn health() -> Json<HealthRes> {
    Json(HealthRes { name: "decka".to_string(), version: crate::VERSION.to_string(), protocol: crate::PROTOCOL_VERSION })
}

pub fn error_response(status: StatusCode, code: ErrorCode, message: &str) -> Response {
    (status, Json(HttpError { error: ErrorBody { code, message: message.to_string() } })).into_response()
}

fn bad_request(message: &str) -> Response {
    error_response(StatusCode::BAD_REQUEST, ErrorCode::BadMessage, message)
}

/// Any unknown /api/* path.
pub async fn api_not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, ErrorCode::NotFound, "no such API endpoint")
}

/// Reads the body with the 16 KiB limit; an oversized body becomes 413 with an HttpError body.
pub(super) async fn read_body(req: Request) -> Result<axum::body::Bytes, Response> {
    to_bytes(req.into_body(), MAX_BODY_BYTES)
        .await
        .map_err(|_| error_response(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::PayloadTooLarge, "request body is too large"))
}

fn is_json(req: &Request) -> bool {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim_start().to_ascii_lowercase().starts_with("application/json"))
}

fn too_long(value: &Option<String>, max: usize) -> bool {
    value.as_deref().is_some_and(|v| v.chars().count() > max)
}

/// `POST /api/notify`: JSON `NotifyReq`, or plain text (the trimmed body, cut to 120 chars, becomes the title).
pub async fn notify(State(agent): State<Agent>, _auth: ApiAuth, req: Request) -> Response {
    let json = is_json(&req);
    let body = match read_body(req).await {
        Ok(b) => b,
        Err(r) => return r,
    };

    let notify_req = if json {
        let parsed: NotifyReq = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(e) => return bad_request(&format!("invalid JSON: {e}")),
        };
        if parsed.title.is_empty() {
            return bad_request("title must not be empty");
        }
        if parsed.title.chars().count() > MAX_TITLE || too_long(&parsed.body, MAX_BODY) || too_long(&parsed.source, MAX_SOURCE) {
            return bad_request("a field exceeds its length limit");
        }
        parsed
    } else {
        let Ok(text) = std::str::from_utf8(&body) else { return bad_request("body is not valid UTF-8") };
        let text = text.trim();
        if text.is_empty() {
            return bad_request("body is empty");
        }
        NotifyReq { title: text.chars().take(MAX_TITLE).collect(), body: None, level: None, source: None }
    };

    let id = agent.push_notification(notify_req);
    (StatusCode::ACCEPTED, Json(NotifyRes { id })).into_response()
}
