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

/// What a system operation runs (DATA_MODEL.md 10.8): a program with arguments, or a call into Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemPlan {
    Program { program: &'static str, args: &'static [&'static str] },
    DisplayOff,
    EmptyRecycleBin,
}

/// Pure mapping from the operation to what is run; nothing is executed here.
pub fn system_plan(op: SystemOp) -> SystemPlan {
    let program = |program, args| SystemPlan::Program { program, args };
    match op {
        SystemOp::Lock => program("rundll32.exe", &["user32.dll,LockWorkStation"]),
        SystemOp::Sleep => program("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"]),
        SystemOp::Shutdown => program("shutdown.exe", &["/s", "/t", "0"]),
        SystemOp::Restart => program("shutdown.exe", &["/r", "/t", "0"]),
        SystemOp::Signout => program("shutdown.exe", &["/l"]),
        SystemOp::Hibernate => program("shutdown.exe", &["/h"]),
        SystemOp::TaskManager => program("taskmgr.exe", &[]),
        SystemOp::Settings => program("explorer.exe", &["ms-settings:"]),
        SystemOp::Explorer => program("explorer.exe", &[]),
        SystemOp::DisplayOff => SystemPlan::DisplayOff,
        SystemOp::EmptyRecycleBin => SystemPlan::EmptyRecycleBin,
    }
}

pub fn system(op: SystemOp) -> Result<(), String> {
    match system_plan(op) {
        SystemPlan::Program { program, args } => {
            let mut cmd = Command::new(program);
            cmd.args(args);
            if op == SystemOp::Hibernate {
                // Hibernation can be switched off in Windows: the exit code tells, so this one is awaited.
                cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW);
                let status = cmd.status().map_err(|e| e.to_string())?;
                return if status.success() { Ok(()) } else { Err("Hibernation is turned off in Windows".to_string()) };
            }
            spawn(cmd, true)
        }
        SystemPlan::DisplayOff => display_off(),
        SystemPlan::EmptyRecycleBin => empty_recycle_bin(),
    }
}

/// Turns the monitors off: `WM_SYSCOMMAND` / `SC_MONITORPOWER` with 2 (off), broadcast.
fn display_off() -> Result<(), String> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, HWND_BROADCAST, SC_MONITORPOWER, WM_SYSCOMMAND};
    unsafe { PostMessageW(Some(HWND_BROADCAST), WM_SYSCOMMAND, WPARAM(SC_MONITORPOWER as usize), LPARAM(2)) }.map_err(|e| e.to_string())
}

/// Empties the recycle bin without questions; an empty bin is not an error.
fn empty_recycle_bin() -> Result<(), String> {
    use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND};
    const S_FALSE_BIN_EMPTY: i32 = 0x8000_0100_u32 as i32; // the shell answers this when there is nothing to delete
    match unsafe { SHEmptyRecycleBinW(None, None, SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) } {
        Ok(()) => Ok(()),
        Err(e) if e.code().0 == S_FALSE_BIN_EMPTY || e.code().0 == 0 => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launching_a_missing_program_fails() {
        assert!(launch("decka-no-such-program.exe", &[], None, false).is_err());
    }

    #[test]
    fn system_ops_map_to_programs_and_arguments() {
        let program = |p: &'static str, a: &'static [&'static str]| SystemPlan::Program { program: p, args: a };
        assert_eq!(system_plan(SystemOp::Shutdown), program("shutdown.exe", &["/s", "/t", "0"]));
        assert_eq!(system_plan(SystemOp::Restart), program("shutdown.exe", &["/r", "/t", "0"]));
        assert_eq!(system_plan(SystemOp::Signout), program("shutdown.exe", &["/l"]));
        assert_eq!(system_plan(SystemOp::Hibernate), program("shutdown.exe", &["/h"]));
        assert_eq!(system_plan(SystemOp::TaskManager), program("taskmgr.exe", &[]));
        assert_eq!(system_plan(SystemOp::Settings), program("explorer.exe", &["ms-settings:"]));
        assert_eq!(system_plan(SystemOp::Explorer), program("explorer.exe", &[]));
        assert_eq!(system_plan(SystemOp::Lock), program("rundll32.exe", &["user32.dll,LockWorkStation"]));
        assert_eq!(system_plan(SystemOp::DisplayOff), SystemPlan::DisplayOff);
        assert_eq!(system_plan(SystemOp::EmptyRecycleBin), SystemPlan::EmptyRecycleBin);
    }

    #[test]
    fn open_url_rejects_non_http_schemes() {
        assert_eq!(open_url("file:///c:/x"), Err("unsupported URL scheme".to_string()));
        assert!(open_url("not a url").is_err());
    }
}
