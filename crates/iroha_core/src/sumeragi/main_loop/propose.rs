//! Proposal assembly and pacemaker-driven propose path.

use super::proposals::block_payload_bytes;
use super::*;
use crate::smartcontracts::isi::triggers::set::SetReadOnly;
use crate::smartcontracts::isi::triggers::specialized::LoadedActionTrait;
use core::num::{NonZeroU64, NonZeroUsize};
use iroha_data_model::block::BlockExecutionContextBundle;
use iroha_data_model::consensus::{
    CommitStakeSnapshot as ModelCommitStakeSnapshot,
    CommitStakeSnapshotEntry as ModelCommitStakeSnapshotEntry, PreviousRosterEvidence,
    ValidatorSetCheckpoint,
};
use iroha_data_model::events::EventFilter;
use iroha_data_model::prelude::Repeats;

const PROPOSAL_STALE_WINDOW_TX_QUANTUM: usize = 128;
const PROPOSAL_STALE_WINDOW_MAX_MULTIPLIER: u32 = 4;

pub(super) const fn should_seed_frontier_backup_transport(
    da_enabled: bool,
    inline_frontier_block_created_transport: bool,
    inline_block_created_backup: bool,
) -> bool {
    da_enabled && inline_frontier_block_created_transport && inline_block_created_backup
}

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
            ValidatorSetCheckpoint::new_with_chain_order(
                qc.height,
                qc.view,
                qc.subject_block_hash,
                qc.chain_order_hash,
                qc.rechain_seq,
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

#[cfg(test)]
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

fn trim_batch_for_size_cap_with_plans<T, U, V>(
    tx_batch: &mut Vec<T>,
    routing_batch: &mut Vec<U>,
    routing_plan_batch: &mut Vec<V>,
    sizes: &mut Vec<usize>,
    removed: &mut Vec<(T, V)>,
    mut excess_bytes: usize,
) -> usize {
    debug_assert_eq!(tx_batch.len(), routing_batch.len());
    debug_assert_eq!(tx_batch.len(), routing_plan_batch.len());
    debug_assert_eq!(tx_batch.len(), sizes.len());
    let mut removed_count = 0usize;
    while excess_bytes > 0 && tx_batch.len() > 1 {
        let tx = match tx_batch.pop() {
            Some(tx) => tx,
            None => break,
        };
        let _routing = match routing_batch.pop() {
            Some(routing) => routing,
            None => break,
        };
        let routing_plan = match routing_plan_batch.pop() {
            Some(routing_plan) => routing_plan,
            None => break,
        };
        let size = sizes.pop().unwrap_or(1).max(1);
        excess_bytes = excess_bytes.saturating_sub(size);
        removed.push((tx, routing_plan));
        removed_count = removed_count.saturating_add(1);
    }
    removed_count
}

#[cfg(test)]
fn canonicalize_parallel_batch_by_key<T, U, K, F>(
    tx_batch: &mut Vec<T>,
    routing_batch: &mut Vec<U>,
    sizes: &mut Vec<usize>,
    key: F,
) where
    K: Ord,
    F: Fn(&T) -> K,
{
    assert_eq!(
        tx_batch.len(),
        routing_batch.len(),
        "routing decisions must align with transactions"
    );
    assert_eq!(
        tx_batch.len(),
        sizes.len(),
        "transaction sizes must align with transactions"
    );
    if tx_batch.len() <= 1 {
        return;
    }

    let mut entries: Vec<_> = std::mem::take(tx_batch)
        .into_iter()
        .zip(std::mem::take(routing_batch))
        .zip(std::mem::take(sizes))
        .enumerate()
        .map(|(idx, ((tx, routing), size))| (key(&tx), idx, tx, routing, size))
        .collect();

    entries.sort_unstable_by(|lhs, rhs| lhs.0.cmp(&rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));

    tx_batch.reserve(entries.len());
    routing_batch.reserve(entries.len());
    sizes.reserve(entries.len());
    for (_, _, tx, routing, size) in entries {
        tx_batch.push(tx);
        routing_batch.push(routing);
        sizes.push(size);
    }
}

#[cfg(test)]
fn canonicalize_proposal_batch(
    tx_batch: &mut Vec<AcceptedTransaction<'static>>,
    routing_batch: &mut Vec<RoutingDecision>,
    sizes: &mut Vec<usize>,
) {
    canonicalize_parallel_batch_by_key(
        tx_batch,
        routing_batch,
        sizes,
        crate::tx::AcceptedTransaction::hash_as_entrypoint,
    );
}

fn canonicalize_proposal_batch_with_plans(
    tx_batch: &mut Vec<AcceptedTransaction<'static>>,
    routing_batch: &mut Vec<RoutingDecision>,
    routing_plan_batch: &mut Vec<crate::queue::RoutingPlan>,
    sizes: &mut Vec<usize>,
) {
    assert_eq!(
        tx_batch.len(),
        routing_batch.len(),
        "routing decisions must align with transactions"
    );
    assert_eq!(
        tx_batch.len(),
        routing_plan_batch.len(),
        "routing plans must align with transactions"
    );
    assert_eq!(
        tx_batch.len(),
        sizes.len(),
        "transaction sizes must align with transactions"
    );
    if tx_batch.len() <= 1 {
        return;
    }

    let mut entries: Vec<_> = std::mem::take(tx_batch)
        .into_iter()
        .zip(std::mem::take(routing_batch))
        .zip(std::mem::take(routing_plan_batch))
        .zip(std::mem::take(sizes))
        .enumerate()
        .map(|(idx, (((tx, routing), routing_plan), size))| {
            (
                tx.hash_as_entrypoint(),
                idx,
                tx,
                routing,
                routing_plan,
                size,
            )
        })
        .collect();

    entries.sort_unstable_by(|lhs, rhs| lhs.0.cmp(&rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));

    tx_batch.reserve(entries.len());
    routing_batch.reserve(entries.len());
    routing_plan_batch.reserve(entries.len());
    sizes.reserve(entries.len());
    for (_, _, tx, routing, routing_plan, size) in entries {
        tx_batch.push(tx);
        routing_batch.push(routing);
        routing_plan_batch.push(routing_plan);
        sizes.push(size);
    }
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

    pub(super) fn only_pacing_backpressure(self) -> bool {
        (self.queue_state.is_saturated() || self.consensus_queue_backpressure)
            && !(self.active_pending || self.rbc_backlog || self.relay_backpressure)
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
) -> usize {
    let rbc_budget = rbc_chunk_max_bytes
        .max(1)
        .saturating_mul(usize::try_from(RBC_MAX_TOTAL_CHUNKS).expect("fits in usize"));
    let pending_budget = rbc_pending_max_bytes.min(
        rbc_chunk_max_bytes
            .max(1)
            .saturating_mul(rbc_pending_max_chunks.max(1)),
    );
    let payload_budget = block_max_payload_bytes.map_or(usize::MAX, NonZeroUsize::get);
    payload_budget.min(rbc_budget).min(pending_budget)
}

impl Actor {
    fn native_amx_attestation_body(
        tx: &AcceptedTransaction<'_>,
        plan_digest: Hash,
        coordinator: RoutingDecision,
        participant: crate::queue::RouteLeg,
        phase: NativeAmxPhase,
        block_height: u64,
    ) -> NativeAmxAttestationBodyV1 {
        let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(tx.hash().as_ref());
        NativeAmxAttestationBodyV1 {
            source_id,
            tx_entrypoint_hash: tx.hash_as_entrypoint(),
            plan_digest,
            phase,
            coordinator_lane_id: coordinator.lane_id,
            coordinator_dataspace_id: coordinator.dataspace_id,
            participant_lane_id: participant.route.lane_id,
            participant_dataspace_id: participant.route.dataspace_id,
            planned_coordinator_block_height: block_height,
        }
    }

    fn native_amx_vote_roster(&self) -> Vec<PeerId> {
        let mut roster = self.effective_commit_topology();
        roster.retain(roster_member_allowed_bls);
        roster
    }

    fn native_amx_receipt_for_plan(
        &mut self,
        tx: &AcceptedTransaction<'_>,
        plan: &crate::queue::RoutingPlan,
        block_height: u64,
    ) -> Result<Option<NativeAmxReceipt>, &'static str> {
        let crate::queue::RoutingPlan::NativeAmx(native_plan) = plan else {
            return Ok(None);
        };
        let validator_set = self.native_amx_vote_roster();
        if validator_set.is_empty() {
            return Err("native AMX participant attestation roster is empty");
        }
        let min_signers =
            crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1);
        let coordinator = native_plan.coordinator.route;
        let key = {
            let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
            source_id.copy_from_slice(tx.hash().as_ref());
            NativeAmxSessionKey {
                source_id,
                plan_digest: native_plan.plan_digest,
            }
        };

        let mut pending = false;
        let mut legs = Vec::with_capacity(native_plan.participants.len());
        for participant in &native_plan.participants {
            let prepare_body = Self::native_amx_attestation_body(
                tx,
                native_plan.plan_digest,
                coordinator,
                *participant,
                NativeAmxPhase::Prepare,
                block_height,
            );
            let commit_body = Self::native_amx_attestation_body(
                tx,
                native_plan.plan_digest,
                coordinator,
                *participant,
                NativeAmxPhase::Commit,
                block_height,
            );

            let prepare_votes = self
                .native_amx_sessions
                .sorted_votes_for_body(key, &prepare_body);
            let commit_votes = self
                .native_amx_sessions
                .sorted_votes_for_body(key, &commit_body);
            if prepare_votes.len() < min_signers {
                pending = true;
                self.schedule_background(BackgroundRequest::BroadcastNativeAmx {
                    message: NativeAmxMessage::PrepareRequest(prepare_body),
                });
                continue;
            }
            if commit_votes.len() < min_signers {
                pending = true;
                self.schedule_background(BackgroundRequest::BroadcastNativeAmx {
                    message: NativeAmxMessage::CommitRequest(commit_body),
                });
                continue;
            }

            let prepare_qc = aggregate_votes_to_qc(
                prepare_body,
                validator_set.clone(),
                &prepare_votes,
                min_signers,
            )
            .map_err(|_| "native AMX prepare QC could not be assembled")?;
            let commit_qc = aggregate_votes_to_qc(
                commit_body,
                validator_set.clone(),
                &commit_votes,
                min_signers,
            )
            .map_err(|_| "native AMX commit QC could not be assembled")?;
            legs.push(NativeAmxLegRecord {
                lane_id: participant.route.lane_id,
                dataspace_id: participant.route.dataspace_id,
                prepare_qc,
                commit_qc,
            });
        }

        if pending {
            return Err("native AMX participant attestations are still pending");
        }

        let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(tx.hash().as_ref());
        Ok(Some(NativeAmxReceipt {
            version: 1,
            source_id,
            plan_digest: native_plan.plan_digest,
            lane_id: coordinator.lane_id,
            dataspace_id: coordinator.dataspace_id,
            block_height,
            legs,
        }))
    }

    fn native_amx_receipts_for_batch(
        &mut self,
        tx_batch: &[AcceptedTransaction<'static>],
        routing_plan_batch: &[crate::queue::RoutingPlan],
        block_height: u64,
    ) -> Result<Vec<Option<NativeAmxReceipt>>, &'static str> {
        let mut receipts = Vec::with_capacity(tx_batch.len());
        for (tx, plan) in tx_batch.iter().zip(routing_plan_batch) {
            receipts.push(self.native_amx_receipt_for_plan(tx, plan, block_height)?);
        }
        Ok(receipts)
    }

    pub(super) fn frontier_missing_qc_liveness_active(&self, height: u64, view: u64) -> bool {
        self.subsystems
            .propose
            .proposal_liveness
            .is_some_and(|slot| {
                slot.height == height
                    && slot.view == view
                    && matches!(
                        slot.state,
                        ProposalLivenessState::AwaitingProposalAfterMissingQc
                            | ProposalLivenessState::RecoveryAcquireDependencies
                    )
            })
    }

    fn missing_qc_liveness_allows_frontier_self_proposal(
        &self,
        height: u64,
        view: u64,
        committed_height: u64,
        _pending_queue_len: usize,
        precommit_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    ) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
        if height != committed_height.saturating_add(1) || view == 0 {
            return None;
        }
        if !self.frontier_missing_qc_liveness_active(height, view) {
            return None;
        }
        let qc = precommit_qc?;
        if height == qc.height.saturating_add(1) {
            Some(qc)
        } else {
            None
        }
    }

    fn vote_locked_frontier_recovery_ready(&self, height: u64, view: u64, now: Instant) -> bool {
        let Some(lock) = self.same_height_vote_lock_blocking_candidate(height, view, None) else {
            return false;
        };
        if self.same_height_block_has_observed_qc(lock.block_hash, height, lock.view) {
            return false;
        }
        let min_stale_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1));
        let hard_stale_age = min_stale_age.saturating_mul(3);
        self.stale_same_height_recovery_age(height, lock.view, now)
            .is_some_and(|age| age >= hard_stale_age)
            || self.same_height_vote_recovery_escalation_view_gap_exhausted(
                lock.view,
                view,
                lock.total_validators,
            )
    }

    pub(super) fn frontier_recovery_ingress_override_active(
        &self,
        height: u64,
        view: u64,
        now: Instant,
        ingress_grace: Duration,
    ) -> bool {
        if !self.config.resilience.enabled
            || height != self.committed_height_snapshot().saturating_add(1)
            || view == 0
            || !self.frontier_proposal_or_view_starved_past_ingress_grace(
                height,
                now,
                ingress_grace,
            )
        {
            return false;
        }
        self.vote_locked_frontier_recovery_ready(height, view, now)
            || self.frontier_missing_qc_liveness_active(height, view)
            || self.exact_frontier_body_repair_active_at_height(height)
    }

    fn warn_resilience_frontier_proposal_deferred(
        &mut self,
        height: u64,
        view: u64,
        reason: &'static str,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        pending_queue_len: usize,
        now: Instant,
    ) {
        if !self.config.resilience.enabled
            || height != self.committed_height_snapshot().saturating_add(1)
            || view == 0
            || highest_qc.height.saturating_add(1) != height
        {
            return;
        }
        if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
            ProposalDeferWarningKind::FrontierRecoveryProposalDeferred,
            height,
            view,
            highest_qc.subject_block_hash,
            now,
            Duration::from_secs(2),
        ) {
            warn!(
                height,
                view,
                reason,
                queue_len = pending_queue_len,
                highest_height = highest_qc.height,
                highest_view = highest_qc.view,
                highest_hash = %highest_qc.subject_block_hash,
                suppressed_since_last,
                "resilience frontier recovery proposal deferred"
            );
        }
    }

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

    pub(super) fn max_tx_budget_for_commit_time(
        queue_len: usize,
        block_param_limit: u64,
        config_cap: Option<NonZeroUsize>,
        fast_finality_config_cap: Option<NonZeroUsize>,
        commit_time_ms: u64,
        effective_commit_time_ms: u64,
    ) -> (usize, NonZeroUsize, bool) {
        let (configured_target, _) = Self::max_tx_budget(queue_len, block_param_limit, config_cap);
        let fast_cap = fast_finality_config_cap
            .filter(|_| Self::fast_finality_cap_applies(commit_time_ms, effective_commit_time_ms))
            .map(NonZeroUsize::get);
        let max_tx_target = fast_cap.map_or(configured_target, |cap| configured_target.min(cap));
        let max_in_block = NonZeroUsize::new(queue_len.min(max_tx_target).max(1))
            .expect("non-zero by construction");
        let fast_tx_capped = max_tx_target < configured_target;
        (max_tx_target, max_in_block, fast_tx_capped)
    }

    pub(super) fn fast_finality_cap_applies(
        commit_time_ms: u64,
        effective_commit_time_ms: u64,
    ) -> bool {
        let threshold = iroha_config::parameters::defaults::sumeragi::FAST_FINALITY_COMMIT_TIME_MS;
        commit_time_ms <= threshold || effective_commit_time_ms <= threshold
    }

    pub(super) fn cap_gas_limit_for_fast_commit(
        gas_limit_per_block: Option<NonZeroU64>,
        commit_time_ms: u64,
        effective_commit_time_ms: u64,
        fast_gas_limit_per_block: Option<NonZeroU64>,
    ) -> Option<NonZeroU64> {
        let Some(base_limit) = gas_limit_per_block else {
            return None;
        };
        let Some(cap) = fast_gas_limit_per_block else {
            return Some(base_limit);
        };
        if !Self::fast_finality_cap_applies(commit_time_ms, effective_commit_time_ms) {
            return Some(base_limit);
        }
        let capped = base_limit.get().min(cap.get());
        Some(NonZeroU64::new(capped).expect("non-zero by construction"))
    }

    pub(super) fn proposal_assembly_stale_window(base: Duration, tx_count: usize) -> Duration {
        let batches = tx_count.saturating_add(PROPOSAL_STALE_WINDOW_TX_QUANTUM - 1)
            / PROPOSAL_STALE_WINDOW_TX_QUANTUM;
        let full_batch_grace = usize::from(tx_count >= PROPOSAL_STALE_WINDOW_TX_QUANTUM);
        let multiplier = batches
            .saturating_add(full_batch_grace)
            .max(1)
            .min(PROPOSAL_STALE_WINDOW_MAX_MULTIPLIER as usize);
        let multiplier = u32::try_from(multiplier).expect("proposal stale multiplier fits u32");
        base.saturating_mul(multiplier)
    }

    pub(super) fn is_ivm_heavy_transaction(
        tx: &AcceptedTransaction<'_>,
        replay_ivm_proved: bool,
    ) -> bool {
        fn is_heavy_executable(
            executable: &iroha_data_model::transaction::Executable,
            replay_ivm_proved: bool,
        ) -> bool {
            matches!(
                executable,
                iroha_data_model::transaction::Executable::ContractCall(_)
                    | iroha_data_model::transaction::Executable::Ivm(_)
            ) || (replay_ivm_proved
                && matches!(
                    executable,
                    iroha_data_model::transaction::Executable::IvmProved(_)
                ))
        }

        match tx.entrypoint() {
            iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
                is_heavy_executable(signed.instructions(), replay_ivm_proved)
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
                is_heavy_executable(
                    reveal.signed_transaction().instructions(),
                    replay_ivm_proved,
                )
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_)
            | iroha_data_model::transaction::TransactionEntrypoint::PrivateKaigi(_)
            | iroha_data_model::transaction::TransactionEntrypoint::Time(_) => false,
        }
    }

    pub(super) fn pull_transactions_for_proposal(
        &self,
        state: &State,
        max_in_block: NonZeroUsize,
        scan_budget: usize,
        gas_limit_per_block: Option<NonZeroU64>,
        max_ivm_transactions: Option<NonZeroUsize>,
        replay_ivm_proved: bool,
        tx_guards: &mut Vec<crate::queue::TransactionGuard>,
        height: u64,
        view: u64,
    ) -> Vec<(AcceptedTransaction<'static>, crate::queue::RoutingPlan)> {
        let mut lane_consumption: BTreeMap<LaneId, u64> = BTreeMap::new();
        let mut deferred_accumulator: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut fetched_total = 0usize;
        let mut gas_used_in_block = 0u64;
        let gas_limit_per_block = gas_limit_per_block.map(NonZeroU64::get);
        let max_ivm_transactions = max_ivm_transactions.map(NonZeroUsize::get);
        let mut ivm_transactions_included = 0usize;
        let mut ivm_transactions_deferred = 0usize;
        let scan_budget = scan_budget.max(1);
        let committed_nexus = state.nexus_snapshot();
        let multilane_lookahead =
            committed_nexus.enabled && committed_nexus.uses_multilane_catalogs();
        if self.queue.reconfigure_nexus_with_state_if_needed(
            &committed_nexus,
            state,
            self.queue.lane_compliance_engine(),
        ) {
            info!(
                height,
                view, "proposal queue routing refreshed from committed Nexus state"
            );
        }

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
            let fetch_cap = if multilane_lookahead {
                remaining_budget
            } else {
                remaining_budget.min(remaining_slots)
            };
            let fetch_cap = NonZeroUsize::new(fetch_cap).expect("non-zero by construction");
            let mut fetched = Vec::new();
            self.queue
                .get_transactions_for_block_with_state(state, fetch_cap, &mut fetched);
            if fetched.is_empty() {
                break;
            }
            fetched_total = fetched_total.saturating_add(fetched.len());
            let deferred = self
                .queue
                .enforce_lane_teu_limits_with_consumption_and_routing_plans(
                    &mut fetched,
                    &mut lane_consumption,
                );
            if !deferred.is_empty() {
                deferred_accumulator.extend(deferred);
            }

            let fetched_routing: Vec<RoutingDecision> = fetched
                .iter()
                .map(crate::queue::TransactionGuard::routing)
                .collect();
            let order = interleave_lane_indices_for_slot(&fetched_routing, height, view);
            let mut fetched_slots: Vec<Option<crate::queue::TransactionGuard>> =
                fetched.into_iter().map(Some).collect();
            let mut accepted = Vec::with_capacity(remaining_slots.min(fetched_slots.len()));
            let release_lane_consumption =
                |guard: &crate::queue::TransactionGuard,
                 lane_consumption: &mut BTreeMap<LaneId, u64>| {
                    let lane_id = guard.routing().lane_id;
                    let teu = guard.teu_weight();
                    if let Some(used) = lane_consumption.get_mut(&lane_id) {
                        *used = used.saturating_sub(teu);
                    }
                };

            for idx in order {
                let Some(guard) = fetched_slots.get_mut(idx).and_then(Option::take) else {
                    continue;
                };

                if tx_guards.len().saturating_add(accepted.len()) >= max_in_block.get() {
                    release_lane_consumption(&guard, &mut lane_consumption);
                    deferred_accumulator.push((guard.clone_accepted(), guard.routing_plan()));
                    continue;
                }

                let is_ivm_heavy =
                    Self::is_ivm_heavy_transaction(guard.as_accepted(), replay_ivm_proved);
                if let Some(limit) = max_ivm_transactions
                    && is_ivm_heavy
                    && ivm_transactions_included >= limit
                {
                    release_lane_consumption(&guard, &mut lane_consumption);
                    ivm_transactions_deferred = ivm_transactions_deferred.saturating_add(1);
                    deferred_accumulator.push((guard.clone_accepted(), guard.routing_plan()));
                    continue;
                }

                if let Some(limit) = gas_limit_per_block {
                    let gas_cost = guard.gas_cost();
                    let remaining_gas = limit.saturating_sub(gas_used_in_block);
                    let would_exceed = gas_cost > remaining_gas && gas_cost > 0;
                    let allow_oversized =
                        gas_used_in_block == 0 && tx_guards.is_empty() && accepted.is_empty();

                    if would_exceed && !allow_oversized {
                        release_lane_consumption(&guard, &mut lane_consumption);
                        deferred_accumulator.push((guard.clone_accepted(), guard.routing_plan()));
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
                }

                if is_ivm_heavy {
                    ivm_transactions_included = ivm_transactions_included.saturating_add(1);
                }
                accepted.push(guard);
            }
            tx_guards.extend(accepted);

            if let Some(limit) = gas_limit_per_block {
                if gas_used_in_block >= limit {
                    break;
                }
            }
        }

        if ivm_transactions_deferred > 0 {
            debug!(
                height,
                view,
                max_ivm_transactions,
                ivm_transactions_included,
                ivm_transactions_deferred,
                "proposal IVM-heavy transaction budget reached"
            );
        }

        deferred_accumulator
    }

    fn requeue_accepted_transaction(
        &self,
        tx: AcceptedTransaction<'static>,
        routing_plan: crate::queue::RoutingPlan,
        warn_context: &'static str,
    ) {
        let tx_hash = tx.as_ref().hash();
        if let Err(err) =
            self.queue
                .push_requeued_with_routing_plan(tx, routing_plan, self.state.as_ref())
        {
            match err.err {
                crate::queue::Error::IsInQueue => {
                    trace!(
                        tx = %tx_hash,
                        "transaction already in queue during proposal requeue"
                    );
                }
                crate::queue::Error::InBlockchain => {
                    trace!(
                        tx = %tx_hash,
                        "transaction already committed during proposal requeue"
                    );
                }
                err => {
                    warn!(?err, "{warn_context}");
                }
            }
        }
    }

    pub(super) fn drop_stale_pending_block(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<(usize, usize, usize, usize)> {
        self.drop_stale_pending_block_skipping_known_committed(
            pending_hash,
            height,
            view,
            true,
            None,
        )
    }

    pub(super) fn drop_stale_pending_block_skipping_committed_txs(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        known_committed_hashes: Option<&BTreeSet<HashOf<SignedTransaction>>>,
    ) -> Option<(usize, usize, usize, usize)> {
        self.drop_stale_pending_block_skipping_known_committed(
            pending_hash,
            height,
            view,
            true,
            known_committed_hashes,
        )
    }

    fn drop_stale_pending_block_for_fresh_proposal(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<(usize, usize, usize, usize)> {
        self.drop_stale_pending_block_skipping_known_committed(
            pending_hash,
            height,
            view,
            false,
            None,
        )
    }

    fn drop_stale_pending_block_skipping_known_committed(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        retain_for_body_repair: bool,
        known_committed_hashes: Option<&BTreeSet<HashOf<SignedTransaction>>>,
    ) -> Option<(usize, usize, usize, usize)> {
        if retain_for_body_repair
            && self.should_retain_stale_pending_payload_for_body_repair(pending_hash, height, view)
        {
            let mut pending = self
                .pending
                .pending_blocks
                .remove(&pending_hash)
                .expect("checked pending block exists");
            if !pending.is_retired_same_height() {
                pending.retire_same_height();
            }
            let frontier_info =
                self.authoritative_slot_frontier_info(pending.height, pending.view, pending_hash);
            self.slot_tracker.note_retained_branch(
                pending.height,
                pending.view,
                pending_hash,
                frontier_info,
                true,
                Instant::now(),
            );
            self.pending.pending_fetch_requests.remove(&pending_hash);
            self.pending
                .pending_block_body_requests
                .remove(&pending_hash);
            self.clear_validation_ownership_for_block(pending_hash);
            self.subsystems
                .propose
                .proposal_cache
                .pop_hint(height, view);
            self.subsystems
                .propose
                .proposal_cache
                .pop_proposal(height, view);
            self.pending.pending_blocks.insert(pending_hash, pending);
            debug!(
                height,
                view,
                block = %pending_hash,
                "retired stale vote-backed pending payload for exact body repair"
            );
            return Some((0, 0, 0, 0));
        }

        if self.active_commit_inflight_blocks_stale_owner_clear(pending_hash, height, view, true) {
            return None;
        }

        let (tx_count, requeued, failures, duplicate_failures) =
            super::drop_pending_block_and_requeue_skipping_known_committed(
                &mut self.pending.pending_blocks,
                pending_hash,
                self.queue.as_ref(),
                self.state.as_ref(),
                known_committed_hashes,
            )?;

        self.clear_validation_ownership_for_block(pending_hash);
        self.clean_rbc_sessions_for_block(pending_hash, height);
        self.qc_cache
            .retain(|(_, hash, _, _, _, _, _), _| hash != &pending_hash);
        self.qc_signer_tally
            .retain(|(_, hash, _, _, _, _, _), _| hash != &pending_hash);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        // Keep proposals_seen so we don't re-propose in the same view after dropping a stale block.
        let _ =
            self.active_commit_inflight_blocks_stale_owner_clear(pending_hash, height, view, false);

        Some((tx_count, requeued, failures, duplicate_failures))
    }

    fn should_retain_stale_pending_payload_for_body_repair(
        &self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        if !self.runtime_da_enabled() || self.kura.get_block_height_by_hash(pending_hash).is_some()
        {
            return false;
        }
        let Some(pending) = self.pending.pending_blocks.get(&pending_hash) else {
            return false;
        };
        if pending.height != height
            || pending.view != view
            || pending.is_retired_same_height()
            || matches!(pending.validation_status, ValidationStatus::Invalid)
        {
            return false;
        }
        let committed_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        if !super::pending_extends_tip(
            pending.height,
            pending.block.header().prev_block_hash(),
            committed_height,
            tip_hash,
        ) {
            return false;
        }
        pending.local_commit_vote_emitted()
            || pending.commit_qc_observed()
            || self.pending_block_has_commit_votes(pending_hash, pending.height, pending.view)
            || self.pending_block_has_qc(pending_hash, pending.height, pending.view)
    }

    fn active_commit_inflight_blocks_stale_owner_clear(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        requeue_transactions: bool,
    ) -> bool {
        let Some(inflight) = self.subsystems.commit.inflight.as_ref().filter(|inflight| {
            inflight.block_hash == block_hash
                && inflight.pending.height == height
                && inflight.pending.view == view
        }) else {
            return false;
        };
        let tx_count = inflight.pending.block.external_entrypoints_cloned().count();
        warn!(
            height,
            view,
            block = %block_hash,
            commit_id = inflight.id,
            requeue_transactions,
            tx_count,
            elapsed_ms = Instant::now()
                .saturating_duration_since(inflight.enqueue_time)
                .as_millis(),
            "stale commit-inflight owner is still running; waiting for commit worker result"
        );
        true
    }

    fn request_frontier_owner_body_repair(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        now: Instant,
    ) -> bool {
        if self.frontier_block_materialized_locally(block_hash) {
            return false;
        }

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let mut roster = self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if roster.is_empty() {
            roster = self.rbc_roster_for_session((block_hash, height, view));
        }
        if roster.is_empty() {
            roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
        }
        if roster.is_empty() {
            roster = self.effective_commit_topology();
        }
        if roster.is_empty() {
            return false;
        }

        let topology = super::network_topology::Topology::new(roster);
        let seeded = self.handle_frontier_body_gap_with_topology(
            block_hash,
            height,
            view,
            &BTreeSet::new(),
            &topology,
            true,
            now,
        );
        let fetch_requested = self.emit_frontier_block_body_fetch_urgent(now);
        if seeded || fetch_requested {
            debug!(
                height,
                view,
                block = %block_hash,
                seeded_frontier_body_repair = seeded,
                fetch_requested,
                "requested exact frontier body repair for protected owner"
            );
        }
        seeded || fetch_requested
    }

    fn escalate_stale_vote_locked_frontier_owner_recovery(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        now: Instant,
        trigger: &'static str,
    ) -> bool {
        let mut progress = self.request_frontier_owner_body_repair(block_hash, height, view, now);
        if self.frontier_block_materialized_locally(block_hash) {
            let targets =
                self.known_block_commit_qc_recovery_targets(block_hash, height, view, &[]);
            progress |= self.maybe_request_known_block_commit_qc_recovery(
                block_hash, height, view, &targets, None, trigger,
            );
        }
        if self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.height == height && slot.view == view && slot.block_hash == block_hash
        }) {
            progress |= matches!(
                self.handle_frontier_slot_event(
                    now,
                    FrontierSlotEvent::OnLagWindowExpired {
                        reason: "frontier_stall_reset",
                    },
                ),
                FrontierRecoveryAdvance::CatchUp | FrontierRecoveryAdvance::Rotate
            );
        }
        if !progress {
            progress |=
                self.request_range_pull_from_anchor(height, "frontier_stall_reset_fallback", now);
        }
        progress
    }

    pub(super) fn maybe_yield_stale_frontier_owner_for_fresh_proposal(
        &mut self,
        height: u64,
        view: u64,
        owner_hash: HashOf<BlockHeader>,
        owner_view: u64,
        now: Instant,
        pending_queue_len: usize,
    ) -> bool {
        let missing_qc_liveness_active = self.frontier_missing_qc_liveness_active(height, view);
        let committed_height = self.committed_height_snapshot();
        let same_height_recovery_view =
            height == committed_height.saturating_add(1) && owner_view < view && view > 0;
        if !self.config.resilience.enabled
            || (pending_queue_len == 0 && !missing_qc_liveness_active && !same_height_recovery_view)
            || height != committed_height.saturating_add(1)
            || owner_view >= view
        {
            return false;
        }
        let min_yield_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1));
        let hard_yield_age = min_yield_age.saturating_mul(3);
        let owner_qc_observed =
            self.same_height_block_has_recoverable_qc(owner_hash, height, owner_view);
        let owner_slot_evidence = self
            .frontier_slot
            .as_ref()
            .filter(|slot| {
                slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash
            })
            .map(|slot| {
                (
                    now.saturating_duration_since(slot.timers.last_progress_at)
                        .max(now.saturating_duration_since(slot.timers.observed_at)),
                    slot.quorum_progress.commit_qc_observed,
                    self.frontier_slot_competing_quorum_locked_for_view(slot, view),
                )
            });
        let local_vote_consensus_locked = self
            .local_same_height_vote(height, self.epoch_for_height(height))
            .as_ref()
            .is_some_and(|vote| self.local_same_height_vote_has_consensus_lock(height, vote));
        let local_commit_vote_blocks_fresh_branch = self
            .local_same_height_vote(height, self.epoch_for_height(height))
            .as_ref()
            .is_some_and(|vote| {
                matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit)
                    && !self.local_same_height_vote_is_committed_parent_marker(height, view, vote)
            });
        let commit_inflight_live =
            self.subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == owner_hash
                        && !inflight.pending.aborted
                        && inflight.pending.validation_status != ValidationStatus::Invalid
                        && inflight.pending.height == height
                        && inflight.pending.view == owner_view
                });
        let owner_pending_commit_qc_observed = self
            .pending
            .pending_blocks
            .get(&owner_hash)
            .is_some_and(|pending| {
                pending.height == height
                    && pending.view == owner_view
                    && !pending.aborted
                    && pending.validation_status != ValidationStatus::Invalid
                    && pending.commit_qc_observed()
            });
        let new_view_qc_supersedes_owner = self.latest_committed_qc().is_some_and(|highest_qc| {
            self.new_view_qc_supersedes_same_height_vote_conflict(
                height, view, highest_qc, owner_hash, owner_view,
            )
        });
        let Some(owner_pending) = self
            .pending
            .pending_blocks
            .get(&owner_hash)
            .filter(|pending| {
                pending.height == height
                    && pending.view == owner_view
                    && !pending.commit_qc_observed()
            })
        else {
            let mut body_repair_requested = false;
            let mut stale_vote_locked_recovery_requested = false;
            if let Some((owner_age, frontier_commit_qc_observed, competing_quorum_locked)) =
                owner_slot_evidence
            {
                let slot_commit_qc_repairable =
                    frontier_commit_qc_observed && owner_age < hard_yield_age;
                let protected_owner = owner_qc_observed
                    || slot_commit_qc_repairable
                    || local_vote_consensus_locked
                    || (local_commit_vote_blocks_fresh_branch && !new_view_qc_supersedes_owner)
                    || (competing_quorum_locked && !new_view_qc_supersedes_owner)
                    || commit_inflight_live;
                body_repair_requested = protected_owner
                    && self.request_frontier_owner_body_repair(owner_hash, height, owner_view, now);
                let stale_vote_locked_owner = protected_owner
                    && owner_age >= hard_yield_age
                    && (local_vote_consensus_locked
                        || (local_commit_vote_blocks_fresh_branch
                            && !new_view_qc_supersedes_owner)
                        || (competing_quorum_locked && !new_view_qc_supersedes_owner))
                    && !owner_qc_observed
                    && !owner_pending_commit_qc_observed
                    && !commit_inflight_live;
                if stale_vote_locked_owner {
                    stale_vote_locked_recovery_requested = self
                        .escalate_stale_vote_locked_frontier_owner_recovery(
                            owner_hash,
                            height,
                            owner_view,
                            now,
                            "stale_vote_locked_frontier_owner",
                        );
                    if stale_vote_locked_recovery_requested {
                        info!(
                            height,
                            view,
                            owner_view,
                            owner = %owner_hash,
                            owner_age_ms = owner_age.as_millis(),
                            hard_yield_age_ms = hard_yield_age.as_millis(),
                            local_vote_consensus_locked,
                            competing_quorum_locked,
                            new_view_qc_supersedes_owner,
                            "escalated stale vote-locked frontier owner to committed-anchor catch-up"
                        );
                    }
                }
                let stale_unprotected_owner = owner_age >= min_yield_age && !protected_owner;
                let recovery_exhausted = owner_age >= hard_yield_age;
                if (stale_unprotected_owner || recovery_exhausted)
                    && !owner_qc_observed
                    && !owner_pending_commit_qc_observed
                    && !local_vote_consensus_locked
                    && !(local_commit_vote_blocks_fresh_branch && !new_view_qc_supersedes_owner)
                    && (!competing_quorum_locked
                        || new_view_qc_supersedes_owner
                        || recovery_exhausted)
                    && !commit_inflight_live
                {
                    self.frontier_slot = None;
                    info!(
                        height,
                        view,
                        owner_view,
                        owner = %owner_hash,
                        owner_age_ms = owner_age.as_millis(),
                        hard_yield_age_ms = hard_yield_age.as_millis(),
                        queue_len = pending_queue_len,
                        frontier_commit_qc_observed,
                        owner_pending_commit_qc_observed,
                        competing_quorum_locked,
                        new_view_qc_supersedes_owner,
                        "cleared no-pending stale frontier owner for fresh resilience proposal"
                    );
                    return true;
                }
            }
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::FrontierOwnerYieldBlocked,
                height,
                view,
                owner_hash,
                now,
                Duration::from_secs(3),
            ) {
                let pending_snapshot = self.pending.pending_blocks.get(&owner_hash);
                warn!(
                    height,
                    view,
                    owner_view,
                    owner = %owner_hash,
                    pending_present = pending_snapshot.is_some(),
                    pending_aborted = pending_snapshot.is_some_and(|pending| pending.aborted),
                    pending_retired = pending_snapshot
                        .is_some_and(PendingBlock::is_retired_same_height),
                    pending_validation = ?pending_snapshot.map(|pending| pending.validation_status),
                    pending_commit_qc_observed = pending_snapshot
                        .is_some_and(PendingBlock::commit_qc_observed),
                    owner_slot_present = owner_slot_evidence.is_some(),
                    owner_slot_age_ms = ?owner_slot_evidence.map(|(age, _, _)| age.as_millis()),
                    hard_yield_age_ms = hard_yield_age.as_millis(),
                    owner_qc_observed,
                    owner_pending_commit_qc_observed,
                    local_vote_consensus_locked,
                    local_commit_vote_blocks_fresh_branch,
                    commit_inflight_live,
                    body_repair_requested,
                    stale_vote_locked_recovery_requested,
                    new_view_qc_supersedes_owner,
                    frontier_commit_qc_observed = owner_slot_evidence
                        .is_some_and(|(_, observed, _)| observed),
                    competing_quorum_locked = owner_slot_evidence
                        .is_some_and(|(_, _, locked)| locked),
                    suppressed_since_last,
                    "stale frontier owner yield blocked: owner pending is not yieldable"
                );
            }
            return false;
        };
        let owner_age = owner_pending
            .progress_age(now)
            .max(now.saturating_duration_since(owner_pending.inserted_at));
        let recovery_age = self.stale_same_height_recovery_age(height, owner_view, now);
        let recovery_exhausted =
            owner_age >= hard_yield_age || recovery_age.is_some_and(|age| age >= hard_yield_age);
        let local_vote = self.local_same_height_vote(height, self.epoch_for_height(height));
        let local_vote_new_view_qc_supersedes = local_vote.as_ref().is_some_and(|vote| {
            self.latest_committed_qc().is_some_and(|highest_qc| {
                self.new_view_qc_supersedes_same_height_vote_conflict(
                    height,
                    view,
                    highest_qc,
                    vote.block_hash,
                    vote.view,
                )
            })
        });
        let local_vote_blocks = local_vote.as_ref().is_some_and(|vote| {
            !local_vote_new_view_qc_supersedes
                && self.local_same_height_vote_blocks_fresh_proposal(height, view, vote, now, false)
        });
        let local_commit_vote_blocks_yield = local_vote.as_ref().is_some_and(|vote| {
            !local_vote_new_view_qc_supersedes
                && matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit)
                && !self.local_same_height_vote_is_committed_parent_marker(height, view, vote)
        });
        let (frontier_commit_qc_observed, competing_quorum_locked) = self
            .frontier_slot
            .as_ref()
            .filter(|slot| {
                slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash
            })
            .map_or((false, false), |slot| {
                (
                    slot.quorum_progress.commit_qc_observed,
                    self.frontier_slot_competing_quorum_locked_for_view(slot, view),
                )
            });
        let frontier_commit_qc_blocks_yield = frontier_commit_qc_observed && !recovery_exhausted;
        let competing_quorum_blocks_yield =
            competing_quorum_locked && !new_view_qc_supersedes_owner;
        if owner_qc_observed
            || frontier_commit_qc_blocks_yield
            || local_vote_consensus_locked
            || competing_quorum_blocks_yield
            || local_commit_vote_blocks_yield
            || (local_vote_blocks && !recovery_exhausted)
        {
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::FrontierOwnerYieldBlocked,
                height,
                view,
                owner_hash,
                now,
                Duration::from_secs(3),
            ) {
                warn!(
                    height,
                    view,
                    owner_view,
                    owner = %owner_hash,
                    owner_age_ms = owner_age.as_millis(),
                    recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                    min_yield_age_ms = min_yield_age.as_millis(),
                    hard_yield_age_ms = hard_yield_age.as_millis(),
                    recovery_exhausted,
                    owner_qc_observed,
                    frontier_commit_qc_observed,
                    frontier_commit_qc_blocks_yield,
                    local_vote_consensus_locked,
                    local_commit_vote_blocks_yield,
                    local_vote_blocks,
                    competing_quorum_locked,
                    competing_quorum_blocks_yield,
                    new_view_qc_supersedes_owner,
                    local_vote_new_view_qc_supersedes,
                    suppressed_since_last,
                    "stale frontier owner yield blocked by consensus evidence"
                );
            }
            return false;
        }
        if owner_age < min_yield_age && !recovery_exhausted {
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::FrontierOwnerYieldBlocked,
                height,
                view,
                owner_hash,
                now,
                Duration::from_secs(3),
            ) {
                warn!(
                    height,
                    view,
                    owner_view,
                    owner = %owner_hash,
                    owner_age_ms = owner_age.as_millis(),
                    recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                    min_yield_age_ms = min_yield_age.as_millis(),
                    suppressed_since_last,
                    "stale frontier owner yield blocked: owner still inside yield grace"
                );
            }
            return false;
        }

        if self
            .active_commit_inflight_blocks_stale_owner_clear(owner_hash, height, owner_view, true)
        {
            return false;
        }

        let dropped =
            self.drop_stale_pending_block_for_fresh_proposal(owner_hash, height, owner_view);
        if self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash
        }) {
            self.frontier_slot = None;
        }
        if let Some((tx_count, requeued, failures, duplicate_failures)) = dropped {
            info!(
                height,
                view,
                owner_view,
                owner = %owner_hash,
                owner_age_ms = owner_age.as_millis(),
                recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                min_yield_age_ms = min_yield_age.as_millis(),
                tx_count,
                requeued,
                failures,
                duplicate_failures,
                queue_len = pending_queue_len,
                "yielded stale frontier owner for fresh resilience proposal"
            );
        } else {
            info!(
                height,
                view,
                owner_view,
                owner = %owner_hash,
                owner_age_ms = owner_age.as_millis(),
                recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                min_yield_age_ms = min_yield_age.as_millis(),
                cleared_inflight = false,
                queue_len = pending_queue_len,
                "cleared stale frontier owner for fresh resilience proposal"
            );
        }
        true
    }

    fn local_same_height_vote_has_consensus_lock(
        &self,
        proposal_height: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        self.locked_qc
            .is_some_and(|locked| locked.height >= proposal_height)
            || self.same_height_block_has_recoverable_qc(
                existing_vote.block_hash,
                proposal_height,
                existing_vote.view,
            )
    }

    pub(super) fn same_height_block_has_observed_qc(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        self.same_height_block_has_recoverable_qc(block_hash, height, view)
            || self.frontier_slot.as_ref().is_some_and(|slot| {
                slot.height == height
                    && slot.view == view
                    && slot.block_hash == block_hash
                    && slot.quorum_progress.commit_qc_observed
            })
    }

    fn local_same_height_vote_has_hard_lock(
        &self,
        proposal_height: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        self.local_same_height_vote_has_consensus_lock(proposal_height, existing_vote)
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == existing_vote.block_hash
                        && !inflight.pending.aborted
                        && inflight.pending.validation_status != ValidationStatus::Invalid
                })
    }

    pub(super) fn stale_same_height_recovery_age(
        &self,
        proposal_height: u64,
        subject_view: u64,
        now: Instant,
    ) -> Option<Duration> {
        self.frontier_recovery
            .as_ref()
            .filter(|state| {
                state.frontier_height == proposal_height
                    && matches!(state.last_cause, "missing_qc" | "quorum_timeout")
                    && state
                        .last_rotation_view
                        .is_some_and(|view| view >= subject_view)
            })
            .map(|state| now.saturating_duration_since(state.entered_at))
    }

    pub(super) fn same_height_vote_recovery_view_gap_exhausted(
        &self,
        subject_view: u64,
        proposal_view: u64,
        total_validators: usize,
    ) -> bool {
        let view_gap = proposal_view.saturating_sub(subject_view);
        let min_view_gap = u64::try_from(total_validators.saturating_mul(8))
            .unwrap_or(u64::MAX)
            .max(8);
        view_gap >= min_view_gap
    }

    pub(super) fn same_height_vote_recovery_escalation_view_gap_exhausted(
        &self,
        subject_view: u64,
        proposal_view: u64,
        total_validators: usize,
    ) -> bool {
        let view_gap = proposal_view.saturating_sub(subject_view);
        // Escalating exact recovery only re-requests the vote-backed branch; it does not let
        // a fresh conflicting branch bypass the longer rotation/supersession gate.
        let min_view_gap = u64::try_from(total_validators.saturating_mul(2))
            .unwrap_or(u64::MAX)
            .max(8);
        view_gap >= min_view_gap
    }

    fn local_same_height_vote_has_live_proposal_material(
        &self,
        proposal_height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> bool {
        self.pending
            .pending_blocks
            .get(&block_hash)
            .is_some_and(|pending| pending.height == proposal_height && !pending.is_retry_aborted())
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == block_hash
                        && inflight.pending.height == proposal_height
                        && !inflight.pending.aborted
                })
            || self
                .pending
                .pending_processing
                .get()
                .is_some_and(|pending| pending == block_hash)
            || self
                .kura
                .get_block_height_by_hash(block_hash)
                .is_some_and(|height| {
                    u64::try_from(height.get()).is_ok_and(|height| height == proposal_height)
                })
    }

    pub(super) fn local_same_height_vote_blocks_fresh_proposal(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
        now: Instant,
        block_active_tip_owner: bool,
    ) -> bool {
        if existing_vote.view > proposal_view {
            return true;
        }
        if matches!(
            existing_vote.phase,
            crate::sumeragi::consensus::Phase::Commit
        ) && !self.local_same_height_vote_is_committed_parent_marker(
            proposal_height,
            proposal_view,
            existing_vote,
        ) {
            return true;
        }
        if existing_vote.view == proposal_view {
            return self.local_same_height_vote_has_hard_lock(proposal_height, existing_vote)
                || self.local_same_height_vote_has_live_proposal_material(
                    proposal_height,
                    existing_vote.block_hash,
                );
        }
        if !self.config.resilience.enabled {
            return true;
        }
        if self.local_same_height_vote_has_hard_lock(proposal_height, existing_vote) {
            return true;
        }
        let min_stale_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1));
        let hard_stale_age = min_stale_age.saturating_mul(3);
        let recovery_exhausted = self
            .stale_same_height_recovery_age(proposal_height, existing_vote.view, now)
            .is_some_and(|age| age >= hard_stale_age)
            || self.same_height_vote_recovery_view_gap_exhausted(
                existing_vote.view,
                proposal_view,
                self.effective_commit_topology().len(),
            );
        if let Some(pending) = self
            .pending
            .pending_blocks
            .get(&existing_vote.block_hash)
            .filter(|pending| {
                pending.height == proposal_height
                    && pending.view == existing_vote.view
                    && !pending.aborted
                    && pending.validation_status != ValidationStatus::Invalid
            })
        {
            let tip_height = self.state.committed_height();
            let tip_hash = self.state.latest_block_hash_fast();
            let pending_age = pending
                .progress_age(now)
                .max(now.saturating_duration_since(pending.inserted_at));
            let active_tip_owner = block_active_tip_owner
                && self.pending_block_is_active_for_tip(
                    existing_vote.block_hash,
                    pending,
                    tip_height,
                    tip_hash,
                );
            // Keep old-view active-owner protection through the hard stale window, but do
            // not let a no-QC branch anchor fresh proposal assembly forever.
            let stale_age = if active_tip_owner {
                min_stale_age.saturating_mul(3)
            } else {
                min_stale_age
            };
            if pending_age < stale_age && !recovery_exhausted {
                return true;
            }
        }
        if self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.height == proposal_height
                && slot.view == existing_vote.view
                && slot.block_hash == existing_vote.block_hash
                && (slot.quorum_progress.commit_qc_observed
                    || self.frontier_slot_competing_quorum_locked_for_view(slot, proposal_view))
        }) {
            return true;
        }
        false
    }

    pub(super) fn local_same_height_vote_blocks_fresh_proposal_assembly(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
        now: Instant,
        block_active_tip_owner: bool,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> bool {
        if !self.local_same_height_vote_blocks_fresh_proposal(
            proposal_height,
            proposal_view,
            existing_vote,
            now,
            block_active_tip_owner,
        ) {
            return false;
        }
        !self.stale_local_commit_vote_allows_proposal_assembly_after_missing_qc_repair(
            proposal_height,
            proposal_view,
            existing_vote,
            now,
            highest_qc,
        )
    }

    fn stale_local_commit_vote_allows_proposal_assembly_after_missing_qc_repair(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
        now: Instant,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> bool {
        if !self.config.resilience.enabled
            || !matches!(
                existing_vote.phase,
                crate::sumeragi::consensus::Phase::Commit
            )
            || proposal_view <= existing_vote.view
            || proposal_height != self.committed_height_snapshot().saturating_add(1)
            || !self.frontier_missing_qc_liveness_active(proposal_height, proposal_view)
            || highest_qc.height.saturating_add(1) != proposal_height
            || !self.highest_qc_is_canonical_committed_tip(highest_qc)
        {
            return false;
        }
        if self.local_same_height_vote_is_committed_parent_marker(
            proposal_height,
            proposal_view,
            existing_vote,
        ) {
            return true;
        }
        if self.local_same_height_vote_has_consensus_lock(proposal_height, existing_vote)
            || self.same_height_block_has_observed_qc(
                existing_vote.block_hash,
                proposal_height,
                existing_vote.view,
            )
            || self.same_height_has_recoverable_qc(proposal_height)
            || self
                .same_height_vote_lock_blocking_candidate(proposal_height, proposal_view, None)
                .is_some()
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == existing_vote.block_hash
                        && inflight.pending.height == proposal_height
                        && inflight.pending.view == existing_vote.view
                        && !inflight.pending.aborted
                        && inflight.pending.validation_status != ValidationStatus::Invalid
                })
        {
            return false;
        }

        let repair_window = self
            .known_block_commit_qc_recovery_view_change_window()
            .max(self.quorum_timeout(self.runtime_da_enabled()))
            .max(Duration::from_millis(1));
        self.pending
            .pending_blocks
            .get(&existing_vote.block_hash)
            .filter(|pending| {
                pending.height == proposal_height && pending.view == existing_vote.view
            })
            .is_none_or(|pending| {
                pending.is_retired_same_height()
                    || pending.is_retry_aborted()
                    || pending.validation_status == ValidationStatus::Invalid
                    || (!pending.commit_qc_observed()
                        && pending
                            .progress_age(now)
                            .max(now.saturating_duration_since(pending.inserted_at))
                            >= repair_window)
            })
    }

    pub(super) fn local_same_height_vote_is_committed_parent_marker(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        existing_vote.view == proposal_view
            && proposal_height > 0
            && self.committed_block_hash_for_height(proposal_height.saturating_sub(1))
                == Some(existing_vote.block_hash)
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
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
        self.assemble_and_broadcast_proposal_with_recovery_heartbeat(
            height,
            view,
            highest_qc,
            topology,
            leader_index,
            local_validator_index,
            view_snapshot,
            now,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn assemble_and_broadcast_proposal_with_recovery_heartbeat(
        &mut self,
        height: u64,
        view: u64,
        mut highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        topology: &mut super::network_topology::Topology,
        leader_index: usize,
        local_validator_index: u32,
        view_snapshot: Option<StateView<'_>>,
        now: Instant,
        allow_recovery_heartbeat: bool,
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
        if let Some(lock) =
            self.same_height_vote_lock_blocking_candidate(proposal_height, view, None)
        {
            if self.new_view_qc_supersedes_same_height_vote_lock(
                proposal_height,
                view,
                highest_qc,
                &lock,
            ) {
                info!(
                    height = proposal_height,
                    view,
                    epoch = proposal_epoch,
                    locked_block = %lock.block_hash,
                    locked_view = lock.view,
                    locked_votes = lock.vote_count,
                    conflicting_voters = lock.conflicting_voters,
                    candidate_possible_votes = lock.candidate_possible_votes,
                    required = lock.required,
                    total_validators = lock.total_validators,
                    "allowing proposal assembly: NEW_VIEW QC supersedes raw same-height vote lock"
                );
            } else {
                let min_stale_age = self
                    .quorum_timeout(self.runtime_da_enabled())
                    .max(self.frontier_slot_lag_window())
                    .max(Duration::from_millis(1));
                let hard_stale_age = min_stale_age.saturating_mul(3);
                let recovery_exhausted = self
                    .stale_same_height_recovery_age(proposal_height, lock.view, now)
                    .is_some_and(|age| age >= hard_stale_age)
                    || self.same_height_vote_recovery_view_gap_exhausted(
                        lock.view,
                        view,
                        lock.total_validators,
                    );
                let recovery_escalation_due = recovery_exhausted
                    || self.same_height_vote_recovery_escalation_view_gap_exhausted(
                        lock.view,
                        view,
                        lock.total_validators,
                    );
                let qc_observed = self.same_height_block_has_observed_qc(
                    lock.block_hash,
                    proposal_height,
                    lock.view,
                );
                let _ = self.seed_frontier_slot_from_same_height_evidence(
                    proposal_height,
                    view,
                    now,
                    "vote_locked_same_height",
                    false,
                );
                let highest_qc_dependency_repair_requested = if recovery_escalation_due
                    && highest_qc.height.saturating_add(1) == proposal_height
                    && !self.block_known_locally(highest_qc.subject_block_hash)
                {
                    let _ = self.mark_highest_qc_missing_defer_for_round(
                        proposal_height,
                        view,
                        highest_qc,
                    );
                    self.observe_new_view_highest_qc_exact_repair(highest_qc)
                } else {
                    false
                };
                let stale_vote_locked_recovery_requested = recovery_escalation_due
                    && !qc_observed
                    && self.escalate_stale_vote_locked_frontier_owner_recovery(
                        lock.block_hash,
                        proposal_height,
                        lock.view,
                        now,
                        "exhausted_vote_locked_same_height",
                    );
                warn!(
                    height = proposal_height,
                    view,
                    epoch = proposal_epoch,
                    locked_block = %lock.block_hash,
                    locked_view = lock.view,
                    locked_votes = lock.vote_count,
                    conflicting_voters = lock.conflicting_voters,
                    candidate_possible_votes = lock.candidate_possible_votes,
                    required = lock.required,
                    total_validators = lock.total_validators,
                    recovery_exhausted,
                    recovery_escalation_due,
                    qc_observed,
                    highest_qc_dependency_repair_requested,
                    highest_qc_height = highest_qc.height,
                    highest_qc_view = highest_qc.view,
                    highest_qc_block = %highest_qc.subject_block_hash,
                    stale_vote_locked_recovery_requested,
                    "deferring proposal assembly: same-height vote history makes a fresh branch non-viable"
                );
                return Ok(false);
            }
        }
        if let Some(existing_vote) = self.local_same_height_vote(proposal_height, proposal_epoch) {
            let new_view_qc_supersedes = self.new_view_qc_supersedes_same_height_vote_conflict(
                proposal_height,
                view,
                highest_qc,
                existing_vote.block_hash,
                existing_vote.view,
            );
            if new_view_qc_supersedes
                || !self.local_same_height_vote_blocks_fresh_proposal_assembly(
                    proposal_height,
                    view,
                    &existing_vote,
                    now,
                    true,
                    highest_qc,
                )
            {
                debug!(
                    height = proposal_height,
                    view,
                    epoch = proposal_epoch,
                    voted_view = existing_vote.view,
                    voted_phase = ?existing_vote.phase,
                    voted_block = %existing_vote.block_hash,
                    new_view_qc_supersedes,
                    "allowing fresh proposal after stale prior-view local same-height vote"
                );
            } else {
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
        }
        if self.same_height_vote_verification_pending_at_or_before_view(
            proposal_height,
            view,
            proposal_epoch,
        ) {
            debug!(
                height = proposal_height,
                view,
                epoch = proposal_epoch,
                "deferring proposal assembly: same-height vote verification is pending"
            );
            return Ok(false);
        }
        if let Err(LockedQcRejection::HeightRegressed { locked, highest }) =
            ensure_locked_qc_allows(self.locked_qc, highest_qc)
        {
            let Some(lock) = self.promote_locked_qc_to_highest_if_needed("proposal_assembly")
            else {
                return Ok(false);
            };
            info!(
                locked_height = locked,
                highest_height = highest,
                height = proposal_height,
                view,
                lock_hash = %lock.subject_block_hash,
                "replacing regressed highest QC with locked QC for direct proposal assembly"
            );
            highest_qc = lock;
        }
        let _lock_lag_highest_qc_deferred = !self.highest_qc_extends_locked(highest_qc)
            && self.defer_highest_qc_update_for_lock_catchup(
                height, view, highest_qc, now, "proposal",
            );
        if proposal_height > 1 && !self.block_known_locally(highest_qc.subject_block_hash) {
            if self.mark_highest_qc_missing_defer_for_round(proposal_height, view, highest_qc) {
                self.observe_new_view_highest_qc_exact_repair(highest_qc);
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
                    self.observe_new_view_highest_qc_exact_repair(highest_qc);
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

        let preflight_elapsed_ms = now.elapsed().as_millis();
        let queue_len = self.queue.queued_len();
        let mut tx_guards = Vec::new();
        let tx_select_started_at = Instant::now();
        let (
            _block_digest,
            conf_features,
            mut transactions,
            mut routing_decisions,
            mut routing_plans,
            mut tx_sizes,
            deferred_transactions,
        ) = {
            let (block_max_param, commit_time_ms, effective_commit_time_ms, base_gas_limit, digest) =
                if let Some(state_view) = view_snapshot.as_ref() {
                    let block_max_param =
                        state_view.world().parameters().block().max_transactions();
                    let sumeragi_params = state_view.world().parameters().sumeragi();
                    let commit_time_ms = sumeragi_params.commit_time_ms();
                    let effective_commit_time_ms = sumeragi_params.effective_commit_time_ms();
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
                        commit_time_ms,
                        effective_commit_time_ms,
                        base_gas_limit,
                        digest,
                    )
                } else {
                    let world = self.state.world_view();
                    let block_max_param = world.parameters().block().max_transactions();
                    let sumeragi_params = world.parameters().sumeragi();
                    let commit_time_ms = sumeragi_params.commit_time_ms();
                    let effective_commit_time_ms = sumeragi_params.effective_commit_time_ms();
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
                        commit_time_ms,
                        effective_commit_time_ms,
                        base_gas_limit,
                        digest,
                    )
                };
            let (max_tx_target, max_in_block, fast_tx_capped) = Self::max_tx_budget_for_commit_time(
                queue_len,
                block_max_param.get(),
                self.config.block.max_transactions,
                self.config.block.fast_finality_max_transactions,
                commit_time_ms,
                effective_commit_time_ms,
            );
            let fast_gas_limit_per_block = self.config.block.fast_gas_limit_per_block;
            let proposal_gas_limit = Self::cap_gas_limit_for_fast_commit(
                base_gas_limit,
                commit_time_ms,
                effective_commit_time_ms,
                fast_gas_limit_per_block,
            );
            let fast_gas_capped = proposal_gas_limit != base_gas_limit;
            let scan_budget = self.proposal_scan_budget(max_in_block);
            let max_ivm_transactions = self.config.block.max_ivm_transactions;
            let replay_ivm_proved = {
                let state_view = self.state.view();
                let pipeline = state_view.pipeline();
                pipeline.ivm_proved.enabled && !pipeline.ivm_proved.skip_replay
            };
            info!(
                height,
                view,
                queue_len,
                max_tx_param = block_max_param.get(),
                max_tx_target,
                max_in_block = max_in_block.get(),
                max_ivm_transactions = max_ivm_transactions.map(NonZeroUsize::get),
                scan_budget,
                scan_multiplier = self.config.block.proposal_queue_scan_multiplier.get(),
                commit_time_ms,
                effective_commit_time_ms,
                gas_limit_per_block = base_gas_limit.map(NonZeroU64::get),
                fast_gas_limit_per_block = fast_gas_limit_per_block.map(NonZeroU64::get),
                proposal_gas_limit = proposal_gas_limit.map(NonZeroU64::get),
                fast_gas_capped,
                fast_tx_capped,
                "proposal assembly budget"
            );
            // Bound queue scanning to keep proposal assembly from stalling under sustained load.
            let deferred_accumulator = self.pull_transactions_for_proposal(
                self.state.as_ref(),
                max_in_block,
                scan_budget,
                proposal_gas_limit,
                max_ivm_transactions,
                replay_ivm_proved,
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
            let routing_plans: Vec<crate::queue::RoutingPlan> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::routing_plan)
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
                routing_plans,
                tx_sizes,
                deferred_accumulator,
            )
        };
        let tx_select_ms = tx_select_started_at.elapsed().as_millis();

        let tx_prepare_started_at = Instant::now();
        let (
            filtered_guards,
            filtered_transactions,
            filtered_routing,
            filtered_routing_plans,
            filtered_sizes,
            _dropped,
        ) = Self::filter_committed_transactions_for_proposal(
            self.state.as_ref(),
            tx_guards,
            transactions,
            routing_decisions,
            routing_plans,
            tx_sizes,
            height,
            view,
        );
        tx_guards = filtered_guards;
        transactions = filtered_transactions;
        routing_decisions = filtered_routing;
        routing_plans = filtered_routing_plans;
        tx_sizes = filtered_sizes;

        if transactions.len() > 1 {
            // Lane interleaving is a budget-selection policy only. The default block builder
            // still canonicalizes normal-lane payload order by entrypoint hash for consensus.
            let order = interleave_lane_indices_for_slot(&routing_decisions, height, view);

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
                reorder_vec(&mut routing_plans, &order);
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
            if allow_recovery_heartbeat {
                let heartbeat = self.build_recovery_heartbeat_transaction(proposal_height)?;
                let encoded_len = heartbeat.encoded_len();
                transactions.push(heartbeat);
                routing_decisions.push(RoutingDecision::default());
                routing_plans.push(crate::queue::RoutingPlan::single(RoutingDecision::default()));
                tx_sizes.push(encoded_len);
                info!(
                    height = proposal_height,
                    view,
                    queue_len = queue_len_after_pop,
                    "injecting recovery heartbeat transaction for empty leader queue"
                );
                None
            } else {
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
            }
        } else {
            None
        };

        let da_enabled = self.runtime_da_enabled();
        let mut overflow_transactions: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut oversized_frame_len: Option<usize> = None;
        let tx_sizes_in = tx_sizes;
        let routing_plans_in = routing_plans;
        let mut tx_batch;
        let mut routing_batch;
        let mut routing_plan_batch;
        let mut tx_sizes;
        if da_enabled {
            let mut remaining_budget = da_payload_budget(
                self.config.rbc.chunk_max_bytes,
                self.config.rbc.pending_max_bytes,
                self.config.rbc.pending_max_chunks,
                self.config.block.max_payload_bytes,
            );
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            routing_plan_batch = Vec::with_capacity(routing_plans_in.len());
            let mut tx_sizes_out = Vec::with_capacity(transactions.len());
            for (((tx, routing), routing_plan), encoded_len) in transactions
                .into_iter()
                .zip(routing_decisions.into_iter())
                .zip(routing_plans_in.into_iter())
                .zip(tx_sizes_in.into_iter())
            {
                if encoded_len > self.consensus_payload_frame_cap {
                    oversized_frame_len =
                        Some(oversized_frame_len.map_or(encoded_len, |prev| prev.max(encoded_len)));
                }
                if encoded_len > remaining_budget {
                    overflow_transactions.push((tx, routing_plan));
                    continue;
                }
                remaining_budget = remaining_budget.saturating_sub(encoded_len);
                tx_sizes_out.push(encoded_len);
                tx_batch.push(tx);
                routing_batch.push(routing);
                routing_plan_batch.push(routing_plan);
            }
            tx_sizes = tx_sizes_out;
        } else {
            let mut payload_budget = Some(non_rbc_payload_budget(
                self.config.block.max_payload_bytes,
                self.consensus_payload_frame_cap,
            ));
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            routing_plan_batch = Vec::with_capacity(routing_plans_in.len());
            let mut tx_sizes_out = Vec::with_capacity(transactions.len());
            for (((tx, routing), routing_plan), encoded_len) in transactions
                .into_iter()
                .zip(routing_decisions.into_iter())
                .zip(routing_plans_in.into_iter())
                .zip(tx_sizes_in.into_iter())
            {
                if let Some(budget) = payload_budget {
                    if encoded_len > budget {
                        overflow_transactions.push((tx, routing_plan));
                        continue;
                    }
                    payload_budget = Some(budget.saturating_sub(encoded_len));
                    tx_sizes_out.push(encoded_len);
                }
                tx_batch.push(tx);
                routing_batch.push(routing);
                routing_plan_batch.push(routing_plan);
            }
            tx_sizes = tx_sizes_out;
        }

        if tx_batch.len() > 1 {
            if let Some(admin_idx) = tx_batch.iter().position(Self::is_peer_admin_transaction) {
                let admin_tx = tx_batch.remove(admin_idx);
                let admin_route = routing_batch.remove(admin_idx);
                let admin_plan = routing_plan_batch.remove(admin_idx);
                let admin_size = tx_sizes.remove(admin_idx);
                overflow_transactions.extend(tx_batch.drain(..).zip(routing_plan_batch.drain(..)));
                routing_batch.clear();
                tx_sizes.clear();
                tx_batch.push(admin_tx);
                routing_batch.push(admin_route);
                routing_plan_batch.push(admin_plan);
                tx_sizes.push(admin_size);
            }
        }
        canonicalize_proposal_batch_with_plans(
            &mut tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &mut tx_sizes,
        );
        let tx_prepare_ms = tx_prepare_started_at.elapsed().as_millis();

        let native_precheck_started_at = Instant::now();
        if let Err(reason) =
            self.native_amx_receipts_for_batch(&tx_batch, &routing_plan_batch, proposal_height)
        {
            self.queue.release_transaction_guards(&mut tx_guards);
            for (tx, routing) in tx_batch.drain(..).zip(routing_plan_batch.drain(..)) {
                if crate::tx::is_heartbeat_accepted_transaction(&tx) {
                    continue;
                }
                self.requeue_accepted_transaction(tx, routing, "native AMX attestations pending");
            }
            routing_batch.clear();
            tx_sizes.clear();
            for (tx, routing) in std::mem::take(&mut overflow_transactions) {
                if crate::tx::is_heartbeat_accepted_transaction(&tx) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction overflowed by RBC budget",
                );
            }
            info!(
                height = proposal_height,
                view,
                reason,
                "deferring proposal while native AMX participant attestations are collected"
            );
            return Ok(false);
        }
        let native_precheck_ms = native_precheck_started_at.elapsed().as_millis();

        if tx_batch.is_empty() {
            self.queue.release_transaction_guards(&mut tx_guards);
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

        let original_for_requeue: Vec<(AcceptedTransaction<'static>, crate::queue::RoutingPlan)> =
            tx_batch
                .iter()
                .cloned()
                .zip(routing_plan_batch.iter().cloned())
                .collect();
        let previous_roster_started_at = Instant::now();
        let previous_roster_evidence = prev_block.as_deref().and_then(|parent| {
            previous_roster_evidence_for_parent(
                self.state.as_ref(),
                self.kura.as_ref(),
                self.consensus_context_for_height(parent.header().height().get())
                    .0,
                parent,
            )
        });
        let previous_roster_ms = previous_roster_started_at.elapsed().as_millis();
        let mut removed_for_chunk_cap: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut removed_for_frame_cap: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut last_sidecar_ms = 0_u128;
        let mut last_block_build_ms = 0_u128;
        let mut last_payload_encode_ms = 0_u128;
        let mut last_frontier_wire_ms = 0_u128;
        let block_loop_started_at = Instant::now();
        let assembly_result: Result<()> = (|| {
            if tx_sizes.len() < tx_batch.len() {
                for tx in tx_batch.iter().skip(tx_sizes.len()) {
                    tx_sizes.push(tx.encoded_len());
                }
            }
            canonicalize_proposal_batch_with_plans(
                &mut tx_batch,
                &mut routing_batch,
                &mut routing_plan_batch,
                &mut tx_sizes,
            );
            let (
                signed_block,
                block_created_msg,
                payload_bytes,
                payload_hash,
                proposal,
                proposal_hint,
                transactions_for_plan,
                tx_sizes_for_plan,
                block_hash,
                block_created_frame_len,
            ) = loop {
                let sidecar_started_at = Instant::now();
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
                let npos_effects =
                    self.build_npos_consensus_effects_for_proposal(proposal_height)?;
                builder = builder.with_npos_consensus_effects(npos_effects);
                let sccp_messages =
                    crate::bridge::collect_sccp_messages_from_accepted_transactions(&tx_batch);
                builder = builder.with_sccp_commitment_root(
                    crate::bridge::sccp_commitment_root_from_messages(&sccp_messages),
                );

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

                let native_amx_receipts = self
                    .native_amx_receipts_for_batch(&tx_batch, &routing_plan_batch, proposal_height)
                    .map_err(|reason| {
                        eyre!("native AMX participant attestations unavailable: {reason}")
                    })?;
                let execution_context = tx_batch
                    .iter()
                    .zip(routing_plan_batch.iter())
                    .zip(native_amx_receipts.into_iter())
                    .map(|((tx, plan), receipt)| {
                        let context = crate::queue::execution_context_for_routing_plan(
                            tx.hash_as_entrypoint(),
                            plan,
                        );
                        if let Some(receipt) = receipt {
                            context.with_native_amx_receipt(receipt)
                        } else {
                            context
                        }
                    })
                    .collect::<Vec<_>>();
                if !execution_context.is_empty() {
                    builder = builder.with_execution_context(Some(
                        BlockExecutionContextBundle::new(execution_context),
                    ));
                }
                last_sidecar_ms = sidecar_started_at.elapsed().as_millis();

                let block_build_started_at = Instant::now();
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
                last_block_build_ms = block_build_started_at.elapsed().as_millis();
                let payload_encode_started_at = Instant::now();
                let payload_bytes = block_payload_bytes(&signed_block);
                last_payload_encode_ms = payload_encode_started_at.elapsed().as_millis();
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
                            let _removed_routing = routing_batch
                                .pop()
                                .expect("routing batch should align with tx batch");
                            let removed_plan = routing_plan_batch
                                .pop()
                                .expect("routing plan batch should align with tx batch");
                            let _ = tx_sizes.pop();
                            removed_for_chunk_cap.push((removed_tx, removed_plan));
                            continue;
                        }
                    }
                }
                let frontier_wire_started_at = Instant::now();
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
                    .frontier_block_created_for_local_proposal_wire_with_payload(
                        &signed_block,
                        &proposal,
                        topology.as_ref(),
                        &payload_bytes,
                        payload_hash,
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
                if frame_len > self.consensus_payload_frame_cap && !da_enabled {
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
                    let removed = trim_batch_for_size_cap_with_plans(
                        &mut tx_batch,
                        &mut routing_batch,
                        &mut routing_plan_batch,
                        &mut tx_sizes,
                        &mut removed_for_frame_cap,
                        excess,
                    );
                    if removed == 0 {
                        if let Some(removed_tx) = tx_batch.pop() {
                            let _removed_routing = routing_batch
                                .pop()
                                .expect("routing batch should align with tx batch");
                            let removed_plan = routing_plan_batch
                                .pop()
                                .expect("routing plan batch should align with tx batch");
                            let _ = tx_sizes.pop();
                            removed_for_frame_cap.push((removed_tx, removed_plan));
                            continue;
                        }
                    }
                    continue;
                }
                last_frontier_wire_ms = frontier_wire_started_at.elapsed().as_millis();

                break (
                    signed_block,
                    block_created_msg,
                    payload_bytes,
                    payload_hash,
                    proposal,
                    proposal_hint,
                    tx_batch.clone(),
                    tx_sizes.clone(),
                    block_hash,
                    frame_len,
                );
            };
            let block_loop_ms = block_loop_started_at.elapsed().as_millis();

            let elapsed = now.elapsed();
            let base_stale_window = self
                .quorum_timeout(da_enabled)
                .max(Duration::from_millis(1));
            let stale_window = Self::proposal_assembly_stale_window(
                base_stale_window,
                transactions_for_plan.len(),
            );
            if elapsed >= stale_window {
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(proposal_height, view);
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(proposal_height, view);
                warn!(
                    height = proposal_height,
                    view,
                    elapsed_ms = elapsed.as_millis(),
                    base_stale_window_ms = base_stale_window.as_millis(),
                    stale_window_ms = stale_window.as_millis(),
                    tx_count = transactions_for_plan.len(),
                    block_hash = %block_hash,
                    preflight_elapsed_ms,
                    tx_select_ms,
                    tx_prepare_ms,
                    native_precheck_ms,
                    previous_roster_ms,
                    block_loop_ms,
                    sidecar_ms = last_sidecar_ms,
                    block_build_ms = last_block_build_ms,
                    payload_encode_ms = last_payload_encode_ms,
                    frontier_wire_ms = last_frontier_wire_ms,
                    "aborting stale proposal assembly before broadcast"
                );
                return Err(eyre!(
                    "proposal assembly exceeded view window: elapsed={}ms window={}ms",
                    elapsed.as_millis(),
                    stale_window.as_millis()
                ));
            }

            // Loop back consensus messages locally so the leader participates immediately.
            let frontier_block_created_ready = matches!(
                &block_created_msg,
                BlockMessage::BlockCreated(created) if created.frontier.is_some()
            );
            let block_created_frame_fits =
                block_created_frame_len <= self.consensus_payload_frame_cap;
            self.subsystems
                .propose
                .proposal_cache
                .insert_hint(proposal_hint);
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal);
            let frontier_rbc_transport_needed = frontier_block_created_ready
                && (rbc::chunk_count(payload_bytes.len(), self.config.rbc.chunk_max_bytes) > 1
                    || !block_created_frame_fits);
            let inline_frontier_block_created_transport =
                frontier_block_created_ready && !frontier_rbc_transport_needed;
            let seed_frontier_backup_transport = should_seed_frontier_backup_transport(
                da_enabled,
                inline_frontier_block_created_transport,
                self.config.rbc.inline_block_created_backup,
            );
            let use_rbc_transport = da_enabled
                && (!inline_frontier_block_created_transport || seed_frontier_backup_transport);
            let mut rbc_plan = if use_rbc_transport {
                // Inline single-frame frontier BlockCreated messages can skip RBC body backup
                // when configured to favor low-latency steady-state transport. Multi-chunk,
                // oversized, and non-frontier recovery payloads continue to rely on RBC as
                // their primary DA body transport.
                self.prepare_rbc_plan(rbc::RbcPlanInputs {
                    signed_block: &signed_block,
                    transactions: &transactions_for_plan,
                    routing: &routing_batch,
                    tx_sizes: &tx_sizes_for_plan,
                    payload: &payload_bytes,
                    payload_hash,
                    height: proposal_height,
                    view,
                    epoch: proposal_epoch,
                    local_validator_index,
                })?
            } else {
                None
            };
            drop(payload_bytes);

            if let Some(plan) = rbc_plan.as_ref() {
                // Non-frontier recovery always uses RBC transport. Frontier proposals keep the
                // inline fast path only when the exact body fits a consensus frame; multi-chunk
                // or otherwise oversized BlockCreated bodies use Proposal + RBC.
                self.install_rbc_session_plan(&plan.primary)?;
                if let Some(dup) = plan.duplicate.as_ref() {
                    self.install_rbc_session_plan(dup)?;
                }
                self.publish_rbc_backlog_snapshot();
            }

            let block_created_wire = block_created_frame_fits.then(|| {
                let wire = Arc::new(block_created_msg.clone());
                let encoded = Arc::new(BlockMessageWire::encode_message(wire.as_ref()));
                (wire, encoded)
            });
            // A locally assembled proposal is authoritative evidence that this slot was observed,
            // even when the inline BlockCreated path skips proposal handling or validation consumes
            // and reinserts the proposal cache entry.
            self.note_proposal_seen(proposal_height, view, payload_hash);

            let topology_peers = topology.as_ref();
            let local_peer_id = self.common_config.peer.id().clone();
            // Put the exact body on the wire before local self-processing can emit READY/QC
            // evidence. Multi-chunk frontier payloads still use Proposal + RBC for DA transport,
            // but the body companion prevents a single missed chunk from pushing peers onto the
            // slower recovery path.
            if let Some((block_created_wire, block_created_encoded)) = block_created_wire.as_ref() {
                for peer in topology_peers {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: BlockMessageWire::with_encoded(
                            Arc::clone(block_created_wire),
                            Arc::clone(block_created_encoded),
                        ),
                    });
                }
            } else {
                debug!(
                    height = proposal_height,
                    view,
                    frame_len = block_created_frame_len,
                    cap = self.consensus_payload_frame_cap,
                    "skipping exact BlockCreated companion; Proposal + RBC will carry the payload"
                );
            }
            self.post_proposal_metadata_to_topology(
                topology_peers,
                &local_peer_id,
                proposal_hint,
                proposal,
            );
            if let Some(plan) = rbc_plan.take() {
                self.broadcast_rbc_session_plan(plan.primary)?;
                if let Some(dup) = plan.duplicate {
                    self.broadcast_rbc_session_plan(dup)?;
                }
            }

            if let BlockMessage::BlockCreated(block_msg) = block_created_msg {
                self.handle_block_created(block_msg, None)?;
            }
            if !inline_frontier_block_created_transport {
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

            let relay_envelopes = crate::sumeragi::status::lane_relay_envelopes_snapshot();
            if !relay_envelopes.is_empty() {
                self.subsystems.merge.lane_relay.broadcast(relay_envelopes);
            }

            self.record_phase_sample(PipelinePhase::Propose, proposal_height, view);

            let tx_count = transactions_for_plan.len();
            iroha_logger::info!(
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
            self.queue.release_transaction_guards(&mut tx_guards);
            for (tx, routing) in original_for_requeue {
                if crate::tx::is_heartbeat_accepted_transaction(&tx) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction after assembly failure",
                );
            }
            for (tx, routing) in std::mem::take(&mut overflow_transactions) {
                if crate::tx::is_heartbeat_accepted_transaction(&tx) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction overflowed by RBC budget",
                );
            }
            for (tx, routing) in removed_for_chunk_cap {
                if crate::tx::is_heartbeat_accepted_transaction(&tx) {
                    continue;
                }
                self.requeue_accepted_transaction(
                    tx,
                    routing,
                    "failed to requeue transaction trimmed by RBC chunk cap",
                );
            }
            for (tx, routing) in removed_for_frame_cap {
                if crate::tx::is_heartbeat_accepted_transaction(&tx) {
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
        self.queue.release_transaction_guards(&mut tx_guards);
        for (tx, routing) in std::mem::take(&mut overflow_transactions) {
            if crate::tx::is_heartbeat_accepted_transaction(&tx) {
                continue;
            }
            self.requeue_accepted_transaction(
                tx,
                routing,
                "failed to requeue transaction overflowed by RBC budget",
            );
        }
        for (tx, routing) in removed_for_chunk_cap {
            if crate::tx::is_heartbeat_accepted_transaction(&tx) {
                continue;
            }
            self.requeue_accepted_transaction(
                tx,
                routing,
                "failed to requeue transaction trimmed by RBC chunk cap",
            );
        }
        for (tx, routing) in removed_for_frame_cap {
            if crate::tx::is_heartbeat_accepted_transaction(&tx) {
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

    fn build_recovery_heartbeat_transaction(
        &self,
        proposal_height: u64,
    ) -> Result<AcceptedTransaction<'static>> {
        let (max_clock_drift, tx_limits) = {
            let world = self.state.world_view();
            let params = world.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let time_source = iroha_primitives::time::TimeSource::new_system();
        let signed = crate::tx::build_heartbeat_transaction_with_time_source(
            self.state.chain_id_ref().clone(),
            &self.common_config.key_pair,
            &tx_limits,
            proposal_height,
            &time_source,
        );
        let crypto = self.state.crypto();
        AcceptedTransaction::accept_with_time_source(
            signed,
            self.state.chain_id_ref(),
            max_clock_drift,
            tx_limits,
            &crypto,
            &time_source,
        )
        .map_err(|err| eyre!("failed to build recovery heartbeat transaction: {err}"))
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

    pub(super) fn post_proposal_metadata_to_topology(
        &mut self,
        topology_peers: &[PeerId],
        local_peer_id: &PeerId,
        proposal_hint: super::message::ProposalHint,
        proposal: crate::sumeragi::consensus::Proposal,
    ) -> usize {
        let proposal_hint_msg = Arc::new(BlockMessage::ProposalHint(proposal_hint));
        let proposal_hint_encoded =
            Arc::new(BlockMessageWire::encode_message(proposal_hint_msg.as_ref()));
        let proposal_msg = Arc::new(BlockMessage::Proposal(proposal));
        let proposal_encoded = Arc::new(BlockMessageWire::encode_message(proposal_msg.as_ref()));
        let mut scheduled = 0usize;

        for peer in topology_peers {
            if peer == local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&proposal_hint_msg),
                    Arc::clone(&proposal_hint_encoded),
                ),
            });
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&proposal_msg),
                    Arc::clone(&proposal_encoded),
                ),
            });
            scheduled = scheduled.saturating_add(1);
        }

        scheduled
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

    fn missing_qc_frontier_backpressure_override_active(&self, now: Instant) -> bool {
        if !self.config.resilience.enabled || self.queue.active_len() == 0 {
            return false;
        }
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let height = committed_height.saturating_add(1);
        let Some(view) = self.phase_tracker.current_view(height) else {
            return false;
        };
        if view == 0 || !self.frontier_missing_qc_liveness_active(height, view) {
            return false;
        }
        self.frontier_proposal_or_view_starved_past_ingress_grace(
            height,
            now,
            self.frontier_ingress_drain_grace(self.runtime_da_enabled()),
        )
    }

    pub(super) fn proposal_backpressure_at(&mut self, now: Instant) -> ProposalBackpressure {
        self.subsystems.propose.backpressure_gate.refresh();
        let mut queue_state = self.subsystems.propose.backpressure_gate.state();
        let queue_pressure = self.queue.pressure_snapshot();
        if queue_pressure.saturated_by_age && !queue_pressure.saturated_by_count {
            // Age-only pressure means transactions have waited too long; consensus should keep
            // proposing instead of treating that condition as a reason to wait even longer.
            queue_state = BackpressureState::Healthy {
                queued: queue_state.queued(),
                capacity: queue_state.capacity(),
            };
        }
        let blocking_pending = self.blocking_pending_blocks_len_with_progress(now);
        let queue_depths = status::worker_queue_depth_snapshot();
        let mut consensus_queue_backpressure = consensus_queue_backpressure(
            queue_depths,
            self.config.queues.block_payload,
            self.config.queues.rbc_chunks,
        );
        let backpressure_override_due = self.backpressure_override_due(now);
        let missing_qc_frontier_override =
            self.missing_qc_frontier_backpressure_override_active(now);
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let (pending_votes_or_qc, live_pending_under_congestion) =
            self.pending.pending_blocks.values().fold(
                (false, false),
                |(has_votes_or_qc, has_live_pending), pending| {
                    if has_votes_or_qc && has_live_pending {
                        return (has_votes_or_qc, has_live_pending);
                    }
                    if pending.aborted || pending.validation_status == ValidationStatus::Invalid {
                        return (has_votes_or_qc, has_live_pending);
                    }
                    let block_hash = pending.block.hash();
                    let extends_tip = super::pending_extends_tip(
                        pending.height,
                        pending.block.header().prev_block_hash(),
                        tip_height,
                        tip_hash,
                    );
                    let has_consensus_progress = pending.local_commit_vote_emitted()
                        || pending.commit_qc_observed()
                        || self.pending_block_has_votes(block_hash, pending.height, pending.view)
                        || self.pending_block_has_qc(block_hash, pending.height, pending.view);
                    (
                        has_votes_or_qc || has_consensus_progress,
                        // In normal operation, payload-only pending blocks stay on the fast path.
                        // Under saturation, live pending blocks at or beyond the frontier become
                        // a proposal pacing signal so targeted load cannot churn around recovery.
                        has_live_pending
                            || extends_tip
                            || pending.height
                                > u64::try_from(tip_height.saturating_add(1)).unwrap_or(u64::MAX),
                    )
                },
            );
        let ingress_starvation_override = self.config.resilience.enabled
            && self.queue.active_len() > 0
            && (backpressure_override_due
                || missing_qc_frontier_override
                || self.frontier_proposal_starved_past_ingress_grace(
                    now,
                    self.frontier_ingress_drain_grace(self.runtime_da_enabled()),
                ));
        let congested_tip_pending = (queue_state.is_saturated() || consensus_queue_backpressure)
            && live_pending_under_congestion
            && !ingress_starvation_override;
        let mut active_pending = pending_votes_or_qc
            || congested_tip_pending
            || blocking_pending > self.config.pacemaker.active_pending_soft_limit;
        let rbc_backlog_summary = self.proposal_rbc_backlog_summary();
        let mut rbc_backlog = self.rbc_backlog_exceeds_pacemaker_soft_limits(rbc_backlog_summary);
        let liveness_backpressure_override =
            backpressure_override_due || missing_qc_frontier_override;
        let relay_backpressure = if liveness_backpressure_override {
            // Liveness override: don't let prolonged relay/RBC backpressure stall proposals.
            rbc_backlog = false;
            false
        } else {
            self.relay_backpressure_active(now, self.rebroadcast_cooldown())
        };
        if missing_qc_frontier_override {
            active_pending = false;
            consensus_queue_backpressure = false;
            if queue_state.is_saturated() {
                queue_state = BackpressureState::Healthy {
                    queued: queue_state.queued(),
                    capacity: queue_state.capacity(),
                };
            }
        }
        let queue_only_saturation = queue_state.is_saturated()
            && !active_pending
            && !rbc_backlog
            && !relay_backpressure
            && !consensus_queue_backpressure;
        let queue_only_starved = ingress_starvation_override;
        if queue_only_saturation && queue_only_starved {
            queue_state = BackpressureState::Healthy {
                queued: queue_state.queued(),
                capacity: queue_state.capacity(),
            };
        }
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
        routing_plans: Vec<crate::queue::RoutingPlan>,
        tx_sizes: Vec<usize>,
        height: u64,
        view: u64,
    ) -> (
        Vec<crate::queue::TransactionGuard>,
        Vec<AcceptedTransaction<'static>>,
        Vec<RoutingDecision>,
        Vec<crate::queue::RoutingPlan>,
        Vec<usize>,
        usize,
    ) {
        let mut retained_guards = Vec::with_capacity(tx_guards.len());
        let mut retained_transactions = Vec::with_capacity(transactions.len());
        let mut retained_routing = Vec::with_capacity(routing_decisions.len());
        let mut retained_routing_plans = Vec::with_capacity(routing_plans.len());
        let mut retained_sizes = Vec::with_capacity(tx_sizes.len());
        let mut dropped = 0usize;

        let mut guard_iter = tx_guards.into_iter();
        for (((tx, routing), routing_plan), size) in transactions
            .into_iter()
            .zip(routing_decisions.into_iter())
            .zip(routing_plans.into_iter())
            .zip(tx_sizes.into_iter())
        {
            let guard = guard_iter.next();
            if state.has_committed_transaction(tx.hash()) {
                dropped = dropped.saturating_add(1);
                continue;
            }
            if let Some(guard) = guard {
                retained_guards.push(guard);
            }
            retained_transactions.push(tx);
            retained_routing.push(routing);
            retained_routing_plans.push(routing_plan);
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
            retained_routing_plans,
            retained_sizes,
            dropped,
        )
    }

    pub(super) fn maybe_rebroadcast_cached_proposal(
        &mut self,
        height: u64,
        view: u64,
        pending_queue_len: usize,
        now: Instant,
    ) -> Option<HashOf<BlockHeader>> {
        let cached_proposal = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view)
            .cloned();
        let cached_hint = self
            .subsystems
            .propose
            .proposal_cache
            .get_hint(height, view)
            .copied();
        let owner_hint = cached_hint
            .map(|hint| hint.block_hash)
            .or_else(|| self.authoritative_slot_owner_hash(height, view))
            .or_else(|| {
                self.frontier_slot
                    .as_ref()
                    .filter(|slot| slot.height == height && slot.view == view)
                    .map(|slot| slot.block_hash)
            });
        if cached_proposal.is_none() && owner_hint.is_none() {
            return None;
        }

        let (pending_block, block_hash, pending_payload_bytes, pending_payload_hash) = {
            let Some(pending) = self.pending.pending_blocks.values().find(|pending| {
                !pending.aborted
                    && pending.height == height
                    && pending.view == view
                    && cached_proposal
                        .as_ref()
                        .is_none_or(|proposal| pending.payload_hash == proposal.payload_hash)
                    && owner_hint.is_none_or(|hint| pending.block.hash() == hint)
            }) else {
                trace!(
                    height,
                    view,
                    payload = ?cached_proposal.as_ref().map(|proposal| proposal.payload_hash),
                    block = ?owner_hint,
                    "skipping cached proposal rebroadcast: pending block not found"
                );
                return None;
            };
            (
                pending.block.clone(),
                pending.block.hash(),
                pending.payload_bytes().to_vec(),
                pending.payload_hash,
            )
        };

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let proposal_roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
        if proposal_roster.is_empty() {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast: empty commit topology"
            );
            return None;
        }
        let mut topology = super::network_topology::Topology::new(proposal_roster.clone());
        let leader_index = match self.leader_index_for(&mut topology, height, view) {
            Ok(idx) => idx,
            Err(err) => {
                warn!(
                    ?err,
                    height, view, "failed to compute leader index for cached proposal rebroadcast"
                );
                return None;
            }
        };

        let Some(local_pos) = topology.position(self.common_config.peer.id().public_key()) else {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast: local peer not in validator set"
            );
            return None;
        };
        let locally_authoritative_frontier = self
            .locally_authoritative_frontier_info_for_block(&pending_block)
            .filter(|frontier| {
                frontier.payload_hash == pending_payload_hash
                    && usize::try_from(frontier.proposer) == Ok(leader_index)
                    && frontier.epoch == self.epoch_for_height(height)
            });
        let proposal = if let Some(proposal) = cached_proposal {
            if usize::try_from(proposal.header.proposer) != Ok(leader_index) {
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    proposer = proposal.header.proposer,
                    leader_index,
                    "skipping cached proposal rebroadcast: cached proposer is not the selected leader"
                );
                return None;
            }
            proposal
        } else if let Some(hint) = cached_hint.filter(|hint| hint.block_hash == block_hash) {
            let proposer = match u32::try_from(leader_index) {
                Ok(proposer) => proposer,
                Err(_) => {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        leader_index,
                        "skipping cached proposal rebroadcast: leader index exceeds proposal field"
                    );
                    return None;
                }
            };
            let proposal = Self::build_consensus_proposal(
                &pending_block,
                pending_payload_hash,
                hint.highest_qc,
                proposer,
                view,
                self.epoch_for_height(height),
            );
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal.clone());
            proposal
        } else if let Some(frontier) = locally_authoritative_frontier.clone() {
            let proposal = Self::build_consensus_proposal(
                &pending_block,
                pending_payload_hash,
                frontier.highest_qc,
                frontier.proposer,
                view,
                frontier.epoch,
            );
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal.clone());
            proposal
        } else {
            trace!(
                height,
                view,
                block = %block_hash,
                "skipping cached proposal rebroadcast: no proposal metadata retained"
            );
            return None;
        };

        let frontier_recovery_cached = self.config.resilience.enabled
            && height == self.committed_height_snapshot().saturating_add(1)
            && view > 0
            && pending_queue_len > 0;
        let payload_cooldown = self.payload_rebroadcast_cooldown();
        let cooldown = if frontier_recovery_cached {
            payload_cooldown.min(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
        } else {
            payload_cooldown
        };
        if !frontier_recovery_cached && self.relay_backpressure_active(now, cooldown) {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast due to relay backpressure"
            );
            return None;
        }
        if !self
            .proposal_rebroadcast_log
            .allow(block_hash, now, cooldown)
        {
            trace!(
                height,
                view,
                block = %block_hash,
                local_idx = local_pos,
                leader_index,
                cooldown_ms = cooldown.as_millis(),
                "skipping cached proposal rebroadcast due to cooldown"
            );
            return None;
        }

        let local_peer_id = self.common_config.peer.id().clone();
        let Some(block_created) = self.frontier_block_created_for_local_proposal_wire_with_payload(
            &pending_block,
            &proposal,
            &proposal_roster,
            &pending_payload_bytes,
            pending_payload_hash,
        ) else {
            warn!(
                height,
                view,
                block = %block_hash,
                "skipping cached proposal rebroadcast because frontier metadata could not be rebuilt"
            );
            return None;
        };
        let block_msg = Arc::new(BlockMessage::BlockCreated(block_created));
        let block_encoded = Arc::new(BlockMessageWire::encode_message(block_msg.as_ref()));
        let proposal_hint = super::message::ProposalHint {
            block_hash,
            height,
            view,
            highest_qc: proposal.header.highest_qc,
        };
        let mut scheduled = 0usize;
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
            scheduled = scheduled.saturating_add(1);
        }
        self.post_proposal_metadata_to_topology(
            topology.as_ref(),
            &local_peer_id,
            proposal_hint,
            proposal,
        );
        if scheduled == 0 {
            trace!(
                height,
                view,
                block = %block_hash,
                "skipping cached proposal rebroadcast: no remote validators"
            );
            return None;
        }
        if pending_queue_len > 0 {
            iroha_logger::info!(
                height,
                view,
                block = %block_hash,
                frontier_recovery_cached,
                local_idx = local_pos,
                leader_index,
                cooldown_ms = cooldown.as_millis(),
                "rebroadcasting cached proposal"
            );
        } else {
            debug!(
                height,
                view,
                block = %block_hash,
                frontier_recovery_cached,
                local_idx = local_pos,
                leader_index,
                cooldown_ms = cooldown.as_millis(),
                "rebroadcasting cached proposal"
            );
        }
        Some(block_hash)
    }

    fn nudge_frontier_recovery_proposal_retry(&mut self, now: Instant) {
        if let Some(next_deadline) = now.checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
            && self.subsystems.propose.pacemaker.next_deadline > next_deadline
        {
            self.subsystems.propose.pacemaker.next_deadline = next_deadline;
        }
    }

    pub(super) fn maybe_progress_existing_slot_proposal(
        &mut self,
        height: u64,
        view: u64,
        pending_queue_len: usize,
        now: Instant,
        trigger: &'static str,
    ) -> bool {
        let mut progressed = self
            .maybe_rebroadcast_cached_proposal(height, view, pending_queue_len, now)
            .is_some();

        let owner_hint = self
            .authoritative_slot_owner_hash(height, view)
            .or_else(|| {
                self.frontier_slot
                    .as_ref()
                    .filter(|slot| slot.height == height && slot.view == view)
                    .map(|slot| slot.block_hash)
            });
        if let Some(block_hash) = owner_hint
            && !self.frontier_block_materialized_locally(block_hash)
        {
            progressed |= self.request_frontier_owner_body_repair(block_hash, height, view, now);
        }

        let pending_hashes: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .filter_map(|(block_hash, pending)| {
                (!pending.aborted
                    && !pending.is_retired_same_height()
                    && pending.validation_status == ValidationStatus::Valid
                    && pending.height == height
                    && pending.view == view)
                    .then_some(*block_hash)
            })
            .collect();
        if pending_hashes.is_empty() {
            if progressed {
                self.nudge_frontier_recovery_proposal_retry(now);
            }
            return progressed;
        }

        let commit_topology = self.effective_commit_topology();
        for block_hash in pending_hashes {
            if self.maybe_emit_local_commit_vote_for_pending_event(
                block_hash,
                height,
                view,
                &commit_topology,
                trigger,
            ) {
                progressed = true;
                continue;
            }

            if self.maybe_replay_known_block_commit_evidence(
                block_hash,
                height,
                view,
                &commit_topology,
                trigger,
            ) {
                self.request_commit_pipeline_for_pending(
                    block_hash,
                    super::status::RoundEventCauseTrace::VoteReceived,
                    None,
                );
                progressed = true;
            }
        }

        if progressed {
            self.nudge_frontier_recovery_proposal_retry(now);
        }
        progressed
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

    pub(super) fn maybe_rebroadcast_new_view_votes(&mut self, height: u64, now: Instant) {
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
        let frontier_recovery_new_view = self.config.resilience.enabled
            && target_height == self.committed_height_snapshot().saturating_add(1)
            && target_view > 0;
        let cooldown = if frontier_recovery_new_view {
            self.rebroadcast_cooldown()
                .min(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
        } else {
            self.rebroadcast_cooldown()
        };
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
        let targets = self.new_view_rebroadcast_targets(block_hash, target_height, target_view);
        let rebroadcasted = self.rebroadcast_block_votes_to_targets_with_backpressure(
            crate::sumeragi::consensus::Phase::NewView,
            block_hash,
            target_height,
            target_view,
            &targets,
            true,
            "new_view_convergence_rebroadcast",
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

    pub(super) fn precommit_vote_blocks_proposal_assembly(
        &self,
        vote: &crate::sumeragi::consensus::Vote,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> bool {
        vote.phase == crate::sumeragi::consensus::Phase::Commit
            && vote.height == height
            && vote.view == view
            && vote.epoch == epoch
            && self.vote_payload_actionable_for_proposal(vote.block_hash, height, view)
    }

    fn slot_has_actionable_vote_backed_proposal_evidence(
        &self,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> bool {
        self.stored_votes().any(|vote| {
            matches!(
                vote.phase,
                crate::sumeragi::consensus::Phase::Prepare
                    | crate::sumeragi::consensus::Phase::Commit
            ) && vote.height == height
                && vote.view == view
                && vote.epoch == epoch
                && self.vote_payload_actionable_for_proposal(vote.block_hash, height, view)
        }) || self.qc_cache.values().any(|qc| {
            matches!(
                qc.phase,
                crate::sumeragi::consensus::Phase::Prepare
                    | crate::sumeragi::consensus::Phase::Commit
            ) && qc.height == height
                && qc.view == view
                && qc.epoch == epoch
                && self.vote_payload_actionable_for_proposal(qc.subject_block_hash, height, view)
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_pacemaker_propose_ready(&mut self, now: Instant) -> bool {
        self.on_pacemaker_propose_ready_with_dependency_override(now, false)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_pacemaker_propose_ready_with_dependency_override(
        &mut self,
        now: Instant,
        allow_dependency_gated_reproposal: bool,
    ) -> bool {
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
        let queue_depths = super::status::worker_queue_depth_snapshot();
        let ingress_grace = self.frontier_ingress_drain_grace(self.runtime_da_enabled());
        let frontier_recovery_ingress_override = self.frontier_recovery_ingress_override_active(
            tracked_height,
            tracked_view,
            now,
            ingress_grace,
        );
        let frontier_proposal_ingress_deferring = self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && tracked_view > 0
            && !frontier_recovery_ingress_override
            && self.frontier_proposal_ingress_defer_active(
                tracked_height,
                tracked_view,
                now,
                queue_depths,
                ingress_grace,
            );
        let active_cached_frontier_slot = self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && self
                .subsystems
                .propose
                .proposal_cache
                .get_proposal(tracked_height, tracked_view)
                .is_some();
        if self.proposal_gated_by_missing_dependencies(tracked_height)
            && !allow_dependency_gated_reproposal
            && !active_cached_frontier_slot
        {
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
        let frontier_partial_new_view_support_detected = self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && tracked_view > 0
            && (self
                .subsystems
                .propose
                .new_view_tracker
                .entries
                .get(&(tracked_height, tracked_view))
                .is_some_and(|entry| {
                    let roster_set: BTreeSet<_> = topology.as_ref().iter().cloned().collect();
                    let support = entry.count_in_roster(&roster_set, local_peer.as_ref());
                    support > 0 && support < required
                })
                || self.frontier_partial_new_view_vote_support(
                    tracked_height,
                    tracked_view,
                    required,
                    &topology,
                ));
        let frontier_partial_new_view_support = frontier_partial_new_view_support_detected
            && self.frontier_partial_new_view_support_still_converging(tracked_height, now);
        if frontier_partial_new_view_support_detected && !frontier_partial_new_view_support {
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                convergence_grace_ms = self
                    .frontier_partial_new_view_convergence_grace()
                    .as_millis(),
                "partial NEW_VIEW support did not converge before the grace window; allowing committed-QC frontier fallback"
            );
        }

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
        let missing_qc_frontier_self_proposal_qc = self
            .missing_qc_liveness_allows_frontier_self_proposal(
                tracked_height,
                tracked_view,
                committed_height,
                pending_queue_len,
                precommit_qc,
            );
        let missing_qc_frontier_self_proposal_ready =
            missing_qc_frontier_self_proposal_qc.is_some();
        let missing_qc_frontier_self_proposal_blocked_by_ingress =
            missing_qc_frontier_self_proposal_ready && frontier_proposal_ingress_deferring;
        let frontier_partial_new_view_support_blocks_missing_qc_fallback =
            missing_qc_frontier_self_proposal_qc.is_some_and(|qc| {
                self.frontier_partial_new_view_support_blocks_qc_fallback(
                    tracked_height,
                    tracked_view,
                    required,
                    &topology,
                    local_peer.as_ref(),
                    qc,
                    frontier_partial_new_view_support,
                )
            });
        let frontier_partial_new_view_support_blocks_precommit_qc_fallback = precommit_qc
            .is_some_and(|qc| {
                self.frontier_partial_new_view_support_blocks_qc_fallback(
                    tracked_height,
                    tracked_view,
                    required,
                    &topology,
                    local_peer.as_ref(),
                    qc,
                    frontier_partial_new_view_support,
                )
            });
        if frontier_partial_new_view_support
            && !frontier_partial_new_view_support_blocks_precommit_qc_fallback
        {
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                "partial NEW_VIEW support is compatible with the committed-QC frontier fallback"
            );
        }
        let missing_qc_committed_frontier_fallback_allowed = missing_qc_frontier_self_proposal_qc
            .is_some_and(|qc| {
                self.config.resilience.enabled
                    && tracked_view > 0
                    && pending_queue_len > 0
                    && tracked_height == committed_height.saturating_add(1)
                    && qc.phase == crate::sumeragi::consensus::Phase::Commit
                    && qc.height == committed_height
                    && !frontier_partial_new_view_support_blocks_missing_qc_fallback
                    && !missing_qc_frontier_self_proposal_blocked_by_ingress
                    && (active_cached_frontier_slot
                        || !self.proposal_gated_by_missing_dependencies(tracked_height))
            });
        let new_view_quorum_free_frontier_proposal_allowed =
            tracked_view == 0 || required <= 1 || missing_qc_committed_frontier_fallback_allowed;
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
            let roster_set: BTreeSet<_> = topology.as_ref().iter().cloned().collect();
            let current_view_idx = current_view.unwrap_or_default();
            let new_view_quorum_at_tracked_height =
                self.subsystems.propose.new_view_tracker.entries.iter().any(
                    |((entry_height, _), entry)| {
                        *entry_height == tracked_height
                            && entry.count_in_roster(&roster_set, local_peer.as_ref()) >= required
                    },
                );
            let exact_new_view_quorum_ready = new_view_quorum_at_tracked_height
                && (pending_queue_len == 0
                    || view_age.is_some_and(|age| age >= self.commit_quorum_timeout()));
            let future_new_view_quorum_observed = tracked_height < desired_height
                && self.subsystems.propose.new_view_tracker.entries.iter().any(
                    |((entry_height, _), entry)| {
                        *entry_height >= tracked_height.saturating_add(1)
                            && entry.count_in_roster(&roster_set, local_peer.as_ref()) >= required
                    },
                );
            let cached_current_slot = self
                .subsystems
                .propose
                .proposal_cache
                .get_proposal(tracked_height, current_view_idx)
                .is_some();
            let missing_qc_recovery_active =
                self.subsystems
                    .propose
                    .proposal_liveness
                    .is_some_and(|slot| {
                        slot.height == tracked_height
                            && slot.view == current_view_idx
                            && matches!(
                                slot.state,
                                ProposalLivenessState::AwaitingProposalAfterMissingQc
                                    | ProposalLivenessState::RecoveryAcquireDependencies
                            )
                    });
            let forced_recovery_view = self
                .subsystems
                .propose
                .forced_view_after_timeout
                .is_some_and(|(forced_height, forced_view)| {
                    forced_height == tracked_height && forced_view >= current_view_idx
                });
            let committed_qc_frontier_recovery_candidate = self.config.resilience.enabled
                && tracked_height == committed_height.saturating_add(1)
                && current_view_idx > 0
                && self
                    .subsystems
                    .propose
                    .new_view_tracker
                    .entries
                    .get(&(tracked_height, current_view_idx))
                    .is_none_or(|entry| entry.senders.is_empty())
                && !self.proposal_gated_by_missing_dependencies(tracked_height)
                && precommit_qc.is_some_and(|qc| tracked_height == qc.height.saturating_add(1));
            let empty_recovery_view = pending_queue_len == 0 && current_view_idx > 0;
            let recovery_or_quorum_evidence = empty_recovery_view
                || exact_new_view_quorum_ready
                || future_new_view_quorum_observed
                || cached_current_slot
                || missing_qc_recovery_active
                || committed_qc_frontier_recovery_candidate
                || forced_recovery_view;
            if required > 1
                && tracked_height == committed_height.saturating_add(1)
                && self.subsystems.propose.last_successful_proposal.is_none()
                && !recovery_or_quorum_evidence
            {
                self.subsystems.propose.pacemaker.next_deadline = now
                    .checked_add(
                        self.subsystems
                            .propose
                            .pacemaker
                            .propose_interval
                            .max(Duration::from_millis(1)),
                    )
                    .unwrap_or(now);
                self.maybe_rebroadcast_new_view_votes(tracked_height, now);
                return false;
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

        let mut candidate = self
            .subsystems
            .propose
            .new_view_tracker
            .select_with_quorum_for_height(
                tracked_height,
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

        if candidate.is_none() && frontier_proposal_ingress_deferring {
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            self.subsystems.propose.pacemaker.next_deadline = now
                .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                .unwrap_or(now);
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                queue_len = pending_queue_len,
                vote_rx_depth = queue_depths.vote_rx,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                block_rx_depth = queue_depths.block_rx,
                "deferring committed-QC frontier fallback while proposal ingress drains"
            );
            return false;
        }

        if candidate.is_none()
            && !frontier_partial_new_view_support_blocks_missing_qc_fallback
            && let Some(qc) = self.missing_qc_liveness_allows_frontier_self_proposal(
                tracked_height,
                tracked_view,
                committed_height,
                pending_queue_len,
                precommit_qc,
            )
        {
            candidate = Some(NewViewSelection {
                key: (tracked_height, tracked_view),
                quorum: required,
                highest_qc: qc,
            });
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                qc_height = qc.height,
                qc_view = qc.view,
                "using committed-QC frontier candidate after missing-QC no-proposal liveness timeout"
            );
        }
        if candidate.is_none()
            && self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && tracked_view > 0
            && !frontier_partial_new_view_support_blocks_precommit_qc_fallback
            && self
                .subsystems
                .propose
                .new_view_tracker
                .entries
                .get(&(tracked_height, tracked_view))
                .is_none_or(|entry| entry.senders.is_empty())
            && !self.proposal_gated_by_missing_dependencies(tracked_height)
            && let Some(qc) = precommit_qc
            && tracked_height == qc.height.saturating_add(1)
        {
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            candidate = Some(NewViewSelection {
                key: (tracked_height, tracked_view),
                quorum: required,
                highest_qc: qc,
            });
            warn!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                qc_height = qc.height,
                qc_view = qc.view,
                queue_len = pending_queue_len,
                "using committed-QC frontier candidate without NEW_VIEW quorum under resilience liveness pressure"
            );
        }

        if candidate.is_none()
            && tracked_height < desired_height
            && let Some(future_selection) = self
                .subsystems
                .propose
                .new_view_tracker
                .select_with_quorum_at_or_above_height(
                    tracked_height.saturating_add(1),
                    required,
                    local_peer.as_ref(),
                    topology.as_ref(),
                )
        {
            let highest_qc_observed =
                self.observe_new_view_highest_qc_exact_repair(future_selection.highest_qc);
            let reanchor_requested = self.request_range_pull_from_anchor(
                tracked_height,
                FUTURE_NEW_VIEW_FRONTIER_REANCHOR_REASON,
                now,
            );
            info!(
                height = tracked_height,
                desired_height,
                future_height = future_selection.key.0,
                future_view = future_selection.key.1,
                future_quorum = future_selection.quorum,
                future_highest_qc_height = future_selection.highest_qc.height,
                future_highest_qc_view = future_selection.highest_qc.view,
                highest_qc_observed,
                reanchor_requested,
                "deferring proposal: future NEW_VIEW quorum observed, reanchoring local frontier"
            );
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            return false;
        }

        let Some(selection) = candidate.or_else(|| {
            // Fallback: bootstrap the first view using the latest committed QC when no NEW_VIEW
            // quorum has been observed yet. This prevents the pacemaker from stalling indefinitely
            // at startup before any view changes occur.
            if !new_view_quorum_free_frontier_proposal_allowed {
                return None;
            }
            let _local_idx = local_idx?;
            let qc = missing_qc_frontier_self_proposal_qc.or_else(|| self.latest_committed_qc())?;
            Some(NewViewSelection {
                key: (
                    qc.height.saturating_add(1),
                    if bootstrap_view { 0 } else { tracked_view },
                ),
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
        let _ = self.promote_committed_qc_for_frontier_selection(
            height,
            &mut highest_qc,
            "proposal_selection",
        );

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
                self.precommit_vote_blocks_proposal_assembly(vote, height, view_idx, epoch)
            })
            .count();

        // Avoid rebuilding multiple proposals for the same (height, view) slot. Reassembly in the
        // same view causes double-voting and scatters QC collection; wait for the existing
        // proposal to gather votes or transition via a view change instead.
        if let Some(cached_payload_hash) = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view_idx)
            .map(|proposal| proposal.payload_hash)
        {
            let cached_owner_hint = self
                .subsystems
                .propose
                .proposal_cache
                .get_hint(height, view_idx)
                .map(|hint| hint.block_hash)
                .or_else(|| self.authoritative_slot_owner_hash(height, view_idx))
                .or_else(|| {
                    self.frontier_slot
                        .as_ref()
                        .filter(|slot| slot.height == height && slot.view == view_idx)
                        .map(|slot| slot.block_hash)
                });
            // Rebroadcast cached proposals when the leader is still responsible for the slot so
            // peers that missed the initial messages can recover without forcing a view change.
            let _ =
                self.maybe_rebroadcast_cached_proposal(height, view_idx, pending_queue_len, now);
            if precommit_votes_at_view > 0 {
                debug!(
                    height,
                    view = view_idx,
                    precommit_votes = precommit_votes_at_view,
                    queue_len = pending_queue_len,
                    "proposal already cached for this slot; precommit votes observed, continuing cached-slot liveness checks"
                );
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
            let mut live_same_slot_pending = 0usize;
            let cached_wait_age = self
                .pending
                .pending_blocks
                .values()
                .filter(|pending| {
                    if pending.aborted || pending.height != height || pending.view != view_idx {
                        return false;
                    }
                    live_same_slot_pending = live_same_slot_pending.saturating_add(1);
                    pending.payload_hash == cached_payload_hash
                        && cached_owner_hint.is_none_or(|hint| pending.block.hash() == hint)
                })
                .map(|pending| {
                    pending
                        .progress_age(now)
                        .max(now.saturating_duration_since(pending.inserted_at))
                })
                .max();
            let cached_pending_present = cached_wait_age.is_some();
            if !cached_pending_present && live_same_slot_pending > 0 {
                let dropped_proposal = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(height, view_idx)
                    .is_some();
                let dropped_hint = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(height, view_idx)
                    .is_some();
                let progressed = self.maybe_progress_existing_slot_proposal(
                    height,
                    view_idx,
                    pending_queue_len,
                    now,
                    "cached_proposal_metadata_mismatch",
                );
                warn!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    live_same_slot_pending,
                    cached_payload = ?cached_payload_hash,
                    cached_block = ?cached_owner_hint,
                    dropped_proposal,
                    dropped_hint,
                    progressed_existing_slot = progressed,
                    "cached proposal metadata does not match live pending body; dropping stale metadata"
                );
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "cached_proposal_metadata_mismatch",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
            if !cached_pending_present
                && self.config.resilience.enabled
                && height == self.committed_height_snapshot().saturating_add(1)
                && view_idx > 0
            {
                let cached_hint = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .get_hint(height, view_idx)
                    .cloned();
                let repair_window = self
                    .frontier_slot_lag_window()
                    .max(self.recovery_deferred_qc_ttl())
                    .max(quorum_timeout)
                    .max(self.rebroadcast_cooldown())
                    .max(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL);
                let cache_age = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .observed_at(height, view_idx)
                    .map(|observed_at| now.saturating_duration_since(observed_at));
                if let Some(hint) = cached_hint.as_ref() {
                    let validation_inflight = self
                        .subsystems
                        .validation
                        .inflight
                        .contains_key(&hint.block_hash);
                    let commit_inflight =
                        self.subsystems
                            .commit
                            .inflight
                            .as_ref()
                            .is_some_and(|inflight| {
                                inflight.block_hash == hint.block_hash
                                    && !inflight.pending.aborted
                                    && inflight.pending.height == height
                                    && inflight.pending.view == view_idx
                            });
                    let pending_processing = self
                        .pending
                        .pending_processing
                        .get()
                        .is_some_and(|processing| processing == hint.block_hash);
                    let deferred_body = self.deferred_block_sync_updates.keys().any(
                        |(deferred_height, deferred_view, deferred_hash)| {
                            *deferred_height == height
                                && *deferred_view == view_idx
                                && *deferred_hash == hint.block_hash
                        },
                    );
                    if validation_inflight || commit_inflight || pending_processing || deferred_body
                    {
                        self.subsystems.propose.pacemaker.next_deadline = now
                            .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                            .unwrap_or(now);
                        debug!(
                            height,
                            view = view_idx,
                            block = %hint.block_hash,
                            validation_inflight,
                            commit_inflight,
                            pending_processing,
                            deferred_body,
                            queue_len = pending_queue_len,
                            "cached proposal has no live pending body but local processing still owns it; deferring rotation"
                        );
                        self.maybe_rebroadcast_new_view_votes(height, now);
                        self.warn_resilience_frontier_proposal_deferred(
                            height,
                            view_idx,
                            "cached_proposal_local_processing",
                            highest_qc,
                            pending_queue_len,
                            now,
                        );
                        return false;
                    }
                }
                let repair_age = cached_hint.as_ref().and_then(|hint| {
                    self.frontier_slot.as_ref().and_then(|slot| {
                        (slot.height == height
                            && slot.view == view_idx
                            && slot.block_hash == hint.block_hash
                            && slot.exact_fetch_armed
                            && !slot.body_present)
                            .then(|| now.saturating_duration_since(slot.lag_started_at()))
                    })
                });
                let repair_exhausted = repair_age
                    .is_some_and(|age| repair_window != Duration::ZERO && age >= repair_window);
                if let Some(hint) = cached_hint.as_ref().filter(|_| !repair_exhausted) {
                    let seeded = self.handle_frontier_body_gap_with_topology(
                        hint.block_hash,
                        height,
                        view_idx,
                        &BTreeSet::new(),
                        &commit_topology,
                        true,
                        now,
                    );
                    let fetch_requested = self.emit_frontier_block_body_fetch_urgent(now);
                    let repair_active = repair_age.is_some()
                        || self.frontier_slot.as_ref().is_some_and(|slot| {
                            slot.height == height
                                && slot.view == view_idx
                                && slot.block_hash == hint.block_hash
                                && slot.exact_fetch_armed
                                && !slot.body_present
                        });
                    if repair_active {
                        debug!(
                            height,
                            view = view_idx,
                            block = %hint.block_hash,
                            queue_len = pending_queue_len,
                            repair_window_ms = repair_window.as_millis(),
                            repair_age_ms = repair_age.map(|age| age.as_millis()),
                            seeded_frontier_body_repair = seeded,
                            fetch_requested,
                            "cached proposal has no live pending body; requesting exact body repair before rotating"
                        );
                        self.maybe_rebroadcast_new_view_votes(height, now);
                        self.warn_resilience_frontier_proposal_deferred(
                            height,
                            view_idx,
                            "cached_proposal_body_repair",
                            highest_qc,
                            pending_queue_len,
                            now,
                        );
                        return false;
                    }
                    debug!(
                        height,
                        view = view_idx,
                        block = %hint.block_hash,
                        queue_len = pending_queue_len,
                        repair_window_ms = repair_window.as_millis(),
                        repair_age_ms = repair_age.map(|age| age.as_millis()),
                        seeded_frontier_body_repair = seeded,
                        fetch_requested,
                        "cached proposal has no live pending body and no active exact repair; rotating recovery view"
                    );
                }
                if (cached_hint.is_some() || pending_queue_len == 0)
                    && !repair_exhausted
                    && cache_age.is_some_and(|age| age < repair_window)
                {
                    self.subsystems.propose.pacemaker.next_deadline = now
                        .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                        .unwrap_or(now);
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        repair_window_ms = repair_window.as_millis(),
                        cache_age_ms = cache_age.map(|age| age.as_millis()),
                        "cached proposal has no live pending body but was just observed; deferring rotation"
                    );
                    self.maybe_rebroadcast_new_view_votes(height, now);
                    self.warn_resilience_frontier_proposal_deferred(
                        height,
                        view_idx,
                        "cached_proposal_body_materializing",
                        highest_qc,
                        pending_queue_len,
                        now,
                    );
                    return false;
                }
                let dropped_proposal = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(height, view_idx)
                    .is_some();
                let dropped_hint = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(height, view_idx)
                    .is_some();
                warn!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    dropped_proposal,
                    dropped_hint,
                    repair_window_ms = repair_window.as_millis(),
                    repair_age_ms = repair_age.map(|age| age.as_millis()),
                    cache_age_ms = cache_age.map(|age| age.as_millis()),
                    "cached proposal has no live pending body; rotating recovery view"
                );
                self.apply_view_change_after_exhausted_frontier_recovery(
                    height,
                    view_idx,
                    ViewChangeCause::QuorumTimeout,
                );
                self.maybe_rebroadcast_new_view_votes(height, now);
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "cached_proposal_without_pending",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
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
                        let seeded = self
                            .seed_frontier_recovery_for_quorum_timeout_without_local_pending(
                                height, view_idx, now,
                            );
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
                        self.seed_frontier_recovery_for_quorum_timeout_without_local_pending(
                            height, view_idx, now,
                        );
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
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "cached_proposal_quorum_timeout",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
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
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "cached_proposal_waiting",
                highest_qc,
                pending_queue_len,
                now,
            );
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
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "precommit_votes_observed",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        if height == self.committed_height_snapshot().saturating_add(1) && view_idx > 0 {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            let ingress_grace = self.frontier_ingress_drain_grace(da_enabled);
            let selected_frontier_recovery_candidate = self.config.resilience.enabled
                && highest_qc.phase == crate::sumeragi::consensus::Phase::Commit
                && highest_qc.height.saturating_add(1) == height;
            let frontier_recovery_ingress_override = self
                .frontier_recovery_ingress_override_active(height, view_idx, now, ingress_grace)
                || (selected_frontier_recovery_candidate
                    && self.frontier_proposal_or_view_starved_past_ingress_grace(
                        height,
                        now,
                        ingress_grace,
                    ));
            if Self::frontier_consensus_ingress_queued(queue_depths) {
                if !frontier_recovery_ingress_override
                    && self.frontier_proposal_ingress_defer_active(
                        height,
                        view_idx,
                        now,
                        queue_depths,
                        ingress_grace,
                    )
                {
                    self.subsystems.propose.pacemaker.next_deadline = now
                        .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                        .unwrap_or(now);
                    debug!(
                        height,
                        view = view_idx,
                        vote_rx_depth = queue_depths.vote_rx,
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        queue_len = pending_queue_len,
                        "deferring fresh frontier proposal while proposal ingress drains"
                    );
                    self.warn_resilience_frontier_proposal_deferred(
                        height,
                        view_idx,
                        "proposal_ingress_draining",
                        highest_qc,
                        pending_queue_len,
                        now,
                    );
                    return false;
                }
                let view_age = self.phase_tracker.view_age(height, now).unwrap_or_default();
                let proposal_starved =
                    self.frontier_proposal_starved_past_ingress_grace(now, ingress_grace);
                if view_age < ingress_grace && !proposal_starved {
                    self.subsystems.propose.pacemaker.next_deadline = now
                        .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                        .unwrap_or(now);
                    debug!(
                        height,
                        view = view_idx,
                        view_age_ms = view_age.as_millis(),
                        ingress_grace_ms = ingress_grace.as_millis(),
                        vote_rx_depth = queue_depths.vote_rx,
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        queue_len = pending_queue_len,
                        "deferring fresh frontier proposal while consensus ingress drains"
                    );
                    self.warn_resilience_frontier_proposal_deferred(
                        height,
                        view_idx,
                        "consensus_ingress_draining",
                        highest_qc,
                        pending_queue_len,
                        now,
                    );
                    return false;
                }
                if view_age < ingress_grace {
                    debug!(
                        height,
                        view = view_idx,
                        view_age_ms = view_age.as_millis(),
                        ingress_grace_ms = ingress_grace.as_millis(),
                        vote_rx_depth = queue_depths.vote_rx,
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        queue_len = pending_queue_len,
                        "allowing fresh frontier proposal after proposal starvation despite queued consensus ingress"
                    );
                }
            }
        }

        if height == self.committed_height_snapshot().saturating_add(1)
            && (self.slot_has_proposal_evidence(height, view_idx)
                || self
                    .frontier_slot_live_local_owner_for_round(height, view_idx)
                    .is_some()
                || self.slot_has_actionable_vote_backed_proposal_evidence(height, view_idx, epoch))
        {
            let _ = self.seed_frontier_slot_from_same_height_evidence(
                height,
                view_idx,
                now,
                "missing_qc",
                true,
            );
        }

        if let Some(block_hash) = self.authoritative_slot_owner_hash(height, view_idx) {
            let progressed = self.maybe_progress_existing_slot_proposal(
                height,
                view_idx,
                pending_queue_len,
                now,
                "authoritative_slot_owner",
            );
            if !progressed {
                self.nudge_frontier_recovery_proposal_retry(now);
            }
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
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "authoritative_slot_owner",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        if self.slot_has_proposal_evidence(height, view_idx) {
            let progressed = self.maybe_progress_existing_slot_proposal(
                height,
                view_idx,
                pending_queue_len,
                now,
                "slot_has_proposal_evidence",
            );
            if !progressed {
                self.nudge_frontier_recovery_proposal_retry(now);
            }
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
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "slot_has_proposal_evidence",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        if let Some((owner_hash, owner_view)) = self
            .frontier_slot_live_local_owner_for_round(height, view_idx)
            .filter(|(_, owner_view)| *owner_view < view_idx)
        {
            if self.maybe_yield_stale_frontier_owner_for_fresh_proposal(
                height,
                view_idx,
                owner_hash,
                owner_view,
                now,
                pending_queue_len,
            ) {
                debug!(
                    height,
                    view = view_idx,
                    owner = %owner_hash,
                    owner_view,
                    queue_len = pending_queue_len,
                    "stale same-height frontier owner yielded; continuing fresh proposal assembly"
                );
            } else if pending_queue_len > 0 {
                let progressed = self.maybe_progress_existing_slot_proposal(
                    height,
                    owner_view,
                    pending_queue_len,
                    now,
                    "same_height_owner_live",
                );
                if !progressed {
                    self.nudge_frontier_recovery_proposal_retry(now);
                }
                debug!(
                    height,
                    view = view_idx,
                    owner = %owner_hash,
                    owner_view,
                    queue_len = pending_queue_len,
                    "same-height frontier owner is still locally live for this round; deferring reassembly"
                );
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "same_height_owner_live",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            } else {
                let progressed = self.maybe_progress_existing_slot_proposal(
                    height,
                    owner_view,
                    pending_queue_len,
                    now,
                    "same_height_owner_live",
                );
                if !progressed {
                    self.nudge_frontier_recovery_proposal_retry(now);
                }
                trace!(
                    height,
                    view = view_idx,
                    owner = %owner_hash,
                    owner_view,
                    "same-height frontier owner is still locally live for this round; deferring reassembly"
                );
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "same_height_owner_live",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
        }

        if height == self.committed_height_snapshot().saturating_add(1)
            && let Some(existing_vote) =
                self.local_same_height_vote(height, self.epoch_for_height(height))
        {
            let new_view_qc_supersedes = self.new_view_qc_supersedes_same_height_vote_conflict(
                height,
                view_idx,
                highest_qc,
                existing_vote.block_hash,
                existing_vote.view,
            );
            if new_view_qc_supersedes
                || !self.local_same_height_vote_blocks_fresh_proposal_assembly(
                    height,
                    view_idx,
                    &existing_vote,
                    now,
                    true,
                    highest_qc,
                )
            {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    voted_view = existing_vote.view,
                    voted_phase = ?existing_vote.phase,
                    voted_block = %existing_vote.block_hash,
                    new_view_qc_supersedes,
                    "allowing fresh proposal after stale prior-view local same-height vote"
                );
            } else {
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
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "local_same_height_vote_blocks",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
        }

        if let Some((pending_age, pending_view)) = self
            .pending
            .pending_blocks
            .values()
            .filter(|pending| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            })
            .map(|pending| {
                (
                    now.saturating_duration_since(pending.inserted_at),
                    pending.view,
                )
            })
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
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "pending_block_quorum_window",
                    highest_qc,
                    pending_queue_len,
                    now,
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
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "pending_block_same_slot",
                    highest_qc,
                    pending_queue_len,
                    now,
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
                    let Some(lock) = self.promote_locked_qc_to_highest_if_needed("proposal") else {
                        return false;
                    };
                    iroha_logger::info!(
                        locked_height = locked,
                        highest_height = highest,
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        lock_hash = %lock.subject_block_hash,
                        "replacing regressed highest QC with locked QC for proposal assembly"
                    );
                    highest_qc = lock;
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
                            self.observe_new_view_highest_qc_exact_repair(highest_qc);
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
            if self.maybe_rotate_missing_qc_nonleader_after_proposal_timeout(
                height,
                view_idx,
                pending_queue_len,
                now,
            ) {
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "local_not_leader_timeout_rotate",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
            let missing_qc_reacquire_requested = if self.config.resilience.enabled
                && self.frontier_missing_qc_liveness_active(height, view_idx)
                && height == self.committed_height_snapshot().saturating_add(1)
            {
                let requested = self.reacquire_missing_qc_dependencies(height, view_idx, now, true);
                self.maybe_rebroadcast_new_view_votes(height, now);
                self.nudge_frontier_recovery_proposal_retry(now);
                requested
            } else {
                false
            };
            if pending_queue_len > 0 {
                iroha_logger::debug!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    queue_len = pending_queue_len,
                    missing_qc_reacquire_requested,
                    "deferring proposal: local node is not leader for this round"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    missing_qc_reacquire_requested,
                    "local node is not leader for this round"
                );
            }
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                if missing_qc_reacquire_requested {
                    "local_not_leader_reacquire"
                } else {
                    "local_not_leader"
                },
                highest_qc,
                pending_queue_len,
                now,
            );
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
        let allow_recovery_heartbeat = view_idx > 0 && height == committed_height.saturating_add(1);
        if !has_queue_work && !has_internal_work && !allow_recovery_heartbeat {
            trace!(
                height,
                view = view_idx,
                "deferring proposal: no queued transactions or internal work"
            );
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "no_queue_or_internal_work",
                highest_qc,
                pending_queue_len,
                now,
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
        let assembled = match self.assemble_and_broadcast_proposal_with_recovery_heartbeat(
            height,
            view_idx,
            highest_qc,
            &mut topology,
            leader_index,
            local_idx_val,
            view_snapshot,
            now,
            allow_recovery_heartbeat,
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
        canonicalize_parallel_batch_by_key, canonicalize_proposal_batch,
        consensus_queue_backpressure, da_payload_budget, next_cached_slot_timeout_streak,
        trim_batch_for_size_cap,
    };
    use crate::queue::{BackpressureState, RoutingDecision};
    use crate::sumeragi::status;
    use crate::tx::AcceptedTransaction;
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        ChainId, Level,
        isi::Log,
        nexus::{DataSpaceId, LaneId},
        prelude::{AccountId, TransactionBuilder},
    };
    use std::borrow::Cow;
    use std::num::NonZeroUsize;
    use std::time::{Duration, Instant};

    fn accepted_log_transaction(message: &str) -> AcceptedTransaction<'static> {
        let chain: ChainId = "proposal-canonicalization".parse().expect("chain id");
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.clone().into_parts();
        let authority = AccountId::new(key_pair.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions([Log::new(Level::INFO, message.to_owned())])
            .sign(&private_key);

        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    #[test]
    fn da_payload_budget_caps_to_rbc_budget() {
        let budget = da_payload_budget(1, 8 * 1024, 1024, None);
        let rbc_budget =
            usize::try_from(super::super::RBC_MAX_TOTAL_CHUNKS).expect("fits in usize");
        assert_eq!(budget, rbc_budget.min(8 * 1024));
    }

    #[test]
    fn da_payload_budget_honors_block_payload_cap() {
        let cap = NonZeroUsize::new(4096).expect("non-zero");
        let budget = da_payload_budget(256 * 1024, 32 * 1024, 1024, Some(cap));
        assert_eq!(budget, 4096);
    }

    #[test]
    fn da_payload_budget_honors_pending_caps() {
        let budget = da_payload_budget(256 * 1024, 4 * 1024, 1, None);
        assert_eq!(budget, 4 * 1024);
    }

    #[test]
    fn da_payload_budget_is_not_limited_by_single_consensus_frame() {
        let budget = da_payload_budget(256 * 1024, 512 * 1024, 1024, None);
        assert_eq!(budget, 512 * 1024);
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
    fn canonicalize_parallel_batch_by_key_reorders_companions_stably() {
        let mut txs = vec![30, 10, 20, 10];
        let mut routes = vec!["c", "a", "b", "a2"];
        let mut sizes = vec![3, 1, 2, 4];

        canonicalize_parallel_batch_by_key(&mut txs, &mut routes, &mut sizes, |tx| *tx);

        assert_eq!(txs, vec![10, 10, 20, 30]);
        assert_eq!(routes, vec!["a", "a2", "b", "c"]);
        assert_eq!(sizes, vec![1, 4, 2, 3]);
    }

    #[test]
    fn canonicalize_proposal_batch_keeps_routing_aligned_with_transaction_order() {
        let txs = ["first", "second", "third", "fourth"]
            .into_iter()
            .map(accepted_log_transaction)
            .collect::<Vec<_>>();
        let mut entries = txs
            .into_iter()
            .enumerate()
            .map(|(idx, tx)| {
                let route = RoutingDecision::new(
                    LaneId::new(u32::try_from(idx + 1).expect("lane id")),
                    DataSpaceId::new(u64::try_from(idx + 10).expect("dataspace id")),
                );
                let size = 100 + idx;
                (tx.as_ref().hash_as_entrypoint(), tx, route, size)
            })
            .collect::<Vec<_>>();

        entries.sort_unstable_by(|left, right| right.0.cmp(&left.0));
        let mut expected = entries.clone();
        expected.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        let mut tx_batch = entries
            .iter()
            .map(|(_, tx, _, _)| tx.clone())
            .collect::<Vec<_>>();
        let mut routing_batch = entries
            .iter()
            .map(|(_, _, route, _)| *route)
            .collect::<Vec<_>>();
        let mut sizes = entries
            .iter()
            .map(|(_, _, _, size)| *size)
            .collect::<Vec<_>>();

        canonicalize_proposal_batch(&mut tx_batch, &mut routing_batch, &mut sizes);

        let actual_hashes = tx_batch
            .iter()
            .map(|tx| tx.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>();
        let expected_hashes = expected
            .iter()
            .map(|(hash, _, _, _)| *hash)
            .collect::<Vec<_>>();
        let expected_routes = expected
            .iter()
            .map(|(_, _, route, _)| *route)
            .collect::<Vec<_>>();
        let expected_sizes = expected
            .iter()
            .map(|(_, _, _, size)| *size)
            .collect::<Vec<_>>();

        assert_eq!(actual_hashes, expected_hashes);
        assert_eq!(routing_batch, expected_routes);
        assert_eq!(sizes, expected_sizes);
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
        assert!(backpressure.only_pacing_backpressure());
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
