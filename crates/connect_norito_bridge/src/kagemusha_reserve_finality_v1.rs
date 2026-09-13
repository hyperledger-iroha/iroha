//! Native finality for an exact, independently retained Offline reserve intent.
//!
//! Response coordinates are lookup hints only. A caller must independently authenticate its
//! exact network/height/context before verification. Success authenticates the finalized reserve
//! receipt and returns an existing canonical wire payload; local Core still admits its release,
//! hardware profile and monetary proof before mint staging or result retirement.

use super::*;
use iroha_data_model::{
    block::consensus_v2::HeightContextId,
    isi::kagemusha_v1::{
        KagemushaFinalityTrustAnchorV1, KagemushaOperationKindV1, KagemushaOperationResultV1,
        KagemushaOperationStateV1,
    },
};
use iroha_torii_shared::kagemusha_api::{
    KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1, KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1,
    KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1, decode_kagemusha_redemption_request_v1,
    decode_kagemusha_top_up_request_v1, decode_unverified_kagemusha_operation_status_json_v1,
};

struct ExpectedRequest<'a> {
    kind: KagemushaOperationKindV1,
    operation_id: [u8; 32],
    network_id: NetworkId,
    canonical: &'a [u8],
}

fn trusted_anchor(
    network: [u8; 32],
    height: u64,
    context: [u8; 32],
) -> BridgeResult<KagemushaFinalityTrustAnchorV1> {
    // Hash::prehashed sets a marker bit. Externally trusted coordinates must instead decode
    // exactly, without silently selecting a different network or consensus context.
    let network_id = network_id_from_raw_bytes(&network).map_err(|_| BridgeError::KagemushaV1)?;
    let context_hash = hex::encode(context)
        .parse::<Hash>()
        .map_err(|_| BridgeError::KagemushaV1)?;
    let anchor = KagemushaFinalityTrustAnchorV1 {
        network_id,
        block_height: height,
        height_context_id: HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
            context_hash,
        )),
    };
    anchor.validate().map_err(|_| BridgeError::KagemushaV1)?;
    Ok(anchor)
}

fn expected_request(kind: u8, canonical: &[u8]) -> BridgeResult<ExpectedRequest<'_>> {
    let (kind, operation_id, network_id) = match kind {
        0 => {
            let request = decode_kagemusha_top_up_request_v1(canonical)
                .map_err(|_| BridgeError::KagemushaV1)?;
            (
                KagemushaOperationKindV1::TopUp,
                request.operation_id,
                request.network_id,
            )
        }
        1 => {
            let request = decode_kagemusha_redemption_request_v1(canonical)
                .map_err(|_| BridgeError::KagemushaV1)?;
            (
                KagemushaOperationKindV1::Redemption,
                request.operation_id,
                request.lifecycle().network_id,
            )
        }
        _ => return Err(BridgeError::KagemushaV1),
    };
    Ok(ExpectedRequest {
        kind,
        operation_id,
        network_id,
        canonical,
    })
}

fn anchor_hint_json(response: &[u8]) -> BridgeResult<Vec<u8>> {
    let unverified = decode_unverified_kagemusha_operation_status_json_v1(response)
        .map_err(|_| BridgeError::KagemushaV1)?;
    let hint = unverified.finality_anchor_hint();
    let value = match hint {
        None => norito::json::Value::Null,
        Some(hint) => norito::json!({
            "version": 1,
            "network_id": (hex::encode(hint.network_id.as_bytes())),
            "block_height": (hint.block_height.to_string()),
            "height_context_id": (hex::encode(hint.height_context_id.0.as_ref())),
        }),
    };
    norito::json::to_vec(&value).map_err(|_| BridgeError::KagemushaV1)
}

fn verified_payload(
    response: &[u8],
    expected: &ExpectedRequest<'_>,
    anchor: &KagemushaFinalityTrustAnchorV1,
) -> BridgeResult<Vec<u8>> {
    anchor.validate().map_err(|_| BridgeError::KagemushaV1)?;
    if anchor.network_id != expected.network_id {
        return Err(BridgeError::KagemushaV1);
    }
    let unverified = decode_unverified_kagemusha_operation_status_json_v1(response)
        .map_err(|_| BridgeError::KagemushaV1)?;
    if unverified.state() != KagemushaOperationStateV1::Applied
        || unverified.kind() != expected.kind
        || unverified.operation_id() != expected.operation_id
    {
        return Err(BridgeError::KagemushaV1);
    }
    // The authoritative Rust implementation checks the independently pinned context, certificate
    // and exact reserve witness. No Java/Swift callback can substitute a structural verdict here.
    let status = unverified
        .verify_against(anchor)
        .map_err(|_| BridgeError::KagemushaV1)?;
    let (request, payload, maximum) = match status.result.ok_or(BridgeError::KagemushaV1)? {
        KagemushaOperationResultV1::TopUp(result) => (
            norito::encode_canonical(&result.request),
            norito::encode_canonical(&result.mint_credit),
            iroha_data_model::kagemusha::KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
        ),
        KagemushaOperationResultV1::Redemption(result) => (
            norito::encode_canonical(&result.request),
            norito::encode_canonical(&result.request.voucher),
            iroha_data_model::kagemusha::KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1,
        ),
    };
    if request.map_err(|_| BridgeError::KagemushaV1)?.as_slice() != expected.canonical {
        return Err(BridgeError::KagemushaV1);
    }
    let payload = payload.map_err(|_| BridgeError::KagemushaV1)?;
    if payload.is_empty() || payload.len() > maximum {
        return Err(BridgeError::KagemushaV1);
    }
    Ok(payload)
}

/// Decode bounded untrusted lookup coordinates; pending/rejected responses return JSON `null`.
///
/// The JSON object has version 1 and exact network/context hex plus unsigned decimal block height.
/// It contains no monetary result and must never become its own verification trust anchor.
/// Output is empty on failure and freed with `connect_norito_free` after success.
///
/// # Safety
/// Input must point to `response_json_len` readable bytes. Non-null output pointers must be valid
/// writable locations and must not alias each other or input. An existing allocation is caller-owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_reserve_finality_hint_v1(
    response_json: *const c_uchar,
    response_json_len: c_ulong,
    out_json: *mut *mut c_uchar,
    out_json_len: *mut c_ulong,
) -> c_int {
    clear_bridge_output(out_json, out_json_len);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        clear_bridge_output_or_null(out_json, out_json_len)?;
        let response = unsafe {
            read_kagemusha_v1_bytes(
                response_json,
                response_json_len,
                KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
            )
        }?;
        let json = anchor_hint_json(response)?;
        unsafe { write_bytes_bridge(out_json, out_json_len, &json) }
    }))
    .unwrap_or(Err(BridgeError::KagemushaV1));
    bridge_result_to_code(result)
}

/// Authenticate an applied response against an independently resolved anchor and exact saved intent.
///
/// Kind is 0 for top-up or 1 for redemption; every other value is rejected. The expected request is
/// the existing canonical V1 request, never a request selected from the response. Returns canonical
/// `KagemushaMintCreditV1` for top-up or canonical `KagemushaRedemptionVoucherV1` for redemption.
/// Pending/rejected responses cannot yield a monetary payload. Output is empty on failure.
/// Save the original response and independently authenticated anchor provenance before native
/// result retirement; a cached output payload is not a replacement for re-verifying that evidence.
///
/// # Safety
/// Every input must point to its declared readable range. Non-null output pointers must be writable
/// and must not alias each other or input. Release successful output with `connect_norito_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_reserve_finality_verify_v1(
    response_json: *const c_uchar,
    response_json_len: c_ulong,
    expected_kind: u8,
    expected_request_ptr: *const c_uchar,
    expected_request_len: c_ulong,
    trusted_network_id: *const c_uchar,
    trusted_network_id_len: c_ulong,
    trusted_block_height: u64,
    trusted_context_id: *const c_uchar,
    trusted_context_id_len: c_ulong,
    out_payload: *mut *mut c_uchar,
    out_payload_len: *mut c_ulong,
) -> c_int {
    clear_bridge_output(out_payload, out_payload_len);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        clear_bridge_output_or_null(out_payload, out_payload_len)?;
        let maximum = match expected_kind {
            0 => KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
            1 => KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1,
            _ => return Err(BridgeError::KagemushaV1),
        };
        let response = unsafe {
            read_kagemusha_v1_bytes(
                response_json,
                response_json_len,
                KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
            )
        }?;
        let request = unsafe {
            read_kagemusha_v1_bytes(expected_request_ptr, expected_request_len, maximum)
        }?;
        let network = unsafe {
            read_fixed_array::<32>(
                trusted_network_id,
                trusted_network_id_len,
                BridgeError::KagemushaV1,
            )
        }?;
        let context = unsafe {
            read_fixed_array::<32>(
                trusted_context_id,
                trusted_context_id_len,
                BridgeError::KagemushaV1,
            )
        }?;
        // Validate all trust coordinates before decoding an expensive caller request.
        let anchor = trusted_anchor(network, trusted_block_height, context)?;
        let expected = expected_request(expected_kind, request)?;
        let payload = verified_payload(response, &expected, &anchor)?;
        unsafe { write_bytes_bridge(out_payload, out_payload_len, &payload) }
    }))
    .unwrap_or(Err(BridgeError::KagemushaV1));
    bridge_result_to_code(result)
}

#[cfg(test)]
#[path = "kagemusha_reserve_finality_v1_tests.rs"]
mod tests;

/// The client work bound is shared with the existing detached transaction bridge.
/// It is not a network protocol maximum or a replacement for the node's ingress limit.
const MAX_TOP_UP_SIGNED_TRANSACTION_BYTES: usize = DETACHED_TRANSACTION_SCAFFOLD_MAX_BYTES;

fn validate_top_up_submission(signed: &[u8], expected: &[u8]) -> BridgeResult<()> {
    if signed.is_empty() || signed.len() > MAX_TOP_UP_SIGNED_TRANSACTION_BYTES {
        return Err(BridgeError::KagemushaV1);
    }
    let expected_request =
        decode_kagemusha_top_up_request_v1(expected).map_err(|_| BridgeError::KagemushaV1)?;
    let transaction = decode_signed_transaction(signed).map_err(|_| BridgeError::KagemushaV1)?;
    let actual_request =
        iroha_torii_shared::kagemusha_api::validate_kagemusha_top_up_signed_transaction_v1(
            &expected_request.network_id,
            &transaction,
        )
        .map_err(|_| BridgeError::KagemushaV1)?;
    let actual = norito::encode_canonical(actual_request).map_err(|_| BridgeError::KagemushaV1)?;
    if actual.as_slice() != expected {
        return Err(BridgeError::KagemushaV1);
    }
    Ok(())
}

/// Validate a canonical payer-signed top-up against the complete original reviewed request.
///
/// Checks native signature, exact network, QueuePlanSynced admission, one top-up instruction,
/// authority=payer, request shape and byte-for-byte canonical request equality. No result or
/// monetary authority is released. The caller still owns fee review, session, bank approval and
/// persistence before dispatch. Returns zero only on success; there is no structural fallback.
///
/// # Safety
/// Each input must point to its declared readable range. Inputs are borrowed only for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_top_up_signed_request_validate_v1(
    signed_transaction_ptr: *const c_uchar,
    signed_transaction_len: c_ulong,
    expected_request_ptr: *const c_uchar,
    expected_request_len: c_ulong,
) -> c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let signed = unsafe {
            read_kagemusha_v1_bytes(
                signed_transaction_ptr,
                signed_transaction_len,
                MAX_TOP_UP_SIGNED_TRANSACTION_BYTES,
            )
        }?;
        let expected = unsafe {
            read_kagemusha_v1_bytes(
                expected_request_ptr,
                expected_request_len,
                KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
            )
        }?;
        validate_top_up_submission(signed, expected)
    }))
    .unwrap_or(Err(BridgeError::KagemushaV1));
    bridge_result_to_code(result)
}
