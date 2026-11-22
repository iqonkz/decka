//! End of the phrase by silence (DATA_MODEL.md 12.6a). A pure function over the RMS of 100 ms chunks (PCM normalised to 1.0):
//! `continue | end | no_speech`. The phone has the same logic with the same constants in `client/src/voice/endpoint.ts`.

/// One chunk is 100 ms.
pub const CHUNK_MS: u32 = 100;
/// The first 300 ms measure the noise floor.
pub const CALIBRATION_MS: u32 = 300;
/// Speech is a chunk louder than `max(floor * NOISE_FACTOR, MIN_THRESHOLD)`.
pub const NOISE_FACTOR: f32 = 3.0;
pub const MIN_THRESHOLD: f32 = 0.015;
/// Speech counts after this much of it.
pub const MIN_SPEECH_MS: u32 = 300;
/// This much silence in a row after speech ends the phrase.
pub const END_SILENCE_MS: u32 = 2000;
/// No speech by then: nothing was said.
pub const NO_SPEECH_MS: u32 = 5000;
/// A recording is never longer.
pub const MAX_MS: u32 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Continue,
    End,
    NoSpeech,
}

#[derive(Debug, Clone, Default)]
pub struct Endpointer {
    elapsed_ms: u32,
    floor_sum: f32,
    floor_n: u32,
    threshold: f32,
    speech_ms: u32,
    silence_ms: u32,
}

impl Endpointer {
    pub fn new() -> Endpointer {
        Endpointer::default()
    }

    /// The RMS of the next chunk of 100 ms.
    pub fn feed(&mut self, rms: f32) -> Verdict {
        self.elapsed_ms += CHUNK_MS;
        if self.floor_n < CALIBRATION_MS / CHUNK_MS {
            self.floor_sum += rms;
            self.floor_n += 1;
            if self.floor_n == CALIBRATION_MS / CHUNK_MS {
                self.threshold = (self.floor_sum / self.floor_n as f32 * NOISE_FACTOR).max(MIN_THRESHOLD);
            }
        } else if rms > self.threshold {
            self.speech_ms += CHUNK_MS;
            self.silence_ms = 0;
        } else if self.speech_ms >= MIN_SPEECH_MS {
            self.silence_ms += CHUNK_MS;
        }
        if self.elapsed_ms >= MAX_MS || self.silence_ms >= END_SILENCE_MS {
            Verdict::End
        } else if self.speech_ms < MIN_SPEECH_MS && self.elapsed_ms >= NO_SPEECH_MS {
            Verdict::NoSpeech
        } else {
            Verdict::Continue
        }
    }
}

/// The wave of the pill: rises at once, falls with a time constant of 150 ms.
#[derive(Debug, Clone, Default)]
pub struct LevelSmoother {
    level: f32,
}

impl LevelSmoother {
    /// RMS of voice at a desk microphone is 0.03..0.2; the gain brings that to most of the height.
    pub const GAIN: f32 = 6.0;

    pub fn feed(&mut self, rms: f32) -> f32 {
        let raw = (rms * Self::GAIN).clamp(0.0, 1.0);
        let decay = (-(CHUNK_MS as f32) / 150.0).exp();
        self.level = raw.max(self.level * decay);
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET: f32 = 0.002;
    const SPEECH: f32 = 0.1;

    /// Feeds chunks; returns the index (1-based) of the first verdict that is not `Continue`.
    fn run(chunks: &[f32]) -> Option<(usize, Verdict)> {
        let mut e = Endpointer::new();
        chunks.iter().enumerate().find_map(|(i, c)| match e.feed(*c) {
            Verdict::Continue => None,
            v => Some((i + 1, v)),
        })
    }

    fn rep(v: f32, n: usize) -> Vec<f32> {
        vec![v; n]
    }

    #[test]
    fn silence_after_speech_ends_after_two_seconds() {
        let mut c = rep(QUIET, 3);
        c.extend(rep(SPEECH, 10));
        c.extend(rep(QUIET, 30));
        assert_eq!(run(&c), Some((3 + 10 + 20, Verdict::End)));
    }

    #[test]
    fn noise_without_speech_is_no_speech_at_five_seconds() {
        // Steady noise: the floor follows it, so it is not speech.
        assert_eq!(run(&rep(0.02, 80)), Some((50, Verdict::NoSpeech)));
        assert_eq!(run(&rep(QUIET, 80)), Some((50, Verdict::NoSpeech)));
    }

    #[test]
    fn a_pause_of_one_second_does_not_cut_the_phrase() {
        let mut c = rep(QUIET, 3);
        c.extend(rep(SPEECH, 5));
        c.extend(rep(QUIET, 10));
        c.extend(rep(SPEECH, 5));
        c.extend(rep(QUIET, 19));
        assert_eq!(run(&c), None, "1 s of silence and then speech again goes on");
        c.push(QUIET);
        assert_eq!(run(&c), Some((c.len(), Verdict::End)));
    }

    #[test]
    fn a_click_is_not_speech() {
        let mut c = rep(QUIET, 3);
        c.extend(rep(SPEECH, 2)); // 200 ms
        c.extend(rep(QUIET, 60));
        assert_eq!(run(&c), Some((50, Verdict::NoSpeech)));
    }

    #[test]
    fn twenty_seconds_end_the_recording() {
        let mut c = rep(QUIET, 3);
        c.extend(rep(SPEECH, 300));
        assert_eq!(run(&c), Some((200, Verdict::End)));
    }

    #[test]
    fn the_wave_rises_at_once_and_falls_in_about_150_ms() {
        let mut s = LevelSmoother::default();
        assert!((s.feed(0.1) - 0.6).abs() < 1e-6);
        let after = s.feed(0.0);
        assert!(after > 0.25 && after < 0.35, "{after}");
        assert_eq!(s.feed(1.0), 1.0);
    }
}
