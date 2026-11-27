//! T32.1: the worker process of the wake word (`decka-agent --wake-worker ...`). It loads the sherpa-onnx C API of the
//! downloaded runtime (version pinned by the hash of the manifest, so the layout of its structs is fixed), listens to the
//! microphone and prints `ready`, `wake` or `error <text>`. A fault inside the native library ends this process only.
//! It ends when its standard input closes, that is, when the agent is gone. Nothing it hears is stored.

use std::path::PathBuf;

use super::wake::Sensitivity;

/// The first argument that turns the executable into the worker.
pub const FLAG: &str = "--wake-worker";

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerArgs {
    pub library: PathBuf,
    pub model: PathBuf,
    pub sensitivity: Sensitivity,
    pub mic: Option<String>,
}

impl WorkerArgs {
    pub fn to_args(&self) -> Vec<std::ffi::OsString> {
        let mut out: Vec<std::ffi::OsString> = vec![FLAG.into(), "--library".into(), self.library.clone().into(), "--model".into(), self.model.clone().into(), "--sensitivity".into(), self.sensitivity.name().into()];
        if let Some(mic) = &self.mic {
            out.push("--mic".into());
            out.push(mic.into());
        }
        out
    }

    pub fn parse(args: &[String]) -> Option<WorkerArgs> {
        if args.first().map(String::as_str) != Some(FLAG) {
            return None;
        }
        let value = |name: &str| args.chunks_exact(1).enumerate().find(|(_, a)| a[0] == name).and_then(|(i, _)| args.get(i + 1).cloned());
        Some(WorkerArgs {
            library: value("--library")?.into(),
            model: value("--model")?.into(),
            sensitivity: Sensitivity::parse(&value("--sensitivity")?)?,
            mic: value("--mic"),
        })
    }
}

#[cfg(windows)]
pub use engine::{run_worker, Spotter};

#[cfg(windows)]
mod engine {
    use std::ffi::{c_char, c_void, CString};
    use std::io::{Read, Write};
    use std::path::Path;
    use std::ptr::null;
    use std::sync::mpsc;

    use windows::core::{HSTRING, PCSTR};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH};

    use super::WorkerArgs;
    use crate::voice::wake::{self, Gate, Gated};

    // The structs of `c-api.h`, sherpa-onnx v2.0.3, field by field.
    #[repr(C)]
    struct Transducer {
        encoder: *const c_char,
        decoder: *const c_char,
        joiner: *const c_char,
    }

    #[repr(C)]
    struct ModelConfig {
        transducer: Transducer,
        paraformer: [*const c_char; 2],
        zipformer2_ctc: *const c_char,
        tokens: *const c_char,
        num_threads: i32,
        provider: *const c_char,
        debug: i32,
        model_type: *const c_char,
        modeling_unit: *const c_char,
        bpe_vocab: *const c_char,
        tokens_buf: *const c_char,
        tokens_buf_size: i32,
        nemo_ctc: *const c_char,
        t_one_ctc: *const c_char,
    }

    #[repr(C)]
    struct SpotterConfig {
        sample_rate: i32,
        feature_dim: i32,
        model: ModelConfig,
        max_active_paths: i32,
        num_trailing_blanks: i32,
        keywords_score: f32,
        keywords_threshold: f32,
        keywords_file: *const c_char,
        keywords_buf: *const c_char,
        keywords_buf_size: i32,
    }

    /// Only the first field of `SherpaOnnxKeywordResult` is read.
    #[repr(C)]
    struct Found {
        keyword: *const c_char,
    }

    type Create = unsafe extern "C" fn(*const SpotterConfig) -> *const c_void;
    type Destroy = unsafe extern "C" fn(*const c_void);
    type CreateStream = unsafe extern "C" fn(*const c_void) -> *const c_void;
    type Ready = unsafe extern "C" fn(*const c_void, *const c_void) -> i32;
    type Step = unsafe extern "C" fn(*const c_void, *const c_void);
    type Result_ = unsafe extern "C" fn(*const c_void, *const c_void) -> *const Found;
    type DestroyResult = unsafe extern "C" fn(*const Found);
    type Accept = unsafe extern "C" fn(*const c_void, i32, *const f32, i32);

    struct Api {
        create: Create,
        destroy: Destroy,
        create_stream: CreateStream,
        destroy_stream: Destroy,
        ready: Ready,
        decode: Step,
        reset: Step,
        result: Result_,
        destroy_result: DestroyResult,
        accept: Accept,
    }

    fn load(library: &Path) -> Result<Api, String> {
        if !library.is_absolute() || !library.is_file() {
            return Err("the wake word library is missing".into());
        }
        // The altered search path takes onnxruntime.dll from the folder of the library, not the older one of Windows.
        let module = unsafe { LoadLibraryExW(&HSTRING::from(library.as_os_str()), None, LOAD_WITH_ALTERED_SEARCH_PATH) }.map_err(|e| format!("the wake word library did not load: {e}"))?;
        macro_rules! symbol {
            ($name:literal, $t:ty) => {{
                let address = unsafe { GetProcAddress(module, PCSTR(concat!($name, "\0").as_ptr())) }.ok_or_else(|| format!("the wake word library has no {}", $name))?;
                // SAFETY: the signature is the one of `c-api.h` of the pinned release.
                unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, $t>(address) }
            }};
        }
        Ok(Api {
            create: symbol!("SherpaOnnxCreateKeywordSpotter", Create),
            destroy: symbol!("SherpaOnnxDestroyKeywordSpotter", Destroy),
            create_stream: symbol!("SherpaOnnxCreateKeywordStream", CreateStream),
            destroy_stream: symbol!("SherpaOnnxDestroyOnlineStream", Destroy),
            ready: symbol!("SherpaOnnxIsKeywordStreamReady", Ready),
            decode: symbol!("SherpaOnnxDecodeKeywordStream", Step),
            reset: symbol!("SherpaOnnxResetKeywordStream", Step),
            result: symbol!("SherpaOnnxGetKeywordResult", Result_),
            destroy_result: symbol!("SherpaOnnxDestroyKeywordResult", DestroyResult),
            accept: symbol!("SherpaOnnxOnlineStreamAcceptWaveform", Accept),
        })
    }

    /// A keyword spotter with its stream.
    pub struct Spotter {
        api: Api,
        spotter: *const c_void,
        stream: *const c_void,
    }

    impl Spotter {
        pub fn open(library: &Path, model: &Path, keywords: &str) -> Result<Spotter, String> {
            let api = load(library)?;
            let path = |name: &str| {
                let p = model.join(name);
                if !p.is_file() {
                    return Err(format!("the wake word file {name} is missing"));
                }
                CString::new(p.to_string_lossy().into_owned()).map_err(|e| e.to_string())
            };
            let [encoder, decoder, joiner, tokens] = [path(wake::MODEL_FILES[0])?, path(wake::MODEL_FILES[1])?, path(wake::MODEL_FILES[2])?, path(wake::MODEL_FILES[3])?];
            let words = CString::new(keywords).map_err(|e| e.to_string())?;
            let provider = c"cpu";
            let config = SpotterConfig {
                sample_rate: 16_000,
                feature_dim: 80,
                model: ModelConfig {
                    transducer: Transducer { encoder: encoder.as_ptr(), decoder: decoder.as_ptr(), joiner: joiner.as_ptr() },
                    paraformer: [null(); 2],
                    zipformer2_ctc: null(),
                    tokens: tokens.as_ptr(),
                    num_threads: 1,
                    provider: provider.as_ptr(),
                    debug: 0,
                    model_type: null(),
                    modeling_unit: null(),
                    bpe_vocab: null(),
                    tokens_buf: null(),
                    tokens_buf_size: 0,
                    nemo_ctc: null(),
                    t_one_ctc: null(),
                },
                max_active_paths: 4,
                num_trailing_blanks: 2,
                keywords_score: 1.0,
                keywords_threshold: 0.25,
                keywords_file: null(),
                keywords_buf: words.as_ptr(),
                keywords_buf_size: words.as_bytes().len() as i32,
            };
            // SAFETY: every pointer of the config lives until the call returns (the library copies what it keeps).
            let spotter = unsafe { (api.create)(&config) };
            if spotter.is_null() {
                return Err("the wake word model did not load".into());
            }
            let stream = unsafe { (api.create_stream)(spotter) };
            if stream.is_null() {
                unsafe { (api.destroy)(spotter) };
                return Err("the wake word stream did not start".into());
            }
            Ok(Spotter { api, spotter, stream })
        }

        /// Takes 16 kHz mono samples; true when the word was heard.
        pub fn feed(&mut self, pcm: &[i16]) -> bool {
            let samples: Vec<f32> = pcm.iter().map(|s| f32::from(*s) / 32768.0).collect();
            let mut heard = false;
            // SAFETY: `spotter` and `stream` are alive as long as `self`; the sample pointer is valid for its length.
            unsafe {
                (self.api.accept)(self.stream, 16_000, samples.as_ptr(), samples.len() as i32);
                while (self.api.ready)(self.spotter, self.stream) == 1 {
                    (self.api.decode)(self.spotter, self.stream);
                    let found = (self.api.result)(self.spotter, self.stream);
                    if found.is_null() {
                        continue;
                    }
                    let keyword = (*found).keyword;
                    if !keyword.is_null() && *keyword != 0 {
                        heard = true;
                        (self.api.reset)(self.spotter, self.stream);
                    }
                    (self.api.destroy_result)(found);
                }
            }
            heard
        }

        /// Forgets what was heard (the sound ended).
        pub fn reset(&mut self) {
            // SAFETY: as above.
            unsafe { (self.api.reset)(self.spotter, self.stream) }
        }
    }

    impl Drop for Spotter {
        fn drop(&mut self) {
            // SAFETY: both were created by this library and are destroyed once.
            unsafe {
                (self.api.destroy_stream)(self.stream);
                (self.api.destroy)(self.spotter);
            }
        }
    }

    enum Heard {
        Chunk(Vec<i16>, f32),
        Failed(String),
    }

    fn say(line: &str) -> bool {
        let mut out = std::io::stdout().lock();
        writeln!(out, "{line}").and_then(|()| out.flush()).is_ok()
    }

    /// The body of the worker; the return value is the exit code.
    pub fn run_worker(args: &WorkerArgs) -> i32 {
        // The agent holds the other end of standard input: when it closes, there is nobody to listen for.
        std::thread::spawn(|| {
            let mut buf = [0u8; 64];
            while std::io::stdin().read(&mut buf).is_ok_and(|n| n > 0) {}
            std::process::exit(0);
        });
        let mut spotter = match Spotter::open(&args.library, &args.model, &wake::keywords(args.sensitivity)) {
            Ok(s) => s,
            Err(e) => {
                say(&format!("error {e}"));
                return 2;
            }
        };
        let (tx, rx) = mpsc::channel::<Heard>();
        let (chunks, errors) = (tx.clone(), tx);
        let mic = crate::voice::mic::start(
            args.mic.clone(),
            false,
            move |fed| {
                let pcm = fed.pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
                let _ = chunks.send(Heard::Chunk(pcm, fed.decision.rms));
            },
            move |e| {
                let _ = errors.send(Heard::Failed(e));
            },
            |_| {},
        );
        let _mic = match mic {
            Ok(m) => m,
            Err(e) => {
                say(&format!("error {e}"));
                return 3;
            }
        };
        say("ready");
        let mut gate = Gate::default();
        for heard in rx {
            match heard {
                Heard::Chunk(pcm, rms) => match gate.feed(pcm, rms) {
                    Gated::Skip => {}
                    Gated::Close => spotter.reset(),
                    Gated::Feed(chunks) => {
                        // Every chunk goes in, even after a hit.
                        let hits = chunks.iter().filter(|c| spotter.feed(c)).count();
                        if hits > 0 && !say("wake") {
                            return 0;
                        }
                    }
                },
                Heard::Failed(e) => {
                    say(&format!("error {e}"));
                    return 4;
                }
            }
        }
        0
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The sizes of the C structs on x64; a field in the wrong place would crash the library.
        #[test]
        fn the_structs_have_the_layout_of_the_header() {
            assert_eq!(std::mem::size_of::<ModelConfig>(), 136);
            assert_eq!(std::mem::size_of::<SpotterConfig>(), 184);
            assert_eq!(std::mem::offset_of!(ModelConfig, tokens), 48);
            assert_eq!(std::mem::offset_of!(ModelConfig, nemo_ctc), 120);
            assert_eq!(std::mem::offset_of!(SpotterConfig, model), 8);
            assert_eq!(std::mem::offset_of!(SpotterConfig, keywords_buf), 168);
        }

        #[test]
        fn a_missing_library_is_an_error_not_a_crash() {
            let dir = tempfile::tempdir().unwrap();
            let e = Spotter::open(&dir.path().join("sherpa-onnx-c-api.dll"), dir.path(), "x\n").err().unwrap();
            assert!(e.contains("missing"), "{e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arguments_of_the_worker_come_back_as_they_were_sent() {
        let a = WorkerArgs { library: "C:\\a b\\x.dll".into(), model: "C:\\m".into(), sensitivity: Sensitivity::Low, mic: Some("Mic (USB)".into()) };
        let text: Vec<String> = a.to_args().iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(WorkerArgs::parse(&text), Some(a.clone()));
        assert_eq!(WorkerArgs::parse(&["--minimized".to_string()]), None);
        let without = WorkerArgs { mic: None, ..a };
        let text: Vec<String> = without.to_args().iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(WorkerArgs::parse(&text), Some(without));
    }
}
