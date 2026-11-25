//! Reading an answer aloud (T26.3 `speak`): the system voice of Windows (SAPI), blocking until the text has been read. The
//! text is data for the voice, nothing else; it never reaches a shell or a command.

/// The longest text that is read aloud as an answer.
pub const MAX_SPOKEN: usize = 1_200;
/// The longest text that is read aloud on "read aloud" (T27.9).
pub const MAX_READ_ALOUD: usize = 6_000;

static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// "Stop": ends the reading that is going on (if any).
pub fn stop() {
    STOP.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Reads `text` (at most `MAX_READ_ALOUD` characters) and ends early on `stop()`. Blocking.
pub fn read_aloud(text: &str) -> Result<(), String> {
    STOP.store(false, std::sync::atomic::Ordering::Relaxed);
    read_aloud_impl(&text.chars().take(MAX_READ_ALOUD).collect::<String>())
}

#[cfg(windows)]
fn read_aloud_impl(text: &str) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Media::Speech::{ISpVoice, SpVoice};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};
    // SAFETY: COM on this thread only; the voice is dropped before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let spoken = (|| -> Result<(), String> {
            let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL).map_err(|e| format!("No system voice: {e}"))?;
            // SPF_ASYNC (1): the call returns at once and the text is polled for the end or for "stop".
            voice.Speak(&HSTRING::from(text), 1, None).map_err(|e| format!("The voice failed: {e}"))?;
            let mut status = windows::Win32::Media::Speech::SPVOICESTATUS::default();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(80));
                // SPRS_DONE (1): the text has been read.
                if voice.GetStatus(&mut status, std::ptr::null_mut()).is_ok() && status.dwRunningState & 1 != 0 {
                    break;
                }
                if STOP.load(std::sync::atomic::Ordering::Relaxed) {
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
fn read_aloud_impl(_: &str) -> Result<(), String> {
    Err("unsupported platform".into())
}

/// Reads `text` aloud with the default voice of the system. Blocks until it is done (call it from a blocking thread).
#[cfg(windows)]
pub fn speak(text: &str) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Media::Speech::{ISpVoice, SpVoice};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

    let text: String = text.chars().take(MAX_SPOKEN).collect();
    // SAFETY: COM on this thread only; the voice is dropped before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let spoken = (|| -> Result<(), String> {
            let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL).map_err(|e| format!("No system voice: {e}"))?;
            voice.Speak(&HSTRING::from(text.as_str()), 0, None).map_err(|e| format!("The voice failed: {e}"))?;
            Ok(())
        })();
        if initialised {
            CoUninitialize();
        }
        spoken
    }
}

#[cfg(not(windows))]
pub fn speak(_: &str) -> Result<(), String> {
    Err("unsupported platform".into())
}
