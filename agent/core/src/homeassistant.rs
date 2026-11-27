//! Home Assistant (TASKS.md T30.8): a `ha` action switches an entity over the REST API, and the state of the chosen
//! entities follows the WebSocket API into the variables `ha.<entity id>` (a button can `bind` to them).
//!
//! The address and the token are written only in the settings window; the token is kept under DPAPI. `https://` goes
//! anywhere, `http://` only to this computer and the local network - the rules of the webhooks. A phone and the voice
//! name an action or an entity the user chose; neither can name an address or a service of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

use crate::model::{HaService, HttpMethod, VarValue};
use crate::state::Store;
use crate::webhook;

/// The most entities the user can choose for voice control and buttons.
pub const MAX_ENTITIES: usize = 100;
const TIMEOUT: Duration = Duration::from_secs(10);
/// The list of a big home is a few megabytes; more than this is not read.
const MAX_BODY: usize = 16 << 20;
const RETRY_MAX: Duration = Duration::from_secs(60);

pub const NOT_CONFIGURED: &str = "Home Assistant is not set up";
pub const REFUSED: &str = "Home Assistant refused the token";
pub const NO_ANSWER: &str = "Home Assistant did not answer";

/// One entity as Home Assistant describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    /// `None`: neither on nor off (a sensor, an unavailable device).
    pub on: Option<bool>,
}

/// `domain.object`, short enough to be a variable `ha.<id>`.
pub fn is_entity_id(s: &str) -> bool {
    let part = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    s.len() <= 61 && s.split_once('.').is_some_and(|(d, o)| part(d) && part(o))
}

fn domain(entity: &str) -> &str {
    entity.split('.').next().unwrap_or_default()
}

/// Locks, doors, alarms, valves and sirens wait for a "yes" when a voice asks.
pub fn needs_yes(entity: &str) -> bool {
    ["lock", "cover", "alarm_control_panel", "valve", "siren"].contains(&domain(entity))
}

pub fn is_on(state: &str) -> Option<bool> {
    match state {
        "on" | "open" | "playing" => Some(true),
        "off" | "closed" | "paused" | "idle" | "standby" => Some(false),
        _ => None,
    }
}

/// The service of Home Assistant behind a service of a button: `(domain, service)`.
pub fn service_of(service: HaService) -> (&'static str, &'static str) {
    match service {
        HaService::Toggle => ("homeassistant", "toggle"),
        HaService::TurnOn => ("homeassistant", "turn_on"),
        HaService::TurnOff => ("homeassistant", "turn_off"),
        HaService::Scene => ("scene", "turn_on"),
    }
}

/// The address of the server, checked, without a trailing slash.
pub fn base(url: &str) -> Result<String, String> {
    let parsed = webhook::check_url(url.trim()).map_err(str::to_string)?;
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("the address of Home Assistant has no query".into());
    }
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn bearer(token: &str) -> Result<BTreeMap<String, String>, String> {
    let headers = BTreeMap::from([("Authorization".to_string(), format!("Bearer {token}")), ("Content-Type".to_string(), "application/json".to_string())]);
    // A token with a line break would be another header.
    if token.is_empty() || token.len() > 480 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(REFUSED.into());
    }
    Ok(headers)
}

fn status_error(code: u16) -> String {
    match code {
        401 | 403 => REFUSED.into(),
        n => format!("Home Assistant answered HTTP {n}"),
    }
}

/// Runs a service on one entity. Blocking (call it from a blocking thread).
pub fn call(url: &str, token: &str, entity: &str, service: HaService) -> Result<(), String> {
    if !is_entity_id(entity) || (service == HaService::Scene && domain(entity) != "scene") {
        return Err("Unknown entity".into());
    }
    let (d, s) = service_of(service);
    let target = format!("{}/api/services/{d}/{s}", base(url)?);
    let body = json!({ "entity_id": entity }).to_string();
    let code = webhook::send(HttpMethod::Post, &target, Some(&body), Some(&bearer(token)?)).map_err(|e| if e == webhook::NO_ANSWER { NO_ANSWER.into() } else { e })?;
    if (200..300).contains(&code) {
        Ok(())
    } else {
        Err(status_error(code))
    }
}

/// The addresses of the server, refused when plain `http` would leave the local network.
async fn resolve(url: &Url) -> Result<Vec<SocketAddr>, String> {
    let port = url.port_or_known_default().ok_or(NO_ANSWER)?;
    let host = url.host_str().ok_or(NO_ANSWER)?.trim_start_matches('[').trim_end_matches(']').to_string();
    let found = tokio::task::spawn_blocking(move || (host.as_str(), port).to_socket_addrs().map(Iterator::collect::<Vec<_>>))
        .await
        .map_err(|_| NO_ANSWER)?
        .map_err(|_| NO_ANSWER)?;
    let plain = matches!(url.scheme(), "http" | "ws");
    if !webhook::allowed_target(if plain { "http" } else { "https" }, &found) {
        return Err(webhook::NOT_LOCAL.into());
    }
    Ok(found)
}

pub fn entity_of(state: &Value) -> Option<Entity> {
    let id = state.get("entity_id")?.as_str()?.to_string();
    if !is_entity_id(&id) {
        return None;
    }
    let name = state.pointer("/attributes/friendly_name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&id);
    let name: String = name.chars().take(60).collect();
    Some(Entity { on: state.get("state").and_then(Value::as_str).and_then(is_on), name, id })
}

/// Every entity of the server (the settings window).
pub async fn states(url: &str, token: &str) -> Result<Vec<Entity>, String> {
    let target = Url::parse(&format!("{}/api/states", base(url)?)).map_err(|_| NO_ANSWER)?;
    let addresses = resolve(&target).await?;
    let mut client = reqwest::Client::builder().timeout(TIMEOUT).redirect(reqwest::redirect::Policy::none());
    if let Some(url::Host::Domain(name)) = target.host() {
        client = client.resolve_to_addrs(name, &addresses);
    }
    if target.scheme() == "http" || addresses.iter().all(|a| webhook::is_local(a.ip())) {
        client = client.no_proxy();
    }
    let mut request = client.build().map_err(|_| NO_ANSWER)?.get(target);
    for (name, value) in bearer(token)? {
        request = request.header(name, value);
    }
    let mut answer = request.send().await.map_err(|_| NO_ANSWER)?;
    if !answer.status().is_success() {
        return Err(status_error(answer.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = answer.chunk().await.map_err(|_| NO_ANSWER)? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_BODY {
            return Err("The list of Home Assistant is too large".into());
        }
    }
    let list: Vec<Value> = serde_json::from_slice(&body).map_err(|_| "Home Assistant answered something unreadable")?;
    let mut found: Vec<Entity> = list.iter().filter_map(entity_of).collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(found)
}

// ---- The live state ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Known {
    entities: BTreeMap<String, Entity>,
    connected: bool,
    error: Option<String>,
    /// Counts the connections: a task that was replaced writes nothing any more.
    generation: u64,
}

/// The connection to the server and what it said last.
#[derive(Clone)]
pub struct Hub {
    store: Store,
    known: Arc<Mutex<Known>>,
    task: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
}

/// `ha.<id>` of the watched entities that are on or off.
fn publish(store: &Store, known: &Known, watched: &BTreeSet<String>) {
    store.update_vars(|vars| {
        vars.retain(|k, _| !k.starts_with("ha."));
        if known.connected {
            for id in watched {
                if let Some(on) = known.entities.get(id).and_then(|e| e.on) {
                    vars.insert(format!("ha.{id}"), VarValue::Bool(on));
                }
            }
        }
    });
}

impl Hub {
    pub fn new(store: Store) -> Hub {
        Hub { store, known: Arc::default(), task: Arc::default() }
    }

    /// Connects (and keeps connecting) in place of what ran before.
    pub fn start(&self, runtime: &tokio::runtime::Handle, url: String, token: String, watched: BTreeSet<String>) {
        self.stop();
        let generation = self.known.lock().unwrap().generation;
        let hub = self.clone();
        let task = runtime.spawn(async move {
            let mut wait = Duration::from_secs(1);
            loop {
                let (connected, error) = hub.session(&url, &token, &watched, generation).await;
                if !hub.change(generation, &watched, |k| {
                    k.connected = false;
                    k.error = Some(error.clone());
                }) {
                    return;
                }
                tracing::debug!("home assistant: {error}");
                // A wrong token does not become right by asking often.
                wait = if connected { Duration::from_secs(1) } else if error == REFUSED { RETRY_MAX } else { (wait * 2).min(RETRY_MAX) };
                tokio::time::sleep(wait).await;
            }
        });
        *self.task.lock().unwrap() = Some(task.abort_handle());
    }

    pub fn stop(&self) {
        if let Some(task) = self.task.lock().unwrap().take() {
            task.abort();
        }
        let mut known = self.known.lock().unwrap();
        *known = Known { generation: known.generation + 1, ..Known::default() };
        publish(&self.store, &known, &BTreeSet::new());
    }

    /// What the server listed last (empty while it never answered).
    pub fn entities(&self) -> Vec<Entity> {
        self.known.lock().unwrap().entities.values().cloned().collect()
    }

    /// (connected, the last problem).
    pub fn status(&self) -> (bool, Option<String>) {
        let known = self.known.lock().unwrap();
        (known.connected, known.error.clone())
    }

    /// Applies `f` and publishes, unless this connection was replaced. `false`: it was.
    fn change(&self, generation: u64, watched: &BTreeSet<String>, f: impl FnOnce(&mut Known)) -> bool {
        let mut known = self.known.lock().unwrap();
        if known.generation != generation {
            return false;
        }
        f(&mut known);
        publish(&self.store, &known, watched);
        true
    }

    /// One connection, until it ends: whether it got as far as the states, and why it ended.
    async fn session(&self, url: &str, token: &str, watched: &BTreeSet<String>, generation: u64) -> (bool, String) {
        let target = match base(url).and_then(|b| Url::parse(&format!("{}/api/websocket", b.replacen("http", "ws", 1))).map_err(|_| NO_ANSWER.to_string())) {
            Ok(t) => t,
            Err(e) => return (false, e),
        };
        let connect = async {
            let addresses = resolve(&target).await?;
            let tcp = tokio::net::TcpStream::connect(addresses.as_slice()).await.map_err(|_| NO_ANSWER.to_string())?;
            Ok::<_, String>(tcp)
        };
        let tcp = match tokio::time::timeout(TIMEOUT, connect).await {
            Ok(Ok(tcp)) => tcp,
            Ok(Err(e)) => return (false, e),
            Err(_) => return (false, NO_ANSWER.into()),
        };
        if target.scheme() == "ws" {
            return self.talk(target.as_str(), tcp, token, watched, generation).await;
        }
        use rustls_platform_verifier::ConfigVerifierExt;
        use tokio_rustls::rustls::{pki_types::ServerName, ClientConfig};
        let secure = async {
            let config = ClientConfig::with_platform_verifier().map_err(|_| NO_ANSWER)?;
            let host = target.host_str().ok_or(NO_ANSWER)?.trim_start_matches('[').trim_end_matches(']').to_string();
            let name = ServerName::try_from(host).map_err(|_| NO_ANSWER)?;
            tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp).await.map_err(|_| NO_ANSWER)
        };
        match tokio::time::timeout(TIMEOUT, secure).await {
            Ok(Ok(tls)) => self.talk(target.as_str(), tls, token, watched, generation).await,
            _ => (false, NO_ANSWER.into()),
        }
    }

    async fn talk<S: AsyncRead + AsyncWrite + Unpin>(&self, url: &str, stream: S, token: &str, watched: &BTreeSet<String>, generation: u64) -> (bool, String) {
        let Ok(Ok((mut ws, _))) = tokio::time::timeout(TIMEOUT, tokio_tungstenite::client_async(url, stream)).await else {
            return (false, NO_ANSWER.into());
        };
        let mut connected = false;
        loop {
            // Silence before the states is a failure; afterwards the home may simply be quiet.
            let next = if connected { Ok(ws.next().await) } else { tokio::time::timeout(TIMEOUT, ws.next()).await };
            let text = match next {
                Ok(Some(Ok(Message::Text(t)))) => t,
                Ok(Some(Ok(Message::Close(_)) | Err(_)) | None) => return (connected, "Home Assistant closed the connection".into()),
                Ok(Some(Ok(_))) => continue,
                Err(_) => return (false, NO_ANSWER.into()),
            };
            let Ok(v) = serde_json::from_str::<Value>(text.as_str()) else { continue };
            let say: Vec<Value> = match v.get("type").and_then(Value::as_str) {
                Some("auth_required") => vec![json!({ "type": "auth", "access_token": token })],
                Some("auth_invalid") => return (false, REFUSED.into()),
                // The events first: a change between the two answers is not lost.
                Some("auth_ok") => vec![json!({ "id": 1, "type": "subscribe_events", "event_type": "state_changed" }), json!({ "id": 2, "type": "get_states" })],
                Some("result") if v.get("id").and_then(Value::as_u64) == Some(2) => {
                    let list: Vec<Entity> = v.get("result").and_then(Value::as_array).map(|l| l.iter().filter_map(entity_of).collect()).unwrap_or_default();
                    connected = true;
                    if !self.change(generation, watched, |k| {
                        (k.connected, k.error) = (true, None);
                        k.entities = list.into_iter().map(|e| (e.id.clone(), e)).collect();
                    }) {
                        return (connected, "stopped".into());
                    }
                    Vec::new()
                }
                Some("event") if connected => {
                    let Some(data) = v.pointer("/event/data") else { continue };
                    let Some(id) = data.get("entity_id").and_then(Value::as_str).map(str::to_string) else { continue };
                    let new = data.get("new_state").and_then(entity_of);
                    if !self.change(generation, watched, |k| {
                        match new {
                            Some(e) => k.entities.insert(id, e),
                            None => k.entities.remove(&id),
                        };
                    }) {
                        return (connected, "stopped".into());
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for message in say {
                if ws.send(Message::text(message.to_string())).await.is_err() {
                    return (connected, NO_ANSWER.into());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn ids_states_and_addresses_are_checked() {
        for good in ["light.bedroom", "scene.movie_night", "switch.x2"] {
            assert!(is_entity_id(good), "{good}");
        }
        let long = format!("light.{}", "x".repeat(60));
        for bad in ["light", "light.", ".x", "Light.bedroom", "light.bed room", "light.a/../b", "a.b.c", long.as_str()] {
            assert!(!is_entity_id(bad), "{bad}");
        }
        assert!(needs_yes("lock.front") && needs_yes("cover.garage") && !needs_yes("light.hall"));
        assert_eq!(base("http://homeassistant.local:8123/").as_deref(), Ok("http://homeassistant.local:8123"));
        assert!(base("http://example.com:8123").is_err() && base("https://home.example.com/?x=1").is_err() && base("ftp://nas").is_err());
        assert_eq!([is_on("on"), is_on("closed"), is_on("unavailable")], [Some(true), Some(false), None]);
        assert!(bearer("abc\r\nX: y").is_err() && bearer("").is_err());
    }

    /// A one-request HTTP server: gives back what was asked and answers with `status` and `body`.
    async fn http(status: &'static str, body: &'static str) -> (String, mpsc::UnboundedReceiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let n = socket.read(&mut buffer).await.unwrap_or(0);
                    request.extend_from_slice(&buffer[..n]);
                    let text = String::from_utf8_lossy(&request);
                    let Some((head, got)) = text.split_once("\r\n\r\n") else {
                        if n == 0 { break } else { continue }
                    };
                    let want = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                    if got.len() >= want || n == 0 {
                        break;
                    }
                }
                let _ = tx.send(String::from_utf8_lossy(&request).to_string());
                let answer = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = socket.write_all(answer.as_bytes()).await;
            }
        });
        (url, rx)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_service_is_called_on_the_entity_with_the_token() {
        let (url, mut asked) = http("200 OK", "[]").await;
        let to = url.clone();
        tokio::task::spawn_blocking(move || call(&to, "tok.en", "light.bedroom", HaService::Toggle)).await.unwrap().unwrap();
        let request = asked.recv().await.unwrap();
        assert!(request.starts_with("POST /api/services/homeassistant/toggle HTTP/1.1"), "{request}");
        assert!(request.to_ascii_lowercase().contains("authorization: bearer tok.en") && request.ends_with(r#"{"entity_id":"light.bedroom"}"#), "{request}");
        let to = url.clone();
        tokio::task::spawn_blocking(move || call(&to, "t", "scene.movie", HaService::Scene)).await.unwrap().unwrap();
        assert!(asked.recv().await.unwrap().starts_with("POST /api/services/scene/turn_on "));
        assert!(call(&url, "t", "light.x", HaService::Scene).is_err(), "a scene service is for scenes");
        let (refusing, _) = http("401 Unauthorized", "{}").await;
        let said = tokio::task::spawn_blocking(move || call(&refusing, "bad", "light.x", HaService::TurnOn)).await.unwrap();
        assert_eq!(said, Err(REFUSED.to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_list_of_entities_is_read_with_their_names() {
        let body = r#"[{"entity_id":"switch.desk","state":"off","attributes":{}},{"entity_id":"light.bedroom","state":"on","attributes":{"friendly_name":"Свет в спальне"}},{"entity_id":"BAD","state":"on"}]"#;
        let (url, _asked) = http("200 OK", body).await;
        let found = states(&url, "t").await.unwrap();
        assert_eq!(found, [Entity { id: "light.bedroom".into(), name: "Свет в спальне".into(), on: Some(true) }, Entity { id: "switch.desk".into(), name: "switch.desk".into(), on: Some(false) }]);
        let (refusing, _) = http("401 Unauthorized", "{}").await;
        assert_eq!(states(&refusing, "t").await, Err(REFUSED.to_string()));
    }

    /// The WebSocket API of the mock: the sign-in, the states, then the changes the test sends.
    async fn websocket(mut changes: mpsc::UnboundedReceiver<Value>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (socket, _): (TcpStream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            let say = |v: Value| Message::text(v.to_string());
            ws.send(say(json!({ "type": "auth_required" }))).await.unwrap();
            let auth: Value = serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(auth, json!({ "type": "auth", "access_token": "secret" }));
            ws.send(say(json!({ "type": "auth_ok" }))).await.unwrap();
            let first: Value = serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            let second: Value = serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            assert_eq!((first["type"].as_str(), second["type"].as_str()), (Some("subscribe_events"), Some("get_states")));
            ws.send(say(json!({ "id": 2, "type": "result", "success": true, "result": [
                { "entity_id": "light.bedroom", "state": "off", "attributes": { "friendly_name": "Bedroom" } },
                { "entity_id": "light.other", "state": "on" } ] }))).await.unwrap();
            while let Some(change) = changes.recv().await {
                ws.send(say(change)).await.unwrap();
            }
            // Dropping the connection ends it.
        });
        url
    }

    async fn until(store: &Store, want: Option<bool>) {
        for _ in 0..300 {
            if store.get("vars").get("ha.light.bedroom").and_then(Value::as_bool) == want {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("ha.light.bedroom never became {want:?}: {}", store.get("vars"));
    }

    #[tokio::test]
    async fn the_state_of_a_watched_entity_follows_the_connection() {
        let (tx, rx) = mpsc::unbounded_channel();
        let url = websocket(rx).await;
        let store = Store::new(json!({}));
        let hub = Hub::new(store.clone());
        hub.start(&tokio::runtime::Handle::current(), url, "secret".into(), BTreeSet::from(["light.bedroom".to_string()]));
        until(&store, Some(false)).await;
        assert!(store.get("vars").get("ha.light.other").is_none(), "only what was chosen");
        assert_eq!(hub.entities().len(), 2);
        assert_eq!(hub.status(), (true, None));
        let change = |state: &str| json!({ "id": 1, "type": "event", "event": { "event_type": "state_changed", "data": { "entity_id": "light.bedroom", "new_state": { "entity_id": "light.bedroom", "state": state, "attributes": {} } } } });
        tx.send(change("on")).unwrap();
        until(&store, Some(true)).await;
        tx.send(change("unavailable")).unwrap();
        until(&store, None).await;
        tx.send(change("on")).unwrap();
        until(&store, Some(true)).await;
        // The server goes away: the state is not shown as if it were known.
        drop(tx);
        until(&store, None).await;
        assert!(!hub.status().0);
        hub.stop();
        assert_eq!(hub.status(), (false, None));
    }
}
