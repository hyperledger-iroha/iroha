//! Module with general communication primitives like an HTTP request builder.

use core::borrow::Borrow;

pub use http::{Method, Response, StatusCode};
use url::Url;

/// General HTTP request builder.
///
/// To use custom builder with client, you need to implement this trait for some type and pass it
/// to the client that will fill it with data.
///
/// The order of builder methods invocation is not strict. There is no guarantee that builder user calls
/// all methods. Only [`RequestBuilder::new`] is the required one.
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
