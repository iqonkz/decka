//! The few strings the agent itself shows (tray menu, system notifications) in English and Russian (DESIGN.md 6).
//! Everything else the user sees is in the window or on the phone and comes from `shared/src/i18n`.

/// Primary language id of Russian in a Windows `LANGID` (the low 10 bits).
const PRIMARY_RUSSIAN: u16 = 0x19;

/// A string of the tray menu or a system notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayKey {
    OpenSettings,
    PairDevice,
    Quit,
}

/// The text of `key` for `lang` (`"ru"` is Russian, anything else English).
pub fn tray_text(lang: &str, key: TrayKey) -> &'static str {
    match (lang, key) {
        ("ru", TrayKey::OpenSettings) => "Открыть настройки",
        ("ru", TrayKey::PairDevice) => "Подключить телефон",
        ("ru", TrayKey::Quit) => "Выход",
        (_, TrayKey::OpenSettings) => "Open settings",
        (_, TrayKey::PairDevice) => "Pair a device",
        (_, TrayKey::Quit) => "Quit",
    }
}

/// The tray item that appears when a newer release exists: `Update available (0.2.0)`.
pub fn update_available_text(lang: &str, version: &str) -> String {
    match lang {
        "ru" => format!("Доступно обновление ({version})"),
        _ => format!("Update available ({version})"),
    }
}

/// `display.language` (`auto` | `en` | `ru`, absent = `auto`) and the primary language id of the system UI -> `"en"` or `"ru"`.
pub fn resolve_language(setting: Option<&str>, system_lang_id: u16) -> &'static str {
    match setting {
        Some("ru") => "ru",
        Some("en") => "en",
        _ if system_lang_id & 0x3ff == PRIMARY_RUSSIAN => "ru",
        _ => "en",
    }
}

/// The user interface language of Windows as a `LANGID`; 0 when it cannot be read (then English is used).
pub fn system_lang_id() -> u16 {
    #[cfg(windows)]
    {
        // SAFETY: a plain query without arguments or pointers.
        unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// The language the agent speaks now: the layout's setting, `auto` following Windows.
pub fn agent_language(setting: Option<&str>) -> &'static str {
    resolve_language(setting, system_lang_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_tray_texts() {
        assert_eq!(tray_text("ru", TrayKey::Quit), "Выход");
        assert_eq!(tray_text("ru", TrayKey::OpenSettings), "Открыть настройки");
        assert_eq!(tray_text("ru", TrayKey::PairDevice), "Подключить телефон");
    }

    #[test]
    fn the_update_item_names_the_version() {
        assert_eq!(update_available_text("en", "0.2.0"), "Update available (0.2.0)");
        assert_eq!(update_available_text("ru", "0.2.0"), "Доступно обновление (0.2.0)");
    }

    #[test]
    fn english_is_the_default() {
        assert_eq!(tray_text("en", TrayKey::Quit), "Quit");
        assert_eq!(tray_text("de", TrayKey::PairDevice), "Pair a device");
    }

    #[test]
    fn auto_follows_the_system_language() {
        // 0x0419 is ru-RU, 0x0409 is en-US, 0x043f is kk-KZ.
        assert_eq!(resolve_language(None, 0x0419), "ru");
        assert_eq!(resolve_language(Some("auto"), 0x0419), "ru");
        assert_eq!(resolve_language(Some("auto"), 0x0409), "en");
        assert_eq!(resolve_language(Some("auto"), 0x043f), "en");
        assert_eq!(resolve_language(Some("auto"), 0), "en");
    }

    #[test]
    fn an_explicit_choice_wins_over_the_system() {
        assert_eq!(resolve_language(Some("en"), 0x0419), "en");
        assert_eq!(resolve_language(Some("ru"), 0x0409), "ru");
    }
}
