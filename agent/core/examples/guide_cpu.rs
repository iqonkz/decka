//! How much processor a waiting guide costs (TASKS.md T30.5): the guide shows one step on the window given by its handle
//! and waits; the processor time of this process (the reading of the window) and of the process of the window (which
//! answers UI Automation) is taken over the wait.
//!
//! `cargo run --release -p decka-core --example guide_cpu -- --hwnd <handle> --pid <pid of the window> [--seconds 60]`

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use decka_core::voice::guides::Guides;
use decka_core::voice::highlight::{Highlighter, Recorder};
use decka_core::voice::screenhelp::UiAction;
use decka_core::voice::screenrun::{BoxFut, NoGuideSink, Screen, ScreenRun, StepAsker};
use decka_core::voice::tools::GuideCtl;
use decka_core::voice::ui_snapshot::{self, Rect, Snapshot};
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

struct OneWindow {
    handle: isize,
    started: Instant,
    looks: Arc<AtomicU64>,
}

impl Screen for OneWindow {
    fn snapshot(&self) -> Option<Snapshot> {
        self.looks.fetch_add(1, Ordering::Relaxed);
        ui_snapshot::window(Some(self.handle))
    }
    fn process(&self) -> String {
        "guide-cpu.exe".into()
    }
    fn foreground(&self) -> u64 {
        self.handle as u64
    }
    fn perform(&self, _: Rect, _: UiAction) -> Result<String, String> {
        Err("a measurement presses nothing".into())
    }
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn click(&self) -> Option<(i32, i32)> {
        None
    }
    fn watch(&self, events: tokio::sync::mpsc::UnboundedSender<()>) -> Option<Box<dyn Send>> {
        decka_core::voice::uia_events::watch(Some(self.handle), events).map(|w| Box::new(w) as Box<dyn Send>)
    }
}

/// The first element of the window is the step.
struct First;

impl StepAsker for First {
    fn ask(&self, _: serde_json::Value) -> BoxFut<Option<String>> {
        Box::pin(async { Some(r#"{"next":1,"say":"Press it"}"#.to_string()) })
    }
}

/// Processor time (kernel and user) of a process so far, in milliseconds.
fn cpu_ms(pid: u32) -> u64 {
    // SAFETY: the handle is closed right after the query; the four structures are local.
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return 0 };
        let (mut a, mut b, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let ok = GetProcessTimes(process, &mut a, &mut b, &mut kernel, &mut user).is_ok();
        let _ = CloseHandle(process);
        let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
        if ok { (ticks(kernel) + ticks(user)) / 10_000 } else { 0 }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<i64>().ok());
    let (handle, pid) = (get("--hwnd").expect("--hwnd") as isize, get("--pid").expect("--pid") as u32);
    let seconds = get("--seconds").unwrap_or(60) as u64;
    // `--events`: only count the news of the window over the time (does UI Automation tell about it at all?).
    if args.iter().any(|a| a == "--events") {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let watch = decka_core::voice::uia_events::watch(Some(handle), tx);
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        drop(watch);
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        println!("news={count} in {seconds} s");
        return;
    }
    let looks = Arc::new(AtomicU64::new(0));
    let screen = Arc::new(OneWindow { handle, started: Instant::now(), looks: looks.clone() });
    let elements = ui_snapshot::window(Some(handle)).map_or(0, |s| s.elements.len());
    let dir = std::env::temp_dir().join("decka-guide-cpu");
    let run = Arc::new(ScreenRun::new(screen, Highlighter::new(Arc::new(Recorder::default())), Guides::new(&dir), Arc::new(NoGuideSink), vec![]));
    let running = run.clone();
    let guide = tokio::spawn(async move { running.guide("measure", "English", &First).await });
    // The first step is made before the measurement.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let (me, window, seen) = (cpu_ms(std::process::id()), cpu_ms(pid), looks.load(Ordering::Relaxed));
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    let (me, window, seen) = (cpu_ms(std::process::id()) - me, cpu_ms(pid) - window, looks.load(Ordering::Relaxed) - seen);
    let _ = run.control(GuideCtl::Stop);
    let end = guide.await;
    let share = |ms: u64| ms as f64 / (seconds as f64 * 10.0);
    println!("elements={elements} seconds={seconds} looks={seen} guide_cpu_ms={me} ({:.2}% of a core) window_cpu_ms={window} ({:.2}% of a core) end={end:?}", share(me), share(window));
    let _ = std::fs::remove_dir_all(dir);
}
