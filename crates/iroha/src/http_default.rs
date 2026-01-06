//! Defaults for various items used in communication over http(s).
//!
//! These implementations rely on the `reqwest` and `tungstenite` crates and
//! provide a simple, feature-complete transport layer used by the client. The
//! module does not yet expose configuration hooks for alternative backends.
use std::{net::TcpStream, sync::OnceLock, thread};

use eyre::{Error, Result, WrapErr, eyre};
use http::header::{HeaderName, HeaderValue};
use reqwest::blocking::Client as BlockingClient;
pub use tungstenite::{Error as WebSocketError, Message as WebSocketMessage};
use tungstenite::{WebSocket, client::IntoClientRequest, stream::MaybeTlsStream};
use url::Url;

use crate::http::{Method, RequestBuilder, Response};

type Bytes = Vec<u8>;

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
}

#[derive(Debug)]
struct PreparedRequest {
    method: Method,
    url: Url,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Vec<u8>,
    timeout: Option<std::time::Duration>,
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
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        RequestSnapshot {
            method: self.prepared.method.clone(),
            url: self.prepared.url.clone(),
            headers: headers_vec,
            body: self.prepared.body.clone(),
            timeout: self.prepared.timeout,
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
        } = self.prepared;

        let client = http_client();
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

        ClientResponse(response).try_into()
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
        let (stream, _) = tungstenite::connect(self.0)?;
        Ok(stream)
    }

    /// Open [`AsyncWebSocketStream`].
    pub async fn connect_async(self) -> Result<AsyncWebSocketStream> {
        let (stream, _) = tokio_tungstenite::connect_async(self.0).await?;
        Ok(stream)
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
    CLIENT.get_or_init(|| {
        BlockingClient::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to build blocking HTTP client")
    })
}

struct ClientResponse(reqwest::blocking::Response);

impl TryFrom<ClientResponse> for Response<Bytes> {
    type Error = Error;

    fn try_from(response: ClientResponse) -> Result<Self> {
        let ClientResponse(response) = response;
        let status = response.status();
        let headers: Vec<(HeaderName, HeaderValue)> = response
            .headers()
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        let body = response
            .bytes()
            .wrap_err("Failed to get response as bytes")?;

        let mut builder = Response::builder().status(status);
        let headers_map = builder
            .headers_mut()
            .ok_or_else(|| eyre!("Failed to get headers map reference."))?;
        for (key, value) in headers {
            headers_map.insert(key, value);
        }
        builder
            .body(body.to_vec())
            .wrap_err("Failed to construct response bytes body")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

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
}
