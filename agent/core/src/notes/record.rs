//! The sound of a meeting: the microphone and what the PC plays (WASAPI loopback through `cpal`), both as 16 kHz mono,
//! added together. The two devices have their own clocks and the PC one says nothing while nothing plays, so the mix is
//! paced by the wall clock and a source that is behind counts as silence.

use std::collections::VecDeque;

use super::text::RATE;
use super::{Sink, Source};

/// The mix is given out this far behind the clock, so that the blocks of the devices have arrived.
const DELAY_MS: u64 = 300;
/// A source that runs ahead keeps at most this much.
const MAX_QUEUE: usize = 2 * RATE;

#[derive(Debug, Default)]
pub struct Mixer {
    mic: VecDeque<i16>,
    pc: VecDeque<i16>,
    given: u64,
}

fn push(queue: &mut VecDeque<i16>, samples: &[i16]) {
    queue.extend(samples);
    let extra = queue.len().saturating_sub(MAX_QUEUE);
    queue.drain(..extra);
}

impl Mixer {
    pub fn mic(&mut self, samples: &[i16]) {
        push(&mut self.mic, samples);
    }

    pub fn pc(&mut self, samples: &[i16]) {
        push(&mut self.pc, samples);
    }

    /// The mix up to sample `due` of the recording.
    pub fn take(&mut self, due: u64) -> Vec<i16> {
        let n = due.saturating_sub(self.given) as usize;
        self.given = self.given.max(due);
        (0..n).map(|_| self.mic.pop_front().unwrap_or(0).saturating_add(self.pc.pop_front().unwrap_or(0))).collect()
    }
}

/// The samples that are due `ms` after the start.
pub fn due_after(ms: u64) -> u64 {
    ms.saturating_sub(DELAY_MS) * RATE as u64 / 1000
}

/// The real devices.
pub struct DeviceSource;

#[cfg(windows)]
mod device {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleFormat;

    use super::{due_after, Mixer, Sink};
    use crate::voice::mic;

    /// A running recording of both devices.
    pub struct Recording {
        stop: Arc<AtomicBool>,
        threads: Vec<JoinHandle<()>>,
        mic: Option<mic::Mic>,
    }

    impl Drop for Recording {
        fn drop(&mut self) {
            self.mic.take();
            self.stop.store(true, Ordering::SeqCst);
            for t in self.threads.drain(..) {
                let _ = t.join();
            }
        }
    }

    /// The loopback stream of the default output on a thread of its own (a stream is not `Send` on Windows).
    fn loopback(mixer: Arc<Mutex<Mixer>>, stop: Arc<AtomicBool>) -> Result<JoinHandle<()>, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("decka-loopback".into())
            .spawn(move || {
                let build = || -> Result<cpal::Stream, String> {
                    let device = cpal::default_host().default_output_device().ok_or("no output device")?;
                    let supported = device.default_output_config().map_err(|e| e.to_string())?;
                    let (rate, channels, format) = (supported.sample_rate(), supported.channels(), supported.sample_format());
                    let config = supported.config();
                    let mut capture = mic::Capture::new(rate, channels);
                    let mixer = mixer.clone();
                    let mut feed = move |frames: Vec<i16>| {
                        for fed in capture.feed(&frames) {
                            let samples: Vec<i16> = fed.pcm.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
                            mixer.lock().unwrap().pc(&samples);
                        }
                    };
                    let on_error = |e: cpal::Error| mic::log_line(&format!("loopback: {e}"));
                    // An input stream on an output device records what it plays.
                    let stream = match format {
                        SampleFormat::F32 => device.build_input_stream(config, move |d: &[f32], _: &_| feed(d.iter().map(|s| mic::f32_to_i16(*s)).collect()), on_error, None),
                        SampleFormat::I16 => device.build_input_stream(config, move |d: &[i16], _: &_| feed(d.to_vec()), on_error, None),
                        SampleFormat::U16 => device.build_input_stream(config, move |d: &[u16], _: &_| feed(d.iter().map(|s| mic::u16_to_i16(*s)).collect()), on_error, None),
                        SampleFormat::I32 => device.build_input_stream(config, move |d: &[i32], _: &_| feed(d.iter().map(|s| mic::i32_to_i16(*s)).collect()), on_error, None),
                        other => return Err(format!("output format {other:?} is not supported")),
                    }
                    .map_err(|e| e.to_string())?;
                    stream.play().map_err(|e| e.to_string())?;
                    Ok(stream)
                };
                match build() {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        while !stop.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        ready_rx.recv().map_err(|_| "the sound thread stopped".to_string())??;
        Ok(thread)
    }

    pub fn start(wanted: Option<String>, mut sink: Sink) -> Result<Recording, String> {
        let mixer = Arc::new(Mutex::new(Mixer::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let to = mixer.clone();
        // A microphone that falls silent later (muted, unplugged) does not end the note: the PC sound goes on.
        let mic = mic::start(
            wanted,
            false,
            move |fed| {
                let samples: Vec<i16> = fed.pcm.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
                to.lock().unwrap().mic(&samples);
            },
            |e| mic::log_line(&format!("note microphone: {e}")),
            |_| {},
        );
        let mut threads = Vec::new();
        match loopback(mixer.clone(), stop.clone()) {
            Ok(t) => threads.push(t),
            Err(e) => {
                mic::log_line(&format!("note: the PC sound is not recorded: {e}"));
                // Nothing at all to record.
                if let Err(why) = &mic {
                    return Err(why.clone());
                }
            }
        }
        let halt = stop.clone();
        let clock = std::thread::Builder::new()
            .name("decka-note-mix".into())
            .spawn(move || {
                let started = Instant::now();
                loop {
                    let ending = halt.load(Ordering::SeqCst);
                    // At the end the delayed rest is given out too.
                    let ms = started.elapsed().as_millis() as u64 + if ending { super::DELAY_MS } else { 0 };
                    let mix = mixer.lock().unwrap().take(due_after(ms));
                    if !mix.is_empty() {
                        sink(mix);
                    }
                    if ending {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            })
            .map_err(|e| e.to_string())?;
        threads.push(clock);
        Ok(Recording { stop, threads, mic: mic.ok() })
    }
}

#[cfg(windows)]
impl Source for DeviceSource {
    fn start(&self, mic: Option<String>, sink: Sink) -> Result<Box<dyn Send>, String> {
        Ok(Box::new(device::start(mic, sink)?))
    }
}

#[cfg(not(windows))]
impl Source for DeviceSource {
    fn start(&self, _: Option<String>, _: Sink) -> Result<Box<dyn Send>, String> {
        Err("Notes are recorded on Windows only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_sources_are_added_and_a_late_one_is_silence() {
        let mut m = Mixer::default();
        m.mic(&[100, 200, 30000]);
        m.pc(&[1, 2, 30000, 4]);
        assert_eq!(m.take(2), [101, 202]);
        assert_eq!(m.take(5), [i16::MAX, 4, 0]);
        assert_eq!(m.take(3), Vec::<i16>::new(), "nothing is given twice");
        assert_eq!((due_after(100), due_after(1300)), (0, RATE as u64));
        m.pc(&vec![7; 3 * RATE]);
        assert_eq!(m.pc.len(), MAX_QUEUE, "a source that runs ahead is cut");
    }
}
