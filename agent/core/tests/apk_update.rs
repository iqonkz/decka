//! T20.4: the phone app update from the PC. The APK is fetched from a local stand-in for the release (no network), its SHA-256
//! is checked against `SHA256SUMS.txt`, the signature of that file against the built-in updater key (fixtures signed with it),
//! and the pieces handed to a phone reassemble to the same file.

use std::collections::HashMap;
use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::agent::ApkGetError;
use decka_core::apk_update::{self, ApkError, DownloadGate, CHUNK, UPDATE_PUBKEY};
use decka_core::proto::ServerMsg;
use decka_core::{Agent, AgentOptions};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SUMS: &str = include_str!("../../../shared/fixtures/apk-update/SHA256SUMS.txt");
const SIG: &str = include_str!("../../../shared/fixtures/apk-update/SHA256SUMS.txt.sig");
const FOREIGN_SIG: &str = include_str!("../../../shared/fixtures/apk-update/SHA256SUMS.foreign.sig");
const APK: &[u8] = include_bytes!("../../../shared/fixtures/apk-update/Decka-9.9.0.apk");
const TAURI_CONF: &str = include_str!("../../src-tauri/tauri.conf.json");

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Serves `files` (path -> bytes) over HTTP on a free port; returns the base URL.
async fn fake_release(files: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let files: Arc<HashMap<String, Vec<u8>>> = Arc::new(files.into_iter().map(|(p, b)| (p.to_string(), b)).collect());
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let files = files.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, body) = match files.get(&path) {
                    Some(b) => (200, b.clone()),
                    None => (404, Vec::new()),
                };
                let head = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
    base
}

fn release_files(sums: &str, sig: &str, apk: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    vec![("/SHA256SUMS.txt", sums.as_bytes().to_vec()), ("/SHA256SUMS.txt.sig", sig.as_bytes().to_vec()), ("/Decka-9.9.0.apk", apk.to_vec())]
}

#[test]
fn the_built_in_key_is_the_one_of_the_updater_config() {
    let conf: serde_json::Value = serde_json::from_str(TAURI_CONF).unwrap();
    assert_eq!(conf["plugins"]["updater"]["pubkey"].as_str(), Some(UPDATE_PUBKEY));
}

#[test]
fn the_signature_of_the_sums_file_is_checked() {
    assert_eq!(apk_update::verify_sums(SUMS, SIG), Ok(()));
    let tampered = SUMS.replacen('0', "1", 1);
    assert_eq!(apk_update::verify_sums(&tampered, SIG), Err(ApkError::BadSignature), "a changed file");
    assert_eq!(apk_update::verify_sums(SUMS, FOREIGN_SIG), Err(ApkError::BadSignature), "a signature of another key");
    assert_eq!(apk_update::verify_sums(SUMS, "not a signature"), Err(ApkError::BadSignature));
    assert_eq!(apk_update::verify_sums(SUMS, ""), Err(ApkError::BadSignature));
}

#[test]
fn the_apk_line_is_read_from_the_sums() {
    assert_eq!(apk_update::parse_sums(SUMS, "Decka-9.9.0.apk"), Some(hex(APK)));
    assert_eq!(apk_update::parse_sums(SUMS, "Decka-9.9.1.apk"), None);
    assert_eq!(apk_update::parse_sums("zz  Decka-9.9.0.apk\n", "Decka-9.9.0.apk"), None, "a short or non-hex hash");
}

#[tokio::test]
async fn a_good_release_is_stored_and_its_pieces_rebuild_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let base = fake_release(release_files(SUMS, SIG, APK)).await;
    let update = apk_update::prepare(&base, "9.9.0", dir.path(), UPDATE_PUBKEY).await.expect("signed and matching");
    assert_eq!((update.size, update.sha256.as_str()), (APK.len() as u64, hex(APK).as_str()));
    assert!(dir.path().join("Decka-9.9.0.apk").exists());
    assert!(!dir.path().join("Decka-9.9.0.apk.part").exists());

    // The pieces a phone asks for, one by one, give the same bytes and the same SHA-256.
    let mut rebuilt = Vec::new();
    let mut offset = 0u64;
    while offset < update.size {
        let piece = apk_update::read_chunk(&update, offset).expect("inside the file");
        assert!(piece.len() <= CHUNK);
        rebuilt.extend_from_slice(&piece);
        offset += CHUNK as u64;
    }
    assert_eq!(rebuilt, APK);
    assert_eq!(hex(&rebuilt), update.sha256);
    assert!(apk_update::read_chunk(&update, 1).is_none(), "not a multiple of 3000");
    assert!(apk_update::read_chunk(&update, update.size).is_none(), "past the end");
}

#[tokio::test]
async fn an_apk_that_does_not_match_is_refused_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut changed = APK.to_vec();
    changed[100] ^= 0xFF;
    let base = fake_release(release_files(SUMS, SIG, &changed)).await;
    assert_eq!(apk_update::prepare(&base, "9.9.0", dir.path(), UPDATE_PUBKEY).await, Err(ApkError::HashMismatch));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "no APK and no partial file");
}

#[tokio::test]
async fn a_release_with_a_foreign_or_missing_signature_is_refused_before_any_download() {
    let dir = tempfile::tempdir().unwrap();
    let foreign = fake_release(release_files(SUMS, FOREIGN_SIG, APK)).await;
    assert_eq!(apk_update::prepare(&foreign, "9.9.0", dir.path(), UPDATE_PUBKEY).await, Err(ApkError::BadSignature));
    let unsigned = fake_release(vec![("/SHA256SUMS.txt", SUMS.as_bytes().to_vec()), ("/Decka-9.9.0.apk", APK.to_vec())]).await;
    assert_eq!(apk_update::prepare(&unsigned, "9.9.0", dir.path(), UPDATE_PUBKEY).await, Err(ApkError::Network));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn one_download_per_device_at_a_time() {
    let mut gate = DownloadGate::default();
    assert!(gate.allow("dev1", "usb", 1_000));
    assert!(gate.allow("dev1", "usb", 2_000), "the same connection goes on");
    assert!(!gate.allow("dev1", "lan", 3_000), "a second connection of the same device waits");
    assert!(gate.allow("dev2", "lan", 3_000), "another device is independent");
    assert!(gate.allow("dev1", "lan", 2_000 + 15_000), "the first one has been silent for 15 s");
    gate.release("dev2", "lan");
    assert!(gate.allow("dev2", "usb", 3_001), "a finished connection frees its place");
}

#[tokio::test]
async fn the_agent_offers_only_a_verified_apk_and_serves_it_in_pieces() {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()), config_dir: Some(dir.path().to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None }).await.unwrap();
    assert!(matches!(agent.apk_chunk("dev", "c1", 0), Err(ApkGetError::NoUpdate)));
    assert_eq!(agent.store().get("update"), serde_json::Value::Null);

    let foreign = fake_release(release_files(SUMS, FOREIGN_SIG, APK)).await;
    assert!(!agent.prepare_app_update(&foreign, "9.9.0", UPDATE_PUBKEY).await);
    assert_eq!(agent.store().get("update"), serde_json::Value::Null, "nothing is offered after a failed check");

    let good = fake_release(release_files(SUMS, SIG, APK)).await;
    assert!(agent.prepare_app_update(&good, "9.9.0", UPDATE_PUBKEY).await);
    let section = agent.store().get("update");
    assert_eq!(section["app"]["version"], "9.9.0");
    assert_eq!(section["app"]["size"], APK.len() as u64);
    assert_eq!(section["app"]["sha256"], hex(APK));

    let Ok(ServerMsg::ApkChunk { offset, data, total, sha256 }) = agent.apk_chunk("dev", "c1", 3000) else { panic!("a chunk") };
    assert_eq!((offset, total, sha256.as_str()), (3000, APK.len() as u64, hex(APK).as_str()));
    use base64::Engine as _;
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(data).unwrap(), &APK[3000..6000]);
    assert!(matches!(agent.apk_chunk("dev", "c1", 100), Err(ApkGetError::BadOffset)));
    assert!(matches!(agent.apk_chunk("dev", "c1", 9000), Err(ApkGetError::BadOffset)));
    assert!(matches!(agent.apk_chunk("dev", "c2", 0), Err(ApkGetError::Busy)), "a second connection of the same device");
    agent.apk_release("dev", "c1");
    assert!(agent.apk_chunk("dev", "c2", 0).is_ok());
}
