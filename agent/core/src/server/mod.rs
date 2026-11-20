pub mod api_auth;
pub mod http;
pub mod static_files;
pub mod ws;

use std::path::PathBuf;

use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, delete, get, post, put};
use axum::{middleware, Router};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::agent::Agent;
use crate::netpolicy::{ConnInfo, Verdict};

/// Request body limit for HTTP endpoints (PROTOCOL.md 1).
pub const MAX_BODY_BYTES: usize = 16384;

/// The network policy for plain HTTP (DATA_MODEL.md 11.2): on a public network the web client and `/api/*` do not exist.
/// `/ws` decides for itself, because it needs the handshake mode.
async fn network_policy(State(agent): State<Agent>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let kind = if path == "/ws" {
        return next.run(req).await;
    } else if path == "/api" || path.starts_with("/api/") {
        crate::netpolicy::Request::Api
    } else {
        crate::netpolicy::Request::Static
    };
    let Some(conn) = req.extensions().get::<ConnectInfo<ConnInfo>>().map(|c| c.0) else { return next.run(req).await };
    // The first lookup of a network asks Windows: keep it off the async threads.
    let verdict = tokio::task::spawn_blocking(move || agent.network_verdict(&conn, kind)).await.unwrap_or(Verdict::NotFound);
    match verdict {
        Verdict::Allow => next.run(req).await,
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn router(agent: Agent, client_dist: Option<PathBuf>) -> Router {
    let policy_state = agent.clone();
    let router = Router::new()
        .route("/ws", get(ws::handler))
        .route("/api/health", get(http::health))
        .route("/api/notify", post(http::notify))
        .route("/api/status", post(http::status_upsert))
        .route("/api/status/{id}", delete(http::status_delete))
        .route("/api/vars/{name}", put(http::var_put).delete(http::var_delete))
        .route("/api/cc/statusline", post(http::cc_statusline))
        .route("/api/cc/permission", post(http::cc_permission))
        .route("/api", any(http::api_not_found))
        .route("/api/{*rest}", any(http::api_not_found));
    static_files::attach(router, client_dist)
        .layer(middleware::from_fn(static_files::no_cache_html))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(policy_state, network_policy))
        .with_state(agent)
}
