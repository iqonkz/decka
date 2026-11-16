//! OBS against a stand-in for obs-websocket 5 (no OBS needed): the handshake with a password, an action and its event,
//! a wrong password (close 4009, no tries after it), OBS not running.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::config::{default_actions, default_config, save_json_atomic};
use decka_core::model::{Action, ObsConfig, ObsOp, SwitchConfig};
use decka_core::obs::auth_string;
use decka_core::proto::ErrorCode;
use decka_core::{Agent, AgentOptions};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
use tokio_tungstenite::tungstenite::Message;

const SALT: &str = "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=";
const CHALLENGE: &str = "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY=";

/// A fake OBS on a free port. `password` None = no authentication. Returns the port and the number of connections made.
async fn fake_obs(password: Option<&'static str>) -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else { return };
            count.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                // obs-websocket answers with its subprotocol; the client asks for it.
                let callback = |req: &Request, mut res: Response| {
                    assert_eq!(req.headers().get("sec-websocket-protocol").unwrap(), "obswebsocket.json");
                    res.headers_mut().insert("sec-websocket-protocol", "obswebsocket.json".parse().unwrap());
                    Ok(res)
                };
                let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(socket, callback).await else { return };
                let mut hello = json!({ "obsWebSocketVersion": "5.5.0", "rpcVersion": 1 });
                if password.is_some() {
                    hello["authentication"] = json!({ "challenge": CHALLENGE, "salt": SALT });
                }
                ws.send(Message::text(json!({ "op": 0, "d": hello }).to_string())).await.unwrap();
                let Some(Ok(Message::Text(identify))) = ws.next().await else { return };
                let identify: Value = serde_json::from_str(identify.as_str()).unwrap();
                assert_eq!((identify["op"].as_u64(), identify["d"]["rpcVersion"].as_u64(), identify["d"]["eventSubscriptions"].as_u64()), (Some(1), Some(1), Some(68)));
                if let Some(pw) = password {
                    if identify["d"]["authentication"] != json!(auth_string(pw, SALT, CHALLENGE)) {
                        let frame = CloseFrame { code: CloseCode::from(4009), reason: "Authentication failed.".into() };
                        let _ = ws.send(Message::Close(Some(frame))).await;
                        return;
                    }
                }
                ws.send(Message::text(json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string())).await.unwrap();
                let mut recording = false;
                while let Some(Ok(message)) = ws.next().await {
                    let Message::Text(text) = message else { continue };
                    let v: Value = serde_json::from_str(text.as_str()).unwrap();
                    if v["op"] != 6 {
                        continue;
                    }
                    let (kind, id) = (v["d"]["requestType"].as_str().unwrap().to_string(), v["d"]["requestId"].clone());
                    let data = match kind.as_str() {
                        "GetRecordStatus" => json!({ "outputActive": false, "outputDuration": 0 }),
                        "GetStreamStatus" => json!({ "outputActive": false, "outputDuration": 0 }),
                        "GetCurrentProgramScene" => json!({ "currentProgramSceneName": "Gaming" }),
                        "GetSceneList" => json!({ "scenes": [{ "sceneName": "Gaming" }, { "sceneName": "Talk" }] }),
                        "GetInputList" => json!({ "inputs": [{ "inputName": "Mic/Aux" }] }),
                        _ => json!({}),
                    };
                    if kind == "GetReplayBufferStatus" {
                        let status = json!({ "result": false, "code": 604, "comment": "Replay buffer is not available" });
                        ws.send(Message::text(json!({ "op": 7, "d": { "requestType": kind, "requestId": id, "requestStatus": status } }).to_string())).await.unwrap();
                        continue;
                    }
                    if kind == "SetCurrentProgramScene" && v["d"]["requestData"]["sceneName"] == "Missing" {
                        let status = json!({ "result": false, "code": 600, "comment": "No scene was found by the name of `Missing`." });
                        ws.send(Message::text(json!({ "op": 7, "d": { "requestType": kind, "requestId": id, "requestStatus": status } }).to_string())).await.unwrap();
                        continue;
                    }
                    let status = json!({ "result": true, "code": 100 });
                    ws.send(Message::text(json!({ "op": 7, "d": { "requestType": kind, "requestId": id, "requestStatus": status, "responseData": data } }).to_string())).await.unwrap();
                    if kind == "ToggleRecord" {
                        recording = !recording;
                        let event = json!({ "eventType": "RecordStateChanged", "eventIntent": 64, "eventData": { "outputActive": recording, "outputState": "OBS_WEBSOCKET_OUTPUT_STARTED" } });
                        ws.send(Message::text(json!({ "op": 5, "d": event }).to_string())).await.unwrap();
                    }
                }
            });
        }
    });
    (port, connections)
}

async fn start(dir: &std::path::Path, port: u16) -> Agent {
    let mut config = default_config();
    config.obs = Some(ObsConfig { enabled: true, port: u32::from(port), password: None, password_set: None });
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    let mut actions = default_actions();
    let obs = |op, scene: Option<&str>| Action::Obs { name: "OBS".into(), toggle_var: None, op, scene: scene.map(str::to_string), input: None };
    actions.actions.insert("obs-record".into(), obs(ObsOp::RecordToggle, None));
    actions.actions.insert("obs-scene".into(), obs(ObsOp::Scene, Some("Talk")));
    actions.actions.insert("obs-missing".into(), obs(ObsOp::Scene, Some("Missing")));
    save_json_atomic(&dir.join("actions.json"), &actions).unwrap();
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None }).await.unwrap()
}

async fn until(secs: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    done()
}

#[tokio::test]
async fn a_password_is_proved_an_action_runs_and_its_event_updates_the_section() {
    let (port, _) = fake_obs(Some("supersecretpassword")).await;
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), port).await;
    agent.set_obs_password("supersecretpassword").unwrap();

    assert!(until(5, || agent.store().get("obs")["connected"] == true).await, "never connected: {:?}", agent.store().get("obs"));
    assert_eq!(agent.store().get("obs")["scene"], "Gaming");
    let vars = agent.store().get("vars");
    assert_eq!((&vars["obs.connected"], &vars["obs.recording"], &vars["obs.scene"]), (&json!(true), &json!(false), &json!("Gaming")));

    agent.execute("obs-record").await.unwrap();
    assert!(until(3, || agent.store().get("obs")["recording"] == true).await, "{:?}", agent.store().get("obs"));
    assert!(agent.store().get("obs")["recordStartedAt"].is_u64());
    assert_eq!(agent.store().get("vars")["obs.recording"], true);

    let status = agent.obs_status().await;
    assert_eq!((status.connected, status.scenes, status.inputs), (true, vec!["Gaming".to_string(), "Talk".to_string()], vec!["Mic/Aux".to_string()]));
    agent.shutdown().await;
}

#[tokio::test]
async fn a_refusal_of_obs_comes_back_as_the_error_of_the_key() {
    let (port, _) = fake_obs(None).await;
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), port).await;
    assert!(until(5, || agent.store().get("obs")["connected"] == true).await);
    agent.execute("obs-scene").await.unwrap();
    let err = agent.execute("obs-missing").await.unwrap_err();
    assert_eq!(err, (ErrorCode::ActionFailed, "No scene was found by the name of `Missing`.".to_string()));
    agent.shutdown().await;
}

#[tokio::test]
async fn a_wrong_password_closes_with_4009_says_so_once_and_is_not_retried() {
    let (port, connections) = fake_obs(Some("good")).await;
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), port).await;
    agent.set_obs_password("bad").unwrap();

    assert!(until(5, || agent.store().get("notifications").as_array().is_some_and(|n| n.iter().any(|x| x["title"] == "OBS: wrong password"))).await, "no notification");
    assert_eq!(agent.store().get("obs")["connected"], false);
    assert_eq!(agent.obs_status().await.error.as_deref(), Some("Wrong password"));
    // The first retry of a normal failure comes after 5 s: wait past it.
    let seen = connections.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(6500)).await;
    assert_eq!(connections.load(Ordering::SeqCst), seen, "it tried again with the same password");
    let notices = agent.store().get("notifications").as_array().unwrap().iter().filter(|x| x["title"] == "OBS: wrong password").count();
    assert_eq!(notices, 1);

    // A new password starts it again.
    agent.set_obs_password("good").unwrap();
    assert!(until(5, || agent.store().get("obs")["connected"] == true).await);
    agent.shutdown().await;
}

#[tokio::test]
async fn a_password_that_is_needed_but_missing_counts_as_wrong() {
    let (port, _) = fake_obs(Some("good")).await;
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), port).await;
    assert!(until(5, || agent.store().get("notifications").as_array().is_some_and(|n| !n.is_empty())).await);
    assert_eq!(agent.store().get("obs")["connected"], false);
    agent.shutdown().await;
}

#[tokio::test]
async fn obs_not_running_is_quiet_and_its_keys_say_so() {
    // Nothing listens on this port.
    let free = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = free.local_addr().unwrap().port();
    drop(free);
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), port).await;

    assert!(until(5, || agent.store().get("obs")["connected"] == false).await, "{:?}", agent.store().get("obs"));
    assert_eq!(agent.store().get("notifications"), json!([]), "no notification while OBS is simply not running");
    assert_eq!(agent.store().get("vars")["obs.connected"], false);
    assert!(agent.store().get("vars").get("obs.recording").is_none());
    let err = agent.execute("obs-record").await.unwrap_err();
    assert_eq!(err, (ErrorCode::ActionFailed, "OBS is not connected".to_string()));
    let status = agent.obs_status().await;
    assert!(!status.connected && status.scenes.is_empty());
    agent.shutdown().await;
}

#[tokio::test]
async fn the_password_never_reaches_the_window_or_the_disk_in_the_clear() {
    let (port, _) = fake_obs(Some("topsecret42")).await;
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), port).await;
    agent.set_obs_password("topsecret42").unwrap();
    let window = serde_json::to_string(&agent.config_for_window()).unwrap();
    assert!(!window.contains("topsecret42") && !window.contains("dpapi"), "{window}");
    assert_eq!(agent.config_for_window().obs.unwrap().password_set, Some(true));
    let disk = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(disk.contains("dpapi:") && !disk.contains("topsecret42"));
    for path in std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_file()) {
        assert!(!std::fs::read_to_string(&path).unwrap_or_default().contains("topsecret42"), "{path:?}");
    }
    // An empty password removes it.
    agent.set_obs_password("").unwrap();
    assert_eq!(agent.config_for_window().obs.unwrap().password_set, Some(false));
    agent.shutdown().await;
}
