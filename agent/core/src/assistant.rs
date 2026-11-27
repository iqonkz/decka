//! Reminders and lists of the voice assistant (DATA_MODEL.md 13.4): the files `reminders.json` and `lists.json`, the limits, the
//! reading of a time (`remind` takes minutes, a day and a clock time, or a local date-time; the model never does date
//! arithmetic), and `list_toggle` from the phone. Everything is on this PC; the phone only sends ids that the agent issued.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const MAX_REMINDERS: usize = 500;
pub const MAX_LISTS: usize = 50;
pub const MAX_ITEMS: usize = 200;
pub const MAX_TEXT: usize = 200;
/// What the phone sections carry (DATA_MODEL.md 13.7).
pub const PHONE_REMINDERS: usize = 5;
pub const PHONE_ITEMS: usize = 50;
/// A reminder may be set at most this far ahead.
pub const MAX_AHEAD_MINUTES: i64 = 365 * 24 * 60;

// ---- Local time -------------------------------------------------------------------------------------------------

/// A wall-clock time to the minute, as the user sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Local {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
pub(crate) fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + (m <= 2) as i64) as i32, m, d)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

impl Local {
    pub fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Option<Local> {
        ((1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&day) && hour < 24 && minute < 60 && (1970..=9999).contains(&year))
            .then_some(Local { year, month, day, hour, minute })
    }

    /// Minutes since 1970-01-01 00:00 of this wall clock.
    pub fn minutes(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 1440 + self.hour as i64 * 60 + self.minute as i64
    }

    pub fn from_minutes(m: i64) -> Local {
        let (y, mo, d) = civil_from_days(m.div_euclid(1440));
        let rest = m.rem_euclid(1440);
        Local { year: y, month: mo, day: d, hour: (rest / 60) as u32, minute: (rest % 60) as u32 }
    }

    /// 0 = Monday .. 6 = Sunday.
    pub fn weekday(&self) -> u32 {
        // 1970-01-01 was a Thursday.
        (days_from_civil(self.year, self.month, self.day) + 3).rem_euclid(7) as u32
    }

    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}T{:02}:{:02}", self.year, self.month, self.day, self.hour, self.minute)
    }

    /// `YYYY-MM-DDTHH:MM` with optional `:SS` (ignored); a zone or any other suffix is refused: the time is local.
    pub fn parse_iso(s: &str) -> Option<Local> {
        let s = s.trim();
        let (date, time) = s.split_once(['T', ' '])?;
        let mut d = date.split('-');
        let (y, mo, da) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
        if d.next().is_some() || date.len() != 10 {
            return None;
        }
        let mut t = time.split(':');
        let (h, mi) = (t.next()?.parse().ok()?, t.next()?.parse().ok()?);
        if let Some(sec) = t.next() {
            sec.parse::<u32>().ok().filter(|s| *s < 60)?;
        }
        if t.next().is_some() || time.len() > 8 {
            return None;
        }
        Local::new(y, mo, da, h, mi)
    }

    #[cfg(windows)]
    pub fn now() -> Local {
        use windows::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY: a plain query without arguments.
        let t = unsafe { GetLocalTime() };
        Local { year: t.wYear as i32, month: t.wMonth as u32, day: t.wDay as u32, hour: t.wHour as u32, minute: t.wMinute as u32 }
    }

    #[cfg(not(windows))]
    pub fn now() -> Local {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Local::from_minutes((secs / 60) as i64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Day {
    Today,
    Tomorrow,
    /// 0 = Monday .. 6 = Sunday: the next such day.
    Weekday(u32),
}

impl Day {
    pub fn parse(s: &str) -> Option<Day> {
        Some(match s {
            "today" => Day::Today,
            "tomorrow" => Day::Tomorrow,
            "mon" => Day::Weekday(0),
            "tue" => Day::Weekday(1),
            "wed" => Day::Weekday(2),
            "thu" => Day::Weekday(3),
            "fri" => Day::Weekday(4),
            "sat" => Day::Weekday(5),
            "sun" => Day::Weekday(6),
            _ => return None,
        })
    }
}

/// When a reminder fires, as the tool call states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    InMinutes(u32),
    At(Local),
    Day { day: Day, hour: u32, minute: u32 },
}

pub const PAST: &str = "That time has already passed";
pub const TOO_FAR: &str = "A reminder can be set at most a year ahead";

/// The moment of `when` counted from `now`: in the future and within a year, else the reason why not.
pub fn resolve(when: &When, now: &Local) -> Result<Local, &'static str> {
    let now_m = now.minutes();
    let target = match when {
        When::InMinutes(m) => now_m + *m as i64,
        When::At(l) => l.minutes(),
        When::Day { day, hour, minute } => {
            let today = now_m.div_euclid(1440);
            let clock = *hour as i64 * 60 + *minute as i64;
            let date = match day {
                Day::Today => today,
                Day::Tomorrow => today + 1,
                Day::Weekday(w) => {
                    let mut ahead = (*w as i64 - now.weekday() as i64).rem_euclid(7);
                    // The same weekday later today counts; at or after the time it is next week.
                    if ahead == 0 && today * 1440 + clock <= now_m {
                        ahead = 7;
                    }
                    today + ahead
                }
            };
            date * 1440 + clock
        }
    };
    if target <= now_m {
        Err(PAST)
    } else if target - now_m > MAX_AHEAD_MINUTES {
        Err(TOO_FAR)
    } else {
        Ok(Local::from_minutes(target))
    }
}

// ---- Data -------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub text: String,
    /// Local time `YYYY-MM-DDTHH:MM`; of a repeating reminder the next time it fires.
    pub at: String,
    pub done: bool,
    /// 0.23 (DATA_MODEL.md 13.4): a series; absent = once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat>,
}

/// How a series repeats. `weekly` keeps the weekday of `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Daily,
    Weekdays,
    Weekly,
}

impl Repeat {
    pub fn parse(s: &str) -> Option<Repeat> {
        Some(match s {
            "daily" => Repeat::Daily,
            "weekdays" => Repeat::Weekdays,
            "weekly" => Repeat::Weekly,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Repeat::Daily => "daily",
            Repeat::Weekdays => "weekdays",
            Repeat::Weekly => "weekly",
        }
    }
}

fn day_of(l: &Local) -> i64 {
    days_from_civil(l.year, l.month, l.day)
}

fn is_weekend(day: i64) -> bool {
    (day + 3).rem_euclid(7) >= 5
}

fn step(day: i64, repeat: Repeat) -> i64 {
    match repeat {
        Repeat::Daily => day + 1,
        Repeat::Weekly => day + 7,
        Repeat::Weekdays => {
            let mut d = day + 1;
            while is_weekend(d) {
                d += 1;
            }
            d
        }
    }
}

/// The moment of a series that follows `at` (`include_at` false: after `at`) and lies after `now`. The wall clock time of day
/// is kept (the local clock, so a change of summer time moves nothing); `weekdays` skips Saturday and Sunday, `weekly` keeps
/// the weekday, the end of a month and of a year are plain calendar days.
pub fn occurrence_after(at: &Local, repeat: Repeat, now: &Local, include_at: bool) -> Local {
    let clock = at.hour as i64 * 60 + at.minute as i64;
    let now_m = now.minutes();
    let mut d = day_of(at);
    if repeat == Repeat::Weekdays {
        while is_weekend(d) {
            d += 1;
        }
    }
    if !include_at {
        d = step(d, repeat);
    }
    let gap = now_m.div_euclid(1440) - d;
    if gap > 0 {
        match repeat {
            Repeat::Daily => d += gap,
            Repeat::Weekly => d += gap / 7 * 7,
            Repeat::Weekdays => {}
        }
    }
    let mut guard = 0;
    while d * 1440 + clock <= now_m && guard < 20_000 {
        d = step(d, repeat);
        guard += 1;
    }
    Local::from_minutes(d * 1440 + clock)
}

/// A moment of `day` at `hour:minute`, whether it has passed or not (the series decides what comes next).
fn day_clock(day: &Day, hour: u32, minute: u32, now: &Local) -> Local {
    let today = now.minutes().div_euclid(1440);
    let date = match day {
        Day::Today => today,
        Day::Tomorrow => today + 1,
        Day::Weekday(w) => today + (*w as i64 - now.weekday() as i64).rem_euclid(7),
    };
    Local::from_minutes(date * 1440 + hour as i64 * 60 + minute as i64)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct List {
    pub id: String,
    pub name: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RemindersFile {
    reminders: Vec<Reminder>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ListsFile {
    lists: Vec<List>,
}

fn new_id() -> String {
    format!("{:016x}", rand::random::<u64>())
}

fn clean_text(s: &str, what: &str) -> Result<String, String> {
    let t: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err(format!("The {what} is empty"));
    }
    if t.chars().count() > MAX_TEXT {
        return Err(format!("The {what} is too long (at most {MAX_TEXT} characters)"));
    }
    Ok(t)
}

/// The name of the list or the item the user said: exact (any case), then a part of it, then a close spelling.
pub fn find_name<'a>(names: impl Iterator<Item = &'a str>, query: &str) -> Option<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let names: Vec<String> = names.map(|n| n.to_lowercase()).collect();
    if let Some(i) = names.iter().position(|n| *n == q) {
        return Some(i);
    }
    if let Some(i) = names.iter().position(|n| n.contains(&q) || (q.chars().count() >= 4 && q.contains(n.as_str()))) {
        return Some(i);
    }
    // A slip of a letter or two (the name comes from speech recognition): at most a quarter of the name may differ.
    let close = names
        .iter()
        .enumerate()
        .map(|(i, n)| (edit_distance(&q, n), n.chars().count(), i))
        .filter(|(d, len, _)| *len >= 3 && *d <= (*len / 4).max(1))
        .min_by_key(|(d, _, _)| *d);
    if let Some((_, _, i)) = close {
        return Some(i);
    }
    names
        .iter()
        .enumerate()
        .map(|(i, n)| (super::voice::matcher::trigram_sim(&q, n), i))
        .filter(|(s, _)| *s >= super::voice::matcher::FUZZY_THRESHOLD)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
}

/// Levenshtein distance in characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The two files of the assistant. Every change is saved at once (write to a temporary file, then rename).
pub struct Assistant {
    dir: PathBuf,
    inner: Mutex<(RemindersFile, ListsFile)>,
    /// Counts the saved changes; the agent publishes the phone sections when it moves (DATA_MODEL.md 13.7).
    changed: tokio::sync::watch::Sender<u64>,
}

fn read<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> T {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// What `list_show` and the phone get.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListView {
    pub id: String,
    pub name: String,
    pub items: Vec<Item>,
}

impl Assistant {
    pub fn load(dir: &Path) -> Assistant {
        let mut rem: RemindersFile = read(&dir.join("reminders.json"));
        let mut lists: ListsFile = read(&dir.join("lists.json"));
        // A hand-edited file may exceed the limits or hold broken ids: it is cut to what the agent itself would have written.
        rem.reminders.truncate(MAX_REMINDERS);
        lists.lists.truncate(MAX_LISTS);
        for l in &mut lists.lists {
            l.items.truncate(MAX_ITEMS);
        }
        Assistant { dir: dir.to_path_buf(), inner: Mutex::new((rem, lists)), changed: tokio::sync::watch::channel(0).0 }
    }

    fn save_reminders(&self, f: &RemindersFile) -> Result<(), String> {
        let r = write(&self.dir.join("reminders.json"), f);
        self.changed.send_modify(|n| *n += 1);
        r
    }

    fn save_lists(&self, f: &ListsFile) -> Result<(), String> {
        let r = write(&self.dir.join("lists.json"), f);
        self.changed.send_modify(|n| *n += 1);
        r
    }

    // ---- The Reminders window (DATA_MODEL.md 13.7); ids are the ones the agent issued, anything else is refused ----

    /// Every reminder, finished ones too, soonest first.
    pub fn reminders_all(&self) -> Vec<Reminder> {
        let mut v = self.inner.lock().unwrap().0.reminders.clone();
        v.sort_by(|a, b| a.at.cmp(&b.at));
        v
    }

    /// Adds a reminder (`id` = None) or changes the text and the time of one. A time that is new must lie in the future, within a
    /// year; the same time as before is kept whatever it is. Changing the time of a finished reminder makes it open again.
    /// `repeat`: `None` keeps the series of an existing reminder (a new one is once), `Some(None)` makes it once, `Some(Some(r))` a series.
    pub fn reminder_upsert(&self, id: Option<&str>, text: &str, at_iso: &str, repeat: Option<Option<Repeat>>, now: &Local) -> Result<Reminder, String> {
        let text = clean_text(text, "reminder")?;
        let at = Local::parse_iso(at_iso).ok_or("The time must be YYYY-MM-DDTHH:MM")?;
        let mut g = self.inner.lock().unwrap();
        let Some(id) = id else {
            drop(g);
            let wanted = repeat.flatten();
            // A series starts at its first moment on or after the chosen one (weekdays skip a weekend).
            let first = match wanted {
                Some(r) => {
                    resolve(&When::At(at), now).map_err(String::from)?;
                    occurrence_after(&at, r, now, true)
                }
                None => at,
            };
            return self.add_reminder_repeat(&text, &When::At(first), None, now).and_then(|r| match wanted {
                Some(rep) => self.set_repeat(&r.id, Some(rep)).ok_or_else(|| "There is no such reminder".to_string()),
                None => Ok(r),
            });
        };
        let Some(i) = g.0.reminders.iter().position(|r| r.id == id) else { return Err("There is no such reminder".into()) };
        if g.0.reminders[i].at != at.iso() {
            resolve(&When::At(at), now).map_err(String::from)?;
            g.0.reminders[i].done = false;
        }
        g.0.reminders[i].text = text;
        g.0.reminders[i].at = at.iso();
        if let Some(wanted) = repeat {
            g.0.reminders[i].repeat = wanted;
            // A series of weekdays never starts on a weekend.
            if let (Some(r), Some(l)) = (wanted, Local::parse_iso(&g.0.reminders[i].at)) {
                if r == Repeat::Weekdays {
                    g.0.reminders[i].at = occurrence_after(&l, r, now, true).iso();
                }
            }
        }
        let r = g.0.reminders[i].clone();
        self.save_reminders(&g.0)?;
        Ok(r)
    }

    /// Makes a reminder of this agent a series (or once); `None` for an id it did not issue.
    fn set_repeat(&self, id: &str, repeat: Option<Repeat>) -> Option<Reminder> {
        let mut g = self.inner.lock().unwrap();
        let r = g.0.reminders.iter_mut().find(|r| r.id == id)?;
        r.repeat = repeat;
        let out = r.clone();
        let _ = self.save_reminders(&g.0);
        Some(out)
    }

    /// Deletes a reminder (the whole series of a repeating one); false for an id the agent did not issue.
    pub fn reminder_delete(&self, id: &str) -> bool {
        self.delete_reminder(id)
    }

    /// Creates a list (`id` = None) or replaces the name and the items of one. Items with an id must belong to the list (a foreign
    /// id is an error); items without an id are new. The order of `items` is the order of the list.
    pub fn list_upsert(&self, id: Option<&str>, name: &str, items: &[(Option<String>, String, bool)]) -> Result<ListView, String> {
        let name = clean_text(name, "list name")?;
        if items.len() > MAX_ITEMS {
            return Err("The list is full".into());
        }
        let mut g = self.inner.lock().unwrap();
        let existing = match id {
            Some(id) => Some(g.1.lists.iter().position(|l| l.id == id).ok_or("There is no such list")?),
            None => None,
        };
        if existing.is_none() && g.1.lists.len() >= MAX_LISTS {
            return Err("There are too many lists".into());
        }
        // Everything is checked before anything changes.
        let mut new_items: Vec<Item> = Vec::new();
        for (item_id, text, done) in items {
            let text = clean_text(text, "item")?;
            let item_id = match item_id {
                Some(i) => {
                    if !existing.is_some_and(|at| g.1.lists[at].items.iter().any(|x| &x.id == i)) {
                        return Err("There is no such item".into());
                    }
                    if new_items.iter().any(|x| &x.id == i) {
                        return Err("An item is there twice".into());
                    }
                    i.clone()
                }
                None => new_id(),
            };
            new_items.push(Item { id: item_id, text, done: *done });
        }
        let at = match existing {
            Some(at) => at,
            None => {
                g.1.lists.push(List { id: new_id(), name: name.clone(), items: Vec::new() });
                g.1.lists.len() - 1
            }
        };
        g.1.lists[at].name = name;
        g.1.lists[at].items = new_items;        let l = &g.1.lists[at];
        let view = ListView { id: l.id.clone(), name: l.name.clone(), items: l.items.clone() };
        self.save_lists(&g.1)?;
        Ok(view)
    }

    /// Deletes a list by id; false for an id the agent did not issue.
    pub fn list_delete_id(&self, id: &str) -> bool {
        let mut g = self.inner.lock().unwrap();
        let before = g.1.lists.len();
        g.1.lists.retain(|l| l.id != id);
        let removed = g.1.lists.len() != before;
        if removed {
            let _ = self.save_lists(&g.1);
        }
        removed
    }

    /// Every list with all its items, as the IPC command `lists_get` returns them.
    pub fn lists_json(&self) -> serde_json::Value {
        serde_json::json!(self.lists().iter().map(|l| serde_json::json!({
            "id": l.id, "name": l.name,
            "items": l.items.iter().map(|i| serde_json::json!({ "id": i.id, "text": i.text, "done": i.done })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>())
    }

    /// Wakes when a reminder or a list was saved.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changed.subscribe()
    }

    /// The `reminders` and `lists` sections of the phone state (DATA_MODEL.md 13.7): the next 5 reminders and every list with
    /// its first 50 items.
    pub fn sections(&self) -> (serde_json::Value, serde_json::Value) {
        // `repeat` is present only for a series.
        let row = |r: &Reminder| {
            let mut v = serde_json::json!({ "id": r.id, "text": r.text, "at": r.at });
            if let Some(repeat) = r.repeat {
                v["repeat"] = repeat.as_str().into();
            }
            v
        };
        let reminders = serde_json::json!({ "items": self.upcoming(PHONE_REMINDERS).iter().map(row).collect::<Vec<_>>() });
        let lists = serde_json::json!({ "lists": self.lists().iter().map(|l| serde_json::json!({
            "id": l.id, "name": l.name,
            "items": l.items.iter().take(PHONE_ITEMS).map(|i| serde_json::json!({ "id": i.id, "text": i.text, "done": i.done })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>() });
        (reminders, lists)
    }

    // ---- Reminders ----

    pub fn add_reminder(&self, text: &str, when: &When, now: &Local) -> Result<Reminder, String> {
        self.add_reminder_repeat(text, when, None, now)
    }

    /// `add_reminder` for a series: `when` must be a time of day (with a day or without), the first firing is the first moment of
    /// the series after `now`.
    pub fn add_reminder_repeat(&self, text: &str, when: &When, repeat: Option<Repeat>, now: &Local) -> Result<Reminder, String> {
        let text = clean_text(text, "reminder")?;
        let at = match (repeat, when) {
            (None, w) => resolve(w, now).map_err(String::from)?,
            (Some(r), When::Day { day, hour, minute }) => occurrence_after(&day_clock(day, *hour, *minute, now), r, now, true),
            (Some(_), _) => return Err("A repeating reminder needs a time of the day".into()),
        };
        let mut g = self.inner.lock().unwrap();
        // Finished reminders make room for new ones.
        if g.0.reminders.len() >= MAX_REMINDERS {
            g.0.reminders.retain(|r| !r.done);
        }
        if g.0.reminders.len() >= MAX_REMINDERS {
            return Err("There are too many reminders".into());
        }
        let r = Reminder { id: new_id(), text, at: at.iso(), done: false, repeat };
        g.0.reminders.push(r.clone());
        self.save_reminders(&g.0)?;
        Ok(r)
    }

    /// The reminders that are due at `now` (finished ones are marked and saved, so each fires once).
    pub fn take_due(&self, now: &Local) -> Vec<Reminder> {
        let mut g = self.inner.lock().unwrap();
        let now_iso = now.iso();
        let mut due: Vec<Reminder> = g.0.reminders.iter().filter(|r| !r.done && r.at <= now_iso).cloned().collect();
        if due.is_empty() {
            return due;
        }
        due.sort_by(|a, b| a.at.cmp(&b.at));
        for r in &mut g.0.reminders {
            if !r.done && r.at <= now_iso {
                // A series is not finished by firing: it moves to its next moment after now (what was missed in between is skipped,
                // so a series that was due several times while the PC was off is one reminder, one `Missed`).
                match (r.repeat, Local::parse_iso(&r.at)) {
                    (Some(repeat), Some(at)) => r.at = occurrence_after(&at, repeat, now, false).iso(),
                    _ => r.done = true,
                }
            }
        }
        let _ = self.save_reminders(&g.0);
        due
    }

    /// The next not finished reminders, soonest first.
    pub fn upcoming(&self, n: usize) -> Vec<Reminder> {
        let g = self.inner.lock().unwrap();
        let mut v: Vec<Reminder> = g.0.reminders.iter().filter(|r| !r.done).cloned().collect();
        v.sort_by(|a, b| a.at.cmp(&b.at));
        v.truncate(n);
        v
    }

    pub fn delete_reminder(&self, id: &str) -> bool {
        let mut g = self.inner.lock().unwrap();
        let before = g.0.reminders.len();
        g.0.reminders.retain(|r| r.id != id);
        let removed = g.0.reminders.len() != before;
        if removed {
            let _ = self.save_reminders(&g.0);
        }
        removed
    }

    // ---- Lists ----

    pub fn lists(&self) -> Vec<ListView> {
        self.inner.lock().unwrap().1.lists.iter().map(|l| ListView { id: l.id.clone(), name: l.name.clone(), items: l.items.clone() }).collect()
    }

    /// Adds `item` to the list called `list`; a list that does not exist is made. Returns the name of the list.
    pub fn list_add(&self, list: &str, item: &str) -> Result<String, String> {
        let item = clean_text(item, "item")?;
        let mut g = self.inner.lock().unwrap();
        let at = match find_name(g.1.lists.iter().map(|l| l.name.as_str()), list) {
            Some(i) => i,
            None => {
                let name = clean_text(list, "list name")?;
                if g.1.lists.len() >= MAX_LISTS {
                    return Err("There are too many lists".into());
                }
                g.1.lists.push(List { id: new_id(), name, items: Vec::new() });
                g.1.lists.len() - 1
            }
        };
        let l = &mut g.1.lists[at];
        if l.items.len() >= MAX_ITEMS {
            return Err("The list is full".into());
        }
        l.items.push(Item { id: new_id(), text: item, done: false });
        let name = l.name.clone();
        self.save_lists(&g.1)?;
        Ok(name)
    }

    pub fn list_show(&self, list: &str) -> Result<ListView, String> {
        let g = self.inner.lock().unwrap();
        let i = find_name(g.1.lists.iter().map(|l| l.name.as_str()), list).ok_or("There is no such list")?;
        let l = &g.1.lists[i];
        Ok(ListView { id: l.id.clone(), name: l.name.clone(), items: l.items.clone() })
    }

    /// Removes one item (the closest to `item`). Returns `(list name, item text)`.
    pub fn list_remove_item(&self, list: &str, item: &str) -> Result<(String, String), String> {
        let mut g = self.inner.lock().unwrap();
        let li = find_name(g.1.lists.iter().map(|l| l.name.as_str()), list).ok_or("There is no such list")?;
        let ii = find_name(g.1.lists[li].items.iter().map(|i| i.text.as_str()), item).ok_or("There is no such item")?;
        let removed = g.1.lists[li].items.remove(ii);
        let name = g.1.lists[li].name.clone();
        self.save_lists(&g.1)?;
        Ok((name, removed.text))
    }

    /// Deletes the whole list (the tool asks for a confirmation first). Returns its name.
    pub fn list_delete(&self, list: &str) -> Result<String, String> {
        let mut g = self.inner.lock().unwrap();
        let li = find_name(g.1.lists.iter().map(|l| l.name.as_str()), list).ok_or("There is no such list")?;
        let l = g.1.lists.remove(li);
        self.save_lists(&g.1)?;
        Ok(l.name)
    }

    /// `list_toggle{listId, itemId}` of the phone: marks or unmarks one item. Ids the agent did not issue change nothing.
    pub fn toggle(&self, list_id: &str, item_id: &str) -> bool {
        let mut g = self.inner.lock().unwrap();
        let Some(item) = g.1.lists.iter_mut().find(|l| l.id == list_id).and_then(|l| l.items.iter_mut().find(|i| i.id == item_id)) else {
            return false;
        };
        item.done = !item.done;
        let _ = self.save_lists(&g.1);
        true
    }
}

/// The text a list is read out as: `Shopping: milk, bread (done: eggs)`.
pub fn list_text(l: &ListView) -> String {
    if l.items.is_empty() {
        return format!("{}: empty", l.name);
    }
    let open: Vec<&str> = l.items.iter().filter(|i| !i.done).map(|i| i.text.as_str()).collect();
    let done: Vec<&str> = l.items.iter().filter(|i| i.done).map(|i| i.text.as_str()).collect();
    let mut s = format!("{}: {}", l.name, if open.is_empty() { "all done".to_string() } else { open.join(", ") });
    if !done.is_empty() && !open.is_empty() {
        s.push_str(&format!(" (done: {})", done.join(", ")));
    }
    s
}

/// The reminders as one line for the pill: `18:30 call mom; tomorrow 09:00 pay rent`.
pub fn reminders_text(list: &[Reminder], now: &Local) -> String {
    if list.is_empty() {
        return "No reminders".into();
    }
    let today = now.minutes().div_euclid(1440);
    list.iter()
        .map(|r| {
            let at = Local::parse_iso(&r.at);
            let when = match at {
                Some(l) if l.minutes().div_euclid(1440) == today => format!("{:02}:{:02}", l.hour, l.minute),
                Some(l) if l.minutes().div_euclid(1440) == today + 1 => format!("tomorrow {:02}:{:02}", l.hour, l.minute),
                Some(l) => format!("{:02}.{:02} {:02}:{:02}", l.day, l.month, l.hour, l.minute),
                None => r.at.clone(),
            };
            format!("{when} {}", r.text)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The title and the body of a fired reminder; several at once (missed while the PC was off) are one `Missed` notice.
pub fn due_notice(due: &[Reminder], on_start: bool) -> Option<(String, String)> {
    match due {
        [] => None,
        [r] if !on_start => Some(("Reminder".into(), r.text.clone())),
        many => Some((
            if on_start { "Missed".into() } else { "Reminders".into() },
            many.iter().map(|r| r.text.as_str()).collect::<Vec<_>>().join("; ").chars().take(300).collect(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> Local {
        Local::new(y, mo, d, h, mi).unwrap()
    }

    // 2026-11-25 is a Wednesday.
    fn now() -> Local {
        at(2026, 11, 25, 14, 30)
    }

    // ---- Series (0.23, DATA_MODEL.md 13.4 "Повтор") ----

    #[test]
    fn the_next_moment_of_a_series() {
        // 2026-11-25 is a Wednesday: Friday is the 27th, Monday the 30th.
        let fired = |l: Local, r: Repeat, now: Local| occurrence_after(&l, r, &now, false).iso();
        assert_eq!(fired(at(2026, 11, 25, 9, 0), Repeat::Daily, at(2026, 11, 25, 9, 0)), "2026-11-26T09:00");
        assert_eq!(fired(at(2026, 11, 25, 9, 0), Repeat::Weekly, at(2026, 11, 25, 9, 0)), "2026-12-02T09:00");
        // Weekdays skip the weekend: Friday -> Monday, Thursday -> Friday.
        assert_eq!(fired(at(2026, 11, 27, 9, 0), Repeat::Weekdays, at(2026, 11, 27, 9, 0)), "2026-11-30T09:00");
        assert_eq!(fired(at(2026, 11, 26, 9, 0), Repeat::Weekdays, at(2026, 11, 26, 9, 0)), "2026-11-27T09:00");
        // The end of a month, a year and a leap February are plain calendar days.
        assert_eq!(fired(at(2026, 11, 30, 23, 30), Repeat::Daily, at(2026, 11, 30, 23, 30)), "2026-12-01T23:30");
        assert_eq!(fired(at(2026, 12, 31, 8, 0), Repeat::Daily, at(2026, 12, 31, 8, 0)), "2027-01-01T08:00");
        assert_eq!(fired(at(2028, 2, 28, 8, 0), Repeat::Daily, at(2028, 2, 28, 8, 0)), "2028-02-29T08:00");
        assert_eq!(fired(at(2027, 2, 28, 8, 0), Repeat::Daily, at(2027, 2, 28, 8, 0)), "2027-03-01T08:00");
        assert_eq!(fired(at(2026, 12, 29, 8, 0), Repeat::Weekly, at(2026, 12, 29, 8, 0)), "2027-01-05T08:00");
        // A series that was due many times while the PC was off continues after `now`, never in the past.
        assert_eq!(fired(at(2026, 11, 10, 9, 0), Repeat::Daily, at(2026, 11, 25, 10, 0)), "2026-11-26T09:00");
        assert_eq!(fired(at(2026, 11, 10, 9, 0), Repeat::Daily, at(2026, 11, 25, 8, 0)), "2026-11-25T09:00");
        assert_eq!(fired(at(2026, 11, 2, 9, 0), Repeat::Weekly, at(2026, 11, 25, 10, 0)), "2026-11-30T09:00");
        assert_eq!(fired(at(2026, 11, 2, 9, 0), Repeat::Weekdays, at(2026, 11, 28, 10, 0)), "2026-11-30T09:00", "from a Saturday: Monday");
        // The first moment of a new series: on or after the start, weekdays never on a weekend.
        let first = |l: Local, r: Repeat| occurrence_after(&l, r, &now(), true).iso();
        assert_eq!(first(at(2026, 11, 28, 9, 0), Repeat::Weekdays), "2026-11-30T09:00");
        assert_eq!(first(at(2026, 11, 26, 9, 0), Repeat::Weekdays), "2026-11-26T09:00");
        assert_eq!(first(at(2026, 11, 25, 9, 0), Repeat::Daily), "2026-11-26T09:00", "today 09:00 has passed at 14:30");
    }

    #[test]
    fn the_wall_clock_survives_a_change_of_summer_time() {
        // Europe moved its clocks on 2026-03-29 and 2026-10-25: a reminder of 02:30 or 08:00 stays at that wall time.
        for (from, repeat, hour, minute) in [((2026, 3, 28), Repeat::Daily, 8, 0), ((2026, 10, 24), Repeat::Daily, 8, 0), ((2026, 3, 28), Repeat::Daily, 2, 30), ((2026, 10, 23), Repeat::Weekdays, 7, 15)] {
            let mut moment = at(from.0, from.1, from.2, hour, minute);
            for _ in 0..4 {
                moment = occurrence_after(&moment, repeat, &moment, false);
                assert_eq!((moment.hour, moment.minute), (hour, minute), "{}", moment.iso());
            }
        }
    }

    #[test]
    fn firing_a_series_does_not_finish_it_and_the_window_and_the_phone_see_it() {
        let (_dir, a) = assistant();
        let r = a.add_reminder_repeat("take pills", &When::Day { day: Day::Today, hour: 9, minute: 0 }, Some(Repeat::Daily), &now()).unwrap();
        assert_eq!((r.at.as_str(), r.repeat), ("2026-11-26T09:00", Some(Repeat::Daily)), "09:00 has passed today: the first moment is tomorrow");
        let once = a.add_reminder("one", &When::InMinutes(5), &now()).unwrap();
        let later = at(2026, 11, 26, 9, 0);
        let due = a.take_due(&later);
        assert_eq!(due.iter().map(|d| d.text.as_str()).collect::<Vec<_>>(), ["one", "take pills"]);
        let all = a.reminders_all();
        let series = all.iter().find(|x| x.id == r.id).unwrap();
        assert!(!series.done, "a series is not done after it fired");
        assert_eq!(series.at, "2026-11-27T09:00");
        assert!(all.iter().find(|x| x.id == once.id).is_some_and(|x| x.done), "a single reminder is finished as before");
        assert!(a.take_due(&later).is_empty(), "no second firing at the same time");
        // The phone section carries `repeat` for the series only.
        let (reminders, _) = a.sections();
        let items = reminders["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["repeat"], "daily");
        a.add_reminder("plain", &When::InMinutes(20), &later).unwrap();
        let (reminders, _) = a.sections();
        assert!(reminders["items"].as_array().unwrap().iter().any(|i| i["text"] == "plain" && i.get("repeat").is_none()));
    }

    #[test]
    fn a_series_missed_for_days_is_one_missed_notice_and_continues() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("reminders.json"),
            r#"{"reminders":[
              {"id":"0000000000000001","text":"pills","at":"2026-11-20T09:00","done":false,"repeat":"daily"},
              {"id":"0000000000000002","text":"standup","at":"2026-11-23T10:00","done":false,"repeat":"weekdays"},
              {"id":"0000000000000003","text":"rent","at":"2026-11-24T12:00","done":false}
            ]}"#,
        )
        .unwrap();
        let a = Assistant::load(dir.path());
        let start = at(2026, 11, 25, 14, 30);
        let due = a.take_due(&start);
        assert_eq!(due.len(), 3, "one entry per series and per reminder, however many times it was due");
        let (title, body) = due_notice(&due, true).unwrap();
        assert_eq!(title, "Missed");
        assert_eq!(body.matches("pills").count() + body.matches("standup").count() + body.matches("rent").count(), 3);
        let after = a.reminders_all();
        assert_eq!(after.iter().find(|r| r.text == "pills").unwrap().at, "2026-11-26T09:00");
        assert_eq!(after.iter().find(|r| r.text == "standup").unwrap().at, "2026-11-26T10:00");
        assert!(after.iter().find(|r| r.text == "rent").unwrap().done);
        assert!(a.take_due(&start).is_empty(), "the next start has nothing to say");
        // The file keeps the series (done stays false) and its repeat.
        let file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join("reminders.json")).unwrap()).unwrap();
        let rows = file["reminders"].as_array().unwrap();
        assert_eq!((rows[0]["repeat"].as_str(), rows[0]["done"].as_bool()), (Some("daily"), Some(false)));
        assert_eq!((rows[1]["repeat"].as_str(), rows[2].get("repeat")), (Some("weekdays"), None));
    }

    #[test]
    fn deleting_removes_the_whole_series_and_the_window_edits_the_repeat() {
        let (_dir, a) = assistant();
        let r = a.add_reminder_repeat("gym", &When::Day { day: Day::Weekday(0), hour: 7, minute: 0 }, Some(Repeat::Weekly), &now()).unwrap();
        assert_eq!(r.at, "2026-11-30T07:00", "the next Monday");
        assert!(a.reminder_delete(&r.id));
        assert!(a.reminders_all().is_empty() && a.take_due(&at(2027, 1, 1, 0, 0)).is_empty());
        // From the window: a new series on a weekend starts on Monday; an edit without `repeat` keeps it, "once" ends it.
        let s = a.reminder_upsert(None, "stretch", "2026-11-28T09:00", Some(Some(Repeat::Weekdays)), &now()).unwrap();
        assert_eq!((s.at.as_str(), s.repeat), ("2026-11-30T09:00", Some(Repeat::Weekdays)));
        let kept = a.reminder_upsert(Some(&s.id), "stretch more", "2026-11-30T09:00", None, &now()).unwrap();
        assert_eq!((kept.text.as_str(), kept.repeat), ("stretch more", Some(Repeat::Weekdays)));
        let once = a.reminder_upsert(Some(&s.id), "stretch more", "2026-11-30T09:00", Some(None), &now()).unwrap();
        assert_eq!(once.repeat, None);
        assert!(a.add_reminder_repeat("x", &When::InMinutes(5), Some(Repeat::Daily), &now()).is_err(), "a series needs a time of the day");
        assert_eq!(Repeat::parse("weekly"), Some(Repeat::Weekly));
        assert_eq!(Repeat::parse("monthly"), None);
    }

    #[test]
    fn dates_and_weekdays_are_right() {
        assert_eq!(now().weekday(), 2, "Wednesday");
        assert_eq!(at(1970, 1, 1, 0, 0).weekday(), 3, "Thursday");
        assert_eq!(at(2024, 2, 29, 0, 0).weekday(), 3);
        assert!(Local::new(2026, 2, 29, 0, 0).is_none() && Local::new(2024, 2, 29, 0, 0).is_some());
        for m in [0i64, 1, 1439, 1440, 20_000_000, 29_000_000] {
            assert_eq!(Local::from_minutes(m).minutes(), m);
        }
        assert_eq!(Local::from_minutes(now().minutes() + 24 * 60 * 36).iso(), "2026-12-31T14:30");
    }

    #[test]
    fn iso_times_are_read_strictly() {
        assert_eq!(Local::parse_iso("2026-11-26T09:05"), Some(at(2026, 11, 26, 9, 5)));
        assert_eq!(Local::parse_iso("2026-11-26T09:05:59"), Some(at(2026, 11, 26, 9, 5)));
        assert_eq!(Local::parse_iso("2026-11-26 09:05"), Some(at(2026, 11, 26, 9, 5)));
        for bad in ["2026-11-26", "2026-13-01T10:00", "2026-11-26T25:00", "2026-11-26T10:00Z", "2026-11-26T10:00+05:00", "tomorrow", "2026-02-30T10:00", "26-11-26T10:00"] {
            assert_eq!(Local::parse_iso(bad), None, "{bad}");
        }
    }

    #[test]
    fn minutes_today_tomorrow_and_weekdays() {
        assert_eq!(resolve(&When::InMinutes(15), &now()), Ok(at(2026, 11, 25, 14, 45)));
        assert_eq!(resolve(&When::InMinutes(60 * 10), &now()), Ok(at(2026, 11, 26, 0, 30)), "across midnight");
        assert_eq!(resolve(&When::Day { day: Day::Today, hour: 18, minute: 0 }, &now()), Ok(at(2026, 11, 25, 18, 0)));
        assert_eq!(resolve(&When::Day { day: Day::Today, hour: 9, minute: 0 }, &now()), Err(PAST));
        assert_eq!(resolve(&When::Day { day: Day::Today, hour: 14, minute: 30 }, &now()), Err(PAST), "this very minute is not the future");
        assert_eq!(resolve(&When::Day { day: Day::Tomorrow, hour: 9, minute: 0 }, &now()), Ok(at(2026, 11, 26, 9, 0)));
        // Friday is two days ahead; Monday is next week; Wednesday after the time is next Wednesday; before it, today.
        assert_eq!(resolve(&When::Day { day: Day::Weekday(4), hour: 8, minute: 0 }, &now()), Ok(at(2026, 11, 27, 8, 0)));
        assert_eq!(resolve(&When::Day { day: Day::Weekday(0), hour: 8, minute: 0 }, &now()), Ok(at(2026, 11, 30, 8, 0)));
        assert_eq!(resolve(&When::Day { day: Day::Weekday(2), hour: 8, minute: 0 }, &now()), Ok(at(2026, 12, 2, 8, 0)));
        assert_eq!(resolve(&When::Day { day: Day::Weekday(2), hour: 20, minute: 0 }, &now()), Ok(at(2026, 11, 25, 20, 0)));
        // Month and year borders.
        assert_eq!(resolve(&When::Day { day: Day::Tomorrow, hour: 1, minute: 0 }, &at(2026, 12, 31, 23, 0)), Ok(at(2027, 1, 1, 1, 0)));
    }

    #[test]
    fn an_exact_time_must_be_in_the_future_and_within_a_year() {
        assert_eq!(resolve(&When::At(at(2026, 11, 25, 14, 31)), &now()), Ok(at(2026, 11, 25, 14, 31)));
        assert_eq!(resolve(&When::At(at(2026, 11, 25, 14, 30)), &now()), Err(PAST));
        assert_eq!(resolve(&When::At(at(2025, 1, 1, 0, 0)), &now()), Err(PAST));
        assert_eq!(resolve(&When::At(at(2027, 11, 25, 14, 30)), &now()), Ok(at(2027, 11, 25, 14, 30)), "exactly a year");
        assert_eq!(resolve(&When::At(at(2027, 11, 25, 14, 31)), &now()), Err(TOO_FAR));
        assert_eq!(resolve(&When::InMinutes(525_601), &now()), Err(TOO_FAR));
    }

    fn assistant() -> (tempfile::TempDir, Assistant) {
        let dir = tempfile::tempdir().unwrap();
        let a = Assistant::load(dir.path());
        (dir, a)
    }

    #[test]
    fn a_reminder_fires_once_and_is_saved() {
        let (dir, a) = assistant();
        let r = a.add_reminder("  call   mom ", &When::InMinutes(10), &now()).unwrap();
        assert_eq!((r.text.as_str(), r.at.as_str(), r.done), ("call mom", "2026-11-25T14:40", false));
        assert_eq!(r.id.len(), 16);
        assert!(a.take_due(&at(2026, 11, 25, 14, 39)).is_empty());
        let due = a.take_due(&at(2026, 11, 25, 14, 40));
        assert_eq!(due.len(), 1);
        assert!(a.take_due(&at(2026, 11, 25, 15, 0)).is_empty(), "never twice");
        let reloaded = Assistant::load(dir.path());
        assert!(reloaded.upcoming(5).is_empty() && reloaded.take_due(&at(2030, 1, 1, 0, 0)).is_empty(), "done is saved");
    }

    #[test]
    fn what_was_missed_while_the_pc_was_off_comes_as_one_notice() {
        let (dir, a) = assistant();
        a.add_reminder("pay rent", &When::InMinutes(5), &now()).unwrap();
        a.add_reminder("call mom", &When::InMinutes(60), &now()).unwrap();
        a.add_reminder("later", &When::Day { day: Day::Tomorrow, hour: 9, minute: 0 }, &now()).unwrap();
        drop(a);
        let a = Assistant::load(dir.path());
        let missed = a.take_due(&at(2026, 11, 25, 20, 0));
        assert_eq!(missed.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["pay rent", "call mom"], "in the order of time");
        assert_eq!(due_notice(&missed, true), Some(("Missed".into(), "pay rent; call mom".into())));
        assert_eq!(due_notice(&missed[..1], false), Some(("Reminder".into(), "pay rent".into())));
        assert_eq!(due_notice(&[], true), None);
        assert_eq!(a.upcoming(5).iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["later"]);
    }

    #[test]
    fn limits_hold_for_reminders_texts_and_lists() {
        let (_dir, a) = assistant();
        assert!(a.add_reminder("", &When::InMinutes(1), &now()).is_err());
        assert!(a.add_reminder(&"x".repeat(201), &When::InMinutes(1), &now()).is_err());
        assert!(a.add_reminder(&"x".repeat(200), &When::InMinutes(1), &now()).is_ok());
        assert_eq!(a.add_reminder("past", &When::At(at(2020, 1, 1, 0, 0)), &now()).unwrap_err(), PAST);
        for i in 0..MAX_REMINDERS - 1 {
            a.add_reminder(&format!("r{i}"), &When::InMinutes(1), &now()).unwrap();
        }
        assert_eq!(a.add_reminder("one too many", &When::InMinutes(1), &now()).unwrap_err(), "There are too many reminders");
        // Fired ones make room again.
        a.take_due(&at(2026, 11, 25, 15, 0));
        assert!(a.add_reminder("fits now", &When::InMinutes(1), &now()).is_ok());
        // Names that cannot be taken for one another: aaaabbbb, aaaacccc, ...
        let name = |i: usize| format!("{}{}", char::from(b'a' + (i / 7) as u8).to_string().repeat(4), char::from(b'a' + (i % 7) as u8 + 10).to_string().repeat(4));
        for i in 0..MAX_LISTS {
            a.list_add(&name(i), "x").unwrap();
        }
        assert_eq!(a.lists().len(), MAX_LISTS);
        assert_eq!(a.list_add("zzzzyyyy", "x").unwrap_err(), "There are too many lists");
        for i in 0..MAX_ITEMS - 1 {
            a.list_add(&name(0), &format!("item {i}")).unwrap();
        }
        assert_eq!(a.list_add(&name(0), "overflow").unwrap_err(), "The list is full");
    }

    #[test]
    fn lists_are_found_by_a_close_name_and_made_when_missing() {
        let (dir, a) = assistant();
        assert_eq!(a.list_add("shopping", "milk").unwrap(), "shopping");
        assert_eq!(a.list_add("Shopping", "bread").unwrap(), "shopping", "any case, the same list");
        assert_eq!(a.list_add("shoping", "eggs").unwrap(), "shopping", "a close spelling");
        assert_eq!(a.list_add("work", "report").unwrap(), "work");
        assert_eq!(a.lists().len(), 2);
        assert_eq!(list_text(&a.list_show("shop").unwrap()), "shopping: milk, bread, eggs");
        assert_eq!(a.list_remove_item("shopping", "brad").unwrap(), ("shopping".to_string(), "bread".to_string()));
        assert!(a.list_remove_item("shopping", "tractor").is_err() && a.list_remove_item("nothing", "x").is_err());
        assert_eq!(a.list_delete("work").unwrap(), "work");
        assert!(a.list_show("work").is_err());
        let reloaded = Assistant::load(dir.path());
        assert_eq!(list_text(&reloaded.list_show("shopping").unwrap()), "shopping: milk, eggs");
    }

    #[test]
    fn a_toggle_from_the_phone_changes_only_what_the_agent_issued() {
        let (dir, a) = assistant();
        a.list_add("shopping", "milk").unwrap();
        a.list_add("shopping", "bread").unwrap();
        let l = a.list_show("shopping").unwrap();
        assert!(a.toggle(&l.id, &l.items[0].id));
        assert_eq!(list_text(&a.list_show("shopping").unwrap()), "shopping: bread (done: milk)");
        assert!(a.toggle(&l.id, &l.items[0].id), "a second tap unmarks");
        // Foreign ids: unknown list, unknown item, an item of another list, junk.
        a.list_add("work", "report").unwrap();
        let w = a.list_show("work").unwrap();
        for (lid, iid) in [("0000000000000000", l.items[0].id.as_str()), (l.id.as_str(), "ffffffffffffffff"), (l.id.as_str(), w.items[0].id.as_str()), ("", ""), ("../x", "y")] {
            assert!(!a.toggle(lid, iid), "{lid} {iid}");
        }
        assert_eq!(list_text(&a.list_show("shopping").unwrap()), "shopping: milk, bread");
        assert_eq!(list_text(&Assistant::load(dir.path()).list_show("work").unwrap()), "work: report");
    }

    #[test]
    fn list_toggle_is_two_ids_and_nothing_else() {
        use crate::proto::ClientMsg;
        let m: ClientMsg = serde_json::from_str(r#"{"t":"list_toggle","listId":"0123456789abcdef","itemId":"fedcba9876543210"}"#).unwrap();
        assert_eq!(m, ClientMsg::ListToggle { list_id: "0123456789abcdef".into(), item_id: "fedcba9876543210".into() });
        assert!(serde_json::from_str::<ClientMsg>(r#"{"t":"list_toggle","listId":"a"}"#).is_err());
    }

    #[test]
    fn the_texts_read_out_well() {
        let (_dir, a) = assistant();
        let r1 = a.add_reminder("call mom", &When::Day { day: Day::Today, hour: 18, minute: 30 }, &now()).unwrap();
        let r2 = a.add_reminder("pay rent", &When::Day { day: Day::Tomorrow, hour: 9, minute: 0 }, &now()).unwrap();
        let r3 = a.add_reminder("dentist", &When::Day { day: Day::Weekday(4), hour: 8, minute: 15 }, &now()).unwrap();
        assert_eq!(reminders_text(&[r1, r2, r3], &now()), "18:30 call mom; tomorrow 09:00 pay rent; 27.11 08:15 dentist");
        assert_eq!(reminders_text(&[], &now()), "No reminders");
        a.list_add("empty", "x").unwrap();
        a.list_remove_item("empty", "x").unwrap();
        assert_eq!(list_text(&a.list_show("empty").unwrap()), "empty: empty");
    }

    #[test]
    fn broken_or_oversized_files_do_not_stop_the_agent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("reminders.json"), "{ nonsense").unwrap();
        let big: Vec<Reminder> = (0..600).map(|i| Reminder { id: format!("{i:016x}"), text: "x".into(), at: "2030-01-01T00:00".into(), done: false, repeat: None }).collect();
        std::fs::write(dir.path().join("lists.json"), serde_json::to_string(&ListsFile { lists: vec![] }).unwrap()).unwrap();
        let a = Assistant::load(dir.path());
        assert!(a.upcoming(10).is_empty());
        std::fs::write(dir.path().join("reminders.json"), serde_json::to_string(&RemindersFile { reminders: big }).unwrap()).unwrap();
        assert_eq!(Assistant::load(dir.path()).upcoming(1000).len(), MAX_REMINDERS);
    }

    #[test]
    fn the_window_edits_reminders_and_lists_by_issued_ids_only() {
        let (_d, a) = {
            let d = tempfile::tempdir().unwrap();
            let a = Assistant::load(d.path());
            (d, a)
        };
        let r = a.reminder_upsert(None, "  call   mom ", "2026-11-25T18:30", None, &now()).unwrap();
        assert_eq!((r.text.as_str(), r.at.as_str(), r.done), ("call mom", "2026-11-25T18:30", false));
        assert!(a.reminder_upsert(None, "x", "2026-11-25T09:00", None, &now()).is_err(), "a new time must be in the future");
        assert!(a.reminder_upsert(None, "x", "2028-01-01T09:00", None, &now()).is_err(), "at most a year ahead");
        assert!(a.reminder_upsert(None, "", "2026-11-26T09:00", None, &now()).is_err() && a.reminder_upsert(None, "x", "tomorrow", None, &now()).is_err());
        assert!(a.reminder_upsert(None, &"y".repeat(201), "2026-11-26T09:00", None, &now()).is_err());
        let edited = a.reminder_upsert(Some(&r.id), "call dad", "2026-11-25T18:30", None, &now()).unwrap();
        assert_eq!((edited.text.as_str(), edited.id.as_str()), ("call dad", r.id.as_str()), "the text changes, the time is kept");
        assert_eq!(a.reminder_upsert(Some("0000000000000000"), "x", "2026-11-26T09:00", None, &now()).unwrap_err(), "There is no such reminder");
        assert!(!a.reminder_delete("0000000000000000") && a.reminder_delete(&r.id));
        assert!(a.reminders_all().is_empty());

        let l = a.list_upsert(None, "Shopping", &[(None, "milk".into(), false), (None, "bread".into(), true)]).unwrap();
        assert_eq!((l.items.len(), l.items[1].done), (2, true));
        let (milk, bread) = (l.items[0].id.clone(), l.items[1].id.clone());
        let l2 = a.list_upsert(Some(&l.id), "Groceries", &[(Some(bread.clone()), "bread".into(), false), (Some(milk.clone()), "milk 2l".into(), false), (None, "eggs".into(), false)]).unwrap();
        assert_eq!((l2.name.as_str(), l2.items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>()), ("Groceries", vec!["bread", "milk 2l", "eggs"]));
        assert_eq!((l2.items[0].id.as_str(), l2.items[1].id.as_str()), (bread.as_str(), milk.as_str()), "kept ids");
        // Foreign ids and broken input change nothing.
        assert_eq!(a.list_upsert(Some("0000000000000000"), "x", &[]).unwrap_err(), "There is no such list");
        assert_eq!(a.list_upsert(Some(&l.id), "x", &[(Some("0000000000000000".into()), "x".into(), false)]).unwrap_err(), "There is no such item");
        assert_eq!(a.list_upsert(None, "New", &[(Some(milk.clone()), "x".into(), false)]).unwrap_err(), "There is no such item", "an id of another list");
        assert_eq!(a.list_upsert(Some(&l.id), "x", &[(Some(milk.clone()), "a".into(), false), (Some(milk.clone()), "b".into(), false)]).unwrap_err(), "An item is there twice");
        assert!(a.list_upsert(None, "Bad", &[(None, "".into(), false)]).is_err());
        assert_eq!(a.lists().len(), 1, "no half-made list is left");
        assert_eq!(a.lists()[0].name, "Groceries");
        let too_many: Vec<(Option<String>, String, bool)> = (0..=MAX_ITEMS).map(|i| (None, format!("i{i}"), false)).collect();
        assert_eq!(a.list_upsert(None, "Big", &too_many).unwrap_err(), "The list is full");
        for i in 0..MAX_LISTS - 1 {
            a.list_upsert(None, &format!("list {i}"), &[]).unwrap();
        }
        assert_eq!(a.list_upsert(None, "one more", &[]).unwrap_err(), "There are too many lists");
        assert!(!a.list_delete_id("0000000000000000") && a.list_delete_id(&l.id));
    }
}
