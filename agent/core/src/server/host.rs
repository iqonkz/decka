//! T29.P: protection from DNS rebinding. A web page of a stranger can point its own name at this PC and then talk to the
//! agent as "its own origin"; the `Host` of such a request carries that name. So only an address, `localhost`, the name
//! of this PC (also `<name>.local`) and the names of `server.allowedHosts` are served; anything else gets 403.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;

/// The `Host` header without its port: `[::1]:8787` -> `[::1]`, `pc:8787` -> `pc`.
fn without_port(host: &str) -> Option<&str> {
    if host.starts_with('[') {
        let end = host.find(']')?;
        let rest = &host[end + 1..];
        return (rest.is_empty() || rest.strip_prefix(':').is_some_and(is_port)).then(|| &host[..=end]);
    }
    match host.rsplit_once(':') {
        Some((name, port)) => is_port(port).then_some(name),
        None => Some(host),
    }
}

fn is_port(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) && text.parse::<u16>().is_ok()
}

/// Whether a request that names `host` is served. `names` are the names of this PC, `extra` the ones the owner added.
pub fn allowed(host: &str, names: &[String], extra: &[String]) -> bool {
    let Some(name) = without_port(host) else { return false };
    if let Some(inside) = name.strip_prefix('[').and_then(|n| n.strip_suffix(']')) {
        // A zone (`%eth0`) belongs to a link-local address.
        return inside.split('%').next().is_some_and(|a| a.parse::<Ipv6Addr>().is_ok());
    }
    if name.parse::<Ipv4Addr>().is_ok() {
        return true;
    }
    let name = name.strip_suffix('.').unwrap_or(name);
    let same = |other: &str| !other.is_empty() && name.eq_ignore_ascii_case(other.trim_end_matches('.'));
    same("localhost") || names.iter().filter(|n| !n.is_empty()).any(|n| same(n) || same(&format!("{n}.local"))) || extra.iter().any(|e| same(e.trim()))
}

/// The names Windows knows this PC by (the short one and the DNS one may differ).
pub fn pc_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names: Vec<String> = [std::env::var("COMPUTERNAME").ok(), sysinfo::System::host_name().ok()].into_iter().flatten().filter(|n| !n.is_empty()).collect();
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        names
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_localhost_and_the_names_of_this_pc_are_served() {
        let names = vec!["DESK-PC".to_string()];
        for ok in [
            "127.0.0.1", "127.0.0.1:8787", "192.168.1.20:8787", "100.64.1.2", "[::1]", "[::1]:8787", "[fe80::1%12]:8787",
            "localhost", "LOCALHOST:8787", "localhost.", "desk-pc", "Desk-PC:8787", "desk-pc.local", "desk-pc.local.:8787",
        ] {
            assert!(allowed(ok, &names, &[]), "{ok}");
        }
        for no in [
            "evil.example", "evil.example:8787", "127.0.0.1.nip.io", "localhost.evil.example", "desk-pc.evil.example", "desk-pc.lan",
            "other-pc", "", ":8787", "[::1", "[::1]x", "[not-an-address]", "127.0.0.1:99999", "127.0.0.1:8787:1", "::1", "user@127.0.0.1",
        ] {
            assert!(!allowed(no, &names, &[]), "{no}");
        }
        let extra = vec!["decka.tail1234.ts.net".to_string(), "".to_string()];
        assert!(allowed("Decka.Tail1234.ts.net:8787", &names, &extra));
        assert!(!allowed("x.decka.tail1234.ts.net", &names, &extra) && !allowed("", &names, &extra));
        assert!(!allowed(".local", &[String::new()], &[]));
    }
}
