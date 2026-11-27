//! "Save a problem report" (T31.3): a ZIP on the desktop with the versions, the hardware, the ends of the logs, the
//! settings and the result of "Check voice". Nothing is sent anywhere. No token, password or DPAPI blob goes into it, and
//! no text of the user: the settings keep only words that cannot be personal, the log lines are cleaned one by one.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::agent::Agent;
use crate::assistant::Local;

/// Lines of each log that go into the report.
pub const LOG_LINES: usize = 500;
const HIDDEN: &str = "<hidden>";

/// Texts of the settings that are fixed words of the program or the name of a public mail server. Every other text is hidden,
/// so a setting added later is hidden until somebody decides otherwise.
const OPEN_KEYS: &[&str] = &["bind", "units", "restingMode", "barPosition", "pcOverlay", "mode", "host"];
/// Numbers that say where the user is.
const PLACE_KEYS: &[&str] = &["lat", "lon"];

fn clean_value(key: &str, value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), clean_value(k, v))).collect::<Map<_, _>>()),
        Value::Array(items) => Value::Array(items.iter().map(|v| clean_value(key, v)).collect()),
        Value::String(text) if text.starts_with(crate::dpapi::PREFIX) => json!("<secret>"),
        Value::String(text) if text.is_empty() || OPEN_KEYS.contains(&key) => json!(text),
        Value::String(_) => json!(HIDDEN),
        Value::Number(_) if PLACE_KEYS.contains(&key) => json!(HIDDEN),
        other => other.clone(),
    }
}

/// `config.json` without secrets and personal texts.
pub fn clean_config(config: &Value) -> Value {
    clean_value("", config)
}

/// The texts that were hidden in the settings: they are cut out of the log lines too.
fn secret_texts(key: &str, value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => map.iter().for_each(|(k, v)| secret_texts(k, v, out)),
        Value::Array(items) => items.iter().for_each(|v| secret_texts(key, v, out)),
        Value::String(text) if text.len() >= 6 && !OPEN_KEYS.contains(&key) => out.push(text.clone()),
        _ => {}
    }
}

/// `"…"` for everything between quotes. A single quote counts only where it opens a text (not in "can't").
fn hide_quoted(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let opens = c == '"' || (c == '\'' && (i == 0 || matches!(chars[i - 1], ' ' | '=' | '(' | ':')));
        if !opens {
            out.push(c);
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < chars.len() && !(chars[end] == c && chars[end - 1] != '\\') {
            end += 1;
        }
        out.push(c);
        out.push('…');
        // An unclosed quote hides the rest of the line.
        if end < chars.len() {
            out.push(c);
        }
        i = end + 1;
    }
    out
}

/// Replaces what follows `mark` up to `until` (or the end of the line).
fn hide_after(line: &mut String, mark: &str, until: &str, with: &str) {
    if let Some(at) = line.find(mark) {
        let from = at + mark.len();
        let to = if until.is_empty() { None } else { line[from..].find(until) };
        let to = to.map_or(line.len(), |n| from + n);
        line.replace_range(from..to, with);
    }
}

/// `C:\Users\name\...` keeps everything but the name.
fn hide_user_folder(line: &mut String) {
    for mark in ["\\Users\\", "/Users/", "\\\\Users\\\\"] {
        let mut from = 0;
        while let Some(at) = line[from..].find(mark) {
            let start = from + at + mark.len();
            let end = line[start..].find(['\\', '/', '"', '\'']).map_or(line.len(), |n| start + n);
            line.replace_range(start..end, "…");
            from = start;
        }
    }
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '/' | '=')
}

/// Long runs of letters mixed with digits are tokens, keys or hashes; `dk_` is an API token at any length.
fn hide_tokens(word: &str) -> String {
    let mut out = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        let mixed = run.chars().any(|c| c.is_ascii_digit()) && run.chars().any(|c| c.is_ascii_alphabetic());
        if (run.len() >= 24 && mixed) || (run.starts_with("dk_") && run.len() >= 8) {
            out.push_str(HIDDEN);
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in word.chars() {
        if is_token_char(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// `192.168.1.20:9247` -> `192.168.x.x:9247`; loopback and `0.0.0.0` stay.
fn mask_address(word: &str) -> Option<String> {
    let start = word.find(|c: char| c.is_ascii_digit())?;
    let rest = &word[start..];
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
    let address: std::net::Ipv4Addr = rest[..end].trim_end_matches('.').parse().ok()?;
    if address.is_loopback() || address.is_unspecified() {
        return None;
    }
    let [a, b, _, _] = address.octets();
    Some(format!("{}{a}.{b}.x.x{}", &word[..start], &rest[end..]))
}

fn clean_word(word: &str, previous: &str) -> String {
    let lower = word.to_ascii_lowercase();
    let before = previous.to_ascii_lowercase();
    if before == "bearer" || ["password:", "token:", "secret:", "key:"].iter().any(|m| before.ends_with(m)) {
        return HIDDEN.into();
    }
    for mark in ["password=", "token=", "secret=", "key="] {
        if let Some(at) = lower.find(mark) {
            return format!("{}{HIDDEN}", &word[..at + mark.len()]);
        }
    }
    if let Some(at) = word.find("://") {
        let host = &word[at + 3..];
        return if host.starts_with("127.0.0.1") || host.starts_with("localhost") { hide_tokens(word) } else { format!("{}…", &word[..at + 3]) };
    }
    if word.split_once('@').is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.')) {
        return "<email>".into();
    }
    mask_address(word).unwrap_or_else(|| hide_tokens(word))
}

/// Runs of letters outside ASCII are texts in the user's language (a program name, a phrase, a Windows message).
fn hide_foreign(line: &str) -> String {
    let mut out = String::new();
    let mut hiding = false;
    for c in line.chars() {
        if !c.is_ascii() && c.is_alphabetic() {
            if !hiding {
                out.push('…');
            }
            hiding = true;
        } else {
            hiding = false;
            out.push(c);
        }
    }
    out
}

/// One log line for the report; `None` leaves it out. `secrets` and `names` are cut out wherever they appear.
pub fn clean_line(line: &str, secrets: &[String], names: &[String]) -> Option<String> {
    // The model servers print a lot; only what explains a failure to load or run them is kept.
    if line.contains("-server: ") {
        let lower = line.to_ascii_lowercase();
        if !["error", "fail", "warn", "system_info", "load_model", "offload", "vulkan", "cuda", "memory", "vram"].iter().any(|w| lower.contains(w)) {
            return None;
        }
    }
    let mut line = line.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            line = line.replace(secret.as_str(), HIDDEN);
        }
    }
    for name in names.iter().filter(|n| n.len() >= 3) {
        line = line.replace(name.as_str(), "<name>");
    }
    while let Some(at) = line.find(crate::dpapi::PREFIX) {
        let end = line[at..].find(|c: char| c.is_whitespace() || c == '"').map_or(line.len(), |n| at + n);
        line.replace_range(at..end, "<secret>");
    }
    // Places of the log where a name chosen by the user appears.
    hide_after(&mut line, "project=", " -> ", "…");
    hide_after(&mut line, "name=", " rate=", "…");
    hide_after(&mut line, " press ", " device=", "<action>");
    if line.contains(": system ") {
        hide_after(&mut line, " by ", "", "<device>");
    }
    hide_user_folder(&mut line);
    let line = hide_quoted(&line);
    let mut previous = "";
    let words: Vec<String> = line
        .split(' ')
        .map(|word| {
            let clean = clean_word(word, previous);
            if !word.is_empty() {
                previous = word;
            }
            clean
        })
        .collect();
    Some(hide_foreign(&words.join(" ")))
}

/// The last `LOG_LINES` clean lines of the files, oldest file first.
fn log_tail(files: &[PathBuf], secrets: &[String], names: &[String]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for file in files {
        let Ok(bytes) = std::fs::read(file) else { continue };
        lines.extend(String::from_utf8_lossy(&bytes).lines().filter_map(|l| clean_line(l, secrets, names)));
    }
    let skip = lines.len().saturating_sub(LOG_LINES);
    let mut text = lines.split_off(skip).join("\n");
    text.push('\n');
    text
}

/// The steps of "Check voice" without what was heard and understood.
pub fn clean_check(check: &Value, secrets: &[String], names: &[String]) -> Value {
    let Some(steps) = check.as_array() else {
        let text = check.get("error").and_then(Value::as_str).unwrap_or("not run");
        return json!({ "notRun": clean_line(text, secrets, names) });
    };
    Value::Array(
        steps
            .iter()
            .map(|step| {
                let mut out = Map::new();
                for key in ["id", "state", "durationMs", "reason", "errorKey", "confirm"] {
                    if let Some(v) = step.get(key) {
                        out.insert(key.into(), v.clone());
                    }
                }
                if let Some(error) = step.get("error").and_then(Value::as_str).and_then(|e| clean_line(e, secrets, names)) {
                    out.insert("error".into(), json!(error));
                }
                // The detail of the microphone step is its level; the others hold texts of the user.
                if step.get("id").and_then(Value::as_str) == Some("mic") {
                    if let Some(level) = step.get("detail") {
                        out.insert("level".into(), level.clone());
                    }
                }
                Value::Object(out)
            })
            .collect(),
    )
}

/// What the report needs from outside the agent.
#[derive(Debug, Clone, Default)]
pub struct Request {
    /// The folder the archive is written to (the desktop).
    pub out_dir: PathBuf,
    /// `voice.log`, which lies outside the config folder.
    pub voice_log: Option<PathBuf>,
    /// The answer of `voice_status`.
    pub voice_status: Value,
    /// The steps of "Check voice", or `{"error": "..."}` when it could not run.
    pub voice_check: Value,
    pub webview: Option<String>,
}

fn hardware(agent: &Agent) -> Value {
    use sysinfo::{CpuRefreshKind, Disks, RefreshKind, System};
    let system = System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing())).ok();
    let cpus = system.as_ref().map(|s| s.cpus()).unwrap_or_default();
    let cpu = cpus.first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
    let gpu = crate::ai::hardware::detect_hardware();
    let free = Disks::new_with_refreshed_list().ok().and_then(|disks| {
        disks
            .iter()
            .filter(|d| agent.config_dir().starts_with(d.mount_point()))
            .max_by_key(|d| d.mount_point().as_os_str().len())
            .map(|d| d.available_space() / (1024 * 1024 * 1024))
    });
    json!({ "windows": System::long_os_version().ok(), "kernel": System::kernel_version().ok(), "cpu": cpu, "cpuThreads": cpus.len(), "graphics": gpu, "diskFreeGb": free })
}

/// The files of the report.
pub fn build(agent: &Agent, request: &Request, at: &Local) -> Vec<(String, Vec<u8>)> {
    let config = serde_json::to_value(agent.config()).unwrap_or(Value::Null);
    let mut secrets = Vec::new();
    secret_texts("", &config, &mut secrets);
    // The longest first, so a secret that holds a shorter one is cut out whole.
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    let names: Vec<String> = ["USERNAME", "COMPUTERNAME", "USERDOMAIN"].iter().filter_map(|k| std::env::var(k).ok()).collect();

    let status = agent.status();
    let mut voice = Map::new();
    for key in ["state", "reason", "model", "downloadGb", "hardware"] {
        if let Some(v) = request.voice_status.get(key) {
            voice.insert(key.into(), v.clone());
        }
    }
    let mail: Vec<Value> = agent.mail_status().iter().enumerate().map(|(i, s)| json!({ "mailbox": i + 1, "reason": s.reason, "step": s.step, "noPassword": s.no_password })).collect();
    let system = json!({
        "made": format!("{:04}-{:02}-{:02} {:02}:{:02}", at.year, at.month, at.day, at.hour, at.minute),
        "decka": status.version,
        "webview": request.webview,
        "pc": hardware(agent),
        "voice": voice,
        "phonesConnected": status.clients.len(),
        "mail": mail,
    });

    let logs = crate::logging::logs_dir(agent.config_dir());
    let pretty = |v: &Value| serde_json::to_vec_pretty(v).unwrap_or_default();
    let mut files = vec![
        ("system.json".to_string(), pretty(&system)),
        ("config.json".to_string(), pretty(&clean_config(&config))),
        ("voice-check.json".to_string(), pretty(&clean_check(&request.voice_check, &secrets, &names))),
        ("decka.log".to_string(), log_tail(&[logs.join("decka.log.1"), logs.join("decka.log")], &secrets, &names).into_bytes()),
    ];
    if let Some(path) = &request.voice_log {
        files.push(("voice.log".to_string(), log_tail(std::slice::from_ref(path), &secrets, &names).into_bytes()));
    }
    files
}

/// Writes `decka-report-<date>-<time>.zip` into `request.out_dir`; an earlier report of the same minute is kept.
pub fn save(agent: &Agent, request: &Request) -> Result<PathBuf, String> {
    let at = Local::now();
    let bytes = crate::zipfile::write(&build(agent, request, &at), &at).map_err(|e| e.to_string())?;
    let stem = format!("decka-report-{:04}{:02}{:02}-{:02}{:02}", at.year, at.month, at.day, at.hour, at.minute);
    write_new(&request.out_dir, &stem, "zip", &bytes)
}

/// Writes a file that does not exist yet: `<stem>.<ext>`, else `<stem>-2.<ext>` and so on.
pub fn write_new(dir: &Path, stem: &str, ext: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    for n in 1..100 {
        let name = if n == 1 { format!("{stem}.{ext}") } else { format!("{stem}-{n}.{ext}") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => return file.write_all(bytes).map(|()| path).map_err(|e| format!("cannot write the file: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("cannot write the file: {e}")),
        }
    }
    Err("there are too many files with this name".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(line: &str) -> String {
        clean_line(line, &["me@example.org".to_string(), "Kitchen box".to_string()], &["seitz".to_string()]).unwrap_or_default()
    }

    #[test]
    fn tokens_passwords_and_blobs_leave_a_line() {
        let token = format!("dk_{}", "a1B2".repeat(11));
        for line in [
            format!("auth {token} refused"),
            "stored dpapi:AQAAANCMnd8BFdERjHoAwE/Cl+sBAAAA end".to_string(),
            "Authorization: Bearer hunter2".to_string(),
            "connect password=hunter2 now".to_string(),
            "token: hunter2".to_string(),
            "key 3f9a0c7d21b84e6f9a0c7d21b84e6f1122 stored".to_string(),
        ] {
            let out = clean(&line);
            assert!(!out.contains("hunter2") && !out.contains("dpapi:") && !out.contains(&token) && !out.contains("3f9a0c7d21"), "{line} -> {out}");
        }
    }

    #[test]
    fn texts_of_the_user_leave_a_line() {
        assert_eq!(clean(r#"INFO x: dropped toggleVar "audio.muted" from key"#), r#"INFO x: dropped toggleVar "…" from key"#);
        assert_eq!(clean("INFO x: press open-my-bank device=ab12 remote=192.168.1.31:5000 id=3"), "INFO x: press <action> device=ab12 remote=192.168.x.x:5000 id=3");
        assert_eq!(clean("INFO decka_core::agent: system lock by Anna's phone"), "INFO decka_core::agent: system lock by <device>");
        assert_eq!(clean("1 device name=Anna's AirPods rate=48000"), "1 device name=… rate=48000");
        assert_eq!(clean("WARN open C:\\Users\\Anna Smith\\notes.txt: denied"), "WARN open C:\\Users\\…\\notes.txt: denied");
        assert_eq!(clean("WARN webhook https://hooks.example.com/T00/B00 failed"), "WARN webhook https://… failed");
        assert_eq!(clean("WARN mail me@example.org and other@x.io in Kitchen box"), "WARN mail <hidden> and <email> in <hidden>");
        assert_eq!(clean("ERROR Не удается найти файл (os error 2)"), "ERROR … … … … (os error 2)");
        assert_eq!(clean("INFO home of seitz"), "INFO home of <name>");
        assert!(!clean("INFO x: can't open 'my secret plan.docx' today").contains("plan"));
        // What explains a problem stays.
        assert_eq!(clean("INFO decka_core::agent: listening on 0.0.0.0:9247"), "INFO decka_core::agent: listening on 0.0.0.0:9247");
    }

    #[test]
    fn the_chatter_of_the_model_servers_is_left_out() {
        assert_eq!(clean_line("INFO voice: llama-server: slot print_timing: eval time = 35 ms", &[], &[]), None);
        assert_eq!(clean_line("INFO voice: whisper-server: [00:00.000] open the door", &[], &[]), None);
        assert!(clean_line("INFO voice: llama-server: error: out of device memory", &[], &[]).is_some());
    }

    #[test]
    fn the_settings_keep_only_words_that_cannot_be_personal() {
        let config = json!({
            "bind": "0.0.0.0", "port": 9247, "agentName": "Anna-PC", "apiToken": "dk_x",
            "weather": { "enabled": true, "place": "Almaty", "lat": 43.2, "lon": 76.9, "units": "metric" },
            "mail": { "accounts": [{ "host": "imap.gmail.com", "user": "me@x.org", "password": "dpapi:AAA", "port": 993 }] },
            "ha": { "url": "http://home:8123", "token": "dpapi:BBB", "entities": ["light.anna"] },
            "somethingNew": { "refreshToken": "abc" }
        });
        let out = clean_config(&config);
        let text = out.to_string();
        for secret in ["Anna", "dk_x", "Almaty", "43.2", "me@x.org", "dpapi:", "home:8123", "light.anna", "abc"] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
        assert_eq!(out["port"], 9247);
        assert_eq!(out["mail"]["accounts"][0]["host"], "imap.gmail.com");
        assert_eq!(out["mail"]["accounts"][0]["password"], "<secret>");
        assert_eq!(out["weather"]["enabled"], true);
    }

    #[test]
    fn the_voice_check_loses_what_was_heard() {
        let check = json!([
            { "id": "mic", "state": "ok", "detail": "42 %" },
            { "id": "stt", "state": "ok", "durationMs": 10, "detail": "call my bank" },
            { "id": "action", "state": "ok", "understood": { "text": "Open Bank" } }
        ]);
        let text = clean_check(&check, &[], &[]).to_string();
        assert!(text.contains("42 %") && !text.contains("bank") && !text.contains("Bank"), "{text}");
        assert_eq!(clean_check(&json!({ "error": "Voice control is not installed" }), &[], &[])["notRun"], "Voice control is not installed");
    }
}
