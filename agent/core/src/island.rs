//! The state machine of the "island" (T26.4, TASKS.md M26): one window at the top of the main monitor that shows the voice
//! assistant and the lyrics of the song. This file holds only the decision - which of `hidden | tab | compact | expanded` is
//! right now - as a pure step function with tests; the window, the cursor and the timers live in the application.

use serde::Serialize;
use serde_json::{json, Value};

use crate::lyrics_overlay::PcOverlay;
use crate::voice::slowpath::Progress;

/// A new track in `auto` mode shows the line this long, then folds into the tab.
pub const TRACK_COMPACT_MS: u64 = 6_000;
/// The cursor left the panel: it folds into the tab after this.
pub const TAB_LEAVE_MS: u64 = 1_500;
/// An answer stays this long when nobody points at it; 20 s when it has buttons.
pub const ANSWER_HOLD_MS: u64 = 8_000;
pub const ANSWER_HOLD_ACTIONS_MS: u64 = 20_000;
/// Pointing at an answer keeps it at least this long after the cursor leaves.
pub const HOVER_GRACE_MS: u64 = 2_000;
/// The application asks the cursor this often.
pub const POLL_MS: u64 = 60;

/// The media button of the island window (`media_control { kind }`) as the same media operation the deck buttons use; anything
/// outside the four names is refused (T26.F.10).
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

#[derive(Debug, Clone, Copy)]
pub struct Inputs<'a> {
    pub now_ms: u64,
    pub voice: VoiceInput,
    /// The cursor is inside the visible panel (or the zone of the tab).
    pub hovered: bool,
    /// `lyrics_overlay::visible`: a synced song plays, the feature is on, not hidden by voice.
    pub lyrics_visible: bool,
    pub lyrics_mode: PcOverlay,
    /// Identifies the track (changes with the track).
    pub track: Option<&'a str>,
    pub fullscreen: bool,
}

/// The voice part of the event `island_state` (shared `IslandVoiceState`) and what the machine needs to know of it, made of
/// the payload the voice host sends to the PC window (`host::pill_payload`). `None` payload: nothing is going on.
pub fn voice_view(payload: Option<&Value>, slow: Option<Progress>, chips: &[Value]) -> (VoiceInput, Option<Value>) {
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
    for (from, to) in [("level", "level"), ("heard", "heard"), ("reply", "reply"), ("replyKey", "replyKey"), ("replyArgs", "replyArgs"), ("confirm", "confirm"), ("id", "confirmId")] {
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
    if !chips.is_empty() && phase == "listening" {
        o.insert("chips".into(), Value::Array(chips.to_vec()));
    }
    (input, Some(v))
}

#[derive(Debug, Default)]
pub struct Machine {
    track: Option<String>,
    compact_until: u64,
    was_hovered: bool,
    left_at: Option<u64>,
    answer_since: Option<u64>,
    hold_until: u64,
}

impl Machine {
    pub fn step(&mut self, i: &Inputs) -> Mode {
        match i.voice {
            VoiceInput::Active => {
                self.answer_since = None;
                self.was_hovered = i.hovered;
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
                    self.was_hovered = i.hovered;
                    return Mode::Expanded;
                }
            }
            VoiceInput::Idle => self.answer_since = None,
        }
        self.lyrics(i)
    }

    /// The part of the song: the voice is not on show.
    fn lyrics(&mut self, i: &Inputs) -> Mode {
        if !i.lyrics_visible {
            self.track = None;
            self.was_hovered = i.hovered;
            self.left_at = None;
            return Mode::Hidden;
        }
        // A full-screen program hides it; only `always` is meant to stay over it (DESIGN.md 4.17).
        if i.fullscreen && i.lyrics_mode != PcOverlay::Always {
            return Mode::Hidden;
        }
        if i.lyrics_mode == PcOverlay::Always {
            return Mode::Compact;
        }
        if self.track.as_deref() != i.track {
            self.track = i.track.map(str::to_string);
            self.compact_until = i.now_ms + TRACK_COMPACT_MS;
        }
        if i.hovered {
            self.left_at = None;
        } else if self.was_hovered {
            self.left_at = Some(i.now_ms);
        }
        self.was_hovered = i.hovered;
        let lingering = self.left_at.is_some_and(|t| i.now_ms.saturating_sub(t) < TAB_LEAVE_MS);
        if i.now_ms < self.compact_until || i.hovered || lingering {
            Mode::Compact
        } else {
            Mode::Tab
        }
    }
}

#[cfg(test)]
mod tests {
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

    fn song(now_ms: u64) -> Inputs<'static> {
        Inputs { now_ms, voice: VoiceInput::Idle, hovered: false, lyrics_visible: true, lyrics_mode: PcOverlay::Auto, track: Some("a"), fullscreen: false }
    }

    #[test]
    fn nothing_to_show_is_hidden() {
        let mut m = Machine::default();
        assert_eq!(m.step(&Inputs { lyrics_visible: false, ..song(0) }), Mode::Hidden);
    }

    #[test]
    fn voice_is_expanded_and_an_answer_is_held_8_seconds_or_20_with_buttons() {
        let mut m = Machine::default();
        assert_eq!(m.step(&Inputs { voice: VoiceInput::Active, ..song(0) }), Mode::Expanded);
        let answer = |now, actions| Inputs { voice: VoiceInput::Answer { actions }, lyrics_visible: false, ..song(now) };
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
        let answer = |now, hovered| Inputs { voice: VoiceInput::Answer { actions: false }, hovered, lyrics_visible: false, ..song(now) };
        assert_eq!(m.step(&answer(0, false)), Mode::Expanded);
        assert_eq!(m.step(&answer(7_000, true)), Mode::Expanded);
        assert_eq!(m.step(&answer(30_000, true)), Mode::Expanded, "held while pointed at");
        assert_eq!(m.step(&answer(31_999, false)), Mode::Expanded);
        assert_eq!(m.step(&answer(32_000, false)), Mode::Hidden);
    }

    #[test]
    fn an_answer_returns_to_the_song_that_was_playing() {
        let mut m = Machine::default();
        assert_eq!(m.step(&song(0)), Mode::Compact);
        assert_eq!(m.step(&Inputs { voice: VoiceInput::Answer { actions: false }, ..song(1_000) }), Mode::Expanded);
        assert_eq!(m.step(&Inputs { voice: VoiceInput::Answer { actions: false }, ..song(9_000) }), Mode::Tab, "the track is old by now");
    }

    #[test]
    fn auto_shows_a_new_track_for_six_seconds_then_folds_into_the_tab() {
        let mut m = Machine::default();
        assert_eq!(m.step(&song(0)), Mode::Compact);
        assert_eq!(m.step(&song(5_999)), Mode::Compact);
        assert_eq!(m.step(&song(6_000)), Mode::Tab);
        // The next track opens it again.
        assert_eq!(m.step(&Inputs { track: Some("b"), ..song(20_000) }), Mode::Compact);
        assert_eq!(m.step(&Inputs { track: Some("b"), ..song(26_000) }), Mode::Tab);
    }

    #[test]
    fn pointing_at_the_tab_opens_it_and_it_folds_a_second_and_a_half_after_the_cursor_left() {
        let mut m = Machine::default();
        assert_eq!(m.step(&song(0)), Mode::Compact);
        assert_eq!(m.step(&song(10_000)), Mode::Tab);
        assert_eq!(m.step(&Inputs { hovered: true, ..song(10_100) }), Mode::Compact);
        assert_eq!(m.step(&Inputs { hovered: true, ..song(20_000) }), Mode::Compact);
        assert_eq!(m.step(&song(20_100)), Mode::Compact, "just left");
        assert_eq!(m.step(&song(21_599)), Mode::Compact);
        assert_eq!(m.step(&song(21_600)), Mode::Tab);
    }

    #[test]
    fn always_is_compact_and_never_folds_even_over_a_full_screen() {
        let mut m = Machine::default();
        for t in [0, 10_000, 100_000] {
            assert_eq!(m.step(&Inputs { lyrics_mode: PcOverlay::Always, ..song(t) }), Mode::Compact);
        }
        assert_eq!(m.step(&Inputs { lyrics_mode: PcOverlay::Always, fullscreen: true, ..song(1) }), Mode::Compact);
    }

    #[test]
    fn a_full_screen_program_hides_auto_but_not_the_voice() {
        let mut m = Machine::default();
        assert_eq!(m.step(&Inputs { fullscreen: true, ..song(0) }), Mode::Hidden);
        assert_eq!(m.step(&Inputs { fullscreen: true, voice: VoiceInput::Active, ..song(1) }), Mode::Expanded);
        assert_eq!(m.step(&Inputs { fullscreen: true, voice: VoiceInput::Answer { actions: false }, ..song(2) }), Mode::Expanded);
    }

    #[test]
    fn the_song_going_away_resets_so_the_next_one_opens_again() {
        let mut m = Machine::default();
        assert_eq!(m.step(&song(0)), Mode::Compact);
        assert_eq!(m.step(&song(10_000)), Mode::Tab);
        assert_eq!(m.step(&Inputs { lyrics_visible: false, ..song(11_000) }), Mode::Hidden);
        assert_eq!(m.step(&song(12_000)), Mode::Compact, "the same track again counts as new after a gap");
    }

    #[test]
    fn the_voice_payload_becomes_the_voice_of_the_event() {
        let none = voice_view(None, None, &[]);
        assert_eq!((none.0, none.1), (VoiceInput::Idle, None));
        let idle = json!({ "state": "idle" });
        assert_eq!(voice_view(Some(&idle), None, &[]).0, VoiceInput::Idle);
        let listening = json!({ "state": "listening", "level": 0.4, "id": "a1", "hotkey": "Ctrl+Alt+Space", "source": "pc" });
        let (input, v) = voice_view(Some(&listening), None, &[json!({ "label": "Translate", "command": "translate" })]);
        let v = v.unwrap();
        assert_eq!(input, VoiceInput::Active);
        assert_eq!((v["active"].clone(), v["phase"].clone(), v["level"].clone()), (json!(true), json!("listening"), json!(0.4)));
        assert!(v.get("confirmId").is_none() && v.get("hotkey").is_none() && v.get("source").is_none(), "only what the island shows");
        assert_eq!(v["chips"][0]["label"], "Translate", "chips are offered while it listens");
        let done = json!({ "state": "done", "reply": "It is about cats.", "actions": ["copy", "insert"], "replyKey": "voice.status.cpu", "replyArgs": { "pct": "23" } });
        let (input, v) = voice_view(Some(&done), None, &[json!({ "label": "x" })]);
        let v = v.unwrap();
        assert_eq!(input, VoiceInput::Answer { actions: true });
        assert_eq!((v["active"].clone(), v["actions"].clone(), v["replyArgs"]["pct"].clone()), (json!(false), json!(["copy", "insert"]), json!("23")));
        assert!(v.get("chips").is_none());
        let plain = json!({ "state": "error", "reply": "Nothing to continue" });
        assert_eq!(voice_view(Some(&plain), None, &[]).0, VoiceInput::Answer { actions: false });
        let confirm = json!({ "state": "confirm", "id": "c9", "confirm": { "text": "Lock the computer", "key": "voice.confirm.lock" } });
        assert_eq!(voice_view(Some(&confirm), None, &[]).1.unwrap()["confirmId"], "c9");
        let slow = voice_view(Some(&json!({ "state": "thinking" })), Some(Progress { eta_sec: 10, elapsed_sec: 3 }), &[]).1.unwrap();
        assert_eq!(slow["slow"], json!({ "etaSec": 10, "elapsedSec": 3 }));
    }
}