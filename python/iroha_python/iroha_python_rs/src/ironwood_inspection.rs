//! Authenticated original-wire inspection for the BOI Ironwood registry batch.
//!
//! This is signature, intent, grammar and compiled-profile inspection. It does
//! not claim inclusion, pool-state admission, recipient decryption or finality.
use super::*;

pub(crate) fn inspected(signed: &SignedTransaction, chain: u16) -> PyResult<String> {
    use iroha_data_model::{
        isi::privacy::SubmitPrivacyIronwoodProofV1,
        transaction::{Executable, ExecutableBatchItem},
    };
    let (_, envelope) = python_authenticated_privacy_action_envelope_v1(
        signed,
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
        "Ironwood",
    )?;
    let profile = iroha_core_privacy::privacy_profiles::compiled_privacy_profile_v1(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    )
    .map_err(|_| PyValueError::new_err("Ironwood compiled profile unavailable"))?;
    if !profile.matches_envelope_v1(&envelope) {
        return Err(PyValueError::new_err("Ironwood compiled profile mismatch"));
    }
    let PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement) = &envelope.statement else {
        return Err(PyValueError::new_err("Ironwood statement mismatch"));
    };
    let Executable::Batch(items) = signed.instructions() else {
        return Err(PyValueError::new_err(
            "Ironwood registry payment requires one ordered Batch",
        ));
    };
    if items.len() != 3
        || !matches!(&items[0], ExecutableBatchItem::ContractCall(_))
        || !matches!(&items[1], ExecutableBatchItem::Instruction(i) if i.as_any().is::<SubmitPrivacyIronwoodProofV1>())
        || !matches!(&items[2], ExecutableBatchItem::ContractCall(_))
        || statement.context.action_index != 0
    {
        return Err(PyValueError::new_err(
            "Ironwood registry payment requires call, proof, call",
        ));
    }
    let _guard = ChainDiscriminantGuard::enter(chain);
    let payload_json = json::to_json(signed.payload())
        .map_err(|_| PyValueError::new_err("cannot encode original transaction payload"))?;
    let statement_json = json::to_value(statement)
        .map_err(|_| PyValueError::new_err("cannot encode Ironwood statement"))?;
    let profile_digest = crate::privacy_wallet_worker::compiled_profile_digest(
        "ironwood-halo2-actions-v1",
        &crate::privacy_wallet_worker::CompiledProfileBinding {
            parameter_id: *profile.parameter_id.as_bytes(),
            parameter_digest: *profile.parameter_digest.as_bytes(),
            verifier_digest: *profile.verifier_digest.as_bytes(),
            statement_schema_digest: *profile.statement_schema_digest.as_bytes(),
            engine_manifest_digest: *profile.engine_manifest_digest.as_bytes(),
        },
    )
    .map_err(|_| PyValueError::new_err("cannot bind Ironwood profile"))?;
    json::to_json(&norito::json!({
        "unsigned_payload_json": payload_json,
        "statement": statement_json,
        "transaction_hash_hex": (hex::encode(signed.hash().as_ref())),
        "compiled_profile_digest_hex": (hex::encode(profile_digest)),
        "direct_instruction_index": 1u8,
        "asset_definition_id": (statement.asset_definition_id.to_string()),
        "pool_id_hex": (hex::encode(statement.pool_id.as_bytes())),
        "anchor_hex": (hex::encode(statement.anchor.as_bytes())),
        "anchor_epoch": (statement.anchor_epoch),
        "expiry_height": (statement.expiry_height),
        "global_scope": (statement.public_balance_scope == iroha_data_model::asset::AssetBalanceScope::Global),
        "value_balance": (json::to_value(&statement.value_balance).map_err(|_| PyValueError::new_err("cannot encode value balance"))?),
    })).map_err(|_| PyValueError::new_err("cannot encode Ironwood original inspection"))
}

#[pyfunction]
#[pyo3(name = "inspect_signed_privacy_ironwood_note_action_v1", signature = (signed_transaction_versioned, network_id, *, chain_discriminant))]
fn inspect_py(
    signed_transaction_versioned: &[u8],
    network_id: &PyNetworkId,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
) -> PyResult<String> {
    let signed = decode_canonical_signed_transaction_v1(signed_transaction_versioned)?;
    if signed.network_id() != Some(&network_id.inner) {
        return Err(PyValueError::new_err("Ironwood original network mismatch"));
    }
    inspected(&signed, chain_discriminant)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(inspect_py, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ironwood_original_inspection_refuses_noncanonical_and_empty_wire() {
        assert!(decode_canonical_signed_transaction_v1(&[]).is_err());
        assert!(
            decode_canonical_signed_transaction_v1(b"untrusted JSON is not a signed original")
                .is_err()
        );
    }
}
