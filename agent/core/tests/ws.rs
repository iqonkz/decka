use std::sync::Arc;
use std::time::Duration;

use decka_core::actions::DryRunner;
use decka_core::{Agent, AgentOptions};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CODE: &str = "TESTCDE2";

struct Env {
    agent: Agent,
    port: u16,
    _dir: tempfile::TempDir,
}

async fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    // The OS collectors would publish `focus` / `media` at times of their own and move `rev` under the tests.
    let mut config = decka_core::config::default_config();
    config.collectors.focus = Some(decka_core::model::SwitchConfig { enabled: false });
    config.collectors.media = Some(decka_core::model::SwitchConfig { enabled: false });
    decka_core::config::save_json_atomic(&dir.path().join("config.json"), &config).unwrap();
    let agent = Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()),
        config_dir: Some(dir.path().to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        runner: Arc::new(DryRunner),
        fixed_pair_code: Some(CODE.to_string()),
    })
    .await
    .unwrap();
    let port = agent.status().listening.unwrap().port;
    Env { agent, port, _dir: dir }
}

async fn connect(port: u16) -> Ws {
    connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap().0
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

async fn send_raw(ws: &mut Ws, s: &str) {
    ws.send(Message::text(s.to_string())).await.unwrap();
}

/// Next text frame as JSON (fails after 2 s).
async fn recv(ws: &mut Ws) -> Value {
    loop {
        let msg = timeout(Duration::from_secs(2), ws.next()).await.expect("timed out").expect("socket closed").unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}

/// Reads until the close frame and returns its code.
async fn close_code(ws: &mut Ws) -> u16 {
    loop {
        let msg = timeout(Duration::from_secs(2), ws.next()).await.expect("timed out").expect("no close frame");
        if let Ok(Message::Close(Some(frame))) = msg {
            return u16::from(frame.code);
        }
    }
}

fn client() -> Value {
    json!({ "name": "test", "app": "web", "version": "0.0.0" })
}

/// Pairs a new device; returns the socket (after welcome) and the device token.
async fn pair(e: &Env) -> (Ws, String) {
    let mut ws = connect(e.port).await;
    send(&mut ws, json!({ "t": "pair", "v": 1, "code": CODE, "client": client() })).await;
    let paired = recv(&mut ws).await;
    assert_eq!(paired["t"], "paired");
    let token = paired["token"].as_str().unwrap().to_string();
    assert_eq!(recv(&mut ws).await["t"], "welcome");
    (ws, token)
}

#[tokio::test]
async fn pair_gives_paired_then_welcome() {
    let e = env().await;
    let mut ws = connect(e.port).await;
    send(&mut ws, json!({ "t": "pair", "v": 1, "code": CODE, "client": client() })).await;
    let paired = recv(&mut ws).await;
    assert_eq!(paired["t"], "paired");
    assert_eq!(paired["token"].as_str().unwrap().len(), 43);
    assert!(paired["deviceId"].as_str().unwrap().starts_with("d-"));
    let welcome = recv(&mut ws).await;
    assert_eq!(welcome["t"], "welcome");
    assert_eq!(welcome["deviceId"], paired["deviceId"]);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn hello_with_token_gives_welcome_with_five_sections() {
    let e = env().await;
    let (_first, token) = pair(&e).await;
    let mut ws = connect(e.port).await;
    send(&mut ws, json!({ "t": "hello", "v": 1, "token": token, "client": client() })).await;
    let welcome = recv(&mut ws).await;
    assert_eq!(welcome["t"], "welcome");
    let state = welcome["state"].as_object().unwrap();
    for s in ["layout", "sys", "statuses", "notifications", "vars"] {
        assert!(state.contains_key(s), "missing {s}");
    }
    e.agent.shutdown().await;
}

#[tokio::test]
async fn hello_with_garbage_token_is_unauthorized() {
    let e = env().await;
    let mut ws = connect(e.port).await;
    send(&mut ws, json!({ "t": "hello", "v": 1, "token": "x".repeat(43), "client": client() })).await;
    let err = recv(&mut ws).await;
    assert_eq!((err["t"].as_str(), err["code"].as_str(), err["fatal"].as_bool()), (Some("error"), Some("UNAUTHORIZED"), Some(true)));
    assert_eq!(close_code(&mut ws).await, 4401);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn unsupported_version_is_rejected_with_supported_list() {
    let e = env().await;
    let mut ws = connect(e.port).await;
    send(&mut ws, json!({ "t": "hello", "v": 2, "token": "x".repeat(43), "client": client() })).await;
    let err = recv(&mut ws).await;
    assert_eq!(err["code"], "PROTOCOL_VERSION");
    assert_eq!(err["supported"], json!([2]));
    assert_eq!(close_code(&mut ws).await, 4426);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn wrong_pair_code_is_rejected() {
    let e = env().await;
    let mut ws = connect(e.port).await;
    send(&mut ws, json!({ "t": "pair", "v": 1, "code": "WRONGCDE", "client": client() })).await;
    assert_eq!(recv(&mut ws).await["code"], "PAIR_CODE_INVALID");
    assert_eq!(close_code(&mut ws).await, 4401);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn first_frame_press_is_unauthorized() {
    let e = env().await;
    let mut ws = connect(e.port).await;
    send(&mut ws, json!({ "t": "press", "id": 1, "actionId": "lock" })).await;
    assert_eq!(recv(&mut ws).await["code"], "UNAUTHORIZED");
    assert_eq!(close_code(&mut ws).await, 4401);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn store_changes_arrive_as_patches_and_duplicates_do_not() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;

    assert!(e.agent.store().set("vars", json!({ "mic.muted": true })));
    // Background sources (live system state) patch other sections, and other `vars` keys, at any moment: wait for the patch
    // of OUR value; every other frame goes to stderr, so a failure shows what arrived (section and value).
    let ours = json!({ "mic.muted": true });
    let mut seen = Vec::new();
    // The last value of every section the phone was sent: a section sent twice with the same value is a bug of the dedupe.
    let mut last: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    let mut resent = |frame: &serde_json::Value| {
        if frame["t"] == "patch" {
            let section = frame["section"].as_str().unwrap_or_default().to_string();
            assert!(last.get(&section) != Some(&frame["value"]), "section {section} was re-sent unchanged: {}", frame["value"]);
            last.insert(section, frame["value"].clone());
        }
    };
    let patch = loop {
        let frame = recv(&mut ws).await;
        resent(&frame);
        if frame["t"] == "patch" && frame["section"] == "vars" && frame["value"] == ours {
            break frame;
        }
        eprintln!("unexpected frame before our patch: t={} section={} value={}", frame["t"], frame["section"], frame["value"]);
        seen.push(frame);
        assert!(seen.len() < 20, "too many frames before our patch: {seen:?}");
    };
    assert!(patch["rev"].as_u64().unwrap() >= 1);

    assert!(!e.agent.store().set("vars", json!({ "mic.muted": true })));
    // Only a re-send of the same value of the same section is a bug.
    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while let Ok(Some(Ok(frame))) = tokio::time::timeout_at(deadline, ws.next()).await {
        let text = frame.into_text().map(|t| t.to_string()).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        eprintln!("frame after the no-op set: section={} value={}", v["section"], v["value"]);
        resent(&v);
        assert!(!(v["t"] == "patch" && v["section"] == "vars" && v["value"] == ours), "duplicate vars patch after a no-op set: {text}");
    }
    e.agent.shutdown().await;
}

/// T24.2: `lyrics_offset` is data. Off by default, an unknown key and a wrong step change nothing, nothing is sent back, and
/// the socket stays usable; the switch is a config value that survives a read.
#[tokio::test]
async fn lyrics_offset_changes_nothing_it_was_not_given_a_key_for() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;
    assert!(!e.agent.lyrics_enabled(), "off by default");
    for msg in [
        json!({ "t": "lyrics_offset", "trackKey": "0123456789abcdef", "deltaMs": 500 }),
        json!({ "t": "lyrics_offset", "trackKey": "0123456789abcdef", "deltaMs": 1000 }),
        json!({ "t": "lyrics_offset", "trackKey": "../../x", "deltaMs": -500 }),
    ] {
        send(&mut ws, msg).await;
    }
    send(&mut ws, json!({ "t": "ping", "ts": 7 })).await;
    // Whatever background sources send, no lyrics patch and no error come back.
    loop {
        let frame = recv(&mut ws).await;
        assert_ne!(frame["section"], "lyrics", "{frame}");
        assert_ne!(frame["t"], "error", "{frame}");
        if frame["t"] == "pong" {
            break;
        }
    }
    assert_eq!(e.agent.store().get("lyrics"), serde_json::Value::Null);
    assert!(!e.agent.lyrics_offset("0123456789abcdef", 500));
    // The switch is kept in the configuration.
    assert_eq!(e.agent.set_lyrics_enabled(true), Ok(true));
    assert!(e.agent.lyrics_enabled());
    assert_eq!(e.agent.config().media.and_then(|m| m.lyrics).map(|l| l.enabled), Some(true));
    assert_eq!(e.agent.set_lyrics_enabled(false), Ok(false));
    assert!(!e.agent.lyrics_enabled());
    e.agent.shutdown().await;
}

/// T24.S: the messages of the phone that are data (not commands) take only the ids the agent issued; a malformed id is ignored
/// quietly (no error, no effect, nothing of it in the log) and the socket stays usable.
#[tokio::test]
async fn data_messages_ignore_malformed_and_unknown_ids() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;
    let long = "9".repeat(3000);
    let bad = [
        json!({ "t": "list_toggle", "listId": "../../x", "itemId": "0123456789abcdef" }),
        json!({ "t": "list_toggle", "listId": "0123456789abcdef", "itemId": "FEDCBA9876543210" }),
        json!({ "t": "list_toggle", "listId": long, "itemId": long }),
        json!({ "t": "notif_open", "notifId": "abc" }),
        json!({ "t": "notif_open", "notifId": long }),
        json!({ "t": "notif_open", "notifId": "99999" }),
        json!({ "t": "notif_dismiss", "notifId": "../1" }),
        json!({ "t": "perm_answer", "reqId": "zz", "decision": "allow" }),
        json!({ "t": "perm_answer", "reqId": "0123456789abcdef", "decision": "allow" }),
        json!({ "t": "lyrics_offset", "trackKey": long, "deltaMs": 500 }),
        json!({ "t": "lyrics_offset", "trackKey": "0123456789abcdef", "deltaMs": 99999 }),
        json!({ "t": "page_shown", "pageId": "../../x" }),
        json!({ "t": "page_shown", "pageId": "no-such-page" }),
    ];
    for msg in bad {
        // A frame over 4096 bytes is refused by the size limit of the socket, the others are ignored: either way no effect.
        let text = msg.to_string();
        if text.len() > 4000 {
            continue;
        }
        send(&mut ws, msg).await;
    }
    send(&mut ws, json!({ "t": "ping", "ts": 11 })).await;
    loop {
        let frame = recv(&mut ws).await;
        assert_ne!(frame["t"], "error", "{frame}");
        if frame["t"] == "pong" {
            break;
        }
    }
    assert_eq!(e.agent.voice().shown_page(), None, "an unknown page is not remembered");
    let page = e.agent.layout().pages[0].id.clone();
    send(&mut ws, json!({ "t": "page_shown", "pageId": page })).await;
    send(&mut ws, json!({ "t": "ping", "ts": 13 })).await;
    while recv(&mut ws).await["t"] != "pong" {}
    assert_eq!(e.agent.voice().shown_page(), Some(page));
    e.agent.shutdown().await;
}

/// T24.S: every message of the phone has a limit per second on the socket (20, as a press): list_toggle, notif_open,
/// notif_dismiss, perm_answer and lyrics_offset answer `RATE_LIMITED` (not fatal) beyond it.
#[tokio::test]
async fn data_messages_have_a_rate_limit() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;
    let kinds = [
        json!({ "t": "list_toggle", "listId": "0123456789abcdef", "itemId": "0123456789abcdef" }),
        json!({ "t": "notif_open", "notifId": "1" }),
        json!({ "t": "notif_dismiss", "notifId": "1" }),
        json!({ "t": "perm_answer", "reqId": "0123456789abcdef", "decision": "deny" }),
        json!({ "t": "lyrics_offset", "trackKey": "0123456789abcdef", "deltaMs": 500 }),
        json!({ "t": "page_shown", "pageId": "home" }),
    ];
    for msg in &kinds {
        // 60 in a row (one second window): the first 20 pass, the rest are refused.
        for _ in 0..60 {
            send(&mut ws, msg.clone()).await;
        }
    }
    send(&mut ws, json!({ "t": "ping", "ts": 12 })).await;
    let mut limited = 0;
    loop {
        let frame = recv(&mut ws).await;
        if frame["t"] == "error" {
            assert_eq!((frame["code"].as_str(), frame["fatal"].as_bool()), (Some("RATE_LIMITED"), Some(false)), "{frame}");
            limited += 1;
        }
        if frame["t"] == "pong" {
            break;
        }
    }
    // 6 kinds x 40 refused each (the window may roll once on a slow machine: at least half of them).
    assert!(limited >= 120, "{limited} refusals");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn ping_gets_pong_with_same_ts() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;
    send(&mut ws, json!({ "t": "ping", "ts": 1794650002000i64 })).await;
    assert_eq!(recv(&mut ws).await, json!({ "t": "pong", "ts": 1794650002000i64 }));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn revoking_a_device_closes_its_socket() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;
    let device = e.agent.list_devices().remove(0);
    assert!(device.connected);
    assert_eq!(e.agent.status().clients.len(), 1);

    e.agent.revoke_device(&device.id);
    let err = recv(&mut ws).await;
    assert_eq!((err["code"].as_str(), err["fatal"].as_bool()), (Some("DEVICE_REVOKED"), Some(true)));
    assert_eq!(close_code(&mut ws).await, 4403);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn garbage_after_auth_gives_bad_message_and_socket_survives() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;
    send_raw(&mut ws, "this is not json").await;
    let err = recv(&mut ws).await;
    assert_eq!((err["code"].as_str(), err["fatal"].as_bool()), (Some("BAD_MESSAGE"), Some(false)));
    send(&mut ws, json!({ "t": "ping", "ts": 5 })).await;
    // A `patch` of the live data may arrive first.
    let mut answer = recv(&mut ws).await;
    while answer["t"] == "patch" {
        answer = recv(&mut ws).await;
    }
    assert_eq!(answer["t"], "pong", "{answer}");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn notif_dismiss_removes_the_notification() {
    let e = env().await;
    let (mut ws, _) = pair(&e).await;
    let id = e.agent.push_notification(decka_core::proto::NotifyReq { title: "hi".into(), body: None, level: None, source: None });
    assert_eq!(recv(&mut ws).await["section"], "notifications");
    send(&mut ws, json!({ "t": "notif_dismiss", "notifId": id })).await;
    let patch = recv(&mut ws).await;
    assert_eq!(patch["section"], "notifications");
    assert_eq!(patch["value"], json!([]));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn foreign_origin_is_refused_before_upgrade() {
    let e = env().await;
    let mut req = format!("ws://127.0.0.1:{}/ws", e.port).into_client_request().unwrap();
    req.headers_mut().insert("Origin", "http://evil.example".parse().unwrap());
    match connect_async(req).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => assert_eq!(resp.status(), 403),
        other => panic!("expected HTTP 403, got {other:?}"),
    }

    // The agent's own origin and a missing Origin are allowed.
    let mut req = format!("ws://127.0.0.1:{}/ws", e.port).into_client_request().unwrap();
    req.headers_mut().insert("Origin", format!("http://127.0.0.1:{}", e.port).parse().unwrap());
    assert!(connect_async(req).await.is_ok());
    e.agent.shutdown().await;
}
