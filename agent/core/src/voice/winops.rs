//! The Windows side of the voice tools that have no action of their own (DATA_MODEL.md 12.5): switching to and closing
//! a program, moving the active window to the other monitor, telling which monitor it is on, typing text.
//! The decisions (which monitor is next, where the window goes, which windows belong to a program, the text of a timer)
//! are pure functions with tests; the `win` module is the thin layer over the Win32 calls.

/// A rectangle in screen coordinates (`right` and `bottom` are exclusive, as in Win32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Where the monitors are, in a stable order: left to right, then top to bottom. "Monitor 1" is the left-most one.
pub fn order_monitors(mut areas: Vec<Rect>) -> Vec<Rect> {
    areas.sort_by_key(|r| (r.left, r.top));
    areas
}

/// The monitor after `current`, wrapping around; with one monitor there is nowhere to go.
pub fn next_monitor(current: usize, count: usize) -> Option<usize> {
    (count > 1).then(|| (current + 1) % count)
}

/// The answer to "which monitor": 1-based.
pub fn monitor_text(index: usize, count: usize) -> String {
    format!("Monitor {} of {}", index + 1, count)
}

/// `win` moved from the work area `from` to the work area `to`: the same relative position and the same size, shrunk when
/// the new area is smaller, and always fully inside it.
pub fn move_window(win: Rect, from: Rect, to: Rect) -> Rect {
    let (w, h) = (win.width().min(to.width()).max(1), win.height().min(to.height()).max(1));
    // Relative offset of the window inside its monitor, scaled to the size of the target.
    let scale = |off: i64, from_len: i32, to_len: i32| -> i32 { if from_len <= 0 { 0 } else { (off * to_len as i64 / from_len as i64) as i32 } };
    let x = to.left + scale((win.left - from.left) as i64, from.width(), to.width());
    let y = to.top + scale((win.top - from.top) as i64, from.height(), to.height());
    let x = x.clamp(to.left, (to.right - w).max(to.left));
    let y = y.clamp(to.top, (to.bottom - h).max(to.top));
    Rect { left: x, top: y, right: x + w, bottom: y + h }
}

/// The index of the monitor that holds most of `win` (the one holding its centre).
pub fn monitor_of(win: Rect, areas: &[Rect]) -> usize {
    let (cx, cy) = ((win.left + win.right) / 2, (win.top + win.bottom) / 2);
    areas
        .iter()
        .position(|a| cx >= a.left && cx < a.right && cy >= a.top && cy < a.bottom)
        .unwrap_or_else(|| {
            // Off every screen: the nearest by the distance of the centres.
            areas
                .iter()
                .enumerate()
                .min_by_key(|(_, a)| {
                    let (ax, ay) = ((a.left + a.right) / 2, (a.top + a.bottom) / 2);
                    ((cx - ax) as i64).pow(2) + ((cy - ay) as i64).pow(2)
                })
                .map_or(0, |(i, _)| i)
        })
}

/// Whether the exe of a window is the program of the installed-programs list: the same file name, any case.
pub fn same_program(window_exe: &str, program: &str) -> bool {
    let name = |p: &str| p.rsplit(['\\', '/']).next().unwrap_or(p).to_ascii_lowercase();
    let (a, b) = (name(window_exe), name(program));
    !a.is_empty() && a == b
}

/// At most this many timers run at once: a model or a speaker cannot pile them up without limit.
pub const MAX_TIMERS: usize = 10;

/// The notification of a finished timer: `(title, body)`.
pub fn timer_notice(minutes: u32, label: Option<&str>) -> (String, String) {
    let body = match label.map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => l.to_string(),
        None => format!("{minutes} min"),
    };
    ("Timer".to_string(), body)
}

/// The reply of a timer that was set.
pub fn timer_set_text(minutes: u32) -> String {
    format!("Timer set for {minutes} min")
}

/// The reply of a finished volume change.
pub fn volume_text(percent: u8) -> String {
    format!("Volume {percent}%")
}

#[cfg(windows)]
pub use win::{close_app, move_to_other_monitor, switch_app, which_monitor};

/// Types `text` into the field that has the focus, as Unicode characters (not key chords).
#[cfg(windows)]
pub fn type_text(text: &str) -> Result<(), String> {
    use enigo::{Enigo, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("keyboard unavailable: {e}"))?;
    enigo.text(text).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
mod other {
    const NO: &str = "unsupported platform";
    pub fn switch_app(_: &str, _: &str) -> Result<(), String> {
        Err(NO.into())
    }
    pub fn close_app(_: &str, _: &str) -> Result<(), String> {
        Err(NO.into())
    }
    pub fn move_to_other_monitor() -> Result<String, String> {
        Err(NO.into())
    }
    pub fn which_monitor() -> Result<String, String> {
        Err(NO.into())
    }
    pub fn type_text(_: &str) -> Result<(), String> {
        Err(NO.into())
    }
}
#[cfg(not(windows))]
pub use other::{close_app, move_to_other_monitor, switch_app, type_text, which_monitor};

#[cfg(windows)]
mod win {
    use windows::core::{BOOL, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, PostMessageW,
        SetForegroundWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOZORDER, SW_MAXIMIZE, SW_RESTORE, WM_CLOSE,
    };

    use super::{monitor_of, monitor_text, move_window, next_monitor, order_monitors, same_program, Rect};

    fn rect(r: RECT) -> Rect {
        Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }

    /// Full path of the exe of a process.
    fn exe_path(pid: u32) -> Option<String> {
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
            Some(String::from_utf16_lossy(&buffer[..len as usize]))
        }
    }

    /// Visible top-level windows with a title: the windows of the taskbar, more or less.
    fn app_windows() -> Vec<HWND> {
        unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // SAFETY: `lparam` is the address of the Vec in `app_windows`, which lives until EnumWindows returns.
            let list = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
            if unsafe { IsWindowVisible(hwnd) }.as_bool() && unsafe { GetWindowTextLengthW(hwnd) } > 0 {
                list.push(hwnd);
            }
            BOOL(1)
        }
        let mut list: Vec<HWND> = Vec::new();
        // SAFETY: see `visit`.
        let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::EnumWindows(Some(visit), LPARAM(&mut list as *mut Vec<HWND> as isize)) };
        list
    }

    fn windows_of(program: &str) -> Vec<HWND> {
        app_windows()
            .into_iter()
            .filter(|h| {
                let mut pid = 0u32;
                // SAFETY: plain query of a window handle.
                unsafe { GetWindowThreadProcessId(*h, Some(&mut pid)) };
                exe_path(pid).is_some_and(|p| same_program(&p, program))
            })
            .collect()
    }

    /// Brings the first window of `program` to the front (restoring it when minimised).
    pub fn switch_app(program: &str, name: &str) -> Result<(), String> {
        let Some(hwnd) = windows_of(program).into_iter().next() else {
            return Err(format!("{name} is not running"));
        };
        // SAFETY: Win32 calls on a handle just listed; the input queues are detached again before returning.
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            // Windows lets only the foreground thread give the focus away: join its input queue for a moment.
            let (fore, me) = (GetWindowThreadProcessId(GetForegroundWindow(), None), GetCurrentThreadId());
            let attached = fore != 0 && fore != me && AttachThreadInput(me, fore, true).as_bool();
            let ok = SetForegroundWindow(hwnd).as_bool();
            if attached {
                let _ = AttachThreadInput(me, fore, false);
            }
            if ok { Ok(()) } else { Err(format!("Windows did not let Decka switch to {name}")) }
        }
    }

    /// Asks every window of `program` to close (`WM_CLOSE`): the program itself asks about unsaved work.
    pub fn close_app(program: &str, name: &str) -> Result<(), String> {
        let windows = windows_of(program);
        if windows.is_empty() {
            return Err(format!("{name} is not running"));
        }
        for hwnd in windows {
            // SAFETY: posting a message to a window handle just listed.
            let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
        Ok(())
    }

    /// Work areas of all monitors in the stable order of `order_monitors`, each with its handle.
    fn monitors() -> Vec<(HMONITOR, Rect)> {
        unsafe extern "system" fn visit(monitor: HMONITOR, _: HDC, _: *mut RECT, lparam: LPARAM) -> BOOL {
            // SAFETY: `lparam` is the address of the Vec in `monitors`, which lives until EnumDisplayMonitors returns.
            let list = unsafe { &mut *(lparam.0 as *mut Vec<(HMONITOR, RECT)>) };
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
                list.push((monitor, info.rcWork));
            }
            BOOL(1)
        }
        let mut list: Vec<(HMONITOR, RECT)> = Vec::new();
        // SAFETY: see `visit`.
        let _ = unsafe { EnumDisplayMonitors(None, None, Some(visit), LPARAM(&mut list as *mut Vec<(HMONITOR, RECT)> as isize)) };
        let areas = order_monitors(list.iter().map(|(_, r)| rect(*r)).collect());
        // The same order for the handles: look each area up.
        areas.into_iter().filter_map(|a| list.iter().find(|(_, r)| rect(*r) == a).map(|(h, _)| (*h, a))).collect()
    }

    /// The active window, its rectangle and the index of its monitor.
    fn active() -> Result<(HWND, Rect, Vec<Rect>, usize), String> {
        // SAFETY: plain queries of the foreground window.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return Err("There is no active window".into());
            }
            let mut r = RECT::default();
            GetWindowRect(hwnd, &mut r).map_err(|e| e.message())?;
            let areas: Vec<Rect> = monitors().into_iter().map(|m| m.1).collect();
            // The monitor Windows says the window is on; the centre test is the fallback for windows halfway between.
            let on = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let index = monitors().iter().position(|(h, _)| *h == on).unwrap_or_else(|| monitor_of(rect(r), &areas));
            Ok((hwnd, rect(r), areas, index))
        }
    }

    pub fn which_monitor() -> Result<String, String> {
        let (_, _, areas, index) = active()?;
        Ok(monitor_text(index, areas.len()))
    }

    /// Moves the active window to the next monitor, keeping its relative position and whether it is maximised.
    pub fn move_to_other_monitor() -> Result<String, String> {
        let (hwnd, win, areas, index) = active()?;
        let Some(next) = next_monitor(index, areas.len()) else {
            return Err("There is only one monitor".into());
        };
        // SAFETY: Win32 calls on the foreground window handle.
        unsafe {
            let maximised = IsZoomed(hwnd).as_bool();
            if maximised {
                // A maximised window cannot be moved: restore it, move it, maximise it on the other screen.
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let from = areas[index];
            let current = if maximised {
                let mut r = RECT::default();
                GetWindowRect(hwnd, &mut r).map_err(|e| e.message())?;
                rect(r)
            } else {
                win
            };
            let target = move_window(current, from, areas[next]);
            SetWindowPos(hwnd, None, target.left, target.top, target.width(), target.height(), SWP_NOZORDER | SWP_NOACTIVATE).map_err(|e| e.message())?;
            if maximised {
                let _ = ShowWindow(hwnd, SW_MAXIMIZE);
            }
        }
        Ok(monitor_text(next, areas.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect { left, top, right, bottom }
    }

    #[test]
    fn monitors_are_numbered_left_to_right() {
        let sorted = order_monitors(vec![r(1920, 0, 3840, 1040), r(0, 0, 1920, 1040), r(-1280, 100, 0, 1000)]);
        assert_eq!(sorted.iter().map(|a| a.left).collect::<Vec<_>>(), [-1280, 0, 1920]);
    }

    #[test]
    fn the_next_monitor_wraps_and_a_single_screen_has_none() {
        assert_eq!(next_monitor(0, 2), Some(1));
        assert_eq!(next_monitor(1, 2), Some(0));
        assert_eq!(next_monitor(2, 3), Some(0));
        assert_eq!(next_monitor(0, 1), None);
        assert_eq!(next_monitor(0, 0), None);
    }

    #[test]
    fn the_monitor_text_is_one_based() {
        assert_eq!(monitor_text(1, 2), "Monitor 2 of 2");
        assert_eq!(monitor_text(0, 3), "Monitor 1 of 3");
    }

    #[test]
    fn a_window_keeps_its_relative_place_on_the_other_monitor() {
        let (a, b) = (r(0, 0, 1920, 1080), r(1920, 0, 3840, 1080));
        // A quarter of the way across, a tenth down.
        let moved = move_window(r(480, 108, 1080, 708), a, b);
        assert_eq!(moved, r(2400, 108, 3000, 708));
        // And back.
        assert_eq!(move_window(moved, b, a), r(480, 108, 1080, 708));
    }

    #[test]
    fn a_window_is_scaled_to_a_smaller_monitor_and_stays_inside() {
        let (big, small) = (r(0, 0, 3840, 2160), r(3840, 0, 5120, 720));
        let moved = move_window(r(100, 100, 3700, 2000), big, small);
        assert!(moved.left >= small.left && moved.right <= small.right && moved.top >= small.top && moved.bottom <= small.bottom, "{moved:?}");
        assert_eq!((moved.width(), moved.height()), (1280, 720), "shrunk to the new area");
    }

    #[test]
    fn a_window_off_screen_is_pulled_in() {
        let (a, b) = (r(0, 0, 1000, 1000), r(1000, 0, 2000, 1000));
        let moved = move_window(r(900, 900, 1500, 1500), a, b);
        assert!(moved.right <= b.right && moved.bottom <= b.bottom, "{moved:?}");
    }

    #[test]
    fn the_monitor_of_a_window_is_the_one_with_its_centre() {
        let areas = [r(0, 0, 1920, 1080), r(1920, 0, 3840, 1080)];
        assert_eq!(monitor_of(r(100, 100, 700, 600), &areas), 0);
        assert_eq!(monitor_of(r(1800, 100, 2600, 600), &areas), 1, "most of it is on the second");
        assert_eq!(monitor_of(r(5000, 100, 5600, 600), &areas), 1, "off every screen: the nearest");
    }

    #[test]
    fn a_window_belongs_to_a_program_by_file_name() {
        assert!(same_program("C:\\Windows\\System32\\calc.exe", "C:\\Windows\\calc.exe"));
        assert!(same_program("C:\\A\\Chrome.EXE", "d:\\b\\chrome.exe"));
        assert!(!same_program("C:\\A\\chrome.exe", "C:\\A\\chromium.exe"));
        assert!(!same_program("", ""));
    }

    #[test]
    fn the_timer_notice_names_the_label_or_the_minutes() {
        assert_eq!(timer_notice(5, Some("pasta")), ("Timer".to_string(), "pasta".to_string()));
        assert_eq!(timer_notice(5, Some("  ")).1, "5 min");
        assert_eq!(timer_notice(25, None).1, "25 min");
        assert_eq!(timer_set_text(5), "Timer set for 5 min");
        assert_eq!(volume_text(30), "Volume 30%");
    }
}
