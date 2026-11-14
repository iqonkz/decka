pub mod api_auth;
pub mod http;
pub mod static_files;
pub mod ws;

use std::path::PathBuf;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue};
use axum::routing::{any, delete, get, post, put};
use axum::{middleware, Router};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::agent::Agent;

/// Request body limit for HTTP endpoints (PROTOCOL.md 1).
pub const MAX_BODY_BYTES: usize = 16384;

pub fn router(agent: Agent, client_dist: Option<PathBuf>) -> Router {
    let router = Router::new()
        .route("/ws", get(ws::handler))
        .route("/api/health", get(http::health))
        .route("/api/notify", post(http::notify))
        .route("/api/status", post(http::status_upsert))
        .route("/api/status/{id}", delete(http::status_delete))
        .route("/api/vars/{name}", put(http::var_put).delete(http::var_delete))
        .route("/api", any(http::api_not_found))
        .route("/api/{*rest}", any(http::api_not_found));
    static_files::attach(router, client_dist)
        .layer(middleware::from_fn(static_files::no_cache_html))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(agent)
}
