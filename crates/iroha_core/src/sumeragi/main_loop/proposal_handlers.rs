//! Proposal- and block-created message handling.

use std::{sync::mpsc, time::Instant};

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

pub(super) fn allow_stale_block_created(missing_request: bool, retained_match: bool) -> bool {
    missing_request || retained_match
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

    fn should_force_missing_highest_fetch(&self, hash: HashOf<BlockHeader>) -> bool {
        self.pending
            .pending_blocks
            .get(&hash)
            .is_some_and(|pending| {
                pending.is_retry_aborted()
                    && !matches!(pending.validation_status, ValidationStatus::Invalid)
            })
            || self
                .pending
                .pending_processing
                .get()
                .is_some_and(|pending| pending == hash)
    }

    pub(crate) fn should_clear_missing_request_on_locked_reject(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
        locked_hash: HashOf<BlockHeader>,
        locked_height: u64,
        known_parent: Option<HashOf<BlockHeader>>,
    ) -> bool {
        if hash == locked_hash {
            return false;
        }
        if height == locked_height && self.kura.get_block_height_by_hash(locked_hash).is_some() {
            // Same-height recovery is disproven once the competing lock subject is already
            // durable in local storage. If the lock subject only exists in pending/inflight
            // state, keep the request alive until committed history or later local evidence
            // settles the conflict.
            return true;
        }
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let known_conflict = self
            .committed_block_hash_for_height(height)
            .is_some_and(|known_hash| known_hash != hash);
        let locally_conflicts_with_locked = matches!(
            super::chain_extends_tip(
                hash,
                height,
                locked_height,
                locked_hash,
                |candidate, candidate_height| {
                    if candidate == hash {
                        known_parent.or_else(|| self.parent_hash_for(candidate, candidate_height))
                    } else {
                        self.parent_hash_for(candidate, candidate_height)
                    }
                }
            ),
            Some(false)
        );
        let header_proves_locked_conflict = known_parent.is_some() && locally_conflicts_with_locked;
        let locked_chain_committed = locked_height <= committed_height;
        // Lock rejection may clear requests once local evidence disproves the branch:
        // either committed history conflicts with the hash, or local ancestry proves the hash
        // does not extend a lock that is already anchored by committed history. Preserve
        // unresolved requests when lock ancestry may still legitimately realign.
        (height <= committed_height || height <= locked_height) && known_conflict
            || header_proves_locked_conflict
            || (locked_chain_committed && locally_conflicts_with_locked)
    }

    fn should_clear_missing_request_on_stale_block_drop(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
        committed_height: u64,
    ) -> bool {
        if height < committed_height {
            return true;
        }
        // Keep committed-height recovery requests alive when the payload is still unavailable and
        // local storage has not disproven the hash. This preserves dwell/attempt continuity.
        if self.block_payload_available_locally(hash) {
            return true;
        }
        self.committed_block_hash_for_height(height)
            .is_some_and(|known_hash| known_hash != hash)
    }

    fn trigger_active_height_lock_reject_recovery(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        source: &'static str,
    ) {
        let active_height = self.committed_height_snapshot().saturating_add(1);
        if height != active_height {
            return;
        }
        let now = Instant::now();
        let highest_qc_fetch_requested = self
            .highest_qc
            .or(self.latest_committed_qc())
            .is_some_and(|highest| {
                self.request_missing_block_for_highest_qc_force(highest, source)
            });
        let recovery_advance =
            self.advance_frontier_recovery("missing_qc", height, view, false, true, true, now);
        let view_change_triggered = matches!(recovery_advance, FrontierRecoveryAdvance::Rotate);
        warn!(
            height,
            view,
            block = %block_hash,
            source,
            highest_qc_fetch_requested,
            recovery_advance = ?recovery_advance,
            view_change_triggered,
            "routed lock-rejected active-height recovery through unified frontier recovery"
        );
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
        let state_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
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
                let committed_conflict_suppressed = self
                    .suppress_committed_edge_conflicting_highest_qc(highest_qc, "proposal_hint");
                info!(
                        height,
                        view,
                    committed_height,
                    committed_hash = %stored_hash.unwrap_or(highest_qc.subject_block_hash),
                    highest_hash = %highest_qc.subject_block_hash,
                    committed_conflict_suppressed,
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
            let committed_conflict_suppressed =
                self.suppress_committed_edge_conflicting_highest_qc(highest_qc, "proposal_hint");
            info!(
                height,
                view,
                committed_height,
                highest_height = highest_qc.height,
                block = %highest_qc.subject_block_hash,
                committed_conflict_suppressed,
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
                "deferring proposal hint until highest QC dependency is repaired"
            );
            self.defer_round_until_highest_qc_dependency_resolves(
                height,
                view,
                highest_qc,
                "proposal_hint",
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ProposalHint,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::MissingHighestQc,
            );
            return Ok(());
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
            if self.defer_highest_qc_update_for_lock_catchup(
                height,
                view,
                highest_qc,
                Instant::now(),
                "proposal_hint",
            ) {
                debug!(
                    height,
                    view,
                    highest_height = highest_qc.height,
                    highest_view = highest_qc.view,
                    "deferring highest QC update for proposal hint under lock-lag catch-up"
                );
            } else {
                self.highest_qc = Some(highest_qc);
                super::status::set_highest_qc(highest_qc.height, highest_qc.view);
                super::status::set_highest_qc_hash(highest_qc.subject_block_hash);
            }
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
        if self.slot_tracker.proposals_seen.insert((height, view)) {
            iroha_logger::info!(
                height,
                view,
                block = %hint_block,
                "observed proposal hint for view"
            );
        }
        self.prune_proposals_seen_horizon(state_height);
        let _ = self.maybe_release_committed_edge_conflict_owner("proposal_hint_seen");
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

    pub(super) fn defer_highest_qc_update_for_lock_catchup(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: super::consensus::QcHeaderRef,
        now: Instant,
        source: &'static str,
    ) -> bool {
        let Some(catchup_height) = self.lock_lag_catchup_frontier_for_highest(highest_qc) else {
            return false;
        };
        let marked = if self.block_known_locally(highest_qc.subject_block_hash) {
            self.subsystems
                .propose
                .highest_qc_missing_defer_markers
                .retain(|(marker_height, _, marker_hash)| {
                    *marker_height != height || *marker_hash != highest_qc.subject_block_hash
                });
            false
        } else {
            self.mark_highest_qc_missing_defer_for_round(height, view, highest_qc)
        };
        let requested_highest = self.request_missing_block_for_highest_qc_force(highest_qc, source);
        let requested_range =
            self.request_range_pull_from_anchor(catchup_height, "lock_lag_highest_qc_defer", now);
        debug!(
            height,
            view,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            highest_hash = %highest_qc.subject_block_hash,
            catchup_height,
            marked,
            requested_highest,
            requested_range,
            source,
            "deferring highest QC update while locked ancestry catch-up is unresolved"
        );
        true
    }

    fn defer_round_until_highest_qc_dependency_resolves(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: super::consensus::QcHeaderRef,
        source: &'static str,
    ) -> bool {
        if self.defer_highest_qc_update_for_lock_catchup(
            height,
            view,
            highest_qc,
            Instant::now(),
            source,
        ) {
            return true;
        }
        let marked = self.mark_highest_qc_missing_defer_for_round(height, view, highest_qc);
        let requested = if self.should_force_missing_highest_fetch(highest_qc.subject_block_hash) {
            self.request_missing_block_for_highest_qc_force(highest_qc, source)
        } else {
            self.request_missing_block_for_highest_qc(highest_qc, source)
        };
        debug!(
            height,
            view,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            highest_hash = %highest_qc.subject_block_hash,
            marked,
            requested,
            source,
            "deferring slot until highest QC dependency is resolved locally"
        );
        true
    }

    fn preserve_block_created_for_highest_qc_repair(
        &mut self,
        block: &SignedBlock,
        frontier: Option<super::message::BlockCreatedFrontierInfo>,
        inline_hint: Option<super::message::ProposalHint>,
        inline_proposal: Option<crate::sumeragi::consensus::Proposal>,
        sender: Option<PeerId>,
    ) {
        let block_hash = block.hash();
        let header = block.header();
        let height = header.height().get();
        let view = header.view_change_index();
        if let Some(frontier) = frontier {
            self.note_authoritative_slot_frontier_info(height, view, block_hash, frontier);
        }
        if let Some(hint) = inline_hint {
            self.subsystems.propose.proposal_cache.insert_hint(hint);
        }
        if let Some(proposal) = inline_proposal {
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal);
        }
        self.cache_deferred_block_sync_update(
            super::message::BlockSyncUpdate::from(block),
            sender,
            block_hash,
            height,
            view,
            "missing_highest_qc_block_created",
        );
        self.flush_frontier_body_requesters(block);
        self.flush_pending_fetch_requests(block);
        self.clear_missing_block_request(&block_hash, MissingBlockClearReason::PayloadAvailable);
        self.clear_missing_block_view_change(&block_hash);
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
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
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
                let committed_conflict_suppressed =
                    self.suppress_committed_edge_conflicting_highest_qc(highest_qc, "proposal");
                info!(
                        height,
                        view,
                        committed_height = committed_qc_height,
                    committed_hash = %stored_hash.unwrap_or(highest_qc.subject_block_hash),
                    highest_hash = %highest_qc.subject_block_hash,
                    payload = %proposal.payload_hash,
                    committed_conflict_suppressed,
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
            let committed_conflict_suppressed =
                self.suppress_committed_edge_conflicting_highest_qc(highest_qc, "proposal");
            info!(
                height,
                view,
                committed_height = committed_qc_height,
                highest_height = highest_qc.height,
                block = %highest_qc.subject_block_hash,
                payload = %proposal.payload_hash,
                committed_conflict_suppressed,
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
                "deferring proposal until highest QC dependency is repaired"
            );
            self.defer_round_until_highest_qc_dependency_resolves(
                height, view, highest_qc, "proposal",
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Proposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::MissingHighestQc,
            );
            return Ok(());
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
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let proposal_roster = self.canonical_round_roster_with_mode(height, view, consensus_mode);
        let proposal_roster = if proposal_roster.is_empty() {
            self.effective_commit_topology()
        } else {
            proposal_roster
        };
        if !proposal_roster.is_empty() {
            let mut topology = super::network_topology::Topology::new(proposal_roster);
            if let Ok(leader_index) = self.leader_index_for(&mut topology, height, view) {
                super::status::set_leader_index(u64::try_from(leader_index).unwrap_or(u64::MAX));
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
            if self.defer_highest_qc_update_for_lock_catchup(
                height,
                view,
                highest_qc,
                Instant::now(),
                "proposal",
            ) {
                debug!(
                    height,
                    view,
                    highest_height = highest_qc.height,
                    highest_view = highest_qc.view,
                    "deferring highest QC update for proposal under lock-lag catch-up"
                );
            } else {
                self.highest_qc = Some(highest_qc);
                super::status::set_highest_qc(highest_qc.height, highest_qc.view);
                super::status::set_highest_qc_hash(highest_qc.subject_block_hash);
            }
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

    pub(super) fn authoritative_slot_owner_hash(
        &self,
        height: u64,
        view: u64,
    ) -> Option<HashOf<BlockHeader>> {
        self.slot_tracker
            .authoritative_slot_owner_hash(height, view)
    }

    pub(super) fn note_authoritative_slot_owner(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
    ) {
        self.slot_tracker
            .note_authoritative_slot_owner(height, view, block_hash);
        let _ = self.maybe_release_committed_edge_conflict_owner("authoritative_slot_owner");
    }

    pub(super) fn authoritative_slot_frontier_info(
        &self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<super::message::BlockCreatedFrontierInfo> {
        self.slot_tracker
            .authoritative_slot_frontier_info(height, view, block_hash)
    }

    fn note_authoritative_slot_frontier_info(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        frontier: super::message::BlockCreatedFrontierInfo,
    ) {
        self.slot_tracker
            .note_authoritative_slot_frontier_info(height, view, block_hash, frontier);
    }

    fn note_frontier_block_created(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
        sender: Option<&PeerId>,
        now: Instant,
    ) {
        self.note_frontier_block_created_with_owner_policy(
            block_hash,
            height,
            view,
            frontier_info,
            sender,
            now,
            false,
        );
    }

    fn note_frontier_block_created_authoritatively(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
        sender: Option<&PeerId>,
        now: Instant,
    ) {
        self.note_frontier_block_created_with_owner_policy(
            block_hash,
            height,
            view,
            frontier_info,
            sender,
            now,
            true,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn note_frontier_block_created_with_owner_policy(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
        sender: Option<&PeerId>,
        now: Instant,
        authoritative_supersede: bool,
    ) {
        if let Some(frontier) = frontier_info.clone() {
            self.note_authoritative_slot_frontier_info(height, view, block_hash, frontier);
        }
        let leader = sender
            .cloned()
            .filter(|peer| peer != self.common_config.peer.id());
        let voters = leader.iter().cloned().collect();
        let updated = if authoritative_supersede {
            let advance = self.handle_frontier_slot_event(
                now,
                super::FrontierSlotEvent::OnAuthoritativeSupersede {
                    block_hash,
                    view,
                    frontier_info,
                    leader,
                    voters,
                    body_present: true,
                    requester: None,
                },
            );
            !matches!(advance, super::FrontierRecoveryAdvance::None)
                || self.frontier_slot_has_active_owner_state(height)
        } else {
            self.update_frontier_slot(
                block_hash,
                height,
                view,
                leader,
                voters,
                /*block_created_seen*/ true,
                /*exact_fetch_armed*/ true,
                true,
                frontier_info,
                None,
                now,
            )
        };
        if updated {
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            self.clear_missing_block_view_change(&block_hash);
        }
        let _ = self.maybe_release_committed_edge_conflict_owner("frontier_block_created");
    }

    pub(super) fn drop_superseded_contiguous_frontier_owner_state(
        &mut self,
        incoming_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) {
        if height != self.committed_height_snapshot().saturating_add(1) {
            return;
        }
        let mut superseded = BTreeSet::new();
        if let Some(slot) = self.frontier_slot.as_ref()
            && slot.height == height
            && slot.block_hash != incoming_hash
        {
            superseded.insert((slot.block_hash, slot.view));
        }
        if let Some(owner_hash) = self.authoritative_slot_owner_hash(height, view)
            && owner_hash != incoming_hash
        {
            superseded.insert((owner_hash, view));
        }
        for (hash, old_view) in superseded {
            self.pending.pending_blocks.remove(&hash);
            let _ = self.supersede_validation_inflight(hash);
            self.pending.pending_fetch_requests.remove(&hash);
            self.clear_missing_block_request(&hash, MissingBlockClearReason::Obsolete);
            self.clear_missing_block_view_change(&hash);
            self.clean_rbc_sessions_for_block(hash, height);
            debug!(
                height,
                incoming_view = view,
                superseded_view = old_view,
                incoming_block = %incoming_hash,
                superseded_block = %hash,
                "dropping superseded contiguous-frontier owner after stronger same-height evidence"
            );
        }
    }

    pub(super) fn note_proposal_seen(&mut self, height: u64, view: u64, payload_hash: Hash) {
        if self
            .subsystems
            .propose
            .proposal_liveness
            .is_some_and(|slot| slot.height == height && slot.view == view)
        {
            self.mark_proposal_liveness_state(
                height,
                view,
                ProposalLivenessState::Normal,
                Instant::now(),
            );
            self.subsystems.propose.proposal_liveness = None;
        }
        if self.slot_tracker.proposals_seen.insert((height, view)) {
            iroha_logger::info!(
                height,
                view,
                payload = %payload_hash,
                "observed proposal for view"
            );
        }
        self.prune_proposals_seen_horizon(self.last_committed_height);
        let _ = self.maybe_release_committed_edge_conflict_owner("proposal_seen");
    }

    pub(super) fn slot_has_proposal_evidence(&self, height: u64, view: u64) -> bool {
        self.slot_has_authoritative_payload(height, view)
            || self.slot_tracker.proposals_seen.contains(&(height, view))
            || self
                .subsystems
                .propose
                .proposal_cache
                .get_proposal(height, view)
                .is_some()
            || self
                .authoritative_slot_owner_hash(height, view)
                .and_then(|block_hash| {
                    self.authoritative_slot_frontier_info(height, view, block_hash)
                })
                .is_some()
            || self.frontier_slot_has_active_owner_state_for_view(height, view)
    }

    fn locally_authoritative_frontier_info_for_block(
        &self,
        block: &SignedBlock,
    ) -> Option<super::message::BlockCreatedFrontierInfo> {
        let header = block.header();
        self.authoritative_slot_frontier_info(
            header.height().get(),
            header.view_change_index(),
            block.hash(),
        )
        .or_else(|| {
            self.frontier_slot.as_ref().and_then(|slot| {
                (slot.block_hash == block.hash()
                    && slot.height == header.height().get()
                    && slot.view == header.view_change_index())
                .then(|| slot.frontier_info.clone())
                .flatten()
            })
        })
    }

    pub(super) fn frontier_block_created_from_proposal(
        &self,
        block: &SignedBlock,
        proposal: &crate::sumeragi::consensus::Proposal,
    ) -> Option<super::message::BlockCreated> {
        let header = block.header();
        let payload_bytes = super::proposals::block_payload_bytes(block);
        let payload_hash = Hash::new(&payload_bytes);
        if payload_hash != proposal.payload_hash {
            debug!(
                height = header.height().get(),
                view = header.view_change_index(),
                block = %block.hash(),
                expected = ?proposal.payload_hash,
                observed = ?payload_hash,
                "skipping frontier BlockCreated manifest build: proposal payload hash mismatches local block payload"
            );
            return None;
        }
        let header = block.header();
        let key = Self::session_key(
            &block.hash(),
            header.height().get(),
            header.view_change_index(),
        );
        let rebuilt_init = match self.rebuild_rbc_init_from_block(block, key) {
            Some(init) => init,
            None => {
                debug!(
                    height = header.height().get(),
                    view = header.view_change_index(),
                    block = %block.hash(),
                    "skipping frontier BlockCreated manifest build: deterministic RBC INIT unavailable"
                );
                return None;
            }
        };
        if rebuilt_init.payload_hash != payload_hash {
            debug!(
                height = header.height().get(),
                view = header.view_change_index(),
                block = %block.hash(),
                expected = ?payload_hash,
                observed = ?rebuilt_init.payload_hash,
                "skipping frontier BlockCreated manifest build: rebuilt payload hash mismatches local block payload"
            );
            return None;
        }
        if rebuilt_init.epoch != proposal.header.epoch {
            debug!(
                height = header.height().get(),
                view = header.view_change_index(),
                block = %block.hash(),
                expected_epoch = proposal.header.epoch,
                observed_epoch = rebuilt_init.epoch,
                "skipping frontier BlockCreated manifest build: rebuilt epoch mismatches proposal epoch"
            );
            return None;
        }
        let leader_signature = block
            .signatures()
            .find(|signature| signature.index() == u64::from(proposal.header.proposer))
            .cloned()
            .or_else(|| {
                let signature = (rebuilt_init.leader_signature.index()
                    == u64::from(proposal.header.proposer))
                .then(|| rebuilt_init.leader_signature.clone())
                .or_else(|| block.signatures().next().cloned());
                if signature.is_some() {
                    debug!(
                        height = header.height().get(),
                        view = header.view_change_index(),
                        block = %block.hash(),
                        proposer = proposal.header.proposer,
                        "falling back to first block signature while building frontier BlockCreated manifest"
                    );
                }
                signature
            })?;
        let frontier = super::message::BlockCreatedFrontierInfo {
            highest_qc: proposal.header.highest_qc,
            payload_hash,
            proposer: proposal.header.proposer,
            epoch: proposal.header.epoch,
            roster_hash: rebuilt_init.roster_hash,
            total_chunks: rebuilt_init.total_chunks,
            chunk_digests: rebuilt_init.chunk_digests,
            chunk_root: rebuilt_init.chunk_root,
            leader_signature,
        };
        Some(super::message::BlockCreated::with_frontier(
            block.clone(),
            frontier,
        ))
    }

    pub(super) fn frontier_block_created_for_proposal_wire(
        &self,
        block: &SignedBlock,
        proposal: &crate::sumeragi::consensus::Proposal,
    ) -> Option<super::message::BlockCreated> {
        self.frontier_block_created_from_proposal(block, proposal)
            .or_else(|| {
                self.locally_authoritative_frontier_info_for_block(block)
                    .filter(|frontier| {
                        frontier.payload_hash == proposal.payload_hash
                            && frontier.highest_qc == proposal.header.highest_qc
                            && frontier.proposer == proposal.header.proposer
                            && frontier.epoch == proposal.header.epoch
                    })
                    .map(|frontier| {
                        super::message::BlockCreated::with_frontier(block.clone(), frontier)
                    })
            })
    }

    pub(super) fn frontier_block_created_for_wire(
        &self,
        block: &SignedBlock,
    ) -> super::message::BlockCreated {
        let header = block.header();
        if let Some(created) = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(header.height().get(), header.view_change_index())
            .and_then(|proposal| self.frontier_block_created_for_proposal_wire(block, proposal))
        {
            return created;
        }
        if let Some(frontier) = self.locally_authoritative_frontier_info_for_block(block) {
            return super::message::BlockCreated::with_frontier(block.clone(), frontier);
        }
        super::message::BlockCreated::from(block)
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
        self.slot_tracker
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
        let min_authoritative_height =
            active_height.saturating_sub(super::AUTHORITATIVE_FRONTIER_WIRE_HEIGHT_WINDOW);
        self.slot_tracker
            .authoritative_block_slots
            .retain(|(entry_height, _), _| *entry_height >= min_authoritative_height);
        self.slot_tracker
            .authoritative_block_frontiers
            .retain(|(entry_height, _, _), _| *entry_height >= min_authoritative_height);
        self.slot_tracker
            .retained_branches
            .retain(|(entry_height, _, _), _| *entry_height >= min_authoritative_height);
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
        if self.slot_has_round_liveness(height, view) {
            return;
        }
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

    /// Drop cached branch state at `height` that does not extend `locked_parent_hash`.
    ///
    /// This keeps lock-conflicting branch artifacts from repeatedly re-triggering
    /// missing-qc rotations after deterministic lock rejection.
    fn purge_locked_conflicting_branch_state(
        &mut self,
        height: u64,
        locked_parent_hash: HashOf<BlockHeader>,
    ) -> (usize, usize, usize) {
        let conflicting_pending: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .filter(|(_, pending)| pending.height == height)
            .filter_map(|(hash, pending)| {
                pending
                    .block
                    .header()
                    .prev_block_hash()
                    .filter(|parent| *parent != locked_parent_hash)
                    .map(|_| *hash)
            })
            .collect();

        let mut pending_removed = 0usize;
        let mut deferred_removed = 0usize;
        for hash in conflicting_pending {
            if let Some(pending) = self.pending.pending_blocks.remove(&hash) {
                pending_removed = pending_removed.saturating_add(1);
                self.subsystems.validation.inflight.remove(&hash);
                self.subsystems.validation.superseded_results.remove(&hash);
                self.pending.pending_fetch_requests.remove(&hash);
                if self
                    .deferred_block_sync_updates
                    .remove(&(pending.height, pending.view, hash))
                    .is_some()
                {
                    deferred_removed = deferred_removed.saturating_add(1);
                }
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(pending.height, pending.view);
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(pending.height, pending.view);
                if self.should_clear_missing_request_on_locked_reject(
                    hash,
                    pending.height,
                    locked_parent_hash,
                    height.saturating_sub(1),
                    pending.block.header().prev_block_hash(),
                ) {
                    self.clear_missing_block_request(&hash, MissingBlockClearReason::Obsolete);
                } else {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        locked_hash = %locked_parent_hash,
                        "preserving missing request for lock-conflicting pending block: hash not yet disproven by committed chain"
                    );
                }
                self.clean_rbc_sessions_for_block(hash, pending.height);
            }
        }

        let stale_missing: Vec<_> = self
            .pending
            .missing_block_requests
            .iter()
            .filter(|(_, request)| request.height == height)
            .map(|(hash, request)| (*hash, request.height))
            .collect();
        let mut missing_removed = 0usize;
        for (hash, request_height) in stale_missing {
            if self.should_clear_missing_request_on_locked_reject(
                hash,
                request_height,
                locked_parent_hash,
                height.saturating_sub(1),
                None,
            ) && self.pending.missing_block_requests.contains_key(&hash)
            {
                self.clear_missing_block_request(&hash, MissingBlockClearReason::Obsolete);
                missing_removed = missing_removed.saturating_add(1);
            }
        }

        let stale_deferred: Vec<_> = self
            .deferred_block_sync_updates
            .keys()
            .copied()
            .filter(|(entry_height, _, hash)| {
                *entry_height == height && *hash != locked_parent_hash
            })
            .collect();
        for key in stale_deferred {
            if self.deferred_block_sync_updates.remove(&key).is_some() {
                deferred_removed = deferred_removed.saturating_add(1);
            }
        }

        (pending_removed, missing_removed, deferred_removed)
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub(super) fn handle_block_created(
        &mut self,
        msg: super::message::BlockCreated,
        sender: Option<PeerId>,
    ) -> Result<()> {
        self.handle_block_created_with_preserve_policy(msg, sender, true, false)
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub(super) fn handle_block_created_from_block_sync(
        &mut self,
        msg: super::message::BlockCreated,
        sender: Option<PeerId>,
        allow_frontier_owner_preserve_on_payload_mismatch: bool,
        allow_authoritative_frontier_owner_supersede: bool,
    ) -> Result<()> {
        self.handle_block_created_with_preserve_policy(
            msg,
            sender,
            allow_frontier_owner_preserve_on_payload_mismatch,
            allow_authoritative_frontier_owner_supersede,
        )
    }

    fn retry_rbc_progress_after_block_created_hydration(&mut self, session_key: SessionKey) {
        // Duplicate/alternate BlockCreated paths can return before later RBC recovery passes.
        // Retry READY/DELIVER immediately once the local payload has hydrated the session.
        if let Err(err) = self.maybe_emit_rbc_ready(session_key) {
            debug!(
                height = session_key.1,
                view = session_key.2,
                block = %session_key.0,
                ?err,
                "failed to retry RBC READY after BlockCreated hydration"
            );
        }
        if let Err(err) = self.maybe_emit_rbc_deliver(session_key) {
            debug!(
                height = session_key.1,
                view = session_key.2,
                block = %session_key.0,
                ?err,
                "failed to retry RBC DELIVER after BlockCreated hydration"
            );
        }
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    fn handle_block_created_with_preserve_policy(
        &mut self,
        msg: super::message::BlockCreated,
        sender: Option<PeerId>,
        allow_frontier_owner_preserve_on_payload_mismatch: bool,
        allow_authoritative_frontier_owner_supersede: bool,
    ) -> Result<()> {
        if crate::sumeragi::status::local_peer_removed() {
            debug!(
                ?sender,
                "dropping BlockCreated because local peer removed from world"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockCreated,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ModeMismatch,
            );
            return Ok(());
        }
        let frontier = msg.frontier;
        let block = msg.block;
        let block_hash = block.hash();
        let header = block.header();
        let height = header.height().get();
        let view = header.view_change_index();
        let session_key = Self::session_key(&block_hash, height, view);
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let committed_hash = self.state.latest_block_hash_fast();
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
            self.subsystems.validation.inflight.remove(&block_hash);
            self.subsystems
                .validation
                .superseded_results
                .remove(&block_hash);
            self.pending.pending_fetch_requests.remove(&block_hash);
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
            if self.should_clear_missing_request_on_stale_block_drop(
                block_hash,
                height,
                committed_height,
            ) {
                self.clear_missing_block_request(&block_hash, MissingBlockClearReason::Obsolete);
            } else if missing_request {
                debug!(
                    height,
                    view,
                    committed_height,
                    block = %block_hash,
                    "preserving committed-height missing request after stale BlockCreated drop"
                );
            }
            let matches_committed = committed_hash.is_some_and(|hash| hash == block_hash);
            if matches_committed {
                self.clean_rbc_sessions_for_committed_block_if_settled(block_hash, height);
            } else {
                self.clean_rbc_sessions_for_block(block_hash, height);
            }
            if !matches_committed {
                self.qc_cache
                    .retain(|(_, hash, _, _, _), _| *hash != block_hash);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _), _| *hash != block_hash);
            }
            return Ok(());
        }
        let da_enabled = self.runtime_da_enabled();
        let stale_view = self.stale_view(height, view);
        let stale_retired_match =
            self.pending
                .pending_blocks
                .get(&block_hash)
                .is_some_and(|pending| {
                    pending.is_retired_same_height()
                        && pending.height == height
                        && pending.view == view
                });
        let authoritative_frontier_owner_supersede = allow_authoritative_frontier_owner_supersede
            && height == committed_height.saturating_add(1);
        let passive_conflicting_same_height_vote = !authoritative_frontier_owner_supersede
            && self
                .local_conflicting_frontier_vote(height, block_hash)
                .is_some();
        let passive_conflicting_same_height_owner = !authoritative_frontier_owner_supersede
            && self
                .frontier_slot_conflicts_with_live_local_owner(height, view, block_hash)
                .is_some();
        let passive_conflicting_same_height =
            passive_conflicting_same_height_vote || passive_conflicting_same_height_owner;
        if authoritative_frontier_owner_supersede {
            debug!(
                height,
                view,
                block = %block_hash,
                "accepting BlockCreated as authoritative same-height supersede for the contiguous frontier"
            );
        } else if passive_conflicting_same_height_vote {
            debug!(
                height,
                view,
                block = %block_hash,
                "accepting BlockCreated as passive retained branch because local same-height vote history conflicts"
            );
        } else if passive_conflicting_same_height_owner {
            debug!(
                height,
                view,
                block = %block_hash,
                "accepting BlockCreated as passive retained branch because the current same-height frontier owner is still locally live"
            );
        }
        if let Some(local_view) = stale_view {
            if !allow_stale_block_created(missing_request, stale_retired_match) {
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
                missing_request,
                stale_retired_match,
                "accepting BlockCreated for stale view to recover missing payload"
            );
        }
        if let Some(sink) = self.active_lock_rejected_block_sink(height, block_hash) {
            let (
                lock_reject_votes_purged,
                lock_reject_qc_cache_purged,
                lock_reject_qc_tally_purged,
                lock_reject_deferred_roster_purged,
                lock_reject_known_work_purged,
                lock_reject_deferred_updates_purged,
                lock_reject_defer_markers_purged,
                lock_reject_pending_purged,
                lock_reject_hints_purged,
                lock_reject_proposals_purged,
            ) = self.purge_lock_rejected_block_artifacts(height, view, block_hash);
            warn!(
                height,
                view,
                block = %block_hash,
                locked_height = sink.locked_height,
                locked_hash = %sink.locked_hash,
                lock_reject_sink_rejections = sink.rejections,
                lock_reject_sink_fetch_suppressions = sink.fetch_suppressions,
                lock_reject_sink_last_source = sink.last_reject_source,
                lock_reject_votes_purged,
                lock_reject_qc_cache_purged,
                lock_reject_qc_tally_purged,
                lock_reject_deferred_roster_purged,
                lock_reject_known_work_purged,
                lock_reject_deferred_updates_purged,
                lock_reject_defer_markers_purged,
                lock_reject_pending_purged,
                lock_reject_hints_purged,
                lock_reject_proposals_purged,
                "dropping BlockCreated for an active lock-rejected branch sink"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockCreated,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return Ok(());
        }
        if allow_frontier_owner_preserve_on_payload_mismatch
            && let Some(owner_hash) = self.authoritative_slot_owner_hash(height, view)
            && owner_hash != block_hash
        {
            info!(
                height,
                view,
                block = %block_hash,
                owner = %owner_hash,
                "dropping conflicting BlockCreated for an authoritative slot owner"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockCreated,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadMismatch,
            );
            self.clear_missing_block_request(&block_hash, MissingBlockClearReason::Obsolete);
            self.clear_payload_mismatch_state(session_key, block_hash, height, view);
            self.finalize_collector_plan(false);
            return Ok(());
        }
        let expected_height = committed_height.saturating_add(1);
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
            let time_triggers_due = self.state.time_triggers_due_for_block_fast(&block.header());
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
                self.subsystems.validation.inflight.remove(&block_hash);
                self.subsystems
                    .validation
                    .superseded_results
                    .remove(&block_hash);
                self.pending.pending_fetch_requests.remove(&block_hash);
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
        let pending_status = self.pending.pending_blocks.get(&block_hash).map(|pending| {
            (
                pending.is_retry_aborted(),
                pending.is_retired_same_height(),
                pending.validation_status,
                pending.commit_qc_observed(),
            )
        });
        let stale_payload_only = stale_view.is_some() || passive_conflicting_same_height;
        let revive_aborted = !stale_payload_only
            && pending_status.is_some_and(|(aborted, _, status, commit_qc_seen)| {
                aborted && commit_qc_seen && !matches!(status, ValidationStatus::Invalid)
            });
        if pending_status.is_some() && !revive_aborted && !stale_payload_only {
            if da_enabled {
                let session_key = Self::session_key(&block_hash, height, view);
                if self.frontier_slot_is_exact_height(height) {
                    // The exact-frontier path already has a local pending block, so the live RBC
                    // runtime can retire. Keep the persisted snapshot and operator summary until
                    // commit or TTL cleanup so restart recovery still has durable payload state.
                    self.clear_rbc_runtime_state(session_key, false);
                    self.publish_rbc_backlog_snapshot();
                } else {
                    let payload_hash = self
                        .pending
                        .pending_blocks
                        .get(&block_hash)
                        .map(|pending| pending.payload_hash)
                        .unwrap_or_else(|| {
                            Hash::new(&super::proposals::block_payload_bytes(&block))
                        });
                    let rebroadcast_missing_init = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .pending
                        .contains_key(&session_key);
                    let mut seed_inflight = false;
                    if let Some(intent) = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .seed_inflight
                        .get_mut(&session_key)
                    {
                        if rebroadcast_missing_init {
                            intent.rebroadcast_missing_init = true;
                        }
                        seed_inflight = true;
                    }
                    let mut queued_seed = false;
                    if !seed_inflight
                        && !self
                            .subsystems
                            .da_rbc
                            .rbc
                            .sessions
                            .contains_key(&session_key)
                    {
                        if let Some(seed_tx) = self.subsystems.da_rbc.rbc.seed_tx.as_ref() {
                            let payload_bytes = super::proposals::block_payload_bytes(&block);
                            let payload_len = payload_bytes.len();
                            let work = super::rbc::RbcSeedWork {
                                key: session_key,
                                payload_hash,
                                payload_bytes,
                                chunking: super::rbc::RbcChunkingSpec::from_config(
                                    &self.config.rbc,
                                ),
                                epoch: self.epoch_for_height(height),
                                started_at: Instant::now(),
                            };
                            match seed_tx.try_send(work) {
                                Ok(()) => {
                                    self.subsystems.da_rbc.rbc.seed_inflight.insert(
                                        session_key,
                                        super::RbcSeedIntent {
                                            rebroadcast_missing_init,
                                        },
                                    );
                                    match self.insert_stub_rbc_session_from_block(
                                        session_key,
                                        &block,
                                        payload_hash,
                                        payload_len,
                                    ) {
                                        Ok(_) => {
                                            queued_seed = true;
                                            seed_inflight = true;
                                        }
                                        Err(err) => {
                                            warn!(
                                                height,
                                                view,
                                                block = %block_hash,
                                                error = %err,
                                                "failed to insert stub RBC session after seed enqueue"
                                            );
                                            self.subsystems
                                                .da_rbc
                                                .rbc
                                                .seed_inflight
                                                .remove(&session_key);
                                        }
                                    }
                                }
                                Err(mpsc::TrySendError::Full(_work)) => {
                                    debug!(
                                        ?session_key,
                                        "RBC seed queue full; seeding synchronously"
                                    );
                                }
                                Err(mpsc::TrySendError::Disconnected(_work)) => {
                                    warn!(
                                        ?session_key,
                                        "RBC seed worker disconnected; seeding synchronously"
                                    );
                                    self.subsystems.da_rbc.rbc.seed_tx = None;
                                    self.subsystems.da_rbc.rbc.seed_rx = None;
                                    self.subsystems.da_rbc.rbc.seed_inflight.clear();
                                }
                            }
                        }
                        if !queued_seed {
                            self.seed_rbc_session_from_block(
                                session_key,
                                &block,
                                payload_hash,
                                rebroadcast_missing_init,
                            )?;
                        }
                    }
                    if queued_seed
                        && self
                            .subsystems
                            .da_rbc
                            .rbc
                            .sessions
                            .get(&session_key)
                            .is_some_and(|session| {
                                super::rbc_session_needs_payload(session, payload_hash)
                            })
                    {
                        let payload_bytes = super::proposals::block_payload_bytes(&block);
                        self.hydrate_rbc_session_from_block(
                            session_key,
                            &payload_bytes,
                            payload_hash,
                            sender.as_ref(),
                        )?;
                        self.subsystems
                            .da_rbc
                            .rbc
                            .seed_inflight
                            .remove(&session_key);
                        seed_inflight = false;
                        self.retry_rbc_progress_after_block_created_hydration(session_key);
                        debug!(
                            height,
                            view,
                            block = %block_hash,
                            "hydrated RBC session inline after seed queueing for duplicate BlockCreated"
                        );
                        if rebroadcast_missing_init
                            && let Some(session) = self
                                .subsystems
                                .da_rbc
                                .rbc
                                .sessions
                                .get(&session_key)
                                .cloned()
                        {
                            self.rebroadcast_rbc_payload_for_missing_init(session_key, &session);
                        }
                    }
                    if !seed_inflight
                        && self
                            .subsystems
                            .da_rbc
                            .rbc
                            .sessions
                            .get(&session_key)
                            .is_some_and(|session| {
                                super::rbc_session_needs_payload(session, payload_hash)
                            })
                    {
                        if let Some(seed_tx) = self.subsystems.da_rbc.rbc.seed_tx.as_ref() {
                            let payload_bytes = super::proposals::block_payload_bytes(&block);
                            let work = super::rbc::RbcSeedWork {
                                key: session_key,
                                payload_hash,
                                payload_bytes,
                                chunking: super::rbc::RbcChunkingSpec::from_config(
                                    &self.config.rbc,
                                ),
                                epoch: self.epoch_for_height(height),
                                started_at: Instant::now(),
                            };
                            match seed_tx.try_send(work) {
                                Ok(()) => {
                                    self.subsystems.da_rbc.rbc.seed_inflight.insert(
                                        session_key,
                                        super::RbcSeedIntent {
                                            rebroadcast_missing_init,
                                        },
                                    );
                                    queued_seed = true;
                                }
                                Err(mpsc::TrySendError::Full(_work)) => {
                                    debug!(
                                        ?session_key,
                                        "RBC seed queue full; hydrating synchronously"
                                    );
                                }
                                Err(mpsc::TrySendError::Disconnected(_work)) => {
                                    warn!(
                                        ?session_key,
                                        "RBC seed worker disconnected; hydrating synchronously"
                                    );
                                    self.subsystems.da_rbc.rbc.seed_tx = None;
                                    self.subsystems.da_rbc.rbc.seed_rx = None;
                                    self.subsystems.da_rbc.rbc.seed_inflight.clear();
                                }
                            }
                        }
                        let payload_bytes = super::proposals::block_payload_bytes(&block);
                        self.hydrate_rbc_session_from_block(
                            session_key,
                            &payload_bytes,
                            payload_hash,
                            sender.as_ref(),
                        )?;
                        self.retry_rbc_progress_after_block_created_hydration(session_key);
                        if queued_seed {
                            self.subsystems
                                .da_rbc
                                .rbc
                                .seed_inflight
                                .remove(&session_key);
                            debug!(
                                height,
                                view,
                                block = %block_hash,
                                "hydrated RBC session inline after payload-seed queueing for duplicate BlockCreated"
                            );
                        }
                        if rebroadcast_missing_init
                            && let Some(session) = self
                                .subsystems
                                .da_rbc
                                .rbc
                                .sessions
                                .get(&session_key)
                                .cloned()
                        {
                            self.rebroadcast_rbc_payload_for_missing_init(session_key, &session);
                        }
                    }
                    let metadata_populated =
                        self.populate_rbc_session_metadata_from_block(session_key, &block);
                    if metadata_populated
                        || self
                            .subsystems
                            .da_rbc
                            .rbc
                            .sessions
                            .contains_key(&session_key)
                    {
                        self.retry_rbc_progress_after_block_created_hydration(session_key);
                    }
                }
            }
            self.note_frontier_block_created(
                block_hash,
                height,
                view,
                frontier.clone(),
                sender.as_ref(),
                Instant::now(),
            );
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
        let da_bundle_ms = if let Some(bundle) = block.da_commitments() {
            let da_start = Instant::now();
            self.validate_da_bundle(bundle)?;
            u64::try_from(da_start.elapsed().as_millis()).unwrap_or(u64::MAX)
        } else {
            0
        };
        let hint_start = Instant::now();
        let inline_hint = frontier
            .as_ref()
            .map(|frontier| super::message::ProposalHint {
                block_hash,
                height,
                view,
                highest_qc: frontier.highest_qc,
            });
        let inline_proposal = frontier.as_ref().map(|frontier| {
            Self::build_consensus_proposal(
                &block,
                frontier.payload_hash,
                frontier.highest_qc,
                frontier.proposer,
                view,
                frontier.epoch,
            )
        });
        let mut cached_hint = inline_hint.or_else(|| {
            self.subsystems
                .propose
                .proposal_cache
                .get_hint(height, view)
                .copied()
        });
        let cached_proposal = inline_proposal.clone().or_else(|| {
            self.subsystems
                .propose
                .proposal_cache
                .get_proposal(height, view)
                .copied()
        });
        let mut payload_bytes = None;
        let mut payload_hash = None;
        let mut payload_bytes_ms = 0u64;
        let mut payload_hash_ms = 0u64;
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
                let bytes_start = Instant::now();
                let computed_bytes = block_payload_bytes(&block);
                payload_bytes_ms =
                    u64::try_from(bytes_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                let hash_start = Instant::now();
                let computed_hash = Hash::new(&computed_bytes);
                payload_hash_ms =
                    u64::try_from(hash_start.elapsed().as_millis()).unwrap_or(u64::MAX);
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
            if hint_highest_missing {
                info!(
                    height,
                    view,
                    highest_height = hint_highest.height,
                    highest_view = hint_highest.view,
                    highest_hash = %hint_highest.subject_block_hash,
                    "deferring BlockCreated until highest QC dependency is repaired"
                );
                self.defer_round_until_highest_qc_dependency_resolves(
                    height,
                    view,
                    hint_highest,
                    "block_created_hint",
                );
                self.preserve_block_created_for_highest_qc_repair(
                    &block,
                    frontier.clone(),
                    inline_hint,
                    inline_proposal,
                    sender.clone(),
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockCreated,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::MissingHighestQc,
                );
                return Ok(());
            }
            if let Err(initial_reason) = ensure_locked_qc_allows(self.locked_qc, hint_highest) {
                if let Some(new_lock) = realign_locked_to_committed_if_extends(
                    self.locked_qc,
                    self.latest_committed_qc(),
                    hint_highest,
                    |hash, height| self.parent_hash_for(hash, height),
                ) {
                    if self.locked_qc != Some(new_lock) {
                        info!(
                            ?initial_reason,
                            locked_qc_height = self.locked_qc.map(|qc| qc.height),
                            locked_qc_view = self.locked_qc.map(|qc| qc.view),
                            locked_qc_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                            committed_qc_height = new_lock.height,
                            committed_qc_view = new_lock.view,
                            committed_qc_hash = %new_lock.subject_block_hash,
                            hint_highest_qc_height = hint.highest_qc.height,
                            hint_highest_qc_view = hint.highest_qc.view,
                            hint_highest_qc_hash = %hint.highest_qc.subject_block_hash,
                            height,
                            view,
                            "realigning stale locked QC to committed chain before applying BlockCreated hint"
                        );
                        self.locked_qc = Some(new_lock);
                        super::status::set_locked_qc(
                            new_lock.height,
                            new_lock.view,
                            Some(new_lock.subject_block_hash),
                        );
                    }
                }

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
                    } else if let Some(lock) = self.locked_qc.filter(|lock| {
                        matches!(reason, LockedQcRejection::HeightRegressed { .. })
                            && lock.height == height
                            && lock.subject_block_hash == block_hash
                    }) {
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
                            block = %block_hash,
                            "accepting BlockCreated for already-locked block despite stale hint highest QC"
                        );
                        hint_highest = lock;
                    } else {
                        if let Some(lock) = self.locked_qc {
                            self.note_lock_rejected_block(
                                height,
                                block_hash,
                                lock.height,
                                lock.subject_block_hash,
                                "block_created_hint_locked_qc_gate",
                            );
                            let _ =
                                self.purge_lock_rejected_block_artifacts(height, view, block_hash);
                        }
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
                let (pending_conflicts_purged, missing_conflicts_purged, deferred_conflicts_purged) =
                    self.locked_qc.map_or((0, 0, 0), |lock| {
                        self.purge_locked_conflicting_branch_state(height, lock.subject_block_hash)
                    });
                let mut lock_reject_votes_purged = 0usize;
                let mut lock_reject_qc_cache_purged = 0usize;
                let mut lock_reject_qc_tally_purged = 0usize;
                let mut lock_reject_deferred_roster_purged = 0usize;
                let mut lock_reject_known_work_purged = 0usize;
                let mut lock_reject_deferred_updates_purged = 0usize;
                let mut lock_reject_defer_markers_purged = 0usize;
                let mut lock_reject_pending_purged = 0usize;
                let mut lock_reject_hints_purged = 0usize;
                let mut lock_reject_proposals_purged = 0usize;
                if let Some(lock) = self.locked_qc {
                    self.note_lock_rejected_block(
                        height,
                        block_hash,
                        lock.height,
                        lock.subject_block_hash,
                        "block_created_hint_lock_gate",
                    );
                    (
                        lock_reject_votes_purged,
                        lock_reject_qc_cache_purged,
                        lock_reject_qc_tally_purged,
                        lock_reject_deferred_roster_purged,
                        lock_reject_known_work_purged,
                        lock_reject_deferred_updates_purged,
                        lock_reject_defer_markers_purged,
                        lock_reject_pending_purged,
                        lock_reject_hints_purged,
                        lock_reject_proposals_purged,
                    ) = self.purge_lock_rejected_block_artifacts(height, view, block_hash);
                    if self.should_clear_missing_request_on_locked_reject(
                        block_hash,
                        height,
                        lock.subject_block_hash,
                        lock.height,
                        header.prev_block_hash(),
                    ) {
                        self.clear_missing_block_request(
                            &block_hash,
                            MissingBlockClearReason::Obsolete,
                        );
                    } else {
                        debug!(
                            height,
                            view,
                            block = %block_hash,
                            locked_hash = %lock.subject_block_hash,
                            "preserving missing request for lock-rejected block: hash is not yet committed-conflicting"
                        );
                    }
                } else {
                    self.clear_missing_block_request(
                        &block_hash,
                        MissingBlockClearReason::Obsolete,
                    );
                }
                if let Some(lock) = self.locked_qc
                    && let Some(parent_hash) = header.prev_block_hash()
                {
                    let parent_height = height.saturating_sub(1);
                    if self.should_clear_missing_request_on_locked_reject(
                        parent_hash,
                        parent_height,
                        lock.subject_block_hash,
                        lock.height,
                        None,
                    ) {
                        self.clear_missing_block_request(
                            &parent_hash,
                            MissingBlockClearReason::Obsolete,
                        );
                    } else {
                        debug!(
                            height,
                            view,
                            parent_height,
                            parent_hash = %parent_hash,
                            locked_hash = %lock.subject_block_hash,
                            "preserving missing-parent request after lock rejection: parent hash is not yet committed locally"
                        );
                    }
                }
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
                    pending_conflicts_purged,
                    missing_conflicts_purged,
                    deferred_conflicts_purged,
                    lock_reject_votes_purged,
                    lock_reject_qc_cache_purged,
                    lock_reject_qc_tally_purged,
                    lock_reject_deferred_roster_purged,
                    lock_reject_known_work_purged,
                    lock_reject_deferred_updates_purged,
                    lock_reject_defer_markers_purged,
                    lock_reject_pending_purged,
                    lock_reject_hints_purged,
                    lock_reject_proposals_purged,
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
            if let Some(mut lock) = self.locked_qc {
                if let Some(highest_qc) = self.highest_qc {
                    let should_adopt_highest_conflict = highest_qc.height == lock.height
                        && highest_qc.subject_block_hash != lock.subject_block_hash
                        && highest_qc.view > lock.view
                        && ensure_locked_qc_allows(Some(lock), highest_qc).is_ok();
                    if should_adopt_highest_conflict {
                        info!(
                            locked_qc_height = lock.height,
                            locked_qc_view = lock.view,
                            locked_qc_hash = %lock.subject_block_hash,
                            highest_qc_height = highest_qc.height,
                            highest_qc_view = highest_qc.view,
                            highest_qc_hash = %highest_qc.subject_block_hash,
                            height,
                            view,
                            "adopting local higher-view highest QC before validating BlockCreated without hint"
                        );
                        self.locked_qc = Some(highest_qc);
                        super::status::set_locked_qc(
                            highest_qc.height,
                            highest_qc.view,
                            Some(highest_qc.subject_block_hash),
                        );
                        lock = highest_qc;
                    }
                }
                let locked_hash = lock.subject_block_hash;
                if self.block_known_for_lock(locked_hash) {
                    let parent_hash = header.prev_block_hash();
                    let mut extends = super::chain_extends_tip(
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
                    if matches!(extends, Some(false)) {
                        let block_qc = crate::sumeragi::consensus::QcHeaderRef {
                            height,
                            view,
                            epoch: self.epoch_for_height(height),
                            subject_block_hash: block_hash,
                            phase: crate::sumeragi::consensus::Phase::Commit,
                        };
                        if let Some(new_lock) = realign_locked_to_committed_if_extends(
                            self.locked_qc,
                            self.latest_committed_qc(),
                            block_qc,
                            |hash, height| {
                                if hash == block_hash {
                                    parent_hash
                                } else {
                                    self.parent_hash_for(hash, height)
                                }
                            },
                        ) {
                            if self.locked_qc != Some(new_lock) {
                                info!(
                                    locked_qc_height = self.locked_qc.map(|qc| qc.height),
                                    locked_qc_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                                    block_height = height,
                                    block_hash = %block_hash,
                                    committed_qc_height = new_lock.height,
                                    committed_qc_hash = %new_lock.subject_block_hash,
                                    "resetting locked QC to committed chain before accepting BlockCreated without hint"
                                );
                                self.locked_qc = Some(new_lock);
                                super::status::set_locked_qc(
                                    new_lock.height,
                                    new_lock.view,
                                    Some(new_lock.subject_block_hash),
                                );
                            }
                        }
                        if let Some(updated_lock) = self.locked_qc {
                            let updated_locked_hash = updated_lock.subject_block_hash;
                            extends = super::chain_extends_tip(
                                block_hash,
                                height,
                                updated_lock.height,
                                updated_locked_hash,
                                |hash, height| {
                                    if hash == block_hash {
                                        parent_hash
                                    } else {
                                        self.parent_hash_for(hash, height)
                                    }
                                },
                            );
                        }
                    }
                    match extends {
                        Some(false) => {
                            let (
                                pending_conflicts_purged,
                                missing_conflicts_purged,
                                deferred_conflicts_purged,
                            ) = self.purge_locked_conflicting_branch_state(height, locked_hash);
                            let (
                                lock_reject_votes_purged,
                                lock_reject_qc_cache_purged,
                                lock_reject_qc_tally_purged,
                                lock_reject_deferred_roster_purged,
                                lock_reject_known_work_purged,
                                lock_reject_deferred_updates_purged,
                                lock_reject_defer_markers_purged,
                                lock_reject_pending_purged,
                                lock_reject_hints_purged,
                                lock_reject_proposals_purged,
                            ) = {
                                self.note_lock_rejected_block(
                                    height,
                                    block_hash,
                                    lock.height,
                                    locked_hash,
                                    "block_created_no_hint_lock_gate",
                                );
                                self.purge_lock_rejected_block_artifacts(height, view, block_hash)
                            };
                            // Invariant A: a lock-conflicting branch must not keep stale
                            // missing-parent requests alive after deterministic rejection.
                            if self.should_clear_missing_request_on_locked_reject(
                                block_hash,
                                height,
                                locked_hash,
                                lock.height,
                                parent_hash,
                            ) {
                                self.clear_missing_block_request(
                                    &block_hash,
                                    MissingBlockClearReason::Obsolete,
                                );
                            } else {
                                debug!(
                                    height,
                                    view,
                                    block = %block_hash,
                                    locked_hash = %locked_hash,
                                    "preserving missing request for lock-rejected block: hash is not yet committed-conflicting"
                                );
                            }
                            let parent_hash_for_reclear = parent_hash;
                            let mut cleared_existing_parent_request = false;
                            let mut parent_request_tracked = false;
                            if let Some(parent_hash) = parent_hash_for_reclear.as_ref() {
                                let parent_had_request = self
                                    .pending
                                    .missing_block_requests
                                    .contains_key(parent_hash);
                                parent_request_tracked = parent_had_request;
                                let parent_height = height.saturating_sub(1);
                                if self.should_clear_missing_request_on_locked_reject(
                                    *parent_hash,
                                    parent_height,
                                    locked_hash,
                                    lock.height,
                                    None,
                                ) {
                                    self.clear_missing_block_request(
                                        parent_hash,
                                        MissingBlockClearReason::Obsolete,
                                    );
                                    if parent_had_request {
                                        cleared_existing_parent_request = true;
                                    }
                                } else {
                                    debug!(
                                        height,
                                        view,
                                        parent_height,
                                        parent_hash = %parent_hash,
                                        locked_hash = %locked_hash,
                                        "preserving missing-parent request after lock rejection: parent hash is not yet committed locally"
                                    );
                                }
                            }
                            super::status::inc_block_created_dropped_by_lock();
                            #[cfg(feature = "telemetry")]
                            self.telemetry.inc_block_created_dropped_by_lock();
                            if !cleared_existing_parent_request && !parent_request_tracked {
                                self.trigger_active_height_lock_reject_recovery(
                                    height,
                                    view,
                                    block_hash,
                                    "block_created_no_hint_lock_gate",
                                );
                            }
                            // Active-height recovery can schedule immediate fetches; keep lock-rejected
                            // parent requests removed when this branch already proved they are obsolete.
                            if cleared_existing_parent_request
                                && let Some(parent_hash) = parent_hash_for_reclear.as_ref()
                            {
                                self.clear_missing_block_request(
                                    parent_hash,
                                    MissingBlockClearReason::Obsolete,
                                );
                            }
                            warn!(
                                locked_qc_height = lock.height,
                                locked_qc_view = lock.view,
                                locked_qc_hash = %locked_hash,
                                height,
                                view,
                                block = %block_hash,
                                pending_conflicts_purged,
                                missing_conflicts_purged,
                                deferred_conflicts_purged,
                                lock_reject_votes_purged,
                                lock_reject_qc_cache_purged,
                                lock_reject_qc_tally_purged,
                                lock_reject_deferred_roster_purged,
                                lock_reject_known_work_purged,
                                lock_reject_deferred_updates_purged,
                                lock_reject_defer_markers_purged,
                                lock_reject_pending_purged,
                                lock_reject_hints_purged,
                                lock_reject_proposals_purged,
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
        let hint_validation_ms =
            u64::try_from(hint_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let payload_bytes = match payload_bytes {
            Some(payload_bytes) => payload_bytes,
            None => {
                let bytes_start = Instant::now();
                let computed_bytes = block_payload_bytes(&block);
                payload_bytes_ms =
                    u64::try_from(bytes_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                computed_bytes
            }
        };
        let payload_hash = match payload_hash {
            Some(payload_hash) => payload_hash,
            None => {
                let hash_start = Instant::now();
                let computed_hash = Hash::new(&payload_bytes);
                payload_hash_ms =
                    u64::try_from(hash_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                computed_hash
            }
        };
        let cached_proposal_mismatch = cached_proposal
            .as_ref()
            .and_then(|proposal| detect_proposal_mismatch(proposal, &header, &payload_hash));
        if let Some(reason) = proposal_mismatch
            .or(cached_proposal_mismatch)
            .map(|mismatch| mismatch.reason())
        {
            let preserve_frontier_owner = allow_frontier_owner_preserve_on_payload_mismatch
                && self.preserve_contiguous_frontier_owner_on_payload_mismatch(height, view);
            self.invalidate_proposal(block_hash, height, view, reason, preserve_frontier_owner)?;
            if preserve_frontier_owner {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockCreated,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::PayloadMismatch,
                );
                self.clear_payload_mismatch_state(session_key, block_hash, height, view);
                self.finalize_collector_plan(false);
                return Ok(());
            }
            // Keep processing the payload after invalidating a stale proposal.
        }
        let exact_frontier_block_created = self.frontier_slot_is_exact_height(height);
        let (rbc_seed_ms, rbc_hydrate_ms) = if da_enabled && !exact_frontier_block_created {
            let mut seed_ms = 0u64;
            let mut hydrate_ms = 0u64;
            let rebroadcast_missing_init = self
                .subsystems
                .da_rbc
                .rbc
                .pending
                .contains_key(&session_key);
            let mut seed_inflight = false;
            if let Some(intent) = self
                .subsystems
                .da_rbc
                .rbc
                .seed_inflight
                .get_mut(&session_key)
            {
                if rebroadcast_missing_init {
                    intent.rebroadcast_missing_init = true;
                }
                seed_inflight = true;
            }
            if !seed_inflight {
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
                        "BlockCreated arrived before RBC session initialised; queuing seed work"
                    );
                    let seed_start = Instant::now();
                    let mut queued_seed = false;
                    if let Some(seed_tx) = self.subsystems.da_rbc.rbc.seed_tx.as_ref() {
                        let payload_len = payload_bytes.len();
                        let work = super::rbc::RbcSeedWork {
                            key: session_key,
                            payload_hash,
                            payload_bytes: payload_bytes.clone(),
                            chunking: super::rbc::RbcChunkingSpec::from_config(&self.config.rbc),
                            epoch: self.epoch_for_height(height),
                            started_at: Instant::now(),
                        };
                        match seed_tx.try_send(work) {
                            Ok(()) => {
                                self.subsystems.da_rbc.rbc.seed_inflight.insert(
                                    session_key,
                                    super::RbcSeedIntent {
                                        rebroadcast_missing_init,
                                    },
                                );
                                match self.insert_stub_rbc_session_from_block(
                                    session_key,
                                    &block,
                                    payload_hash,
                                    payload_len,
                                ) {
                                    Ok(_) => {
                                        queued_seed = true;
                                    }
                                    Err(err) => {
                                        warn!(
                                            height,
                                            view,
                                            block = %block_hash,
                                            error = %err,
                                            "failed to insert stub RBC session after seed enqueue"
                                        );
                                        self.subsystems
                                            .da_rbc
                                            .rbc
                                            .seed_inflight
                                            .remove(&session_key);
                                    }
                                }
                            }
                            Err(mpsc::TrySendError::Full(_work)) => {
                                debug!(?session_key, "RBC seed queue full; seeding synchronously");
                            }
                            Err(mpsc::TrySendError::Disconnected(_work)) => {
                                warn!(
                                    ?session_key,
                                    "RBC seed worker disconnected; seeding synchronously"
                                );
                                self.subsystems.da_rbc.rbc.seed_tx = None;
                                self.subsystems.da_rbc.rbc.seed_rx = None;
                                self.subsystems.da_rbc.rbc.seed_inflight.clear();
                            }
                        }
                    }
                    if !queued_seed {
                        self.seed_rbc_session_from_block(
                            session_key,
                            &block,
                            payload_hash,
                            rebroadcast_missing_init,
                        )?;
                    }
                    seed_ms = u64::try_from(seed_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    if queued_seed
                        && self
                            .subsystems
                            .da_rbc
                            .rbc
                            .sessions
                            .get(&session_key)
                            .is_some_and(|session| {
                                super::rbc_session_needs_payload(session, payload_hash)
                            })
                    {
                        let hydrate_start = Instant::now();
                        self.hydrate_rbc_session_from_block(
                            session_key,
                            &payload_bytes,
                            payload_hash,
                            sender.as_ref(),
                        )?;
                        hydrate_ms =
                            u64::try_from(hydrate_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                        self.subsystems
                            .da_rbc
                            .rbc
                            .seed_inflight
                            .remove(&session_key);
                        self.retry_rbc_progress_after_block_created_hydration(session_key);
                        debug!(
                            height,
                            view,
                            block = %block_hash,
                            "hydrated RBC session inline after seed queueing for BlockCreated"
                        );
                        if rebroadcast_missing_init
                            && let Some(session) = self
                                .subsystems
                                .da_rbc
                                .rbc
                                .sessions
                                .get(&session_key)
                                .cloned()
                        {
                            self.rebroadcast_rbc_payload_for_missing_init(session_key, &session);
                        }
                    }
                } else if self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&session_key)
                    .is_some_and(|session| super::rbc_session_needs_payload(session, payload_hash))
                {
                    let seed_start = Instant::now();
                    let mut queued_seed = false;
                    if let Some(seed_tx) = self.subsystems.da_rbc.rbc.seed_tx.as_ref() {
                        let work = super::rbc::RbcSeedWork {
                            key: session_key,
                            payload_hash,
                            payload_bytes: payload_bytes.clone(),
                            chunking: super::rbc::RbcChunkingSpec::from_config(&self.config.rbc),
                            epoch: self.epoch_for_height(height),
                            started_at: Instant::now(),
                        };
                        match seed_tx.try_send(work) {
                            Ok(()) => {
                                self.subsystems.da_rbc.rbc.seed_inflight.insert(
                                    session_key,
                                    super::RbcSeedIntent {
                                        rebroadcast_missing_init,
                                    },
                                );
                                queued_seed = true;
                            }
                            Err(mpsc::TrySendError::Full(_work)) => {
                                debug!(
                                    ?session_key,
                                    "RBC seed queue full; hydrating synchronously"
                                );
                            }
                            Err(mpsc::TrySendError::Disconnected(_work)) => {
                                warn!(
                                    ?session_key,
                                    "RBC seed worker disconnected; hydrating synchronously"
                                );
                                self.subsystems.da_rbc.rbc.seed_tx = None;
                                self.subsystems.da_rbc.rbc.seed_rx = None;
                                self.subsystems.da_rbc.rbc.seed_inflight.clear();
                            }
                        }
                    }
                    seed_ms = u64::try_from(seed_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let hydrate_start = Instant::now();
                    self.hydrate_rbc_session_from_block(
                        session_key,
                        &payload_bytes,
                        payload_hash,
                        sender.as_ref(),
                    )?;
                    self.retry_rbc_progress_after_block_created_hydration(session_key);
                    hydrate_ms =
                        u64::try_from(hydrate_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    if queued_seed {
                        self.subsystems
                            .da_rbc
                            .rbc
                            .seed_inflight
                            .remove(&session_key);
                        debug!(
                            height,
                            view,
                            block = %block_hash,
                            "hydrated RBC session inline after payload-seed queueing for BlockCreated"
                        );
                    }
                    if rebroadcast_missing_init
                        && let Some(session) = self
                            .subsystems
                            .da_rbc
                            .rbc
                            .sessions
                            .get(&session_key)
                            .cloned()
                    {
                        self.rebroadcast_rbc_payload_for_missing_init(session_key, &session);
                    }
                }
            }
            (seed_ms, hydrate_ms)
        } else {
            if da_enabled && exact_frontier_block_created {
                self.clear_rbc_runtime_state(session_key, false);
                self.persist_exact_frontier_rbc_recovery_snapshot(
                    session_key,
                    &block,
                    &payload_bytes,
                    payload_hash,
                )?;
            }
            (0, 0)
        };
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
        if da_enabled && !exact_frontier_block_created {
            let _ = self.populate_rbc_session_metadata_from_block(session_key, &block);
        }
        match self.pending.pending_blocks.entry(block_hash) {
            Entry::Occupied(mut occ) => {
                if revive_aborted {
                    let commit_qc_epoch = occ.get().commit_qc_epoch;
                    let pending = occ.get_mut();
                    pending.revive_after_abort(block, payload_hash, height, view);
                    if let Some(epoch) = commit_qc_epoch {
                        pending.note_commit_qc_observed(epoch);
                    }
                } else if stale_payload_only {
                    let pending = occ.get_mut();
                    if pending.is_retired_same_height() {
                        pending.refresh_retired_payload(block, payload_hash, height, view);
                    } else {
                        pending.replace_block(block, payload_hash, height, view);
                        pending.retire_same_height();
                    }
                } else {
                    occ.get_mut()
                        .replace_block(block, payload_hash, height, view);
                }
            }
            Entry::Vacant(vac) => {
                let mut pending = PendingBlock::new(block, payload_hash, height, view);
                if stale_payload_only {
                    pending.retire_same_height();
                }
                vac.insert(pending);
            }
        }
        if stale_payload_only {
            self.slot_tracker.note_retained_branch(
                height,
                view,
                block_hash,
                frontier.clone(),
                true,
                Instant::now(),
            );
        }
        if !stale_payload_only {
            if authoritative_frontier_owner_supersede {
                self.drop_superseded_contiguous_frontier_owner_state(block_hash, height, view);
            }
            if let Some(hint) = inline_hint {
                self.subsystems.propose.proposal_cache.insert_hint(hint);
            }
            if let Some(proposal) = inline_proposal {
                self.subsystems
                    .propose
                    .proposal_cache
                    .insert_proposal(proposal);
                self.note_proposal_seen(height, view, payload_hash);
            }
            self.note_authoritative_slot_owner(height, view, block_hash);
        }
        if stale_payload_only {
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            self.clear_missing_block_view_change(&block_hash);
        } else {
            if authoritative_frontier_owner_supersede {
                self.note_frontier_block_created_authoritatively(
                    block_hash,
                    height,
                    view,
                    frontier.clone(),
                    sender.as_ref(),
                    Instant::now(),
                );
            } else {
                self.note_frontier_block_created(
                    block_hash,
                    height,
                    view,
                    frontier.clone(),
                    sender.as_ref(),
                    Instant::now(),
                );
            }
            if height == committed_height.saturating_add(1) {
                self.note_view_change_from_block(height, view);
            }
        }
        self.record_phase_sample(PipelinePhase::CollectDa, height, view);
        if let Some(qc) = qc_cache_for_subject(&self.qc_cache, block_hash)
            .filter(|qc| {
                qc.phase == crate::sumeragi::consensus::Phase::Commit && qc.height == height
            })
            .max_by_key(|qc| qc.view)
            .cloned()
        {
            let block_known_for_lock = self.block_known_for_lock(block_hash);
            if self.process_precommit_qc(&qc, block_known_for_lock, false) {
                debug!(
                    height,
                    view,
                    block = %block_hash,
                    "applied cached precommit QC after block became known"
                );
            }
        }

        if da_enabled && !exact_frontier_block_created {
            let mut should_update_status = false;
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
                    should_update_status = true;
                }
            }
            if should_update_status {
                if let Some(session) = self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&session_key)
                    .cloned()
                {
                    self.update_rbc_status_entry(session_key, &session, false);
                }
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
                        block_hash,
                        height,
                        view,
                        format!(
                            "payload hash mismatch: expected {expected_hash:?}, observed {payload_hash:?}",
                        ),
                        allow_frontier_owner_preserve_on_payload_mismatch
                            && self.preserve_contiguous_frontier_owner_on_payload_mismatch(height, view),
                    )?;
                    self.clear_payload_mismatch_state(session_key, block_hash, height, view);
                    self.finalize_collector_plan(false);
                    return Ok(());
                }
            }
        }

        // Keep proposal context cached for this slot so stalled peers can recover from
        // `BlockCreated` retransmits that race ahead of proposal delivery.
        if da_enabled
            && !exact_frontier_block_created
            && self.promote_rbc_session_roster_and_retry(session_key)
        {
            debug!(
                height,
                view,
                block = %block_hash,
                "promoted derived RBC roster and retried pending READY/DELIVER after BlockCreated"
            );
        }
        if let Some(block) = self
            .pending
            .pending_blocks
            .get(&block_hash)
            .map(|pending| pending.block.clone())
        {
            self.flush_frontier_body_requesters(&block);
            self.flush_pending_fetch_requests(&block);
        }
        self.clear_missing_block_request(&block_hash, MissingBlockClearReason::PayloadAvailable);

        // If votes or cached QCs already exist for this block, re-evaluate now that the payload is
        // present so late-arriving block payloads can still finalize with previously collected votes.
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        self.observe_sidecar_mismatch_for_height(height, block_hash, "proposal_payload_ready");
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let allow_hash_coupled_roster = height <= committed_height;
        let allow_sidecar = !self.sidecar_quarantined_for_height(height);
        let mut commit_topology =
            if self.vote_roster_cache.contains_key(&block_hash) || !allow_hash_coupled_roster {
                Vec::new()
            } else {
                super::persisted_roster_for_block(
                    self.state.as_ref(),
                    &self.kura,
                    consensus_mode,
                    height,
                    block_hash,
                    Some(view),
                    &self.roster_validation_cache,
                    Some(&mut self.block_sync_roster_cache),
                    allow_sidecar,
                )
                .map(|selection| {
                    self.cache_vote_roster(block_hash, height, view, selection.roster.clone());
                    selection.roster
                })
                .unwrap_or_default()
            };
        if commit_topology.is_empty() {
            commit_topology =
                self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        }
        let qc_replay_start = Instant::now();
        if !commit_topology.is_empty() {
            self.cache_vote_roster(block_hash, height, view, commit_topology.clone());
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
            let cached_qcs: Vec<_> = qc_cache_for_subject(&self.qc_cache, block_hash)
                .filter(|qc| !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit))
                .cloned()
                .collect();
            for qc in cached_qcs {
                if let Err(err) = self.handle_qc(qc) {
                    warn!(?err, "failed to replay cached QC after block payload");
                }
            }
            let _ = self.try_replay_deferred_qcs();
            let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
        }
        let qc_replay_ms = u64::try_from(qc_replay_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.request_commit_pipeline_for_pending(
            block_hash,
            super::status::RoundEventCauseTrace::BlockAvailable,
            None,
        );
        debug!(
            height,
            view,
            block = %block_hash,
            da_bundle_ms,
            hint_validation_ms,
            payload_bytes_ms,
            payload_hash_ms,
            rbc_seed_ms,
            rbc_hydrate_ms,
            qc_replay_ms,
            "block created substep timings"
        );
        Ok(())
    }
    pub(super) fn invalidate_proposal(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        reason: String,
        preserve_frontier_owner: bool,
    ) -> Result<()> {
        let now = Instant::now();
        let proposal_opt = if preserve_frontier_owner {
            self.subsystems
                .propose
                .proposal_cache
                .get_proposal(height, view)
                .cloned()
        } else {
            self.subsystems
                .propose
                .proposal_cache
                .pop_proposal(height, view)
        };
        if !preserve_frontier_owner {
            self.subsystems
                .propose
                .proposal_cache
                .pop_hint(height, view);
        }
        super::status::inc_block_created_proposal_mismatch();
        #[cfg(feature = "telemetry")]
        self.telemetry.inc_block_created_proposal_mismatch();
        if let Some(proposal) = proposal_opt {
            warn!(
                height,
                view,
                reason = reason.as_str(),
                preserve_frontier_owner,
                "dropping BlockCreated due to proposal mismatch"
            );
            let evidence = invalid_proposal_evidence(proposal, reason);
            self.record_and_broadcast_evidence(evidence)?;
        } else {
            warn!(
                height,
                view,
                reason = reason.as_str(),
                preserve_frontier_owner,
                "proposal mismatch detected but no cached proposal available to emit evidence"
            );
        }
        if preserve_frontier_owner {
            debug!(
                height,
                view,
                block = %block_hash,
                "suppressing payload-mismatch recovery bundle while an active contiguous-frontier owner remains in place"
            );
            return Ok(());
        }
        let _ = self.maybe_trigger_payload_mismatch_recovery_bundle(
            height,
            view,
            block_hash,
            now,
            "proposal_payload_mismatch",
        );
        Ok(())
    }

    fn preserve_contiguous_frontier_owner_on_payload_mismatch(
        &self,
        height: u64,
        view: u64,
    ) -> bool {
        let committed_height = self.committed_height_snapshot();
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        height == committed_height.saturating_add(1)
            && self.pending.pending_blocks.values().any(|pending| {
                let block_hash = pending.block.hash();
                !pending.aborted
                    && pending.height == height
                    && pending.view == view
                    && pending_extends_tip(
                        pending.height,
                        pending.block.header().prev_block_hash(),
                        tip_height,
                        tip_hash,
                    )
                    && self.pending_block_has_consensus_evidence(block_hash, pending)
            })
    }

    pub(super) fn clear_payload_mismatch_state(
        &mut self,
        session_key: SessionKey,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) {
        self.pending.pending_blocks.remove(&block_hash);
        self.subsystems.validation.inflight.remove(&block_hash);
        self.subsystems
            .validation
            .superseded_results
            .remove(&block_hash);
        self.pending.pending_fetch_requests.remove(&block_hash);
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
