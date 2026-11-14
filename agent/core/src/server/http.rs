use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::proto::{ErrorBody, ErrorCode, HealthRes, HttpError};

pub async fn health() -> Json<HealthRes> {
    Json(HealthRes { name: "decka".to_string(), version: crate::VERSION.to_string(), protocol: crate::PROTOCOL_VERSION })
}

pub fn error_response(status: StatusCode, code: ErrorCode, message: &str) -> Response {
    (status, Json(HttpError { error: ErrorBody { code, message: message.to_string() } })).into_response()
}

/// Any unknown /api/* path.
pub async fn api_not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, ErrorCode::NotFound, "no such API endpoint")
}
