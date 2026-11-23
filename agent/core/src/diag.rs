//! Connection diagnostics: why a phone cannot reach the agent. Every check is one line with a status the window words in
//! both languages; nothing here changes the system except `fix_firewall`, which asks Windows for administrator rights.

use std::time::Duration;

use serde::Serialize;
use tokio::net::TcpStream;

use crate::agent::AgentStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagStatus {
    Ok,
    Warn,
    Fail,
    /// The check could not run here (not Windows, PowerShell missing).
    Unknown,
}

/// Mirrors `DiagCheck` in shared/src/ipc.ts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagCheck {
    pub id: String,
    pub status: DiagStatus,
    /// A short value for the sentence (an address, a rule name); empty when there is none.
    pub detail: String,
}

fn check(id: &str, status: DiagStatus, detail: impl Into<String>) -> DiagCheck {
    DiagCheck { id: id.to_string(), status, detail: detail.into() }
}

/// One line of the firewall probe: `rule|<name>|<action>|<profile>` or `net|<interface>|<category>`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FirewallFacts {
    /// (name, action, profiles) of the enabled inbound rules of this program.
    pub rules: Vec<(String, String, String)>,
    /// (interface, category) of the connected networks: `Public`, `Private` or `DomainAuthenticated`.
    pub networks: Vec<(String, String)>,
}

pub fn parse_firewall_probe(text: &str) -> FirewallFacts {
    let mut facts = FirewallFacts::default();
    for line in text.lines() {
        let parts: Vec<&str> = line.trim().split('|').collect();
        match parts.as_slice() {
            ["rule", name, action, profile] => facts.rules.push(((*name).to_string(), (*action).to_string(), (*profile).to_string())),
            ["net", name, category] => facts.networks.push(((*name).to_string(), (*category).to_string())),
            _ => {}
        }
    }
    facts
}

fn covers(profile: &str, wanted: &str) -> bool {
    profile.split(',').map(str::trim).any(|p| p.eq_ignore_ascii_case(wanted) || p.eq_ignore_ascii_case("any"))
}

/// The firewall verdict: a blocking rule (what the first-run Windows prompt leaves when it is cancelled) wins over any allow
/// rule; the networks the PC is on now must be covered by an allow rule.
pub fn evaluate_firewall(facts: &FirewallFacts) -> DiagCheck {
    let networks: Vec<&str> = facts.networks.iter().map(|(_, c)| c.as_str()).collect();
    let on = |category: &str| networks.iter().any(|c| c.eq_ignore_ascii_case(category));
    let current: Vec<&str> = ["Public", "Private"].into_iter().filter(|c| on(c)).collect();
    let current = if current.is_empty() { vec!["Public", "Private"] } else { current };

    if let Some((name, _, _)) = facts.rules.iter().find(|(_, action, profile)| action.eq_ignore_ascii_case("block") && current.iter().any(|c| covers(profile, c))) {
        return check("firewall", DiagStatus::Fail, format!("block:{name}"));
    }
    let allowed = |c: &str| facts.rules.iter().any(|(_, action, profile)| action.eq_ignore_ascii_case("allow") && covers(profile, c));
    let missing: Vec<&str> = current.iter().copied().filter(|c| !allowed(c)).collect();
    if missing.is_empty() {
        check("firewall", DiagStatus::Ok, current.join("+"))
    } else {
        check("firewall", DiagStatus::Fail, format!("missing:{}", missing.join("+")))
    }
}

/// The checks that need only the agent's own status: the server, the bind address and the PC's addresses.
pub fn status_checks(status: &AgentStatus) -> Vec<DiagCheck> {
    let mut out = Vec::new();
    match &status.listening {
        Some(l) => {
            out.push(check("server", DiagStatus::Ok, format!("{}:{}", l.bind, l.port)));
            if l.bind == "0.0.0.0" {
                out.push(check("bind", DiagStatus::Ok, ""));
            } else {
                out.push(check("bind", DiagStatus::Warn, l.bind.clone()));
            }
        }
        None => out.push(check("server", DiagStatus::Fail, status.listen_error.clone().unwrap_or_default())),
    }
    if status.lan_ips.is_empty() {
        out.push(check("address", DiagStatus::Fail, ""));
    } else {
        out.push(check("address", DiagStatus::Ok, status.lan_ips.join(", ")));
    }
    out
}

/// Opens a TCP connection to the agent's own port on every LAN address: proves the server answers on them.
async fn reach_check(status: &AgentStatus) -> Option<DiagCheck> {
    let port = status.listening.as_ref()?.port;
    if status.lan_ips.is_empty() {
        return None;
    }
    let mut failed = Vec::new();
    for ip in &status.lan_ips {
        let ok = matches!(tokio::time::timeout(Duration::from_millis(1500), TcpStream::connect((ip.as_str(), port))).await, Ok(Ok(_)));
        if !ok {
            failed.push(ip.clone());
        }
    }
    Some(if failed.is_empty() { check("reach", DiagStatus::Ok, status.lan_ips.join(", ")) } else { check("reach", DiagStatus::Fail, failed.join(", ")) })
}

#[cfg(windows)]
const PROBE: &str = r#"
$exe = '__EXE__'
try {
  Get-NetFirewallApplicationFilter | Where-Object { $_.Program -eq $exe } | Get-NetFirewallRule | Where-Object { $_.Direction -eq 'Inbound' -and $_.Enabled -eq 'True' } | ForEach-Object { "rule|$($_.DisplayName)|$($_.Action)|$($_.Profile)" }
  Get-NetConnectionProfile | ForEach-Object { "net|$($_.InterfaceAlias)|$($_.NetworkCategory)" }
} catch { }
"#;

#[cfg(windows)]
async fn run_powershell(script: String) -> Option<String> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use std::os::windows::process::CommandExt;
    // -EncodedCommand (UTF-16LE, base64) keeps the quotes of the script out of the command line.
    let encoded = STANDARD.encode(script.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>());
    let task = tokio::task::spawn_blocking(move || {
        std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded]).creation_flags(0x0800_0000).output()
    });
    let output = tokio::time::timeout(Duration::from_secs(25), task).await.ok()?.ok()?.ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(windows)]
async fn firewall_check() -> DiagCheck {
    let exe = match std::env::current_exe() {
        Ok(p) => p.display().to_string().replace('\'', "''"),
        Err(_) => return check("firewall", DiagStatus::Unknown, ""),
    };
    match run_powershell(PROBE.replace("__EXE__", &exe)).await {
        Some(text) => evaluate_firewall(&parse_firewall_probe(&text)),
        None => check("firewall", DiagStatus::Unknown, ""),
    }
}

#[cfg(not(windows))]
async fn firewall_check() -> DiagCheck {
    check("firewall", DiagStatus::Unknown, "")
}

/// Every check, in the order the window lists them.
pub async fn run(status: &AgentStatus) -> Vec<DiagCheck> {
    let mut out = status_checks(status);
    if let Some(reach) = reach_check(status).await {
        out.push(reach);
    }
    out.push(firewall_check().await);
    out.push(check("clients", if status.clients.is_empty() { DiagStatus::Warn } else { DiagStatus::Ok }, status.clients.len().to_string()));
    out
}

/// Adds the allow rule for this program on private and public networks (Windows asks for administrator rights once).
pub async fn fix_firewall() -> Result<(), String> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string();
        let rule = format!(
            "/c netsh advfirewall firewall delete rule name=\"Decka\" & netsh advfirewall firewall add rule name=\"Decka\" dir=in action=allow program=\"{exe}\" enable=yes profile=private,public"
        );
        let script = format!("Start-Process -FilePath cmd.exe -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList '{}'", rule.replace('\'', "''"));
        run_powershell(script).await.map(drop).ok_or_else(|| "PowerShell did not answer".to_string())
    }
    #[cfg(not(windows))]
    {
        Err("Windows only".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Listening;

    fn status(listening: Option<Listening>, ips: &[&str]) -> AgentStatus {
        AgentStatus {
            version: String::new(),
            listening,
            listen_error: None,
            listen_busy: None,
            lan_ips: ips.iter().map(|s| s.to_string()).collect(),
            clients: Vec::new(),
            usb: crate::agent::UsbStatus { enabled: false, adb_found: false, devices: Vec::new(), last_error: None },
            focus_app: None,
            config_dir: String::new(),
        }
    }

    #[test]
    fn probe_lines_are_parsed_and_noise_is_ignored() {
        let facts = parse_firewall_probe("rule|Decka|Allow|Private, Public\nnoise\nnet|Wi-Fi|Public\n");
        assert_eq!(facts.rules, vec![("Decka".into(), "Allow".into(), "Private, Public".into())]);
        assert_eq!(facts.networks, vec![("Wi-Fi".into(), "Public".into())]);
    }

    #[test]
    fn an_allow_rule_for_the_current_network_is_ok() {
        let f = parse_firewall_probe("rule|Decka|Allow|Private, Public\nnet|Wi-Fi|Public");
        assert_eq!(evaluate_firewall(&f).status, DiagStatus::Ok);
        let any = parse_firewall_probe("rule|Decka|Allow|Any\nnet|Wi-Fi|Private");
        assert_eq!(evaluate_firewall(&any).status, DiagStatus::Ok);
    }

    #[test]
    fn a_private_only_rule_fails_on_a_public_network() {
        let f = parse_firewall_probe("rule|decka.exe|Allow|Private\nnet|Hotspot|Public");
        let c = evaluate_firewall(&f);
        assert_eq!(c.status, DiagStatus::Fail);
        assert_eq!(c.detail, "missing:Public");
    }

    #[test]
    fn a_block_rule_wins_over_an_allow_rule() {
        let f = parse_firewall_probe("rule|Decka|Allow|Any\nrule|decka.exe|Block|Public\nnet|Wi-Fi|Public");
        let c = evaluate_firewall(&f);
        assert_eq!(c.status, DiagStatus::Fail);
        assert_eq!(c.detail, "block:decka.exe");
        // A block on a network the PC is not on does not matter.
        let other = parse_firewall_probe("rule|Decka|Allow|Any\nrule|decka.exe|Block|Public\nnet|Home|Private");
        assert_eq!(evaluate_firewall(&other).status, DiagStatus::Ok);
    }

    #[test]
    fn no_rule_at_all_fails() {
        assert_eq!(evaluate_firewall(&parse_firewall_probe("net|Wi-Fi|Private")).status, DiagStatus::Fail);
    }

    #[test]
    fn status_checks_cover_server_bind_and_addresses() {
        let ok = status_checks(&status(Some(Listening { bind: "0.0.0.0".into(), port: 9247 }), &["192.168.1.5"]));
        assert!(ok.iter().all(|c| c.status == DiagStatus::Ok));
        let local = status_checks(&status(Some(Listening { bind: "127.0.0.1".into(), port: 9247 }), &[]));
        assert_eq!(local.iter().find(|c| c.id == "bind").unwrap().status, DiagStatus::Warn);
        assert_eq!(local.iter().find(|c| c.id == "address").unwrap().status, DiagStatus::Fail);
        let down = status_checks(&status(None, &["10.0.0.2"]));
        assert_eq!(down[0].status, DiagStatus::Fail);
    }

    /// Runs the real read-only probe (PowerShell); run it by hand with `-- --ignored`.
    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn the_real_probe_answers() {
        let c = firewall_check().await;
        println!("{c:?}");
        assert_ne!(c.status, DiagStatus::Unknown);
    }

    #[tokio::test]
    async fn the_reach_check_connects_to_a_listening_port() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let s = status(Some(Listening { bind: "127.0.0.1".into(), port }), &["127.0.0.1"]);
        assert_eq!(reach_check(&s).await.unwrap().status, DiagStatus::Ok);
        drop(listener);
        let closed = reach_check(&s).await.unwrap();
        assert_eq!(closed.status, DiagStatus::Fail);
    }
}
