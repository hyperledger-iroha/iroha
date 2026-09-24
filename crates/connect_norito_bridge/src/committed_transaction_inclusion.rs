//! Current selective committed-transaction proof for mobile clients.
//!
//! The Torii response is only a candidate. The caller must pin both the native
//! NetworkId and the first height-context anchor independently of that response.

use std::{ptr, slice};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    bridge::{BridgeFinalityBundle, BridgeFinalityVerifier},
    query::{CommittedTransaction, QueryOutputBatchBox, QueryResponse},
    transaction::TransactionEntrypoint,
};
use libc::{c_int, c_uchar, c_ulong};
use norito::json;

pub(crate) const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_ROW_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_CHAIN_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHAIN_BUNDLES: usize = 4096;
const ERR_COMMITTED_INCLUSION: c_int = -508;

pub(crate) struct VerifiedCommittedTransaction {
    pub row: Vec<u8>,
    pub output_hash: [u8; 32],
    pub block_hash: [u8; 32],
    pub block_height: u64,
    pub result_ok: bool,
}

fn decode_candidate_response(
    response_bytes: &[u8],
) -> Result<Option<CommittedTransaction>, String> {
    let response: QueryResponse = norito::decode_canonical_with_limits(
        response_bytes,
        norito::canonical_decode_limits(response_bytes.len()),
    )
    .map_err(|error| format!("invalid committed transaction query response: {error}"))?;
    let QueryResponse::Iterable(output) = response else {
        return Err("committed transaction response must be iterable".into());
    };
    if output.has_more
        || output.continue_cursor.is_some()
        || output
            .remaining_items
            .is_some_and(|remaining| remaining != 0)
    {
        return Err("committed transaction response must contain exactly one page".into());
    }
    let mut rows = Vec::new();
    let mut committed_batches = 0_usize;
    for batch in output.batch {
        let QueryOutputBatchBox::CommittedTransaction(mut batch) = batch else {
            return Err("committed transaction response has a foreign batch type".into());
        };
        committed_batches += 1;
        rows.append(&mut batch);
    }
    if committed_batches != 1 || rows.len() > 1 {
        return Err("committed transaction response must contain zero or one exact row".into());
    }
    Ok(rows.pop())
}

fn decode_single_response(response_bytes: &[u8]) -> Result<CommittedTransaction, String> {
    decode_candidate_response(response_bytes)?
        .ok_or_else(|| "committed transaction response has no selected row".into())
}

/// Untrusted carrier block hash used only to locate consecutive finality
/// bundles. It does not confer finality or authorize a transaction result.
pub(crate) fn candidate_block_hash(
    response_bytes: &[u8],
    expected_transaction_hash: HashOf<TransactionEntrypoint>,
) -> Result<Option<[u8; 32]>, String> {
    if response_bytes.is_empty() || response_bytes.len() > MAX_RESPONSE_BYTES {
        return Err("committed transaction response exceeds its bound".into());
    }
    let Some(row) = decode_candidate_response(response_bytes)? else {
        return Ok(None);
    };
    if row.entrypoint_hash != expected_transaction_hash {
        return Err("candidate row does not match requested transaction hash".into());
    }
    Ok(Some(*row.block_hash.as_ref()))
}

/// Decode only the exact untrusted carrier hash from a one-row response. A
/// canonical empty committed-transaction page returns status 1 and zeroed
/// output. This is a routing hint, never an authenticated proof. A caller must
/// verify the finality chain and selected row with the verifier below before use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_committed_transaction_candidate_block_hash_v1(
    response_ptr: *const c_uchar,
    response_len: c_ulong,
    expected_transaction_hash_ptr: *const c_uchar,
    expected_transaction_hash_len: c_ulong,
    out_block_hash_32: *mut c_uchar,
) -> c_int {
    if !out_block_hash_32.is_null() {
        unsafe { ptr::write_bytes(out_block_hash_32, 0, 32) };
    }
    let result = std::panic::catch_unwind(|| {
        let response_len = usize::try_from(response_len)
            .map_err(|_| "candidate response length exceeds platform size")?;
        if response_ptr.is_null()
            || expected_transaction_hash_ptr.is_null()
            || out_block_hash_32.is_null()
            || response_len == 0
            || response_len > MAX_RESPONSE_BYTES
            || expected_transaction_hash_len != 32
        {
            return Err("invalid candidate block-hash argument".to_owned());
        }
        let transaction_bytes: [u8; 32] = unsafe {
            slice::from_raw_parts(expected_transaction_hash_ptr, 32)
                .try_into()
                .expect("fixed transaction hash length")
        };
        if transaction_bytes[31] & 1 == 0 {
            return Err("unmarked transaction hash".to_owned());
        }
        candidate_block_hash(
            unsafe { slice::from_raw_parts(response_ptr, response_len) },
            HashOf::from_untyped_unchecked(Hash::prehashed(transaction_bytes)),
        )
    });
    let Ok(Ok(candidate)) = result else {
        return ERR_COMMITTED_INCLUSION;
    };
    if let Some(hash) = candidate {
        unsafe { ptr::copy_nonoverlapping(hash.as_ptr(), out_block_hash_32, 32) };
        0
    } else {
        1
    }
}

pub(crate) fn verify_committed_transaction_inclusion(
    response_bytes: &[u8],
    chain_json: &[u8],
    expected_network_id: NetworkId,
    trusted_height_context_id: &str,
    expected_transaction_hash: HashOf<TransactionEntrypoint>,
) -> Result<VerifiedCommittedTransaction, String> {
    if response_bytes.is_empty() || response_bytes.len() > MAX_RESPONSE_BYTES {
        return Err("committed transaction response exceeds its bound".into());
    }
    if chain_json.is_empty() || chain_json.len() > MAX_CHAIN_JSON_BYTES {
        return Err("finality bundle chain exceeds its bound".into());
    }
    let anchor_value = json::Value::String(trusted_height_context_id.to_owned());
    let anchor: Hash = json::from_value(anchor_value.clone())
        .map_err(|error| format!("invalid trusted height context id: {error}"))?;
    if json::to_value(&anchor).map_err(|error| error.to_string())? != anchor_value {
        return Err("trusted height context id is not canonical".into());
    }
    let chain_json =
        std::str::from_utf8(chain_json).map_err(|_| "finality bundle chain is not UTF-8")?;
    let chain: Vec<BridgeFinalityBundle> = json::from_json(chain_json)
        .map_err(|error| format!("invalid finality bundle chain: {error}"))?;
    if chain.is_empty() || chain.len() > MAX_CHAIN_BUNDLES {
        return Err("finality bundle chain must contain 1..4096 bundles".into());
    }
    let mut verifier = BridgeFinalityVerifier::with_context(
        expected_network_id,
        HeightContextId(HashOf::from_untyped_unchecked(anchor)),
    );
    for (index, bundle) in chain.iter().enumerate() {
        verifier
            .verify_bundle(bundle)
            .map_err(|error| format!("finality bundle {index} failed: {error}"))?;
    }
    let last = chain.last().expect("nonempty chain");
    let commitment = &last
        .finality_proof
        .finality_artifact
        .commit_qc
        .execution_commitment;
    let header = &last.finality_proof.block_header;
    if header.hash() != last.commitment.block_hash
        || header.height().get() != last.commitment.block_height
    {
        return Err("authenticated header differs from finality commitment".into());
    }
    let committed = decode_single_response(response_bytes)?;
    if committed.entrypoint_hash != expected_transaction_hash {
        return Err("committed row does not match requested transaction hash".into());
    }
    if !committed.verify_selective_in_authenticated_execution(
        &expected_network_id,
        header,
        commitment,
    ) {
        return Err("committed row does not verify against authenticated execution".into());
    }
    let row = norito::to_bytes(&committed)
        .map_err(|error| format!("failed to encode canonical committed row: {error}"))?;
    if row.is_empty() || row.len() > MAX_ROW_BYTES {
        return Err("canonical committed row exceeds its bound".into());
    }
    Ok(VerifiedCommittedTransaction {
        row,
        output_hash: *committed.output_hash.as_ref(),
        block_hash: *committed.block_hash.as_ref(),
        block_height: last.commitment.block_height,
        result_ok: committed.result().0.is_ok(),
    })
}

/// Authenticate one current selective `CommittedTransaction` row against an
/// independently pinned NetworkId, height context and exact transaction hash.
/// On success the returned row is bare canonical Norito and must be freed with
/// `connect_norito_free`. Every output is cleared on failure.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn connect_norito_verify_committed_transaction_inclusion_v1(
    response_ptr: *const c_uchar,
    response_len: c_ulong,
    chain_json_ptr: *const c_uchar,
    chain_json_len: c_ulong,
    expected_network_id_ptr: *const c_uchar,
    expected_network_id_len: c_ulong,
    trusted_height_context_id_ptr: *const c_uchar,
    trusted_height_context_id_len: c_ulong,
    expected_transaction_hash_ptr: *const c_uchar,
    expected_transaction_hash_len: c_ulong,
    out_row_ptr: *mut *mut c_uchar,
    out_row_len: *mut c_ulong,
    out_output_hash: *mut c_uchar,
    out_block_hash: *mut c_uchar,
    out_block_height: *mut u64,
    out_result_ok: *mut c_uchar,
) -> c_int {
    super::clear_bridge_output(out_row_ptr, out_row_len);
    if !out_output_hash.is_null() {
        unsafe { ptr::write_bytes(out_output_hash, 0, 32) };
    }
    if !out_block_hash.is_null() {
        unsafe { ptr::write_bytes(out_block_hash, 0, 32) };
    }
    if !out_block_height.is_null() {
        unsafe { *out_block_height = 0 };
    }
    if !out_result_ok.is_null() {
        unsafe { *out_result_ok = 0 };
    }
    let result = std::panic::catch_unwind(|| {
        if response_ptr.is_null()
            || chain_json_ptr.is_null()
            || expected_network_id_ptr.is_null()
            || trusted_height_context_id_ptr.is_null()
            || expected_transaction_hash_ptr.is_null()
            || out_row_ptr.is_null()
            || out_row_len.is_null()
            || out_output_hash.is_null()
            || out_block_hash.is_null()
            || out_block_height.is_null()
            || out_result_ok.is_null()
        {
            return Err("null committed inclusion argument".to_owned());
        }
        let response_len = usize::try_from(response_len).map_err(|error| error.to_string())?;
        let chain_len = usize::try_from(chain_json_len).map_err(|error| error.to_string())?;
        if response_len == 0
            || response_len > MAX_RESPONSE_BYTES
            || chain_len == 0
            || chain_len > MAX_CHAIN_JSON_BYTES
            || expected_network_id_len != 32
            || expected_transaction_hash_len != 32
            || trusted_height_context_id_len == 0
            || trusted_height_context_id_len > 128
        {
            return Err("invalid committed inclusion input length".to_owned());
        }
        let network_bytes: [u8; 32] = unsafe {
            slice::from_raw_parts(expected_network_id_ptr, 32)
                .try_into()
                .expect("fixed network byte count")
        };
        let transaction_hash_bytes: [u8; 32] = unsafe {
            slice::from_raw_parts(expected_transaction_hash_ptr, 32)
                .try_into()
                .expect("fixed transaction hash byte count")
        };
        if network_bytes[31] & 1 == 0 || transaction_hash_bytes[31] & 1 == 0 {
            return Err("unmarked network or transaction hash".to_owned());
        }
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::prehashed(network_bytes),
        ));
        let expected_transaction_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed(transaction_hash_bytes));
        let anchor = std::str::from_utf8(unsafe {
            slice::from_raw_parts(
                trusted_height_context_id_ptr,
                trusted_height_context_id_len as usize,
            )
        })
        .map_err(|error| error.to_string())?;
        verify_committed_transaction_inclusion(
            unsafe { slice::from_raw_parts(response_ptr, response_len) },
            unsafe { slice::from_raw_parts(chain_json_ptr, chain_len) },
            network_id,
            anchor,
            expected_transaction_hash,
        )
    });
    let Ok(Ok(verified)) = result else {
        return ERR_COMMITTED_INCLUSION;
    };
    if unsafe { super::write_bytes(out_row_ptr, out_row_len, &verified.row) }.is_err() {
        return ERR_COMMITTED_INCLUSION;
    }
    unsafe {
        ptr::copy_nonoverlapping(verified.output_hash.as_ptr(), out_output_hash, 32);
        ptr::copy_nonoverlapping(verified.block_hash.as_ptr(), out_block_hash, 32);
        *out_block_height = verified.block_height;
        *out_result_ok = u8::from(verified.result_ok);
    }
    0
}

#[cfg(test)]
#[path = "committed_transaction_inclusion/tests.rs"]
mod tests;
