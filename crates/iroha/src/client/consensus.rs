//! Operator-authorized observations of the node's consensus runtime.

use super::{ACCEPT_NORITO_PREFERRED, Client, OperatorClient, dispatch, join_torii_url};
use crate::{
    Error, Result,
    http::{Method, Response, StatusCode},
};
use iroha_data_model::block::consensus::SumeragiDiagnosticsStatus;
use iroha_torii_shared::route_catalog;

// Preserve the previously supported transport response ceiling explicitly.
const MAX_DIAGNOSTICS_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const DIAGNOSTICS: &str = "sumeragi.diagnostics.read";
const APPLICATION_JSON: &str = "application/json";
const APPLICATION_NORITO: &str = "application/x-norito";

/// Consensus runtime observations available only to an explicit operator.
///
/// Public and account contexts cannot inspect this capability:
///
/// ```compile_fail
/// fn read(client: &iroha::client::Client) { let _ = client.consensus(); }
/// ```
///
/// ```compile_fail
/// fn read(account: &iroha::client::AccountClient) { let _ = account.consensus(); }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Consensus<'a> {
    operator: &'a OperatorClient,
}

impl OperatorClient {
    /// Access consensus diagnostics under this immutable operator identity.
    #[must_use]
    pub const fn consensus(&self) -> Consensus<'_> {
        Consensus { operator: self }
    }
}

impl Consensus<'_> {
    /// Read the node's typed operator and lane diagnostics.
    ///
    /// These observations do not authenticate finality. The route requires the
    /// node's telemetry feature and policy; unavailability is returned directly.
    /// Uses one signed request, without compatibility probes or retries.
    ///
    /// # Errors
    /// Returns structured signing, transport, deadline, HTTP, response-bound or
    /// decoding failures, including invalid `NPoS` diagnostics.
    pub async fn diagnostics(&self) -> Result<SumeragiDiagnosticsStatus> {
        let client = &self.operator.context;
        let request = client
            .identity_signed_request(
                &self.operator.operator_key_pair,
                Method::GET,
                join_torii_url(
                    &client.torii_url,
                    route_catalog::sumeragi::DIAGNOSTICS.path(),
                ),
                Vec::new(),
            )
            .map_err(|error| Error::RequestSigning {
                operation: DIAGNOSTICS,
                details: error.to_string(),
            })?
            .max_response_bytes(MAX_DIAGNOSTICS_RESPONSE_BYTES);
        let response =
            dispatch::send(client, DIAGNOSTICS, request, ACCEPT_NORITO_PREFERRED).await?;
        let diagnostics = decode_response(response)?;
        if client
            .http_transport
            .deadline()
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            return Err(Error::Timeout {
                operation: DIAGNOSTICS,
            });
        }
        Ok(diagnostics)
    }
}

fn decode_response(response: Response<Vec<u8>>) -> Result<SumeragiDiagnosticsStatus> {
    if response.status() != StatusCode::OK {
        return Err(Error::Http {
            operation: DIAGNOSTICS,
            status: response.status().as_u16(),
            retry_after: crate::error::retry_after(response.headers()),
            body: response.into_body(),
        });
    }
    let media = dispatch::media_type(DIAGNOSTICS, &response).map_err(|error| Error::Decode {
        operation: DIAGNOSTICS,
        details: format!("invalid content-type: {error}"),
    })?;
    let wire: SumeragiDiagnosticsStatus = if media.eq_ignore_ascii_case(APPLICATION_NORITO) {
        norito::decode_from_bytes(response.body()).map_err(|error| Error::Decode {
            operation: DIAGNOSTICS,
            details: error.to_string(),
        })?
    } else if media.eq_ignore_ascii_case(APPLICATION_JSON)
        && Client::is_exact_json_content_type(
            response.headers()[http::header::CONTENT_TYPE]
                .to_str()
                .unwrap_or_default(),
        )
    {
        norito::json::from_slice(response.body()).map_err(|error| Error::Decode {
            operation: DIAGNOSTICS,
            details: error.to_string(),
        })?
    } else {
        return Err(Error::Decode {
            operation: DIAGNOSTICS,
            details: "invalid content-type: expected application/x-norito or application/json"
                .to_owned(),
        });
    };
    if let Some(npos) = &wire.npos {
        npos.validate().map_err(|error| Error::Decode {
            operation: DIAGNOSTICS,
            details: format!("Invalid NPoS diagnostics payload: {error}"),
        })?;
    }
    Ok(wire)
}

#[cfg(test)]
#[path = "consensus_tests.rs"]
pub(super) mod tests;
