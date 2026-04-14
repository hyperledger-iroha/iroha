//! Proposal assembly and pacemaker-driven propose path.

use super::proposals::block_payload_bytes;
use super::*;
use crate::smartcontracts::isi::triggers::set::SetReadOnly;
use crate::smartcontracts::isi::triggers::specialized::LoadedActionTrait;
use core::num::{NonZeroU64, NonZeroUsize};
use iroha_data_model::consensus::{
    CommitStakeSnapshot as ModelCommitStakeSnapshot,
    CommitStakeSnapshotEntry as ModelCommitStakeSnapshotEntry, PreviousRosterEvidence,
    ValidatorSetCheckpoint,
};
use iroha_data_model::events::EventFilter;
use iroha_data_model::prelude::Repeats;
pub(super) fn resolve_prev_block_for_proposal(
    proposal_height: u64,
    highest_qc: &crate::sumeragi::consensus::QcHeaderRef,
    kura: &Kura,
    pending_blocks: &BTreeMap<HashOf<BlockHeader>, PendingBlock>,
) -> Option<Arc<SignedBlock>> {
    let mut prev_block = proposal_height.checked_sub(1).and_then(|prev| {
        let prev_height_usize = if let Ok(value) = usize::try_from(prev) {
            value
        } else {
            warn!(
                height = proposal_height,
                "previous height exceeds usize; skipping cached block lookup"
            );
            return None;
        };
        NonZeroUsize::new(prev_height_usize).and_then(|nz| kura.get_block(nz))
    });
    if prev_block.is_none() && proposal_height > 1 {
        if let Some(pending_parent) = pending_blocks
            .get(&highest_qc.subject_block_hash)
            .filter(|pending| pending.height + 1 == proposal_height)
        {
            prev_block = Some(pending_parent.block.clone().into());
        }
    }
    prev_block
}

fn precommit_qc_for_view_change(
    highest_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    committed_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
    let highest_precommit =
        highest_qc.filter(|qc| qc.phase == crate::sumeragi::consensus::Phase::Commit);
    match (highest_precommit, committed_qc) {
        (Some(highest), Some(committed)) => {
            if (highest.height, highest.view) >= (committed.height, committed.view) {
                Some(highest)
            } else {
                Some(committed)
            }
        }
        (Some(highest), None) => Some(highest),
        (None, Some(committed)) => Some(committed),
        (None, None) => None,
    }
}

fn model_stake_snapshot(
    snapshot: crate::sumeragi::stake_snapshot::CommitStakeSnapshot,
) -> ModelCommitStakeSnapshot {
    ModelCommitStakeSnapshot {
        validator_set_hash: snapshot.validator_set_hash,
        entries: snapshot
            .entries
            .into_iter()
            .map(|entry| ModelCommitStakeSnapshotEntry {
                peer_id: entry.peer_id,
                stake: entry.stake,
            })
            .collect(),
    }
}

fn previous_roster_evidence_for_parent(
    state: &State,
    kura: &Kura,
    fallback_consensus_mode: ConsensusMode,
    parent_block: &SignedBlock,
) -> Option<PreviousRosterEvidence> {
    let parent_height = parent_block.header().height().get();
    let parent_hash = parent_block.hash();
    let metadata = crate::block_sync::message::roster_metadata_from_state(
        state,
        kura,
        parent_height,
        parent_hash,
        fallback_consensus_mode,
    )?;
    let checkpoint = metadata.validator_checkpoint.or_else(|| {
        metadata.commit_qc.as_ref().map(|qc| {
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
        })
    })?;
    Some(PreviousRosterEvidence {
        height: parent_height,
        block_hash: parent_hash,
        validator_checkpoint: checkpoint,
        stake_snapshot: metadata.stake_snapshot.map(model_stake_snapshot),
    })
}

fn next_cached_slot_timeout_streak(
    previous: Option<CachedSlotTimeoutTrigger>,
    height: u64,
    view: u64,
) -> u8 {
    previous
        .filter(|last| last.height == height && view > last.view)
        .map_or(0, |last| last.streak.saturating_add(1))
}

fn cached_slot_timeout_hysteresis_remaining(
    mode: ConsensusMode,
    quorum_timeout: Duration,
    previous: Option<CachedSlotTimeoutTrigger>,
    height: u64,
    view: u64,
    now: Instant,
) -> Option<Duration> {
    if !matches!(mode, ConsensusMode::Npos) || quorum_timeout == Duration::ZERO {
        return None;
    }
    let last = previous?;
    if last.height != height || view <= last.view {
        return None;
    }
    let streak = next_cached_slot_timeout_streak(previous, height, view)
        .max(1)
        .min(3);
    let hysteresis = super::saturating_mul_duration(quorum_timeout, u32::from(streak) + 1);
    let elapsed = now.saturating_duration_since(last.at);
    (elapsed < hysteresis).then(|| hysteresis.saturating_sub(elapsed))
}

pub(super) fn cached_slot_effective_quorum_timeout(
    quorum_timeout: Duration,
    rebroadcast_cooldown: Duration,
    precommit_votes_at_view: usize,
    quorum: usize,
    missing_local_data: bool,
    consensus_queue_backlog: bool,
    rbc_session_incomplete: bool,
) -> Duration {
    let near_commit_quorum = precommit_votes_at_view > 0
        && precommit_votes_at_view < quorum
        && precommit_votes_at_view.saturating_add(1) >= quorum;
    let near_quorum_fast_timeout_allowed = near_commit_quorum
        && missing_local_data
        && !consensus_queue_backlog
        && !rbc_session_incomplete;
    if near_quorum_fast_timeout_allowed {
        super::reschedule::near_quorum_payload_timeout(rebroadcast_cooldown).min(quorum_timeout)
    } else {
        quorum_timeout
    }
}

fn trim_batch_for_size_cap<T, U>(
    tx_batch: &mut Vec<T>,
    routing_batch: &mut Vec<U>,
    sizes: &mut Vec<usize>,
    removed: &mut Vec<(T, U)>,
    mut excess_bytes: usize,
) -> usize {
    debug_assert_eq!(tx_batch.len(), routing_batch.len());
    debug_assert_eq!(tx_batch.len(), sizes.len());
    let mut removed_count = 0usize;
    while excess_bytes > 0 && tx_batch.len() > 1 {
        let tx = match tx_batch.pop() {
            Some(tx) => tx,
            None => break,
        };
        let routing = match routing_batch.pop() {
            Some(routing) => routing,
            None => break,
        };
        let size = sizes.pop().unwrap_or(1).max(1);
        excess_bytes = excess_bytes.saturating_sub(size);
        removed.push((tx, routing));
        removed_count = removed_count.saturating_add(1);
    }
    removed_count
}

const PROPOSAL_TIME_PADDING: std::time::Duration = std::time::Duration::from_millis(1);

#[derive(Debug, Clone, Copy)]
pub(super) struct InternalProposalWork {
    pub(super) time_triggers: bool,
    pub(super) da_commitments: bool,
    pub(super) da_pin_intents: bool,
}

impl InternalProposalWork {
    pub(super) const fn has_work(self) -> bool {
        self.time_triggers || self.da_commitments || self.da_pin_intents
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProposalBackpressure {
    pub(super) queue_state: BackpressureState,
    pub(super) active_pending: bool,
    pub(super) rbc_backlog: bool,
    pub(super) relay_backpressure: bool,
    pub(super) consensus_queue_backpressure: bool,
}

impl ProposalBackpressure {
    pub(super) fn should_defer(self) -> bool {
        self.queue_state.is_saturated()
            || self.active_pending
            || self.rbc_backlog
            || self.relay_backpressure
            || self.consensus_queue_backpressure
    }

    pub(super) fn only_queue_saturation(self) -> bool {
        self.queue_state.is_saturated()
            && !(self.active_pending
                || self.rbc_backlog
                || self.relay_backpressure
                || self.consensus_queue_backpressure)
    }
}

fn consensus_queue_backpressure(
    depths: status::WorkerQueueDepthSnapshot,
    block_payload_cap: usize,
    rbc_chunk_cap: usize,
) -> bool {
    let block_payload_cap = u64::try_from(block_payload_cap.max(1)).unwrap_or(u64::MAX);
    let rbc_chunk_cap = u64::try_from(rbc_chunk_cap.max(1)).unwrap_or(u64::MAX);
    depths.block_payload_rx >= block_payload_cap || depths.rbc_chunk_rx >= rbc_chunk_cap
}

fn da_payload_budget(
    rbc_chunk_max_bytes: usize,
    rbc_pending_max_bytes: usize,
    rbc_pending_max_chunks: usize,
    block_max_payload_bytes: Option<NonZeroUsize>,
    consensus_payload_frame_cap: usize,
) -> usize {
    let rbc_budget = rbc_chunk_max_bytes
        .max(1)
        .saturating_mul(usize::try_from(RBC_MAX_TOTAL_CHUNKS).expect("fits in usize"));
    let pending_budget = rbc_pending_max_bytes.min(
        rbc_chunk_max_bytes
            .max(1)
            .saturating_mul(rbc_pending_max_chunks.max(1)),
    );
    let payload_budget =
        non_rbc_payload_budget(block_max_payload_bytes, consensus_payload_frame_cap);
    payload_budget.min(rbc_budget).min(pending_budget)
}

impl Actor {
    pub(super) fn internal_proposal_work(
        &mut self,
        proposal_height: u64,
        prev_block: Option<&SignedBlock>,
    ) -> InternalProposalWork {
        let time_triggers = self.proposal_time_triggers_due(proposal_height, prev_block);
        if !self.runtime_da_enabled() {
            return InternalProposalWork {
                time_triggers,
                da_commitments: false,
                da_pin_intents: false,
            };
        }
        let (da_commitments, da_pin_intents) = self.proposal_da_spool_work();
        InternalProposalWork {
            time_triggers,
            da_commitments,
            da_pin_intents,
        }
    }

    fn proposal_time_triggers_due(
        &self,
        proposal_height: u64,
        prev_block: Option<&SignedBlock>,
    ) -> bool {
        let now = iroha_primitives::time::TimeSource::new_system().get_unix_time();
        let prev_block_time = prev_block.map_or(std::time::Duration::ZERO, |block| {
            block.header().creation_time()
        });
        let creation_time = std::cmp::max(
            now,
            std::cmp::max(
                prev_block_time.saturating_add(PROPOSAL_TIME_PADDING),
                PROPOSAL_TIME_PADDING,
            ),
        );
        let since = NonZeroUsize::new(self.state.committed_height())
            .and_then(|height| self.state.block_by_height(height))
            .map_or(creation_time, |block| block.header().creation_time());
        let (since, length) = creation_time
            .checked_sub(since)
            .map_or((creation_time, std::time::Duration::ZERO), |length| {
                (since, length)
            });
        let event = iroha_data_model::events::time::TimeEvent {
            interval: iroha_data_model::events::time::TimeInterval::new(since, length),
        };
        let key_height = "__registered_block_height"
            .parse::<iroha_data_model::name::Name>()
            .ok();
        let world = self.state.world_view();
        world
            .triggers()
            .time_triggers()
            .iter()
            .filter(|(_, action)| {
                crate::smartcontracts::isi::triggers::trigger_is_enabled(action.metadata())
            })
            .any(|(_, action)| {
                let mut count = action.filter.count_matches(&event);
                if let Repeats::Exactly(repeats) = action.repeats {
                    count = std::cmp::min(repeats, count);
                }
                if count == 0 {
                    return false;
                }
                let registered_height = key_height
                    .as_ref()
                    .and_then(|key| action.metadata().get(key))
                    .and_then(|json| json.try_into_any_norito::<u64>().ok());
                registered_height.is_some_and(|height| height != proposal_height)
            })
    }

    fn proposal_da_spool_work(&mut self) -> (bool, bool) {
        let da_rbc = &mut self.subsystems.da_rbc;
        let commitment_has = match da_rbc.spool_cache.load_commitment_bundle(&da_rbc.spool_dir) {
            Ok((value, cache_outcome)) => {
                #[cfg(feature = "telemetry")]
                self.telemetry.note_da_spool_cache(
                    crate::telemetry::DaSpoolCacheKind::Commitments,
                    cache_outcome.as_telemetry(),
                );
                #[cfg(not(feature = "telemetry"))]
                let _ = cache_outcome;
                value.is_some_and(|bundle| {
                    bundle.commitments.iter().any(|record| {
                        let key =
                            iroha_data_model::da::commitment::DaCommitmentKey::from_record(record);
                        !da_rbc.da.sealed_commitments.contains(&key)
                    })
                })
            }
            Err(err) => {
                warn!(
                    ?err,
                    spool = %da_rbc.spool_dir.display(),
                    "failed to load DA commitments from spool; proceeding without DA bundle"
                );
                false
            }
        };

        let pin_intent_has = match da_rbc.spool_cache.load_pin_bundle(&da_rbc.spool_dir) {
            Ok((value, cache_outcome)) => {
                #[cfg(feature = "telemetry")]
                self.telemetry.note_da_spool_cache(
                    crate::telemetry::DaSpoolCacheKind::PinIntents,
                    cache_outcome.as_telemetry(),
                );
                #[cfg(not(feature = "telemetry"))]
                let _ = cache_outcome;
                value.is_some_and(|bundle| {
                    bundle.intents.iter().any(|intent| {
                        let key = (intent.lane_id.as_u32(), intent.epoch, intent.sequence);
                        !da_rbc.da.sealed_pin_intents.contains(&key)
                    })
                })
            }
            Err(err) => {
                warn!(
                    ?err,
                    spool = %da_rbc.spool_dir.display(),
                    "failed to load DA pin intents from spool; proceeding without pin bundle"
                );
                false
            }
        };

        (commitment_has, pin_intent_has)
    }

    pub(super) fn max_tx_budget(
        queue_len: usize,
        block_param_limit: u64,
        config_cap: Option<NonZeroUsize>,
    ) -> (usize, NonZeroUsize) {
        let param_limit = usize::try_from(block_param_limit).unwrap_or_else(|_| {
            warn!(
                block_param_limit,
                "block max transactions exceeds usize; capping to usize::MAX"
            );
            usize::MAX
        });
        let max_tx_target = config_cap
            .map(NonZeroUsize::get)
            .map_or(param_limit, |cfg| cfg.min(param_limit));
        let max_in_block = NonZeroUsize::new(queue_len.min(max_tx_target).max(1))
            .expect("non-zero by construction");
        (max_tx_target, max_in_block)
    }

    pub(super) fn cap_gas_limit_for_fast_commit(
        gas_limit_per_block: Option<NonZeroU64>,
        effective_commit_time_ms: u64,
        fast_gas_limit_per_block: Option<NonZeroU64>,
    ) -> Option<NonZeroU64> {
        let Some(base_limit) = gas_limit_per_block else {
            return None;
        };
        let Some(cap) = fast_gas_limit_per_block else {
            return Some(base_limit);
        };
        if effective_commit_time_ms
            > iroha_config::parameters::defaults::sumeragi::FAST_FINALITY_COMMIT_TIME_MS
        {
            return Some(base_limit);
        }
        let capped = base_limit.get().min(cap.get());
        Some(NonZeroU64::new(capped).expect("non-zero by construction"))
    }

    pub(super) fn pull_transactions_for_proposal(
        &self,
        state: &State,
        max_in_block: NonZeroUsize,
        scan_budget: usize,
        gas_limit_per_block: Option<NonZeroU64>,
        tx_guards: &mut Vec<crate::queue::TransactionGuard>,
        height: u64,
        view: u64,
    ) -> Vec<(AcceptedTransaction<'static>, RoutingDecision)> {
        let mut lane_consumption: BTreeMap<LaneId, u64> = BTreeMap::new();
        let mut deferred_accumulator: Vec<(AcceptedTransaction<'static>, RoutingDecision)> =
            Vec::new();
        let mut fetched_total = 0usize;
        let mut gas_used_in_block = 0u64;
        let gas_limit_per_block = gas_limit_per_block.map(NonZeroU64::get);
        let scan_budget = scan_budget.max(1);

        loop {
            let remaining_budget = scan_budget.saturating_sub(fetched_total);
            if remaining_budget == 0 {
                debug!(
                    height,
                    view, scan_budget, fetched_total, "proposal queue scan budget reached"
                );
                break;
            }
            let remaining_slots = max_in_block.get().saturating_sub(tx_guards.len());
            if remaining_slots == 0 {
                break;
            }
            if let Some(limit) = gas_limit_per_block {
                let remaining_gas = limit.saturating_sub(gas_used_in_block);
                if remaining_gas == 0 {
                    debug!(
                        height,
                        view,
                        gas_limit = limit,
                        gas_used = gas_used_in_block,
                        "proposal gas budget reached"
                    );
                    break;
                }
            }
            let fetch_cap = NonZeroUsize::new(remaining_budget.min(remaining_slots))
                .expect("non-zero by construction");
            let mut fetched = Vec::new();
            self.queue
                .get_transactions_for_block_with_state(state, fetch_cap, &mut fetched);
            if fetched.is_empty() {
                break;
            }
            fetched_total = fetched_total.saturating_add(fetched.len());
            let deferred = self
                .queue
                .enforce_lane_teu_limits_with_consumption_and_routing(
                    &mut fetched,
                    &mut lane_consumption,
                );
            if !deferred.is_empty() {
                deferred_accumulator.extend(deferred);
            }

            if let Some(limit) = gas_limit_per_block {
                let mut accepted = Vec::with_capacity(fetched.len());
                for guard in fetched {
                    let gas_cost = guard.gas_cost();
                    let remaining_gas = limit.saturating_sub(gas_used_in_block);
                    let would_exceed = gas_cost > remaining_gas && gas_cost > 0;
                    let allow_oversized = gas_used_in_block == 0 && accepted.is_empty();

                    if would_exceed && !allow_oversized {
                        let lane_id = guard.routing().lane_id;
                        let teu = guard.teu_weight();
                        if let Some(used) = lane_consumption.get_mut(&lane_id) {
                            *used = used.saturating_sub(teu);
                        }
                        deferred_accumulator.push((guard.clone_accepted(), guard.routing()));
                        continue;
                    }

                    if would_exceed {
                        debug!(
                            height,
                            view,
                            gas_cost,
                            gas_limit = limit,
                            "proposal gas cap exceeded by single tx; admitting to avoid stall"
                        );
                    }
                    gas_used_in_block = gas_used_in_block.saturating_add(gas_cost);
                    accepted.push(guard);
                }
                tx_guards.extend(accepted);
            } else {
                tx_guards.extend(fetched);
            }

            if let Some(limit) = gas_limit_per_block {
                if gas_used_in_block >= limit {
                    break;
                }
            }
        }

        deferred_accumulator
    }

    fn requeue_accepted_transaction(
        &self,
        tx: AcceptedTransaction<'static>,
        routing_decision: crate::queue::RoutingDecision,
        warn_context: &'static str,
    ) {
        if let Err(err) =
            self.queue
                .push_requeued_with_routing(tx, routing_decision, self.state.as_ref())
        {
            warn!(?err.err, "{warn_context}");
        }
    }

    pub(super) fn drop_stale_pending_block(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<(usize, usize, usize, usize)> {
        let (tx_count, requeued, failures, duplicate_failures) =
            super::drop_pending_block_and_requeue(
                &mut self.pending.pending_blocks,
                pending_hash,
                self.queue.as_ref(),
                self.state.as_ref(),
            )?;

        self.clean_rbc_sessions_for_block(pending_hash, height);
        self.qc_cache
            .retain(|(_, hash, _, _, _), _| hash != &pending_hash);
        self.qc_signer_tally
            .retain(|(_, hash, _, _, _), _| hash != &pending_hash);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        // Keep proposals_seen so we don't re-propose in the same view after dropping a stale block.

        Some((tx_count, requeued, failures, duplicate_failures))
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    pub(super) fn assemble_and_broadcast_proposal(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        topology: &mut super::network_topology::Topology,
        leader_index: usize,
        local_validator_index: u32,
        view_snapshot: Option<StateView<'_>>,
        now: Instant,
    ) -> Result<bool> {
        if self.is_observer() {
            return Ok(false);
        }
        if view == u64::MAX {
            warn!(
                height,
                view, "skipping proposal assembly: view-change index overflow"
            );
            return Ok(false);
        }
        super::status::set_leader_index(leader_index as u64);
        let required_for_commit = topology.min_votes_for_commit();
        debug!(
            height,
            view,
            topology_size = topology.as_ref().len(),
            required_for_commit,
            "proposal topology snapshot"
        );
        let proposal_height = height;
        let proposal_epoch = self.epoch_for_height(proposal_height);
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        self.prune_highest_qc_missing_defer_markers(committed_height);
        self.init_collector_plan(topology, proposal_height, view);
        if let Some(existing_vote) = self.local_same_height_vote(proposal_height, proposal_epoch) {
            warn!(
                height = proposal_height,
                view,
                epoch = proposal_epoch,
                voted_view = existing_vote.view,
                voted_phase = ?existing_vote.phase,
                voted_block = %existing_vote.block_hash,
                "deferring proposal assembly: local same-height vote already anchors another branch"
            );
            return Ok(false);
        }
        let _lock_lag_highest_qc_deferred = !self.highest_qc_extends_locked(highest_qc)
            && self.defer_highest_qc_update_for_lock_catchup(
                height, view, highest_qc, now, "proposal",
            );
        if proposal_height > 1 && !self.block_known_locally(highest_qc.subject_block_hash) {
            if self.mark_highest_qc_missing_defer_for_round(proposal_height, view, highest_qc) {
                self.observe_new_view_highest_qc(highest_qc);
            }
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::HighestQcMissing,
                proposal_height,
                view,
                highest_qc.subject_block_hash,
                now,
                Duration::from_secs(5),
            ) {
                warn!(
                    height = proposal_height,
                    view,
                    highest_height = highest_qc.height,
                    highest_hash = %highest_qc.subject_block_hash,
                    suppressed_since_last,
                    "deferring proposal assembly: highest QC block not available locally"
                );
            }
            return Ok(false);
        }
        let prev_block = resolve_prev_block_for_proposal(
            proposal_height,
            &highest_qc,
            &self.kura,
            &self.pending.pending_blocks,
        );
        if prev_block.is_none() && proposal_height > 1 {
            if !self.block_known_locally(highest_qc.subject_block_hash) {
                if self.mark_highest_qc_missing_defer_for_round(proposal_height, view, highest_qc) {
                    self.observe_new_view_highest_qc(highest_qc);
                }
            }
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::ParentMissing,
                proposal_height,
                view,
                highest_qc.subject_block_hash,
                now,
                Duration::from_secs(5),
            ) {
                warn!(
                    height = proposal_height,
                    view,
                    parent_height = proposal_height.saturating_sub(1),
                    highest_height = highest_qc.height,
                    highest_hash = %highest_qc.subject_block_hash,
                    suppressed_since_last,
                    "deferring proposal assembly: parent block not available locally"
                );
            }
            return Ok(false);
        }

        let queue_len = self.queue.queued_len();
        let mut tx_guards = Vec::new();
        let (
            _block_digest,
            conf_features,
            mut transactions,
            mut routing_decisions,
            mut tx_sizes,
            deferred_transactions,
        ) = {
            let (block_max_param, effective_commit_time_ms, base_gas_limit, digest) =
                if let Some(state_view) = view_snapshot.as_ref() {
                    let block_max_param =
                        state_view.world().parameters().block().max_transactions();
                    let effective_commit_time_ms = state_view
                        .world()
                        .parameters()
                        .sumeragi()
                        .effective_commit_time_ms();
                    let base_gas_limit = NonZeroU64::new(crate::state::gas_limit_from_parameters(
                        state_view.world().parameters(),
                    ));
                    let committed_height = u64::try_from(state_view.height())
                        .expect("committed height exceeds u64::MAX");
                    let next_height = committed_height
                        .checked_add(1)
                        .expect("block height exceeds u64::MAX");
                    let digest = self.state.cached_confidential_feature_digest(
                        state_view.world(),
                        &state_view.zk,
                        next_height,
                    );
                    (
                        block_max_param,
                        effective_commit_time_ms,
                        base_gas_limit,
                        digest,
                    )
                } else {
                    let world = self.state.world_view();
                    let block_max_param = world.parameters().block().max_transactions();
                    let effective_commit_time_ms =
                        world.parameters().sumeragi().effective_commit_time_ms();
                    let base_gas_limit = NonZeroU64::new(crate::state::gas_limit_from_parameters(
                        world.parameters(),
                    ));
                    let committed_height = u64::try_from(self.state.committed_height())
                        .expect("committed height exceeds u64::MAX");
                    let next_height = committed_height
                        .checked_add(1)
                        .expect("block height exceeds u64::MAX");
                    let zk = self.state.zk_snapshot();
                    let digest =
                        self.state
                            .cached_confidential_feature_digest(&world, &zk, next_height);
                    (
                        block_max_param,
                        effective_commit_time_ms,
                        base_gas_limit,
                        digest,
                    )
                };
            let (max_tx_target, max_in_block) = Self::max_tx_budget(
                queue_len,
                block_max_param.get(),
                self.config.block.max_transactions,
            );
            let fast_gas_limit_per_block = self.config.block.fast_gas_limit_per_block;
            let proposal_gas_limit = Self::cap_gas_limit_for_fast_commit(
                base_gas_limit,
                effective_commit_time_ms,
                fast_gas_limit_per_block,
            );
            let fast_gas_capped = proposal_gas_limit != base_gas_limit;
            let scan_budget = self.proposal_scan_budget(max_in_block);
            debug!(
                height,
                view,
                queue_len,
                max_tx_param = block_max_param.get(),
                max_tx_target,
                max_in_block = max_in_block.get(),
                scan_budget,
                scan_multiplier = self.config.block.proposal_queue_scan_multiplier.get(),
                effective_commit_time_ms,
                gas_limit_per_block = base_gas_limit.map(NonZeroU64::get),
                fast_gas_limit_per_block = fast_gas_limit_per_block.map(NonZeroU64::get),
                proposal_gas_limit = proposal_gas_limit.map(NonZeroU64::get),
                fast_gas_capped,
                "proposal assembly budget"
            );
            // Bound queue scanning to keep proposal assembly from stalling under sustained load.
            let deferred_accumulator = self.pull_transactions_for_proposal(
                self.state.as_ref(),
                max_in_block,
                scan_budget,
                proposal_gas_limit,
                &mut tx_guards,
                height,
                view,
            );
            let transactions: Vec<AcceptedTransaction<'static>> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::clone_accepted)
                .collect();
            let routing_decisions: Vec<RoutingDecision> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::routing)
                .collect();
            let tx_sizes: Vec<usize> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::encoded_len)
                .collect();
            let conf_features = if digest.is_empty() {
                None
            } else {
                Some(digest)
            };
            (
                digest,
                conf_features,
                transactions,
                routing_decisions,
                tx_sizes,
                deferred_accumulator,
            )
        };

        let (filtered_guards, filtered_transactions, filtered_routing, filtered_sizes, _dropped) =
            Self::filter_committed_transactions_for_proposal(
                self.state.as_ref(),
                tx_guards,
                transactions,
                routing_decisions,
                tx_sizes,
                height,
                view,
            );
        tx_guards = filtered_guards;
        transactions = filtered_transactions;
        routing_decisions = filtered_routing;
        tx_sizes = filtered_sizes;

        if transactions.len() > 1 {
            let order = interleave_lane_indices(&routing_decisions);

            if order.iter().enumerate().any(|(idx, &value)| idx != value) {
                fn reorder_vec<T: Clone>(vec: &mut Vec<T>, order: &[usize]) {
                    let original = vec.clone();
                    vec.clear();
                    for &idx in order {
                        vec.push(original[idx].clone());
                    }
                }
                reorder_vec(&mut transactions, &order);
                reorder_vec(&mut routing_decisions, &order);
                reorder_vec(&mut tx_sizes, &order);
            }
        }

        for (tx, routing) in deferred_transactions {
            self.requeue_accepted_transaction(
                tx,
                routing,
                "failed to requeue transaction deferred by lane TEU limits",
            );
        }

        let queue_len_after_pop = self.queue.queued_len();
        let mut internal_work = if transactions.is_empty() {
            let work = self.internal_proposal_work(proposal_height, prev_block.as_deref());
            if !work.has_work() {
                info!(
                    height,
                    view,
                    queue_len = queue_len_after_pop,
                    "skipping empty proposal; empty blocks are disallowed"
                );
                return Ok(false);
            }
            Some(work)
        } else {
            None
        };

        let da_enabled = self.runtime_da_enabled();
        let mut overflow_transactions: Vec<(AcceptedTransaction<'static>, RoutingDecision)> =
            Vec::new();
        let mut oversized_frame_len: Option<usize> = None;
        let tx_sizes_in = tx_sizes;
        let mut tx_batch;
        let mut routing_batch;
        let mut tx_sizes;
        if da_enabled {
            let mut remaining_budget = da_payload_budget(
                self.config.rbc.chunk_max_bytes,
                self.config.rbc.pending_max_bytes,
                self.config.rbc.pending_max_chunks,
                self.config.block.max_payload_bytes,
                self.consensus_payload_frame_cap,
            );
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            let mut tx_sizes_out = Vec::with_capacity(transactions.len());
            for ((tx, routing), encoded_len) in transactions
                .into_iter()
                .zip(routing_decisions.into_iter())
                .zip(tx_sizes_in.into_iter())
            {
                if encoded_len > self.consensus_payload_frame_cap {
                    oversized_frame_len =
                        Some(oversized_frame_len.map_or(encoded_len, |prev| prev.max(encoded_len)));
                }
                if encoded_len > remaining_budget {
                    overflow_transactions.push((tx, routing));
                    continue;
                }
                remaining_budget = remaining_budget.saturating_sub(encoded_len);
                tx_sizes_out.push(encoded_len);
                tx_batch.push(tx);
                routing_batch.push(routing);
            }
            tx_sizes = tx_sizes_out;
        } else {
            let mut payload_budget = Some(non_rbc_payload_budget(
                self.config.block.max_payload_bytes,
                self.consensus_payload_frame_cap,
            ));
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            let mut tx_sizes_out = Vec::with_capacity(transactions.len());
            for ((tx, routing), encoded_len) in transactions
                .into_iter()
                .zip(routing_decisions.into_iter())
                .zip(tx_sizes_in.into_iter())
            {
                if let Some(budget) = payload_budget {
                    if encoded_len > budget {
                        overflow_transactions.push((tx, routing));
                        continue;
                    }
                    payload_budget = Some(budget.saturating_sub(encoded_len));
                    tx_sizes_out.push(encoded_len);
                }
                tx_batch.push(tx);
                routing_batch.push(routing);
            }
            tx_sizes = tx_sizes_out;
        }

        if tx_batch.len() > 1 {
            if let Some(admin_idx) = tx_batch.iter().position(Self::is_peer_admin_transaction) {
                let admin_tx = tx_batch.remove(admin_idx);
                let admin_route = routing_batch.remove(admin_idx);
                let admin_size = tx_sizes.remove(admin_idx);
                overflow_transactions.extend(tx_batch.drain(..).zip(routing_batch.drain(..)));
                tx_sizes.clear();
                tx_batch.push(admin_tx);
                routing_batch.push(admin_route);
                tx_sizes.push(admin_size);
            }
        }

        if tx_batch.is_empty() {
            tx_guards.clear();
            for (tx, routing) in std::mem::take(&mut overflow_transactions) {
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue oversized transaction",
                );
            }
            let has_internal_work = internal_work
                .get_or_insert_with(|| {
                    self.internal_proposal_work(proposal_height, prev_block.as_deref())
                })
                .has_work();
            if !has_internal_work {
                if let Some(frame_len) = oversized_frame_len {
                    return Err(eyre!(
                        "proposal frame size {frame_len} exceeds consensus payload cap {}",
                        self.consensus_payload_frame_cap
                    ));
                }
                info!(
                    height = proposal_height,
                    view,
                    queue_len = queue_len_after_pop,
                    "deferring proposal: no transactions fit within payload budget"
                );
                return Ok(false);
            }
            debug!(
                height = proposal_height,
                view, "assembling proposal without external transactions"
            );
        }

        let original_for_requeue: Vec<(AcceptedTransaction<'static>, RoutingDecision)> = tx_batch
            .iter()
            .cloned()
            .zip(routing_batch.iter().copied())
            .collect();
        let previous_roster_evidence = prev_block.as_deref().and_then(|parent| {
            previous_roster_evidence_for_parent(
                self.state.as_ref(),
                self.kura.as_ref(),
                self.consensus_context_for_height(parent.header().height().get())
                    .0,
                parent,
            )
        });
        let mut removed_for_chunk_cap: Vec<(AcceptedTransaction<'static>, RoutingDecision)> =
            Vec::new();
        let mut removed_for_frame_cap: Vec<(AcceptedTransaction<'static>, RoutingDecision)> =
            Vec::new();
        let assembly_result: Result<()> = (|| {
            if tx_sizes.len() < tx_batch.len() {
                for tx in tx_batch.iter().skip(tx_sizes.len()) {
                    tx_sizes.push(tx.as_ref().encode().len());
                }
            }
            let (
                signed_block,
                block_created_msg,
                payload_bytes,
                payload_hash,
                proposal,
                proposal_hint,
                transactions_for_plan,
                block_hash,
            ) = loop {
                let nexus = self.state.nexus_snapshot();
                let nexus_enabled = nexus.enabled;
                let lane_config = nexus.lane_config.clone();
                let mut builder =
                    BlockBuilder::new(tx_batch.clone()).chain(view, prev_block.as_deref());
                if proposal_height > 2 && previous_roster_evidence.is_none() {
                    return Err(eyre!(
                        "missing previous-roster evidence for parent block at height {}",
                        proposal_height.saturating_sub(1),
                    ));
                }
                builder = builder.with_previous_roster_evidence(previous_roster_evidence.clone());

                let receipt_plan = if nexus_enabled {
                    let cursor_snapshot = self.state.da_receipt_cursor_snapshot();
                    let (receipts, cache_outcome) = {
                        let da_rbc = &mut self.subsystems.da_rbc;
                        crate::da::receipts::prune_spool(&da_rbc.spool_dir, &cursor_snapshot);
                        da_rbc
                            .spool_cache
                            .load_receipt_entries(&da_rbc.spool_dir)
                            .map_err(|err| eyre!(err))?
                    };
                    #[cfg(feature = "telemetry")]
                    self.telemetry.note_da_spool_cache(
                        crate::telemetry::DaSpoolCacheKind::Receipts,
                        cache_outcome.as_telemetry(),
                    );
                    #[cfg(not(feature = "telemetry"))]
                    let _ = cache_outcome;
                    crate::da::receipts::plan_committable_receipts(
                        &lane_config,
                        &cursor_snapshot,
                        &self.subsystems.da_rbc.da.sealed_commitments,
                        receipts,
                    )
                    .map_err(|err| eyre!(err))?
                } else {
                    Vec::new()
                };

                let mut bundle_opt = {
                    let da_rbc = &mut self.subsystems.da_rbc;
                    match da_rbc.spool_cache.load_commitment_bundle(&da_rbc.spool_dir) {
                        Ok((value, cache_outcome)) => {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_spool_cache(
                                crate::telemetry::DaSpoolCacheKind::Commitments,
                                cache_outcome.as_telemetry(),
                            );
                            #[cfg(not(feature = "telemetry"))]
                            let _ = cache_outcome;
                            value
                        }
                        Err(err) => {
                            warn!(
                                ?err,
                                spool = %da_rbc.spool_dir.display(),
                                "failed to load DA commitments from spool; proceeding without DA bundle"
                            );
                            None
                        }
                    }
                };

                if bundle_opt.is_none() && nexus_enabled && !receipt_plan.is_empty() {
                    return Err(eyre!(
                        "DA receipts are present but no commitment records are available in the spool"
                    ));
                }

                if let Some(bundle) = bundle_opt.as_mut() {
                    // Drop commitments that were already sealed to avoid duplication.
                    let filtered = {
                        let da_rbc = &mut self.subsystems.da_rbc;
                        let mut kept = Vec::with_capacity(bundle.commitments.len());
                        for record in &bundle.commitments {
                            let key =
                                iroha_data_model::da::commitment::DaCommitmentKey::from_record(
                                    record,
                                );
                            if da_rbc.da.sealed_commitments.contains(&key) {
                                continue;
                            }
                            let policy = lane_config.manifest_policy(record.lane_id);
                            let (available, cache_outcome) =
                                crate::sumeragi::main_loop::manifest_available_for_commitment(
                                    &mut da_rbc.manifest_cache,
                                    &da_rbc.spool_dir,
                                    record,
                                    policy,
                                );
                            #[cfg(feature = "telemetry")]
                            self.telemetry
                                .note_da_manifest_cache(cache_outcome.as_telemetry());
                            #[cfg(not(feature = "telemetry"))]
                            let _ = cache_outcome;
                            if available {
                                kept.push(record.clone());
                            }
                        }
                        kept
                    };
                    bundle.commitments = filtered;

                    if nexus_enabled {
                        if receipt_plan.is_empty() {
                            bundle.commitments.clear();
                        } else {
                            let filtered = crate::da::receipts::align_commitments_for_receipts(
                                &receipt_plan,
                                &bundle.commitments,
                            )
                            .map_err(|err| eyre!(err))?;
                            bundle.commitments = filtered;
                        }
                    }

                    if bundle.is_empty() {
                        bundle_opt = None;
                    } else {
                        self.validate_da_bundle(bundle)?;
                    }

                    if let Some(bundle) = bundle_opt.as_ref() {
                        let shard_cursor_path = crate::da::DaShardCursorJournal::journal_path(
                            &self.subsystems.da_rbc.spool_dir,
                        );
                        let mut shard_journal = match crate::da::DaShardCursorJournal::load(
                            &lane_config,
                            shard_cursor_path.clone(),
                        ) {
                            Ok(journal) => journal,
                            Err(err) => {
                                warn!(
                                    ?err,
                                    path = %shard_cursor_path.display(),
                                    "failed to load DA shard cursor journal; rebuilding from scratch"
                                );
                                crate::da::DaShardCursorJournal::new(
                                    &lane_config,
                                    shard_cursor_path.clone(),
                                )
                            }
                        };

                        if let Err(err) = shard_journal.record_bundle(proposal_height, bundle) {
                            warn!(
                                ?err,
                                "failed to update shard cursors from DA bundle; leaving journal unchanged"
                            );
                        } else if let Err(err) = shard_journal.persist() {
                            warn!(
                                ?err,
                                path = %shard_cursor_path.display(),
                                "failed to persist DA shard cursor journal"
                            );
                        }
                    }
                }

                if let Some(bundle) = bundle_opt {
                    for record in &bundle.commitments {
                        let key =
                            iroha_data_model::da::commitment::DaCommitmentKey::from_record(record);
                        self.subsystems.da_rbc.da.sealed_commitments.insert(key);
                    }
                    self.subsystems
                        .da_rbc
                        .da
                        .da_bundles
                        .insert(proposal_height, bundle.clone());
                    builder = builder.with_da_commitments(Some(bundle));
                }

                let pin_bundle_opt = {
                    let da_rbc = &mut self.subsystems.da_rbc;
                    match da_rbc.spool_cache.load_pin_bundle(&da_rbc.spool_dir) {
                        Ok((value, cache_outcome)) => {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_spool_cache(
                                crate::telemetry::DaSpoolCacheKind::PinIntents,
                                cache_outcome.as_telemetry(),
                            );
                            #[cfg(not(feature = "telemetry"))]
                            let _ = cache_outcome;
                            value
                        }
                        Err(err) => {
                            warn!(
                                ?err,
                                spool = %da_rbc.spool_dir.display(),
                                "failed to load DA pin intents from spool; proceeding without pin bundle"
                            );
                            None
                        }
                    }
                };

                if let Some(bundle) = pin_bundle_opt {
                    let world = self.state.world_view();
                    let account_exists = |account: &iroha_data_model::account::AccountId| -> bool {
                        world.accounts().get(account).is_some()
                    };
                    let (mut intents, rejected) = crate::da::sanitize_pin_intents(
                        bundle.intents,
                        &lane_config,
                        account_exists,
                    );
                    if !rejected.is_empty() {
                        for reason in rejected {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_pin_intent_spool(
                                crate::telemetry::PinIntentSpoolResult::Dropped,
                                crate::telemetry::PinIntentSpoolReason::from(&reason),
                            );
                            warn!(
                                height = proposal_height,
                                ?reason,
                                "dropping invalid DA pin intent before sealing bundle"
                            );
                        }
                    }
                    #[cfg(feature = "telemetry")]
                    let dedupe_before = intents.len();
                    intents.retain(|intent| {
                        let key = (intent.lane_id.as_u32(), intent.epoch, intent.sequence);
                        !self.subsystems.da_rbc.da.sealed_pin_intents.contains(&key)
                    });
                    #[cfg(feature = "telemetry")]
                    {
                        let deduped = dedupe_before.saturating_sub(intents.len());
                        for _ in 0..deduped {
                            self.telemetry.note_da_pin_intent_spool(
                                crate::telemetry::PinIntentSpoolResult::Dropped,
                                crate::telemetry::PinIntentSpoolReason::SealedDuplicate,
                            );
                        }
                    }
                    if !intents.is_empty() {
                        let sanitized_bundle = DaPinIntentBundle::new(intents);
                        for intent in &sanitized_bundle.intents {
                            let key = (intent.lane_id.as_u32(), intent.epoch, intent.sequence);
                            self.subsystems.da_rbc.da.sealed_pin_intents.insert(key);
                        }
                        #[cfg(feature = "telemetry")]
                        for _ in &sanitized_bundle.intents {
                            self.telemetry.note_da_pin_intent_spool(
                                crate::telemetry::PinIntentSpoolResult::Kept,
                                crate::telemetry::PinIntentSpoolReason::Kept,
                            );
                        }
                        self.subsystems
                            .da_rbc
                            .da
                            .da_pin_bundles
                            .insert(proposal_height, sanitized_bundle.clone());
                        builder = builder.with_da_pin_intents(Some(sanitized_bundle));
                    }
                }

                let proof_policy_bundle = crate::da::proof_policy_bundle(&lane_config);
                builder = builder.with_da_proof_policies(Some(proof_policy_bundle));

                let new_block = builder
                    .with_confidential_features(conf_features)
                    .sign_with_index(
                        self.common_config.key_pair.private_key(),
                        u64::from(local_validator_index),
                    )
                    .unpack(|event| self.emit_pipeline_event(event));
                let signed_block: SignedBlock = new_block.into();
                let built_height = signed_block.header().height().get();
                if built_height != proposal_height {
                    debug!(
                        expected = proposal_height,
                        actual = built_height,
                        "constructed block height differs from NEW_VIEW target"
                    );
                }
                let block_hash = signed_block.hash();
                let payload_bytes = block_payload_bytes(&signed_block);
                if da_enabled {
                    let total_chunks =
                        rbc::chunk_count(payload_bytes.len(), self.config.rbc.chunk_max_bytes);
                    if total_chunks > usize::try_from(RBC_MAX_TOTAL_CHUNKS).expect("fits in usize")
                    {
                        if tx_batch.len() <= 1 {
                            warn!(
                                height = proposal_height,
                                view,
                                total_chunks,
                                max_chunks = RBC_MAX_TOTAL_CHUNKS,
                                "block payload exceeds RBC chunk cap; unable to assemble proposal"
                            );
                            return Err(eyre!(
                                "proposal payload requires {total_chunks} chunks, exceeding cap {}",
                                RBC_MAX_TOTAL_CHUNKS
                            ));
                        }
                        if let Some(removed_tx) = tx_batch.pop() {
                            let removed_routing = routing_batch
                                .pop()
                                .expect("routing batch should align with tx batch");
                            let _ = tx_sizes.pop();
                            removed_for_chunk_cap.push((removed_tx, removed_routing));
                            continue;
                        }
                    }
                }
                let payload_hash = Hash::new(&payload_bytes);
                let proposal = Self::build_consensus_proposal(
                    &signed_block,
                    payload_hash,
                    highest_qc,
                    local_validator_index,
                    view,
                    proposal_epoch,
                );

                let proposal_hint = super::message::ProposalHint {
                    block_hash,
                    height: proposal_height,
                    view,
                    highest_qc,
                };
                self.subsystems
                    .propose
                    .proposal_cache
                    .insert_hint(proposal_hint);
                let block_created = if let Some(block_created) = self
                    .frontier_block_created_for_local_proposal_wire(
                        &signed_block,
                        &proposal,
                        topology.as_ref(),
                    ) {
                    block_created
                } else {
                    warn!(
                        height = proposal_height,
                        view,
                        block = %block_hash,
                        "aborting active proposal because frontier metadata could not be rebuilt"
                    );
                    return Err(eyre!(
                        "failed to rebuild authoritative frontier metadata for active proposal"
                    ));
                };
                let block_created_msg = BlockMessage::BlockCreated(block_created);
                let frame_len = super::consensus_block_wire_len(
                    self.common_config.peer.id(),
                    &block_created_msg,
                );
                if frame_len > self.consensus_payload_frame_cap {
                    if tx_batch.len() <= 1 {
                        warn!(
                            height = proposal_height,
                            view,
                            frame_len,
                            cap = self.consensus_payload_frame_cap,
                            da_enabled,
                            "BlockCreated frame exceeds consensus payload cap; unable to assemble proposal"
                        );
                        return Err(eyre!(
                            "proposal frame size {frame_len} exceeds consensus payload cap {}",
                            self.consensus_payload_frame_cap
                        ));
                    }
                    let excess = frame_len.saturating_sub(self.consensus_payload_frame_cap);
                    let removed = trim_batch_for_size_cap(
                        &mut tx_batch,
                        &mut routing_batch,
                        &mut tx_sizes,
                        &mut removed_for_frame_cap,
                        excess,
                    );
                    if removed == 0 {
                        if let Some(removed_tx) = tx_batch.pop() {
                            let removed_routing = routing_batch
                                .pop()
                                .expect("routing batch should align with tx batch");
                            let _ = tx_sizes.pop();
                            removed_for_frame_cap.push((removed_tx, removed_routing));
                            continue;
                        }
                    }
                    continue;
                }

                break (
                    signed_block,
                    block_created_msg,
                    payload_bytes,
                    payload_hash,
                    proposal,
                    proposal_hint,
                    tx_batch.clone(),
                    block_hash,
                );
            };

            // Loop back consensus messages locally so the leader participates immediately.
            let frontier_block_created_ready = matches!(
                &block_created_msg,
                BlockMessage::BlockCreated(created) if created.frontier.is_some()
            );
            self.subsystems
                .propose
                .proposal_cache
                .insert_hint(proposal_hint);
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal);
            let mut rbc_plan = if frontier_block_created_ready {
                None
            } else {
                self.prepare_rbc_plan(rbc::RbcPlanInputs {
                    signed_block: &signed_block,
                    transactions: &transactions_for_plan,
                    routing: &routing_batch,
                    payload: &payload_bytes,
                    payload_hash,
                    height: proposal_height,
                    view,
                    epoch: proposal_epoch,
                    local_validator_index,
                })?
            };
            drop(payload_bytes);

            if let Some(plan) = rbc_plan.as_ref() {
                // Non-frontier recovery still uses RBC transport, but exact frontier proposals
                // are owned entirely by BlockCreated plus exact body fetch.
                self.install_rbc_session_plan(&plan.primary)?;
                if let Some(dup) = plan.duplicate.as_ref() {
                    self.install_rbc_session_plan(dup)?;
                }
                self.publish_rbc_backlog_snapshot();
            }

            let block_created_wire = Arc::new(block_created_msg.clone());
            let block_created_encoded = Arc::new(BlockMessageWire::encode_message(
                block_created_wire.as_ref(),
            ));
            if let BlockMessage::BlockCreated(block_msg) = block_created_msg.clone() {
                self.handle_block_created(block_msg, None)?;
                if frontier_block_created_ready {
                    // Exact frontier proposals can skip handle_proposal(), so record the locally
                    // assembled view as observed here to avoid immediate no-proposal churn.
                    self.note_proposal_seen(proposal_height, view, payload_hash);
                }
            }
            if !frontier_block_created_ready {
                self.handle_proposal(proposal)?;
                // Local handling can consume cache entries while validating or finalizing the slot.
                // Reinsert the advisory metadata so same-view rebroadcast and recovery remain intact.
                self.subsystems
                    .propose
                    .proposal_cache
                    .insert_hint(proposal_hint);
                self.subsystems
                    .propose
                    .proposal_cache
                    .insert_proposal(proposal);
            }

            let topology_peers = topology.as_ref();
            let local_peer_id = self.common_config.peer.id().clone();
            for peer in topology_peers {
                if peer == &local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessageWire::with_encoded(
                        Arc::clone(&block_created_wire),
                        Arc::clone(&block_created_encoded),
                    ),
                });
            }
            if !frontier_block_created_ready {
                let proposal_msg = Arc::new(BlockMessage::Proposal(proposal));
                let proposal_encoded =
                    Arc::new(BlockMessageWire::encode_message(proposal_msg.as_ref()));
                for peer in topology_peers {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: BlockMessageWire::with_encoded(
                            Arc::clone(&proposal_msg),
                            Arc::clone(&proposal_encoded),
                        ),
                    });
                }
            }

            if let Some(plan) = rbc_plan.take() {
                // Start body transport immediately after the frontier advertisement posts.
                self.broadcast_rbc_session_plan(plan.primary)?;
                if let Some(dup) = plan.duplicate {
                    self.broadcast_rbc_session_plan(dup)?;
                }
            }

            let relay_envelopes = crate::sumeragi::status::lane_relay_envelopes_snapshot();
            if !relay_envelopes.is_empty() {
                self.subsystems.merge.lane_relay.broadcast(relay_envelopes);
            }

            self.record_phase_sample(PipelinePhase::Propose, proposal_height, view);

            let tx_count = transactions_for_plan.len();
            iroha_logger::debug!(
                height = proposal_height,
                view,
                tx_count,
                queue_len,
                leader_index,
                block_hash = %block_hash,
                "assembled proposal"
            );

            Ok(())
        })();

        if let Err(err) = assembly_result {
            tx_guards.clear();
            for (tx, routing) in original_for_requeue {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction after assembly failure",
                );
            }
            for (tx, routing) in std::mem::take(&mut overflow_transactions) {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction overflowed by RBC budget",
                );
            }
            for (tx, routing) in removed_for_chunk_cap {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction trimmed by RBC chunk cap",
                );
            }
            for (tx, routing) in removed_for_frame_cap {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction trimmed by consensus frame cap",
                );
            }
            return Err(err);
        }
        tx_guards.clear();
        for (tx, routing) in std::mem::take(&mut overflow_transactions) {
            if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                continue;
            }
            self.requeue_accepted_transaction(
                tx,
                routing,
                "failed to requeue transaction overflowed by RBC budget",
            );
        }
        for (tx, routing) in removed_for_chunk_cap {
            if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                continue;
            }
            self.requeue_accepted_transaction(
                tx,
                routing,
                "failed to requeue transaction trimmed by RBC chunk cap",
            );
        }
        for (tx, routing) in removed_for_frame_cap {
            if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                continue;
            }
            self.requeue_accepted_transaction(
                tx,
                routing,
                "failed to requeue transaction trimmed by consensus frame cap",
            );
        }

        Ok(true)
    }

    /// Enforce DA proof/commitment caps before embedding them into a block.
    ///
    /// The current `PoR` proof bundle is tracked by commitments only; we bound proof
    /// openings by the same count until proof summaries are threaded through the
    /// consensus path.
    pub(super) fn validate_da_bundle(&mut self, bundle: &DaCommitmentBundle) -> Result<()> {
        let lane_config = self.state.nexus_snapshot().lane_config.clone();
        validate_da_bundle_caps(
            bundle,
            self.config.da.max_commitments_per_block,
            self.config.da.max_proof_openings_per_block,
        )?;

        for record in &bundle.commitments {
            let policy = lane_config.manifest_policy(record.lane_id);
            let (outcome, cache_outcome) = {
                let da_rbc = &mut self.subsystems.da_rbc;
                manifest_guard_outcome(
                    &mut da_rbc.manifest_cache,
                    &da_rbc.spool_dir,
                    record,
                    policy,
                )
            };
            #[cfg(feature = "telemetry")]
            self.telemetry
                .note_da_manifest_cache(cache_outcome.as_telemetry());
            #[cfg(not(feature = "telemetry"))]
            let _ = cache_outcome;
            match outcome {
                ManifestGuardOutcome::Pass => {}
                ManifestGuardOutcome::Warn(err) => warn!(
                    ?err,
                    ?policy,
                    lane = record.lane_id.as_u32(),
                    epoch = record.epoch,
                    sequence = record.sequence,
                    "audit-only lane missing DA manifest; sealing commitment with warning"
                ),
                ManifestGuardOutcome::Reject(err) => {
                    return Err(eyre!(
                        "DA manifest guard failed for lane {} epoch {} seq {}: {err}",
                        record.lane_id.as_u32(),
                        record.epoch,
                        record.sequence
                    ));
                }
            }

            crate::da::validate_confidential_compute_record(&lane_config, record).map_err(
                |err| {
                    eyre!(
                        "confidential-compute validation failed for lane {} epoch {} seq {}: {err}",
                        record.lane_id.as_u32(),
                        record.epoch,
                        record.sequence
                    )
                },
            )?;
        }

        crate::da::validate_commitment_bundle(bundle, &lane_config)
            .map_err(|err| eyre!("DA commitment bundle failed validation: {err}"))?;

        Ok(())
    }

    pub(super) fn build_consensus_proposal(
        block: &SignedBlock,
        payload_hash: Hash,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        proposer: u32,
        view: u64,
        epoch: u64,
    ) -> crate::sumeragi::consensus::Proposal {
        let header = block.header();
        let parent_hash = header.prev_block_hash().unwrap_or_else(|| {
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]))
        });
        let tx_root = header
            .merkle_root()
            .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from);
        let state_root = header
            .result_merkle_root()
            .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from);
        let block_height = header.height().get();

        crate::sumeragi::consensus::Proposal {
            header: crate::sumeragi::consensus::ConsensusBlockHeader {
                parent_hash,
                tx_root,
                state_root,
                proposer,
                height: block_height,
                view,
                epoch,
                highest_qc,
            },
            payload_hash,
        }
    }

    #[cfg(test)]
    pub(super) fn proposal_backpressure(&mut self) -> ProposalBackpressure {
        self.proposal_backpressure_at(Instant::now())
    }

    pub(super) fn proposal_backpressure_at(&mut self, now: Instant) -> ProposalBackpressure {
        self.subsystems.propose.backpressure_gate.refresh();
        let queue_state = self.subsystems.propose.backpressure_gate.state();
        let blocking_pending = self.blocking_pending_blocks_len_with_progress(now);
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let pending_votes_or_qc = self.pending.pending_blocks.values().any(|pending| {
            if pending.aborted
                || !super::pending_extends_tip(
                    pending.height,
                    pending.block.header().prev_block_hash(),
                    tip_height,
                    tip_hash,
                )
            {
                return false;
            }
            let block_hash = pending.block.hash();
            pending.local_commit_vote_emitted()
                || pending.commit_qc_observed()
                || self.pending_block_has_votes(block_hash, pending.height, pending.view)
                || self.pending_block_has_qc(block_hash, pending.height, pending.view)
        });
        let active_pending = pending_votes_or_qc
            || blocking_pending > self.config.pacemaker.active_pending_soft_limit;
        let rbc_backlog_summary = self.proposal_rbc_backlog_summary();
        let mut rbc_backlog = self.rbc_backlog_exceeds_pacemaker_soft_limits(rbc_backlog_summary);
        let queue_depths = status::worker_queue_depth_snapshot();
        let consensus_queue_backpressure = consensus_queue_backpressure(
            queue_depths,
            self.config.queues.block_payload,
            self.config.queues.rbc_chunks,
        );
        let relay_backpressure = if self.backpressure_override_due(now) {
            // Liveness override: don't let prolonged relay/RBC backpressure stall proposals.
            rbc_backlog = false;
            false
        } else {
            self.relay_backpressure_active(now, self.rebroadcast_cooldown())
        };
        ProposalBackpressure {
            queue_state,
            active_pending,
            rbc_backlog,
            relay_backpressure,
            consensus_queue_backpressure,
        }
    }

    pub(super) fn proposal_scan_budget(&self, max_in_block: NonZeroUsize) -> usize {
        max_in_block
            .get()
            .saturating_mul(self.config.block.proposal_queue_scan_multiplier.get())
    }

    pub(super) fn filter_committed_transactions_for_proposal(
        state: &State,
        tx_guards: Vec<crate::queue::TransactionGuard>,
        transactions: Vec<AcceptedTransaction<'static>>,
        routing_decisions: Vec<RoutingDecision>,
        tx_sizes: Vec<usize>,
        height: u64,
        view: u64,
    ) -> (
        Vec<crate::queue::TransactionGuard>,
        Vec<AcceptedTransaction<'static>>,
        Vec<RoutingDecision>,
        Vec<usize>,
        usize,
    ) {
        let mut retained_guards = Vec::with_capacity(tx_guards.len());
        let mut retained_transactions = Vec::with_capacity(transactions.len());
        let mut retained_routing = Vec::with_capacity(routing_decisions.len());
        let mut retained_sizes = Vec::with_capacity(tx_sizes.len());
        let mut dropped = 0usize;

        for (((guard, tx), routing), size) in tx_guards
            .into_iter()
            .zip(transactions.into_iter())
            .zip(routing_decisions.into_iter())
            .zip(tx_sizes.into_iter())
        {
            if state.has_committed_transaction(tx.hash()) {
                dropped = dropped.saturating_add(1);
                continue;
            }
            retained_guards.push(guard);
            retained_transactions.push(tx);
            retained_routing.push(routing);
            retained_sizes.push(size);
        }

        if dropped > 0 {
            debug!(
                height,
                view, dropped, "dropping committed transactions from proposal batch"
            );
        }

        (
            retained_guards,
            retained_transactions,
            retained_routing,
            retained_sizes,
            dropped,
        )
    }

    fn maybe_rebroadcast_cached_proposal(
        &mut self,
        height: u64,
        view: u64,
        pending_queue_len: usize,
        now: Instant,
    ) {
        let Some(proposal) = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view)
            .cloned()
        else {
            return;
        };

        let (pending_block, block_hash) = {
            let Some(pending) = self.pending.pending_blocks.values().find(|pending| {
                !pending.aborted
                    && pending.height == height
                    && pending.view == view
                    && pending.payload_hash == proposal.payload_hash
            }) else {
                trace!(
                    height,
                    view,
                    payload = %proposal.payload_hash,
                    "skipping cached proposal rebroadcast: pending block not found"
                );
                return;
            };
            (pending.block.clone(), pending.block.hash())
        };

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let proposal_roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
        if proposal_roster.is_empty() {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast: empty commit topology"
            );
            return;
        }
        let mut topology = super::network_topology::Topology::new(proposal_roster);
        let leader_index = match self.leader_index_for(&mut topology, height, view) {
            Ok(idx) => idx,
            Err(err) => {
                warn!(
                    ?err,
                    height, view, "failed to compute leader index for cached proposal rebroadcast"
                );
                return;
            }
        };

        let Some(local_pos) = topology.position(self.common_config.peer.id().public_key()) else {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast: local peer not in validator set"
            );
            return;
        };
        if local_pos != leader_index {
            trace!(
                height,
                view,
                local_idx = local_pos,
                leader_index,
                "skipping cached proposal rebroadcast: local peer is not leader"
            );
            return;
        }

        let cooldown = self.payload_rebroadcast_cooldown();
        if self.relay_backpressure_active(now, cooldown) {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast due to relay backpressure"
            );
            return;
        }
        if !self
            .proposal_rebroadcast_log
            .allow(block_hash, now, cooldown)
        {
            trace!(
                height,
                view,
                block = %block_hash,
                cooldown_ms = cooldown.as_millis(),
                "skipping cached proposal rebroadcast due to cooldown"
            );
            return;
        }

        let local_peer_id = self.common_config.peer.id().clone();
        let Some(block_created) =
            self.frontier_block_created_for_proposal_wire(&pending_block, &proposal)
        else {
            warn!(
                height,
                view,
                block = %block_hash,
                "skipping cached proposal rebroadcast because frontier metadata could not be rebuilt"
            );
            return;
        };
        let block_msg = Arc::new(BlockMessage::BlockCreated(block_created));
        let block_encoded = Arc::new(BlockMessageWire::encode_message(block_msg.as_ref()));
        for peer in topology.iter() {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&block_msg),
                    Arc::clone(&block_encoded),
                ),
            });
        }
        if pending_queue_len > 0 {
            iroha_logger::info!(
                height,
                view,
                block = %block_hash,
                "rebroadcasting cached proposal"
            );
        } else {
            debug!(
                height,
                view,
                block = %block_hash,
                "rebroadcasting cached proposal"
            );
        }
    }

    pub(super) fn on_pacemaker_backpressure_deferral(
        &mut self,
        now: Instant,
        state: BackpressureState,
    ) {
        let blocking_pending = self.blocking_pending_blocks_len_with_progress(now);
        let active_pending = blocking_pending > self.config.pacemaker.active_pending_soft_limit;
        let rbc_backlog_summary = self.rbc_backlog_summary();
        let rbc_backlog = self.rbc_backlog_exceeds_pacemaker_soft_limits(rbc_backlog_summary);
        let relay_backpressure = self.relay_backpressure_active(now, self.rebroadcast_cooldown());
        super::status::inc_pacemaker_backpressure_deferrals();
        #[cfg(feature = "telemetry")]
        {
            self.telemetry.inc_pacemaker_backpressure_deferrals();
        }
        debug!(
            ?now,
            tx_queue_depth = state.queued(),
            tx_queue_capacity = state.capacity().get(),
            active_pending,
            blocking_pending,
            pending_soft_limit = self.config.pacemaker.active_pending_soft_limit,
            rbc_backlog,
            rbc_sessions = rbc_backlog_summary.sessions_pending,
            rbc_missing_chunks = rbc_backlog_summary.missing_chunks_total,
            rbc_session_soft_limit = self.config.pacemaker.rbc_backlog_session_soft_limit,
            rbc_chunk_soft_limit = self.config.pacemaker.rbc_backlog_chunk_soft_limit,
            relay_backpressure,
            "Pacemaker deferred proposal assembly due to backpressure"
        );
    }

    fn maybe_rebroadcast_new_view_votes(&mut self, height: u64, now: Instant) {
        if self.is_observer() {
            return;
        }
        let target = self
            .subsystems
            .propose
            .new_view_tracker
            .entries
            .iter()
            .rev()
            .find(|((entry_height, _), _)| *entry_height == height)
            .map(|(key, entry)| (*key, entry.highest_qc.subject_block_hash));
        let Some(((target_height, target_view), block_hash)) = target else {
            return;
        };
        let cooldown = self.rebroadcast_cooldown();
        if !self.new_view_rebroadcast_log.allow(
            block_hash,
            target_height,
            target_view,
            now,
            cooldown,
        ) {
            trace!(
                height = target_height,
                view = target_view,
                block = ?block_hash,
                cooldown_ms = cooldown.as_millis(),
                "skipping NEW_VIEW vote rebroadcast due to cooldown"
            );
            return;
        }
        let rebroadcasted = self.rebroadcast_block_votes(
            crate::sumeragi::consensus::Phase::NewView,
            block_hash,
            target_height,
            target_view,
            false,
        );
        if rebroadcasted == 0 {
            debug!(
                height = target_height,
                view = target_view,
                block = ?block_hash,
                "no NEW_VIEW votes available for rebroadcast"
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_pacemaker_propose_ready(&mut self, now: Instant) -> bool {
        trace!(?now, "pacemaker evaluating NEW_VIEW gating");
        if self.round_liveness_isolated() {
            self.subsystems.propose.pacemaker.next_deadline = now
                .checked_add(
                    self.subsystems
                        .propose
                        .pacemaker
                        .propose_interval
                        .max(Duration::from_millis(1)),
                )
                .unwrap_or(now);
            debug!("suppressing proposal path while round liveness catch-up isolation is active");
            return false;
        }
        let prev_attempt = self.subsystems.propose.last_pacemaker_attempt.replace(now);
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let pending_queue_len = self.queue.queued_len();
        let active_pending = self.active_pending_blocks_len_for_tip(tip_height, tip_hash);
        let view_height = tip_height;
        let committed_height = view_height as u64;

        // Drop NEW_VIEW entries that point at already-committed heights so the pacemaker
        // cannot re-propose a finalized height after a commit.
        self.subsystems
            .propose
            .new_view_tracker
            .prune(committed_height);

        self.promote_locked_qc_to_highest_if_needed("pacemaker");

        let da_enabled = self.runtime_da_enabled();
        let committed_qc = self.latest_committed_qc();
        let precommit_qc = precommit_qc_for_view_change(self.highest_qc, committed_qc);
        let desired_height = active_round_height(self.highest_qc, committed_qc, committed_height);
        let tracked_height = desired_height.min(committed_height.saturating_add(1));
        if tracked_height != desired_height {
            debug!(
                desired_height,
                tracked_height,
                committed_height,
                "clamping pacemaker active round height to local commit horizon"
            );
        }
        let (consensus_mode, _, _) = self.consensus_context_for_height(tracked_height);
        let topology_peers = self.roster_for_live_vote_with_mode(tracked_height, consensus_mode);
        let active_topology_peers = topology_peers.clone();
        let tracked_view = self.phase_tracker.current_view(tracked_height).unwrap_or(0);
        if self.proposal_gated_by_missing_dependencies(tracked_height) {
            self.subsystems.propose.pacemaker.next_deadline = now
                .checked_add(
                    self.subsystems
                        .propose
                        .pacemaker
                        .propose_interval
                        .max(Duration::from_millis(1)),
                )
                .unwrap_or(now);
            debug!(
                height = tracked_height,
                view = tracked_view,
                "deferring proposal while canonical dependencies are still recovering"
            );
            return false;
        }
        // Drop stale NEW_VIEW entries so proposals cannot regress after higher QCs arrive.
        self.subsystems
            .propose
            .new_view_tracker
            .drop_below_height(tracked_height);

        if topology_peers.is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                tracked_height,
                tracked_view,
                tip_hash,
                pending_queue_len,
                now,
                ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "pacemaker_propose_ready",
            );
            return false;
        }

        let mut topology = super::network_topology::Topology::new(topology_peers);
        let mut required = topology.min_votes_for_commit();
        let local_peer_id = self.common_config.peer.id().clone();
        let local_idx = self.local_validator_index_for_topology(&topology);
        let local_peer = local_idx.map(|_| local_peer_id.clone());

        let has_queue_work = pending_queue_len > 0;
        if da_enabled && !has_queue_work {
            trace!(
                da_enabled,
                "DA enabled and transaction queue is empty; checking internal work"
            );
        }

        let online_peers = self
            .network
            .online_peers(|peers| super::count_online_validators(peers, topology.as_ref()));
        // `online_peers` counts only remote peers in the current validator roster; include the
        // local node if it is part of the commit topology so we do not stall when exactly
        // `required` validators are up.
        let online_total = online_peers + usize::from(local_idx.is_some());
        let mut view_age = self.phase_tracker.view_age(tracked_height, now);
        if view_age.is_none() {
            self.phase_tracker.start_new_round(tracked_height, now);
            view_age = self.phase_tracker.view_age(tracked_height, now);
        }
        let current_view = self.phase_tracker.current_view(tracked_height);
        self.clear_consensus_recovery_for_round(tracked_height, current_view.unwrap_or(0));
        let bootstrap_view = current_view.is_none_or(|view| view == 0);
        if let Some(view) = current_view {
            // Avoid proposing stale views by pruning NEW_VIEW entries below the local view.
            self.subsystems
                .propose
                .new_view_tracker
                .drop_below_view(tracked_height, view);
        }
        self.reconcile_new_view_tracker_with_local_blocks();
        if pending_queue_len > 0
            && self
                .subsystems
                .propose
                .propose_attempt_monitor
                .should_log(now)
        {
            let since_last_attempt = prev_attempt.map(|ts| now.saturating_duration_since(ts));
            let since_last_success = self
                .subsystems
                .propose
                .last_successful_proposal
                .map(|ts| now.saturating_duration_since(ts));
            iroha_logger::info!(
                height = tracked_height,
                view = current_view,
                view_age_ms = view_age.map(|d| d.as_millis()),
                pending_blocks = self.pending.pending_blocks.len(),
                queue_len = pending_queue_len,
                topology_len = topology.as_ref().len(),
                required,
                online_total,
                since_last_attempt_ms = since_last_attempt.map(|d| d.as_millis()),
                since_last_success_ms = since_last_success.map(|d| d.as_millis()),
                "pacemaker attempt with queued transactions"
            );
        }
        if online_total < required {
            let throttle_hash = tip_hash.unwrap_or_else(|| {
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]))
            });
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::InsufficientOnlinePeers,
                tracked_height,
                current_view.unwrap_or_default(),
                throttle_hash,
                now,
                Duration::from_secs(5),
            ) {
                warn!(
                    queue_len = pending_queue_len,
                    height = tracked_height,
                    view = current_view,
                    required,
                    online_peers,
                    online_total,
                    age_ms = view_age.map(|age| age.as_millis()),
                    suppressed_since_last,
                    "online peer count below commit quorum; continuing proposal flow"
                );
            }
        }

        if required == 1 && topology.as_ref().len() == 1 {
            if let Some(local_peer) = local_peer.as_ref() {
                if let Some(qc) = precommit_qc {
                    // Seed NEW_VIEW tracker so single-validator networks can progress.
                    if self.highest_qc.is_none_or(|current| {
                        let incoming = (qc.height, qc.view);
                        let existing = (current.height, current.view);
                        incoming > existing
                            || (incoming == existing
                                && current.phase != crate::sumeragi::consensus::Phase::Commit)
                    }) {
                        self.highest_qc = Some(qc);
                        super::status::set_highest_qc(qc.height, qc.view);
                        super::status::set_highest_qc_hash(qc.subject_block_hash);
                    }
                    self.subsystems.propose.new_view_tracker.record(
                        qc.height.saturating_add(1),
                        0,
                        local_peer.clone(),
                        qc,
                    );
                }
            }
        }

        if pending_queue_len > 0 {
            iroha_logger::debug!(
                queue_len = pending_queue_len,
                topology_len = topology.as_ref().len(),
                required,
                height = tracked_height,
                "pacemaker evaluating proposal assembly with queued transactions"
            );
            if active_pending > 0 {
                iroha_logger::debug!(
                    height = tracked_height,
                    pending = active_pending,
                    "pending block already assembled for current slot; waiting for view-change"
                );
            }
        }
        let new_view_summary: Vec<String> = self
            .subsystems
            .propose
            .new_view_tracker
            .entries
            .iter()
            .map(|((h, v), entry)| format!("{h}:{v}={}", entry.senders.len()))
            .collect();
        debug!(
            height = tracked_height,
            required,
            local_idx = ?local_idx,
            forced = ?self.subsystems.propose.forced_view_after_timeout,
            new_view_slots = ?new_view_summary,
            "pacemaker NEW_VIEW snapshot before selection"
        );

        let mut candidate = self.subsystems.propose.new_view_tracker.select_with_quorum(
            required,
            local_peer.as_ref(),
            topology.as_ref(),
        );
        if pending_queue_len > 0 {
            if let Some((forced_height, forced_view)) =
                self.subsystems.propose.forced_view_after_timeout
            {
                if let Some(qc) = precommit_qc {
                    let should_override = candidate.as_ref().is_none_or(|selection| {
                        selection.key.0 != forced_height || selection.key.1 < forced_view
                    });
                    if forced_height == qc.height.saturating_add(1) && should_override {
                        candidate = Some(NewViewSelection {
                            key: (forced_height, forced_view),
                            quorum: required,
                            highest_qc: qc,
                        });
                        self.subsystems.propose.forced_view_after_timeout = None;
                    }
                }
            }
        }

        if candidate.is_none() {
            if let Some((forced_height, forced_view)) =
                self.subsystems.propose.forced_view_after_timeout
            {
                if let Some(qc) = precommit_qc {
                    if forced_height == qc.height.saturating_add(1) {
                        candidate = Some(NewViewSelection {
                            key: (forced_height, forced_view),
                            quorum: required,
                            highest_qc: qc,
                        });
                        self.subsystems.propose.forced_view_after_timeout = None;
                    }
                }
            }
        }

        let Some(selection) = candidate.or_else(|| {
            // Fallback: bootstrap the first view using the latest committed QC when no NEW_VIEW
            // quorum has been observed yet. This prevents the pacemaker from stalling indefinitely
            // at startup before any view changes occur.
            if !bootstrap_view {
                return None;
            }
            let _local_idx = local_idx?;
            let qc = self.latest_committed_qc()?;
            Some(NewViewSelection {
                key: (qc.height.saturating_add(1), 0),
                quorum: required,
                highest_qc: qc,
            })
        }) else {
            debug!(
                queue_len = pending_queue_len,
                height = tracked_height,
                required,
                local_idx = ?local_idx,
                new_view_slots = ?new_view_summary,
                "deferring proposal: awaiting NEW_VIEW quorum"
            );
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            return false;
        };

        let (height, view_idx) = selection.key;
        let quorum = selection.quorum;
        let mut highest_qc = selection.highest_qc;

        debug!(
            height,
            view = view_idx,
            quorum,
            local_idx = ?local_idx,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            new_view_slots = ?new_view_summary,
            "selected NEW_VIEW candidate"
        );
        let epoch = self.epoch_for_height(height);
        let precommit_votes_at_view = self
            .vote_log
            .values()
            .filter(|vote| {
                vote.phase == crate::sumeragi::consensus::Phase::Commit
                    && vote.height == height
                    && vote.view == view_idx
                    && vote.epoch == epoch
                    && self.block_known_locally(vote.block_hash)
            })
            .count();

        // Avoid rebuilding multiple proposals for the same (height, view) slot. Reassembly in the
        // same view causes double-voting and scatters QC collection; wait for the existing
        // proposal to gather votes or transition via a view change instead.
        if self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view_idx)
            .is_some()
        {
            // Rebroadcast cached proposals when the leader is still responsible for the slot so
            // peers that missed the initial messages can recover without forcing a view change.
            self.maybe_rebroadcast_cached_proposal(height, view_idx, pending_queue_len, now);
            if precommit_votes_at_view > 0 {
                debug!(
                    height,
                    view = view_idx,
                    precommit_votes = precommit_votes_at_view,
                    queue_len = pending_queue_len,
                    "proposal already cached for this slot; precommit votes observed, deferring view change"
                );
                return false;
            }
            let quorum_timeout = self.quorum_timeout(da_enabled);
            let queue_depths = super::status::worker_queue_depth_snapshot();
            let consensus_queue_backlog =
                self.consensus_queue_backlog_blocks_near_quorum_timeout(queue_depths);
            let (consensus_mode, _, _) = self.consensus_context_for_height(height);
            let mut commit_roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
            if commit_roster.is_empty() {
                commit_roster = self.effective_commit_topology();
            }
            let commit_topology = super::network_topology::Topology::new(commit_roster);
            let mut missing_local_data = false;
            let mut rbc_session_incomplete = false;
            for pending in self.pending.pending_blocks.values().filter(|pending| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            }) {
                let payload_available = da_enabled
                    && Self::payload_available_for_da(
                        &self.subsystems.da_rbc.rbc.sessions,
                        &self.subsystems.da_rbc.rbc.status_handle,
                        pending,
                    );
                if !da_enabled || payload_available {
                    continue;
                }
                missing_local_data = true;
                let rbc_key = (pending.block.hash(), pending.height, pending.view);
                rbc_session_incomplete |= self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&rbc_key)
                    .is_some_and(|session| {
                        if session.is_invalid() || session.delivered {
                            return false;
                        }
                        let progress_started = session.total_chunks() != 0
                            || session.received_chunks() != 0
                            || !session.ready_signatures.is_empty()
                            || self.subsystems.da_rbc.rbc.pending.contains_key(&rbc_key);
                        if !progress_started {
                            return false;
                        }
                        let missing_chunks = session.total_chunks() != 0
                            && session.received_chunks() < session.total_chunks();
                        let ready_quorum = session.ready_signatures.len()
                            >= self.rbc_deliver_quorum(&commit_topology);
                        missing_chunks || !ready_quorum
                    });
                if rbc_session_incomplete {
                    break;
                }
            }
            let effective_quorum_timeout = cached_slot_effective_quorum_timeout(
                quorum_timeout,
                self.rebroadcast_cooldown(),
                precommit_votes_at_view,
                quorum,
                missing_local_data,
                consensus_queue_backlog,
                rbc_session_incomplete,
            );
            let cached_wait_age = self
                .pending
                .pending_blocks
                .values()
                .filter(|pending| {
                    !pending.aborted && pending.height == height && pending.view == view_idx
                })
                .map(|pending| pending.progress_age(now))
                .max();
            if effective_quorum_timeout != Duration::ZERO
                && cached_wait_age.is_some_and(|age| age >= effective_quorum_timeout)
            {
                let wait_age_ms = cached_wait_age.map(|age| age.as_millis()).unwrap_or(0);
                let already_forced = self
                    .subsystems
                    .propose
                    .forced_view_after_timeout
                    .is_some_and(|(forced_height, forced_view)| {
                        forced_height == height && forced_view > view_idx
                    });
                if already_forced {
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        wait_age_ms,
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        base_quorum_timeout_ms = quorum_timeout.as_millis(),
                        forced = ?self.subsystems.propose.forced_view_after_timeout,
                        "cached proposal slot stalled past quorum timeout; awaiting scheduled view change"
                    );
                } else if let Some(wait_remaining) = cached_slot_timeout_hysteresis_remaining(
                    consensus_mode,
                    effective_quorum_timeout,
                    self.subsystems.propose.last_cached_slot_timeout_trigger,
                    height,
                    view_idx,
                    now,
                ) {
                    let next_streak = next_cached_slot_timeout_streak(
                        self.subsystems.propose.last_cached_slot_timeout_trigger,
                        height,
                        view_idx,
                    );
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        wait_age_ms,
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        base_quorum_timeout_ms = quorum_timeout.as_millis(),
                        hysteresis_wait_ms = wait_remaining.as_millis(),
                        timeout_streak = next_streak,
                        "cached proposal slot stalled past quorum timeout; waiting for NPoS timeout hysteresis"
                    );
                } else {
                    let timeout_streak = next_cached_slot_timeout_streak(
                        self.subsystems.propose.last_cached_slot_timeout_trigger,
                        height,
                        view_idx,
                    );
                    let committed_height = self.committed_height_snapshot();
                    let contiguous_frontier = height == committed_height.saturating_add(1);
                    let same_slot_recovery_active = contiguous_frontier
                        && self.frontier_recovery_quorum_timeout_same_height_recovery_active(
                            height,
                            view_idx,
                            now,
                            queue_depths,
                        );
                    if same_slot_recovery_active {
                        let seeded =
                            self.seed_frontier_recovery_for_quorum_timeout(height, view_idx, now);
                        debug!(
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            wait_age_ms,
                            quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                            base_quorum_timeout_ms = quorum_timeout.as_millis(),
                            timeout_streak,
                            seeded_frontier_owner = seeded,
                            "cached proposal slot quorum-timeout suppressed while same-slot frontier recovery remains active"
                        );
                    } else if contiguous_frontier {
                        warn!(
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            wait_age_ms,
                            quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                            base_quorum_timeout_ms = quorum_timeout.as_millis(),
                            timeout_streak,
                            "cached proposal slot stalled past quorum timeout; routing through unified frontier recovery"
                        );
                        self.seed_frontier_recovery_for_quorum_timeout(height, view_idx, now);
                        let _ = self.advance_frontier_recovery(
                            "quorum_timeout",
                            height,
                            view_idx,
                            false,
                            true,
                            true,
                            now,
                        );
                    } else {
                        warn!(
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            wait_age_ms,
                            quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                            base_quorum_timeout_ms = quorum_timeout.as_millis(),
                            timeout_streak,
                            "cached proposal slot stalled past quorum timeout; forcing view change"
                        );
                        self.trigger_view_change_with_cause(
                            height,
                            view_idx,
                            ViewChangeCause::QuorumTimeout,
                        );
                    }
                    self.subsystems.propose.last_cached_slot_timeout_trigger =
                        Some(CachedSlotTimeoutTrigger {
                            height,
                            view: view_idx,
                            at: now,
                            streak: timeout_streak,
                        });
                }
                self.maybe_rebroadcast_new_view_votes(height, now);
                return false;
            }
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already cached for this slot; waiting for votes/view change"
                );
                // Provide one cooldown-gated NEW_VIEW assist while waiting so peers that
                // missed prior messages can converge without immediate view rotation.
                self.maybe_rebroadcast_new_view_votes(height, now);
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "proposal already cached for this slot; deferring reassembly"
                );
            }
            return false;
        }

        if precommit_votes_at_view > 0 {
            debug!(
                height,
                view = view_idx,
                precommit_votes = precommit_votes_at_view,
                queue_len = pending_queue_len,
                "deferring proposal: precommit votes already observed for this view"
            );
            return false;
        }

        if height == self.committed_height_snapshot().saturating_add(1) {
            let _ = self.seed_frontier_slot_from_same_height_evidence(
                height,
                view_idx,
                now,
                "missing_qc",
                true,
            );
        }

        if let Some(block_hash) = self.authoritative_slot_owner_hash(height, view_idx) {
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    block = %block_hash,
                    "authoritative BlockCreated already owns this slot; waiting for progress"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    block = %block_hash,
                    "authoritative BlockCreated already owns this slot; deferring reassembly"
                );
            }
            return false;
        }

        if self.slot_has_proposal_evidence(height, view_idx) {
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already observed for this slot; waiting for progress"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "proposal already observed for this slot; deferring reassembly"
                );
            }
            return false;
        }

        if let Some((owner_hash, owner_view)) = self
            .frontier_slot_live_local_owner_for_round(height, view_idx)
            .filter(|(_, owner_view)| *owner_view < view_idx)
        {
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    owner = %owner_hash,
                    owner_view,
                    queue_len = pending_queue_len,
                    "same-height frontier owner is still locally live for this round; deferring reassembly"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    owner = %owner_hash,
                    owner_view,
                    "same-height frontier owner is still locally live for this round; deferring reassembly"
                );
            }
            return false;
        }

        if height == self.committed_height_snapshot().saturating_add(1)
            && let Some(existing_vote) =
                self.local_same_height_vote(height, self.epoch_for_height(height))
        {
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    voted_view = existing_vote.view,
                    voted_phase = ?existing_vote.phase,
                    voted_block = %existing_vote.block_hash,
                    "same-height local vote history already anchors the frontier; deferring fresh proposal assembly"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    voted_view = existing_vote.view,
                    voted_phase = ?existing_vote.phase,
                    voted_block = %existing_vote.block_hash,
                    "same-height local vote history already anchors the frontier; deferring fresh proposal assembly"
                );
            }
            return false;
        }

        if let Some((pending_age, pending_view)) = self
            .pending
            .pending_blocks
            .values()
            .filter(|pending| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            })
            .map(|pending| (pending.age(), pending.view))
            .min_by_key(|(age, _)| *age)
        {
            let quorum_timeout = self.quorum_timeout(da_enabled);
            if quorum_timeout != Duration::ZERO && pending_age < quorum_timeout {
                debug!(
                    height,
                    pending_view,
                    target_view = view_idx,
                    age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = quorum_timeout.as_millis(),
                    queue_len = pending_queue_len,
                    "deferring proposal: pending block still within quorum timeout window"
                );
                return false;
            }
        }

        if let Some((pending_hash, pending_parent)) = self
            .pending
            .pending_blocks
            .iter()
            .find(|(_, pending)| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            })
            .map(|(hash, pending)| (*hash, pending.block.header().prev_block_hash()))
        {
            if pending_block_stale_for_tip(height, pending_parent, tip_height, tip_hash) {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    pending_hash = %pending_hash,
                    pending_parent = ?pending_parent,
                    committed_hash = ?tip_hash,
                    "dropping stale pending proposal that no longer builds on committed chain"
                );
                if let Some((tx_count, requeued, failures, duplicate_failures)) =
                    self.drop_stale_pending_block(pending_hash, height, view_idx)
                {
                    if tx_count > 0 {
                        iroha_logger::info!(
                            height,
                            view = view_idx,
                            tx_count,
                            requeued,
                            failures,
                            duplicate_failures,
                            "requeued transactions from stale pending proposal"
                        );
                    }
                }
            } else {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already pending for this slot; deferring reassembly"
                );
                return false;
            }
        }

        if self.highest_qc.is_none_or(|current| {
            (highest_qc.height, highest_qc.view) > (current.height, current.view)
        }) {
            self.highest_qc = Some(highest_qc);
            super::status::set_highest_qc(highest_qc.height, highest_qc.view);
            super::status::set_highest_qc_hash(highest_qc.subject_block_hash);
        }

        if let Err(reason) = ensure_locked_qc_allows(self.locked_qc, highest_qc) {
            match reason {
                LockedQcRejection::HeightRegressed { locked, highest } => {
                    let Some(lock) = self.locked_qc else {
                        return false;
                    };
                    self.promote_locked_qc_to_highest_if_needed("proposal");
                    iroha_logger::info!(
                        locked_height = locked,
                        highest_height = highest,
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        lock_hash = %lock.subject_block_hash,
                        "deferring proposal: highest QC lags locked QC; retrying after promotion"
                    );
                    return false;
                }
                LockedQcRejection::HashMismatch { .. } => {
                    let locked_hash = self.locked_qc.map(|qc| qc.subject_block_hash);
                    let locked_missing =
                        locked_hash.is_some_and(|hash| !self.block_known_for_lock(hash));
                    let highest_missing =
                        !self.block_payload_available_for_progress(highest_qc.subject_block_hash);
                    if locked_missing {
                        iroha_logger::warn!(
                            ?reason,
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            locked_hash = ?locked_hash,
                            "clearing locked QC that is missing from kura"
                        );
                        self.locked_qc = Some(highest_qc);
                        super::status::set_locked_qc(
                            highest_qc.height,
                            highest_qc.view,
                            Some(highest_qc.subject_block_hash),
                        );
                    }
                    if highest_missing {
                        if self
                            .suppress_committed_edge_conflicting_highest_qc(highest_qc, "proposal")
                        {
                            self.clear_missing_block_view_change(&highest_qc.subject_block_hash);
                            return false;
                        }
                        let first_defer_in_round = self
                            .mark_highest_qc_missing_defer_for_round(height, view_idx, highest_qc);
                        if first_defer_in_round {
                            self.observe_new_view_highest_qc(highest_qc);
                        }
                        if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                            ProposalDeferWarningKind::HighestQcMissing,
                            height,
                            view_idx,
                            highest_qc.subject_block_hash,
                            now,
                            Duration::from_secs(5),
                        ) {
                            iroha_logger::warn!(
                                ?reason,
                                height,
                                view = view_idx,
                                queue_len = pending_queue_len,
                                highest_hash = ?highest_qc.subject_block_hash,
                                locked_hash = ?locked_hash,
                                suppressed_since_last,
                                first_defer_in_round,
                                "highest QC block missing locally; deferring proposal"
                            );
                        }
                        return false;
                    }
                    if !locked_missing {
                        let lock_lag_deferred = self.defer_highest_qc_update_for_lock_catchup(
                            height, view_idx, highest_qc, now, "proposal",
                        );
                        iroha_logger::info!(
                            ?reason,
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            lock_lag_deferred,
                            "deferring proposal: locked QC prevents proposal"
                        );
                        return false;
                    }
                }
            }
        }

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
                        view = view_idx,
                        highest_height = highest_qc.height,
                        highest_hash = %highest_qc.subject_block_hash,
                        locked_height = new_lock.height,
                        locked_hash = %new_lock.subject_block_hash,
                        "resetting locked QC to committed chain to unblock proposal"
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

        self.maybe_realign_locked_to_committed_tip();

        if !self.highest_qc_extends_locked(highest_qc) {
            let _ = self.defer_highest_qc_update_for_lock_catchup(
                height, view_idx, highest_qc, now, "proposal",
            );
            if let Some(lock) = self.locked_qc
                && (highest_qc.height, highest_qc.view) <= (lock.height, lock.view)
            {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    highest_height = highest_qc.height,
                    highest_hash = %highest_qc.subject_block_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    queue_len = pending_queue_len,
                    "replacing non-extending highest QC with locked QC"
                );
                highest_qc = lock;
            } else if let Some(new_lock) = realign_locked_to_committed_if_extends(
                self.locked_qc,
                self.latest_committed_qc(),
                highest_qc,
                |hash, height| self.parent_hash_for(hash, height),
            ) {
                if self.locked_qc != Some(new_lock) {
                    iroha_logger::info!(
                        height,
                        view = view_idx,
                        locked_height = new_lock.height,
                        locked_hash = %new_lock.subject_block_hash,
                        highest_height = highest_qc.height,
                        highest_hash = %highest_qc.subject_block_hash,
                        queue_len = pending_queue_len,
                        "realigning locked QC to committed chain for proposal assembly"
                    );
                    self.locked_qc = Some(new_lock);
                    super::status::set_locked_qc(
                        new_lock.height,
                        new_lock.view,
                        Some(new_lock.subject_block_hash),
                    );
                }
                if !self.highest_qc_extends_locked(highest_qc) {
                    iroha_logger::info!(
                        height,
                        view = view_idx,
                        highest_height = highest_qc.height,
                        highest_hash = ?highest_qc.subject_block_hash,
                        locked_height = ?self.locked_qc.map(|qc| qc.height),
                        locked_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                        queue_len = pending_queue_len,
                        "deferring proposal: highest QC does not extend locked chain"
                    );
                    return false;
                }
            } else {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    highest_height = highest_qc.height,
                    highest_hash = ?highest_qc.subject_block_hash,
                    locked_height = ?self.locked_qc.map(|qc| qc.height),
                    locked_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                    queue_len = pending_queue_len,
                    "deferring proposal: highest QC does not extend locked chain"
                );
                return false;
            }
        }

        let proposal_roster = active_topology_peers;
        if proposal_roster.is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                height,
                view_idx,
                Some(highest_qc.subject_block_hash),
                pending_queue_len,
                now,
                ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "proposal_roster_selected_empty",
            );
            return false;
        }
        topology = super::network_topology::Topology::new(proposal_roster);
        required = topology.min_votes_for_commit();

        let leader_index = match self.leader_index_for(&mut topology, height, view_idx) {
            Ok(idx) => idx,
            Err(err) => {
                warn!(
                    ?err,
                    height,
                    view = view_idx,
                    "failed to compute leader index"
                );
                return false;
            }
        };

        let Some(local_pos) = topology.position(self.common_config.peer.id().public_key()) else {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "deferring proposal: local node not part of validator set"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "local node not part of validator set"
                );
            }
            return false;
        };
        if local_pos != leader_index {
            let leader_peer = topology.iter().next().cloned();
            if pending_queue_len > 0 {
                iroha_logger::debug!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    queue_len = pending_queue_len,
                    "deferring proposal: local node is not leader for this round"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    "local node is not leader for this round"
                );
            }
            return false;
        }

        let Ok(local_idx_val) = u32::try_from(local_pos) else {
            warn!(local_pos, "local validator index exceeds u32 limits");
            return false;
        };

        let has_internal_work = if has_queue_work {
            false
        } else {
            let prev_block = resolve_prev_block_for_proposal(
                height,
                &highest_qc,
                &self.kura,
                &self.pending.pending_blocks,
            );
            self.internal_proposal_work(height, prev_block.as_deref())
                .has_work()
        };
        if !has_queue_work && !has_internal_work {
            trace!(
                height,
                view = view_idx,
                "deferring proposal: no queued transactions or internal work"
            );
            return false;
        }

        debug!(
            height,
            view = view_idx,
            quorum,
            required,
            leader_index,
            "NEW_VIEW quorum satisfied; assembling proposal"
        );

        let leader_peer = topology.iter().next().cloned();

        iroha_logger::info!(
            height,
            view = view_idx,
            leader_index,
            leader = ?leader_peer,
            local_idx = local_pos,
            quorum,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            "starting proposal assembly"
        );

        let view_snapshot = None;
        let assembled = match self.assemble_and_broadcast_proposal(
            height,
            view_idx,
            highest_qc,
            &mut topology,
            leader_index,
            local_idx_val,
            view_snapshot,
            now,
        ) {
            Ok(assembled) => assembled,
            Err(err) => {
                warn!(?err, height, view = view_idx, "failed to assemble proposal");
                return false;
            }
        };

        if !assembled {
            return false;
        }

        iroha_logger::info!(
            height,
            view = view_idx,
            local_idx = local_idx_val,
            leader_index,
            quorum,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            highest_hash = %highest_qc.subject_block_hash,
            "proposal assembly succeeded"
        );

        self.subsystems
            .propose
            .new_view_tracker
            .remove(height, view_idx);
        self.subsystems.propose.last_cached_slot_timeout_trigger = None;
        self.subsystems.propose.last_missing_qc_timeout_trigger = None;
        self.subsystems.propose.last_successful_proposal = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProposalBackpressure, cached_slot_timeout_hysteresis_remaining,
        consensus_queue_backpressure, da_payload_budget, next_cached_slot_timeout_streak,
        trim_batch_for_size_cap,
    };
    use crate::queue::BackpressureState;
    use crate::sumeragi::status;
    use std::num::NonZeroUsize;
    use std::time::{Duration, Instant};

    #[test]
    fn da_payload_budget_caps_to_rbc_budget() {
        let budget = da_payload_budget(1, 8 * 1024, 1024, None, 64 * 1024);
        let rbc_budget =
            usize::try_from(super::super::RBC_MAX_TOTAL_CHUNKS).expect("fits in usize");
        assert_eq!(budget, rbc_budget.min(8 * 1024));
    }

    #[test]
    fn da_payload_budget_honors_block_payload_cap() {
        let cap = NonZeroUsize::new(4096).expect("non-zero");
        let budget = da_payload_budget(256 * 1024, 32 * 1024, 1024, Some(cap), 64 * 1024);
        assert_eq!(budget, 4096);
    }

    #[test]
    fn da_payload_budget_honors_pending_caps() {
        let budget = da_payload_budget(256 * 1024, 4 * 1024, 1, None, 64 * 1024);
        assert_eq!(budget, 4 * 1024);
    }

    #[test]
    fn consensus_queue_backpressure_flags_full_queues() {
        let mut depths = status::WorkerQueueDepthSnapshot::default();
        depths.block_payload_rx = 2;
        assert!(consensus_queue_backpressure(depths, 2, 10));

        depths.block_payload_rx = 1;
        depths.rbc_chunk_rx = 5;
        assert!(consensus_queue_backpressure(depths, 10, 5));

        depths.block_payload_rx = 1;
        depths.rbc_chunk_rx = 4;
        assert!(!consensus_queue_backpressure(depths, 10, 5));
    }

    #[test]
    fn trim_batch_for_size_cap_removes_multiple_entries() {
        let mut txs = vec![1, 2, 3, 4];
        let mut routes = vec![10, 11, 12, 13];
        let mut sizes = vec![10, 10, 10, 10];
        let mut removed = Vec::new();

        let removed_count =
            trim_batch_for_size_cap(&mut txs, &mut routes, &mut sizes, &mut removed, 15);

        assert_eq!(removed_count, 2);
        assert_eq!(txs, vec![1, 2]);
        assert_eq!(routes, vec![10, 11]);
        assert_eq!(sizes, vec![10, 10]);
        assert_eq!(removed.len(), 2);
    }

    #[test]
    fn trim_batch_for_size_cap_keeps_single_entry() {
        let mut txs = vec![1, 2, 3];
        let mut routes = vec![10, 11, 12];
        let mut sizes = vec![5, 5, 5];
        let mut removed = Vec::new();

        let removed_count =
            trim_batch_for_size_cap(&mut txs, &mut routes, &mut sizes, &mut removed, 100);

        assert_eq!(removed_count, 2);
        assert_eq!(txs.len(), 1);
        assert_eq!(routes.len(), 1);
        assert_eq!(sizes.len(), 1);
    }

    #[test]
    fn consensus_queue_backpressure_trips_on_payload_or_rbc_queue() {
        let depths = super::status::WorkerQueueDepthSnapshot {
            block_payload_rx: 4,
            ..super::status::WorkerQueueDepthSnapshot::default()
        };
        assert!(consensus_queue_backpressure(depths, 4, 8));

        let depths = super::status::WorkerQueueDepthSnapshot {
            rbc_chunk_rx: 8,
            ..super::status::WorkerQueueDepthSnapshot::default()
        };
        assert!(consensus_queue_backpressure(depths, 4, 8));

        let depths = super::status::WorkerQueueDepthSnapshot {
            block_payload_rx: 3,
            rbc_chunk_rx: 7,
            ..super::status::WorkerQueueDepthSnapshot::default()
        };
        assert!(!consensus_queue_backpressure(depths, 4, 8));
    }

    #[test]
    fn proposal_backpressure_defers_on_consensus_queue_backpressure() {
        let backpressure = ProposalBackpressure {
            queue_state: BackpressureState::Healthy {
                queued: 0,
                capacity: NonZeroUsize::new(1).expect("non-zero"),
            },
            active_pending: false,
            rbc_backlog: false,
            relay_backpressure: false,
            consensus_queue_backpressure: true,
        };
        assert!(backpressure.should_defer());
        assert!(!backpressure.only_queue_saturation());
    }

    #[test]
    fn timeout_streak_advances_for_repeated_height_views() {
        let now = Instant::now();
        let trigger = super::CachedSlotTimeoutTrigger {
            height: 10,
            view: 2,
            at: now,
            streak: 1,
        };

        assert_eq!(
            next_cached_slot_timeout_streak(Some(trigger), 10, 3),
            2,
            "next view at same height should increase streak"
        );
        assert_eq!(
            next_cached_slot_timeout_streak(Some(trigger), 11, 0),
            0,
            "new height should reset streak"
        );
    }

    #[test]
    fn npos_timeout_hysteresis_applies_after_previous_trigger() {
        let now = Instant::now();
        let previous = super::CachedSlotTimeoutTrigger {
            height: 42,
            view: 1,
            at: now,
            streak: 1,
        };
        let quorum_timeout = Duration::from_secs(2);
        let remaining = cached_slot_timeout_hysteresis_remaining(
            super::ConsensusMode::Npos,
            quorum_timeout,
            Some(previous),
            42,
            2,
            now + Duration::from_secs(1),
        );
        assert!(
            remaining.is_some(),
            "NPoS repeated timeout should be delayed by hysteresis window"
        );
        let no_hysteresis = cached_slot_timeout_hysteresis_remaining(
            super::ConsensusMode::Permissioned,
            quorum_timeout,
            Some(previous),
            42,
            2,
            now + Duration::from_secs(1),
        );
        assert!(
            no_hysteresis.is_none(),
            "permissioned mode should not apply NPoS hysteresis"
        );
    }
}
