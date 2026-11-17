//! Twitch: the numbers of the signed-in channel (live, viewers, followers) and two keys (a clip, an announcement in the chat).
//! Only the agent talks to Twitch; the phone gets the `twitch` section. Sign-in is the device code flow of a public client: the
//! owner registers an application on dev.twitch.tv and enters its client id, the agent shows a code to type on twitch.tv/activate,
//! and no client secret exists anywhere. The tokens are kept encrypted with DPAPI (never in the section, never in a log).
//! The hosts are built into the code; parsing is separate from the network so that it is tested on saved answers.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::TwitchInfo;

pub const ID_URL: &str = "https://id.twitch.tv";
pub const API_URL: &str = "https://api.twitch.tv/helix";
/// Followers of the channel, a clip, an announcement: nothing more.
pub const SCOPES: &str = "moderator:read:followers clips:edit moderator:manage:announcements";
/// The numbers are asked for again this often.
pub const POLL_EVERY: Duration = Duration::from_secs(30);
const TIMEOUT: Duration = Duration::from_secs(10);

/// The access and the refresh token of the signed-in channel; stored together as one DPAPI blob.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
}

/// What the window shows while the owner types the code on twitch.tv/activate.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchLogin {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Poll {
    Pending,
    SlowDown,
    Done(Tokens),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    /// 401: the access token is no longer valid (it is refreshed once).
    Unauthorized,
    Failed(String),
}

/// A live stream, as far as the section needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamNow {
    pub viewers: u64,
    pub title: String,
    pub game: String,
    pub started_at: Option<u64>,
}

// ===== parsing =====

fn text(v: &Value, key: &str, max: usize) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").chars().take(max).collect()
}

/// `{"data":[{"id","login"}]}` of `GET /users`.
pub fn parse_user(body: &str) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let user = v.get("data")?.as_array()?.first()?;
    let (id, login) = (text(user, "id", 40), text(user, "login", 40));
    (!id.is_empty() && !login.is_empty()).then_some((id, login))
}

/// `GET /streams`: an empty `data` is "not live".
pub fn parse_stream(body: &str) -> Option<Option<StreamNow>> {
    let v: Value = serde_json::from_str(body).ok()?;
    let list = v.get("data")?.as_array()?;
    let Some(s) = list.iter().find(|s| s.get("type").and_then(Value::as_str).is_none_or(|t| t == "live")) else { return Some(None) };
    Some(Some(StreamNow {
        viewers: s.get("viewer_count").and_then(Value::as_u64).unwrap_or(0),
        title: text(s, "title", 140),
        game: text(s, "game_name", 80),
        started_at: s.get("started_at").and_then(Value::as_str).and_then(crate::ai::parse_timestamp_ms),
    }))
}

/// `GET /channels/followers`: the total.
pub fn parse_followers(body: &str) -> Option<u64> {
    serde_json::from_str::<Value>(body).ok()?.get("total")?.as_u64()
}

/// `POST /clips`: the address where the clip is edited (shown nowhere; its presence is the proof the clip exists).
pub fn parse_clip(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let clip = v.get("data")?.as_array()?.first()?;
    Some(text(clip, "edit_url", 300)).filter(|u| !u.is_empty()).or_else(|| Some(text(clip, "id", 80)).filter(|u| !u.is_empty()))
}

/// The `message` of an error answer of Twitch, or the status.
pub fn error_text(status: u16, body: &str) -> String {
    let message = serde_json::from_str::<Value>(body).ok().and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string));
    match message {
        Some(m) if !m.is_empty() => format!("Twitch: {}", m.chars().take(200).collect::<String>()),
        _ => format!("Twitch: HTTP {status}"),
    }
}

/// The section for the phone.
pub fn build_info(login: &str, stream: Option<StreamNow>, followers: u64) -> TwitchInfo {
    TwitchInfo {
        connected: true,
        login: login.chars().take(40).collect(),
        live: stream.is_some(),
        viewers: stream.as_ref().map_or(0, |s| s.viewers),
        followers,
        title: stream.as_ref().map(|s| s.title.clone()).filter(|t| !t.is_empty()),
        game: stream.as_ref().map(|s| s.game.clone()).filter(|g| !g.is_empty()),
        started_at: stream.and_then(|s| s.started_at),
    }
}

fn parse_tokens(body: &str) -> Option<Tokens> {
    let v: Value = serde_json::from_str(body).ok()?;
    let (access, refresh) = (v.get("access_token")?.as_str()?, v.get("refresh_token")?.as_str()?);
    (!access.is_empty() && !refresh.is_empty()).then(|| Tokens { access: access.to_string(), refresh: refresh.to_string() })
}

fn form(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new()).extend_pairs(pairs).finish()
}

// ===== network =====

/// The two hosts of Twitch; the tests point them at a local server.
#[derive(Debug, Clone)]
pub struct Api {
    pub id: String,
    pub api: String,
}

impl Api {
    pub fn real() -> Api {
        Api { id: ID_URL.to_string(), api: API_URL.to_string() }
    }

    async fn send(method: reqwest::Method, url: &str, headers: &[(&str, &str)], body: Option<(String, &str)>) -> Result<(u16, String), String> {
        let client = reqwest::Client::builder().timeout(TIMEOUT).user_agent(format!("decka-agent/{}", crate::VERSION)).build().map_err(|e| e.to_string())?;
        let mut req = client.request(method, url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        if let Some((text, content_type)) = body {
            req = req.header("Content-Type", content_type).body(text);
        }
        let response = req.send().await.map_err(|e| e.without_url().to_string())?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|e| e.without_url().to_string())?;
        Ok((status, text))
    }

    /// Step 1 of the sign-in: a code for the owner to type on twitch.tv/activate.
    pub async fn device_start(&self, client_id: &str) -> Result<DeviceCode, String> {
        let body = form(&[("client_id", client_id), ("scopes", SCOPES)]);
        let (status, text) = Self::send(reqwest::Method::POST, &format!("{}/oauth2/device", self.id), &[], Some((body, "application/x-www-form-urlencoded"))).await?;
        if status != 200 {
            return Err(error_text(status, &text));
        }
        let v: Value = serde_json::from_str(&text).map_err(|_| "Twitch: unexpected answer".to_string())?;
        let get = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Ok(DeviceCode {
            device_code: get("device_code").ok_or("Twitch: unexpected answer")?,
            user_code: get("user_code").ok_or("Twitch: unexpected answer")?,
            verification_uri: get("verification_uri").unwrap_or_else(|| "https://www.twitch.tv/activate".to_string()),
            expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(600),
            interval: v.get("interval").and_then(Value::as_u64).unwrap_or(5).max(1),
        })
    }

    /// Step 2: asked every `interval` seconds until the owner has typed the code.
    pub async fn device_poll(&self, client_id: &str, device_code: &str) -> Result<Poll, String> {
        let body = form(&[("client_id", client_id), ("scopes", SCOPES), ("device_code", device_code), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")]);
        let (status, text) = Self::send(reqwest::Method::POST, &format!("{}/oauth2/token", self.id), &[], Some((body, "application/x-www-form-urlencoded"))).await?;
        if status == 200 {
            return parse_tokens(&text).map(Poll::Done).ok_or_else(|| "Twitch: unexpected answer".to_string());
        }
        if text.contains("authorization_pending") {
            Ok(Poll::Pending)
        } else if text.contains("slow_down") {
            Ok(Poll::SlowDown)
        } else if text.contains("expired") || text.contains("invalid device code") {
            Err("The code has expired".to_string())
        } else if text.contains("access_denied") {
            Err("The sign-in was refused".to_string())
        } else {
            Err(error_text(status, &text))
        }
    }

    pub async fn refresh(&self, client_id: &str, refresh_token: &str) -> Result<Tokens, String> {
        let body = form(&[("client_id", client_id), ("grant_type", "refresh_token"), ("refresh_token", refresh_token)]);
        let (status, text) = Self::send(reqwest::Method::POST, &format!("{}/oauth2/token", self.id), &[], Some((body, "application/x-www-form-urlencoded"))).await?;
        if status != 200 {
            return Err(error_text(status, &text));
        }
        parse_tokens(&text).ok_or_else(|| "Twitch: unexpected answer".to_string())
    }

    async fn helix(&self, method: reqwest::Method, path: &str, client_id: &str, access: &str, body: Option<String>) -> Result<String, ApiError> {
        let auth = format!("Bearer {access}");
        let headers = [("Authorization", auth.as_str()), ("Client-Id", client_id)];
        let (status, text) = Self::send(method, &format!("{}{path}", self.api), &headers, body.map(|b| (b, "application/json"))).await.map_err(ApiError::Failed)?;
        match status {
            200..=299 => Ok(text),
            401 => Err(ApiError::Unauthorized),
            _ => Err(ApiError::Failed(error_text(status, &text))),
        }
    }

    /// The channel of the access token: `(id, login)`.
    pub async fn user(&self, client_id: &str, access: &str) -> Result<(String, String), ApiError> {
        let text = self.helix(reqwest::Method::GET, "/users", client_id, access, None).await?;
        parse_user(&text).ok_or_else(|| ApiError::Failed("Twitch: unexpected answer".to_string()))
    }

    pub async fn stream(&self, client_id: &str, access: &str, user_id: &str) -> Result<Option<StreamNow>, ApiError> {
        let path = format!("/streams?{}", form(&[("user_id", user_id)]));
        let text = self.helix(reqwest::Method::GET, &path, client_id, access, None).await?;
        parse_stream(&text).ok_or_else(|| ApiError::Failed("Twitch: unexpected answer".to_string()))
    }

    pub async fn followers(&self, client_id: &str, access: &str, user_id: &str) -> Result<u64, ApiError> {
        let path = format!("/channels/followers?{}", form(&[("broadcaster_id", user_id), ("first", "1")]));
        let text = self.helix(reqwest::Method::GET, &path, client_id, access, None).await?;
        parse_followers(&text).ok_or_else(|| ApiError::Failed("Twitch: unexpected answer".to_string()))
    }

    /// A clip of the last seconds of the live stream (Twitch refuses when the channel is not live).
    pub async fn clip(&self, client_id: &str, access: &str, user_id: &str) -> Result<String, ApiError> {
        let path = format!("/clips?{}", form(&[("broadcaster_id", user_id)]));
        let text = self.helix(reqwest::Method::POST, &path, client_id, access, None).await?;
        parse_clip(&text).ok_or_else(|| ApiError::Failed("Twitch: unexpected answer".to_string()))
    }

    /// An announcement in the chat of the channel (the owner is the moderator).
    pub async fn announce(&self, client_id: &str, access: &str, user_id: &str, message: &str) -> Result<(), ApiError> {
        let path = format!("/chat/announcements?{}", form(&[("broadcaster_id", user_id), ("moderator_id", user_id)]));
        let body = serde_json::json!({ "message": message }).to_string();
        self.helix(reqwest::Method::POST, &path, client_id, access, Some(body)).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_answers_of_twitch() {
        assert_eq!(parse_user(r#"{"data":[{"id":"141981764","login":"twitchdev","x":1}]}"#), Some(("141981764".into(), "twitchdev".into())));
        assert_eq!(parse_user(r#"{"data":[]}"#), None);
        assert_eq!(parse_followers(r#"{"total":68,"data":[],"pagination":{}}"#), Some(68));
        assert_eq!(parse_followers(r#"{"data":[]}"#), None);
        assert_eq!(parse_clip(r#"{"data":[{"id":"Ab","edit_url":"https://clips.twitch.tv/Ab/edit"}]}"#).as_deref(), Some("https://clips.twitch.tv/Ab/edit"));
        assert_eq!(parse_clip(r#"{"data":[]}"#), None);

        let live = r#"{"data":[{"type":"live","viewer_count":57,"title":"Speedrun","game_name":"Celeste","started_at":"2026-11-17T10:00:00Z"}]}"#;
        let s = parse_stream(live).unwrap().unwrap();
        assert_eq!((s.viewers, s.title.as_str(), s.game.as_str(), s.started_at), (57, "Speedrun", "Celeste", Some(1_794_909_600_000)));
        assert_eq!(parse_stream(r#"{"data":[]}"#), Some(None), "empty data = not live");
        assert_eq!(parse_stream("nonsense"), None);
    }

    #[test]
    fn the_section_has_no_secret_and_not_live_is_zero_viewers() {
        let live = build_info("me", parse_stream(r#"{"data":[{"viewer_count":5,"title":"T","game_name":"","started_at":"2026-11-17T10:00:00Z"}]}"#).unwrap(), 12);
        assert_eq!((live.live, live.viewers, live.followers, live.title.as_deref(), live.game), (true, 5, 12, Some("T"), None));
        let off = build_info("me", None, 12);
        assert_eq!((off.live, off.viewers, off.started_at), (false, 0, None));
        let json = serde_json::to_string(&off).unwrap();
        assert!(!json.contains("token") && json.contains("\"connected\":true"));
    }

    #[test]
    fn error_texts_use_the_message_of_twitch() {
        assert_eq!(error_text(400, r#"{"error":"Bad Request","status":400,"message":"Missing scope"}"#), "Twitch: Missing scope");
        assert_eq!(error_text(502, "<html>"), "Twitch: HTTP 502");
    }

    #[test]
    fn forms_are_encoded() {
        assert_eq!(form(&[("scopes", "a:b c")]), "scopes=a%3Ab+c");
    }
}
