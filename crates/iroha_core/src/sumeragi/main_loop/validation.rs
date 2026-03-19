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
    pub(super) topology: super::network_topology::Topology,
    pub(super) commit_topology: Vec<PeerId>,
}

#[derive(Debug)]
pub(super) struct ValidationResult {
    pub(super) id: u64,
    pub(super) hash: HashOf<BlockHeader>,
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) commit_topology: Vec<PeerId>,
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
    Inline,
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
                        mut topology,
                        commit_topology,
                    } = work;
                    let mut voting_block = None;
                    let outcome = validate_block_for_voting(
                        block,
                        &mut topology,
                        &chain_id,
                        &genesis_account,
                        state.as_ref(),
                        &mut voting_block,
                    );
                    if result_tx
                        .send(ValidationResult {
                            id,
                            hash,
                            height,
                            view,
                            commit_topology,
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

    fn validation_queue_full_inline_cutover(&self) -> Duration {
        let fast_timeout = self.pending_fast_path_timeout_current();
        let divisor = self
            .config
            .worker
            .validation_queue_full_inline_cutover_divisor
            .max(1);
        if fast_timeout == Duration::ZERO {
            Duration::ZERO
        } else {
            (fast_timeout / divisor).max(Duration::from_millis(1))
        }
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
        if matches!(dispatch, ValidationDispatch::Inline) {
            // Inline validation supersedes any stale worker intent for this block.
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

        let validation_priority_reason =
            self.pending_block_validation_priority_reason(hash, &pending);
        let has_commit_qc = pending.commit_qc_seen
            || self.pending_block_has_commit_qc(hash, pending.height, pending.view);
        if !has_commit_qc
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
        } else if let Some(reason) = validation_priority_reason {
            debug!(
                height = pending.height,
                view = pending.view,
                block = %hash,
                reason,
                "allowing validation before proposal evidence due to near-tip commit readiness"
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

        let superseded_by_newer_view = self.pending.pending_blocks.values().any(|other| {
            other.height == pending.height
                && other.view > pending.view
                && !other.aborted
                && !matches!(other.validation_status, ValidationStatus::Invalid)
        });
        if superseded_by_newer_view && !pending.commit_qc_seen {
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

        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
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

        if matches!(dispatch, ValidationDispatch::TryWorker)
            && !self.subsystems.validation.work_txs.is_empty()
        {
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

            let id = self.subsystems.validation.next_id();
            let mut work = ValidationWork {
                id,
                hash,
                block: pending.block.clone(),
                height: pending.height,
                view: pending.view,
                topology: topology.clone(),
                commit_topology: commit_topology.to_vec(),
            };
            let mut dispatched = false;
            let mut disconnected = Vec::new();
            let total = self.subsystems.validation.work_txs.len();
            for _ in 0..total {
                let idx = self.subsystems.validation.next_worker % total;
                self.subsystems.validation.next_worker =
                    self.subsystems.validation.next_worker.saturating_add(1);
                let work_tx = &self.subsystems.validation.work_txs[idx];
                match work_tx.try_send(work) {
                    Ok(()) => {
                        dispatched = true;
                        break;
                    }
                    Err(mpsc::TrySendError::Full(returned)) => {
                        work = returned;
                    }
                    Err(mpsc::TrySendError::Disconnected(returned)) => {
                        work = returned;
                        disconnected.push(idx);
                    }
                }
            }

            if !disconnected.is_empty() {
                disconnected.sort_unstable();
                disconnected.dedup();
                for idx in disconnected.into_iter().rev() {
                    if idx < self.subsystems.validation.work_txs.len() {
                        self.subsystems.validation.work_txs.swap_remove(idx);
                    }
                }
                if self.subsystems.validation.next_worker
                    >= self.subsystems.validation.work_txs.len()
                {
                    self.subsystems.validation.next_worker = 0;
                }
            }

            if dispatched {
                self.subsystems.validation.superseded_results.remove(&hash);
                self.subsystems.validation.inflight.insert(
                    hash,
                    super::ValidationInFlight {
                        id,
                        started_at: Instant::now(),
                    },
                );
                pending.validation_status = ValidationStatus::Pending;
                self.pending.pending_blocks.insert(hash, pending);
                return ValidationGateOutcome::Deferred;
            }

            if self.subsystems.validation.work_txs.is_empty() {
                warn!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    "validation workers unavailable; running pre-vote validation inline"
                );
                self.subsystems.validation.result_rx = None;
                self.subsystems.validation.inflight.clear();
                self.subsystems.validation.superseded_results.clear();
            } else {
                let pending_age = pending.age();
                let fast_timeout = self.pending_fast_path_timeout_current();
                let inline_cutover = self.validation_queue_full_inline_cutover();
                if pending_age < inline_cutover {
                    warn!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        pending_age_ms = pending_age.as_millis(),
                        fast_timeout_ms = fast_timeout.as_millis(),
                        inline_cutover_ms = inline_cutover.as_millis(),
                        "validation worker queue full; deferring pre-vote validation"
                    );
                    pending.validation_status = ValidationStatus::Pending;
                    self.pending.pending_blocks.insert(hash, pending);
                    return ValidationGateOutcome::Deferred;
                }
                warn!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    pending_age_ms = pending_age.as_millis(),
                    fast_timeout_ms = fast_timeout.as_millis(),
                    inline_cutover_ms = inline_cutover.as_millis(),
                    "validation worker queue saturated; running pre-vote validation inline"
                );
            }
        }

        // This block is now taking the inline path (either explicitly requested
        // or because workers were unavailable/full), so any prior async marker
        // is stale and must not keep the block deferred.
        let _ = self.supersede_validation_inflight(hash);

        let mut voting_block = self.voting_block.take();
        let result = validate_block_for_voting(
            pending.block.clone(),
            &mut topology,
            &self.common_config.chain,
            &self.genesis_account,
            self.state.as_ref(),
            &mut voting_block,
        );
        // Avoid holding onto a voting block from the pre-vote validation path.
        self.voting_block = None;

        match result {
            Ok(roots) => {
                if let Some(roots) = roots {
                    pending.parent_state_root = Some(roots.parent_state_root);
                    pending.post_state_root = Some(roots.post_state_root);
                } else {
                    pending.parent_state_root = None;
                    pending.post_state_root = None;
                }
                pending.validation_status = ValidationStatus::Valid;
                self.pending.pending_blocks.insert(hash, pending);
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
                        commit_qc_seen = pending.commit_qc_seen,
                        has_cached_qc = self.pending_block_has_qc(hash, pending.height, pending.view),
                        "accepting pending block for commit-only progression despite signature mismatch: local peer outside commit roster"
                    );
                    pending.validation_status = ValidationStatus::Valid;
                    self.pending.pending_blocks.insert(hash, pending);
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
                        commit_topology,
                        outcome,
                    } = result;
                    let inflight = match self.subsystems.validation.inflight.remove(&hash) {
                        Some(inflight) => inflight,
                        None => {
                            if let Some(superseded_id) =
                                self.subsystems.validation.superseded_results.remove(&hash)
                            {
                                if superseded_id == id {
                                    debug!(
                                        block = %hash,
                                        result_id = id,
                                        "validation result superseded by inline fallback; dropping stale worker result"
                                    );
                                } else {
                                    warn!(
                                        block = %hash,
                                        result_id = id,
                                        superseded_id,
                                        "validation result id mismatch for superseded inflight; dropping stale worker result"
                                    );
                                }
                            } else if self.pending.pending_blocks.contains_key(&hash) {
                                warn!(
                                    block = %hash,
                                    result_id = id,
                                    "validation result received without inflight"
                                );
                            } else {
                                debug!(
                                    block = %hash,
                                    result_id = id,
                                    "dropping validation result for unknown block"
                                );
                            }
                            continue;
                        }
                    };
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

                    match outcome {
                        Ok(roots) => {
                            if let Some(roots) = roots {
                                pending.parent_state_root = Some(roots.parent_state_root);
                                pending.post_state_root = Some(roots.post_state_root);
                            } else {
                                pending.parent_state_root = None;
                                pending.post_state_root = None;
                            }
                            pending.validation_status = ValidationStatus::Valid;
                            self.pending.pending_blocks.insert(hash, pending);
                            self.request_commit_pipeline();
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
                                    commit_qc_seen = pending.commit_qc_seen,
                                    has_cached_qc = self
                                        .pending_block_has_qc(hash, pending.height, pending.view),
                                    "accepting pending block for commit-only progression despite signature mismatch: local peer outside commit roster"
                                );
                                pending.validation_status = ValidationStatus::Valid;
                                self.pending.pending_blocks.insert(hash, pending);
                                self.request_commit_pipeline();
                                progress = true;
                                continue;
                            }
                            let _ = self.finalize_validation_failure(hash, pending, &err);
                            self.request_commit_pipeline();
                        }
                    }
                    progress = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("validation worker result channel closed; falling back to inline");
                    self.subsystems.validation.work_txs.clear();
                    self.subsystems.validation.inflight.clear();
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
            )
        );
        if !signature_mismatch {
            return false;
        }
        let has_commit_qc =
            pending.commit_qc_seen || self.pending_block_has_qc(hash, pending.height, pending.view);
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
        let txs = pending.block.transactions_vec().clone();
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
            .retain(|(_, cached_hash, _, _, _), _| cached_hash != &hash);
        self.qc_signer_tally
            .retain(|(_, cached_hash, _, _, _), _| cached_hash != &hash);
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
