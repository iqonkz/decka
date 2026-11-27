//! A small ZIP writer and reader for the problem report (T31.3) and the backup file (T31.4). Entries are stored, not
//! compressed: the files are small, and no archive library is needed. The reader accepts only what the writer makes.

use crate::assistant::Local;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
/// Names are UTF-8.
const UTF8: u16 = 0x0800;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipError {
    #[error("the file is not a ZIP archive or is cut short")]
    Broken,
    #[error("the archive was packed again by another program (compressed or encrypted entries)")]
    Unsupported,
    #[error("the archive is too large")]
    TooLarge,
    #[error("the archive has an unsafe file name")]
    Name,
    #[error("a file in the archive is damaged")]
    Checksum,
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// A relative path with forward slashes that stays inside its folder.
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['\\', ':', '\0'])
        && name.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

fn dos_time(at: &Local) -> (u16, u16) {
    let year = u16::try_from(at.year.clamp(1980, 2107) - 1980).unwrap_or(0);
    let part = |v: u32| u16::try_from(v).unwrap_or(0);
    ((part(at.hour) << 11) | (part(at.minute) << 5), (year << 9) | (part(at.month) << 5) | part(at.day))
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// The archive of `entries` (name, content), dated `at`.
pub fn write(entries: &[(String, Vec<u8>)], at: &Local) -> Result<Vec<u8>, ZipError> {
    let (time, date) = dos_time(at);
    let count = u16::try_from(entries.len()).map_err(|_| ZipError::TooLarge)?;
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in entries {
        if !safe_name(name) {
            return Err(ZipError::Name);
        }
        let offset = u32::try_from(out.len()).map_err(|_| ZipError::TooLarge)?;
        let size = u32::try_from(data.len()).map_err(|_| ZipError::TooLarge)?;
        let name_len = u16::try_from(name.len()).map_err(|_| ZipError::Name)?;
        let crc = crc32(data);
        put32(&mut out, LOCAL);
        for v in [20, UTF8, 0, time, date] {
            put16(&mut out, v);
        }
        for v in [crc, size, size] {
            put32(&mut out, v);
        }
        put16(&mut out, name_len);
        put16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        put32(&mut directory, CENTRAL);
        for v in [20, 20, UTF8, 0, time, date] {
            put16(&mut directory, v);
        }
        for v in [crc, size, size] {
            put32(&mut directory, v);
        }
        // Name length, extra, comment, disk, internal attributes.
        for v in [name_len, 0, 0, 0, 0] {
            put16(&mut directory, v);
        }
        put32(&mut directory, 0);
        put32(&mut directory, offset);
        directory.extend_from_slice(name.as_bytes());
    }
    let start = u32::try_from(out.len()).map_err(|_| ZipError::TooLarge)?;
    let length = u32::try_from(directory.len()).map_err(|_| ZipError::TooLarge)?;
    out.extend_from_slice(&directory);
    put32(&mut out, END);
    for v in [0, 0, count, count] {
        put16(&mut out, v);
    }
    put32(&mut out, length);
    put32(&mut out, start);
    put16(&mut out, 0);
    Ok(out)
}

fn get16(bytes: &[u8], at: usize) -> Result<u16, ZipError> {
    let part = bytes.get(at..at.checked_add(2).ok_or(ZipError::Broken)?).ok_or(ZipError::Broken)?;
    Ok(u16::from_le_bytes([part[0], part[1]]))
}

fn get32(bytes: &[u8], at: usize) -> Result<u32, ZipError> {
    let part = bytes.get(at..at.checked_add(4).ok_or(ZipError::Broken)?).ok_or(ZipError::Broken)?;
    Ok(u32::from_le_bytes([part[0], part[1], part[2], part[3]]))
}

/// The entries of an archive made by `write`. At most `max_total` bytes of content.
pub fn read(bytes: &[u8], max_total: usize) -> Result<Vec<(String, Vec<u8>)>, ZipError> {
    // The end record is the last 22 bytes, or lies before a comment of up to 65535 bytes.
    let last = bytes.len().checked_sub(22).ok_or(ZipError::Broken)?;
    let end = (last.saturating_sub(65_535)..=last).rev().find(|at| get32(bytes, *at) == Ok(END)).ok_or(ZipError::Broken)?;
    let count = get16(bytes, end + 10)?;
    let mut at = get32(bytes, end + 16)? as usize;
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total = 0usize;
    for _ in 0..count {
        if get32(bytes, at)? != CENTRAL {
            return Err(ZipError::Broken);
        }
        let flags = get16(bytes, at + 8)?;
        let method = get16(bytes, at + 10)?;
        let crc = get32(bytes, at + 16)?;
        let packed = get32(bytes, at + 20)? as usize;
        let size = get32(bytes, at + 24)? as usize;
        let name_len = get16(bytes, at + 28)? as usize;
        let skip = get16(bytes, at + 30)? as usize + get16(bytes, at + 32)? as usize;
        let local = get32(bytes, at + 42)? as usize;
        let name = bytes.get(at + 46..at + 46 + name_len).ok_or(ZipError::Broken)?;
        let name = std::str::from_utf8(name).map_err(|_| ZipError::Name)?.to_string();
        at += 46 + name_len + skip;
        // A folder entry has nothing to read.
        if name.ends_with('/') && size == 0 {
            continue;
        }
        if method != 0 || flags & 1 != 0 || packed != size {
            return Err(ZipError::Unsupported);
        }
        if !safe_name(&name) || out.iter().any(|(n, _)| *n == name) {
            return Err(ZipError::Name);
        }
        total = total.checked_add(size).filter(|t| *t <= max_total).ok_or(ZipError::TooLarge)?;
        if get32(bytes, local)? != LOCAL {
            return Err(ZipError::Broken);
        }
        let start = local + 30 + get16(bytes, local + 26)? as usize + get16(bytes, local + 28)? as usize;
        let data = bytes.get(start..start.checked_add(size).ok_or(ZipError::Broken)?).ok_or(ZipError::Broken)?;
        if crc32(data) != crc {
            return Err(ZipError::Checksum);
        }
        out.push((name, data.to_vec()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> Local {
        Local::new(2026, 11, 27, 14, 30).unwrap()
    }

    fn sample() -> Vec<(String, Vec<u8>)> {
        vec![("manifest.json".into(), b"{\"a\":1}".to_vec()), ("icons/abc.png".into(), vec![0, 1, 2, 255]), ("empty.txt".into(), Vec::new())]
    }

    #[test]
    fn what_is_written_is_read_back() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        let bytes = write(&sample(), &at()).unwrap();
        assert_eq!(read(&bytes, 1000).unwrap(), sample());
    }

    #[test]
    fn damage_size_and_unsafe_names_are_refused() {
        let good = write(&sample(), &at()).unwrap();
        assert_eq!(read(&good[..good.len() - 3], 1000), Err(ZipError::Broken));
        assert_eq!(read(b"not an archive at all, just words in a file", 1000), Err(ZipError::Broken));
        assert_eq!(read(&good, 5), Err(ZipError::TooLarge));
        let mut damaged = good.clone();
        // The first content byte, after the 30 byte header and the name.
        damaged[30 + "manifest.json".len()] ^= 1;
        assert_eq!(read(&damaged, 1000), Err(ZipError::Checksum));
        for name in ["../x", "a/../../x", "/x", "C:/x", "a\\b", ""] {
            assert_eq!(write(&[(name.to_string(), Vec::new())], &at()), Err(ZipError::Name), "{name}");
        }
        // A name changed inside a finished archive is refused by the reader too.
        let mut escaped = write(&[("aa/x".into(), b"1".to_vec())], &at()).unwrap();
        let places: Vec<usize> = escaped.windows(4).enumerate().filter(|(_, w)| *w == b"aa/x").map(|(i, _)| i).collect();
        for i in places {
            escaped[i..i + 2].copy_from_slice(b"..");
        }
        assert_eq!(read(&escaped, 1000), Err(ZipError::Name));
    }

    #[cfg(windows)]
    #[test]
    fn windows_opens_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.zip");
        std::fs::write(&file, write(&sample(), &at()).unwrap()).unwrap();
        let tar = std::path::Path::new(&std::env::var("SystemRoot").unwrap()).join("System32").join("tar.exe");
        let out = std::process::Command::new(tar).arg("-xf").arg(&file).arg("-C").arg(dir.path()).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(std::fs::read(dir.path().join("icons").join("abc.png")).unwrap(), [0, 1, 2, 255]);
    }
}
