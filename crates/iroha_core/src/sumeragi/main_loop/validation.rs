//! Pending-block validation gates used by the commit pipeline.

use iroha_logger::prelude::*;

use super::*;

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
            Ok(()) => {
                pending.validation_status = ValidationStatus::Valid;
                self.pending.pending_blocks.insert(hash, pending);
                ValidationGateOutcome::Valid
            }
            Err(err) => {
                if let BlockValidationError::PrevBlockHeightMismatch { expected, actual } = &err {
                    self.request_missing_parent_for_validation(
                        &pending,
                        *expected,
                        *actual,
                        commit_topology,
                    );
                }
                self.finalize_validation_failure(hash, pending, err)
            }
        }
    }

    fn request_missing_parent_for_validation(
        &mut self,
        pending: &PendingBlock,
        expected: usize,
        actual: usize,
        commit_topology: &[PeerId],
    ) {
        if actual <= expected {
            return;
        }
        let Some(parent_hash) = pending.block.header().prev_block_hash() else {
            return;
        };
        if self.block_known_locally(parent_hash) {
            return;
        }
        if commit_topology.is_empty() {
            debug!(
                height = pending.height,
                view = pending.view,
                block = %pending.block.hash(),
                missing_parent = ?parent_hash,
                "skipping missing-parent fetch: commit topology empty"
            );
            return;
        }

        let parent_height = pending.height.saturating_sub(1);
        let topology = super::network_topology::Topology::new(commit_topology.to_vec());
        let retry_window = self.quorum_timeout(self.runtime_da_enabled());
        let now = Instant::now();
        let signers = BTreeSet::new();
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let decision = super::plan_missing_block_fetch(
            &mut requests,
            parent_hash,
            parent_height,
            &signers,
            &topology,
            now,
            retry_window,
            self.config.missing_block_signer_fallback_attempts,
        );
        self.pending.missing_block_requests = requests;
        let dwell = self
            .pending
            .missing_block_requests
            .get(&parent_hash)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let targets_len = match &decision {
            MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
        match decision {
            MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(parent_hash, &targets);
                info!(
                    height = pending.height,
                    view = pending.view,
                    expected_height = expected,
                    actual_height = actual,
                    block = %pending.block.hash(),
                    missing_parent = ?parent_hash,
                    targets = ?targets,
                    target_kind = target_kind.label(),
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "requested missing parent block after validation mismatch"
                );
            }
            MissingBlockFetchDecision::NoTargets => {
                warn!(
                    height = pending.height,
                    view = pending.view,
                    expected_height = expected,
                    actual_height = actual,
                    block = %pending.block.hash(),
                    missing_parent = ?parent_hash,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "missing parent fetch deferred: no targets available"
                );
            }
            MissingBlockFetchDecision::Backoff => {
                trace!(
                    height = pending.height,
                    view = pending.view,
                    expected_height = expected,
                    actual_height = actual,
                    block = %pending.block.hash(),
                    missing_parent = ?parent_hash,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "missing parent fetch skipped due to backoff"
                );
            }
        }
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
        err: BlockValidationError,
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
        let reason_label = validation_reject_reason_label(&err);
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
                    err.to_string(),
                )
            })
            .map(Box::new);
        drop(pending);

        let (_requeued, failures, _duplicates, _) =
            requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
        if failures > 0 {
            warn!(
                height,
                view, failures, "failed to requeue some transactions after validation rejection"
            );
        }
        self.subsystems.propose.proposal_cache.pop_proposal(height, view);
        self.subsystems.propose.proposal_cache.pop_hint(height, view);
        self.clean_rbc_sessions_for_block(hash, height);
        self.qc_cache
            .retain(|(_, cached_hash, _, _, _), _| cached_hash != &hash);
        self.qc_signer_tally
            .retain(|(_, cached_hash, _, _, _), _| cached_hash != &hash);
        self.execution_qc_cache.remove(&hash);
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
