//! The microphone of the PC (DATA_MODEL.md 12.6): `cpal` delivers frames in whatever format the device has; this module
//! turns them into 16 kHz mono PCM16 chunks of 100 ms, with the level for the pill and the end-of-speech decision.
//! The conversion (`Capture`, `choose_device`) is pure; the thin layer at the bottom talks to `cpal` on a thread of its own.

use super::audio::{calculate_rms, resample_to_16k};
use super::endpoint::{Decision, Endpointer, LevelSmoother, Verdict};

/// One chunk is 100 ms of audio, as on the phone (PROTOCOL.md 3).
const CHUNK_MS: u32 = 100;

/// What `Capture` hands over for every 100 ms.
#[derive(Debug, Clone, PartialEq)]
pub struct Fed {
    /// 16 kHz mono PCM16, little endian.
    pub pcm: Vec<u8>,
    /// 0..1 for the wave of the pill (smoothed: rises at once, falls in 150 ms).
    pub level: f32,
    /// The end-of-phrase decision (DATA_MODEL.md 12.6a).
    pub verdict: Verdict,
    /// What the endpoint saw (for oice.log).
    pub decision: Decision,
}

/// One line of %LOCALAPPDATA%\Decka\logs\voice.log: numbers about the decisions only, never sound or text. The file is cut to
/// its last half when it grows past 256 KB. Failures to write are ignored.
pub fn log_line(line: &str) {
    use std::io::Write;
    let Some(base) = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).or_else(dirs::data_local_dir) else { return };
    let dir = base.join("Decka").join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("voice.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let keep = text.len() / 2;
            let cut = text.char_indices().map(|(i, _)| i).find(|i| *i >= keep).unwrap_or(0);
            let _ = std::fs::write(&path, &text[cut..]);
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "{secs} {line}");
    }
}

/// A chunk this quiet is digital silence (all zeros), not a quiet room.
pub const DIGITAL_SILENCE: f32 = 0.00003;
/// This many chunks of digital silence in a row (1 s) mean the stream is dead.
pub const DEAD_CHUNKS: u32 = 10;

static RING: std::sync::Mutex<std::collections::VecDeque<String>> = std::sync::Mutex::new(std::collections::VecDeque::new());

/// Keeps the last 50 endpoint decisions in memory, whatever the limit of logged requests is.
pub fn ring_push(line: String) {
    if let Ok(mut r) = RING.lock() {
        if r.len() >= 50 {
            r.pop_front();
        }
        r.push_back(line);
    }
}

pub fn ring_clear() {
    if let Ok(mut r) = RING.lock() {
        r.clear();
    }
}

/// Writes the ring to voice.log (for NoSpeech and CaptureFailed).
pub fn ring_dump(reason: &str) {
    let lines: Vec<String> = RING.lock().map(|mut r| r.drain(..).collect()).unwrap_or_default();
    log_line(&format!("ring dump ({reason}): {} decisions", lines.len()));
    for l in lines {
        log_line(&format!("  {l}"));
    }
}
/// The line for a chunk: rms, floor, threshold and the running counters.
pub fn decision_line(d: &Decision, verdict: Verdict) -> String {
    format!("chunk rms={:.4} floor={:.4} thr={:.4} speech_ms={} silence_ms={} verdict={:?}", d.rms, d.floor, d.threshold, d.speech_ms, d.silence_ms, verdict)
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
    endpointer: Endpointer,
    smoother: LevelSmoother,
}

impl Capture {
    pub fn new(rate: u32, channels: u16) -> Capture {
        let chunk_len = (rate as usize * CHUNK_MS as usize / 1000).max(1);
        Capture {
            channels,
            pending: Vec::new(),
            chunk_len,
            rate,
            endpointer: Endpointer::new(),
            smoother: LevelSmoother::default(),
        }
    }

    /// Takes interleaved frames of any length; returns every complete 100 ms chunk.
    pub fn feed(&mut self, interleaved: &[i16]) -> Vec<Fed> {
        self.pending.extend(downmix(interleaved, self.channels));
        let mut out = Vec::new();
        while self.pending.len() >= self.chunk_len {
            let chunk: Vec<i16> = self.pending.drain(..self.chunk_len).collect();
            let mono16 = resample_to_16k(&chunk, self.rate);
            let rms = calculate_rms(&mono16) / 32768.0;
            let verdict = self.endpointer.feed(rms);
            let level = self.smoother.feed(rms);
            out.push(Fed { pcm: mono16.iter().flat_map(|s| s.to_le_bytes()).collect(), level, verdict, decision: self.endpointer.last() });
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;
    use std::thread::JoinHandle;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleFormat;

    use super::{choose_device, DEAD_CHUNKS, DIGITAL_SILENCE, f32_to_i16, i32_to_i16, u16_to_i16, unique_names, Capture, Choice, Fed};

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

    type Sink = Arc<Mutex<Box<dyn FnMut(Fed) + Send>>>;

    /// Opens the stream. `dead` is raised when a second of digital silence (all zeros) comes in a row.
    fn build(wanted: &Option<String>, log: bool, sink: &Sink, on_error: &Arc<dyn Fn(String) + Send + Sync>, dead: &Arc<AtomicBool>) -> Result<cpal::Stream, String> {
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
        if log {
            super::log_line(&format!("device rate={rate} channels={channels} format={format:?}"));
        }
        let config = supported.config();
        let mut capture = Capture::new(rate, channels);
        // A buffer under/overrun is a glitch of one block, not the end of the device (it used to close every recording).
        let on_error = on_error.clone();
        let on_err = move |e: cpal::Error| {
            if e.kind() == cpal::ErrorKind::Xrun {
                super::log_line("xrun ignored");
            } else {
                on_error(format!("Microphone stopped: {e}"));
            }
        };
        let (sink, dead) = (sink.clone(), dead.clone());
        let mut zeros = 0u32;
        let mut emit = move |frames: Vec<i16>| {
            for fed in capture.feed(&frames) {
                // After an xrun some drivers hand out zeros for good: a second of them is a dead stream.
                if fed.decision.rms < DIGITAL_SILENCE {
                    zeros += 1;
                    if zeros >= DEAD_CHUNKS {
                        dead.store(true, Ordering::SeqCst);
                    }
                } else {
                    zeros = 0;
                }
                if let Ok(mut f) = sink.lock() {
                    f(fed);
                }
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
    }

    /// Opens the microphone (`wanted` by name, else the default) and calls `on_chunk` for every 100 ms. The stream lives on a
    /// thread of its own (it is not `Send` on Windows); an error while recording goes to `on_error`. A stream that gives only
    /// digital silence for a second is reopened once.
    pub fn start(
        wanted: Option<String>,
        log: bool,
        on_chunk: impl FnMut(Fed) + Send + 'static,
        on_error: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Mic, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let sink: Sink = Arc::new(Mutex::new(Box::new(on_chunk)));
        let on_error: Arc<dyn Fn(String) + Send + Sync> = Arc::new(on_error);
        let join = std::thread::Builder::new()
            .name("decka-mic".into())
            .spawn(move || {
                let dead = Arc::new(AtomicBool::new(false));
                match build(&wanted, log, &sink, &on_error, &dead) {
                    Ok(mut stream) => {
                        let _ = ready_tx.send(Ok(()));
                        let mut reopened = false;
                        loop {
                            match stop_rx.recv_timeout(Duration::from_millis(100)) {
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                _ => break, // stop, or the handle was dropped
                            }
                            if !reopened && dead.load(Ordering::SeqCst) {
                                reopened = true;
                                drop(stream);
                                dead.store(false, Ordering::SeqCst);
                                match build(&wanted, log, &sink, &on_error, &dead) {
                                    Ok(s) => {
                                        super::log_line("stream reopened");
                                        stream = s;
                                    }
                                    Err(e) => {
                                        on_error(e);
                                        return;
                                    }
                                }
                            }
                        }
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
    fn speech_then_silence_ends_the_recording_and_silence_alone_says_no_speech() {
        let mut c = Capture::new(16_000, 1);
        let first = (0..60).map(|_| c.feed(&vec![0; 1600]).remove(0).verdict).position(|v| v != Verdict::Continue);
        assert_eq!(first, Some(49), "5 s of silence: nothing was said");
        let mut c = Capture::new(16_000, 1);
        for _ in 0..3 {
            c.feed(&vec![0; 1600]);
        }
        for _ in 0..5 {
            assert_eq!(c.feed(&tone(1600, 4000))[0].verdict, Verdict::Continue);
        }
        let ended = (0..30).map(|_| c.feed(&vec![0; 1600]).remove(0).verdict).position(|v| v == Verdict::End);
        // 2 s of silence = the 20th chunk.
        assert_eq!(ended, Some(19));
    }

    /// A recording as the microphone delivers it: stereo, 48 kHz, i16. A segment is `(seconds, rms of the sound, rms of the noise)`;
    /// the sound is a few voiced partials with a syllable rhythm, the noise is a deterministic hash noise.
    fn recording(segments: &[(f32, f32, f32)]) -> Vec<i16> {
        let (rate, mut n) = (48_000usize, 0u64);
        let mut out = Vec::new();
        for &(secs, speech, noise) in segments {
            for i in 0..(secs * rate as f32) as usize {
                let t = i as f32 / rate as f32;
                let syllable = if speech > 0.0 { 0.55 + 0.45 * (t * 14.0).sin().abs() } else { 0.0 };
                let voice = ((t * 220.0 * std::f32::consts::TAU).sin() + 0.5 * (t * 880.0 * std::f32::consts::TAU).sin()) * 0.63 * syllable;
                n = n.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let hiss = ((n >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 3.46;
                let v = ((voice * speech * 2.2 + hiss * noise) * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
                out.extend([v, v]);
            }
        }
        out
    }

    /// Runs a recording through the capture in pieces of the size a device delivers; returns the second of the end verdict.
    fn ends_at(rec: &[i16]) -> Option<f32> {
        let mut c = Capture::new(48_000, 2);
        let mut chunks = 0;
        for piece in rec.chunks(960 * 2) {
            for fed in c.feed(piece) {
                chunks += 1;
                if fed.verdict == Verdict::End {
                    return Some(chunks as f32 / 10.0);
                }
            }
        }
        None
    }

    #[test]
    fn a_recorded_phrase_ends_within_2_5_seconds_after_the_speech_on_any_microphone() {
        // (name, lead-in noise seconds, speech seconds, speech level, noise level)
        let cases: [(&str, f32, f32, f32, f32); 5] = [("ordinary", 0.5, 2.0, 0.08, 0.004), ("quiet", 0.5, 2.0, 0.02, 0.0015), ("noisy room", 0.5, 2.0, 0.1, 0.012), ("speech at once", 0.0, 2.5, 0.08, 0.004), ("very quiet", 0.3, 2.0, 0.03, 0.001)];
        for (name, lead, speech, level, noise) in cases {
            let rec = recording(&[(lead.max(0.05), 0.0, noise), (speech, level, noise), (4.0, 0.0, noise)]);
            let speech_end = lead.max(0.05) + speech;
            let end = ends_at(&rec).unwrap_or_else(|| panic!("{name}: the recording never ended"));
            assert!(end >= speech_end + 1.5 && end <= speech_end + 2.5, "{name}: speech ended at {speech_end} s, the verdict came at {end} s");
        }
    }

    #[test]
    fn a_microphone_that_raises_its_gain_in_the_pause_still_ends_the_phrase() {
        // Automatic gain: the noise after the speech is four times louder than before it.
        let rec = recording(&[(0.5, 0.0, 0.004), (2.0, 0.08, 0.004), (5.0, 0.0, 0.017)]);
        let end = ends_at(&rec).expect("the recording never ended");
        assert!(end <= 4.5 + 0.1, "the verdict came at {end} s");
    }

    #[test]
    fn the_decision_of_a_chunk_is_in_the_log_line() {
        let mut c = Capture::new(16_000, 1);
        let fed = c.feed(&tone(1600, 3000)).remove(0);
        let line = decision_line(&fed.decision, fed.verdict);
        assert!(line.starts_with("chunk rms=0.0") && line.contains("floor=") && line.contains("thr=0.0300") && line.ends_with("verdict=Continue"), "{line}");
    }

    #[test]
    fn a_recording_is_cut_at_20_seconds() {
        let mut c = Capture::new(16_000, 1);
        let last = (0..300).map(|_| c.feed(&tone(1600, 4000)).remove(0).verdict).position(|v| v == Verdict::End);
        assert_eq!(last, Some(199));
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
