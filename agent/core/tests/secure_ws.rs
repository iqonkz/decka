//! Protocol v2 over a real socket (PROTOCOL.md 10): handshake, encrypted frames, refusals.

use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use decka_core::actions::DryRunner;
use decka_core::secure::{self, Opener, Sealer};
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

async fn env_in(dir: tempfile::TempDir) -> Env {
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

async fn env() -> Env {
    env_in(tempfile::tempdir().unwrap()).await
}

fn client() -> Value {
    json!({ "name": "test", "app": "web", "version": "0.0.0" })
}

/// An encrypted connection after a successful handshake.
struct Secure {
    ws: Ws,
    tx: Sealer,
    rx: Opener,
}

impl Secure {
    async fn send(&mut self, v: Value) {
        let frame = self.tx.seal(v.to_string().as_bytes());
        self.ws.send(Message::Binary(frame.into())).await.unwrap();
    }

    async fn send_frame(&mut self, frame: Vec<u8>) {
        self.ws.send(Message::Binary(frame.into())).await.unwrap();
    }

    async fn recv(&mut self) -> Value {
        loop {
            let msg = timeout(Duration::from_secs(3), self.ws.next()).await.expect("timed out").expect("socket closed").unwrap();
            if let Message::Binary(b) = msg {
                let plain = self.rx.open(&b).expect("the agent frame opens");
                return serde_json::from_slice(&plain).unwrap();
            }
        }
    }
}

enum Proof {
    Device(String, [u8; 32]),
    PairQr([u8; 32]),
    PairCode(String),
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

/// Starts the handshake and stops after `challenge`.
async fn hello2(host: &str, port: u16, mode: &str, device_id: Option<&str>) -> (Ws, [u8; 16], [u8; 16]) {
    let mut ws = connect_async(format!("ws://{host}:{port}/ws")).await.unwrap().0;
    let cn: [u8; 16] = secure::random();
    let mut hello = json!({ "t": "hello2", "v": 2, "mode": mode, "cn": URL_SAFE_NO_PAD.encode(cn), "client": client() });
    if let Some(id) = device_id {
        hello["deviceId"] = json!(id);
    }
    ws.send(Message::text(hello.to_string())).await.unwrap();
    let challenge = recv_text(&mut ws).await;
    assert_eq!(challenge["t"], "challenge");
    let sn: [u8; 16] = URL_SAFE_NO_PAD.decode(challenge["sn"].as_str().unwrap()).unwrap().try_into().unwrap();
    (ws, cn, sn)
}

/// A full handshake as the phone does it.
async fn handshake(port: u16, proof: Proof) -> Secure {
    let (mode, device_id) = match &proof {
        Proof::Device(id, _) => ("device", Some(id.as_str())),
        Proof::PairQr(_) => ("pair_qr", None),
        Proof::PairCode(_) => ("pair_code", None),
    };
    let (mut ws, cn, sn) = hello2("127.0.0.1", port, mode, device_id).await;
    let ikm: [u8; 32] = match &proof {
        Proof::Device(_, k) => *k,
        Proof::PairQr(s) => *s,
        Proof::PairCode(code) => secure::s_code(code, &cn, &sn),
    };
    let keys = secure::derive_keys(&ikm, &cn, &sn);
    let mac = secure::mac(&keys.auth, secure::LABEL_CLIENT, &cn, &sn);
    ws.send(Message::text(json!({ "t": "auth", "mac": URL_SAFE_NO_PAD.encode(mac) }).to_string())).await.unwrap();
    let ok = recv_text(&mut ws).await;
    assert_eq!(ok["t"], "auth_ok", "{ok}");
    let want = secure::mac(&keys.auth, secure::LABEL_SERVER, &cn, &sn);
    assert_eq!(ok["mac"].as_str().unwrap(), URL_SAFE_NO_PAD.encode(want));
    Secure { ws, tx: Sealer::new(&keys.c2s), rx: Opener::new(&keys.s2c) }
}

/// Pairs by the typed code; returns (deviceId, token).
async fn pair_by_code(e: &Env) -> (String, String) {
    let mut s = handshake(e.port, Proof::PairCode(CODE.to_string())).await;
    let paired = s.recv().await;
    assert_eq!(paired["t"], "paired");
    (paired["deviceId"].as_str().unwrap().to_string(), paired["token"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn pair_by_code_then_device_login_gives_an_encrypted_welcome() {
    let e = env().await;
    let (id, token) = pair_by_code(&e).await;
    assert_eq!(token.len(), 43);
    let mut s = handshake(e.port, Proof::Device(id.clone(), secure::k_dev(&token))).await;
    let welcome = s.recv().await;
    assert_eq!((welcome["t"].as_str(), welcome["v"].as_u64(), welcome["deviceId"].as_str()), (Some("welcome"), Some(2), Some(id.as_str())));
    s.send(json!({ "t": "ping", "ts": 7 })).await;
    assert_eq!(s.recv().await, json!({ "t": "pong", "ts": 7 }));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn pair_by_qr_works_once() {
    let e = env().await;
    let info = e.agent.start_pairing();
    let s_text = info.url.split("&s=").nth(1).expect("the QR carries S_qr").to_string();
    let s_qr: [u8; 32] = URL_SAFE_NO_PAD.decode(s_text).unwrap().try_into().unwrap();
    let mut s = handshake(e.port, Proof::PairQr(s_qr)).await;
    assert_eq!(s.recv().await["t"], "paired");

    // The secret is used up: the same QR does not pair a second phone.
    let (mut ws, cn, sn) = hello2("127.0.0.1", e.port, "pair_qr", None).await;
    let keys = secure::derive_keys(&s_qr, &cn, &sn);
    let mac = secure::mac(&keys.auth, secure::LABEL_CLIENT, &cn, &sn);
    ws.send(Message::text(json!({ "t": "auth", "mac": URL_SAFE_NO_PAD.encode(mac) }).to_string())).await.unwrap();
    assert_eq!(recv_text(&mut ws).await["code"], "PAIR_CODE_INVALID");
    assert_eq!(close_code(&mut ws).await, 4401);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn a_wrong_mac_closes_with_4401() {
    let e = env().await;
    let (mut ws, _cn, _sn) = hello2("127.0.0.1", e.port, "pair_code", None).await;
    ws.send(Message::text(json!({ "t": "auth", "mac": URL_SAFE_NO_PAD.encode([0u8; 32]) }).to_string())).await.unwrap();
    let err = recv_text(&mut ws).await;
    assert_eq!((err["code"].as_str(), err["fatal"].as_bool()), (Some("PAIR_CODE_INVALID"), Some(true)));
    assert_eq!(close_code(&mut ws).await, 4401);

    // A device that does not exist gets the same answer as a wrong key.
    let (mut ws, cn, sn) = hello2("127.0.0.1", e.port, "device", Some("d-000000")).await;
    let keys = secure::derive_keys(&[1u8; 32], &cn, &sn);
    let mac = secure::mac(&keys.auth, secure::LABEL_CLIENT, &cn, &sn);
    ws.send(Message::text(json!({ "t": "auth", "mac": URL_SAFE_NO_PAD.encode(mac) }).to_string())).await.unwrap();
    assert_eq!(recv_text(&mut ws).await["code"], "UNAUTHORIZED");
    assert_eq!(close_code(&mut ws).await, 4401);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn replayed_skipped_and_changed_frames_close_with_4400() {
    let e = env().await;
    let (id, token) = pair_by_code(&e).await;
    let key = secure::k_dev(&token);

    // Replay: the same frame twice.
    let mut s = handshake(e.port, Proof::Device(id.clone(), key)).await;
    assert_eq!(s.recv().await["t"], "welcome");
    let frame = s.tx.seal(json!({ "t": "ping", "ts": 1 }).to_string().as_bytes());
    s.send_frame(frame.clone()).await;
    assert_eq!(s.recv().await["t"], "pong");
    s.send_frame(frame).await;
    assert_eq!(close_code(&mut s.ws).await, 4400);

    // Gap: the counter jumps over a value.
    let mut s = handshake(e.port, Proof::Device(id.clone(), key)).await;
    assert_eq!(s.recv().await["t"], "welcome");
    let _skipped = s.tx.seal(b"{}");
    let frame = s.tx.seal(json!({ "t": "ping", "ts": 1 }).to_string().as_bytes());
    s.send_frame(frame).await;
    assert_eq!(close_code(&mut s.ws).await, 4400);

    // A changed byte.
    let mut s = handshake(e.port, Proof::Device(id.clone(), key)).await;
    assert_eq!(s.recv().await["t"], "welcome");
    let mut frame = s.tx.seal(json!({ "t": "ping", "ts": 1 }).to_string().as_bytes());
    frame[12] ^= 1;
    s.send_frame(frame).await;
    assert_eq!(close_code(&mut s.ws).await, 4400);

    // Plain text after the handshake.
    let mut s = handshake(e.port, Proof::Device(id, key)).await;
    assert_eq!(s.recv().await["t"], "welcome");
    s.ws.send(Message::text(json!({ "t": "ping", "ts": 1 }).to_string())).await.unwrap();
    assert_eq!(close_code(&mut s.ws).await, 4400);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn v1_works_from_loopback_and_is_refused_from_the_network() {
    let e = env().await;
    // Loopback: the old protocol still answers (USB, development, tests).
    let mut ws = connect_async(format!("ws://127.0.0.1:{}/ws", e.port)).await.unwrap().0;
    ws.send(Message::text(json!({ "t": "pair", "v": 1, "code": CODE, "client": client() }).to_string())).await.unwrap();
    assert_eq!(recv_text(&mut ws).await["t"], "paired");
    assert_eq!(recv_text(&mut ws).await["t"], "welcome");

    // From another address of this PC the same message gets 4426 and the supported list. The agent of this test listens on
    // 127.0.0.1 only (no firewall prompt on a developer PC), so the address of the network card refuses the connection: the
    // refusal of v1 from the network is then covered by `netpolicy.rs`, which fakes the category of the connection.
    let Some(ip) = decka_core::agent::lan_ips().into_iter().next() else { return e.agent.shutdown().await };
    let Ok((mut ws, _)) = connect_async(format!("ws://{ip}:{}/ws", e.port)).await else { return e.agent.shutdown().await };
    ws.send(Message::text(json!({ "t": "hello", "v": 1, "token": "x".repeat(43), "client": client() }).to_string())).await.unwrap();
    let err = recv_text(&mut ws).await;
    assert_eq!((err["code"].as_str(), err["supported"].clone()), (Some("PROTOCOL_VERSION"), json!([2])));
    assert_eq!(close_code(&mut ws).await, 4426);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn an_old_devices_json_with_a_token_hash_logs_in_with_the_old_token() {
    let dir = tempfile::tempdir().unwrap();
    let token = "dGVzdC10b2tlbi1mb3ItZGVja2EtY3J5cHRvLXYyLTAxMjM";
    let hash: String = secure::k_dev(token).iter().map(|b| format!("{b:02x}")).collect();
    let old = format!(r#"{{"version":1,"devices":[{{"id":"d-abc123","name":"Old phone","tokenHash":"{hash}","createdAt":1,"lastSeenAt":2}}]}}"#);
    std::fs::write(dir.path().join("devices.json"), old).unwrap();
    let e = env_in(dir).await;

    let mut s = handshake(e.port, Proof::Device("d-abc123".to_string(), secure::k_dev(token))).await;
    assert_eq!(s.recv().await["t"], "welcome");
    let on_disk = std::fs::read_to_string(e._dir.path().join("devices.json")).unwrap();
    assert!(!on_disk.contains(&hash) && on_disk.contains("keyDpapi"));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn a_forged_auth_ok_is_detectable() {
    // The phone checks the server MAC: a server without K_dev cannot produce it.
    let e = env().await;
    let (id, token) = pair_by_code(&e).await;
    let (mut ws, cn, sn) = hello2("127.0.0.1", e.port, "device", Some(&id)).await;
    let keys = secure::derive_keys(&secure::k_dev(&token), &cn, &sn);
    let wrong = secure::derive_keys(&[9u8; 32], &cn, &sn);
    let forged = secure::mac(&wrong.auth, secure::LABEL_SERVER, &cn, &sn);
    let good = secure::mac(&keys.auth, secure::LABEL_SERVER, &cn, &sn);
    assert!(!secure::verify_mac(&keys.auth, secure::LABEL_SERVER, &cn, &sn, &forged));
    assert!(secure::verify_mac(&keys.auth, secure::LABEL_SERVER, &cn, &sn, &good));
    ws.close(None).await.ok();
    e.agent.shutdown().await;
}

const HOOK_BODY: &[u8] = br#"{"tool_name":"Bash","tool_input":{"command":"git status"},"cwd":"C:\\p","session_id":"abcdef123456"}"#;

async fn first_pending(agent: &Agent) -> String {
    for _ in 0..200 {
        if let Some(id) = agent.store().get("ai")["pending"][0]["reqId"].as_str() {
            return id.to_string();
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("no permission question appeared");
}

#[tokio::test]
async fn only_a_phone_with_the_right_gets_and_answers_claude_code_questions() {
    let e = env().await;
    let home = tempfile::tempdir().unwrap();
    e.agent.cc_install_in(home.path()).unwrap();
    let (id, token) = pair_by_code(&e).await;
    assert!(!e.agent.list_devices()[0].can_approve, "a new phone starts without the right");
    let mut phone = handshake(e.port, Proof::Device(id.clone(), secure::k_dev(&token))).await;
    assert_eq!(phone.recv().await["t"], "welcome");

    // A connected phone without the right: the question goes straight to the terminal.
    assert_eq!(e.agent.cc_permission(HOOK_BODY, false).await, None);

    // With the right the question reaches the phone; an answer from it is taken.
    assert!(e.agent.set_can_approve(&id, true));
    let task = tokio::spawn({
        let agent = e.agent.clone();
        async move { agent.cc_permission(HOOK_BODY, false).await }
    });
    let req = first_pending(&e.agent).await;
    // The right is taken away while the question is open: a perm_answer is ignored ...
    assert!(e.agent.set_can_approve(&id, false));
    phone.send(json!({ "t": "perm_answer", "reqId": req, "decision": "allow" })).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished(), "an answer of a phone without the right must be ignored");
    // ... and taken once the right is back.
    assert!(e.agent.set_can_approve(&id, true));
    phone.send(json!({ "t": "perm_answer", "reqId": req, "decision": "allow" })).await;
    let answer = timeout(Duration::from_secs(3), task).await.expect("the question was answered").unwrap();
    assert!(answer.is_some_and(|a| a.contains("allow")));
    e.agent.shutdown().await;
}

/// M21: a voice_* message with a bad shape gets `BAD_MESSAGE`, voice is `NOT_FOUND` while it is off, and a phone
/// cannot start a request with a made-up id; the connection survives all of them.
#[tokio::test]
async fn voice_messages_are_checked_and_answered_with_protocol_errors() {
    let e = env().await;
    let (id, token) = pair_by_code(&e).await;
    let mut phone = handshake(e.port, Proof::Device(id, secure::k_dev(&token))).await;
    assert_eq!(phone.recv().await["t"], "welcome");

    // Voice control is off: no pipeline is attached.
    phone.send(json!({ "t": "voice_start", "id": "00112233aabbccdd" })).await;
    let err = phone.recv().await;
    assert_eq!((err["t"].as_str(), err["code"].as_str(), err["fatal"].as_bool()), (Some("error"), Some("NOT_FOUND"), Some(false)));

    // Wrong shapes: missing field, wrong type, unknown extra command-like message.
    for bad in [
        json!({ "t": "voice_start" }),
        json!({ "t": "voice_chunk", "id": "00112233aabbccdd", "seq": "1", "data": "AAAA" }),
        json!({ "t": "voice_confirm", "id": "00112233aabbccdd", "yes": "yes" }),
        json!({ "t": "voice_run", "id": "00112233aabbccdd", "cmd": "calc.exe" }),
    ] {
        phone.send(bad).await;
        let err = phone.recv().await;
        assert_eq!((err["t"].as_str(), err["code"].as_str()), (Some("error"), Some("BAD_MESSAGE")), "{err}");
    }

    // A pipeline is attached: the id must be 16 hex characters, a chunk needs a recording.
    struct Sink;
    impl decka_core::voice::PhoneSink for Sink {
        fn start(&self, _: &str) -> Result<(), decka_core::voice::VoiceError> {
            Ok(())
        }
        fn audio(&self, _: &[u8]) {}
        fn end(&self, _: &str) {}
        fn cancel(&self, _: &str) {}
        fn confirm(&self, _: &str, _: bool) {}
    }
    e.agent.voice().set_sink(Some(Arc::new(Sink)));
    phone.send(json!({ "t": "voice_start", "id": "not-hex" })).await;
    assert_eq!(phone.recv().await["code"], "BAD_MESSAGE");
    phone.send(json!({ "t": "voice_chunk", "id": "00112233aabbccdd", "seq": 0, "data": "AAAA" })).await;
    assert_eq!(phone.recv().await["code"], "BAD_MESSAGE");
    phone.send(json!({ "t": "voice_start", "id": "00112233aabbccdd" })).await;
    phone.send(json!({ "t": "voice_start", "id": "ffffffffffffffff" })).await;
    assert_eq!(phone.recv().await["code"], "RATE_LIMITED", "a second recording of the same phone");
    phone.send(json!({ "t": "voice_chunk", "id": "00112233aabbccdd", "seq": 0, "data": "AAAAAA==" })).await;
    phone.send(json!({ "t": "ping", "ts": 9 })).await;
    assert_eq!(phone.recv().await, json!({ "t": "pong", "ts": 9 }), "good messages produce no answer and the link lives on");
    e.agent.shutdown().await;
}

/// Plain v1 (loopback, unencrypted) must not carry voice.
#[tokio::test]
async fn voice_is_refused_outside_the_encrypted_channel() {
    let e = env().await;
    let (id, token) = pair_by_code(&e).await;
    let _ = (id, &token);
    let mut ws = connect_async(format!("ws://127.0.0.1:{}/ws", e.port)).await.unwrap().0;
    ws.send(Message::text(json!({ "t": "hello", "v": 1, "token": token, "client": client() }).to_string())).await.unwrap();
    assert_eq!(recv_text(&mut ws).await["t"], "welcome");
    ws.send(Message::text(json!({ "t": "voice_start", "id": "00112233aabbccdd" }).to_string())).await.unwrap();
    let err = recv_text(&mut ws).await;
    assert_eq!((err["t"].as_str(), err["code"].as_str()), (Some("error"), Some("BAD_MESSAGE")));
    e.agent.shutdown().await;
}
