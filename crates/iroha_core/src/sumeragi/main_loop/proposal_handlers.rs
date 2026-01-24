//! Proposal- and block-created message handling.

use iroha_logger::prelude::*;

use crate::sumeragi::rbc_store::SessionKey;

use super::proposals::block_payload_bytes;
use super::*;

pub(super) fn invalid_proposal_evidence(
    proposal: crate::sumeragi::consensus::Proposal,
    reason: String,
) -> crate::sumeragi::consensus::Evidence {
    crate::sumeragi::consensus::Evidence {
        kind: crate::sumeragi::consensus::EvidenceKind::InvalidProposal,
        payload: crate::sumeragi::consensus::EvidencePayload::InvalidProposal { proposal, reason },
    }
}

#[inline]
fn stale_height(height: u64, committed_height: u64) -> bool {
    height <= committed_height
}

pub(super) fn allow_stale_block_created(da_enabled: bool, missing_request: bool) -> bool {
    da_enabled || missing_request
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct MissingProposalContext {
    pub(super) hint_seen: bool,
    pub(super) proposal_cached: bool,
    pub(super) pending_blocks: usize,
    pub(super) pending_match: bool,
    pub(super) pending_gate: Option<GateReason>,
    pub(super) pending_validation: Option<ValidationStatus>,
    pub(super) pending_age_ms: Option<u64>,
    pub(super) commit_inflight: Option<(u64, u64)>,
    pub(super) forced_view_after_timeout: Option<(u64, u64)>,
    pub(super) da_enabled: bool,
}

impl Actor {
    pub(crate) fn expected_collector_params_for_advert(
        &self,
        advert: &super::message::ConsensusParamsAdvert,
    ) -> (u64, u64) {
        let (expected_k, expected_r) = advert.membership.map_or_else(
            || self.collector_plan_params(),
            |membership| self.collector_plan_params_for_height(membership.height),
        );
        (
            u64::try_from(expected_k).unwrap_or(u64::MAX),
            u64::from(expected_r),
        )
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_consensus_params(
        &self,
        advert: super::message::ConsensusParamsAdvert,
    ) -> Result<()> {
        let (expected_k, expected_r) = self.expected_collector_params_for_advert(&advert);
        if u64::from(advert.collectors_k) != expected_k {
            warn!(
                advertised = advert.collectors_k,
                expected = expected_k,
                "collector parameter mismatch"
            );
        }
        if u64::from(advert.redundant_send_r) != expected_r {
            warn!(
                advertised = advert.redundant_send_r,
                expected = expected_r,
                "redundant send parameter mismatch"
            );
        }
        #[cfg(feature = "telemetry")]
        self.telemetry.set_collectors_params(
            u64::from(advert.collectors_k),
            u64::from(advert.redundant_send_r),
        );
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_proposal_hint(
        &mut self,
        hint: super::message::ProposalHint,
    ) -> Result<()> {
        let highest_qc = hint.highest_qc;
        let height = hint.height;
        let view = hint.view;
        let state_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        if stale_height(height, state_height) {
            debug!(
                height,
                view,
                state_height,
                block = %hint.block_hash,
                "dropping proposal hint at or below committed height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ProposalHint,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleHeight,
            );
            self.subsystems
                .propose
                .proposal_cache
                .prune_height_leq(state_height);
            return Ok(());
        }
        if self.drop_stale_view(height, view, "ProposalHint") {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ProposalHint,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
            return Ok(());
        }
        let expected_highest_height = height.saturating_sub(1);
        if highest_qc.height != expected_highest_height {
            warn!(
                height,
                view,
                highest_height = highest_qc.height,
                expected_highest_height,
                block = %highest_qc.subject_block_hash,
                "dropping proposal hint: highest QC height does not match proposal height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ProposalHint,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::HighestQcMismatch,
            );
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(highest_qc.height);
        if highest_qc.epoch != expected_epoch {
            warn!(
                height,
                view,
                highest_height = highest_qc.height,
                highest_epoch = highest_qc.epoch,
                expected_epoch,
                block = %highest_qc.subject_block_hash,
                "dropping proposal hint: highest QC epoch mismatch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ProposalHint,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
            );
            return Ok(());
        }
        let committed_height = self.latest_committed_qc().map_or(0, |qc| qc.height);

        if let Some(existing) = self
            .subsystems
            .propose
            .proposal_cache
            .get_hint(height, view)
        {
            let conflict = existing.block_hash != hint.block_hash
                || existing.height != hint.height
                || existing.view != hint.view
                || existing.highest_qc != highest_qc;
            if conflict {
                let committed_conflict = usize::try_from(existing.highest_qc.height)
                    .ok()
                    .and_then(NonZeroUsize::new)
                    .and_then(|nz| self.kura.get_block(nz))
                    .map(|block| block.hash())
                    .is_some_and(|hash| hash != existing.highest_qc.subject_block_hash)
                    && existing.highest_qc.height <= committed_height;
                if committed_conflict {
                    info!(
                        height,
                        view,
                        existing_block = %existing.block_hash,
                        incoming_block = %hint.block_hash,
                        existing_highest = %existing.highest_qc.subject_block_hash,
                        incoming_highest = %highest_qc.subject_block_hash,
                        "replacing cached proposal hint that conflicts with committed tip"
                    );
                } else {
                    info!(
                        height,
                        view,
                        existing_block = %existing.block_hash,
                        incoming_block = %hint.block_hash,
                        existing_highest = %existing.highest_qc.subject_block_hash,
                        incoming_highest = %highest_qc.subject_block_hash,
                        "ignoring conflicting proposal hint for cached slot"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::ProposalHint,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::Duplicate,
                    );
                    return Ok(());
                }
            }
        }

        if let Some(stored_height) = self
            .kura
            .get_block_height_by_hash(highest_qc.subject_block_hash)
            .and_then(|nz| u64::try_from(nz.get()).ok())
        {
            if stored_height != highest_qc.height {
                warn!(
                        height,
                        view,
                    stored_height,
                    highest_height = highest_qc.height,
                    block = %highest_qc.subject_block_hash,
                    "dropping proposal hint: highest QC hash stored at different height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::ProposalHint,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                return Ok(());
            }
            let stored_hash = usize::try_from(stored_height)
                .ok()
                .and_then(NonZeroUsize::new)
                .and_then(|nz| self.kura.get_block(nz))
                .map(|block| block.hash());
            if highest_qc.height <= committed_height
                && stored_hash.is_some_and(|hash| hash != highest_qc.subject_block_hash)
            {
                info!(
                        height,
                        view,
                    committed_height,
                    committed_hash = %stored_hash.unwrap_or(highest_qc.subject_block_hash),
                    highest_hash = %highest_qc.subject_block_hash,
                    "dropping proposal hint: highest QC conflicts with committed block at height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::ProposalHint,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::CommitConflict,
                );
                return Ok(());
            }
        } else if highest_qc.height <= committed_height {
            info!(
                height,
                view,
                committed_height,
                highest_height = highest_qc.height,
                block = %highest_qc.subject_block_hash,
                "dropping proposal hint: highest QC block missing locally for committed height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ProposalHint,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::MissingHighestQc,
            );
            return Ok(());
        } else {
            info!(
                height,
                view,
                highest_height = highest_qc.height,
                block = %highest_qc.subject_block_hash,
                "caching proposal hint without local highest QC block; awaiting sync"
            );
        }
        if let Some((local_height, local_view)) =
            self.local_block_height_view(highest_qc.subject_block_hash)
        {
            if local_height != highest_qc.height {
                warn!(
                        height,
                        view,
                    highest_height = highest_qc.height,
                    local_height,
                    block = %highest_qc.subject_block_hash,
                    "dropping proposal hint: highest QC height does not match local block height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::ProposalHint,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                return Ok(());
            }
            if local_view != highest_qc.view {
                warn!(
                        height,
                        view,
                        highest_height = highest_qc.height,
                    highest_view = highest_qc.view,
                    local_view,
                    block = %highest_qc.subject_block_hash,
                    "dropping proposal hint: highest QC view does not match local block view"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::ProposalHint,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                return Ok(());
            }
        }

        self.update_prf_context(height, view);
        if !self.ensure_highest_qc_extends_locked(height, view, highest_qc, "proposal hint") {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ProposalHint,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return Ok(());
        }
        let should_update_highest = self.highest_qc.is_none_or(|current| {
            let incoming = (highest_qc.height, highest_qc.view);
            let existing = (current.height, current.view);
            let promotes_phase = incoming == existing
                && highest_qc.phase == crate::sumeragi::consensus::Phase::Commit
                && current.phase != crate::sumeragi::consensus::Phase::Commit;
            incoming > existing || promotes_phase
        });
        if should_update_highest {
            self.highest_qc = Some(highest_qc);
            super::status::set_highest_qc(highest_qc.height, highest_qc.view);
            super::status::set_highest_qc_hash(highest_qc.subject_block_hash);
        } else {
            debug!(
                height,
                view,
                highest_height = highest_qc.height,
                highest_view = highest_qc.view,
                current_height = self.highest_qc.map(|qc| qc.height),
                current_view = self.highest_qc.map(|qc| qc.view),
                "skipping highest QC update for stale proposal hint"
            );
        }
        self.record_phase_sample(PipelinePhase::Propose, hint.height, hint.view);
        let hint_block = hint.block_hash;
        self.subsystems.propose.proposal_cache.insert_hint(hint);
        if self
            .subsystems
            .propose
            .proposals_seen
            .insert((height, view))
        {
            iroha_logger::info!(
                height,
                view,
                block = %hint_block,
                "observed proposal hint for view"
            );
        }
        self.prune_proposals_seen_horizon(state_height);
        Ok(())
    }

    pub(super) fn update_prf_context(&self, height: u64, view: u64) {
        let (consensus_mode, _, prf_seed) = self.consensus_context_for_height(height);
        if let ConsensusMode::Npos = consensus_mode {
            if let Some(seed) = prf_seed {
                super::status::set_prf_context(seed, height, view);
                #[cfg(feature = "telemetry")]
                self.telemetry.set_prf_context(Some(seed), height, view);
            }
        }
    }

    pub(super) fn ensure_highest_qc_extends_locked(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: super::consensus::QcHeaderRef,
        source: &'static str,
    ) -> bool {
        if !self.highest_qc_extends_locked(highest_qc) {
            if let Some(new_lock) = realign_locked_to_committed_if_extends(
                self.locked_qc,
                self.latest_committed_qc(),
                highest_qc,
                |hash, height| self.parent_hash_for(hash, height),
            ) {
                if self.locked_qc != Some(new_lock) {
                    info!(
                        height,
                        view,
                        highest_height = highest_qc.height,
                        highest_hash = %highest_qc.subject_block_hash,
                        locked_height = new_lock.height,
                        locked_hash = %new_lock.subject_block_hash,
                        context = source,
                        "resetting locked QC to committed chain before caching"
                    );
                    self.locked_qc = Some(new_lock);
                    super::status::set_locked_qc(
                        new_lock.height,
                        new_lock.view,
                        Some(new_lock.subject_block_hash),
                    );
                }
            }
        }
        if self.highest_qc_extends_locked(highest_qc) {
            return true;
        }
        debug!(
            height,
            view,
            highest_height = highest_qc.height,
            highest_hash = ?highest_qc.subject_block_hash,
            locked_height = ?self.locked_qc.map(|qc| qc.height),
            locked_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
            context = source,
            "dropping highest QC that does not extend locked chain"
        );
        false
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::Proposal,
    ) -> Result<()> {
        let height = proposal.header.height;
        let view = proposal.header.view;
        let committed_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        if stale_height(height, committed_height) {
            debug!(
                height,
                view,
                committed_height,
                payload = %proposal.payload_hash,
                "dropping proposal at or below committed height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleHeight,
            );
            self.subsystems
                .propose
                .proposal_cache
                .prune_height_leq(committed_height);
            return Ok(());
        }
        if self.drop_stale_view(height, view, "Proposal") {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(height);
        if proposal.header.epoch != expected_epoch {
            warn!(
                height,
                view,
                expected_epoch,
                proposal_epoch = proposal.header.epoch,
                payload = %proposal.payload_hash,
                "dropping proposal with mismatched epoch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
            );
            return Ok(());
        }
        let highest_qc = proposal.header.highest_qc;
        let expected_highest_height = height.saturating_sub(1);
        if highest_qc.height != expected_highest_height {
            warn!(
                height,
                view,
                highest_height = highest_qc.height,
                expected_highest_height,
                block = %highest_qc.subject_block_hash,
                payload = %proposal.payload_hash,
                "dropping proposal: highest QC height does not match proposal height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::HighestQcMismatch,
            );
            return Ok(());
        }
        let expected_highest_epoch = self.epoch_for_height(highest_qc.height);
        if highest_qc.epoch != expected_highest_epoch {
            warn!(
                height,
                view,
                highest_height = highest_qc.height,
                highest_epoch = highest_qc.epoch,
                expected_highest_epoch,
                block = %highest_qc.subject_block_hash,
                payload = %proposal.payload_hash,
                "dropping proposal: highest QC epoch mismatch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
            );
            return Ok(());
        }
        if proposal.header.parent_hash != highest_qc.subject_block_hash {
            warn!(
                height,
                view,
                parent = %proposal.header.parent_hash,
                highest_hash = %highest_qc.subject_block_hash,
                payload = %proposal.payload_hash,
                "dropping proposal: parent hash does not match highest QC"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::HighestQcMismatch,
            );
            return Ok(());
        }
        let committed_qc_height = self.latest_committed_qc().map_or(0, |qc| qc.height);
        if let Some(stored_height) = self
            .kura
            .get_block_height_by_hash(highest_qc.subject_block_hash)
            .and_then(|nz| u64::try_from(nz.get()).ok())
        {
            if stored_height != highest_qc.height {
                warn!(
                        height,
                        view,
                    stored_height,
                    highest_height = highest_qc.height,
                    block = %highest_qc.subject_block_hash,
                    payload = %proposal.payload_hash,
                    "dropping proposal: highest QC hash stored at different height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Proposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                return Ok(());
            }
            let stored_hash = usize::try_from(stored_height)
                .ok()
                .and_then(NonZeroUsize::new)
                .and_then(|nz| self.kura.get_block(nz))
                .map(|block| block.hash());
            if highest_qc.height <= committed_qc_height
                && stored_hash.is_some_and(|hash| hash != highest_qc.subject_block_hash)
            {
                info!(
                        height,
                        view,
                        committed_height = committed_qc_height,
                    committed_hash = %stored_hash.unwrap_or(highest_qc.subject_block_hash),
                    highest_hash = %highest_qc.subject_block_hash,
                    payload = %proposal.payload_hash,
                    "dropping proposal: highest QC conflicts with committed block at height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Proposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::CommitConflict,
                );
                return Ok(());
            }
        } else if highest_qc.height <= committed_qc_height {
            info!(
                height,
                view,
                committed_height = committed_qc_height,
                highest_height = highest_qc.height,
                block = %highest_qc.subject_block_hash,
                payload = %proposal.payload_hash,
                "dropping proposal: highest QC block missing locally for committed height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::MissingHighestQc,
            );
            return Ok(());
        } else {
            info!(
                height,
                view,
                highest_height = highest_qc.height,
                block = %highest_qc.subject_block_hash,
                payload = %proposal.payload_hash,
                "caching proposal without local highest QC block; awaiting sync"
            );
        }
        if let Some((local_height, local_view)) =
            self.local_block_height_view(highest_qc.subject_block_hash)
        {
            if local_height != highest_qc.height {
                warn!(
                        height,
                        view,
                    highest_height = highest_qc.height,
                    local_height,
                    block = %highest_qc.subject_block_hash,
                    payload = %proposal.payload_hash,
                    "dropping proposal: highest QC height does not match local block height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Proposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                return Ok(());
            }
            if local_view != highest_qc.view {
                warn!(
                        height,
                        view,
                        highest_height = highest_qc.height,
                        highest_view = highest_qc.view,
                    local_view,
                    block = %highest_qc.subject_block_hash,
                    payload = %proposal.payload_hash,
                    "dropping proposal: highest QC view does not match local block view"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Proposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                return Ok(());
            }
        }
        self.update_prf_context(height, view);
        if !self.ensure_highest_qc_extends_locked(height, view, highest_qc, "proposal") {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return Ok(());
        }
        let should_update_highest = self.highest_qc.is_none_or(|current| {
            let incoming = (highest_qc.height, highest_qc.view);
            let existing = (current.height, current.view);
            let promotes_phase = incoming == existing
                && highest_qc.phase == crate::sumeragi::consensus::Phase::Commit
                && current.phase != crate::sumeragi::consensus::Phase::Commit;
            incoming > existing || promotes_phase
        });
        if should_update_highest {
            self.highest_qc = Some(highest_qc);
            super::status::set_highest_qc(highest_qc.height, highest_qc.view);
            super::status::set_highest_qc_hash(highest_qc.subject_block_hash);
        } else {
            debug!(
                height,
                view,
                highest_height = highest_qc.height,
                highest_view = highest_qc.view,
                current_height = self.highest_qc.map(|qc| qc.height),
                current_view = self.highest_qc.map(|qc| qc.view),
                "skipping highest QC update for stale proposal"
            );
        }
        self.record_phase_sample(PipelinePhase::Propose, height, view);
        self.note_proposal_seen(height, view, proposal.payload_hash);
        self.subsystems
            .propose
            .proposal_cache
            .insert_proposal(proposal);
        Ok(())
    }

    pub(super) fn note_proposal_seen(&mut self, height: u64, view: u64, payload_hash: Hash) {
        if self
            .subsystems
            .propose
            .proposals_seen
            .insert((height, view))
        {
            iroha_logger::info!(
                height,
                view,
                payload = %payload_hash,
                "observed proposal for view"
            );
        }
        self.prune_proposals_seen_horizon(self.last_committed_height);
    }

    pub(super) fn prune_proposals_seen_horizon(&mut self, committed_height: u64) {
        let highest_commit = self
            .highest_qc
            .filter(|qc| qc.phase == crate::sumeragi::consensus::Phase::Commit);
        let anchor_height =
            highest_commit.map_or(committed_height, |qc| qc.height.max(committed_height));
        let active_height = anchor_height.saturating_add(1);
        let min_height = active_height.saturating_sub(super::PROPOSALS_SEEN_HEIGHT_WINDOW);
        let max_height = active_height.saturating_add(super::PROPOSALS_SEEN_HEIGHT_WINDOW);
        let current_view = self.phase_tracker.current_view(active_height);
        let max_view =
            current_view.map(|view| view.saturating_add(super::PROPOSALS_SEEN_VIEW_WINDOW));
        self.subsystems
            .propose
            .proposals_seen
            .retain(|(entry_height, entry_view)| {
                if *entry_height <= committed_height
                    || *entry_height < min_height
                    || *entry_height > max_height
                {
                    return false;
                }
                if *entry_height == active_height {
                    if let Some(current_view) = current_view {
                        if *entry_view < current_view {
                            return false;
                        }
                    }
                    if let Some(max_view) = max_view {
                        if *entry_view > max_view {
                            return false;
                        }
                    }
                }
                true
            });
    }

    pub(super) fn missing_proposal_context(
        &self,
        height: u64,
        view: u64,
    ) -> MissingProposalContext {
        let hint_seen = self
            .subsystems
            .propose
            .proposal_cache
            .get_hint(height, view)
            .is_some();
        let proposal_cached = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view)
            .is_some();
        let pending = self
            .pending
            .pending_blocks
            .values()
            .find(|pending| pending.height == height && pending.view == view);
        let pending_match = pending.is_some();
        let pending_gate = pending.and_then(|pending| pending.last_gate);
        let pending_validation = pending.map(|pending| pending.validation_status);
        let pending_age_ms = pending.map(|pending| {
            u64::try_from(pending.inserted_at.elapsed().as_millis()).unwrap_or(u64::MAX)
        });
        let commit_inflight = self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .map(|inflight| (inflight.pending.height, inflight.pending.view));
        let da_enabled = sumeragi_da_enabled(&self.state);
        MissingProposalContext {
            hint_seen,
            proposal_cached,
            pending_blocks: self.pending.pending_blocks.len(),
            pending_match,
            pending_gate,
            pending_validation,
            pending_age_ms,
            commit_inflight,
            forced_view_after_timeout: self.subsystems.propose.forced_view_after_timeout,
            da_enabled,
        }
    }

    pub(super) fn assert_proposal_seen(&self, height: u64, view: u64, reason: &'static str) {
        if height <= 1 {
            return;
        }
        if !self
            .subsystems
            .propose
            .proposals_seen
            .contains(&(height, view))
        {
            let context = self.missing_proposal_context(height, view);
            iroha_logger::error!(
                height,
                view,
                trigger = reason,
                hint_seen = context.hint_seen,
                proposal_cached = context.proposal_cached,
                pending_blocks = context.pending_blocks,
                pending_match = context.pending_match,
                pending_gate = ?context.pending_gate,
                pending_validation = ?context.pending_validation,
                pending_age_ms = ?context.pending_age_ms,
                commit_inflight = ?context.commit_inflight,
                forced_view_after_timeout = ?context.forced_view_after_timeout,
                da_enabled = context.da_enabled,
                "no proposal observed for view before changing view"
            );
        }
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub(super) fn handle_block_created(
        &mut self,
        msg: super::message::BlockCreated,
        sender: Option<PeerId>,
    ) -> Result<()> {
        let block = msg.block;
        let block_hash = block.hash();
        let header = block.header();
        let height = header.height().get();
        let view = header.view_change_index();
        let (committed_height, committed_hash) = {
            let state_view = self.state.view();
            (
                u64::try_from(state_view.height()).unwrap_or(u64::MAX),
                state_view.latest_block_hash(),
            )
        };
        let missing_request = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        if stale_height(height, committed_height) {
            debug!(
                height,
                view,
                committed_height,
                committed_hash = ?committed_hash,
                block = %block_hash,
                "dropping BlockCreated at or below committed height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockCreated,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleHeight,
            );
            self.pending.pending_blocks.remove(&block_hash);
            self.subsystems
                .propose
                .proposal_cache
                .pop_hint(height, view);
            self.subsystems
                .propose
                .proposal_cache
                .pop_proposal(height, view);
            self.subsystems
                .propose
                .proposal_cache
                .prune_height_leq(committed_height);
            self.clear_missing_block_request(&block_hash, MissingBlockClearReason::Obsolete);
            self.clean_rbc_sessions_for_block(block_hash, height);
            let matches_committed = committed_hash.is_some_and(|hash| hash == block_hash);
            if !matches_committed {
                self.qc_cache
                    .retain(|(_, hash, _, _, _), _| *hash != block_hash);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _), _| *hash != block_hash);
            }
            return Ok(());
        }
        let da_enabled = self.runtime_da_enabled();
        if let Some(local_view) = self.stale_view(height, view) {
            if !allow_stale_block_created(da_enabled, missing_request) {
                debug!(
                    height,
                    view,
                    local_view,
                    kind = "BlockCreated",
                    "dropping consensus message for stale view"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockCreated,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StaleView,
                );
                return Ok(());
            }
            debug!(
                height,
                view,
                local_view,
                da_enabled,
                missing_request,
                "accepting BlockCreated for stale view to recover missing payload"
            );
        }
        let expected_height = committed_height.saturating_add(1);
        let is_active_height = height == expected_height;
        if height > expected_height {
            let active_commit_topology = self.effective_commit_topology();
            let (consensus_mode, _, _) = self.consensus_context_for_height(height);
            let mut commit_topology =
                self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
            if commit_topology.is_empty() {
                commit_topology.clone_from(&active_commit_topology);
            }
            let expected_usize = usize::try_from(expected_height).ok();
            let actual_usize = usize::try_from(height).ok();
            if let Some(parent_hash) = block.header().prev_block_hash() {
                self.request_missing_parent(
                    block_hash,
                    height,
                    view,
                    parent_hash,
                    &commit_topology,
                    None,
                    expected_usize,
                    actual_usize,
                    "block_created",
                );
            }
            if height > expected_height.saturating_add(1) {
                self.request_missing_parents_for_gap(
                    &active_commit_topology,
                    None,
                    "block_created_gap",
                );
            }
        }
        if block.is_empty() {
            let queue_len = self.queue.queued_len();
            let time_triggers_due = {
                let state_view = self.state.view();
                state_view.time_triggers_due_for_block(&block.header())
            };
            if time_triggers_due {
                iroha_logger::info!(
                    height,
                    view,
                    queue_len,
                    block = %block_hash,
                    "accepting empty BlockCreated; time triggers are due"
                );
            } else {
                iroha_logger::info!(
                    height,
                    view,
                    queue_len,
                    block = %block_hash,
                    "received empty BlockCreated; empty blocks are disallowed"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockCreated,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidPayload,
                );
                self.pending.pending_blocks.remove(&block_hash);
                let proposal = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(height, view);
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(height, view);
                if let Some(proposal) = proposal {
                    warn!(height, view, "dropping BlockCreated due to empty payload");
                    let evidence = invalid_proposal_evidence(
                        proposal,
                        "empty blocks are disallowed".to_string(),
                    );
                    self.record_and_broadcast_evidence(evidence)?;
                }
                self.clear_missing_block_request(&block_hash, MissingBlockClearReason::Obsolete);
                self.clean_rbc_sessions_for_block(block_hash, height);
                self.qc_cache
                    .retain(|(_, hash, _, _, _), _| *hash != block_hash);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _), _| *hash != block_hash);
                return Ok(());
            }
        }
        let pending_status = self
            .pending
            .pending_blocks
            .get(&block_hash)
            .map(|pending| (pending.aborted, pending.validation_status));
        let revive_aborted = pending_status.is_some_and(|(aborted, status)| {
            aborted && !matches!(status, ValidationStatus::Invalid)
        });
        if pending_status.is_some() && !revive_aborted {
            if da_enabled {
                let session_key = Self::session_key(&block_hash, height, view);
                let payload_bytes = super::proposals::block_payload_bytes(&block);
                let payload_hash = Hash::new(&payload_bytes);
                if !self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .contains_key(&session_key)
                {
                    let rebroadcast_missing_init = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .pending
                        .contains_key(&session_key);
                    self.seed_rbc_session_from_block(
                        session_key,
                        &block,
                        payload_hash,
                        rebroadcast_missing_init,
                    )?;
                }
                self.hydrate_rbc_session_from_block(
                    session_key,
                    &payload_bytes,
                    payload_hash,
                    sender.as_ref(),
                )?;
            }
            debug!(
                height,
                view,
                block = %block_hash,
                "dropping duplicate BlockCreated"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockCreated,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Duplicate,
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        if revive_aborted {
            debug!(
                height,
                view,
                block = %block_hash,
                "accepting BlockCreated to revive aborted pending block"
            );
        }
        iroha_logger::info!(
            height,
            view,
            block = %block_hash,
            committed_height,
            "received BlockCreated"
        );
        if let Some(bundle) = block.da_commitments() {
            self.validate_da_bundle(bundle)?;
        }
        let mut cached_hint = self
            .subsystems
            .propose
            .proposal_cache
            .get_hint(height, view)
            .copied();
        let cached_proposal = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view)
            .copied();
        let mut payload_bytes = None;
        let mut payload_hash = None;
        let mut proposal_mismatch = None;
        let parent_view = header
            .prev_block_hash()
            .and_then(|parent_hash| self.local_block_height_view(parent_hash))
            .map(|(_, view)| view);
        if let Some(hint) = cached_hint {
            match Self::validate_block_against_hint(&block_hash, &header, &hint, parent_view) {
                Ok(()) => {}
                Err(HintMismatch::BlockHash) => {
                    super::status::inc_block_created_hint_mismatch();
                    #[cfg(feature = "telemetry")]
                    self.telemetry.inc_block_created_hint_mismatch();
                    info!(
                        cached_block = %hint.block_hash,
                        incoming_block = %block_hash,
                        height,
                        view,
                        "BlockCreated hash mismatches cached hint; replacing cached hint for processing"
                    );
                    cached_hint = Some(super::message::ProposalHint { block_hash, ..hint });
                }
                Err(reason @ (HintMismatch::HighestQcParentHash | HintMismatch::HighestQcView)) => {
                    super::status::inc_block_created_hint_mismatch();
                    #[cfg(feature = "telemetry")]
                    self.telemetry.inc_block_created_hint_mismatch();
                    warn!(
                        ?reason,
                        ?block_hash,
                        height,
                        view,
                        "BlockCreated hint mismatch; dropping cached hint and continuing"
                    );
                    self.subsystems
                        .propose
                        .proposal_cache
                        .pop_hint(height, view);
                    cached_hint = None;
                }
                Err(reason) => {
                    super::status::inc_block_created_hint_mismatch();
                    #[cfg(feature = "telemetry")]
                    self.telemetry.inc_block_created_hint_mismatch();
                    warn!(
                        ?reason,
                        ?block_hash,
                        height,
                        view,
                        "BlockCreated hint mismatch"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockCreated,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::HintMismatch,
                    );
                    return Ok(());
                }
            }
        }
        if cached_hint.is_none() {
            if let Some(proposal) = cached_proposal.as_ref() {
                let computed_bytes = block_payload_bytes(&block);
                let computed_hash = Hash::new(&computed_bytes);
                payload_bytes = Some(computed_bytes);
                payload_hash = Some(computed_hash);
                let mismatch = detect_proposal_mismatch(proposal, &header, &computed_hash);
                if mismatch.is_none() {
                    cached_hint = Some(super::message::ProposalHint {
                        block_hash,
                        height,
                        view,
                        highest_qc: proposal.header.highest_qc,
                    });
                } else {
                    proposal_mismatch = mismatch;
                }
            }
        }
        if let Some(hint) = cached_hint {
            let mut hint_highest = hint.highest_qc;
            let hint_highest_missing = !self.block_known_for_lock(hint_highest.subject_block_hash);
            if let Err(reason) = ensure_locked_qc_allows(self.locked_qc, hint_highest) {
                let locked_hash = self.locked_qc.map(|qc| qc.subject_block_hash);
                let locked_missing =
                    locked_hash.is_some_and(|hash| !self.block_known_for_lock(hash));
                if locked_missing {
                    warn!(
                        ?reason,
                        locked_qc_height = self.locked_qc.map(|qc| qc.height),
                        locked_qc_view = self.locked_qc.map(|qc| qc.view),
                        locked_qc_hash = ?locked_hash,
                        hint_highest_qc_height = hint.highest_qc.height,
                        hint_highest_qc_view = hint.highest_qc.view,
                        hint_highest_qc_hash = ?hint.highest_qc.subject_block_hash,
                        height,
                        view,
                        "locked QC missing from kura; accepting BlockCreated and replacing lock"
                    );
                    self.locked_qc = Some(hint.highest_qc);
                    super::status::set_locked_qc(
                        hint.highest_qc.height,
                        hint.highest_qc.view,
                        Some(hint.highest_qc.subject_block_hash),
                    );
                } else if hint_highest_missing {
                    if let Some(lock) = self.locked_qc {
                        info!(
                            ?reason,
                            locked_qc_height = lock.height,
                            locked_qc_view = lock.view,
                            locked_qc_hash = %lock.subject_block_hash,
                            hint_highest_qc_height = hint.highest_qc.height,
                            hint_highest_qc_view = hint.highest_qc.view,
                            hint_highest_qc_hash = %hint.highest_qc.subject_block_hash,
                            height,
                            view,
                            "highest QC block missing locally; accepting BlockCreated on locked chain"
                        );
                        hint_highest = lock;
                    }
                } else {
                    super::status::inc_block_created_dropped_by_lock();
                    #[cfg(feature = "telemetry")]
                    self.telemetry.inc_block_created_dropped_by_lock();
                    warn!(
                        ?reason,
                        locked_qc_height = self.locked_qc.map(|qc| qc.height),
                        locked_qc_view = self.locked_qc.map(|qc| qc.view),
                        locked_qc_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                        hint_highest_qc_height = hint.highest_qc.height,
                        hint_highest_qc_view = hint.highest_qc.view,
                        hint_highest_qc_hash = ?hint.highest_qc.subject_block_hash,
                        height,
                        view,
                        "BlockCreated rejected by locked QC gate"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockCreated,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
                    );
                    return Ok(());
                }
            }
            if !self.highest_qc_extends_locked(hint_highest) {
                if let Some(new_lock) = realign_locked_to_committed_if_extends(
                    self.locked_qc,
                    self.latest_committed_qc(),
                    hint_highest,
                    |hash, height| self.parent_hash_for(hash, height),
                ) {
                    if self.locked_qc != Some(new_lock) {
                        info!(
                            locked_qc_height = self.locked_qc.map(|qc| qc.height),
                            locked_qc_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                            highest_qc_height = hint_highest.height,
                            highest_qc_hash = ?hint_highest.subject_block_hash,
                            height,
                            view,
                            "resetting locked QC to committed chain before accepting BlockCreated"
                        );
                        self.locked_qc = Some(new_lock);
                        super::status::set_locked_qc(
                            new_lock.height,
                            new_lock.view,
                            Some(new_lock.subject_block_hash),
                        );
                    }
                }
            }
            if !self.highest_qc_extends_locked(hint_highest) {
                super::status::inc_block_created_dropped_by_lock();
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_block_created_dropped_by_lock();
                warn!(
                    locked_qc_height = self.locked_qc.map(|qc| qc.height),
                    locked_qc_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                    highest_qc_height = hint_highest.height,
                    highest_qc_hash = ?hint_highest.subject_block_hash,
                    height,
                    view,
                    "BlockCreated rejected: highest QC does not extend locked chain"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockCreated,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::LockedQc,
                );
                return Ok(());
            }
        } else {
            trace!(
                height,
                view, "BlockCreated arrived without cached ProposalHint"
            );
            if let Some(lock) = self.locked_qc {
                let locked_hash = lock.subject_block_hash;
                if self.block_known_for_lock(locked_hash) {
                    let parent_hash = header.prev_block_hash();
                    let extends = super::chain_extends_tip(
                        block_hash,
                        height,
                        lock.height,
                        locked_hash,
                        |hash, height| {
                            if hash == block_hash {
                                parent_hash
                            } else {
                                self.parent_hash_for(hash, height)
                            }
                        },
                    );
                    match extends {
                        Some(false) => {
                            super::status::inc_block_created_dropped_by_lock();
                            #[cfg(feature = "telemetry")]
                            self.telemetry.inc_block_created_dropped_by_lock();
                            warn!(
                                locked_qc_height = lock.height,
                                locked_qc_view = lock.view,
                                locked_qc_hash = %locked_hash,
                                height,
                                view,
                                block = %block_hash,
                                "BlockCreated rejected without hint: block does not extend locked chain"
                            );
                            self.record_consensus_message_handling(
                                super::status::ConsensusMessageKind::BlockCreated,
                                super::status::ConsensusMessageOutcome::Dropped,
                                super::status::ConsensusMessageReason::LockedQc,
                            );
                            return Ok(());
                        }
                        Some(true) => {}
                        None => {
                            debug!(
                                locked_qc_height = lock.height,
                                locked_qc_view = lock.view,
                                locked_qc_hash = %locked_hash,
                                height,
                                view,
                                block = %block_hash,
                                "accepting BlockCreated without hint: locked ancestry unknown"
                            );
                        }
                    }
                } else {
                    debug!(
                        locked_qc_height = lock.height,
                        locked_qc_view = lock.view,
                        locked_qc_hash = %locked_hash,
                        height,
                        view,
                        block = %block_hash,
                        "skipping locked QC check for BlockCreated without hint: locked block missing locally"
                    );
                }
            }
        }
        let payload_bytes = payload_bytes.unwrap_or_else(|| block_payload_bytes(&block));
        let payload_hash = payload_hash.unwrap_or_else(|| Hash::new(&payload_bytes));
        if let Some(reason) = proposal_mismatch
            .or_else(|| {
                cached_proposal
                    .as_ref()
                    .and_then(|proposal| detect_proposal_mismatch(proposal, &header, &payload_hash))
            })
            .map(|mismatch| mismatch.reason())
        {
            self.invalidate_proposal(height, view, reason)?;
            // Keep processing the payload after invalidating a stale proposal.
        }
        let session_key = Self::session_key(&block_hash, height, view);
        if da_enabled {
            if !self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .contains_key(&session_key)
            {
                debug!(
                    height,
                    view,
                    "BlockCreated arrived before RBC session initialised; seeding local RBC snapshot"
                );
                let rebroadcast_missing_init = self
                    .subsystems
                    .da_rbc
                    .rbc
                    .pending
                    .contains_key(&session_key);
                self.seed_rbc_session_from_block(
                    session_key,
                    &block,
                    payload_hash,
                    rebroadcast_missing_init,
                )?;
            }
            self.hydrate_rbc_session_from_block(
                session_key,
                &payload_bytes,
                payload_hash,
                sender.as_ref(),
            )?;
        }
        if self
            .pending
            .pending_processing
            .get()
            .is_some_and(|pending| pending == block_hash)
        {
            debug!(
                height,
                view,
                block = %block_hash,
                "BlockCreated received while pending block is being processed; skipping pending update"
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        if self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .is_some_and(|inflight| inflight.block_hash == block_hash)
        {
            debug!(
                height,
                view,
                block = %block_hash,
                "BlockCreated received while commit is in flight; skipping pending update"
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        match self.pending.pending_blocks.entry(block_hash) {
            Entry::Occupied(mut occ) => {
                if revive_aborted {
                    occ.get_mut()
                        .revive_after_abort(block, payload_hash, height, view);
                } else {
                    occ.get_mut()
                        .replace_block(block, payload_hash, height, view);
                }
            }
            Entry::Vacant(vac) => {
                vac.insert(PendingBlock::new(block, payload_hash, height, view));
            }
        }
        if is_active_height {
            self.note_view_change_from_block(height, view);
        }
        if let Some(qc) = qc_cache_for_subject(&self.qc_cache, block_hash)
            .filter(|qc| {
                qc.phase == crate::sumeragi::consensus::Phase::Commit && qc.height == height
            })
            .max_by_key(|qc| qc.view)
            .cloned()
        {
            if self.process_precommit_qc(&qc, true, false) {
                debug!(
                    height,
                    view,
                    block = %block_hash,
                    "applied cached precommit QC after block became known"
                );
            }
        }

        let mut status_update = None;
        let mut mismatch_expected = None;
        {
            if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get_mut(&session_key) {
                if let Some(expected_hash) = session.payload_hash() {
                    if expected_hash != payload_hash {
                        session.invalid = true;
                        mismatch_expected = Some(expected_hash);
                    }
                } else {
                    session.payload_hash = Some(payload_hash);
                }
                status_update = Some((
                    session.total_chunks(),
                    session.received_chunks(),
                    session.ready_signatures.len() as u64,
                    session.delivered,
                    session.payload_hash(),
                    session.recovered_from_disk(),
                    session.is_invalid(),
                ));
            }
        }
        if let Some((
            total_chunks,
            received_chunks,
            ready_count,
            delivered_flag,
            payload_hash_opt,
            recovered_flag,
            invalid_flag,
        )) = status_update
        {
            let (lane_backlog, dataspace_backlog) = self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .get(&session_key)
                .map_or_else(
                    || (Vec::new(), Vec::new()),
                    |session| {
                        (
                            session.lane_backlog_entries(),
                            session.dataspace_backlog_entries(),
                        )
                    },
                );
            let summary = super::rbc_status::Summary {
                block_hash: session_key.0,
                height: session_key.1,
                view: session_key.2,
                total_chunks,
                received_chunks,
                ready_count,
                delivered: delivered_flag,
                payload_hash: payload_hash_opt,
                recovered_from_disk: recovered_flag,
                invalid: invalid_flag,
                lane_backlog,
                dataspace_backlog,
            };
            self.subsystems
                .da_rbc
                .rbc
                .status_handle
                .update(summary, SystemTime::now());
            if let Some(expected_hash) = mismatch_expected {
                debug!(
                    height,
                    view,
                    expected = ?expected_hash,
                    observed = ?payload_hash,
                    "BlockCreated payload hash mismatches RBC session"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockCreated,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::PayloadMismatch,
                );
                self.invalidate_proposal(
                    height,
                    view,
                    format!(
                        "payload hash mismatch: expected {expected_hash:?}, observed {payload_hash:?}",
                    ),
                )?;
                self.clear_payload_mismatch_state(session_key, block_hash, height, view);
                self.finalize_collector_plan(false);
                return Ok(());
            }
        }

        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        self.clear_missing_block_request(&block_hash, MissingBlockClearReason::PayloadAvailable);

        // If votes or cached QCs already exist for this block, re-evaluate now that the payload is
        // present so late-arriving block payloads can still finalize with previously collected votes.
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let commit_topology =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if !commit_topology.is_empty() {
            let topology = super::network_topology::Topology::new(commit_topology.clone());
            let epochs = distinct_epochs_for_block_votes(
                &self.vote_log,
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                height,
                view,
            );
            for epoch in epochs {
                self.try_form_qc_from_votes(
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    height,
                    view,
                    epoch,
                    &topology,
                );
            }
            let cached_qc = qc_cache_for_subject(&self.qc_cache, block_hash)
                .find(|cached| {
                    cached.phase == crate::sumeragi::consensus::Phase::Commit
                        && cached.height == height
                        && cached.view == view
                })
                .cloned();
            if let Some(qc) = cached_qc {
                let _ = self.handle_qc(qc);
            }
        }
        self.request_commit_pipeline();
        Ok(())
    }
    pub(super) fn invalidate_proposal(
        &mut self,
        height: u64,
        view: u64,
        reason: String,
    ) -> Result<()> {
        let proposal_opt = self
            .subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        super::status::inc_block_created_proposal_mismatch();
        #[cfg(feature = "telemetry")]
        self.telemetry.inc_block_created_proposal_mismatch();
        if let Some(proposal) = proposal_opt {
            warn!(
                height,
                view,
                reason = reason.as_str(),
                "dropping BlockCreated due to proposal mismatch"
            );
            let evidence = invalid_proposal_evidence(proposal, reason);
            self.record_and_broadcast_evidence(evidence)?;
        } else {
            warn!(
                height,
                view,
                reason = reason.as_str(),
                "proposal mismatch detected but no cached proposal available to emit evidence"
            );
        }
        Ok(())
    }

    pub(super) fn clear_payload_mismatch_state(
        &mut self,
        session_key: SessionKey,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) {
        self.pending.pending_blocks.remove(&block_hash);
        self.purge_rbc_state(session_key, block_hash, height, view);
    }
}

#[cfg(test)]
mod tests {
    use super::stale_height;

    #[test]
    fn stale_height_rejects_committed_or_lower() {
        assert!(stale_height(1, 1));
        assert!(stale_height(2, 5));
        assert!(stale_height(5, 5));
    }

    #[test]
    fn stale_height_allows_above_committed() {
        assert!(!stale_height(2, 1));
        assert!(!stale_height(6, 5));
    }
}
