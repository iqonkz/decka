//! The state machine of the "island" (T26.4, TASKS.md M26): one window at the top of the main monitor that shows the voice
//! assistant and the lyrics of the song. This file holds only the decision - which of `hidden | tab | compact | expanded` is
//! right now - as a pure step function with tests; the window, the cursor and the timers live in the application.

use serde::Serialize;
use serde_json::{json, Value};

use crate::lyrics_overlay::PcOverlay;
use crate::voice::slowpath::Progress;

/// T29.5: the cursor rests on the bar this long before it opens into the words.
pub const BAR_OPEN_MS: u64 = 150;
/// The cursor left the words: they fold into the bar after this.
pub const BAR_CLOSE_MS: u64 = 600;
/// An answer stays this long when nobody points at it; 20 s when it has buttons.
pub const ANSWER_HOLD_MS: u64 = 8_000;
pub const ANSWER_HOLD_ACTIONS_MS: u64 = 20_000;
/// Pointing at an answer keeps it at least this long after the cursor leaves.
pub const HOVER_GRACE_MS: u64 = 2_000;
/// The application asks the cursor this often.
pub const POLL_MS: u64 = 60;

/// The media button of the island window (`media_control { kind }`) as the same media operation the deck buttons use; anything
/// outside the four names is refused (T26.F.10).
/// The state of the lyrics for the island (`IslandLyricsState.status`): lines found, a search running, or nothing (off / not found).
pub fn lyrics_status(has_lines: bool, loading: bool) -> &'static str {
    if has_lines {
        "ready"
    } else if loading {
        "loading"
    } else {
        "none"
    }
}

pub fn media_op(kind: &str) -> Option<crate::model::MediaOp> {
    use crate::model::MediaOp;
    Some(match kind {
        "prev" => MediaOp::Prev,
        "play_pause" => MediaOp::PlayPause,
        "next" => MediaOp::Next,
        "mute" => MediaOp::Mute,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Hidden,
    Tab,
    Compact,
    Expanded,
    /// T29.4: a dictation - the window is a small orb at the text cursor (decided by the application from `voice::orb`).
    Orb,
}

pub const BAR_HEIGHT: u32 = 10;
pub const BAR_HEIGHTS: std::ops::RangeInclusive<u32> = 6..=16;

/// Where the bar and the words rest along the top edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

impl Rest {
    pub fn name(self) -> &'static str {
        match self {
            Rest::Bar => "bar",
            Rest::Lyrics => "lyrics",
        }
    }
}

/// The settings of the island as they are used (`config.island` with the defaults filled in), and as the settings window
/// reads and writes them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(serialize_with = "rest_name")]
    pub resting_mode: Rest,
    pub bar_height: u32,
    pub bar_position: Align,
}

fn rest_name<S: serde::Serializer>(rest: &Rest, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(rest.name())
}

impl Default for Settings {
    fn default() -> Self {
        Settings { resting_mode: Rest::Bar, bar_height: BAR_HEIGHT, bar_position: Align::Center }
    }
}

impl Settings {
    pub fn of(config: Option<&crate::model::IslandConfig>) -> Settings {
        let defaults = Settings::default();
        let Some(c) = config else { return defaults };
        Settings {
            resting_mode: if c.resting_mode.as_deref() == Some("lyrics") { Rest::Lyrics } else { Rest::Bar },
            bar_height: c.bar_height.map_or(defaults.bar_height, |h| h.clamp(*BAR_HEIGHTS.start(), *BAR_HEIGHTS.end())),
            bar_position: match c.bar_position.as_deref() {
                Some("left") => Align::Left,
                Some("right") => Align::Right,
                _ => Align::Center,
            },
        }
    }
}

/// What the voice assistant is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceInput {
    Idle,
    /// Listening, recognising, thinking, waiting for a confirmation.
    Active,
    /// An answer or an error is on show; `actions`: it has buttons.
    Answer { actions: bool },
}

/// What the island is while a song plays and nobody asks anything (`config.island.rest`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Rest {
    /// A thin bar at the top edge; pointing at it opens the words.
    #[default]
    Bar,
    /// The words all the time.
    Lyrics,
}

#[derive(Debug, Clone, Copy)]
pub struct Inputs {
    pub now_ms: u64,
    pub voice: VoiceInput,
    /// The cursor is inside the visible panel (or the zone of the tab).
    pub hovered: bool,
    /// `lyrics_overlay::visible`: a synced song plays, the feature is on, not hidden by voice.
    pub lyrics_visible: bool,
    pub lyrics_mode: PcOverlay,
    /// A song is playing (or was a moment ago) and the island may say so: the overlay is not off and not hidden by voice.
    pub music: bool,
    pub rest: Rest,
    pub fullscreen: bool,
}

/// The voice part of the event `island_state` (shared `IslandVoiceState`) and what the machine needs to know of it, made of
/// the payload the voice host sends to the PC window (`host::pill_payload`). `None` payload: nothing is going on.
pub fn voice_view(payload: Option<&Value>, slow: Option<Progress>, download: Option<u8>, loading_sec: Option<u32>, chips: &[Value]) -> (VoiceInput, Option<Value>) {
    let Some(p) = payload else { return (VoiceInput::Idle, None) };
    let state = p.get("state").and_then(Value::as_str).unwrap_or("idle");
    let actions: Vec<Value> = p.get("actions").and_then(Value::as_array).cloned().unwrap_or_default();
    let (input, phase) = match state {
        "listening" | "loading" | "recognizing" | "thinking" | "confirm" => (VoiceInput::Active, state),
        "done" | "error" => (VoiceInput::Answer { actions: !actions.is_empty() }, state),
        _ => return (VoiceInput::Idle, None),
    };
    let mut v = json!({ "active": input == VoiceInput::Active, "phase": phase });
    let o = v.as_object_mut().expect("an object");
    for (from, to) in [("level", "level"), ("heard", "heard"), ("understood", "understood"), ("executing", "executing"), ("reply", "reply"), ("replyKey", "replyKey"), ("replyArgs", "replyArgs"), ("confirm", "confirm"), ("id", "confirmId")] {
        match p.get(from) {
            // The id is only a handle of the confirmation.
            Some(_) if to == "confirmId" && phase != "confirm" => {}
            Some(x) => {
                o.insert(to.into(), x.clone());
            }
            None => {}
        }
    }
    if !actions.is_empty() {
        o.insert("actions".into(), Value::Array(actions));
    }
    if let Some(s) = slow {
        o.insert("slow".into(), json!({ "etaSec": s.eta_sec, "elapsedSec": s.elapsed_sec }));
    }
    if let Some(pct) = download {
        o.insert("download".into(), json!({ "pct": pct }));
    }
    if let Some(sec) = loading_sec.filter(|_| phase == "loading") {
        o.insert("loading".into(), json!({ "etaSec": sec }));
    }
    if !chips.is_empty() && phase == "listening" {
        o.insert("chips".into(), Value::Array(chips.to_vec()));
    }
    (input, Some(v))
}

#[derive(Debug, Default)]
pub struct Machine {
    /// The bar was opened into the words by the cursor.
    open: bool,
    hover_since: Option<u64>,
    left_at: Option<u64>,
    answer_since: Option<u64>,
    hold_until: u64,
}

impl Machine {
    pub fn step(&mut self, i: &Inputs) -> Mode {
        match i.voice {
            VoiceInput::Active => {
                self.answer_since = None;
                self.fold();
                return Mode::Expanded;
            }
            VoiceInput::Answer { actions } => {
                if self.answer_since.is_none() {
                    self.answer_since = Some(i.now_ms);
                    self.hold_until = i.now_ms + if actions { ANSWER_HOLD_ACTIONS_MS } else { ANSWER_HOLD_MS };
                }
                if i.hovered {
                    self.hold_until = self.hold_until.max(i.now_ms + HOVER_GRACE_MS);
                }
                if i.now_ms < self.hold_until {
                    self.fold();
                    return Mode::Expanded;
                }
            }
            VoiceInput::Idle => self.answer_since = None,
        }
        self.lyrics(i)
    }

    /// Back to the bar: after a request the words are not left open.
    fn fold(&mut self) {
        self.open = false;
        self.hover_since = None;
        self.left_at = None;
    }

    /// The part of the song: the voice is not on show.
    fn lyrics(&mut self, i: &Inputs) -> Mode {
        // A full-screen program hides it; only `always` is meant to stay over it (DESIGN.md 4.17).
        if (!i.lyrics_visible && !i.music) || (i.fullscreen && i.lyrics_mode != PcOverlay::Always) {
            self.fold();
            return Mode::Hidden;
        }
        if i.lyrics_mode == PcOverlay::Always || i.rest == Rest::Lyrics {
            return Mode::Compact;
        }
        if i.hovered {
            self.left_at = None;
            let since = *self.hover_since.get_or_insert(i.now_ms);
            self.open |= i.now_ms.saturating_sub(since) >= BAR_OPEN_MS;
        } else {
            self.hover_since = None;
            if self.open {
                let left = *self.left_at.get_or_insert(i.now_ms);
                if i.now_ms.saturating_sub(left) >= BAR_CLOSE_MS {
                    self.fold();
                }
            }
        }
        if self.open {
            Mode::Compact
        } else {
            Mode::Tab
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn the_lyrics_status_is_loading_only_while_searching() {
        assert_eq!([lyrics_status(true, true), lyrics_status(false, true), lyrics_status(false, false)], ["ready", "loading", "none"]);
    }

    #[test]
    fn media_buttons_name_four_operations_and_nothing_else() {
        use crate::model::MediaOp;
        assert_eq!(media_op("mute"), Some(MediaOp::Mute), "mute is the toggle of the output");
        assert_eq!([media_op("prev"), media_op("play_pause"), media_op("next")], [Some(MediaOp::Prev), Some(MediaOp::PlayPause), Some(MediaOp::Next)]);
        for bad in ["", "vol_up", "shuffle", "Mute", "run:calc", "next "] {
            assert_eq!(media_op(bad), None, "{bad}");
        }
    }

    use super::*;

    fn song(now_ms: u64) -> Inputs {
        Inputs { now_ms, voice: VoiceInput::Idle, hovered: false, lyrics_visible: true, lyrics_mode: PcOverlay::Auto, music: true, rest: Rest::Bar, fullscreen: false }
    }

    /// Nothing plays.
    fn silence(now_ms: u64) -> Inputs {
        Inputs { lyrics_visible: false, music: false, ..song(now_ms) }
    }

    #[test]
    fn nothing_to_show_is_hidden() {
        let mut m = Machine::default();
        assert_eq!(m.step(&silence(0)), Mode::Hidden);
    }

    #[test]
    fn voice_is_expanded_and_an_answer_is_held_8_seconds_or_20_with_buttons() {
        let mut m = Machine::default();
        assert_eq!(m.step(&Inputs { voice: VoiceInput::Active, ..song(0) }), Mode::Expanded);
        let answer = |now, actions| Inputs { voice: VoiceInput::Answer { actions }, ..silence(now) };
        assert_eq!(m.step(&answer(1_000, false)), Mode::Expanded);
        assert_eq!(m.step(&answer(8_999, false)), Mode::Expanded);
        assert_eq!(m.step(&answer(9_000, false)), Mode::Hidden, "8 s after the answer, back to what was");
        let mut m = Machine::default();
        assert_eq!(m.step(&answer(0, true)), Mode::Expanded);
        assert_eq!(m.step(&answer(19_999, true)), Mode::Expanded);
        assert_eq!(m.step(&answer(20_000, true)), Mode::Hidden);
    }

    #[test]
    fn pointing_at_an_answer_keeps_it_and_it_leaves_two_seconds_after_the_cursor() {
        let mut m = Machine::default();
        let answer = |now, hovered| Inputs { voice: VoiceInput::Answer { actions: false }, hovered, ..silence(now) };
        assert_eq!(m.step(&answer(0, false)), Mode::Expanded);
        assert_eq!(m.step(&answer(7_000, true)), Mode::Expanded);
        assert_eq!(m.step(&answer(30_000, true)), Mode::Expanded, "held while pointed at");
        assert_eq!(m.step(&answer(31_999, false)), Mode::Expanded);
        assert_eq!(m.step(&answer(32_000, false)), Mode::Hidden);
    }

    #[test]
    fn a_song_rests_as_the_bar_and_an_answer_returns_to_it() {
        let mut m = Machine::default();
        assert_eq!(m.step(&song(0)), Mode::Tab);
        assert_eq!(m.step(&Inputs { voice: VoiceInput::Active, ..song(500) }), Mode::Expanded);
        assert_eq!(m.step(&Inputs { voice: VoiceInput::Answer { actions: false }, ..song(1_000) }), Mode::Expanded);
        assert_eq!(m.step(&Inputs { voice: VoiceInput::Answer { actions: false }, ..song(9_000) }), Mode::Tab);
        // A song without lines still shows the bar.
        assert_eq!(m.step(&Inputs { lyrics_visible: false, ..song(10_000) }), Mode::Tab);
        assert_eq!(m.step(&silence(11_000)), Mode::Hidden);
    }

    #[test]
    fn the_bar_opens_after_a_short_hover_and_folds_after_the_cursor_left() {
        let mut m = Machine::default();
        let at = |now, hovered| Inputs { hovered, ..song(now) };
        assert_eq!(m.step(&at(0, true)), Mode::Tab);
        assert_eq!(m.step(&at(149, true)), Mode::Tab);
        assert_eq!(m.step(&at(150, true)), Mode::Compact);
        assert_eq!(m.step(&at(5_000, false)), Mode::Compact, "just left");
        assert_eq!(m.step(&at(5_599, false)), Mode::Compact);
        assert_eq!(m.step(&at(5_600, false)), Mode::Tab);
        // Passing over it does not open it.
        assert_eq!(m.step(&at(6_000, true)), Mode::Tab);
        assert_eq!(m.step(&at(6_060, false)), Mode::Tab);
        assert_eq!(m.step(&at(6_200, true)), Mode::Tab);
        // Coming back before the fold keeps it open.
        assert_eq!(m.step(&at(6_400, true)), Mode::Compact);
        assert_eq!(m.step(&at(7_000, false)), Mode::Compact);
        assert_eq!(m.step(&at(7_300, true)), Mode::Compact);
        assert_eq!(m.step(&at(7_800, false)), Mode::Compact);
    }

    #[test]
    fn words_all_the_time_by_the_setting_or_the_overlay() {
        let mut m = Machine::default();
        assert_eq!(m.step(&Inputs { rest: Rest::Lyrics, ..song(0) }), Mode::Compact);
        assert_eq!(m.step(&Inputs { rest: Rest::Lyrics, fullscreen: true, ..song(1) }), Mode::Hidden);
        for t in [0, 10_000, 100_000] {
            assert_eq!(m.step(&Inputs { lyrics_mode: PcOverlay::Always, ..song(t) }), Mode::Compact);
        }
        assert_eq!(m.step(&Inputs { lyrics_mode: PcOverlay::Always, fullscreen: true, ..song(1) }), Mode::Compact);
    }

    #[test]
    fn a_full_screen_program_hides_the_bar_but_not_the_voice() {
        let mut m = Machine::default();
        assert_eq!(m.step(&Inputs { fullscreen: true, ..song(0) }), Mode::Hidden);
        assert_eq!(m.step(&Inputs { fullscreen: true, voice: VoiceInput::Active, ..song(1) }), Mode::Expanded);
        assert_eq!(m.step(&Inputs { fullscreen: true, voice: VoiceInput::Answer { actions: false }, ..song(2) }), Mode::Expanded);
    }
    #[test]
    fn the_settings_have_defaults_and_stay_in_range() {
        use crate::model::IslandConfig;
        assert_eq!(Settings::of(None), Settings { resting_mode: Rest::Bar, bar_height: 10, bar_position: Align::Center });
        let c = IslandConfig { resting_mode: Some("lyrics".into()), bar_height: Some(40), bar_position: Some("right".into()) };
        assert_eq!(Settings::of(Some(&c)), Settings { resting_mode: Rest::Lyrics, bar_height: 16, bar_position: Align::Right });
        let c = IslandConfig { resting_mode: Some("else".into()), bar_height: Some(1), bar_position: Some("else".into()) };
        assert_eq!(Settings::of(Some(&c)), Settings { bar_height: 6, ..Settings::default() });
        assert_eq!(serde_json::to_value(Settings::default()).unwrap(), json!({ "restingMode": "bar", "barHeight": 10, "barPosition": "center" }));
    }

    #[test]
    fn the_voice_payload_becomes_the_voice_of_the_event() {
        let none = voice_view(None, None, None, None, &[]);
        assert_eq!((none.0, none.1), (VoiceInput::Idle, None));
        let idle = json!({ "state": "idle" });
        assert_eq!(voice_view(Some(&idle), None, None, None, &[]).0, VoiceInput::Idle);
        let listening = json!({ "state": "listening", "level": 0.4, "id": "a1", "hotkey": "Ctrl+Alt+Space", "source": "pc" });
        let (input, v) = voice_view(Some(&listening), None, None, None, &[json!({ "label": "Translate", "command": "translate" })]);
        let v = v.unwrap();
        assert_eq!(input, VoiceInput::Active);
        assert_eq!((v["active"].clone(), v["phase"].clone(), v["level"].clone()), (json!(true), json!("listening"), json!(0.4)));
        assert!(v.get("confirmId").is_none() && v.get("hotkey").is_none() && v.get("source").is_none(), "only what the island shows");
        assert_eq!(v["chips"][0]["label"], "Translate", "chips are offered while it listens");
        let done = json!({ "state": "done", "reply": "It is about cats.", "actions": ["copy", "insert"], "replyKey": "voice.status.cpu", "replyArgs": { "pct": "23" } });
        let (input, v) = voice_view(Some(&done), None, None, None, &[json!({ "label": "x" })]);
        let v = v.unwrap();
        assert_eq!(input, VoiceInput::Answer { actions: true });
        assert_eq!((v["active"].clone(), v["actions"].clone(), v["replyArgs"]["pct"].clone()), (json!(false), json!(["copy", "insert"]), json!("23")));
        assert!(v.get("chips").is_none());
        let plain = json!({ "state": "error", "reply": "Nothing to continue" });
        assert_eq!(voice_view(Some(&plain), None, None, None, &[]).0, VoiceInput::Answer { actions: false });
        let confirm = json!({ "state": "confirm", "id": "c9", "confirm": { "text": "Lock the computer", "key": "voice.confirm.lock" } });
        assert_eq!(voice_view(Some(&confirm), None, None, None, &[]).1.unwrap()["confirmId"], "c9");
        let slow = voice_view(Some(&json!({ "state": "thinking" })), Some(Progress { eta_sec: 10, elapsed_sec: 3 }), None, None, &[]).1.unwrap();
        assert_eq!(slow["slow"], json!({ "etaSec": 10, "elapsedSec": 3 }));
        let downloading = voice_view(Some(&json!({ "state": "thinking" })), None, Some(42), None, &[]).1.unwrap();
        assert_eq!(downloading["download"], json!({ "pct": 42 }));
        let loading = voice_view(Some(&json!({ "state": "loading" })), None, None, Some(7), &[]).1.unwrap();
        assert_eq!(loading["loading"], json!({ "etaSec": 7 }));
        assert!(voice_view(Some(&json!({ "state": "thinking" })), None, None, Some(7), &[]).1.unwrap().get("loading").is_none());
    }
}