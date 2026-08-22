// Consensus validation for activation-bound Kagemusha V4 Taira canaries.

const KAGEMUSHA_V4_PROMOTION_ID_DOMAIN: &str = "kagemusha-v4-promotion-id";
const KAGEMUSHA_V4_PROMOTION_BINDING_DOMAIN: &str = "kagemusha-v4-promotion-binding";
const KAGEMUSHA_V4_TAIRA_CANARY_DOMAIN: &str = "kagemusha-v4-taira-canary";
const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SLOT_DOMAIN: &str =
    "kagemusha-v4-taira-canary-authorization-slot";
const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZED_CALL_DOMAIN: &str =
    "kagemusha-v4-taira-canary-authorized-call";

fn v4_promotion_binding_marker(
    binding: &iroha_data_model::offline::KagemushaV4PromotionBindingV1,
) -> Result<Hash, Error> {
    binding.validate().map_err(|error| {
        labeled_invariant(
            "recursive_release_invalid",
            format!("invalid Kagemusha V4 promotion binding: {error}"),
        )
    })?;
    let canonical_binding = norito::encode_canonical(binding).map_err(|error| {
        labeled_invariant(
            "recursive_release_invalid",
            format!("failed to encode Kagemusha V4 promotion binding: {error}"),
        )
    })?;
    Ok(kagemusha_v2_marker(
        KAGEMUSHA_V4_PROMOTION_BINDING_DOMAIN,
        &[&canonical_binding],
    ))
}

fn plan_v4_promotion_binding(
    binding: &iroha_data_model::offline::KagemushaV4PromotionBindingV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(Hash, Hash), Error> {
    let promotion_marker = plan_v4_promotion_id(binding.promotion_id, state_transaction)?;
    let binding_marker = v4_promotion_binding_marker(binding)?;
    if state_transaction
        .world
        .kagemusha_replay_keys
        .get(&binding_marker)
        .is_some()
    {
        return Err(labeled_invariant(
            "promotion_replay",
            "Kagemusha V4 promotion binding was already committed by an activation",
        )
        .into());
    }
    Ok((promotion_marker, binding_marker))
}

fn commit_v4_promotion_binding(
    promotion_marker: Hash,
    binding_marker: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    commit_v4_promotion_id(promotion_marker, state_transaction);
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(binding_marker, ());
}

fn require_v4_promotion_binding(
    binding: &iroha_data_model::offline::KagemushaV4PromotionBindingV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let promotion_marker =
        kagemusha_v2_marker(KAGEMUSHA_V4_PROMOTION_ID_DOMAIN, &[&binding.promotion_id]);
    let binding_marker = v4_promotion_binding_marker(binding)?;
    if state_transaction
        .world
        .kagemusha_replay_keys
        .get(&promotion_marker)
        .is_none()
        || state_transaction
            .world
            .kagemusha_replay_keys
            .get(&binding_marker)
            .is_none()
    {
        return Err(labeled_invariant(
            "canary_activation_missing",
            "Kagemusha V4 Taira canary permit is not bound to a committed activation",
        )
        .into());
    }
    Ok(())
}

fn plan_v4_taira_canary(
    promotion_id: [u8; 32],
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Hash, Error> {
    if promotion_id == [0; 32] {
        return Err(labeled_invariant(
            "canary_invalid",
            "Kagemusha V4 Taira canary promotion id must be nonzero",
        )
        .into());
    }
    let marker = kagemusha_v2_marker(KAGEMUSHA_V4_TAIRA_CANARY_DOMAIN, &[&promotion_id]);
    if state_transaction
        .world
        .kagemusha_replay_keys
        .get(&marker)
        .is_some()
    {
        return Err(labeled_invariant(
            "canary_replay",
            "Kagemusha V4 Taira canary was already consumed for this promotion",
        )
        .into());
    }
    Ok(marker)
}

fn commit_v4_taira_canary(marker: Hash, state_transaction: &mut StateTransaction<'_, '_>) {
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(marker, ());
}

fn v4_taira_canary_authorization_markers(
    promotion_id: [u8; 32],
    exact_call_hash: Hash,
) -> Result<(Hash, Hash), Error> {
    if promotion_id == [0; 32] || exact_call_hash.as_ref().iter().all(|byte| *byte == 0) {
        return Err(labeled_invariant(
            "canary_authorization_invalid",
            "Kagemusha V4 Taira canary authorization identity must be nonzero",
        )
        .into());
    }
    let slot = kagemusha_v2_marker(
        KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SLOT_DOMAIN,
        &[promotion_id.as_slice()],
    );
    let exact_call = kagemusha_v2_marker(
        KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZED_CALL_DOMAIN,
        &[promotion_id.as_slice(), exact_call_hash.as_ref()],
    );
    Ok((slot, exact_call))
}

fn plan_v4_taira_canary_authorization(
    promotion_id: [u8; 32],
    exact_call_hash: Hash,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<(Hash, Hash)>, Error> {
    let (slot, exact_call) = v4_taira_canary_authorization_markers(promotion_id, exact_call_hash)?;
    let slot_exists = state_transaction
        .world
        .kagemusha_replay_keys
        .get(&slot)
        .is_some();
    let exact_call_exists = state_transaction
        .world
        .kagemusha_replay_keys
        .get(&exact_call)
        .is_some();
    match (slot_exists, exact_call_exists) {
        (false, false) => {
            let _ = plan_v4_taira_canary(promotion_id, state_transaction)?;
            Ok(Some((slot, exact_call)))
        }
        (true, true) => Ok(None),
        _ => Err(labeled_invariant(
            "canary_authorization_replay",
            "a different exact Taira canary call already occupies this promotion slot",
        )
        .into()),
    }
}

fn commit_v4_taira_canary_authorization(
    slot: Hash,
    exact_call: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(slot, ());
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(exact_call, ());
}

fn require_v4_taira_canary_authorization(
    promotion_id: [u8; 32],
    exact_call_hash: Hash,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let (slot, exact_call) = v4_taira_canary_authorization_markers(promotion_id, exact_call_hash)?;
    if state_transaction
        .world
        .kagemusha_replay_keys
        .get(&slot)
        .is_none()
        || state_transaction
            .world
            .kagemusha_replay_keys
            .get(&exact_call)
            .is_none()
    {
        return Err(labeled_invariant(
            "canary_authorization_missing",
            "current Kagemusha V4 Taira canary transaction was not exactly pre-authorized",
        )
        .into());
    }
    Ok(())
}

fn plan_kagemusha_v4_activation_binding(
    promotion_binding: &iroha_data_model::offline::KagemushaV4PromotionBindingV1,
    activation: &iroha_data_model::offline::KagemushaRecursiveSpendReleaseActivationV4,
    release_binding: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
    policy_bytes: &[u8],
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(Hash, Hash, Vec<u8>), Error> {
    promotion_binding.validate().map_err(|error| {
        labeled_invariant(
            "recursive_release_invalid",
            format!("invalid Kagemusha V4 promotion binding: {error}"),
        )
    })?;
    if &promotion_binding.network_id != state_transaction.network_id() {
        return Err(labeled_invariant(
            "wrong_network",
            "Kagemusha V4 promotion binding targets a different network",
        )
        .into());
    }
    let release_record_bytes =
        norito::encode_canonical(&activation.release_record).map_err(|error| {
            labeled_invariant(
                "recursive_release_invalid",
                format!("failed to encode Kagemusha V4 release record: {error}"),
            )
        })?;
    let manifest = &activation.release_record.manifest;
    if promotion_binding.reviewed_source_closure_descriptor_sha256
        != manifest.reviewed_source_closure_descriptor_sha256
        || promotion_binding.manifest_sha256 != release_binding.manifest_sha256
        || promotion_binding.release_record_sha256 != Sha256::digest(&release_record_bytes).into()
        || promotion_binding.release_policy_source.sha256 != activation.configured_policy_sha256
        || !promotion_binding
            .device_attestation_policy_norito
            .matches_bytes(policy_bytes)
    {
        return Err(labeled_invariant(
            "recursive_release_invalid",
            "Kagemusha V4 promotion binding differs from the activated release or policy",
        )
        .into());
    }
    let (promotion_marker, binding_marker) =
        plan_v4_promotion_binding(promotion_binding, state_transaction)?;
    Ok((promotion_marker, binding_marker, release_record_bytes))
}

impl Execute for RecordKagemushaTairaCanaryV4 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.permit
            .verify_for_execution(
                state_transaction.network_id(),
                authority,
                state_transaction.block_unix_timestamp_ms(),
                state_transaction.block_height(),
            )
            .map_err(|error| {
                labeled_invariant(
                    "invalid_taira_canary_permit",
                    format!("invalid Kagemusha V4 Taira canary permit: {error}"),
                )
            })?;
        let binding = &self.permit.body.binding;
        require_v4_promotion_binding(binding, state_transaction)?;
        let exact_call_hash = state_transaction.tx_call_hash.ok_or_else(|| {
            labeled_invariant(
                "canary_authorization_missing",
                "Kagemusha V4 Taira canary requires an exact transaction call hash",
            )
        })?;
        require_v4_taira_canary_authorization(
            binding.promotion_id,
            exact_call_hash,
            state_transaction,
        )?;
        let marker = plan_v4_taira_canary(binding.promotion_id, state_transaction)?;
        commit_v4_taira_canary(marker, state_transaction);
        Ok(())
    }
}

impl Execute for AuthorizeKagemushaTairaCanaryV4 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.reservation
            .verify_for_execution(
                state_transaction.network_id(),
                authority,
                state_transaction.block_unix_timestamp_ms(),
                state_transaction.block_height(),
            )
            .map_err(|error| {
                labeled_invariant(
                    "invalid_taira_canary_authorization",
                    format!("invalid exact Kagemusha V4 Taira canary authorization: {error}"),
                )
            })?;
        let binding = &self.reservation.body.permit.body.binding;
        require_v4_promotion_binding(binding, state_transaction)?;
        let exact_call_hash = self.reservation.body.canary_entrypoint_hash;
        let authorization_markers = plan_v4_taira_canary_authorization(
            binding.promotion_id,
            exact_call_hash,
            state_transaction,
        )?;
        if let Some((slot, exact_call)) = authorization_markers {
            commit_v4_taira_canary_authorization(slot, exact_call, state_transaction);
        }
        Ok(())
    }
}
