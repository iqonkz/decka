//! Unread mail (DATA_MODEL.md 10.7): IMAP over TLS, three commands only. Nothing but the number of unseen messages in INBOX is
//! read: no headers, no bodies. The password is read from its DPAPI blob for the login and goes nowhere else (never into a log).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;

use crate::model::{MailAccount, MailAccountInfo, MailInfo, VarValue};
use rustls_platform_verifier::ConfigVerifierExt;

/// The whole dialog, connect to logout.
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// How often the boxes are looked at.
pub const INTERVAL: Duration = Duration::from_secs(120);
/// A line longer than this is not an IMAP answer.
const MAX_LINE: usize = 16 * 1024;
/// More lines than this before the tagged answer means the server is not behaving.
const MAX_LINES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailError {
    /// `NO` / `BAD` on LOGIN: wrong user or password, or the account needs an app password.
    Auth,
    /// Anything else: no route, a refused connection, a bad certificate, a timeout, an unreadable answer.
    Network,
}

impl MailError {
    pub fn as_str(self) -> &'static str {
        match self {
            MailError::Auth => "auth",
            MailError::Network => "network",
        }
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

async fn read_line<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> Result<String, MailError> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await.map_err(|_| MailError::Network)?;
    if n == 0 || line.len() > MAX_LINE {
        return Err(MailError::Network);
    }
    Ok(line)
}

async fn until_tagged<R: AsyncBufReadExt + Unpin>(reader: &mut R, tag: &str, unseen: &mut Option<u32>) -> Result<Reply, MailError> {
    for _ in 0..MAX_LINES {
        let line = read_line(reader).await?;
        if let Some(n) = parse_unseen(&line) {
            *unseen = Some(n);
        }
        if let Some(reply) = parse_tagged(&line, tag) {
            return Ok(reply);
        }
    }
    Err(MailError::Network)
}

/// The dialog on any stream: greeting, LOGIN, STATUS, LOGOUT. Pure of sockets, so a test can feed it text.
pub async fn dialog<S>(stream: S, user: &str, password: &str) -> Result<u32, MailError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (read, mut write) = tokio::io::split(stream);
    let mut reader = BufReader::new(read);
    let greeting = read_line(&mut reader).await?;
    if !greeting.starts_with("* OK") && !greeting.to_ascii_uppercase().starts_with("* PREAUTH") {
        return Err(MailError::Network);
    }
    let login = login_command("a1", user, password).ok_or(MailError::Auth)?;
    write.write_all(format!("{login}\r\n").as_bytes()).await.map_err(|_| MailError::Network)?;
    let mut ignored = None;
    match until_tagged(&mut reader, "a1", &mut ignored).await? {
        Reply::Ok => {}
        Reply::Refused => return Err(MailError::Auth),
    }
    write.write_all(b"a2 STATUS INBOX (UNSEEN)\r\n").await.map_err(|_| MailError::Network)?;
    let mut unseen = None;
    let status = until_tagged(&mut reader, "a2", &mut unseen).await?;
    // The count is in hand; a failing LOGOUT changes nothing.
    let _ = write.write_all(b"a3 LOGOUT\r\n").await;
    let _ = write.shutdown().await;
    match (status, unseen) {
        (Reply::Ok, Some(n)) => Ok(n),
        _ => Err(MailError::Network),
    }
}

/// The number of unseen messages in INBOX of one account. TLS only (no STARTTLS, no plain login); the certificate is checked by
/// the platform verifier. The whole dialog has `TIMEOUT`.
pub async fn unseen_count(host: &str, port: u16, user: &str, password: &str) -> Result<u32, MailError> {
    let work = async {
        let name = ServerName::try_from(host.to_string()).map_err(|_| MailError::Network)?;
        let config = ClientConfig::with_platform_verifier().map_err(|_| MailError::Network)?;
        let tcp = TcpStream::connect((host, port)).await.map_err(|_| MailError::Network)?;
        let tls = TlsConnector::from(Arc::new(config)).connect(name, tcp).await.map_err(|_| MailError::Network)?;
        dialog(tls, user, password).await
    };
    tokio::time::timeout(TIMEOUT, work).await.map_err(|_| MailError::Network)?
}

/// The `mail` section from what one round found; accounts without a result keep `unread: null`.
pub fn build_info(accounts: &[(MailAccount, Result<u32, MailError>)], now_ms: u64) -> MailInfo {
    MailInfo {
        accounts: accounts
            .iter()
            .map(|(a, r)| MailAccountInfo {
                id: a.id.clone(),
                name: a.name.clone(),
                unread: r.as_ref().ok().copied(),
                error: r.as_ref().err().map(|e| e.as_str().to_string()),
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

    /// The dialog on text instead of a socket: what is sent and how the answers are read.
    async fn run(server_says: &str) -> (Result<u32, MailError>, String) {
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
        let result = dialog(client, "me", "p\"w").await;
        (result, task.await.unwrap())
    }

    #[tokio::test]
    async fn a_good_dialog_counts_unseen_and_sends_three_commands() {
        let (result, sent) = run("* OK ready\r\n* CAPABILITY IMAP4rev1\r\na1 OK LOGIN completed\r\n* STATUS INBOX (UNSEEN 4)\r\na2 OK STATUS completed\r\n").await;
        assert_eq!(result, Ok(4));
        assert_eq!(sent, "a1 LOGIN \"me\" \"p\\\"w\"\r\na2 STATUS INBOX (UNSEEN)\r\na3 LOGOUT\r\n");
    }

    #[tokio::test]
    async fn a_refused_login_is_an_auth_error_and_nothing_more_is_sent() {
        let (result, sent) = run("* OK ready\r\na1 NO [AUTHENTICATIONFAILED] bad\r\n").await;
        assert_eq!(result, Err(MailError::Auth));
        assert_eq!(sent, "a1 LOGIN \"me\" \"p\\\"w\"\r\n");
        let (result, _) = run("* OK ready\r\na1 BAD nope\r\n").await;
        assert_eq!(result, Err(MailError::Auth));
    }

    #[tokio::test]
    async fn a_strange_server_is_a_network_error() {
        assert_eq!(run("* BYE go away\r\n").await.0, Err(MailError::Network));
        assert_eq!(run("* OK ready\r\na1 OK done\r\na2 OK done but no count\r\n").await.0, Err(MailError::Network));
        assert_eq!(run("").await.0, Err(MailError::Network));
    }

    #[test]
    fn variables_follow_the_accounts_that_answered() {
        let account = |id: &str| MailAccount { id: id.into(), name: id.into(), host: "h".into(), port: 993, user: "u".into(), password: None, password_set: None, open_action_id: None };
        let info = build_info(&[(account("home"), Ok(3)), (account("work"), Err(MailError::Auth)), (account("old"), Ok(2))], 5);
        assert_eq!(info.accounts[1].unread, None);
        assert_eq!(info.accounts[1].error.as_deref(), Some("auth"));
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
