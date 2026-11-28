//! T32.1: measures the wake word on recordings, through the same gate and spotter as the worker.
//!
//! `cargo run --release -p decka-core --example wake_eval -- <sherpa-onnx-c-api.dll> <model folder> <low|medium|high> <positives> <negatives>`
//!
//! `positives`: one wake word per WAV file (16-bit PCM) - a file without a detection is a miss. `negatives`: any speech and
//! music without the word - every detection is a false alarm. `stop words` (T34.2): one "stop" or "enough" per file. The model folder is the one the agent downloads
//! (`%LOCALAPPDATA%\Decka\ai\wake\model-...`).

#[cfg(windows)]
fn main() -> Result<(), String> {
    use std::path::Path;

    use decka_core::voice::audio::{calculate_rms, resample_to_16k};
    use decka_core::voice::wake::{keywords, Cooldown, Gate, Gated, Keyword, Sensitivity, GATE_MIN};
    use decka_core::voice::wake_engine::Spotter;

    /// Mono 16 kHz samples of a 16-bit PCM WAV file.
    fn read(path: &Path) -> Result<Vec<i16>, String> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let bad = || format!("{}: not a 16-bit PCM WAV file", path.display());
        if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(bad());
        }
        let (mut at, mut format, mut samples) = (12, None, None);
        while at + 8 <= data.len() {
            let size = u32::from_le_bytes(data[at + 4..at + 8].try_into().map_err(|_| bad())?) as usize;
            let body = &data[at + 8..(at + 8 + size).min(data.len())];
            if &data[at..at + 4] == b"fmt " && body.len() >= 16 {
                format = Some((u16::from_le_bytes([body[0], body[1]]), u16::from_le_bytes([body[2], body[3]]), u32::from_le_bytes([body[4], body[5], body[6], body[7]]), u16::from_le_bytes([body[14], body[15]])));
            } else if &data[at..at + 4] == b"data" {
                samples = Some(body.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect::<Vec<_>>());
            }
            at += 8 + size + (size & 1);
        }
        let ((kind, channels, rate, bits), samples) = (format.ok_or_else(bad)?, samples.ok_or_else(bad)?);
        if !(kind == 1 || kind == 0xfffe) || bits != 16 || channels == 0 {
            return Err(bad());
        }
        let mono: Vec<i16> = samples.chunks_exact(channels as usize).map(|f| (f.iter().map(|s| i32::from(*s)).sum::<i32>() / i32::from(channels)) as i16).collect();
        Ok(resample_to_16k(&mono, rate))
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let [library, model, level, positives, negatives, rest @ ..] = args.as_slice() else {
        return Err("usage: wake_eval <dll> <model folder> <low|medium|high> <positives> <negatives> [stop words]".into());
    };
    let sensitivity = Sensitivity::parse(level).ok_or("sensitivity: low, medium or high")?;
    // `DECKA_WAKE_KEYWORDS`: a file with other keyword lines, to compare spellings and thresholds.
    let words = match std::env::var("DECKA_WAKE_KEYWORDS") {
        Ok(file) => std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?,
        Err(_) => keywords(sensitivity),
    };
    let mut spotter = Spotter::open(&std::path::absolute(library).map_err(|e| e.to_string())?, Path::new(model), &words, sensitivity.active_paths())?;
    let mut sets = vec![(positives, Some(Keyword::Decka)), (negatives, None)];
    sets.extend(rest.first().map(|folder| (folder, Some(Keyword::Stop))));
    for (folder, expected) in sets {
        let mut files: Vec<_> = std::fs::read_dir(folder).map_err(|e| format!("{folder}: {e}"))?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav"))).collect();
        files.sort();
        let (mut hits, mut stops, mut missed, mut seconds, mut busy) = (0u32, 0u32, 0u32, 0f64, std::time::Duration::ZERO);
        // From the last loud chunk of a file to the chunk that gave the expected word, in milliseconds.
        let mut late: Vec<i64> = Vec::new();
        for file in &files {
            let mut pcm = read(file)?;
            seconds += pcm.len() as f64 / 16_000.0;
            // Silence around a recording, as between two sentences.
            let mut padded = vec![0i16; 16_000];
            padded.append(&mut pcm);
            padded.resize(padded.len() + 32_000, 0);
            let (mut gate, mut cooldown, mut here, mut stopped) = (Gate::default(), Cooldown::default(), 0u32, 0u32);
            // The end of the sound: the last chunk above the quietest threshold of the gate.
            let end = padded.chunks(1600).rposition(|c| calculate_rms(c) / 32768.0 >= GATE_MIN).map_or(0, |i| i as i64);
            let mut first = None;
            let start = std::time::Instant::now();
            for (i, chunk) in padded.chunks(1600).enumerate() {
                let rms = calculate_rms(chunk) / 32768.0;
                match gate.feed(chunk.to_vec(), rms) {
                    Gated::Skip => {}
                    Gated::Close => spotter.reset(),
                    Gated::Feed(chunks) => {
                        let found: Vec<Keyword> = chunks.iter().filter_map(|c| spotter.feed(c)).collect();
                        if expected.is_some_and(|k| found.contains(&k)) {
                            first.get_or_insert(i as i64);
                        }
                        if found.contains(&Keyword::Stop) {
                            stopped += 1;
                        }
                        if found.contains(&Keyword::Decka) && cooldown.accept(start + std::time::Duration::from_millis(i as u64 * 100)) {
                            here += 1;
                        }
                    }
                }
            }
            busy += start.elapsed();
            spotter.reset();
            hits += here;
            stops += stopped;
            late.extend(first.map(|i| (i - end) * 100));
            if expected.is_some() && first.is_none() {
                missed += 1;
                println!("missed: {}", file.display());
            } else if expected.is_none() && here + stopped > 0 {
                println!("false alarm decka x{here} stop x{stopped}: {}", file.display());
            }
        }
        let load = busy.as_secs_f64() / seconds.max(0.001) * 100.0;
        late.sort_unstable();
        let at = |q: usize| late.get(late.len().saturating_sub(1) * q / 100).copied().unwrap_or(0);
        match expected {
            Some(word) => println!("{word:?}: {} files, missed {missed} ({:.1} %), heard after the end of the sound: median {} ms, 90 % {} ms; processor {load:.1} % of one core", files.len(), f64::from(missed) * 100.0 / files.len().max(1) as f64, at(50), at(90)),
            None => println!("negatives: {:.1} min, false alarms: decka {hits} ({:.2} per hour), stop {stops} ({:.2} per hour); processor {load:.1} % of one core", seconds / 60.0, f64::from(hits) * 3600.0 / seconds.max(0.001), f64::from(stops) * 3600.0 / seconds.max(0.001)),
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("the wake word runs on Windows");
}
