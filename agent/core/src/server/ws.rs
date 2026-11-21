//! `/ws` endpoint: handshake, welcome, patches, ping, revocation (PROTOCOL.md 1, 4, 6, 10).
//!
//! Protocol v2 (M18): `hello2` -> `challenge` -> `auth` -> `auth_ok`, then only encrypted binary frames. Protocol v1
//! (plain text, token in the clear) is accepted from loopback only.

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, timeout_at, Instant};

use crate::agent::{Agent, ApkGetError};
use crate::model::Notification;
use crate::netpolicy::{policy, Category, ConnInfo, Request, Verdict};
use crate::proto::{AgentInfo, ClientInfo, ClientMsg, ErrorBody, ErrorCode, Hello2Mode, ServerMsg};
use crate::ratelimit::RateLimiter;
use crate::safety::PhoneCtx;
use crate::secure::{self, Keys, Opener, Sealer};

/// Largest plain JSON message of the client (and largest plain handshake frame).
const MAX_MESSAGE_BYTES: usize = 4096;
/// An encrypted frame adds the counter (8 bytes) and the GCM tag (16 bytes).
const MAX_FRAME_BYTES: usize = MAX_MESSAGE_BYTES + 24;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const PRESS_PER_SECOND: u32 = 20;
const SUPPORTED_VERSIONS: [u32; 1] = [2];

const CLOSE_NORMAL: u16 = 1000;
const CLOSE_GOING_AWAY: u16 = 1001;
const CLOSE_TOO_BIG: u16 = 1009;
const CLOSE_LAGGED: u16 = 1011;
const CLOSE_BAD_FRAME: u16 = 4400;
const CLOSE_UNAUTHORIZED: u16 = 4401;
const CLOSE_REVOKED: u16 = 4403;
const CLOSE_VERSION: u16 = 4426;
const CLOSE_RATE: u16 = 4429;

/// Origin allow-list (checked before the upgrade): absent header is fine (non-browser client).
fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else { return true };
    let Ok(origin) = origin.to_str() else { return false };
    if matches!(origin, "http://localhost" | "http://localhost:5173" | "http://127.0.0.1:5173") {
        return true;
    }
    headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|host| origin == format!("http://{host}"))
}

pub async fn handler(
    State(agent): State<Agent>,
    ConnectInfo(conn): ConnectInfo<ConnInfo>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| connection(agent, conn, socket))
}

/// Both directions of an encrypted connection.
struct Secure {
    tx: Sealer,
    rx: Opener,
}

/// A socket plus, after the v2 handshake, its encryption.
struct Link {
    ws: WebSocket,
    sec: Option<Secure>,
}

impl Link {
    async fn send(&mut self, msg: &ServerMsg) -> bool {
        let text = serde_json::to_string(msg).expect("server message serializes");
        let frame = match &mut self.sec {
            Some(sec) => Message::Binary(sec.tx.seal(text.as_bytes()).into()),
            None => Message::Text(text.into()),
        };
        self.ws.send(frame).await.is_ok()
    }

    /// Sends a close frame, then waits briefly for the peer so earlier frames are not lost to a TCP reset.
    async fn close(&mut self, code: u16) {
        let _ = self.ws.send(Message::Close(Some(CloseFrame { code, reason: "".into() }))).await;
        let _ = timeout(Duration::from_secs(1), async { while let Some(Ok(_)) = self.ws.recv().await {} }).await;
    }

    async fn fatal(&mut self, code: ErrorCode, message: &str, close_code: u16, supported: Option<Vec<u32>>) {
        self.send(&ServerMsg::Error { code, message: message.to_string(), fatal: true, supported }).await;
        self.close(close_code).await;
    }

    /// Next text frame, skipping ping/pong. `None` = closed, errored, too long, or a non-text frame.
    async fn recv_first_text(&mut self) -> Option<String> {
        loop {
            match self.ws.recv().await? {
                Ok(Message::Text(t)) if t.as_str().len() <= MAX_MESSAGE_BYTES => return Some(t.as_str().to_string()),
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
                _ => return None,
            }
        }
    }
}

fn error_msg(code: ErrorCode, message: &str) -> ServerMsg {
    ServerMsg::Error { code, message: message.to_string(), fatal: false, supported: None }
}

/// Who is on the other end of an authorized socket (for the press audit log).
struct Peer {
    device_id: String,
    name: String,
    remote: SocketAddr,
    /// When this connection was established (dangerous system actions wait for 60 s, DATA_MODEL.md 11.4).
    since: std::time::Instant,
}

/// What the handshake knows about the other side once it has proven itself.
struct Proven {
    keys: Keys,
    /// The proof was made with the active pairing code or its QR secret (the code is then used up).
    used_active_code: bool,
}

async fn connection(agent: Agent, conn: ConnInfo, socket: WebSocket) {
    let remote = conn.remote;
    let mut link = Link { ws: socket, sec: None };
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    // The first lookup of a network asks Windows: keep it off the async threads.
    let category = {
        let agent = agent.clone();
        tokio::task::spawn_blocking(move || agent.networks().category(conn.local.ip())).await.unwrap_or(Category::Public)
    };
    let allow_public = agent.allow_public();
    if policy(category, Request::Static, allow_public) == Verdict::Drop {
        return; // phones on public networks are switched off
    }
    let unauthorized = "authentication required";
    let first = timeout_at(deadline, link.recv_first_text()).await.ok().flatten();
    let Some(first) = first else {
        return link.fatal(ErrorCode::Unauthorized, unauthorized, CLOSE_UNAUTHORIZED, None).await;
    };

    match serde_json::from_str::<ClientMsg>(&first) {
        Ok(ClientMsg::Hello2 { v, mode, device_id, cn, client }) => {
            if !SUPPORTED_VERSIONS.contains(&v) {
                return version_error(&mut link, v).await;
            }
            let wanted = match mode {
                Hello2Mode::PairCode => Some(Request::PairCode),
                Hello2Mode::PairQr => Some(Request::PairQr),
                Hello2Mode::Device => None,
            };
            if let Some(Verdict::Refuse(message)) = wanted.map(|r| policy(category, r, allow_public)) {
                return link.fatal(ErrorCode::BadMessage, message, CLOSE_UNAUTHORIZED, None).await;
            }
            let Some(proven) = handshake_v2(&agent, remote, category, &mut link, deadline, mode, device_id.as_deref(), &cn).await else { return };
            link.sec = Some(Secure { tx: Sealer::new(&proven.keys.s2c), rx: Opener::new(&proven.keys.c2s) });
            match mode {
                Hello2Mode::Device => {
                    let id = device_id.unwrap_or_default();
                    agent.devices().touch_with_version(&id, Some(&client.version));
                    run_device(agent, remote, link, id, client).await;
                }
                Hello2Mode::PairQr | Hello2Mode::PairCode => {
                    if proven.used_active_code {
                        agent.pairing().burn();
                    }
                    let (record, token) = agent.devices().create(&client.name);
                    agent.devices().touch_with_version(&record.id, Some(&client.version));
                    // The token is shown to the phone once, encrypted; the phone then reconnects as a device.
                    if link.send(&ServerMsg::Paired { device_id: record.id, token }).await {
                        link.close(CLOSE_NORMAL).await;
                    }
                }
            }
        }
        // Protocol v1 (plain text): loopback only — USB through `adb reverse`, development and tests.
        Ok(ClientMsg::Pair { v, code, client }) if policy(category, Request::V1, allow_public) == Verdict::Allow => {
            if !agent.handshake_limiter(category).check(&remote.ip().to_string()) {
                return link.fatal(ErrorCode::RateLimited, "too many pairing attempts", CLOSE_RATE, None).await;
            }
            if v != 1 {
                return version_error(&mut link, v).await;
            }
            if !agent.pairing().try_consume(&code) {
                return link.fatal(ErrorCode::PairCodeInvalid, "pairing code rejected", CLOSE_UNAUTHORIZED, None).await;
            }
            let (record, token) = agent.devices().create(&client.name);
            agent.devices().touch_with_version(&record.id, Some(&client.version));
            if !link.send(&ServerMsg::Paired { device_id: record.id.clone(), token }).await {
                return;
            }
            run_device(agent, remote, link, record.id, client).await;
        }
        Ok(ClientMsg::Hello { v, token, client }) if policy(category, Request::V1, allow_public) == Verdict::Allow => {
            if v != 1 {
                return version_error(&mut link, v).await;
            }
            let Some(device_id) = agent.devices().verify(&token) else {
                return link.fatal(ErrorCode::Unauthorized, "invalid device token", CLOSE_UNAUTHORIZED, None).await;
            };
            agent.devices().touch_with_version(&device_id, Some(&client.version));
            run_device(agent, remote, link, device_id, client).await;
        }
        // v1 from the network: the client must update.
        Ok(ClientMsg::Pair { v, .. }) | Ok(ClientMsg::Hello { v, .. }) => version_error(&mut link, v).await,
        _ => link.fatal(ErrorCode::Unauthorized, unauthorized, CLOSE_UNAUTHORIZED, None).await,
    }
}

fn decode16(text: &str) -> Option<[u8; 16]> {
    URL_SAFE_NO_PAD.decode(text).ok()?.try_into().ok()
}

/// The v2 handshake after `hello2` (PROTOCOL.md 10). On success `auth_ok` has been sent; on failure the socket is closed.
#[allow(clippy::too_many_arguments)]
async fn handshake_v2(
    agent: &Agent,
    remote: SocketAddr,
    category: Category,
    link: &mut Link,
    deadline: Instant,
    mode: Hello2Mode,
    device_id: Option<&str>,
    cn_text: &str,
) -> Option<Proven> {
    let ip = remote.ip().to_string();
    if !agent.handshake_limiter(category).check(&ip) || agent.auth_fail_limiter().exhausted(&ip) {
        link.fatal(ErrorCode::RateLimited, "too many connection attempts", CLOSE_RATE, None).await;
        return None;
    }
    let Some(cn) = decode16(cn_text) else {
        link.fatal(ErrorCode::BadMessage, "invalid handshake", CLOSE_UNAUTHORIZED, None).await;
        return None;
    };
    let sn: [u8; 16] = secure::random();
    let agent_info = AgentInfo { name: agent.config().agent_name, version: crate::VERSION.to_string() };
    if !link.send(&ServerMsg::Challenge { sn: URL_SAFE_NO_PAD.encode(sn), agent: agent_info }).await {
        return None;
    }

    let reply = timeout_at(deadline, link.recv_first_text()).await.ok().flatten();
    let proof = reply
        .and_then(|text| serde_json::from_str::<ClientMsg>(&text).ok())
        .and_then(|msg| if let ClientMsg::Auth { mac } = msg { URL_SAFE_NO_PAD.decode(mac).ok() } else { None });
    let Some(proof) = proof else {
        link.fatal(ErrorCode::Unauthorized, "authentication required", CLOSE_UNAUTHORIZED, None).await;
        return None;
    };

    // Every secret this connection could legitimately know; (secret, is it the active pairing code).
    let mut secrets: Vec<([u8; 32], bool)> = Vec::new();
    match mode {
        Hello2Mode::Device => secrets.push((device_id.and_then(|id| agent.devices().key(id)).unwrap_or_else(secure::random::<32>), false)),
        Hello2Mode::PairQr => {
            let s = agent.pairing().candidates().active.map(|(_, s)| s);
            secrets.push((s.unwrap_or_else(secure::random::<32>), true));
        }
        Hello2Mode::PairCode => {
            let c = agent.pairing().candidates();
            let mut codes: Vec<(String, bool)> = Vec::new();
            if let Some((code, _)) = c.active {
                codes.push((code, true));
            }
            if let Some(code) = c.fixed {
                codes.push((code, false));
            }
            for (code, active) in codes {
                // PBKDF2 is deliberately slow: keep it off the async threads.
                let derived = tokio::task::spawn_blocking(move || secure::s_code(&code, &cn, &sn)).await.ok()?;
                secrets.push((derived, active));
            }
        }
    }
    let mut proven = None;
    for (secret, active) in secrets {
        let keys = secure::derive_keys(&secret, &cn, &sn);
        if secure::verify_mac(&keys.auth, secure::LABEL_CLIENT, &cn, &sn, &proof) {
            proven = Some(Proven { keys, used_active_code: active });
            break;
        }
    }
    let Some(proven) = proven else {
        agent.auth_fail_limiter().check(&ip);
        let code = match mode {
            Hello2Mode::Device => ErrorCode::Unauthorized,
            Hello2Mode::PairQr | Hello2Mode::PairCode => {
                agent.pairing().record_failure();
                ErrorCode::PairCodeInvalid
            }
        };
        link.fatal(code, "authentication failed", CLOSE_UNAUTHORIZED, None).await;
        return None;
    };
    let server_mac = secure::mac(&proven.keys.auth, secure::LABEL_SERVER, &cn, &sn);
    if !link.send(&ServerMsg::AuthOk { mac: URL_SAFE_NO_PAD.encode(server_mac) }).await {
        return None;
    }
    Some(proven)
}

/// Registers an authorized device, sends `welcome` and runs the session.
async fn run_device(agent: Agent, remote: SocketAddr, mut link: Link, device_id: String, client: ClientInfo) {
    // Subscribe BEFORE the snapshot so no change can fall between them; older patches are skipped later.
    let (guard, mut kicked) = agent.register_connection(&device_id, &client.name, remote);
    let mut patches = agent.store().subscribe();
    let (snapshot_rev, state) = agent.store().snapshot();

    let version = if link.sec.is_some() { crate::PROTOCOL_VERSION } else { 1 };
    let welcome = ServerMsg::Welcome {
        v: version,
        agent: AgentInfo { name: agent.config().agent_name, version: crate::VERSION.to_string() },
        device_id: device_id.clone(),
        rev: snapshot_rev,
        state,
    };
    if !link.send(&welcome).await {
        return;
    }
    let name = agent.devices().name_of(&device_id).unwrap_or_else(|| client.name.clone());
    let who = Peer { device_id, name, remote, since: std::time::Instant::now() };
    session(&agent, &mut link, &mut patches, &mut kicked, snapshot_rev, &who).await;
    agent.apk_release(&who.device_id, &who.remote.to_string());
    agent.voice().device_gone(&who.device_id);
    drop(guard);
}

async fn version_error(link: &mut Link, v: u32) {
    let msg = format!("unsupported protocol version {v}");
    link.fatal(ErrorCode::ProtocolVersion, &msg, CLOSE_VERSION, Some(SUPPORTED_VERSIONS.to_vec())).await;
}

async fn session(
    agent: &Agent,
    link: &mut Link,
    patches: &mut tokio::sync::broadcast::Receiver<crate::state::Patch>,
    kicked: &mut tokio::sync::oneshot::Receiver<()>,
    snapshot_rev: u64,
    who: &Peer,
) {
    let idle = sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);
    // Per-socket press limit; press tasks answer through `out_tx` so reading is never blocked.
    let press_limiter = RateLimiter::new(PRESS_PER_SECOND, Duration::from_secs(1));
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ServerMsg>();
    loop {
        tokio::select! {
            Some(msg) = out_rx.recv() => {
                if !link.send(&msg).await { return; }
            }
            patch = patches.recv() => match patch {
                Ok(p) if p.rev > snapshot_rev => {
                    let msg = ServerMsg::Patch { rev: p.rev, section: p.section.to_string(), value: (*p.value).clone() };
                    if !link.send(&msg).await { return; }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => return link.close(CLOSE_LAGGED).await,
                Err(RecvError::Closed) => return,
            },
            _ = &mut *kicked => {
                return link.fatal(ErrorCode::DeviceRevoked, "device was revoked", CLOSE_REVOKED, None).await;
            }
            _ = &mut idle => return link.close(CLOSE_GOING_AWAY).await,
            frame = link.ws.recv() => {
                let Some(Ok(frame)) = frame else { return };
                idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
                match frame {
                    Message::Binary(bytes) if link.sec.is_some() => {
                        let opened = link.sec.as_mut().map(|s| s.rx.open(&bytes));
                        let Some(Ok(plain)) = opened else {
                            // Replay, gap, reorder or a changed byte: the connection is over.
                            return link.close(CLOSE_BAD_FRAME).await;
                        };
                        if plain.len() > MAX_MESSAGE_BYTES {
                            return link.close(CLOSE_TOO_BIG).await;
                        }
                        let Ok(text) = String::from_utf8(plain) else { return link.close(CLOSE_BAD_FRAME).await };
                        if !handle_text(agent, link, &text, &press_limiter, &out_tx, who).await { return; }
                    }
                    // After the handshake every v2 frame is encrypted: plain text is a protocol violation.
                    Message::Text(_) if link.sec.is_some() => return link.close(CLOSE_BAD_FRAME).await,
                    Message::Text(text) => {
                        if !handle_text(agent, link, text.as_str(), &press_limiter, &out_tx, who).await { return; }
                    }
                    Message::Binary(_) => {
                        if !link.send(&error_msg(ErrorCode::BadMessage, "binary frames are not supported")).await { return; }
                    }
                    Message::Ping(_) | Message::Pong(_) => {}
                    Message::Close(_) => return,
                }
            }
        }
    }
}

/// Handles one authorized message. Returns false if the socket is gone.
async fn handle_text(
    agent: &Agent,
    link: &mut Link,
    text: &str,
    press_limiter: &RateLimiter,
    out: &mpsc::UnboundedSender<ServerMsg>,
    who: &Peer,
) -> bool {
    match serde_json::from_str::<ClientMsg>(text) {
        Ok(ClientMsg::Ping { ts }) => link.send(&ServerMsg::Pong { ts }).await,
        Ok(ClientMsg::NotifDismiss { notif_id }) => {
            agent.store().update_notifications(|list: &mut Vec<Notification>| list.retain(|n| n.id != notif_id));
            true
        }
        Ok(ClientMsg::ApkGet { offset }) => {
            // The APK goes only through the encrypted channel to a paired device (T20.4).
            if link.sec.is_none() {
                return link.send(&error_msg(ErrorCode::BadMessage, "the update needs the encrypted channel")).await;
            }
            match agent.apk_chunk(&who.device_id, &who.remote.to_string(), offset) {
                Ok(chunk) => link.send(&chunk).await,
                Err(ApkGetError::NoUpdate) => link.send(&error_msg(ErrorCode::NotFound, "no update to download")).await,
                Err(ApkGetError::Busy) => link.send(&error_msg(ErrorCode::RateLimited, "another download of this device is running")).await,
                Err(ApkGetError::BadOffset) => link.send(&error_msg(ErrorCode::BadMessage, "bad offset")).await,
            }
        }
        // M21: voice from the phone. Audio and answers only (S24); what to do is decided by the PC.
        Ok(
            msg @ (ClientMsg::VoiceStart { .. }
            | ClientMsg::VoiceChunk { .. }
            | ClientMsg::VoiceEnd { .. }
            | ClientMsg::VoiceCancel { .. }
            | ClientMsg::VoiceConfirm { .. }),
        ) => {
            if link.sec.is_none() {
                return link.send(&error_msg(ErrorCode::BadMessage, "voice needs the encrypted channel")).await;
            }
            let device = who.device_id.as_str();
            let voice = agent.voice();
            let result = match msg {
                ClientMsg::VoiceStart { id } => voice.start(device, &id),
                ClientMsg::VoiceChunk { id, seq, data } => voice.chunk(device, &id, seq, &data, std::time::Instant::now()),
                ClientMsg::VoiceEnd { id } => Ok(voice.end(device, &id)),
                ClientMsg::VoiceCancel { id } => Ok(voice.cancel(device, &id)),
                ClientMsg::VoiceConfirm { id, yes } => Ok(voice.confirm(device, &id, yes)),
                _ => Ok(()),
            };
            match result {
                Ok(()) => true,
                Err(e) => link.send(&error_msg(e.code(), &e.message())).await,
            }
        }
        Ok(ClientMsg::NotifOpen { notif_id }) => {
            // Only the id of the notification comes from the phone (ARCHITECTURE.md S21); the program is the agent's own record.
            tracing::info!("notif_open {notif_id} device={}", who.device_id);
            agent.notif_open(&notif_id).await;
            true
        }
        Ok(ClientMsg::PermAnswer { req_id, decision, reply }) => {
            tracing::info!("perm_answer {req_id} {decision:?} device={}", who.device_id);
            agent.answer_permission(&who.device_id, &req_id, decision, reply.map(usize::from));
            true
        }
        Ok(ClientMsg::Press { id, action_id }) => {
            // Audit trail (ARCHITECTURE.md 6.4, S15): every press is logged, whatever its outcome.
            tracing::info!("press {action_id} device={} remote={} id={id}", who.device_id, who.remote);
            if !press_limiter.check("press") {
                let error = ErrorBody { code: ErrorCode::RateLimited, message: "too many presses".to_string() };
                return link.send(&ServerMsg::PressResult { id, ok: false, error: Some(error) }).await;
            }
            let agent = agent.clone();
            let out = out.clone();
            let phone = PhoneCtx { device_name: who.name.clone(), connected_for: who.since.elapsed() };
            tokio::spawn(async move {
                let error = agent.execute_for(&action_id, Some(&phone)).await.err().map(|(code, message)| ErrorBody { code, message });
                let _ = out.send(ServerMsg::PressResult { id, ok: error.is_none(), error });
            });
            true
        }
        Ok(ClientMsg::Pair { .. }) | Ok(ClientMsg::Hello { .. }) | Ok(ClientMsg::Hello2 { .. }) | Ok(ClientMsg::Auth { .. }) => true,
        Err(_) => link.send(&error_msg(ErrorCode::BadMessage, "invalid message")).await,
    }
}
