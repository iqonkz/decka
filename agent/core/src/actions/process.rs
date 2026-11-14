//! launch / open_url / system (DATA_MODEL.md 2.3). Always `Command` without a shell; spawn and forget.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use crate::model::SystemOp;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn spawn(mut cmd: Command, no_window: bool) -> Result<(), String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if no_window {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // The child is not awaited: success means the process was started.
    cmd.spawn().map(drop).map_err(|e| e.to_string())
}

pub fn launch(program: &str, args: &[String], cwd: Option<&str>, hidden: bool) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    spawn(cmd, hidden)
}

pub fn open_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("unsupported URL scheme".to_string());
    }
    let mut cmd = Command::new("rundll32.exe");
    cmd.arg("url.dll,FileProtocolHandler").arg(parsed.as_str());
    spawn(cmd, true)
}

pub fn system(op: SystemOp) -> Result<(), String> {
    let mut cmd = Command::new("rundll32.exe");
    match op {
        SystemOp::Lock => cmd.arg("user32.dll,LockWorkStation"),
        SystemOp::Sleep => cmd.arg("powrprof.dll,SetSuspendState").arg("0,1,0"),
    };
    spawn(cmd, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launching_a_missing_program_fails() {
        assert!(launch("decka-no-such-program.exe", &[], None, false).is_err());
    }

    #[test]
    fn open_url_rejects_non_http_schemes() {
        assert_eq!(open_url("file:///c:/x"), Err("unsupported URL scheme".to_string()));
        assert!(open_url("not a url").is_err());
    }
}
