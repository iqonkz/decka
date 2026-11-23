use std::sync::Arc;
use std::time::Duration;

use decka_core::actions::testing::RecordingRunner;
use decka_core::config::{default_actions, default_config, save_json_atomic};
use decka_core::model::{Action, SwitchConfig};
use decka_core::{Agent, AgentOptions};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Env {
    agent: Agent,
    ws: Ws,
    _dir: tempfile::TempDir,
}

async fn env(runner: Arc<RecordingRunner>) -> Env {
    let dir = tempfile::tempdir().unwrap();
    // The default actions plus one that toggles a user variable (`vol-mute` no longer toggles `audio.muted`: Decka sets it).
    let mut actions = default_actions();
    actions.actions.insert("flag".into(), Action::Hotkey { name: "Flag".into(), toggle_var: Some("test.flag".into()), keys: vec!["f13".into()] });
    save_json_atomic(&dir.path().join("actions.json"), &actions).unwrap();
    // The sound and player collectors would put `audio.*` / `media.*` variables next to the ones the tests look at.
    let mut config = default_config();
    config.collectors.audio = Some(SwitchConfig { enabled: false });
    config.collectors.media = Some(SwitchConfig { enabled: false });
    save_json_atomic(&dir.path().join("config.json"), &config).unwrap();
    let agent = Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()),
        config_dir: Some(dir.path().to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        runner,
        fixed_pair_code: Some("TESTCDE2".to_string()),
    })
    .await
    .unwrap();
    let port = agent.status().listening.unwrap().port;
    let mut ws = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap().0;
    let client = json!({ "name": "test", "app": "web", "version": "0.0.0" });
    send(&mut ws, json!({ "t": "pair", "v": 1, "code": "TESTCDE2", "client": client })).await;
    assert_eq!(recv(&mut ws).await["t"], "paired");
    assert_eq!(recv(&mut ws).await["t"], "welcome");
    Env { agent, ws, _dir: dir }
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

async fn recv(ws: &mut Ws) -> Value {
    loop {
        let msg = timeout(Duration::from_secs(2), ws.next()).await.expect("timed out").expect("closed").unwrap();
        if let Message::Text(t) = msg {
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            // The collectors publish on their own clock; their patches are not part of what these tests check.
            if v["t"] == "patch" && matches!(v["section"].as_str(), Some("sys" | "media" | "focus")) {
                continue;
            }
            return v;
        }
    }
}

/// Collects `n` messages (press results and patches may arrive in any order).
async fn recv_n(ws: &mut Ws, n: usize) -> Vec<Value> {
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(recv(ws).await);
    }
    out
}

#[tokio::test]
async fn press_of_a_defined_action_runs_it() {
    let runner = Arc::new(RecordingRunner::new());
    let mut e = env(runner.clone()).await;
    send(&mut e.ws, json!({ "t": "press", "id": 1, "actionId": "media-play" })).await;
    let res = recv(&mut e.ws).await;
    assert_eq!(res, json!({ "t": "press_result", "id": 1, "ok": true }));
    assert_eq!(runner.calls(), ["media-play"]);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn press_of_an_unknown_action_is_rejected() {
    let runner = Arc::new(RecordingRunner::new());
    let mut e = env(runner.clone()).await;
    send(&mut e.ws, json!({ "t": "press", "id": 2, "actionId": "foo" })).await;
    let res = recv(&mut e.ws).await;
    assert_eq!(res["ok"], false);
    assert_eq!(res["error"]["code"], "UNKNOWN_ACTION");
    assert_eq!(res["error"]["message"], "action 'foo' is not defined");
    assert!(runner.calls().is_empty());
    e.agent.shutdown().await;
}

/// S1: the protocol carries only an `actionId`; whatever else a client adds is ignored.
#[tokio::test]
async fn extra_fields_are_ignored() {
    let runner = Arc::new(RecordingRunner::new());
    let mut e = env(runner.clone()).await;

    send(&mut e.ws, json!({ "t": "press", "id": 1, "actionId": "media-play", "program": "calc.exe", "args": ["/c", "calc"] })).await;
    assert_eq!(recv(&mut e.ws).await, json!({ "t": "press_result", "id": 1, "ok": true }));
    assert_eq!(runner.calls(), ["media-play"]);

    send(&mut e.ws, json!({ "t": "press", "id": 2, "actionId": "nope", "program": "calc.exe", "args": ["/c", "calc"] })).await;
    let res = recv(&mut e.ws).await;
    assert_eq!(res["error"]["code"], "UNKNOWN_ACTION");
    assert_eq!(runner.calls(), ["media-play"], "the unknown action must not run anything");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn failing_runner_gives_action_failed_and_leaves_vars_alone() {
    let runner = Arc::new(RecordingRunner::failing("boom"));
    let mut e = env(runner).await;
    send(&mut e.ws, json!({ "t": "press", "id": 3, "actionId": "vol-mute" })).await;
    let res = recv(&mut e.ws).await;
    assert_eq!(res["error"]["code"], "ACTION_FAILED");
    assert_eq!(res["error"]["message"], "boom");
    assert_eq!(e.agent.store().get("vars"), json!({}));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn toggle_var_flips_and_broadcasts_each_time() {
    let runner = Arc::new(RecordingRunner::new());
    let mut e = env(runner).await;

    for (id, expected) in [(1, true), (2, false)] {
        send(&mut e.ws, json!({ "t": "press", "id": id, "actionId": "flag" })).await;
        let msgs = recv_n(&mut e.ws, 2).await;
        let patch = msgs.iter().find(|m| m["t"] == "patch").expect("vars patch");
        let result = msgs.iter().find(|m| m["t"] == "press_result").expect("press_result");
        assert_eq!(patch["section"], "vars");
        assert_eq!(patch["value"], json!({ "test.flag": expected }));
        assert_eq!(result["ok"], true);
    }
    assert_eq!(e.agent.store().get("vars"), json!({ "test.flag": false }));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn thirty_presses_hit_the_rate_limit_and_the_socket_survives() {
    let runner = Arc::new(RecordingRunner::new());
    let mut e = env(runner).await;
    for id in 1..=30 {
        send(&mut e.ws, json!({ "t": "press", "id": id, "actionId": "media-play" })).await;
    }
    let results = recv_n(&mut e.ws, 30).await;
    let limited = results.iter().filter(|r| r["error"]["code"] == "RATE_LIMITED").count();
    assert!(limited >= 1, "no RATE_LIMITED among {} results", results.len());
    assert!(results.iter().any(|r| r["ok"] == true));

    send(&mut e.ws, json!({ "t": "ping", "ts": 9 })).await;
    assert_eq!(recv(&mut e.ws).await, json!({ "t": "pong", "ts": 9 }));
    e.agent.shutdown().await;
}
