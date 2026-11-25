//! The window of the words on the PC (0.30, DESIGN.md 4.17): a transparent, click-through, always-on-top strip at the bottom
//! of the main monitor that never takes the focus. When it is shown is decided by `decka_core::lyrics_overlay` (tested there);
//! this file only samples the sections of the agent, moves the window and sends the two lines to its page.

use std::time::{Duration, Instant};

use decka_core::lyrics_overlay::{foreground_is_fullscreen, synced_lines, two_lines, visible, Inputs, PcOverlay};
use decka_core::model::MediaInfo;
use decka_core::Agent;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "lyrics";
const WIDTH: f64 = 900.0;
const HEIGHT: f64 = 104.0;
/// Above the taskbar (the work area already ends there), logical pixels.
const BOTTOM_GAP: f64 = 48.0;
const TICK: Duration = Duration::from_millis(250);

#[derive(Serialize, Clone, PartialEq)]
struct Lines {
    current: Option<String>,
    next: Option<String>,
}

fn create(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?lyrics=1".into()))
        .title("Decka lyrics")
        .inner_size(WIDTH, HEIGHT)
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
        .map_err(|e| tracing::error!("cannot create the lyrics window: {e}"))
        .ok()?;
    // Clicks and keys go to the program below.
    let _ = window.set_ignore_cursor_events(true);
    Some(window)
}

/// Bottom centre of the primary monitor, `BOTTOM_GAP` above the work area (the taskbar is outside of it).
fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let Some(m) = app.primary_monitor().ok().flatten() else { return };
    let scale = m.scale_factor();
    let area = m.work_area();
    let width = WIDTH.min(area.size.width as f64 / scale - 32.0).max(320.0);
    let _ = window.set_size(LogicalSize::new(width, HEIGHT));
    let x = area.position.x as f64 + (area.size.width as f64 - width * scale) / 2.0;
    let y = area.position.y as f64 + area.size.height as f64 - (HEIGHT + BOTTOM_GAP) * scale;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// The two lines for the moment `now`: the song position (plus the time since it was reported, while it plays) and the shift.
fn lines_now(media: &MediaInfo, lyrics: &Value, now: u64) -> Option<Lines> {
    let lines = synced_lines(lyrics)?;
    let offset = lyrics.get("offsetMs").and_then(Value::as_i64).unwrap_or(0);
    let base = media.position_sec.unwrap_or(0) as i64 * 1000;
    let moved = if media.playing { now.saturating_sub(media.position_at) as i64 } else { 0 };
    let (current, next) = two_lines(&lines, base + moved + offset);
    Some(Lines { current, next })
}

pub fn setup(app: &AppHandle, agent: &Agent) {
    let app = app.clone();
    let agent = agent.clone();
    tauri::async_runtime::spawn(async move {
        let mut paused_since: Option<Instant> = None;
        let mut shown = false;
        let mut last: Option<Lines> = None;
        loop {
            tokio::time::sleep(TICK).await;
            let mode = agent.lyrics_overlay_mode();
            let store = agent.store();
            let media: Option<MediaInfo> = serde_json::from_value(store.get("media")).ok();
            let lyrics = store.get("lyrics");

            let playing = media.as_ref().is_some_and(|m| m.playing);
            if playing {
                paused_since = None;
            } else if paused_since.is_none() {
                paused_since = Some(Instant::now());
            }
            let lines = media.as_ref().and_then(|m| lines_now(m, &lyrics, now_ms()));
            // Only ask Windows about a full screen when it matters.
            let wanted = mode != PcOverlay::Off && agent.lyrics_enabled() && lines.is_some();
            let inputs = Inputs {
                mode,
                lyrics_enabled: agent.lyrics_enabled(),
                synced: lines.is_some(),
                playing,
                paused_for_ms: paused_since.map_or(0, |t| t.elapsed().as_millis() as u64),
                fullscreen: wanted && foreground_is_fullscreen(),
            };
            let show = wanted && visible(&inputs);

            if show {
                let window = match app.get_webview_window(LABEL) {
                    Some(w) => w,
                    None => match create(&app) {
                        Some(w) => w,
                        None => continue,
                    },
                };
                if !shown {
                    place(&app, &window);
                    let _ = window.show();
                    shown = true;
                    last = None; // a window that was just shown needs the lines again
                }
                if last != lines {
                    if let Some(l) = &lines {
                        let _ = window.emit("lyrics-lines", l.clone());
                    }
                    last = lines;
                }
            } else if shown {
                if let Some(w) = app.get_webview_window(LABEL) {
                    let _ = w.hide();
                }
                shown = false;
            }
        }
    });
}
