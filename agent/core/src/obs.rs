//! OBS Studio over obs-websocket 5 (DATA_MODEL.md 6.4): the agent is the client, the address is `ws://127.0.0.1:<port>` and
//! nothing else. It follows the state of OBS (the `obs` section and the `obs.*` variables), runs the `obs` actions, and tells the
//! window the scenes and inputs. The password never leaves this module: it is read once from the DPAPI blob and used for the
//! handshake only.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

use crate::model::{ObsInfo, ObsOp, VarValue};
use crate::state::Store;

/// Scenes (1 << 2) and Outputs (1 << 6): the events that matter here.
const EVENT_SUBSCRIPTIONS: u64 = 4 | 64;
/// obs-websocket closes with this code when the password is wrong.
const CLOSE_AUTH_FAILED: u16 = 4009;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Between tries while OBS is not running: 5, 10, 20, 40, 60, 60... seconds.
pub const RETRY_START_SECS: u64 = 5;
pub const RETRY_MAX_SECS: u64 = 60;

/// `base64(sha256(base64(sha256(password + salt)) + challenge))`, the authentication of obs-websocket 5.
pub fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let secret = STANDARD.encode(Sha256::digest(format!("{password}{salt}")));
    STANDARD.encode(Sha256::digest(format!("{secret}{challenge}")))
}

/// The wait after the `tries`-th failure in a row (the first is 1): 5, 10, 20, 40, 60, 60...
pub fn retry_delay(tries: u32) -> Duration {
    let secs = RETRY_START_SECS.saturating_mul(1u64 << tries.saturating_sub(1).min(8));
    Duration::from_secs(secs.min(RETRY_MAX_SECS))
}

/// What the agent knows about OBS.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ObsState {
    pub connected: bool,
    pub recording: bool,
    pub record_started_at: Option<u64>,
    pub streaming: bool,
    pub stream_started_at: Option<u64>,
    pub scene: Option<String>,
    pub replay_buffer: bool,
    pub scenes: Vec<String>,
    pub inputs: Vec<String>,
}

fn started_at(now_ms: u64, duration_ms: Option<u64>) -> u64 {
    now_ms.saturating_sub(duration_ms.unwrap_or(0))
}

impl ObsState {
    /// The `obs` section.
    pub fn info(&self) -> ObsInfo {
        ObsInfo {
            connected: self.connected,
            recording: self.connected && self.recording,
            record_started_at: if self.connected && self.recording { self.record_started_at } else { None },
            streaming: self.connected && self.streaming,
            stream_started_at: if self.connected && self.streaming { self.stream_started_at } else { None },
            scene: if self.connected { self.scene.clone().map(|s| s.chars().take(100).collect()) } else { None },
            replay_buffer: self.connected && self.replay_buffer,
        }
    }

    /// An event of OBS (`RecordStateChanged`, `StreamStateChanged`, `CurrentProgramSceneChanged`, `ReplayBufferStateChanged`).
    pub fn apply_event(&mut self, event_type: &str, data: &Value, now_ms: u64) {
        let active = data.get("outputActive").and_then(Value::as_bool);
        match event_type {
            "RecordStateChanged" => {
                if let Some(on) = active {
                    self.recording = on;
                    self.record_started_at = on.then_some(now_ms);
                }
            }
            "StreamStateChanged" => {
                if let Some(on) = active {
                    self.streaming = on;
                    self.stream_started_at = on.then_some(now_ms);
                }
            }
            "ReplayBufferStateChanged" => {
                if let Some(on) = active {
                    self.replay_buffer = on;
                }
            }
            "CurrentProgramSceneChanged" => {
                if let Some(name) = data.get("sceneName").and_then(Value::as_str) {
                    self.scene = Some(name.to_string());
                }
            }
            _ => {}
        }
    }

    fn apply_status(&mut self, kind: Init, data: &Value, now_ms: u64) {
        let active = data.get("outputActive").and_then(Value::as_bool).unwrap_or(false);
        let duration = data.get("outputDuration").and_then(Value::as_u64);
        match kind {
            Init::Record => {
                self.recording = active;
                self.record_started_at = active.then(|| started_at(now_ms, duration));
            }
            Init::Stream => {
                self.streaming = active;
                self.stream_started_at = active.then(|| started_at(now_ms, duration));
            }
            Init::Replay => self.replay_buffer = active,
            Init::Scene => self.scene = data.get("currentProgramSceneName").and_then(Value::as_str).map(str::to_string),
            Init::SceneList => self.scenes = names(data, "scenes", "sceneName"),
            Init::InputList => self.inputs = names(data, "inputs", "inputName"),
        }
    }
}

fn names(data: &Value, list: &str, key: &str) -> Vec<String> {
    data.get(list)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get(key).and_then(Value::as_str)).map(|s| s.chars().take(100).collect()).collect())
        .unwrap_or_default()
}

/// `obs.connected`, `obs.recording`, `obs.streaming`, `obs.scene` for `bind` (DATA_MODEL.md 6.4). When OBS is not
/// connected only `obs.connected` stays (false): a key bound to the others shows as unavailable.
pub fn apply_vars(vars: &mut std::collections::BTreeMap<String, VarValue>, info: &ObsInfo) {
    vars.insert("obs.connected".into(), VarValue::Bool(info.connected));
    if info.connected {
        vars.insert("obs.recording".into(), VarValue::Bool(info.recording));
        vars.insert("obs.streaming".into(), VarValue::Bool(info.streaming));
        match &info.scene {
            Some(s) => vars.insert("obs.scene".into(), VarValue::Str(s.clone())),
            None => vars.remove("obs.scene"),
        };
    } else {
        for name in ["obs.recording", "obs.streaming", "obs.scene"] {
            vars.remove(name);
        }
    }
}

/// The integration is off: no section, no variables.
pub fn clear(store: &Store) {
    store.set("obs", Value::Null);
    store.update_vars(|vars| {
        for name in ["obs.connected", "obs.recording", "obs.streaming", "obs.scene"] {
            vars.remove(name);
        }
    });
}

fn publish(store: &Store, state: &ObsState) {
    let info = state.info();
    store.set("obs", serde_json::to_value(&info).expect("obs serializes"));
    store.update_vars(|vars| apply_vars(vars, &info));
}

/// The request of an action: the type of the obs-websocket request and its data.
pub fn request_for(op: ObsOp, scene: Option<&str>, input: Option<&str>) -> Result<(&'static str, Value), String> {
    Ok(match op {
        ObsOp::RecordToggle => ("ToggleRecord", Value::Null),
        ObsOp::StreamToggle => ("ToggleStream", Value::Null),
        ObsOp::SaveReplay => ("SaveReplayBuffer", Value::Null),
        ObsOp::Scene => ("SetCurrentProgramScene", json!({ "sceneName": scene.ok_or("no scene")? })),
        ObsOp::InputMuteToggle => ("ToggleInputMute", json!({ "inputName": input.ok_or("no input")? })),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Init {
    Record,
    Stream,
    Replay,
    Scene,
    SceneList,
    InputList,
}

enum Pending {
    User(oneshot::Sender<Result<Value, String>>),
    Init(Init),
}

enum Cmd {
    Request { kind: String, data: Value, reply: oneshot::Sender<Result<Value, String>> },
}

enum Failure {
    /// Closed with 4009, or a password was needed and there is none: no more tries until the password changes.
    WrongPassword,
    Other(String),
}

/// The connection to OBS: a task that connects, follows and reconnects.
pub struct ObsClient {
    task: JoinHandle<()>,
    tx: mpsc::UnboundedSender<Cmd>,
    shared: Arc<Mutex<ObsState>>,
    /// Why it is not connected, for the window: `Wrong password`, or the last error.
    error: Arc<Mutex<Option<String>>>,
}

impl Drop for ObsClient {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl ObsClient {
    /// Starts the loop on the runtime. `on_wrong_password` is called once when OBS refuses the password.
    pub fn start(runtime: &tokio::runtime::Handle, port: u16, password: Option<String>, store: Store, on_wrong_password: Arc<dyn Fn() + Send + Sync>) -> ObsClient {
        let (tx, rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Mutex::new(ObsState::default()));
        let error = Arc::new(Mutex::new(None));
        let task = runtime.spawn(run(port, password, store, rx, shared.clone(), error.clone(), on_wrong_password));
        ObsClient { task, tx, shared, error }
    }

    pub fn state(&self) -> ObsState {
        self.shared.lock().unwrap().clone()
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }

    /// One request to OBS; the answer's `responseData`. Not connected -> `OBS is not connected`; OBS refusing -> its comment.
    pub async fn request(&self, kind: &str, data: Value) -> Result<Value, String> {
        if !self.shared.lock().unwrap().connected {
            return Err("OBS is not connected".to_string());
        }
        let (reply, answer) = oneshot::channel();
        self.tx.send(Cmd::Request { kind: kind.to_string(), data, reply }).map_err(|_| "OBS is not connected".to_string())?;
        match tokio::time::timeout(REQUEST_TIMEOUT, answer).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("OBS is not connected".to_string()),
            Err(_) => Err("OBS did not answer".to_string()),
        }
    }
}

async fn run(
    port: u16,
    password: Option<String>,
    store: Store,
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    shared: Arc<Mutex<ObsState>>,
    error: Arc<Mutex<Option<String>>>,
    on_wrong_password: Arc<dyn Fn() + Send + Sync>,
) {
    let mut tries = 0u32;
    publish(&store, &shared.lock().unwrap().clone());
    loop {
        let outcome = session(port, password.as_deref(), &store, &mut rx, &shared, &error, &mut tries).await;
        {
            let mut state = shared.lock().unwrap();
            let (scenes, inputs) = (std::mem::take(&mut state.scenes), std::mem::take(&mut state.inputs));
            *state = ObsState { scenes, inputs, ..ObsState::default() };
            publish(&store, &state);
        }
        match outcome {
            Failure::WrongPassword => {
                *error.lock().unwrap() = Some("Wrong password".to_string());
                on_wrong_password();
                // No more tries until the password changes: the agent starts a new client then.
                std::future::pending::<()>().await;
            }
            Failure::Other(message) => {
                tracing::debug!("obs: {message}");
                *error.lock().unwrap() = Some(message);
            }
        }
        tries += 1;
        // Requests that came while there was no connection are answered at once.
        while let Ok(Cmd::Request { reply, .. }) = rx.try_recv() {
            let _ = reply.send(Err("OBS is not connected".to_string()));
        }
        tokio::time::sleep(retry_delay(tries)).await;
    }
}

async fn session(
    port: u16,
    password: Option<&str>,
    store: &Store,
    rx: &mut mpsc::UnboundedReceiver<Cmd>,
    shared: &Arc<Mutex<ObsState>>,
    error: &Arc<Mutex<Option<String>>>,
    tries: &mut u32,
) -> Failure {
    let other = |m: &str| Failure::Other(m.to_string());
    let mut request = match format!("ws://127.0.0.1:{port}").into_client_request() {
        Ok(r) => r,
        Err(e) => return Failure::Other(e.to_string()),
    };
    request.headers_mut().insert("Sec-WebSocket-Protocol", HeaderValue::from_static("obswebsocket.json"));
    let Ok(Ok((mut ws, _))) = tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(request)).await else {
        return other("OBS is not running");
    };

    // Hello -> Identify (with the answer to the challenge, when OBS wants a password) -> Identified.
    let hello = match next_message(&mut ws).await {
        Ok(Some((0, d))) => d,
        _ => return other("OBS did not say hello"),
    };
    let mut identify = json!({ "rpcVersion": 1, "eventSubscriptions": EVENT_SUBSCRIPTIONS });
    if let Some(auth) = hello.get("authentication") {
        let (Some(salt), Some(challenge)) = (auth.get("salt").and_then(Value::as_str), auth.get("challenge").and_then(Value::as_str)) else {
            return other("OBS sent a strange hello");
        };
        let Some(password) = password else { return Failure::WrongPassword };
        identify["authentication"] = Value::String(auth_string(password, salt, challenge));
    }
    if ws.send(Message::text(json!({ "op": 1, "d": identify }).to_string())).await.is_err() {
        return other("connection lost");
    }
    match next_message(&mut ws).await {
        Ok(Some((2, _))) => {}
        Err(Failure::WrongPassword) => return Failure::WrongPassword,
        Err(f) => return f,
        _ => return other("OBS did not accept the connection"),
    }

    // Connected: the first state, then events.
    *tries = 0;
    *error.lock().unwrap() = None;
    shared.lock().unwrap().connected = true;
    publish(store, &shared.lock().unwrap().clone());
    let mut pending: HashMap<String, Pending> = HashMap::new();
    let mut next_id = 0u64;
    for (kind, init) in [("GetRecordStatus", Init::Record), ("GetStreamStatus", Init::Stream), ("GetCurrentProgramScene", Init::Scene), ("GetReplayBufferStatus", Init::Replay), ("GetSceneList", Init::SceneList), ("GetInputList", Init::InputList)] {
        next_id += 1;
        let id = format!("init-{next_id}");
        if ws.send(Message::text(json!({ "op": 6, "d": { "requestType": kind, "requestId": id } }).to_string())).await.is_err() {
            return other("connection lost");
        }
        pending.insert(id, Pending::Init(init));
    }

    loop {
        tokio::select! {
            message = ws.next() => {
                let text = match message {
                    None => return other("OBS closed the connection"),
                    Some(Err(e)) => return Failure::Other(e.to_string()),
                    Some(Ok(Message::Close(frame))) => {
                        let code = frame.map(|f| u16::from(f.code)).unwrap_or(0);
                        return if code == CLOSE_AUTH_FAILED { Failure::WrongPassword } else { other("OBS closed the connection") };
                    }
                    Some(Ok(Message::Text(t))) => t,
                    Some(Ok(_)) => continue,
                };
                let Ok(v) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                let (op, d) = (v.get("op").and_then(Value::as_u64), v.get("d").cloned().unwrap_or(Value::Null));
                match op {
                    Some(5) => {
                        let kind = d.get("eventType").and_then(Value::as_str).unwrap_or("");
                        let data = d.get("eventData").cloned().unwrap_or(Value::Null);
                        let mut state = shared.lock().unwrap();
                        state.apply_event(kind, &data, crate::auth::now_ms());
                        publish(store, &state);
                    }
                    Some(7) => {
                        let id = d.get("requestId").and_then(Value::as_str).unwrap_or("").to_string();
                        let status = d.get("requestStatus").cloned().unwrap_or(Value::Null);
                        let ok = status.get("result").and_then(Value::as_bool).unwrap_or(false);
                        let data = d.get("responseData").cloned().unwrap_or(Value::Null);
                        match pending.remove(&id) {
                            Some(Pending::User(reply)) => {
                                let comment = status.get("comment").and_then(Value::as_str).map(str::to_string);
                                let code = status.get("code").and_then(Value::as_u64).unwrap_or(0);
                                let _ = reply.send(if ok { Ok(data) } else { Err(comment.unwrap_or_else(|| format!("OBS error {code}"))) });
                            }
                            Some(Pending::Init(init)) if ok => {
                                let mut state = shared.lock().unwrap();
                                state.apply_status(init, &data, crate::auth::now_ms());
                                publish(store, &state);
                            }
                            _ => {} // an init request that failed (no replay buffer): the default stays
                        }
                    }
                    _ => {}
                }
            }
            command = rx.recv() => {
                let Some(Cmd::Request { kind, data, reply }) = command else { return other("stopped") };
                next_id += 1;
                let id = format!("req-{next_id}");
                let mut d = json!({ "requestType": kind, "requestId": id });
                if !data.is_null() {
                    d["requestData"] = data;
                }
                if ws.send(Message::text(json!({ "op": 6, "d": d }).to_string())).await.is_err() {
                    let _ = reply.send(Err("OBS is not connected".to_string()));
                    return other("connection lost");
                }
                pending.insert(id, Pending::User(reply));
            }
        }
    }
}

/// The next message of OBS as `(op, d)`; `Ok(None)` for something that is not JSON text; a close with 4009 is `WrongPassword`.
async fn next_message<S>(ws: &mut S) -> Result<Option<(u64, Value)>, Failure>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let message = tokio::time::timeout(CONNECT_TIMEOUT, ws.next()).await.map_err(|_| Failure::Other("OBS did not answer".to_string()))?;
    match message {
        Some(Ok(Message::Text(t))) => {
            let v: Value = serde_json::from_str(t.as_str()).map_err(|_| Failure::Other("strange answer".to_string()))?;
            Ok(v.get("op").and_then(Value::as_u64).map(|op| (op, v.get("d").cloned().unwrap_or(Value::Null))))
        }
        Some(Ok(Message::Close(frame))) => {
            if frame.map(|f| u16::from(f.code)) == Some(CLOSE_AUTH_FAILED) {
                Err(Failure::WrongPassword)
            } else {
                Err(Failure::Other("OBS closed the connection".to_string()))
            }
        }
        Some(Ok(_)) => Ok(None),
        Some(Err(e)) => Err(Failure::Other(e.to_string())),
        None => Err(Failure::Other("OBS closed the connection".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_authentication_follows_the_formula_of_obs_websocket() {
        // From the obs-websocket 5 protocol documentation.
        assert_eq!(
            auth_string("supersecretpassword", "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=", "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY="),
            "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4="
        );
    }

    #[test]
    fn the_waits_between_tries_are_5_10_20_40_60_60() {
        assert_eq!((1..=7).map(|n| retry_delay(n).as_secs()).collect::<Vec<_>>(), [5, 10, 20, 40, 60, 60, 60]);
        assert_eq!(retry_delay(0).as_secs(), 5);
        assert_eq!(retry_delay(u32::MAX).as_secs(), 60);
    }

    #[test]
    fn events_follow_recording_streaming_scene_and_replay() {
        let mut s = ObsState { connected: true, ..ObsState::default() };
        s.apply_event("RecordStateChanged", &json!({ "outputActive": true, "outputState": "OBS_WEBSOCKET_OUTPUT_STARTED" }), 5_000);
        s.apply_event("StreamStateChanged", &json!({ "outputActive": true }), 6_000);
        s.apply_event("CurrentProgramSceneChanged", &json!({ "sceneName": "Gaming" }), 7_000);
        s.apply_event("ReplayBufferStateChanged", &json!({ "outputActive": true }), 8_000);
        s.apply_event("SomethingElse", &json!({}), 9_000);
        let info = s.info();
        assert_eq!((info.recording, info.record_started_at, info.streaming, info.stream_started_at), (true, Some(5_000), true, Some(6_000)));
        assert_eq!((info.scene.as_deref(), info.replay_buffer), (Some("Gaming"), true));

        s.apply_event("RecordStateChanged", &json!({ "outputActive": false }), 10_000);
        assert_eq!((s.info().recording, s.info().record_started_at), (false, None));
    }

    #[test]
    fn the_first_status_counts_the_start_back_from_the_duration() {
        let mut s = ObsState { connected: true, ..ObsState::default() };
        s.apply_status(Init::Record, &json!({ "outputActive": true, "outputDuration": 754_000 }), 1_000_000);
        assert_eq!(s.info().record_started_at, Some(246_000));
        s.apply_status(Init::Stream, &json!({ "outputActive": false, "outputDuration": 0 }), 1_000_000);
        assert_eq!((s.info().streaming, s.info().stream_started_at), (false, None));
        s.apply_status(Init::SceneList, &json!({ "scenes": [{ "sceneName": "A" }, { "sceneName": "B" }] }), 0);
        s.apply_status(Init::InputList, &json!({ "inputs": [{ "inputName": "Mic/Aux", "inputKind": "wasapi_input_capture" }] }), 0);
        assert_eq!((s.scenes.clone(), s.inputs.clone()), (vec!["A".to_string(), "B".to_string()], vec!["Mic/Aux".to_string()]));
    }

    #[test]
    fn not_connected_shows_nothing_on_air_and_the_variables_go() {
        let live = ObsState { connected: true, recording: true, record_started_at: Some(1), scene: Some("A".into()), ..ObsState::default() };
        let mut vars = std::collections::BTreeMap::new();
        apply_vars(&mut vars, &live.info());
        assert_eq!(serde_json::to_value(&vars).unwrap(), json!({ "obs.connected": true, "obs.recording": true, "obs.streaming": false, "obs.scene": "A" }));
        let gone = ObsState { recording: true, record_started_at: Some(1), ..ObsState::default() };
        assert_eq!(gone.info(), ObsInfo { connected: false, recording: false, record_started_at: None, streaming: false, stream_started_at: None, scene: None, replay_buffer: false });
        apply_vars(&mut vars, &gone.info());
        assert_eq!(serde_json::to_value(&vars).unwrap(), json!({ "obs.connected": false }));
    }

    #[test]
    fn the_actions_become_requests() {
        assert_eq!(request_for(ObsOp::RecordToggle, None, None).unwrap(), ("ToggleRecord", Value::Null));
        assert_eq!(request_for(ObsOp::StreamToggle, None, None).unwrap().0, "ToggleStream");
        assert_eq!(request_for(ObsOp::SaveReplay, None, None).unwrap().0, "SaveReplayBuffer");
        assert_eq!(request_for(ObsOp::Scene, Some("Gaming"), None).unwrap(), ("SetCurrentProgramScene", json!({ "sceneName": "Gaming" })));
        assert_eq!(request_for(ObsOp::InputMuteToggle, None, Some("Mic")).unwrap(), ("ToggleInputMute", json!({ "inputName": "Mic" })));
        assert!(request_for(ObsOp::Scene, None, None).is_err());
    }
}
