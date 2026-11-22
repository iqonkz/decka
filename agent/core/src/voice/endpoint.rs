//! End of the phrase by silence (DATA_MODEL.md 12.6a). A pure function over the RMS of 100 ms chunks (PCM normalised to 1.0):
//! `continue | end | no_speech`. The phone has the same logic with the same constants in `client/src/voice/endpoint.ts`.

/// One chunk is 100 ms.
pub const CHUNK_MS: u32 = 100;
/// The noise floor is a low percentile of the last `FLOOR_WINDOW` chunks, so speech from the first second cannot raise it.
pub const FLOOR_WINDOW: usize = 30;
pub const FLOOR_PERCENTILE: f32 = 0.2;
/// Speech is a chunk louder than `clamp(floor * NOISE_FACTOR, MIN_THRESHOLD, MAX_THRESHOLD)`. The cap keeps a loud first
/// second (or a very noisy room) from hiding the speech.
pub const NOISE_FACTOR: f32 = 3.0;
pub const MIN_THRESHOLD: f32 = 0.015;
pub const MAX_THRESHOLD: f32 = 0.03;
/// After speech, a chunk is silence below this share of the mean speech level (a microphone whose gain rises in a pause
/// would otherwise keep the phrase open for ever).
pub const END_RATIO: f32 = 0.25;
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

/// What the last chunk looked like, for `voice.log` (numbers only: no sound, no text).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Decision {
    pub rms: f32,
    pub floor: f32,
    pub threshold: f32,
    pub speech_ms: u32,
    pub silence_ms: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Endpointer {
    elapsed_ms: u32,
    window: std::collections::VecDeque<f32>,
    speech_ms: u32,
    silence_ms: u32,
    speech_sum: f32,
    speech_n: u32,
    last: Decision,
}

impl Endpointer {
    pub fn new() -> Endpointer {
        Endpointer::default()
    }

    /// The decision about the chunk fed last.
    pub fn last(&self) -> Decision {
        self.last
    }

    fn floor(&self) -> f32 {
        let mut v: Vec<f32> = self.window.iter().copied().collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v.get(((v.len().saturating_sub(1)) as f32 * FLOOR_PERCENTILE) as usize).copied().unwrap_or(0.0)
    }

    /// The RMS of the next chunk of 100 ms.
    pub fn feed(&mut self, rms: f32) -> Verdict {
        self.elapsed_ms += CHUNK_MS;
        self.window.push_back(rms);
        if self.window.len() > FLOOR_WINDOW {
            self.window.pop_front();
        }
        let floor = self.floor();
        let start = (floor * NOISE_FACTOR).clamp(MIN_THRESHOLD, MAX_THRESHOLD);
        let in_speech = self.speech_ms >= MIN_SPEECH_MS;
        let threshold = if in_speech { start.max(END_RATIO * self.speech_sum / self.speech_n.max(1) as f32) } else { start };
        if rms > threshold {
            self.speech_ms += CHUNK_MS;
            self.silence_ms = 0;
            self.speech_sum += rms;
            self.speech_n += 1;
        } else if in_speech {
            self.silence_ms += CHUNK_MS;
        }
        self.last = Decision { rms, floor, threshold, speech_ms: self.speech_ms, silence_ms: self.silence_ms };
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
