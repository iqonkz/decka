//! Downloader of the voice package (DATA_MODEL.md 12.1): resumable (`Range`), SHA-256 checked, restricted to a fixed host
//! list (S18), archives unpacked with the built-in `tar.exe`. Nothing here talks to the phone.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The only hosts a download (and every redirect) may go to. `*.name` allows every subdomain of `name`, not `name` itself.
/// Hugging Face serves files from changing hosts of its own domain (`us.aws.cdn.hf.co` since 2026-11); the SHA-256 of the
/// manifest decides whether the bytes are accepted, the host list only narrows where they may come from.
pub const ALLOWED_HOSTS: [&str; 7] = [
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "huggingface.co",
    "cdn-lfs.hf.co",
    "cas-bridge.xethub.hf.co",
    "*.hf.co",
];

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DownloadError {
    #[error("Download damaged — try again")]
    Damaged,
    #[error("Download host is not allowed")]
    HostNotAllowed,
    #[error("Download cancelled")]
    Cancelled,
    #[error("Download failed: {0}")]
    Network(String),
    #[error("Disk error: {0}")]
    Io(String),
    #[error("Could not unpack the archive: {0}")]
    Unpack(String),
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    /// `runtime` | `llm` | `stt`
    pub kind: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub license: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ManifestShape {
    List(Vec<ManifestEntry>),
    Wrapped { files: Vec<ManifestEntry> },
}

/// Parses `shared/ai/manifest.json`: a bare array or `{ "files": [...] }`. Every URL must point to an allowed host.
pub fn parse_manifest(text: &str) -> Result<Vec<ManifestEntry>, String> {
    let entries = match serde_json::from_str::<ManifestShape>(text).map_err(|e| e.to_string())? {
        ManifestShape::List(v) | ManifestShape::Wrapped { files: v } => v,
    };
    for e in &entries {
        if e.sha256.len() != 64 || !e.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{}: bad sha256", e.id));
        }
        if !host_allowed(&e.url, &ALLOWED_HOSTS, false) {
            return Err(format!("{}: host is not allowed", e.id));
        }
    }
    Ok(entries)
}

/// True when the URL is `https` (or `http` when `allow_http`) and its host is in `hosts`.
pub fn host_allowed<S: AsRef<str>>(url: &str, hosts: &[S], allow_http: bool) -> bool {
    let Ok(u) = url::Url::parse(url) else { return false };
    if u.scheme() != "https" && !(allow_http && u.scheme() == "http") {
        return false;
    }
    u.host_str().is_some_and(|h| {
        hosts.iter().any(|a| match a.as_ref().strip_prefix("*.") {
            // A real subdomain: a dot before the name, so `evilhf.co` and `hf.co` itself do not match `*.hf.co`.
            Some(suffix) => h.len() > suffix.len() + 1 && h.to_ascii_lowercase().ends_with(&format!(".{}", suffix.to_ascii_lowercase())),
            None => a.as_ref().eq_ignore_ascii_case(h),
        })
    })
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// The downloader; the host list is injectable so tests can use a local server.
#[derive(Clone)]
pub struct Downloader {
    hosts: Arc<Vec<String>>,
    allow_http: bool,
}

impl Default for Downloader {
    fn default() -> Self {
        Downloader { hosts: Arc::new(ALLOWED_HOSTS.iter().map(|h| h.to_string()).collect()), allow_http: false }
    }
}

impl Downloader {
    /// Test constructor: plain `http` and an arbitrary host list.
    pub fn with_hosts(hosts: &[&str]) -> Self {
        Downloader { hosts: Arc::new(hosts.iter().map(|h| h.to_string()).collect()), allow_http: true }
    }

    fn client(&self) -> Result<reqwest::Client, DownloadError> {
        let hosts = self.hosts.clone();
        let allow_http = self.allow_http;
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 8 {
                attempt.error("too many redirects")
            } else if host_allowed(attempt.url().as_str(), &hosts, allow_http) {
                attempt.follow()
            } else {
                attempt.error("redirect to a host that is not allowed")
            }
        });
        reqwest::Client::builder()
            .redirect(policy)
            .user_agent(format!("decka-agent/{}", crate::VERSION))
            .connect_timeout(std::time::Duration::from_secs(20))
            .read_timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| DownloadError::Network(e.to_string()))
    }

    /// Downloads `entry` to `dest` (a finished file with the right hash is kept as is). Progress is reported in bytes.
    /// On a hash mismatch the partial file is deleted; on cancel or a network error it stays for the next resume.
    pub async fn fetch(
        &self,
        entry: &ManifestEntry,
        dest: &Path,
        cancel: &AtomicBool,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> Result<(), DownloadError> {
        if !host_allowed(&entry.url, &self.hosts, self.allow_http) {
            return Err(DownloadError::HostNotAllowed);
        }
        if dest.exists() && sha256_file(dest)? == entry.sha256.to_ascii_lowercase() {
            progress(entry.size);
            return Ok(());
        }
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let part = part_path(dest);
        let mut offset = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if offset > entry.size {
            let _ = std::fs::remove_file(&part);
            offset = 0;
        }

        if offset < entry.size {
            let client = self.client()?;
            let mut req = client.get(&entry.url);
            if offset > 0 {
                req = req.header(reqwest::header::RANGE, format!("bytes={offset}-"));
            }
            let mut resp = req.send().await.map_err(|e| DownloadError::Network(e.to_string()))?;
            let status = resp.status().as_u16();
            if status == 416 {
                // The partial file is not usable: start over next time.
                let _ = std::fs::remove_file(&part);
                return Err(DownloadError::Damaged);
            }
            if !(status == 200 || status == 206) {
                return Err(DownloadError::Network(format!("HTTP {status}")));
            }
            // 200 to a Range request means the server ignored it: rewrite from the start.
            let resumed = status == 206 && offset > 0;
            let mut file = if resumed {
                std::fs::OpenOptions::new().append(true).open(&part)?
            } else {
                offset = 0;
                std::fs::File::create(&part)?
            };
            let mut done = offset;
            progress(done);
            while let Some(chunk) = resp.chunk().await.map_err(|e| DownloadError::Network(e.to_string()))? {
                if cancel.load(Ordering::Relaxed) {
                    return Err(DownloadError::Cancelled);
                }
                done += chunk.len() as u64;
                // More than the manifest says is a wrong file: it must not fill the disk before the hash is checked.
                if done > entry.size {
                    drop(file);
                    let _ = std::fs::remove_file(&part);
                    return Err(DownloadError::Damaged);
                }
                file.write_all(&chunk)?;
                progress(done);
            }
            file.flush()?;
        }

        let ok = std::fs::metadata(&part).map(|m| m.len() == entry.size).unwrap_or(false)
            && sha256_file(&part)? == entry.sha256.to_ascii_lowercase();
        if !ok {
            let _ = std::fs::remove_file(&part);
            return Err(DownloadError::Damaged);
        }
        std::fs::rename(&part, dest)?;
        Ok(())
    }
}

/// `tar.exe -xf <zip> -C <tmp>`, then the temporary folder is renamed to `dir` (an existing `dir` is replaced).
pub fn extract_archive(archive: &Path, dir: &Path) -> Result<(), DownloadError> {
    let mut tmp_name = dir.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = dir.with_file_name(tmp_name);
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;

    let mut cmd = std::process::Command::new("tar.exe");
    cmd.arg("-xf").arg(archive).arg("-C").arg(&tmp);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| DownloadError::Unpack(e.to_string()))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(DownloadError::Unpack(String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("tar failed").to_string()));
    }
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::rename(&tmp, dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn hex(data: &[u8]) -> String {
        Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn entry(url: &str, data: &[u8]) -> ManifestEntry {
        ManifestEntry { id: "t".into(), kind: "llm".into(), url: url.into(), sha256: hex(data), size: data.len() as u64, license: "MIT".into() }
    }

    /// A tiny HTTP/1.1 server: serves `body`, honours `Range: bytes=N-`, can ignore it, and records the Range values.
    async fn serve(body: Vec<u8>, honour_range: bool) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://127.0.0.1:{}/file.bin", listener.local_addr().unwrap().port());
        let ranges = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = ranges.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                let (body, seen) = (body.clone(), seen.clone());
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let mut len = 0;
                    while !buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
                        let n = sock.read(&mut buf[len..]).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        len += n;
                    }
                    let head = String::from_utf8_lossy(&buf[..len]).to_string();
                    let start = head
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("range: bytes=").map(|r| r.trim_end_matches('-').parse::<usize>().unwrap_or(0)));
                    if let Some(s) = start {
                        seen.lock().unwrap().push(format!("bytes={s}-"));
                    }
                    let reply = match start {
                        Some(s) if honour_range => {
                            let part = &body[s..];
                            let h = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                                part.len(), s, body.len() - 1, body.len()
                            );
                            [h.into_bytes(), part.to_vec()].concat()
                        }
                        _ => [format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes(), body.clone()].concat(),
                    };
                    let _ = sock.write_all(&reply).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        (url, ranges)
    }

    #[test]
    fn host_list_is_enforced() {
        assert!(host_allowed("https://huggingface.co/a/b.gguf", &ALLOWED_HOSTS, false));
        assert!(host_allowed("https://github.com/x", &ALLOWED_HOSTS, false));
        assert!(!host_allowed("https://evil.example/x", &ALLOWED_HOSTS, false));
        assert!(!host_allowed("https://github.com.evil.example/x", &ALLOWED_HOSTS, false));
        // The CDN host of Hugging Face that broke the first real download, and the look-alikes that must stay out.
        assert!(host_allowed("https://us.aws.cdn.hf.co/xet-bridge-us/abc?X=1", &ALLOWED_HOSTS, false));
        assert!(!host_allowed("https://evilhf.co/x", &ALLOWED_HOSTS, false));
        assert!(!host_allowed("https://hf.co.evil.example/x", &ALLOWED_HOSTS, false));
        assert!(!host_allowed("http://us.aws.cdn.hf.co/x", &ALLOWED_HOSTS, false));
        assert!(!host_allowed("http://github.com/x", &ALLOWED_HOSTS, false), "plain http is refused");
        assert!(!host_allowed("not a url", &ALLOWED_HOSTS, false));
    }

    #[test]
    fn manifest_parses_both_shapes_and_rejects_bad_entries() {
        let sha = "a".repeat(64);
        let one = |url: &str, sha: &str| format!(r#"{{"id":"stt","kind":"stt","url":"{url}","sha256":"{sha}","size":5,"license":"MIT"}}"#);
        let list = format!("[{}]", one("https://huggingface.co/x.bin", &sha));
        assert_eq!(parse_manifest(&list).unwrap()[0].id, "stt");
        let wrapped = format!(r#"{{"files":[{}]}}"#, one("https://github.com/x.zip", &sha));
        assert_eq!(parse_manifest(&wrapped).unwrap().len(), 1);
        assert!(parse_manifest(&format!("[{}]", one("https://evil.example/x", &sha))).is_err());
        assert!(parse_manifest(&format!("[{}]", one("https://github.com/x", "zz"))).is_err());
    }

    #[tokio::test]
    async fn host_outside_the_list_is_refused_before_any_request() {
        let dir = tempfile::tempdir().unwrap();
        let dl = Downloader::default();
        let e = entry("https://evil.example/model.gguf", b"data");
        let err = dl.fetch(&e, &dir.path().join("m.gguf"), &AtomicBool::new(false), &mut |_| {}).await.unwrap_err();
        assert_eq!(err, DownloadError::HostNotAllowed);
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none(), "nothing was written");
    }

    #[tokio::test]
    async fn hash_mismatch_is_an_error_and_the_partial_file_is_deleted() {
        let (url, _) = serve(b"hello world, real content".to_vec(), true).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        let mut e = entry(&url, b"hello world, real content");
        e.sha256 = hex(b"something else");
        let err = Downloader::with_hosts(&["127.0.0.1"]).fetch(&e, &dest, &AtomicBool::new(false), &mut |_| {}).await.unwrap_err();
        assert_eq!(err, DownloadError::Damaged);
        assert_eq!(err.to_string(), "Download damaged — try again");
        assert!(!dest.exists() && !part_path(&dest).exists());
    }

    #[tokio::test]
    async fn a_file_longer_than_the_manifest_says_is_stopped_and_deleted() {
        let body = vec![7u8; 200_000];
        let (url, _) = serve(body.clone(), true).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        let e = entry(&url, &body[..100]);
        let mut most = 0;
        let err = Downloader::with_hosts(&["127.0.0.1"]).fetch(&e, &dest, &AtomicBool::new(false), &mut |done| most = most.max(done)).await.unwrap_err();
        assert_eq!(err, DownloadError::Damaged);
        assert!(most <= e.size && !dest.exists() && !part_path(&dest).exists(), "{most}");
    }

    #[tokio::test]
    async fn download_resumes_from_the_partial_file_with_a_range_request() {
        let body: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let (url, ranges) = serve(body.clone(), true).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        std::fs::write(part_path(&dest), &body[..4000]).unwrap();
        let mut last = 0;
        Downloader::with_hosts(&["127.0.0.1"]).fetch(&entry(&url, &body), &dest, &AtomicBool::new(false), &mut |d| last = d).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert!(!part_path(&dest).exists());
        assert_eq!(ranges.lock().unwrap().as_slice(), ["bytes=4000-"]);
        assert_eq!(last, 10_000);
    }

    #[tokio::test]
    async fn a_server_that_ignores_range_restarts_the_file() {
        let body: Vec<u8> = (0..5_000u32).map(|i| (i % 199) as u8).collect();
        let (url, _) = serve(body.clone(), false).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        std::fs::write(part_path(&dest), &body[..1000]).unwrap();
        Downloader::with_hosts(&["127.0.0.1"]).fetch(&entry(&url, &body), &dest, &AtomicBool::new(false), &mut |_| {}).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
    }

    #[tokio::test]
    async fn cancel_keeps_the_partial_file_and_a_finished_file_is_not_downloaded_again() {
        let body = vec![7u8; 3000];
        let (url, ranges) = serve(body.clone(), true).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        let dl = Downloader::with_hosts(&["127.0.0.1"]);
        let err = dl.fetch(&entry(&url, &body), &dest, &AtomicBool::new(true), &mut |_| {}).await.unwrap_err();
        assert_eq!(err, DownloadError::Cancelled);
        assert!(!dest.exists());
        dl.fetch(&entry(&url, &body), &dest, &AtomicBool::new(false), &mut |_| {}).await.unwrap();
        let requests = AtomicUsize::new(ranges.lock().unwrap().len());
        dl.fetch(&entry("http://127.0.0.1:1/never", &body), &dest, &AtomicBool::new(false), &mut |_| {}).await.unwrap();
        assert_eq!(requests.load(Ordering::Relaxed), ranges.lock().unwrap().len());
    }

    #[cfg(windows)]
    #[test]
    fn tar_unpacks_a_zip_into_a_fresh_folder() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("llama-server.exe"), b"x").unwrap();
        let zip = dir.path().join("a.zip");
        let status = std::process::Command::new("tar.exe").arg("-a").arg("-cf").arg(&zip).arg("-C").arg(&src).arg("llama-server.exe").status().unwrap();
        assert!(status.success());
        let out = dir.path().join("runtime").join("llama-b1");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        extract_archive(&zip, &out).unwrap();
        assert!(out.join("llama-server.exe").exists());
        assert!(!out.with_file_name("llama-b1.tmp").exists());
        assert!(extract_archive(&dir.path().join("missing.zip"), &out).is_err());
    }
}
