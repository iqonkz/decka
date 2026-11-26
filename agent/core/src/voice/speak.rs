//! Reading an answer aloud (T26.3 `speak`): the system voice of Windows (SAPI), blocking until the text has been read. The
//! text is data for the voice, nothing else; it never reaches a shell or a command.

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

/// Reads `text` (at most `MAX_READ_ALOUD` characters) and ends early on `stop()`. Blocking.
pub fn read_aloud(text: &str) -> Result<(), String> {
    read(&text.chars().take(MAX_READ_ALOUD).collect::<String>(), None)
}

/// T29.8: an answer in the voice of Windows for `lang`, at most `MAX_ANSWER` characters; ends early on `stop()`. Blocking.
pub fn answer(text: &str, lang: &str) -> Result<(), String> {
    read(&text.chars().take(MAX_ANSWER).collect::<String>(), language_id(lang))
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

fn read(text: &str, language: Option<&str>) -> Result<(), String> {
    let turn = next_turn();
    SPEAKING.fetch_add(1, Ordering::SeqCst);
    let done = read_aloud_impl(text, language, turn);
    SPEAKING.fetch_sub(1, Ordering::SeqCst);
    done
}

/// Chooses an installed voice of the language; the default voice stays when there is none.
///
/// # Safety
/// COM is initialised on this thread.
#[cfg(windows)]
unsafe fn choose_voice(voice: &windows::Win32::Media::Speech::ISpVoice, language: &str) {
    use windows::core::{w, HSTRING};
    use windows::Win32::Media::Speech::{ISpObjectTokenCategory, SpObjectTokenCategory};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    // SAFETY: the caller initialised COM; every object lives inside this function.
    unsafe {
        let Ok(voices) = CoCreateInstance::<_, ISpObjectTokenCategory>(&SpObjectTokenCategory, None, CLSCTX_ALL) else { return };
        if voices.SetId(w!("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Voices"), false).is_err() {
            return;
        }
        for id in language.split(';') {
            let Ok(found) = voices.EnumTokens(&HSTRING::from(format!("Language={id}")), None) else { continue };
            let mut token = None;
            if found.Next(1, &mut token, None).is_ok() {
                if let Some(token) = token {
                    let _ = voice.SetVoice(&token);
                    return;
                }
            }
        }
    }
}

#[cfg(windows)]
fn read_aloud_impl(text: &str, language: Option<&str>, turn: u64) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Media::Speech::{ISpVoice, SpVoice};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};
    // SAFETY: COM on this thread only; the voice is dropped before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let spoken = (|| -> Result<(), String> {
            let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL).map_err(|e| format!("No system voice: {e}"))?;
            if let Some(language) = language {
                choose_voice(&voice, language);
            }
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
fn read_aloud_impl(_: &str, _: Option<&str>, _: u64) -> Result<(), String> {
    Err("unsupported platform".into())
}

/// Reads `text` aloud with the default voice of the system (the "speak" button of an answer). Blocks until it is done or
/// stopped (call it from a blocking thread).
pub fn speak(text: &str) -> Result<(), String> {
    read(&text.chars().take(MAX_SPOKEN).collect::<String>(), None)
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
    fn a_stop_and_a_newer_reading_end_the_one_before() {
        let first = next_turn();
        assert!(still(first));
        let second = next_turn();
        assert!(!still(first) && still(second));
        stop();
        assert!(!still(second));
    }
}
