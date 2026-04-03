//! Block sync and missing-block request handlers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use iroha_logger::prelude::*;
use norito::codec::Encode as _;

use crate::sumeragi::message::BlockMessageWire;

use super::locked_qc::qc_extends_locked_with_lookup;
use super::message::FetchPendingBlockPriority;
use super::*;

fn allow_uncertified_block_sync_roster(
    block_height: u64,
    local_height: u64,
    requested_missing_block: bool,
) -> bool {
    let _ = (block_height, local_height);
    requested_missing_block
}

impl Actor {
    fn should_defer_canonical_committed_fetch_response(
        &self,
        block: &SignedBlock,
        msg: &BlockMessage,
    ) -> bool {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let local_committed_height = self.committed_height_snapshot();
        if block_height != local_committed_height {
            return false;
        }
        if self.committed_block_hash_for_height(block_height) != Some(block_hash) {
            return false;
        }
        match msg {
            BlockMessage::BlockCreated(_) => true,
            BlockMessage::BlockSyncUpdate(update) => {
                update.commit_qc.is_none() && update.validator_checkpoint.is_none()
            }
            _ => false,
        }
    }

    pub(super) fn commit_qc_from_validator_checkpoint(
        &self,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        checkpoint: &ValidatorSetCheckpoint,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        if checkpoint.block_hash != block_hash {
            warn!(
                expected = %block_hash,
                actual = %checkpoint.block_hash,
                "ignoring validator checkpoint that does not match block hash"
            );
            return None;
        }
        if checkpoint.height != block_height {
            warn!(
                expected = block_height,
                actual = checkpoint.height,
                block = %block_hash,
                "ignoring validator checkpoint that does not match block height"
            );
            return None;
        }
        if checkpoint.view != block_view {
            warn!(
                expected = block_view,
                actual = checkpoint.view,
                block = %block_hash,
                height = block_height,
                "ignoring validator checkpoint that does not match block view"
            );
            return None;
        }
        let (consensus_mode, mode_tag, _prf_seed) = self.consensus_context_for_height(block_height);
        let expected_epoch = self.epoch_for_height(block_height);
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => self
                .roster_validation_cache
                .stake_snapshot_for_roster(&checkpoint.validator_set),
        };
        let inputs = self.roster_validation_cache.inputs_for_roster(
            &checkpoint.validator_set,
            consensus_mode,
            stake_snapshot.as_ref(),
        );
        let allow_genesis_stub = block_height == 1 && block_view == 0;
        if let Err(err) = super::validate_checkpoint_roster_cached(
            &self.roster_validation_cache,
            checkpoint,
            block_hash,
            block_height,
            Some(block_view),
            consensus_mode,
            &self.common_config.chain,
            mode_tag,
            expected_epoch,
            Some((checkpoint.parent_state_root, checkpoint.post_state_root)),
            allow_genesis_stub,
            &inputs,
        ) {
            warn!(
                ?err,
                block = %block_hash,
                height = block_height,
                view = block_view,
                "ignoring uncertified validator checkpoint sidecar"
            );
            return None;
        }
        Some(crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: checkpoint.block_hash,
            parent_state_root: checkpoint.parent_state_root,
            post_state_root: checkpoint.post_state_root,
            height: checkpoint.height,
            view: checkpoint.view,
            epoch: expected_epoch,
            mode_tag: mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: checkpoint.validator_set_hash,
            validator_set_hash_version: checkpoint.validator_set_hash_version,
            validator_set: checkpoint.validator_set.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: checkpoint.signers_bitmap.clone(),
                bls_aggregate_signature: checkpoint.bls_aggregate_signature.clone(),
            },
        })
    }

    pub(super) fn block_sync_qc_is_missing_context_error(err: &QcValidationError) -> bool {
        matches!(
            err,
            QcValidationError::MissingVotes { .. }
                | QcValidationError::StakeSnapshotUnavailable
                // Block-sync QC hints can arrive before sidecar/roster convergence. Treat
                // aggregate mismatches as retryable to avoid churny drop/refetch loops.
                | QcValidationError::AggregateMismatch
        )
    }

    pub(super) fn has_missing_block_request_for_height(&self, height: u64) -> bool {
        self.pending
            .missing_block_requests
            .values()
            .any(|request| request.height == height)
    }

    fn block_sync_qc_final_drop(&mut self, reason: &'static str) {
        super::status::inc_blocksync_qc_final_drop(reason);
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_blocksync_qc_final_drop(reason);
        }
    }

    fn quarantine_block_sync_qc_candidate(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        reason: &'static str,
        target: QuarantinedQcTarget,
    ) {
        let key = Self::qc_tally_key(&qc);
        let now = Instant::now();
        if let Some(existing) = self.quarantined_block_sync_qcs.get_mut(&key) {
            existing.qc = qc;
            existing.reason = reason;
            existing.last_attempt = now;
            return;
        }
        if self.quarantined_block_sync_qcs.len() >= QUARANTINED_BLOCK_SYNC_QC_CAP {
            let oldest = self
                .quarantined_block_sync_qcs
                .iter()
                .min_by_key(|(key, entry)| (entry.first_seen, **key))
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                self.quarantined_block_sync_qcs.remove(&oldest);
                self.block_sync_qc_final_drop("capacity");
            }
        }
        self.quarantined_block_sync_qcs.insert(
            key,
            QuarantinedQcCandidate {
                qc,
                first_seen: now,
                last_attempt: now,
                attempts: 0,
                escalated_fetch: false,
                reason,
                target,
            },
        );
        super::status::inc_blocksync_qc_quarantine();
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_blocksync_qc_quarantine();
        }
    }

    fn force_block_sync_fetch_for_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        reason: &'static str,
    ) {
        let mut roster = qc.validator_set.clone();
        if roster.is_empty() {
            let (consensus_mode, _, _) = self.consensus_context_for_height(qc.height);
            roster = self.roster_for_vote_with_mode(
                qc.subject_block_hash,
                qc.height,
                qc.view,
                consensus_mode,
            );
        }
        if roster.is_empty() {
            return;
        }
        let topology = super::network_topology::Topology::new(roster);
        let signer_set =
            super::qc_signer_indices(qc, topology.as_ref().len(), topology.as_ref().len())
                .map(|parsed| parsed.voting.into_iter().collect::<BTreeSet<_>>())
                .unwrap_or_default();
        let targets = Self::build_fetch_targets(&signer_set, &topology);
        if targets.is_empty() {
            return;
        }
        self.request_missing_block(
            qc.subject_block_hash,
            qc.height,
            qc.view,
            super::MissingBlockPriority::Consensus,
            &targets,
        );
        debug!(
            height = qc.height,
            view = qc.view,
            block = %qc.subject_block_hash,
            targets = targets.len(),
            reason,
            "forcing block-sync fetch for quarantined QC"
        );
    }

    pub(super) fn keep_exact_frontier_block_sync_repair_in_slot(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        topology: &super::network_topology::Topology,
        reason: &'static str,
    ) -> bool {
        let now = Instant::now();
        if !self.handle_frontier_body_gap_with_topology(
            block_hash,
            block_height,
            block_view,
            signers,
            topology,
            true,
            now,
        ) {
            return false;
        }
        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            signer_count = signers.len(),
            roster_len = topology.as_ref().len(),
            reason,
            "routing contiguous frontier block sync recovery through exact body repair"
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_request_pending_block_for_missing_qc(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        block_signer_count: usize,
        commit_quorum: usize,
        block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        topology: &super::network_topology::Topology,
    ) -> bool {
        let now = Instant::now();
        let retry_window = self.rebroadcast_cooldown();
        let aggressive_after_attempts = self.recovery_missing_fetch_aggressive_after_attempts();
        let existing_attempts = self
            .pending
            .missing_block_requests
            .get(&block_hash)
            .map_or(0, |stats| stats.attempts);
        let fetch_mode = if existing_attempts >= aggressive_after_attempts {
            super::MissingBlockFetchMode::AggressiveTopology
        } else {
            super::MissingBlockFetchMode::Default
        };
        let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
        let decision = super::plan_missing_block_fetch_with_mode(
            &mut self.pending.missing_block_requests,
            block_hash,
            block_height,
            block_view,
            crate::sumeragi::consensus::Phase::Commit,
            super::MissingBlockPriority::Background,
            block_signers,
            topology,
            now,
            retry_window,
            None,
            signer_fallback_attempts,
            fetch_mode,
            false,
        );
        let dwell = self
            .pending
            .missing_block_requests
            .get(&block_hash)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let targets_len = match &decision {
            super::MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);

        // Invariant A: every sparse missing-QC signal must either advance request state in-place
        // or be explicitly backoff-suppressed with an existing request.
        let no_targets = matches!(&decision, super::MissingBlockFetchDecision::NoTargets);
        match &decision {
            super::MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(
                    block_hash,
                    block_height,
                    block_view,
                    super::MissingBlockPriority::Background,
                    &targets,
                );
                info!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    block_signers = block_signer_count,
                    commit_quorum,
                    target_kind = target_kind.label(),
                    retry_window_ms = retry_window.as_millis(),
                    "requesting pending block to recover missing QC"
                );
            }
            super::MissingBlockFetchDecision::Backoff => {
                trace!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    retry_window_ms = retry_window.as_millis(),
                    "suppressing duplicate pending-block fetch during missing-block backoff"
                );
            }
            super::MissingBlockFetchDecision::NoTargets => {
                warn!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    block_signers = block_signer_count,
                    commit_quorum,
                    retry_window_ms = retry_window.as_millis(),
                    "missing-block recovery tracked but no fetch targets available"
                );
            }
        }
        let tracked = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        debug_assert!(
            tracked || no_targets,
            "sparse missing-QC recovery must track request state unless no targets are available"
        );
        tracked
    }

    pub(super) fn try_replay_quarantined_block_sync_qcs(
        &mut self,
        now: Instant,
        tick_deadline: Option<Instant>,
    ) -> bool {
        if self.quarantined_block_sync_qcs.is_empty() {
            return false;
        }

        enum ReplayAction {
            Replay {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
                target: QuarantinedQcTarget,
            },
            Escalate {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
                target: QuarantinedQcTarget,
            },
            Expire {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
                target: QuarantinedQcTarget,
            },
        }

        let mut actions = Vec::new();
        let keys: Vec<_> = self
            .quarantined_block_sync_qcs
            .keys()
            .cloned()
            .take(QUARANTINED_BLOCK_SYNC_QC_PER_TICK)
            .collect();
        for key in keys {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
            let Some(entry) = self.quarantined_block_sync_qcs.get(&key) else {
                continue;
            };
            if self.block_known_locally(entry.qc.subject_block_hash) {
                actions.push(ReplayAction::Replay {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                    target: entry.target,
                });
                continue;
            }
            let age = now.saturating_duration_since(entry.first_seen);
            if age < self.recovery_deferred_qc_ttl() {
                continue;
            }
            if entry.escalated_fetch || entry.attempts >= QUARANTINED_BLOCK_SYNC_QC_MAX_ATTEMPTS {
                actions.push(ReplayAction::Expire {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                    target: entry.target,
                });
            } else {
                actions.push(ReplayAction::Escalate {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                    target: entry.target,
                });
            }
        }

        if actions.is_empty() {
            return false;
        }

        let mut progress = false;
        for action in actions {
            match action {
                ReplayAction::Replay {
                    key,
                    qc,
                    reason,
                    target,
                } => {
                    self.quarantined_block_sync_qcs.remove(&key);
                    match self.handle_qc(qc.clone()) {
                        Ok(()) => {
                            super::status::inc_blocksync_qc_revalidated();
                            #[cfg(feature = "telemetry")]
                            if let Some(telemetry) = self.telemetry_handle() {
                                telemetry.inc_blocksync_qc_revalidated();
                            }
                            progress = true;
                        }
                        Err(err) => {
                            warn!(
                                ?err,
                                reason,
                                ?target,
                                "failed to replay quarantined block-sync QC"
                            );
                            self.block_sync_qc_final_drop("replay_error");
                            progress = true;
                        }
                    }
                }
                ReplayAction::Escalate {
                    key,
                    qc,
                    reason,
                    target,
                } => {
                    if let Some(entry) = self.quarantined_block_sync_qcs.get_mut(&key) {
                        entry.escalated_fetch = true;
                        entry.attempts = entry.attempts.saturating_add(1);
                        entry.first_seen = now;
                        entry.last_attempt = now;
                    }
                    self.force_block_sync_fetch_for_qc(&qc, reason);
                    debug!(?target, reason, "escalated quarantined block-sync QC fetch");
                    progress = true;
                }
                ReplayAction::Expire {
                    key,
                    qc,
                    reason,
                    target,
                } => {
                    self.quarantined_block_sync_qcs.remove(&key);
                    self.force_block_sync_fetch_for_qc(&qc, reason);
                    self.block_sync_qc_final_drop("expired");
                    warn!(?target, reason, "quarantined block-sync QC expired");
                    let current_view = self.phase_tracker.current_view(qc.height).unwrap_or(0);
                    let _ = self.handle_roster_unavailable_recovery(
                        qc.height,
                        current_view,
                        Some(qc.subject_block_hash),
                        self.queue.queued_len(),
                        now,
                        super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                        "quarantined_block_sync_qc_expired_empty_commit_topology",
                    );
                    progress = true;
                }
            }
        }

        progress
    }

    pub(super) fn enqueue_fetch_pending_block_response(&mut self, peer: PeerId, msg: BlockMessage) {
        let mut msg = BlockMessageWire::new(msg);
        if !self.prepare_background_block_message(&mut msg) {
            return;
        }
        let request = BackgroundRequest::Post { peer, msg };
        if self.config.debug.disable_background_worker {
            self.dispatch_background_inline(request);
            return;
        }
        let dispatched = {
            #[cfg(feature = "telemetry")]
            {
                background::dispatch_background_request(
                    self.background_post_tx.as_ref(),
                    request,
                    &self.telemetry,
                )
            }
            #[cfg(not(feature = "telemetry"))]
            {
                background::dispatch_background_request(self.background_post_tx.as_ref(), request)
            }
        };
        if let Err(request) = dispatched {
            self.dispatch_background_fallback(*request);
        }
    }

    fn dispatch_fetch_pending_block_response(
        &mut self,
        peer: PeerId,
        msg: BlockMessage,
        bypass_queue: bool,
    ) {
        if bypass_queue {
            let mut msg = BlockMessageWire::new(msg);
            if !self.prepare_background_block_message(&mut msg) {
                return;
            }
            #[cfg(test)]
            self.record_background_request(&BackgroundRequest::Post {
                peer: peer.clone(),
                msg: msg.clone(),
            });
            self.dispatch_background_fallback(BackgroundRequest::Post { peer, msg });
            return;
        }
        self.enqueue_fetch_pending_block_response(peer, msg);
    }

    fn block_body_response_from_payload(
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        body: BlockMessage,
    ) -> super::message::BlockBodyResponse {
        let body = match body {
            BlockMessage::BlockCreated(created) => {
                super::message::BlockBodyData::BlockCreated(created)
            }
            BlockMessage::BlockSyncUpdate(update) => {
                super::message::BlockBodyData::BlockSyncUpdate(update)
            }
            other => unreachable!(
                "exact body fetch payload builder returned unexpected variant: {}",
                Self::block_message_kind(&other)
            ),
        };
        super::message::BlockBodyResponse {
            block_hash,
            height,
            view,
            body,
        }
    }

    pub(super) fn block_body_response_for_wire(
        &self,
        block: &SignedBlock,
    ) -> super::message::BlockBodyResponse {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let body = self.build_fetch_pending_block_payload(block);
        Self::block_body_response_from_payload(block_hash, height, view, body)
    }

    fn send_block_body_response(&mut self, peer: PeerId, block: &SignedBlock) {
        let response = self.block_body_response_for_wire(block);
        self.dispatch_fetch_pending_block_response(
            peer,
            BlockMessage::BlockBodyResponse(response),
            /*bypass_queue*/ true,
        );
    }

    pub(super) fn flush_frontier_body_requesters(&mut self, block: &SignedBlock) {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let Some(slot) = self.frontier_slot.as_mut() else {
            return;
        };
        if slot.block_hash != block_hash || slot.height != height || slot.view != view {
            return;
        }
        slot.candidate.body_state = super::FrontierBodyState::Available;
        slot.body_present = true;
        slot.phase = super::FrontierSlotPhase::ValidateBody;
        slot.timers.last_progress_at = Instant::now();
        let requesters = std::mem::take(&mut slot.repair_state.pending_requesters);
        slot.pending_requesters.clear();
        slot.sync_compat_fields();
        for peer in requesters {
            self.send_block_body_response(peer, block);
        }
    }

    fn fetch_response_targets_highest_qc(&self, msg: &BlockMessage) -> bool {
        let Some(highest) = self.highest_qc else {
            return false;
        };
        let (block_hash, height, view) = match msg {
            BlockMessage::BlockSyncUpdate(update) => {
                let header = update.block.header();
                (
                    update.block.hash(),
                    header.height().get(),
                    header.view_change_index(),
                )
            }
            BlockMessage::BlockCreated(created) => {
                let header = created.block.header();
                (
                    created.block.hash(),
                    header.height().get(),
                    header.view_change_index(),
                )
            }
            BlockMessage::RbcInit(init) => (init.block_hash, init.height, init.view),
            BlockMessage::RbcChunk(chunk) => (chunk.block_hash, chunk.height, chunk.view),
            BlockMessage::RbcChunkCompact(chunk) => (
                chunk.block_hash,
                u64::from(chunk.height),
                u64::from(chunk.view),
            ),
            _ => return false,
        };
        highest.subject_block_hash == block_hash && highest.height == height && highest.view == view
    }

    fn fetch_response_should_bypass_queue(
        &self,
        msg: &BlockMessage,
        allow_highest_qc_bypass: bool,
    ) -> bool {
        (allow_highest_qc_bypass && self.fetch_response_targets_highest_qc(msg))
            || matches!(
                msg,
                // Missing-block recovery must deliver the block payload eagerly; otherwise
                // peers can receive RBC chunks first and stall waiting for BlockCreated.
                BlockMessage::BlockCreated(_)
                    | BlockMessage::RbcInit(_)
                    | BlockMessage::RbcChunk(_)
                    | BlockMessage::RbcChunkCompact(_)
                    | BlockMessage::RbcReady(_)
                    | BlockMessage::RbcDeliver(_)
            )
    }

    fn send_fetch_pending_block_response(
        &mut self,
        peer: PeerId,
        mut msg: BlockMessage,
        priority: FetchPendingBlockPriority,
        force_bypass_queue: bool,
        allow_highest_qc_bypass: bool,
        allow_hintless_block_sync_bypass: bool,
        requester_roster_proof_known: bool,
    ) {
        let mut hintless_block_sync = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(update)
                if update.commit_qc.is_none() && update.validator_checkpoint.is_none()
        );
        if hintless_block_sync
            && matches!(
                super::decide_hintless_block_sync_response_policy(
                    requester_roster_proof_known,
                    allow_hintless_block_sync_bypass,
                ),
                super::HintlessBlockSyncResponsePolicy::DowngradeToBlockCreated
            )
        {
            if let BlockMessage::BlockSyncUpdate(update) = &msg {
                let header = update.block.header();
                debug!(
                    height = header.height().get(),
                    view = header.view_change_index(),
                    block = %update.block.hash(),
                    peer = %peer,
                    "enforcing hintless BlockSyncUpdate send gate: downgrading to BlockCreated"
                );
                msg = BlockMessage::BlockCreated(super::message::BlockCreated {
                    block: update.block.clone(),
                    frontier: None,
                });
            }
            hintless_block_sync = false;
        }
        let bypass_queue = force_bypass_queue
            || matches!(priority, FetchPendingBlockPriority::Consensus)
            || self.fetch_response_should_bypass_queue(&msg, allow_highest_qc_bypass)
            || (allow_hintless_block_sync_bypass && hintless_block_sync);
        if let BlockMessage::BlockSyncUpdate(update) = &mut msg {
            let block_hash = update.block.hash();
            let height = update.block.header().height().get();
            let view = update.block.header().view_change_index();
            let expected_epoch = self.epoch_for_height(height);
            Self::apply_cached_qcs_to_block_sync_update(
                update,
                &self.qc_cache,
                &self.vote_log,
                block_hash,
                height,
                view,
                expected_epoch,
                self.state.as_ref(),
                self.config.consensus_mode,
            );
            if !self.trim_block_sync_update_for_frame_cap(update) {
                let fallback = BlockMessage::BlockCreated(super::message::BlockCreated {
                    block: update.block.clone(),
                    frontier: None,
                });
                let fallback_len =
                    super::consensus_block_wire_len(self.common_config.peer.id(), &fallback);
                if fallback_len > self.consensus_payload_frame_cap {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        cap = self.consensus_payload_frame_cap,
                        fallback_len,
                        "dropping oversized block sync response; BlockCreated still exceeds cap"
                    );
                    return;
                }
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    cap = self.consensus_payload_frame_cap,
                    fallback_len,
                    "block sync response exceeds frame cap; sending BlockCreated instead"
                );
                self.dispatch_fetch_pending_block_response(peer, fallback, bypass_queue);
                return;
            }
        }
        self.dispatch_fetch_pending_block_response(peer, msg, bypass_queue);
    }

    pub(super) fn build_fetch_pending_block_payload(&self, block: &SignedBlock) -> BlockMessage {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        let update = super::block_sync_update_with_roster(
            block,
            self.state.as_ref(),
            self.kura.as_ref(),
            self.config.consensus_mode,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
            &self.roster_validation_cache,
        );
        let mut update = update;
        let expected_epoch = self.epoch_for_height(block_height);
        Self::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &self.qc_cache,
            &self.vote_log,
            block_hash,
            block_height,
            block_view,
            expected_epoch,
            self.state.as_ref(),
            self.config.consensus_mode,
        );
        let (consensus_mode, _, _) = self.consensus_context_for_height(block_height);
        let has_roster = super::block_sync_update_has_roster(&update, consensus_mode);
        let has_cached_qc = update.commit_qc.is_some() || !update.commit_votes.is_empty();
        let send_block_sync = match consensus_mode {
            ConsensusMode::Permissioned => has_roster || has_cached_qc,
            // Missing-block recovery in NPoS must stay on the BlockSyncUpdate path even when
            // roster sidecars/hints are unavailable, otherwise responders fall back to
            // BlockCreated and receivers can livelock on lock-conflicting hintless payloads.
            ConsensusMode::Npos => true,
        };
        if !send_block_sync {
            BlockMessage::BlockCreated(self.frontier_block_created_for_wire(block))
        } else {
            BlockMessage::BlockSyncUpdate(update)
        }
    }

    fn block_sync_qc_extends_locked_chain(&self, qc: &crate::sumeragi::consensus::Qc) -> bool {
        let Some(lock) = self.locked_qc else {
            return true;
        };
        if !self.block_known_for_lock(lock.subject_block_hash) {
            return false;
        }
        let candidate = Self::qc_to_header_ref(qc);
        qc_extends_locked_with_lookup(lock, candidate, |hash, lookup_height| {
            self.parent_hash_for(hash, lookup_height)
        })
    }

    fn block_sync_qc_same_height_conflict(
        lock: crate::sumeragi::consensus::QcHeaderRef,
        qc: &crate::sumeragi::consensus::Qc,
    ) -> bool {
        qc.height == lock.height && qc.subject_block_hash != lock.subject_block_hash
    }

    fn block_sync_qc_same_height_recoverable(
        _lock: crate::sumeragi::consensus::QcHeaderRef,
        _qc: &crate::sumeragi::consensus::Qc,
        _allow_nonextending_qc: bool,
    ) -> bool {
        false
    }

    fn defer_block_sync_qc_while_locked_payload_missing(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        context: &'static str,
    ) -> bool {
        let Some(lock) = self.locked_qc else {
            return false;
        };
        if self.block_known_locally(lock.subject_block_hash) {
            return false;
        }
        if qc.height == lock.height && qc.subject_block_hash == lock.subject_block_hash {
            return false;
        }
        self.drop_missing_lock_if_unknown(qc);
        debug!(
            context,
            height = qc.height,
            view = qc.view,
            incoming_hash = %qc.subject_block_hash,
            locked_height = lock.height,
            locked_view = lock.view,
            locked_hash = %lock.subject_block_hash,
            "deferring block sync QC while locked payload remains unavailable"
        );
        self.quarantine_block_sync_qc_candidate(
            qc.clone(),
            "locked_payload_missing",
            QuarantinedQcTarget::LockedPayload,
        );
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::Qc,
            super::status::ConsensusMessageOutcome::Dropped,
            super::status::ConsensusMessageReason::LockedQc,
        );
        true
    }

    pub(super) fn block_sync_qc_is_stale_against_lock(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
    ) -> bool {
        self.locked_qc.is_some_and(|lock| qc.height < lock.height)
    }

    fn log_block_sync_locked_qc_conflict(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        lock: crate::sumeragi::consensus::QcHeaderRef,
        context: &'static str,
    ) {
        let warn_cooldown = self
            .rebroadcast_cooldown()
            .max(super::BLOCK_SYNC_WARN_COOLDOWN_FLOOR);
        let warn_now = Instant::now();
        if let Some(suppressed_since_last) = self.block_sync_warning_log.allow(
            super::BlockSyncWarningKind::LockedQcConflict,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            warn_now,
            warn_cooldown,
            super::BLOCK_SYNC_WARN_BURST_WINDOW,
            super::BLOCK_SYNC_WARN_BURST_CAP,
        ) {
            self.hotspot_log_summary.record_block_sync_warn();
            if suppressed_since_last > 0 {
                self.hotspot_log_summary
                    .record_block_sync_suppressed(suppressed_since_last);
            }
            info!(
                context,
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                incoming_hash = %qc.subject_block_hash,
                locked_height = lock.height,
                locked_view = lock.view,
                locked_hash = %lock.subject_block_hash,
                suppressed_since_last,
                warn_cooldown_ms = warn_cooldown.as_millis(),
                "dropping block sync QC that conflicts with locked chain"
            );
        } else {
            self.hotspot_log_summary.record_block_sync_suppressed(1);
        }
        self.hotspot_log_summary.emit_if_due(warn_now);
    }

    fn take_pending_fetch_requesters(
        &mut self,
        block_hash: &HashOf<BlockHeader>,
    ) -> BTreeMap<PeerId, super::PendingFetchRequestMeta> {
        self.pending
            .pending_fetch_requests
            .remove(block_hash)
            .unwrap_or_default()
    }

    fn take_pending_block_body_requesters(
        &mut self,
        block_hash: &HashOf<BlockHeader>,
    ) -> BTreeSet<PeerId> {
        self.pending
            .pending_block_body_requests
            .remove(block_hash)
            .unwrap_or_default()
    }

    fn stash_pending_fetch_request(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        peer: PeerId,
        priority: FetchPendingBlockPriority,
        requester_roster_proof_known: bool,
    ) {
        let meta = super::PendingFetchRequestMeta {
            priority,
            requester_roster_proof_known,
        };
        let entry = self
            .pending
            .pending_fetch_requests
            .entry(block_hash)
            .or_default();
        entry
            .entry(peer)
            .and_modify(|stored| {
                stored.priority = stored.priority.max(meta.priority);
                stored.requester_roster_proof_known |= meta.requester_roster_proof_known;
            })
            .or_insert(meta);
    }

    fn stash_pending_block_body_request(&mut self, block_hash: HashOf<BlockHeader>, peer: PeerId) {
        self.pending
            .pending_block_body_requests
            .entry(block_hash)
            .or_default()
            .insert(peer);
    }

    fn send_fetch_pending_block_responses(
        &mut self,
        peers: BTreeMap<PeerId, super::PendingFetchRequestMeta>,
        block: &SignedBlock,
        force_bypass_queue: bool,
        allow_highest_qc_bypass: bool,
        allow_hintless_block_sync_bypass: bool,
    ) {
        if peers.is_empty() {
            return;
        }
        let msg = self.build_fetch_pending_block_payload(block);
        let hintless_block_sync = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(update)
                if update.commit_qc.is_none() && update.validator_checkpoint.is_none()
        );

        if hintless_block_sync {
            let created = BlockMessage::BlockCreated(self.frontier_block_created_for_wire(block));
            let header = block.header();
            for (peer, meta) in peers {
                let hintless_policy = super::decide_hintless_block_sync_response_policy(
                    meta.requester_roster_proof_known,
                    allow_hintless_block_sync_bypass,
                );
                let allow_hintless_for_peer = matches!(
                    hintless_policy,
                    super::HintlessBlockSyncResponsePolicy::AllowHintlessBlockSync
                );
                let peer_msg = match hintless_policy {
                    super::HintlessBlockSyncResponsePolicy::AllowHintlessBlockSync => msg.clone(),
                    super::HintlessBlockSyncResponsePolicy::DowngradeToBlockCreated => {
                        debug!(
                            height = header.height().get(),
                            view = header.view_change_index(),
                            block = %block.hash(),
                            peer = %peer,
                            requester_priority = ?meta.priority,
                            requester_roster_proof_known = meta.requester_roster_proof_known,
                            "downgrading hintless BlockSyncUpdate to BlockCreated: requester roster proof not confirmed"
                        );
                        created.clone()
                    }
                };
                let bypass_rosterless_created =
                    allow_hintless_for_peer && matches!(peer_msg, BlockMessage::BlockCreated(_));
                self.send_fetch_pending_block_response(
                    peer.clone(),
                    peer_msg,
                    meta.priority,
                    force_bypass_queue || bypass_rosterless_created,
                    allow_highest_qc_bypass,
                    allow_hintless_for_peer,
                    meta.requester_roster_proof_known,
                );
            }
            return;
        }

        let bypass_rosterless_created =
            allow_hintless_block_sync_bypass && matches!(msg, BlockMessage::BlockCreated(_));
        if matches!(msg, BlockMessage::BlockSyncUpdate(_)) {
            let created = BlockMessage::BlockCreated(self.frontier_block_created_for_wire(block));
            let created_len =
                super::consensus_block_wire_len(self.common_config.peer.id(), &created);
            // For roster-hinted updates, include a companion BlockCreated copy so peers can
            // recover payload bytes even if they defer BlockSyncUpdate processing.
            let send_created_copy = true;
            if send_created_copy && created_len <= self.consensus_payload_frame_cap {
                for (peer, meta) in peers.iter() {
                    self.send_fetch_pending_block_response(
                        peer.clone(),
                        created.clone(),
                        meta.priority,
                        force_bypass_queue || bypass_rosterless_created,
                        allow_highest_qc_bypass,
                        allow_hintless_block_sync_bypass,
                        meta.requester_roster_proof_known,
                    );
                }
            } else if send_created_copy {
                let header = block.header();
                warn!(
                    height = header.height().get(),
                    view = header.view_change_index(),
                    block = %block.hash(),
                    cap = self.consensus_payload_frame_cap,
                    created_len,
                    "skipping BlockCreated fetch response; payload exceeds frame cap"
                );
            }
        }
        for (peer, meta) in peers {
            self.send_fetch_pending_block_response(
                peer.clone(),
                msg.clone(),
                meta.priority,
                force_bypass_queue || bypass_rosterless_created,
                allow_highest_qc_bypass,
                allow_hintless_block_sync_bypass,
                meta.requester_roster_proof_known,
            );
        }
    }

    pub(super) fn flush_pending_fetch_requests(&mut self, block: &SignedBlock) {
        let block_hash = block.hash();
        let requesters = self.take_pending_fetch_requesters(&block_hash);
        self.send_fetch_pending_block_responses(
            requesters, block, /*force_bypass_queue*/ false,
            /*allow_highest_qc_bypass*/ false,
            /*allow_hintless_block_sync_bypass*/ false,
        );
    }

    pub(super) fn flush_pending_fetch_requests_if_ready(&mut self, block: &SignedBlock) -> bool {
        let block_hash = block.hash();
        if !self
            .pending
            .pending_fetch_requests
            .contains_key(&block_hash)
        {
            return false;
        }
        let msg = self.build_fetch_pending_block_payload(block);
        if self.should_defer_canonical_committed_fetch_response(block, &msg) {
            return false;
        }
        self.flush_pending_fetch_requests(block);
        true
    }

    pub(super) fn flush_pending_block_body_requests_if_ready(
        &mut self,
        block: &SignedBlock,
    ) -> bool {
        let block_hash = block.hash();
        if !self
            .pending
            .pending_block_body_requests
            .contains_key(&block_hash)
        {
            return false;
        }
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let payload = self.build_fetch_pending_block_payload(block);
        if self.should_defer_canonical_committed_fetch_response(block, &payload) {
            return false;
        }
        let response = Self::block_body_response_from_payload(block_hash, height, view, payload);
        let requesters = self.take_pending_block_body_requesters(&block_hash);
        for peer in requesters {
            self.dispatch_fetch_pending_block_response(
                peer,
                BlockMessage::BlockBodyResponse(response.clone()),
                /*bypass_queue*/ true,
            );
        }
        true
    }

    pub(super) fn should_drop_future_block_sync_update(
        &self,
        block_hash: &HashOf<BlockHeader>,
        parent_hash: Option<HashOf<BlockHeader>>,
        height: u64,
        view: u64,
        requested_missing_block: bool,
    ) -> bool {
        if self.block_known_locally(*block_hash) {
            return false;
        }
        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let requested_margin = self.recovery_missing_request_stale_height_margin().max(1);
        let far_ahead_by_committed = height > local_height.saturating_add(requested_margin);
        if requested_missing_block {
            // Requested missing-block recovery is only allowed within a bounded forward window.
            // Apply this gate before parent-availability short-circuit so far-ahead chains cannot
            // keep inflating pending payload/RBC state while the tracked missing height is stalled.
            if far_ahead_by_committed {
                return true;
            }
            return false;
        }
        let unresolved_lower_missing = self
            .lowest_unresolved_missing_block_height(local_height)
            .is_some_and(|missing_height| missing_height < height);
        if unresolved_lower_missing && far_ahead_by_committed {
            // A lower missing height is still unresolved; keep recovery deterministic by
            // suppressing sparse far-ahead updates until the gap closes.
            return true;
        }
        if parent_hash.is_some_and(|hash| self.block_payload_available_locally(hash)) {
            return false;
        }
        self.should_drop_future_consensus_message(height, view, "BlockSyncUpdate")
    }

    fn block_sync_update_deferral_reason(&self) -> Option<&'static str> {
        if self.subsystems.commit.inflight.is_some() {
            return Some("commit_inflight");
        }
        if !self.subsystems.validation.inflight.is_empty() {
            return Some("validation_inflight");
        }
        if self.pending.pending_processing.get().is_some() {
            return Some("pending_processing");
        }
        None
    }

    fn merge_deferred_block_sync_update(
        existing: &mut super::DeferredBlockSyncUpdate,
        mut incoming: super::DeferredBlockSyncUpdate,
    ) {
        if existing.update.commit_qc.is_none() {
            existing.update.commit_qc = incoming.update.commit_qc.take();
        }
        if existing.update.validator_checkpoint.is_none() {
            existing.update.validator_checkpoint = incoming.update.validator_checkpoint.take();
        }
        if existing.update.stake_snapshot.is_none() {
            existing.update.stake_snapshot = incoming.update.stake_snapshot.take();
        }
        if incoming.sender.is_some() {
            existing.sender = incoming.sender;
        }
    }

    fn deferred_block_sync_has_commit_evidence(entry: &super::DeferredBlockSyncUpdate) -> bool {
        entry.update.commit_qc.is_some()
            || entry.update.validator_checkpoint.is_some()
            || entry.update.stake_snapshot.is_some()
    }

    fn enforce_deferred_block_sync_cap(&mut self) {
        let cap = self.recovery_pending_block_sync_cap();
        if cap == 0 {
            return;
        }
        let mut evictions = 0u64;
        while self.deferred_block_sync_updates.len() > cap {
            let candidate = self
                .deferred_block_sync_updates
                .iter()
                .min_by_key(|(key, entry)| {
                    let (height, view, hash) = *key;
                    (
                        // Prefer retaining entries carrying commit evidence.
                        u8::from(Self::deferred_block_sync_has_commit_evidence(entry)),
                        // Prefer retaining newer views/heights.
                        view,
                        height,
                        hash,
                    )
                })
                .map(|(key, _)| *key);
            let Some((height, view, hash)) = candidate else {
                break;
            };
            if self
                .deferred_block_sync_updates
                .remove(&(height, view, hash))
                .is_some()
            {
                evictions = evictions.saturating_add(1);
                debug!(
                    height,
                    view,
                    block = %hash,
                    deferred = self.deferred_block_sync_updates.len(),
                    cap,
                    "evicting deferred block sync update due to bounded queue cap"
                );
            }
        }
        if evictions > 0 {
            super::status::inc_pending_queue_evictions_total(evictions);
        }
    }

    pub(super) fn cache_deferred_block_sync_update(
        &mut self,
        mut update: super::message::BlockSyncUpdate,
        sender: Option<PeerId>,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        reason: &'static str,
    ) {
        update.commit_votes.clear();
        let entry = super::DeferredBlockSyncUpdate { update, sender };
        let key = (block_height, block_view, block_hash);
        if let Some(existing) = self.deferred_block_sync_updates.get_mut(&key) {
            Self::merge_deferred_block_sync_update(existing, entry);
        } else {
            self.deferred_block_sync_updates.insert(key, entry);
        }
        self.enforce_deferred_block_sync_cap();
        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            deferred = self.deferred_block_sync_updates.len(),
            reason,
            "cached deferred block sync payload for later replay"
        );
    }

    pub(super) fn defer_block_sync_update(
        &mut self,
        update: super::message::BlockSyncUpdate,
        sender: Option<PeerId>,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        reason: &'static str,
    ) {
        self.cache_deferred_block_sync_update(
            update,
            sender,
            block_hash,
            block_height,
            block_view,
            reason,
        );
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::BlockSyncUpdate,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::CommitPipelineActive,
        );
        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            deferred = self.deferred_block_sync_updates.len(),
            reason,
            commit_inflight = self.subsystems.commit.inflight.is_some(),
            validation_inflight = self.subsystems.validation.inflight.len(),
            "deferring block sync update while commit/validation work is in flight"
        );
    }

    /// Replay deferred block-sync updates once commit/validation work is idle.
    pub(super) fn try_replay_deferred_block_sync_updates(&mut self) -> bool {
        if self.deferred_block_sync_updates.is_empty() {
            return false;
        }
        if self.subsystems.commit.inflight.is_some()
            || !self.subsystems.validation.inflight.is_empty()
        {
            return false;
        }
        let Some(key) = self.deferred_block_sync_updates.keys().next().cloned() else {
            return false;
        };
        let (height, view, block_hash) = key;
        let Some(entry) = self.deferred_block_sync_updates.remove(&key) else {
            return false;
        };
        debug!(
            height,
            view,
            block = %block_hash,
            deferred = self.deferred_block_sync_updates.len(),
            "replaying deferred block sync update"
        );
        if let Err(err) = self.handle_block_sync_update(entry.update, entry.sender) {
            warn!(
                ?err,
                height,
                view,
                block = %block_hash,
                "failed to replay deferred block sync update"
            );
        }
        true
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub(super) fn handle_block_sync_update(
        &mut self,
        update: super::message::BlockSyncUpdate,
        sender: Option<PeerId>,
    ) -> Result<()> {
        if crate::sumeragi::status::local_peer_removed() {
            debug!(
                ?sender,
                "dropping BlockSyncUpdate because local peer removed from world"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ModeMismatch,
            );
            return Ok(());
        }
        let super::message::BlockSyncUpdate {
            block,
            commit_votes,
            commit_qc: incoming_qc,
            validator_checkpoint,
            stake_snapshot,
        } = update;
        let mut incoming_qc = incoming_qc;
        let mut validator_checkpoint = validator_checkpoint;
        let mut stake_snapshot = stake_snapshot;
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        let parent_hash = block.header().prev_block_hash();
        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let expected_epoch = self.epoch_for_height(block_height);
        let requested_missing_block_by_hash = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        let requested_missing_block_by_height = !requested_missing_block_by_hash
            && self.has_missing_block_request_for_height(block_height);
        let explicit_requested_missing_block =
            requested_missing_block_by_hash || requested_missing_block_by_height;
        let mut requested_missing_block = explicit_requested_missing_block;
        if requested_missing_block_by_height {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                "treating block sync update as missing-block recovery traffic due to same-height request"
            );
        }
        let block_known_locally = self.block_known_locally(block_hash);
        let has_commit_votes = !commit_votes.is_empty();
        let has_commit_evidence =
            incoming_qc.is_some() || validator_checkpoint.is_some() || has_commit_votes;
        self.prune_frontier_slot_state();
        let entry_deferral_reason = self.block_sync_update_deferral_reason();
        let exact_frontier_body_repair = self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.block_hash == block_hash && slot.height == block_height && slot.view == block_view
        });
        let frontier_lane_owned = (local_height.saturating_add(1)..=local_height.saturating_add(2))
            .contains(&block_height);
        let frontier_lane_locked = self
            .frontier_slot
            .as_ref()
            .is_some_and(|slot| slot.height == local_height.saturating_add(1))
            || self
                .next_slot_prefetch
                .as_ref()
                .is_some_and(|slot| slot.height == local_height.saturating_add(2));
        let frontier_lane_deep_catchup = block_height > local_height.saturating_add(1)
            && frontier_lane_owned
            && has_commit_evidence
            && (requested_missing_block || block_known_locally);
        let block_known_sidecar_fast_path =
            block_known_locally && (incoming_qc.is_some() || validator_checkpoint.is_some());
        let frontier_lane_fast_path = frontier_lane_owned
            && !frontier_lane_deep_catchup
            && (block_known_sidecar_fast_path
                || (entry_deferral_reason.is_none()
                    && (block_known_locally
                        || (explicit_requested_missing_block
                            && (incoming_qc.is_some() || validator_checkpoint.is_some()))
                        || (exact_frontier_body_repair
                            && (incoming_qc.is_some() || validator_checkpoint.is_some())))));
        if frontier_lane_fast_path {
            let mut processed_votes = 0usize;
            let mut dropped_votes = 0usize;
            for vote in commit_votes {
                if vote.phase != crate::sumeragi::consensus::Phase::Commit
                    || vote.block_hash != block_hash
                    || vote.height != block_height
                    || vote.view != block_view
                    || vote.epoch != expected_epoch
                {
                    dropped_votes = dropped_votes.saturating_add(1);
                    continue;
                }
                self.handle_vote(vote);
                processed_votes = processed_votes.saturating_add(1);
            }
            if dropped_votes > 0 {
                debug!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    dropped_votes,
                    "dropping mismatched commit votes from contiguous frontier block sync update"
                );
            }
            if block_known_locally {
                debug!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                    processed_votes,
                    has_commit_qc = incoming_qc.is_some(),
                    has_checkpoint = validator_checkpoint.is_some(),
                    "ignoring frontier-lane BlockSyncUpdate sidecars for a locally known block"
                );
                let mut sidecar_qc = incoming_qc.take().or_else(|| {
                    validator_checkpoint.as_ref().and_then(|checkpoint| {
                        self.commit_qc_from_validator_checkpoint(
                            block_hash,
                            block_height,
                            block_view,
                            checkpoint,
                        )
                    })
                });
                if sidecar_qc.is_some() {
                    let qc = sidecar_qc
                        .take()
                        .expect("sidecar QC presence checked above");
                    let world_view = self.state.world_view();
                    let consensus_mode = super::effective_consensus_mode_for_height_from_world(
                        &world_view,
                        block_height,
                        self.config.consensus_mode,
                    );
                    let mode_tag = match consensus_mode {
                        ConsensusMode::Permissioned => PERMISSIONED_TAG,
                        ConsensusMode::Npos => NPOS_TAG,
                    };
                    let prf_seed = Some(super::prf_seed_for_height_from_world(
                        &world_view,
                        &self.common_config.chain,
                        block_height,
                    ));
                    drop(world_view);
                    let checkpoint = validator_checkpoint.clone().unwrap_or_else(|| {
                        ValidatorSetCheckpoint::new(
                            qc.height,
                            qc.view,
                            qc.subject_block_hash,
                            qc.parent_state_root,
                            qc.post_state_root,
                            qc.validator_set.clone(),
                            qc.aggregate.signers_bitmap.clone(),
                            qc.aggregate.bls_aggregate_signature.clone(),
                            qc.validator_set_hash_version,
                            None,
                        )
                    });
                    self.state
                        .record_commit_roster(&qc, &checkpoint, stake_snapshot.clone());
                    let topology = super::network_topology::Topology::new(qc.validator_set.clone());
                    if let Some(work) = self.prepare_known_block_qc_work(
                        qc,
                        Arc::new(block),
                        topology,
                        stake_snapshot.clone(),
                        consensus_mode,
                        mode_tag,
                        prf_seed,
                        true,
                    ) {
                        let buffered_local_block =
                            self.pending
                                .pending_blocks
                                .get(&block_hash)
                                .is_some_and(|pending| !pending.is_retry_aborted())
                                || self.subsystems.commit.inflight.as_ref().is_some_and(
                                    |inflight| {
                                        inflight.block_hash == block_hash
                                            && !inflight.pending.aborted
                                    },
                                )
                                || self
                                    .pending
                                    .pending_processing
                                    .get()
                                    .is_some_and(|pending| pending == block_hash);
                        if buffered_local_block {
                            let _ = self.apply_known_block_qc_work(work);
                        } else {
                            self.enqueue_known_block_qc_work(work);
                        }
                    }
                    let _ = self.try_replay_deferred_qcs();
                    let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
                    self.request_commit_pipeline_for_pending(
                        block_hash,
                        super::status::RoundEventCauseTrace::BlockSyncUpdated,
                        None,
                    );
                }
                return Ok(());
            }
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                processed_votes,
                has_commit_qc = incoming_qc.is_some(),
                has_checkpoint = validator_checkpoint.is_some(),
                "routing frontier-lane BlockSyncUpdate through BlockCreated owner"
            );
            let recovery_block = block.clone();
            let result = self.handle_block_created_from_block_sync(
                super::message::BlockCreated {
                    block,
                    frontier: None,
                },
                sender.clone(),
                incoming_qc.is_none() && validator_checkpoint.is_none(),
                incoming_qc.is_some() || validator_checkpoint.is_some(),
                has_commit_votes || incoming_qc.is_some() || validator_checkpoint.is_some(),
            );
            let payload_materialized = result.is_ok()
                && self.materialize_frontier_block_sync_payload_for_qc_recovery(
                    &recovery_block,
                    incoming_qc.as_ref().map(|qc| qc.epoch),
                );
            let mut local_block_for_qc = result
                .is_ok()
                .then(|| self.local_signed_block_for_hash(block_hash))
                .flatten();
            if payload_materialized || local_block_for_qc.is_some() {
                let _ = self.try_replay_deferred_qcs();
                let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
                self.request_commit_pipeline_for_pending(
                    block_hash,
                    super::status::RoundEventCauseTrace::BlockSyncUpdated,
                    None,
                );
                if local_block_for_qc.is_none() {
                    local_block_for_qc = self.local_signed_block_for_hash(block_hash);
                }
            }
            if result.is_ok()
                && let Some(local_block) = local_block_for_qc
                && let Some(qc) = incoming_qc.take().or_else(|| {
                    validator_checkpoint.as_ref().and_then(|checkpoint| {
                        self.commit_qc_from_validator_checkpoint(
                            block_hash,
                            block_height,
                            block_view,
                            checkpoint,
                        )
                    })
                })
            {
                let world_view = self.state.world_view();
                let consensus_mode = super::effective_consensus_mode_for_height_from_world(
                    &world_view,
                    block_height,
                    self.config.consensus_mode,
                );
                let mode_tag = match consensus_mode {
                    ConsensusMode::Permissioned => PERMISSIONED_TAG,
                    ConsensusMode::Npos => NPOS_TAG,
                };
                let prf_seed = Some(super::prf_seed_for_height_from_world(
                    &world_view,
                    &self.common_config.chain,
                    block_height,
                ));
                drop(world_view);
                let checkpoint = validator_checkpoint.clone().unwrap_or_else(|| {
                    ValidatorSetCheckpoint::new(
                        qc.height,
                        qc.view,
                        qc.subject_block_hash,
                        qc.parent_state_root,
                        qc.post_state_root,
                        qc.validator_set.clone(),
                        qc.aggregate.signers_bitmap.clone(),
                        qc.aggregate.bls_aggregate_signature.clone(),
                        qc.validator_set_hash_version,
                        None,
                    )
                });
                self.state
                    .record_commit_roster(&qc, &checkpoint, stake_snapshot.clone());
                let topology = super::network_topology::Topology::new(qc.validator_set.clone());
                if let Some(work) = self.prepare_known_block_qc_work(
                    qc,
                    local_block,
                    topology,
                    stake_snapshot.clone(),
                    consensus_mode,
                    mode_tag,
                    prf_seed,
                    true,
                ) {
                    self.enqueue_known_block_qc_work(work);
                }
            }
            return result;
        }
        if frontier_lane_deep_catchup {
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                block_known_locally,
                has_commit_qc = incoming_qc.is_some(),
                has_checkpoint = validator_checkpoint.is_some(),
                has_commit_votes,
                "processing contiguous frontier BlockSyncUpdate as deep catch-up"
            );
        }
        if !block_known_locally
            && !requested_missing_block
            && frontier_lane_locked
            && block_height > local_height.saturating_add(2)
        {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                local_height,
                frontier_slot_height = self.frontier_slot.as_ref().map(|slot| slot.height),
                next_slot_prefetch_height =
                    self.next_slot_prefetch.as_ref().map(|slot| slot.height),
                "dropping block sync update beyond the active frontier lanes"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::FutureWindow,
            );
            return Ok(());
        }
        let has_commit_votes = !commit_votes.is_empty();
        let has_commit_evidence =
            incoming_qc.is_some() || validator_checkpoint.is_some() || has_commit_votes;
        if self.should_drop_future_block_sync_update(
            &block_hash,
            parent_hash,
            block_height,
            block_view,
            requested_missing_block,
        ) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::FutureWindow,
            );
            if let Some(parent_hash) = parent_hash {
                let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                let expected_height = local_height.saturating_add(1);
                let commit_topology = self.effective_commit_topology();
                let expected_usize = usize::try_from(expected_height).ok();
                let actual_usize = usize::try_from(block_height).ok();
                self.request_missing_parent(
                    block_hash,
                    block_height,
                    block_view,
                    parent_hash,
                    &commit_topology,
                    None,
                    expected_usize,
                    actual_usize,
                    "block_sync_future_window",
                );
                if block_height > expected_height.saturating_add(1) {
                    self.request_missing_parents_for_gap(
                        &commit_topology,
                        None,
                        "block_sync_future_gap",
                    );
                }
            }
            return Ok(());
        }
        if let Some(local_view) = self.stale_view(block_height, block_view) {
            if !requested_missing_block && !block_known_locally && !has_commit_evidence {
                debug!(
                    height = block_height,
                    view = block_view,
                    local_view,
                    kind = "BlockSyncUpdate",
                    "dropping consensus message for stale view without missing request"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockSyncUpdate,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StaleView,
                );
                return Ok(());
            }
            let da_enabled = self.runtime_da_enabled();
            debug!(
                height = block_height,
                view = block_view,
                local_view,
                da_enabled,
                block_known_locally,
                has_commit_evidence,
                missing_request = requested_missing_block,
                "accepting BlockSyncUpdate for stale view"
            );
        }
        if let Some(qc) = incoming_qc.as_ref() {
            if let Some(lock) = self.locked_qc {
                let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, qc);
                if same_height_conflict {
                    if self.defer_block_sync_qc_while_locked_payload_missing(
                        qc,
                        "block_sync_update.prefilter.missing_locked_payload",
                    ) {
                        return Ok(());
                    }
                    self.log_block_sync_locked_qc_conflict(
                        qc,
                        lock,
                        "block_sync_update.prefilter.height_conflict",
                    );
                    crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
                    );
                    incoming_qc = None;
                }
            }
        }
        let (consensus_mode, mode_tag, prf_seed, local_height) = {
            let world_view = self.state.world_view();
            let consensus_mode = super::effective_consensus_mode_for_height_from_world(
                &world_view,
                block_height,
                self.config.consensus_mode,
            );
            let mode_tag = match consensus_mode {
                ConsensusMode::Permissioned => PERMISSIONED_TAG,
                ConsensusMode::Npos => NPOS_TAG,
            };
            let prf_seed = Some(super::prf_seed_for_height_from_world(
                &world_view,
                &self.common_config.chain,
                block_height,
            ));
            let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
            (consensus_mode, mode_tag, prf_seed, local_height)
        };
        let kura_committed_start = Instant::now();
        if let Ok(height_usize) = usize::try_from(block_height)
            && let Some(nz_height) = NonZeroUsize::new(height_usize)
        {
            if let Some(committed) = self.kura.get_block(nz_height) {
                let committed_hash = committed.hash();
                if committed_hash != block_hash {
                    let Some(commit_qc) = incoming_qc.take() else {
                        info!(
                            committed_height = height_usize,
                            committed_hash = %committed_hash,
                            incoming_hash = %block_hash,
                            "dropping block sync update that conflicts with committed block without commit QC"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::CommitConflict,
                        );
                        self.clear_missing_block_request(
                            &block_hash,
                            MissingBlockClearReason::Obsolete,
                        );
                        return Ok(());
                    };
                    let inputs = self.roster_validation_cache.inputs_for_roster(
                        &commit_qc.validator_set,
                        consensus_mode,
                        stake_snapshot.as_ref(),
                    );
                    let allow_genesis_stub = block_height == 1 && block_view == 0;
                    if let Err(err) = super::validate_commit_qc_roster_cached(
                        &self.roster_validation_cache,
                        &commit_qc,
                        block_hash,
                        block_height,
                        Some(block_view),
                        consensus_mode,
                        expected_epoch,
                        &self.common_config.chain,
                        mode_tag,
                        allow_genesis_stub,
                        &inputs,
                    ) {
                        warn!(
                            ?err,
                            committed_height = height_usize,
                            committed_hash = %committed_hash,
                            incoming_hash = %block_hash,
                            "dropping commit-conflict block sync update with invalid commit QC"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::CommitConflict,
                        );
                        self.clear_missing_block_request(
                            &block_hash,
                            MissingBlockClearReason::Obsolete,
                        );
                        return Ok(());
                    }
                    info!(
                        committed_height = height_usize,
                        committed_hash = %committed_hash,
                        incoming_hash = %block_hash,
                        view = block_view,
                        "rejecting conflicting commit QC at committed height; enforcing finality"
                    );
                    #[cfg(feature = "telemetry")]
                    {
                        self.telemetry.inc_commit_conflict_detected();
                    }
                    let evidence = crate::sumeragi::consensus::Evidence {
                        kind: crate::sumeragi::consensus::EvidenceKind::InvalidQc,
                        payload: crate::sumeragi::consensus::EvidencePayload::InvalidQc {
                            certificate: commit_qc,
                            reason: "commit_conflict_finality".to_owned(),
                        },
                    };
                    if let Err(err) = self.record_and_broadcast_evidence(evidence) {
                        warn!(
                            ?err,
                            committed_height = height_usize,
                            committed_hash = %committed_hash,
                            incoming_hash = %block_hash,
                            "failed to record commit-conflict evidence"
                        );
                    }
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockSyncUpdate,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::CommitConflict,
                    );
                    self.clear_missing_block_request(
                        &block_hash,
                        MissingBlockClearReason::Obsolete,
                    );
                    return Ok(());
                }
            }
        }
        let kura_committed_ms =
            u64::try_from(kura_committed_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let kura_known_start = Instant::now();
        let block_known = self.kura.get_block_height_by_hash(block_hash).is_some();
        let kura_known_ms =
            u64::try_from(kura_known_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let has_roster_hint = incoming_qc.is_some()
            || validator_checkpoint.is_some()
            || stake_snapshot.is_some()
            || !commit_votes.is_empty();
        if block_known && !has_roster_hint {
            info!(
                hash = ?block_hash,
                height = block_height,
                "skipping block sync update for already known block"
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        if self.runtime_da_enabled()
            && !requested_missing_block
            && !block_known_locally
            && block_height <= local_height.saturating_add(1)
        {
            // Aborted pending payloads are retained for recovery but must still be treated as
            // missing for consensus progression, otherwise sparse next-height block-sync updates
            // can be dropped before they revive the pending entry.
            requested_missing_block = true;
        }
        let mut commit_votes = Some(commit_votes);
        let mut process_commit_votes = |actor: &mut Actor| {
            let Some(commit_votes) = commit_votes.take() else {
                return (0usize, 0usize);
            };
            let mut processed_votes = 0usize;
            let mut dropped_votes = 0usize;
            for vote in commit_votes {
                if vote.phase != crate::sumeragi::consensus::Phase::Commit
                    || vote.block_hash != block_hash
                    || vote.height != block_height
                    || vote.view != block_view
                    || vote.epoch != expected_epoch
                {
                    dropped_votes = dropped_votes.saturating_add(1);
                    continue;
                }
                actor.handle_vote(vote);
                processed_votes = processed_votes.saturating_add(1);
            }
            if dropped_votes > 0 {
                debug!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    dropped_votes,
                    "dropping mismatched commit votes from block sync update"
                );
            }
            (processed_votes, dropped_votes)
        };
        let commit_votes_start = Instant::now();
        let (commit_votes_processed, commit_votes_dropped) = process_commit_votes(self);
        let commit_votes_pre_ms =
            u64::try_from(commit_votes_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        // Commit votes can arm missing-block recovery before we reach the roster/payload path.
        // Keep the local gate in sync with the tracked request state so vote-backed stale-view
        // recovery does not get dropped as if no recovery request existed.
        requested_missing_block |= self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        let vote_only_known_block_fast_path = block_known
            && has_commit_votes
            && incoming_qc.is_none()
            && validator_checkpoint.is_none()
            && stake_snapshot.is_none();
        if vote_only_known_block_fast_path {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                commit_votes_pre_ms,
                commit_votes_processed,
                commit_votes_dropped,
                "processed known-block vote-only block sync update via fast-path"
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        if let Some(reason) = entry_deferral_reason {
            self.defer_block_sync_update(
                super::message::BlockSyncUpdate {
                    block,
                    commit_votes: Vec::new(),
                    commit_qc: incoming_qc,
                    validator_checkpoint,
                    stake_snapshot,
                },
                sender,
                block_hash,
                block_height,
                block_view,
                reason,
            );
            return Ok(());
        }
        let cached_frontier_qc = cached_qc_for(
            &self.qc_cache,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            block_height,
            block_view,
            expected_epoch,
        )
        .is_some();
        let parent_known_locally = parent_hash.is_some_and(|hash| self.block_known_locally(hash));
        let frontier_lane_payload_only = frontier_lane_owned
            && !frontier_lane_deep_catchup
            && !block_known
            && incoming_qc.is_none()
            && validator_checkpoint.is_none()
            && !has_commit_votes
            && !cached_frontier_qc
            && (requested_missing_block
                || parent_known_locally
                || (matches!(consensus_mode, ConsensusMode::Npos)
                    && block_height == local_height.saturating_add(1)));
        if frontier_lane_payload_only {
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                processed_votes = commit_votes_processed,
                "routing payload-only frontier-lane BlockSyncUpdate through BlockCreated owner"
            );
            let creation_result = self.handle_block_created_from_block_sync(
                super::message::BlockCreated {
                    block: block.clone(),
                    frontier: None,
                },
                sender.clone(),
                true,
                false,
                requested_missing_block,
            );
            let payload_materialized = creation_result.is_ok()
                && self.materialize_frontier_block_sync_payload_for_qc_recovery(&block, None);
            if creation_result.is_ok() {
                let mut roster = self.roster_for_vote_with_mode(
                    block_hash,
                    block_height,
                    block_view,
                    consensus_mode,
                );
                if roster.is_empty() {
                    roster = self.effective_commit_topology();
                }
                if roster.is_empty() {
                    roster = self.trusted_topology();
                }
                self.cache_vote_roster(block_hash, block_height, block_view, roster);
            }
            if payload_materialized || self.block_known_locally(block_hash) {
                let _ = self.try_replay_deferred_qcs();
                let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
                self.request_commit_pipeline_for_pending(
                    block_hash,
                    super::status::RoundEventCauseTrace::BlockSyncUpdated,
                    None,
                );
            }
            return creation_result;
        }
        // For known blocks, prefer the locally recorded commit roster snapshot and ignore
        // mismatching hints to avoid re-validating rosters on the main loop.
        let snapshot = block_known
            .then(|| {
                self.state
                    .commit_roster_snapshot_for_block(block_height, block_hash)
            })
            .flatten();
        if let Some(snapshot) = snapshot.as_ref() {
            if let Some(qc) = incoming_qc.as_ref()
                && HashOf::new(qc) != HashOf::new(&snapshot.commit_qc)
            {
                let same_validator_set = qc.validator_set_hash_version
                    == snapshot.commit_qc.validator_set_hash_version
                    && qc.validator_set_hash == snapshot.commit_qc.validator_set_hash
                    && qc.validator_set == snapshot.commit_qc.validator_set;
                if same_validator_set {
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        incoming_signers = qc_signer_count(qc),
                        local_signers = qc_signer_count(&snapshot.commit_qc),
                        "incoming block sync QC differs from local snapshot; revalidating"
                    );
                } else {
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "dropping block sync QC: does not match local commit roster snapshot"
                    );
                    incoming_qc = None;
                }
            }
            if let Some(checkpoint) = validator_checkpoint.as_ref()
                && HashOf::new(checkpoint) != HashOf::new(&snapshot.validator_checkpoint)
            {
                info!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync validator checkpoint: does not match local snapshot"
                );
                validator_checkpoint = None;
            }
            if let Some(stake) = stake_snapshot.as_ref()
                && snapshot
                    .stake_snapshot
                    .as_ref()
                    .is_none_or(|local| HashOf::new(local) != HashOf::new(stake))
            {
                info!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync stake snapshot: does not match local snapshot"
                );
                stake_snapshot = None;
            }
        }
        let snapshot_selection = snapshot.as_ref().and_then(|snapshot| {
            let roster = snapshot.commit_qc.validator_set.clone();
            if roster.is_empty() {
                return None;
            }
            let stake_snapshot = snapshot
                .stake_snapshot
                .as_ref()
                .filter(|snapshot| snapshot.matches_roster(&roster))
                .cloned();
            let selection = BlockSyncRosterSelection {
                roster,
                source: BlockSyncRosterSource::CommitRosterJournal,
                commit_qc: Some(snapshot.commit_qc.clone()),
                checkpoint: Some(snapshot.validator_checkpoint.clone()),
                stake_snapshot,
            };
            if let Some(key) = BlockSyncRosterCacheKey::from_hints(
                block_hash,
                block_height,
                block_view,
                consensus_mode,
                selection.commit_qc.as_ref(),
                selection.checkpoint.as_ref(),
                selection.stake_snapshot.as_ref(),
            ) {
                self.block_sync_roster_cache.insert(key, selection.clone());
            }
            Some(selection)
        });
        let roster_start = Instant::now();
        let persisted_roster_start = Instant::now();
        let allow_sidecar = !self.sidecar_quarantined_for_height(block_height);
        let persisted_roster = snapshot_selection.or_else(|| {
            persisted_roster_for_block(
                self.state.as_ref(),
                &self.kura,
                consensus_mode,
                block_height,
                block_hash,
                Some(block_view),
                &self.roster_validation_cache,
                Some(&mut self.block_sync_roster_cache),
                allow_sidecar,
            )
        });
        let roster_persisted_ms =
            u64::try_from(persisted_roster_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let cert_hint = incoming_qc.as_ref();
        let checkpoint_hint = validator_checkpoint.as_ref();
        let roster_cache_key = super::BlockSyncRosterCacheKey::from_hints(
            block_hash,
            block_height,
            block_view,
            consensus_mode,
            cert_hint,
            checkpoint_hint,
            stake_snapshot.as_ref(),
        );
        // Allow next-height block sync updates without roster artifacts; missing-block requests
        // already opt into the uncertified path.
        let allow_uncertified = allow_uncertified_block_sync_roster(
            block_height,
            local_height,
            requested_missing_block,
        );
        let selection_start = Instant::now();
        let selection = if let Some(selection) = persisted_roster {
            Some(selection)
        } else if let Some(selection) = roster_cache_key
            .as_ref()
            .and_then(|key| self.block_sync_roster_cache.get(key))
        {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                source = selection.source.as_str(),
                "block sync roster cache hit"
            );
            Some(selection)
        } else {
            let selection = select_block_sync_roster(
                &block,
                block_hash,
                block_height,
                None,
                cert_hint,
                checkpoint_hint,
                stake_snapshot.as_ref(),
                self.state.as_ref(),
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
                consensus_mode,
                mode_tag,
                allow_uncertified,
                &self.roster_validation_cache,
            );
            if let (Some(selection), Some(key)) = (selection.as_ref(), roster_cache_key.as_ref()) {
                if selection.commit_qc.is_some() || selection.checkpoint.is_some() {
                    self.block_sync_roster_cache
                        .insert(key.clone(), selection.clone());
                }
            }
            selection
        };
        let roster_select_ms =
            u64::try_from(selection_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let roster_ms = u64::try_from(roster_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let roster_validate_ms = roster_ms;
        let roster_snapshot = self
            .state
            .commit_roster_snapshot_for_block(block_height, block_hash);
        let Some(selection) = selection else {
            if block_known
                && cert_hint.is_none()
                && checkpoint_hint.is_none()
                && stake_snapshot.is_none()
                && has_commit_votes
            {
                if roster_snapshot.is_some() {
                    debug!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "dropping vote-only block sync update for known block with local commit roster snapshot"
                    );
                } else {
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "processing commit votes without roster hints for known block"
                    );
                    process_commit_votes(self);
                }
                self.clear_missing_block_request(
                    &block_hash,
                    MissingBlockClearReason::PayloadAvailable,
                );
                return Ok(());
            }
            if !block_known {
                let mut fallback_roster = self.effective_commit_topology();
                if fallback_roster.is_empty() {
                    fallback_roster = self.trusted_topology();
                }
                if !fallback_roster.is_empty() {
                    let fallback_topology = super::network_topology::Topology::new(fallback_roster);
                    let empty_signers =
                        BTreeSet::<crate::sumeragi::consensus::ValidatorIndex>::new();
                    if self.keep_exact_frontier_block_sync_repair_in_slot(
                        block_hash,
                        block_height,
                        block_view,
                        &empty_signers,
                        &fallback_topology,
                        "block_sync_update_missing_roster",
                    ) {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Deferred,
                            super::status::ConsensusMessageReason::RosterMissing,
                        );
                        return Ok(());
                    }
                    if self.maybe_request_pending_block_for_missing_qc(
                        block_hash,
                        block_height,
                        block_view,
                        block.signatures().count(),
                        fallback_topology.min_votes_for_commit().max(1),
                        &empty_signers,
                        &fallback_topology,
                    ) {
                        requested_missing_block = true;
                    }
                }
                if requested_missing_block {
                    let failover_requested = self.force_tracked_missing_height_sidecar_failover(
                        block_height,
                        block_hash,
                        "block_sync_update_missing_roster",
                    );
                    if failover_requested {
                        debug!(
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            "forced deterministic sidecar failover for tracked missing block without verifiable roster"
                        );
                    }
                }
            }
            warn!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                cert_hint = cert_hint.is_some(),
                checkpoint_hint = checkpoint_hint.is_some(),
                requested_missing_block,
                roster_snapshot = roster_snapshot.is_some(),
                roster_validate_ms = roster_ms,
                "dropping block sync update: no verifiable roster available"
            );
            super::status::inc_block_sync_drop_invalid_signatures();
            super::status::inc_block_sync_roster_drop_missing();
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::RosterMissing,
            );
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.note_block_sync_roster_drop("missing");
            }
            if block_known {
                self.clear_missing_block_request(
                    &block_hash,
                    MissingBlockClearReason::PayloadAvailable,
                );
            }
            return Ok(());
        };
        super::status::inc_block_sync_roster_source(selection.source.as_str());
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.note_block_sync_roster_source(selection.source.as_str());
        }
        info!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            source = selection.source.as_str(),
            "block sync roster selected"
        );
        self.cache_vote_roster(
            block_hash,
            block_height,
            block_view,
            selection.roster.clone(),
        );
        let topology = super::network_topology::Topology::new(selection.roster.clone());
        if let Some(checkpoint) = selection.checkpoint.clone() {
            super::status::record_validator_checkpoint(checkpoint);
        }
        // Persist commit rosters only once the block is known locally.
        let commit_roster_record = selection.commit_qc.as_ref().map(|cert| {
            let checkpoint = selection.checkpoint.clone().unwrap_or_else(|| {
                ValidatorSetCheckpoint::new(
                    cert.height,
                    cert.view,
                    cert.subject_block_hash,
                    cert.parent_state_root,
                    cert.post_state_root,
                    cert.validator_set.clone(),
                    cert.aggregate.signers_bitmap.clone(),
                    cert.aggregate.bls_aggregate_signature.clone(),
                    cert.validator_set_hash_version,
                    None,
                )
            });
            (cert.clone(), checkpoint, selection.stake_snapshot.clone())
        });
        if block_known {
            if let Some((cert, checkpoint, stake_snapshot)) = commit_roster_record.as_ref() {
                self.state
                    .record_commit_roster(cert, checkpoint, stake_snapshot.clone());
            }
            info!(
                hash = ?block_hash,
                height = block_height,
                "skipping block sync update for already known block"
            );
            process_commit_votes(self);
            // Known blocks may still be waiting on a commit QC (e.g., persisted before QC arrival).
            if let Some(qc) = incoming_qc.take().or_else(|| selection.commit_qc.clone()) {
                let qc_hash = HashOf::new(&qc);
                let cached_qc_match = cached_qc_for(
                    &self.qc_cache,
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    block_height,
                    block_view,
                    expected_epoch,
                )
                .is_some_and(|cached| HashOf::new(&cached) == qc_hash);
                let local_snapshot_qc_match = roster_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| HashOf::new(&snapshot.commit_qc) == qc_hash);
                if cached_qc_match && local_snapshot_qc_match {
                    debug!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "skipping redundant known-block QC replay: commit QC already cached locally"
                    );
                } else {
                    let commit_qc_match = selection
                        .commit_qc
                        .as_ref()
                        .is_some_and(|cert| HashOf::new(cert) == qc_hash);
                    let work = self.prepare_known_block_qc_work(
                        qc,
                        Arc::new(block),
                        topology.clone(),
                        stake_snapshot.clone(),
                        consensus_mode,
                        mode_tag,
                        prf_seed,
                        commit_qc_match,
                    );
                    if let Some(work) = work {
                        self.enqueue_known_block_qc_work(work);
                    }
                }
            }
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        let had_incoming_qc = incoming_qc.is_some();
        let signer_cache_key = BlockSignerCacheKey::new(
            block_hash,
            selection.roster.as_slice(),
            consensus_mode,
            prf_seed,
        );
        let signature_start = Instant::now();
        let cached_block_signers = signer_cache_key
            .as_ref()
            .and_then(|key| self.block_signer_cache.get(key));
        let block_signers = if let Some(signers) = cached_block_signers {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                signers = signers.len(),
                "block sync signer cache hit"
            );
            signers
        } else {
            let block_signers_result = {
                let world_view = self.state.world_view();
                validated_block_signers_from_world(
                    &block,
                    &topology,
                    &world_view,
                    mode_tag,
                    prf_seed,
                )
            };
            match block_signers_result {
                Ok(signers) => {
                    if let Some(key) = signer_cache_key.clone() {
                        self.block_signer_cache.insert(key, signers.clone());
                    }
                    signers
                }
                Err(err) => {
                    let local_height =
                        u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                    let parent_missing = block
                        .header()
                        .prev_block_hash()
                        .is_some_and(|hash| !self.block_known_locally(hash));
                    let ahead = block_height > local_height.saturating_add(1);
                    let defer_signatures = matches!(
                        err,
                        crate::block::SignatureVerificationError::UnknownSignature
                            | crate::block::SignatureVerificationError::UnknownSignatory
                            | crate::block::SignatureVerificationError::MissingPop
                    );
                    if parent_missing && ahead && defer_signatures {
                        let expected_height = local_height.saturating_add(1);
                        let expected_usize = usize::try_from(expected_height).ok();
                        let actual_usize = usize::try_from(block_height).ok();
                        if let Some(parent_hash) = block.header().prev_block_hash() {
                            let commit_topology = self.effective_commit_topology();
                            self.request_missing_parent(
                                block_hash,
                                block_height,
                                block_view,
                                parent_hash,
                                &commit_topology,
                                Some(&selection.roster),
                                expected_usize,
                                actual_usize,
                                "block_sync_signatures",
                            );
                            if block_height > expected_height.saturating_add(1) {
                                self.request_missing_parents_for_gap(
                                    &commit_topology,
                                    Some(&selection.roster),
                                    "block_sync_gap",
                                );
                            }
                        }
                        info!(
                            ?err,
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            local_height,
                            "deferring block sync update due to signature mismatch while behind"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Deferred,
                            super::status::ConsensusMessageReason::SignatureMismatchDeferred,
                        );
                        let created = super::message::BlockCreated {
                            block,
                            frontier: None,
                        };
                        let _ = self.handle_block_created_from_block_sync(
                            created,
                            sender.clone(),
                            true,
                            false,
                            false,
                        );
                        return Ok(());
                    }
                    let has_roster_evidence = incoming_qc.is_some()
                        || selection.commit_qc.is_some()
                        || selection.checkpoint.is_some();
                    if has_roster_evidence {
                        warn!(
                            ?err,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            has_incoming_qc = incoming_qc.is_some(),
                            has_commit_qc = selection.commit_qc.is_some(),
                            has_checkpoint = selection.checkpoint.is_some(),
                            "continuing block sync update despite signature mismatch because roster/QC evidence is available"
                        );
                        BTreeSet::new()
                    } else {
                        super::status::inc_block_sync_drop_invalid_signatures();
                        warn!(
                            ?err,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            "dropping block sync update with invalid or insufficient signatures"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::InvalidSignature,
                        );
                        return Ok(());
                    }
                }
            }
        };
        let signature_verify_ms =
            u64::try_from(signature_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let qc_candidate_start = Instant::now();
        let commit_quorum = topology.min_votes_for_commit().max(1);
        let mut candidate_qc = {
            let world_view = self.state.world_view();
            incoming_qc
                .or_else(|| selection.commit_qc.clone())
                .or_else(|| {
                    crate::block_sync::BlockSynchronizer::block_sync_qc_for_world(
                        &world_view,
                        self.config.consensus_mode,
                        &block,
                    )
                })
                .or_else(|| {
                    cached_qc_for(
                        &self.qc_cache,
                        crate::sumeragi::consensus::Phase::Commit,
                        block_hash,
                        block_height,
                        block_view,
                        expected_epoch,
                    )
                })
        };
        candidate_qc = candidate_qc.and_then(|qc| {
            if qc.height != block_height {
                warn!(
                    height = block_height,
                    view = block_view,
                    hash = %block_hash,
                    qc_height = qc.height,
                    "dropping block sync QC with mismatched height"
                );
                return None;
            }
            if qc.subject_block_hash != block_hash {
                warn!(
                    height = block_height,
                    view = block_view,
                    hash = %block_hash,
                    qc_hash = %qc.subject_block_hash,
                    "dropping block sync QC with mismatched block hash"
                );
                return None;
            }
            if qc.epoch != expected_epoch {
                warn!(
                    height = block_height,
                    view = block_view,
                    hash = %block_hash,
                    expected_epoch,
                    qc_epoch = qc.epoch,
                    "dropping block sync QC with mismatched epoch"
                );
                return None;
            }
            if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
                warn!(
                    height = block_height,
                    view = block_view,
                    hash = %block_hash,
                    phase = ?qc.phase,
                    "dropping block sync QC with non-precommit phase"
                );
                return None;
            }
            Some(qc)
        });
        let original_candidate_qc = candidate_qc.clone();
        let qc_candidate_ms =
            u64::try_from(qc_candidate_start.elapsed().as_millis()).unwrap_or(u64::MAX);

        let qc_validate_start = Instant::now();
        let commit_cert_hint_present = selection.commit_qc.is_some();
        let checkpoint_present = selection.checkpoint.is_some();
        let candidate_qc_present = candidate_qc.is_some();
        let candidate_qc_signers = candidate_qc.as_ref().map(qc_signer_count);
        let block_signer_count = block_signers.len();
        let signature_quorum_met = match consensus_mode {
            ConsensusMode::Permissioned => block_signer_count >= commit_quorum,
            ConsensusMode::Npos => {
                let signature_topology = super::topology_for_view(
                    &topology,
                    block_height,
                    block_view,
                    mode_tag,
                    prf_seed,
                );
                let mut signer_peers = BTreeSet::new();
                for signer in &block_signers {
                    let Ok(idx) = usize::try_from(*signer) else {
                        continue;
                    };
                    let Some(peer) = signature_topology.as_ref().get(idx) else {
                        continue;
                    };
                    signer_peers.insert(peer.clone());
                }
                if let Some(snapshot) = selection.stake_snapshot.as_ref() {
                    super::stake_snapshot::stake_quorum_reached_for_snapshot(
                        snapshot,
                        &selection.roster,
                        &signer_peers,
                    )
                    .unwrap_or(false)
                } else {
                    false
                }
            }
        };
        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let mut cached_qc_tally: Option<QcSignerTally> = None;
        let cached_qc_match = candidate_qc.as_ref().and_then(|qc| {
            cached_qc_for(
                &self.qc_cache,
                qc.phase,
                qc.subject_block_hash,
                qc.height,
                qc.view,
                qc.epoch,
            )
            .filter(|cached| HashOf::new(cached) == HashOf::new(qc))
        });
        // Reuse prior validation to skip expensive aggregate verification for identical QCs.
        let aggregate_ok = candidate_qc.as_ref().and_then(|qc| {
            if cached_qc_match.is_some() {
                Some(true)
            } else if selection
                .commit_qc
                .as_ref()
                .is_some_and(|cert| HashOf::new(cert) == HashOf::new(qc))
            {
                Some(true)
            } else {
                None
            }
        });
        let validated_qc = candidate_qc.as_ref().and_then(|qc| {
            let world_view = self.state.world_view();
            match validate_block_sync_qc(
                qc,
                &topology,
                &world_view,
                &block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
                aggregate_ok,
            ) {
                Ok((signers, present_signers)) => {
                    cached_qc_tally = Some(QcSignerTally {
                        voting_signers: signers,
                        present_signers,
                    });
                    Some(qc.clone())
                }
                Err(err) => {
                    record_qc_validation_error(self.telemetry_handle(), &err);
                    if had_incoming_qc {
                        super::status::inc_block_sync_qc_replaced();
                    }
                    let reason = qc_validation_reason(&err);
                    if Self::block_sync_qc_is_missing_context_error(&err) {
                        self.quarantine_block_sync_qc_candidate(
                            qc.clone(),
                            reason,
                            QuarantinedQcTarget::BlockSync,
                        );
                        warn!(
                            ?err,
                            reason,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            block_signers = block_signer_count,
                            candidate_qc_signers,
                            had_incoming_qc,
                            "quarantining block sync QC while dependencies are unresolved"
                        );
                    } else {
                        self.block_sync_qc_final_drop(reason);
                        warn!(
                            ?err,
                            reason,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            block_signers = block_signer_count,
                            candidate_qc_signers,
                            had_incoming_qc,
                            "dropping block sync QC after validation failure"
                        );
                    }
                    None
                }
            }
        });

        let derive_valid_qc = || {
            cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                block_height,
                block_view,
                expected_epoch,
            )
            .and_then(|qc| {
                let world_view = self.state.world_view();
                validate_block_sync_qc(
                    &qc,
                    &topology,
                    &world_view,
                    &block_signers,
                    block_view,
                    &self.roster_validation_cache.pops,
                    &self.common_config.chain,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                    mode_tag,
                    prf_seed,
                    Some(true),
                )
                .ok()
                .map(|_| qc)
            })
        };

        let (mut incoming_qc, incoming_qc_validated) = match (candidate_qc.take(), validated_qc) {
            (None, None) => {
                let derived = derive_valid_qc();
                let derived_validated = derived.is_some();
                (derived, derived_validated)
            }
            (_, Some(qc)) => (Some(qc), true),
            (Some(_), None) if had_incoming_qc => {
                let derived = derive_valid_qc();
                let derived_validated = derived.is_some();
                (derived, derived_validated)
            }
            (Some(_), None) => (None, false),
        };
        if incoming_qc_validated {
            if let (Some(qc), Some(tally)) = (incoming_qc.as_ref(), cached_qc_tally.take()) {
                self.note_validated_qc_tally(qc, tally);
            }
        }
        let qc_fallback_ms = if incoming_qc.is_none() && had_incoming_qc {
            let qc_fallback_start = Instant::now();
            if let Some(qc) = original_candidate_qc {
                let aggregate_ok = super::qc_aggregate_consistent(
                    &qc,
                    &topology,
                    &self.roster_validation_cache.pops,
                    &self.common_config.chain,
                    mode_tag,
                );
                if aggregate_ok {
                    let stake_quorum_ok = match consensus_mode {
                        ConsensusMode::Permissioned => true,
                        ConsensusMode::Npos => {
                            let mut ok = false;
                            if let Some(snapshot) = stake_snapshot.as_ref() {
                                let roster_len = topology.as_ref().len();
                                match super::qc_signer_indices(&qc, roster_len, roster_len) {
                                    Ok(parsed) => match super::signer_peers_for_topology(
                                        &parsed.voting,
                                        &topology,
                                    ) {
                                        Ok(signer_peers) => {
                                            ok = super::stake_snapshot::stake_quorum_reached_for_snapshot(
                                                snapshot,
                                                topology.as_ref(),
                                                &signer_peers,
                                            )
                                            .unwrap_or(false);
                                        }
                                        Err(_) => {
                                            warn!(
                                                height = block_height,
                                                view = block_view,
                                                block = %block_hash,
                                                "dropping block sync QC: signer mapping failed"
                                            );
                                        }
                                    },
                                    Err(_) => {
                                        warn!(
                                            height = block_height,
                                            view = block_view,
                                            block = %block_hash,
                                            "dropping block sync QC: invalid signer bitmap"
                                        );
                                    }
                                }
                            } else {
                                warn!(
                                    height = block_height,
                                    view = block_view,
                                    block = %block_hash,
                                    "dropping block sync QC: missing stake snapshot"
                                );
                            }
                            ok
                        }
                    };
                    if stake_quorum_ok {
                        let qc_signers = qc_signer_count(&qc);
                        info!(
                            hash = %block_hash,
                            height = block_height,
                            view = block_view,
                            qc_signers,
                            "accepting block sync QC validated from aggregate signature despite local validation failure"
                        );
                        incoming_qc = Some(qc);
                    }
                }
            }
            u64::try_from(qc_fallback_start.elapsed().as_millis()).unwrap_or(u64::MAX)
        } else {
            0
        };
        let qc_validate_ms =
            u64::try_from(qc_validate_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let hard_locked_conflict = incoming_qc.as_ref().is_some_and(|qc| {
            self.locked_qc.is_some_and(|lock| {
                Self::block_sync_qc_same_height_conflict(lock, qc)
                    && !Self::block_sync_qc_same_height_recoverable(lock, qc, true)
            })
        });
        if hard_locked_conflict
            && let (Some(qc), Some(lock)) = (incoming_qc.as_ref(), self.locked_qc)
        {
            self.log_block_sync_locked_qc_conflict(qc, lock, "block_sync_update.prefetch_cache");
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            // Treat a locked-chain conflict as unusable QC evidence in this update path.
            incoming_qc = None;
        }
        let incoming_qc_usable = incoming_qc_validated && !hard_locked_conflict;
        if incoming_qc_usable {
            if let Some(qc) = incoming_qc.as_ref() {
                self.quarantined_block_sync_qcs
                    .remove(&Self::qc_tally_key(qc));
                self.qc_cache.insert(
                    (
                        qc.phase,
                        qc.subject_block_hash,
                        qc.height,
                        qc.view,
                        qc.epoch,
                    ),
                    qc.clone(),
                );
            }
        }
        let qc_evidence_present = incoming_qc.is_some();
        let commit_cert_present = super::block_sync_commit_cert_present(
            commit_cert_hint_present,
            incoming_qc_validated,
            hard_locked_conflict,
        );
        let invalid_qc_present = had_incoming_qc && !incoming_qc_validated && !qc_evidence_present;
        let block_quorum_met = block_signer_count >= commit_quorum;
        if invalid_qc_present && !block_quorum_met && !commit_cert_present && !checkpoint_present {
            warn!(
                hash = ?block_hash,
                height = block_height,
                view = block_view,
                "dropping block sync update with invalid QC and insufficient quorum"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let mut quorum_available = block_sync_quorum_available(
            block_signer_count,
            commit_quorum,
            signature_quorum_met,
            qc_evidence_present,
            commit_cert_present,
            checkpoint_present,
            explicit_requested_missing_block,
            block_height,
            local_height,
        );
        if !quorum_available
            && !qc_evidence_present
            && !commit_cert_present
            && !checkpoint_present
            && block_signer_count < commit_quorum
            && !requested_missing_block
        {
            if self.maybe_request_pending_block_for_missing_qc(
                block_hash,
                block_height,
                block_view,
                block_signer_count,
                commit_quorum,
                &block_signers,
                &topology,
            ) {
                // Invariant A: sparse missing-QC updates must transition request state in this
                // same event step (or stay explicitly suppressed via backoff).
                requested_missing_block = true;
                quorum_available = block_sync_quorum_available(
                    block_signer_count,
                    commit_quorum,
                    signature_quorum_met,
                    qc_evidence_present,
                    commit_cert_present,
                    checkpoint_present,
                    requested_missing_block,
                    block_height,
                    local_height,
                );
            }
        }
        if !quorum_available {
            if !qc_evidence_present
                && !commit_cert_present
                && !checkpoint_present
                && block_signer_count < commit_quorum
                && !requested_missing_block
            {
                debug!(
                    hash = ?block_hash,
                    height = block_height,
                    view = block_view,
                    "sparse block sync update remained unrequested after recovery planning"
                );
            }
            if self.keep_exact_frontier_block_sync_repair_in_slot(
                block_hash,
                block_height,
                block_view,
                &block_signers,
                &topology,
                "block_sync_update_missing_commit_quorum",
            ) {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockSyncUpdate,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::QuorumMissing,
                );
                return Ok(());
            }
            super::status::inc_block_sync_drop_invalid_signatures();
            let warn_cooldown = self
                .rebroadcast_cooldown()
                .max(super::BLOCK_SYNC_WARN_COOLDOWN_FLOOR);
            let warn_now = Instant::now();
            if let Some(suppressed_since_last) = self.block_sync_warning_log.allow(
                super::BlockSyncWarningKind::MissingCommitRoleQuorum,
                block_hash,
                block_height,
                block_view,
                warn_now,
                warn_cooldown,
                super::BLOCK_SYNC_WARN_BURST_WINDOW,
                super::BLOCK_SYNC_WARN_BURST_CAP,
            ) {
                self.hotspot_log_summary.record_block_sync_warn();
                if suppressed_since_last > 0 {
                    self.hotspot_log_summary
                        .record_block_sync_suppressed(suppressed_since_last);
                }
                warn!(
                    hash = ?block_hash,
                    height = block_height,
                    view = block_view,
                    block_signers = block_signer_count,
                    signatures = block.signatures().count(),
                    commit_quorum,
                    candidate_qc_present,
                    candidate_qc_signers,
                    qc_evidence_present,
                    incoming_qc_validated,
                    missing_request = requested_missing_block,
                    local_height,
                    suppressed_since_last,
                    warn_cooldown_ms = warn_cooldown.as_millis(),
                    "dropping block sync update missing commit-role quorum"
                );
            } else {
                self.hotspot_log_summary.record_block_sync_suppressed(1);
            }
            self.hotspot_log_summary.emit_if_due(warn_now);
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::QuorumMissing,
            );
            return Ok(());
        }
        let incoming_qc_signers = incoming_qc.as_ref().map(qc_signer_count);
        let allow_nonextending_qc = selection.commit_qc.is_some()
            || incoming_qc.as_ref().is_some_and(|cert| {
                let inputs = self.roster_validation_cache.inputs_for_roster(
                    &cert.validator_set,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                );
                super::validate_commit_qc_roster_cached(
                    &self.roster_validation_cache,
                    cert,
                    block_hash,
                    block_height,
                    Some(block_view),
                    consensus_mode,
                    expected_epoch,
                    &self.common_config.chain,
                    mode_tag,
                    false,
                    &inputs,
                )
                .is_ok()
            })
            || incoming_qc_usable;
        info!(
            hash = ?block_hash,
            height = block_height,
            block_signers = block_signer_count,
            candidate_qc_present,
            candidate_qc_signers,
            incoming_qc_signers,
            "applying block sync update"
        );
        let quorum_only_same_height_frontier_conflict = block_quorum_met
            && !incoming_qc_usable
            && !commit_cert_present
            && !checkpoint_present
            && self
                .local_conflicting_frontier_vote(block_height, block_hash)
                .is_some();
        // Raw block-signature quorum is enough to hydrate the payload locally and keep stale-view
        // catch-up moving, but it is not authoritative enough to steal same-height frontier
        // ownership from a branch that this validator already voted on. Only certified evidence
        // may bypass the passive retained branch path in that exact conflict case.
        let allow_frontier_owner_preserve_on_payload_mismatch =
            !incoming_qc_usable && !commit_cert_present && !checkpoint_present;
        let allow_authoritative_frontier_owner_supersede = incoming_qc_usable
            || commit_cert_present
            || checkpoint_present
            || (block_quorum_met && !quorum_only_same_height_frontier_conflict);
        let created = super::message::BlockCreated {
            block,
            frontier: None,
        };
        let block_apply_start = Instant::now();
        let creation_result = self.handle_block_created_from_block_sync(
            created,
            sender.clone(),
            allow_frontier_owner_preserve_on_payload_mismatch,
            allow_authoritative_frontier_owner_supersede,
            has_commit_votes || incoming_qc_usable || commit_cert_present || checkpoint_present,
        );
        let block_apply_ms =
            u64::try_from(block_apply_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let block_known_after_creation = self.block_known_locally(block_hash);
        let creation_ok = creation_result.is_ok();
        let ready_for_qc = block_sync_ready_for_qc(block_known_after_creation, &creation_result);
        if block_known_after_creation {
            if let Some((cert, checkpoint, stake_snapshot)) = commit_roster_record.as_ref() {
                self.state
                    .record_commit_roster(cert, checkpoint, stake_snapshot.clone());
            }
        }
        if !ready_for_qc {
            if let Err(err) = &creation_result {
                warn!(
                    ?err,
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync update: failed to apply block payload"
                );
            } else {
                warn!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync update: block not accepted locally"
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadUnapplied,
            );
        }
        let commit_votes_post_start = Instant::now();
        process_commit_votes(self);
        let commit_votes_post_ms =
            u64::try_from(commit_votes_post_start.elapsed().as_millis()).unwrap_or(u64::MAX);

        let qc_to_apply = if ready_for_qc {
            incoming_qc.take()
        } else {
            None
        };

        let mut qc_apply_tally_ms = 0;
        let mut qc_apply_process_ms = 0;
        let mut qc_apply_commit_ms = 0;
        let qc_apply_start = Instant::now();
        let qc_apply_result = block_sync_apply_qc_after_block(
            creation_result,
            block_known_after_creation,
            qc_to_apply,
            |qc| {
                if topology.as_ref().is_empty() {
                    let _ = self.handle_roster_unavailable_recovery(
                        block_height,
                        block_view,
                        Some(block_hash),
                        self.queue.queued_len(),
                        Instant::now(),
                        super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                        "block_sync_update_qc_empty_commit_topology",
                    );
                    warn!(
                        height = block_height,
                        view = block_view,
                        "dropping block sync QC: empty commit topology"
                    );
                    return Ok(());
                }
                if qc.subject_block_hash != block_hash {
                    warn!(
                        incoming_hash = %block_hash,
                        qc_hash = %qc.subject_block_hash,
                        "ignoring block sync QC that does not match block hash"
                    );
                    return Ok(());
                }
                if qc.height != block_height {
                    warn!(
                        incoming_hash = %block_hash,
                        height = block_height,
                        qc_height = qc.height,
                        "ignoring block sync QC that does not match block height"
                    );
                    return Ok(());
                }
                let expected_epoch = self.epoch_for_height(block_height);
                if qc.epoch != expected_epoch {
                    warn!(
                        incoming_hash = %block_hash,
                        height = block_height,
                        expected_epoch,
                        qc_epoch = qc.epoch,
                        "ignoring block sync QC with mismatched epoch"
                    );
                    return Ok(());
                }
                if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
                    warn!(
                        incoming_hash = %block_hash,
                        phase = ?qc.phase,
                        "ignoring block sync QC with non-precommit phase"
                    );
                    return Ok(());
                }
                if let Some(lock) = self.locked_qc {
                    let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, &qc);
                    if same_height_conflict
                        && !Self::block_sync_qc_same_height_recoverable(
                            lock,
                            &qc,
                            allow_nonextending_qc,
                        )
                    {
                        crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                        self.log_block_sync_locked_qc_conflict(
                            &qc,
                            lock,
                            "block_sync_update.height_conflict",
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::Qc,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::LockedQc,
                        );
                        return Ok(());
                    }
                }
                if self.block_sync_qc_is_stale_against_lock(&qc) {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        incoming_hash = %qc.subject_block_hash,
                        locked_height = self.locked_qc.map(|lock| lock.height),
                        "dropping stale block sync QC below locked height"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
                    );
                    return Ok(());
                }
                let extends_locked = self.block_sync_qc_extends_locked_chain(&qc);
                if !extends_locked && !allow_nonextending_qc {
                    if self.defer_block_sync_qc_while_locked_payload_missing(
                        &qc,
                        "block_sync_update.non_extending.missing_locked_payload",
                    ) {
                        return Ok(());
                    }
                    if self.block_sync_qc_is_stale_against_lock(&qc) {
                        debug!(
                            height = qc.height,
                            view = qc.view,
                            incoming_hash = %qc.subject_block_hash,
                            locked_height = self.locked_qc.map(|lock| lock.height),
                            "dropping stale block sync QC below locked height"
                        );
                    } else if let Some(lock) = self.locked_qc {
                        self.log_block_sync_locked_qc_conflict(
                            &qc,
                            lock,
                            "block_sync_update.non_extending",
                        );
                    } else {
                        info!(
                            height = qc.height,
                            view = qc.view,
                            incoming_hash = %qc.subject_block_hash,
                            "dropping block sync QC that does not extend locked chain"
                        );
                    }
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
                    );
                    return Ok(());
                }
                if !extends_locked {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        incoming_hash = %qc.subject_block_hash,
                        "retaining non-extending block sync QC for lock realignment"
                    );
                }
                let qc_signers = qc_signer_count(&qc);
                let tally_start = Instant::now();
                let tally_result = if let Some(tally) =
                    self.qc_signer_tally.get(&Self::qc_tally_key(&qc)).cloned()
                {
                    Ok(tally)
                } else {
                    let world_view = self.state.world_view();
                    tally_qc_against_block_signers(
                        &qc,
                        &topology,
                        &world_view,
                        &block_signers,
                        block_view,
                        &self.roster_validation_cache.pops,
                        &self.common_config.chain,
                        consensus_mode,
                        stake_snapshot.as_ref(),
                        mode_tag,
                        prf_seed,
                        None,
                    )
                };
                qc_apply_tally_ms =
                    u64::try_from(tally_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                match tally_result {
                    Ok(tally) => {
                        crate::sumeragi::status::record_precommit_signers(
                            crate::sumeragi::status::PrecommitSignerRecord {
                                block_hash,
                                height: qc.height,
                                view: qc.view,
                                epoch: qc.epoch,
                                parent_state_root: qc.parent_state_root,
                                post_state_root: qc.post_state_root,
                                signers: tally.voting_signers.clone(),
                                bls_aggregate_signature: qc
                                    .aggregate
                                    .bls_aggregate_signature
                                    .clone(),
                                roster_len: topology.as_ref().len(),
                                mode_tag: mode_tag.to_string(),
                                validator_set: topology.as_ref().to_vec(),
                                stake_snapshot: stake_snapshot.clone(),
                            },
                        );
                        self.note_validated_qc_tally(&qc, tally.clone());
                        let block_known_for_commit =
                            self.pending
                                .pending_blocks
                                .get(&block_hash)
                                .is_some_and(|pending| {
                                    !pending.is_retry_aborted()
                                        && pending.validation_status == ValidationStatus::Valid
                                })
                                || self.subsystems.commit.inflight.as_ref().is_some_and(
                                    |inflight| {
                                        inflight.block_hash == block_hash
                                            && !inflight.pending.aborted
                                    },
                                )
                                || self.kura.get_block_height_by_hash(block_hash).is_some();
                        let process_start = Instant::now();
                        let process_ok = self.process_precommit_qc(
                            &qc,
                            block_known_for_commit,
                            allow_nonextending_qc,
                        );
                        qc_apply_process_ms =
                            u64::try_from(process_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                        if !process_ok {
                            if self.block_sync_qc_is_stale_against_lock(&qc) {
                                debug!(
                                    height = qc.height,
                                    view = qc.view,
                                    incoming_hash = %qc.subject_block_hash,
                                    locked_height = self.locked_qc.map(|lock| lock.height),
                                    "dropping stale block sync QC below locked height"
                                );
                            } else if let Some(lock) = self.locked_qc {
                                self.log_block_sync_locked_qc_conflict(
                                    &qc,
                                    lock,
                                    "block_sync_update.precommit_reject",
                                );
                            }
                            return Ok(());
                        }
                        super::status::record_commit_qc(qc.clone());
                        self.qc_cache.insert(
                            (
                                qc.phase,
                                qc.subject_block_hash,
                                qc.height,
                                qc.view,
                                qc.epoch,
                            ),
                            qc.clone(),
                        );
                        #[cfg(feature = "telemetry")]
                        if let Some(telemetry) = self.telemetry_handle() {
                            telemetry.note_qc_signer_counts(
                                "precommit",
                                tally.present_signers,
                                tally.voting_signers.len(),
                            );
                        }
                        debug!(
                            incoming_hash = %block_hash,
                            signers = tally.voting_signers.len(),
                            qc_signers,
                            "applied block sync QC after validation"
                        );
                        if block_known_for_commit {
                            let commit_start = Instant::now();
                            self.apply_commit_qc(
                                &qc,
                                topology.as_ref(),
                                block_hash,
                                block_height,
                                block_view,
                            );
                            if self.runtime_da_enabled() {
                                self.clean_rbc_sessions_for_committed_block_if_settled(
                                    block_hash,
                                    block_height,
                                );
                            }
                            qc_apply_commit_ms = u64::try_from(commit_start.elapsed().as_millis())
                                .unwrap_or(u64::MAX);
                            self.request_commit_pipeline_for_round(
                                block_height,
                                block_view,
                                super::status::RoundPhaseTrace::WaitCommitQc,
                                super::status::RoundEventCauseTrace::BlockSyncUpdated,
                                None,
                            );
                        } else {
                            debug!(
                                incoming_hash = %block_hash,
                                height = block_height,
                                view = block_view,
                                "deferring commit apply for block sync QC until block is validated"
                            );
                        }
                    }
                    Err(err) => {
                        record_qc_validation_error(self.telemetry_handle(), &err);
                        warn!(
                            ?err,
                            reason = qc_validation_reason(&err),
                            incoming_hash = %block_hash,
                            height = block_height,
                            view = block_view,
                            qc_signers,
                            block_signers = block_signer_count,
                            "dropping block sync QC after validation failure"
                        );
                    }
                }
                Ok(())
            },
        );
        let qc_apply_ms = u64::try_from(qc_apply_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        qc_apply_result?;
        if creation_ok && !block_known_after_creation {
            if let Some(qc) = incoming_qc.take() {
                // Cache the QC so we can reuse it once the block becomes available locally.
                self.cache_block_sync_qc_for_unknown_block(
                    qc,
                    block_hash,
                    block_height,
                    block_view,
                    &topology,
                    &block_signers,
                    allow_nonextending_qc,
                    consensus_mode,
                    stake_snapshot.clone(),
                    mode_tag,
                    prf_seed,
                );
            }
        }

        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            kura_committed_ms,
            kura_known_ms,
            roster_validate_ms,
            roster_persisted_ms,
            roster_select_ms,
            signature_verify_ms,
            commit_votes_pre_ms,
            commit_votes_post_ms,
            qc_candidate_ms,
            qc_validate_ms,
            qc_fallback_ms,
            block_apply_ms,
            qc_apply_ms,
            qc_apply_tally_ms,
            qc_apply_process_ms,
            qc_apply_commit_ms,
            "block sync update substep timings"
        );

        Ok(())
    }

    /// Cache a validated precommit QC from block sync when the block payload is not ready yet.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn cache_block_sync_qc_for_unknown_block(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        topology: &super::network_topology::Topology,
        block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        allow_nonextending_qc: bool,
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<super::stake_snapshot::CommitStakeSnapshot>,
        mode_tag: &str,
        prf_seed: Option<[u8; 32]>,
    ) {
        if topology.as_ref().is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                block_height,
                block_view,
                Some(block_hash),
                self.queue.queued_len(),
                Instant::now(),
                super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "cache_block_sync_qc_empty_commit_topology",
            );
            warn!(
                height = block_height,
                view = block_view,
                "dropping cached block sync QC: empty commit topology"
            );
            return;
        }
        if qc.subject_block_hash != block_hash {
            warn!(
                incoming_hash = %block_hash,
                qc_hash = %qc.subject_block_hash,
                "ignoring cached block sync QC that does not match block hash"
            );
            return;
        }
        if qc.height != block_height {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                qc_height = qc.height,
                "ignoring cached block sync QC that does not match block height"
            );
            return;
        }
        let expected_epoch = self.epoch_for_height(block_height);
        if qc.epoch != expected_epoch {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                expected_epoch,
                qc_epoch = qc.epoch,
                "ignoring cached block sync QC with mismatched epoch"
            );
            return;
        }
        if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            warn!(
                incoming_hash = %block_hash,
                phase = ?qc.phase,
                "ignoring cached block sync QC with non-precommit phase"
            );
            return;
        }
        let qc_ref = crate::sumeragi::consensus::QcHeaderRef {
            phase: qc.phase,
            subject_block_hash: qc.subject_block_hash,
            height: qc.height,
            view: qc.view,
            epoch: qc.epoch,
        };
        if let Some(lock) = self.locked_qc {
            let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, &qc);
            if same_height_conflict
                && !Self::block_sync_qc_same_height_recoverable(lock, &qc, allow_nonextending_qc)
            {
                crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                self.log_block_sync_locked_qc_conflict(
                    &qc,
                    lock,
                    "cached_block_sync_qc.height_conflict",
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Qc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::LockedQc,
                );
                return;
            }
        }
        if self.block_sync_qc_is_stale_against_lock(&qc) {
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                locked_height = self.locked_qc.map(|lock| lock.height),
                "dropping stale cached block sync QC below locked height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return;
        }
        let extends_locked = self.block_sync_qc_extends_locked_chain(&qc);
        if !extends_locked && !allow_nonextending_qc {
            if self.defer_block_sync_qc_while_locked_payload_missing(
                &qc,
                "cached_block_sync_qc.non_extending.missing_locked_payload",
            ) {
                return;
            }
            if self.block_sync_qc_is_stale_against_lock(&qc) {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    incoming_hash = %qc.subject_block_hash,
                    locked_height = self.locked_qc.map(|lock| lock.height),
                    "dropping stale block sync QC below locked height"
                );
            } else if let Some(lock) = self.locked_qc {
                self.log_block_sync_locked_qc_conflict(
                    &qc,
                    lock,
                    "cached_block_sync_qc.non_extending",
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return;
        }
        if !extends_locked {
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                "retaining cached non-extending block sync QC for lock realignment"
            );
        }
        let qc_signers = qc_signer_count(&qc);
        let tally_result = {
            let world_view = self.state.world_view();
            tally_qc_against_block_signers(
                &qc,
                topology,
                &world_view,
                block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
                None,
            )
        };
        match tally_result {
            Ok(tally) => {
                crate::sumeragi::status::record_precommit_signers(
                    crate::sumeragi::status::PrecommitSignerRecord {
                        block_hash,
                        height: qc.height,
                        view: qc.view,
                        epoch: qc.epoch,
                        parent_state_root: qc.parent_state_root,
                        post_state_root: qc.post_state_root,
                        signers: tally.voting_signers.clone(),
                        bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
                        roster_len: topology.as_ref().len(),
                        mode_tag: mode_tag.to_string(),
                        validator_set: topology.as_ref().to_vec(),
                        stake_snapshot: stake_snapshot.clone(),
                    },
                );
                self.note_validated_qc_tally(&qc, tally.clone());
                if !self.process_precommit_qc(&qc, false, allow_nonextending_qc) {
                    if self.block_sync_qc_is_stale_against_lock(&qc) {
                        debug!(
                            height = qc.height,
                            view = qc.view,
                            incoming_hash = %qc.subject_block_hash,
                            locked_height = self.locked_qc.map(|lock| lock.height),
                            "dropping stale block sync QC below locked height"
                        );
                    } else if let Some(lock) = self.locked_qc {
                        if Self::block_sync_qc_same_height_conflict(lock, &qc) {
                            crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                        }
                        self.log_block_sync_locked_qc_conflict(
                            &qc,
                            lock,
                            "cached_block_sync_qc.precommit_reject",
                        );
                    }
                    return;
                }
                if allow_nonextending_qc {
                    let should_update = self
                        .locked_qc
                        .is_none_or(|lock| (qc.height, qc.view) > (lock.height, lock.view));
                    if should_update {
                        super::status::set_locked_qc(
                            qc.height,
                            qc.view,
                            Some(qc.subject_block_hash),
                        );
                        self.locked_qc = Some(qc_ref);
                        self.prune_precommit_votes_conflicting_with_lock(qc_ref);
                    }
                }
                self.quarantined_block_sync_qcs
                    .remove(&Self::qc_tally_key(&qc));
                super::status::record_commit_qc(qc.clone());
                self.qc_cache.insert(
                    (
                        qc.phase,
                        qc.subject_block_hash,
                        qc.height,
                        qc.view,
                        qc.epoch,
                    ),
                    qc,
                );
                debug!(
                    incoming_hash = %block_hash,
                    signers = tally.voting_signers.len(),
                    qc_signers,
                    "cached block sync QC before block payload is ready"
                );
            }
            Err(err) => {
                record_qc_validation_error(self.telemetry_handle(), &err);
                let reason = qc_validation_reason(&err);
                if Self::block_sync_qc_is_missing_context_error(&err) {
                    self.quarantine_block_sync_qc_candidate(
                        qc.clone(),
                        reason,
                        QuarantinedQcTarget::BlockSync,
                    );
                    warn!(
                        ?err,
                        reason,
                        incoming_hash = %block_hash,
                        height = block_height,
                        view = block_view,
                        qc_signers,
                        block_signers = block_signers.len(),
                        "quarantining cached block sync QC after transient validation failure"
                    );
                } else {
                    self.block_sync_qc_final_drop(reason);
                    warn!(
                        ?err,
                        reason,
                        incoming_hash = %block_hash,
                        height = block_height,
                        view = block_view,
                        qc_signers,
                        block_signers = block_signers.len(),
                        "dropping cached block sync QC after validation failure"
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_fetch_pending_block(
        &mut self,
        request: super::message::FetchPendingBlock,
    ) -> Result<()> {
        let block_hash = request.block_hash;
        let peer = request.requester;
        let request_priority = request
            .priority
            .unwrap_or(FetchPendingBlockPriority::Background);
        let dedup_key = super::BlockPayloadDedupKey::FetchPendingBlock {
            height: request.height,
            view: request.view,
            block_hash,
            requester_hash: CryptoHash::new(peer.encode()),
            priority: request_priority,
        };
        let requester_roster_proof_known = request.requester_roster_proof_known.unwrap_or(false);
        let request_meta = super::PendingFetchRequestMeta {
            priority: request_priority,
            requester_roster_proof_known,
        };
        let force_bypass_queue = false;
        let mut invalid_payload = false;

        let inflight_response = if let Some(inflight) = self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .filter(|inflight| inflight.block_hash == block_hash)
        {
            if matches!(
                inflight.pending.validation_status,
                ValidationStatus::Invalid
            ) {
                debug!(
                    hash = %block_hash,
                    "skipping fetch response for invalid inflight pending block"
                );
                invalid_payload = true;
                None
            } else {
                Some(inflight.pending.block.clone())
            }
        } else {
            None
        };
        if let Some(block) = inflight_response {
            let mut requesters = self.take_pending_fetch_requesters(&block_hash);
            requesters
                .entry(peer.clone())
                .and_modify(|stored| {
                    stored.priority = stored.priority.max(request_meta.priority);
                    stored.requester_roster_proof_known |=
                        request_meta.requester_roster_proof_known;
                })
                .or_insert(request_meta);
            self.send_fetch_pending_block_responses(
                requesters,
                &block,
                force_bypass_queue,
                /*allow_highest_qc_bypass*/ true,
                /*allow_hintless_block_sync_bypass*/ false,
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }

        let pending_response = if let Some(pending) = self.pending.pending_blocks.get(&block_hash) {
            if matches!(pending.validation_status, ValidationStatus::Invalid) {
                debug!(
                    hash = %block_hash,
                    "skipping fetch response for invalid pending block"
                );
                invalid_payload = true;
                None
            } else {
                Some(pending.block.clone())
            }
        } else {
            None
        };
        if let Some(block) = pending_response {
            let mut requesters = self.take_pending_fetch_requesters(&block_hash);
            requesters
                .entry(peer.clone())
                .and_modify(|stored| {
                    stored.priority = stored.priority.max(request_meta.priority);
                    stored.requester_roster_proof_known |=
                        request_meta.requester_roster_proof_known;
                })
                .or_insert(request_meta);
            self.send_fetch_pending_block_responses(
                requesters,
                &block,
                force_bypass_queue,
                /*allow_highest_qc_bypass*/ true,
                /*allow_hintless_block_sync_bypass*/ false,
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }

        let deferred_response =
            self.deferred_block_sync_updates
                .iter()
                .find_map(|((height, view, hash), entry)| {
                    (*hash == block_hash)
                        .then(|| (*height, *view, Arc::new(entry.update.block.clone())))
                });
        if let Some((_, _, block)) = deferred_response {
            let mut requesters = self.take_pending_fetch_requesters(&block_hash);
            requesters
                .entry(peer.clone())
                .and_modify(|stored| {
                    stored.priority = stored.priority.max(request_meta.priority);
                    stored.requester_roster_proof_known |=
                        request_meta.requester_roster_proof_known;
                })
                .or_insert(request_meta);
            self.send_fetch_pending_block_responses(
                requesters,
                block.as_ref(),
                force_bypass_queue,
                /*allow_highest_qc_bypass*/ true,
                /*allow_hintless_block_sync_bypass*/ false,
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }

        if let Some(height) = self.kura.get_block_height_by_hash(block_hash) {
            if let Some(block) = self.kura.get_block(height) {
                let block = block.as_ref();
                let mut requesters = self.take_pending_fetch_requesters(&block_hash);
                requesters
                    .entry(peer.clone())
                    .and_modify(|stored| {
                        stored.priority = stored.priority.max(request_meta.priority);
                        stored.requester_roster_proof_known |=
                            request_meta.requester_roster_proof_known;
                    })
                    .or_insert(request_meta);
                let response = self.build_fetch_pending_block_payload(block);
                if self.should_defer_canonical_committed_fetch_response(block, &response) {
                    debug!(
                        height = block.header().height().get(),
                        view = block.header().view_change_index(),
                        block = %block_hash,
                        peer = %peer,
                        "deferring exact-tip fetch response until commit proof is available"
                    );
                    self.pending
                        .pending_fetch_requests
                        .insert(block_hash, requesters);
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::FetchPendingBlock,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::NotFound,
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                }
                self.send_fetch_pending_block_responses(
                    requesters,
                    block,
                    force_bypass_queue,
                    /*allow_highest_qc_bypass*/ true,
                    /*allow_hintless_block_sync_bypass*/ true,
                );
                self.release_block_payload_dedup(&dedup_key);
                return Ok(());
            }
        }

        if !invalid_payload {
            self.stash_pending_fetch_request(
                block_hash,
                peer,
                request_priority,
                requester_roster_proof_known,
            );
        }

        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::FetchPendingBlock,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::NotFound,
        );
        self.release_block_payload_dedup(&dedup_key);
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_fetch_block_body(
        &mut self,
        request: super::message::FetchBlockBody,
    ) -> Result<()> {
        let dedup_key = super::BlockPayloadDedupKey::FetchBlockBody {
            height: request.height,
            view: request.view,
            block_hash: request.block_hash,
            requester_hash: CryptoHash::new(request.requester.encode()),
        };
        let block_hash = request.block_hash;
        let peer = request.requester;
        if let Some(block) = self.local_signed_block_for_hash(block_hash) {
            let header = block.header();
            if header.height().get() == request.height && header.view_change_index() == request.view
            {
                let payload = self.build_fetch_pending_block_payload(block.as_ref());
                if self.should_defer_canonical_committed_fetch_response(block.as_ref(), &payload) {
                    debug!(
                        height = request.height,
                        view = request.view,
                        block = %block_hash,
                        peer = %peer,
                        "deferring exact body response until commit proof is available"
                    );
                    self.stash_pending_block_body_request(block_hash, peer);
                    self.release_block_payload_dedup(&dedup_key);
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::FetchBlockBody,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::NotFound,
                    );
                    return Ok(());
                }
                let response = Self::block_body_response_from_payload(
                    block_hash,
                    request.height,
                    request.view,
                    payload,
                );
                self.dispatch_fetch_pending_block_response(
                    peer,
                    BlockMessage::BlockBodyResponse(response),
                    /*bypass_queue*/ true,
                );
                self.release_block_payload_dedup(&dedup_key);
                return Ok(());
            }
        }
        if let Some(slot) = self.frontier_slot.as_mut()
            && slot.block_hash == block_hash
            && slot.height == request.height
            && slot.view == request.view
        {
            slot.repair_state.pending_requesters.insert(peer);
            slot.sync_compat_fields();
        }
        self.release_block_payload_dedup(&dedup_key);
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::FetchBlockBody,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::NotFound,
        );
        Ok(())
    }

    fn allow_same_height_block_body_repair(
        &self,
        response: &super::message::BlockBodyResponse,
    ) -> bool {
        if !self.frontier_slot_is_exact_height(response.height) {
            return false;
        }
        let committed_height = self.committed_height_snapshot();
        let now = Instant::now();
        self.pending
            .missing_block_requests
            .get(&response.block_hash)
            .is_some_and(|request| {
                request.phase == crate::sumeragi::consensus::Phase::Commit
                    && request.height == response.height
                    && request.view == response.view
                    && self.missing_block_request_has_actionable_dependency(
                        response.block_hash,
                        request,
                        committed_height,
                        now,
                    )
            })
            || self.deferred_missing_payload_qcs.values().any(|entry| {
                entry.qc.phase == crate::sumeragi::consensus::Phase::Commit
                    && entry.qc.subject_block_hash == response.block_hash
                    && entry.qc.height == response.height
                    && entry.qc.view == response.view
                    && self.deferred_missing_payload_qc_has_actionable_dependency(
                        entry,
                        committed_height,
                        now,
                    )
            })
            || self.missing_commit_qc_repair_active_for_round(
                response.block_hash,
                response.height,
                response.view,
                committed_height,
                now,
            )
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_block_body_response(
        &mut self,
        response: super::message::BlockBodyResponse,
        sender: Option<PeerId>,
    ) -> Result<()> {
        let dedup_key = super::BlockPayloadDedupKey::BlockBodyResponse {
            height: response.height,
            view: response.view,
            block_hash: response.block_hash,
        };
        if !self.frontier_slot_is_exact_height(response.height) {
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }
        let slot_matches = self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.block_hash == response.block_hash
                && slot.height == response.height
                && slot.view == response.view
        });
        let allow_same_height_repair =
            !slot_matches && self.allow_same_height_block_body_repair(&response);
        if !slot_matches && !allow_same_height_repair {
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }
        let sender_for_slot = sender
            .clone()
            .filter(|peer| peer != self.common_config.peer.id());
        if allow_same_height_repair {
            info!(
                height = response.height,
                view = response.view,
                block = %response.block_hash,
                active_frontier = ?self
                    .frontier_slot
                    .as_ref()
                    .map(|slot| (slot.height, slot.view, slot.block_hash)),
                "accepting BlockBodyResponse for exact-height same-slot repair after frontier ownership moved"
            );
        }
        let result = match response.body {
            super::message::BlockBodyData::BlockCreated(block_created) => {
                let header = block_created.block.header();
                if block_created.block.hash() != response.block_hash
                    || header.height().get() != response.height
                    || header.view_change_index() != response.view
                {
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockBodyResponse,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::InvalidPayload,
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                }
                self.handle_block_created(block_created, sender)
            }
            super::message::BlockBodyData::BlockSyncUpdate(update) => {
                let header = update.block.header();
                if update.block.hash() != response.block_hash
                    || header.height().get() != response.height
                    || header.view_change_index() != response.view
                {
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockBodyResponse,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::InvalidPayload,
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                }
                self.handle_block_sync_update(update, sender)
            }
        };
        let body_materialized = self.frontier_block_materialized_locally(response.block_hash);
        let slot_matches_after = self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.block_hash == response.block_hash
                && slot.height == response.height
                && slot.view == response.view
        });
        if body_materialized && slot_matches_after {
            let _ = self.handle_frontier_slot_event(
                Instant::now(),
                super::FrontierSlotEvent::OnBodyAvailable {
                    block_hash: response.block_hash,
                    view: response.view,
                    sender: sender_for_slot,
                },
            );
        } else {
            self.release_block_payload_dedup(&dedup_key);
        }
        result
    }

    fn materialize_frontier_block_sync_payload_for_qc_recovery(
        &mut self,
        block: &SignedBlock,
        observed_commit_qc_epoch: Option<u64>,
    ) -> bool {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        if self.block_known_locally(block_hash) || !self.frontier_slot_is_exact_height(block_height)
        {
            return false;
        }

        let authoritative_owner = self.authoritative_slot_owner_hash(block_height, block_view);
        let deferred_commit_qc_epoch =
            self.deferred_missing_payload_qcs
                .values()
                .find_map(|entry| {
                    (entry.qc.subject_block_hash == block_hash
                        && entry.qc.height == block_height
                        && entry.qc.view == block_view
                        && matches!(entry.qc.phase, crate::sumeragi::consensus::Phase::Commit))
                    .then_some(entry.qc.epoch)
                });
        if authoritative_owner != Some(block_hash) && deferred_commit_qc_epoch.is_none() {
            return false;
        }

        let payload_bytes = super::proposals::block_payload_bytes(block);
        let payload_hash = Hash::new(&payload_bytes);
        let mut pending = PendingBlock::new(block.clone(), payload_hash, block_height, block_view);
        if let Some(epoch) = observed_commit_qc_epoch.or(deferred_commit_qc_epoch) {
            pending.note_commit_qc_observed(epoch);
        }

        self.pending.pending_blocks.insert(block_hash, pending);
        self.deferred_block_sync_updates
            .remove(&(block_height, block_view, block_hash));
        self.flush_frontier_body_requesters(block);
        self.flush_pending_fetch_requests(block);
        self.clear_missing_block_request(&block_hash, MissingBlockClearReason::PayloadAvailable);
        self.clear_missing_block_view_change(&block_hash);
        info!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            commit_qc_observed = observed_commit_qc_epoch
                .or(deferred_commit_qc_epoch)
                .is_some(),
            "materialized frontier block-sync payload for deferred QC recovery"
        );
        true
    }

    fn prepare_known_block_qc_work(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        block: Arc<SignedBlock>,
        topology: super::network_topology::Topology,
        stake_snapshot: Option<CommitStakeSnapshot>,
        consensus_mode: ConsensusMode,
        mode_tag: &'static str,
        prf_seed: Option<[u8; 32]>,
        commit_qc_match: bool,
    ) -> Option<KnownBlockQcWork> {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        if topology.as_ref().is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                block_height,
                block_view,
                Some(block_hash),
                self.queue.queued_len(),
                Instant::now(),
                super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "prepare_known_block_qc_work_empty_commit_topology",
            );
            warn!(
                height = block_height,
                view = block_view,
                "dropping block sync QC: empty commit topology"
            );
            return None;
        }
        if qc.subject_block_hash != block_hash {
            warn!(
                incoming_hash = %block_hash,
                qc_hash = %qc.subject_block_hash,
                "ignoring block sync QC that does not match block hash"
            );
            return None;
        }
        if qc.height != block_height {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                qc_height = qc.height,
                "ignoring block sync QC that does not match block height"
            );
            return None;
        }
        let expected_epoch = self.epoch_for_height(block_height);
        if qc.epoch != expected_epoch {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                expected_epoch,
                qc_epoch = qc.epoch,
                "ignoring block sync QC with mismatched epoch"
            );
            return None;
        }
        if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            warn!(
                incoming_hash = %block_hash,
                phase = ?qc.phase,
                "ignoring block sync QC with non-commit phase"
            );
            return None;
        }
        if let Some(lock) = self.locked_qc {
            let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, &qc);
            if same_height_conflict {
                if self.defer_block_sync_qc_while_locked_payload_missing(
                    &qc,
                    "known_block_qc.height_conflict.missing_locked_payload",
                ) {
                    return None;
                }
                if !Self::block_sync_qc_same_height_recoverable(lock, &qc, true) {
                    crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                    self.log_block_sync_locked_qc_conflict(
                        &qc,
                        lock,
                        "known_block_qc.height_conflict",
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
                    );
                    return None;
                }
            }
        }
        if self.block_sync_qc_is_stale_against_lock(&qc) {
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                locked_height = self.locked_qc.map(|lock| lock.height),
                "dropping stale known-block QC below locked height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return None;
        }
        if !self.block_sync_qc_extends_locked_chain(&qc) {
            if self.defer_block_sync_qc_while_locked_payload_missing(
                &qc,
                "known_block_qc.non_extending.missing_locked_payload",
            ) {
                return None;
            }
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                "retaining known-block non-extending QC for lock realignment"
            );
        }
        Some(KnownBlockQcWork {
            qc,
            block,
            topology,
            stake_snapshot,
            consensus_mode,
            mode_tag,
            prf_seed,
            commit_qc_match,
            aggregate_ok: None,
        })
    }

    fn enqueue_known_block_qc_work(&mut self, work: KnownBlockQcWork) {
        let key = Self::qc_tally_key(&work.qc);
        if self.known_block_qc_work.contains_key(&key) {
            debug!(
                phase = ?work.qc.phase,
                height = work.qc.height,
                view = work.qc.view,
                block = %work.qc.subject_block_hash,
                "dropping duplicate known-block QC work item"
            );
            return;
        }
        self.known_block_qc_work.insert(key, work);
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::Qc,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::AggregateVerifyDeferred,
        );
        debug!(
            height = key.2,
            view = key.3,
            block = %key.1,
            queued = self.known_block_qc_work.len(),
            "deferred known-block QC processing off payload queue"
        );
        if let Some(wake) = self.wake_tx.as_ref() {
            let _ = wake.try_send(());
        }
    }

    pub(super) fn drain_known_block_qc_work(&mut self, tick_deadline: Option<Instant>) -> bool {
        if self.known_block_qc_work.is_empty() {
            return false;
        }
        let mut progress = false;
        let mut processed = 0usize;
        while processed < KNOWN_BLOCK_QC_WORK_PER_TICK {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
            let key = match self.known_block_qc_work.keys().next().cloned() {
                Some(key) => key,
                None => break,
            };
            let Some(work) = self.known_block_qc_work.remove(&key) else {
                continue;
            };
            if self.apply_known_block_qc_work(work) {
                progress = true;
            }
            processed = processed.saturating_add(1);
        }
        if processed > 0 {
            debug!(
                processed,
                remaining = self.known_block_qc_work.len(),
                "drained known-block QC work items"
            );
        }
        progress
    }

    fn dispatch_known_block_qc_verify(
        &mut self,
        work: KnownBlockQcWork,
    ) -> Option<KnownBlockQcWork> {
        if self.subsystems.qc_verify.work_txs.is_empty() {
            return Some(work);
        }
        let canonical_roster = super::roster::canonicalize_roster(work.topology.as_ref().to_vec());
        let canonical_topology = super::network_topology::Topology::new(canonical_roster);
        let Some(inputs) = super::qc_aggregate_inputs(
            &work.qc,
            &canonical_topology,
            &self.roster_validation_cache.pops,
            &self.common_config.chain,
            work.mode_tag,
        ) else {
            return Some(work);
        };
        let key = super::QcVerifyKey::from_qc(&work.qc);
        if self.subsystems.qc_verify.inflight.contains_key(&key) {
            debug!(
                height = work.qc.height,
                view = work.qc.view,
                phase = ?work.qc.phase,
                block = %work.qc.subject_block_hash,
                "known-block QC verify already in flight"
            );
            return None;
        }
        let id = self.subsystems.qc_verify.next_id();
        let mut verify_work = super::qc_verify::QcVerifyWork {
            id,
            key: key.clone(),
            inputs,
        };
        let mut dispatched = false;
        let mut disconnected = Vec::new();
        let total = self.subsystems.qc_verify.work_txs.len();
        for _ in 0..total {
            let idx = self.subsystems.qc_verify.next_worker % total;
            self.subsystems.qc_verify.next_worker =
                self.subsystems.qc_verify.next_worker.saturating_add(1);
            let work_tx = &self.subsystems.qc_verify.work_txs[idx];
            match work_tx.try_send(verify_work) {
                Ok(()) => {
                    dispatched = true;
                    break;
                }
                Err(std::sync::mpsc::TrySendError::Full(returned)) => {
                    verify_work = returned;
                }
                Err(std::sync::mpsc::TrySendError::Disconnected(returned)) => {
                    verify_work = returned;
                    disconnected.push(idx);
                }
            }
        }
        if !disconnected.is_empty() {
            disconnected.sort_unstable();
            disconnected.dedup();
            for idx in disconnected.into_iter().rev() {
                if idx < self.subsystems.qc_verify.work_txs.len() {
                    self.subsystems.qc_verify.work_txs.swap_remove(idx);
                }
            }
            if self.subsystems.qc_verify.next_worker >= self.subsystems.qc_verify.work_txs.len() {
                self.subsystems.qc_verify.next_worker = 0;
            }
        }
        if dispatched {
            self.subsystems.qc_verify.inflight.insert(
                key,
                super::QcVerifyInFlight {
                    id,
                    target: super::QcVerifyTarget::KnownBlock(work),
                },
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Deferred,
                super::status::ConsensusMessageReason::AggregateVerifyDeferred,
            );
            return None;
        }
        if self.subsystems.qc_verify.work_txs.is_empty() {
            warn!(
                height = work.qc.height,
                view = work.qc.view,
                phase = ?work.qc.phase,
                block = %work.qc.subject_block_hash,
                "QC verify workers unavailable for known-block QC; running aggregate verification inline"
            );
            self.subsystems.qc_verify.result_rx = None;
            self.subsystems.qc_verify.inflight.clear();
        } else {
            warn!(
                height = work.qc.height,
                view = work.qc.view,
                phase = ?work.qc.phase,
                block = %work.qc.subject_block_hash,
                "QC verify worker queue full for known-block QC; running aggregate verification inline"
            );
        }
        Some(work)
    }

    pub(super) fn apply_known_block_qc_work(&mut self, work: KnownBlockQcWork) -> bool {
        if self.block_sync_qc_is_stale_against_lock(&work.qc) {
            debug!(
                height = work.qc.height,
                view = work.qc.view,
                incoming_hash = %work.qc.subject_block_hash,
                locked_height = self.locked_qc.map(|lock| lock.height),
                "dropping stale known-block QC below locked height before verify dispatch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return true;
        }
        let cached_qc_match = cached_qc_for(
            &self.qc_cache,
            work.qc.phase,
            work.qc.subject_block_hash,
            work.qc.height,
            work.qc.view,
            work.qc.epoch,
        )
        .filter(|cached| HashOf::new(cached) == HashOf::new(&work.qc));
        let cached_tally = if cached_qc_match.is_some() || work.commit_qc_match {
            self.qc_signer_tally
                .get(&Self::qc_tally_key(&work.qc))
                .cloned()
        } else {
            None
        };
        let mut work = work;
        if cached_tally.is_none() && work.aggregate_ok.is_none() {
            if let Some(pending) = self.dispatch_known_block_qc_verify(work) {
                work = pending;
            } else {
                return false;
            }
        }
        let KnownBlockQcWork {
            qc,
            block,
            topology,
            stake_snapshot,
            consensus_mode,
            mode_tag,
            prf_seed,
            commit_qc_match: _,
            aggregate_ok,
        } = work;
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        let qc_signers = qc_signer_count(&qc);
        let tally = if let Some(tally) = cached_tally {
            Ok(tally)
        } else {
            let signer_cache_key =
                BlockSignerCacheKey::new(block_hash, topology.as_ref(), consensus_mode, prf_seed);
            let cached_block_signers = signer_cache_key
                .as_ref()
                .and_then(|key| self.block_signer_cache.get(key));
            let block_signers = if let Some(signers) = cached_block_signers {
                signers
            } else {
                let block_signers = {
                    let world_view = self.state.world_view();
                    validated_block_signers_from_world(
                        &block,
                        &topology,
                        &world_view,
                        mode_tag,
                        prf_seed,
                    )
                };
                match block_signers {
                    Ok(signers) => {
                        if let Some(key) = signer_cache_key {
                            self.block_signer_cache.insert(key, signers.clone());
                        }
                        signers
                    }
                    Err(err) => {
                        warn!(
                            ?err,
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            "block sync QC received for known block with invalid signatures; proceeding without signer subset check"
                        );
                        BTreeSet::new()
                    }
                }
            };
            let world_view = self.state.world_view();
            tally_qc_against_block_signers(
                &qc,
                &topology,
                &world_view,
                &block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
                aggregate_ok,
            )
        };
        let tally = match tally {
            Ok(tally) => tally,
            Err(err) => {
                let reason = qc_validation_reason(&err);
                if Self::block_sync_qc_is_missing_context_error(&err) {
                    self.quarantine_block_sync_qc_candidate(
                        qc.clone(),
                        reason,
                        QuarantinedQcTarget::BlockSync,
                    );
                } else {
                    self.block_sync_qc_final_drop(reason);
                }
                warn!(
                    ?err,
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    qc_signers,
                    "dropping block sync QC: tally validation failed"
                );
                return false;
            }
        };
        crate::sumeragi::status::record_precommit_signers(
            crate::sumeragi::status::PrecommitSignerRecord {
                block_hash,
                height: qc.height,
                view: qc.view,
                epoch: qc.epoch,
                parent_state_root: qc.parent_state_root,
                post_state_root: qc.post_state_root,
                signers: tally.voting_signers.clone(),
                bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
                roster_len: topology.as_ref().len(),
                mode_tag: mode_tag.to_string(),
                validator_set: topology.as_ref().to_vec(),
                stake_snapshot: stake_snapshot.clone(),
            },
        );
        self.note_validated_qc_tally(&qc, tally.clone());
        let block_known_for_commit =
            self.pending
                .pending_blocks
                .get(&block_hash)
                .is_some_and(|pending| {
                    !pending.is_retry_aborted()
                        && pending.validation_status == ValidationStatus::Valid
                })
                || self
                    .subsystems
                    .commit
                    .inflight
                    .as_ref()
                    .is_some_and(|inflight| {
                        inflight.block_hash == block_hash && !inflight.pending.aborted
                    })
                || self.kura.get_block_height_by_hash(block_hash).is_some();
        let process_ok = self.process_precommit_qc(&qc, block_known_for_commit, true);
        if !process_ok {
            if self.block_sync_qc_is_stale_against_lock(&qc) {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    incoming_hash = %qc.subject_block_hash,
                    locked_height = self.locked_qc.map(|lock| lock.height),
                    "dropping stale block sync QC below locked height"
                );
            } else if let Some(lock) = self.locked_qc {
                if Self::block_sync_qc_same_height_conflict(lock, &qc) {
                    crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                }
                self.log_block_sync_locked_qc_conflict(
                    &qc,
                    lock,
                    "known_block_qc.apply.precommit_reject",
                );
            }
            return true;
        }
        let checkpoint = ValidatorSetCheckpoint::new(
            qc.height,
            qc.view,
            qc.subject_block_hash,
            qc.parent_state_root,
            qc.post_state_root,
            qc.validator_set.clone(),
            qc.aggregate.signers_bitmap.clone(),
            qc.aggregate.bls_aggregate_signature.clone(),
            qc.validator_set_hash_version,
            None,
        );
        if self
            .state
            .record_commit_roster(&qc, &checkpoint, stake_snapshot.clone())
        {
            debug!(
                incoming_hash = %block_hash,
                height = block_height,
                view = block_view,
                "recorded commit roster from block sync QC"
            );
        }
        let qc_key = Self::qc_tally_key(&qc);
        self.deferred_missing_payload_qcs.remove(&qc_key);
        self.quarantined_block_sync_qcs.remove(&qc_key);
        super::status::record_commit_qc(qc.clone());
        self.qc_cache.insert(
            (
                qc.phase,
                qc.subject_block_hash,
                qc.height,
                qc.view,
                qc.epoch,
            ),
            qc.clone(),
        );
        self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
        debug!(
            incoming_hash = %block_hash,
            signers = tally.voting_signers.len(),
            qc_signers,
            "applied block sync QC for known block"
        );
        if block_known_for_commit {
            self.apply_commit_qc(&qc, topology.as_ref(), block_hash, block_height, block_view);
            self.request_commit_pipeline_for_round(
                block_height,
                block_view,
                super::status::RoundPhaseTrace::WaitCommitQc,
                super::status::RoundEventCauseTrace::BlockSyncUpdated,
                None,
            );
        } else {
            debug!(
                incoming_hash = %block_hash,
                height = block_height,
                view = block_view,
                "deferring commit apply for block sync QC until block is validated"
            );
        }
        true
    }
}

#[cfg(test)]
mod allow_uncertified_block_sync_roster_tests {
    use super::allow_uncertified_block_sync_roster;

    #[test]
    fn rejects_next_height_without_explicit_request() {
        assert!(!allow_uncertified_block_sync_roster(11, 10, false));
        assert!(!allow_uncertified_block_sync_roster(12, 10, false));
    }

    #[test]
    fn allows_any_height_when_missing_block_is_requested() {
        assert!(allow_uncertified_block_sync_roster(25, 10, true));
    }
}
