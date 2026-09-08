//! Shared asynchronous dispatch, deadlines and errors for capability operations.

use super::{Client, DefaultRequestBuilder};
use crate::{Error, Result, TransportErrorKind, http::Response};

pub(super) fn transport_error(operation: &'static str, error: &eyre::Report) -> Error {
    if let Some(typed) = error.downcast_ref::<Error>() {
        return typed.clone();
    }
    let io_kind = error.chain().find_map(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind)
    });
    if error
        .downcast_ref::<reqwest::Error>()
        .is_some_and(reqwest::Error::is_timeout)
        || io_kind == Some(std::io::ErrorKind::TimedOut)
    {
        return Error::Timeout { operation };
    }
    Error::Transport {
        operation,
        kind: io_kind.map_or(TransportErrorKind::Other, TransportErrorKind::Io),
        details: error.to_string(),
    }
}

pub(super) async fn send(
    client: &Client,
    operation: &'static str,
    builder: DefaultRequestBuilder,
    accept: &'static str,
) -> Result<Response<Vec<u8>>> {
    let mut builder = builder.replace_header(http::header::ACCEPT, accept);
    if !client.torii_request_timeout.is_zero() {
        builder = builder.timeout(client.torii_request_timeout);
    }
    let request = builder.build().map_err(|error| Error::InvalidRequest {
        operation,
        details: error.to_string(),
    })?;
    let response = if client.torii_request_timeout.is_zero() {
        client.dispatch_request(request).await
    } else {
        tokio::time::timeout(
            client.torii_request_timeout,
            client.dispatch_request(request),
        )
        .await
        .map_err(|_| Error::Timeout { operation })?
    };
    response.map_err(|error| transport_error(operation, &error))
}

pub(super) fn media_type<'a>(
    operation: &'static str,
    response: &'a Response<Vec<u8>>,
) -> Result<&'a str> {
    let mut values = response
        .headers()
        .get_all(http::header::CONTENT_TYPE)
        .iter();
    let value = values.next().and_then(|value| value.to_str().ok());
    if values.next().is_some() {
        return Err(Error::Decode {
            operation,
            details: "expected exactly one response content type".to_owned(),
        });
    }
    value
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains(','))
        .ok_or_else(|| Error::Decode {
            operation,
            details: "expected exactly one valid response content type".to_owned(),
        })
}
