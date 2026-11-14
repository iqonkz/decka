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
