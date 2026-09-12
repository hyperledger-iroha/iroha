//! Public asynchronous node diagnostics on one immutable client context.

use super::{Client, WireFormatPreference, dispatch, join_torii_url};
use crate::{
    Error, Result,
    http::{Method, Response, StatusCode},
};
use iroha_torii_shared::{status::Status as NodeStatus, uri};

pub(super) const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_VERSION_BYTES: usize = 16 * 1024;
const GET: &str = "diagnostic.status";
const VERSION: &str = "core.api_version";

/// Public node status and protocol-version reads.
#[derive(Clone, Copy, Debug)]
pub struct Status<'a> {
    client: &'a Client,
}

impl Client {
    /// Access public node diagnostics through the context's asynchronous transport.
    #[must_use]
    pub const fn status(&self) -> Status<'_> {
        Status { client: self }
    }
}

impl Status<'_> {
    /// Read one negotiated node status document.
    ///
    /// # Errors
    /// Returns structured transport, deadline, response-bound, HTTP or decode errors.
    pub async fn get(&self) -> Result<NodeStatus> {
        let response = dispatch::send(
            self.client,
            GET,
            self.client
                .default_request(
                    Method::GET,
                    join_torii_url(&self.client.torii_url, uri::STATUS),
                )
                .max_response_bytes(MAX_RESPONSE_BYTES),
            self.client.wire_format_preference.accept_header(),
        )
        .await?;
        decode_response(response, self.client.wire_format_preference)
    }

    /// Read the node's active API version from its canonical text endpoint.
    ///
    /// # Errors
    /// Returns structured dispatch errors or rejects an invalid text response.
    pub async fn version(&self) -> Result<String> {
        let response = dispatch::send(
            self.client,
            VERSION,
            self.client
                .default_request(
                    Method::GET,
                    join_torii_url(&self.client.torii_url, uri::API_VERSION),
                )
                .max_response_bytes(MAX_VERSION_BYTES),
            // Public routes negotiate canonical typed errors before dispatch;
            // the version endpoint's successful representation remains text.
            "text/plain, application/json",
        )
        .await?;
        if response.status() != StatusCode::OK {
            return Err(Error::Http {
                operation: VERSION,
                status: response.status().as_u16(),
                retry_after: crate::error::retry_after(response.headers()),
                body: response.into_body(),
            });
        }
        if !dispatch::media_type(VERSION, &response)?.eq_ignore_ascii_case("text/plain") {
            return Err(decode_error(
                VERSION,
                "expected text/plain version response",
            ));
        }
        let version = std::str::from_utf8(response.body())
            .map_err(|error| decode_error(VERSION, error))?
            .trim();
        if version.is_empty() {
            return Err(decode_error(VERSION, "server version response was empty"));
        }
        Ok(version.to_owned())
    }
}

fn decode_error(operation: &'static str, details: impl std::fmt::Display) -> Error {
    Error::Decode {
        operation,
        details: details.to_string(),
    }
}

pub(super) fn decode_response(
    response: Response<Vec<u8>>,
    preference: WireFormatPreference,
) -> Result<NodeStatus> {
    if response.status() != StatusCode::OK {
        return Err(Error::Http {
            operation: GET,
            status: response.status().as_u16(),
            retry_after: crate::error::retry_after(response.headers()),
            body: response.into_body(),
        });
    }
    let media_type = dispatch::media_type(GET, &response)?;
    let is_json = media_type.eq_ignore_ascii_case("application/json");
    let is_norito = media_type.eq_ignore_ascii_case("application/x-norito");
    match preference {
        WireFormatPreference::NoritoOnly if !is_norito => {
            return Err(decode_error(GET, "status response violates NoritoOnly"));
        }
        WireFormatPreference::JsonOnly if !is_json => {
            return Err(decode_error(GET, "status response violates JsonOnly"));
        }
        _ => {}
    }
    if is_json {
        norito::json::from_slice(response.body()).map_err(|error| decode_error(GET, error))
    } else if is_norito {
        norito::decode_from_bytes(response.body()).map_err(|error| {
            decode_error(
                GET,
                format!("failed to decode status Norito payload: {error}"),
            )
        })
    } else {
        Err(decode_error(GET, "expected JSON or Norito status response"))
    }
}
