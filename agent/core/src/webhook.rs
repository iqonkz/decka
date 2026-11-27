//! T29.9: the `http` action (a webhook). It is written only in the settings window of the PC; a phone and the voice name
//! its id. `https://` goes anywhere, `http://` only to this computer and the local network - checked when it is saved and
//! again, on the resolved addresses, when it is sent. No redirects, no cookies, 10 s. The answer is never shown or kept,
//! only its status code. The URL and the headers may hold secrets: they are never logged.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

use url::{Host, Url};

use crate::model::{Action, ActionsFile, HttpMethod};

pub const TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_URL: usize = 2000;
pub const MAX_BODY: usize = 4096;
pub const MAX_HEADERS: usize = 10;
const MAX_HEADER_NAME: usize = 64;
const MAX_HEADER_VALUE: usize = 500;
/// The base64 of a protected value of that length, with room to spare.
const MAX_STORED_VALUE: usize = 4000;
/// Headers that belong to the connection, not to the user.
const OWN_HEADERS: [&str; 8] = ["host", "content-length", "transfer-encoding", "connection", "upgrade", "te", "trailer", "keep-alive"];

pub const NOT_LOCAL: &str = "http:// is only for this computer and the local network - use https://";
pub const NO_ANSWER: &str = "The webhook did not answer";
pub const UNREADABLE_HEADER: &str = "A header of the webhook cannot be read: enter it again";

/// This computer or the local network: loopback, the private ranges and link-local.
pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_local(IpAddr::V4(v4)),
            None => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
        },
    }
}

/// A name that can only be a machine of the local network; its addresses are checked again when the request is sent.
fn local_name(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    !name.contains('.') || [".local", ".lan", ".home", ".home.arpa", ".internal", ".localhost"].iter().any(|s| name.ends_with(s))
}

/// What can be told about a URL without asking the network.
pub fn check_url(text: &str) -> Result<Url, &'static str> {
    if text.chars().count() > MAX_URL {
        return Err("url is too long");
    }
    let url = Url::parse(text).map_err(|_| "url must be a valid http or https URL")?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("url must be a valid http or https URL");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("put a password into a header, not into the url");
    }
    let host = url.host().ok_or("url must be a valid http or https URL")?;
    if url.scheme() == "http" {
        let local = match host {
            Host::Ipv4(ip) => is_local(ip.into()),
            Host::Ipv6(ip) => is_local(ip.into()),
            Host::Domain(name) => local_name(name),
        };
        if !local {
            return Err(NOT_LOCAL);
        }
    }
    Ok(url)
}

pub fn check_header(name: &str, value: &str) -> Result<(), &'static str> {
    let token = !name.is_empty() && name.len() <= MAX_HEADER_NAME && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
    if !token {
        return Err("invalid header name");
    }
    if OWN_HEADERS.contains(&name.to_ascii_lowercase().as_str()) || name.to_ascii_lowercase().starts_with("proxy-") {
        return Err("this header is set by the connection");
    }
    // A stored value is a DPAPI blob; the text inside was checked before it was protected and is checked again before sending.
    if let Some(blob) = value.strip_prefix(crate::dpapi::PREFIX) {
        let base64 = !blob.is_empty() && blob.len() <= MAX_STORED_VALUE && blob.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b));
        return if base64 { Ok(()) } else { Err("invalid header value") };
    }
    if value.len() > MAX_HEADER_VALUE || value.chars().any(|c| c.is_control() && c != '\t') {
        return Err("invalid header value");
    }
    Ok(())
}

/// T29.P: header values are secrets (tokens, keys): `actions.json` keeps them as DPAPI blobs, like the passwords and
/// tokens of the integrations. Returns whether something changed. A value that would not pass the check stays as it is,
/// so that the validation still refuses it; without DPAPI the text is left alone.
pub fn protect_headers(actions: &mut ActionsFile) -> bool {
    let mut changed = false;
    for action in actions.actions.values_mut() {
        let Action::Http { headers: Some(headers), .. } = action else { continue };
        for (name, value) in headers.iter_mut() {
            if value.is_empty() || value.starts_with(crate::dpapi::PREFIX) || check_header(name, value).is_err() {
                continue;
            }
            match crate::dpapi::protect(value) {
                Ok(blob) => {
                    *value = blob;
                    changed = true;
                }
                Err(e) => tracing::warn!("cannot protect a header of a webhook: {e}"),
            }
        }
    }
    changed
}

/// The values as they are sent.
fn open_headers(headers: Option<&BTreeMap<String, String>>) -> Result<BTreeMap<String, String>, String> {
    let mut open = headers.cloned().unwrap_or_default();
    for value in open.values_mut() {
        if value.starts_with(crate::dpapi::PREFIX) {
            *value = crate::dpapi::unprotect(value).map_err(|_| UNREADABLE_HEADER.to_string())?;
        }
    }
    Ok(open)
}

/// `http://` only when every address the name stands for is local (a name that also points outside is refused).
pub fn allowed_target(scheme: &str, addresses: &[SocketAddr]) -> bool {
    !addresses.is_empty() && (scheme == "https" || addresses.iter().all(|a| is_local(a.ip())))
}

async fn request(method: HttpMethod, url: Url, body: Option<String>, headers: BTreeMap<String, String>, addresses: Vec<SocketAddr>, timeout: Duration) -> Result<u16, String> {
    let mut builder = reqwest::Client::builder().timeout(timeout).redirect(reqwest::redirect::Policy::none());
    // The connection goes to the addresses that were checked, and to a local one never through a proxy.
    if let Some(Host::Domain(name)) = url.host() {
        builder = builder.resolve_to_addrs(name, &addresses);
    }
    if url.scheme() == "http" || addresses.iter().all(|a| is_local(a.ip())) {
        builder = builder.no_proxy();
    }
    let client = builder.build().map_err(|_| NO_ANSWER.to_string())?;
    let mut request = match method {
        HttpMethod::Get => client.get(url),
        HttpMethod::Post => client.post(url).body(body.unwrap_or_default()),
    };
    for (name, value) in headers {
        request = request.header(name, value);
    }
    match request.send().await {
        // The body of the answer is not read.
        Ok(answer) => Ok(answer.status().as_u16()),
        Err(e) => {
            tracing::warn!("webhook: {}", e.without_url());
            Err(NO_ANSWER.to_string())
        }
    }
}

/// Sends the request and returns the status code. Blocking (call it from a blocking thread).
pub fn send(method: HttpMethod, url: &str, body: Option<&str>, headers: Option<&BTreeMap<String, String>>) -> Result<u16, String> {
    send_within(method, url, body, headers, TIMEOUT)
}

fn send_within(method: HttpMethod, url: &str, body: Option<&str>, headers: Option<&BTreeMap<String, String>>, timeout: Duration) -> Result<u16, String> {
    // The rules are applied again here: the file may have been edited by hand.
    let url = check_url(url).map_err(str::to_string)?;
    let headers = open_headers(headers)?;
    for (name, value) in &headers {
        check_header(name, value).map_err(str::to_string)?;
    }
    let body = body.map(|b| b.chars().take(MAX_BODY).collect::<String>());
    let port = url.port_or_known_default().ok_or_else(|| NO_ANSWER.to_string())?;
    let host = url.host_str().ok_or_else(|| NO_ANSWER.to_string())?.trim_start_matches('[').trim_end_matches(']').to_string();
    // A thread of its own: the caller may or may not be inside a runtime.
    let worker = std::thread::spawn(move || -> Result<u16, String> {
        let addresses: Vec<SocketAddr> = (host.as_str(), port).to_socket_addrs().map_err(|_| NO_ANSWER.to_string())?.collect();
        if !allowed_target(url.scheme(), &addresses) {
            return Err(NOT_LOCAL.to_string());
        }
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_| NO_ANSWER.to_string())?;
        runtime.block_on(request(method, url, body, headers, addresses, timeout))
    });
    worker.join().map_err(|_| NO_ANSWER.to_string())?
}

/// The result of a press: 2xx is success, anything else is the code (shown as the error of the key).
pub fn outcome(code: u16) -> Result<(), String> {
    if (200..300).contains(&code) {
        Ok(())
    } else {
        Err(format!("HTTP {code}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn plain_http_stays_in_the_local_network() {
        for ok in [
            "http://127.0.0.1:8123/api", "http://localhost/x", "http://192.168.1.20/on", "http://10.0.0.5", "http://172.16.3.4:80", "http://[::1]/",
            "http://[fd00::1]/", "http://nas/share", "http://homeassistant.local:8123/api/webhook/abc", "https://example.com/hook", "https://8.8.8.8/",
        ] {
            assert!(check_url(ok).is_ok(), "{ok}");
        }
        for no in [
            "http://example.com/", "http://8.8.8.8/", "http://172.32.0.1/", "http://100.64.0.1/", "http://[2001:db8::1]/", "http://[::ffff:8.8.8.8]/",
            "http://192.168.1.2.example.com/", "http://127.0.0.1.nip.io/",
        ] {
            assert_eq!(check_url(no), Err(NOT_LOCAL), "{no}");
        }
        for bad in ["ftp://192.168.1.1/", "file:///c:/x", "javascript:alert(1)", "https://user:pass@example.com/", "http://", "", "relative/path"] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
        assert!(check_url(&format!("https://a.example/{}", "x".repeat(2000))).is_err());
    }

    #[test]
    fn a_local_name_that_resolves_outside_is_refused() {
        let local: SocketAddr = "192.168.1.5:80".parse().unwrap();
        let public: SocketAddr = "93.184.216.34:80".parse().unwrap();
        assert!(allowed_target("http", &[local]));
        assert!(!allowed_target("http", &[local, public]) && !allowed_target("http", &[public]) && !allowed_target("http", &[]));
        assert!(allowed_target("https", &[public]));
    }

    #[test]
    fn headers_cannot_take_over_the_connection_or_break_lines() {
        assert!(check_header("Authorization", "Bearer abc").is_ok() && check_header("X-Api-Key", "1").is_ok());
        for (n, v) in [("Host", "evil"), ("content-length", "0"), ("Proxy-Authorization", "x"), ("a b", "1"), ("", "1"), ("X", "a\r\nY: 1")] {
            assert!(check_header(n, v).is_err(), "{n}: {v:?}");
        }
    }

    /// A one-shot server on this computer: answers `status`, returns what it received.
    fn serve(status: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/hook", listener.local_addr().unwrap().port());
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let n = socket.read(&mut buffer).unwrap();
                got.extend_from_slice(&buffer[..n]);
                let text = String::from_utf8_lossy(&got);
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            write!(socket, "HTTP/1.1 {status}\r\nLocation: http://127.0.0.1:1/elsewhere\r\nContent-Length: 6\r\n\r\nsecret").unwrap();
            String::from_utf8_lossy(&got).into_owned()
        });
        (url, thread)
    }

    #[test]
    fn a_post_carries_its_body_and_headers_and_only_the_code_comes_back() {
        let (url, server) = serve("204 No Content");
        let headers = BTreeMap::from([("X-Token".to_string(), "t1".to_string())]);
        assert_eq!(send(HttpMethod::Post, &url, Some("{\"on\":true}"), Some(&headers)), Ok(204));
        let got = server.join().unwrap();
        assert!(got.starts_with("POST /hook HTTP/1.1") && got.contains("x-token: t1") && got.ends_with("{\"on\":true}"), "{got}");
    }

    #[test]
    fn a_redirect_is_not_followed_and_an_error_status_is_the_result() {
        let (url, server) = serve("302 Found");
        assert_eq!(send(HttpMethod::Get, &url, None, None), Ok(302));
        server.join().unwrap();
        assert_eq!(outcome(200), Ok(()));
        assert_eq!(outcome(302), Err("HTTP 302".into()));
    }

    #[cfg(windows)]
    #[test]
    fn header_values_are_stored_protected_and_sent_in_clear() {
        let (url, server) = serve("200 OK");
        let mut file: ActionsFile = serde_json::from_value(serde_json::json!({ "version": 1, "actions": {
            "hook": { "type": "http", "name": "Hook", "method": "GET", "url": url, "headers": { "Authorization": "Bearer s3cret", "X-Empty": "" } },
            "bad": { "type": "http", "name": "Bad", "method": "GET", "url": url, "headers": { "X-Bad": "a\r\nb" } },
        } })).unwrap();
        assert!(protect_headers(&mut file));
        let text = serde_json::to_string(&file).unwrap();
        assert!(!text.contains("s3cret") && text.contains("dpapi:") && text.contains("a\\r\\nb"), "{text}");
        assert!(!protect_headers(&mut file), "a blob is not wrapped twice");
        let Action::Http { headers, .. } = &file.actions["hook"] else { panic!() };
        let stored = headers.as_ref().unwrap();
        assert!(check_header("Authorization", &stored["Authorization"]).is_ok() && stored["X-Empty"].is_empty());
        assert_eq!(send(HttpMethod::Get, &url, None, Some(stored)), Ok(200));
        assert!(server.join().unwrap().contains("authorization: Bearer s3cret"));
        // A blob of another PC or user: nothing is sent.
        let foreign = BTreeMap::from([("X-Token".to_string(), "dpapi:AAAA".to_string())]);
        assert_eq!(send(HttpMethod::Get, "http://127.0.0.1:1/", None, Some(&foreign)), Err(UNREADABLE_HEADER.to_string()));
        assert!(check_header("X", "dpapi:a b").is_err() && check_header("X", "dpapi:").is_err());
    }

    #[test]
    fn silence_ends_after_the_time_limit_and_a_forbidden_target_is_never_contacted() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/", listener.local_addr().unwrap().port());
        let started = std::time::Instant::now();
        assert_eq!(send_within(HttpMethod::Get, &url, None, None, Duration::from_millis(400)), Err(NO_ANSWER.to_string()));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(send(HttpMethod::Get, "http://example.com/", None, None), Err(NOT_LOCAL.to_string()));
    }
}
