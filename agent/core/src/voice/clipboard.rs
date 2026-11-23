//! The clipboard as a tool of voice control (DATA_MODEL.md 13.1, 13.2): paste a long dictation, or take the selected text
//! (`Ctrl+C`) and put the result back (`Ctrl+V`). The user's own clipboard text is saved first and restored afterwards.
//! The order of steps is pure over the `Clipboard` and `Keys` traits; `System*` are the thin Windows layer.
//! Only text is preserved: a picture or files on the clipboard are replaced for a moment and are not restored.

use std::time::{Duration, Instant};

pub trait Clipboard: Send + Sync {
    fn get(&self) -> Option<String>;
    fn set(&self, text: &str) -> Result<(), String>;
    /// Grows with every change of the clipboard (any format).
    fn seq(&self) -> u32;
}

pub trait Keys: Send + Sync {
    fn copy(&self) -> Result<(), String>;
    fn paste(&self) -> Result<(), String>;
}

/// How long `Ctrl+C` may take to change the clipboard.
pub const COPY_WAIT: Duration = Duration::from_millis(400);
const POLL: Duration = Duration::from_millis(20);
/// The program needs a moment to read the clipboard after `Ctrl+V` before it is restored.
pub const PASTE_SETTLE: Duration = Duration::from_millis(150);

/// The selection and the text that was on the clipboard before it was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grabbed {
    pub text: String,
    pub previous: Option<String>,
}

/// `Ctrl+C`, wait for the clipboard to change (<= `wait`), read the text. `Ok(None)`: nothing is selected (the clipboard did
/// not change); the old text is still in place. `sleep` is injected so tests do not wait.
pub fn grab_selection(cb: &dyn Clipboard, keys: &dyn Keys, wait: Duration, sleep: &dyn Fn(Duration)) -> Result<Option<Grabbed>, String> {
    let previous = cb.get();
    let before = cb.seq();
    keys.copy()?;
    let start = Instant::now();
    let mut waited = Duration::ZERO;
    while cb.seq() == before {
        if waited >= wait || start.elapsed() >= wait {
            return Ok(None);
        }
        sleep(POLL);
        waited += POLL;
    }
    match cb.get().filter(|t| !t.is_empty()) {
        Some(text) => Ok(Some(Grabbed { text, previous })),
        None => {
            restore(cb, previous.as_deref());
            Ok(None)
        }
    }
}

fn restore(cb: &dyn Clipboard, previous: Option<&str>) {
    if let Some(p) = previous {
        let _ = cb.set(p);
    }
}

/// Puts `text` on the clipboard, `Ctrl+V`, then returns the user's old text to the clipboard after `restore_after`.
pub fn paste_text(cb: &dyn Clipboard, keys: &dyn Keys, text: &str, previous: Option<String>, restore_after: Duration, sleep: &dyn Fn(Duration)) -> Result<(), String> {
    let previous = previous.or_else(|| cb.get());
    cb.set(text)?;
    let pasted = keys.paste();
    sleep(restore_after.max(PASTE_SETTLE));
    restore(cb, previous.as_deref());
    pasted
}

/// Gives the old clipboard text back without pasting anything (the result is only shown).
pub fn give_back(cb: &dyn Clipboard, grabbed: &Grabbed) {
    restore(cb, grabbed.previous.as_deref());
}

#[cfg(windows)]
mod win {
    use super::{Clipboard, Keys};
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    /// `CF_UNICODETEXT`.
    const CF_UNICODETEXT: u32 = 13;

    pub struct SystemClipboard;

    /// Opens the clipboard, retrying for a moment: another program may hold it.
    fn open() -> Result<(), String> {
        for _ in 0..10 {
            // SAFETY: a plain call; no window owns the clipboard for this thread.
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        Err("The clipboard is busy".into())
    }

    impl Clipboard for SystemClipboard {
        fn get(&self) -> Option<String> {
            open().ok()?;
            // SAFETY: the clipboard is open; the handle is read under `GlobalLock` and copied before it is released.
            let text = unsafe {
                GetClipboardData(CF_UNICODETEXT).ok().and_then(|h| {
                    let g = HGLOBAL(h.0);
                    let p = GlobalLock(g) as *const u16;
                    if p.is_null() {
                        return None;
                    }
                    let mut len = 0usize;
                    while *p.add(len) != 0 && len < 4_000_000 {
                        len += 1;
                    }
                    let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
                    let _ = GlobalUnlock(g);
                    Some(s)
                })
            };
            // SAFETY: closes what `open` opened.
            let _ = unsafe { CloseClipboard() };
            text
        }

        fn set(&self, text: &str) -> Result<(), String> {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            open()?;
            // SAFETY: the clipboard is open; the block is filled before it is handed over, and the system owns it afterwards.
            let result = unsafe {
                (|| {
                    let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).map_err(|e| e.to_string())?;
                    let p = GlobalLock(mem) as *mut u16;
                    if p.is_null() {
                        return Err("The clipboard is unavailable".to_string());
                    }
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                    let _ = GlobalUnlock(mem);
                    EmptyClipboard().map_err(|e| e.to_string())?;
                    SetClipboardData(CF_UNICODETEXT, Some(HANDLE(mem.0))).map(|_| ()).map_err(|e| e.to_string())
                })()
            };
            // SAFETY: closes what `open` opened.
            let _ = unsafe { CloseClipboard() };
            result
        }

        fn seq(&self) -> u32 {
            // SAFETY: a plain query.
            unsafe { GetClipboardSequenceNumber() }
        }
    }

    pub struct SystemKeys;

    fn chord(letter: char) -> Result<(), String> {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut e = Enigo::new(&Settings::default()).map_err(|e| format!("keyboard unavailable: {e}"))?;
        e.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
        let clicked = e.key(Key::Unicode(letter), Direction::Click).map_err(|e| e.to_string());
        let _ = e.key(Key::Control, Direction::Release);
        clicked
    }

    impl Keys for SystemKeys {
        fn copy(&self) -> Result<(), String> {
            chord('c')
        }
        fn paste(&self) -> Result<(), String> {
            chord('v')
        }
    }
}

#[cfg(windows)]
pub use win::{SystemClipboard, SystemKeys};

#[cfg(test)]
pub mod fakes {
    use super::*;
    use std::sync::Mutex;

    /// A clipboard and a keyboard in one: `Ctrl+C` copies `selected` (when there is one), `Ctrl+V` records the pasted text.
    pub struct Fake {
        pub text: Mutex<Option<String>>,
        pub seq: Mutex<u32>,
        pub selected: Option<String>,
        pub pasted: Mutex<Vec<String>>,
    }

    impl Fake {
        pub fn new(clipboard: Option<&str>, selected: Option<&str>) -> Fake {
            Fake { text: Mutex::new(clipboard.map(String::from)), seq: Mutex::new(1), selected: selected.map(String::from), pasted: Mutex::new(vec![]) }
        }
    }

    impl Clipboard for Fake {
        fn get(&self) -> Option<String> {
            self.text.lock().unwrap().clone()
        }
        fn set(&self, text: &str) -> Result<(), String> {
            *self.text.lock().unwrap() = Some(text.to_string());
            *self.seq.lock().unwrap() += 1;
            Ok(())
        }
        fn seq(&self) -> u32 {
            *self.seq.lock().unwrap()
        }
    }

    impl Keys for Fake {
        fn copy(&self) -> Result<(), String> {
            if let Some(s) = &self.selected {
                self.set(s)?;
            }
            Ok(())
        }
        fn paste(&self) -> Result<(), String> {
            let t = self.get().unwrap_or_default();
            self.pasted.lock().unwrap().push(t);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::Fake;
    use super::*;

    fn no_sleep(_: Duration) {}

    #[test]
    fn a_selection_is_taken_and_the_old_clipboard_text_is_remembered() {
        let f = Fake::new(Some("mine"), Some("selected words"));
        let g = grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap().unwrap();
        assert_eq!((g.text.as_str(), g.previous.as_deref()), ("selected words", Some("mine")));
    }

    #[test]
    fn nothing_selected_means_the_clipboard_did_not_change_and_is_left_alone() {
        let f = Fake::new(Some("mine"), None);
        assert_eq!(grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap(), None);
        assert_eq!(f.get().as_deref(), Some("mine"));
    }

    #[test]
    fn a_paste_puts_the_text_in_and_the_old_text_back() {
        let f = Fake::new(Some("mine"), None);
        paste_text(&f, &f, "a long dictated text", None, Duration::from_secs(1), &no_sleep).unwrap();
        assert_eq!(*f.pasted.lock().unwrap(), ["a long dictated text"]);
        assert_eq!(f.get().as_deref(), Some("mine"), "restored");
    }

    #[test]
    fn replacing_a_selection_pastes_the_result_and_restores_the_buffer() {
        let f = Fake::new(Some("mine"), Some("hello"));
        let g = grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap().unwrap();
        paste_text(&f, &f, "Hello, dear friend", g.previous.clone(), Duration::from_secs(1), &no_sleep).unwrap();
        assert_eq!(*f.pasted.lock().unwrap(), ["Hello, dear friend"]);
        assert_eq!(f.get().as_deref(), Some("mine"));
    }

    #[test]
    fn showing_only_gives_the_buffer_back_without_a_paste() {
        let f = Fake::new(Some("mine"), Some("hello"));
        let g = grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap().unwrap();
        assert_eq!(f.get().as_deref(), Some("hello"));
        give_back(&f, &g);
        assert_eq!(f.get().as_deref(), Some("mine"));
        assert!(f.pasted.lock().unwrap().is_empty());
    }
}
