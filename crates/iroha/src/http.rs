//! Module with general communication primitives like an HTTP request builder.
use core::borrow::Borrow;
use eyre::{Result, eyre};
use http::header::{HeaderName, HeaderValue};
pub use http::{Method, Response, StatusCode};
use std::{future::Future, pin::Pin, time::Duration};
use url::Url;

/// Fully prepared HTTP request passed to an injected client transport.
pub struct TransportRequest {
    /// HTTP method.
    pub method: Method,
    /// Absolute request URL.
    pub url: Url,
    /// Ordered HTTP headers.
    pub headers: Vec<(HeaderName, HeaderValue)>,
    /// Request body bytes.
    pub body: Vec<u8>,
    /// Per-request deadline override.
    pub timeout: Option<Duration>,
    /// Maximum decoded response body retained in memory.
    pub max_response_bytes: usize,
    /// Require the transport to bypass proxies for an exact cleartext loopback URL.
    pub direct_loopback: bool,
}

impl std::fmt::Debug for TransportRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TransportRequest")
            .field("method", &self.method)
            .field("url_origin", &self.url.origin().ascii_serialization())
            .field(
                "header_names",
                &self
                    .headers
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>(),
            )
            .field("body_len", &self.body.len())
            .field("timeout", &self.timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("direct_loopback", &self.direct_loopback)
            .finish()
    }
}

/// Future returned by an asynchronous custom HTTP transport.
pub type TransportFuture<'a> = Pin<Box<dyn Future<Output = Result<Response<Vec<u8>>>> + Send + 'a>>;

/// Client-owned HTTP transport boundary.
///
/// Implementations must dispatch each request at most once, respect
/// [`TransportRequest::max_response_bytes`], and must not follow redirects for
/// signed submissions. The SDK shares one implementation among clones of the
/// same client context and never shares it with separately constructed clients.
pub trait HttpTransport: std::fmt::Debug + Send + Sync {
    // TODO: Remove synchronous transport dispatch after the remaining synchronous
    // `client::Client` route methods move behind `iroha::blocking::Client`.
    /// Dispatch one request through the remaining synchronous path.
    ///
    /// # Errors
    /// Returns transport, protocol, or response-bound failures.
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>>;

    /// Dispatch one request asynchronously.
    fn send(&self, request: TransportRequest) -> TransportFuture<'_>;
}

// TODO: Extend the context-owned transport boundary to WebSocket event and block
// streams once their request types carry an injected connector. HTTP requests,
// including asynchronous transaction submission, already use this boundary.
#[doc = include_str!("http_docs/request_builder.md")]
pub trait RequestBuilder {
    /// Create a new builder with specified method and URL. Entrypoint for most client operations.
    #[must_use]
    fn new(method: Method, url: Url) -> Self;
    /// Add multiple query params at once. Uses [`RequestBuilder::param`] for each param.
    #[must_use]
    fn params<P, K, V>(mut self, params: P) -> Self
    where
        P: IntoIterator,
        P::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: ToString,
        Self: Sized,
    {
        for pair in params {
            let (k, v) = pair.borrow();
            self = self.param(k, v);
        }
        self
    }
    /// Add a single query param
    #[must_use]
    fn param<K: AsRef<str>, V: ToString + ?Sized>(self, key: K, value: &V) -> Self;
    /// Add multiple headers at once. Uses [`RequestBuilder::header`] for each param.
    #[must_use]
    fn headers<H: IntoIterator, N: AsRef<str>, V: ToString>(mut self, headers: H) -> Self
    where
        H::Item: Borrow<(N, V)>,
        Self: Sized,
    {
        for pair in headers {
            let (k, v) = pair.borrow();
            self = self.header(k, v);
        }
        self
    }
    /// Add a single header
    #[must_use]
    fn header<N: AsRef<str>, V: ToString + ?Sized>(self, name: N, value: &V) -> Self;
    /// Set request's binary body
    #[must_use]
    fn body(self, data: Vec<u8>) -> Self;
}
/// Generalization of `WebSocket` client's functionality
pub mod ws {
    use super::{RequestBuilder, Result, eyre};
    use url::Url;
    #[doc = include_str!("http_docs/websocket_flow.md")]
    pub mod conn_flow {
        use super::*;
        /// Initial data to initialize connection and acquire handshake. Produced by implementor of [`Init`].
        pub struct InitData<R, E>
        where
            R: RequestBuilder,
            E: Events,
        {
            /// Built HTTP request to init WS connection
            pub req: R,
            /// Should be sent immediately after WS connection establishment
            pub first_message: Vec<u8>,
            /// Handler for the next flow stage - handshake
            pub next: E,
        }
        impl<R, E> InitData<R, E>
        where
            R: RequestBuilder,
            E: Events,
        {
            /// Construct new item.
            pub fn new(req: R, first_message: Vec<u8>, next: E) -> Self {
                Self {
                    req,
                    first_message,
                    next,
                }
            }
        }
        /// Initial flow stage.
        pub trait Init<R: RequestBuilder> {
            /// The next handler
            type Next: Events;
            #[doc = include_str!("http_docs/init_flow.md")]
            fn init(self) -> InitData<R, Self::Next>;
        }
        /// Events flow stage.
        pub trait Events {
            /// Something yielded by the handler
            type Event;
            #[doc = include_str!("http_docs/events_flow.md")]
            fn message(&self, message: Vec<u8>) -> Result<Self::Event>;
        }
    }
    /// Replaces `http(s)://` with `ws(s)://`
    ///
    /// # Errors
    /// Fails if passed URL doesn't have a valid protocol
    pub fn transform_ws_url(mut url: Url) -> Result<Url> {
        match url.scheme() {
            "https" => url.set_scheme("wss").expect("Valid substitution"),
            "http" => url.set_scheme("ws").expect("Valid substitution"),
            _ => {
                return Err(eyre!(
                    "Provided URL scheme is neither `http` nor `https`: {}",
                    url
                ));
            }
        }
        Ok(url)
    }
}
