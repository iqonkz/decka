//! The few strings the agent itself shows (tray menu, system notifications) in the 11 languages of the interface (DESIGN.md 6).
//! Everything else the user sees is in the window or on the phone and comes from `shared/src/i18n`.

/// The languages of the interface (M19) in the order of DESIGN.md 6.
pub const LANGUAGES: [&str; 11] = ["en", "zh", "hi", "es", "ar", "fr", "pt", "ru", "de", "ja", "kk"];

/// Primary language ids of a Windows `LANGID` (the low 10 bits) -> language code (DESIGN.md 6).
const PRIMARY_IDS: [(u16, &str); 10] = [(0x19, "ru"), (0x3F, "kk"), (0x0A, "es"), (0x04, "zh"), (0x39, "hi"), (0x01, "ar"), (0x0C, "fr"), (0x16, "pt"), (0x07, "de"), (0x11, "ja")];

/// A string of the tray menu or a system notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayKey {
    OpenSettings,
    PairDevice,
    Quit,
    /// T32.1: the tooltip of the icon while the wake word is on.
    WakeListening,
}

/// `[Open settings, Pair a device, Quit]` and the text of the update item (`{v}` is the version) for each language.
fn words(lang: &str) -> ([&'static str; 3], &'static str) {
    match lang {
        "ru" => (["Открыть настройки", "Подключить телефон", "Выход"], "Доступно обновление ({v})"),
        "kk" => (["Параметрлерді ашу", "Телефонды қосу", "Шығу"], "Жаңарту қолжетімді ({v})"),
        "es" => (["Abrir ajustes", "Conectar un teléfono", "Salir"], "Actualización disponible ({v})"),
        "zh" => (["打开设置", "连接手机", "退出"], "有可用更新（{v}）"),
        "ja" => (["設定を開く", "スマートフォンを接続", "終了"], "アップデートがあります（{v}）"),
        "hi" => (["सेटिंग खोलें", "फ़ोन जोड़ें", "बाहर निकलें"], "अपडेट उपलब्ध है ({v})"),
        "ar" => (["فتح الإعدادات", "ربط هاتف", "إنهاء"], "يتوفر تحديث ({v})"),
        "fr" => (["Ouvrir les paramètres", "Associer un téléphone", "Quitter"], "Mise à jour disponible ({v})"),
        "pt" => (["Abrir configurações", "Conectar um telefone", "Sair"], "Atualização disponível ({v})"),
        "de" => (["Einstellungen öffnen", "Telefon koppeln", "Beenden"], "Update verfügbar ({v})"),
        _ => (["Open settings", "Pair a device", "Quit"], "Update available ({v})"),
    }
}

/// The text of `key` for `lang` (a language code of the list above; anything else is English).
pub fn tray_text(lang: &str, key: TrayKey) -> &'static str {
    let w = words(lang).0;
    match key {
        TrayKey::OpenSettings => w[0],
        TrayKey::PairDevice => w[1],
        TrayKey::Quit => w[2],
        TrayKey::WakeListening => match lang {
            "ru" => "Decka — микрофон слушает слово «Декка»",
            "kk" => "Decka — микрофон «Decka» сөзін тыңдап тұр",
            "es" => "Decka: el micrófono escucha la palabra «Decka»",
            "zh" => "Decka — 麦克风正在监听唤醒词“Decka”",
            "ja" => "Decka — マイクは「Decka」という言葉を聞いています",
            "hi" => "Decka — माइक्रोफ़ोन “Decka” शब्द सुन रहा है",
            "ar" => "Decka — الميكروفون يستمع إلى كلمة «Decka»",
            "fr" => "Decka : le micro écoute le mot « Decka »",
            "pt" => "Decka — o microfone está ouvindo a palavra “Decka”",
            "de" => "Decka – das Mikrofon hört auf das Wort „Decka“",
            _ => "Decka — the microphone listens for the word “Decka”",
        },
    }
}

/// The tray item that appears when a newer release exists: `Update available (0.2.0)`.
pub fn update_available_text(lang: &str, version: &str) -> String {
    words(lang).1.replace("{v}", version)
}

/// `display.language` (`auto` or a language code, absent = `auto`) and the primary language id of the system UI -> a language code.
/// `auto` and anything unknown follow Windows; a language that is not in the list gives English.
pub fn resolve_language(setting: Option<&str>, system_lang_id: u16) -> &'static str {
    if let Some(code) = setting.and_then(|s| LANGUAGES.iter().find(|l| **l == s)) {
        return code;
    }
    let primary = system_lang_id & 0x3ff;
    PRIMARY_IDS.iter().find(|(id, _)| *id == primary).map_or("en", |(_, code)| code)
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
        assert_eq!(update_available_text("de", "0.2.0"), "Update verfügbar (0.2.0)");
    }

    #[test]
    fn every_language_has_its_own_three_texts_and_english_is_the_default() {
        assert_eq!(tray_text("en", TrayKey::Quit), "Quit");
        assert_eq!(tray_text("xx", TrayKey::PairDevice), "Pair a device");
        for lang in LANGUAGES.iter().filter(|l| **l != "en") {
            for key in [TrayKey::OpenSettings, TrayKey::PairDevice, TrayKey::Quit] {
                assert_ne!(tray_text(lang, key), tray_text("en", key), "{lang}");
            }
            assert!(update_available_text(lang, "1.0.0").contains("1.0.0"), "{lang}");
        }
    }

    #[test]
    fn auto_follows_the_system_language() {
        // 0x0419 ru-RU, 0x0409 en-US, 0x043f kk-KZ, 0x0c0a es-ES, 0x0804 zh-CN, 0x0439 hi-IN, 0x0401 ar-SA, 0x040c fr-FR,
        // 0x0416 pt-BR, 0x0816 pt-PT, 0x0407 de-DE, 0x0411 ja-JP, 0x0436 af-ZA (not in the list).
        let cases = [(0x0419, "ru"), (0x0409, "en"), (0x043f, "kk"), (0x0c0a, "es"), (0x0804, "zh"), (0x0439, "hi"), (0x0401, "ar"), (0x040c, "fr"), (0x0416, "pt"), (0x0816, "pt"), (0x0407, "de"), (0x0411, "ja"), (0x0436, "en"), (0, "en")];
        for (id, expected) in cases {
            assert_eq!(resolve_language(None, id), expected, "{id:#x}");
            assert_eq!(resolve_language(Some("auto"), id), expected, "{id:#x}");
        }
    }

    #[test]
    fn an_explicit_choice_wins_over_the_system() {
        assert_eq!(resolve_language(Some("en"), 0x0419), "en");
        assert_eq!(resolve_language(Some("ru"), 0x0409), "ru");
        assert_eq!(resolve_language(Some("ja"), 0x0419), "ja");
        assert_eq!(resolve_language(Some("xx"), 0x0419), "ru", "an unknown code is `auto`");
    }
}
