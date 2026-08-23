// Consensus validation for activation-bound Kagemusha V4 Taira canaries.

use iroha_data_model::{offline::KagemushaExactBytesDigestV1, transaction::Executable};

const KAGEMUSHA_V4_PROMOTION_ID_DOMAIN: &str = "kagemusha-v4-promotion-id";
const KAGEMUSHA_V4_PROMOTION_BINDING_DOMAIN: &str = "kagemusha-v4-promotion-binding";
const KAGEMUSHA_V4_TAIRA_CANARY_DOMAIN: &str = "kagemusha-v4-taira-canary";
const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SLOT_DOMAIN: &str =
    "kagemusha-v4-taira-canary-authorization-slot";
const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZED_CALL_DOMAIN: &str =
    "kagemusha-v4-taira-canary-authorized-call";
const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZED_WIRE_DOMAIN: &str =
    "kagemusha-v4-taira-canary-authorized-wire";
const KAGEMUSHA_V4_TAIRA_CANARY_EXACT_RESERVATION_DOMAIN: &str =
    "kagemusha-v4-taira-canary-exact-reservation";

/// Derive the complete wire identity only for one direct signed canary record.
///
/// Top-level batches, contract/IVM paths, and dynamically emitted instructions
/// receive no signed-wire authorization.
pub(crate) fn signed_kagemusha_taira_canary_wire_identity_v1(
    transaction: &SignedTransaction,
) -> Result<Option<KagemushaExactBytesDigestV1>, iroha_data_model::ValidationFail> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Ok(None);
    };
    let [instruction] = instructions.as_ref() else {
        return Ok(None);
    };
    if instruction
        .as_any()
        .downcast_ref::<RecordKagemushaTairaCanaryV4>()
        .is_none()
    {
        return Ok(None);
    }
    let wire = transaction.encode_wire_v1().map_err(|error| {
        iroha_data_model::ValidationFail::InternalError(format!(
            "failed to encode exact signed Kagemusha Taira canary wire: {error}"
        ))
    })?;
    KagemushaExactBytesDigestV1::from_bytes(&wire)
        .map(Some)
        .map_err(|error| {
            iroha_data_model::ValidationFail::InternalError(format!(
                "failed to derive exact signed Kagemusha Taira canary wire identity: {error}"
            ))
        })
}

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
    wire_identity: iroha_data_model::offline::KagemushaExactBytesDigestV1,
) -> Result<(Hash, Hash, Hash), Error> {
    if promotion_id == [0; 32] || exact_call_hash.as_ref().iter().all(|byte| *byte == 0) {
        return Err(labeled_invariant(
            "canary_authorization_invalid",
            "Kagemusha V4 Taira canary authorization identity must be nonzero",
        )
        .into());
    }
    wire_identity.validate().map_err(|error| {
        labeled_invariant(
            "canary_authorization_invalid",
            format!("invalid exact Kagemusha V4 Taira canary wire identity: {error}"),
        )
    })?;
    let slot = kagemusha_v2_marker(
        KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SLOT_DOMAIN,
        &[promotion_id.as_slice()],
    );
    let exact_call = kagemusha_v2_marker(
        KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZED_CALL_DOMAIN,
        &[promotion_id.as_slice(), exact_call_hash.as_ref()],
    );
    let byte_len = wire_identity.byte_len.to_le_bytes();
    let exact_wire = kagemusha_v2_marker(
        KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZED_WIRE_DOMAIN,
        &[
            promotion_id.as_slice(),
            exact_call_hash.as_ref(),
            &byte_len,
            &wire_identity.sha256,
        ],
    );
    Ok((slot, exact_call, exact_wire))
}

fn plan_v4_taira_canary_authorization(
    promotion_id: [u8; 32],
    exact_call_hash: Hash,
    reservation: &iroha_data_model::offline::KagemushaV4TairaCanaryReservationV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<(Hash, Hash, Hash, Hash)>, Error> {
    let (slot, exact_call, exact_wire) = v4_taira_canary_authorization_markers(
        promotion_id,
        exact_call_hash,
        reservation.body.canary_transaction_wire,
    )?;
    let reservation_bytes = norito::encode_canonical(reservation).map_err(|error| {
        labeled_invariant(
            "canary_authorization_invalid",
            format!("failed to encode exact Kagemusha V4 Taira canary reservation: {error}"),
        )
    })?;
    let exact_reservation = kagemusha_v2_marker(
        KAGEMUSHA_V4_TAIRA_CANARY_EXACT_RESERVATION_DOMAIN,
        &[
            promotion_id.as_slice(),
            exact_call_hash.as_ref(),
            &reservation_bytes,
        ],
    );
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
    let exact_wire_exists = state_transaction
        .world
        .kagemusha_replay_keys
        .get(&exact_wire)
        .is_some();
    let exact_reservation_exists = state_transaction
        .world
        .kagemusha_replay_keys
        .get(&exact_reservation)
        .is_some();
    match (
        slot_exists,
        exact_call_exists,
        exact_wire_exists,
        exact_reservation_exists,
    ) {
        (false, false, false, false) => {
            let _ = plan_v4_taira_canary(promotion_id, state_transaction)?;
            Ok(Some((slot, exact_call, exact_wire, exact_reservation)))
        }
        (true, true, true, true) => Ok(None),
        _ => Err(labeled_invariant(
            "canary_authorization_replay",
            "a different exact Taira canary reservation already occupies this promotion slot",
        )
        .into()),
    }
}

fn commit_v4_taira_canary_authorization(
    slot: Hash,
    exact_call: Hash,
    exact_wire: Hash,
    exact_reservation: Hash,
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
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(exact_wire, ());
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(exact_reservation, ());
}

fn require_v4_taira_canary_authorization(
    promotion_id: [u8; 32],
    exact_call_hash: Hash,
    wire_identity: iroha_data_model::offline::KagemushaExactBytesDigestV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let (slot, exact_call, exact_wire) =
        v4_taira_canary_authorization_markers(promotion_id, exact_call_hash, wire_identity)?;
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
        || state_transaction
            .world
            .kagemusha_replay_keys
            .get(&exact_wire)
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
        || promotion_binding.release_record_sha256
            != <[u8; 32]>::from(Sha256::digest(&release_record_bytes))
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
        let wire_identity = state_transaction
            .kagemusha_taira_canary_wire_identity
            .ok_or_else(|| {
                labeled_invariant(
                    "canary_authorization_missing",
                    "Kagemusha V4 Taira canary requires its complete signed transaction wire",
                )
            })?;
        require_v4_taira_canary_authorization(
            binding.promotion_id,
            exact_call_hash,
            wire_identity,
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
            &self.reservation,
            state_transaction,
        )?;
        if let Some((slot, exact_call, exact_wire, exact_reservation)) = authorization_markers {
            commit_v4_taira_canary_authorization(
                slot,
                exact_call,
                exact_wire,
                exact_reservation,
                state_transaction,
            );
        }
        Ok(())
    }
}
