//! Reading an answer aloud (T26.3 `speak`): the system voice of Windows (SAPI), blocking until the text has been read. The
//! text is data for the voice, nothing else; it never reaches a shell or a command.

/// The longest text that is read aloud.
pub const MAX_SPOKEN: usize = 1_200;

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
