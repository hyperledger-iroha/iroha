//! Defaults for various items used in communication over http(s).

use attohttpc::{
    body as atto_body, RequestBuilder as AttoHttpRequestBuilder, Response as AttoHttpResponse,
};
use eyre::{eyre, Error, Result, WrapErr};
use http::header::HeaderName;
use url::Url;

use crate::http::{Method, RequestBuilder, Response};

type Bytes = Vec<u8>;
type AttoHttpRequestBuilderWithBytes = AttoHttpRequestBuilder<atto_body::Bytes<Bytes>>;

fn header_name_from_str(str: &str) -> Result<HeaderName> {
    str.parse::<HeaderName>()
        .wrap_err_with(|| format!("Failed to parse header name {str}"))
}

/// Default request builder implemented on top of `attohttpc` crate.
#[derive(Debug)]
pub struct DefaultRequestBuilder {
    inner: Result<AttoHttpRequestBuilder>,
    body: Option<Vec<u8>>,
}

impl DefaultRequestBuilder {
    /// Apply `.and_then()` semantics to the inner `Result` with underlying request builder.
    fn and_then<F>(self, fun: F) -> Self
    where
        F: FnOnce(AttoHttpRequestBuilder) -> Result<AttoHttpRequestBuilder>,
    {
        Self {
            inner: self.inner.and_then(fun),
            ..self
        }
    }

    /// Build request by consuming self.
    pub fn build(self) -> Result<DefaultRequest> {
        self.inner
            .map(|b| DefaultRequest(b.bytes(self.body.map_or_else(Vec::new, |vec| vec))))
    }
}

/// Request built by [`DefaultRequestBuilder`].
#[derive(Debug)]
pub struct DefaultRequest(AttoHttpRequestBuilderWithBytes);

impl DefaultRequest {
    /// Sends itself and returns byte response
    ///
    /// # Errors
    /// Fails if request building and sending fails or response transformation fails
    pub fn send(mut self) -> Result<Response<Bytes>> {
        let (method, url) = {
            let inspect = self.0.inspect();
            (inspect.method().clone(), inspect.url().clone())
        };

        let response = self
            .0
            .send()
            .wrap_err_with(|| format!("Failed to send http {method} request to {url}"))?;

        ClientResponse(response).try_into()
    }
}

impl RequestBuilder for DefaultRequestBuilder {
    fn new(method: Method, url: Url) -> Self {
        Self {
            inner: Ok(AttoHttpRequestBuilder::new(method, url)),
            body: None,
        }
    }

    fn header<K: AsRef<str>, V: ToString + ?Sized>(self, key: K, value: &V) -> Self {
        self.and_then(|builder| {
            Ok(builder.header(header_name_from_str(key.as_ref())?, value.to_string()))
        })
    }

    fn param<K: AsRef<str>, V: ToString + ?Sized>(self, key: K, value: &V) -> Self {
        self.and_then(|b| Ok(b.param(key, value.to_string())))
    }

    fn body(self, data: Vec<u8>) -> Self {
        Self {
            body: Some(data),
            ..self
        }
    }
}

struct ClientResponse(AttoHttpResponse);

impl TryFrom<ClientResponse> for Response<Bytes> {
    type Error = Error;

    fn try_from(response: ClientResponse) -> Result<Self> {
        let ClientResponse(response) = response;
        let mut builder = Response::builder().status(response.status());
        let headers = builder
            .headers_mut()
            .ok_or_else(|| eyre!("Failed to get headers map reference."))?;
        for (key, value) in response.headers() {
            headers.insert(key, value.clone());
        }
        response
            .bytes()
            .wrap_err("Failed to get response as bytes")
            .and_then(|bytes| {
                builder
                    .body(bytes)
                    .wrap_err("Failed to construct response bytes body")
            })
    }
}
