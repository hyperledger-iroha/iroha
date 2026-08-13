//! Strict native inspection for Python SoraFS orderbook submission.

use std::str::FromStr as _;

use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    sorafs::orderbook_submission::{
        SorafsOrderbookSubmissionIdentityV1, SorafsOrderbookSubmissionRouteV1,
        decode_and_verify_sorafs_orderbook_submission_receipt_v1,
        inspect_sorafs_orderbook_submission_v1 as inspect_submission,
    },
    transaction::{SignedTransaction, TransactionEntrypoint, TransactionSubmissionReceipt},
};
use pyo3::{
    Py, PyErr, PyResult, Python,
    exceptions::PyValueError,
    pyfunction,
    types::{PyDict, PyDictMethods},
};

use crate::PyNetworkId;

fn invalid(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}

fn parse_exact<T>(literal: &str, label: &str) -> PyResult<T>
where
    T: FromStr + ToString,
{
    let parsed = literal
        .parse::<T>()
        .map_err(|_| invalid(format!("{label} is invalid")))?;
    if parsed.to_string() != literal {
        return Err(invalid(format!("{label} must be exact canonical text")));
    }
    Ok(parsed)
}

fn parse_identity(
    tx_hash: &str,
    entrypoint_hash: &str,
    signed_transaction_hash: &str,
) -> PyResult<SorafsOrderbookSubmissionIdentityV1> {
    Ok(SorafsOrderbookSubmissionIdentityV1 {
        tx_hash: parse_exact::<HashOf<SignedTransaction>>(tx_hash, "tx_hash")?,
        entrypoint_hash: parse_exact::<HashOf<TransactionEntrypoint>>(
            entrypoint_hash,
            "entrypoint_hash",
        )?,
        signed_transaction_hash: parse_exact::<HashOf<SignedTransaction>>(
            signed_transaction_hash,
            "signed_transaction_hash",
        )?,
    })
}

#[pyfunction]
#[pyo3(name = "inspect_sorafs_orderbook_submission_v1")]
pub(crate) fn inspect_sorafs_orderbook_submission_v1_py(
    py: Python<'_>,
    route: &str,
    expected_network_id: &PyNetworkId,
    expected_receipt_signer: &str,
    signed_transaction_versioned: &[u8],
) -> PyResult<Py<PyDict>> {
    let route = SorafsOrderbookSubmissionRouteV1::parse_sdk_label(route)
        .map_err(|error| invalid(error.to_string()))?;
    let _expected_receipt_signer =
        parse_exact::<PublicKey>(expected_receipt_signer, "expected_receipt_signer")?;
    let identity = inspect_submission(
        signed_transaction_versioned,
        route,
        expected_network_id.as_inner(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let result = PyDict::new(py);
    result.set_item("tx_hash", identity.tx_hash.to_string())?;
    result.set_item("entrypoint_hash", identity.entrypoint_hash.to_string())?;
    result.set_item(
        "signed_transaction_hash",
        identity.signed_transaction_hash.to_string(),
    )?;
    Ok(result.unbind())
}

#[pyfunction]
#[pyo3(name = "verify_sorafs_orderbook_submission_receipt_v1")]
pub(crate) fn verify_sorafs_orderbook_submission_receipt_v1_py(
    receipt_norito: &[u8],
    tx_hash: &str,
    entrypoint_hash: &str,
    signed_transaction_hash: &str,
    expected_receipt_signer: &str,
) -> PyResult<String> {
    let identity = parse_identity(tx_hash, entrypoint_hash, signed_transaction_hash)?;
    let signer = parse_exact::<PublicKey>(expected_receipt_signer, "expected_receipt_signer")?;
    let receipt = decode_and_verify_sorafs_orderbook_submission_receipt_v1(
        receipt_norito,
        &identity,
        &signer,
    )
    .map_err(|error| invalid(error.to_string()))?;
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
