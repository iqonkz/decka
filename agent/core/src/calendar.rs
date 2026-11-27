//! T32.2: the calendar. An ICS link (Google, Outlook) read without any account: parsed here, recurring events expanded,
//! the next events published as the section `calendar`. The link of a meeting stays on the PC: the phone gets an id of an
//! action (`cal-join-...`) and presses it. Neither the ICS link nor the links of meetings are ever logged.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assistant::{days_from_civil, Local};

/// The ICS link is read this often.
pub const REFRESH: std::time::Duration = std::time::Duration::from_secs(10 * 60);
/// The list is looked at again this often (events that ended leave it).
pub const REBUILD: std::time::Duration = std::time::Duration::from_secs(30);
/// How far ahead the list looks.
pub const AHEAD_DAYS: i64 = 7;
pub const MAX_EVENTS: usize = 20;
pub const MAX_BYTES: usize = 5 * 1024 * 1024;
/// The prefix of the action a phone presses to join a meeting.
pub const JOIN_PREFIX: &str = "cal-join-";

const DAY: i64 = 86_400;

/// One event of the section, in the wall clock of the PC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    /// `HH:mm`; empty for an event of a whole day.
    pub start_time: String,
    pub end_time: String,
    /// `YYYY-MM-DD` of the start (for an event that runs now over several days: today).
    pub day: String,
    /// Unix milliseconds.
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meeting_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_action_id: Option<String>,
}

/// The section `calendar`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarInfo {
    /// Unix milliseconds of the last successful reading; 0 = none yet.
    pub updated_at: i64,
    pub events: Vec<CalendarEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The wall clock of the PC, injectable for tests. Both are seconds since 1970 (`civil` reads the wall clock as if UTC).
pub trait Zone: Send + Sync {
    fn to_utc(&self, civil: i64) -> i64;
    fn to_civil(&self, utc: i64) -> i64;
}

/// A constant offset in seconds east of Greenwich.
pub struct Fixed(pub i64);

impl Zone for Fixed {
    fn to_utc(&self, civil: i64) -> i64 {
        civil - self.0
    }
    fn to_civil(&self, utc: i64) -> i64 {
        utc + self.0
    }
}

/// The zone Windows is set to, with its summer time.
pub struct PcZone;

#[cfg(windows)]
impl Zone for PcZone {
    fn to_utc(&self, civil: i64) -> i64 {
        convert(civil, false).unwrap_or(civil)
    }
    fn to_civil(&self, utc: i64) -> i64 {
        convert(utc, true).unwrap_or(utc)
    }
}

#[cfg(windows)]
fn convert(secs: i64, to_local: bool) -> Option<i64> {
    use windows::Win32::Foundation::SYSTEMTIME;
    use windows::Win32::System::Time::{SystemTimeToTzSpecificLocalTime, TzSpecificLocalTimeToSystemTime};

    let (year, month, day) = civil_from_days(secs.div_euclid(DAY));
    let in_day = secs.rem_euclid(DAY);
    let from = SYSTEMTIME { wYear: u16::try_from(year).ok()?, wMonth: month as u16, wDay: day as u16, wHour: (in_day / 3600) as u16, wMinute: (in_day / 60 % 60) as u16, wSecond: (in_day % 60) as u16, ..Default::default() };
    let mut to = SYSTEMTIME::default();
    // SAFETY: both pointers are to valid structs for the length of the call; no time zone = the current one.
    let done = unsafe {
        if to_local {
            SystemTimeToTzSpecificLocalTime(None, &from, &mut to)
        } else {
            TzSpecificLocalTimeToSystemTime(None, &from, &mut to)
        }
    };
    done.ok()?;
    Some(days_from_civil(i32::from(to.wYear), u32::from(to.wMonth), u32::from(to.wDay)) * DAY + i64::from(to.wHour) * 3600 + i64::from(to.wMinute) * 60 + i64::from(to.wSecond))
}

#[cfg(not(windows))]
impl Zone for PcZone {
    fn to_utc(&self, civil: i64) -> i64 {
        civil
    }
    fn to_civil(&self, utc: i64) -> i64 {
        utc
    }
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let l = Local::from_minutes(days * 1440);
    (l.year, l.month, l.day)
}

/// Monday = 0.
fn weekday(days: i64) -> i64 {
    (days + 3).rem_euclid(7)
}

// ---- Reading the file ------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum ZoneRef {
    Utc,
    /// The wall clock of the PC (a "floating" time).
    Floating,
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
struct When {
    civil: i64,
    zone: ZoneRef,
    date_only: bool,
}

#[derive(Debug, Clone, Default)]
struct Rrule {
    freq: String,
    interval: i64,
    count: Option<u32>,
    until: Option<When>,
    /// (ordinal, weekday); ordinal 0 = every such weekday.
    by_day: Vec<(i64, i64)>,
    by_month_day: Vec<i64>,
    by_month: Vec<i64>,
    by_set_pos: Vec<i64>,
    week_start: i64,
}

#[derive(Debug, Clone)]
struct Event {
    uid: String,
    title: String,
    location: String,
    join: Option<(&'static str, String)>,
    start: When,
    /// Seconds.
    length: i64,
    rrule: Option<Rrule>,
    extra: Vec<When>,
    excluded: Vec<When>,
    replaces: Option<When>,
    cancelled: bool,
}

#[derive(Debug, Clone)]
struct Observance {
    start: i64,
    offset: i64,
    rrule: Option<Rrule>,
}

/// A parsed calendar.
#[derive(Debug, Clone, Default)]
pub struct Calendar {
    events: Vec<Event>,
    zones: HashMap<String, Vec<Observance>>,
}

/// `NAME;PARAM=x;OTHER="a:b":value`.
struct Prop {
    name: String,
    params: HashMap<String, String>,
    value: String,
}

fn parse_prop(line: &str) -> Option<Prop> {
    let mut quoted = false;
    let colon = line.char_indices().find(|(_, c)| {
        if *c == '"' {
            quoted = !quoted;
        }
        *c == ':' && !quoted
    })?.0;
    let mut parts = line[..colon].split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts.filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_ascii_uppercase(), v.trim_matches('"').to_string())).collect();
    Some(Prop { name, params, value: line[colon + 1..].to_string() })
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// `20261127`, `20261127T093000`, `20261127T093000Z`.
fn parse_when(value: &str, tzid: Option<&String>) -> Option<When> {
    let v = value.trim();
    let digits = |from: usize, to: usize| v.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (digits(0, 4)? as i32, digits(4, 6)? as u32, digits(6, 8)? as u32);
    Local::new(year, month, day, 0, 0)?;
    let days = days_from_civil(year, month, day);
    if v.len() == 8 {
        return Some(When { civil: days * DAY, zone: ZoneRef::Floating, date_only: true });
    }
    let (h, m, s) = (digits(9, 11)?, digits(11, 13)?, digits(13, 15)?);
    if v.as_bytes().get(8) != Some(&b'T') || h > 23 || m > 59 || s > 60 {
        return None;
    }
    let zone = if v.ends_with('Z') { ZoneRef::Utc } else { tzid.map_or(ZoneRef::Floating, |t| ZoneRef::Named(t.clone())) };
    Some(When { civil: days * DAY + h * 3600 + m * 60 + s, zone, date_only: false })
}

/// `P1DT2H30M`, `PT45M`, `P1W` in seconds.
fn parse_duration(value: &str) -> Option<i64> {
    let (mut total, mut number) = (0i64, 0i64);
    for c in value.trim().trim_start_matches('+').strip_prefix('P')?.chars() {
        match c {
            '0'..='9' => number = number.saturating_mul(10).saturating_add(i64::from(c as u8 - b'0')),
            'T' => {}
            other => {
                let unit = match other {
                    'W' => 7 * DAY,
                    'D' => DAY,
                    'H' => 3600,
                    'M' => 60,
                    'S' => 1,
                    _ => return None,
                };
                total = total.saturating_add(number.saturating_mul(unit));
                number = 0;
            }
        }
    }
    Some(total)
}

fn parse_weekday(code: &str) -> Option<i64> {
    ["MO", "TU", "WE", "TH", "FR", "SA", "SU"].iter().position(|d| *d == code).map(|i| i as i64)
}

fn parse_rrule(value: &str, tzid: Option<&String>) -> Option<Rrule> {
    let mut r = Rrule { interval: 1, ..Default::default() };
    let numbers = |v: &str| v.split(',').filter_map(|n| n.trim().parse::<i64>().ok()).collect::<Vec<_>>();
    for part in value.split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k {
            "FREQ" => r.freq = v.to_string(),
            "INTERVAL" => r.interval = v.parse::<i64>().ok()?.max(1),
            "COUNT" => r.count = v.parse().ok(),
            "UNTIL" => r.until = parse_when(v, tzid),
            "BYDAY" => {
                r.by_day = v.split(',').filter_map(|d| {
                    let at = d.len().checked_sub(2)?;
                    let ordinal = d.get(..at)?;
                    Some((if ordinal.is_empty() { 0 } else { ordinal.parse().ok()? }, parse_weekday(d.get(at..)?)?))
                }).collect();
            }
            "BYMONTHDAY" => r.by_month_day = numbers(v),
            "BYMONTH" => r.by_month = numbers(v),
            "BYSETPOS" => r.by_set_pos = numbers(v),
            "WKST" => r.week_start = parse_weekday(v).unwrap_or(0),
            _ => {}
        }
    }
    ["DAILY", "WEEKLY", "MONTHLY", "YEARLY"].contains(&r.freq.as_str()).then_some(r)
}

/// The service of a meeting link, `None` for any other address. Only `https` to the hosts of the three services.
pub fn meeting_of(link: &str) -> Option<(&'static str, String)> {
    let url = url::Url::parse(link).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    // Outlook wraps links of other services.
    if host.ends_with(".safelinks.protection.outlook.com") {
        let inner = url.query_pairs().find(|(k, _)| k == "url")?.1.into_owned();
        return if inner.contains(".safelinks.") { None } else { meeting_of(&inner) };
    }
    let path = url.path();
    let provider = if (host == "zoom.us" || host.ends_with(".zoom.us")) && ["/j/", "/my/", "/w/", "/s/", "/meeting/"].iter().any(|p| path.starts_with(p)) {
        "Zoom"
    } else if host == "meet.google.com" && path.len() > 1 {
        "Google Meet"
    } else if (host == "teams.microsoft.com" || host == "teams.live.com") && (path.starts_with("/l/meetup-join") || path.starts_with("/meet")) {
        "Teams"
    } else {
        return None;
    };
    Some((provider, url.to_string()))
}

fn find_meeting(texts: &[String]) -> Option<(&'static str, String)> {
    texts.iter().find_map(|text| {
        text.match_indices("https://").find_map(|(at, _)| {
            let rest = &text[at..];
            let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '|' | ')' | ']')).unwrap_or(rest.len());
            meeting_of(rest[..end].trim_end_matches(['.', ',', ';']))
        })
    })
}

#[derive(Default)]
struct Draft {
    props: Vec<Prop>,
}

impl Draft {
    fn first(&self, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.name == name)
    }

    fn when(&self, name: &str) -> Option<When> {
        self.first(name).and_then(|p| parse_when(&p.value, p.params.get("TZID")))
    }

    fn list(&self, name: &str) -> Vec<When> {
        self.props.iter().filter(|p| p.name == name).flat_map(|p| p.value.split(',').filter_map(|v| parse_when(v, p.params.get("TZID")))).collect()
    }

    fn text(&self, name: &str) -> String {
        self.first(name).map(|p| unescape(&p.value).trim().to_string()).unwrap_or_default()
    }

    fn event(&self) -> Option<Event> {
        let start = self.when("DTSTART")?;
        let length = match (self.when("DTEND"), self.first("DURATION").and_then(|p| parse_duration(&p.value))) {
            // Both in the zone of the start in practice.
            (Some(end), _) => end.civil - start.civil,
            (None, Some(d)) => d,
            (None, None) => if start.date_only { DAY } else { 0 },
        };
        // Where a link of a meeting can stand, the most precise first.
        let places: Vec<String> = ["X-GOOGLE-CONFERENCE", "X-MICROSOFT-SKYPETEAMSMEETINGURL", "URL", "LOCATION", "DESCRIPTION"].iter().map(|n| self.text(n)).collect();
        Some(Event {
            uid: self.text("UID"),
            title: self.text("SUMMARY"),
            location: self.text("LOCATION"),
            join: find_meeting(&places),
            length: length.clamp(0, 366 * DAY),
            rrule: self.first("RRULE").and_then(|p| parse_rrule(&p.value, self.first("DTSTART").and_then(|d| d.params.get("TZID")))),
            extra: self.list("RDATE"),
            excluded: self.list("EXDATE"),
            replaces: self.when("RECURRENCE-ID"),
            cancelled: self.text("STATUS").eq_ignore_ascii_case("CANCELLED"),
            start,
        })
    }
}

/// Reads an ICS file. An event that cannot be read is left out.
pub fn parse(text: &str) -> Result<Calendar, String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        match (raw.strip_prefix(' ').or_else(|| raw.strip_prefix('\t')), lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => lines.push(raw.trim_end_matches('\r').to_string()),
        }
    }
    if !lines.iter().any(|l| l.trim_start_matches('\u{feff}').trim().eq_ignore_ascii_case("BEGIN:VCALENDAR")) {
        return Err("This is not a calendar (ICS) link".into());
    }
    let mut calendar = Calendar::default();
    let mut stack: Vec<String> = Vec::new();
    let (mut event, mut observance, mut zone) = (Draft::default(), Draft::default(), (String::new(), Vec::new()));
    for line in &lines {
        let Some(prop) = parse_prop(line) else { continue };
        let value = prop.value.trim().to_ascii_uppercase();
        match prop.name.as_str() {
            "BEGIN" => {
                match value.as_str() {
                    "VEVENT" => event = Draft::default(),
                    "VTIMEZONE" => zone = (String::new(), Vec::new()),
                    "STANDARD" | "DAYLIGHT" => observance = Draft::default(),
                    _ => {}
                }
                stack.push(value);
            }
            "END" => {
                stack.pop();
                match value.as_str() {
                    "VEVENT" => calendar.events.extend(event.event()),
                    "STANDARD" | "DAYLIGHT" => {
                        let offset = observance.first("TZOFFSETTO").and_then(|p| parse_offset(&p.value));
                        if let (Some(start), Some(offset)) = (observance.when("DTSTART"), offset) {
                            zone.1.push(Observance { start: start.civil, offset, rrule: observance.first("RRULE").and_then(|p| parse_rrule(&p.value, None)) });
                        }
                    }
                    "VTIMEZONE" if !zone.0.is_empty() => {
                        calendar.zones.insert(zone.0.clone(), std::mem::take(&mut zone.1));
                    }
                    _ => {}
                }
            }
            _ => match stack.last().map(String::as_str) {
                Some("VEVENT") => event.props.push(prop),
                Some("STANDARD" | "DAYLIGHT") => observance.props.push(prop),
                Some("VTIMEZONE") if prop.name == "TZID" => zone.0 = prop.value.trim().to_string(),
                _ => {}
            },
        }
    }
    Ok(calendar)
}

/// `+0500`, `-033000`.
fn parse_offset(value: &str) -> Option<i64> {
    let v = value.trim();
    let sign = if v.starts_with('-') { -1 } else { 1 };
    let digits = v.trim_start_matches(['+', '-']);
    let part = |a: usize| digits.get(a..a + 2).and_then(|p| p.parse::<i64>().ok());
    Some(sign * (part(0)? * 3600 + part(2)? * 60 + part(4).unwrap_or(0)))
}

// ---- Repeating events ------------------------------------------------------------------------------------------

/// Whether the day (since 1970) matches the month rules of `r`; `anchor` is the day of the first event.
fn in_month(r: &Rrule, day: i64, anchor: i64) -> bool {
    let (year, month, dom) = civil_from_days(day);
    let first = days_from_civil(year, month, 1);
    let next = if month == 12 { days_from_civil(year + 1, 1, 1) } else { days_from_civil(year, month + 1, 1) };
    let length = next - first;
    let by_day = |d: i64| {
        r.by_day.iter().any(|(ordinal, wd)| {
            let at = d - first;
            weekday(d) == *wd && match ordinal {
                0 => true,
                n if *n > 0 => at / 7 + 1 == *n,
                n => (length - 1 - at) / 7 + 1 == -*n,
            }
        })
    };
    let by_month_day = |d: i64| r.by_month_day.iter().any(|n| if *n > 0 { d - first + 1 == *n } else { next - d == -*n });
    if r.by_day.is_empty() && r.by_month_day.is_empty() {
        return dom == civil_from_days(anchor).2;
    }
    let matches = |d: i64| (r.by_day.is_empty() || by_day(d)) && (r.by_month_day.is_empty() || by_month_day(d));
    if !matches(day) {
        return false;
    }
    if r.by_set_pos.is_empty() {
        return true;
    }
    let set: Vec<i64> = (first..next).filter(|d| matches(*d)).collect();
    r.by_set_pos.iter().any(|p| {
        let index = if *p > 0 { *p - 1 } else { set.len() as i64 + *p };
        usize::try_from(index).ok().and_then(|i| set.get(i)) == Some(&day)
    })
}

fn occurs(r: &Rrule, day: i64, anchor: i64) -> bool {
    let (year, month, _) = civil_from_days(day);
    let (y0, m0, _) = civil_from_days(anchor);
    if !r.by_month.is_empty() && !r.by_month.contains(&i64::from(month)) {
        return false;
    }
    match r.freq.as_str() {
        "DAILY" => (day - anchor) % r.interval == 0 && (r.by_day.is_empty() || r.by_day.iter().any(|d| d.1 == weekday(day))) && (r.by_month_day.is_empty() || in_month(r, day, anchor)),
        "WEEKLY" => {
            let week = |d: i64| (d - (weekday(d) - r.week_start).rem_euclid(7)).div_euclid(7);
            let named = if r.by_day.is_empty() { weekday(day) == weekday(anchor) } else { r.by_day.iter().any(|d| d.1 == weekday(day)) };
            named && (week(day) - week(anchor)) % r.interval == 0
        }
        "MONTHLY" => (i64::from(year - y0) * 12 + i64::from(month) - i64::from(m0)) % r.interval == 0 && in_month(r, day, anchor),
        "YEARLY" => (i64::from(year - y0)) % r.interval == 0 && (!r.by_month.is_empty() || month == m0) && in_month(r, day, anchor),
        _ => false,
    }
}

impl Calendar {
    /// The offset of a named zone at a wall clock time, from the rules of the file.
    fn offset(&self, name: &str, civil: i64) -> Option<i64> {
        let zone = self.zones.get(name)?;
        let year = civil_from_days(civil.div_euclid(DAY)).0;
        let mut latest: Option<(i64, i64)> = None;
        for o in zone {
            let starts: Vec<i64> = match &o.rrule {
                None => vec![o.start],
                Some(r) => {
                    let anchor = o.start.div_euclid(DAY);
                    let time = o.start.rem_euclid(DAY);
                    [year - 1, year]
                        .iter()
                        .flat_map(|y| {
                            let (from, to) = (days_from_civil(*y, 1, 1), days_from_civil(y + 1, 1, 1));
                            (from.max(anchor)..to).filter(move |d| occurs(r, *d, anchor)).map(move |d| d * DAY + time)
                        })
                        // A rule that ended: its UNTIL is given in UTC, a day of slack does no harm.
                        .filter(|at| r.until.as_ref().is_none_or(|u| *at <= u.civil + DAY))
                        .collect()
                }
            };
            for at in starts.into_iter().filter(|at| *at <= civil) {
                if latest.is_none_or(|l| at > l.0) {
                    latest = Some((at, o.offset));
                }
            }
        }
        latest.map(|l| l.1).or_else(|| zone.iter().min_by_key(|o| o.start).map(|o| o.offset))
    }

    fn utc(&self, when: &When, civil: i64, pc: &dyn Zone) -> i64 {
        match &when.zone {
            ZoneRef::Utc => civil,
            ZoneRef::Floating => pc.to_utc(civil),
            ZoneRef::Named(name) => self.offset(name, civil).map_or_else(|| pc.to_utc(civil), |o| civil - o),
        }
    }

    /// The wall clock starts of an event whose instances may reach into `[from, to)` (days since 1970, in its own zone).
    fn starts(&self, e: &Event, from: i64, to: i64, pc: &dyn Zone) -> Vec<i64> {
        let first = e.start.civil.div_euclid(DAY);
        let time = e.start.civil.rem_euclid(DAY);
        let reach = e.length / DAY + 2;
        let mut out: Vec<i64> = Vec::new();
        match &e.rrule {
            None => out.push(e.start.civil),
            Some(r) => {
                // Only a counted rule has to be walked from its beginning.
                let mut day = if r.count.is_some() { first } else { first.max(from - reach) };
                let mut seen = 0u32;
                while day < to && day - first < 366 * 60 {
                    if day == first || occurs(r, day, first) {
                        let civil = day * DAY + time;
                        let ended = r.until.as_ref().is_some_and(|u| if u.date_only { day > u.civil.div_euclid(DAY) } else { self.utc(&e.start, civil, pc) > self.utc(u, u.civil, pc) });
                        seen += 1;
                        if ended || r.count.is_some_and(|c| seen > c) {
                            break;
                        }
                        out.push(civil);
                    }
                    day += 1;
                }
            }
        }
        out.extend(e.extra.iter().map(|w| w.civil));
        out.retain(|c| (from - reach..to).contains(&c.div_euclid(DAY)));
        out
    }

    /// The events that have not ended at `now` (unix seconds) and start within `AHEAD_DAYS`, with the links to join them
    /// by action id. By day; within a day those with a time come first.
    pub fn upcoming(&self, now: i64, pc: &dyn Zone) -> (Vec<CalendarEvent>, HashMap<String, String>) {
        let today = pc.to_civil(now).div_euclid(DAY);
        let (from, to) = (today - 1, today + AHEAD_DAYS + 1);
        // Instances that another entry of the same series replaces (moved or cancelled).
        let replaced: HashSet<(&str, i64)> = self.events.iter().filter_map(|e| e.replaces.as_ref().map(|w| (e.uid.as_str(), self.utc(w, w.civil, pc)))).collect();
        let mut found: Vec<(CalendarEvent, Option<String>)> = Vec::new();
        for e in self.events.iter().filter(|e| !e.cancelled) {
            let excluded: HashSet<i64> = e.excluded.iter().map(|w| self.utc(w, w.civil, pc)).collect();
            for civil in self.starts(e, from, to, pc) {
                let start = self.utc(&e.start, civil, pc);
                if excluded.contains(&start) || (e.replaces.is_none() && replaced.contains(&(e.uid.as_str(), start))) {
                    continue;
                }
                let end = if e.start.date_only { pc.to_utc(civil + e.length.max(DAY)) } else { start + e.length };
                // A meeting without a length stays for a quarter of an hour.
                if end.max(start + 900) <= now {
                    continue;
                }
                let local = if e.start.date_only { civil } else { pc.to_civil(start) };
                let day = local.div_euclid(DAY).max(today);
                if day >= today + AHEAD_DAYS {
                    continue;
                }
                let key: String = Sha256::digest(format!("{}|{}|{start}", e.uid, e.title)).iter().take(8).map(|b| format!("{b:02x}")).collect();
                let clock = |at: i64| format!("{:02}:{:02}", at.rem_euclid(DAY) / 3600, at.rem_euclid(DAY) / 60 % 60);
                let (y, m, d) = civil_from_days(day);
                let location = e.location.lines().next().unwrap_or("");
                let join = e.join.clone();
                found.push((
                    CalendarEvent {
                        id: format!("cal-{key}"),
                        title: e.title.chars().take(120).collect(),
                        start_time: if e.start.date_only { String::new() } else { clock(local) },
                        end_time: if e.start.date_only { String::new() } else { clock(pc.to_civil(end)) },
                        day: format!("{y:04}-{m:02}-{d:02}"),
                        start: start * 1000,
                        end: end * 1000,
                        all_day: e.start.date_only,
                        // An address of a page says nothing on a tile and may be the link itself.
                        location: (!location.is_empty() && !location.contains("://")).then(|| location.chars().take(80).collect()),
                        meeting_provider: join.as_ref().map(|j| j.0.to_string()),
                        join_action_id: join.as_ref().map(|_| format!("{JOIN_PREFIX}{key}")),
                    },
                    join.map(|j| j.1),
                ));
            }
        }
        found.sort_by(|a, b| (&a.0.day, a.0.all_day, a.0.start, &a.0.title).cmp(&(&b.0.day, b.0.all_day, b.0.start, &b.0.title)));
        found.dedup_by(|a, b| a.0.id == b.0.id);
        found.truncate(MAX_EVENTS);
        let links = found.iter().filter_map(|(e, link)| Some((e.join_action_id.clone()?, link.clone()?))).collect();
        (found.into_iter().map(|f| f.0).collect(), links)
    }
}

// ---- What is said ----------------------------------------------------------------------------------------------

/// What the voice asks the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    Today,
    Tomorrow,
    Next,
}

impl Ask {
    pub fn parse(s: &str) -> Option<Ask> {
        match s {
            "today" => Some(Ask::Today),
            "tomorrow" => Some(Ask::Tomorrow),
            "next" => Some(Ask::Next),
            _ => None,
        }
    }
}

fn events_of(section: &serde_json::Value) -> Vec<CalendarEvent> {
    section.get("events").and_then(|e| serde_json::from_value(e.clone()).ok()).unwrap_or_default()
}

/// The events of the day `YYYY-MM-DD` that have not ended.
pub fn of_day(section: &serde_json::Value, day: &str, unix_ms: i64) -> Vec<CalendarEvent> {
    events_of(section).into_iter().filter(|e| e.day == day && (e.all_day || e.end.max(e.start + 900_000) > unix_ms)).collect()
}

/// The answer from the section the agent keeps: no model is needed, nothing is invented.
pub fn answer(section: &serde_json::Value, ask: Ask, now: &Local, unix_ms: i64) -> crate::voice::status::Answer {
    let said = |key: &'static str, args: Vec<(&str, String)>, text: String| crate::voice::status::Answer { key, args: args.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), text };
    if section.is_null() {
        return said("voice.reply.calendar_off", Vec::new(), "The calendar is not connected".into());
    }
    let tomorrow = Local::from_minutes(now.minutes() + 1440).iso()[..10].to_string();
    let today = now.iso()[..10].to_string();
    if ask == Ask::Next {
        let next = events_of(section).into_iter().find(|e| !e.all_day && e.start > unix_ms);
        let Some(e) = next else { return said("voice.reply.calendar_none_next", Vec::new(), "No meetings ahead".into()) };
        let mut args = vec![("time", e.start_time.clone()), ("title", e.title.clone())];
        return if e.day == today {
            said("voice.reply.calendar_next", args, format!("Next: {} at {}", e.title, e.start_time))
        } else if e.day == tomorrow {
            said("voice.reply.calendar_next_tomorrow", args, format!("Next: {} tomorrow at {}", e.title, e.start_time))
        } else {
            let date = format!("{}.{}", e.day.get(8..10).unwrap_or(""), e.day.get(5..7).unwrap_or(""));
            args.push(("date", date.clone()));
            said("voice.reply.calendar_next_date", args, format!("Next: {} on {date} at {}", e.title, e.start_time))
        };
    }
    let day = if ask == Ask::Today { &today } else { &tomorrow };
    let list: Vec<String> = of_day(section, day, unix_ms).iter().take(6).map(|e| format!("{} {}", e.start_time, e.title).trim().to_string()).collect();
    match (ask, list.is_empty()) {
        (Ask::Today, true) => said("voice.reply.calendar_none_today", Vec::new(), "Nothing else in the calendar today".into()),
        (_, true) => said("voice.reply.calendar_none_tomorrow", Vec::new(), "Nothing in the calendar tomorrow".into()),
        (ask, false) => {
            let list = list.join(", ");
            let (key, word) = if ask == Ask::Today { ("voice.reply.calendar_today", "Today") } else { ("voice.reply.calendar_tomorrow", "Tomorrow") };
            said(key, vec![("list", list.clone())], format!("{word}: {list}"))
        }
    }
}

// ---- The link --------------------------------------------------------------------------------------------------

/// What the user pasted -> the address that is read: `https` only (`webcal` is the same address), no user in it.
pub fn clean_link(input: &str) -> Result<String, String> {
    let text = input.trim();
    let text = text.strip_prefix("webcal://").or_else(|| text.strip_prefix("webcals://")).map_or_else(|| text.to_string(), |rest| format!("https://{rest}"));
    let url = url::Url::parse(&text).map_err(|_| "This is not a link".to_string())?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || text.len() > 2048 {
        return Err("The calendar link must start with https://".into());
    }
    Ok(url.to_string())
}

/// Downloads the file; the message of an error never holds the link.
pub async fn fetch(link: &str) -> Result<String, String> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 5 || attempt.url().scheme() != "https" {
            attempt.error("redirect refused")
        } else {
            attempt.follow()
        }
    });
    let client = reqwest::Client::builder()
        .redirect(policy)
        .user_agent(format!("decka-agent/{}", crate::VERSION))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.without_url().to_string())?;
    let mut response = client.get(link).send().await.map_err(|_| "The calendar did not answer".to_string())?;
    if !response.status().is_success() {
        return Err(format!("The calendar answered HTTP {}", response.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "The calendar did not answer".to_string())? {
        if body.len() + chunk.len() > MAX_BYTES {
            return Err("The calendar is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, hour: i64, minute: i64) -> i64 {
        days_from_civil(2026, 11, day) * DAY + hour * 3600 + minute * 60
    }

    fn calendar(events: &str) -> Calendar {
        parse(&format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{events}END:VCALENDAR\r\n")).unwrap()
    }

    fn event(body: &str) -> String {
        format!("BEGIN:VEVENT\r\n{body}\r\nEND:VEVENT\r\n")
    }

    fn titles(list: &[CalendarEvent]) -> Vec<String> {
        list.iter().map(|e| format!("{} {} {}", e.day, e.start_time, e.title)).collect()
    }

    /// 2026-11-27 is a Friday; the PC is five hours east of Greenwich.
    const PC: Fixed = Fixed(5 * 3600);

    #[test]
    fn a_utc_event_is_shown_in_the_time_of_the_pc_and_ends_leave_the_list() {
        let c = calendar(&event("UID:1\r\nSUMMARY:Stand\r\n -up\\, daily\r\nDTSTART:20261127T040000Z\r\nDTEND:20261127T043000Z\r\nLOCATION:Room 5"));
        let (list, _) = c.upcoming(PC.to_utc(at(27, 8, 0)), &PC);
        assert_eq!(titles(&list), ["2026-11-27 09:00 Stand-up, daily"]);
        assert_eq!((list[0].end_time.as_str(), list[0].location.as_deref(), list[0].join_action_id.as_deref()), ("09:30", Some("Room 5"), None));
        assert!(c.upcoming(PC.to_utc(at(27, 9, 31)), &PC).0.is_empty());
        assert_eq!(c.upcoming(PC.to_utc(at(27, 9, 10)), &PC).0.len(), 1, "a running event stays");
    }

    #[test]
    fn a_named_zone_uses_the_rules_of_the_file_with_summer_time() {
        let zone = "BEGIN:VTIMEZONE\r\nTZID:Europe/Berlin\r\nBEGIN:DAYLIGHT\r\nDTSTART:19700329T020000\r\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0200\r\nEND:DAYLIGHT\r\nBEGIN:STANDARD\r\nDTSTART:19701025T030000\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n";
        let c = calendar(&format!("{zone}{}", event("UID:w\r\nSUMMARY:Winter\r\nDTSTART;TZID=Europe/Berlin:20261127T100000\r\nDURATION:PT1H")));
        // 10:00 in Berlin in November is 09:00 UTC = 14:00 here.
        assert_eq!(titles(&c.upcoming(PC.to_utc(at(27, 0, 0)), &PC).0), ["2026-11-27 14:00 Winter"]);
        assert_eq!(c.offset("Europe/Berlin", days_from_civil(2026, 7, 1) * DAY), Some(7200));
        assert_eq!(c.offset("Europe/Berlin", days_from_civil(2026, 12, 1) * DAY), Some(3600));
    }

    #[test]
    fn weekly_events_with_exceptions_and_a_moved_instance() {
        let series = event("UID:s\r\nSUMMARY:Sync\r\nDTSTART:20261102T100000\r\nDTEND:20261102T103000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\r\nEXDATE:20261130T100000");
        let moved = event("UID:s\r\nSUMMARY:Sync moved\r\nRECURRENCE-ID:20261202T100000\r\nDTSTART:20261202T150000\r\nDTEND:20261202T153000");
        let (list, _) = calendar(&format!("{series}{moved}")).upcoming(PC.to_utc(at(27, 8, 0)), &PC);
        assert_eq!(titles(&list), ["2026-11-27 10:00 Sync", "2026-12-02 15:00 Sync moved"]);
    }

    #[test]
    fn counted_and_ended_series_stop() {
        let counted = event("UID:c\r\nSUMMARY:Three\r\nDTSTART:20261126T120000\r\nRRULE:FREQ=DAILY;COUNT=3");
        let until = event("UID:u\r\nSUMMARY:Until\r\nDTSTART:20261120T130000\r\nRRULE:FREQ=DAILY;INTERVAL=7;UNTIL=20261127T235959");
        let old = event("UID:o\r\nSUMMARY:Old\r\nDTSTART:20200101T130000\r\nRRULE:FREQ=DAILY;COUNT=5");
        let (list, _) = calendar(&format!("{counted}{until}{old}")).upcoming(PC.to_utc(at(27, 8, 0)), &PC);
        assert_eq!(titles(&list), ["2026-11-27 12:00 Three", "2026-11-27 13:00 Until", "2026-11-28 12:00 Three"]);
    }

    #[test]
    fn monthly_and_yearly_rules() {
        let last_friday = event("UID:m\r\nSUMMARY:Last Friday\r\nDTSTART:20260130T170000\r\nRRULE:FREQ=MONTHLY;BYDAY=-1FR");
        let set = event("UID:p\r\nSUMMARY:Last workday\r\nDTSTART:20260130T090000\r\nRRULE:FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1");
        let birthday = event("UID:b\r\nSUMMARY:Birthday\r\nDTSTART;VALUE=DATE:19901128\r\nRRULE:FREQ=YEARLY");
        let (list, _) = calendar(&format!("{last_friday}{set}{birthday}")).upcoming(PC.to_utc(at(27, 8, 0)), &PC);
        assert_eq!(titles(&list), ["2026-11-27 17:00 Last Friday", "2026-11-28  Birthday", "2026-11-30 09:00 Last workday"]);
        assert!(list[1].all_day);
    }

    #[test]
    fn an_all_day_event_over_several_days_stands_under_today_after_the_timed_ones() {
        let trip = event("UID:t\r\nSUMMARY:Trip\r\nDTSTART;VALUE=DATE:20261125\r\nDTEND;VALUE=DATE:20261129");
        let call = event("UID:k\r\nSUMMARY:Call\r\nDTSTART:20261127T110000");
        let gone = event("UID:g\r\nSUMMARY:Gone\r\nDTSTART;VALUE=DATE:20261126");
        let off = event("UID:x\r\nSUMMARY:Off\r\nSTATUS:CANCELLED\r\nDTSTART:20261127T120000");
        let (list, _) = calendar(&format!("{trip}{call}{gone}{off}")).upcoming(PC.to_utc(at(27, 8, 0)), &PC);
        assert_eq!(titles(&list), ["2026-11-27 11:00 Call", "2026-11-27  Trip"]);
    }

    #[test]
    fn the_link_of_a_meeting_stays_on_the_pc_and_only_the_three_services_count() {
        let c = calendar(&event("UID:z\r\nSUMMARY:Call\r\nDTSTART:20261127T110000\r\nDESCRIPTION:Join: https://evil.example/zoom.us/j/1\\nhttps://us02web.zoom.us/j/123?pwd=abc."));
        let (list, links) = c.upcoming(PC.to_utc(at(27, 8, 0)), &PC);
        let id = list[0].join_action_id.clone().unwrap();
        assert!(id.starts_with(JOIN_PREFIX) && id.len() == 25);
        assert_eq!((list[0].meeting_provider.as_deref(), links[&id].as_str()), (Some("Zoom"), "https://us02web.zoom.us/j/123?pwd=abc"));
        assert!(!serde_json::to_string(&list).unwrap().contains("zoom.us"), "no address goes to the phone");
        for ok in ["https://meet.google.com/abc-defg-hij", "https://teams.microsoft.com/l/meetup-join/19%3a", "https://eur01.safelinks.protection.outlook.com/?url=https%3A%2F%2Fzoom.us%2Fj%2F5&data=x"] {
            assert!(meeting_of(ok).is_some(), "{ok}");
        }
        for bad in ["http://zoom.us/j/1", "https://zoom.us.evil.example/j/1", "https://evilzoom.us/j/1", "https://u@zoom.us/j/1", "https://meet.google.com/", "file:///c:/x", "https://zoom.us/download"] {
            assert!(meeting_of(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn only_a_calendar_is_read_and_only_over_https() {
        assert!(parse("<html>sign in</html>").is_err());
        assert_eq!(clean_link(" webcal://example.com/a.ics ").unwrap(), "https://example.com/a.ics");
        for bad in ["http://example.com/a.ics", "file:///c:/a.ics", "https://me:pw@example.com/a.ics", "calendar"] {
            assert!(clean_link(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_voice_says_today_tomorrow_and_the_next_meeting() {
        let events = [event("UID:a\r\nSUMMARY:Standup\r\nDTSTART:20261127T090000"), event("UID:b\r\nSUMMARY:Review\r\nDTSTART:20261128T100000"), event("UID:c\r\nSUMMARY:Fair\r\nDTSTART;VALUE=DATE:20261127")].concat();
        let now_ms = PC.to_utc(at(27, 8, 0)) * 1000;
        let (list, _) = calendar(&events).upcoming(now_ms / 1000, &PC);
        let section = serde_json::json!({ "updatedAt": 1, "events": list });
        let now = Local::new(2026, 11, 27, 8, 0).unwrap();
        assert_eq!(answer(&section, Ask::Today, &now, now_ms).text, "Today: 09:00 Standup, Fair");
        assert_eq!(answer(&section, Ask::Tomorrow, &now, now_ms).text, "Tomorrow: 10:00 Review");
        assert_eq!(answer(&section, Ask::Next, &now, now_ms).key, "voice.reply.calendar_next");
        assert_eq!(answer(&section, Ask::Next, &now, now_ms + 3 * 3_600_000).key, "voice.reply.calendar_next_tomorrow");
        assert_eq!(answer(&serde_json::Value::Null, Ask::Next, &now, now_ms).key, "voice.reply.calendar_off");
        // The note comes within five minutes, once, and never over a full screen program.
        use crate::voice::notices::{soon, Notices, Rule, Sample};
        let due = soon(&section, now_ms + 56 * 60_000);
        assert_eq!((due.len(), due[0].minutes), (1, 4));
        assert!(soon(&section, now_ms + 50 * 60_000).is_empty());
        let mut notices = Notices::default();
        let mut sample = Sample { soon: due, fullscreen: true, ..Default::default() };
        assert!(notices.step(true, &sample).is_empty());
        sample.fullscreen = false;
        assert_eq!(notices.step(true, &sample).iter().map(|n| n.rule).collect::<Vec<_>>(), [Rule::Event]);
        assert!(notices.step(true, &sample).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn the_zone_of_the_pc_goes_there_and_back() {
        let civil = at(27, 12, 34);
        assert_eq!(PcZone.to_civil(PcZone.to_utc(civil)), civil);
    }
}
