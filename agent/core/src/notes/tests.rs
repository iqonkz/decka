use std::collections::VecDeque;

use serde_json::json;

use super::*;

/// Recognition gives the next of `texts`; the model answers one fixed summary.
struct Fake {
    live: bool,
    texts: Mutex<VecDeque<String>>,
    heard: Mutex<Vec<usize>>,
    asked: Mutex<Vec<Value>>,
}

impl Fake {
    fn new(live: bool, texts: &[&str]) -> Arc<Fake> {
        Arc::new(Fake { live, texts: Mutex::new(texts.iter().map(|t| t.to_string()).collect()), heard: Mutex::default(), asked: Mutex::default() })
    }
}

impl Backend for Fake {
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn live(&self) -> bool {
        self.live
    }
    fn language(&self) -> String {
        "ru".into()
    }
    fn microphone(&self) -> Option<String> {
        None
    }
    fn transcribe(&self, pcm: Vec<u8>, _: String) -> BoxFut<Result<String, String>> {
        self.heard.lock().unwrap().push(pcm.len() / 2);
        let text = self.texts.lock().unwrap().pop_front().unwrap_or_default();
        Box::pin(async move { Ok(text) })
    }
    fn model(&self, body: Value) -> BoxFut<Option<String>> {
        self.asked.lock().unwrap().push(body);
        Box::pin(async { Some(json!({ "summary": "Обсудили релиз.", "decisions": ["Выпуск в пятницу"], "tasks": [], "questions": [] }).to_string()) })
    }
}

/// Gives all its sound at once; `open` keeps the recording running until `stop`.
struct Memory {
    seconds: u64,
    open: bool,
}

impl Source for Memory {
    fn start(&self, _: Option<String>, mut sink: Sink) -> Result<Box<dyn Send>, String> {
        for _ in 0..self.seconds * 10 {
            sink((0..RATE / 10).map(|i| if i % 2 == 0 { 3000 } else { -3000 }).collect());
        }
        Ok(if self.open { Box::new(sink) } else { Box::new(()) })
    }
}

fn notes(dir: &Path, backend: Arc<Fake>, seconds: u64, open: bool) -> (Notes, Arc<Mutex<Vec<Event>>>) {
    let notes = Notes::new(dir.to_path_buf(), tokio::runtime::Handle::current());
    notes.set_backend(backend);
    notes.set_source(Arc::new(Memory { seconds, open }));
    notes.set_free_space(Arc::new(|_| Some(u64::MAX)));
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    notes.listen(Arc::new(move |e| sink.lock().unwrap().push(e)));
    (notes, events)
}

async fn finished(notes: &Notes) {
    for _ in 0..500 {
        if !notes.status().processing && !notes.status().active {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the note did not finish");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recording_becomes_one_file_with_the_summary_and_the_stitched_text_and_no_sound() {
    for live in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let backend = Fake::new(live, &["Обсудили план релиза и", "решили выпустить в пятницу."]);
        let (notes, events) = notes(dir.path(), backend.clone(), 45, true);
        notes.start(false).unwrap();
        assert!(notes.status().active);
        assert_eq!(notes.start(false), Err(ALREADY.to_string()));
        assert!(notes.stop() && !notes.stop());
        finished(&notes).await;
        let list = notes.list();
        assert_eq!(list.len(), 1, "the draft is replaced by the note: {list:?}");
        assert_eq!(list[0].title, "Обсудили план релиза и решили выпустить");
        assert!(list[0].markdown.contains("Обсудили план релиза и решили выпустить в пятницу.") && list[0].markdown.contains("- Выпуск в пятницу") && list[0].markdown.contains("1 мин"), "{}", list[0].markdown);
        assert_eq!(backend.heard.lock().unwrap().iter().sum::<usize>(), 45 * RATE, "every sample was recognised once");
        assert!(!dir.path().join(".tmp").join("1").exists());
        assert!(std::fs::read_dir(dir.path()).unwrap().flatten().all(|f| f.path().extension().is_none_or(|x| x != "wav" && x != "pcm")), "no sound is kept");
        let events = events.lock().unwrap();
        assert!(matches!(events.as_slice(), [Event::Started, Event::Stopped(StopReason::User), Event::Saved { summarized: true, .. }]), "{events:?}");
        notes.delete(&list[0].id).unwrap();
        assert!(notes.list().is_empty());
        assert!(notes.delete("..\\x").is_err() && notes.delete("").is_err());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sound_is_kept_only_when_it_was_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let (notes, _) = notes(dir.path(), Fake::new(true, &["Привет всем."]), 3, true);
    notes.start(true).unwrap();
    notes.stop();
    finished(&notes).await;
    let wav = dir.path().join(format!("{}.wav", notes.list()[0].id));
    let bytes = std::fs::read(&wav).unwrap();
    assert_eq!(bytes.len(), 44 + 3 * RATE * 2);
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize, 3 * RATE * 2);
    notes.delete(&notes.list()[0].id).unwrap();
    assert!(!wav.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_time_limit_stops_the_note_and_saves_it() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Fake::new(true, &["Первая часть."]);
    let (notes, events) = notes(dir.path(), backend.clone(), 40, true);
    notes.set_limits(Limits { max_samples: 20 * RATE as u64, min_free_bytes: 0, disk_check_every: u64::MAX });
    notes.start(false).unwrap();
    finished(&notes).await;
    assert_eq!(backend.heard.lock().unwrap().iter().sum::<usize>(), 20 * RATE);
    assert!(events.lock().unwrap().contains(&Event::Stopped(StopReason::TimeLimit)));
    assert_eq!(notes.list().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_disk_stops_the_note_with_what_was_said_and_refuses_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let (notes, events) = notes(dir.path(), Fake::new(true, &["Успели сказать это."]), 12, true);
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    // Enough at the start, less than 1 GB at the first look after 10 s.
    notes.set_free_space(Arc::new(move |_| Some(if count.fetch_add(1, Ordering::SeqCst) == 0 { u64::MAX } else { 1000 })));
    notes.set_limits(Limits { max_samples: u64::MAX, min_free_bytes: 1 << 30, disk_check_every: 10 * RATE as u64 });
    notes.start(false).unwrap();
    finished(&notes).await;
    assert!(events.lock().unwrap().contains(&Event::Stopped(StopReason::DiskFull)));
    assert_eq!(notes.list().len(), 1);
    assert_eq!(notes.start(false), Err(NO_DISK.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn silence_leaves_no_file_and_no_backend_means_no_recording() {
    let dir = tempfile::tempdir().unwrap();
    let bare = Notes::new(dir.path().to_path_buf(), tokio::runtime::Handle::current());
    assert_eq!(bare.start(false), Err(NOT_READY.to_string()));
    let (notes, events) = notes(dir.path(), Fake::new(true, &["Продолжение следует..."]), 4, false);
    notes.start(false).unwrap();
    finished(&notes).await;
    assert!(notes.list().is_empty());
    assert!(events.lock().unwrap().contains(&Event::Empty));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_long_text_is_summed_up_part_by_part_and_then_as_a_whole() {
    let backend = Fake::new(true, &[]);
    let long = "Это предложение про работу над проектом. ".repeat(400);
    let summary = summarize(backend.as_ref(), &long, "ru").await.unwrap();
    assert_eq!(summary.summary, "Обсудили релиз.");
    assert_eq!(summary.decisions, ["Выпуск в пятницу"], "repeats are merged");
    let asked = backend.asked.lock().unwrap();
    assert!(asked.len() >= 5, "four parts and the whole: {}", asked.len());
    assert!(asked.iter().all(|b| b["messages"][1]["content"].as_str().unwrap().chars().count() <= text::PIECE_CHARS));
}
