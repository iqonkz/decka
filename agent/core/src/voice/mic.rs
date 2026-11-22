//! The microphone of the PC (DATA_MODEL.md 12.6): `cpal` delivers frames in whatever format the device has; this module
//! turns them into 16 kHz mono PCM16 chunks of 100 ms, with the level for the pill and the end-of-speech decision.
//! The conversion (`Capture`, `choose_device`) is pure; the thin layer at the bottom talks to `cpal` on a thread of its own.

use super::audio::{calculate_rms, resample_to_16k, SilenceDecision, SilenceDetector};
use super::MAX_RECORDING_SECONDS;

/// One chunk is 100 ms of audio, as on the phone (PROTOCOL.md 3).
const CHUNK_MS: u32 = 100;
/// Silence that ends a recording after speech (DATA_MODEL.md 12.4 item 1).
pub const TRAILING_SILENCE_MS: u32 = 1200;
/// RMS (16-bit scale) of ordinary speech at a desk microphone is a few thousand; this is the floor of speech.
pub const SPEECH_RMS: f32 = 350.0;
/// An RMS at which the pill wave is at full height.
const LEVEL_FULL_RMS: f32 = 6000.0;

/// What `Capture` hands over for every 100 ms.
#[derive(Debug, Clone, PartialEq)]
pub struct Fed {
    /// 16 kHz mono PCM16, little endian.
    pub pcm: Vec<u8>,
    /// 0..1 for the wave of the pill.
    pub level: f32,
    pub decision: SilenceDecision,
}

/// Sample conversions of the formats `cpal` hands out.
pub fn f32_to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

pub fn u16_to_i16(x: u16) -> i16 {
    (x as i32 - 32768) as i16
}

pub fn i32_to_i16(x: i32) -> i16 {
    (x >> 16) as i16
}

/// Interleaved frames -> mono (the average of the channels).
pub fn downmix(interleaved: &[i16], channels: u16) -> Vec<i16> {
    let n = channels.max(1) as usize;
    if n == 1 {
        return interleaved.to_vec();
    }
    interleaved.chunks_exact(n).map(|f| (f.iter().map(|s| *s as i32).sum::<i32>() / n as i32) as i16).collect()
}

/// The device-rate samples -> 16 kHz chunks of 100 ms with the silence decision.
pub struct Capture {
    channels: u16,
    /// Mono samples at the device rate, not yet a whole chunk.
    pending: Vec<i16>,
    chunk_len: usize,
    rate: u32,
    detector: SilenceDetector,
}

impl Capture {
    pub fn new(rate: u32, channels: u16) -> Capture {
        let chunk_len = (rate as usize * CHUNK_MS as usize / 1000).max(1);
        Capture {
            channels,
            pending: Vec::new(),
            chunk_len,
            rate,
            detector: SilenceDetector::new(16_000, SPEECH_RMS, TRAILING_SILENCE_MS, MAX_RECORDING_SECONDS as u32),
        }
    }

    /// Takes interleaved frames of any length; returns every complete 100 ms chunk.
    pub fn feed(&mut self, interleaved: &[i16]) -> Vec<Fed> {
        self.pending.extend(downmix(interleaved, self.channels));
        let mut out = Vec::new();
        while self.pending.len() >= self.chunk_len {
            let chunk: Vec<i16> = self.pending.drain(..self.chunk_len).collect();
            let mono16 = resample_to_16k(&chunk, self.rate);
            let decision = self.detector.process_chunk(&mono16);
            let level = (calculate_rms(&mono16) / LEVEL_FULL_RMS).clamp(0.0, 1.0);
            out.push(Fed { pcm: mono16.iter().flat_map(|s| s.to_le_bytes()).collect(), level, decision });
        }
        out
    }
}

/// Which device records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// The default microphone of Windows.
    Default,
    /// The device with this index in the list.
    Named(usize),
    /// Nothing to record with.
    NoMic,
}

/// The chosen name if the device is still there, else the default one (an unplugged headset must not silence voice control).
pub fn choose_device(names: &[String], wanted: Option<&str>) -> Choice {
    if names.is_empty() {
        return Choice::NoMic;
    }
    match wanted.filter(|w| !w.is_empty()).and_then(|w| names.iter().position(|n| n == w)) {
        Some(i) => Choice::Named(i),
        None => Choice::Default,
    }
}

/// Device names without duplicates (Windows lists one microphone once per endpoint), order kept.
pub fn unique_names(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        if !n.is_empty() && !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

// ---- The thin layer over cpal -----------------------------------------------------------------------------------

#[cfg(windows)]
mod device {
    use std::sync::mpsc;
    use std::thread::JoinHandle;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleFormat;

    use super::{choose_device, f32_to_i16, i32_to_i16, u16_to_i16, unique_names, Capture, Choice, Fed};

    fn input_names(host: &cpal::Host) -> Vec<String> {
        host.input_devices().map(|list| unique_names(list.filter_map(|d| d.description().ok().map(|x| x.name().to_string())))).unwrap_or_default()
    }

    /// The names of the microphones for the settings window.
    pub fn list_mics() -> Vec<String> {
        input_names(&cpal::default_host())
    }

    /// A running recording; dropping it (or `stop`) closes the device.
    pub struct Mic {
        stop: Option<mpsc::Sender<()>>,
        join: Option<JoinHandle<()>>,
    }

    impl Mic {
        pub fn stop(&mut self) {
            if let Some(tx) = self.stop.take() {
                let _ = tx.send(());
            }
            if let Some(j) = self.join.take() {
                let _ = j.join();
            }
        }
    }

    impl Drop for Mic {
        fn drop(&mut self) {
            self.stop();
        }
    }

    /// Opens the microphone (`wanted` by name, else the default) and calls `on_chunk` for every 100 ms. The stream lives on a
    /// thread of its own (it is not `Send` on Windows); an error while recording goes to `on_error`.
    pub fn start(
        wanted: Option<String>,
        mut on_chunk: impl FnMut(Fed) + Send + 'static,
        on_error: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Mic, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let join = std::thread::Builder::new()
            .name("decka-mic".into())
            .spawn(move || {
                let built = (|| -> Result<cpal::Stream, String> {
                    let host = cpal::default_host();
                    let names = input_names(&host);
                    let device = match choose_device(&names, wanted.as_deref()) {
                        Choice::NoMic => return Err("No microphone found".to_string()),
                        Choice::Default => host.default_input_device().ok_or_else(|| "No microphone found".to_string())?,
                        Choice::Named(i) => host
                            .input_devices()
                            .map_err(|e| e.to_string())?
                            .find(|d| d.description().is_ok_and(|x| x.name() == names[i]))
                            .or_else(|| host.default_input_device())
                            .ok_or_else(|| "No microphone found".to_string())?,
                    };
                    let supported = device.default_input_config().map_err(|e| format!("Microphone is not available: {e}"))?;
                    let (rate, channels, format) = (supported.sample_rate(), supported.channels(), supported.sample_format());
                    let config = supported.config();
                    let mut capture = Capture::new(rate, channels);
                    let on_err = move |e: cpal::Error| on_error(format!("Microphone stopped: {e}"));
                    let mut emit = move |frames: Vec<i16>| {
                        for fed in capture.feed(&frames) {
                            on_chunk(fed);
                        }
                    };
                    let stream = match format {
                        SampleFormat::F32 => device.build_input_stream(config, move |d: &[f32], _: &_| emit(d.iter().map(|s| f32_to_i16(*s)).collect()), on_err, None),
                        SampleFormat::I16 => device.build_input_stream(config, move |d: &[i16], _: &_| emit(d.to_vec()), on_err, None),
                        SampleFormat::U16 => device.build_input_stream(config, move |d: &[u16], _: &_| emit(d.iter().map(|s| u16_to_i16(*s)).collect()), on_err, None),
                        SampleFormat::I32 => device.build_input_stream(config, move |d: &[i32], _: &_| emit(d.iter().map(|s| i32_to_i16(*s)).collect()), on_err, None),
                        other => return Err(format!("Microphone format {other:?} is not supported")),
                    }
                    .map_err(|e| format!("Microphone is not available: {e}"))?;
                    stream.play().map_err(|e| format!("Microphone is not available: {e}"))?;
                    Ok(stream)
                })();
                match built {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        let _ = stop_rx.recv(); // returns on stop or when the handle is dropped
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        match ready_rx.recv().map_err(|_| "Microphone thread died".to_string())? {
            Ok(()) => Ok(Mic { stop: Some(stop_tx), join: Some(join) }),
            Err(e) => {
                let _ = join.join();
                Err(e)
            }
        }
    }
}

#[cfg(windows)]
pub use device::{list_mics, start, Mic};

#[cfg(not(windows))]
pub fn list_mics() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(len: usize, amp: i16) -> Vec<i16> {
        (0..len).map(|i| if i % 2 == 0 { amp } else { -amp }).collect()
    }

    #[test]
    fn sample_formats_are_converted_to_i16() {
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(1.0), i16::MAX);
        assert_eq!(f32_to_i16(-2.0), -i16::MAX);
        assert_eq!(u16_to_i16(32768), 0);
        assert_eq!(u16_to_i16(0), i16::MIN);
        assert_eq!(i32_to_i16(i32::MAX), i16::MAX);
        assert_eq!(i32_to_i16(-65536), -1);
    }

    #[test]
    fn stereo_is_averaged_to_mono() {
        assert_eq!(downmix(&[100, 300, -50, 50, 7, 9], 2), [200, 0, 8]);
        assert_eq!(downmix(&[1, 2, 3], 1), [1, 2, 3]);
        // A trailing half frame is dropped.
        assert_eq!(downmix(&[10, 20, 30], 2), [15]);
    }

    #[test]
    fn capture_makes_100_ms_chunks_of_16k_from_any_rate_and_keeps_the_rest() {
        let mut c = Capture::new(48_000, 2);
        // 4800 frames of 2 channels = 100 ms at 48 kHz, delivered in odd pieces.
        assert!(c.feed(&tone(3000, 100)).is_empty());
        let chunks = c.feed(&tone(6600, 100));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].pcm.len(), 1600 * 2, "100 ms of 16 kHz PCM16");
        // 4800 frames were used exactly; 4200 frames are not a chunk yet, 600 more complete it.
        assert!(c.feed(&tone(8400, 100)).is_empty());
        assert_eq!(c.feed(&tone(1200, 100)).len(), 1);
    }

    #[test]
    fn the_level_follows_the_loudness_and_stays_in_range() {
        let mut c = Capture::new(16_000, 1);
        let quiet = c.feed(&vec![0; 1600]);
        let loud = c.feed(&tone(1600, 30_000));
        assert_eq!(quiet[0].level, 0.0);
        assert_eq!(loud[0].level, 1.0);
        let mid = c.feed(&tone(1600, 3000));
        assert!(mid[0].level > 0.3 && mid[0].level < 0.7, "{}", mid[0].level);
    }

    #[test]
    fn speech_then_silence_ends_the_recording_and_silence_alone_never_does() {
        let mut c = Capture::new(16_000, 1);
        for _ in 0..30 {
            assert!(c.feed(&vec![0; 1600]).iter().all(|f| f.decision == SilenceDecision::Continue), "silence before speech");
        }
        let mut c = Capture::new(16_000, 1);
        for _ in 0..5 {
            assert_eq!(c.feed(&tone(1600, 4000))[0].decision, SilenceDecision::Continue);
        }
        let ended = (0..20).map(|_| c.feed(&vec![0; 1600]).remove(0).decision).position(|d| d == SilenceDecision::EndOfSpeech);
        // 1.2 s of silence = the 12th chunk.
        assert_eq!(ended, Some(11));
    }

    #[test]
    fn a_recording_is_cut_at_30_seconds() {
        let mut c = Capture::new(16_000, 1);
        let last = (0..300).map(|_| c.feed(&tone(1600, 4000)).remove(0).decision).last();
        assert_eq!(last, Some(SilenceDecision::MaxDurationExceeded));
    }

    #[test]
    fn the_chosen_microphone_is_used_while_it_exists() {
        let names = vec!["Headset".to_string(), "Webcam".to_string()];
        assert_eq!(choose_device(&names, Some("Webcam")), Choice::Named(1));
        assert_eq!(choose_device(&names, None), Choice::Default);
        assert_eq!(choose_device(&names, Some("")), Choice::Default);
        assert_eq!(choose_device(&names, Some("Unplugged")), Choice::Default, "falls back to the default microphone");
        assert_eq!(choose_device(&[], Some("Headset")), Choice::NoMic);
        assert_eq!(unique_names(["A".to_string(), "".to_string(), "B".to_string(), "A".to_string()]), ["A", "B"]);
    }
}
