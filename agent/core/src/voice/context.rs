//! Hints by context (T26.7): when the island opens by voice, and only then, the PC looks at the name of the active program, its
//! window title and whether text is selected, and offers up to three chips. Rules only: no screenshot, no OCR, no model; nothing
//! is stored. Off by default (`config.voice.context.enabled`). A tap on a chip is the same request as saying it.

/// A hint chip: `label` is an i18n key (`voice.chip.*`), `command` one of `COMMANDS` (IPC `voice_chip`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub label: &'static str,
    pub command: &'static str,
}

/// The only commands a chip may carry.
pub const COMMANDS: [&str; 4] = ["translate", "explain", "reply", "error"];

const TRANSLATE: Chip = Chip { label: "voice.chip.translate", command: "translate" };
const EXPLAIN: Chip = Chip { label: "voice.chip.explain", command: "explain" };
const REPLY: Chip = Chip { label: "voice.chip.reply", command: "reply" };
const ERROR: Chip = Chip { label: "voice.chip.error", command: "error" };

/// At most this many chips.
pub const MAX_CHIPS: usize = 3;

/// What the PC saw when the island opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Lower-case exe name of the active program (`outlook.exe`).
    pub process: String,
    pub title: String,
    pub has_selection: bool,
    /// The Screen help package is installed (an error can be explained).
    pub vision_ready: bool,
}

const MESSENGERS: [&str; 6] = ["outlook.exe", "thunderbird.exe", "telegram.exe", "whatsapp.exe", "slack.exe", "discord.exe"];
const BROWSERS: [&str; 6] = ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "vivaldi.exe"];
const MAIL_WORDS: [&str; 3] = ["gmail", "outlook", "mail"];
const ERROR_WORDS: [&str; 5] = ["error", "exception", "failed", "ошибка", "сбой"];

fn contains_any(text: &str, words: &[&str]) -> bool {
    let lower = text.to_lowercase();
    words.iter().any(|w| lower.contains(w))
}

/// Whether the program is a mailbox or a messenger, where a draft reply makes sense.
fn is_conversation(s: &Snapshot) -> bool {
    MESSENGERS.contains(&s.process.as_str()) || (BROWSERS.contains(&s.process.as_str()) && contains_any(&s.title, &MAIL_WORDS))
}

/// The chips for what was seen, at most `MAX_CHIPS`.
pub fn chips(s: &Snapshot) -> Vec<Chip> {
    let mut out = Vec::new();
    if s.has_selection {
        out.push(TRANSLATE);
        out.push(EXPLAIN);
        if is_conversation(s) {
            out.push(REPLY);
        }
    } else if s.vision_ready && contains_any(&s.title, &ERROR_WORDS) {
        out.push(ERROR);
    }
    out.truncate(MAX_CHIPS);
    out
}

/// The chips as the event of the island carries them.
pub fn chips_json(chips: &[Chip]) -> Vec<serde_json::Value> {
    chips.iter().map(|c| serde_json::json!({ "label": c.label, "command": c.command })).collect()
}

/// The title of the foreground window.
#[cfg(windows)]
pub fn foreground_title() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};
    // SAFETY: a plain Win32 call with a buffer that outlives it.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut buffer = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut buffer);
        (n > 0).then(|| String::from_utf16_lossy(&buffer[..n as usize]))
    }
}

#[cfg(not(windows))]
pub fn foreground_title() -> Option<String> {
    None
}

/// Looks at the active program and the selection (UI Automation, the clipboard is not touched). Blocking: up to ~1.2 s.
pub fn snapshot(vision_ready: bool) -> Snapshot {
    let process = crate::collectors::focus::foreground_app().unwrap_or_default();
    let title = foreground_title().unwrap_or_default();
    let has_selection = super::uia::selected_text().is_some();
    Snapshot { process, title, has_selection, vision_ready }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(process: &str, title: &str, selection: bool, vision: bool) -> Snapshot {
        Snapshot { process: process.into(), title: title.into(), has_selection: selection, vision_ready: vision }
    }

    fn commands(s: &Snapshot) -> Vec<&'static str> {
        chips(s).iter().map(|c| c.command).collect()
    }

    #[test]
    fn a_selection_offers_translate_and_explain() {
        assert_eq!(commands(&snap("notepad.exe", "notes", true, false)), ["translate", "explain"]);
    }

    #[test]
    fn a_mail_program_or_messenger_adds_reply() {
        for p in ["outlook.exe", "thunderbird.exe", "telegram.exe", "whatsapp.exe", "slack.exe", "discord.exe"] {
            assert_eq!(commands(&snap(p, "Chat", true, false)), ["translate", "explain", "reply"], "{p}");
        }
        assert_eq!(commands(&snap("chrome.exe", "Inbox (3) - me@example.com - Gmail", true, false)), ["translate", "explain", "reply"]);
        assert_eq!(commands(&snap("msedge.exe", "Outlook - Mail", true, false)), ["translate", "explain", "reply"]);
        assert_eq!(commands(&snap("chrome.exe", "Wikipedia", true, false)), ["translate", "explain"], "an ordinary page: no reply");
        assert_eq!(commands(&snap("code.exe", "mail.rs", true, false)), ["translate", "explain"], "a title alone is not enough outside a browser");
    }

    #[test]
    fn an_error_in_the_title_offers_the_question_only_when_screen_help_is_installed() {
        for title in ["Error 0x80070005", "Build FAILED", "Unhandled Exception", "Ошибка установки", "Сбой приложения"] {
            assert_eq!(commands(&snap("explorer.exe", title, false, true)), ["error"], "{title}");
            assert!(chips(&snap("explorer.exe", title, false, false)).is_empty(), "no package: {title}");
        }
    }

    #[test]
    fn a_selection_wins_over_an_error_title_and_nothing_special_gives_no_chips() {
        assert_eq!(commands(&snap("code.exe", "error.log", true, true)), ["translate", "explain"]);
        assert!(chips(&snap("code.exe", "main.rs", false, true)).is_empty());
        assert!(chips(&Snapshot::default()).is_empty());
    }

    #[test]
    fn never_more_than_three_and_only_known_commands() {
        let all = chips(&snap("telegram.exe", "x", true, true));
        assert!(all.len() <= MAX_CHIPS);
        assert!(all.iter().all(|c| COMMANDS.contains(&c.command) && c.label.starts_with("voice.chip.")));
        assert_eq!(chips_json(&all)[0], serde_json::json!({ "label": "voice.chip.translate", "command": "translate" }));
    }
}
