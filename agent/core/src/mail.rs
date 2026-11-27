//! Unread mail (DATA_MODEL.md 10.7): IMAP over TLS, three commands only. Nothing but the number of unseen messages in INBOX is
//! read: no headers, no bodies. The password is read from its DPAPI blob for the login and goes nowhere else (never into a log).
//! Every failure has a step and a reason (T31.2), so the settings window can say what to do.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;

use crate::model::{MailAccount, MailAccountInfo, MailInfo, VarValue};

/// The whole check of one mailbox, from the name lookup to logout.
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// How often the boxes are looked at.
pub const INTERVAL: Duration = Duration::from_secs(120);
/// A line longer than this is not an IMAP answer.
const MAX_LINE: usize = 16 * 1024;
/// More lines than this before the tagged answer means the server is not behaving.
const MAX_LINES: usize = 200;
/// The most of a server's or the system's error text shown in the window.
const MAX_DETAIL: usize = 200;

/// Why a mailbox could not be read. The window shows it with advice; the phone gets only `auth` or `network` (PROTOCOL.md 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MailReason {
    /// The server refused the user name or password (or there is no password yet).
    Auth,
    /// The encrypted connection could not be set up: certificate, wrong port, something in between.
    Tls,
    /// No such server, no connection, or the answer is not IMAP.
    Host,
    /// No answer in time.
    Timeout,
    /// The service accepts only an app password over IMAP.
    AppPasswordRequired,
    /// The mailbox allows only OAuth sign-in.
    OauthUnsupported,
}

impl MailReason {
    pub fn as_str(self) -> &'static str {
        match self {
            MailReason::Auth => "auth",
            MailReason::Tls => "tls",
            MailReason::Host => "host",
            MailReason::Timeout => "timeout",
            MailReason::AppPasswordRequired => "app_password_required",
            MailReason::OauthUnsupported => "oauth_unsupported",
        }
    }

    /// The server turned the sign-in down: trying again with the same password only risks a lock-out.
    pub fn is_refusal(self) -> bool {
        matches!(self, MailReason::Auth | MailReason::AppPasswordRequired | MailReason::OauthUnsupported)
    }

    /// The `error` of the `mail` section the phone gets.
    pub fn wire(self) -> &'static str {
        if self.is_refusal() {
            "auth"
        } else {
            "network"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MailStepId {
    Dns,
    Tcp,
    Tls,
    Login,
    Folder,
}

const STEPS: [MailStepId; 5] = [MailStepId::Dns, MailStepId::Tcp, MailStepId::Tls, MailStepId::Login, MailStepId::Folder];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MailStepState {
    Ok,
    Error,
    Skipped,
}

/// One step of a connection check (`mail_test`, PROTOCOL.md IPC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailStep {
    pub id: MailStepId,
    pub state: MailStepState,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<MailReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Where and why a check ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailFailure {
    pub step: MailStepId,
    pub reason: MailReason,
    /// The server's or the system's words, without the user name and the password. For the window only, never logged.
    pub detail: Option<String>,
    pub no_password: bool,
}

/// What a check of one mailbox found.
#[derive(Debug, Clone)]
pub struct Probe {
    pub steps: Vec<MailStep>,
    pub outcome: Result<u32, MailFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailTestResult {
    pub steps: Vec<MailStep>,
    pub unread: Option<u32>,
    pub no_password: bool,
}

impl From<Probe> for MailTestResult {
    fn from(p: Probe) -> Self {
        let no_password = p.outcome.as_ref().err().is_some_and(|f| f.no_password);
        MailTestResult { unread: p.outcome.ok(), steps: p.steps, no_password }
    }
}

/// `mail_status`: how the last round of the mail loop went for one mailbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountStatus {
    pub account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<MailReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<MailStepId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub no_password: bool,
}

impl MailAccountStatus {
    pub fn new(account_id: &str, outcome: &Result<u32, MailFailure>) -> Self {
        let failure = outcome.as_ref().err();
        MailAccountStatus {
            account_id: account_id.to_string(),
            reason: failure.map(|f| f.reason),
            step: failure.map(|f| f.step),
            detail: failure.and_then(|f| f.detail.clone()),
            no_password: failure.is_some_and(|f| f.no_password),
        }
    }
}

type StepError = (MailReason, Option<String>);

fn clip(text: &str) -> Option<String> {
    let text: String = text.trim().chars().filter(|c| !c.is_control()).take(MAX_DETAIL).collect();
    (!text.is_empty()).then_some(text)
}

/// A server line as detail: some servers repeat the refused command, so the secrets are cut out first.
fn server_text(line: &str, user: &str, password: &str) -> Option<String> {
    let mut text = line.trim().to_string();
    for secret in [quote(password), Some(password.to_string()), quote(user), Some(user.to_string())].into_iter().flatten() {
        if !secret.is_empty() {
            text = text.replace(&secret, "…");
        }
    }
    clip(&text)
}

fn in_domain(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|rest| rest.ends_with('.'))
}

/// What a refused LOGIN means, from the server's answer and the server's name.
pub fn classify_refusal(host: &str, answer: &str) -> MailReason {
    let answer = answer.to_ascii_lowercase();
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let says = |words: &[&str]| words.iter().any(|w| answer.contains(w));
    let at = |domains: &[&str]| domains.iter().any(|d| in_domain(&host, d));
    if says(&["[unavailable]", "[limit]", "too many", "try again later"]) {
        // A passing refusal: the next round tries again.
        MailReason::Host
    } else if says(&["oauth", "basicauthblocked", "basic authentication is disabled", "logindisabled"]) {
        MailReason::OauthUnsupported
    } else if says(&["application-specific password", "app-specific password", "app password", "application password"]) {
        MailReason::AppPasswordRequired
    } else if at(&["outlook.com", "office365.com", "hotmail.com", "live.com"]) {
        MailReason::OauthUnsupported
    } else if at(&["gmail.com", "googlemail.com", "me.com", "mail.ru", "yandex.ru", "yandex.com", "yandex.kz", "ya.ru"]) {
        MailReason::AppPasswordRequired
    } else {
        MailReason::Auth
    }
}

/// `"text"` with `\` and `"` escaped; `None` when the text has a control character (it could start another command).
pub fn quote(text: &str) -> Option<String> {
    if text.chars().any(|c| c.is_control()) {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        if c == '\\' || c == '"' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    Some(out)
}

/// `a1 LOGIN "<user>" "<pass>"` without the line end; `None` when user or password cannot be quoted.
pub fn login_command(tag: &str, user: &str, password: &str) -> Option<String> {
    Some(format!("{tag} LOGIN {} {}", quote(user)?, quote(password)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Ok,
    /// `NO` or `BAD`.
    Refused,
}

/// The tagged answer `<tag> OK|NO|BAD ...`; `None` for any other line (an untagged one, or another tag).
pub fn parse_tagged(line: &str, tag: &str) -> Option<Reply> {
    let rest = line.trim_end().strip_prefix(tag)?.strip_prefix(' ')?;
    let word = rest.split(' ').next()?.to_ascii_uppercase();
    match word.as_str() {
        "OK" => Some(Reply::Ok),
        "NO" | "BAD" => Some(Reply::Refused),
        _ => None,
    }
}

/// The count in `* STATUS INBOX (UNSEEN 5)` (the mailbox name may be quoted, other items may come first).
pub fn parse_unseen(line: &str) -> Option<u32> {
    let line = line.trim_end();
    if !line.to_ascii_uppercase().starts_with("* STATUS ") {
        return None;
    }
    let open = line.find('(')?;
    let items: Vec<&str> = line[open + 1..].trim_end_matches(')').split_whitespace().collect();
    items.windows(2).find(|w| w[0].eq_ignore_ascii_case("UNSEEN")).and_then(|w| w[1].parse().ok())
}

async fn read_line<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> Result<String, StepError> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await.map_err(|e| (MailReason::Host, clip(&e.to_string())))?;
    if n == 0 {
        return Err((MailReason::Host, Some("the server closed the connection".into())));
    }
    if line.len() > MAX_LINE {
        return Err((MailReason::Host, Some("the answer is too long for IMAP".into())));
    }
    Ok(line)
}

/// The tagged answer and its line.
async fn until_tagged<R: AsyncBufReadExt + Unpin>(reader: &mut R, tag: &str, unseen: &mut Option<u32>) -> Result<(Reply, String), StepError> {
    for _ in 0..MAX_LINES {
        let line = read_line(reader).await?;
        if let Some(n) = parse_unseen(&line) {
            *unseen = Some(n);
        }
        if let Some(reply) = parse_tagged(&line, tag) {
            return Ok((reply, line));
        }
    }
    Err((MailReason::Host, Some("too many lines without an answer".into())))
}

/// Times the steps of one check under a common deadline.
struct Run {
    steps: Vec<MailStep>,
    deadline: Instant,
}

impl Run {
    fn new(timeout: Duration) -> Self {
        Run { steps: Vec::with_capacity(STEPS.len()), deadline: Instant::now() + timeout }
    }

    async fn step<T>(&mut self, id: MailStepId, work: impl Future<Output = Result<T, StepError>>) -> Result<T, MailFailure> {
        let started = Instant::now();
        let result = tokio::time::timeout_at(self.deadline, work).await.unwrap_or(Err((MailReason::Timeout, None)));
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match result {
            Ok(value) => {
                self.steps.push(MailStep { id, state: MailStepState::Ok, duration_ms, reason: None, detail: None });
                Ok(value)
            }
            Err((reason, detail)) => {
                self.steps.push(MailStep { id, state: MailStepState::Error, duration_ms, reason: Some(reason), detail: detail.clone() });
                Err(MailFailure { step: id, reason, detail, no_password: false })
            }
        }
    }

    /// The steps after a failure are listed as not reached.
    fn finish(mut self, outcome: Result<u32, MailFailure>) -> Probe {
        for id in STEPS {
            if !self.steps.iter().any(|s| s.id == id) {
                self.steps.push(MailStep { id, state: MailStepState::Skipped, duration_ms: 0, reason: None, detail: None });
            }
        }
        self.steps.sort_by_key(|s| STEPS.iter().position(|id| *id == s.id));
        Probe { steps: self.steps, outcome }
    }
}

/// Greeting, LOGIN, STATUS, LOGOUT on an open stream.
async fn talk<S>(run: &mut Run, stream: S, host: &str, user: &str, password: Option<&str>) -> Result<u32, MailFailure>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read, mut write) = tokio::io::split(stream);
    let mut reader = BufReader::new(read);
    let signed_in = run
        .step(MailStepId::Login, async {
            let greeting = read_line(&mut reader).await?;
            let upper = greeting.to_ascii_uppercase();
            if !upper.starts_with("* OK") && !upper.starts_with("* PREAUTH") {
                return Err((MailReason::Host, clip(&greeting)));
            }
            // The server says itself that it takes no password: do not send one.
            if upper.contains("LOGINDISABLED") {
                return Err((MailReason::OauthUnsupported, clip(&greeting)));
            }
            let Some(password) = password else { return Err((MailReason::Auth, None)) };
            let login = login_command("a1", user, password).ok_or((MailReason::Auth, Some("the user name or password has a line break or another control character".into())))?;
            write.write_all(format!("{login}\r\n").as_bytes()).await.map_err(|e| (MailReason::Host, clip(&e.to_string())))?;
            let mut ignored = None;
            match until_tagged(&mut reader, "a1", &mut ignored).await? {
                (Reply::Ok, _) => Ok(()),
                (Reply::Refused, line) => Err((classify_refusal(host, &line), server_text(&line, user, password))),
            }
        })
        .await;
    signed_in.map_err(|f| MailFailure { no_password: password.is_none() && f.reason == MailReason::Auth, ..f })?;
    let unseen = run
        .step(MailStepId::Folder, async {
            write.write_all(b"a2 STATUS INBOX (UNSEEN)\r\n").await.map_err(|e| (MailReason::Host, clip(&e.to_string())))?;
            let mut unseen = None;
            let (reply, line) = until_tagged(&mut reader, "a2", &mut unseen).await?;
            match (reply, unseen) {
                (Reply::Ok, Some(n)) => Ok(n),
                _ => Err((MailReason::Host, server_text(&line, user, password.unwrap_or_default()))),
            }
        })
        .await?;
    // The count is in hand; a failing or slow LOGOUT changes nothing.
    let _ = tokio::time::timeout_at(run.deadline, async {
        write.write_all(b"a3 LOGOUT\r\n").await?;
        write.shutdown().await
    })
    .await;
    Ok(unseen)
}

/// The IMAP part on any stream. Pure of sockets, so a test can feed it text.
pub async fn dialog<S>(stream: S, host: &str, user: &str, password: &str) -> Result<u32, MailFailure>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    talk(&mut Run::new(TIMEOUT), stream, host, user, Some(password)).await
}

async fn reach(run: &mut Run, host: &str, port: u16, user: &str, password: Option<&str>) -> Result<u32, MailFailure> {
    let addresses = run
        .step(MailStepId::Dns, async {
            let found: Vec<_> = tokio::net::lookup_host((host, port)).await.map_err(|e| (MailReason::Host, clip(&e.to_string())))?.collect();
            if found.is_empty() {
                return Err((MailReason::Host, Some("the name has no address".into())));
            }
            Ok(found)
        })
        .await?;
    let tcp = run
        .step(MailStepId::Tcp, async {
            TcpStream::connect(&addresses[..]).await.map_err(|e| {
                let reason = if e.kind() == std::io::ErrorKind::TimedOut { MailReason::Timeout } else { MailReason::Host };
                (reason, clip(&e.to_string()))
            })
        })
        .await?;
    let tls = run
        .step(MailStepId::Tls, async {
            let name = ServerName::try_from(host.to_string()).map_err(|e| (MailReason::Host, clip(&e.to_string())))?;
            let config = crate::tls::client_config().map_err(|e| (MailReason::Tls, clip(&e.to_string())))?;
            TlsConnector::from(Arc::new(config)).connect(name, tcp).await.map_err(|e| (MailReason::Tls, clip(&e.to_string())))
        })
        .await?;
    talk(run, tls, host, user, password).await
}

/// One check of one mailbox: DNS, TCP, TLS, sign-in, INBOX. TLS only (no STARTTLS, no plain login); the certificate is checked
/// by the platform verifier. All steps share `timeout`. Without a password the check ends at the sign-in.
pub async fn probe(host: &str, port: u16, user: &str, password: Option<&str>, timeout: Duration) -> Probe {
    let mut run = Run::new(timeout);
    let outcome = reach(&mut run, host, port, user, password).await;
    run.finish(outcome)
}

/// The `mail` section from what one round found; accounts without a result keep `unread: null`.
pub fn build_info(accounts: &[(MailAccount, Result<u32, MailFailure>)], now_ms: u64) -> MailInfo {
    MailInfo {
        accounts: accounts
            .iter()
            .map(|(a, r)| MailAccountInfo {
                id: a.id.clone(),
                name: a.name.clone(),
                unread: r.as_ref().ok().copied(),
                error: r.as_ref().err().map(|f| f.reason.wire().to_string()),
                open_action_id: a.open_action_id.clone(),
            })
            .collect(),
        updated_at: now_ms,
    }
}

fn num(n: u32) -> VarValue {
    VarValue::Num(serde_json::Number::from(n))
}

/// `mail.unread` (the sum) and `mail.<id>.unread` for the accounts that answered; the other `mail.` variables are removed.
pub fn apply_vars(vars: &mut BTreeMap<String, VarValue>, info: Option<&MailInfo>) {
    vars.retain(|k, _| !k.starts_with("mail."));
    let Some(info) = info else { return };
    let mut total = 0u32;
    for a in &info.accounts {
        if let Some(n) = a.unread {
            total += n;
            vars.insert(format!("mail.{}.unread", a.id), num(n));
        }
    }
    vars.insert("mail.unread".to_string(), num(total));
}

pub fn clear_vars(vars: &mut BTreeMap<String, VarValue>) {
    apply_vars(vars, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_backslashes_are_escaped_in_login() {
        assert_eq!(login_command("a1", "me@x.org", "pa\"ss\\word").unwrap(), r#"a1 LOGIN "me@x.org" "pa\"ss\\word""#);
        assert_eq!(quote("plain").unwrap(), "\"plain\"");
    }

    #[test]
    fn a_control_character_cannot_start_another_command() {
        assert!(login_command("a1", "me", "x\r\na9 LOGOUT").is_none());
        assert!(quote("a\nb").is_none());
        assert!(quote("tab\there").is_none());
    }

    #[test]
    fn status_reply_gives_the_unseen_count() {
        assert_eq!(parse_unseen("* STATUS INBOX (UNSEEN 7)\r\n"), Some(7));
        assert_eq!(parse_unseen("* STATUS \"INBOX\" (MESSAGES 40 UNSEEN 0)"), Some(0));
        assert_eq!(parse_unseen("* status inbox (unseen 12)"), Some(12));
        assert_eq!(parse_unseen("* STATUS INBOX (MESSAGES 3)"), None);
        assert_eq!(parse_unseen("* OK hello"), None);
        assert_eq!(parse_unseen("a2 OK STATUS completed"), None);
        assert_eq!(parse_unseen("* STATUS INBOX (UNSEEN x)"), None);
    }

    #[test]
    fn tagged_answers_are_told_apart() {
        assert_eq!(parse_tagged("a1 OK LOGIN completed\r\n", "a1"), Some(Reply::Ok));
        assert_eq!(parse_tagged("a1 NO [AUTHENTICATIONFAILED] Invalid credentials", "a1"), Some(Reply::Refused));
        assert_eq!(parse_tagged("a1 BAD Command Argument Error", "a1"), Some(Reply::Refused));
        assert_eq!(parse_tagged("a2 OK done", "a1"), None);
        assert_eq!(parse_tagged("* OK still here", "a1"), None);
        assert_eq!(parse_tagged("a10 OK done", "a1"), None);
    }

    #[test]
    fn a_refusal_gets_a_reason_from_the_words_and_the_server() {
        use MailReason::*;
        let cases = [
            ("imap.example.org", "a1 NO [AUTHENTICATIONFAILED] Invalid credentials", Auth),
            ("imap.gmail.com", "a1 NO [ALERT] Application-specific password required: https://support.google.com (Failure)", AppPasswordRequired),
            ("imap.gmail.com", "a1 NO [AUTHENTICATIONFAILED] Invalid credentials (Failure)", AppPasswordRequired),
            ("imap.mail.ru", "a1 NO [AUTHENTICATIONFAILED] Application password is REQUIRED", AppPasswordRequired),
            ("imap.yandex.ru", "a1 NO [AUTHENTICATIONFAILED] LOGIN invalid credentials or IMAP is disabled", AppPasswordRequired),
            ("imap.mail.me.com", "a1 NO [AUTHENTICATIONFAILED] Authentication Failed.", AppPasswordRequired),
            ("outlook.office365.com", "a1 NO LOGIN failed.", OauthUnsupported),
            ("imap-mail.outlook.com", "a1 NO LOGIN failed.", OauthUnsupported),
            ("imap.corp.example", "a1 NO Basic authentication is disabled, use OAuth", OauthUnsupported),
            ("imap.gmail.com", "a1 NO [UNAVAILABLE] Temporary System Error", Host),
            // A look-alike name is not the service.
            ("imap.notgmail.com", "a1 NO bad", Auth),
        ];
        for (host, answer, expected) in cases {
            assert_eq!(classify_refusal(host, answer), expected, "{host}: {answer}");
        }
    }

    /// The dialog on text instead of a socket: what is sent and how the answers are read.
    async fn run(server_says: &str) -> (Result<u32, MailFailure>, String) {
        run_as("imap.example.org", server_says).await
    }

    async fn run_as(host: &str, server_says: &str) -> (Result<u32, MailFailure>, String) {
        let (client, mut server) = tokio::io::duplex(8192);
        let script = server_says.to_string();
        let task = tokio::spawn(async move {
            server.write_all(script.as_bytes()).await.unwrap();
            // The script is all the server says: after it the client sees the end of the stream instead of waiting.
            server.shutdown().await.unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 512];
            loop {
                match tokio::io::AsyncReadExt::read(&mut server, &mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => seen.extend_from_slice(&buf[..n]),
                }
            }
            String::from_utf8_lossy(&seen).to_string()
        });
        let result = dialog(client, host, "me@example.org", "p\"w-secret").await;
        (result, task.await.unwrap())
    }

    fn failed(result: Result<u32, MailFailure>) -> (MailStepId, MailReason) {
        let f = result.unwrap_err();
        (f.step, f.reason)
    }

    #[tokio::test]
    async fn a_good_dialog_counts_unseen_and_sends_three_commands() {
        let (result, sent) = run("* OK ready\r\n* CAPABILITY IMAP4rev1\r\na1 OK LOGIN completed\r\n* STATUS INBOX (UNSEEN 4)\r\na2 OK STATUS completed\r\n").await;
        assert_eq!(result, Ok(4));
        assert_eq!(sent, "a1 LOGIN \"me@example.org\" \"p\\\"w-secret\"\r\na2 STATUS INBOX (UNSEEN)\r\na3 LOGOUT\r\n");
    }

    #[tokio::test]
    async fn a_refused_login_is_an_auth_error_and_nothing_more_is_sent() {
        let (result, sent) = run("* OK ready\r\na1 NO [AUTHENTICATIONFAILED] bad\r\n").await;
        assert_eq!(failed(result), (MailStepId::Login, MailReason::Auth));
        assert_eq!(sent, "a1 LOGIN \"me@example.org\" \"p\\\"w-secret\"\r\n");
        let (result, _) = run("* OK ready\r\na1 BAD nope\r\n").await;
        assert_eq!(failed(result), (MailStepId::Login, MailReason::Auth));
    }

    #[tokio::test]
    async fn a_server_that_repeats_the_command_does_not_put_the_secrets_into_the_detail() {
        let (result, _) = run("* OK ready\r\na1 BAD parse error: LOGIN \"me@example.org\" \"p\\\"w-secret\" (p\"w-secret)\r\n").await;
        let detail = result.unwrap_err().detail.unwrap();
        assert!(!detail.contains("w-secret") && !detail.contains("me@example.org"), "{detail}");
        assert!(detail.contains("parse error"));
    }

    #[tokio::test]
    async fn no_password_is_sent_to_a_server_that_refuses_passwords() {
        let (result, sent) = run_as("outlook.office365.com", "* OK [CAPABILITY IMAP4rev1 LOGINDISABLED AUTH=XOAUTH2] ready\r\n").await;
        assert_eq!(failed(result), (MailStepId::Login, MailReason::OauthUnsupported));
        assert_eq!(sent, "");
    }

    #[tokio::test]
    async fn a_strange_server_is_a_host_error_at_the_step_it_happened() {
        assert_eq!(failed(run("* BYE go away\r\n").await.0), (MailStepId::Login, MailReason::Host));
        assert_eq!(failed(run("* OK ready\r\na1 OK done\r\na2 OK done but no count\r\n").await.0), (MailStepId::Folder, MailReason::Host));
        assert_eq!(failed(run("").await.0), (MailStepId::Login, MailReason::Host));
    }

    fn states(p: &Probe) -> Vec<(MailStepId, MailStepState)> {
        p.steps.iter().map(|s| (s.id, s.state)).collect()
    }

    #[tokio::test]
    async fn a_name_that_does_not_exist_stops_at_dns() {
        let p = probe("mail.nothing.invalid", 993, "me", Some("x"), TIMEOUT).await;
        use MailStepState::*;
        assert_eq!(states(&p), [(MailStepId::Dns, Error), (MailStepId::Tcp, Skipped), (MailStepId::Tls, Skipped), (MailStepId::Login, Skipped), (MailStepId::Folder, Skipped)]);
        assert_eq!(failed(p.outcome), (MailStepId::Dns, MailReason::Host));
    }

    #[tokio::test]
    async fn a_closed_port_stops_at_tcp() {
        // A port that was just free: nothing listens on it.
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let p = probe("127.0.0.1", port, "me", Some("x"), TIMEOUT).await;
        assert_eq!(p.steps[0].state, MailStepState::Ok);
        assert_eq!(failed(p.outcome), (MailStepId::Tcp, MailReason::Host));
    }

    #[tokio::test]
    async fn a_server_without_tls_stops_at_tls_and_a_silent_one_times_out() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let plain = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"* OK plain IMAP, no encryption\r\n").await.unwrap();
            let (silent, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop((socket, silent));
        });
        let p = probe("127.0.0.1", port, "me", Some("x"), TIMEOUT).await;
        assert_eq!(failed(p.outcome), (MailStepId::Tls, MailReason::Tls));
        let p = probe("127.0.0.1", port, "me", Some("x"), Duration::from_millis(300)).await;
        assert_eq!(failed(p.outcome), (MailStepId::Tls, MailReason::Timeout));
        plain.abort();
    }

    #[test]
    fn variables_follow_the_accounts_that_answered() {
        let account = |id: &str| MailAccount { id: id.into(), name: id.into(), host: "h".into(), port: 993, user: "u".into(), password: None, password_set: None, open_action_id: None };
        let refused = MailFailure { step: MailStepId::Login, reason: MailReason::AppPasswordRequired, detail: None, no_password: false };
        let away = MailFailure { step: MailStepId::Dns, reason: MailReason::Timeout, detail: None, no_password: false };
        let info = build_info(&[(account("home"), Ok(3)), (account("work"), Err(refused)), (account("old"), Ok(2)), (account("far"), Err(away))], 5);
        assert_eq!(info.accounts[1].unread, None);
        // The phone knows only the two values of the protocol.
        assert_eq!(info.accounts[1].error.as_deref(), Some("auth"));
        assert_eq!(info.accounts[3].error.as_deref(), Some("network"));
        let mut vars = BTreeMap::new();
        vars.insert("mail.gone.unread".to_string(), num(9));
        vars.insert("keep.me".to_string(), num(1));
        apply_vars(&mut vars, Some(&info));
        assert_eq!(vars.get("mail.unread"), Some(&num(5)));
        assert_eq!(vars.get("mail.home.unread"), Some(&num(3)));
        assert!(!vars.contains_key("mail.work.unread") && !vars.contains_key("mail.gone.unread"));
        assert!(vars.contains_key("keep.me"));
        clear_vars(&mut vars);
        assert_eq!(vars.keys().collect::<Vec<_>>(), ["keep.me"]);
    }
}
