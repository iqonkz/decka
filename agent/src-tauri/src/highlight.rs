//! The window of the highlight (T27.2): one transparent, click-through, always-on-top window over the monitor that holds the element.
//! The page (`index.html?highlight=1`) draws a frame and a bubble from the event `highlight`; the decision when it ends is
//! `decka_core::voice::highlight` (tested there); this file feeds it the focused window and the clicks.

use std::sync::Arc;
use std::time::Duration;

use decka_core::voice::highlight::{Highlight, HighlightSink, Highlighter, Watch};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "highlight";

pub struct WindowSink {
    app: AppHandle,
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
    let _ = w.set_ignore_cursor_events(true);
    Some(w)
}

impl HighlightSink for WindowSink {
    fn show(&self, h: Highlight) {
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
        let _ = w.show();
        let _ = w.emit("highlight", json!({ "rect": rect, "text": h.text, "step": h.step, "total": h.total }));
    }

    fn clear(&self) {
        if let Some(w) = self.app.get_webview_window(LABEL) {
            let _ = w.emit("highlight_clear", ());
            let _ = w.hide();
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
    let highlighter = Highlighter::new(Arc::new(WindowSink { app: app.clone() }));
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
