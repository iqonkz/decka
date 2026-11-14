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
    let agent = Agent::start(AgentOptions {
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
    assert_eq!(err["supported"], json!([1]));
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
    let patch = recv(&mut ws).await;
    assert_eq!((patch["t"].as_str(), patch["section"].as_str()), (Some("patch"), Some("vars")));
    assert!(patch["rev"].as_u64().unwrap() >= 1);
    assert_eq!(patch["value"], json!({ "mic.muted": true }));

    assert!(!e.agent.store().set("vars", json!({ "mic.muted": true })));
    assert!(timeout(Duration::from_millis(300), ws.next()).await.is_err(), "unexpected frame after a no-op set");
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
    assert_eq!(recv(&mut ws).await["t"], "pong");
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
