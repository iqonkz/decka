//! T32.3: the window of the mini panel of the tray. Made at the click on the icon, never active (like the island), gone after
//! a click outside or Esc, destroyed 30 s later. Where it stands and when it goes away: `decka_core::tray_panel`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use decka_core::overlay;
use decka_core::tray_panel::{self, Area, HEIGHT, KEEP, MARGIN, REOPEN_GUARD, WIDTH};
use decka_core::Agent;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const LABEL: &str = "tray";
const POLL: Duration = Duration::from_millis(30);

#[derive(Default)]
pub struct Mini {
    open: AtomicBool,
    /// Every opening and closing counts: an old watcher or timer sees that its time is over.
    turn: AtomicU64,
    closed_at: Mutex<Option<Instant>>,
}

fn create(app: &AppHandle) -> Option<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?tray=1".into()))
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
        .map_err(|e| tracing::error!("cannot create the tray panel: {e}"))
        .ok()?;
    crate::island::on_hwnd(&window, |hwnd| {
        overlay::guard(hwnd);
        overlay::set_click_through(hwnd, false);
    });
    Some(window)
}

/// Puts the window above (or below) the icon that was clicked.
fn place(app: &AppHandle, window: &WebviewWindow, click: PhysicalPosition<f64>) {
    let monitor = app.monitor_from_point(click.x, click.y).ok().flatten().or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let size = ((WIDTH * scale).round() as i32, (HEIGHT * scale).round() as i32);
    let area = Area { x: work.position.x, y: work.position.y, width: work.size.width as i32, height: work.size.height as i32 };
    let (x, y) = tray_panel::place((click.x as i32, click.y as i32), area, size, (MARGIN * scale).round() as i32);
    // The position first: the size is in the pixels of the monitor the window is on.
    let moved = window.set_position(PhysicalPosition::new(x, y)).and_then(|()| window.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32)));
    if let Err(e) = moved {
        tracing::warn!("tray panel: {e}");
    }
}

/// The left click on the icon in the tray.
pub fn toggle(app: &AppHandle, click: PhysicalPosition<f64>) {
    let Some(mini) = app.try_state::<Arc<Mini>>().map(|m| m.inner().clone()) else { return };
    if mini.open.load(Ordering::SeqCst) {
        return close(app);
    }
    if mini.closed_at.lock().unwrap().is_some_and(|t| t.elapsed() < REOPEN_GUARD) {
        return;
    }
    // Creating a window inside the handler of the tray event blocks the event loop: on a task of its own.
    let app = app.clone();
    tauri::async_runtime::spawn(async move { open(&app, &mini, click) });
}

fn open(app: &AppHandle, mini: &Arc<Mini>, click: PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window(LABEL).or_else(|| create(app)) else { return };
    if mini.open.swap(true, Ordering::SeqCst) {
        return;
    }
    let turn = mini.turn.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(agent) = app.try_state::<Agent>() {
        agent.tray_panel_open(true);
    }
    place(app, &window, click);
    crate::island::set_resting(&window, false);
    crate::island::on_hwnd(&window, overlay::show);
    let (app, mini) = (app.clone(), mini.clone());
    std::thread::spawn(move || {
        let Some(hwnd) = window.hwnd().ok().map(|h| h.0 as isize) else { return };
        let mut before = tray_panel::input();
        while mini.turn.load(Ordering::SeqCst) == turn {
            std::thread::sleep(POLL);
            let now = tray_panel::input();
            if tray_panel::window_area(hwnd).is_some_and(|panel| tray_panel::dismisses(before, now, panel)) {
                return close(&app);
            }
            before = now;
        }
    });
}

/// Hides the panel; after `KEEP` without another opening its window is destroyed.
pub fn close(app: &AppHandle) {
    let Some(mini) = app.try_state::<Arc<Mini>>().map(|m| m.inner().clone()) else { return };
    if !mini.open.swap(false, Ordering::SeqCst) {
        return;
    }
    let turn = mini.turn.fetch_add(1, Ordering::SeqCst) + 1;
    *mini.closed_at.lock().unwrap() = Some(Instant::now());
    if let Some(agent) = app.try_state::<Agent>() {
        agent.tray_panel_open(false);
    }
    let Some(window) = app.get_webview_window(LABEL) else { return };
    crate::island::on_hwnd(&window, overlay::hide);
    crate::island::set_resting(&window, true);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(KEEP);
        if mini.turn.load(Ordering::SeqCst) == turn && !mini.open.load(Ordering::SeqCst) {
            if let Some(window) = app.get_webview_window(LABEL) {
                if let Err(e) = window.destroy() {
                    tracing::warn!("tray panel: {e}");
                }
            }
        }
    });
}
