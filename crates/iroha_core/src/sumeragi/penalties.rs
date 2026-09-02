//! Deterministic `NPoS` consensus-evidence slashing.
#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    smartcontracts::isi::staking::{
        PublicLaneStakeIndex, PublicLaneStakeShareKey, apply_indexed_consensus_slash_to_validator,
        apply_indexed_slash_to_validator_without_observability,
        indexed_slashable_validator_exposure, max_slash_amount, validator_tenure_contains_height,
    },
    state::{
        State, StateBlock, StateTransaction, StateView, WorldReadOnly,
        public_lane_validator_record_matches_key,
    },
};
use eyre::{Result, WrapErr, eyre};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{Evidence, EvidencePenaltyStatus, EvidenceRecord, ValidatorIndex},
        consensus_v2::HeightContext,
    },
    consensus::{
        NposConsensusEffects, NposConsensusSlashAction, NposMarkConsensusEvidenceAppliedAction,
        NposPenaltyAction,
    },
    nexus::LaneId,
    prelude::{AccountId, PeerId},
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, Default)]
pub struct PenaltyOutcome {
    pub applied: u64,
    pub slashed: u64,
}
#[derive(Clone, Copy)]
enum EffectsApplicationMode {
    Commit,
    ValidateOnly,
}
#[derive(Clone)]
struct ValidatorLocator {
    lane_id: LaneId,
    validator: AccountId,
    slashable_exposure: Quantity,
    activation_height: u64,
    deactivation_height: Option<u64>,
    share_keys: Vec<PublicLaneStakeShareKey>,
}
struct ParentPenaltySnapshot {
    evidence: super::evidence::V2CommittedEvidenceSnapshot,
    slashing_delay: u64,
    max_slash_bps: u16,
    validator_map: BTreeMap<PublicKey, Vec<ValidatorLocator>>,
    #[cfg(test)]
    stake_share_rows_scanned: usize,
}
fn consensus_penalty_is_due(
    recorded_at_height: u64,
    slashing_delay: u64,
    current_height: u64,
) -> bool {
    recorded_at_height
        .checked_add(slashing_delay)
        .is_some_and(|eligible_height| eligible_height <= current_height)
}
pub struct PenaltyApplier<'a> {
    state: &'a State,
}
impl<'a> PenaltyApplier<'a> {
    pub(crate) fn new(
        state: &'a State,
        #[cfg(feature = "telemetry")] _telemetry: Option<&'a StateTelemetry>,
        #[cfg(not(feature = "telemetry"))] _telemetry: Option<()>,
    ) -> Self {
        Self { state }
    }
    fn parent_snapshot(
        view: &StateView<'_>,
        evidence: super::evidence::V2CommittedEvidenceSnapshot,
    ) -> Result<ParentPenaltySnapshot> {
        if evidence.record_capacity_exceeded {
            return Err(eyre!(
                "committed Sumeragi v2 evidence exceeds the record capacity"
            ));
        }
        if evidence.byte_capacity_exceeded {
            return Err(eyre!(
                "committed Sumeragi v2 evidence exceeds the proof-byte capacity"
            ));
        }
        let world = view.world();
        let slashing_delay = crate::sumeragi::resolve_npos_slashing_delay_blocks_from_world(world)
            .ok_or_else(|| eyre!("NPoS penalty derivation requires signed NPoS parameters"))?;
        let exposure_index = PublicLaneStakeIndex::from_world(
            world,
            view.nexus.staking.max_stake_shares_per_validator.get(),
            view.nexus.staking.max_pending_unbonds_per_share.get(),
        )
        .wrap_err("failed to index slashable public-lane stake exposure")?;
        let mut candidates_map: BTreeMap<PublicKey, Vec<ValidatorLocator>> = BTreeMap::new();
        let mut validator_counts = BTreeMap::<LaneId, u32>::new();
        for (key, record) in world.public_lane_validators().iter() {
            if !public_lane_validator_record_matches_key(key, record) {
                continue;
            }
            let (lane_id, validator_id) = key;
            let validator_count = validator_counts.entry(*lane_id).or_default();
            *validator_count = validator_count
                .checked_add(1)
                .ok_or_else(|| eyre!("public-lane validator count overflows u32"))?;
            if *validator_count > view.nexus.staking.max_validators.get() {
                return Err(eyre!(
                    "public lane {lane_id} exceeds retained validator capacity"
                ));
            }
            if !view.is_lane_active_for_authority(*lane_id)
                || view.staking_authority_lane(*lane_id) != Some(*lane_id)
            {
                continue;
            }
            let activation_height = record.activation_height;
            validator_tenure_contains_height(record, activation_height)
                .wrap_err("retained public-lane validator tenure is non-canonical")?;
            candidates_map
                .entry(record.peer_id.public_key().clone())
                .or_default()
                .push(ValidatorLocator {
                    lane_id: *lane_id,
                    validator: validator_id.clone(),
                    slashable_exposure: exposure_index
                        .total_exposure(*lane_id, validator_id)
                        .map_err(|error| {
                            eyre!("invalid slashable stake exposure for {validator_id}: {error}")
                        })?,
                    activation_height,
                    deactivation_height: record.deactivation_height,
                    share_keys: exposure_index.share_keys(*lane_id, validator_id).to_vec(),
                });
        }
        for locators in candidates_map.values_mut() {
            locators.sort_by(|lhs, rhs| {
                lhs.lane_id
                    .cmp(&rhs.lane_id)
                    .then_with(|| lhs.validator.cmp(&rhs.validator))
            });
        }
        Ok(ParentPenaltySnapshot {
            evidence,
            slashing_delay,
            max_slash_bps: view.nexus.staking.max_slash_bps,
            validator_map: candidates_map,
            #[cfg(test)]
            stake_share_rows_scanned: exposure_index.rows_scanned(),
        })
    }
    pub(crate) fn derive_npos_consensus_effects(
        &self,
        block_header: &BlockHeader,
    ) -> Result<NposConsensusEffects> {
        let (v2_evidence_admissions, penalty_actions) =
            self.derive_from_stable_parent(block_header, true)?;
        Ok(NposConsensusEffects {
            finalized_global_beacon_pulse: None,
            v2_evidence_admissions,
            penalty_actions,
        })
    }
    /// Derive only deterministic penalty actions from pre-block state.
    pub(crate) fn derive_npos_penalty_actions(
        &self,
        block_header: &BlockHeader,
    ) -> Result<Vec<NposPenaltyAction>> {
        self.derive_from_stable_parent(block_header, false)
            .map(|(_, actions)| actions)
    }
    fn derive_from_stable_parent(
        &self,
        block_header: &BlockHeader,
        include_admissions: bool,
    ) -> Result<(
        Vec<iroha_data_model::block::consensus::SumeragiV2EquivocationEvidence>,
        Vec<NposPenaltyAction>,
    )> {
        loop {
            let generation_before = self.state.state_view_generation();
            if generation_before % 2 != 0 {
                std::thread::yield_now();
                continue;
            }
            let view = self.state.view();
            let evidence = super::evidence::v2_committed_evidence_snapshot(view.world());
            let result = Self::parent_snapshot(&view, evidence).and_then(|snapshot| {
                let admissions = if include_admissions {
                    super::evidence::pending_v2_evidence_admissions_from_snapshot(
                        self.state,
                        block_header.height().get(),
                        &snapshot.evidence,
                    )
                } else {
                    Vec::new()
                };
                drop(view);
                self.derive_consensus_penalty_actions(block_header, snapshot)
                    .map(|actions| (admissions, actions))
            });
            let generation_after = self.state.state_view_generation();
            if generation_before == generation_after && generation_after % 2 == 0 {
                return result;
            }
            std::thread::yield_now();
        }
    }
    #[allow(clippy::too_many_lines)]
    fn derive_consensus_penalty_actions(
        &self,
        block_header: &BlockHeader,
        snapshot: ParentPenaltySnapshot,
    ) -> Result<Vec<NposPenaltyAction>> {
        let current_height = block_header.height().get();
        let mut pending: Vec<(Hash, EvidenceRecord)> = Vec::new();
        for (key, record) in snapshot.evidence.records {
            if record.penalty_status.is_terminal() {
                continue;
            }
            if record.recorded_at_height >= current_height {
                // Evidence admitted by the block currently under construction
                // can never drive its own deterministic penalty attachment.
                continue;
            }
            if !consensus_penalty_is_due(
                record.recorded_at_height,
                snapshot.slashing_delay,
                current_height,
            ) {
                continue;
            }
            pending.push((key, record));
        }
        if pending.is_empty() {
            return Ok(Vec::new());
        }
        pending.sort_by(|left, right| left.0.cmp(&right.0));
        let _witness_suppression =
            crate::sumeragi::witness::suppress_recording_for_current_thread();
        let mut scratch = self
            .state
            .consensus_effects_probe_block(block_header.clone());
        let mut actions = Vec::new();
        for (key, record) in pending {
            // Admission already validated and anchored this immutable context.
            // Re-reading mutable local Kura files here would make block
            // construction depend on node-local I/O after consensus admission.
            let context = &record.evidence.equivocation.context;
            let roster = context
                .roster
                .iter()
                .map(|validator| validator.validator.clone())
                .collect::<Vec<_>>();
            let offenders = offender_indices(&record.evidence, record.recorded_at_height, context);
            let slash_id = key;
            for signer in offenders {
                let Some((peer_id, locators)) = self.locate_validator_in_roster_cached(
                    signer,
                    &roster,
                    &snapshot.validator_map,
                ) else {
                    continue;
                };
                for locator in locators {
                    if context.height < locator.activation_height
                        || locator
                            .deactivation_height
                            .is_some_and(|height| context.height >= height)
                    {
                        continue;
                    }
                    let validator_key = (locator.lane_id, locator.validator.clone());
                    let current_record = scratch
                        .world
                        .public_lane_validators
                        .get(&validator_key)
                        .ok_or_else(|| {
                        eyre!(
                            "penalty planning lost retained validator {} on lane {}",
                            locator.validator,
                            locator.lane_id
                        )
                    })?;
                    let current_exposure = indexed_slashable_validator_exposure(
                        &scratch.world,
                        locator.lane_id,
                        &locator.validator,
                        current_record,
                        context.height,
                        &locator.share_keys,
                    )
                    .wrap_err_with(|| {
                        format!(
                            "failed to recompute slashable exposure for {} on lane {}",
                            locator.validator, locator.lane_id
                        )
                    })?;
                    if current_exposure > locator.slashable_exposure {
                        return Err(eyre!(
                            "slashable exposure increased while planning one penalty bundle"
                        ));
                    }
                    let amount = max_slash_amount(&current_exposure, snapshot.max_slash_bps)?;
                    if amount.is_zero() {
                        continue;
                    }
                    let slash = NposConsensusSlashAction {
                        evidence_key: key,
                        signer,
                        peer_id: peer_id.clone(),
                        lane_id: locator.lane_id,
                        validator: locator.validator.clone(),
                        slash_id,
                        amount,
                    };
                    let mut transaction = scratch.consensus_effects_transaction();
                    apply_indexed_slash_to_validator_without_observability(
                        &mut transaction,
                        slash.lane_id,
                        &slash.validator,
                        slash.slash_id,
                        &slash.amount,
                        block_header.creation_time_ms,
                        context.height,
                        &locator.share_keys,
                    )
                    .wrap_err_with(|| {
                        format!(
                            "failed to plan consensus slash for {} on lane {}",
                            slash.validator, slash.lane_id
                        )
                    })?;
                    transaction.apply_consensus_effects();
                    actions.push(NposPenaltyAction::ConsensusSlash(slash));
                }
            }
            // A removed, inactive, or zero-stake offender is still terminal:
            // retaining an unslashable record forever would exhaust the
            // bounded committed evidence table and suppress future proofs.
            actions.push(NposPenaltyAction::MarkConsensusEvidenceApplied(
                NposMarkConsensusEvidenceAppliedAction {
                    evidence_key: key,
                    height: current_height,
                },
            ));
        }
        actions.sort();
        actions.dedup();
        Ok(actions)
    }
    #[allow(clippy::unused_self)]
    fn locate_validator_in_roster_cached(
        &self,
        signer: ValidatorIndex,
        roster: &[PeerId],
        map: &BTreeMap<PublicKey, Vec<ValidatorLocator>>,
    ) -> Option<(PeerId, Vec<ValidatorLocator>)> {
        let signer_idx = usize::try_from(signer).ok()?;
        let peer = roster.get(signer_idx)?;
        map.get(peer.public_key())
            .cloned()
            .map(|locator| (peer.clone(), locator))
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_npos_consensus_effects_to_transaction(
    tx: &mut StateTransaction<'_, '_>,
    effects: &NposConsensusEffects,
    evidence_prune_keys: &[Hash],
    expected_beacon_anchor: Option<iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1>,
    authenticated_roster: &[PeerId],
    current_height: u64,
    current_view: u64,
    now_ms: u64,
) -> Result<PenaltyOutcome> {
    apply_npos_consensus_effects_to_transaction_inner(
        tx,
        effects,
        evidence_prune_keys,
        expected_beacon_anchor,
        authenticated_roster,
        current_height,
        current_view,
        now_ms,
        EffectsApplicationMode::Commit,
    )
}
/// Validate post-execution consensus effects in a rollback-only transaction.
///
/// The caller must pass the exact prune plan derived from immutable parent
/// state. Operational slash counters and telemetry are suppressed because the
/// transaction is deliberately discarded. Consensus effects never contribute
/// to the transaction execution witness in either application mode.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_npos_consensus_effects_after_execution(
    state_block: &mut StateBlock<'_>,
    effects: &NposConsensusEffects,
    evidence_prune_keys: &[Hash],
    expected_beacon_anchor: Option<iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1>,
    authenticated_roster: &[PeerId],
    current_height: u64,
    current_view: u64,
    now_ms: u64,
) -> Result<()> {
    let mut tx = state_block.consensus_effects_transaction();
    apply_npos_consensus_effects_to_transaction_inner(
        &mut tx,
        effects,
        evidence_prune_keys,
        expected_beacon_anchor,
        authenticated_roster,
        current_height,
        current_view,
        now_ms,
        EffectsApplicationMode::ValidateOnly,
    )?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn apply_npos_consensus_effects_to_transaction_inner(
    tx: &mut StateTransaction<'_, '_>,
    effects: &NposConsensusEffects,
    evidence_prune_keys: &[Hash],
    expected_beacon_anchor: Option<iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1>,
    authenticated_roster: &[PeerId],
    current_height: u64,
    current_view: u64,
    now_ms: u64,
    mode: EffectsApplicationMode,
) -> Result<PenaltyOutcome> {
    // These are finality effects, not transaction execution. Suppress the
    // process-global recorder in both commit and rollback-only validation so
    // concurrent in-process State instances cannot contaminate one another.
    let _witness_suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let mut outcome = PenaltyOutcome::default();
    if let Some(pulse) = effects.finalized_global_beacon_pulse {
        if pulse.network_id != tx.network_id
            || pulse.height != current_height
            || pulse.round != crate::beacon::GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1
        {
            return Err(eyre!(
                "finalized global beacon pulse differs from the applying block"
            ));
        }
        let expected_anchor = expected_beacon_anchor
            .ok_or_else(|| eyre!("finalized global beacon pulse has no parent anchor"))?;
        if expected_anchor.height.checked_add(1) != Some(current_height) {
            return Err(eyre!(
                "finalized global beacon pulse parent anchor has the wrong height"
            ));
        }
        let key_record = tx
            .world
            .global_beacon_key_sessions
            .get(&pulse.session_id)
            .cloned()
            .ok_or_else(|| eyre!("global beacon pulse key session is absent"))?;
        if !key_record.is_active_at(current_height) {
            return Err(eyre!(
                "global beacon pulse key session is not active at the pulse height"
            ));
        }
        let expected_roster_hash =
            crate::beacon::authenticated_global_threshold_beacon_roster_hash_v1(
                &key_record.session,
                authenticated_roster,
            )
            .wrap_err(
                "pulse-height global beacon key differs from the authenticated height roster",
            )?;
        let binding = crate::beacon::GlobalThresholdBeaconSessionBindingV1 {
            network_id: tx.network_id,
            session_id: pulse.session_id,
            roster_hash: expected_roster_hash,
            transcript_hash: key_record.session.transcript_hash,
        };
        let session = crate::beacon::validate_global_threshold_beacon_session_v1(
            key_record.session,
            &binding,
        )
        .wrap_err("pulse-height global beacon public DKG session failed validation")?;
        tx.world
            .verify_and_advance_global_beacon_pulse(&session, pulse, expected_anchor)
            .wrap_err("failed to persist finalized global beacon pulse")?;
    }
    if !evidence_prune_keys.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(eyre!(
            "Sumeragi v2 parent evidence prune plan is not canonical"
        ));
    }
    for key in evidence_prune_keys {
        let record = tx
            .world
            .consensus_evidence
            .get(key)
            .ok_or_else(|| eyre!("Sumeragi v2 parent evidence prune target is absent"))?;
        if !record.penalty_status.is_terminal() {
            return Err(eyre!(
                "Sumeragi v2 parent evidence prune target is not terminal"
            ));
        }
        if !super::evidence::v2_committed_evidence_record_is_prunable(
            &tx.world,
            record,
            current_height,
        ) {
            return Err(eyre!(
                "Sumeragi v2 parent evidence prune target is not stale under the post-execution evidence horizon"
            ));
        }
    }
    for key in evidence_prune_keys {
        tx.world.consensus_evidence.remove(*key);
    }
    if tx
        .world
        .consensus_evidence
        .iter()
        .count()
        .saturating_add(effects.v2_evidence_admissions.len())
        > super::evidence::MAX_V2_COMMITTED_EVIDENCE_RECORDS
    {
        return Err(eyre!(
            "bounded Sumeragi v2 evidence table has no reclaimable capacity"
        ));
    }
    let mut retained_evidence_bytes = 0_usize;
    for (_, record) in tx.world.consensus_evidence.iter() {
        let encoded_len = super::evidence::v2_evidence_encoded_len(&record.evidence.equivocation);
        if encoded_len > super::evidence::MAX_V2_EVIDENCE_ADMISSION_BYTES {
            return Err(eyre!(
                "committed Sumeragi v2 evidence contains an oversized individual proof"
            ));
        }
        retained_evidence_bytes = super::evidence::checked_v2_evidence_byte_sum(
            retained_evidence_bytes,
            [encoded_len],
            super::evidence::MAX_V2_COMMITTED_EVIDENCE_BYTES,
        )
        .ok_or_else(|| {
            eyre!("bounded Sumeragi v2 evidence table exceeds its proof-byte capacity")
        })?;
    }
    let incoming_evidence_bytes = super::evidence::checked_v2_evidence_byte_sum(
        0,
        effects
            .v2_evidence_admissions
            .iter()
            .map(super::evidence::v2_evidence_encoded_len),
        super::evidence::MAX_V2_EVIDENCE_ADMISSION_BYTES,
    )
    .ok_or_else(|| eyre!("Sumeragi v2 evidence admission batch exceeds its byte capacity"))?;
    if super::evidence::checked_v2_evidence_byte_sum(
        retained_evidence_bytes,
        [incoming_evidence_bytes],
        super::evidence::MAX_V2_COMMITTED_EVIDENCE_BYTES,
    )
    .is_none()
    {
        return Err(eyre!(
            "bounded Sumeragi v2 evidence table has no reclaimable proof-byte capacity"
        ));
    }
    for admission in &effects.v2_evidence_admissions {
        let evidence = super::evidence::canonical_v2_evidence(admission);
        let key = super::evidence::v2_evidence_admission_key(admission);
        if tx.world.consensus_evidence.get(&key).is_some() {
            return Err(eyre::eyre!(
                "Sumeragi v2 evidence was already admitted by a committed block"
            ));
        }
        tx.world.consensus_evidence.insert(
            key,
            EvidenceRecord {
                evidence,
                recorded_at_height: current_height,
                recorded_at_view: current_view,
                recorded_at_ms: now_ms,
                penalty_status: EvidencePenaltyStatus::Pending,
            },
        );
    }
    let stake_index = effects
        .penalty_actions
        .iter()
        .any(|action| matches!(action, NposPenaltyAction::ConsensusSlash(_)))
        .then(|| {
            PublicLaneStakeIndex::from_world(
                &tx.world,
                tx.nexus.staking.max_stake_shares_per_validator.get(),
                tx.nexus.staking.max_pending_unbonds_per_share.get(),
            )
        })
        .transpose()
        .wrap_err("failed to index the exact consensus-effects staking overlay")?;
    for action in &effects.penalty_actions {
        match action {
            NposPenaltyAction::ConsensusSlash(action) => {
                ensure_evidence_penalty_is_unresolved(tx, &action.evidence_key)?;
                let offence_height = tx
                    .world
                    .consensus_evidence
                    .get(&action.evidence_key)
                    .expect("validated unresolved evidence exists")
                    .evidence
                    .equivocation
                    .context
                    .height;
                if !tx.is_lane_active_for_authority(action.lane_id) {
                    return Err(eyre!(
                        "consensus slash targets a lane made inactive by block execution"
                    ));
                }
                let share_keys = stake_index
                    .as_ref()
                    .expect("a slash action constructs the staking index")
                    .share_keys(action.lane_id, &action.validator);
                match mode {
                    EffectsApplicationMode::Commit => apply_indexed_consensus_slash_to_validator(
                        tx,
                        action.lane_id,
                        &action.validator,
                        action.slash_id,
                        &action.amount,
                        now_ms,
                        offence_height,
                        share_keys,
                    )?,
                    EffectsApplicationMode::ValidateOnly => {
                        apply_indexed_slash_to_validator_without_observability(
                            tx,
                            action.lane_id,
                            &action.validator,
                            action.slash_id,
                            &action.amount,
                            now_ms,
                            offence_height,
                            share_keys,
                        )?;
                    }
                }
                outcome.applied = outcome.applied.saturating_add(1);
                outcome.slashed = outcome.slashed.saturating_add(1);
            }
            NposPenaltyAction::MarkConsensusEvidenceApplied(action) => {
                if action.height != current_height {
                    return Err(eyre!(
                        "consensus evidence-applied marker has the wrong block height"
                    ));
                }
                ensure_evidence_penalty_is_unresolved(tx, &action.evidence_key)?;
                let mut record = tx
                    .world
                    .consensus_evidence
                    .get(&action.evidence_key)
                    .cloned()
                    .expect("validated unresolved evidence exists");
                record.penalty_status = EvidencePenaltyStatus::Applied {
                    height: action.height,
                };
                tx.world
                    .consensus_evidence
                    .insert(action.evidence_key, record);
            }
        }
    }
    Ok(outcome)
}
fn ensure_evidence_penalty_is_unresolved(
    tx: &StateTransaction<'_, '_>,
    evidence_key: &Hash,
) -> Result<()> {
    let record = tx
        .world
        .consensus_evidence
        .get(evidence_key)
        .ok_or_else(|| eyre!("consensus penalty action references missing evidence"))?;
    match record.penalty_status {
        EvidencePenaltyStatus::Pending => {}
        EvidencePenaltyStatus::Applied { .. } => {
            return Err(eyre!(
                "consensus penalty action references already applied evidence"
            ));
        }
        EvidencePenaltyStatus::Cancelled { .. } => {
            return Err(eyre!(
                "consensus penalty action references cancelled evidence"
            ));
        }
    }
    Ok(())
}
fn canonical_indices(
    indices: impl IntoIterator<Item = ValidatorIndex>,
    roster_len: usize,
) -> Vec<ValidatorIndex> {
    indices
        .into_iter()
        .filter(|index| usize::try_from(*index).is_ok_and(|index| index < roster_len))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn evidence_context_height(evidence: &Evidence, _recorded_at_height: u64) -> u64 {
    evidence.equivocation.context.height
}
fn offender_indices(
    evidence: &Evidence,
    recorded_at_height: u64,
    context: &HeightContext,
) -> Vec<ValidatorIndex> {
    if evidence_context_height(evidence, recorded_at_height) != context.height {
        return Vec::new();
    }
    let signer = match &evidence.equivocation.conflict {
        iroha_data_model::block::consensus_v2::SumeragiV2Equivocation::Proposal {
            first, ..
        } => first.proposer,
        iroha_data_model::block::consensus_v2::SumeragiV2Equivocation::PhaseVote {
            first, ..
        } => first.signer,
        iroha_data_model::block::consensus_v2::SumeragiV2Equivocation::TimeoutVote {
            first,
            ..
        } => first.signer,
    };
    canonical_indices([signer], context.roster.len())
}

#[cfg(test)]
fn penalty_staking_fixture_ids() -> (
    iroha_data_model::asset::AssetDefinitionId,
    AccountId,
    AccountId,
) {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{asset::AssetDefinitionId, domain::DomainId};

    let account = |seed: u8| {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic penalty-custody key");
        AccountId::new(key.public_key().clone())
    };
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("penalty", "universal").expect("penalty fixture domain id"),
        "stake".parse().expect("penalty fixture asset name"),
    );
    (asset_definition, account(0xE1), account(0xE2))
}

#[cfg(test)]
fn penalty_staking_fixture_header() -> BlockHeader {
    BlockHeader::new(
        core::num::NonZeroU64::new(1).expect("non-zero penalty fixture height"),
        None,
        None,
        None,
        1,
        0,
    )
}

/// Install the governed stake asset and distinct escrow/slash-sink accounts used by
/// Sumeragi penalty tests.
///
/// The setup is committed through a real world overlay so asset indexes and the
/// definition incarnation stay consistent with production instruction execution.
#[cfg(test)]
pub(crate) fn configure_penalty_staking_state_for_tests(state: &mut State) {
    use crate::smartcontracts::Execute as _;
    use iroha_data_model::{
        account::Account,
        asset::{AssetBalancePolicy, AssetDefinition},
        isi::Register,
    };

    let (asset_definition, escrow, slash_sink) = penalty_staking_fixture_ids();
    assert_ne!(
        escrow, slash_sink,
        "penalty fixture must exercise an actual escrow-to-sink movement"
    );
    let mut nexus = state.nexus_snapshot();
    nexus.staking.stake_asset_id = asset_definition.to_string();
    nexus.staking.stake_escrow_account_id = escrow.to_string();
    nexus.staking.slash_sink_account_id = slash_sink.to_string();
    state
        .set_nexus(nexus)
        .expect("install penalty staking custody configuration");

    let mut state_block = state.block(penalty_staking_fixture_header());
    let mut transaction = state_block.transaction();
    Register::account(Account::new(escrow.clone()))
        .execute(&escrow, &mut transaction)
        .expect("register penalty stake escrow account");
    Register::account(Account::new(slash_sink))
        .execute(&escrow, &mut transaction)
        .expect("register penalty slash sink account");
    Register::asset_definition(AssetDefinition::numeric(
        asset_definition,
        "Penalty stake".to_owned(),
        AssetBalancePolicy::Global,
        None,
    ))
    .execute(&escrow, &mut transaction)
    .expect("register penalty stake asset definition");
    transaction.apply();
    state_block
        .commit_world_overlay_for_testing()
        .expect("commit penalty staking custody fixture");
}

/// Seed one validator with an account, an exactly backed escrow balance, and
/// the matching retained validator/share rows needed by the slash executor.
#[cfg(test)]
pub(crate) fn seed_penalty_validator_for_tests(
    state: &State,
    lane_id: LaneId,
    peer: &PeerId,
    stake: Quantity,
) -> AccountId {
    use crate::smartcontracts::Execute as _;
    use iroha_data_model::{
        account::Account,
        asset::AssetId,
        isi::{Mint, Register},
        metadata::Metadata,
        nexus::{PublicLaneStakeShare, PublicLaneValidatorRecord, PublicLaneValidatorStatus},
    };

    assert!(!stake.is_zero(), "penalty validator stake must be non-zero");
    let (asset_definition, escrow, _slash_sink) = penalty_staking_fixture_ids();
    let validator = AccountId::new(peer.public_key().clone());
    let mut state_block = state.block(penalty_staking_fixture_header());
    let mut transaction = state_block.transaction();
    if transaction.world.accounts.get(&validator).is_none() {
        Register::account(Account::new(validator.clone()))
            .execute(&validator, &mut transaction)
            .expect("register penalty validator account");
    }
    Mint::asset_quantity(
        stake.clone(),
        AssetId::new(asset_definition, escrow.clone()),
    )
    .execute(&escrow, &mut transaction)
    .expect("mint exact penalty stake into escrow");
    assert!(
        transaction
            .world
            .public_lane_validators
            .insert(
                (lane_id, validator.clone()),
                PublicLaneValidatorRecord {
                    lane_id,
                    validator: validator.clone(),
                    peer_id: peer.clone(),
                    stake_account: validator.clone(),
                    total_stake: stake.clone(),
                    self_stake: stake.clone(),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_height: 1,
                    deactivation_height: None,
                    last_reward_epoch: None,
                },
            )
            .is_none(),
        "penalty validator fixture must not replace an existing row"
    );
    assert!(
        transaction
            .world
            .public_lane_stake_shares
            .insert(
                (lane_id, validator.clone(), validator.clone()),
                PublicLaneStakeShare {
                    lane_id,
                    validator: validator.clone(),
                    staker: validator.clone(),
                    bonded: stake,
                    pending_unbonds: BTreeMap::new(),
                    metadata: Metadata::default(),
                },
            )
            .is_none(),
        "penalty validator fixture must not replace an existing stake share"
    );
    transaction.apply();
    state_block
        .commit_world_overlay_for_testing()
        .expect("commit exactly backed penalty validator fixture");
    validator
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::isi::staking::apply_slash_to_validator_without_observability,
        state::{State, StateBlock, World},
        sumeragi::evidence::evidence_key,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        IntoKeyValue, NetworkId,
        asset::{Asset, AssetDefinitionId, AssetId},
        block::{
            BlockHeader, SignedBlock,
            consensus::{Evidence, EvidenceRecord},
            consensus_v2::{
                BlockSubject, ConsensusMode as V2ConsensusMode, ConsensusRound,
                DataAvailabilityLayout, DualQuorum, ExecutionCommitment, GlobalPhase,
                HeightContext, PayloadEncoding, QuorumCertificate, ValidatorPower,
                finality::V2FinalityArtifact,
            },
        },
        metadata::Metadata,
        nexus::{
            LaneCatalog, LaneConfig, LaneId, LaneVisibility, PublicLaneStakeShare,
            PublicLaneUnbonding,
        },
        parameter::{Parameter, system::SumeragiNposParameters},
        prelude::{AccountId, PeerId},
    };
    use iroha_primitives::numeric::Quantity;
    use std::{
        num::{NonZeroU32, NonZeroU64},
        sync::Arc,
    };
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("penalty fixture key generation should succeed")
    }
    fn penalty_staking_ids() -> (AssetDefinitionId, AccountId, AccountId) {
        penalty_staking_fixture_ids()
    }
    fn fresh_state() -> State {
        let mut state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        configure_penalty_staking_state_for_tests(&mut state);
        state
    }

    fn enable_shared_public_staking_lanes(state: &mut State) {
        let mut nexus = state.nexus_snapshot();
        nexus.lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "penalty-sibling".to_owned(),
                    visibility: LaneVisibility::Public,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("shared public penalty lane catalog");
        state
            .set_nexus(nexus)
            .expect("install shared public penalty lane catalog");
    }

    fn roster_keys() -> Vec<KeyPair> {
        let mut keys = (0_u8..4)
            .map(|index| {
                KeyPair::try_from_seed(vec![0xD0 + index; 32], Algorithm::BlsNormal)
                    .expect("deterministic penalty-roster BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        keys
    }
    fn roster() -> Vec<PeerId> {
        roster_keys()
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect()
    }
    fn test_block_hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
    }
    fn phase_vote_evidence(context: &HeightContext, signer: ValidatorIndex, view: u64) -> Evidence {
        let round = ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let execution_commitment =
            ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
                Hash::new(b"penalty evidence parent state"),
                Hash::new(b"penalty evidence post state"),
                Hash::new(b"penalty evidence ordinary writes"),
                1,
                Hash::new(b"penalty evidence executed block"),
            );
        let keys = roster_keys();
        let vote = |seed: u8| {
            let mut vote = iroha_data_model::block::consensus_v2::Vote {
                round,
                proposal_round: round,
                phase: GlobalPhase::Prepare,
                subject: BlockSubject {
                    parent_block_hash: None,
                    block_hash: test_block_hash(seed),
                    payload_hash: Hash::new([seed]),
                },
                execution_commitment,
                signer,
                signature: Vec::new(),
            };
            let key = &keys[usize::try_from(signer).expect("fixture signer index")];
            vote.signature = Signature::new(key.private_key(), &vote.signature_preimage())
                .payload()
                .to_vec();
            vote
        };
        Evidence {
            equivocation: iroha_data_model::block::consensus::SumeragiV2EquivocationEvidence {
                context: context.clone(),
                proofs_of_possession: keys
                    .iter()
                    .map(|key| {
                        iroha_crypto::bls_normal_pop_prove(key.private_key())
                            .expect("fixture validator PoP")
                    })
                    .collect(),
                conflict:
                    iroha_data_model::block::consensus_v2::SumeragiV2Equivocation::PhaseVote {
                        first: vote(0x31),
                        second: vote(0x32),
                    },
            },
        }
    }
    fn height_one_context(
        network_id: NetworkId,
        roster: &[PeerId],
        _block_hash: HashOf<BlockHeader>,
    ) -> HeightContext {
        let roster = roster
            .iter()
            .cloned()
            .map(|validator| ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        let (offline_cash_mint_finality_epoch_id, offline_cash_mint_finality_epoch_roster) =
            crate::offline_cash_v1_test_fixtures::mint_finality_roster_and_id(
                network_id, 0, &roster,
            );
        HeightContext {
            network_id,
            protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: V2ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
            roster,
            offline_cash_mint_finality_epoch_id,
            offline_cash_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::new(b"penalties v2 test context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x42; 32],
        }
    }
    fn install_height_one_artifact(state: &State, roster: &[PeerId]) -> HeightContext {
        install_height_one_artifact_with_network(state, roster, state.network_id_ref().clone())
    }
    fn install_height_one_artifact_with_network(
        state: &State,
        roster: &[PeerId],
        network_id: NetworkId,
    ) -> HeightContext {
        let roster_keys = roster_keys();
        assert_eq!(
            roster,
            roster_keys
                .iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect::<Vec<_>>(),
            "artifact fixture roster must retain its deterministic signing keys"
        );
        let signing_key = &roster_keys[0];
        let committed =
            ValidBlock::new_dummy_and_modify_header(signing_key.private_key(), |header| {
                header.set_height(NonZeroU64::new(1).expect("non-zero height"));
                header.set_prev_block_hash(None);
                header.merkle_root = None;
            })
            .commit_unchecked()
            .unpack(|_| {});
        let mut executed_block: SignedBlock = committed.into();
        executed_block
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("attach deterministic penalties fixture results");
        let block = Arc::new(executed_block);
        state
            .kura()
            .store_block(Arc::clone(&block))
            .expect("store canonical test block");
        let context = height_one_context(network_id, roster, block.hash());
        let subject = BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal wire"),
        };
        let execution_commitment =
            ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
                Hash::new(b"penalties fixture parent state"),
                Hash::new(b"penalties fixture post state"),
                Hash::new(b"penalties fixture ordinary writes"),
                u64::try_from(block.encode_wire().expect("penalties block wire").len())
                    .expect("penalties block wire length fits u64"),
                block
                    .executed_block_wire_hash()
                    .expect("canonical executed block wire"),
            );
        let round = ConsensusRound {
            context_id: context.id(),
            height: 1,
            view: block.header().view_change_index(),
        };
        let mut certificate = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x5A; 48],
        };
        let preimage = certificate
            .signer_preimage(&context, 0)
            .expect("valid penalties finality fixture signer");
        let shares = roster_keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        certificate.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate fixture CommitQC");
        let validator_set_pops = roster_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect();
        let _receipt = state
            .kura()
            .store_v2_finality_artifact(&V2FinalityArtifact::new(
                context.clone(),
                subject,
                certificate,
                validator_set_pops,
            ))
            .expect("persist canonical v2 finality artifact");
        context
    }
    fn set_commit_topology(state: &State, peers: Vec<PeerId>) {
        let mut topology = state.commit_topology.block();
        topology.clear();
        topology.extend(peers);
        topology.commit();
    }
    fn insert_evidence(state: &State, evidence: Evidence, recorded_at_height: u64) -> Hash {
        let key = evidence_key(&evidence);
        let record = EvidenceRecord {
            evidence,
            recorded_at_height,
            recorded_at_view: 0,
            recorded_at_ms: recorded_at_height.saturating_mul(1_000),
            penalty_status: EvidencePenaltyStatus::Pending,
        };
        let mut block = state.world.consensus_evidence.block();
        block.insert(key, record);
        block.commit();
        key
    }

    fn fund_penalty_escrow(state: &State, amount: &Quantity) {
        let (asset_definition, escrow, _) = penalty_staking_ids();
        let escrow_asset = AssetId::new(asset_definition.clone(), escrow);
        {
            let mut assets = state.world.assets.block();
            let current = assets
                .get(&escrow_asset)
                .map(|value| value.as_ref().clone())
                .unwrap_or_else(Quantity::zero);
            let balance = current
                .checked_add(amount)
                .expect("penalty escrow balance remains bounded");
            let (_, value) = Asset::new(escrow_asset.clone(), balance).into_key_value();
            assets.insert(escrow_asset, value);
            assets.commit();
        }
        {
            let mut definitions = state.world.asset_definitions.block();
            let mut definition = definitions
                .get(&asset_definition)
                .cloned()
                .expect("penalty stake definition exists");
            definition.total_quantity = definition
                .total_quantity
                .checked_add(amount)
                .expect("penalty stake issuance remains bounded");
            definitions.insert(asset_definition, definition);
            definitions.commit();
        }
    }

    fn add_validator_record_on_lane(state: &State, lane_id: LaneId, peer: &PeerId) -> AccountId {
        seed_penalty_validator_for_tests(state, lane_id, peer, Quantity::from(10_000_u64))
    }

    fn add_validator_record(state: &State, peer: &PeerId) -> AccountId {
        add_validator_record_on_lane(state, LaneId::SINGLE, peer)
    }

    fn install_one_block_delay_npos(state: &State) {
        let mut parameters = state.world.parameters.block();
        let npos = SumeragiNposParameters {
            slashing_delay_blocks: 1,
            ..SumeragiNposParameters::default()
        };
        parameters.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
        parameters.commit();
    }
    fn penalty_header(height: u64) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero penalty test height"),
            None,
            None,
            None,
            height.saturating_mul(1_000),
            0,
        )
    }
    fn height_two_state_block(state: &State) -> StateBlock<'_> {
        state.block(penalty_header(2))
    }
    fn retire_primary_lane_in_candidate(state_block: &mut StateBlock<'_>) {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![LaneConfig {
                id: LaneId::new(1),
                alias: "post-execution-lane".to_owned(),
                ..LaneConfig::default()
            }],
        )
        .expect("sparse post-execution lane catalog");
        state_block.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog);
        state_block.nexus.lane_catalog = catalog;
    }
    #[test]
    fn consensus_penalty_delay_does_not_saturate_into_early_eligibility() {
        assert!(consensus_penalty_is_due(u64::MAX, 0, u64::MAX));
        assert!(consensus_penalty_is_due(u64::MAX - 1, 1, u64::MAX));
        assert!(!consensus_penalty_is_due(u64::MAX, 1, u64::MAX));
        assert!(!consensus_penalty_is_due(u64::MAX - 1, 2, u64::MAX));
    }
    #[test]
    fn parent_snapshot_indexes_every_stake_share_exactly_once() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let peers = roster();
        let first = add_validator_record(&state, &peers[0]);
        let second = add_validator_record(&state, &peers[1]);
        let delegator = AccountId::new(
            KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
                .expect("deterministic penalty delegator")
                .public_key()
                .clone(),
        );
        let pending_id = Hash::new(b"indexed pending unbond");
        {
            let key = (LaneId::SINGLE, first.clone(), first.clone());
            let mut block = state.world.public_lane_stake_shares.block();
            let mut share = block.get(&key).cloned().expect("first self-share exists");
            share.bonded = Quantity::from(9_000_u64);
            share.pending_unbonds.insert(
                pending_id,
                PublicLaneUnbonding {
                    request_id: pending_id,
                    amount: Quantity::from(1_000_u64),
                    release_at_ms: 10_000,
                    slashable_through_height: 1,
                    liability_release_height: 3,
                },
            );
            block.insert(key, share);
            block.insert(
                (LaneId::SINGLE, first.clone(), delegator.clone()),
                PublicLaneStakeShare {
                    lane_id: LaneId::SINGLE,
                    validator: first.clone(),
                    staker: delegator,
                    bonded: Quantity::from(3_000_u64),
                    pending_unbonds: BTreeMap::new(),
                    metadata: Metadata::default(),
                },
            );
            block.commit();
        }
        fund_penalty_escrow(&state, &Quantity::from(3_000_u64));
        {
            let key = (LaneId::SINGLE, first.clone());
            let mut block = state.world.public_lane_validators.block();
            let mut record = block.get(&key).cloned().expect("first validator exists");
            record.total_stake = Quantity::from(12_000_u64);
            record.self_stake = Quantity::from(9_000_u64);
            block.insert(key, record);
            block.commit();
        }

        let view = state.view();
        let snapshot = PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        )
        .expect("canonical multi-validator stake snapshot");

        assert_eq!(snapshot.stake_share_rows_scanned, 3);
        let first_locator = snapshot
            .validator_map
            .get(peers[0].public_key())
            .and_then(|locators| locators.first())
            .expect("first validator indexed");
        assert_eq!(first_locator.validator, first);
        assert_eq!(first_locator.slashable_exposure, Quantity::from(13_000_u64));
        let second_locator = snapshot
            .validator_map
            .get(peers[1].public_key())
            .and_then(|locators| locators.first())
            .expect("second validator indexed");
        assert_eq!(second_locator.validator, second);
        assert_eq!(
            second_locator.slashable_exposure,
            Quantity::from(10_000_u64)
        );
    }
    #[test]
    fn parent_snapshot_rejects_corrupt_stake_share_outside_candidate_set() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let validator = AccountId::new(
            KeyPair::try_from_seed(vec![0xA6; 32], Algorithm::Ed25519)
                .expect("deterministic corrupt-share validator")
                .public_key()
                .clone(),
        );
        let staker = AccountId::new(
            KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::Ed25519)
                .expect("deterministic corrupt-share staker")
                .public_key()
                .clone(),
        );
        let share = PublicLaneStakeShare {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            staker: staker.clone(),
            bonded: Quantity::from(1_u64),
            pending_unbonds: BTreeMap::new(),
            metadata: Metadata::default(),
        };
        let mut block = state.world.public_lane_stake_shares.block();
        block.insert((LaneId::SINGLE, validator, staker), share);
        block.commit();

        let view = state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("a corrupt share row must fail the complete parent snapshot"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("failed to index slashable public-lane stake exposure"),
            "unexpected rejection: {error}"
        );
        assert!(
            format!("{error:#}").contains("stake share does not match its storage key"),
            "unexpected rejection chain: {error:#}"
        );
    }
    #[test]
    fn parent_snapshot_rejects_orphan_stake_share_aggregate() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let validator = AccountId::new(
            KeyPair::try_from_seed(vec![0xA8; 32], Algorithm::Ed25519)
                .expect("deterministic orphan-share validator")
                .public_key()
                .clone(),
        );
        let share = PublicLaneStakeShare {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            staker: validator.clone(),
            bonded: Quantity::from(1_u64),
            pending_unbonds: BTreeMap::new(),
            metadata: Metadata::default(),
        };
        let mut block = state.world.public_lane_stake_shares.block();
        block.insert((LaneId::SINGLE, validator.clone(), validator), share);
        block.commit();

        let view = state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("an orphan stake-share aggregate must fail the parent snapshot"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("has no validator record"),
            "unexpected rejection: {error:#}"
        );
    }
    #[test]
    fn parent_snapshot_enforces_stake_share_and_pending_unbond_caps() {
        let mut share_capped_state = fresh_state();
        share_capped_state
            .nexus
            .get_mut()
            .staking
            .max_stake_shares_per_validator = NonZeroU32::new(1).expect("non-zero stake-share cap");
        install_one_block_delay_npos(&share_capped_state);
        let peers = roster();
        let validator = add_validator_record(&share_capped_state, &peers[0]);
        let delegator = AccountId::new(
            KeyPair::try_from_seed(vec![0xA9; 32], Algorithm::Ed25519)
                .expect("deterministic capped-share delegator")
                .public_key()
                .clone(),
        );
        let share = PublicLaneStakeShare {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            staker: delegator.clone(),
            bonded: Quantity::from(1_u64),
            pending_unbonds: BTreeMap::new(),
            metadata: Metadata::default(),
        };
        let mut block = share_capped_state.world.public_lane_stake_shares.block();
        block.insert((LaneId::SINGLE, validator, delegator), share);
        block.commit();
        let view = share_capped_state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("stake-share cap overflow must fail the parent snapshot"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("exceeds stake-share capacity"),
            "unexpected rejection: {error:#}"
        );

        let mut pending_capped_state = fresh_state();
        pending_capped_state
            .nexus
            .get_mut()
            .staking
            .max_pending_unbonds_per_share =
            NonZeroU32::new(1).expect("non-zero pending-unbond cap");
        install_one_block_delay_npos(&pending_capped_state);
        let validator = add_validator_record(&pending_capped_state, &peers[0]);
        let key = (LaneId::SINGLE, validator.clone(), validator);
        let mut block = pending_capped_state.world.public_lane_stake_shares.block();
        let mut share = block.get(&key).cloned().expect("capped self-share exists");
        for marker in [0xB0_u8, 0xB1_u8] {
            let request_id = Hash::new([marker]);
            share.pending_unbonds.insert(
                request_id,
                PublicLaneUnbonding {
                    request_id,
                    amount: Quantity::from(1_u64),
                    release_at_ms: 10_000,
                    slashable_through_height: 1,
                    liability_release_height: 3,
                },
            );
        }
        block.insert(key, share);
        block.commit();
        let view = pending_capped_state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("pending-unbond cap overflow must fail the parent snapshot"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("exceeds pending-unbond capacity"),
            "unexpected rejection: {error:#}"
        );
    }
    #[test]
    fn parent_snapshot_enforces_retained_validator_capacity() {
        let mut state = fresh_state();
        state.nexus.get_mut().staking.max_validators =
            NonZeroU32::new(1).expect("non-zero validator cap");
        install_one_block_delay_npos(&state);
        let peers = roster();
        add_validator_record(&state, &peers[0]);
        add_validator_record(&state, &peers[1]);

        let view = state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("validator-cap overflow must fail the parent snapshot"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("exceeds retained validator capacity"),
            "unexpected rejection: {error:#}"
        );
    }
    #[test]
    fn parent_snapshot_rejects_non_canonical_pending_unbond() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let peers = roster();
        let validator = add_validator_record(&state, &peers[0]);
        let key = (LaneId::SINGLE, validator.clone(), validator);
        let map_key = Hash::new(b"pending map key");
        let payload_id = Hash::new(b"different pending payload id");
        let mut block = state.world.public_lane_stake_shares.block();
        let mut share = block.get(&key).cloned().expect("self-share exists");
        share.pending_unbonds.insert(
            map_key,
            PublicLaneUnbonding {
                request_id: payload_id,
                amount: Quantity::from(1_u64),
                release_at_ms: 10_000,
                slashable_through_height: 1,
                liability_release_height: 3,
            },
        );
        block.insert(key, share);
        block.commit();

        let view = state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("a non-canonical pending unbond must fail the parent snapshot"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("pending unbond is non-canonical"),
            "unexpected rejection: {error:#}"
        );
    }
    #[test]
    fn parent_snapshot_rejects_non_validator_self_stake_account() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let peers = roster();
        let validator = add_validator_record(&state, &peers[0]);
        let foreign_stake_account = AccountId::new(
            KeyPair::try_from_seed(vec![0xB2; 32], Algorithm::Ed25519)
                .expect("deterministic foreign stake account")
                .public_key()
                .clone(),
        );
        let key = (LaneId::SINGLE, validator);
        let mut block = state.world.public_lane_validators.block();
        let mut record = block.get(&key).cloned().expect("validator record exists");
        record.stake_account = foreign_stake_account;
        block.insert(key, record);
        block.commit();

        let view = state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("a foreign self-stake account must fail the parent snapshot"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("stake account must match the validator account"),
            "unexpected rejection: {error:#}"
        );
    }
    #[test]
    fn parent_snapshot_rejects_validator_totals_that_disagree_with_index() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let peers = roster();
        let validator = add_validator_record(&state, &peers[0]);
        let key = (LaneId::SINGLE, validator);
        let mut block = state.world.public_lane_validators.block();
        let mut record = block.get(&key).cloned().expect("validator record exists");
        record.total_stake = Quantity::from(9_999_u64);
        block.insert(key, record);
        block.commit();

        let view = state.view();
        let error = match PenaltyApplier::parent_snapshot(
            &view,
            crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
        ) {
            Ok(_) => panic!("mismatched validator totals must fail the parent snapshot"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}")
                .contains("public-lane validator totals do not match canonical stake shares"),
            "unexpected rejection: {error:#}"
        );
    }
    #[test]
    fn admitted_self_contained_evidence_does_not_require_a_kura_reread() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = height_one_context(
            *state.network_id_ref(),
            &frozen_roster,
            test_block_hash(0x81),
        );
        let evidence = phase_vote_evidence(&context, 1, 0);
        let key = insert_evidence(&state, evidence, 1);
        let applier = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let actions = applier
            .derive_npos_consensus_effects(&penalty_header(2))
            .expect("admitted proof is self-contained")
            .penalty_actions;
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::MarkConsensusEvidenceApplied(mark)
                if mark.evidence_key == key && mark.height == 2
        )));
    }
    #[test]
    fn signer_indices_are_canonical_and_do_not_rotate_with_view() {
        let state = fresh_state();
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let view_zero = phase_vote_evidence(&context, 2, 0);
        let late_view = phase_vote_evidence(&context, 2, u64::MAX);
        assert_eq!(offender_indices(&view_zero, 1, &context), vec![2]);
        assert_eq!(offender_indices(&late_view, 1, &context), vec![2]);
    }
    #[test]
    fn npos_mode_does_not_remap_equal_vote_evidence_signer_indices() {
        let state = fresh_state();
        let frozen_roster = roster();
        let mut context = height_one_context(
            state.network_id_ref().clone(),
            &frozen_roster,
            test_block_hash(0x90),
        );
        context.mode = V2ConsensusMode::Npos;
        context.validate().expect("valid equal-vote NPoS context");
        let evidence = phase_vote_evidence(&context, 1, 47);
        assert_eq!(offender_indices(&evidence, 1, &context), vec![1]);
    }
    #[test]
    fn canonical_indices_filter_duplicates_and_out_of_range_signers() {
        assert_eq!(canonical_indices([3, 1, 3, 7, u32::MAX], 4), vec![1, 3]);
        assert!(canonical_indices([0], 0).is_empty());
    }
    #[test]
    fn derived_slash_targets_frozen_roster_even_when_live_topology_diverges() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        set_commit_topology(
            &state,
            vec![PeerId::new(checked_keypair().public_key().clone())],
        );
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record(&state, &offender);
        let evidence = phase_vote_evidence(&context, 1, 37);
        assert_eq!(offender_indices(&evidence, 1, &context), vec![1]);
        let key = insert_evidence(&state, evidence, 1);
        {
            let view = state.view();
            let snapshot = PenaltyApplier::parent_snapshot(
                &view,
                crate::sumeragi::evidence::v2_committed_evidence_snapshot(view.world()),
            )
            .expect("canonical penalty parent snapshot");
            let locators = snapshot
                .validator_map
                .get(offender.public_key())
                .expect("active validator tenure is indexed by its frozen-roster key");
            assert_eq!(locators.len(), 1);
            assert_eq!(locators[0].activation_height, 1);
            assert_eq!(locators[0].slashable_exposure, Quantity::from(10_000_u64));
            assert_eq!(snapshot.max_slash_bps, 10_000);
            assert_eq!(
                max_slash_amount(&locators[0].slashable_exposure, snapshot.max_slash_bps)
                    .expect("canonical slash amount"),
                Quantity::from(10_000_u64)
            );
        }
        {
            let mut scratch = state.consensus_effects_probe_block(penalty_header(2));
            let mut transaction = scratch.consensus_effects_transaction();
            apply_slash_to_validator_without_observability(
                &mut transaction,
                LaneId::SINGLE,
                &validator,
                key,
                &Quantity::from(10_000_u64),
                2_000,
            )
            .expect("the canonical penalty fixture must admit its derived slash");
        }
        let actions = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("canonical evidence produces deterministic effects")
        .penalty_actions;
        assert!(
            actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::ConsensusSlash(slash)
                    if slash.evidence_key == key
                        && slash.signer == 1
                        && slash.peer_id == offender
                        && slash.validator == validator
            )),
            "derived actions: {actions:#?}"
        );
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::MarkConsensusEvidenceApplied(mark)
                if mark.evidence_key == key && mark.height == 2
        )));
    }
    #[test]
    fn multiple_due_evidence_records_slash_only_remaining_custody() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record(&state, &offender);
        let first_key = insert_evidence(&state, phase_vote_evidence(&context, 1, 37), 1);
        let second_key = insert_evidence(&state, phase_vote_evidence(&context, 1, 38), 1);
        assert_ne!(first_key, second_key);

        let actions = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("sequential penalties consume only custody that remains")
        .penalty_actions;

        let slashes = actions
            .iter()
            .filter_map(|action| match action {
                NposPenaltyAction::ConsensusSlash(slash) => Some(slash),
                NposPenaltyAction::MarkConsensusEvidenceApplied(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(slashes.len(), 1);
        assert_eq!(slashes[0].validator, validator);
        assert_eq!(slashes[0].amount, Quantity::from(10_000_u64));
        let terminal_keys = actions
            .iter()
            .filter_map(|action| match action {
                NposPenaltyAction::MarkConsensusEvidenceApplied(mark) => Some(mark.evidence_key),
                NposPenaltyAction::ConsensusSlash(_) => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(terminal_keys, BTreeSet::from([first_key, second_key]));
    }
    #[test]
    fn penalty_derivation_fails_closed_on_missing_staking_custody_definition() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let offender = frozen_roster[1].clone();
        add_validator_record(&state, &offender);
        insert_evidence(&state, phase_vote_evidence(&context, 1, 37), 1);
        let (asset_definition, _, _) = penalty_staking_ids();
        let mut definitions = state.world.asset_definitions.block();
        definitions
            .remove(asset_definition)
            .expect("remove fixture staking definition");
        definitions.commit();

        let error = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect_err("invalid custody must abort rather than silently terminalize evidence");
        assert!(
            format!("{error:#}").contains("stake asset definition missing"),
            "unexpected rejection: {error:#}"
        );
    }
    #[test]
    fn evidence_cannot_slash_a_peer_tenure_activated_after_the_offence() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record(&state, &offender);
        let validator_key = (LaneId::SINGLE, validator.clone());
        let mut records = state.world.public_lane_validators.block();
        let mut record = records
            .get(&validator_key)
            .cloned()
            .expect("validator tenure exists");
        record.activation_height = 2;
        records.insert(validator_key, record);
        records.commit();
        let evidence = phase_vote_evidence(&context, 1, 37);
        let key = insert_evidence(&state, evidence, 1);

        let actions = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("pre-tenure evidence remains terminal and deterministic")
        .penalty_actions;

        assert!(
            !actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::ConsensusSlash(slash)
                    if slash.evidence_key == key || slash.validator == validator
            )),
            "evidence from an earlier peer tenure must not slash newly activated stake"
        );
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::MarkConsensusEvidenceApplied(mark)
                if mark.evidence_key == key && mark.height == 2
        )));
    }
    #[test]
    fn multiple_due_evidence_for_fully_slashed_signer_is_sequential_and_terminal() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record(&state, &offender);
        let first_key = insert_evidence(&state, phase_vote_evidence(&context, 1, 7), 1);
        let second_key = insert_evidence(&state, phase_vote_evidence(&context, 1, 8), 1);
        assert_ne!(first_key, second_key);

        let actions = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("sequential scratch application handles multiple due proofs")
        .penalty_actions;

        let slashes = actions
            .iter()
            .filter_map(|action| match action {
                NposPenaltyAction::ConsensusSlash(slash) => Some(slash),
                NposPenaltyAction::MarkConsensusEvidenceApplied(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            slashes.len(),
            1,
            "the first canonical proof consumes all retained stake, so a second slash cannot be derived"
        );
        assert_eq!(slashes[0].validator, validator);
        assert_eq!(slashes[0].amount, Quantity::from(10_000_u64));
        assert!(slashes[0].evidence_key == first_key || slashes[0].evidence_key == second_key);
        for evidence_key in [&first_key, &second_key] {
            assert!(actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::MarkConsensusEvidenceApplied(mark)
                    if &mark.evidence_key == evidence_key && mark.height == 2
            )));
        }

        let (asset_definition, escrow, slash_sink) = penalty_staking_fixture_ids();
        let escrow_asset = iroha_data_model::asset::AssetId::new(asset_definition, escrow);
        let view = state.view();
        assert_eq!(
            view.world
                .assets()
                .get(&escrow_asset)
                .map(|balance| balance.as_ref().clone()),
            Some(Quantity::from(10_000_u64)),
            "scratch derivation must not debit committed escrow"
        );
        assert!(
            view.world
                .assets()
                .iter()
                .all(|(asset, _)| asset.account() != &slash_sink),
            "scratch derivation must not create a committed slash-sink balance"
        );
    }
    #[test]
    fn consensus_penalty_ignores_singleton_non_owner_shared_dataspace_projection() {
        let mut state = fresh_state();
        enable_shared_public_staking_lanes(&mut state);
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record_on_lane(&state, LaneId::new(1), &offender);
        let evidence = phase_vote_evidence(&context, 1, 37);
        let key = insert_evidence(&state, evidence, 1);

        let actions = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("canonical evidence remains processable")
        .penalty_actions;

        assert!(
            !actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::ConsensusSlash(slash)
                    if slash.evidence_key == key
                        || slash.peer_id == offender
                        || slash.validator == validator
            )),
            "a non-owner compatibility projection must not receive a consensus slash action"
        );
        assert!(
            actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::MarkConsensusEvidenceApplied(mark)
                    if mark.evidence_key == key
            )),
            "an unslashable offence must still reach a terminal state"
        );
    }
    #[test]
    fn post_execution_lane_retirement_rejects_the_entire_consensus_penalty_bundle() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = height_one_context(
            state.network_id_ref().clone(),
            &frozen_roster,
            test_block_hash(0xA1),
        );
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record(&state, &offender);
        let evidence_key = insert_evidence(&state, phase_vote_evidence(&context, 1, 0), 1);
        let effects = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("due evidence derives a complete penalty bundle");
        assert!(effects.penalty_actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::ConsensusSlash(slash)
                if slash.evidence_key == evidence_key && slash.validator == validator
        )));

        let evidence_prune_keys =
            crate::sumeragi::evidence::v2_committed_evidence_prune_keys_from_state(
                &state,
                2,
                effects.v2_evidence_admissions.len(),
            );
        let mut state_block = height_two_state_block(&state);
        retire_primary_lane_in_candidate(&mut state_block);
        let error = validate_npos_consensus_effects_after_execution(
            &mut state_block,
            &effects,
            &evidence_prune_keys,
            None,
            &frozen_roster,
            2,
            0,
            2_000,
        )
        .expect_err("retiring a slash target must reject the candidate block");
        assert!(
            error
                .to_string()
                .contains("lane made inactive by block execution"),
            "unexpected rejection: {error}"
        );
        let record = state_block
            .world
            .consensus_evidence
            .get(&evidence_key)
            .expect("rollback preserves the unresolved evidence");
        assert_eq!(record.penalty_status, EvidencePenaltyStatus::Pending);
    }
    #[test]
    fn post_execution_evidence_cancellation_rejects_slash_and_mark_atomically() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = height_one_context(
            state.network_id_ref().clone(),
            &frozen_roster,
            test_block_hash(0xA2),
        );
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record(&state, &offender);
        let evidence_key = insert_evidence(&state, phase_vote_evidence(&context, 1, 0), 1);
        let effects = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("due evidence derives a complete penalty bundle");
        let evidence_prune_keys =
            crate::sumeragi::evidence::v2_committed_evidence_prune_keys_from_state(
                &state,
                2,
                effects.v2_evidence_admissions.len(),
            );
        let mut state_block = height_two_state_block(&state);
        {
            let mut transaction = state_block.transaction();
            let mut record = transaction
                .world
                .consensus_evidence
                .get(&evidence_key)
                .cloned()
                .expect("candidate cancellation target exists");
            record.penalty_status = EvidencePenaltyStatus::Cancelled { height: 2 };
            transaction
                .world
                .consensus_evidence
                .insert(evidence_key, record);
            transaction.apply();
        }

        let error = validate_npos_consensus_effects_after_execution(
            &mut state_block,
            &effects,
            &evidence_prune_keys,
            None,
            &frozen_roster,
            2,
            0,
            2_000,
        )
        .expect_err("same-block cancellation must reject the candidate penalty bundle");
        assert!(
            error.to_string().contains("cancelled evidence"),
            "unexpected rejection: {error}"
        );
        let evidence = state_block
            .world
            .consensus_evidence
            .get(&evidence_key)
            .expect("candidate cancellation remains staged");
        assert_eq!(
            evidence.penalty_status,
            EvidencePenaltyStatus::Cancelled { height: 2 }
        );
        let validator_record = state_block
            .world
            .public_lane_validators
            .get(&(LaneId::SINGLE, validator))
            .expect("validator record remains staged");
        assert_eq!(validator_record.total_stake, Quantity::from(10_000_u64));
    }
    #[test]
    fn post_execution_penalty_validation_cannot_write_an_active_global_witness() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = height_one_context(
            state.network_id_ref().clone(),
            &frozen_roster,
            test_block_hash(0xA3),
        );
        let offender = frozen_roster[1].clone();
        add_validator_record(&state, &offender);
        insert_evidence(&state, phase_vote_evidence(&context, 1, 0), 1);
        let effects = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("due evidence derives a complete penalty bundle");
        let evidence_prune_keys =
            crate::sumeragi::evidence::v2_committed_evidence_prune_keys_from_state(
                &state,
                2,
                effects.v2_evidence_admissions.len(),
            );
        let mut state_block = height_two_state_block(&state);

        let witness_guard = crate::sumeragi::witness::exec_witness_guard();
        crate::sumeragi::witness::start_block();
        validate_npos_consensus_effects_after_execution(
            &mut state_block,
            &effects,
            &evidence_prune_keys,
            None,
            &frozen_roster,
            2,
            0,
            2_000,
        )
        .expect("valid penalty effects remain applicable in the rollback-only overlay");
        let witness = crate::sumeragi::witness::drain_exec_witness();
        drop(witness_guard);

        assert!(witness.reads.is_empty());
        assert!(witness.writes.is_empty());
        assert!(witness.fastpq_transcripts.is_empty());
        assert!(witness.fastpq_batches.is_empty());
    }
    #[test]
    fn committed_consensus_penalty_cannot_publish_transaction_execution_evidence() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        let context = height_one_context(
            state.network_id_ref().clone(),
            &frozen_roster,
            test_block_hash(0xA4),
        );
        let offender = frozen_roster[1].clone();
        add_validator_record(&state, &offender);
        insert_evidence(&state, phase_vote_evidence(&context, 1, 0), 1);
        let effects = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("due evidence derives a complete penalty bundle");
        let evidence_prune_keys =
            crate::sumeragi::evidence::v2_committed_evidence_prune_keys_from_state(
                &state,
                2,
                effects.v2_evidence_admissions.len(),
            );
        let mut state_block = height_two_state_block(&state);

        let witness_guard = crate::sumeragi::witness::exec_witness_guard();
        crate::sumeragi::witness::start_block();
        let mut transaction = state_block.consensus_effects_transaction();
        apply_npos_consensus_effects_to_transaction(
            &mut transaction,
            &effects,
            &evidence_prune_keys,
            None,
            &frozen_roster,
            2,
            0,
            2_000,
        )
        .expect("valid committed penalty effects apply");
        transaction.apply_consensus_effects();
        let witness = crate::sumeragi::witness::drain_exec_witness();
        drop(witness_guard);

        assert!(witness.reads.is_empty());
        assert!(witness.writes.is_empty());
        assert!(witness.fastpq_transcripts.is_empty());
        assert!(witness.fastpq_batches.is_empty());
        assert!(state_block.drain_transfer_transcripts().is_empty());
    }
}
