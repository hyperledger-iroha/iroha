//! Native verification of the certified pool cut used to fund an Ironwood registry payment.
use crate::privacy_native_actions::network_id_from_genesis_hash_bytes;
use crate::*;
use iroha_core_privacy::{
    ironwood_pool_cut::IronwoodPoolCutProofV1, privacy_profiles::compiled_privacy_profile_v1,
};
use iroha_data_model::{
    privacy::{PrivacyPoolIdV1, PrivacyRuntimeProtocolIdV1},
    sumeragi_finality::{
        SumeragiFinalityCheckpoint, SumeragiFinalityProof, verify_checkpoint_page,
    },
};
const MAX: usize = 16 * 1024 * 1024;
fn bytes(text: &str, max: usize) -> Result<Vec<u8>, NativeValidationError> {
    if text.is_empty()
        || text.len() > max * 2
        || text.len() % 2 != 0
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(NativeValidationError::Value(
            "noncanonical or oversized Ironwood cut hex".into(),
        ));
    }
    hex::decode(text).map_err(|_| NativeValidationError::Value("malformed Ironwood cut hex".into()))
}
fn fixed(text: &str) -> Result<[u8; 32], NativeValidationError> {
    bytes(text, 32)?
        .try_into()
        .map_err(|_| NativeValidationError::Value("Ironwood cut identity must be 32 bytes".into()))
}
/// Verify one exact cut against an independently retained checkpoint and consecutive full frames.
#[pyfunction]
#[pyo3(name = "verify_ironwood_pool_cut_v1")]
pub(super) fn verify_ironwood_pool_cut_v1_py(
    py: Python<'_>,
    proof: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    network_id_hex: &str,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
) -> PyResult<String> {
    // PyO3 keeps the immutable bytes/strings alive for this call. Only Rust views
    // and owned frame strings cross this boundary; errors become Python values
    // after reattachment. Keep decoding, finality, cut and signature checks detached.
    py.detach(move || {
        verify_ironwood_pool_cut_v1(
            proof,
            checkpoint_hex,
            proof_frames_hex,
            network_id_hex,
            chain_discriminant,
            pool_id_hex,
            asset_definition_id,
            payer_account_id,
            expected_profile_digest_hex,
        )
    })
    .map_err(NativeValidationError::into_py)
}
fn verify_ironwood_pool_cut_v1(
    proof: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    network_id_hex: &str,
    chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
) -> Result<String, NativeValidationError> {
    let invalid = || NativeValidationError::Value("Ironwood pool cut authentication failed".into());
    if proof.is_empty()
        || proof.len() > MAX
        || proof_frames_hex.is_empty()
        || proof_frames_hex.len() > 64
    {
        return Err(invalid());
    }
    let network_bytes = fixed(network_id_hex)?;
    if network_bytes[31] & 1 == 0 {
        return Err(invalid());
    }
    let network = network_id_from_genesis_hash_bytes(network_bytes);
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&bytes(checkpoint_hex, MAX)?)
        .map_err(|_| invalid())?;
    let mut total = 0usize;
    let mut frames = Vec::new();
    for frame in &proof_frames_hex {
        let encoded = bytes(frame, MAX)?;
        total = total
            .checked_add(encoded.len())
            .filter(|n| *n <= MAX)
            .ok_or_else(invalid)?;
        frames.push(
            norito::decode_canonical_for_admission::<SumeragiFinalityProof>(
                &encoded,
                norito::canonical_decode_limits(encoded.len()),
            )
            .map_err(|_| invalid())?,
        );
    }
    let page =
        verify_checkpoint_page(network, &checkpoint, &frames, 64, MAX).map_err(|_| invalid())?;
    page.tip()
        .verify_global_scope(network, checkpoint.chain_id())
        .map_err(|_| invalid())?;
    let cut: IronwoodPoolCutProofV1 =
        norito::decode_canonical_for_admission(proof, norito::canonical_decode_limits(proof.len()))
            .map_err(|_| invalid())?;
    let _guard = ChainDiscriminantGuard::enter(chain_discriminant);
    let asset: iroha_data_model::asset::AssetDefinitionId =
        asset_definition_id.parse().map_err(|_| invalid())?;
    if asset.to_string() != asset_definition_id {
        return Err(invalid());
    }
    let payer =
        parse_selected_account_id_native(payer_account_id, "payer_account_id", chain_discriminant)?;
    let pool = PrivacyPoolIdV1::new(fixed(pool_id_hex)?);
    let profile = compiled_privacy_profile_v1(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1)
        .map_err(|_| invalid())?;
    let digest = crate::privacy_wallet_worker::compiled_profile_digest(
        "ironwood-halo2-actions-v1",
        &crate::privacy_wallet_worker::CompiledProfileBinding {
            parameter_id: *profile.parameter_id.as_bytes(),
            parameter_digest: *profile.parameter_digest.as_bytes(),
            verifier_digest: *profile.verifier_digest.as_bytes(),
            statement_schema_digest: *profile.statement_schema_digest.as_bytes(),
            engine_manifest_digest: *profile.engine_manifest_digest.as_bytes(),
        },
    )
    .map_err(|_| invalid())?;
    if fixed(expected_profile_digest_hex)? != digest {
        return Err(invalid());
    }
    let schemas = iroha_data_model::sumeragi_finality::native_world_schema_family_v1();
    let state = cut
        .verify(
            network,
            schemas.as_slice(),
            pool,
            &asset,
            &payer,
            page.tip(),
        )
        .map_err(|_| invalid())?;
    let amount = |value: &Option<iroha_data_model::asset::AssetValue>| {
        value
            .as_ref()
            .map_or_else(|| "0".to_owned(), |v| v.0.to_string())
    };
    norito::json::to_json(&norito::json!({
        "height": (page.tip().height()), "block_hash_hex": (hex::encode(page.tip().header().hash().as_ref())),
        "core_hash_hex": (hex::encode(page.tip().core_hash().0)), "context_id_hex": (hex::encode(page.tip().context_id().as_ref())),
        "pool_id_hex": pool_id_hex, "profile_digest_hex": (hex::encode(digest)), "parameter_id_hex": (hex::encode(profile.parameter_id.as_bytes())),
        "asset_definition_id": (asset.to_string()), "payer_account_id": (payer.to_string()), "reserve_account_id": (state.reserve_account().to_string()),
        "public_balance_scope": "global", "anchor_hex": (hex::encode(state.root().as_bytes())), "anchor_epoch": (state.epoch()),
        "payer_balance_decimal": (amount(&cut.payer_balance)), "reserve_balance_decimal": (amount(&cut.reserve_balance)),
        "checkpoint_hex": (hex::encode(page.checkpoint().encode_canonical().map_err(|_| invalid())?)),
    })).map_err(|_| invalid())
}

/// Authenticate the exact committed public bridge effect using a retained preparation cut.
/// No live-tip balance subtraction is used: unrelated transactions may change either account.
#[pyfunction]
#[pyo3(name = "verify_ironwood_payment_effect_v1")]
pub(super) fn verify_ironwood_payment_effect_v1_py(
    py: Python<'_>,
    proof: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    payment_proof_frames_hex: Vec<String>,
    signed_transaction_versioned: &[u8],
    network_id_hex: &str,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
    expected_amount_decimal: &str,
) -> PyResult<String> {
    // PyO3 keeps the immutable bytes/strings alive for this call. Only Rust views
    // and owned frame strings cross this boundary; errors become Python values
    // after reattachment. Keep decoding, finality, cut and signature checks detached.
    py.detach(move || {
        verify_ironwood_payment_effect_v1(
            proof,
            checkpoint_hex,
            proof_frames_hex,
            payment_proof_frames_hex,
            signed_transaction_versioned,
            network_id_hex,
            chain_discriminant,
            pool_id_hex,
            asset_definition_id,
            payer_account_id,
            expected_profile_digest_hex,
            expected_amount_decimal,
        )
    })
    .map_err(NativeValidationError::into_py)
}
fn verify_ironwood_payment_effect_v1(
    proof: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    payment_proof_frames_hex: Vec<String>,
    signed_transaction_versioned: &[u8],
    network_id_hex: &str,
    chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
    expected_amount_decimal: &str,
) -> Result<String, NativeValidationError> {
    let invalid =
        || NativeValidationError::Value("Ironwood payment effect authentication failed".into());
    let cut_json = verify_ironwood_pool_cut_v1(
        proof,
        checkpoint_hex,
        proof_frames_hex,
        network_id_hex,
        chain_discriminant,
        pool_id_hex,
        asset_definition_id,
        payer_account_id,
        expected_profile_digest_hex,
    )?;
    let cut: norito::json::Value = norito::json::from_str(&cut_json).map_err(|_| invalid())?;
    let cut_height = cut["height"].as_u64().ok_or_else(invalid)?;
    let amount: u64 = expected_amount_decimal.parse().map_err(|_| invalid())?;
    if amount == 0 || amount.to_string() != expected_amount_decimal {
        return Err(invalid());
    }
    let network = network_id_from_genesis_hash_bytes(fixed(network_id_hex)?);
    // This checkpoint comes ONLY from the authenticated preparation page above.
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&bytes(
        cut["checkpoint_hex"].as_str().ok_or_else(invalid)?,
        MAX,
    )?)
    .map_err(|_| invalid())?;
    if payment_proof_frames_hex.is_empty() || payment_proof_frames_hex.len() > 64 {
        return Err(invalid());
    }
    let mut total = 0usize;
    let mut frames = Vec::new();
    for text in payment_proof_frames_hex {
        let frame = bytes(&text, MAX)?;
        total = total
            .checked_add(frame.len())
            .filter(|n| *n <= MAX)
            .ok_or_else(invalid)?;
        frames.push(
            norito::decode_canonical_for_admission::<SumeragiFinalityProof>(
                &frame,
                norito::canonical_decode_limits(frame.len()),
            )
            .map_err(|_| invalid())?,
        );
    }
    let page =
        verify_checkpoint_page(network, &checkpoint, &frames, 64, MAX).map_err(|_| invalid())?;
    page.tip()
        .verify_global_scope(network, checkpoint.chain_id())
        .map_err(|_| invalid())?;
    let _guard = ChainDiscriminantGuard::enter(chain_discriminant);
    let signed = decode_canonical_signed_transaction_v1_native(signed_transaction_versioned)?;
    let (_, envelope) = authenticated_privacy_action_envelope_v1_native(
        &signed,
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
        "Ironwood",
    )?;
    let profile = compiled_privacy_profile_v1(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1)
        .map_err(|_| invalid())?;
    if !profile.matches_envelope_v1(&envelope) {
        return Err(invalid());
    }
    let direct_index =
        crate::privacy_wallet_worker::ironwood_direct_index(&signed).map_err(|_| invalid())?;
    let payer =
        parse_selected_account_id_native(payer_account_id, "payer_account_id", chain_discriminant)?;
    if signed.network_id() != Some(&network) || signed.authority() != &payer {
        return Err(invalid());
    }
    let (_, submission) = signed
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let submission_envelope = submission.runtime_envelope();
    let iroha_data_model::privacy::PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement) =
        &submission_envelope.statement
    else {
        return Err(invalid());
    };
    if statement.context.network_id != network
        || statement.pool_id.as_bytes() != &fixed(pool_id_hex)?
        || statement.asset_definition_id.to_string() != asset_definition_id
        || statement.public_balance_scope != iroha_data_model::asset::AssetBalanceScope::Global
        || hex::encode(statement.anchor.as_bytes())
            != cut["anchor_hex"].as_str().ok_or_else(invalid)?
        || Some(statement.anchor_epoch) != cut["anchor_epoch"].as_u64()
        || Some(statement.expiry_height) != cut_height.checked_add(32)
        || page.tip().height() <= cut_height
        || page.tip().height() > statement.expiry_height
        || statement.value_balance.direction
            != iroha_data_model::privacy::PrivacyValueBalanceDirectionV1::IntoPool
        || statement.value_balance.amount != u128::from(amount)
    {
        return Err(invalid());
    }
    let block = page.tip().block();
    let mut matches = 0;
    for output in block.execution_outputs() {
        use iroha_data_model::block::execution_output::ExecutionOutputV1 as O;
        let execution = match output {
            O::Network(n) => n,
            O::NetworkWithLifecycle(n) => &n.execution,
            _ => continue,
        };
        let Some((hash, committed)) = block.external_signed_transaction_at(
            usize::try_from(execution.input_index).map_err(|_| invalid())?,
        ) else {
            continue;
        };
        if hash != signed.hash_as_entrypoint() {
            continue;
        }
        if execution.result.0.is_err() || committed != &signed {
            return Err(invalid());
        }
        matches += 1;
    }
    if matches != 1 {
        return Err(invalid());
    }
    // A successful selected native Ironwood effect atomically debits its authority and
    // credits the pool's immutable governed reserve by this exact directional balance.
    norito::json::to_json(&norito::json!({
        "height": (page.tip().height()), "block_hash_hex": (hex::encode(page.tip().header().hash().as_ref())),
        "core_hash_hex": (hex::encode(page.tip().core_hash().0)), "context_id_hex": (hex::encode(page.tip().context_id().as_ref())),
        "transaction_hash_hex": (hex::encode(signed.hash_as_entrypoint().as_ref())), "direct_instruction_index": direct_index,
        "pool_id_hex": pool_id_hex, "profile_digest_hex": expected_profile_digest_hex,
        "asset_definition_id": asset_definition_id, "payer_account_id": payer_account_id,
        "reserve_account_id": (cut["reserve_account_id"].clone()), "public_balance_scope": "global",
        "payer_debit_decimal": expected_amount_decimal, "reserve_credit_decimal": expected_amount_decimal,
        "checkpoint_hex": (hex::encode(page.checkpoint().encode_canonical().map_err(|_| invalid())?)),
    })).map_err(|_| invalid())
}

/// Emit the canonical typed governance bootstrap instruction, without signing or submitting.
#[pyfunction]
#[pyo3(name = "ironwood_pool_bootstrap_instruction_json_v1")]
pub(super) fn ironwood_pool_bootstrap_instruction_json_v1_py(
    pool_id_hex: &str,
    asset_definition_id: &str,
    reserve_account_id: &str,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
) -> PyResult<String> {
    let _guard = ChainDiscriminantGuard::enter(chain_discriminant);
    let invalid = || PyValueError::new_err("invalid Ironwood bootstrap selection");
    let asset: iroha_data_model::asset::AssetDefinitionId =
        asset_definition_id.parse().map_err(|_| invalid())?;
    if asset.to_string() != asset_definition_id {
        return Err(invalid());
    }
    let reserve =
        parse_selected_account_id(reserve_account_id, "reserve_account_id", chain_discriminant)?;
    let bootstrap = iroha_data_model::privacy::PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new(fixed(pool_id_hex).map_err(NativeValidationError::into_py)?),
        asset,
        iroha_data_model::asset::AssetBalanceScope::Global,
        reserve,
    )
    .map_err(|_| invalid())?;
    let instruction: iroha_data_model::isi::InstructionBox =
        iroha_data_model::isi::privacy::BootstrapPrivacyIronwoodPoolV1::new(bootstrap).into();
    norito::json::to_json(&vec![instruction]).map_err(|_| invalid())
}

#[cfg(test)]
#[path = "tests/ironwood_pool_cut_tests.rs"]
pub(crate) mod tests;
