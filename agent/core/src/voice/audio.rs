//! Voice audio utilities: WAV header creation, resampling to 16 kHz, and silence detection.
//! (TASKS.md T21.2 / M21, DATA_MODEL.md §12.3).

/// Standard 44-byte RIFF/WAVE header for 16-bit PCM audio.
pub fn make_wav_header(pcm_bytes_len: u32, sample_rate: u32, channels: u16, bits_per_sample: u16) -> [u8; 44] {
    let mut header = [0u8; 44];
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;
    let riff_chunk_size = 36u32.saturating_add(pcm_bytes_len);

    // RIFF chunk descriptor
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&riff_chunk_size.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");

    // "fmt " sub-chunk
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes()); // Subchunk1Size for PCM
    header[20..22].copy_from_slice(&1u16.to_le_bytes());  // AudioFormat 1 = PCM
    header[22..24].copy_from_slice(&channels.to_le_bytes());
    header[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&bits_per_sample.to_le_bytes());

    // "data" sub-chunk
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&pcm_bytes_len.to_le_bytes());

    header
}

/// Wraps mono 16-bit PCM samples with a 44-byte WAV header at the given sample rate.
pub fn encode_wav_pcm16(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let pcm_bytes_len = (samples.len() * 2) as u32;
    let header = make_wav_header(pcm_bytes_len, sample_rate, 1, 16);
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(&header);
    for &sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Resamples mono 16-bit PCM audio from `from_rate` to `to_rate`.
///
/// When downsampling with integer ratios (e.g. 48000 -> 16000, ratio 3),
/// samples are averaged with a boxcar filter to prevent aliasing.
/// For other ratios, linear interpolation is applied.
pub fn resample_pcm16(input: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if input.is_empty() {
        return Vec::new();
    }
    if from_rate == to_rate {
        return input.to_vec();
    }

    // Fast path for exact integer downsampling (e.g. 48 kHz -> 16 kHz, ratio 3)
    if from_rate > to_rate && from_rate.is_multiple_of(to_rate) {
        let ratio = (from_rate / to_rate) as usize;
        let out_len = input.len() / ratio;
        let mut out = Vec::with_capacity(out_len);
        for i in 0..out_len {
            let start = i * ratio;
            let mut sum = 0i64;
            for j in 0..ratio {
                sum += input[start + j] as i64;
            }
            let avg = (sum / ratio as i64).clamp(i16::MIN as i64, i16::MAX as i64) as i16;
            out.push(avg);
        }
        return out;
    }

    // General linear interpolation
    let out_len = ((input.len() as u64 * to_rate as u64) / from_rate as u64) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = (i as f64 * from_rate as f64) / to_rate as f64;
        let idx = src_pos.floor() as usize;
        let frac = src_pos - idx as f64;
        let s1 = input[idx] as f64;
        let s2 = if idx + 1 < input.len() {
            input[idx + 1] as f64
        } else {
            s1
        };
        let sample = s1 + frac * (s2 - s1);
        out.push(sample.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    out
}

/// Resamples mono 16-bit PCM to 16 kHz.
pub fn resample_to_16k(input: &[i16], from_rate: u32) -> Vec<i16> {
    resample_pcm16(input, from_rate, 16000)
}

/// Computes root-mean-square (RMS) of audio samples.
pub fn calculate_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sum_sq = 0.0f64;
    for &s in samples {
        let val = s as f64;
        sum_sq += val * val;
    }
    ((sum_sq / samples.len() as f64).sqrt()) as f32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilenceDecision {
    /// Speech continues or silence has not exceeded threshold.
    Continue,
    /// Trailing silence threshold reached after speech was detected.
    EndOfSpeech,
    /// Maximum recording duration reached (e.g. 30 seconds).
    MaxDurationExceeded,
}

/// Voice Activity & Silence Detector for audio streams.
///
/// Stops on trailing silence (default 800 ms) after speech has started,
/// or after reaching `max_duration_sec` (default 30 s, DATA_MODEL.md §12.6).
#[derive(Debug, Clone)]
pub struct SilenceDetector {
    sample_rate: u32,
    threshold_rms: f32,
    silence_samples_limit: usize,
    min_voice_samples: usize,
    max_total_samples: usize,

    total_samples: usize,
    voice_samples: usize,
    trailing_silence_samples: usize,
    voice_active: bool,
}

impl SilenceDetector {
    /// Creates a new silence detector.
    ///
    /// - `sample_rate`: e.g. 16000
    /// - `threshold_rms`: RMS threshold for speech (e.g. 350.0)
    /// - `trailing_silence_ms`: silence duration in ms to end speech (e.g. 800)
    /// - `max_duration_sec`: maximum recording length in seconds (default 30)
    pub fn new(sample_rate: u32, threshold_rms: f32, trailing_silence_ms: u32, max_duration_sec: u32) -> Self {
        let silence_samples_limit = ((sample_rate as u64 * trailing_silence_ms as u64) / 1000) as usize;
        let min_voice_samples = ((sample_rate as u64 * 200) / 1000) as usize; // at least 200ms voice
        let max_total_samples = (sample_rate as usize) * (max_duration_sec as usize);

        Self {
            sample_rate,
            threshold_rms,
            silence_samples_limit,
            min_voice_samples,
            max_total_samples,
            total_samples: 0,
            voice_samples: 0,
            trailing_silence_samples: 0,
            voice_active: false,
        }
    }

    /// Creates a detector with standard defaults for 16 kHz audio:
    /// RMS threshold 350, 800 ms trailing silence, 30 s max recording.
    pub fn default_16k() -> Self {
        Self::new(16000, 350.0, 800, 30)
    }

    /// Ingests a chunk of samples and returns the decision.
    pub fn process_chunk(&mut self, chunk: &[i16]) -> SilenceDecision {
        self.total_samples += chunk.len();
        if self.total_samples >= self.max_total_samples {
            return SilenceDecision::MaxDurationExceeded;
        }

        let rms = calculate_rms(chunk);
        if rms >= self.threshold_rms {
            self.voice_samples += chunk.len();
            if self.voice_samples >= self.min_voice_samples {
                self.voice_active = true;
            }
            self.trailing_silence_samples = 0;
            SilenceDecision::Continue
        } else {
            if self.voice_active {
                self.trailing_silence_samples += chunk.len();
                if self.trailing_silence_samples >= self.silence_samples_limit {
                    return SilenceDecision::EndOfSpeech;
                }
            }
            SilenceDecision::Continue
        }
    }

    /// Sample rate configured for this detector.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Whether voice activity has been triggered.
    pub fn is_voice_active(&self) -> bool {
        self.voice_active
    }

    /// Total samples processed so far.
    pub fn total_samples(&self) -> usize {
        self.total_samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header() {
        let sample_rate = 16000u32;
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let pcm_bytes_len = 3200u32; // 100 ms of mono 16-bit 16kHz audio

        let header = make_wav_header(pcm_bytes_len, sample_rate, channels, bits_per_sample);
        assert_eq!(&header[0..4], b"RIFF");
        let riff_size = u32::from_le_bytes(header[4..8].try_into().unwrap());
        assert_eq!(riff_size, 36 + pcm_bytes_len);
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(header[16..20].try_into().unwrap()), 16); // PCM chunk size
        assert_eq!(u16::from_le_bytes(header[20..22].try_into().unwrap()), 1);  // PCM format
        assert_eq!(u16::from_le_bytes(header[22..24].try_into().unwrap()), channels);
        assert_eq!(u32::from_le_bytes(header[24..28].try_into().unwrap()), sample_rate);
        assert_eq!(u32::from_le_bytes(header[28..32].try_into().unwrap()), 32000); // 16000 * 2
        assert_eq!(u16::from_le_bytes(header[32..34].try_into().unwrap()), 2);     // block align
        assert_eq!(u16::from_le_bytes(header[34..36].try_into().unwrap()), 16);    // bits
        assert_eq!(&header[36..40], b"data");
        assert_eq!(u32::from_le_bytes(header[40..44].try_into().unwrap()), pcm_bytes_len);

        // encode_wav_pcm16 wrapper
        let dummy_samples = vec![0i16; 1600];
        let wav_data = encode_wav_pcm16(&dummy_samples, 16000);
        assert_eq!(wav_data.len(), 44 + 3200);
        assert_eq!(&wav_data[0..4], b"RIFF");
        assert_eq!(&wav_data[36..40], b"data");
    }

    #[test]
    fn test_resample_48k_to_16k_length_and_zeros() {
        // 1 second of 48 kHz silence (48,000 samples of zero)
        let zeros_48k = vec![0i16; 48000];
        let out_16k = resample_to_16k(&zeros_48k, 48000);

        // Length must be exactly 16,000 (1/3 of 48,000)
        assert_eq!(out_16k.len(), 16000);
        // All samples must be zero
        assert!(out_16k.iter().all(|&s| s == 0), "Resampled silence must contain all zeros");

        // Non-zero signal: 4800 samples of 48 kHz sine wave -> 1600 samples
        let non_zeros: Vec<i16> = (0..4800)
            .map(|i| ((i as f64 * 0.1).sin() * 10000.0) as i16)
            .collect();
        let resampled = resample_to_16k(&non_zeros, 48000);
        assert_eq!(resampled.len(), 1600);
        assert!(resampled.iter().any(|&s| s != 0), "Resampled sine wave must contain non-zero samples");
    }

    #[test]
    fn test_silence_detector_synthetic_signal() {
        // Detector with 500 ms trailing silence, 350 RMS threshold, 10s max duration
        let sample_rate = 16000u32;
        let mut detector = SilenceDetector::new(sample_rate, 350.0, 500, 10);
        let chunk_size = 1600; // 100 ms chunks

        // 1. Initial silence: 3 chunks (300 ms) of amplitude 0 -> Continue
        let silence_chunk = vec![0i16; chunk_size];
        for _ in 0..3 {
            let res = detector.process_chunk(&silence_chunk);
            assert_eq!(res, SilenceDecision::Continue);
            assert!(!detector.is_voice_active());
        }

        // 2. Synthetic speech: 5 chunks (500 ms) of 1000 Hz tone, amplitude 4000 (RMS ~ 2828)
        let speech_chunk: Vec<i16> = (0..chunk_size)
            .map(|i| ((2.0 * std::f64::consts::PI * 1000.0 * (i as f64 / sample_rate as f64)).sin() * 4000.0) as i16)
            .collect();

        for _ in 0..5 {
            let res = detector.process_chunk(&speech_chunk);
            assert_eq!(res, SilenceDecision::Continue);
        }
        assert!(detector.is_voice_active(), "Detector should detect voice activity");

        // 3. Trailing silence:
        // Chunks 1 to 4 (400 ms) -> Continue
        for _ in 0..4 {
            let res = detector.process_chunk(&silence_chunk);
            assert_eq!(res, SilenceDecision::Continue);
        }
        // Chunk 5 (at 500 ms) -> EndOfSpeech
        let res = detector.process_chunk(&silence_chunk);
        assert_eq!(res, SilenceDecision::EndOfSpeech, "Should detect end of speech after 500ms trailing silence");

        // 4. Test max duration cutoff
        let mut max_detector = SilenceDetector::new(16000, 350.0, 500, 1); // 1 sec max
        let mut hit_max = false;
        for _ in 0..15 {
            let res = max_detector.process_chunk(&speech_chunk);
            if res == SilenceDecision::MaxDurationExceeded {
                hit_max = true;
                break;
            }
        }
        assert!(hit_max, "Should stop when max duration exceeded");
    }
}
