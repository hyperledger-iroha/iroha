//! Pending-block validation gates used by the commit pipeline.

use std::sync::{Arc, mpsc};

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

pub(super) fn spawn_validation_workers(
    state: Arc<State>,
    chain_id: ChainId,
    genesis_account: AccountId,
    wake_tx: Option<mpsc::SyncSender<()>>,
    worker_threads: usize,
    work_queue_cap: usize,
    result_queue_cap: usize,
) -> ValidationWorkerHandle {
    let threads = worker_threads.max(1);
    let work_queue_cap = work_queue_cap.max(1);
    let result_queue_cap = result_queue_cap.max(1);
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
        let join_handle = std::thread::Builder::new()
            .name(name)
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
    /// Validate a pending block (stateless + stateful) before sending any votes.
    pub(super) fn validate_pending_block_for_voting(
        &mut self,
        hash: HashOf<BlockHeader>,
        commit_topology: &[PeerId],
    ) -> ValidationGateOutcome {
        let pending = match self.pending.pending_blocks.remove(&hash) {
            Some(pending) => pending,
            None => return ValidationGateOutcome::Deferred,
        };

        let mut pending = match self.check_pending_validation_status(hash, pending) {
            Ok(pending) => pending,
            Err(outcome) => return outcome,
        };

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

        if !self.subsystems.validation.work_txs.is_empty() {
            if self.subsystems.validation.inflight.contains_key(&hash) {
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
                self.subsystems.validation.inflight.insert(hash, id);
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
            } else {
                warn!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    "validation worker queue full; deferring pre-vote validation"
                );
                pending.validation_status = ValidationStatus::Pending;
                self.pending.pending_blocks.insert(hash, pending);
                return ValidationGateOutcome::Deferred;
            }
        }

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
                            warn!(block = %hash, "validation result received without inflight");
                            continue;
                        }
                    };
                    if inflight != id {
                        warn!(
                            block = %hash,
                            inflight_id = inflight,
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
