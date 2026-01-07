//! Pending-block rescheduling and quorum timeout handling.

use iroha_logger::prelude::*;

use super::*;

impl Actor {
    #[allow(clippy::too_many_lines)]
    pub(super) fn reschedule_stale_pending_blocks(&mut self) -> bool {
        if self.active_pending_blocks_len() == 0 {
            return false;
        }

        let reschedule_start = Instant::now();
        let commit_roster = self.effective_commit_topology();
        if commit_roster.is_empty() {
            return false;
        }
        let commit_topology = super::network_topology::Topology::new(commit_roster);

        let roster_len = commit_topology.as_ref().len();
        let local_peer_id = self.common_config.peer.id().clone();
        let min_votes_commit = commit_topology.min_votes_for_commit();
        let min_votes = min_votes_commit;
        let da_enabled = self.runtime_da_enabled();
        let quorum_timeout = self.quorum_timeout(da_enabled);
        let quorum_reschedule_cooldown = quorum_timeout.max(QUORUM_RESCHEDULE_COOLDOWN);
        let now = Instant::now();
        let (tip_height, tip_hash) = {
            let view = self.state.view();
            (view.height(), view.latest_block_hash())
        };

        let mut stale_pending = Vec::new();
        let mut to_reschedule = Vec::new();
        let mut prevote_timeouts = Vec::new();
        let mut reschedule_backoff_skipped = 0usize;
        let mut stale_removed = 0usize;
        for (hash, pending) in &self.pending.pending_blocks {
            if pending.aborted {
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
            let pending_age = pending.age();
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
                prevote_timeouts.push((key, pending_age, qc_any));
                continue;
            }
            let (vote_count, quorum_reached) =
                if pending.commit_qc_seen || commit_qc_cached {
                (0, true)
            } else {
                self.commit_vote_quorum_status_for_block(*hash, pending.height, pending.view)
            };
            if missing_quorum_stale(pending_age, quorum_timeout, quorum_reached) {
                if !pending.reschedule_due(now, quorum_reschedule_cooldown) {
                    reschedule_backoff_skipped = reschedule_backoff_skipped.saturating_add(1);
                    continue;
                }
                to_reschedule.push((key, pending_age, vote_count));
            }
        }
        let scan_done = Instant::now();

        let to_reschedule_len = to_reschedule.len();
        let prevote_timeout_len = prevote_timeouts.len();

        for (hash, height) in stale_pending {
            self.pending.pending_blocks.remove(&hash);
            self.clean_rbc_sessions_for_block(hash, height);
            self.qc_cache
                .retain(|(_, qc_hash, _, _, _), _| qc_hash != &hash);
            self.qc_signer_tally
                .retain(|(_, qc_hash, _, _, _), _| qc_hash != &hash);
            stale_removed = stale_removed.saturating_add(1);
        }

        let mut progress = false;
        for (key, age, vote_count) in to_reschedule {
            if let Some(pending) = self.pending.pending_blocks.remove(&key.0) {
                self.reschedule_pending_quorum_block(
                    pending,
                    age,
                    min_votes,
                    vote_count,
                    quorum_timeout,
                    quorum_reschedule_cooldown,
                    now,
                );
                self.trigger_view_change_with_cause(
                    key.1,
                    key.2,
                    view_change_cause_for_quorum(vote_count),
                );
                progress = true;
            }
        }

        for (key, pending_age, qc) in prevote_timeouts {
            if let Some(pending) = self.pending.pending_blocks.remove(&key.0) {
                let vote_count = qc
                    .as_ref()
                    .map_or(0, |qc| qc_voting_signer_count(qc, roster_len));
                let txs: Vec<SignedTransaction> = pending.block.transactions_vec().clone();
                let (requeued, failures, _duplicate_failures, _gossip_hashes) =
                    requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
                let msg = BlockMessage::BlockCreated(super::message::BlockCreated {
                    block: pending.block.clone(),
                });
                for peer in commit_topology.as_ref() {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: msg.clone(),
                    });
                }
                if let Some(qc) = qc {
                    let msg = BlockMessage::Qc(qc.clone());
                    for peer in commit_topology.as_ref() {
                        if peer == &local_peer_id {
                            continue;
                        }
                        self.schedule_background(BackgroundRequest::Post {
                            peer: peer.clone(),
                            msg: msg.clone(),
                        });
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
                warn!(
                    block = %key.0,
                    height = key.1,
                    view = key.2,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = quorum_timeout.as_millis(),
                    vote_count,
                    requeued,
                    failures,
                    "prevote quorum stalled; rebroadcasting and rotating view"
                );
                self.trigger_view_change_with_cause(
                    key.1,
                    key.2,
                    view_change_cause_for_quorum(vote_count),
                );
                progress = true;
            }
        }

        let scan_cost = scan_done.saturating_duration_since(reschedule_start);
        let total_cost = reschedule_start.elapsed();
        if total_cost >= RESCHEDULE_TIMING_LOG_THRESHOLD
            || progress
            || reschedule_backoff_skipped > 0
        {
            iroha_logger::info!(
                pending = self.pending.pending_blocks.len(),
                rescheduled = to_reschedule_len,
                prevote_timeouts = prevote_timeout_len,
                stale_removed,
                backoff_skipped = reschedule_backoff_skipped,
                scan_ms = scan_cost.as_millis(),
                total_ms = total_cost.as_millis(),
                "reschedule sweep timing"
            );
        }

        progress
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn reschedule_pending_quorum_block(
        &mut self,
        mut pending: PendingBlock,
        pending_age: Duration,
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
        let _queue_depth = self.queue.tx_len();
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
        let last_reschedule_ms = pending
            .last_quorum_reschedule
            .map(|ts| now.saturating_duration_since(ts).as_millis());
        let drop_pending = already_rescheduled && reschedule_vote_count < min_votes_for_commit;
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
        let topology_peers = self.effective_commit_topology();
        let local_peer_id = self.common_config.peer.id().clone();
        let rebroadcast = self.rebroadcast_pending_block_updates(
            &pending,
            block_hash,
            height,
            view,
            drop_pending,
            &topology_peers,
            &local_peer_id,
        );

        if drop_pending {
            if !keep_commit_qc {
                self.clean_rbc_sessions_for_block(block_hash, height);
            }
            self.qc_cache
                .retain(|(phase, qc_hash, _, _, _), _| {
                    *qc_hash != block_hash
                        || (keep_commit_qc
                            && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
                });
            self.qc_signer_tally
                .retain(|(phase, qc_hash, _, _, _), _| {
                    *qc_hash != block_hash
                        || (keep_commit_qc
                            && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
                });
        } else {
            // Keep the pending block and cached certificates so late commit certificates
            // can still finalize it.
            // We only requeue the transactions to allow a new view to assemble a fresh proposal.
            self.pending.pending_blocks.insert(block_hash, pending);
        }

        warn!(
            ?block_hash,
            height,
            view,
            pending_age_ms = pending_age.as_millis(),
            quorum_timeout_ms = quorum_timeout.as_millis(),
            votes = vote_count,
            min_votes = min_votes_for_commit,
            requeued,
            failures,
            rebroadcasted_votes = rebroadcast.votes,
            rebroadcasted_block = rebroadcast.block,
            rebroadcasted_block_sync = rebroadcast.block_sync,
            rebroadcasted_hint = rebroadcast.hint,
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
        pending: &PendingBlock,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        drop_pending: bool,
        topology_peers: &[PeerId],
        local_peer_id: &PeerId,
    ) -> RescheduleRebroadcast {
        let votes = self.rebroadcast_block_votes(
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
        );
        let block_sync = if drop_pending || topology_peers.is_empty() {
            false
        } else {
            let mut update = block_sync_update_with_roster(
                &pending.block,
                self.state.as_ref(),
                self.kura.as_ref(),
                self.config.consensus_mode,
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
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
            );
            for peer in topology_peers {
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessage::BlockSyncUpdate(update.clone()),
                });
            }
            true
        };
        let hint = if drop_pending {
            false
        } else {
            self.subsystems
                .propose
                .proposal_cache
                .get_hint(height, view)
                .copied()
                .is_some_and(|hint| {
                    for peer in topology_peers {
                        if peer == local_peer_id {
                            continue;
                        }
                        self.schedule_background(BackgroundRequest::Post {
                            peer: peer.clone(),
                            msg: BlockMessage::ProposalHint(hint),
                        });
                    }
                    true
                })
        };
        // Keep and rebroadcast the pending block so late payload requests can still succeed while
        // allowing a fresh proposal to be assembled from the requeued transactions.
        let block = if drop_pending {
            false
        } else {
            let msg = BlockMessage::BlockCreated(super::message::BlockCreated {
                block: pending.block.clone(),
            });
            for peer in topology_peers {
                if peer == local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: msg.clone(),
                });
            }
            true
        };
        RescheduleRebroadcast {
            votes,
            block_sync,
            hint,
            block,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RescheduleRebroadcast {
    votes: usize,
    block_sync: bool,
    hint: bool,
    block: bool,
}
