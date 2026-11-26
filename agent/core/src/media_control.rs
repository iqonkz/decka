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

/// Only these three go through the session with a key as the fallback; volume is always a key.
pub fn uses_session(op: MediaOp) -> bool {
    matches!(op, MediaOp::PlayPause | MediaOp::Next | MediaOp::Prev)
}

/// `shuffle` and `repeat` (M15): the session only, no key to fall back on (DATA_MODEL.md 2.3).
pub fn session_only(op: MediaOp) -> bool {
    matches!(op, MediaOp::Shuffle | MediaOp::Repeat)
}

/// The repeat mode after a press: `none -> list -> track -> none`.
pub fn next_repeat(current: &str) -> &'static str {
    match current {
        "none" => "list",
        "list" => "track",
        _ => "none",
    }
}

/// What a `shuffle` / `repeat` press answers when the session did not do it.
pub const UNSUPPORTED: &str = "The player does not support this";

pub fn op_name(op: MediaOp) -> &'static str {
    match op {
        MediaOp::PlayPause => "play_pause",
        MediaOp::Next => "next",
        MediaOp::Prev => "prev",
        MediaOp::VolUp => "vol_up",
        MediaOp::VolDown => "vol_down",
        MediaOp::Mute => "mute",
        MediaOp::Shuffle => "shuffle",
        MediaOp::Repeat => "repeat",
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

/// "Play music" (T29.2): starts the current media session if it is paused; a playing one is left alone. Blocks for at most
/// about 2 s. Anything but `Done` leaves it to the play/pause key.
pub fn resume() -> SessionOutcome {
    #[cfg(windows)]
    {
        win::resume()
    }
    #[cfg(not(windows))]
    {
        SessionOutcome::NoSession
    }
}

#[cfg(windows)]
mod win {
    use std::sync::mpsc::{self, Sender};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use windows::Media::Control::{GlobalSystemMediaTransportControlsSessionManager, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Playback};
    use windows::Media::MediaPlaybackAutoRepeatMode;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    use super::{next_repeat, MediaOp, SessionOutcome};

    const TIMEOUT: Duration = Duration::from_secs(2);
    /// The session needs a moment to report the new state before it is read again.
    const SETTLE: Duration = Duration::from_millis(150);

    struct Job {
        /// `None`: resume.
        op: Option<MediaOp>,
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

    fn run(manager: &mut Option<GlobalSystemMediaTransportControlsSessionManager>, op: Option<MediaOp>) -> SessionOutcome {
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
            None => {
                if session.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).is_ok_and(|s| s == Playback::Playing) {
                    return SessionOutcome::Done;
                }
                session.TryPlayAsync()
            }
            Some(op) => match op {
            MediaOp::PlayPause => session.TryTogglePlayPauseAsync(),
            MediaOp::Next => session.TrySkipNextAsync(),
            MediaOp::Prev => session.TrySkipPreviousAsync(),
            MediaOp::Shuffle => {
                // A source that reports no shuffle state does not support the command.
                let Ok(current) = session.GetPlaybackInfo().and_then(|i| i.IsShuffleActive()).and_then(|r| r.Value()) else {
                    return SessionOutcome::Refused;
                };
                session.TryChangeShuffleActiveAsync(!current)
            }
            MediaOp::Repeat => {
                let Ok(current) = session.GetPlaybackInfo().and_then(|i| i.AutoRepeatMode()).and_then(|r| r.Value()) else {
                    return SessionOutcome::Refused;
                };
                let current = match current {
                    MediaPlaybackAutoRepeatMode::List => "list",
                    MediaPlaybackAutoRepeatMode::Track => "track",
                    _ => "none",
                };
                session.TryChangeAutoRepeatModeAsync(match next_repeat(current) {
                    "list" => MediaPlaybackAutoRepeatMode::List,
                    "track" => MediaPlaybackAutoRepeatMode::Track,
                    _ => MediaPlaybackAutoRepeatMode::None,
                })
            }
            _ => return SessionOutcome::Error,
            },
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
        send(Some(op))
    }

    pub fn resume() -> SessionOutcome {
        send(None)
    }

    fn send(op: Option<MediaOp>) -> SessionOutcome {
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
    fn repeat_goes_round_none_list_track() {
        assert_eq!([next_repeat("none"), next_repeat("list"), next_repeat("track"), next_repeat("whatever")], ["list", "track", "none", "none"]);
    }

    #[test]
    fn shuffle_and_repeat_have_no_key_to_fall_back_on() {
        assert!(session_only(MediaOp::Shuffle) && session_only(MediaOp::Repeat));
        assert!(!uses_session(MediaOp::Shuffle) && !uses_session(MediaOp::Repeat));
        assert!(!session_only(MediaOp::PlayPause) && !session_only(MediaOp::Mute));
    }

    #[test]
    fn volume_never_goes_through_the_session() {
        assert!(uses_session(MediaOp::PlayPause) && uses_session(MediaOp::Next) && uses_session(MediaOp::Prev));
        assert!(!uses_session(MediaOp::VolUp) && !uses_session(MediaOp::VolDown) && !uses_session(MediaOp::Mute));
    }
}
