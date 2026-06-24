//! Commit/finalization pipeline helpers.

use std::{
    cmp::Reverse,
    collections::BTreeSet,
    io,
    sync::{Arc, mpsc},
    time::{Duration, Instant, SystemTime},
};

use iroha_crypto::blake2::{Blake2b512, Digest as BlakeDigest};
use iroha_data_model::Encode as _;
use iroha_logger::prelude::*;

use super::locked_qc::qc_satisfies_locked_with_lookup;
use super::pacing::{Pacemaker, PacemakerBackpressure, PacemakerBackpressureAction};
use super::pending_block::ValidatedCommitArtifact;
use super::propose::ProposalBackpressure;
use super::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum EpochRefreshPhase {
    PreCommit,
    PostCommit,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NewViewVoteEmission {
    Normal,
    CompleteNearQuorum,
}

#[derive(Debug, Clone)]
pub(super) struct CommitWork {
    pub(super) id: u64,
    pub(super) block: SignedBlock,
    pub(super) validated_commit_artifact: Option<ValidatedCommitArtifact>,
    pub(super) commit_topology: Vec<PeerId>,
    pub(super) signature_topology: Vec<PeerId>,
    pub(super) consensus_mode: ConsensusMode,
    pub(super) qc_signers: Option<BTreeSet<ValidatorIndex>>,
    pub(super) commit_qc: Option<crate::sumeragi::consensus::Qc>,
    pub(super) allow_signature_index_recovery: bool,
    pub(super) events_sender: crate::EventsSender,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KnownBlockCommitQcRecoveryRequestPlan {
    pub(super) commit_qc_only: bool,
    pub(super) body: bool,
}

const KNOWN_BLOCK_COMMIT_QC_VIEW_CHANGE_GRACE_MULTIPLIER: u32 = 8;

pub(super) const fn known_block_commit_qc_recovery_request_plan(
    payload_materialized_locally: bool,
) -> KnownBlockCommitQcRecoveryRequestPlan {
    KnownBlockCommitQcRecoveryRequestPlan {
        commit_qc_only: payload_materialized_locally,
        body: !payload_materialized_locally,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializeQcResult {
    Cached,
    Recovered,
    Formed,
    Rebuilt,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MaterializeQcDecision {
    try_form_votes: bool,
    attempts_kura_recovery: bool,
    result: MaterializeQcResult,
    caches_materialized_qc: bool,
}

fn materialize_qc_decision(
    cached_existing: bool,
    empty_roster: bool,
    formed_after_try: bool,
    kura_recovery_available: bool,
    rebuild_from_votes_available: bool,
) -> MaterializeQcDecision {
    let try_form_votes = !cached_existing && !empty_roster;
    let attempts_kura_recovery =
        !cached_existing && (empty_roster || (!formed_after_try && kura_recovery_available));
    let result = if cached_existing {
        MaterializeQcResult::Cached
    } else if attempts_kura_recovery && kura_recovery_available {
        MaterializeQcResult::Recovered
    } else if formed_after_try {
        MaterializeQcResult::Formed
    } else if rebuild_from_votes_available {
        MaterializeQcResult::Rebuilt
    } else {
        MaterializeQcResult::None
    };
    MaterializeQcDecision {
        try_form_votes,
        attempts_kura_recovery,
        result,
        caches_materialized_qc: matches!(
            result,
            MaterializeQcResult::Recovered
                | MaterializeQcResult::Formed
                | MaterializeQcResult::Rebuilt
        ),
    }
}

fn pending_allows_stale_view_commit_qc_fetch(
    pending: &PendingBlock,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    tip_height: usize,
    tip_hash: Option<HashOf<BlockHeader>>,
) -> bool {
    pending.block.hash() == block_hash
        && pending.height == height
        && pending.view == view
        && pending.validation_status != ValidationStatus::Invalid
        && !pending.is_consensus_inactive()
        && pending.local_commit_vote_emitted()
        && pending_extends_tip(
            pending.height,
            pending.block.header().prev_block_hash(),
            tip_height,
            tip_hash,
        )
}

fn sign_vote_with_local_key(
    chain_id: &iroha_data_model::ChainId,
    mode_tag: &str,
    private_key: &iroha_crypto::PrivateKey,
    vote: &mut crate::sumeragi::consensus::Vote,
) -> Result<(), iroha_crypto::Error> {
    vote.bls_sig.clear();
    let preimage = vote_preimage(chain_id, mode_tag, vote);
    let signature = Signature::try_new(private_key, &preimage)?;
    vote.bls_sig = signature.payload().to_vec();
    Ok(())
}

#[derive(Debug)]
pub(super) struct CommitResult {
    pub(super) id: u64,
    pub(super) outcome: CommitOutcome,
    pub(super) timings: CommitStageTimings,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CommitStageTimings {
    pub(super) qc_verify_ms: Option<u64>,
    pub(super) persist_ms: Option<u64>,
    pub(super) kura_store_ms: Option<u64>,
    pub(super) state_apply_ms: Option<u64>,
    pub(super) state_commit_ms: Option<u64>,
    pub(super) validation: Option<crate::block::valid::ValidationTimings>,
    pub(super) used_prevalidated_artifact: bool,
}

impl CommitStageTimings {
    fn has_recorded_stages(self) -> bool {
        self.qc_verify_ms.is_some()
            || self.persist_ms.is_some()
            || self.kura_store_ms.is_some()
            || self.state_apply_ms.is_some()
            || self.state_commit_ms.is_some()
            || self.validation.is_some()
            || self.used_prevalidated_artifact
    }

    fn blocking_total_ms(self) -> Option<u64> {
        [self.qc_verify_ms, self.persist_ms]
            .into_iter()
            .flatten()
            .fold(None, |total: Option<u64>, ms| {
                Some(total.unwrap_or_default().saturating_add(ms))
            })
    }

    fn max_observed_stage_ms(self) -> Option<u64> {
        let validation = self.validation;
        [
            self.qc_verify_ms,
            self.persist_ms,
            self.kura_store_ms,
            self.state_apply_ms,
            self.state_commit_ms,
            validation.map(|timings| timings.total_ms),
            validation.map(|timings| timings.stateless_ms),
            validation.map(|timings| timings.execution_ms),
            validation.map(|timings| timings.execution_tx_ms),
            validation.map(|timings| timings.execution_tx_apply_ms),
            validation.map(|timings| timings.execution_tx_finalize_ms),
        ]
        .into_iter()
        .flatten()
        .max()
    }
}

fn trusted_prevalidated_commit_artifact(
    artifact: Option<ValidatedCommitArtifact>,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    commit_qc: Option<&crate::sumeragi::consensus::Qc>,
) -> Option<ValidatedCommitArtifact> {
    let artifact = artifact?;
    if artifact.block_hash != block_hash || artifact.height != height || artifact.view != view {
        return None;
    }
    let commit_qc = commit_qc?;
    (commit_qc.subject_block_hash == block_hash
        && commit_qc.height == height
        && commit_qc.view == view
        && matches!(commit_qc.phase, crate::sumeragi::consensus::Phase::Commit)
        && commit_qc.parent_state_root == artifact.parent_state_root
        && commit_qc.post_state_root == artifact.post_state_root)
        .then_some(artifact)
}

fn prevalidated_roots_match_witness(
    artifact: ValidatedCommitArtifact,
    witness: Option<&ExecWitness>,
) -> bool {
    witness.is_some_and(|witness| {
        parent_state_from_witness(witness) == artifact.parent_state_root
            && post_state_from_witness(witness) == artifact.post_state_root
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CommitDrainSummary {
    pub(super) progress: bool,
    pub(super) results: u64,
    pub(super) qc_verify_ms: u64,
    pub(super) persist_ms: u64,
    pub(super) kura_store_ms: u64,
    pub(super) state_apply_ms: u64,
    pub(super) state_commit_ms: u64,
}

impl CommitDrainSummary {
    fn record(&mut self, timings: CommitStageTimings) {
        self.results = self.results.saturating_add(1);
        if let Some(value) = timings.qc_verify_ms {
            self.qc_verify_ms = self.qc_verify_ms.saturating_add(value);
        }
        if let Some(value) = timings.persist_ms {
            self.persist_ms = self.persist_ms.saturating_add(value);
        }
        if let Some(value) = timings.kura_store_ms {
            self.kura_store_ms = self.kura_store_ms.saturating_add(value);
        }
        if let Some(value) = timings.state_apply_ms {
            self.state_apply_ms = self.state_apply_ms.saturating_add(value);
        }
        if let Some(value) = timings.state_commit_ms {
            self.state_commit_ms = self.state_commit_ms.saturating_add(value);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct CommitPostApplySnapshot {
    pub(super) world_peers: Vec<PeerId>,
    pub(super) stake_snapshot: Option<crate::sumeragi::stake_snapshot::CommitStakeSnapshot>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CommitPipelineTimings {
    pub(super) ran: bool,
    pub(super) total: Duration,
    pub(super) drain_results: Duration,
    pub(super) drain_result_count: u64,
    pub(super) drain_qc_verify_ms: u64,
    pub(super) drain_persist_ms: u64,
    pub(super) drain_kura_store_ms: u64,
    pub(super) drain_state_apply_ms: u64,
    pub(super) drain_state_commit_ms: u64,
    pub(super) abort_inflight: Duration,
    pub(super) event_reschedule: Duration,
    pub(super) qc_rebuild: Duration,
    pub(super) validation: Duration,
    pub(super) gate: Duration,
    pub(super) finalize: Duration,
    pub(super) blocks_considered: u64,
    pub(super) blocks_processed: u64,
}

impl CommitPipelineTimings {
    fn finish(mut self, start: Instant) -> Self {
        self.total = start.elapsed();
        self
    }
}

fn duration_to_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn commit_stage_timings_exceed_threshold(timings: CommitStageTimings, threshold: Duration) -> bool {
    if threshold.is_zero() {
        return false;
    }
    let threshold_ms = duration_to_ms(threshold);
    timings
        .blocking_total_ms()
        .is_some_and(|total| total >= threshold_ms)
        || timings
            .max_observed_stage_ms()
            .is_some_and(|stage| stage >= threshold_ms)
}

fn commit_pipeline_sample_from_timings(
    timings: CommitPipelineTimings,
) -> crate::sumeragi::status::CommitPipelineSample {
    crate::sumeragi::status::CommitPipelineSample {
        total_ms: duration_to_ms(timings.total),
        validation_ms: duration_to_ms(timings.validation),
        qc_rebuild_ms: duration_to_ms(timings.qc_rebuild),
        gate_ms: duration_to_ms(timings.gate),
        finalize_ms: duration_to_ms(timings.finalize),
        drain_results_ms: duration_to_ms(timings.drain_results),
        drain_qc_verify_ms: timings.drain_qc_verify_ms,
        drain_persist_ms: timings.drain_persist_ms,
        drain_kura_store_ms: timings.drain_kura_store_ms,
        drain_state_apply_ms: timings.drain_state_apply_ms,
        drain_state_commit_ms: timings.drain_state_commit_ms,
    }
}

fn autoscale_transition_committed_at(
    nexus: &iroha_config::parameters::actual::Nexus,
    committed_height: u64,
) -> bool {
    nexus.autoscale.enabled && nexus.autoscale.last_transition_height == committed_height
}

#[derive(Debug)]
pub(super) enum CommitOutcome {
    Rejected {
        failed_block: SignedBlock,
        error: BlockValidationError,
        pipeline_events: Vec<PipelineEventBox>,
    },
    #[allow(dead_code)]
    KuraStoreFailed {
        committed_block: crate::block::CommittedBlock,
        error: crate::kura::Error,
    },
    StateCommitFailed {
        committed_block: crate::block::CommittedBlock,
        error: String,
        error_kind: Option<crate::state::storage_transactions::TransactionsBlockError>,
    },
    Success {
        committed_block: crate::block::CommittedBlock,
        exec_witness: Option<ExecWitness>,
        fastpq_witness_context: Option<crate::fastpq::FastpqWitnessContext>,
        pipeline_events: Vec<PipelineEventBox>,
        state_events: Vec<EventBox>,
        post_apply_snapshot: CommitPostApplySnapshot,
        post_commit_persistence_error: Option<String>,
    },
}

#[derive(Debug)]
#[allow(dead_code)] // Spawned from unit-test-only commit worker harnesses.
pub(super) struct CommitWorkerHandle {
    pub(super) work_tx: mpsc::SyncSender<CommitWork>,
    pub(super) result_rx: mpsc::Receiver<CommitResult>,
    pub(super) join_handle: std::thread::JoinHandle<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CommitQuorumStatus {
    pub vote_count: usize,
    pub quorum_reached: bool,
    pub stake_quorum_missing: bool,
}

pub(super) fn p2p_topology_with_trusted(
    world_peers: &BTreeSet<PeerId>,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
) -> BTreeSet<PeerId> {
    let mut topology = world_peers.clone();
    topology.insert(trusted.myself.id().clone());
    topology.extend(trusted.others.iter().map(|peer| peer.id().clone()));
    topology
}

fn peer_ids_outside_topology(
    expected_topology: &BTreeSet<PeerId>,
    online_peer_ids: &[PeerId],
) -> Vec<PeerId> {
    online_peer_ids
        .iter()
        .filter(|peer_id| !expected_topology.contains(*peer_id))
        .cloned()
        .collect()
}

#[allow(dead_code)] // Spawned from unit-test-only commit worker harnesses.
pub(super) fn spawn_commit_worker(
    state: Arc<State>,
    kura: Arc<Kura>,
    chain_id: ChainId,
    genesis_account: AccountId,
    wake_tx: Option<mpsc::SyncSender<()>>,
    work_queue_cap: usize,
    result_queue_cap: usize,
) -> io::Result<CommitWorkerHandle> {
    let work_queue_cap = work_queue_cap.max(1);
    let result_queue_cap = result_queue_cap.max(1);
    let (work_tx, work_rx) = mpsc::sync_channel::<CommitWork>(work_queue_cap);
    let (result_tx, result_rx) = mpsc::sync_channel::<CommitResult>(result_queue_cap);
    let spawn_result =
        crate::sumeragi::sumeragi_thread_builder("sumeragi-commit").spawn(move || {
            while let Ok(work) = work_rx.recv() {
                let id = work.id;
                let (outcome, timings) = execute_commit_work(
                    state.as_ref(),
                    kura.as_ref(),
                    &chain_id,
                    &genesis_account,
                    work,
                );
                let mut result = CommitResult {
                    id,
                    outcome,
                    timings,
                };
                loop {
                    match result_tx.try_send(result) {
                        Ok(()) => break,
                        Err(mpsc::TrySendError::Full(pending)) => {
                            // Nudge the main loop to drain results before retrying.
                            if let Some(wake) = wake_tx.as_ref() {
                                let _ = wake.try_send(());
                            }
                            result = pending;
                            std::thread::yield_now();
                        }
                        Err(mpsc::TrySendError::Disconnected(_)) => return,
                    }
                }
                if let Some(wake) = wake_tx.as_ref() {
                    let _ = wake.try_send(());
                }
            }
        });
    finish_commit_worker_spawn(work_tx, result_rx, spawn_result)
}

fn finish_commit_worker_spawn(
    work_tx: mpsc::SyncSender<CommitWork>,
    result_rx: mpsc::Receiver<CommitResult>,
    spawn_result: io::Result<std::thread::JoinHandle<()>>,
) -> io::Result<CommitWorkerHandle> {
    spawn_result.map(|join_handle| CommitWorkerHandle {
        work_tx,
        result_rx,
        join_handle,
    })
}

pub(super) fn execute_commit_work(
    state: &State,
    kura: &Kura,
    chain_id: &ChainId,
    genesis_account: &AccountId,
    work: CommitWork,
) -> (CommitOutcome, CommitStageTimings) {
    let CommitWork {
        block,
        id,
        validated_commit_artifact,
        commit_topology,
        signature_topology,
        consensus_mode,
        qc_signers: _qc_signers,
        commit_qc,
        allow_signature_index_recovery,
        events_sender: _events_sender,
        ..
    } = work;
    let mut timings = CommitStageTimings::default();
    let mut pipeline_events: Vec<PipelineEventBox> = Vec::new();
    let time_source = TimeSource::new_system();
    let mut voting_block = None;
    let topology = super::network_topology::Topology::new(signature_topology);
    let block_hash = block.hash();
    let header = block.header();
    let block_height = header.height().get();
    let block_view = header.view_change_index();
    let to_ms =
        |duration: std::time::Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    let log_stage_start = |stage: &'static str| {
        debug!(
            commit_id = id,
            height = block_height,
            view = block_view,
            block = %block_hash,
            stage,
            "commit stage start"
        );
    };
    let log_stage_end = |stage: &'static str, start: Instant| {
        debug!(
            commit_id = id,
            height = block_height,
            view = block_view,
            block = %block_hash,
            stage,
            elapsed_ms = to_ms(start.elapsed()),
            "commit stage end"
        );
    };
    debug!(
        commit_id = id,
        height = block_height,
        view = block_view,
        block = %block_hash,
        "commit work start"
    );
    let prevalidated_artifact = trusted_prevalidated_commit_artifact(
        validated_commit_artifact,
        block_hash,
        block_height,
        block_view,
        commit_qc.as_ref(),
    );
    let qc_start = Instant::now();
    log_stage_start("validate_block");
    let validate_block =
        |candidate: SignedBlock,
         candidate_topology: &super::network_topology::Topology,
         voting_block: &mut Option<crate::sumeragi::VotingBlock>,
         pipeline_events: &mut Vec<PipelineEventBox>,
         validation_timings: &mut crate::block::valid::ValidationTimings| {
            ValidBlock::validate_keep_voting_block_with_events_and_timing(
                candidate,
                candidate_topology,
                chain_id,
                genesis_account,
                &time_source,
                state,
                voting_block,
                false,
                validation_timings,
                |event| pipeline_events.push(event),
            )
            .unpack(|event| pipeline_events.push(event))
        };
    let mut validation_timings = crate::block::valid::ValidationTimings::new();
    let full_validation_block = prevalidated_artifact.map(|_| block.clone());
    let mut result = if prevalidated_artifact.is_some() {
        ValidBlock::validate_prevalidated_commit_keep_voting_block_with_events_and_timing(
            block,
            &topology,
            chain_id,
            genesis_account,
            &time_source,
            state,
            &mut voting_block,
            &mut validation_timings,
            |event| pipeline_events.push(event),
        )
        .unpack(|event| pipeline_events.push(event))
    } else {
        validate_block(
            block,
            &topology,
            &mut voting_block,
            &mut pipeline_events,
            &mut validation_timings,
        )
    };
    let mut used_prevalidated_artifact = prevalidated_artifact.is_some() && result.is_ok();
    if prevalidated_artifact.is_some() && result.is_err() {
        warn!(
            commit_id = id,
            height = block_height,
            view = block_view,
            block = %block_hash,
            "prevalidated commit execution rejected; retrying full commit validation"
        );
        pipeline_events.clear();
        voting_block = None;
        validation_timings = crate::block::valid::ValidationTimings::new();
        let retry_block = full_validation_block.or_else(|| {
            result
                .as_ref()
                .err()
                .map(|(failed_block, _)| (**failed_block).clone())
        });
        if let Some(retry_block) = retry_block {
            result = validate_block(
                retry_block,
                &topology,
                &mut voting_block,
                &mut pipeline_events,
                &mut validation_timings,
            );
        } else {
            warn!(
                commit_id = id,
                height = block_height,
                view = block_view,
                block = %block_hash,
                "prevalidated commit retry skipped because no original block was available"
            );
        }
        used_prevalidated_artifact = false;
    }
    let original_failed_block = result
        .as_ref()
        .err()
        .map(|(failed_block, _)| (**failed_block).clone());
    if allow_signature_index_recovery {
        let should_retry = |err: &BlockValidationError| {
            matches!(
                err,
                BlockValidationError::SignatureVerification(
                    crate::block::SignatureVerificationError::UnknownSignature
                        | crate::block::SignatureVerificationError::UnknownSignatory
                        | crate::block::SignatureVerificationError::LeaderMissing
                )
            )
        };

        match result {
            Err((failed_block, err))
                if matches!(
                    *err,
                    BlockValidationError::SignatureVerification(
                        crate::block::SignatureVerificationError::UnknownSignature
                            | crate::block::SignatureVerificationError::UnknownSignatory
                    )
                ) =>
            {
                let mut recovered = *failed_block;
                match remap_block_signature_indices_to_topology(&mut recovered, &topology) {
                    Ok(()) => {
                        warn!(
                            commit_id = id,
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            "retrying commit validation after signature index recovery"
                        );
                        pipeline_events.clear();
                        voting_block = None;
                        validation_timings = crate::block::valid::ValidationTimings::new();
                        result = validate_block(
                            recovered,
                            &topology,
                            &mut voting_block,
                            &mut pipeline_events,
                            &mut validation_timings,
                        );
                    }
                    Err(remap_err) => {
                        result = Err((Box::new(recovered), Box::new(remap_err)));
                    }
                }
            }
            Err((failed_block, err)) => {
                result = Err((failed_block, err));
            }
            Ok(validated) => {
                result = Ok(validated);
            }
        }

        if let (Some(failed_block), Err((_, err))) = (original_failed_block, &result) {
            if should_retry(err.as_ref()) {
                let base_peers = topology.as_ref().to_vec();
                if base_peers.len() > 1 {
                    for offset in 1..base_peers.len() {
                        let mut rotated = base_peers.clone();
                        rotated.rotate_left(offset);
                        let rotated_topology = super::network_topology::Topology::new(rotated);

                        pipeline_events.clear();
                        voting_block = None;
                        validation_timings = crate::block::valid::ValidationTimings::new();
                        let mut attempt = validate_block(
                            failed_block.clone(),
                            &rotated_topology,
                            &mut voting_block,
                            &mut pipeline_events,
                            &mut validation_timings,
                        );
                        let needs_remap = matches!(
                            &attempt,
                            Err((_, rotated_err))
                                if matches!(
                                    **rotated_err,
                                    BlockValidationError::SignatureVerification(
                                        crate::block::SignatureVerificationError::UnknownSignature
                                            | crate::block::SignatureVerificationError::UnknownSignatory
                                    )
                                )
                        );
                        if needs_remap {
                            let Err((failed_rotated, _)) = &attempt else {
                                continue;
                            };
                            let mut remapped = (**failed_rotated).clone();
                            if remap_block_signature_indices_to_topology(
                                &mut remapped,
                                &rotated_topology,
                            )
                            .is_ok()
                            {
                                pipeline_events.clear();
                                voting_block = None;
                                validation_timings = crate::block::valid::ValidationTimings::new();
                                attempt = validate_block(
                                    remapped,
                                    &rotated_topology,
                                    &mut voting_block,
                                    &mut pipeline_events,
                                    &mut validation_timings,
                                );
                            }
                        }

                        if attempt.is_ok() {
                            warn!(
                                commit_id = id,
                                height = block_height,
                                view = block_view,
                                block = %block_hash,
                                offset,
                                "retrying commit validation with rotated signature topology"
                            );
                            result = attempt;
                            break;
                        }
                    }
                }
            }
        }
    }
    log_stage_end("validate_block", qc_start);
    timings.validation = Some(validation_timings);
    timings.used_prevalidated_artifact = used_prevalidated_artifact;
    let result = result.and_then(|(valid_block, mut state_block)| {
        let exec_witness = state_block.take_exec_witness();
        let fastpq_witness_context = state_block.take_fastpq_witness_context();
        if let Some(artifact) = prevalidated_artifact
            && !prevalidated_roots_match_witness(artifact, exec_witness.as_ref())
        {
            warn!(
                commit_id = id,
                height = block_height,
                view = block_view,
                block = %block_hash,
                expected_parent_state_root = %artifact.parent_state_root,
                expected_post_state_root = %artifact.post_state_root,
                actual_parent_state_root = ?exec_witness.as_ref().map(parent_state_from_witness),
                actual_post_state_root = ?exec_witness.as_ref().map(post_state_from_witness),
                "prevalidated commit execution roots do not match validation artifact"
            );
            return Err((
                Box::new(valid_block.into()),
                Box::new(BlockValidationError::ExecutionContextInvalid(
                    "prevalidated commit execution roots mismatch".to_owned(),
                )),
            ));
        }
        log_stage_start("commit_with_certificate");
        let commit_start = Instant::now();
        let commit_result = valid_block.commit_with_certificate();
        let commit_result = commit_result
            .unpack(|event| pipeline_events.push(event))
            .map(|committed_block| {
                (
                    committed_block,
                    state_block,
                    exec_witness,
                    fastpq_witness_context,
                )
            })
            .map_err(|(failed_block, err)| (Box::new((*failed_block).into()), err));
        log_stage_end("commit_with_certificate", commit_start);
        commit_result
    });
    timings.qc_verify_ms = Some(to_ms(qc_start.elapsed()));
    match result {
        Ok((committed_block, mut state_block, exec_witness, fastpq_witness_context)) => {
            let persist_start = Instant::now();
            let pipeline_events = pipeline_events;
            let validated_commit_artifact_for_manifest = validated_commit_artifact.or_else(|| {
                exec_witness
                    .as_ref()
                    .map(|witness| ValidatedCommitArtifact {
                        block_hash,
                        height: block_height,
                        view: block_view,
                        parent_state_root: parent_state_from_witness(witness),
                        post_state_root: post_state_from_witness(witness),
                    })
            });
            let committed_block_for_kura = committed_block.clone();
            log_stage_start("kura_store");
            let kura_start = Instant::now();
            if let Err(err) = kura.store_block(committed_block_for_kura) {
                log_stage_end("kura_store", kura_start);
                timings.kura_store_ms = Some(to_ms(kura_start.elapsed()));
                timings.persist_ms = Some(to_ms(persist_start.elapsed()));
                return (
                    CommitOutcome::KuraStoreFailed {
                        committed_block,
                        error: err,
                    },
                    timings,
                );
            }
            timings.kura_store_ms = Some(to_ms(kura_start.elapsed()));
            log_stage_end("kura_store", kura_start);
            log_stage_start("state_apply");
            let apply_start = Instant::now();
            let stake_snapshot_roster = commit_topology.clone();
            let state_events = state_block.apply_without_execution_with_commit_qc(
                &committed_block,
                commit_topology,
                commit_qc.as_ref(),
            );
            let post_apply_snapshot = {
                let world = state_block.world();
                let world_peers = world.peers().iter().cloned().collect::<Vec<_>>();
                let stake_snapshot = if matches!(consensus_mode, ConsensusMode::Npos) {
                    crate::sumeragi::stake_snapshot::CommitStakeSnapshot::from_roster(
                        world,
                        &stake_snapshot_roster,
                    )
                } else {
                    None
                };
                CommitPostApplySnapshot {
                    world_peers,
                    stake_snapshot,
                }
            };
            timings.state_apply_ms = Some(to_ms(apply_start.elapsed()));
            log_stage_end("state_apply", apply_start);
            log_stage_start("state_commit");
            let state_commit_start = Instant::now();
            if let Err(err) = state_block.commit() {
                log_stage_end("state_commit", state_commit_start);
                timings.state_commit_ms = Some(to_ms(state_commit_start.elapsed()));
                timings.persist_ms = Some(to_ms(persist_start.elapsed()));
                return (
                    CommitOutcome::StateCommitFailed {
                        committed_block,
                        error: err.to_string(),
                        error_kind: Some(err),
                    },
                    timings,
                );
            }
            timings.state_commit_ms = Some(to_ms(state_commit_start.elapsed()));
            log_stage_end("state_commit", state_commit_start);
            let mut post_commit_persistence_error = None;
            let wsv_checkpoint_hash = crate::snapshot::canonical_state_snapshot_hash(state);
            if std::env::var_os("IROHA_DEBUG_WSV_COMPONENTS").is_some() {
                let components = crate::snapshot::canonical_state_snapshot_component_hashes(state);
                let commit_qcs = crate::snapshot::canonical_state_commit_qc_summaries(state);
                warn!(
                    height = block_height,
                    block = %block_hash,
                    checkpoint = %wsv_checkpoint_hash,
                    ?components,
                    ?commit_qcs,
                    "computed committed WSV checkpoint components"
                );
            }
            if let Err(err) =
                kura.store_wsv_checkpoint(block_height, block_hash, wsv_checkpoint_hash)
            {
                post_commit_persistence_error = Some(format!("WSV checkpoint: {err}"));
                error!(
                    ?err,
                    height = block_height,
                    block = %block_hash,
                    checkpoint = %wsv_checkpoint_hash,
                    "failed to persist Kura WSV checkpoint after state commit"
                );
            }
            let (parent_state_root, post_state_root) = validated_commit_artifact_for_manifest
                .map(|artifact| {
                    (
                        Some(artifact.parent_state_root),
                        Some(artifact.post_state_root),
                    )
                })
                .unwrap_or((None, None));
            let commit_qc_hash = commit_qc
                .as_ref()
                .map(|qc| iroha_crypto::Hash::new(qc.encode()));
            let commit_manifest = crate::kura::CommitManifest::new(
                block_height,
                block_hash,
                parent_state_root,
                post_state_root,
                wsv_checkpoint_hash,
                commit_qc_hash,
            );
            if let Err(err) = kura.store_commit_manifest(commit_manifest) {
                post_commit_persistence_error = Some(match post_commit_persistence_error.take() {
                    Some(previous) => format!("{previous}; commit manifest: {err}"),
                    None => format!("commit manifest: {err}"),
                });
                error!(
                    ?err,
                    height = block_height,
                    block = %block_hash,
                    checkpoint = %wsv_checkpoint_hash,
                    "failed to persist Kura commit manifest after state commit"
                );
            }
            crate::sumeragi::status::record_round_gap_state_commit(
                block_height,
                block_view,
                block_hash,
            );
            timings.persist_ms = Some(to_ms(persist_start.elapsed()));
            (
                CommitOutcome::Success {
                    committed_block,
                    exec_witness,
                    fastpq_witness_context,
                    pipeline_events,
                    state_events,
                    post_apply_snapshot,
                    post_commit_persistence_error,
                },
                timings,
            )
        }
        Err((failed_block, err)) => (
            CommitOutcome::Rejected {
                failed_block: *failed_block,
                error: *err,
                pipeline_events,
            },
            timings,
        ),
    }
}

fn execute_commit_work_on_dedicated_stack(
    state: Arc<State>,
    kura: Arc<Kura>,
    chain_id: ChainId,
    genesis_account: AccountId,
    work: CommitWork,
) -> (CommitOutcome, CommitStageTimings) {
    let thread_state = Arc::clone(&state);
    let thread_kura = Arc::clone(&kura);
    let thread_chain_id = chain_id.clone();
    let thread_genesis_account = genesis_account.clone();
    let thread_work = work.clone();
    let spawn_result =
        crate::sumeragi::sumeragi_thread_builder("sumeragi-commit-inline").spawn(move || {
            execute_commit_work(
                thread_state.as_ref(),
                thread_kura.as_ref(),
                &thread_chain_id,
                &thread_genesis_account,
                thread_work,
            )
        });
    finish_dedicated_commit_spawn(work, spawn_result)
}

fn finish_dedicated_commit_spawn(
    work: CommitWork,
    spawn_result: io::Result<std::thread::JoinHandle<(CommitOutcome, CommitStageTimings)>>,
) -> (CommitOutcome, CommitStageTimings) {
    match spawn_result {
        Ok(join_handle) => match join_handle.join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        },
        Err(error) => {
            warn!(
                %error,
                "failed to spawn inline Sumeragi commit thread; rejecting commit work without applying state"
            );
            (
                CommitOutcome::Rejected {
                    failed_block: work.block,
                    error: BlockValidationError::ExecutionContextInvalid(format!(
                        "failed to spawn inline Sumeragi commit thread: {error}"
                    )),
                    pipeline_events: Vec::new(),
                },
                CommitStageTimings::default(),
            )
        }
    }
}

fn has_commit_quorum_signers(
    qc_signers: Option<&BTreeSet<ValidatorIndex>>,
    min_votes_for_commit: usize,
) -> bool {
    qc_signers.is_some_and(|signers| signers.len() >= min_votes_for_commit)
}

fn remap_block_signature_indices_to_topology(
    block: &mut SignedBlock,
    topology: &super::network_topology::Topology,
) -> Result<(), BlockValidationError> {
    use crate::block::SignatureVerificationError;
    use iroha_data_model::block::BlockSignature;

    let block_hash = block.hash();
    let mut remapped: BTreeSet<BlockSignature> = BTreeSet::new();

    for signature in block.signatures() {
        let is_eligible = |peer: &PeerId| {
            crate::sumeragi::is_bls_normal_public_key(peer.public_key())
                && !matches!(
                    topology.role(peer),
                    super::network_topology::Role::Undefined
                )
        };

        let mut resolved: Option<usize> = None;
        let mut ambiguous = false;

        if let Ok(raw_idx) = usize::try_from(signature.index()) {
            if let Some(peer) = topology.as_ref().get(raw_idx) {
                if is_eligible(peer)
                    && signature
                        .signature()
                        .verify_hash(peer.public_key(), block_hash)
                        .is_ok()
                {
                    resolved = Some(raw_idx);
                }
            }
        }

        if resolved.is_none() {
            for (idx, peer) in topology.as_ref().iter().enumerate() {
                if !is_eligible(peer) {
                    continue;
                }
                if signature
                    .signature()
                    .verify_hash(peer.public_key(), block_hash)
                    .is_err()
                {
                    continue;
                }
                if resolved.is_some() {
                    ambiguous = true;
                    break;
                }
                resolved = Some(idx);
            }
        }

        let Some(mapped_idx) = resolved else {
            return Err(BlockValidationError::SignatureVerification(
                SignatureVerificationError::UnknownSignature,
            ));
        };
        if ambiguous {
            return Err(BlockValidationError::SignatureVerification(
                SignatureVerificationError::UnknownSignature,
            ));
        }

        let mapped = BlockSignature::new(mapped_idx as u64, signature.signature().clone());
        if !remapped.insert(mapped) {
            return Err(BlockValidationError::SignatureVerification(
                SignatureVerificationError::DuplicateSignature { signer: mapped_idx },
            ));
        }
    }

    if remapped.is_empty() {
        return Err(BlockValidationError::SignatureVerification(
            SignatureVerificationError::NotEnoughSignatures {
                votes_count: 0,
                min_votes_for_commit: topology.min_votes_for_commit(),
            },
        ));
    }

    block.replace_signatures(remapped).map_err(|_| {
        BlockValidationError::SignatureVerification(SignatureVerificationError::Other)
    })?;
    Ok(())
}

fn commit_qc_from_cache_or_history(
    qc_cache: &BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    mode_tag: &str,
    commit_topology: &[PeerId],
) -> Option<crate::sumeragi::consensus::Qc> {
    if let Some(qc) = cached_qc_for(
        qc_cache,
        crate::sumeragi::consensus::Phase::Commit,
        block_hash,
        height,
        view,
        epoch,
    ) {
        return Some(qc);
    }
    commit_qc_from_history(block_hash, height, view, epoch, mode_tag)
        .filter(|qc| qc.validator_set.as_slice() == commit_topology)
}

fn commit_qc_from_history(
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    mode_tag: &str,
) -> Option<crate::sumeragi::consensus::Qc> {
    super::status::commit_qc_history().into_iter().find(|qc| {
        qc.phase == crate::sumeragi::consensus::Phase::Commit
            && qc.subject_block_hash == block_hash
            && qc.height == height
            && qc.view == view
            && qc.epoch == epoch
            && qc.mode_tag == mode_tag
            && !qc.aggregate.bls_aggregate_signature.is_empty()
    })
}

fn validate_block_sync_update_commit_qc(
    qc: &crate::sumeragi::consensus::Qc,
    state: &State,
    consensus_mode: ConsensusMode,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    aggregate_ok: Option<bool>,
) -> Result<(), super::QcValidationError> {
    let mode_tag = match consensus_mode {
        ConsensusMode::Permissioned => super::PERMISSIONED_TAG,
        ConsensusMode::Npos => super::NPOS_TAG,
    };
    let world = state.world_view();
    let roster_cache =
        super::RosterValidationCache::from_world(&world, super::EPOCH_LENGTH_BLOCKS, None);
    let topology = super::network_topology::Topology::new(qc.validator_set.clone());
    let block_signers = BTreeSet::new();
    let prf_seed = Some(super::prf_seed_for_height_from_world(
        &world,
        state.chain_id_ref(),
        height,
    ));
    for (byte_idx, byte) in qc.aggregate.signers_bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            let Some(peer) = qc.validator_set.get(idx) else {
                continue;
            };
            if !roster_cache.pops.contains_key(peer.public_key()) {
                debug!(
                    height,
                    view,
                    block = %block_hash,
                    signer = idx,
                    "skipping commit QC aggregate gate for block-sync response: signer PoP unavailable"
                );
                return Ok(());
            }
        }
    }
    let resolved_stake_snapshot = if matches!(consensus_mode, ConsensusMode::Npos) {
        stake_snapshot
            .filter(|snapshot| snapshot.matches_roster(&qc.validator_set))
            .cloned()
            .or_else(|| CommitStakeSnapshot::from_roster(&world, &qc.validator_set))
    } else {
        None
    };
    super::validate_block_sync_qc(
        qc,
        &topology,
        &world,
        &block_signers,
        view,
        &roster_cache.pops,
        state.chain_id_ref(),
        consensus_mode,
        resolved_stake_snapshot.as_ref(),
        mode_tag,
        prf_seed,
        aggregate_ok,
    )
    .map(|_| ())
    .map_err(|err| {
        warn!(
            ?err,
            height,
            view,
            block = %block_hash,
            qc_height = qc.height,
            qc_view = qc.view,
            qc_block = %qc.subject_block_hash,
            "dropping invalid commit QC from block-sync response"
        );
        err
    })
}

impl Actor {
    fn promote_commit_anchor_qc(&mut self, qc: crate::sumeragi::consensus::QcHeaderRef) {
        let new_highest = match self.highest_qc {
            Some(current) if (current.height, current.view) > (qc.height, qc.view) => current,
            _ => qc,
        };
        self.highest_qc = Some(new_highest);
        super::status::set_highest_qc(new_highest.height, new_highest.view);
        super::status::set_highest_qc_hash(new_highest.subject_block_hash);

        let previous_lock = self.locked_qc;
        let new_locked = match self.locked_qc {
            Some(current) if (current.height, current.view) >= (qc.height, qc.view) => current,
            _ => qc,
        };
        self.locked_qc = Some(new_locked);
        super::status::set_locked_qc(
            new_locked.height,
            new_locked.view,
            Some(new_locked.subject_block_hash),
        );
        if previous_lock != Some(new_locked) {
            self.prune_precommit_votes_conflicting_with_lock(new_locked);
        }

        if let Some(lock) = self.locked_qc
            && let Some(highest) = self.highest_qc
            && !qc_satisfies_locked_with_lookup(lock, highest, |hash, height| {
                self.parent_hash_for(hash, height)
            })
        {
            info!(
                highest_height = highest.height,
                highest_hash = %highest.subject_block_hash,
                locked_height = lock.height,
                locked_hash = %lock.subject_block_hash,
                "realigning highest QC to locked chain after commit"
            );
            self.highest_qc = Some(lock);
            super::status::set_highest_qc(lock.height, lock.view);
            super::status::set_highest_qc_hash(lock.subject_block_hash);
        }
    }

    /// Attach cached commit certificates and votes for the given block to a `BlockSyncUpdate`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_cached_qcs_to_block_sync_update(
        update: &mut super::message::BlockSyncUpdate,
        qc_cache: &BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
        vote_log: &BTreeMap<votes::VoteLogKey, crate::sumeragi::consensus::Vote>,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        state: &State,
        fallback_consensus_mode: ConsensusMode,
    ) {
        let world = state.world_view();
        let consensus_mode = super::effective_consensus_mode_for_height_from_world(
            &world,
            height,
            fallback_consensus_mode,
        );
        let initial_commit_qc_present = update.commit_qc.is_some();
        if update.commit_qc.is_none() {
            let mode_tag = match consensus_mode {
                ConsensusMode::Permissioned => super::PERMISSIONED_TAG,
                ConsensusMode::Npos => super::NPOS_TAG,
            };
            let commit_topology = update
                .validator_checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.validator_set.clone())
                .or_else(|| {
                    state
                        .commit_roster_snapshot_for_block(height, block_hash)
                        .map(|snapshot| snapshot.commit_qc.validator_set)
                });
            update.commit_qc = commit_topology
                .as_ref()
                .and_then(|topology| {
                    commit_qc_from_cache_or_history(
                        qc_cache, block_hash, height, view, epoch, mode_tag, topology,
                    )
                })
                .or_else(|| {
                    cached_qc_for(
                        qc_cache,
                        crate::sumeragi::consensus::Phase::Commit,
                        block_hash,
                        height,
                        view,
                        epoch,
                    )
                });
        }
        if update.commit_qc.is_none() {
            if let Some(record) = crate::sumeragi::status::precommit_signers_for_round(
                block_hash, height, view, epoch,
            ) {
                if let Some(derived) = super::derive_block_sync_qc_from_signers(
                    block_hash,
                    height,
                    view,
                    record.epoch,
                    record.chain_order_hash,
                    record.rechain_seq,
                    record.parent_state_root,
                    record.post_state_root,
                    &record.validator_set,
                    consensus_mode,
                    record.stake_snapshot.as_ref(),
                    &record.mode_tag,
                    &record.signers,
                    record.bls_aggregate_signature.clone(),
                ) {
                    update.commit_qc = Some(derived);
                    if update.stake_snapshot.is_none() {
                        update.stake_snapshot.clone_from(&record.stake_snapshot);
                    }
                }
            }
        }
        if !initial_commit_qc_present && let Some(qc) = update.commit_qc.take() {
            let cache_match = cached_qc_for(
                qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                height,
                view,
                epoch,
            )
            .is_some_and(|cached| HashOf::new(&cached) == HashOf::new(&qc));
            let aggregate_ok = cache_match.then_some(true);
            if validate_block_sync_update_commit_qc(
                &qc,
                state,
                consensus_mode,
                block_hash,
                height,
                view,
                update.stake_snapshot.as_ref(),
                aggregate_ok,
            )
            .is_ok()
            {
                update.commit_qc = Some(qc);
            }
        }
        if !initial_commit_qc_present && update.commit_qc.is_none() {
            update.commit_qc = crate::block_sync::BlockSynchronizer::block_sync_qc_for_world(
                &world,
                fallback_consensus_mode,
                &update.block,
            );
        }
        if !initial_commit_qc_present && let Some(qc) = update.commit_qc.take() {
            if validate_block_sync_update_commit_qc(
                &qc,
                state,
                consensus_mode,
                block_hash,
                height,
                view,
                update.stake_snapshot.as_ref(),
                None,
            )
            .is_ok()
            {
                update.commit_qc = Some(qc);
            }
        }
        if update.validator_checkpoint.is_none()
            && let Some(qc) = update.commit_qc.as_ref()
        {
            update.validator_checkpoint = Some(ValidatorSetCheckpoint::new_with_chain_order(
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
            ));
        }
        if matches!(consensus_mode, ConsensusMode::Npos)
            && (update.commit_qc.is_some() || update.validator_checkpoint.is_some())
        {
            let roster = update
                .commit_qc
                .as_ref()
                .map(|qc| qc.validator_set.as_slice())
                .or_else(|| {
                    update
                        .validator_checkpoint
                        .as_ref()
                        .map(|chk| chk.validator_set.as_slice())
                });
            if let Some(roster) = roster {
                let matches = update
                    .stake_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.matches_roster(roster));
                if !matches {
                    let world = state.world_view();
                    update.stake_snapshot = CommitStakeSnapshot::from_roster(&world, roster);
                }
            }
        }
        if update.commit_votes.is_empty() {
            let votes: Vec<_> = vote_log
                .values()
                .filter(|vote| {
                    vote.phase == crate::sumeragi::consensus::Phase::Commit
                        && vote.block_hash == block_hash
                        && vote.height == height
                        && vote.view == view
                        && vote.epoch == epoch
                })
                .cloned()
                .collect();
            if !votes.is_empty() {
                update.commit_votes = votes;
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn precommit_signer_record_from_cached_qc(
        qc: &crate::sumeragi::consensus::Qc,
        commit_topology: &[PeerId],
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<crate::sumeragi::stake_snapshot::CommitStakeSnapshot>,
    ) -> Option<crate::sumeragi::status::PrecommitSignerRecord> {
        if commit_topology.is_empty() {
            warn!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                "skipping precommit signer record: empty commit topology"
            );
            return None;
        }
        let roster_len = commit_topology.len();
        let parsed = match super::qc_signer_indices(qc, roster_len, roster_len) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    ?err,
                    height = qc.height,
                    view = qc.view,
                    block = %qc.subject_block_hash,
                    roster_len,
                    "skipping precommit signer record: invalid cached QC bitmap"
                );
                return None;
            }
        };
        let aggregate_signature = qc.aggregate.bls_aggregate_signature.clone();
        if aggregate_signature.is_empty() {
            warn!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                "skipping precommit signer record: cached QC missing aggregate signature"
            );
            return None;
        }
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => stake_snapshot,
        };
        match consensus_mode {
            ConsensusMode::Permissioned => {
                let required = super::network_topology::commit_quorum_from_len(roster_len).max(1);
                if parsed.voting.len() < required {
                    warn!(
                        height = qc.height,
                        view = qc.view,
                        block = %qc.subject_block_hash,
                        signers = parsed.voting.len(),
                        required,
                        "skipping precommit signer record: cached QC below commit quorum"
                    );
                    return None;
                }
            }
            ConsensusMode::Npos => {
                let snapshot = stake_snapshot.as_ref()?;
                let mut signer_peers = BTreeSet::new();
                for signer in &parsed.voting {
                    let Ok(idx) = usize::try_from(*signer) else {
                        return None;
                    };
                    let peer = commit_topology.get(idx)?;
                    signer_peers.insert(peer.clone());
                }
                match super::stake_snapshot::stake_quorum_reached_for_snapshot(
                    snapshot,
                    commit_topology,
                    &signer_peers,
                ) {
                    Ok(true) => {}
                    Ok(false) => {
                        warn!(
                            height = qc.height,
                            view = qc.view,
                            block = %qc.subject_block_hash,
                            signers = parsed.voting.len(),
                            "skipping precommit signer record: cached QC below stake quorum"
                        );
                        return None;
                    }
                    Err(_) => {
                        warn!(
                            height = qc.height,
                            view = qc.view,
                            block = %qc.subject_block_hash,
                            signers = parsed.voting.len(),
                            "skipping precommit signer record: stake snapshot unavailable"
                        );
                        return None;
                    }
                }
            }
        }
        Some(crate::sumeragi::status::PrecommitSignerRecord {
            block_hash: qc.subject_block_hash,
            height: qc.height,
            view: qc.view,
            epoch: qc.epoch,
            chain_order_hash: qc.chain_order_hash,
            rechain_seq: qc.rechain_seq,
            parent_state_root: qc.parent_state_root,
            post_state_root: qc.post_state_root,
            signers: parsed.voting,
            bls_aggregate_signature: aggregate_signature,
            roster_len,
            mode_tag: qc.mode_tag.clone(),
            validator_set: commit_topology.to_vec(),
            stake_snapshot,
        })
    }

    fn clear_commit_worker_state(&mut self) {
        self.subsystems.commit.work_tx = None;
        self.subsystems.commit.result_rx = None;
    }

    fn warn_commit_worker_disconnected_once(&mut self, message: &'static str) {
        if self.subsystems.commit.worker_disconnect_logged {
            return;
        }
        self.subsystems.commit.worker_disconnect_logged = true;
        warn!("{message}");
    }

    pub(super) fn retire_committed_commit_inflight(&mut self, context: &'static str) -> bool {
        let Some(inflight) = self.subsystems.commit.inflight.as_ref() else {
            return false;
        };
        let height = inflight.pending.height;
        if height > self.committed_height_snapshot() {
            return false;
        }
        if self.committed_block_hash_for_height(height) != Some(inflight.block_hash) {
            return false;
        }
        let Some(inflight) = self.subsystems.commit.inflight.take() else {
            return false;
        };
        let block_hash = inflight.block_hash;
        let view = inflight.pending.view;
        let commit_id = inflight.id;
        super::status::record_commit_inflight_finish(commit_id);
        if self
            .pending
            .pending_processing
            .get()
            .is_some_and(|hash| hash == block_hash)
        {
            self.pending.pending_processing.set(None);
            self.pending.pending_processing_parent.set(None);
        }
        self.pending.pending_blocks.remove(&block_hash);
        self.clear_validation_ownership_for_block(block_hash);
        self.clean_rbc_sessions_for_committed_block_if_settled(block_hash, height);
        info!(
            height,
            view,
            block = %block_hash,
            commit_id,
            context,
            "retired commit inflight after committed state catch-up"
        );
        true
    }

    fn execute_commit_job_inline(&mut self, inflight: CommitInFlight, work: CommitWork) -> bool {
        super::status::record_commit_inflight_start(
            inflight.id,
            inflight.pending.height,
            inflight.pending.view,
            inflight.block_hash,
        );
        self.subsystems.commit.inflight = Some(inflight);
        let (outcome, timings) = execute_commit_work_on_dedicated_stack(
            Arc::clone(&self.state),
            Arc::clone(&self.kura),
            self.common_config.chain.clone(),
            self.genesis_account.clone(),
            work,
        );
        let Some(inflight) = self.subsystems.commit.inflight.take() else {
            warn!("inline commit finished without an inflight marker; leaving outcome unapplied");
            return false;
        };
        let committed = self.apply_commit_outcome(inflight, outcome, timings);
        if committed {
            let _ = self.kickstart_pacemaker_after_durable_commit();
        }
        committed
    }

    pub(super) fn drain_commit_results(&mut self) -> CommitDrainSummary {
        let mut summary = CommitDrainSummary::default();
        while let Some(recv_result) = self
            .subsystems
            .commit
            .result_rx
            .as_ref()
            .map(mpsc::Receiver::try_recv)
        {
            match recv_result {
                Ok(result) => {
                    let CommitResult {
                        id,
                        outcome,
                        timings,
                    } = result;
                    let inflight = match self.subsystems.commit.inflight.take() {
                        Some(inflight) if inflight.id == id => inflight,
                        Some(inflight) => {
                            warn!(
                                result_id = id,
                                inflight_id = inflight.id,
                                inflight_hash = %inflight.block_hash,
                                "commit result id mismatch; ignoring"
                            );
                            self.subsystems.commit.inflight = Some(inflight);
                            continue;
                        }
                        None => {
                            warn!(
                                result_id = id,
                                "commit result received without inflight; ignoring"
                            );
                            continue;
                        }
                    };
                    let committed = self.apply_commit_outcome(inflight, outcome, timings);
                    summary.record(timings);
                    summary.progress = true;
                    if committed {
                        let _ = self.kickstart_pacemaker_after_durable_commit();
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.warn_commit_worker_disconnected_once(
                        "commit result channel closed; falling back to inline commit",
                    );
                    self.clear_commit_worker_state();
                    if let Some(inflight) = self.subsystems.commit.inflight.take() {
                        let local_outside_commit_topology = inflight
                            .commit_topology
                            .iter()
                            .all(|peer| peer != self.common_config.peer.id());
                        let allow_signature_index_recovery =
                            local_outside_commit_topology && inflight.commit_qc.is_some();
                        let work = CommitWork {
                            id: inflight.id,
                            block: inflight.pending.block.clone(),
                            validated_commit_artifact: inflight.pending.validated_commit_artifact,
                            commit_topology: inflight.commit_topology.clone(),
                            signature_topology: inflight.signature_topology.clone(),
                            consensus_mode: self
                                .consensus_context_for_height(inflight.pending.height)
                                .0,
                            qc_signers: inflight.qc_signers.clone(),
                            commit_qc: inflight.commit_qc.clone(),
                            allow_signature_index_recovery,
                            events_sender: self.events_sender.clone(),
                        };
                        let (outcome, timings) = execute_commit_work_on_dedicated_stack(
                            Arc::clone(&self.state),
                            Arc::clone(&self.kura),
                            self.common_config.chain.clone(),
                            self.genesis_account.clone(),
                            work,
                        );
                        let committed = self.apply_commit_outcome(inflight, outcome, timings);
                        summary.record(timings);
                        summary.progress = true;
                        if committed {
                            let _ = self.kickstart_pacemaker_after_durable_commit();
                        }
                    }
                    break;
                }
            }
        }
        if self.retire_committed_commit_inflight("drain_commit_results") {
            summary.progress = true;
        }
        summary
    }

    pub(super) fn start_commit_job(&mut self, inflight: CommitInFlight, work: CommitWork) -> bool {
        let pending_height = inflight.pending.height;
        let pending_view = inflight.pending.view;
        let block_hash = inflight.block_hash;
        let _ = self.retire_committed_commit_inflight("start_commit_job");
        if self.subsystems.commit.inflight.is_some() {
            if self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|current| current.block_hash == block_hash)
            {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    "commit already in flight; skipping finalize"
                );
                return false;
            }
            self.pending
                .pending_blocks
                .insert(block_hash, inflight.pending);
            return false;
        }

        let Some(worker_tx) = self.subsystems.commit.work_tx.clone() else {
            return self.execute_commit_job_inline(inflight, work);
        };
        if self.subsystems.commit.result_rx.is_none() {
            self.warn_commit_worker_disconnected_once(
                "commit worker result channel missing; falling back to inline commit",
            );
            self.clear_commit_worker_state();
            return self.execute_commit_job_inline(inflight, work);
        }

        match worker_tx.try_send(work) {
            Ok(()) => {
                super::status::record_commit_inflight_start(
                    inflight.id,
                    pending_height,
                    pending_view,
                    block_hash,
                );
                self.subsystems.commit.inflight = Some(inflight);
                true
            }
            Err(mpsc::TrySendError::Full(_work)) => {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    "commit worker queue full; keeping pending block queued"
                );
                self.pending
                    .pending_blocks
                    .insert(block_hash, inflight.pending);
                false
            }
            Err(mpsc::TrySendError::Disconnected(work)) => {
                self.warn_commit_worker_disconnected_once(
                    "commit worker channel disconnected; falling back to inline commit",
                );
                self.clear_commit_worker_state();
                self.execute_commit_job_inline(inflight, work)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply_commit_outcome(
        &mut self,
        inflight: CommitInFlight,
        outcome: CommitOutcome,
        timings: CommitStageTimings,
    ) -> bool {
        super::status::record_commit_inflight_finish(inflight.id);
        #[cfg(not(feature = "telemetry"))]
        let _ = timings;
        let CommitInFlight {
            lock,
            block_hash,
            pending,
            commit_topology,
            signature_topology,
            qc_signers,
            commit_qc,
            post_commit_qc,
            ..
        } = inflight;
        let pending_height = pending.height;
        let pending_view = pending.view;
        let pending_tx_count = pending.block.external_transactions().len();
        let now = Instant::now();
        let da_enabled = self.runtime_da_enabled();
        let mut block_hash_to_clean = None;
        let mut exec_witness_to_emit: Option<(
            ExecWitness,
            Option<crate::fastpq::FastpqWitnessContext>,
        )> = None;
        let mut parent_to_cleanup: Option<HashOf<BlockHeader>> = None;
        let mut reschedule_quorum: Option<(
            PendingBlock,
            Duration,
            Duration,
            usize,
            usize,
            Duration,
        )> = None;
        let mut committed = false;
        let mut committed_pending_tail: Option<PendingBlock> = None;
        let mut committed_block_tail: Option<crate::block::CommittedBlock> = None;
        let mut committed_cached_qc_tail: Option<crate::sumeragi::consensus::Qc> = None;
        let mut committed_pipeline_events_tail: Vec<PipelineEventBox> = Vec::new();
        let mut committed_state_events_tail: Vec<EventBox> = Vec::new();
        let mut committed_post_apply_snapshot_tail: Option<CommitPostApplySnapshot> = None;
        let mut committed_consensus_mode_tail: Option<ConsensusMode> = None;
        let mut committed_mode_tag_tail: Option<&'static str> = None;
        let mut pending_previously_marked_kura_persisted = false;

        let topology = super::network_topology::Topology::new(signature_topology.clone());
        let canonical_topology = super::network_topology::Topology::new(commit_topology.clone());
        let min_votes_for_commit = topology.min_votes_for_commit();
        let quorum_signer_count = qc_signers.as_ref().map(BTreeSet::len);
        let has_quorum_signers =
            has_commit_quorum_signers(qc_signers.as_ref(), min_votes_for_commit);
        let view_signers = qc_signers.as_ref().and_then(|signers| {
            let mapped =
                super::normalize_signer_indices_to_view(signers, &topology, &canonical_topology);
            if mapped.len() == signers.len() {
                Some(mapped)
            } else {
                warn!(
                    height = pending_height,
                    view = pending_view,
                    signers = signers.len(),
                    view_signers = mapped.len(),
                    "skipping vote aggregation: signer mapping to view topology incomplete"
                );
                None
            }
        });

        let mut pending_opt = Some(pending);
        macro_rules! take_pending_or_return {
            () => {
                match pending_opt.take() {
                    Some(pending) => pending,
                    None => {
                        warn!(
                            height = pending_height,
                            view = pending_view,
                            block = %block_hash,
                            "commit outcome branch had no pending block left; leaving outcome unapplied"
                        );
                        return false;
                    }
                }
            };
        }

        #[cfg(feature = "telemetry")]
        {
            if let Some(ms) = timings.qc_verify_ms {
                self.telemetry
                    .observe_commit_stage_ms(crate::telemetry::CommitStage::QcVerify, ms);
            }
            if let Some(ms) = timings.kura_store_ms {
                self.telemetry
                    .observe_commit_stage_ms(crate::telemetry::CommitStage::KuraStore, ms);
            }
            if let Some(ms) = timings.state_apply_ms {
                self.telemetry
                    .observe_commit_stage_ms(crate::telemetry::CommitStage::StateApply, ms);
            }
            if let Some(ms) = timings.state_commit_ms {
                self.telemetry
                    .observe_commit_stage_ms(crate::telemetry::CommitStage::StateCommit, ms);
            }
            if let Some(ms) = timings.persist_ms {
                self.telemetry
                    .observe_commit_stage_ms(crate::telemetry::CommitStage::Persist, ms);
            }
        }
        if timings.has_recorded_stages() {
            let blocking_total_ms = timings.blocking_total_ms();
            let max_observed_stage_ms = timings.max_observed_stage_ms();
            let validation = timings.validation;
            if commit_stage_timings_exceed_threshold(
                timings,
                self.config.persistence.commit_inflight_timeout,
            ) {
                info!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    tx_count = pending_tx_count,
                    blocking_total_ms = ?blocking_total_ms,
                    max_observed_stage_ms = ?max_observed_stage_ms,
                    qc_verify_ms = ?timings.qc_verify_ms,
                    kura_store_ms = ?timings.kura_store_ms,
                    state_apply_ms = ?timings.state_apply_ms,
                    state_commit_ms = ?timings.state_commit_ms,
                    persist_ms = ?timings.persist_ms,
                    validation_total_ms = validation.map(|timings| timings.total_ms),
                    validation_stateless_ms = validation.map(|timings| timings.stateless_ms),
                    validation_execution_ms = validation.map(|timings| timings.execution_ms),
                    validation_execution_tx_ms =
                        validation.map(|timings| timings.execution_tx_ms),
                    validation_execution_tx_signature_batch_ms =
                        validation.map(|timings| timings.execution_tx_signature_batch_ms),
                    validation_execution_tx_stateless_ms =
                        validation.map(|timings| timings.execution_tx_stateless_ms),
                    validation_execution_tx_access_ms =
                        validation.map(|timings| timings.execution_tx_access_ms),
                    validation_execution_tx_overlay_ms =
                        validation.map(|timings| timings.execution_tx_overlay_ms),
                    validation_execution_tx_dag_ms =
                        validation.map(|timings| timings.execution_tx_dag_ms),
                    validation_execution_tx_schedule_ms =
                        validation.map(|timings| timings.execution_tx_schedule_ms),
                    validation_execution_tx_apply_ms =
                        validation.map(|timings| timings.execution_tx_apply_ms),
                    validation_execution_tx_apply_setup_ms =
                        validation.map(|timings| timings.execution_tx_apply_setup_ms),
                    validation_execution_tx_apply_layer_build_ms =
                        validation.map(|timings| timings.execution_tx_apply_layer_build_ms),
                    validation_execution_tx_apply_prep_ms =
                        validation.map(|timings| timings.execution_tx_apply_prep_ms),
                    validation_execution_tx_apply_detached_ms =
                        validation.map(|timings| timings.execution_tx_apply_detached_ms),
                    validation_execution_tx_apply_merge_ms =
                        validation.map(|timings| timings.execution_tx_apply_merge_ms),
                    validation_execution_tx_apply_fallback_ms =
                        validation.map(|timings| timings.execution_tx_apply_fallback_ms),
                    validation_execution_tx_apply_quarantine_ms =
                        validation.map(|timings| timings.execution_tx_apply_quarantine_ms),
                    validation_execution_tx_apply_sequential_ms =
                        validation.map(|timings| timings.execution_tx_apply_sequential_ms),
                    validation_execution_tx_apply_results_ms =
                        validation.map(|timings| timings.execution_tx_apply_results_ms),
                    validation_execution_tx_apply_other_ms =
                        validation.map(|timings| timings.execution_tx_apply_other_ms),
                    validation_execution_tx_time_triggers_ms =
                        validation.map(|timings| timings.execution_tx_time_triggers_ms),
                    validation_execution_tx_finalize_ms =
                        validation.map(|timings| timings.execution_tx_finalize_ms),
                    validation_execution_tx_finalize_digest_submit_ms =
                        validation.map(|timings| timings.execution_tx_finalize_digest_submit_ms),
                    validation_execution_tx_finalize_dataspaces_ms =
                        validation.map(|timings| timings.execution_tx_finalize_dataspaces_ms),
                    validation_execution_tx_finalize_tx_set_ms =
                        validation.map(|timings| timings.execution_tx_finalize_tx_set_ms),
                    validation_execution_tx_finalize_transcripts_ms =
                        validation.map(|timings| timings.execution_tx_finalize_transcripts_ms),
                    validation_execution_tx_finalize_axt_ms =
                        validation.map(|timings| timings.execution_tx_finalize_axt_ms),
                    validation_execution_tx_finalize_set_results_ms =
                        validation.map(|timings| timings.execution_tx_finalize_set_results_ms),
                    validation_execution_tx_finalize_other_ms =
                        validation.map(|timings| timings.execution_tx_finalize_other_ms),
                    commit_inflight_timeout_ms =
                        self.config.persistence.commit_inflight_timeout.as_millis(),
                    "slow commit stage timings"
                );
            } else {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    tx_count = pending_tx_count,
                    blocking_total_ms = ?blocking_total_ms,
                    max_observed_stage_ms = ?max_observed_stage_ms,
                    qc_verify_ms = ?timings.qc_verify_ms,
                    kura_store_ms = ?timings.kura_store_ms,
                    state_apply_ms = ?timings.state_apply_ms,
                    state_commit_ms = ?timings.state_commit_ms,
                    persist_ms = ?timings.persist_ms,
                    validation_total_ms = validation.map(|timings| timings.total_ms),
                    validation_stateless_ms = validation.map(|timings| timings.stateless_ms),
                    validation_execution_ms = validation.map(|timings| timings.execution_ms),
                    validation_execution_tx_ms =
                        validation.map(|timings| timings.execution_tx_ms),
                    validation_execution_tx_signature_batch_ms =
                        validation.map(|timings| timings.execution_tx_signature_batch_ms),
                    validation_execution_tx_stateless_ms =
                        validation.map(|timings| timings.execution_tx_stateless_ms),
                    validation_execution_tx_access_ms =
                        validation.map(|timings| timings.execution_tx_access_ms),
                    validation_execution_tx_overlay_ms =
                        validation.map(|timings| timings.execution_tx_overlay_ms),
                    validation_execution_tx_dag_ms =
                        validation.map(|timings| timings.execution_tx_dag_ms),
                    validation_execution_tx_schedule_ms =
                        validation.map(|timings| timings.execution_tx_schedule_ms),
                    validation_execution_tx_apply_ms =
                        validation.map(|timings| timings.execution_tx_apply_ms),
                    validation_execution_tx_apply_setup_ms =
                        validation.map(|timings| timings.execution_tx_apply_setup_ms),
                    validation_execution_tx_apply_layer_build_ms =
                        validation.map(|timings| timings.execution_tx_apply_layer_build_ms),
                    validation_execution_tx_apply_prep_ms =
                        validation.map(|timings| timings.execution_tx_apply_prep_ms),
                    validation_execution_tx_apply_detached_ms =
                        validation.map(|timings| timings.execution_tx_apply_detached_ms),
                    validation_execution_tx_apply_merge_ms =
                        validation.map(|timings| timings.execution_tx_apply_merge_ms),
                    validation_execution_tx_apply_fallback_ms =
                        validation.map(|timings| timings.execution_tx_apply_fallback_ms),
                    validation_execution_tx_apply_quarantine_ms =
                        validation.map(|timings| timings.execution_tx_apply_quarantine_ms),
                    validation_execution_tx_apply_sequential_ms =
                        validation.map(|timings| timings.execution_tx_apply_sequential_ms),
                    validation_execution_tx_apply_results_ms =
                        validation.map(|timings| timings.execution_tx_apply_results_ms),
                    validation_execution_tx_apply_other_ms =
                        validation.map(|timings| timings.execution_tx_apply_other_ms),
                    validation_execution_tx_time_triggers_ms =
                        validation.map(|timings| timings.execution_tx_time_triggers_ms),
                    validation_execution_tx_finalize_ms =
                        validation.map(|timings| timings.execution_tx_finalize_ms),
                    validation_execution_tx_finalize_digest_submit_ms =
                        validation.map(|timings| timings.execution_tx_finalize_digest_submit_ms),
                    validation_execution_tx_finalize_dataspaces_ms =
                        validation.map(|timings| timings.execution_tx_finalize_dataspaces_ms),
                    validation_execution_tx_finalize_tx_set_ms =
                        validation.map(|timings| timings.execution_tx_finalize_tx_set_ms),
                    validation_execution_tx_finalize_transcripts_ms =
                        validation.map(|timings| timings.execution_tx_finalize_transcripts_ms),
                    validation_execution_tx_finalize_axt_ms =
                        validation.map(|timings| timings.execution_tx_finalize_axt_ms),
                    validation_execution_tx_finalize_set_results_ms =
                        validation.map(|timings| timings.execution_tx_finalize_set_results_ms),
                    validation_execution_tx_finalize_other_ms =
                        validation.map(|timings| timings.execution_tx_finalize_other_ms),
                    "commit stage timings"
                );
            }
        }

        match outcome {
            CommitOutcome::Success {
                committed_block,
                exec_witness,
                fastpq_witness_context,
                pipeline_events,
                state_events,
                post_apply_snapshot,
                post_commit_persistence_error,
            } => {
                let pending = take_pending_or_return!();
                self.note_view_change_from_block(pending_height, pending_view);
                let committed_tx_hashes = committed_block
                    .as_ref()
                    .external_transactions()
                    .map(|tx| tx.hash());
                self.queue
                    .remove_committed_hashes(committed_tx_hashes, None);
                let committed_nexus = self.state.nexus_snapshot();
                if autoscale_transition_committed_at(&committed_nexus, pending_height) {
                    let lane_compliance = self.queue.lane_compliance_engine();
                    self.queue.reconfigure_nexus_with_state(
                        &committed_nexus,
                        self.state.as_ref(),
                        lane_compliance,
                    );
                    debug!(
                        height = pending_height,
                        lanes = committed_nexus.lane_catalog.lane_count().get(),
                        "reconfigured queue after deterministic Nexus autoscale transition"
                    );
                }
                crate::sumeragi::status::record_kura_stage(
                    pending_height,
                    pending_view,
                    block_hash,
                );
                if let Some(error) = post_commit_persistence_error {
                    crate::sumeragi::status::record_kura_post_commit_sidecar_failure(
                        pending_height,
                        pending_view,
                        block_hash,
                    );
                    error!(
                        height = pending_height,
                        view = pending_view,
                        block = %block_hash,
                        error,
                        "post-commit Kura durability sidecar persistence failed after state advanced"
                    );
                }
                pending_previously_marked_kura_persisted = pending.kura_persisted;
                let (chain_order_hash, rechain_seq) =
                    self.vnext_chain_order_binding_for(pending_height, pending_view);
                let qc_key = (
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    pending_height,
                    pending_view,
                    lock.epoch,
                    chain_order_hash,
                    rechain_seq,
                );
                let (consensus_mode, mode_tag, _) =
                    self.consensus_context_for_height(pending_height);
                let mut cached_qc = commit_qc.or_else(|| {
                    commit_qc_from_cache_or_history(
                        &self.qc_cache,
                        block_hash,
                        pending_height,
                        pending_view,
                        lock.epoch,
                        mode_tag,
                        &commit_topology,
                    )
                });
                if let Some(qc) = cached_qc.as_ref() {
                    self.qc_cache.entry(qc_key).or_insert_with(|| qc.clone());
                }
                if cached_qc.is_none() {
                    if let (Some(signers), Some(view_signers)) =
                        (qc_signers.as_ref(), view_signers.as_ref())
                    {
                        let accepted_votes = self.accepted_votes_for_qc_slot(
                            crate::sumeragi::consensus::Phase::Commit,
                            block_hash,
                            pending_height,
                            pending_view,
                            lock.epoch,
                            &topology,
                        );
                        let aggregate_signature = match super::aggregate_vote_signatures(
                            &accepted_votes,
                            crate::sumeragi::consensus::Phase::Commit,
                            block_hash,
                            pending_height,
                            pending_view,
                            lock.epoch,
                            view_signers,
                        ) {
                            Ok(signature) => signature,
                            Err(err) => {
                                warn!(
                                    ?err,
                                    height = pending_height,
                                    view = pending_view,
                                    block = %block_hash,
                                    "failed to aggregate precommit signatures for cached QC"
                                );
                                Vec::new()
                            }
                        };
                        let stake_snapshot = match consensus_mode {
                            ConsensusMode::Permissioned => None,
                            ConsensusMode::Npos => {
                                let world = self.state.world_view();
                                CommitStakeSnapshot::from_roster(&world, &commit_topology)
                            }
                        };
                        if let Some((parent_state_root, post_state_root)) =
                            pending.parent_state_root.zip(pending.post_state_root)
                        {
                            let (chain_order_hash, rechain_seq) =
                                self.vnext_chain_order_binding_for(pending_height, pending_view);
                            if let Some(derived_qc) = super::derive_block_sync_qc_from_signers(
                                block_hash,
                                pending_height,
                                pending_view,
                                lock.epoch,
                                chain_order_hash,
                                rechain_seq,
                                parent_state_root,
                                post_state_root,
                                &commit_topology,
                                consensus_mode,
                                stake_snapshot.as_ref(),
                                mode_tag,
                                signers,
                                aggregate_signature,
                            ) {
                                self.qc_cache
                                    .insert(Self::qc_tally_key(&derived_qc), derived_qc.clone());
                                cached_qc = Some(derived_qc);
                            }
                        } else {
                            warn!(
                                height = pending_height,
                                view = pending_view,
                                block = %block_hash,
                                "skipping derived QC cache: missing execution roots"
                            );
                        }
                    }
                }
                info!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    "state committed for block"
                );
                exec_witness_to_emit =
                    exec_witness.map(|witness| (witness, fastpq_witness_context));
                parent_to_cleanup = pending.block.header().prev_block_hash();
                committed_pipeline_events_tail = pipeline_events;
                committed_state_events_tail = state_events;
                committed_post_apply_snapshot_tail = Some(post_apply_snapshot);
                committed_cached_qc_tail = cached_qc;
                committed_pending_tail = Some(pending);
                committed_block_tail = Some(committed_block);
                committed_consensus_mode_tail = Some(consensus_mode);
                committed_mode_tag_tail = Some(mode_tag);
                trace!(
                    height = pending_height,
                    view = pending_view,
                    block = ?block_hash,
                    "Committed block after DA availability gate cleared"
                );
                committed = true;
            }
            CommitOutcome::KuraStoreFailed {
                committed_block,
                error,
            } => {
                let pending = take_pending_or_return!();
                crate::sumeragi::status::record_kura_stage(
                    pending_height,
                    pending_view,
                    block_hash,
                );
                error!(
                    ?error,
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    "failed to store committed block in kura; keeping block pending"
                );
                crate::sumeragi::status::record_kura_stage_rollback(
                    pending_height,
                    pending_view,
                    block_hash,
                    kura::KURA_STAGE_ROLLBACK_REASON_STORE,
                );
                let failure = Self::handle_kura_store_failure(
                    pending,
                    committed_block.clone().into(),
                    block_hash,
                    pending_height,
                    pending_view,
                    now,
                    self.config.persistence.kura_retry_interval,
                    self.config.persistence.kura_retry_max_attempts,
                    self.queue.as_ref(),
                    self.state.as_ref(),
                    self.telemetry_handle(),
                );
                if let Some(pending) = failure.pending {
                    self.pending.pending_blocks.insert(block_hash, pending);
                }
                if failure.clean_block_hash {
                    self.qc_cache
                        .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                    self.qc_signer_tally
                        .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                    self.clean_rbc_sessions_for_block(block_hash, pending_height);
                    block_hash_to_clean = Some(block_hash);
                    let latest_committed_qc = self.latest_committed_qc();
                    kura::reset_qcs_after_kura_abort(
                        &mut self.locked_qc,
                        &mut self.highest_qc,
                        self.state.as_ref(),
                        latest_committed_qc,
                        kura::KURA_LOCK_RESET_REASON_ABORT,
                    );
                    self.trigger_view_change_with_cause(
                        pending_height,
                        pending_view,
                        ViewChangeCause::CommitFailure,
                    );
                }
            }
            CommitOutcome::StateCommitFailed {
                committed_block,
                error,
                error_kind,
            } => {
                let mut pending = take_pending_or_return!();
                crate::sumeragi::status::record_kura_stage(
                    pending_height,
                    pending_view,
                    block_hash,
                );
                warn!(
                    height = pending_height,
                    view = pending_view,
                    block = ?block_hash,
                    error = %error,
                    "failed to commit state for block after persisting; keeping it pending"
                );
                crate::sumeragi::status::record_kura_stage_rollback(
                    pending_height,
                    pending_view,
                    block_hash,
                    kura::KURA_STAGE_ROLLBACK_REASON_STATE,
                );
                let state_height = self.state.committed_height();
                let state_tip_hash = self.state.latest_block_hash_fast();
                let state_height_u64 = u64::try_from(state_height).unwrap_or(u64::MAX);
                let state_aligned_with_block = state_tip_hash.is_some_and(|tip| tip == block_hash)
                    && state_height_u64 >= pending_height;
                if matches!(
                    error_kind,
                    Some(crate::state::storage_transactions::TransactionsBlockError::HeightMismatch { .. })
                ) && state_height_u64 >= pending_height
                {
                    if state_aligned_with_block {
                        info!(
                            height = pending_height,
                            view = pending_view,
                            block = %block_hash,
                            state_height,
                            "state already reflects block after state-commit retry; dropping duplicate pending"
                        );
                        self.clean_rbc_sessions_for_committed_block_if_settled(
                            block_hash,
                            pending_height,
                        );
                        if let Some(parent) = pending.block.header().prev_block_hash() {
                            self.qc_cache
                                .retain(|(_, hash, _, _, _, _, _), _| hash != &parent);
                            self.qc_signer_tally
                                .retain(|(_, hash, _, _, _, _, _), _| hash != &parent);
                        }
                    } else {
                        let txs: Vec<_> = pending.block.external_entrypoints_cloned().collect();
                        let (requeued, failures, duplicate_failures, _) =
                            requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
                        warn!(
                            height = pending_height,
                            view = pending_view,
                            block = %block_hash,
                            state_height,
                            state_tip_hash = ?state_tip_hash,
                            requeued,
                            failures,
                            duplicate_failures,
                            "state advanced to a different head after persisted commit failure; dropping stale pending block"
                        );
                        self.clean_rbc_sessions_for_block(block_hash, pending_height);
                        self.qc_cache
                            .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                        self.qc_signer_tally
                            .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                        self.subsystems
                            .propose
                            .proposal_cache
                            .pop_hint(pending_height, pending_view);
                        self.subsystems
                            .propose
                            .proposal_cache
                            .pop_proposal(pending_height, pending_view);
                        self.trigger_view_change_after_commit_failure(
                            pending_height,
                            pending_view,
                        );
                    }
                } else {
                    pending.mark_kura_persisted();
                    pending.set_block(committed_block.into());
                    self.pending.pending_blocks.insert(block_hash, pending);
                }
            }
            CommitOutcome::Rejected {
                failed_block,
                error,
                pipeline_events,
            } => {
                let mut pending = take_pending_or_return!();
                let mut emit_pipeline_events_now = false;
                let commit_signatures_missing = matches!(
                    &error,
                    crate::block::BlockValidationError::SignatureVerification(
                        crate::block::SignatureVerificationError::NotEnoughSignatures { .. }
                    )
                );
                let tally = crate::block::valid::commit_signature_tally(&failed_block, &topology);
                crate::sumeragi::status::record_commit_quorum_snapshot(
                    pending_height,
                    pending_view,
                    block_hash,
                    tally.present as u64,
                    tally.counted as u64,
                    tally.set_b_signatures as u64,
                    topology.min_votes_for_commit() as u64,
                );
                #[cfg(feature = "telemetry")]
                {
                    self.telemetry.set_commit_signature_totals(
                        tally.present as u64,
                        tally.counted as u64,
                        tally.set_b_signatures as u64,
                        topology.min_votes_for_commit() as u64,
                    );
                }
                let sig_indices: Vec<u32> = failed_block
                    .signatures()
                    .map(|sig| u32::try_from(sig.index()).unwrap_or_default())
                    .collect();
                let now = Instant::now();
                let pending_age = pending.age();
                let progress_age = pending.progress_age(now);
                let vote_count = sig_indices.len();
                let quorum_timeout = self.quorum_timeout(da_enabled);
                let availability_timeout = self.availability_timeout(quorum_timeout, da_enabled);
                let missing_local_data =
                    matches!(pending.last_gate, Some(GateReason::MissingLocalData));
                let quorum_reached = has_quorum_signers || vote_count >= min_votes_for_commit;
                let fast_timeout = self.pending_fast_path_timeout_current();
                let has_votes = pending.local_commit_vote_emitted()
                    || vote_count > 0
                    || self.pending_block_has_votes(block_hash, pending_height, pending_view);
                let has_qc = pending.commit_qc_observed()
                    || self.pending_block_has_qc(block_hash, pending_height, pending_view);
                let quorum_stall_age = if has_votes || has_qc {
                    progress_age
                } else {
                    pending_age
                };
                let validation_inflight = pending.validation_status == ValidationStatus::Pending
                    && self
                        .subsystems
                        .validation
                        .inflight
                        .contains_key(&block_hash);
                let fast_path_allowed =
                    !da_enabled && !has_votes && !has_qc && !validation_inflight;
                let effective_quorum_timeout = if fast_path_allowed {
                    fast_timeout.min(quorum_timeout)
                } else {
                    quorum_timeout
                };

                if commit_signatures_missing
                    && !has_quorum_signers
                    && missing_quorum_stale(
                        quorum_stall_age,
                        effective_quorum_timeout,
                        quorum_reached,
                    )
                {
                    let reschedule_backoff =
                        super::quorum_reschedule_backoff_from_timeout(quorum_timeout);
                    if missing_local_data && pending_age < availability_timeout {
                        debug!(
                            height = pending_height,
                            view = pending_view,
                            block = ?block_hash,
                            pending_age_ms = pending_age.as_millis(),
                            availability_timeout_ms = availability_timeout.as_millis(),
                            "deferring quorum reschedule while awaiting local payload"
                        );
                        pending.set_block(failed_block);
                        self.pending.pending_blocks.insert(block_hash, pending);
                    } else if self.rbc_availability_unresolved_for_reschedule(
                        (block_hash, pending_height, pending_view),
                        &topology,
                        pending_age,
                        availability_timeout,
                    ) {
                        debug!(
                            height = pending_height,
                            view = pending_view,
                            block = ?block_hash,
                            pending_age_ms = pending_age.as_millis(),
                            quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                            "deferring quorum reschedule while RBC availability is unresolved"
                        );
                        pending.set_block(failed_block);
                        self.pending.pending_blocks.insert(block_hash, pending);
                    } else {
                        let queue_depths = super::status::worker_queue_depth_snapshot();
                        if queue_depths.vote_rx > 0 {
                            debug!(
                                height = pending_height,
                                view = pending_view,
                                block = ?block_hash,
                                pending_age_ms = pending_age.as_millis(),
                                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                                quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                                vote_rx_depth = queue_depths.vote_rx,
                                "deferring quorum reschedule while vote queue is backlogged"
                            );
                            pending.set_block(failed_block);
                            self.pending.pending_blocks.insert(block_hash, pending);
                        } else if pending.reschedule_due(now, reschedule_backoff) {
                            reschedule_quorum = Some((
                                pending,
                                pending_age,
                                quorum_stall_age,
                                min_votes_for_commit,
                                vote_count,
                                quorum_timeout,
                            ));
                        } else {
                            pending.set_block(failed_block);
                            self.pending.pending_blocks.insert(block_hash, pending);
                        }
                    }
                } else {
                    if matches!(
                        &error,
                        crate::block::BlockValidationError::SignatureVerification(
                            crate::block::SignatureVerificationError::LeaderMissing
                        )
                    ) {
                        let hash = failed_block.hash();
                        let mut matched: Vec<PeerId> = Vec::new();
                        for peer in &commit_topology {
                            if failed_block.signatures().any(|sig| {
                                sig.signature().verify_hash(peer.public_key(), hash).is_ok()
                            }) {
                                matched.push(peer.clone());
                            }
                        }
                        iroha_logger::warn!(
                            block = %hash,
                            matched_peers = ?matched,
                            "leader signature debug match set"
                        );
                    }

                    let height_or_hash_mismatch = matches!(
                        &error,
                        crate::block::BlockValidationError::PrevBlockHeightMismatch { .. }
                            | crate::block::BlockValidationError::PrevBlockHashMismatch { .. }
                    );
                    if height_or_hash_mismatch {
                        let outcome = handle_prev_block_mismatch(
                            self.queue.as_ref(),
                            self.state.as_ref(),
                            failed_block.external_entrypoints_cloned().collect(),
                        );
                        if outcome.failures > 0 {
                            warn!(
                                height = pending_height,
                                view = pending_view,
                                failures = outcome.failures,
                                requeued = outcome.requeued,
                                "failed to requeue some transactions after block mismatch"
                            );
                        }
                        self.qc_cache
                            .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                        self.qc_signer_tally
                            .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                        block_hash_to_clean = Some(block_hash);
                        emit_pipeline_events_now = true;
                    } else if has_quorum_signers {
                        warn!(
                            height = pending_height,
                            view = pending_view,
                            block = ?block_hash,
                            quorum_signers = quorum_signer_count.unwrap_or(0),
                            min_votes = min_votes_for_commit,
                            ?error,
                            "Failed to commit block after quorum signatures; requeueing payload and triggering view change"
                        );
                        let proposer_idx = u32::try_from(topology.leader_index()).unwrap_or(0);
                        let proposal = Self::build_consensus_proposal(
                            &failed_block,
                            pending.payload_hash,
                            lock,
                            proposer_idx,
                            pending_view,
                            lock.epoch,
                        );
                        let reason = error.to_string();
                        let evidence = invalid_proposal_evidence(proposal, reason);
                        let _ = self.handle_evidence(evidence);
                        let latest_committed = self.latest_committed_qc();
                        let outcome = handle_commit_failure_with_qc_quorum(
                            pending,
                            failed_block,
                            block_hash,
                            pending_height,
                            pending_view,
                            self.queue.as_ref(),
                            self.state.as_ref(),
                            self.locked_qc,
                            self.highest_qc,
                            latest_committed,
                        );
                        debug!(
                            height = pending_height,
                            view = pending_view,
                            block = ?block_hash,
                            requeued = outcome.requeued,
                            failed_requeues = outcome.failed_requeues,
                            drop_pending = outcome.drop_pending,
                            "commit failure requeue outcome"
                        );
                        if outcome.view_change_triggered {
                            self.trigger_view_change_after_commit_failure(
                                pending_height,
                                pending_view,
                            );
                        }
                        if !outcome.drop_pending {
                            self.pending
                                .pending_blocks
                                .insert(block_hash, outcome.pending);
                        }
                        if outcome.clean_block_hash {
                            self.qc_cache
                                .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                            self.qc_signer_tally
                                .retain(|(_, hash, _, _, _, _, _), _| hash != &block_hash);
                            block_hash_to_clean = Some(block_hash);
                        }
                        emit_pipeline_events_now = outcome.drop_pending;
                        if self.locked_qc != outcome.locked_qc {
                            self.locked_qc = outcome.locked_qc;
                            if let Some(lock) = self.locked_qc {
                                super::status::set_locked_qc(
                                    lock.height,
                                    lock.view,
                                    Some(lock.subject_block_hash),
                                );
                                self.prune_precommit_votes_conflicting_with_lock(lock);
                            } else {
                                super::status::set_locked_qc(0, 0, None);
                            }
                        }
                        if self.highest_qc != outcome.highest_qc {
                            self.highest_qc = outcome.highest_qc;
                            if let Some(highest) = self.highest_qc {
                                super::status::set_highest_qc(highest.height, highest.view);
                                super::status::set_highest_qc_hash(highest.subject_block_hash);
                            } else {
                                super::status::set_highest_qc(0, 0);
                                super::status::set_highest_qc_hash(HashOf::from_untyped_unchecked(
                                    Hash::prehashed([0; Hash::LENGTH]),
                                ));
                            }
                        }
                    } else {
                        warn!(
                            height = pending_height,
                            view = pending_view,
                            block = ?block_hash,
                            leader = ?topology.leader(),
                            topology_len = topology.as_ref().len(),
                            min_votes = self.commit_min_votes(&topology),
                            sig_count = sig_indices.len(),
                            sig_indices = ?sig_indices,
                            ?error,
                            "Failed to commit block; keeping it pending for retry"
                        );
                        pending.set_block(failed_block);
                        self.pending.pending_blocks.insert(block_hash, pending);
                    }
                }
                if emit_pipeline_events_now {
                    emit_pipeline_events(&self.events_sender, pipeline_events);
                }
            }
        }

        if let Some(hash) = block_hash_to_clean {
            self.clean_rbc_sessions_for_block(hash, pending_height);
        }
        if let Some((pending, age, quorum_stall_age, min_votes, vote_count, quorum_timeout)) =
            reschedule_quorum
        {
            self.reschedule_pending_quorum_block(
                pending,
                age,
                quorum_stall_age,
                min_votes,
                vote_count,
                quorum_timeout,
                super::quorum_reschedule_backoff_from_timeout(quorum_timeout),
                None,
                Instant::now(),
            );
        }
        if committed {
            self.finalize_collector_plan(true);
            self.promote_commit_anchor_qc(lock);

            if let Some(child_qc) = post_commit_qc {
                self.promote_commit_anchor_qc(child_qc);
            }

            crate::sumeragi::status::record_round_gap_unblocked(
                pending_height,
                pending_view,
                block_hash,
            );
            self.record_round_trace_event(super::RoundTraceEvent {
                key: super::RoundTraceKey {
                    height: pending_height,
                    view: pending_view,
                },
                phase: super::status::RoundPhaseTrace::Commit,
                cause: super::status::RoundEventCauseTrace::CommitCompleted,
                queue_latency_ms: None,
                no_progress_wake: false,
            });
            let refreshed = self.refresh_tip_activated_pending_progress(
                pending_height,
                block_hash,
                Instant::now(),
            );
            if refreshed > 0 {
                debug!(
                    height = pending_height,
                    block = %block_hash,
                    refreshed,
                    "refreshed pending progress for proposals activated by the committed tip"
                );
            }
            if let (
                Some(committed_block),
                Some(pending),
                Some(post_apply_snapshot),
                Some(consensus_mode),
                Some(mode_tag),
            ) = (
                committed_block_tail.take(),
                committed_pending_tail.take(),
                committed_post_apply_snapshot_tail.take(),
                committed_consensus_mode_tail,
                committed_mode_tag_tail,
            ) {
                if let Some(qc) = committed_cached_qc_tail.as_ref() {
                    super::status::record_commit_qc(qc.clone());
                }
                emit_pipeline_events(
                    &self.events_sender,
                    std::mem::take(&mut committed_pipeline_events_tail),
                );
                for event in std::mem::take(&mut committed_state_events_tail) {
                    if let Err(err) = self.events_sender.send(event) {
                        debug!(?err, "failed to send pipeline event");
                    }
                }

                let params_snapshot = {
                    let world = self.state.world_view();
                    let params = world.parameters();
                    self.update_effective_timing_status_from_world(&world, self.consensus_mode);
                    (
                        params.block().max_transactions().get(),
                        params.smart_contract().execution_depth(),
                        params.executor().execution_depth(),
                    )
                };
                debug!(
                    height = pending_height,
                    view = pending_view,
                    max_tx = params_snapshot.0,
                    sc_depth = params_snapshot.1,
                    exec_depth = params_snapshot.2,
                    "state parameters after commit"
                );
                self.refresh_p2p_topology_with_current(
                    post_apply_snapshot.world_peers.iter().cloned().collect(),
                );

                if let Some(signers) = qc_signers.as_ref() {
                    let accepted_votes = self.accepted_votes_for_qc_slot(
                        crate::sumeragi::consensus::Phase::Commit,
                        block_hash,
                        pending_height,
                        pending_view,
                        lock.epoch,
                        &topology,
                    );
                    let aggregate_signature = committed_cached_qc_tail.as_ref().map_or_else(
                        || {
                            view_signers
                                .as_ref()
                                .and_then(|view_signers| {
                                    super::aggregate_vote_signatures(
                                        &accepted_votes,
                                        crate::sumeragi::consensus::Phase::Commit,
                                        block_hash,
                                        pending_height,
                                        pending_view,
                                        lock.epoch,
                                        view_signers,
                                    )
                                    .ok()
                                })
                                .unwrap_or_default()
                        },
                        |qc| qc.aggregate.bls_aggregate_signature.clone(),
                    );
                    if aggregate_signature.is_empty() {
                        warn!(
                            height = pending_height,
                            view = pending_view,
                            block = %block_hash,
                            "skipping precommit signer record: missing aggregate signature"
                        );
                    } else {
                        let roots = committed_cached_qc_tail
                            .as_ref()
                            .map(|qc| (qc.parent_state_root, qc.post_state_root))
                            .or_else(|| pending.parent_state_root.zip(pending.post_state_root));
                        if let Some((parent_state_root, post_state_root)) = roots {
                            let stake_snapshot = post_apply_snapshot.stake_snapshot.clone();
                            let (chain_order_hash, rechain_seq) =
                                committed_cached_qc_tail.as_ref().map_or_else(
                                    || {
                                        self.vnext_chain_order_binding_for(
                                            pending_height,
                                            pending_view,
                                        )
                                    },
                                    |qc| (qc.chain_order_hash, qc.rechain_seq),
                                );
                            crate::sumeragi::status::record_precommit_signers(
                                crate::sumeragi::status::PrecommitSignerRecord {
                                    block_hash,
                                    height: pending_height,
                                    view: pending_view,
                                    epoch: lock.epoch,
                                    chain_order_hash,
                                    rechain_seq,
                                    parent_state_root,
                                    post_state_root,
                                    signers: signers.clone(),
                                    bls_aggregate_signature: aggregate_signature,
                                    roster_len: commit_topology.len(),
                                    mode_tag: mode_tag.to_string(),
                                    validator_set: commit_topology.clone(),
                                    stake_snapshot,
                                },
                            );
                        } else {
                            warn!(
                                height = pending_height,
                                view = pending_view,
                                block = %block_hash,
                                "skipping precommit signer record: missing execution roots"
                            );
                        }
                    }
                } else if let Some(qc) = committed_cached_qc_tail.as_ref() {
                    if let Some(record) = Self::precommit_signer_record_from_cached_qc(
                        qc,
                        &commit_topology,
                        consensus_mode,
                        post_apply_snapshot.stake_snapshot.clone(),
                    ) {
                        crate::sumeragi::status::record_precommit_signers(record);
                    }
                }
                let mut commit_roster_recorded = false;
                if let Some(qc) = committed_cached_qc_tail.as_ref() {
                    let checkpoint = ValidatorSetCheckpoint::new_with_chain_order(
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
                    );
                    if self.state.record_commit_roster_with_sidecar(
                        qc,
                        &checkpoint,
                        post_apply_snapshot.stake_snapshot.clone(),
                    ) {
                        commit_roster_recorded = true;
                        debug!(
                            height = qc.height,
                            view = qc.view,
                            block = %qc.subject_block_hash,
                            "persisted commit roster sidecar from commit certificate"
                        );
                    }
                }
                if !commit_roster_recorded {
                    self.persist_roster_sidecar_for_commit(
                        committed_block.as_ref(),
                        &commit_topology,
                    );
                }
                self.flush_pending_fetch_requests_if_ready(committed_block.as_ref());
                self.flush_pending_block_body_requests_if_ready(committed_block.as_ref());
                if pending_height == 1 {
                    // Seed the genesis roster after the block is durably persisted.
                    self.ensure_genesis_commit_roster();
                }

                let set_b_signers = |signers: &BTreeSet<ValidatorIndex>| -> usize {
                    let proxy_tail_idx = topology.proxy_tail_index();
                    signers
                        .iter()
                        .filter(|signer| {
                            super::view_index_for_canonical_signer(
                                **signer,
                                &topology,
                                &canonical_topology,
                            )
                            .and_then(|idx| usize::try_from(idx).ok())
                            .is_some_and(|idx| idx > proxy_tail_idx)
                        })
                        .count()
                };

                let tally = if let Some(signers) = qc_signers.as_ref() {
                    crate::block::valid::SignatureTally {
                        present: signers.len(),
                        counted: signers.len(),
                        set_b_signatures: set_b_signers(signers),
                    }
                } else if let Some(qc) = committed_cached_qc_tail.as_ref() {
                    let roster_len = commit_topology.len();
                    match super::qc_signer_indices(qc, roster_len, roster_len) {
                        Ok(parsed) => crate::block::valid::SignatureTally {
                            present: parsed.present.len(),
                            counted: parsed.voting.len(),
                            set_b_signatures: set_b_signers(&parsed.voting),
                        },
                        Err(_) => crate::block::valid::commit_signature_tally(
                            committed_block.as_ref(),
                            &topology,
                        ),
                    }
                } else {
                    crate::block::valid::commit_signature_tally(committed_block.as_ref(), &topology)
                };
                crate::sumeragi::status::record_commit_quorum_snapshot(
                    pending_height,
                    pending_view,
                    block_hash,
                    tally.present as u64,
                    tally.counted as u64,
                    tally.set_b_signatures as u64,
                    topology.min_votes_for_commit() as u64,
                );
                #[cfg(feature = "telemetry")]
                {
                    self.telemetry.set_commit_signature_totals(
                        tally.present as u64,
                        tally.counted as u64,
                        tally.set_b_signatures as u64,
                        topology.min_votes_for_commit() as u64,
                    );
                }
                info!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    pending_previously_marked_kura_persisted,
                    "stored committed block to kura"
                );
                self.drive_vnext_commit_persisted_for_block(
                    block_hash,
                    pending_height,
                    pending_view,
                );
                if let Some((height, payload_len)) =
                    self.kura.durable_block_payload_len_by_hash(block_hash)
                {
                    self.schedule_background(BackgroundRequest::Broadcast {
                        msg: BlockMessageWire::new(BlockMessage::KuraReplicaAdvert(
                            super::message::KuraReplicaAdvert {
                                height,
                                block_hash,
                                payload_len,
                            },
                        )),
                    });
                }
                #[cfg(feature = "telemetry")]
                {
                    self.telemetry
                        .report_block_commit_blocking(&committed_block.as_ref().header());
                }

                #[cfg(feature = "telemetry")]
                let block_sync_start = Instant::now();
                let sync_block: SignedBlock = committed_block.as_ref().clone();
                self.broadcast_block_created_for_block_sync(
                    self.frontier_block_created_for_wire(&sync_block),
                    &post_apply_snapshot.world_peers,
                );
                #[cfg(feature = "telemetry")]
                {
                    let ms =
                        u64::try_from(block_sync_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    self.telemetry
                        .observe_commit_stage_ms(crate::telemetry::CommitStage::BlockSync, ms);
                }
            }

            if let Some((witness, fastpq_context)) = exec_witness_to_emit {
                self.emit_exec_artifacts(
                    block_hash,
                    pending_height,
                    pending_view,
                    witness,
                    fastpq_context,
                );
            }
            // Commit finished; keep undelivered RBC sessions alive under DA so peers that
            // committed through another path can still converge their local RBC status.
            self.clean_rbc_sessions_for_committed_block_if_settled(block_hash, pending_height);

            self.prune_descendants_not_on_tip(pending_height, block_hash);
            let obsolete_missing: Vec<_> = self
                .pending
                .missing_block_requests
                .iter()
                .filter(|(_, stats)| stats.height <= pending_height)
                .map(|(hash, _)| *hash)
                .collect();
            for hash in obsolete_missing {
                let reason = if hash == block_hash {
                    MissingBlockClearReason::PayloadAvailable
                } else {
                    MissingBlockClearReason::Obsolete
                };
                self.clear_missing_block_request(&hash, reason);
            }

            // Drop stale pending blocks and cached proposals/QCs at or below the committed height
            // to avoid resurrecting divergent chains in later views.
            let stale: Vec<_> = self
                .pending
                .pending_blocks
                .iter()
                .filter_map(|(hash, pending)| {
                    (pending.height <= pending_height && hash != &block_hash)
                        .then_some((*hash, pending.height))
                })
                .collect();
            for (stale_hash, stale_height) in stale {
                self.pending.pending_blocks.remove(&stale_hash);
                self.clear_validation_ownership_for_block(stale_hash);
                self.clean_rbc_sessions_for_block(stale_hash, stale_height);
                self.qc_cache
                    .retain(|(_, hash, _, _, _, _, _), _| hash != &stale_hash);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _, _, _), _| hash != &stale_hash);
                self.block_signer_cache.remove_block(&stale_hash);
            }
            self.qc_cache.retain(|(_, hash, height, _, _, _, _), _| {
                *hash == block_hash || *height > pending_height
            });
            self.qc_signer_tally
                .retain(|(_, hash, height, _, _, _, _), _| {
                    *hash == block_hash || *height > pending_height
                });
            self.subsystems
                .propose
                .proposal_cache
                .prune_height_leq(pending_height);
            if let Some(parent) = parent_to_cleanup {
                self.qc_cache
                    .retain(|(_, hash, _, _, _, _, _), _| hash != &parent);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _, _, _), _| hash != &parent);
                self.block_signer_cache.remove_block(&parent);
            }
            let retention_floor = pending_height.saturating_sub(1);
            self.vote_log
                .retain(|(_, height, _, _, _, _, _), _| *height >= retention_floor);
            self.try_replay_deferred_votes();
            let _ = self.maybe_request_frontier_gap_realign_after_commit(Instant::now());
        }
        committed
    }

    fn kickstart_pacemaker_after_durable_commit(&mut self) -> bool {
        let backpressure = self.proposal_backpressure_at(Instant::now());
        kickstart_pacemaker_after_commit(self.queue.queued_len(), backpressure, |now| {
            self.on_pacemaker_propose_ready(now)
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn finalize_pending_block(
        &mut self,
        lock: crate::sumeragi::consensus::QcHeaderRef,
        mut pending: PendingBlock,
        post_commit_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    ) -> bool {
        let block_hash = lock.subject_block_hash;
        let pending_height = pending.height;
        let pending_view = pending.view;
        let now = Instant::now();
        debug!(
            height = pending_height,
            view = pending_view,
            block = %block_hash,
            "finalizing pending block"
        );
        if let Some(inflight) = self.subsystems.commit.inflight.as_ref() {
            if inflight.block_hash == block_hash {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    "commit already in flight; skipping finalize"
                );
                return false;
            }
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let state_height = self.state.committed_height();
        let state_tip_hash = self.state.latest_block_hash_fast();
        if pending.aborted {
            let pending_parent = pending.block.header().prev_block_hash();
            let extends_tip = super::pending_extends_tip(
                pending_height,
                pending_parent,
                state_height,
                state_tip_hash,
            );
            let conflicting_local_vote = self
                .local_conflicting_slot_vote(pending_height, lock.epoch, block_hash)
                .is_some();
            if pending.is_retired_same_height() {
                if pending.commit_qc_observed() && extends_tip {
                    debug!(
                        height = pending_height,
                        view = pending_view,
                        block = %block_hash,
                        conflicting_local_vote,
                        "finalizing retired same-height pending block with matching commit QC"
                    );
                } else {
                    debug!(
                        height = pending_height,
                        view = pending_view,
                        block = %block_hash,
                        extends_tip,
                        conflicting_local_vote,
                        "retired same-height pending block not eligible for finalize"
                    );
                    self.pending.pending_blocks.insert(block_hash, pending);
                    return false;
                }
            } else if pending.commit_qc_observed() && extends_tip {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    "reviving aborted pending block to finalize with commit QC"
                );
                pending.aborted = false;
            } else {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    "pending block marked aborted; skipping finalize"
                );
                self.pending.pending_blocks.insert(block_hash, pending);
                return false;
            }
        }
        let kura_has_block = self.kura.get_block_height_by_hash(block_hash).is_some();
        if kura::kura_and_state_aligned_for_block(
            kura_has_block,
            state_height,
            state_tip_hash,
            pending_height,
            block_hash,
        ) {
            debug!(
                height = pending_height,
                view = pending_view,
                block = %block_hash,
                "pending block already committed; skipping finalize"
            );
            self.promote_commit_anchor_qc(lock);
            if let Some(child_qc) = post_commit_qc {
                self.promote_commit_anchor_qc(child_qc);
            }
            self.clean_rbc_sessions_for_committed_block_if_settled(block_hash, pending_height);
            if let Some(parent) = pending.block.header().prev_block_hash() {
                self.qc_cache
                    .retain(|(_, hash, _, _, _, _, _), _| hash != &parent);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _, _, _), _| hash != &parent);
            }
            return true;
        }
        if !pending.commit_qc_observed() {
            debug!(
                height = pending_height,
                view = pending_view,
                block = %block_hash,
                "commit certificate missing; deferring finalize"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        if !super::pending_extends_tip(
            pending_height,
            pending.block.header().prev_block_hash(),
            state_height,
            state_tip_hash,
        ) {
            debug!(
                height = pending_height,
                view = pending_view,
                block = %block_hash,
                "commit certificate received before tip; deferring finalize"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        if kura_has_block && !pending.kura_persisted {
            info!(
                height = pending_height,
                view = pending_view,
                block = %block_hash,
                state_height,
                "block already persisted in kura; retrying commit with mandatory durability check"
            );
            pending.mark_kura_persisted();
        }
        let gate = self.refresh_da_gate_status(&mut pending);
        if let Some(reason) = gate.reason {
            debug!(
                ?reason,
                da_enabled = gate.da_enabled,
                height = pending_height,
                view = pending_view,
                block = %block_hash,
                "DA availability missing; deferring finalize until gate clears"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let (consensus_mode, mode_tag, _) = self.consensus_context_for_height(pending_height);
        let (chain_order_hash, rechain_seq) =
            self.vnext_chain_order_binding_for(pending_height, pending_view);
        let qc_key = (
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            pending_height,
            pending_view,
            lock.epoch,
            chain_order_hash,
            rechain_seq,
        );
        let cached_commit_qc = self.qc_cache.get(&qc_key).cloned();
        let history_commit_qc = cached_commit_qc
            .as_ref()
            .is_none()
            .then(|| {
                commit_qc_from_history(
                    block_hash,
                    pending_height,
                    pending_view,
                    lock.epoch,
                    mode_tag,
                )
            })
            .flatten();
        let certified_roster = cached_commit_qc
            .as_ref()
            .or(history_commit_qc.as_ref())
            .map(|qc| qc.validator_set.clone())
            .filter(|roster| !roster.is_empty());
        let cached_vote_roster = self
            .vote_roster_cache
            .get(&block_hash)
            .filter(|cached| {
                cached.height == pending_height
                    && cached.view == pending_view
                    && !cached.roster.is_empty()
            })
            .map(|cached| {
                super::roster::canonicalize_roster_for_mode(cached.roster.clone(), consensus_mode)
            });
        let mut commit_topology = cached_vote_roster
            // A cached QC's validator_set is order-sensitive: block and aggregate
            // signature indices are validated against this exact roster.
            .or(certified_roster)
            .unwrap_or_else(|| {
                self.roster_for_vote_with_mode(
                    block_hash,
                    pending_height,
                    pending_view,
                    consensus_mode,
                )
            });
        if commit_topology.is_empty() {
            commit_topology = self.roster_for_live_vote_with_mode(pending_height, consensus_mode);
        }
        iroha_logger::info!(
            commit_topology_len = commit_topology.len(),
            commit_topology = ?commit_topology,
            "finalizing pending block with commit topology"
        );
        if commit_topology.is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                pending_height,
                pending_view,
                Some(block_hash),
                self.queue.queued_len(),
                now,
                ProposalDeferWarningKind::EmptyCommitTopologyFinalize,
                "finalize_pending_block",
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        self.clear_consensus_recovery_for_round(pending_height, pending_view);
        let canonical_topology = super::network_topology::Topology::new(commit_topology.clone());
        let mut topology = canonical_topology.clone();
        if let Err(err) = self.leader_index_for(&mut topology, pending_height, pending_view) {
            warn!(
                ?err,
                height = pending_height,
                view = pending_view,
                block = %block_hash,
                "failed to align commit topology with view; deferring finalize"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let signature_topology = topology.as_ref().to_vec();
        let quorum_signers = self
            .qc_signer_tally
            .get(&qc_key)
            .map(|tally| tally.voting_signers.clone())
            .or_else(|| {
                self.qc_cache
                    .get(&qc_key)
                    .and_then(|qc| {
                        super::qc_signer_indices(
                            qc,
                            topology.as_ref().len(),
                            topology.as_ref().len(),
                        )
                        .ok()
                    })
                    .map(|parsed| parsed.voting)
            });
        let view_signers = quorum_signers.as_ref().and_then(|signers| {
            let mapped =
                super::normalize_signer_indices_to_view(signers, &topology, &canonical_topology);
            if mapped.len() == signers.len() {
                Some(mapped)
            } else {
                warn!(
                    height = pending_height,
                    view = pending_view,
                    signers = signers.len(),
                    view_signers = mapped.len(),
                    "skipping vote aggregation: signer mapping to view topology incomplete"
                );
                None
            }
        });
        let mut commit_qc = cached_commit_qc.or(history_commit_qc).or_else(|| {
            commit_qc_from_cache_or_history(
                &self.qc_cache,
                block_hash,
                pending_height,
                pending_view,
                lock.epoch,
                mode_tag,
                &commit_topology,
            )
        });
        if commit_qc.is_none() {
            if let (Some(signers), Some(view_signers)) =
                (quorum_signers.as_ref(), view_signers.as_ref())
            {
                let accepted_votes = self.accepted_votes_for_qc_slot(
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    pending_height,
                    pending_view,
                    lock.epoch,
                    &topology,
                );
                let aggregate_signature = match super::aggregate_vote_signatures(
                    &accepted_votes,
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    pending_height,
                    pending_view,
                    lock.epoch,
                    view_signers,
                ) {
                    Ok(signature) => signature,
                    Err(err) => {
                        warn!(
                            ?err,
                            height = pending_height,
                            view = pending_view,
                            block = %block_hash,
                            "failed to aggregate precommit signatures for cached QC"
                        );
                        Vec::new()
                    }
                };
                let stake_snapshot = match consensus_mode {
                    ConsensusMode::Permissioned => None,
                    ConsensusMode::Npos => {
                        let world = self.state.world_view();
                        CommitStakeSnapshot::from_roster(&world, &commit_topology)
                    }
                };
                if let Some((parent_state_root, post_state_root)) =
                    pending.parent_state_root.zip(pending.post_state_root)
                {
                    let (chain_order_hash, rechain_seq) =
                        self.vnext_chain_order_binding_for(pending_height, pending_view);
                    if let Some(derived_qc) = super::derive_block_sync_qc_from_signers(
                        block_hash,
                        pending_height,
                        pending_view,
                        lock.epoch,
                        chain_order_hash,
                        rechain_seq,
                        parent_state_root,
                        post_state_root,
                        &commit_topology,
                        consensus_mode,
                        stake_snapshot.as_ref(),
                        mode_tag,
                        signers,
                        aggregate_signature,
                    ) {
                        commit_qc = Some(derived_qc);
                    }
                } else {
                    warn!(
                        height = pending_height,
                        view = pending_view,
                        block = %block_hash,
                        "skipping derived QC cache: missing execution roots"
                    );
                }
            }
        }

        iroha_logger::info!(
            height = pending_height,
            view = pending_view,
            block = %block_hash,
            mode = ?self.consensus_mode,
            "committing with commit certificate"
        );

        let id = self.subsystems.commit.next_id();
        if pending.validated_commit_artifact.is_none()
            && let Some((parent_state_root, post_state_root)) =
                pending.parent_state_root.zip(pending.post_state_root)
        {
            pending.note_validated_commit_artifact(
                block_hash,
                pending_height,
                pending_view,
                parent_state_root,
                post_state_root,
            );
        }
        let local_outside_commit_topology = commit_topology
            .iter()
            .all(|peer| peer != self.common_config.peer.id());
        let allow_signature_index_recovery = local_outside_commit_topology && commit_qc.is_some();
        let work = CommitWork {
            id,
            block: pending.block.clone(),
            validated_commit_artifact: pending.validated_commit_artifact,
            commit_topology: commit_topology.clone(),
            signature_topology: signature_topology.clone(),
            consensus_mode,
            qc_signers: quorum_signers.clone(),
            commit_qc: commit_qc.clone(),
            allow_signature_index_recovery,
            events_sender: self.events_sender.clone(),
        };
        let inflight = CommitInFlight {
            id,
            lock,
            block_hash,
            pending,
            commit_topology,
            signature_topology,
            qc_signers: quorum_signers,
            commit_qc,
            post_commit_qc,
            enqueue_time: now,
            timeout_reported: false,
        };
        self.record_round_trace_event(super::RoundTraceEvent {
            key: super::RoundTraceKey {
                height: pending_height,
                view: pending_view,
            },
            phase: super::status::RoundPhaseTrace::Commit,
            cause: super::status::RoundEventCauseTrace::CommitRequested,
            queue_latency_ms: None,
            no_progress_wake: false,
        });
        self.start_commit_job(inflight, work)
    }
}

impl Actor {
    pub(super) fn process_commit_candidates(&mut self) {
        let _ = self.process_commit_candidates_with_trigger(CommitPipelineTrigger::Event, None);
    }

    pub(in crate::sumeragi) fn poll_commit_results(&mut self) -> bool {
        self.drain_commit_results().progress
    }

    fn report_inflight_commit_if_timed_out(&mut self, now: Instant) -> bool {
        let timeout = self.config.persistence.commit_inflight_timeout;
        if timeout.is_zero() {
            return false;
        }
        let Some(inflight) = self.subsystems.commit.inflight.as_mut() else {
            return false;
        };
        let elapsed = now.saturating_duration_since(inflight.enqueue_time);
        if elapsed < timeout {
            return false;
        }
        if inflight.timeout_reported {
            return false;
        }
        inflight.timeout_reported = true;
        let height = inflight.pending.height;
        let view = inflight.pending.view;
        let block_hash = inflight.block_hash;
        super::status::record_commit_inflight_timeout(height, view, block_hash, elapsed);
        warn!(
            height,
            view,
            block = %block_hash,
            commit_id = inflight.id,
            elapsed_ms = elapsed.as_millis(),
            timeout_ms = timeout.as_millis(),
            has_commit_qc = inflight.commit_qc.is_some(),
            quorum_signers = inflight.qc_signers.as_ref().map_or(0, BTreeSet::len),
            "inflight commit exceeded timeout; waiting for commit worker result"
        );
        true
    }

    fn commit_pipeline_budget_exhausted(
        &mut self,
        tick_deadline: Option<Instant>,
        now: Instant,
    ) -> bool {
        let Some(deadline) = tick_deadline else {
            return false;
        };
        if now < deadline {
            return false;
        }
        self.pending.commit_pipeline_wakeup = true;
        true
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn handle_validation_reject(
        &mut self,
        invalid_hash: HashOf<BlockHeader>,
        invalid_height: u64,
        invalid_view: u64,
        evidence: Option<Box<crate::sumeragi::consensus::Evidence>>,
        reason: String,
        reason_label: &'static str,
    ) {
        if let Some(pending) = self.pending.pending_blocks.remove(&invalid_hash) {
            self.subsystems.validation.inflight.remove(&invalid_hash);
            self.subsystems
                .validation
                .superseded_results
                .remove(&invalid_hash);
            self.clean_rbc_sessions_for_block(invalid_hash, pending.height);
        }
        if let Some(ev) = evidence {
            if let Err(err) = self.handle_evidence(*ev) {
                warn!(
                    ?err,
                    height = invalid_height,
                    view = invalid_view,
                    block = %invalid_hash,
                    "failed to store invalid-proposal evidence after validation reject"
                );
            }
        }
        super::status::record_validation_reject(
            reason_label,
            invalid_height,
            invalid_view,
            invalid_hash,
        );
        #[cfg(feature = "telemetry")]
        self.telemetry
            .note_validation_reject(reason_label, invalid_height, invalid_view);
        warn!(
            height = invalid_height,
            view = invalid_view,
            block = %invalid_hash,
            reason_label,
            reason = %reason,
            "triggering view change after validation rejection"
        );
        self.trigger_view_change_after_validation_reject(
            invalid_height,
            invalid_view,
            invalid_hash,
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn process_commit_candidates_with_trigger(
        &mut self,
        trigger: CommitPipelineTrigger,
        tick_deadline: Option<Instant>,
    ) -> CommitPipelineTimings {
        self.process_commit_candidates_with_trigger_inner(trigger, tick_deadline, false)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn process_commit_candidates_with_trigger_inner(
        &mut self,
        trigger: CommitPipelineTrigger,
        tick_deadline: Option<Instant>,
        include_recovery_candidates: bool,
    ) -> CommitPipelineTimings {
        let pipeline_start = Instant::now();
        let finish_timings = |timings: CommitPipelineTimings| {
            let timings = timings.finish(pipeline_start);
            crate::sumeragi::status::record_commit_pipeline_sample(
                commit_pipeline_sample_from_timings(timings),
            );
            timings
        };
        let stale_validation = self.prune_validation_inflight_without_pending();
        if stale_validation > 0 {
            debug!(
                stale_validation,
                "pruned validation inflight entries without matching pending blocks"
            );
        }
        let mut timings = CommitPipelineTimings {
            ran: true,
            ..CommitPipelineTimings::default()
        };
        let drain_start = Instant::now();
        let drain_summary = self.drain_commit_results();
        timings.drain_results += drain_start.elapsed();
        timings.drain_result_count = drain_summary.results;
        timings.drain_qc_verify_ms = drain_summary.qc_verify_ms;
        timings.drain_persist_ms = drain_summary.persist_ms;
        timings.drain_kura_store_ms = drain_summary.kura_store_ms;
        timings.drain_state_apply_ms = drain_summary.state_apply_ms;
        timings.drain_state_commit_ms = drain_summary.state_commit_ms;
        let now = Instant::now();
        let timeout_start = Instant::now();
        let _ = self.report_inflight_commit_if_timed_out(now);
        timings.abort_inflight += timeout_start.elapsed();
        if self.commit_pipeline_budget_exhausted(tick_deadline, now) {
            return finish_timings(timings);
        }

        if matches!(trigger, CommitPipelineTrigger::Event) {
            let reschedule_start = Instant::now();
            let _ = self.reschedule_stale_pending_blocks(None);
            timings.event_reschedule += reschedule_start.elapsed();
            let queue_depths = super::status::worker_queue_depth_snapshot();
            let consensus_queue_backlog = queue_depths.vote_rx > 0
                || queue_depths.block_payload_rx > 0
                || queue_depths.rbc_chunk_rx > 0
                || queue_depths.block_rx > 0;
            if consensus_queue_backlog {
                debug!(
                    vote_rx_depth = queue_depths.vote_rx,
                    block_payload_rx_depth = queue_depths.block_payload_rx,
                    rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                    block_rx_depth = queue_depths.block_rx,
                    "consensus queue backlog detected while processing commit pipeline event"
                );
            }
        }
        // Commit certificates remain authoritative, but the QC pipeline is required to
        // keep NEW_VIEW liveness (precommit QCs) and backfill telemetry.
        let enable_qc_pipeline = true;
        let da_enabled = self.runtime_da_enabled();
        let rebroadcast_cooldown = self.control_plane_rebroadcast_cooldown();
        let local_peer_id = self.common_config.peer.id().clone();

        if self.commit_candidate_blocks_len(include_recovery_candidates) == 0 {
            let inflight = self.subsystems.commit.inflight.is_some();
            if matches!(trigger, CommitPipelineTrigger::Tick) {
                super::status::note_commit_pipeline_tick(self.consensus_mode, inflight);
                #[cfg(feature = "telemetry")]
                self.telemetry
                    .note_commit_pipeline_tick(self.mode_tag(), inflight);
            }
            return finish_timings(timings);
        }

        if matches!(trigger, CommitPipelineTrigger::Tick) {
            super::status::note_commit_pipeline_tick(self.consensus_mode, true);
            #[cfg(feature = "telemetry")]
            self.telemetry
                .note_commit_pipeline_tick(self.mode_tag(), true);
        }
        if self.commit_pipeline_budget_exhausted(tick_deadline, Instant::now()) {
            return finish_timings(timings);
        }

        let world = self.state.world_view();
        let block_time = self.block_time_for_mode_from_world(&world, self.consensus_mode);
        let qc_rebuild_cooldown = block_time.max(REBROADCAST_COOLDOWN_FLOOR);
        self.pending.last_commit_pipeline_run = self.pending.last_commit_pipeline_run.max(now);
        let should_rebuild_qcs =
            now.saturating_duration_since(self.last_qc_rebuild) >= qc_rebuild_cooldown;
        if enable_qc_pipeline && should_rebuild_qcs {
            self.last_qc_rebuild = now;
            let active_commit_topology = self.effective_commit_topology();
            let rebuild_start = Instant::now();
            self.rebuild_qcs_from_cached_votes(&active_commit_topology);
            timings.qc_rebuild += rebuild_start.elapsed();
        }

        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let active_pending_exists =
            self.active_pending_blocks_len_for_tip(tip_height, tip_hash) > 0;
        let mut pending_hashes: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .filter(|(hash, pending)| {
                self.pending_block_is_commit_pipeline_candidate(
                    **hash,
                    pending,
                    tip_height,
                    tip_hash,
                    include_recovery_candidates,
                    active_pending_exists,
                )
            })
            .map(|(hash, pending)| (pending.height, pending.view, *hash))
            .collect();
        pending_hashes.sort_by(|(h1, v1, hash1), (h2, v2, hash2)| {
            (h1, Reverse(*v1), hash1).cmp(&(h2, Reverse(*v2), hash2))
        });
        for (pending_height, pending_view, hash) in pending_hashes {
            if self.commit_pipeline_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
            timings.blocks_considered = timings.blocks_considered.saturating_add(1);
            let block_start = Instant::now();
            let validation_start = Instant::now();
            let retargeted_sidecar = self.observe_certified_frontier_sidecar_mismatch_for_hash(
                pending_height,
                hash,
                "commit_pipeline_vote_roster_lookup",
            );
            if retargeted_sidecar && !self.pending.pending_blocks.contains_key(&hash) {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %hash,
                    "skipping stale pending block after certified frontier sidecar retarget"
                );
                continue;
            }
            let (consensus_mode, _, _) = self.consensus_context_for_height(pending_height);
            let commit_topology =
                self.roster_for_vote_with_mode(hash, pending_height, pending_view, consensus_mode);
            if commit_topology.is_empty() {
                let _ = self.handle_roster_unavailable_recovery(
                    pending_height,
                    pending_view,
                    Some(hash),
                    self.queue.queued_len(),
                    now,
                    ProposalDeferWarningKind::EmptyCommitTopologyFinalize,
                    "commit_pipeline_empty_commit_topology",
                );
                warn!(
                    height = pending_height,
                    view = pending_view,
                    block = %hash,
                    "deferring pending block: empty commit roster"
                );
                continue;
            }
            let inline_fallback_timeout = self.commit_validation_inline_fallback_timeout();
            // Prefer validating via background workers to keep the tick loop responsive under load.
            // Inline validation can take hundreds of milliseconds and risks stalling vote/proposal
            // handling, which in turn causes view changes and reschedules.
            let validation_outcome = self.validate_pending_block_for_voting(hash, &commit_topology);
            if matches!(validation_outcome, ValidationGateOutcome::Deferred) {
                if let Some((pending_height_snapshot, pending_view_snapshot, pending_age)) = self
                    .pending
                    .pending_blocks
                    .get(&hash)
                    .map(|pending| (pending.height, pending.view, pending.age()))
                {
                    if self.validation_inflight_elapsed(hash).is_some() {
                        if let Some(reason) =
                            self.validation_inflight_inline_reason(hash, pending_height_snapshot)
                        {
                            match reason {
                                super::validation::ValidationInflightInlineReason::WorkerDisconnected => {
                                    warn!(
                                        height = pending_height_snapshot,
                                        view = pending_view_snapshot,
                                        block = %hash,
                                        "validation worker channel disconnected; redriving pre-vote validation through vNext"
                                    );
                                }
                                super::validation::ValidationInflightInlineReason::StaleFrontier {
                                    frontier_generation,
                                } => {
                                    warn!(
                                        height = pending_height_snapshot,
                                        view = pending_view_snapshot,
                                        block = %hash,
                                        frontier_generation,
                                        "validation inflight frontier generation is stale; redriving pre-vote validation through vNext"
                                    );
                                }
                                super::validation::ValidationInflightInlineReason::Stalled {
                                    elapsed,
                                    stall_timeout,
                                } => {
                                    warn!(
                                        height = pending_height_snapshot,
                                        view = pending_view_snapshot,
                                        block = %hash,
                                        inflight_elapsed_ms = elapsed.as_millis(),
                                        worker_stall_timeout_ms = stall_timeout.as_millis(),
                                        validation_duration_ema_ms = self
                                            .validation_duration_ema()
                                            .map(|duration| duration.as_millis()),
                                        "validation inflight exceeded worker stall timeout; redriving pre-vote validation through vNext"
                                    );
                                }
                            }
                            if self.subsystems.validation.inflight.contains_key(&hash)
                                && !self
                                    .subsystems
                                    .validation
                                    .vnext_inflight
                                    .contains_key(&hash)
                            {
                                let _ = self.supersede_validation_inflight(hash);
                            }
                            if let Some((height, view, payload_hash)) =
                                self.pending.pending_blocks.get(&hash).map(|pending| {
                                    (pending.height, pending.view, pending.payload_hash)
                                })
                            {
                                debug!(
                                    height,
                                    view,
                                    block = %hash,
                                    reason = "commit_pipeline_legacy_inflight_redrive",
                                    "redriving pending validation through vNext"
                                );
                                let _ = self.drive_vnext_validation_for_pending(
                                    hash,
                                    height,
                                    view,
                                    payload_hash,
                                );
                            }
                        }
                    } else if pending_age >= inline_fallback_timeout {
                        if self.vnext_validation_owns_block(
                            hash,
                            pending_height_snapshot,
                            pending_view_snapshot,
                        ) {
                            warn!(
                                height = pending_height_snapshot,
                                view = pending_view_snapshot,
                                block = %hash,
                                pending_age_ms = pending_age.as_millis(),
                                inline_fallback_timeout_ms = inline_fallback_timeout.as_millis(),
                                "vNext-owned frontier validation exceeded legacy inline fallback timeout; keeping validation deferred"
                            );
                        } else {
                            let redrive_now = Instant::now();
                            if let Some((height, view, payload_hash, should_redrive)) =
                                self.pending.pending_blocks.get_mut(&hash).map(|pending| {
                                    let should_redrive = pending.validation_redrive_due(
                                        redrive_now,
                                        inline_fallback_timeout,
                                    );
                                    if should_redrive {
                                        pending.mark_validation_redrive(redrive_now);
                                    }
                                    (
                                        pending.height,
                                        pending.view,
                                        pending.payload_hash,
                                        should_redrive,
                                    )
                                })
                            {
                                if !should_redrive {
                                    debug!(
                                        height,
                                        view,
                                        block = %hash,
                                        pending_age_ms = pending_age.as_millis(),
                                        inline_fallback_timeout_ms =
                                            inline_fallback_timeout.as_millis(),
                                        "pending frontier validation redrive is cooling down"
                                    );
                                } else {
                                    warn!(
                                        height = pending_height_snapshot,
                                        view = pending_view_snapshot,
                                        block = %hash,
                                        pending_age_ms = pending_age.as_millis(),
                                        inline_fallback_timeout_ms =
                                            inline_fallback_timeout.as_millis(),
                                        "pending frontier validation exceeded legacy inline fallback timeout; redriving validation through vNext"
                                    );
                                    debug!(
                                        height,
                                        view,
                                        block = %hash,
                                        reason = "commit_pipeline_queue_full_redrive",
                                        "redriving pending validation through vNext"
                                    );
                                    let _ = self.drive_vnext_validation_for_pending(
                                        hash,
                                        height,
                                        view,
                                        payload_hash,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            let validation_cost = validation_start.elapsed();
            timings.validation += validation_cost;
            timings.blocks_processed = timings.blocks_processed.saturating_add(1);
            match validation_outcome {
                ValidationGateOutcome::Valid => {}
                ValidationGateOutcome::Deferred => {
                    if let Some(pending) = self.pending.pending_blocks.get(&hash) {
                        let pending_age = pending.age();
                        if pending_age >= inline_fallback_timeout {
                            let rbc_log = {
                                let key: super::rbc_store::SessionKey =
                                    (hash, pending_height, pending_view);
                                self.subsystems
                                    .da_rbc
                                    .rbc
                                    .sessions
                                    .get(&key)
                                    .map(|session| {
                                        let topology = super::network_topology::Topology::new(
                                            commit_topology.clone(),
                                        );
                                        (
                                            session.ready_signatures.len(),
                                            self.rbc_deliver_quorum(&topology),
                                            session.received_chunks(),
                                            session.total_chunks(),
                                            session.delivered,
                                            session.sent_ready,
                                            session.is_invalid(),
                                        )
                                    })
                            };
                            debug!(
                                height = pending.height,
                                view = pending.view,
                                block = %hash,
                                pending_age_ms = pending_age.as_millis(),
                                inline_fallback_timeout_ms =
                                    inline_fallback_timeout.as_millis(),
                                validation_status = ?pending.validation_status,
                                inflight_validations = self.subsystems.validation.inflight.len(),
                                validation_workers = self.subsystems.validation.work_txs.len(),
                                commit_roster_len = commit_topology.len(),
                                rbc_session = rbc_log.is_some(),
                                rbc_ready = rbc_log.as_ref().map(|entry| entry.0),
                                rbc_required = rbc_log.as_ref().map(|entry| entry.1),
                                rbc_received_chunks = rbc_log.as_ref().map(|entry| entry.2),
                                rbc_total_chunks = rbc_log.as_ref().map(|entry| entry.3),
                                rbc_delivered = rbc_log.as_ref().map(|entry| entry.4),
                                rbc_sent_ready = rbc_log.as_ref().map(|entry| entry.5),
                                rbc_invalid = rbc_log.as_ref().map(|entry| entry.6),
                                trigger = ?trigger,
                                "commit pipeline defers validation while vNext owns validation"
                            );
                        }
                    }
                    continue;
                }
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
                    continue;
                }
            }
            let (aborted, payload_available) = match self.pending.pending_blocks.get(&hash) {
                Some(snapshot) => {
                    let payload_available = da_enabled && self.payload_available_for_da(snapshot);
                    (snapshot.aborted, payload_available)
                }
                None => continue,
            };
            let kura_has_block = self.kura.get_block_height_by_hash(hash).is_some();
            let state_height = self.state.committed_height();
            let state_tip_hash = self.state.latest_block_hash_fast();
            let state_aligned = state_tip_hash.is_some_and(|tip| tip == hash)
                && usize::try_from(pending_height)
                    .is_ok_and(|pending_height| state_height >= pending_height);
            if let Some(pending) = self.pending.pending_blocks.get_mut(&hash) {
                if kura_has_block && !pending.kura_persisted {
                    pending.mark_kura_persisted();
                }
            }
            if state_aligned
                || kura::kura_and_state_aligned_for_block(
                    kura_has_block,
                    state_height,
                    state_tip_hash,
                    pending_height,
                    hash,
                )
            {
                if let Some(pending) = self.pending.pending_blocks.remove(&hash) {
                    self.subsystems.validation.inflight.remove(&hash);
                    self.subsystems.validation.vnext_inflight.remove(&hash);
                    self.subsystems.validation.superseded_results.remove(&hash);
                    self.clean_rbc_sessions_for_committed_block_if_settled(hash, pending.height);
                }
                continue;
            }
            let certified_aborted_pending = self
                .pending
                .pending_blocks
                .get(&hash)
                .is_some_and(|pending| pending.commit_qc_observed())
                || self
                    .cached_commit_qc_for_block(hash, pending_height, pending_view)
                    .is_some();
            if aborted && !certified_aborted_pending {
                debug!(
                    ?hash,
                    height = pending_height,
                    view = pending_view,
                    "skipping aborted pending block"
                );
                continue;
            }
            let topology = super::network_topology::Topology::new(commit_topology.clone());
            let local_vote_topology = super::network_topology::Topology::new(
                self.local_commit_vote_roster(pending_height, &commit_topology),
            );
            let roster_len = topology.as_ref().len();
            let min_votes_for_commit = self.commit_min_votes(&topology);
            let missing_local_data = da_enabled && !payload_available;
            let delivered = payload_available;
            let mut emit_precommit = false;
            let mut abort_due_to_kura = false;
            let mut replay_msg: Option<BlockMessage> = None;
            let mut replay_rbc_init: Option<crate::sumeragi::consensus::RbcInit> = None;
            let mut precommit_action: Option<&'static str> = None;
            let gate_start = Instant::now();
            let mut pending = match self.pending.pending_blocks.remove(&hash) {
                Some(pending) => pending,
                None => continue,
            };
            if !pending.commit_qc_observed()
                && let Some(qc) =
                    self.cached_commit_qc_for_block(hash, pending_height, pending_view)
            {
                pending.note_commit_qc_observed(qc.epoch);
            }
            let proposal_evidence_seen =
                self.slot_has_proposal_evidence(pending_height, pending_view);
            let qc_evidence_seen = pending.commit_qc_observed()
                || self.pending_block_has_qc(hash, pending_height, pending_view);
            let priority_reason = self.pending_block_validation_priority_reason(hash, &pending);
            if !qc_evidence_seen && !proposal_evidence_seen && priority_reason.is_none() {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %hash,
                    validation_status = ?pending.validation_status,
                    "deferring commit pipeline: proposal not observed for pending block"
                );
                self.pending.pending_blocks.insert(hash, pending);
                continue;
            }
            if !proposal_evidence_seen && let Some(reason) = priority_reason {
                debug!(
                    height = pending_height,
                    view = pending_view,
                    block = %hash,
                    reason,
                    "allowing commit pipeline before proposal evidence due to near-tip consensus readiness"
                );
            }
            self.pending.pending_processing.set(Some(hash));
            self.pending
                .pending_processing_parent
                .set(pending.block.header().prev_block_hash());

            let pending_age = pending.age();
            let pending_age_ms = pending_age.as_millis();
            let gate = recompute_da_gate_status(&mut pending, da_enabled, missing_local_data);
            let kura_ready = pending.kura_retry_due(now);
            let vote_epoch = self.epoch_for_height(pending_height);
            let mut commit_epoch = pending.commit_qc_epoch.unwrap_or(vote_epoch);
            let mut ready_to_finalize = pending.commit_qc_observed() && kura_ready;
            if pending.kura_aborted {
                warn!(
                    ?hash,
                    height = pending_height,
                    view = pending_view,
                    attempts = pending.kura_retry_attempts,
                    "kura persistence retries exhausted; aborting pending block"
                );
                abort_due_to_kura = true;
                precommit_action = Some("kura_aborted");
            } else if kura_ready {
                if enable_qc_pipeline
                    && !pending.local_commit_vote_emitted()
                    && !pending.commit_qc_observed()
                    && !missing_local_data
                {
                    if self.should_defer_tip_precommit_for_same_height_conflict(
                        hash,
                        pending_height,
                        pending_view,
                        vote_epoch,
                    ) {
                        debug!(
                            block = %hash,
                            height = pending_height,
                            view = pending_view,
                            epoch = vote_epoch,
                            "deferring precommit: conflicting same-height consensus evidence is pending or cached"
                        );
                        precommit_action = Some("same_height_conflict_evidence");
                    } else {
                        emit_precommit = true;
                    }
                }
            } else {
                debug!(
                    ?hash,
                    height = pending_height,
                    view = pending_view,
                    attempts = pending.kura_retry_attempts,
                    "deferring commit while awaiting kura retry window"
                );
                precommit_action = Some("kura_backoff");
            }

            let gate_cost = gate_start.elapsed();
            timings.gate += gate_cost;

            if let Some(msg) = replay_msg.take() {
                let msg = Arc::new(msg);
                let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                for peer in &commit_topology {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
                    });
                }
            }
            if let Some(init) = replay_rbc_init.take() {
                let msg = Arc::new(BlockMessage::RbcInit(init));
                let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                for peer in &commit_topology {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
                    });
                }
            }
            let gate_reason = gate.reason;
            let gate_da_enabled = gate.da_enabled;
            record_da_gate_telemetry(self.telemetry_handle(), &gate);

            if enable_qc_pipeline {
                let has_precommit_qc = qc_cache_for_subject(&self.qc_cache, hash).any(|qc| {
                    matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit)
                        && qc.height == pending_height
                        && qc.view == pending_view
                });
                if !has_precommit_qc && pending.precommit_rebroadcast_due(now, rebroadcast_cooldown)
                {
                    let rebroadcasted = self.rebroadcast_block_votes(
                        crate::sumeragi::consensus::Phase::Commit,
                        hash,
                        pending_height,
                        pending_view,
                        true,
                    );
                    if rebroadcasted > 0 {
                        pending.mark_precommit_rebroadcast(now);
                        debug!(
                            height = pending_height,
                            view = pending_view,
                            block = %hash,
                            rebroadcasted,
                            cooldown_ms = rebroadcast_cooldown.as_millis(),
                            "rebroadcasting cached precommit votes to unblock commit quorum"
                        );
                    }
                }
            }

            if abort_due_to_kura {
                pending.mark_aborted();
                self.clean_rbc_sessions_for_block(hash, pending_height);
                self.qc_cache
                    .retain(|(_, qc_hash, _, _, _, _, _), _| qc_hash != &hash);
                self.qc_signer_tally
                    .retain(|(_, qc_hash, _, _, _, _, _), _| qc_hash != &hash);
                let latest_committed_qc = self.latest_committed_qc();
                kura::reset_qcs_after_kura_abort(
                    &mut self.locked_qc,
                    &mut self.highest_qc,
                    self.state.as_ref(),
                    latest_committed_qc,
                    kura::KURA_LOCK_RESET_REASON_ABORT,
                );
                self.trigger_view_change_with_cause(
                    pending_height,
                    pending_view,
                    ViewChangeCause::CommitFailure,
                );
                self.pending.pending_processing.set(None);
                self.pending.pending_processing_parent.set(None);
                continue;
            }

            let finalize_start = Instant::now();
            let fast_timeout = self.pending_fast_path_timeout_current();
            let mut replay_commit_evidence_after_reinsert = None;
            if enable_qc_pipeline && emit_precommit {
                let parent_hash = pending.block.header().prev_block_hash();
                let pending_roots = pending.parent_state_root.zip(pending.post_state_root);
                if self.emit_precommit_vote(
                    hash,
                    pending_height,
                    pending_view,
                    vote_epoch,
                    pending.validation_status,
                    &local_vote_topology,
                    parent_hash,
                    pending_roots,
                ) {
                    pending.note_local_commit_vote_emitted();
                    self.note_frontier_owner_local_vote_emitted(hash, pending_height, pending_view);
                    replay_commit_evidence_after_reinsert = Some("local_commit_vote_emitted");
                    precommit_action = Some("emitted");
                } else if self
                    .local_same_slot_vote(
                        crate::sumeragi::consensus::Phase::Commit,
                        pending_height,
                        pending_view,
                        vote_epoch,
                    )
                    .is_some_and(|existing| existing.block_hash == hash)
                {
                    pending.note_local_commit_vote_emitted();
                    self.note_frontier_owner_local_vote_emitted(hash, pending_height, pending_view);
                    replay_commit_evidence_after_reinsert =
                        Some("local_commit_vote_already_recorded");
                    precommit_action = Some("already_recorded");
                } else {
                    precommit_action = Some("emit_failed");
                }
            }
            if precommit_action.is_none() {
                if !enable_qc_pipeline {
                    precommit_action = Some("qc_pipeline_disabled");
                } else if pending.commit_qc_observed() {
                    precommit_action = Some("commit_qc_seen");
                } else if pending.local_commit_vote_emitted() {
                    precommit_action = Some("already_sent");
                }
            }
            if let Some(action) = precommit_action {
                if pending_age >= fast_timeout && !pending.commit_qc_observed() {
                    let rbc_log = {
                        let key: super::rbc_store::SessionKey =
                            (hash, pending_height, pending_view);
                        self.subsystems
                            .da_rbc
                            .rbc
                            .sessions
                            .get(&key)
                            .map(|session| {
                                (
                                    session.ready_signatures.len(),
                                    self.rbc_deliver_quorum(&topology),
                                    session.received_chunks(),
                                    session.total_chunks(),
                                    session.delivered,
                                    session.sent_ready,
                                    session.is_invalid(),
                                )
                            })
                    };
                    debug!(
                        height = pending_height,
                        view = pending_view,
                        block = %hash,
                        action,
                        pending_age_ms = pending_age_ms,
                        fast_timeout_ms = fast_timeout.as_millis(),
                        kura_ready,
                        kura_attempts = pending.kura_retry_attempts,
                        precommit_sent = pending.local_commit_vote_emitted(),
                        commit_qc_seen = pending.commit_qc_observed(),
                        gate = ?gate_reason,
                        gate_satisfied = ?gate.satisfaction,
                        delivered,
                        missing_local_data,
                        roster_len,
                        min_votes = min_votes_for_commit,
                        rbc_session = rbc_log.is_some(),
                        rbc_ready = rbc_log.as_ref().map(|entry| entry.0),
                        rbc_required = rbc_log.as_ref().map(|entry| entry.1),
                        rbc_received_chunks = rbc_log.as_ref().map(|entry| entry.2),
                        rbc_total_chunks = rbc_log.as_ref().map(|entry| entry.3),
                        rbc_delivered = rbc_log.as_ref().map(|entry| entry.4),
                        rbc_sent_ready = rbc_log.as_ref().map(|entry| entry.5),
                        rbc_invalid = rbc_log.as_ref().map(|entry| entry.6),
                        trigger = ?trigger,
                        "precommit gating past fast timeout"
                    );
                }
            }

            if enable_qc_pipeline
                && pending.local_commit_vote_emitted()
                && !pending.commit_qc_observed()
                && !missing_local_data
                && pending.validation_status == ValidationStatus::Valid
                && self.pending_block_commit_votes_count(hash, pending_height, pending_view)
                    >= min_votes_for_commit
            {
                self.try_form_qc_from_votes(
                    crate::sumeragi::consensus::Phase::Commit,
                    hash,
                    pending_height,
                    pending_view,
                    vote_epoch,
                    &topology,
                );
                if let Some(qc) =
                    self.cached_commit_qc_for_block(hash, pending_height, pending_view)
                {
                    pending.note_commit_qc_observed(qc.epoch);
                    commit_epoch = qc.epoch;
                    if kura_ready {
                        ready_to_finalize = true;
                    }
                    debug!(
                        height = pending_height,
                        view = pending_view,
                        block = %hash,
                        epoch = qc.epoch,
                        "commit pipeline formed local commit QC from cached votes before peer recovery"
                    );
                }
            }

            if enable_qc_pipeline
                && pending.local_commit_vote_emitted()
                && !pending.commit_qc_observed()
                && !missing_local_data
                && pending.validation_status == ValidationStatus::Valid
                && pending_age >= fast_timeout
                && pending_extends_tip(
                    pending_height,
                    pending.block.header().prev_block_hash(),
                    self.state.committed_height(),
                    self.state.latest_block_hash_fast(),
                )
            {
                let recovery_targets = self.known_block_commit_qc_recovery_targets(
                    hash,
                    pending_height,
                    pending_view,
                    &commit_topology,
                );
                if self.maybe_request_known_block_commit_qc_recovery(
                    hash,
                    pending_height,
                    pending_view,
                    &recovery_targets,
                    Some(&pending),
                    "commit_pipeline_local_vote_missing_commit_qc",
                ) {
                    debug!(
                        height = pending_height,
                        view = pending_view,
                        block = %hash,
                        pending_age_ms,
                        fast_timeout_ms = fast_timeout.as_millis(),
                        "arming known-block commit-QC recovery for locally voted pending block"
                    );
                }
            }

            if ready_to_finalize {
                let qc_header = crate::sumeragi::consensus::QcHeaderRef {
                    phase: crate::sumeragi::consensus::Phase::Commit,
                    subject_block_hash: hash,
                    height: pending_height,
                    view: pending_view,
                    epoch: commit_epoch,
                };
                let _ = self.finalize_pending_block(qc_header, pending, None);
                self.pending.pending_processing.set(None);
                self.pending.pending_processing_parent.set(None);
                let finalize_cost = finalize_start.elapsed();
                timings.finalize += finalize_cost;
                continue;
            }
            let finalize_cost = finalize_start.elapsed();
            timings.finalize += finalize_cost;
            self.pending.pending_blocks.insert(hash, pending);
            if let Some(trigger) = replay_commit_evidence_after_reinsert {
                let _ = self.maybe_replay_known_block_commit_evidence(
                    hash,
                    pending_height,
                    pending_view,
                    local_vote_topology.as_ref(),
                    trigger,
                );
            }

            let cached_precommit_votes = qc_cache_for_subject(&self.qc_cache, hash)
                .find(|qc| {
                    matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit)
                        && qc.height == pending_height
                        && qc.view == pending_view
                })
                .map(|qc| precommit_vote_count(qc, roster_len));
            let total_cost = block_start.elapsed();
            if total_cost >= COMMIT_PIPELINE_BLOCK_LOG_THRESHOLD {
                iroha_logger::warn!(
                        block = %hash,
                        height = pending_height,
                        view = pending_view,
                        age_ms = pending_age_ms,
                        gate = ?gate_reason,
                        da_enabled = gate_da_enabled,
                        delivered,
                        validation_ms = validation_cost.as_millis(),
                        gate_ms = gate_cost.as_millis(),
                        finalize_ms = finalize_cost.as_millis(),
                    total_ms = total_cost.as_millis(),
                    cached_precommit_votes = cached_precommit_votes,
                    min_votes = min_votes_for_commit,
                    trigger = ?trigger,
                    "commit pipeline block processing slow"
                );
            }
            self.pending.pending_processing.set(None);
            self.pending.pending_processing_parent.set(None);
            if self.commit_pipeline_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
        }
        if matches!(trigger, CommitPipelineTrigger::Event)
            && !drain_summary.progress
            && timings.blocks_processed == 0
            && !self.pending.pending_blocks.is_empty()
        {
            self.record_round_no_progress_wake();
        }
        finish_timings(timings)
    }

    #[cfg(test)]
    #[allow(dead_code)] // Queried by unit-test-only vote-log assertions.
    pub(super) fn local_precommit_vote_for(
        &self,
        height: u64,
        view: u64,
        epoch: u64,
        topology: &super::network_topology::Topology,
    ) -> Option<crate::sumeragi::consensus::Vote> {
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology = topology_for_view(topology, height, view, mode_tag, prf_seed);
        let local_idx = self.local_validator_index_for_topology(&signature_topology)?;
        let (chain_order_hash, rechain_seq) = self
            .vnext_chain_order_binding_for_signature_topology(
                height,
                view,
                consensus_mode,
                &signature_topology,
            );
        let key = (
            crate::sumeragi::consensus::Phase::Commit,
            height,
            view,
            epoch,
            local_idx,
            chain_order_hash,
            rechain_seq,
        );
        if let Some(vote) = self.vote_log.get(&key) {
            return Some(vote.clone());
        }
        let fallback_idx = self.local_validator_index_for_topology(topology)?;
        if fallback_idx == local_idx {
            return None;
        }
        let fallback_key = (
            crate::sumeragi::consensus::Phase::Commit,
            height,
            view,
            epoch,
            fallback_idx,
            chain_order_hash,
            rechain_seq,
        );
        if let Some(vote) = self.vote_log.get(&fallback_key) {
            return Some(vote.clone());
        }

        let canonical_roster =
            super::roster::canonicalize_roster_for_mode(topology.as_ref().to_vec(), consensus_mode);
        let canonical_topology = super::network_topology::Topology::new(canonical_roster);
        let canonical_signature_topology =
            topology_for_view(&canonical_topology, height, view, mode_tag, prf_seed);
        let local_peer = self.common_config.peer.id();
        self.stored_votes()
            .find(|vote| {
                if vote.phase != crate::sumeragi::consensus::Phase::Commit
                    || vote.height != height
                    || vote.view != view
                    || vote.epoch != epoch
                {
                    return false;
                }
                let Ok(idx) = usize::try_from(vote.signer) else {
                    return false;
                };
                signature_topology
                    .as_ref()
                    .get(idx)
                    .is_some_and(|peer| peer == local_peer)
                    || topology
                        .as_ref()
                        .get(idx)
                        .is_some_and(|peer| peer == local_peer)
                    || canonical_signature_topology
                        .as_ref()
                        .get(idx)
                        .is_some_and(|peer| peer == local_peer)
                    || canonical_topology
                        .as_ref()
                        .get(idx)
                        .is_some_and(|peer| peer == local_peer)
            })
            .cloned()
    }

    fn broadcast_cached_commit_qc_to_targets_with_backpressure(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        targets: &[PeerId],
        bypass_relay_backpressure: bool,
        trigger: &'static str,
    ) -> usize {
        if !bypass_relay_backpressure
            && self.relay_backpressure_active(
                Instant::now(),
                self.control_plane_rebroadcast_cooldown(),
            )
        {
            debug!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                trigger,
                "skipping cached commit QC replay due to relay backpressure"
            );
            return 0;
        }
        let local_peer_id = self.common_config.peer.id().clone();
        let mut replayed = 0usize;
        let msg = Arc::new(BlockMessage::Qc(qc));
        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
        for peer in targets {
            if *peer == local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
            });
            replayed = replayed.saturating_add(1);
        }
        replayed
    }

    fn broadcast_certified_commit_response_to_targets(
        &mut self,
        block: &SignedBlock,
        commit_qc: &crate::sumeragi::consensus::Qc,
        targets: &[PeerId],
        trigger: &'static str,
    ) -> usize {
        let Some(response) =
            self.certified_block_fetch_response_for_block_with_qc(block, commit_qc.clone())
        else {
            debug!(
                height = block.header().height().get(),
                view = block.header().view_change_index(),
                block = %block.hash(),
                trigger,
                "skipping certified commit response broadcast: response unavailable"
            );
            return 0;
        };
        let local_peer_id = self.common_config.peer.id().clone();
        let mut targets = targets
            .iter()
            .filter(|peer| *peer != &local_peer_id)
            .cloned()
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup();

        let mut replayed = 0usize;
        for peer in targets {
            if self.dispatch_certified_block_fetch_response(peer, response.clone()) {
                replayed = replayed.saturating_add(1);
            }
        }
        replayed
    }

    pub(super) fn maybe_replay_known_block_commit_evidence(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        topology_peers: &[PeerId],
        trigger: &'static str,
    ) -> bool {
        let Some(pending) = self.pending.pending_blocks.get(&block_hash) else {
            return false;
        };
        if pending.height != height || pending.view != view || pending.aborted {
            debug!(
                height,
                view,
                block = %block_hash,
                "skipping known-block commit evidence replay for inactive pending block"
            );
            return false;
        }
        let commit_votes = self.pending_block_commit_votes_count(block_hash, height, view);
        let commit_qc = self.cached_commit_qc_for_block(block_hash, height, view);
        let has_commit_qc = commit_qc.is_some();

        let world = self.state.world_view();
        let block_time = self.block_time_for_mode_from_world(&world, self.consensus_mode);
        let cooldown = block_time.max(std::time::Duration::from_millis(200));
        let now = std::time::Instant::now();
        if !self
            .block_sync_rebroadcast_log
            .allow(block_hash, now, cooldown)
        {
            iroha_logger::trace!(
                height,
                view,
                block = %block_hash,
                cooldown_ms = cooldown.as_millis(),
                trigger,
                "skipping known-block commit evidence replay due to cooldown"
            );
            return false;
        }

        let should_replay = {
            let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash) else {
                return false;
            };
            let allow_stalled_retry = commit_votes > 0 || has_commit_qc;
            pending.should_replay_commit_evidence(
                view,
                commit_votes,
                has_commit_qc,
                allow_stalled_retry,
            )
        };
        if !should_replay {
            iroha_logger::trace!(
                height,
                view,
                block = %block_hash,
                commit_votes,
                has_commit_qc,
                trigger,
                "skipping known-block commit evidence replay: no new progress"
            );
            return false;
        }

        let targets =
            self.known_block_commit_qc_recovery_targets(block_hash, height, view, topology_peers);
        if targets.is_empty() {
            return false;
        }

        let replayed = if let Some(commit_qc) = commit_qc {
            self.broadcast_cached_commit_qc_to_targets_with_backpressure(
                commit_qc, &targets, true, trigger,
            )
        } else {
            self.rebroadcast_block_votes_to_targets_with_backpressure(
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                height,
                view,
                &targets,
                true,
                trigger,
            )
        };
        if replayed == 0 {
            return false;
        }
        let rebroadcasted_block = self.maybe_rebroadcast_known_frontier_block_for_commit_evidence(
            block_hash,
            height,
            view,
            &targets,
            commit_votes,
            has_commit_qc,
            trigger,
        );

        iroha_logger::info!(
            height,
            view,
            block = %block_hash,
            replayed,
            has_commit_qc,
            rebroadcasted_block,
            trigger,
            "replaying known-block commit evidence"
        );
        true
    }

    fn maybe_rebroadcast_known_frontier_block_for_commit_evidence(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        targets: &[PeerId],
        commit_votes: usize,
        has_commit_qc: bool,
        trigger: &'static str,
    ) -> bool {
        if !self.config.resilience.enabled
            || has_commit_qc
            || commit_votes == 0
            || height != self.committed_height_snapshot().saturating_add(1)
        {
            return false;
        }
        let Some(pending) = self.pending.pending_blocks.get(&block_hash) else {
            return false;
        };
        if pending.aborted
            || matches!(pending.validation_status, ValidationStatus::Invalid)
            || pending.height != height
            || pending.view != view
            || !pending_extends_tip(
                height,
                pending.block.header().prev_block_hash(),
                self.state.committed_height(),
                self.state.latest_block_hash_fast(),
            )
        {
            return false;
        }
        let local_peer = self.common_config.peer.id();
        let mut targets = targets
            .iter()
            .filter(|peer| *peer != local_peer)
            .cloned()
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup();
        if targets.is_empty() {
            return false;
        }

        let created = self.frontier_block_created_for_wire(&pending.block);
        let msg = Arc::new(BlockMessage::BlockCreated(created));
        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
        for peer in &targets {
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
            });
        }
        debug!(
            height,
            view,
            block = %block_hash,
            targets = targets.len(),
            commit_votes,
            trigger,
            "rebroadcasting known frontier block with commit evidence"
        );
        true
    }

    pub(super) fn known_block_commit_qc_recovery_targets(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        topology_peers: &[PeerId],
    ) -> Vec<PeerId> {
        let mut targets = topology_peers.to_vec();
        if targets.is_empty() {
            let (consensus_mode, _, _) = self.consensus_context_for_height(height);
            targets = self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
            if targets.is_empty() {
                targets = self.effective_commit_topology();
            }
        }
        targets
    }

    fn tip_extending_local_payload_known_at_height(&self, height: u64) -> bool {
        let state_height = self.state.committed_height();
        let state_tip_hash = self.state.latest_block_hash_fast();
        self.pending.pending_blocks.values().any(|pending| {
            !pending.is_retry_aborted()
                && !matches!(pending.validation_status, ValidationStatus::Invalid)
                && pending.height == height
                && super::pending_extends_tip(
                    pending.height,
                    pending.block.header().prev_block_hash(),
                    state_height,
                    state_tip_hash,
                )
        }) || self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .is_some_and(|inflight| {
                !inflight.pending.aborted
                    && !matches!(
                        inflight.pending.validation_status,
                        ValidationStatus::Invalid
                    )
                    && inflight.pending.height == height
                    && super::pending_extends_tip(
                        inflight.pending.height,
                        inflight.pending.block.header().prev_block_hash(),
                        state_height,
                        state_tip_hash,
                    )
            })
            || self.deferred_block_sync_updates.values().any(|entry| {
                let block = &entry.update.block;
                block.header().height().get() == height
                    && super::pending_extends_tip(
                        height,
                        block.header().prev_block_hash(),
                        state_height,
                        state_tip_hash,
                    )
            })
    }

    fn highest_future_frontier_recovery_evidence_height(
        &self,
        frontier_height: u64,
    ) -> Option<u64> {
        let observed_head = self.observed_recovery_qc_head().map(|qc| qc.height);
        let highest_qc = self.highest_qc.map(|qc| qc.height);
        let cached_qc = self
            .qc_cache
            .keys()
            .filter(|(_, _, height, _, _, _, _)| *height > frontier_height)
            .map(|(_, _, height, _, _, _, _)| *height)
            .max();
        let deferred_qc = self
            .deferred_missing_payload_qcs
            .values()
            .filter(|entry| entry.qc.height > frontier_height)
            .map(|entry| entry.qc.height)
            .max();
        let deferred_block_sync = self
            .deferred_block_sync_updates
            .keys()
            .filter(|(height, _, _)| *height > frontier_height)
            .map(|(height, _, _)| *height)
            .max();

        [
            observed_head,
            highest_qc,
            cached_qc,
            deferred_qc,
            deferred_block_sync,
        ]
        .into_iter()
        .flatten()
        .max()
    }

    pub(super) fn maybe_request_frontier_gap_realign_after_commit(&mut self, now: Instant) -> bool {
        let committed_height = self.committed_height_snapshot();
        let frontier_height = committed_height.saturating_add(1);
        let Some(future_evidence_height) =
            self.highest_future_frontier_recovery_evidence_height(frontier_height)
        else {
            return false;
        };
        if future_evidence_height <= frontier_height {
            return false;
        }
        if self.tip_extending_local_payload_known_at_height(frontier_height) {
            trace!(
                committed_height,
                frontier_height,
                future_evidence_height,
                "skipping post-commit frontier reanchor because a tip-extending frontier payload is already local"
            );
            return false;
        }

        let requested =
            self.request_range_pull_from_anchor(frontier_height, "frontier_gap_realign", now);
        debug!(
            committed_height,
            frontier_height,
            future_evidence_height,
            requested,
            "evaluated post-commit canonical frontier reanchor"
        );
        requested
    }

    pub(super) fn maybe_request_known_block_commit_qc_recovery(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        targets: &[PeerId],
        pending_override: Option<&PendingBlock>,
        trigger: &'static str,
    ) -> bool {
        let local_round_known = pending_override
            .map(|pending| pending.height == height && pending.view == view)
            .unwrap_or_else(|| {
                self.pending
                    .pending_blocks
                    .get(&block_hash)
                    .is_some_and(|pending| {
                        pending.height == height
                            && pending.view == view
                            && !matches!(pending.validation_status, ValidationStatus::Invalid)
                    })
                    || self
                        .local_signed_block_for_hash(block_hash)
                        .is_some_and(|block| {
                            let header = block.header();
                            header.height().get() == height && header.view_change_index() == view
                        })
            });
        if !local_round_known {
            debug!(
                height,
                view,
                block = %block_hash,
                trigger,
                "skipping known-block commit-QC recovery because the local block for this round is unavailable"
            );
            return false;
        }
        if self
            .cached_commit_qc_for_block(block_hash, height, view)
            .is_some()
        {
            trace!(
                height,
                view,
                block = %block_hash,
                trigger,
                "skipping known-block commit-QC recovery because cached commit QC is available"
            );
            return false;
        }
        let superseded_by_local_view =
            self.known_block_commit_qc_recovery_superseded_by_local_view(height, view);
        let allow_stale_view_cert_fetch = superseded_by_local_view
            && self.known_block_commit_qc_recovery_stale_view_cert_fetch_allowed(
                block_hash,
                height,
                view,
                pending_override,
            );
        if superseded_by_local_view && !allow_stale_view_cert_fetch {
            debug!(
                height,
                view,
                block = %block_hash,
                trigger,
                "skipping known-block commit-QC recovery for stale local view"
            );
            self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
            return false;
        }
        if allow_stale_view_cert_fetch {
            debug!(
                height,
                view,
                block = %block_hash,
                trigger,
                "allowing known-block commit-QC recovery for vote-backed stale local view"
            );
        }

        let filtered_targets = super::missing_block_request_targets_without_local(
            self.common_config.peer.id(),
            targets,
        );
        if filtered_targets.is_empty() {
            return false;
        }

        let now = Instant::now();
        let payload_materialized_locally = pending_override.is_some_and(|pending| {
            pending.block.hash() == block_hash
                && pending.height == height
                && pending.view == view
                && pending.validation_status == ValidationStatus::Valid
                && !pending.is_retry_aborted()
        }) || self
            .frontier_block_materialized_locally(block_hash);
        let mut request_stalled = false;
        if height == self.committed_height_snapshot().saturating_add(1) {
            let stall_window = self.frontier_slot_lag_window();
            let dwell_window = stall_window.checked_mul(2).unwrap_or(stall_window);
            let (dependency_stalled, dwell_stalled) = self
                .pending
                .missing_commit_qc_requests
                .get(&block_hash)
                .map_or((false, false), |stats| {
                    (
                        now.saturating_duration_since(stats.last_dependency_progress)
                            >= stall_window,
                        now.saturating_duration_since(stats.first_seen) >= dwell_window,
                    )
                });
            request_stalled = dependency_stalled || dwell_stalled;
            let lag_window_expired =
                !payload_materialized_locally && self.frontier_slot_lag_window_expired(height, now);
            let catchup_stalled =
                !payload_materialized_locally && (dependency_stalled || dwell_stalled);
            let mut catchup_advance = FrontierRecoveryAdvance::None;
            if catchup_stalled || lag_window_expired {
                catchup_advance = self.handle_frontier_slot_event(
                    now,
                    super::FrontierSlotEvent::OnLagWindowExpired {
                        reason: "frontier_stall_reset",
                    },
                );
            }
            if matches!(catchup_advance, FrontierRecoveryAdvance::None)
                && catchup_stalled
                && self.request_range_pull_from_anchor(height, "frontier_stall_reset_fallback", now)
            {
                catchup_advance = FrontierRecoveryAdvance::CatchUp;
            }
            if !payload_materialized_locally
                && (self.frontier_slot_allows_deep_catchup(height, "frontier_stall_reset")
                    || matches!(catchup_advance, FrontierRecoveryAdvance::CatchUp))
            {
                info!(
                    height,
                    view,
                    block = %block_hash,
                    request_stalled,
                    catchup_advance = ?catchup_advance,
                    trigger,
                    "routing known-block commit-QC recovery through frontier stall-reset catch-up"
                );
                return true;
            }
        }

        let committed_round = height <= self.committed_height_snapshot()
            && self.committed_block_hash_for_height(height) == Some(block_hash);
        let retry_window = self
            .missing_block_retry_window_with_rbc_progress(
                block_hash,
                height,
                view,
                self.rebroadcast_cooldown(),
            )
            .max(
                payload_materialized_locally
                    .then(|| self.local_payload_commit_qc_recovery_retry_window())
                    .unwrap_or(Duration::ZERO),
            )
            .max(
                committed_round
                    .then(|| self.recovery_missing_qc_reacquire_window())
                    .unwrap_or(Duration::ZERO),
            );
        let view_change_window = Some(self.known_block_commit_qc_recovery_view_change_window());
        let topology = super::network_topology::Topology::new(filtered_targets.clone());
        let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
        let decision = super::plan_missing_block_fetch_with_mode(
            &mut self.pending.missing_commit_qc_requests,
            block_hash,
            height,
            view,
            crate::sumeragi::consensus::Phase::Commit,
            super::MissingBlockPriority::Consensus,
            &BTreeSet::new(),
            &topology,
            now,
            retry_window,
            view_change_window,
            signer_fallback_attempts,
            super::MissingBlockFetchMode::AggressiveTopology,
            false,
        );
        let dwell = self
            .pending
            .missing_commit_qc_requests
            .get(&block_hash)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let dwell_ms = dwell.as_millis().try_into().unwrap_or(u64::MAX);
        let targets_len = match &decision {
            super::MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
        super::status::record_missing_block_fetch(targets_len, dwell_ms);

        let requester_roster_proof_known =
            self.requester_has_local_roster_proof(block_hash, height, view);
        match decision {
            super::MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                if height == self.committed_height_snapshot().saturating_add(1)
                    && !payload_materialized_locally
                    && !request_stalled
                    && self.try_route_missing_block_through_exact_frontier_slot(
                        block_hash, height, view, &targets,
                    )
                {
                    info!(
                        height,
                        view,
                        block = %block_hash,
                        targets = ?targets,
                        target_kind = target_kind.label(),
                        trigger,
                        retry_window_ms = retry_window.as_millis(),
                        dwell_ms,
                        "routing known-block commit-QC recovery through exact frontier body repair"
                    );
                    return true;
                }
                let request_plan =
                    known_block_commit_qc_recovery_request_plan(payload_materialized_locally);
                if request_plan.commit_qc_only {
                    super::send_missing_commit_qc_request(
                        &self.network,
                        &self.common_config.peer.id,
                        block_hash,
                        height,
                        view,
                        super::MissingBlockPriority::Consensus,
                        requester_roster_proof_known,
                        &targets,
                    );
                }
                if request_plan.body {
                    super::send_missing_block_request(
                        &self.network,
                        &self.common_config.peer.id,
                        block_hash,
                        height,
                        view,
                        super::MissingBlockPriority::Consensus,
                        requester_roster_proof_known,
                        &targets,
                    );
                }
                info!(
                    height,
                    view,
                    block = %block_hash,
                    targets = ?targets,
                    target_kind = target_kind.label(),
                    commit_qc_only = request_plan.commit_qc_only,
                    request_stalled,
                    body_request = request_plan.body,
                    trigger,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    "requesting known-block pending update to recover missing commit QC"
                );
                true
            }
            super::MissingBlockFetchDecision::NoTargets => {
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    trigger,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    "unable to request known-block pending update: no peers available"
                );
                false
            }
            super::MissingBlockFetchDecision::Backoff => {
                trace!(
                    height,
                    view,
                    block = %block_hash,
                    trigger,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    "skipping known-block commit-QC recovery during retry backoff"
                );
                false
            }
        }
    }

    pub(super) fn retry_known_block_commit_qc_requests(
        &mut self,
        now: Instant,
        tick_deadline: Option<Instant>,
    ) -> bool {
        if self.pending.missing_commit_qc_requests.is_empty() {
            return false;
        }

        let mut progress = false;
        let pending_keys: Vec<_> = self
            .pending
            .missing_commit_qc_requests
            .keys()
            .copied()
            .collect();
        for block_hash in pending_keys {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
            let Some(stats_snapshot) = self
                .pending
                .missing_commit_qc_requests
                .get(&block_hash)
                .cloned()
            else {
                continue;
            };
            let committed_height = self.committed_height_snapshot();
            if !self.missing_commit_qc_request_has_actionable_dependency(
                block_hash,
                &stats_snapshot,
                committed_height,
                now,
            ) {
                self.clear_missing_commit_qc_request(
                    &block_hash,
                    MissingBlockClearReason::Obsolete,
                );
                progress = true;
                continue;
            }
            if self.maybe_rotate_stalled_known_block_commit_qc_recovery(
                block_hash,
                &stats_snapshot,
                now,
            ) {
                progress = true;
                continue;
            }

            let targets = self.known_block_commit_qc_recovery_targets(
                block_hash,
                stats_snapshot.height,
                stats_snapshot.view,
                &[],
            );
            if self.maybe_request_known_block_commit_qc_recovery(
                block_hash,
                stats_snapshot.height,
                stats_snapshot.view,
                &targets,
                None,
                "retry_known_block_commit_qc",
            ) {
                progress = true;
            }
        }

        progress
    }

    fn known_block_commit_qc_recovery_superseded_by_local_view(
        &self,
        height: u64,
        view: u64,
    ) -> bool {
        height == self.committed_height_snapshot().saturating_add(1)
            && self
                .phase_tracker
                .current_view(height)
                .is_some_and(|current_view| current_view > view)
    }

    fn known_block_commit_qc_recovery_stale_view_cert_fetch_allowed(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        pending_override: Option<&PendingBlock>,
    ) -> bool {
        if height != self.committed_height_snapshot().saturating_add(1) {
            return false;
        }
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        if pending_override.is_some_and(|pending| {
            pending_allows_stale_view_commit_qc_fetch(
                pending, block_hash, height, view, tip_height, tip_hash,
            )
        }) {
            return true;
        }
        self.pending
            .pending_blocks
            .get(&block_hash)
            .is_some_and(|pending| {
                pending_allows_stale_view_commit_qc_fetch(
                    pending, block_hash, height, view, tip_height, tip_hash,
                )
            })
    }

    pub(super) fn known_block_commit_qc_recovery_view_change_window(&self) -> Duration {
        let quorum_timeout = self.quorum_timeout(self.runtime_da_enabled());
        let availability_timeout =
            self.availability_timeout(quorum_timeout, self.runtime_da_enabled());
        super::saturating_mul_duration(
            quorum_timeout.max(Duration::from_millis(1)),
            KNOWN_BLOCK_COMMIT_QC_VIEW_CHANGE_GRACE_MULTIPLIER,
        )
        .max(availability_timeout)
        .max(quorum_timeout.saturating_add(self.recovery_missing_qc_reacquire_window()))
    }

    fn maybe_rotate_stalled_known_block_commit_qc_recovery(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        request: &MissingBlockRequest,
        now: Instant,
    ) -> bool {
        if !matches!(request.phase, crate::sumeragi::consensus::Phase::Commit)
            || request.height != self.committed_height_snapshot().saturating_add(1)
            || request.height != self.active_consensus_round_height()
            || self.known_block_commit_qc_recovery_superseded_by_local_view(
                request.height,
                request.view,
            )
            || request.attempts < 2
            || !request.view_change_due(now)
        {
            return false;
        }

        let stall_window = self
            .frontier_slot_lag_window()
            .max(request.view_change_window.unwrap_or_default());
        let dependency_stall = now.saturating_duration_since(request.last_dependency_progress);
        if dependency_stall < stall_window {
            return false;
        }

        if self.maybe_defer_frontier_advance_for_new_view_convergence(
            request.height,
            request.view,
            dependency_stall,
            stall_window,
            now,
        ) {
            return false;
        }

        if let Some((lock, request_votes, local_commit_vote)) =
            self.vote_locked_known_block_commit_qc_recovery(request, block_hash)
        {
            debug!(
                height = request.height,
                view = request.view,
                block = %block_hash,
                locked_block = ?lock.as_ref().map(|lock| lock.block_hash),
                locked_view = ?lock.as_ref().map(|lock| lock.view),
                locked_votes = ?lock.as_ref().map(|lock| lock.vote_count),
                request_votes,
                local_commit_vote,
                conflicting_voters = ?lock.as_ref().map(|lock| lock.conflicting_voters),
                candidate_possible_votes = ?lock.as_ref().map(|lock| lock.candidate_possible_votes),
                required = ?lock.as_ref().map(|lock| lock.required),
                total_validators = ?lock.as_ref().map(|lock| lock.total_validators),
                dependency_stall_ms = dependency_stall.as_millis(),
                stall_window_ms = stall_window.as_millis(),
                "known-block commit-QC recovery remains active for vote-locked branch"
            );
            return false;
        }

        let Some(stored) = self.pending.missing_commit_qc_requests.get_mut(&block_hash) else {
            return false;
        };
        if !stored.mark_view_change_if_due(now) {
            return false;
        }
        let dwell = now.saturating_duration_since(stored.first_seen);
        warn!(
            height = stored.height,
            view = stored.view,
            block = %block_hash,
            attempts = stored.attempts,
            dwell_ms = dwell.as_millis(),
            "known-block commit-QC recovery exceeded bounded repair window; rotating frontier view"
        );
        let height = stored.height;
        let view = stored.view;
        self.trigger_view_change_with_cause(height, view, ViewChangeCause::MissingQc);
        self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
        true
    }

    fn vote_locked_known_block_commit_qc_recovery(
        &self,
        request: &MissingBlockRequest,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<(Option<SameHeightVoteLock>, usize, bool)> {
        if !self.config.resilience.enabled
            || !matches!(request.phase, crate::sumeragi::consensus::Phase::Commit)
            || request.height != self.committed_height_snapshot().saturating_add(1)
            || self
                .cached_commit_qc_for_block(block_hash, request.height, request.view)
                .is_some()
            || self.known_block_commit_qc_request_is_superseded_by_higher_new_view_quorum(
                request.height,
                request.view,
            )
        {
            return None;
        }

        let current_view = self
            .phase_tracker
            .current_view(request.height)
            .unwrap_or(request.view);
        let candidate_view = current_view.max(request.view.saturating_add(1));
        let lock =
            self.same_height_vote_lock_blocking_candidate(request.height, candidate_view, None);
        let local_commit_vote = self
            .local_same_height_vote(request.height, self.epoch_for_height(request.height))
            .is_some_and(|vote| {
                matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit)
                    && vote.block_hash == block_hash
                    && vote.view == request.view
                    && !self.local_same_height_vote_is_committed_parent_marker(
                        request.height,
                        candidate_view,
                        &vote,
                    )
            });
        if lock.is_none() && !local_commit_vote {
            return None;
        }
        if self.latest_committed_qc().is_some_and(|highest_qc| {
            lock.as_ref().is_some_and(|lock| {
                self.new_view_qc_supersedes_same_height_vote_lock(
                    request.height,
                    candidate_view,
                    highest_qc,
                    lock,
                )
            }) || (local_commit_vote
                && self.new_view_qc_supersedes_same_height_vote_conflict(
                    request.height,
                    candidate_view,
                    highest_qc,
                    block_hash,
                    request.view,
                ))
        }) {
            return None;
        }

        let vote_status = self.commit_vote_quorum_status_for_block_detail(
            block_hash,
            request.height,
            request.view,
        );
        (local_commit_vote
            || vote_status.vote_count > 0
            || lock
                .as_ref()
                .is_some_and(|lock| lock.block_hash == block_hash))
        .then_some((lock, vote_status.vote_count, local_commit_vote))
    }

    pub(super) fn maybe_emit_local_commit_vote_for_pending_event(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        commit_topology: &[PeerId],
        trigger: &'static str,
    ) -> bool {
        let now = Instant::now();
        let Some(pending) = self.pending.pending_blocks.get(&block_hash) else {
            return false;
        };
        if pending.height != height
            || pending.view != view
            || pending.aborted
            || pending.local_commit_vote_emitted()
            || pending.commit_qc_observed()
            || pending.validation_status != ValidationStatus::Valid
            || !pending.kura_retry_due(now)
        {
            return false;
        }
        let local_commit_topology = self.local_commit_vote_roster(height, commit_topology);
        if local_commit_topology.is_empty() {
            return false;
        }

        let topology = super::network_topology::Topology::new(local_commit_topology);
        let vote_epoch = self.epoch_for_height(height);
        if self.should_defer_tip_precommit_for_same_height_conflict(
            block_hash, height, view, vote_epoch,
        ) {
            debug!(
                block = %block_hash,
                height,
                view,
                epoch = vote_epoch,
                trigger,
                "deferring event-driven precommit: conflicting same-height consensus evidence is pending or cached"
            );
            return false;
        }
        let parent_hash = pending.block.header().prev_block_hash();
        let pending_roots = pending.parent_state_root.zip(pending.post_state_root);
        let emitted = self.emit_precommit_vote(
            block_hash,
            height,
            view,
            vote_epoch,
            pending.validation_status,
            &topology,
            parent_hash,
            pending_roots,
        );
        if !emitted {
            return false;
        }

        if let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash) {
            pending.note_local_commit_vote_emitted();
        }
        self.note_frontier_owner_local_vote_emitted(block_hash, height, view);
        let _ = self.maybe_replay_known_block_commit_evidence(
            block_hash,
            height,
            view,
            topology.as_ref(),
            trigger,
        );
        self.request_commit_pipeline_for_pending(
            block_hash,
            super::status::RoundEventCauseTrace::VoteReceived,
            None,
        );
        true
    }

    pub(super) fn maybe_retry_local_commit_votes_after_new_view_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        commit_topology: &[PeerId],
        trigger: &'static str,
    ) -> bool {
        if !matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView) {
            return false;
        }
        let Some(highest_qc) = qc.highest_qc else {
            return false;
        };
        if !self
            .cached_new_view_qc_extends_committed_frontier(qc.height, qc.view, qc.view, highest_qc)
        {
            return false;
        }

        let now = Instant::now();
        let candidates: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .filter_map(|(block_hash, pending)| {
                (pending.height == qc.height
                    && pending.view == qc.view
                    && !pending.aborted
                    && !pending.local_commit_vote_emitted()
                    && !pending.commit_qc_observed()
                    && pending.validation_status == ValidationStatus::Valid
                    && pending.kura_retry_due(now)
                    && pending.block.header().prev_block_hash()
                        == Some(highest_qc.subject_block_hash))
                .then_some(*block_hash)
            })
            .collect();

        let mut emitted = false;
        for block_hash in candidates {
            if self.maybe_emit_local_commit_vote_for_pending_event(
                block_hash,
                qc.height,
                qc.view,
                commit_topology,
                trigger,
            ) {
                emitted = true;
            }
        }
        emitted
    }

    pub(super) fn maybe_start_validation_for_pending_after_new_view_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        trigger: &'static str,
    ) -> bool {
        if !matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView) {
            return false;
        }
        let Some(highest_qc) = qc.highest_qc else {
            return false;
        };
        if !self
            .cached_new_view_qc_extends_committed_frontier(qc.height, qc.view, qc.view, highest_qc)
        {
            return false;
        }

        let epoch = self.epoch_for_height(qc.height);
        let candidates: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .filter_map(|(block_hash, pending)| {
                let retained = self.slot_tracker.retained_branches.contains_key(&(
                    qc.height,
                    qc.view,
                    *block_hash,
                ));
                let authoritative_owner =
                    self.authoritative_slot_owner_hash(qc.height, qc.view) == Some(*block_hash);
                let branch_evidence = retained
                    || authoritative_owner
                    || self
                        .slot_tracker
                        .proposals_seen
                        .contains(&(qc.height, qc.view))
                    || self.pending_block_has_commit_votes(*block_hash, qc.height, qc.view)
                    || self.pending_block_has_qc(*block_hash, qc.height, qc.view);
                (pending.height == qc.height
                    && pending.view == qc.view
                    && (pending.is_retired_same_height() || !pending.aborted)
                    && !pending.local_commit_vote_emitted()
                    && !pending.commit_qc_observed()
                    && pending.validation_status == ValidationStatus::Pending
                    && pending.block.header().prev_block_hash()
                        == Some(highest_qc.subject_block_hash)
                    && branch_evidence
                    && !self.should_defer_tip_precommit_for_same_height_conflict(
                        *block_hash,
                        qc.height,
                        qc.view,
                        epoch,
                    ))
                .then_some((*block_hash, pending.payload_hash))
            })
            .collect();

        let mut started = false;
        for (block_hash, payload_hash) in candidates {
            self.drop_superseded_contiguous_frontier_owner_state(
                block_hash, qc.height, qc.view, false,
            );
            let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash) else {
                continue;
            };
            pending.reactivate_retired_same_height();
            self.note_authoritative_slot_owner(qc.height, qc.view, block_hash);
            self.note_proposal_seen(qc.height, qc.view, payload_hash);
            self.drive_vnext_proposal_accepted_for_block(
                block_hash,
                qc.height,
                qc.view,
                payload_hash,
            );
            self.drive_vnext_availability_ready_for_block(block_hash, qc.height, qc.view);
            if self.drive_vnext_validation_for_pending(block_hash, qc.height, qc.view, payload_hash)
            {
                info!(
                    height = qc.height,
                    view = qc.view,
                    block = %block_hash,
                    trigger,
                    "started validation for passive same-height pending block after NEW_VIEW QC"
                );
                started = true;
            }
        }
        started
    }

    pub(super) fn maybe_start_validation_for_pending_after_cached_new_view_qc(
        &mut self,
        height: u64,
        view: u64,
        trigger: &'static str,
    ) -> bool {
        let expected_epoch = self.epoch_for_height(height);
        let qcs: Vec<_> = self
            .qc_cache
            .values()
            .filter(|qc| {
                matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView)
                    && qc.height == height
                    && qc.view == view
                    && qc.epoch == expected_epoch
            })
            .cloned()
            .collect();

        let mut started = false;
        for qc in qcs {
            if self.maybe_start_validation_for_pending_after_new_view_qc(&qc, trigger) {
                started = true;
            }
        }
        started
    }

    pub(super) fn maybe_start_validation_for_block_created_after_cached_new_view_qc(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        trigger: &'static str,
    ) -> bool {
        let expected_epoch = self.epoch_for_height(height);
        let qcs: Vec<_> = self
            .qc_cache
            .values()
            .filter(|qc| {
                matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView)
                    && qc.height == height
                    && qc.view == view
                    && qc.epoch == expected_epoch
            })
            .cloned()
            .collect();

        let mut started = false;
        for qc in qcs {
            let Some(highest_qc) = qc.highest_qc else {
                continue;
            };
            if !self.cached_new_view_qc_extends_committed_frontier(
                qc.height, qc.view, qc.view, highest_qc,
            ) {
                continue;
            }
            let epoch = self.epoch_for_height(qc.height);
            let candidate_matches =
                self.pending
                    .pending_blocks
                    .get(&block_hash)
                    .is_some_and(|pending| {
                        pending.height == qc.height
                            && pending.view == qc.view
                            && (pending.is_retired_same_height() || !pending.aborted)
                            && !pending.local_commit_vote_emitted()
                            && !pending.commit_qc_observed()
                            && pending.validation_status == ValidationStatus::Pending
                            && pending.block.header().prev_block_hash()
                                == Some(highest_qc.subject_block_hash)
                    });
            if !candidate_matches
                || self.should_defer_tip_precommit_for_same_height_conflict(
                    block_hash, qc.height, qc.view, epoch,
                )
            {
                continue;
            }

            self.note_authoritative_slot_owner(qc.height, qc.view, block_hash);
            if self.maybe_start_validation_for_pending_after_new_view_qc(&qc, trigger) {
                started = true;
            }
        }
        started
    }

    pub(super) fn request_missing_commit_vote_payloads_after_new_view_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        commit_topology: &[PeerId],
        trigger: &'static str,
    ) -> bool {
        if !matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView) {
            return false;
        }
        let Some(highest_qc) = qc.highest_qc else {
            return false;
        };
        if !self
            .cached_new_view_qc_extends_committed_frontier(qc.height, qc.view, qc.view, highest_qc)
        {
            return false;
        }

        let epoch = self.epoch_for_height(qc.height);
        let candidate_hashes: BTreeSet<_> = self
            .stored_votes()
            .filter(|vote| {
                vote.phase == crate::sumeragi::consensus::Phase::Commit
                    && vote.height == qc.height
                    && vote.view == qc.view
                    && vote.epoch == epoch
                    && !self.block_known_locally(vote.block_hash)
            })
            .map(|vote| vote.block_hash)
            .collect();
        if candidate_hashes.is_empty() {
            return false;
        }

        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(qc.height);
        let mut requested = false;
        for block_hash in candidate_hashes {
            if self.try_recover_missing_block_from_local_rbc_session(block_hash, qc.height, qc.view)
                || self.block_known_locally(block_hash)
            {
                continue;
            }
            let mut roster = self.roster_for_vote_with_mode_observing_sidecar(
                block_hash,
                qc.height,
                qc.view,
                consensus_mode,
                "new_view_qc_missing_commit_payload",
            );
            if roster.is_empty() {
                roster = self.local_commit_vote_roster(qc.height, commit_topology);
            }
            if roster.is_empty() {
                roster = self.effective_commit_topology();
            }
            if roster.is_empty() {
                continue;
            }
            let topology = super::network_topology::Topology::new(
                super::roster::canonicalize_roster_for_mode(roster, consensus_mode),
            );
            let signature_topology =
                super::topology_for_view(&topology, qc.height, qc.view, mode_tag, prf_seed);
            let accepted = self.accepted_votes_for_qc_slot(
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                qc.height,
                qc.view,
                epoch,
                &signature_topology,
            );
            let signers: BTreeSet<_> = accepted.keys().copied().collect();
            if signers.is_empty() {
                continue;
            }
            requested |= self.request_missing_commit_vote_payload(
                block_hash,
                qc.height,
                qc.view,
                &signers,
                &signature_topology,
                true,
                trigger,
            );
        }
        requested
    }

    pub(super) fn request_missing_commit_vote_payload(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        signers: &BTreeSet<ValidatorIndex>,
        topology: &super::network_topology::Topology,
        force_retry_now: bool,
        trigger: &'static str,
    ) -> bool {
        if signers.is_empty()
            || self.block_known_locally(block_hash)
            || self.try_recover_missing_block_from_local_rbc_session(block_hash, height, view)
        {
            return false;
        }
        if self.should_suppress_lock_rejected_block_fetch(
            height,
            block_hash,
            "request_missing_commit_vote_payload",
        ) {
            self.clear_missing_block_view_change(&block_hash);
            return false;
        }

        let now = Instant::now();
        let retry_window = self.missing_block_retry_window_with_rbc_progress(
            block_hash,
            height,
            view,
            self.rebroadcast_cooldown(),
        );
        let view_change_window = Some(self.quorum_timeout(self.runtime_da_enabled()));
        let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
        let decision = super::plan_missing_block_fetch_with_mode(
            &mut self.pending.missing_block_requests,
            block_hash,
            height,
            view,
            crate::sumeragi::consensus::Phase::Commit,
            super::MissingBlockPriority::Consensus,
            signers,
            topology,
            now,
            retry_window,
            view_change_window,
            signer_fallback_attempts,
            super::MissingBlockFetchMode::Default,
            force_retry_now,
        );
        let super::MissingBlockFetchDecision::Requested {
            targets,
            target_kind,
        } = decision
        else {
            return false;
        };
        let remote_targets = super::missing_block_request_targets_without_local(
            self.common_config.peer.id(),
            &targets,
        );
        if remote_targets.is_empty() {
            return false;
        }

        let routed_exact = self.request_missing_block(
            block_hash,
            height,
            view,
            super::MissingBlockPriority::Consensus,
            &remote_targets,
        );
        self.note_missing_block_height_attempt(
            block_hash,
            height,
            view,
            super::MissingBlockRecoveryStage::HashFetch,
            None,
            now,
        );
        info!(
            height,
            view,
            block = %block_hash,
            targets = remote_targets.len(),
            target_kind = target_kind.label(),
            routed_exact,
            force_retry_now,
            trigger,
            "requested missing block payload for accepted commit votes"
        );
        true
    }

    fn local_commit_vote_roster(&self, height: u64, commit_topology: &[PeerId]) -> Vec<PeerId> {
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if height == committed_height.saturating_add(1) {
            let (consensus_mode, _, _) = self.consensus_context_for_height(height);
            let live = self.roster_for_live_vote_with_mode(height, consensus_mode);
            if !live.is_empty() {
                return live;
            }
        }

        commit_topology.to_vec()
    }

    fn vote_emission_topology_for_height(
        &self,
        height: u64,
        consensus_mode: ConsensusMode,
        fallback: &super::network_topology::Topology,
    ) -> super::network_topology::Topology {
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if height == committed_height.saturating_add(1) {
            let live = self.roster_for_live_vote_with_mode(height, consensus_mode);
            if !live.is_empty() {
                return super::network_topology::Topology::new(live);
            }
        }
        fallback.clone()
    }

    #[allow(clippy::too_many_arguments)]
    fn build_vote(
        &self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        signer: ValidatorIndex,
        chain_order_binding: Option<(Hash, u64)>,
        highest_qc: Option<crate::sumeragi::consensus::QcRef>,
        roots: Option<(Hash, Hash)>,
    ) -> Option<crate::sumeragi::consensus::Vote> {
        let (parent_state_root, post_state_root) =
            if phase == crate::sumeragi::consensus::Phase::Commit {
                if let Some(roots) = roots {
                    roots
                } else {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        "missing execution roots; skipping commit vote"
                    );
                    return None;
                }
            } else {
                let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
                (zero_root, zero_root)
            };
        let (chain_order_hash, rechain_seq) = chain_order_binding.unwrap_or_else(|| {
            self.vnext_chain_order_binding_for_phase(phase, block_hash, height, view)
        });
        let mut vote = crate::sumeragi::consensus::Vote {
            phase,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            chain_order_hash,
            rechain_seq,
            highest_qc,
            signer,
            bls_sig: Vec::new(),
        };
        let (_, mode_tag, _) = self.consensus_context_for_height(height);
        if let Err(err) = sign_vote_with_local_key(
            &self.common_config.chain,
            mode_tag,
            self.common_config.key_pair.private_key(),
            &mut vote,
        ) {
            warn!(
                height,
                view,
                block = %block_hash,
                signer,
                error = %err,
                "failed to sign local consensus vote; skipping vote"
            );
            return None;
        }
        Some(vote)
    }

    fn top_up_remote_targets_to_floor(
        signature_topology: &super::network_topology::Topology,
        local_peer_id: &PeerId,
        targets: &mut Vec<PeerId>,
        remote_floor: usize,
    ) -> usize {
        if remote_floor == 0 || targets.len() >= remote_floor {
            return 0;
        }
        let mut added = 0usize;
        for peer in signature_topology.as_ref() {
            if peer == local_peer_id || targets.iter().any(|existing| existing == peer) {
                continue;
            }
            targets.push(peer.clone());
            added = added.saturating_add(1);
            if targets.len() >= remote_floor {
                break;
            }
        }
        added
    }

    fn restore_initial_precommit_collector_state(&mut self) {
        let Some(primary) = self
            .subsystems
            .propose
            .collector_plan_targets
            .first()
            .cloned()
        else {
            return;
        };
        let targets = self.subsystems.propose.collector_plan_targets.clone();
        self.subsystems.propose.collector_plan =
            Some(super::collectors::CollectorPlan::with_sent(targets, 1));
        self.subsystems.propose.collectors_contacted.clear();
        self.note_collector_contact(primary, false);
    }

    fn vote_recorded_or_queued_for_validation(
        &self,
        vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        let local_public_key = self.common_config.peer.id().public_key().clone();
        let identity_key = (
            vote.phase,
            vote.height,
            vote.view,
            vote.epoch,
            vote.signer,
            vote.chain_order_hash,
            vote.rechain_seq,
            local_public_key.clone(),
        );
        if self
            .vote_log_identities
            .get(&identity_key)
            .is_some_and(|existing| existing.block_hash == vote.block_hash)
        {
            return true;
        }
        if self
            .vote_log
            .get(&votes::vote_key(vote))
            .filter(|existing| {
                self.vote_identity_key_from_vote(existing).as_ref() == Some(&identity_key)
            })
            .is_some_and(|existing| existing.block_hash == vote.block_hash)
        {
            return true;
        }
        let verify_key =
            super::VoteVerifyKey::from_vote_with_signer_public_key(vote, Some(local_public_key));
        self.subsystems
            .vote_verify
            .pending_validation
            .contains_key(&verify_key)
            || self
                .subsystems
                .vote_verify
                .pending
                .contains_key(&verify_key)
            || self
                .subsystems
                .vote_verify
                .inflight
                .contains_key(&verify_key)
    }

    pub(super) fn candidate_commit_quorum_completes_with_local_vote(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        topology: &super::network_topology::Topology,
        signature_topology: &super::network_topology::Topology,
        local_idx: ValidatorIndex,
        pending_roots: Option<(Hash, Hash)>,
    ) -> bool {
        let Some((parent_state_root, post_state_root)) = pending_roots else {
            return false;
        };
        let epoch = self.epoch_for_height(height);
        let mut signers = self
            .accepted_votes_for_qc_slot(
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                height,
                view,
                epoch,
                signature_topology,
            )
            .into_iter()
            .filter_map(|(signer, vote)| {
                (vote.parent_state_root == parent_state_root
                    && vote.post_state_root == post_state_root)
                    .then_some(signer)
            })
            .collect::<BTreeSet<_>>();
        if signers.is_empty() || signers.contains(&local_idx) {
            return false;
        }

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        match consensus_mode {
            ConsensusMode::Permissioned => {
                let min_votes = topology.min_votes_for_commit().max(1);
                let before = signers.len();
                signers.insert(local_idx);
                before < min_votes && signers.len() >= min_votes
            }
            ConsensusMode::Npos => {
                let stake_roster =
                    self.npos_stake_roster_for_qc(topology, topology, signature_topology, height);
                if stake_roster.is_empty() {
                    return false;
                }
                let reaches_stake_quorum = |signers: &BTreeSet<ValidatorIndex>| {
                    let roster_set: BTreeSet<_> = stake_roster.iter().cloned().collect();
                    let mut signer_peers = BTreeSet::new();
                    for signer in signers {
                        let Ok(idx) = usize::try_from(*signer) else {
                            return false;
                        };
                        let Some(peer) = signature_topology.as_ref().get(idx) else {
                            return false;
                        };
                        if !roster_set.contains(peer) {
                            return false;
                        }
                        signer_peers.insert(peer.clone());
                    }
                    let world = self.state.world_view();
                    super::stake_snapshot::stake_quorum_reached_for_world(
                        &world,
                        &stake_roster,
                        &signer_peers,
                    )
                    .unwrap_or(false)
                };
                let before = reaches_stake_quorum(&signers);
                signers.insert(local_idx);
                !before && reaches_stake_quorum(&signers)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_precommit_vote(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        validation_status: ValidationStatus,
        topology: &super::network_topology::Topology,
        parent_hash: Option<HashOf<BlockHeader>>,
        pending_roots: Option<(Hash, Hash)>,
    ) -> bool {
        if self.is_observer() {
            return false;
        }
        if self.round_liveness_isolated() {
            debug!(
                height,
                view,
                block = ?block_hash,
                "skipping precommit vote while round liveness catch-up isolation is active"
            );
            return false;
        }
        self.process_committed_blocks_before_consensus("emit_precommit_vote");
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let topology = self.vote_emission_topology_for_height(height, consensus_mode, topology);
        let signature_topology = topology_for_view(&topology, height, view, mode_tag, prf_seed);
        let Some(local_idx) = self.local_validator_index_for_topology(&signature_topology) else {
            warn!(
                height,
                view,
                block = ?block_hash,
                topology_len = signature_topology.as_ref().len(),
                "skipping precommit: local peer not present in view-aligned topology"
            );
            return false;
        };
        let Ok(local_idx_usize) = usize::try_from(local_idx) else {
            return false;
        };
        if signature_topology.as_ref().get(local_idx_usize).is_none() {
            warn!(
                height,
                view,
                block = ?block_hash,
                signer = local_idx,
                topology_len = signature_topology.as_ref().len(),
                "skipping precommit: derived validator index outside view-aligned topology"
            );
            return false;
        }
        if validation_status != ValidationStatus::Valid {
            warn!(
                height,
                view,
                block = ?block_hash,
                ?validation_status,
                "skipping precommit: pending block not validated"
            );
            return false;
        }
        if self
            .local_same_slot_vote(
                crate::sumeragi::consensus::Phase::Commit,
                height,
                view,
                epoch,
            )
            .is_some_and(|existing| existing.block_hash == block_hash)
        {
            debug!(
                height,
                view,
                block = ?block_hash,
                signer = local_idx,
                "skipping precommit: already voted for this round"
            );
            return false;
        }
        let conflicting_vote = self.local_conflicting_slot_vote(height, epoch, block_hash);
        if let Some(conflict) = conflicting_vote {
            let new_view_qc_supersedes = self
                .proposal_or_new_view_highest_qc_for_slot(height, view)
                .is_some_and(|highest_qc| {
                    self.new_view_qc_supersedes_same_height_vote_conflict(
                        height,
                        view,
                        highest_qc,
                        conflict.block_hash,
                        conflict.view,
                    )
                });
            let stale_vote_can_rotate = !self.local_same_height_vote_blocks_fresh_proposal(
                height,
                view,
                &conflict,
                Instant::now(),
                true,
            );
            let conflict_has_recoverable_qc = self.same_height_block_has_recoverable_qc(
                conflict.block_hash,
                height,
                conflict.view,
            ) || self.same_height_has_recoverable_qc(height);
            if new_view_qc_supersedes || stale_vote_can_rotate {
                info!(
                    height,
                    view,
                    epoch,
                    block = ?block_hash,
                    previous_view = conflict.view,
                    previous_phase = ?conflict.phase,
                    previous_block = ?conflict.block_hash,
                    signer = local_idx,
                    new_view_qc_supersedes,
                    stale_vote_can_rotate,
                    conflict_has_recoverable_qc,
                    "allowing precommit: same-height local vote is superseded"
                );
            } else {
                warn!(
                    height,
                    view,
                    epoch,
                    block = ?block_hash,
                    previous_view = conflict.view,
                    previous_phase = ?conflict.phase,
                    previous_block = ?conflict.block_hash,
                    signer = local_idx,
                    conflict_has_recoverable_qc,
                    "skipping precommit: local validator already voted for a different same-height block"
                );
                return false;
            }
        }
        if let Some(lock) = self.locked_qc {
            let candidate = crate::sumeragi::consensus::QcHeaderRef {
                phase: crate::sumeragi::consensus::Phase::Commit,
                subject_block_hash: block_hash,
                height,
                view,
                epoch,
            };
            let locked_payload_known = self.block_known_for_lock(lock.subject_block_hash);
            let locked_hash_committed =
                self.committed_block_hash_for_height(lock.height) == Some(lock.subject_block_hash);
            if !locked_payload_known
                && candidate.height == lock.height
                && candidate.subject_block_hash != lock.subject_block_hash
            {
                let _ = self.request_missing_locked_qc_payload("emit_precommit_vote");
                warn!(
                    height,
                    view,
                    block = ?block_hash,
                    locked_height = lock.height,
                    locked_view = lock.view,
                    locked_hash = %lock.subject_block_hash,
                    "skipping precommit: locked same-height block missing locally"
                );
                return false;
            }
            if (locked_payload_known || locked_hash_committed)
                && candidate.height == lock.height
                && candidate.subject_block_hash != lock.subject_block_hash
            {
                warn!(
                    height,
                    view,
                    block = ?block_hash,
                    locked_height = lock.height,
                    locked_view = lock.view,
                    locked_hash = %lock.subject_block_hash,
                    "skipping precommit: block conflicts with locked block at the same height"
                );
                return false;
            }
            if !locked_payload_known && candidate.view <= lock.view {
                let _ = self.request_missing_locked_qc_payload("emit_precommit_vote");
                warn!(
                    height,
                    view,
                    block = ?block_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    "skipping precommit: locked QC block missing locally"
                );
                return false;
            }
            let extends_locked =
                qc_satisfies_locked_with_lookup(lock, candidate, |hash, lookup_height| {
                    if hash == block_hash && lookup_height == height {
                        parent_hash
                    } else {
                        self.parent_hash_for(hash, lookup_height)
                    }
                });
            if !extends_locked {
                warn!(
                    height,
                    view,
                    block = ?block_hash,
                    parent_hash = ?parent_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    "skipping precommit: block does not extend locked chain"
                );
                return false;
            }
        }

        let roots = pending_roots.or_else(|| {
            self.pending
                .pending_blocks
                .get(&block_hash)
                .and_then(
                    |pending| match (pending.parent_state_root, pending.post_state_root) {
                        (Some(parent), Some(post)) => Some((parent, post)),
                        _ => None,
                    },
                )
        });
        let Some(vote) = self.build_vote(
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            local_idx,
            Some(self.vnext_chain_order_binding_for_signature_topology(
                height,
                view,
                consensus_mode,
                &signature_topology,
            )),
            None,
            roots,
        ) else {
            return false;
        };
        let chain_id = self.common_config.chain.clone();
        let evidence_context = super::evidence::EvidenceValidationContext {
            topology: &topology,
            chain_id: &chain_id,
            mode_tag,
            prf_seed,
        };
        if !self.validate_and_record_vote_with_signature_result(
            &vote,
            &signature_topology,
            &evidence_context,
            mode_tag,
            None,
        ) {
            warn!(
                height,
                view,
                epoch,
                block = ?block_hash,
                signer = local_idx,
                "skipping precommit broadcast: local vote rejected before recording"
            );
            return false;
        }
        let topology_peers = topology.as_ref().to_vec();
        let roster_hash = HashOf::new(&topology_peers);
        let pops = self.cached_vote_verify_pops(&topology_peers, &roster_hash);
        let context = super::VoteProcessingContext {
            topology: topology.clone(),
            signature_topology: Arc::new(signature_topology.clone()),
            consensus_mode,
            mode_tag,
            prf_seed,
            stale_view: self.stale_view(height, view),
            pops,
        };
        self.apply_validated_vote(vote.clone(), context);
        debug!(
            height,
            view,
            epoch,
            block = %block_hash,
            signer = local_idx,
            "emitted local precommit vote"
        );

        let vote_msg = Arc::new(BlockMessage::QcVote(vote));
        let vote_encoded = Arc::new(BlockMessageWire::encode_message(vote_msg.as_ref()));
        self.ensure_collector_plan(&signature_topology, height, view);
        self.restore_initial_precommit_collector_state();
        let local_peer_id = self.common_config.peer.id().clone();
        let min_votes_for_commit = topology.min_votes_for_commit().max(1);
        let permissioned_full_fanout = matches!(consensus_mode, ConsensusMode::Permissioned);
        let mut vote_targets = if permissioned_full_fanout {
            signature_topology.as_ref().to_vec()
        } else {
            self.quorum_retransmit_targets_for_missing_votes(
                block_hash,
                height,
                view,
                topology.as_ref(),
                min_votes_for_commit,
                1,
            )
        };
        let mut fallback_to_collector_seed = false;
        if !permissioned_full_fanout && vote_targets.is_empty() {
            fallback_to_collector_seed = true;
            vote_targets = self
                .subsystems
                .propose
                .collectors_contacted
                .iter()
                .cloned()
                .collect();
        }
        vote_targets.retain(|peer| peer != &local_peer_id);
        if !permissioned_full_fanout && vote_targets.is_empty() {
            fallback_to_collector_seed = true;
            vote_targets = signature_topology.as_ref().to_vec();
            vote_targets.retain(|peer| peer != &local_peer_id);
        }
        let initial_targets = u64::try_from(vote_targets.len()).unwrap_or(u64::MAX);
        super::status::set_collectors_targeted_current(initial_targets);
        #[cfg(feature = "telemetry")]
        self.telemetry
            .set_collectors_targeted_current(initial_targets);
        iroha_logger::info!(
            height,
            view,
            block = ?block_hash,
            signer = local_idx,
            initial_targets = vote_targets.len(),
            min_votes_for_commit,
            seeded_collectors = self.subsystems.propose.collectors_contacted.len(),
            fallback_to_collector_seed,
            "sending initial precommit vote"
        );
        if vote_targets.is_empty() {
            debug!(
                height,
                view,
                block = ?block_hash,
                signer = local_idx,
                "initial precommit vote had no remote targets after local-only topology fallback"
            );
        }
        for peer in vote_targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&vote_msg),
                    Arc::clone(&vote_encoded),
                ),
            });
        }
        true
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn emit_new_view_vote(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: crate::sumeragi::consensus::QcRef,
        topology: &super::network_topology::Topology,
    ) -> bool {
        self.emit_new_view_vote_with_mode(
            height,
            view,
            highest_qc,
            topology,
            NewViewVoteEmission::Normal,
            None,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn emit_new_view_vote_to_complete_near_quorum(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: crate::sumeragi::consensus::QcRef,
        topology: &super::network_topology::Topology,
        chain_order_binding: (Hash, u64),
    ) -> bool {
        self.emit_new_view_vote_with_mode(
            height,
            view,
            highest_qc,
            topology,
            NewViewVoteEmission::CompleteNearQuorum,
            Some(chain_order_binding),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn emit_new_view_vote_with_mode(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: crate::sumeragi::consensus::QcRef,
        topology: &super::network_topology::Topology,
        emission: NewViewVoteEmission,
        chain_order_binding_override: Option<(Hash, u64)>,
    ) -> bool {
        if self.is_observer() {
            return false;
        }
        let completing_near_quorum = matches!(emission, NewViewVoteEmission::CompleteNearQuorum);
        if self.round_liveness_isolated() {
            debug!(
                height,
                view,
                highest_height = highest_qc.height,
                highest_view = highest_qc.view,
                "skipping NEW_VIEW vote while round liveness catch-up isolation is active"
            );
            return false;
        }
        if self.suppress_contiguous_frontier_owned_by_committed_edge_conflict(
            height,
            view,
            "new_view_vote",
            Instant::now(),
            false,
        ) {
            return false;
        }
        if !completing_near_quorum {
            self.process_committed_blocks_before_consensus("emit_new_view_vote");
        }
        let epoch = self.epoch_for_height(height);
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let topology = if completing_near_quorum {
            topology.clone()
        } else {
            self.vote_emission_topology_for_height(height, consensus_mode, topology)
        };
        let signature_topology = topology_for_view(&topology, height, view, mode_tag, prf_seed);
        let Some(local_idx) = self.local_validator_index_for_topology(&signature_topology) else {
            warn!(
                height,
                view,
                highest_height = highest_qc.height,
                highest_view = highest_qc.view,
                "skipping NEW_VIEW vote: local peer not present in view-aligned topology"
            );
            return false;
        };
        if highest_qc.epoch > epoch {
            warn!(
                height,
                view,
                highest_epoch = highest_qc.epoch,
                local_epoch = epoch,
                "skipping NEW_VIEW vote: highest QC epoch exceeds local epoch"
            );
            return false;
        }
        if let Some(higher_view) =
            self.local_higher_view_new_view_vote(height, view, consensus_mode, mode_tag, prf_seed)
        {
            if completing_near_quorum
                && !self
                    .local_higher_new_view_vote_conflicts_with_highest_qc(height, view, highest_qc)
            {
                debug!(
                    height,
                    view,
                    higher_view,
                    signer = local_idx,
                    highest_height = highest_qc.height,
                    highest_view = highest_qc.view,
                    highest_block = %highest_qc.subject_block_hash,
                    "allowing late NEW_VIEW vote to complete same-highest near quorum"
                );
            } else {
                info!(
                    height,
                    view,
                    higher_view,
                    signer = local_idx,
                    "skipping NEW_VIEW vote: local validator already voted in a higher view"
                );
                return false;
            }
        }
        let higher_new_view_qc = self
            .qc_cache
            .keys()
            .filter_map(|(phase, _, qc_height, qc_view, _, _, _)| {
                (*phase == crate::sumeragi::consensus::Phase::NewView
                    && *qc_height == height
                    && *qc_view > view)
                    .then_some(*qc_view)
            })
            .max();
        if let Some(higher_view) = higher_new_view_qc {
            debug!(
                height,
                view,
                higher_view,
                signer = local_idx,
                "skipping NEW_VIEW vote: higher view QC already exists"
            );
            return false;
        }
        let required = topology.min_votes_for_commit();
        if let Some(higher_view) = self
            .subsystems
            .propose
            .new_view_tracker
            .highest_quorum_view_for_height(height, required, topology.as_ref())
            && higher_view > view
        {
            if completing_near_quorum {
                debug!(
                    height,
                    view,
                    higher_view,
                    signer = local_idx,
                    "allowing late NEW_VIEW vote despite higher f+1 view support"
                );
            } else {
                info!(
                    height,
                    view,
                    higher_view,
                    signer = local_idx,
                    "skipping NEW_VIEW vote: higher view commit quorum already observed"
                );
                return false;
            }
        }
        let chain_order_binding = chain_order_binding_override.unwrap_or_else(|| {
            self.vnext_chain_order_binding_for_signature_topology(
                height,
                view,
                consensus_mode,
                &signature_topology,
            )
        });
        if let Some(existing) = self.local_same_slot_vote(
            crate::sumeragi::consensus::Phase::NewView,
            height,
            view,
            epoch,
        ) {
            if completing_near_quorum
                && Self::new_view_highest_qc_supersedes_same_slot_vote(
                    &existing,
                    height,
                    view,
                    epoch,
                    chain_order_binding,
                    highest_qc,
                )
            {
                info!(
                    height,
                    view,
                    signer = local_idx,
                    existing_highest = ?existing.highest_qc,
                    new_highest = ?highest_qc,
                    "superseding local NEW_VIEW vote with higher highest-QC to complete near quorum"
                );
            } else if existing.block_hash != highest_qc.subject_block_hash {
                warn!(
                    height,
                    view,
                    signer = local_idx,
                    existing_hash = %existing.block_hash,
                    new_hash = %highest_qc.subject_block_hash,
                    "skipping NEW_VIEW vote: local validator already voted for a different subject"
                );
            }
            if !completing_near_quorum
                || !Self::new_view_highest_qc_supersedes_same_slot_vote(
                    &existing,
                    height,
                    view,
                    epoch,
                    chain_order_binding,
                    highest_qc,
                )
            {
                return false;
            }
        }
        let Some(vote) = self.build_vote(
            crate::sumeragi::consensus::Phase::NewView,
            highest_qc.subject_block_hash,
            height,
            view,
            epoch,
            local_idx,
            Some(chain_order_binding),
            Some(highest_qc),
            None,
        ) else {
            return false;
        };
        self.handle_vote(vote.clone());
        if !self.vote_recorded_or_queued_for_validation(&vote) {
            warn!(
                height,
                view,
                epoch,
                signer = local_idx,
                highest_height = highest_qc.height,
                highest_view = highest_qc.view,
                "skipping NEW_VIEW broadcast: local vote rejected before recording"
            );
            return false;
        }

        let vote_msg = Arc::new(BlockMessage::QcVote(vote));
        let vote_encoded = Arc::new(BlockMessageWire::encode_message(vote_msg.as_ref()));
        let local_peer_id = self.common_config.peer.id().clone();
        let leader = signature_topology.leader().clone();
        self.ensure_collector_plan(&signature_topology, height, view);
        while let Some(peer) = self.next_redundant_collector() {
            self.note_collector_contact(peer.clone(), true);
        }
        let mut targets: Vec<_> = self
            .subsystems
            .propose
            .collectors_contacted
            .iter()
            .cloned()
            .collect();
        let mut fallback_to_topology = false;
        if targets.is_empty() {
            fallback_to_topology = true;
            targets = signature_topology.as_ref().to_vec();
        }
        targets.retain(|peer| peer != &local_peer_id);
        if targets.is_empty() {
            fallback_to_topology = true;
            targets = signature_topology.as_ref().to_vec();
            targets.retain(|peer| peer != &local_peer_id);
        }
        let force_frontier_recovery_topology = self.config.resilience.enabled
            && height == self.committed_height_snapshot().saturating_add(1)
            && view > 0
            && self.frontier_missing_qc_liveness_active(height, view);
        if completing_near_quorum || force_frontier_recovery_topology {
            fallback_to_topology = true;
            targets = signature_topology.as_ref().to_vec();
            targets.retain(|peer| peer != &local_peer_id);
        }
        let remote_floor = usize::from(self.subsystems.propose.collector_redundant_limit.max(1))
            .min(signature_topology.as_ref().len().saturating_sub(1));
        let mut parallel_added = 0usize;
        if !fallback_to_topology {
            let parallel = self.effective_parallel_topology_fanout();
            if parallel > 0 {
                let mut parallel_targets: Vec<_> = signature_topology
                    .topology_fanout_from_tail(parallel)
                    .into_iter()
                    .filter_map(|idx| signature_topology.as_ref().get(idx).cloned())
                    .collect();
                parallel_targets.retain(|peer| peer != &local_peer_id);
                for peer in parallel_targets {
                    if !targets.contains(&peer) {
                        targets.push(peer);
                        parallel_added = parallel_added.saturating_add(1);
                    }
                }
            }
            let _ = Self::top_up_remote_targets_to_floor(
                &signature_topology,
                &local_peer_id,
                &mut targets,
                remote_floor,
            );
        }
        if leader != local_peer_id && !targets.contains(&leader) {
            targets.push(leader.clone());
        }
        if targets.is_empty() {
            return true;
        }
        if fallback_to_topology {
            info!(
                height,
                view,
                signer = local_idx,
                leader = %leader,
                targets = targets.len(),
                "sending NEW_VIEW vote to commit topology (collector plan empty or local-only)"
            );
        } else if parallel_added > 0 {
            info!(
                height,
                view,
                signer = local_idx,
                leader = %leader,
                targets = targets.len(),
                "sending NEW_VIEW vote to collectors with parallel topology fanout"
            );
        } else {
            info!(
                height,
                view,
                signer = local_idx,
                leader = %leader,
                targets = targets.len(),
                "sending NEW_VIEW vote to collectors"
            );
        }
        for peer in targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&vote_msg),
                    Arc::clone(&vote_encoded),
                ),
            });
        }
        true
    }

    fn local_higher_new_view_vote_conflicts_with_highest_qc(
        &self,
        height: u64,
        view: u64,
        highest_qc: crate::sumeragi::consensus::QcRef,
    ) -> bool {
        let local_peer = self.common_config.peer.id();
        self.stored_votes().any(|vote| {
            vote.phase == crate::sumeragi::consensus::Phase::NewView
                && vote.height == height
                && vote.view > view
                && self
                    .vote_signer_peer(vote)
                    .as_ref()
                    .is_some_and(|peer| peer == local_peer)
                && (vote.block_hash != highest_qc.subject_block_hash
                    || vote.highest_qc != Some(highest_qc))
        })
    }

    fn local_higher_view_new_view_vote(
        &self,
        height: u64,
        view: u64,
        consensus_mode: ConsensusMode,
        mode_tag: &str,
        prf_seed: Option<[u8; 32]>,
    ) -> Option<u64> {
        let local_peer = self.common_config.peer.id();
        let mut highest: Option<u64> = None;
        for vote in self.stored_votes() {
            if vote.phase != crate::sumeragi::consensus::Phase::NewView {
                continue;
            }
            if vote.height != height || vote.view <= view {
                continue;
            }
            let roster = self.roster_for_new_view_with_mode(
                vote.block_hash,
                vote.height,
                vote.view,
                consensus_mode,
            );
            if roster.is_empty() {
                continue;
            }
            let topology = super::network_topology::Topology::new(roster);
            let signature_topology =
                topology_for_view(&topology, vote.height, vote.view, mode_tag, prf_seed);
            let signer_peer = usize::try_from(vote.signer)
                .ok()
                .and_then(|idx| signature_topology.as_ref().get(idx));
            if signer_peer == Some(local_peer) {
                highest = Some(highest.map_or(vote.view, |current| current.max(vote.view)));
            }
        }
        highest
    }

    pub(super) fn commit_vote_quorum_status_for_block_detail(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> CommitQuorumStatus {
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let commit_topology =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if commit_topology.is_empty() {
            return CommitQuorumStatus {
                vote_count: 0,
                quorum_reached: false,
                stake_quorum_missing: false,
            };
        }
        let topology = super::network_topology::Topology::new(commit_topology.clone());
        let signature_topology = topology_for_view(&topology, height, view, mode_tag, prf_seed);
        let epoch = self.epoch_for_height(height);
        let mut signers = self.qc_signers_for_votes(
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signature_topology,
        );
        let mut accepted_votes = self.accepted_votes_for_qc_slot(
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signature_topology,
        );
        let npos_stake_roster = if matches!(consensus_mode, ConsensusMode::Npos) {
            let stake_roster =
                self.npos_stake_roster_for_qc(&topology, &topology, &signature_topology, height);
            if stake_roster.is_empty() {
                return CommitQuorumStatus {
                    vote_count: signers.len(),
                    quorum_reached: false,
                    stake_quorum_missing: !signers.is_empty(),
                };
            }
            Some(stake_roster)
        } else {
            None
        };
        if !signers.is_empty() {
            let filtered = match consensus_mode {
                ConsensusMode::Permissioned => {
                    let (filtered, _groups) = super::qc::select_commit_root_signers(
                        &accepted_votes,
                        block_hash,
                        height,
                        view,
                        epoch,
                        &signers,
                    );
                    filtered
                }
                ConsensusMode::Npos => {
                    let Some(stake_roster) = npos_stake_roster.as_deref() else {
                        return CommitQuorumStatus {
                            vote_count: signers.len(),
                            quorum_reached: false,
                            stake_quorum_missing: true,
                        };
                    };
                    let world = self.state.world_view();
                    match super::qc::select_commit_root_signers_by_stake(
                        &accepted_votes,
                        block_hash,
                        height,
                        view,
                        epoch,
                        &signers,
                        &signature_topology,
                        &world,
                        stake_roster,
                    ) {
                        Ok((filtered, _groups)) => filtered,
                        Err(_) => BTreeSet::new(),
                    }
                }
            };
            accepted_votes.retain(|signer, _| filtered.contains(signer));
            signers = filtered;
        }
        let vote_count = signers.len();
        let mut stake_result: Option<Result<bool, super::stake_snapshot::StakeQuorumError>> = None;
        let quorum_reached = match consensus_mode {
            ConsensusMode::Permissioned => vote_count >= signature_topology.min_votes_for_commit(),
            ConsensusMode::Npos => {
                let result = (|| {
                    let Some(stake_roster) = npos_stake_roster.as_deref() else {
                        return Ok(false);
                    };
                    let roster_set: BTreeSet<_> = stake_roster.iter().cloned().collect();
                    let mut signer_peers = BTreeSet::new();
                    for signer in &signers {
                        let idx = usize::try_from(*signer).map_err(|_| {
                            super::stake_snapshot::StakeQuorumError::SignerOutOfRoster
                        })?;
                        let peer = signature_topology
                            .as_ref()
                            .get(idx)
                            .ok_or(super::stake_snapshot::StakeQuorumError::SignerOutOfRoster)?;
                        if !roster_set.contains(peer) {
                            return Err(super::stake_snapshot::StakeQuorumError::SignerOutOfRoster);
                        }
                        signer_peers.insert(peer.clone());
                    }
                    let world = self.state.world_view();
                    super::stake_snapshot::stake_quorum_reached_for_world(
                        &world,
                        stake_roster,
                        &signer_peers,
                    )
                })();
                stake_result = Some(result);
                stake_result
                    .as_ref()
                    .and_then(|result| result.ok())
                    .unwrap_or(false)
            }
        };
        let stake_quorum_missing = matches!(consensus_mode, ConsensusMode::Npos)
            && vote_count > 0
            && matches!(stake_result, Some(Ok(false) | Err(_)));
        CommitQuorumStatus {
            vote_count,
            quorum_reached,
            stake_quorum_missing,
        }
    }

    #[cfg(test)]
    pub(super) fn commit_vote_quorum_status_for_block(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> (usize, bool) {
        let status = self.commit_vote_quorum_status_for_block_detail(block_hash, height, view);
        (status.vote_count, status.quorum_reached)
    }

    pub(super) fn apply_commit_qc(
        &mut self,
        cert: &Qc,
        roster: &[PeerId],
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) {
        if cert.validator_set.as_slice() != roster {
            warn!(
                height,
                view,
                block = %block_hash,
                "commit certificate validator set does not match commit roster"
            );
        }
        if crate::sumeragi::consensus::qc_signer_count(cert) == 0 {
            warn!(
                height,
                view,
                block = %block_hash,
                "commit certificate has empty signer bitmap"
            );
            return;
        }
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.set_commit_qc_summary(cert);
        }
        let qc_header = crate::sumeragi::consensus::QcHeaderRef {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            height,
            view,
            epoch: cert.epoch,
        };
        self.promote_commit_anchor_qc(qc_header);
        let Some(pending) = self.pending.pending_blocks.remove(&block_hash) else {
            return;
        };
        self.subsystems.validation.inflight.remove(&block_hash);
        self.subsystems
            .validation
            .superseded_results
            .remove(&block_hash);
        let mut pending = pending;
        pending.note_commit_qc_observed(cert.epoch);
        let _ = self.finalize_pending_block(qc_header, pending, None);
    }

    pub(super) fn rebroadcast_block_votes(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        target_missing_only: bool,
    ) -> usize {
        if target_missing_only {
            let (consensus_mode, _, _) = self.consensus_context_for_height(height);
            let mut topology_peers = if matches!(phase, crate::sumeragi::consensus::Phase::NewView)
            {
                self.roster_for_new_view_with_mode(block_hash, height, view, consensus_mode)
            } else {
                self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode)
            };
            if topology_peers.is_empty() {
                topology_peers = self.effective_commit_topology();
            }
            if topology_peers.is_empty() {
                return 0;
            }
            let topology = super::network_topology::Topology::new(topology_peers.clone());
            let votes: Vec<_> = self
                .vote_log
                .values()
                .filter(|vote| {
                    vote.phase == phase
                        && vote.block_hash == block_hash
                        && vote.height == height
                        && vote.view == view
                })
                .cloned()
                .collect();
            if votes.is_empty() {
                return 0;
            }
            let min_votes_for_commit = topology.min_votes_for_commit().max(1);
            let missing_targets = self.quorum_retransmit_targets_for_missing_votes(
                block_hash,
                height,
                view,
                &topology_peers,
                min_votes_for_commit,
                votes.len(),
            );
            return self.rebroadcast_block_votes_to_targets(
                phase,
                block_hash,
                height,
                view,
                &missing_targets,
            );
        }
        if self.relay_backpressure_active(Instant::now(), self.control_plane_rebroadcast_cooldown())
        {
            debug!(
                height,
                view,
                block = ?block_hash,
                phase = ?phase,
                "skipping vote rebroadcast due to relay backpressure"
            );
            return 0;
        }
        let votes: Vec<_> = self
            .vote_log
            .values()
            .filter(|vote| {
                vote.phase == phase
                    && vote.block_hash == block_hash
                    && vote.height == height
                    && vote.view == view
            })
            .cloned()
            .collect();
        if votes.is_empty() {
            return 0;
        }
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let mut topology_peers = if matches!(phase, crate::sumeragi::consensus::Phase::NewView) {
            self.roster_for_new_view_with_mode(block_hash, height, view, consensus_mode)
        } else {
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode)
        };
        if topology_peers.is_empty() {
            topology_peers = self.effective_commit_topology();
        }
        if topology_peers.is_empty() {
            return 0;
        }
        let topology = super::network_topology::Topology::new(topology_peers);
        let signature_topology = topology_for_view(&topology, height, view, mode_tag, prf_seed);
        self.ensure_collector_plan(&signature_topology, height, view);
        while let Some(peer) = self.next_redundant_collector() {
            self.note_collector_contact(peer.clone(), true);
        }
        let mut collector_targets: Vec<_> = self
            .subsystems
            .propose
            .collectors_contacted
            .iter()
            .cloned()
            .collect();
        let mut fallback_to_topology = false;
        if collector_targets.is_empty() {
            fallback_to_topology = true;
            collector_targets = signature_topology.as_ref().to_vec();
        }
        let local_peer_id = self.common_config.peer.id().clone();
        collector_targets.retain(|peer| peer != &local_peer_id);
        if collector_targets.is_empty() {
            fallback_to_topology = true;
            collector_targets = signature_topology.as_ref().to_vec();
            collector_targets.retain(|peer| peer != &local_peer_id);
        }
        let remote_floor = usize::from(self.subsystems.propose.collector_redundant_limit.max(1))
            .min(signature_topology.as_ref().len().saturating_sub(1));
        let mut parallel_added = 0usize;
        if !fallback_to_topology {
            let parallel = self.effective_parallel_topology_fanout();
            if parallel > 0 {
                let mut parallel_targets: Vec<_> = signature_topology
                    .topology_fanout_from_tail(parallel)
                    .into_iter()
                    .filter_map(|idx| signature_topology.as_ref().get(idx).cloned())
                    .collect();
                parallel_targets.retain(|peer| peer != &local_peer_id);
                for peer in parallel_targets {
                    if collector_targets.iter().all(|existing| existing != &peer) {
                        collector_targets.push(peer);
                        parallel_added = parallel_added.saturating_add(1);
                    }
                }
            }
            let _ = Self::top_up_remote_targets_to_floor(
                &signature_topology,
                &local_peer_id,
                &mut collector_targets,
                remote_floor,
            );
        }

        debug!(
            height,
            view,
            block = ?block_hash,
            phase = ?phase,
            targets = collector_targets.len(),
            fallback_to_topology,
            parallel_added,
            target_missing_only,
            "rebroadcasting votes"
        );

        let mut rebroadcasted = 0usize;
        for vote in votes {
            let msg = match phase {
                crate::sumeragi::consensus::Phase::Prepare
                | crate::sumeragi::consensus::Phase::Commit
                | crate::sumeragi::consensus::Phase::NewView => BlockMessage::QcVote(vote),
            };
            let msg = Arc::new(msg);
            let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
            for peer in &collector_targets {
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
                });
            }
            rebroadcasted = rebroadcasted.saturating_add(1);
        }

        rebroadcasted
    }

    pub(super) fn new_view_rebroadcast_targets(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Vec<PeerId> {
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let mut targets =
            self.roster_for_new_view_with_mode(block_hash, height, view, consensus_mode);
        if targets.is_empty() {
            targets = self.effective_commit_topology();
        }
        targets
    }

    pub(super) fn rebroadcast_block_votes_to_targets(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        targets: &[PeerId],
    ) -> usize {
        self.rebroadcast_block_votes_to_targets_with_backpressure(
            phase,
            block_hash,
            height,
            view,
            targets,
            false,
            "rebroadcast_block_votes_to_targets",
        )
    }

    pub(super) fn rebroadcast_block_votes_to_targets_with_backpressure(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        targets: &[PeerId],
        bypass_relay_backpressure: bool,
        trigger: &'static str,
    ) -> usize {
        if !bypass_relay_backpressure
            && self.relay_backpressure_active(
                Instant::now(),
                self.control_plane_rebroadcast_cooldown(),
            )
        {
            debug!(
                height,
                view,
                block = ?block_hash,
                phase = ?phase,
                trigger,
                "skipping vote rebroadcast due to relay backpressure"
            );
            return 0;
        }
        let mut explicit_targets: Vec<_> = targets
            .iter()
            .filter(|peer| *peer != self.common_config.peer.id())
            .cloned()
            .collect();
        if explicit_targets.is_empty() {
            return 0;
        }
        explicit_targets.sort();
        explicit_targets.dedup();

        let votes: Vec<_> = self
            .vote_log
            .values()
            .filter(|vote| {
                vote.phase == phase
                    && vote.block_hash == block_hash
                    && vote.height == height
                    && vote.view == view
            })
            .cloned()
            .collect();
        if votes.is_empty() {
            return 0;
        }

        debug!(
            height,
            view,
            block = ?block_hash,
            phase = ?phase,
            targets = explicit_targets.len(),
            explicit_targets = true,
            "rebroadcasting votes"
        );

        let mut rebroadcasted = 0usize;
        for vote in votes {
            let msg = match phase {
                crate::sumeragi::consensus::Phase::Prepare
                | crate::sumeragi::consensus::Phase::Commit
                | crate::sumeragi::consensus::Phase::NewView => BlockMessage::QcVote(vote),
            };
            let msg = Arc::new(msg);
            let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
            for peer in &explicit_targets {
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
                });
            }
            rebroadcasted = rebroadcasted.saturating_add(1);
        }

        rebroadcasted
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn emit_exec_artifacts(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        witness: ExecWitness,
        fastpq_context: Option<crate::fastpq::FastpqWitnessContext>,
    ) {
        if self.is_observer() {
            return;
        }
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let epoch = match consensus_mode {
            ConsensusMode::Permissioned => 0,
            ConsensusMode::Npos => self.epoch_for_height(height),
        };

        let topology_peers =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if topology_peers.is_empty() {
            return;
        }
        let topology = super::network_topology::Topology::new(topology_peers);
        let signature_topology = topology_for_view(&topology, height, view, mode_tag, prf_seed);
        let Some(local_idx) = self.local_validator_index_for_topology(&signature_topology) else {
            warn!(
                height,
                view,
                block = ?block_hash,
                "skipping exec vote: local peer not present in view-aligned topology"
            );
            return;
        };
        let (collectors_k, redundant_r) = self.collector_plan_params_for_mode(consensus_mode);
        let routing_collectors_k = self.effective_collector_k_for_routing(
            consensus_mode,
            &signature_topology,
            collectors_k,
        );
        let mut collector_targets = if collectors_k == 0 {
            Vec::new()
        } else {
            super::collectors::deterministic_collectors(
                &signature_topology,
                consensus_mode,
                routing_collectors_k,
                prf_seed,
                height,
                view,
            )
        };
        if !collector_targets.is_empty() {
            let redundant_limit = if self.config.resilience.enabled
                && self.subsystems.propose.adaptive_state.applied()
            {
                signature_topology
                    .redundant_send_r_floor(self.subsystems.propose.collector_redundant_limit)
            } else {
                signature_topology.redundant_send_r_floor(redundant_r)
            };
            let limit = usize::from(redundant_limit.max(1));
            collector_targets.truncate(limit);
        }
        let mut fallback_to_topology = false;
        if collector_targets.is_empty() {
            fallback_to_topology = true;
            collector_targets = signature_topology.as_ref().to_vec();
        }
        let local_peer_id = self.common_config.peer.id().clone();
        collector_targets.retain(|peer| peer != &local_peer_id);
        if collector_targets.is_empty() {
            fallback_to_topology = true;
            collector_targets = signature_topology.as_ref().to_vec();
            collector_targets.retain(|peer| peer != &local_peer_id);
        }
        if fallback_to_topology {
            iroha_logger::info!(
                height,
                view,
                block = ?block_hash,
                signer = local_idx,
                targets = collector_targets.len(),
                "sending exec witness to commit topology (collector plan empty or local-only)"
            );
        } else {
            iroha_logger::info!(
                height,
                view,
                block = ?block_hash,
                signer = local_idx,
                targets = collector_targets.len(),
                "sending exec witness to collectors"
            );
        }
        let witness_msg = ExecWitnessMsg {
            block_hash,
            height,
            view,
            epoch,
            witness: witness.clone(),
        };
        self.handle_exec_witness(witness_msg.clone());
        let fastpq_job = crate::fastpq::lane::FastpqWitnessJob {
            block_hash,
            height,
            view,
            witness,
            context: fastpq_context.unwrap_or_default(),
        };
        if !crate::fastpq::lane::try_submit(fastpq_job) {
            debug!(
                height,
                view, "fastpq lane: witness queue full; dropping prover job"
            );
        }

        let msg = Arc::new(BlockMessage::ExecWitness(witness_msg));
        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
        for peer in collector_targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
            });
        }
    }

    /// Check whether an RBC session already has complete payload bytes for this exact slot.
    /// Status snapshots remain diagnostic-only because they do not carry byte-level
    /// availability evidence for DA gating.
    #[cfg(test)]
    fn ensure_block_matches_rbc_payload(
        sessions: &BTreeMap<super::rbc_store::SessionKey, RbcSession>,
        handle: &rbc_status::Handle,
        block_hash: &HashOf<BlockHeader>,
        height: u64,
        view: u64,
        payload_hash: &Hash,
    ) -> bool {
        rbc_payload_matches(sessions, handle, block_hash, height, view, payload_hash)
    }

    pub(super) fn local_payload_matches_hash(block: &SignedBlock, payload_hash: &Hash) -> bool {
        let payload_bytes = super::proposals::block_payload_bytes(block);
        Hash::new(&payload_bytes) == *payload_hash
    }

    /// Return true when the pending block payload is available locally or via RBC.
    pub(super) fn payload_available_for_da(&self, pending: &PendingBlock) -> bool {
        if Hash::new(pending.payload_bytes()) == pending.payload_hash {
            return true;
        }
        let key = (pending.block.hash(), pending.height, pending.view);
        self.subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .is_some_and(|session| {
                self.rbc_session_has_verified_payload_for_da(key, session, &pending.payload_hash)
            })
    }

    #[cfg(test)]
    pub(super) fn payload_available_for_da_from_sessions(
        sessions: &BTreeMap<super::rbc_store::SessionKey, RbcSession>,
        handle: &rbc_status::Handle,
        pending: &PendingBlock,
    ) -> bool {
        if Hash::new(pending.payload_bytes()) == pending.payload_hash {
            return true;
        }
        Self::ensure_block_matches_rbc_payload(
            sessions,
            handle,
            &pending.block.hash(),
            pending.height,
            pending.view,
            &pending.payload_hash,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    pub(super) fn compute_da_gate_status(
        pending: &mut PendingBlock,
        da_enabled: bool,
        missing_local_data: bool,
        manifest_cache: &mut ManifestSpoolCache,
        spool_dir: &Path,
        lane_config: &LaneConfigSnapshot,
        telemetry: Option<&crate::telemetry::Telemetry>,
    ) -> DaGateStatus {
        if !da_enabled {
            return recompute_da_gate_status(pending, da_enabled, missing_local_data);
        }
        if pending.block.da_commitments().is_some() {
            let mut cache_outcome = super::CacheOutcome::Hit;
            match manifests_available_for_block(
                manifest_cache,
                spool_dir,
                lane_config,
                &pending.block,
                &mut cache_outcome,
            ) {
                Ok(warnings) => {
                    #[cfg(feature = "telemetry")]
                    if warnings.is_empty() {
                        if let Some(telemetry) = telemetry {
                            telemetry.note_da_manifest_guard(
                                crate::telemetry::ManifestGuardResult::Allowed,
                                crate::telemetry::ManifestGuardReason::Ok,
                            );
                        }
                    }
                    for err in warnings {
                        let (lane, epoch, sequence) = err.lane_epoch_sequence();
                        let policy = lane_config.manifest_policy(LaneId::new(lane));
                        #[cfg(feature = "telemetry")]
                        if let Some(telemetry) = telemetry {
                            telemetry.note_da_manifest_guard(
                                crate::telemetry::ManifestGuardResult::Allowed,
                                manifest_guard_reason(&err),
                            );
                        }
                        warn!(
                            ?err,
                            lane,
                            epoch,
                            sequence,
                            height = pending.height,
                            view = pending.view,
                            ?policy,
                            "audit-only lane missing DA manifest; skipping availability guard"
                        );
                    }
                }
                Err(err) => {
                    let (lane, epoch, sequence) = err.lane_epoch_sequence();
                    #[cfg(feature = "telemetry")]
                    if let Some(telemetry) = telemetry {
                        telemetry.note_da_manifest_guard(
                            crate::telemetry::ManifestGuardResult::Rejected,
                            manifest_guard_reason(&err),
                        );
                    }
                    let manifest_reason = err.gate_reason();
                    let reason = if missing_local_data {
                        GateReason::MissingLocalData
                    } else {
                        manifest_reason
                    };
                    let previous = pending.last_gate;
                    let satisfaction = if missing_local_data {
                        None
                    } else {
                        super::da::gate_satisfaction(previous, Some(reason))
                    };
                    if let Some(satisfied) = satisfaction {
                        pending.last_gate_satisfied = Some(satisfied);
                    }
                    let changed = previous != Some(reason);
                    if changed {
                        super::status::record_da_gate_transition(previous, Some(reason));
                    }
                    pending.last_gate = Some(reason);
                    warn!(
                        ?err,
                        lane,
                        epoch,
                        sequence,
                        height = pending.height,
                        view = pending.view,
                        da_enabled,
                        "DA manifest unavailable or mismatched; keeping gate active"
                    );
                    return DaGateStatus {
                        reason: Some(reason),
                        satisfaction,
                        changed,
                        da_enabled,
                    };
                }
            }
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = telemetry {
                telemetry.note_da_manifest_cache(cache_outcome.as_telemetry());
            }
        }

        recompute_da_gate_status(pending, da_enabled, missing_local_data)
    }

    fn refresh_da_gate_status(&mut self, pending: &mut PendingBlock) -> DaGateStatus {
        let da_enabled = self.runtime_da_enabled();
        let missing_local_data = da_enabled && !self.payload_available_for_da(pending);
        let lane_config = self.state.nexus_snapshot().lane_config.clone();
        let telemetry = {
            #[cfg(feature = "telemetry")]
            {
                Some(&self.telemetry)
            }
            #[cfg(not(feature = "telemetry"))]
            {
                None
            }
        };

        let gate = {
            let da_rbc = &mut self.subsystems.da_rbc;
            Self::compute_da_gate_status(
                pending,
                da_enabled,
                missing_local_data,
                &mut da_rbc.manifest_cache,
                &da_rbc.spool_dir,
                &lane_config,
                telemetry,
            )
        };
        record_da_gate_telemetry(telemetry, &gate);
        gate
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn broadcast_block_created_for_block_sync(
        &mut self,
        created: super::message::BlockCreated,
        peers: &[PeerId],
    ) {
        let height = created.block.header().height().get();
        let view = created.block.header().view_change_index();
        let block_hash = created.block.hash();
        let fanout_peers =
            self.transport_fanout_targets_for_round(peers, height, view, "block_created");
        let online_peers = self
            .network
            .online_peers(|set| set.iter().map(|peer| peer.id().clone()).collect::<Vec<_>>());
        let world = self.state.world_view();
        let registered_peers = world.peers().iter().cloned().collect::<Vec<_>>();
        let trusted = self.common_config.trusted_peers.value();
        let trusted_peers: Vec<PeerId> = std::iter::once(trusted.myself.id().clone())
            .chain(trusted.others.iter().map(|peer| peer.id().clone()))
            .collect();
        let seed = created.block.hash();
        let targets = Self::block_sync_update_targets_for_peers(
            self.common_config.peer.id(),
            self.block_sync_gossip_limit,
            &fanout_peers,
            &registered_peers,
            &trusted_peers,
            &online_peers,
            seed.as_ref(),
        );
        if targets.is_empty() {
            trace!(
                height = created.block.header().height().get(),
                view = created.block.header().view_change_index(),
                block = ?block_hash,
                "skipping block payload gossip: no targets"
            );
            return;
        }
        let created = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view)
            .and_then(|proposal| {
                (block_hash == created.block.hash())
                    .then(|| self.frontier_block_created_from_proposal(&created.block, proposal))
                    .flatten()
            })
            .unwrap_or(created);
        let msg = Arc::new(BlockMessage::BlockCreated(created));
        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
        for peer in targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
            });
        }
    }

    pub(super) fn block_sync_update_targets_for_peers(
        local_peer: &PeerId,
        gossip_limit: usize,
        peers: &[PeerId],
        registered_peers: &[PeerId],
        trusted_peers: &[PeerId],
        online_peers: &[PeerId],
        seed: &[u8],
    ) -> Vec<PeerId> {
        if gossip_limit == 0 || peers.is_empty() {
            return Vec::new();
        }

        let world_peers: BTreeSet<_> = peers.iter().cloned().collect();
        // Only target online peers that remain registered or explicitly trusted (e.g., observers),
        // not unregistered strays.
        let mut registered: BTreeSet<_> = registered_peers.iter().cloned().collect();
        registered.extend(trusted_peers.iter().cloned());
        let strays: Vec<PeerId> = online_peers
            .iter()
            .filter(|peer| {
                *peer != local_peer && !world_peers.contains(*peer) && registered.contains(*peer)
            })
            .cloned()
            .collect();
        let world_online: Vec<PeerId> = online_peers
            .iter()
            .filter(|peer| *peer != local_peer && world_peers.contains(*peer))
            .cloned()
            .collect();
        let mut targets = Vec::new();
        if !strays.is_empty() {
            let ordered = Self::order_gossip_targets(strays, seed, local_peer);
            let take = usize::min(gossip_limit, ordered.len());
            targets.extend(ordered.into_iter().take(take));
        }

        let remaining = gossip_limit.saturating_sub(targets.len());
        if remaining == 0 {
            return targets;
        }
        let world_candidates_all = peers
            .iter()
            .filter(|peer| *peer != local_peer)
            .cloned()
            .collect::<Vec<_>>();
        let world_candidates = if world_online.is_empty() {
            world_candidates_all
        } else {
            world_online
        };
        if world_candidates.is_empty() {
            return targets;
        }
        let ordered = Self::order_gossip_targets(world_candidates, seed, local_peer);
        let take = usize::min(remaining, ordered.len());
        targets.extend(ordered.into_iter().take(take));
        targets
    }

    fn order_gossip_targets(
        mut peers: Vec<PeerId>,
        seed: &[u8],
        local_peer: &PeerId,
    ) -> Vec<PeerId> {
        peers.sort_by(|lhs, rhs| {
            let lhs_score = Self::gossip_target_score(seed, local_peer, lhs);
            let rhs_score = Self::gossip_target_score(seed, local_peer, rhs);
            lhs_score.cmp(&rhs_score).then_with(|| lhs.cmp(rhs))
        });
        peers
    }

    fn gossip_target_score(seed: &[u8], local_peer: &PeerId, peer: &PeerId) -> [u8; 32] {
        let mut hasher = Blake2b512::new();
        hasher.update(seed);
        hasher.update(local_peer.encode());
        hasher.update(peer.encode());
        let digest = BlakeDigest::finalize(hasher);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn broadcast_block_created(
        &mut self,
        created: super::message::BlockCreated,
        peers: &[PeerId],
    ) {
        let msg = Arc::new(BlockMessage::BlockCreated(created));
        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
        for peer in peers {
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
            });
        }
    }

    #[allow(dead_code)]
    fn rebroadcast_highest_qc_payload(
        &mut self,
        qc: &crate::sumeragi::consensus::QcHeaderRef,
        topology_peers: &[PeerId],
    ) {
        if topology_peers.is_empty() {
            return;
        }
        let block_hash = qc.subject_block_hash;
        let block_from_kura = self
            .kura
            .get_block_height_by_hash(block_hash)
            .and_then(|height| self.kura.get_block(height));
        if let Some(block) = block_from_kura {
            let block_height = block.header().height().get();
            debug!(
                height = block_height,
                view = qc.view,
                block = %block_hash,
                targets = topology_peers.len(),
                "rebroadcasting committed block for highest QC"
            );
            self.broadcast_block_created_for_block_sync(
                self.frontier_block_created_for_wire(block.as_ref()),
                topology_peers,
            );
            return;
        }

        if let Some(pending) = self.pending.pending_blocks.get(&block_hash) {
            if pending.aborted {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %block_hash,
                    "skipping rebroadcast of aborted pending block for highest QC"
                );
                return;
            }
            let block_height = pending.block.header().height().get();
            let created = self.frontier_block_created_for_wire(&pending.block);
            debug!(
                height = block_height,
                view = qc.view,
                block = %block_hash,
                targets = topology_peers.len(),
                "rebroadcasting pending block for highest QC"
            );
            self.broadcast_block_created(created, topology_peers);
        }
    }

    #[allow(dead_code)]
    fn rebroadcast_highest_qc_payload_throttled(
        &mut self,
        qc: &crate::sumeragi::consensus::QcHeaderRef,
        topology_peers: &[PeerId],
    ) {
        if topology_peers.is_empty() {
            return;
        }
        let world = self.state.world_view();
        let timeouts = super::resolve_npos_timeouts_from_world(&world, &self.config.npos);
        let cooldown = timeouts.propose.max(Duration::from_millis(50));
        let now = Instant::now();
        if !self
            .payload_rebroadcast_log
            .allow(qc.subject_block_hash, now, cooldown)
        {
            trace!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                "skipping payload rebroadcast due to cooldown"
            );
            return;
        }
        self.rebroadcast_highest_qc_payload(qc, topology_peers);
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn materialize_qc_for_header(
        &mut self,
        qc: crate::sumeragi::consensus::QcHeaderRef,
        topology_peers: &[PeerId],
    ) -> Option<crate::sumeragi::consensus::Qc> {
        if let Some(existing) = cached_qc_for(
            &self.qc_cache,
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
        ) {
            let decision = materialize_qc_decision(
                /*cached_existing*/ true, /*empty_roster*/ false,
                /*formed_after_try*/ false, /*kura_recovery_available*/ false,
                /*rebuild_from_votes_available*/ false,
            );
            debug_assert_eq!(decision.result, MaterializeQcResult::Cached);
            debug_assert!(!decision.caches_materialized_qc);
            return Some(existing);
        }
        if topology_peers.is_empty() {
            if let Some(recovered) = self.recover_highest_qc_from_kura(&qc) {
                let decision = materialize_qc_decision(
                    /*cached_existing*/ false, /*empty_roster*/ true,
                    /*formed_after_try*/ false, /*kura_recovery_available*/ true,
                    /*rebuild_from_votes_available*/ false,
                );
                debug_assert!(decision.attempts_kura_recovery);
                debug_assert_eq!(decision.result, MaterializeQcResult::Recovered);
                debug_assert!(decision.caches_materialized_qc);
                self.qc_cache
                    .insert(Self::qc_tally_key(&recovered), recovered.clone());
                return Some(recovered);
            }
            let decision = materialize_qc_decision(
                /*cached_existing*/ false, /*empty_roster*/ true,
                /*formed_after_try*/ false, /*kura_recovery_available*/ false,
                /*rebuild_from_votes_available*/ false,
            );
            debug_assert!(!decision.try_form_votes);
            debug_assert!(decision.attempts_kura_recovery);
            debug_assert_eq!(decision.result, MaterializeQcResult::None);
            debug!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                block = %qc.subject_block_hash,
                "skipping QC materialization: empty commit topology"
            );
            return None;
        }
        let topology = super::network_topology::Topology::new(topology_peers.to_vec());
        let preform_decision = materialize_qc_decision(
            /*cached_existing*/ false, /*empty_roster*/ false,
            /*formed_after_try*/ false, /*kura_recovery_available*/ false,
            /*rebuild_from_votes_available*/ false,
        );
        debug_assert!(preform_decision.try_form_votes);
        self.try_form_qc_from_votes(
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
            &topology,
        );
        if let Some(formed) = cached_qc_for(
            &self.qc_cache,
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
        ) {
            let decision = materialize_qc_decision(
                /*cached_existing*/ false, /*empty_roster*/ false,
                /*formed_after_try*/ true, /*kura_recovery_available*/ false,
                /*rebuild_from_votes_available*/ false,
            );
            debug_assert_eq!(decision.result, MaterializeQcResult::Formed);
            debug_assert!(decision.caches_materialized_qc);
            return Some(formed);
        }
        if let Some(recovered) = self.recover_highest_qc_from_kura(&qc) {
            let decision = materialize_qc_decision(
                /*cached_existing*/ false, /*empty_roster*/ false,
                /*formed_after_try*/ false, /*kura_recovery_available*/ true,
                /*rebuild_from_votes_available*/ false,
            );
            debug_assert!(decision.attempts_kura_recovery);
            debug_assert_eq!(decision.result, MaterializeQcResult::Recovered);
            debug_assert!(decision.caches_materialized_qc);
            self.qc_cache
                .insert(Self::qc_tally_key(&recovered), recovered.clone());
            return Some(recovered);
        }

        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(qc.height);
        let signature_topology =
            super::topology_for_view(&topology, qc.height, qc.view, mode_tag, prf_seed);
        let mut signers = self.qc_signers_for_votes(
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
            &signature_topology,
        );
        let mut accepted_votes = self.accepted_votes_for_qc_slot(
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
            &signature_topology,
        );
        let npos_stake_roster = if matches!(consensus_mode, ConsensusMode::Npos) {
            let stake_roster =
                self.npos_stake_roster_for_qc(&topology, &topology, &signature_topology, qc.height);
            if stake_roster.is_empty() {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    phase = ?qc.phase,
                    block = %qc.subject_block_hash,
                    "skipping QC materialization: active NPoS stake roster unavailable"
                );
                return None;
            }
            Some(stake_roster)
        } else {
            None
        };
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) && !signers.is_empty() {
            let filtered = match consensus_mode {
                ConsensusMode::Permissioned => {
                    let (filtered, _groups) = super::qc::select_commit_root_signers(
                        &accepted_votes,
                        qc.subject_block_hash,
                        qc.height,
                        qc.view,
                        qc.epoch,
                        &signers,
                    );
                    filtered
                }
                ConsensusMode::Npos => {
                    let Some(stake_roster) = npos_stake_roster.as_deref() else {
                        return None;
                    };
                    let world = self.state.world_view();
                    match super::qc::select_commit_root_signers_by_stake(
                        &accepted_votes,
                        qc.subject_block_hash,
                        qc.height,
                        qc.view,
                        qc.epoch,
                        &signers,
                        &signature_topology,
                        &world,
                        stake_roster,
                    ) {
                        Ok((filtered, _groups)) => filtered,
                        Err(err) => {
                            debug!(
                                ?err,
                                height = qc.height,
                                view = qc.view,
                                phase = ?qc.phase,
                                block = %qc.subject_block_hash,
                                "skipping QC materialization: failed to select commit root signers"
                            );
                            return None;
                        }
                    }
                }
            };
            accepted_votes.retain(|signer, _| filtered.contains(signer));
            signers = filtered;
        }
        if signers.is_empty() {
            debug!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                block = %qc.subject_block_hash,
                "skipping QC materialization: no local votes cached"
            );
            return None;
        }
        let required = signature_topology.min_votes_for_commit();
        let voting_len = signature_topology.as_ref().len();
        let voting_signers = super::voting_signer_count(&signers, voting_len);
        let quorum_met = match consensus_mode {
            ConsensusMode::Permissioned => voting_signers >= required,
            ConsensusMode::Npos => {
                let signer_peers =
                    match super::signer_peers_for_topology(&signers, &signature_topology) {
                        Ok(peers) => peers,
                        Err(err) => {
                            debug!(
                                ?err,
                                height = qc.height,
                                view = qc.view,
                                phase = ?qc.phase,
                                block = %qc.subject_block_hash,
                                "skipping QC materialization: failed to map signers"
                            );
                            return None;
                        }
                    };
                let world = self.state.world_view();
                let Some(stake_roster) = npos_stake_roster.as_deref() else {
                    return None;
                };
                super::stake_snapshot::stake_quorum_reached_for_world(
                    &world,
                    stake_roster,
                    &signer_peers,
                )
                .unwrap_or(false)
            }
        };
        if !quorum_met {
            match consensus_mode {
                ConsensusMode::Permissioned => {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = %qc.subject_block_hash,
                        voting_signers,
                        required,
                        "skipping QC materialization: quorum not reached"
                    );
                }
                ConsensusMode::Npos => {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = %qc.subject_block_hash,
                        voting_signers,
                        "skipping QC materialization: stake quorum not reached"
                    );
                }
            }
            return None;
        }
        let aggregate_signature = match super::aggregate_vote_signatures(
            &accepted_votes,
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
            &signers,
        ) {
            Ok(signature) => signature,
            Err(err) => {
                warn!(
                    height = qc.height,
                    view = qc.view,
                    phase = ?qc.phase,
                    block = %qc.subject_block_hash,
                    ?err,
                    "failed to aggregate QC signatures for materialized header"
                );
                return None;
            }
        };
        let canonical_signers =
            super::normalize_signer_indices_to_canonical(&signers, &signature_topology, &topology);
        if canonical_signers.len() != signers.len() {
            warn!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                block = %qc.subject_block_hash,
                signers = signers.len(),
                canonical = canonical_signers.len(),
                "skipping QC materialization: signer mapping to canonical roster incomplete"
            );
            return None;
        }
        let roots = if qc.phase == crate::sumeragi::consensus::Phase::Commit {
            signers.iter().find_map(|signer| {
                accepted_votes.get(signer).and_then(|vote| {
                    if vote.block_hash == qc.subject_block_hash {
                        Some((vote.parent_state_root, vote.post_state_root))
                    } else {
                        None
                    }
                })
            })
        } else {
            None
        };
        let (chain_order_hash, rechain_seq) = self
            .vnext_chain_order_binding_for_signature_topology(
                qc.height,
                qc.view,
                consensus_mode,
                &signature_topology,
            );
        let rebuilt = self.build_qc_from_signers(
            QcBuildContext {
                phase: qc.phase,
                block_hash: qc.subject_block_hash,
                height: qc.height,
                view: qc.view,
                epoch: qc.epoch,
                chain_order_hash,
                rechain_seq,
                mode_tag: mode_tag.to_string(),
                highest_qc: None,
            },
            &canonical_signers,
            &topology,
            aggregate_signature,
            roots,
        );
        let decision = materialize_qc_decision(
            /*cached_existing*/ false, /*empty_roster*/ false,
            /*formed_after_try*/ false, /*kura_recovery_available*/ false,
            /*rebuild_from_votes_available*/ true,
        );
        debug_assert_eq!(decision.result, MaterializeQcResult::Rebuilt);
        debug_assert!(decision.caches_materialized_qc);
        self.qc_cache
            .insert(Self::qc_tally_key(&rebuilt), rebuilt.clone());
        Some(rebuilt)
    }

    fn recover_qc_from_kura_block(
        qc: &crate::sumeragi::consensus::QcHeaderRef,
        kura: &Kura,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        if qc.phase != crate::sumeragi::consensus::Phase::Commit {
            return None;
        }
        let height_usize = usize::try_from(qc.height).ok()?;
        let height_nz = std::num::NonZeroUsize::new(height_usize)?;
        let block = kura.get_block(height_nz)?;
        if block.hash() != qc.subject_block_hash {
            return None;
        }
        let record = crate::sumeragi::status::precommit_signers_for_round(
            block.hash(),
            qc.height,
            qc.view,
            qc.epoch,
        )?;
        if record.bls_aggregate_signature.is_empty() {
            return None;
        }
        let consensus_mode = match record.mode_tag.as_str() {
            NPOS_TAG => ConsensusMode::Npos,
            _ => ConsensusMode::Permissioned,
        };
        super::derive_block_sync_qc_from_signers(
            block.hash(),
            qc.height,
            qc.view,
            qc.epoch,
            record.chain_order_hash,
            record.rechain_seq,
            record.parent_state_root,
            record.post_state_root,
            &record.validator_set,
            consensus_mode,
            record.stake_snapshot.as_ref(),
            &record.mode_tag,
            &record.signers,
            record.bls_aggregate_signature,
        )
    }

    fn recover_highest_qc_from_kura(
        &self,
        qc: &crate::sumeragi::consensus::QcHeaderRef,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        Self::recover_qc_from_kura_block(qc, self.kura.as_ref())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn prune_descendants_not_on_tip(
        &mut self,
        committed_height: u64,
        committed_hash: HashOf<BlockHeader>,
    ) {
        let mut stale_pending = Vec::new();
        for (hash, pending) in &self.pending.pending_blocks {
            let extends = chain_extends_tip(
                *hash,
                pending.height,
                committed_height,
                committed_hash,
                |head, height| self.parent_hash_for(head, height),
            );
            if pending.height <= committed_height || matches!(extends, Some(false) | None) {
                stale_pending.push((*hash, pending.height, pending.view));
            }
        }
        let committed_tx_hashes =
            self.committed_tip_transaction_hashes(committed_height, committed_hash);

        for (hash, height, view) in stale_pending {
            if self.pending_block_already_committed(hash, height, committed_height, committed_hash)
            {
                if let Some(tx_count) =
                    self.drop_committed_pending_block_without_requeue(hash, height, view)
                {
                    info!(
                        height,
                        view,
                        tx_count,
                        block = %hash,
                        committed_height,
                        committed_hash = %committed_hash,
                        "dropped pending block already committed on the canonical chain"
                    );
                }
                continue;
            }
            info!(
                height,
                view,
                block = %hash,
                committed_height,
                committed_hash = %committed_hash,
                "dropping pending block that diverges from committed tip"
            );
            if let Some((tx_count, requeued, failures, duplicate_failures)) = self
                .drop_stale_pending_block_skipping_committed_txs(
                    hash,
                    height,
                    view,
                    committed_tx_hashes.as_ref(),
                )
            {
                if tx_count > 0 {
                    info!(
                        height,
                        view,
                        tx_count,
                        requeued,
                        failures,
                        duplicate_failures,
                        "requeued transactions from pending block pruned off the tip"
                    );
                }
            }
        }

        let mut stale_hints = Vec::new();
        for ((height, view), hint) in &self.subsystems.propose.proposal_cache.hints {
            let extends = chain_extends_tip(
                hint.block_hash,
                *height,
                committed_height,
                committed_hash,
                |head, h| self.parent_hash_for(head, h),
            );
            if *height <= committed_height || matches!(extends, Some(false)) {
                info!(
                    height = *height,
                    view = *view,
                    block = %hint.block_hash,
                    highest_height = hint.highest_qc.height,
                    highest_hash = %hint.highest_qc.subject_block_hash,
                    committed_height,
                    committed_hash = %committed_hash,
                    "dropping cached proposal hint that diverges from committed tip"
                );
                stale_hints.push((*height, *view));
            }
        }
        for (height, view) in stale_hints {
            // Keep proposals_seen so we don't re-propose in the same view after divergence.
            self.subsystems
                .propose
                .proposal_cache
                .pop_hint(height, view);
        }

        let mut stale_proposals = Vec::new();
        for ((height, view), proposal) in &self.subsystems.propose.proposal_cache.proposals {
            let parent_height = height.saturating_sub(1);
            let extends = chain_extends_tip(
                proposal.header.parent_hash,
                parent_height,
                committed_height,
                committed_hash,
                |head, h| self.parent_hash_for(head, h),
            );
            if *height <= committed_height || matches!(extends, Some(false)) {
                info!(
                    height = *height,
                    view = *view,
                    parent = %proposal.header.parent_hash,
                    committed_height,
                    committed_hash = %committed_hash,
                    "dropping cached proposal that diverges from committed tip"
                );
                stale_proposals.push((*height, *view));
            }
        }
        for (height, view) in stale_proposals {
            // Keep proposals_seen so we don't re-propose in the same view after divergence.
            self.subsystems
                .propose
                .proposal_cache
                .pop_proposal(height, view);
        }

        let mut stale_qcs: Vec<QcVoteKey> = Vec::new();
        for (phase, hash, height, view, epoch, chain_order_hash, rechain_seq) in
            self.qc_cache.keys()
        {
            let extends = chain_extends_tip(
                *hash,
                *height,
                committed_height,
                committed_hash,
                |head, h| self.parent_hash_for(head, h),
            );
            let drop_entry = *height < committed_height || matches!(extends, Some(false) | None);
            if drop_entry {
                stale_qcs.push((
                    *phase,
                    *hash,
                    *height,
                    *view,
                    *epoch,
                    *chain_order_hash,
                    *rechain_seq,
                ));
            }
        }
        for key in stale_qcs {
            let _ = self.qc_cache.remove(&key);
            let _ = self.qc_signer_tally.remove(&key);
        }
    }

    fn committed_tip_transaction_hashes(
        &self,
        committed_height: u64,
        committed_hash: HashOf<BlockHeader>,
    ) -> Option<BTreeSet<HashOf<SignedTransaction>>> {
        if let Some(pending) = self
            .pending
            .pending_blocks
            .get(&committed_hash)
            .filter(|pending| pending.height == committed_height)
        {
            return Some(super::block_external_transaction_hashes(&pending.block));
        }
        let height_usize = usize::try_from(committed_height).ok()?;
        let height_nz = NonZeroUsize::new(height_usize)?;
        let block = self.kura.get_block(height_nz)?;
        (block.hash() == committed_hash).then(|| super::block_external_transaction_hashes(&block))
    }

    fn pending_block_already_committed(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        committed_height: u64,
        committed_hash: HashOf<BlockHeader>,
    ) -> bool {
        if height == committed_height && block_hash == committed_hash {
            return true;
        }
        self.kura
            .get_block_height_by_hash(block_hash)
            .and_then(|stored_height| u64::try_from(stored_height.get()).ok())
            .is_some_and(|stored_height| stored_height == height)
    }

    fn drop_committed_pending_block_without_requeue(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<usize> {
        let pending = self.pending.pending_blocks.remove(&block_hash)?;
        let tx_count = pending.block.external_entrypoints_cloned().count();
        self.pending.pending_fetch_requests.remove(&block_hash);
        self.pending.pending_block_body_requests.remove(&block_hash);
        self.clear_validation_ownership_for_block(block_hash);
        self.clean_rbc_sessions_for_block(block_hash, height);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        Some(tx_count)
    }

    fn clean_rbc_sessions_for_block_inner(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        purge_persisted_sessions: bool,
    ) {
        let telemetry = self.telemetry_handle().cloned();
        let live_session_keys: BTreeSet<_> = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .keys()
            .filter(|(hash, _, _)| *hash == block_hash)
            .copied()
            .collect();
        let verified_live_payload_keys: BTreeSet<_> = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .iter()
            .filter_map(|(key, session)| {
                (key.0 == block_hash
                    && self.rbc_session_has_local_authoritative_payload_for_progress(*key, session))
                .then_some(*key)
            })
            .collect();
        let delivered_payload_fallbacks = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .iter()
            .filter_map(|(key, session)| {
                ((*key).0 == block_hash && session.delivered)
                    .then_some(*key)
                    .and_then(|key| {
                        self.rbc_session_authoritative_payload_bytes_for_telemetry(key, session)
                            .map(|bytes| (key, bytes))
                    })
            })
            .collect();
        let pending_keys: Vec<_> = self
            .subsystems
            .da_rbc
            .rbc
            .pending
            .keys()
            .filter(|(hash, _, _)| *hash == block_hash)
            .copied()
            .collect();
        for key in pending_keys {
            self.clear_pending_rbc(&key);
        }
        let chunk_store = if self.ensure_rbc_chunk_store() {
            self.subsystems.da_rbc.rbc.chunk_store.as_ref()
        } else {
            None
        };
        let (lane_totals, dataspace_totals) = super::drain_rbc_state_for_block(
            block_hash,
            &mut self.subsystems.da_rbc.rbc.sessions,
            &mut self.subsystems.da_rbc.rbc.pending,
            &mut self.subsystems.da_rbc.rbc.session_rosters,
            &mut self.subsystems.da_rbc.rbc.session_roster_sources,
            Some(&mut self.subsystems.da_rbc.rbc.payload_metric_recorded_sessions),
            &self.subsystems.da_rbc.rbc.status_handle,
            telemetry.as_ref(),
            Some(&delivered_payload_fallbacks),
            chunk_store,
            purge_persisted_sessions,
        );
        self.deferred_votes.remove(&block_hash);
        self.deferred_qcs
            .retain(|(_, hash, _, _, _, _, _), _| *hash != block_hash);
        self.deferred_missing_payload_qcs
            .retain(|(_, hash, _, _, _, _, _), _| *hash != block_hash);
        self.quarantined_block_sync_qcs
            .retain(|(_, hash, _, _, _, _, _), _| *hash != block_hash);
        let orphan_keys: Vec<_> = self
            .collect_rbc_keys_for_block(block_hash)
            .into_iter()
            .collect();
        for key in orphan_keys {
            let live_session_payload_verified =
                !live_session_keys.contains(&key) || verified_live_payload_keys.contains(&key);
            let retained_summary_refreshed = live_session_payload_verified
                && self.refresh_retained_rbc_summary_from_local_payload(key);
            // Commit cleanup retains the final status summary for observability and restart
            // recovery, while still clearing runtime-only RBC state. If the live session has
            // already retired, only local payload evidence can promote the retained summary to
            // the same delivered terminal state; counters alone are not authoritative.
            if let Some(mut summary) = self.subsystems.da_rbc.rbc.status_handle.get(&key)
                && !summary.invalid
            {
                let local_payload_matches_summary = summary.payload_hash.is_some_and(|expected| {
                    self.with_local_payload_for_progress(key.0, |height, view, _bytes, hash| {
                        height == key.1 && view == key.2 && expected == hash
                    })
                    .unwrap_or(false)
                });
                let can_promote_from_local_payload = local_payload_matches_summary
                    && live_session_payload_verified
                    && retained_summary_refreshed
                    && summary.total_chunks > 0
                    && summary.received_chunks <= summary.total_chunks;
                let mut changed = false;
                if can_promote_from_local_payload && summary.received_chunks != summary.total_chunks
                {
                    summary.received_chunks = summary.total_chunks;
                    changed = true;
                }
                if can_promote_from_local_payload && !summary.delivered {
                    summary.delivered = true;
                    changed = true;
                }
                if changed {
                    self.subsystems
                        .da_rbc
                        .rbc
                        .status_handle
                        .update(summary, SystemTime::now());
                }
            }
            if live_session_payload_verified && retained_summary_refreshed {
                self.maybe_record_rbc_payload_bytes_metric_for_retained_summary(key);
            }
            self.clear_rbc_runtime_state(key, false);
        }

        let telemetry_ref = self.telemetry_handle();
        if !lane_totals.is_empty() || !dataspace_totals.is_empty() {
            let (lane_commitments, dataspace_commitments) = build_commitment_snapshots_from_totals(
                lane_totals,
                dataspace_totals,
                block_hash,
                height,
            );
            if let Some(telemetry) = telemetry_ref {
                let queue_limits = self.queue.queue_limits();
                telemetry.record_lane_commitments(
                    &lane_commitments,
                    &dataspace_commitments,
                    &queue_limits,
                );
            }
            super::status::set_lane_commitments(lane_commitments, dataspace_commitments);
        }

        self.publish_rbc_backlog_snapshot();
    }

    fn maybe_record_rbc_payload_bytes_metric_for_retained_summary(
        &mut self,
        key: super::rbc_store::SessionKey,
    ) {
        let Some(summary) = self.subsystems.da_rbc.rbc.status_handle.get(&key) else {
            return;
        };
        if summary.invalid || !summary.delivered {
            return;
        }
        if self
            .subsystems
            .da_rbc
            .rbc
            .payload_metric_recorded_sessions
            .contains(&key)
        {
            return;
        }
        let Some(expected_payload_hash) = summary.payload_hash else {
            return;
        };
        let bytes = self
            .with_local_payload_for_progress(
                key.0,
                |height, view, payload_bytes, local_payload_hash| {
                    (height == key.1
                        && view == key.2
                        && local_payload_hash == expected_payload_hash)
                        .then(|| u64::try_from(payload_bytes.len()).unwrap_or(u64::MAX))
                },
            )
            .flatten();
        if let Some(bytes) = bytes {
            self.record_rbc_payload_bytes_metric_for_active_session(key, bytes);
        }
    }

    fn refresh_retained_rbc_summary_from_local_payload(
        &mut self,
        key: super::rbc_store::SessionKey,
    ) -> bool {
        let Some(expected_payload_hash) = self
            .subsystems
            .da_rbc
            .rbc
            .status_handle
            .get(&key)
            .and_then(|summary| (!summary.invalid).then_some(summary.payload_hash))
            .flatten()
        else {
            return false;
        };

        let Some(block) = self.local_signed_block_for_hash(key.0) else {
            return false;
        };
        let header = block.header();
        if block.hash() != key.0
            || header.height().get() != key.1
            || header.view_change_index() != key.2
        {
            return false;
        }
        let (payload_bytes, payload_hash) = self
            .with_local_payload_for_progress(key.0, |height, view, bytes, hash| {
                (height == key.1 && view == key.2).then(|| (bytes.to_vec(), hash))
            })
            .flatten()
            .unwrap_or_else(|| {
                let payload_bytes = super::proposals::block_payload_bytes(block.as_ref());
                let payload_hash = Hash::new(&payload_bytes);
                (payload_bytes, payload_hash)
            });
        if expected_payload_hash != payload_hash {
            return false;
        }
        match self.persist_exact_frontier_rbc_recovery_snapshot(
            key,
            block.as_ref(),
            payload_bytes.as_slice(),
            payload_hash,
        ) {
            Ok(refreshed) => refreshed,
            Err(err) => {
                debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                ?err,
                "failed to refresh retained committed RBC snapshot from local payload"
                );
                false
            }
        }
    }

    pub(super) fn clean_rbc_sessions_for_block(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
    ) {
        self.clean_rbc_sessions_for_block_inner(block_hash, height, true);
    }

    pub(super) fn should_retain_rbc_sessions_after_commit(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
    ) -> bool {
        if !self.runtime_da_enabled() {
            return false;
        }
        self.subsystems
            .da_rbc
            .rbc
            .sessions
            .iter()
            .any(|(key, session)| {
                key.0 == block_hash
                    && key.1 == height
                    && !session.is_invalid()
                    && !rbc_session_has_complete_delivery(session)
            })
    }

    pub(super) fn clean_rbc_sessions_for_committed_block_if_settled(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
    ) -> bool {
        if self.should_retain_rbc_sessions_after_commit(block_hash, height) {
            return false;
        }

        // Keep the persisted RBC snapshot after commit so lagging peers and restart recovery
        // can still hydrate block bodies from disk even after runtime session state is drained.
        self.clean_rbc_sessions_for_block_inner(block_hash, height, false);
        true
    }

    pub(super) fn refresh_npos_seed(
        &mut self,
        seed: [u8; 32],
        height: u64,
        phase: EpochRefreshPhase,
    ) {
        let (cfg, epoch_params, seed_for_height, epoch_seed_param) = {
            let world = self.state.world_view();
            let cfg = if matches!(self.consensus_mode, ConsensusMode::Npos) {
                Some(
                    super::load_npos_collector_config_from_world(&world, self.state.chain_id_ref())
                        .or(self.npos_collectors)
                        .unwrap_or(NposCollectorConfig {
                            seed,
                            k: self.config.collectors.k,
                            redundant_send_r: self.config.collectors.redundant_send_r,
                        }),
                )
            } else {
                None
            };
            let epoch_params = super::load_npos_epoch_params_from_world(&world, &self.config.npos);
            let seed_for_height =
                super::prf_seed_for_height_from_world(&world, self.state.chain_id_ref(), height);
            let epoch_seed_param = world.sumeragi_npos_parameters().map(|params| {
                let seed = params.epoch_seed();
                <[u8; 32]>::from(seed)
            });
            (cfg, epoch_params, seed_for_height, epoch_seed_param)
        };
        let mut next_seed = seed;
        if let Some(manager) = self.epoch_manager.as_mut() {
            manager.set_params(
                epoch_params.epoch_length_blocks,
                epoch_params.commit_deadline_offset,
                epoch_params.reveal_deadline_offset,
            );
            if matches!(phase, EpochRefreshPhase::PostCommit) {
                let epoch_for_height = manager.epoch_for_height(height);
                let expected_epoch =
                    if height > 0 && height.is_multiple_of(manager.epoch_length_blocks()) {
                        epoch_for_height.saturating_add(1)
                    } else {
                        epoch_for_height
                    };
                if manager.epoch() != expected_epoch {
                    let reset_seed = epoch_seed_param
                        .or_else(|| cfg.map(|cfg| cfg.seed))
                        .unwrap_or(seed_for_height);
                    manager.reset_epoch_state(expected_epoch, reset_seed);
                    self.subsystems.vrf.reset();
                    next_seed = reset_seed;
                }
            }
            super::status::set_epoch_parameters(
                manager.epoch_length_blocks(),
                manager.commit_window_end(),
                manager.reveal_window_end(),
            );
            #[cfg(feature = "telemetry")]
            self.telemetry.set_epoch_parameters(
                manager.epoch_length_blocks(),
                manager.commit_window_end(),
                manager.reveal_window_end(),
            );
        }
        if let Some(cfg) = cfg {
            self.npos_collectors = Some(cfg);
            if let Some(cfg) = self.npos_collectors.as_mut() {
                cfg.seed = next_seed;
            }
        } else {
            self.npos_collectors = None;
        }
    }

    pub(super) fn poll_committed_blocks(&mut self) -> bool {
        match self.try_poll_committed_blocks() {
            Ok(progress) => progress,
            Err(err) => {
                warn!(?err, "failed to process committed block height");
                false
            }
        }
    }

    fn try_poll_committed_blocks(&mut self) -> Result<bool> {
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let mut progress = false;
        let mut next_height = self.last_committed_height.saturating_add(1);
        while next_height <= committed_height {
            if let Some((activate_at, roster)) = self.pending_roster_activation.clone() {
                if next_height >= activate_at {
                    if let Err(err) = self.install_elected_roster(&roster) {
                        warn!(
                            ?err,
                            "failed to install pending elected roster; retaining pending activation"
                        );
                    } else {
                        self.pending_roster_activation = None;
                    }
                }
            }
            self.on_block_commit(next_height)?;
            self.block_count.0 = usize::try_from(next_height).unwrap_or(usize::MAX);
            self.last_committed_height = next_height;
            next_height = next_height.saturating_add(1);
            progress = true;
        }
        if !progress {
            // Refresh P2P topology even when height is unchanged to catch world-peer updates
            // that land after commit processing (e.g., late-applied peer registrations).
            self.refresh_p2p_topology();
            if let Some((activate_at, roster)) = self.pending_roster_activation.clone() {
                if committed_height >= activate_at {
                    if let Err(err) = self.install_elected_roster(&roster) {
                        warn!(
                            ?err,
                            "failed to install pending elected roster; retaining pending activation"
                        );
                    } else {
                        self.pending_roster_activation = None;
                    }
                }
            }
        }
        progress |= self.retire_committed_commit_inflight("poll_committed_blocks");
        Ok(progress)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_block_commit(&mut self, height: u64) -> Result<()> {
        self.refresh_roster_validation_cache();
        let committed_block = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .and_then(|nz| self.kura.get_block(nz));
        let pre_prune_commit_topology = self.effective_commit_topology();
        let cached_committed_qc_before_prune = committed_block.as_ref().and_then(|block| {
            self.materialize_qc_for_header(
                crate::sumeragi::consensus::QcHeaderRef {
                    phase: crate::sumeragi::consensus::Phase::Commit,
                    subject_block_hash: block.hash(),
                    height,
                    view: block.header().view_change_index(),
                    epoch: self.epoch_for_height(height),
                },
                &pre_prune_commit_topology,
            )
        });
        self.subsystems.propose.new_view_tracker.prune(height);
        self.prune_proposals_seen_horizon(height);
        self.slot_tracker.prune_committed(height);
        self.prune_vote_caches_horizon(height);
        self.subsystems.propose.forced_view_after_timeout = self
            .subsystems
            .propose
            .forced_view_after_timeout
            .filter(|(forced_height, _)| *forced_height > height);
        let now = Instant::now();
        self.prune_lock_rejected_block_sinks(now);
        self.prune_stale_missing_requests_for_committed_height(height, now);
        self.clear_missing_block_recovery_for_height(height, now);
        self.clear_sidecar_mismatch_for_height(height);
        self.prune_frontier_slot_state();
        let _ = self.maybe_release_committed_edge_conflict_owner("committed_height_advanced");
        self.prune_missing_block_recovery_state(now);
        self.refresh_p2p_topology();
        if let Some(baseline_roster) = self.recovery_pending_baseline_restore.remove(&height) {
            if let Err(err) = self.install_elected_roster(&baseline_roster) {
                warn!(
                    ?err,
                    height,
                    roster_len = baseline_roster.len(),
                    "failed to restore baseline roster after temporary recovery shrink"
                );
            } else {
                let _ = self.refresh_commit_topology_state(&baseline_roster);
                info!(
                    height,
                    roster_len = baseline_roster.len(),
                    "restored baseline roster after temporary recovery shrink"
                );
            }
        }
        let commit_topology = self.effective_commit_topology();
        match self.refresh_commit_topology_state(&commit_topology) {
            CommitTopologyChange::None => {}
            CommitTopologyChange::Membership => {
                // Preserve proposals_seen to avoid re-proposing the same (height, view) after
                // a roster change clears consensus caches.
                self.reset_consensus_state_for_roster_change(true);
                debug!(
                    height,
                    roster_len = commit_topology.len(),
                    "commit topology changed; cleared consensus caches"
                );
            }
            CommitTopologyChange::OrderOnly => {
                debug!(
                    height,
                    roster_len = commit_topology.len(),
                    "commit topology order changed; retaining consensus caches"
                );
            }
        }
        self.update_missing_block_gauges();
        if let Some(block) = committed_block.as_ref() {
            self.prune_descendants_not_on_tip(height, block.hash());
            let refreshed =
                self.refresh_tip_activated_pending_progress(height, block.hash(), Instant::now());
            if refreshed > 0 {
                debug!(
                    height,
                    block = %block.hash(),
                    refreshed,
                    "refreshed pending progress for proposals activated by the new committed tip"
                );
            }
            self.note_view_change_from_block(height, block.header().view_change_index());
        }
        self.refresh_frontier_round_tracking_after_commit(height, now);
        if let Some(committed_qc) = self.latest_committed_qc() {
            let promote_highest = self
                .highest_qc
                .is_none_or(|qc| (qc.height, qc.view) < (committed_qc.height, committed_qc.view));
            if promote_highest {
                self.highest_qc = Some(committed_qc);
                super::status::set_highest_qc(committed_qc.height, committed_qc.view);
                super::status::set_highest_qc_hash(committed_qc.subject_block_hash);
            }
            let promote_lock = self
                .locked_qc
                .is_none_or(|qc| (qc.height, qc.view) < (committed_qc.height, committed_qc.view));
            if promote_lock {
                self.locked_qc = Some(committed_qc);
                super::status::set_locked_qc(
                    committed_qc.height,
                    committed_qc.view,
                    Some(committed_qc.subject_block_hash),
                );
            }
        }
        if let Some(block) = committed_block.as_ref() {
            let qc_header = crate::sumeragi::consensus::QcHeaderRef {
                phase: crate::sumeragi::consensus::Phase::Commit,
                subject_block_hash: block.hash(),
                height,
                view: block.header().view_change_index(),
                epoch: self.epoch_for_height(height),
            };
            let committed_qc = self
                .materialize_qc_for_header(qc_header, &commit_topology)
                .or(cached_committed_qc_before_prune);
            if committed_qc.is_none() {
                debug!(
                    height,
                    view = qc_header.view,
                    block = %qc_header.subject_block_hash,
                    "unable to cache QC for committed block from kura"
                );
            }
            if let Some(commit_qc) = committed_qc {
                let recovery_targets = self.known_block_commit_qc_recovery_targets(
                    qc_header.subject_block_hash,
                    height,
                    qc_header.view,
                    &commit_topology,
                );
                let replayed = self.broadcast_cached_commit_qc_to_targets_with_backpressure(
                    commit_qc.clone(),
                    &recovery_targets,
                    true,
                    "post_commit_finality",
                );
                let certified_responses = self.broadcast_certified_commit_response_to_targets(
                    block,
                    &commit_qc,
                    &recovery_targets,
                    "post_commit_finality",
                );
                if replayed > 0 || certified_responses > 0 {
                    info!(
                        height,
                        view = qc_header.view,
                        block = %qc_header.subject_block_hash,
                        replayed,
                        certified_responses,
                        "broadcasting committed block QC after durable commit"
                    );
                }
            }
        }
        if !matches!(
            self.consensus_mode,
            ConsensusMode::Permissioned | ConsensusMode::Npos
        ) {
            return Ok(());
        }
        let local_signer = self.local_validator_index_current();
        let (_, roster_len, roster_indices) = self.current_height_and_roster();
        let roster_len_hint = u32::try_from(roster_len).unwrap_or_else(|_| {
            warn!(
                roster_len,
                "validator roster exceeds u32::MAX; snapshot hint clamped to u32::MAX"
            );
            u32::MAX
        });
        if let Some(manager) = self.epoch_manager.as_mut() {
            apply_roster_indices_to_manager(manager, roster_len, roster_indices);
        } else {
            return Ok(());
        }

        if let Some(local_idx) = local_signer {
            self.maybe_emit_vrf_messages(height, roster_len_hint, local_idx)?;
        }

        let (seed, snapshot) = {
            let Some(manager) = self.epoch_manager.as_mut() else {
                return Ok(());
            };
            manager.on_block_commit(height);
            let seed = manager.seed();
            let snapshot = manager.take_last_epoch_snapshot();
            let _ = manager.take_last_penalties();
            let _ = manager.take_last_penalties_detailed();
            (seed, snapshot)
        };

        let election_outcome = if matches!(self.consensus_mode, ConsensusMode::Npos) {
            if let Some(snapshot) = snapshot.as_ref() {
                let epoch_to_service = snapshot.epoch.saturating_add(1);
                Some(self.run_validator_election(
                    epoch_to_service,
                    height,
                    seed,
                    roster_len_hint,
                )?)
            } else {
                None
            }
        } else {
            None
        };

        self.refresh_npos_seed(seed, height, EpochRefreshPhase::PostCommit);
        super::status::set_prf_context(seed, height, 0);
        #[cfg(feature = "telemetry")]
        self.telemetry.set_prf_context(Some(seed), height, 0);

        if let Some(snapshot) = snapshot {
            let epoch = snapshot.epoch;
            let roster_len = snapshot.roster_len;
            let committed_no_reveal = snapshot.committed_no_reveal.clone();
            let no_participation = snapshot.no_participation.clone();
            let late_reveals_total = snapshot.late_reveals.len();

            self.stage_vrf_snapshot(snapshot, true, election_outcome.clone())?;
            if let Some(manager) = self.epoch_manager.as_ref() {
                let new_epoch = manager.epoch();
                let record_exists = {
                    let world = self.state.world_view();
                    world.vrf_epochs().get(&new_epoch).is_some()
                        || self.pending_npos_vrf_records.contains_key(&new_epoch)
                };
                if !record_exists {
                    let seed_snapshot = manager.snapshot_current_epoch(roster_len_hint, height);
                    self.stage_vrf_snapshot(seed_snapshot, false, None)?;
                }
            }

            epoch_report::update(epoch_report::VrfPenaltiesReport {
                epoch,
                committed_no_reveal: committed_no_reveal.clone(),
                no_participation: no_participation.clone(),
                roster_len,
            });

            super::status::set_vrf_penalties(
                epoch,
                committed_no_reveal.len() as u64,
                no_participation.len() as u64,
                late_reveals_total as u64,
            );

            #[cfg(feature = "telemetry")]
            {
                for idx in &committed_no_reveal {
                    if let Ok(i) = usize::try_from(*idx) {
                        self.telemetry.inc_vrf_non_reveal_for_signer(i);
                    }
                }
                if !committed_no_reveal.is_empty() {
                    self.telemetry
                        .inc_vrf_non_reveal_total(committed_no_reveal.len() as u64, epoch);
                }
                for idx in &no_participation {
                    if let Ok(i) = usize::try_from(*idx) {
                        self.telemetry.inc_vrf_no_participation_for_signer(i);
                    }
                }
                if !no_participation.is_empty() {
                    self.telemetry
                        .inc_vrf_no_participation_total(no_participation.len() as u64, epoch);
                }
            }

            if let Some(outcome) = election_outcome {
                super::status::record_npos_election(outcome.clone());
                if !outcome.validator_set.is_empty() {
                    let activate_at = height.saturating_add(outcome.params.finality_margin_blocks);
                    self.pending_roster_activation =
                        Some((activate_at, outcome.validator_set.clone()));
                }
            }
        }

        if let Some(epoch) = self.epoch_manager.as_ref().map(EpochManager::epoch) {
            let _ = self.subsystems.vrf.state_mut(self.consensus_mode, epoch);
        }

        self.note_committed_npos_effects(height, committed_block.as_deref());

        Ok(())
    }

    pub(super) fn build_npos_consensus_effects_for_proposal(
        &self,
        proposal_height: u64,
    ) -> Result<Option<NposConsensusEffects>> {
        if !matches!(self.consensus_mode, ConsensusMode::Npos) {
            return Ok(None);
        }
        let telemetry = {
            #[cfg(feature = "telemetry")]
            {
                Some(self.state.metrics())
            }
            #[cfg(not(feature = "telemetry"))]
            {
                None
            }
        };
        let applier = PenaltyApplier::new(
            self.state.as_ref(),
            &self.config,
            #[cfg(feature = "telemetry")]
            telemetry,
            #[cfg(not(feature = "telemetry"))]
            telemetry,
        );
        let effects = applier.derive_npos_consensus_effects(
            proposal_height,
            self.pending_npos_vrf_records.values().cloned(),
        )?;
        Ok((!effects.is_empty()).then_some(effects))
    }

    fn note_committed_npos_effects(&mut self, height: u64, committed_block: Option<&SignedBlock>) {
        let Some(effects) = committed_block.and_then(SignedBlock::npos_consensus_effects) else {
            return;
        };
        for record in &effects.vrf_epoch_seals {
            if let Some(pending) = self.pending_npos_vrf_records.get(&record.epoch).cloned() {
                if Self::committed_vrf_record_covers_pending(record, &pending) {
                    self.pending_npos_vrf_records.remove(&record.epoch);
                } else if let Some(merged) = Self::merge_vrf_epoch_records(record, &pending) {
                    debug!(
                        epoch = record.epoch,
                        committed_updated_at_height = record.updated_at_height,
                        pending_updated_at_height = pending.updated_at_height,
                        "retaining committed-compatible pending VRF epoch record after stale committed seal"
                    );
                    if &merged == record {
                        self.pending_npos_vrf_records.remove(&record.epoch);
                    } else {
                        self.pending_npos_vrf_records.insert(record.epoch, merged);
                    }
                } else {
                    warn!(
                        epoch = record.epoch,
                        committed_updated_at_height = record.updated_at_height,
                        pending_updated_at_height = pending.updated_at_height,
                        "dropping pending VRF epoch record that conflicts with committed seal"
                    );
                    self.pending_npos_vrf_records.remove(&record.epoch);
                }
            }
            if let Some((activate_at, roster, apply_now)) =
                Self::activation_plan_from_vrf_record(height, record)
            {
                if apply_now {
                    if let Err(err) = self.install_elected_roster(&roster) {
                        warn!(
                            ?err,
                            epoch = record.epoch,
                            "failed to install committed elected roster"
                        );
                    }
                } else {
                    self.pending_roster_activation = Some((activate_at, roster));
                }
            }
        }
        let mut vrf_applied = 0_u64;
        let mut consensus_applied = 0_u64;
        for action in &effects.penalty_actions {
            match action {
                iroha_data_model::consensus::NposPenaltyAction::VrfJail(_) => {
                    vrf_applied = vrf_applied.saturating_add(1);
                }
                iroha_data_model::consensus::NposPenaltyAction::ConsensusSlash(_) => {
                    consensus_applied = consensus_applied.saturating_add(1);
                }
                iroha_data_model::consensus::NposPenaltyAction::MarkVrfPenaltiesApplied(_)
                | iroha_data_model::consensus::NposPenaltyAction::MarkConsensusEvidenceApplied(_) =>
                    {}
            }
        }
        super::status::inc_vrf_penalties_applied(vrf_applied);
        super::status::inc_consensus_penalties_applied(consensus_applied);
    }

    pub(super) fn committed_vrf_record_covers_pending(
        committed: &VrfEpochRecord,
        pending: &VrfEpochRecord,
    ) -> bool {
        pending.updated_at_height <= committed.updated_at_height
            && pending
                .participants
                .iter()
                .all(|participant| committed.participants.contains(participant))
            && pending
                .late_reveals
                .iter()
                .all(|late_reveal| committed.late_reveals.contains(late_reveal))
    }

    #[allow(clippy::unnecessary_wraps)]
    fn run_validator_election(
        &self,
        epoch: u64,
        snapshot_height: u64,
        seed: [u8; 32],
        roster_len_hint: u32,
    ) -> Result<ValidatorElectionOutcome> {
        let params = {
            let world = self.state.world_view();
            super::resolve_npos_election_params_from_world(&world, &self.config.npos)
        };
        let Some(epoch_roster) = self.state.epoch_validator_peer_ids_fast(epoch) else {
            let reason = "stake snapshot unavailable";
            warn!(epoch, %reason, "validator election skipped");
            return Ok(ValidatorElectionOutcome {
                epoch,
                snapshot_height,
                seed,
                candidates_total: 0,
                validator_set_hash: HashOf::new(&Vec::new()),
                validator_set: Vec::new(),
                params,
                rejection_reason: Some(reason.to_owned()),
                tie_break: Vec::new(),
            });
        };
        let profiles = {
            let world = self.state.world_view();
            self.collect_candidate_profiles(&world, &epoch_roster)
        };

        let filtered = election::filter_candidates_with_constraints(profiles, &params);
        if filtered.is_empty() {
            let reason = "no candidates after applying election constraints";
            warn!(
                epoch,
                %reason,
                "validator election produced no eligible validators"
            );
            return Ok(ValidatorElectionOutcome {
                epoch,
                snapshot_height,
                seed,
                candidates_total: filtered.len().try_into().unwrap_or(u32::MAX),
                validator_set_hash: HashOf::new(&Vec::new()),
                validator_set: Vec::new(),
                params,
                rejection_reason: Some(reason.to_owned()),
                tie_break: Vec::new(),
            });
        }

        let outcome = election::elect_validator_set(epoch, snapshot_height, seed, filtered, params);
        if outcome.validator_set.is_empty() {
            warn!(
                epoch,
                "validator election produced an empty set; retaining existing topology"
            );
        } else if outcome.validator_set.len()
            < usize::try_from(roster_len_hint).unwrap_or(usize::MAX)
        {
            info!(
                epoch,
                selected = outcome.validator_set.len(),
                roster_len_hint,
                "elected validator set smaller than current roster"
            );
        }

        Ok(outcome)
    }

    pub(super) fn activation_plan_from_vrf_record(
        current_height: u64,
        record: &VrfEpochRecord,
    ) -> Option<(u64, Vec<PeerId>, bool)> {
        let election = record.validator_election.as_ref()?;
        if election.validator_set.is_empty() {
            return None;
        }
        let activate_at = record
            .updated_at_height
            .saturating_add(election.params.finality_margin_blocks);
        let apply_now = current_height >= activate_at;
        Some((activate_at, election.validator_set.clone(), apply_now))
    }

    #[allow(clippy::unused_self)]
    fn collect_candidate_profiles(
        &self,
        world: &impl WorldReadOnly,
        candidates: &[PeerId],
    ) -> Vec<election::CandidateProfile> {
        use iroha_data_model::{
            account::AccountId,
            nexus::{
                LaneId,
                staking::{PublicLaneStakeShare, PublicLaneValidatorRecord},
            },
        };

        let mut record_map: BTreeMap<PeerId, PublicLaneValidatorRecord> = BTreeMap::new();
        for ((_lane_id, _validator_id), record) in world.public_lane_validators().iter() {
            record_map
                .entry(record.peer_id.clone())
                .or_insert_with(|| record.clone());
        }

        let mut share_map: BTreeMap<(LaneId, AccountId), Vec<PublicLaneStakeShare>> =
            BTreeMap::new();
        for ((lane_id, validator, _staker), share) in world.public_lane_stake_shares().iter() {
            share_map
                .entry((*lane_id, validator.clone()))
                .or_default()
                .push(share.clone());
        }

        candidates
            .iter()
            .map(|peer| {
                let record = record_map.get(peer).cloned();
                let stake_shares = record
                    .as_ref()
                    .and_then(|rec| {
                        share_map
                            .get(&(rec.lane_id, rec.validator.clone()))
                            .cloned()
                    })
                    .unwrap_or_default();
                election::CandidateProfile {
                    peer_id: peer.clone(),
                    record,
                    stake_shares,
                }
            })
            .collect()
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn install_elected_roster(&self, roster: &[PeerId]) -> Result<()> {
        if roster.is_empty() {
            return Ok(());
        }
        let prev = {
            let mut block = self.state.commit_topology.block();
            let prev = block.take_vec();
            block.commit();
            prev
        };
        {
            let mut block = self.state.prev_commit_topology.block();
            block.mutate_vec(|vec| *vec = prev);
            block.commit();
        }
        {
            let mut block = self.state.commit_topology.block();
            block.mutate_vec(|vec| *vec = roster.to_vec());
            block.commit();
        }
        info!(
            len = roster.len(),
            "activated elected validator set for upcoming epoch"
        );
        Ok(())
    }

    fn refresh_roster_validation_cache(&mut self) {
        let world = self.state.world.view();
        self.roster_validation_cache.refresh_from_world(
            &world,
            self.config.npos.epoch_length_blocks,
            Some(&self.common_config.trusted_peers.value().pops),
        );
        drop(world);
        self.block_sync_roster_cache.clear();
        self.block_signer_cache.clear();
    }

    pub(super) fn refresh_commit_topology_state(
        &mut self,
        topology: &[PeerId],
    ) -> CommitTopologyChange {
        let order_hash = HashOf::new(&topology.to_vec());
        let mut membership = topology.to_vec();
        membership.sort();
        let membership_hash = HashOf::new(&membership);

        if self.last_commit_topology_hash == Some(order_hash) {
            return CommitTopologyChange::None;
        }

        let membership_changed = self.last_commit_topology_membership_hash != Some(membership_hash);
        self.last_commit_topology_hash = Some(order_hash);
        self.last_commit_topology_membership_hash = Some(membership_hash);

        if membership_changed {
            // Only reset view-change state when the validator set changes; order-only rotations
            // are expected as part of leader selection.
            self.subsystems.propose.new_view_tracker = NewViewTracker::default();
            self.subsystems.propose.forced_view_after_timeout = None;
            CommitTopologyChange::Membership
        } else {
            CommitTopologyChange::OrderOnly
        }
    }

    /// Resets consensus caches when the validator roster changes.
    pub(super) fn reset_consensus_state_for_roster_change(
        &mut self,
        preserve_proposals_seen: bool,
    ) {
        self.pending.pending_blocks.clear();
        self.subsystems.validation.inflight.clear();
        self.subsystems.validation.vnext_inflight.clear();
        self.subsystems.validation.superseded_results.clear();
        self.pending.pending_fetch_requests.clear();
        self.pending.pending_block_body_requests.clear();
        self.pending.missing_block_requests.clear();
        self.pending.missing_commit_qc_requests.clear();
        self.pending.pending_processing.set(None);
        self.pending.pending_processing_parent.set(None);
        self.vote_log.clear();
        self.vote_log_identities.clear();
        self.vote_validation_cache.clear();
        self.vote_validation_cache_identities.clear();
        self.deferred_votes.clear();
        self.consensus_recovery.clear();
        self.recovery_pending_baseline_restore.clear();
        self.deferred_qcs.clear();
        self.deferred_qc_roster_state.clear();
        self.deferred_missing_payload_qcs.clear();
        self.quarantined_block_sync_qcs.clear();
        self.vote_roster_cache.clear();
        self.qc_cache.clear();
        self.qc_signer_tally.clear();
        self.lock_rejected_block_sinks.clear();
        self.block_signer_cache.clear();
        self.voting_block = None;
        if !preserve_proposals_seen {
            self.slot_tracker.proposals_seen.clear();
        }
        self.slot_tracker.authoritative_block_slots.clear();
        self.slot_tracker.authoritative_block_frontiers.clear();
        self.slot_tracker.retained_branches.clear();
        self.subsystems.propose.proposal_cache =
            ProposalCache::new(self.recovery_pending_proposal_cap());
        self.reset_collector_state();
        self.clear_all_pending_rbc();
        self.subsystems.da_rbc.rbc.sessions.clear();
        self.subsystems.da_rbc.rbc.session_rosters.clear();
        self.subsystems.da_rbc.rbc.session_roster_sources.clear();
        self.subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .clear();
        self.subsystems
            .da_rbc
            .rbc
            .ready_rebroadcast_last_sent
            .clear();
        self.subsystems.da_rbc.rbc.deliver_deferral.clear();
        self.subsystems.da_rbc.rbc.persisted_sessions.clear();
        self.subsystems.da_rbc.rbc.persist_inflight.clear();
        self.subsystems.da_rbc.rbc.persist_pending_refresh.clear();
        // Preserve operator-facing RBC summaries across roster resets so sessions recovered from
        // disk remain observable while the runtime-only consensus state is cleared.
        self.subsystems.da_rbc.da.da_bundles.clear();
        self.subsystems.da_rbc.da.da_pin_bundles.clear();
        self.subsystems.da_rbc.da.sealed_commitments.clear();
        self.subsystems.da_rbc.da.sealed_pin_intents.clear();
        self.new_view_rebroadcast_log.clear();
        self.proposal_rebroadcast_log.clear();
        self.payload_rebroadcast_log.clear();
        self.block_sync_rebroadcast_log.clear();
        self.block_sync_fetch_log.clear();
        self.block_sync_warning_log.clear();
        self.qc_insufficient_warning_log.clear();
        self.round_recovery_bundle_window_gates.clear();
        let now = Instant::now();
        self.tick_lag_last_progress_at = now;
        self.tick_lag_last_progress_height = self.state.committed_height();
        self.tick_lag_last_progress_queue_len = self.queue.active_len();
        self.tick_lag_last_progress_pending_blocks = self.pending.pending_blocks.len();
        self.tick_lag_warn_streak = 0;
        self.tick_lag_last_warn = None;
        self.hotspot_log_summary.reset(now);
    }

    #[cfg(test)]
    #[allow(dead_code)]
    /// Test-only wrapper around the commit hook.
    pub(super) fn on_block_commit_for_tests(&mut self, height: u64) -> Result<()> {
        self.on_block_commit(height)
    }

    pub(super) fn refresh_p2p_topology(&mut self) {
        let world = self.state.world_view();
        let current: BTreeSet<_> = world.peers().iter().cloned().collect::<BTreeSet<_>>();
        drop(world);
        self.refresh_p2p_topology_with_current(current);
    }

    fn refresh_p2p_topology_with_current(&mut self, current: BTreeSet<PeerId>) {
        let trusted_peers = self.common_config.trusted_peers.value();
        let expected_topology = p2p_topology_with_trusted(&current, trusted_peers);

        let local_peer = self.common_config.peer.id();
        let (local_peer_seen, removed) = super::topology_refresh_local_status(
            &current,
            local_peer,
            self.local_peer_seen_in_world,
        );
        self.local_peer_seen_in_world = local_peer_seen;
        crate::sumeragi::status::set_local_removed_from_world(removed);
        if removed {
            iroha_logger::warn!(
                current_len = current.len(),
                local = %local_peer,
                "local peer removed from world state; disconnecting from p2p"
            );
            self.queue.clear_all();
            let advertise =
                super::topology_update_for_local_removal(&self.last_advertised_topology);
            self.last_advertised_topology.clone_from(&advertise);
            self.peers_gossiper
                .update_topology(UpdateTopology(advertise.into_iter().collect()));
            return;
        }

        let online_peer_ids: Vec<_> = self
            .network
            .online_peers(|online| online.iter().map(|peer| peer.id().clone()).collect());
        let stray_online = peer_ids_outside_topology(&expected_topology, &online_peer_ids);

        let (decision, advertise) = super::topology_advertisement_for_refresh(
            &current,
            &self.last_advertised_topology,
            &stray_online,
        );
        match decision {
            TopologyRefreshDecision::NoPeers => {
                iroha_logger::debug!("skipping p2p topology advertise: world state has no peers");
                return;
            }
            TopologyRefreshDecision::Unchanged => {
                iroha_logger::debug!(
                    topology_len = current.len(),
                    "p2p topology unchanged; not re-advertising"
                );
                return;
            }
            TopologyRefreshDecision::AdvertiseForStrays { stray_count } => iroha_logger::warn!(
                topology_len = current.len(),
                stray_count,
                stray_peers = ?stray_online,
                "p2p topology unchanged but network has peers outside world state; disconnecting strays"
            ),
            TopologyRefreshDecision::AdvertiseChanged => iroha_logger::info!(
                topology_len = current.len(),
                "advertising updated p2p topology from world state"
            ),
        }

        let advertise = advertise.expect("advertise topology required for decision");
        self.last_advertised_topology.clone_from(&advertise);
        let network_topology = p2p_topology_with_trusted(&advertise, trusted_peers);
        self.network
            .update_topology(UpdateTopology(network_topology.into_iter().collect()));
        self.peers_gossiper
            .update_topology(UpdateTopology(advertise.into_iter().collect()));
    }

    pub(super) fn refresh_backpressure_state(&mut self) -> bool {
        let refreshed = self.subsystems.propose.backpressure_gate.refresh();
        // Always publish the latest snapshot so operator status endpoints report
        // correct queue capacity even when the state has not changed.
        super::status::set_tx_queue_pressure(self.queue.pressure_snapshot());
        refreshed
    }

    #[allow(dead_code)]
    pub(super) fn queue_backpressure_state(&self) -> BackpressureState {
        self.subsystems.propose.backpressure_gate.state()
    }

    pub(super) fn evaluate_pacemaker(
        pacemaker: &mut Pacemaker,
        pacemaker_backpressure: &mut PacemakerBackpressure,
        backpressure: ProposalBackpressure,
        now: Instant,
    ) -> (bool, bool, bool) {
        let deferring = backpressure.should_defer();
        let backpressure_action = pacemaker_backpressure.update(deferring);
        let log_initial_deferral =
            matches!(backpressure_action, PacemakerBackpressureAction::First);
        let should_fire_now = pacemaker.should_fire(now);
        if deferring {
            if backpressure.only_pacing_backpressure() {
                if should_fire_now {
                    // Allow proposals to proceed once the pacemaker deadline elapses when only
                    // pacing pressure is present; keep logging the deferral.
                    return (log_initial_deferral, true, true);
                }
                // Defer proposal assembly under backpressure, but still request a log on the first
                // deferral of a saturation window even if the pacemaker deadline has not yet elapsed.
                return (log_initial_deferral, log_initial_deferral, false);
            }
            // Non-queue backpressure keeps proposals deferred even when the pacemaker fires.
            return (log_initial_deferral, should_fire_now, false);
        }
        (log_initial_deferral, false, should_fire_now)
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn telemetry_handle(&self) -> Option<&crate::telemetry::Telemetry> {
        #[cfg(feature = "telemetry")]
        {
            Some(&self.telemetry)
        }
        #[cfg(not(feature = "telemetry"))]
        {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        io,
        net::SocketAddr,
        sync::{Arc, mpsc},
        time::Duration,
    };

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, StateReadOnly, World},
    };
    use iroha_config::{
        base::{WithOrigin, util::Bytes},
        kura::{FsyncMode, InitMode},
        parameters::{
            actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
            defaults::kura::{
                BLOCK_SYNC_ROSTER_RETENTION, BLOCKS_IN_MEMORY, FSYNC_INTERVAL,
                MERGE_LEDGER_CACHE_CAPACITY, ROSTER_SIDECAR_RETENTION,
            },
        },
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair, MerkleTree, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId, Registrable,
        block::{BlockSignature, SignedBlock},
        peer::{Peer, PeerId},
        prelude::{Account, AccountId, Domain, EventBox, Level, Log, TransactionBuilder},
        transaction::SignedTransaction,
    };
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_primitives::{numeric::Numeric, time::TimeSource, unique_vec::UniqueVec};
    use tempfile::TempDir;

    // This suite runs with the default parallel test runner and can be CPU-contended on CI.
    // Use a conservative timeout to avoid flakiness in wake/result channel assertions.
    const COMMIT_WORKER_TIMEOUT: Duration = Duration::from_secs(180);

    #[test]
    fn materialize_qc_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            cached_existing: bool,
            empty_roster: bool,
            formed_after_try: bool,
            kura_recovery_available: bool,
            rebuild_from_votes_available: bool,
        }

        let cases = [
            Case {
                label: "cached_existing",
                cached_existing: true,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "empty_roster_recovery",
                cached_existing: false,
                empty_roster: true,
                formed_after_try: false,
                kura_recovery_available: true,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "empty_roster_no_recovery",
                cached_existing: false,
                empty_roster: true,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "formed_from_votes",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: true,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "recover_after_form_miss",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: true,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "npos_missing_stake_roster",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "commit_root_filter_empty",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "no_votes",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "permissioned_under_quorum",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "permissioned_quorum",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: true,
            },
            Case {
                label: "prepare_quorum",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: true,
            },
            Case {
                label: "npos_signer_map_error",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "npos_stake_quorum_false",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "aggregate_error",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
            Case {
                label: "canonical_mapping_incomplete",
                cached_existing: false,
                empty_roster: false,
                formed_after_try: false,
                kura_recovery_available: false,
                rebuild_from_votes_available: false,
            },
        ];

        for case in cases {
            let spec_try_form = !case.cached_existing && !case.empty_roster;
            let spec_attempts_kura_recovery = !case.cached_existing
                && (case.empty_roster || (!case.formed_after_try && case.kura_recovery_available));
            let spec_result = if case.cached_existing {
                MaterializeQcResult::Cached
            } else if spec_attempts_kura_recovery && case.kura_recovery_available {
                MaterializeQcResult::Recovered
            } else if case.formed_after_try {
                MaterializeQcResult::Formed
            } else if case.rebuild_from_votes_available {
                MaterializeQcResult::Rebuilt
            } else {
                MaterializeQcResult::None
            };
            let spec_cache_insert = matches!(
                spec_result,
                MaterializeQcResult::Recovered
                    | MaterializeQcResult::Formed
                    | MaterializeQcResult::Rebuilt
            );

            let actual = materialize_qc_decision(
                case.cached_existing,
                case.empty_roster,
                case.formed_after_try,
                case.kura_recovery_available,
                case.rebuild_from_votes_available,
            );

            assert_eq!(
                actual.try_form_votes, spec_try_form,
                "{} try_form_votes mismatch",
                case.label
            );
            assert_eq!(
                actual.attempts_kura_recovery, spec_attempts_kura_recovery,
                "{} attempts_kura_recovery mismatch",
                case.label
            );
            assert_eq!(actual.result, spec_result, "{} result mismatch", case.label);
            assert_eq!(
                actual.caches_materialized_qc, spec_cache_insert,
                "{} caches_materialized_qc mismatch",
                case.label
            );
        }
    }

    #[test]
    fn prevalidated_roots_match_witness_matches_formal_boundaries() {
        use crate::sumeragi::consensus::{ExecKv, ExecWitness};

        let witness = ExecWitness {
            reads: vec![ExecKv {
                key: b"balance".to_vec(),
                value: b"10".to_vec(),
            }],
            writes: vec![ExecKv {
                key: b"balance".to_vec(),
                value: b"7".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let header = iroha_data_model::block::BlockHeader::new(
            core::num::NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            0,
            0,
        );
        let artifact = ValidatedCommitArtifact {
            block_hash: iroha_crypto::HashOf::new(&header),
            height: 1,
            view: 0,
            parent_state_root: parent_state_from_witness(&witness),
            post_state_root: post_state_from_witness(&witness),
        };

        assert!(!prevalidated_roots_match_witness(artifact, None));
        assert!(prevalidated_roots_match_witness(artifact, Some(&witness)));

        let parent_mismatch = ValidatedCommitArtifact {
            parent_state_root: Hash::prehashed([0xA1; Hash::LENGTH]),
            ..artifact
        };
        assert!(!prevalidated_roots_match_witness(
            parent_mismatch,
            Some(&witness)
        ));

        let post_mismatch = ValidatedCommitArtifact {
            post_state_root: Hash::prehashed([0xB2; Hash::LENGTH]),
            ..artifact
        };
        assert!(!prevalidated_roots_match_witness(
            post_mismatch,
            Some(&witness)
        ));
    }

    #[test]
    fn commit_stage_timings_threshold_uses_clear_latency_helpers() {
        let timings = CommitStageTimings {
            qc_verify_ms: Some(3_000),
            persist_ms: Some(2_100),
            kura_store_ms: Some(100),
            state_apply_ms: Some(1_000),
            state_commit_ms: Some(1_000),
            validation: None,
            used_prevalidated_artifact: false,
        };

        assert_eq!(timings.blocking_total_ms(), Some(5_100));
        assert_eq!(timings.max_observed_stage_ms(), Some(3_000));
        assert!(commit_stage_timings_exceed_threshold(
            timings,
            Duration::from_secs(5)
        ));
        assert!(!commit_stage_timings_exceed_threshold(
            timings,
            Duration::from_secs(6)
        ));
        assert!(!commit_stage_timings_exceed_threshold(
            timings,
            Duration::ZERO
        ));

        let slow_stage = CommitStageTimings {
            state_apply_ms: Some(6_000),
            ..CommitStageTimings::default()
        };
        assert_eq!(slow_stage.blocking_total_ms(), None);
        assert_eq!(slow_stage.max_observed_stage_ms(), Some(6_000));
        assert!(commit_stage_timings_exceed_threshold(
            slow_stage,
            Duration::from_secs(5)
        ));
    }

    #[test]
    fn autoscale_transition_committed_at_requires_enabled_matching_height() {
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.autoscale.enabled = true;
        nexus.autoscale.last_transition_height = 42;

        assert!(autoscale_transition_committed_at(&nexus, 42));
        assert!(!autoscale_transition_committed_at(&nexus, 41));

        nexus.autoscale.enabled = false;
        assert!(!autoscale_transition_committed_at(&nexus, 42));
    }

    struct CommitFixture {
        genesis_key: KeyPair,
        genesis_account_id: AccountId,
        chain_id: ChainId,
        state: State,
        kura: Arc<Kura>,
    }

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("commit fixture key generation should succeed")
    }

    fn checked_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("commit fixture BLS key generation should succeed")
    }

    fn checked_peer() -> PeerId {
        PeerId::new(checked_keypair().public_key().clone())
    }

    fn checked_bls_keypairs(count: usize) -> Vec<KeyPair> {
        (0..count).map(|_| checked_bls_keypair()).collect()
    }

    fn commit_fixture_with_kura(kura: Arc<Kura>) -> CommitFixture {
        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, Arc::clone(&kura), query_handle);
        let chain_id = state.view().chain_id().clone();

        CommitFixture {
            genesis_key,
            genesis_account_id,
            chain_id,
            state,
            kura,
        }
    }

    fn genesis_log_block(
        chain_id: &ChainId,
        genesis_account_id: &AccountId,
        genesis_key: &KeyPair,
        message: &str,
    ) -> SignedBlock {
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(Level::DEBUG, message.to_owned())])
        .sign(genesis_key.private_key());
        SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None)
    }

    fn genesis_log_block_for_state(
        state: &State,
        chain_id: &ChainId,
        genesis_account_id: &AccountId,
        genesis_key: &KeyPair,
        message: &str,
    ) -> SignedBlock {
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(Level::DEBUG, message.to_owned())])
        .sign(genesis_key.private_key());
        let view = state.view();
        let digest = crate::state::compute_confidential_feature_digest(view.world(), &view.zk, 1);
        let confidential_features = (!digest.is_empty()).then_some(digest);
        SignedBlock::genesis(
            vec![tx],
            genesis_key.private_key(),
            confidential_features,
            None,
        )
    }

    fn single_peer_topology() -> Vec<PeerId> {
        vec![checked_peer()]
    }

    fn commit_work(id: u64, block: SignedBlock, topology: Vec<PeerId>) -> CommitWork {
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(4);
        CommitWork {
            id,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender,
        }
    }

    fn execute_commit_work_on_sumeragi_thread(
        state: &State,
        kura: &Kura,
        chain_id: &ChainId,
        genesis_account: &AccountId,
        work: CommitWork,
    ) -> (CommitOutcome, CommitStageTimings) {
        std::thread::scope(|scope| {
            crate::sumeragi::sumeragi_thread_builder("sumeragi-commit-test")
                .spawn_scoped(scope, || {
                    execute_commit_work(state, kura, chain_id, genesis_account, work)
                })
                .expect("spawn commit test worker")
                .join()
                .expect("commit test worker should not panic")
        })
    }

    #[test]
    fn inline_commit_spawn_failure_rejects_without_applying_state() {
        let fixture = commit_fixture_with_kura(Kura::blank_kura_for_testing());
        let block = genesis_log_block_for_state(
            &fixture.state,
            &fixture.chain_id,
            &fixture.genesis_account_id,
            &fixture.genesis_key,
            "inline commit spawn fallback",
        );
        let state = Arc::new(fixture.state);
        let work = commit_work(11, block, single_peer_topology());

        let (outcome, timings) =
            finish_dedicated_commit_spawn(work, Err(io::Error::other("simulated spawn failure")));

        assert!(matches!(
            outcome,
            CommitOutcome::Rejected {
                error: BlockValidationError::ExecutionContextInvalid(message),
                ..
            } if message.contains("simulated spawn failure")
        ));
        assert!(!timings.has_recorded_stages());
        assert_eq!(state.view().height(), 0);
    }

    #[test]
    fn commit_worker_spawn_failure_returns_error_without_handle() {
        let (work_tx, _work_rx) = mpsc::sync_channel::<CommitWork>(1);
        let (_result_tx, result_rx) = mpsc::sync_channel::<CommitResult>(1);

        let result = finish_commit_worker_spawn(
            work_tx,
            result_rx,
            Err(io::Error::other("simulated commit worker spawn failure")),
        );

        assert!(result.is_err());
    }

    fn signers_from_bitmap(signers_bitmap: &[u8], roster_len: usize) -> Vec<usize> {
        let mut signers = Vec::new();
        for (byte_idx, byte) in signers_bitmap.iter().enumerate() {
            for bit in 0u8..8 {
                if byte & (1u8 << bit) == 0 {
                    continue;
                }
                let idx = byte_idx * 8 + usize::from(bit);
                if idx < roster_len {
                    signers.push(idx);
                }
            }
        }
        signers
    }

    fn trusted_self() -> (iroha_config::parameters::actual::TrustedPeers, PeerId) {
        let key_pair = checked_bls_keypair();
        let peer_id = PeerId::new(key_pair.public_key().clone());
        let address: SocketAddr = "127.0.0.1:7016".parse().expect("socket address parses");
        let peer = Peer::new(address.into(), peer_id.clone());
        let pop = iroha_crypto::bls_normal_pop_prove(key_pair.private_key()).expect("pop proves");
        let mut pops = BTreeMap::new();
        pops.insert(peer_id.public_key().clone(), pop);
        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: peer,
            others: UniqueVec::new(),
            pops,
        };
        (trusted, peer_id)
    }

    fn p2p_formal_peer_ids() -> Vec<PeerId> {
        (1..=5)
            .map(|idx| {
                PeerId::new(
                    KeyPair::try_from_seed(
                        format!("p2p-topology-trusted-{idx}").into_bytes(),
                        Algorithm::BlsNormal,
                    )
                    .expect("generate checked P2P topology fixture keypair")
                    .public_key()
                    .clone(),
                )
            })
            .collect()
    }

    fn p2p_peer_set(peers: &[PeerId], indices: &[usize]) -> BTreeSet<PeerId> {
        indices.iter().map(|idx| peers[idx - 1].clone()).collect()
    }

    fn p2p_peer_vec(peers: &[PeerId], indices: &[usize]) -> Vec<PeerId> {
        indices.iter().map(|idx| peers[idx - 1].clone()).collect()
    }

    fn trusted_with_formal_peer_indices(
        peers: &[PeerId],
        trusted_indices: &[usize],
    ) -> iroha_config::parameters::actual::TrustedPeers {
        iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new(
                "127.0.0.1:7101".parse::<SocketAddr>().expect("addr").into(),
                peers[0].clone(),
            ),
            others: trusted_indices
                .iter()
                .enumerate()
                .map(|(offset, idx)| {
                    let port = 7_102_u16
                        .saturating_add(u16::try_from(offset).expect("trusted peer index fits"));
                    Peer::new(
                        format!("127.0.0.1:{port}")
                            .parse::<SocketAddr>()
                            .expect("addr")
                            .into(),
                        peers[idx - 1].clone(),
                    )
                })
                .collect::<UniqueVec<_>>(),
            pops: BTreeMap::new(),
        }
    }

    #[test]
    fn p2p_topology_trusted_formal_gate_matrix() {
        struct Case {
            world: &'static [usize],
            trusted: &'static [usize],
            online: &'static [usize],
            expected_topology: &'static [usize],
            expected_strays: &'static [usize],
        }

        let peers = p2p_formal_peer_ids();
        for case in [
            Case {
                world: &[2, 3],
                trusted: &[],
                online: &[1, 2, 3, 4],
                expected_topology: &[1, 2, 3],
                expected_strays: &[4],
            },
            Case {
                world: &[1, 2],
                trusted: &[3],
                online: &[1, 2, 3, 4],
                expected_topology: &[1, 2, 3],
                expected_strays: &[4],
            },
            Case {
                world: &[1, 2, 3],
                trusted: &[2, 3, 4],
                online: &[4, 5, 4],
                expected_topology: &[1, 2, 3, 4],
                expected_strays: &[5],
            },
            Case {
                world: &[2],
                trusted: &[],
                online: &[1, 2, 3],
                expected_topology: &[1, 2],
                expected_strays: &[3],
            },
            Case {
                world: &[1],
                trusted: &[4],
                online: &[5, 2, 4, 5, 3],
                expected_topology: &[1, 4],
                expected_strays: &[5, 2, 5, 3],
            },
            Case {
                world: &[],
                trusted: &[2],
                online: &[1, 2, 3],
                expected_topology: &[1, 2],
                expected_strays: &[3],
            },
        ] {
            let world = p2p_peer_set(&peers, case.world);
            let trusted = trusted_with_formal_peer_indices(&peers, case.trusted);
            let expected_topology = p2p_peer_set(&peers, case.expected_topology);
            let actual_topology = p2p_topology_with_trusted(&world, &trusted);

            assert_eq!(actual_topology, expected_topology);
            assert_eq!(actual_topology.len(), case.expected_topology.len());

            let online = p2p_peer_vec(&peers, case.online);
            let expected_strays = p2p_peer_vec(&peers, case.expected_strays);
            let actual_strays = peer_ids_outside_topology(&actual_topology, &online);

            assert_eq!(actual_strays, expected_strays);
        }
    }

    #[test]
    fn peer_ids_outside_topology_skips_trusted_observer() {
        let (mut trusted, local_peer) = trusted_self();
        let validator = PeerId::new(checked_bls_keypair().public_key().clone());
        let observer = PeerId::new(checked_bls_keypair().public_key().clone());
        let stranger = PeerId::new(checked_bls_keypair().public_key().clone());
        let observer_peer = Peer::new(
            "127.0.0.1:7017"
                .parse::<SocketAddr>()
                .expect("socket address parses")
                .into(),
            observer.clone(),
        );
        let _ = trusted.others.push(observer_peer);

        let world_peers: BTreeSet<_> = [local_peer.clone(), validator.clone()]
            .into_iter()
            .collect();
        let expected_topology = p2p_topology_with_trusted(&world_peers, &trusted);
        let online = vec![local_peer, validator, observer, stranger.clone()];

        let stray_online = peer_ids_outside_topology(&expected_topology, &online);

        assert_eq!(stray_online, vec![stranger]);
    }

    #[test]
    fn execute_commit_work_emits_pipeline_events_before_state_apply() {
        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let temp_dir = TempDir::new().expect("create temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            eviction_required_replicas:
                iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
        };
        let (kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        let kura = Arc::new(kura);
        assert!(
            !kura.store_root().as_os_str().is_empty(),
            "kura store root should be set for commit roster persistence"
        );
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, Arc::clone(&kura), query_handle);
        let chain_id = state.view().chain_id().clone();

        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(Level::DEBUG, "genesis commit test".to_string())])
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);

        let peer_key = checked_keypair();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, mut events_rx) = tokio::sync::broadcast::channel(64);
        let work = CommitWork {
            id: 1,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender,
        };

        let (outcome, timings) = execute_commit_work_on_sumeragi_thread(
            &state,
            kura.as_ref(),
            &chain_id,
            &genesis_account_id,
            work,
        );
        let CommitOutcome::Success {
            pipeline_events, ..
        } = outcome
        else {
            panic!("expected commit success");
        };
        assert!(timings.qc_verify_ms.is_some());
        assert!(timings.persist_ms.is_some());
        assert!(timings.kura_store_ms.is_some());
        assert!(timings.state_apply_ms.is_some());
        assert!(timings.state_commit_ms.is_some());
        assert!(
            !pipeline_events.is_empty(),
            "commit worker should defer pipeline events until the main loop applies the result"
        );

        let mut got_pipeline_event = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match events_rx.try_recv() {
                Ok(event) => {
                    if matches!(event, EventBox::Pipeline(_) | EventBox::PipelineBatch(_)) {
                        got_pipeline_event = true;
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        assert!(
            !got_pipeline_event,
            "commit worker should not emit pipeline events before the main loop unblocks proposals"
        );
    }

    #[test]
    fn execute_commit_work_persists_commit_roster_without_recording_status_history() {
        let _guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();

        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, Arc::clone(&kura), query_handle);
        let chain_id = state.view().chain_id().clone();

        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(Level::DEBUG, "commit qc test".to_string())])
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let epoch = 0_u64;

        let consensus_key = checked_bls_keypair();
        let consensus_public_key = consensus_key.public_key().clone();
        let keypairs = vec![consensus_key];
        let peer_id = PeerId::new(consensus_public_key);
        let topology = vec![peer_id.clone()];
        let signers_bitmap = vec![0b0000_0001];
        let aggregate_signature = aggregate_signature_for_bitmap(
            &chain_id,
            super::super::PERMISSIONED_TAG,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers_bitmap,
            &keypairs,
        );
        let qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height,
            view,
            epoch,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: super::super::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&topology),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: topology.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap,
                bls_aggregate_signature: aggregate_signature,
            },
        };

        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(4);
        let work = CommitWork {
            id: 7,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: Some(qc.clone()),
            allow_signature_index_recovery: false,
            events_sender,
        };

        let (outcome, _timings) = execute_commit_work_on_sumeragi_thread(
            &state,
            kura.as_ref(),
            &chain_id,
            &genesis_account_id,
            work,
        );
        match &outcome {
            CommitOutcome::Success { .. } => {}
            CommitOutcome::Rejected { error, .. } => {
                panic!("commit rejected: {error:?}");
            }
            CommitOutcome::KuraStoreFailed { error, .. } => {
                panic!("kura store failed: {error:?}");
            }
            CommitOutcome::StateCommitFailed { error, .. } => {
                panic!("state commit failed: {error:?}");
            }
        }
        let history = crate::sumeragi::status::commit_qc_history();
        assert!(
            history.is_empty(),
            "commit worker should not record commit QC before the main loop applies the result"
        );
        let view = state.view();
        let stored = view.world().commit_qcs().get(&block_hash);
        assert_eq!(
            stored,
            Some(&qc),
            "commit worker should persist commit QC into world state for restart recovery"
        );
        let snapshot = state.commit_roster_snapshot_for_block(height, block_hash);
        assert!(
            snapshot.is_some(),
            "commit worker should persist commit-roster evidence before the main loop records status history"
        );
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_validator_checkpoints_for_tests();
    }

    #[test]
    fn execute_commit_work_does_not_advance_state_when_kura_store_fails() {
        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let temp_dir = TempDir::new().expect("create temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: Bytes(0),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            eviction_required_replicas:
                iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
        };
        let (kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        kura.fail_next_store_for_tests();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, Arc::clone(&kura), query_handle);
        let chain_id = state.view().chain_id().clone();

        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(Level::DEBUG, "kura failure test".to_string())])
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);

        let peer_key = checked_keypair();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(4);
        let work = CommitWork {
            id: 1,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender,
        };

        let (outcome, timings) = execute_commit_work_on_sumeragi_thread(
            &state,
            kura.as_ref(),
            &chain_id,
            &genesis_account_id,
            work,
        );
        let CommitOutcome::KuraStoreFailed { error: _, .. } = outcome else {
            panic!("expected Kura store failure");
        };
        assert!(timings.persist_ms.is_some());
        assert!(timings.kura_store_ms.is_some());
        assert!(timings.state_apply_ms.is_none());
        assert!(timings.state_commit_ms.is_none());
        assert_eq!(state.view().height(), 0);
        assert_eq!(kura.blocks_count(), 0);
    }

    #[test]
    fn execute_commit_work_persists_block_before_exposing_committed_state() {
        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, Arc::clone(&kura), query_handle);
        let chain_id = state.view().chain_id().clone();

        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(Level::DEBUG, "kura ordering test".to_string())])
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);

        let peer_key = checked_keypair();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(4);
        let work = CommitWork {
            id: 2,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender,
        };

        let (outcome, timings) = execute_commit_work_on_sumeragi_thread(
            &state,
            kura.as_ref(),
            &chain_id,
            &genesis_account_id,
            work,
        );
        let CommitOutcome::Success {
            committed_block, ..
        } = outcome
        else {
            panic!("expected commit success");
        };
        assert!(timings.kura_store_ms.is_some());
        assert!(timings.state_apply_ms.is_some());
        assert!(timings.state_commit_ms.is_some());
        assert_eq!(state.view().height(), 1);
        assert_eq!(kura.blocks_count(), 1);
        assert!(
            kura.commit_manifest(1)
                .expect("read commit manifest")
                .is_some(),
            "successful commit should persist a Kura commit manifest"
        );
        let latest = state.view().latest_block().expect("latest committed block");
        assert_eq!(latest.hash(), committed_block.as_ref().hash());
    }

    #[test]
    fn execute_commit_work_accepts_already_durable_block_without_duplicate_append() {
        let fixture = commit_fixture_with_kura(Kura::blank_kura_for_testing());
        let block = genesis_log_block(
            &fixture.chain_id,
            &fixture.genesis_account_id,
            &fixture.genesis_key,
            "idempotent kura retry",
        );
        let block_hash = block.hash();
        fixture
            .kura
            .store_block(block.clone())
            .expect("pre-store committed block in Kura");
        let lengths_before = fixture.kura.block_file_lengths_for_tests();

        let (outcome, timings) = execute_commit_work_on_sumeragi_thread(
            &fixture.state,
            fixture.kura.as_ref(),
            &fixture.chain_id,
            &fixture.genesis_account_id,
            commit_work(3, block, single_peer_topology()),
        );
        let CommitOutcome::Success { .. } = outcome else {
            panic!("expected idempotent commit success");
        };

        assert!(timings.kura_store_ms.is_some());
        assert!(timings.state_commit_ms.is_some());
        assert_eq!(fixture.state.view().height(), 1);
        assert_eq!(fixture.kura.blocks_count(), 1);
        assert_eq!(
            fixture
                .kura
                .get_durable_block_hash(std::num::NonZeroUsize::new(1).expect("non-zero height")),
            Some(block_hash)
        );
        let lengths_after = fixture.kura.block_file_lengths_for_tests();
        assert_eq!(
            lengths_after, lengths_before,
            "retrying an already durable block must not append duplicate bytes"
        );
    }

    #[test]
    fn execute_commit_work_treats_post_commit_sidecar_failures_as_nonfatal() {
        let fixture = commit_fixture_with_kura(Kura::blank_kura_for_testing());
        let block = genesis_log_block_for_state(
            &fixture.state,
            &fixture.chain_id,
            &fixture.genesis_account_id,
            &fixture.genesis_key,
            "post-commit sidecar failure",
        );
        let block_hash = block.hash();
        fixture.kura.fail_next_wsv_checkpoint_write_for_tests();
        fixture.kura.fail_next_commit_manifest_write_for_tests();

        let (outcome, timings) = execute_commit_work_on_sumeragi_thread(
            &fixture.state,
            fixture.kura.as_ref(),
            &fixture.chain_id,
            &fixture.genesis_account_id,
            commit_work(6, block, single_peer_topology()),
        );

        let CommitOutcome::Success {
            post_commit_persistence_error,
            ..
        } = &outcome
        else {
            panic!(
                "post-commit sidecar failures must not roll back the committed block: {outcome:?}"
            );
        };
        let error = post_commit_persistence_error
            .as_deref()
            .expect("sidecar failure should be reported");
        assert!(
            error.contains("WSV checkpoint"),
            "checkpoint sidecar failure must be surfaced: {error}"
        );
        assert!(
            error.contains("commit manifest"),
            "commit-manifest sidecar failure must be surfaced: {error}"
        );
        assert!(timings.kura_store_ms.is_some());
        assert!(timings.state_commit_ms.is_some());
        assert_eq!(fixture.state.view().height(), 1);
        assert_eq!(fixture.kura.blocks_count(), 1);
        assert_eq!(
            fixture
                .kura
                .get_durable_block_hash(std::num::NonZeroUsize::new(1).expect("non-zero height")),
            Some(block_hash)
        );
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read checkpoint")
                .is_none(),
            "injected checkpoint failure should leave only the canonical block durable"
        );
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read manifest")
                .is_none(),
            "injected manifest failure should leave only the canonical block durable"
        );
    }

    #[test]
    fn execute_commit_work_uses_trusted_validated_commit_artifact() {
        let fixture = commit_fixture_with_kura(Kura::blank_kura_for_testing());
        let block = genesis_log_block(
            &fixture.chain_id,
            &fixture.genesis_account_id,
            &fixture.genesis_key,
            "prevalidated commit artifact",
        );
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let mut state_block = fixture.state.block(block.header());
        let _valid = crate::block::ValidBlock::validate_unchecked(block.clone(), &mut state_block)
            .unpack(|_| {});
        let exec_witness = state_block
            .take_exec_witness()
            .expect("validation captures execution witness");
        let artifact = ValidatedCommitArtifact {
            block_hash,
            height,
            view,
            parent_state_root: parent_state_from_witness(&exec_witness),
            post_state_root: post_state_from_witness(&exec_witness),
        };
        drop(state_block);

        let consensus_key = checked_bls_keypair();
        let topology = vec![PeerId::new(consensus_key.public_key().clone())];
        let signers_bitmap = vec![0b0000_0001];
        let keypairs = vec![consensus_key];
        let qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: artifact.parent_state_root,
            post_state_root: artifact.post_state_root,
            height,
            view,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: super::super::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&topology),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: topology.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                bls_aggregate_signature: aggregate_signature_for_bitmap(
                    &fixture.chain_id,
                    super::super::PERMISSIONED_TAG,
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    height,
                    view,
                    0,
                    &signers_bitmap,
                    &keypairs,
                ),
                signers_bitmap,
            },
        };
        let mut work = commit_work(5, block, topology);
        work.validated_commit_artifact = Some(artifact);
        work.commit_qc = Some(qc);

        let (outcome, timings) = execute_commit_work_on_sumeragi_thread(
            &fixture.state,
            fixture.kura.as_ref(),
            &fixture.chain_id,
            &fixture.genesis_account_id,
            work,
        );
        let CommitOutcome::Success { .. } = outcome else {
            panic!("expected commit success");
        };
        assert!(
            timings.used_prevalidated_artifact,
            "matching artifact and commit QC roots should use the prevalidated commit path"
        );
    }

    #[test]
    fn execute_commit_work_rejects_kura_height_conflict_before_state_commit() {
        let fixture = commit_fixture_with_kura(Kura::blank_kura_for_testing());
        let stored = genesis_log_block(
            &fixture.chain_id,
            &fixture.genesis_account_id,
            &fixture.genesis_key,
            "stored competing genesis",
        );
        let candidate = genesis_log_block(
            &fixture.chain_id,
            &fixture.genesis_account_id,
            &fixture.genesis_key,
            "candidate genesis",
        );
        let stored_hash = stored.hash();
        let candidate_hash = candidate.hash();
        assert_ne!(stored_hash, candidate_hash);
        fixture
            .kura
            .store_block(stored)
            .expect("pre-store conflicting block in Kura");

        let (outcome, timings) = execute_commit_work_on_sumeragi_thread(
            &fixture.state,
            fixture.kura.as_ref(),
            &fixture.chain_id,
            &fixture.genesis_account_id,
            commit_work(4, candidate, single_peer_topology()),
        );
        let CommitOutcome::KuraStoreFailed { error, .. } = outcome else {
            panic!("expected Kura conflict before state commit");
        };

        assert!(matches!(
            error,
            crate::kura::Error::BlockHeightConflict {
                height: 1,
                expected,
                actual,
            } if expected == stored_hash && actual == candidate_hash
        ));
        assert!(timings.kura_store_ms.is_some());
        assert!(timings.state_apply_ms.is_none());
        assert!(timings.state_commit_ms.is_none());
        assert_eq!(
            fixture.state.view().height(),
            0,
            "state must not advance when Kura rejects the canonical height"
        );
        assert_eq!(fixture.kura.blocks_count(), 1);
        assert_eq!(
            fixture
                .kura
                .get_durable_block_hash(std::num::NonZeroUsize::new(1).expect("non-zero height")),
            Some(stored_hash)
        );
    }

    #[test]
    fn commit_worker_wakes_on_result() {
        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(
            world,
            Arc::clone(&kura),
            query_handle,
        ));
        let chain_id = state.view().chain_id().clone();
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);

        let handle = spawn_commit_worker(
            Arc::clone(&state),
            Arc::clone(&kura),
            chain_id.clone(),
            genesis_account_id.clone(),
            Some(wake_tx),
            1,
            1,
        )
        .expect("spawn commit worker");

        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(
            Level::DEBUG,
            "commit worker wake test".to_string(),
        )])
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let peer_key = checked_keypair();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(16);
        let work = CommitWork {
            id: 42,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender,
        };

        handle.work_tx.send(work).expect("send commit work");
        let result = handle
            .result_rx
            .recv_timeout(COMMIT_WORKER_TIMEOUT)
            .expect("commit result");
        assert!(result.timings.qc_verify_ms.is_some());
        assert!(result.timings.persist_ms.is_some());
        assert!(result.timings.kura_store_ms.is_some());
        assert!(result.timings.state_apply_ms.is_some());
        assert!(result.timings.state_commit_ms.is_some());
        wake_rx
            .recv_timeout(COMMIT_WORKER_TIMEOUT)
            .expect("wake signal");

        drop(handle.work_tx);
        if let Err(err) = handle.join_handle.join() {
            panic!("commit worker panicked: {err:?}");
        }
    }

    #[test]
    fn commit_worker_wakes_when_result_queue_full() {
        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(
            world,
            Arc::clone(&kura),
            query_handle,
        ));
        let chain_id = state.view().chain_id().clone();
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);

        let handle = spawn_commit_worker(
            Arc::clone(&state),
            Arc::clone(&kura),
            chain_id.clone(),
            genesis_account_id.clone(),
            Some(wake_tx),
            1,
            1,
        )
        .expect("spawn commit worker");

        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(
            Level::DEBUG,
            "commit worker queue-full wake test".to_string(),
        )])
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let peer_key = checked_keypair();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(16);

        let work = CommitWork {
            id: 100,
            block: block.clone(),
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology.clone(),
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender: events_sender.clone(),
        };

        handle.work_tx.send(work).expect("send commit work 1");
        wake_rx
            .recv_timeout(COMMIT_WORKER_TIMEOUT)
            .expect("wake for first result");

        // Keep the result queue full, then enqueue another commit.
        let work = CommitWork {
            id: 101,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender,
        };
        handle.work_tx.send(work).expect("send commit work 2");
        wake_rx
            .recv_timeout(COMMIT_WORKER_TIMEOUT)
            .expect("wake while result queue full");

        let _result = handle
            .result_rx
            .recv_timeout(COMMIT_WORKER_TIMEOUT)
            .expect("commit result 1");
        let _result = handle
            .result_rx
            .recv_timeout(COMMIT_WORKER_TIMEOUT)
            .expect("commit result 2");
        let _ = wake_rx.try_recv();

        drop(handle.work_tx);
        if let Err(err) = handle.join_handle.join() {
            panic!("commit worker panicked: {err:?}");
        }
    }

    #[test]
    fn commit_worker_does_not_block_on_full_wake_channel() {
        let genesis_key = checked_keypair();
        let genesis_account_id = AccountId::new(genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(
            world,
            Arc::clone(&kura),
            query_handle,
        ));
        let chain_id = state.view().chain_id().clone();
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        wake_tx.try_send(()).expect("prefill wake");

        let handle = spawn_commit_worker(
            Arc::clone(&state),
            Arc::clone(&kura),
            chain_id.clone(),
            genesis_account_id.clone(),
            Some(wake_tx),
            1,
            1,
        )
        .expect("spawn commit worker");

        let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(0));
        let tx = TransactionBuilder::new_with_time_source(
            chain_id.clone(),
            genesis_account_id.clone(),
            &time_source,
        )
        .with_instructions([Log::new(
            Level::DEBUG,
            "commit worker full wake test".to_string(),
        )])
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let peer_key = checked_keypair();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(16);
        let work = CommitWork {
            id: 43,
            block,
            validated_commit_artifact: None,
            commit_topology: topology.clone(),
            signature_topology: topology,
            consensus_mode: ConsensusMode::Permissioned,
            qc_signers: None,
            commit_qc: None,
            allow_signature_index_recovery: false,
            events_sender,
        };

        handle.work_tx.send(work).expect("send commit work");
        let result = handle
            .result_rx
            .recv_timeout(COMMIT_WORKER_TIMEOUT)
            .expect("commit result");
        assert!(result.timings.qc_verify_ms.is_some());
        assert!(result.timings.persist_ms.is_some());
        assert!(result.timings.kura_store_ms.is_some());
        assert!(result.timings.state_apply_ms.is_some());
        assert!(result.timings.state_commit_ms.is_some());

        assert!(wake_rx.try_recv().is_ok(), "prefilled wake should remain");
        assert!(matches!(wake_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));

        drop(handle.work_tx);
        if let Err(err) = handle.join_handle.join() {
            panic!("commit worker panicked: {err:?}");
        }
    }

    #[test]
    fn commit_quorum_signers_requires_min_votes() {
        let min_votes_for_commit = 3;
        assert!(!has_commit_quorum_signers(None, min_votes_for_commit));

        let mut signers = BTreeSet::from([0_u32, 1_u32]);
        assert!(!has_commit_quorum_signers(
            Some(&signers),
            min_votes_for_commit
        ));

        signers.insert(2_u32);
        assert!(has_commit_quorum_signers(
            Some(&signers),
            min_votes_for_commit
        ));
    }

    #[test]
    fn sign_vote_with_local_key_attaches_verifiable_signature() {
        let chain = "test-chain".parse::<ChainId>().expect("chain id");
        let key_pair = checked_bls_keypair();
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let mut vote = crate::sumeragi::consensus::Vote {
            phase: crate::sumeragi::consensus::Phase::Prepare,
            block_hash: sample_block(3, 0).hash(),
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 3,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: vec![0xAA; 4],
        };

        sign_vote_with_local_key(&chain, PERMISSIONED_TAG, key_pair.private_key(), &mut vote)
            .expect("local vote signing succeeds");

        assert!(!vote.bls_sig.is_empty());
        assert_ne!(vote.bls_sig, vec![0xAA; 4]);
        let preimage = vote_preimage(&chain, PERMISSIONED_TAG, &vote);
        Signature::from_bytes(&vote.bls_sig)
            .verify(key_pair.public_key(), &preimage)
            .expect("signed vote verifies against local key");
    }

    #[test]
    fn block_sync_update_targets_cap_and_excludes_local() {
        let local = checked_peer();
        let peers: Vec<_> = (0..6).map(|_| checked_peer()).collect();
        let mut online = Vec::new();
        online.push(local.clone());
        online.extend(peers.clone());
        let seed = [0xB1; 32];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local, 3, &online, &online, &online, &online, &seed,
        );
        let repeat = Actor::block_sync_update_targets_for_peers(
            &local, 3, &online, &online, &online, &online, &seed,
        );

        assert_eq!(targets, repeat);
        assert_eq!(targets.len(), 3);
        assert!(!targets.contains(&local));
        assert!(targets.iter().all(|peer| online.contains(peer)));
    }

    #[test]
    fn block_sync_update_targets_prioritizes_strays() {
        let local = checked_peer();
        let world_peers: Vec<_> = (0..2).map(|_| checked_peer()).collect();
        let stray_peers: Vec<_> = (0..2).map(|_| checked_peer()).collect();
        let mut online = Vec::new();
        online.push(local.clone());
        online.extend(world_peers.clone());
        online.extend(stray_peers.clone());
        let mut world = Vec::with_capacity(world_peers.len() + 1);
        world.push(local.clone());
        world.extend(world_peers.clone());
        let mut registered = world.clone();
        registered.extend(stray_peers.clone());
        let seed = [0xCA; 32];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            2,
            &world,
            &registered,
            &registered,
            &online,
            &seed,
        );

        assert_eq!(targets.len(), 2);
        assert!(!targets.contains(&local));
        assert!(targets.iter().all(|peer| stray_peers.contains(peer)));
    }

    #[test]
    fn block_sync_update_targets_skip_unregistered_strays() {
        let local = checked_peer();
        let world_peers: Vec<_> = (0..2).map(|_| checked_peer()).collect();
        let stray = checked_peer();
        let mut online = Vec::new();
        online.push(local.clone());
        online.extend(world_peers.clone());
        online.push(stray.clone());
        let mut world = Vec::with_capacity(world_peers.len() + 1);
        world.push(local.clone());
        world.extend(world_peers.clone());
        let registered = world.clone();
        let seed = [0x5E; 32];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            3,
            &world,
            &registered,
            &[],
            &online,
            &seed,
        );

        assert!(!targets.contains(&stray));
    }

    #[test]
    fn block_sync_update_targets_include_trusted_unregistered() {
        let local = checked_peer();
        let stray = checked_peer();
        let world = vec![local.clone()];
        let registered = world.clone();
        let trusted = vec![stray.clone()];
        let online = vec![local.clone(), stray.clone()];
        let seed = [0x7D; 32];

        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            1,
            &world,
            &registered,
            &trusted,
            &online,
            &seed,
        );

        assert_eq!(targets, vec![stray]);
    }

    #[test]
    fn block_sync_update_targets_for_peers_prefers_online_world() {
        let local = checked_peer();
        let peer_a = checked_peer();
        let peer_b = checked_peer();
        let peer_c = checked_peer();
        let peers = vec![local.clone(), peer_a, peer_b.clone(), peer_c];
        let online = vec![local.clone(), peer_b.clone()];
        let seed = [0x12; 32];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            3,
            &peers,
            &peers,
            &[],
            &online,
            &seed,
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], peer_b);
        assert!(online.contains(&targets[0]));
        assert!(!targets.contains(&local));
    }

    #[test]
    fn block_sync_update_targets_for_peers_fallback_to_world() {
        let local = checked_peer();
        let peer_a = checked_peer();
        let peer_b = checked_peer();
        let peers = vec![local.clone(), peer_a.clone(), peer_b.clone()];
        let online = vec![local.clone()];
        let seed = [0xDE; 32];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            1,
            &peers,
            &peers,
            &[],
            &online,
            &seed,
        );
        let repeat = Actor::block_sync_update_targets_for_peers(
            &local,
            1,
            &peers,
            &peers,
            &[],
            &online,
            &seed,
        );

        assert_eq!(targets, repeat);
        assert_eq!(targets.len(), 1);
        assert!(!targets.contains(&local));
        assert!(targets.iter().all(|peer| peers.contains(peer)));
    }

    #[test]
    fn block_sync_update_targets_formal_gate_matrix() {
        let local = checked_peer();
        let world_a = checked_peer();
        let world_b = checked_peer();
        let world_offline = checked_peer();
        let registered_stray = checked_peer();
        let trusted_stray = checked_peer();
        let unregistered_stray = checked_peer();
        let seed = [0xA5; 32];
        let set = |peers: &[PeerId]| peers.iter().cloned().collect::<BTreeSet<_>>();

        let world = vec![local.clone(), world_a.clone()];
        let registered = vec![local.clone(), world_a.clone()];
        let online = vec![local.clone(), world_a.clone()];
        assert!(
            Actor::block_sync_update_targets_for_peers(
                &local,
                0,
                &world,
                &registered,
                &[],
                &online,
                &seed,
            )
            .is_empty(),
            "zero gossip limit should fail closed"
        );
        assert!(
            Actor::block_sync_update_targets_for_peers(
                &local,
                2,
                &[],
                &registered,
                &[],
                &online,
                &seed,
            )
            .is_empty(),
            "empty world-peer input should not target online strays"
        );

        let world = vec![local.clone(), world_a.clone()];
        let registered = vec![local.clone(), world_a.clone(), registered_stray.clone()];
        let trusted = vec![trusted_stray.clone()];
        let online = vec![
            local.clone(),
            world_a.clone(),
            registered_stray.clone(),
            trusted_stray.clone(),
        ];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            2,
            &world,
            &registered,
            &trusted,
            &online,
            &seed,
        );
        assert_eq!(
            set(&targets),
            BTreeSet::from([registered_stray.clone(), trusted_stray.clone()]),
            "registered and trusted strays should fill the cap before world peers"
        );

        let world = vec![local.clone(), world_a.clone(), world_b.clone()];
        let registered = vec![
            local.clone(),
            world_a.clone(),
            world_b.clone(),
            registered_stray.clone(),
        ];
        let online = vec![
            local.clone(),
            world_a.clone(),
            world_b.clone(),
            registered_stray.clone(),
        ];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            3,
            &world,
            &registered,
            &[],
            &online,
            &seed,
        );
        assert_eq!(
            set(&targets),
            BTreeSet::from([registered_stray.clone(), world_a.clone(), world_b.clone()]),
            "stray priority should still backfill remaining capacity with online world peers"
        );

        let world = vec![local.clone(), world_a.clone()];
        let registered = vec![local.clone(), world_a.clone()];
        let online = vec![local.clone(), world_a.clone(), unregistered_stray.clone()];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            2,
            &world,
            &registered,
            &[],
            &online,
            &seed,
        );
        assert_eq!(
            set(&targets),
            BTreeSet::from([world_a.clone()]),
            "unregistered online strays should be ignored"
        );
        assert!(
            !targets.contains(&unregistered_stray),
            "unregistered stray should never be selected"
        );

        let world = vec![local.clone()];
        let registered = vec![local.clone()];
        let trusted = vec![trusted_stray.clone()];
        let online = vec![local.clone(), trusted_stray.clone()];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            1,
            &world,
            &registered,
            &trusted,
            &online,
            &seed,
        );
        assert_eq!(
            targets,
            vec![trusted_stray.clone()],
            "trusted online strays should be eligible even when absent from world peers"
        );

        let world = vec![local.clone(), world_a.clone(), world_offline.clone()];
        let registered = world.clone();
        let online = vec![local.clone(), world_a.clone()];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            2,
            &world,
            &registered,
            &[],
            &online,
            &seed,
        );
        assert_eq!(
            targets,
            vec![world_a.clone()],
            "online world peers should be preferred over offline world fallback"
        );

        let online = vec![local.clone()];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            1,
            &world,
            &registered,
            &[],
            &online,
            &seed,
        );
        assert_eq!(
            targets.len(),
            1,
            "world fallback should fill when no world peer is online"
        );
        assert!(
            !targets.contains(&local),
            "world fallback must not target local"
        );
        assert!(
            targets.iter().all(|peer| world.contains(peer)),
            "world fallback should use only world peers"
        );

        let world = vec![local.clone(), world_a.clone(), world_b.clone()];
        let registered = world.clone();
        let online = vec![local.clone(), world_a.clone(), world_b.clone()];
        let targets = Actor::block_sync_update_targets_for_peers(
            &local,
            1,
            &world,
            &registered,
            &[],
            &online,
            &seed,
        );
        assert_eq!(
            targets.len(),
            1,
            "world-only selection must obey the gossip cap"
        );
        assert!(
            !targets.contains(&local),
            "world-only selection must exclude local"
        );
        assert!(
            targets
                .iter()
                .all(|peer| [world_a.clone(), world_b.clone()].contains(peer)),
            "world-only selection should use eligible remote world peers"
        );

        let world = vec![local.clone()];
        let registered = vec![local.clone()];
        let online = vec![local.clone()];
        assert!(
            Actor::block_sync_update_targets_for_peers(
                &local,
                2,
                &world,
                &registered,
                &[],
                &online,
                &seed,
            )
            .is_empty(),
            "only-local world and online inputs should yield no targets"
        );
    }

    fn qc_preimage(
        chain_id: &ChainId,
        mode_tag: &str,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> Vec<u8> {
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let vote = crate::sumeragi::consensus::Vote {
            phase,
            block_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height,
            view,
            epoch,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        crate::sumeragi::consensus::vote_preimage(chain_id, mode_tag, &vote)
    }

    #[allow(clippy::too_many_arguments)]
    fn aggregate_signature_for_bitmap(
        chain_id: &ChainId,
        mode_tag: &str,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        signers_bitmap: &[u8],
        keypairs: &[KeyPair],
    ) -> Vec<u8> {
        let preimage = qc_preimage(chain_id, mode_tag, phase, block_hash, height, view, epoch);
        let signers = signers_from_bitmap(signers_bitmap, keypairs.len());
        let mut signatures = Vec::with_capacity(signers.len());
        for idx in signers {
            let kp = keypairs.get(idx).expect("keypair for signer");
            let sig = Signature::try_new(kp.private_key(), &preimage)
                .expect("sign checked commit QC fixture");
            signatures.push(sig.payload().to_vec());
        }
        let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
        iroha_crypto::bls_normal_aggregate_signatures(&sig_refs).expect("aggregate signature")
    }

    fn sample_block(height: u64, view: u64) -> SignedBlock {
        let header = BlockHeader {
            height: core::num::NonZeroU64::new(height).expect("non-zero height"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
            npos_effects_hash: None,
            execution_context_hash: None,
            sccp_commitment_root: None,
            creation_time_ms: 0,
            view_change_index: view,
            confidential_features: None,
        };
        let key_pair =
            KeyPair::try_random().expect("generate checked commit block fixture keypair");
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::try_from_hash(&private_key, header.hash())
            .expect("sign checked commit block fixture hash");
        let block_signature = BlockSignature::new(0, signature);
        SignedBlock::presigned(block_signature, header, Vec::<SignedTransaction>::new())
    }

    #[test]
    fn local_payload_matches_hash_accepts_block_payload() {
        let block = sample_block(2, 0);
        let payload_hash = Hash::new(super::super::proposals::block_payload_bytes(&block));
        assert!(Actor::local_payload_matches_hash(&block, &payload_hash));
    }

    #[test]
    fn local_payload_matches_hash_rejects_mismatched_payload() {
        let block = sample_block(2, 0);
        let payload_hash = Hash::new(b"not-a-payload");
        assert!(!Actor::local_payload_matches_hash(&block, &payload_hash));
    }

    #[test]
    fn payload_available_for_da_accepts_local_payload_without_rbc() {
        let block = sample_block(2, 0);
        let payload_hash = Hash::new(super::super::proposals::block_payload_bytes(&block));
        let pending = PendingBlock::new(block, payload_hash, 2, 0);
        let sessions = BTreeMap::new();
        let handle = rbc_status::Handle::new();

        assert!(Actor::payload_available_for_da_from_sessions(
            &sessions, &handle, &pending
        ));
    }

    #[test]
    fn payload_available_for_da_ignores_summary_only_rbc_delivery() {
        let block = sample_block(2, 0);
        let payload_hash = Hash::new(b"not-a-payload");
        let pending = PendingBlock::new(block, payload_hash, 2, 0);
        let sessions = BTreeMap::new();
        let handle = rbc_status::Handle::new();

        let summary = rbc_status::Summary {
            block_hash: pending.block.hash(),
            height: pending.height,
            view: pending.view,
            total_chunks: 1,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: true,
            payload_hash: Some(pending.payload_hash),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary, std::time::SystemTime::now());

        assert!(
            !Actor::payload_available_for_da_from_sessions(&sessions, &handle, &pending),
            "summary-only RBC delivery does not carry payload bytes and must not satisfy DA availability",
        );
    }

    #[test]
    fn payload_available_for_da_accepts_complete_rbc_payload_without_delivery() {
        let block = sample_block(2, 0);
        let payload = b"authoritative-rbc-payload".to_vec();
        let payload_hash = Hash::new(&payload);
        let pending = PendingBlock::new(block.clone(), payload_hash, 2, 0);
        let mut sessions = BTreeMap::new();
        let handle = rbc_status::Handle::new();

        let mut session = Actor::build_rbc_session_from_payload(&payload, payload_hash, 1024, 0)
            .expect("rbc session");
        session.test_set_block_header_and_signature(&block);
        sessions.insert((block.hash(), 2, 0), session);

        assert!(Actor::payload_available_for_da_from_sessions(
            &sessions, &handle, &pending
        ));
    }

    #[test]
    fn payload_available_for_da_rejects_complete_rbc_payload_without_metadata() {
        let block = sample_block(2, 0);
        let payload = b"authoritative-rbc-payload".to_vec();
        let payload_hash = Hash::new(&payload);
        let pending = PendingBlock::new(block.clone(), payload_hash, 2, 0);
        let mut sessions = BTreeMap::new();
        let handle = rbc_status::Handle::new();

        let session = Actor::build_rbc_session_from_payload(&payload, payload_hash, 1024, 0)
            .expect("rbc session");
        sessions.insert((block.hash(), 2, 0), session);

        assert!(
            !Actor::payload_available_for_da_from_sessions(&sessions, &handle, &pending),
            "DA availability must not accept RBC bytes that are not bound to block metadata",
        );
    }

    #[test]
    fn payload_available_for_da_rejects_complete_rbc_payload_with_wrong_bytes() {
        let block = sample_block(2, 0);
        let payload_hash = Hash::new(b"advertised-rbc-payload");
        let pending = PendingBlock::new(block.clone(), payload_hash, 2, 0);
        let mut sessions = BTreeMap::new();
        let handle = rbc_status::Handle::new();

        let mut session = RbcSession::test_new(1, Some(payload_hash), None, 0);
        session.test_set_block_header_and_signature(&block);
        session.test_note_chunk(0, b"different-complete-bytes".to_vec(), 0);
        sessions.insert((block.hash(), 2, 0), session);

        assert!(
            !Actor::payload_available_for_da_from_sessions(&sessions, &handle, &pending),
            "DA availability must not accept complete RBC counters when the reconstructed bytes hash differently",
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn block_sync_update_has_roster_requires_stake_snapshot_in_npos() {
        let block = sample_block(4, 0);
        let mut update = super::super::message::BlockSyncUpdate::from(&block);

        assert!(!super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Permissioned
        ));
        assert!(!super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Npos
        ));

        let keypair = checked_bls_keypair();
        let validator_set = vec![iroha_data_model::peer::PeerId::new(
            keypair.public_key().clone(),
        )];
        update.commit_qc = Some(crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block.hash(),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 4,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: super::super::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![0xAA; 96],
            },
        });

        assert!(super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Permissioned
        ));
        assert!(!super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Npos
        ));

        update.stake_snapshot = Some(crate::sumeragi::stake_snapshot::CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&validator_set),
            entries: validator_set
                .iter()
                .cloned()
                .map(
                    |peer_id| crate::sumeragi::stake_snapshot::CommitStakeSnapshotEntry {
                        peer_id,
                        stake: Numeric::from(1_u64),
                    },
                )
                .collect(),
        });
        assert!(super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Npos
        ));

        update.commit_qc = None;
        update.stake_snapshot = None;
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        update.validator_checkpoint =
            Some(iroha_data_model::consensus::ValidatorSetCheckpoint::new(
                4,
                0,
                block.hash(),
                zero_root,
                zero_root,
                validator_set,
                vec![0b0000_0001],
                vec![0xBB; 96],
                iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                None,
            ));

        assert!(super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Permissioned
        ));
        assert!(!super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Npos
        ));

        update.stake_snapshot = Some(crate::sumeragi::stake_snapshot::CommitStakeSnapshot {
            validator_set_hash: HashOf::new(
                &update
                    .validator_checkpoint
                    .as_ref()
                    .expect("checkpoint present")
                    .validator_set,
            ),
            entries: update
                .validator_checkpoint
                .as_ref()
                .expect("checkpoint present")
                .validator_set
                .iter()
                .cloned()
                .map(
                    |peer_id| crate::sumeragi::stake_snapshot::CommitStakeSnapshotEntry {
                        peer_id,
                        stake: Numeric::from(1_u64),
                    },
                )
                .collect(),
        });
        assert!(super::super::block_sync_update_has_roster(
            &update,
            ConsensusMode::Npos
        ));
    }

    #[test]
    fn block_sync_update_attaches_cached_qcs() {
        let block = sample_block(4, 0);
        let block_hash = block.hash();
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        let chain: ChainId = "block-sync-qcs".parse().expect("chain id parses");
        let signers_bitmap = vec![0b0000_0111];
        let keypairs = checked_bls_keypairs(3);
        let validator_set: Vec<_> = keypairs
            .iter()
            .map(|kp| iroha_data_model::peer::PeerId::new(kp.public_key().clone()))
            .collect();
        let validator_set_hash = HashOf::new(&validator_set);
        let make_cert = |phase| crate::sumeragi::consensus::Qc {
            phase,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 4,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: super::super::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash,
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: aggregate_signature_for_bitmap(
                    &chain,
                    super::super::PERMISSIONED_TAG,
                    phase,
                    block_hash,
                    4,
                    0,
                    0,
                    &signers_bitmap,
                    &keypairs,
                ),
            },
        };
        let qc_precommit = make_cert(crate::sumeragi::consensus::Phase::Commit);

        let mut qc_cache = BTreeMap::new();
        qc_cache.insert(
            (
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                4,
                0,
                0,
                crate::sumeragi::consensus::default_chain_order_hash(),
                0,
            ),
            qc_precommit.clone(),
        );
        let vote = crate::sumeragi::consensus::Vote {
            phase: crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 4,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut vote_log = BTreeMap::new();
        vote_log.insert(
            (
                crate::sumeragi::consensus::Phase::Commit,
                4,
                0,
                0,
                0,
                crate::sumeragi::consensus::default_chain_order_hash(),
                0,
            ),
            vote,
        );
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &qc_cache,
            &vote_log,
            block_hash,
            4,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );

        assert_eq!(update.commit_qc, Some(qc_precommit));
        assert_eq!(update.commit_votes.len(), 1);
    }

    #[test]
    fn cached_precommit_signers_attach_to_block_sync_update() {
        let _history_guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_precommit_signer_history_for_tests();

        let chain: ChainId = "block-sync-precommit-signers"
            .parse()
            .expect("chain id parses");
        let block = sample_block(7, 2);
        let block_hash = block.hash();
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let epoch = 0;
        let keypairs = checked_bls_keypairs(4);
        let signers: BTreeSet<_> = [0_u32, 1_u32, 2_u32].into_iter().collect();
        let signers_bitmap = vec![0b0000_0111];
        let validator_set: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let aggregate_signature = aggregate_signature_for_bitmap(
            &chain,
            super::super::PERMISSIONED_TAG,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers_bitmap,
            &keypairs,
        );
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);

        crate::sumeragi::status::record_precommit_signers(
            crate::sumeragi::status::PrecommitSignerRecord {
                block_hash,
                height,
                view,
                epoch,
                chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
                rechain_seq: 0,
                parent_state_root: zero_root,
                post_state_root: zero_root,
                signers,
                bls_aggregate_signature: aggregate_signature.clone(),
                roster_len: keypairs.len(),
                mode_tag: super::super::PERMISSIONED_TAG.to_string(),
                validator_set,
                stake_snapshot: None,
            },
        );

        let mut update = super::message::BlockSyncUpdate::from(&block);
        let qc_cache = BTreeMap::new();
        let vote_log = BTreeMap::new();

        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &qc_cache,
            &vote_log,
            block_hash,
            height,
            view,
            epoch,
            &state,
            ConsensusMode::Permissioned,
        );

        let qc = update
            .commit_qc
            .expect("derived certificate should be attached");
        assert_eq!(qc.height, height);
        assert_eq!(qc.view, view);
        assert_eq!(qc.epoch, epoch);
        assert_eq!(qc.subject_block_hash, block_hash);
        assert_eq!(qc.aggregate.signers_bitmap, signers_bitmap);
        assert_eq!(qc.aggregate.bls_aggregate_signature, aggregate_signature);

        crate::sumeragi::status::reset_precommit_signer_history_for_tests();
    }

    #[test]
    fn cached_qc_builds_precommit_signer_record() {
        let chain: ChainId = "cached-qc-precommit-signers"
            .parse()
            .expect("chain id parses");
        let block = sample_block(8, 1);
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let epoch = 0;
        let roster_len = 4;
        let signers_bitmap = vec![0b0000_0111];
        let keypairs = checked_bls_keypairs(4);
        let validator_set: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let aggregate_signature = aggregate_signature_for_bitmap(
            &chain,
            super::super::PERMISSIONED_TAG,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers_bitmap,
            &keypairs,
        );
        let qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height,
            view,
            epoch,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: super::super::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: aggregate_signature.clone(),
            },
        };

        let record = Actor::precommit_signer_record_from_cached_qc(
            &qc,
            &validator_set,
            ConsensusMode::Permissioned,
            None,
        )
        .expect("record built");

        let expected_signers: BTreeSet<_> = [0_u32, 1_u32, 2_u32].into_iter().collect();
        assert_eq!(record.block_hash, block_hash);
        assert_eq!(record.height, height);
        assert_eq!(record.view, view);
        assert_eq!(record.epoch, epoch);
        assert_eq!(record.roster_len, roster_len);
        assert_eq!(record.signers, expected_signers);
        assert_eq!(record.bls_aggregate_signature, aggregate_signature);
    }

    #[test]
    fn commit_qc_from_history_falls_back_when_cache_missing() {
        let _guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        let chain: ChainId = "commit-qc-history-fallback"
            .parse()
            .expect("chain id parses");
        let block = sample_block(9, 0);
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let epoch = 0;
        let signers_bitmap = vec![0b0000_0111];
        let keypairs = checked_bls_keypairs(4);
        let validator_set: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let aggregate_signature = aggregate_signature_for_bitmap(
            &chain,
            super::super::PERMISSIONED_TAG,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers_bitmap,
            &keypairs,
        );
        let qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height,
            view,
            epoch,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: super::super::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: aggregate_signature.clone(),
            },
        };
        crate::sumeragi::status::record_commit_qc(qc.clone());

        let qc_cache = BTreeMap::new();
        let fetched = commit_qc_from_cache_or_history(
            &qc_cache,
            block_hash,
            height,
            view,
            epoch,
            super::super::PERMISSIONED_TAG,
            &validator_set,
        );

        assert_eq!(fetched, Some(qc));
        crate::sumeragi::status::reset_commit_certs_for_tests();
    }

    #[test]
    fn apply_cached_qcs_to_block_sync_update_uses_checkpoint_roster_to_recover_history_qc() {
        let _guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();

        let chain: ChainId = "apply-cached-qc-history-via-checkpoint"
            .parse()
            .expect("chain id parses");
        let block = sample_block(11, 2);
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let epoch = 0;
        let signers_bitmap = vec![0b0000_0111];
        let keypairs = checked_bls_keypairs(4);
        let validator_set: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let aggregate_signature = aggregate_signature_for_bitmap(
            &chain,
            super::super::PERMISSIONED_TAG,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers_bitmap,
            &keypairs,
        );
        let qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height,
            view,
            epoch,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: super::super::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: aggregate_signature.clone(),
            },
        };
        crate::sumeragi::status::record_commit_qc(qc.clone());

        let kura = Kura::blank_kura_for_testing();
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let (trusted, me_id) = trusted_self();
        let roster_cache = {
            let view = state.view();
            super::RosterValidationCache::from_world(view.world(), super::EPOCH_LENGTH_BLOCKS, None)
        };
        let mut update = block_sync_update_with_roster(
            &block,
            &state,
            kura.as_ref(),
            ConsensusMode::Permissioned,
            &trusted,
            &me_id,
            &roster_cache,
        );
        update.commit_qc = None;
        update.validator_checkpoint = Some(ValidatorSetCheckpoint::new(
            height,
            view,
            block_hash,
            qc.parent_state_root,
            qc.post_state_root,
            validator_set,
            signers_bitmap,
            aggregate_signature,
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        ));

        let qc_cache = BTreeMap::new();
        let vote_log = BTreeMap::new();
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &qc_cache,
            &vote_log,
            block_hash,
            height,
            view,
            epoch,
            &state,
            ConsensusMode::Permissioned,
        );

        assert_eq!(update.commit_qc, Some(qc));
        crate::sumeragi::status::reset_commit_certs_for_tests();
    }

    #[test]
    fn recover_qc_from_kura_block_falls_back_to_roster() {
        let _guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        let chain: ChainId = "recover-qc-from-kura".parse().expect("chain id parses");
        let kura = Kura::blank_kura_for_testing();
        let block = sample_block(1, 0);
        let block_hash = block.hash();
        kura.store_block(block.clone())
            .expect("block should be persisted in kura");

        let qc_header = crate::sumeragi::consensus::QcHeaderRef {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            height: block.header().height().get(),
            view: block.header().view_change_index(),
            epoch: 0,
        };
        let keypairs = checked_bls_keypairs(1);
        let validator_set: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let signers: BTreeSet<_> = [0_u32].into_iter().collect();
        let signers_bitmap = vec![0b0000_0001];
        let aggregate_signature = aggregate_signature_for_bitmap(
            &chain,
            super::super::PERMISSIONED_TAG,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            qc_header.height,
            qc_header.view,
            qc_header.epoch,
            &signers_bitmap,
            &keypairs,
        );
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        crate::sumeragi::status::record_precommit_signers(
            crate::sumeragi::status::PrecommitSignerRecord {
                block_hash,
                height: qc_header.height,
                view: qc_header.view,
                epoch: qc_header.epoch,
                chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
                rechain_seq: 0,
                parent_state_root: zero_root,
                post_state_root: zero_root,
                signers,
                bls_aggregate_signature: aggregate_signature,
                roster_len: keypairs.len(),
                mode_tag: super::super::PERMISSIONED_TAG.to_string(),
                validator_set,
                stake_snapshot: None,
            },
        );
        let recovered = Actor::recover_qc_from_kura_block(&qc_header, kura.as_ref())
            .expect("fallback should yield QC");

        assert_eq!(recovered.height, qc_header.height);
        assert_eq!(recovered.view, qc_header.view);
        assert_eq!(recovered.subject_block_hash, qc_header.subject_block_hash);
        assert_eq!(recovered.aggregate.signers_bitmap, signers_bitmap);
        crate::sumeragi::status::reset_commit_certs_for_tests();
    }

    #[test]
    fn cached_votes_attach_to_block_sync_updates() {
        let block = sample_block(4, 0);
        let block_hash = block.hash();
        let kura = Kura::blank_kura_for_testing();
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );

        let mut vote_log: BTreeMap<votes::VoteLogKey, crate::sumeragi::consensus::Vote> =
            BTreeMap::new();
        let vote = crate::sumeragi::consensus::Vote {
            phase: crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 4,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: vec![0u8; 96],
        };
        vote_log.insert(
            (
                vote.phase,
                vote.height,
                vote.view,
                vote.epoch,
                vote.signer,
                vote.chain_order_hash,
                vote.rechain_seq,
            ),
            vote,
        );

        let (trusted, me_id) = trusted_self();
        let roster_cache = {
            let view = state.view();
            super::RosterValidationCache::from_world(view.world(), super::EPOCH_LENGTH_BLOCKS, None)
        };
        let mut update = block_sync_update_with_roster(
            &block,
            &state,
            kura.as_ref(),
            ConsensusMode::Permissioned,
            &trusted,
            &me_id,
            &roster_cache,
        );
        let qc_cache: BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc> = BTreeMap::new();

        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &qc_cache,
            &vote_log,
            block_hash,
            4,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );

        assert_eq!(update.commit_votes.len(), 1);
        assert_eq!(update.commit_votes[0].signer, 0);
    }

    #[test]
    fn apply_cached_qcs_to_block_sync_update_formal_gate_matrix() {
        let _guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_precommit_signer_history_for_tests();

        let chain: ChainId = "apply-cached-qcs-formal-gate"
            .parse()
            .expect("chain id parses");
        let kura = Kura::blank_kura_for_testing();
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let keypairs = checked_bls_keypairs(3);
        let validator_set: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let alternate_keypairs = checked_bls_keypairs(3);
        let alternate_validator_set: Vec<_> = alternate_keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let signers_bitmap = vec![0b0000_0111];
        let zero_roots = (
            Hash::prehashed([0u8; Hash::LENGTH]),
            Hash::prehashed([1u8; Hash::LENGTH]),
        );
        let make_qc = |block_hash: HashOf<BlockHeader>,
                       height: u64,
                       view: u64,
                       epoch: u64,
                       mode_tag: &str,
                       roots: (Hash, Hash),
                       roster: &[PeerId],
                       keys: &[KeyPair]| {
            let validator_set = roster.to_vec();
            crate::sumeragi::consensus::Qc {
                phase: crate::sumeragi::consensus::Phase::Commit,
                subject_block_hash: block_hash,
                parent_state_root: roots.0,
                post_state_root: roots.1,
                height,
                view,
                epoch,
                chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
                rechain_seq: 0,
                mode_tag: mode_tag.to_string(),
                highest_qc: None,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set,
                aggregate: crate::sumeragi::consensus::QcAggregate {
                    signers_bitmap: signers_bitmap.clone(),
                    bls_aggregate_signature: aggregate_signature_for_bitmap(
                        &chain,
                        mode_tag,
                        crate::sumeragi::consensus::Phase::Commit,
                        block_hash,
                        height,
                        view,
                        epoch,
                        &signers_bitmap,
                        keys,
                    ),
                },
            }
        };
        let make_checkpoint = |qc: &crate::sumeragi::consensus::Qc| {
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
        };
        let make_vote =
            |block_hash: HashOf<BlockHeader>, height: u64, view: u64, epoch: u64, signer: u32| {
                crate::sumeragi::consensus::Vote {
                    phase: crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    parent_state_root: zero_roots.0,
                    post_state_root: zero_roots.1,
                    height,
                    view,
                    epoch,
                    chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
                    rechain_seq: 0,
                    highest_qc: None,
                    signer,
                    bls_sig: vec![u8::try_from(signer).expect("test signer fits u8"); 96],
                }
            };
        let insert_vote =
            |vote_log: &mut BTreeMap<votes::VoteLogKey, crate::sumeragi::consensus::Vote>,
             vote: crate::sumeragi::consensus::Vote| {
                vote_log.insert(
                    (
                        vote.phase,
                        vote.height,
                        vote.view,
                        vote.epoch,
                        vote.signer,
                        vote.chain_order_hash,
                        vote.rechain_seq,
                    ),
                    vote,
                );
            };
        let raw_cache = |qc: crate::sumeragi::consensus::Qc| {
            let mut cache = BTreeMap::new();
            cache.insert(
                (
                    qc.phase,
                    qc.subject_block_hash,
                    qc.height,
                    qc.view,
                    qc.epoch,
                    qc.chain_order_hash,
                    qc.rechain_seq,
                ),
                qc,
            );
            cache
        };
        let empty_votes = BTreeMap::new();

        let block = sample_block(12, 1);
        let block_hash = block.hash();
        let existing_qc = make_qc(
            block_hash,
            12,
            1,
            0,
            super::super::PERMISSIONED_TAG,
            zero_roots,
            &validator_set,
            &keypairs,
        );
        let raw_qc = make_qc(
            block_hash,
            12,
            1,
            0,
            super::super::PERMISSIONED_TAG,
            (
                Hash::prehashed([2u8; Hash::LENGTH]),
                Hash::prehashed([3u8; Hash::LENGTH]),
            ),
            &alternate_validator_set,
            &alternate_keypairs,
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        update.commit_qc = Some(existing_qc.clone());
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &raw_cache(raw_qc),
            &empty_votes,
            block_hash,
            12,
            1,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert_eq!(
            update.commit_qc,
            Some(existing_qc.clone()),
            "existing commit QC must not be overwritten by cached evidence"
        );
        assert_eq!(
            update
                .validator_checkpoint
                .as_ref()
                .expect("checkpoint synthesized from existing QC")
                .validator_set,
            existing_qc.validator_set
        );

        let block = sample_block(13, 2);
        let block_hash = block.hash();
        let raw_qc = make_qc(
            block_hash,
            13,
            2,
            0,
            super::super::PERMISSIONED_TAG,
            zero_roots,
            &validator_set,
            &keypairs,
        );
        let existing_checkpoint = ValidatorSetCheckpoint::new(
            99,
            7,
            block_hash,
            Hash::prehashed([4u8; Hash::LENGTH]),
            Hash::prehashed([5u8; Hash::LENGTH]),
            alternate_validator_set.clone(),
            signers_bitmap.clone(),
            vec![9; 96],
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        update.validator_checkpoint = Some(existing_checkpoint.clone());
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &raw_cache(raw_qc.clone()),
            &empty_votes,
            block_hash,
            13,
            2,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert_eq!(update.commit_qc, Some(raw_qc));
        assert_eq!(
            update.validator_checkpoint,
            Some(existing_checkpoint),
            "existing validator checkpoint must not be overwritten"
        );

        let block = sample_block(14, 0);
        let block_hash = block.hash();
        let raw_qc = make_qc(
            block_hash,
            14,
            0,
            0,
            super::super::PERMISSIONED_TAG,
            zero_roots,
            &validator_set,
            &keypairs,
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &raw_cache(raw_qc.clone()),
            &empty_votes,
            block_hash,
            14,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert_eq!(update.commit_qc, Some(raw_qc.clone()));
        assert_eq!(
            update.validator_checkpoint,
            Some(make_checkpoint(&raw_qc)),
            "missing checkpoint should be synthesized from the final QC"
        );

        let block = sample_block(15, 0);
        let block_hash = block.hash();
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &BTreeMap::new(),
            &empty_votes,
            block_hash,
            15,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert!(update.commit_qc.is_none());
        assert!(
            update.validator_checkpoint.is_none(),
            "helper must not synthesize a checkpoint without a QC"
        );
        assert!(update.commit_votes.is_empty());

        let block = sample_block(16, 0);
        let block_hash = block.hash();
        let npos_qc = make_qc(
            block_hash,
            16,
            0,
            0,
            super::super::NPOS_TAG,
            zero_roots,
            &validator_set,
            &keypairs,
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &raw_cache(npos_qc.clone()),
            &empty_votes,
            block_hash,
            16,
            0,
            0,
            &state,
            ConsensusMode::Npos,
        );
        assert_eq!(update.commit_qc, Some(npos_qc.clone()));
        assert!(
            update
                .stake_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.matches_roster(&validator_set)),
            "NPoS update with commit QC should repair a missing stake snapshot"
        );
        let repaired_snapshot = update
            .stake_snapshot
            .clone()
            .expect("NPoS stake snapshot repaired");

        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        update.stake_snapshot = Some(repaired_snapshot.clone());
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &raw_cache(npos_qc.clone()),
            &empty_votes,
            block_hash,
            16,
            0,
            0,
            &state,
            ConsensusMode::Npos,
        );
        assert_eq!(
            update.stake_snapshot,
            Some(repaired_snapshot.clone()),
            "matching NPoS stake snapshot should be preserved"
        );

        let mismatched_snapshot = crate::sumeragi::stake_snapshot::CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&alternate_validator_set),
            entries: alternate_validator_set
                .iter()
                .cloned()
                .map(
                    |peer_id| crate::sumeragi::stake_snapshot::CommitStakeSnapshotEntry {
                        peer_id,
                        stake: Numeric::from(1_u64),
                    },
                )
                .collect(),
        };
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        update.stake_snapshot = Some(mismatched_snapshot.clone());
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &raw_cache(npos_qc.clone()),
            &empty_votes,
            block_hash,
            16,
            0,
            0,
            &state,
            ConsensusMode::Npos,
        );
        assert_ne!(
            update.stake_snapshot,
            Some(mismatched_snapshot.clone()),
            "mismatched NPoS stake snapshot should be repaired"
        );
        assert!(
            update
                .stake_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.matches_roster(&validator_set))
        );

        let permissioned_qc = make_qc(
            block_hash,
            16,
            0,
            0,
            super::super::PERMISSIONED_TAG,
            zero_roots,
            &validator_set,
            &keypairs,
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        update.stake_snapshot = Some(mismatched_snapshot.clone());
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &raw_cache(permissioned_qc),
            &empty_votes,
            block_hash,
            16,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert_eq!(
            update.stake_snapshot,
            Some(mismatched_snapshot),
            "permissioned updates should not repair stake snapshots"
        );

        let block = sample_block(17, 1);
        let block_hash = block.hash();
        let record_stake_snapshot =
            crate::sumeragi::stake_snapshot::CommitStakeSnapshot::from_roster(
                state.view().world(),
                &validator_set,
            )
            .expect("record stake snapshot");
        let signers: BTreeSet<_> = [0_u32, 1_u32, 2_u32].into_iter().collect();
        let aggregate_signature = aggregate_signature_for_bitmap(
            &chain,
            super::super::NPOS_TAG,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            17,
            1,
            0,
            &signers_bitmap,
            &keypairs,
        );
        crate::sumeragi::status::record_precommit_signers(
            crate::sumeragi::status::PrecommitSignerRecord {
                block_hash,
                height: 17,
                view: 1,
                epoch: 0,
                chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
                rechain_seq: 0,
                parent_state_root: zero_roots.0,
                post_state_root: zero_roots.1,
                signers,
                bls_aggregate_signature: aggregate_signature,
                roster_len: validator_set.len(),
                mode_tag: super::super::NPOS_TAG.to_string(),
                validator_set: validator_set.clone(),
                stake_snapshot: Some(record_stake_snapshot.clone()),
            },
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &BTreeMap::new(),
            &empty_votes,
            block_hash,
            17,
            1,
            0,
            &state,
            ConsensusMode::Npos,
        );
        assert!(update.commit_qc.is_some());
        assert_eq!(
            update.stake_snapshot,
            Some(record_stake_snapshot),
            "NPoS signer-history derivation should clone the recorded stake snapshot"
        );
        crate::sumeragi::status::reset_precommit_signer_history_for_tests();

        let block = sample_block(18, 0);
        let block_hash = block.hash();
        let mut vote_log = BTreeMap::new();
        let matching_vote = make_vote(block_hash, 18, 0, 0, 0);
        insert_vote(&mut vote_log, matching_vote.clone());
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &BTreeMap::new(),
            &vote_log,
            block_hash,
            18,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert_eq!(
            update.commit_votes,
            vec![matching_vote],
            "matching cached commit votes should attach when update votes are empty"
        );

        let existing_vote = make_vote(block_hash, 18, 0, 0, 1);
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        update.commit_votes = vec![existing_vote.clone()];
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &BTreeMap::new(),
            &vote_log,
            block_hash,
            18,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert_eq!(
            update.commit_votes,
            vec![existing_vote],
            "existing commit votes must not be overwritten by cached votes"
        );

        let mut wrong_context_votes = BTreeMap::new();
        insert_vote(&mut wrong_context_votes, make_vote(block_hash, 19, 0, 0, 0));
        insert_vote(&mut wrong_context_votes, make_vote(block_hash, 18, 1, 0, 1));
        insert_vote(&mut wrong_context_votes, make_vote(block_hash, 18, 0, 1, 2));
        let other_block_hash = sample_block(20, 0).hash();
        insert_vote(
            &mut wrong_context_votes,
            make_vote(other_block_hash, 18, 0, 0, 3),
        );
        let mut update = super::super::message::BlockSyncUpdate::from(&block);
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &BTreeMap::new(),
            &wrong_context_votes,
            block_hash,
            18,
            0,
            0,
            &state,
            ConsensusMode::Permissioned,
        );
        assert!(
            update.commit_votes.is_empty(),
            "wrong block/height/view cached votes should be ignored"
        );

        crate::sumeragi::status::reset_commit_certs_for_tests();
        crate::sumeragi::status::reset_precommit_signer_history_for_tests();
    }

    #[test]
    fn rbc_payload_bundle_builds_init_and_chunks() {
        let block = sample_block(5, 0);
        let block_hash = block.hash();
        let payload_hash = Hash::prehashed([0x11; 32]);
        let chunk_root = Hash::prehashed([0x22; 32]);
        let mut session = RbcSession::test_new(2, Some(payload_hash), Some(chunk_root), 0);
        session.test_set_block_header_and_signature(&block);
        session.test_note_chunk(0, vec![1, 2, 3], 0);
        session.test_note_chunk(1, vec![4, 5], 0);
        let roster = vec![checked_peer()];
        let roster_hash = super::rbc::rbc_roster_hash(&roster);

        let (init, chunks) = super::super::Actor::rbc_payload_bundle_from_cached_parts(
            (block_hash, 5, 0),
            &session,
            &roster,
        )
        .expect("bundle");

        assert_eq!(init.block_hash, block_hash);
        assert_eq!(init.total_chunks, 2);
        assert_eq!(init.chunk_root, chunk_root);
        assert_eq!(init.roster, roster);
        assert_eq!(init.roster_hash, roster_hash);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].idx, 0);
        assert_eq!(chunks[0].bytes, vec![1, 2, 3]);
        assert_eq!(chunks[1].idx, 1);
        assert_eq!(chunks[1].bytes, vec![4, 5]);
    }

    #[test]
    fn rbc_payload_bundle_allows_empty_chunks() {
        let block = sample_block(7, 0);
        let block_hash = block.hash();
        let payload_hash = Hash::prehashed([0x55; 32]);
        let chunk_digests = vec![[0x11; 32], [0x22; 32]];
        let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(chunk_digests.clone())
            .root()
            .map(Hash::from)
            .expect("chunk root");
        let mut session = RbcSession::new(
            2,
            Some(payload_hash),
            Some(chunk_root),
            Some(chunk_digests.clone()),
            0,
        )
        .expect("session");
        session.test_set_block_header_and_signature(&block);
        let roster = vec![checked_peer()];

        let (init, chunks) = super::super::Actor::rbc_payload_bundle_from_cached_parts(
            (block_hash, 7, 0),
            &session,
            &roster,
        )
        .expect("bundle");

        assert_eq!(init.total_chunks, 2);
        assert_eq!(init.chunk_root, chunk_root);
        assert_eq!(init.chunk_digests, chunk_digests);
        assert!(
            chunks.is_empty(),
            "missing cached chunks should still emit INIT"
        );
    }

    #[test]
    fn rbc_ready_bundle_clones_all_readies() {
        let block = sample_block(6, 0);
        let block_hash = block.hash();
        let payload_hash = Hash::prehashed([0x33; 32]);
        let chunk_root = Hash::prehashed([0x44; 32]);
        let mut session = RbcSession::test_new(1, Some(payload_hash), Some(chunk_root), 0);
        session.record_ready(0, vec![9, 9, 9]);
        session.record_ready(2, vec![7, 8]);
        let roster = vec![checked_peer()];
        let roster_hash = super::rbc::rbc_roster_hash(&roster);

        let readies =
            super::super::Actor::rbc_ready_bundle((block_hash, 6, 0), &session, roster_hash)
                .expect("ready set");
        let senders: BTreeSet<_> = readies.iter().map(|ready| ready.sender).collect();

        assert_eq!(senders, BTreeSet::from([0, 2]));
        assert!(readies.iter().all(|ready| ready.chunk_root == chunk_root));
        assert!(readies.iter().all(|ready| ready.roster_hash == roster_hash));
    }
}
