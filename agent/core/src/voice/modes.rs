//! Work modes (T27.10): `%APPDATA%/Decka/modes.json` is a list of named layouts - which programs, where on the screen. "Remember the
//! layout as work" saves the windows that are open now; "work mode" opens the missing programs and puts every window in its place.
//! Only the programs written in the saved mode are ever started, and the file is the owner's: nothing a phone or a phrase says
//! becomes a command.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::matcher::{tokens, trigram_sim};
use super::ui_snapshot::Rect;

pub const MAX_APPS: usize = 12;
const FILE: &str = "modes.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeApp {
    /// The full path of the program.
    pub exe: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    pub name: String,
    pub apps: Vec<ModeApp>,
    #[serde(rename = "focusMinutes", default, skip_serializing_if = "Option::is_none")]
    pub focus_minutes: Option<u32>,
}

impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Rect, D::Error> {
        #[derive(Deserialize)]
        struct R {
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        }
        let r = R::deserialize(d)?;
        Ok(Rect { x: r.x, y: r.y, width: r.width, height: r.height })
    }
}

pub fn load(dir: &Path) -> Vec<Mode> {
    std::fs::read(dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn write(dir: &Path, modes: &[Mode]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(FILE), serde_json::to_vec_pretty(modes).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// The mode of this name (case and word forms do not matter much).
pub fn find<'a>(modes: &'a [Mode], name: &str) -> Option<&'a Mode> {
    let q = tokens(name).join(" ");
    modes
        .iter()
        .map(|m| (trigram_sim(&q, &tokens(&m.name).join(" ")), m))
        .filter(|s| s.0 >= 0.6)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|s| s.1)
}

/// The longest focus a mode may ask for.
pub const MAX_FOCUS_MINUTES: u32 = 180;
/// The end of a focus is still worth telling this long after it (a full screen program holds the note back).
const FOCUS_TOLD_WITHIN: Duration = Duration::from_secs(600);

/// `focusMinutes` of the mode of this name; `None` when it has none or an impossible one.
pub fn focus_of(modes: &[Mode], name: &str) -> Option<u32> {
    find(modes, name)?.focus_minutes.filter(|m| (1..=MAX_FOCUS_MINUTES).contains(m))
}

/// T30.4: the focus timer a work mode starts. There is one: a new one replaces it.
#[derive(Debug, Default)]
pub struct FocusTimer {
    generation: AtomicU64,
    ended: Mutex<Option<Instant>>,
}

impl FocusTimer {
    /// A new focus begins; the number names it.
    pub fn arm(&self) -> u64 {
        *self.ended.lock().unwrap() = None;
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The time of focus `generation` is up. `false`: a later one replaced it.
    pub fn finish(&self, generation: u64, now: Instant) -> bool {
        if self.generation.load(Ordering::SeqCst) != generation {
            return false;
        }
        *self.ended.lock().unwrap() = Some(now);
        true
    }

    /// An end that the island has not told yet.
    pub fn ended(&self, now: Instant) -> bool {
        self.ended.lock().unwrap().is_some_and(|t| now.saturating_duration_since(t) < FOCUS_TOLD_WITHIN)
    }

    pub fn told(&self) {
        *self.ended.lock().unwrap() = None;
    }
}

/// The notification at the end of a focus, in the language of Decka: the phone shows it as it is.
pub fn focus_over(lang: &str) -> &'static str {
    match lang {
        "ru" => "Фокус закончен",
        "es" => "Se acabó el tiempo de concentración",
        "de" => "Die Fokuszeit ist um",
        "fr" => "La séance de concentration est terminée",
        "pt" => "O tempo de foco acabou",
        "zh" => "专注时间结束",
        "hi" => "फ़ोकस का समय पूरा हुआ",
        "ar" => "انتهى وقت التركيز",
        "ja" => "集中タイムが終わりました",
        "kk" => "Зейін уақыты аяқталды",
        _ => "The focus timer is over",
    }
}

/// What running a mode does with one program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Not open: start it, then place it.
    Launch(ModeApp),
    /// Open: only place it.
    Place(ModeApp),
}

/// The steps for a mode given the programs that have a window now. Only the programs of the mode appear in them.
pub fn plan(mode: &Mode, running: &[String]) -> Vec<Step> {
    mode.apps
        .iter()
        .take(MAX_APPS)
        .map(|a| if running.iter().any(|r| r.eq_ignore_ascii_case(&a.exe)) { Step::Place(a.clone()) } else { Step::Launch(a.clone()) })
        .collect()
}

/// A program may be in a mode only when it is a full path to a file that exists.
pub fn launchable(exe: &str) -> bool {
    let p = Path::new(exe);
    p.is_absolute() && p.is_file()
}

/// Replaces (or adds) a mode edited in the settings. The programs must be full paths; a mode has 1..12 of them.
pub fn put(dir: &Path, mode: Mode, original_name: Option<&str>) -> Result<(), String> {
    if mode.name.trim().is_empty() {
        return Err("The mode needs a name".into());
    }
    if mode.apps.is_empty() || mode.apps.len() > MAX_APPS {
        return Err("A mode has 1 to 12 programs".into());
    }
    if mode.apps.iter().any(|a| !Path::new(&a.exe).is_absolute()) {
        return Err("A program must be a full path".into());
    }
    let mut modes = load(dir);
    let key = original_name.unwrap_or(&mode.name).to_lowercase();
    modes.retain(|m| m.name.to_lowercase() != key && m.name.to_lowercase() != mode.name.to_lowercase());
    modes.push(mode);
    write(dir, &modes)
}

pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    let mut modes = load(dir);
    let before = modes.len();
    modes.retain(|m| !m.name.eq_ignore_ascii_case(name));
    if modes.len() == before {
        return Err("No such mode".into());
    }
    write(dir, &modes)
}

/// Saves the windows that are open now as the mode `name` (the same name replaces the older mode).
pub fn save(dir: &Path, name: &str) -> Result<String, String> {
    save_with(dir, name, current_layout())
}

pub fn save_with(dir: &Path, name: &str, apps: Vec<ModeApp>) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("The mode needs a name".into());
    }
    if apps.is_empty() {
        return Err("No program windows to remember".into());
    }
    let mut modes = load(dir);
    let mode = Mode { name: name.to_string(), apps: apps.into_iter().take(MAX_APPS).collect(), focus_minutes: None };
    match modes.iter().position(|m| m.name.eq_ignore_ascii_case(name)) {
        Some(i) => modes[i] = mode,
        None => modes.push(mode),
    }
    write(dir, &modes)?;
    Ok(format!("Remembered the layout as {name}"))
}

/// Opens what is missing and puts the windows in place.
pub fn run(dir: &Path, name: &str) -> Result<String, String> {
    let modes = load(dir);
    let mode = find(&modes, name).ok_or_else(|| format!("No mode called {name}"))?.clone();
    let steps = plan(&mode, &running_programs());
    let mut launched = 0;
    for s in &steps {
        if let Step::Launch(a) = s {
            if !launchable(&a.exe) {
                continue;
            }
            if std::process::Command::new(&a.exe).spawn().is_ok() {
                launched += 1;
            }
        }
    }
    if launched > 0 {
        // The new windows need a moment to appear.
        std::thread::sleep(std::time::Duration::from_millis(2_500));
    }
    for s in &steps {
        let (Step::Launch(a) | Step::Place(a)) = s;
        if let Some(r) = a.rect {
            place(&a.exe, r);
        }
    }
    Ok(format!("Mode {}: {} program(s) started", mode.name, launched))
}

#[cfg(windows)]
fn windows_of_programs() -> Vec<(isize, String, Rect)> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };

    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the pointer to the vector of this call; the Win32 calls take handles of this desktop.
        unsafe {
            let out = &mut *(lparam.0 as *mut Vec<(isize, String, Rect)>);
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() || GetWindowTextLengthW(hwnd) == 0 || (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0 {
                return BOOL(1);
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return BOOL(1) };
            let mut buf = [0u16; 520];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_FORMAT(0), windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            let mut r = RECT::default();
            if ok && GetWindowRect(hwnd, &mut r).is_ok() && r.right > r.left && r.bottom > r.top {
                out.push((hwnd.0 as isize, String::from_utf16_lossy(&buf[..len as usize]), Rect { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top }));
            }
            BOOL(1)
        }
    }

    let mut out: Vec<(isize, String, Rect)> = Vec::new();
    // SAFETY: the callback runs inside this call only.
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

#[cfg(not(windows))]
fn windows_of_programs() -> Vec<(isize, String, Rect)> {
    Vec::new()
}

fn running_programs() -> Vec<String> {
    windows_of_programs().into_iter().map(|w| w.1).collect()
}

/// The program windows open now: one entry per program (its largest window), without Decka itself and the shell.
pub fn current_layout() -> Vec<ModeApp> {
    let me = std::env::current_exe().ok().map(|p| p.to_string_lossy().to_lowercase());
    let mut out: Vec<ModeApp> = Vec::new();
    for (_, exe, rect) in windows_of_programs() {
        let lower = exe.to_lowercase();
        if Some(&lower) == me.as_ref() || lower.ends_with("\\explorer.exe") || lower.ends_with("\\applicationframehost.exe") || lower.ends_with("\\textinputhost.exe") {
            continue;
        }
        match out.iter_mut().find(|a| a.exe.eq_ignore_ascii_case(&exe)) {
            Some(a) => {
                if a.rect.is_some_and(|r| r.width * r.height < rect.width * rect.height) {
                    a.rect = Some(rect);
                }
            }
            None => out.push(ModeApp { exe, monitor: None, rect: Some(rect) }),
        }
    }
    out
}

/// Puts the window of the program in `rect`.
#[cfg(windows)]
fn place(exe: &str, rect: Rect) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};
    if let Some((h, _, _)) = windows_of_programs().into_iter().filter(|w| w.1.eq_ignore_ascii_case(exe)).max_by_key(|w| w.2.width * w.2.height) {
        // SAFETY: a window of this desktop found a moment ago; the call only moves it.
        unsafe {
            let _ = SetWindowPos(HWND(h as *mut _), None, rect.x, rect.y, rect.width, rect.height, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

#[cfg(not(windows))]
fn place(_: &str, _: Rect) {}

#[cfg(test)]
mod tests {
    /// T30.4: only the latest focus ends, and its end is told once.
    #[test]
    fn a_mode_with_focus_minutes_has_one_timer_whose_end_is_told_once() {
        let modes = [Mode { name: "work".into(), apps: vec![app("C:\\a.exe")], focus_minutes: Some(25) }, Mode { name: "game".into(), apps: vec![app("C:\\g.exe")], focus_minutes: Some(500) }];
        assert_eq!((focus_of(&modes, "Work"), focus_of(&modes, "game"), focus_of(&modes, "sleep")), (Some(25), None, None));
        let timer = FocusTimer::default();
        let now = Instant::now();
        let (first, second) = (timer.arm(), timer.arm());
        assert!(!timer.finish(first, now) && !timer.ended(now));
        assert!(timer.finish(second, now) && timer.ended(now));
        assert!(!timer.ended(now + FOCUS_TOLD_WITHIN));
        timer.told();
        assert!(!timer.ended(now));
        assert_eq!((focus_over("ru"), focus_over("xx")), ("Фокус закончен", "The focus timer is over"));
    }

    #[test]
    fn a_mode_edited_in_the_settings_is_checked_replaced_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let mode = |name: &str, exe: &str| Mode { name: name.into(), apps: vec![app(exe)], focus_minutes: Some(25) };
        put(dir.path(), mode("work", "C:\\a.exe"), None).unwrap();
        put(dir.path(), mode("study", "C:\\b.exe"), Some("work")).unwrap();
        assert_eq!(load(dir.path()).iter().map(|m| m.name.clone()).collect::<Vec<_>>(), vec!["study".to_string()], "renamed");
        assert!(put(dir.path(), mode("x", "a.exe"), None).is_err(), "not a full path");
        assert!(put(dir.path(), Mode { name: "y".into(), apps: vec![], focus_minutes: None }, None).is_err());
        assert!(delete(dir.path(), "STUDY").is_ok());
        assert!(delete(dir.path(), "study").is_err());
    }

    use super::*;

    fn app(exe: &str) -> ModeApp {
        ModeApp { exe: exe.into(), monitor: None, rect: Some(Rect { x: 0, y: 0, width: 800, height: 600 }) }
    }

    #[test]
    fn a_mode_is_saved_replaced_by_name_and_found_again() {
        let dir = tempfile::tempdir().unwrap();
        save_with(dir.path(), "Work", vec![app("C:\\a.exe"), app("C:\\b.exe")]).unwrap();
        save_with(dir.path(), "work", vec![app("C:\\c.exe")]).unwrap();
        let modes = load(dir.path());
        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].apps[0].exe, "C:\\c.exe");
        assert_eq!(find(&modes, "WORK").unwrap().name, "work");
        assert!(find(&modes, "gaming").is_none());
        assert!(save_with(dir.path(), "  ", vec![app("C:\\a.exe")]).is_err());
        assert!(save_with(dir.path(), "x", vec![]).is_err());
    }

    #[test]
    fn running_a_mode_launches_only_what_is_missing_and_only_what_it_lists() {
        let mode = Mode { name: "work".into(), apps: vec![app("C:\\a.exe"), app("C:\\b.exe")], focus_minutes: Some(25) };
        let steps = plan(&mode, &["c:\\A.EXE".to_string(), "C:\\other.exe".to_string()]);
        assert_eq!(steps, vec![Step::Place(app("C:\\a.exe")), Step::Launch(app("C:\\b.exe"))]);
        assert!(steps.iter().all(|s| matches!(s, Step::Launch(a) | Step::Place(a) if a.exe.starts_with("C:\\a") || a.exe.starts_with("C:\\b"))), "nothing outside the mode");
    }

    #[test]
    fn a_program_is_launchable_only_as_an_existing_full_path() {
        assert!(!launchable("notepad.exe"), "a bare name could be anything on the path");
        assert!(!launchable("C:\\no\\such\\file.exe"));
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.exe");
        std::fs::write(&f, b"").unwrap();
        assert!(launchable(f.to_str().unwrap()));
    }

    #[test]
    fn a_mode_has_at_most_twelve_programs() {
        let dir = tempfile::tempdir().unwrap();
        let many: Vec<ModeApp> = (0..20).map(|i| app(&format!("C:\\p{i}.exe"))).collect();
        save_with(dir.path(), "big", many).unwrap();
        assert_eq!(load(dir.path())[0].apps.len(), MAX_APPS);
    }
}
