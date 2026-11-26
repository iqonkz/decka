//! T29.7: a screenshot of the monitor, of the active window or of an area chosen with the mouse. It goes to the clipboard
//! and to `%TEMP%\Decka\shots\<date-time>.png`; files older than 7 days are removed. Decka's own overlays (island, orb,
//! highlight) are left out of the picture. The decisions are pure; `win` is the thin Windows layer.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::model::ShotArea;

pub const KEEP: Duration = Duration::from_secs(7 * 24 * 3600);
/// A smaller selection is a click, not an area.
const MIN_SIDE: i32 = 4;

/// BGRA pixels, rows from the top.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Left, top, width, height.
pub type Area = (i32, i32, i32, i32);

pub fn shots_dir() -> PathBuf {
    std::env::temp_dir().join("Decka").join("shots")
}

/// `2026-11-27_01-40-05.png`, with `-2`, `-3` when that second has a picture already.
pub fn file_name(time: [u16; 6], taken: impl Fn(&str) -> bool) -> String {
    let [y, mo, d, h, mi, s] = time;
    let stem = format!("{y:04}-{mo:02}-{d:02}_{h:02}-{mi:02}-{s:02}");
    let first = format!("{stem}.png");
    if !taken(&first) {
        return first;
    }
    (2..).map(|n| format!("{stem}-{n}.png")).find(|n| !taken(n)).expect("an unused name exists")
}

/// A picture of ours that is older than `KEEP`. Only names this module writes are ever removed.
pub fn expired(name: &str, modified: SystemTime, now: SystemTime) -> bool {
    let ours = name.len() >= 23 && name.ends_with(".png") && name.bytes().take(19).enumerate().all(|(i, b)| match i {
        4 | 7 | 13 | 16 => b == b'-',
        10 => b == b'_',
        _ => b.is_ascii_digit(),
    });
    ours && now.duration_since(modified).is_ok_and(|age| age > KEEP)
}

pub fn clean(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // `symlink_metadata`: a link is not followed.
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else { continue };
        if meta.is_file() && meta.modified().is_ok_and(|m| expired(&name, m, now)) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The rectangle between two corners, cut to a `width` x `height` picture; `None` when it is too small to be meant.
pub fn selection(a: (i32, i32), b: (i32, i32), width: i32, height: i32) -> Option<Area> {
    let (x0, x1) = (a.0.min(b.0).clamp(0, width), a.0.max(b.0).clamp(0, width));
    let (y0, y1) = (a.1.min(b.1).clamp(0, height), a.1.max(b.1).clamp(0, height));
    (x1 - x0 >= MIN_SIDE && y1 - y0 >= MIN_SIDE).then_some((x0, y0, x1 - x0, y1 - y0))
}

/// The part of a picture inside `area` (cut to the picture).
pub fn crop(p: &Picture, area: Area) -> Picture {
    let (w, h) = (p.width as i32, p.height as i32);
    let x = area.0.clamp(0, w);
    let y = area.1.clamp(0, h);
    let cw = area.2.min(w - x).max(0) as usize;
    let ch = area.3.min(h - y).max(0) as usize;
    let mut bgra = Vec::with_capacity(cw * ch * 4);
    for row in y as usize..y as usize + ch {
        let from = (row * w as usize + x as usize) * 4;
        bgra.extend_from_slice(&p.bgra[from..from + cw * 4]);
    }
    Picture { bgra, width: cw as u32, height: ch as u32 }
}

/// The picture as `CF_DIB`: a `BITMAPINFOHEADER` and the rows from the bottom, as every program reads it.
pub fn dib(p: &Picture) -> Vec<u8> {
    let row = p.width as usize * 4;
    let mut out = Vec::with_capacity(40 + p.bgra.len());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(p.width as i32).to_le_bytes());
    out.extend_from_slice(&(p.height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(p.bgra.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]); // resolution and palette: not set
    if row > 0 {
        for line in p.bgra.chunks_exact(row).rev() {
            out.extend_from_slice(line);
        }
    }
    out
}

/// Takes the picture and delivers it. An area is chosen by the user, which takes as long as it takes: it runs on a thread of
/// its own and this call returns at once.
pub fn take(area: ShotArea) -> Result<(), String> {
    #[cfg(windows)]
    {
        match area {
            ShotArea::Screen => win::deliver(&win::capture(win::Of::Monitor)?),
            ShotArea::Window => win::deliver(&win::capture(win::Of::Window)?),
            ShotArea::Region => win::choose_region(),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = area;
        Err("unsupported platform".into())
    }
}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant, SystemTime};

    use windows::core::w;
    use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EndPaint, FrameRect, GetDC, GetMonitorInfoW, GetStockObject,
        InvalidateRect, MonitorFromWindow, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, CAPTUREBLT, DIB_RGB_COLORS, HBITMAP, HBRUSH, HDC, HGDIOBJ,
        MONITORINFO, MONITOR_DEFAULTTOPRIMARY, PAINTSTRUCT, SRCCOPY, WHITE_BRUSH,
    };
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::SystemInformation::GetLocalTime;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture, SetCapture, VK_ESCAPE};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowRect, KillTimer, LoadCursorW,
        PostQuitMessage, RegisterClassW, SetForegroundWindow, SetTimer, ShowWindow, TranslateMessage, IDC_CROSS, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN, WM_TIMER,
        WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    use super::{crop, dib, file_name, selection, Area, Picture};

    /// `CF_DIB`.
    const CF_DIB: u32 = 8;
    /// The chooser gives up after this long.
    const CHOOSE_FOR: Duration = Duration::from_secs(60);

    /// Decka's overlays are not in the picture; they come back when this is dropped.
    struct Hidden;

    impl Hidden {
        fn new() -> Hidden {
            if crate::overlay::hide_from_capture(true) {
                // The compositor needs a frame to leave them out.
                std::thread::sleep(Duration::from_millis(60));
            }
            Hidden
        }
    }

    impl Drop for Hidden {
        fn drop(&mut self) {
            crate::overlay::hide_from_capture(false);
        }
    }

    pub enum Of {
        Monitor,
        Window,
    }

    /// The pixels of a rectangle of the desktop.
    fn grab(rect: RECT) -> Result<Picture, String> {
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        if !(1..=32_000).contains(&w) || !(1..=32_000).contains(&h) {
            return Err("The screen could not be read".into());
        }
        // SAFETY: plain GDI calls; every object made here is released before returning, and the bits of the section are copied
        // while it exists.
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let section = section(screen, w, h);
            let result = section.and_then(|(bitmap, bits)| {
                let old = SelectObject(mem, HGDIOBJ(bitmap.0));
                let copied = BitBlt(mem, 0, 0, w, h, Some(screen), rect.left, rect.top, SRCCOPY | CAPTUREBLT).map_err(|e| e.to_string());
                let mut bgra = std::slice::from_raw_parts(bits, w as usize * h as usize * 4).to_vec();
                SelectObject(mem, old);
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                for p in bgra.as_chunks_mut::<4>().0 {
                    p[3] = 255; // GDI leaves the alpha at 0
                }
                copied.map(|_| Picture { bgra, width: w as u32, height: h as u32 })
            });
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            result
        }
    }

    /// A 32-bit top-down bitmap whose pixels can be reached.
    unsafe fn section(dc: HDC, w: i32, h: i32) -> Result<(HBITMAP, *mut u8), String> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, ..Default::default() },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        // SAFETY: `info` and `bits` live for the call.
        let bitmap = unsafe { CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) }.map_err(|e| e.to_string())?;
        if bits.is_null() {
            return Err("The screen could not be read".into());
        }
        Ok((bitmap, bits.cast()))
    }

    pub fn capture(of: Of) -> Result<Picture, String> {
        let _hidden = Hidden::new();
        // SAFETY: plain queries about the foreground window and its monitor; the structures live for the calls.
        unsafe {
            let window = GetForegroundWindow();
            match of {
                Of::Monitor => {
                    let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTOPRIMARY);
                    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                    if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                        return Err("The screen could not be read".into());
                    }
                    grab(info.rcMonitor)
                }
                Of::Window => {
                    if window.0.is_null() {
                        return Err("No window to look at".into());
                    }
                    let (bgra, width, height) = crate::voice::screen::capture_active_window()?;
                    let picture = Picture { bgra, width, height };
                    // The picture of a window has an invisible border around it: cut to what is seen.
                    let (mut outer, mut seen) = (RECT::default(), RECT::default());
                    let known = GetWindowRect(window, &mut outer).is_ok()
                        && DwmGetWindowAttribute(window, DWMWA_EXTENDED_FRAME_BOUNDS, (&mut seen as *mut RECT).cast(), std::mem::size_of::<RECT>() as u32).is_ok();
                    Ok(if known { crop(&picture, (seen.left - outer.left, seen.top - outer.top, seen.right - seen.left, seen.bottom - seen.top)) } else { picture })
                }
            }
        }
    }

    fn to_clipboard(data: &[u8]) -> Result<(), String> {
        // SAFETY: the clipboard is opened and closed here; the block is filled before the system takes it.
        unsafe {
            let mut opened = false;
            for _ in 0..10 {
                if OpenClipboard(None).is_ok() {
                    opened = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            if !opened {
                return Err("The clipboard is busy".into());
            }
            let result = (|| {
                let block = GlobalAlloc(GMEM_MOVEABLE, data.len()).map_err(|e| e.to_string())?;
                let p = GlobalLock(block) as *mut u8;
                if p.is_null() {
                    return Err("The clipboard is unavailable".to_string());
                }
                std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                let _ = GlobalUnlock(block);
                EmptyClipboard().map_err(|e| e.to_string())?;
                SetClipboardData(CF_DIB, Some(HANDLE(block.0))).map(|_| ()).map_err(|e| e.to_string())
            })();
            let _ = CloseClipboard();
            result
        }
    }

    /// Clipboard first (it is what the user pastes), then the file.
    pub fn deliver(picture: &Picture) -> Result<(), String> {
        to_clipboard(&dib(picture))?;
        let dir = super::shots_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot save the screenshot: {e}"))?;
        super::clean(&dir, SystemTime::now());
        let (bgra, w, h) = (picture.bgra.clone(), picture.width, picture.height);
        let png = crate::sta::run(move || crate::imaging::encode_png(&bgra, w, h).map_err(|e| e.to_string()))??;
        // SAFETY: a plain query of the clock.
        let t = unsafe { GetLocalTime() };
        let name = file_name([t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond], |n| dir.join(n).exists());
        std::fs::write(dir.join(name), png).map_err(|e| format!("Cannot save the screenshot: {e}"))
    }

    // ---- Choosing an area -------------------------------------------------------------------------------------------

    struct Chooser {
        /// The screen as it was, and darkened; both in memory DCs.
        bright: HDC,
        dim: HDC,
        back: HDC,
        width: i32,
        height: i32,
        from: Option<(i32, i32)>,
        to: (i32, i32),
        chosen: Option<Area>,
        since: Instant,
    }

    thread_local! {
        static CHOOSER: RefCell<Option<Chooser>> = const { RefCell::new(None) };
    }

    fn point(lparam: LPARAM) -> (i32, i32) {
        ((lparam.0 & 0xFFFF) as u16 as i16 as i32, ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32)
    }

    unsafe extern "system" fn chooser_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // SAFETY (all calls below): plain window and GDI calls on the thread that owns the window; the state is this thread's.
        let finish = || unsafe {
            let _ = DestroyWindow(window);
        };
        match message {
            WM_LBUTTONDOWN => {
                CHOOSER.with_borrow_mut(|c| {
                    if let Some(c) = c {
                        c.from = Some(point(lparam));
                        c.to = point(lparam);
                    }
                });
                unsafe { SetCapture(window) };
            }
            WM_MOUSEMOVE => {
                let moved = CHOOSER.with_borrow_mut(|c| c.as_mut().is_some_and(|c| {
                    c.to = point(lparam);
                    c.from.is_some()
                }));
                if moved {
                    unsafe {
                        let _ = InvalidateRect(Some(window), None, false);
                    }
                }
            }
            WM_LBUTTONUP => {
                unsafe {
                    let _ = ReleaseCapture();
                }
                let done = CHOOSER.with_borrow_mut(|c| {
                    c.as_mut().is_some_and(|c| {
                        c.chosen = c.from.take().and_then(|from| selection(from, point(lparam), c.width, c.height));
                        c.chosen.is_some()
                    })
                });
                if done {
                    finish();
                }
            }
            WM_RBUTTONDOWN => finish(),
            WM_KEYDOWN if wparam.0 == usize::from(VK_ESCAPE.0) => finish(),
            WM_TIMER => {
                // Esc also without the keyboard focus (Windows may refuse it to a program in the background).
                let escape = unsafe { GetAsyncKeyState(i32::from(VK_ESCAPE.0)) } as u16 & 0x8000 != 0;
                let late = CHOOSER.with_borrow(|c| c.as_ref().is_some_and(|c| c.since.elapsed() > CHOOSE_FOR));
                if escape || late {
                    finish();
                }
            }
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = unsafe { BeginPaint(window, &mut paint) };
                CHOOSER.with_borrow(|c| {
                    let Some(c) = c else { return };
                    unsafe {
                        let _ = BitBlt(c.back, 0, 0, c.width, c.height, Some(c.dim), 0, 0, SRCCOPY);
                        if let Some((x, y, w, h)) = c.from.and_then(|from| selection(from, c.to, c.width, c.height)) {
                            let _ = BitBlt(c.back, x, y, w, h, Some(c.bright), x, y, SRCCOPY);
                            let frame = RECT { left: x - 1, top: y - 1, right: x + w + 1, bottom: y + h + 1 };
                            FrameRect(c.back, &frame, HBRUSH(GetStockObject(WHITE_BRUSH).0));
                        }
                        let _ = BitBlt(dc, 0, 0, c.width, c.height, Some(c.back), 0, 0, SRCCOPY);
                    }
                });
                unsafe {
                    let _ = EndPaint(window, &paint);
                }
            }
            WM_DESTROY => unsafe { PostQuitMessage(0) },
            _ => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
        LRESULT(0)
    }

    /// Shows the darkened screen and waits for the area. `Ok(None)`: cancelled.
    fn choose(shot: &Picture, left: i32, top: i32) -> Result<Option<Area>, String> {
        let (w, h) = (shot.width as i32, shot.height as i32);
        // SAFETY: a window and three bitmaps made, used and released on this thread.
        unsafe {
            let screen = GetDC(None);
            let mut made: Vec<(HDC, HBITMAP, HGDIOBJ)> = Vec::new();
            for dimmed in [false, true, false] {
                let (bitmap, bits) = section(screen, w, h)?;
                let pixels = std::slice::from_raw_parts_mut(bits, shot.bgra.len());
                pixels.copy_from_slice(&shot.bgra);
                if dimmed {
                    for p in pixels.as_chunks_mut::<4>().0 {
                        for c in &mut p[..3] {
                            *c = (u16::from(*c) * 45 / 100) as u8;
                        }
                    }
                }
                let dc = CreateCompatibleDC(Some(screen));
                let old = SelectObject(dc, HGDIOBJ(bitmap.0));
                made.push((dc, bitmap, old));
            }
            ReleaseDC(None, screen);
            CHOOSER.set(Some(Chooser { bright: made[0].0, dim: made[1].0, back: made[2].0, width: w, height: h, from: None, to: (0, 0), chosen: None, since: Instant::now() }));

            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = w!("DeckaShotArea");
            let definition = WNDCLASSW {
                lpfnWndProc: Some(chooser_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_CROSS).map_err(|e| e.to_string())?,
                lpszClassName: class,
                ..Default::default()
            };
            // A second registration fails harmlessly: the class is there.
            RegisterClassW(&definition);
            let window = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW, class, w!("Decka screenshot"), WS_POPUP, left, top, w, h, None, None, Some(instance.into()), None)
                .map_err(|e| e.to_string());
            let result = window.map(|window| {
                let _ = ShowWindow(window, SW_SHOW);
                let _ = SetForegroundWindow(window);
                let timer = SetTimer(Some(window), 1, 30, None);
                let mut message = MSG::default();
                while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                let _ = KillTimer(Some(window), timer);
                CHOOSER.with_borrow(|c| c.as_ref().and_then(|c| c.chosen))
            });
            CHOOSER.set(None);
            for (dc, bitmap, old) in made {
                SelectObject(dc, old);
                let _ = DeleteDC(dc);
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            result
        }
    }

    static CHOOSING: AtomicBool = AtomicBool::new(false);

    pub fn choose_region() -> Result<(), String> {
        if CHOOSING.swap(true, Ordering::SeqCst) {
            return Ok(()); // one chooser is on the screen already
        }
        let spawned = std::thread::Builder::new().name("decka-shot-area".into()).spawn(|| {
            let run = || -> Result<(), String> {
                // SAFETY: plain queries.
                let (left, top, w, h) = unsafe {
                    (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN), GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN))
                };
                let (shot, previous) = {
                    let _hidden = Hidden::new();
                    // SAFETY: a plain query.
                    (grab(RECT { left, top, right: left + w, bottom: top + h })?, unsafe { GetForegroundWindow() })
                };
                let chosen = choose(&shot, left, top)?;
                // SAFETY: gives the focus back to where the user was.
                unsafe {
                    let _ = SetForegroundWindow(previous);
                }
                match chosen {
                    Some(area) => deliver(&crop(&shot, area)),
                    None => Ok(()),
                }
            };
            if let Err(e) = run() {
                tracing::warn!("screenshot of an area: {e}");
            }
            CHOOSING.store(false, Ordering::SeqCst);
        });
        spawned.map(|_| ()).map_err(|e| {
            CHOOSING.store(false, Ordering::SeqCst);
            e.to_string()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(w: u32, h: u32) -> Picture {
        Picture { bgra: (0..w * h).flat_map(|i| [i as u8, 0, 0, 255]).collect(), width: w, height: h }
    }

    #[test]
    fn names_carry_the_time_and_never_overwrite() {
        let t = [2026, 11, 27, 1, 4, 5];
        assert_eq!(file_name(t, |_| false), "2026-11-27_01-04-05.png");
        assert_eq!(file_name(t, |n| n == "2026-11-27_01-04-05.png"), "2026-11-27_01-04-05-2.png");
    }

    #[test]
    fn only_our_pictures_older_than_a_week_are_removed() {
        let now = SystemTime::UNIX_EPOCH + KEEP * 3;
        let old = now - KEEP - Duration::from_secs(1);
        assert!(expired("2026-11-27_01-04-05.png", old, now));
        assert!(expired("2026-11-27_01-04-05-2.png", old, now));
        assert!(!expired("2026-11-27_01-04-05.png", now - KEEP + Duration::from_secs(60), now));
        for other in ["holiday.png", "2026-11-27_01-04-05.jpg", "2026-11-27 01-04-05.png", "x.png"] {
            assert!(!expired(other, old, now), "{other}");
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("2020-01-01_00-00-00.png"), b"x").unwrap();
        std::fs::write(dir.path().join("mine.txt"), b"x").unwrap();
        clean(dir.path(), SystemTime::now() + KEEP + Duration::from_secs(60));
        assert!(!dir.path().join("2020-01-01_00-00-00.png").exists() && dir.path().join("mine.txt").exists());
    }

    #[test]
    fn a_selection_in_any_direction_is_a_rectangle_inside_the_picture() {
        assert_eq!(selection((10, 20), (50, 60), 100, 100), Some((10, 20, 40, 40)));
        assert_eq!(selection((50, 60), (10, 20), 100, 100), Some((10, 20, 40, 40)));
        assert_eq!(selection((-20, -5), (200, 30), 100, 100), Some((0, 0, 100, 30)));
        assert_eq!(selection((10, 10), (12, 40), 100, 100), None, "a click is not an area");
    }

    #[test]
    fn a_crop_takes_exactly_those_pixels() {
        let p = picture(4, 3);
        let c = crop(&p, (1, 1, 2, 2));
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(c.bgra.chunks(4).map(|px| px[0]).collect::<Vec<_>>(), [5, 6, 9, 10]);
        assert_eq!(crop(&p, (3, 2, 10, 10)).bgra.len(), 4, "cut to the picture");
    }

    #[test]
    fn the_clipboard_bitmap_is_a_header_and_rows_from_the_bottom() {
        let d = dib(&picture(2, 2));
        assert_eq!(d.len(), 40 + 16);
        assert_eq!((d[0], i32::from_le_bytes(d[4..8].try_into().unwrap()), i32::from_le_bytes(d[8..12].try_into().unwrap()), d[14]), (40, 2, 2, 32));
        assert_eq!((d[40], d[44], d[48], d[52]), (2, 3, 0, 1));
    }

    /// By hand: `cargo test -p decka-core --lib take_a_screenshot -- --ignored` (writes into %TEMP%\Decka\shots).
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn take_a_screenshot() {
        take(ShotArea::Window).unwrap();
        take(ShotArea::Screen).unwrap();
    }
}
