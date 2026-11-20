//! Pairing URL and its QR code (PROTOCOL.md 8).

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// Percent-encodes everything except unreserved characters (a space becomes `%20`).
fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `http://<primaryIp>:<port>/#c=<CODE>&h=<ip1>,<ip2>&p=<port>&n=<name>&s=<S_qr>`; the code and `S_qr` (base64url) sit in
/// the fragment so they never reach an HTTP request or a log.
pub fn pair_url(primary_ip: &str, ips: &[String], port: u16, code: &str, agent_name: &str, s_qr: &str) -> String {
    format!("http://{primary_ip}:{port}/#c={code}&h={}&p={port}&n={}&s={s_qr}", ips.join(","), percent_encode(agent_name))
}

/// SVG with error correction level M, black on white, quiet zone included.
pub fn qr_svg(text: &str) -> String {
    match QrCode::with_error_correction_level(text, EcLevel::M) {
        Ok(code) => code
            .render::<svg::Color>()
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .quiet_zone(true)
            .min_dimensions(260, 260)
            .build(),
        Err(_) => String::new(), // text too long for a QR code: the UI still shows the code and the URL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_matches_the_documented_format() {
        let ips = vec!["192.168.1.5".to_string(), "10.0.0.2".to_string()];
        assert_eq!(
            pair_url("192.168.1.5", &ips, 9247, "K7M2QX9D", "My PC", "AAEC"),
            "http://192.168.1.5:9247/#c=K7M2QX9D&h=192.168.1.5,10.0.0.2&p=9247&n=My%20PC&s=AAEC"
        );
    }

    #[test]
    fn names_are_percent_encoded() {
        assert_eq!(percent_encode("a&b=c/ü"), "a%26b%3Dc%2F%C3%BC");
    }

    #[test]
    fn svg_is_an_svg_document() {
        let svg = qr_svg("http://192.168.1.5:9247/#c=K7M2QX9D");
        assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"), "{}", &svg[..40.min(svg.len())]);
        assert!(svg.contains("#000000") && svg.contains("#ffffff"));
    }
}
