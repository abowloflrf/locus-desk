use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use reqwest::{
    StatusCode,
    header::{ACCEPT, CONTENT_ENCODING, CONTENT_TYPE, LOCATION},
    redirect::Policy,
};
use thiserror::Error;
use tokio::{net::lookup_host, time::timeout};
use url::{Host, Url};

use super::{ExtractedDocument, MAX_SOURCE_BYTES, extract_document};

const MAX_URL_BYTES: usize = 8_192;
const MAX_REDIRECTS: usize = 5;
const FETCH_TIMEOUT: Duration = Duration::from_secs(40);
const DNS_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str = concat!(
    "LocusDesk/",
    env!("CARGO_PKG_VERSION"),
    " (+self-hosted reader)"
);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContentError {
    #[error("The page URL is invalid.")]
    InvalidUrl,
    #[error("Only HTTP and HTTPS page URLs are supported.")]
    UnsupportedScheme,
    #[error("Page URLs must not contain user information.")]
    UserInfoNotAllowed,
    #[error("The page URL must include a host.")]
    MissingHost,
    #[error("The page host could not be resolved in time.")]
    DnsTimeout,
    #[error("The page host could not be resolved.")]
    DnsResolutionFailed,
    #[error("The page host resolves to a restricted network address.")]
    UnsafeAddress,
    #[error("The page request failed.")]
    RequestFailed,
    #[error("The page redirected too many times.")]
    TooManyRedirects,
    #[error("The page returned an invalid redirect.")]
    InvalidRedirect,
    #[error("The page returned an unsuccessful HTTP status ({0}).")]
    HttpStatus(u16),
    #[error("The response is not an HTML document.")]
    UnsupportedContentType,
    #[error("The response uses an unsupported content encoding.")]
    UnsupportedContentEncoding,
    #[error("The page is larger than the allowed limit.")]
    ResponseTooLarge,
    #[error("The page does not contain extractable content.")]
    EmptyContent,
}

impl ContentError {
    pub fn public_message(&self) -> &'static str {
        match self {
            Self::InvalidUrl => "The page URL is invalid.",
            Self::UnsupportedScheme => "Only HTTP and HTTPS page URLs are supported.",
            Self::UserInfoNotAllowed => "Page URLs must not contain user information.",
            Self::MissingHost => "The page URL must include a host.",
            Self::DnsTimeout => "The page host could not be resolved in time.",
            Self::DnsResolutionFailed => "The page host could not be resolved.",
            Self::UnsafeAddress => "The page host resolves to a restricted network address.",
            Self::RequestFailed => "The page request failed.",
            Self::TooManyRedirects => "The page redirected too many times.",
            Self::InvalidRedirect => "The page returned an invalid redirect.",
            Self::HttpStatus(_) => "The page returned an unsuccessful HTTP status.",
            Self::UnsupportedContentType => "The response is not an HTML document.",
            Self::UnsupportedContentEncoding => {
                "The response uses an unsupported content encoding."
            }
            Self::ResponseTooLarge => "The page is larger than the allowed limit.",
            Self::EmptyContent => "The page does not contain extractable content.",
        }
    }
}

#[async_trait]
pub trait PageFetcher: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<ExtractedDocument, ContentError>;
}

#[derive(Clone)]
pub struct SecurePageFetcher {
    resolver: Arc<dyn HostResolver>,
    transport: Arc<dyn HttpTransport>,
    policy: FetchPolicy,
}

impl SecurePageFetcher {
    pub fn new() -> Self {
        Self {
            resolver: Arc::new(TokioHostResolver),
            transport: Arc::new(ReqwestTransport),
            policy: FetchPolicy::default(),
        }
    }

    #[cfg(test)]
    fn with_components(resolver: Arc<dyn HostResolver>, transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            resolver,
            transport,
            policy: FetchPolicy::default(),
        }
    }

    async fn resolve_target(&self, url: &Url) -> Result<ResolvedTarget, ContentError> {
        let port = url
            .port_or_known_default()
            .ok_or(ContentError::InvalidUrl)?;

        let (domain, mut addresses) = match url.host().ok_or(ContentError::MissingHost)? {
            Host::Ipv4(address) => (None, vec![SocketAddr::new(IpAddr::V4(address), port)]),
            Host::Ipv6(address) => (None, vec![SocketAddr::new(IpAddr::V6(address), port)]),
            Host::Domain(domain) => {
                let resolved =
                    timeout(self.policy.dns_timeout, self.resolver.resolve(domain, port))
                        .await
                        .map_err(|_| ContentError::DnsTimeout)??;
                (Some(domain.to_owned()), resolved)
            }
        };

        if addresses.is_empty() {
            return Err(ContentError::DnsResolutionFailed);
        }
        if addresses.iter().any(|address| !is_public_ip(address.ip())) {
            return Err(ContentError::UnsafeAddress);
        }

        addresses.sort_unstable();
        addresses.dedup();
        Ok(ResolvedTarget { domain, addresses })
    }

    async fn fetch_inner(&self, input: &str) -> Result<ExtractedDocument, ContentError> {
        let mut current_url = parse_page_url(input)?;
        let mut redirect_count = 0;

        loop {
            let target = self.resolve_target(&current_url).await?;
            let response = self
                .transport
                .get(TransportRequest {
                    url: current_url.clone(),
                    domain: target.domain,
                    addresses: target.addresses,
                    connect_timeout: self.policy.connect_timeout,
                    total_timeout: self.policy.total_timeout,
                    max_body_bytes: self.policy.max_body_bytes,
                })
                .await?;

            if is_redirect_status(response.status) {
                if redirect_count == self.policy.max_redirects {
                    return Err(ContentError::TooManyRedirects);
                }
                let location = response.location.ok_or(ContentError::InvalidRedirect)?;
                let next_url = current_url
                    .join(&location)
                    .map_err(|_| ContentError::InvalidRedirect)?;
                current_url = validate_page_url(next_url)?;
                redirect_count += 1;
                continue;
            }

            if !(200..300).contains(&response.status) {
                return Err(ContentError::HttpStatus(response.status));
            }
            if !is_html_content_type(response.content_type.as_deref()) {
                return Err(ContentError::UnsupportedContentType);
            }
            if response.body.len() > self.policy.max_body_bytes {
                return Err(ContentError::ResponseTooLarge);
            }

            return extract_document(current_url.as_str(), response.body);
        }
    }
}

impl Default for SecurePageFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PageFetcher for SecurePageFetcher {
    async fn fetch(&self, input: &str) -> Result<ExtractedDocument, ContentError> {
        timeout(FETCH_TIMEOUT, self.fetch_inner(input))
            .await
            .map_err(|_| ContentError::RequestFailed)?
    }
}

#[derive(Clone, Copy)]
struct FetchPolicy {
    dns_timeout: Duration,
    connect_timeout: Duration,
    total_timeout: Duration,
    max_redirects: usize,
    max_body_bytes: usize,
}

impl Default for FetchPolicy {
    fn default() -> Self {
        Self {
            dns_timeout: DNS_TIMEOUT,
            connect_timeout: CONNECT_TIMEOUT,
            total_timeout: TOTAL_TIMEOUT,
            max_redirects: MAX_REDIRECTS,
            max_body_bytes: MAX_SOURCE_BYTES,
        }
    }
}

struct ResolvedTarget {
    domain: Option<String>,
    addresses: Vec<SocketAddr>,
}

#[async_trait]
trait HostResolver: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ContentError>;
}

struct TokioHostResolver;

#[async_trait]
impl HostResolver for TokioHostResolver {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ContentError> {
        lookup_host((host, port))
            .await
            .map(|addresses| addresses.collect())
            .map_err(|_| ContentError::DnsResolutionFailed)
    }
}

struct TransportRequest {
    url: Url,
    domain: Option<String>,
    addresses: Vec<SocketAddr>,
    connect_timeout: Duration,
    total_timeout: Duration,
    max_body_bytes: usize,
}

struct TransportResponse {
    status: u16,
    location: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

#[async_trait]
trait HttpTransport: Send + Sync {
    async fn get(&self, request: TransportRequest) -> Result<TransportResponse, ContentError>;
}

struct ReqwestTransport;

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn get(&self, request: TransportRequest) -> Result<TransportResponse, ContentError> {
        let mut builder = reqwest::Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .user_agent(USER_AGENT)
            .connect_timeout(request.connect_timeout)
            .timeout(request.total_timeout)
            .gzip(true)
            .deflate(true);
        if let Some(domain) = request.domain.as_deref() {
            builder = builder.resolve_to_addrs(domain, &request.addresses);
        }
        let client = builder.build().map_err(|_| ContentError::RequestFailed)?;
        let mut response = client
            .get(request.url)
            .header(ACCEPT, "text/html, application/xhtml+xml;q=0.9")
            .send()
            .await
            .map_err(|_| ContentError::RequestFailed)?;

        let status = response.status();
        let location = response
            .headers()
            .get(LOCATION)
            .map(|value| value.to_str().map(str::to_owned))
            .transpose()
            .map_err(|_| ContentError::InvalidRedirect)?;
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        if is_redirect_status(status.as_u16()) || !status.is_success() {
            return Ok(TransportResponse {
                status: status.as_u16(),
                location,
                content_type,
                body: Vec::new(),
            });
        }
        if !is_html_content_type(content_type.as_deref()) {
            return Err(ContentError::UnsupportedContentType);
        }
        if response.headers().contains_key(CONTENT_ENCODING) {
            return Err(ContentError::UnsupportedContentEncoding);
        }
        if response
            .content_length()
            .is_some_and(|length| length > request.max_body_bytes as u64)
        {
            return Err(ContentError::ResponseTooLarge);
        }

        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ContentError::RequestFailed)?
        {
            if chunk.len() > request.max_body_bytes.saturating_sub(body.len()) {
                return Err(ContentError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }

        Ok(TransportResponse {
            status: status.as_u16(),
            location,
            content_type,
            body,
        })
    }
}

fn parse_page_url(input: &str) -> Result<Url, ContentError> {
    if input.len() > MAX_URL_BYTES {
        return Err(ContentError::InvalidUrl);
    }
    let url = Url::parse(input).map_err(|_| ContentError::InvalidUrl)?;
    validate_page_url(url)
}

fn validate_page_url(mut url: Url) -> Result<Url, ContentError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ContentError::UnsupportedScheme);
    }
    if url_has_userinfo(&url) {
        return Err(ContentError::UserInfoNotAllowed);
    }
    if url.host().is_none() {
        return Err(ContentError::MissingHost);
    }
    if url.as_str().len() > MAX_URL_BYTES {
        return Err(ContentError::InvalidUrl);
    }
    url.set_fragment(None);
    Ok(url)
}

fn url_has_userinfo(url: &Url) -> bool {
    let Some((_, remainder)) = url.as_str().split_once("://") else {
        return false;
    };
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    remainder[..authority_end].contains('@')
}

fn is_redirect_status(status: u16) -> bool {
    matches!(
        StatusCode::from_u16(status).ok(),
        Some(
            StatusCode::MOVED_PERMANENTLY
                | StatusCode::FOUND
                | StatusCode::SEE_OTHER
                | StatusCode::TEMPORARY_REDIRECT
                | StatusCode::PERMANENT_REDIRECT
        )
    )
}

fn is_html_content_type(content_type: Option<&str>) -> bool {
    content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("text/html")
                || value.eq_ignore_ascii_case("application/xhtml+xml")
        })
}

fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let address = u32::from(address);
    ![
        ("0.0.0.0", 8),
        ("10.0.0.0", 8),
        ("100.64.0.0", 10),
        ("127.0.0.0", 8),
        ("169.254.0.0", 16),
        ("172.16.0.0", 12),
        ("192.0.0.0", 24),
        ("192.0.2.0", 24),
        ("192.88.99.0", 24),
        ("192.168.0.0", 16),
        ("198.18.0.0", 15),
        ("198.51.100.0", 24),
        ("203.0.113.0", 24),
        ("224.0.0.0", 4),
        ("240.0.0.0", 4),
    ]
    .into_iter()
    .any(|(network, prefix)| {
        ipv4_in_prefix(
            address,
            u32::from(network.parse::<Ipv4Addr>().expect("valid IPv4 constant")),
            prefix,
        )
    })
}

fn ipv4_in_prefix(address: u32, network: u32, prefix: u32) -> bool {
    let mask = u32::MAX << (32 - prefix);
    address & mask == network & mask
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if address.to_ipv4_mapped().is_some() {
        return false;
    }

    let address = u128::from(address);
    let global_unicast = ipv6_in_prefix(
        address,
        u128::from_be_bytes([0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        3,
    );
    if !global_unicast {
        return false;
    }

    ![
        ("2001::", 23),
        ("2001:db8::", 32),
        ("2002::", 16),
        ("3fff::", 20),
    ]
    .into_iter()
    .any(|(network, prefix)| {
        ipv6_in_prefix(
            address,
            u128::from(network.parse::<Ipv6Addr>().expect("valid IPv6 constant")),
            prefix,
        )
    })
}

fn ipv6_in_prefix(address: u128, network: u128, prefix: u32) -> bool {
    let mask = u128::MAX << (128 - prefix);
    address & mask == network & mask
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::{Read, Write},
        net::{IpAddr, TcpListener},
        str::FromStr,
        sync::{Arc, Mutex},
        thread,
    };

    use flate2::{Compression, write::GzEncoder};

    use super::*;

    struct StaticResolver {
        address: IpAddr,
        hosts: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl HostResolver for StaticResolver {
        async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ContentError> {
            self.hosts.lock().expect("hosts lock").push(host.to_owned());
            Ok(vec![SocketAddr::new(self.address, port)])
        }
    }

    struct MockTransport {
        responses: Mutex<VecDeque<TransportResponse>>,
        requests: Mutex<Vec<(String, Vec<SocketAddr>)>>,
    }

    #[async_trait]
    impl HttpTransport for MockTransport {
        async fn get(&self, request: TransportRequest) -> Result<TransportResponse, ContentError> {
            self.requests
                .lock()
                .expect("requests lock")
                .push((request.url.into(), request.addresses));
            self.responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .ok_or(ContentError::RequestFailed)
        }
    }

    fn response(
        status: u16,
        location: Option<&str>,
        content_type: Option<&str>,
        body: impl Into<Vec<u8>>,
    ) -> TransportResponse {
        TransportResponse {
            status,
            location: location.map(str::to_owned),
            content_type: content_type.map(str::to_owned),
            body: body.into(),
        }
    }

    #[test]
    fn rejects_non_public_ip_ranges() {
        let blocked = [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "172.16.0.1",
            "192.0.0.170",
            "192.0.2.1",
            "192.88.99.1",
            "192.168.1.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
            "64:ff9b:1::1",
            "100::1",
            "2001::1",
            "2001:2::1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
            "fc00::1",
            "fe80::1",
            "ff00::1",
        ];
        for address in blocked {
            assert!(
                !is_public_ip(IpAddr::from_str(address).expect("valid test IP")),
                "{address}"
            );
        }
    }

    #[test]
    fn permits_public_unicast_addresses() {
        for address in [
            "1.1.1.1",
            "8.8.8.8",
            "2001:4860:4860::8888",
            "2606:4700:4700::1111",
        ] {
            assert!(
                is_public_ip(IpAddr::from_str(address).expect("valid test IP")),
                "{address}"
            );
        }
    }

    #[tokio::test]
    async fn redirects_are_manual_and_each_host_is_resolved() {
        let resolver = Arc::new(StaticResolver {
            address: IpAddr::from_str("93.184.216.34").expect("valid test IP"),
            hosts: Mutex::new(Vec::new()),
        });
        let transport = Arc::new(MockTransport {
            responses: Mutex::new(VecDeque::from([
                response(302, Some("https://next.example/article"), None, Vec::new()),
                response(
                    200,
                    None,
                    Some("text/html; charset=utf-8"),
                    "<main><p>Readable body</p></main>",
                ),
            ])),
            requests: Mutex::new(Vec::new()),
        });
        let fetcher = SecurePageFetcher::with_components(resolver.clone(), transport.clone());

        let document = fetcher
            .fetch("https://start.example/path#fragment")
            .await
            .expect("page fetch succeeds");

        assert_eq!(document.final_url, "https://next.example/article");
        assert_eq!(
            *resolver.hosts.lock().expect("hosts lock"),
            ["start.example", "next.example"]
        );
        let requests = transport.requests.lock().expect("requests lock");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].0, "https://start.example/path");
        assert_eq!(requests[1].0, "https://next.example/article");
        assert!(
            requests
                .iter()
                .all(|(_, addresses)| { addresses == &[SocketAddr::new(resolver.address, 443)] })
        );
    }

    #[tokio::test]
    async fn rejects_unsafe_address_before_transport() {
        let resolver = Arc::new(StaticResolver {
            address: IpAddr::from_str("127.0.0.1").expect("valid test IP"),
            hosts: Mutex::new(Vec::new()),
        });
        let transport = Arc::new(MockTransport {
            responses: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
        });
        let fetcher = SecurePageFetcher::with_components(resolver, transport.clone());

        assert_eq!(
            fetcher.fetch("https://example.com/").await,
            Err(ContentError::UnsafeAddress)
        );
        assert!(transport.requests.lock().expect("requests lock").is_empty());
    }

    #[tokio::test]
    async fn rejects_unsupported_content_type() {
        let fetcher = test_fetcher([response(200, None, Some("text/plain"), "not html")]);

        assert_eq!(
            fetcher.fetch("https://example.com/").await,
            Err(ContentError::UnsupportedContentType)
        );
    }

    #[tokio::test]
    async fn rejects_decompressed_body_over_limit() {
        let fetcher = test_fetcher([response(
            200,
            None,
            Some("application/xhtml+xml"),
            vec![b'x'; MAX_SOURCE_BYTES + 1],
        )]);

        assert_eq!(
            fetcher.fetch("https://example.com/").await,
            Err(ContentError::ResponseTooLarge)
        );
    }

    #[tokio::test]
    async fn reqwest_enforces_limit_after_gzip_decompression_and_sets_user_agent() {
        let Some(listener) = loopback_listener("gzip transport test") else {
            return;
        };
        let address = listener.local_addr().expect("test listener address");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&vec![b'x'; MAX_SOURCE_BYTES + 1])
            .expect("test body compresses");
        let compressed = encoder.finish().expect("test gzip finishes");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test connection accepts");
            let mut request = [0_u8; 4_096];
            let length = stream.read(&mut request).expect("test request reads");
            let request = String::from_utf8_lossy(&request[..length]).into_owned();
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                compressed.len()
            );
            stream
                .write_all(headers.as_bytes())
                .expect("test headers write");
            stream.write_all(&compressed).expect("test response writes");
            request
        });

        let result = ReqwestTransport
            .get(TransportRequest {
                url: Url::parse(&format!("http://{address}/page")).expect("test URL parses"),
                domain: None,
                addresses: vec![address],
                connect_timeout: Duration::from_secs(2),
                total_timeout: Duration::from_secs(5),
                max_body_bytes: MAX_SOURCE_BYTES,
            })
            .await;
        let request = server.join().expect("test server joins");

        assert!(matches!(result, Err(ContentError::ResponseTooLarge)));
        assert!(
            request
                .to_ascii_lowercase()
                .contains(&format!("user-agent: {}", USER_AGENT.to_ascii_lowercase()))
        );
    }

    #[tokio::test]
    async fn reqwest_rejects_unsupported_content_encodings() {
        let Some(listener) = loopback_listener("content encoding transport test") else {
            return;
        };
        let address = listener.local_addr().expect("test listener address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test connection accepts");
            let mut request = [0_u8; 4_096];
            let bytes_read = stream.read(&mut request).expect("test request reads");
            assert!(bytes_read > 0, "test request is not empty");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Encoding: br\r\nContent-Length: 4\r\nConnection: close\r\n\r\ntest",
                )
                .expect("test response writes");
        });

        let result = ReqwestTransport
            .get(TransportRequest {
                url: Url::parse(&format!("http://{address}/page")).expect("test URL parses"),
                domain: None,
                addresses: vec![address],
                connect_timeout: Duration::from_secs(2),
                total_timeout: Duration::from_secs(5),
                max_body_bytes: MAX_SOURCE_BYTES,
            })
            .await;
        server.join().expect("test server joins");

        assert!(matches!(
            result,
            Err(ContentError::UnsupportedContentEncoding)
        ));
    }

    #[tokio::test]
    async fn limits_redirects_to_five_hops() {
        let redirects = (0..=MAX_REDIRECTS)
            .map(|index| response(302, Some(&format!("/redirect/{index}")), None, Vec::new()));
        let fetcher = test_fetcher(redirects);

        assert_eq!(
            fetcher.fetch("https://example.com/").await,
            Err(ContentError::TooManyRedirects)
        );
    }

    #[tokio::test]
    async fn rejects_userinfo_on_initial_and_redirect_urls() {
        let fetcher = test_fetcher(Vec::<TransportResponse>::new());
        assert_eq!(
            fetcher.fetch("https://user:secret@example.com/").await,
            Err(ContentError::UserInfoNotAllowed)
        );

        let fetcher = test_fetcher([response(
            302,
            Some("https://user@example.com/private"),
            None,
            Vec::new(),
        )]);
        assert_eq!(
            fetcher.fetch("https://example.com/").await,
            Err(ContentError::UserInfoNotAllowed)
        );
    }

    #[test]
    fn public_errors_do_not_include_status_or_target_details() {
        assert_eq!(
            ContentError::HttpStatus(418).public_message(),
            "The page returned an unsuccessful HTTP status."
        );
    }

    fn test_fetcher(responses: impl IntoIterator<Item = TransportResponse>) -> SecurePageFetcher {
        let resolver = Arc::new(StaticResolver {
            address: IpAddr::from_str("93.184.216.34").expect("valid test IP"),
            hosts: Mutex::new(Vec::new()),
        });
        let transport = Arc::new(MockTransport {
            responses: Mutex::new(responses.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        });
        SecurePageFetcher::with_components(resolver, transport)
    }

    fn loopback_listener(test_name: &str) -> Option<TcpListener> {
        match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => Some(listener),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("{test_name} skipped because the test sandbox forbids loopback sockets");
                None
            }
            Err(error) => panic!("test listener binds: {error}"),
        }
    }
}
