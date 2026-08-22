//! Module with general communication primitives like an HTTP request builder.
use core::borrow::Borrow;
use eyre::{Result, eyre};
pub use http::{Method, Response, StatusCode};
use url::Url;
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
