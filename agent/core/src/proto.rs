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
    Pair { v: u32, code: String, client: ClientInfo },
    Hello { v: u32, token: String, client: ClientInfo },
    Press { id: u64, action_id: String },
    NotifDismiss { notif_id: String },
    /// M17: the phone asks to open the program a Windows notification came from; only the id of the notification is sent.
    NotifOpen { notif_id: String },
    Ping { ts: i64 },
    /// M16: the answer of the phone to a permission question (DATA_MODEL.md 6.8).
    PermAnswer {
        req_id: String,
        decision: PermDecision,
        #[serde(default)]
        reply: Option<u8>,
    },
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
