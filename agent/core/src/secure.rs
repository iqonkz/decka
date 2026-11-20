//! Protocol v2 cryptography (PROTOCOL.md section 10). Only `ring` primitives are used: SHA-256, HKDF, PBKDF2, HMAC
//! (verified in constant time by `hmac::verify`) and AES-256-GCM with a strict frame counter.

use std::num::NonZeroU32;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use ring::{digest, hkdf, hmac, pbkdf2};

pub const NONCE_LEN: usize = 16;
const PBKDF2_ITERATIONS: u32 = 200_000;
const AAD: &[u8] = b"decka2";
const HKDF_INFO: &[u8] = b"decka v2 keys";
const PAIR_SALT_PREFIX: &[u8] = b"decka-pair-v2";
const TAG_LEN: usize = 16;

/// Direction labels of the two handshake MACs.
pub const LABEL_CLIENT: u8 = b'C';
pub const LABEL_SERVER: u8 = b'S';

/// The three keys of one connection.
pub struct Keys {
    pub auth: [u8; 32],
    pub c2s: [u8; 32],
    pub s2c: [u8; 32],
}

/// Fresh random bytes (nonces, secrets, device tokens).
pub fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    SystemRandom::new().fill(&mut out).expect("the system random generator works");
    out
}

/// `K_dev` = SHA-256 of the device token text.
pub fn k_dev(token: &str) -> [u8; 32] {
    let d = digest::digest(&digest::SHA256, token.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

/// `S_code` = PBKDF2-HMAC-SHA256(CODE, "decka-pair-v2" || cn || sn, 200000 iterations).
pub fn s_code(code: &str, cn: &[u8], sn: &[u8]) -> [u8; 32] {
    let mut salt = PAIR_SALT_PREFIX.to_vec();
    salt.extend_from_slice(cn);
    salt.extend_from_slice(sn);
    let mut out = [0u8; 32];
    let rounds = NonZeroU32::new(PBKDF2_ITERATIONS).expect("non-zero");
    pbkdf2::derive(pbkdf2::PBKDF2_HMAC_SHA256, rounds, &salt, code.to_uppercase().as_bytes(), &mut out);
    out
}

struct Len(usize);
impl hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

/// HKDF-SHA256(salt = cn || sn, ikm, info = "decka v2 keys", 96 bytes) split into the three keys.
pub fn derive_keys(ikm: &[u8], cn: &[u8], sn: &[u8]) -> Keys {
    let mut salt = cn.to_vec();
    salt.extend_from_slice(sn);
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &salt).extract(ikm);
    let mut okm = [0u8; 96];
    prk.expand(&[HKDF_INFO], Len(96)).expect("96 bytes fit HKDF-SHA256").fill(&mut okm).expect("length matches");
    let part = |i: usize| -> [u8; 32] { okm[i * 32..(i + 1) * 32].try_into().expect("32 bytes") };
    Keys { auth: part(0), c2s: part(1), s2c: part(2) }
}

fn mac_input(label: u8, cn: &[u8], sn: &[u8]) -> Vec<u8> {
    let mut m = vec![label];
    m.extend_from_slice(cn);
    m.extend_from_slice(sn);
    m
}

/// `HMAC-SHA256(k_auth, label || cn || sn)`.
pub fn mac(k_auth: &[u8; 32], label: u8, cn: &[u8], sn: &[u8]) -> Vec<u8> {
    hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_auth), &mac_input(label, cn, sn)).as_ref().to_vec()
}

/// Constant-time check of a received MAC.
pub fn verify_mac(k_auth: &[u8; 32], label: u8, cn: &[u8], sn: &[u8], received: &[u8]) -> bool {
    hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, k_auth), &mac_input(label, cn, sn), received).is_ok()
}

fn nonce_for(counter: u64) -> Nonce {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&counter.to_be_bytes());
    Nonce::assume_unique_for_key(n)
}

fn aes_key(key: &[u8; 32]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).expect("32-byte AES-256 key"))
}

/// Encrypts frames of one direction with a counter that starts at 0.
pub struct Sealer {
    key: LessSafeKey,
    counter: u64,
}

impl Sealer {
    pub fn new(key: &[u8; 32]) -> Sealer {
        Sealer { key: aes_key(key), counter: 0 }
    }

    /// `counter(8, big-endian) || ciphertext || tag`.
    pub fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
        let counter = self.counter;
        self.counter += 1;
        let mut buf = plain.to_vec();
        self.key.seal_in_place_append_tag(nonce_for(counter), Aad::from(AAD), &mut buf).expect("sealing cannot fail");
        let mut frame = counter.to_be_bytes().to_vec();
        frame.extend_from_slice(&buf);
        frame
    }
}

/// Decrypts frames of one direction and demands exactly the next counter value.
pub struct Opener {
    key: LessSafeKey,
    expected: u64,
}

impl Opener {
    pub fn new(key: &[u8; 32]) -> Opener {
        Opener { key: aes_key(key), expected: 0 }
    }

    /// `Err` on a wrong counter (replay, reorder, gap), a short frame or a failed tag. Any error ends the connection.
    pub fn open(&mut self, frame: &[u8]) -> Result<Vec<u8>, ()> {
        if frame.len() < 8 + TAG_LEN {
            return Err(());
        }
        let counter = u64::from_be_bytes(frame[..8].try_into().map_err(|_| ())?);
        if counter != self.expected {
            return Err(());
        }
        let mut buf = frame[8..].to_vec();
        let plain = self.key.open_in_place(nonce_for(counter), Aad::from(AAD), &mut buf).map_err(|_| ())?;
        let plain = plain.to_vec();
        self.expected += 1;
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use serde_json::Value;

    fn fixture() -> Value {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/fixtures/crypto-v2.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap()).collect()
    }

    fn to_hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn s<'a>(v: &'a Value, key: &str) -> &'a str {
        v[key].as_str().unwrap()
    }

    #[test]
    fn shared_vectors_match() {
        let f = fixture();
        let cn = URL_SAFE_NO_PAD.decode(s(&f, "cn")).unwrap();
        let sn = URL_SAFE_NO_PAD.decode(s(&f, "sn")).unwrap();
        assert_eq!(to_hex(&k_dev(s(&f, "token"))), s(&f, "kDev"));
        assert_eq!(to_hex(&s_code(s(&f, "code"), &cn, &sn)), s(&f, "sCode"));
        // The code is case-insensitive.
        assert_eq!(s_code(&s(&f, "code").to_lowercase(), &cn, &sn), s_code(s(&f, "code"), &cn, &sn));

        for mode in ["device", "pair_qr", "pair_code"] {
            let m = &f["modes"][mode];
            let ikm = hex_to_bytes(s(m, "ikm"));
            let keys = derive_keys(&ikm, &cn, &sn);
            let okm: Vec<u8> = [keys.auth, keys.c2s, keys.s2c].concat();
            assert_eq!(to_hex(&okm), s(m, "okm"), "{mode} okm");
            assert_eq!(URL_SAFE_NO_PAD.encode(mac(&keys.auth, LABEL_CLIENT, &cn, &sn)), s(m, "macC"), "{mode} macC");
            assert_eq!(URL_SAFE_NO_PAD.encode(mac(&keys.auth, LABEL_SERVER, &cn, &sn)), s(m, "macS"), "{mode} macS");
            assert!(verify_mac(&keys.auth, LABEL_CLIENT, &cn, &sn, &URL_SAFE_NO_PAD.decode(s(m, "macC")).unwrap()));
            assert!(!verify_mac(&keys.auth, LABEL_SERVER, &cn, &sn, &URL_SAFE_NO_PAD.decode(s(m, "macC")).unwrap()));
        }
        assert_eq!(to_hex(&hex_to_bytes(s(&f["modes"]["pair_qr"], "ikm"))), s(&f, "sQr"));
        assert_eq!(s(&f["modes"]["pair_code"], "ikm"), s(&f, "sCode"));
    }

    #[test]
    fn shared_frames_match_and_open() {
        let f = fixture();
        let cn = URL_SAFE_NO_PAD.decode(s(&f, "cn")).unwrap();
        let sn = URL_SAFE_NO_PAD.decode(s(&f, "sn")).unwrap();
        let keys = derive_keys(&hex_to_bytes(s(&f, "kDev")), &cn, &sn);

        let mut sealer = Sealer::new(&keys.c2s);
        let mut opener = Opener::new(&keys.c2s);
        for frame in f["frames"]["c2s"].as_array().unwrap() {
            let sealed = sealer.seal(s(frame, "plain").as_bytes());
            assert_eq!(to_hex(&sealed), s(frame, "hex"));
            assert_eq!(opener.open(&hex_to_bytes(s(frame, "hex"))).unwrap(), s(frame, "plain").as_bytes());
        }
        let frame = &f["frames"]["s2c"][0];
        assert_eq!(to_hex(&Sealer::new(&keys.s2c).seal(s(frame, "plain").as_bytes())), s(frame, "hex"));
    }

    #[test]
    fn replay_gap_tamper_and_wrong_key_are_refused() {
        let key = [7u8; 32];
        let mut sealer = Sealer::new(&key);
        let (f0, f1, f2) = (sealer.seal(b"zero"), sealer.seal(b"one"), sealer.seal(b"two"));

        let mut o = Opener::new(&key);
        assert_eq!(o.open(&f0).unwrap(), b"zero");
        assert!(o.open(&f0).is_err(), "replay");
        assert!(o.open(&f2).is_err(), "gap");
        assert_eq!(o.open(&f1).unwrap(), b"one");

        let mut tampered = f0.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(Opener::new(&key).open(&tampered).is_err(), "changed byte");
        let mut tampered = f0.clone();
        tampered[10] ^= 1;
        assert!(Opener::new(&key).open(&tampered).is_err(), "changed ciphertext");
        assert!(Opener::new(&[8u8; 32]).open(&f0).is_err(), "wrong key");
        assert!(Opener::new(&key).open(&f0[..10]).is_err(), "short frame");
    }
}
