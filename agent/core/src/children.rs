//! T32.H.4: Decka leaves nothing behind. Every helper it starts (the model servers, the worker of the wake word) joins a
//! Job Object that kills it when the agent is gone (`ai::supervisor::Job`); this file finds what an earlier run left anyway
//! and ends it before new ones are started.

use std::path::Path;

/// What is needed to know about a process.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub pid: u32,
    pub parent: Option<u32>,
    /// Seconds since 1970.
    pub started: u64,
    pub exe: String,
    pub args: Vec<String>,
}

/// The helpers of an earlier run: programs of the AI package (`root`) and workers of the wake word whose parent is gone. A
/// process id that was given to a younger process is not a parent. Helpers of a Decka that runs have a living parent and
/// are left alone.
pub fn orphans(all: &[Seen], root: &Path, me: u32) -> Vec<u32> {
    let root = format!("{}\\", root.to_string_lossy().to_lowercase().trim_end_matches('\\'));
    all.iter()
        .filter(|p| p.pid != me)
        .filter(|p| {
            let exe = p.exe.to_lowercase();
            let worker = exe.ends_with("\\decka-agent.exe") && p.args.iter().any(|a| a == crate::voice::wake_engine::FLAG);
            exe.starts_with(&root) || worker
        })
        .filter(|p| !p.parent.is_some_and(|parent| all.iter().any(|o| o.pid == parent && o.started <= p.started)))
        .map(|p| p.pid)
        .collect()
}

/// Ends the orphans; returns how many.
pub fn sweep(root: &Path) -> usize {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut system = match System::new() {
        Ok(system) => system,
        Err(e) => {
            tracing::warn!("cannot look for helpers of an earlier run: {e}");
            return 0;
        }
    };
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::Always).with_cmd(UpdateKind::Always));
    let seen: Vec<Seen> = system
        .processes()
        .iter()
        .map(|(pid, p)| Seen {
            pid: pid.as_u32(),
            parent: p.parent().map(|x| x.as_u32()),
            started: p.start_time(),
            exe: p.exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default(),
            args: p.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect(),
        })
        .collect();
    let mut ended = 0;
    for pid in orphans(&seen, root, std::process::id()) {
        if let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) {
            let name = process.name().to_string_lossy().into_owned();
            if process.kill() {
                tracing::warn!("ended {name} (pid {pid}) left by an earlier run");
                ended += 1;
            } else {
                tracing::warn!("{name} (pid {pid}) left by an earlier run could not be ended");
            }
        }
    }
    ended
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent: Option<u32>, started: u64, exe: &str, args: &[&str]) -> Seen {
        Seen { pid, parent, started, exe: exe.into(), args: args.iter().map(|a| a.to_string()).collect() }
    }

    #[test]
    fn only_helpers_without_a_parent_are_ended() {
        let root = Path::new(r"C:\Users\a\AppData\Local\Decka\ai");
        let llama = r"C:\Users\a\AppData\Local\Decka\AI\runtime\llama\llama-server.exe";
        let agent = r"C:\Program Files\Decka\decka-agent.exe";
        let all = [
            p(10, Some(1), 100, agent, &[agent]),
            // A running Decka and its helpers.
            p(11, Some(10), 110, llama, &[]),
            p(12, Some(10), 111, agent, &[agent, "--wake-worker"]),
            // Left by an earlier run: the parent is gone, or its id now belongs to a younger process.
            p(20, Some(999), 50, llama, &[]),
            p(21, Some(30), 50, agent, &[agent, "--wake-worker"]),
            p(30, Some(1), 500, r"C:\Windows\notepad.exe", &[]),
            // Not ours: another folder, an agent that is not a worker, a program an action started.
            p(40, Some(999), 50, r"C:\tools\llama-server.exe", &[]),
            p(41, Some(999), 50, agent, &[agent, "--minimized"]),
            p(42, Some(999), 50, r"C:\Users\a\AppData\Local\Decka\ai-old\llama-server.exe", &[]),
        ];
        assert_eq!(orphans(&all, root, 10), [20, 21]);
        assert_eq!(orphans(&all, root, 20), [21], "never itself");
    }
}
