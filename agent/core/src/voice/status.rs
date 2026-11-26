//! `status` (T26.1): facts about this PC and its widgets, answered from the sections the agent already keeps (the same store
//! `brief.rs` reads). The answer is a fixed template with numbers (`voice.status.<topic>`), never written by the model, and
//! never fetched from the internet: no section, or the widget is off, means a `voice.status.none.<topic>` answer.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::assistant::{Local, Reminder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Weather,
    Cpu,
    Ram,
    Disk,
    Network,
    Battery,
    Media,
    AiLimits,
    Focus,
    NextReminder,
}

/// Names of the topics as the model and the phrase tables spell them (the `enum` of the tool schema).
pub const TOPICS: &[&str] = &["weather", "cpu", "ram", "disk", "network", "battery", "media", "ai_limits", "focus", "next_reminder"];

impl Topic {
    pub fn parse(name: &str) -> Option<Topic> {
        Some(match name {
            "weather" => Topic::Weather,
            "cpu" => Topic::Cpu,
            "ram" => Topic::Ram,
            "disk" => Topic::Disk,
            "network" => Topic::Network,
            "battery" => Topic::Battery,
            "media" => Topic::Media,
            "ai_limits" => Topic::AiLimits,
            "focus" => Topic::Focus,
            "next_reminder" => Topic::NextReminder,
            _ => return None,
        })
    }

    /// (answer key, "no data" key) in `shared/src/i18n`.
    fn keys(self) -> (&'static str, &'static str) {
        match self {
            Topic::Weather => ("voice.status.weather", "voice.status.none.weather"),
            Topic::Cpu => ("voice.status.cpu", "voice.status.none.cpu"),
            Topic::Ram => ("voice.status.ram", "voice.status.none.ram"),
            Topic::Disk => ("voice.status.disk", "voice.status.none.disk"),
            Topic::Network => ("voice.status.network", "voice.status.none.network"),
            Topic::Battery => ("voice.status.battery", "voice.status.none.battery"),
            Topic::Media => ("voice.status.media", "voice.status.none.media"),
            Topic::AiLimits => ("voice.status.ai_limits", "voice.status.none.ai_limits"),
            Topic::Focus => ("voice.status.focus", "voice.status.none.focus"),
            Topic::NextReminder => ("voice.status.next_reminder", "voice.status.none.next_reminder"),
        }
    }
}

/// What the answer reads: the sections of the store, as they are.
#[derive(Debug, Clone, Default)]
pub struct Sections {
    pub weather: Value,
    pub sys: Value,
    pub battery: Value,
    pub media: Value,
    pub ai: Value,
    pub focus: Value,
    pub reminder: Option<Reminder>,
}

/// An answer: the i18n key with its arguments, and the English text (the fallback of the key).
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub key: &'static str,
    pub args: BTreeMap<String, String>,
    pub text: String,
}

/// Words that introduce a place ("weather in Paris"), per language; the word after one is the place.
const PLACE_PREPOSITIONS: &[&str] = &["in", "at", "for", "в", "во", "для", "für", "à", "au", "pour", "en", "em", "no", "na", "para", "في", "在"];
const PLACE_POSTPOSITIONS: &[&str] = &["में", "पर", "के", "の", "で", "に", "的", "үшін", "бойынша"];
const PLACE_SUFFIXES: &[&str] = &[];
const PLACE_IGNORE: &[&str] = &[
    "the", "a", "my", "this", "now", "today", "tomorrow", "here", "outside",
    "сейчас", "сегодня", "завтра", "моём", "моем", "этом", "улице", "городе",
    "der", "die", "das", "heute", "morgen", "jetzt", "meiner", "dieser",
    "le", "la", "mon", "ce", "maintenant", "aujourdhui", "demain", "dehors",
    "el", "mi", "este", "hoy", "ahora", "mañana", "afuera",
    "o", "meu", "hoje", "agora", "amanhã", "fora",
    "اليوم", "الآن", "غدا", "الخارج", "هذا",
    "आज", "अभी", "कल", "बाहर", "यहाँ", "इस",
    "今", "日", "今日", "明日", "ここ", "外",
    "今", "天", "现", "在", "明", "这",
    "бүгін", "қазір", "ертең", "осында", "сыртта", "дала", "далада", "ауа", "райы", "ауада",
];

fn names_other_place_core<S: AsRef<str>>(
    phrase: &str,
    configured: Option<&str>,
    prepositions: &[S],
    postpositions: &[S],
    suffixes: &[S],
    ignore: &[S],
) -> bool {
    let toks = super::matcher::tokens(phrase);
    let is_ignored = |w: &str| ignore.iter().any(|x| x.as_ref() == w);
    let matches_cfg = |word: &str| {
        configured.is_some_and(|place| {
            super::matcher::tokens(place)
                .iter()
                .any(|p| super::matcher::trigram_sim(p, word) >= 0.5 || p.starts_with(word) || word.starts_with(p.as_str()))
        })
    };

    for (i, t) in toks.iter().enumerate() {
        if prepositions.iter().any(|p| p.as_ref() == t) {
            if let Some(word) = toks.get(i + 1) {
                if !is_ignored(word) && !matches_cfg(word) {
                    return true;
                }
            }
        }
        if i > 0 && postpositions.iter().any(|p| p.as_ref() == t) {
            let word = &toks[i - 1];
            if !is_ignored(word) && !matches_cfg(word) {
                return true;
            }
        }
    }

    for tok in &toks {
        if is_ignored(tok) {
            continue;
        }
        let total = tok.chars().count();
        let stem = suffixes
            .iter()
            .map(AsRef::as_ref)
            .filter(|s| !s.is_empty() && tok.ends_with(s) && total.saturating_sub(s.chars().count()) >= 3)
            .max_by_key(|s| s.chars().count())
            .map(|s| &tok[..tok.len() - s.len()]);
        if let Some(stem) = stem {
            if !is_ignored(stem) && !matches_cfg(stem) {
                return true;
            }
        }
    }

    false
}

/// Whether a weather phrase names a place other than `configured` (the widget's city): "погода в Париже" with Almaty set.
/// A place equal to the configured one (up to a case ending) or no place at all is `false`.
pub fn names_other_place(phrase: &str, configured: Option<&str>) -> bool {
    names_other_place_core(phrase, configured, PLACE_PREPOSITIONS, PLACE_POSTPOSITIONS, PLACE_SUFFIXES, PLACE_IGNORE)
}

/// Per-table version of `names_other_place` backed by `shared/voice/*.json`.
pub fn names_other_place_with(
    phrase: &str,
    configured: Option<&str>,
    prepositions: &[String],
    postpositions: &[String],
    suffixes: &[String],
    ignore: &[String],
) -> bool {
    names_other_place_core(phrase, configured, prepositions, postpositions, suffixes, ignore)
}

/// Languages that write the decimal comma.
fn decimal_comma(lang: &str) -> bool {
    matches!(lang, "ru" | "de" | "fr" | "es" | "pt" | "kk")
}

/// One decimal, or none for a whole number: `11,2` / `16`.
fn num(v: f64, lang: &str) -> String {
    let s = if (v - v.round()).abs() < 0.05 { format!("{}", v.round() as i64) } else { format!("{v:.1}") };
    if decimal_comma(lang) { s.replace('.', ",") } else { s }
}

fn f(v: &Value, pointer: &str) -> Option<f64> {
    v.pointer(pointer)?.as_f64()
}

fn answer(key: &'static str, pairs: &[(&str, String)], text: String) -> Answer {
    Answer { key, args: pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(), text }
}

/// The answer to `topic`. `lang` only picks the decimal sign; the words come from the i18n template.
pub fn answer_for(topic: Topic, s: &Sections, lang: &str, now: &Local) -> Answer {
    let (key, none_key) = topic.keys();
    let none = |text: &str| Answer { key: none_key, args: BTreeMap::new(), text: text.to_string() };
    match topic {
        Topic::Weather => {
            let (Some(place), Some(temp)) = (s.weather.get("place").and_then(Value::as_str), f(&s.weather, "/current/temp")) else {
                return none("Weather is not set up - Settings > Weather");
            };
            let unit = if s.weather.get("units").and_then(Value::as_str) == Some("imperial") { "F" } else { "C" };
            let (min, max) = (f(&s.weather, "/daily/0/min"), f(&s.weather, "/daily/0/max"));
            let (min, max) = (min.map_or("-".into(), |v| num(v, lang)), max.map_or("-".into(), |v| num(v, lang)));
            let rain = s.weather.get("hourly").and_then(Value::as_array).and_then(|hours| {
                hours.iter().find_map(|h| {
                    let wet = matches!(h.get("code")?.as_i64()?, 51..=67 | 71..=77 | 80..=86 | 95..=99);
                    wet.then(|| h.get("time")?.as_str().map(str::to_string)).flatten()
                })
            });
            let base = format!("{place}: now {}\u{b0}{unit}, today {min}...{max}\u{b0}{unit}", num(temp, lang));
            let mut pairs = vec![("place", place.to_string()), ("temp", num(temp, lang)), ("unit", unit.to_string()), ("min", min), ("max", max)];
            match rain {
                Some(time) => {
                    pairs.push(("time", time.clone()));
                    answer("voice.status.weather_precip", &pairs, format!("{base}. Precipitation expected around {time}"))
                }
                None => answer(key, &pairs, base),
            }
        }
        Topic::Cpu => match f(&s.sys, "/cpu") {
            Some(c) => answer(key, &[("pct", num(c, lang))], format!("Processor: {}% busy", num(c, lang))),
            None => none("No data on the processor yet"),
        },
        Topic::Ram => match (f(&s.sys, "/mem/usedMb"), f(&s.sys, "/mem/totalMb")) {
            (Some(used), Some(total)) if total > 0.0 => {
                let (u, t, free) = (used / 1024.0, total / 1024.0, (total - used).max(0.0) / 1024.0);
                let pairs = [("used", num(u, lang)), ("total", num(t, lang)), ("free", num(free, lang))];
                answer(key, &pairs, format!("RAM: {} of {} GB used, {} GB free", pairs[0].1, pairs[1].1, pairs[2].1))
            }
            _ => none("No data on memory yet"),
        },
        Topic::Disk => {
            let disks = s.sys.get("disks").and_then(Value::as_array);
            // The system disk first, then whatever comes first.
            let disk = disks.and_then(|d| d.iter().find(|x| x.get("mount").and_then(Value::as_str).is_some_and(|m| m.to_ascii_uppercase().starts_with('C'))).or_else(|| d.first()));
            match disk.and_then(|d| Some((d.get("mount")?.as_str()?, d.get("usedGb")?.as_f64()?, d.get("totalGb")?.as_f64()?))) {
                Some((mount, used, total)) if total > 0.0 => {
                    let pairs = [("disk", mount.to_string()), ("used", num(used, lang)), ("total", num(total, lang)), ("free", num((total - used).max(0.0), lang))];
                    answer(key, &pairs, format!("Disk {}: {} of {} GB used, {} GB free", pairs[0].1, pairs[1].1, pairs[2].1, pairs[3].1))
                }
                _ => none("No data on disks yet"),
            }
        }
        Topic::Network => match (f(&s.sys, "/net/rxKbps"), f(&s.sys, "/net/txKbps")) {
            (Some(rx), Some(tx)) => {
                // Kbit/s -> Mbit/s with one decimal.
                let (rx, tx) = (num(rx / 1000.0, lang), num(tx / 1000.0, lang));
                answer(key, &[("rx", rx.clone()), ("tx", tx.clone())], format!("Network: {rx} Mbps down, {tx} Mbps up"))
            }
            _ => none("No data on the network yet"),
        },
        Topic::Battery => {
            let items: Vec<String> = s
                .battery
                .get("devices")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|d| Some(format!("{} {}%", d.get("name")?.as_str()?, d.get("pct")?.as_u64()?)))
                .take(3)
                .collect();
            if items.is_empty() {
                return none("No battery data - Settings > Battery");
            }
            let list = items.join(", ");
            answer(key, &[("devices", list.clone())], format!("Battery: {list}"))
        }
        Topic::Media => {
            let title = s.media.get("title").and_then(Value::as_str).filter(|t| !t.is_empty());
            let Some(title) = title else { return none("Nothing is playing") };
            let artist = s.media.get("artist").and_then(Value::as_str).filter(|a| !a.is_empty());
            let track = artist.map_or(title.to_string(), |a| format!("{a} - {title}"));
            answer(key, &[("track", track.clone())], format!("Playing: {track}"))
        }
        Topic::AiLimits => {
            let mut parts = Vec::new();
            for (k, name) in [("claude", "Claude"), ("codex", "Codex")] {
                let wins: Vec<String> = s
                    .ai
                    .get(k)
                    .and_then(|p| p.get("windows"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|w| Some(format!("{} {}%", w.get("label")?.as_str()?, num(w.get("pct")?.as_f64()?, lang))))
                    .collect();
                if !wins.is_empty() {
                    parts.push(format!("{name} {}", wins.join(", ")));
                }
            }
            if parts.is_empty() {
                return none("No AI limits data");
            }
            let limits = parts.join("; ");
            answer(key, &[("limits", limits.clone())], format!("AI limits used: {limits}"))
        }
        Topic::Focus => match s.focus.get("app").and_then(Value::as_str).filter(|a| !a.is_empty()) {
            Some(app) => answer(key, &[("app", app.to_string())], format!("Active program: {app}")),
            None => none("No active program known"),
        },
        Topic::NextReminder => match &s.reminder {
            Some(r) => {
                // Today: the time only; another day: the date too.
                let at = if r.at.starts_with(&now.iso()[..10]) { r.at.get(11..).unwrap_or(&r.at).to_string() } else { r.at.replace('T', " ") };
                answer(key, &[("text", r.text.clone()), ("at", at.clone())], format!("Next reminder: {} at {at}", r.text))
            }
            None => none("No reminders"),
        },
    }
}

// ---- Carrying the key through the executor --------------------------------------------------------------------------

/// The result of an executor is a string; an answer with a key travels as `\u{1}key\u{1}json-args\u{1}text`, which no other
/// executor writes. The pipeline turns it back into a `ReplyKey`.
pub fn encode(a: &Answer) -> String {
    format!("\u{1}{}\u{1}{}\u{1}{}", a.key, serde_json::to_string(&a.args).unwrap_or_else(|_| "{}".into()), a.text)
}

/// The static key, the arguments and the text of an encoded answer; `None` for any other result.
pub fn decode(s: &str) -> Option<(&'static str, BTreeMap<String, String>, String)> {
    let rest = s.strip_prefix('\u{1}')?;
    let mut parts = rest.splitn(3, '\u{1}');
    let (key, args, text) = (parts.next()?, parts.next()?, parts.next()?);
    let key = ALL_KEYS.iter().find(|k| **k == key)?;
    Some((key, serde_json::from_str(args).ok()?, text.to_string()))
}

const ALL_KEYS: &[&str] = &[
    "voice.reply.button_added",
    "voice.reply.screenshot",
    "voice.reply.shot_area",
    "voice.reply.theme_light",
    "voice.reply.theme_dark",
    "voice.reply.theme_decka",
    "voice.reply.setup_button",
    "voice.status.weather",
    "voice.status.weather_precip",
    "voice.status.cpu",
    "voice.status.ram",
    "voice.status.disk",
    "voice.status.network",
    "voice.status.battery",
    "voice.status.media",
    "voice.status.ai_limits",
    "voice.status.focus",
    "voice.status.next_reminder",
    "voice.status.none.weather",
    "voice.status.none.cpu",
    "voice.status.none.ram",
    "voice.status.none.disk",
    "voice.status.none.network",
    "voice.status.none.battery",
    "voice.status.none.media",
    "voice.status.none.ai_limits",
    "voice.status.none.focus",
    "voice.status.none.next_reminder",
];

/// Every i18n key this module can produce (checked against the dictionaries by a test of the shared package).
pub fn all_keys() -> &'static [&'static str] {
    ALL_KEYS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> Local {
        Local::new(2026, 11, 25, 14, 0).unwrap()
    }

    #[test]
    fn other_place_is_not_the_widget_place() {
        assert!(names_other_place("погода в Париже", Some("Almaty")));
        assert!(names_other_place("what is the weather in Paris", Some("Almaty")));
        assert!(!names_other_place("какая погода", Some("Almaty")));
        assert!(!names_other_place("погода в Алматы", Some("Алматы")));
        assert!(!names_other_place("weather in Almaty", Some("Almaty")));
        assert!(!names_other_place("погода сегодня", Some("Almaty")));
    }

    fn full() -> Sections {
        Sections {
            weather: json!({ "place": "Almaty", "units": "metric", "current": { "temp": 7, "code": 3 }, "daily": [{ "min": 2, "max": 9 }],
                "hourly": [{ "time": "15:00", "code": 3 }, { "time": "17:00", "code": 61 }] }),
            sys: json!({ "cpu": 23, "mem": { "usedMb": 11469, "totalMb": 16384 }, "disks": [{ "mount": "D:", "usedGb": 100, "totalGb": 500 }, { "mount": "C:", "usedGb": 210.5, "totalGb": 476 }],
                "net": { "rxKbps": 12500, "txKbps": 800 } }),
            battery: json!({ "devices": [{ "name": "Laptop", "pct": 90, "kind": "pc" }, { "name": "Pixel 7", "pct": 64, "kind": "bt" }] }),
            media: json!({ "title": "Song", "artist": "Band", "playing": true }),
            ai: json!({ "claude": { "windows": [{ "label": "5h", "pct": 42.0 }, { "label": "7d", "pct": 10.5 }] } }),
            focus: json!({ "app": "code.exe" }),
            reminder: Some(Reminder { id: "a".into(), text: "call mom".into(), at: "2026-11-25T18:30".into(), done: false, repeat: None }),
        }
    }

    #[test]
    fn every_topic_is_answered_from_the_store() {
        let s = full();
        let a = |t: Topic, lang: &str| answer_for(t, &s, lang, &now());
        assert_eq!(a(Topic::Cpu, "en").text, "Processor: 23% busy");
        let ram = a(Topic::Ram, "ru");
        assert_eq!(ram.key, "voice.status.ram");
        assert_eq!((ram.args["used"].as_str(), ram.args["total"].as_str(), ram.args["free"].as_str()), ("11,2", "16", "4,8"), "decimal comma in Russian");
        assert_eq!(a(Topic::Ram, "en").text, "RAM: 11.2 of 16 GB used, 4.8 GB free");
        let disk = a(Topic::Disk, "en");
        assert_eq!((disk.args["disk"].as_str(), disk.args["used"].as_str()), ("C:", "210.5"), "the system disk first");
        assert_eq!(a(Topic::Network, "en").text, "Network: 12.5 Mbps down, 0.8 Mbps up");
        assert_eq!(a(Topic::Battery, "en").text, "Battery: Laptop 90%, Pixel 7 64%");
        assert_eq!(a(Topic::Media, "en").text, "Playing: Band - Song");
        assert_eq!(a(Topic::AiLimits, "en").text, "AI limits used: Claude 5h 42%, 7d 10.5%");
        assert_eq!(a(Topic::Focus, "en").text, "Active program: code.exe");
        assert_eq!(a(Topic::NextReminder, "en").text, "Next reminder: call mom at 18:30");
        let w = a(Topic::Weather, "en");
        assert_eq!((w.key, w.args["time"].as_str()), ("voice.status.weather_precip", "17:00"));
        assert!(w.text.starts_with("Almaty: now 7\u{b0}C, today 2...9\u{b0}C"));
    }

    #[test]
    fn a_dry_forecast_has_no_precipitation_part() {
        let mut s = full();
        s.weather["hourly"] = json!([{ "time": "15:00", "code": 3 }]);
        assert_eq!(answer_for(Topic::Weather, &s, "en", &now()).key, "voice.status.weather");
    }

    #[test]
    fn an_empty_store_gives_the_none_answers_for_every_topic() {
        for name in TOPICS {
            let t = Topic::parse(name).unwrap();
            let a = answer_for(t, &Sections::default(), "en", &now());
            assert_eq!(a.key, format!("voice.status.none.{name}"), "{name}");
            assert!(a.args.is_empty() && !a.text.is_empty());
        }
        assert!(Topic::parse("web").is_none());
    }

    #[test]
    fn another_day_shows_the_date_and_keys_survive_the_executor() {
        let mut s = full();
        s.reminder.as_mut().unwrap().at = "2026-11-27T09:00".into();
        let a = answer_for(Topic::NextReminder, &s, "en", &now());
        assert_eq!(a.args["at"], "2026-11-27 09:00");
        let (key, args, text) = decode(&encode(&a)).unwrap();
        assert_eq!((key, args, text), (a.key, a.args.clone(), a.text.clone()));
        assert!(decode("Opened Calculator").is_none());
        assert!(decode("\u{1}voice.status.unknown\u{1}{}\u{1}x").is_none(), "only known keys");
    }

    #[test]
    fn every_key_the_module_makes_is_listed() {
        for name in TOPICS {
            let (k, n) = Topic::parse(name).unwrap().keys();
            assert!(all_keys().contains(&k) && all_keys().contains(&n), "{name}");
        }
        assert!(all_keys().contains(&"voice.status.weather_precip"));
    }
}
