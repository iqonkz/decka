//! The OBS password is kept encrypted for the current Windows user (DPAPI, `CryptProtectData`), as `dpapi:<base64>`
//! (DATA_MODEL.md 6.4, ARCHITECTURE.md S17). Only this PC and this Windows user can read it back.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

pub const PREFIX: &str = "dpapi:";

/// `secret` -> `dpapi:<base64>`.
pub fn protect(secret: &str) -> Result<String, String> {
    let blob = protect_bytes(secret.as_bytes())?;
    Ok(format!("{PREFIX}{}", STANDARD.encode(blob)))
}

/// `dpapi:<base64>` -> the secret. Anything else (or another user's blob) is an error.
pub fn unprotect(stored: &str) -> Result<String, String> {
    let encoded = stored.strip_prefix(PREFIX).ok_or("not a DPAPI blob")?;
    let blob = STANDARD.decode(encoded).map_err(|_| "not a DPAPI blob".to_string())?;
    String::from_utf8(unprotect_bytes(&blob)?).map_err(|_| "the stored password is not text".to_string())
}

#[cfg(windows)]
fn protect_bytes(plain: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: plain.len() as u32, pbData: plain.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` points at `plain` for the length given; `out` is filled by the call and freed with LocalFree below.
    unsafe {
        CryptProtectData(&input, windows::core::w!("Decka"), None, None, None, 0, &mut out).map_err(|e| e.message())?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        Ok(bytes)
    }
}

#[cfg(windows)]
fn unprotect_bytes(blob: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: blob.len() as u32, pbData: blob.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: as in `protect_bytes`.
    unsafe {
        CryptUnprotectData(&input, None, None, None, None, 0, &mut out).map_err(|e| e.message())?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        Ok(bytes)
    }
}

#[cfg(not(windows))]
fn protect_bytes(_plain: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI exists on Windows only".to_string())
}

#[cfg(not(windows))]
fn unprotect_bytes(_blob: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI exists on Windows only".to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_password_comes_back_and_is_not_in_the_blob() {
        let stored = protect("hunter2-пароль").unwrap();
        assert!(stored.starts_with("dpapi:") && !stored.contains("hunter2"));
        assert_eq!(unprotect(&stored).unwrap(), "hunter2-пароль");
        // Two blobs of the same password differ (DPAPI salts them), both read back.
        let again = protect("hunter2-пароль").unwrap();
        assert_ne!(stored, again);
        assert_eq!(unprotect(&again).unwrap(), "hunter2-пароль");
    }

    #[test]
    fn foreign_text_is_refused() {
        assert!(unprotect("hunter2").is_err());
        assert!(unprotect("dpapi:%%%").is_err());
        assert!(unprotect("dpapi:AAAA").is_err());
    }

    #[test]
    fn an_empty_password_round_trips() {
        assert_eq!(unprotect(&protect("").unwrap()).unwrap(), "");
    }
}
