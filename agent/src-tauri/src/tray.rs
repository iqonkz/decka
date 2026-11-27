//! System tray icon and its menu.

use std::sync::Mutex;
use std::time::Instant;

use decka_core::tray_text::{agent_language, tray_text, update_available_text, TrayKey};
use decka_core::Agent;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// The items of the tray menu, kept so that their texts can change when the language does.
struct TrayItems {
    menu: Menu<tauri::Wry>,
    open: MenuItem<tauri::Wry>,
    pair: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    /// `Update available (X.Y.Z)`: in the menu only while a newer release is known.
    update: MenuItem<tauri::Wry>,
    /// The version the update item names now, `None` while the item is not in the menu.
    update_version: Mutex<Option<String>>,
    /// T32.1: the icon carries the dot of the microphone now.
    wake_dot: Mutex<bool>,
}

/// T31.5: when the settings window was asked for, until the new window reports its first frame.
#[derive(Default)]
pub struct WindowOpening(pub Mutex<Option<Instant>>);

/// Shows the settings window. It is destroyed when closed (DATA_MODEL.md 10.1: no WebView2 processes while it is shut),
/// so it is created again here from the same config entry.
pub fn show_settings(app: &AppHandle) {
    open_settings(app, None);
}

/// T29.6: shows the settings window at a target of the window (`keys:catalog:<id>`): a window that is open hears the event, a
/// new one finds the target in its address.
pub fn show_settings_at(app: &AppHandle, target: &str) {
    open_settings(app, Some(target));
}

fn open_settings(app: &AppHandle, target: Option<&str>) {
    let window = match app.get_webview_window("settings") {
        Some(w) => {
            if let Some(target) = target {
                let _ = app.emit("decka://navigate", target);
            }
            w
        }
        None => {
            if let Some(opening) = app.try_state::<WindowOpening>() {
                *opening.0.lock().unwrap() = Some(Instant::now());
            }
            let Some(mut cfg) = app.config().app.windows.iter().find(|w| w.label == "settings").cloned() else { return };
            // Only the characters of a catalog id: the target goes into the address as it is.
            if let Some(target) = target.filter(|t| t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-'))) {
                cfg.url = tauri::WebviewUrl::App(format!("index.html?go={target}").into());
            }
            let started = Instant::now();
            match tauri::WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
                Ok(w) => {
                    // The part of the opening that is WebView2 itself; the rest is the page.
                    tracing::info!("settings window: created in {} ms", started.elapsed().as_millis());
                    w
                }
                Err(e) => {
                    tracing::error!("cannot create the settings window: {e}");
                    return;
                }
            }
        }
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// The only way to leave the app: stop the agent cleanly, then exit.
pub fn quit(app: &AppHandle) {
    if let Some(agent) = app.try_state::<Agent>() {
        tauri::async_runtime::block_on(agent.shutdown());
    }
    app.exit(0);
}

/// The language of the tray now: `display.language` of the layout, `auto` following Windows.
fn current_language(app: &AppHandle) -> &'static str {
    let setting = app.try_state::<Agent>().and_then(|agent| agent.layout().display.language);
    agent_language(setting.as_deref())
}

/// Puts the texts of the menu in the language of the layout; called after the layout was saved.
pub fn refresh_language(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let lang = current_language(app);
    let _ = items.open.set_text(tray_text(lang, TrayKey::OpenSettings));
    let _ = items.pair.set_text(tray_text(lang, TrayKey::PairDevice));
    let _ = items.quit.set_text(tray_text(lang, TrayKey::Quit));
    if *items.wake_dot.lock().unwrap() {
        if let Some(tray) = app.tray_by_id("main") {
            if let Err(e) = tray.set_tooltip(Some(tray_text(lang, TrayKey::WakeListening))) {
                tracing::warn!("tray tooltip: {e}");
            }
        }
    }
    let shown = items.update_version.lock().unwrap().clone();
    if let Some(version) = shown {
        let _ = items.update.set_text(update_available_text(lang, &version));
    }
}

/// Shows `Update available (X.Y.Z)` in the menu while a newer release is known (`Some`), removes it otherwise.
pub fn show_update(app: &AppHandle, version: Option<&str>) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let mut shown = items.update_version.lock().unwrap();
    match (version, shown.is_some()) {
        (Some(v), was_shown) => {
            let _ = items.update.set_text(update_available_text(current_language(app), v));
            if !was_shown {
                // After `Open settings` and `Pair a device`, before the separator.
                let _ = items.menu.insert(&items.update, 2);
            }
            *shown = Some(v.to_string());
        }
        (None, true) => {
            let _ = items.menu.remove(&items.update);
            *shown = None;
        }
        (None, false) => {}
    }
}

/// T32.1: while the wake word is on, the icon in the tray carries a dot of a microphone.
pub fn show_wake(app: &AppHandle, on: bool) {
    let (Some(items), Some(tray), Some(icon)) = (app.try_state::<TrayItems>(), app.tray_by_id("main"), app.default_window_icon()) else { return };
    let mut shown = items.wake_dot.lock().unwrap();
    if *shown == on {
        return;
    }
    let image = if on {
        tauri::image::Image::new_owned(decka_core::voice::wake::dot_icon(icon.rgba(), icon.width(), icon.height()), icon.width(), icon.height())
    } else {
        icon.clone().to_owned()
    };
    let tooltip = if on { tray_text(current_language(app), TrayKey::WakeListening) } else { "Decka" };
    match tray.set_icon(Some(image)).and_then(|()| tray.set_tooltip(Some(tooltip))) {
        Ok(()) => *shown = on,
        Err(e) => tracing::warn!("tray icon of the wake word: {e}"),
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let lang = current_language(app);
    let open = MenuItem::with_id(app, "open", tray_text(lang, TrayKey::OpenSettings), true, None::<&str>)?;
    let pair = MenuItem::with_id(app, "pair", tray_text(lang, TrayKey::PairDevice), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", tray_text(lang, TrayKey::Quit), true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", update_available_text(lang, ""), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pair, &separator, &quit_item])?;
    app.manage(TrayItems { menu: menu.clone(), open, pair, quit: quit_item, update, update_version: Mutex::new(None), wake_dot: Mutex::new(false) });

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Decka")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_settings(app),
            "pair" => {
                show_settings(app);
                let _ = app.emit("decka://navigate", "pairing");
            }
            "update" => {
                show_settings(app);
                let _ = app.emit("decka://navigate", "about");
            }
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // T32.3: the left click opens the mini panel; the menu with "Open settings" stays on the right one.
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, .. } = event {
                crate::mini::toggle(tray.app_handle(), position);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    // A check may have finished before the tray existed.
    if let Some(agent) = app.try_state::<Agent>() {
        show_update(app, agent.update_status().available.as_ref().map(|a| a.version.as_str()));
    }
    Ok(())
}
