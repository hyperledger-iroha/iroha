//! Defaults for various items used in communication over http(s).
//!
//! These implementations rely on the `reqwest` and `tungstenite` crates and
//! provide a simple, feature-complete transport layer used by the client. The
//! module does not yet expose configuration hooks for alternative backends.
use crate::http::{Method, RequestBuilder, Response};
use eyre::{Error, Result, WrapErr, eyre};
use http::header::{HeaderName, HeaderValue};
use reqwest::blocking::Client as BlockingClient;
use std::{net::TcpStream, sync::OnceLock, thread};
pub use tungstenite::handshake::client::Response as WebSocketResponse;
pub use tungstenite::{Error as WebSocketError, Message as WebSocketMessage};
use tungstenite::{WebSocket, client::IntoClientRequest, stream::MaybeTlsStream};
use url::Url;
type Bytes = Vec<u8>;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const RESPONSE_INITIAL_ALLOCATION_BYTES: usize = 16 * 1024;
const RESPONSE_READ_BUFFER_BYTES: usize = 16 * 1024;
#[cfg(test)]
use std::sync::{Arc, Mutex};
fn header_name_from_str(str: &str) -> Result<HeaderName> {
    str.parse::<HeaderName>()
        .wrap_err_with(|| format!("Failed to parse header name {str}"))
}
#[derive(Debug)]
struct PendingRequest {
    method: Method,
    url: Url,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Option<Vec<u8>>,
    timeout: Option<std::time::Duration>,
    max_response_bytes: usize,
    direct_loopback: bool,
}
#[derive(Debug)]
struct PreparedRequest {
    method: Method,
    url: Url,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Vec<u8>,
    timeout: Option<std::time::Duration>,
    max_response_bytes: usize,
    direct_loopback: bool,
}
/// Default request builder implemented on top of `reqwest`.
#[derive(Debug)]
pub struct DefaultRequestBuilder {
    inner: Result<PendingRequest>,
}
impl DefaultRequestBuilder {
    /// Apply `.and_then()` semantics to the inner `Result` with underlying request state.
    fn and_then<F>(self, fun: F) -> Self
    where
        F: FnOnce(PendingRequest) -> Result<PendingRequest>,
    {
        Self {
            inner: self.inner.and_then(fun),
        }
    }
    /// Build request by consuming self.
    pub fn build(self) -> Result<DefaultRequest> {
        self.inner.map(|pending| DefaultRequest {
            prepared: PreparedRequest {
                method: pending.method,
                url: pending.url,
                headers: pending.headers,
                body: pending.body.unwrap_or_default(),
                timeout: pending.timeout,
                max_response_bytes: pending.max_response_bytes,
                direct_loopback: pending.direct_loopback,
            },
        })
    }
    /// Apply per-request timeout (overrides the client default when set).
    #[must_use]
    pub fn timeout(self, timeout: std::time::Duration) -> Self {
        self.and_then(|mut pending| {
            pending.timeout = Some(timeout);
            Ok(pending)
        })
    }
    /// Bound the decoded HTTP response body retained in memory.
    ///
    /// The limit applies even when a peer omits or lies about `Content-Length` and after any
    /// transparent content decoding performed by the HTTP transport.
    #[must_use]
    pub fn max_response_bytes(self, max_response_bytes: usize) -> Self {
        self.and_then(|mut pending| {
            if max_response_bytes == 0 {
                return Err(eyre!("HTTP response byte limit must be positive"));
            }
            pending.max_response_bytes = max_response_bytes;
            Ok(pending)
        })
    }

    /// Require a direct, proxy-free cleartext connection to an exact loopback host.
    pub(crate) fn direct_loopback(self) -> Self {
        self.and_then(|mut pending| {
            let loopback = match pending.url.host() {
                Some(url::Host::Domain(domain)) => domain == "localhost",
                Some(url::Host::Ipv4(address)) => address.is_loopback(),
                Some(url::Host::Ipv6(address)) => address.is_loopback(),
                None => false,
            };
            if pending.url.scheme() != "http" || !loopback {
                return Err(eyre!(
                    "direct cleartext HTTP is restricted to exact localhost, 127/8, or ::1 loopback hosts"
                ));
            }
            pending.direct_loopback = true;
            Ok(pending)
        })
    }
}
/// Request built by [`DefaultRequestBuilder`].
#[derive(Debug)]
pub struct DefaultRequest {
    prepared: PreparedRequest,
}
#[cfg(test)]
#[derive(Clone, Debug)]
pub struct RequestSnapshot {
    pub method: Method,
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timeout: Option<std::time::Duration>,
    pub max_response_bytes: usize,
    pub direct_loopback: bool,
}
#[cfg(test)]
type SendHook = Arc<dyn Fn(RequestSnapshot) -> Result<Response<Bytes>> + Send + Sync + 'static>;
#[cfg(test)]
fn send_hook_slot() -> &'static Mutex<Option<SendHook>> {
    static HOOK: OnceLock<Mutex<Option<SendHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}
#[cfg(test)]
pub fn set_send_hook(hook: Option<SendHook>) {
    *send_hook_slot().lock().expect("set send hook") = hook;
}
#[cfg(test)]
pub fn with_send_hook<R>(hook: SendHook, f: impl FnOnce() -> R) -> R {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    static HOOK_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = HOOK_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("hook guard");
    set_send_hook(Some(hook));
    let outcome = catch_unwind(AssertUnwindSafe(f));
    set_send_hook(None);
    drop(guard);
    match outcome {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
#[cfg(test)]
fn try_send_with_hook(request: &DefaultRequest) -> Option<Result<Response<Bytes>>> {
    let hook_opt = send_hook_slot()
        .lock()
        .expect("lock send hook")
        .as_ref()
        .cloned();
    hook_opt.map(|hook| hook(request.snapshot()))
}
#[cfg(test)]
impl DefaultRequest {
    fn snapshot(&self) -> RequestSnapshot {
        let headers_vec = self
            .prepared
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    std::str::from_utf8(value.as_bytes())
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
            .collect();
        RequestSnapshot {
            method: self.prepared.method.clone(),
            url: self.prepared.url.clone(),
            headers: headers_vec,
            body: self.prepared.body.clone(),
            timeout: self.prepared.timeout,
            max_response_bytes: self.prepared.max_response_bytes,
            direct_loopback: self.prepared.direct_loopback,
        }
    }
}
impl DefaultRequest {
    #[cfg(test)]
    #[must_use]
    pub fn uri(&self) -> &Url {
        &self.prepared.url
    }
    /// Sends itself and returns byte response
    ///
    /// # Errors
    /// Fails if request building and sending fails or response transformation fails
    pub fn send(self) -> Result<Response<Bytes>> {
        #[cfg(test)]
        if let Some(result) = try_send_with_hook(&self) {
            return result;
        }
        // If we are running inside a Tokio runtime, offload the blocking reqwest call to
        // a dedicated thread to avoid nested-runtime drops in a non-blocking context.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let flavor = handle.runtime_flavor();
            return thread::spawn(move || self.into_response())
                .join()
                .unwrap_or_else(|_| {
                    Err(eyre!(
                        "blocking HTTP request thread panicked; runtime {flavor:?}"
                    ))
                });
        }
        self.into_response()
    }
    fn into_response(self) -> Result<Response<Bytes>> {
        let PreparedRequest {
            method,
            url,
            headers,
            body,
            timeout,
            max_response_bytes,
            direct_loopback,
        } = self.prepared;
        let direct_client = direct_loopback
            .then(|| build_direct_loopback_http_client(&url))
            .transpose()?;
        #[allow(clippy::option_if_let_else, reason = "lazy client selection")]
        let client = match &direct_client {
            Some(client) => client,
            None => http_client(),
        };
        let mut builder = client.request(method.clone(), url.clone());
        for (name, value) in &headers {
            builder = builder.header(name.clone(), value.clone());
        }
        if !body.is_empty() {
            builder = builder.body(body);
        }
        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }
        let response = builder
            .send()
            .wrap_err_with(|| format!("Failed to send http {method} request to {url}"))?;
        ClientResponse {
            response,
            max_response_bytes,
        }
        .try_into()
    }
}
impl RequestBuilder for DefaultRequestBuilder {
    fn new(method: Method, url: Url) -> Self {
        Self {
            inner: Ok(PendingRequest {
                method,
                url,
                headers: Vec::new(),
                body: None,
                timeout: None,
                max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
                direct_loopback: false,
            }),
        }
    }
    fn header<K: AsRef<str>, V: ToString + ?Sized>(self, key: K, value: &V) -> Self {
        self.and_then(|mut pending| {
            let name = header_name_from_str(key.as_ref())?;
            let header_value = HeaderValue::from_str(&value.to_string())
                .wrap_err_with(|| format!("Failed to parse header value for {name}"))?;
            pending.headers.push((name, header_value));
            Ok(pending)
        })
    }
    fn param<K: AsRef<str>, V: ToString + ?Sized>(self, key: K, value: &V) -> Self {
        self.and_then(|mut pending| {
            {
                let mut pairs = pending.url.query_pairs_mut();
                pairs.append_pair(key.as_ref(), &value.to_string());
            }
            Ok(pending)
        })
    }
    fn body(self, data: Vec<u8>) -> Self {
        self.and_then(|mut pending| {
            pending.body = Some(data);
            Ok(pending)
        })
    }
}
/// Request builder built on top of [`http::request::Builder`]. Used for `WebSocket` connections.
pub struct DefaultWebSocketRequestBuilder(Result<http::request::Builder>);
impl DefaultWebSocketRequestBuilder {
    /// Same as [`DefaultRequestBuilder::and_then`].
    fn and_then<F>(self, func: F) -> Self
    where
        F: FnOnce(http::request::Builder) -> Result<http::request::Builder>,
    {
        Self(self.0.and_then(func))
    }
    /// Consumes itself to build request.
    pub fn build(self) -> Result<DefaultWebSocketStreamRequest> {
        let builder = self.0?;
        let mut request = builder
            .uri_ref()
            .ok_or_else(|| eyre!("Missing URI"))?
            .into_client_request()?;
        for (header, value) in builder
            .headers_ref()
            .ok_or_else(|| eyre!("No headers found"))?
        {
            request.headers_mut().entry(header).or_insert(value.clone());
        }
        Ok(DefaultWebSocketStreamRequest(request))
    }
}
/// `WebSocket` request built by [`DefaultWebSocketRequestBuilder`]
pub struct DefaultWebSocketStreamRequest(http::Request<()>);
impl DefaultWebSocketStreamRequest {
    /// Open [`WebSocketStream`] synchronously.
    pub fn connect(self) -> Result<WebSocketStream> {
        let (stream, _) = self.connect_with_response()?;
        Ok(stream)
    }
    /// Open [`WebSocketStream`] synchronously and retain the HTTP upgrade response.
    pub fn connect_with_response(self) -> Result<(WebSocketStream, WebSocketResponse)> {
        Ok(tungstenite::connect(self.0)?)
    }
    /// Open [`AsyncWebSocketStream`].
    pub async fn connect_async(self) -> Result<AsyncWebSocketStream> {
        let (stream, _) = self.connect_async_with_response().await?;
        Ok(stream)
    }
    /// Open [`AsyncWebSocketStream`] and retain the HTTP upgrade response.
    pub async fn connect_async_with_response(
        self,
    ) -> Result<(AsyncWebSocketStream, WebSocketResponse)> {
        Ok(tokio_tungstenite::connect_async(self.0).await?)
    }
}
impl RequestBuilder for DefaultWebSocketRequestBuilder {
    fn new(method: Method, url: Url) -> Self {
        Self(Ok(http::Request::builder()
            .method(method)
            .uri(url.as_ref())))
    }
    fn param<K, V: ?Sized>(self, _key: K, _val: &V) -> Self {
        Self(self.0.and(Err(eyre!("No params expected"))))
    }
    fn header<N: AsRef<str>, V: ToString + ?Sized>(self, name: N, value: &V) -> Self {
        self.and_then(|b| Ok(b.header(header_name_from_str(name.as_ref())?, value.to_string())))
    }
    fn body(self, data: Vec<u8>) -> Self {
        self.and_then(|b| {
            if data.is_empty() {
                Ok(b)
            } else {
                Err(eyre!("Empty body expected, got: {:?}", data))
            }
        })
    }
}
pub type WebSocketStream = WebSocket<MaybeTlsStream<TcpStream>>;
pub type AsyncWebSocketStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
fn http_client() -> &'static BlockingClient {
    static CLIENT: OnceLock<BlockingClient> = OnceLock::new();
    CLIENT.get_or_init(build_http_client)
}
fn blocking_http_client_builder() -> reqwest::blocking::ClientBuilder {
    BlockingClient::builder()
        // This transport carries one-shot signed requests. Following a redirect
        // could replay a body after the original endpoint already admitted it.
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
}
fn build_http_client() -> BlockingClient {
    blocking_http_client_builder()
        .build()
        .expect("Failed to build blocking HTTP client")
}
fn build_direct_loopback_http_client(url: &Url) -> Result<BlockingClient> {
    let mut builder = blocking_http_client_builder().no_proxy();
    match url.host() {
        Some(url::Host::Domain("localhost")) => {
            let addresses = [
                std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
                std::net::SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 0)),
            ];
            builder = builder.resolve_to_addrs("localhost", &addresses);
        }
        Some(url::Host::Ipv4(address)) if address.is_loopback() => {}
        Some(url::Host::Ipv6(address)) if address.is_loopback() => {}
        _ => return Err(eyre!("direct HTTP client requires an exact loopback URL")),
    }
    builder
        .build()
        .wrap_err("failed to build direct loopback HTTP client")
}
struct ClientResponse {
    response: reqwest::blocking::Response,
    max_response_bytes: usize,
}
fn read_bounded_response_body(
    reader: &mut impl std::io::Read,
    advertised_length: Option<u64>,
    max_response_bytes: usize,
) -> Result<Vec<u8>> {
    if max_response_bytes == 0 {
        return Err(eyre!("HTTP response byte limit must be positive"));
    }
    let max_response_bytes_u64 = u64::try_from(max_response_bytes).unwrap_or(u64::MAX);
    if let Some(length) = advertised_length.filter(|length| *length > max_response_bytes_u64) {
        return Err(eyre!(
            "HTTP response Content-Length {length} exceeds the {max_response_bytes}-byte limit"
        ));
    }
    let initial_capacity = response_initial_capacity(advertised_length, max_response_bytes);
    let mut body = Vec::new();
    reserve_response_body_capacity(&mut body, initial_capacity, max_response_bytes)?;
    let mut buffer = [0_u8; RESPONSE_READ_BUFFER_BYTES];
    loop {
        let remaining = max_response_bytes - body.len();
        let read_capacity = remaining.min(buffer.len());
        if read_capacity == 0 {
            let read = read_response_body_chunk(reader, &mut buffer[..1])?;
            if read == 0 {
                break;
            }
            return Err(eyre!(
                "HTTP response body exceeds the {max_response_bytes}-byte limit"
            ));
        }
        let read = read_response_body_chunk(reader, &mut buffer[..read_capacity])?;
        if read == 0 {
            break;
        }
        let required_len = body
            .len()
            .checked_add(read)
            .ok_or_else(|| eyre!("HTTP response body length overflow"))?;
        reserve_response_body_capacity(&mut body, required_len, max_response_bytes)?;
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(body)
}
fn response_initial_capacity(advertised_length: Option<u64>, max_response_bytes: usize) -> usize {
    advertised_length
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(RESPONSE_INITIAL_ALLOCATION_BYTES)
        .min(RESPONSE_INITIAL_ALLOCATION_BYTES)
        .min(max_response_bytes)
}
fn read_response_body_chunk(reader: &mut impl std::io::Read, buffer: &mut [u8]) -> Result<usize> {
    loop {
        match reader.read(buffer) {
            Ok(read) if read <= buffer.len() => return Ok(read),
            Ok(read) => {
                return Err(eyre!(
                    "HTTP response reader reported {read} bytes for a {}-byte buffer",
                    buffer.len()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error).wrap_err("Failed to read HTTP response body"),
        }
    }
}
fn reserve_response_body_capacity(
    body: &mut Vec<u8>,
    required_len: usize,
    max_response_bytes: usize,
) -> Result<()> {
    if required_len > max_response_bytes {
        return Err(eyre!(
            "HTTP response body exceeds the {max_response_bytes}-byte limit"
        ));
    }
    if required_len <= body.capacity() {
        return Ok(());
    }
    let target_capacity = body
        .capacity()
        .saturating_mul(2)
        .max(required_len)
        .min(max_response_bytes);
    let additional = target_capacity
        .checked_sub(body.len())
        .ok_or_else(|| eyre!("HTTP response body capacity accounting underflow"))?;
    body.try_reserve_exact(additional)
        .wrap_err_with(|| format!("Failed to reserve {target_capacity} HTTP response body bytes"))
}
impl TryFrom<ClientResponse> for Response<Bytes> {
    type Error = Error;
    fn try_from(response: ClientResponse) -> Result<Self> {
        let ClientResponse {
            mut response,
            max_response_bytes,
        } = response;
        let status = response.status();
        let advertised_length = response.content_length();
        let headers: Vec<(HeaderName, HeaderValue)> = response
            .headers()
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        let body =
            read_bounded_response_body(&mut response, advertised_length, max_response_bytes)?;
        let mut builder = Response::builder().status(status);
        let headers_map = builder
            .headers_mut()
            .ok_or_else(|| eyre!("Failed to get headers map reference."))?;
        for (key, value) in headers {
            headers_map.insert(key, value);
        }
        builder
            .body(body)
            .wrap_err("Failed to construct response bytes body")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{ErrorKind, Read, Write},
        net::TcpListener,
        sync::Arc,
        time::{Duration, Instant},
    };

    #[test]
    fn direct_loopback_builder_is_fail_closed_and_leaves_https_proxy_capable() {
        for allowed in [
            "http://localhost:8080/v1/fees/quote",
            "http://127.44.55.66:8080/v1/fees/quote",
            "http://[::1]:8080/v1/fees/quote",
        ] {
            let request = DefaultRequestBuilder::new(
                crate::http::Method::POST,
                Url::parse(allowed).expect("loopback URL"),
            )
            .direct_loopback()
            .build()
            .expect("direct loopback request");
            assert!(request.snapshot().direct_loopback);
        }
        for rejected in [
            "http://example.com/v1/fees/quote",
            "http://[::2]/v1/fees/quote",
            "https://localhost:8080/v1/fees/quote",
        ] {
            assert!(
                DefaultRequestBuilder::new(
                    crate::http::Method::POST,
                    Url::parse(rejected).expect("rejected URL"),
                )
                .direct_loopback()
                .build()
                .is_err(),
                "direct-loopback mode admitted {rejected}"
            );
        }
        let https = DefaultRequestBuilder::new(
            crate::http::Method::POST,
            Url::parse("https://fees.example/v1/fees/quote").expect("HTTPS URL"),
        )
        .build()
        .expect("ordinary HTTPS request");
        assert!(
            !https.snapshot().direct_loopback,
            "HTTPS must retain the ordinary system-proxy-capable transport"
        );
    }

    #[test]
    fn kagemusha_lifecycle_loopback_transport_ignores_proxy_environment() {
        fn serve_once(listener: &TcpListener, status: &str) -> bool {
            listener
                .set_nonblocking(true)
                .expect("nonblocking listener");
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .expect("proxy test stream read timeout");
                        let mut request = [0_u8; 2048];
                        let read = stream.read(&mut request).expect("read proxy test request");
                        assert!(read > 0, "proxy test request must not be empty");
                        write!(
                            stream,
                            "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .expect("write proxy test response");
                        return true;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return false;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("proxy test listener failed: {error}"),
                }
            }
        }

        const CHILD: &str = "IROHA_LOOPBACK_PROXY_TEST_CHILD";
        const TARGET: &str = "IROHA_LOOPBACK_PROXY_TEST_TARGET";
        if std::env::var_os(CHILD).is_some() {
            let url = std::env::var(TARGET).expect("child target URL");
            let response = DefaultRequestBuilder::new(
                crate::http::Method::POST,
                Url::parse(&url).expect("child target URL parse"),
            )
            .direct_loopback()
            .body(b"authenticated fee quote".to_vec())
            .timeout(Duration::from_secs(2))
            .build()
            .expect("child direct request")
            .send()
            .expect("child direct response");
            assert_eq!(response.status(), http::StatusCode::OK);
            return;
        }

        let target_listener = TcpListener::bind("127.0.0.1:0").expect("target listener");
        let target_address = target_listener.local_addr().expect("target address");
        let proxy_listener = TcpListener::bind("127.0.0.1:0").expect("proxy listener");
        let proxy_address = proxy_listener.local_addr().expect("proxy address");
        let target_server = thread::spawn(move || serve_once(&target_listener, "200 OK"));
        let proxy_server = thread::spawn(move || serve_once(&proxy_listener, "502 Bad Gateway"));
        let proxy_url = format!("http://{proxy_address}");
        let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "http_default::tests::kagemusha_lifecycle_loopback_transport_ignores_proxy_environment",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env(
                TARGET,
                format!("http://localhost:{}/v1/fees/quote", target_address.port()),
            )
            .env("HTTP_PROXY", &proxy_url)
            .env("http_proxy", &proxy_url)
            .env("ALL_PROXY", &proxy_url)
            .env("all_proxy", &proxy_url)
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .env_remove("REQUEST_METHOD")
            .output()
            .expect("run isolated proxy child");
        let target_received = target_server.join().expect("target server");
        let proxy_received = proxy_server.join().expect("proxy server");
        assert!(
            child.status.success(),
            "isolated direct-loopback child failed: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        assert!(
            target_received,
            "direct loopback target received no request"
        );
        assert!(
            !proxy_received,
            "HTTP_PROXY/ALL_PROXY captured the cleartext loopback request"
        );
    }

    #[test]
    fn owned_http_client_does_not_follow_signed_body_redirects() {
        for (status_code, reason) in [
            (307_u16, "Temporary Redirect"),
            (308_u16, "Permanent Redirect"),
        ] {
            let redirect_listener = TcpListener::bind("127.0.0.1:0").expect("redirect listener");
            let redirect_addr = redirect_listener.local_addr().expect("redirect address");
            let target_listener = TcpListener::bind("127.0.0.1:0").expect("target listener");
            let target_addr = target_listener.local_addr().expect("target address");
            target_listener
                .set_nonblocking(true)
                .expect("nonblocking target listener");
            let redirect_server = thread::spawn(move || {
                let (mut stream, _) = redirect_listener.accept().expect("redirect request");
                let mut request = [0_u8; 1024];
                let request_len = stream.read(&mut request).expect("read redirect request");
                assert!(request_len > 0, "redirect request must not be empty");
                write!(
                    stream,
                    "HTTP/1.1 {status_code} {reason}\r\nLocation: http://{target_addr}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .expect("write redirect response");
            });
            let target_server = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_millis(750);
                loop {
                    match target_listener.accept() {
                        Ok((mut stream, _)) => {
                            let mut request = [0_u8; 1024];
                            let request_len =
                                stream.read(&mut request).expect("read redirected request");
                            assert!(request_len > 0, "redirected request must not be empty");
                            stream
                                .write_all(
                                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                )
                                .expect("write target response");
                            return true;
                        }
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            if Instant::now() >= deadline {
                                return false;
                            }
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("target listener failed: {error}"),
                    }
                }
            });
            let response = build_http_client()
                .post(format!("http://{redirect_addr}/query"))
                .body(vec![0x01, 0x02, 0x03])
                .send()
                .expect("redirect response");
            redirect_server.join().expect("redirect server");
            let followed = target_server.join().expect("target server");
            assert_eq!(response.status().as_u16(), status_code);
            assert!(!followed, "one-shot signed body must not be redirected");
        }
    }
    #[test]
    fn owned_http_client_does_not_retry_signed_body_after_server_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener");
        let address = listener.local_addr().expect("test address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("first request");
            let mut request = [0_u8; 1024];
            let request_len = stream.read(&mut request).expect("read first request");
            assert!(request_len > 0, "first request must not be empty");
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("write first response");
            drop(stream);
            listener
                .set_nonblocking(true)
                .expect("nonblocking retry listener");
            let deadline = Instant::now() + Duration::from_millis(750);
            loop {
                match listener.accept() {
                    Ok((_stream, _)) => return true,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return false;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("retry listener failed: {error}"),
                }
            }
        });
        let response = build_http_client()
            .post(format!("http://{address}/transaction"))
            .body(vec![0x01, 0x02, 0x03])
            .send()
            .expect("server response");
        assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            !server.join().expect("test server"),
            "signed body was retried"
        );
    }
    #[test]
    fn send_is_safe_inside_tokio_runtime_multi_thread() {
        let request = DefaultRequestBuilder::new(
            crate::http::Method::GET,
            Url::parse("http://127.0.0.1/status").expect("url"),
        )
        .build()
        .expect("build request");
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = with_send_hook(
            Arc::new(|snapshot| {
                assert_eq!(snapshot.url.as_str(), "http://127.0.0.1/status");
                assert_eq!(snapshot.max_response_bytes, DEFAULT_MAX_RESPONSE_BYTES);
                Response::builder()
                    .status(http::StatusCode::OK)
                    .body(Vec::new())
                    .map_err(Into::into)
            }),
            || rt.block_on(async { request.send() }),
        );
        assert!(result.is_ok());
    }
    #[test]
    fn send_is_safe_inside_current_thread_runtime() {
        let request = DefaultRequestBuilder::new(
            crate::http::Method::GET,
            Url::parse("http://127.0.0.1/status").expect("url"),
        )
        .build()
        .expect("build request");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let result = with_send_hook(
            Arc::new(|snapshot| {
                assert_eq!(snapshot.url.as_str(), "http://127.0.0.1/status");
                assert_eq!(snapshot.max_response_bytes, DEFAULT_MAX_RESPONSE_BYTES);
                Response::builder()
                    .status(http::StatusCode::OK)
                    .body(Vec::new())
                    .map_err(Into::into)
            }),
            || rt.block_on(async { request.send() }),
        );
        assert!(result.is_ok());
    }
    #[test]
    fn builder_timeout_is_forwarded() {
        let timeout = std::time::Duration::from_secs(2);
        let request = DefaultRequestBuilder::new(
            crate::http::Method::GET,
            Url::parse("http://127.0.0.1/status").expect("url"),
        )
        .timeout(timeout)
        .build()
        .expect("build request");
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = with_send_hook(
            Arc::new(move |snapshot| {
                assert_eq!(snapshot.timeout, Some(timeout));
                Response::builder()
                    .status(http::StatusCode::OK)
                    .body(Vec::new())
                    .map_err(Into::into)
            }),
            || rt.block_on(async { request.send() }),
        );
        assert!(result.is_ok());
    }
    #[test]
    fn builder_response_limit_is_forwarded() {
        let request = DefaultRequestBuilder::new(
            crate::http::Method::GET,
            Url::parse("http://127.0.0.1/status").expect("url"),
        )
        .max_response_bytes(4096)
        .build()
        .expect("build request");
        let result = with_send_hook(
            Arc::new(|snapshot| {
                assert_eq!(snapshot.max_response_bytes, 4096);
                Response::builder()
                    .status(http::StatusCode::OK)
                    .body(Vec::new())
                    .map_err(Into::into)
            }),
            || request.send(),
        );
        assert!(result.is_ok());
    }
    #[test]
    fn request_snapshot_preserves_utf8_header_bytes_used_by_account_ids() {
        let account = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
        let request = DefaultRequestBuilder::new(
            crate::http::Method::GET,
            Url::parse("http://127.0.0.1/status").expect("url"),
        )
        .header("x-iroha-account", &account)
        .build()
        .expect("build request");
        let snapshot = request.snapshot();
        assert_eq!(
            snapshot.headers,
            vec![("x-iroha-account".to_owned(), account.to_owned())]
        );
    }
    #[test]
    fn builder_rejects_zero_response_limit() {
        let error = DefaultRequestBuilder::new(
            crate::http::Method::GET,
            Url::parse("http://127.0.0.1/status").expect("url"),
        )
        .max_response_bytes(0)
        .build()
        .expect_err("zero response limit must be rejected");
        assert!(error.to_string().contains("must be positive"));
    }
    #[test]
    fn bounded_response_rejects_zero_limit_without_reading() {
        struct PanicReader;
        impl std::io::Read for PanicReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                panic!("zero response limit must reject before reading")
            }
        }
        let error = read_bounded_response_body(&mut PanicReader, None, 0)
            .expect_err("zero response limit must be rejected");
        assert!(error.to_string().contains("must be positive"));
    }
    #[test]
    fn bounded_response_rejects_advertised_oversize_without_reading() {
        struct PanicReader;
        impl std::io::Read for PanicReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                panic!("oversized Content-Length must reject before reading")
            }
        }
        let error = read_bounded_response_body(&mut PanicReader, Some(9), 8)
            .expect_err("advertised oversized response must reject");
        assert!(error.to_string().contains("Content-Length"));
    }
    #[test]
    fn bounded_response_accepts_exact_limit() {
        for advertised_length in [None, Some(0), Some(1), Some(8)] {
            let mut reader = std::io::Cursor::new(b"12345678");
            let body = read_bounded_response_body(&mut reader, advertised_length, 8)
                .expect("exact-limit response must be accepted");
            assert_eq!(body, b"12345678");
        }
    }
    #[test]
    fn bounded_response_rejects_missing_understated_or_encoded_content_length() {
        // A small advertised length with a larger decoded reader models transparent
        // decompression without coupling this unit test to a particular content codec.
        for advertised_length in [None, Some(0), Some(1), Some(8)] {
            let mut reader = std::io::Cursor::new(b"123456789");
            let error = read_bounded_response_body(&mut reader, advertised_length, 8)
                .expect_err("actual oversized response must reject");
            assert!(error.to_string().contains("body exceeds"));
        }
    }
    #[test]
    fn bounded_response_does_not_preallocate_from_large_content_length() {
        let advertised_length = u64::from(u32::MAX);
        let max_response_bytes = usize::try_from(advertised_length)
            .expect("supported targets represent a u32 response limit");
        assert_eq!(
            response_initial_capacity(Some(advertised_length), max_response_bytes),
            RESPONSE_INITIAL_ALLOCATION_BYTES
        );
        assert_eq!(response_initial_capacity(None, 7), 7);
        let mut reader = std::io::Cursor::new(Vec::<u8>::new());
        let body =
            read_bounded_response_body(&mut reader, Some(advertised_length), max_response_bytes)
                .expect("empty body with an in-range advertised length must be readable");
        assert!(body.is_empty());
    }
    #[test]
    fn bounded_response_never_reserves_beyond_limit() {
        let mut body = Vec::with_capacity(16);
        let original_capacity = body.capacity();
        let error = reserve_response_body_capacity(&mut body, 9, 8)
            .expect_err("capacity above the response limit must be rejected");
        assert!(error.to_string().contains("body exceeds"));
        assert_eq!(body.capacity(), original_capacity);
    }
    #[test]
    fn bounded_response_reads_only_one_sentinel_byte_beyond_limit() {
        #[derive(Default)]
        struct InfiniteReader {
            requested_lengths: Vec<usize>,
        }
        impl std::io::Read for InfiniteReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.requested_lengths.push(buffer.len());
                buffer.fill(0x5a);
                Ok(buffer.len())
            }
        }
        let mut reader = InfiniteReader::default();
        let error = read_bounded_response_body(&mut reader, None, 8)
            .expect_err("an unbounded reader must be rejected after the sentinel byte");
        assert!(error.to_string().contains("body exceeds"));
        assert_eq!(reader.requested_lengths, [8, 1]);
    }
    #[test]
    fn bounded_response_rejects_reader_length_contract_violation_without_panicking() {
        struct MisreportingReader;
        impl std::io::Read for MisreportingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                Ok(buffer.len() + 1)
            }
        }
        let error = read_bounded_response_body(&mut MisreportingReader, None, 8)
            .expect_err("a reader cannot report more bytes than its buffer");
        assert!(error.to_string().contains("reader reported"));
    }
    #[test]
    fn bounded_response_propagates_reader_failure() {
        struct FailingReader;
        impl std::io::Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("adversarial read failure"))
            }
        }
        let error = read_bounded_response_body(&mut FailingReader, None, 8)
            .expect_err("reader failure must reject");
        assert!(error.to_string().contains("Failed to read"));
    }
    #[test]
    fn bounded_response_discards_partial_body_on_reader_failure() {
        struct PartialThenFailingReader {
            first_read: bool,
        }
        impl std::io::Read for PartialThenFailingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.first_read {
                    return Err(std::io::Error::other("adversarial failure after bytes"));
                }
                self.first_read = true;
                buffer[..4].copy_from_slice(b"1234");
                Ok(4)
            }
        }
        let error = read_bounded_response_body(
            &mut PartialThenFailingReader { first_read: false },
            None,
            8,
        )
        .expect_err("partial body followed by a reader failure must reject");
        assert!(format!("{error:#}").contains("adversarial failure after bytes"));
    }
    #[test]
    fn bounded_response_retries_interrupted_reads() {
        struct InterruptedReader {
            state: u8,
        }
        impl std::io::Read for InterruptedReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                match self.state {
                    0 => {
                        self.state = 1;
                        Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
                    }
                    1 => {
                        self.state = 2;
                        buffer[..4].copy_from_slice(b"1234");
                        Ok(4)
                    }
                    _ => Ok(0),
                }
            }
        }
        let body = read_bounded_response_body(&mut InterruptedReader { state: 0 }, None, 8)
            .expect("interrupted reads must be retried");
        assert_eq!(body, b"1234");
    }
}
