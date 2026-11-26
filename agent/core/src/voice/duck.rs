//! T29.3: while the microphone records (and while Decka speaks), every other sound of Windows is turned down to a fifth and
//! comes back afterwards. The levels from before are kept until they are back, also across a crash (`duck.json`). A session
//! the user changed meanwhile is left as the user set it. The steps are pure over `Mixer`; `win` is the Core Audio layer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// What is left of the volume while ducked.
pub const DUCK_TO: f32 = 0.2;
pub const DOWN: Duration = Duration::from_millis(150);
pub const UP: Duration = Duration::from_millis(300);
const STEP: Duration = Duration::from_millis(25);
/// A recording that restarts right away (the answer to a question) does not make the music jump up and down.
const RELEASE_AFTER: Duration = Duration::from_millis(250);
/// Windows reports volumes as floats: closer than this is "unchanged".
const SAME: f32 = 0.006;

/// One audio session of another program.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: String,
    pub pid: u32,
    pub volume: f32,
}

pub trait Mixer {
    fn sessions(&mut self) -> Vec<Session>;
    /// `None`: the session is gone.
    fn volume(&mut self, id: &str) -> Option<f32>;
    fn set(&mut self, id: &str, volume: f32) -> bool;
}

/// A session that was turned down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Held {
    pub id: String,
    /// The volume to give back.
    pub original: f32,
    /// What was set last: anything else means the user moved it.
    pub last: f32,
}

/// Takes the sessions that are not held yet (not our own, not silent ones).
pub fn take(held: &mut Vec<Held>, mixer: &mut dyn Mixer, own_pid: u32) {
    for s in mixer.sessions() {
        if s.pid != own_pid && s.volume > SAME && !held.iter().any(|h| h.id == s.id) {
            held.push(Held { id: s.id, original: s.volume, last: s.volume });
        }
    }
}

/// Moves every held session to `target(original)` in `steps` equal steps. Sessions that are gone or were changed by someone
/// else are dropped. `pause` waits one step and says how many steps should be done by now (steps are skipped when Windows
/// is slow, so the fade still ends in time); `None` stops it where it is (a newer order came).
pub fn fade(held: &mut Vec<Held>, mixer: &mut dyn Mixer, target: impl Fn(f32) -> f32, steps: u32, pause: &mut dyn FnMut() -> Option<u32>) -> bool {
    let from: Vec<(String, f32)> = held.iter().map(|h| (h.id.clone(), h.last)).collect();
    let steps = steps.max(1);
    let mut k = 1;
    loop {
        held.retain_mut(|h| {
            let Some(now) = mixer.volume(&h.id) else { return false };
            if (now - h.last).abs() > SAME {
                return false;
            }
            let start = from.iter().find(|(id, _)| *id == h.id).map_or(h.last, |(_, v)| *v);
            let goal = target(h.original);
            let next = if k == steps { goal } else { start + (goal - start) * k as f32 / steps as f32 };
            if !mixer.set(&h.id, next) {
                return false;
            }
            h.last = next;
            true
        });
        if k >= steps {
            return true;
        }
        let Some(due) = pause() else { return false };
        k = (k + 1).max(due).min(steps);
    }
}

fn steps(total: Duration) -> u32 {
    (total.as_millis() / STEP.as_millis()) as u32
}

/// No step is skipped and nothing interrupts: for a change that is made at once.
fn at_once() -> Option<u32> {
    Some(0)
}

/// The levels on the disk, for the next start after a crash.
fn save(path: &Path, held: &[Held]) {
    if held.is_empty() {
        let _ = std::fs::remove_file(path);
    } else if let Ok(text) = serde_json::to_string(held) {
        if let Err(e) = std::fs::write(path, text) {
            tracing::warn!("cannot remember the volumes: {e}");
        }
    }
}

fn load(path: &Path) -> Vec<Held> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

enum Order {
    Duck,
    Release,
    /// Give everything back now and say so.
    Quit(Sender<()>),
}

/// The thread that owns the mixer and obeys the orders.
fn work(orders: Receiver<Order>, mut mixer: Box<dyn Mixer>, file: PathBuf, own_pid: u32) {
    // Left over from a crash or a kill: give the volumes back first.
    let mut held = load(&file);
    if !held.is_empty() {
        fade(&mut held, mixer.as_mut(), |original| original, 1, &mut at_once);
        held.clear();
        save(&file, &held);
    }
    let mut next: Option<Order> = None;
    loop {
        let order = match next.take() {
            Some(o) => o,
            None => match orders.recv() {
                Ok(o) => o,
                Err(_) => Order::Quit(mpsc::channel().0),
            },
        };
        match order {
            Order::Duck => {
                take(&mut held, mixer.as_mut(), own_pid);
                // Written before the first change: a kill in the middle of the fade can still be undone.
                save(&file, &held);
                fade(&mut held, mixer.as_mut(), |original| original * DUCK_TO, steps(DOWN), &mut waiter(&orders, &mut next));
                save(&file, &held);
            }
            Order::Release => {
                match orders.recv_timeout(RELEASE_AFTER) {
                    Ok(newer) => {
                        next = Some(newer);
                        continue;
                    }
                    Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
                }
                if fade(&mut held, mixer.as_mut(), |original| original, steps(UP), &mut waiter(&orders, &mut next)) {
                    held.clear();
                }
                save(&file, &held);
            }
            Order::Quit(done) => {
                fade(&mut held, mixer.as_mut(), |original| original, 1, &mut at_once);
                held.clear();
                save(&file, &held);
                let _ = done.send(());
                return;
            }
        }
    }
}

/// The pause of a fade: waits one step and says how far the fade should be by now; `None` when a newer order arrived (kept
/// in `next`).
fn waiter<'a>(orders: &'a Receiver<Order>, next: &'a mut Option<Order>) -> impl FnMut() -> Option<u32> + 'a {
    let started = Instant::now();
    move || match orders.recv_timeout(STEP) {
        Ok(o) => {
            *next = Some(o);
            None
        }
        Err(_) => Some(1 + (started.elapsed().as_millis() / STEP.as_millis()) as u32),
    }
}

/// The handle of the ducking thread. Cheap to call: nothing waits for Windows on the caller.
pub struct Ducker {
    orders: Sender<Order>,
}

impl Ducker {
    pub fn start(config_dir: &Path) -> Ducker {
        let file = config_dir.join("duck.json");
        let (orders, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("decka-duck".into()).spawn(move || {
            #[cfg(windows)]
            work(rx, Box::new(win::SystemMixer::new()), file, std::process::id());
            #[cfg(not(windows))]
            work(rx, Box::new(NoMixer), file, std::process::id());
        });
        if let Err(e) = spawned {
            tracing::warn!("ducking thread: {e}");
        }
        Ducker { orders }
    }

    pub fn duck(&self) {
        let _ = self.orders.send(Order::Duck);
    }

    pub fn release(&self) {
        let _ = self.orders.send(Order::Release);
    }

    /// Gives every volume back before the process ends (waits at most a second).
    pub fn quit(&self) {
        let (done, wait) = mpsc::channel();
        if self.orders.send(Order::Quit(done)).is_ok() {
            let _ = wait.recv_timeout(Duration::from_secs(1));
        }
    }
}

#[cfg(not(windows))]
struct NoMixer;

#[cfg(not(windows))]
impl Mixer for NoMixer {
    fn sessions(&mut self) -> Vec<Session> {
        Vec::new()
    }
    fn volume(&mut self, _: &str) -> Option<f32> {
        None
    }
    fn set(&mut self, _: &str, _: f32) -> bool {
        false
    }
}

#[cfg(windows)]
mod win {
    use std::collections::HashMap;

    use windows::core::Interface;
    use windows::Win32::Media::Audio::{
        eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

    use super::{Mixer, Session};

    /// The audio sessions of every active output device. Lives on the ducking thread.
    pub struct SystemMixer {
        found: HashMap<String, ISimpleAudioVolume>,
    }

    impl SystemMixer {
        pub fn new() -> SystemMixer {
            // SAFETY: this thread stays in the multithreaded apartment for its whole life.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            SystemMixer { found: HashMap::new() }
        }

        fn look(&mut self) -> windows::core::Result<Vec<Session>> {
            self.found.clear();
            let mut out = Vec::new();
            // SAFETY: COM calls on this initialised thread; the strings returned by the system are freed after the copy.
            unsafe {
                let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let outputs = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
                for d in 0..outputs.GetCount()? {
                    let Ok(manager) = outputs.Item(d)?.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
                    let sessions = manager.GetSessionEnumerator()?;
                    for i in 0..sessions.GetCount()? {
                        let Ok(control) = sessions.GetSession(i).and_then(|s| s.cast::<IAudioSessionControl2>()) else { continue };
                        let Ok(volume) = control.cast::<ISimpleAudioVolume>() else { continue };
                        let Ok(raw) = control.GetSessionInstanceIdentifier() else { continue };
                        let id = raw.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(raw.0 as *const _));
                        let (Ok(pid), Ok(level)) = (control.GetProcessId(), volume.GetMasterVolume()) else { continue };
                        out.push(Session { id: id.clone(), pid, volume: level });
                        self.found.insert(id, volume);
                    }
                }
            }
            Ok(out)
        }
    }

    impl Mixer for SystemMixer {
        fn sessions(&mut self) -> Vec<Session> {
            self.look().unwrap_or_else(|e| {
                tracing::debug!("audio sessions: {e}");
                Vec::new()
            })
        }

        fn volume(&mut self, id: &str) -> Option<f32> {
            if !self.found.contains_key(id) {
                let _ = self.look();
            }
            // SAFETY: the interface was obtained on this thread.
            self.found.get(id).and_then(|v| unsafe { v.GetMasterVolume() }.ok())
        }

        fn set(&mut self, id: &str, volume: f32) -> bool {
            // SAFETY: the interface was obtained on this thread.
            self.found.get(id).is_some_and(|v| unsafe { v.SetMasterVolume(volume.clamp(0.0, 1.0), std::ptr::null()) }.is_ok())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Sessions shared between a test and the mixer it hands out.
    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<Vec<Session>>>);

    impl Fake {
        fn with(list: &[(&str, u32, f32)]) -> Fake {
            Fake(Arc::new(Mutex::new(list.iter().map(|(id, pid, volume)| Session { id: id.to_string(), pid: *pid, volume: *volume }).collect())))
        }
        fn at(&self, id: &str) -> f32 {
            self.0.lock().unwrap().iter().find(|s| s.id == id).unwrap().volume
        }
        fn user_sets(&self, id: &str, v: f32) {
            self.0.lock().unwrap().iter_mut().find(|s| s.id == id).unwrap().volume = v;
        }
    }

    impl Mixer for Fake {
        fn sessions(&mut self) -> Vec<Session> {
            self.0.lock().unwrap().clone()
        }
        fn volume(&mut self, id: &str) -> Option<f32> {
            self.0.lock().unwrap().iter().find(|s| s.id == id).map(|s| s.volume)
        }
        fn set(&mut self, id: &str, volume: f32) -> bool {
            self.0.lock().unwrap().iter_mut().find(|s| s.id == id).map(|s| s.volume = volume).is_some()
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    /// By hand: the volume of every session of other programs, ten times a second for 20 s (`DECKA_WATCH_PID` narrows it).
    /// `cargo test -p decka-core --lib watch_the_sessions -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn watch_the_sessions() {
        let only: Option<u32> = std::env::var("DECKA_WATCH_PID").ok().and_then(|v| v.parse().ok());
        let mut mixer = super::win::SystemMixer::new();
        let mut last = String::new();
        let started = std::time::Instant::now();
        while started.elapsed() < Duration::from_secs(20) {
            let now: Vec<String> = mixer.sessions().iter().filter(|s| only.is_none_or(|p| s.pid == p)).map(|s| format!("{}={:.3}", s.pid, s.volume)).collect();
            if now.join(" ") != last {
                last = now.join(" ");
                println!("{:>6} ms  {last}", started.elapsed().as_millis());
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    #[test]
    fn others_go_down_to_a_fifth_and_come_back_and_our_own_stays() {
        let mut m = Fake::with(&[("music", 10, 0.8), ("game", 11, 0.5), ("decka", 99, 1.0), ("silent", 12, 0.0)]);
        let mut held = Vec::new();
        take(&mut held, &mut m, 99);
        assert!(fade(&mut held, &mut m, |o| o * DUCK_TO, steps(DOWN), &mut at_once));
        assert!(close(m.at("music"), 0.16) && close(m.at("game"), 0.1));
        assert_eq!((m.at("decka"), m.at("silent")), (1.0, 0.0));
        // A second order while ducked keeps the first originals.
        take(&mut held, &mut m, 99);
        assert!(close(held.iter().find(|h| h.id == "music").unwrap().original, 0.8));
        assert!(fade(&mut held, &mut m, |o| o, steps(UP), &mut at_once));
        assert!(close(m.at("music"), 0.8) && close(m.at("game"), 0.5));
    }

    #[test]
    fn a_session_the_user_changed_is_left_as_the_user_set_it() {
        let mut m = Fake::with(&[("music", 10, 0.8), ("game", 11, 0.5)]);
        let mut held = Vec::new();
        take(&mut held, &mut m, 99);
        fade(&mut held, &mut m, |o| o * DUCK_TO, 6, &mut at_once);
        m.user_sets("music", 0.6);
        fade(&mut held, &mut m, |o| o, 12, &mut at_once);
        assert!(close(m.at("music"), 0.6) && close(m.at("game"), 0.5));
        assert_eq!(held.len(), 1);
    }

    #[test]
    fn a_fade_stops_for_a_newer_order_and_goes_on_from_there() {
        let mut m = Fake::with(&[("music", 10, 1.0)]);
        let mut held = Vec::new();
        take(&mut held, &mut m, 99);
        let mut left = 2;
        assert!(!fade(&mut held, &mut m, |o| o * DUCK_TO, 4, &mut || {
            left -= 1;
            (left > 0).then_some(0)
        }));
        assert!(close(m.at("music"), 0.6), "half way: {}", m.at("music"));
        assert!(fade(&mut held, &mut m, |o| o, 4, &mut at_once));
        assert!(close(m.at("music"), 1.0));
    }

    #[test]
    fn a_slow_mixer_skips_steps_and_still_arrives() {
        let mut m = Fake::with(&[("music", 10, 1.0)]);
        let mut held = Vec::new();
        take(&mut held, &mut m, 99);
        let mut pauses = 0;
        assert!(fade(&mut held, &mut m, |o| o * DUCK_TO, 6, &mut || {
            pauses += 1;
            Some(100)
        }));
        assert!(close(m.at("music"), 0.2) && pauses == 1);
    }

    #[test]
    fn the_thread_gives_volumes_back_on_quit_and_after_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("duck.json");
        let m = Fake::with(&[("music", 10, 0.9)]);
        let (tx, rx) = mpsc::channel();
        let (mixer, path) = (m.clone(), file.clone());
        let thread = std::thread::spawn(move || work(rx, Box::new(mixer), path, 99));
        tx.send(Order::Duck).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert!(close(m.at("music"), 0.18) && file.exists(), "ducked and remembered");
        let (done, wait) = mpsc::channel();
        tx.send(Order::Quit(done)).unwrap();
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
        thread.join().unwrap();
        assert!(close(m.at("music"), 0.9) && !file.exists());

        // A crash left the session ducked: the next start gives it back.
        m.user_sets("music", 0.18);
        std::fs::write(&file, r#"[{"id":"music","original":0.9,"last":0.18}]"#).unwrap();
        let (tx, rx) = mpsc::channel();
        let (mixer, path) = (m.clone(), file.clone());
        let thread = std::thread::spawn(move || work(rx, Box::new(mixer), path, 99));
        drop(tx);
        thread.join().unwrap();
        assert!(close(m.at("music"), 0.9) && !file.exists());
    }
}
