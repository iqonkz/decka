//! Log file of the tray application: `<configDir>\logs\decka.log` (headless keeps writing to stderr).

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

pub fn logs_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("logs")
}

pub fn log_level() -> tracing::Level {
    match std::env::var("DECKA_LOG").unwrap_or_default().to_ascii_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    }
}

/// A log larger than `max` bytes becomes `<name>.1`; an older `.1` is deleted.
pub fn rotate_if_large(log: &Path, max: u64) -> io::Result<()> {
    let Ok(meta) = fs::metadata(log) else { return Ok(()) };
    if meta.len() <= max {
        return Ok(());
    }
    let mut old = log.as_os_str().to_owned();
    old.push(".1");
    let old = PathBuf::from(old);
    if old.exists() {
        fs::remove_file(&old)?;
    }
    fs::rename(log, old)
}

fn open_log(dir: &Path) -> io::Result<File> {
    let logs = logs_dir(dir);
    fs::create_dir_all(&logs)?;
    let path = logs.join("decka.log");
    rotate_if_large(&path, MAX_LOG_BYTES)?;
    OpenOptions::new().create(true).append(true).open(path)
}

/// Starts logging into `<dir>\logs\decka.log` (append). Falls back to stderr when the file cannot be opened.
pub fn init_file_logging(dir: &Path) {
    match open_log(dir) {
        Ok(file) => {
            let _ = tracing_subscriber::fmt().with_max_level(log_level()).with_ansi(false).with_writer(Mutex::new(file)).try_init();
        }
        Err(e) => {
            let _ = tracing_subscriber::fmt().with_max_level(log_level()).try_init();
            tracing::warn!("cannot open the log file: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("decka-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_small_log_is_kept() {
        let dir = temp_dir("small");
        let log = dir.join("decka.log");
        fs::write(&log, "x").unwrap();
        rotate_if_large(&log, 10).unwrap();
        assert!(log.exists() && !dir.join("decka.log.1").exists());
    }

    #[test]
    fn a_large_log_becomes_dot_one_and_replaces_the_old_one() {
        let dir = temp_dir("large");
        let log = dir.join("decka.log");
        fs::write(&log, "0123456789ab").unwrap();
        fs::write(dir.join("decka.log.1"), "older").unwrap();
        rotate_if_large(&log, 10).unwrap();
        assert!(!log.exists());
        assert_eq!(fs::read_to_string(dir.join("decka.log.1")).unwrap(), "0123456789ab");
    }

    #[test]
    fn a_missing_log_is_fine() {
        let dir = temp_dir("missing");
        rotate_if_large(&dir.join("decka.log"), 10).unwrap();
    }

    #[test]
    fn opening_creates_the_folder_and_appends() {
        let dir = temp_dir("open");
        drop(open_log(&dir).unwrap());
        assert!(logs_dir(&dir).join("decka.log").exists());
    }
}
