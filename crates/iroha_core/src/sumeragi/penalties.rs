//! Penalty enforcement for `NPoS`: VRF non-participation and consensus evidence slashing.
#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    smartcontracts::isi::staking::{apply_slash_to_validator, max_slash_amount},
    state::{
        State, StateTransaction, WorldReadOnly, WorldTransaction,
        public_lane_validator_record_matches_key,
    },
    sumeragi::consensus::ValidatorIndex,
};
use eyre::{Result, WrapErr, eyre};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    block::{
        consensus::{Evidence, EvidenceRecord},
        consensus_v2::{ConsensusMode, HeightContext},
    },
    consensus::{
        NposConsensusEffects, NposConsensusSlashAction, NposMarkConsensusEvidenceAppliedAction,
        NposMarkVrfPenaltiesAppliedAction, NposPenaltyAction, NposVrfJailAction, VrfEpochRecord,
    },
    nexus::{LaneId, PublicLaneValidatorStatus},
    prelude::{AccountId, PeerId},
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Default)]
pub struct PenaltyOutcome {
    pub applied: u64,
    pub slashed: u64,
    pub jailed: u64,
}
#[derive(Clone)]
struct ValidatorLocator {
    lane_id: LaneId,
    validator: AccountId,
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
    pub(crate) fn from_committed_state(
        state: &'a State,
        _consensus_mode: ConsensusMode,
        #[cfg(feature = "telemetry")] _telemetry: Option<&'a StateTelemetry>,
        #[cfg(not(feature = "telemetry"))] _telemetry: Option<()>,
    ) -> Self {
        Self { state }
    }
    pub(crate) fn from_parts(
        state: &'a State,
        #[cfg(feature = "telemetry")] telemetry: Option<&'a StateTelemetry>,
        #[cfg(not(feature = "telemetry"))] telemetry: Option<()>,
    ) -> Self {
        Self::from_committed_state(
            state,
            ConsensusMode::Npos,
            #[cfg(feature = "telemetry")]
            telemetry,
            #[cfg(not(feature = "telemetry"))]
            telemetry,
        )
    }
    fn build_validator_locator_map(&self) -> BTreeMap<PublicKey, ValidatorLocator> {
        let world = self.state.world_view();
        let mut candidates_map: BTreeMap<PublicKey, Vec<ValidatorLocator>> = BTreeMap::new();
        for (key, record) in world.public_lane_validators().iter() {
            if !public_lane_validator_record_matches_key(key, record) {
                continue;
            }
            let (lane_id, validator_id) = key;
            if !self.state.is_lane_active_for_authority(*lane_id)
                || self.state.staking_authority_lane(*lane_id) != Some(*lane_id)
            {
                continue;
            }
            candidates_map
                .entry(record.peer_id.public_key().clone())
                .or_default()
                .push(ValidatorLocator {
                    lane_id: *lane_id,
                    validator: validator_id.clone(),
                });
        }
        let mut result = BTreeMap::new();
        for (pk, mut locators) in candidates_map {
            locators.sort_by(|lhs, rhs| {
                lhs.lane_id
                    .cmp(&rhs.lane_id)
                    .then_with(|| lhs.validator.cmp(&rhs.validator))
            });
            if let Some(best) = locators.into_iter().next() {
                result.insert(pk, best);
            }
        }
        result
    }
    pub(crate) fn derive_npos_consensus_effects(
        &self,
        current_height: u64,
        vrf_epoch_seals: impl IntoIterator<Item = VrfEpochRecord>,
    ) -> Result<NposConsensusEffects> {
        let mut effects = NposConsensusEffects {
            vrf_epoch_seals: vrf_epoch_seals.into_iter().collect(),
            v2_evidence_admissions: super::evidence::pending_v2_evidence_admissions(
                self.state,
                current_height,
            ),
            penalty_actions: self.derive_npos_penalty_actions(current_height)?,
        };
        effects.vrf_epoch_seals.sort_by_key(|record| record.epoch);
        effects.vrf_epoch_seals.dedup_by_key(|record| record.epoch);
        Ok(effects)
    }
    /// Derive only deterministic penalty actions from pre-block state.
    pub(crate) fn derive_npos_penalty_actions(
        &self,
        current_height: u64,
    ) -> Result<Vec<NposPenaltyAction>> {
        let mut actions = self.derive_vrf_penalty_actions(current_height)?;
        actions.extend(self.derive_consensus_penalty_actions(current_height)?);
        actions.sort();
        actions.dedup();
        Ok(actions)
    }
    fn derive_vrf_penalty_actions(&self, current_height: u64) -> Result<Vec<NposPenaltyAction>> {
        let view = self.state.world.vrf_epochs.view();
        let mut due_records: Vec<VrfEpochRecord> = Vec::new();
        for (_epoch, record) in view.iter() {
            if !record.finalized || record.penalties_applied {
                continue;
            }
            // A boundary record becomes immutable pre-state only at the next
            // height.  Its canonical Kura finality artifact is verified below
            // before any absence can authorize a state change.
            if record.updated_at_height >= current_height {
                continue;
            }
            due_records.push(record.clone());
        }
        drop(view);
        if due_records.is_empty() {
            return Ok(Vec::new());
        }
        let validator_map = self.build_validator_locator_map();
        let mut actions = Vec::new();
        for record in due_records {
            let artifact = self
                .state
                .kura()
                .v2_finality_artifact(record.updated_at_height)
                .wrap_err_with(|| {
                    format!(
                        "failed to read the VRF epoch {} boundary finality artifact at height {}",
                        record.epoch, record.updated_at_height
                    )
                })?
                .ok_or_else(|| {
                    eyre!(
                        "missing canonical finality artifact for VRF epoch {} boundary height {}",
                        record.epoch,
                        record.updated_at_height
                    )
                })?;
            if &artifact.height_context.network_id != self.state.network_id_ref() {
                return Err(eyre!(
                    "VRF epoch {} boundary finality artifact belongs to another chain",
                    record.epoch
                ));
            }
            super::v2_npos::validate_finalized_epoch_record(
                &artifact.height_context,
                &record,
            )
            .map_err(|error| {
                eyre!(
                    "VRF epoch {} is not authenticated by its boundary finality artifact: {error}",
                    record.epoch
                )
            })?;
            // The reason encodes the epoch through which the status remains
            // jailed.  Using the successor keeps the offender ineligible for
            // the next election instead of restoring it one block later.
            let jail_epoch = record.epoch.checked_add(1).ok_or_else(|| {
                eyre!(
                    "VRF epoch {} cannot identify a finite successor jail epoch",
                    record.epoch
                )
            })?;
            // A signed commitment is an attributable promise to reveal.  The
            // exact boundary record (including its non-reveal partition) is
            // covered by the verified CommitQC, so a proposer cannot jail a
            // validator unilaterally.  Validators with no commitment are not
            // jailed: network absence alone is not attributable evidence.
            for signer in &record.committed_no_reveal {
                let peer_id = artifact
                    .height_context
                    .roster
                    .get(usize::try_from(*signer).map_err(|_| {
                        eyre!("VRF non-reveal signer index cannot address the certified roster")
                    })?)
                    .ok_or_else(|| eyre!("VRF non-reveal signer is outside the certified roster"))?
                    .validator
                    .clone();
                let Some(locator) = validator_map.get(peer_id.public_key()) else {
                    continue;
                };
                actions.push(NposPenaltyAction::VrfJail(NposVrfJailAction {
                    epoch: record.epoch,
                    signer: *signer,
                    peer_id,
                    lane_id: locator.lane_id,
                    validator: locator.validator.clone(),
                    reason: format!("vrf_penalty_epoch_{jail_epoch}"),
                }));
            }
            actions.push(NposPenaltyAction::MarkVrfPenaltiesApplied(
                NposMarkVrfPenaltiesAppliedAction {
                    epoch: record.epoch,
                    height: current_height,
                },
            ));
        }
        Ok(actions)
    }
    #[allow(clippy::too_many_lines)]
    fn derive_consensus_penalty_actions(
        &self,
        current_height: u64,
    ) -> Result<Vec<NposPenaltyAction>> {
        let slashing_delay = {
            let world = self.state.world_view();
            crate::sumeragi::resolve_npos_slashing_delay_blocks_from_world(&world)
                .ok_or_else(|| eyre!("NPoS penalty derivation requires signed NPoS parameters"))?
        };
        let evidence_view = self.state.world.consensus_evidence.view();
        let mut pending: Vec<(Vec<u8>, EvidenceRecord)> = Vec::new();
        for (key, record) in evidence_view.iter() {
            if record.penalty_applied || record.penalty_cancelled {
                continue;
            }
            if record
                .consensus_admitted_at_height
                .is_none_or(|height| height >= current_height)
            {
                // Node-local observations and evidence admitted by the block
                // currently under construction can never drive deterministic
                // penalty attachments.
                continue;
            }
            if !consensus_penalty_is_due(record.recorded_at_height, slashing_delay, current_height)
            {
                continue;
            }
            pending.push((key.clone(), record.clone()));
        }
        drop(evidence_view);
        if pending.is_empty() {
            return Ok(Vec::new());
        }
        let validator_map = self.build_validator_locator_map();
        let mut actions = Vec::new();
        for (key, record) in pending {
            let Some(context) = height_context_for_evidence(
                self.state,
                &record.evidence,
                record.recorded_at_height,
            )?
            else {
                continue;
            };
            let roster = context
                .roster
                .iter()
                .map(|validator| validator.validator.clone())
                .collect::<Vec<_>>();
            let offenders = offender_indices(&record.evidence, record.recorded_at_height, &context);
            if offenders.is_empty() {
                continue;
            }
            let slash_id = Hash::new(key.clone());
            let mut slashes = 0_u64;
            for signer in offenders {
                let Some((peer_id, locator)) =
                    self.locate_validator_in_roster_cached(signer, &roster, &validator_map)
                else {
                    continue;
                };
                let Some(amount) = max_slash_amount_for_validator_from_state(
                    self.state,
                    &locator,
                    self.state.nexus_snapshot().staking.max_slash_bps,
                )?
                else {
                    continue;
                };
                actions.push(NposPenaltyAction::ConsensusSlash(
                    NposConsensusSlashAction {
                        evidence_key: key.clone(),
                        signer,
                        peer_id,
                        lane_id: locator.lane_id,
                        validator: locator.validator,
                        slash_id,
                        amount,
                    },
                ));
                slashes = slashes.saturating_add(1);
            }
            if slashes > 0 {
                actions.push(NposPenaltyAction::MarkConsensusEvidenceApplied(
                    NposMarkConsensusEvidenceAppliedAction {
                        evidence_key: key,
                        height: current_height,
                    },
                ));
            }
        }
        Ok(actions)
    }
    #[allow(clippy::unused_self)]
    fn locate_validator_in_roster_cached(
        &self,
        signer: ValidatorIndex,
        roster: &[PeerId],
        map: &BTreeMap<PublicKey, ValidatorLocator>,
    ) -> Option<(PeerId, ValidatorLocator)> {
        let signer_idx = usize::try_from(signer).ok()?;
        let peer = roster.get(signer_idx)?;
        map.get(peer.public_key())
            .cloned()
            .map(|locator| (peer.clone(), locator))
    }
}
pub(crate) fn apply_npos_consensus_effects_to_transaction(
    tx: &mut StateTransaction<'_, '_>,
    effects: &NposConsensusEffects,
    current_height: u64,
    current_view: u64,
    now_ms: u64,
    #[cfg(feature = "telemetry")] telemetry: Option<&StateTelemetry>,
) -> Result<PenaltyOutcome> {
    let mut outcome = PenaltyOutcome::default();
    for record in &effects.vrf_epoch_seals {
        tx.world.vrf_epochs.insert(record.epoch, record.clone());
    }
    for admission in &effects.v2_evidence_admissions {
        let evidence = super::evidence::canonical_v2_evidence(admission);
        let key = super::evidence::v2_evidence_admission_key(admission);
        if tx
            .world
            .consensus_evidence
            .get(&key)
            .is_some_and(|record| record.consensus_admitted_at_height.is_some())
        {
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
                penalty_applied: false,
                penalty_cancelled: false,
                penalty_cancelled_at_height: None,
                penalty_applied_at_height: None,
                consensus_admitted_at_height: Some(current_height),
            },
        );
    }
    for action in &effects.penalty_actions {
        match action {
            NposPenaltyAction::VrfJail(action) => {
                if !tx.is_lane_active_for_authority(action.lane_id) {
                    continue;
                }
                let locator = ValidatorLocator {
                    lane_id: action.lane_id,
                    validator: action.validator.clone(),
                };
                if jail_in_transaction(
                    &mut tx.world,
                    &locator,
                    &action.reason,
                    #[cfg(feature = "telemetry")]
                    telemetry,
                    #[cfg(not(feature = "telemetry"))]
                    None,
                ) {
                    outcome.applied = outcome.applied.saturating_add(1);
                    outcome.jailed = outcome.jailed.saturating_add(1);
                }
            }
            NposPenaltyAction::ConsensusSlash(action) => {
                if !tx.is_lane_active_for_authority(action.lane_id) {
                    continue;
                }
                apply_slash_to_validator(
                    tx,
                    action.lane_id,
                    &action.validator,
                    action.slash_id,
                    &action.amount,
                    now_ms,
                )?;
                outcome.applied = outcome.applied.saturating_add(1);
                outcome.slashed = outcome.slashed.saturating_add(1);
            }
            NposPenaltyAction::MarkVrfPenaltiesApplied(action) => {
                let mut record = tx.world.vrf_epochs.get(&action.epoch).cloned();
                if let Some(record) = record.as_mut() {
                    record.penalties_applied = true;
                    record.penalties_applied_at_height = Some(action.height);
                    tx.world.vrf_epochs.insert(action.epoch, record.clone());
                }
            }
            NposPenaltyAction::MarkConsensusEvidenceApplied(action) => {
                let mut record = tx
                    .world
                    .consensus_evidence
                    .get(&action.evidence_key)
                    .cloned();
                if let Some(record) = record.as_mut() {
                    record.penalty_applied = true;
                    record.penalty_applied_at_height = Some(action.height);
                    tx.world
                        .consensus_evidence
                        .insert(action.evidence_key.clone(), record.clone());
                }
            }
        }
    }
    Ok(outcome)
}
fn height_context_for_evidence(
    state: &State,
    evidence: &Evidence,
    recorded_at_height: u64,
) -> Result<Option<HeightContext>> {
    let height = evidence_context_height(evidence, recorded_at_height);
    if height == 0 || height > recorded_at_height {
        return Ok(None);
    }
    let artifact = state
        .kura()
        .v2_finality_artifact(height)
        .wrap_err_with(|| {
            format!("failed to read Sumeragi v2 finality artifact at height {height}")
        })?;
    let Some(artifact) = artifact else {
        return Err(eyre!(
            "missing canonical Sumeragi v2 finality artifact at evidence height {height}"
        ));
    };
    if &artifact.height_context.network_id != state.network_id_ref() {
        return Err(eyre!(
            "Sumeragi v2 finality artifact at evidence height {height} belongs to another chain"
        ));
    }
    if !evidence_matches_height_context(evidence, recorded_at_height, &artifact.height_context) {
        return Ok(None);
    }
    Ok(Some(artifact.height_context))
}
fn evidence_matches_height_context(
    evidence: &Evidence,
    recorded_at_height: u64,
    context: &HeightContext,
) -> bool {
    evidence_context_height(evidence, recorded_at_height) == context.height
        && &evidence.equivocation.context == context
        && super::evidence::validate_v2_equivocation(&evidence.equivocation).is_ok()
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
fn max_slash_amount_for_validator_from_state(
    state: &State,
    locator: &ValidatorLocator,
    max_bps: u16,
) -> Result<Option<Quantity>> {
    if !state.is_lane_active_for_authority(locator.lane_id) {
        return Ok(None);
    }
    let world = state.world_view();
    let Some(record) = world
        .public_lane_validators()
        .get(&(locator.lane_id, locator.validator.clone()))
        .cloned()
    else {
        return Ok(None);
    };
    let amount = max_slash_amount(&record.total_stake, max_bps)?;
    if amount.is_zero() {
        return Ok(None);
    }
    Ok(Some(amount))
}
fn jail_in_transaction(
    tx: &mut WorldTransaction<'_, '_>,
    locator: &ValidatorLocator,
    reason: &str,
    #[cfg(feature = "telemetry")] telemetry: Option<&StateTelemetry>,
    #[cfg(not(feature = "telemetry"))] _telemetry: Option<()>,
) -> bool {
    let Some(record) = tx
        .public_lane_validators
        .get_mut(&(locator.lane_id, locator.validator.clone()))
    else {
        return false;
    };
    let should_update = matches!(
        record.status,
        PublicLaneValidatorStatus::Active | PublicLaneValidatorStatus::PendingActivation(_)
    );
    if !should_update {
        return false;
    }
    #[cfg(feature = "telemetry")]
    let previous_status = Some(record.status.clone());
    record.status = PublicLaneValidatorStatus::Jailed(reason.to_string());
    #[cfg(feature = "telemetry")]
    if let Some(t) = telemetry {
        t.record_public_lane_validator_status(
            locator.lane_id,
            previous_status.as_ref(),
            &record.status,
        );
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
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
                HeightContext, HeightContextId, PayloadEncoding, QuorumCertificate, ValidatorPower,
                VrfCommit, finality::V2FinalityArtifact,
            },
        },
        consensus::{VrfCommitProof, VrfParticipantRecord},
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
        let block: Arc<SignedBlock> = Arc::new(committed.into());
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
    fn install_npos_boundary_artifact(
        state: &State,
        roster: &[PeerId],
        boundary_height: u64,
    ) -> HeightContext {
        let roster_keys = roster_keys();
        assert_eq!(
            roster,
            roster_keys
                .iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect::<Vec<_>>()
        );
        let signing_key = &roster_keys[0];
        let mut parent = None;
        let mut tip = None;
        for height in 1..=boundary_height {
            let committed =
                ValidBlock::new_dummy_and_modify_header(signing_key.private_key(), |header| {
                    header.set_height(NonZeroU64::new(height).expect("non-zero height"));
                    header.set_prev_block_hash(parent);
                    header.merkle_root = None;
                })
                .commit_unchecked()
                .unpack(|_| {});
            let block: Arc<SignedBlock> = Arc::new(committed.into());
            state
                .kura()
                .store_block(Arc::clone(&block))
                .expect("store canonical NPoS boundary fixture block");
            parent = Some(block.hash());
            tip = Some(block);
        }
        let block = tip.expect("positive boundary height produces a block");
        let powers = roster
            .iter()
            .cloned()
            .map(|validator| ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        let parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"penalties parent context",
            ))),
            height: boundary_height - 1,
            view: 0,
        };
        let parent_block_hash = block
            .header()
            .prev_block_hash()
            .expect("non-genesis boundary has a parent");
        let parent_commit_qc = QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: GlobalPhase::Commit,
            subject: BlockSubject {
                parent_block_hash: None,
                block_hash: parent_block_hash,
                payload_hash: Hash::new(b"penalties parent payload"),
            },
            execution_commitment: ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"penalties parent state"),
                Hash::new(b"penalties parent post state"),
                Hash::new(b"penalties parent ordinary writes"),
                1,
                Hash::new(b"penalties parent executed wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x33; 48],
        };
        let validator_set_pops = roster_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect::<Vec<_>>();
        let mut context = HeightContext {
            network_id: state.network_id_ref().clone(),
            protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            height: boundary_height,
            epoch: 0,
            epoch_end_height: boundary_height,
            next_epoch_snapshot: None,
            mode: V2ConsensusMode::Npos,
            parent_commit_qc: Some(parent_commit_qc),
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&powers).expect("valid fixture quorum"),
            roster: powers,
            nexus_amx_context_hash: Hash::new(b"penalties NPoS boundary nexus"),
            execution_policy_hash: Hash::new(b"penalties NPoS boundary policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x42; 32],
        };
        context.next_epoch_snapshot = Some(
            iroha_data_model::block::consensus_v2::finality::FinalizedNextEpochSnapshot {
                epoch: 1,
                epoch_end_height: boundary_height + 10,
                mode: V2ConsensusMode::Npos,
                roster: context.roster.clone(),
                validator_set_pops: validator_set_pops.clone(),
                quorum: context.quorum,
                leader_seed: [0x43; 32],
            },
        );
        context.validate().expect("valid NPoS boundary context");
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal wire"),
        };
        let executed_block_wire = block
            .encode_wire()
            .expect("encode canonical executed block wire");
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"penalties boundary parent state"),
            Hash::new(b"penalties boundary post state"),
            Hash::new(b"penalties boundary ordinary writes"),
            u64::try_from(executed_block_wire.len()).expect("canonical wire length fits u64"),
            Hash::new(&executed_block_wire),
        );
        let round = ConsensusRound {
            context_id: context.id(),
            height: boundary_height,
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
            .expect("valid boundary finality signer");
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
                .expect("aggregate boundary CommitQC");
        let _commit_receipt = state
            .kura()
            .store_v2_finality_artifact(&V2FinalityArtifact::new(
                context.clone(),
                subject,
                certificate,
                validator_set_pops,
            ))
            .expect("persist canonical NPoS boundary finality artifact");
        context
    }
    fn finalized_non_reveal_record(
        context: &HeightContext,
        signer_key: &KeyPair,
        signer: ValidatorIndex,
    ) -> VrfEpochRecord {
        let mut commit = VrfCommit {
            epoch: context.epoch,
            commitment: [0xA4; 32],
            signer,
            bls_sig: Vec::new(),
        };
        commit.bls_sig = Signature::try_new(
            signer_key.private_key(),
            &crate::sumeragi::consensus::v2_vrf_commit_preimage(
                &context.network_id,
                crate::sumeragi::consensus::NPOS_TAG,
                &commit,
            ),
        )
        .expect("sign fixture VRF commitment")
        .payload()
        .to_vec();
        VrfEpochRecord {
            epoch: context.epoch,
            seed: context.leader_seed,
            epoch_length: context.epoch_end_height,
            commit_deadline_offset: 3,
            reveal_deadline_offset: 6,
            roster_len: u32::try_from(context.roster.len()).expect("small fixture roster"),
            finalized: true,
            updated_at_height: context.height,
            participants: vec![VrfParticipantRecord {
                signer,
                commitment: Some(commit.commitment),
                reveal: None,
                commit_proof: Some(VrfCommitProof {
                    epoch: commit.epoch,
                    commitment: commit.commitment,
                    signer,
                    signature: commit.bls_sig,
                    observed_at_height: 2,
                }),
                reveal_proof: None,
                last_updated_height: 2,
            }],
            late_reveals: Vec::new(),
            committed_no_reveal: vec![signer],
            no_participation: (0..u32::try_from(context.roster.len())
                .expect("small fixture roster"))
                .filter(|index| *index != signer)
                .collect(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        }
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
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
            consensus_admitted_at_height: Some(recorded_at_height),
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
    #[test]
    fn consensus_penalty_delay_does_not_saturate_into_early_eligibility() {
        assert!(consensus_penalty_is_due(u64::MAX, 0, u64::MAX));
        assert!(consensus_penalty_is_due(u64::MAX - 1, 1, u64::MAX));
        assert!(!consensus_penalty_is_due(u64::MAX, 1, u64::MAX));
        assert!(!consensus_penalty_is_due(u64::MAX - 1, 2, u64::MAX));
    }
    #[test]
    fn canonical_artifact_context_is_the_only_roster_authority() {
        let state = fresh_state();
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let mutable_fallback = PeerId::new(checked_keypair().public_key().clone());
        set_commit_topology(&state, vec![mutable_fallback.clone()]);
        let evidence = phase_vote_evidence(&context, 1, 99);
        let resolved = height_context_for_evidence(&state, &evidence, 1)
            .expect("Kura lookup succeeds")
            .expect("canonical artifact exists");
        assert_eq!(resolved, context);
        assert_eq!(
            resolved
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>(),
            frozen_roster
        );
        assert!(
            resolved
                .roster
                .iter()
                .all(|entry| entry.validator != mutable_fallback)
        );
        assert_eq!(offender_indices(&evidence, 1, &resolved), vec![1]);
    }
    #[test]
    fn quorum_certified_signed_non_reveal_jails_only_the_committer() {
        let state = fresh_state();
        let frozen_roster = roster();
        let context = install_npos_boundary_artifact(&state, &frozen_roster, 10);
        let keys = roster_keys();
        let offender = &frozen_roster[0];
        let offender_account = add_validator_record(&state, offender);
        let merely_absent = &frozen_roster[1];
        let absent_account = add_validator_record(&state, merely_absent);
        let record = finalized_non_reveal_record(&context, &keys[0], 0);
        let mut epochs = state.world.vrf_epochs.block();
        epochs.insert(record.epoch, record);
        epochs.commit();
        let actions = PenaltyApplier::from_parts(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_penalty_actions(11)
        .expect("verified boundary finality deterministically authorizes the penalty");
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::VrfJail(jail)
                if jail.epoch == 0
                    && jail.signer == 0
                    && jail.peer_id == *offender
                    && jail.validator == offender_account
                    && jail.reason == "vrf_penalty_epoch_1"
        )));
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::MarkVrfPenaltiesApplied(mark)
                if mark.epoch == 0 && mark.height == 11
        )));
        assert!(
            !actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::VrfJail(jail)
                    if jail.peer_id == *merely_absent || jail.validator == absent_account
            )),
            "unsigned network absence must not become attributable jail evidence"
        );
    }
    #[test]
    fn vrf_penalty_ignores_singleton_non_owner_shared_dataspace_projection() {
        let mut state = fresh_state();
        enable_shared_public_staking_lanes(&mut state);
        let frozen_roster = roster();
        let context = install_npos_boundary_artifact(&state, &frozen_roster, 10);
        let keys = roster_keys();
        let offender = &frozen_roster[0];
        let offender_account = add_validator_record_on_lane(&state, LaneId::new(1), offender);
        let record = finalized_non_reveal_record(&context, &keys[0], 0);
        let mut epochs = state.world.vrf_epochs.block();
        epochs.insert(record.epoch, record);
        epochs.commit();

        let actions = PenaltyApplier::from_parts(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_vrf_penalty_actions(11)
        .expect("verified boundary finality remains processable");

        assert!(
            !actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::VrfJail(jail)
                    if jail.peer_id == *offender || jail.validator == offender_account
            )),
            "a non-owner compatibility projection must not receive a VRF jail action"
        );
        assert!(actions.iter().any(|action| matches!(
            action,
            NposPenaltyAction::MarkVrfPenaltiesApplied(mark)
                if mark.epoch == 0 && mark.height == 11
        )));
    }

    #[test]
    fn finalized_vrf_record_without_canonical_finality_fails_closed() {
        let state = fresh_state();
        let frozen_roster = roster();
        let mut context = height_one_context(
            state.network_id_ref().clone(),
            &frozen_roster,
            test_block_hash(0x91),
        );
        context.height = 10;
        context.epoch_end_height = 10;
        context.mode = V2ConsensusMode::Npos;
        let record = VrfEpochRecord {
            epoch: 0,
            seed: context.leader_seed,
            epoch_length: 10,
            commit_deadline_offset: 3,
            reveal_deadline_offset: 6,
            roster_len: 4,
            finalized: true,
            updated_at_height: 10,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: vec![0, 1, 2, 3],
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };
        let mut epochs = state.world.vrf_epochs.block();
        epochs.insert(record.epoch, record);
        epochs.commit();
        let error = PenaltyApplier::from_parts(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_penalty_actions(11)
        .expect_err("an unauthenticated absence partition must not be marked or punished");
        assert!(error.to_string().contains("boundary finality artifact"));
    }
    #[test]
    fn missing_artifact_fails_closed_without_mutable_topology_fallback() {
        let state = fresh_state();
        install_one_block_delay_npos(&state);
        let frozen_roster = roster();
        set_commit_topology(&state, frozen_roster.clone());
        let context = height_one_context(
            *state.network_id_ref(),
            &frozen_roster,
            test_block_hash(0x81),
        );
        let evidence = phase_vote_evidence(&context, 1, 0);
        let error = height_context_for_evidence(&state, &evidence, 1)
            .expect_err("missing canonical provenance must stop derivation");
        assert!(
            error
                .to_string()
                .contains("missing canonical Sumeragi v2 finality artifact")
        );
        insert_evidence(&state, evidence, 1);
        let applier = PenaltyApplier::from_parts(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        assert!(
            applier
                .derive_npos_consensus_effects(2, std::iter::empty())
                .is_err(),
            "a validator missing canonical evidence provenance must not derive a block"
        );
    }
    #[test]
    fn future_dated_evidence_is_rejected_before_history_lookup() {
        let state = fresh_state();
        let frozen_roster = roster();
        let mut context = height_one_context(
            *state.network_id_ref(),
            &frozen_roster,
            test_block_hash(0x82),
        );
        context.height = 2;
        context.epoch_end_height = 2;
        let evidence = phase_vote_evidence(&context, 0, 0);
        assert!(
            height_context_for_evidence(&state, &evidence, 1)
                .expect("future evidence rejection is deterministic")
                .is_none()
        );
    }
    #[test]
    fn artifact_from_another_chain_fails_closed() {
        let state = fresh_state();
        let frozen_roster = roster();
        let context = install_height_one_artifact_with_network(
            &state,
            &frozen_roster,
            crate::sumeragi::synthetic_network_id("wrong-genesis"),
        );
        let error = height_context_for_evidence(&state, &phase_vote_evidence(&context, 0, 0), 1)
            .expect_err("cross-chain provenance must never authorize a slash");
        assert!(error.to_string().contains("belongs to another chain"));
    }
    #[test]
    fn corrupt_finality_artifact_propagates_a_fail_closed_error() {
        let state = fresh_state();
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        state
            .kura()
            .overwrite_v2_finality_bytes_for_tests(1, b"not a Norito finality artifact")
            .expect("corrupt test artifact");
        let error = height_context_for_evidence(&state, &phase_vote_evidence(&context, 0, 0), 1)
            .expect_err("corrupt canonical provenance must stop derivation");
        assert!(
            error
                .to_string()
                .contains("failed to read Sumeragi v2 finality artifact at height 1")
        );
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
        let actions = PenaltyApplier::from_parts(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(2, std::iter::empty())
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

        let actions = PenaltyApplier::from_parts(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(2, std::iter::empty())
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
            !actions.iter().any(|action| matches!(
                action,
                NposPenaltyAction::MarkConsensusEvidenceApplied(mark)
                    if mark.evidence_key == key
            )),
            "unresolved evidence must remain pending instead of being marked applied"
        );
    }
}
