//! T29.11: the light or dark theme of Windows (not of Decka): the two values of `...\Themes\Personalize` and the message
//! that tells the running programs to look again. The decisions are pure over `ThemeStore`; `SystemTheme` is the registry.

/// `AppsUseLightTheme` (the windows of programs) and `SystemUsesLightTheme` (the taskbar and Start).
pub const VALUES: [&str; 2] = ["AppsUseLightTheme", "SystemUsesLightTheme"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    Light,
    Dark,
    Toggle,
}

pub trait ThemeStore {
    /// Whether programs use the light theme now; `None` when Windows has not written it (it is light then).
    fn apps_light(&self) -> Option<bool>;
    fn write(&self, value: &str, light: bool) -> Result<(), String>;
    /// Tells the running programs that the colours changed.
    fn announce(&self);
}

/// Switches the theme; `true`: it is light now.
pub fn set(store: &dyn ThemeStore, wanted: Wanted) -> Result<bool, String> {
    let light = match wanted {
        Wanted::Light => true,
        Wanted::Dark => false,
        Wanted::Toggle => !store.apps_light().unwrap_or(true),
    };
    for value in VALUES {
        store.write(value, light)?;
    }
    store.announce();
    Ok(light)
}

#[cfg(windows)]
mod win {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Registry::{RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD};
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};

    const KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

    pub struct SystemTheme;

    impl super::ThemeStore for SystemTheme {
        fn apps_light(&self) -> Option<bool> {
            let mut value = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            // SAFETY: `value` and `size` live for the call and match each other.
            let read = unsafe { RegGetValueW(HKEY_CURRENT_USER, KEY, w!("AppsUseLightTheme"), RRF_RT_REG_DWORD, None, Some((&mut value as *mut u32).cast()), Some(&mut size)) };
            read.is_ok().then_some(value != 0)
        }

        fn write(&self, name: &str, light: bool) -> Result<(), String> {
            let value = u32::from(light);
            let name = HSTRING::from(name);
            // SAFETY: the data is one DWORD that lives for the call; the key is of the current user.
            let written = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, KEY, &name, REG_DWORD.0, Some((&value as *const u32).cast()), std::mem::size_of::<u32>() as u32) };
            written.ok().map_err(|e| format!("Windows did not accept the theme: {e}"))
        }

        /// One window is asked after another, which can take half a minute with many slow ones: nothing waits for it.
        fn announce(&self) {
            let sent = std::thread::Builder::new().name("decka-theme".into()).spawn(|| {
                let what: Vec<u16> = "ImmersiveColorSet\0".encode_utf16().collect();
                // SAFETY: a broadcast with a string that lives for the call; a window that hangs is skipped.
                let _ = unsafe { SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(what.as_ptr() as isize), SMTO_ABORTIFHUNG, 200, None) };
            });
            if let Err(e) = sent {
                tracing::warn!("theme announcement: {e}");
            }
        }
    }
}

#[cfg(windows)]
pub use win::SystemTheme;

/// Switches the theme of this PC. Blocking (programs are told one after another).
pub fn apply(wanted: Wanted) -> Result<bool, String> {
    #[cfg(windows)]
    {
        set(&SystemTheme, wanted)
    }
    #[cfg(not(windows))]
    {
        let _ = wanted;
        Err("unsupported platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        now: Option<bool>,
        log: RefCell<Vec<String>>,
        refuse: bool,
    }

    impl ThemeStore for Fake {
        fn apps_light(&self) -> Option<bool> {
            self.now
        }
        fn write(&self, value: &str, light: bool) -> Result<(), String> {
            if self.refuse {
                return Err("refused".into());
            }
            self.log.borrow_mut().push(format!("{value}={}", u8::from(light)));
            Ok(())
        }
        fn announce(&self) {
            self.log.borrow_mut().push("announce".into());
        }
    }

    #[test]
    fn both_values_are_written_and_then_announced() {
        let store = Fake::default();
        assert_eq!(set(&store, Wanted::Dark), Ok(false));
        assert_eq!(*store.log.borrow(), ["AppsUseLightTheme=0", "SystemUsesLightTheme=0", "announce"]);
        let store = Fake::default();
        assert_eq!(set(&store, Wanted::Light), Ok(true));
        assert_eq!(*store.log.borrow(), ["AppsUseLightTheme=1", "SystemUsesLightTheme=1", "announce"]);
    }

    #[test]
    fn toggle_goes_to_the_other_one_and_an_unknown_state_is_light() {
        let dark = Fake { now: Some(false), ..Default::default() };
        assert_eq!(set(&dark, Wanted::Toggle), Ok(true));
        assert_eq!(set(&Fake { now: Some(true), ..Default::default() }, Wanted::Toggle), Ok(false));
        assert_eq!(set(&Fake::default(), Wanted::Toggle), Ok(false));
    }

    #[test]
    fn a_refused_write_is_an_error_and_nothing_is_announced() {
        let store = Fake { refuse: true, ..Default::default() };
        assert!(set(&store, Wanted::Dark).is_err());
        assert!(store.log.borrow().is_empty());
    }

    /// By hand, reads only: `cargo test -p decka-core --lib what_theme_is_it -- --ignored --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn what_theme_is_it() {
        println!("apps use the light theme: {:?}", SystemTheme.apps_light());
    }
}
