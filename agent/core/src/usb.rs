//! USB mode: keeps `adb reverse tcp:<port> tcp:<port>` in place for phones connected by cable
//! (ARCHITECTURE.md 5.5). adb is not shipped with Decka; its path comes from `usb.adbPath`.

use std::collections::HashSet;
use std::io;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;

use crate::agent::{Agent, UsbDevice};

const POLL: Duration = Duration::from_secs(5);
const RETRY_WITHOUT_ADB: Duration = Duration::from_secs(30);
const FORGET_EVERY: Duration = Duration::from_secs(60);
const ADB_TIMEOUT: Duration = Duration::from_secs(4);

/// What the loop knows; `Agent` adds `enabled` from the config when it builds `AgentStatus`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsbState {
    pub adb_found: bool,
    pub devices: Vec<UsbDevice>,
    pub last_error: Option<String>,
}

/// Parses `adb devices` output into (serial, state) pairs.
pub fn parse_devices(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('*') && !l.starts_with("List of devices attached"))
        .filter_map(|l| {
            let mut fields = l.split_whitespace();
            Some((fields.next()?.to_string(), fields.next()?.to_string()))
        })
        .collect()
}

/// Runs adb with a 4 second limit; returns (success, stdout, stderr).
async fn run_adb(adb: &str, args: &[&str]) -> io::Result<(bool, String, String)> {
    let mut command = tokio::process::Command::new(adb);
    command.args(args).stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let output = tokio::time::timeout(ADB_TIMEOUT, command.output())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "adb did not answer within 4 s"))??;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

/// Starts the loop on `runtime`; the caller aborts the returned handle to stop it (which also forgets
/// every device, so a restart begins from a clean set).
pub fn spawn(runtime: &tokio::runtime::Handle, agent: Agent) -> JoinHandle<()> {
    runtime.spawn(async move {
        let mut reversed: HashSet<String> = HashSet::new();
        let mut cleared_at = Instant::now();
        loop {
            let adb = agent.config().usb.adb_path;
            let Some(port) = agent.listening_port() else {
                // No server, nothing to forward to.
                agent.set_usb_state(UsbState::default());
                tokio::time::sleep(POLL).await;
                continue;
            };
            // Self-healing after PC sleep or an adb server restart.
            if cleared_at.elapsed() >= FORGET_EVERY {
                reversed.clear();
                cleared_at = Instant::now();
            }

            let pause = match run_adb(&adb, &["devices"]).await {
                Err(e) => {
                    reversed.clear();
                    let missing = e.kind() != io::ErrorKind::TimedOut;
                    agent.set_usb_state(UsbState { adb_found: !missing, devices: Vec::new(), last_error: Some(format!("cannot run \"{adb}\": {e}")) });
                    if missing { RETRY_WITHOUT_ADB } else { POLL }
                }
                Ok((_, stdout, _)) => {
                    let found = parse_devices(&stdout);
                    reversed.retain(|serial| found.iter().any(|(s, _)| s == serial));
                    let mut last_error = None;
                    for (serial, state) in &found {
                        if state == "unauthorized" {
                            last_error = Some(format!("device {serial} is unauthorized: accept the USB debugging prompt on the phone"));
                        } else if state == "device" && !reversed.contains(serial) {
                            let spec = format!("tcp:{port}");
                            match run_adb(&adb, &["-s", serial, "reverse", &spec, &spec]).await {
                                Ok((true, _, _)) => {
                                    reversed.insert(serial.clone());
                                }
                                Ok((false, out, err)) => {
                                    let text = if err.trim().is_empty() { out } else { err };
                                    last_error = Some(format!("adb reverse failed for {serial}: {}", text.trim()));
                                }
                                Err(e) => last_error = Some(format!("adb reverse failed for {serial}: {e}")),
                            }
                        }
                    }
                    let devices = found.iter().map(|(serial, _)| UsbDevice { serial: serial.clone(), reversed: reversed.contains(serial) }).collect();
                    agent.set_usb_state(UsbState { adb_found: true, devices, last_error });
                    POLL
                }
            };
            tokio::time::sleep(pause).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_devices_one_unauthorized() {
        let out = "List of devices attached\nR58M12ABCDE\tdevice\n0123456789ABCDEF\tunauthorized\n\n";
        assert_eq!(
            parse_devices(out),
            vec![("R58M12ABCDE".to_string(), "device".to_string()), ("0123456789ABCDEF".to_string(), "unauthorized".to_string())]
        );
    }

    #[test]
    fn empty_list() {
        assert!(parse_devices("List of devices attached\n\n").is_empty());
        assert!(parse_devices("").is_empty());
    }

    #[test]
    fn daemon_chatter_is_skipped() {
        let out = "* daemon not running; starting now at tcp:5037\n* daemon started successfully\nList of devices attached\nR58M12ABCDE\tdevice\n";
        assert_eq!(parse_devices(out), vec![("R58M12ABCDE".to_string(), "device".to_string())]);
    }

    #[test]
    fn crlf_and_offline_devices() {
        let out = "List of devices attached\r\nemulator-5554   offline\r\n";
        assert_eq!(parse_devices(out), vec![("emulator-5554".to_string(), "offline".to_string())]);
    }
}
