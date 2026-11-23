//! The plain-text call to the local model that the assistant functions of M22 share (dictation `clean`, `text_transform`,
//! the day brief). The tool call of a command (`llm.rs`) has its own schema; here the answer is free text, and every caller
//! checks it (`dictation::accept_clean`, `textops::accept_transform`, `brief::check_brief`) before it is used.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::dictation;
use crate::ai::supervisor::Supervisor;

/// `POST /v1/chat/completions` with `body`; the text of the first answer. Any failure is `None`; the status only goes to the
/// log, never the body (it may echo the text of the user).
pub async fn chat_text_at(port: u16, key: &str, body: &Value, timeout: Duration) -> Option<String> {
    let client = reqwest::Client::builder().timeout(timeout).build().ok()?;
    let answer = client
        .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
        .bearer_auth(key)
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(body).ok()?)
        .send()
        .await
        .map_err(|e| tracing::warn!("assistant model: {}", e.without_url()))
        .ok()?;
    if !answer.status().is_success() {
        tracing::warn!("assistant model: HTTP {}", answer.status().as_u16());
        return None;
    }
    dictation::parse_clean_completion(&answer.text().await.ok()?).filter(|t| !t.is_empty())
}

/// Starts the servers when they are not running, then `chat_text_at`.
pub async fn chat_text(sup: &Supervisor, body: &Value, timeout: Duration) -> Option<String> {
    let endpoints = sup.ensure_started().await.map_err(|e| tracing::warn!("assistant model: {e}")).ok()?;
    let answer = chat_text_at(endpoints.llama_port, &endpoints.llama_key, body, timeout).await;
    sup.touch();
    answer
}

/// The `clean` pass of a dictation: the text of the model when it passes the guard, else the raw text.
pub async fn clean_dictation(sup: &Supervisor, lang: &str, raw: &str, dictionary: &[String]) -> String {
    let body = dictation::clean_request_body(raw, lang, dictionary);
    let started = Instant::now();
    let answer = chat_text(sup, &body, dictation::CLEAN_TIMEOUT).await;
    match answer {
        Some(cleaned) => dictation::accept_clean(raw, &cleaned, started.elapsed()),
        None => raw.trim().to_string(),
    }
}
