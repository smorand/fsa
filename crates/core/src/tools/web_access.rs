//! SPEC-0097: operator access lists for the web tools. This part reads the entries of
//! `web.black_list_url` and `web.white_list_url` once, at boot, and refuses a configuration
//! holding an entry that is not a host name, a `*.host` name, an address or an address range.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::config::WebConfig;
use crate::errors::{Result, ToolError};

const LABEL_MAX: usize = 63;
const NAME_MAX: usize = 253;
/// Redirects followed before giving up, the default of the HTTP client.
const MAX_HOPS: usize = 10;
/// The redirect statuses the client follows by default.
const FOLLOWED: [u16; 5] = [301, 302, 303, 307, 308];

/// One list entry, already in the form the matching reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    /// `docs.example`, in its lower case ASCII form.
    Name(String),
    /// `*.corp.example`, kept without the `*.`.
    Below(String),
    /// `10.0.0.5` or `fd12::1`.
    Addr(IpAddr),
    /// `169.254.0.0/16`: the address as written and the prefix length.
    Range(IpAddr, u8),
}

/// Both lists, parsed.
#[derive(Debug, Default)]
pub(crate) struct AccessLists {
    black: Vec<Entry>,
    white: Vec<Entry>,
    /// Test only name table, consulted instead of the real resolver (DEC-015).
    #[cfg(test)]
    net: Option<std::sync::Arc<TestNet>>,
}

impl AccessLists {
    /// Parse both lists, black before white and in list order, so the first invalid entry is
    /// the one named. Linear in the total length of the entries.
    pub(crate) fn from_config(web: &WebConfig) -> Result<Self> {
        Ok(Self {
            black: parse_list(&web.black_list_url, "black")?,
            white: parse_list(&web.white_list_url, "white")?,
            #[cfg(test)]
            net: web.test_net.clone(),
        })
    }

    /// Both lists empty: the tools take their original path, untouched (DEC-007).
    pub(crate) fn is_empty(&self) -> bool {
        self.black.is_empty() && self.white.is_empty()
    }

    /// Decide a request (DEC-013): black on the typed site, then on every address a name
    /// resolves to, then the white rule (the typed name matches a name entry, or the name has
    /// addresses and every one matches). A name is looked up once, before the deadline, and
    /// the addresses judged are the ones the request is then pinned to (DEC-009, DEC-011).
    /// Linear in the entries times the addresses.
    pub(crate) async fn admit(
        &self,
        url: &str,
        deadline: tokio::time::Instant,
    ) -> Result<Admission> {
        if self.is_empty() {
            return Ok(Admission::default());
        }
        // An address the standard cannot read has no site to judge; the sender refuses it with
        // the message it always had, since it parses with the same crate.
        let Some(read) = url::Url::parse(url).ok().as_ref().and_then(read_site) else {
            return Ok(Admission::default());
        };
        let refused = || {
            ToolError::forbidden(format!(
                "host '{}' is not allowed by the web access lists",
                read.text
            ))
        };
        if self.black.iter().any(|e| e.matches(&read.site)) {
            return Err(refused());
        }
        let white_by_name = self.white.iter().any(|e| e.matches(&read.site));
        let Site::Domain(name) = &read.site else {
            // A literal address is its own and only address: nothing to look up or pin.
            return if self.white.is_empty() || white_by_name {
                Ok(Admission::default())
            } else {
                Err(refused())
            };
        };
        let addrs = self.lookup(name, deadline).await?;
        let sites: Vec<Site> = addrs.iter().map(|a| ip_site(a.ip())).collect();
        if sites.iter().any(|s| self.black.iter().any(|e| e.matches(s))) {
            return Err(refused());
        }
        let white_by_addrs =
            !sites.is_empty() && sites.iter().all(|s| self.white.iter().any(|e| e.matches(s)));
        if !self.white.is_empty() && !white_by_name && !white_by_addrs {
            return Err(refused());
        }
        if addrs.is_empty() {
            return Err(ToolError::internal(format!(
                "fetch request failed: no address for host '{}'",
                read.text
            )));
        }
        Ok(Admission { host: Some(name.clone()), addrs })
    }

    /// The addresses of `name`, empty on any lookup error; an overrun of the deadline fails
    /// the call as a slow site would.
    async fn lookup(&self, name: &str, deadline: tokio::time::Instant) -> Result<Vec<SocketAddr>> {
        #[cfg(test)]
        if let Some(net) = &self.net {
            return tokio::time::timeout_at(deadline, net.resolve(name))
                .await
                .map_err(|_| ToolError::internal("request timed out"));
        }
        // The port is not used: the connection keeps the port of the request (DEC-009).
        match tokio::time::timeout_at(deadline, tokio::net::lookup_host((name, 0))).await {
            Err(_) => Err(ToolError::internal("request timed out")),
            Ok(Err(_)) => Ok(Vec::new()),
            Ok(Ok(found)) => Ok(found.collect()),
        }
    }
}

impl AccessLists {
    /// Send `url`, admitted as `admission`. With both lists empty the client follows redirects
    /// itself, as it always did (DEC-007). Otherwise each redirect is followed by hand so its
    /// destination is admitted like a first request before it is sent, and before the hop limit,
    /// so a refused site is named even past the limit (DEC-010). Returns the answer and the
    /// address it was really requested from. Time O(hops), one lookup per hop at most.
    pub(crate) async fn send(
        &self,
        url: &str,
        mut admission: Admission,
        deadline: tokio::time::Instant,
    ) -> Result<(reqwest::Response, String)> {
        let follow = self.is_empty();
        let mut hop = url.to_string();
        let mut followed = 0;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                return Err(ToolError::internal("request timed out"));
            }
            let sent = self.target(&hop, &admission);
            let resp =
                self.client(&admission, left, follow)?.get(&sent).send().await.map_err(|e| {
                    if e.is_timeout() {
                        ToolError::internal("request timed out")
                    } else {
                        ToolError::internal(format!("fetch request failed: {e}"))
                    }
                })?;
            let next = if follow { None } else { next_hop(&hop, &resp) };
            let Some(next) = next else { return Ok((resp, sent)) };
            admission = self.admit(next.as_str(), deadline).await?;
            if followed == MAX_HOPS {
                return Err(ToolError::internal(format!(
                    "fetch request failed: error following redirect for url ({hop}): too many \
                     redirects"
                )));
            }
            followed += 1;
            hop = next.into();
        }
    }

    /// The client of one hop: the shared setup, pinned to the checked addresses of a looked up
    /// name (DEC-009). The environment proxy stays in effect (owner option A, BL-0157).
    fn client(
        &self,
        admission: &Admission,
        timeout: std::time::Duration,
        follow: bool,
    ) -> Result<reqwest::Client> {
        let mut builder = crate::tools::web::client_builder(timeout);
        if !follow {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
        if let Some((host, addrs)) = admission.pin() {
            builder = builder.resolve_to_addrs(host, addrs);
        }
        #[cfg(test)]
        if let Some(port) = self.net.as_ref().and_then(|n| n.proxy) {
            let proxy = reqwest::Proxy::http(format!("http://127.0.0.1:{port}"))
                .map_err(|e| ToolError::internal(format!("http client error: {e}")))?;
            builder = builder.proxy(proxy);
        }
        builder.build().map_err(|e| ToolError::internal(format!("http client error: {e}")))
    }

    /// The address the bytes are requested from: `url` itself, except under the test seam,
    /// which sends a site to its stand in after the decision (DEC-015).
    fn target(&self, url: &str, admission: &Admission) -> String {
        #[cfg(test)]
        if let Some(net) = &self.net {
            return net.rewrite(url, admission.pin().map_or(&[][..], |(_, addrs)| addrs));
        }
        let _ = admission;
        url.to_string()
    }
}

/// The destination of a redirect answer, read against the hop that sent it, so a relative or
/// protocol relative destination lands where a browser would go. `None` when the answer is not
/// a followed redirect or its destination is missing or unreadable: it is then the answer.
fn next_hop(hop: &str, resp: &reqwest::Response) -> Option<url::Url> {
    if !FOLLOWED.contains(&resp.status().as_u16()) {
        return None;
    }
    let location = resp.headers().get(reqwest::header::LOCATION)?.to_str().ok()?;
    url::Url::parse(hop).ok()?.join(location).ok()
}

/// What a request was admitted as: for a looked up name, the addresses that were judged,
/// which the sender connects to so a second lookup can never pick another (DEC-009).
#[derive(Debug, Default)]
pub(crate) struct Admission {
    host: Option<String>,
    addrs: Vec<SocketAddr>,
}

impl Admission {
    /// The name to pin and its checked addresses, when the request was judged on a lookup.
    fn pin(&self) -> Option<(&str, &[SocketAddr])> {
        self.host.as_deref().map(|h| (h, self.addrs.as_slice()))
    }
}

/// Test only: where the bytes of a request really go, applied after the decision (DEC-015).
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct TestNet {
    sites: std::collections::HashMap<String, u16>,
    names: std::collections::HashMap<String, Vec<NameAnswer>>,
    /// How many lookups each name already answered, to pick its next answer.
    answered: std::sync::Mutex<std::collections::HashMap<String, usize>>,
    lookups: std::sync::atomic::AtomicUsize,
    /// A stand in proxy, as if the server's environment named one (DEC-015).
    proxy: Option<u16>,
}

/// Test only answer of one name lookup.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) enum NameAnswer {
    Addrs(Vec<IpAddr>),
    NoAddress,
    /// Never answers, so only the deadline ends the lookup.
    Hang,
}

#[cfg(test)]
impl TestNet {
    /// Send every request for `site` (a name or a literal address) to `127.0.0.1:port`.
    pub(crate) fn with_site(mut self, site: &str, port: u16) -> Self {
        self.sites.insert(site.to_string(), port);
        self
    }

    /// `name` answers `answers` on its successive lookups, the last one repeating.
    pub(crate) fn with_name(mut self, name: &str, answers: Vec<NameAnswer>) -> Self {
        self.names.insert(name.to_string(), answers);
        self
    }

    /// Route every request through the stand in proxy at `127.0.0.1:port`.
    pub(crate) fn with_proxy(mut self, port: u16) -> Self {
        self.proxy = Some(port);
        self
    }

    /// How many name lookups were made so far, every name together.
    pub(crate) fn lookups(&self) -> usize {
        self.lookups.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The answer of one lookup of `name`; a hang never completes.
    async fn resolve(&self, name: &str) -> Vec<SocketAddr> {
        match self.answer(name) {
            NameAnswer::Addrs(ips) => ips.into_iter().map(|ip| SocketAddr::new(ip, 0)).collect(),
            NameAnswer::NoAddress => Vec::new(),
            NameAnswer::Hang => std::future::pending().await,
        }
    }

    /// The next answer for `name`; a name missing from the table has no address, so no test
    /// ever reaches the real resolver.
    fn answer(&self, name: &str) -> NameAnswer {
        self.lookups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let Some(answers) = self.names.get(name) else { return NameAnswer::NoAddress };
        let mut answered = self.answered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let n = answered.entry(name.to_string()).or_default();
        let pick = answers.get(*n).or(answers.last()).cloned().unwrap_or(NameAnswer::NoAddress);
        *n += 1;
        pick
    }

    /// `url` with its host and port replaced when its site, or else the first address it was
    /// admitted on, is in the table. Keying on the checked address lets a test tell which
    /// address a request would really have reached.
    pub(crate) fn rewrite(&self, url: &str, checked: &[std::net::SocketAddr]) -> String {
        let Ok(mut parsed) = url::Url::parse(url) else { return url.to_string() };
        let by_site = read_site(&parsed).and_then(|r| self.sites.get(&r.text).copied());
        let by_addr = || checked.first().and_then(|a| self.sites.get(&a.ip().to_string()).copied());
        let Some(port) = by_site.or_else(by_addr) else {
            return url.to_string();
        };
        let _ = parsed.set_host(Some("127.0.0.1"));
        let _ = parsed.set_port(Some(port));
        parsed.to_string()
    }
}

/// The site of a request as the web standard reads it.
#[derive(Debug, PartialEq, Eq)]
enum Site {
    /// Lower case ASCII name, no trailing dot.
    Domain(String),
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

/// A site and the text a message shows for it.
struct ReadSite {
    site: Site,
    text: String,
}

/// Read the site of `url`. The text comes from `host_str` and not from `Ipv6Addr::to_string`,
/// because Rust prints an IPv4 mapped address as `::ffff:127.0.0.1` where the standard (and
/// the spec) write `::ffff:7f00:1`.
fn read_site(url: &url::Url) -> Option<ReadSite> {
    let site = match url.host()? {
        url::Host::Domain(d) => Site::Domain(d.strip_suffix('.').unwrap_or(d).to_string()),
        url::Host::Ipv4(a) => Site::V4(a),
        url::Host::Ipv6(a) => Site::V6(a),
    };
    let shown = url.host_str()?;
    let text = match &site {
        Site::Domain(d) => d.clone(),
        _ => shown.trim_start_matches('[').trim_end_matches(']').to_string(),
    };
    Some(ReadSite { site, text })
}

/// A resolved address read as a request site, so the same matching judges both.
fn ip_site(ip: IpAddr) -> Site {
    match ip {
        IpAddr::V4(a) => Site::V4(a),
        IpAddr::V6(a) => Site::V6(a),
    }
}

/// The address a site stands for, an IPv4 mapped IPv6 address being its IPv4 address.
fn site_addr(site: &Site) -> Option<IpAddr> {
    match site {
        Site::Domain(_) => None,
        Site::V4(a) => Some(IpAddr::V4(*a)),
        Site::V6(a) => Some(a.to_ipv4_mapped().map_or(IpAddr::V6(*a), IpAddr::V4)),
    }
}

impl Entry {
    fn matches(&self, site: &Site) -> bool {
        match (self, site) {
            (Self::Name(n), Site::Domain(d)) => n == d,
            (Self::Below(n), Site::Domain(d)) => {
                d.len() > n.len() + 1
                    && d.ends_with(n.as_str())
                    && d[..d.len() - n.len()].ends_with('.')
            }
            (Self::Addr(a), _) => site_addr(site).is_some_and(|s| s == unmap(*a)),
            (Self::Range(net, bits), _) => {
                site_addr(site).is_some_and(|s| in_range(s, *net, *bits))
            }
            _ => false,
        }
    }
}

fn unmap(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(a) => a.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

/// Is `addr` inside `net/bits`? An IPv6 range of 96 bits or more that starts with the IPv4
/// mapped prefix is the IPv4 range of the bits left, so both spellings cover the same hosts.
fn in_range(addr: IpAddr, net: IpAddr, bits: u8) -> bool {
    let (net, bits) = match net {
        IpAddr::V6(n) if bits >= 96 => match n.to_ipv4_mapped() {
            Some(v4) => (IpAddr::V4(v4), bits - 96),
            None => (net, bits),
        },
        _ => (net, bits),
    };
    match (addr, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(bits)).unwrap_or(0);
            u32::from(a) & mask == u32::from(n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(bits)).unwrap_or(0);
            u128::from(a) & mask == u128::from(n) & mask
        }
        _ => false,
    }
}

fn parse_list(entries: &[String], list: &str) -> Result<Vec<Entry>> {
    entries
        .iter()
        .map(|raw| {
            parse_entry(raw).map_err(|()| {
                ToolError::invalid_argument(format!(
                    "invalid entry '{raw}' in the web {list} list: expected a host name, a \
                     *.host name, an IP address or an address range"
                ))
            })
        })
        .collect()
}

/// `Err(())` carries no detail: the caller words the refusal and names the list.
pub(crate) fn parse_entry(raw: &str) -> std::result::Result<Entry, ()> {
    if let Some((addr, len)) = raw.split_once('/') {
        return parse_range(addr, len);
    }
    if let Ok(ip) = raw.parse::<IpAddr>() {
        return Ok(Entry::Addr(ip));
    }
    match raw.strip_prefix("*.") {
        Some(rest) => parse_name(rest).map(Entry::Below),
        None => parse_name(raw).map(Entry::Name),
    }
}

fn parse_range(addr: &str, len: &str) -> std::result::Result<Entry, ()> {
    let ip: IpAddr = addr.parse().map_err(|_| ())?;
    // Digits only and no leading zero: `+8`, `08` and `-1` are not lengths.
    let plain = (1..=3).contains(&len.len())
        && len.bytes().all(|b| b.is_ascii_digit())
        && (len == "0" || !len.starts_with('0'));
    let bits: u8 = if plain { len.parse().map_err(|_| ())? } else { return Err(()) };
    let max = if ip.is_ipv4() { 32 } else { 128 };
    (bits <= max).then_some(Entry::Range(ip, bits)).ok_or(())
}

fn parse_name(name: &str) -> std::result::Result<String, ()> {
    // Exactly one trailing dot is ignored (`blocked.example.` is `blocked.example`), so
    // `blocked.example..` and `.` leave an empty label and are refused below.
    let name = name.strip_suffix('.').unwrap_or(name);
    // An accented name is written the way the standard writes it before the label rules read
    // it, so it is compared with `xn--` requests; `straße` stays apart from `strasse`.
    let ascii;
    let name = if name.is_ascii() {
        name
    } else {
        // `Host::parse` percent decodes, so a `%` would let an accent turn `%63` into a letter
        // and make an entry valid that the plain ASCII rules refuse.
        if name.contains('%') {
            return Err(());
        }
        match url::Host::parse(name).map_err(|_| ())? {
            url::Host::Domain(d) => {
                ascii = d;
                ascii.as_str()
            }
            _ => return Err(()),
        }
    };
    if name.is_empty() || name.len() > NAME_MAX {
        return Err(());
    }
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= LABEL_MAX
            && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    if !name.split('.').all(label_ok) {
        return Err(());
    }
    // A numeric last label is how an address is written, so it must be one: `10.1` and
    // `1.2.3.4.5` are not names (an octal or short form would reach another machine).
    let last = name.rsplit('.').next().unwrap_or(name);
    let numeric = last.bytes().all(|b| b.is_ascii_digit()) || last.starts_with("0x");
    if numeric && name.parse::<Ipv4Addr>().is_err() {
        return Err(());
    }
    match url::Host::parse(name).map_err(|_| ())? {
        url::Host::Domain(d) => Ok(d),
        _ => Err(()),
    }
}
