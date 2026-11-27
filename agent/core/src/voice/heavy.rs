//! "What uses the memory" (TASKS.md T30.2): the heaviest programs of the user, and closing one of them on a "yes" -
//! politely first (`WM_CLOSE`, so the program can ask about unsaved work), by force only after a second "yes".
//!
//! What is never offered and never closed: everything that does not run as the user who runs Decka (SYSTEM and the
//! services), the parts of Windows in `SYSTEM_NAMES`, and Decka itself with the processes it started.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

pub const SHOWN: usize = 5;
/// A program gets this long to close by itself.
pub const CLOSE_WAIT: Duration = Duration::from_secs(5);
const STEP: Duration = Duration::from_millis(250);
/// The executor's reply that turns into the question "close it by force?" (the rest is the JSON of the program).
pub const FORCE_PREFIX: &str = "\u{1}force-close:";
pub const NOT_ALLOWED: &str = "Decka does not close this program";
pub const NOT_RUNNING: &str = "That program is not running any more";
pub const NOT_CLOSED: &str = "Windows did not let Decka close it";

/// Parts of Windows and of Decka, by the name of the file without `.exe`.
const SYSTEM_NAMES: &[&str] = &[
    "system", "registry", "secure system", "memory compression", "memcompression", "smss", "csrss", "wininit", "winlogon", "services", "lsass", "lsaiso",
    "svchost", "fontdrvhost", "dwm", "explorer", "sihost", "taskhostw", "ctfmon", "runtimebroker", "shellexperiencehost", "startmenuexperiencehost",
    "searchhost", "searchapp", "searchindexer", "textinputhost", "shellhost", "conhost", "dllhost", "audiodg", "spoolsv", "wudfhost", "msmpeng", "nissrv",
    "securityhealthservice", "securityhealthsystray", "smartscreen", "applicationframehost", "backgroundtaskhost", "systemsettings", "lockapp",
    "useroobebroker", "widgets", "widgetservice", "wmiprvse", "vmmem", "vmmemwsl", "vmmemwsl2", "msedgewebview2", "decka", "decka-agent", "decka-headless",
    "llama-server", "whisper-server",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub parent: Option<u32>,
    /// The full path of the program.
    pub exe: String,
    /// Runs as the same user as Decka. `false` also when Windows does not say.
    pub mine: bool,
    pub bytes: u64,
}

/// One program with all its processes.
#[derive(Debug, Clone, PartialEq)]
pub struct Heavy {
    /// One of its processes: what the island names when its button is pressed.
    pub pid: u32,
    /// What the program calls itself ("Google Chrome"), else the title of its window, else the name of its file.
    pub name: String,
    /// The name of the file without `.exe`.
    pub stem: String,
    pub exe: String,
    pub mb: u64,
}

fn stem_of(exe: &str) -> String {
    let file = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    match file.get(file.len().saturating_sub(4)..) {
        Some(ext) if ext.eq_ignore_ascii_case(".exe") => file[..file.len() - 4].to_string(),
        _ => file.to_string(),
    }
}

/// Whether Decka may show and close the program of `p`.
pub fn allowed(p: &Proc) -> bool {
    p.mine && !p.exe.is_empty() && !SYSTEM_NAMES.contains(&stem_of(&p.exe).to_lowercase().as_str())
}

/// Decka and everything it started.
fn own(procs: &[Proc], me: u32) -> HashSet<u32> {
    let mut out = HashSet::from([me]);
    loop {
        let before = out.len();
        let children: Vec<u32> = procs.iter().filter(|p| p.parent.is_some_and(|x| out.contains(&x))).map(|p| p.pid).collect();
        out.extend(children);
        if out.len() == before {
            return out;
        }
    }
}

/// The processes of the program `exe` that may be closed.
pub fn processes_of(procs: &[Proc], me: u32, exe: &str) -> Vec<u32> {
    let own = own(procs, me);
    procs.iter().filter(|p| allowed(p) && !own.contains(&p.pid) && p.exe.eq_ignore_ascii_case(exe)).map(|p| p.pid).collect()
}

fn short(title: &str) -> String {
    let t = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= 40 {
        t
    } else {
        format!("{}…", t.chars().take(39).collect::<String>().trim_end())
    }
}

/// The `n` programs of the user that use the most memory; `titles`: the window title by process, `describe`: the
/// description of a program file.
pub fn heaviest(procs: &[Proc], titles: &HashMap<u32, String>, describe: &dyn Fn(&str) -> Option<String>, me: u32, n: usize) -> Vec<Heavy> {
    let own = own(procs, me);
    let mut by_exe: HashMap<String, Vec<&Proc>> = HashMap::new();
    for p in procs.iter().filter(|p| allowed(p) && !own.contains(&p.pid)) {
        by_exe.entry(p.exe.to_lowercase()).or_default().push(p);
    }
    let mut out: Vec<Heavy> = by_exe
        .into_values()
        .map(|mut group| {
            group.sort_by_key(|p| std::cmp::Reverse(p.bytes));
            let stem = stem_of(&group[0].exe);
            let window = group.iter().find_map(|p| titles.get(&p.pid).map(|t| (p.pid, short(t)))).filter(|w| !w.1.is_empty());
            let mut chars = stem.chars();
            let plain = chars.next().map(|c| c.to_uppercase().chain(chars).collect::<String>()).unwrap_or_default();
            let bytes: u64 = group.iter().map(|p| p.bytes).sum();
            Heavy { pid: window.as_ref().map_or(group[0].pid, |w| w.0), name: window.map_or(plain, |w| w.1), stem, exe: group[0].exe.clone(), mb: bytes / (1024 * 1024) }
        })
        .collect();
    out.sort_by(|a, b| b.mb.cmp(&a.mb).then_with(|| a.stem.cmp(&b.stem)));
    out.truncate(n);
    // Reading a file is for the few that are shown only.
    for h in &mut out {
        if let Some(said) = describe(&h.exe).map(|d| short(&d)).filter(|d| !d.is_empty()) {
            h.name = said;
        }
    }
    out
}

/// What closing needs from Windows.
pub trait Ops {
    fn processes(&self) -> Vec<Proc>;
    fn me(&self) -> u32;
    /// `WM_CLOSE` to every window of these processes.
    fn ask_to_close(&self, pids: &[u32]);
    fn kill(&self, pid: u32) -> bool;
    fn sleep(&self, d: Duration);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    Yes,
    /// It is still there after `CLOSE_WAIT`: force needs another "yes".
    Still,
}

/// Closes the program `exe`: asks it to, or ends its processes when `force`. What may be closed is looked at again here.
pub fn close(ops: &dyn Ops, exe: &str, force: bool) -> Result<Closed, String> {
    let all = ops.processes();
    let pids = processes_of(&all, ops.me(), exe);
    if pids.is_empty() {
        let there = all.iter().any(|p| p.exe.eq_ignore_ascii_case(exe));
        return Err(if there { NOT_ALLOWED } else { NOT_RUNNING }.into());
    }
    if force {
        let failed = pids.iter().filter(|p| !ops.kill(**p)).count();
        ops.sleep(STEP);
        // A process may end by itself while the others are ended: only what is still there counts.
        return if failed > 0 && !processes_of(&ops.processes(), ops.me(), exe).is_empty() { Err(NOT_CLOSED.into()) } else { Ok(Closed::Yes) };
    }
    ops.ask_to_close(&pids);
    let mut waited = Duration::ZERO;
    while waited < CLOSE_WAIT {
        ops.sleep(STEP);
        waited += STEP;
        if processes_of(&ops.processes(), ops.me(), exe).is_empty() {
            return Ok(Closed::Yes);
        }
    }
    Ok(Closed::Still)
}

/// The reply that makes the pipeline ask about force.
pub fn force_reply(exe: &str, name: &str) -> String {
    format!("{FORCE_PREFIX}{}", serde_json::json!({ "exe": exe, "name": name }))
}

/// `(exe, name)` out of `force_reply`.
pub fn force_question(reply: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(reply.strip_prefix(FORCE_PREFIX)?).ok()?;
    Some((v.get("exe")?.as_str()?.to_string(), v.get("name")?.as_str()?.to_string()))
}

/// "Chrome — 1830 MB; Code — 900 MB": the answer in words.
pub fn list_text(list: &[Heavy]) -> String {
    list.iter().map(|h| format!("{} — {} MB", h.name, h.mb)).collect::<Vec<_>>().join("; ")
}

#[cfg(windows)]
pub use win::WinOps;

#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use std::time::Duration;

    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    use windows::core::{BOOL, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, GW_OWNER, WM_CLOSE};

    use super::{Ops, Proc};

    /// The visible top-level windows with a title, with their processes.
    fn windows() -> Vec<(HWND, u32, String)> {
        unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // SAFETY: `lparam` is the address of the Vec in `windows`, which lives until EnumWindows returns.
            let list = unsafe { &mut *(lparam.0 as *mut Vec<(HWND, u32, String)>) };
            // SAFETY: plain queries of the handle the system just gave; the buffer outlives the call.
            unsafe {
                if IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).map_or(true, |o| o.0.is_null()) {
                    let mut text = [0u16; 256];
                    let len = GetWindowTextW(hwnd, &mut text);
                    if len > 0 {
                        let mut pid = 0u32;
                        GetWindowThreadProcessId(hwnd, Some(&mut pid));
                        list.push((hwnd, pid, String::from_utf16_lossy(&text[..len as usize])));
                    }
                }
            }
            BOOL(1)
        }
        let mut list: Vec<(HWND, u32, String)> = Vec::new();
        // SAFETY: see `visit`.
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut list as *mut _ as isize)) };
        list
    }

    /// `FileDescription` of the version information of a program file ("Google Chrome").
    pub fn description(exe: &str) -> Option<String> {
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let path = wide(exe);
        // SAFETY: every pointer is into a buffer that outlives the call; the lengths are the ones Windows returned.
        unsafe {
            let size = GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            GetFileVersionInfoW(PCWSTR(path.as_ptr()), None, size, data.as_mut_ptr().cast()).ok()?;
            let query = |key: &str| -> Option<(*const u16, u32)> {
                let key = wide(key);
                let (mut at, mut len) = (std::ptr::null_mut(), 0u32);
                VerQueryValueW(data.as_ptr().cast(), PCWSTR(key.as_ptr()), &mut at, &mut len).as_bool().then_some((at.cast_const().cast::<u16>(), len))
            };
            // The languages the file says it has (pairs of words), then the usual English ones.
            let mut tables: Vec<(u16, u16)> = query("\\VarFileInfo\\Translation")
                .map(|(at, bytes)| (0..bytes as usize / 4).map(|i| (at.add(2 * i).read_unaligned(), at.add(2 * i + 1).read_unaligned())).collect())
                .unwrap_or_default();
            tables.extend([(0x0409, 0x04b0), (0x0409, 0x04e4), (0, 0x04b0)]);
            tables.into_iter().find_map(|(lang, page)| {
                let (at, len) = query(&format!("\\StringFileInfo\\{lang:04x}{page:04x}\\FileDescription"))?;
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(at, len as usize));
                Some(text.trim_matches(['\0', ' ']).to_string()).filter(|t| !t.is_empty())
            })
        }
    }

    pub struct WinOps;

    impl WinOps {
        /// The window title of each process that has one.
        pub fn titles(&self) -> HashMap<u32, String> {
            let mut out = HashMap::new();
            for (_, pid, title) in windows() {
                out.entry(pid).or_insert(title);
            }
            out
        }
    }

    impl Ops for WinOps {
        fn processes(&self) -> Vec<Proc> {
            let Ok(mut sys) = System::new() else { return Vec::new() };
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_memory().with_user(UpdateKind::Always).with_exe(UpdateKind::Always));
            let me = sys.process(sysinfo::Pid::from_u32(std::process::id())).and_then(|p| p.user_id().cloned());
            sys.processes()
                .iter()
                // Threads are listed as processes by some systems.
                .filter(|(_, p)| p.thread_kind().is_none())
                .map(|(pid, p)| Proc {
                    pid: pid.as_u32(),
                    parent: p.parent().map(sysinfo::Pid::as_u32),
                    exe: p.exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default(),
                    mine: me.is_some() && p.user_id() == me.as_ref(),
                    bytes: p.memory(),
                })
                .collect()
        }

        fn me(&self) -> u32 {
            std::process::id()
        }

        fn ask_to_close(&self, pids: &[u32]) {
            for (hwnd, pid, _) in windows() {
                if pids.contains(&pid) {
                    // SAFETY: posting a message to a window handle just listed.
                    let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
                }
            }
        }

        fn kill(&self, pid: u32) -> bool {
            // SAFETY: the handle is closed right after the call that uses it.
            unsafe {
                let Ok(process) = OpenProcess(PROCESS_TERMINATE, false, pid) else { return false };
                let done = TerminateProcess(process, 1).is_ok();
                let _ = CloseHandle(process);
                done
            }
        }

        fn sleep(&self, d: Duration) {
            std::thread::sleep(d);
        }
    }
}

/// The heaviest programs as they are now.
pub fn look() -> Vec<Heavy> {
    #[cfg(windows)]
    {
        let ops = WinOps;
        heaviest(&ops.processes(), &ops.titles(), &win::description, ops.me(), SHOWN)
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    const MB: u64 = 1024 * 1024;

    fn p(pid: u32, exe: &str, mine: bool, mb: u64) -> Proc {
        Proc { pid, parent: None, exe: format!(r"C:\Apps\{exe}"), mine, bytes: mb * MB }
    }

    fn table() -> Vec<Proc> {
        let mut list = vec![
            p(10, "chrome.exe", true, 900),
            p(11, "chrome.exe", true, 600),
            p(20, "Code.exe", true, 800),
            p(30, "game.exe", true, 300),
            p(40, "explorer.exe", true, 5_000),
            p(50, "service.exe", false, 4_000),
            p(60, "decka-agent.exe", true, 400),
            p(61, "helper.exe", true, 700),
            p(70, "msedgewebview2.exe", true, 3_000),
        ];
        list[7].parent = Some(60);
        list
    }

    #[test]
    fn the_heaviest_programs_are_those_of_the_user_without_windows_parts_and_decka() {
        let titles = HashMap::from([(11, "YouTube - Google Chrome".to_string())]);
        let none = |_: &str| None;
        let top = heaviest(&table(), &titles, &none, 60, SHOWN);
        assert_eq!(top.iter().map(|h| (h.stem.as_str(), h.mb)).collect::<Vec<_>>(), [("chrome", 1500), ("Code", 800), ("game", 300)]);
        assert_eq!((top[0].name.as_str(), top[0].pid), ("YouTube - Google Chrome", 11));
        assert_eq!(top[2].name, "Game");
        assert_eq!(heaviest(&table(), &titles, &none, 60, 1).len(), 1);
        // The name the program gives itself wins over the title of its window.
        let described = heaviest(&table(), &titles, &|exe| exe.ends_with("chrome.exe").then(|| "Google Chrome".to_string()), 60, 2);
        assert_eq!(described.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(), ["Google Chrome", "Code"]);
        assert_eq!(list_text(&top[1..]), "Code — 800 MB; Game — 300 MB");
    }

    #[test]
    fn nothing_of_the_system_of_others_or_of_decka_is_closed() {
        for exe in [r"C:\Apps\explorer.exe", r"C:\Apps\service.exe", r"C:\Apps\decka-agent.exe", r"C:\Apps\helper.exe", r"C:\Apps\msedgewebview2.exe"] {
            let ops = Fake::new(table());
            assert_eq!(close(&ops, exe, false), Err(NOT_ALLOWED.into()), "{exe}");
            assert_eq!(close(&ops, exe, true), Err(NOT_ALLOWED.into()), "{exe}");
            assert!(ops.asked.borrow().is_empty() && ops.killed.borrow().is_empty());
        }
        assert_eq!(close(&Fake::new(table()), r"C:\Apps\gone.exe", false), Err(NOT_RUNNING.into()));
    }

    struct Fake {
        list: RefCell<Vec<Proc>>,
        /// A program that closes by itself after this many looks.
        leaves_after: Cell<Option<u32>>,
        asked: RefCell<Vec<u32>>,
        killed: RefCell<Vec<u32>>,
        slept: Cell<Duration>,
    }

    impl Fake {
        fn new(list: Vec<Proc>) -> Fake {
            Fake { list: RefCell::new(list), leaves_after: Cell::new(None), asked: RefCell::default(), killed: RefCell::default(), slept: Cell::new(Duration::ZERO) }
        }
    }

    impl Ops for Fake {
        fn processes(&self) -> Vec<Proc> {
            self.list.borrow().clone()
        }
        fn me(&self) -> u32 {
            60
        }
        fn ask_to_close(&self, pids: &[u32]) {
            self.asked.borrow_mut().extend(pids);
        }
        fn kill(&self, pid: u32) -> bool {
            self.killed.borrow_mut().push(pid);
            self.list.borrow_mut().retain(|p| p.pid != pid);
            true
        }
        fn sleep(&self, d: Duration) {
            self.slept.set(self.slept.get() + d);
            if let Some(n) = self.leaves_after.get() {
                if n == 0 {
                    self.list.borrow_mut().retain(|p| !self.asked.borrow().contains(&p.pid));
                } else {
                    self.leaves_after.set(Some(n - 1));
                }
            }
        }
    }

    #[test]
    fn a_program_is_asked_first_and_force_is_only_for_one_that_stays() {
        let exe = r"C:\Apps\chrome.exe";
        let polite = Fake::new(table());
        polite.leaves_after.set(Some(3));
        assert_eq!(close(&polite, exe, false), Ok(Closed::Yes));
        assert_eq!(*polite.asked.borrow(), [10, 11]);
        assert!(polite.killed.borrow().is_empty() && polite.slept.get() < CLOSE_WAIT);

        let stubborn = Fake::new(table());
        assert_eq!(close(&stubborn, exe, false), Ok(Closed::Still));
        assert_eq!(stubborn.slept.get(), CLOSE_WAIT);
        assert!(stubborn.killed.borrow().is_empty(), "never by force without the second yes");
        assert_eq!(close(&stubborn, exe, true), Ok(Closed::Yes));
        assert_eq!(*stubborn.killed.borrow(), [10, 11]);
    }

    /// A look at the real PC: `cargo test -p decka-core --lib heavy -- --ignored --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn the_list_of_this_pc() {
        let list = look();
        for h in &list {
            println!("{} | {} | {} MB", h.name, h.stem, h.mb);
        }
        assert!(!list.is_empty() && list.iter().all(|h| !SYSTEM_NAMES.contains(&h.stem.to_lowercase().as_str())));
    }

    #[test]
    fn the_force_question_round_trips() {
        let r = force_reply(r"C:\Apps\game.exe", "Game \"2\"");
        assert_eq!(force_question(&r), Some((r"C:\Apps\game.exe".into(), "Game \"2\"".into())));
        assert_eq!(force_question("Closed Game"), None);
    }
}
