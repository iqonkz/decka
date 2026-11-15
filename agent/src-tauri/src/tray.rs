//! System tray icon and its menu.

use decka_core::tray_text::{agent_language, tray_text, TrayKey};
use decka_core::Agent;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// The items of the tray menu, kept so that their texts can change when the language does.
struct TrayItems {
    open: MenuItem<tauri::Wry>,
    pair: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

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
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let lang = current_language(app);
    let open = MenuItem::with_id(app, "open", tray_text(lang, TrayKey::OpenSettings), true, None::<&str>)?;
    let pair = MenuItem::with_id(app, "pair", tray_text(lang, TrayKey::PairDevice), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", tray_text(lang, TrayKey::Quit), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pair, &separator, &quit_item])?;
    app.manage(TrayItems { open, pair, quit: quit_item });

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
