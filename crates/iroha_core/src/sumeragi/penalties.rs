//! Penalty enforcement for `NPoS`: VRF non-participation and consensus evidence slashing.

use std::collections::{BTreeMap, BTreeSet};

use eyre::Result;
use iroha_config::parameters::actual::{ConsensusMode, Sumeragi as SumeragiConfig};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    block::consensus::{Evidence, EvidencePayload, EvidenceRecord},
    consensus::{Qc, ValidatorSetCheckpoint, VrfEpochRecord},
    nexus::{LaneId, PublicLaneValidatorStatus},
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
    pub pending: u64,
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
    config: &'a SumeragiConfig,
    #[cfg(feature = "telemetry")]
    telemetry: Option<&'a StateTelemetry>,
    #[cfg(not(feature = "telemetry"))]
    #[allow(dead_code)]
    telemetry: Option<()>,
}

impl<'a> PenaltyApplier<'a> {
    pub(crate) fn new(
        state: &'a State,
        config: &'a SumeragiConfig,
        #[cfg(feature = "telemetry")] telemetry: Option<&'a StateTelemetry>,
        #[cfg(not(feature = "telemetry"))] telemetry: Option<()>,
    ) -> Self {
        Self {
            state,
            config,
            telemetry,
        }
    }

    fn build_validator_locator_map(&self) -> BTreeMap<PublicKey, ValidatorLocator> {
        let world = self.state.world_view();
        let mut candidates_map: BTreeMap<PublicKey, Vec<ValidatorLocator>> = BTreeMap::new();

        for ((lane_id, validator_id), record) in world.public_lane_validators().iter() {
            if let Some(pk) = record.validator.try_signatory() {
                candidates_map
                    .entry(pk.clone())
                    .or_default()
                    .push(ValidatorLocator {
                        lane_id: *lane_id,
                        validator: validator_id.clone(),
                    });
            }
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

    pub(crate) fn apply_vrf_penalties(&self, current_height: u64) -> PenaltyOutcome {
        let mut outcome = PenaltyOutcome::default();
        let activation_lag = {
            let world = self.state.world_view();
            crate::sumeragi::resolve_npos_activation_lag_blocks_from_world(
                &world,
                &self.config.npos,
            )
        };
        let view = self.state.world.vrf_epochs.view();
        let mut due_records: Vec<VrfEpochRecord> = Vec::new();
        for (_epoch, record) in view.iter() {
            if !record.finalized || record.penalties_applied {
                continue;
            }
            if record.updated_at_height.saturating_add(activation_lag) > current_height {
                outcome.pending = outcome.pending.saturating_add(1);
                continue;
            }
            due_records.push(record.clone());
        }
        drop(view);

        if due_records.is_empty() {
            return outcome;
        }

        let lane_config = self.state.nexus_snapshot().lane_config.clone();
        let validator_map = self.build_validator_locator_map();
        let commit_topology = self.state.commit_topology_snapshot();

        for record in due_records {
            let offenders: BTreeSet<u32> = record
                .committed_no_reveal
                .iter()
                .chain(record.no_participation.iter())
                .copied()
                .collect();
            // Resolve validators before opening a write transaction to avoid re-entrant locks.
            let mut applied_here = offenders.is_empty();
            let mut unmapped_offenders = false;
            let mut locators = Vec::new();
            for signer in offenders {
                if let Some(locator) =
                    Self::locate_validator_cached(signer, &commit_topology, &validator_map)
                {
                    locators.push(locator);
                } else {
                    unmapped_offenders = true;
                }
            }
            let mut block = self.state.world.block();
            #[cfg(feature = "telemetry")]
            let mut tx = block.trasaction(self.telemetry, lane_config.clone(), current_height);
            #[cfg(not(feature = "telemetry"))]
            let mut tx = block.trasaction(lane_config.clone(), current_height);
            for locator in locators {
                if jail_in_transaction(
                    &mut tx,
                    &locator,
                    &format!("vrf_penalty_epoch_{}", record.epoch),
                    #[cfg(feature = "telemetry")]
                    self.telemetry,
                    #[cfg(not(feature = "telemetry"))]
                    None,
                ) {
                    outcome.applied = outcome.applied.saturating_add(1);
                    outcome.jailed = outcome.jailed.saturating_add(1);
                    applied_here = true;
                }
            }

            let mut updated = record.clone();
            if applied_here || unmapped_offenders {
                updated.penalties_applied = true;
                updated.penalties_applied_at_height = Some(current_height);
            }
            tx.vrf_epochs.insert(updated.epoch, updated);
            tx.apply();
            block.commit();
        }

        outcome
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn apply_consensus_penalties(&self, current_height: u64) -> Result<PenaltyOutcome> {
        let mut outcome = PenaltyOutcome::default();
        let slashing_delay = {
            let world = self.state.world_view();
            crate::sumeragi::resolve_npos_slashing_delay_blocks_from_world(
                &world,
                &self.config.npos,
            )
        };
        let evidence_view = self.state.world.consensus_evidence.view();
        let mut pending: Vec<(Vec<u8>, EvidenceRecord)> = Vec::new();
        for (key, record) in evidence_view.iter() {
            if record.penalty_applied || record.penalty_cancelled {
                continue;
            }
            if record.recorded_at_height.saturating_add(slashing_delay) > current_height {
                outcome.pending = outcome.pending.saturating_add(1);
                continue;
            }
            pending.push((key.clone(), record.clone()));
        }
        drop(evidence_view);

        if pending.is_empty() {
            return Ok(outcome);
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
                crate::sumeragi::load_npos_epoch_params_from_world(&world, &self.config.npos);
            EpochScheduleSnapshot::from_world_with_fallback(
                &world,
                epoch_params.epoch_length_blocks,
            )
        };

        let nexus = self.state.nexus_snapshot();
        let staking_cfg = nexus.staking.clone();
        let lane_config = nexus.lane_config.clone();
        let validator_map = self.build_validator_locator_map();

        let commit_certs = crate::sumeragi::status::commit_qc_history();
        let checkpoints = crate::sumeragi::status::validator_checkpoint_history();
        for (key, mut record) in pending {
            let consensus_mode = consensus_mode_for_evidence(
                self.state,
                &record.evidence,
                record.recorded_at_height,
                self.config.consensus_mode,
            );
            let is_censorship =
                matches!(record.evidence.payload, EvidencePayload::Censorship { .. });
            let evidence_epoch =
                evidence_epoch(&record.evidence, record.recorded_at_height, &epoch_schedule);
            let prf_seed = match consensus_mode {
                ConsensusMode::Permissioned => None,
                ConsensusMode::Npos => epoch_seeds.get(&evidence_epoch).copied(),
            };
            // Prefer evidence-specific rosters to avoid misattribution after topology rotation.
            let evidence_roster =
                roster_for_evidence(self.state, &record.evidence, &commit_certs, &checkpoints)
                    .filter(|roster| !roster.is_empty());
            let Some(roster) = evidence_roster.as_ref() else {
                outcome.pending = outcome.pending.saturating_add(1);
                continue;
            };
            if matches!(consensus_mode, ConsensusMode::Npos) && prf_seed.is_none() {
                outcome.pending = outcome.pending.saturating_add(1);
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
            let slash_id = Hash::new(key.clone());
            // Resolve validators before opening a write transaction to avoid re-entrant locks.
            if offenders.is_empty() {
                if is_censorship || !self::evidence_has_legitimate_empty_offenders(&record.evidence)
                {
                    outcome.pending = outcome.pending.saturating_add(1);
                    continue;
                }
                let mut block = self.state.world.block();
                #[cfg(feature = "telemetry")]
                let mut tx = block.trasaction(self.telemetry, lane_config.clone(), current_height);
                #[cfg(not(feature = "telemetry"))]
                let mut tx = block.trasaction(lane_config.clone(), current_height);
                record.penalty_applied = true;
                record.penalty_applied_at_height = Some(current_height);
                tx.consensus_evidence.insert(key, record);
                tx.apply();
                block.commit();
                continue;
            }
            let mut locators = Vec::new();
            for signer in offenders {
                if let Some(locator) =
                    self.locate_validator_in_roster_cached(signer, roster, &validator_map)
                {
                    locators.push(locator);
                }
            }
            if locators.is_empty() {
                outcome.pending = outcome.pending.saturating_add(1);
                continue;
            }
            let mut block = self.state.world.block();
            #[cfg(feature = "telemetry")]
            let mut tx = block.trasaction(self.telemetry, lane_config.clone(), current_height);
            #[cfg(not(feature = "telemetry"))]
            let mut tx = block.trasaction(lane_config.clone(), current_height);
            let nexus = self.state.nexus_snapshot();
            let mut applied_here = false;
            for locator in locators {
                if let Some(amount) =
                    max_slash_amount_for_validator(&tx, &locator, staking_cfg.max_slash_bps)?
                {
                    apply_slash_to_validator(
                        &mut tx,
                        &nexus.dataspace_catalog,
                        &staking_cfg,
                        locator.lane_id,
                        &locator.validator,
                        slash_id,
                        &amount,
                        self.state
                            .latest_block_header_fast()
                            .map(|header| header.creation_time_ms)
                            .unwrap_or(0),
                        #[cfg(feature = "telemetry")]
                        self.telemetry,
                        #[cfg(not(feature = "telemetry"))]
                        None,
                    )?;
                    outcome.applied = outcome.applied.saturating_add(1);
                    outcome.slashed = outcome.slashed.saturating_add(1);
                    applied_here = true;
                }
            }
            if applied_here {
                record.penalty_applied = true;
                record.penalty_applied_at_height = Some(current_height);
            } else {
                outcome.pending = outcome.pending.saturating_add(1);
            }
            tx.consensus_evidence.insert(key, record);
            tx.apply();
            block.commit();
        }

        Ok(outcome)
    }

    fn locate_validator_cached(
        signer: ValidatorIndex,
        commit_topology: &[PeerId],
        map: &BTreeMap<PublicKey, ValidatorLocator>,
    ) -> Option<ValidatorLocator> {
        let signer_idx = usize::try_from(signer).ok()?;
        let peer = commit_topology.get(signer_idx)?;
        map.get(peer.public_key()).cloned()
    }

    #[allow(clippy::unused_self)]
    fn locate_validator_in_roster_cached(
        &self,
        signer: ValidatorIndex,
        roster: &[PeerId],
        map: &BTreeMap<PublicKey, ValidatorLocator>,
    ) -> Option<ValidatorLocator> {
        let signer_idx = usize::try_from(signer).ok()?;
        let peer = roster.get(signer_idx)?;
        map.get(peer.public_key()).cloned()
    }
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
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    if topology_len == 0 {
        return None;
    }
    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &seed);
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &height.to_be_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &view.to_be_bytes());
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut w = [0u8; 8];
    w.copy_from_slice(&digest[..8]);
    let modulus = u128::try_from(topology_len).ok()?;
    Some((u128::from(u64::from_be_bytes(w)) % modulus) as usize)
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

fn max_slash_amount_for_validator(
    tx: &WorldTransaction<'_, '_>,
    locator: &ValidatorLocator,
    max_bps: u16,
) -> Result<Option<Numeric>> {
    let Some(record) = tx
        .public_lane_validators
        .get(&(locator.lane_id, locator.validator.clone()))
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
        SumeragiRbc, SumeragiRecovery, SumeragiWorker,
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
                chunk_fanout: None,
                pending_max_chunks: 0,
                pending_max_bytes: 0,
                pending_session_limit: 0,
                pending_ttl: Duration::from_secs(0),
                session_ttl: Duration::from_secs(0),
                rebroadcast_sessions_per_tick: 1,
                payload_chunks_per_tick: 1,
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
            penalties_applied: false,
            penalties_applied_at_height: None,
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
    fn censorship_evidence_epoch_caps_to_recorded_height() {
        let key_pair = KeyPair::random();
        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xB0; 32]));
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
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
    fn censorship_evidence_attributes_to_leader() {
        let key_pair = KeyPair::random();
        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xB1; 32]));
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
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
    fn vrf_penalties_jail_offenders_and_mark_record() {
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
        let outcome = applier.apply_vrf_penalties(5);

        assert_eq!(outcome.applied, 1);
        assert_eq!(outcome.jailed, 1);
        assert_eq!(outcome.pending, 0);

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
    }

    #[test]
    fn vrf_penalties_marked_when_offenders_missing_from_topology() {
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
        let outcome = applier.apply_vrf_penalties(5);

        assert_eq!(outcome.applied, 0, "no validator located to jail");
        assert_eq!(outcome.pending, 0, "record should not remain pending");

        let view = state.world.vrf_epochs.view();
        let updated = view.get(&vrf_record.epoch).expect("vrf record present");
        assert!(updated.penalties_applied);
        assert_eq!(updated.penalties_applied_at_height, Some(5));
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
        let outcome = applier.apply_consensus_penalties(5)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);
        assert_eq!(outcome.pending, 0);

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
        let outcome = applier.apply_consensus_penalties(5)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);
        assert_eq!(outcome.pending, 0);

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
        let outcome = applier.apply_consensus_penalties(12)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);
        assert_eq!(outcome.pending, 1);

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
        let outcome = applier.apply_consensus_penalties(5)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);
        assert_eq!(outcome.pending, 1);

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
        let outcome = applier.apply_consensus_penalties(5)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);
        assert_eq!(outcome.pending, 1);

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
        let outcome = applier.apply_consensus_penalties(5)?;
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.slashed, 0);
        assert_eq!(outcome.pending, 1);

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence present");
        assert!(!updated.penalty_applied);
        assert_eq!(updated.penalty_applied_at_height, None);

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

        let domain: DomainId = "test".parse().expect("domain id");
        let validator: AccountId = AccountId::new(key_pair.public_key().clone());
        let escrow_key_pair = KeyPair::random();
        let escrow_account: AccountId = AccountId::new(escrow_key_pair.public_key().clone());
        let stake_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "test".parse().unwrap(),
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
        let outcome = applier.apply_consensus_penalties(5)?;
        assert_eq!(outcome.applied, 1);
        assert_eq!(outcome.slashed, 1);
        assert_eq!(outcome.pending, 0);

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
        let locator = applier
            .locate_validator_in_roster_cached(0, &[peer], &map)
            .expect("locator resolved");
        assert_eq!(locator.lane_id, LaneId::new(1));
        assert_eq!(locator.validator, validator);
    }
}
