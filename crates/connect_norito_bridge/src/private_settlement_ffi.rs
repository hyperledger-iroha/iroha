//! Native fail-closed verification boundary for private-settlement Torii views.
//!
//! The bridge accepts the exact bounded JSON bytes received from Torii and
//! delegates typed binding, digest, roster, proof-of-possession, and BLS
//! verification to `iroha_torii_shared`.  It returns only a redacted status
//! code: restricted response material is never copied into a second output
//! buffer or included in an error string.

use core::str::FromStr as _;

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{NetworkId, nexus::ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1};
use iroha_torii_shared::private_settlement_api::{
    PrivateSettlementAuditApprovalRequestV1, PrivateSettlementAuditApprovalResponseV1,
    PrivateSettlementAuditorCapsuleRequestV1, PrivateSettlementAuditorCapsuleResponseV1,
    PrivateSettlementCommitteeProofResponseV1,
    validate_private_settlement_audit_approval_response_v1,
    validate_private_settlement_auditor_capsule_response_v1,
    validate_private_settlement_auditor_identity_v1,
    validate_private_settlement_committee_proof_response_v1,
};
use libc::{c_char, c_int, c_uchar, c_ulong};

use super::{BridgeError, BridgeResult, bridge_result_to_code, read_fixed_array, read_vec_bytes};

/// Maximum accepted JSON bytes for one private-settlement Torii response.
pub const CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum accepted JSON bytes for one exact governed-auditor request.
pub const CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1: usize = 1024 * 1024;
const PRIVATE_SETTLEMENT_PUBLIC_KEY_LITERAL_MAX_BYTES_V1: usize = 1024;

unsafe fn read_bounded_public_bytes(
    pointer: *const c_uchar,
    length: c_ulong,
    maximum: usize,
) -> BridgeResult<Vec<u8>> {
    let length = usize::try_from(length).map_err(|_| BridgeError::PrivateSettlementResponse)?;
    if pointer.is_null() || length == 0 || length > maximum {
        return Err(BridgeError::PrivateSettlementResponse);
    }
    unsafe { read_vec_bytes(pointer, length as c_ulong) }
}

unsafe fn expected_network_id(pointer: *const c_uchar, length: c_ulong) -> BridgeResult<NetworkId> {
    let bytes =
        unsafe { read_fixed_array::<32>(pointer, length, BridgeError::PrivateSettlementResponse) }?;
    Ok(NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::prehashed(bytes)),
    ))
}

unsafe fn requested_payload_digest(pointer: *const c_uchar, length: c_ulong) -> BridgeResult<Hash> {
    unsafe { read_fixed_array::<32>(pointer, length, BridgeError::PrivateSettlementResponse) }
        .map(Hash::prehashed)
}

unsafe fn auditor_signing_key(pointer: *const c_char, length: c_ulong) -> BridgeResult<PublicKey> {
    let bytes = unsafe {
        read_bounded_public_bytes(
            pointer.cast(),
            length,
            PRIVATE_SETTLEMENT_PUBLIC_KEY_LITERAL_MAX_BYTES_V1,
        )
    }?;
    let literal =
        core::str::from_utf8(&bytes).map_err(|_| BridgeError::PrivateSettlementResponse)?;
    if literal.trim() != literal {
        return Err(BridgeError::PrivateSettlementResponse);
    }
    let key = PublicKey::from_str(literal).map_err(|_| BridgeError::PrivateSettlementResponse)?;
    if key.to_string() != literal {
        return Err(BridgeError::PrivateSettlementResponse);
    }
    Ok(key)
}

fn decode_committee_proof_response(
    response_json: &[u8],
) -> BridgeResult<PrivateSettlementCommitteeProofResponseV1> {
    norito::json::from_slice(response_json).map_err(|_| BridgeError::PrivateSettlementResponse)
}

fn decode_auditor_capsule_response(
    response_json: &[u8],
) -> BridgeResult<PrivateSettlementAuditorCapsuleResponseV1> {
    norito::json::from_slice(response_json).map_err(|_| BridgeError::PrivateSettlementResponse)
}

fn decode_auditor_capsule_request(
    request_json: &[u8],
) -> BridgeResult<PrivateSettlementAuditorCapsuleRequestV1> {
    norito::json::from_slice(request_json).map_err(|_| BridgeError::PrivateSettlementResponse)
}

fn decode_audit_approval_request(
    request_json: &[u8],
) -> BridgeResult<PrivateSettlementAuditApprovalRequestV1> {
    norito::json::from_slice(request_json).map_err(|_| BridgeError::PrivateSettlementResponse)
}

fn decode_audit_approval_response(
    response_json: &[u8],
) -> BridgeResult<PrivateSettlementAuditApprovalResponseV1> {
    norito::json::from_slice(response_json).map_err(|_| BridgeError::PrivateSettlementResponse)
}

/// Verify one exact committee proof response using the production Rust rules.
///
/// The caller supplies the configured network genesis hash and requested
/// payload digest as exact 32-byte values.  Zero means success; every failure
/// maps to the same redacted private-settlement bridge error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_private_settlement_committee_proof_response_verify_v1(
    response_json_ptr: *const c_uchar,
    response_json_len: c_ulong,
    expected_network_id_ptr: *const c_uchar,
    expected_network_id_len: c_ulong,
    requested_payload_digest_ptr: *const c_uchar,
    requested_payload_digest_len: c_ulong,
) -> c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let response_json = unsafe {
            read_bounded_public_bytes(
                response_json_ptr,
                response_json_len,
                CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1,
            )
        }?;
        let expected_network =
            unsafe { expected_network_id(expected_network_id_ptr, expected_network_id_len) }?;
        let requested = unsafe {
            requested_payload_digest(requested_payload_digest_ptr, requested_payload_digest_len)
        }?;
        let response = decode_committee_proof_response(&response_json)?;
        validate_private_settlement_committee_proof_response_v1(
            &expected_network,
            requested,
            &response,
        )
        .map_err(|_| BridgeError::PrivateSettlementResponse)
    }));
    result.map_or_else(
        |_| BridgeError::PrivateSettlementResponse.code(),
        bridge_result_to_code,
    )
}

/// Verify one exact auditor capsule response and governed key separation.
///
/// The auditor key is a canonical public-key literal.  Plaintext capsule
/// contents and spending witnesses never cross this boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_private_settlement_auditor_capsule_response_verify_v1(
    response_json_ptr: *const c_uchar,
    response_json_len: c_ulong,
    expected_network_id_ptr: *const c_uchar,
    expected_network_id_len: c_ulong,
    requested_payload_digest_ptr: *const c_uchar,
    requested_payload_digest_len: c_ulong,
    auditor_signing_key_ptr: *const c_char,
    auditor_signing_key_len: c_ulong,
) -> c_int {
    // This legacy ABI cannot authenticate the policy-bearing POST request and
    // therefore deliberately fails closed. Route clients must use the
    // request-aware symbol below.
    let _ = (
        response_json_ptr,
        response_json_len,
        expected_network_id_ptr,
        expected_network_id_len,
        requested_payload_digest_ptr,
        requested_payload_digest_len,
        auditor_signing_key_ptr,
        auditor_signing_key_len,
    );
    BridgeError::PrivateSettlementResponse.code()
}

/// Verify one exact auditor capsule request/response pair and governed identity.
///
/// The request bytes are the exact bounded JSON bytes signed and sent in the
/// policy-bearing POST. The auditor key is a canonical public-key literal.
/// Plaintext capsule contents and spending witnesses never cross this boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1(
    response_json_ptr: *const c_uchar,
    response_json_len: c_ulong,
    request_json_ptr: *const c_uchar,
    request_json_len: c_ulong,
    expected_network_id_ptr: *const c_uchar,
    expected_network_id_len: c_ulong,
    requested_payload_digest_ptr: *const c_uchar,
    requested_payload_digest_len: c_ulong,
    auditor_signing_key_ptr: *const c_char,
    auditor_signing_key_len: c_ulong,
) -> c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let response_json = unsafe {
            read_bounded_public_bytes(
                response_json_ptr,
                response_json_len,
                CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1,
            )
        }?;
        let request_json = unsafe {
            read_bounded_public_bytes(
                request_json_ptr,
                request_json_len,
                CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1,
            )
        }?;
        let expected_network =
            unsafe { expected_network_id(expected_network_id_ptr, expected_network_id_len) }?;
        let requested = unsafe {
            requested_payload_digest(requested_payload_digest_ptr, requested_payload_digest_len)
        }?;
        let auditor_key =
            unsafe { auditor_signing_key(auditor_signing_key_ptr, auditor_signing_key_len) }?;
        let request = decode_auditor_capsule_request(&request_json)?;
        let response = decode_auditor_capsule_response(&response_json)?;
        validate_private_settlement_auditor_capsule_response_v1(
            &expected_network,
            requested,
            &request,
            &response,
        )
        .map_err(|_| BridgeError::PrivateSettlementResponse)?;
        validate_private_settlement_auditor_identity_v1(&auditor_key, &response)
            .map_err(|_| BridgeError::PrivateSettlementResponse)
    }));
    result.map_or_else(
        |_| BridgeError::PrivateSettlementResponse.code(),
        bridge_result_to_code,
    )
}

/// Verify one exact approval request/acknowledgement pair.
///
/// This verifies the locally prepared approval signature with the advertised
/// auditor key before validating the responder's roster membership, PoP,
/// recomputed acknowledgement digest, and BLS signature.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_private_settlement_audit_approval_response_verify_v1(
    response_json_ptr: *const c_uchar,
    response_json_len: c_ulong,
    request_json_ptr: *const c_uchar,
    request_json_len: c_ulong,
    expected_network_id_ptr: *const c_uchar,
    expected_network_id_len: c_ulong,
    requested_payload_digest_ptr: *const c_uchar,
    requested_payload_digest_len: c_ulong,
    auditor_signing_key_ptr: *const c_char,
    auditor_signing_key_len: c_ulong,
) -> c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let response_json = unsafe {
            read_bounded_public_bytes(
                response_json_ptr,
                response_json_len,
                CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1,
            )
        }?;
        let request_json = unsafe {
            read_bounded_public_bytes(
                request_json_ptr,
                request_json_len,
                CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1,
            )
        }?;
        let expected_network =
            unsafe { expected_network_id(expected_network_id_ptr, expected_network_id_len) }?;
        let requested = unsafe {
            requested_payload_digest(requested_payload_digest_ptr, requested_payload_digest_len)
        }?;
        let auditor_key =
            unsafe { auditor_signing_key(auditor_signing_key_ptr, auditor_signing_key_len) }?;
        let request = decode_audit_approval_request(&request_json)?;
        if request.approval.body.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1
            || request.approval.body.network_id != expected_network
            || request
                .approval
                .signature
                .verify(&auditor_key, &request.approval.body)
                .is_err()
        {
            return Err(BridgeError::PrivateSettlementResponse);
        }
        let response = decode_audit_approval_response(&response_json)?;
        validate_private_settlement_audit_approval_response_v1(requested, &request, &response)
            .map(|_| ())
            .map_err(|_| BridgeError::PrivateSettlementResponse)
    }));
    result.map_or_else(
        |_| BridgeError::PrivateSettlementResponse.code(),
        bridge_result_to_code,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_ERROR: c_int = -507;

    #[test]
    fn committee_proof_bridge_rejects_untyped_json() {
        let json = b"{}";
        let digest = [7_u8; 32];
        let code = unsafe {
            connect_norito_private_settlement_committee_proof_response_verify_v1(
                json.as_ptr(),
                json.len() as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
            )
        };
        assert_eq!(code, EXPECTED_ERROR);
    }

    #[test]
    fn auditor_capsule_bridge_rejects_noncanonical_key() {
        let json = b"{}";
        let digest = [8_u8; 32];
        let key = b" not-a-key ";
        let code = unsafe {
            connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1(
                json.as_ptr(),
                json.len() as c_ulong,
                json.as_ptr(),
                json.len() as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
                key.as_ptr().cast(),
                key.len() as c_ulong,
            )
        };
        assert_eq!(code, EXPECTED_ERROR);
    }

    #[test]
    fn legacy_auditor_capsule_bridge_fails_closed_without_request() {
        let json = b"{}";
        let digest = [8_u8; 32];
        let key = b"not-a-key";
        let code = unsafe {
            connect_norito_private_settlement_auditor_capsule_response_verify_v1(
                json.as_ptr(),
                json.len() as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
                key.as_ptr().cast(),
                key.len() as c_ulong,
            )
        };
        assert_eq!(code, EXPECTED_ERROR);
    }

    #[test]
    fn approval_bridge_rejects_oversized_request_before_decode() {
        let json = b"{}";
        let digest = [9_u8; 32];
        let key = b"not-a-key";
        let code = unsafe {
            connect_norito_private_settlement_audit_approval_response_verify_v1(
                json.as_ptr(),
                json.len() as c_ulong,
                json.as_ptr(),
                (CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1 + 1) as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
                digest.as_ptr(),
                digest.len() as c_ulong,
                key.as_ptr().cast(),
                key.len() as c_ulong,
            )
        };
        assert_eq!(code, EXPECTED_ERROR);
    }
}
