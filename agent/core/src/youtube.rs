//! YouTube: the live numbers of the signed-in channel (live, viewers, subscribers) and one key (a message in the chat of the live
//! stream). Only the agent talks to Google; the phone gets the `youtube` section. Sign-in is the device flow of Google for
//! limited-input devices: the owner makes an OAuth client of the type "TVs and Limited Input devices" in the Google Cloud console
//! (its client id and secret; Google requires the secret for this flow and does not treat it as confidential), the agent shows a
//! code to type on google.com/device. The secret and the tokens are kept encrypted with DPAPI (never in the section, never in a
//! log). The hosts are built into the code; parsing is separate from the network so that it is tested on saved answers.
//!
//! Quota: the free quota is 10 000 units a day and every list call costs 1, so the channel is looked at once a minute
//! (3 calls while live), about 4 300 units in a day of continuous live stream. A chat message costs 50.

use std::time::Duration;

use serde_json::Value;

use crate::model::YoutubeInfo;
pub use crate::twitch::{ApiError, DeviceCode, Poll, Tokens, TwitchLogin as YoutubeLogin};

pub const AUTH_URL: &str = "https://oauth2.googleapis.com";
pub const API_URL: &str = "https://www.googleapis.com/youtube/v3";
pub const SCOPE: &str = "https://www.googleapis.com/auth/youtube";
/// The numbers are asked for again this often.
pub const POLL_EVERY: Duration = Duration::from_secs(60);
const TIMEOUT: Duration = Duration::from_secs(10);

/// The channel: `(title, subscribers)`; `subscribers` is `None` when the channel hides the count.
pub fn parse_channel(body: &str) -> Option<(String, Option<u64>)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let item = v.get("items")?.as_array()?.first()?;
    let title: String = item.pointer("/snippet/title")?.as_str()?.chars().take(80).collect();
    let hidden = item.pointer("/statistics/hiddenSubscriberCount").and_then(Value::as_bool).unwrap_or(false);
    // YouTube writes its counts as strings.
    let count = item.pointer("/statistics/subscriberCount").and_then(|c| c.as_str().and_then(|s| s.parse().ok()).or_else(|| c.as_u64()));
    Some((title, if hidden { None } else { count }))
}

/// The live broadcast of the channel, as far as the section needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Broadcast {
    pub video_id: String,
    pub title: String,
    pub live_chat_id: Option<String>,
    pub started_at: Option<u64>,
}

/// `GET /liveBroadcasts?broadcastStatus=active`: an empty list is "not live".
pub fn parse_broadcast(body: &str) -> Option<Option<Broadcast>> {
    let v: Value = serde_json::from_str(body).ok()?;
    let Some(item) = v.get("items")?.as_array()?.first() else { return Some(None) };
    Some(Some(Broadcast {
        video_id: item.get("id")?.as_str()?.to_string(),
        title: item.pointer("/snippet/title").and_then(Value::as_str).unwrap_or("").chars().take(140).collect(),
        live_chat_id: item.pointer("/snippet/liveChatId").and_then(Value::as_str).map(str::to_string),
        started_at: item.pointer("/snippet/actualStartTime").and_then(Value::as_str).and_then(crate::ai::parse_timestamp_ms),
    }))
}

/// `GET /videos?part=liveStreamingDetails`: the viewers now (a count written as a string; absent while hidden or not started).
pub fn parse_viewers(body: &str) -> Option<u64> {
    let v: Value = serde_json::from_str(body).ok()?;
    let item = v.get("items")?.as_array()?.first()?;
    Some(item.pointer("/liveStreamingDetails/concurrentViewers").and_then(|c| c.as_str().and_then(|s| s.parse().ok()).or_else(|| c.as_u64())).unwrap_or(0))
}

/// The `message` of an error answer of Google, or the status. A used-up quota is said plainly.
pub fn error_text(status: u16, body: &str) -> String {
    let v: Option<Value> = serde_json::from_str(body).ok();
    if body.contains("quotaExceeded") || body.contains("rateLimitExceeded") {
        return "YouTube: the daily quota is used up".to_string();
    }
    let message = v.as_ref().and_then(|v| v.pointer("/error/message").or_else(|| v.get("error_description")).or_else(|| v.get("error"))).and_then(Value::as_str).map(str::to_string);
    match message {
        Some(m) if !m.is_empty() => format!("YouTube: {}", m.chars().take(200).collect::<String>()),
        _ => format!("YouTube: HTTP {status}"),
    }
}

/// The section for the phone.
pub fn build_info(channel: &str, subscribers: Option<u64>, live: Option<(Broadcast, u64)>) -> YoutubeInfo {
    YoutubeInfo {
        connected: true,
        channel: channel.chars().take(80).collect(),
        live: live.is_some(),
        viewers: live.as_ref().map_or(0, |(_, v)| *v),
        subscribers,
        title: live.as_ref().map(|(b, _)| b.title.clone()).filter(|t| !t.is_empty()),
        started_at: live.and_then(|(b, _)| b.started_at),
    }
}

fn parse_tokens(body: &str, old_refresh: Option<&str>) -> Option<Tokens> {
    let v: Value = serde_json::from_str(body).ok()?;
    let access = v.get("access_token")?.as_str()?;
    // A refresh answer carries no new refresh token: the old one stays.
    let refresh = v.get("refresh_token").and_then(Value::as_str).or(old_refresh)?;
    (!access.is_empty() && !refresh.is_empty()).then(|| Tokens { access: access.to_string(), refresh: refresh.to_string() })
}

fn form(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new()).extend_pairs(pairs).finish()
}

/// The two hosts of Google; the tests point them at a local server.
#[derive(Debug, Clone)]
pub struct Api {
    pub auth: String,
    pub api: String,
}

impl Api {
    pub fn real() -> Api {
        Api { auth: AUTH_URL.to_string(), api: API_URL.to_string() }
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

    /// Step 1 of the sign-in: a code for the owner to type on google.com/device.
    pub async fn device_start(&self, client_id: &str) -> Result<DeviceCode, String> {
        let body = form(&[("client_id", client_id), ("scope", SCOPE)]);
        let (status, text) = Self::send(reqwest::Method::POST, &format!("{}/device/code", self.auth), &[], Some((body, "application/x-www-form-urlencoded"))).await?;
        if status != 200 {
            return Err(error_text(status, &text));
        }
        let v: Value = serde_json::from_str(&text).map_err(|_| "YouTube: unexpected answer".to_string())?;
        let get = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Ok(DeviceCode {
            device_code: get("device_code").ok_or("YouTube: unexpected answer")?,
            user_code: get("user_code").ok_or("YouTube: unexpected answer")?,
            verification_uri: get("verification_url").unwrap_or_else(|| "https://www.google.com/device".to_string()),
            expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(1800),
            interval: v.get("interval").and_then(Value::as_u64).unwrap_or(5).max(1),
        })
    }

    /// Step 2: asked every `interval` seconds until the owner has typed the code.
    pub async fn device_poll(&self, client_id: &str, secret: &str, device_code: &str) -> Result<Poll, String> {
        let body = form(&[("client_id", client_id), ("client_secret", secret), ("device_code", device_code), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")]);
        let (status, text) = Self::send(reqwest::Method::POST, &format!("{}/token", self.auth), &[], Some((body, "application/x-www-form-urlencoded"))).await?;
        if status == 200 {
            return parse_tokens(&text, None).map(Poll::Done).ok_or_else(|| "YouTube: unexpected answer".to_string());
        }
        if text.contains("authorization_pending") {
            Ok(Poll::Pending)
        } else if text.contains("slow_down") {
            Ok(Poll::SlowDown)
        } else if text.contains("expired") {
            Err("The code has expired".to_string())
        } else if text.contains("access_denied") {
            Err("The sign-in was refused".to_string())
        } else {
            Err(error_text(status, &text))
        }
    }

    pub async fn refresh(&self, client_id: &str, secret: &str, refresh_token: &str) -> Result<Tokens, String> {
        let body = form(&[("client_id", client_id), ("client_secret", secret), ("grant_type", "refresh_token"), ("refresh_token", refresh_token)]);
        let (status, text) = Self::send(reqwest::Method::POST, &format!("{}/token", self.auth), &[], Some((body, "application/x-www-form-urlencoded"))).await?;
        if status != 200 {
            return Err(error_text(status, &text));
        }
        parse_tokens(&text, Some(refresh_token)).ok_or_else(|| "YouTube: unexpected answer".to_string())
    }

    async fn call(&self, method: reqwest::Method, path: &str, access: &str, body: Option<String>) -> Result<String, ApiError> {
        let auth = format!("Bearer {access}");
        let headers = [("Authorization", auth.as_str())];
        let (status, text) = Self::send(method, &format!("{}{path}", self.api), &headers, body.map(|b| (b, "application/json"))).await.map_err(ApiError::Failed)?;
        match status {
            200..=299 => Ok(text),
            401 => Err(ApiError::Unauthorized),
            _ => Err(ApiError::Failed(error_text(status, &text))),
        }
    }

    /// The channel of the signed-in account: `(title, subscribers)`.
    pub async fn channel(&self, access: &str) -> Result<(String, Option<u64>), ApiError> {
        let text = self.call(reqwest::Method::GET, &format!("/channels?{}", form(&[("part", "snippet,statistics"), ("mine", "true")])), access, None).await?;
        parse_channel(&text).ok_or_else(|| ApiError::Failed("YouTube: this account has no channel".to_string()))
    }

    pub async fn broadcast(&self, access: &str) -> Result<Option<Broadcast>, ApiError> {
        let text = self.call(reqwest::Method::GET, &format!("/liveBroadcasts?{}", form(&[("part", "snippet"), ("broadcastStatus", "active")])), access, None).await?;
        parse_broadcast(&text).ok_or_else(|| ApiError::Failed("YouTube: unexpected answer".to_string()))
    }

    pub async fn viewers(&self, access: &str, video_id: &str) -> Result<u64, ApiError> {
        let text = self.call(reqwest::Method::GET, &format!("/videos?{}", form(&[("part", "liveStreamingDetails"), ("id", video_id)])), access, None).await?;
        parse_viewers(&text).ok_or_else(|| ApiError::Failed("YouTube: unexpected answer".to_string()))
    }

    /// A message in the chat of the live stream.
    pub async fn chat(&self, access: &str, live_chat_id: &str, message: &str) -> Result<(), ApiError> {
        let body = serde_json::json!({ "snippet": { "liveChatId": live_chat_id, "type": "textMessageEvent", "textMessageDetails": { "messageText": message } } }).to_string();
        self.call(reqwest::Method::POST, &format!("/liveChat/messages?{}", form(&[("part", "snippet")])), access, Some(body)).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_answers_of_youtube() {
        let ch = r#"{"items":[{"snippet":{"title":"My channel"},"statistics":{"subscriberCount":"1234","hiddenSubscriberCount":false}}]}"#;
        assert_eq!(parse_channel(ch), Some(("My channel".into(), Some(1234))));
        let hidden = r#"{"items":[{"snippet":{"title":"X"},"statistics":{"subscriberCount":"5","hiddenSubscriberCount":true}}]}"#;
        assert_eq!(parse_channel(hidden), Some(("X".into(), None)));
        assert_eq!(parse_channel(r#"{"items":[]}"#), None);

        let live = r#"{"items":[{"id":"vid1","snippet":{"title":"Stream","liveChatId":"chat1","actualStartTime":"2026-11-17T10:00:00Z"}}]}"#;
        let b = parse_broadcast(live).unwrap().unwrap();
        assert_eq!((b.video_id.as_str(), b.title.as_str(), b.live_chat_id.as_deref(), b.started_at), ("vid1", "Stream", Some("chat1"), Some(1_794_909_600_000)));
        assert_eq!(parse_broadcast(r#"{"items":[]}"#), Some(None), "an empty list = not live");
        assert_eq!(parse_broadcast("nonsense"), None);

        assert_eq!(parse_viewers(r#"{"items":[{"liveStreamingDetails":{"concurrentViewers":"42"}}]}"#), Some(42));
        assert_eq!(parse_viewers(r#"{"items":[{"liveStreamingDetails":{}}]}"#), Some(0));
    }

    #[test]
    fn the_section_has_no_secret_and_a_hidden_count_is_absent() {
        let b = Broadcast { video_id: "v".into(), title: "T".into(), live_chat_id: None, started_at: Some(1) };
        let info = build_info("Me", None, Some((b, 9)));
        assert_eq!((info.live, info.viewers, info.subscribers, info.title.as_deref()), (true, 9, None, Some("T")));
        let json = serde_json::to_string(&build_info("Me", Some(3), None)).unwrap();
        assert!(!json.contains("token") && !json.contains("secret") && json.contains("\"subscribers\":3") && json.contains("\"live\":false"));
    }

    #[test]
    fn a_refresh_keeps_the_old_refresh_token() {
        assert_eq!(parse_tokens(r#"{"access_token":"a","expires_in":3600}"#, Some("old")), Some(Tokens { access: "a".into(), refresh: "old".into() }));
        assert_eq!(parse_tokens(r#"{"access_token":"a","refresh_token":"r"}"#, None), Some(Tokens { access: "a".into(), refresh: "r".into() }));
        assert_eq!(parse_tokens(r#"{"access_token":"a"}"#, None), None);
    }

    #[test]
    fn error_texts_are_plain() {
        assert_eq!(error_text(403, r#"{"error":{"code":403,"message":"x","errors":[{"reason":"quotaExceeded"}]}}"#), "YouTube: the daily quota is used up");
        assert_eq!(error_text(400, r#"{"error":{"message":"Bad things"}}"#), "YouTube: Bad things");
        assert_eq!(error_text(400, r#"{"error":"invalid_client","error_description":"Unauthorized"}"#), "YouTube: Unauthorized");
        assert_eq!(error_text(502, "<html>"), "YouTube: HTTP 502");
    }
}
