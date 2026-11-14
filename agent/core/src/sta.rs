//! One dedicated thread with COM in a single-threaded apartment for Shell calls (icons, shortcuts) and the
//! WinRT imaging that goes with them. Jobs run one after another; callers block until their job is done.

use std::panic::AssertUnwindSafe;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};

type Job = Box<dyn FnOnce() + Send>;

static SENDER: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();

fn sender() -> mpsc::Sender<Job> {
    SENDER
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("decka-shell-sta".to_string())
                .spawn(move || {
                    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
                    // SAFETY: first call on this thread, balanced by the thread's life.
                    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok() {
                        tracing::warn!("shell thread: COM init failed: {e}");
                    }
                    for job in rx {
                        // A panicking job must not kill the thread for everyone else.
                        let _ = std::panic::catch_unwind(AssertUnwindSafe(job));
                    }
                })
                .expect("spawn shell thread");
            Mutex::new(tx)
        })
        .lock()
        .unwrap()
        .clone()
}

/// Runs `f` on the shell thread and waits for its result.
pub fn run<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> Result<R, String> {
    let (result_tx, result_rx) = mpsc::channel();
    sender()
        .send(Box::new(move || {
            let _ = result_tx.send(f());
        }))
        .map_err(|_| "the shell thread is not running".to_string())?;
    result_rx.recv().map_err(|_| "the shell operation failed unexpectedly".to_string())
}
