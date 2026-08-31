//! Deterministic `NPoS` consensus-evidence slashing.
#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    smartcontracts::isi::staking::{
        apply_consensus_slash_to_validator, apply_slash_to_validator_without_observability,
        max_slash_amount,
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
#[derive(Clone, Copy, Default)]
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
    total_stake: Quantity,
}
struct ParentPenaltySnapshot {
    evidence: super::evidence::V2CommittedEvidenceSnapshot,
    slashing_delay: u64,
    max_slash_bps: u16,
    validator_map: BTreeMap<PublicKey, Vec<ValidatorLocator>>,
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
        let world = view.world();
        let slashing_delay = crate::sumeragi::resolve_npos_slashing_delay_blocks_from_world(world)
            .ok_or_else(|| eyre!("NPoS penalty derivation requires signed NPoS parameters"))?;
        let mut candidates_map: BTreeMap<PublicKey, Vec<ValidatorLocator>> = BTreeMap::new();
        for (key, record) in world.public_lane_validators().iter() {
            if !public_lane_validator_record_matches_key(key, record) {
                continue;
            }
            let (lane_id, validator_id) = key;
            if !view.is_lane_active_for_authority(*lane_id)
                || view.staking_authority_lane(*lane_id) != Some(*lane_id)
            {
                continue;
            }
            candidates_map
                .entry(record.peer_id.public_key().clone())
                .or_default()
                .push(ValidatorLocator {
                    lane_id: *lane_id,
                    validator: validator_id.clone(),
                    total_stake: record.total_stake.clone(),
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
    ) -> Result<(Vec<iroha_data_model::block::consensus::SumeragiV2EquivocationEvidence>, Vec<NposPenaltyAction>)>
    {
        loop {
            let generation_before = self.state.state_view_generation();
            if generation_before % 2 != 0 {
                std::thread::yield_now();
                continue;
            }
            let view = self.state.view();
            let evidence = super::evidence::v2_committed_evidence_snapshot(view.world());
            let result = Self::parent_snapshot(&view, evidence.clone()).and_then(|snapshot| {
                let admissions = if include_admissions {
                    super::evidence::pending_v2_evidence_admissions_from_snapshot(
                        self.state,
                        block_header.height().get(),
                        &evidence,
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
        let mut pending: Vec<(Vec<u8>, EvidenceRecord)> = Vec::new();
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
            let slash_id = Hash::new(key.clone());
            for signer in offenders {
                let Some((peer_id, locators)) =
                    self.locate_validator_in_roster_cached(
                        signer,
                        &roster,
                        &snapshot.validator_map,
                    )
                else {
                    continue;
                };
                for locator in locators {
                    let amount = max_slash_amount(&locator.total_stake, snapshot.max_slash_bps)?;
                    if amount.is_zero() {
                        continue;
                    }
                    let slash = NposConsensusSlashAction {
                        evidence_key: key.clone(),
                        signer,
                        peer_id: peer_id.clone(),
                        lane_id: locator.lane_id,
                        validator: locator.validator,
                        slash_id,
                        amount,
                    };
                    let mut transaction = scratch.consensus_effects_transaction();
                    if apply_slash_to_validator_without_observability(
                        &mut transaction,
                        slash.lane_id,
                        &slash.validator,
                        slash.slash_id,
                        &slash.amount,
                        block_header.creation_time_ms,
                    )
                    .is_ok()
                    {
                        transaction.apply_consensus_effects();
                        actions.push(NposPenaltyAction::ConsensusSlash(slash));
                    }
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
    evidence_prune_keys: &[Vec<u8>],
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
    evidence_prune_keys: &[Vec<u8>],
    expected_beacon_anchor: Option<iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1>,
    authenticated_roster: &[PeerId],
    current_height: u64,
    current_view: u64,
    now_ms: u64,
) -> Result<()> {
    let mut tx = state_block.transaction();
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
    evidence_prune_keys: &[Vec<u8>],
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
    }
    for key in evidence_prune_keys {
        tx.world.consensus_evidence.remove(key.clone());
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
    for action in &effects.penalty_actions {
        match action {
            NposPenaltyAction::ConsensusSlash(action) => {
                ensure_evidence_penalty_is_unresolved(tx, &action.evidence_key)?;
                if !tx.is_lane_active_for_authority(action.lane_id) {
                    return Err(eyre!(
                        "consensus slash targets a lane made inactive by block execution"
                    ));
                }
                match mode {
                    EffectsApplicationMode::Commit => apply_consensus_slash_to_validator(
                        tx,
                        action.lane_id,
                        &action.validator,
                        action.slash_id,
                        &action.amount,
                        now_ms,
                    )?,
                    EffectsApplicationMode::ValidateOnly => {
                        apply_slash_to_validator_without_observability(
                            tx,
                            action.lane_id,
                            &action.validator,
                            action.slash_id,
                            &action.amount,
                            now_ms,
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
                    .insert(action.evidence_key.clone(), record);
            }
        }
    }
    Ok(outcome)
}
fn ensure_evidence_penalty_is_unresolved(
    tx: &StateTransaction<'_, '_>,
    evidence_key: &[u8],
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
mod tests {
    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, StateBlock, World},
        sumeragi::evidence::evidence_key,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        NetworkId,
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
            LaneCatalog, LaneConfig, LaneId, LaneVisibility, PublicLaneValidatorRecord,
            PublicLaneValidatorStatus,
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
    fn fresh_state() -> State {
        State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
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
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
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
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
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
    fn insert_evidence(state: &State, evidence: Evidence, recorded_at_height: u64) -> Vec<u8> {
        let key = evidence_key(&evidence);
        let record = EvidenceRecord {
            evidence,
            recorded_at_height,
            recorded_at_view: 0,
            recorded_at_ms: recorded_at_height.saturating_mul(1_000),
            penalty_status: EvidencePenaltyStatus::Pending,
        };
        let mut block = state.world.consensus_evidence.block();
        block.insert(key.clone(), record);
        block.commit();
        key
    }

    fn add_validator_record_on_lane(state: &State, lane_id: LaneId, peer: &PeerId) -> AccountId {
        let validator = AccountId::new(peer.public_key().clone());
        let record = PublicLaneValidatorRecord {
            lane_id,
            validator: validator.clone(),
            peer_id: peer.clone(),
            stake_account: validator.clone(),
            total_stake: Quantity::from(10_000_u64),
            self_stake: Quantity::from(10_000_u64),
            metadata: Metadata::default(),
            status: PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };
        let mut block = state.world.public_lane_validators.block();
        block.insert((lane_id, validator.clone()), record);
        block.commit();
        validator
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
    fn height_two_state_block(state: &State) -> StateBlock<'_> {
        state.block(BlockHeader::new(
            NonZeroU64::new(2).expect("non-zero penalty test height"),
            None,
            None,
            None,
            2_000,
            0,
        ))
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
            .derive_npos_consensus_effects(2)
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
        let key = insert_evidence(&state, evidence, 1);
        let actions = PenaltyApplier::new(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(2)
        .expect("canonical evidence produces deterministic effects")
        .penalty_actions;
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::ConsensusSlash(slash)
                if slash.evidence_key == key
                    && slash.signer == 1
                    && slash.peer_id == offender
                    && slash.validator == validator
        )));
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::MarkConsensusEvidenceApplied(mark)
                if mark.evidence_key == key && mark.height == 2
        )));
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
        .derive_npos_consensus_effects(2)
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
        .derive_npos_consensus_effects(2)
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
        .derive_npos_consensus_effects(2)
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
                .insert(evidence_key.clone(), record);
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
        .derive_npos_consensus_effects(2)
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
        .derive_npos_consensus_effects(2)
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
        let mut transaction = state_block.transaction();
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
        transaction.apply();
        let witness = crate::sumeragi::witness::drain_exec_witness();
        drop(witness_guard);

        assert!(witness.reads.is_empty());
        assert!(witness.writes.is_empty());
        assert!(witness.fastpq_transcripts.is_empty());
        assert!(witness.fastpq_batches.is_empty());
        assert!(state_block.drain_transfer_transcripts().is_empty());
    }
}
