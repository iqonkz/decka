//! Voice control (DATA_MODEL.md 12): phrase matching (`matcher`), the tools that are its only power (`tools`), the state
//! machine of one request (`pipeline`) and, here, the entrance for the phone: the `voice_*` messages of protocol v2.
//!
//! The phone sends audio and answers; it never says what to run. Every message passes `VoiceHub`, which checks the shape
//! and the limits, and only then reaches the pipeline (ARCHITECTURE.md S24).

pub mod assist;
pub mod audio;
pub mod clipboard;
pub mod dictation;
pub mod endpoint;
pub mod host;
pub mod hotkey;
pub mod llm;
pub mod matcher;
pub mod mic;
pub mod pipeline;
pub mod screen;
pub mod textops;
pub mod tools;
pub mod winkeys;
pub mod winops;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::proto::ErrorCode;

/// 16 kHz mono PCM16.
pub const BYTES_PER_SECOND: usize = 32_000;
pub const MAX_RECORDING_SECONDS: usize = 30;
pub const MAX_RECORDING_BYTES: usize = BYTES_PER_SECOND * MAX_RECORDING_SECONDS;
/// One chunk is 100 ms.
pub const MAX_CHUNK_BYTES: usize = 3200;
pub const MAX_CHUNKS_PER_SECOND: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceError {
    /// Voice control is not turned on on this PC.
    Off,
    /// A recording of this phone, or another voice request, is already running.
    Busy,
    BadId,
    /// A chunk without a recording, with a gap in `seq`, bad base64, odd or oversized data.
    BadChunk(&'static str),
    TooFast,
}

impl VoiceError {
    pub fn code(&self) -> ErrorCode {
        match self {
            VoiceError::Off => ErrorCode::NotFound,
            VoiceError::Busy | VoiceError::TooFast => ErrorCode::RateLimited,
            VoiceError::BadId | VoiceError::BadChunk(_) => ErrorCode::BadMessage,
        }
    }

    pub fn message(&self) -> String {
        match self {
            VoiceError::Off => "voice control is off".into(),
            VoiceError::Busy => "another voice request is running".into(),
            VoiceError::BadId => "bad voice id".into(),
            VoiceError::BadChunk(why) => format!("bad voice chunk: {why}"),
            VoiceError::TooFast => "too many voice chunks".into(),
        }
    }
}

/// What the hub needs from the pipeline (the pipeline `Driver` implements it).
pub trait PhoneSink: Send + Sync {
    /// A phone wants to record. `Err(Busy)` when a request is already running.
    fn start(&self, id: &str) -> Result<(), VoiceError>;
    fn audio(&self, pcm: &[u8]);
    fn end(&self, id: &str);
    fn cancel(&self, id: &str);
    fn confirm(&self, id: &str, yes: bool);
}

impl PhoneSink for pipeline::Driver {
    fn start(&self, id: &str) -> Result<(), VoiceError> {
        use pipeline::{Event, Phase, Source};
        if !matches!(self.state().phase, Phase::Idle | Phase::Done(_) | Phase::Failed(_)) {
            return Err(VoiceError::Busy);
        }
        self.handle_event(Event::Start { id: id.to_string(), source: Source::Phone });
        Ok(())
    }
    fn audio(&self, pcm: &[u8]) {
        self.push_audio(pcm);
    }
    fn end(&self, id: &str) {
        if self.state().id.as_deref() == Some(id) {
            self.handle_event(pipeline::Event::AudioEnd);
        }
    }
    fn cancel(&self, id: &str) {
        if self.state().id.as_deref() == Some(id) {
            self.handle_event(pipeline::Event::Cancel);
        }
    }
    fn confirm(&self, id: &str, yes: bool) {
        self.handle_event(pipeline::Event::Confirm { id: id.to_string(), yes });
    }
}

struct Session {
    id: String,
    next_seq: u64,
    bytes: usize,
    recent: VecDeque<Instant>,
}

#[derive(Default)]
struct Hub {
    sink: Option<Arc<dyn PhoneSink>>,
    /// One recording per device.
    sessions: HashMap<String, Session>,
    /// The device and id of the latest request: only it may answer its confirmation.
    owner: Option<(String, String)>,
}

/// The entrance of the `voice_*` messages. Without a sink (voice control off) every start is refused.
#[derive(Clone, Default)]
pub struct VoiceHub {
    inner: Arc<Mutex<Hub>>,
}

fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl VoiceHub {
    pub fn new() -> VoiceHub {
        VoiceHub::default()
    }

    /// Connects the pipeline (voice control turned on) or disconnects it (`None`).
    pub fn set_sink(&self, sink: Option<Arc<dyn PhoneSink>>) {
        let mut hub = self.inner.lock().unwrap();
        hub.sink = sink;
        if hub.sink.is_none() {
            hub.sessions.clear();
            hub.owner = None;
        }
    }

    pub fn start(&self, device: &str, id: &str) -> Result<(), VoiceError> {
        if !valid_id(id) {
            return Err(VoiceError::BadId);
        }
        let mut hub = self.inner.lock().unwrap();
        let sink = hub.sink.clone().ok_or(VoiceError::Off)?;
        if hub.sessions.contains_key(device) {
            return Err(VoiceError::Busy);
        }
        sink.start(id)?;
        hub.sessions.insert(device.to_string(), Session { id: id.to_string(), next_seq: 0, bytes: 0, recent: VecDeque::new() });
        hub.owner = Some((device.to_string(), id.to_string()));
        Ok(())
    }

    /// One chunk (`seq` counts from 0 and grows by exactly 1). A gap cancels the recording; more than 30 s is cut and ended.
    pub fn chunk(&self, device: &str, id: &str, seq: u64, data: &str, now: Instant) -> Result<(), VoiceError> {
        let mut hub = self.inner.lock().unwrap();
        let sink = hub.sink.clone().ok_or(VoiceError::Off)?;
        let session = hub.sessions.get_mut(device).filter(|s| s.id == id).ok_or(VoiceError::BadChunk("no such recording"))?;
        if seq != session.next_seq {
            hub.sessions.remove(device);
            sink.cancel(id);
            return Err(VoiceError::BadChunk("a chunk is missing"));
        }
        let session = hub.sessions.get_mut(device).expect("checked above");
        while session.recent.front().is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(1)) {
            session.recent.pop_front();
        }
        if session.recent.len() >= MAX_CHUNKS_PER_SECOND {
            return Err(VoiceError::TooFast);
        }
        if data.len() > MAX_CHUNK_BYTES * 4 / 3 + 4 {
            return Err(VoiceError::BadChunk("too big"));
        }
        let mut pcm = STANDARD.decode(data).map_err(|_| VoiceError::BadChunk("not base64"))?;
        if pcm.len() > MAX_CHUNK_BYTES {
            return Err(VoiceError::BadChunk("too big"));
        }
        if pcm.is_empty() || pcm.len() % 2 != 0 {
            return Err(VoiceError::BadChunk("not 16-bit samples"));
        }
        session.next_seq += 1;
        session.recent.push_back(now);
        let room = MAX_RECORDING_BYTES - session.bytes;
        let last = pcm.len() >= room;
        pcm.truncate(room);
        session.bytes += pcm.len();
        sink.audio(&pcm);
        if last {
            hub.sessions.remove(device);
            sink.end(id);
        }
        Ok(())
    }

    /// `voice_end` of an unknown recording is ignored (it may have ended by the 30 s limit).
    pub fn end(&self, device: &str, id: &str) {
        let mut hub = self.inner.lock().unwrap();
        if hub.sessions.get(device).is_some_and(|s| s.id == id) {
            hub.sessions.remove(device);
            if let Some(sink) = hub.sink.clone() {
                sink.end(id);
            }
        }
    }

    pub fn cancel(&self, device: &str, id: &str) {
        let mut hub = self.inner.lock().unwrap();
        let ours = hub.sessions.get(device).is_some_and(|s| s.id == id) || hub.owner.as_ref().is_some_and(|o| o.0 == device && o.1 == id);
        if ours {
            hub.sessions.remove(device);
            if let Some(sink) = hub.sink.clone() {
                sink.cancel(id);
            }
        }
    }

    /// `voice_confirm`: only the phone that started the request may answer; anything else is ignored.
    pub fn confirm(&self, device: &str, id: &str, yes: bool) {
        let hub = self.inner.lock().unwrap();
        if hub.owner.as_ref().is_some_and(|o| o.0 == device && o.1 == id) {
            if let Some(sink) = hub.sink.clone() {
                sink.confirm(id, yes);
            }
        }
    }

    /// The phone went away: its recording is cancelled.
    pub fn device_gone(&self, device: &str) {
        let mut hub = self.inner.lock().unwrap();
        if let Some(s) = hub.sessions.remove(device) {
            if let Some(sink) = hub.sink.clone() {
                sink.cancel(&s.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Rec {
        log: Mutex<Vec<String>>,
        busy: Mutex<bool>,
        audio: Mutex<usize>,
    }

    impl PhoneSink for Rec {
        fn start(&self, id: &str) -> Result<(), VoiceError> {
            if *self.busy.lock().unwrap() {
                return Err(VoiceError::Busy);
            }
            self.log.lock().unwrap().push(format!("start {id}"));
            Ok(())
        }
        fn audio(&self, pcm: &[u8]) {
            *self.audio.lock().unwrap() += pcm.len();
        }
        fn end(&self, id: &str) {
            self.log.lock().unwrap().push(format!("end {id}"));
        }
        fn cancel(&self, id: &str) {
            self.log.lock().unwrap().push(format!("cancel {id}"));
        }
        fn confirm(&self, id: &str, yes: bool) {
            self.log.lock().unwrap().push(format!("confirm {id} {yes}"));
        }
    }

    const ID: &str = "00112233aabbccdd";

    fn hub() -> (VoiceHub, Arc<Rec>) {
        let (h, r) = (VoiceHub::new(), Arc::new(Rec::default()));
        h.set_sink(Some(r.clone()));
        (h, r)
    }

    fn chunk_data(n: usize) -> String {
        STANDARD.encode(vec![1u8; n])
    }

    #[test]
    fn without_a_pipeline_voice_is_off() {
        let h = VoiceHub::new();
        assert_eq!(h.start("d1", ID), Err(VoiceError::Off));
        assert_eq!(VoiceError::Off.code(), ErrorCode::NotFound);
    }

    #[test]
    fn ids_must_be_16_hex_characters() {
        let (h, _) = hub();
        for bad in ["", "abc", "00112233AABBCCDD", "00112233aabbccd", "00112233aabbccddee", "zz112233aabbccdd"] {
            assert_eq!(h.start("d1", bad), Err(VoiceError::BadId), "{bad:?}");
        }
        assert_eq!(VoiceError::BadId.code(), ErrorCode::BadMessage);
        assert_eq!(h.start("d1", ID), Ok(()));
    }

    #[test]
    fn a_second_recording_of_the_same_phone_is_refused() {
        let (h, r) = hub();
        h.start("d1", ID).unwrap();
        assert_eq!(h.start("d1", "ffffffffffffffff"), Err(VoiceError::Busy));
        assert_eq!(VoiceError::Busy.code(), ErrorCode::RateLimited);
        // Another phone is refused by the pipeline while it is busy.
        *r.busy.lock().unwrap() = true;
        assert_eq!(h.start("d2", "ffffffffffffffff"), Err(VoiceError::Busy));
    }

    #[test]
    fn chunks_count_from_zero_and_a_gap_cancels_the_recording() {
        let (h, r) = hub();
        let t = Instant::now();
        h.start("d1", ID).unwrap();
        assert_eq!(h.chunk("d1", ID, 0, &chunk_data(3200), t), Ok(()));
        assert_eq!(h.chunk("d1", ID, 1, &chunk_data(3200), t), Ok(()));
        assert_eq!(*r.audio.lock().unwrap(), 6400);
        assert_eq!(h.chunk("d1", ID, 3, &chunk_data(3200), t), Err(VoiceError::BadChunk("a chunk is missing")));
        assert!(r.log.lock().unwrap().contains(&format!("cancel {ID}")));
        // The recording is gone: the next chunk has nothing to join.
        assert!(matches!(h.chunk("d1", ID, 2, &chunk_data(3200), t), Err(VoiceError::BadChunk(_))));
        // A repeated seq is a gap too.
        h.start("d1", ID).unwrap();
        h.chunk("d1", ID, 0, &chunk_data(320), t).unwrap();
        assert!(h.chunk("d1", ID, 0, &chunk_data(320), t).is_err());
    }

    #[test]
    fn bad_chunks_are_refused_without_ending_the_recording() {
        let (h, r) = hub();
        let t = Instant::now();
        h.start("d1", ID).unwrap();
        for bad in [chunk_data(3202), chunk_data(3), chunk_data(0), "not base64!!".to_string(), "A".repeat(10_000)] {
            let e = h.chunk("d1", ID, 0, &bad, t).unwrap_err();
            assert_eq!(e.code(), ErrorCode::BadMessage, "{bad:.20}");
        }
        assert!(h.chunk("d1", "ffffffffffffffff", 0, &chunk_data(320), t).is_err(), "another id");
        assert!(h.chunk("d2", ID, 0, &chunk_data(320), t).is_err(), "another device");
        assert_eq!(h.chunk("d1", ID, 0, &chunk_data(320), t), Ok(()), "the recording survived");
        assert_eq!(*r.audio.lock().unwrap(), 320);
    }

    #[test]
    fn more_than_fifteen_chunks_in_a_second_are_refused() {
        let (h, _) = hub();
        let t = Instant::now();
        h.start("d1", ID).unwrap();
        for seq in 0..15 {
            h.chunk("d1", ID, seq, &chunk_data(320), t).unwrap();
        }
        assert_eq!(h.chunk("d1", ID, 15, &chunk_data(320), t), Err(VoiceError::TooFast));
        assert_eq!(h.chunk("d1", ID, 15, &chunk_data(320), t + Duration::from_millis(1000)), Ok(()));
    }

    #[test]
    fn a_recording_over_30_seconds_is_cut_and_ended() {
        let (h, r) = hub();
        let t = Instant::now();
        h.start("d1", ID).unwrap();
        let per_chunk = 3200usize;
        let chunks = MAX_RECORDING_BYTES / per_chunk; // 300
        for seq in 0..chunks as u64 - 1 {
            h.chunk("d1", ID, seq, &chunk_data(per_chunk), t + Duration::from_millis(100 * seq)).unwrap();
        }
        assert!(!r.log.lock().unwrap().iter().any(|l| l.starts_with("end")));
        // The 300th chunk reaches 30 s exactly; one more second is cut off and the recording ends.
        h.chunk("d1", ID, chunks as u64 - 1, &chunk_data(per_chunk), t + Duration::from_millis(100 * chunks as u64)).unwrap();
        assert_eq!(*r.audio.lock().unwrap(), MAX_RECORDING_BYTES);
        assert!(r.log.lock().unwrap().contains(&format!("end {ID}")));
        assert!(h.chunk("d1", ID, chunks as u64, &chunk_data(per_chunk), t + Duration::from_secs(40)).is_err(), "nothing is accepted after the end");
        // A single oversized tail is truncated to the limit.
        h.start("d1", ID).unwrap();
        for seq in 0..299u64 {
            h.chunk("d1", ID, seq, &chunk_data(per_chunk), t + Duration::from_secs(50 + seq / 10)).unwrap();
        }
        let before = *r.audio.lock().unwrap();
        h.chunk("d1", ID, 299, &chunk_data(per_chunk), t + Duration::from_secs(90)).unwrap();
        assert_eq!(*r.audio.lock().unwrap() - before, per_chunk);
    }

    #[test]
    fn end_cancel_and_confirm_reach_the_pipeline_only_from_their_owner() {
        let (h, r) = hub();
        h.start("d1", ID).unwrap();
        h.confirm("d2", ID, true); // another phone
        h.confirm("d1", "ffffffffffffffff", true); // another request
        assert!(!r.log.lock().unwrap().iter().any(|l| l.starts_with("confirm")));
        h.end("d1", ID);
        h.end("d1", ID); // a repeated end is ignored
        h.confirm("d1", ID, false); // the recording has ended, the request lives on
        let log = r.log.lock().unwrap().clone();
        assert_eq!(log, [format!("start {ID}"), format!("end {ID}"), format!("confirm {ID} false")]);
        h.cancel("d2", ID);
        assert_eq!(r.log.lock().unwrap().len(), 3);
        h.cancel("d1", ID);
        assert_eq!(r.log.lock().unwrap().last().unwrap(), &format!("cancel {ID}"));
    }

    #[test]
    fn a_disconnected_phone_cancels_its_recording_and_turning_voice_off_forgets_everything() {
        let (h, r) = hub();
        h.start("d1", ID).unwrap();
        h.device_gone("d1");
        assert_eq!(r.log.lock().unwrap().last().unwrap(), &format!("cancel {ID}"));
        h.start("d1", ID).unwrap();
        h.set_sink(None);
        assert_eq!(h.start("d1", ID), Err(VoiceError::Off));
        h.set_sink(Some(r.clone()));
        assert_eq!(h.start("d1", ID), Ok(()), "the old recording was forgotten");
    }
}
