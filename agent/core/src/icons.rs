//! Icon store (M12, DATA_MODEL.md 8): `<configDir>\icons\<hash16>.png`, 96x96 PNG with transparency.
//! Icons come from the PC itself (the Shell icon of an exe, or a picture file); nothing is downloaded or shipped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use base64::Engine;
use sha2::{Digest, Sha256};

use crate::model::{LayoutFile, LayoutItem, ValidationIssue};

/// Side of a stored icon in pixels.
pub const ICON_SIDE: u32 = 96;
/// A data URL longer than this is refused.
pub const MAX_DATA_URL_CHARS: usize = 32_768;
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

/// The data URL of a stored icon; missing or oversized files -> `None`.
pub fn icon_data_url(config_dir: &Path, hash: &str) -> Option<String> {
    if !is_hash(hash) {
        return None;
    }
    let png = std::fs::read(icons_dir(config_dir).join(format!("{hash}.png"))).ok()?;
    let url = data_url(&png);
    (url.len() <= MAX_DATA_URL_CHARS).then_some(url)
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
    let missing = |icon: &Option<String>| icon.as_deref().and_then(img_hash).is_some_and(|h| !dir.join(format!("{h}.png")).exists());
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

/// Icon of a program (`.exe`) from the Shell, stored as `img:<hash16>`.
pub fn icon_from_program(config_dir: &Path, path: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        let (dir, path) = (config_dir.to_path_buf(), path.to_string());
        crate::sta::run(move || win::program_png(&path).and_then(|png| save_icon(&dir, &png)))?
    }
    #[cfg(not(windows))]
    {
        let _ = (config_dir, path);
        Err("Program icons are available on Windows only".to_string())
    }
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
        crate::sta::run(move || win::picture_png(&bytes).and_then(|png| save_icon(&dir, &png)))?
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
    use windows::Win32::Graphics::Gdi::{DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ};
    use windows::Win32::System::Com::IBindCtx;
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY};

    use super::{blit_centered, fit_into, ICON_SIDE};
    use crate::imaging;

    fn err(what: &str, e: windows::core::Error) -> String {
        format!("{what}: {}", e.message())
    }

    /// BGRA (premultiplied) pixels, width and height of the Shell icon of `path` at `size` px.
    pub fn shell_icon_bgra(path: &str, size: i32) -> Result<(Vec<u8>, u32, u32), String> {
        let wide = HSTRING::from(path);
        // SAFETY: plain Shell/GDI calls; every handle created here is released before returning.
        unsafe {
            let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&wide, None::<&IBindCtx>).map_err(|e| err("cannot open the file", e))?;
            let hbitmap = factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY).map_err(|e| err("the file has no icon", e))?;
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
            // Icons without an alpha channel come back fully transparent: make them opaque.
            if pixels.chunks_exact(4).all(|p| p[3] == 0) {
                pixels.chunks_exact_mut(4).for_each(|p| p[3] = 255);
            }
            Ok((pixels, w as u32, h as u32))
        }
    }

    /// 96x96 PNG of the Shell icon of a program.
    pub fn program_png(path: &str) -> Result<Vec<u8>, String> {
        if !std::path::Path::new(path).is_file() {
            return Err("The program was not found".to_string());
        }
        let (pixels, w, h) = shell_icon_bgra(path, ICON_SIDE as i32)?;
        compose(&pixels, w, h)
    }

    /// 96x96 PNG of a picture file's content.
    pub fn picture_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
        let (w, h) = imaging::decoded_size(bytes).map_err(|_| "This file is not a picture Windows can read".to_string())?;
        fit_and_center(bytes, w, h)
    }

    /// BGRA pixels of any size -> 96x96 PNG.
    fn compose(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
        if w <= ICON_SIDE && h <= ICON_SIDE && (w == ICON_SIDE || h == ICON_SIDE) {
            return imaging::encode_png(&blit_centered(pixels, w, h, ICON_SIDE), ICON_SIDE, ICON_SIDE).map_err(|e| err("cannot encode the icon", e));
        }
        let png = imaging::encode_png(pixels, w, h).map_err(|e| err("cannot encode the icon", e))?;
        fit_and_center(&png, w, h)
    }

    /// Scale the encoded picture to fit 96x96 (keeping proportions) and centre it on a transparent canvas.
    fn fit_and_center(encoded: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
        let (tw, th) = fit_into(w, h, ICON_SIDE);
        let pixels = imaging::scaled_pixels(encoded, tw, th, BitmapAlphaMode::Premultiplied).map_err(|e| err("cannot scale the picture", e))?;
        imaging::encode_png(&blit_centered(&pixels, tw, th, ICON_SIDE), ICON_SIDE, ICON_SIDE).map_err(|e| err("cannot encode the icon", e))
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
        assert_eq!(save_icon(dir.path(), &vec![7u8; 30_000]), Err("Icon is too large".to_string()));
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

    #[cfg(windows)]
    #[test]
    fn the_icon_of_notepad_is_a_96_px_png_and_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let notepad = r"C:\Windows\System32\notepad.exe";
        let icon = icon_from_program(dir.path(), notepad).expect("icon");
        let hash = img_hash(&icon).expect("img: + 16 hex");
        let file = icons_dir(dir.path()).join(format!("{hash}.png"));
        assert!(file.exists());
        let png = std::fs::read(&file).unwrap();
        assert_eq!(crate::sta::run(move || crate::imaging::decoded_size(&png).unwrap()).unwrap(), (96, 96));
        assert!(icon_data_url(dir.path(), hash).unwrap().len() <= MAX_DATA_URL_CHARS);
        assert_eq!(icon_from_program(dir.path(), notepad).unwrap(), icon);
        assert_eq!(std::fs::read_dir(icons_dir(dir.path())).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn a_missing_program_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(icon_from_program(dir.path(), r"C:\no\such\program.exe").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn a_large_picture_becomes_a_96_px_icon() {
        let dir = tempfile::tempdir().unwrap();
        let pixels: Vec<u8> = std::iter::repeat([200u8, 100, 50, 255]).take(600 * 300).flatten().collect();
        let png = crate::sta::run(move || crate::imaging::encode_png(&pixels, 600, 300).unwrap()).unwrap();
        let file = dir.path().join("wide.png");
        std::fs::write(&file, &png).unwrap();
        let icon = icon_from_image(dir.path(), file.to_str().unwrap()).expect("icon");
        let saved = std::fs::read(icons_dir(dir.path()).join(format!("{}.png", img_hash(&icon).unwrap()))).unwrap();
        assert_eq!(crate::sta::run(move || crate::imaging::decoded_size(&saved).unwrap()).unwrap(), (96, 96));
        assert!(icon_from_image(dir.path(), dir.path().join("none.png").to_str().unwrap()).is_err());
    }
}
