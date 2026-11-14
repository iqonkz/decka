//! Watches actions.json and layout.json in the config directory (DATA_MODEL.md 1).

use std::collections::HashSet;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::agent::Agent;

pub const WATCHED: [&str; 2] = ["actions.json", "layout.json"];
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Keeps the OS watcher and the debounce task alive; dropping it stops both.
pub struct WatchHandle {
    _watcher: RecommendedWatcher,
    task: JoinHandle<()>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn spawn(agent: &Agent) -> notify::Result<WatchHandle> {
    let (tx, mut rx) = mpsc::unbounded_channel::<&'static str>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if event.kind.is_access() {
            return;
        }
        for path in &event.paths {
            let file = path.file_name().and_then(|n| n.to_str());
            if let Some(name) = WATCHED.iter().find(|w| Some(**w) == file) {
                let _ = tx.send(name);
            }
        }
    })?;
    watcher.watch(agent.config_dir(), RecursiveMode::NonRecursive)?;

    let agent = agent.clone();
    let task = tokio::spawn(async move {
        loop {
            let Some(first) = rx.recv().await else { return };
            let mut pending: HashSet<&'static str> = HashSet::from([first]);
            // Debounce: wait until no event arrived for 300 ms.
            loop {
                match timeout(DEBOUNCE, rx.recv()).await {
                    Ok(Some(name)) => {
                        pending.insert(name);
                    }
                    Ok(None) => return,
                    Err(_) => break,
                }
            }
            // WATCHED order: actions first, so a new layout is validated against new actions.
            for name in WATCHED {
                if pending.contains(name) {
                    agent.reload_file(name);
                }
            }
        }
    });
    Ok(WatchHandle { _watcher: watcher, task })
}
