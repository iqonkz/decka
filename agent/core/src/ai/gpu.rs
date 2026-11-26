//! How many layers of the model go to the video card (TASKS.md T28.1). A card that also holds the speech recognition
//! cannot take a whole model: `llama-server -ngl 99` then fails to load. The share that fits is computed from the free
//! video memory, the size of the files and the number of layers read from the GGUF header.

use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

pub const ALL_LAYERS: u32 = 99;
/// Windows, the screen and the other programs.
pub const SYSTEM_RESERVE_GB: f64 = 1.0;
/// The speech recognition, when it runs on the card.
pub const WHISPER_RESERVE_GB: f64 = 1.0;
/// The context cache (`-c 4096`).
pub const CACHE_GB: f64 = 0.5;
/// Below this share of the model the card is not worth it: everything runs on the processor.
pub const MIN_SHARE: f64 = 0.25;
/// Used when the header cannot be read (the file is then broken and `llama-server` says so itself).
pub const DEFAULT_LAYER_COUNT: u32 = 32;

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// The video memory left for the model, in GiB. No dedicated card: nothing.
pub fn free_vram_gb(vram_gb: Option<f64>, whisper_gpu: bool) -> f64 {
    let whisper = if whisper_gpu { WHISPER_RESERVE_GB } else { 0.0 };
    (vram_gb.unwrap_or(0.0) - SYSTEM_RESERVE_GB - whisper).max(0.0)
}

/// `-ngl`: 99 when the model, the projector and the cache fit; else the share of the layers that fits beside the cache
/// (rounded down), or 0 when that is less than a quarter. Below 99 the projector stays on the processor.
pub fn gpu_layers(vram_gb: Option<f64>, whisper_gpu: bool, model_bytes: u64, mmproj_bytes: u64, layer_count: u32) -> u32 {
    let free = free_vram_gb(vram_gb, whisper_gpu);
    let model = model_bytes as f64 / GIB;
    if model + mmproj_bytes as f64 / GIB + CACHE_GB <= free {
        return ALL_LAYERS;
    }
    if model <= 0.0 {
        return 0;
    }
    let share = ((free - CACHE_GB) / model).clamp(0.0, 1.0);
    if share < MIN_SHARE {
        0
    } else {
        ((share * f64::from(layer_count)).floor() as u32).min(ALL_LAYERS - 1)
    }
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(r: &mut impl Read) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn skip(r: &mut (impl Read + Seek), bytes: u64) -> io::Result<()> {
    r.seek(SeekFrom::Current(i64::try_from(bytes).map_err(|_| io::ErrorKind::InvalidData)?)).map(|_| ())
}

/// Bytes of a value of a fixed size; `None` for strings and arrays.
fn fixed_size(kind: u32) -> Option<u64> {
    match kind {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

/// `<architecture>.block_count` of a GGUF file (the number of layers `-ngl` counts).
pub fn gguf_layer_count(path: &Path) -> Option<u32> {
    parse_layer_count(&mut BufReader::new(std::fs::File::open(path).ok()?)).ok().flatten()
}

fn parse_layer_count(r: &mut (impl Read + Seek)) -> io::Result<Option<u32>> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != b"GGUF" || read_u32(r)? < 2 {
        return Ok(None);
    }
    let _tensors = read_u64(r)?;
    let pairs = read_u64(r)?.min(10_000);
    for _ in 0..pairs {
        let len = read_u64(r)?;
        if len > 1_024 {
            return Ok(None);
        }
        let mut key = vec![0u8; len as usize];
        r.read_exact(&mut key)?;
        let kind = read_u32(r)?;
        if key.ends_with(b".block_count") {
            return Ok(match kind {
                4 | 5 => Some(read_u32(r)?),
                10 | 11 => u32::try_from(read_u64(r)?).ok(),
                _ => None,
            });
        }
        match kind {
            8 => {
                let n = read_u64(r)?;
                skip(r, n)?;
            }
            9 => {
                let inner = read_u32(r)?;
                let count = read_u64(r)?;
                match fixed_size(inner) {
                    Some(size) => skip(r, count.saturating_mul(size))?,
                    None if inner == 8 => {
                        for _ in 0..count {
                            let n = read_u64(r)?;
                            skip(r, n)?;
                        }
                    }
                    None => return Ok(None),
                }
            }
            k => match fixed_size(k) {
                Some(size) => skip(r, size)?,
                None => return Ok(None),
            },
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sizes of the files of the manifest.
    const MODEL_4B: u64 = 2_740_937_888;
    const MMPROJ: u64 = 700_000_000;

    #[test]
    fn two_gigabytes_with_recognition_on_the_card_leave_nothing() {
        assert_eq!(gpu_layers(Some(2.0), true, MODEL_4B, MMPROJ, 32), 0);
        assert_eq!(gpu_layers(Some(2.0), false, MODEL_4B, 0, 32), 0, "0.5 GB beside the cache is under a quarter");
        assert_eq!(gpu_layers(None, false, MODEL_4B, 0, 32), 0, "no dedicated card");
        assert_eq!(gpu_layers(Some(0.1), false, MODEL_4B, 0, 32), 0, "integrated graphics");
    }

    #[test]
    fn eight_gigabytes_take_the_whole_model() {
        assert_eq!(gpu_layers(Some(8.0), true, MODEL_4B, MMPROJ, 32), ALL_LAYERS);
        assert_eq!(gpu_layers(Some(7.8), true, MODEL_4B, MMPROJ, 32), ALL_LAYERS, "what DXGI says of an 8 GB card");
    }

    #[test]
    fn four_gigabytes_take_a_part() {
        // 4 - 1 - 1 = 2 GB free, 1.5 GB of it for the layers of a 2.55 GB model.
        assert_eq!(gpu_layers(Some(4.0), true, MODEL_4B, MMPROJ, 32), 18);
        // Without recognition on the card: 3 GB free, the model and the cache do not fit with the projector.
        assert_eq!(gpu_layers(Some(4.0), false, MODEL_4B, MMPROJ, 32), 31);
        // Everything but the projector fits: all layers, still below 99 (the projector goes to the processor).
        assert_eq!(gpu_layers(Some(4.4), false, MODEL_4B, MMPROJ, 32), 32);
    }

    /// `-ngl` of the three models of the manifest on common PCs (their sizes, and `block_count` from their headers:
    /// 24, 32, 32), without and with the Screen help projector.
    #[test]
    fn the_table_of_common_pcs() {
        let models = [(1_280_835_840u64, 668_227_264u64, 24u32), (MODEL_4B, 672_423_616, 32), (5_680_522_464, 918_166_080, 32)];
        let row = |vram: Option<f64>, whisper: bool, projector: bool| models.map(|(model, mmproj, layers)| gpu_layers(vram, whisper, model, if projector { mmproj } else { 0 }, layers));
        let table: [(Option<f64>, bool, [u32; 3], [u32; 3]); 9] = [
            (None, false, [0, 0, 0], [0, 0, 0]),
            (Some(2.0), true, [0, 0, 0], [0, 0, 0]),
            (Some(2.0), false, [10, 0, 0], [10, 0, 0]),
            (Some(4.0), true, [99, 18, 9], [24, 18, 9]),
            (Some(4.0), false, [99, 31, 15], [99, 31, 15]),
            (Some(6.0), true, [99, 99, 21], [99, 99, 21]),
            (Some(6.0), false, [99, 99, 27], [99, 99, 27]),
            (Some(8.0), true, [99, 99, 99], [99, 99, 32]),
            (Some(12.0), true, [99, 99, 99], [99, 99, 99]),
        ];
        for (vram, whisper, plain, with_projector) in table {
            assert_eq!(row(vram, whisper, false), plain, "{vram:?} whisper={whisper}");
            assert_eq!(row(vram, whisper, true), with_projector, "{vram:?} whisper={whisper}, projector");
        }
    }

    fn gguf(pairs: &[(&str, u32, Vec<u8>)]) -> Vec<u8> {
        let mut b = b"GGUF".to_vec();
        b.extend(3u32.to_le_bytes());
        b.extend(0u64.to_le_bytes());
        b.extend((pairs.len() as u64).to_le_bytes());
        for (key, kind, value) in pairs {
            b.extend((key.len() as u64).to_le_bytes());
            b.extend(key.as_bytes());
            b.extend(kind.to_le_bytes());
            b.extend(value);
        }
        b
    }

    fn string(s: &str) -> Vec<u8> {
        let mut b = (s.len() as u64).to_le_bytes().to_vec();
        b.extend(s.as_bytes());
        b
    }

    #[test]
    fn the_layer_count_is_read_from_the_header_past_strings_and_arrays() {
        let mut tokens = 8u32.to_le_bytes().to_vec();
        tokens.extend(2u64.to_le_bytes());
        tokens.extend(string("a"));
        tokens.extend(string("bb"));
        let file = gguf(&[("general.architecture", 8, string("qwen35")), ("t.tokens", 9, tokens), ("general.alignment", 4, 32u32.to_le_bytes().to_vec()), ("qwen35.block_count", 4, 24u32.to_le_bytes().to_vec())]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        std::fs::write(&path, &file).unwrap();
        assert_eq!(gguf_layer_count(&path), Some(24));
        std::fs::write(&path, b"not a model").unwrap();
        assert_eq!(gguf_layer_count(&path), None);
        std::fs::write(&path, &file[..40]).unwrap();
        assert_eq!(gguf_layer_count(&path), None, "a cut file");
        assert_eq!(gguf_layer_count(&dir.path().join("absent")), None);
    }
}
