use std::path::PathBuf;

/// Config directory: explicit override, then env DECKA_CONFIG_DIR, then `<platform config dir>/Decka`.
pub fn config_dir(override_dir: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = override_dir {
        return dir;
    }
    if let Some(dir) = std::env::var_os("DECKA_CONFIG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Decka")
}
