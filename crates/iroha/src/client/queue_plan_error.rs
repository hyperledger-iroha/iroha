//! Canonical `QueuePlan` outcome-unknown evidence and ambiguity classification.

use super::{
    APPLICATION_JSON, APPLICATION_NORITO, ErrorEnvelope, QUEUE_PLAN_OUTCOME_UNKNOWN_ENVELOPE_CODE,
    QUEUE_PLAN_OUTCOME_UNKNOWN_REJECT_CODE, QueuePlanOutcomeUnknownIdentity,
    SIGNED_TRANSACTION_HASH_HEADER, TRANSACTION_ENTRYPOINT_HASH_HEADER,
    exact_single_response_header,
};
use crate::http::{Response, StatusCode};
use eyre::Result;
use norito::{decode_from_bytes, to_bytes};

fn decode_canonical_queue_plan_error_envelope(
    response: &Response<Vec<u8>>,
) -> Result<ErrorEnvelope, String> {
    let content_types = response
        .headers()
        .get_all(http::header::CONTENT_TYPE)
        .iter()
        .collect::<Vec<_>>();
    if content_types.len() != 1 {
        return Err(
            "QueuePlan outcome-unknown content-type header is missing or duplicated".to_owned(),
        );
    }
    let content_type = content_types[0].to_str().map_err(|_| {
        "QueuePlan outcome-unknown content-type header is not valid text".to_owned()
    })?;
    let envelope = match content_type {
        APPLICATION_NORITO => decode_from_bytes::<ErrorEnvelope>(response.body())
            .map_err(|_| "QueuePlan outcome-unknown Norito envelope is invalid".to_owned())?,
        APPLICATION_JSON => norito::json::from_slice::<ErrorEnvelope>(response.body())
            .map_err(|_| "QueuePlan outcome-unknown JSON envelope is invalid".to_owned())?,
        _ => {
            return Err(
                "QueuePlan outcome-unknown content type is not canonical Norito or JSON".to_owned(),
            );
        }
    };
    let canonical = if content_type == APPLICATION_NORITO {
        to_bytes(&envelope)
            .map_err(|_| "QueuePlan outcome-unknown Norito envelope cannot be encoded".to_owned())?
    } else {
        norito::json::to_vec(&envelope)
            .map_err(|_| "QueuePlan outcome-unknown JSON envelope cannot be encoded".to_owned())?
    };
    if canonical != response.body().as_slice() {
        return Err("QueuePlan outcome-unknown envelope bytes are not canonical".to_owned());
    }
    Ok(envelope)
}
pub(super) fn classify(
    response: &Response<Vec<u8>>,
    expected: &QueuePlanOutcomeUnknownIdentity,
) -> Result<Option<QueuePlanOutcomeUnknownIdentity>, String> {
    let envelope = decode_canonical_queue_plan_error_envelope(response);
    if !claims_outcome_unknown(response) {
        return Ok(None);
    }
    let reject_headers = response
        .headers()
        .get_all("x-iroha-reject-code")
        .iter()
        .collect::<Vec<_>>();
    if response.status() != StatusCode::SERVICE_UNAVAILABLE {
        return Err(
            "QueuePlan outcome-unknown evidence did not use 503 Service Unavailable".to_owned(),
        );
    }
    if reject_headers.len() != 1
        || !reject_headers[0]
            .to_str()
            .is_ok_and(|value| value == QUEUE_PLAN_OUTCOME_UNKNOWN_REJECT_CODE)
    {
        return Err(
            "QueuePlan outcome-unknown reject-code header is missing, duplicated, or invalid"
                .to_owned(),
        );
    }
    let envelope = envelope?;
    if envelope.code() != QUEUE_PLAN_OUTCOME_UNKNOWN_ENVELOPE_CODE {
        return Err("QueuePlan outcome-unknown envelope code is invalid".to_owned());
    }
    let details = envelope
        .details
        .as_ref()
        .ok_or_else(|| "QueuePlan outcome-unknown envelope is missing details".to_owned())?;
    if details.reject_code.as_deref() != Some(QUEUE_PLAN_OUTCOME_UNKNOWN_REJECT_CODE) {
        return Err("QueuePlan outcome-unknown envelope reject code is invalid".to_owned());
    }
    let expected_entrypoint_hash = expected.entrypoint_hash.to_string();
    let entrypoint_hash_header = exact_single_response_header(
        response,
        TRANSACTION_ENTRYPOINT_HASH_HEADER,
    )
    .map_err(|_| {
        "QueuePlan outcome-unknown entrypoint header is missing, duplicated, or invalid".to_owned()
    })?;
    if entrypoint_hash_header != expected_entrypoint_hash {
        return Err(
            "QueuePlan outcome-unknown entrypoint header does not match the submitted transaction"
                .to_owned(),
        );
    }
    if details.entrypoint_hash.as_deref() != Some(expected_entrypoint_hash.as_str()) {
        return Err(
            "QueuePlan outcome-unknown entrypoint identity is missing or does not match the submitted transaction"
                .to_owned(),
        );
    }
    let expected_signed_transaction_hash = expected.signed_transaction_hash.to_string();
    let signed_transaction_hash_header =
        exact_single_response_header(response, SIGNED_TRANSACTION_HASH_HEADER).map_err(|_| {
            "QueuePlan outcome-unknown signed-transaction header is missing, duplicated, or invalid"
                .to_owned()
        })?;
    if signed_transaction_hash_header != expected_signed_transaction_hash {
        return Err(
            "QueuePlan outcome-unknown signed-transaction header does not match the submitted transaction"
                .to_owned(),
        );
    }
    if details.tx_hash.as_deref() != Some(expected_signed_transaction_hash.as_str()) {
        return Err(
            "QueuePlan outcome-unknown signed-transaction identity is missing or does not match the submitted transaction"
                .to_owned(),
        );
    }
    Ok(Some(expected.clone()))
}
fn claims_outcome_unknown(response: &Response<Vec<u8>>) -> bool {
    // Claim detection is deliberately more permissive than evidence acceptance. A proxy that
    // damages the content type, byte canonicality, or one identity field still cannot turn an
    // indeterminate admission into a definite rejection that callers may safely resubmit.
    let claimed_envelope = decode_from_bytes::<ErrorEnvelope>(response.body())
        .ok()
        .or_else(|| norito::json::from_slice::<ErrorEnvelope>(response.body()).ok());
    let envelope_claims_outcome_unknown = claimed_envelope.as_ref().is_some_and(|envelope| {
        envelope.code() == QUEUE_PLAN_OUTCOME_UNKNOWN_ENVELOPE_CODE
            || envelope.details.as_ref().is_some_and(|details| {
                details.reject_code.as_deref() == Some(QUEUE_PLAN_OUTCOME_UNKNOWN_REJECT_CODE)
            })
    });
    let body_claims_outcome_unknown = [
        QUEUE_PLAN_OUTCOME_UNKNOWN_ENVELOPE_CODE,
        QUEUE_PLAN_OUTCOME_UNKNOWN_REJECT_CODE,
    ]
    .into_iter()
    .any(|claim| {
        response
            .body()
            .windows(claim.len())
            .any(|window| window == claim.as_bytes())
    });
    let header_claims_outcome_unknown = response
        .headers()
        .get_all("x-iroha-reject-code")
        .iter()
        .any(|value| {
            value.to_str().is_ok_and(|value| {
                value == QUEUE_PLAN_OUTCOME_UNKNOWN_REJECT_CODE
                    || value == QUEUE_PLAN_OUTCOME_UNKNOWN_ENVELOPE_CODE
            })
        });
    // Successful QueuePlan responses legitimately carry both identity headers. On a rejection,
    // those headers are reserved for outcome-unknown, so even damaged code/body evidence must
    // keep the locally computed identity ambiguous.
    let rejection_identity_headers_claim_outcome_unknown =
        !matches!(response.status(), StatusCode::OK | StatusCode::ACCEPTED)
            && (response
                .headers()
                .contains_key(TRANSACTION_ENTRYPOINT_HASH_HEADER)
                || response
                    .headers()
                    .contains_key(SIGNED_TRANSACTION_HASH_HEADER));
    envelope_claims_outcome_unknown
        || body_claims_outcome_unknown
        || header_claims_outcome_unknown
        || rejection_identity_headers_claim_outcome_unknown
}
