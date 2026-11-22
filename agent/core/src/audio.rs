//! `audio` collector and the `audio` actions (DATA_MODEL.md 6.6, 2.3): volume and mute of the default output device and
//! the mute of the default microphone, read through Core Audio every 500 ms and published as variables.
//!
//! Variables: `audio.volume` (0..100), `audio.muted`, `mic.muted`. A missing device removes its variables.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::model::{AudioOp, VarValue};
use crate::state::Store;

const POLL: Duration = Duration::from_millis(500);

/// What the default devices say right now; `None` = no such device.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioState {
    /// Output volume, whole percent 0..=100.
    pub volume: Option<u8>,
    pub out_muted: Option<bool>,
    pub mic_muted: Option<bool>,
}

/// Percent from a Core Audio scalar (0.0..=1.0), rounded and clamped.
pub fn percent(scalar: f32) -> u8 {
    (scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// Puts the state into the variables: a value is set, a missing device removes its variable. Other variables are untouched.
pub fn apply_vars(vars: &mut BTreeMap<String, VarValue>, state: &AudioState) {
    let mut set = |name: &str, value: Option<VarValue>| match value {
        Some(v) => {
            vars.insert(name.to_string(), v);
        }
        None => {
            vars.remove(name);
        }
    };
    set("audio.volume", state.volume.map(|v| VarValue::Num(v.into())));
    set("audio.muted", state.out_muted.map(VarValue::Bool));
    set("mic.muted", state.mic_muted.map(VarValue::Bool));
}

/// Removes the variables of this collector (it was switched off).
pub fn clear_vars(vars: &mut BTreeMap<String, VarValue>) {
    apply_vars(vars, &AudioState::default());
}

/// Reads the default devices. Other platforms: nothing.
pub fn read_state() -> AudioState {
    #[cfg(windows)]
    {
        win::read_state()
    }
    #[cfg(not(windows))]
    {
        AudioState::default()
    }
}

/// `mic_toggle` / `out_mute_toggle`: flips the mute of the default device. No device: `No microphone` / `No audio output`.
pub fn toggle(op: AudioOp) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::toggle(op)
    }
    #[cfg(not(windows))]
    {
        let _ = op;
        Err("unsupported platform".to_string())
    }
}

/// The scalar Core Audio takes for a whole percent (values above 100 are 100).
pub fn scalar(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

/// Sets the volume of the default output device to `percent` (voice tool `volume set`) and unmutes it.
pub fn set_volume(percent: u8) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::set_volume(percent)
    }
    #[cfg(not(windows))]
    {
        let _ = percent;
        Err("unsupported platform".to_string())
    }
}

pub struct AudioCollector {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AudioCollector {
    /// Starts the polling thread. It is a plain thread like the other collectors: Core Audio is COM and wants one thread of its own.
    pub fn start(store: Store) -> AudioCollector {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("decka-audio-collector".to_string())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    let state = read_state();
                    store.update_vars(|vars| apply_vars(vars, &state));
                    // Short slices: a stop request is honoured quickly.
                    let end = Instant::now() + POLL;
                    while Instant::now() < end && !flag.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            })
            .expect("spawn audio collector thread");
        AudioCollector { stop, thread: Some(thread) }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for AudioCollector {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(windows)]
mod win {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{eCapture, eConsole, eRender, EDataFlow, IMMDeviceEnumerator, MMDeviceEnumerator};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

    use super::{percent, AudioOp, AudioState};

    /// Runs `f` on a thread with COM initialised; the matching `CoUninitialize` only when this call did the initialising.
    fn with_com<T>(f: impl FnOnce() -> T) -> T {
        // SAFETY: plain COM initialisation of the calling thread.
        let initialised = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        let result = f();
        if initialised {
            // SAFETY: balances the successful CoInitializeEx above; every COM object of `f` is gone by now.
            unsafe { CoUninitialize() };
        }
        result
    }

    /// The volume interface of the default device of `flow`; `None` when there is no such device.
    fn endpoint(flow: EDataFlow) -> Option<IAudioEndpointVolume> {
        // SAFETY: COM calls on an initialised thread; the objects live only inside this function and its caller.
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let device = enumerator.GetDefaultAudioEndpoint(flow, eConsole).ok()?;
            device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()
        }
    }

    pub fn read_state() -> AudioState {
        with_com(|| {
            // SAFETY: the interfaces come from `endpoint` on this thread.
            let output = endpoint(eRender);
            let mic = endpoint(eCapture);
            unsafe {
                AudioState {
                    volume: output.as_ref().and_then(|e| e.GetMasterVolumeLevelScalar().ok()).map(percent),
                    out_muted: output.as_ref().and_then(|e| e.GetMute().ok()).map(|b| b.as_bool()),
                    mic_muted: mic.as_ref().and_then(|e| e.GetMute().ok()).map(|b| b.as_bool()),
                }
            }
        })
    }

    pub fn set_volume(percent: u8) -> Result<(), String> {
        with_com(|| {
            let volume = endpoint(eRender).ok_or("No audio output")?;
            // SAFETY: `volume` was activated on this thread.
            unsafe {
                volume.SetMasterVolumeLevelScalar(super::scalar(percent), std::ptr::null()).map_err(|e| e.message())?;
                // A level set by voice is meant to be heard.
                if percent > 0 {
                    let _ = volume.SetMute(false, std::ptr::null());
                }
            }
            Ok(())
        })
    }

    pub fn toggle(op: AudioOp) -> Result<(), String> {
        let (flow, missing) = match op {
            AudioOp::MicToggle => (eCapture, "No microphone"),
            AudioOp::OutMuteToggle => (eRender, "No audio output"),
        };
        with_com(|| {
            let volume = endpoint(flow).ok_or(missing)?;
            // SAFETY: `volume` was activated on this thread.
            unsafe {
                let muted = volume.GetMute().map_err(|e| e.message())?.as_bool();
                volume.SetMute(!muted, std::ptr::null()).map_err(|e| e.message())?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(vars: &BTreeMap<String, VarValue>) -> serde_json::Value {
        serde_json::to_value(vars).unwrap()
    }

    #[test]
    fn the_scalar_becomes_a_whole_percent() {
        assert_eq!([percent(0.0), percent(0.42), percent(0.425), percent(1.0), percent(1.7), percent(-1.0)], [0, 42, 43, 100, 100, 0]);
    }

    #[test]
    fn a_percent_becomes_a_scalar_and_back() {
        assert_eq!([scalar(0), scalar(50), scalar(100), scalar(250)], [0.0, 0.5, 1.0, 1.0]);
        assert!((0..=100u8).all(|p| percent(scalar(p)) == p), "the round trip loses nothing");
    }

    #[test]
    fn the_state_becomes_three_variables() {
        let mut vars = BTreeMap::new();
        apply_vars(&mut vars, &AudioState { volume: Some(42), out_muted: Some(false), mic_muted: Some(true) });
        assert_eq!(json(&vars), serde_json::json!({ "audio.volume": 42, "audio.muted": false, "mic.muted": true }));
    }

    #[test]
    fn a_missing_device_removes_its_variables_and_only_those() {
        let mut vars = BTreeMap::new();
        vars.insert("my.flag".to_string(), VarValue::Bool(true));
        apply_vars(&mut vars, &AudioState { volume: Some(10), out_muted: Some(true), mic_muted: Some(false) });
        apply_vars(&mut vars, &AudioState { volume: Some(10), out_muted: Some(true), mic_muted: None });
        assert_eq!(json(&vars), serde_json::json!({ "my.flag": true, "audio.volume": 10, "audio.muted": true }));
        clear_vars(&mut vars);
        assert_eq!(json(&vars), serde_json::json!({ "my.flag": true }));
    }

    #[test]
    fn an_unchanged_state_is_not_broadcast() {
        let store = Store::new(serde_json::json!({}));
        let mut rx = store.subscribe();
        let state = AudioState { volume: Some(30), out_muted: Some(false), mic_muted: Some(false) };
        store.update_vars(|v| apply_vars(v, &state));
        assert!(rx.try_recv().is_ok());
        store.update_vars(|v| apply_vars(v, &state));
        assert!(rx.try_recv().is_err(), "the same values were sent again");
        store.update_vars(|v| apply_vars(v, &AudioState { volume: Some(32), ..state }));
        assert!(rx.try_recv().is_ok());
    }
}
