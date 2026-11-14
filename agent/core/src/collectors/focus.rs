//! `focus` collector: which program is in front on the PC (M11, DATA_MODEL.md 6.1).
//!
//! Only the executable file name is read, in lower case. The window title is NEVER read (ARCHITECTURE.md 6.4, S16).
//! A new value is published only after it has been in front for 700 ms without a break, so a quick
//! Alt+Tab through other windows does not make the phone flip pages.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::state::Store;

const POLL: Duration = Duration::from_millis(250);
/// How long a value must stay in front before it is published.
pub const HOLD_MS: u64 = 700;
/// Our own windows never count: while the settings window is in front the phone keeps its page.
pub const IGNORED: [&str; 2] = ["decka-agent.exe", "decka-headless.exe"];

fn is_ignored(app: &str) -> bool {
    IGNORED.contains(&app)
}

/// Turns raw samples into published values.
#[derive(Debug, Default)]
pub struct Stabilizer {
    /// (value, time it was first seen), for the value currently in front.
    candidate: Option<(Option<String>, u64)>,
    /// `None` = nothing published yet; `Some(v)` = `v` is the published value.
    published: Option<Option<String>>,
}

impl Stabilizer {
    /// Feeds one sample taken at `now_ms`; returns the new value when it just became the published one.
    /// Samples of our own programs are skipped as if they were never taken.
    pub fn push(&mut self, now_ms: u64, value: Option<String>) -> Option<Option<String>> {
        if value.as_deref().is_some_and(is_ignored) {
            return None;
        }
        match &self.candidate {
            Some((current, _)) if *current == value => {}
            _ => self.candidate = Some((value, now_ms)),
        }
        let (current, since) = self.candidate.as_ref().expect("candidate was just set");
        if now_ms.saturating_sub(*since) >= HOLD_MS && self.published.as_ref() != Some(current) {
            self.published = Some(current.clone());
            return Some(current.clone());
        }
        None
    }
}

/// The published value after a whole sequence of (time in ms, value) samples; `None` = nothing published yet.
pub fn stable_value(samples: &[(u64, Option<String>)]) -> Option<Option<String>> {
    let mut stabilizer = Stabilizer::default();
    let mut result = None;
    for (t, value) in samples {
        if let Some(published) = stabilizer.push(*t, value.clone()) {
            result = Some(published);
        }
    }
    result
}

/// Calls `on_change` with each newly published value (also from the collector thread).
pub type OnChange = Arc<dyn Fn(Option<String>) + Send + Sync>;

pub struct FocusCollector {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FocusCollector {
    pub fn start(store: Store, on_change: OnChange) -> FocusCollector {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("decka-focus-collector".to_string())
            .spawn(move || run(store, on_change, flag))
            .expect("spawn focus collector thread");
        FocusCollector { stop, thread: Some(thread) }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for FocusCollector {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(store: Store, on_change: OnChange, stop: Arc<AtomicBool>) {
    let started = Instant::now();
    let mut stabilizer = Stabilizer::default();
    while !stop.load(Ordering::Relaxed) {
        let now_ms = started.elapsed().as_millis() as u64;
        if let Some(app) = stabilizer.push(now_ms, foreground_app()) {
            store.set("focus", serde_json::json!({ "app": app }));
            on_change(app);
        }
        std::thread::sleep(POLL);
    }
}

/// Lower-case exe file name of the program that owns the foreground window, if it can be read and looks like an exe name.
#[cfg(windows)]
pub fn foreground_app() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    // SAFETY: plain Win32 calls with valid out-pointers; the process handle is closed by `exe_name_of`.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        exe_name_of(pid)
    }
}

#[cfg(windows)]
fn exe_name_of(pid: u32) -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};

    if pid == 0 {
        return None;
    }
    // SAFETY: the buffer outlives the call and `len` holds its size in characters.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        queried.ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        normalize_exe(&path)
    }
}

/// `C:\Program Files\Code\Code.exe` -> `code.exe`; names that do not look like an exe name -> `None`.
pub fn normalize_exe(path: &str) -> Option<String> {
    let file = path.rsplit(['\\', '/']).next()?.to_lowercase();
    crate::validate::is_app_name(&file).then_some(file)
}

#[cfg(not(windows))]
pub fn foreground_app() -> Option<String> {
    None
}

/// Exe names (lower case, unique, sorted) of processes that own a visible top-level window, without our own programs.
#[cfg(windows)]
pub fn list_window_apps() -> Vec<String> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible};

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the address of the Vec below, which lives until EnumWindows returns.
        let pids = unsafe { &mut *(lparam.0 as *mut Vec<u32>) };
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            pids.push(pid);
        }
        BOOL(1)
    }

    let mut pids: Vec<u32> = Vec::new();
    // SAFETY: see `visit`.
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut pids as *mut Vec<u32> as isize)) };
    pids.sort_unstable();
    pids.dedup();
    let mut apps: Vec<String> = pids.into_iter().filter_map(exe_name_of).filter(|a| !is_ignored(a)).collect();
    apps.sort();
    apps.dedup();
    apps
}

#[cfg(not(windows))]
pub fn list_window_apps() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn a_value_held_for_700_ms_is_published() {
        let samples: Vec<(u64, Option<String>)> = (0..=3).map(|i| (i * 250, s("code.exe"))).collect();
        assert_eq!(stable_value(&samples), Some(s("code.exe")));
    }

    #[test]
    fn a_short_burst_of_300_ms_is_not_published() {
        let mut samples: Vec<(u64, Option<String>)> = (0..=3).map(|i| (i * 250, s("code.exe"))).collect();
        samples.push((1000, s("chrome.exe")));
        samples.push((1250, s("chrome.exe"))); // only 250 ms in front
        samples.push((1500, s("code.exe")));
        assert_eq!(stable_value(&samples), Some(s("code.exe")));
    }

    #[test]
    fn a_new_value_is_published_after_700_ms_and_unknown_is_a_value() {
        let mut stabilizer = Stabilizer::default();
        assert_eq!(stabilizer.push(0, s("a.exe")), None);
        assert_eq!(stabilizer.push(750, s("a.exe")), Some(s("a.exe")));
        assert_eq!(stabilizer.push(1000, None), None);
        assert_eq!(stabilizer.push(1500, None), None);
        assert_eq!(stabilizer.push(1750, None), Some(None)); // the desktop: published as "unknown"
        assert_eq!(stabilizer.push(2000, None), None); // not published twice
    }

    #[test]
    fn our_own_programs_do_not_change_the_result() {
        let samples = vec![
            (0, s("code.exe")),
            (250, s("code.exe")),
            (500, s("decka-agent.exe")),
            (750, s("code.exe")),
            (1000, s("decka-agent.exe")),
            (1250, s("decka-headless.exe")),
            (1500, s("code.exe")),
        ];
        assert_eq!(stable_value(&samples), Some(s("code.exe")));
        // Even a long stay in the settings window leaves the previous value.
        let mut long: Vec<(u64, Option<String>)> = samples;
        long.extend((0..10).map(|i| (2000 + i * 250, s("decka-agent.exe"))));
        assert_eq!(stable_value(&long), Some(s("code.exe")));
    }

    #[test]
    fn exe_names_are_reduced_to_a_lower_case_file_name() {
        assert_eq!(normalize_exe(r"C:\Program Files\Microsoft VS Code\Code.exe"), s("code.exe"));
        assert_eq!(normalize_exe("C:/x/Zoom.EXE"), s("zoom.exe"));
        assert_eq!(normalize_exe(r"C:\x\readme.txt"), None);
        assert_eq!(normalize_exe(""), None);
    }
}
