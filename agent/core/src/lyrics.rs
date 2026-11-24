//! Lyrics of the current song (DATA_MODEL.md 14, PROTOCOL.md `LyricsInfo` / `lyrics_offset`). Off by default
//! (`config.media.lyrics.enabled`): while it is off not a single request leaves the PC. Only the artist, the title and the
//! duration go to lrclib.net, once per track; the answer is kept in a small cache on this PC. The phone only sends
//! `lyrics_offset{trackKey, deltaMs ±500}`: data, not a command (a `trackKey` the PC did not issue is ignored).

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::model::MediaInfo;

pub const LRCLIB: &str = "https://lrclib.net/api";
pub const MAX_LINES: usize = 300;
pub const MAX_LINE_CHARS: usize = 200;
/// The cache keeps at most this many files; the least recently used go first.
pub const CACHE_FILES: usize = 500;
/// "Not found" is remembered this long.
pub const NOT_FOUND_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// A record whose duration differs from the track by more than this is another version.
pub const DURATION_TOLERANCE_SEC: f64 = 3.0;
pub const OFFSETS_MAX: usize = 1000;
pub const OFFSET_LIMIT_MS: i64 = 30_000;
const TIMEOUT: Duration = Duration::from_secs(5);

// ---- Section ----------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<u64>,
    pub text: String,
}

/// The `lyrics` section (PROTOCOL.md `LyricsInfo`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsInfo {
    pub track_key: String,
    pub synced: bool,
    pub lines: Vec<Line>,
    pub offset_ms: i64,
}

/// What was found for a track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub synced: bool,
    pub lines: Vec<Line>,
}

// ---- The title --------------------------------------------------------------------------------------------------

const NOISE_WORDS: &[&str] = &[
    "official", "video", "audio", "lyric", "lyrics", "mv", "hd", "4k", "remaster", "remastered", "live", "visualizer", "клип", "премьера", "текст",
];
const BROWSERS: &[&str] = &["chrome", "msedge", "edge", "firefox", "opera", "brave", "vivaldi", "yandex", "browser"];

fn words_of(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect()
}

/// Removes every `(...)` / `[...]` group that holds a noise word ("Official Video", "Lyrics", "HD", "Remastered 2011").
fn strip_noise_groups(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        let close = match c {
            '(' => ')',
            '[' => ']',
            _ => {
                out.push(c);
                continue;
            }
        };
        let mut inner = String::new();
        let mut closed = false;
        for d in chars.by_ref() {
            if d == close {
                closed = true;
                break;
            }
            inner.push(d);
        }
        if !closed {
            // An unclosed bracket: keep the text as it was.
            out.push(c);
            out.push_str(&inner);
        } else if !words_of(&inner).iter().any(|w| NOISE_WORDS.contains(&w.as_str())) {
            out.push(c);
            out.push_str(&inner);
            out.push(close);
        }
    }
    out
}

/// Cuts `feat.`, `ft.` and `featuring` and everything after (with the bracket that opened it).
fn cut_featuring(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut cut = s.len();
    for tag in ["feat.", "feat ", "ft.", "ft ", "featuring "] {
        let mut from = 0;
        while let Some(i) = lower[from..].find(tag) {
            let at = from + i;
            // A tag of its own: at the start of the text or after a space or a bracket.
            let before = lower[..at].chars().next_back();
            if before.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '[')) {
                let start = if before.is_some_and(|c| matches!(c, '(' | '[')) { at - 1 } else { at };
                cut = cut.min(start);
                break;
            }
            from = at + tag.len();
        }
    }
    s[..cut].to_string()
}

fn tidy(s: &str) -> String {
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | '—' | '|' | ',' | ':')).to_string()
}

fn clean_part(s: &str) -> String {
    let before_bar = s.split(" | ").next().unwrap_or(s);
    tidy(&cut_featuring(&strip_noise_groups(before_bar)))
}

/// The artist and the title as lrclib knows them. A video title of a browser (`Artist - Title (Official Video) | Label`) is
/// split when the artist is empty or the name of a channel; tails and brackets of noise words and `feat.` are cut.
pub fn clean(source: &str, artist: &str, title: &str) -> (String, String) {
    let artist = artist.trim().strip_suffix(" - Topic").unwrap_or(artist.trim()).trim();
    let source = source.to_lowercase();
    let from_browser = BROWSERS.iter().any(|b| source.contains(b));
    let (mut artist, mut title) = (artist.to_string(), title.trim().to_string());
    if let Some((left, right)) = title.split_once(" - ") {
        let left = left.trim();
        // The artist of a player is right already; a browser says the channel (or nothing).
        if !left.is_empty() && !right.trim().is_empty() && (artist.is_empty() || from_browser) {
            artist = left.to_string();
            title = right.trim().to_string();
        }
    }
    (clean_part(&artist), clean_part(&title))
}

/// `trackKey` of PROTOCOL.md: the first 16 hex characters of SHA-256 over `lower(artist)|lower(title)|durationSec`.
pub fn track_key(artist: &str, title: &str, duration_sec: u64) -> String {
    let digest = Sha256::digest(format!("{}|{}|{}", artist.to_lowercase(), title.to_lowercase(), duration_sec).as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn is_track_key(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// ---- LRCLIB -----------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub synced_lyrics: Option<String>,
}

pub fn get_url(base: &str, artist: &str, title: &str, duration_sec: u64) -> String {
    let mut url = url::Url::parse(&format!("{base}/get")).expect("the lyrics address is valid");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("artist_name", artist);
        q.append_pair("track_name", title);
        if duration_sec > 0 {
            q.append_pair("duration", &duration_sec.to_string());
        }
    }
    url.to_string()
}

pub fn search_url(base: &str, artist: &str, title: &str) -> String {
    let mut url = url::Url::parse(&format!("{base}/search")).expect("the lyrics address is valid");
    url.query_pairs_mut().append_pair("track_name", title).append_pair("artist_name", artist);
    url.to_string()
}

fn near(record: &Record, duration_sec: u64) -> bool {
    duration_sec > 0 && record.duration.is_some_and(|d| (d - duration_sec as f64).abs() <= DURATION_TOLERANCE_SEC)
}

/// From a list of search results: the first one whose duration is within 3 s of the track, else the first one.
pub fn pick_search_result(results: Vec<Record>, duration_sec: u64) -> Option<Record> {
    let index = results.iter().position(|r| near(r, duration_sec)).unwrap_or(0);
    results.into_iter().nth(index)
}

/// `[mm:ss.xx]` tags in front of the text of a line (several may stand together); metadata tags (`[ar:..]`) are skipped.
fn parse_lrc(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim_start();
        let mut times = Vec::new();
        while let Some(tag) = rest.strip_prefix('[') {
            let Some(end) = tag.find(']') else { break };
            let (stamp, after) = (&tag[..end], &tag[end + 1..]);
            let Some((m, s)) = stamp.split_once(':') else { break };
            let (Ok(m), Ok(s)) = (m.parse::<u64>(), s.parse::<f64>()) else { break };
            if !s.is_finite() || !(0.0..60.0).contains(&s) {
                break;
            }
            times.push(m * 60_000 + (s * 1000.0).round() as u64);
            rest = after.trim_start();
        }
        for t in times {
            out.push(Line { t: Some(t), text: rest.trim().chars().take(MAX_LINE_CHARS).collect() });
        }
    }
    out.sort_by_key(|l| l.t);
    out.truncate(MAX_LINES);
    out
}

fn plain_lines(text: &str) -> Vec<Line> {
    text.lines().map(|l| Line { t: None, text: l.trim().chars().take(MAX_LINE_CHARS).collect() }).take(MAX_LINES).collect()
}

/// DATA_MODEL.md 14 "Выбор текста": synced when the record has timed lines and its length is within 3 s of the track,
/// else plain text (or the timed lines without their times), else nothing (also for an instrumental).
pub fn choose(record: &Record, duration_sec: u64) -> Option<Found> {
    if record.instrumental {
        return None;
    }
    let synced = record.synced_lyrics.as_deref().map(parse_lrc).filter(|l| !l.is_empty());
    if let Some(lines) = &synced {
        if near(record, duration_sec) {
            return Some(Found { synced: true, lines: lines.clone() });
        }
    }
    let plain = record.plain_lyrics.as_deref().map(plain_lines).filter(|l| l.iter().any(|x| !x.text.is_empty()));
    let lines = plain.or_else(|| synced.map(|l| l.into_iter().map(|x| Line { t: None, text: x.text }).collect()))?;
    Some(Found { synced: false, lines })
}

// ---- Network (replaceable in tests) -----------------------------------------------------------------------------

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// `GET url` -> (status, body); an error is the text of the failure.
pub trait Http: Send + Sync {
    fn get(&self, url: String) -> BoxFut<Result<(u16, String), String>>;
}

pub struct RealHttp;

impl Http for RealHttp {
    fn get(&self, url: String) -> BoxFut<Result<(u16, String), String>> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(TIMEOUT)
                .user_agent(format!("Decka/{} (https://github.com/iqonkz/decka)", crate::VERSION))
                .build()
                .map_err(|e| e.to_string())?;
            let response = client.get(&url).send().await.map_err(|e| e.without_url().to_string())?;
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|e| e.without_url().to_string())?;
            Ok((status, body))
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Found(Found),
    NotFound,
}

/// `get`, and on 404 `search`. Any other failure is an error: it is not cached, the next track (or the next try) asks again.
pub async fn lookup(http: &dyn Http, base: &str, artist: &str, title: &str, duration_sec: u64) -> Result<Outcome, String> {
    let (status, body) = http.get(get_url(base, artist, title, duration_sec)).await?;
    let record = match status {
        200 => serde_json::from_str::<Record>(&body).ok(),
        404 => {
            let (status, body) = http.get(search_url(base, artist, title)).await?;
            match status {
                200 => pick_search_result(serde_json::from_str::<Vec<Record>>(&body).unwrap_or_default(), duration_sec),
                404 => None,
                other => return Err(format!("HTTP {other}")),
            }
        }
        other => return Err(format!("HTTP {other}")),
    };
    Ok(record.and_then(|r| choose(&r, duration_sec)).map_or(Outcome::NotFound, Outcome::Found))
}

// ---- Cache ------------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct CacheFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    found: Option<Found>,
    /// Unix ms of a "not found".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    not_found_at: Option<u64>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// `%LOCALAPPDATA%\Decka\lyrics`, one file per track.
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(dir: PathBuf) -> Cache {
        Cache { dir }
    }

    fn file(&self, key: &str) -> Option<PathBuf> {
        is_track_key(key).then(|| self.dir.join(format!("{key}.json")))
    }

    /// The remembered answer; a "not found" older than 7 days is nothing. A read counts as a use (the file is touched).
    pub fn get(&self, key: &str, now: u64) -> Option<Outcome> {
        let path = self.file(key)?;
        let file: CacheFile = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
        let outcome = match (file.found, file.not_found_at) {
            (Some(found), _) => Outcome::Found(found),
            (None, Some(at)) if now.saturating_sub(at) < NOT_FOUND_TTL.as_millis() as u64 => Outcome::NotFound,
            _ => return None,
        };
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(outcome)
    }

    pub fn put(&self, key: &str, outcome: &Outcome, now: u64) {
        let Some(path) = self.file(key) else { return };
        let file = match outcome {
            Outcome::Found(f) => CacheFile { found: Some(f.clone()), not_found_at: None },
            Outcome::NotFound => CacheFile { found: None, not_found_at: Some(now) },
        };
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        if let Ok(text) = serde_json::to_string(&file) {
            let _ = std::fs::write(&path, text);
        }
        self.prune();
    }

    /// At most `CACHE_FILES` files stay: the least recently used are deleted.
    pub fn prune(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return };
        let mut files: Vec<(SystemTime, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .collect();
        if files.len() <= CACHE_FILES {
            return;
        }
        files.sort();
        for (_, path) in files.iter().take(files.len() - CACHE_FILES) {
            let _ = std::fs::remove_file(path);
        }
    }
}

// ---- Offsets ----------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct OffsetEntry {
    key: String,
    ms: i64,
}

/// `lyrics_offsets.json` in the configuration folder: the shift the user chose for a track, at most 1000 tracks (the oldest go).
pub struct Offsets {
    path: PathBuf,
    entries: Vec<OffsetEntry>,
}

impl Offsets {
    pub fn load(config_dir: &Path) -> Offsets {
        let path = config_dir.join("lyrics_offsets.json");
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Vec<OffsetEntry>>(&t).ok())
            .map(|v| v.into_iter().filter(|e| is_track_key(&e.key) && e.ms.abs() <= OFFSET_LIMIT_MS && e.ms % 500 == 0).take(OFFSETS_MAX).collect())
            .unwrap_or_default();
        Offsets { path, entries }
    }

    pub fn get(&self, key: &str) -> i64 {
        self.entries.iter().find(|e| e.key == key).map_or(0, |e| e.ms)
    }

    /// Moves the shift of a track by ±500 ms (anything else is refused), within ±30 s; returns the new shift.
    pub fn nudge(&mut self, key: &str, delta_ms: i64) -> Option<i64> {
        if !is_track_key(key) || !matches!(delta_ms, -500 | 500) {
            return None;
        }
        let now = (self.get(key) + delta_ms).clamp(-OFFSET_LIMIT_MS, OFFSET_LIMIT_MS);
        self.entries.retain(|e| e.key != key);
        // A shift back at zero is not worth a record.
        if now != 0 {
            self.entries.push(OffsetEntry { key: key.to_string(), ms: now });
        }
        while self.entries.len() > OFFSETS_MAX {
            self.entries.remove(0);
        }
        if let Ok(text) = serde_json::to_string(&self.entries) {
            let _ = std::fs::write(&self.path, text);
        }
        Some(now)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

// ---- The service ------------------------------------------------------------------------------------------------

struct Current {
    key: String,
    found: Option<Found>,
}

/// The lyrics of the current track: asks (when on and the track changed), remembers, applies the shifts.
pub struct Lyrics {
    http: Arc<dyn Http>,
    base: String,
    cache: Cache,
    offsets: Mutex<Offsets>,
    current: Mutex<Option<Current>>,
}

impl Lyrics {
    pub fn new(config_dir: &Path, cache_dir: PathBuf) -> Lyrics {
        Lyrics::with_http(config_dir, cache_dir, Arc::new(RealHttp), LRCLIB.to_string())
    }

    /// The network and the address come from the caller (the tests use recorded answers).
    pub fn with_http(config_dir: &Path, cache_dir: PathBuf, http: Arc<dyn Http>, base: String) -> Lyrics {
        Lyrics { http, base, cache: Cache::new(cache_dir), offsets: Mutex::new(Offsets::load(config_dir)), current: Mutex::new(None) }
    }

    fn info(&self, key: &str, found: &Found) -> Value {
        let offset_ms = self.offsets.lock().unwrap().get(key);
        serde_json::to_value(LyricsInfo { track_key: key.to_string(), synced: found.synced, lines: found.lines.clone(), offset_ms }).expect("lyrics serialize")
    }

    /// The `lyrics` section for the current media session: `null` while off, without a track or without lyrics.
    /// Off = no request and nothing remembered. A track is asked for once (cache, then lrclib.net).
    pub async fn section(&self, media: Option<&MediaInfo>, enabled: bool) -> Value {
        let Some(media) = media.filter(|_| enabled) else {
            *self.current.lock().unwrap() = None;
            return Value::Null;
        };
        let (artist, title) = clean(&media.source, media.artist.as_deref().unwrap_or(""), &media.title);
        if title.is_empty() {
            return Value::Null;
        }
        let duration = media.duration_sec.unwrap_or(0);
        let key = track_key(&artist, &title, duration);
        if let Some(c) = self.current.lock().unwrap().as_ref().filter(|c| c.key == key) {
            return c.found.as_ref().map_or(Value::Null, |f| self.info(&key, f));
        }
        let now = now_ms();
        let outcome = match self.cache.get(&key, now) {
            Some(o) => o,
            None => match lookup(self.http.as_ref(), &self.base, &artist, &title, duration).await {
                Ok(o) => {
                    self.cache.put(&key, &o, now);
                    o
                }
                Err(e) => {
                    // Not cached: asked again at the next change of the track. The text of the track stays out of the log.
                    tracing::debug!("lyrics: {e}");
                    return Value::Null;
                }
            },
        };
        let found = match outcome {
            Outcome::Found(f) => Some(f),
            Outcome::NotFound => None,
        };
        let value = found.as_ref().map_or(Value::Null, |f| self.info(&key, f));
        *self.current.lock().unwrap() = Some(Current { key, found });
        value
    }

    /// `lyrics_offset` of a phone. Only the key of the current track counts; the new section value is returned when it changed.
    pub fn nudge(&self, key: &str, delta_ms: i64) -> Option<Value> {
        let guard = self.current.lock().unwrap();
        let current = guard.as_ref().filter(|c| c.key == key)?;
        let found = current.found.as_ref()?;
        self.offsets.lock().unwrap().nudge(key, delta_ms)?;
        Some(self.info(key, found))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const GET: &str = include_str!("../../../shared/fixtures/lrclib-get.json");
    const SEARCH: &str = include_str!("../../../shared/fixtures/lrclib-search.json");

    /// The recorded answers of lrclib.net by address; every request is counted.
    struct Fake {
        get: (u16, String),
        search: (u16, String),
        calls: AtomicUsize,
        urls: Mutex<Vec<String>>,
    }

    impl Fake {
        fn new(get: (u16, &str), search: (u16, &str)) -> Arc<Fake> {
            Arc::new(Fake { get: (get.0, get.1.into()), search: (search.0, search.1.into()), calls: AtomicUsize::new(0), urls: Mutex::new(Vec::new()) })
        }
    }

    impl Http for Fake {
        fn get(&self, url: String) -> BoxFut<Result<(u16, String), String>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let answer = if url.contains("/search?") { self.search.clone() } else { self.get.clone() };
            self.urls.lock().unwrap().push(url);
            Box::pin(async move { Ok(answer) })
        }
    }

    fn media(title: &str, artist: Option<&str>, duration: u64) -> MediaInfo {
        MediaInfo {
            source: "Spotify.exe".into(),
            title: title.into(),
            artist: artist.map(str::to_string),
            album: None,
            playing: true,
            position_sec: Some(0),
            duration_sec: Some(duration),
            position_at: 0,
            art: None,
            shuffle: None,
            repeat: None,
        }
    }

    fn service(fake: Arc<Fake>) -> (Lyrics, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let l = Lyrics::with_http(dir.path(), dir.path().join("cache"), fake, "https://lrclib.test/api".into());
        (l, dir)
    }

    // ---- Title cleanup: 15 YouTube / browser titles ----

    #[test]
    fn fifteen_youtube_titles_are_cleaned() {
        // (source, artist = the channel, title) -> (artist, title)
        let cases: [(&str, &str, &str, &str, &str); 15] = [
            ("chrome.exe", "M83", "M83 - Midnight City (Official Video)", "M83", "Midnight City"),
            ("chrome.exe", "", "Daft Punk - Get Lucky (Official Audio) ft. Pharrell Williams", "Daft Punk", "Get Lucky"),
            ("msedge.exe", "Queen Official", "Queen - Bohemian Rhapsody (Remastered 2011)", "Queen", "Bohemian Rhapsody"),
            ("firefox.exe", "Adele", "Adele - Hello [Lyrics]", "Adele", "Hello"),
            ("chrome.exe", "VEVO", "Rihanna - Diamonds | Official Music Video 4K", "Rihanna", "Diamonds"),
            ("chrome.exe", "Топ Музыка", "Кино - Группа крови (Official Video)", "Кино", "Группа крови"),
            ("yandex.exe", "Ночные снайперы", "Ночные снайперы - Лови каждое мгновенье [клип]", "Ночные снайперы", "Лови каждое мгновенье"),
            ("chrome.exe", "Imagine Dragons", "Imagine Dragons - Believer (Lyric Video) [HD]", "Imagine Dragons", "Believer"),
            ("chrome.exe", "Eminem - Topic", "Lose Yourself (feat. Someone)", "Eminem", "Lose Yourself"),
            ("opera.exe", "", "Coldplay - Yellow - Live at Glastonbury", "Coldplay", "Yellow - Live at Glastonbury"),
            ("chrome.exe", "Ed Sheeran", "Ed Sheeran - Shape of You (Official Music Video)", "Ed Sheeran", "Shape of You"),
            ("brave.exe", "Label", "The Weeknd - Blinding Lights [Official Visualizer]", "The Weeknd", "Blinding Lights"),
            ("chrome.exe", "Музыка", "Валерия - Часы | Премьера клипа", "Валерия", "Часы"),
            ("chrome.exe", "Nirvana", "Nirvana - Smells Like Teen Spirit (Official Music Video) (Remastered)", "Nirvana", "Smells Like Teen Spirit"),
            ("Spotify.exe", "M83", "Midnight City - Remastered", "M83", "Midnight City - Remastered"),
        ];
        for (source, artist, title, want_artist, want_title) in cases {
            assert_eq!(clean(source, artist, title), (want_artist.to_string(), want_title.to_string()), "{title}");
        }
    }

    #[test]
    fn the_key_is_sixteen_hex_characters_and_ignores_case() {
        let key = track_key("M83", "Midnight City", 243);
        assert!(is_track_key(&key), "{key}");
        assert_eq!(key, track_key("m83", "MIDNIGHT CITY", 243));
        assert_ne!(key, track_key("M83", "Midnight City", 244));
        assert!(!is_track_key("ABCDEF0123456789") && !is_track_key("../etc") && !is_track_key("0123"));
    }

    // ---- Choosing the text ----

    #[test]
    fn synced_needs_a_duration_within_three_seconds() {
        let record: Record = serde_json::from_str(GET).unwrap();
        // 243 s record, track of 245 s: synced.
        let found = choose(&record, 245).unwrap();
        assert!(found.synced);
        assert_eq!(found.lines[0], Line { t: Some(5_200), text: "Waiting in a car".into() });
        // Two tags on one line become two lines, sorted by time; metadata tags are no lines.
        assert_eq!(found.lines.iter().filter(|l| l.text == "Look and see her eyes, they glow").count(), 2);
        assert!(found.lines.windows(2).all(|w| w[0].t <= w[1].t));
        assert!(found.lines.iter().all(|l| !l.text.starts_with("ar:")));
        // The track is 5 s longer (another version): the lines stay, the times go.
        let plain = choose(&record, 248).unwrap();
        assert!(!plain.synced && plain.lines.iter().all(|l| l.t.is_none()));
        assert_eq!(plain.lines[0].text, "Waiting in a car");
        // Unknown duration of the track: never synced.
        assert!(!choose(&record, 0).unwrap().synced);
    }

    #[test]
    fn instrumental_or_empty_is_nothing_and_synced_only_becomes_plain() {
        let instrumental: Record = serde_json::from_str(r#"{"duration":100.0,"instrumental":true,"plainLyrics":"x"}"#).unwrap();
        assert_eq!(choose(&instrumental, 100), None);
        let empty: Record = serde_json::from_str(r#"{"duration":100.0,"instrumental":false}"#).unwrap();
        assert_eq!(choose(&empty, 100), None);
        let only_synced: Record = serde_json::from_str(r#"{"duration":100.0,"syncedLyrics":"[00:01.00] one\n[00:02.00] two"}"#).unwrap();
        let found = choose(&only_synced, 130).unwrap();
        assert_eq!((found.synced, found.lines.len(), found.lines[1].text.as_str()), (false, 2, "two"));
    }

    #[test]
    fn long_texts_are_cut() {
        let text = (0..400).map(|i| format!("[{:02}:{:02}.00] {}", i / 60, i % 60, "я".repeat(300))).collect::<Vec<_>>().join("\n");
        let record = Record { duration: Some(500.0), synced_lyrics: Some(text), ..Default::default() };
        let found = choose(&record, 500).unwrap();
        assert_eq!(found.lines.len(), MAX_LINES);
        assert!(found.lines.iter().all(|l| l.text.chars().count() <= MAX_LINE_CHARS));
    }

    #[test]
    fn search_takes_the_first_within_three_seconds_else_the_first() {
        let results: Vec<Record> = serde_json::from_str(SEARCH).unwrap();
        // 244 s: the remix (328 s) is skipped, the second one (244.4 s) fits.
        assert_eq!(pick_search_result(results.clone(), 244).unwrap().duration, Some(244.4));
        // Nothing near 100 s: the first.
        assert_eq!(pick_search_result(results, 100).unwrap().duration, Some(328.0));
        assert!(pick_search_result(vec![], 100).is_none());
    }

    #[test]
    fn addresses_carry_only_artist_title_and_duration() {
        let url = get_url(LRCLIB, "M83", "Midnight City", 243);
        assert_eq!(url, "https://lrclib.net/api/get?artist_name=M83&track_name=Midnight+City&duration=243");
        assert_eq!(search_url(LRCLIB, "M83", "Midnight City"), "https://lrclib.net/api/search?track_name=Midnight+City&artist_name=M83");
        assert!(!get_url(LRCLIB, "a", "b", 0).contains("duration"));
    }

    // ---- The lookup and the service (recorded answers, no network) ----

    #[tokio::test]
    async fn get_answers_at_once_and_404_falls_back_to_search() {
        let fake = Fake::new((200, GET), (200, SEARCH));
        assert!(matches!(lookup(fake.as_ref(), "https://x/api", "M83", "Midnight City", 243).await, Ok(Outcome::Found(f)) if f.synced));
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);

        let fake = Fake::new((404, "{}"), (200, SEARCH));
        let outcome = lookup(fake.as_ref(), "https://x/api", "M83", "Midnight City", 244).await;
        assert!(matches!(outcome, Ok(Outcome::Found(f)) if f.synced && f.lines.len() == 2));
        assert_eq!(fake.calls.load(Ordering::SeqCst), 2);
        assert!(fake.urls.lock().unwrap()[1].contains("/search?"));

        let fake = Fake::new((404, "{}"), (404, "[]"));
        assert_eq!(lookup(fake.as_ref(), "https://x/api", "a", "b", 1).await, Ok(Outcome::NotFound));
        let fake = Fake::new((500, ""), (200, "[]"));
        assert_eq!(lookup(fake.as_ref(), "https://x/api", "a", "b", 1).await, Err("HTTP 500".to_string()));
    }

    #[tokio::test]
    async fn off_means_zero_requests() {
        let fake = Fake::new((200, GET), (200, SEARCH));
        let (l, _dir) = service(fake.clone());
        assert_eq!(l.section(Some(&media("Midnight City", Some("M83"), 243)), false).await, Value::Null);
        assert_eq!(l.section(None, true).await, Value::Null);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_track_is_asked_once_and_then_comes_from_the_cache() {
        let fake = Fake::new((200, GET), (200, SEARCH));
        let (l, dir) = service(fake.clone());
        let m = media("Midnight City", Some("M83"), 243);
        let section = l.section(Some(&m), true).await;
        assert_eq!(section["synced"], true);
        assert_eq!(section["offsetMs"], 0);
        assert!(is_track_key(section["trackKey"].as_str().unwrap()));
        assert_eq!(l.section(Some(&m), true).await, section, "same track: nothing is asked again");
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        // A new service (a restart of the agent) reads the file.
        let again = Lyrics::with_http(dir.path(), dir.path().join("cache"), fake.clone(), "https://lrclib.test/api".into());
        assert_eq!(again.section(Some(&m), true).await, section);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1, "cache hit");
        // Only the artist, the title and the duration are in the address.
        let url = fake.urls.lock().unwrap()[0].clone();
        assert_eq!(url, "https://lrclib.test/api/get?artist_name=M83&track_name=Midnight+City&duration=243");
    }

    #[tokio::test]
    async fn not_found_is_remembered_for_seven_days_and_errors_are_not() {
        let fake = Fake::new((404, "{}"), (404, "[]"));
        let (l, dir) = service(fake.clone());
        let m = media("Unknown Song", Some("Nobody"), 100);
        assert_eq!(l.section(Some(&m), true).await, Value::Null);
        let calls = fake.calls.load(Ordering::SeqCst);
        // Another track and back again: the file answers.
        let other = Lyrics::with_http(dir.path(), dir.path().join("cache"), fake.clone(), "https://lrclib.test/api".into());
        assert_eq!(other.section(Some(&m), true).await, Value::Null);
        assert_eq!(fake.calls.load(Ordering::SeqCst), calls, "not found is cached");
        let key = track_key("Nobody", "Unknown Song", 100);
        let cache = Cache::new(dir.path().join("cache"));
        assert_eq!(cache.get(&key, now_ms()), Some(Outcome::NotFound));
        assert_eq!(cache.get(&key, now_ms() + NOT_FOUND_TTL.as_millis() as u64 + 1), None, "after 7 days it is asked again");

        let broken = Fake::new((500, ""), (500, ""));
        let (l, dir) = service(broken.clone());
        assert_eq!(l.section(Some(&m), true).await, Value::Null);
        assert_eq!(Cache::new(dir.path().join("cache")).get(&key, now_ms()), None, "a failure is not remembered");
    }

    #[test]
    fn the_cache_keeps_500_files_the_oldest_go() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf());
        let found = Outcome::Found(Found { synced: false, lines: vec![Line { t: None, text: "x".into() }] });
        for i in 0..(CACHE_FILES + 5) {
            cache.put(&track_key("a", "b", i as u64), &found, 1);
            // Different modification times: the first files are the oldest.
            let path = dir.path().join(format!("{}.json", track_key("a", "b", i as u64)));
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&path) {
                let _ = f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + i as u64));
            }
        }
        let count = std::fs::read_dir(dir.path()).unwrap().count();
        assert!(count <= CACHE_FILES, "{count}");
        assert!(cache.get(&track_key("a", "b", 0), 1).is_none(), "the oldest was deleted");
        assert!(cache.get(&track_key("a", "b", (CACHE_FILES + 4) as u64), 1).is_some());
        // A bad key never touches the disk.
        cache.put("../../x", &found, 1);
        assert!(cache.get("../../x", 1).is_none());
    }

    // ---- The offsets ----

    #[tokio::test]
    async fn offsets_move_by_half_a_second_for_the_current_track_only() {
        let fake = Fake::new((200, GET), (200, SEARCH));
        let (l, dir) = service(fake);
        let section = l.section(Some(&media("Midnight City", Some("M83"), 243)), true).await;
        let key = section["trackKey"].as_str().unwrap().to_string();
        assert_eq!(l.nudge(&key, 500).unwrap()["offsetMs"], 500);
        assert_eq!(l.nudge(&key, 500).unwrap()["offsetMs"], 1000);
        assert_eq!(l.nudge(&key, -500).unwrap()["offsetMs"], 500);
        // Anything but ±500, and a key that was not issued, change nothing.
        for bad in [0, 1, 499, 1000, -1000, 30_000, i64::MAX, i64::MIN] {
            assert!(l.nudge(&key, bad).is_none(), "{bad}");
        }
        assert!(l.nudge("0123456789abcdef", 500).is_none());
        assert!(l.nudge("../../etc/passwd", 500).is_none());
        assert!(l.nudge("", -500).is_none());
        // The shift is kept for the track: a new service reads the file.
        let again = Lyrics::with_http(dir.path(), dir.path().join("cache"), Fake::new((200, GET), (200, SEARCH)), "https://lrclib.test/api".into());
        assert_eq!(again.section(Some(&media("Midnight City", Some("M83"), 243)), true).await["offsetMs"], 500);
    }

    #[test]
    fn a_shift_stays_within_thirty_seconds_and_the_file_holds_a_thousand() {
        let dir = tempfile::tempdir().unwrap();
        let mut offsets = Offsets::load(dir.path());
        let key = track_key("a", "b", 1);
        for _ in 0..100 {
            offsets.nudge(&key, 500);
        }
        assert_eq!(offsets.get(&key), OFFSET_LIMIT_MS);
        for i in 0..(OFFSETS_MAX + 20) {
            offsets.nudge(&track_key("x", "y", i as u64), 500);
        }
        assert_eq!(offsets.len(), OFFSETS_MAX);
        assert_eq!(offsets.get(&key), 0, "the oldest record went");
        // Back at zero leaves no record.
        let k2 = track_key("p", "q", 9);
        offsets.nudge(&k2, 500);
        offsets.nudge(&k2, -500);
        assert_eq!(offsets.get(&k2), 0);
        // A hand-edited file with bad entries is read without them.
        std::fs::write(dir.path().join("lyrics_offsets.json"), r#"[{"key":"nope","ms":500},{"key":"0123456789abcdef","ms":999999},{"key":"fedcba9876543210","ms":1500}]"#).unwrap();
        let loaded = Offsets::load(dir.path());
        assert_eq!((loaded.len(), loaded.get("fedcba9876543210")), (1, 1500));
    }
}
