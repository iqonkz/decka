//! The window of the island (T26.4): one transparent, always-on-top window at the top centre of the main monitor, in place of
//! the pill of the voice and the strip of the lyrics. What it shows is decided by `decka_core::island` (tested there); this file
//! samples the sources (the voice host, the sections of the agent), polls the cursor, lets clicks through everywhere except on
//! the visible panel, and sends the event `island_state` to the page.
//!
//! Not on Windows 10 / 11 alike: the glass of the panel is drawn by the page (the opacity of the theme surface); an acrylic
//! backdrop of the window would fill its whole rectangle, not the rounded panel.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use decka_core::island::{voice_view, Align, Inputs, Machine, Mode, Settings, VoiceInput, POLL_MS};
use decka_core::lyrics_overlay::{foreground_is_fullscreen, ms_until_first_line, previous_line, synced_lines, two_lines, visible, Inputs as LyricsInputs};
use decka_core::model::MediaInfo;
use decka_core::overlay;
use decka_core::voice::host::Host;
use decka_core::voice::orb;
use decka_core::Agent;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use serde::Serialize;


#[derive(Serialize, Clone, PartialEq)]
struct Lines {
    prev: Option<String>,
    current: Option<String>,
    next: Option<String>,
    /// Milliseconds until the first line of the song; `None` once it has started.
    first_in_ms: Option<u64>,
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
    let position = base + moved + offset;
    let (current, next) = two_lines(&lines, position);
    Some(Lines { prev: previous_line(&lines, position), current, next, first_in_ms: ms_until_first_line(&lines, position) })
}


const LABEL: &str = "island";
/// A paused song keeps the bar this long.
const PAUSE_LINGER_MS: u64 = 5_000;
/// The size of the bar, logical pixels wide (`island.css`).
const BAR_WIDTH: f64 = 140.0;
/// The window starts at this size and then follows the visible panel (`island_rect`) plus `PAD` on each side, logical pixels.
const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 460.0;
/// Room for the shadow around the panel; the page pads its host by the same amount (`IslandApp`).
pub const PAD: f64 = 24.0;
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
pub struct Shared {
    rect: Mutex<Option<Rect>>,
    /// The size the window was last given, logical pixels.
    size: Mutex<(f64, f64)>,
    /// The last payload of the voice host for the PC window (`None`: idle).
    voice: Mutex<Option<Value>>,
    /// A guide is running: the window stays up to show its card even when the island itself has nothing to say (T27.5).
    guide: std::sync::atomic::AtomicBool,
    /// Where the window stands now.
    spot: Mutex<Spot>,
}

/// The places of the window on the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Spot {
    /// The panel of a request: top centre, a little below the edge.
    #[default]
    Panel,
    /// T29.5: the words of a song: the panel touches the top edge.
    Words(Align),
    /// T29.5: the resting bar: a window of exactly the bar at the very top.
    Bar(Align, u32),
    /// T29.4: the orb of a dictation, at this caret.
    Orb((i32, i32)),
}

impl Default for Shared {
    fn default() -> Shared {
        Shared { rect: Mutex::default(), size: Mutex::new((WIDTH, HEIGHT)), voice: Mutex::default(), guide: Default::default(), spot: Mutex::default() }
    }
}

/// The window size for a panel of this size: the panel plus the shadow margin on every side.
pub fn window_size(panel_width: f64, panel_height: f64) -> (f64, f64) {
    (panel_width + 2.0 * PAD, panel_height + 2.0 * PAD)
}

impl Shared {
    pub fn set_rect(&self, rect: Rect) {
        *self.rect.lock().unwrap() = Some(rect);
    }

    pub fn size(&self) -> (f64, f64) {
        *self.size.lock().unwrap()
    }

    pub fn set_size(&self, size: (f64, f64)) {
        *self.size.lock().unwrap() = size;
    }

    pub fn set_guide(&self, on: bool) {
        self.guide.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn guide_on(&self) -> bool {
        self.guide.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn spot(&self) -> Spot {
        *self.spot.lock().unwrap()
    }

    pub fn set_voice(&self, payload: Option<Value>) {
        *self.voice.lock().unwrap() = payload;
    }

}

/// The sections of the agent the lyrics part reads, sampled four times a second.
struct Sample {
    visible: bool,
    mode: decka_core::lyrics_overlay::PcOverlay,
    fullscreen: bool,
    lyrics: Option<Value>,
    /// A song plays (or did a moment ago) and the island may say so.
    music: bool,
    settings: Settings,
    /// A song is playing (the media section), with or without a lyrics line.
    playing: bool,
    /// The output of the PC is muted (`audio.muted`).
    muted: bool,
    /// The track of the media section, for the lines before the first lyric line.
    track_info: Option<Value>,
}

impl Default for Sample {
    fn default() -> Sample {
        Sample { visible: false, mode: decka_core::lyrics_overlay::PcOverlay::Auto, fullscreen: false, lyrics: None, music: false, settings: Settings::default(), playing: false, muted: false, track_info: None }
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
    let allowed = mode != decka_core::lyrics_overlay::PcOverlay::Off && !agent.lyrics_hidden();
    let paused_ms = paused_since.map_or(0, |t| t.elapsed().as_millis() as u64);
    let music = allowed && media.as_ref().is_some_and(|m| !m.title.is_empty() && (m.playing || paused_ms < PAUSE_LINGER_MS));
    let fullscreen = (wanted || music) && foreground_is_fullscreen();
    let inputs = LyricsInputs {
        mode,
        lyrics_enabled: agent.lyrics_enabled(),
        synced: lines.is_some(),
        playing,
        paused_for_ms: paused_ms,
        fullscreen,
        hidden: agent.lyrics_hidden(),
    };
    let shown = wanted && visible(&inputs);
    // A song without lines still reports its lyrics state: three dots while the search runs, then just the track (T26.F.13).
    let status = decka_core::island::lyrics_status(lines.is_some(), agent.lyrics_enabled() && agent.lyrics_loading());
    let current = match &lines {
        Some(l) => Some(json!({ "playing": playing, "status": status, "prev": l.prev, "current": l.current, "next": l.next, "firstLineInMs": l.first_in_ms })),
        None if media.is_some() => Some(json!({ "playing": playing, "status": status })),
        None => None,
    };
    let muted = store.get("vars").get("audio.muted").and_then(Value::as_bool).unwrap_or(false);
    let song = media.as_ref().filter(|m| !m.title.is_empty()).map(|m| json!({ "title": m.title, "artist": m.artist }));
    Sample { visible: shown, mode, fullscreen, lyrics: current, music, settings: agent.island_settings(), playing, muted, track_info: song }
}

/// The glass of the panel (T26.F.3): `css` (the page draws a translucent surface) or `acrylic` (DWM backdrop cut to the rounded panel).
/// `DECKA_ISLAND_GLASS=acrylic` switches it; the owner picks the default.
fn acrylic_wanted() -> bool {
    std::env::var("DECKA_ISLAND_GLASS").is_ok_and(|v| v == "acrylic")
}

/// What the glass was last cut for: the mode and the rounded panel rectangle.
type GlassKey = (Mode, Option<(i64, i64, i64, i64)>);

/// Panel radius in CSS pixels, as the page draws it (`island.css`).
fn panel_radius(mode: Mode) -> i32 {
    if mode == Mode::Expanded { 24 } else { 22 }
}

/// Acrylic on the window, cut to the panel; off (and no cut) for the tab, which has no panel.
fn apply_glass(window: &tauri::WebviewWindow, mode: Mode, rect: Option<Rect>) {
    let Ok(hwnd) = window.hwnd() else { return };
    let hwnd = hwnd.0 as isize;
    match rect {
        Some(r) if !matches!(mode, Mode::Tab | Mode::Orb) && r.width > 1.0 => {
            let scale = window.scale_factor().unwrap_or(1.0);
            decka_core::glass::set_acrylic(hwnd, true);
            let px = |v: f64| (v * scale).round() as i32;
            decka_core::glass::set_region(hwnd, Some((px(r.x), px(r.y), px(r.width), px(r.height))), px(f64::from(panel_radius(mode))));
        }
        _ => {
            decka_core::glass::set_region(hwnd, None, 0);
            decka_core::glass::set_acrylic(hwnd, false);
        }
    }
}

fn create(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let mut url = String::from("index.html?island=1");
    if acrylic_wanted() {
        url.push_str("&glass=acrylic");
    }
    // For snapshots only: `DECKA_ISLAND_THEME=light|dark` overrides the colour scheme of the system.
    if let Ok(theme) = std::env::var("DECKA_ISLAND_THEME") {
        if theme == "light" || theme == "dark" {
            url.push_str(&format!("&theme={theme}"));
        }
    }
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(url.into()))
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
    // T29.1: never the active window. Clicks go through until the cursor is on the panel.
    on_hwnd(&window, |hwnd| {
        overlay::guard(hwnd);
        overlay::set_click_through(hwnd, true);
    });
    Some(window)
}

/// T29.1: an overlay window is shown, hidden and made click-through by `decka_core::overlay` on its own thread; the calls of
/// the window library for these (`show`, `set_ignore_cursor_events`) would activate it and take the focus from the user.
pub fn on_hwnd(window: &tauri::WebviewWindow, f: impl FnOnce(isize) + Send + 'static) {
    let target = window.clone();
    let _ = window.run_on_main_thread(move || {
        if let Ok(hwnd) = target.hwnd() {
            f(hwnd.0 as isize);
        }
    });
}

/// T29.4: the window of the orb, logical pixels: the orb and room for a bubble to its right. The page pads it by `ORB_PAD`.
const ORB_SIZE: (f64, f64) = (360.0, 48.0);
const ORB_PAD: f64 = 4.0;

/// The left edge of a window of `width` on a screen from `left` that is `screen` wide, all in one unit; `inset`: the free
/// space it keeps to the side.
fn aligned(align: Align, left: f64, screen: f64, width: f64, inset: f64) -> f64 {
    match align {
        Align::Left => left + inset,
        Align::Center => left + (screen - width) / 2.0,
        Align::Right => left + screen - width - inset,
    }
}

/// Puts the window in its place; `size` is the size the page asked for (used by the panel and the words).
fn place(app: &AppHandle, window: &tauri::WebviewWindow, spot: Spot, size: (f64, f64)) {
    let monitor = match spot {
        Spot::Orb(at) => app.monitor_from_point(f64::from(at.0), f64::from(at.1)).ok().flatten(),
        _ => None,
    };
    let Some(m) = monitor.or_else(|| app.primary_monitor().ok().flatten()) else { return };
    let scale = m.scale_factor();
    let (left, top) = (f64::from(m.position().x), f64::from(m.position().y));
    let screen = (f64::from(m.size().width), f64::from(m.size().height));
    let (size, at) = match spot {
        Spot::Panel | Spot::Words(_) => {
            let width = size.0.min(screen.0 / scale);
            let (align, gap) = match spot {
                Spot::Words(align) => (align, 0.0),
                _ => (Align::Center, TOP_GAP),
            };
            ((width, size.1), (aligned(align, left, screen.0, width * scale, 0.0), top + (gap - PAD) * scale))
        }
        Spot::Bar(align, height) => ((BAR_WIDTH, f64::from(height)), (aligned(align, left, screen.0, BAR_WIDTH * scale, PAD * scale), top)),
        Spot::Orb(anchor) => {
            let px = ((ORB_SIZE.0 * scale).round() as i32, (ORB_SIZE.1 * scale).round() as i32);
            let rect = (left as i32, top as i32, (left + screen.0) as i32, (top + screen.1) as i32);
            let (x, y) = orb::origin(anchor, px, ((orb::GAP - ORB_PAD) * scale).round() as i32, rect);
            (ORB_SIZE, (f64::from(x), f64::from(y)))
        }
    };
    let position = PhysicalPosition::new(at.0.round() as i32, at.1.round() as i32);
    // Position, size, position: a move to a monitor with another scale changes the size in logical pixels.
    let _ = window.set_position(position);
    let _ = window.set_size(LogicalSize::new(size.0, size.1));
    let _ = window.set_position(position);
}

/// The page reported a new panel: the window takes its size (T26.F). An empty or unchanged panel changes nothing, and the
/// bar and the orb have sizes of their own.
pub fn fit(app: &AppHandle, rect: Rect) {
    if rect.width < 1.0 || rect.height < 1.0 {
        return;
    }
    let Some(shared) = app.try_state::<Arc<Shared>>() else { return };
    let spot = shared.spot();
    if matches!(spot, Spot::Bar(..) | Spot::Orb(_)) {
        return;
    }
    let size = window_size(rect.width.ceil(), rect.height.ceil());
    let old = shared.size();
    if (size.0 - old.0).abs() < 1.0 && (size.1 - old.1).abs() < 1.0 {
        return;
    }
    shared.set_size(size);
    if let Some(window) = app.get_webview_window(LABEL) {
        if window.is_visible().unwrap_or(false) {
            place(app, &window, spot, size);
        }
    }
}

/// Whether the cursor is on the visible panel; `None`: on the whole window.
fn cursor_on_panel(app: &AppHandle, window: &tauri::WebviewWindow, rect: Option<Rect>) -> bool {
    let (Ok(cursor), Ok(origin)) = (app.cursor_position(), window.outer_position()) else { return false };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (left, top, width, height) = match rect {
        Some(r) => (origin.x as f64 + r.x * scale, origin.y as f64 + r.y * scale, r.width * scale, r.height * scale),
        None => {
            let Ok(size) = window.outer_size() else { return false };
            (origin.x as f64, origin.y as f64, f64::from(size.width), f64::from(size.height))
        }
    };
    cursor.x >= left && cursor.x < left + width && cursor.y >= top && cursor.y < top + height
}
/// The loop of the island. Called once the page of the island exists (T26.R).
#[allow(dead_code)]
pub fn setup(app: &AppHandle, agent: &Agent) -> Arc<Shared> {
    let shared = Arc::new(Shared::default());
    app.manage(shared.clone());
    agent.set_island_present();
    let (app, agent, loop_shared) = (app.clone(), agent.clone(), shared.clone());
    tauri::async_runtime::spawn(async move {
        let mut machine = Machine::default();
        let meter = decka_core::audio::PeakMeter::start();
        let acrylic = acrylic_wanted();
        let mut last_glass: Option<GlassKey> = None;
        let (mut shown, mut ignoring, mut paused_since) = (false, true, None);
        let (mut last_event, mut fresh_until): (Option<Value>, Option<Instant>) = (None, None);
        let (mut tick, mut sampled) = (0u64, Sample::default());
        // Where the window was put last.
        let mut placed: Option<Spot> = None;
        let started = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
            tick += 1;
            if tick % 4 == 1 {
                sampled = sample(&agent, &mut paused_since);
            }
            let rect = *loop_shared.rect.lock().unwrap();
            let window = app.get_webview_window(LABEL);
            // The bar is the whole of its window; everything else is the panel the page reported.
            let on_panel = match (&window, loop_shared.spot(), rect) {
                (Some(w), Spot::Bar(..), _) if shown => cursor_on_panel(&app, w, None),
                (Some(w), _, Some(r)) if shown => cursor_on_panel(&app, w, Some(r)),
                _ => false,
            };

            let host = app.try_state::<Host>();
            let slow = host.as_ref().and_then(|h| h.slow_progress());
            // T29.4: a dictation is the orb at the caret, not the panel.
            let orb = host.as_ref().and_then(|h| h.orb());
            let level = loop_shared.voice.lock().unwrap().as_ref().and_then(|p| p.get("level").and_then(Value::as_f64)).unwrap_or(0.0);
            let (voice, voice_json) = {
                let payload = loop_shared.voice.lock().unwrap().clone().filter(|_| orb.is_none());
                let chips = host.as_ref().map(|h| h.context_chips()).unwrap_or_default();
                voice_view(payload.as_ref(), slow, host.as_ref().and_then(|h| h.download_progress()), host.as_ref().and_then(|h| h.loading_eta_sec()), &chips)
            };
            let island = machine.step(&Inputs {
                now_ms: started.elapsed().as_millis() as u64,
                voice,
                hovered: on_panel,
                lyrics_visible: sampled.visible,
                lyrics_mode: sampled.mode,
                music: sampled.music,
                rest: sampled.settings.resting_mode,
                fullscreen: sampled.fullscreen,
            });
            let anchor = orb.as_ref().and_then(|o| o.anchor);
            let mode = match &orb {
                // Until the caret is found nothing is shown.
                Some(_) if anchor.is_none() => Mode::Hidden,
                Some(_) => Mode::Orb,
                None => island,
            };
            // The orb and the bar never take a click: they only look.
            let clickable = !matches!(mode, Mode::Orb | Mode::Tab);
            let on_panel = on_panel && clickable;

            if mode == Mode::Hidden && !loop_shared.guide_on() {
                // T27.F.2: a hidden island reads no WASAPI peak (the meter used to stay on after compact -> hidden).
                meter.set_active(false);
                if shown {
                    if let Some(w) = &window {
                        on_hwnd(w, overlay::hide);
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
            let spot = match mode {
                Mode::Orb => Spot::Orb(anchor.unwrap_or_default()),
                Mode::Tab => Spot::Bar(sampled.settings.bar_position, sampled.settings.bar_height),
                Mode::Compact => Spot::Words(sampled.settings.bar_position),
                Mode::Expanded | Mode::Hidden => Spot::Panel,
            };
            if placed != Some(spot) || !shown {
                *loop_shared.spot.lock().unwrap() = spot;
                // The panel reported last belongs to the old place.
                if placed.as_ref().map(std::mem::discriminant) != Some(std::mem::discriminant(&spot)) {
                    *loop_shared.rect.lock().unwrap() = None;
                }
                place(&app, &window, spot, loop_shared.size());
                placed = Some(spot);
            }
            if !shown {
                on_hwnd(&window, |hwnd| {
                    overlay::set_click_through(hwnd, true);
                    overlay::show(hwnd);
                });
                shown = true;
                ignoring = true;
                fresh_until = Some(Instant::now() + FRESH);
            }
            // Clicks reach the panel only while the cursor is on it.
            if ignoring == on_panel {
                ignoring = !on_panel;
                let through = ignoring;
                on_hwnd(&window, move |hwnd| overlay::set_click_through(hwnd, through));
            }
            if acrylic {
                let key = (mode, rect.map(|r| (r.x.round() as i64, r.y.round() as i64, r.width.round() as i64, r.height.round() as i64)));
                if last_glass != Some(key) {
                    apply_glass(&window, mode, rect);
                    last_glass = Some(key);
                }
            }
            // The level of the music is read only while a song plays and something shows it (T26.F.10, T29.5).
            let music = sampled.playing && matches!(mode, Mode::Tab | Mode::Compact | Mode::Expanded);
            meter.set_active(music);
            let peak = if music { decka_core::audio::music_level(meter.level()) } else { 0.0 };
            // The thin meter of the bar moves in tenths: fewer events for a window that is there for the whole song.
            let level_of_music = if mode == Mode::Tab { (peak * 10.0).round() / 10.0 } else { peak };
            let event = json!({
                "mode": mode,
                "lyrics": if voice == VoiceInput::Idle || sampled.visible { sampled.lyrics.clone() } else { None },
                "voice": voice_json,
                "playing": sampled.playing,
                "muted": sampled.muted,
                "track": sampled.track_info,
                "musicLevel": level_of_music,
                "barHeight": sampled.settings.bar_height,
                "barPosition": sampled.settings.bar_position,
                "orb": orb.as_ref().map(|o| json!({ "phase": o.phase.name(), "level": level, "text": o.bubble.as_ref().map(|b| &b.text), "key": o.bubble.as_ref().and_then(|b| b.key) })),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_is_the_panel_plus_the_shadow_margin() {
        assert_eq!(window_size(520.0, 230.0), (568.0, 278.0));
        assert_eq!(window_size(160.0, 24.0), (208.0, 72.0));
    }
}
