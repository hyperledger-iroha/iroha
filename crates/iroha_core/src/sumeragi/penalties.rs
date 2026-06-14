//! Penalty enforcement for `NPoS`: VRF non-participation and consensus evidence slashing.

use std::collections::{BTreeMap, BTreeSet};

use eyre::Result;
use iroha_config::parameters::actual::{ConsensusMode, Sumeragi as SumeragiConfig, SumeragiNpos};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    block::consensus::{Evidence, EvidencePayload, EvidenceRecord},
    consensus::{
        NposConsensusEffects, NposConsensusSlashAction, NposMarkConsensusEvidenceAppliedAction,
        NposMarkVrfPenaltiesAppliedAction, NposPenaltyAction, NposVrfJailAction, Qc,
        ValidatorSetCheckpoint, VrfEpochRecord,
    },
    nexus::{DataSpaceCatalog, LaneId, PublicLaneValidatorStatus},
    prelude::{AccountId, PeerId},
    transaction::TransactionSubmissionReceipt,
};
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;

use super::EpochScheduleSnapshot;
#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    smartcontracts::isi::staking::{apply_slash_to_validator, max_slash_amount},
    state::{State, WorldReadOnly, WorldTransaction},
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
    npos_config: &'a SumeragiNpos,
    consensus_mode: ConsensusMode,
}

impl<'a> PenaltyApplier<'a> {
    pub(crate) fn new(
        state: &'a State,
        config: &'a SumeragiConfig,
        #[cfg(feature = "telemetry")] _telemetry: Option<&'a StateTelemetry>,
        #[cfg(not(feature = "telemetry"))] _telemetry: Option<()>,
    ) -> Self {
        Self {
            state,
            npos_config: &config.npos,
            consensus_mode: config.consensus_mode,
        }
    }

    pub(crate) fn from_parts(
        state: &'a State,
        npos_config: &'a SumeragiNpos,
        consensus_mode: ConsensusMode,
        #[cfg(feature = "telemetry")] _telemetry: Option<&'a StateTelemetry>,
        #[cfg(not(feature = "telemetry"))] _telemetry: Option<()>,
    ) -> Self {
        Self {
            state,
            npos_config,
            consensus_mode,
        }
    }

    fn build_validator_locator_map(&self) -> BTreeMap<PublicKey, ValidatorLocator> {
        let world = self.state.world_view();
        let mut candidates_map: BTreeMap<PublicKey, Vec<ValidatorLocator>> = BTreeMap::new();

        for ((lane_id, validator_id), record) in world.public_lane_validators().iter() {
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
            penalty_actions: Vec::new(),
        };
        effects.vrf_epoch_seals.sort_by_key(|record| record.epoch);
        effects.vrf_epoch_seals.dedup_by_key(|record| record.epoch);
        effects
            .penalty_actions
            .extend(self.derive_vrf_penalty_actions(current_height));
        effects
            .penalty_actions
            .extend(self.derive_consensus_penalty_actions(current_height)?);
        effects.penalty_actions.sort();
        effects.penalty_actions.dedup();
        Ok(effects)
    }

    fn derive_vrf_penalty_actions(&self, current_height: u64) -> Vec<NposPenaltyAction> {
        let activation_lag = {
            let world = self.state.world_view();
            crate::sumeragi::resolve_npos_activation_lag_blocks_from_world(&world, self.npos_config)
        };
        let view = self.state.world.vrf_epochs.view();
        let mut due_records: Vec<VrfEpochRecord> = Vec::new();
        for (_epoch, record) in view.iter() {
            if !record.finalized || record.penalties_applied {
                continue;
            }
            if record.updated_at_height.saturating_add(activation_lag) > current_height {
                continue;
            }
            due_records.push(record.clone());
        }
        drop(view);

        if due_records.is_empty() {
            return Vec::new();
        }

        let validator_map = self.build_validator_locator_map();
        let commit_topology = self.state.commit_topology_snapshot();
        let mut actions = Vec::new();
        for record in due_records {
            let offenders: BTreeSet<u32> = record
                .committed_no_reveal
                .iter()
                .chain(record.no_participation.iter())
                .copied()
                .collect();
            let mut all_offenders_mapped = true;
            for signer in offenders.iter().copied() {
                let Some((peer_id, locator)) =
                    Self::locate_validator_cached(signer, &commit_topology, &validator_map)
                else {
                    all_offenders_mapped = false;
                    continue;
                };
                actions.push(NposPenaltyAction::VrfJail(NposVrfJailAction {
                    epoch: record.epoch,
                    signer,
                    peer_id,
                    lane_id: locator.lane_id,
                    validator: locator.validator,
                    reason: format!("vrf_penalty_epoch_{}", record.epoch),
                }));
            }
            if offenders.is_empty() || all_offenders_mapped {
                actions.push(NposPenaltyAction::MarkVrfPenaltiesApplied(
                    NposMarkVrfPenaltiesAppliedAction {
                        epoch: record.epoch,
                        height: current_height,
                    },
                ));
            }
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
            crate::sumeragi::resolve_npos_slashing_delay_blocks_from_world(&world, self.npos_config)
        };
        let evidence_view = self.state.world.consensus_evidence.view();
        let mut pending: Vec<(Vec<u8>, EvidenceRecord)> = Vec::new();
        for (key, record) in evidence_view.iter() {
            if record.penalty_applied || record.penalty_cancelled {
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
            let epoch_params =
                crate::sumeragi::load_npos_epoch_params_from_world(&world, self.npos_config);
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
                matches!(record.evidence.payload, EvidencePayload::Censorship { .. });
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
            if matches!(consensus_mode, ConsensusMode::Npos) && prf_seed.is_none() {
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

    fn locate_validator_cached(
        signer: ValidatorIndex,
        commit_topology: &[PeerId],
        map: &BTreeMap<PublicKey, ValidatorLocator>,
    ) -> Option<(PeerId, ValidatorLocator)> {
        let signer_idx = usize::try_from(signer).ok()?;
        let peer = commit_topology.get(signer_idx)?;
        map.get(peer.public_key())
            .cloned()
            .map(|locator| (peer.clone(), locator))
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
    tx: &mut WorldTransaction<'_, '_>,
    effects: &NposConsensusEffects,
    dataspace_catalog: &DataSpaceCatalog,
    staking_cfg: &iroha_config::parameters::actual::NexusStaking,
    current_height: u64,
    now_ms: u64,
    #[cfg(feature = "telemetry")] telemetry: Option<&StateTelemetry>,
    #[cfg(not(feature = "telemetry"))] telemetry: Option<&crate::telemetry::StateTelemetry>,
) -> Result<PenaltyOutcome> {
    let mut outcome = PenaltyOutcome::default();
    for record in &effects.vrf_epoch_seals {
        tx.vrf_epochs.insert(record.epoch, record.clone());
    }
    for action in &effects.penalty_actions {
        match action {
            NposPenaltyAction::VrfJail(action) => {
                let locator = ValidatorLocator {
                    lane_id: action.lane_id,
                    validator: action.validator.clone(),
                };
                if jail_in_transaction(
                    tx,
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
                apply_slash_to_validator(
                    tx,
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
                let mut record = tx.vrf_epochs.get(&action.epoch).cloned();
                if let Some(record) = record.as_mut() {
                    record.penalties_applied = true;
                    record.penalties_applied_at_height = Some(action.height);
                    tx.vrf_epochs.insert(action.epoch, record.clone());
                }
            }
            NposPenaltyAction::MarkConsensusEvidenceApplied(action) => {
                let mut record = tx.consensus_evidence.get(&action.evidence_key).cloned();
                if let Some(record) = record.as_mut() {
                    record.penalty_applied = true;
                    record.penalty_applied_at_height = Some(action.height);
                    tx.consensus_evidence
                        .insert(action.evidence_key.clone(), record.clone());
                }
            }
        }
    }
    let _ = current_height;
    Ok(outcome)
}

fn roster_for_evidence(
    state: &State,
    evidence: &Evidence,
    commit_certs: &[Qc],
    checkpoints: &[ValidatorSetCheckpoint],
) -> Option<Vec<PeerId>> {
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
    }
}

fn evidence_has_legitimate_empty_offenders(evidence: &Evidence) -> bool {
    match &evidence.payload {
        EvidencePayload::InvalidQc { certificate, .. } => {
            bitmap_indices(&certificate.aggregate.signers_bitmap).is_empty()
        }
        EvidencePayload::Censorship { .. }
        | EvidencePayload::DoubleVote { .. }
        | EvidencePayload::InvalidProposal { .. } => false,
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
    use std::{collections::BTreeSet, num::NonZeroU32, time::Duration};

    use eyre::Result;
    use iroha_config::parameters::actual::{
        AdaptiveObservability, ConsensusMode, NodeRole, ProofPolicy, Sumeragi as SumeragiConfig,
        SumeragiBlock, SumeragiCollectors, SumeragiDa, SumeragiDebug, SumeragiDebugRbc,
        SumeragiFinality, SumeragiGating, SumeragiKeys, SumeragiModeFlip, SumeragiNpos,
        SumeragiNposElection, SumeragiNposReconfig, SumeragiNposTimeoutOverrides, SumeragiNposVrf,
        SumeragiPacemaker, SumeragiPacingGovernor, SumeragiPersistence, SumeragiQueues,
        SumeragiRbc, SumeragiRecovery, SumeragiResilience, SumeragiVNext, SumeragiWorker,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        Registrable,
        account::{AccountDetails, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::{Evidence, EvidenceKind, EvidencePayload, EvidenceRecord},
        common::Owned,
        consensus::{Qc, ValidatorSetCheckpoint, VrfEpochRecord},
        domain::Domain,
        nexus::{LaneCatalog, LaneConfig},
        parameter::system::SumeragiConsensusMode,
        prelude::{BlockHeader, DomainId, PeerId},
        transaction::{TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload},
    };
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        sumeragi::{
            consensus::{PERMISSIONED_TAG, Phase, QcAggregate, Vote},
            evidence::evidence_key,
        },
        telemetry::StateTelemetry,
    };

    fn fresh_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::with_telemetry(World::default(), kura, query, StateTelemetry::default())
    }

    #[allow(clippy::too_many_lines)]
    fn test_sumeragi_config() -> SumeragiConfig {
        SumeragiConfig {
            role: NodeRole::Validator,
            consensus_mode: ConsensusMode::Npos,
            mode_flip: SumeragiModeFlip {
                enabled: iroha_config::parameters::defaults::sumeragi::MODE_FLIP_ENABLED,
            },
            collectors: SumeragiCollectors {
                k: 1,
                redundant_send_r: 1,
                parallel_topology_fanout: 0,
            },
            block: SumeragiBlock {
                max_transactions: None,
                max_ivm_transactions:
                    iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_IVM_TRANSACTIONS,
                fast_finality_max_transactions:
                    iroha_config::parameters::defaults::sumeragi::FAST_FINALITY_MAX_TRANSACTIONS,
                fast_gas_limit_per_block:
                    iroha_config::parameters::defaults::sumeragi::FAST_FINALITY_GAS_LIMIT_PER_BLOCK,
                max_payload_bytes: None,
                proposal_queue_scan_multiplier:
                    iroha_config::parameters::defaults::sumeragi::PROPOSAL_QUEUE_SCAN_MULTIPLIER,
            },
            queues: SumeragiQueues {
                votes: iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_VOTES,
                block_payload:
                    iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_BLOCK_PAYLOAD,
                rbc_chunks: iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_RBC_CHUNKS,
                blocks: iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_BLOCKS,
                control: iroha_config::parameters::defaults::sumeragi::CONTROL_MSG_CHANNEL_CAP,
            },
            worker: SumeragiWorker {
                iteration_budget_cap: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::WORKER_ITERATION_BUDGET_CAP_MS,
                ),
                iteration_drain_budget_cap: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::WORKER_ITERATION_DRAIN_BUDGET_CAP_MS,
                ),
                tick_work_budget_cap: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::WORKER_TICK_WORK_BUDGET_CAP_MS,
                ),
                parallel_ingress:
                    iroha_config::parameters::defaults::sumeragi::WORKER_PARALLEL_INGRESS,
                validation_worker_threads:
                    iroha_config::parameters::defaults::sumeragi::VALIDATION_WORKER_THREADS,
                validation_work_queue_cap:
                    iroha_config::parameters::defaults::sumeragi::VALIDATION_WORK_QUEUE_CAP,
                validation_result_queue_cap:
                    iroha_config::parameters::defaults::sumeragi::VALIDATION_RESULT_QUEUE_CAP,
                validation_queue_full_inline_cutover_divisor:
                    iroha_config::parameters::defaults::sumeragi::
                        VALIDATION_QUEUE_FULL_INLINE_CUTOVER_DIVISOR,
                fast_finality_inline_validation_max_transactions:
                    iroha_config::parameters::defaults::sumeragi::
                        VALIDATION_FAST_FINALITY_INLINE_MAX_TRANSACTIONS,
                validation_stall_da_per_entrypoint_floor: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::
                        VALIDATION_STALL_DA_PER_ENTRYPOINT_FLOOR_MS,
                ),
                validation_stall_inline_fallback_multiplier:
                    iroha_config::parameters::defaults::sumeragi::
                        VALIDATION_STALL_INLINE_FALLBACK_MULTIPLIER,
                validation_stall_ema_multiplier: iroha_config::parameters::defaults::sumeragi::
                    VALIDATION_STALL_EMA_MULTIPLIER,
                validation_stall_non_da_cap: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::VALIDATION_STALL_NON_DA_CAP_MS,
                ),
                validation_stall_da_cap: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::VALIDATION_STALL_DA_CAP_MS,
                ),
                qc_verify_worker_threads:
                    iroha_config::parameters::defaults::sumeragi::QC_VERIFY_WORKER_THREADS,
                qc_verify_work_queue_cap:
                    iroha_config::parameters::defaults::sumeragi::QC_VERIFY_WORK_QUEUE_CAP,
                qc_verify_result_queue_cap:
                    iroha_config::parameters::defaults::sumeragi::QC_VERIFY_RESULT_QUEUE_CAP,
                validation_pending_cap:
                    iroha_config::parameters::defaults::sumeragi::VALIDATION_PENDING_CAP,
                vote_burst_cap_with_payload_backlog:
                    iroha_config::parameters::defaults::sumeragi::
                        WORKER_VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
                max_urgent_before_da_critical: iroha_config::parameters::defaults::sumeragi::
                    WORKER_MAX_URGENT_BEFORE_DA_CRITICAL,
            },
            pacemaker: SumeragiPacemaker {
                backoff_multiplier: 1,
                rtt_floor_multiplier: 1,
                max_backoff: Duration::from_secs(0),
                jitter_frac_permille: 0,
                pending_stall_grace: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::PACEMAKER_PENDING_STALL_GRACE_MS,
                ),
                da_fast_reschedule:
                    iroha_config::parameters::defaults::sumeragi::PACEMAKER_DA_FAST_RESCHEDULE,
                active_pending_soft_limit:
                    iroha_config::parameters::defaults::sumeragi::PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT,
                rbc_backlog_session_soft_limit: iroha_config::parameters::defaults::sumeragi::
                    PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT,
                rbc_backlog_chunk_soft_limit: iroha_config::parameters::defaults::sumeragi::
                    PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT,
            },
            pacing_governor: SumeragiPacingGovernor::default(),
            da: SumeragiDa {
                enabled: false,
                quorum_timeout_multiplier:
                    iroha_config::parameters::defaults::sumeragi::DA_QUORUM_TIMEOUT_MULTIPLIER,
                availability_timeout_multiplier: iroha_config::parameters::defaults::sumeragi::
                    DA_AVAILABILITY_TIMEOUT_MULTIPLIER,
                availability_timeout_floor: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::DA_AVAILABILITY_TIMEOUT_FLOOR_MS,
                ),
                max_commitments_per_block: 0,
                max_proof_openings_per_block: 0,
            },
            persistence: SumeragiPersistence {
                kura_retry_interval: Duration::from_millis(1),
                kura_retry_max_attempts: 1,
                commit_inflight_timeout: Duration::from_millis(5_000),
                commit_work_queue_cap:
                    iroha_config::parameters::defaults::sumeragi::COMMIT_WORK_QUEUE_CAP,
                commit_result_queue_cap:
                    iroha_config::parameters::defaults::sumeragi::COMMIT_RESULT_QUEUE_CAP,
            },
            recovery: SumeragiRecovery {
                height_attempt_cap:
                    iroha_config::parameters::defaults::sumeragi::RECOVERY_HEIGHT_ATTEMPT_CAP,
                height_window: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::RECOVERY_HEIGHT_WINDOW_MS,
                ),
                hash_miss_cap_before_range_pull:
                    iroha_config::parameters::defaults::sumeragi::RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL,
                missing_qc_reacquire_window: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS,
                ),
                max_forced_proposal_attempts_per_view:
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_MAX_FORCED_PROPOSAL_ATTEMPTS_PER_VIEW,
                rotate_after_reacquire_exhausted:
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_ROTATE_AFTER_REACQUIRE_EXHAUSTED,
                missing_block_signer_fallback_attempts:
                    iroha_config::parameters::defaults::sumeragi::MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS,
                missing_block_retry_backoff_multiplier: iroha_config::parameters::defaults::
                    sumeragi::RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER,
                missing_block_retry_backoff_cap: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS,
                ),
                view_change_backlog_extension_factor:
                    iroha_config::parameters::defaults::sumeragi::VIEW_CHANGE_BACKLOG_EXTENSION_FACTOR,
                view_change_backlog_extension_cap: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::VIEW_CHANGE_BACKLOG_EXTENSION_CAP_MS,
                ),
                deferred_qc_ttl: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::DEFERRED_QC_TTL_MS,
                ),
                missing_block_height_attempt_cap:
                    iroha_config::parameters::defaults::sumeragi::MISSING_BLOCK_HEIGHT_ATTEMPT_CAP,
                missing_block_height_ttl: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::MISSING_BLOCK_HEIGHT_TTL_MS,
                ),
                sidecar_mismatch_retry_cap:
                    iroha_config::parameters::defaults::sumeragi::SIDECAR_MISMATCH_RETRY_CAP,
                sidecar_mismatch_ttl: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::SIDECAR_MISMATCH_TTL_MS,
                ),
                range_pull_escalation_after_hash_misses:
                    iroha_config::parameters::defaults::sumeragi::RANGE_PULL_ESCALATION_AFTER_HASH_MISSES,
                missing_request_stale_height_margin:
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_MISSING_REQUEST_STALE_HEIGHT_MARGIN,
                pending_block_sync_cap:
                    iroha_config::parameters::defaults::sumeragi::RECOVERY_PENDING_BLOCK_SYNC_CAP,
                pending_proposal_cap:
                    iroha_config::parameters::defaults::sumeragi::RECOVERY_PENDING_PROPOSAL_CAP,
                missing_fetch_aggressive_after_attempts:
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_MISSING_FETCH_AGGRESSIVE_AFTER_ATTEMPTS,
                authoritative_body_ingress_fetch_grace: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_AUTHORITATIVE_BODY_INGRESS_FETCH_GRACE_MS,
                ),
                exact_body_fetch_retry_floor: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::
                        RECOVERY_EXACT_BODY_FETCH_RETRY_FLOOR_MS,
                ),
            },
            fanout: iroha_config::parameters::actual::SumeragiFanout {
                large_set_threshold:
                    iroha_config::parameters::defaults::sumeragi::FANOUT_LARGE_SET_THRESHOLD,
                activity_lookback_blocks:
                    iroha_config::parameters::defaults::sumeragi::FANOUT_ACTIVITY_LOOKBACK_BLOCKS,
            },
            gating: SumeragiGating {
                future_height_window:
                    iroha_config::parameters::defaults::sumeragi::CONSENSUS_FUTURE_HEIGHT_WINDOW,
                future_view_window:
                    iroha_config::parameters::defaults::sumeragi::CONSENSUS_FUTURE_VIEW_WINDOW,
                invalid_sig_penalty_threshold:
                    iroha_config::parameters::defaults::sumeragi::INVALID_SIG_PENALTY_THRESHOLD,
                invalid_sig_penalty_window: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::INVALID_SIG_PENALTY_WINDOW_MS,
                ),
                invalid_sig_penalty_cooldown: Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::INVALID_SIG_PENALTY_COOLDOWN_MS,
                ),
                membership_mismatch_alert_threshold:
                    iroha_config::parameters::defaults::sumeragi::MEMBERSHIP_MISMATCH_ALERT_THRESHOLD,
                membership_mismatch_fail_closed:
                    iroha_config::parameters::defaults::sumeragi::MEMBERSHIP_MISMATCH_FAIL_CLOSED,
            },
            rbc: SumeragiRbc {
                chunk_max_bytes: 0,
                encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
                data_shards: 0,
                parity_shards: 0,
                chunk_fanout: None,
                rs16_initial_fanout: iroha_config::parameters::actual::RbcRs16InitialFanout::Full,
                pending_max_chunks: 0,
                pending_max_bytes: 0,
                pending_session_limit: 0,
                pending_ttl: Duration::from_secs(0),
                session_ttl: Duration::from_secs(0),
                rebroadcast_sessions_per_tick: 1,
                payload_chunks_per_tick: 1,
                inline_block_created_backup: iroha_config::parameters::defaults::sumeragi::RBC_INLINE_BLOCK_CREATED_BACKUP,
                store_max_sessions: 0,
                store_soft_sessions: 0,
                store_max_bytes: 0,
                store_soft_bytes: 0,
                disk_store_ttl: Duration::from_secs(0),
                disk_store_max_bytes: 0,
            },
            finality: SumeragiFinality {
                proof_policy: ProofPolicy::Off,
                commit_cert_history_cap: 0,
                zk_finality_k: 0,
                require_precommit_qc: false,
            },
            keys: SumeragiKeys {
                activation_lead_blocks: 0,
                overlap_grace_blocks: 0,
                expiry_grace_blocks: 0,
                require_hsm: false,
                allowed_algorithms: BTreeSet::from([Algorithm::BlsNormal]),
                allowed_hsm_providers: BTreeSet::new(),
            },
            npos: SumeragiNpos {
                timeouts_overrides: SumeragiNposTimeoutOverrides::default(),
                vrf: SumeragiNposVrf::default(),
                reconfig: SumeragiNposReconfig::default(),
                election: SumeragiNposElection::default(),
                epoch_length_blocks: 0,
                use_stake_snapshot_roster: false,
            },
            resilience: SumeragiResilience::default(),
            vnext: SumeragiVNext::default(),
            adaptive_observability: AdaptiveObservability::default(),
            debug: SumeragiDebug {
                force_soft_fork: false,
                disable_background_worker: false,
                rbc: SumeragiDebugRbc {
                    drop_every_nth_chunk: None,
                    shuffle_chunks: false,
                    duplicate_inits: false,
                    force_deliver_quorum_one: false,
                    corrupt_witness_ack: false,
                    corrupt_ready_signature: false,
                    drop_validator_mask: 0,
                    equivocate_chunk_mask: 0,
                    equivocate_validator_mask: 0,
                    conflicting_ready_mask: 0,
                    partial_chunk_mask: 0,
                },
            },
        }
    }

    #[test]
    fn canonicalize_index_for_view_permissioned_wraps() {
        let signer = ValidatorIndex::try_from(1_usize).expect("validator index");
        let canonical =
            canonicalize_index_for_view(signer, 10, 7, 5, ConsensusMode::Permissioned, None)
                .expect("canonical index");
        let expected = ValidatorIndex::try_from(3_usize).expect("expected index");
        assert_eq!(canonical, expected);
    }

    #[test]
    fn canonicalize_indices_match_formal_permissioned_boundaries() {
        assert_eq!(
            canonicalize_index_for_view(2, 10, 0, 4, ConsensusMode::Permissioned, None),
            Some(2),
            "view zero must preserve permissioned signer indices"
        );
        assert_eq!(
            canonicalize_index_for_view(3, 10, 5, 4, ConsensusMode::Permissioned, None),
            Some(0),
            "permissioned rotation wraps by view modulo topology length"
        );
        assert_eq!(
            canonicalize_index_for_view(4, 10, 0, 4, ConsensusMode::Permissioned, None),
            None,
            "out-of-range signers are rejected"
        );
        assert_eq!(
            canonicalize_index_for_view(0, 10, 0, 0, ConsensusMode::Permissioned, None),
            None,
            "empty topologies cannot produce offenders"
        );
        assert_eq!(
            canonicalize_indices_for_view(
                [3, 1, 3, 0],
                10,
                1,
                4,
                ConsensusMode::Permissioned,
                None
            ),
            vec![0, 1, 2],
            "canonicalized offender indices are deduplicated and sorted"
        );
    }

    #[test]
    fn npos_shuffle_prf_slot_rejects_zero_modulus_and_stays_bounded() {
        let seed = [0x13_u8; 32];

        assert_eq!(
            npos_shuffle_prf_slot(seed, 7, 0, 0),
            None,
            "empty candidate sets cannot select a shuffle slot"
        );
        let slot = npos_shuffle_prf_slot(seed, 7, 0, 5).expect("non-empty candidate set");
        assert!(
            slot < 5,
            "shuffle slot must stay inside the current candidate set"
        );
    }

    #[test]
    fn npos_leader_index_cycles_through_height_permutation_without_repeats() {
        let seed = [0x35_u8; 32];
        let height = 19;
        let topology_len = 5;
        let first_cycle: Vec<_> = (0..topology_len)
            .map(|view| {
                npos_leader_index(seed, height, view as u64, topology_len)
                    .expect("leader index should resolve")
            })
            .collect();
        let unique: BTreeSet<_> = first_cycle.iter().copied().collect();
        assert_eq!(
            unique.len(),
            topology_len,
            "one NPoS leader cycle should visit every validator exactly once"
        );
        assert_eq!(
            npos_leader_index(seed, height, topology_len as u64, topology_len),
            Some(first_cycle[0]),
            "NPoS leader selection should repeat only after a full cycle"
        );
    }

    #[test]
    fn npos_leader_selection_requires_seed_and_binds_height() {
        let seed = [0x42_u8; 32];
        let topology_len = 5;
        let view = 2;
        let baseline =
            npos_leader_index(seed, 3, view, topology_len).expect("baseline leader resolves");
        let changed_height = (4..64)
            .find(|height| npos_leader_index(seed, *height, view, topology_len) != Some(baseline))
            .expect("test seed should produce a different leader for some nearby height");
        assert_ne!(
            npos_leader_index(seed, changed_height, view, topology_len),
            Some(baseline),
            "NPoS leader selection must bind block height"
        );

        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal: test_proposal(0, 3, view, 0, test_block_hash(0x31)),
                reason: "missing seed".to_owned(),
            },
        };
        assert!(
            offender_indices(&evidence, 3, topology_len, ConsensusMode::Npos, None).is_empty(),
            "NPoS attribution without a VRF seed must not fall back to permissioned rotation"
        );
    }

    fn insert_epoch_seed(state: &State, epoch: u64, seed: [u8; 32]) {
        let record = VrfEpochRecord {
            epoch,
            seed,
            epoch_length: 10,
            commit_deadline_offset: 3,
            reveal_deadline_offset: 6,
            roster_len: 1,
            finalized: true,
            updated_at_height: 1,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            penalties_applied: true,
            penalties_applied_at_height: Some(1),
            validator_election: None,
        };
        let mut block = state.world.vrf_epochs.block();
        block.insert(epoch, record);
        block.commit();
    }

    fn record_roster_history(height: u64, block_hash: HashOf<BlockHeader>, roster: Vec<PeerId>) {
        let commit_cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            height,
            commit_cert.view,
            block_hash,
            commit_cert.parent_state_root,
            commit_cert.post_state_root,
            roster,
            Vec::new(),
            Vec::new(),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        crate::sumeragi::status::record_commit_qc(commit_cert);
        crate::sumeragi::status::record_validator_checkpoint(checkpoint);
    }

    fn test_block_hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
    }

    fn test_vote(
        signer: ValidatorIndex,
        height: u64,
        view: u64,
        epoch: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Vote {
        Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([1u8; iroha_crypto::Hash::LENGTH]),
            height,
            view,
            epoch,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer,
            bls_sig: Vec::new(),
        }
    }

    fn test_proposal(
        proposer: ValidatorIndex,
        height: u64,
        view: u64,
        epoch: u64,
        parent_hash: HashOf<BlockHeader>,
    ) -> iroha_data_model::block::consensus::Proposal {
        iroha_data_model::block::consensus::Proposal {
            header: iroha_data_model::block::consensus::ConsensusBlockHeader {
                parent_hash,
                tx_root: Hash::prehashed([0xA2; Hash::LENGTH]),
                state_root: Hash::prehashed([0xA3; Hash::LENGTH]),
                proposer,
                height,
                view,
                epoch,
                highest_qc: iroha_data_model::block::consensus::QcRef {
                    height: height.saturating_sub(1),
                    view: 0,
                    epoch: epoch.saturating_sub(1),
                    subject_block_hash: parent_hash,
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xA4; Hash::LENGTH]),
        }
    }

    fn test_qc(
        height: u64,
        view: u64,
        epoch: u64,
        block_hash: HashOf<BlockHeader>,
        signers_bitmap: Vec<u8>,
        roster: Vec<PeerId>,
    ) -> Qc {
        Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([1u8; iroha_crypto::Hash::LENGTH]),
            height,
            view,
            epoch,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster,
            aggregate: QcAggregate {
                signers_bitmap,
                bls_aggregate_signature: Vec::new(),
            },
        }
    }

    fn test_checkpoint(
        height: u64,
        block_hash: HashOf<BlockHeader>,
        roster: Vec<PeerId>,
    ) -> ValidatorSetCheckpoint {
        ValidatorSetCheckpoint::new(
            height,
            0,
            block_hash,
            iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            iroha_crypto::Hash::prehashed([1u8; iroha_crypto::Hash::LENGTH]),
            roster,
            Vec::new(),
            Vec::new(),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            None,
        )
    }

    fn test_censorship_receipt(
        tx_hash: HashOf<iroha_data_model::transaction::SignedTransaction>,
        height: u64,
        byte: u8,
    ) -> TransactionSubmissionReceipt {
        let key_pair = KeyPair::random();
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: u64::from(byte),
            submitted_at_height: height,
            signer: key_pair.public_key().clone(),
        };
        TransactionSubmissionReceipt::sign(payload, &key_pair)
    }

    fn test_epoch_schedule(fallback_epoch_length: u64) -> EpochScheduleSnapshot {
        EpochScheduleSnapshot {
            finalized: Vec::new(),
            last_finalized_epoch: None,
            last_finalized_end: 0,
            fallback_epoch_length,
        }
    }

    fn double_prepare_evidence(
        signer: ValidatorIndex,
        height: u64,
        view: u64,
        epoch: u64,
        first_block_byte: u8,
        second_block_byte: u8,
    ) -> Evidence {
        Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: test_vote(
                    signer,
                    height,
                    view,
                    epoch,
                    test_block_hash(first_block_byte),
                ),
                v2: test_vote(
                    signer,
                    height,
                    view,
                    epoch,
                    test_block_hash(second_block_byte),
                ),
            },
        }
    }

    fn insert_consensus_evidence_record(
        state: &State,
        evidence: Evidence,
        recorded_at_height: u64,
        penalty_applied: bool,
        penalty_cancelled: bool,
    ) -> Vec<u8> {
        let key = evidence_key(&evidence);
        let record = EvidenceRecord {
            evidence,
            recorded_at_height,
            recorded_at_view: 0,
            recorded_at_ms: recorded_at_height.saturating_mul(10),
            penalty_applied,
            penalty_cancelled,
            penalty_cancelled_at_height: penalty_cancelled.then_some(recorded_at_height),
            penalty_applied_at_height: penalty_applied.then_some(recorded_at_height),
        };
        let mut block = state.world.consensus_evidence.block();
        block.insert(key.clone(), record);
        block.commit();
        key
    }

    fn add_public_lane_validator(
        state: &State,
        peer: &PeerId,
        lane_id: LaneId,
        total_stake: Numeric,
    ) -> AccountId {
        let validator = AccountId::new(peer.public_key().clone());
        let record = iroha_data_model::nexus::PublicLaneValidatorRecord {
            lane_id,
            validator: validator.clone(),
            peer_id: peer.clone(),
            stake_account: validator.clone(),
            total_stake,
            self_stake: Numeric::new(0, 0),
            metadata: iroha_data_model::metadata::Metadata::default(),
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

    fn derive_penalty_actions_for_test(
        state: &State,
        config: &SumeragiConfig,
        current_height: u64,
    ) -> Result<Vec<NposPenaltyAction>> {
        let applier = PenaltyApplier::new(
            state,
            config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        Ok(applier
            .derive_npos_consensus_effects(current_height, Vec::new())?
            .penalty_actions)
    }

    fn apply_effects_for_test(
        state: &State,
        effects: &NposConsensusEffects,
        height: u64,
    ) -> Result<PenaltyOutcome> {
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            height,
            0,
        );
        let mut state_block = state.block(header);
        let nexus = state.nexus_snapshot();
        let mut tx = state_block.transaction();
        let outcome = apply_npos_consensus_effects_to_transaction(
            &mut tx.world,
            effects,
            &nexus.dataspace_catalog,
            &nexus.staking,
            height,
            height,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )?;
        tx.apply();
        state_block.commit()?;
        Ok(outcome)
    }

    #[test]
    fn offender_indices_canonicalize_view_rotation_in_permissioned_mode() {
        let parent_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; 32]));
        let proposal = iroha_data_model::block::consensus::Proposal {
            header: iroha_data_model::block::consensus::ConsensusBlockHeader {
                parent_hash,
                tx_root: Hash::prehashed([0xA2; 32]),
                state_root: Hash::prehashed([0xA3; 32]),
                proposer: 0,
                height: 2,
                view: 1,
                epoch: 0,
                highest_qc: iroha_data_model::block::consensus::QcRef {
                    height: 1,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: parent_hash,
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xA4; 32]),
        };
        let proposal_height = proposal.header.height;
        let proposal_view = proposal.header.view;
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "test".to_owned(),
            },
        };

        let offenders = super::offender_indices(
            &evidence,
            proposal_height,
            4,
            ConsensusMode::Permissioned,
            None,
        );
        assert_eq!(offenders, vec![1]);

        let seed = [0x11_u8; 32];
        let leader = super::npos_leader_index(seed, proposal_height, proposal_view, 4)
            .expect("leader index should resolve");
        let offenders_npos = super::offender_indices(
            &evidence,
            proposal_height,
            4,
            ConsensusMode::Npos,
            Some(seed),
        );
        let expected = ValidatorIndex::try_from(leader).expect("leader index fits validator index");
        assert_eq!(offenders_npos, vec![expected]);
    }

    #[test]
    fn offender_indices_match_formal_evidence_source_and_bitmap_cases() {
        let block_a = test_block_hash(0x40);
        let block_b = test_block_hash(0x41);
        let v1 = test_vote(1, 8, 0, 7, block_a);
        let v2 = test_vote(3, 8, 0, 99, block_b);
        let double_vote = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_eq!(
            offender_indices(&double_vote, 99, 4, ConsensusMode::Permissioned, None),
            vec![1],
            "double-vote attribution uses the first observed vote"
        );
        assert_eq!(evidence_epoch(&double_vote, 99, &test_epoch_schedule(5)), 7);

        let proposal = test_proposal(0, 9, 1, 8, block_a);
        let invalid_proposal = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "invalid proposal".to_owned(),
            },
        };
        assert_eq!(
            offender_indices(&invalid_proposal, 99, 4, ConsensusMode::Permissioned, None),
            vec![1],
            "invalid-proposal attribution uses the proposal header proposer"
        );
        assert_eq!(
            evidence_epoch(&invalid_proposal, 99, &test_epoch_schedule(5)),
            8
        );

        let invalid_qc = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: test_qc(
                    10,
                    0,
                    9,
                    block_a,
                    vec![0b0000_0101, 0b0000_0010],
                    Vec::new(),
                ),
                reason: "invalid qc".to_owned(),
            },
        };
        assert_eq!(
            offender_indices(&invalid_qc, 99, 12, ConsensusMode::Permissioned, None),
            vec![0, 2, 9],
            "invalid-QC attribution expands set bits across every bitmap byte"
        );
        assert_eq!(evidence_epoch(&invalid_qc, 99, &test_epoch_schedule(5)), 9);

        let empty_qc = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: test_qc(10, 0, 9, block_a, Vec::new(), Vec::new()),
                reason: "empty bitmap".to_owned(),
            },
        };
        assert!(offender_indices(&empty_qc, 99, 12, ConsensusMode::Permissioned, None).is_empty());
        assert!(
            evidence_has_legitimate_empty_offenders(&empty_qc),
            "empty invalid-QC bitmaps are legitimate empty-offender evidence"
        );
    }

    #[test]
    fn censorship_evidence_epoch_caps_to_recorded_height() {
        let key_pair = KeyPair::random();
        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xB0; 32]));
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: 1,
            submitted_at_height: 10,
            signer: key_pair.public_key().clone(),
        };
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![receipt],
            },
        };

        let epoch_schedule = EpochScheduleSnapshot {
            finalized: Vec::new(),
            last_finalized_epoch: None,
            last_finalized_end: 0,
            fallback_epoch_length: 5,
        };
        let epoch = super::evidence_epoch(&evidence, 5, &epoch_schedule);
        assert_eq!(epoch, 0);
    }

    #[test]
    fn censorship_anchor_epoch_and_offenders_match_formal_cases() {
        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xB2; Hash::LENGTH]));
        let receipts = vec![
            test_censorship_receipt(tx_hash, 2, 1),
            test_censorship_receipt(tx_hash, 8, 2),
            test_censorship_receipt(tx_hash, 12, 3),
        ];
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        };

        let EvidencePayload::Censorship { receipts, .. } = &evidence.payload else {
            unreachable!("censorship evidence")
        };
        assert_eq!(
            censorship_anchor_height(receipts, 10),
            Some(10),
            "censorship anchor is max receipt height capped by recorded height"
        );
        assert_eq!(
            evidence_epoch(&evidence, 10, &test_epoch_schedule(5)),
            1,
            "censorship evidence epoch derives from the capped anchor"
        );
        assert_eq!(
            offender_indices(&evidence, 10, 4, ConsensusMode::Permissioned, None),
            vec![0],
            "permissioned censorship attribution uses the view-zero leader"
        );

        let empty = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: Vec::new(),
            },
        };
        assert!(
            offender_indices(&empty, 10, 4, ConsensusMode::Permissioned, None).is_empty(),
            "censorship evidence without receipts has no attributable leader"
        );
        assert_eq!(evidence_epoch(&empty, 10, &test_epoch_schedule(5)), 0);
        assert!(!evidence_has_legitimate_empty_offenders(&empty));
    }

    #[test]
    fn censorship_evidence_attributes_to_leader() {
        let key_pair = KeyPair::random();
        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xB1; 32]));
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: 1,
            submitted_at_height: 2,
            signer: key_pair.public_key().clone(),
        };
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![receipt],
            },
        };

        let offenders = super::offender_indices(&evidence, 3, 4, ConsensusMode::Permissioned, None);
        assert_eq!(offenders, vec![0]);

        let seed = [0x11_u8; 32];
        let expected =
            super::npos_leader_index(seed, 2, 0, 4).expect("leader index should resolve");
        let offenders_npos =
            super::offender_indices(&evidence, 3, 4, ConsensusMode::Npos, Some(seed));
        let expected_idx = ValidatorIndex::try_from(expected).expect("leader index fits");
        assert_eq!(offenders_npos, vec![expected_idx]);
    }

    #[test]
    fn vrf_penalties_jail_offenders_and_mark_record() -> Result<()> {
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;

        // Topology with one validator
        let kp = KeyPair::random();
        let peer = PeerId::from(kp.public_key().clone());
        {
            let mut block = state.commit_topology.block();
            block.get_mut().push(peer.clone());
            block.commit();
        }

        // Public lane validator with matching signatory
        let validator: AccountId = AccountId::new(kp.public_key().clone());
        let record = iroha_data_model::nexus::PublicLaneValidatorRecord {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            peer_id: peer.clone(),
            stake_account: validator.clone(),
            total_stake: Numeric::new(100, 0),
            self_stake: Numeric::new(50, 0),
            metadata: iroha_data_model::metadata::Metadata::default(),
            status: PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert((record.lane_id, validator.clone()), record);
            block.commit();
        }

        // VRF epoch record with one offender (index 0)
        let vrf_record = VrfEpochRecord {
            epoch: 1,
            seed: [0xAA; 32],
            epoch_length: 10,
            commit_deadline_offset: 3,
            reveal_deadline_offset: 6,
            roster_len: 1,
            finalized: true,
            updated_at_height: 1,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: vec![0],
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };
        {
            let mut block = state.world.vrf_epochs.block();
            block.insert(vrf_record.epoch, vrf_record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        let outcome = apply_effects_for_test(&state, &effects, 5)?;

        assert_eq!(outcome.applied, 1);
        assert_eq!(outcome.jailed, 1);

        let view = state.world.vrf_epochs.view();
        let updated = view.get(&vrf_record.epoch).expect("vrf record present");
        assert!(updated.penalties_applied);
        assert_eq!(updated.penalties_applied_at_height, Some(5));

        let validators = state.world.public_lane_validators.view();
        let retained = validators
            .get(&(LaneId::new(1), validator.clone()))
            .expect("validator present");
        assert!(matches!(
            retained.status,
            PublicLaneValidatorStatus::Jailed(ref reason)
                if reason == "vrf_penalty_epoch_1"
        ));

        Ok(())
    }

    #[test]
    fn vrf_penalties_remain_pending_when_offenders_missing_from_topology() -> Result<()> {
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;

        let vrf_record = VrfEpochRecord {
            epoch: 2,
            seed: [0xBB; 32],
            epoch_length: 4,
            commit_deadline_offset: 2,
            reveal_deadline_offset: 3,
            roster_len: 1,
            finalized: true,
            updated_at_height: 1,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: vec![3], // No corresponding validator index in topology
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };
        {
            let mut block = state.world.vrf_epochs.block();
            block.insert(vrf_record.epoch, vrf_record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        assert!(
            effects.penalty_actions.is_empty(),
            "unmapped VRF offenders must not produce committed effects"
        );

        let view = state.world.vrf_epochs.view();
        let updated = view.get(&vrf_record.epoch).expect("vrf record present");
        assert!(!updated.penalties_applied);
        assert_eq!(updated.penalties_applied_at_height, None);

        Ok(())
    }

    #[test]
    fn consensus_penalties_mark_records_when_due() -> Result<()> {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;
        config.npos.reconfig.slashing_delay_blocks = 0;
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();
        insert_epoch_seed(&state, 0, [0x10; 32]);

        // Evidence with empty signer bitmap (no offenders but should mark applied)
        let keypair = KeyPair::random();
        let roster = vec![PeerId::new(keypair.public_key().clone())];
        let qc = Qc {
            phase: Phase::Prepare,
            subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0x11; Hash::LENGTH],
            )),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        record_roster_history(qc.height, qc.subject_block_hash, roster);
        let evidence = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: qc,
                reason: "empty bitmap".to_owned(),
            },
        };
        let record = EvidenceRecord {
            evidence,
            recorded_at_height: 1,
            recorded_at_view: 1,
            recorded_at_ms: 123,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        let outcome = apply_effects_for_test(&state, &effects, 5)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(updated.penalty_applied);
        assert_eq!(updated.penalty_applied_at_height, Some(5));

        Ok(())
    }

    #[test]
    fn consensus_penalties_skip_cancelled_record() -> Result<()> {
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;
        config.npos.reconfig.slashing_delay_blocks = 0;

        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x51; Hash::LENGTH]));
        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height: 1,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x52; Hash::LENGTH]));
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let record = EvidenceRecord {
            evidence,
            recorded_at_height: 1,
            recorded_at_view: 1,
            recorded_at_ms: 321,
            penalty_applied: false,
            penalty_cancelled: true,
            penalty_cancelled_at_height: Some(1),
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        let outcome = apply_effects_for_test(&state, &effects, 5)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(!updated.penalty_applied);
        assert!(updated.penalty_cancelled);
        assert_eq!(updated.penalty_applied_at_height, None);

        Ok(())
    }

    #[test]
    fn consensus_penalties_pending_until_slashing_delay_elapses() -> Result<()> {
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;
        config.npos.reconfig.slashing_delay_blocks = 10;

        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x61; Hash::LENGTH]));
        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height: 5,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x62; Hash::LENGTH]));
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let record = EvidenceRecord {
            evidence,
            recorded_at_height: 5,
            recorded_at_view: 1,
            recorded_at_ms: 555,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(12, Vec::new())?;
        assert!(
            effects.penalty_actions.is_empty(),
            "not-yet-due evidence must not produce committed effects"
        );

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(!updated.penalty_applied);
        assert_eq!(updated.penalty_applied_at_height, None);

        Ok(())
    }

    #[test]
    fn consensus_penalties_skip_unmapped_offender() -> Result<()> {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;
        config.npos.reconfig.slashing_delay_blocks = 0;
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();
        insert_epoch_seed(&state, 0, [0x12; 32]);

        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; Hash::LENGTH]));
        let roster = vec![PeerId::new(KeyPair::random().public_key().clone())];
        record_roster_history(2, block_hash, roster);
        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 2,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x23; Hash::LENGTH]));
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let record = EvidenceRecord {
            evidence,
            recorded_at_height: 2,
            recorded_at_view: 1,
            recorded_at_ms: 456,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        assert!(
            effects.penalty_actions.is_empty(),
            "unmapped offender must not produce committed effects"
        );

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(!updated.penalty_applied);
        assert_eq!(updated.penalty_applied_at_height, None);

        Ok(())
    }

    #[test]
    fn consensus_penalties_pending_without_roster() -> Result<()> {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;
        config.npos.reconfig.slashing_delay_blocks = 0;
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();
        insert_epoch_seed(&state, 0, [0x13; 32]);

        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x31; Hash::LENGTH]));
        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 3,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x32; Hash::LENGTH]));
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let record = EvidenceRecord {
            evidence,
            recorded_at_height: 3,
            recorded_at_view: 1,
            recorded_at_ms: 999,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        assert!(
            effects.penalty_actions.is_empty(),
            "evidence without roster must not produce committed effects"
        );

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(!updated.penalty_applied);
        assert_eq!(updated.penalty_applied_at_height, None);

        Ok(())
    }

    #[test]
    fn consensus_penalties_pending_without_prf_seed() -> Result<()> {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;
        config.npos.reconfig.slashing_delay_blocks = 0;
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();

        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x41; Hash::LENGTH]));
        let roster = vec![PeerId::new(KeyPair::random().public_key().clone())];
        record_roster_history(4, block_hash, roster);

        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 4,
            view: 1,
            epoch: 1,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; Hash::LENGTH]));
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let record = EvidenceRecord {
            evidence,
            recorded_at_height: 4,
            recorded_at_view: 1,
            recorded_at_ms: 555,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        assert!(
            effects.penalty_actions.is_empty(),
            "NPoS evidence without PRF seed must not produce committed effects"
        );

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(!updated.penalty_applied);
        assert_eq!(updated.penalty_applied_at_height, None);

        Ok(())
    }

    #[test]
    fn consensus_penalty_actions_match_formal_eligibility_boundaries() -> Result<()> {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();

        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.consensus_mode = ConsensusMode::Permissioned;
        config.npos.reconfig.slashing_delay_blocks = 3;

        let key_pair = KeyPair::random();
        let peer = PeerId::from(key_pair.public_key().clone());
        let validator =
            add_public_lane_validator(&state, &peer, LaneId::new(1), Numeric::new(100, 0));
        record_roster_history(9, test_block_hash(0x79), vec![peer.clone()]);

        insert_consensus_evidence_record(
            &state,
            double_prepare_evidence(0, 2, 0, 0, 0x71, 0x72),
            2,
            true,
            false,
        );
        insert_consensus_evidence_record(
            &state,
            double_prepare_evidence(0, 3, 0, 0, 0x73, 0x74),
            3,
            false,
            true,
        );
        insert_consensus_evidence_record(
            &state,
            double_prepare_evidence(0, 10, 0, 0, 0x75, 0x76),
            10,
            false,
            false,
        );
        let due_key = insert_consensus_evidence_record(
            &state,
            double_prepare_evidence(0, 9, 0, 0, 0x79, 0x7A),
            9,
            false,
            false,
        );

        let actions = derive_penalty_actions_for_test(&state, &config, 12)?;
        assert_eq!(
            actions.len(),
            2,
            "only the boundary-due record should produce slash and marker actions"
        );

        let NposPenaltyAction::ConsensusSlash(slash) = &actions[0] else {
            panic!("first action should be the consensus slash");
        };
        assert_eq!(slash.evidence_key, due_key);
        assert_eq!(slash.signer, 0);
        assert_eq!(slash.peer_id, peer);
        assert_eq!(slash.lane_id, LaneId::new(1));
        assert_eq!(slash.validator, validator);
        assert_eq!(slash.slash_id, Hash::new(due_key.clone()));
        assert_eq!(slash.amount, Numeric::new(100, 0));

        let NposPenaltyAction::MarkConsensusEvidenceApplied(mark) = &actions[1] else {
            panic!("second action should mark the due evidence");
        };
        assert_eq!(mark.evidence_key, due_key);
        assert_eq!(mark.height, 12);

        Ok(())
    }

    #[test]
    fn consensus_penalty_actions_match_formal_pending_and_empty_offender_cases() -> Result<()> {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();

        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.consensus_mode = ConsensusMode::Permissioned;
        config.npos.reconfig.slashing_delay_blocks = 0;

        let peer = PeerId::from(KeyPair::random().public_key().clone());
        add_public_lane_validator(&state, &peer, LaneId::new(1), Numeric::new(0, 0));
        {
            let mut block = state.commit_topology.block();
            block.get_mut().push(peer.clone());
            block.commit();
        }
        record_roster_history(4, test_block_hash(0x81), vec![peer.clone()]);
        record_roster_history(5, test_block_hash(0x83), vec![peer]);

        insert_consensus_evidence_record(
            &state,
            double_prepare_evidence(0, 4, 0, 0, 0x81, 0x82),
            4,
            false,
            false,
        );
        insert_consensus_evidence_record(
            &state,
            double_prepare_evidence(3, 5, 0, 0, 0x83, 0x84),
            5,
            false,
            false,
        );
        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0x85; Hash::LENGTH]));
        insert_consensus_evidence_record(
            &state,
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: Vec::new(),
                },
            },
            6,
            false,
            false,
        );

        let actions = derive_penalty_actions_for_test(&state, &config, 7)?;
        assert!(
            actions.is_empty(),
            "mapped offenders without slash amount, non-legitimate empty offenders, and empty censorship evidence stay pending"
        );

        Ok(())
    }

    #[test]
    fn consensus_penalty_actions_emit_slashes_marks_and_sorted_effects() -> Result<()> {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();

        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.consensus_mode = ConsensusMode::Permissioned;
        config.npos.reconfig.slashing_delay_blocks = 0;

        let first_peer = PeerId::from(KeyPair::random().public_key().clone());
        let second_peer = PeerId::from(KeyPair::random().public_key().clone());
        let first_validator =
            add_public_lane_validator(&state, &first_peer, LaneId::new(1), Numeric::new(50, 0));
        let second_validator =
            add_public_lane_validator(&state, &second_peer, LaneId::new(2), Numeric::new(70, 0));
        let roster = vec![first_peer.clone(), second_peer.clone()];
        let block_hash = test_block_hash(0x91);
        record_roster_history(8, block_hash, roster);

        let evidence = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: test_qc(8, 0, 0, block_hash, vec![0b0000_0011], Vec::new()),
                reason: "two invalid signers".to_owned(),
            },
        };
        let key = insert_consensus_evidence_record(&state, evidence, 8, false, false);

        let actions = derive_penalty_actions_for_test(&state, &config, 9)?;
        let mut sorted_unique = actions.clone();
        sorted_unique.sort();
        sorted_unique.dedup();
        assert_eq!(
            actions, sorted_unique,
            "derived penalty effects must be sorted and deduplicated"
        );
        assert_eq!(actions.len(), 3);

        let slashes = actions
            .iter()
            .filter_map(|action| match action {
                NposPenaltyAction::ConsensusSlash(action) => Some(action),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(slashes.len(), 2);
        assert_eq!(slashes[0].evidence_key, key);
        assert_eq!(slashes[0].signer, 0);
        assert_eq!(slashes[0].peer_id, first_peer);
        assert_eq!(slashes[0].lane_id, LaneId::new(1));
        assert_eq!(slashes[0].validator, first_validator);
        assert_eq!(slashes[0].slash_id, Hash::new(key.clone()));
        assert_eq!(slashes[0].amount, Numeric::new(50, 0));
        assert_eq!(slashes[1].evidence_key, key);
        assert_eq!(slashes[1].signer, 1);
        assert_eq!(slashes[1].peer_id, second_peer);
        assert_eq!(slashes[1].lane_id, LaneId::new(2));
        assert_eq!(slashes[1].validator, second_validator);
        assert_eq!(slashes[1].slash_id, Hash::new(key.clone()));
        assert_eq!(slashes[1].amount, Numeric::new(70, 0));

        let marks = actions
            .iter()
            .filter_map(|action| match action {
                NposPenaltyAction::MarkConsensusEvidenceApplied(action) => Some(action),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].evidence_key, key);
        assert_eq!(marks[0].height, 9);

        Ok(())
    }

    #[test]
    fn mark_consensus_evidence_application_matches_formal_existing_and_missing_cases() -> Result<()>
    {
        let state = fresh_state();
        let existing_key = insert_consensus_evidence_record(
            &state,
            double_prepare_evidence(0, 1, 0, 0, 0xA1, 0xA2),
            1,
            false,
            false,
        );
        let missing_key = vec![0xFF, 0x00, 0xFE];
        assert!(
            state
                .world
                .consensus_evidence
                .view()
                .get(&missing_key)
                .is_none(),
            "missing marker key should start absent"
        );

        let effects = NposConsensusEffects {
            vrf_epoch_seals: Vec::new(),
            penalty_actions: vec![
                NposPenaltyAction::MarkConsensusEvidenceApplied(
                    NposMarkConsensusEvidenceAppliedAction {
                        evidence_key: existing_key.clone(),
                        height: 7,
                    },
                ),
                NposPenaltyAction::MarkConsensusEvidenceApplied(
                    NposMarkConsensusEvidenceAppliedAction {
                        evidence_key: missing_key.clone(),
                        height: 8,
                    },
                ),
            ],
        };
        let outcome = apply_effects_for_test(&state, &effects, 7)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);
        assert_eq!(outcome.jailed, 0);

        let view = state.world.consensus_evidence.view();
        let existing = view.get(&existing_key).expect("existing evidence remains");
        assert!(existing.penalty_applied);
        assert_eq!(existing.penalty_applied_at_height, Some(7));
        assert!(
            view.get(&missing_key).is_none(),
            "marking a missing evidence key must be a no-op"
        );

        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn consensus_penalties_mark_censorship_and_slash() -> Result<()> {
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.consensus_mode = ConsensusMode::Permissioned;
        config.npos.reconfig.activation_lag_blocks = 0;
        config.npos.reconfig.slashing_delay_blocks = 0;

        let key_pair = KeyPair::random();
        let peer = PeerId::from(key_pair.public_key().clone());
        {
            let mut block = state.commit_topology.block();
            block.get_mut().push(peer.clone());
            block.commit();
        }

        let domain: DomainId = DomainId::try_new("test", "universal").expect("domain id");
        let validator: AccountId = AccountId::new(key_pair.public_key().clone());
        let escrow_key_pair = KeyPair::random();
        let escrow_account: AccountId = AccountId::new(escrow_key_pair.public_key().clone());
        let stake_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("test", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let slash_amount = Numeric::new(100, 0);
        {
            let mut block = state.world.block();
            block.domains.insert(
                domain.clone(),
                Domain::new(domain.clone()).build(&validator),
            );
            block
                .accounts
                .insert(validator.clone(), Owned::new(AccountDetails::default()));
            block.accounts.insert(
                escrow_account.clone(),
                Owned::new(AccountDetails::default()),
            );
            block.asset_definitions.insert(
                stake_asset_id.clone(),
                AssetDefinition::numeric(stake_asset_id.clone()).build(&validator),
            );
            block
                .domain_asset_definitions
                .insert(domain.clone(), BTreeSet::from([stake_asset_id.clone()]));
            let escrow_stake_asset_id =
                AssetId::new(stake_asset_id.clone(), escrow_account.clone());
            block
                .assets
                .insert(escrow_stake_asset_id, Owned::new(slash_amount.clone()));
            block.asset_definition_holders.insert(
                stake_asset_id.clone(),
                BTreeSet::from([escrow_account.clone()]),
            );
            block.commit();
        }
        {
            let mut nexus = state.nexus.write();
            nexus.enabled = true;
            nexus.staking.stake_asset_id = stake_asset_id.to_string();
            nexus.staking.stake_escrow_account_id = escrow_account.to_string();
            nexus.staking.slash_sink_account_id = escrow_account.to_string();
            nexus.lane_catalog = LaneCatalog::new(
                NonZeroU32::new(2).expect("lane count"),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: LaneId::new(1),
                        alias: "lane-1".to_string(),
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("lane catalog");
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }
        let record = iroha_data_model::nexus::PublicLaneValidatorRecord {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            peer_id: PeerId::from(validator.signatory().clone()),
            stake_account: validator.clone(),
            total_stake: slash_amount.clone(),
            self_stake: slash_amount.clone(),
            metadata: iroha_data_model::metadata::Metadata::default(),
            status: PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert((record.lane_id, validator.clone()), record);
            block.commit();
        }

        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xB1; 32]));
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: 10,
            submitted_at_height: 2,
            signer: key_pair.public_key().clone(),
        };
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![receipt],
            },
        };
        let record = EvidenceRecord {
            evidence,
            recorded_at_height: 2,
            recorded_at_view: 1,
            recorded_at_ms: 321,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record.clone());
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let effects = applier.derive_npos_consensus_effects(5, Vec::new())?;
        let outcome = apply_effects_for_test(&state, &effects, 5)?;
        assert_eq!(outcome.applied, 1);
        assert_eq!(outcome.slashed, 1);

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(updated.penalty_applied);
        assert_eq!(updated.penalty_applied_at_height, Some(5));

        let validators = state.world.public_lane_validators.view();
        let retained = validators
            .get(&(LaneId::new(1), validator.clone()))
            .expect("validator present");
        let slash_id = Hash::new(key.clone());
        assert!(matches!(
            retained.status,
            PublicLaneValidatorStatus::Slashed(id) if id == slash_id
        ));

        Ok(())
    }

    #[test]
    fn evidence_block_refs_capture_double_vote_candidates() {
        let block_hash_a =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x10; Hash::LENGTH]));
        let block_hash_b =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x20; Hash::LENGTH]));
        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash: block_hash_a,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 5,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = block_hash_b;
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };

        let refs = crate::sumeragi::evidence::evidence_block_refs(&evidence);
        assert_eq!(refs, vec![(5, block_hash_a), (5, block_hash_b)]);
    }

    #[test]
    fn consensus_mode_for_evidence_prefers_subject_height() {
        let state = fresh_state();
        {
            let mut block = state.block_hashes.block();
            for idx in 0u8..12 {
                let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [idx; Hash::LENGTH],
                ));
                block.push(hash);
            }
            block.commit_for_tests();
        }
        {
            let mut block = state.world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(10);
            block.commit();
        }

        let block_hash_a =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA0; Hash::LENGTH]));
        let block_hash_b =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xB0; Hash::LENGTH]));
        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash: block_hash_a,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = block_hash_b;
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };

        let mode =
            super::consensus_mode_for_evidence(&state, &evidence, 12, ConsensusMode::Permissioned);
        assert_eq!(mode, ConsensusMode::Permissioned);
    }

    #[test]
    fn consensus_mode_for_evidence_uses_recorded_height_without_subject() {
        let state = fresh_state();
        {
            let mut block = state.block_hashes.block();
            for idx in 0u8..12 {
                block.push(test_block_hash(idx));
            }
            block.commit_for_tests();
        }
        {
            let mut block = state.world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(10);
            block.commit();
        }

        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xD1; Hash::LENGTH])),
                receipts: Vec::new(),
            },
        };

        let mode =
            super::consensus_mode_for_evidence(&state, &evidence, 12, ConsensusMode::Permissioned);
        assert_eq!(mode, ConsensusMode::Npos);
    }

    #[test]
    fn roster_for_evidence_uses_commit_history_candidates() {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        let state = fresh_state();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();

        let keypair0 = KeyPair::random();
        let keypair1 = KeyPair::random();
        let peer0 = PeerId::new(keypair0.public_key().clone());
        let peer1 = PeerId::new(keypair1.public_key().clone());
        let roster = vec![peer1.clone(), peer0.clone()];
        let height = 7_u64;
        let block_hash_a =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA0; Hash::LENGTH]));
        let block_hash_b =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xB0; Hash::LENGTH]));

        let commit_cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash_b,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            height,
            commit_cert.view,
            block_hash_b,
            commit_cert.parent_state_root,
            commit_cert.post_state_root,
            roster.clone(),
            Vec::new(),
            Vec::new(),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        crate::sumeragi::status::record_commit_qc(commit_cert);
        crate::sumeragi::status::record_validator_checkpoint(checkpoint);

        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash: block_hash_a,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = block_hash_b;
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };

        let commit_certs = crate::sumeragi::status::commit_qc_history();
        let checkpoints = crate::sumeragi::status::validator_checkpoint_history();
        let resolved = super::roster_for_evidence(&state, &evidence, &commit_certs, &checkpoints)
            .expect("roster resolved");
        assert_eq!(resolved, roster);
    }

    #[test]
    fn roster_for_evidence_matches_formal_fallback_ordering() {
        let _commit_history_guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();

        let peers = || {
            (0..2)
                .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
                .collect::<Vec<_>>()
        };
        let current_roster = peers();
        let state_roster = peers();
        let cert_roster = peers();
        let checkpoint_roster = peers();
        let height = 13_u64;
        let block_a = test_block_hash(0xE1);
        let block_b = test_block_hash(0xE2);
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: test_vote(0, height, 0, 0, block_a),
                v2: test_vote(0, height, 0, 0, block_b),
            },
        };

        let current_state = fresh_state();
        {
            let mut block = current_state.commit_topology.block();
            block.get_mut().extend(current_roster.clone());
            block.commit();
        }
        let no_ref_evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xD2; Hash::LENGTH])),
                receipts: Vec::new(),
            },
        };
        assert_eq!(
            roster_for_evidence(&current_state, &no_ref_evidence, &[], &[]),
            Some(current_roster.clone()),
            "evidence without block refs uses current commit topology when non-empty"
        );
        assert_eq!(
            roster_for_evidence(&fresh_state(), &no_ref_evidence, &[], &[]),
            None,
            "empty current topology is ignored for no-ref evidence"
        );

        let state = fresh_state();
        let state_qc = test_qc(height, 0, 0, block_a, Vec::new(), state_roster.clone());
        let state_checkpoint = test_checkpoint(height, block_a, state_roster.clone());
        assert!(state.record_commit_roster(&state_qc, &state_checkpoint, None));
        let cert_qc = test_qc(height, 0, 0, block_a, Vec::new(), cert_roster.clone());
        let checkpoint = test_checkpoint(height, block_a, checkpoint_roster.clone());
        assert_eq!(
            roster_for_evidence(
                &state,
                &evidence,
                core::slice::from_ref(&cert_qc),
                core::slice::from_ref(&checkpoint)
            ),
            Some(state_roster),
            "state commit-roster snapshot has priority over sidecar fallbacks"
        );

        assert_eq!(
            roster_for_evidence(
                &fresh_state(),
                &evidence,
                core::slice::from_ref(&cert_qc),
                core::slice::from_ref(&checkpoint)
            ),
            Some(cert_roster.clone()),
            "commit certificate is used when no state snapshot is present"
        );

        let empty_cert = test_qc(height, 0, 0, block_a, Vec::new(), Vec::new());
        assert_eq!(
            roster_for_evidence(
                &fresh_state(),
                &evidence,
                core::slice::from_ref(&empty_cert),
                core::slice::from_ref(&checkpoint)
            ),
            Some(checkpoint_roster),
            "validator checkpoint is used after empty commit certificates are ignored"
        );
        assert_eq!(
            roster_for_evidence(&fresh_state(), &evidence, &[empty_cert], &[]),
            None,
            "unresolved evidence returns no roster instead of an empty roster"
        );
    }

    #[test]
    fn locate_validator_in_roster_prefers_matching_peer() {
        let state = fresh_state();
        let mut config = test_sumeragi_config();
        config.npos.reconfig.activation_lag_blocks = 0;

        let keypair = KeyPair::random();
        let peer = PeerId::new(keypair.public_key().clone());
        let validator = AccountId::new(keypair.public_key().clone());
        let record = iroha_data_model::nexus::PublicLaneValidatorRecord {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            peer_id: peer.clone(),
            stake_account: validator.clone(),
            total_stake: Numeric::new(100, 0),
            self_stake: Numeric::new(50, 0),
            metadata: iroha_data_model::metadata::Metadata::default(),
            status: PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert((record.lane_id, validator.clone()), record);
            block.commit();
        }

        let applier = PenaltyApplier::new(
            &state,
            &config,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        );
        let map = applier.build_validator_locator_map();
        let (_peer, locator) = applier
            .locate_validator_in_roster_cached(0, &[peer], &map)
            .expect("locator resolved");
        assert_eq!(locator.lane_id, LaneId::new(1));
        assert_eq!(locator.validator, validator);
    }
}
