//! The global hotkey of voice control (DATA_MODEL.md 12.6): `RegisterHotKey` from the chord of `VoiceStatus.hotkey`.
//! Which chord to register and when (`parse_chord`, `Tracker`, `aux_keys`) is pure; the thin layer at the bottom owns the
//! thread with the Windows message loop.

pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;

pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_RETURN: u32 = 0x0D;

/// A chord as `RegisterHotKey` takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub mods: u32,
    pub vk: u32,
}

fn named_vk(key: &str) -> Option<u32> {
    Some(match key {
        "space" => 0x20,
        "enter" => VK_RETURN,
        "tab" => 0x09,
        "backspace" => 0x08,
        "delete" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        _ => return None,
    })
}

/// The virtual-key code of a main key: a letter, a digit, `f1`..`f24` or a named key. `esc` is not a main key: it cancels.
fn main_vk(key: &str) -> Option<u32> {
    if let Some(vk) = named_vk(key) {
        return Some(vk);
    }
    let b = key.as_bytes();
    match b {
        [c @ b'a'..=b'z'] => Some(c.to_ascii_uppercase() as u32),
        [c @ b'0'..=b'9'] => Some(*c as u32),
        [b'f', rest @ ..] => key[1..].parse::<u32>().ok().filter(|n| (1..=24).contains(n) && !rest.is_empty()).map(|n| 0x6F + n),
        _ => None,
    }
}

/// `["ctrl","alt","space"]` -> modifiers + key. One main key is needed; modifiers are needed too, except for `f1`..`f24`.
pub fn parse_chord(keys: &[String]) -> Result<Chord, String> {
    let (mut mods, mut main) = (0u32, None);
    for k in keys {
        let k = k.to_ascii_lowercase();
        let flag = match k.as_str() {
            "ctrl" | "control" => MOD_CONTROL,
            "alt" => MOD_ALT,
            "shift" => MOD_SHIFT,
            "win" | "meta" | "super" => MOD_WIN,
            _ => 0,
        };
        if flag != 0 {
            mods |= flag;
        } else if main.replace(main_vk(&k).ok_or_else(|| format!("Unknown hotkey: {k}"))?).is_some() {
            return Err("A hotkey has one key besides the modifiers".into());
        }
    }
    let vk = main.ok_or("A hotkey needs a key besides the modifiers")?;
    let function_key = (0x70..=0x87).contains(&vk);
    if mods == 0 && !function_key {
        return Err("A hotkey needs Ctrl, Alt, Shift or Win".into());
    }
    Ok(Chord { mods, vk })
}

/// `["ctrl","alt","space"]` -> `Ctrl+Alt+Space`.
pub fn format_chord(keys: &[String]) -> String {
    keys.iter()
        .map(|k| {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// What to do with the registration after a settings change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Nothing,
    Register(Chord),
    Unregister,
}

/// Remembers which chord was tried last, so that every change of the status does not repeat a failed registration.
#[derive(Debug, Default)]
pub struct Tracker {
    attempted: Option<Vec<String>>,
}

impl Tracker {
    /// `wanted`: the chord while voice control is on, `None` while it is off. An unusable chord is an `Err` (once).
    pub fn plan(&mut self, wanted: Option<&[String]>) -> Result<Plan, String> {
        match wanted {
            None => Ok(if self.attempted.take().is_some() { Plan::Unregister } else { Plan::Nothing }),
            Some(keys) if self.attempted.as_deref() == Some(keys) => Ok(Plan::Nothing),
            Some(keys) => {
                self.attempted = Some(keys.to_vec());
                parse_chord(keys).map(Plan::Register)
            }
        }
    }

    /// The next `plan` tries again (the `Retry` button).
    pub fn forget(&mut self) {
        self.attempted = None;
    }
}

/// The text for `VoiceStatus.error` when Windows refuses the chord (another program has it).
pub fn taken_text(keys: &[String]) -> String {
    format!("The hotkey {} is used by another program", format_chord(keys))
}

/// Keys that are caught only while a request is on: `(esc, enter)`. Esc cancels from the first word until the result;
/// Enter answers `Yes` while a confirmation is open. The pill never takes the focus, so these cannot reach it any other way.
pub fn aux_keys(listening_or_busy: bool, confirming: bool) -> (bool, bool) {
    (listening_or_busy || confirming, confirming)
}

/// What the message loop reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    Main,
    Esc,
    Enter,
    /// The chord of dictation (a chord instead of the double Ctrl).
    Dictate,
}

// ---- The thin layer over Windows --------------------------------------------------------------------------------

#[cfg(windows)]
mod win {
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex};

    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS};
    use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY};

    use super::{Chord, Pressed, VK_ESCAPE, VK_RETURN};

    const ID_MAIN: i32 = 1;
    const ID_ESC: i32 = 2;
    const ID_ENTER: i32 = 3;
    const ID_DICT: i32 = 4;
    const WAKE: u32 = WM_APP;
    const QUIT: u32 = WM_APP + 1;
    /// A hotkey that does not repeat while it is held.
    const NO_REPEAT: u32 = 0x4000;
    /// `ERROR_HOTKEY_ALREADY_REGISTERED`.
    const TAKEN: i32 = 1409;

    enum Cmd {
        Main(Option<Chord>, Sender<Result<(), String>>),
        Dict(Option<Chord>, Sender<Result<(), String>>),
        Aux { esc: bool, enter: bool },
    }

    /// The registered hotkeys and their message loop. Dropping it unregisters everything and ends the thread.
    pub struct Hotkeys {
        thread_id: u32,
        tx: Sender<Cmd>,
        join: Mutex<Option<std::thread::JoinHandle<()>>>,
    }

    fn register(id: i32, mods: u32, vk: u32) -> windows::core::Result<()> {
        // SAFETY: a plain call without pointers; the hotkey belongs to the calling thread (no window).
        unsafe { RegisterHotKey(None, id, HOT_KEY_MODIFIERS(mods | NO_REPEAT), vk) }
    }

    fn unregister(id: i32) {
        // SAFETY: as above; an id that was never registered is an ignored error.
        let _ = unsafe { UnregisterHotKey(None, id) };
    }

    fn run(rx: Receiver<Cmd>, on_press: Arc<dyn Fn(Pressed) + Send + Sync>) {
        let (mut main, mut esc, mut enter, mut dict) = (false, false, false, false);
        let mut msg = MSG::default();
        loop {
            // SAFETY: `msg` is a valid MSG; a failure (-1) or WM_QUIT (0) ends the loop.
            let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if got.0 <= 0 || msg.message == QUIT {
                break;
            }
            match msg.message {
                WM_HOTKEY => match msg.wParam.0 as i32 {
                    ID_MAIN => on_press(Pressed::Main),
                    ID_ESC => on_press(Pressed::Esc),
                    ID_ENTER => on_press(Pressed::Enter),
                    ID_DICT => on_press(Pressed::Dictate),
                    _ => {}
                },
                WAKE => {
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            Cmd::Main(chord, reply) => {
                                if main {
                                    unregister(ID_MAIN);
                                    main = false;
                                }
                                let result = match chord {
                                    None => Ok(()),
                                    Some(c) => register(ID_MAIN, c.mods, c.vk).map(|_| main = true).map_err(|e| {
                                        if e.code().0 & 0xffff == TAKEN { String::from("taken") } else { e.to_string() }
                                    }),
                                };
                                let _ = reply.send(result);
                            }
                            Cmd::Dict(chord, reply) => {
                                if dict {
                                    unregister(ID_DICT);
                                    dict = false;
                                }
                                let result = match chord {
                                    None => Ok(()),
                                    Some(c) => register(ID_DICT, c.mods, c.vk).map(|_| dict = true).map_err(|e| {
                                        if e.code().0 & 0xffff == TAKEN { String::from("taken") } else { e.to_string() }
                                    }),
                                };
                                let _ = reply.send(result);
                            }
                            Cmd::Aux { esc: want_esc, enter: want_enter } => {
                                if want_esc != esc {
                                    if want_esc { esc = register(ID_ESC, 0, VK_ESCAPE).is_ok() } else { unregister(ID_ESC); esc = false }
                                }
                                if want_enter != enter {
                                    if want_enter { enter = register(ID_ENTER, 0, VK_RETURN).is_ok() } else { unregister(ID_ENTER); enter = false }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for (on, id) in [(main, ID_MAIN), (esc, ID_ESC), (enter, ID_ENTER), (dict, ID_DICT)] {
            if on {
                unregister(id);
            }
        }
    }

    impl Hotkeys {
        pub fn start(on_press: Arc<dyn Fn(Pressed) + Send + Sync>) -> Result<Hotkeys, String> {
            let (tx, rx) = mpsc::channel();
            let (id_tx, id_rx) = mpsc::channel();
            let join = std::thread::Builder::new()
                .name("decka-hotkey".into())
                .spawn(move || {
                    let mut msg = MSG::default();
                    // SAFETY: touching the queue creates it, so that posted messages are not lost.
                    let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };
                    // SAFETY: a plain query.
                    let _ = id_tx.send(unsafe { GetCurrentThreadId() });
                    run(rx, on_press);
                })
                .map_err(|e| e.to_string())?;
            let thread_id = id_rx.recv().map_err(|_| "The hotkey thread did not start".to_string())?;
            Ok(Hotkeys { thread_id, tx, join: Mutex::new(Some(join)) })
        }

        fn wake(&self, message: u32) {
            // SAFETY: posts a message without data to the thread that owns the loop.
            let _ = unsafe { PostThreadMessageW(self.thread_id, message, WPARAM(0), LPARAM(0)) };
        }

        /// Registers the chord (replacing the old one) or removes it (`None`). `Err` carries the text for the user.
        pub fn set_main(&self, chord: Option<Chord>, shown: &[String]) -> Result<(), String> {
            let (reply_tx, reply_rx) = mpsc::channel();
            self.tx.send(Cmd::Main(chord, reply_tx)).map_err(|_| "The hotkey thread stopped".to_string())?;
            self.wake(WAKE);
            match reply_rx.recv_timeout(std::time::Duration::from_secs(2)) {
                Ok(Err(e)) if e == "taken" => Err(super::taken_text(shown)),
                Ok(r) => r,
                Err(_) => Err("The hotkey thread did not answer".to_string()),
            }
        }

        /// Registers the chord of dictation (replacing the old one) or removes it (None).
        pub fn set_dictation(&self, chord: Option<Chord>, shown: &[String]) -> Result<(), String> {
            let (reply_tx, reply_rx) = mpsc::channel();
            self.tx.send(Cmd::Dict(chord, reply_tx)).map_err(|_| "The hotkey thread stopped".to_string())?;
            self.wake(WAKE);
            match reply_rx.recv_timeout(std::time::Duration::from_secs(2)) {
                Ok(Err(e)) if e == "taken" => Err(super::taken_text(shown)),
                Ok(r) => r,
                Err(_) => Err("The hotkey thread did not answer".to_string()),
            }
        }

        /// Esc and Enter while a request is on (`aux_keys`).
        pub fn set_aux(&self, esc: bool, enter: bool) {
            if self.tx.send(Cmd::Aux { esc, enter }).is_ok() {
                self.wake(WAKE);
            }
        }
    }

    impl Drop for Hotkeys {
        fn drop(&mut self) {
            self.wake(QUIT);
            if let Some(j) = self.join.lock().unwrap().take() {
                let _ = j.join();
            }
        }
    }
}

#[cfg(windows)]
pub use win::Hotkeys;

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_chord_becomes_modifiers_and_a_virtual_key() {
        assert_eq!(parse_chord(&keys(&["ctrl", "alt", "space"])), Ok(Chord { mods: MOD_CONTROL | MOD_ALT, vk: 0x20 }));
        assert_eq!(parse_chord(&keys(&["Win", "Shift", "V"])), Ok(Chord { mods: MOD_WIN | MOD_SHIFT, vk: 0x56 }));
        assert_eq!(parse_chord(&keys(&["ctrl", "5"])), Ok(Chord { mods: MOD_CONTROL, vk: 0x35 }));
        assert_eq!(parse_chord(&keys(&["f9"])), Ok(Chord { mods: 0, vk: 0x78 }), "a function key may stand alone");
        assert_eq!(parse_chord(&keys(&["ctrl", "f24"])).unwrap().vk, 0x87);
        assert_eq!(parse_chord(&keys(&["alt", "pagedown"])).unwrap().vk, 0x22);
    }

    #[test]
    fn unusable_chords_are_refused_with_a_reason() {
        assert!(parse_chord(&keys(&[])).is_err());
        assert!(parse_chord(&keys(&["ctrl", "alt"])).is_err(), "no key");
        assert!(parse_chord(&keys(&["a"])).is_err(), "a plain letter would break typing");
        assert!(parse_chord(&keys(&["ctrl", "a", "b"])).is_err(), "two keys");
        assert!(parse_chord(&keys(&["ctrl", "esc"])).is_err(), "esc is the cancel key");
        assert!(parse_chord(&keys(&["ctrl", "f25"])).is_err());
        assert!(parse_chord(&keys(&["ctrl", "f"])).is_ok());
        assert!(parse_chord(&keys(&["ctrl", "nonsense"])).unwrap_err().contains("nonsense"));
    }

    #[test]
    fn the_chord_is_shown_the_way_the_window_shows_it() {
        assert_eq!(format_chord(&keys(&["ctrl", "alt", "space"])), "Ctrl+Alt+Space");
        assert_eq!(taken_text(&keys(&["ctrl", "f9"])), "The hotkey Ctrl+F9 is used by another program");
    }

    #[test]
    fn the_registration_follows_voice_on_off_and_the_chord_and_does_not_repeat_a_failed_try() {
        let mut t = Tracker::default();
        let a = keys(&["ctrl", "alt", "space"]);
        let b = keys(&["ctrl", "f9"]);
        assert_eq!(t.plan(None), Ok(Plan::Nothing), "off and nothing registered");
        assert_eq!(t.plan(Some(&a)), Ok(Plan::Register(Chord { mods: MOD_CONTROL | MOD_ALT, vk: 0x20 })));
        assert_eq!(t.plan(Some(&a)), Ok(Plan::Nothing), "the same chord again: no repeat (a refusal by Windows stays refused)");
        assert_eq!(t.plan(Some(&b)), Ok(Plan::Register(Chord { mods: MOD_CONTROL, vk: 0x78 })), "a new chord replaces the old one");
        assert_eq!(t.plan(None), Ok(Plan::Unregister));
        assert_eq!(t.plan(None), Ok(Plan::Nothing));
        assert_eq!(t.plan(Some(&a)), Ok(Plan::Register(Chord { mods: MOD_CONTROL | MOD_ALT, vk: 0x20 })));
        t.forget();
        assert!(matches!(t.plan(Some(&a)), Ok(Plan::Register(_))), "Retry tries again");
        // A bad chord is reported once.
        let bad = keys(&["ctrl"]);
        assert!(t.plan(Some(&bad)).is_err());
        assert_eq!(t.plan(Some(&bad)), Ok(Plan::Nothing));
    }

    #[test]
    fn esc_and_enter_are_caught_only_while_they_mean_something() {
        assert_eq!(aux_keys(false, false), (false, false));
        assert_eq!(aux_keys(true, false), (true, false), "recording or thinking: Esc cancels");
        assert_eq!(aux_keys(false, true), (true, true), "confirmation: Esc = No, Enter = Yes");
    }
}
