//! Pending-block rescheduling and quorum timeout handling.

use iroha_logger::prelude::*;

use super::*;

fn adaptive_quorum_reschedule_backoff(
    base_backoff: Duration,
    quorum_stall_age: Duration,
    quorum_timeout: Duration,
    vote_count: usize,
    min_votes_for_commit: usize,
) -> (Duration, bool) {
    if base_backoff == Duration::ZERO {
        return (Duration::ZERO, false);
    }

    let vote_deficit = min_votes_for_commit.saturating_sub(vote_count);
    let mut multiplier = if vote_deficit >= min_votes_for_commit.saturating_sub(1) {
        3
    } else if vote_deficit > 0 {
        2
    } else {
        1
    };
    let mut escalated = false;
    if quorum_timeout != Duration::ZERO {
        let severe_stall = super::saturating_mul_duration(quorum_timeout, 4);
        let moderate_stall = super::saturating_mul_duration(quorum_timeout, 2);
        if quorum_stall_age >= severe_stall {
            multiplier = multiplier.max(5);
            escalated = true;
        } else if quorum_stall_age >= moderate_stall {
            multiplier = multiplier.max(4);
            escalated = true;
        }
    }

    (
        super::saturating_mul_duration(base_backoff, multiplier),
        escalated,
    )
}

impl Actor {
    pub(super) fn rbc_availability_unresolved_for_reschedule(
        &self,
        key: super::rbc_store::SessionKey,
        commit_topology: &super::network_topology::Topology,
        pending_age: Duration,
        availability_timeout: Duration,
    ) -> bool {
        if !self.runtime_da_enabled() {
            return false;
        }
        // After the availability timeout, allow reschedules even if RBC is still incomplete.
        if availability_timeout != Duration::ZERO && pending_age >= availability_timeout {
            return false;
        }
        if self.block_payload_available_locally(key.0) {
            return false;
        }
        if self.subsystems.da_rbc.rbc.pending.contains_key(&key) {
            return true;
        }
        let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) else {
            return false;
        };
        if session.is_invalid() {
            return false;
        }
        if session.delivered {
            return false;
        }
        let missing_chunks =
            session.total_chunks() != 0 && session.received_chunks() < session.total_chunks();
        let required = self.rbc_deliver_quorum(commit_topology);
        let ready_quorum = session.ready_signatures.len() >= required;
        missing_chunks || !ready_quorum
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn reschedule_stale_pending_blocks(
        &mut self,
        tick_deadline: Option<Instant>,
    ) -> bool {
        if self.pending.pending_blocks.is_empty() {
            return false;
        }
        // Allow pruning aborted payloads even when no active pending blocks remain.
        let has_aborted = self
            .pending
            .pending_blocks
            .values()
            .any(|pending| pending.aborted);
        if !has_aborted && self.active_pending_blocks_len() == 0 {
            return false;
        }

        let reschedule_start = Instant::now();
        let mut budget_exhausted = false;
        let mut active_roster: Option<Vec<PeerId>> = None;
        let local_peer_id = self.common_config.peer.id().clone();
        let da_enabled = self.runtime_da_enabled();
        let quorum_timeout = self.quorum_timeout(da_enabled);
        let quorum_reschedule_cooldown = quorum_timeout.max(QUORUM_RESCHEDULE_COOLDOWN);
        let availability_timeout = self.availability_timeout(quorum_timeout, da_enabled);
        // Keep aborted payloads long enough for missing-block fetches after reschedule drops.
        let retention_factor = self
            .config
            .recovery
            .missing_block_signer_fallback_attempts
            .saturating_add(2)
            .max(4);
        let aborted_retention = quorum_reschedule_cooldown.saturating_mul(retention_factor);
        let queue_depths = super::status::worker_queue_depth_snapshot();
        let now = Instant::now();
        let relay_backpressure = self.relay_backpressure_active(now, self.rebroadcast_cooldown());
        let (tip_height, tip_hash, fast_timeout_permissioned, fast_timeout_npos) = {
            let view = self.state.view();
            (
                view.height(),
                view.latest_block_hash(),
                self.pending_fast_path_timeout(&view, ConsensusMode::Permissioned),
                self.pending_fast_path_timeout(&view, ConsensusMode::Npos),
            )
        };

        let mut stale_pending = Vec::new();
        let mut aborted_expired = Vec::new();
        let mut to_reschedule = Vec::new();
        let mut prevote_timeouts = Vec::new();
        let mut reschedule_backoff_skipped = 0usize;
        let mut missing_data_backoff_skipped = 0usize;
        let mut quorum_stall_escalations = 0usize;
        let mut stale_removed = 0usize;
        let mut aborted_removed = 0usize;
        for (hash, pending) in &self.pending.pending_blocks {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            if pending.aborted {
                if self.kura.get_block_height_by_hash(*hash).is_some() {
                    aborted_expired.push((*hash, pending.height, pending.view));
                    continue;
                }
                let has_votes = self.vote_log.values().any(|vote| {
                    vote.block_hash == *hash
                        && vote.height == pending.height
                        && vote.view == pending.view
                });
                let missing_request = self.pending.missing_block_requests.contains_key(hash);
                let expected_epoch = self.epoch_for_height(pending.height);
                let commit_qc_cached = cached_qc_for(
                    &self.qc_cache,
                    crate::sumeragi::consensus::Phase::Commit,
                    *hash,
                    pending.height,
                    pending.view,
                    expected_epoch,
                )
                .is_some();
                if has_votes || missing_request || commit_qc_cached {
                    continue;
                }
                if pending.age() >= aborted_retention {
                    aborted_expired.push((*hash, pending.height, pending.view));
                }
                continue;
            }
            if self.kura.get_block_height_by_hash(*hash).is_some() {
                info!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    "dropping pending block already committed in kura"
                );
                stale_pending.push((*hash, pending.height));
                continue;
            }
            if !pending_extends_tip(
                pending.height,
                pending.block.header().prev_block_hash(),
                tip_height,
                tip_hash,
            ) {
                continue;
            }
            let (consensus_mode, _, _) = self.consensus_context_for_height(pending.height);
            let pending_age = pending.age();
            let fast_timeout = match consensus_mode {
                ConsensusMode::Permissioned => fast_timeout_permissioned,
                ConsensusMode::Npos => fast_timeout_npos,
            };
            if pending_age < fast_timeout {
                continue;
            }
            let mut commit_roster =
                self.roster_for_vote_with_mode(*hash, pending.height, pending.view, consensus_mode);
            if commit_roster.is_empty() {
                let fallback =
                    active_roster.get_or_insert_with(|| self.effective_commit_topology());
                commit_roster.clone_from(fallback);
            }
            if commit_roster.is_empty() {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    "skipping reschedule: empty commit roster"
                );
                continue;
            }
            let commit_topology = super::network_topology::Topology::new(commit_roster.clone());
            let min_votes_for_commit = commit_topology.min_votes_for_commit();

            let key = (*hash, pending.height, pending.view);
            let expected_epoch = self.epoch_for_height(pending.height);
            let qc_precommit = cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                *hash,
                pending.height,
                pending.view,
                expected_epoch,
            );
            let commit_qc_cached = qc_precommit.is_some();
            let qc_any = qc_precommit.clone().or_else(|| {
                cached_qc_for(
                    &self.qc_cache,
                    crate::sumeragi::consensus::Phase::Prepare,
                    *hash,
                    pending.height,
                    pending.view,
                    expected_epoch,
                )
            });
            let qc_phase = qc_any.as_ref().map(|qc| qc.phase);
            if prevote_quorum_stale(qc_phase, pending_age, quorum_timeout) {
                prevote_timeouts.push((key, pending_age, qc_any, commit_roster));
                continue;
            }
            let (vote_count, quorum_reached, stake_quorum_missing) =
                if pending.commit_qc_seen || commit_qc_cached {
                    (0, true, false)
                } else {
                    let status = self.commit_vote_quorum_status_for_block_detail(
                        *hash,
                        pending.height,
                        pending.view,
                    );
                    (
                        status.vote_count,
                        status.quorum_reached,
                        status.stake_quorum_missing,
                    )
                };
            let has_qc = pending.commit_qc_seen || commit_qc_cached || qc_any.is_some();
            let validation_inflight = pending.validation_status == ValidationStatus::Pending
                && self.subsystems.validation.inflight.contains_key(hash);
            let payload_available = da_enabled
                && Self::payload_available_for_da(
                    &self.subsystems.da_rbc.rbc.sessions,
                    &self.subsystems.da_rbc.rbc.status_handle,
                    pending,
                );
            let allow_da_fast_reschedule =
                da_enabled && self.config.pacemaker.da_fast_reschedule && payload_available;
            let has_votes = vote_count > 0;
            let fast_path_allowed = (!da_enabled || allow_da_fast_reschedule)
                && !has_votes
                && !has_qc
                && !validation_inflight;
            let effective_quorum_timeout = if fast_path_allowed {
                fast_timeout.min(quorum_timeout)
            } else {
                quorum_timeout
            };
            let quorum_stall_age =
                if (has_votes || has_qc) && pending.last_quorum_reschedule.is_some() {
                    pending.progress_age(now)
                } else {
                    pending_age
                };
            if missing_quorum_stale(quorum_stall_age, effective_quorum_timeout, quorum_reached) {
                let rbc_key = (*hash, pending.height, pending.view);
                if queue_depths.vote_rx > 0 {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        pending_age_ms = pending_age.as_millis(),
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        vote_rx_depth = queue_depths.vote_rx,
                        "deferring quorum reschedule while vote queue is backlogged"
                    );
                    continue;
                }
                let rbc_session_incomplete = da_enabled
                    && self
                        .subsystems
                        .da_rbc
                        .rbc
                        .sessions
                        .get(&rbc_key)
                        .is_some_and(|session| {
                            if session.is_invalid() || session.delivered {
                                return false;
                            }
                            let missing_chunks = session.total_chunks() != 0
                                && session.received_chunks() < session.total_chunks();
                            let ready_quorum = session.ready_signatures.len()
                                >= self.rbc_deliver_quorum(&commit_topology);
                            missing_chunks || !ready_quorum
                        });
                if rbc_session_incomplete && pending_age < availability_timeout {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        pending_age_ms = pending_age.as_millis(),
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        "deferring quorum reschedule while RBC session is incomplete"
                    );
                    continue;
                }
                if self.rbc_availability_unresolved_for_reschedule(
                    rbc_key,
                    &commit_topology,
                    pending_age,
                    availability_timeout,
                ) {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        pending_age_ms = pending_age.as_millis(),
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        "deferring quorum reschedule while RBC availability is unresolved"
                    );
                    continue;
                }
                let missing_local_data = da_enabled && !payload_available;
                if (missing_local_data
                    || matches!(pending.last_gate, Some(GateReason::MissingLocalData)))
                    && pending_age < availability_timeout
                {
                    missing_data_backoff_skipped = missing_data_backoff_skipped.saturating_add(1);
                    continue;
                }
                let (effective_reschedule_backoff, stall_escalated) =
                    adaptive_quorum_reschedule_backoff(
                        quorum_reschedule_cooldown,
                        quorum_stall_age,
                        effective_quorum_timeout,
                        vote_count,
                        min_votes_for_commit,
                    );
                if stall_escalated {
                    quorum_stall_escalations = quorum_stall_escalations.saturating_add(1);
                    super::status::inc_quorum_stall_age_escalation();
                }
                if !pending.reschedule_due(now, effective_reschedule_backoff) {
                    reschedule_backoff_skipped = reschedule_backoff_skipped.saturating_add(1);
                    continue;
                }
                to_reschedule.push((
                    key,
                    pending_age,
                    quorum_stall_age,
                    vote_count,
                    min_votes_for_commit,
                    stake_quorum_missing,
                    effective_reschedule_backoff,
                ));
            }
        }
        let scan_done = Instant::now();

        let to_reschedule_len = to_reschedule.len();
        let prevote_timeout_len = prevote_timeouts.len();

        for (hash, height, view) in aborted_expired {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            let expected_epoch = self.epoch_for_height(height);
            let keep_commit_qc = cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                hash,
                height,
                view,
                expected_epoch,
            )
            .is_some();
            if !keep_commit_qc {
                self.clean_rbc_sessions_for_block(hash, height);
            }
            self.qc_cache.retain(|(phase, qc_hash, _, _, _), _| {
                *qc_hash != hash
                    || (keep_commit_qc
                        && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
            });
            self.qc_signer_tally.retain(|(phase, qc_hash, _, _, _), _| {
                *qc_hash != hash
                    || (keep_commit_qc
                        && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
            });
            self.block_signer_cache.remove_block(&hash);
            self.pending.pending_blocks.remove(&hash);
            aborted_removed = aborted_removed.saturating_add(1);
        }

        for (hash, height) in stale_pending {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            self.pending.pending_blocks.remove(&hash);
            self.clean_rbc_sessions_for_block(hash, height);
            self.qc_cache
                .retain(|(_, qc_hash, _, _, _), _| qc_hash != &hash);
            self.qc_signer_tally
                .retain(|(_, qc_hash, _, _, _), _| qc_hash != &hash);
            self.block_signer_cache.remove_block(&hash);
            stale_removed = stale_removed.saturating_add(1);
        }

        let mut progress = aborted_removed > 0;
        for (
            key,
            age,
            quorum_stall_age,
            vote_count,
            min_votes,
            stake_quorum_missing,
            effective_reschedule_backoff,
        ) in to_reschedule
        {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            if let Some(pending) = self.pending.pending_blocks.remove(&key.0) {
                self.reschedule_pending_quorum_block(
                    pending,
                    age,
                    quorum_stall_age,
                    min_votes,
                    vote_count,
                    quorum_timeout,
                    effective_reschedule_backoff,
                    now,
                );
                self.trigger_view_change_with_cause(
                    key.1,
                    key.2,
                    view_change_cause_for_quorum(vote_count, stake_quorum_missing),
                );
                progress = true;
            }
        }

        for (key, pending_age, qc, commit_roster) in prevote_timeouts {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            if let Some(pending) = self.pending.pending_blocks.remove(&key.0) {
                let roster_len = commit_roster.len();
                let vote_count = qc
                    .as_ref()
                    .map_or(0, |qc| qc_voting_signer_count(qc, roster_len));
                let txs: Vec<SignedTransaction> = pending.block.transactions_vec().clone();
                let (requeued, failures, _duplicate_failures, _gossip_hashes) =
                    requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
                if relay_backpressure {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        "skipping prevote-timeout rebroadcast due to relay backpressure"
                    );
                } else {
                    let msg = Arc::new(BlockMessage::BlockCreated(super::message::BlockCreated {
                        block: pending.block.clone(),
                    }));
                    let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                    for peer in &commit_roster {
                        if peer == &local_peer_id {
                            continue;
                        }
                        self.schedule_background(BackgroundRequest::Post {
                            peer: peer.clone(),
                            msg: BlockMessageWire::with_encoded(
                                Arc::clone(&msg),
                                Arc::clone(&encoded),
                            ),
                        });
                    }
                    if let Some(qc) = qc {
                        let msg = Arc::new(BlockMessage::Qc(qc.clone()));
                        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                        for peer in &commit_roster {
                            if peer == &local_peer_id {
                                continue;
                            }
                            self.schedule_background(BackgroundRequest::Post {
                                peer: peer.clone(),
                                msg: BlockMessageWire::with_encoded(
                                    Arc::clone(&msg),
                                    Arc::clone(&encoded),
                                ),
                            });
                        }
                    }
                }
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_prevote_timeout(self.mode_tag());
                super::status::inc_prevote_timeout();
                self.clean_rbc_sessions_for_block(key.0, key.1);
                self.qc_cache
                    .retain(|(_, qc_hash, _, _, _), _| qc_hash != &key.0);
                self.qc_signer_tally
                    .retain(|(_, qc_hash, _, _, _), _| qc_hash != &key.0);
                self.block_signer_cache.remove_block(&key.0);
                if let Some(highest) = self.highest_qc {
                    if highest.subject_block_hash == key.0
                        && highest.height == key.1
                        && highest.view == key.2
                    {
                        if let Some(committed) = self.latest_committed_qc() {
                            self.highest_qc = Some(committed);
                            super::status::set_highest_qc(committed.height, committed.view);
                            super::status::set_highest_qc_hash(committed.subject_block_hash);
                        } else {
                            self.highest_qc = None;
                            super::status::set_highest_qc(0, 0);
                            super::status::set_highest_qc_hash(
                                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                                    [0; Hash::LENGTH],
                                )),
                            );
                        }
                    }
                }
                let queue_depths = super::status::worker_queue_depth_snapshot();
                warn!(
                    block = %key.0,
                    height = key.1,
                    view = key.2,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = quorum_timeout.as_millis(),
                    vote_count,
                    requeued,
                    failures,
                    vote_rx_depth = queue_depths.vote_rx,
                    block_payload_rx_depth = queue_depths.block_payload_rx,
                    rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                    block_rx_depth = queue_depths.block_rx,
                    consensus_rx_depth = queue_depths.consensus_rx,
                    lane_relay_rx_depth = queue_depths.lane_relay_rx,
                    background_rx_depth = queue_depths.background_rx,
                    "prevote quorum stalled; rebroadcasting and rotating view"
                );
                self.trigger_view_change_with_cause(
                    key.1,
                    key.2,
                    view_change_cause_for_quorum(vote_count, false),
                );
                progress = true;
            }
        }

        let scan_cost = scan_done.saturating_duration_since(reschedule_start);
        let total_cost = reschedule_start.elapsed();
        if total_cost >= RESCHEDULE_TIMING_LOG_THRESHOLD
            || progress
            || reschedule_backoff_skipped > 0
            || missing_data_backoff_skipped > 0
            || aborted_removed > 0
        {
            iroha_logger::info!(
                pending = self.pending.pending_blocks.len(),
                rescheduled = to_reschedule_len,
                prevote_timeouts = prevote_timeout_len,
                stale_removed,
                aborted_removed,
                backoff_skipped = reschedule_backoff_skipped,
                missing_data_skipped = missing_data_backoff_skipped,
                stall_escalations = quorum_stall_escalations,
                budget_exhausted,
                scan_ms = scan_cost.as_millis(),
                total_ms = total_cost.as_millis(),
                "reschedule sweep timing"
            );
        }

        progress
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn reschedule_pending_quorum_block(
        &mut self,
        mut pending: PendingBlock,
        pending_age: Duration,
        quorum_stall_age: Duration,
        min_votes_for_commit: usize,
        vote_count: usize,
        quorum_timeout: Duration,
        reschedule_backoff: Duration,
        now: Instant,
    ) {
        let block_hash = pending.block.hash();
        let height = pending.height;
        let view = pending.view;
        let expected_epoch = self.epoch_for_height(height);
        // Preserve commit QCs so late payloads can still finalize after a drop.
        let keep_commit_qc = cached_qc_for(
            &self.qc_cache,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            expected_epoch,
        )
        .is_some();
        let _queue_depth = self.queue.queued_len();
        let (state_height, tip_hash) = {
            let state_view = self.state.view();
            (state_view.height(), state_view.latest_block_hash())
        };
        let pending_parent = pending.block.header().prev_block_hash();
        if !pending_extends_tip(height, pending_parent, state_height, tip_hash) {
            debug!(
                ?block_hash,
                height,
                view,
                expected_height = u64::try_from(state_height.saturating_add(1))
                    .unwrap_or(u64::MAX),
                tip_hash = ?tip_hash,
                prev_hash = ?pending_parent,
                "skipping quorum reschedule: pending block not on local tip"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return;
        }

        let precommit_vote_count = self
            .vote_log
            .values()
            .filter(|vote| {
                vote.phase == crate::sumeragi::consensus::Phase::Commit
                    && vote.block_hash == block_hash
                    && vote.height == height
                    && vote.view == view
            })
            .count();
        let commit_vote_count = vote_count;
        let reschedule_vote_count = precommit_vote_count.max(commit_vote_count);
        let has_reschedule_votes = reschedule_vote_count > 0;
        let already_rescheduled = pending.last_quorum_reschedule.is_some();
        let progress_age = pending.progress_age(now);
        let last_reschedule_ms = pending
            .last_quorum_reschedule
            .map(|ts| now.saturating_duration_since(ts).as_millis());
        let no_commit_evidence = reschedule_vote_count == 0 && !keep_commit_qc;
        let drop_pending = already_rescheduled
            && no_commit_evidence
            && (quorum_timeout == Duration::ZERO || progress_age >= quorum_timeout);
        let (requeued, failures, _duplicate_failures, _gossip_hashes) =
            if !has_reschedule_votes || drop_pending {
                // Avoid conflicting proposals once votes exist (precommit or commit), unless we've
                // already retried with availability evidence and need to unblock proposal assembly.
                let txs: Vec<SignedTransaction> = pending.block.transactions_vec().clone();
                requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs)
            } else {
                (0, 0, 0, Vec::new())
            };
        pending.mark_quorum_reschedule(now);
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let mut topology_peers =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if topology_peers.is_empty() {
            topology_peers = self.effective_commit_topology();
        }
        let rebroadcast = self.rebroadcast_pending_block_updates(
            &mut pending,
            block_hash,
            height,
            view,
            drop_pending,
            &topology_peers,
            now,
        );

        if drop_pending {
            if !keep_commit_qc {
                self.clean_rbc_sessions_for_block(block_hash, height);
            }
            self.qc_cache.retain(|(phase, qc_hash, _, _, _), _| {
                *qc_hash != block_hash
                    || (keep_commit_qc
                        && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
            });
            self.qc_signer_tally.retain(|(phase, qc_hash, _, _, _), _| {
                *qc_hash != block_hash
                    || (keep_commit_qc
                        && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
            });
            pending.mark_aborted();
            pending.tx_batch = None;
            self.pending.pending_blocks.insert(block_hash, pending);
        } else {
            // Keep the pending block and cached certificates so late commit certificates
            // can still finalize it.
            // We only requeue the transactions to allow a new view to assemble a fresh proposal.
            self.pending.pending_blocks.insert(block_hash, pending);
        }

        let queue_depths = super::status::worker_queue_depth_snapshot();
        warn!(
            ?block_hash,
            height,
            view,
            pending_age_ms = pending_age.as_millis(),
            quorum_stall_age_ms = quorum_stall_age.as_millis(),
            progress_age_ms = progress_age.as_millis(),
            quorum_timeout_ms = quorum_timeout.as_millis(),
            votes = vote_count,
            min_votes = min_votes_for_commit,
            requeued,
            failures,
            vote_rx_depth = queue_depths.vote_rx,
            block_payload_rx_depth = queue_depths.block_payload_rx,
            rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
            block_rx_depth = queue_depths.block_rx,
            consensus_rx_depth = queue_depths.consensus_rx,
            lane_relay_rx_depth = queue_depths.lane_relay_rx,
            background_rx_depth = queue_depths.background_rx,
            rebroadcasted_votes = rebroadcast.votes,
            rebroadcasted_block = rebroadcast.block,
            rebroadcasted_block_sync = rebroadcast.block_sync,
            drop_pending,
            precommit_votes = precommit_vote_count,
            commit_votes = commit_vote_count,
            reschedule_backoff_ms = reschedule_backoff.as_millis(),
            last_reschedule_ms = last_reschedule_ms,
            "commit quorum missing past timeout; rescheduling block for reassembly"
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn rebroadcast_pending_block_updates(
        &mut self,
        pending: &mut PendingBlock,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        drop_pending: bool,
        topology_peers: &[PeerId],
        now: Instant,
    ) -> RescheduleRebroadcast {
        if self.relay_backpressure_active(now, self.rebroadcast_cooldown()) {
            debug!(
                height,
                view,
                block = %block_hash,
                "skipping reschedule rebroadcast due to relay backpressure"
            );
            return RescheduleRebroadcast {
                votes: 0,
                block_sync: false,
                block: false,
            };
        }
        let retransmit_targets = self.quorum_retransmit_targets_for_missing_votes(
            block_hash,
            height,
            view,
            topology_peers,
        );
        let votes = self.rebroadcast_block_votes(
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            true,
        );
        let mut block_sync = false;
        if !drop_pending && !retransmit_targets.is_empty() {
            let mut update = block_sync_update_with_roster(
                &pending.block,
                self.state.as_ref(),
                self.kura.as_ref(),
                self.config.consensus_mode,
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
                &self.roster_validation_cache,
            );
            let expected_epoch = self.epoch_for_height(height);
            Self::apply_cached_qcs_to_block_sync_update(
                &mut update,
                &self.qc_cache,
                &self.vote_log,
                block_hash,
                height,
                view,
                expected_epoch,
                self.state.as_ref(),
                self.config.consensus_mode,
            );
            let commit_votes = update.commit_votes.len();
            let has_commit_qc = update.commit_qc.is_some();
            if pending.should_broadcast_block_sync_update(view, commit_votes, has_commit_qc) {
                let (consensus_mode, _, _) = self.consensus_context_for_height(height);
                if self.prepare_block_sync_update_for_broadcast(&mut update, consensus_mode) {
                    self.broadcast_block_sync_update(update, &retransmit_targets);
                    block_sync = true;
                } else {
                    debug!(
                        height,
                        view,
                        block = %block_hash,
                        "skipping block sync update rebroadcast: no verifiable roster"
                    );
                }
            } else {
                debug!(
                    height,
                    view,
                    block = %block_hash,
                    commit_votes,
                    has_commit_qc,
                    "skipping block sync update rebroadcast: no new commit evidence"
                );
            }
        }
        // Keep and rebroadcast the pending block so late payload requests can still succeed while
        // allowing a fresh proposal to be assembled from the requeued transactions.
        let block = if drop_pending {
            false
        } else if retransmit_targets.is_empty() {
            false
        } else {
            let cooldown = self.payload_rebroadcast_cooldown();
            if self
                .payload_rebroadcast_log
                .allow(block_hash, now, cooldown)
            {
                self.broadcast_block_created_for_block_sync(
                    super::message::BlockCreated::from(&pending.block),
                    &retransmit_targets,
                );
                true
            } else {
                debug!(
                    height,
                    view,
                    block = %block_hash,
                    cooldown_ms = cooldown.as_millis(),
                    "skipping pending block rebroadcast due to cooldown"
                );
                false
            }
        };
        RescheduleRebroadcast {
            votes,
            block_sync,
            block,
        }
    }

    fn quorum_retransmit_targets_for_missing_votes(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        topology_peers: &[PeerId],
    ) -> Vec<PeerId> {
        if topology_peers.is_empty() {
            return Vec::new();
        }
        let local_peer_id = self.common_config.peer.id();
        let observed_signers: std::collections::BTreeSet<usize> = self
            .vote_log
            .values()
            .filter(|vote| {
                vote.phase == crate::sumeragi::consensus::Phase::Commit
                    && vote.block_hash == block_hash
                    && vote.height == height
                    && vote.view == view
            })
            .filter_map(|vote| usize::try_from(vote.signer).ok())
            .collect();

        let mut missing_targets = Vec::new();
        for (idx, peer) in topology_peers.iter().enumerate() {
            if peer == local_peer_id {
                continue;
            }
            if !observed_signers.contains(&idx) {
                missing_targets.push(peer.clone());
            }
        }
        if missing_targets.is_empty() {
            topology_peers
                .iter()
                .filter(|peer| *peer != local_peer_id)
                .cloned()
                .collect()
        } else {
            missing_targets
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RescheduleRebroadcast {
    votes: usize,
    block_sync: bool,
    block: bool,
}
