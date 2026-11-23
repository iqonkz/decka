//! The network policy end to end (DATA_MODEL.md 11.2). The category of the network is forced by the test; Windows is not asked.

use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use decka_core::actions::DryRunner;
use decka_core::netpolicy::Category;
use decka_core::secure;
use decka_core::{Agent, AgentOptions};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CODE: &str = "TESTCDE2";

struct Env {
    agent: Agent,
    port: u16,
    _dir: tempfile::TempDir,
}

async fn env(allow_public: Option<bool>, category: Category) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let mut config = decka_core::config::default_config();
    config.collectors.focus = Some(decka_core::model::SwitchConfig { enabled: false });
    config.collectors.media = Some(decka_core::model::SwitchConfig { enabled: false });
    config.network = allow_public.map(|allow_public| decka_core::model::NetworkConfig { allow_public });
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
    agent.networks().force(Some(category));
    let port = agent.status().listening.unwrap().port;
    Env { agent, port, _dir: dir }
}

async fn recv_text(ws: &mut Ws) -> Value {
    loop {
        let msg = timeout(Duration::from_secs(3), ws.next()).await.expect("timed out").expect("socket closed").unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}

async fn close_code(ws: &mut Ws) -> u16 {
    loop {
        let msg = timeout(Duration::from_secs(3), ws.next()).await.expect("timed out").expect("no close frame");
        if let Ok(Message::Close(Some(frame))) = msg {
            return u16::from(frame.code);
        }
    }
}

fn client() -> Value {
    json!({ "name": "test", "app": "web", "version": "0.0.0" })
}

async fn hello2(port: u16, mode: &str) -> (Ws, [u8; 16]) {
    let mut ws = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap().0;
    let cn: [u8; 16] = secure::random();
    let hello = json!({ "t": "hello2", "v": 2, "mode": mode, "cn": URL_SAFE_NO_PAD.encode(cn), "client": client() });
    ws.send(Message::text(hello.to_string())).await.unwrap();
    (ws, cn)
}

async fn status_of(port: u16, path: &str) -> u16 {
    reqwest::get(format!("http://127.0.0.1:{port}{path}")).await.unwrap().status().as_u16()
}

#[tokio::test]
async fn public_network_has_no_web_client_and_no_api() {
    let e = env(None, Category::Public).await;
    assert_eq!(status_of(e.port, "/").await, 404);
    assert_eq!(status_of(e.port, "/index.html").await, 404);
    assert_eq!(status_of(e.port, "/api/health").await, 404);
    e.agent.shutdown().await;

    // The same requests on a private network are answered.
    let e = env(None, Category::Private).await;
    assert_eq!(status_of(e.port, "/api/health").await, 200);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn public_network_refuses_the_typed_code_but_takes_the_qr() {
    let e = env(None, Category::Public).await;
    let (mut ws, _cn) = hello2(e.port, "pair_code").await;
    let err = recv_text(&mut ws).await;
    assert_eq!((err["code"].as_str(), err["message"].as_str()), (Some("BAD_MESSAGE"), Some("On a public network pair with the QR code or USB")));
    assert_eq!(close_code(&mut ws).await, 4401);

    let info = e.agent.start_pairing();
    let s_qr: [u8; 32] = URL_SAFE_NO_PAD.decode(info.url.split("&s=").nth(1).unwrap()).unwrap().try_into().unwrap();
    let (mut ws, cn) = hello2(e.port, "pair_qr").await;
    let challenge = recv_text(&mut ws).await;
    assert_eq!(challenge["t"], "challenge");
    let sn: [u8; 16] = URL_SAFE_NO_PAD.decode(challenge["sn"].as_str().unwrap()).unwrap().try_into().unwrap();
    let keys = secure::derive_keys(&s_qr, &cn, &sn);
    let mac = secure::mac(&keys.auth, secure::LABEL_CLIENT, &cn, &sn);
    ws.send(Message::text(json!({ "t": "auth", "mac": URL_SAFE_NO_PAD.encode(mac) }).to_string())).await.unwrap();
    assert_eq!(recv_text(&mut ws).await["t"], "auth_ok");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn public_network_allows_five_handshakes_a_minute() {
    let e = env(None, Category::Public).await;
    for _ in 0..5 {
        let (mut ws, _cn) = hello2(e.port, "pair_qr").await;
        assert_eq!(recv_text(&mut ws).await["t"], "challenge");
    }
    let (mut ws, _cn) = hello2(e.port, "pair_qr").await;
    assert_eq!(recv_text(&mut ws).await["code"], "RATE_LIMITED");
    assert_eq!(close_code(&mut ws).await, 4429);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn v1_is_refused_on_private_and_public_networks() {
    for category in [Category::Private, Category::Public] {
        let e = env(None, category).await;
        let mut ws = connect_async(format!("ws://127.0.0.1:{}/ws", e.port)).await.unwrap().0;
        ws.send(Message::text(json!({ "t": "hello", "v": 1, "token": "x".repeat(43), "client": client() }).to_string())).await.unwrap();
        let err = recv_text(&mut ws).await;
        assert_eq!((err["code"].as_str(), err["supported"].clone()), (Some("PROTOCOL_VERSION"), json!([2])), "{category:?}");
        assert_eq!(close_code(&mut ws).await, 4426);
        e.agent.shutdown().await;
    }
}

#[tokio::test]
async fn the_typed_code_works_on_a_private_network() {
    let e = env(None, Category::Private).await;
    let (mut ws, cn) = hello2(e.port, "pair_code").await;
    let challenge = recv_text(&mut ws).await;
    assert_eq!(challenge["t"], "challenge");
    let sn: [u8; 16] = URL_SAFE_NO_PAD.decode(challenge["sn"].as_str().unwrap()).unwrap().try_into().unwrap();
    let keys = secure::derive_keys(&secure::s_code(CODE, &cn, &sn), &cn, &sn);
    let mac = secure::mac(&keys.auth, secure::LABEL_CLIENT, &cn, &sn);
    ws.send(Message::text(json!({ "t": "auth", "mac": URL_SAFE_NO_PAD.encode(mac) }).to_string())).await.unwrap();
    assert_eq!(recv_text(&mut ws).await["t"], "auth_ok");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn allow_public_off_closes_public_connections_at_once() {
    let e = env(Some(false), Category::Public).await;
    // The TCP connection is accepted and closed: no HTTP answer, no WebSocket upgrade.
    assert!(reqwest::get(format!("http://127.0.0.1:{}/api/health", e.port)).await.is_err());
    assert!(connect_async(format!("ws://127.0.0.1:{}/ws", e.port)).await.is_err());

    // A private network is not affected by the switch.
    e.agent.networks().force(Some(Category::Private));
    assert_eq!(status_of(e.port, "/api/health").await, 200);
    e.agent.shutdown().await;
}
