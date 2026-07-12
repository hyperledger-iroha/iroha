//! Penalty enforcement for `NPoS`: VRF non-participation and consensus evidence slashing.

use std::collections::{BTreeMap, BTreeSet};

use eyre::{Result, eyre};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    block::{
        consensus::{Evidence, EvidencePayload, EvidenceRecord},
        consensus_v2::ConsensusMode,
    },
    consensus::{
        NposConsensusEffects, NposConsensusSlashAction, NposMarkConsensusEvidenceAppliedAction,
        NposMarkVrfPenaltiesAppliedAction, NposPenaltyAction, Qc, ValidatorSetCheckpoint,
        VrfEpochRecord,
    },
    nexus::{DataSpaceCatalog, LaneId, PublicLaneValidatorStatus},
    parameter::system::SumeragiNposParameters,
    prelude::{AccountId, PeerId},
    transaction::TransactionSubmissionReceipt,
};
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;

use super::{EpochScheduleSnapshot, NposEpochParams};
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

pub struct PenaltyApplier<'a> {
    state: &'a State,
    consensus_mode: ConsensusMode,
}

impl<'a> PenaltyApplier<'a> {
    pub(crate) fn from_committed_state(
        state: &'a State,
        consensus_mode: ConsensusMode,
        #[cfg(feature = "telemetry")] _telemetry: Option<&'a StateTelemetry>,
        #[cfg(not(feature = "telemetry"))] _telemetry: Option<()>,
    ) -> Self {
        Self {
            state,
            consensus_mode,
        }
    }

    fn epoch_params_from_committed_state(
        &self,
        world: &impl WorldReadOnly,
    ) -> Result<NposEpochParams> {
        if let Some(params) = world.sumeragi_npos_parameters() {
            let commit_deadline_offset = params.vrf_commit_window_blocks();
            return Ok(NposEpochParams {
                epoch_length_blocks: params.epoch_length_blocks(),
                commit_deadline_offset,
                reveal_deadline_offset: commit_deadline_offset
                    .saturating_add(params.vrf_reveal_window_blocks()),
            });
        }
        if self.consensus_mode == ConsensusMode::Permissioned {
            let params = SumeragiNposParameters::default();
            let commit_deadline_offset = params.vrf_commit_window_blocks();
            return Ok(NposEpochParams {
                epoch_length_blocks: params.epoch_length_blocks(),
                commit_deadline_offset,
                reveal_deadline_offset: commit_deadline_offset
                    .saturating_add(params.vrf_reveal_window_blocks()),
            });
        }
        Err(eyre!(
            "authoritative v2 NPoS penalties require committed sumeragi_npos_parameters"
        ))
    }

    fn slashing_delay_from_committed_state(&self, world: &impl WorldReadOnly) -> Result<u64> {
        if let Some(params) = world.sumeragi_npos_parameters() {
            return Ok(params.slashing_delay_blocks());
        }
        if self.consensus_mode == ConsensusMode::Permissioned {
            return Ok(SumeragiNposParameters::default().slashing_delay_blocks());
        }
        Err(eyre!(
            "authoritative v2 NPoS penalties require committed sumeragi_npos_parameters"
        ))
    }

    fn build_validator_locator_map(&self) -> BTreeMap<PublicKey, ValidatorLocator> {
        let world = self.state.world_view();
        let nexus_enabled = self.state.nexus_snapshot().enabled;
        let mut candidates_map: BTreeMap<PublicKey, Vec<ValidatorLocator>> = BTreeMap::new();

        for (key, record) in world.public_lane_validators().iter() {
            if !public_lane_validator_record_matches_key(key, record) {
                continue;
            }
            let (lane_id, validator_id) = key;
            if nexus_enabled && !self.state.is_lane_active_for_authority(*lane_id) {
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

    /// Derive only the deterministic penalty actions from pre-block state.
    ///
    /// This deliberately does not inspect node-local pending admission
    /// candidates and is therefore safe for follower-side candidate checks.
    ///
    /// # Errors
    ///
    /// Returns an error when a due consensus slash amount cannot be derived
    /// from the committed staking state.
    pub(crate) fn derive_npos_penalty_actions(
        &self,
        current_height: u64,
    ) -> Result<Vec<NposPenaltyAction>> {
        let mut actions = self.derive_vrf_penalty_actions(current_height);
        actions.extend(self.derive_consensus_penalty_actions(current_height)?);
        actions.sort();
        actions.dedup();
        Ok(actions)
    }

    fn derive_vrf_penalty_actions(&self, current_height: u64) -> Vec<NposPenaltyAction> {
        let view = self.state.world.vrf_epochs.view();
        let mut due_records: Vec<VrfEpochRecord> = Vec::new();
        for (_epoch, record) in view.iter() {
            if !record.finalized || record.penalties_applied {
                continue;
            }
            // A boundary record becomes pre-state only at the next height.
            // Its absence sets are proposer observations, not quorum-certified
            // evidence, so they can never authorize jailing.  Marking the
            // record processed is deterministic and prevents repeated work.
            if record.updated_at_height >= current_height {
                continue;
            }
            due_records.push(record.clone());
        }
        drop(view);

        if due_records.is_empty() {
            return Vec::new();
        }

        let mut actions = Vec::new();
        for record in due_records {
            actions.push(NposPenaltyAction::MarkVrfPenaltiesApplied(
                NposMarkVrfPenaltiesAppliedAction {
                    epoch: record.epoch,
                    height: current_height,
                },
            ));
        }
        actions
    }

    #[allow(clippy::too_many_lines)]
    fn derive_consensus_penalty_actions(
        &self,
        current_height: u64,
    ) -> Result<Vec<NposPenaltyAction>> {
        let slashing_delay = {
            let world = self.state.world_view();
            self.slashing_delay_from_committed_state(&world)?
        };
        let evidence_view = self.state.world.consensus_evidence.view();
        let mut pending: Vec<(Vec<u8>, EvidenceRecord)> = Vec::new();
        for (key, record) in evidence_view.iter() {
            if record.penalty_applied || record.penalty_cancelled {
                continue;
            }
            if matches!(
                &record.evidence.payload,
                EvidencePayload::SumeragiV2Equivocation(_)
            ) && record
                .consensus_admitted_at_height
                .is_none_or(|height| height >= current_height)
            {
                // Node-local observations and evidence admitted by the block
                // currently under construction can never drive deterministic
                // penalty attachments.
                continue;
            }
            if record.recorded_at_height.saturating_add(slashing_delay) > current_height {
                continue;
            }
            pending.push((key.clone(), record.clone()));
        }
        drop(evidence_view);

        if pending.is_empty() {
            return Ok(Vec::new());
        }

        let epoch_seeds = {
            let view = self.state.world.vrf_epochs.view();
            let mut map = BTreeMap::new();
            for (epoch, record) in view.iter() {
                map.insert(*epoch, record.seed);
            }
            map
        };
        let epoch_schedule = {
            let world = self.state.world_view();
            let epoch_params = self.epoch_params_from_committed_state(&world)?;
            EpochScheduleSnapshot::from_world_with_fallback(
                &world,
                epoch_params.epoch_length_blocks,
            )
        };

        let validator_map = self.build_validator_locator_map();
        let commit_certs = crate::sumeragi::status::commit_qc_history();
        let checkpoints = crate::sumeragi::status::validator_checkpoint_history();
        let mut actions = Vec::new();
        for (key, record) in pending {
            let consensus_mode = consensus_mode_for_evidence(
                self.state,
                &record.evidence,
                record.recorded_at_height,
                self.consensus_mode,
            );
            let is_censorship =
                matches!(&record.evidence.payload, EvidencePayload::Censorship { .. });
            let has_frozen_v2_roster = matches!(
                &record.evidence.payload,
                EvidencePayload::SumeragiV2Equivocation(_)
            );
            let evidence_epoch =
                evidence_epoch(&record.evidence, record.recorded_at_height, &epoch_schedule);
            let prf_seed = match consensus_mode {
                ConsensusMode::Permissioned => None,
                ConsensusMode::Npos => epoch_seeds.get(&evidence_epoch).copied(),
            };
            let evidence_roster =
                roster_for_evidence(self.state, &record.evidence, &commit_certs, &checkpoints)
                    .filter(|roster| !roster.is_empty());
            let Some(roster) = evidence_roster.as_ref() else {
                continue;
            };
            // Legacy NPoS indices require the historical PRF rotation. V2
            // evidence already carries the exact immutable roster whose raw
            // indices authenticated both artifacts, so no mutable seed lookup
            // is needed for attribution.
            if matches!(consensus_mode, ConsensusMode::Npos)
                && !has_frozen_v2_roster
                && prf_seed.is_none()
            {
                continue;
            }
            let roster = roster.as_slice();
            let offenders = offender_indices(
                &record.evidence,
                record.recorded_at_height,
                roster.len(),
                consensus_mode,
                prf_seed,
            );
            if offenders.is_empty() {
                if !is_censorship && evidence_has_legitimate_empty_offenders(&record.evidence) {
                    actions.push(NposPenaltyAction::MarkConsensusEvidenceApplied(
                        NposMarkConsensusEvidenceAppliedAction {
                            evidence_key: key,
                            height: current_height,
                        },
                    ));
                }
                continue;
            }
            let slash_id = Hash::new(key.clone());
            let mut slashes = 0_u64;
            for signer in offenders {
                let Some((peer_id, locator)) =
                    self.locate_validator_in_roster_cached(signer, roster, &validator_map)
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
    dataspace_catalog: &DataSpaceCatalog,
    staking_cfg: &iroha_config::parameters::actual::NexusStaking,
    current_height: u64,
    current_view: u64,
    now_ms: u64,
    #[cfg(feature = "telemetry")] telemetry: Option<&StateTelemetry>,
    #[cfg(not(feature = "telemetry"))] telemetry: Option<&crate::telemetry::StateTelemetry>,
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
                    &mut tx.world,
                    dataspace_catalog,
                    staking_cfg,
                    action.lane_id,
                    &action.validator,
                    action.slash_id,
                    &action.amount,
                    now_ms,
                    telemetry,
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

fn roster_for_evidence(
    state: &State,
    evidence: &Evidence,
    commit_certs: &[Qc],
    checkpoints: &[ValidatorSetCheckpoint],
) -> Option<Vec<PeerId>> {
    if let EvidencePayload::SumeragiV2Equivocation(evidence) = &evidence.payload {
        return Some(
            evidence
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect(),
        );
    }
    let refs = super::evidence::evidence_block_refs(evidence);
    if refs.is_empty() {
        let roster = state.commit_topology_snapshot();
        if !roster.is_empty() {
            return Some(roster);
        }
    }
    for (height, hash) in refs {
        if let Some(snapshot) = state.commit_roster_snapshot_for_block(height, hash) {
            let roster = snapshot.validator_checkpoint.validator_set;
            if !roster.is_empty() {
                return Some(roster);
            }
        }
        if let Some(cert) = commit_certs
            .iter()
            .find(|cert| cert.height == height && cert.subject_block_hash == hash)
        {
            if !cert.validator_set.is_empty() {
                return Some(cert.validator_set.clone());
            }
        }
        if let Some(checkpoint) = checkpoints
            .iter()
            .find(|checkpoint| checkpoint.height == height && checkpoint.block_hash == hash)
        {
            if !checkpoint.validator_set.is_empty() {
                return Some(checkpoint.validator_set.clone());
            }
        }
    }
    None
}

fn consensus_mode_for_evidence(
    state: &State,
    evidence: &Evidence,
    recorded_at_height: u64,
    fallback: ConsensusMode,
) -> ConsensusMode {
    if let EvidencePayload::SumeragiV2Equivocation(evidence) = &evidence.payload {
        return match evidence.context.mode {
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned => {
                ConsensusMode::Permissioned
            }
            iroha_data_model::block::consensus_v2::ConsensusMode::Npos => ConsensusMode::Npos,
        };
    }
    let (subject_height, _) = super::evidence::evidence_subject_height_view(evidence);
    let height = subject_height.unwrap_or(recorded_at_height);
    let world = state.world_view();
    crate::sumeragi::effective_consensus_mode_for_height_from_world(&world, height, fallback)
}

fn npos_leader_index(seed: [u8; 32], height: u64, view: u64, topology_len: usize) -> Option<usize> {
    if topology_len == 0 {
        return None;
    }
    let slot = usize::try_from(view % u64::try_from(topology_len).ok()?).ok()?;
    Some(
        npos_shuffled_indices(seed, height, topology_len)
            .get(slot)
            .copied()?,
    )
}

fn npos_shuffled_indices(seed: [u8; 32], height: u64, len: usize) -> Vec<usize> {
    let mut slots: Vec<usize> = (0..len).collect();
    let mut shuffled = Vec::with_capacity(len);
    let mut ctr: u64 = 0;
    while !slots.is_empty() {
        let Some(pos) = npos_shuffle_prf_slot(seed, height, ctr, slots.len()) else {
            break;
        };
        shuffled.push(slots.swap_remove(pos));
        ctr = ctr.saturating_add(1);
    }
    shuffled
}

fn npos_shuffle_prf_slot(seed: [u8; 32], height: u64, ctr: u64, modulus: usize) -> Option<usize> {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    if modulus == 0 {
        return None;
    }

    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &seed);
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &height.to_be_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &ctr.to_be_bytes());
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut w = [0u8; 8];
    w.copy_from_slice(&digest[..8]);
    let Ok(modulus) = u128::try_from(modulus) else {
        return None;
    };
    usize::try_from(u128::from(u64::from_be_bytes(w)) % modulus).ok()
}

fn canonicalize_index_for_view(
    signer: ValidatorIndex,
    height: u64,
    view: u64,
    topology_len: usize,
    consensus_mode: ConsensusMode,
    prf_seed: Option<[u8; 32]>,
) -> Option<ValidatorIndex> {
    if topology_len == 0 {
        return None;
    }
    let idx = usize::try_from(signer).ok()?;
    if idx >= topology_len {
        return None;
    }
    let topology_len_u64 = u64::try_from(topology_len).ok()?;
    let rotation = match consensus_mode {
        ConsensusMode::Permissioned => {
            if view == 0 {
                return Some(signer);
            }
            usize::try_from(view % topology_len_u64).ok()?
        }
        ConsensusMode::Npos => {
            let seed = prf_seed?;
            npos_leader_index(seed, height, view, topology_len)?
        }
    };
    let canonical_idx = (idx + rotation) % topology_len;
    ValidatorIndex::try_from(canonical_idx).ok()
}

fn canonicalize_indices_for_view(
    indices: impl IntoIterator<Item = ValidatorIndex>,
    height: u64,
    view: u64,
    topology_len: usize,
    consensus_mode: ConsensusMode,
    prf_seed: Option<[u8; 32]>,
) -> Vec<ValidatorIndex> {
    let mut out = BTreeSet::new();
    for signer in indices {
        if let Some(canonical) = canonicalize_index_for_view(
            signer,
            height,
            view,
            topology_len,
            consensus_mode,
            prf_seed,
        ) {
            out.insert(canonical);
        }
    }
    out.into_iter().collect()
}

fn censorship_anchor_height(
    receipts: &[TransactionSubmissionReceipt],
    recorded_at_height: u64,
) -> Option<u64> {
    let max_receipt_height = receipts
        .iter()
        .map(|receipt| receipt.payload.submitted_at_height)
        .max()?;
    Some(max_receipt_height.min(recorded_at_height))
}

fn evidence_epoch(
    evidence: &Evidence,
    recorded_at_height: u64,
    epoch_schedule: &EpochScheduleSnapshot,
) -> u64 {
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, .. } => v1.epoch,
        EvidencePayload::InvalidProposal { proposal, .. } => proposal.header.epoch,
        EvidencePayload::InvalidQc { certificate, .. } => certificate.epoch,
        EvidencePayload::Censorship { receipts, .. } => {
            let Some(anchor) = censorship_anchor_height(receipts, recorded_at_height) else {
                return 0;
            };
            epoch_schedule.epoch_for_height(anchor)
        }
        EvidencePayload::SumeragiV2Equivocation(evidence) => evidence.context.epoch,
    }
}

fn offender_indices(
    evidence: &Evidence,
    recorded_at_height: u64,
    topology_len: usize,
    consensus_mode: ConsensusMode,
    prf_seed: Option<[u8; 32]>,
) -> Vec<ValidatorIndex> {
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, .. } => canonicalize_indices_for_view(
            [v1.signer],
            v1.height,
            v1.view,
            topology_len,
            consensus_mode,
            prf_seed,
        ),
        EvidencePayload::InvalidProposal { proposal, .. } => canonicalize_indices_for_view(
            [proposal.header.proposer],
            proposal.header.height,
            proposal.header.view,
            topology_len,
            consensus_mode,
            prf_seed,
        ),
        EvidencePayload::InvalidQc { certificate, .. } => canonicalize_indices_for_view(
            bitmap_indices(&certificate.aggregate.signers_bitmap),
            certificate.height,
            certificate.view,
            topology_len,
            consensus_mode,
            prf_seed,
        ),
        EvidencePayload::Censorship { receipts, .. } => {
            let Some(anchor) = censorship_anchor_height(receipts, recorded_at_height) else {
                return Vec::new();
            };
            canonicalize_indices_for_view([0], anchor, 0, topology_len, consensus_mode, prf_seed)
        }
        EvidencePayload::SumeragiV2Equivocation(evidence) => {
            let signer = match &evidence.conflict {
                iroha_data_model::block::consensus_v2::SumeragiV2Equivocation::Proposal {
                    first,
                    ..
                } => first.proposer,
                iroha_data_model::block::consensus_v2::SumeragiV2Equivocation::PhaseVote {
                    first,
                    ..
                } => first.signer,
                iroha_data_model::block::consensus_v2::SumeragiV2Equivocation::TimeoutVote {
                    first,
                    ..
                } => first.signer,
            };
            usize::try_from(signer)
                .ok()
                .filter(|index| *index < topology_len)
                .map(|_| vec![signer])
                .unwrap_or_default()
        }
    }
}

fn evidence_has_legitimate_empty_offenders(evidence: &Evidence) -> bool {
    match &evidence.payload {
        EvidencePayload::InvalidQc { certificate, .. } => {
            bitmap_indices(&certificate.aggregate.signers_bitmap).is_empty()
        }
        EvidencePayload::Censorship { .. }
        | EvidencePayload::DoubleVote { .. }
        | EvidencePayload::InvalidProposal { .. }
        | EvidencePayload::SumeragiV2Equivocation(_) => false,
    }
}

fn bitmap_indices(bitmap: &[u8]) -> Vec<ValidatorIndex> {
    let mut indices = Vec::new();
    for (byte_idx, byte) in bitmap.iter().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                if let Ok(idx) = u32::try_from(byte_idx * 8 + bit) {
                    indices.push(idx);
                }
            }
        }
    }
    indices
}

fn max_slash_amount_for_validator_from_state(
    state: &State,
    locator: &ValidatorLocator,
    max_bps: u16,
) -> Result<Option<Numeric>> {
    if state.nexus_snapshot().enabled && !state.is_lane_active_for_authority(locator.lane_id) {
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
