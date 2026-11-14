//! System tray icon and its menu.

use decka_core::Agent;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// The only way to leave the app: stop the agent cleanly, then exit.
pub fn quit(app: &AppHandle) {
    if let Some(agent) = app.try_state::<Agent>() {
        tauri::async_runtime::block_on(agent.shutdown());
    }
    app.exit(0);
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open settings", true, None::<&str>)?;
    let pair = MenuItem::with_id(app, "pair", "Pair a device", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pair, &separator, &quit_item])?;

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
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_settings(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
