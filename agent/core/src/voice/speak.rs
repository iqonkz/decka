//! Reading an answer aloud (T26.3 `speak`): the system voice of Windows (SAPI), blocking until the text has been read. The
//! text is data for the voice, nothing else; it never reaches a shell or a command. Before the voice reads it the text is
//! made fit for speech (`spoken`, T33.2).

use std::sync::atomic::{AtomicU64, Ordering};

/// The longest text that is read aloud as an answer.
pub const MAX_SPOKEN: usize = 1_200;
/// The longest text that is read aloud on "read aloud" (T27.9).
pub const MAX_READ_ALOUD: usize = 6_000;

/// The longest answer that is spoken by itself (T29.8, "answer aloud"): the length of a reply of the protocol.
pub const MAX_ANSWER: usize = 300;

/// Counts the readings and the stops: a reading goes on only while the number is its own.
static TURN: AtomicU64 = AtomicU64::new(0);
static SPEAKING: AtomicU64 = AtomicU64::new(0);

/// The speed of speech, percent of the normal one (T33.2): the least, the most, the one of a new installation and the
/// step of "faster" and "slower".
pub const MIN_SPEED: u32 = 80;
pub const MAX_SPEED: u32 = 130;
pub const DEFAULT_SPEED: u32 = 105;
pub const SPEED_STEP: u32 = 10;

/// How a text is to be read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Percent of the normal speed.
    pub speed: u32,
    /// Which of the installed voices of a language reads, counted round ("change the voice").
    pub voice: u32,
}

/// The rate of a Windows voice (-10..10, where 10 is three times as fast) for a speed in percent.
fn sapi_rate(percent: u32) -> i32 {
    let speed = f64::from(percent.clamp(MIN_SPEED, MAX_SPEED)) / 100.0;
    (10.0 * speed.ln() / 3f64.ln()).round() as i32
}

/// "Stop": ends the reading that is going on (if any).
pub fn stop() {
    TURN.fetch_add(1, Ordering::SeqCst);
}

/// Whether something is being read now.
pub fn is_speaking() -> bool {
    SPEAKING.load(Ordering::SeqCst) > 0
}

/// Takes the next turn: the reading before it ends.
fn next_turn() -> u64 {
    TURN.fetch_add(1, Ordering::SeqCst) + 1
}

fn still(turn: u64) -> bool {
    TURN.load(Ordering::SeqCst) == turn
}

/// Reads `text` (at most `MAX_READ_ALOUD` characters) and ends early on `stop()`. `hint` is the language of the
/// interface. Blocking.
pub fn read_aloud(text: &str, hint: &str, style: Style) -> Result<(), String> {
    read_any(&text.chars().take(MAX_READ_ALOUD).collect::<String>(), hint, style)
}

/// T29.8: an answer in the voice of Windows for `lang`, at most `MAX_ANSWER` characters; ends early on `stop()`. Blocking.
pub fn answer(text: &str, lang: &str, style: Style) -> Result<(), String> {
    let text: String = text.chars().take(MAX_ANSWER).collect();
    let said = super::spoken::for_speech(&text, super::spoken::language_of(&text, lang));
    read(&said, language_id(lang), style)
}

/// A text of any language: its numbers are said in the language its letters show, and a voice of that language is
/// chosen when the default one would not read it (Cyrillic, or Latin under a Cyrillic interface).
fn read_any(text: &str, hint: &str, style: Style) -> Result<(), String> {
    let lang = super::spoken::language_of(text, hint);
    let own = matches!(lang, "ru" | "kk") || (lang == "en" && matches!(hint, "ru" | "kk"));
    read(&super::spoken::for_speech(text, lang), own.then(|| language_id(lang)).flatten(), style)
}

/// The Windows language id (hexadecimal, as the voices are labelled) of an interface language.
pub fn language_id(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "en" => "409",
        "ru" => "419",
        "de" => "407",
        "es" => "C0A;40A",
        "fr" => "40C",
        "pt" => "416;816",
        "zh" => "804",
        "ja" => "411",
        "hi" => "439",
        "ar" => "401",
        "kk" => "43F",
        _ => return None,
    })
}

fn read(text: &str, language: Option<&str>, style: Style) -> Result<(), String> {
    let turn = next_turn();
    // Nothing of the text is to be said (code, signs).
    if text.is_empty() {
        return Ok(());
    }
    SPEAKING.fetch_add(1, Ordering::SeqCst);
    let done = read_aloud_impl(text, language, style, turn);
    SPEAKING.fetch_sub(1, Ordering::SeqCst);
    done
}

/// The installed voices of a language (the ids of `language_id`).
///
/// # Safety
/// COM is initialised on this thread; the tokens are dropped before it is closed.
#[cfg(windows)]
unsafe fn installed(language: &str) -> Vec<windows::Win32::Media::Speech::ISpObjectToken> {
    use windows::core::{w, HSTRING};
    use windows::Win32::Media::Speech::{ISpObjectTokenCategory, SpObjectTokenCategory};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    let mut out = Vec::new();
    // SAFETY: the caller initialised COM.
    unsafe {
        let Ok(voices) = CoCreateInstance::<_, ISpObjectTokenCategory>(&SpObjectTokenCategory, None, CLSCTX_ALL) else { return out };
        if voices.SetId(w!("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Voices"), false).is_err() {
            return out;
        }
        for id in language.split(';') {
            let Ok(found) = voices.EnumTokens(&HSTRING::from(format!("Language={id}")), None) else { continue };
            loop {
                let mut token = None;
                if found.Next(1, &mut token, None).is_err() {
                    break;
                }
                let Some(token) = token else { break };
                out.push(token);
            }
        }
    }
    out
}

/// How many voices of an interface language Windows has.
#[cfg(windows)]
pub fn voices_of(lang: &str) -> usize {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    let Some(language) = language_id(lang) else { return 0 };
    // SAFETY: COM on this thread only; the tokens are gone before it is closed.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let count = installed(language).len();
        if initialised {
            CoUninitialize();
        }
        count
    }
}

#[cfg(not(windows))]
pub fn voices_of(_: &str) -> usize {
    0
}

#[cfg(windows)]
fn read_aloud_impl(text: &str, language: Option<&str>, style: Style, turn: u64) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Media::Speech::{ISpVoice, SpVoice};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};
    // SAFETY: COM on this thread only; the voice is dropped before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let spoken = (|| -> Result<(), String> {
            let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL).map_err(|e| format!("No system voice: {e}"))?;
            if let Some(language) = language {
                // The default voice stays when the language has none.
                let mut found = installed(language);
                if !found.is_empty() {
                    let nth = style.voice as usize % found.len();
                    let _ = voice.SetVoice(&found.swap_remove(nth));
                }
            }
            let _ = voice.SetRate(sapi_rate(style.speed));
            if !still(turn) {
                return Ok(());
            }
            // SPF_ASYNC (1): the call returns at once and the text is polled for the end or for "stop".
            voice.Speak(&HSTRING::from(text), 1, None).map_err(|e| format!("The voice failed: {e}"))?;
            let mut status = windows::Win32::Media::Speech::SPVOICESTATUS::default();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(80));
                // SPRS_DONE (1): the text has been read.
                if voice.GetStatus(&mut status, std::ptr::null_mut()).is_ok() && status.dwRunningState & 1 != 0 {
                    break;
                }
                if !still(turn) {
                    // SPF_PURGEBEFORESPEAK (2) with an empty text cuts what is being read.
                    let _ = voice.Speak(&HSTRING::from(""), 2, None);
                    break;
                }
            }
            Ok(())
        })();
        if initialised {
            CoUninitialize();
        }
        spoken
    }
}

#[cfg(not(windows))]
fn read_aloud_impl(_: &str, _: Option<&str>, _: Style, _: u64) -> Result<(), String> {
    Err("unsupported platform".into())
}

/// Reads `text` aloud with the default voice of the system (the "speak" button of an answer). Blocks until it is done or
/// stopped (call it from a blocking thread).
pub fn speak(text: &str, hint: &str, style: Style) -> Result<(), String> {
    read_any(&text.chars().take(MAX_SPOKEN).collect::<String>(), hint, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_interface_language_has_a_windows_language() {
        for lang in super::super::textops::LANGS {
            assert!(language_id(lang).is_some(), "{lang}");
        }
        assert_eq!(language_id("xx"), None);
    }

    #[test]
    fn the_speed_becomes_the_rate_of_a_windows_voice() {
        assert_eq!([80, 90, 100, DEFAULT_SPEED, 115, MAX_SPEED].map(sapi_rate), [-2, -1, 0, 0, 1, 2]);
        assert_eq!((sapi_rate(1), sapi_rate(900)), (sapi_rate(MIN_SPEED), sapi_rate(MAX_SPEED)));
    }

    #[test]
    fn a_text_with_nothing_to_say_is_not_read() {
        let style = Style { speed: DEFAULT_SPEED, voice: 0 };
        assert_eq!(read_aloud("```\ncode\n```", "en", style), Ok(()));
        assert!(!is_speaking());
    }

    #[test]
    fn a_stop_and_a_newer_reading_end_the_one_before() {
        let first = next_turn();
        assert!(still(first));
        let second = next_turn();
        assert!(!still(first) && still(second));
        stop();
        assert!(!still(second));
    }
}
