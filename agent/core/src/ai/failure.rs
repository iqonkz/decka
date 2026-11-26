//! Why a local server did not start or stopped (TASKS.md T28.2): one reason for the user, told from the output of
//! `llama-server` / `whisper-server` and the exit code. The settings window and the island show the reason, never the raw line.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Not enough video memory, and the retry on the processor did not help either.
    Vram,
    /// Not enough memory of the PC.
    Ram,
    /// A model file cannot be read or is damaged: `Download again`.
    ModelFile,
    /// The port is taken.
    Port,
    /// The process stopped; `Failure::code` is its exit code.
    Crash,
    /// No microphone or no access to it.
    Mic,
    /// Speech recognition did not start.
    Stt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Server {
    Llm,
    Stt,
}

impl Server {
    pub fn name(self) -> &'static str {
        match self {
            Server::Llm => "llama-server",
            Server::Stt => "whisper-server",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub server: Server,
    pub reason: Reason,
    /// The exit code of the process, when it exited by itself.
    pub code: Option<i32>,
    /// The line of its output that tells the most (for the log and the settings window).
    pub detail: String,
}

/// `STATUS_NO_MEMORY`.
const EXIT_NO_MEMORY: i32 = 0xC000_0017_u32 as i32;

fn rank(reason: Reason) -> u8 {
    match reason {
        Reason::Vram => 4,
        Reason::Ram => 3,
        Reason::Port => 2,
        Reason::ModelFile => 1,
        _ => 0,
    }
}

/// What one line of the output says about a failure; `None` for an ordinary line.
pub fn line_hint(line: &str) -> Option<Reason> {
    let l = line.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| l.contains(n));
    if has(&["erroroutofdevicememory", "error_out_of_device_memory", "out of device memory", "failed to allocate vulkan", "unable to allocate vulkan", "cuda error: out of memory", "cudamalloc failed", "failed to allocate cuda"])
        || (l.contains("device memory allocation") && l.contains("failed"))
    {
        Some(Reason::Vram)
    } else if has(&["erroroutofhostmemory", "bad_alloc", "not enough memory", "failed to allocate cpu", "unable to allocate cpu", "failed to allocate buffer", "insufficient system resources", "paging file is too small"]) {
        Some(Reason::Ram)
    } else if has(&["couldn't bind", "could not bind", "failed to bind", "address already in use", "only one usage of each socket address"]) {
        Some(Reason::Port)
    } else if has(&["failed to load model", "error loading model", "invalid magic", "bad magic", "wrong magic", "failed to open gguf", "failed to read magic", "gguf_init_from_file", "not within the file bounds", "corrupted", "unknown model architecture", "failed to load mmproj", "failed to load multimodal", "failed to initialize whisper context"]) {
        Some(Reason::ModelFile)
    } else {
        None
    }
}

/// The stronger of two hints: a card that ran out of memory also prints `failed to load model` afterwards.
pub fn stronger(old: Option<(Reason, String)>, line: &str) -> Option<(Reason, String)> {
    match line_hint(line) {
        Some(new) if old.as_ref().is_none_or(|(r, _)| rank(new) > rank(*r)) => Some((new, line.to_string())),
        _ => old,
    }
}

/// The reason of a failed start or a crash. `why` is what the supervisor saw (`exited (...)`, `did not become ready in time`).
pub fn classify(server: Server, hint: Option<(Reason, String)>, code: Option<i32>, last: &str, why: &str) -> Failure {
    let seen = hint.as_ref().map(|(r, _)| *r).or((code == Some(EXIT_NO_MEMORY)).then_some(Reason::Ram));
    let reason = match (server, seen) {
        (Server::Llm, Some(r)) => r,
        (Server::Llm, None) => Reason::Crash,
        // Whatever else keeps the recognition from starting is `stt`.
        (Server::Stt, Some(r @ (Reason::Port | Reason::ModelFile))) => r,
        (Server::Stt, _) => Reason::Stt,
    };
    let detail = match hint {
        Some((_, line)) => line,
        None if !last.is_empty() => last.to_string(),
        None => format!("{}: {why}", server.name()),
    };
    Failure { server, reason, code, detail }
}

/// After the retry on the processor failed too: its own reason, except that a bare crash after a card out of memory is
/// still about the video memory.
pub fn after_cpu_retry(first: &Failure, second: Failure) -> Failure {
    if first.reason == Reason::Vram && second.reason == Reason::Crash {
        Failure { reason: Reason::Vram, ..second }
    } else {
        second
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(server: Server, lines: &[&str], code: Option<i32>) -> Failure {
        let hint = lines.iter().fold(None, |h, l| stronger(h, l));
        classify(server, hint, code, lines.last().unwrap_or(&""), "exited")
    }

    #[test]
    fn video_memory_wins_over_the_general_failure_that_follows_it() {
        let f = read(Server::Llm, &["ggml_vulkan: Device memory allocation of size 2684354560 failed.", "ggml_vulkan: vk::Device::allocateMemory: ErrorOutOfDeviceMemory", "llama_model_load: error loading model: unable to allocate Vulkan0 buffer", "main: exiting due to model loading error"], Some(1));
        assert_eq!((f.reason, f.code), (Reason::Vram, Some(1)));
        assert!(f.detail.contains("Device memory allocation"), "{}", f.detail);
    }

    #[test]
    fn every_reason_has_its_lines() {
        assert_eq!(read(Server::Llm, &["ggml_backend_cpu_buffer_type_alloc_buffer: failed to allocate buffer of size 99", "failed to load model"], Some(1)).reason, Reason::Ram);
        assert_eq!(read(Server::Llm, &[], Some(EXIT_NO_MEMORY)).reason, Reason::Ram);
        assert_eq!(read(Server::Llm, &["gguf_init_from_file_ptr: invalid magic characters: 'abcd', expected 'GGUF'", "main: exiting due to model loading error"], Some(1)).reason, Reason::ModelFile);
        assert_eq!(read(Server::Llm, &["main: couldn't bind HTTP server socket, hostname: 127.0.0.1, port: 5000"], Some(1)).reason, Reason::Port);
        assert_eq!(read(Server::Llm, &["something odd"], Some(3)).reason, Reason::Crash);
        assert_eq!(read(Server::Stt, &["CUDA error: out of memory"], Some(1)).reason, Reason::Stt);
        assert_eq!(read(Server::Stt, &["error: failed to initialize whisper context"], Some(3)).reason, Reason::ModelFile);
        assert_eq!(read(Server::Stt, &[], None).reason, Reason::Stt);
        assert_eq!(read(Server::Stt, &[], None).detail, "whisper-server: exited");
        assert_eq!(serde_json::to_value(Reason::ModelFile).unwrap(), "model_file");
    }

    #[test]
    fn a_crash_on_the_processor_after_a_full_card_is_still_the_video_memory() {
        let vram = read(Server::Llm, &["ErrorOutOfDeviceMemory"], Some(1));
        let crash = read(Server::Llm, &["??"], Some(5));
        assert_eq!(after_cpu_retry(&vram, crash.clone()).reason, Reason::Vram);
        let ram = read(Server::Llm, &["std::bad_alloc"], Some(1));
        assert_eq!(after_cpu_retry(&vram, ram).reason, Reason::Ram);
        assert_eq!(after_cpu_retry(&crash, crash.clone()).reason, Reason::Crash);
    }
}
