//! `/ws` endpoint: handshake, welcome, patches, ping, revocation (PROTOCOL.md 1, 4, 6).

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::sync::broadcast::error::RecvError;
use tokio::time::{sleep, timeout, Instant};

use crate::agent::Agent;
use crate::model::Notification;
use crate::proto::{AgentInfo, ClientMsg, ErrorBody, ErrorCode, ServerMsg};

const MAX_MESSAGE_BYTES: usize = 4096;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const SUPPORTED_VERSIONS: [u32; 1] = [1];

const CLOSE_GOING_AWAY: u16 = 1001;
const CLOSE_LAGGED: u16 = 1011;
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
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection(agent, remote, socket))
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    let text = serde_json::to_string(msg).expect("server message serializes");
    socket.send(Message::Text(text.into())).await.is_ok()
}

/// Sends a close frame, then waits briefly for the peer so earlier frames are not lost to a TCP reset.
async fn close(socket: &mut WebSocket, code: u16) {
    let _ = socket.send(Message::Close(Some(CloseFrame { code, reason: "".into() }))).await;
    let _ = timeout(Duration::from_secs(1), async { while let Some(Ok(_)) = socket.recv().await {} }).await;
}

async fn fatal(socket: &mut WebSocket, code: ErrorCode, message: &str, close_code: u16, supported: Option<Vec<u32>>) {
    send(socket, &ServerMsg::Error { code, message: message.to_string(), fatal: true, supported }).await;
    close(socket, close_code).await;
}

fn error_msg(code: ErrorCode, message: &str) -> ServerMsg {
    ServerMsg::Error { code, message: message.to_string(), fatal: false, supported: None }
}

/// Next text frame, skipping ping/pong. `None` = closed, errored, or a non-text frame.
async fn recv_first_text(socket: &mut WebSocket) -> Option<String> {
    loop {
        match socket.recv().await? {
            Ok(Message::Text(t)) => return Some(t.as_str().to_string()),
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            _ => return None,
        }
    }
}

async fn connection(agent: Agent, remote: SocketAddr, mut socket: WebSocket) {
    let first = timeout(HANDSHAKE_TIMEOUT, recv_first_text(&mut socket)).await.ok().flatten();
    let unauthorized = "authentication required";
    let Some(first) = first else {
        return fatal(&mut socket, ErrorCode::Unauthorized, unauthorized, CLOSE_UNAUTHORIZED, None).await;
    };

    let (device_id, client, paired_token) = match serde_json::from_str::<ClientMsg>(&first) {
        Ok(ClientMsg::Pair { v, code, client }) => {
            if !agent.pair_limiter().check(&remote.ip().to_string()) {
                return fatal(&mut socket, ErrorCode::RateLimited, "too many pairing attempts", CLOSE_RATE, None).await;
            }
            if !SUPPORTED_VERSIONS.contains(&v) {
                return version_error(&mut socket, v).await;
            }
            if !agent.pairing().try_consume(&code) {
                return fatal(&mut socket, ErrorCode::PairCodeInvalid, "pairing code rejected", CLOSE_UNAUTHORIZED, None).await;
            }
            let (record, token) = agent.devices().create(&client.name);
            (record.id, client, Some(token))
        }
        Ok(ClientMsg::Hello { v, token, client }) => {
            if !SUPPORTED_VERSIONS.contains(&v) {
                return version_error(&mut socket, v).await;
            }
            let Some(device_id) = agent.devices().verify(&token) else {
                return fatal(&mut socket, ErrorCode::Unauthorized, "invalid device token", CLOSE_UNAUTHORIZED, None).await;
            };
            agent.devices().touch(&device_id);
            (device_id, client, None)
        }
        _ => return fatal(&mut socket, ErrorCode::Unauthorized, unauthorized, CLOSE_UNAUTHORIZED, None).await,
    };

    // Subscribe BEFORE the snapshot so no change can fall between them; older patches are skipped later.
    let (guard, mut kicked) = agent.register_connection(&device_id, &client.name, remote);
    let mut patches = agent.store().subscribe();
    let (snapshot_rev, state) = agent.store().snapshot();

    if let Some(token) = paired_token {
        if !send(&mut socket, &ServerMsg::Paired { device_id: device_id.clone(), token }).await {
            return;
        }
    }
    let welcome = ServerMsg::Welcome {
        v: crate::PROTOCOL_VERSION,
        agent: AgentInfo { name: agent.config().agent_name, version: crate::VERSION.to_string() },
        device_id: device_id.clone(),
        rev: snapshot_rev,
        state,
    };
    if !send(&mut socket, &welcome).await {
        return;
    }
    session(&agent, &mut socket, &mut patches, &mut kicked, snapshot_rev).await;
    drop(guard);
}

async fn version_error(socket: &mut WebSocket, v: u32) {
    let msg = format!("unsupported protocol version {v}");
    fatal(socket, ErrorCode::ProtocolVersion, &msg, CLOSE_VERSION, Some(SUPPORTED_VERSIONS.to_vec())).await;
}

async fn session(
    agent: &Agent,
    socket: &mut WebSocket,
    patches: &mut tokio::sync::broadcast::Receiver<crate::state::Patch>,
    kicked: &mut tokio::sync::oneshot::Receiver<()>,
    snapshot_rev: u64,
) {
    let idle = sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);
    loop {
        tokio::select! {
            patch = patches.recv() => match patch {
                Ok(p) if p.rev > snapshot_rev => {
                    let msg = ServerMsg::Patch { rev: p.rev, section: p.section.to_string(), value: (*p.value).clone() };
                    if !send(socket, &msg).await { return; }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => return close(socket, CLOSE_LAGGED).await,
                Err(RecvError::Closed) => return,
            },
            _ = &mut *kicked => {
                return fatal(socket, ErrorCode::DeviceRevoked, "device was revoked", CLOSE_REVOKED, None).await;
            }
            _ = &mut idle => return close(socket, CLOSE_GOING_AWAY).await,
            frame = socket.recv() => {
                let Some(Ok(frame)) = frame else { return };
                idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
                match frame {
                    Message::Text(text) => {
                        if !handle_text(agent, socket, text.as_str()).await { return; }
                    }
                    Message::Binary(_) => {
                        if !send(socket, &error_msg(ErrorCode::BadMessage, "binary frames are not supported")).await { return; }
                    }
                    Message::Ping(_) | Message::Pong(_) => {}
                    Message::Close(_) => return,
                }
            }
        }
    }
}

/// Handles one authorized text frame. Returns false if the socket is gone.
async fn handle_text(agent: &Agent, socket: &mut WebSocket, text: &str) -> bool {
    match serde_json::from_str::<ClientMsg>(text) {
        Ok(ClientMsg::Ping { ts }) => send(socket, &ServerMsg::Pong { ts }).await,
        Ok(ClientMsg::NotifDismiss { notif_id }) => {
            agent.store().update_notifications(|list: &mut Vec<Notification>| list.retain(|n| n.id != notif_id));
            true
        }
        Ok(ClientMsg::Press { id, action_id }) => {
            // Real execution arrives in T3.1.
            let error = ErrorBody { code: ErrorCode::UnknownAction, message: format!("action '{action_id}' is not defined") };
            send(socket, &ServerMsg::PressResult { id, ok: false, error: Some(error) }).await
        }
        Ok(ClientMsg::Pair { .. }) | Ok(ClientMsg::Hello { .. }) => true,
        Err(_) => send(socket, &error_msg(ErrorCode::BadMessage, "invalid message")).await,
    }
}
