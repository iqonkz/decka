//! The acrylic glass of the island window (T26.F.3): the system backdrop of the window (`DWMSBT_TRANSIENTWINDOW`) cut to the
//! rounded panel with a window region. Windows 11 22H2 or later; elsewhere (and on any failure) the call says `false` and the
//! page keeps its own translucent background.

/// Switches the acrylic backdrop of the window on or off. `true` when Windows accepted it.
pub fn set_acrylic(hwnd: isize, on: bool) -> bool {
    #[cfg(windows)]
    {
        win::set_acrylic(hwnd, on)
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, on);
        false
    }
}

/// Cuts the window to a rounded rectangle (physical pixels from the top-left corner of the window); `None` removes the cut.
pub fn set_region(hwnd: isize, rect: Option<(i32, i32, i32, i32)>, radius: i32) {
    #[cfg(windows)]
    win::set_region(hwnd, rect, radius);
    #[cfg(not(windows))]
    let _ = (hwnd, rect, radius);
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE};
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    use windows::Win32::UI::Controls::MARGINS;

    // DWMSBT_NONE = 1, DWMSBT_TRANSIENTWINDOW = 3 (acrylic).
    pub fn set_acrylic(hwnd: isize, on: bool) -> bool {
        let hwnd = HWND(hwnd as *mut _);
        let kind = DWM_SYSTEMBACKDROP_TYPE(if on { 3 } else { 1 });
        let margins = MARGINS { cxLeftWidth: if on { -1 } else { 0 }, cxRightWidth: if on { -1 } else { 0 }, cyTopHeight: if on { -1 } else { 0 }, cyBottomHeight: if on { -1 } else { 0 } };
        // SAFETY: plain DWM calls on a window of this process; the pointers live for the call.
        unsafe {
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
            DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &kind as *const _ as *const _, std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32).is_ok()
        }
    }

    pub fn set_region(hwnd: isize, rect: Option<(i32, i32, i32, i32)>, radius: i32) {
        let hwnd = HWND(hwnd as *mut _);
        // SAFETY: the region is handed to the window, which owns it from then on.
        unsafe {
            match rect {
                Some((x, y, w, h)) => {
                    let rgn = CreateRoundRectRgn(x, y, x + w + 1, y + h + 1, radius * 2, radius * 2);
                    SetWindowRgn(hwnd, Some(rgn), true);
                }
                None => {
                    SetWindowRgn(hwnd, None, true);
                }
            }
        }
    }
}
