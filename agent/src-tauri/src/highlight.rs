//! The window of the highlight (T27.2): one transparent, click-through, always-on-top window over the monitor that holds the element.
//! The page (`index.html?highlight=1`) draws a frame and a bubble from the event `highlight`; the decision when it ends is
//! `decka_core::voice::highlight` (tested there); this file feeds it the focused window and the clicks.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use decka_core::overlay;
use decka_core::voice::highlight::{Highlight, HighlightSink, Highlighter, Watch};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Listener, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::island::{on_hwnd, set_resting};

const LABEL: &str = "highlight";
/// T29.P: the window is closed this long after the last highlight ended, and made again when it is needed.
const CLOSE_AFTER: Duration = Duration::from_secs(30);

pub struct WindowSink {
    app: AppHandle,
    /// What is highlighted now: a window that has just been made asks for it when its page listens.
    current: Arc<Mutex<Option<Value>>>,
    /// Counts shows and clears: a pending close counts only while nothing else happened.
    turn: Arc<AtomicU64>,
    /// The window was told to close and may still be there for a moment.
    closing: Arc<AtomicBool>,
}

impl WindowSink {
    /// Cancels a pending close and waits for one that has begun.
    fn wake(&self) {
        {
            let _held = self.current.lock().unwrap();
            self.turn.fetch_add(1, Ordering::SeqCst);
        }
        if self.closing.swap(false, Ordering::SeqCst) {
            for _ in 0..100 {
                if self.app.get_webview_window(LABEL).is_none() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn close_later(&self, turn: u64) {
        let (app, current, turns, closing) = (self.app.clone(), self.current.clone(), self.turn.clone(), self.closing.clone());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(CLOSE_AFTER).await;
            // Held while the close is asked for, so that a new highlight sees it.
            let held = current.lock().unwrap();
            if turns.load(Ordering::SeqCst) == turn && held.is_none() {
                if let Some(w) = app.get_webview_window(LABEL) {
                    closing.store(true, Ordering::SeqCst);
                    if let Err(e) = w.destroy() {
                        tracing::warn!("cannot close the highlight window: {e}");
                    }
                }
            }
        });
    }
}

fn window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Some(w);
    }
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?highlight=1".into()))
        .title("Decka")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|e| tracing::error!("cannot create the highlight window: {e}"))
        .ok()?;
    // T29.1: never the active window, and every click goes to what is under it.
    on_hwnd(&w, |hwnd| {
        overlay::guard(hwnd);
        overlay::set_click_through(hwnd, true);
    });
    Some(w)
}

impl HighlightSink for WindowSink {
    fn show(&self, h: Highlight) {
        self.wake();
        let Some(w) = window(&self.app) else { return };
        // The monitor that holds the centre of the element.
        let (cx, cy) = (h.rect.x + h.rect.width / 2, h.rect.y + h.rect.height / 2);
        let monitor = self.app.monitor_from_point(f64::from(cx), f64::from(cy)).ok().flatten().or_else(|| self.app.primary_monitor().ok().flatten());
        let Some(m) = monitor else { return };
        let (origin, size, scale) = (*m.position(), *m.size(), m.scale_factor());
        let _ = w.set_position(PhysicalPosition::new(origin.x, origin.y));
        let _ = w.set_size(PhysicalSize::new(size.width, size.height));
        // The page works in CSS pixels from the corner of its own window.
        let rel = |v: i32, o: i32| f64::from(v - o) / scale;
        let rect = json!({ "x": rel(h.rect.x, origin.x), "y": rel(h.rect.y, origin.y), "width": f64::from(h.rect.width) / scale, "height": f64::from(h.rect.height) / scale });
        let event = json!({ "rect": rect, "text": h.text, "step": h.step, "total": h.total });
        *self.current.lock().unwrap() = Some(event.clone());
        set_resting(&w, false);
        on_hwnd(&w, overlay::show);
        let _ = w.emit("highlight", event);
    }

    fn clear(&self) {
        let turn = {
            let mut current = self.current.lock().unwrap();
            *current = None;
            self.turn.fetch_add(1, Ordering::SeqCst) + 1
        };
        if let Some(w) = self.app.get_webview_window(LABEL) {
            let _ = w.emit("highlight_clear", ());
            on_hwnd(&w, overlay::hide);
            set_resting(&w, true);
            self.close_later(turn);
        }
    }
}

/// The events of a guide go to the island (`guide_step`), which shows the goal, the step and the buttons.
pub struct GuideEmitter(pub AppHandle);

impl decka_core::voice::screenrun::GuideSink for GuideEmitter {
    fn event(&self, e: decka_core::voice::screenrun::GuideEvent) {
        if let Some(shared) = self.0.try_state::<Arc<crate::island::Shared>>() {
            shared.set_guide(e.active);
        }
        let _ = self.0.emit_to("island", "guide_step", e);
    }
}

/// The highlighter and its watcher: a few looks a second at the focused window and at the left mouse button.
pub fn setup(app: &AppHandle) -> Highlighter {
    let current: Arc<Mutex<Option<Value>>> = Arc::default();
    // The first event of a new window is sent before its page exists: the page says when it listens.
    let (again, emitter) = (current.clone(), app.clone());
    app.listen_any("highlight_ready", move |_| {
        let event = again.lock().unwrap().clone();
        if let Some(event) = event {
            let _ = emitter.emit("highlight", event);
        }
    });
    let highlighter = Highlighter::new(Arc::new(WindowSink { app: app.clone(), current, turn: Arc::default(), closing: Arc::default() }));
    app.manage(highlighter.clone());
    let watcher = highlighter.clone();
    let started = std::time::Instant::now();
    std::thread::Builder::new()
        .name("decka-highlight-watch".into())
        .spawn(move || {
            let mut was_down = false;
            loop {
                std::thread::sleep(Duration::from_millis(100));
                if watcher.current().is_none() {
                    was_down = false;
                    continue;
                }
                let (down, at) = decka_core::voice::winops::left_button_and_cursor();
                let click = (down && !was_down).then_some(at);
                was_down = down;
                watcher.watch(&Watch { now_ms: started.elapsed().as_millis() as u64, foreground: decka_core::voice::winops::foreground_handle(), click, stop: false });
            }
        })
        .expect("spawn highlight watcher");
    highlighter
}
