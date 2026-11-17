use axum::body::to_bytes;
use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use super::api_auth::ApiAuth;
use super::MAX_BODY_BYTES;
use crate::agent::{Agent, CcDenied};
use crate::model::VarValue;
use crate::proto::{ErrorBody, ErrorCode, HealthRes, HttpError, NotifyReq, NotifyRes, OkRes, StatusReq, VarReq};

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
/// A leading UTF-8 BOM is dropped (Windows PowerShell 5.1 writes files with one), PROTOCOL.md 5.
pub(super) async fn read_body(req: Request) -> Result<axum::body::Bytes, Response> {
    let body = to_bytes(req.into_body(), MAX_BODY_BYTES)
        .await
        .map_err(|_| error_response(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::PayloadTooLarge, "request body is too large"))?;
    Ok(if body.starts_with(&[0xEF, 0xBB, 0xBF]) { body.slice(3..) } else { body })
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

fn too_long_str(value: &str, max: usize) -> bool {
    value.chars().count() > max
}

/// `POST /api/status`: upsert by id (source is always "push").
pub async fn status_upsert(State(agent): State<Agent>, _auth: ApiAuth, req: Request) -> Response {
    let body = match read_body(req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let parsed: StatusReq = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return bad_request(&format!("invalid JSON: {e}")),
    };
    if !crate::validate::is_id(&parsed.id) {
        return bad_request("invalid status id");
    }
    if parsed.label.as_deref().is_some_and(|l| l.is_empty() || too_long_str(l, 60))
        || parsed.text.as_deref().is_some_and(|t| too_long_str(t, 80))
        || parsed.group.as_deref().is_some_and(|g| too_long_str(g, 40))
        || parsed.ttl_sec.is_some_and(|t| !(1..=86400).contains(&t))
    {
        return bad_request("a field is outside its allowed range");
    }
    match agent.statuses().upsert(parsed) {
        Ok(()) => Json(OkRes { ok: true }).into_response(),
        Err(message) => bad_request(&message),
    }
}

/// `DELETE /api/status/{id}`
pub async fn status_delete(State(agent): State<Agent>, _auth: ApiAuth, Path(id): Path<String>) -> Response {
    if agent.statuses().delete(&id) {
        Json(OkRes { ok: true }).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, ErrorCode::NotFound, "no such status")
    }
}

/// `PUT /api/vars/{name}`
pub async fn var_put(State(agent): State<Agent>, _auth: ApiAuth, Path(name): Path<String>, req: Request) -> Response {
    let body = match read_body(req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    if !crate::validate::is_var_name(&name) {
        return bad_request("invalid variable name");
    }
    if crate::validate::is_reserved_var(&name) {
        return bad_request("reserved variable");
    }
    let parsed: VarReq = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return bad_request(&format!("invalid JSON: {e}")),
    };
    if matches!(&parsed.value, VarValue::Str(s) if too_long_str(s, 200)) {
        return bad_request("string value is longer than 200 characters");
    }
    agent.store().update_vars(|vars| {
        vars.insert(name, parsed.value);
    });
    Json(OkRes { ok: true }).into_response()
}

/// Checks a call of `/api/cc/*`: from this PC only (404 for others, so the endpoints do not show), with the hook token (401).
fn cc_guard(agent: &Agent, req: &Request) -> Result<(), Response> {
    let ip = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip());
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, t)| t.trim());
    agent.cc_authorize(ip, token).map_err(|denied| match denied {
        CcDenied::NotFound => error_response(StatusCode::NOT_FOUND, ErrorCode::NotFound, "no such API endpoint"),
        CcDenied::Unauthorized => error_response(StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized, "missing or invalid hook token"),
        CcDenied::RateLimited => error_response(StatusCode::TOO_MANY_REQUESTS, ErrorCode::RateLimited, "too many requests"),
    })
}

async fn cc_body(req: Request) -> Result<axum::body::Bytes, Response> {
    to_bytes(req.into_body(), crate::ai::MAX_HOOK_BODY)
        .await
        .map_err(|_| error_response(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::PayloadTooLarge, "request body is too large"))
}

/// `POST /api/cc/statusline`: the status line for the terminal of Claude Code, as plain text.
pub async fn cc_statusline(State(agent): State<Agent>, req: Request) -> Response {
    if let Err(r) = cc_guard(&agent, &req) {
        return r;
    }
    let body = match cc_body(req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], agent.cc_statusline(&body)).into_response()
}

/// `POST /api/cc/permission`: held until the phone answers, the wait runs out, or Claude Code hangs up.
pub async fn cc_permission(State(agent): State<Agent>, req: Request) -> Response {
    if let Err(r) = cc_guard(&agent, &req) {
        return r;
    }
    // The hook of Codex calls the same address with `?agent=codex`.
    let codex = req.uri().query().is_some_and(|q| q.split('&').any(|p| p == "agent=codex"));
    let body = match cc_body(req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    match agent.cc_permission(&body, codex).await {
        Some(json) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
        None => StatusCode::OK.into_response(),
    }
}

/// `DELETE /api/vars/{name}`
pub async fn var_delete(State(agent): State<Agent>, _auth: ApiAuth, Path(name): Path<String>) -> Response {
    if crate::validate::is_reserved_var(&name) {
        return bad_request("reserved variable");
    }
    let mut existed = false;
    agent.store().update_vars(|vars| existed = vars.remove(&name).is_some());
    if existed {
        Json(OkRes { ok: true }).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, ErrorCode::NotFound, "no such variable")
    }
}