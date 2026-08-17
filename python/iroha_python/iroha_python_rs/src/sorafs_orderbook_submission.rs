//! Strict native inspection for Python SoraFS orderbook submission.
#[rustfmt::skip]
use iroha_data_model::{sorafs::orderbook_submission::{SorafsOrderbookSubmissionRouteV1, decode_and_verify_sorafs_orderbook_submission_receipt_v1, inspect_sorafs_orderbook_submission_for_discriminant_v1 as inspect_submission, parse_sorafs_orderbook_receipt_signer_v1, parse_sorafs_orderbook_submission_identity_v1}, transaction::TransactionSubmissionReceipt};
#[rustfmt::skip]
use pyo3::{Py, PyErr, PyResult, Python, exceptions::PyValueError, pyfunction, types::{PyDict, PyDictMethods}};
use crate::PyNetworkId;
fn invalid(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}
#[pyfunction]
#[pyo3(name = "inspect_sorafs_orderbook_submission_v1")]
#[rustfmt::skip]
pub(crate) fn inspect_sorafs_orderbook_submission_v1_py(
    py: Python<'_>,
    route: &str,
    expected_network_id: &PyNetworkId,
    expected_chain_discriminant: u16,
    expected_receipt_signer: &str,
    signed_transaction_versioned: &[u8],
) -> PyResult<Py<PyDict>> {
    let route = SorafsOrderbookSubmissionRouteV1::parse_sdk_label(route).map_err(|error| invalid(error.to_string()))?;
    parse_sorafs_orderbook_receipt_signer_v1(expected_receipt_signer).ok_or_else(|| invalid("expected_receipt_signer must be exact canonical text"))?;
    let validated = inspect_submission(signed_transaction_versioned, route, expected_network_id.as_inner(), expected_chain_discriminant).map_err(|error| invalid(error.to_string()))?;
    let identity = validated.identity;
    let result = PyDict::new(py);
    result.set_item("entrypoint_hash", identity.entrypoint_hash.to_string())?;
    result.set_item("signed_transaction_hash", identity.signed_transaction_hash.to_string())?;
    Ok(result.unbind())
}
#[pyfunction]
#[pyo3(name = "verify_sorafs_orderbook_submission_receipt_v1")]
#[rustfmt::skip]
pub(crate) fn verify_sorafs_orderbook_submission_receipt_v1_py(
    receipt_norito: &[u8],
    entrypoint_hash: &str,
    signed_transaction_hash: &str,
    expected_receipt_signer: &str,
) -> PyResult<String> {
    let identity = parse_sorafs_orderbook_submission_identity_v1(entrypoint_hash, signed_transaction_hash).ok_or_else(|| invalid("receipt identities must be exact canonical text"))?;
    let signer = parse_sorafs_orderbook_receipt_signer_v1(expected_receipt_signer).ok_or_else(|| invalid("expected_receipt_signer must be exact canonical text"))?;
    let receipt = decode_and_verify_sorafs_orderbook_submission_receipt_v1(receipt_norito, &identity, &signer).map_err(|error| invalid(error.to_string()))?;
    norito::json::to_json(&receipt).map_err(|error| invalid(error.to_string()))
}
#[pyfunction]
#[pyo3(name = "decode_transaction_receipt_json")]
pub(crate) fn decode_transaction_receipt_json_py(receipt_bytes: &[u8]) -> PyResult<String> {
    let receipt: TransactionSubmissionReceipt = norito::decode_from_bytes(receipt_bytes)
        .map_err(|error| invalid(format!("failed to decode transaction receipt: {error}")))?;
    norito::json::to_json(&receipt)
        .map_err(|error| invalid(format!("failed to serialize receipt: {error}")))
}
