//! Icon store (M12, DATA_MODEL.md 8): `<configDir>\icons\<hash16>.png`, PNG with transparency (96x96 before M14, 256x256 now).
//! Icons come from the PC itself (the Shell icon of an exe, or a picture file); nothing is downloaded or shipped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use base64::Engine;
use sha2::{Digest, Sha256};

use crate::model::{LayoutFile, LayoutItem, ValidationIssue};

/// Side of a new icon in pixels (M14, DATA_MODEL.md 8.1: big icons need a sharp source).
pub const ICON_SIDE: u32 = 256;
/// Smaller sides tried when the 256 px PNG does not fit `MAX_DATA_URL_CHARS`.
pub const FALLBACK_SIDES: [u32; 2] = [192, 128];
/// A data URL longer than this is refused.
pub const MAX_DATA_URL_CHARS: usize = 131_072;
const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

pub fn icons_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("icons")
}

/// `hash16`: 16 lower-case hex characters.
pub fn is_hash(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The hash of `img:<hash16>`; anything else (other icon kinds, malformed) -> `None`.
pub fn img_hash(icon: &str) -> Option<&str> {
    icon.strip_prefix("img:").filter(|h| is_hash(h))
}

/// First 16 hex characters of the SHA-256 of the PNG.
pub fn hash_of(png: &[u8]) -> String {
    Sha256::digest(png).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

/// Size a `w` x `h` picture is scaled to so that it fits `side` x `side`, keeping proportions (it is enlarged too).
pub fn fit_into(w: u32, h: u32, side: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (side, side);
    }
    let longest = u64::from(w.max(h));
    let scale = |v: u32| ((u64::from(v) * u64::from(side) + longest / 2) / longest).clamp(1, u64::from(side)) as u32;
    (scale(w), scale(h))
}

/// Puts BGRA pixels in the middle of a transparent `side` x `side` canvas (the source must not be larger).
pub fn blit_centered(src: &[u8], w: u32, h: u32, side: u32) -> Vec<u8> {
    let mut canvas = vec![0u8; (side * side * 4) as usize];
    let (x0, y0) = ((side.saturating_sub(w)) / 2, (side.saturating_sub(h)) / 2);
    for y in 0..h.min(side) {
        let from = (y * w * 4) as usize;
        let to = (((y0 + y) * side + x0) * 4) as usize;
        let len = (w.min(side) * 4) as usize;
        canvas[to..to + len].copy_from_slice(&src[from..from + len]);
    }
    canvas
}

/// Writes the PNG into the store (once: same content, same file) and returns `img:<hash16>`.
pub fn save_icon(config_dir: &Path, png: &[u8]) -> Result<String, String> {
    if data_url(png).len() > MAX_DATA_URL_CHARS {
        return Err("Icon is too large".to_string());
    }
    let hash = hash_of(png);
    let dir = icons_dir(config_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create the icons folder: {e}"))?;
    let file = dir.join(format!("{hash}.png"));
    if !file.exists() {
        std::fs::write(&file, png).map_err(|e| format!("cannot write the icon: {e}"))?;
    }
    Ok(format!("img:{hash}"))
}

/// The data URL of a stored icon (a PNG, or an SVG from Phosphor); missing or oversized files -> `None`.
pub fn icon_data_url(config_dir: &Path, hash: &str) -> Option<String> {
    if !is_hash(hash) {
        return None;
    }
    let dir = icons_dir(config_dir);
    if let Ok(png) = std::fs::read(dir.join(format!("{hash}.png"))) {
        let url = data_url(&png);
        return (url.len() <= MAX_DATA_URL_CHARS).then_some(url);
    }
    let svg = std::fs::read(dir.join(format!("{hash}.svg"))).ok()?;
    let url = svg_data_url(&svg);
    (url.len() <= MAX_SVG_DATA_URL_CHARS).then_some(url)
}

fn icon_file_exists(dir: &Path, hash: &str) -> bool {
    dir.join(format!("{hash}.png")).exists() || dir.join(format!("{hash}.svg")).exists()
}

// ===== SVG icons (DATA_MODEL.md 8.4) =====

/// Longest SVG markup accepted.
pub const MAX_SVG_CHARS: usize = 12_000;
/// An SVG data URL longer than this is refused.
pub const MAX_SVG_DATA_URL_CHARS: usize = 16_384;
const UNSUPPORTED_SVG: &str = "Unsupported SVG";

pub fn svg_data_url(svg: &[u8]) -> String {
    format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg))
}

/// Checks that `svg` is plain vector markup from the icon library and nothing that can run or load something.
pub fn validate_svg(svg: &str) -> Result<(), String> {
    let bad = || Err(UNSUPPORTED_SVG.to_string());
    if svg.chars().count() > MAX_SVG_CHARS || !svg.trim_start().to_ascii_lowercase().starts_with("<svg") {
        return bad();
    }
    let lower = svg.to_ascii_lowercase();
    if ["<script", "<foreignobject", "javascript:"].iter().any(|needle| lower.contains(needle)) {
        return bad();
    }
    if !lower.contains("viewbox=") || has_event_attribute(&lower) || has_external_href(&lower) {
        return bad();
    }
    Ok(())
}

/// An attribute such as ` onload=`: whitespace, `on`, letters, optional spaces, `=`.
fn has_event_attribute(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut i = 0;
    while let Some(pos) = lower[i..].find("on") {
        let at = i + pos;
        i = at + 2;
        let after_space = at > 0 && (bytes[at - 1].is_ascii_whitespace() || bytes[at - 1] == b'"' || bytes[at - 1] == b'\'');
        if !after_space {
            continue;
        }
        let mut j = at + 2;
        while j < bytes.len() && bytes[j].is_ascii_lowercase() {
            j += 1;
        }
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j > at + 2 && j < bytes.len() && bytes[j] == b'=' {
            return true;
        }
    }
    false
}

/// An `href` (also `xlink:href`) whose value does not point inside the document (`#...`).
fn has_external_href(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut i = 0;
    while let Some(pos) = lower[i..].find("href") {
        let mut j = i + pos + 4;
        i = j;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            continue;
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
            j += 1;
        }
        if bytes.get(j) != Some(&b'#') {
            return true;
        }
    }
    false
}

/// Rewrites every ` name="value"` / ` name='value'` of the markup with `f(value)`; `None` drops the attribute.
fn rewrite_attr(svg: &str, name: &str, f: impl Fn(&str) -> Option<String>) -> String {
    let needle = format!(" {name}=");
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(pos) = rest.find(&needle) {
        let after = &rest[pos + needle.len()..];
        let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            out.push_str(&rest[..pos + needle.len()]);
            rest = after;
            continue;
        };
        let Some(end) = after[1..].find(quote) else { break };
        let value = &after[1..1 + end];
        out.push_str(&rest[..pos]);
        if let Some(new) = f(value) {
            out.push_str(&format!(" {name}={quote}{new}{quote}"));
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// What gets stored: fills and strokes other than `none` become `#000` (a mask only needs the alpha), and the root
/// loses `width`/`height` so the picture scales with its box.
pub fn normalize_svg(svg: &str) -> String {
    let paint = |v: &str| Some(if v.trim().eq_ignore_ascii_case("none") { v.to_string() } else { "#000".to_string() });
    let painted = rewrite_attr(&rewrite_attr(svg.trim(), "fill", paint), "stroke", paint);
    // The root tag is the first `<svg ...>`.
    let Some(end) = painted.find('>') else { return painted };
    let (root, rest) = painted.split_at(end);
    let root = rewrite_attr(&rewrite_attr(root, "width", |_| None), "height", |_| None);
    format!("{root}{rest}")
}

/// Stores an SVG from the icon library as `<hash16>.svg` and returns `img:<hash16>` (same markup, same file).
pub fn icon_from_svg(config_dir: &Path, svg: &str) -> Result<String, String> {
    validate_svg(svg)?;
    let normalized = normalize_svg(svg);
    if svg_data_url(normalized.as_bytes()).len() > MAX_SVG_DATA_URL_CHARS {
        return Err(UNSUPPORTED_SVG.to_string());
    }
    let hash = hash_of(normalized.as_bytes());
    let dir = icons_dir(config_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create the icons folder: {e}"))?;
    let file = dir.join(format!("{hash}.svg"));
    if !file.exists() {
        std::fs::write(&file, normalized).map_err(|e| format!("cannot write the icon: {e}"))?;
    }
    Ok(format!("img:{hash}"))
}

/// Data URL for a settings-window preview of an `img:` icon.
pub fn icon_preview(config_dir: &Path, icon: &str) -> Option<String> {
    icon_data_url(config_dir, img_hash(icon)?)
}

/// Every `img:` icon the layout refers to (buttons and their state looks).
pub fn referenced_hashes(layout: &LayoutFile) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for page in &layout.pages {
        for item in &page.items {
            if let LayoutItem::Button(b) = item {
                out.extend(b.icon.as_deref().and_then(img_hash).map(str::to_string));
                for look in b.bind.iter().flat_map(|bind| bind.states.values()) {
                    out.extend(look.icon.as_deref().and_then(img_hash).map(str::to_string));
                }
            }
        }
    }
    out
}

/// The `icons` section: `{ hash16: dataURL }` for the icons the layout uses (missing files are left out).
pub fn collect_icons(config_dir: &Path, layout: &LayoutFile) -> serde_json::Value {
    let map: BTreeMap<String, String> = referenced_hashes(layout)
        .into_iter()
        .filter_map(|hash| icon_data_url(config_dir, &hash).map(|url| (hash, url)))
        .collect();
    serde_json::to_value(map).expect("icons serialize")
}

/// DATA_MODEL.md 8.2, rule 11: an `img:` icon needs its file.
pub fn missing_icon_issues(config_dir: &Path, layout: &LayoutFile) -> Vec<ValidationIssue> {
    let dir = icons_dir(config_dir);
    let missing = |icon: &Option<String>| icon.as_deref().and_then(img_hash).is_some_and(|h| !icon_file_exists(&dir, h));
    let issue = |path: String| ValidationIssue { path, message: "icon file not found — choose the icon again".to_string() };
    let mut out = Vec::new();
    for (pi, page) in layout.pages.iter().enumerate() {
        for (ii, item) in page.items.iter().enumerate() {
            let LayoutItem::Button(b) = item else { continue };
            let base = format!("/pages/{pi}/items/{ii}");
            if missing(&b.icon) {
                out.push(issue(format!("{base}/icon")));
            }
            for (key, look) in b.bind.iter().flat_map(|bind| bind.states.iter()) {
                if missing(&look.icon) {
                    out.push(issue(format!("{base}/bind/states/{}/icon", key.replace('~', "~0").replace('/', "~1"))));
                }
            }
        }
    }
    out
}

/// Width and height of a PNG from its IHDR chunk; anything else -> `None`.
pub fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return None;
    }
    let be = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    Some((be(16), be(20)))
}

// ===== Icon clean-up (DATA_MODEL.md 8.1, "small icons") =====

/// Version of the icon processing (trim, margin, frame cut). Icons stored by an older version are refreshed.
pub const PROCESSING_VERSION: u32 = 2;
/// Pixels with an alpha above this count as content when trimming.
const CONTENT_ALPHA: u8 = 16;
/// Share of the side kept free on every edge around the content.
const CONTENT_MARGIN: f32 = 0.06;

/// Side of the box the trimmed content is fitted into: the canvas minus a 6% margin on each edge.
pub fn inner_side(side: u32) -> u32 {
    side - 2 * (side as f32 * CONTENT_MARGIN).round() as u32
}

/// Bounding box `(x0, y0, x1, y1)` (end exclusive) of the pixels with alpha above 16; `None` when nothing is drawn.
pub fn content_bounds(bgra: &[u8], w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if bgra[((y * w + x) * 4 + 3) as usize] > CONTENT_ALPHA {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

/// The part of the picture inside the box.
pub fn crop_bgra(bgra: &[u8], w: u32, bounds: (u32, u32, u32, u32)) -> (Vec<u8>, u32, u32) {
    let (x0, y0, x1, y1) = bounds;
    let mut out = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
    for y in y0..y1 {
        out.extend_from_slice(&bgra[((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize]);
    }
    (out, x1 - x0, y1 - y0)
}

/// Thickness (0, 1 or 2 px) of a one-colour frame round the picture: every pixel of the outer ring has alpha above 200
/// and the same colour (within 8 per channel). Such a frame is drawn by the Shell, not by the program.
pub fn frame_width(bgra: &[u8], w: u32, h: u32) -> u32 {
    let at = |x: u32, y: u32| &bgra[((y * w + x) * 4) as usize..((y * w + x) * 4 + 4) as usize];
    let ring_is_flat = |t: u32, reference: &[u8]| -> bool {
        if w <= 2 * t + 4 || h <= 2 * t + 4 {
            return false;
        }
        let near = |p: &[u8]| p[3] > 200 && p[..3].iter().zip(&reference[..3]).all(|(a, b)| a.abs_diff(*b) <= 8);
        (t..w - t).all(|x| near(at(x, t)) && near(at(x, h - 1 - t))) && (t..h - t).all(|y| near(at(t, y)) && near(at(w - 1 - t, y)))
    };
    let reference = at(0, 0).to_vec();
    if reference[3] <= 200 || !ring_is_flat(0, &reference) {
        return 0;
    }
    if ring_is_flat(1, &reference) { 2 } else { 1 }
}

/// Trims transparent margins; for a Shell image also cuts the Shell frame and trims again. A picture with nothing drawn
/// is returned as it is.
pub fn refine_icon(bgra: Vec<u8>, w: u32, h: u32, from_shell: bool) -> (Vec<u8>, u32, u32) {
    let Some(bounds) = content_bounds(&bgra, w, h) else { return (bgra, w, h) };
    let (mut px, mut pw, mut ph) = crop_bgra(&bgra, w, bounds);
    if from_shell {
        let f = frame_width(&px, pw, ph);
        if f > 0 {
            let inner = crop_bgra(&px, pw, (f, f, pw - f, ph - f));
            (px, pw, ph) = inner;
            if let Some(b) = content_bounds(&px, pw, ph) {
                (px, pw, ph) = crop_bgra(&px, pw, b);
            }
        }
    }
    (px, pw, ph)
}

fn index_path(config_dir: &Path) -> PathBuf {
    icons_dir(config_dir).join("index.json")
}

fn read_index(config_dir: &Path) -> BTreeMap<String, u32> {
    std::fs::read(index_path(config_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Serialises read-modify-write of `index.json` (icons are made on blocking threads).
static INDEX_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Remembers that the file of `icon` (`img:<hash16>`) was made by the current processing version.
pub fn mark_current(config_dir: &Path, icon: &str) {
    let Some(hash) = img_hash(icon) else { return };
    let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut index = read_index(config_dir);
    if index.get(hash) == Some(&PROCESSING_VERSION) {
        return;
    }
    index.insert(hash.to_string(), PROCESSING_VERSION);
    if let Ok(text) = serde_json::to_vec(&index) {
        let _ = std::fs::write(index_path(config_dir), text);
    }
}

/// Whether the file of `hash` was made by the current processing version.
fn is_current(index: &BTreeMap<String, u32>, hash: &str) -> bool {
    index.get(hash).is_some_and(|v| *v >= PROCESSING_VERSION)
}

/// Tries `render(side)` for 256, 192 and 128 px until the PNG fits the data URL limit, then stores it.
pub fn save_first_that_fits(config_dir: &Path, mut render: impl FnMut(u32) -> Result<Vec<u8>, String>) -> Result<String, String> {
    for side in std::iter::once(ICON_SIDE).chain(FALLBACK_SIDES) {
        let png = render(side)?;
        if data_url(&png).len() <= MAX_DATA_URL_CHARS {
            return save_icon(config_dir, &png);
        }
    }
    Err("Icon is too large".to_string())
}

/// Icon of a program (`.exe`) from the Shell, stored as `img:<hash16>`.
pub fn icon_from_program(config_dir: &Path, path: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        let (dir, path) = (config_dir.to_path_buf(), path.to_string());
        crate::sta::run(move || {
            let icon = save_first_that_fits(&dir, |side| win::program_png(&path, side))?;
            mark_current(&dir, &icon);
            Ok(icon)
        })?
    }
    #[cfg(not(windows))]
    {
        let _ = (config_dir, path);
        Err("Program icons are available on Windows only".to_string())
    }
}

/// The buttons whose icon is a stored PNG made before the current processing version (`index.json`) and whose action
/// launches a program: `(page index, item index, program)`. Those are the ones `refresh_program_icons` can redo.
pub fn lowres_launch_buttons(config_dir: &Path, layout: &LayoutFile, actions: &crate::model::ActionsFile) -> Vec<(usize, usize, String)> {
    let dir = icons_dir(config_dir);
    let index = read_index(config_dir);
    let mut out = Vec::new();
    for (pi, page) in layout.pages.iter().enumerate() {
        for (ii, item) in page.items.iter().enumerate() {
            let LayoutItem::Button(b) = item else { continue };
            let Some(hash) = b.icon.as_deref().and_then(img_hash) else { continue };
            let Some(crate::model::Action::Launch { program, .. }) = b.action_id.as_ref().and_then(|id| actions.actions.get(id)) else { continue };
            // An SVG icon has no .png file and is left alone.
            if !dir.join(format!("{hash}.png")).exists() {
                continue;
            }
            if !is_current(&index, hash) {
                out.push((pi, ii, program.clone()));
            }
        }
    }
    out
}

/// How many buttons `refresh_program_icons` would sharpen (the `Look` tab shows a line when it is not 0).
pub fn count_lowres_icons(config_dir: &Path, layout: &LayoutFile, actions: &crate::model::ActionsFile) -> usize {
    lowres_launch_buttons(config_dir, layout, actions).len()
}

/// Extracts the icon of every low-resolution program button again (256 px) and puts the new `img:` in the layout;
/// returns how many were replaced and how many could not be extracted. Old files stay (other buttons may use them).
pub fn refresh_program_icons(config_dir: &Path, layout: &mut LayoutFile, actions: &crate::model::ActionsFile) -> (usize, usize) {
    let (mut updated, mut failed) = (0, 0);
    let mut made: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (pi, ii, program) in lowres_launch_buttons(config_dir, layout, actions) {
        let icon = made.entry(program.to_lowercase()).or_insert_with(|| icon_from_program(config_dir, &program).ok()).clone();
        match (icon, layout.pages.get_mut(pi).and_then(|p| p.items.get_mut(ii))) {
            (Some(icon), Some(LayoutItem::Button(b))) => {
                b.icon = Some(icon);
                updated += 1;
            }
            _ => failed += 1,
        }
    }
    (updated, failed)
}

/// Icon from a picture file (png, jpg, ico, bmp), stored as `img:<hash16>`.
pub fn icon_from_image(config_dir: &Path, path: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        let meta = std::fs::metadata(path).map_err(|e| format!("cannot read the file: {e}"))?;
        if !meta.is_file() || meta.len() > MAX_IMAGE_BYTES {
            return Err("Choose a picture file smaller than 20 MB".to_string());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read the file: {e}"))?;
        let dir = config_dir.to_path_buf();
        crate::sta::run(move || {
            let icon = save_first_that_fits(&dir, |side| win::picture_png(&bytes, side))?;
            mark_current(&dir, &icon);
            Ok(icon)
        })?
    }
    #[cfg(not(windows))]
    {
        let _ = (config_dir, path);
        Err("Picture icons are available on Windows only".to_string())
    }
}

#[cfg(windows)]
pub(crate) mod win {
    use std::ffi::c_void;

    use windows::core::HSTRING;
    use windows::Graphics::Imaging::BitmapAlphaMode;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ};
    use windows::Win32::System::Com::IBindCtx;
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SHDefExtractIconW, SIIGBF_ICONONLY};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

    use super::{blit_centered, fit_into};
    use crate::imaging;

    fn err(what: &str, e: windows::core::Error) -> String {
        format!("{what}: {}", e.message())
    }

    /// Reads (and deletes) a GDI bitmap as top-down 32-bit BGRA pixels.
    fn read_bitmap(hbitmap: HBITMAP) -> Result<(Vec<u8>, u32, u32), String> {
        // SAFETY: plain GDI calls; the bitmap is deleted before returning, the device context is released.
        unsafe {
            let object = HGDIOBJ(hbitmap.0);
            let mut bitmap = BITMAP::default();
            if GetObjectW(object, std::mem::size_of::<BITMAP>() as i32, Some(&mut bitmap as *mut BITMAP as *mut c_void)) == 0 {
                let _ = DeleteObject(object);
                return Err("cannot read the icon bitmap".to_string());
            }
            let (w, h) = (bitmap.bmWidth.max(1), bitmap.bmHeight.max(1));
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            let hdc = GetDC(None);
            let lines = GetDIBits(hdc, hbitmap, 0, h as u32, Some(pixels.as_mut_ptr() as *mut c_void), &mut info, DIB_RGB_COLORS);
            ReleaseDC(None, hdc);
            let _ = DeleteObject(object);
            if lines == 0 {
                return Err("cannot read the icon pixels".to_string());
            }
            Ok((pixels, w as u32, h as u32))
        }
    }

    /// Icons without an alpha channel come back fully transparent: make them opaque.
    fn opaque_if_no_alpha(pixels: &mut [u8]) {
        if pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
            pixels.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p[3] = 255);
        }
    }

    /// BGRA (premultiplied) pixels, width and height of the Shell icon of `path` at `size` px.
    pub fn shell_icon_bgra(path: &str, size: i32) -> Result<(Vec<u8>, u32, u32), String> {
        let wide = HSTRING::from(path);
        // SAFETY: plain Shell calls; the bitmap is handed to `read_bitmap`, which deletes it.
        let hbitmap = unsafe {
            let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&wide, None::<&IBindCtx>).map_err(|e| err("cannot open the file", e))?;
            factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY).map_err(|e| err("the file has no icon", e))?
        };
        let (mut pixels, w, h) = read_bitmap(hbitmap)?;
        opaque_if_no_alpha(&mut pixels);
        Ok((pixels, w, h))
    }

    /// BGRA pixels of the biggest icon in the resources of `path` (up to 256 px), without the Shell frame; `None` when the
    /// file has no icon of its own or anything goes wrong (the caller falls back to `shell_icon_bgra`).
    pub fn extracted_icon_bgra(path: &str) -> Option<(Vec<u8>, u32, u32)> {
        let wide = HSTRING::from(path);
        let mut large = HICON::default();
        // SAFETY: the icon handle is destroyed on every path; GetIconInfo bitmaps go to `read_bitmap`, which deletes them.
        unsafe {
            if SHDefExtractIconW(&wide, 0, 0, Some(&mut large), None, 256).is_err() || large.is_invalid() {
                return None;
            }
            let mut info = ICONINFO::default();
            let got = GetIconInfo(large, &mut info);
            let _ = DestroyIcon(large);
            got.ok()?;
            if info.hbmColor.is_invalid() {
                if !info.hbmMask.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
                }
                return None;
            }
            let color = read_bitmap(info.hbmColor);
            let mask = if info.hbmMask.is_invalid() { None } else { read_bitmap(info.hbmMask).ok() };
            let (mut pixels, w, h) = color.ok()?;
            if pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
                // An icon without alpha: the AND mask says which pixels are drawn (black = drawn).
                match mask {
                    Some((m, mw, mh)) if mw == w && mh == h => {
                        for (p, q) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(m.as_chunks::<4>().0.iter()) {
                            p[3] = if q[0] < 128 { 255 } else { 0 };
                        }
                    }
                    _ => opaque_if_no_alpha(&mut pixels),
                }
            }
            Some((pixels, w, h))
        }
    }

    /// `side` x `side` PNG of the Shell icon of a program: the biggest icon of the file, else the Shell image with its
    /// frame cut off; transparent margins are trimmed and the content is fitted with a 6% margin (DATA_MODEL.md 8.1).
    pub fn program_png(path: &str, side: u32) -> Result<Vec<u8>, String> {
        if !std::path::Path::new(path).is_file() {
            return Err("The program was not found".to_string());
        }
        let (pixels, w, h, from_shell) = match extracted_icon_bgra(path) {
            Some((p, w, h)) => (p, w, h, false),
            None => {
                let (p, w, h) = shell_icon_bgra(path, side as i32)?;
                (p, w, h, true)
            }
        };
        let (pixels, w, h) = super::refine_icon(pixels, w, h, from_shell);
        let png = imaging::encode_png(&pixels, w, h).map_err(|e| err("cannot encode the icon", e))?;
        fit_and_center(&png, w, h, super::inner_side(side), side)
    }

    /// `side` x `side` PNG of a picture file's content.
    pub fn picture_png(bytes: &[u8], side: u32) -> Result<Vec<u8>, String> {
        let (w, h) = imaging::decoded_size(bytes).map_err(|_| "This file is not a picture Windows can read".to_string())?;
        fit_and_center(bytes, w, h, side, side)
    }

    /// Scale the encoded picture to fit `inner` x `inner` (keeping proportions, Fant) and centre it on a transparent
    /// `side` x `side` canvas.
    fn fit_and_center(encoded: &[u8], w: u32, h: u32, inner: u32, side: u32) -> Result<Vec<u8>, String> {
        let (tw, th) = fit_into(w, h, inner);
        let pixels = imaging::scaled_pixels(encoded, tw, th, BitmapAlphaMode::Premultiplied).map_err(|e| err("cannot scale the picture", e))?;
        imaging::encode_png(&blit_centered(&pixels, tw, th, side), side, side).map_err(|e| err("cannot encode the icon", e))
    }

    /// 32x32 PNG data URL of the Shell icon of a program (for lists); `None` on any error.
    pub fn list_icon_data_url(path: &str) -> Option<String> {
        let (pixels, w, h) = shell_icon_bgra(path, 32).ok()?;
        let png = if w == 32 && h == 32 { imaging::encode_png(&pixels, w, h).ok()? } else { compose_to(&pixels, w, h, 32)? };
        Some(super::data_url(&png))
    }

    fn compose_to(pixels: &[u8], w: u32, h: u32, side: u32) -> Option<Vec<u8>> {
        let png = imaging::encode_png(pixels, w, h).ok()?;
        let (tw, th) = fit_into(w, h, side);
        let scaled = imaging::scaled_pixels(&png, tw, th, BitmapAlphaMode::Premultiplied).ok()?;
        imaging::encode_png(&blit_centered(&scaled, tw, th, side), side, side).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_with(icon: &str, state_icon: &str) -> LayoutFile {
        let text = format!(
            r#"{{ "version": 1, "grid": {{ "cols": 5, "rows": 3 }},
              "display": {{ "dimAfterSec": 60, "dimLevel": 0.3, "sleepAfterSec": 600, "pixelShift": true, "night": {{ "enabled": false, "from": "23:00", "to": "07:00" }} }},
              "pages": [ {{ "id": "main", "name": "Main", "items": [
                {{ "id": "b1", "kind": "button", "x": 0, "y": 0, "w": 1, "h": 1, "icon": "{icon}", "actionId": "lock",
                   "bind": {{ "var": "v", "states": {{ "true": {{ "icon": "{state_icon}" }} }} }} }} ] }} ] }}"#
        );
        serde_json::from_str(&text).expect("layout parses")
    }

    #[test]
    fn hashes_are_16_lower_case_hex() {
        assert!(is_hash("0123456789abcdef"));
        assert!(!is_hash("0123456789ABCDEF"));
        assert!(!is_hash("0123"));
        assert!(!is_hash("0123456789abcdeg"));
        assert_eq!(img_hash("img:0123456789abcdef"), Some("0123456789abcdef"));
        assert_eq!(img_hash("lucide:play"), None);
        assert_eq!(img_hash("img:XYZ"), None);
    }

    #[test]
    fn the_same_content_has_the_same_hash() {
        assert_eq!(hash_of(b"abc"), hash_of(b"abc"));
        assert_ne!(hash_of(b"abc"), hash_of(b"abd"));
        assert!(is_hash(&hash_of(b"abc")));
        // SHA-256("abc") starts with ba7816bf8f01cfea
        assert_eq!(hash_of(b"abc"), "ba7816bf8f01cfea");
    }

    #[test]
    fn pictures_fit_the_box_keeping_proportions() {
        assert_eq!(fit_into(600, 600, 96), (96, 96));
        assert_eq!(fit_into(600, 300, 96), (96, 48));
        assert_eq!(fit_into(32, 32, 96), (96, 96));
        assert_eq!(fit_into(1, 1000, 96), (1, 96));
    }

    #[test]
    fn a_small_picture_is_centered_on_a_transparent_canvas() {
        let src = vec![255u8; 2 * 2 * 4];
        let canvas = blit_centered(&src, 2, 2, 4);
        let at = |x: usize, y: usize| canvas[(y * 4 + x) * 4 + 3];
        assert_eq!((at(0, 0), at(1, 1), at(2, 2), at(3, 3)), (0, 255, 255, 0));
    }

    #[test]
    fn saving_twice_gives_one_file_and_the_same_icon() {
        let dir = tempfile::tempdir().unwrap();
        let png = b"\x89PNG fake content";
        let a = save_icon(dir.path(), png).unwrap();
        let b = save_icon(dir.path(), png).unwrap();
        assert_eq!(a, b);
        assert_eq!(std::fs::read_dir(icons_dir(dir.path())).unwrap().count(), 1);
        assert!(icon_preview(dir.path(), &a).unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn a_huge_icon_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(save_icon(dir.path(), &vec![7u8; 100_000]), Err("Icon is too large".to_string()));
    }

    #[test]
    fn rule_11_flags_a_missing_file_for_buttons_and_states() {
        let dir = tempfile::tempdir().unwrap();
        let png = b"\x89PNG one";
        let have = save_icon(dir.path(), png).unwrap();
        let layout = layout_with(&have, "img:0123456789abcdef");
        let issues = missing_icon_issues(dir.path(), &layout);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path, "/pages/0/items/0/bind/states/true/icon");
        assert_eq!(issues[0].message, "icon file not found — choose the icon again");
        assert!(missing_icon_issues(dir.path(), &layout_with(&have, &have)).is_empty());
        assert!(missing_icon_issues(dir.path(), &layout_with("lucide:play", "emoji:x")).is_empty());
    }

    #[test]
    fn the_section_holds_only_referenced_existing_icons() {
        let dir = tempfile::tempdir().unwrap();
        let have = save_icon(dir.path(), b"\x89PNG two").unwrap();
        let hash = have.strip_prefix("img:").unwrap().to_string();
        let layout = layout_with(&have, "img:0123456789abcdef");
        let section = collect_icons(dir.path(), &layout);
        let obj = section.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert!(obj[&hash].as_str().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(collect_icons(dir.path(), &layout_with("lucide:play", "lucide:pause")), serde_json::json!({}));
    }

    // A Phosphor "regular" icon: a root with width/height-less viewBox and a fill of currentColor.
    const HEART: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M178,40c-20.65,0-38.73,8.88-50,23.89Z"/></svg>"##;

    #[test]
    fn a_phosphor_svg_is_stored_normalised_and_gives_an_img_icon() {
        let dir = tempfile::tempdir().unwrap();
        let icon = icon_from_svg(dir.path(), HEART).unwrap();
        let hash = img_hash(&icon).expect("img: + 16 hex");
        let stored = std::fs::read_to_string(icons_dir(dir.path()).join(format!("{hash}.svg"))).unwrap();
        assert!(stored.contains(r##"fill="#000""##) && !stored.contains("currentColor"), "{stored}");
        assert!(icon_data_url(dir.path(), hash).unwrap().starts_with("data:image/svg+xml;base64,"));
        // The same markup gives the same icon and no second file.
        assert_eq!(icon_from_svg(dir.path(), HEART).unwrap(), icon);
        assert_eq!(std::fs::read_dir(icons_dir(dir.path())).unwrap().count(), 1);
    }

    #[test]
    fn normalising_keeps_none_and_drops_root_size() {
        let svg = r##"<svg width="32" height="32" viewBox="0 0 256 256" fill="none"><path d="M1" stroke="currentColor" stroke-width="16" fill="red"/></svg>"##;
        let n = normalize_svg(svg);
        assert!(n.starts_with(r##"<svg viewBox="0 0 256 256" fill="none">"##), "{n}");
        assert!(n.contains(r##"stroke="#000""##) && n.contains(r##"fill="#000""##) && n.contains(r##"stroke-width="16""##), "{n}");
    }

    #[test]
    fn dangerous_or_foreign_svgs_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            r##"<svg viewBox="0 0 1 1"><script>alert(1)</script></svg>"##,
            r##"<svg viewBox="0 0 1 1" onload="alert(1)"><path d="M0"/></svg>"##,
            r##"<svg viewBox="0 0 1 1"><image href="https://evil.example/x.png"/></svg>"##,
            r##"<svg viewBox="0 0 1 1"><use xlink:href="http://evil.example/x.svg#a"/></svg>"##,
            r##"<svg viewBox="0 0 1 1"><foreignObject><div/></foreignObject></svg>"##,
            r##"<svg viewBox="0 0 1 1"><a href="javascript:alert(1)"/></svg>"##,
            r##"<svg><path d="M0"/></svg>"##, // no viewBox
            r##"<html><svg viewBox="0 0 1 1"/></html>"##,
        ];
        for svg in cases {
            assert_eq!(icon_from_svg(dir.path(), svg), Err("Unsupported SVG".to_string()), "{svg}");
        }
        assert!(!icons_dir(dir.path()).exists() || std::fs::read_dir(icons_dir(dir.path())).unwrap().count() == 0);
    }

    #[test]
    fn an_internal_reference_is_fine_and_a_huge_svg_is_not() {
        assert!(validate_svg(r##"<svg viewBox="0 0 1 1"><use href="#a"/></svg>"##).is_ok());
        let big = format!("<svg viewBox=\"0 0 1 1\"><path d=\"{}\"/></svg>", "M0 0 ".repeat(3000));
        assert_eq!(validate_svg(&big), Err("Unsupported SVG".to_string()));
    }

    #[test]
    fn words_that_merely_contain_on_are_not_event_attributes() {
        assert!(validate_svg(r##"<svg viewBox="0 0 1 1"><path d="M0" opacity="0.2" stroke-linejoin="round"/></svg>"##).is_ok());
    }

    /// Not a test: `DECKA_MAKE_ICONS=<config dir> DECKA_PHOSPHOR=<assets dir> cargo test make_design_icons -- --ignored --nocapture`
    /// stores a few Phosphor icons through the real path and prints `name img:<hash>` (for design snapshots).
    #[test]
    #[ignore]
    fn make_design_icons() {
        let (Ok(dir), Ok(assets)) = (std::env::var("DECKA_MAKE_ICONS"), std::env::var("DECKA_PHOSPHOR")) else { return };
        for (name, weight) in [("house", "bold"), ("music-notes", "fill"), ("rocket-launch", "duotone"), ("gear", "regular"), ("lightning", "fill"), ("microphone", "light"), ("camera", "bold"), ("heart", "duotone")] {
            let file = if weight == "regular" { format!("{name}.svg") } else { format!("{name}-{weight}.svg") };
            let svg = std::fs::read_to_string(std::path::Path::new(&assets).join(weight).join(file)).expect("phosphor svg");
            println!("{name} {}", icon_from_svg(std::path::Path::new(&dir), &svg).expect("stored"));
        }
    }

    /// stores the icons of the programs in `DECKA_PROGRAMS` (`;`-separated paths) through the real path and prints `path img:<hash>`
    /// (for design snapshots of the `full` face).
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn make_program_icons() {
        let (Ok(dir), Ok(list)) = (std::env::var("DECKA_MAKE_ICONS"), std::env::var("DECKA_PROGRAMS")) else { return };
        for program in list.split(';').filter(|p| !p.is_empty()) {
            match icon_from_program(std::path::Path::new(&dir), program) {
                Ok(icon) => println!("{program} {icon}"),
                Err(e) => println!("{program} ERROR {e}"),
            }
        }
    }

    #[test]
    fn rule_11_accepts_an_svg_file() {
        let dir = tempfile::tempdir().unwrap();
        let icon = icon_from_svg(dir.path(), HEART).unwrap();
        assert!(missing_icon_issues(dir.path(), &layout_with(&icon, "lucide:play")).is_empty());
        let section = collect_icons(dir.path(), &layout_with(&icon, "lucide:play"));
        assert_eq!(section.as_object().unwrap().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn the_icon_of_notepad_is_a_256_px_png_and_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let notepad = r"C:\Windows\System32\notepad.exe";
        let icon = icon_from_program(dir.path(), notepad).expect("icon");
        let hash = img_hash(&icon).expect("img: + 16 hex");
        let file = icons_dir(dir.path()).join(format!("{hash}.png"));
        assert!(file.exists());
        let png = std::fs::read(&file).unwrap();
        // 256 px, or 192/128 when the 256 px PNG would not fit the data URL limit (DATA_MODEL.md 8.1).
        let size = crate::sta::run(move || crate::imaging::decoded_size(&png).unwrap()).unwrap();
        assert!([(256, 256), (192, 192), (128, 128)].contains(&size), "{size:?}");
        assert!(icon_data_url(dir.path(), hash).unwrap().len() <= MAX_DATA_URL_CHARS);
        assert_eq!(icon_from_program(dir.path(), notepad).unwrap(), icon);
        let pngs = std::fs::read_dir(icons_dir(dir.path())).unwrap().filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "png")).count();
        assert_eq!(pngs, 1);
        // The new icon is marked as made by the current processing version.
        assert!(is_current(&read_index(dir.path()), hash));
    }

    #[cfg(windows)]
    #[test]
    fn a_missing_program_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(icon_from_program(dir.path(), r"C:\no\such\program.exe").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn a_large_picture_becomes_a_256_px_icon() {
        let dir = tempfile::tempdir().unwrap();
        let pixels: Vec<u8> = std::iter::repeat_n([200u8, 100, 50, 255], 600 * 300).flatten().collect();
        let png = crate::sta::run(move || crate::imaging::encode_png(&pixels, 600, 300).unwrap()).unwrap();
        let file = dir.path().join("wide.png");
        std::fs::write(&file, &png).unwrap();
        let icon = icon_from_image(dir.path(), file.to_str().unwrap()).expect("icon");
        let saved = std::fs::read(icons_dir(dir.path()).join(format!("{}.png", img_hash(&icon).unwrap()))).unwrap();
        assert_eq!(crate::sta::run(move || crate::imaging::decoded_size(&saved).unwrap()).unwrap(), (256, 256));
        assert!(icon_from_image(dir.path(), dir.path().join("none.png").to_str().unwrap()).is_err());
    }

    /// A valid PNG header (IHDR) of the given size; the rest is not needed by `png_size`.
    fn fake_png(w: u32, h: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
        v.extend_from_slice(&13u32.to_be_bytes());
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v
    }

    #[test]
    fn png_size_reads_the_header() {
        assert_eq!(png_size(&fake_png(96, 96)), Some((96, 96)));
        assert_eq!(png_size(&fake_png(256, 128)), Some((256, 128)));
        assert_eq!(png_size(b"not a png at all, just bytes"), None);
        assert_eq!(png_size(b""), None);
    }

    #[test]
    fn a_png_that_does_not_fit_is_retried_smaller_down_to_128_then_refused() {
        let dir = tempfile::tempdir().unwrap();
        // Heavy at 256 and 192, light at 128: the third try is stored.
        let mut tried = Vec::new();
        let stored = save_first_that_fits(dir.path(), |side| {
            tried.push(side);
            let mut png = fake_png(side, side);
            if side > 128 {
                png.extend(std::iter::repeat_n(7u8, 100_000)); // base64 of it is > 131 072 characters
            }
            Ok(png)
        })
        .unwrap();
        assert_eq!(tried, [256, 192, 128]);
        let file = icons_dir(dir.path()).join(format!("{}.png", img_hash(&stored).unwrap()));
        assert_eq!(png_size(&std::fs::read(file).unwrap()), Some((128, 128)));

        let err = save_first_that_fits(dir.path(), |side| Ok([fake_png(side, side), vec![7u8; 120_000]].concat())).unwrap_err();
        assert_eq!(err, "Icon is too large");
    }

    #[test]
    fn a_small_png_in_the_limit_is_taken_at_256_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut tried = Vec::new();
        save_first_that_fits(dir.path(), |side| {
            tried.push(side);
            Ok(fake_png(side, side))
        })
        .unwrap();
        assert_eq!(tried, [256]);
    }

    /// BGRA picture `w` x `h`, transparent, with an opaque `size` x `size` square of `colour` in the middle.
    fn square_on_clear(w: u32, h: u32, size: u32, colour: [u8; 3]) -> Vec<u8> {
        let mut px = vec![0u8; (w * h * 4) as usize];
        let (x0, y0) = ((w - size) / 2, (h - size) / 2);
        for y in y0..y0 + size {
            for x in x0..x0 + size {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[colour[0], colour[1], colour[2], 255]);
            }
        }
        px
    }

    #[test]
    fn a_32_px_mark_in_a_clear_256_field_fills_at_least_85_percent_after_processing() {
        let px = square_on_clear(256, 256, 32, [200, 40, 40]);
        let (out, w, h) = refine_icon(px, 256, 256, false);
        assert_eq!((w, h, out.len()), (32, 32, 32 * 32 * 4));
        let (tw, th) = fit_into(w, h, inner_side(256));
        assert!(f64::from(tw) / 256.0 >= 0.85 && f64::from(th) / 256.0 >= 0.85, "{tw}x{th}");
        assert_eq!(inner_side(256), 226);
    }

    /// `square_on_clear` plus a one-colour frame of `thickness` px round the 32 px mark.
    fn framed(thickness: u32) -> Vec<u8> {
        let mut px = square_on_clear(256, 256, 48, [90, 90, 90]);
        let inner = 48 - 2 * thickness;
        let (x0, y0) = ((256 - inner) / 2, (256 - inner) / 2);
        for y in y0..y0 + inner {
            for x in x0..x0 + inner {
                let i = ((y * 256 + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[40, 160, 220, 255]);
            }
        }
        px
    }

    #[test]
    fn a_one_colour_shell_frame_is_cut_off_but_only_for_a_shell_image() {
        for thickness in [1u32, 2] {
            let (out, w, h) = refine_icon(framed(thickness), 256, 256, true);
            assert_eq!((w, h), (48 - 2 * thickness, 48 - 2 * thickness), "frame {thickness}");
            // Only the mark's own colour is left.
            assert!(out.as_chunks::<4>().0.iter().all(|p| p[..3] == [40, 160, 220]), "frame {thickness}");
            // A program's own icon keeps its frame.
            assert_eq!(refine_icon(framed(thickness), 256, 256, false).1, 48);
        }
    }

    #[test]
    fn a_picture_whose_edge_is_not_one_colour_keeps_its_edge() {
        let mut px = square_on_clear(64, 64, 40, [10, 10, 10]);
        // Make the top-left pixel of the square differ from the rest of the edge.
        let i = ((12 * 64 + 12) * 4) as usize;
        px[i..i + 4].copy_from_slice(&[200, 200, 200, 255]);
        assert_eq!(frame_width(&crop_bgra(&px, 64, content_bounds(&px, 64, 64).unwrap()).0, 40, 40), 0);
        // Nothing drawn: returned unchanged.
        assert_eq!(refine_icon(vec![0; 16 * 16 * 4], 16, 16, true).1, 16);
    }

    /// Layout with one program button (action `app`, icon `icon`) and one with an SVG icon.
    fn program_layout(icon: &str, svg_icon: &str) -> (LayoutFile, crate::model::ActionsFile) {
        let layout: LayoutFile = serde_json::from_str(&format!(
            r#"{{ "version": 1, "grid": {{ "cols": 5, "rows": 3 }},
              "display": {{ "dimAfterSec": 60, "dimLevel": 0.3, "sleepAfterSec": 600, "pixelShift": true, "night": {{ "enabled": false, "from": "23:00", "to": "07:00" }} }},
              "pages": [ {{ "id": "main", "name": "Main", "items": [
                {{ "id": "p", "kind": "button", "x": 0, "y": 0, "w": 1, "h": 1, "icon": "{icon}", "actionId": "app" }},
                {{ "id": "s", "kind": "button", "x": 1, "y": 0, "w": 1, "h": 1, "icon": "{svg_icon}", "actionId": "app" }},
                {{ "id": "l", "kind": "button", "x": 2, "y": 0, "w": 1, "h": 1, "icon": "{icon}", "actionId": "lock" }} ] }} ] }}"#
        ))
        .unwrap();
        let actions: crate::model::ActionsFile = serde_json::from_str(
            r#"{ "version": 1, "actions": { "app": { "name": "Notepad", "type": "launch", "program": "C:\\Windows\\System32\\notepad.exe" },
                                           "lock": { "name": "Lock", "type": "system", "op": "lock" } } }"#,
        )
        .unwrap();
        (layout, actions)
    }

    #[test]
    fn only_low_resolution_program_buttons_are_counted() {
        let dir = tempfile::tempdir().unwrap();
        let old = save_icon(dir.path(), &fake_png(96, 96)).unwrap();
        let sharp = save_icon(dir.path(), &fake_png(256, 256)).unwrap();
        mark_current(dir.path(), &sharp);
        let svg = icon_from_svg(dir.path(), HEART).unwrap();
        let (layout, actions) = program_layout(&old, &svg);
        // `p` (made before this processing version, launch) counts; `s` is SVG, `l` is a system action.
        assert_eq!(count_lowres_icons(dir.path(), &layout, &actions), 1);
        let (sharp_layout, _) = program_layout(&sharp, &svg);
        assert_eq!(count_lowres_icons(dir.path(), &sharp_layout, &actions), 0);
    }

    #[cfg(windows)]
    #[test]
    fn refreshing_replaces_the_96_px_icon_of_a_program_and_leaves_svg_alone() {
        let dir = tempfile::tempdir().unwrap();
        let notepad = r"C:\Windows\System32\notepad.exe".to_string();
        let old_png = crate::sta::run(move || win::program_png(&notepad, 96).unwrap()).unwrap();
        let old = save_icon(dir.path(), &old_png).unwrap();
        let svg = icon_from_svg(dir.path(), HEART).unwrap();
        let (mut layout, actions) = program_layout(&old, &svg);
        assert_eq!(count_lowres_icons(dir.path(), &layout, &actions), 1);

        let (updated, failed) = refresh_program_icons(dir.path(), &mut layout, &actions);
        assert_eq!((updated, failed), (1, 0));
        let LayoutItem::Button(p) = &layout.pages[0].items[0] else { panic!() };
        let LayoutItem::Button(s) = &layout.pages[0].items[1] else { panic!() };
        assert_ne!(p.icon.as_deref(), Some(old.as_str()));
        assert_eq!(s.icon.as_deref(), Some(svg.as_str()));
        let new_hash = img_hash(p.icon.as_deref().unwrap()).unwrap();
        let (w, h) = png_size(&std::fs::read(icons_dir(dir.path()).join(format!("{new_hash}.png"))).unwrap()).unwrap();
        assert!(w == h && w >= 128, "{w}x{h}");
        // The old file stays (another button may use it), and nothing is left to sharpen.
        assert!(icons_dir(dir.path()).join(format!("{}.png", img_hash(&old).unwrap())).exists());
        assert_eq!(count_lowres_icons(dir.path(), &layout, &actions), 0);
    }}
