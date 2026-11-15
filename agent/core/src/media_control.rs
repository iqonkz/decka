//! Play/pause/next/previous through the current Windows media session (T12.0, DATA_MODEL.md 2.3).
//! Synthetic media keys are not reliable (Spotify ignores them in some setups); the session is asked first and
//! the key path stays as the fallback.

use crate::model::MediaOp;

/// What the session said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOutcome {
    /// The session accepted the command.
    Done,
    /// There is no current session.
    NoSession,
    /// The session refused (`false`).
    Refused,
    /// WinRT failed or timed out.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Path {
    Session,
    Key,
}

/// `session ok` -> the session did it; anything else -> the media key.
pub fn choose_path(outcome: SessionOutcome) -> Path {
    match outcome {
        SessionOutcome::Done => Path::Session,
        SessionOutcome::NoSession | SessionOutcome::Refused | SessionOutcome::Error => Path::Key,
    }
}

/// Only these three go through the session; volume is always a key.
pub fn uses_session(op: MediaOp) -> bool {
    matches!(op, MediaOp::PlayPause | MediaOp::Next | MediaOp::Prev)
}

pub fn op_name(op: MediaOp) -> &'static str {
    match op {
        MediaOp::PlayPause => "play_pause",
        MediaOp::Next => "next",
        MediaOp::Prev => "prev",
        MediaOp::VolUp => "vol_up",
        MediaOp::VolDown => "vol_down",
        MediaOp::Mute => "mute",
    }
}

/// Asks the current media session to do `op`. Blocks for at most about 2 s. Other platforms: `NoSession`.
pub fn try_session(op: MediaOp) -> SessionOutcome {
    #[cfg(windows)]
    {
        win::try_session(op)
    }
    #[cfg(not(windows))]
    {
        let _ = op;
        SessionOutcome::NoSession
    }
}

#[cfg(windows)]
mod win {
    use std::sync::mpsc::{self, Sender};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    use super::{MediaOp, SessionOutcome};

    const TIMEOUT: Duration = Duration::from_secs(2);
    /// The session needs a moment to report the new state before it is read again.
    const SETTLE: Duration = Duration::from_millis(150);

    struct Job {
        op: MediaOp,
        reply: Sender<SessionOutcome>,
    }

    static WORKER: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

    fn worker() -> Sender<Job> {
        WORKER
            .get_or_init(|| {
                let (tx, rx) = mpsc::channel::<Job>();
                std::thread::Builder::new()
                    .name("decka-media-control".to_string())
                    .spawn(move || {
                        // SAFETY: first WinRT call on this thread.
                        if let Err(e) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                            tracing::warn!("media control: WinRT is unavailable: {e}");
                        }
                        let mut manager: Option<GlobalSystemMediaTransportControlsSessionManager> = None;
                        for job in rx {
                            let outcome = run(&mut manager, job.op);
                            let _ = job.reply.send(outcome);
                            if outcome == SessionOutcome::Done {
                                std::thread::sleep(SETTLE);
                                crate::collectors::media::request_refresh();
                            }
                        }
                    })
                    .expect("spawn media control thread");
                Mutex::new(tx)
            })
            .lock()
            .unwrap()
            .clone()
    }

    fn run(manager: &mut Option<GlobalSystemMediaTransportControlsSessionManager>, op: MediaOp) -> SessionOutcome {
        if manager.is_none() {
            match GlobalSystemMediaTransportControlsSessionManager::RequestAsync().and_then(|r| r.join()) {
                Ok(m) => *manager = Some(m),
                Err(e) => {
                    tracing::debug!("media control: no session manager: {e}");
                    return SessionOutcome::Error;
                }
            }
        }
        let Ok(session) = manager.as_ref().expect("set above").GetCurrentSession() else {
            return SessionOutcome::NoSession;
        };
        let started = match op {
            MediaOp::PlayPause => session.TryTogglePlayPauseAsync(),
            MediaOp::Next => session.TrySkipNextAsync(),
            MediaOp::Prev => session.TrySkipPreviousAsync(),
            _ => return SessionOutcome::Error,
        };
        // Waits for the operation, but never longer than TIMEOUT (`AsyncStatus::Started` is 0).
        let waited = started.and_then(|o| {
            let begun = Instant::now();
            while o.Status()?.0 == 0 {
                if begun.elapsed() >= TIMEOUT {
                    let _ = o.Cancel();
                    return Err(windows::core::Error::from(windows::Win32::Foundation::E_ABORT));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            o.GetResults()
        });
        match waited {
            Ok(true) => SessionOutcome::Done,
            Ok(false) => SessionOutcome::Refused,
            Err(e) => {
                tracing::debug!("media control: {e}");
                *manager = None; // a stale manager is renewed next time
                SessionOutcome::Error
            }
        }
    }

    pub fn try_session(op: MediaOp) -> SessionOutcome {
        let (reply, answer) = mpsc::channel();
        if worker().send(Job { op, reply }).is_err() {
            return SessionOutcome::Error;
        }
        answer.recv_timeout(TIMEOUT + Duration::from_millis(500)).unwrap_or(SessionOutcome::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_accepted_session_command_skips_the_key() {
        assert_eq!(choose_path(SessionOutcome::Done), Path::Session);
        for outcome in [SessionOutcome::NoSession, SessionOutcome::Refused, SessionOutcome::Error] {
            assert_eq!(choose_path(outcome), Path::Key, "{outcome:?}");
        }
    }

    #[test]
    fn volume_never_goes_through_the_session() {
        assert!(uses_session(MediaOp::PlayPause) && uses_session(MediaOp::Next) && uses_session(MediaOp::Prev));
        assert!(!uses_session(MediaOp::VolUp) && !uses_session(MediaOp::VolDown) && !uses_session(MediaOp::Mute));
    }
}
