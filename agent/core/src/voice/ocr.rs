//! A window that UI Automation says nothing about (a game, an Electron program without accessibility, a picture) is read
//! as text instead (TASKS.md T30.6): a picture of the active window goes to `Windows.Media.Ocr` (offline, in the languages
//! of Windows) and every block of text becomes an element of the snapshot with an id of its own. The model still only picks
//! an id; a press is a click in the middle of the block. The picture lives in memory only and is dropped after the reading.

use super::ui_snapshot::{Element, Rect, Snapshot, MAX_ELEMENTS};

/// A snapshot with fewer elements than this is read as text too.
pub const FEW: usize = 3;
/// Words of one line farther apart than this many heights of a letter belong to different blocks (two menu entries).
const GAP: f64 = 1.2;
const MAX_NAME: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A zero in front of small letters is the letter O the recognition mistook ("0ptions").
fn letters(word: &str) -> String {
    match word.strip_prefix('0') {
        Some(rest) if rest.starts_with(char::is_lowercase) => format!("O{rest}"),
        _ => word.to_string(),
    }
}

/// The words of the lines joined into blocks: neighbours stay together, a wide gap parts them.
pub fn blocks(lines: &[Vec<Word>]) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    for line in lines {
        let mut block: Option<Word> = None;
        for w in line.iter().filter(|w| !w.text.trim().is_empty() && w.width > 0.0 && w.height > 0.0) {
            match block.as_mut() {
                Some(b) if w.x - (b.x + b.width) <= GAP * b.height.max(w.height) => {
                    let right = (b.x + b.width).max(w.x + w.width);
                    let bottom = (b.y + b.height).max(w.y + w.height);
                    (b.x, b.y) = (b.x.min(w.x), b.y.min(w.y));
                    (b.width, b.height) = (right - b.x, bottom - b.y);
                    // Chinese and Japanese are written without spaces.
                    let cjk = |c: char| matches!(c, '\u{3040}'..='\u{30ff}' | '\u{4e00}'..='\u{9fff}');
                    if !(b.text.ends_with(cjk) && w.text.starts_with(cjk)) {
                        b.text.push(' ');
                    }
                    b.text.push_str(&letters(w.text.trim()));
                }
                _ => {
                    out.extend(block.replace(Word { text: letters(w.text.trim()), ..w.clone() }));
                }
            }
        }
        out.extend(block);
    }
    // A single sign (an icon read as a letter) is nothing to press.
    out.retain(|b| b.text.chars().filter(|c| c.is_alphanumeric()).count() >= 2);
    out
}

/// The elements UI Automation found, followed by the blocks of text. `origin`: the place of the picture on the screen;
/// `scale`: screen pixels per pixel of the picture.
pub fn with_text(base: Snapshot, blocks: &[Word], origin: (i32, i32), scale: f64) -> Snapshot {
    let mut out = base;
    out.text = true;
    for b in blocks {
        if out.elements.len() >= MAX_ELEMENTS {
            out.truncated = true;
            break;
        }
        let at = |v: f64| (v * scale).round() as i32;
        out.elements.push(Element {
            id: out.elements.len() as u32 + 1,
            role: "text".into(),
            name: b.text.chars().take(MAX_NAME).collect(),
            automation_id: String::new(),
            rect: Rect { x: origin.0 + at(b.x), y: origin.1 + at(b.y), width: at(b.width).max(1), height: at(b.height).max(1) },
            parent_name: String::new(),
        });
    }
    out
}

#[cfg(windows)]
mod win {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    use super::Word;

    /// The lines of text of a picture (BGRA), as words with their places.
    pub fn read(pixels: &[u8], width: u32, height: u32) -> windows::core::Result<Vec<Vec<Word>>> {
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let writer = DataWriter::new()?;
        writer.WriteBytes(pixels)?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&writer.DetachBuffer()?, BitmapPixelFormat::Bgra8, width as i32, height as i32)?;
        let found = engine.RecognizeAsync(&bitmap)?.join()?;
        let mut lines = Vec::new();
        for line in found.Lines()? {
            let mut words = Vec::new();
            for word in line.Words()? {
                let r = word.BoundingRect()?;
                words.push(Word { text: word.Text()?.to_string(), x: f64::from(r.X), y: f64::from(r.Y), width: f64::from(r.Width), height: f64::from(r.Height) });
            }
            lines.push(words);
        }
        Ok(lines)
    }

    /// The longest side the recognition takes.
    pub fn max_side() -> u32 {
        OcrEngine::MaxImageDimension().unwrap_or(2_600)
    }
}

/// The blocks of text of a picture (BGRA) and the number of its pixels per pixel that was read. `Err`: Windows has no
/// language for recognition.
#[cfg(windows)]
pub fn read_picture(pixels: Vec<u8>, width: u32, height: u32) -> Result<(Vec<Word>, f64), String> {
    crate::sta::run(move || {
        let side = win::max_side();
        let long = width.max(height);
        let (pixels, w, h) = if long > side {
            let (w, h) = ((u64::from(width) * u64::from(side) / u64::from(long)).max(1) as u32, (u64::from(height) * u64::from(side) / u64::from(long)).max(1) as u32);
            (super::screen::downscale(&pixels, width, height, w, h), w, h)
        } else {
            (pixels, width, height)
        };
        let lines = win::read(&pixels, w, h).map_err(|e| format!("The text of the window cannot be read: {e}"))?;
        Ok::<_, String>((blocks(&lines), f64::from(width) / f64::from(w)))
    })?
}

/// `base` (what UI Automation said about the active window) with the text seen in the window. `None`: the window cannot be
/// pictured or read.
#[cfg(windows)]
pub fn active_window(base: Snapshot) -> Option<Snapshot> {
    let (pixels, width, height, origin) = super::screen::capture_active_window_at().map_err(|e| tracing::debug!("screen text: {e}")).ok()?;
    let (blocks, scale) = read_picture(pixels, width, height).map_err(|e| tracing::warn!("screen text: {e}")).ok()?;
    Some(with_text(base, &blocks, origin, scale))
}

#[cfg(not(windows))]
pub fn active_window(_: Snapshot) -> Option<Snapshot> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, x: f64, width: f64) -> Word {
        Word { text: text.into(), x, y: 10.0, width, height: 20.0 }
    }

    #[test]
    fn near_words_make_a_block_and_a_wide_gap_parts_them() {
        let lines = [vec![word("Start", 0.0, 50.0), word("Game", 58.0, 50.0), word("Options", 200.0, 70.0), word("x", 400.0, 8.0)], vec![word("終了", 0.0, 40.0), word("する", 41.0, 40.0)]];
        let found = blocks(&lines);
        assert_eq!(found.iter().map(|b| b.text.as_str()).collect::<Vec<_>>(), ["Start Game", "Options", "終了する"]);
        assert_eq!((found[0].x, found[0].width), (0.0, 108.0));
        assert_eq!((letters("0ptions"), letters("0.5"), letters("007")), ("Options".to_string(), "0.5".to_string(), "007".to_string()));
    }

    #[test]
    fn the_blocks_become_elements_on_the_screen_after_those_of_ui_automation() {
        let base = Snapshot { elements: vec![Element { id: 1, role: "button".into(), name: "Close".into(), automation_id: String::new(), rect: Rect::default(), parent_name: String::new() }], ..Default::default() };
        let s = with_text(base, &[word("Play", 10.0, 40.0)], (100, 200), 2.0);
        assert!(s.text);
        assert_eq!(s.ids(), [1, 2]);
        let play = s.get(2).unwrap();
        assert_eq!((play.role.as_str(), play.name.as_str(), play.rect), ("text", "Play", Rect { x: 120, y: 220, width: 80, height: 40 }));
        let many: Vec<Word> = (0..300).map(|i| word(&format!("w{i:03}"), 0.0, 10.0)).collect();
        let full = with_text(Snapshot::default(), &many, (0, 0), 1.0);
        assert!(full.truncated && full.elements.len() == MAX_ELEMENTS);
    }

    /// The picture `tests/fixtures/screen_text.png` shows "Start Game" at (60, 40), "Options" at (60, 140) and "Quit" at
    /// (60, 240) in 48 px letters. A PC without a recognition language cannot run it: said, not failed.
    #[cfg(windows)]
    #[test]
    fn the_text_of_a_picture_is_found_where_it_is() {
        let png = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/screen_text.png")).unwrap();
        let (pixels, w, h) = crate::sta::run(move || {
            let (w, h) = crate::imaging::decoded_size(&png).unwrap();
            (crate::imaging::scaled_pixels(&png, w, h, windows::Graphics::Imaging::BitmapAlphaMode::Premultiplied).unwrap(), w, h)
        })
        .unwrap();
        let (found, scale) = match read_picture(pixels, w, h) {
            Ok(f) => f,
            Err(e) => return println!("skipped: {e}"),
        };
        assert_eq!(scale, 1.0);
        for (text, y) in [("Start Game", 40.0), ("Options", 140.0), ("Quit", 240.0)] {
            let block = found.iter().find(|b| b.text.eq_ignore_ascii_case(text)).unwrap_or_else(|| panic!("{text} not in {found:?}"));
            assert!((40.0..=100.0).contains(&block.x) && (y - 10.0..=y + 40.0).contains(&block.y), "{block:?}");
        }
    }
}
