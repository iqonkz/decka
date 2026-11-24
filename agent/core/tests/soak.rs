//! T24.S: a phone that comes and goes. Pair once, then connect and disconnect 500 times (a polite close) and 50 times with an
//! abrupt drop of the socket: the table of connections, the subscriptions of the store and the tasks of the runtime return to
//! what they were before, so a phone on a bad Wi-Fi leaks nothing.

use std::sync::Arc;
use std::time::Duration;

use decka_core::actions::DryRunner;
use decka_core::{Agent, AgentOptions};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CODE: &str = "TESTCDE2";

async fn start() -> (Agent, u16, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = decka_core::config::default_config();
    config.collectors.focus = Some(decka_core::model::SwitchConfig { enabled: false });
    config.collectors.media = Some(decka_core::model::SwitchConfig { enabled: false });
    decka_core::config::save_json_atomic(&dir.path().join("config.json"), &config).unwrap();
    let agent = Agent::start(AgentOptions {
        bind_override: Some("127.0.0.1".to_string()),
        config_dir: Some(dir.path().to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        runner: Arc::new(DryRunner),
        fixed_pair_code: Some(CODE.to_string()),
    })
    .await
    .unwrap();
    let port = agent.status().listening.unwrap().port;
    (agent, port, dir)
}

async fn recv(ws: &mut Ws) -> Value {
    loop {
        let msg = timeout(Duration::from_secs(5), ws.next()).await.expect("timed out").expect("socket closed").unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}

fn client() -> Value {
    json!({ "name": "soak", "app": "web", "version": "0.0.0" })
}

async fn connect(port: u16) -> Ws {
    connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap().0
}

/// What has to return to the baseline: connections, subscribers of the store, living tasks of the runtime.
fn gauge(agent: &Agent) -> (usize, usize, usize) {
    (agent.status().clients.len(), agent.store().receiver_count(), tokio::runtime::Handle::current().metrics().num_alive_tasks())
}

/// Waits (at most 10 s) until the gauge is no higher than `base` in every part, then returns it.
async fn settle(agent: &Agent, base: (usize, usize, usize)) -> (usize, usize, usize) {
    let mut g = gauge(agent);
    for _ in 0..200 {
        g = gauge(agent);
        if g.0 == 0 && g.1 <= base.1 && g.2 <= base.2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    g
}

#[tokio::test]
async fn five_hundred_reconnects_and_fifty_abrupt_drops_leak_nothing() {
    let (agent, port, _dir) = start().await;
    let mut ws = connect(port).await;
    ws.send(Message::text(json!({ "t": "pair", "v": 1, "code": CODE, "client": client() }).to_string())).await.unwrap();
    let paired = recv(&mut ws).await;
    let token = paired["token"].as_str().unwrap().to_string();
    assert_eq!(recv(&mut ws).await["t"], "welcome");
    let _ = ws.close(None).await;
    drop(ws);
    // The baseline: the agent with its own tasks, nobody connected.
    let base = settle(&agent, (0, usize::MAX, usize::MAX)).await;
    assert_eq!(base.0, 0, "the paired phone left");

    for i in 0..550 {
        let mut ws = connect(port).await;
        ws.send(Message::text(json!({ "t": "hello", "v": 1, "token": token, "client": client() }).to_string())).await.unwrap();
        assert_eq!(recv(&mut ws).await["t"], "welcome", "connection {i}");
        if i % 11 == 0 {
            // Some traffic on the way: a press of an unknown action and a ping.
            ws.send(Message::text(json!({ "t": "ping", "ts": i }).to_string())).await.unwrap();
            assert_eq!(recv(&mut ws).await["t"], "pong");
        }
        if i < 500 {
            let _ = ws.close(None).await;
        }
        // The last 50 are dropped without a close frame: the socket just goes away.
        drop(ws);
    }

    let after = settle(&agent, base).await;
    assert_eq!(after.0, 0, "connections left in the table: {after:?} (baseline {base:?})");
    assert!(after.1 <= base.1, "subscribers of the store grew: {after:?} (baseline {base:?})");
    assert!(after.2 <= base.2, "tasks of the runtime grew: {after:?} (baseline {base:?})");
    // The paired device is still known, and the agent still answers a new phone.
    let mut ws = connect(port).await;
    ws.send(Message::text(json!({ "t": "hello", "v": 1, "token": token, "client": client() }).to_string())).await.unwrap();
    assert_eq!(recv(&mut ws).await["t"], "welcome");
    drop(ws);
    agent.shutdown().await;
}
