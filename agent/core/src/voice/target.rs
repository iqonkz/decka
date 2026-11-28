//! Where the user was when a voice request began (T29.1): the window and the element with the focus. The selection is read
//! from there and a dictation is typed there; when the focus has moved by the end of the recognition, the text goes into the
//! new place only if that is a field, else to the clipboard. The decisions are pure; `FocusMemory` is the thin Windows layer,
//! a thread of its own that holds the UI Automation element.
//!
//! Nothing read here is logged or written to disk.

use std::time::Duration;

use super::clipboard::{Clipboard, Grabbed, Keys};
use super::textops;

/// Whether text can be typed into an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    Yes,
    /// Surely not: the desktop, a button, a web page without a field.
    No,
    /// The program does not tell (games, custom editors).
    Unknown,
}

/// A place of the focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    pub window: isize,
    /// The runtime id of the focused element; empty when the program gives none.
    pub element: Vec<i32>,
    pub edit: Edit,
}

impl Spot {
    /// The same window, and the same element as far as both are known.
    pub fn same(&self, other: &Spot) -> bool {
        self.window == other.window && (self.element.is_empty() || other.element.is_empty() || self.element == other.element)
    }
}

/// Where a recognised text goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Into the focused field.
    Field,
    /// To the clipboard, with "Copied: paste with Ctrl+V".
    Clipboard,
}

/// The route of a dictated or inserted text. `before`: the focus when the request began; `now`: the focus at its end
/// (`None`: no window has it).
pub fn route(before: Option<&Spot>, now: Option<&Spot>) -> Route {
    let Some(now) = now else { return Route::Clipboard };
    match (now.edit, before) {
        (Edit::No, _) => Route::Clipboard,
        (Edit::Yes, _) | (Edit::Unknown, None) => Route::Field,
        // A program that does not tell is trusted only where the user already was.
        (Edit::Unknown, Some(before)) => {
            if before.same(now) {
                Route::Field
            } else {
                Route::Clipboard
            }
        }
    }
}

/// The route of a rewritten selection: it replaces the selection only where that selection still has the focus.
pub fn route_replace(before: Option<&Spot>, now: Option<&Spot>) -> Route {
    match (before, now) {
        (_, None) => Route::Clipboard,
        // T32.H.1: text that cannot be typed into (a page of a browser) cannot be replaced.
        (_, Some(now)) if now.edit == Edit::No => Route::Clipboard,
        (None, Some(_)) => Route::Field,
        (Some(before), Some(now)) => {
            if before.same(now) {
                Route::Field
            } else {
                Route::Clipboard
            }
        }
    }
}

/// Windows whose program loses typed Unicode characters (it reads the input late and repeats the last one): text goes into
/// them through the clipboard. The Notepad of Windows 11.
pub fn typing_is_lossy(window_class: &str) -> bool {
    window_class == "Notepad"
}

/// The desktop of the screen, seen from here.
pub trait Desk {
    /// The window in front.
    fn window(&self) -> Option<isize>;
    /// The selection of the element that was remembered before the island was shown.
    fn remembered_selection(&self) -> Option<String>;
    /// The selection of the element that has the focus now.
    fn focused_selection(&self) -> Option<String>;
    /// Brings a window to the front; `false` when Windows refuses.
    fn activate(&self, window: isize) -> bool;
}

/// The selection for an operation: from the remembered element; else `Ctrl+C` in the remembered window (brought back to the
/// front when the user has moved on). Without a remembered place, the focused element as before.
pub fn take_selection(desk: &dyn Desk, before: Option<&Spot>, cb: &dyn Clipboard, keys: &dyn Keys, sleep: &dyn Fn(Duration)) -> Result<Grabbed, String> {
    if let Some(before) = before {
        if let Some(text) = desk.remembered_selection().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
            if text.chars().count() > textops::MAX_SELECTION {
                return Err(textops::TOO_LONG.to_string());
            }
            return Ok(Grabbed { text, previous: None });
        }
        if desk.window() != Some(before.window) && !desk.activate(before.window) {
            return Err(textops::SELECT_FIRST.to_string());
        }
    }
    textops::take_selection_uia_first(&|| desk.focused_selection(), cb, keys, sleep)
}

#[cfg(windows)]
mod win {
    use std::sync::mpsc::{self, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::System::Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationValuePattern, UIA_ButtonControlTypeId,
        UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_HyperlinkControlTypeId, UIA_ImageControlTypeId, UIA_ListControlTypeId,
        UIA_ListItemControlTypeId, UIA_MenuItemControlTypeId, UIA_TabItemControlTypeId, UIA_TextPatternId, UIA_TreeItemControlTypeId, UIA_ValuePatternId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, SetForegroundWindow};

    use super::{Desk, Edit, Spot};
    use crate::voice::uia::UIA_TIMEOUT;

    /// More than this is never read (the callers refuse a longer selection anyway).
    const MAX_CHARS: i32 = 7_000;

    /// What the worker thread keeps between jobs.
    struct Ctx {
        automation: Option<IUIAutomation>,
        remembered: Option<IUIAutomationElement>,
    }

    type Job = Box<dyn FnOnce(&mut Ctx) + Send>;

    fn worker() -> Sender<Job> {
        let (tx, rx) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new().name("decka-focus".into()).spawn(move || {
            // SAFETY: this thread stays in the multithreaded apartment for its whole life.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            // SAFETY: creates the UI Automation client of this thread.
            let automation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok();
            let mut ctx = Ctx { automation, remembered: None };
            while let Ok(job) = rx.recv() {
                job(&mut ctx);
            }
        });
        if let Err(e) = spawned {
            tracing::warn!("focus thread: {e}");
        }
        tx
    }

    fn foreground() -> Option<isize> {
        // SAFETY: a plain query.
        let h = unsafe { GetForegroundWindow() };
        (!h.0.is_null()).then_some(h.0 as isize)
    }

    pub fn window_class(window: isize) -> String {
        let mut buf = [0u16; 64];
        // SAFETY: `buf` lives for the call.
        let n = unsafe { GetClassNameW(HWND(window as *mut _), &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    /// The desktop and the taskbar take no text.
    fn is_shell(window: isize) -> bool {
        matches!(window_class(window).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
    }

    fn runtime_id(element: &IUIAutomationElement) -> Vec<i32> {
        // SAFETY: the array belongs to us: read within its bounds and destroyed here.
        unsafe {
            let Ok(array) = element.GetRuntimeId() else { return Vec::new() };
            if array.is_null() {
                return Vec::new();
            }
            let mut out = Vec::new();
            if let (Ok(low), Ok(high)) = (SafeArrayGetLBound(array, 1), SafeArrayGetUBound(array, 1)) {
                for i in low..=high.min(low + 63) {
                    let mut value = 0i32;
                    if SafeArrayGetElement(array, &i, (&mut value as *mut i32).cast()).is_ok() {
                        out.push(value);
                    }
                }
            }
            let _ = SafeArrayDestroy(array);
            out
        }
    }

    fn editable(element: &IUIAutomationElement) -> Edit {
        // SAFETY: plain queries of an element; a program that does not answer gives an error.
        unsafe {
            let Ok(kind) = element.CurrentControlType() else { return Edit::Unknown };
            let read_only = element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).ok().and_then(|p| p.CurrentIsReadOnly().ok()).map(|b| b.as_bool());
            if kind == UIA_EditControlTypeId {
                return if read_only == Some(true) { Edit::No } else { Edit::Yes };
            }
            match read_only {
                Some(false) => return Edit::Yes,
                // A web page without a field is a read-only document.
                Some(true) if kind == UIA_DocumentControlTypeId => return Edit::No,
                _ => {}
            }
            let not_a_field = [
                UIA_ButtonControlTypeId,
                UIA_HyperlinkControlTypeId,
                UIA_ImageControlTypeId,
                UIA_ListControlTypeId,
                UIA_ListItemControlTypeId,
                UIA_MenuItemControlTypeId,
                UIA_TabItemControlTypeId,
                UIA_TreeItemControlTypeId,
            ];
            if not_a_field.contains(&kind) {
                return Edit::No;
            }
        }
        Edit::Unknown
    }

    fn look(ctx: &Ctx) -> (Option<Spot>, Option<IUIAutomationElement>) {
        let Some(window) = foreground() else { return (None, None) };
        // SAFETY: a query of the client of this thread.
        let element = ctx.automation.as_ref().and_then(|a| unsafe { a.GetFocusedElement() }.ok());
        let (id, edit) = element.as_ref().map_or((Vec::new(), Edit::Unknown), |e| (runtime_id(e), editable(e)));
        let edit = if is_shell(window) { Edit::No } else { edit };
        (Some(Spot { window, element: id, edit }), element)
    }

    fn selection_of(element: &IUIAutomationElement) -> Option<String> {
        // SAFETY: plain queries of an element and of the ranges it returns.
        unsafe {
            let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
            let ranges = pattern.GetSelection().ok()?;
            let mut text = String::new();
            for i in 0..ranges.Length().ok()? {
                text.push_str(&ranges.GetElement(i).ok()?.GetText(MAX_CHARS).ok()?.to_string());
                if text.chars().count() > MAX_CHARS as usize {
                    break;
                }
            }
            Some(text)
        }
    }

    /// The remembered focus of the running request.
    #[derive(Clone)]
    pub struct FocusMemory {
        jobs: Arc<Mutex<Sender<Job>>>,
        spot: Arc<Mutex<Option<Spot>>>,
    }

    impl Default for FocusMemory {
        fn default() -> Self {
            FocusMemory { jobs: Arc::new(Mutex::new(worker())), spot: Arc::default() }
        }
    }

    impl FocusMemory {
        /// Runs a job on the focus thread and waits for it. A thread stuck in a program that hangs is left behind.
        fn ask<T: Send + 'static>(&self, job: impl FnOnce(&mut Ctx) -> T + Send + 'static, wait: Duration) -> Option<T> {
            let (tx, rx) = mpsc::channel();
            let boxed: Job = Box::new(move |ctx| {
                let _ = tx.send(job(ctx));
            });
            if self.jobs.lock().unwrap().send(boxed).is_err() {
                *self.jobs.lock().unwrap() = worker();
                return None;
            }
            match rx.recv_timeout(wait) {
                Ok(v) => Some(v),
                Err(_) => {
                    *self.jobs.lock().unwrap() = worker();
                    None
                }
            }
        }

        /// Remembers where the focus is now. Does not wait.
        pub fn remember(&self) {
            // The window is known at once; the element follows from the focus thread.
            *self.spot.lock().unwrap() = foreground().map(|window| Spot { window, element: Vec::new(), edit: Edit::Unknown });
            let spot = self.spot.clone();
            let job: Job = Box::new(move |ctx| {
                let (found, element) = look(ctx);
                ctx.remembered = element;
                *spot.lock().unwrap() = found;
            });
            let _ = self.jobs.lock().unwrap().send(job);
        }

        /// The place that was remembered. Waits for a look that is still running.
        pub fn remembered(&self) -> Option<Spot> {
            let _ = self.ask(|_| (), UIA_TIMEOUT);
            self.spot.lock().unwrap().clone()
        }

        /// Where the focus is now.
        pub fn current(&self) -> Option<Spot> {
            self.ask(|ctx| look(ctx).0, UIA_TIMEOUT).unwrap_or_else(|| foreground().map(|window| Spot { window, element: Vec::new(), edit: Edit::Unknown }))
        }
    }

    impl Desk for FocusMemory {
        fn window(&self) -> Option<isize> {
            foreground()
        }

        fn remembered_selection(&self) -> Option<String> {
            self.ask(|ctx| ctx.remembered.as_ref().and_then(selection_of), UIA_TIMEOUT).flatten()
        }

        fn focused_selection(&self) -> Option<String> {
            crate::voice::uia::selected_text()
        }

        fn activate(&self, window: isize) -> bool {
            // SAFETY: asks the window manager; a window that is gone or refused gives `false`.
            let _ = unsafe { SetForegroundWindow(HWND(window as *mut _)) };
            for _ in 0..10 {
                if foreground() == Some(window) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        }
    }
}

#[cfg(windows)]
pub use win::{window_class, FocusMemory};

#[cfg(not(windows))]
pub fn window_class(_: isize) -> String {
    String::new()
}

/// Elsewhere there is no focus to remember.
#[cfg(not(windows))]
#[derive(Clone, Default)]
pub struct FocusMemory;

#[cfg(not(windows))]
impl FocusMemory {
    pub fn remember(&self) {}
    pub fn remembered(&self) -> Option<Spot> {
        None
    }
    pub fn current(&self) -> Option<Spot> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::clipboard::fakes::Fake;
    use std::cell::Cell;

    fn spot(window: isize, element: &[i32], edit: Edit) -> Spot {
        Spot { window, element: element.to_vec(), edit }
    }

    #[test]
    fn an_unchanged_focus_takes_the_text_unless_it_is_surely_no_field() {
        let before = spot(1, &[7], Edit::Yes);
        assert_eq!(route(Some(&before), Some(&before)), Route::Field);
        let silent = spot(1, &[], Edit::Unknown);
        assert_eq!(route(Some(&silent), Some(&silent)), Route::Field, "a program that tells nothing");
        let desktop = spot(2, &[], Edit::No);
        assert_eq!(route(Some(&desktop), Some(&desktop)), Route::Clipboard, "nowhere to type");
        assert_eq!(route(Some(&before), None), Route::Clipboard);
    }

    #[test]
    fn a_moved_focus_takes_the_text_only_into_a_field() {
        let before = spot(1, &[7], Edit::Yes);
        assert_eq!(route(Some(&before), Some(&spot(2, &[1], Edit::Yes))), Route::Field);
        assert_eq!(route(Some(&before), Some(&spot(2, &[1], Edit::Unknown))), Route::Clipboard);
        assert_eq!(route(Some(&before), Some(&spot(1, &[8], Edit::Unknown))), Route::Clipboard, "another element of the window");
        assert_eq!(route(Some(&before), Some(&spot(2, &[], Edit::No))), Route::Clipboard);
        assert_eq!(route(None, Some(&spot(2, &[], Edit::Unknown))), Route::Field, "nothing remembered: as before");
    }

    #[test]
    fn a_selection_is_replaced_only_where_it_still_has_the_focus() {
        let before = spot(1, &[7], Edit::Yes);
        assert_eq!(route_replace(Some(&before), Some(&before)), Route::Field);
        assert_eq!(route_replace(Some(&before), Some(&spot(2, &[1], Edit::Yes))), Route::Clipboard);
        let page = spot(1, &[7], Edit::No);
        assert_eq!(route_replace(Some(&page), Some(&page)), Route::Clipboard, "a page of a browser is not a field");
        assert_eq!(route_replace(Some(&before), None), Route::Clipboard);
        assert_eq!(route_replace(None, Some(&before)), Route::Field);
    }

    struct FakeDesk {
        front: Cell<isize>,
        remembered: Option<&'static str>,
        focused: Option<&'static str>,
        may_activate: bool,
        activated: Cell<isize>,
    }

    impl Desk for FakeDesk {
        fn window(&self) -> Option<isize> {
            Some(self.front.get())
        }
        fn remembered_selection(&self) -> Option<String> {
            self.remembered.map(String::from)
        }
        fn focused_selection(&self) -> Option<String> {
            self.focused.map(String::from)
        }
        fn activate(&self, window: isize) -> bool {
            self.activated.set(window);
            if self.may_activate {
                self.front.set(window);
            }
            self.may_activate
        }
    }

    fn desk(front: isize, remembered: Option<&'static str>, focused: Option<&'static str>, may_activate: bool) -> FakeDesk {
        FakeDesk { front: Cell::new(front), remembered, focused, may_activate, activated: Cell::new(0) }
    }

    fn no_sleep(_: Duration) {}

    /// By hand: `cargo test -p decka-core --lib what_has_the_focus -- --ignored --nocapture`, then click into a field within 3 s.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn what_has_the_focus() {
        let memory = FocusMemory::default();
        std::thread::sleep(Duration::from_secs(3));
        memory.remember();
        let (before, now) = (memory.remembered(), memory.current());
        println!("before {before:?}\nnow {now:?}\nroute {:?}", route(before.as_ref(), now.as_ref()));
    }

    /// T29.1: the selection comes from the window remembered before the island was shown, not from what is in front now.
    #[test]
    fn the_selection_comes_from_the_remembered_window() {
        let before = spot(1, &[7], Edit::Yes);
        let keys = Fake::new(Some("mine"), Some("this text"));
        let d = desk(9, Some("words of the document"), Some("this text"), true);
        let got = take_selection(&d, Some(&before), &keys, &keys, &no_sleep).unwrap();
        assert_eq!(got.text, "words of the document");
        assert_eq!((keys.copies(), d.activated.get()), (0, 0), "no keys, no window moved");
    }

    #[test]
    fn without_a_text_pattern_ctrl_c_goes_to_the_remembered_window() {
        let before = spot(1, &[7], Edit::Yes);
        let keys = Fake::new(Some("mine"), Some("copied words"));
        let d = desk(9, None, None, true);
        assert_eq!(take_selection(&d, Some(&before), &keys, &keys, &no_sleep).unwrap().text, "copied words");
        assert_eq!((d.activated.get(), keys.copies()), (1, 1), "brought back first");

        // Windows refuses to bring it back: no keys go to a foreign window.
        let keys = Fake::new(Some("mine"), Some("foreign words"));
        let d = desk(9, None, Some("foreign words"), false);
        assert_eq!(take_selection(&d, Some(&before), &keys, &keys, &no_sleep).unwrap_err(), textops::SELECT_FIRST);
        assert_eq!(keys.copies(), 0);
    }

    #[test]
    fn a_remembered_selection_that_is_too_long_is_refused() {
        let before = spot(1, &[], Edit::Yes);
        let long: &'static str = Box::leak("a".repeat(textops::MAX_SELECTION + 1).into_boxed_str());
        let keys = Fake::new(None, None);
        let d = desk(1, Some(long), None, true);
        assert_eq!(take_selection(&d, Some(&before), &keys, &keys, &no_sleep).unwrap_err(), textops::TOO_LONG);
    }
}
