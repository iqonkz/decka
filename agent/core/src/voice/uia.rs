//! UI Automation (T26.2): the selected text of the focused element, read through `TextPattern.GetSelection`. The clipboard is
//! not touched, which is why it is tried before `Ctrl+C`. Many programs have no text pattern: `None` then, and the caller
//! falls back to the clipboard.

use std::time::Duration;

/// UIA calls go to the other program and can hang with it: the answer is waited for this long.
pub const UIA_TIMEOUT: Duration = Duration::from_millis(1_200);
/// More than this is never read (the callers refuse a longer selection anyway).
const MAX_CHARS: i32 = 7_000;

/// The selected text of the focused element, or `None` (no selection, no text pattern, an error or a timeout).
pub fn selected_text() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_selection());
    });
    rx.recv_timeout(UIA_TIMEOUT).ok().flatten().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

#[cfg(windows)]
fn read_selection() -> Option<String> {
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId};

    // SAFETY: COM calls on a thread of their own; every COM object is dropped inside `read` before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let read = || -> Option<String> {
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let element = automation.GetFocusedElement().ok()?;
            let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
            let ranges = pattern.GetSelection().ok()?;
            let mut text = String::new();
            for i in 0..ranges.Length().ok()? {
                let part = ranges.GetElement(i).ok()?.GetText(MAX_CHARS).ok()?;
                text.push_str(&part.to_string());
                if text.chars().count() > MAX_CHARS as usize {
                    break;
                }
            }
            Some(text)
        };
        let out = read();
        if initialised {
            CoUninitialize();
        }
        out
    }
}

#[cfg(not(windows))]
fn read_selection() -> Option<String> {
    None
}

/// The whole text of the focused element (its value, else the document of its text pattern), to see whether a paste
/// arrived (T32.H.1). `None`: the program does not tell. Never logged.
pub fn focused_text() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if tx.send(read_focused()).is_err() {
            tracing::debug!("the text of the element came too late");
        }
    });
    rx.recv_timeout(UIA_TIMEOUT).ok().flatten()
}

/// More than this of a document is not looked through.
#[cfg(windows)]
const MAX_DOCUMENT: i32 = 200_000;

#[cfg(windows)]
fn read_focused() -> Option<String> {
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTextPattern, IUIAutomationValuePattern, UIA_TextPatternId, UIA_ValuePatternId};

    // SAFETY: COM calls on a thread of their own; every COM object is dropped inside `read` before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let read = || -> Option<String> {
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let element = automation.GetFocusedElement().ok()?;
            if let Ok(pattern) = element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                return pattern.DocumentRange().ok()?.GetText(MAX_DOCUMENT).ok().map(|t| t.to_string());
            }
            let value = element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).ok()?;
            value.CurrentValue().ok().map(|t| t.to_string())
        };
        let out = read();
        if initialised {
            CoUninitialize();
        }
        out
    }
}

#[cfg(not(windows))]
fn read_focused() -> Option<String> {
    None
}

/// The text of the element under the mouse cursor (its text pattern, else its name), for "read aloud" without a selection.
pub fn text_at_cursor() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_at_cursor());
    });
    rx.recv_timeout(UIA_TIMEOUT).ok().flatten().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

#[cfg(windows)]
fn read_at_cursor() -> Option<String> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    // SAFETY: COM calls on a thread of their own; every COM object is dropped inside `read` before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let read = || -> Option<String> {
            let mut p = POINT::default();
            GetCursorPos(&mut p).ok()?;
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let element = automation.ElementFromPoint(p).ok()?;
            if let Ok(pattern) = element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                if let Ok(t) = pattern.DocumentRange().and_then(|r| r.GetText(MAX_CHARS)) {
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
            element.CurrentName().ok().map(|s| s.to_string())
        };
        let out = read();
        if initialised {
            CoUninitialize();
        }
        out
    }
}

#[cfg(not(windows))]
fn read_at_cursor() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn reading_the_selection_of_whatever_has_the_focus_never_panics_or_hangs() {
        let started = std::time::Instant::now();
        let _ = super::selected_text();
        assert!(started.elapsed() < super::UIA_TIMEOUT + std::time::Duration::from_millis(500));
    }
}
