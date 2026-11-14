use std::path::PathBuf;

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::Router;
use tower_http::services::{ServeDir, ServeFile};

use crate::agent::Agent;

async fn not_bundled() -> Response {
    (StatusCode::NOT_FOUND, "client not bundled").into_response()
}

/// Serves the built web client with an SPA fallback to index.html; 404 text if there is no client.
pub fn attach(router: Router<Agent>, client_dist: Option<PathBuf>) -> Router<Agent> {
    match client_dist.filter(|d| d.is_dir()) {
        Some(dist) => {
            let index = dist.join("index.html");
            router.fallback_service(ServeDir::new(dist).fallback(ServeFile::new(index)))
        }
        None => router.fallback(not_bundled),
    }
}

/// index.html (any HTML response) must never be cached, so a new client build is picked up.
pub async fn no_cache_html(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut res = next.run(req).await;
    let is_html = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if is_html {
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    res
}
