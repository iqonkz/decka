//! `media` collector: the current Windows media session ("Now playing": Spotify, a browser, a player).
//!
//! Read through the system media transport controls (WinRT). Only title, artist, album, playback state,
//! position and the cover are taken; nothing from the session can be controlled from here.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use std::collections::BTreeMap;

use crate::model::{MediaInfo, VarValue};
use crate::state::Store;

const POLL: Duration = Duration::from_secs(1);
/// A jump of the position by more than this, compared with the calculated one, is a seek.
const SEEK_TOLERANCE_SEC: i64 = 3;
const MAX_TEXT: usize = 200;
const MAX_SOURCE: usize = 120;
const MAX_ART_CHARS: usize = 65_536;

/// Cuts `s` to `max` characters (never in the middle of a character).
pub fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Where the position of `info` should be `now_ms` (unix ms), by the formula of PROTOCOL.md 4.5.
pub fn expected_position(info: &MediaInfo, now_ms: u64) -> Option<i64> {
    let position = i64::try_from(info.position_sec?).ok()?;
    if !info.playing {
        return Some(position);
    }
    let elapsed = now_ms.saturating_sub(info.position_at) as i64 / 1000;
    let moved = position + elapsed;
    Some(match info.duration_sec.and_then(|d| i64::try_from(d).ok()) {
        Some(duration) => moved.min(duration),
        None => moved,
    })
}

/// Whether `next` has to be sent to the phones, given what they already have (`prev`).
/// Normal playback only moves the position, which the phone calculates itself, so it is not sent.
pub fn should_publish(prev: Option<&MediaInfo>, next: &MediaInfo, now_ms: u64) -> bool {
    let Some(prev) = prev else { return true };
    if prev.source != next.source
        || prev.title != next.title
        || prev.artist != next.artist
        || prev.album != next.album
        || prev.playing != next.playing
        || prev.duration_sec != next.duration_sec
        || prev.art != next.art
        || prev.shuffle != next.shuffle
        || prev.repeat != next.repeat
    {
        return true;
    }
    match (expected_position(prev, now_ms), next.position_sec.and_then(|p| i64::try_from(p).ok())) {
        (Some(expected), Some(actual)) => (actual - expected).abs() > SEEK_TOLERANCE_SEC,
        (None, None) => false,
        _ => true,
    }
}

/// `data:image/<type>;base64,...` from the bytes of a cover; the type comes from the signature.
/// Unknown signature or a result longer than 65 536 characters: no cover.
pub fn cover_data_uri(bytes: &[u8]) -> Option<String> {
    use base64::Engine;
    let kind = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpeg"
    } else {
        return None;
    };
    let uri = format!("data:image/{kind};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
    (uri.chars().count() <= MAX_ART_CHARS).then_some(uri)
}

/// Longest side of a cover sent to the phones, in pixels.
const ART_MAX_SIDE: u32 = 192;

/// Size a `width` x `height` picture is scaled to: the longer side becomes `ART_MAX_SIDE`, proportions are kept,
/// a smaller picture is never enlarged.
pub fn art_target_size(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= ART_MAX_SIDE || longest == 0 {
        return (width.max(1), height.max(1));
    }
    let scale = |side: u32| ((u64::from(side) * u64::from(ART_MAX_SIDE) + u64::from(longest) / 2) / u64::from(longest)).max(1) as u32;
    (scale(width), scale(height))
}

/// The variables of the session for `bind` (DATA_MODEL.md 6.6): `media.playing`, `media.shuffle`, `media.repeat`.
/// No session, or a source without shuffle / repeat: the variable is removed, so a key bound to it shows as unavailable.
pub fn apply_vars(vars: &mut BTreeMap<String, VarValue>, info: Option<&MediaInfo>) {
    let mut set = |name: &str, value: Option<VarValue>| match value {
        Some(v) => {
            vars.insert(name.to_string(), v);
        }
        None => {
            vars.remove(name);
        }
    };
    set("media.playing", info.map(|i| VarValue::Bool(i.playing)));
    set("media.shuffle", info.and_then(|i| i.shuffle).map(VarValue::Bool));
    set("media.repeat", info.and_then(|i| i.repeat.clone()).map(VarValue::Str));
}

/// Removes the variables of this collector (it was switched off).
pub fn clear_vars(vars: &mut BTreeMap<String, VarValue>) {
    apply_vars(vars, None);
}

pub struct MediaCollector {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MediaCollector {
    pub fn start(store: Store) -> MediaCollector {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("decka-media-collector".to_string())
            .spawn(move || run(store, flag))
            .expect("spawn media collector thread");
        MediaCollector { stop, thread: Some(thread) }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for MediaCollector {
    fn drop(&mut self) {
        self.stop();
    }
}

static REFRESH: AtomicBool = AtomicBool::new(false);

/// Wakes the collector for an immediate re-read (called after a media command through the session).
pub fn request_refresh() {
    REFRESH.store(true, Ordering::Relaxed);
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(not(windows))]
fn run(_store: Store, _stop: Arc<AtomicBool>) {
    // No media sessions outside Windows: the section stays null.
}

#[cfg(windows)]
fn run(store: Store, stop: Arc<AtomicBool>) {
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    // SAFETY: called once on this thread before any WinRT object is created.
    if let Err(e) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        tracing::warn!("media collector: WinRT is unavailable: {e}");
        return;
    }
    let mut reader: Option<winrt::Reader> = None;
    let mut published: Option<MediaInfo> = None;
    let mut last_warning: Option<Instant> = None;

    while !stop.load(Ordering::Relaxed) {
        if reader.is_none() {
            match winrt::Reader::new() {
                Ok(r) => reader = Some(r),
                Err(e) => warn_once_a_minute(&mut last_warning, &format!("cannot open the media session manager: {e}")),
            }
        }
        let next = match reader.as_mut().map(winrt::Reader::read) {
            Some(Ok(info)) => info,
            Some(Err(e)) => {
                warn_once_a_minute(&mut last_warning, &format!("cannot read the media session: {e}"));
                None
            }
            None => None,
        };
        let now = now_ms();
        store.update_vars(|vars| apply_vars(vars, next.as_ref()));
        match (&published, &next) {
            (Some(_), None) => {
                store.set("media", serde_json::Value::Null);
                published = None;
            }
            (_, Some(info)) if should_publish(published.as_ref(), info, now) => {
                store.set("media", serde_json::to_value(info).expect("media serializes"));
                published = Some(info.clone());
            }
            _ => {}
        }
        // Sleep in short slices so a stop request is honoured quickly.
        // A media command asks for an immediate re-read, so the pause button changes at once.
        let end = Instant::now() + POLL;
        while Instant::now() < end && !stop.load(Ordering::Relaxed) {
            if REFRESH.swap(false, Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(windows)]
fn warn_once_a_minute(last: &mut Option<Instant>, message: &str) {
    if last.is_none_or(|t| t.elapsed() >= Duration::from_secs(60)) {
        tracing::warn!("media collector: {message}");
        *last = Some(Instant::now());
    }
}

#[cfg(windows)]
mod winrt {
    use super::{art_target_size, cover_data_uri, now_ms, truncate_chars, ART_MAX_SIDE, MAX_SOURCE, MAX_TEXT};
    use crate::model::MediaInfo;
    use crate::imaging::{self, read_all};
    use windows::Graphics::Imaging::BitmapAlphaMode;
    use windows::Media::MediaPlaybackAutoRepeatMode;
    use windows::Media::Control::{GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager, GlobalSystemMediaTransportControlsSessionPlaybackStatus};

    /// 100 ns ticks between 1601-01-01 (Windows) and 1970-01-01 (unix).
    const EPOCH_DIFF_MS: i64 = 11_644_473_600_000;

    pub struct Reader {
        manager: GlobalSystemMediaTransportControlsSessionManager,
        /// (title, artist, album) the cached cover belongs to, and the cover.
        cover: Option<((String, String, String), Option<String>)>,
        cover_warned: Option<std::time::Instant>,
    }

    impl Reader {
        pub fn new() -> windows::core::Result<Reader> {
            let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
            Ok(Reader { manager, cover: None, cover_warned: None })
        }

        /// The current session as `MediaInfo`; `None` = no session or an empty title.
        pub fn read(&mut self) -> windows::core::Result<Option<MediaInfo>> {
            let session = match self.manager.GetCurrentSession() {
                Ok(session) => session,
                Err(_) => return Ok(None), // no session
            };
            let props = session.TryGetMediaPropertiesAsync()?.join()?;
            let title = props.Title()?.to_string();
            if title.is_empty() {
                return Ok(None);
            }
            let artist = props.Artist()?.to_string();
            let album = props.AlbumTitle()?.to_string();

            // The cover is read only when the track changes.
            let key = (title.clone(), artist.clone(), album.clone());
            let art = match &self.cover {
                Some((cached, art)) if *cached == key => art.clone(),
                _ => {
                    let art = match read_cover(&props) {
                        Ok(art) => art,
                        Err(e) => {
                            super::warn_once_a_minute(&mut self.cover_warned, &format!("cannot prepare the cover: {e}"));
                            None
                        }
                    };
                    self.cover = Some((key, art.clone()));
                    art
                }
            };

            let playback = session.GetPlaybackInfo()?;
            let playing = playback.PlaybackStatus()? == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
            // A source without these states simply reports nothing: the fields stay absent.
            let shuffle = playback.IsShuffleActive().and_then(|r| r.Value()).ok();
            let repeat = playback.AutoRepeatMode().and_then(|r| r.Value()).ok().map(|mode| {
                match mode {
                    MediaPlaybackAutoRepeatMode::List => "list",
                    MediaPlaybackAutoRepeatMode::Track => "track",
                    _ => "none",
                }
                .to_string()
            });
            let (position_sec, duration_sec, position_at) = timeline(&session);
            let non_empty = |s: String| (!s.is_empty()).then(|| truncate_chars(&s, MAX_TEXT));
            Ok(Some(MediaInfo {
                source: truncate_chars(&session.SourceAppUserModelId().map(|s| s.to_string()).unwrap_or_default(), MAX_SOURCE),
                title: truncate_chars(&title, MAX_TEXT),
                artist: non_empty(artist),
                album: non_empty(album),
                playing,
                position_sec,
                duration_sec,
                position_at,
                art,
                shuffle,
                repeat,
            }))
        }
    }

    /// (position in whole seconds, duration if known, unix ms the position refers to)
    fn timeline(session: &GlobalSystemMediaTransportControlsSession) -> (Option<u64>, Option<u64>, u64) {
        let Ok(t) = session.GetTimelineProperties() else { return (None, None, now_ms()) };
        let seconds = |ticks: i64| (ticks.max(0) / 10_000_000) as u64;
        let position = t.Position().ok().map(|p| seconds(p.Duration));
        let duration = t.EndTime().ok().filter(|e| e.Duration > 0).map(|e| seconds(e.Duration));
        let at = t
            .LastUpdatedTime()
            .ok()
            .map(|d| d.UniversalTime / 10_000 - EPOCH_DIFF_MS)
            .filter(|ms| *ms > 0)
            .map_or_else(now_ms, |ms| ms as u64);
        (position, duration, at)
    }

    fn read_cover(props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> windows::core::Result<Option<String>> {
        let Ok(thumbnail) = props.Thumbnail() else { return Ok(None) };
        let stream = thumbnail.OpenReadAsync()?.join()?;
        let size = stream.Size()? as u32;
        if size == 0 || size > MAX_SOURCE_BYTES {
            return Ok(None); // empty, or unreasonably large
        }
        let bytes = read_all(&stream, size)?;
        let shrunk = shrink_art(&bytes)?;
        let uri = cover_data_uri(&shrunk);
        tracing::debug!("media cover: {} bytes -> {} bytes, kept = {}", bytes.len(), shrunk.len(), uri.is_some());
        Ok(uri)
    }

    /// Covers larger than this are not even decoded.
    const MAX_SOURCE_BYTES: u32 = 8_000_000;

    /// The bytes to send as a cover: the original when it is small enough in both pixels and characters,
    /// otherwise a JPEG (quality 0.8) whose longer side is 192 px, scaled by Windows (no extra libraries).
    pub fn shrink_art(bytes: &[u8]) -> windows::core::Result<Vec<u8>> {
        let (width, height) = imaging::decoded_size(bytes)?;
        if width.max(height) <= ART_MAX_SIDE && cover_data_uri(bytes).is_some() {
            return Ok(bytes.to_vec());
        }
        let (target_w, target_h) = art_target_size(width, height);
        let pixels = imaging::scaled_pixels(bytes, target_w, target_h, BitmapAlphaMode::Straight)?;
        imaging::encode_jpeg(&pixels, target_w, target_h, 0.8)
    }

    /// A solid-colour PNG, for tests.
    #[cfg(test)]
    pub fn solid_png(width: u32, height: u32) -> windows::core::Result<Vec<u8>> {
        let pixels: Vec<u8> = std::iter::repeat_n([200u8, 100, 50, 255], (width * height) as usize).flatten().collect();
        imaging::encode_png(&pixels, width, height)
    }

    /// Size of an encoded picture, for tests.
    #[cfg(test)]
    pub fn decoded_size(bytes: &[u8]) -> windows::core::Result<(u32, u32)> {
        imaging::decoded_size(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(position: u64, at: u64, playing: bool) -> MediaInfo {
        MediaInfo {
            source: "Spotify.exe".into(),
            title: "Midnight City".into(),
            artist: Some("M83".into()),
            album: None,
            playing,
            position_sec: Some(position),
            duration_sec: Some(244),
            position_at: at,
            art: None,
            shuffle: None,
            repeat: None,
        }
    }

    #[test]
    fn ten_seconds_of_normal_playback_publish_nothing() {
        let prev = info(42, 1_000_000, true);
        for sec in 1..=10u64 {
            let next = info(42 + sec, 1_000_000 + sec * 1000, true);
            assert!(!should_publish(Some(&prev), &next, 1_000_000 + sec * 1000), "tick {sec}");
        }
    }

    #[test]
    fn a_seek_to_30_s_is_published() {
        let prev = info(100, 1_000_000, true);
        let next = info(30, 1_005_000, true); // 5 s later it should be at 105
        assert!(should_publish(Some(&prev), &next, 1_005_000));
        // A small hiccup of 3 s is not a seek.
        assert!(!should_publish(Some(&prev), &info(102, 1_005_000, true), 1_005_000));
        assert!(should_publish(Some(&prev), &info(109, 1_005_000, true), 1_005_000));
    }

    #[test]
    fn pause_and_resume_are_published() {
        let prev = info(50, 1_000_000, true);
        assert!(should_publish(Some(&prev), &info(53, 1_003_000, false), 1_003_000));
        let paused = info(53, 1_003_000, false);
        // A paused track stays where it is: nothing to send while time passes.
        assert!(!should_publish(Some(&paused), &info(53, 1_060_000, false), 1_060_000));
        assert!(should_publish(Some(&paused), &info(53, 1_061_000, true), 1_061_000));
    }

    #[test]
    fn a_changed_track_or_cover_is_published() {
        let prev = info(10, 1_000_000, true);
        let mut other = info(11, 1_001_000, true);
        other.title = "Another".into();
        assert!(should_publish(Some(&prev), &other, 1_001_000));
        let mut with_art = info(11, 1_001_000, true);
        with_art.art = Some("data:image/png;base64,AAAA".into());
        assert!(should_publish(Some(&prev), &with_art, 1_001_000));
        assert!(should_publish(None, &prev, 1_001_000));
    }

    #[test]
    fn a_changed_shuffle_or_repeat_is_published() {
        let prev = info(10, 1_000_000, true);
        let mut shuffled = info(11, 1_001_000, true);
        shuffled.shuffle = Some(true);
        assert!(should_publish(Some(&prev), &shuffled, 1_001_000));
        let mut repeating = info(11, 1_001_000, true);
        repeating.repeat = Some("track".into());
        assert!(should_publish(Some(&prev), &repeating, 1_001_000));
    }

    #[test]
    fn the_session_variables_follow_the_session() {
        let mut vars = BTreeMap::new();
        let mut now = info(10, 1_000_000, true);
        now.shuffle = Some(false);
        now.repeat = Some("list".into());
        apply_vars(&mut vars, Some(&now));
        assert_eq!(serde_json::to_value(&vars).unwrap(), serde_json::json!({ "media.playing": true, "media.shuffle": false, "media.repeat": "list" }));

        // A source without shuffle / repeat: only `media.playing` stays.
        apply_vars(&mut vars, Some(&info(10, 1_000_000, false)));
        assert_eq!(serde_json::to_value(&vars).unwrap(), serde_json::json!({ "media.playing": false }));

        // No session at all, or the collector switched off: nothing is left, and other variables are untouched.
        vars.insert("my.flag".into(), VarValue::Bool(true));
        apply_vars(&mut vars, Some(&now));
        clear_vars(&mut vars);
        assert_eq!(serde_json::to_value(&vars).unwrap(), serde_json::json!({ "my.flag": true }));
    }

    #[test]
    fn the_calculated_position_stops_at_the_duration() {
        let prev = info(240, 1_000_000, true);
        assert_eq!(expected_position(&prev, 1_100_000), Some(244));
    }

    #[test]
    fn texts_are_cut_by_characters() {
        assert_eq!(truncate_chars("абвгд", 3), "абв");
        assert_eq!(truncate_chars("ab", 5), "ab");
    }

    #[test]
    fn art_is_scaled_to_192_keeping_proportions() {
        assert_eq!(art_target_size(600, 600), (192, 192));
        assert_eq!(art_target_size(640, 320), (192, 96));
        assert_eq!(art_target_size(100, 50), (100, 50));
        assert_eq!(art_target_size(10_000, 1), (192, 1));
    }

    #[cfg(windows)]
    #[test]
    fn a_large_png_becomes_a_192_px_jpeg() {
        let png = winrt::solid_png(600, 600).expect("encode png");
        assert!(png.starts_with(&[0x89, 0x50, 0x4E, 0x47]));
        let shrunk = winrt::shrink_art(&png).expect("shrink");
        assert!(shrunk.starts_with(&[0xFF, 0xD8, 0xFF]), "not a JPEG");
        let uri = cover_data_uri(&shrunk).expect("fits the limit");
        assert!(uri.starts_with("data:image/jpeg;base64,") && uri.len() < MAX_ART_CHARS);
        assert_eq!(winrt::decoded_size(&shrunk).unwrap(), (192, 192));
    }

    #[cfg(windows)]
    #[test]
    fn a_small_cover_is_kept_as_is() {
        let png = winrt::solid_png(100, 80).expect("encode png");
        assert_eq!(winrt::shrink_art(&png).unwrap(), png);
    }

    #[test]
    fn covers_get_a_type_from_their_signature_and_a_size_limit() {
        assert!(cover_data_uri(&[0x89, 0x50, 0x4E, 0x47, 1, 2, 3]).unwrap().starts_with("data:image/png;base64,"));
        assert!(cover_data_uri(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap().starts_with("data:image/jpeg;base64,"));
        assert_eq!(cover_data_uri(b"GIF89a"), None);
        let mut big = vec![0xFF, 0xD8, 0xFF];
        big.extend(std::iter::repeat_n(0u8, 60_000));
        assert_eq!(cover_data_uri(&big), None);
    }
}
