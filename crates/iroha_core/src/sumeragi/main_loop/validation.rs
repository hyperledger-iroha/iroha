//! Pending-block validation gates used by the commit pipeline.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use iroha_logger::prelude::*;

use super::*;

#[derive(Debug)]
pub(super) struct ValidationWork {
    pub(super) id: u64,
    pub(super) hash: HashOf<BlockHeader>,
    pub(super) block: SignedBlock,
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) frontier_generation: Option<u64>,
    pub(super) topology: super::network_topology::Topology,
    pub(super) commit_topology: Vec<PeerId>,
}

#[derive(Debug)]
pub(super) struct ValidationResult {
    pub(super) id: u64,
    pub(super) hash: HashOf<BlockHeader>,
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) frontier_generation: Option<u64>,
    pub(super) commit_topology: Vec<PeerId>,
    pub(super) duration: Duration,
    pub(super) outcome: Result<Option<StateRoots>, BlockValidationError>,
}

#[derive(Debug)]
pub(super) struct ValidationWorkerHandle {
    pub(super) work_txs: Vec<mpsc::SyncSender<ValidationWork>>,
    pub(super) result_rx: mpsc::Receiver<ValidationResult>,
    pub(super) join_handles: Vec<std::thread::JoinHandle<()>>,
}

#[derive(Copy, Clone, Debug)]
enum ValidationDispatch {
    TryWorker,
    #[cfg(test)]
    Inline,
}

impl ValidationDispatch {
    fn try_worker(self) -> bool {
        #[cfg(test)]
        {
            matches!(self, Self::TryWorker)
        }
        #[cfg(not(test))]
        {
            let _ = self;
            true
        }
    }

    #[cfg(test)]
    fn inline(self) -> bool {
        matches!(self, Self::Inline)
    }
}

#[derive(Copy, Clone, Debug)]
pub(super) enum ValidationInflightInlineReason {
    WorkerDisconnected,
    StaleFrontier {
        frontier_generation: u64,
    },
    Stalled {
        elapsed: Duration,
        stall_timeout: Duration,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum VNextValidationRedriveReason {
    OrphanedQueued,
    OrphanedRunning,
    StalledRunning,
    Backpressured,
}

impl VNextValidationRedriveReason {
    fn label(self) -> &'static str {
        match self {
            Self::OrphanedQueued => "orphaned_queued",
            Self::OrphanedRunning => "orphaned_running",
            Self::StalledRunning => "stalled_running",
            Self::Backpressured => "backpressured",
        }
    }
}

pub(super) fn spawn_validation_workers(
    state: Arc<State>,
    chain_id: ChainId,
    genesis_account: AccountId,
    wake_tx: Option<mpsc::SyncSender<()>>,
    worker_threads: usize,
    work_queue_cap: usize,
    result_queue_cap: usize,
) -> ValidationWorkerHandle {
    const AUTO_WORKER_MIN: usize = 2;
    const AUTO_WORKER_MAX: usize = 8;

    let threads = if worker_threads == 0 {
        let detected = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        detected.clamp(AUTO_WORKER_MIN, AUTO_WORKER_MAX)
    } else {
        worker_threads.max(1)
    };
    let work_queue_cap = if work_queue_cap == 0 {
        threads.saturating_mul(4).max(4)
    } else {
        work_queue_cap
    };
    let result_queue_cap = if result_queue_cap == 0 {
        threads.saturating_mul(8).max(8)
    } else {
        result_queue_cap
    };
    let (result_tx, result_rx) = mpsc::sync_channel::<ValidationResult>(result_queue_cap);
    let mut work_txs = Vec::with_capacity(threads);
    let mut join_handles = Vec::with_capacity(threads);
    for idx in 0..threads {
        let (work_tx, work_rx) = mpsc::sync_channel::<ValidationWork>(work_queue_cap);
        work_txs.push(work_tx);
        let result_tx = result_tx.clone();
        let wake_tx = wake_tx.clone();
        let state = Arc::clone(&state);
        let chain_id = chain_id.clone();
        let genesis_account = genesis_account.clone();
        let name = format!("sumeragi-validate-{idx}");
        let join_handle = crate::sumeragi::sumeragi_thread_builder(name)
            .spawn(move || {
                while let Ok(work) = work_rx.recv() {
                    let ValidationWork {
                        id,
                        hash,
                        block,
                        height,
                        view,
                        frontier_generation,
                        mut topology,
                        commit_topology,
                    } = work;
                    let mut voting_block = None;
                    let validation_start = Instant::now();
                    let outcome = validate_block_for_voting(
                        block,
                        &mut topology,
                        &chain_id,
                        &genesis_account,
                        state.as_ref(),
                        &mut voting_block,
                    );
                    let duration = validation_start.elapsed();
                    if result_tx
                        .send(ValidationResult {
                            id,
                            hash,
                            height,
                            view,
                            frontier_generation,
                            commit_topology,
                            duration,
                            outcome,
                        })
                        .is_err()
                    {
                        break;
                    }
                    if let Some(wake) = wake_tx.as_ref() {
                        let _ = wake.try_send(());
                    }
                }
            })
            .expect("failed to spawn sumeragi validation worker thread");
        join_handles.push(join_handle);
    }

    ValidationWorkerHandle {
        work_txs,
        result_rx,
        join_handles,
    }
}

impl Actor {
    const SUPERSEDED_VALIDATION_RESULT_CAP: usize = 4_096;

    pub(super) fn fast_finality_inline_validation_tx_count(
        &self,
        hash: HashOf<BlockHeader>,
        pending: &PendingBlock,
        local_height: u64,
        validation_priority_reason: Option<&'static str>,
    ) -> Option<usize> {
        if !self.runtime_da_enabled() || validation_priority_reason.is_some() {
            return None;
        }
        if pending.height != local_height.saturating_add(1) {
            return None;
        }
        if !self.slot_has_proposal_evidence(pending.height, pending.view) {
            return None;
        }
        if self.subsystems.validation.inflight.contains_key(&hash) {
            return None;
        }
        if !Self::local_payload_matches_hash(&pending.block, &pending.payload_hash) {
            return None;
        }
        let tx_cap = self
            .config
            .worker
            .fast_finality_inline_validation_max_transactions;
        if tx_cap == 0 {
            return None;
        }
        let tx_count = pending.block.external_entrypoints_cloned().count();
        (tx_count <= tx_cap).then_some(tx_count)
    }

    pub(in crate::sumeragi::main_loop) fn maybe_corrupt_debug_witness_roots(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        roots: StateRoots,
    ) -> StateRoots {
        if !self.config.debug.rbc.corrupt_witness_ack {
            return roots;
        }

        let mut salt_input = Vec::new();
        salt_input.extend_from_slice(b"sumeragi/debug/corrupt-witness-ack/v1");
        salt_input.extend_from_slice(hash.as_ref().as_ref());
        salt_input.extend_from_slice(&height.to_be_bytes());
        salt_input.extend_from_slice(&view.to_be_bytes());
        salt_input.extend_from_slice(self.common_config.peer.id().to_string().as_bytes());
        let salt = Hash::new(&salt_input);

        let mut post_state_root = *roots.post_state_root.as_ref();
        for (byte, salt_byte) in post_state_root.iter_mut().zip(salt.as_ref()) {
            *byte ^= *salt_byte;
        }
        if post_state_root == *roots.post_state_root.as_ref() {
            post_state_root[0] ^= 1;
        }

        warn!(
            height,
            view,
            block = %hash,
            "debug RBC witness corruption changed local execution root"
        );
        StateRoots {
            parent_state_root: roots.parent_state_root,
            post_state_root: Hash::prehashed(post_state_root),
        }
    }

    fn replay_cached_precommit_qc_for_valid_block(
        &mut self,
        hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        _commit_topology: &[PeerId],
        source: &'static str,
    ) {
        if !self.block_known_for_lock(hash) {
            return;
        }
        let Some(qc) = qc_cache_for_subject(&self.qc_cache, hash)
            .filter(|qc| {
                qc.phase == crate::sumeragi::consensus::Phase::Commit && qc.height == height
            })
            .max_by_key(|qc| qc.view)
            .cloned()
        else {
            return;
        };
        if self.process_precommit_qc(&qc, true, false) {
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.set_commit_qc_summary(&qc);
            }
            debug!(
                height,
                view,
                block = %hash,
                qc_view = qc.view,
                source,
                "replayed cached precommit QC after validation"
            );
        }
        let commit_ready = self
            .pending
            .pending_blocks
            .get(&hash)
            .and_then(|pending| {
                let state_height = self.state.committed_height();
                let tip_hash = self.state.latest_block_hash_fast();
                let parent = pending.block.header().prev_block_hash();
                super::pending_extends_tip(pending.height, parent, state_height, tip_hash)
                    .then_some(())
            })
            .is_some();
        if commit_ready {
            if let Some(pending) = self.pending.pending_blocks.get_mut(&hash) {
                pending.note_commit_qc_observed(qc.epoch);
            }
            self.request_commit_pipeline_for_pending(
                hash,
                super::status::RoundEventCauseTrace::QcReceived,
                None,
            );
            debug!(
                height,
                view,
                block = %hash,
                qc_view = qc.view,
                source,
                "applied cached precommit QC after validation"
            );
        } else if let Some(pending) = self.pending.pending_blocks.get_mut(&hash) {
            pending.note_commit_qc_observed(qc.epoch);
            debug!(
                height,
                view,
                block = %hash,
                qc_view = qc.view,
                source,
                "retaining cached precommit QC after validation until block extends committed tip"
            );
        }
    }

    fn remember_superseded_validation_result(&mut self, hash: HashOf<BlockHeader>, id: u64) {
        if self.subsystems.validation.superseded_results.len()
            >= Self::SUPERSEDED_VALIDATION_RESULT_CAP
        {
            if let Some(oldest_hash) = self
                .subsystems
                .validation
                .superseded_results
                .keys()
                .next()
                .copied()
            {
                self.subsystems
                    .validation
                    .superseded_results
                    .remove(&oldest_hash);
            }
        }
        self.subsystems
            .validation
            .superseded_results
            .insert(hash, id);
    }

    pub(super) fn supersede_validation_inflight(
        &mut self,
        hash: HashOf<BlockHeader>,
    ) -> Option<super::ValidationInFlight> {
        let inflight = self.subsystems.validation.inflight.remove(&hash)?;
        self.subsystems.validation.vnext_inflight.remove(&hash);
        self.remember_superseded_validation_result(hash, inflight.id);
        Some(inflight)
    }

    pub(super) fn prune_validation_inflight_without_pending(&mut self) -> usize {
        let before = self.subsystems.validation.inflight.len();
        self.subsystems
            .validation
            .inflight
            .retain(|hash, _| self.pending.pending_blocks.contains_key(hash));
        self.subsystems
            .validation
            .superseded_results
            .retain(|hash, _| self.pending.pending_blocks.contains_key(hash));
        self.subsystems
            .validation
            .vnext_inflight
            .retain(|hash, _| self.pending.pending_blocks.contains_key(hash));
        before.saturating_sub(self.subsystems.validation.inflight.len())
    }

    pub(super) fn validation_inflight_elapsed(
        &self,
        hash: HashOf<BlockHeader>,
    ) -> Option<std::time::Duration> {
        self.subsystems
            .validation
            .inflight
            .get(&hash)
            .map(|inflight| Instant::now().saturating_duration_since(inflight.started_at))
    }

    #[cfg(test)]
    pub(super) fn validation_worker_stall_timeout(&self) -> Duration {
        self.validation_worker_stall_timeout_with_floor(Duration::ZERO)
    }

    pub(super) fn validation_worker_stall_timeout_for(
        &self,
        hash: HashOf<BlockHeader>,
    ) -> Duration {
        let tx_scaled_floor = self
            .runtime_da_enabled()
            .then(|| {
                self.pending
                    .pending_blocks
                    .get(&hash)
                    .map(|pending| pending.block.external_entrypoints_cloned().len())
            })
            .flatten()
            .map(|tx_count| {
                let tx_count = u64::try_from(tx_count).unwrap_or(u64::MAX);
                let floor_ms = u64::try_from(
                    self.config
                        .worker
                        .validation_stall_da_per_entrypoint_floor
                        .as_millis(),
                )
                .unwrap_or(u64::MAX);
                Duration::from_millis(tx_count.saturating_mul(floor_ms))
            })
            .unwrap_or(Duration::ZERO);

        self.validation_worker_stall_timeout_with_floor(tx_scaled_floor)
    }

    pub(super) fn validation_inflight_fresh_timeout_floor(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
        now: Instant,
    ) -> Option<Duration> {
        let inflight = self.subsystems.validation.inflight.get(&hash)?;
        let vnext_owned = self
            .subsystems
            .validation
            .vnext_inflight
            .contains_key(&hash);
        if !vnext_owned
            && (self.subsystems.validation.result_rx.is_none()
                || self.subsystems.validation.work_txs.is_empty())
        {
            return None;
        }
        if let Some(frontier_generation) = inflight.frontier_generation
            && !self.frontier_owner_generation_matches(height, hash, frontier_generation)
        {
            return None;
        }
        let stall_timeout = self.validation_worker_stall_timeout_for(hash);
        (now.saturating_duration_since(inflight.started_at) < stall_timeout)
            .then_some(stall_timeout)
    }

    fn validation_worker_stall_timeout_with_floor(&self, extra_floor: Duration) -> Duration {
        let inline_floor = super::saturating_mul_duration(
            self.commit_validation_inline_fallback_timeout(),
            self.config
                .worker
                .validation_stall_inline_fallback_multiplier,
        );
        let ema_floor = self
            .subsystems
            .validation
            .duration_ema
            .map(|ema| {
                super::saturating_mul_duration(
                    ema,
                    self.config.worker.validation_stall_ema_multiplier,
                )
            })
            .unwrap_or(Duration::ZERO);
        let cap = if self.runtime_da_enabled() {
            self.config.worker.validation_stall_da_cap
        } else {
            self.config.worker.validation_stall_non_da_cap
        };
        inline_floor.max(ema_floor).max(extra_floor).min(cap)
    }

    pub(super) fn validation_duration_ema(&self) -> Option<Duration> {
        self.subsystems.validation.duration_ema
    }

    pub(super) fn vnext_validation_owns_block(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        self.subsystems
            .validation
            .vnext_inflight
            .contains_key(&hash)
            || self
                .vnext_rounds
                .get(&(height, view))
                .and_then(|round| round.slot(hash))
                .is_some_and(|slot| {
                    matches!(
                        slot.validation,
                        super::vnext::ValidationState::Queued { .. }
                            | super::vnext::ValidationState::Running { .. }
                            | super::vnext::ValidationState::Backpressured { .. }
                    )
                })
    }

    pub(super) fn validation_inflight_inline_reason(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
    ) -> Option<ValidationInflightInlineReason> {
        let inflight = self.subsystems.validation.inflight.get(&hash)?;
        let vnext_owned = self
            .subsystems
            .validation
            .vnext_inflight
            .contains_key(&hash);
        if !vnext_owned
            && (self.subsystems.validation.result_rx.is_none()
                || self.subsystems.validation.work_txs.is_empty())
        {
            return Some(ValidationInflightInlineReason::WorkerDisconnected);
        }
        if let Some(frontier_generation) = inflight.frontier_generation
            && !self.frontier_owner_generation_matches(height, hash, frontier_generation)
        {
            return Some(ValidationInflightInlineReason::StaleFrontier {
                frontier_generation,
            });
        }
        let elapsed = Instant::now().saturating_duration_since(inflight.started_at);
        let stall_timeout = self.validation_worker_stall_timeout_for(hash);
        (elapsed >= stall_timeout).then_some(ValidationInflightInlineReason::Stalled {
            elapsed,
            stall_timeout,
        })
    }

    pub(super) fn vnext_validation_redrive_reason(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<VNextValidationRedriveReason> {
        let round_slot = self
            .vnext_rounds
            .get(&(height, view))
            .and_then(|round| round.slot(hash))?;

        match round_slot.slot_state {
            super::vnext::SlotState::Recovering { .. }
            | super::vnext::SlotState::Aborted { .. }
            | super::vnext::SlotState::Committed { .. } => return None,
            super::vnext::SlotState::Idle
            | super::vnext::SlotState::Proposed { .. }
            | super::vnext::SlotState::AwaitingAvailability { .. }
            | super::vnext::SlotState::AwaitingValidation { .. }
            | super::vnext::SlotState::Prepared { .. } => {}
        }

        let has_inflight = self.subsystems.validation.inflight.contains_key(&hash);
        let has_vnext_inflight = self
            .subsystems
            .validation
            .vnext_inflight
            .contains_key(&hash);
        match round_slot.validation {
            super::vnext::ValidationState::Queued { queued_at_ms, .. } => {
                let retry_ms =
                    u64::try_from(self.commit_validation_inline_fallback_timeout().as_millis())
                        .unwrap_or(u64::MAX);
                (Self::vnext_now_ms().saturating_sub(queued_at_ms) >= retry_ms)
                    .then_some(VNextValidationRedriveReason::OrphanedQueued)
            }
            super::vnext::ValidationState::Running { .. } => {
                if !has_inflight || !has_vnext_inflight {
                    Some(VNextValidationRedriveReason::OrphanedRunning)
                } else {
                    self.validation_inflight_inline_reason(hash, height)
                        .map(|_| VNextValidationRedriveReason::StalledRunning)
                }
            }
            super::vnext::ValidationState::Backpressured { since_ms } => {
                let retry_ms =
                    u64::try_from(self.commit_validation_inline_fallback_timeout().as_millis())
                        .unwrap_or(u64::MAX);
                (Self::vnext_now_ms().saturating_sub(since_ms) >= retry_ms)
                    .then_some(VNextValidationRedriveReason::Backpressured)
            }
            super::vnext::ValidationState::Unqueued
            | super::vnext::ValidationState::Valid { .. }
            | super::vnext::ValidationState::Invalid { .. } => None,
        }
    }

    pub(super) fn redrive_stale_vnext_validation_for_pending(
        &mut self,
        hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        payload_hash: Hash,
        trigger: &'static str,
    ) -> bool {
        let Some(reason) = self.vnext_validation_redrive_reason(hash, height, view) else {
            return false;
        };

        let Some(slot) = self
            .vnext_rounds
            .get(&(height, view))
            .and_then(|round| round.slot(hash))
            .map(|slot| slot.slot)
        else {
            return false;
        };

        if self.subsystems.validation.inflight.contains_key(&hash) {
            let _ = self.supersede_validation_inflight(hash);
        } else {
            self.subsystems.validation.vnext_inflight.remove(&hash);
        }

        let _ = self.mark_vnext_validation_deferred(slot);

        warn!(
            height,
            view,
            block = %hash,
            reason = reason.label(),
            trigger,
            "redriving stale vNext-owned pending validation"
        );
        self.drive_vnext_validation_for_pending(hash, height, view, payload_hash)
    }

    /// Validate a pending block (stateless + stateful) before sending any votes.
    pub(super) fn validate_pending_block_for_voting(
        &mut self,
        hash: HashOf<BlockHeader>,
        commit_topology: &[PeerId],
    ) -> ValidationGateOutcome {
        self.validate_pending_block_for_voting_with_dispatch(
            hash,
            commit_topology,
            ValidationDispatch::TryWorker,
        )
    }

    /// Validate a pending block before voting, running validation inline.
    #[cfg(test)]
    pub(super) fn validate_pending_block_for_voting_inline(
        &mut self,
        hash: HashOf<BlockHeader>,
        commit_topology: &[PeerId],
    ) -> ValidationGateOutcome {
        self.validate_pending_block_for_voting_with_dispatch(
            hash,
            commit_topology,
            ValidationDispatch::Inline,
        )
    }

    fn validate_pending_block_for_voting_with_dispatch(
        &mut self,
        hash: HashOf<BlockHeader>,
        commit_topology: &[PeerId],
        dispatch: ValidationDispatch,
    ) -> ValidationGateOutcome {
        if let Some((height, view, payload_hash)) = self
            .pending
            .pending_blocks
            .get(&hash)
            .map(|pending| (pending.height, pending.view, pending.payload_hash))
            && self.vnext_validation_owns_block(hash, height, view)
        {
            if dispatch.try_worker()
                && self.redrive_stale_vnext_validation_for_pending(
                    hash,
                    height,
                    view,
                    payload_hash,
                    "validation_gate",
                )
            {
                return ValidationGateOutcome::Deferred;
            }
            debug!(
                block = %hash,
                "deferring legacy validation while vNext owns validation"
            );
            return ValidationGateOutcome::Deferred;
        }

        #[cfg(test)]
        if dispatch.inline() {
            let _ = self.supersede_validation_inflight(hash);
        }

        let pending = match self.pending.pending_blocks.remove(&hash) {
            Some(pending) => pending,
            None => return ValidationGateOutcome::Deferred,
        };

        let mut pending = match self.check_pending_validation_status(hash, pending) {
            Ok(pending) => pending,
            Err(outcome) => return outcome,
        };
        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if pending.height == local_height.saturating_add(1) {
            let expected_parent = self.state.view().latest_block_hash();
            let actual_parent = pending.block.header().prev_block_hash();
            if actual_parent != expected_parent {
                let err = BlockValidationError::PrevBlockHashMismatch {
                    expected: expected_parent,
                    actual: actual_parent,
                };
                return self.finalize_validation_failure(hash, pending, &err);
            }
        }

        let validation_priority_reason =
            self.pending_block_validation_priority_reason(hash, &pending);
        let has_qc = pending.commit_qc_observed()
            || self.pending_block_has_qc(hash, pending.height, pending.view);
        if !has_qc
            && !self.slot_has_proposal_evidence(pending.height, pending.view)
            && validation_priority_reason.is_none()
        {
            debug!(
                height = pending.height,
                view = pending.view,
                block = %hash,
                "deferring validation before voting: proposal not observed for pending block"
            );
            pending.validation_status = ValidationStatus::Pending;
            self.pending.pending_blocks.insert(hash, pending);
            return ValidationGateOutcome::Deferred;
        }
        if let Some(reason) = validation_priority_reason {
            debug!(
                height = pending.height,
                view = pending.view,
                block = %hash,
                reason,
                "allowing validation with priority due to near-tip commit readiness"
            );
        }

        if commit_topology.is_empty() {
            warn!(
                height = pending.height,
                view = pending.view,
                block = %hash,
                "deferring validation before voting: empty commit topology"
            );
            pending.validation_status = ValidationStatus::Pending;
            self.pending.pending_blocks.insert(hash, pending);
            return ValidationGateOutcome::Deferred;
        }

        let mut topology = super::network_topology::Topology::new(commit_topology.to_vec());
        if let Err(err) = self.leader_index_for(&mut topology, pending.height, pending.view) {
            warn!(
                ?err,
                height = pending.height,
                view = pending.view,
                block = %hash,
                "deferring validation before voting: failed to align topology"
            );
            pending.validation_status = ValidationStatus::Pending;
            self.pending.pending_blocks.insert(hash, pending);
            return ValidationGateOutcome::Deferred;
        }

        let near_quorum_commit_votes = matches!(validation_priority_reason, Some("commit_votes"))
            && self
                .pending_block_commit_votes_count(hash, pending.height, pending.view)
                .saturating_add(1)
                >= topology.min_votes_for_commit().max(1);
        let inline_tx_count = dispatch
            .try_worker()
            .then(|| {
                self.fast_finality_inline_validation_tx_count(
                    hash,
                    &pending,
                    local_height,
                    validation_priority_reason,
                )
            })
            .flatten();
        let fast_finality_validation_reason =
            if matches!(validation_priority_reason, Some("commit_qc" | "cached_qc")) {
                validation_priority_reason
            } else if near_quorum_commit_votes {
                Some("commit_votes_near_quorum")
            } else if inline_tx_count.is_some() {
                Some("small_fast_finality_block")
            } else {
                None
            };
        if dispatch.try_worker() && fast_finality_validation_reason.is_some() {
            debug!(
                height = pending.height,
                view = pending.view,
                block = %hash,
                reason = fast_finality_validation_reason,
                tx_count = inline_tx_count,
                "routing fast-finality validation evidence through vNext worker path"
            );
        }
        let superseded_by_newer_view = self.pending.pending_blocks.values().any(|other| {
            other.height == pending.height
                && other.view > pending.view
                && !other.aborted
                && !matches!(other.validation_status, ValidationStatus::Invalid)
        });
        if superseded_by_newer_view
            && !pending.commit_qc_observed()
            && validation_priority_reason.is_none()
        {
            debug!(
                height = pending.height,
                view = pending.view,
                block = %hash,
                "deferring validation before voting: newer pending view already exists"
            );
            pending.validation_status = ValidationStatus::Pending;
            self.pending.pending_blocks.insert(hash, pending);
            return ValidationGateOutcome::Deferred;
        }

        let expected_height = local_height.saturating_add(1);
        if pending.height > expected_height {
            if let Some(parent_hash) = pending.block.header().prev_block_hash() {
                self.request_missing_parent(
                    hash,
                    pending.height,
                    pending.view,
                    parent_hash,
                    commit_topology,
                    None,
                    usize::try_from(expected_height).ok(),
                    usize::try_from(pending.height).ok(),
                    "validation_precheck",
                );
            }
            debug!(
                height = pending.height,
                view = pending.view,
                block = %hash,
                local_height,
                expected_height,
                "deferring validation before voting: block is ahead of local height"
            );
            pending.validation_status = ValidationStatus::Pending;
            self.pending.pending_blocks.insert(hash, pending);
            return ValidationGateOutcome::Deferred;
        }

        if dispatch.try_worker() {
            if self.subsystems.validation.work_txs.is_empty() {
                let pending_height = pending.height;
                let pending_view = pending.view;
                let payload_hash = pending.payload_hash;
                pending.validation_status = ValidationStatus::Pending;
                self.pending.pending_blocks.insert(hash, pending);
                self.drive_vnext_validation_for_pending(
                    hash,
                    pending_height,
                    pending_view,
                    payload_hash,
                );
                return ValidationGateOutcome::Deferred;
            }

            if self.subsystems.validation.inflight.contains_key(&hash) {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    "deferring validation before voting: validation in progress"
                );
                pending.validation_status = ValidationStatus::Pending;
                self.pending.pending_blocks.insert(hash, pending);
                return ValidationGateOutcome::Deferred;
            }

            let pending_height = pending.height;
            let pending_view = pending.view;
            let payload_hash = pending.payload_hash;
            pending.validation_status = ValidationStatus::Pending;
            self.pending.pending_blocks.insert(hash, pending);
            self.drive_vnext_validation_for_pending(
                hash,
                pending_height,
                pending_view,
                payload_hash,
            );
            return ValidationGateOutcome::Deferred;
        }

        // Inline fallback is reserved for active-frontier recovery when worker
        // or vNext validation is disconnected, stale, or stalled.
        let mut voting_block = self.voting_block.take();
        let validation_start = Instant::now();
        let result = validate_block_for_voting(
            pending.block.clone(),
            &mut topology,
            &self.common_config.chain,
            &self.genesis_account,
            self.state.as_ref(),
            &mut voting_block,
        );
        self.subsystems
            .validation
            .record_duration(validation_start.elapsed());
        // Avoid holding onto a voting block from the pre-vote validation path.
        self.voting_block = None;

        match result {
            Ok(roots) => {
                if let Some(roots) = roots {
                    let roots = self.maybe_corrupt_debug_witness_roots(
                        hash,
                        pending.height,
                        pending.view,
                        roots,
                    );
                    pending.parent_state_root = Some(roots.parent_state_root);
                    pending.post_state_root = Some(roots.post_state_root);
                    pending.note_validated_commit_artifact(
                        hash,
                        pending.height,
                        pending.view,
                        roots.parent_state_root,
                        roots.post_state_root,
                    );
                } else {
                    pending.parent_state_root = None;
                    pending.post_state_root = None;
                    pending.validated_commit_artifact = None;
                }
                let pending_height = pending.height;
                let pending_view = pending.view;
                pending.validation_status = ValidationStatus::Valid;
                pending.touch_progress(Instant::now());
                self.pending.pending_blocks.insert(hash, pending);
                self.replay_cached_precommit_qc_for_valid_block(
                    hash,
                    pending_height,
                    pending_view,
                    commit_topology,
                    "validation_inline",
                );
                ValidationGateOutcome::Valid
            }
            Err(err) => {
                if let BlockValidationError::PrevBlockHeightMismatch { expected, actual } = &err {
                    if let Some(parent_hash) = pending.block.header().prev_block_hash() {
                        self.request_missing_parent(
                            hash,
                            pending.height,
                            pending.view,
                            parent_hash,
                            commit_topology,
                            None,
                            Some(*expected),
                            Some(*actual),
                            "validation",
                        );
                    }
                }
                if self.should_accept_observer_signature_mismatch_with_commit_qc(
                    hash,
                    &pending,
                    commit_topology,
                    &err,
                ) {
                    warn!(
                        ?err,
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        commit_qc_seen = pending.commit_qc_observed(),
                        has_cached_qc = self.pending_block_has_qc(hash, pending.height, pending.view),
                        "accepting pending block for commit-only progression despite signature mismatch: local peer outside commit roster"
                    );
                    let pending_height = pending.height;
                    let pending_view = pending.view;
                    pending.validation_status = ValidationStatus::Valid;
                    pending.touch_progress(Instant::now());
                    self.pending.pending_blocks.insert(hash, pending);
                    self.replay_cached_precommit_qc_for_valid_block(
                        hash,
                        pending_height,
                        pending_view,
                        commit_topology,
                        "validation_inline_signature_recovery",
                    );
                    return ValidationGateOutcome::Valid;
                }
                self.finalize_validation_failure(hash, pending, &err)
            }
        }
    }

    pub(in crate::sumeragi) fn poll_validation_results(&mut self) -> bool {
        let Some(result_rx) = self.subsystems.validation.result_rx.take() else {
            return false;
        };
        let mut progress = false;
        let mut keep_rx = true;
        loop {
            match result_rx.try_recv() {
                Ok(result) => {
                    let ValidationResult {
                        id,
                        hash,
                        height,
                        view,
                        frontier_generation,
                        commit_topology,
                        duration,
                        outcome,
                    } = result;
                    self.subsystems.validation.record_duration(duration);
                    let (inflight_frontier_generation, vnext_inflight) = match self
                        .subsystems
                        .validation
                        .inflight
                        .remove(&hash)
                    {
                        Some(inflight) => {
                            if inflight.id != id {
                                let inflight_id = inflight.id;
                                self.subsystems.validation.inflight.insert(hash, inflight);
                                warn!(
                                    block = %hash,
                                    inflight_id,
                                    result_id = id,
                                    "validation result id mismatch; ignoring"
                                );
                                continue;
                            }
                            let vnext_inflight =
                                self.subsystems.validation.vnext_inflight.remove(&hash);
                            (inflight.frontier_generation, vnext_inflight)
                        }
                        None => {
                            self.subsystems.validation.vnext_inflight.remove(&hash);
                            if let Some(superseded_id) =
                                self.subsystems.validation.superseded_results.remove(&hash)
                            {
                                if superseded_id == id {
                                    debug!(
                                        block = %hash,
                                        result_id = id,
                                        "validation result superseded by validation redrive; dropping stale worker result"
                                    );
                                } else {
                                    warn!(
                                        block = %hash,
                                        result_id = id,
                                        superseded_id,
                                        "validation result id mismatch for superseded inflight; dropping stale worker result"
                                    );
                                }
                                progress = true;
                                continue;
                            }

                            let Some(pending) = self.pending.pending_blocks.get(&hash) else {
                                debug!(
                                    block = %hash,
                                    result_id = id,
                                    "dropping validation result for unknown block"
                                );
                                progress = true;
                                continue;
                            };
                            if pending.height != height || pending.view != view {
                                warn!(
                                    block = %hash,
                                    pending_height = pending.height,
                                    pending_view = pending.view,
                                    result_height = height,
                                    result_view = view,
                                    result_id = id,
                                    "validation result without inflight does not match pending block"
                                );
                                progress = true;
                                continue;
                            }
                            if pending.validation_status != ValidationStatus::Pending {
                                debug!(
                                    block = %hash,
                                    result_id = id,
                                    ?pending.validation_status,
                                    "dropping validation result without inflight for non-pending block"
                                );
                                progress = true;
                                continue;
                            }
                            if let Some(frontier_generation) = frontier_generation
                                && !self.frontier_owner_generation_matches(
                                    height,
                                    hash,
                                    frontier_generation,
                                )
                            {
                                debug!(
                                    block = %hash,
                                    result_id = id,
                                    height,
                                    view,
                                    frontier_generation,
                                    "dropping validation result without inflight after frontier owner supersede"
                                );
                                progress = true;
                                continue;
                            }

                            warn!(
                                block = %hash,
                                result_id = id,
                                height,
                                view,
                                "recovering validation result after inflight marker disappeared"
                            );
                            (None, None)
                        }
                    };
                    if let Some(frontier_generation) = inflight_frontier_generation
                        && !self.frontier_owner_generation_matches(
                            height,
                            hash,
                            frontier_generation,
                        )
                    {
                        debug!(
                            block = %hash,
                            result_id = id,
                            height,
                            view,
                            frontier_generation,
                            "dropping validation result for superseded frontier owner generation"
                        );
                        progress = true;
                        continue;
                    }

                    let mut vnext_result = vnext_inflight.map(|vnext| {
                        let outcome = match &outcome {
                            Ok(Some(roots)) => {
                                let roots = self
                                    .maybe_corrupt_debug_witness_roots(hash, height, view, *roots);
                                super::vnext::ValidationWorkerOutcome::Valid(
                                    super::vnext::ValidationRoots {
                                        parent_state_root: roots.parent_state_root,
                                        post_state_root: roots.post_state_root,
                                    },
                                )
                            }
                            Ok(None) => super::vnext::ValidationWorkerOutcome::Invalid(
                                super::vnext::ValidationFailure {
                                    reason_label: "validation_roots_missing".to_owned(),
                                    evidence_hash: None,
                                },
                            ),
                            Err(err) => super::vnext::ValidationWorkerOutcome::Invalid(
                                super::vnext::ValidationFailure {
                                    reason_label: validation_reject_reason_label(err).to_owned(),
                                    evidence_hash: None,
                                },
                            ),
                        };
                        (
                            vnext.slot,
                            super::vnext::ValidationWorkerResult {
                                id,
                                generation: vnext.generation,
                                outcome,
                            },
                        )
                    });

                    let Some(mut pending) = self.pending.pending_blocks.remove(&hash) else {
                        warn!(block = %hash, "validation result received without pending block");
                        progress = true;
                        continue;
                    };
                    if pending.height != height || pending.view != view {
                        warn!(
                            block = %hash,
                            pending_height = pending.height,
                            pending_view = pending.view,
                            result_height = height,
                            result_view = view,
                            "validation result does not match pending block"
                        );
                        self.pending.pending_blocks.insert(hash, pending);
                        progress = true;
                        continue;
                    }
                    if let Some(frontier_generation) = frontier_generation
                        && !self.frontier_owner_generation_matches(
                            height,
                            hash,
                            frontier_generation,
                        )
                    {
                        debug!(
                            block = %hash,
                            result_id = id,
                            height,
                            view,
                            frontier_generation,
                            "dropping late validation result after frontier owner supersede"
                        );
                        self.pending.pending_blocks.insert(hash, pending);
                        progress = true;
                        continue;
                    }

                    if let Some((slot, result)) = vnext_result.take() {
                        if matches!(
                            result.outcome,
                            super::vnext::ValidationWorkerOutcome::Valid(_)
                        ) {
                            self.pending.pending_blocks.insert(hash, pending);
                            self.handle_vnext_validation_result(slot, result);
                            progress = true;
                            continue;
                        }
                        vnext_result = Some((slot, result));
                    }

                    match outcome {
                        Ok(roots) => {
                            if let Some(roots) = roots {
                                let roots = self
                                    .maybe_corrupt_debug_witness_roots(hash, height, view, roots);
                                pending.parent_state_root = Some(roots.parent_state_root);
                                pending.post_state_root = Some(roots.post_state_root);
                                pending.note_validated_commit_artifact(
                                    hash,
                                    pending.height,
                                    pending.view,
                                    roots.parent_state_root,
                                    roots.post_state_root,
                                );
                            } else {
                                pending.parent_state_root = None;
                                pending.post_state_root = None;
                                pending.validated_commit_artifact = None;
                            }
                            pending.validation_status = ValidationStatus::Valid;
                            pending.touch_progress(Instant::now());
                            self.pending.pending_blocks.insert(hash, pending);
                            self.replay_cached_precommit_qc_for_valid_block(
                                hash,
                                height,
                                view,
                                &commit_topology,
                                "validation_worker",
                            );
                            let _ = self.maybe_emit_local_commit_vote_for_pending_event(
                                hash,
                                height,
                                view,
                                &commit_topology,
                                "validation_passed",
                            );
                            self.request_commit_pipeline_for_pending(
                                hash,
                                super::status::RoundEventCauseTrace::ValidationPassed,
                                None,
                            );
                        }
                        Err(err) => {
                            if let BlockValidationError::PrevBlockHeightMismatch {
                                expected,
                                actual,
                            } = &err
                            {
                                if let Some(parent_hash) = pending.block.header().prev_block_hash()
                                {
                                    self.request_missing_parent(
                                        hash,
                                        pending.height,
                                        pending.view,
                                        parent_hash,
                                        &commit_topology,
                                        None,
                                        Some(*expected),
                                        Some(*actual),
                                        "validation",
                                    );
                                }
                            }
                            if self.should_accept_observer_signature_mismatch_with_commit_qc(
                                hash,
                                &pending,
                                &commit_topology,
                                &err,
                            ) {
                                warn!(
                                    ?err,
                                    height = pending.height,
                                    view = pending.view,
                                    block = %hash,
                                    commit_qc_seen = pending.commit_qc_observed(),
                                    has_cached_qc = self
                                        .pending_block_has_qc(hash, pending.height, pending.view),
                                    "accepting pending block for commit-only progression despite signature mismatch: local peer outside commit roster"
                                );
                                pending.validation_status = ValidationStatus::Valid;
                                pending.touch_progress(Instant::now());
                                self.pending.pending_blocks.insert(hash, pending);
                                self.replay_cached_precommit_qc_for_valid_block(
                                    hash,
                                    height,
                                    view,
                                    &commit_topology,
                                    "validation_worker_signature_recovery",
                                );
                                let _ = self.maybe_emit_local_commit_vote_for_pending_event(
                                    hash,
                                    height,
                                    view,
                                    &commit_topology,
                                    "validation_passed",
                                );
                                self.request_commit_pipeline_for_pending(
                                    hash,
                                    super::status::RoundEventCauseTrace::ValidationPassed,
                                    None,
                                );
                                progress = true;
                                continue;
                            }
                            match self.finalize_validation_failure(hash, pending, &err) {
                                ValidationGateOutcome::Invalid {
                                    hash: invalid_hash,
                                    height: invalid_height,
                                    view: invalid_view,
                                    evidence,
                                    reason,
                                    reason_label,
                                } => {
                                    self.handle_validation_reject(
                                        invalid_hash,
                                        invalid_height,
                                        invalid_view,
                                        evidence,
                                        reason,
                                        reason_label,
                                    );
                                    if let Some((slot, result)) = vnext_result.take() {
                                        self.handle_vnext_validation_result(slot, result);
                                    }
                                    progress = true;
                                    continue;
                                }
                                ValidationGateOutcome::Deferred => {
                                    if let Some((slot, _result)) = vnext_result.take() {
                                        let _ = self.mark_vnext_validation_deferred(slot);
                                    }
                                    self.request_commit_pipeline();
                                }
                                ValidationGateOutcome::Valid => {
                                    vnext_result = None;
                                    self.request_commit_pipeline();
                                }
                            }
                        }
                    }
                    if let Some((slot, result)) = vnext_result {
                        self.handle_vnext_validation_result(slot, result);
                    }
                    progress = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("validation worker result channel closed; falling back to inline");
                    self.subsystems.validation.work_txs.clear();
                    self.subsystems.validation.inflight.clear();
                    self.subsystems.validation.vnext_inflight.clear();
                    self.subsystems.validation.superseded_results.clear();
                    keep_rx = false;
                    break;
                }
            }
        }
        if keep_rx {
            self.subsystems.validation.result_rx = Some(result_rx);
        }
        progress
    }

    pub(super) fn accept_vnext_validated_slot(
        &mut self,
        slot: super::vnext::SlotId,
        roots: super::vnext::ValidationRoots,
    ) {
        let Some(mut pending) = self.pending.pending_blocks.remove(&slot.block_hash) else {
            warn!(
                height = slot.height,
                view = slot.view,
                block = %slot.block_hash,
                "vNext accepted validation for missing pending block"
            );
            return;
        };
        if pending.height != slot.height || pending.view != slot.view {
            warn!(
                pending_height = pending.height,
                pending_view = pending.view,
                slot_height = slot.height,
                slot_view = slot.view,
                block = %slot.block_hash,
                "vNext accepted validation for mismatched pending slot"
            );
            self.pending.pending_blocks.insert(slot.block_hash, pending);
            return;
        }

        pending.parent_state_root = Some(roots.parent_state_root);
        pending.post_state_root = Some(roots.post_state_root);
        pending.note_validated_commit_artifact(
            slot.block_hash,
            slot.height,
            slot.view,
            roots.parent_state_root,
            roots.post_state_root,
        );
        pending.validation_status = ValidationStatus::Valid;
        pending.touch_progress(Instant::now());
        self.pending.pending_blocks.insert(slot.block_hash, pending);

        let commit_topology = self
            .vnext_rounds
            .get(&(slot.height, slot.view))
            .map(|round| round.chain_order.ordered_validators.clone())
            .unwrap_or_else(|| self.effective_commit_topology());
        self.replay_cached_precommit_qc_for_valid_block(
            slot.block_hash,
            slot.height,
            slot.view,
            &commit_topology,
            "vnext_validation",
        );
        let _ = self.maybe_emit_local_commit_vote_for_pending_event(
            slot.block_hash,
            slot.height,
            slot.view,
            &commit_topology,
            "vnext_validation_passed",
        );
        self.request_commit_pipeline_for_pending(
            slot.block_hash,
            super::status::RoundEventCauseTrace::ValidationPassed,
            None,
        );
    }

    pub(super) fn reject_vnext_validation_slot(
        &mut self,
        slot: super::vnext::SlotId,
        failure: super::vnext::ValidationFailure,
    ) {
        let Some(mut pending) = self.pending.pending_blocks.remove(&slot.block_hash) else {
            debug!(
                height = slot.height,
                view = slot.view,
                block = %slot.block_hash,
                reason = %failure.reason_label,
                "vNext rejected validation after the actor already handled the pending block"
            );
            return;
        };
        if pending.height != slot.height || pending.view != slot.view {
            warn!(
                pending_height = pending.height,
                pending_view = pending.view,
                slot_height = slot.height,
                slot_view = slot.view,
                block = %slot.block_hash,
                reason = %failure.reason_label,
                "vNext rejected validation for mismatched pending slot"
            );
            self.pending.pending_blocks.insert(slot.block_hash, pending);
            return;
        }

        let should_requeue =
            pending.validation_status != ValidationStatus::Invalid && !pending.aborted;
        if should_requeue {
            let txs: Vec<_> = pending.block.external_entrypoints_cloned().collect();
            let (_requeued, failures, _duplicates, _) =
                requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
            if failures > 0 {
                warn!(
                    height = slot.height,
                    view = slot.view,
                    block = %slot.block_hash,
                    failures,
                    "failed to requeue some transactions after vNext validation rejection"
                );
            }
        }
        pending.validation_status = ValidationStatus::Invalid;
        pending.mark_aborted();
        let _ = pending;

        self.subsystems.validation.inflight.remove(&slot.block_hash);
        self.subsystems
            .validation
            .vnext_inflight
            .remove(&slot.block_hash);
        self.subsystems
            .validation
            .superseded_results
            .remove(&slot.block_hash);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(slot.height, slot.view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(slot.height, slot.view);
        self.clean_rbc_sessions_for_block(slot.block_hash, slot.height);
        self.qc_cache
            .retain(|(_, cached_hash, _, _, _, _, _), _| cached_hash != &slot.block_hash);
        self.qc_signer_tally
            .retain(|(_, cached_hash, _, _, _, _, _), _| cached_hash != &slot.block_hash);

        let reason_label = vnext_validation_reject_reason_label(&failure.reason_label);
        let reason = format!("vNext validation rejected: {}", failure.reason_label);
        self.handle_validation_reject(
            slot.block_hash,
            slot.height,
            slot.view,
            None,
            reason,
            reason_label,
        );
    }

    fn check_pending_validation_status(
        &mut self,
        hash: HashOf<BlockHeader>,
        pending: PendingBlock,
    ) -> Result<PendingBlock, ValidationGateOutcome> {
        match pending.validation_status {
            ValidationStatus::Valid => {
                self.pending.pending_blocks.insert(hash, pending);
                Err(ValidationGateOutcome::Valid)
            }
            ValidationStatus::Invalid => {
                let height = pending.height;
                let view = pending.view;
                self.pending.pending_blocks.insert(hash, pending);
                Err(ValidationGateOutcome::Invalid {
                    hash,
                    height,
                    view,
                    reason: "pending block previously marked invalid".to_owned(),
                    reason_label: VALIDATION_REASON_STATELESS,
                    evidence: None,
                })
            }
            ValidationStatus::Pending => Ok(pending),
        }
    }

    pub(super) fn should_accept_observer_signature_mismatch_with_commit_qc(
        &self,
        hash: HashOf<BlockHeader>,
        pending: &PendingBlock,
        commit_topology: &[PeerId],
        err: &BlockValidationError,
    ) -> bool {
        let local_in_commit_topology = commit_topology
            .iter()
            .any(|peer| peer == self.common_config.peer.id());
        if local_in_commit_topology {
            return false;
        }
        let signature_mismatch = matches!(
            err,
            BlockValidationError::SignatureVerification(
                crate::block::SignatureVerificationError::UnknownSignature
                    | crate::block::SignatureVerificationError::UnknownSignatory
                    | crate::block::SignatureVerificationError::MissingPop
                    | crate::block::SignatureVerificationError::LeaderMissing
            )
        );
        if !signature_mismatch {
            return false;
        }
        let has_commit_qc = pending.commit_qc_observed()
            || self.pending_block_has_qc(hash, pending.height, pending.view);
        if !has_commit_qc {
            return false;
        }
        true
    }

    fn finalize_validation_failure(
        &mut self,
        hash: HashOf<BlockHeader>,
        mut pending: PendingBlock,
        err: &BlockValidationError,
    ) -> ValidationGateOutcome {
        if let BlockValidationError::PrevBlockHeightMismatch { expected, actual } = &err {
            if actual > expected {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    expected,
                    actual,
                    "deferring validation for block ahead of local height"
                );
                pending.validation_status = ValidationStatus::Pending;
                self.pending.pending_blocks.insert(hash, pending);
                return ValidationGateOutcome::Deferred;
            }
        }
        let height = pending.height;
        let view = pending.view;
        let parent = pending.block.header().prev_block_hash();
        let txs: Vec<_> = pending.block.external_entrypoints_cloned().collect();
        let reason_label = validation_reject_reason_label(err);
        let proposal_epoch = self.epoch_for_height(height);
        pending.validation_status = ValidationStatus::Invalid;
        pending.mark_aborted();
        warn!(
            ?err,
            height,
            view,
            block = %hash,
            "rejecting pending block before voting due to validation failure"
        );
        let evidence = self
            .qc_for_validation_evidence(height, parent)
            .map(|qc| {
                build_invalid_proposal_evidence(
                    &pending.block,
                    pending.payload_hash,
                    qc,
                    proposal_epoch,
                    err.to_string(),
                )
            })
            .map(Box::new);
        let _ = pending;

        let (_requeued, failures, _duplicates, _) =
            requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
        if failures > 0 {
            warn!(
                height,
                view, failures, "failed to requeue some transactions after validation rejection"
            );
        }
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.clean_rbc_sessions_for_block(hash, height);
        self.qc_cache
            .retain(|(_, cached_hash, _, _, _, _, _), _| cached_hash != &hash);
        self.qc_signer_tally
            .retain(|(_, cached_hash, _, _, _, _, _), _| cached_hash != &hash);
        if matches!(err, BlockValidationError::PreviousRosterEvidenceInvalid(_)) {
            let _ = self.maybe_trigger_payload_mismatch_recovery_bundle(
                height,
                view,
                hash,
                Instant::now(),
                "validation_previous_roster_evidence",
            );
        }
        ValidationGateOutcome::Invalid {
            hash,
            height,
            view,
            reason: err.to_string(),
            reason_label,
            evidence,
        }
    }

    fn qc_for_validation_evidence(
        &self,
        block_height: u64,
        parent_hash: Option<HashOf<BlockHeader>>,
    ) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
        let parent_hash = parent_hash?;
        let candidates = [self.highest_qc, self.locked_qc, self.latest_committed_qc()];
        candidates
            .into_iter()
            .flatten()
            .find(|qc| qc.subject_block_hash == parent_hash && qc.height < block_height)
    }
}
