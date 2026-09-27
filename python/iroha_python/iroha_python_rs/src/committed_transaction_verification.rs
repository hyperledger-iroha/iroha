//! Rooted consensus authentication of a Python consumer's exact committed output.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
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
    finality_bundle_chain_json: &str,
    expected_network_id: NetworkId,
    trusted_height_context_id: &str,
) -> PyResult<(CommittedTransaction, BridgeFinalityBundle)> {
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
    let header = &bundle.finality_proof.block_header;
    if header.hash() != bundle.commitment.block_hash
        || header.height().get() != bundle.commitment.block_height
    {
        return Err(PyValueError::new_err(
            "authenticated header differs from finality commitment",
        ));
    }
    if transaction_response_bytes.is_empty() || transaction_response_bytes.len() > 32 * 1024 * 1024
    {
        return Err(PyValueError::new_err(
            "selective query response must contain 1..32 MiB",
        ));
    }
    let committed = super::decode_single_committed_transaction(transaction_response_bytes)?;
    if committed.entrypoint_hash != expected {
        return Err(PyValueError::new_err(
            "committed transaction response does not match the requested transaction hash",
        ));
    }
    if !committed.verify_selective_in_authenticated_execution(
        &expected_network_id,
        header,
        commitment,
    ) {
        return Err(PyValueError::new_err(
            "committed transaction does not verify against authenticated execution commitment",
        ));
    }
    Ok((committed, bundle))
}

#[cfg(test)]
#[path = "tests/committed_transaction_verification_tests.rs"]
mod tests;
