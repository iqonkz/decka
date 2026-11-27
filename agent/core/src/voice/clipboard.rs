//! The clipboard as a tool of voice control (DATA_MODEL.md 13.1, 13.2): paste a long dictation, or take the selected text
//! (`Ctrl+C`) and put the result back (`Ctrl+V`). Everything the user had on the clipboard - text, pictures, files, any
//! format - is saved first and restored afterwards, and neither the text Decka puts there for a moment nor the restored
//! content enters the clipboard history (Win+V) or the cloud clipboard (T29.P).
//! The order of steps is pure over the `Clipboard` and `Keys` traits; `System*` are the thin Windows layer.

use std::time::{Duration, Instant};

/// Everything that was on the clipboard: `(format, bytes)` in the order the system lists them.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Snapshot(pub Vec<(u32, Vec<u8>)>);

/// The content may be private: only its shape is ever printed.
impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Snapshot({} formats)", self.0.len())
    }
}

pub trait Clipboard: Send + Sync {
    fn get(&self) -> Option<String>;
    /// A text the user is meant to keep (`Ctrl+V` later): it behaves like any copy.
    fn set(&self, text: &str) -> Result<(), String>;
    /// A text that is there for a moment only: it stays out of the clipboard history and the cloud.
    fn set_temporary(&self, text: &str) -> Result<(), String>;
    /// The whole content; `None` when the clipboard cannot be read (then nothing is restored over it).
    fn snapshot(&self) -> Option<Snapshot>;
    /// Puts a snapshot back (an empty one empties the clipboard), without a new entry in the history.
    fn restore(&self, snapshot: &Snapshot) -> Result<(), String>;
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

/// The selection and what was on the clipboard before it was taken (`None`: the clipboard was not used).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grabbed {
    pub text: String,
    pub previous: Option<Snapshot>,
}

/// `Ctrl+C`, wait for the clipboard to change (<= `wait`), read the text. `Ok(None)`: nothing is selected (the clipboard did
/// not change); the old content is still in place. `sleep` is injected so tests do not wait.
pub fn grab_selection(cb: &dyn Clipboard, keys: &dyn Keys, wait: Duration, sleep: &dyn Fn(Duration)) -> Result<Option<Grabbed>, String> {
    let previous = cb.snapshot();
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
            restore(cb, previous.as_ref());
            Ok(None)
        }
    }
}

fn restore(cb: &dyn Clipboard, previous: Option<&Snapshot>) {
    if let Some(p) = previous {
        if let Err(e) = cb.restore(p) {
            tracing::warn!("the clipboard could not be restored: {e}");
        }
    }
}

/// Puts `text` on the clipboard, `Ctrl+V`, then returns the user's old content to the clipboard after `restore_after`.
pub fn paste_text(cb: &dyn Clipboard, keys: &dyn Keys, text: &str, previous: Option<Snapshot>, restore_after: Duration, sleep: &dyn Fn(Duration)) -> Result<(), String> {
    let previous = previous.or_else(|| cb.snapshot());
    cb.set_temporary(text)?;
    let pasted = keys.paste();
    sleep(restore_after.max(PASTE_SETTLE));
    restore(cb, previous.as_ref());
    pasted
}

/// Gives the old clipboard content back without pasting anything (the result is only shown).
pub fn give_back(cb: &dyn Clipboard, grabbed: &Grabbed) {
    restore(cb, grabbed.previous.as_ref());
}

#[cfg(windows)]
mod win {
    use super::{Clipboard, Keys, Snapshot};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::Graphics::Gdi::{DeleteEnhMetaFile, GetEnhMetaFileBits, SetEnhMetaFileBits, HENHMETAFILE};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardFormatNameW, GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

    const CF_BITMAP: u32 = 2;
    const CF_METAFILEPICT: u32 = 3;
    const CF_PALETTE: u32 = 9;
    const CF_UNICODETEXT: u32 = 13;
    const CF_ENHMETAFILE: u32 = 14;
    /// Registered formats start here; below `0x200` are the standard ones.
    const FIRST_REGISTERED: u32 = 0xC000;
    /// A snapshot never grows beyond this: a format that would pass it is left out.
    const MAX_SNAPSHOT: usize = 256 * 1024 * 1024;
    /// Windows keeps content with these marks out of the clipboard history and the cloud clipboard.
    const PRIVATE_MARKS: [&str; 3] = ["ExcludeClipboardContentFromMonitorProcessing", "CanIncludeInClipboardHistory", "CanUploadToCloudClipboard"];

    pub struct SystemClipboard;

    /// The clipboard, open for this thread until the value is dropped.
    struct Open;

    impl Open {
        /// Retries for a moment: another program may hold it.
        fn new() -> Result<Open, String> {
            for _ in 0..10 {
                // SAFETY: a plain call; no window owns the clipboard for this thread.
                if unsafe { OpenClipboard(None) }.is_ok() {
                    return Ok(Open);
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            Err("The clipboard is busy".into())
        }
    }

    impl Drop for Open {
        fn drop(&mut self) {
            // SAFETY: closes what `new` opened.
            let _ = unsafe { CloseClipboard() };
        }
    }

    /// The bytes of a memory block of the clipboard. The clipboard must be open.
    unsafe fn read_block(handle: HANDLE) -> Option<Vec<u8>> {
        let block = HGLOBAL(handle.0);
        let size = GlobalSize(block);
        let p = GlobalLock(block) as *const u8;
        if p.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(p, size).to_vec();
        let _ = GlobalUnlock(block);
        Some(bytes)
    }

    /// Hands a copy of `bytes` to the clipboard, which owns it afterwards. The clipboard must be open and emptied.
    unsafe fn write_block(format: u32, bytes: &[u8]) -> Result<(), String> {
        let block = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|e| e.to_string())?;
        let p = GlobalLock(block) as *mut u8;
        if p.is_null() {
            return Err("The clipboard is unavailable".to_string());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(block);
        SetClipboardData(format, Some(HANDLE(block.0))).map(|_| ()).map_err(|e| e.to_string())
    }

    fn registered(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a terminated string that lives for the call.
        unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
    }

    fn format_name(format: u32) -> String {
        let mut buffer = [0u16; 128];
        // SAFETY: the buffer is given with its length.
        let length = unsafe { GetClipboardFormatNameW(format, &mut buffer) };
        String::from_utf16_lossy(&buffer[..length.max(0) as usize])
    }

    /// Whether the data of a format is a plain memory block that can be copied and put back.
    fn copyable(format: u32) -> bool {
        match format {
            // Windows makes the bitmap and the old metafile from the bitmap bytes and the enhanced metafile.
            CF_BITMAP | CF_METAFILEPICT | CF_PALETTE => false,
            // The other standard formats up to CF_DIBV5 are memory blocks.
            1..=17 => true,
            // OLE bookkeeping points at objects of the program that copied; put back, it would point at nothing.
            f if f >= FIRST_REGISTERED => !matches!(format_name(f).as_str(), "DataObject" | "Ole Private Data"),
            // Owner-drawn, display, private and GDI object formats are not memory blocks.
            _ => false,
        }
    }

    fn mark_private() {
        for name in PRIVATE_MARKS {
            let format = registered(name);
            if format != 0 {
                // SAFETY: the caller holds the clipboard open. A DWORD of zero: "no".
                let _ = unsafe { write_block(format, &0u32.to_le_bytes()) };
            }
        }
    }

    impl SystemClipboard {
        fn put_text(text: &str, private: bool) -> Result<(), String> {
            let bytes: Vec<u8> = text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect();
            let _open = Open::new()?;
            // SAFETY: the clipboard is open; it is emptied before data is handed over.
            unsafe {
                EmptyClipboard().map_err(|e| e.to_string())?;
                write_block(CF_UNICODETEXT, &bytes)?;
            }
            if private {
                mark_private();
            }
            Ok(())
        }
    }

    impl Clipboard for SystemClipboard {
        fn get(&self) -> Option<String> {
            let _open = Open::new().ok()?;
            // SAFETY: the clipboard is open; the block is copied before it is released.
            let bytes = unsafe { GetClipboardData(CF_UNICODETEXT).ok().and_then(|h| read_block(h)) }?;
            let wide: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&c| c != 0).take(4_000_000).collect();
            Some(String::from_utf16_lossy(&wide))
        }

        fn set(&self, text: &str) -> Result<(), String> {
            Self::put_text(text, false)
        }

        fn set_temporary(&self, text: &str) -> Result<(), String> {
            Self::put_text(text, true)
        }

        fn snapshot(&self) -> Option<Snapshot> {
            let _open = Open::new().ok()?;
            let mut formats = Vec::new();
            let mut total = 0usize;
            let mut format = 0u32;
            loop {
                // SAFETY: the clipboard is open; 0 starts the list and ends it.
                format = unsafe { EnumClipboardFormats(format) };
                if format == 0 {
                    break;
                }
                if !copyable(format) {
                    continue;
                }
                // SAFETY: the clipboard is open; asking for the data makes a program that renders late deliver it.
                let Ok(handle) = (unsafe { GetClipboardData(format) }) else { continue };
                let bytes = if format == CF_ENHMETAFILE {
                    let metafile = HENHMETAFILE(handle.0);
                    // SAFETY: a metafile of the clipboard: first its size, then its bytes into a buffer of that size.
                    unsafe {
                        let size = GetEnhMetaFileBits(metafile, None);
                        let mut buffer = vec![0u8; size as usize];
                        (size > 0 && GetEnhMetaFileBits(metafile, Some(&mut buffer)) == size).then_some(buffer)
                    }
                } else {
                    // SAFETY: a memory block of the open clipboard.
                    unsafe { read_block(handle) }
                };
                let Some(bytes) = bytes else { continue };
                if total + bytes.len() > MAX_SNAPSHOT {
                    tracing::warn!("a very large clipboard format is not kept");
                    continue;
                }
                total += bytes.len();
                formats.push((format, bytes));
            }
            Some(Snapshot(formats))
        }

        fn restore(&self, snapshot: &Snapshot) -> Result<(), String> {
            let _open = Open::new()?;
            // SAFETY: the clipboard is open.
            unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
            if snapshot.0.is_empty() {
                return Ok(());
            }
            let marks: Vec<u32> = PRIVATE_MARKS.iter().map(|n| registered(n)).collect();
            let mut failed = None;
            for (format, bytes) in snapshot.0.iter().filter(|(f, _)| !marks.contains(f)) {
                let done = if *format == CF_ENHMETAFILE {
                    // SAFETY: the bytes came from a metafile; the clipboard owns the new one once it is taken.
                    unsafe {
                        let metafile = SetEnhMetaFileBits(bytes);
                        let taken = SetClipboardData(*format, Some(HANDLE(metafile.0))).map(|_| ()).map_err(|e| e.to_string());
                        if taken.is_err() {
                            let _ = DeleteEnhMetaFile(Some(metafile));
                        }
                        taken
                    }
                } else {
                    // SAFETY: the clipboard is open and was emptied.
                    unsafe { write_block(*format, bytes) }
                };
                if let Err(e) = done {
                    failed = Some(e);
                }
            }
            // The content was already in the history when it was first copied.
            mark_private();
            failed.map_or(Ok(()), Err)
        }

        fn seq(&self) -> u32 {
            // SAFETY: a plain query.
            unsafe { GetClipboardSequenceNumber() }
        }
    }

    pub struct SystemKeys;

    /// The virtual key of a letter key, the same on every keyboard layout.
    fn chord(vk: u32) -> Result<(), String> {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut e = Enigo::new(&Settings::default()).map_err(|e| format!("keyboard unavailable: {e}"))?;
        e.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
        // Never `Key::Unicode`: on a layout without that letter (Russian, Kazakh) it types the letter instead of the chord.
        let clicked = e.key(Key::Other(vk), Direction::Click).map_err(|e| e.to_string());
        let _ = e.key(Key::Control, Direction::Release);
        clicked
    }

    impl Keys for SystemKeys {
        fn copy(&self) -> Result<(), String> {
            chord(0x43) // C
        }
        fn paste(&self) -> Result<(), String> {
            chord(0x56) // V
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The real clipboard of this PC (it is put back afterwards): run by hand with `-- --ignored`.
        #[test]
        #[ignore = "uses the clipboard of the PC"]
        fn pictures_files_and_text_come_back_and_stay_out_of_the_history() {
            let cb = SystemClipboard;
            // What was there comes back whatever happens in the test.
            struct PutBack(Snapshot);
            impl Drop for PutBack {
                fn drop(&mut self) {
                    let _ = SystemClipboard.restore(&self.0);
                }
            }
            let _mine = PutBack(cb.snapshot().expect("the clipboard can be read"));
            let png = registered("PNG");
            // A 1x1 bitmap (BITMAPINFOHEADER and one pixel), a custom format and a list of files.
            let mut dib = vec![40, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 32, 0];
            dib.resize(40, 0);
            dib.extend([1, 2, 3, 255]);
            let mut files: Vec<u8> = vec![20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
            files.extend("C:\\Windows\\win.ini\0\0".encode_utf16().flat_map(u16::to_le_bytes));
            let original = Snapshot(vec![(8, dib.clone()), (15, files.clone()), (png, vec![7, 7, 7])]);
            cb.restore(&original).unwrap();
            let taken = cb.snapshot().unwrap();
            for (format, bytes) in &original.0 {
                let got = taken.0.iter().find(|(f, _)| f == format).unwrap_or_else(|| panic!("format {format} is gone"));
                assert!(got.1.starts_with(bytes), "format {format}");
            }
            // For the check of the history by hand: the first of these two must not show in Win+V, the second does.
            let pause = || std::thread::sleep(std::time::Duration::from_millis(1200));
            cb.set_temporary("decka-temporary-check").unwrap();
            pause();
            if std::env::var_os("DECKA_HISTORY_CHECK").is_some() {
                cb.set("decka-kept-check").unwrap();
                pause();
            }
            cb.set_temporary("a moment").unwrap();
            assert_eq!(cb.get().as_deref(), Some("a moment"));
            let marked = cb.snapshot().unwrap();
            for name in PRIVATE_MARKS {
                assert!(marked.0.iter().any(|(f, _)| *f == registered(name)), "{name}");
            }
            cb.restore(&taken).unwrap();
            let back = cb.snapshot().unwrap();
            assert!(back.0.iter().any(|(f, b)| *f == 15 && b.starts_with(&files)) && back.0.iter().any(|(f, b)| *f == 8 && b.starts_with(&dib)));
            cb.restore(&Snapshot::default()).unwrap();
            assert!(cb.snapshot().unwrap().0.is_empty());
        }
    }
}

#[cfg(windows)]
pub use win::{SystemClipboard, SystemKeys};

#[cfg(test)]
pub mod fakes {
    use super::*;
    use std::sync::Mutex;

    /// `CF_UNICODETEXT` of the fake: its text; format 8 is its picture.
    pub const TEXT: u32 = 13;
    pub const PICTURE: u32 = 8;

    /// A clipboard and a keyboard in one: `Ctrl+C` copies `selected` (when there is one), `Ctrl+V` records the pasted text.
    pub struct Fake {
        content: Mutex<Vec<(u32, Vec<u8>)>>,
        pub seq: Mutex<u32>,
        pub selected: Option<String>,
        pub pasted: Mutex<Vec<String>>,
        /// Texts that were put there for the history to keep.
        pub kept: Mutex<Vec<String>>,
        copied: Mutex<u32>,
    }

    impl Fake {
        pub fn new(clipboard: Option<&str>, selected: Option<&str>) -> Fake {
            let content = clipboard.map(|t| vec![(TEXT, t.as_bytes().to_vec())]).unwrap_or_default();
            Fake { content: Mutex::new(content), seq: Mutex::new(1), selected: selected.map(String::from), pasted: Mutex::new(vec![]), kept: Mutex::new(vec![]), copied: Mutex::new(0) }
        }

        /// Replaces the content with a picture (no text).
        pub fn hold_picture(&self, bytes: &[u8]) {
            self.put(vec![(PICTURE, bytes.to_vec())]);
        }

        pub fn picture(&self) -> Option<Vec<u8>> {
            self.content.lock().unwrap().iter().find(|(f, _)| *f == PICTURE).map(|(_, b)| b.clone())
        }

        /// How many times `Ctrl+C` was sent.
        pub fn copies(&self) -> u32 {
            *self.copied.lock().unwrap()
        }

        fn put(&self, content: Vec<(u32, Vec<u8>)>) {
            *self.content.lock().unwrap() = content;
            *self.seq.lock().unwrap() += 1;
        }
    }

    impl Clipboard for Fake {
        fn get(&self) -> Option<String> {
            self.content.lock().unwrap().iter().find(|(f, _)| *f == TEXT).map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        }
        fn set(&self, text: &str) -> Result<(), String> {
            self.kept.lock().unwrap().push(text.to_string());
            self.set_temporary(text)
        }
        fn set_temporary(&self, text: &str) -> Result<(), String> {
            self.put(vec![(TEXT, text.as_bytes().to_vec())]);
            Ok(())
        }
        fn snapshot(&self) -> Option<Snapshot> {
            Some(Snapshot(self.content.lock().unwrap().clone()))
        }
        fn restore(&self, snapshot: &Snapshot) -> Result<(), String> {
            self.put(snapshot.0.clone());
            Ok(())
        }
        fn seq(&self) -> u32 {
            *self.seq.lock().unwrap()
        }
    }

    impl Keys for Fake {
        fn copy(&self) -> Result<(), String> {
            *self.copied.lock().unwrap() += 1;
            if let Some(s) = &self.selected {
                self.set_temporary(s)?;
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

    fn text(snapshot: Option<&Snapshot>) -> Option<String> {
        snapshot.and_then(|s| s.0.first()).map(|(_, b)| String::from_utf8_lossy(b).into_owned())
    }

    #[test]
    fn a_selection_is_taken_and_the_old_clipboard_content_is_remembered() {
        let f = Fake::new(Some("mine"), Some("selected words"));
        let g = grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap().unwrap();
        assert_eq!((g.text.as_str(), text(g.previous.as_ref()).as_deref()), ("selected words", Some("mine")));
    }

    #[test]
    fn nothing_selected_means_the_clipboard_did_not_change_and_is_left_alone() {
        let f = Fake::new(Some("mine"), None);
        assert_eq!(grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap(), None);
        assert_eq!(f.get().as_deref(), Some("mine"));
    }

    #[test]
    fn a_paste_puts_the_text_in_and_the_old_text_back_without_a_history_entry() {
        let f = Fake::new(Some("mine"), None);
        paste_text(&f, &f, "a long dictated text", None, Duration::from_secs(1), &no_sleep).unwrap();
        assert_eq!(*f.pasted.lock().unwrap(), ["a long dictated text"]);
        assert_eq!(f.get().as_deref(), Some("mine"), "restored");
        assert!(f.kept.lock().unwrap().is_empty(), "nothing of it is meant for the history");
    }

    #[test]
    fn replacing_a_selection_pastes_the_result_and_restores_the_buffer() {
        let f = Fake::new(Some("mine"), Some("hello"));
        let g = grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap().unwrap();
        paste_text(&f, &f, "Hello, dear friend", g.previous.clone(), Duration::from_secs(1), &no_sleep).unwrap();
        assert_eq!(*f.pasted.lock().unwrap(), ["Hello, dear friend"]);
        assert_eq!(f.get().as_deref(), Some("mine"));
        assert!(f.kept.lock().unwrap().is_empty());
    }

    #[test]
    fn a_picture_survives_a_translation_and_an_empty_clipboard_stays_empty() {
        let f = Fake::new(None, Some("hello"));
        f.hold_picture(&[1, 2, 3]);
        let g = grab_selection(&f, &f, COPY_WAIT, &no_sleep).unwrap().unwrap();
        paste_text(&f, &f, "Hallo", g.previous.clone(), Duration::from_secs(1), &no_sleep).unwrap();
        assert_eq!((f.picture(), f.get()), (Some(vec![1, 2, 3]), None));
        let empty = Fake::new(None, None);
        paste_text(&empty, &empty, "dictated", None, Duration::from_secs(1), &no_sleep).unwrap();
        assert_eq!(empty.get(), None, "the dictated text does not stay behind");
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
