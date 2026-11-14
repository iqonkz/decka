use super::ActionRunner;
use crate::model::Action;

/// Real executor for Windows. On other platforms every action fails with "unsupported platform".
pub struct WindowsRunner;

#[cfg(windows)]
impl ActionRunner for WindowsRunner {
    fn run(&self, _id: &str, action: &Action) -> Result<(), String> {
        use super::process;
        match action {
            Action::Launch { program, args, cwd, hidden, .. } => {
                process::launch(program, args.as_deref().unwrap_or(&[]), cwd.as_deref(), hidden.unwrap_or(false))
            }
            Action::OpenUrl { url, .. } => process::open_url(url),
            Action::System { op, .. } => process::system(*op),
            Action::Hotkey { .. } | Action::Media { .. } => Err("not implemented".to_string()), // T3.3
        }
    }
}

#[cfg(not(windows))]
impl ActionRunner for WindowsRunner {
    fn run(&self, _id: &str, _action: &Action) -> Result<(), String> {
        Err("unsupported platform".to_string())
    }
}
