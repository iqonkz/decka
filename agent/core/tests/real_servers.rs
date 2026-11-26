//! T28.1 / T28.2 on the real servers of the installed AI package (`%LOCALAPPDATA%\Decka\ai`). Ignored: it needs the
//! package and starts `llama-server` and `whisper-server` (children of the test, killed with it).
//!
//!   cargo test -p decka-core --test real_servers -- --ignored --nocapture --test-threads=1

use std::path::PathBuf;

use decka_core::ai::failure::{Reason, Server};
use decka_core::ai::gpu;
use decka_core::ai::service::{default_root, find_file};
use decka_core::ai::supervisor::{Supervisor, SupervisorConfig};

fn dir_starting(root: &std::path::Path, prefix: &str) -> Option<PathBuf> {
    std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(prefix)))
}

fn installed() -> SupervisorConfig {
    let root = default_root();
    let runtime = root.join("runtime");
    let llama = dir_starting(&runtime, "llama-runtime").expect("llama runtime installed");
    let (whisper, whisper_gpu) = match dir_starting(&runtime, "whisper-runtime-cuda") {
        Some(d) => (d, true),
        None => (dir_starting(&runtime, "whisper-runtime").expect("whisper runtime installed"), false),
    };
    SupervisorConfig {
        llama_exe: find_file(&llama, "llama-server.exe").expect("llama-server.exe"),
        llama_model: dir_starting(&root.join("models"), "Qwen3.5-").expect("a model"),
        mmproj: None,
        ngl: gpu::ALL_LAYERS,
        whisper_exe: find_file(&whisper, "whisper-server.exe").expect("whisper-server.exe"),
        whisper_model: root.join("models").join("ggml-large-v3-turbo-q5_0.bin"),
        keep_loaded: false,
        whisper_gpu,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the installed AI package"]
async fn a_damaged_model_is_named_and_the_recognition_still_starts() {
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.gguf");
    std::fs::write(&broken, vec![7u8; 4096]).unwrap();
    let sup = Supervisor::new(tokio::runtime::Handle::current()).unwrap();
    sup.configure(Some(SupervisorConfig { llama_model: broken, ..installed() }));
    let failure = sup.ensure_llm().await.expect_err("a broken file does not load");
    println!("model: reason={:?} exit={:?}\n  {}", failure.reason, failure.code, failure.detail);
    assert_eq!(failure.reason, Reason::ModelFile);
    sup.ensure_stt().await.expect("the recognition needs no model");
    assert!(sup.is_up(Server::Stt) && !sup.is_up(Server::Llm));
    sup.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the installed AI package"]
async fn the_model_loads_with_a_part_of_its_layers_and_with_none() {
    let base = installed();
    let count = gpu::gguf_layer_count(&base.llama_model).expect("block_count in the header");
    println!("{}: {count} layers", base.llama_model.file_name().unwrap().to_string_lossy());
    for ngl in [18, 0] {
        let sup = Supervisor::new(tokio::runtime::Handle::current()).unwrap();
        sup.configure(Some(SupervisorConfig { ngl, ..base.clone() }));
        let started = std::time::Instant::now();
        let (port, key) = sup.ensure_llm().await.unwrap_or_else(|f| panic!("-ngl {ngl}: {f:?}"));
        let ready = started.elapsed();
        let ctx = decka_core::voice::tools::Context::default();
        let asked = std::time::Instant::now();
        let answer = decka_core::voice::llm::ask_at(port, &key, "what time is it", &ctx, "en").await;
        println!("-ngl {ngl}: ready in {:.1} s, answered in {:.1} s: {:?}", ready.as_secs_f64(), asked.elapsed().as_secs_f64(), answer.map(|(c, _)| c.tool));
        sup.stop().await;
    }
}
