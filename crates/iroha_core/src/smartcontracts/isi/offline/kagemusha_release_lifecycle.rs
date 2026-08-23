//! Consensus-enforced staged lifecycle for Kagemusha V4 release issuance.

use super::*;

use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_SCHEMA_V1,
    KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1, KagemushaExactBytesDigestV1,
    KagemushaRecursiveSpendArtifactBindingV4, KagemushaV4PromotionBindingV1,
    KagemushaV4ReleaseCancelledV1, KagemushaV4ReleaseDeactivatedV1, KagemushaV4ReleaseEnabledV1,
    KagemushaV4ReleaseLifecyclePhaseV1, KagemushaV4ReleaseLifecycleStateV1,
    kagemusha_v4_release_lifecycle_state_key,
};

const KAGEMUSHA_V4_RELEASE_TRANSITION_DOMAIN: &str = "kagemusha-v4-release-transition";

/// Exact direct lifecycle instruction admitted by the signed transaction carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleEntrypointKind {
    Stage,
    Enable,
    Cancel,
    Deactivate,
}

/// Classify one native instruction in the closed Kagemusha release-lifecycle surface.
pub(crate) fn direct_lifecycle_entrypoint_kind(
    instruction: &iroha_data_model::isi::InstructionBox,
) -> Option<LifecycleEntrypointKind> {
    let any = instruction.as_any();
    if any
        .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
        .is_some()
    {
        Some(LifecycleEntrypointKind::Stage)
    } else if any
        .downcast_ref::<EnableKagemushaRecursiveIssuanceV4>()
        .is_some()
    {
        Some(LifecycleEntrypointKind::Enable)
    } else if any
        .downcast_ref::<CancelKagemushaRecursiveReleaseV4>()
        .is_some()
    {
        Some(LifecycleEntrypointKind::Cancel)
    } else if any
        .downcast_ref::<DeactivateKagemushaRecursiveIssuanceV4>()
        .is_some()
    {
        Some(LifecycleEntrypointKind::Deactivate)
    } else {
        None
    }
}

/// One-shot identity proving that execution still matches the direct external carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifecycleEntrypointContext {
    kind: LifecycleEntrypointKind,
    transaction_intent: HashOf<SignedTransaction>,
    instruction_digest: Hash,
}

/// Derive lifecycle context only from one ordinary, direct native instruction.
pub(crate) fn signed_lifecycle_entrypoint_context(
    transaction: &SignedTransaction,
) -> Result<Option<LifecycleEntrypointContext>, iroha_data_model::ValidationFail> {
    use iroha_data_model::transaction::{Executable, TransactionAdmissionIntent};

    if transaction.admission_intent() != TransactionAdmissionIntent::Ordinary {
        return Ok(None);
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Ok(None);
    };
    let [instruction] = instructions.as_ref() else {
        return Ok(None);
    };
    let Some(kind) = direct_lifecycle_entrypoint_kind(instruction) else {
        return Ok(None);
    };
    Ok(Some(LifecycleEntrypointContext {
        kind,
        transaction_intent: transaction.hash(),
        instruction_digest: Hash::new(&norito::encode_canonical(instruction).map_err(|error| {
            iroha_data_model::ValidationFail::InternalError(format!(
                "failed to encode exact Kagemusha lifecycle instruction: {error}"
            ))
        })?),
    }))
}

struct LoadedLifecycle {
    key: StatePath,
    bytes: Vec<u8>,
    state: KagemushaV4ReleaseLifecycleStateV1,
}

/// An already validated lifecycle record ready to join the activation write set.
pub(super) struct StagedLifecyclePlan {
    key: StatePath,
    bytes: Vec<u8>,
}

fn invalid(message: impl Into<String>) -> Error {
    labeled_invariant("recursive_release_lifecycle_invalid", message).into()
}

fn lifecycle_key(manifest_sha256: &[u8; 32]) -> Result<StatePath, String> {
    kagemusha_v4_release_lifecycle_state_key(manifest_sha256)
        .map_err(|error| format!("invalid Kagemusha V4 lifecycle key: {error}"))?
        .parse()
        .map_err(|_| "Kagemusha V4 lifecycle state key is invalid".to_owned())
}

fn load_lifecycle(
    world: &impl WorldReadOnly,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<Option<LoadedLifecycle>, String> {
    binding
        .validate()
        .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
    let loaded = load_lifecycle_by_manifest(world, &binding.manifest_sha256)?;
    if loaded
        .as_ref()
        .is_some_and(|loaded| loaded.state.artifact_binding != *binding)
    {
        return Err("Kagemusha V4 lifecycle artifact binding changed".to_owned());
    }
    Ok(loaded)
}

fn load_lifecycle_by_manifest(
    world: &impl WorldReadOnly,
    manifest_sha256: &[u8; 32],
) -> Result<Option<LoadedLifecycle>, String> {
    let key = lifecycle_key(manifest_sha256)?;
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let state = KagemushaV4ReleaseLifecycleStateV1::decode_canonical(bytes)
        .map_err(|error| format!("invalid Kagemusha V4 lifecycle state: {error}"))?;
    if state.artifact_binding.manifest_sha256 != *manifest_sha256
        || state.promotion_binding.manifest_sha256 != *manifest_sha256
    {
        return Err(
            "Kagemusha V4 lifecycle state differs from its manifest-addressed key".to_owned(),
        );
    }
    Ok(Some(LoadedLifecycle {
        key,
        bytes: bytes.clone(),
        state,
    }))
}

fn require_bound_consensus_artifacts(
    world: &impl WorldReadOnly,
    state: &KagemushaV4ReleaseLifecycleStateV1,
    require_live_policy: bool,
) -> Result<(), String> {
    let binding = &state.artifact_binding;
    let release_key = kagemusha_terminal_registry_v4::release_state_key(binding)?;
    let release_bytes = world
        .smart_contract_state()
        .get(&release_key)
        .ok_or_else(|| "Kagemusha V4 lifecycle release record is absent".to_owned())?;
    if !state.release_record_norito.matches_bytes(release_bytes) {
        return Err("Kagemusha V4 lifecycle release record identity changed".to_owned());
    }
    if require_live_policy {
        let policy_bytes = world
            .smart_contract_state()
            .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
            .ok_or_else(|| "Kagemusha V4 lifecycle device policy is absent".to_owned())?;
        if !state
            .promotion_binding
            .device_attestation_policy_norito
            .matches_bytes(policy_bytes)
        {
            return Err("Kagemusha V4 lifecycle device policy identity changed".to_owned());
        }
    }

    let expected_eq = iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
        binding.manifest_sha256,
    );
    let expected_ep = iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
        binding.manifest_sha256,
    );
    if state.step_eq_verifier_key_id != expected_eq || state.step_ep_verifier_key_id != expected_ep
    {
        return Err("Kagemusha V4 lifecycle verifier identities changed".to_owned());
    }
    for (id, parity, role) in [
        (
            &state.step_eq_verifier_key_id,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            "Eq",
        ),
        (
            &state.step_ep_verifier_key_id,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
            "Ep",
        ),
    ] {
        let record = world
            .verifying_keys()
            .get(id)
            .ok_or_else(|| format!("Kagemusha V4 lifecycle {role} verifier is absent"))?;
        ensure_release_qualified_kagemusha_v4_verifier_id(id, record, parity, role)?;
        if record.version != state.verifier_version
            || record.status != ConfidentialStatus::Active
            || world
                .verifying_keys_by_circuit()
                .get(&(record.circuit_id.clone(), record.version))
                != Some(id)
        {
            return Err(format!(
                "Kagemusha V4 lifecycle {role} verifier record changed"
            ));
        }
    }
    Ok(())
}

/// Return whether the exact manifest is enabled and still bound to consensus artifacts.
pub(super) fn issuance_enabled(
    world: &impl WorldReadOnly,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<bool, String> {
    let Some(loaded) = load_lifecycle(world, binding)? else {
        return Ok(false);
    };
    if !loaded.state.issuance_enabled() {
        return Ok(false);
    }
    require_bound_consensus_artifacts(world, &loaded.state, true)?;
    Ok(true)
}

/// Load the exact governed policy retained for redemption of one release's notes.
///
/// Redemption deliberately does not require an issuance-active phase. Notes
/// remain redeemable after governance deactivates their release, while new
/// issuance and offline change remain independently disabled.
pub(super) fn redemption_policy(
    world: &impl WorldReadOnly,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<OfflineDeviceAttestationPolicy, String> {
    let loaded = load_lifecycle(world, binding)?
        .ok_or_else(|| "Kagemusha V4 release lifecycle is absent".to_owned())?;
    let policy_bytes = norito::encode_canonical(&loaded.state.device_attestation_policy)
        .map_err(|error| format!("failed to encode Kagemusha V4 redemption policy: {error}"))?;
    if !loaded
        .state
        .promotion_binding
        .device_attestation_policy_norito
        .matches_bytes(&policy_bytes)
    {
        return Err("Kagemusha V4 redemption policy identity changed".to_owned());
    }
    Ok(loaded.state.device_attestation_policy)
}

/// Require the exact promotion to remain staged for canary authorization or consumption.
pub(super) fn require_staged(
    promotion_binding: &KagemushaV4PromotionBindingV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let cached = state_transaction
        .kagemusha_release_catalog
        .get(&promotion_binding.manifest_sha256)
        .cloned()
        .ok_or_else(|| invalid("Kagemusha V4 manifest is absent from the authenticated catalog"))?;
    let binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: cached.release_record().manifest.generation.clone(),
        manifest_sha256: promotion_binding.manifest_sha256,
    };
    let loaded = load_lifecycle(&state_transaction.world, &binding)
        .map_err(invalid)?
        .ok_or_else(|| invalid("Kagemusha V4 release lifecycle is absent"))?;
    if loaded.state.promotion_binding != *promotion_binding
        || !matches!(
            loaded.state.phase,
            KagemushaV4ReleaseLifecyclePhaseV1::Staged
        )
    {
        return Err(invalid(
            "Kagemusha V4 canary operations require the exact staged release",
        ));
    }
    require_bound_consensus_artifacts(&state_transaction.world, &loaded.state, false)
        .map_err(invalid)
}

/// Validate and encode the staged lifecycle record before activation mutates consensus state.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_staged(
    authority: &AccountId,
    promotion_binding: KagemushaV4PromotionBindingV1,
    artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
    device_attestation_policy: OfflineDeviceAttestationPolicy,
    release_record_bytes: &[u8],
    step_eq_verifier_key_id: iroha_data_model::proof::VerifyingKeyId,
    step_ep_verifier_key_id: iroha_data_model::proof::VerifyingKeyId,
    verifier_version: u32,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<StagedLifecyclePlan, Error> {
    let stage_transaction_intent = state_transaction
        .current_tx_hash
        .clone()
        .ok_or_else(|| invalid("Kagemusha V4 staging requires a signed transaction identity"))?;
    let release_record_norito = KagemushaExactBytesDigestV1::from_bytes(release_record_bytes)
        .map_err(|error| invalid(format!("invalid release-record identity: {error}")))?;
    let state = KagemushaV4ReleaseLifecycleStateV1 {
        schema: KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
        promotion_binding,
        artifact_binding,
        governance_authority: authority.clone(),
        stage_transaction_intent,
        staged_at_height: state_transaction.block_height(),
        staged_at_unix_ms: state_transaction.block_unix_timestamp_ms(),
        release_record_norito,
        device_attestation_policy,
        step_eq_verifier_key_id,
        step_ep_verifier_key_id,
        verifier_version,
        phase: KagemushaV4ReleaseLifecyclePhaseV1::Staged,
    };
    state
        .validate()
        .map_err(|error| invalid(format!("invalid staged lifecycle: {error}")))?;
    let key = lifecycle_key(&state.artifact_binding.manifest_sha256).map_err(invalid)?;
    if state_transaction
        .world
        .smart_contract_state
        .get(&key)
        .is_some()
    {
        return Err(invalid(
            "Kagemusha V4 manifest already has a lifecycle record",
        ));
    }
    let bytes = norito::encode_canonical(&state)
        .map_err(|error| invalid(format!("failed to encode staged lifecycle: {error}")))?;
    Ok(StagedLifecyclePlan { key, bytes })
}

/// Join a planned staged lifecycle record to the activation write set.
pub(super) fn commit_staged(
    plan: StagedLifecyclePlan,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    state_transaction
        .world
        .smart_contract_state
        .insert(plan.key, plan.bytes);
}

fn current_transaction_intent(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<HashOf<SignedTransaction>, Error> {
    state_transaction.current_tx_hash.clone().ok_or_else(|| {
        invalid("Kagemusha V4 lifecycle transition requires a signed transaction identity")
    })
}

fn require_direct_entrypoint(
    expected: LifecycleEntrypointKind,
    instruction: &iroha_data_model::isi::InstructionBox,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let current = state_transaction.current_tx_hash.as_ref();
    let instruction_digest =
        Hash::new(&norito::encode_canonical(instruction).map_err(|error| {
            invalid(format!(
                "failed to encode exact lifecycle instruction: {error}"
            ))
        })?);
    if !consume_direct_entrypoint(
        &mut state_transaction.kagemusha_release_lifecycle_entrypoint,
        state_transaction.kagemusha_taira_canary_external_entrypoint,
        current,
        expected,
        instruction_digest,
    ) {
        return Err(invalid(
            "Kagemusha V4 lifecycle mutation requires one exact direct External instruction",
        ));
    }
    Ok(())
}

fn consume_direct_entrypoint(
    context: &mut Option<LifecycleEntrypointContext>,
    external: bool,
    current: Option<&HashOf<SignedTransaction>>,
    expected: LifecycleEntrypointKind,
    instruction_digest: Hash,
) -> bool {
    let context = context.take();
    external
        && context.as_ref().is_some_and(|context| {
            context.kind == expected
                && Some(&context.transaction_intent) == current
                && context.instruction_digest == instruction_digest
        })
}

fn liveness_terminal_is_current_parent(
    terminal_height: u64,
    terminal_hash: HashOf<iroha_data_model::block::BlockHeader>,
    current_height: u64,
    header_parent_hash: Option<HashOf<iroha_data_model::block::BlockHeader>>,
    canonical_parent_hash: Option<HashOf<iroha_data_model::block::BlockHeader>>,
) -> bool {
    terminal_height
        .checked_add(1)
        .is_some_and(|expected_height| expected_height == current_height)
        && header_parent_hash == Some(terminal_hash)
        && canonical_parent_hash == Some(terminal_hash)
}

fn validator_set_matches(
    current: &[iroha_data_model::peer::PeerId],
    expected: &[iroha_data_model::peer::PeerId],
) -> bool {
    if current.len() != expected.len() {
        return false;
    }
    let mut current = current.to_vec();
    let mut expected = expected.to_vec();
    current.sort();
    expected.sort();
    current == expected
}

/// Consume the exact direct activation carrier before any activation state is planned.
pub(super) fn require_direct_stage(
    instruction: &ActivateKagemushaRecursiveReleaseV4,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    require_direct_entrypoint(
        LifecycleEntrypointKind::Stage,
        &iroha_data_model::isi::InstructionBox::from(instruction.clone()),
        state_transaction,
    )
}

fn require_transition_authority(
    authority: &AccountId,
    loaded: &LoadedLifecycle,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    super::isi::ensure_kagemusha_recursive_release_v4_activation_authorized(
        state_transaction,
        authority,
    )?;
    if authority != &loaded.state.governance_authority
        || state_transaction.network_id() != &loaded.state.promotion_binding.network_id
    {
        return Err(invalid(
            "Kagemusha V4 lifecycle transition authority or network changed",
        ));
    }
    Ok(())
}

fn transition_marker(
    transition_id: &[u8; 32],
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Hash, Error> {
    let marker = kagemusha_v2_marker(KAGEMUSHA_V4_RELEASE_TRANSITION_DOMAIN, &[transition_id]);
    if state_transaction
        .world
        .kagemusha_replay_keys
        .get(&marker)
        .is_some()
    {
        return Err(invalid(
            "Kagemusha V4 lifecycle transition id was already consumed",
        ));
    }
    Ok(marker)
}

fn commit_transition(
    marker: Hash,
    loaded: LoadedLifecycle,
    next: KagemushaV4ReleaseLifecycleStateV1,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    if state_transaction
        .world
        .smart_contract_state
        .get(&loaded.key)
        != Some(&loaded.bytes)
    {
        return Err(invalid(
            "Kagemusha V4 lifecycle changed after predecessor validation",
        ));
    }
    next.validate()
        .map_err(|error| invalid(format!("invalid lifecycle transition: {error}")))?;
    let bytes = norito::encode_canonical(&next)
        .map_err(|error| invalid(format!("failed to encode lifecycle transition: {error}")))?;
    state_transaction
        .world
        .smart_contract_state
        .remove(loaded.key.clone());
    state_transaction
        .world
        .smart_contract_state
        .insert(loaded.key, bytes);
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(marker, ());
    Ok(())
}

impl Execute for EnableKagemushaRecursiveIssuanceV4 {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_direct_entrypoint(
            LifecycleEntrypointKind::Enable,
            &iroha_data_model::isi::InstructionBox::from(self.clone()),
            state_transaction,
        )?;
        self.validate()
            .map_err(|error| invalid(format!("invalid enable witness: {error}")))?;
        let witness = self.witness;
        let promotion_binding = &witness.stage_expectations.body.binding;
        let cached = state_transaction
            .kagemusha_release_catalog
            .get(&promotion_binding.manifest_sha256)
            .cloned()
            .ok_or_else(|| {
                invalid("Kagemusha V4 manifest is absent from the authenticated catalog")
            })?;
        let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: cached.release_record().manifest.generation.clone(),
            manifest_sha256: promotion_binding.manifest_sha256,
        };
        let loaded = load_lifecycle(&state_transaction.world, &artifact_binding)
            .map_err(invalid)?
            .ok_or_else(|| invalid("Kagemusha V4 staged lifecycle is absent"))?;
        require_transition_authority(authority, &loaded, state_transaction)?;
        if loaded.state.promotion_binding != *promotion_binding
            || !matches!(
                loaded.state.phase,
                KagemushaV4ReleaseLifecyclePhaseV1::Staged
            )
        {
            return Err(invalid("only the exact staged release may be enabled"));
        }
        let predecessor = loaded
            .state
            .exact_bytes_digest()
            .map_err(|error| invalid(format!("invalid lifecycle predecessor: {error}")))?;
        if predecessor != witness.expected_predecessor_lifecycle {
            return Err(invalid("enable witness lifecycle predecessor changed"));
        }
        require_bound_consensus_artifacts(&state_transaction.world, &loaded.state, false)
            .map_err(invalid)?;
        let marker = transition_marker(&witness.transition_id, state_transaction)?;

        let reservation_bytes = norito::encode_canonical(&witness.promotion_reservation)
            .map_err(|error| invalid(format!("failed to encode promotion reservation: {error}")))?;
        let expectations_bytes = norito::encode_canonical(&witness.stage_expectations)
            .map_err(|error| invalid(format!("failed to encode stage expectations: {error}")))?;
        let receipt_bytes = norito::encode_canonical(&witness.stage_finality_receipt)
            .map_err(|error| invalid(format!("failed to encode stage receipt: {error}")))?;
        let authorization_bytes = norito::encode_canonical(&witness.canary_authorization)
            .map_err(|error| invalid(format!("failed to encode canary authorization: {error}")))?;
        let canary_evidence_bytes = norito::encode_canonical(&witness.canary_evidence)
            .map_err(|error| invalid(format!("failed to encode canary evidence: {error}")))?;
        let liveness_bytes = norito::encode_canonical(&witness.validator_liveness_evidence)
            .map_err(|error| invalid(format!("failed to encode liveness evidence: {error}")))?;

        let controller = &loaded.state.promotion_binding.promotion_controller;
        let expectations = witness
            .stage_expectations
            .verify_exact(&expectations_bytes, controller, &reservation_bytes)
            .map_err(|error| invalid(format!("invalid stage expectations: {error}")))?;
        let verified_receipt = witness
            .stage_finality_receipt
            .verify(&expectations)
            .map_err(|error| invalid(format!("invalid stage finality receipt: {error}")))?;
        if expectations.binding() != &loaded.state.promotion_binding
            || expectations.governance_authority() != &loaded.state.governance_authority
            || verified_receipt.activation_transaction_intent()
                != loaded.state.stage_transaction_intent
            || verified_receipt.finalized_height() != loaded.state.staged_at_height
        {
            return Err(invalid(
                "stage evidence differs from consensus lifecycle state",
            ));
        }
        let iroha_data_model::transaction::Executable::Instructions(stage_instructions) = witness
            .stage_expectations
            .body
            .activation_transaction
            .instructions()
        else {
            return Err(invalid(
                "stage transaction is not a direct native instruction",
            ));
        };
        let [stage_instruction] = stage_instructions.as_ref() else {
            return Err(invalid("stage transaction is not one exact instruction"));
        };
        let staged_policy = stage_instruction
            .as_any()
            .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
            .ok_or_else(|| invalid("stage transaction does not carry release activation"))?
            .device_attestation_policy()
            .clone();
        let now_ms = state_transaction.block_unix_timestamp_ms();
        super::isi::validate_offline_attestation_policy_for_release_activation(
            &staged_policy,
            now_ms,
        )?;
        super::isi::validate_offline_attestation_policy_transition_from_state(
            &staged_policy,
            state_transaction,
        )?;
        let staged_policy_bytes = norito::encode_canonical(&staged_policy)
            .map_err(|error| invalid(format!("failed to encode staged device policy: {error}")))?;
        if !loaded
            .state
            .promotion_binding
            .device_attestation_policy_norito
            .matches_bytes(&staged_policy_bytes)
        {
            return Err(invalid("staged device policy differs from release binding"));
        }

        let verified_canary = witness
            .canary_evidence
            .verify_exact(
                &canary_evidence_bytes,
                &witness.canary_authorization,
                &authorization_bytes,
                &expectations,
                &witness.stage_finality_receipt,
                &receipt_bytes,
            )
            .map_err(|error| invalid(format!("invalid canary evidence: {error}")))?;
        if verified_canary.promotion_id() != promotion_binding.promotion_id {
            return Err(invalid("canary evidence differs from staged release"));
        }
        super::isi::require_v4_taira_canary_consumed(
            promotion_binding.promotion_id,
            state_transaction,
        )?;

        let expected_canary = &witness
            .validator_liveness_evidence
            .body
            .challenge
            .body
            .canary_anchor;
        let canary_finality_proof = witness
            .canary_evidence
            .body
            .finality_proof_chain
            .last()
            .ok_or_else(|| invalid("canary finality proof chain is empty"))?;
        let verified_liveness = witness
            .validator_liveness_evidence
            .verify_exact(
                &liveness_bytes,
                &expectations,
                &verified_canary,
                expected_canary,
                canary_finality_proof,
            )
            .map_err(|error| invalid(format!("invalid four-validator liveness: {error}")))?;
        let terminal_finality_proof = witness
            .validator_liveness_evidence
            .body
            .post_canary_finality_proof_chain
            .last()
            .unwrap_or(canary_finality_proof);
        let challenge = &witness.validator_liveness_evidence.body.challenge.body;
        let last_response_ms = witness
            .validator_liveness_evidence
            .body
            .observations
            .iter()
            .map(|observation| observation.response_completed_at_unix_ms)
            .max()
            .ok_or_else(|| invalid("four-validator liveness observations are empty"))?;
        let current_height = state_transaction.block_height();
        let runtime = &expectations.validator_bodies()[0].runtime_effective_config;
        let current_topology = state_transaction
            .commit_topology()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let expected_topology = runtime
            .validators
            .iter()
            .map(|validator| validator.validator_id.clone())
            .collect::<Vec<_>>();
        let canonical_parent_hash = state_transaction.block_hashes().iter().next_back().copied();
        let canonical_genesis_hash = state_transaction.block_hashes().iter().next().copied();
        let manifest = &cached.release_record().manifest;
        if verified_liveness.promotion_id() != promotion_binding.promotion_id
            || !liveness_terminal_is_current_parent(
                terminal_finality_proof.finality_artifact.height,
                terminal_finality_proof.finality_artifact.block_hash,
                current_height,
                state_transaction._curr_block.prev_block_hash(),
                canonical_parent_hash,
            )
            || terminal_finality_proof.finality_artifact.height
                != verified_liveness.highest_observed_tip_height()
            || terminal_finality_proof.block_header.hash()
                != terminal_finality_proof.finality_artifact.block_hash
            || !validator_set_matches(&current_topology, &expected_topology)
            || canonical_genesis_hash != Some(runtime.genesis_expected_hash)
            || state_transaction.chain_id() != &runtime.chain
            || iroha_data_model::account::address::chain_discriminant()
                != runtime.chain_discriminant
            || state_transaction.network_id() != &promotion_binding.network_id
            || state_transaction
                .settlement
                .offline
                .kagemusha_max_decoded_bytes
                != runtime.kagemusha_max_decoded_bytes
            || current_height < manifest.activation_height
            || current_height >= manifest.withdrawal_height
            || now_ms < last_response_ms
            || now_ms >= challenge.expires_at_unix_ms
        {
            return Err(invalid(
                "enable transition is stale, premature, or outside the release window",
            ));
        }

        let enable_witness_norito = KagemushaExactBytesDigestV1::from_bytes(
            &norito::encode_canonical(&witness)
                .map_err(|error| invalid(format!("failed to encode enable witness: {error}")))?,
        )
        .map_err(|error| invalid(format!("invalid enable-witness identity: {error}")))?;
        let validator_liveness_evidence = KagemushaExactBytesDigestV1::from_bytes(&liveness_bytes)
            .map_err(|error| invalid(format!("invalid liveness identity: {error}")))?;
        let enabled = KagemushaV4ReleaseEnabledV1 {
            expected_staged_lifecycle: predecessor,
            transition_id: witness.transition_id,
            enable_witness_norito,
            enable_transaction_intent: current_transaction_intent(state_transaction)?,
            enabled_at_height: current_height,
            enabled_at_unix_ms: now_ms,
            validator_liveness_evidence,
            canary_transaction_intent: verified_liveness.canary_transaction_intent(),
            canary_finalized_height: verified_liveness.canary_finalized_height(),
            canary_finalized_block_hash: verified_liveness.canary_finalized_block_hash(),
            endpoint_challenge: verified_liveness.endpoint_challenge(),
            validator_ids: verified_liveness.validator_ids().clone(),
            observed_tip_heights: *verified_liveness.observed_tip_heights(),
            highest_observed_tip_height: verified_liveness.highest_observed_tip_height(),
        };
        let mut next = loaded.state.clone();
        next.phase = KagemushaV4ReleaseLifecyclePhaseV1::Enabled(enabled);
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            staged_policy_bytes,
        );
        commit_transition(marker, loaded, next, state_transaction)
    }
}

fn withdraw_cancelled_verifiers(
    state: &KagemushaV4ReleaseLifecycleStateV1,
    current_height: u64,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    for id in [
        &state.step_eq_verifier_key_id,
        &state.step_ep_verifier_key_id,
    ] {
        let Some(mut record) = state_transaction.world.verifying_keys.get(id).cloned() else {
            continue;
        };
        if record.version != state.verifier_version {
            continue;
        }
        record.status = ConfidentialStatus::Withdrawn;
        record.withdraw_height = Some(current_height);
        record.key = None;
        record.vk_len = 0;
        state_transaction.world.verifying_keys.remove(id.clone());
        state_transaction
            .world
            .verifying_keys
            .insert(id.clone(), record);
    }
}

impl Execute for CancelKagemushaRecursiveReleaseV4 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_direct_entrypoint(
            LifecycleEntrypointKind::Cancel,
            &iroha_data_model::isi::InstructionBox::from(self.clone()),
            state_transaction,
        )?;
        self.validate()
            .map_err(|error| invalid(format!("invalid cancellation: {error}")))?;
        let cancellation = self.cancellation;
        let loaded =
            load_lifecycle_by_manifest(&state_transaction.world, &cancellation.manifest_sha256)
                .map_err(invalid)?
                .ok_or_else(|| invalid("Kagemusha V4 staged lifecycle is absent"))?;
        require_transition_authority(authority, &loaded, state_transaction)?;
        if cancellation.promotion_id != loaded.state.promotion_binding.promotion_id
            || !matches!(
                loaded.state.phase,
                KagemushaV4ReleaseLifecyclePhaseV1::Staged
            )
            || cancellation.expected_predecessor_lifecycle
                != loaded
                    .state
                    .exact_bytes_digest()
                    .map_err(|error| invalid(format!("invalid predecessor: {error}")))?
        {
            return Err(invalid(
                "cancellation does not match the exact staged state",
            ));
        }
        let marker = transition_marker(&cancellation.transition_id, state_transaction)?;
        let current_height = state_transaction.block_height();
        let cancelled = KagemushaV4ReleaseCancelledV1 {
            cancellation,
            cancellation_transaction_intent: current_transaction_intent(state_transaction)?,
            cancelled_at_height: current_height,
            cancelled_at_unix_ms: state_transaction.block_unix_timestamp_ms(),
        };
        let mut next = loaded.state.clone();
        next.phase = KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancelled);
        next.validate()
            .map_err(|error| invalid(format!("invalid cancellation transition: {error}")))?;
        withdraw_cancelled_verifiers(&loaded.state, current_height, state_transaction);
        commit_transition(marker, loaded, next, state_transaction)
    }
}

impl Execute for DeactivateKagemushaRecursiveIssuanceV4 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_direct_entrypoint(
            LifecycleEntrypointKind::Deactivate,
            &iroha_data_model::isi::InstructionBox::from(self.clone()),
            state_transaction,
        )?;
        self.validate()
            .map_err(|error| invalid(format!("invalid deactivation: {error}")))?;
        let deactivation = self.deactivation;
        let loaded =
            load_lifecycle_by_manifest(&state_transaction.world, &deactivation.manifest_sha256)
                .map_err(invalid)?
                .ok_or_else(|| invalid("Kagemusha V4 enabled lifecycle is absent"))?;
        require_transition_authority(authority, &loaded, state_transaction)?;
        let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(enabled) = &loaded.state.phase else {
            return Err(invalid(
                "only enabled Kagemusha V4 issuance may be deactivated",
            ));
        };
        if deactivation.promotion_id != loaded.state.promotion_binding.promotion_id
            || deactivation.expected_predecessor_lifecycle
                != loaded
                    .state
                    .exact_bytes_digest()
                    .map_err(|error| invalid(format!("invalid predecessor: {error}")))?
        {
            return Err(invalid(
                "deactivation does not match the exact enabled state",
            ));
        }
        let marker = transition_marker(&deactivation.transition_id, state_transaction)?;
        let deactivated = KagemushaV4ReleaseDeactivatedV1 {
            enabled: enabled.clone(),
            deactivation,
            deactivation_transaction_intent: current_transaction_intent(state_transaction)?,
            deactivated_at_height: state_transaction.block_height(),
            deactivated_at_unix_ms: state_transaction.block_unix_timestamp_ms(),
        };
        let mut next = loaded.state.clone();
        next.phase = KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated);
        commit_transition(marker, loaded, next, state_transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::World;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::{MultisigMember, MultisigPolicy},
        block::BlockHeader,
        offline::{
            KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1, KagemushaV4ReleaseCancellationV1,
            KagemushaV4ReleaseLifecycleReasonV1,
        },
        transaction::{FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder},
    };

    fn cancellation(transition_byte: u8) -> CancelKagemushaRecursiveReleaseV4 {
        CancelKagemushaRecursiveReleaseV4::new(KagemushaV4ReleaseCancellationV1 {
            schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [0x11; 32],
            manifest_sha256: [0x22; 32],
            expected_predecessor_lifecycle: KagemushaExactBytesDigestV1 {
                byte_len: 1,
                sha256: [0x33; 32],
            },
            transition_id: [transition_byte; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
            evidence: None,
        })
    }

    fn signed(
        instructions: impl IntoIterator<Item = CancelKagemushaRecursiveReleaseV4>,
    ) -> SignedTransaction {
        let key = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .expect("derive lifecycle context test key");
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"lifecycle-context-test")),
        );
        TransactionBuilder::new(
            network_id,
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(key.private_key())
    }

    fn terminal_lifecycle_with_retained_policy() -> (
        KagemushaRecursiveSpendArtifactBindingV4,
        KagemushaV4ReleaseLifecycleStateV1,
        OfflineDeviceAttestationPolicy,
    ) {
        let controller = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519)
            .expect("derive lifecycle promotion controller");
        let governance_members = (0_u8..3)
            .map(|index| {
                let key = KeyPair::try_from_seed(vec![0x70 + index; 32], Algorithm::Ed25519)
                    .expect("derive lifecycle governance member");
                MultisigMember::new(key.public_key().clone(), 1)
                    .expect("valid lifecycle governance member")
            })
            .collect();
        let governance_authority = AccountId::new_multisig(
            MultisigPolicy::new(3, governance_members).expect("valid lifecycle governance"),
        );
        let manifest_sha256 = [0x22; 32];
        let release_record_norito =
            KagemushaExactBytesDigestV1::from_bytes(b"release-record").expect("release identity");
        let device_attestation_policy =
            default_offline_device_attestation_policy().expect("built-in attestation policy");
        let device_attestation_policy_norito = KagemushaExactBytesDigestV1::from_bytes(
            &norito::encode_canonical(&device_attestation_policy)
                .expect("canonical retained attestation policy"),
        )
        .expect("retained policy identity");
        let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "release-v4".to_owned(),
            manifest_sha256,
        };
        let promotion_binding = KagemushaV4PromotionBindingV1 {
            promotion_controller: controller.public_key().clone(),
            promotion_reservation: KagemushaExactBytesDigestV1::from_bytes(b"reservation")
                .expect("reservation identity"),
            promotion_id: [0x11; 32],
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"lifecycle-redemption-policy-test",
                )),
            ),
            reviewed_source_closure_descriptor_sha256: [0x33; 32],
            manifest_sha256,
            release_record_sha256: release_record_norito.sha256,
            release_policy_source: KagemushaExactBytesDigestV1::from_bytes(b"release-policy")
                .expect("release policy identity"),
            device_attestation_policy_norito,
            signed_genesis: KagemushaExactBytesDigestV1::from_bytes(b"signed-genesis")
                .expect("genesis identity"),
            catalog_consensus_policy_digest: [0x44; 32],
            execution_policy_hash: Hash::new(b"execution-policy"),
        };
        let mut state = KagemushaV4ReleaseLifecycleStateV1 {
            schema: KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_binding,
            artifact_binding: artifact_binding.clone(),
            governance_authority,
            stage_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
                b"stage-transaction",
            )),
            staged_at_height: 2,
            staged_at_unix_ms: 1_800_000_000_000,
            release_record_norito,
            device_attestation_policy: device_attestation_policy.clone(),
            step_eq_verifier_key_id:
                iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                    iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
                    manifest_sha256,
                ),
            step_ep_verifier_key_id:
                iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                    iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
                    manifest_sha256,
                ),
            verifier_version: 1,
            phase: KagemushaV4ReleaseLifecyclePhaseV1::Staged,
        };
        let predecessor = state.exact_bytes_digest().expect("valid staged lifecycle");
        state.phase =
            KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(KagemushaV4ReleaseCancelledV1 {
                cancellation: KagemushaV4ReleaseCancellationV1 {
                    schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
                    version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                    promotion_id: state.promotion_binding.promotion_id,
                    manifest_sha256,
                    expected_predecessor_lifecycle: predecessor,
                    transition_id: [0x55; 32],
                    reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
                    evidence: None,
                },
                cancellation_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
                    b"cancel-transaction",
                )),
                cancelled_at_height: 3,
                cancelled_at_unix_ms: 1_800_000_000_001,
            });
        state.validate().expect("valid terminal lifecycle");
        (artifact_binding, state, device_attestation_policy)
    }

    #[test]
    fn redemption_policy_is_available_in_terminal_lifecycle_state() {
        let (binding, lifecycle, expected_policy) = terminal_lifecycle_with_retained_policy();
        let key = lifecycle_key(&binding.manifest_sha256).expect("lifecycle state key");
        let mut world = World::default();
        world.smart_contract_state.insert(
            key,
            norito::encode_canonical(&lifecycle).expect("canonical terminal lifecycle"),
        );

        assert_eq!(
            redemption_policy(&world, &binding).expect("terminal redemption policy"),
            expected_policy,
        );
    }

    #[test]
    fn direct_lifecycle_context_binds_exact_instruction_and_transaction() {
        let instruction = cancellation(0x44);
        let boxed = iroha_data_model::isi::InstructionBox::from(instruction.clone());
        let transaction = signed([instruction]);
        let context = signed_lifecycle_entrypoint_context(&transaction)
            .expect("classify direct lifecycle transaction")
            .expect("direct lifecycle context");
        assert_eq!(context.kind, LifecycleEntrypointKind::Cancel);
        assert_eq!(context.transaction_intent, transaction.hash());
        assert_eq!(
            context.instruction_digest,
            Hash::new(&norito::encode_canonical(&boxed).expect("encode exact instruction"))
        );

        let changed = signed([cancellation(0x45)]);
        let changed_context = signed_lifecycle_entrypoint_context(&changed)
            .expect("classify changed lifecycle transaction")
            .expect("changed direct lifecycle context");
        assert_ne!(
            context.instruction_digest,
            changed_context.instruction_digest
        );
        assert_ne!(
            context.transaction_intent,
            changed_context.transaction_intent
        );
    }

    #[test]
    fn lifecycle_context_rejects_multi_instruction_and_nonordinary_carriers() {
        let multiple = signed([cancellation(0x46), cancellation(0x47)]);
        assert_eq!(
            signed_lifecycle_entrypoint_context(&multiple)
                .expect("classify multiple lifecycle instructions"),
            None
        );

        let key = KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
            .expect("derive nonordinary lifecycle test key");
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"lifecycle-context-nonordinary-test",
            )));
        let queue_plan = TransactionBuilder::new(
            network_id,
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([cancellation(0x48)])
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
        .sign(key.private_key());
        assert_eq!(
            signed_lifecycle_entrypoint_context(&queue_plan)
                .expect("classify nonordinary lifecycle transaction"),
            None
        );
    }

    #[test]
    fn direct_lifecycle_context_is_consumed_exactly_once() {
        let transaction = signed([cancellation(0x49)]);
        let mut context = signed_lifecycle_entrypoint_context(&transaction)
            .expect("classify lifecycle context")
            .expect("direct lifecycle context");
        let digest = context.instruction_digest;
        let intent = context.transaction_intent.clone();
        let mut slot = Some(context.clone());
        assert!(consume_direct_entrypoint(
            &mut slot,
            true,
            Some(&intent),
            LifecycleEntrypointKind::Cancel,
            digest,
        ));
        assert!(slot.is_none());
        assert!(!consume_direct_entrypoint(
            &mut slot,
            true,
            Some(&intent),
            LifecycleEntrypointKind::Cancel,
            digest,
        ));

        context.kind = LifecycleEntrypointKind::Enable;
        let mut mismatched = Some(context);
        assert!(!consume_direct_entrypoint(
            &mut mismatched,
            true,
            Some(&intent),
            LifecycleEntrypointKind::Cancel,
            digest,
        ));
        assert!(mismatched.is_none());
    }

    #[test]
    fn liveness_terminal_must_be_the_exact_canonical_parent() {
        let terminal = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"kagemusha-liveness-terminal",
        ));
        let fork =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"kagemusha-liveness-fork"));
        assert!(liveness_terminal_is_current_parent(
            7,
            terminal,
            8,
            Some(terminal),
            Some(terminal),
        ));
        assert!(!liveness_terminal_is_current_parent(
            7,
            terminal,
            9,
            Some(terminal),
            Some(terminal),
        ));
        assert!(!liveness_terminal_is_current_parent(
            7,
            terminal,
            8,
            Some(terminal),
            Some(fork),
        ));
    }

    #[test]
    fn runtime_validator_set_is_exact_but_rotation_independent() {
        let mut expected = (0_u8..4)
            .map(|index| {
                iroha_data_model::peer::PeerId::new(
                    KeyPair::from_seed(vec![0x80 + index; 32], Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                )
            })
            .collect::<Vec<_>>();
        expected.sort();
        let mut rotated = expected.clone();
        rotated.rotate_left(1);
        assert!(validator_set_matches(&rotated, &expected));

        let replacement = iroha_data_model::peer::PeerId::new(
            KeyPair::from_seed(vec![0x90; 32], Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        rotated[0] = replacement;
        assert!(!validator_set_matches(&rotated, &expected));
        assert!(!validator_set_matches(&expected[..3], &expected));
    }
}
