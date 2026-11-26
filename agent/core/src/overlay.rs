//! Windows that never take the focus (T29.1): the island, the highlight and the orb of a dictation. The user keeps typing
//! where they were, so showing such a window, clicking it and letting clicks through it must not activate it.
//!
//! The window library cannot be used for that: after a window was created with `focused(false)` it forgets the wish, and every
//! change of a style (show, click-through) goes through `ShowWindow(SW_SHOW)`, which activates the window. These calls talk to
//! Win32 directly instead; once a window is guarded, the application must not call `show` / `set_ignore_cursor_events` of the
//! library on it.

#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWLP_WNDPROC, GWL_EXSTYLE, HWND_TOPMOST,
        MA_NOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE,
        WM_MOUSEACTIVATE, WM_NCDESTROY, WNDPROC, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
    };

    /// The window procedure each guarded window had before.
    fn previous() -> &'static Mutex<HashMap<isize, isize>> {
        static PREVIOUS: OnceLock<Mutex<HashMap<isize, isize>>> = OnceLock::new();
        PREVIOUS.get_or_init(Default::default)
    }

    fn handle(hwnd: isize) -> HWND {
        HWND(hwnd as *mut _)
    }

    unsafe extern "system" fn no_activate(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // A click (also on a child window of the web view, which passes the question up) never activates the window.
        if message == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        let key = hwnd.0 as isize;
        let old = previous().lock().ok().and_then(|m| m.get(&key).copied());
        if message == WM_NCDESTROY {
            if let Ok(mut m) = previous().lock() {
                m.remove(&key);
            }
        }
        match old {
            // SAFETY: `old` is the procedure this window had when it was guarded.
            Some(old) => unsafe { CallWindowProcW(std::mem::transmute::<isize, WNDPROC>(old), hwnd, message, wparam, lparam) },
            // SAFETY: the default procedure accepts any message.
            None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    pub fn guard(hwnd: isize) {
        let window = handle(hwnd);
        // SAFETY: plain calls on a window of this process, made on the thread that owns it.
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
            SetWindowLongPtrW(window, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
            let Ok(mut map) = previous().lock() else { return };
            if map.contains_key(&hwnd) {
                return;
            }
            let procedure = no_activate as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;
            let old = GetWindowLongPtrW(window, GWLP_WNDPROC);
            map.insert(hwnd, old);
            drop(map);
            SetWindowLongPtrW(window, GWLP_WNDPROC, procedure as usize as isize);
        }
    }

    pub fn show(hwnd: isize) {
        // SAFETY: shows the window and puts it on top of the others without activating it.
        unsafe {
            let _ = ShowWindow(handle(hwnd), SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(handle(hwnd), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE);
        }
    }

    pub fn hide(hwnd: isize) {
        // SAFETY: hiding a window of this process.
        let _ = unsafe { ShowWindow(handle(hwnd), SW_HIDE) };
    }

    pub fn set_click_through(hwnd: isize, on: bool) {
        let window = handle(hwnd);
        let flags = (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as isize;
        // SAFETY: changes two style bits of a window of this process and refreshes its frame without activating it.
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
            let wanted = if on { style | flags } else { style & !flags } | WS_EX_NOACTIVATE.0 as isize;
            if wanted != style {
                SetWindowLongPtrW(window, GWL_EXSTYLE, wanted);
                let _ = SetWindowPos(window, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
            }
        }
    }
}

/// Makes the window one that never becomes active. Call it once, on the thread that owns the window.
pub fn guard(hwnd: isize) {
    #[cfg(windows)]
    win::guard(hwnd);
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Shows the window on top of the others without taking the focus.
pub fn show(hwnd: isize) {
    #[cfg(windows)]
    win::show(hwnd);
    #[cfg(not(windows))]
    let _ = hwnd;
}

pub fn hide(hwnd: isize) {
    #[cfg(windows)]
    win::hide(hwnd);
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Whether mouse clicks fall through the window to what is under it.
pub fn set_click_through(hwnd: isize, on: bool) {
    #[cfg(windows)]
    win::set_click_through(hwnd, on);
    #[cfg(not(windows))]
    let _ = (hwnd, on);
}

#[cfg(all(test, windows))]
mod tests {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, GetForegroundWindow, GetWindowLongPtrW, IsWindowVisible, PeekMessageW, SendMessageW, GWL_EXSTYLE,
        MA_NOACTIVATE, MSG, PM_REMOVE, WM_MOUSEACTIVATE, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
    };

    fn pump() {
        for _ in 0..20 {
            let mut msg = MSG::default();
            // SAFETY: the queue of this thread.
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                // SAFETY: a message just taken from the queue.
                unsafe { DispatchMessageW(&msg) };
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// T29.1: showing the island, and switching its click-through, leaves the active window alone.
    #[test]
    fn showing_a_guarded_window_does_not_change_the_active_window() {
        // SAFETY: a small popup of a system class, owned by this thread and destroyed below.
        let window = unsafe { CreateWindowExW(Default::default(), w!("STATIC"), w!("decka overlay test"), WS_POPUP, 0, 0, 8, 8, None, None, None, None) }.unwrap();
        let hwnd = window.0 as isize;
        // SAFETY: plain queries.
        let before = unsafe { GetForegroundWindow() };
        super::guard(hwnd);
        super::show(hwnd);
        pump();
        // SAFETY: queries of the window created above.
        unsafe {
            assert!(IsWindowVisible(window).as_bool(), "shown");
            assert_eq!(GetForegroundWindow(), before, "showing took the focus");
            assert_eq!(SendMessageW(window, WM_MOUSEACTIVATE, None, None).0, MA_NOACTIVATE as isize, "a click would activate");
        }
        for on in [true, false, true] {
            super::set_click_through(hwnd, on);
            pump();
            // SAFETY: queries of the window created above.
            let (style, front) = unsafe { (GetWindowLongPtrW(window, GWL_EXSTYLE), GetForegroundWindow()) };
            assert_eq!(style & WS_EX_TRANSPARENT.0 as isize != 0, on);
            assert!(style & WS_EX_NOACTIVATE.0 as isize != 0);
            assert_eq!(front, before, "click-through took the focus");
        }
        super::hide(hwnd);
        pump();
        // SAFETY: the window of this test.
        unsafe {
            assert!(!IsWindowVisible(window).as_bool(), "hidden");
            let _ = DestroyWindow(window);
        }
    }
}
