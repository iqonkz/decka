//! The lyrics window on the PC (0.30, DESIGN.md 4.17, DATA_MODEL.md 14): when it is shown and which two lines it carries.
//! Pure functions over the sections that already exist (`media`, `lyrics`, `layout`): no network, no new data. The window
//! itself lives in the Tauri crate; the rules are here so they can be tested without a screen.

use serde_json::Value;

/// A paused song keeps the window this long, then it goes away (DESIGN.md 4.17).
pub const PAUSE_HIDE_MS: u64 = 3_000;

/// `config.media.lyrics.pcOverlay`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcOverlay {
    Off,
    Auto,
    Always,
}

impl PcOverlay {
    /// Absent or unknown values mean `Auto`.
    pub fn parse(value: Option<&str>) -> PcOverlay {
        match value {
            Some("off") => PcOverlay::Off,
            Some("always") => PcOverlay::Always,
            _ => PcOverlay::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PcOverlay::Off => "off",
            PcOverlay::Auto => "auto",
            PcOverlay::Always => "always",
        }
    }
}

/// Everything the decision depends on.
#[derive(Debug, Clone, Copy)]
pub struct Inputs {
    pub mode: PcOverlay,
    /// `media.lyrics.enabled`: the overlay is part of the lyrics feature, off with it.
    pub lyrics_enabled: bool,
    /// There are synced lyrics for the current track (plain text is never shown on the PC).
    pub synced: bool,
    pub playing: bool,
    /// How long the song has been paused (0 while it plays).
    pub paused_for_ms: u64,
    pub phone_connected: bool,
    /// A phone is connected and the layout has a `media` widget of size `L` (4×2 or bigger), see `phone_shows_big_media`.
    pub phone_shows_media_l: bool,
    /// A full-screen program (game, video) is in the foreground.
    pub fullscreen: bool,
}

/// Whether the window is on screen.
pub fn visible(i: &Inputs) -> bool {
    if i.mode == PcOverlay::Off || !i.lyrics_enabled || !i.synced {
        return false;
    }
    if i.fullscreen {
        return false;
    }
    // A paused song stays for 3 s, then the window goes.
    if !i.playing && i.paused_for_ms >= PAUSE_HIDE_MS {
        return false;
    }
    match i.mode {
        PcOverlay::Always => true,
        // Auto: the phone already shows the words when it is connected and has the big player.
        _ => !(i.phone_connected && i.phone_shows_media_l),
    }
}

/// Whether a full-screen program (game, video, presentation) owns the screen: Windows' own answer to "may I show a
/// notification now" (`SHQueryUserNotificationState`), the same one toasts obey. Not on Windows: `false`.
#[cfg(windows)]
pub fn foreground_is_fullscreen() -> bool {
    use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
    // SAFETY: a plain query without arguments other than the result slot.
    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE,
        Err(_) => false,
    }
}

#[cfg(not(windows))]
pub fn foreground_is_fullscreen() -> bool {
    false
}

/// A `media` widget of size `L` (`w >= 4`, `h >= 2`, DESIGN.md 4.17) on a normal page of the layout section.
/// The PC does not know which page the phone is on, so any such widget counts (it errs on the side of showing nothing twice).
pub fn phone_shows_big_media(layout: &Value) -> bool {
    let Some(pages) = layout.get("pages").and_then(Value::as_array) else { return false };
    pages
        .iter()
        .filter(|p| p.get("popup").and_then(Value::as_bool) != Some(true))
        .filter_map(|p| p.get("items").and_then(Value::as_array))
        .flatten()
        .any(|it| {
            it.get("kind").and_then(Value::as_str) == Some("media")
                && it.get("w").and_then(Value::as_u64).unwrap_or(0) >= 4
                && it.get("h").and_then(Value::as_u64).unwrap_or(0) >= 2
        })
}

/// The current line and the next one at `position_ms` (the song position plus the per-track shift). Before the first
/// line there is no current line, only the next one.
pub fn two_lines(lines: &[(u64, String)], position_ms: i64) -> (Option<String>, Option<String>) {
    let mut current: Option<usize> = None;
    for (n, (t, _)) in lines.iter().enumerate() {
        if (*t as i64) <= position_ms {
            current = Some(n);
        } else {
            break;
        }
    }
    let next_index = current.map_or(0, |n| n + 1);
    let text = |n: usize| lines.get(n).map(|(_, s)| s.clone()).filter(|s| !s.trim().is_empty());
    (current.and_then(text), text(next_index))
}

/// The synced lines of a `lyrics` section value: `(t, text)` in ms; `None` for plain text or no lyrics.
pub fn synced_lines(lyrics: &Value) -> Option<Vec<(u64, String)>> {
    if lyrics.get("synced").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let lines: Vec<(u64, String)> = lyrics
        .get("lines")?
        .as_array()?
        .iter()
        .filter_map(|l| Some((l.get("t")?.as_u64()?, l.get("text")?.as_str()?.to_string())))
        .collect();
    (!lines.is_empty()).then_some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base() -> Inputs {
        Inputs {
            mode: PcOverlay::Auto,
            lyrics_enabled: true,
            synced: true,
            playing: true,
            paused_for_ms: 0,
            phone_connected: false,
            phone_shows_media_l: false,
            fullscreen: false,
        }
    }

    #[test]
    fn unknown_or_absent_mode_is_auto() {
        assert_eq!(PcOverlay::parse(None), PcOverlay::Auto);
        assert_eq!(PcOverlay::parse(Some("sometimes")), PcOverlay::Auto);
        assert_eq!(PcOverlay::parse(Some("")), PcOverlay::Auto);
        assert_eq!(PcOverlay::parse(Some("off")), PcOverlay::Off);
        assert_eq!(PcOverlay::parse(Some("always")), PcOverlay::Always);
        assert_eq!(PcOverlay::parse(Some("auto")), PcOverlay::Auto);
    }

    #[test]
    fn off_never_shows() {
        assert!(!visible(&Inputs { mode: PcOverlay::Off, ..base() }));
    }

    #[test]
    fn needs_the_lyrics_feature_and_synced_text() {
        assert!(visible(&base()));
        assert!(!visible(&Inputs { lyrics_enabled: false, ..base() }));
        assert!(!visible(&Inputs { synced: false, ..base() }));
        assert!(!visible(&Inputs { synced: false, mode: PcOverlay::Always, ..base() }));
    }

    #[test]
    fn fullscreen_program_hides_even_always() {
        assert!(!visible(&Inputs { fullscreen: true, ..base() }));
        assert!(!visible(&Inputs { fullscreen: true, mode: PcOverlay::Always, ..base() }));
    }

    #[test]
    fn pause_keeps_the_window_for_three_seconds() {
        assert!(visible(&Inputs { playing: false, paused_for_ms: 2_999, ..base() }));
        assert!(!visible(&Inputs { playing: false, paused_for_ms: 3_000, ..base() }));
        assert!(!visible(&Inputs { playing: false, paused_for_ms: 10_000, mode: PcOverlay::Always, ..base() }));
    }

    #[test]
    fn auto_hides_when_the_phone_shows_media_l_always_does_not() {
        let on_phone = Inputs { phone_connected: true, phone_shows_media_l: true, ..base() };
        assert!(!visible(&on_phone));
        assert!(visible(&Inputs { mode: PcOverlay::Always, ..on_phone }));
        // A phone that is not connected shows nothing: the PC does.
        assert!(visible(&Inputs { phone_connected: false, phone_shows_media_l: true, ..base() }));
        // Connected but without the big player: the PC shows the words.
        assert!(visible(&Inputs { phone_connected: true, phone_shows_media_l: false, ..base() }));
    }

    #[test]
    fn media_l_means_a_media_widget_of_4x2_on_a_normal_page() {
        let layout = |kind: &str, w: u64, h: u64, popup: bool| {
            json!({ "pages": [{ "id": "p", "popup": popup, "items": [{ "id": "n", "kind": kind, "x": 0, "y": 0, "w": w, "h": h }] }] })
        };
        assert!(phone_shows_big_media(&layout("media", 4, 2, false)));
        assert!(phone_shows_big_media(&layout("media", 6, 3, false)));
        assert!(!phone_shows_big_media(&layout("media", 3, 2, false)));
        assert!(!phone_shows_big_media(&layout("media", 4, 1, false)));
        assert!(!phone_shows_big_media(&layout("weather", 4, 2, false)));
        assert!(!phone_shows_big_media(&layout("media", 4, 2, true)));
        assert!(!phone_shows_big_media(&json!({})));
    }

    #[test]
    fn two_lines_follow_the_position_and_the_shift() {
        let lines = vec![(1_000, "one".to_string()), (5_000, "two".to_string()), (9_000, "three".to_string())];
        assert_eq!(two_lines(&lines, 0), (None, Some("one".into())));
        assert_eq!(two_lines(&lines, 1_000), (Some("one".into()), Some("two".into())));
        assert_eq!(two_lines(&lines, 4_999), (Some("one".into()), Some("two".into())));
        assert_eq!(two_lines(&lines, 5_000 + 500), (Some("two".into()), Some("three".into())));
        assert_eq!(two_lines(&lines, 60_000), (Some("three".into()), None));
        // A shift of the track moves the position the same way the phone does.
        assert_eq!(two_lines(&lines, 4_600 + 500), (Some("two".into()), Some("three".into())));
    }

    #[test]
    fn empty_lines_are_skipped_not_shown() {
        let lines = vec![(1_000, "".to_string()), (5_000, "two".to_string())];
        assert_eq!(two_lines(&lines, 2_000), (None, Some("two".into())));
    }

    #[test]
    fn only_synced_sections_give_lines() {
        let synced = json!({ "trackKey": "k", "synced": true, "offsetMs": 0, "lines": [{ "t": 1000, "text": "a" }, { "t": 2000, "text": "b" }] });
        assert_eq!(synced_lines(&synced), Some(vec![(1000, "a".into()), (2000, "b".into())]));
        let plain = json!({ "trackKey": "k", "synced": false, "offsetMs": 0, "lines": [{ "text": "a" }] });
        assert_eq!(synced_lines(&plain), None);
        assert_eq!(synced_lines(&Value::Null), None);
    }
}
