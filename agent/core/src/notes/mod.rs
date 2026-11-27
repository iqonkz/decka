//! Meeting notes (TASKS.md T30.1): the microphone and the sound of the PC are recorded together, recognised in pieces of
//! 30 s by the local whisper and, at the end, summed up by the local model into a Markdown file in `<config>\notes`.
//!
//! Privacy: everything stays on this PC (both models listen on 127.0.0.1). A piece of sound is a file only until it is
//! recognised; the whole recording is kept only when the user switched "Keep the sound" on. While a note is recorded the
//! island shows it (`status`).

pub mod record;
pub mod text;

use std::future::Future;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::assistant::Local;
use text::{Chunker, Summary, RATE};

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The local models and the settings of voice control.
pub trait Backend: Send + Sync {
    /// `Err`: why a note cannot be made now (voice control is not installed).
    fn available(&self) -> Result<(), String>;
    /// Recognition keeps up while the meeting runs (it runs on the video card). Otherwise it waits for the end: the PC
    /// stays free for the call, and a spoken command does not wait behind a piece of the meeting.
    fn live(&self) -> bool;
    fn language(&self) -> String;
    fn microphone(&self) -> Option<String>;
    /// 16 kHz mono PCM16.
    fn transcribe(&self, pcm: Vec<u8>, language: String) -> BoxFut<Result<String, String>>;
    /// A chat request to the local model; the text of its answer.
    fn model(&self, body: Value) -> BoxFut<Option<String>>;
}

pub type Sink = Box<dyn FnMut(Vec<i16>) + Send>;

/// Where the sound comes from (16 kHz mono, microphone and PC mixed). Dropping the handle ends the recording.
pub trait Source: Send + Sync {
    fn start(&self, mic: Option<String>, sink: Sink) -> Result<Box<dyn Send>, String>;
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_samples: u64,
    pub min_free_bytes: u64,
    /// The disk is looked at after this much sound.
    pub disk_check_every: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { max_samples: 3 * 3600 * RATE as u64, min_free_bytes: 1 << 30, disk_check_every: 30 * RATE as u64 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    User,
    TimeLimit,
    DiskFull,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Started,
    Stopped(StopReason),
    Saved { title: String, path: PathBuf, summarized: bool },
    /// Nothing was said: no file is left.
    Empty,
    Failed(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub active: bool,
    pub elapsed_sec: u64,
    /// A finished recording is still being recognised or summed up.
    pub processing: bool,
}

/// A note as the settings window shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub date: String,
    pub markdown: String,
}

pub const ALREADY: &str = "A note is being recorded already";
pub const NO_DISK: &str = "Less than 1 GB of free disk space";
pub const NOT_READY: &str = "Turn on voice control first (More → Voice)";
const MAX_SHOWN: u64 = 1 << 20;

type FreeSpace = Arc<dyn Fn(&Path) -> Option<u64> + Send + Sync>;
type Listener = Arc<dyn Fn(Event) + Send + Sync>;

struct Active {
    id: u64,
    started: Instant,
    capture: Box<dyn Send>,
}

struct Inner {
    dir: PathBuf,
    runtime: tokio::runtime::Handle,
    backend: Mutex<Option<Arc<dyn Backend>>>,
    source: Mutex<Arc<dyn Source>>,
    free: Mutex<FreeSpace>,
    limits: Mutex<Limits>,
    active: Mutex<Option<Active>>,
    processing: AtomicUsize,
    next_id: AtomicU64,
    /// The island and the voice host: one listener each.
    listeners: Mutex<Vec<Listener>>,
}

#[derive(Clone)]
pub struct Notes {
    inner: Arc<Inner>,
}

/// What one recording needs until its file is written.
struct Session {
    id: u64,
    tmp: PathBuf,
    keep_audio: bool,
    when: Local,
    backend: Arc<dyn Backend>,
}

impl Notes {
    pub fn new(dir: PathBuf, runtime: tokio::runtime::Handle) -> Notes {
        // Sound left behind by a run that ended badly is not kept.
        let _ = std::fs::remove_dir_all(dir.join(".tmp"));
        Notes {
            inner: Arc::new(Inner {
                dir,
                runtime,
                backend: Mutex::new(None),
                source: Mutex::new(Arc::new(record::DeviceSource)),
                free: Mutex::new(Arc::new(free_space)),
                limits: Mutex::new(Limits::default()),
                active: Mutex::new(None),
                processing: AtomicUsize::new(0),
                next_id: AtomicU64::new(1),
                listeners: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.inner.dir
    }

    pub fn set_backend(&self, backend: Arc<dyn Backend>) {
        *self.inner.backend.lock().unwrap() = Some(backend);
    }

    /// Tests record from memory and watch their own limits.
    pub fn set_source(&self, source: Arc<dyn Source>) {
        *self.inner.source.lock().unwrap() = source;
    }

    pub fn set_limits(&self, limits: Limits) {
        *self.inner.limits.lock().unwrap() = limits;
    }

    pub fn set_free_space(&self, free: Arc<dyn Fn(&Path) -> Option<u64> + Send + Sync>) {
        *self.inner.free.lock().unwrap() = free;
    }

    pub fn listen(&self, listener: Arc<dyn Fn(Event) + Send + Sync>) {
        self.inner.listeners.lock().unwrap().push(listener);
    }

    fn emit(&self, event: Event) {
        let listeners = self.inner.listeners.lock().unwrap().clone();
        for l in listeners {
            l(event.clone());
        }
    }

    pub fn status(&self) -> Status {
        let active = self.inner.active.lock().unwrap();
        Status {
            active: active.is_some(),
            elapsed_sec: active.as_ref().map_or(0, |a| a.started.elapsed().as_secs()),
            processing: self.inner.processing.load(Ordering::SeqCst) > 0,
        }
    }

    pub fn start(&self, keep_audio: bool) -> Result<(), String> {
        {
            let mut active = self.inner.active.lock().unwrap();
            if active.is_some() {
                return Err(ALREADY.into());
            }
            let backend = self.inner.backend.lock().unwrap().clone().ok_or(NOT_READY)?;
            backend.available()?;
            let free = self.inner.free.lock().unwrap().clone();
            if free(&self.inner.dir).is_some_and(|f| f < self.inner.limits.lock().unwrap().min_free_bytes) {
                return Err(NO_DISK.into());
            }
            let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
            let tmp = self.inner.dir.join(".tmp").join(id.to_string());
            std::fs::create_dir_all(&tmp).map_err(|e| format!("Cannot create the notes folder: {e}"))?;
            let (tx, rx) = unbounded_channel::<Vec<i16>>();
            let source = self.inner.source.lock().unwrap().clone();
            let capture = source.start(backend.microphone(), Box::new(move |samples| {
                let _ = tx.send(samples);
            }))?;
            *active = Some(Active { id, started: Instant::now(), capture });
            self.inner.processing.fetch_add(1, Ordering::SeqCst);
            let session = Session { id, tmp, keep_audio, when: Local::now(), backend };
            let me = self.clone();
            self.inner.runtime.spawn(async move {
                if let Err(e) = me.run(session, rx).await {
                    tracing::warn!("note: {e}");
                    me.emit(Event::Failed(e));
                }
                me.inner.processing.fetch_sub(1, Ordering::SeqCst);
            });
        }
        self.emit(Event::Started);
        Ok(())
    }

    /// Ends the running recording; the text and the summary follow. `false`: nothing was being recorded.
    pub fn stop(&self) -> bool {
        self.stop_for(None, StopReason::User)
    }

    fn stop_for(&self, only: Option<u64>, reason: StopReason) -> bool {
        let taken = {
            let mut active = self.inner.active.lock().unwrap();
            match active.as_ref() {
                Some(a) if only.is_none_or(|id| a.id == id) => active.take(),
                _ => None,
            }
        };
        let Some(active) = taken else { return false };
        // Closing the devices waits for their threads: not on the caller.
        std::thread::spawn(move || drop(active.capture));
        self.emit(Event::Stopped(reason));
        true
    }

    async fn run(&self, session: Session, mut sound: UnboundedReceiver<Vec<i16>>) -> Result<(), String> {
        let limits = *self.inner.limits.lock().unwrap();
        let (pieces, queue) = unbounded_channel::<PathBuf>();
        let lang = session.backend.language();
        let draft = free_name(&self.inner.dir, &stamp(&session.when, '-'), "md");
        let mut queue = Some(queue);
        let mut recognising = None;
        if session.backend.live() {
            recognising = Some(self.inner.runtime.spawn(recognise(session.backend.clone(), queue.take().expect("the queue"), draft.clone(), lang.clone())));
        }
        let mut whole = if session.keep_audio { Some(Wav::create(&session.tmp.join("whole.wav"))?) } else { None };
        let mut chunker = Chunker::default();
        let (mut total, mut checked, mut count) = (0u64, 0u64, 0u32);
        let mut save = |piece: Vec<i16>| -> Result<(), String> {
            if !text::has_speech(&piece) {
                return Ok(());
            }
            count += 1;
            let path = session.tmp.join(format!("{count:05}.pcm"));
            let bytes: Vec<u8> = piece.iter().flat_map(|s| s.to_le_bytes()).collect();
            std::fs::write(&path, bytes).map_err(|e| format!("Cannot write the recording: {e}"))?;
            let _ = pieces.send(path);
            Ok(())
        };
        let mut broken = None;
        while let Some(mut samples) = sound.recv().await {
            // Sound that arrives after the limit is not part of the note.
            samples.truncate(limits.max_samples.saturating_sub(total).try_into().unwrap_or(usize::MAX));
            if samples.is_empty() || broken.is_some() {
                continue;
            }
            total += samples.len() as u64;
            let written = whole.as_mut().map_or(Ok(()), |w| w.append(&samples)).and_then(|()| chunker.push(&samples).into_iter().try_for_each(&mut save));
            if let Err(e) = written {
                broken = Some(e);
                self.stop_for(Some(session.id), StopReason::DiskFull);
                continue;
            }
            if total >= limits.max_samples {
                self.stop_for(Some(session.id), StopReason::TimeLimit);
            } else if total - checked >= limits.disk_check_every {
                checked = total;
                let free = self.inner.free.lock().unwrap().clone();
                if free(&self.inner.dir).is_some_and(|f| f < limits.min_free_bytes) {
                    self.stop_for(Some(session.id), StopReason::DiskFull);
                }
            }
        }
        // The source may end on its own (the device went away).
        self.stop_for(Some(session.id), StopReason::User);
        if let Some(rest) = chunker.finish() {
            if let Err(e) = save(rest) {
                broken.get_or_insert(e);
            }
        }
        drop(pieces);
        let transcript = match recognising {
            Some(task) => task.await.map_err(|e| e.to_string())?,
            None => recognise(session.backend.clone(), queue.take().expect("the queue"), draft.clone(), lang.clone()).await,
        };
        let whole = whole.map(Wav::finish).transpose()?;
        if transcript.is_empty() {
            let _ = std::fs::remove_file(&draft);
            let _ = std::fs::remove_dir_all(&session.tmp);
            return match broken {
                Some(e) => Err(e),
                None => {
                    self.emit(Event::Empty);
                    Ok(())
                }
            };
        }
        let summary = summarize(session.backend.as_ref(), &transcript, &lang).await;
        let title = text::title_of(&transcript, &lang);
        let minutes = (total / RATE as u64).div_ceil(60);
        let w = &session.when;
        let shown = format!("{:04}-{:02}-{:02} {:02}:{:02}", w.year, w.month, w.day, w.hour, w.minute);
        let body = text::markdown(&title, &shown, minutes, summary.as_ref(), &transcript, &lang);
        let path = free_name(&self.inner.dir, &format!("{} {}", stamp(w, '-'), text::file_safe(&title)), "md");
        std::fs::write(&path, body).map_err(|e| format!("Cannot save the note: {e}"))?;
        let _ = std::fs::remove_file(&draft);
        if let Some(sound) = whole {
            std::fs::rename(sound, path.with_extension("wav")).map_err(|e| format!("Cannot keep the sound: {e}"))?;
        }
        let _ = std::fs::remove_dir_all(&session.tmp);
        // The folder of the temporary sound goes too, once no other recording uses it.
        let _ = std::fs::remove_dir(self.inner.dir.join(".tmp"));
        self.emit(Event::Saved { title, path, summarized: summary.is_some() });
        Ok(())
    }

    /// The saved notes, the newest first.
    pub fn list(&self) -> Vec<Entry> {
        let Ok(dir) = std::fs::read_dir(&self.inner.dir) else { return Vec::new() };
        let mut out: Vec<Entry> = dir
            .flatten()
            .filter_map(|f| {
                let path = f.path();
                if path.extension()? != "md" || f.metadata().ok()?.len() > MAX_SHOWN {
                    return None;
                }
                let id = path.file_stem()?.to_str()?.to_string();
                let markdown = std::fs::read_to_string(&path).ok()?;
                let title = markdown.lines().find_map(|l| l.strip_prefix("# ")).unwrap_or(&id).to_string();
                let date = id.get(..16).map_or_else(String::new, |d| format!("{} {}", &d[..10], d.get(11..).unwrap_or("").replace('-', ":")));
                Some(Entry { id, title, date, markdown })
            })
            .collect();
        out.sort_by(|a, b| b.id.cmp(&a.id));
        out
    }

    /// Removes one note (and its sound). `id` is a file name of `list`, never a path.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        if id.is_empty() || id.contains(['/', '\\', ':']) || id.contains("..") {
            return Err("Unknown note".into());
        }
        let path = self.inner.dir.join(format!("{id}.md"));
        std::fs::remove_file(&path).map_err(|e| format!("Cannot delete the note: {e}"))?;
        let _ = std::fs::remove_file(path.with_extension("wav"));
        Ok(())
    }
}

fn stamp(w: &Local, sep: char) -> String {
    format!("{:04}-{:02}-{:02} {:02}{sep}{:02}", w.year, w.month, w.day, w.hour, w.minute)
}

/// `<dir>\<name>.<ext>`, or `<name> (2).<ext>` and so on when it is taken.
fn free_name(dir: &Path, name: &str, ext: &str) -> PathBuf {
    let mut path = dir.join(format!("{name}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{name} ({n}).{ext}"));
        n += 1;
    }
    path
}

/// Recognises the pieces in their order. Each piece leaves the disk once it was tried; the text so far is kept in `draft`,
/// so that a crash does not lose the meeting.
async fn recognise(backend: Arc<dyn Backend>, mut queue: UnboundedReceiver<PathBuf>, draft: PathBuf, lang: String) -> String {
    let mut parts: Vec<String> = Vec::new();
    while let Some(path) = queue.recv().await {
        let Ok(pcm) = std::fs::read(&path) else { continue };
        let mut answer = backend.transcribe(pcm.clone(), lang.clone()).await;
        if answer.is_err() {
            answer = backend.transcribe(pcm, lang.clone()).await;
        }
        let _ = std::fs::remove_file(&path);
        match answer {
            Ok(text) => parts.push(text),
            Err(e) => {
                tracing::warn!("note: a piece was not recognised: {e}");
                parts.push("[…]".into());
            }
        }
        let so_far = text::stitch(&parts);
        if !so_far.is_empty() {
            let w = text::words(&lang);
            if let Err(e) = std::fs::write(&draft, format!("# {}\n\n## {}\n\n{so_far}\n", w.note, w.full)) {
                tracing::warn!("note: the draft was not written: {e}");
            }
        }
    }
    // A recording where nothing but failures came back has no text.
    if parts.iter().all(|p| p == "[…]") {
        return String::new();
    }
    text::stitch(&parts)
}

async fn summarize(backend: &dyn Backend, transcript: &str, lang: &str) -> Option<Summary> {
    let mut parts = Vec::new();
    for piece in text::split_pieces(transcript, text::PIECE_CHARS) {
        if let Some(s) = backend.model(text::part_request(&piece, lang)).await.as_deref().and_then(text::parse_summary) {
            parts.push(s);
        }
    }
    if parts.is_empty() {
        return None;
    }
    let mut all = text::merge(&parts);
    if parts.len() > 1 {
        // The summaries of the parts become one; of a very long meeting in more than one round.
        let mut whole = all.summary.clone();
        for _ in 0..3 {
            let mut shorter = Vec::new();
            for piece in text::split_pieces(&whole, text::PIECE_CHARS) {
                match backend.model(text::overall_request(&piece, lang)).await.as_deref().and_then(text::parse_summary) {
                    Some(s) if !s.summary.is_empty() => shorter.push(s.summary),
                    _ => return Some(all),
                }
            }
            whole = shorter.join("\n\n");
            if shorter.len() <= 1 {
                break;
            }
        }
        all.summary = whole;
    }
    Some(all)
}

/// A 16 kHz mono PCM16 file written as it grows; the lengths in its header are set at the end.
struct Wav {
    path: PathBuf,
    file: std::fs::File,
    bytes: u32,
}

impl Wav {
    fn create(path: &Path) -> Result<Wav, String> {
        let mut file = std::fs::File::create(path).map_err(|e| format!("Cannot write the recording: {e}"))?;
        file.write_all(&crate::voice::audio::make_wav_header(0, RATE as u32, 1, 16)).map_err(|e| format!("Cannot write the recording: {e}"))?;
        Ok(Wav { path: path.to_path_buf(), file, bytes: 0 })
    }

    fn append(&mut self, samples: &[i16]) -> Result<(), String> {
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.file.write_all(&bytes).map_err(|e| format!("Cannot write the recording: {e}"))?;
        self.bytes = self.bytes.saturating_add(bytes.len() as u32);
        Ok(())
    }

    fn finish(mut self) -> Result<PathBuf, String> {
        let header = crate::voice::audio::make_wav_header(self.bytes, RATE as u32, 1, 16);
        self.file.rewind().and_then(|()| self.file.write_all(&header)).map_err(|e| format!("Cannot write the recording: {e}"))?;
        Ok(self.path)
    }
}

/// Free bytes of the disk that holds `path` (or its nearest folder that exists).
fn free_space(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let folder = path.ancestors().find(|p| p.is_dir())?;
        let wide: Vec<u16> = folder.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut free = 0u64;
        // SAFETY: a zero-terminated path and a local number that outlives the call.
        unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut free), None, None) }.ok()?;
        Some(free)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
mod tests;
