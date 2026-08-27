//! Consensus-enforced staged lifecycle for Kagemusha V4 release issuance.

use super::*;

use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1,
    KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS,
    KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1,
    KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_SCHEMA_V1, KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
    KagemushaExactBytesDigestV1, KagemushaRecursiveSpendArtifactBindingV4,
    KagemushaV4PromotionBindingV1, KagemushaV4ReleaseCancelledV1, KagemushaV4ReleaseDeactivatedV1,
    KagemushaV4ReleaseEnabledV1, KagemushaV4ReleaseLifecyclePhaseV1,
    KagemushaV4ReleaseLifecycleStateV1, kagemusha_v4_release_lifecycle_state_key,
};
use iroha_data_model::proof::VerifyingKeyId;

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
    stage_expires_at_height: Option<u64>,
}

impl LifecycleEntrypointContext {
    /// Reject a Stage carrier whose height expiry can never fit the receipt's
    /// bounded successor chain, even under the newest possible trusted anchor.
    pub(crate) fn validate_stage_expiry_horizon(
        &self,
        carrier_height: u64,
    ) -> Result<(), iroha_data_model::ValidationFail> {
        if self.kind != LifecycleEntrypointKind::Stage {
            return Ok(());
        }
        let expiry = self.stage_expires_at_height.ok_or_else(|| {
            iroha_data_model::ValidationFail::NotPermitted(
                "Kagemusha V4 Stage lifecycle transaction requires a nonzero expires_at_height"
                    .to_owned(),
            )
        })?;
        // A valid receipt has at least one successor after its pre-submission
        // anchor, so `anchor < carrier`. Its exclusive expiry is bounded by
        // `anchor + proof_count_max + 1`, hence necessarily by
        // `carrier + proof_count_max`.
        let proof_count_max = u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
            .expect("Kagemusha V4 finality proof-count bound fits u64");
        let maximum_possible_expiry = carrier_height.saturating_add(proof_count_max);
        if expiry > maximum_possible_expiry {
            return Err(iroha_data_model::ValidationFail::NotPermitted(format!(
                "Kagemusha V4 Stage lifecycle expires_at_height {expiry} exceeds the receipt-capable maximum {maximum_possible_expiry} at carrier height {carrier_height}"
            )));
        }
        Ok(())
    }
}

fn lifecycle_expires_at_height(
    transaction: &SignedTransaction,
    kind: LifecycleEntrypointKind,
) -> Result<Option<u64>, iroha_data_model::ValidationFail> {
    // Height TTL is generic transaction metadata and may be required by
    // network policy for every lifecycle kind. Stage alone makes its presence
    // part of the later activation-receipt contract.
    let expiry = transaction.expires_at_height().map_err(|error| {
        iroha_data_model::ValidationFail::NotPermitted(format!(
            "Kagemusha V4 {kind:?} lifecycle expires_at_height must be an unsigned integer: {error}"
        ))
    })?;
    if kind == LifecycleEntrypointKind::Stage && expiry.is_none_or(|height| height == 0) {
        return Err(iroha_data_model::ValidationFail::NotPermitted(
            "Kagemusha V4 Stage lifecycle transaction requires a nonzero expires_at_height"
                .to_owned(),
        ));
    }
    Ok(expiry)
}

fn require_distinct_governance_signers(
    transaction: &SignedTransaction,
    kind: LifecycleEntrypointKind,
) -> Result<(), iroha_data_model::ValidationFail> {
    transaction.verify_signature().map_err(|error| {
        iroha_data_model::ValidationFail::NotPermitted(format!(
            "Kagemusha V4 {kind:?} lifecycle signature verification failed: {error}"
        ))
    })?;
    let signer_count = transaction
        .multisig_signatures()
        .map_or(0, |bundle| bundle.signatures.len());
    if signer_count < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS {
        return Err(iroha_data_model::ValidationFail::NotPermitted(format!(
            "Kagemusha V4 {kind:?} lifecycle transaction requires at least {KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS} verified distinct governance signers"
        )));
    }
    Ok(())
}

fn require_no_proof_attachments(
    transaction: &SignedTransaction,
) -> Result<(), iroha_data_model::ValidationFail> {
    if transaction.attachments().is_some() {
        return Err(iroha_data_model::ValidationFail::NotPermitted(
            "Kagemusha V4 lifecycle transactions must not carry proof attachments".to_owned(),
        ));
    }
    Ok(())
}

/// Derive lifecycle context only from one ordinary, direct native instruction carrying at least
/// two verified canonical distinct governance signatures and no proof attachments. Stage also
/// requires the nonzero height expiry later bounded against its carrier height.
pub(crate) fn signed_lifecycle_entrypoint_context(
    transaction: &SignedTransaction,
) -> Result<Option<LifecycleEntrypointContext>, iroha_data_model::ValidationFail> {
    use iroha_data_model::transaction::{Executable, TransactionAdmissionIntent};

    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Ok(None);
    };
    let [instruction] = instructions.as_ref() else {
        return Ok(None);
    };
    let Some(kind) = direct_lifecycle_entrypoint_kind(instruction) else {
        return Ok(None);
    };
    if transaction.admission_intent() != TransactionAdmissionIntent::Ordinary {
        return Err(iroha_data_model::ValidationFail::NotPermitted(
            "Kagemusha V4 lifecycle mutation requires ordinary transaction admission".to_owned(),
        ));
    }
    require_no_proof_attachments(transaction)?;
    require_distinct_governance_signers(transaction, kind)?;
    let expires_at_height = lifecycle_expires_at_height(transaction, kind)?;
    let stage_expires_at_height = match kind {
        LifecycleEntrypointKind::Stage => {
            Some(expires_at_height.expect("Stage expiry was checked above"))
        }
        LifecycleEntrypointKind::Enable
        | LifecycleEntrypointKind::Cancel
        | LifecycleEntrypointKind::Deactivate => None,
    };
    Ok(Some(LifecycleEntrypointContext {
        kind,
        transaction_intent: transaction.hash(),
        instruction_digest: Hash::new(&norito::encode_canonical(instruction).map_err(|error| {
            iroha_data_model::ValidationFail::InternalError(format!(
                "failed to encode exact Kagemusha lifecycle instruction: {error}"
            ))
        })?),
        stage_expires_at_height,
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

fn exact_lifecycle_verifier_record<'world>(
    world: &'world impl WorldReadOnly,
    state: &KagemushaV4ReleaseLifecycleStateV1,
    id: &VerifyingKeyId,
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    role: &str,
) -> Result<&'world VerifyingKeyRecord, String> {
    let record = world
        .verifying_keys()
        .get(id)
        .ok_or_else(|| format!("Kagemusha V4 lifecycle {role} verifier is absent"))?;
    ensure_release_qualified_kagemusha_v4_verifier_id(id, record, parity, role)?;
    let expected_index = (
        kagemusha_v4_circuit_id(parity).to_owned(),
        state.verifier_version,
    );
    if record.version != state.verifier_version
        || record.status != ConfidentialStatus::Active
        || world.verifying_keys_by_circuit().get(&expected_index) != Some(id)
    {
        return Err(format!(
            "Kagemusha V4 lifecycle {role} verifier record changed"
        ));
    }
    Ok(record)
}

fn exact_lifecycle_verifier_records<'world>(
    world: &'world impl WorldReadOnly,
    state: &KagemushaV4ReleaseLifecycleStateV1,
) -> Result<[&'world VerifyingKeyRecord; 2], String> {
    let binding = &state.artifact_binding;
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
    Ok([
        exact_lifecycle_verifier_record(
            world,
            state,
            &state.step_eq_verifier_key_id,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            "Eq",
        )?,
        exact_lifecycle_verifier_record(
            world,
            state,
            &state.step_ep_verifier_key_id,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
            "Ep",
        )?,
    ])
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
    exact_lifecycle_verifier_records(world, state)?;
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

fn active_runtime_effective_config_sha256(
    world: &impl WorldReadOnly,
) -> Result<Option<[u8; 32]>, String> {
    let range_start: StatePath = KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1
        .parse()
        .map_err(|_| "Kagemusha V4 lifecycle state-key prefix is invalid".to_owned())?;
    let mut active = None;
    for (key, bytes) in world.smart_contract_state().range(range_start..) {
        if !key
            .as_ref()
            .starts_with(KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1)
        {
            break;
        }
        let state = KagemushaV4ReleaseLifecycleStateV1::decode_canonical(bytes)
            .map_err(|error| format!("invalid Kagemusha V4 lifecycle state: {error}"))?;
        if key != &lifecycle_key(&state.artifact_binding.manifest_sha256)? {
            return Err(
                "Kagemusha V4 lifecycle state is stored under a non-canonical key".to_owned(),
            );
        }
        if !matches!(
            &state.phase,
            KagemushaV4ReleaseLifecyclePhaseV1::Staged
                | KagemushaV4ReleaseLifecyclePhaseV1::Enabled(_)
        ) {
            continue;
        }
        if active
            .replace(state.runtime_effective_config_sha256)
            .is_some()
        {
            return Err("multiple active Kagemusha V4 release lifecycle records exist".to_owned());
        }
    }
    Ok(active)
}

/// Fail closed unless every active lifecycle is locked to this node's startup projection.
pub(crate) fn require_local_runtime_effective_config(
    world: &impl WorldReadOnly,
    local_runtime_effective_config_sha256: Option<[u8; 32]>,
) -> Result<(), String> {
    let Some(expected) = active_runtime_effective_config_sha256(world)? else {
        return Ok(());
    };
    if local_runtime_effective_config_sha256 != Some(expected) {
        return Err(
            "active Kagemusha V4 release requires a different complete runtime projection"
                .to_owned(),
        );
    }
    Ok(())
}

/// Return whether consensus parameters are frozen by a staged or enabled release.
pub(crate) fn runtime_consensus_parameters_frozen(
    world: &impl WorldReadOnly,
) -> Result<bool, String> {
    active_runtime_effective_config_sha256(world).map(|active| active.is_some())
}

/// Reject the on-chain consensus-parameter routes bound by Kagemusha qualification.
///
/// An installed authenticated catalog already participates in the execution-policy digest, so
/// locking at that boundary closes the interval between validator qualification and Stage.
pub(crate) fn validate_runtime_consensus_parameter_update(
    parameter: &iroha_data_model::parameter::Parameter,
    world: &impl WorldReadOnly,
    catalog_configured: bool,
) -> Result<(), Error> {
    use iroha_data_model::{
        isi::error::{InstructionExecutionError, InvalidParameterError},
        parameter::{Parameter, system::SumeragiParameter},
    };

    let is_bound = match parameter {
        Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(_)) => true,
        Parameter::Custom(custom) => {
            custom.id()
                == &iroha_data_model::parameter::system::SumeragiNposParameters::parameter_id()
        }
        _ => false,
    };
    if !is_bound {
        return Ok(());
    }
    let lifecycle_frozen = runtime_consensus_parameters_frozen(world).map_err(|error| {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(format!(
            "invalid Kagemusha release lifecycle while checking consensus parameter lock: {error}"
        )))
    })?;
    if catalog_configured || lifecycle_frozen {
        return Err(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(
                "authenticated Kagemusha qualification freezes consensus runtime parameters"
                    .to_owned(),
            ),
        ));
    }
    Ok(())
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
    runtime_effective_config_sha256: [u8; 32],
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
        runtime_effective_config_sha256,
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
    if active_runtime_effective_config_sha256(&state_transaction.world)
        .map_err(invalid)?
        .is_some()
    {
        return Err(invalid(
            "another Kagemusha V4 release is already staged or enabled",
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
        let runtime_effective_config_sha256 = runtime
            .consensus_sha256()
            .map_err(|error| invalid(format!("invalid runtime projection identity: {error}")))?;
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
            || loaded.state.runtime_effective_config_sha256 != runtime_effective_config_sha256
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
        next.phase = KagemushaV4ReleaseLifecyclePhaseV1::Enabled(Box::new(enabled));
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            staged_policy_bytes,
        );
        commit_transition(marker, loaded, next, state_transaction)
    }
}

struct CancelledVerifierWithdrawalPlan {
    records: [(VerifyingKeyId, VerifyingKeyRecord); 2],
}

impl CancelledVerifierWithdrawalPlan {
    fn apply(self, state_transaction: &mut StateTransaction<'_, '_>) {
        for (id, record) in self.records {
            state_transaction.world.verifying_keys.remove(id.clone());
            state_transaction.world.verifying_keys.insert(id, record);
        }
    }
}

fn plan_cancelled_verifier_withdrawal(
    state: &KagemushaV4ReleaseLifecycleStateV1,
    current_height: u64,
    world: &impl WorldReadOnly,
) -> Result<CancelledVerifierWithdrawalPlan, String> {
    let [step_eq, step_ep] = exact_lifecycle_verifier_records(world, state)?;
    let mut records = [
        (state.step_eq_verifier_key_id.clone(), step_eq.clone()),
        (state.step_ep_verifier_key_id.clone(), step_ep.clone()),
    ];
    for (_, record) in &mut records {
        record.status = ConfidentialStatus::Withdrawn;
        // A staged release is cancelled before its scheduled activation. Clear
        // that never-reached boundary so the retained tombstone cannot encode
        // an inverted activation/withdrawal interval.
        record.activation_height = None;
        record.withdraw_height = Some(current_height);
        record.key = None;
        record.vk_len = 0;
    }
    Ok(CancelledVerifierWithdrawalPlan { records })
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
        next.phase = KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(Box::new(cancelled));
        next.validate()
            .map_err(|error| invalid(format!("invalid cancellation transition: {error}")))?;
        let withdrawal = plan_cancelled_verifier_withdrawal(
            &loaded.state,
            current_height,
            &state_transaction.world,
        )
        .map_err(invalid)?;
        commit_transition(marker, loaded, next, state_transaction)?;
        withdrawal.apply(state_transaction);
        Ok(())
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
            enabled: enabled.as_ref().clone(),
            deactivation,
            deactivation_transaction_intent: current_transaction_intent(state_transaction)?,
            deactivated_at_height: state_transaction.block_height(),
            deactivated_at_unix_ms: state_transaction.block_unix_timestamp_ms(),
        };
        let mut next = loaded.state.clone();
        next.phase = KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(Box::new(deactivated));
        commit_transition(marker, loaded, next, state_transaction)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId, NetworkId, Registrable,
        account::{Account, MultisigMember, MultisigPolicy},
        block::BlockHeader,
        isi::offline::DeactivateKagemushaRecursiveIssuanceV4,
        offline::{
            KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT, KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1,
            KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1, KagemushaV4ReleaseCancellationV1,
            KagemushaV4ReleaseDeactivationV1, KagemushaV4ReleaseLifecycleReasonV1,
        },
        permission::{Permission, Permissions},
        transaction::{
            FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder,
            TransactionEntrypoint, signed::SealedTransactionReveal,
        },
    };
    use iroha_primitives::json::Json;

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
        signed_with_metadata(
            instructions,
            iroha_data_model::metadata::Metadata::default(),
        )
    }

    fn signed_with_metadata(
        instructions: impl IntoIterator<Item = CancelKagemushaRecursiveReleaseV4>,
        metadata: iroha_data_model::metadata::Metadata,
    ) -> SignedTransaction {
        let keys = [0x51_u8, 0x52].map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive lifecycle context test key")
        });
        let policy = MultisigPolicy::new(
            2,
            keys.iter()
                .map(|key| {
                    MultisigMember::new(key.public_key().clone(), 1)
                        .expect("valid lifecycle context member")
                })
                .collect(),
        )
        .expect("valid lifecycle context policy");
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"lifecycle-context-test")),
        );
        TransactionBuilder::new(
            network_id,
            AccountId::new_multisig(policy),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .with_metadata(metadata)
        .sign_multisig([keys[0].private_key(), keys[1].private_key()])
    }

    fn signed_with_height_expiry(expiry: u64) -> SignedTransaction {
        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            "expires_at_height"
                .parse()
                .expect("valid expiry metadata key"),
            Json::from(expiry),
        );
        signed_with_metadata([cancellation(0x43)], metadata)
    }

    fn weighted_signed(
        instruction: CancelKagemushaRecursiveReleaseV4,
        include_second_signer: bool,
    ) -> SignedTransaction {
        let keys = [0x53_u8, 0x54].map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive weighted lifecycle context test key")
        });
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(keys[0].public_key().clone(), 2)
                    .expect("valid weight-two lifecycle member"),
                MultisigMember::new(keys[1].public_key().clone(), 1)
                    .expect("valid weight-one lifecycle member"),
            ],
        )
        .expect("valid weighted lifecycle context policy");
        let builder = TransactionBuilder::new(
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"weighted-lifecycle-context-test",
            ))),
            AccountId::new_multisig(policy),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction]);
        if include_second_signer {
            builder.sign_multisig([keys[0].private_key(), keys[1].private_key()])
        } else {
            builder.sign_multisig([keys[0].private_key()])
        }
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
            super::super::isi::default_offline_device_attestation_policy()
                .expect("built-in attestation policy");
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
            runtime_effective_config_sha256: [0x55; 32],
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
        state.phase = KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(Box::new(
            KagemushaV4ReleaseCancelledV1 {
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
            },
        ));
        state.validate().expect("valid terminal lifecycle");
        (artifact_binding, state, device_attestation_policy)
    }

    /// Build one structurally valid staged lifecycle for production-path consensus tests.
    pub(crate) fn staged_lifecycle_for_test(
        runtime_effective_config_sha256: [u8; 32],
        network_id: NetworkId,
    ) -> KagemushaV4ReleaseLifecycleStateV1 {
        let (_, mut lifecycle, _) = terminal_lifecycle_with_retained_policy();
        lifecycle.phase = KagemushaV4ReleaseLifecyclePhaseV1::Staged;
        lifecycle.runtime_effective_config_sha256 = runtime_effective_config_sha256;
        lifecycle.promotion_binding.network_id = network_id;
        lifecycle
            .validate()
            .expect("valid staged lifecycle test fixture");
        lifecycle
    }

    fn lifecycle_governance_keys() -> Vec<KeyPair> {
        (0_u8..3)
            .map(|index| {
                KeyPair::try_from_seed(vec![0x70 + index; 32], Algorithm::Ed25519)
                    .expect("derive lifecycle governance member")
            })
            .collect()
    }

    fn lifecycle_transaction(
        lifecycle: &KagemushaV4ReleaseLifecycleStateV1,
        instruction: iroha_data_model::isi::InstructionBox,
    ) -> SignedTransaction {
        let keys = lifecycle_governance_keys();
        let authority = AccountId::new_multisig(
            MultisigPolicy::new(
                3,
                keys.iter()
                    .map(|key| {
                        MultisigMember::new(key.public_key().clone(), 1)
                            .expect("valid lifecycle governance member")
                    })
                    .collect(),
            )
            .expect("valid lifecycle governance policy"),
        );
        assert_eq!(authority, lifecycle.governance_authority);
        TransactionBuilder::new(
            lifecycle.promotion_binding.network_id,
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .with_admission_intent(TransactionAdmissionIntent::Ordinary)
        .sign_multisig(keys.iter().map(KeyPair::private_key))
    }

    fn lifecycle_state(lifecycle: &KagemushaV4ReleaseLifecycleStateV1) -> State {
        let authority = lifecycle.governance_authority.clone();
        let mut world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
        let mut permissions = Permissions::new();
        for name in [
            CAN_ACTIVATE_KAGEMUSHA_RECURSIVE_RELEASE_V4_PERMISSION,
            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
        ] {
            permissions.insert(Permission::new(name.to_owned(), Json::new(())));
        }
        world.account_permissions.insert(authority, permissions);
        world.smart_contract_state.insert(
            lifecycle_key(&lifecycle.artifact_binding.manifest_sha256)
                .expect("canonical lifecycle key"),
            norito::encode_canonical(lifecycle).expect("canonical lifecycle fixture"),
        );
        world.smart_contract_state.insert(
            kagemusha_terminal_registry_v4::release_state_key(&lifecycle.artifact_binding)
                .expect("canonical release-state key"),
            b"release-record".to_vec(),
        );
        let owner =
            kagemusha_terminal_registry_v4::verifier_owner_manifest_id(&lifecycle.artifact_binding)
                .expect("canonical verifier owner");
        for (id, parity, curve, key_byte) in [
            (
                &lifecycle.step_eq_verifier_key_id,
                iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
                0x41,
            ),
            (
                &lifecycle.step_ep_verifier_key_id,
                iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V4,
                0x42,
            ),
        ] {
            let key_bytes = vec![key_byte; 32];
            let key = VerifyingKeyBox::new(
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
                key_bytes,
            );
            let commitment = crate::zk::hash_vk(&key);
            let mut record = VerifyingKeyRecord::new_with_owner(
                lifecycle.verifier_version,
                kagemusha_v4_circuit_id(parity),
                Some(owner.clone()),
                iroha_data_model::offline::KAGEMUSHA_VERIFIER_NAMESPACE,
                BackendTag::Halo2IpaPasta,
                curve,
                [0x31; 32],
                commitment,
            );
            record.vk_len = u32::try_from(key.bytes.len()).expect("bounded verifier fixture");
            record.max_proof_bytes = 1_024;
            record.activation_height = Some(
                lifecycle
                    .staged_at_height
                    .checked_add(10)
                    .expect("future verifier activation height"),
            );
            record.key = Some(key);
            record.status = ConfidentialStatus::Active;
            world
                .verifying_keys_by_circuit
                .insert((record.circuit_id.clone(), record.version), id.clone());
            world.verifying_keys.insert(id.clone(), record);
        }
        State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("kagemusha-lifecycle-execution-test"),
            lifecycle.promotion_binding.network_id,
        )
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CancelVerifierCorruption {
        MissingRecord,
        SubstitutedLifecycleId,
        OwnerMismatch,
        EpOwnerMismatch,
        VersionMismatch,
        StatusMismatch,
        IndexMismatch,
    }

    fn assert_cancel_verifier_corruption_is_atomic(
        corruption: CancelVerifierCorruption,
        expected_error: &str,
    ) {
        use iroha_data_model::offline::KagemushaPastaCycleParityV1;

        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"kagemusha-invalid-cancel-verifier-network",
            )));
        let lifecycle = staged_lifecycle_for_test([0x55; 32], network_id);
        let mut persisted_lifecycle = lifecycle.clone();
        if corruption == CancelVerifierCorruption::SubstitutedLifecycleId {
            persisted_lifecycle.step_eq_verifier_key_id =
                iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                    KagemushaPastaCycleParityV1::StepEq,
                    [0xE1; 32],
                );
            persisted_lifecycle
                .validate()
                .expect("portable substituted lifecycle verifier id remains structural");
        }
        let predecessor = persisted_lifecycle
            .exact_bytes_digest()
            .expect("staged lifecycle identity");
        let transition_id = [0xD1; 32];
        let instruction =
            CancelKagemushaRecursiveReleaseV4::new(KagemushaV4ReleaseCancellationV1 {
                schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
                version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                promotion_id: persisted_lifecycle.promotion_binding.promotion_id,
                manifest_sha256: persisted_lifecycle.promotion_binding.manifest_sha256,
                expected_predecessor_lifecycle: predecessor,
                transition_id,
                reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
                evidence: None,
            });
        let signed = lifecycle_transaction(&persisted_lifecycle, instruction.clone().into());
        let state = lifecycle_state(&lifecycle);
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(3).expect("nonzero lifecycle height"),
            None,
            None,
            None,
            1_800_000_000_002,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        transaction.kagemusha_taira_canary_external_entrypoint = true;
        transaction.current_tx_hash = Some(signed.hash());
        transaction.kagemusha_release_lifecycle_entrypoint =
            signed_lifecycle_entrypoint_context(&signed)
                .expect("authenticate direct lifecycle transaction");

        let lifecycle_key = lifecycle_key(&lifecycle.artifact_binding.manifest_sha256)
            .expect("canonical lifecycle key");
        if corruption == CancelVerifierCorruption::SubstitutedLifecycleId {
            transaction
                .world
                .smart_contract_state
                .remove(lifecycle_key.clone());
            transaction.world.smart_contract_state.insert(
                lifecycle_key.clone(),
                norito::encode_canonical(&persisted_lifecycle)
                    .expect("canonical substituted lifecycle fixture"),
            );
        }

        let eq_id = lifecycle.step_eq_verifier_key_id.clone();
        let ep_id = lifecycle.step_ep_verifier_key_id.clone();
        let eq_index = (
            kagemusha_v4_circuit_id(KagemushaPastaCycleParityV1::StepEq).to_owned(),
            lifecycle.verifier_version,
        );
        let ep_index = (
            kagemusha_v4_circuit_id(KagemushaPastaCycleParityV1::StepEp).to_owned(),
            lifecycle.verifier_version,
        );
        match corruption {
            CancelVerifierCorruption::MissingRecord => {
                transaction.world.verifying_keys.remove(eq_id.clone());
            }
            CancelVerifierCorruption::SubstitutedLifecycleId => {}
            CancelVerifierCorruption::OwnerMismatch => {
                let mut record = transaction
                    .world
                    .verifying_keys
                    .get(&eq_id)
                    .expect("Eq verifier fixture")
                    .clone();
                record.owner_manifest_id = Some(
                    iroha_data_model::offline::kagemusha_recursive_spend_verifier_owner_manifest_id_v4(
                        [0xE2; 32],
                    ),
                );
                transaction.world.verifying_keys.remove(eq_id.clone());
                transaction
                    .world
                    .verifying_keys
                    .insert(eq_id.clone(), record);
            }
            CancelVerifierCorruption::EpOwnerMismatch => {
                let mut record = transaction
                    .world
                    .verifying_keys
                    .get(&ep_id)
                    .expect("Ep verifier fixture")
                    .clone();
                record.owner_manifest_id = Some(
                    iroha_data_model::offline::kagemusha_recursive_spend_verifier_owner_manifest_id_v4(
                        [0xE3; 32],
                    ),
                );
                transaction.world.verifying_keys.remove(ep_id.clone());
                transaction
                    .world
                    .verifying_keys
                    .insert(ep_id.clone(), record);
            }
            CancelVerifierCorruption::VersionMismatch => {
                let mut record = transaction
                    .world
                    .verifying_keys
                    .get(&eq_id)
                    .expect("Eq verifier fixture")
                    .clone();
                record.version = record.version.saturating_add(1);
                transaction.world.verifying_keys.remove(eq_id.clone());
                transaction
                    .world
                    .verifying_keys
                    .insert(eq_id.clone(), record);
            }
            CancelVerifierCorruption::StatusMismatch => {
                let mut record = transaction
                    .world
                    .verifying_keys
                    .get(&eq_id)
                    .expect("Eq verifier fixture")
                    .clone();
                record.status = ConfidentialStatus::Proposed;
                transaction.world.verifying_keys.remove(eq_id.clone());
                transaction
                    .world
                    .verifying_keys
                    .insert(eq_id.clone(), record);
            }
            CancelVerifierCorruption::IndexMismatch => {
                transaction
                    .world
                    .verifying_keys_by_circuit
                    .remove(eq_index.clone());
                transaction
                    .world
                    .verifying_keys_by_circuit
                    .insert(eq_index.clone(), ep_id.clone());
            }
        }

        let lifecycle_before = transaction
            .world
            .smart_contract_state
            .get(&lifecycle_key)
            .expect("staged lifecycle exists")
            .clone();
        let verifiers_before = [
            transaction.world.verifying_keys.get(&eq_id).cloned(),
            transaction.world.verifying_keys.get(&ep_id).cloned(),
        ];
        let indexes_before = [
            transaction
                .world
                .verifying_keys_by_circuit
                .get(&eq_index)
                .cloned(),
            transaction
                .world
                .verifying_keys_by_circuit
                .get(&ep_index)
                .cloned(),
        ];
        let marker = kagemusha_v2_marker(KAGEMUSHA_V4_RELEASE_TRANSITION_DOMAIN, &[&transition_id]);

        let error = instruction
            .execute(&lifecycle.governance_authority, &mut transaction)
            .expect_err("unqualified cancellation verifier state must fail closed");
        assert!(
            error.to_string().contains(expected_error),
            "unexpected {corruption:?} error: {error}"
        );
        assert_eq!(
            transaction.world.smart_contract_state.get(&lifecycle_key),
            Some(&lifecycle_before),
            "{corruption:?} must not terminalize the lifecycle"
        );
        assert!(
            transaction
                .world
                .kagemusha_replay_keys
                .get(&marker)
                .is_none(),
            "{corruption:?} must not consume the transition id"
        );
        assert_eq!(
            [
                transaction.world.verifying_keys.get(&eq_id).cloned(),
                transaction.world.verifying_keys.get(&ep_id).cloned(),
            ],
            verifiers_before,
            "{corruption:?} must not partially tombstone either verifier"
        );
        assert_eq!(
            [
                transaction
                    .world
                    .verifying_keys_by_circuit
                    .get(&eq_index)
                    .cloned(),
                transaction
                    .world
                    .verifying_keys_by_circuit
                    .get(&ep_index)
                    .cloned(),
            ],
            indexes_before,
            "{corruption:?} must not change either verifier index"
        );
    }

    fn enabled_lifecycle_for_test(network_id: NetworkId) -> KagemushaV4ReleaseLifecycleStateV1 {
        let mut lifecycle = staged_lifecycle_for_test([0x55; 32], network_id);
        let staged = lifecycle
            .exact_bytes_digest()
            .expect("valid staged lifecycle identity");
        let mut validators = (0..KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT)
            .map(|index| {
                let seed = 0x90_u8
                    .checked_add(u8::try_from(index).expect("validator fixture index fits u8"))
                    .expect("validator fixture seed fits u8");
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("derive lifecycle validator");
                iroha_data_model::peer::PeerId::new(key.public_key().clone())
            })
            .collect::<Vec<_>>();
        validators.sort();
        let validator_ids: [_; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] = validators
            .try_into()
            .expect("exactly four lifecycle validators");
        lifecycle.phase =
            KagemushaV4ReleaseLifecyclePhaseV1::Enabled(Box::new(KagemushaV4ReleaseEnabledV1 {
                expected_staged_lifecycle: staged,
                transition_id: [0xA1; 32],
                enable_witness_norito: KagemushaExactBytesDigestV1::from_bytes(b"enable witness")
                    .expect("enable witness identity"),
                enable_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
                    b"enable transaction",
                )),
                enabled_at_height: 3,
                enabled_at_unix_ms: 1_800_000_000_001,
                validator_liveness_evidence: KagemushaExactBytesDigestV1::from_bytes(
                    b"validator liveness",
                )
                .expect("liveness identity"),
                canary_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
                    b"canary transaction",
                )),
                canary_finalized_height: 2,
                canary_finalized_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"canary block",
                )),
                endpoint_challenge: [0xA2; 32],
                validator_ids,
                observed_tip_heights: [2; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
                highest_observed_tip_height: 2,
            }));
        lifecycle
            .validate()
            .expect("valid enabled lifecycle test fixture");
        lifecycle
    }

    #[test]
    fn direct_ordinary_multisig_cancel_executes_exact_staged_transition() {
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"kagemusha-cancel-execution-network",
            )));
        let lifecycle = staged_lifecycle_for_test([0x55; 32], network_id);
        let predecessor = lifecycle
            .exact_bytes_digest()
            .expect("staged lifecycle identity");
        let transition_id = [0xB1; 32];
        let instruction =
            CancelKagemushaRecursiveReleaseV4::new(KagemushaV4ReleaseCancellationV1 {
                schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
                version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                promotion_id: lifecycle.promotion_binding.promotion_id,
                manifest_sha256: lifecycle.promotion_binding.manifest_sha256,
                expected_predecessor_lifecycle: predecessor,
                transition_id,
                reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
                evidence: None,
            });
        let signed = lifecycle_transaction(&lifecycle, instruction.clone().into());
        let state = lifecycle_state(&lifecycle);
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(3).expect("nonzero lifecycle height"),
            None,
            None,
            None,
            1_800_000_000_002,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        transaction.kagemusha_taira_canary_external_entrypoint = true;
        transaction.current_tx_hash = Some(signed.hash());
        transaction.kagemusha_release_lifecycle_entrypoint =
            signed_lifecycle_entrypoint_context(&signed)
                .expect("authenticate direct lifecycle transaction");
        for id in [
            &lifecycle.step_eq_verifier_key_id,
            &lifecycle.step_ep_verifier_key_id,
        ] {
            let verifier = transaction
                .world
                .verifying_keys
                .get(id)
                .expect("active release verifier fixture");
            assert_eq!(verifier.status, ConfidentialStatus::Active);
            assert!(verifier.activation_height.is_some_and(|height| height > 3));
            assert!(verifier.key.is_some());
            assert!(verifier.vk_len > 0);
        }

        instruction
            .execute(&lifecycle.governance_authority, &mut transaction)
            .expect("execute exact cancellation transition");

        let stored = load_lifecycle_by_manifest(
            &transaction.world,
            &lifecycle.artifact_binding.manifest_sha256,
        )
        .expect("decode transitioned lifecycle")
        .expect("transitioned lifecycle exists");
        let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancelled) = stored.state.phase else {
            panic!("cancellation must publish the terminal cancelled phase");
        };
        assert_eq!(cancelled.cancellation_transaction_intent, signed.hash());
        assert!(transaction.kagemusha_release_lifecycle_entrypoint.is_none());
        let marker = kagemusha_v2_marker(KAGEMUSHA_V4_RELEASE_TRANSITION_DOMAIN, &[&transition_id]);
        assert!(
            transaction
                .world
                .kagemusha_replay_keys
                .get(&marker)
                .is_some()
        );
        for id in [
            &lifecycle.step_eq_verifier_key_id,
            &lifecycle.step_ep_verifier_key_id,
        ] {
            let verifier = transaction
                .world
                .verifying_keys
                .get(id)
                .expect("cancelled release retains a withdrawn verifier tombstone");
            assert_eq!(verifier.status, ConfidentialStatus::Withdrawn);
            assert_eq!(verifier.activation_height, None);
            assert_eq!(verifier.withdraw_height, Some(3));
            assert!(verifier.key.is_none());
            assert_eq!(verifier.vk_len, 0);
            assert_eq!(
                transaction
                    .world
                    .verifying_keys_by_circuit
                    .get(&(verifier.circuit_id.clone(), lifecycle.verifier_version)),
                Some(id),
                "cancellation must retain the original release verifier index"
            );
        }
    }

    #[test]
    fn cancellation_rejects_missing_verifier_record_atomically() {
        assert_cancel_verifier_corruption_is_atomic(
            CancelVerifierCorruption::MissingRecord,
            "Eq verifier is absent",
        );
    }

    #[test]
    fn cancellation_rejects_substituted_lifecycle_verifier_id_atomically() {
        assert_cancel_verifier_corruption_is_atomic(
            CancelVerifierCorruption::SubstitutedLifecycleId,
            "verifier identities changed",
        );
    }

    #[test]
    fn cancellation_rejects_verifier_owner_mismatch_atomically() {
        assert_cancel_verifier_corruption_is_atomic(
            CancelVerifierCorruption::OwnerMismatch,
            "not the exact release-qualified registry identity",
        );
    }

    #[test]
    fn cancellation_rejects_ep_owner_mismatch_after_valid_eq_atomically() {
        assert_cancel_verifier_corruption_is_atomic(
            CancelVerifierCorruption::EpOwnerMismatch,
            "not the exact release-qualified registry identity",
        );
    }

    #[test]
    fn cancellation_rejects_verifier_version_mismatch_atomically() {
        assert_cancel_verifier_corruption_is_atomic(
            CancelVerifierCorruption::VersionMismatch,
            "Eq verifier record changed",
        );
    }

    #[test]
    fn cancellation_rejects_inactive_verifier_atomically() {
        assert_cancel_verifier_corruption_is_atomic(
            CancelVerifierCorruption::StatusMismatch,
            "Eq verifier record changed",
        );
    }

    #[test]
    fn cancellation_rejects_verifier_index_mismatch_atomically() {
        assert_cancel_verifier_corruption_is_atomic(
            CancelVerifierCorruption::IndexMismatch,
            "Eq verifier record changed",
        );
    }

    #[test]
    fn direct_ordinary_multisig_deactivate_executes_exact_enabled_transition() {
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"kagemusha-deactivate-execution-network",
            )));
        let lifecycle = enabled_lifecycle_for_test(network_id);
        let predecessor = lifecycle
            .exact_bytes_digest()
            .expect("enabled lifecycle identity");
        let transition_id = [0xB2; 32];
        let instruction =
            DeactivateKagemushaRecursiveIssuanceV4::new(KagemushaV4ReleaseDeactivationV1 {
                schema: KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1.to_owned(),
                version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                promotion_id: lifecycle.promotion_binding.promotion_id,
                manifest_sha256: lifecycle.promotion_binding.manifest_sha256,
                expected_predecessor_lifecycle: predecessor,
                transition_id,
                reason: KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation,
                evidence: None,
            });
        let signed = lifecycle_transaction(&lifecycle, instruction.clone().into());
        let state = lifecycle_state(&lifecycle);
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(4).expect("nonzero lifecycle height"),
            None,
            None,
            None,
            1_800_000_000_003,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        transaction.kagemusha_taira_canary_external_entrypoint = true;
        transaction.current_tx_hash = Some(signed.hash());
        transaction.kagemusha_release_lifecycle_entrypoint =
            signed_lifecycle_entrypoint_context(&signed)
                .expect("authenticate direct lifecycle transaction");

        instruction
            .execute(&lifecycle.governance_authority, &mut transaction)
            .expect("execute exact deactivation transition");

        let stored = load_lifecycle_by_manifest(
            &transaction.world,
            &lifecycle.artifact_binding.manifest_sha256,
        )
        .expect("decode transitioned lifecycle")
        .expect("transitioned lifecycle exists");
        let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) = stored.state.phase
        else {
            panic!("deactivation must publish the terminal deactivated phase");
        };
        assert_eq!(deactivated.deactivation_transaction_intent, signed.hash());
        assert!(transaction.kagemusha_release_lifecycle_entrypoint.is_none());
        let marker = kagemusha_v2_marker(KAGEMUSHA_V4_RELEASE_TRANSITION_DOMAIN, &[&transition_id]);
        assert!(
            transaction
                .world
                .kagemusha_replay_keys
                .get(&marker)
                .is_some()
        );
        for id in [
            &lifecycle.step_eq_verifier_key_id,
            &lifecycle.step_ep_verifier_key_id,
        ] {
            let verifier = transaction
                .world
                .verifying_keys
                .get(id)
                .expect("deactivated release retains its verifier");
            assert_eq!(verifier.status, ConfidentialStatus::Active);
            assert_eq!(verifier.withdraw_height, None);
            assert!(verifier.key.is_some());
            assert!(verifier.vk_len > 0);
            assert_eq!(
                transaction
                    .world
                    .verifying_keys_by_circuit
                    .get(&(verifier.circuit_id.clone(), lifecycle.verifier_version,)),
                Some(id)
            );
        }
    }

    #[test]
    fn terminal_lifecycle_rejects_repeated_and_cross_terminal_transitions() {
        let (_, lifecycle, _) = terminal_lifecycle_with_retained_policy();
        let lifecycle_key = lifecycle_key(&lifecycle.artifact_binding.manifest_sha256)
            .expect("canonical lifecycle key");
        let predecessor = lifecycle
            .exact_bytes_digest()
            .expect("cancelled lifecycle identity");

        let cancellation_transition_id = [0xC1; 32];
        let cancellation =
            CancelKagemushaRecursiveReleaseV4::new(KagemushaV4ReleaseCancellationV1 {
                schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
                version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                promotion_id: lifecycle.promotion_binding.promotion_id,
                manifest_sha256: lifecycle.promotion_binding.manifest_sha256,
                expected_predecessor_lifecycle: predecessor,
                transition_id: cancellation_transition_id,
                reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
                evidence: None,
            });
        let signed = lifecycle_transaction(&lifecycle, cancellation.clone().into());
        let state = lifecycle_state(&lifecycle);
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(4).expect("nonzero lifecycle height"),
            None,
            None,
            None,
            1_800_000_000_002,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        transaction.kagemusha_taira_canary_external_entrypoint = true;
        transaction.current_tx_hash = Some(signed.hash());
        transaction.kagemusha_release_lifecycle_entrypoint =
            signed_lifecycle_entrypoint_context(&signed)
                .expect("authenticate repeated terminal transition");
        let lifecycle_before = transaction
            .world
            .smart_contract_state
            .get(&lifecycle_key)
            .expect("terminal lifecycle exists")
            .clone();
        let error = cancellation
            .execute(&lifecycle.governance_authority, &mut transaction)
            .expect_err("cancelled lifecycle cannot be cancelled again");
        assert!(error.to_string().contains("exact staged state"));
        assert_eq!(
            transaction.world.smart_contract_state.get(&lifecycle_key),
            Some(&lifecycle_before)
        );
        let cancellation_marker = kagemusha_v2_marker(
            KAGEMUSHA_V4_RELEASE_TRANSITION_DOMAIN,
            &[&cancellation_transition_id],
        );
        assert!(
            transaction
                .world
                .kagemusha_replay_keys
                .get(&cancellation_marker)
                .is_none()
        );

        let deactivation_transition_id = [0xC2; 32];
        let deactivation =
            DeactivateKagemushaRecursiveIssuanceV4::new(KagemushaV4ReleaseDeactivationV1 {
                schema: KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1.to_owned(),
                version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                promotion_id: lifecycle.promotion_binding.promotion_id,
                manifest_sha256: lifecycle.promotion_binding.manifest_sha256,
                expected_predecessor_lifecycle: predecessor,
                transition_id: deactivation_transition_id,
                reason: KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation,
                evidence: None,
            });
        let signed = lifecycle_transaction(&lifecycle, deactivation.clone().into());
        transaction.current_tx_hash = Some(signed.hash());
        transaction.kagemusha_release_lifecycle_entrypoint =
            signed_lifecycle_entrypoint_context(&signed)
                .expect("authenticate cross-terminal transition");
        let error = deactivation
            .execute(&lifecycle.governance_authority, &mut transaction)
            .expect_err("cancelled lifecycle cannot be deactivated");
        assert!(error.to_string().contains("only enabled"));
        assert_eq!(
            transaction.world.smart_contract_state.get(&lifecycle_key),
            Some(&lifecycle_before)
        );
        let deactivation_marker = kagemusha_v2_marker(
            KAGEMUSHA_V4_RELEASE_TRANSITION_DOMAIN,
            &[&deactivation_transition_id],
        );
        assert!(
            transaction
                .world
                .kagemusha_replay_keys
                .get(&deactivation_marker)
                .is_none()
        );
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
            redemption_policy(&world.view(), &binding).expect("terminal redemption policy"),
            expected_policy,
        );
    }

    #[test]
    fn runtime_projection_gate_is_active_only_for_staged_or_enabled_state() {
        let (binding, mut lifecycle, _) = terminal_lifecycle_with_retained_policy();
        let key = lifecycle_key(&binding.manifest_sha256).expect("lifecycle state key");
        let mut world = World::default();
        world.smart_contract_state.insert(
            key.clone(),
            norito::encode_canonical(&lifecycle).expect("canonical terminal lifecycle"),
        );
        require_local_runtime_effective_config(&world.view(), None)
            .expect("terminal lifecycle does not freeze runtime projection");

        lifecycle.phase = KagemushaV4ReleaseLifecyclePhaseV1::Staged;
        lifecycle.validate().expect("valid staged lifecycle");
        world.smart_contract_state.insert(
            key,
            norito::encode_canonical(&lifecycle).expect("canonical staged lifecycle"),
        );
        require_local_runtime_effective_config(
            &world.view(),
            Some(lifecycle.runtime_effective_config_sha256),
        )
        .expect("exact local projection is accepted");
        assert!(require_local_runtime_effective_config(&world.view(), None).is_err());
        assert!(require_local_runtime_effective_config(&world.view(), Some([0x56; 32])).is_err());
        assert!(runtime_consensus_parameters_frozen(&world.view()).expect("valid lifecycle"));
    }

    #[test]
    fn authenticated_catalog_closes_consensus_parameter_drift_before_stage() {
        use iroha_data_model::parameter::{
            Parameter, system::SumeragiNposParameters, system::SumeragiParameter,
        };

        let world = World::default();
        let clock_drift = Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333));
        let npos = Parameter::Custom(SumeragiNposParameters::default().into_custom_parameter());
        assert!(
            validate_runtime_consensus_parameter_update(&clock_drift, &world.view(), false).is_ok()
        );
        for parameter in [&clock_drift, &npos] {
            assert!(
                validate_runtime_consensus_parameter_update(parameter, &world.view(), true)
                    .is_err(),
                "an authenticated catalog must lock {parameter:?} before Stage"
            );
        }
    }

    #[test]
    fn lifecycle_state_rejects_a_policy_with_one_threshold_weight_member() {
        let (_, mut lifecycle, _) = terminal_lifecycle_with_retained_policy();
        let keys = [0x81_u8, 0x82].map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive weighted lifecycle policy key")
        });
        lifecycle.phase = KagemushaV4ReleaseLifecyclePhaseV1::Staged;
        lifecycle.governance_authority = AccountId::new_multisig(
            MultisigPolicy::new(
                2,
                vec![
                    MultisigMember::new(keys[0].public_key().clone(), 2)
                        .expect("valid weight-two lifecycle member"),
                    MultisigMember::new(keys[1].public_key().clone(), 1)
                        .expect("valid weight-one lifecycle member"),
                ],
            )
            .expect("structurally valid weighted lifecycle policy"),
        );
        assert!(
            lifecycle.validate().is_err(),
            "a single member must never satisfy Kagemusha's distinct-governor floor"
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
    fn stage_lifecycle_expiry_is_mandatory_and_receipt_bounded() {
        let missing = signed([cancellation(0x42)]);
        let error = lifecycle_expires_at_height(&missing, LifecycleEntrypointKind::Stage)
            .expect_err("Stage must carry the receipt's exclusive height expiry");
        assert!(error.to_string().contains("nonzero expires_at_height"));
        let zero = signed_with_height_expiry(0);
        assert!(
            lifecycle_expires_at_height(&zero, LifecycleEntrypointKind::Stage).is_err(),
            "zero is not an exclusive Stage height expiry"
        );

        let carrier_height = 10;
        let maximum_expiry = carrier_height
            + u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                .expect("proof-count bound fits u64");
        let bounded = signed_with_height_expiry(maximum_expiry);
        assert_eq!(
            lifecycle_expires_at_height(&bounded, LifecycleEntrypointKind::Stage)
                .expect("bounded Stage expiry metadata"),
            Some(maximum_expiry)
        );
        // Height TTL is generic transaction metadata and remains legal on a
        // non-Stage lifecycle transaction (including networks that require it).
        signed_lifecycle_entrypoint_context(&bounded)
            .expect("non-Stage lifecycle height TTL remains valid")
            .expect("exact direct lifecycle context");

        let mut context = LifecycleEntrypointContext {
            kind: LifecycleEntrypointKind::Stage,
            transaction_intent: bounded.hash(),
            instruction_digest: Hash::new(b"stage-expiry-horizon"),
            stage_expires_at_height: Some(maximum_expiry),
        };
        context
            .validate_stage_expiry_horizon(carrier_height)
            .expect("latest receipt-capable expiry must remain admissible");
        context.stage_expires_at_height = Some(maximum_expiry + 1);
        let error = context
            .validate_stage_expiry_horizon(carrier_height)
            .expect_err("expiry beyond every possible trusted anchor must fail");
        assert!(error.to_string().contains("receipt-capable maximum"));
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
        assert!(
            signed_lifecycle_entrypoint_context(&queue_plan).is_err(),
            "an exact lifecycle instruction must fail closed under a nonordinary carrier"
        );
    }

    #[test]
    fn every_lifecycle_kind_rejects_one_threshold_weight_signer() {
        let one_signer = weighted_signed(cancellation(0x4a), false);
        one_signer
            .verify_signature()
            .expect("generic weighted multisig verification accepts the weight-two signer");
        let two_signers = weighted_signed(cancellation(0x4b), true);

        for kind in [
            LifecycleEntrypointKind::Stage,
            LifecycleEntrypointKind::Enable,
            LifecycleEntrypointKind::Cancel,
            LifecycleEntrypointKind::Deactivate,
        ] {
            assert!(
                require_distinct_governance_signers(&one_signer, kind).is_err(),
                "{kind:?} must reject a one-signer weighted quorum"
            );
            require_distinct_governance_signers(&two_signers, kind)
                .unwrap_or_else(|error| panic!("{kind:?} must accept two valid signers: {error}"));
        }
        assert!(
            signed_lifecycle_entrypoint_context(&one_signer).is_err(),
            "the direct carrier must apply the distinct-signer gate"
        );
    }

    #[test]
    fn sealed_reveal_cannot_gain_direct_external_lifecycle_provenance() {
        let instruction = cancellation(0x4c);
        instruction.validate().expect("valid cancellation");
        let signed = signed([instruction.clone()]);
        assert_eq!(
            signed.admission_intent(),
            TransactionAdmissionIntent::Ordinary
        );
        assert_eq!(
            signed
                .multisig_signatures()
                .expect("multisig signature bundle")
                .signatures
                .len(),
            2
        );
        assert!(
            signed_lifecycle_entrypoint_context(&signed)
                .expect("classify direct lifecycle carrier")
                .is_some(),
            "the signed inner transaction must independently qualify for direct lifecycle use"
        );

        let lifecycle = staged_lifecycle_for_test(
            [0x55; 32],
            *signed.network_id().expect("ordinary transaction network"),
        );
        let state = lifecycle_state(&lifecycle);
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(3).expect("nonzero lifecycle height"),
            None,
            None,
            None,
            1_800_000_000_002,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let replay_keys_before = transaction.world.kagemusha_replay_keys.iter().count();
        let signed_hash = signed.hash();
        let authority = signed.authority().clone();
        let reveal = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
            Hash::new(b"sealed lifecycle provenance"),
            signed,
            [0xDA; 32],
        ));

        crate::state::seed_committed_transaction_context(&mut transaction, &reveal, 0);

        assert!(!transaction.kagemusha_taira_canary_external_entrypoint);
        assert!(transaction.kagemusha_release_lifecycle_entrypoint.is_none());
        assert_eq!(transaction.current_tx_hash, Some(signed_hash));
        let error = instruction
            .execute(&authority, &mut transaction)
            .expect_err("sealed reveal must not execute a direct lifecycle mutation");
        assert!(
            error
                .to_string()
                .contains("requires one exact direct External instruction"),
            "unexpected sealed lifecycle error: {error}"
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            replay_keys_before,
            "rejected sealed lifecycle execution must not consume a replay marker"
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
