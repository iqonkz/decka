#[cfg(windows)]
pub mod keys;
#[cfg(windows)]
pub mod process;
pub mod testing;
pub mod windows;

pub use windows::WindowsRunner;

use crate::model::Action;

/// Executes an action on the PC. Implementations must not block for long: spawn and return.
pub trait ActionRunner: Send + Sync {
    fn run(&self, id: &str, action: &Action) -> Result<(), String>;
}

/// Prints `DRY <id> <type>` instead of doing anything (headless tests, `--dry-run`).
pub struct DryRunner;

impl ActionRunner for DryRunner {
    fn run(&self, id: &str, action: &Action) -> Result<(), String> {
        println!("DRY {} {}", id, action.type_name());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SystemOp;

    /// The dry runner never reaches `process::system`: even the most destructive operations only answer `Ok`.
    #[test]
    fn dry_run_runs_no_system_operation() {
        for op in [SystemOp::Shutdown, SystemOp::Restart, SystemOp::Signout, SystemOp::Hibernate, SystemOp::DisplayOff, SystemOp::EmptyRecycleBin] {
            let action = Action::System { name: "x".into(), toggle_var: None, op };
            assert_eq!(DryRunner.run("x", &action), Ok(()));
        }
    }

    #[test]
    fn new_system_ops_parse_and_old_ones_still_do() {
        for name in ["lock", "sleep", "shutdown", "restart", "signout", "hibernate", "display_off", "task_manager", "settings", "explorer", "empty_recycle_bin"] {
            let json = format!(r#"{{"type":"system","name":"x","op":"{name}"}}"#);
            assert!(serde_json::from_str::<Action>(&json).is_ok(), "{name}");
        }
        assert!(serde_json::from_str::<Action>(r#"{"type":"system","name":"x","op":"format_c"}"#).is_err());
    }
}
