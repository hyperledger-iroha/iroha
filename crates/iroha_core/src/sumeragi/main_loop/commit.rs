//! Commit/finalization pipeline helpers.

use std::{
    collections::BTreeSet,
    sync::{Arc, mpsc},
    time::Instant,
};

use iroha_crypto::blake2::{Blake2b512, Digest as BlakeDigest};
use iroha_data_model::Encode as _;
use iroha_logger::prelude::*;

use super::locked_qc::qc_extends_locked_with_lookup;
use super::pacing::{Pacemaker, PacemakerBackpressure, PacemakerBackpressureAction};
use super::propose::ProposalBackpressure;
use super::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum EpochRefreshPhase {
    PreCommit,
    PostCommit,
}

const COMMIT_WORK_QUEUE_CAP: usize = 1;
const COMMIT_RESULT_QUEUE_CAP: usize = 1;

#[derive(Debug)]
pub(super) struct CommitWork {
    pub(super) id: u64,
    pub(super) block: SignedBlock,
    pub(super) commit_topology: Vec<PeerId>,
    pub(super) signature_topology: Vec<PeerId>,
    pub(super) qc_signers: Option<BTreeSet<ValidatorIndex>>,
    pub(super) allow_quorum_bypass: bool,
    pub(super) persist_required: bool,
    pub(super) events_sender: crate::EventsSender,
}

#[derive(Debug)]
pub(super) struct CommitResult {
    pub(super) id: u64,
    pub(super) outcome: CommitOutcome,
}

#[derive(Debug)]
pub(super) enum CommitOutcome {
    Rejected {
        failed_block: SignedBlock,
        error: BlockValidationError,
        pipeline_events: Vec<PipelineEventBox>,
    },
    KuraStoreFailed {
        committed_block: crate::block::CommittedBlock,
        error: crate::kura::Error,
    },
    StateCommitFailed {
        committed_block: crate::block::CommittedBlock,
        error: String,
    },
    Success {
        committed_block: crate::block::CommittedBlock,
        exec_witness: Option<ExecWitness>,
        pipeline_events: Vec<PipelineEventBox>,
        state_events: Vec<EventBox>,
    },
}

#[derive(Debug)]
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

pub(super) fn spawn_commit_worker(
    state: Arc<State>,
    kura: Arc<Kura>,
    chain_id: ChainId,
    genesis_account: AccountId,
    wake_tx: Option<mpsc::SyncSender<()>>,
) -> CommitWorkerHandle {
    let (work_tx, work_rx) = mpsc::sync_channel::<CommitWork>(COMMIT_WORK_QUEUE_CAP);
    let (result_tx, result_rx) = mpsc::sync_channel::<CommitResult>(COMMIT_RESULT_QUEUE_CAP);
    let join_handle = std::thread::Builder::new()
        .name("sumeragi-commit".to_owned())
        .spawn(move || {
            while let Ok(work) = work_rx.recv() {
                let id = work.id;
                let outcome = execute_commit_work(
                    state.as_ref(),
                    kura.as_ref(),
                    &chain_id,
                    &genesis_account,
                    work,
                );
                if result_tx.send(CommitResult { id, outcome }).is_err() {
                    break;
                }
                if let Some(wake) = wake_tx.as_ref() {
                    let _ = wake.try_send(());
                }
            }
        })
        .expect("failed to spawn sumeragi commit worker thread");

    CommitWorkerHandle {
        work_tx,
        result_rx,
        join_handle,
    }
}

pub(super) fn execute_commit_work(
    state: &State,
    kura: &Kura,
    chain_id: &ChainId,
    genesis_account: &AccountId,
    work: CommitWork,
) -> CommitOutcome {
    let CommitWork {
        block,
        commit_topology,
        signature_topology,
        qc_signers: _qc_signers,
        allow_quorum_bypass: _allow_quorum_bypass,
        persist_required,
        events_sender,
        ..
    } = work;
    let mut pipeline_events: Vec<PipelineEventBox> = Vec::new();
    let time_source = TimeSource::new_system();
    let mut voting_block = None;
    let topology = super::network_topology::Topology::new(signature_topology);
    let result = ValidBlock::validate_keep_voting_block_with_events(
        block,
        &topology,
        chain_id,
        genesis_account,
        &time_source,
        state,
        &mut voting_block,
        false,
        |event| pipeline_events.push(event),
    )
    .unpack(|event| pipeline_events.push(event))
    .and_then(|(valid_block, state_block)| {
        let commit_result = valid_block.commit_with_certificate();
        commit_result
            .unpack(|event| pipeline_events.push(event))
            .map(|committed_block| (committed_block, state_block))
            .map_err(|(failed_block, err)| (Box::new((*failed_block).into()), err))
    });

    match result {
        Ok((committed_block, mut state_block)) => {
            let exec_witness = state_block.take_exec_witness();
            if persist_required {
                if let Err(err) = kura.store_block(committed_block.clone()) {
                    return CommitOutcome::KuraStoreFailed {
                        committed_block,
                        error: err,
                    };
                }
            }
            // Emit pipeline events as soon as the commit is durable so clients can observe
            // `Committed` without waiting for WSV application.
            let mut pipeline_events = pipeline_events;
            emit_pipeline_events(&events_sender, std::mem::take(&mut pipeline_events));
            let state_events =
                state_block.apply_without_execution(&committed_block, commit_topology);
            if let Err(err) = state_block.commit() {
                return CommitOutcome::StateCommitFailed {
                    committed_block,
                    error: err.to_string(),
                };
            }
            CommitOutcome::Success {
                committed_block,
                exec_witness,
                pipeline_events,
                state_events,
            }
        }
        Err((failed_block, err)) => CommitOutcome::Rejected {
            failed_block: *failed_block,
            error: *err,
            pipeline_events,
        },
    }
}

fn has_commit_quorum_signers(
    qc_signers: Option<&BTreeSet<ValidatorIndex>>,
    min_votes_for_commit: usize,
) -> bool {
    qc_signers.is_some_and(|signers| signers.len() >= min_votes_for_commit)
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
    let key = (
        crate::sumeragi::consensus::Phase::Commit,
        block_hash,
        height,
        view,
        epoch,
    );
    if let Some(qc) = qc_cache.get(&key) {
        return Some(qc.clone());
    }
    super::status::commit_qc_history().into_iter().find(|qc| {
        qc.phase == crate::sumeragi::consensus::Phase::Commit
            && qc.subject_block_hash == block_hash
            && qc.height == height
            && qc.view == view
            && qc.epoch == epoch
            && qc.mode_tag == mode_tag
            && qc.validator_set.as_slice() == commit_topology
            && !qc.aggregate.bls_aggregate_signature.is_empty()
    })
}

impl Actor {
    /// Attach cached commit certificates and votes for the given block to a `BlockSyncUpdate`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_cached_qcs_to_block_sync_update(
        update: &mut super::message::BlockSyncUpdate,
        qc_cache: &BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
        vote_log: &BTreeMap<
            (
                crate::sumeragi::consensus::Phase,
                u64,
                u64,
                u64,
                crate::sumeragi::consensus::ValidatorIndex,
            ),
            crate::sumeragi::consensus::Vote,
        >,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        state: &State,
        fallback_consensus_mode: ConsensusMode,
    ) {
        let consensus_mode = {
            let view = state.view();
            super::effective_consensus_mode_for_height(&view, height, fallback_consensus_mode)
        };
        update.commit_qc = cached_qc_for(
            qc_cache,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
        );
        if update.commit_qc.is_none() {
            if let Some(record) = crate::sumeragi::status::precommit_signers_for(block_hash) {
                if record.height == height && record.view == view && record.epoch == epoch {
                    if let Some(derived) = super::derive_block_sync_qc_from_signers(
                        block_hash,
                        height,
                        view,
                        record.epoch,
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
                    update.stake_snapshot =
                        CommitStakeSnapshot::from_roster(state.view().world(), roster);
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

    fn drain_commit_results(&mut self) -> bool {
        let mut progress = false;
        while let Some(recv_result) = self
            .subsystems
            .commit
            .result_rx
            .as_ref()
            .map(mpsc::Receiver::try_recv)
        {
            match recv_result {
                Ok(result) => {
                    let inflight = match self.subsystems.commit.inflight.take() {
                        Some(inflight) if inflight.id == result.id => inflight,
                        Some(inflight) => {
                            warn!(
                                result_id = result.id,
                                inflight_id = inflight.id,
                                inflight_hash = %inflight.block_hash,
                                "commit result id mismatch; ignoring"
                            );
                            self.subsystems.commit.inflight = Some(inflight);
                            continue;
                        }
                        None => {
                            warn!(
                                result_id = result.id,
                                "commit result received without inflight; ignoring"
                            );
                            continue;
                        }
                    };
                    let _ = self.apply_commit_outcome(inflight, result.outcome);
                    progress = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("commit worker result channel closed; falling back to inline commit");
                    self.subsystems.commit.result_rx = None;
                    self.subsystems.commit.work_tx = None;
                    if let Some(inflight) = self.subsystems.commit.inflight.take() {
                        let persist_required = !inflight.pending.kura_persisted;
                        let work = CommitWork {
                            id: inflight.id,
                            block: inflight.pending.block.clone(),
                            commit_topology: inflight.commit_topology.clone(),
                            signature_topology: inflight.signature_topology.clone(),
                            qc_signers: inflight.qc_signers.clone(),
                            allow_quorum_bypass: inflight.allow_quorum_bypass,
                            persist_required,
                            events_sender: self.events_sender.clone(),
                        };
                        let outcome = execute_commit_work(
                            self.state.as_ref(),
                            self.kura.as_ref(),
                            &self.common_config.chain,
                            &self.genesis_account,
                            work,
                        );
                        let _ = self.apply_commit_outcome(inflight, outcome);
                        progress = true;
                    }
                    break;
                }
            }
        }
        progress
    }

    fn start_commit_job(&mut self, inflight: CommitInFlight, work: CommitWork) -> bool {
        let pending_height = inflight.pending.height;
        let pending_view = inflight.pending.view;
        let block_hash = inflight.block_hash;
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

        if let Some(work_tx) = self.subsystems.commit.work_tx.as_ref() {
            match work_tx.try_send(work) {
                Ok(()) => {
                    super::status::record_commit_inflight_start(
                        inflight.id,
                        pending_height,
                        pending_view,
                        block_hash,
                    );
                    self.subsystems.commit.inflight = Some(inflight);
                    return false;
                }
                Err(mpsc::TrySendError::Full(_work)) => {
                    warn!(
                        height = pending_height,
                        view = pending_view,
                        block = %block_hash,
                        "commit worker queue full; deferring finalize"
                    );
                    self.pending
                        .pending_blocks
                        .insert(block_hash, inflight.pending);
                    return false;
                }
                Err(mpsc::TrySendError::Disconnected(work)) => {
                    warn!(
                        height = pending_height,
                        view = pending_view,
                        block = %block_hash,
                        "commit worker channel closed; running commit inline"
                    );
                    self.subsystems.commit.work_tx = None;
                    self.subsystems.commit.result_rx = None;
                    let outcome = execute_commit_work(
                        self.state.as_ref(),
                        self.kura.as_ref(),
                        &self.common_config.chain,
                        &self.genesis_account,
                        work,
                    );
                    return self.apply_commit_outcome(inflight, outcome);
                }
            }
        }

        let outcome = execute_commit_work(
            self.state.as_ref(),
            self.kura.as_ref(),
            &self.common_config.chain,
            &self.genesis_account,
            work,
        );
        self.apply_commit_outcome(inflight, outcome)
    }

    #[allow(clippy::too_many_lines)]
    fn apply_commit_outcome(&mut self, inflight: CommitInFlight, outcome: CommitOutcome) -> bool {
        super::status::record_commit_inflight_finish(inflight.id);
        let CommitInFlight {
            lock,
            block_hash,
            pending,
            commit_topology,
            signature_topology,
            qc_signers,
            allow_quorum_bypass,
            post_commit_qc,
            ..
        } = inflight;
        let pending_height = pending.height;
        let pending_view = pending.view;
        let now = Instant::now();
        let da_enabled = self.runtime_da_enabled();
        let mut block_hash_to_clean = None;
        let mut exec_witness_to_emit: Option<ExecWitness> = None;
        let mut parent_to_cleanup: Option<HashOf<BlockHeader>> = None;
        let mut reschedule_quorum: Option<(PendingBlock, Duration, usize, usize, Duration)> = None;
        let mut committed = false;

        let topology = super::network_topology::Topology::new(signature_topology);
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

        match outcome {
            CommitOutcome::Success {
                committed_block,
                exec_witness,
                pipeline_events,
                state_events,
            } => {
                let mut pending = pending_opt.take().expect("pending present");
                self.note_view_change_from_block(pending_height, pending_view);
                crate::sumeragi::status::record_kura_stage(
                    pending_height,
                    pending_view,
                    block_hash,
                );
                let persist_required = !pending.kura_persisted;
                pending.mark_kura_persisted();
                let qc_key = (
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    pending_height,
                    pending_view,
                    lock.epoch,
                );
                let (consensus_mode, mode_tag, _) =
                    self.consensus_context_for_height(pending_height);
                let mut cached_qc = commit_qc_from_cache_or_history(
                    &self.qc_cache,
                    block_hash,
                    pending_height,
                    pending_view,
                    lock.epoch,
                    mode_tag,
                    &commit_topology,
                );
                if let Some(qc) = cached_qc.as_ref() {
                    self.qc_cache.entry(qc_key).or_insert_with(|| qc.clone());
                }
                if !allow_quorum_bypass && cached_qc.is_none() {
                    if let (Some(signers), Some(view_signers)) =
                        (qc_signers.as_ref(), view_signers.as_ref())
                    {
                        let aggregate_signature = match super::aggregate_vote_signatures(
                            &self.vote_log,
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
                            ConsensusMode::Npos => CommitStakeSnapshot::from_roster(
                                self.state.view().world(),
                                topology.as_ref(),
                            ),
                        };
                        if let Some((parent_state_root, post_state_root)) =
                            pending.parent_state_root.zip(pending.post_state_root)
                        {
                            if let Some(derived_qc) = super::derive_block_sync_qc_from_signers(
                                block_hash,
                                pending_height,
                                pending_view,
                                lock.epoch,
                                parent_state_root,
                                post_state_root,
                                topology.as_ref(),
                                consensus_mode,
                                stake_snapshot.as_ref(),
                                mode_tag,
                                signers,
                                aggregate_signature,
                            ) {
                                self.qc_cache.insert(qc_key, derived_qc.clone());
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
                exec_witness_to_emit = exec_witness;
                emit_pipeline_events(&self.events_sender, pipeline_events);
                for event in state_events {
                    if let Err(err) = self.events_sender.send(event) {
                        debug!(?err, "failed to send pipeline event");
                    }
                }

                let params_snapshot = {
                    let view = self.state.view();
                    let params = view.world().parameters();
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
                if let Some(qc) = cached_qc.as_ref() {
                    super::status::record_commit_qc(qc.clone());
                }
                if let Some(signers) = qc_signers.as_ref() {
                    let aggregate_signature = cached_qc.as_ref().map_or_else(
                        || {
                            view_signers
                                .as_ref()
                                .and_then(|view_signers| {
                                    super::aggregate_vote_signatures(
                                        &self.vote_log,
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
                        let roots = cached_qc
                            .as_ref()
                            .map(|qc| (qc.parent_state_root, qc.post_state_root))
                            .or_else(|| pending.parent_state_root.zip(pending.post_state_root));
                        if let Some((parent_state_root, post_state_root)) = roots {
                            let stake_snapshot = match consensus_mode {
                                ConsensusMode::Permissioned => None,
                                ConsensusMode::Npos => CommitStakeSnapshot::from_roster(
                                    self.state.view().world(),
                                    &commit_topology,
                                ),
                            };
                            crate::sumeragi::status::record_precommit_signers(
                                crate::sumeragi::status::PrecommitSignerRecord {
                                    block_hash,
                                    height: pending_height,
                                    view: pending_view,
                                    epoch: lock.epoch,
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
                } else if let Some(qc) = cached_qc.as_ref() {
                    let stake_snapshot = match consensus_mode {
                        ConsensusMode::Permissioned => None,
                        ConsensusMode::Npos => CommitStakeSnapshot::from_roster(
                            self.state.view().world(),
                            &commit_topology,
                        ),
                    };
                    if let Some(record) = Self::precommit_signer_record_from_cached_qc(
                        qc,
                        &commit_topology,
                        consensus_mode,
                        stake_snapshot,
                    ) {
                        crate::sumeragi::status::record_precommit_signers(record);
                    }
                }
                self.persist_roster_sidecar_for_commit(committed_block.as_ref(), &commit_topology);
                let tally = qc_signers.as_ref().map_or_else(
                    || {
                        crate::block::valid::commit_signature_tally(
                            committed_block.as_ref(),
                            &topology,
                        )
                    },
                    |signers| crate::block::valid::SignatureTally {
                        present: signers.len(),
                        counted: signers.len(),
                        set_b_signatures: 0,
                    },
                );
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
                    persisted = persist_required,
                    "stored committed block to kura"
                );
                #[cfg(feature = "telemetry")]
                {
                    self.telemetry
                        .report_block_commit_blocking(&committed_block.as_ref().header());
                }
                // Proactively gossip the committed block so peers that missed the
                // QC or payload can synchronize without waiting for block sync
                // backoff windows.
                let sync_block: SignedBlock = committed_block.as_ref().clone();
                let mut sync_update = block_sync_update_with_roster(
                    &sync_block,
                    &self.state,
                    &self.kura,
                    self.config.consensus_mode,
                    self.common_config.trusted_peers.value(),
                    self.common_config.peer.id(),
                    &self.roster_validation_cache,
                );
                let expected_epoch = self.epoch_for_height(pending_height);
                Self::apply_cached_qcs_to_block_sync_update(
                    &mut sync_update,
                    &self.qc_cache,
                    &self.vote_log,
                    block_hash,
                    pending_height,
                    pending_view,
                    expected_epoch,
                    self.state.as_ref(),
                    self.config.consensus_mode,
                );
                let world_peers = {
                    let view = self.state.view();
                    view.world.peers().iter().cloned().collect::<Vec<_>>()
                };
                let (consensus_mode, _, _) = self.consensus_context_for_height(pending_height);
                if self.prepare_block_sync_update_for_broadcast(&mut sync_update, consensus_mode) {
                    self.broadcast_block_sync_update(sync_update, &world_peers);
                } else {
                    self.broadcast_block_created_for_block_sync(
                        super::message::BlockCreated::from(&sync_block),
                        &world_peers,
                    );
                }
                parent_to_cleanup = pending.block.header().prev_block_hash();
                block_hash_to_clean = Some(block_hash);
                trace!(
                    height = pending_height,
                    view = pending_view,
                    block = ?block_hash,
                    "Committed block (DA availability advisory)"
                );
                committed = true;
            }
            CommitOutcome::KuraStoreFailed {
                committed_block,
                error,
            } => {
                let pending = pending_opt.take().expect("pending present");
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
                    "failed to enqueue committed block to kura; keeping block pending"
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
                    self.config.kura_store_retry_interval,
                    self.config.kura_store_retry_max_attempts,
                    self.queue.as_ref(),
                    self.state.as_ref(),
                    self.telemetry_handle(),
                );
                if let Some(pending) = failure.pending {
                    self.pending.pending_blocks.insert(block_hash, pending);
                }
                if failure.clean_block_hash {
                    self.qc_cache
                        .retain(|(_, hash, _, _, _), _| hash != &block_hash);
                    self.qc_signer_tally
                        .retain(|(_, hash, _, _, _), _| hash != &block_hash);
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
            } => {
                let mut pending = pending_opt.take().expect("pending present");
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
                pending.mark_kura_persisted();
                pending.block = committed_block.into();
                self.pending.pending_blocks.insert(block_hash, pending);
            }
            CommitOutcome::Rejected {
                failed_block,
                error,
                pipeline_events,
            } => {
                let mut pending = pending_opt.take().expect("pending present");
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
                let pending_age = pending.progress_age(now);
                let vote_count = sig_indices.len();
                let quorum_timeout = self.quorum_timeout(da_enabled);
                let availability_timeout = self.availability_timeout(quorum_timeout, da_enabled);
                let missing_local_data =
                    matches!(pending.last_gate, Some(GateReason::MissingLocalData));
                let quorum_reached = has_quorum_signers || vote_count >= min_votes_for_commit;

                if commit_signatures_missing
                    && !has_quorum_signers
                    && missing_quorum_stale(pending_age, quorum_timeout, quorum_reached)
                {
                    let reschedule_backoff = quorum_timeout.max(QUORUM_RESCHEDULE_COOLDOWN);
                    if missing_local_data && pending_age < availability_timeout {
                        debug!(
                            height = pending_height,
                            view = pending_view,
                            block = ?block_hash,
                            pending_age_ms = pending_age.as_millis(),
                            availability_timeout_ms = availability_timeout.as_millis(),
                            "deferring quorum reschedule while awaiting local payload"
                        );
                        pending.block = failed_block;
                        self.pending.pending_blocks.insert(block_hash, pending);
                    } else if self.rbc_availability_unresolved_for_reschedule(
                        (block_hash, pending_height, pending_view),
                        &topology,
                    ) {
                        debug!(
                            height = pending_height,
                            view = pending_view,
                            block = ?block_hash,
                            pending_age_ms = pending_age.as_millis(),
                            quorum_timeout_ms = quorum_timeout.as_millis(),
                            "deferring quorum reschedule while RBC availability is unresolved"
                        );
                        pending.block = failed_block;
                        self.pending.pending_blocks.insert(block_hash, pending);
                    } else {
                        let queue_depths = super::status::worker_queue_depth_snapshot();
                        if queue_depths.vote_rx > 0 {
                            debug!(
                                height = pending_height,
                                view = pending_view,
                                block = ?block_hash,
                                pending_age_ms = pending_age.as_millis(),
                                quorum_timeout_ms = quorum_timeout.as_millis(),
                                vote_rx_depth = queue_depths.vote_rx,
                                "deferring quorum reschedule while vote queue is backlogged"
                            );
                            pending.block = failed_block;
                            self.pending.pending_blocks.insert(block_hash, pending);
                        } else if pending.reschedule_due(now, reschedule_backoff) {
                            reschedule_quorum = Some((
                                pending,
                                pending_age,
                                min_votes_for_commit,
                                vote_count,
                                quorum_timeout,
                            ));
                        } else {
                            pending.block = failed_block;
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
                            failed_block.transactions_vec().clone(),
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
                            .retain(|(_, hash, _, _, _), _| hash != &block_hash);
                        self.qc_signer_tally
                            .retain(|(_, hash, _, _, _), _| hash != &block_hash);
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
                                .retain(|(_, hash, _, _, _), _| hash != &block_hash);
                            self.qc_signer_tally
                                .retain(|(_, hash, _, _, _), _| hash != &block_hash);
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
                        pending.block = failed_block;
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
        if let Some((pending, age, min_votes, vote_count, quorum_timeout)) = reschedule_quorum {
            self.reschedule_pending_quorum_block(
                pending,
                age,
                min_votes,
                vote_count,
                quorum_timeout,
                quorum_timeout.max(QUORUM_RESCHEDULE_COOLDOWN),
                Instant::now(),
            );
            self.trigger_view_change_with_cause(
                pending_height,
                pending_view,
                view_change_cause_for_quorum(vote_count, false),
            );
        }
        if committed {
            // Commit finished; drop any RBC state for the committed block to avoid replaying READY/DELIVER.
            self.clean_rbc_sessions_for_block(block_hash, pending_height);
            if let Some(witness) = exec_witness_to_emit {
                self.emit_exec_artifacts(block_hash, pending_height, pending_view, witness);
            }
            self.finalize_collector_plan(true);
            let new_highest = match self.highest_qc {
                Some(current) if (current.height, current.view) > (lock.height, lock.view) => {
                    current
                }
                _ => lock,
            };
            self.highest_qc = Some(new_highest);
            super::status::set_highest_qc(new_highest.height, new_highest.view);
            super::status::set_highest_qc_hash(new_highest.subject_block_hash);
            let previous_lock = self.locked_qc;
            let new_locked = match self.locked_qc {
                Some(current) if (current.height, current.view) >= (lock.height, lock.view) => {
                    current
                }
                _ => lock,
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
                && !qc_extends_locked_with_lookup(lock, highest, |hash, height| {
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

            let backpressure = self.proposal_backpressure_at(Instant::now());
            let _ =
                kickstart_pacemaker_after_commit(self.queue.queued_len(), backpressure, |now| {
                    self.on_pacemaker_propose_ready(now)
                });

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
                self.clean_rbc_sessions_for_block(stale_hash, stale_height);
                self.qc_cache
                    .retain(|(_, hash, _, _, _), _| hash != &stale_hash);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _), _| hash != &stale_hash);
            }
            self.qc_cache.retain(|(_, hash, height, _, _), _| {
                *hash == block_hash || *height > pending_height
            });
            self.qc_signer_tally.retain(|(_, hash, height, _, _), _| {
                *hash == block_hash || *height > pending_height
            });
            self.subsystems
                .propose
                .proposal_cache
                .prune_height_leq(pending_height);
            if let Some(parent) = parent_to_cleanup {
                self.qc_cache
                    .retain(|(_, hash, _, _, _), _| hash != &parent);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _), _| hash != &parent);
            }
            let retention_floor = pending_height.saturating_sub(1);
            self.vote_log
                .retain(|(_, height, _, _, _), _| *height >= retention_floor);
            self.try_replay_deferred_votes();

            if let Some(child_qc) = post_commit_qc {
                let previous_lock = self.locked_qc;
                let promoted_lock = match self.locked_qc {
                    Some(current)
                        if (current.height, current.view) >= (child_qc.height, child_qc.view) =>
                    {
                        current
                    }
                    _ => child_qc,
                };
                self.locked_qc = Some(promoted_lock);
                super::status::set_locked_qc(
                    promoted_lock.height,
                    promoted_lock.view,
                    Some(promoted_lock.subject_block_hash),
                );
                if previous_lock != Some(promoted_lock) {
                    self.prune_precommit_votes_conflicting_with_lock(promoted_lock);
                }
                let new_highest = match self.highest_qc {
                    Some(current)
                        if (current.height, current.view) >= (child_qc.height, child_qc.view) =>
                    {
                        current
                    }
                    _ => child_qc,
                };
                self.highest_qc = Some(new_highest);
                super::status::set_highest_qc(new_highest.height, new_highest.view);
                super::status::set_highest_qc_hash(new_highest.subject_block_hash);
            }
        }
        committed
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
        let (state_height, state_tip_hash) = {
            let view = self.state.view();
            (view.height(), view.latest_block_hash())
        };
        if pending.aborted {
            let pending_parent = pending.block.header().prev_block_hash();
            if pending.commit_qc_seen
                && super::pending_extends_tip(
                    pending_height,
                    pending_parent,
                    state_height,
                    state_tip_hash,
                )
            {
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
            self.clean_rbc_sessions_for_block(block_hash, pending_height);
            if let Some(parent) = pending.block.header().prev_block_hash() {
                self.qc_cache
                    .retain(|(_, hash, _, _, _), _| hash != &parent);
                self.qc_signer_tally
                    .retain(|(_, hash, _, _, _), _| hash != &parent);
            }
            return true;
        }
        if !pending.commit_qc_seen {
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
                "block already persisted in kura; retrying state commit without re-enqueue"
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
                "DA availability missing; finalize continues"
            );
        }
        let (consensus_mode, _, _) = self.consensus_context_for_height(pending_height);
        let commit_topology = self.roster_for_vote_with_mode(
            block_hash,
            pending_height,
            pending_view,
            consensus_mode,
        );
        iroha_logger::info!(
            commit_topology_len = commit_topology.len(),
            commit_topology = ?commit_topology,
            "finalizing pending block with commit topology"
        );
        if commit_topology.is_empty() {
            warn!(
                height = pending_height,
                view = pending_view,
                block = %block_hash,
                "deferring finalize: empty commit roster"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
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
        let qc_key = (
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            pending_height,
            pending_view,
            lock.epoch,
        );
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
        let allow_quorum_bypass = false;

        iroha_logger::info!(
            height = pending_height,
            view = pending_view,
            block = %block_hash,
            mode = ?self.consensus_mode,
            "committing with commit certificate"
        );

        let id = self.subsystems.commit.next_id();
        let persist_required = !pending.kura_persisted;
        let work = CommitWork {
            id,
            block: pending.block.clone(),
            commit_topology: commit_topology.clone(),
            signature_topology: signature_topology.clone(),
            qc_signers: quorum_signers.clone(),
            allow_quorum_bypass,
            persist_required,
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
            allow_quorum_bypass,
            post_commit_qc,
            enqueue_time: now,
        };
        self.start_commit_job(inflight, work)
    }
}

impl Actor {
    pub(super) fn process_commit_candidates(&mut self) {
        self.process_commit_candidates_with_trigger(CommitPipelineTrigger::Event);
    }

    pub(in crate::sumeragi) fn poll_commit_results(&mut self) -> bool {
        self.drain_commit_results()
    }

    fn abort_inflight_commit_if_timed_out(&mut self, now: Instant) -> bool {
        let timeout = self.config.commit_inflight_timeout;
        if timeout.is_zero() {
            return false;
        }
        let Some(inflight) = self.subsystems.commit.inflight.as_ref() else {
            return false;
        };
        let elapsed = now.saturating_duration_since(inflight.enqueue_time);
        if elapsed < timeout {
            return false;
        }
        let inflight = self
            .subsystems
            .commit
            .inflight
            .take()
            .expect("inflight present for timeout");
        let mut pending = inflight.pending;
        let height = pending.height;
        let view = pending.view;
        let block_hash = inflight.block_hash;
        let txs = pending.block.transactions_vec().clone();
        let (requeued, failures, duplicate_failures, _) =
            requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
        if failures > 0 {
            warn!(
                height,
                view,
                failures,
                requeued,
                duplicate_failures,
                "failed to requeue some transactions after inflight commit timeout"
            );
        }
        pending.mark_aborted();
        pending.tx_batch = None;
        self.clean_rbc_sessions_for_block(block_hash, height);
        self.qc_cache
            .retain(|(_, hash, _, _, _), _| hash != &block_hash);
        self.qc_signer_tally
            .retain(|(_, hash, _, _, _), _| hash != &block_hash);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        // Keep proposals_seen so we don't re-propose in the same view after timeout.
        let session_key = Self::session_key(&block_hash, height, view);
        self.subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .remove(&session_key);
        self.subsystems
            .da_rbc
            .rbc
            .ready_rebroadcast_last_sent
            .remove(&session_key);
        self.subsystems
            .da_rbc
            .rbc
            .deliver_deferral
            .remove(&session_key);
        // Retain the aborted payload so late commit QCs can still recover via block sync.
        self.pending.pending_blocks.insert(block_hash, pending);
        super::status::record_commit_inflight_timeout(height, view, block_hash, elapsed);
        super::status::record_commit_inflight_finish(inflight.id);
        self.trigger_view_change_after_commit_failure(height, view);
        warn!(
            height,
            view,
            block = %block_hash,
            elapsed_ms = elapsed.as_millis(),
            timeout_ms = timeout.as_millis(),
            "aborting inflight commit after timeout"
        );
        true
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_validation_reject(
        &mut self,
        invalid_hash: HashOf<BlockHeader>,
        invalid_height: u64,
        invalid_view: u64,
        evidence: Option<Box<crate::sumeragi::consensus::Evidence>>,
        reason: String,
        reason_label: &'static str,
    ) {
        if let Some(pending) = self.pending.pending_blocks.remove(&invalid_hash) {
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
    ) {
        let _ = self.drain_commit_results();
        let now = Instant::now();
        let _ = self.abort_inflight_commit_if_timed_out(now);

        if matches!(trigger, CommitPipelineTrigger::Event) {
            let _ = self.reschedule_stale_pending_blocks();
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
        let rebroadcast_cooldown = self.rebroadcast_cooldown();
        let local_peer_id = self.common_config.peer.id().clone();

        if self.active_pending_blocks_len() == 0 {
            let inflight = self.subsystems.commit.inflight.is_some();
            if matches!(trigger, CommitPipelineTrigger::Tick) {
                super::status::note_commit_pipeline_tick(self.consensus_mode, inflight);
                #[cfg(feature = "telemetry")]
                self.telemetry
                    .note_commit_pipeline_tick(self.mode_tag(), inflight);
            }
            return;
        }

        if matches!(trigger, CommitPipelineTrigger::Tick) {
            super::status::note_commit_pipeline_tick(self.consensus_mode, true);
            #[cfg(feature = "telemetry")]
            self.telemetry
                .note_commit_pipeline_tick(self.mode_tag(), true);
        }

        let block_time = {
            let view = self.state.view();
            self.block_time_for_mode(&view, self.consensus_mode)
        };
        let qc_rebuild_cooldown = block_time.max(REBROADCAST_COOLDOWN_FLOOR);
        self.pending.last_commit_pipeline_run = self.pending.last_commit_pipeline_run.max(now);
        let should_rebuild_qcs =
            now.saturating_duration_since(self.last_qc_rebuild) >= qc_rebuild_cooldown;
        if enable_qc_pipeline && should_rebuild_qcs {
            self.last_qc_rebuild = now;
            let active_commit_topology = self.effective_commit_topology();
            self.rebuild_qcs_from_cached_votes(&active_commit_topology);
        }

        let mut pending_hashes: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .map(|(hash, pending)| (pending.height, pending.view, *hash))
            .collect();
        pending_hashes
            .sort_by(|(h1, v1, hash1), (h2, v2, hash2)| (h1, v1, hash1).cmp(&(h2, v2, hash2)));
        for (pending_height, pending_view, hash) in pending_hashes {
            let block_start = Instant::now();
            let validation_start = Instant::now();
            let (consensus_mode, _, _) = self.consensus_context_for_height(pending_height);
            let commit_topology =
                self.roster_for_vote_with_mode(hash, pending_height, pending_view, consensus_mode);
            if commit_topology.is_empty() {
                warn!(
                    height = pending_height,
                    view = pending_view,
                    block = %hash,
                    "skipping pending block: empty commit roster"
                );
                continue;
            }
            let validation_outcome = self.validate_pending_block_for_voting(hash, &commit_topology);
            let validation_cost = validation_start.elapsed();
            match validation_outcome {
                ValidationGateOutcome::Valid => {}
                ValidationGateOutcome::Deferred => continue,
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
                    let payload_available = da_enabled
                        && Self::payload_available_for_da(
                            &self.subsystems.da_rbc.rbc.sessions,
                            &self.subsystems.da_rbc.rbc.status_handle,
                            snapshot,
                        );
                    (snapshot.aborted, payload_available)
                }
                None => continue,
            };
            let kura_has_block = self.kura.get_block_height_by_hash(hash).is_some();
            let (state_height, state_tip_hash) = {
                let view = self.state.view();
                (view.height(), view.latest_block_hash())
            };
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
                    self.clean_rbc_sessions_for_block(hash, pending.height);
                }
                continue;
            }
            if aborted {
                debug!(
                    ?hash,
                    height = pending_height,
                    view = pending_view,
                    "skipping aborted pending block"
                );
                continue;
            }
            let topology = super::network_topology::Topology::new(commit_topology.clone());
            let roster_len = topology.as_ref().len();
            let min_votes_for_commit = self.commit_min_votes(&topology);
            let missing_local_data = da_enabled && !payload_available;
            let delivered = payload_available;
            let mut emit_precommit = false;
            let mut abort_due_to_kura = false;
            let mut replay_msg: Option<BlockMessage> = None;
            let mut replay_rbc_init: Option<crate::sumeragi::consensus::RbcInit> = None;
            let gate_start = Instant::now();
            let mut pending = match self.pending.pending_blocks.remove(&hash) {
                Some(pending) => pending,
                None => continue,
            };
            if !pending.commit_qc_seen {
                if let Some(qc) = qc_cache_for_subject(&self.qc_cache, hash).find(|qc| {
                    matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit)
                        && qc.height == pending_height
                        && qc.view == pending_view
                }) {
                    pending.commit_qc_seen = true;
                    pending.commit_qc_epoch = Some(qc.epoch);
                }
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
            let commit_epoch = pending.commit_qc_epoch.unwrap_or(vote_epoch);
            let ready_to_finalize = pending.commit_qc_seen && kura_ready;
            if pending.kura_aborted {
                warn!(
                    ?hash,
                    height = pending_height,
                    view = pending_view,
                    attempts = pending.kura_retry_attempts,
                    "kura persistence retries exhausted; aborting pending block"
                );
                abort_due_to_kura = true;
            } else if kura_ready {
                if enable_qc_pipeline && !pending.precommit_vote_sent && !pending.commit_qc_seen {
                    emit_precommit = true;
                }
            } else {
                debug!(
                    ?hash,
                    height = pending_height,
                    view = pending_view,
                    attempts = pending.kura_retry_attempts,
                    "deferring commit while awaiting kura retry window"
                );
            }

            let gate_cost = gate_start.elapsed();

            if let Some(msg) = replay_msg.take() {
                for peer in &commit_topology {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: msg.clone(),
                    });
                }
            }
            if let Some(init) = replay_rbc_init.take() {
                let msg = BlockMessage::RbcInit(init);
                for peer in &commit_topology {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: msg.clone(),
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
                    );
                    if rebroadcasted > 0 {
                        pending.mark_precommit_rebroadcast(now);
                        iroha_logger::info!(
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
                    .retain(|(_, qc_hash, _, _, _), _| qc_hash != &hash);
                self.qc_signer_tally
                    .retain(|(_, qc_hash, _, _, _), _| qc_hash != &hash);
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
            if enable_qc_pipeline && emit_precommit {
                let parent_hash = pending.block.header().prev_block_hash();
                let pending_roots = pending.parent_state_root.zip(pending.post_state_root);
                if self.emit_precommit_vote(
                    hash,
                    pending_height,
                    pending_view,
                    vote_epoch,
                    &topology,
                    parent_hash,
                    pending_roots,
                ) {
                    pending.precommit_vote_sent = true;
                    if let Some(vote) = self.local_precommit_vote_for(
                        pending_height,
                        pending_view,
                        vote_epoch,
                        &topology,
                    ) {
                        self.maybe_broadcast_block_sync_update_for_precommit_vote(
                            &mut pending,
                            &vote,
                        );
                    }
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
                continue;
            }
            let finalize_cost = finalize_start.elapsed();
            self.pending.pending_blocks.insert(hash, pending);

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
        }
    }

    pub(super) fn local_precommit_vote_for(
        &self,
        height: u64,
        view: u64,
        epoch: u64,
        topology: &super::network_topology::Topology,
    ) -> Option<crate::sumeragi::consensus::Vote> {
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology = topology_for_view(topology, height, view, mode_tag, prf_seed);
        let local_idx = self.local_validator_index_for_topology(&signature_topology)?;
        let key = (
            crate::sumeragi::consensus::Phase::Commit,
            height,
            view,
            epoch,
            local_idx,
        );
        self.vote_log.get(&key).cloned()
    }

    pub(super) fn maybe_broadcast_block_sync_update_for_precommit_vote(
        &mut self,
        pending: &mut PendingBlock,
        vote: &crate::sumeragi::consensus::Vote,
    ) {
        if pending.aborted {
            debug!(
                height = vote.height,
                view = vote.view,
                block = %vote.block_hash,
                "skipping block sync update for aborted pending block"
            );
            return;
        }
        let cooldown = self
            .config
            .npos
            .block_time
            .max(std::time::Duration::from_millis(200));
        let now = std::time::Instant::now();
        if self
            .block_sync_rebroadcast_log
            .allow(vote.block_hash, now, cooldown)
        {
            let mut update = self.block_sync_update_for_precommit_vote(
                &pending.block,
                self.state.as_ref(),
                self.kura.as_ref(),
                &self.qc_cache,
                &self.vote_log,
                vote,
            );
            let commit_votes = update.commit_votes.len();
            let has_commit_qc = update.commit_qc.is_some();
            if !pending.should_broadcast_block_sync_update(vote.view, commit_votes, has_commit_qc) {
                iroha_logger::trace!(
                    height = vote.height,
                    view = vote.view,
                    block = %vote.block_hash,
                    commit_votes,
                    has_commit_qc,
                    "skipping block sync update broadcast: no new commit votes"
                );
                return;
            }
            let (consensus_mode, _, _) = self.consensus_context_for_height(vote.height);
            let mut topology_peers = self.roster_for_vote_with_mode(
                vote.block_hash,
                vote.height,
                vote.view,
                consensus_mode,
            );
            if topology_peers.is_empty() {
                topology_peers = self.effective_commit_topology();
            }
            if topology_peers.is_empty() {
                return;
            }
            if self.prepare_block_sync_update_for_broadcast(&mut update, consensus_mode) {
                self.broadcast_block_sync_update(update, &topology_peers);
                iroha_logger::info!(
                    height = vote.height,
                    view = vote.view,
                    block = %vote.block_hash,
                    signer = vote.signer,
                    targets = topology_peers.len(),
                    "sending block sync update to commit topology after emitting local precommit vote"
                );
            } else {
                self.broadcast_block_created_for_block_sync(
                    super::message::BlockCreated::from(&pending.block),
                    &topology_peers,
                );
                iroha_logger::info!(
                    height = vote.height,
                    view = vote.view,
                    block = %vote.block_hash,
                    signer = vote.signer,
                    targets = topology_peers.len(),
                    "sending BlockCreated payload to commit topology (no verifiable roster yet)"
                );
            }
        } else {
            iroha_logger::trace!(
                height = vote.height,
                view = vote.view,
                block = %vote.block_hash,
                signer = vote.signer,
                cooldown_ms = cooldown.as_millis(),
                "skipping block sync update broadcast due to cooldown"
            );
        }
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
        let mut vote = crate::sumeragi::consensus::Vote {
            phase,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            highest_qc,
            signer,
            bls_sig: Vec::new(),
        };
        let (_, mode_tag, _) = self.consensus_context_for_height(height);
        let preimage = vote_preimage(&self.common_config.chain, mode_tag, &vote);
        let signature = Signature::new(self.common_config.key_pair.private_key(), &preimage);
        vote.bls_sig = signature.payload().to_vec();
        Some(vote)
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_precommit_vote(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        topology: &super::network_topology::Topology,
        parent_hash: Option<HashOf<BlockHeader>>,
        pending_roots: Option<(Hash, Hash)>,
    ) -> bool {
        if self.is_observer() {
            return false;
        }
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology = topology_for_view(topology, height, view, mode_tag, prf_seed);
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
        let sent_key = (
            crate::sumeragi::consensus::Phase::Commit,
            height,
            view,
            epoch,
            local_idx,
        );
        if self.vote_log.contains_key(&sent_key) {
            debug!(
                height,
                view,
                block = ?block_hash,
                signer = local_idx,
                "skipping precommit: already voted for this round"
            );
            return false;
        }
        let local_peer = self.common_config.peer.id();
        let conflicting_vote = self.vote_log.values().find(|vote| {
            if vote.phase != crate::sumeragi::consensus::Phase::Commit {
                return false;
            }
            if vote.height != height || vote.epoch != epoch || vote.block_hash == block_hash {
                return false;
            }
            if vote.view < view {
                return false;
            }
            let signature_topology =
                topology_for_view(topology, vote.height, vote.view, mode_tag, prf_seed);
            let Ok(vote_idx) = usize::try_from(vote.signer) else {
                return false;
            };
            signature_topology
                .as_ref()
                .get(vote_idx)
                .is_some_and(|peer| peer == local_peer)
        });
        if let Some(conflict) = conflicting_vote {
            warn!(
                height,
                view,
                epoch,
                block = ?block_hash,
                previous_view = conflict.view,
                previous_block = ?conflict.block_hash,
                signer = local_idx,
                "skipping precommit: local validator already voted for a different block at this height"
            );
            return false;
        }
        if let Some(lock) = self.locked_qc {
            if !self.block_known_for_lock(lock.subject_block_hash) {
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
            let candidate = crate::sumeragi::consensus::QcHeaderRef {
                phase: crate::sumeragi::consensus::Phase::Commit,
                subject_block_hash: block_hash,
                height,
                view,
                epoch,
            };
            let extends_locked =
                qc_extends_locked_with_lookup(lock, candidate, |hash, lookup_height| {
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
            None,
            roots,
        ) else {
            return false;
        };
        self.handle_vote(vote.clone());

        let vote_msg = BlockMessage::QcVote(vote);
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
        let required = signature_topology.min_votes_for_commit();
        if collector_targets.len() < required {
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
                "sending precommit vote to commit topology (collector plan empty, local-only, or below quorum)"
            );
        } else {
            iroha_logger::info!(
                height,
                view,
                block = ?block_hash,
                signer = local_idx,
                targets = collector_targets.len(),
                "sending precommit vote to collectors"
            );
        }
        for peer in collector_targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: vote_msg.clone(),
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
        if self.is_observer() {
            return false;
        }
        let epoch = self.epoch_for_height(height);
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology = topology_for_view(topology, height, view, mode_tag, prf_seed);
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
        let sent_key = (
            crate::sumeragi::consensus::Phase::NewView,
            height,
            view,
            epoch,
            local_idx,
        );
        if let Some(existing) = self.vote_log.get(&sent_key) {
            if existing.block_hash != highest_qc.subject_block_hash {
                warn!(
                    height,
                    view,
                    signer = local_idx,
                    existing_hash = %existing.block_hash,
                    new_hash = %highest_qc.subject_block_hash,
                    "skipping NEW_VIEW vote: local validator already voted for a different subject"
                );
            }
            return false;
        }
        let Some(vote) = self.build_vote(
            crate::sumeragi::consensus::Phase::NewView,
            highest_qc.subject_block_hash,
            height,
            view,
            epoch,
            local_idx,
            Some(highest_qc),
            None,
        ) else {
            return false;
        };
        self.handle_vote(vote.clone());

        let vote_msg = BlockMessage::QcVote(vote);
        let local_peer_id = self.common_config.peer.id().clone();
        let leader = signature_topology.leader().clone();
        let (collectors_k, _) = self.collector_plan_params_for_mode(consensus_mode);
        let mut targets = if collectors_k == 0 {
            Vec::new()
        } else {
            super::collectors::deterministic_collectors(
                &signature_topology,
                consensus_mode,
                collectors_k,
                prf_seed,
                height,
                view,
            )
        };
        let required = signature_topology.min_votes_for_commit();
        let mut fallback_to_topology = false;
        if targets.is_empty() {
            fallback_to_topology = true;
            targets = signature_topology.as_ref().to_vec();
        }
        targets.retain(|peer| peer != &local_peer_id);
        if targets.len() < required {
            fallback_to_topology = true;
            targets = signature_topology.as_ref().to_vec();
            targets.retain(|peer| peer != &local_peer_id);
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
                "sending NEW_VIEW vote to commit topology (collector plan empty or below quorum)"
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
                msg: vote_msg.clone(),
            });
        }
        true
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
        if !signers.is_empty() {
            let (filtered, _groups) = super::qc::select_commit_root_signers(
                &self.vote_log,
                block_hash,
                height,
                view,
                epoch,
                &signers,
            );
            signers = filtered;
        }
        let vote_count = signers.len();
        let mut stake_result: Option<Result<bool, super::stake_snapshot::StakeQuorumError>> = None;
        let quorum_reached = match consensus_mode {
            ConsensusMode::Permissioned => vote_count >= signature_topology.min_votes_for_commit(),
            ConsensusMode::Npos => {
                let result = (|| {
                    let roster_set: BTreeSet<_> = commit_topology.iter().cloned().collect();
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
                    let view = self.state.view();
                    super::stake_snapshot::stake_quorum_reached_for_peers(
                        &view,
                        &commit_topology,
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
        let Some(pending) = self.pending.pending_blocks.remove(&block_hash) else {
            return;
        };
        let mut pending = pending;
        pending.commit_qc_seen = true;
        pending.commit_qc_epoch = Some(cert.epoch);
        let qc_header = crate::sumeragi::consensus::QcHeaderRef {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            height,
            view,
            epoch: cert.epoch,
        };
        let _ = self.finalize_pending_block(qc_header, pending, None);
    }

    pub(super) fn rebroadcast_block_votes(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> usize {
        if self.relay_backpressure_active(Instant::now(), self.rebroadcast_cooldown()) {
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
        let mut topology_peers =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
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
        let required = signature_topology.min_votes_for_commit();
        if collector_targets.len() < required {
            fallback_to_topology = true;
            collector_targets = signature_topology.as_ref().to_vec();
            collector_targets.retain(|peer| peer != &local_peer_id);
        }
        if fallback_to_topology {
            iroha_logger::info!(
                height,
                view,
                block = ?block_hash,
                phase = ?phase,
                targets = collector_targets.len(),
                "rebroadcasting votes to commit topology (collector plan empty, local-only, or below quorum)"
            );
        } else {
            iroha_logger::info!(
                height,
                view,
                block = ?block_hash,
                phase = ?phase,
                targets = collector_targets.len(),
                "rebroadcasting votes to collectors"
            );
        }

        let mut rebroadcasted = 0usize;
        for vote in votes {
            let msg = match phase {
                crate::sumeragi::consensus::Phase::Prepare
                | crate::sumeragi::consensus::Phase::Commit
                | crate::sumeragi::consensus::Phase::NewView => BlockMessage::QcVote(vote),
            };
            for peer in &collector_targets {
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: msg.clone(),
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
        let mut collector_targets = if collectors_k == 0 {
            Vec::new()
        } else {
            super::collectors::deterministic_collectors(
                &signature_topology,
                consensus_mode,
                collectors_k,
                prf_seed,
                height,
                view,
            )
        };
        if !collector_targets.is_empty() {
            let limit = usize::from(redundant_r.max(1));
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
        let required = signature_topology.min_votes_for_commit();
        if required > 0 && collector_targets.len() < required {
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
                "sending exec witness to commit topology (collector plan empty, local-only, or below quorum)"
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
        };
        if !crate::fastpq::lane::try_submit(fastpq_job) {
            debug!(
                height,
                view, "fastpq lane: witness queue full; dropping prover job"
            );
        }

        for peer in collector_targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: BlockMessage::ExecWitness(witness_msg.clone()),
            });
        }
    }

    /// Check whether an RBC session has delivered a payload matching `payload_hash`.
    /// Consults both in-memory sessions and the persisted status snapshot so restarts
    /// and multi-view deliveries update availability status deterministically.
    fn ensure_block_matches_rbc_payload(
        sessions: &BTreeMap<super::rbc_store::SessionKey, RbcSession>,
        handle: &rbc_status::Handle,
        block_hash: &HashOf<BlockHeader>,
        height: u64,
        payload_hash: &Hash,
    ) -> bool {
        rbc_payload_matches(sessions, handle, block_hash, height, payload_hash)
    }

    fn local_payload_matches_hash(block: &SignedBlock, payload_hash: &Hash) -> bool {
        let payload_bytes = super::proposals::block_payload_bytes(block);
        Hash::new(&payload_bytes) == *payload_hash
    }

    fn payload_available_for_da(
        sessions: &BTreeMap<super::rbc_store::SessionKey, RbcSession>,
        handle: &rbc_status::Handle,
        pending: &PendingBlock,
    ) -> bool {
        if Self::local_payload_matches_hash(&pending.block, &pending.payload_hash) {
            return true;
        }
        Self::ensure_block_matches_rbc_payload(
            sessions,
            handle,
            &pending.block.hash(),
            pending.height,
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
                    let reason = err.gate_reason();
                    let previous = pending.last_gate;
                    let changed = previous != Some(reason);
                    if changed {
                        super::status::record_da_gate_transition(previous, Some(reason));
                    }
                    pending.last_gate = Some(reason);
                    pending.last_gate_satisfied = None;
                    warn!(
                        ?err,
                        lane,
                        epoch,
                        sequence,
                        height = pending.height,
                        view = pending.view,
                        da_enabled,
                        "DA manifest unavailable or mismatched (advisory)"
                    );
                    return DaGateStatus {
                        reason: Some(reason),
                        satisfaction: None,
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
        let missing_local_data = da_enabled
            && !Self::payload_available_for_da(
                &self.subsystems.da_rbc.rbc.sessions,
                &self.subsystems.da_rbc.rbc.status_handle,
                pending,
            );
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
    pub(super) fn broadcast_block_sync_update(
        &mut self,
        update: super::message::BlockSyncUpdate,
        peers: &[PeerId],
    ) {
        let online_peers = self
            .network
            .online_peers(|set| set.iter().map(|peer| peer.id().clone()).collect::<Vec<_>>());
        let registered_peers = {
            let view = self.state.view();
            view.world().peers().iter().cloned().collect::<Vec<_>>()
        };
        let seed = update.block.hash();
        let targets = Self::block_sync_update_targets_for_peers(
            self.common_config.peer.id(),
            self.block_sync_gossip_limit,
            peers,
            &registered_peers,
            &online_peers,
            seed.as_ref(),
        );
        if targets.is_empty() {
            trace!(
                height = update.block.header().height().get(),
                view = update.block.header().view_change_index(),
                block = ?update.block.hash(),
                "skipping block sync update gossip: no targets"
            );
            return;
        }
        for peer in targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: BlockMessage::BlockSyncUpdate(update.clone()),
            });
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn broadcast_block_created_for_block_sync(
        &mut self,
        created: super::message::BlockCreated,
        peers: &[PeerId],
    ) {
        let online_peers = self
            .network
            .online_peers(|set| set.iter().map(|peer| peer.id().clone()).collect::<Vec<_>>());
        let registered_peers = {
            let view = self.state.view();
            view.world().peers().iter().cloned().collect::<Vec<_>>()
        };
        let seed = created.block.hash();
        let targets = Self::block_sync_update_targets_for_peers(
            self.common_config.peer.id(),
            self.block_sync_gossip_limit,
            peers,
            &registered_peers,
            &online_peers,
            seed.as_ref(),
        );
        if targets.is_empty() {
            trace!(
                height = created.block.header().height().get(),
                view = created.block.header().view_change_index(),
                block = ?created.block.hash(),
                "skipping block payload gossip: no targets"
            );
            return;
        }
        for peer in targets {
            self.schedule_background(BackgroundRequest::Post {
                peer,
                msg: BlockMessage::BlockCreated(created.clone()),
            });
        }
    }

    pub(super) fn block_sync_update_targets_for_peers(
        local_peer: &PeerId,
        gossip_limit: usize,
        peers: &[PeerId],
        registered_peers: &[PeerId],
        online_peers: &[PeerId],
        seed: &[u8],
    ) -> Vec<PeerId> {
        if gossip_limit == 0 || peers.is_empty() {
            return Vec::new();
        }

        let world_peers: BTreeSet<_> = peers.iter().cloned().collect();
        // Only target online peers that remain registered (e.g., observers), not unregistered strays.
        let registered: BTreeSet<_> = registered_peers.iter().cloned().collect();
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

    #[allow(dead_code)]
    #[allow(clippy::needless_pass_by_value)]
    fn broadcast_block_created(&mut self, created: super::message::BlockCreated, peers: &[PeerId]) {
        for peer in peers {
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessage::BlockCreated(created.clone()),
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
            let mut update = block_sync_update_with_roster(
                block.as_ref(),
                self.state.as_ref(),
                self.kura.as_ref(),
                self.config.consensus_mode,
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
                &self.roster_validation_cache,
            );
            let expected_epoch = qc.epoch;
            Self::apply_cached_qcs_to_block_sync_update(
                &mut update,
                &self.qc_cache,
                &self.vote_log,
                block_hash,
                block_height,
                qc.view,
                expected_epoch,
                self.state.as_ref(),
                self.config.consensus_mode,
            );
            let (consensus_mode, _, _) = self.consensus_context_for_height(block_height);
            debug!(
                height = block_height,
                view = qc.view,
                block = %block_hash,
                targets = topology_peers.len(),
                "rebroadcasting committed block for highest QC"
            );
            if self.prepare_block_sync_update_for_broadcast(&mut update, consensus_mode) {
                self.broadcast_block_sync_update(update, topology_peers);
            } else {
                self.broadcast_block_created_for_block_sync(
                    super::message::BlockCreated::from(block.as_ref()),
                    topology_peers,
                );
            }
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
            let created = super::message::BlockCreated::from(&pending.block);
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
        let cooldown = self
            .config
            .npos
            .timeouts
            .propose
            .max(Duration::from_millis(50));
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
        let key = (
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
        );
        if let Some(existing) = self.qc_cache.get(&key).cloned() {
            return Some(existing);
        }
        if topology_peers.is_empty() {
            if let Some(recovered) = self.recover_highest_qc_from_kura(&qc) {
                self.qc_cache.insert(key, recovered.clone());
                return Some(recovered);
            }
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
        self.try_form_qc_from_votes(
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
            &topology,
        );
        if let Some(formed) = self.qc_cache.get(&key).cloned() {
            return Some(formed);
        }
        if let Some(recovered) = self.recover_highest_qc_from_kura(&qc) {
            self.qc_cache.insert(key, recovered.clone());
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
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) && !signers.is_empty() {
            let (filtered, _groups) = super::qc::select_commit_root_signers(
                &self.vote_log,
                qc.subject_block_hash,
                qc.height,
                qc.view,
                qc.epoch,
                &signers,
            );
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
                let view = self.state.view();
                super::stake_snapshot::stake_quorum_reached_for_peers(
                    &view,
                    topology.as_ref(),
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
            &self.vote_log,
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
                let key = (qc.phase, qc.height, qc.view, qc.epoch, *signer);
                self.vote_log.get(&key).and_then(|vote| {
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
        let rebuilt = self.build_qc_from_signers(
            QcBuildContext {
                phase: qc.phase,
                block_hash: qc.subject_block_hash,
                height: qc.height,
                view: qc.view,
                epoch: qc.epoch,
                mode_tag: mode_tag.to_string(),
                highest_qc: None,
            },
            &canonical_signers,
            &topology,
            aggregate_signature,
            roots,
        );
        self.qc_cache.insert(key, rebuilt.clone());
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
        let record = crate::sumeragi::status::precommit_signers_for(block.hash())?;
        if record.height != qc.height || record.view != qc.view || record.epoch != qc.epoch {
            return None;
        }
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

        for (hash, height, view) in stale_pending {
            info!(
                height,
                view,
                block = %hash,
                committed_height,
                committed_hash = %committed_hash,
                "dropping pending block that diverges from committed tip"
            );
            if let Some((tx_count, requeued, failures, duplicate_failures)) =
                self.drop_stale_pending_block(hash, height, view)
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
        for (phase, hash, height, view, epoch) in self.qc_cache.keys() {
            let extends = chain_extends_tip(
                *hash,
                *height,
                committed_height,
                committed_hash,
                |head, h| self.parent_hash_for(head, h),
            );
            let drop_entry = *height < committed_height || matches!(extends, Some(false) | None);
            if drop_entry {
                stale_qcs.push((*phase, *hash, *height, *view, *epoch));
            }
        }
        for key in stale_qcs {
            let _ = self.qc_cache.remove(&key);
            let _ = self.qc_signer_tally.remove(&key);
        }
    }

    pub(super) fn clean_rbc_sessions_for_block(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
    ) {
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
            &self.subsystems.da_rbc.rbc.status_handle,
            chunk_store,
        );
        self.deferred_votes.remove(&block_hash);
        self.deferred_qcs
            .retain(|(_, hash, _, _, _), _| *hash != block_hash);
        let payload_keys: Vec<_> = self
            .subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .keys()
            .filter(|(hash, _, _)| *hash == block_hash)
            .copied()
            .collect();
        for key in payload_keys {
            self.subsystems
                .da_rbc
                .rbc
                .payload_rebroadcast_last_sent
                .remove(&key);
        }
        let ready_keys: Vec<_> = self
            .subsystems
            .da_rbc
            .rbc
            .ready_rebroadcast_last_sent
            .keys()
            .filter(|(hash, _, _)| *hash == block_hash)
            .copied()
            .collect();
        for key in ready_keys {
            self.subsystems
                .da_rbc
                .rbc
                .ready_rebroadcast_last_sent
                .remove(&key);
        }
        let deferral_keys: Vec<_> = self
            .subsystems
            .da_rbc
            .rbc
            .deliver_deferral
            .keys()
            .filter(|(hash, _, _)| *hash == block_hash)
            .copied()
            .collect();
        for key in deferral_keys {
            self.subsystems.da_rbc.rbc.ready_deferral.remove(&key);
            self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
        }
        self.subsystems
            .da_rbc
            .rbc
            .persisted_full_sessions
            .retain(|(hash, _, _)| *hash != block_hash);
        self.subsystems
            .da_rbc
            .rbc
            .persist_inflight
            .retain(|(hash, _, _)| *hash != block_hash);

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

    pub(super) fn refresh_npos_seed(
        &mut self,
        seed: [u8; 32],
        height: u64,
        phase: EpochRefreshPhase,
    ) {
        let (cfg, epoch_params, seed_for_height, epoch_schedule) = {
            let view = self.state.view();
            let cfg = if matches!(self.consensus_mode, ConsensusMode::Npos) {
                Some(
                    super::load_npos_collector_config(&view)
                        .or(self.npos_collectors)
                        .unwrap_or(NposCollectorConfig {
                            seed,
                            k: self.config.npos.k_aggregators,
                            redundant_send_r: self.config.npos.redundant_send_r,
                        }),
                )
            } else {
                None
            };
            let epoch_params = super::load_npos_epoch_params(&view, &self.config);
            let seed_for_height = super::prf_seed_for_height(&view, height);
            let epoch_schedule = super::EpochScheduleSnapshot::from_world_with_fallback(
                view.world(),
                epoch_params.epoch_length_blocks,
            );
            (cfg, epoch_params, seed_for_height, epoch_schedule)
        };
        let mut next_seed = seed;
        if let Some(manager) = self.epoch_manager.as_mut() {
            manager.set_params(
                epoch_params.epoch_length_blocks,
                epoch_params.commit_deadline_offset,
                epoch_params.reveal_deadline_offset,
            );
            if matches!(phase, EpochRefreshPhase::PostCommit) {
                let epoch_for_height = epoch_schedule.epoch_for_height(height);
                let expected_epoch = if epoch_schedule.is_epoch_boundary(height) {
                    epoch_for_height.saturating_add(1)
                } else {
                    epoch_for_height
                };
                if manager.epoch() != expected_epoch {
                    manager.reset_epoch_state(expected_epoch, seed_for_height);
                    self.subsystems.vrf.reset();
                    next_seed = seed_for_height;
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
        let committed_height = {
            let view = self.state.view();
            view.height() as u64
        };
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
        Ok(progress)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_block_commit(&mut self, height: u64) -> Result<()> {
        self.refresh_roster_validation_cache();
        self.subsystems.propose.new_view_tracker.prune(height);
        self.prune_proposals_seen_horizon(height);
        self.prune_vote_caches_horizon(height);
        self.subsystems.propose.forced_view_after_timeout = self
            .subsystems
            .propose
            .forced_view_after_timeout
            .filter(|(forced_height, _)| *forced_height > height);
        let _retention_floor = height.saturating_sub(1);
        self.pending
            .missing_block_requests
            .retain(|_, request| request.height > height);
        self.refresh_p2p_topology();
        let commit_topology = self.effective_commit_topology();
        match self.refresh_commit_topology_state(&commit_topology) {
            CommitTopologyChange::None => {}
            CommitTopologyChange::Membership => {
                self.reset_consensus_state_for_roster_change(false);
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
        let committed_block = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .and_then(|nz| self.kura.get_block(nz));
        if let Some(block) = committed_block.as_ref() {
            self.prune_descendants_not_on_tip(height, block.hash());
            self.note_view_change_from_block(height, block.header().view_change_index());
        }
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
        if let Some(block) = committed_block {
            let qc_header = crate::sumeragi::consensus::QcHeaderRef {
                phase: crate::sumeragi::consensus::Phase::Commit,
                subject_block_hash: block.hash(),
                height,
                view: block.header().view_change_index(),
                epoch: self.epoch_for_height(height),
            };
            if self
                .materialize_qc_for_header(qc_header, &commit_topology)
                .is_none()
            {
                debug!(
                    height,
                    view = qc_header.view,
                    block = %qc_header.subject_block_hash,
                    "unable to cache QC for committed block from kura"
                );
            }
        }
        if !matches!(
            self.consensus_mode,
            ConsensusMode::Permissioned | ConsensusMode::Npos
        ) {
            return Ok(());
        }
        let local_signer = {
            let view = self.state.view();
            let idx = self.local_validator_index(&view);
            drop(view);
            idx
        };
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

            self.persist_vrf_snapshot(snapshot, true, election_outcome.clone())?;
            if let Some(manager) = self.epoch_manager.as_ref() {
                let new_epoch = manager.epoch();
                let record_exists = {
                    let view = self.state.view();
                    view.world().vrf_epochs().get(&new_epoch).is_some()
                };
                if !record_exists {
                    let seed_snapshot = manager.snapshot_current_epoch(roster_len_hint, height);
                    self.persist_vrf_snapshot(seed_snapshot, false, None)?;
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

        self.apply_penalties(height)?;

        Ok(())
    }

    fn apply_penalties(&mut self, current_height: u64) -> Result<()> {
        if !matches!(self.consensus_mode, ConsensusMode::Npos) {
            return Ok(());
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
        let vrf = applier.apply_vrf_penalties(current_height);
        let evidence = applier.apply_consensus_penalties(current_height)?;
        super::status::inc_vrf_penalties_applied(vrf.applied);
        super::status::inc_consensus_penalties_applied(evidence.applied);
        super::status::set_penalties_pending(evidence.pending, vrf.pending);
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn run_validator_election(
        &self,
        epoch: u64,
        snapshot_height: u64,
        seed: [u8; 32],
        roster_len_hint: u32,
    ) -> Result<ValidatorElectionOutcome> {
        let (params, _candidates, profiles) = {
            let view = self.state.view();
            let params = super::resolve_npos_election_params(&view, &self.config.npos);
            let Some(epoch_roster) = view.epoch_validator_peer_ids(epoch) else {
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
            let profiles = self.collect_candidate_profiles(&view, &epoch_roster);
            (params, epoch_roster, profiles)
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
        view: &StateView<'_>,
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
        for ((_lane_id, validator_id), record) in view.world.public_lane_validators().iter() {
            if let Some(pk) = validator_id.try_signatory() {
                record_map
                    .entry(PeerId::from(pk.clone()))
                    .or_insert_with(|| record.clone());
            }
        }

        let mut share_map: BTreeMap<(LaneId, AccountId), Vec<PublicLaneStakeShare>> =
            BTreeMap::new();
        for ((lane_id, validator, _staker), share) in view.world.public_lane_stake_shares().iter() {
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
            self.config.epoch_length_blocks,
            Some(&self.common_config.trusted_peers.value().pops),
        );
        drop(world);
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
        self.pending.missing_block_requests.clear();
        self.pending.pending_processing.set(None);
        self.pending.pending_processing_parent.set(None);
        self.vote_log.clear();
        self.deferred_votes.clear();
        self.deferred_qcs.clear();
        self.vote_roster_cache.clear();
        self.qc_cache.clear();
        self.qc_signer_tally.clear();
        self.voting_block = None;
        if !preserve_proposals_seen {
            self.subsystems.propose.proposals_seen.clear();
        }
        self.subsystems.propose.proposal_cache = ProposalCache::new(PROPOSAL_CACHE_LIMIT);
        self.reset_collector_state();
        self.subsystems.da_rbc.rbc.pending.clear();
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
        self.subsystems.da_rbc.rbc.persisted_full_sessions.clear();
        self.subsystems.da_rbc.rbc.persist_inflight.clear();
        self.subsystems.da_rbc.rbc.status_handle.clear();
        self.subsystems.da_rbc.da.da_bundles.clear();
        self.subsystems.da_rbc.da.da_pin_bundles.clear();
        self.subsystems.da_rbc.da.sealed_commitments.clear();
        self.subsystems.da_rbc.da.sealed_pin_intents.clear();
        self.new_view_rebroadcast_log.clear();
        self.proposal_rebroadcast_log.clear();
        self.payload_rebroadcast_log.clear();
        self.block_sync_rebroadcast_log.clear();
        self.block_sync_fetch_log.clear();
    }

    #[cfg(test)]
    #[allow(dead_code)]
    /// Test-only wrapper around the commit hook.
    pub(super) fn on_block_commit_for_tests(&mut self, height: u64) -> Result<()> {
        self.on_block_commit(height)
    }

    pub(super) fn refresh_p2p_topology(&mut self) {
        let current: BTreeSet<_> = {
            let view = self.state.view();
            view.world.peers().iter().cloned().collect::<BTreeSet<_>>()
        };

        let local_peer = self.common_config.peer.id();
        let removed = !current.contains(local_peer) && !current.is_empty();
        crate::sumeragi::status::set_local_removed_from_world(removed);
        if removed {
            iroha_logger::warn!(
                current_len = current.len(),
                local = %local_peer,
                "local peer removed from world state; staying in block-sync follower mode"
            );
            self.queue.clear_all();
            // Keep existing connections so block sync can still fetch updates after removal.
            if current != self.last_advertised_topology {
                self.last_advertised_topology.clone_from(&current);
                self.peers_gossiper
                    .update_topology(UpdateTopology(current.into_iter().collect()));
            }
            return;
        }

        let stray_online: Vec<_> = self.network.online_peers(|online| {
            online
                .iter()
                .filter(|peer| !current.contains(peer.id()))
                .map(|peer| peer.id().clone())
                .collect()
        });

        let decision =
            topology_refresh_decision(&current, &self.last_advertised_topology, &stray_online);
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
                "p2p topology unchanged but network has peers outside world state; re-advertising"
            ),
            TopologyRefreshDecision::AdvertiseChanged => iroha_logger::info!(
                topology_len = current.len(),
                "advertising updated p2p topology from world state"
            ),
        }

        let mut advertise = current;
        if matches!(decision, TopologyRefreshDecision::AdvertiseForStrays { .. })
            && !stray_online.is_empty()
        {
            advertise.extend(stray_online.clone());
            iroha_logger::debug!(
                advertise_len = advertise.len(),
                stray_count = stray_online.len(),
                "keeping connections to peers outside world state to allow block sync"
            );
        }

        self.last_advertised_topology.clone_from(&advertise);
        self.peers_gossiper
            .update_topology(UpdateTopology(advertise.into_iter().collect()));
    }

    pub(super) fn refresh_backpressure_state(&mut self) -> bool {
        let refreshed = self.subsystems.propose.backpressure_gate.refresh();
        // Always publish the latest snapshot so operator status endpoints report
        // correct queue capacity even when the state has not changed.
        super::status::set_tx_queue_backpressure(self.subsystems.propose.backpressure_gate.state());
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
            if backpressure.only_queue_saturation() {
                if should_fire_now {
                    // Allow proposals to proceed once the pacemaker deadline elapses when queue
                    // saturation is the only backpressure signal, but keep logging deferral.
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
    use iroha_crypto::{Algorithm, Hash, KeyPair, MerkleTree, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId, Registrable,
        block::{BlockSignature, SignedBlock},
        peer::{Peer, PeerId},
        prelude::{Account, AccountId, Domain, EventBox, Level, Log, TransactionBuilder},
        transaction::SignedTransaction,
    };
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_primitives::{numeric::Numeric, unique_vec::UniqueVec};

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
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
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

    #[test]
    fn execute_commit_work_emits_pipeline_events_before_state_apply() {
        let genesis_key = KeyPair::random();
        let genesis_account_id =
            AccountId::new(GENESIS_DOMAIN_ID.clone(), genesis_key.public_key().clone());
        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let world = World::with([genesis_domain], [genesis_account], []);
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, Arc::clone(&kura), query_handle);
        let chain_id = state.view().chain_id().clone();

        let tx = TransactionBuilder::new(chain_id.clone(), genesis_account_id.clone())
            .with_instructions([Log::new(Level::DEBUG, "genesis commit test".to_string())])
            .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);

        let peer_key = KeyPair::random();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, mut events_rx) = tokio::sync::broadcast::channel(64);
        let work = CommitWork {
            id: 1,
            block,
            commit_topology: topology.clone(),
            signature_topology: topology,
            qc_signers: None,
            allow_quorum_bypass: false,
            persist_required: true,
            events_sender,
        };

        let outcome =
            execute_commit_work(&state, kura.as_ref(), &chain_id, &genesis_account_id, work);
        let CommitOutcome::Success {
            pipeline_events, ..
        } = outcome
        else {
            panic!("expected commit success");
        };
        assert!(
            pipeline_events.is_empty(),
            "pipeline events should be emitted early"
        );

        let mut got_pipeline_event = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match events_rx.try_recv() {
                Ok(event) => {
                    if matches!(event, EventBox::Pipeline(_)) {
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
        assert!(got_pipeline_event, "expected pipeline event emission");
    }

    #[test]
    fn commit_worker_wakes_on_result() {
        let genesis_key = KeyPair::random();
        let genesis_account_id =
            AccountId::new(GENESIS_DOMAIN_ID.clone(), genesis_key.public_key().clone());
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
        );

        let tx = TransactionBuilder::new(chain_id.clone(), genesis_account_id.clone())
            .with_instructions([Log::new(
                Level::DEBUG,
                "commit worker wake test".to_string(),
            )])
            .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let peer_key = KeyPair::random();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(16);
        let work = CommitWork {
            id: 42,
            block,
            commit_topology: topology.clone(),
            signature_topology: topology,
            qc_signers: None,
            allow_quorum_bypass: false,
            persist_required: false,
            events_sender,
        };

        handle.work_tx.send(work).expect("send commit work");
        handle
            .result_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("commit result");
        wake_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("wake signal");

        drop(handle.work_tx);
        if let Err(err) = handle.join_handle.join() {
            panic!("commit worker panicked: {err:?}");
        }
    }

    #[test]
    fn commit_worker_does_not_block_on_full_wake_channel() {
        let genesis_key = KeyPair::random();
        let genesis_account_id =
            AccountId::new(GENESIS_DOMAIN_ID.clone(), genesis_key.public_key().clone());
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
        );

        let tx = TransactionBuilder::new(chain_id.clone(), genesis_account_id.clone())
            .with_instructions([Log::new(
                Level::DEBUG,
                "commit worker full wake test".to_string(),
            )])
            .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let peer_key = KeyPair::random();
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let topology = vec![peer_id];
        let (events_sender, _events_rx) = tokio::sync::broadcast::channel(16);
        let work = CommitWork {
            id: 43,
            block,
            commit_topology: topology.clone(),
            signature_topology: topology,
            qc_signers: None,
            allow_quorum_bypass: false,
            persist_required: false,
            events_sender,
        };

        handle.work_tx.send(work).expect("send commit work");
        handle
            .result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("commit result");

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
    fn block_sync_update_targets_cap_and_excludes_local() {
        let local = PeerId::new(KeyPair::random().public_key().clone());
        let peers: Vec<_> = (0..6)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let mut online = Vec::new();
        online.push(local.clone());
        online.extend(peers.clone());
        let seed = [0xB1; 32];
        let targets =
            Actor::block_sync_update_targets_for_peers(&local, 3, &online, &online, &online, &seed);
        let repeat =
            Actor::block_sync_update_targets_for_peers(&local, 3, &online, &online, &online, &seed);

        assert_eq!(targets, repeat);
        assert_eq!(targets.len(), 3);
        assert!(!targets.contains(&local));
        assert!(targets.iter().all(|peer| online.contains(peer)));
    }

    #[test]
    fn block_sync_update_targets_prioritizes_strays() {
        let local = PeerId::new(KeyPair::random().public_key().clone());
        let world_peers: Vec<_> = (0..2)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let stray_peers: Vec<_> = (0..2)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
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
            &online,
            &seed,
        );

        assert_eq!(targets.len(), 2);
        assert!(!targets.contains(&local));
        assert!(targets.iter().all(|peer| stray_peers.contains(peer)));
    }

    #[test]
    fn block_sync_update_targets_skip_unregistered_strays() {
        let local = PeerId::new(KeyPair::random().public_key().clone());
        let world_peers: Vec<_> = (0..2)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let stray = PeerId::new(KeyPair::random().public_key().clone());
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
            &online,
            &seed,
        );

        assert!(!targets.contains(&stray));
    }

    #[test]
    fn block_sync_update_targets_for_peers_prefers_online_world() {
        let local = PeerId::new(KeyPair::random().public_key().clone());
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let peer_c = PeerId::new(KeyPair::random().public_key().clone());
        let peers = vec![local.clone(), peer_a, peer_b.clone(), peer_c];
        let online = vec![local.clone(), peer_b.clone()];
        let seed = [0x12; 32];
        let targets =
            Actor::block_sync_update_targets_for_peers(&local, 3, &peers, &peers, &online, &seed);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], peer_b);
        assert!(online.contains(&targets[0]));
        assert!(!targets.contains(&local));
    }

    #[test]
    fn block_sync_update_targets_for_peers_fallback_to_world() {
        let local = PeerId::new(KeyPair::random().public_key().clone());
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let peers = vec![local.clone(), peer_a.clone(), peer_b.clone()];
        let online = vec![local.clone()];
        let seed = [0xDE; 32];
        let targets =
            Actor::block_sync_update_targets_for_peers(&local, 1, &peers, &peers, &online, &seed);
        let repeat =
            Actor::block_sync_update_targets_for_peers(&local, 1, &peers, &peers, &online, &seed);

        assert_eq!(targets, repeat);
        assert_eq!(targets.len(), 1);
        assert!(!targets.contains(&local));
        assert!(targets.iter().all(|peer| peers.contains(peer)));
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
            let sig = Signature::new(kp.private_key(), &preimage);
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
            creation_time_ms: 0,
            view_change_index: view,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
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

        assert!(Actor::payload_available_for_da(
            &sessions, &handle, &pending
        ));
    }

    #[test]
    fn payload_available_for_da_accepts_rbc_delivery() {
        let block = sample_block(2, 0);
        let payload_hash = Hash::new(b"not-a-payload");
        let pending = PendingBlock::new(block, payload_hash, 2, 0);
        let sessions = BTreeMap::new();
        let handle = rbc_status::Handle::new();

        let summary = rbc_status::Summary {
            block_hash: pending.block.hash(),
            height: pending.height,
            view: pending.view,
            total_chunks: 0,
            received_chunks: 0,
            ready_count: 0,
            delivered: true,
            payload_hash: Some(pending.payload_hash),
            recovered_from_disk: false,
            invalid: false,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary, std::time::SystemTime::now());

        assert!(Actor::payload_available_for_da(
            &sessions, &handle, &pending
        ));
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

        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
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
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
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
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut vote_log = BTreeMap::new();
        vote_log.insert(
            (crate::sumeragi::consensus::Phase::Commit, 4, 0, 0, 0),
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
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
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
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
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
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
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
    fn recover_qc_from_kura_block_falls_back_to_roster() {
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
        let keypairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
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

        let mut vote_log: BTreeMap<
            (
                crate::sumeragi::consensus::Phase,
                u64,
                u64,
                u64,
                crate::sumeragi::consensus::ValidatorIndex,
            ),
            crate::sumeragi::consensus::Vote,
        > = BTreeMap::new();
        let vote = crate::sumeragi::consensus::Vote {
            phase: crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 4,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: vec![0u8; 96],
        };
        vote_log.insert(
            (vote.phase, vote.height, vote.view, vote.epoch, vote.signer),
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
        let qc_cache: BTreeMap<
            (
                crate::sumeragi::consensus::Phase,
                HashOf<BlockHeader>,
                u64,
                u64,
                u64,
            ),
            crate::sumeragi::consensus::Qc,
        > = BTreeMap::new();

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
    fn rbc_payload_bundle_builds_init_and_chunks() {
        let block = sample_block(5, 0);
        let block_hash = block.hash();
        let payload_hash = Hash::prehashed([0x11; 32]);
        let chunk_root = Hash::prehashed([0x22; 32]);
        let mut session = RbcSession::test_new(2, Some(payload_hash), Some(chunk_root), 0);
        session.test_set_block_header_and_signature(&block);
        session.test_note_chunk(0, vec![1, 2, 3], 0);
        session.test_note_chunk(1, vec![4, 5], 0);
        let roster = vec![PeerId::new(KeyPair::random().public_key().clone())];
        let roster_hash = super::rbc::rbc_roster_hash(&roster);

        let (init, chunks) =
            super::super::Actor::rbc_payload_bundle((block_hash, 5, 0), &session, &roster)
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
        let roster = vec![PeerId::new(KeyPair::random().public_key().clone())];

        let (init, chunks) =
            super::super::Actor::rbc_payload_bundle((block_hash, 7, 0), &session, &roster)
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
        let roster = vec![PeerId::new(KeyPair::random().public_key().clone())];
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
