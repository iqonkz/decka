//! Phone app update from the PC (T20.4). The agent fetches the APK of a newer release, checks its SHA-256 against
//! `SHA256SUMS.txt` and the minisign signature of that file (the updater key, public part built in), keeps the APK in
//! `%LOCALAPPDATA%\Decka\updates\` and hands it to paired phones in chunks inside the encrypted channel.
//! The signature check is `minisign_verify` (the crate `tauri-plugin-updater` uses); no cryptography is written here.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};

/// Bytes per `apk_chunk` (base64 of it fits a protocol frame).
pub const CHUNK: usize = 3000;
/// The public key of the updater (the same text as `plugins.updater.pubkey` in tauri.conf.json; a test keeps them equal).
pub const UPDATE_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc4NEZENkQ1NUUxMEYwOTgKUldTWThCQmUxZFpQZUdGS2RXdG9PZStUSXNwNlpZVGFrM0NGaHNDejVxYUFKMktZd0FFNzFWK1cK";
/// A download longer than this is refused (the real APK is about 6 MB).
const MAX_APK_BYTES: u64 = 150 * 1024 * 1024;
const MAX_SUMS_BYTES: usize = 16 * 1024;
const TIMEOUT: Duration = Duration::from_secs(300);
/// A device that asked for a chunk this recently is "downloading"; another connection of the same device waits.
const BUSY_MS: u64 = 15_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApkError {
    /// The signature of `SHA256SUMS.txt` is missing, malformed, from another key or does not match the text.
    BadSignature,
    /// `SHA256SUMS.txt` has no line for the APK.
    NoEntry,
    /// The APK does not match its line in `SHA256SUMS.txt`.
    HashMismatch,
    TooBig,
    Network,
    Io,
}

/// An APK that passed both checks and is ready to be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUpdate {
    pub version: String,
    pub size: u64,
    /// Lower-case hex.
    pub sha256: String,
    pub path: PathBuf,
}

fn decode_text(b64: &str) -> Option<String> {
    String::from_utf8(STANDARD.decode(b64.trim()).ok()?).ok()
}

/// Checks the signature of `sums` (`SHA256SUMS.txt`) against `pubkey` (base64 of the minisign key file, as in
/// tauri.conf.json); `signature` is the text of the `.sig` file (base64 of the minisign signature file).
pub fn verify_sums_with(pubkey: &str, sums: &str, signature: &str) -> Result<(), ApkError> {
    let key = decode_text(pubkey).and_then(|t| PublicKey::decode(&t).ok()).ok_or(ApkError::BadSignature)?;
    let sig = decode_text(signature).and_then(|t| Signature::decode(&t).ok()).ok_or(ApkError::BadSignature)?;
    key.verify(sums.as_bytes(), &sig, true).map_err(|_| ApkError::BadSignature)
}

/// `verify_sums_with` for the key built into this agent.
pub fn verify_sums(sums: &str, signature: &str) -> Result<(), ApkError> {
    verify_sums_with(UPDATE_PUBKEY, sums, signature)
}

/// The SHA-256 of `file_name` from `SHA256SUMS.txt` (`<64 hex>  <name>` per line), lower-case; `None` when there is no such line.
pub fn parse_sums(sums: &str, file_name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once("  ")?;
        (name.trim() == file_name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

pub fn apk_name(version: &str) -> String {
    format!("Decka-{version}.apk")
}

/// SHA-256 of a file, lower-case hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn client() -> Result<reqwest::Client, ApkError> {
    reqwest::Client::builder().timeout(TIMEOUT).user_agent(format!("decka-agent/{}", crate::VERSION)).build().map_err(|_| ApkError::Network)
}

async fn fetch_small(client: &reqwest::Client, url: &str) -> Result<String, ApkError> {
    let response = client.get(url).send().await.map_err(|_| ApkError::Network)?;
    if !response.status().is_success() {
        return Err(ApkError::Network);
    }
    let bytes = response.bytes().await.map_err(|_| ApkError::Network)?;
    if bytes.len() > MAX_SUMS_BYTES {
        return Err(ApkError::TooBig);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| ApkError::Network)
}

/// Fetches `SHA256SUMS.txt` and its signature from `base` (the release's download folder), checks them, then fetches the APK,
/// checks its SHA-256 and stores it in `dir` as `Decka-<version>.apk` (atomic: the file appears only when it passed).
/// Older APKs in `dir` are removed. An APK already there that matches is not downloaded again.
pub async fn prepare(base: &str, version: &str, dir: &Path, pubkey: &str) -> Result<AppUpdate, ApkError> {
    let client = client()?;
    let sums = fetch_small(&client, &format!("{base}/SHA256SUMS.txt")).await?;
    let signature = fetch_small(&client, &format!("{base}/SHA256SUMS.txt.sig")).await?;
    verify_sums_with(pubkey, &sums, &signature)?;
    let name = apk_name(version);
    let expected = parse_sums(&sums, &name).ok_or(ApkError::NoEntry)?;
    std::fs::create_dir_all(dir).map_err(|_| ApkError::Io)?;
    let target = dir.join(&name);
    if sha256_file(&target).map(|h| h == expected).unwrap_or(false) {
        return finish(version, target, expected, dir);
    }
    let part = dir.join(format!("{name}.part"));
    let mut response = client.get(format!("{base}/{name}")).send().await.map_err(|_| ApkError::Network)?;
    if !response.status().is_success() {
        return Err(ApkError::Network);
    }
    if response.content_length().is_some_and(|n| n > MAX_APK_BYTES) {
        return Err(ApkError::TooBig);
    }
    let result = async {
        let mut file = std::fs::File::create(&part).map_err(|_| ApkError::Io)?;
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(|_| ApkError::Network)? {
            size += chunk.len() as u64;
            if size > MAX_APK_BYTES {
                return Err(ApkError::TooBig);
            }
            hasher.update(&chunk);
            std::io::Write::write_all(&mut file, &chunk).map_err(|_| ApkError::Io)?;
        }
        let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != expected {
            return Err(ApkError::HashMismatch);
        }
        Ok(())
    }
    .await;
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &target).map_err(|_| ApkError::Io)?;
    finish(version, target, expected, dir)
}

fn finish(version: &str, target: PathBuf, sha256: String, dir: &Path) -> Result<AppUpdate, ApkError> {
    let size = std::fs::metadata(&target).map_err(|_| ApkError::Io)?.len();
    // Only the file of the version in hand stays.
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            if file.starts_with("Decka-") && file.contains(".apk") && file != apk_name(version) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(AppUpdate { version: version.to_string(), size, sha256, path: target })
}

/// The chunk at `offset` (a multiple of `CHUNK`, inside the file); `None` otherwise.
pub fn read_chunk(update: &AppUpdate, offset: u64) -> Option<Vec<u8>> {
    if !offset.is_multiple_of(CHUNK as u64) || offset >= update.size {
        return None;
    }
    let mut file = std::fs::File::open(&update.path).ok()?;
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; CHUNK.min((update.size - offset) as usize)];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// One download at a time per device: a second connection of the same device is refused while the first one is active.
#[derive(Default)]
pub struct DownloadGate {
    /// device id -> (connection, ms of its last chunk request)
    active: HashMap<String, (String, u64)>,
}

impl DownloadGate {
    /// Whether `conn` of `device` may be served now (`now` in ms); remembers it as the active connection when it may.
    pub fn allow(&mut self, device: &str, conn: &str, now: u64) -> bool {
        if let Some((other, last)) = self.active.get(device) {
            if other != conn && now.saturating_sub(*last) < BUSY_MS {
                return false;
            }
        }
        self.active.insert(device.to_string(), (conn.to_string(), now));
        true
    }

    /// A finished or dropped connection frees its place.
    pub fn release(&mut self, device: &str, conn: &str) {
        if self.active.get(device).is_some_and(|(c, _)| c == conn) {
            self.active.remove(device);
        }
    }
}

/// The base of a release's download folder for `version` (the only place the agent downloads an APK from).
pub fn release_base(version: &str) -> String {
    format!("{}download/v{version}", crate::updates::RELEASES_PREFIX)
}
