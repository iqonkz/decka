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
    let Some(path) = log_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
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

/// `%LOCALAPPDATA%\Decka\logs\voice.log`; a test copy (`DECKA_CONFIG_DIR`) writes inside its own folder.
pub fn log_path() -> Option<std::path::PathBuf> {
    if let Some(own) = std::env::var_os("DECKA_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(std::path::PathBuf::from(own).join("logs").join("voice.log"));
    }
    let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).or_else(dirs::data_local_dir)?;
    Some(base.join("Decka").join("logs").join("voice.log"))
}

/// Versions before 0.51.0 wrote the texts of results into `voice.log`: those lines are cut to their first words.
/// `None`: nothing to cut.
pub fn scrub_log(text: &str) -> Option<String> {
    let mut changed = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            for (mark, keep) in [(" executed: ok ", " executed: ok"), (" executed: error ", " executed: error"), (" but refused: ", " but refused")] {
                if let Some(at) = line.find(mark) {
                    changed = true;
                    return format!("{}{keep}", &line[..at]);
                }
            }
            // The following lines of a text with line breaks have no time stamp.
            if line.split(' ').next().is_some_and(|t| t.len() >= 10 && t.bytes().all(|b| b.is_ascii_digit())) {
                line.to_string()
            } else {
                changed = true;
                String::new()
            }
        })
        .filter(|l| !l.is_empty())
        .collect();
    changed.then(|| lines.into_iter().map(|l| l + "\n").collect())
}

/// Rewrites an old `voice.log` once, at the start of the voice host.
pub fn scrub_old_log() {
    let Some(path) = log_path() else { return };
    let Ok(bytes) = std::fs::read(&path) else { return };
    if let Some(clean) = scrub_log(&String::from_utf8_lossy(&bytes)) {
        let _ = std::fs::write(&path, clean);
    }
}

/// The error text that means "no microphone gave any sound" (the pipeline turns it into its own message).
pub const NO_SOUND: &str = "Microphone gives no sound — choose it in More → Voice";

/// Names of devices that only pass sound from other software: they are silent when nothing plays into them.
pub fn is_virtual(name: &str) -> bool {
    let n = name.to_lowercase();
    ["virtual", "steam streaming", "stereo mix", "cable", "audiorelay"].iter().any(|v| n.contains(v))
}

/// Which devices to try when `first` (the device in use) gives only silence: the others in list order, virtual ones skipped.
/// The first item is `first` itself.
pub fn attempt_order(names: &[String], first: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = first.map(|f| f.to_string()).into_iter().collect();
    out.extend(names.iter().filter(|n| !is_virtual(n) && Some(n.as_str()) != first).cloned());
    out
}

/// Counts chunks of digital silence in a row.
#[derive(Debug, Default)]
pub struct DeadStream {
    zeros: u32,
}

impl DeadStream {
    /// `true` once a second of chunks (all of them zeros) has gone by; and for every further silent chunk.
    pub fn push(&mut self, rms: f32) -> bool {
        if rms < DIGITAL_SILENCE {
            self.zeros += 1;
        } else {
            self.zeros = 0;
        }
        self.zeros >= DEAD_CHUNKS
    }
}
/// A chunk this quiet is digital silence (all zeros), not a quiet room.
pub const DIGITAL_SILENCE: f32 = 0.000001;
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

    use super::{attempt_order, choose_device, DeadStream, DIGITAL_SILENCE, f32_to_i16, i32_to_i16, u16_to_i16, unique_names, Capture, Choice, Fed};

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

    /// What a stream tells the thread that owns it.
    #[derive(Clone, Default)]
    struct Flags {
        /// A second of digital silence in a row.
        dead: Arc<AtomicBool>,
        /// A chunk with real sound came in.
        alive: Arc<AtomicBool>,
    }

    /// The name of the device that is used when nothing is chosen.
    #[allow(dead_code)]
    fn default_name(host: &cpal::Host) -> Option<String> {
        host.default_input_device().and_then(|d| d.description().ok().map(|x| x.name().to_string()))
    }

    /// Opens the stream of `wanted` (a name, else the default device); returns it with the name of the device.
    fn build(wanted: &Option<String>, log: bool, sink: &Sink, on_error: &Arc<dyn Fn(String) + Send + Sync>, flags: &Flags) -> Result<(cpal::Stream, Option<String>), String> {
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
        let device_name = device.description().ok().map(|x| x.name().to_string());
        let supported = device.default_input_config().map_err(|e| format!("Microphone is not available: {e}"))?;
        let (rate, channels, format) = (supported.sample_rate(), supported.channels(), supported.sample_format());
        // The name of the microphone is not private data of the user's speech: the log says which device gave the silence.
        super::log_line(&format!("device name={} rate={rate} channels={channels} format={format:?}", device_name.as_deref().unwrap_or("?")));
        let _ = log;
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
        let (sink, flags) = (sink.clone(), flags.clone());
        let mut dead = DeadStream::default();
        let mut emit = move |frames: Vec<i16>| {
            for fed in capture.feed(&frames) {
                if dead.push(fed.decision.rms) {
                    flags.dead.store(true, Ordering::SeqCst);
                }
                if fed.decision.rms >= DIGITAL_SILENCE {
                    flags.alive.store(true, Ordering::SeqCst);
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
        Ok((stream, device_name))
    }

    /// Opens the microphone (`wanted` by name, else the default) and calls `on_chunk` for every 100 ms. The stream lives on a
    /// thread of its own (it is not `Send` on Windows); an error while recording goes to `on_error`. A stream that gives only
    /// digital silence for a second is reopened once; if it is still silent, the other real microphones are tried in turn
    /// (virtual ones are skipped) and the one that works goes to `on_device`. None works: `on_error(NO_SOUND)`.
    pub fn start(
        wanted: Option<String>,
        log: bool,
        on_chunk: impl FnMut(Fed) + Send + 'static,
        on_error: impl Fn(String) + Send + Sync + 'static,
        on_device: impl Fn(String) + Send + 'static,
    ) -> Result<Mic, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let sink: Sink = Arc::new(Mutex::new(Box::new(on_chunk)));
        let on_error: Arc<dyn Fn(String) + Send + Sync> = Arc::new(on_error);
        let join = std::thread::Builder::new()
            .name("decka-mic".into())
            .spawn(move || {
                let flags = Flags::default();
                match build(&wanted, log, &sink, &on_error, &flags) {
                    Ok((mut stream, first)) => {
                        let _ = ready_tx.send(Ok(()));
                        let mut current = first.clone();
                        let mut reopened = false;
                        let mut queue: Option<std::collections::VecDeque<String>> = None;
                        let mut switched = false;
                        loop {
                            if !matches!(stop_rx.recv_timeout(Duration::from_millis(100)), Err(mpsc::RecvTimeoutError::Timeout)) {
                                break; // stop, or the handle was dropped
                            }
                            if switched && flags.alive.load(Ordering::SeqCst) {
                                switched = false;
                                if let Some(name) = &current {
                                    super::log_line(&format!("working microphone found name={name}"));
                                    on_device(name.clone());
                                }
                            }
                            if !flags.dead.swap(false, Ordering::SeqCst) {
                                continue;
                            }
                            flags.alive.store(false, Ordering::SeqCst);
                            drop(stream);
                            if !reopened {
                                reopened = true;
                                super::log_line("digital silence: reopening the stream");
                                match build(&Some(current.clone().unwrap_or_default()).filter(|n| !n.is_empty()), log, &sink, &on_error, &flags) {
                                    Ok((s, _)) => {
                                        super::log_line("stream reopened");
                                        stream = s;
                                        continue;
                                    }
                                    Err(e) => super::log_line(&format!("reopen failed: {e}")),
                                }
                            }
                            // Still silent: the other microphones, one by one.
                            let q = queue.get_or_insert_with(|| {
                                let names = input_names(&cpal::default_host());
                                attempt_order(&names, current.as_deref()).into_iter().skip(1).collect()
                            });
                            let mut found = None;
                            while let Some(next) = q.pop_front() {
                                super::log_line(&format!("digital silence: trying microphone name={next}"));
                                match build(&Some(next.clone()), log, &sink, &on_error, &flags) {
                                    Ok((s, n)) => {
                                        found = Some((s, n.unwrap_or(next)));
                                        break;
                                    }
                                    Err(e) => super::log_line(&format!("microphone failed: {e}")),
                                }
                            }
                            match found {
                                Some((s, n)) => {
                                    stream = s;
                                    current = Some(n);
                                    switched = true;
                                }
                                None => {
                                    // Not on this thread: the error stops the capture, which waits for this thread.
                                    let on_error = on_error.clone();
                                    std::thread::spawn(move || on_error(super::NO_SOUND.to_string()));
                                    return;
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
    fn dead_stream_needs_a_whole_second_of_zeros() {
        let mut d = DeadStream::default();
        assert!((0..9).all(|_| !d.push(0.0)));
        assert!(d.push(0.0));
        assert!(!{ d.push(0.01); d.push(0.0) }, "a real chunk starts the count again");
    }

    #[test]
    fn virtual_microphones_are_skipped_and_the_one_in_use_comes_first() {
        let names: Vec<String> = ["Mic (MCHOSE V9 PRO)", "Микрофон (MCHOSE V9 PRO HEADSET)", "Virtual Mic (AudioRelay)", "Steam Streaming Microphone", "CABLE Output", "Stereo Mix"].map(String::from).to_vec();
        assert!(is_virtual("Virtual Mic (AudioRelay)") && is_virtual("steam streaming microphone") && !is_virtual("Mic (MCHOSE V9 PRO)"));
        // The default is a virtual one: it is tried first (by the reopen), then the real ones.
        assert_eq!(attempt_order(&names, Some("Virtual Mic (AudioRelay)")), ["Virtual Mic (AudioRelay)", "Mic (MCHOSE V9 PRO)", "Микрофон (MCHOSE V9 PRO HEADSET)"]);
        assert_eq!(attempt_order(&names, Some("Mic (MCHOSE V9 PRO)")), ["Mic (MCHOSE V9 PRO)", "Микрофон (MCHOSE V9 PRO HEADSET)"]);
        assert_eq!(attempt_order(&names, None).len(), 2);
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
    fn an_old_log_loses_the_texts_of_results_and_keeps_the_rest() {
        let old = "1790000000 event Executed t=1ms executing -> done\n1790000001 executed: ok Translation: my secret\nsecond line of the secret\n1790000002 table hit open_app but refused: no program called secret (apps known: 3)\n1790000003 executed: error no list called secret\n";
        assert_eq!(
            scrub_log(old).unwrap(),
            "1790000000 event Executed t=1ms executing -> done\n1790000001 executed: ok\n1790000002 table hit open_app but refused\n1790000003 executed: error\n"
        );
        assert_eq!(scrub_log("1790000000 executed: ok\n1790000001   chunk rms=0.1\n"), None);
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
