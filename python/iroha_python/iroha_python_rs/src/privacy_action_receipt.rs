//! Signed-query construction and finalized Exact12 receipt inspection for Python.
//!
//! A successful pipeline result is not sufficient evidence that an Exact12
//! action executed its native semantics.  This boundary accepts only the
//! finalized typed receipt that Core writes atomically with the action effect.

use std::str::FromStr;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    privacy::{
        PrivacyExact12ActionOperationV1, PrivacyLedgerEffectKindV1, PrivacyOperationSchemaV1,
        PrivacyProtocolIdV1,
    },
    query::{
        QueryRequest, QueryResponse, SingularQueryOutputBox,
        privacy::prelude::FindPrivacyActionExecutionReceiptV1,
    },
    transaction::TransactionEntrypoint,
};
use pyo3::{
    Bound, Py, PyResult, Python,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    pyfunction,
    types::{PyAny, PyAnyMethods, PyBytes, PyDict, PyDictMethods},
};

use super::{PyNetworkId, sign_query_request_with_signer};

const RECEIPT_RESPONSE_MAX_BYTES_V1: usize = 256 * 1024;

fn operation_from_index(index: u32) -> Option<PrivacyExact12ActionOperationV1> {
    Some(match index {
        0 => PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,
        1 => PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1,
        2 => PrivacyOperationSchemaV1::VeRangeRangeProofV1,
        3 => PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1,
        4 => PrivacyOperationSchemaV1::ZkAmsProvisionAccountActionV1,
        5 => PrivacyOperationSchemaV1::VegaCredentialPresentationV1,
        6 => PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1,
        7 => PrivacyOperationSchemaV1::JindoPolynomialEvaluationV1,
        8 => PrivacyOperationSchemaV1::BootleLanternCredentialPresentationV1,
        9 => PrivacyOperationSchemaV1::OrchardNoteActionV1,
        10 => PrivacyOperationSchemaV1::FcmpMembershipPaymentV1,
        11 => PrivacyOperationSchemaV1::IvmPrivateNoteActionV1,
        12 => PrivacyOperationSchemaV1::PqMaspNoteActionV1,
        _ => return None,
    })
}

fn expected_operation(index: u32) -> PyResult<PrivacyExact12ActionOperationV1> {
    operation_from_index(index).ok_or_else(|| {
        PyValueError::new_err("Exact12 operation discriminant is outside the closed union")
    })
}

fn transaction_hash(value: &str) -> PyResult<HashOf<TransactionEntrypoint>> {
    let normalized = value.strip_prefix("0x").unwrap_or(value);
    if normalized.len() != 64
        || normalized != normalized.to_ascii_lowercase()
        || normalized.bytes().any(|byte| !byte.is_ascii_hexdigit())
    {
        return Err(PyValueError::new_err(
            "Exact12 transaction hash must be exactly 64 lowercase hexadecimal characters",
        ));
    }
    let hash = Hash::from_str(normalized)
        .map_err(|_| PyValueError::new_err("Exact12 transaction hash is invalid"))?;
    if hash.as_ref().iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err(
            "Exact12 transaction hash must not be all zero",
        ));
    }
    Ok(HashOf::from_untyped_unchecked(hash))
}

fn exact_nonzero_32(value: &[u8], label: &str) -> PyResult<[u8; 32]> {
    let exact: [u8; 32] = value
        .try_into()
        .map_err(|_| PyValueError::new_err(format!("{label} must contain exactly 32 bytes")))?;
    if exact.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err(format!(
            "{label} must not be all zero"
        )));
    }
    Ok(exact)
}

/// Build one authority-signed singular query for the exact native receipt.
#[pyfunction]
#[pyo3(name = "build_find_privacy_action_execution_receipt_query_with_signer")]
pub(crate) fn build_query_with_signer(
    py: Python<'_>,
    authority: &str,
    signer: &Bound<'_, PyAny>,
    network_id: &PyNetworkId,
    operation_index: u32,
    transaction_hash_literal: &str,
    action_index: u32,
) -> PyResult<Py<PyBytes>> {
    if !signer.is_callable() {
        return Err(PyTypeError::new_err("query signer must be callable"));
    }
    let operation = expected_operation(operation_index)?;
    let transaction_hash = transaction_hash(transaction_hash_literal)?;
    let request = QueryRequest::Singular(
        FindPrivacyActionExecutionReceiptV1::new(
            operation.protocol_id(),
            *transaction_hash.as_ref(),
            action_index,
        )
        .into(),
    );
    let signed =
        sign_query_request_with_signer(py, authority, signer, network_id.as_inner(), request)?;
    Ok(Py::from(PyBytes::new(py, &signed)))
}

/// Decode, canonicalize, validate, and bind one finalized native receipt.
#[pyfunction]
#[pyo3(name = "inspect_privacy_action_execution_receipt_response")]
#[expect(
    clippy::too_many_arguments,
    reason = "the public receipt inspector keeps every authenticated binding explicit"
)]
pub(crate) fn inspect_response(
    py: Python<'_>,
    network_id: &PyNetworkId,
    operation_index: u32,
    transaction_hash_literal: &str,
    action_index: u32,
    transaction_intent_digest: &[u8],
    statement_digest: &[u8],
    proof_envelope_hash: &[u8],
    response: &[u8],
) -> PyResult<Py<PyDict>> {
    let expected_operation = expected_operation(operation_index)?;
    let expected_protocol: PrivacyProtocolIdV1 = expected_operation.protocol_id();
    let expected_effect: PrivacyLedgerEffectKindV1 = expected_operation.ledger_effect_kind();
    let expected_transaction_hash = transaction_hash(transaction_hash_literal)?;
    let expected_transaction_intent =
        exact_nonzero_32(transaction_intent_digest, "transaction-intent digest")?;
    let expected_statement = exact_nonzero_32(statement_digest, "statement digest")?;
    let expected_envelope = exact_nonzero_32(proof_envelope_hash, "proof-envelope hash")?;

    if response.is_empty() || response.len() > RECEIPT_RESPONSE_MAX_BYTES_V1 {
        return Err(PyValueError::new_err(
            "Exact12 execution-receipt response is outside its closed byte bound",
        ));
    }
    let decoded: QueryResponse = norito::decode_canonical_with_limits(
        response,
        norito::canonical_decode_limits(response.len()),
    )
    .map_err(|_| {
        PyValueError::new_err("Exact12 execution-receipt response is not canonical Norito")
    })?;
    let canonical = norito::to_bytes(&decoded).map_err(|_| {
        PyRuntimeError::new_err(
            "Exact12 execution-receipt response could not be canonically re-encoded",
        )
    })?;
    if canonical != response {
        return Err(PyValueError::new_err(
            "Exact12 execution-receipt response is not its exact canonical wire",
        ));
    }
    let QueryResponse::Singular(SingularQueryOutputBox::PrivacyActionExecutionReceiptViewV1(
        receipt,
    )) = decoded
    else {
        return Err(PyValueError::new_err(
            "Exact12 execution-receipt query returned an unexpected typed response",
        ));
    };
    receipt.validate().map_err(|_| {
        PyValueError::new_err("Exact12 execution receipt failed its native validation")
    })?;
    if receipt.network_id != *network_id.as_inner()
        || receipt.protocol_id != expected_protocol
        || receipt.operation_schema != expected_operation
        || receipt.ledger_effect_kind != expected_effect
        || receipt.transaction_hash != *expected_transaction_hash.as_ref()
        || receipt.action_index != action_index
        || receipt.transaction_intent_digest.as_bytes() != &expected_transaction_intent
        || receipt.statement_digest.as_bytes() != &expected_statement
        || receipt.proof_envelope_hash != expected_envelope
    {
        return Err(PyValueError::new_err(
            "Exact12 execution receipt differs from the requested action",
        ));
    }

    let output = PyDict::new(py);
    output.set_item("version", receipt.version)?;
    output.set_item("network_id", hex::encode(receipt.network_id.as_bytes()))?;
    output.set_item("protocol_id", receipt.protocol_id.canonical_label())?;
    output.set_item(
        "operation_schema",
        receipt.operation_schema.canonical_label(),
    )?;
    output.set_item(
        "ledger_effect_kind",
        receipt.ledger_effect_kind.canonical_label(),
    )?;
    output.set_item("transaction_hash", hex::encode(receipt.transaction_hash))?;
    output.set_item("action_index", receipt.action_index)?;
    output.set_item(
        "transaction_intent_digest",
        hex::encode(receipt.transaction_intent_digest.as_bytes()),
    )?;
    output.set_item(
        "statement_digest",
        hex::encode(receipt.statement_digest.as_bytes()),
    )?;
    output.set_item(
        "proof_envelope_hash",
        hex::encode(receipt.proof_envelope_hash),
    )?;
    output.set_item(
        "capability_manifest_digest",
        hex::encode(receipt.capability_manifest_digest.as_bytes()),
    )?;
    output.set_item(
        "capability_committed_height",
        receipt.capability_committed_height,
    )?;
    output.set_item("admitted_at_height", receipt.admitted_at_height)?;
    output.set_item("finalized_height", receipt.finalized_height)?;
    output.set_item(
        "finalized_block_hash",
        receipt.finalized_block_hash.to_string(),
    )?;
    Ok(output.unbind())
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        privacy::{
            PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1, PrivacyActionExecutionReceiptViewV1,
            PrivacyExact12CapabilityManifestDigestV1, PrivacyOperationSchemaV1,
            PrivacyStatementDigestV1, PrivacyTransactionIntentDigestV1,
        },
        query::{QueryResponse, SingularQueryOutputBox},
        transaction::TransactionEntrypoint,
    };
    use pyo3::Python;

    use super::inspect_response;
    use crate::PyNetworkId;

    #[test]
    fn inspector_rejects_mutated_requested_statement() {
        Python::attach(|py| {
            let operation = PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1;
            let network_id = NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x71; 32])),
            );
            let transaction_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                Hash::prehashed([0x11; 32]),
            );
            let receipt = PrivacyActionExecutionReceiptViewV1 {
                version: PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1,
                network_id,
                protocol_id: operation.protocol_id(),
                operation_schema: operation,
                ledger_effect_kind: operation.ledger_effect_kind(),
                transaction_hash: *transaction_hash.as_ref(),
                action_index: 0,
                transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x22; 32]),
                statement_digest: PrivacyStatementDigestV1::new([0x33; 32]),
                proof_envelope_hash: [0x44; 32],
                capability_manifest_digest: PrivacyExact12CapabilityManifestDigestV1::new(
                    [0x55; 32],
                ),
                capability_committed_height: 40,
                admitted_at_height: 41,
                finalized_height: 42,
                finalized_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                    Hash::prehashed([0x72; 32]),
                ),
            };
            let response = QueryResponse::Singular(
                SingularQueryOutputBox::PrivacyActionExecutionReceiptViewV1(receipt),
            );
            let wire = norito::to_bytes(&response).expect("encode receipt response");
            let network = PyNetworkId { inner: network_id };
            inspect_response(
                py,
                &network,
                1,
                &transaction_hash.to_string(),
                0,
                &[0x22; 32],
                &[0x33; 32],
                &[0x44; 32],
                &wire,
            )
            .expect("inspect exact receipt");
            assert!(
                inspect_response(
                    py,
                    &network,
                    1,
                    &transaction_hash.to_string(),
                    0,
                    &[0x22; 32],
                    &[0x99; 32],
                    &[0x44; 32],
                    &wire,
                )
                .is_err()
            );
        });
    }
}
