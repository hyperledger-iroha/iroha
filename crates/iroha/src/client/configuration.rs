//! Operator-authorized effective configuration reads.

use super::{OperatorClient, dispatch, join_torii_url};
use crate::{Error, Result, http::Method};
use iroha_torii_shared::{configuration::Configuration as NodeConfiguration, uri};

pub(super) const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const GET: &str = "operator.configuration.read";

/// Effective node configuration available only to an explicit operator context.
///
/// Public and account contexts cannot access this capability:
///
/// ```compile_fail
/// fn read_configuration(client: &iroha::client::Client) {
///     let _ = client.configuration();
/// }
/// ```
///
/// ```compile_fail
/// fn read_configuration(account: &iroha::client::AccountClient) {
///     let _ = account.configuration();
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Configuration<'a> {
    operator: &'a OperatorClient,
}

impl OperatorClient {
    /// Access effective configuration using this context's operator authority.
    #[must_use]
    pub const fn configuration(&self) -> Configuration<'_> {
        Configuration { operator: self }
    }
}

impl Configuration<'_> {
    /// Read the node's effective configuration as the canonical shared JSON DTO.
    ///
    /// The request is signed by this operator for the exact network and request
    /// target. This JSON-only route uses the context's deadline and an 8-MiB
    /// response bound. It performs one request without probing or retrying.
    ///
    /// # Errors
    /// Returns structured signing, transport, deadline, HTTP, response-bound or
    /// decoding errors.
    pub async fn get(&self) -> Result<NodeConfiguration> {
        let client = &self.operator.context;
        let builder = client
            .identity_signed_request(
                &self.operator.operator_key_pair,
                Method::GET,
                join_torii_url(&client.torii_url, uri::CONFIGURATION),
                Vec::new(),
            )
            .map_err(|error| Error::RequestSigning {
                operation: GET,
                details: error.to_string(),
            })?
            .max_response_bytes(MAX_RESPONSE_BYTES);
        let response = dispatch::send(client, GET, builder, "application/json").await?;
        if response.status() != http::StatusCode::OK {
            return Err(Error::Http {
                operation: GET,
                status: response.status().as_u16(),
                retry_after: crate::error::retry_after(response.headers()),
                body: response.into_body(),
            });
        }
        if !dispatch::media_type(GET, &response)?.eq_ignore_ascii_case("application/json") {
            return Err(Error::Decode {
                operation: GET,
                details: "expected application/json configuration response".to_owned(),
            });
        }
        norito::json::from_slice(response.body()).map_err(|error| Error::Decode {
            operation: GET,
            details: error.to_string(),
        })
    }
}
