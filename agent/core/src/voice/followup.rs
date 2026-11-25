//! Continuing a dialogue (T26.3): for 30 seconds after an answer that was shown, the assistant remembers what was asked, the
//! text it worked on and the result - in memory only, never on disk - so that "shorter", "and in German", "another one",
//! "insert it", "copy", "read it aloud" refer to that answer. After 30 s, or after any other command, the context is gone.

use std::time::{Duration, Instant};

use super::textops::Op;

/// How long an answer can be continued.
pub const CONTEXT_TTL: Duration = Duration::from_secs(30);

/// What the PC window may offer under an answer (`actions` of the reply event); `lang` opens the list of languages.
pub const ACTIONS: [&str; 5] = ["copy", "insert", "speak", "again", "lang"];

/// The answer that can be continued.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub tool: &'static str,
    pub op: Op,
    /// The target language of a translation.
    pub lang: Option<String>,
    /// The text that was worked on (the selection or the said text).
    pub source_text: String,
    /// What was shown.
    pub result: String,
    /// The language of the speaker.
    pub ui_lang: String,
}

/// The one remembered answer and when it was given.
#[derive(Debug, Default)]
pub struct Memory {
    slot: Option<(Answer, Instant)>,
}

impl Memory {
    pub fn set(&mut self, answer: Answer, now: Instant) {
        self.slot = Some((answer, now));
    }

    pub fn clear(&mut self) {
        self.slot = None;
    }

    /// The answer, while it is younger than `CONTEXT_TTL`.
    pub fn fresh(&self, now: Instant) -> Option<&Answer> {
        self.slot.as_ref().filter(|(_, at)| now.saturating_duration_since(*at) <= CONTEXT_TTL).map(|(a, _)| a)
    }
}

/// What to do with the shown answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Copy,
    Insert,
    Speak,
    /// Another variant of the same operation on the same source.
    Again,
    /// The same text in another language (`lang:<code>`).
    Lang(String),
}

impl Kind {
    /// `copy`, `insert`, `speak`, `again`, `lang:<code>` (the code is one of the 11 languages).
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "copy" => Kind::Copy,
            "insert" => Kind::Insert,
            "speak" => Kind::Speak,
            "again" => Kind::Again,
            _ => {
                let code = s.strip_prefix("lang:")?;
                Kind::Lang(super::textops::LANGS.iter().find(|l| **l == code)?.to_string())
            }
        })
    }

    pub fn name(&self) -> String {
        match self {
            Kind::Copy => "copy".into(),
            Kind::Insert => "insert".into(),
            Kind::Speak => "speak".into(),
            Kind::Again => "again".into(),
            Kind::Lang(c) => format!("lang:{c}"),
        }
    }
}

/// The text a spoken continuation ("shorter", "more polite", "and in German") works on, or `None` when `op` is not a
/// continuation: a translation starts again from the source (better than a translation of a translation), the other
/// operations work on the result that was shown.
pub fn continued_text(answer: &Answer, op: Op) -> Option<String> {
    match op {
        Op::Translate => Some(if answer.op == Op::Translate { answer.source_text.clone() } else { answer.result.clone() }),
        Op::Shorter | Op::Polite | Op::Formal | Op::Casual => Some(answer.result.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer() -> Answer {
        Answer { tool: "text_transform", op: Op::Translate, lang: Some("zh".into()), source_text: "спасибо за помощь".into(), result: "谢谢你的帮助".into(), ui_lang: "ru".into() }
    }

    #[test]
    fn the_context_lives_thirty_seconds_and_a_clear_ends_it() {
        let t0 = Instant::now();
        let mut m = Memory::default();
        assert!(m.fresh(t0).is_none());
        m.set(answer(), t0);
        assert!(m.fresh(t0 + Duration::from_secs(29)).is_some());
        assert!(m.fresh(t0 + CONTEXT_TTL).is_some());
        assert!(m.fresh(t0 + CONTEXT_TTL + Duration::from_millis(1)).is_none(), "after 30 s: a plain request");
        m.clear();
        assert!(m.fresh(t0).is_none(), "another command resets it");
    }

    #[test]
    fn a_translation_continues_from_the_source_and_a_rewrite_from_the_result() {
        let a = answer();
        assert_eq!(continued_text(&a, Op::Translate).as_deref(), Some("спасибо за помощь"));
        assert_eq!(continued_text(&a, Op::Shorter).as_deref(), Some("谢谢你的帮助"));
        let reply = Answer { op: Op::Reply, ..answer() };
        assert_eq!(continued_text(&reply, Op::Translate).as_deref(), Some("谢谢你的帮助"));
        assert_eq!(continued_text(&reply, Op::Polite).as_deref(), Some("谢谢你的帮助"));
        assert_eq!(continued_text(&reply, Op::Summarize), None, "not a continuation");
        assert_eq!(continued_text(&reply, Op::Fix), None);
    }

    #[test]
    fn kinds_are_a_closed_list_and_languages_are_the_eleven() {
        for k in ["copy", "insert", "speak", "again", "lang:de", "lang:kk"] {
            assert_eq!(Kind::parse(k).unwrap().name(), k);
        }
        for bad in ["", "lang", "lang:xx", "lang:", "run", "COPY", "lang:de;calc"] {
            assert_eq!(Kind::parse(bad), None, "{bad}");
        }
        assert!(ACTIONS.contains(&"insert"));
    }
}
