//! Network policy of every connection (DATA_MODEL.md 11.2): the category of the network a connection arrived on
//! decides what the agent answers. The decision itself is the pure function `policy`; the category comes from Windows
//! (adapter of the local address -> network category) and is cached for 30 s.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Where a connection came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Loopback: USB through `adb reverse`, development, tests.
    Local,
    /// Windows calls the network private or a domain network.
    Private,
    /// Public, or the category could not be determined.
    Public,
}

/// What a connection asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// The web client (any path that is not `/ws` or `/api`).
    Static,
    /// Any `/api/*` path.
    Api,
    /// Pairing handshake with the typed code.
    PairCode,
    /// Pairing handshake with the secret of the QR.
    PairQr,
    /// Protocol v1 (plain text).
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Answer 404 as if there were nothing here.
    NotFound,
    /// Answer with this message and close.
    Refuse(&'static str),
    /// Close the TCP connection without an answer.
    Drop,
}

pub const PUBLIC_PAIR_REFUSAL: &str = "On a public network pair with the QR code or USB";

/// The whole policy in one place. `allow_public` is `config.network.allowPublic`.
pub fn policy(category: Category, request: Request, allow_public: bool) -> Verdict {
    match category {
        Category::Local => Verdict::Allow,
        Category::Private => match request {
            Request::V1 => Verdict::Refuse("protocol v1 is not accepted from the network"),
            _ => Verdict::Allow,
        },
        Category::Public if !allow_public => Verdict::Drop,
        Category::Public => match request {
            Request::Static | Request::Api => Verdict::NotFound,
            Request::PairCode => Verdict::Refuse(PUBLIC_PAIR_REFUSAL),
            Request::PairQr => Verdict::Allow,
            Request::V1 => Verdict::Refuse("protocol v1 is not accepted from the network"),
        },
    }
}

/// Handshakes per minute and IP.
pub fn handshake_limit(category: Category) -> u32 {
    if category == Category::Public {
        5
    } else {
        10
    }
}

/// Both ends of an accepted connection (the extractor `ConnectInfo<ConnInfo>`).
#[derive(Debug, Clone, Copy)]
pub struct ConnInfo {
    pub remote: SocketAddr,
    pub local: SocketAddr,
}

impl axum::extract::connect_info::Connected<axum::serve::IncomingStream<'_, crate::netpolicy::PolicyListener>> for ConnInfo {
    fn connect_info(stream: axum::serve::IncomingStream<'_, crate::netpolicy::PolicyListener>) -> Self {
        let local = stream.io().local_addr().unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0)));
        ConnInfo { remote: *stream.remote_addr(), local }
    }
}

const CACHE_TTL: Duration = Duration::from_secs(30);

/// Category per local address with a 30 s cache; a fixed answer can be set for tests.
#[derive(Default)]
pub struct Networks {
    cache: Mutex<HashMap<IpAddr, (Instant, Category)>>,
    forced: Mutex<Option<Category>>,
}

impl Networks {
    /// Tests: every connection is treated as arriving on a network of this category (`None` = ask Windows).
    pub fn force(&self, category: Option<Category>) {
        *self.forced.lock().unwrap() = category;
    }

    /// The category for connections that arrived on `local`. Blocks for the Windows lookup on a cache miss.
    pub fn category(&self, local: IpAddr) -> Category {
        if let Some(c) = *self.forced.lock().unwrap() {
            return c;
        }
        if local.is_loopback() {
            return Category::Local;
        }
        let now = Instant::now();
        if let Some((at, c)) = self.cache.lock().unwrap().get(&local) {
            if now.duration_since(*at) < CACHE_TTL {
                return *c;
            }
        }
        let category = detect(local);
        self.cache.lock().unwrap().insert(local, (now, category));
        category
    }
}

#[cfg(windows)]
fn detect(local: IpAddr) -> Category {
    match crate::sta::run(move || detect_windows(local)) {
        Ok(Some(c)) => c,
        _ => Category::Public, // not determined: the careful answer
    }
}

#[cfg(not(windows))]
fn detect(_local: IpAddr) -> Category {
    Category::Public
}

/// Adapter GUID (text) that owns the local address.
#[cfg(windows)]
fn adapter_of(local: IpAddr) -> Option<windows::core::GUID> {
    use windows::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH};
    use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6};

    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size = 32 * 1024u32;
    let mut buffer: Vec<u8> = Vec::new();
    for _ in 0..3 {
        buffer.resize(size as usize, 0);
        // SAFETY: `buffer` has `size` bytes; the call fills it with a linked list that stays valid while `buffer` lives.
        let rc = unsafe { GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size) };
        if rc == 0 {
            break;
        }
        if rc != 111 {
            return None; // anything but ERROR_BUFFER_OVERFLOW
        }
    }
    let mut adapter = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    // SAFETY: the list was written by GetAdaptersAddresses into `buffer`, which outlives this loop; every pointer is checked for null.
    unsafe {
        while !adapter.is_null() {
            let a = &*adapter;
            let mut unicast = a.FirstUnicastAddress;
            while !unicast.is_null() {
                let u = &*unicast;
                let sa = u.Address.lpSockaddr;
                if !sa.is_null() {
                    let ip = if (*sa).sa_family == AF_INET {
                        let v4 = &*(sa as *const SOCKADDR_IN);
                        Some(IpAddr::from(std::net::Ipv4Addr::from(u32::from_be(v4.sin_addr.S_un.S_addr))))
                    } else if (*sa).sa_family == AF_INET6 {
                        let v6 = &*(sa as *const SOCKADDR_IN6);
                        Some(IpAddr::from(std::net::Ipv6Addr::from(v6.sin6_addr.u.Byte)))
                    } else {
                        None
                    };
                    if ip == Some(local) && !a.AdapterName.is_null() {
                        let name = a.AdapterName.to_string().ok()?;
                        return windows::core::GUID::try_from(name.trim_matches(|c| c == '{' || c == '}')).ok();
                    }
                }
                unicast = u.Next;
            }
            adapter = a.Next;
        }
    }
    None
}

/// Category of the network of the adapter that owns `local`; `None` if anything could not be found out.
#[cfg(windows)]
fn detect_windows(local: IpAddr) -> Option<Category> {
    use windows::Win32::Networking::NetworkListManager::{INetworkListManager, NetworkListManager, NLM_ENUM_NETWORK_CONNECTED, NLM_NETWORK_CATEGORY_PRIVATE, NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    let adapter = adapter_of(local)?;
    // SAFETY: COM is initialised on the shell thread that runs this function.
    unsafe {
        let manager: INetworkListManager = CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok()?;
        let connections = manager.GetNetworkConnections().ok()?;
        let _ = NLM_ENUM_NETWORK_CONNECTED;
        loop {
            let mut item = [None];
            let mut fetched = 0u32;
            if connections.Next(&mut item, Some(&mut fetched)).is_err() || fetched == 0 {
                return None;
            }
            let Some(connection) = item[0].take() else { return None };
            if connection.GetAdapterId().ok()? == adapter {
                let category = connection.GetNetwork().ok()?.GetCategory().ok()?;
                return Some(if category == NLM_NETWORK_CATEGORY_PRIVATE || category == NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED {
                    Category::Private
                } else {
                    Category::Public
                });
            }
        }
    }
}

/// The listener of the agent: accepts like a `TcpListener` but closes connections of a public network at once when
/// `allowPublic` is off (DATA_MODEL.md 11.2).
pub struct PolicyListener {
    pub inner: tokio::net::TcpListener,
    pub agent: crate::agent::Agent,
}

impl axum::serve::Listener for PolicyListener {
    type Io = tokio::net::TcpStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (io, addr) = match self.inner.accept().await {
                Ok(pair) => pair,
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            if let Ok(local) = io.local_addr() {
                let agent = self.agent.clone();
                let category = tokio::task::spawn_blocking(move || agent.networks().category(local.ip())).await.unwrap_or(Category::Public);
                if policy(category, Request::Static, self.agent.allow_public()) == Verdict::Drop {
                    tracing::info!("closed a connection from {addr}: public network, phones on public networks are off");
                    continue;
                }
            }
            return (io, addr);
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Request; 5] = [Request::Static, Request::Api, Request::PairCode, Request::PairQr, Request::V1];

    #[test]
    fn local_allows_everything() {
        for r in ALL {
            assert_eq!(policy(Category::Local, r, true), Verdict::Allow, "{r:?}");
            assert_eq!(policy(Category::Local, r, false), Verdict::Allow, "{r:?}");
        }
    }

    #[test]
    fn private_allows_all_but_v1() {
        for r in [Request::Static, Request::Api, Request::PairCode, Request::PairQr] {
            assert_eq!(policy(Category::Private, r, true), Verdict::Allow, "{r:?}");
            assert_eq!(policy(Category::Private, r, false), Verdict::Allow, "{r:?}");
        }
        assert!(matches!(policy(Category::Private, Request::V1, true), Verdict::Refuse(_)));
    }

    #[test]
    fn public_is_the_app_with_a_qr_only() {
        assert_eq!(policy(Category::Public, Request::Static, true), Verdict::NotFound);
        assert_eq!(policy(Category::Public, Request::Api, true), Verdict::NotFound);
        assert_eq!(policy(Category::Public, Request::PairCode, true), Verdict::Refuse(PUBLIC_PAIR_REFUSAL));
        assert_eq!(policy(Category::Public, Request::PairQr, true), Verdict::Allow);
        assert!(matches!(policy(Category::Public, Request::V1, true), Verdict::Refuse(_)));
    }

    #[test]
    fn public_with_allow_public_off_is_dropped_whatever_it_asks() {
        for r in ALL {
            assert_eq!(policy(Category::Public, r, false), Verdict::Drop, "{r:?}");
        }
    }

    #[test]
    fn handshake_limits() {
        assert_eq!(handshake_limit(Category::Public), 5);
        assert_eq!(handshake_limit(Category::Private), 10);
        assert_eq!(handshake_limit(Category::Local), 10);
    }

    #[test]
    fn loopback_is_local_without_asking_windows_and_a_forced_category_wins() {
        let n = Networks::default();
        assert_eq!(n.category("127.0.0.1".parse().unwrap()), Category::Local);
        n.force(Some(Category::Public));
        assert_eq!(n.category("127.0.0.1".parse().unwrap()), Category::Public);
        n.force(None);
        assert_eq!(n.category("127.0.0.1".parse().unwrap()), Category::Local);
    }

    /// Asks Windows about the real adapters of this PC: only checks that the lookup finishes and answers.
    #[cfg(windows)]
    #[test]
    fn the_windows_lookup_answers_for_a_real_address() {
        if let Some(ip) = crate::agent::lan_ips().first().and_then(|s| s.parse::<IpAddr>().ok()) {
            let c = Networks::default().category(ip);
            assert!(matches!(c, Category::Private | Category::Public));
        }
    }
}
