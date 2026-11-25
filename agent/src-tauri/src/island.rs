//! The window of the island (T26.4): one transparent, always-on-top window at the top centre of the main monitor, in place of
//! the pill of the voice and the strip of the lyrics. What it shows is decided by `decka_core::island` (tested there); this file
//! samples the sources (the voice host, the sections of the agent), polls the cursor, lets clicks through everywhere except on
//! the visible panel, and sends the event `island_state` to the page.
//!
//! Not on Windows 10 / 11 alike: the glass of the panel is drawn by the page (the opacity of the theme surface); an acrylic
//! backdrop of the window would fill its whole rectangle, not the rounded panel.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use decka_core::island::{voice_view, Inputs, Machine, Mode, VoiceInput, POLL_MS};
use decka_core::lyrics_overlay::{foreground_is_fullscreen, synced_lines, two_lines, visible, Inputs as LyricsInputs};
use decka_core::model::MediaInfo;
use decka_core::voice::host::Host;
use decka_core::Agent;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use serde::Serialize;


#[derive(Serialize, Clone, PartialEq)]
struct Lines {
    current: Option<String>,
    next: Option<String>,
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


const LABEL: &str = "island";
/// Room for the largest panel (440 x 420) and the shadow around it, logical pixels.
const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 460.0;
/// From the top edge of the main monitor.
const TOP_GAP: f64 = 12.0;
/// The page needs a moment to start listening: the event is repeated for this long after the window appears.
const FRESH: Duration = Duration::from_millis(2_000);

/// The visible panel, in CSS pixels from the top-left corner of the window (IPC `island_rect`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// What the loop shares with the commands and the voice hook.
#[derive(Default)]
pub struct Shared {
    rect: Mutex<Option<Rect>>,
    /// The last payload of the voice host for the PC window (`None`: idle).
    voice: Mutex<Option<Value>>,
}

impl Shared {
    pub fn set_rect(&self, rect: Rect) {
        *self.rect.lock().unwrap() = Some(rect);
    }

    pub fn set_voice(&self, payload: Option<Value>) {
        *self.voice.lock().unwrap() = payload;
    }

}

/// The sections of the agent the lyrics part reads, sampled four times a second.
struct Sample {
    visible: bool,
    mode: decka_core::lyrics_overlay::PcOverlay,
    track: Option<String>,
    fullscreen: bool,
    lyrics: Option<Value>,
}

impl Default for Sample {
    fn default() -> Sample {
        Sample { visible: false, mode: decka_core::lyrics_overlay::PcOverlay::Auto, track: None, fullscreen: false, lyrics: None }
    }
}

fn sample(agent: &Agent, paused_since: &mut Option<Instant>) -> Sample {
    let mode = agent.lyrics_overlay_mode();
    let store = agent.store();
    let media: Option<MediaInfo> = serde_json::from_value(store.get("media")).ok();
    let lyrics = store.get("lyrics");
    let playing = media.as_ref().is_some_and(|m| m.playing);
    if playing {
        *paused_since = None;
    } else if paused_since.is_none() {
        *paused_since = Some(Instant::now());
    }
    let lines = media.as_ref().and_then(|m| lines_now(m, &lyrics, now_ms()));
    let wanted = mode != decka_core::lyrics_overlay::PcOverlay::Off && agent.lyrics_enabled() && !agent.lyrics_hidden() && lines.is_some();
    let fullscreen = wanted && foreground_is_fullscreen();
    let inputs = LyricsInputs {
        mode,
        lyrics_enabled: agent.lyrics_enabled(),
        synced: lines.is_some(),
        playing,
        paused_for_ms: paused_since.map_or(0, |t| t.elapsed().as_millis() as u64),
        fullscreen,
        hidden: agent.lyrics_hidden(),
    };
    let shown = wanted && visible(&inputs);
    // The same song is the same track: its key is the one the lyrics section carries.
    let track = lyrics.get("trackKey").and_then(Value::as_str).map(str::to_string).or_else(|| media.as_ref().map(|m| format!("{}|{}", m.title, m.artist.clone().unwrap_or_default())));
    let current = lines.as_ref().map(|l| json!({ "playing": playing, "current": l.current, "next": l.next }));
    Sample { visible: shown, mode, track, fullscreen, lyrics: current }
}

fn create(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?island=1".into()))
        .title("Decka")
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
        .map_err(|e| tracing::error!("cannot create the island: {e}"))
        .ok()?;
    // Clicks go through until the cursor is on the panel.
    let _ = window.set_ignore_cursor_events(true);
    Some(window)
}

/// Top centre of the main monitor, `TOP_GAP` below its edge.
fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let Some(m) = app.primary_monitor().ok().flatten() else { return };
    let scale = m.scale_factor();
    let width = WIDTH.min(m.size().width as f64 / scale - 16.0).max(320.0);
    let _ = window.set_size(LogicalSize::new(width, HEIGHT));
    let x = m.position().x as f64 + (m.size().width as f64 - width * scale) / 2.0;
    let y = m.position().y as f64 + TOP_GAP * scale;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// Whether the cursor is on the visible panel.
fn cursor_on_panel(app: &AppHandle, window: &tauri::WebviewWindow, rect: Rect) -> bool {
    let (Ok(cursor), Ok(origin)) = (app.cursor_position(), window.outer_position()) else { return false };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (left, top) = (origin.x as f64 + rect.x * scale, origin.y as f64 + rect.y * scale);
    cursor.x >= left && cursor.x < left + rect.width * scale && cursor.y >= top && cursor.y < top + rect.height * scale
}

/// The loop of the island. Called once the page of the island exists (T26.R).
#[allow(dead_code)]
pub fn setup(app: &AppHandle, agent: &Agent) -> Arc<Shared> {
    let shared = Arc::new(Shared::default());
    app.manage(shared.clone());
    let (app, agent, loop_shared) = (app.clone(), agent.clone(), shared.clone());
    tauri::async_runtime::spawn(async move {
        let mut machine = Machine::default();
        let (mut shown, mut ignoring, mut paused_since) = (false, true, None);
        let (mut last_event, mut fresh_until): (Option<Value>, Option<Instant>) = (None, None);
        let (mut tick, mut sampled) = (0u64, Sample::default());
        let started = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
            tick += 1;
            if tick % 4 == 1 {
                sampled = sample(&agent, &mut paused_since);
            }
            let rect = *loop_shared.rect.lock().unwrap();
            let window = app.get_webview_window(LABEL);
            let on_panel = match (&window, rect) {
                (Some(w), Some(r)) if shown => cursor_on_panel(&app, w, r),
                _ => false,
            };

            let host = app.try_state::<Host>();
            let slow = host.as_ref().and_then(|h| h.slow_progress());
            let (voice, voice_json) = {
                let payload = loop_shared.voice.lock().unwrap().clone();
                let chips = host.as_ref().map(|h| h.context_chips()).unwrap_or_default();
                voice_view(payload.as_ref(), slow, &chips)
            };
            let mode = machine.step(&Inputs {
                now_ms: started.elapsed().as_millis() as u64,
                voice,
                hovered: on_panel,
                lyrics_visible: sampled.visible,
                lyrics_mode: sampled.mode,
                track: sampled.track.as_deref(),
                fullscreen: sampled.fullscreen,
            });

            if mode == Mode::Hidden {
                if shown {
                    if let Some(w) = &window {
                        let _ = w.hide();
                    }
                    shown = false;
                    last_event = None;
                }
                continue;
            }
            let window = match window {
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
                ignoring = true;
                fresh_until = Some(Instant::now() + FRESH);
            }
            // Clicks reach the panel only while the cursor is on it.
            if ignoring == on_panel {
                ignoring = !on_panel;
                let _ = window.set_ignore_cursor_events(ignoring);
            }
            let event = json!({
                "mode": mode,
                "lyrics": if voice == VoiceInput::Idle || sampled.visible { sampled.lyrics.clone() } else { None },
                "voice": voice_json,
            });
            let repeat = fresh_until.is_some_and(|t| Instant::now() < t) && tick % 5 == 0;
            if last_event.as_ref() != Some(&event) || repeat {
                let _ = window.emit("island_state", event.clone());
                last_event = Some(event);
            }
        }
    });
    shared
}

/// The voice host shows a request on the PC window: the island takes it in place of the pill. `None`: the request is over.
#[allow(dead_code)]
pub fn on_voice(app: &AppHandle, payload: Option<Value>) {
    if let Some(shared) = app.try_state::<Arc<Shared>>() {
        shared.set_voice(payload);
    }
}
