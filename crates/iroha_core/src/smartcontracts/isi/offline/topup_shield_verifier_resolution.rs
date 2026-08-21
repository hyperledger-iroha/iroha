fn resolve_kagemusha_topup_shield_verifier(
    asset: &AssetDefinitionId,
    proof: &ProofAttachment,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(VerifyingKeyBox, VerifyingKeyRecord), Error> {
    ensure_kagemusha_transparent_attachment(proof)?;
    let zk_state = state_transaction
        .world
        .zk_assets
        .get(asset)
        .ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up requires configured confidential asset state",
            )
        })?;
    let binding = zk_state.vk_shield.as_ref().ok_or_else(|| {
        labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha top-up requires an asset-bound shield verifier key",
        )
    })?;
    if proof.vk_ref != binding.id || proof.backend != binding.id.backend {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha top-up proof must reference the asset-bound shield verifier key",
        )
        .into());
    }
    if proof.vk_commitment != Some(binding.commitment) || binding.commitment == [0; 32] {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha top-up verifier commitment does not match the asset binding",
        )
        .into());
    }
    let record = state_transaction
        .world
        .verifying_keys
        .get(&binding.id)
        .cloned()
        .ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up shield verifier key is not registered",
            )
        })?;
    let circuit_key = (record.circuit_id.clone(), record.version);
    if !record.is_active_at(state_transaction.block_height())
        || state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&circuit_key)
            != Some(&binding.id)
    {
        return Err(labeled_invariant(
            "verifier_key_inactive",
            "Kagemusha top-up shield verifier circuit/version is not active",
        )
        .into());
    }
    let expected_schema_hash: [u8; 32] =
        Hash::new(crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2)
            .into();
    if record.namespace != crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE
        || record.backend != BackendTag::Halo2IpaPasta
        || record.circuit_id != crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID
        || record.curve != "pallas"
        || record.public_inputs_schema_hash != expected_schema_hash
        || record.commitment != binding.commitment
        || record.max_proof_bytes == 0
        || proof.proof.bytes.len() > record.max_proof_bytes as usize
    {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha top-up requires the canonical asset-bound shield-v2 verifier",
        )
        .into());
    }
    let vk_box = record.key.clone().ok_or_else(|| {
        labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha top-up shield verifier key is not available inline",
        )
    })?;
    if vk_box.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
        || vk_box.bytes.is_empty()
        || u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len)
        || crate::zk::hash_vk(&vk_box) != record.commitment
    {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha top-up inline shield verifier does not match its registry record",
        )
        .into());
    }
    crate::zk::confidential_v2::ensure_kagemusha_topup_shield_v2_canonical_vk_box(&vk_box)
        .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;
    let envelope = decode_canonical_offline_proof_envelope(
        &proof.proof.bytes,
        "Kagemusha top-up shield proof must be a canonical OpenVerifyEnvelope",
    )?;
    if envelope.backend != BackendTag::Halo2IpaPasta
        || envelope.circuit_id != crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID
        || envelope.public_inputs
            != crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2
        || envelope.vk_hash != binding.commitment
        || !envelope.aux.is_empty()
    {
        return Err(labeled_invariant(
            "invalid_proof",
            "Kagemusha top-up shield proof envelope metadata is inconsistent",
        )
        .into());
    }
    if let Some(envelope_hash) = proof.envelope_hash {
        let expected_hash: [u8; 32] = Hash::new(&proof.proof.bytes).into();
        if envelope_hash != expected_hash {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha top-up shield envelope hash does not match its proof bytes",
            )
            .into());
        }
    }
    Ok((vk_box, record))
}
