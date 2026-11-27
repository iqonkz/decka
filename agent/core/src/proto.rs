//! WebSocket and HTTP message types (PROTOCOL.md section 3).

use serde::{Deserialize, Serialize};

use crate::model::{NotifLevel, StatusState, VarValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Unauthorized,
    PairCodeInvalid,
    DeviceRevoked,
    ProtocolVersion,
    BadMessage,
    UnknownAction,
    ActionFailed,
    RateLimited,
    PayloadTooLarge,
    NotFound,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub app: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ClientMsg {
    /// M18 (PROTOCOL.md 10): first frame of protocol v2; `cn` is 16 random bytes in base64url.
    Hello2 {
        v: u32,
        mode: Hello2Mode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device_id: Option<String>,
        cn: String,
        client: ClientInfo,
    },
    /// M18: the client proof of the v2 handshake (HMAC, base64url).
    Auth { mac: String },
    Pair { v: u32, code: String, client: ClientInfo },
    Hello { v: u32, token: String, client: ClientInfo },
    Press { id: u64, action_id: String },
    NotifDismiss { notif_id: String },
    /// M17: the phone asks to open the program a Windows notification came from; only the id of the notification is sent.
    NotifOpen { notif_id: String },
    /// T22.13 (PROTOCOL.md 3): a tap on an item of a list widget; only the ids the agent issued are taken.
    ListToggle { list_id: String, item_id: String },
    /// T29.P (PROTOCOL.md 3): the page the phone shows now. Data, not a command: the PC only remembers it.
    PageShown { page_id: String },
    /// T24.2 (PROTOCOL.md 3): the user moves the lyrics of the current track by half a second. Data, not a command: the key
    /// is one the PC issued (any other is ignored) and the step is ±500 (anything else is refused).
    LyricsOffset { track_key: String, delta_ms: i64 },
    Ping { ts: i64 },
    /// T20.4: the next piece of the new APK (`offset` is a multiple of 3000). Only inside the encrypted channel.
    ApkGet { offset: u64 },
    /// M21 (PROTOCOL.md 3): voice from the phone; audio and answers only, inside the encrypted channel.
    VoiceStart { id: String },
    VoiceChunk { id: String, seq: u64, data: String },
    VoiceEnd { id: String },
    VoiceCancel { id: String },
    VoiceConfirm { id: String, yes: bool },
    /// M16: the answer of the phone to a permission question (DATA_MODEL.md 6.8).
    PermAnswer {
        req_id: String,
        decision: PermDecision,
        #[serde(default)]
        reply: Option<u8>,
    },
}

/// How a v2 connection proves itself: a paired device, the QR of a pairing, or the typed pairing code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hello2Mode {
    Device,
    PairQr,
    PairCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermDecision {
    Allow,
    /// Allow, and remember the rule that came with the question ("Always allow").
    #[serde(rename = "allow_always")]
    AllowAlways,
    Deny,
    /// "Deny and say": the number of the ready answer comes with it (`reply`).
    Reply,
    /// "Answer on PC": Claude Code asks in the terminal.
    Pc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ServerMsg {
    /// M18: answer to `hello2`; `sn` is 16 random bytes in base64url.
    Challenge { sn: String, agent: AgentInfo },
    /// M18: the server proof of the v2 handshake (HMAC, base64url).
    AuthOk { mac: String },
    Paired {
        device_id: String,
        token: String,
    },
    Welcome {
        v: u32,
        agent: AgentInfo,
        device_id: String,
        rev: u64,
        state: serde_json::Value,
    },
    Patch {
        rev: u64,
        section: String,
        value: serde_json::Value,
    },
    PressResult {
        id: u64,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<ErrorBody>,
    },
    Pong {
        ts: i64,
    },
    /// T20.4: a piece of the new APK; `data` is base64 of at most 3000 bytes, `sha256` is lower-case hex of the whole file.
    ApkChunk {
        offset: u64,
        data: String,
        total: u64,
        sha256: String,
    },
    Error {
        code: ErrorCode,
        message: String,
        fatal: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        supported: Option<Vec<u32>>,
    },
}

// ===== HTTP =====

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthRes {
    pub name: String,
    pub version: String,
    pub protocol: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotifyReq {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<NotifLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyRes {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReq {
    pub id: String,
    pub state: StatusState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_sec: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarReq {
    pub value: VarValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OkRes {
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpError {
    pub error: ErrorBody,
}
