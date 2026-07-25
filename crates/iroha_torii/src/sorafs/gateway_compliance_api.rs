//! Authenticated V1 control surface for the durable gateway-compliance controller.
//!
//! The HTTP boundary owns no feed, ACME, or signing credentials. Every request
//! is bound to the exact method, URI, and canonical JSON bytes through Torii's
//! canonical account-signature authentication, and the caller must hold the
//! governed `sorafs_gateway_compliance_operator` role. Catalog, gateway
//! acknowledgement, and rollback signatures are verified again by the
//! controller before any durable transition commits.

#![cfg(feature = "app_api")]

use std::{
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{
        HeaderMap, HeaderValue, Method, StatusCode, Uri,
        header::CACHE_CONTROL,
    },
    response::{IntoResponse, Response},
};
use iroha_data_model::role::RoleId;
use iroha_logger::warn;
use norito::derive::JsonSerialize;
use sha2::{Digest as _, Sha256};

use super::gateway::{
    GatewayComplianceAcknowledgementV1, GatewayComplianceCatalogV1,
    GatewayComplianceCheckpointV1, GatewayComplianceController, GatewayComplianceError,
    GatewayComplianceHistoryRecordV1, GatewayComplianceRollbackV1,
    MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
};
use crate::{JsonBody, SharedAppState};

const GATEWAY_COMPLIANCE_OPERATOR_ROLE: &str = "sorafs_gateway_compliance_operator";
const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
const IDEMPOTENCY_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.gateway.compliance.idempotency.v1";
static GATEWAY_COMPLIANCE_OPERATOR_ROLE_ID: LazyLock<RoleId> = LazyLock::new(|| {
    GATEWAY_COMPLIANCE_OPERATOR_ROLE
        .parse()
        .expect("SoraFS gateway compliance operator role id is valid")
});

#[derive(Debug, JsonSerialize)]
struct GatewayComplianceActionResponseV1 {
    schema: String,
    action: String,
    catalog_digest_hex: String,
    idempotency_key: String,
    operation_timestamp_unix: u64,
}

#[derive(Debug, JsonSerialize)]
struct GatewayComplianceCatalogStatusV1 {
    digest_hex: String,
    sequence: u64,
    generated_at_unix: u64,
    valid_until_unix: u64,
}

#[derive(Debug, JsonSerialize)]
struct GatewayComplianceLatestActionStatusV1 {
    operation_id_hex: String,
    action: String,
    previous_serving_digest_hex: Option<String>,
    serving_digest_hex: String,
    recorded_at_unix: u64,
    reason_code: String,
}

#[derive(Debug, JsonSerialize)]
struct GatewayComplianceStatusResponseV1 {
    schema: String,
    checkpoint_version: u8,
    policy_digest_hex: String,
    observed_at_unix: u64,
    serving_ready: bool,
    chain_head: Option<GatewayComplianceCatalogStatusV1>,
    serving: Option<GatewayComplianceCatalogStatusV1>,
    previous_serving: Option<GatewayComplianceCatalogStatusV1>,
    candidate: Option<GatewayComplianceCatalogStatusV1>,
    acknowledgement_count: u64,
    accepted_acknowledgement_count: u64,
    rejected_acknowledgement_count: u64,
    history_count: u64,
    latest_action: Option<GatewayComplianceLatestActionStatusV1>,
}

#[derive(Debug, JsonSerialize)]
struct GatewayComplianceErrorResponseV1 {
    schema: String,
    code: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GatewayCompliancePromoteExpectationV1 {
    catalog_digest: [u8; 32],
    sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GatewayComplianceMutationBindingV1 {
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
}

/// Fetch one configured feed through the runtime-injected authenticated
/// address-pinned transport. The returned document is normalized and bounded;
/// this route never promotes it or signs a catalog.
pub(crate) async fn handle_get_sorafs_gateway_compliance_feed(
    State(state): State<SharedAppState>,
    Path(feed_id): Path<String>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
) -> Response {
    if let Err(response) = authorize_gateway_compliance_request(
        &state,
        &headers,
        &method,
        &uri,
        &[],
    ) {
        return response;
    }
    let (controller, transport) = match gateway_compliance_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return response,
    };
    match controller.fetch_feed(&feed_id, transport.as_ref()) {
        Ok(feed) => no_store_response(JsonBody(feed).into_response()),
        Err(error) => gateway_compliance_error_response(error),
    }
}

/// Return a bounded, payload-free projection of the durable controller
/// checkpoint. Catalog rules, signatures, signer identities, feed paths, and
/// acknowledgement bodies never cross this boundary.
pub(crate) async fn handle_get_sorafs_gateway_compliance_status(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
) -> Response {
    if let Err(response) = authorize_gateway_compliance_request(
        &state,
        &headers,
        &method,
        &uri,
        &[],
    ) {
        return response;
    }
    let controller = match gateway_compliance_controller(&state) {
        Ok(controller) => controller,
        Err(response) => return response,
    };
    let observed_at_unix = match current_unix_second() {
        Ok(now) => now,
        Err(response) => return response,
    };
    match controller.checkpoint() {
        Ok(checkpoint) => match status_response(&checkpoint, observed_at_unix) {
            Ok(status) => no_store_response(JsonBody(status).into_response()),
            Err(error) => gateway_compliance_error_response(error),
        },
        Err(error) => gateway_compliance_error_response(error),
    }
}

/// Durably stage one exact canonical threshold-signed catalog.
pub(crate) async fn handle_post_sorafs_gateway_compliance_stage(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    body: Bytes,
) -> Response {
    if let Err(response) =
        authorize_gateway_compliance_request(&state, &headers, &method, &uri, &body)
    {
        return response;
    }
    if let Err(response) = require_mutation_uri_without_query(&uri) {
        return response;
    }
    if let Err(response) = validate_gateway_compliance_body_headers(&headers, &body) {
        return response;
    }
    let catalog = match decode_canonical_catalog(&body) {
        Ok(catalog) => catalog,
        Err(response) => return response,
    };
    let digest = match catalog.payload.catalog_digest() {
        Ok(digest) => digest,
        Err(error) => return gateway_compliance_error_response(error),
    };
    let binding =
        match require_request_idempotency_binding(&headers, "stage", &uri, &body) {
            Ok(binding) => binding,
            Err(response) => return response,
        };
    let operation_timestamp_unix = catalog.payload.generated_at_unix;
    let controller = match gateway_compliance_controller(&state) {
        Ok(controller) => controller,
        Err(response) => return response,
    };
    let observed_at_unix = match current_unix_second() {
        Ok(now) => now,
        Err(response) => return response,
    };
    match controller.stage_catalog(catalog, observed_at_unix) {
        Ok(digest) => action_response(
            StatusCode::ACCEPTED,
            "stage",
            digest,
            binding.idempotency_key,
            operation_timestamp_unix,
        ),
        Err(GatewayComplianceError::InvalidPredecessor) => {
            match catalog_transition_already_committed(controller, digest) {
                Ok(true) => action_response(
                    StatusCode::ACCEPTED,
                    "stage",
                    digest,
                    binding.idempotency_key,
                    operation_timestamp_unix,
                ),
                Ok(false) => {
                    gateway_compliance_error_response(GatewayComplianceError::InvalidPredecessor)
                }
                Err(error) => gateway_compliance_error_response(error),
            }
        }
        Err(error) => gateway_compliance_error_response(error),
    }
}

/// Durably record one exact canonical signed regional-gateway acknowledgement.
pub(crate) async fn handle_post_sorafs_gateway_compliance_acknowledge(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    body: Bytes,
) -> Response {
    if let Err(response) =
        authorize_gateway_compliance_request(&state, &headers, &method, &uri, &body)
    {
        return response;
    }
    if let Err(response) = require_mutation_uri_without_query(&uri) {
        return response;
    }
    if let Err(response) = validate_gateway_compliance_body_headers(&headers, &body) {
        return response;
    }
    let acknowledgement = match decode_canonical_acknowledgement(&body) {
        Ok(acknowledgement) => acknowledgement,
        Err(response) => return response,
    };
    let binding =
        match require_request_idempotency_binding(&headers, "acknowledge", &uri, &body) {
            Ok(binding) => binding,
            Err(response) => return response,
        };
    let digest = acknowledgement.payload.catalog_digest;
    let operation_timestamp_unix = acknowledgement.payload.observed_at_unix;
    let controller = match gateway_compliance_controller(&state) {
        Ok(controller) => controller,
        Err(response) => return response,
    };
    let observed_at_unix = match current_unix_second() {
        Ok(now) => now,
        Err(response) => return response,
    };
    match controller.acknowledge(acknowledgement, observed_at_unix) {
        Ok(()) => action_response(
            StatusCode::ACCEPTED,
            "acknowledge",
            digest,
            binding.idempotency_key,
            operation_timestamp_unix,
        ),
        Err(error) => gateway_compliance_error_response(error),
    }
}

/// Atomically promote the staged catalog after the signed regional-gateway
/// acknowledgement quorum has committed.
pub(crate) async fn handle_post_sorafs_gateway_compliance_promote(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    body: Bytes,
) -> Response {
    if let Err(response) =
        authorize_gateway_compliance_request(&state, &headers, &method, &uri, &body)
    {
        return response;
    }
    if !body.is_empty() {
        return gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "non_empty_promote_body",
            "gateway compliance promotion requires an empty signed request body",
        );
    }
    let expectation = match decode_canonical_promote_expectation(&uri) {
        Ok(expectation) => expectation,
        Err(response) => return response,
    };
    let binding =
        match require_request_idempotency_binding(&headers, "promote", &uri, &body) {
            Ok(binding) => binding,
            Err(response) => return response,
        };
    let controller = match gateway_compliance_controller(&state) {
        Ok(controller) => controller,
        Err(response) => return response,
    };
    match recover_or_validate_promotion(controller, expectation) {
        Ok(Some(recorded_at_unix)) => {
            return action_response(
                StatusCode::OK,
                "promote",
                expectation.catalog_digest,
                binding.idempotency_key,
                recorded_at_unix,
            );
        }
        Ok(None) => {}
        Err(response) => return response,
    }
    let observed_at_unix = match current_unix_second() {
        Ok(now) => now,
        Err(response) => return response,
    };
    match controller.promote(observed_at_unix) {
        Ok(digest) if digest == expectation.catalog_digest => action_response(
            StatusCode::OK,
            "promote",
            digest,
            binding.idempotency_key,
            observed_at_unix,
        ),
        Ok(_) => gateway_compliance_request_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "controller_state_changed",
            "the staged gateway compliance catalog changed during promotion",
        ),
        Err(GatewayComplianceError::NoStagedCatalog) => {
            match recover_or_validate_promotion(controller, expectation) {
                Ok(Some(recorded_at_unix)) => action_response(
                    StatusCode::OK,
                    "promote",
                    expectation.catalog_digest,
                    binding.idempotency_key,
                    recorded_at_unix,
                ),
                Ok(None) => gateway_compliance_error_response(
                    GatewayComplianceError::NoStagedCatalog,
                ),
                Err(response) => response,
            }
        }
        Err(error) => gateway_compliance_error_response(error),
    }
}

/// Atomically roll the serving pointer back to the last-known-good catalog
/// after verifying the exact threshold-signed rollback authorization.
pub(crate) async fn handle_post_sorafs_gateway_compliance_rollback(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    body: Bytes,
) -> Response {
    if let Err(response) =
        authorize_gateway_compliance_request(&state, &headers, &method, &uri, &body)
    {
        return response;
    }
    if let Err(response) = require_mutation_uri_without_query(&uri) {
        return response;
    }
    if let Err(response) = validate_gateway_compliance_body_headers(&headers, &body) {
        return response;
    }
    let rollback = match decode_canonical_rollback(&body) {
        Ok(rollback) => rollback,
        Err(response) => return response,
    };
    let binding = match require_operation_idempotency_binding(
        &headers,
        rollback.payload.operation_id,
        "rollback",
        &uri,
        &body,
    ) {
            Ok(binding) => binding,
            Err(response) => return response,
        };
    let controller = match gateway_compliance_controller(&state) {
        Ok(controller) => controller,
        Err(response) => return response,
    };
    let observed_at_unix = match current_unix_second() {
        Ok(now) => now,
        Err(response) => return response,
    };
    match controller.rollback(&rollback, observed_at_unix) {
        Ok(digest) => action_response(
            StatusCode::OK,
            "rollback",
            digest,
            binding.idempotency_key,
            observed_at_unix,
        ),
        Err(GatewayComplianceError::Replay) => match recover_rollback(controller, &rollback) {
            Ok(Some(recorded_at_unix)) => action_response(
                StatusCode::OK,
                "rollback",
                rollback.payload.to_catalog_digest,
                binding.idempotency_key,
                recorded_at_unix,
            ),
            Ok(None) => gateway_compliance_error_response(GatewayComplianceError::Replay),
            Err(response) => response,
        },
        Err(error) => gateway_compliance_error_response(error),
    }
}

fn gateway_compliance_runtime(
    state: &SharedAppState,
) -> Result<
    (
        &GatewayComplianceController,
        &std::sync::Arc<dyn super::gateway::GatewayComplianceFeedTransport>,
    ),
    Response,
> {
    let controller = gateway_compliance_controller(state)?;
    let transport = state
        .sorafs_gateway_compliance_feed_transport
        .as_ref()
        .ok_or_else(gateway_compliance_unavailable)?;
    Ok((controller, transport))
}

fn gateway_compliance_controller(
    state: &SharedAppState,
) -> Result<&GatewayComplianceController, Response> {
    state
        .sorafs_gateway_compliance_controller
        .as_deref()
        .ok_or_else(gateway_compliance_unavailable)
}

fn gateway_compliance_unavailable() -> Response {
    gateway_compliance_request_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "controller_unavailable",
        "the governed SoraFS gateway compliance controller is not enabled",
    )
}

fn authorize_gateway_compliance_request(
    state: &SharedAppState,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> Result<(), Response> {
    let verified = match crate::app_auth::verify_canonical_request(
        &state.state,
        headers,
        method,
        uri,
        body,
        None,
    ) {
        Ok(Some(verified)) => verified,
        Ok(None) => {
            return Err(gateway_compliance_request_error(
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "SoraFS gateway compliance control requires X-Iroha canonical request authentication",
            ));
        }
        Err(_) => {
            warn!("SoraFS gateway compliance canonical request authentication rejected");
            return Err(gateway_compliance_request_error(
                StatusCode::UNAUTHORIZED,
                "authentication_invalid",
                "invalid SoraFS gateway compliance request authentication",
            ));
        }
    };
    let world = state.state.world_view();
    if world
        .account_roles_iter(&verified.account)
        .any(|role| role == &*GATEWAY_COMPLIANCE_OPERATOR_ROLE_ID)
    {
        Ok(())
    } else {
        Err(gateway_compliance_request_error(
            StatusCode::FORBIDDEN,
            "operator_role_required",
            "SoraFS gateway compliance control requires the governed operator role",
        ))
    }
}

fn validate_gateway_compliance_body_headers(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), Response> {
    crate::utils::canonical_json_request_content_type(headers).map_err(no_store_response)?;
    if body.is_empty() {
        return Err(gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "body_required",
            "a canonical JSON gateway compliance payload is required",
        ));
    }
    if body.len() > MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1 {
        return Err(gateway_compliance_request_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "body_too_large",
            "the gateway compliance payload exceeds the V1 byte bound",
        ));
    }
    Ok(())
}

fn status_response(
    checkpoint: &GatewayComplianceCheckpointV1,
    observed_at_unix: u64,
) -> Result<GatewayComplianceStatusResponseV1, GatewayComplianceError> {
    let acknowledgement_count =
        u64::try_from(checkpoint.acknowledgements.len()).unwrap_or(u64::MAX);
    let accepted_acknowledgement_count = u64::try_from(
        checkpoint
            .acknowledgements
            .iter()
            .filter(|acknowledgement| acknowledgement.payload.accepted)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let rejected_acknowledgement_count =
        acknowledgement_count.saturating_sub(accepted_acknowledgement_count);
    let serving_ready = checkpoint.serving.as_ref().is_some_and(|catalog| {
        catalog.payload.generated_at_unix <= observed_at_unix
            && observed_at_unix < catalog.payload.valid_until_unix
    });

    Ok(GatewayComplianceStatusResponseV1 {
        schema: "sorafs.gateway.compliance.status.v1".to_owned(),
        checkpoint_version: checkpoint.version,
        policy_digest_hex: hex::encode(checkpoint.policy_digest),
        observed_at_unix,
        serving_ready,
        chain_head: checkpoint
            .chain_head
            .as_ref()
            .map(catalog_status)
            .transpose()?,
        serving: checkpoint
            .serving
            .as_ref()
            .map(catalog_status)
            .transpose()?,
        previous_serving: checkpoint
            .previous_serving
            .as_ref()
            .map(catalog_status)
            .transpose()?,
        candidate: checkpoint
            .candidate
            .as_ref()
            .map(catalog_status)
            .transpose()?,
        acknowledgement_count,
        accepted_acknowledgement_count,
        rejected_acknowledgement_count,
        history_count: u64::try_from(checkpoint.history.len()).unwrap_or(u64::MAX),
        latest_action: checkpoint.history.last().map(latest_action_status),
    })
}

fn catalog_status(
    catalog: &GatewayComplianceCatalogV1,
) -> Result<GatewayComplianceCatalogStatusV1, GatewayComplianceError> {
    Ok(GatewayComplianceCatalogStatusV1 {
        digest_hex: hex::encode(catalog.payload.catalog_digest()?),
        sequence: catalog.payload.sequence,
        generated_at_unix: catalog.payload.generated_at_unix,
        valid_until_unix: catalog.payload.valid_until_unix,
    })
}

fn latest_action_status(
    record: &GatewayComplianceHistoryRecordV1,
) -> GatewayComplianceLatestActionStatusV1 {
    GatewayComplianceLatestActionStatusV1 {
        operation_id_hex: hex::encode(record.operation_id),
        action: record.action.clone(),
        previous_serving_digest_hex: record.previous_serving_digest.map(hex::encode),
        serving_digest_hex: hex::encode(record.serving_digest),
        recorded_at_unix: record.recorded_at_unix,
        reason_code: record.reason_code.clone(),
    }
}

fn require_mutation_uri_without_query(uri: &Uri) -> Result<(), Response> {
    if uri.query().is_none() {
        Ok(())
    } else {
        Err(gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "unexpected_query",
            "this gateway compliance mutation does not accept query parameters",
        ))
    }
}

fn decode_canonical_promote_expectation(
    uri: &Uri,
) -> Result<GatewayCompliancePromoteExpectationV1, Response> {
    let query = uri.query().ok_or_else(|| {
        gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "promotion_expectation_missing",
            "gateway compliance promotion requires an exact catalog digest and sequence",
        )
    })?;
    let mut fields = query.split('&');
    let digest_field = fields.next().unwrap_or_default();
    let sequence_field = fields.next().unwrap_or_default();
    if fields.next().is_some() {
        return Err(non_canonical_promote_expectation());
    }
    let Some(digest_hex) = digest_field.strip_prefix("expected_catalog_digest=") else {
        return Err(non_canonical_promote_expectation());
    };
    let Some(sequence_text) = sequence_field.strip_prefix("expected_sequence=") else {
        return Err(non_canonical_promote_expectation());
    };
    let catalog_digest = decode_lower_hex_32(digest_hex)
        .ok_or_else(non_canonical_promote_expectation)?;
    let sequence = sequence_text
        .parse::<u64>()
        .ok()
        .filter(|sequence| *sequence != 0 && sequence.to_string() == sequence_text)
        .ok_or_else(non_canonical_promote_expectation)?;
    let canonical = format!(
        "expected_catalog_digest={digest_hex}&expected_sequence={sequence}"
    );
    if canonical != query {
        return Err(non_canonical_promote_expectation());
    }
    Ok(GatewayCompliancePromoteExpectationV1 {
        catalog_digest,
        sequence,
    })
}

fn non_canonical_promote_expectation() -> Response {
    gateway_compliance_request_error(
        StatusCode::BAD_REQUEST,
        "promotion_expectation_invalid",
        "the promotion expectation must use exact canonical digest and sequence syntax",
    )
}

fn require_request_idempotency_binding(
    headers: &HeaderMap,
    action: &'static str,
    uri: &Uri,
    body: &[u8],
) -> Result<GatewayComplianceMutationBindingV1, Response> {
    let request_digest = request_idempotency_binding(action, uri, body);
    let idempotency_key = require_exact_idempotency_key(headers, request_digest)?;
    Ok(GatewayComplianceMutationBindingV1 {
        idempotency_key,
        request_digest,
    })
}

fn require_operation_idempotency_binding(
    headers: &HeaderMap,
    operation_id: [u8; 32],
    action: &'static str,
    uri: &Uri,
    body: &[u8],
) -> Result<GatewayComplianceMutationBindingV1, Response> {
    if operation_id.iter().all(|byte| *byte == 0) {
        return Err(gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "operation_id_invalid",
            "the signed gateway compliance operation id must be non-zero",
        ));
    }
    let idempotency_key = require_exact_idempotency_key(headers, operation_id)?;
    Ok(GatewayComplianceMutationBindingV1 {
        idempotency_key,
        request_digest: request_idempotency_binding(action, uri, body),
    })
}

fn require_exact_idempotency_key(
    headers: &HeaderMap,
    expected: [u8; 32],
) -> Result<[u8; 32], Response> {
    let actual = validated_idempotency_key(headers)?;
    if actual != expected {
        return Err(gateway_compliance_request_error(
            StatusCode::CONFLICT,
            "idempotency_key_conflict",
            "Idempotency-Key does not match the canonical signed operation binding",
        ));
    }
    Ok(actual)
}

fn validated_idempotency_key(headers: &HeaderMap) -> Result<[u8; 32], Response> {
    let mut values = headers.get_all(IDEMPOTENCY_KEY_HEADER).iter();
    let Some(raw) = values.next() else {
        return Err(gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "idempotency_key_missing",
            "gateway compliance mutations require exactly one Idempotency-Key header",
        ));
    };
    if values.next().is_some() {
        return Err(gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "idempotency_key_invalid",
            "gateway compliance mutations require exactly one Idempotency-Key header",
        ));
    }
    let key = raw.to_str().map_err(|_| {
        gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "idempotency_key_invalid",
            "Idempotency-Key must be lowercase hexadecimal ASCII",
        )
    })?;
    if key.len() != 64
        || key.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || key.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "idempotency_key_invalid",
            "Idempotency-Key must be exactly 64 lowercase hexadecimal characters",
        ));
    }
    decode_lower_hex_32(key).ok_or_else(|| {
        gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "idempotency_key_invalid",
            "Idempotency-Key must be exactly 64 lowercase hexadecimal characters",
        )
    })
}

fn request_idempotency_binding(action: &str, uri: &Uri, body: &[u8]) -> [u8; 32] {
    let request_target = uri
        .path_and_query()
        .map_or_else(|| uri.path(), |value| value.as_str());
    let mut hasher = Sha256::new();
    hasher.update(IDEMPOTENCY_BINDING_DOMAIN_V1);
    hasher.update((action.len() as u64).to_be_bytes());
    hasher.update(action.as_bytes());
    hasher.update((request_target.len() as u64).to_be_bytes());
    hasher.update(request_target.as_bytes());
    hasher.update((body.len() as u64).to_be_bytes());
    hasher.update(body);
    hasher.finalize().into()
}

fn decode_lower_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || value.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return None;
    }
    let decoded = hex::decode(value).ok()?;
    decoded.try_into().ok()
}

fn catalog_transition_already_committed(
    controller: &GatewayComplianceController,
    catalog_digest: [u8; 32],
) -> Result<bool, GatewayComplianceError> {
    let checkpoint = controller.checkpoint()?;
    for catalog in [
        checkpoint.candidate.as_ref(),
        checkpoint.chain_head.as_ref(),
        checkpoint.serving.as_ref(),
        checkpoint.previous_serving.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if catalog.payload.catalog_digest()? == catalog_digest {
            return Ok(true);
        }
    }
    Ok(false)
}

fn recover_or_validate_promotion(
    controller: &GatewayComplianceController,
    expectation: GatewayCompliancePromoteExpectationV1,
) -> Result<Option<u64>, Response> {
    let checkpoint = controller
        .checkpoint()
        .map_err(gateway_compliance_error_response)?;
    if let Some(candidate) = checkpoint.candidate.as_ref() {
        let digest = candidate
            .payload
            .catalog_digest()
            .map_err(gateway_compliance_error_response)?;
        if digest != expectation.catalog_digest || candidate.payload.sequence != expectation.sequence
        {
            return Err(expected_resource_conflict());
        }
        return Ok(None);
    }
    let Some(record) = checkpoint.history.iter().rev().find(|record| {
        record.action == "promotion" && record.serving_digest == expectation.catalog_digest
    }) else {
        return Ok(None);
    };
    let retained_sequence = [
        checkpoint.chain_head.as_ref(),
        checkpoint.serving.as_ref(),
        checkpoint.previous_serving.as_ref(),
    ]
    .into_iter()
    .flatten()
    .find_map(|catalog| {
        catalog
            .payload
            .catalog_digest()
            .ok()
            .filter(|digest| *digest == expectation.catalog_digest)
            .map(|_| catalog.payload.sequence)
    });
    if retained_sequence != Some(expectation.sequence) {
        return Err(expected_resource_conflict());
    }
    Ok(Some(record.recorded_at_unix))
}

fn recover_rollback(
    controller: &GatewayComplianceController,
    rollback: &GatewayComplianceRollbackV1,
) -> Result<Option<u64>, Response> {
    let checkpoint = controller
        .checkpoint()
        .map_err(gateway_compliance_error_response)?;
    let Some(record) = checkpoint
        .history
        .iter()
        .find(|record| record.operation_id == rollback.payload.operation_id)
    else {
        return Ok(None);
    };
    if record.action != "rollback"
        || record.previous_serving_digest != Some(rollback.payload.from_catalog_digest)
        || record.serving_digest != rollback.payload.to_catalog_digest
        || record.reason_code != rollback.payload.reason_code
    {
        return Err(gateway_compliance_request_error(
            StatusCode::CONFLICT,
            "idempotency_key_conflict",
            "Idempotency-Key is already bound to a different rollback operation",
        ));
    }
    Ok(Some(record.recorded_at_unix))
}

fn expected_resource_conflict() -> Response {
    gateway_compliance_request_error(
        StatusCode::CONFLICT,
        "expected_resource_conflict",
        "the expected gateway compliance catalog digest or sequence does not match durable state",
    )
}

fn decode_canonical_catalog(body: &[u8]) -> Result<GatewayComplianceCatalogV1, Response> {
    let value: GatewayComplianceCatalogV1 = norito::json::from_slice(body).map_err(|_| {
        gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "catalog_invalid",
            "invalid canonical gateway compliance catalog JSON",
        )
    })?;
    require_exact_canonical_json(body, &value, "catalog")?;
    Ok(value)
}

fn decode_canonical_acknowledgement(
    body: &[u8],
) -> Result<GatewayComplianceAcknowledgementV1, Response> {
    let value: GatewayComplianceAcknowledgementV1 =
        norito::json::from_slice(body).map_err(|_| {
            gateway_compliance_request_error(
                StatusCode::BAD_REQUEST,
                "acknowledgement_invalid",
                "invalid canonical gateway compliance acknowledgement JSON",
            )
        })?;
    require_exact_canonical_json(body, &value, "acknowledgement")?;
    Ok(value)
}

fn decode_canonical_rollback(body: &[u8]) -> Result<GatewayComplianceRollbackV1, Response> {
    let value: GatewayComplianceRollbackV1 = norito::json::from_slice(body).map_err(|_| {
        gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "rollback_invalid",
            "invalid canonical gateway compliance rollback JSON",
        )
    })?;
    require_exact_canonical_json(body, &value, "rollback")?;
    Ok(value)
}

fn require_exact_canonical_json<T>(
    body: &[u8],
    value: &T,
    label: &'static str,
) -> Result<(), Response>
where
    T: norito::json::JsonSerialize,
{
    let canonical = norito::json::to_vec(value).map_err(|_| {
        warn!(label, "failed to re-encode gateway compliance request");
        gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "canonical_encoding_failed",
            "the gateway compliance payload cannot be represented canonically",
        )
    })?;
    if canonical == body {
        Ok(())
    } else {
        Err(gateway_compliance_request_error(
            StatusCode::BAD_REQUEST,
            "non_canonical_json",
            "the gateway compliance payload must use exact canonical Norito JSON bytes",
        ))
    }
}

fn current_unix_second() -> Result<u64, Response> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| {
            warn!("system clock is before the Unix epoch for gateway compliance control");
            gateway_compliance_request_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "clock_unavailable",
                "the gateway compliance control clock is unavailable",
            )
        })
}

fn action_response(
    status: StatusCode,
    action: &'static str,
    digest: [u8; 32],
    idempotency_key: &str,
    operation_timestamp_unix: u64,
) -> Response {
    no_store_response((
        status,
        JsonBody(GatewayComplianceActionResponseV1 {
            schema: "sorafs.gateway.compliance.action.v1".to_owned(),
            action: action.to_owned(),
            catalog_digest_hex: hex::encode(digest),
            idempotency_key: idempotency_key.to_owned(),
            operation_timestamp_unix,
        }),
    )
        .into_response())
}

fn gateway_compliance_request_error(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> Response {
    no_store_response((
        status,
        JsonBody(GatewayComplianceErrorResponseV1 {
            schema: "sorafs.gateway.compliance.error.v1".to_owned(),
            code: code.to_owned(),
            message: message.to_owned(),
        }),
    )
        .into_response())
}

fn no_store_response(mut response: Response) -> Response {
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, no-store, max-age=0"),
    );
    response
}

fn gateway_compliance_error_response(error: GatewayComplianceError) -> Response {
    use GatewayComplianceError::{
        CatalogEquivocation, CatalogNotFresh, Decompression, DnsRebinding, DuplicateSigner,
        Encoding, FetchTimeout, GatewayEquivocation, GatewayQuorumNotMet, HistoryFull,
        InvalidAcknowledgement, InvalidCatalog, InvalidCheckpoint, InvalidFeed, InvalidPolicy,
        InvalidPredecessor, InvalidRollback, InvalidSignature, MissingRequiredFeed,
        NoLastKnownGood, NoServingCatalog, NoStagedCatalog, NonCanonical, NonPublicAddress,
        Persistence, PolicyDigestMismatch, QuorumNotMet, Replay, ResourceLimit, RevokedSigner,
        RollbackTargetMismatch, SequenceOverflow, StatePoisoned, TimeOverflow, TooManyRedirects,
        TrustPinMismatch, UnsafeAddressSet, UnsafeUrl, UnknownFeed, UntrustedSigner,
    };

    let (status, code, message) = match &error {
        UnknownFeed(_) => (
            StatusCode::NOT_FOUND,
            "feed_not_found",
            "the configured gateway compliance feed does not exist",
        ),
        NoServingCatalog | NoLastKnownGood | NoStagedCatalog => (
            StatusCode::CONFLICT,
            "controller_state_conflict",
            "the gateway compliance controller is not in the required state",
        ),
        CatalogEquivocation { .. }
        | GatewayEquivocation(_)
        | InvalidPredecessor
        | RollbackTargetMismatch
        | Replay
        | HistoryFull
        | GatewayQuorumNotMet { .. } => (
            StatusCode::CONFLICT,
            "transition_rejected",
            "the gateway compliance transition conflicts with durable state",
        ),
        DuplicateSigner(_)
        | UntrustedSigner(_)
        | RevokedSigner(_)
        | InvalidSignature { .. }
        | QuorumNotMet { .. } => (
            StatusCode::FORBIDDEN,
            "signature_policy_rejected",
            "the gateway compliance signature policy rejected the payload",
        ),
        ResourceLimit { .. } => (
            StatusCode::PAYLOAD_TOO_LARGE,
            "resource_limit",
            "the gateway compliance payload exceeds a governed V1 bound",
        ),
        UnsafeUrl(_)
        | UnsafeAddressSet { .. }
        | NonPublicAddress
        | DnsRebinding
        | TrustPinMismatch
        | TooManyRedirects
        | FetchTimeout
        | Decompression(_) => (
            StatusCode::BAD_GATEWAY,
            "feed_transport_rejected",
            "the authenticated gateway compliance feed transport failed closed",
        ),
        Persistence(_) | StatePoisoned | InvalidCheckpoint(_) | Encoding(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "controller_unavailable",
            "the durable gateway compliance controller is unavailable",
        ),
        InvalidPolicy(_)
        | InvalidCatalog(_)
        | InvalidFeed(_)
        | NonCanonical(_)
        | PolicyDigestMismatch
        | CatalogNotFresh
        | InvalidAcknowledgement(_)
        | InvalidRollback(_)
        | MissingRequiredFeed(_)
        | SequenceOverflow
        | TimeOverflow => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "payload_rejected",
            "the gateway compliance payload is invalid for the active policy",
        ),
    };
    warn!(
        response_code = code,
        response_status = status.as_u16(),
        "gateway compliance control operation rejected"
    );
    gateway_compliance_request_error(status, code, message)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::*;
    use crate::sorafs::gateway::{
        GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1, GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
        GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1, GatewayComplianceCatalogApprovalV1,
        GatewayComplianceCatalogPayloadV1,
    };

    fn signed_catalog() -> GatewayComplianceCatalogV1 {
        let signing_key = SigningKey::from_bytes(&[0x31; 32]);
        let payload = GatewayComplianceCatalogPayloadV1 {
            version: GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
            sequence: 1,
            predecessor_digest: None,
            policy_digest: [0xA5; 32],
            generated_at_unix: 1_700_000_000,
            valid_until_unix: 1_700_003_600,
            source_anchors: Vec::new(),
            baseline_rules: Vec::new(),
            appeal_overrides: Vec::new(),
            legal_safety_holds: Vec::new(),
            toggles: Vec::new(),
        };
        let digest = payload
            .signing_digest()
            .expect("catalog signing digest");
        GatewayComplianceCatalogV1 {
            payload,
            approvals: vec![GatewayComplianceCatalogApprovalV1 {
                version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                signer_id: "catalog-a".to_owned(),
                signature: signing_key.sign(&digest).to_bytes(),
            }],
        }
    }

    #[test]
    fn catalog_body_requires_exact_canonical_json_bytes() {
        let catalog = signed_catalog();
        let canonical = norito::json::to_vec(&catalog).expect("canonical catalog JSON");
        assert_eq!(
            decode_canonical_catalog(&canonical)
                .expect("canonical request")
                .payload,
            catalog.payload
        );

        let mut padded = canonical;
        padded.push(b'\n');
        assert!(decode_canonical_catalog(&padded).is_err());
    }

    #[test]
    fn controller_errors_map_to_fail_closed_http_classes() {
        assert_eq!(
            gateway_compliance_error_response(GatewayComplianceError::UnknownFeed(
                "missing".to_owned()
            ))
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            gateway_compliance_error_response(GatewayComplianceError::DnsRebinding).status(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            gateway_compliance_error_response(GatewayComplianceError::Replay).status(),
            StatusCode::CONFLICT
        );
        let signature_response =
            gateway_compliance_error_response(GatewayComplianceError::InvalidSignature {
                signer_id: "catalog-a".to_owned(),
                reason: "invalid".to_owned(),
            });
        assert_eq!(signature_response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            signature_response.headers().get(CACHE_CONTROL),
            Some(&HeaderValue::from_static("private, no-store, max-age=0"))
        );
    }

    #[test]
    fn status_projection_never_serializes_catalog_or_signature_payloads() {
        let catalog = signed_catalog();
        let catalog_digest = catalog
            .payload
            .catalog_digest()
            .expect("catalog digest");
        let checkpoint = GatewayComplianceCheckpointV1 {
            version: GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1,
            policy_digest: [0xA5; 32],
            chain_head: Some(catalog.clone()),
            serving: Some(catalog.clone()),
            previous_serving: None,
            candidate: Some(catalog),
            acknowledgements: Vec::new(),
            history: vec![GatewayComplianceHistoryRecordV1 {
                operation_id: [0x42; 32],
                previous_serving_digest: None,
                serving_digest: catalog_digest,
                recorded_at_unix: 1_700_000_010,
                action: "promotion".to_owned(),
                reason_code: "gateway-quorum".to_owned(),
            }],
        };
        let status =
            status_response(&checkpoint, 1_700_000_020).expect("redacted status projection");
        let json = norito::json::to_string(&status).expect("status JSON");

        assert!(json.len() < 4_096, "status projection must remain tightly bounded");
        assert!(json.contains("\"serving_ready\":true"));
        assert!(json.contains(&hex::encode(catalog_digest)));
        for forbidden in [
            "\"approvals\"",
            "\"signature\"",
            "\"signer_id\"",
            "catalog-a",
            "\"source_anchors\"",
            "\"baseline_rules\"",
            "\"appeal_overrides\"",
            "\"legal_safety_holds\"",
            "\"toggles\"",
            "\"acknowledgements\"",
        ] {
            assert!(
                !json.contains(forbidden),
                "redacted status leaked forbidden field `{forbidden}`: {json}"
            );
        }
    }

    #[test]
    fn status_readiness_is_false_before_generation_and_at_expiry() {
        let catalog = signed_catalog();
        let checkpoint = GatewayComplianceCheckpointV1 {
            version: GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1,
            policy_digest: [0xA5; 32],
            chain_head: Some(catalog.clone()),
            serving: Some(catalog),
            previous_serving: None,
            candidate: None,
            acknowledgements: Vec::new(),
            history: Vec::new(),
        };
        assert!(
            !status_response(&checkpoint, 1_699_999_999)
                .expect("status before generation")
                .serving_ready
        );
        assert!(
            !status_response(&checkpoint, 1_700_003_600)
                .expect("status at exclusive expiry")
                .serving_ready
        );
    }

    #[test]
    fn idempotency_key_is_exact_lower_hex_and_unique() {
        let uri: Uri = "/v1/sorafs/gateway/compliance/stage"
            .parse()
            .expect("stage URI");
        let body = b"{\"catalog\":\"fixture\"}";
        let expected = hex::encode(request_idempotency_binding("stage", &uri, body));
        let mut headers = HeaderMap::new();

        assert!(validated_idempotency_key(&headers).is_err());
        headers.insert(
            IDEMPOTENCY_KEY_HEADER,
            HeaderValue::from_str(&expected).expect("idempotency header"),
        );
        assert_eq!(
            require_request_idempotency_key(&headers, "stage", &uri, body)
                .expect("matching binding"),
            expected
        );

        headers.append(
            IDEMPOTENCY_KEY_HEADER,
            HeaderValue::from_str(&expected).expect("duplicate idempotency header"),
        );
        assert!(validated_idempotency_key(&headers).is_err());
        headers.remove(IDEMPOTENCY_KEY_HEADER);

        for malformed in [
            "11",
            "111111111111111111111111111111111111111111111111111111111111111g",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            headers.insert(
                IDEMPOTENCY_KEY_HEADER,
                HeaderValue::from_str(malformed).expect("malformed ASCII fixture"),
            );
            assert!(validated_idempotency_key(&headers).is_err());
        }
    }

    #[test]
    fn request_binding_rejects_key_reuse_for_changed_signed_material() {
        let uri: Uri = "/v1/sorafs/gateway/compliance/stage"
            .parse()
            .expect("stage URI");
        let original = b"{\"catalog\":1}";
        let changed = b"{\"catalog\":2}";
        let key = hex::encode(request_idempotency_binding("stage", &uri, original));
        let mut headers = HeaderMap::new();
        headers.insert(
            IDEMPOTENCY_KEY_HEADER,
            HeaderValue::from_str(&key).expect("idempotency header"),
        );

        assert!(
            require_request_idempotency_key(&headers, "stage", &uri, original).is_ok()
        );
        let response = require_request_idempotency_key(&headers, "stage", &uri, changed)
            .expect_err("same key must reject a different canonical body");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.headers().get(CACHE_CONTROL),
            Some(&HeaderValue::from_static("private, no-store, max-age=0"))
        );
        assert_ne!(
            request_idempotency_binding("stage", &uri, original),
            request_idempotency_binding("acknowledge", &uri, original)
        );
    }

    #[test]
    fn promotion_expectation_has_one_canonical_signed_uri_shape() {
        let digest = "ab".repeat(32);
        let canonical: Uri = format!(
            "/v1/sorafs/gateway/compliance/promote?expected_catalog_digest={digest}&expected_sequence=7"
        )
        .parse()
        .expect("canonical promotion URI");
        assert_eq!(
            decode_canonical_promote_expectation(&canonical)
                .expect("canonical promotion expectation"),
            GatewayCompliancePromoteExpectationV1 {
                catalog_digest: [0xAB; 32],
                sequence: 7,
            }
        );

        for invalid in [
            "/v1/sorafs/gateway/compliance/promote".to_owned(),
            format!(
                "/v1/sorafs/gateway/compliance/promote?expected_sequence=7&expected_catalog_digest={digest}"
            ),
            format!(
                "/v1/sorafs/gateway/compliance/promote?expected_catalog_digest={digest}&expected_sequence=07"
            ),
            format!(
                "/v1/sorafs/gateway/compliance/promote?expected_catalog_digest={}&expected_sequence=7",
                "AB".repeat(32)
            ),
            format!(
                "/v1/sorafs/gateway/compliance/promote?expected_catalog_digest={digest}&expected_sequence=7&expected_sequence=8"
            ),
        ] {
            let uri: Uri = invalid.parse().expect("syntactically valid invalid URI");
            let response = decode_canonical_promote_expectation(&uri)
                .expect_err("non-canonical promotion expectation must fail");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                response.headers().get(CACHE_CONTROL),
                Some(&HeaderValue::from_static("private, no-store, max-age=0"))
            );
        }
    }

    #[test]
    fn rollback_idempotency_key_must_equal_signed_operation_id() {
        let operation_id = [0x5C; 32];
        let expected = hex::encode(operation_id);
        let mut headers = HeaderMap::new();
        headers.insert(
            IDEMPOTENCY_KEY_HEADER,
            HeaderValue::from_str(&expected).expect("operation id header"),
        );
        assert_eq!(
            require_operation_idempotency_key(&headers, operation_id)
                .expect("matching operation id"),
            expected
        );
        assert_eq!(
            require_operation_idempotency_key(&headers, [0x5D; 32])
                .expect_err("mismatched operation id")
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            require_operation_idempotency_key(&headers, [0; 32])
                .expect_err("zero operation id")
                .status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn every_local_response_constructor_applies_private_no_store() {
        let responses = [
            action_response(
                StatusCode::OK,
                "promote",
                [0x11; 32],
                &"11".repeat(32),
                1_700_000_000,
            ),
            gateway_compliance_request_error(
                StatusCode::BAD_REQUEST,
                "fixture",
                "fixture",
            ),
            gateway_compliance_error_response(GatewayComplianceError::Replay),
        ];
        for response in responses {
            assert_eq!(
                response.headers().get(CACHE_CONTROL),
                Some(&HeaderValue::from_static("private, no-store, max-age=0"))
            );
        }
    }
}
