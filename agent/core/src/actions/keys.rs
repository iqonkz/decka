//! Key name mapping for `hotkey` and `media` actions (DATA_MODEL.md 2.2, 2.3).

use enigo::Key;

use crate::model::MediaOp;

const F_KEYS: [Key; 24] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
    Key::F13,
    Key::F14,
    Key::F15,
    Key::F16,
    Key::F17,
    Key::F18,
    Key::F19,
    Key::F20,
    Key::F21,
    Key::F22,
    Key::F23,
    Key::F24,
];

/// Maps a validated lower-case key name to an enigo key; `None` for anything outside the allowed set.
pub fn map_key(name: &str) -> Option<Key> {
    let named = match name {
        "ctrl" => Key::Control,
        "alt" => Key::Alt,
        "shift" => Key::Shift,
        "win" => Key::Meta,
        "enter" => Key::Return,
        "esc" => Key::Escape,
        "space" => Key::Space,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "printscreen" => Key::PrintScr,
        "insert" => Key::Insert,
        // Punctuation: virtual-key codes (VK_OEM_*), never Unicode, so modifiers stay effective on any layout.
        "period" => Key::Other(0xBE),
        "comma" => Key::Other(0xBC),
        "minus" => Key::Other(0xBD),
        "equal" => Key::Other(0xBB),
        "slash" => Key::Other(0xBF),
        "semicolon" => Key::Other(0xBA),
        _ => return map_char_or_f(name),
    };
    Some(named)
}

fn map_char_or_f(name: &str) -> Option<Key> {
    let mut chars = name.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        // Letters and digits: VK code equals the upper-case ASCII code.
        return (first.is_ascii_lowercase() || first.is_ascii_digit()).then(|| Key::Other(u32::from(first.to_ascii_uppercase())));
    }
    let digits = name.strip_prefix('f')?;
    if digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: usize = digits.parse().ok()?;
    (1..=24).contains(&n).then(|| F_KEYS[n - 1])
}

/// The media key of an op; `None` for `shuffle` / `repeat`, which have no key (only the media session can do them).
pub fn map_media(op: MediaOp) -> Option<Key> {
    Some(match op {
        MediaOp::PlayPause => Key::MediaPlayPause,
        MediaOp::Next => Key::MediaNextTrack,
        MediaOp::Prev => Key::MediaPrevTrack,
        MediaOp::VolUp => Key::VolumeUp,
        MediaOp::VolDown => Key::VolumeDown,
        MediaOp::Mute => Key::VolumeMute,
        MediaOp::Shuffle | MediaOp::Repeat => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_names() -> Vec<String> {
        let mut names: Vec<String> = ["ctrl", "alt", "shift", "win"].map(String::from).to_vec();
        names.extend(('a'..='z').map(|c| c.to_string()));
        names.extend(('0'..='9').map(|c| c.to_string()));
        names.extend((1..=24).map(|n| format!("f{n}")));
        names.extend(
            ["enter", "esc", "space", "tab", "backspace", "delete", "home", "end", "pageup", "pagedown", "up", "down", "left", "right"]
                .map(String::from),
        );
        names.extend(["printscreen", "insert", "period", "comma", "minus", "equal", "slash", "semicolon"].map(String::from)); // M15
        names
    }

    #[test]
    fn every_allowed_name_maps() {
        let names = all_names();
        assert_eq!(names.len(), 86);
        for n in &names {
            assert!(map_key(n).is_some(), "no mapping for {n}");
        }
    }

    #[test]
    fn characters_use_virtual_key_codes() {
        for (name, vk) in [("a", 0x41), ("z", 0x5A), ("0", 0x30), ("9", 0x39), ("period", 0xBE), ("slash", 0xBF)] {
            assert!(matches!(map_key(name), Some(Key::Other(v)) if v == vk), "{name}");
        }
        for n in all_names() {
            assert!(!matches!(map_key(&n), Some(Key::Unicode(_))), "{n} must not be Unicode");
        }
    }

    #[test]
    fn names_outside_the_set_do_not_map() {
        for n in ["f25", "f0", "f01", "ctrl+c", "A", "", "ab", "capslock"] {
            assert!(map_key(n).is_none(), "{n} should not map");
        }
    }

    #[test]
    fn media_ops_map_to_media_keys() {
        assert!(matches!(map_media(MediaOp::PlayPause), Some(Key::MediaPlayPause)));
        assert!(matches!(map_media(MediaOp::Mute), Some(Key::VolumeMute)));
        assert!(map_media(MediaOp::Shuffle).is_none() && map_media(MediaOp::Repeat).is_none());
    }
}
