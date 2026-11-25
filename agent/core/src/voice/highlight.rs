//! The highlight of an element on the screen (T27.2): a frame around `rect` and a bubble with the text of the step. This file is the
//! decision - when a highlight ends - and the sink the tools talk to; the transparent click-through window that draws it belongs to the
//! application (`src-tauri/src/highlight.rs`).

use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::ui_snapshot::Rect;

/// A highlight stays this long when nothing ends it earlier.
pub const HIGHLIGHT_MS: u64 = 30_000;

/// What the highlight window shows: the event `highlight { rect, text, step?, total? }` (physical screen pixels).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Highlight {
    pub rect: Rect,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}

/// Where the tools put a highlight. The application draws it; the tests record it.
pub trait HighlightSink: Send + Sync {
    fn show(&self, highlight: Highlight);
    /// `highlight_clear`.
    fn clear(&self);
}

/// A sink that does nothing (no window yet).
pub struct NoSink;

impl HighlightSink for NoSink {
    fn show(&self, _: Highlight) {}
    fn clear(&self) {}
}

/// A sink that keeps what it was told (tests).
#[derive(Default)]
pub struct Recorder {
    pub shown: Mutex<Vec<Highlight>>,
    pub cleared: Mutex<u32>,
}

impl HighlightSink for Recorder {
    fn show(&self, h: Highlight) {
        self.shown.lock().unwrap().push(h);
    }
    fn clear(&self) {
        *self.cleared.lock().unwrap() += 1;
    }
}

/// What the world looks like to the highlight while it is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watch {
    pub now_ms: u64,
    /// The window that has the focus (any stable number: its handle).
    pub foreground: u64,
    /// The left button went down at this point since the last look.
    pub click: Option<(i32, i32)>,
    /// "Stop" was said.
    pub stop: bool,
}

/// One highlight that is up: it ends on a click into it, a change of the focused window, "stop" or after `HIGHLIGHT_MS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Active {
    pub highlight: Highlight,
    shown_at: u64,
    foreground: u64,
}

impl Active {
    pub fn new(highlight: Highlight, now_ms: u64, foreground: u64) -> Active {
        Active { highlight, shown_at: now_ms, foreground }
    }

    /// Whether the highlight is over.
    pub fn ended(&self, w: &Watch) -> bool {
        let r = self.highlight.rect;
        let inside = w.click.is_some_and(|(x, y)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height);
        w.stop || inside || w.foreground != self.foreground || w.now_ms.saturating_sub(self.shown_at) >= HIGHLIGHT_MS
    }
}

/// The current highlight and its sink, shared by the tools and the watcher.
#[derive(Clone)]
pub struct Highlighter {
    sink: Arc<dyn HighlightSink>,
    active: Arc<Mutex<Option<Active>>>,
}

impl Highlighter {
    pub fn new(sink: Arc<dyn HighlightSink>) -> Highlighter {
        Highlighter { sink, active: Arc::new(Mutex::new(None)) }
    }

    pub fn show(&self, highlight: Highlight, now_ms: u64, foreground: u64) {
        *self.active.lock().unwrap() = Some(Active::new(highlight.clone(), now_ms, foreground));
        self.sink.show(highlight);
    }

    pub fn clear(&self) {
        if self.active.lock().unwrap().take().is_some() {
            self.sink.clear();
        }
    }

    /// The rectangle of the last highlight still up ("this" in `ui_act`).
    pub fn current(&self) -> Option<Highlight> {
        self.active.lock().unwrap().as_ref().map(|a| a.highlight.clone())
    }

    /// Called by the watcher a few times a second; ends the highlight when the rules say so. `true` when it ended now.
    pub fn watch(&self, w: &Watch) -> bool {
        let ended = self.active.lock().unwrap().as_ref().is_some_and(|a| a.ended(w));
        if ended {
            self.clear();
        }
        ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h() -> Highlight {
        Highlight { rect: Rect { x: 100, y: 100, width: 50, height: 20 }, text: "Press Save".into(), step: Some(1), total: None }
    }

    fn watch(now_ms: u64) -> Watch {
        Watch { now_ms, foreground: 7, click: None, stop: false }
    }

    fn up() -> (Highlighter, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        let hl = Highlighter::new(rec.clone());
        hl.show(h(), 1_000, 7);
        (hl, rec)
    }

    #[test]
    fn a_highlight_is_shown_and_remembered_for_this() {
        let (hl, rec) = up();
        assert_eq!(rec.shown.lock().unwrap().len(), 1);
        assert_eq!(hl.current().unwrap().text, "Press Save");
    }

    #[test]
    fn it_stays_while_nothing_happens_and_ends_after_thirty_seconds() {
        let (hl, rec) = up();
        assert!(!hl.watch(&watch(20_000)));
        assert!(hl.current().is_some());
        assert!(hl.watch(&watch(31_000)));
        assert_eq!((*rec.cleared.lock().unwrap(), hl.current()), (1, None));
    }

    #[test]
    fn a_click_inside_ends_it_a_click_outside_does_not() {
        let (hl, _) = up();
        assert!(!hl.watch(&Watch { click: Some((10, 10)), ..watch(2_000) }));
        assert!(hl.watch(&Watch { click: Some((120, 110)), ..watch(2_100) }));
    }

    #[test]
    fn another_window_or_stop_ends_it() {
        let (hl, _) = up();
        assert!(hl.watch(&Watch { foreground: 8, ..watch(2_000) }));
        let (hl, _) = up();
        assert!(hl.watch(&Watch { stop: true, ..watch(2_000) }));
    }

    #[test]
    fn clearing_twice_tells_the_window_once() {
        let (hl, rec) = up();
        hl.clear();
        hl.clear();
        assert_eq!(*rec.cleared.lock().unwrap(), 1);
    }
}
