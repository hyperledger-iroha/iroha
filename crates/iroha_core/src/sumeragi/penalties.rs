//! Penalty enforcement for `NPoS`: VRF non-participation and consensus evidence slashing.

use std::collections::{BTreeMap, BTreeSet};

use eyre::{Result, WrapErr, eyre};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    block::{
        consensus::{Evidence, EvidenceKind, EvidencePayload, EvidenceRecord},
        consensus_v2::{ConsensusMode, HeightContext},
    },
    consensus::{
        NposConsensusEffects, NposConsensusSlashAction, NposMarkConsensusEvidenceAppliedAction,
        NposMarkVrfPenaltiesAppliedAction, NposPenaltyAction, VrfEpochRecord,
    },
    nexus::{DataSpaceCatalog, LaneId, PublicLaneValidatorStatus},
    prelude::{AccountId, PeerId},
    transaction::TransactionSubmissionReceipt,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;

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

    /// Derive only deterministic penalty actions from pre-block state.
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
            crate::sumeragi::resolve_npos_slashing_delay_blocks_from_world(&world)
                .ok_or_else(|| eyre!("NPoS penalty derivation requires signed NPoS parameters"))?
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

        let validator_map = self.build_validator_locator_map();
        let mut actions = Vec::new();
        for (key, record) in pending {
            let is_censorship =
                matches!(&record.evidence.payload, EvidencePayload::Censorship { .. });
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

fn height_context_for_evidence(
    state: &State,
    evidence: &Evidence,
    recorded_at_height: u64,
) -> Result<Option<HeightContext>> {
    let Some(height) = evidence_context_height(evidence, recorded_at_height) else {
        return Ok(None);
    };
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
    if &artifact.height_context.chain_id != state.chain_id_ref() {
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
    if evidence_context_height(evidence, recorded_at_height) != Some(context.height) {
        return false;
    }
    match (&evidence.kind, &evidence.payload) {
        (
            EvidenceKind::DoublePrepare | EvidenceKind::DoubleCommit,
            EvidencePayload::DoubleVote { v1, v2 },
        ) => {
            let phase_matches_kind = matches!(
                (evidence.kind, v1.phase, v2.phase),
                (
                    EvidenceKind::DoublePrepare,
                    iroha_data_model::block::consensus::CertPhase::Prepare,
                    iroha_data_model::block::consensus::CertPhase::Prepare
                ) | (
                    EvidenceKind::DoubleCommit,
                    iroha_data_model::block::consensus::CertPhase::Commit,
                    iroha_data_model::block::consensus::CertPhase::Commit
                ) | (
                    EvidenceKind::DoubleCommit,
                    iroha_data_model::block::consensus::CertPhase::Commit,
                    iroha_data_model::block::consensus::CertPhase::Prepare
                ) | (
                    EvidenceKind::DoubleCommit,
                    iroha_data_model::block::consensus::CertPhase::Prepare,
                    iroha_data_model::block::consensus::CertPhase::Commit
                )
            );
            let conflicting_subject = v1.block_hash != v2.block_hash
                || (v1.phase == iroha_data_model::block::consensus::CertPhase::Commit
                    && v2.phase == iroha_data_model::block::consensus::CertPhase::Commit
                    && (v1.parent_state_root != v2.parent_state_root
                        || v1.post_state_root != v2.post_state_root));
            v1.height == context.height
                && v2.height == context.height
                && v1.epoch == context.epoch
                && v2.epoch == context.epoch
                && v1.view == v2.view
                && v1.signer == v2.signer
                && phase_matches_kind
                && conflicting_subject
        }
        (EvidenceKind::InvalidProposal, EvidencePayload::InvalidProposal { proposal, .. }) => {
            proposal.header.height == context.height && proposal.header.epoch == context.epoch
        }
        (EvidenceKind::InvalidQc, EvidencePayload::InvalidQc { certificate, .. }) => {
            certificate.height == context.height && certificate.epoch == context.epoch
        }
        (EvidenceKind::Censorship, EvidencePayload::Censorship { tx_hash, receipts }) => {
            !receipts.is_empty()
                && receipts
                    .iter()
                    .all(|receipt| receipt.payload.tx_hash == *tx_hash)
        }
        (
            EvidenceKind::SumeragiV2Equivocation,
            EvidencePayload::SumeragiV2Equivocation(v2_evidence),
        ) => {
            &v2_evidence.context == context
                && super::evidence::validate_v2_equivocation(v2_evidence).is_ok()
        }
        _ => false,
    }
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

fn evidence_context_height(evidence: &Evidence, recorded_at_height: u64) -> Option<u64> {
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, .. } => Some(v1.height),
        EvidencePayload::InvalidProposal { proposal, .. } => Some(proposal.header.height),
        EvidencePayload::InvalidQc { certificate, .. } => Some(certificate.height),
        EvidencePayload::Censorship { receipts, .. } => {
            censorship_anchor_height(receipts, recorded_at_height)
        }
        EvidencePayload::SumeragiV2Equivocation(evidence) => Some(evidence.context.height),
    }
}

fn offender_indices(
    evidence: &Evidence,
    recorded_at_height: u64,
    context: &HeightContext,
) -> Vec<ValidatorIndex> {
    if evidence_context_height(evidence, recorded_at_height) != Some(context.height) {
        return Vec::new();
    }
    let roster_len = context.roster.len();
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, .. } if v1.epoch == context.epoch => {
            canonical_indices([v1.signer], roster_len)
        }
        EvidencePayload::InvalidProposal { proposal, .. }
            if proposal.header.epoch == context.epoch =>
        {
            canonical_indices([proposal.header.proposer], roster_len)
        }
        EvidencePayload::InvalidQc { certificate, .. } if certificate.epoch == context.epoch => {
            canonical_indices(
                bitmap_indices(&certificate.aggregate.signers_bitmap),
                roster_len,
            )
        }
        EvidencePayload::Censorship { .. } => canonical_indices([context.leader(0)], roster_len),
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
            canonical_indices([signer], roster_len)
        }
        EvidencePayload::DoubleVote { .. }
        | EvidencePayload::InvalidProposal { .. }
        | EvidencePayload::InvalidQc { .. } => Vec::new(),
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
) -> Result<Option<Quantity>> {
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

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        ChainId,
        block::{
            BlockHeader, SignedBlock,
            consensus::{
                CertPhase, Evidence, EvidenceKind, EvidencePayload, EvidenceRecord, QcVote,
            },
            consensus_v2::{
                BlockSubject, ConsensusMode as V2ConsensusMode, ConsensusRound,
                DataAvailabilityLayout, DualQuorum, ExecutionCommitment, GlobalPhase,
                HeightContext, PayloadEncoding, QuorumCertificate, ValidatorPower,
                finality::V2FinalityArtifact,
            },
        },
        metadata::Metadata,
        nexus::{LaneId, PublicLaneValidatorRecord, PublicLaneValidatorStatus},
        parameter::{Parameter, system::SumeragiNposParameters},
        prelude::{AccountId, PeerId},
        transaction::{
            TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
            signed::SignedTransaction,
        },
    };
    use iroha_primitives::numeric::Quantity;
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        sumeragi::{consensus::default_chain_order_hash, evidence::evidence_key},
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

    fn test_vote(
        signer: ValidatorIndex,
        height: u64,
        view: u64,
        epoch: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> QcVote {
        QcVote {
            phase: CertPhase::Prepare,
            block_hash,
            parent_state_root: Hash::prehashed([0; Hash::LENGTH]),
            post_state_root: Hash::prehashed([1; Hash::LENGTH]),
            height,
            view,
            epoch,
            chain_order_hash: default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer,
            bls_sig: vec![0xA5; 96],
        }
    }

    fn double_prepare_evidence(
        signer: ValidatorIndex,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> Evidence {
        Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: test_vote(signer, height, view, epoch, test_block_hash(0x31)),
                v2: test_vote(signer, height, view, epoch, test_block_hash(0x32)),
            },
        }
    }

    fn height_one_context(
        chain_id: ChainId,
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
            chain_id,
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
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
            leader_seed: [0x42; 32],
        }
    }

    fn install_height_one_artifact(state: &State, roster: &[PeerId]) -> HeightContext {
        install_height_one_artifact_with_chain(state, roster, state.chain_id_ref().clone())
    }

    fn install_height_one_artifact_with_chain(
        state: &State,
        roster: &[PeerId],
        chain_id: ChainId,
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

        let context = height_one_context(chain_id, roster, block.hash());
        let subject = BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal wire"),
        };
        let execution_commitment = ExecutionCommitment::without_topups(
            Hash::new(b"penalties fixture parent state"),
            Hash::new(b"penalties fixture post state"),
            Hash::new(b"penalties fixture ordinary writes"),
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
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
            consensus_admitted_at_height: None,
        };
        let mut block = state.world.consensus_evidence.block();
        block.insert(key.clone(), record);
        block.commit();
        key
    }

    fn add_validator_record(state: &State, peer: &PeerId) -> AccountId {
        let validator = AccountId::new(peer.public_key().clone());
        let record = PublicLaneValidatorRecord {
            lane_id: LaneId::SINGLE,
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
        block.insert((LaneId::SINGLE, validator.clone()), record);
        block.commit();
        validator
    }

    fn install_zero_delay_npos(state: &State) {
        let mut parameters = state.world.parameters.block();
        let npos = SumeragiNposParameters {
            slashing_delay_blocks: 0,
            ..SumeragiNposParameters::default()
        };
        parameters.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
        parameters.commit();
    }

    fn test_censorship_receipt(height: u64) -> TransactionSubmissionReceipt {
        let tx_hash =
            HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([0x77; 32]));
        let signer = checked_keypair();
        TransactionSubmissionReceipt::sign(
            TransactionSubmissionReceiptPayload {
                tx_hash,
                entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
                signed_transaction_hash: Some(tx_hash),
                submitted_at_ms: 1,
                submitted_at_height: height,
                signer: signer.public_key().clone(),
            },
            &signer,
        )
    }

    #[test]
    fn canonical_artifact_context_is_the_only_roster_authority() {
        let state = fresh_state();
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let mutable_fallback = PeerId::new(checked_keypair().public_key().clone());
        set_commit_topology(&state, vec![mutable_fallback.clone()]);

        let evidence = double_prepare_evidence(1, 1, 99, 0);
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
    fn missing_artifact_fails_closed_without_mutable_topology_fallback() {
        let state = fresh_state();
        install_zero_delay_npos(&state);
        set_commit_topology(&state, roster());
        let evidence = double_prepare_evidence(1, 1, 0, 0);

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
        let evidence = double_prepare_evidence(0, 2, 0, 0);
        assert!(
            height_context_for_evidence(&state, &evidence, 1)
                .expect("future evidence rejection is deterministic")
                .is_none()
        );
    }

    #[test]
    fn artifact_context_rejects_epoch_mismatch() {
        let state = fresh_state();
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        let evidence = double_prepare_evidence(0, 1, 0, 1);

        assert!(
            height_context_for_evidence(&state, &evidence, 1)
                .expect("canonical artifact lookup succeeds")
                .is_none(),
            "legacy evidence metadata must not override the frozen v2 epoch"
        );
    }

    #[test]
    fn artifact_from_another_chain_fails_closed() {
        let state = fresh_state();
        let frozen_roster = roster();
        install_height_one_artifact_with_chain(
            &state,
            &frozen_roster,
            ChainId::from("wrong-chain"),
        );

        let error = height_context_for_evidence(&state, &double_prepare_evidence(0, 1, 0, 0), 1)
            .expect_err("cross-chain provenance must never authorize a slash");
        assert!(error.to_string().contains("belongs to another chain"));
    }

    #[test]
    fn artifact_context_rejects_internally_mismatched_double_vote_epoch() {
        let state = fresh_state();
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        let mut evidence = double_prepare_evidence(0, 1, 0, 0);
        let EvidencePayload::DoubleVote { v2, .. } = &mut evidence.payload else {
            unreachable!("fixture is double-vote evidence");
        };
        v2.epoch = 1;

        assert!(
            height_context_for_evidence(&state, &evidence, 1)
                .expect("canonical artifact lookup succeeds")
                .is_none()
        );
    }

    #[test]
    fn artifact_context_rejects_cross_view_double_vote_claim() {
        let state = fresh_state();
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        let mut evidence = double_prepare_evidence(0, 1, 0, 0);
        let EvidencePayload::DoubleVote { v2, .. } = &mut evidence.payload else {
            unreachable!("fixture is double-vote evidence");
        };
        v2.view = 1;

        assert!(
            height_context_for_evidence(&state, &evidence, 1)
                .expect("canonical artifact lookup succeeds")
                .is_none(),
            "votes from different rounds are not an equivocation in the v2 reducer"
        );
    }

    #[test]
    fn artifact_context_rejects_different_signers_in_double_vote_claim() {
        let state = fresh_state();
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        let mut evidence = double_prepare_evidence(0, 1, 0, 0);
        let EvidencePayload::DoubleVote { v2, .. } = &mut evidence.payload else {
            unreachable!("fixture is double-vote evidence");
        };
        v2.signer = 1;

        assert!(
            height_context_for_evidence(&state, &evidence, 1)
                .expect("canonical artifact lookup succeeds")
                .is_none(),
            "a crafted pair must not transfer one validator's fault to another"
        );
    }

    #[test]
    fn artifact_context_rejects_kind_payload_mismatch() {
        let state = fresh_state();
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        let mut evidence = double_prepare_evidence(0, 1, 0, 0);
        evidence.kind = EvidenceKind::InvalidQc;

        assert!(
            height_context_for_evidence(&state, &evidence, 1)
                .expect("canonical artifact lookup succeeds")
                .is_none()
        );
    }

    #[test]
    fn corrupt_finality_artifact_propagates_a_fail_closed_error() {
        let state = fresh_state();
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        let artifact_path = state
            .kura()
            .sumeragi_v2_storage_root()
            .parent()
            .expect("v2 storage root has block-directory parent")
            .join("v2_finality")
            .join("00000000000000000001.norito");
        std::fs::write(&artifact_path, b"not a Norito finality artifact")
            .expect("corrupt test artifact");

        let error = height_context_for_evidence(&state, &double_prepare_evidence(0, 1, 0, 0), 1)
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
        let view_zero = double_prepare_evidence(2, 1, 0, 0);
        let late_view = double_prepare_evidence(2, 1, u64::MAX, 0);

        assert_eq!(offender_indices(&view_zero, 1, &context), vec![2]);
        assert_eq!(offender_indices(&late_view, 1, &context), vec![2]);
    }

    #[test]
    fn npos_voting_power_does_not_remap_evidence_signer_indices() {
        let state = fresh_state();
        let frozen_roster = roster();
        let mut context = height_one_context(
            state.chain_id_ref().clone(),
            &frozen_roster,
            test_block_hash(0x90),
        );
        context.mode = V2ConsensusMode::Npos;
        for (entry, power) in context.roster.iter_mut().zip([1, 100, 3, 7]) {
            entry.power = power;
        }
        context.quorum = DualQuorum::from_roster(&context.roster).expect("weighted quorum");
        context.validate().expect("valid weighted NPoS context");

        let evidence = double_prepare_evidence(1, 1, 47, 0);
        assert_eq!(offender_indices(&evidence, 1, &context), vec![1]);
    }

    #[test]
    fn canonical_indices_filter_duplicates_and_out_of_range_signers() {
        assert_eq!(canonical_indices([3, 1, 3, 7, u32::MAX], 4), vec![1, 3]);
        assert!(canonical_indices([0], 0).is_empty());
        assert_eq!(bitmap_indices(&[0b1000_0101]), vec![0, 2, 7]);
    }

    #[test]
    fn censorship_attribution_uses_the_frozen_v2_leader() {
        let state = fresh_state();
        let frozen_roster = roster();
        let context = install_height_one_artifact(&state, &frozen_roster);
        let receipt = test_censorship_receipt(1);
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: receipt.payload.tx_hash,
                receipts: vec![receipt],
            },
        };

        assert_eq!(
            offender_indices(&evidence, 1, &context),
            vec![context.leader(0)]
        );
    }

    #[test]
    fn derived_slash_targets_frozen_roster_even_when_live_topology_diverges() {
        let state = fresh_state();
        install_zero_delay_npos(&state);
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        set_commit_topology(
            &state,
            vec![PeerId::new(checked_keypair().public_key().clone())],
        );
        let offender = frozen_roster[1].clone();
        let validator = add_validator_record(&state, &offender);
        let evidence = double_prepare_evidence(1, 1, 37, 0);
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
    fn epoch_mismatch_stays_pending_without_marking_or_slashing() {
        let state = fresh_state();
        install_zero_delay_npos(&state);
        let frozen_roster = roster();
        install_height_one_artifact(&state, &frozen_roster);
        add_validator_record(&state, &frozen_roster[0]);
        let key = insert_evidence(&state, double_prepare_evidence(0, 1, 0, 9), 1);
        let actions = PenaltyApplier::from_parts(
            &state,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(2, std::iter::empty())
        .expect("epoch mismatch is a closed, deterministic rejection")
        .penalty_actions;

        assert!(actions.is_empty());
        let view = state.world.consensus_evidence.view();
        let record = view.get(&key).expect("evidence remains persisted");
        assert!(!record.penalty_applied);
    }
}
