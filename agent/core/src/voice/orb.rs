//! T29.4: a dictation shows no island, only a small orb next to the text cursor. Here: when it is on show and what it says
//! (pure), where it goes (pure), and where the text cursor is (UI Automation, else the Win32 caret, else the mouse).

use std::time::{Duration, Instant};

/// The orb goes out this long after the text was typed.
pub const DONE_FOR: Duration = Duration::from_secs(1);
/// A bubble with what went wrong stays this long.
pub const ERROR_FOR: Duration = Duration::from_secs(3);
/// The gap between the caret and the orb, in logical pixels.
pub const GAP: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Listening,
    Thinking,
    Done,
    Error,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Listening => "listening",
            Phase::Thinking => "thinking",
            Phase::Done => "done",
            Phase::Error => "error",
        }
    }
}

/// What the orb says: the English text and, for a fixed phrase, its i18n key.
#[derive(Debug, Clone, PartialEq)]
pub struct Bubble {
    pub text: String,
    pub key: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub phase: Phase,
    pub bubble: Option<Bubble>,
    /// Where the caret ends (physical screen pixels); `None` until it is known.
    pub anchor: Option<(i32, i32)>,
}

/// The state of the orb, fed with what the voice host publishes.
#[derive(Debug, Default)]
pub struct Orb {
    view: Option<View>,
    until: Option<Instant>,
}

impl Orb {
    /// One published state of a request. `dictating`: the request types what it hears. `bubble`: the text of a result that
    /// the user has to read (a failure, or "Copied: paste with Ctrl+V"). Returns `true` when the orb has just appeared.
    pub fn on_state(&mut self, state: &str, dictating: bool, bubble: Option<Bubble>, now: Instant) -> bool {
        let running = match state {
            "listening" => Some(Phase::Listening),
            "loading" | "recognizing" | "thinking" => Some(Phase::Thinking),
            _ => None,
        };
        if let Some(phase) = running {
            if !dictating {
                // Another kind of request: the island takes over.
                self.clear();
                return false;
            }
            let appeared = self.view.is_none() || self.until.is_some();
            let anchor = if appeared { None } else { self.view.as_ref().and_then(|v| v.anchor) };
            self.view = Some(View { phase, bubble: None, anchor });
            self.until = None;
            return appeared;
        }
        // The end of a request counts only for an orb that is waiting for it.
        if self.until.is_some() {
            if state != "idle" {
                // A note that is not part of the dictation (a reminder, an answer).
                self.clear();
            }
            return false;
        }
        let Some(view) = self.view.as_mut() else { return false };
        match (state, bubble) {
            ("done", None) => {
                view.phase = Phase::Done;
                self.until = Some(now + DONE_FOR);
            }
            ("done" | "error", bubble) => {
                view.phase = Phase::Error;
                view.bubble = bubble;
                self.until = Some(now + ERROR_FOR);
            }
            // Cancelled.
            _ => self.clear(),
        }
        false
    }

    pub fn set_anchor(&mut self, anchor: (i32, i32)) {
        if let Some(v) = self.view.as_mut() {
            v.anchor = Some(anchor);
        }
    }

    pub fn clear(&mut self) {
        self.view = None;
        self.until = None;
    }

    pub fn view(&mut self, now: Instant) -> Option<View> {
        if self.until.is_some_and(|t| now >= t) {
            self.clear();
        }
        self.view.clone()
    }
}

/// The top-left corner of the orb window: right and below the caret by `gap`, kept inside the screen (`left, top, right,
/// bottom`); when there is no room below, above the line.
pub fn origin(anchor: (i32, i32), size: (i32, i32), gap: i32, screen: (i32, i32, i32, i32)) -> (i32, i32) {
    let (left, top, right, bottom) = screen;
    let x = (anchor.0 + gap).min(right - size.0).max(left);
    let below = anchor.1 + gap;
    let y = if below + size.1 <= bottom { below } else { anchor.1 - gap - size.1 - 24 };
    (x, y.max(top))
}

/// Where the text cursor ends, in physical screen pixels: the caret of UI Automation, else the Win32 caret of the focused
/// window, else the mouse pointer. Blocks for at most about a second.
pub fn caret_anchor() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(win::automation_caret());
        });
        rx.recv_timeout(Duration::from_millis(800)).ok().flatten().or_else(win::window_caret).or_else(win::mouse)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
mod win {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, SAFEARRAY};
    use windows::Win32::System::Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTextPattern2, IUIAutomationTextRange, TextUnit_Character, UIA_TextPattern2Id};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO};

    /// The first rectangle of a range: left, top, width, height.
    fn first_rect(range: &IUIAutomationTextRange) -> Option<[f64; 4]> {
        // SAFETY: the array belongs to us: read within its bounds and destroyed here.
        unsafe {
            let array: *mut SAFEARRAY = range.GetBoundingRectangles().ok()?;
            if array.is_null() {
                return None;
            }
            let mut out = [0f64; 4];
            let (low, high) = (SafeArrayGetLBound(array, 1).ok(), SafeArrayGetUBound(array, 1).ok());
            let enough = low.zip(high).is_some_and(|(l, h)| h - l >= 3);
            if enough {
                for (i, slot) in out.iter_mut().enumerate() {
                    let index = low.unwrap_or(0) + i as i32;
                    let _ = SafeArrayGetElement(array, &index, (slot as *mut f64).cast());
                }
            }
            let _ = SafeArrayDestroy(array);
            (enough && out[3] > 0.0).then_some(out)
        }
    }

    pub fn automation_caret() -> Option<(i32, i32)> {
        // SAFETY: COM calls on a thread of their own; every object is dropped inside `read` before `CoUninitialize`.
        unsafe {
            let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let read = || -> Option<(i32, i32)> {
                let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
                let element = automation.GetFocusedElement().ok()?;
                let pattern: IUIAutomationTextPattern2 = element.GetCurrentPatternAs(UIA_TextPattern2Id).ok()?;
                let mut active = BOOL(0);
                let range = pattern.GetCaretRange(&mut active).ok()?;
                // An empty range often has no rectangle: the character at the caret has one.
                let (rect, at_start) = match first_rect(&range) {
                    Some(r) => (r, false),
                    None => {
                        let wide = range.Clone().ok()?;
                        wide.ExpandToEnclosingUnit(TextUnit_Character).ok()?;
                        (first_rect(&wide)?, true)
                    }
                };
                let x = if at_start { rect[0] } else { rect[0] + rect[2] };
                Some((x.round() as i32, (rect[1] + rect[3]).round() as i32))
            };
            let out = read();
            if initialised {
                CoUninitialize();
            }
            out
        }
    }

    pub fn window_caret() -> Option<(i32, i32)> {
        // SAFETY: queries about the foreground window; the structure lives for the call.
        unsafe {
            let window = GetForegroundWindow();
            let thread = GetWindowThreadProcessId(window, None);
            let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
            GetGUIThreadInfo(thread, &mut info).ok()?;
            let RECT { right, bottom, top, .. } = info.rcCaret;
            if info.hwndCaret.0.is_null() || bottom <= top {
                return None;
            }
            let mut p = POINT { x: right, y: bottom };
            ClientToScreen(info.hwndCaret, &mut p).as_bool().then_some((p.x, p.y))
        }
    }

    pub fn mouse() -> Option<(i32, i32)> {
        let mut p = POINT::default();
        // SAFETY: `p` lives for the call.
        unsafe { GetCursorPos(&mut p) }.ok().map(|_| (p.x, p.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copied() -> Bubble {
        Bubble { text: "Copied: paste with Ctrl+V".into(), key: Some("voice.reply.copied_paste") }
    }

    #[test]
    fn a_dictation_shows_the_orb_and_it_goes_out_a_second_after_the_end() {
        let (mut orb, t) = (Orb::default(), Instant::now());
        assert!(orb.on_state("listening", true, None, t), "it appears: look for the caret");
        orb.set_anchor((100, 200));
        assert!(!orb.on_state("recognizing", true, None, t));
        assert_eq!(orb.view(t).unwrap().anchor, Some((100, 200)));
        orb.on_state("done", false, None, t);
        assert_eq!(orb.view(t).unwrap().phase, Phase::Done);
        assert!(orb.view(t + DONE_FOR - Duration::from_millis(1)).is_some());
        orb.on_state("idle", false, None, t);
        assert!(orb.view(t + DONE_FOR).is_none());
    }

    #[test]
    fn a_failure_is_a_bubble_for_three_seconds() {
        let (mut orb, t) = (Orb::default(), Instant::now());
        orb.on_state("listening", true, None, t);
        orb.on_state("done", false, Some(copied()), t);
        let v = orb.view(t + Duration::from_secs(2)).unwrap();
        assert_eq!((v.phase, v.bubble), (Phase::Error, Some(copied())));
        // The pipeline goes idle after 4 s; the orb is gone by its own clock before.
        assert!(orb.view(t + ERROR_FOR).is_none());
    }

    #[test]
    fn ordinary_requests_and_cancellations_show_no_orb() {
        let (mut orb, t) = (Orb::default(), Instant::now());
        assert!(!orb.on_state("listening", false, None, t));
        orb.on_state("done", false, Some(copied()), t);
        assert!(orb.view(t).is_none(), "the island answers a command");
        orb.on_state("listening", true, None, t);
        orb.on_state("idle", false, None, t);
        assert!(orb.view(t).is_none(), "cancelled");
        // A command right after a dictation takes the place back.
        orb.on_state("listening", true, None, t);
        orb.on_state("done", false, None, t);
        orb.on_state("listening", false, None, t);
        assert!(orb.view(t).is_none());
    }

    #[test]
    fn the_orb_sits_right_and_below_the_caret_and_stays_on_the_screen() {
        let screen = (0, 0, 1920, 1080);
        assert_eq!(origin((100, 200), (300, 40), 8, screen), (108, 208));
        assert_eq!(origin((1900, 200), (300, 40), 8, screen), (1620, 208), "kept inside on the right");
        assert_eq!(origin((100, 1070), (300, 40), 8, screen), (108, 998), "no room below: above the line");
        assert_eq!(origin((-1900, 10), (300, 40), 8, (-1920, 0, 0, 1080)), (-1892, 18), "a monitor to the left");
    }
}
