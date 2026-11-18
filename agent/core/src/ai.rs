//! AI coding helpers (DATA_MODEL.md 6.8): the limits Claude Code and Codex write on this PC, and the permission questions of
//! Claude Code answered from the phone. Everything is local: nothing here talks to the internet and no OAuth token is read.

use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::model::VarValue;
use crate::proto::PermDecision;
use crate::ratelimit::RateLimiter;
use crate::state::Store;

/// The `ai` section goes out at most this often (a pending request goes out at once).
pub const PUBLISH_EVERY: Duration = Duration::from_secs(2);
pub const MAX_SESSIONS: usize = 6;
pub const SESSION_TTL_MS: u64 = 10 * 60 * 1000;
pub const MAX_PENDING: usize = 5;
const MAX_SUMMARY: usize = 300;
/// Bodies of the Claude Code endpoints (PROTOCOL.md 5).
pub const MAX_HOOK_BODY: usize = 1024 * 1024;
const CODEX_TAIL_BYTES: u64 = 256 * 1024;
const CODEX_MAX_DAYS: usize = 14;

// ===== section types (PROTOCOL.md 3) =====

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiWindow {
    pub label: String,
    pub pct: f64,
    pub resets_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub windows: Vec<AiWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSession {
    pub id: String,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctx_pct: Option<u32>,
    pub last_seen: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermRequest {
    pub req_id: String,
    /// `claude` | `codex`: who asks.
    pub provider: String,
    pub tool: String,
    pub summary: String,
    pub project: String,
    pub session_id: String,
    pub expires_at: u64,
    /// What "Always allow" would remember for this question; absent when nothing safe can be remembered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always: Option<AlwaysOffer>,
    /// Ready answers for "Deny and say": the phone shows them as buttons and sends the number; the text is told to the tool.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub replies: Vec<String>,
}

/// The answers offered with "Deny and say" when the user has not made their own.
pub fn default_replies() -> Vec<String> {
    ["Not this way: please try another approach.", "Show me your plan first and wait for my answer.", "Ask me before you change more files."].iter().map(|s| s.to_string()).collect()
}

/// `kind` is `command` (the first words of a plain shell command), `project` (the same tool inside one project folder), `host`
/// (WebFetch to one site) or `tool` (one MCP tool); `value` is the command words, the project name, the host or the tool.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlwaysOffer {
    pub kind: String,
    pub value: String,
}

/// One answered question, newest first in `AiInfo.history` (kept in memory only; it holds commands and paths).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermLog {
    pub ts: u64,
    pub provider: String,
    pub tool: String,
    pub summary: String,
    pub project: String,
    /// `allow` | `deny` | `pc` (the terminal decided, or nobody answered in time) | `auto` (a rule of "Always allow").
    pub decision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiInfo {
    pub claude: Option<AiProvider>,
    pub codex: Option<AiProvider>,
    pub sessions: Vec<AiSession>,
    pub pending: Vec<PermRequest>,
    pub history: Vec<PermLog>,
}

pub const MAX_HISTORY: usize = 30;
pub const MAX_RULES: usize = 50;

// ===== "Always allow" rules =====

/// A rule made by "Always allow" (kept in config.json, shown and removable in the window).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRule {
    pub id: String,
    /// `claude` | `codex`.
    pub provider: String,
    /// The tool name as the hook sends it (`Bash`, `Edit`, `WebFetch`, `mcp__server__tool`).
    pub tool: String,
    pub kind: String,
    /// `command`: the first words; `project`: the full folder; `host`: the host; `tool`: empty.
    pub value: String,
    pub created_at: u64,
}

/// Programs that run anything or delete things: no rule is offered for them.
const NEVER_REMEMBER: &[&str] = &[
    "rm", "del", "erase", "rmdir", "rd", "remove-item", "ri", "format", "diskpart", "shutdown", "restart-computer", "stop-computer", "sudo", "su", "runas", "curl", "curl.exe", "wget", "iwr", "irm",
    "invoke-webrequest", "invoke-restmethod", "iex", "invoke-expression", "powershell", "powershell.exe", "pwsh", "cmd", "cmd.exe", "bash", "sh", "zsh", "wsl", "eval", "exec", "start-process", "start",
    "npx", "pnpx", "bunx", "reg", "regedit", "netsh", "sc", "schtasks", "taskkill", "kill", "stop-process", "chmod", "chown", "icacls", "takeown", "mklink", "ssh", "scp", "ftp", "nc", "ncat",
];

const SHELL_META: &[char] = &['\n', '\r', ';', '&', '|', '>', '<', '`', '$', '(', ')', '{', '}'];

fn command_of(hook: &Value) -> &str {
    hook.get("tool_input").and_then(|i| str_field(i, "command")).unwrap_or("").trim()
}

/// The words of a command that a rule may remember: a plain command (no chaining, redirection or substitution) of one or two words.
fn rule_words(command: &str) -> Option<Vec<String>> {
    if command.is_empty() || command.contains(SHELL_META) {
        return None;
    }
    let words: Vec<&str> = command.split_whitespace().collect();
    let first = words.first()?.to_lowercase();
    if NEVER_REMEMBER.contains(&first.as_str()) || first.contains(['/', '\\']) {
        return None;
    }
    let plain = |w: &str| !w.starts_with('-') && !w.contains(['/', '\\', ':', '.']);
    match words.as_slice() {
        [_] => Some(vec![first]),
        // `npm run build`: the script is part of the rule, because `run` alone would allow every script of the project.
        [_, second, third, ..] if second.eq_ignore_ascii_case("run") && plain(third) => Some(vec![first, "run".to_string(), third.to_lowercase()]),
        [_, second, ..] if plain(second) && !second.eq_ignore_ascii_case("run") => Some(vec![first, second.to_lowercase()]),
        _ => None,
    }
}

/// `path` inside the folder `cwd` (no `..`), as the text after the folder; `None` when it is outside.
fn inside_folder(path: &str, cwd: &str) -> Option<String> {
    if cwd.is_empty() || path.is_empty() || path.starts_with('~') {
        return None;
    }
    let p = path.replace('\\', "/");
    let absolute = p.starts_with('/') || p.as_bytes().get(1) == Some(&b':');
    let rel = if absolute {
        let r = relative_to(path, cwd);
        if r == path {
            return None;
        }
        r
    } else {
        path.to_string()
    };
    if rel.replace('\\', "/").split('/').any(|seg| seg == "..") {
        return None;
    }
    Some(rel)
}

fn url_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let host = authority.split(':').next()?.to_lowercase();
    if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return None;
    }
    Some(host)
}

/// What "Always allow" would remember for this hook call: (tool name, kind, value); `None` = nothing safe.
pub fn draft_rule(hook: &Value) -> Option<(String, &'static str, String)> {
    let name = str_field(hook, "tool_name")?;
    let input = hook.get("tool_input").unwrap_or(&Value::Null);
    let cwd = str_field(hook, "cwd").unwrap_or("");
    match name {
        "Bash" | "PowerShell" => Some((name.to_string(), "command", rule_words(command_of(hook))?.join(" "))),
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "Read" => {
            let path = str_field(input, "file_path").or_else(|| str_field(input, "notebook_path"))?;
            inside_folder(path, cwd)?;
            Some((name.to_string(), "project", cwd.to_string()))
        }
        "WebFetch" => Some((name.to_string(), "host", url_host(str_field(input, "url")?)?)),
        n if n.starts_with("mcp__") => Some((n.to_string(), "tool", String::new())),
        _ => None,
    }
}

/// Whether a stored rule covers this hook call of `provider`.
pub fn rule_matches(rule: &AiRule, provider: &str, hook: &Value) -> bool {
    if rule.provider != provider || str_field(hook, "tool_name") != Some(rule.tool.as_str()) {
        return false;
    }
    let input = hook.get("tool_input").unwrap_or(&Value::Null);
    let cwd = str_field(hook, "cwd").unwrap_or("");
    match rule.kind.as_str() {
        "command" => {
            // Only a plain command can match, and it may be longer than the rule (`git status -s`): the words of the rule are its first words.
            if rule_words(command_of(hook)).is_none() {
                return false;
            }
            let actual: Vec<String> = command_of(hook).split_whitespace().map(str::to_lowercase).collect();
            let wanted: Vec<&str> = rule.value.split_whitespace().collect();
            !wanted.is_empty() && actual.len() >= wanted.len() && actual.iter().zip(&wanted).all(|(a, w)| a == w)
        }
        "project" => {
            let path = str_field(input, "file_path").or_else(|| str_field(input, "notebook_path")).unwrap_or("");
            cwd.replace('\\', "/").eq_ignore_ascii_case(&rule.value.replace('\\', "/")) && inside_folder(path, cwd).is_some()
        }
        "host" => str_field(input, "url").and_then(url_host).is_some_and(|h| h == rule.value),
        "tool" => true,
        _ => false,
    }
}

// ===== text helpers =====

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

fn one_line(s: &str) -> String {
    s.replace("\r\n", " ⏎ ").replace(['\n', '\r'], " ⏎ ")
}

fn basename(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    truncate(trimmed.rsplit(['/', '\\']).next().unwrap_or(""), 60)
}

fn relative_to(path: &str, cwd: &str) -> String {
    if cwd.is_empty() {
        return path.to_string();
    }
    let p = path.replace('\\', "/");
    let c = cwd.replace('\\', "/");
    let c = c.trim_end_matches('/');
    let inside = p.get(..c.len()).is_some_and(|head| head.eq_ignore_ascii_case(c)) && p.as_bytes().get(c.len()) == Some(&b'/');
    if inside {
        p[c.len() + 1..].to_string()
    } else {
        path.to_string()
    }
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn first_string(v: &Value) -> Option<&str> {
    v.as_object()?.values().find_map(Value::as_str)
}

/// What the question is about: (tool as shown, one-line summary of at most 300 characters). DATA_MODEL.md 6.8.
pub fn summarize(hook: &Value) -> (String, String) {
    let name = str_field(hook, "tool_name").unwrap_or("");
    let input = hook.get("tool_input").unwrap_or(&Value::Null);
    let cwd = str_field(hook, "cwd").unwrap_or("");
    let (tool, raw) = match name {
        // `apply_patch` is the edit tool of Codex: its `command` is the patch text.
        "Bash" | "PowerShell" | "apply_patch" => (name.to_string(), str_field(input, "command").unwrap_or("").to_string()),
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "Read" => {
            let path = str_field(input, "file_path").or_else(|| str_field(input, "notebook_path")).unwrap_or("");
            (name.to_string(), relative_to(path, cwd))
        }
        "WebFetch" => (name.to_string(), str_field(input, "url").unwrap_or("").to_string()),
        "WebSearch" => (name.to_string(), str_field(input, "query").unwrap_or("").to_string()),
        n if n.starts_with("mcp__") => {
            let rest = &n[5..];
            let tool = match rest.split_once("__") {
                Some((server, tool)) => format!("{server}: {tool}"),
                None => rest.to_string(),
            };
            (tool, first_string(input).unwrap_or("").to_string())
        }
        _ => (name.to_string(), first_string(input).unwrap_or("").to_string()),
    };
    (truncate(&tool, 60), truncate(&one_line(&raw), MAX_SUMMARY))
}

fn pct1(x: f64) -> f64 {
    (x.clamp(0.0, 100.0) * 10.0).round() / 10.0
}

// ===== statusline =====

/// The few fields of a Claude Code statusline body that Decka uses; the rest is ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct Statusline {
    pub session: Option<String>,
    pub project: String,
    pub model: Option<String>,
    pub ctx_pct: Option<u32>,
    /// `Some` only when the body has usable `rate_limits` (an API key or the start of a session has none).
    pub windows: Option<Vec<AiWindow>>,
}

pub fn parse_statusline(body: &[u8]) -> Option<Statusline> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let window = |key: &str, label: &str| -> Option<AiWindow> {
        let w = v.pointer(&format!("/rate_limits/{key}"))?;
        let pct = w.get("used_percentage")?.as_f64()?;
        let resets = w.get("resets_at")?.as_f64()?;
        Some(AiWindow { label: label.to_string(), pct: pct1(pct), resets_at: (resets * 1000.0) as u64 })
    };
    let windows: Vec<AiWindow> = [window("five_hour", "5h"), window("seven_day", "7d")].into_iter().flatten().collect();
    Some(Statusline {
        session: str_field(&v, "session_id").map(|s| s.chars().take(8).collect()),
        project: basename(str_field(&v, "cwd").unwrap_or("")),
        model: v.pointer("/model/display_name").and_then(Value::as_str).map(|m| truncate(m, 40)),
        ctx_pct: v.pointer("/context_window/used_percentage").and_then(Value::as_f64).map(|p| p.clamp(0.0, 100.0).round() as u32),
        windows: if windows.is_empty() { None } else { Some(windows) },
    })
}

/// The line Claude Code shows in the terminal: `Decka · 5h 23% · 7d 41%`.
pub fn status_text(claude: Option<&AiProvider>) -> String {
    let mut text = "Decka".to_string();
    for w in claude.map(|c| c.windows.as_slice()).unwrap_or(&[]) {
        text.push_str(&format!(" · {} {:.0}%", w.label, w.pct));
    }
    text
}

// ===== the hub =====

struct Pending {
    req: PermRequest,
    tx: Option<oneshot::Sender<(PermDecision, Option<String>)>>,
    /// What "Always allow" would remember: (tool name, kind, value).
    draft: Option<(String, &'static str, String)>,
}

#[derive(Default)]
struct Data {
    history: VecDeque<PermLog>,
    rules: Vec<AiRule>,
    /// A rule was added or removed since the agent last saved them.
    rules_dirty: bool,
    active: bool,
    claude: Option<AiProvider>,
    codex: Option<AiProvider>,
    sessions: Vec<AiSession>,
    pending: Vec<Pending>,
    last_seen: Option<u64>,
    dirty: bool,
    last_pub: Option<Instant>,
}

fn num(x: f64) -> VarValue {
    VarValue::Num(serde_json::Number::from(x.round() as i64))
}

impl Data {
    fn section(&self) -> Value {
        if !self.active {
            return Value::Null;
        }
        let info = AiInfo {
            claude: self.claude.clone(),
            codex: self.codex.clone(),
            sessions: self.sessions.clone(),
            pending: self.pending.iter().map(|p| p.req.clone()).collect(),
            history: self.history.iter().cloned().collect(),
        };
        serde_json::to_value(info).expect("ai section serializes")
    }

    /// `ai.claude.5h`, `ai.claude.7d`, `ai.codex.primary`, `ai.codex.secondary`, `ai.pending`; a missing value removes the variable.
    fn apply_vars(&self, vars: &mut BTreeMap<String, VarValue>) {
        vars.retain(|k, _| !k.starts_with("ai."));
        if !self.active {
            return;
        }
        for w in self.claude.iter().flat_map(|c| &c.windows) {
            if w.label == "5h" || w.label == "7d" {
                vars.insert(format!("ai.claude.{}", w.label), num(w.pct));
            }
        }
        for (name, w) in ["primary", "secondary"].iter().zip(self.codex.iter().flat_map(|c| &c.windows)) {
            vars.insert(format!("ai.codex.{name}"), num(w.pct));
        }
        vars.insert("ai.pending".to_string(), VarValue::Num(serde_json::Number::from(self.pending.len() as u64)));
    }
}

pub struct AiHub {
    store: Store,
    data: Mutex<Data>,
    limiter: RateLimiter,
}

/// What the permission endpoint needs to know from the agent.
pub struct PermOpts {
    /// `claude` | `codex`.
    pub provider: &'static str,
    /// The ready answers of "Deny and say" (at most 4).
    pub replies: Vec<String>,
    pub approvals: bool,
    pub phones: usize,
    pub timeout: Duration,
}

/// Removes the request when the HTTP call ends in any way (answer, timeout, or Claude Code hanging up).
struct PendingGuard {
    hub: Arc<AiHub>,
    req_id: String,
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.hub.remove(&self.req_id);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

const ALLOW_BODY: &str = r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#;
const DENY_BODY: &str =
    r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied from the Decka phone panel"}}}"#;

impl AiHub {
    pub fn new(store: Store) -> Arc<AiHub> {
        Arc::new(AiHub { store, data: Mutex::new(Data::default()), limiter: RateLimiter::new(20, Duration::from_secs(1)) })
    }

    /// The own limit of the Claude Code endpoints: 20 requests per second.
    pub fn allow_request(&self) -> bool {
        self.limiter.check("cc")
    }

    fn flush(&self, d: &mut Data, force: bool) {
        let now = Instant::now();
        if !force && d.last_pub.is_some_and(|t| now.duration_since(t) < PUBLISH_EVERY) {
            d.dirty = true;
            return;
        }
        d.last_pub = Some(now);
        d.dirty = false;
        self.store.set("ai", d.section());
        let snapshot: &Data = d;
        self.store.update_vars(|vars| snapshot.apply_vars(vars));
    }

    /// Which integrations are on: both off = the section is `null` and the `ai.*` variables are gone.
    pub fn configure(&self, claude_on: bool, codex_on: bool) {
        let mut d = self.data.lock().unwrap();
        d.active = claude_on || codex_on;
        if !claude_on {
            d.claude = None;
            d.sessions.clear();
        }
        if !codex_on {
            d.codex = None;
        }
        self.flush(&mut d, true);
    }

    /// Sends what was held back by the 2-second rule.
    pub fn tick(&self) {
        let mut d = self.data.lock().unwrap();
        if d.dirty {
            self.flush(&mut d, false);
        }
    }

    /// Once a minute: sessions older than 10 minutes and windows that were reset are dropped.
    pub fn prune(&self, now_ms: u64) {
        let mut guard = self.data.lock().unwrap();
        let d = &mut *guard;
        let before = (d.sessions.len(), d.claude.clone(), d.codex.clone());
        d.sessions.retain(|s| now_ms.saturating_sub(s.last_seen) <= SESSION_TTL_MS);
        for p in [&mut d.claude, &mut d.codex].into_iter().flatten() {
            p.windows.retain(|w| w.resets_at > now_ms);
        }
        if before != (d.sessions.len(), d.claude.clone(), d.codex.clone()) {
            self.flush(d, false);
        }
    }

    pub fn set_codex(&self, codex: Option<AiProvider>) {
        let mut d = self.data.lock().unwrap();
        if d.codex != codex {
            d.codex = codex;
            self.flush(&mut d, false);
        }
    }

    pub fn last_seen(&self) -> Option<u64> {
        self.data.lock().unwrap().last_seen
    }

    pub fn snapshot(&self) -> Value {
        self.data.lock().unwrap().section()
    }

    /// A statusline call: updates the limits and the session and answers with the terminal line at once.
    pub fn statusline(&self, s: Option<Statusline>, now_ms: u64) -> String {
        let mut d = self.data.lock().unwrap();
        d.last_seen = Some(now_ms);
        if let Some(s) = s {
            if let Some(windows) = s.windows {
                d.claude = Some(AiProvider { windows, plan: None, updated_at: now_ms });
            }
            if let Some(id) = s.session {
                let session = AiSession { id: id.clone(), project: s.project, model: s.model, ctx_pct: s.ctx_pct, last_seen: now_ms };
                match d.sessions.iter_mut().find(|x| x.id == id) {
                    Some(old) => *old = session,
                    None => d.sessions.push(session),
                }
                d.sessions.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
                d.sessions.truncate(MAX_SESSIONS);
            }
            self.flush(&mut d, false);
        }
        status_text(d.claude.as_ref())
    }

    fn remove(&self, req_id: &str) {
        let mut d = self.data.lock().unwrap();
        if let Some(i) = d.pending.iter().position(|p| p.req.req_id == req_id) {
            d.pending.remove(i);
            self.flush(&mut d, true);
        }
    }

    /// An answer from the phone. The first answer wins; an unknown or already answered request is ignored.
    /// `AllowAlways` allows this question and remembers the rule that was offered with it.
    pub fn answer(&self, req_id: &str, decision: PermDecision) -> bool {
        self.answer_with(req_id, decision, None)
    }

    /// `reply` is the number of one of the request's ready answers (with `PermDecision::Reply`).
    pub fn answer_with(&self, req_id: &str, decision: PermDecision, reply: Option<usize>) -> bool {
        let mut d = self.data.lock().unwrap();
        let Some(i) = d.pending.iter().position(|p| p.req.req_id == req_id) else { return false };
        let mut pending = d.pending.remove(i);
        let mut decision = decision;
        let mut note = None;
        if decision == PermDecision::Reply {
            // A number that is not on the list is a plain denial.
            note = reply.and_then(|i| pending.req.replies.get(i).cloned());
            decision = PermDecision::Deny;
        }
        if decision == PermDecision::AllowAlways {
            decision = PermDecision::Allow;
            if let Some((tool, kind, value)) = pending.draft.take() {
                let exists = d.rules.iter().any(|r| r.provider == pending.req.provider && r.tool == tool && r.kind == kind && r.value == value);
                if !exists && d.rules.len() < MAX_RULES {
                    let id = hex(&crate::config::random_bytes(6));
                    d.rules.push(AiRule { id, provider: pending.req.provider.clone(), tool, kind: kind.to_string(), value, created_at: crate::auth::now_ms() });
                    d.rules_dirty = true;
                }
            }
        }
        if let Some(tx) = pending.tx.take() {
            let _ = tx.send((decision, note));
        }
        self.flush(&mut d, true);
        true
    }

    /// The rules from config.json (at start and after a restart of the loops).
    pub fn set_rules(&self, rules: Vec<AiRule>) {
        let mut d = self.data.lock().unwrap();
        d.rules = rules;
        d.rules_dirty = false;
    }

    /// The rules to save when `take_rules_dirty` says they changed.
    pub fn rules(&self) -> Vec<AiRule> {
        self.data.lock().unwrap().rules.clone()
    }

    pub fn take_rules_dirty(&self) -> bool {
        std::mem::take(&mut self.data.lock().unwrap().rules_dirty)
    }

    /// Forgets one rule (`None` = all); true when something was removed.
    pub fn remove_rules(&self, id: Option<&str>) -> bool {
        let mut d = self.data.lock().unwrap();
        let before = d.rules.len();
        match id {
            Some(id) => d.rules.retain(|r| r.id != id),
            None => d.rules.clear(),
        }
        let changed = d.rules.len() != before;
        d.rules_dirty |= changed;
        changed
    }

    fn log(&self, d: &mut Data, entry: PermLog) {
        d.history.push_front(entry);
        d.history.truncate(MAX_HISTORY);
    }

    /// The `PermissionRequest` hook: `None` = answer with an empty body (Claude Code asks in the terminal),
    /// `Some(json)` = the decision. The call waits for the phone, the timeout, or the hang-up of the HTTP connection.
    pub async fn permission(self: &Arc<Self>, opts: PermOpts, body: &[u8]) -> Option<String> {
        if !opts.approvals {
            return None;
        }
        let hook: Value = serde_json::from_slice(body).ok()?;
        let now = crate::auth::now_ms();
        let (tool, summary) = summarize(&hook);
        let project = basename(str_field(&hook, "cwd").unwrap_or(""));
        // A rule of "Always allow" answers at once, with or without a phone.
        {
            let mut d = self.data.lock().unwrap();
            d.last_seen = Some(now);
            if d.rules.iter().any(|r| rule_matches(r, opts.provider, &hook)) {
                self.log(&mut d, PermLog { ts: now, provider: opts.provider.to_string(), tool: tool.clone(), summary, project: project.clone(), decision: "auto".to_string() });
                self.flush(&mut d, false);
                tracing::info!("permission auto tool={tool} project={project} -> rule");
                return Some(ALLOW_BODY.to_string());
            }
        }
        if opts.phones == 0 {
            return None;
        }
        let req_id = hex(&crate::config::random_bytes(8));
        let draft = draft_rule(&hook);
        let req = PermRequest {
            req_id: req_id.clone(),
            provider: opts.provider.to_string(),
            tool: tool.clone(),
            summary: summary.clone(),
            replies: opts.replies.clone(),
            project: project.clone(),
            session_id: str_field(&hook, "session_id").unwrap_or("").chars().take(8).collect(),
            expires_at: now + opts.timeout.as_millis() as u64,
            always: draft.as_ref().map(|(t, kind, value)| AlwaysOffer {
                kind: (*kind).to_string(),
                // A project rule shows the folder's name; the rule itself keeps the whole path.
                value: match *kind {
                    "project" => format!("{t}: {project}"),
                    "command" => format!("{t}: {value}"),
                    "host" => format!("{t}: {value}"),
                    _ => t.clone(),
                },
            }),
        };
        let (tx, rx) = oneshot::channel();
        {
            let mut d = self.data.lock().unwrap();
            if d.pending.len() >= MAX_PENDING {
                return None;
            }
            d.pending.push(Pending { req, tx: Some(tx), draft });
            self.flush(&mut d, true);
        }
        let _guard = PendingGuard { hub: self.clone(), req_id: req_id.clone() };
        let decision = tokio::select! {
            answer = rx => answer.unwrap_or((PermDecision::Pc, None)),
            _ = tokio::time::sleep(opts.timeout) => (PermDecision::Pc, None),
        };
        let (decision, note) = decision;
        // Never the summary: it can hold a command or a path.
        tracing::info!("permission {req_id} tool={tool} project={project} -> {decision:?}");
        {
            let mut d = self.data.lock().unwrap();
            let word = match decision {
                PermDecision::Allow | PermDecision::AllowAlways => "allow",
                PermDecision::Deny | PermDecision::Reply => "deny",
                PermDecision::Pc => "pc",
            };
            self.log(&mut d, PermLog { ts: now, provider: opts.provider.to_string(), tool, summary, project, decision: word.to_string() });
            self.flush(&mut d, false);
        }
        match decision {
            PermDecision::Allow | PermDecision::AllowAlways => Some(ALLOW_BODY.to_string()),
            PermDecision::Deny | PermDecision::Reply => Some(match note {
                Some(text) => json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": { "behavior": "deny", "message": text } } }).to_string(),
                None => DENY_BODY.to_string(),
            }),
            PermDecision::Pc => None,
        }
    }
}

// ===== Codex =====

fn window_label(minutes: u64) -> String {
    match minutes {
        300 => "5h".to_string(),
        10080 => "7d".to_string(),
        43200 => "30d".to_string(),
        m => format!("{}h", (m as f64 / 60.0).round() as u64),
    }
}

/// `2026-11-17T10:20:30.123Z` (or with `+hh:mm`) as milliseconds since the epoch.
pub fn parse_timestamp_ms(s: &str) -> Option<u64> {
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-');
    let (y, m, day): (i64, i64, i64) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    let split = time.find(['Z', '+', '-']).unwrap_or(time.len());
    let (clock, zone) = time.split_at(split);
    let mut c = clock.split(':');
    let (hh, mm): (i64, i64) = (c.next()?.parse().ok()?, c.next()?.parse().ok()?);
    let secs: f64 = c.next().unwrap_or("0").parse().ok()?;
    let offset_min = match zone.chars().next() {
        Some(sign @ ('+' | '-')) => {
            let (oh, om) = zone[1..].split_once(':')?;
            let total = oh.parse::<i64>().ok()? * 60 + om.parse::<i64>().ok()?;
            if sign == '+' { total } else { -total }
        }
        _ => 0,
    };
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + day - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    let total_ms = ((days * 24 + hh) * 60 + mm - offset_min) as f64 * 60_000.0 + secs * 1000.0;
    (total_ms >= 0.0).then_some(total_ms as u64)
}

/// The last `rate_limits` of a Codex rollout text (the tail of the file). `updated_at` is filled by the caller.
pub fn parse_codex(text: &str) -> Option<(Vec<AiWindow>, Option<String>)> {
    for line in text.lines().rev() {
        if !line.contains("\"rate_limits\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let Some(limits) = v.pointer("/payload/rate_limits").filter(|l| l.is_object()) else { continue };
        let line_ts = str_field(&v, "timestamp").and_then(parse_timestamp_ms);
        let windows = ["primary", "secondary"]
            .iter()
            .filter_map(|key| {
                let w = limits.get(key).filter(|w| w.is_object())?;
                let pct = w.get("used_percent")?.as_f64()?;
                let minutes = w.get("window_minutes")?.as_u64()?;
                let resets_at = match w.get("resets_at").and_then(Value::as_f64) {
                    Some(sec) => (sec * 1000.0) as u64,
                    None => line_ts? + (w.get("resets_in_seconds")?.as_f64()? * 1000.0) as u64,
                };
                Some(AiWindow { label: window_label(minutes), pct: pct1(pct), resets_at })
            })
            .collect();
        return Some((windows, str_field(limits, "plan_type").map(str::to_string)));
    }
    None
}

fn numeric_dirs_desc(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs.reverse();
    dirs
}

/// The newest `rollout-*.jsonl`: date folders newest first, at most 14 days, never the whole tree.
pub fn latest_rollout(sessions: &Path) -> Option<PathBuf> {
    let mut days = 0;
    for year in numeric_dirs_desc(sessions) {
        for month in numeric_dirs_desc(&year) {
            for day in numeric_dirs_desc(&month) {
                if days >= CODEX_MAX_DAYS {
                    return None;
                }
                days += 1;
                let newest = std::fs::read_dir(&day)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.starts_with("rollout-") && name.ends_with(".jsonl")
                    })
                    .map(|e| e.path())
                    .max();
                if newest.is_some() {
                    return newest;
                }
            }
        }
    }
    None
}

pub fn codex_dir(home: &Path) -> PathBuf {
    home.join(".codex").join("sessions")
}

/// The limits Codex wrote last. `updated_at` is the modification time of the file, so an idle file does not change the section.
pub fn read_codex(home: &Path) -> Option<AiProvider> {
    let file = latest_rollout(&codex_dir(home))?;
    let mut f = std::fs::File::open(&file).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(CODEX_TAIL_BYTES))).ok()?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    let (windows, plan) = parse_codex(&String::from_utf8_lossy(&bytes))?;
    let updated_at = f
        .metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Some(AiProvider { windows, plan, updated_at })
}

/// `%USERPROFILE%` (or the home directory); `None` if the system does not say.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from).or_else(dirs::home_dir)
}

// ===== Claude Code settings.json =====

pub fn settings_path(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

fn is_our_status_line(v: &Value) -> bool {
    str_field(v, "command").is_some_and(|c| c.contains("/api/cc/statusline"))
}

/// Our entry: an `http` hook of Claude Code (`url`) or a `command` hook of Codex that calls `/api/cc/`.
fn is_our_hook(entry: &Value) -> bool {
    entry.get("hooks").and_then(Value::as_array).is_some_and(|hooks| {
        hooks.iter().any(|h| ["url", "command"].iter().any(|k| str_field(h, k).is_some_and(|u| u.contains("/api/cc/"))))
    })
}

/// Codex reads hooks from `%USERPROFILE%\.codex\hooks.json`; the command is the same call as the hook of Claude Code,
/// `curl.exe` passes the JSON of Codex on stdin and prints the answer (the decision, or nothing) on stdout.
pub fn codex_hooks_path(home: &Path) -> PathBuf {
    home.join(".codex").join("hooks.json")
}

pub fn apply_install_codex(settings: &mut Value, token: &str, port: u32, timeout_sec: u32) -> Result<(), String> {
    let root = settings.as_object_mut().ok_or("hooks.json is not an object")?;
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or("hooks.json: hooks is not an object")?;
    let list = hooks.entry("PermissionRequest").or_insert_with(|| json!([]));
    let list = list.as_array_mut().ok_or("hooks.json: hooks.PermissionRequest is not a list")?;
    list.retain(|e| !is_our_hook(e));
    let command = format!(
        "curl.exe -s -m {} --data-binary @- -H \"Authorization: Bearer {token}\" \"http://127.0.0.1:{port}/api/cc/permission?agent=codex\"",
        timeout_sec + 10
    );
    list.push(json!({ "hooks": [ { "type": "command", "command": command, "timeout": timeout_sec + 15, "statusMessage": "Asking the Decka phone panel" } ] }));
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".decka-bak");
    path.with_file_name(name)
}

/// Writes our Codex hook into `hooks.json` (a copy `hooks.json.decka-bak` once, other hooks stay).
pub fn install_codex_into(home: &Path, token: &str, port: u32, timeout_sec: u32) -> Result<(), String> {
    let path = codex_hooks_path(home);
    let existing = read_settings(&path)?;
    if existing.is_some() && !backup_path(&path).exists() {
        std::fs::copy(&path, backup_path(&path)).map_err(|e| format!("cannot back up hooks.json: {e}"))?;
    }
    let mut hooks = existing.unwrap_or_else(|| json!({}));
    apply_install_codex(&mut hooks, token, port, timeout_sec)?;
    write_settings(&path, &hooks)
}

pub fn uninstall_codex_from(home: &Path) -> Result<(), String> {
    let path = codex_hooks_path(home);
    let Some(mut hooks) = read_settings(&path)? else { return Ok(()) };
    apply_uninstall(&mut hooks);
    // A file that only held our hook is removed; otherwise the rest stays as it was.
    if hooks.as_object().is_some_and(|o| o.is_empty()) && backup_path(&path).exists() == false {
        std::fs::remove_file(&path).map_err(|e| format!("cannot write hooks.json: {e}"))
    } else {
        write_settings(&path, &hooks)
    }
}

pub fn codex_hook_installed(home: &Path) -> bool {
    matches!(read_settings(&codex_hooks_path(home)), Ok(Some(s)) if read_install_state(&s).0)
}

/// Writes our `statusLine` (unless another tool owns it) and our `PermissionRequest` hook. Returns `statusLineConflict`.
pub fn apply_install(settings: &mut Value, token: &str, port: u32, timeout_sec: u32) -> Result<bool, String> {
    let root = settings.as_object_mut().ok_or("settings.json is not an object")?;
    let mut conflict = false;
    match root.get("statusLine") {
        Some(existing) if !is_our_status_line(existing) => conflict = true,
        _ => {
            let command = format!("curl.exe -s -m 2 --data-binary @- -H \"Authorization: Bearer {token}\" http://127.0.0.1:{port}/api/cc/statusline");
            root.insert("statusLine".to_string(), json!({ "type": "command", "command": command }));
        }
    }
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or("settings.json: hooks is not an object")?;
    let list = hooks.entry("PermissionRequest").or_insert_with(|| json!([]));
    let list = list.as_array_mut().ok_or("settings.json: hooks.PermissionRequest is not a list")?;
    list.retain(|e| !is_our_hook(e));
    list.push(json!({ "hooks": [ {
        "type": "http",
        "url": format!("http://127.0.0.1:{port}/api/cc/permission"),
        "timeout": timeout_sec + 15,
        "headers": { "Authorization": format!("Bearer {token}") }
    } ] }));
    Ok(conflict)
}

/// Removes only what `apply_install` wrote; empty `hooks.PermissionRequest` and `hooks` go too.
pub fn apply_uninstall(settings: &mut Value) {
    let Some(root) = settings.as_object_mut() else { return };
    if root.get("statusLine").is_some_and(is_our_status_line) {
        root.remove("statusLine");
    }
    if let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) {
        if let Some(list) = hooks.get_mut("PermissionRequest").and_then(Value::as_array_mut) {
            list.retain(|e| !is_our_hook(e));
            if list.is_empty() {
                hooks.remove("PermissionRequest");
            }
        }
        if hooks.is_empty() {
            root.remove("hooks");
        }
    }
}

/// (installed, statusLineConflict) as the file says now.
pub fn read_install_state(settings: &Value) -> (bool, bool) {
    let hook = settings.pointer("/hooks/PermissionRequest").and_then(Value::as_array).is_some_and(|l| l.iter().any(is_our_hook));
    let conflict = settings.get("statusLine").is_some_and(|s| !is_our_status_line(s));
    (hook, conflict)
}

fn read_settings(path: &Path) -> Result<Option<Value>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read settings.json: {e}")),
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    serde_json::from_str(text).map(Some).map_err(|_| "settings.json is not valid JSON".to_string())
}

fn write_settings(path: &Path, settings: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot write settings.json: {e}"))?;
    }
    let text = crate::config::json_text(settings).map_err(|e| e.to_string())?;
    crate::config::save_text_atomic(path, &text).map_err(|e| format!("cannot write settings.json: {e}"))
}

/// `cc_install` on a home folder: a copy `settings.json.decka-bak` once, then our entries. Returns `statusLineConflict`.
pub fn install_into(home: &Path, token: &str, port: u32, timeout_sec: u32) -> Result<bool, String> {
    let path = settings_path(home);
    let existing = read_settings(&path)?;
    if existing.is_some() {
        let backup = backup_path(&path);
        if !backup.exists() {
            std::fs::copy(&path, &backup).map_err(|e| format!("cannot back up settings.json: {e}"))?;
        }
    }
    let mut settings = existing.unwrap_or_else(|| json!({}));
    let conflict = apply_install(&mut settings, token, port, timeout_sec)?;
    write_settings(&path, &settings)?;
    Ok(conflict)
}

pub fn uninstall_from(home: &Path) -> Result<(), String> {
    let path = settings_path(home);
    let Some(mut settings) = read_settings(&path)? else { return Ok(()) };
    apply_uninstall(&mut settings);
    write_settings(&path, &settings)
}

/// (installed, statusLineConflict); an unreadable file counts as not installed.
pub fn state_of(home: &Path) -> (bool, bool) {
    match read_settings(&settings_path(home)) {
        Ok(Some(s)) => read_install_state(&s),
        _ => (false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub() -> Arc<AiHub> {
        let hub = AiHub::new(Store::new(json!({})));
        hub.configure(true, true);
        hub
    }

    fn opts(ms: u64) -> PermOpts {
        PermOpts { provider: "claude", replies: vec!["Plan first".to_string(), "Other way".to_string()], approvals: true, phones: 1, timeout: Duration::from_millis(ms) }
    }

    fn hook(tool: &str, input: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({ "tool_name": tool, "tool_input": input, "cwd": "C:\\work\\proj", "session_id": "abcdef123456" })).unwrap()
    }

    fn call(tool: &str, input: Value) -> Value {
        json!({ "tool_name": tool, "tool_input": input, "cwd": "C:\\work\\proj" })
    }

    fn rule(tool: &str, kind: &str, value: &str) -> AiRule {
        AiRule { id: "r1".into(), provider: "claude".into(), tool: tool.into(), kind: kind.into(), value: value.into(), created_at: 0 }
    }

    #[test]
    fn rules_are_offered_only_for_safe_forms() {
        let bash = |c: &str| draft_rule(&call("Bash", json!({ "command": c }))).map(|(_, k, v)| format!("{k}:{v}"));
        assert_eq!(bash("git status").as_deref(), Some("command:git status"));
        assert_eq!(bash("git status -s").as_deref(), Some("command:git status"));
        assert_eq!(bash("npm run build").as_deref(), Some("command:npm run build"));
        assert_eq!(bash("dir").as_deref(), Some("command:dir"));
        // Chaining, redirection, substitution, dangerous programs, flags and paths first: nothing is offered.
        for c in ["git status; rm -rf x", "git status && curl x", "echo a > b", "echo $(whoami)", "rm -rf build", "curl https://x", "npx foo", "ls -la", "git -C x status", "python script.py", "C:\\tools\\a.exe go", "npm run", ""] {
            assert_eq!(bash(c), None, "{c}");
        }
        let edit = |p: &str| draft_rule(&call("Edit", json!({ "file_path": p }))).map(|(_, k, v)| format!("{k}:{v}"));
        assert_eq!(edit("C:\\work\\proj\\src\\a.rs").as_deref(), Some("project:C:\\work\\proj"));
        assert_eq!(edit("C:\\other\\a.rs"), None);
        assert_eq!(edit("C:\\work\\proj\\..\\secret.txt"), None);
        assert_eq!(edit("~/.ssh/config"), None);
        let fetch = |u: &str| draft_rule(&call("WebFetch", json!({ "url": u }))).map(|(_, k, v)| format!("{k}:{v}"));
        assert_eq!(fetch("https://Docs.rs/serde?x=1").as_deref(), Some("host:docs.rs"));
        assert_eq!(fetch("https://user@evil.com/"), None);
        assert_eq!(fetch("ftp://x"), None);
        assert_eq!(draft_rule(&call("mcp__git__status", json!({}))).map(|d| d.1), Some("tool"));
        assert_eq!(draft_rule(&call("WebSearch", json!({ "query": "x" }))), None);
    }

    #[test]
    fn a_rule_covers_only_what_it_was_made_for() {
        let cmd = |c: &str| call("Bash", json!({ "command": c }));
        let git = rule("Bash", "command", "git status");
        assert!(rule_matches(&git, "claude", &cmd("git status")));
        assert!(rule_matches(&git, "claude", &cmd("GIT status -s")));
        assert!(!rule_matches(&git, "claude", &cmd("git push")));
        assert!(!rule_matches(&git, "claude", &cmd("git status; rm -rf /")));
        assert!(!rule_matches(&git, "codex", &cmd("git status")));
        assert!(!rule_matches(&git, "claude", &call("PowerShell", json!({ "command": "git status" }))));

        let project = rule("Edit", "project", "C:\\work\\proj");
        assert!(rule_matches(&project, "claude", &call("Edit", json!({ "file_path": "C:\\work\\proj\\a.rs" }))));
        assert!(!rule_matches(&project, "claude", &call("Edit", json!({ "file_path": "C:\\work\\other\\a.rs" }))));
        assert!(!rule_matches(&project, "claude", &call("Write", json!({ "file_path": "C:\\work\\proj\\a.rs" }))));
        let mut elsewhere = call("Edit", json!({ "file_path": "C:\\work\\proj\\a.rs" }));
        elsewhere["cwd"] = json!("C:\\work\\second");
        assert!(!rule_matches(&project, "claude", &elsewhere));

        let host = rule("WebFetch", "host", "docs.rs");
        assert!(rule_matches(&host, "claude", &call("WebFetch", json!({ "url": "https://docs.rs/x" }))));
        assert!(!rule_matches(&host, "claude", &call("WebFetch", json!({ "url": "https://docs.rs.evil.com/x" }))));
    }

    #[tokio::test]
    async fn always_allow_remembers_the_rule_and_the_next_call_is_answered_at_once() {
        let hub = hub();
        let body = hook("Bash", json!({ "command": "git status" }));
        let call = tokio::spawn({
            let (hub, body) = (hub.clone(), body.clone());
            async move { hub.permission(opts(5000), &body).await }
        });
        let pending = wait_pending(&hub, 1).await;
        assert_eq!(pending[0]["always"]["kind"], "command");
        assert!(hub.answer(pending[0]["reqId"].as_str().unwrap(), PermDecision::AllowAlways));
        assert_eq!(call.await.unwrap().as_deref(), Some(ALLOW_BODY));
        assert!(hub.take_rules_dirty());
        assert_eq!(hub.rules().len(), 1);

        // The same command again is allowed without a question (even with no phone), and shows in the history as `auto`.
        let no_phone = PermOpts { phones: 0, ..opts(50) };
        assert_eq!(hub.permission(no_phone, &body).await.as_deref(), Some(ALLOW_BODY));
        let history = hub.snapshot()["history"].as_array().cloned().unwrap();
        assert_eq!(history[0]["decision"], "auto");
        assert_eq!(history[1]["decision"], "allow");
        // Another command still asks.
        let other = hook("Bash", json!({ "command": "git push" }));
        assert_eq!(hub.permission(opts(30), &other).await, None);

        // Removing the rule brings the question back.
        let id = hub.rules()[0].id.clone();
        assert!(hub.remove_rules(Some(&id)));
        assert_eq!(hub.permission(PermOpts { phones: 0, ..opts(30) }, &body).await, None);
    }

    #[tokio::test]
    async fn the_history_keeps_the_last_answers_newest_first() {
        let hub = hub();
        for (i, decision) in [PermDecision::Deny, PermDecision::Allow].into_iter().enumerate() {
            let body = hook("Bash", json!({ "command": format!("echo {i}") }));
            let task = tokio::spawn({
                let hub = hub.clone();
                async move { hub.permission(opts(5000), &body).await }
            });
            let pending = wait_pending(&hub, 1).await;
            hub.answer(pending[0]["reqId"].as_str().unwrap(), decision);
            task.await.unwrap();
        }
        let history = hub.snapshot()["history"].as_array().cloned().unwrap();
        assert_eq!(history.iter().map(|h| h["decision"].as_str().unwrap()).collect::<Vec<_>>(), ["allow", "deny"]);
        assert_eq!(history[0]["tool"], "Bash");
        for n in 0..40 {
            let b = hook("Bash", json!({ "command": format!("echo {n}") }));
            let _ = hub.permission(opts(1), &b).await;
        }
        assert_eq!(hub.snapshot()["history"].as_array().unwrap().len(), MAX_HISTORY);
    }

    #[tokio::test]
    async fn deny_and_say_tells_the_tool_the_ready_answer() {
        let hub = hub();
        let body = hook("Bash", json!({ "command": "echo hi" }));
        let task = tokio::spawn({
            let (hub, body) = (hub.clone(), body.clone());
            async move { hub.permission(opts(5000), &body).await }
        });
        let pending = wait_pending(&hub, 1).await;
        assert_eq!(pending[0]["replies"], json!(["Plan first", "Other way"]));
        assert!(hub.answer_with(pending[0]["reqId"].as_str().unwrap(), PermDecision::Reply, Some(1)));
        let answer: Value = serde_json::from_str(&task.await.unwrap().unwrap()).unwrap();
        assert_eq!(answer["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(answer["hookSpecificOutput"]["decision"]["message"], "Other way");

        // A number that is not on the list is a plain denial.
        let task = tokio::spawn({
            let hub = hub.clone();
            async move { hub.permission(opts(5000), &body).await }
        });
        let pending = wait_pending(&hub, 1).await;
        hub.answer_with(pending[0]["reqId"].as_str().unwrap(), PermDecision::Reply, Some(9));
        assert_eq!(task.await.unwrap().as_deref(), Some(DENY_BODY));
    }

    async fn wait_pending(hub: &Arc<AiHub>, n: usize) -> Vec<Value> {        for _ in 0..200 {
            let pending = hub.snapshot()["pending"].as_array().cloned().unwrap_or_default();
            if pending.len() == n {
                return pending;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("pending never reached {n}");
    }

    #[test]
    fn statusline_takes_only_the_known_fields() {
        let body = br#"{"session_id":"abcdef123456","cwd":"C:\\work\\decka","model":{"display_name":"Opus"},"context_window":{"used_percentage":33.6},
            "rate_limits":{"five_hour":{"used_percentage":23.04,"resets_at":1800000000},"seven_day":{"used_percentage":41,"resets_at":1800500000}},"extra":1}"#;
        let s = parse_statusline(body).unwrap();
        assert_eq!((s.session.as_deref(), s.project.as_str(), s.model.as_deref(), s.ctx_pct), (Some("abcdef12"), "decka", Some("Opus"), Some(34)));
        let w = s.windows.unwrap();
        assert_eq!((w[0].label.as_str(), w[0].pct, w[0].resets_at), ("5h", 23.0, 1_800_000_000_000));
        assert_eq!((w[1].label.as_str(), w[1].pct), ("7d", 41.0));
        assert!(parse_statusline(b"not json").is_none());
        assert!(parse_statusline(br#"{"session_id":"x"}"#).unwrap().windows.is_none());
    }

    #[test]
    fn statusline_answer_and_state() {
        let hub = hub();
        let line = hub.statusline(parse_statusline(br#"{"session_id":"s1","cwd":"/a/b","rate_limits":{"five_hour":{"used_percentage":23,"resets_at":1800000000},"seven_day":{"used_percentage":41,"resets_at":1800000000}}}"#), 1000);
        assert_eq!(line, "Decka · 5h 23% · 7d 41%");
        // A body without limits keeps the old ones; a broken body answers the same line.
        assert_eq!(hub.statusline(parse_statusline(br#"{"session_id":"s1","cwd":"/a/b"}"#), 2000), line);
        assert_eq!(hub.statusline(None, 3000), line);
        assert_eq!(status_text(None), "Decka");
        assert_eq!(hub.snapshot()["sessions"].as_array().unwrap().len(), 1);
        assert_eq!(hub.last_seen(), Some(3000));
        // Sessions: at most six, old ones go on prune; a passed window goes too.
        for i in 0..8 {
            hub.statusline(parse_statusline(format!(r#"{{"session_id":"sess{i}","cwd":"/p"}}"#).as_bytes()), 5000 + i);
        }
        assert_eq!(hub.data.lock().unwrap().sessions.len(), MAX_SESSIONS);
        hub.prune(5000 + SESSION_TTL_MS + 100);
        assert!(hub.data.lock().unwrap().sessions.is_empty());
        assert_eq!(hub.data.lock().unwrap().claude.as_ref().unwrap().windows.len(), 2);
        hub.prune(1_800_000_000_001);
        assert!(hub.data.lock().unwrap().claude.as_ref().unwrap().windows.is_empty());
    }

    #[test]
    fn summary_per_tool() {
        let s = |tool: &str, input: Value| summarize(&serde_json::from_slice(&hook(tool, input)).unwrap());
        assert_eq!(s("Bash", json!({ "command": "ls\r\nrm x" })), ("Bash".into(), "ls ⏎ rm x".into()));
        assert_eq!(s("PowerShell", json!({ "command": "dir" })).1, "dir");
        assert_eq!(s("Edit", json!({ "file_path": "C:\\work\\proj\\src\\a.ts" })).1, "src/a.ts");
        assert_eq!(s("Read", json!({ "file_path": "D:\\other\\b.ts" })).1, "D:\\other\\b.ts");
        assert_eq!(s("NotebookEdit", json!({ "notebook_path": "C:/work/proj/n.ipynb" })).1, "n.ipynb");
        assert_eq!(s("WebFetch", json!({ "url": "https://a.b" })).1, "https://a.b");
        assert_eq!(s("WebSearch", json!({ "query": "rust" })).1, "rust");
        assert_eq!(s("mcp__github__create_issue", json!({ "title": "Bug" })), ("github: create_issue".into(), "Bug".into()));
        assert_eq!(s("Task", json!({ "n": 1 })), ("Task".into(), String::new()));
        let long = s("Bash", json!({ "command": "x".repeat(500) })).1;
        assert_eq!(long.chars().count(), 300);
        assert!(long.ends_with('…'));
    }

    #[tokio::test]
    async fn permission_allow_deny_and_pc() {
        for (decision, expected) in [(PermDecision::Allow, Some(ALLOW_BODY)), (PermDecision::Deny, Some(DENY_BODY)), (PermDecision::Pc, None)] {
            let hub = hub();
            let h = hub.clone();
            let call = tokio::spawn(async move { h.permission(opts(5000), &hook("Bash", json!({ "command": "ls" }))).await });
            let pending = wait_pending(&hub, 1).await;
            assert_eq!((pending[0]["tool"].as_str(), pending[0]["summary"].as_str(), pending[0]["project"].as_str(), pending[0]["sessionId"].as_str()), (Some("Bash"), Some("ls"), Some("proj"), Some("abcdef12")));
            let id = pending[0]["reqId"].as_str().unwrap().to_string();
            assert_eq!(id.len(), 16);
            assert!(hub.answer(&id, decision));
            // The first answer wins: a second one is ignored.
            assert!(!hub.answer(&id, PermDecision::Deny));
            assert_eq!(call.await.unwrap().as_deref(), expected);
            assert!(wait_pending(&hub, 0).await.is_empty());
        }
    }

    #[tokio::test]
    async fn permission_timeout_no_phone_and_hangup() {
        let hub = hub();
        // Time runs out: an empty body, the request is gone.
        assert_eq!(hub.permission(opts(30), &hook("Bash", json!({ "command": "ls" }))).await, None);
        assert!(wait_pending(&hub, 0).await.is_empty());
        // No phone, or approvals off: an empty body at once and nothing pending.
        assert_eq!(hub.permission(PermOpts { provider: "claude", replies: Vec::new(), approvals: true, phones: 0, timeout: Duration::from_secs(60) }, &hook("Bash", json!({}))).await, None);
        assert_eq!(hub.permission(PermOpts { provider: "claude", replies: Vec::new(), approvals: false, phones: 1, timeout: Duration::from_secs(60) }, &hook("Bash", json!({}))).await, None);
        assert_eq!(hub.permission(opts(60_000), b"not json").await, None);
        assert!(wait_pending(&hub, 0).await.is_empty());
        // Claude Code hangs up (the future is dropped): the request is removed.
        let h = hub.clone();
        let call = tokio::spawn(async move { h.permission(opts(60_000), &hook("Bash", json!({ "command": "ls" }))).await });
        wait_pending(&hub, 1).await;
        call.abort();
        let _ = call.await;
        assert!(wait_pending(&hub, 0).await.is_empty());
        // At most five wait at once; the sixth goes to the terminal.
        let mut calls = Vec::new();
        for _ in 0..MAX_PENDING {
            let h = hub.clone();
            calls.push(tokio::spawn(async move { h.permission(opts(60_000), &hook("Bash", json!({}))).await }));
        }
        wait_pending(&hub, MAX_PENDING).await;
        assert_eq!(hub.permission(opts(60_000), &hook("Bash", json!({}))).await, None);
        for c in calls {
            c.abort();
        }
    }

    #[test]
    fn variables_follow_the_section() {
        let hub = hub();
        hub.statusline(parse_statusline(br#"{"rate_limits":{"five_hour":{"used_percentage":23.4,"resets_at":1800000000}}}"#), 1);
        hub.set_codex(Some(AiProvider { windows: vec![AiWindow { label: "30d".into(), pct: 12.6, resets_at: 1 }], plan: None, updated_at: 1 }));
        // The 2-second rule held the section back; once it has passed, the tick sends it.
        assert_eq!(hub.store.get("vars").get("ai.claude.5h"), None);
        hub.data.lock().unwrap().last_pub = None;
        hub.tick();
        let vars = hub.store.get("vars");
        assert_eq!(vars["ai.claude.5h"], 23);
        assert_eq!(vars["ai.codex.primary"], 13);
        assert_eq!(vars["ai.pending"], 0);
        assert!(vars.get("ai.claude.7d").is_none());
        hub.configure(false, false);
        assert!(hub.store.get("vars").as_object().unwrap().keys().all(|k| !k.starts_with("ai.")));
        assert_eq!(hub.store.get("ai"), Value::Null);
    }

    #[test]
    fn codex_new_and_old_formats() {
        let new = r#"{"timestamp":"2026-11-17T10:00:00.000Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":4.0,"window_minutes":43200,"resets_at":1796743008},"secondary":null,"plan_type":"free"}}}"#;
        let (w, plan) = parse_codex(&format!("{{\"half a line\n{new}\n{{\"payload\":{{}}}}\n")).unwrap();
        assert_eq!(w, vec![AiWindow { label: "30d".into(), pct: 4.0, resets_at: 1_796_743_008_000 }]);
        assert_eq!(plan.as_deref(), Some("free"));
        let old = r#"{"timestamp":"2026-11-17T10:00:00Z","payload":{"rate_limits":{"primary":{"used_percent":50,"window_minutes":300,"resets_in_seconds":600},"secondary":{"used_percent":10,"window_minutes":10080,"resets_in_seconds":3600}}}}"#;
        let (w, plan) = parse_codex(old).unwrap();
        let base = parse_timestamp_ms("2026-11-17T10:00:00Z").unwrap();
        assert_eq!(base, 1_794_909_600_000);
        assert_eq!((w[0].label.as_str(), w[0].resets_at, w[1].label.as_str(), w[1].resets_at), ("5h", base + 600_000, "7d", base + 3_600_000));
        assert_eq!(plan, None);
        assert_eq!(window_label(90), "2h");
        assert!(parse_codex("{}").is_none());
        assert_eq!(parse_timestamp_ms("2026-11-17T12:00:00+02:00"), Some(base));
    }

    #[test]
    fn codex_finds_the_newest_file_in_a_temp_home() {
        let home = tempfile::tempdir().unwrap();
        let sessions = codex_dir(home.path());
        assert!(read_codex(home.path()).is_none());
        for (day, name, pct) in [("2026/11/16", "rollout-a.jsonl", 1), ("2026/11/17", "rollout-b.jsonl", 2)] {
            let dir = sessions.join(day);
            std::fs::create_dir_all(&dir).unwrap();
            let line = format!(r#"{{"payload":{{"rate_limits":{{"primary":{{"used_percent":{pct},"window_minutes":300,"resets_at":1800000000}}}}}}}}"#);
            std::fs::write(dir.join(name), format!("{line}\n")).unwrap();
        }
        assert_eq!(read_codex(home.path()).unwrap().windows[0].pct, 2.0);
    }

    fn token() -> &'static str {
        "dk_test"
    }

    #[test]
    fn install_keeps_foreign_settings_and_is_idempotent() {
        let home = tempfile::tempdir().unwrap();
        let path = settings_path(home.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = json!({ "theme": "dark", "statusLine": { "type": "command", "command": "other.exe" },
            "hooks": { "PreToolUse": [ { "hooks": [ { "type": "command", "command": "x" } ] } ],
                       "PermissionRequest": [ { "matcher": "Bash", "hooks": [ { "type": "command", "command": "mine" } ] } ] } });
        std::fs::write(&path, serde_json::to_string(&original).unwrap()).unwrap();

        assert!(install_into(home.path(), token(), 9247, 120).unwrap(), "foreign statusLine is a conflict");
        let backup = path.with_file_name("settings.json.decka-bak");
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(&backup).unwrap()).unwrap(), original);
        let after: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(after["statusLine"]["command"], "other.exe");
        assert_eq!(after["theme"], "dark");
        assert_eq!(after["hooks"]["PreToolUse"], original["hooks"]["PreToolUse"]);
        assert_eq!(after["hooks"]["PermissionRequest"].as_array().unwrap().len(), 2);
        let ours = &after["hooks"]["PermissionRequest"][1]["hooks"][0];
        assert_eq!((ours["url"].as_str(), ours["timeout"].as_u64(), ours["headers"]["Authorization"].as_str()), (Some("http://127.0.0.1:9247/api/cc/permission"), Some(135), Some("Bearer dk_test")));
        assert_eq!(state_of(home.path()), (true, true));

        // Again with another port: our entries are rewritten, nothing doubles, the backup stays the first copy.
        install_into(home.path(), token(), 9300, 60).unwrap();
        let again: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(again["hooks"]["PermissionRequest"].as_array().unwrap().len(), 2);
        assert_eq!(again["hooks"]["PermissionRequest"][1]["hooks"][0]["url"], "http://127.0.0.1:9300/api/cc/permission");
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(&backup).unwrap()).unwrap(), original);

        uninstall_from(home.path()).unwrap();
        let removed: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(removed, original);
        assert_eq!(state_of(home.path()), (false, true));
    }

    #[test]
    fn install_on_a_fresh_home_and_a_broken_file() {
        let home = tempfile::tempdir().unwrap();
        assert!(!install_into(home.path(), token(), 9247, 120).unwrap());
        let path = settings_path(home.path());
        assert!(!path.with_file_name("settings.json.decka-bak").exists(), "no file before, nothing to back up");
        let s: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(s["statusLine"]["command"].as_str().unwrap().contains("Bearer dk_test"));
        assert_eq!(state_of(home.path()), (true, false));
        // Our statusLine is rewritten, and removing leaves an empty object.
        uninstall_from(home.path()).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(&path).unwrap()).unwrap(), json!({}));
        assert!(uninstall_from(tempfile::tempdir().unwrap().path()).is_ok());

        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(install_into(home.path(), token(), 9247, 120).unwrap_err(), "settings.json is not valid JSON");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }
}
