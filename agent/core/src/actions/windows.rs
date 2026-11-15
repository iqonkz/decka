use super::ActionRunner;
use crate::model::Action;

/// Real executor for Windows. On other platforms every action fails with "unsupported platform".
///
/// Keyboard emulation runs on one dedicated OS thread that owns the `Enigo` instance
/// (it is not guaranteed to be `Send`); jobs reach it through an mpsc channel.
pub struct WindowsRunner {
    #[cfg(windows)]
    keyboard: std::sync::mpsc::Sender<keyboard::Job>,
}

#[cfg(windows)]
mod keyboard {
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::Duration;

    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    pub enum Keys {
        /// Press in order, pause, release in reverse order.
        Hotkey(Vec<Key>),
        Click(Key),
    }

    pub struct Job {
        pub keys: Keys,
        pub reply: Sender<Result<(), String>>,
    }

    fn hotkey(enigo: &mut Enigo, keys: Vec<Key>) -> Result<(), String> {
        let mut pressed: Vec<Key> = Vec::new();
        for key in keys {
            if let Err(e) = enigo.key(key, Direction::Press) {
                for p in pressed.iter().rev() {
                    let _ = enigo.key(*p, Direction::Release);
                }
                return Err(e.to_string());
            }
            pressed.push(key);
        }
        std::thread::sleep(Duration::from_millis(20));
        let mut first_error = None;
        for p in pressed.iter().rev() {
            if let Err(e) = enigo.key(*p, Direction::Release) {
                first_error.get_or_insert(e.to_string());
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn serve(rx: Receiver<Job>) {
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string());
        for job in rx {
            let result = match &mut enigo {
                Ok(enigo) => match job.keys {
                    Keys::Hotkey(keys) => hotkey(enigo, keys),
                    Keys::Click(key) => enigo.key(key, Direction::Click).map_err(|e| e.to_string()),
                },
                Err(e) => Err(format!("keyboard unavailable: {e}")),
            };
            let _ = job.reply.send(result);
        }
    }

    pub fn start() -> Sender<Job> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || serve(rx));
        tx
    }
}

#[cfg(windows)]
impl WindowsRunner {
    pub fn new() -> WindowsRunner {
        WindowsRunner { keyboard: keyboard::start() }
    }

    fn send_keys(&self, keys: keyboard::Keys) -> Result<(), String> {
        let (reply, answer) = std::sync::mpsc::channel();
        self.keyboard.send(keyboard::Job { keys, reply }).map_err(|_| "keyboard thread stopped".to_string())?;
        answer.recv().map_err(|_| "keyboard thread stopped".to_string())?
    }
}

#[cfg(windows)]
impl ActionRunner for WindowsRunner {
    fn run(&self, _id: &str, action: &Action) -> Result<(), String> {
        use super::{keys, process};
        match action {
            Action::Launch { program, args, cwd, hidden, .. } => {
                process::launch(program, args.as_deref().unwrap_or(&[]), cwd.as_deref(), hidden.unwrap_or(false))
            }
            Action::OpenUrl { url, .. } => process::open_url(url),
            Action::System { op, .. } => process::system(*op),
            Action::Hotkey { keys: names, .. } => {
                let mapped: Option<Vec<_>> = names.iter().map(|n| keys::map_key(n)).collect();
                self.send_keys(keyboard::Keys::Hotkey(mapped.ok_or("unknown key name")?))
            }
            Action::Media { op, .. } => {
                use crate::media_control::{choose_path, op_name, try_session, uses_session, Path};
                if uses_session(*op) && choose_path(try_session(*op)) == Path::Session {
                    tracing::info!("media {} via session", op_name(*op));
                    return Ok(());
                }
                tracing::info!("media {} via key", op_name(*op));
                self.send_keys(keyboard::Keys::Click(keys::map_media(*op)))
            }
        }
    }
}

#[cfg(not(windows))]
impl WindowsRunner {
    pub fn new() -> WindowsRunner {
        WindowsRunner {}
    }
}

#[cfg(not(windows))]
impl ActionRunner for WindowsRunner {
    fn run(&self, _id: &str, _action: &Action) -> Result<(), String> {
        Err("unsupported platform".to_string())
    }
}

impl Default for WindowsRunner {
    fn default() -> Self {
        Self::new()
    }
}
