//! The time of a reminder out of a spoken phrase (DATA_MODEL.md 13.4), worked out here and not by the model: whisper writes
//! `в 22.14`, `в 2215`, `22:14`, `в 10 вечера`, `через 5 минут` in whatever way it likes, and a small model guessed wrong
//! (`в 2215` became 22:25, `в 22.14` became 23:53). The phrase of the time is cut out of the text of the reminder.
//! Numbers and day parts are understood in Russian and English; clock forms with digits in every language.

use serde_json::{json, Map, Value};

use super::matcher::{number_words, Intent};
use crate::assistant::{Local, Repeat};

/// Words that stand in front of a time ("в 22:15", "at 9").
const PREPS: &[&str] = &["в", "во", "к", "на", "at", "by", "a", "à", "um"];
/// Words in front of the text of the reminder that are not a part of it ("напомни мне что нужно ...", "remind me to ...").
const FILLERS: &[&str] = &["что", "чтобы", "to", "that", "нужно", "надо", "need"];
/// How many words may stand in front of the lead-in ("в 22:15 напомни ...").
const MAX_LEAD_AT: usize = 6;

fn norm(word: &str) -> String {
    word.chars().filter(|c| !matches!(c, '\'' | '\u{2019}')).flat_map(char::to_lowercase).collect::<String>().trim_matches(|c: char| !c.is_alphanumeric()).to_string()
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// A number written in digits or as a Russian/English word ("десять", "five").
fn number(word: &str) -> Option<u32> {
    if digits(word) && word.len() <= 6 {
        return word.parse().ok();
    }
    match number_words(vec![word.to_string()]).as_slice() {
        [n] if digits(n) => n.parse().ok(),
        _ => None,
    }
}

fn clock(h: u32, m: u32) -> Option<(u32, u32)> {
    (h < 24 && m < 60).then_some((h, m))
}

/// `22:14`, `22.14`, `22,14` -> (22, 14); `2215` and `930` -> (22, 15), (9, 30).
fn clock_word(word: &str, with_prep: bool) -> Option<(u32, u32)> {
    if let Some((h, m)) = word.split_once(':') {
        return (digits(h) && h.len() <= 2 && digits(m) && m.len() == 2).then(|| clock(h.parse().ok()?, m.parse().ok()?)).flatten();
    }
    // A dot or a comma may be a decimal separator: only after "в" / "at" it is a time.
    if let Some((h, m)) = word.split_once(['.', ',']) {
        return (with_prep && digits(h) && h.len() <= 2 && digits(m) && m.len() == 2).then(|| clock(h.parse().ok()?, m.parse().ok()?)).flatten();
    }
    if with_prep && digits(word) && (3..=4).contains(&word.len()) {
        let (h, m) = word.split_at(word.len() - 2);
        return clock(h.parse().ok()?, m.parse().ok()?);
    }
    None
}

/// The hour after a part of the day: `10 вечера` is 22, `12 ночи` is 0, `3 дня` is 15, `7 утра` is 7.
fn day_part(hour: u32, part: &str) -> Option<u32> {
    if hour > 23 {
        return None;
    }
    Some(match part {
        "утра" | "утром" | "am" | "a.m" => {
            if hour == 12 { 0 } else { hour }
        }
        "вечера" | "вечером" | "pm" | "p.m" => {
            if hour < 12 { hour + 12 } else { hour }
        }
        "дня" | "днем" | "днём" => {
            if hour < 12 && hour >= 1 { hour + 12 } else { hour }
        }
        "ночи" | "ночью" => match hour {
            12 => 0,
            10 | 11 => hour + 12,
            _ => hour,
        },
        // "в 10 часов" is only the hour.
        "часов" | "часа" | "час" | "oclock" => hour,
        _ => return None,
    })
}

fn weekday(word: &str) -> Option<&'static str> {
    Some(match word {
        "понедельник" | "monday" => "mon",
        "вторник" | "tuesday" => "tue",
        "среду" | "среда" | "wednesday" => "wed",
        "четверг" | "thursday" => "thu",
        "пятницу" | "пятница" | "friday" => "fri",
        "субботу" | "суббота" | "saturday" => "sat",
        "воскресенье" | "sunday" => "sun",
        _ => return None,
    })
}

#[derive(Debug, PartialEq)]
enum Found {
    /// Minutes from now.
    In(u32),
    /// A time of the day, and a day when it was said.
    At { hour: u32, minute: u32, day: Option<&'static str> },
}

fn minutes_unit(word: &str) -> Option<u32> {
    match word {
        "минуту" | "минуты" | "минут" | "мин" | "minute" | "minutes" | "min" | "mins" => Some(1),
        "час" | "часа" | "часов" | "hour" | "hours" | "hr" | "hrs" => Some(60),
        _ => None,
    }
}

/// The time of the phrase and the indices of the words it was said with.
fn find(words: &[String]) -> Option<(Found, Vec<usize>)> {
    // Relative: "через 5 минут", "in 2 hours", "через час", "через полчаса", "in half an hour".
    for (i, w) in words.iter().enumerate() {
        if !matches!(w.as_str(), "через" | "in") {
            continue;
        }
        let at = |k: usize| words.get(i + k).map(String::as_str);
        let found = match (at(1), at(2), at(3)) {
            (Some("час"), _, _) | (Some("полчаса"), _, _) | (Some("an"), Some("hour"), _) | (Some("half"), Some("an"), Some("hour")) | (Some("a"), Some("half"), _) => {
                let half = matches!(at(1), Some("полчаса" | "half")) || at(2) == Some("half");
                let len = match (at(1), at(2)) {
                    (Some("час" | "полчаса"), _) => 1,
                    (Some("an"), _) => 2,
                    (Some("half"), _) => 3,
                    _ => 0,
                };
                (len > 0).then_some((if half { 30 } else { 60 }, len))
            }
            (Some(n), Some(unit), _) => number(n).zip(minutes_unit(unit)).map(|(n, u)| (n * u, 2)),
            _ => None,
        };
        if let Some((minutes, len)) = found.filter(|(m, _)| (1..=525_600).contains(m)) {
            return Some((Found::In(minutes), (i..=i + len).collect()));
        }
    }
    // A day word that goes with a time of the day.
    let day_at = words.iter().enumerate().find_map(|(i, w)| match w.as_str() {
        "завтра" | "tomorrow" => Some((i, "tomorrow")),
        "сегодня" | "today" => Some((i, "today")),
        other => weekday(other).map(|d| (i, d)),
    });
    for (i, w) in words.iter().enumerate() {
        let prep = i > 0 && PREPS.contains(&words[i - 1].as_str());
        let mut used = vec![i];
        let time = if let Some(hm) = clock_word(w, prep) {
            Some(hm)
        } else if let (true, Some(h), Some(m)) = (prep, number(w).filter(|_| digits(w) && w.len() <= 2), words.get(i + 1).filter(|m| digits(m) && m.len() == 2).and_then(|m| m.parse::<u32>().ok())) {
            // "в 22 14": the hour and the minutes as two numbers.
            clock(h, m).map(|hm| {
                used.push(i + 1);
                hm
            })
        } else {
            // "в 10 вечера", "в десять часов", "at 5 pm".
            number(w).zip(words.get(i + 1)).and_then(|(h, part)| day_part(h, part)).and_then(|h| {
                used.push(i + 1);
                clock(h, 0)
            })
        };
        let Some((hour, minute)) = time else { continue };
        if prep {
            used.push(i - 1);
        }
        let mut day = None;
        if let Some((d, name)) = day_at {
            day = Some(name);
            used.push(d);
            // "на завтра", "в пятницу", "on friday".
            if d > 0 && matches!(words[d - 1].as_str(), "на" | "в" | "во" | "on") && !used.contains(&(d - 1)) {
                used.push(d - 1);
            }
        }
        used.sort_unstable();
        used.dedup();
        return Some((Found::At { hour, minute, day }, used));
    }
    None
}

/// A series said in words: (how it repeats, the weekday of a weekly one, the indices of its words).
fn find_repeat(words: &[String]) -> Option<(Repeat, Option<&'static str>, Vec<usize>)> {
    for i in 0..words.len() {
        let w = words[i].as_str();
        let next = words.get(i + 1).map(String::as_str);
        let after = words.get(i + 2).map(String::as_str);
        match (w, next) {
            ("ежедневно" | "daily" | "everyday", _) => return Some((Repeat::Daily, None, vec![i])),
            ("каждый" | "every", Some("день" | "day")) => return Some((Repeat::Daily, None, vec![i, i + 1])),
            ("every", Some("weekday")) => return Some((Repeat::Weekdays, None, vec![i, i + 1])),
            ("по", Some("будням")) | ("в", Some("будни")) | ("on", Some("weekdays")) => return Some((Repeat::Weekdays, None, vec![i, i + 1])),
            ("по", Some("рабочим")) if after == Some("дням") => return Some((Repeat::Weekdays, None, vec![i, i + 1, i + 2])),
            ("weekdays", _) => return Some((Repeat::Weekdays, None, vec![i])),
            ("каждый" | "каждую" | "каждое" | "every", Some(d)) => {
                if let Some(day) = weekday(d) {
                    return Some((Repeat::Weekly, Some(day), vec![i, i + 1]));
                }
            }
            _ => {}
        }
    }
    None
}

/// The reminder of a phrase that has a lead-in word (one of `leads`, each a list of normalised words) and a time: the
/// `remind` call with the text cut free of the lead-in and the time. `None` when there is no lead-in, no time or no text.
pub fn remind_intent(phrase: &str, leads: &[Vec<String>], now: &Local) -> Option<Intent> {
    let raw: Vec<&str> = phrase.split_whitespace().collect();
    let words: Vec<String> = raw.iter().map(|w| norm(w)).collect();
    // The first lead-in (the longest of those at the same place).
    let (at, len) = (0..words.len().min(MAX_LEAD_AT + 1))
        .find_map(|p| leads.iter().filter(|l| !l.is_empty() && words.len() >= p + l.len() && words[p..p + l.len()] == l[..]).map(|l| (p, l.len())).max_by_key(|(_, n)| *n))?;
    let keep: Vec<usize> = (0..words.len()).filter(|i| *i < at || *i >= at + len).collect();
    let rest: Vec<String> = keep.iter().map(|i| words[*i].clone()).collect();
    // "every day", "on weekdays", "every Monday": a series. Its words are cut first, the time is looked for in the others.
    let series = find_repeat(&rest);
    let (cut, outside): (Vec<usize>, Vec<usize>) = match &series {
        Some((_, _, used)) => (used.clone(), (0..rest.len()).filter(|i| !used.contains(i)).collect()),
        None => (Vec::new(), (0..rest.len()).collect()),
    };
    let rest_free: Vec<String> = outside.iter().map(|i| rest[*i].clone()).collect();
    let (found, used_free) = find(&rest_free)?;
    // The indices of `rest_free` back to those of `rest`.
    let mut used: Vec<usize> = used_free.iter().map(|i| outside[*i]).collect();
    used.extend(cut);
    let mut text: Vec<&str> = keep.iter().enumerate().filter(|(k, _)| !used.contains(k)).map(|(_, i)| raw[*i]).collect();
    while text.first().is_some_and(|w| FILLERS.contains(&norm(w).as_str())) {
        text.remove(0);
    }
    let text = text.join(" ").trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | '-' | ' ' | '!')).to_string();
    if text.is_empty() || text.chars().count() > crate::assistant::MAX_TEXT {
        return None;
    }
    let mut args = Map::new();
    args.insert("text".into(), Value::String(text));
    if let Some((repeat, weekday, _)) = series {
        // A series needs a clock time and no other day; `weekly` carries its weekday.
        let Found::At { hour, minute, day: None } = found else { return None };
        args.insert("repeat".into(), json!(repeat.as_str()));
        args.insert("time".into(), json!(format!("{hour:02}:{minute:02}")));
        if let Some(d) = weekday {
            args.insert("day".into(), json!(d));
        }
        return Some(Intent { tool: "remind".into(), args });
    }
    match found {
        Found::In(m) => {
            args.insert("inMinutes".into(), json!(m));
        }
        Found::At { hour, minute, day } => {
            // A time without a day is the next such time: later today, else tomorrow.
            let day = day.unwrap_or(if (hour * 60 + minute) as i64 > now.minutes().rem_euclid(1440) { "today" } else { "tomorrow" });
            args.insert("day".into(), json!(day));
            args.insert("time".into(), json!(format!("{hour:02}:{minute:02}")));
        }
    }
    Some(Intent { tool: "remind".into(), args })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leads() -> Vec<Vec<String>> {
        [&["напомни", "мне"][..], &["напомни"], &["remind", "me", "to"], &["remind", "me"]].iter().map(|l| l.iter().map(|s| s.to_string()).collect()).collect()
    }

    /// 2026-11-24 21:00, a Tuesday.
    fn evening() -> Local {
        Local::new(2026, 11, 24, 21, 0).unwrap()
    }

    fn call(phrase: &str) -> Option<(String, Value)> {
        remind_intent(phrase, &leads(), &evening()).map(|i| (i.args["text"].as_str().unwrap().to_string(), json!({ "day": i.args.get("day"), "time": i.args.get("time"), "in": i.args.get("inMinutes") })))
    }

    fn at(text: &str, day: &str, time: &str) -> Option<(String, Value)> {
        Some((text.to_string(), json!({ "day": day, "time": time, "in": null })))
    }

    #[test]
    fn the_strings_of_the_owner_report() {
        // "в 2215 сделать дюзен" was saved at 22:25, "в 22.14 сделать дизайн" at 23:53.
        assert_eq!(call("напомни мне в 2215 сделать дюзен"), at("сделать дюзен", "today", "22:15"));
        assert_eq!(call("Напомни в 22.14 сделать дизайн"), at("сделать дизайн", "today", "22:14"));
        assert_eq!(call("напомни мне в 22:14 сделать дизайн"), at("сделать дизайн", "today", "22:14"));
        assert_eq!(call("напомни сделать дизайн в 22,14"), at("сделать дизайн", "today", "22:14"));
        assert_eq!(call("в 22 14 напомни сделать дизайн"), at("сделать дизайн", "today", "22:14"));
    }

    #[test]
    fn day_parts_and_relative_times() {
        assert_eq!(call("напомни мне в 10 вечера позвонить маме"), at("позвонить маме", "today", "22:00"));
        assert_eq!(call("напомни мне в десять вечера позвонить маме"), at("позвонить маме", "today", "22:00"));
        assert_eq!(call("напомни в 7 утра выпить воду"), at("выпить воду", "tomorrow", "07:00"), "7:00 has passed: the next one is tomorrow");
        assert_eq!(call("напомни завтра в 9 утра про встречу"), at("про встречу", "tomorrow", "09:00"));
        assert_eq!(call("напомни в пятницу в 15:30 забрать посылку"), at("забрать посылку", "fri", "15:30"));
        assert_eq!(call("напомни в 12 ночи спать"), at("спать", "tomorrow", "00:00"));
        assert_eq!(call("remind me to call mom at 10 pm"), at("call mom", "today", "22:00"));
        assert_eq!(call("remind me at 9:30 to send the report"), at("send the report", "tomorrow", "09:30"));
        let (text, when) = call("напомни мне через 5 минут проверить чайник").unwrap();
        assert_eq!((text.as_str(), when["in"].clone()), ("проверить чайник", json!(5)));
        assert_eq!(call("напомни через два часа выключить духовку").unwrap().1["in"], json!(120));
        assert_eq!(call("напомни через час позвонить").unwrap().1["in"], json!(60));
        assert_eq!(call("remind me in half an hour to stretch").unwrap().1["in"], json!(30));
        assert_eq!(call("remind me to stand up in 45 minutes").unwrap(), ("stand up".to_string(), json!({ "day": null, "time": null, "in": 45 })));
    }

    /// T24.4: a series said in words; the words and the time are cut from the text.
    #[test]
    fn a_series_comes_with_its_repeat_time_and_weekday() {
        let get = |phrase: &str| {
            remind_intent(phrase, &leads(), &evening()).map(|i| {
                let s = |k: &str| i.args.get(k).and_then(|v| v.as_str()).map(String::from);
                (s("text").unwrap(), s("repeat"), s("time"), s("day"), i.args.get("inMinutes").is_some())
            })
        };
        let row = |text: &str, repeat: &str, time: &str, day: Option<&str>| Some((text.to_string(), Some(repeat.to_string()), Some(time.to_string()), day.map(String::from), false));
        assert_eq!(get("напомни каждый день в 9 утра выпить воду"), row("выпить воду", "daily", "09:00", None));
        assert_eq!(get("напомни мне по будням в 8:30 вставать"), row("вставать", "weekdays", "08:30", None));
        assert_eq!(get("напомни по рабочим дням в 18.00 закрывать задачи"), row("закрывать задачи", "weekdays", "18:00", None));
        assert_eq!(get("напомни каждый понедельник в 10 вечера выносить мусор"), row("выносить мусор", "weekly", "22:00", Some("mon")));
        assert_eq!(get("напомни каждую пятницу в 18:00 подводить итоги"), row("подводить итоги", "weekly", "18:00", Some("fri")));
        assert_eq!(get("напомни в 7:15 ежедневно делать зарядку"), row("делать зарядку", "daily", "07:15", None));
        assert_eq!(get("remind me every day at 7 am to take pills"), row("take pills", "daily", "07:00", None));
        assert_eq!(get("remind me on weekdays at 8:30 to stretch"), row("stretch", "weekdays", "08:30", None));
        assert_eq!(get("remind me every Monday at 9:15 to plan the week"), row("plan the week", "weekly", "09:15", Some("mon")));
        // A series without a clock time, or with "in N minutes", is not a series the table can read.
        assert_eq!(get("напомни каждый день пить воду"), None);
        assert_eq!(get("напомни каждый день через 5 минут пить"), None);
    }

    #[test]
    fn what_has_no_time_or_no_text_is_left_to_others() {
        assert_eq!(call("напомни мне купить хлеб"), None, "no time");
        assert_eq!(call("напомни мне в 2215"), None, "no text");
        assert_eq!(call("открой калькулятор в 2215"), None, "no lead-in");
        assert_eq!(call("напомни купить 2.50 литра"), None, "a decimal is not a time without в");
        assert_eq!(call("напомни в 25:99 сделать"), None, "not a clock");
        assert_eq!(call("напомни мне в 5 яблок"), None);
    }
}
