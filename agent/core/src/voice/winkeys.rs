//! The low-level keyboard hook for the double Ctrl of dictation (DATA_MODEL.md 13.1). The hook only looks: every key goes on
//! to the program that has the focus. Keys that Decka sends itself (`Ctrl+C` of a selection, typed text) are marked as
//! injected and are not counted. Which presses make a double Ctrl is `dictation::DoubleCtrl`.

#[cfg(windows)]
mod win {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Instant;

    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    use crate::voice::dictation::{DoubleCtrl, Key};

    const LLKHF_INJECTED: u32 = 0x10;
    const VK_CONTROL: u32 = 0x11;
    const VK_LCONTROL: u32 = 0xA2;
    const VK_RCONTROL: u32 = 0xA3;

    /// What the hook reports.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Ctrl {
        /// Ctrl pressed and released twice, quickly.
        Double,
        /// One short Ctrl tap (the end of a dictation).
        Tap,
    }

    struct Shared {
        detector: DoubleCtrl,
        t0: Instant,
        on: Arc<dyn Fn(Ctrl) + Send + Sync>,
    }

    /// The hook procedure has no closure: its state lives here. One hook per process.
    static SHARED: OnceLock<Mutex<Option<Shared>>> = OnceLock::new();

    unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            // SAFETY: for WH_KEYBOARD_LL `lparam` points to a KBDLLHOOKSTRUCT that is valid during the call.
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if info.flags.0 & LLKHF_INJECTED == 0 {
                let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                let up = matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP);
                if down || up {
                    let key = if matches!(info.vkCode, VK_CONTROL | VK_LCONTROL | VK_RCONTROL) { Key::Ctrl } else { Key::Other };
                    let mut event = None;
                    if let Some(Ok(mut guard)) = SHARED.get().map(|m| m.lock()) {
                        if let Some(s) = guard.as_mut() {
                            let t = s.t0.elapsed().as_millis() as u64;
                            if s.detector.feed(key, down, t) {
                                event = Some((Ctrl::Double, s.on.clone()));
                            } else if key == Key::Ctrl && up && s.detector.take_tap() {
                                event = Some((Ctrl::Tap, s.on.clone()));
                            }
                        }
                    }
                    // The callback runs outside the lock and must not block the keyboard: it only sends a message on.
                    if let Some((e, on)) = event {
                        on(e);
                    }
                }
            }
        }
        // SAFETY: passes the event on unchanged.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    /// The installed hook and its thread. Dropping it removes the hook.
    pub struct CtrlHook {
        thread_id: u32,
        join: Mutex<Option<std::thread::JoinHandle<()>>>,
    }

    impl CtrlHook {
        pub fn start(on: Arc<dyn Fn(Ctrl) + Send + Sync>) -> Result<CtrlHook, String> {
            let (id_tx, id_rx) = mpsc::channel();
            let join = std::thread::Builder::new()
                .name("decka-ctrl-hook".into())
                .spawn(move || {
                    let mut msg = MSG::default();
                    // SAFETY: touching the queue creates it, so that the quit message is not lost.
                    let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };
                    *SHARED.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(Shared { detector: DoubleCtrl::default(), t0: Instant::now(), on });
                    // SAFETY: a low-level hook needs no module handle; the procedure is a plain function.
                    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0) };
                    // SAFETY: a plain query.
                    let tid = unsafe { GetCurrentThreadId() };
                    match hook {
                        Ok(h) => {
                            let _ = id_tx.send(Ok(tid));
                            // The hook is called from this loop.
                            // SAFETY: `msg` is valid; 0 / -1 end the loop.
                            while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}
                            // SAFETY: removes the hook installed above.
                            let _ = unsafe { UnhookWindowsHookEx(h) };
                        }
                        Err(e) => {
                            let _ = id_tx.send(Err(e.to_string()));
                        }
                    }
                    *SHARED.get_or_init(|| Mutex::new(None)).lock().unwrap() = None;
                })
                .map_err(|e| e.to_string())?;
            let thread_id = id_rx.recv().map_err(|_| "The keyboard hook did not start".to_string())??;
            Ok(CtrlHook { thread_id, join: Mutex::new(Some(join)) })
        }
    }

    impl Drop for CtrlHook {
        fn drop(&mut self) {
            // SAFETY: posts WM_QUIT to the thread that owns the loop.
            let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
            if let Some(j) = self.join.lock().unwrap().take() {
                let _ = j.join();
            }
        }
    }
}

#[cfg(windows)]
pub use win::{Ctrl, CtrlHook};
