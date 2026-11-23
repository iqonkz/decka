//! No secret reaches the log (DATA_MODEL.md 11.6). One test in its own process: it installs the global log subscriber.
//! The run covers pairing, login, a permission answer, OBS and mail with known secret strings.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use decka_core::actions::DryRunner;
use decka_core::secure::{self, Opener, Sealer};
use decka_core::{Agent, AgentOptions};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing_subscriber::fmt::MakeWriter;

const CODE: &str = "TESTCDE2";
const OBS_PASSWORD: &str = "SECRET-OBS-PASSWORD-91x";
const MAIL_PASSWORD: &str = "SECRET-MAIL-PASSWORD-77q";

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = Capture;
    fn make_writer(&'a self) -> Capture {
        self.clone()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::test]
async fn pairing_login_permissions_obs_and_mail_leave_no_secret_in_the_log() {
    let log = Capture::default();
    tracing_subscriber::fmt().with_writer(log.clone()).with_max_level(tracing::Level::DEBUG).with_ansi(false).init();

    let dir = tempfile::tempdir().unwrap();
    let mut config = decka_core::config::default_config();
    config.collectors.focus = Some(decka_core::model::SwitchConfig { enabled: false });
    config.collectors.media = Some(decka_core::model::SwitchConfig { enabled: false });
    config.obs = Some(serde_json::from_value(json!({ "enabled": true, "port": 1999 })).unwrap());
    config.mail = Some(serde_json::from_value(json!({ "accounts": [{ "id": "m1", "name": "Mail", "host": "localhost", "port": 1, "user": "me@example.org" }] })).unwrap());
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

    // OBS and mail with passwords that are known to the test.
    agent.set_obs_password(OBS_PASSWORD).unwrap();
    let account_id = agent.config().mail.unwrap().accounts[0].id.clone();
    agent.set_mail_password(&account_id, MAIL_PASSWORD).unwrap();
    agent.mail_check_now();
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Pairing by the typed code, then a login of the paired device, then a permission answer.
    let (device_id, token) = {
        let mut s = handshake(port, "pair_code", None, |cn, sn| secure::s_code(CODE, cn, sn)).await;
        let paired = s.recv().await;
        (paired["deviceId"].as_str().unwrap().to_string(), paired["token"].as_str().unwrap().to_string())
    };
    let mut phone = handshake(port, "device", Some(&device_id), |_, _| secure::k_dev(&token)).await;
    assert_eq!(phone.recv().await["t"], "welcome");
    phone.send(json!({ "t": "perm_answer", "reqId": "0123456789abcdef", "decision": "allow" })).await;
    phone.send(json!({ "t": "press", "id": 1, "actionId": "media-play" })).await;
    let _ = phone.recv().await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let api_token = agent.config().api_token;
    let secrets = [
        ("device token", token.clone()),
        ("K_dev", hex(&secure::k_dev(&token))),
        ("pairing code", CODE.to_string()),
        ("OBS password", OBS_PASSWORD.to_string()),
        ("mail password", MAIL_PASSWORD.to_string()),
        ("API token", api_token),
    ];
    agent.shutdown().await;

    let text = String::from_utf8_lossy(&log.0.lock().unwrap()).to_string();
    assert!(text.contains("press media-play"), "the log must have been captured: {} bytes", text.len());
    for (name, secret) in secrets {
        assert!(!text.contains(&secret), "the log holds the {name}");
    }
}

struct Chan {
    ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tx: Sealer,
    rx: Opener,
}

impl Chan {
    async fn send(&mut self, v: Value) {
        let frame = self.tx.seal(v.to_string().as_bytes());
        self.ws.send(Message::Binary(frame.into())).await.unwrap();
    }
    async fn recv(&mut self) -> Value {
        loop {
            let msg = timeout(Duration::from_secs(3), self.ws.next()).await.expect("timed out").expect("closed").unwrap();
            if let Message::Binary(b) = msg {
                return serde_json::from_slice(&self.rx.open(&b).unwrap()).unwrap();
            }
        }
    }
}

async fn handshake(port: u16, mode: &str, device_id: Option<&str>, ikm: impl Fn(&[u8; 16], &[u8; 16]) -> [u8; 32]) -> Chan {
    let mut ws = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap().0;
    let cn: [u8; 16] = secure::random();
    let mut hello = json!({ "t": "hello2", "v": 2, "mode": mode, "cn": URL_SAFE_NO_PAD.encode(cn), "client": { "name": "test", "app": "web", "version": "0.0.0" } });
    if let Some(id) = device_id {
        hello["deviceId"] = json!(id);
    }
    ws.send(Message::text(hello.to_string())).await.unwrap();
    let challenge = next_text(&mut ws).await;
    let sn: [u8; 16] = URL_SAFE_NO_PAD.decode(challenge["sn"].as_str().unwrap()).unwrap().try_into().unwrap();
    let keys = secure::derive_keys(&ikm(&cn, &sn), &cn, &sn);
    let mac = secure::mac(&keys.auth, secure::LABEL_CLIENT, &cn, &sn);
    ws.send(Message::text(json!({ "t": "auth", "mac": URL_SAFE_NO_PAD.encode(mac) }).to_string())).await.unwrap();
    assert_eq!(next_text(&mut ws).await["t"], "auth_ok");
    Chan { ws, tx: Sealer::new(&keys.c2s), rx: Opener::new(&keys.s2c) }
}

async fn next_text(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) -> Value {
    loop {
        let msg = timeout(Duration::from_secs(3), ws.next()).await.expect("timed out").expect("closed").unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}
