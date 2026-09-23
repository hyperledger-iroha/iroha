//! Public asynchronous data-availability reads on one immutable client context.

use super::{AccountClient, Client, dispatch, join_torii_url};
use crate::{
    Error, Result,
    http::{Method, RequestBuilder as _},
};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    da::{
        commitment::{DaCommitmentProof, DaProofPolicyBundle},
        pin_intent::DaPinIntentProof,
        types::StorageTicketId,
    },
};
use iroha_torii_shared::{
    da::{
        DA_QUERY_REQUEST_MAX_BYTES, DaCommitmentListRequest, DaCommitmentListResponse,
        DaCommitmentProofRequest, DaCommitmentProofResponse, DaCommitmentVerifyResponse,
        DaManifestResponse, DaPinIntentListRequest, DaPinIntentListResponse,
        DaPinIntentQueryRequest, DaPinIntentVerifyResponse, DaQueryValidationError,
    },
    route_catalog::{RouteDescriptor, data_availability},
};
use norito::json::{JsonDeserialize, JsonSerialize};

mod validation;

pub(super) const MAX_MANIFEST_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MANIFEST: &str = "data_availability.manifest.read";

/// Public data-availability operations using the context's asynchronous transport.
#[derive(Clone, Copy, Debug)]
pub struct DataAvailability<'a> {
    client: &'a Client,
}

/// DA proof operations authorized by an immutable account signing context.
///
/// Public contexts cannot request authenticated proofs:
/// ```compile_fail
/// async fn prove(client: &iroha::client::Client, request: &iroha::da::DaCommitmentProofRequest) {
///     client.da().prove_commitment(request).await;
/// }
/// ```
/// Operator credentials do not grant account authority:
/// ```compile_fail
/// fn prove(operator: &iroha::client::OperatorClient) { operator.da(); }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct AccountDataAvailability<'a> {
    account: &'a AccountClient,
}

impl AccountClient {
    /// Access DA proof operations using this account's canonical request signature.
    #[must_use]
    pub const fn da(&self) -> AccountDataAvailability<'_> {
        AccountDataAvailability { account: self }
    }
}

impl Client {
    /// Access public data-availability records.
    #[must_use]
    pub const fn da(&self) -> DataAvailability<'_> {
        DataAvailability { client: self }
    }
}

impl DataAvailability<'_> {
    /// Fetch the canonical manifest response bound to one storage ticket.
    ///
    /// # Errors
    /// Returns a structured transport, deadline, response-bound, HTTP, decoding or
    /// ticket-binding error. The operation dispatches exactly one request.
    pub async fn manifest(&self, ticket: &StorageTicketId) -> Result<DaManifestResponse> {
        let ticket_hex = hex::encode(ticket.as_bytes());
        let path = data_availability::MANIFEST
            .path()
            .replace("{ticket}", &ticket_hex);
        let manifest: DaManifestResponse = exchange(
            self.client,
            data_availability::MANIFEST,
            self.client.request_without_canonical_account_auth(
                Method::GET,
                join_torii_url(&self.client.torii_url, &path),
            ),
        )
        .await?;
        if manifest.storage_ticket != ticket_hex {
            return Err(Error::ResponseBinding {
                operation: MANIFEST,
                field: "storage_ticket",
            });
        }
        Ok(manifest)
    }

    /// Discover the currently active DA proof policies.
    ///
    /// Historical verification uses the referenced block's authenticated policy sidecar.
    /// # Errors
    /// Returns structured transport, deadline, HTTP, response-bound or decoding errors.
    pub async fn proof_policies(&self) -> Result<DaProofPolicyBundle> {
        let route = data_availability::PROOF_POLICIES;
        let request = self.client.request_without_canonical_account_auth(
            Method::GET,
            join_torii_url(&self.client.torii_url, route.path()),
        );
        exchange(self.client, route, request).await
    }

    /// Read a bounded page of commitments in canonical key order.
    ///
    /// # Errors
    /// Returns request-validation, transport, HTTP, decoding or response-binding errors.
    pub async fn commitments(
        &self,
        query: &DaCommitmentListRequest,
    ) -> Result<DaCommitmentListResponse> {
        let route = data_availability::COMMITMENTS;
        valid_request(route, query.validate())?;
        let response = public_query(self.client, route, query).await?;
        validation::commitments(query, &response).map_err(|field| binding(route, field))?;
        Ok(response)
    }

    /// Read a bounded page of pin intents in canonical block-location order.
    ///
    /// # Errors
    /// Returns request-validation, transport, HTTP, decoding or response-binding errors.
    pub async fn pin_intents(
        &self,
        query: &DaPinIntentListRequest,
    ) -> Result<DaPinIntentListResponse> {
        let route = data_availability::PIN_INTENTS;
        valid_request(route, query.validate())?;
        let response = public_query(self.client, route, query).await?;
        validation::pin_intents(query, &response, &self.client.network_id)
            .map_err(|field| binding(route, field))?;
        Ok(response)
    }
}

impl AccountDataAvailability<'_> {
    /// Request a commitment membership proof matching every supplied selector.
    ///
    /// A returned proof still requires a trusted block header to establish finality.
    /// # Errors
    /// Returns validation, signing, transport, decoding or response-binding errors.
    pub async fn prove_commitment(
        &self,
        query: &DaCommitmentProofRequest,
    ) -> Result<Option<DaCommitmentProofResponse>> {
        let route = data_availability::COMMITMENTS_PROVE;
        valid_request(route, query.validate())?;
        let response: Option<DaCommitmentProofResponse> = self.query(route, query).await?;
        if let Some(response) = &response {
            validation::commitment_proof(query, &response.proof)
                .map_err(|field| binding(route, field))?;
        }
        Ok(response)
    }

    /// Ask Torii to verify a commitment proof against its committed block.
    ///
    /// The returned boolean is a remote verification result, not local finality evidence.
    /// # Errors
    /// Returns signing, transport, decoding or contradictory-response errors.
    pub async fn verify_commitment(
        &self,
        proof: &DaCommitmentProof,
    ) -> Result<DaCommitmentVerifyResponse> {
        let route = data_availability::COMMITMENTS_VERIFY;
        let response: DaCommitmentVerifyResponse = self.query(route, proof).await?;
        validate_verification(route, response.valid, response.error.as_deref())?;
        Ok(response)
    }

    /// Request a pin-intent proof matching every supplied selector and this network.
    ///
    /// A returned proof still requires a trusted block header to establish finality.
    /// # Errors
    /// Returns validation, signing, transport, decoding or response-binding errors.
    pub async fn prove_pin_intent(
        &self,
        query: &DaPinIntentQueryRequest,
    ) -> Result<Option<DaPinIntentProof>> {
        let route = data_availability::PIN_INTENTS_PROVE;
        valid_request(route, query.validate())?;
        let response: Option<DaPinIntentProof> = self.query(route, query).await?;
        if let Some(response) = &response {
            validation::pin_proof(query, response, &self.account.context.network_id)
                .map_err(|field| binding(route, field))?;
        }
        Ok(response)
    }

    /// Ask Torii to verify a pin-intent proof against its committed block.
    ///
    /// # Errors
    /// Returns signing, transport, decoding or contradictory-response errors.
    pub async fn verify_pin_intent(
        &self,
        proof: &DaPinIntentProof,
    ) -> Result<DaPinIntentVerifyResponse> {
        let route = data_availability::PIN_INTENTS_VERIFY;
        let response: DaPinIntentVerifyResponse = self.query(route, proof).await?;
        validate_verification(route, response.valid, response.error.as_deref())?;
        Ok(response)
    }

    async fn query<Q: JsonSerialize + Sync + ?Sized, R: JsonDeserialize>(
        &self,
        route: RouteDescriptor,
        query: &Q,
    ) -> Result<R> {
        let client = &self.account.context;
        self.account
            .ensure_direct_signing_capability()
            .map_err(|error| Error::InvalidRequest {
                operation: route.stable_route_id(),
                details: error.to_string(),
            })?;
        if client.account.controller.single_signatory() != Some(client.key_pair.public_key())
            || client
                .headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("X-Iroha-Witness"))
        {
            return Err(Error::InvalidRequest {
                operation: route.stable_route_id(),
                details: "DA proofs require a direct account signer without witness headers"
                    .to_owned(),
            });
        }
        let request = {
            let _format = ChainDiscriminantGuard::enter(client.account_chain_discriminant);
            let body = encode_query(route, query)?;
            client
                .account_signed_request(
                    Method::POST,
                    join_torii_url(&client.torii_url, route.path()),
                    body,
                )
                .map_err(|error| Error::RequestSigning {
                    operation: route.stable_route_id(),
                    details: error.to_string(),
                })?
                .replace_header(http::header::CONTENT_TYPE, "application/json")
        };
        exchange(client, route, request).await
    }
}

fn encode_query<Q: JsonSerialize + ?Sized>(route: RouteDescriptor, query: &Q) -> Result<Vec<u8>> {
    norito::json::to_json_bounded_boxed(query, DA_QUERY_REQUEST_MAX_BYTES)
        .map(<[u8]>::into_vec)
        .map_err(|error| Error::InvalidRequest {
            operation: route.stable_route_id(),
            details: error.to_string(),
        })
}

async fn public_query<Q: JsonSerialize + Sync + ?Sized, R: JsonDeserialize>(
    client: &Client,
    route: RouteDescriptor,
    query: &Q,
) -> Result<R> {
    let request = {
        let _format = ChainDiscriminantGuard::enter(client.account_chain_discriminant);
        client
            .request_without_canonical_account_auth(
                Method::POST,
                join_torii_url(&client.torii_url, route.path()),
            )
            .body(encode_query(route, query)?)
            .replace_header(http::header::CONTENT_TYPE, "application/json")
    };
    exchange(client, route, request).await
}

async fn exchange<R: JsonDeserialize>(
    client: &Client,
    route: RouteDescriptor,
    request: crate::http_default::DefaultRequestBuilder,
) -> Result<R> {
    let operation = route.stable_route_id();
    let response = dispatch::send(
        client,
        operation,
        request.max_response_bytes(MAX_MANIFEST_RESPONSE_BYTES),
        "application/json",
    )
    .await?;
    if response.status() != http::StatusCode::OK {
        return Err(Error::Http {
            operation,
            status: response.status().as_u16(),
            retry_after: crate::error::retry_after(response.headers()),
            body: response.into_body(),
        });
    }
    if !dispatch::media_type(operation, &response)?.eq_ignore_ascii_case("application/json") {
        return Err(Error::Decode {
            operation,
            details: "expected application/json DA response".to_owned(),
        });
    }
    let _format = ChainDiscriminantGuard::enter(client.account_chain_discriminant);
    norito::json::from_slice(response.body()).map_err(|error| Error::Decode {
        operation,
        details: error.to_string(),
    })
}

fn valid_request(
    route: RouteDescriptor,
    result: core::result::Result<(), DaQueryValidationError>,
) -> Result<()> {
    result.map_err(|error| Error::InvalidRequest {
        operation: route.stable_route_id(),
        details: error.to_string(),
    })
}

fn binding(route: RouteDescriptor, field: &'static str) -> Error {
    Error::ResponseBinding {
        operation: route.stable_route_id(),
        field,
    }
}

fn validate_verification(route: RouteDescriptor, valid: bool, error: Option<&str>) -> Result<()> {
    if valid != error.is_none() {
        return Err(binding(route, "valid/error"));
    }
    Ok(())
}
