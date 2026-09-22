//! Rooted consensus authentication of a Python consumer's exact committed output.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{SignedBlock, consensus_v2::HeightContextId, decode_framed_signed_block},
    bridge::{BridgeFinalityBundle, BridgeFinalityVerifier},
    query::CommittedTransaction,
    transaction::TransactionEntrypoint,
};
use norito::json;
use pyo3::{PyResult, exceptions::PyValueError};

const MAX_FINALITY_CHAIN_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_FINALITY_CHAIN_BUNDLES: usize = 4096;

pub(super) fn authenticate_committed_transaction(
    expected: HashOf<TransactionEntrypoint>,
    transaction_response_bytes: &[u8],
    executed_block_wire: &[u8],
    finality_bundle_chain_json: &str,
    expected_network_id: NetworkId,
    trusted_height_context_id: &str,
) -> PyResult<(CommittedTransaction, SignedBlock, BridgeFinalityBundle)> {
    if finality_bundle_chain_json.len() > MAX_FINALITY_CHAIN_JSON_BYTES {
        return Err(PyValueError::new_err(
            "finality bundle chain exceeds 16 MiB",
        ));
    }
    // JSON's Hash codec accepts only the current checksummed scalar, not bare hex.
    let anchor_value = json::Value::String(trusted_height_context_id.to_owned());
    let anchor: Hash = json::from_value(anchor_value.clone()).map_err(|error| {
        PyValueError::new_err(format!("invalid trusted height context id: {error}"))
    })?;
    if json::to_value(&anchor).map_err(|error| PyValueError::new_err(error.to_string()))?
        != anchor_value
    {
        return Err(PyValueError::new_err(
            "trusted height context id is not canonical",
        ));
    }
    let mut bundles: Vec<BridgeFinalityBundle> = json::from_json(finality_bundle_chain_json)
        .map_err(|error| {
            PyValueError::new_err(format!("invalid finality bundle chain: {error}"))
        })?;
    if bundles.is_empty() || bundles.len() > MAX_FINALITY_CHAIN_BUNDLES {
        return Err(PyValueError::new_err(
            "finality bundle chain must contain 1..4096 bundles",
        ));
    }
    let mut verifier = BridgeFinalityVerifier::with_context(
        expected_network_id,
        HeightContextId(HashOf::from_untyped_unchecked(anchor)),
    );
    for (index, bundle) in bundles.iter().enumerate() {
        verifier.verify_bundle(bundle).map_err(|error| {
            PyValueError::new_err(format!(
                "finality bundle {index} authentication failed: {error}"
            ))
        })?;
    }
    let bundle = bundles.pop().expect("nonempty authenticated bundle chain");
    let commitment = &bundle
        .finality_proof
        .finality_artifact
        .commit_qc
        .execution_commitment;
    // Check the authenticated length and digest against the supplied bytes before decoding.
    if u64::try_from(executed_block_wire.len()).ok() != Some(commitment.executed_block_wire_len)
        || Hash::new(executed_block_wire) != commitment.executed_block_wire_hash
    {
        return Err(PyValueError::new_err(
            "executed wire does not match authenticated execution commitment",
        ));
    }
    let carrier = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(executed_block_wire.len()),
        || decode_framed_signed_block(executed_block_wire),
    )
    .map_err(|error| PyValueError::new_err(format!("invalid executed block wire: {error}")))?;
    let canonical = carrier
        .encode_wire()
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    if canonical.as_slice() != executed_block_wire {
        return Err(PyValueError::new_err(
            "executed block wire is not exact canonical SignedBlockWire",
        ));
    }
    if carrier.header() != bundle.finality_proof.block_header
        || carrier.hash() != bundle.commitment.block_hash
        || carrier.header().height().get() != bundle.commitment.block_height
    {
        return Err(PyValueError::new_err(
            "executed carrier does not match authenticated finality bundle",
        ));
    }
    carrier.validate_proposal_commitments().map_err(|error| {
        PyValueError::new_err(format!(
            "invalid executed carrier proposal commitments: {error}"
        ))
    })?;
    if carrier
        .canonical_proposal_wire_hash()
        .map_err(|error| PyValueError::new_err(error.to_string()))?
        != bundle.finality_proof.finality_artifact.subject.payload_hash
    {
        return Err(PyValueError::new_err(
            "executed proposal does not match authenticated finality subject",
        ));
    }
    let committed = super::decode_single_committed_transaction(transaction_response_bytes)?;
    if committed.entrypoint_hash != expected {
        return Err(PyValueError::new_err(
            "committed transaction response does not match the requested transaction hash",
        ));
    }
    if !committed.verify_inclusion_in_authenticated_execution(&carrier, commitment) {
        return Err(PyValueError::new_err(
            "committed transaction does not verify against authenticated execution commitment",
        ));
    }
    Ok((committed, carrier, bundle))
}

#[cfg(test)]
#[path = "tests/committed_transaction_verification_tests.rs"]
mod tests;
