//! Native authentication for restricted atomic-private-settlement responses.

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{NetworkId, nexus::ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1};
use iroha_torii_shared::private_settlement_api::{
    PrivateSettlementAuditApprovalRequestV1, PrivateSettlementAuditApprovalResponseV1,
    PrivateSettlementAuditorCapsuleResponseV1, PrivateSettlementCommitteeProofResponseV1,
    validate_private_settlement_audit_approval_response_v1,
    validate_private_settlement_auditor_capsule_response_v1,
    validate_private_settlement_auditor_identity_v1,
    validate_private_settlement_committee_proof_response_v1,
};
use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;
use norito::json;
use std::str::FromStr;

const RESPONSE_JSON_MAX_BYTES_V1: usize = 32 * 1024 * 1024;
const APPROVAL_REQUEST_JSON_MAX_BYTES_V1: usize = 1024 * 1024;
const AUDITOR_SIGNING_KEY_MAX_BYTES_V1: usize = 1024;

fn invalid() -> napi::Error {
    napi::Error::new(
        napi::Status::InvalidArg,
        "atomic private settlement response is invalid".to_owned(),
    )
}

fn bounded_json(bytes: &Uint8Array, maximum: usize) -> napi::Result<&[u8]> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid());
    }
    Ok(bytes.as_ref())
}

fn fixed_hash(bytes: &Uint8Array) -> napi::Result<[u8; 32]> {
    bytes.as_ref().try_into().map_err(|_| invalid())
}

fn expected_network(bytes: &Uint8Array) -> napi::Result<NetworkId> {
    Ok(NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::prehashed(fixed_hash(bytes)?)),
    ))
}

fn requested_payload(bytes: &Uint8Array) -> napi::Result<Hash> {
    fixed_hash(bytes).map(Hash::prehashed)
}

fn auditor_signing_key(literal: &str) -> napi::Result<PublicKey> {
    if literal.is_empty()
        || literal.trim() != literal
        || literal.len() > AUDITOR_SIGNING_KEY_MAX_BYTES_V1
        || literal.contains('\0')
    {
        return Err(invalid());
    }
    let key = PublicKey::from_str(literal).map_err(|_| invalid())?;
    if key.to_string() != literal {
        return Err(invalid());
    }
    Ok(key)
}

/// Verify a committee-only proof response against its exact network and leg.
#[napi(js_name = "privateSettlementVerifyCommitteeProofResponseV1")]
pub fn verify_committee_proof_response_v1(
    response_json: Uint8Array,
    expected_network_id: Uint8Array,
    requested_payload_digest: Uint8Array,
) -> napi::Result<()> {
    let response: PrivateSettlementCommitteeProofResponseV1 =
        json::from_slice(bounded_json(&response_json, RESPONSE_JSON_MAX_BYTES_V1)?)
            .map_err(|_| invalid())?;
    validate_private_settlement_committee_proof_response_v1(
        &expected_network(&expected_network_id)?,
        requested_payload(&requested_payload_digest)?,
        &response,
    )
    .map_err(|_| invalid())
}

/// Verify an auditor-only capsule response and the requesting auditor identity.
#[napi(js_name = "privateSettlementVerifyAuditorCapsuleResponseV1")]
pub fn verify_auditor_capsule_response_v1(
    response_json: Uint8Array,
    expected_network_id: Uint8Array,
    requested_payload_digest: Uint8Array,
    auditor_signing_key_literal: String,
) -> napi::Result<()> {
    let response: PrivateSettlementAuditorCapsuleResponseV1 =
        json::from_slice(bounded_json(&response_json, RESPONSE_JSON_MAX_BYTES_V1)?)
            .map_err(|_| invalid())?;
    validate_private_settlement_auditor_capsule_response_v1(
        &expected_network(&expected_network_id)?,
        requested_payload(&requested_payload_digest)?,
        &response,
    )
    .map_err(|_| invalid())?;
    validate_private_settlement_auditor_identity_v1(
        &auditor_signing_key(&auditor_signing_key_literal)?,
        &response,
    )
    .map_err(|_| invalid())
}

/// Verify an approval acknowledgement against the exact signed approval request.
#[napi(js_name = "privateSettlementVerifyAuditApprovalResponseV1")]
pub fn verify_audit_approval_response_v1(
    response_json: Uint8Array,
    request_json: Uint8Array,
    expected_network_id: Uint8Array,
    requested_payload_digest: Uint8Array,
    auditor_signing_key_literal: String,
) -> napi::Result<()> {
    let request: PrivateSettlementAuditApprovalRequestV1 = json::from_slice(bounded_json(
        &request_json,
        APPROVAL_REQUEST_JSON_MAX_BYTES_V1,
    )?)
    .map_err(|_| invalid())?;
    let network = expected_network(&expected_network_id)?;
    let auditor_key = auditor_signing_key(&auditor_signing_key_literal)?;
    if request.approval.body.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1
        || request.approval.body.network_id != network
        || request
            .approval
            .signature
            .verify(&auditor_key, &request.approval.body)
            .is_err()
    {
        return Err(invalid());
    }
    let response: PrivateSettlementAuditApprovalResponseV1 =
        json::from_slice(bounded_json(&response_json, RESPONSE_JSON_MAX_BYTES_V1)?)
            .map_err(|_| invalid())?;
    validate_private_settlement_audit_approval_response_v1(
        requested_payload(&requested_payload_digest)?,
        &request,
        &response,
    )
    .map(|_| ())
    .map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_verifiers_reject_untrusted_inputs_without_details() {
        for error in [
            verify_committee_proof_response_v1(
                Uint8Array::from(b"{}".to_vec()),
                Uint8Array::from(vec![0; 31]),
                Uint8Array::from(vec![1; 32]),
            )
            .expect_err("short network must fail"),
            verify_auditor_capsule_response_v1(
                Uint8Array::from(b"{}".to_vec()),
                Uint8Array::from(vec![0; 31]),
                Uint8Array::from(vec![1; 32]),
                "not-a-key".to_owned(),
            )
            .expect_err("short network must fail"),
            verify_audit_approval_response_v1(
                Uint8Array::from(b"{}".to_vec()),
                Uint8Array::from(b"{}".to_vec()),
                Uint8Array::from(vec![0; 31]),
                Uint8Array::from(vec![1; 32]),
                "not-a-key".to_owned(),
            )
            .expect_err("invalid request must fail"),
        ] {
            assert_eq!(error.status, napi::Status::InvalidArg);
            assert_eq!(
                error.reason,
                "atomic private settlement response is invalid"
            );
        }
    }
}
