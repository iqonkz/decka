//! Pairing codes, device records and tokens (ARCHITECTURE.md 6.2-6.3).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::config::{random_bytes, save_json_atomic};
use crate::model::{DeviceRecord, DevicesFile};

pub const PAIR_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_TTL: Duration = Duration::from_secs(120);
const FIXED_CODE_TTL: Duration = Duration::from_secs(3600);
const MAX_FAILED_ATTEMPTS: u32 = 5;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Constant-time equality: XOR-accumulates over the whole length.
pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

struct Active {
    code: String,
    /// `S_qr` (PROTOCOL.md 10): lives and dies with the code and travels only in the QR.
    s_qr: [u8; 32],
    expires: Instant,
    failures: u32,
}

/// The secrets a v2 pairing handshake may be answered with right now.
pub struct PairCandidates {
    /// The active code and its `S_qr`.
    pub active: Option<(String, [u8; 32])>,
    /// The fixed code of headless runs and tests.
    pub fixed: Option<String>,
}

/// At most one active pairing code; plus an optional fixed code for headless/tests.
pub struct Pairing {
    active: Mutex<Option<Active>>,
    fixed: Option<(String, Instant)>,
}

impl Pairing {
    pub fn new(fixed_code: Option<String>) -> Pairing {
        Pairing { active: Mutex::new(None), fixed: fixed_code.map(|c| (c, Instant::now() + FIXED_CODE_TTL)) }
    }

    /// New 8-character code valid for 120 s; replaces the previous one. Returns (code, expiresAt ms).
    pub fn start(&self) -> (String, u64) {
        let (code, _, expires) = self.start_with_secret();
        (code, expires)
    }

    /// Like `start`, plus the new `S_qr` for the QR fragment.
    pub fn start_with_secret(&self) -> (String, [u8; 32], u64) {
        let (code, _) = self.start_at(Instant::now());
        let s_qr = self.active.lock().unwrap().as_ref().map(|a| a.s_qr).unwrap_or([0; 32]);
        (code, s_qr, now_ms() + CODE_TTL.as_millis() as u64)
    }

    fn start_at(&self, now: Instant) -> (String, Instant) {
        // 32 symbols: masking a random byte with 31 is unbiased.
        let code: String = random_bytes(8).iter().map(|b| PAIR_ALPHABET[(b & 31) as usize] as char).collect();
        let expires = now + CODE_TTL;
        *self.active.lock().unwrap() = Some(Active { code: code.clone(), s_qr: crate::secure::random::<32>(), expires, failures: 0 });
        (code, expires)
    }

    pub fn cancel(&self) {
        *self.active.lock().unwrap() = None;
    }

    /// What a v2 handshake can be checked against now (an expired code is dropped).
    pub fn candidates(&self) -> PairCandidates {
        let now = Instant::now();
        let mut guard = self.active.lock().unwrap();
        if guard.as_ref().is_some_and(|a| now >= a.expires) {
            *guard = None;
        }
        PairCandidates {
            active: guard.as_ref().map(|a| (a.code.clone(), a.s_qr)),
            fixed: self.fixed.as_ref().filter(|(_, expires)| now < *expires).map(|(c, _)| c.clone()),
        }
    }

    /// A failed v2 pairing proof: counts against the active code, which burns after 5.
    pub fn record_failure(&self) {
        let mut guard = self.active.lock().unwrap();
        if let Some(active) = guard.as_mut() {
            active.failures += 1;
            if active.failures >= MAX_FAILED_ATTEMPTS {
                *guard = None;
            }
        }
    }

    /// The active code was used: it works once.
    pub fn burn(&self) {
        *self.active.lock().unwrap() = None;
    }

    pub fn try_consume(&self, code: &str) -> bool {
        self.try_consume_at(code, Instant::now())
    }

    fn try_consume_at(&self, code: &str, now: Instant) -> bool {
        // Both candidates are always compared so timing does not reveal which one exists.
        let fixed_ok = match &self.fixed {
            Some((fixed, expires)) => ct_eq(fixed.as_bytes(), code.as_bytes()) && now < *expires,
            None => false,
        };
        let mut guard = self.active.lock().unwrap();
        let Some(active) = guard.as_mut() else { return fixed_ok };
        if now >= active.expires {
            *guard = None;
            return fixed_ok;
        }
        if ct_eq(active.code.as_bytes(), code.as_bytes()) {
            *guard = None;
            return true;
        }
        if fixed_ok {
            return true; // the fixed code is not subject to the failure counter
        }
        active.failures += 1;
        if active.failures >= MAX_FAILED_ATTEMPTS {
            *guard = None;
        }
        false
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex_32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// A version as the apps write it: `MAJOR.MINOR.PATCH` with an optional `-pre` / `+build` tail, at most 32 characters.
pub fn is_app_version(v: &str) -> bool {
    let core_end = v.find(['-', '+']).unwrap_or(v.len());
    let (core, tail) = v.split_at(core_end);
    let parts: Vec<&str> = core.split('.').collect();
    v.len() <= 32
        && parts.len() == 3
        && parts.iter().all(|p| !p.is_empty() && p.len() <= 6 && p.bytes().all(|b| b.is_ascii_digit()))
        && tail.bytes().all(|b| b.is_ascii_alphanumeric() || b"-+.".contains(&b))
}

/// Paired devices. Only `K_dev` (the SHA-256 of the token), protected by DPAPI, is stored; devices.json is saved on every change.
pub struct Devices {
    path: PathBuf,
    file: Mutex<DevicesFile>,
}

/// `K_dev` as stored: DPAPI-protected hex. Without DPAPI (not Windows) the plain hash stays in the legacy field.
fn store_key(k_dev: &[u8; 32]) -> (Option<String>, Option<String>) {
    match crate::dpapi::protect(&to_hex(k_dev)) {
        Ok(blob) => (Some(blob), None),
        Err(e) => {
            tracing::warn!("cannot protect the device key: {e}");
            (None, Some(to_hex(k_dev)))
        }
    }
}

fn key_of_record(d: &DeviceRecord) -> Option<[u8; 32]> {
    if let Some(blob) = &d.key_dpapi {
        return crate::dpapi::unprotect(blob).ok().and_then(|hex| from_hex_32(&hex));
    }
    d.token_hash.as_deref().and_then(from_hex_32)
}

impl Devices {
    /// Old records (SHA-256 hash in `tokenHash`) are moved to `keyDpapi` here; the hash is the very `K_dev`, so phones stay paired.
    pub fn new(path: PathBuf, mut file: DevicesFile) -> Devices {
        let mut migrated = false;
        for d in file.devices.iter_mut().filter(|d| d.key_dpapi.is_none()) {
            let Some(k) = d.token_hash.as_deref().and_then(from_hex_32) else { continue };
            if let (Some(blob), _) = store_key(&k) {
                d.key_dpapi = Some(blob);
                d.token_hash = None;
                migrated = true;
            }
        }
        let devices = Devices { path, file: Mutex::new(file) };
        if migrated {
            let file = devices.file.lock().unwrap();
            devices.save(&file);
            tracing::info!("devices.json: token hashes moved to DPAPI-protected keys");
        }
        devices
    }

    /// `K_dev` of a paired device (the secret of the v2 handshake).
    pub fn key(&self, id: &str) -> Option<[u8; 32]> {
        self.file.lock().unwrap().devices.iter().find(|d| d.id == id).and_then(key_of_record)
    }

    fn save(&self, file: &DevicesFile) {
        if let Err(e) = save_json_atomic(&self.path, file) {
            tracing::warn!("cannot save devices.json: {e}");
        }
    }

    /// Returns the new record and the plain token (shown to the device once, never stored).
    pub fn create(&self, name: &str) -> (DeviceRecord, String) {
        let token = URL_SAFE_NO_PAD.encode(random_bytes(32));
        let mut file = self.file.lock().unwrap();
        let id = loop {
            let id: String = format!("d-{}", random_bytes(3).iter().map(|b| format!("{b:02x}")).collect::<String>());
            if !file.devices.iter().any(|d| d.id == id) {
                break id;
            }
        };
        let now = now_ms();
        let (key_dpapi, token_hash) = store_key(&crate::secure::k_dev(&token));
        let record = DeviceRecord { id, name: name.to_string(), token_hash, key_dpapi, created_at: now, last_seen_at: now, app_version: None };
        file.devices.push(record.clone());
        self.save(&file);
        (record, token)
    }

    /// Finds the device of a plain token (protocol v1 on loopback only).
    pub fn verify(&self, token: &str) -> Option<String> {
        let wanted = crate::secure::k_dev(token);
        let file = self.file.lock().unwrap();
        // Every device is compared, in constant time per comparison.
        let mut found = None;
        for d in file.devices.iter() {
            if key_of_record(d).is_some_and(|k| ct_eq(&k, &wanted)) {
                found = Some(d.id.clone());
            }
        }
        found
    }

    pub fn touch(&self, id: &str) {
        self.touch_with_version(id, None);
    }

    /// Marks the device as seen now and remembers the app version it reported (`MAJOR.MINOR.PATCH`, maybe `-pre`; anything
    /// else is ignored). `None` leaves the stored version as it is.
    pub fn touch_with_version(&self, id: &str, version: Option<&str>) {
        let mut file = self.file.lock().unwrap();
        if let Some(d) = file.devices.iter_mut().find(|d| d.id == id) {
            d.last_seen_at = now_ms();
            if let Some(v) = version.filter(|v| is_app_version(v)) {
                d.app_version = Some(v.to_string());
            }
            self.save(&file);
        }
    }

    pub fn revoke(&self, id: &str) -> bool {
        let mut file = self.file.lock().unwrap();
        let before = file.devices.len();
        file.devices.retain(|d| d.id != id);
        let removed = file.devices.len() != before;
        if removed {
            self.save(&file);
        }
        removed
    }

    pub fn list(&self) -> Vec<DeviceRecord> {
        self.file.lock().unwrap().devices.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_code(c: &str) -> bool {
        c.len() == 8 && c.bytes().all(|b| PAIR_ALPHABET.contains(&b))
    }

    #[test]
    fn code_is_eight_alphabet_chars() {
        let p = Pairing::new(None);
        for _ in 0..50 {
            assert!(is_code(&p.start().0));
        }
    }

    #[test]
    fn correct_code_works_once() {
        let p = Pairing::new(None);
        let (code, _) = p.start();
        assert!(p.try_consume(&code));
        assert!(!p.try_consume(&code));
    }

    #[test]
    fn five_wrong_attempts_burn_the_code() {
        let p = Pairing::new(None);
        let (code, _) = p.start();
        for _ in 0..5 {
            assert!(!p.try_consume("WRONGCDE"));
        }
        assert!(!p.try_consume(&code));
    }

    #[test]
    fn expired_code_is_rejected() {
        let p = Pairing::new(None);
        let t0 = Instant::now();
        let (code, expires) = p.start_at(t0);
        assert!(!p.try_consume_at(&code, expires));
        let (code, expires) = p.start_at(t0);
        assert!(p.try_consume_at(&code, expires - Duration::from_secs(1)));
    }

    #[test]
    fn starting_again_replaces_the_previous_code() {
        let p = Pairing::new(None);
        let (first, _) = p.start();
        let (second, _) = p.start();
        if first != second {
            assert!(!p.try_consume(&first));
        }
        assert!(p.try_consume(&second));
    }

    #[test]
    fn fixed_code_is_never_consumed_and_has_no_counter() {
        let p = Pairing::new(Some("TESTCDE2".to_string()));
        for _ in 0..10 {
            assert!(!p.try_consume("WRONGCDE"));
        }
        assert!(p.try_consume("TESTCDE2"));
        assert!(p.try_consume("TESTCDE2"));
    }

    #[test]
    fn devices_verify_revoke_and_never_store_the_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        let devices = Devices::new(path.clone(), DevicesFile { version: 1, devices: vec![] });
        let (record, token) = devices.create("Galaxy S9");
        assert_eq!(token.len(), 43);
        assert!(record.id.starts_with("d-") && record.id.len() == 8);
        assert_eq!(devices.verify(&token), Some(record.id.clone()));
        assert_eq!(devices.verify("nope"), None);

        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains(&token));
        // Neither the token nor its hash is in the file in the clear; the key sits there as a DPAPI blob.
        assert!(!on_disk.contains(&to_hex(&crate::secure::k_dev(&token))));
        assert!(on_disk.contains("\"keyDpapi\": \"dpapi:") && !on_disk.contains("tokenHash"));
        assert_eq!(devices.key(&record.id), Some(crate::secure::k_dev(&token)));

        assert!(devices.revoke(&record.id));
        assert_eq!(devices.verify(&token), None);
        assert!(!devices.revoke(&record.id));
        assert!(devices.list().is_empty());
    }

    #[test]
    fn an_old_token_hash_is_migrated_and_the_old_token_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        let token = "dGVzdC10b2tlbi1mb3ItZGVja2EtY3J5cHRvLXYyLTAxMjM";
        let hash = to_hex(&crate::secure::k_dev(token));
        let old = format!(r#"{{"version":1,"devices":[{{"id":"d-aabbcc","name":"Old phone","tokenHash":"{hash}","createdAt":1,"lastSeenAt":2}}]}}"#);
        std::fs::write(&path, &old).unwrap();
        let file: DevicesFile = serde_json::from_str(&old).unwrap();

        let devices = Devices::new(path.clone(), file);
        assert_eq!(devices.verify(token), Some("d-aabbcc".to_string()));
        assert_eq!(devices.key("d-aabbcc"), Some(crate::secure::k_dev(token)));
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains(&hash) && !on_disk.contains("tokenHash") && on_disk.contains("dpapi:"));
    }

    #[test]
    fn candidates_follow_the_active_code_and_failures_burn_it() {
        let p = Pairing::new(Some("TESTCDE2".to_string()));
        assert!(p.candidates().active.is_none());
        assert_eq!(p.candidates().fixed.as_deref(), Some("TESTCDE2"));
        let (code, s_qr, _) = p.start_with_secret();
        assert_eq!(p.candidates().active, Some((code, s_qr)));
        for _ in 0..5 {
            p.record_failure();
        }
        assert!(p.candidates().active.is_none());
        let (_, _, _) = p.start_with_secret();
        p.burn();
        assert!(p.candidates().active.is_none());
    }

    #[test]
    fn the_app_version_of_a_phone_is_remembered_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        let devices = Devices::new(path.clone(), DevicesFile { version: 1, devices: vec![] });
        let (record, _) = devices.create("Galaxy S9");
        assert_eq!(devices.list()[0].app_version, None);
        devices.touch_with_version(&record.id, Some("0.3.1"));
        assert_eq!(devices.list()[0].app_version.as_deref(), Some("0.3.1"));
        // A bad or missing version leaves the stored one alone.
        devices.touch_with_version(&record.id, Some("<script>"));
        devices.touch_with_version(&record.id, None);
        assert_eq!(devices.list()[0].app_version.as_deref(), Some("0.3.1"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"appVersion\": \"0.3.1\""));
    }

    #[test]
    fn what_counts_as_an_app_version() {
        for ok in ["0.1.0", "1.2.3", "10.20.30", "1.0.0-beta.1", "1.0.0+7"] {
            assert!(is_app_version(ok), "{ok}");
        }
        for bad in ["", "1.0", "1.0.0.0", "a.b.c", "1.0.x", "1.0.0 ", "1.0.0<", &"1".repeat(40)] {
            assert!(!is_app_version(bad), "{bad}");
        }
    }
}
