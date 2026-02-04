//! Vote signature verification workers.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Instant,
};

use super::votes::record_vote_drop_without_roster;
use super::*;
use iroha_crypto::{Algorithm, HashOf};

const VOTE_VERIFY_BATCH_MAX: usize = 64;
static VOTE_VERIFY_AGGREGATE_USED_TOTAL: AtomicU64 = AtomicU64::new(0);
static VOTE_VERIFY_AGGREGATE_FALLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);
static VOTE_VERIFY_MISSING_POP_TOTAL: AtomicU64 = AtomicU64::new(0);
static VOTE_VERIFY_MULTI_USED_TOTAL: AtomicU64 = AtomicU64::new(0);
static VOTE_VERIFY_MULTI_FALLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);

fn resolve_worker_config(
    worker_threads: usize,
    work_queue_cap: usize,
    result_queue_cap: usize,
) -> (usize, usize, usize) {
    let threads = if worker_threads == 0 {
        std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
    } else {
        worker_threads
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
    (threads, work_queue_cap, result_queue_cap)
}

/// Vote signature verification request payload.
#[derive(Debug)]
pub(super) struct VoteVerifyWork {
    pub(super) id: u64,
    pub(super) key: VoteVerifyKey,
    pub(super) vote: crate::sumeragi::consensus::Vote,
    pub(super) signature_topology: Arc<super::network_topology::Topology>,
    pub(super) pops: Arc<BTreeMap<PublicKey, Vec<u8>>>,
    pub(super) chain_id: ChainId,
    pub(super) mode_tag: &'static str,
}

/// Vote signature verification result payload.
#[derive(Debug)]
pub(super) struct VoteVerifyResult {
    pub(super) id: u64,
    pub(super) key: VoteVerifyKey,
    pub(super) signature_result: Result<(), VoteSignatureError>,
}

/// Spawn handle for vote signature verification workers.
#[derive(Debug)]
pub(super) struct VoteVerifyWorkerHandle {
    pub(super) work_txs: Vec<mpsc::SyncSender<VoteVerifyWork>>,
    pub(super) result_rx: mpsc::Receiver<VoteVerifyResult>,
    pub(super) join_handles: Vec<std::thread::JoinHandle<()>>,
}

struct PreparedVote {
    work: VoteVerifyWork,
    preimage: Vec<u8>,
    algorithm: Algorithm,
    public_key: PublicKey,
    pop: Option<Vec<u8>>,
}

fn log_vote_verify_rejection(work: &VoteVerifyWork, err: &VoteSignatureError) {
    let roster = work.signature_topology.as_ref().as_ref();
    let roster_len = roster.len();
    let roster_hash = HashOf::new(&roster.to_vec());
    debug!(
        phase = ?work.vote.phase,
        height = work.vote.height,
        view = work.vote.view,
        epoch = work.vote.epoch,
        signer = work.vote.signer,
        block_hash = %work.vote.block_hash,
        roster_len,
        roster_hash = %roster_hash,
        mode_tag = work.mode_tag,
        reason = ?err,
        "vote verify rejected vote"
    );
}

/// Spawn vote signature verification workers.
pub(super) fn spawn_vote_verify_workers(
    wake_tx: Option<mpsc::SyncSender<()>>,
    worker_threads: usize,
    work_queue_cap: usize,
    result_queue_cap: usize,
) -> VoteVerifyWorkerHandle {
    let (threads, work_queue_cap, result_queue_cap) =
        resolve_worker_config(worker_threads, work_queue_cap, result_queue_cap);
    let (result_tx, result_rx) = mpsc::sync_channel::<VoteVerifyResult>(result_queue_cap);
    let mut work_txs = Vec::with_capacity(threads);
    let mut join_handles = Vec::with_capacity(threads);
    for idx in 0..threads {
        let (work_tx, work_rx) = mpsc::sync_channel::<VoteVerifyWork>(work_queue_cap);
        work_txs.push(work_tx);
        let result_tx = result_tx.clone();
        let wake_tx = wake_tx.clone();
        let name = format!("sumeragi-vote-verify-{idx}");
        let join_handle = std::thread::Builder::new()
            .name(name)
            .spawn(move || {
                let mut batch = Vec::with_capacity(VOTE_VERIFY_BATCH_MAX);
                'worker: while let Ok(work) = work_rx.recv() {
                    batch.clear();
                    batch.push(work);
                    let mut disconnected = false;
                    while batch.len() < VOTE_VERIFY_BATCH_MAX {
                        match work_rx.try_recv() {
                            Ok(work) => batch.push(work),
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                disconnected = true;
                                break;
                            }
                        }
                    }

                    let mut results = Vec::with_capacity(batch.len());
                    let mut prepared: Vec<Option<PreparedVote>> = Vec::with_capacity(batch.len());

                    for work in batch.drain(..) {
                        let signer_raw = work.vote.signer;
                        let idx = match usize::try_from(signer_raw) {
                            Ok(idx) => idx,
                            Err(_) => {
                                let signature_result = Err(
                                    VoteSignatureError::SignerIndexOverflow(u64::from(signer_raw)),
                                );
                                if let Err(err) = &signature_result {
                                    log_vote_verify_rejection(&work, err);
                                }
                                results.push(VoteVerifyResult {
                                    id: work.id,
                                    key: work.key,
                                    signature_result,
                                });
                                continue;
                            }
                        };
                        let roster = work.signature_topology.as_ref().as_ref();
                        let Some(peer) = roster.get(idx) else {
                            let signature_result = Err(VoteSignatureError::SignerOutOfRange {
                                signer: idx.try_into().unwrap_or(u32::MAX),
                                roster_len: roster.len().try_into().unwrap_or(u32::MAX),
                            });
                            if let Err(err) = &signature_result {
                                log_vote_verify_rejection(&work, err);
                            }
                            results.push(VoteVerifyResult {
                                id: work.id,
                                key: work.key,
                                signature_result,
                            });
                            continue;
                        };
                        if work.vote.bls_sig.is_empty() {
                            let signature_result = Err(VoteSignatureError::SignatureInvalid);
                            if let Err(err) = &signature_result {
                                log_vote_verify_rejection(&work, err);
                            }
                            results.push(VoteVerifyResult {
                                id: work.id,
                                key: work.key,
                                signature_result,
                            });
                            continue;
                        }
                        let preimage =
                            super::vote_preimage(&work.chain_id, work.mode_tag, &work.vote);
                        let public_key = peer.public_key().clone();
                        let algorithm = public_key.algorithm();
                        let pop = work.pops.get(&public_key).cloned();
                        prepared.push(Some(PreparedVote {
                            work,
                            preimage,
                            algorithm,
                            public_key,
                            pop,
                        }));
                    }

                    let verify_single = |prepared: &PreparedVote| {
                        let signature = Signature::from_bytes(&prepared.work.vote.bls_sig);
                        signature
                            .verify(&prepared.public_key, &prepared.preimage)
                            .map_err(|_| VoteSignatureError::SignatureInvalid)
                    };

                    let mut groups: BTreeMap<(Algorithm, Vec<u8>), Vec<usize>> = BTreeMap::new();
                    let mut fallback_indices = Vec::new();
                    for (idx, entry) in prepared.iter().enumerate() {
                        let Some(prepared_vote) = entry.as_ref() else {
                            continue;
                        };
                        match prepared_vote.algorithm {
                            Algorithm::BlsNormal | Algorithm::BlsSmall => {
                                groups
                                    .entry((
                                        prepared_vote.algorithm,
                                        prepared_vote.preimage.clone(),
                                    ))
                                    .or_default()
                                    .push(idx);
                            }
                            _ => fallback_indices.push(idx),
                        }
                    }

                    for idx in fallback_indices {
                        let prepared_vote = prepared[idx].as_ref().expect("prepared vote");
                        let signature_result = verify_single(prepared_vote);
                        if let Err(err) = &signature_result {
                            log_vote_verify_rejection(&prepared_vote.work, err);
                        }
                        let prepared_vote = prepared[idx].take().expect("prepared vote");
                        results.push(VoteVerifyResult {
                            id: prepared_vote.work.id,
                            key: prepared_vote.work.key,
                            signature_result,
                        });
                    }

                    let mut multi_candidates: BTreeMap<Algorithm, Vec<usize>> = BTreeMap::new();

                    for ((algorithm, preimage), indices) in groups {
                        if indices.len() == 1 {
                            multi_candidates.entry(algorithm).or_default().push(indices[0]);
                            continue;
                        }

                        let mut signatures = Vec::with_capacity(indices.len());
                        for idx in &indices {
                            let sig = prepared[*idx]
                                .as_ref()
                                .expect("prepared vote")
                                .work
                                .vote
                                .bls_sig
                                .as_slice();
                            signatures.push(sig);
                        }
                        let mut public_keys: Vec<&PublicKey> = Vec::with_capacity(indices.len());
                        let mut pops: Vec<&[u8]> = Vec::with_capacity(indices.len());
                        let mut missing_pop_count = 0usize;
                        for idx in &indices {
                            let prepared_vote = prepared[*idx].as_ref().expect("prepared vote");
                            if let Some(pop) = prepared_vote.pop.as_ref() {
                                public_keys.push(&prepared_vote.public_key);
                                pops.push(pop.as_slice());
                            } else {
                                missing_pop_count = missing_pop_count.saturating_add(1);
                            }
                        }
                        if missing_pop_count > 0 {
                            let total = VOTE_VERIFY_MISSING_POP_TOTAL
                                .fetch_add(missing_pop_count as u64, Ordering::Relaxed)
                                .saturating_add(missing_pop_count as u64);
                            if super::status::should_log_vote_drop_count(total) {
                                debug!(
                                    missing_pop_total = total,
                                    missing_pop = missing_pop_count,
                                    batch = indices.len(),
                                    "vote verify missing PoP; aggregate verification disabled"
                                );
                            }
                        }
                        let use_batch = if missing_pop_count > 0 {
                            false
                        } else {
                            match algorithm {
                                Algorithm::BlsNormal => {
                                    iroha_crypto::bls_normal_verify_aggregate_same_message_fast(
                                        &preimage,
                                        &signatures,
                                        &public_keys,
                                        &pops,
                                    )
                                    .is_ok()
                                }
                                Algorithm::BlsSmall => {
                                    iroha_crypto::bls_small_verify_aggregate_same_message_fast(
                                        &preimage,
                                        &signatures,
                                        &public_keys,
                                        &pops,
                                    )
                                    .is_ok()
                                }
                                _ => false,
                            }
                        };
                        let aggregate_total = if use_batch {
                            VOTE_VERIFY_AGGREGATE_USED_TOTAL
                                .fetch_add(indices.len() as u64, Ordering::Relaxed)
                                .saturating_add(indices.len() as u64)
                        } else {
                            VOTE_VERIFY_AGGREGATE_FALLBACK_TOTAL
                                .fetch_add(indices.len() as u64, Ordering::Relaxed)
                                .saturating_add(indices.len() as u64)
                        };
                        if super::status::should_log_vote_drop_count(aggregate_total) {
                            debug!(
                                aggregate_total,
                                batch = indices.len(),
                                use_batch,
                                ?algorithm,
                                "vote verify aggregate status"
                            );
                        }

                        for idx in indices {
                            let prepared_vote = prepared[idx].as_ref().expect("prepared vote");
                            let signature_result = if use_batch {
                                Ok(())
                            } else {
                                verify_single(prepared_vote)
                            };
                            if let Err(err) = &signature_result {
                                log_vote_verify_rejection(&prepared_vote.work, err);
                            }
                            let prepared_vote = prepared[idx].take().expect("prepared vote");
                            results.push(VoteVerifyResult {
                                id: prepared_vote.work.id,
                                key: prepared_vote.work.key,
                                signature_result,
                            });
                        }
                    }

                    for (algorithm, indices) in multi_candidates {
                        if indices.len() == 1 {
                            let idx = indices[0];
                            let prepared_vote = prepared[idx].as_ref().expect("prepared vote");
                            let signature_result = verify_single(prepared_vote);
                            if let Err(err) = &signature_result {
                                log_vote_verify_rejection(&prepared_vote.work, err);
                            }
                            let prepared_vote = prepared[idx].take().expect("prepared vote");
                            results.push(VoteVerifyResult {
                                id: prepared_vote.work.id,
                                key: prepared_vote.work.key,
                                signature_result,
                            });
                            continue;
                        }

                        let mut messages: Vec<&[u8]> = Vec::with_capacity(indices.len());
                        let mut signatures: Vec<&[u8]> = Vec::with_capacity(indices.len());
                        let mut public_keys: Vec<&[u8]> = Vec::with_capacity(indices.len());
                        for idx in &indices {
                            let prepared_vote = prepared[*idx].as_ref().expect("prepared vote");
                            let (_, pk_bytes) = prepared_vote.public_key.to_bytes();
                            messages.push(prepared_vote.preimage.as_slice());
                            signatures.push(prepared_vote.work.vote.bls_sig.as_slice());
                            public_keys.push(pk_bytes);
                        }

                        let use_batch = match algorithm {
                            Algorithm::BlsNormal => {
                                iroha_crypto::BlsNormal::verify_aggregate_multi_message(
                                    &messages,
                                    &signatures,
                                    &public_keys,
                                )
                                .is_ok()
                            }
                            Algorithm::BlsSmall => {
                                iroha_crypto::BlsSmall::verify_aggregate_multi_message(
                                    &messages,
                                    &signatures,
                                    &public_keys,
                                )
                                .is_ok()
                            }
                            _ => false,
                        };
                        let aggregate_total = if use_batch {
                            VOTE_VERIFY_MULTI_USED_TOTAL
                                .fetch_add(indices.len() as u64, Ordering::Relaxed)
                                .saturating_add(indices.len() as u64)
                        } else {
                            VOTE_VERIFY_MULTI_FALLBACK_TOTAL
                                .fetch_add(indices.len() as u64, Ordering::Relaxed)
                                .saturating_add(indices.len() as u64)
                        };
                        if super::status::should_log_vote_drop_count(aggregate_total) {
                            debug!(
                                aggregate_total,
                                batch = indices.len(),
                                use_batch,
                                ?algorithm,
                                "vote verify multi-message aggregate status"
                            );
                        }

                        for idx in indices {
                            let prepared_vote = prepared[idx].as_ref().expect("prepared vote");
                            let signature_result = if use_batch {
                                Ok(())
                            } else {
                                verify_single(prepared_vote)
                            };
                            if let Err(err) = &signature_result {
                                log_vote_verify_rejection(&prepared_vote.work, err);
                            }
                            let prepared_vote = prepared[idx].take().expect("prepared vote");
                            results.push(VoteVerifyResult {
                                id: prepared_vote.work.id,
                                key: prepared_vote.work.key,
                                signature_result,
                            });
                        }
                    }

                    for result in results {
                        if result_tx.send(result).is_err() {
                            break 'worker;
                        }
                    }
                    if let Some(wake) = wake_tx.as_ref() {
                        let _ = wake.try_send(());
                    }
                    if disconnected {
                        break;
                    }
                }
            })
            .expect("failed to spawn sumeragi vote verify worker thread");
        join_handles.push(join_handle);
    }

    VoteVerifyWorkerHandle {
        work_txs,
        result_rx,
        join_handles,
    }
}

impl Actor {
    pub(in crate::sumeragi) fn poll_vote_verify_results(&mut self) -> bool {
        let mut progress = self.process_pending_vote_validation();
        let Some(result_rx) = self.subsystems.vote_verify.result_rx.take() else {
            if self.dispatch_pending_vote_verifications() {
                progress = true;
            }
            return progress;
        };
        let mut keep_rx = true;
        loop {
            match result_rx.try_recv() {
                Ok(result) => {
                    let VoteVerifyResult {
                        id,
                        key,
                        signature_result,
                    } = result;
                    let Some(inflight) = self.subsystems.vote_verify.inflight.remove(&key) else {
                        iroha_logger::warn!(
                            ?key,
                            "vote verify result received without inflight entry"
                        );
                        continue;
                    };
                    if inflight.id != id {
                        iroha_logger::warn!(
                            ?key,
                            inflight_id = inflight.id,
                            result_id = id,
                            "vote verify result id mismatch; ignoring"
                        );
                        continue;
                    }
                    let committed_height =
                        u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
                    let stale_view = self.stale_view(inflight.vote.height, inflight.vote.view);
                    if self.drop_vote_for_height_or_view(
                        &inflight.vote,
                        committed_height,
                        stale_view,
                    ) || self.drop_precommit_vote_for_lock(&inflight.vote)
                    {
                        progress = true;
                        continue;
                    }
                    if self.invalid_sig_penalty.is_suppressed(
                        InvalidSigKind::Vote,
                        inflight.vote.signer.into(),
                        Instant::now(),
                    ) {
                        debug!(
                            phase = ?inflight.vote.phase,
                            height = inflight.vote.height,
                            view = inflight.vote.view,
                            signer = inflight.vote.signer,
                            block_hash = %inflight.vote.block_hash,
                            kind = InvalidSigKind::Vote.as_str(),
                            "dropping vote from penalized signer"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::QcVote,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::PenalizedSender,
                        );
                        record_vote_drop_without_roster(
                            &inflight.vote,
                            super::status::VoteValidationDropReason::PenalizedSender,
                        );
                        progress = true;
                        continue;
                    }
                    let chain_id = self.common_config.chain.clone();
                    let mut context = inflight.context;
                    context.stale_view = stale_view;
                    let evidence_context = super::evidence::EvidenceValidationContext {
                        topology: &context.topology,
                        chain_id: &chain_id,
                        mode_tag: context.mode_tag,
                        prf_seed: context.prf_seed,
                    };
                    if self.validate_and_record_vote_with_signature_result(
                        &inflight.vote,
                        context.signature_topology.as_ref(),
                        &evidence_context,
                        context.mode_tag,
                        Some(signature_result),
                    ) {
                        self.apply_validated_vote(inflight.vote, context);
                    }
                    progress = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    keep_rx = false;
                    break;
                }
            }
        }
        if self.dispatch_pending_vote_verifications() {
            progress = true;
        }
        if keep_rx {
            self.subsystems.vote_verify.result_rx = Some(result_rx);
        } else {
            self.subsystems.vote_verify.work_txs.clear();
            self.subsystems.vote_verify.inflight.clear();
            self.subsystems.vote_verify.pending.clear();
        }
        progress
    }
}

#[cfg(test)]
pub(super) fn reset_vote_verify_batch_metrics_for_tests() {
    VOTE_VERIFY_AGGREGATE_USED_TOTAL.store(0, Ordering::Relaxed);
    VOTE_VERIFY_AGGREGATE_FALLBACK_TOTAL.store(0, Ordering::Relaxed);
    VOTE_VERIFY_MISSING_POP_TOTAL.store(0, Ordering::Relaxed);
    VOTE_VERIFY_MULTI_USED_TOTAL.store(0, Ordering::Relaxed);
    VOTE_VERIFY_MULTI_FALLBACK_TOTAL.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(super) fn vote_verify_multi_batch_totals_for_tests() -> (u64, u64) {
    (
        VOTE_VERIFY_MULTI_USED_TOTAL.load(Ordering::Relaxed),
        VOTE_VERIFY_MULTI_FALLBACK_TOTAL.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vote_verify_worker_config_auto_scales() {
        let expected_threads = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        let (threads, work_cap, result_cap) = resolve_worker_config(0, 0, 0);
        assert_eq!(threads, expected_threads);
        assert_eq!(work_cap, expected_threads.saturating_mul(4).max(4));
        assert_eq!(result_cap, expected_threads.saturating_mul(8).max(8));
    }

    #[cfg(feature = "bls")]
    mod bls {
        use super::*;
        use std::time::Duration;

        use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
        use iroha_data_model::block::BlockHeader;
        use iroha_data_model::peer::PeerId;

        #[test]
        fn vote_verify_uses_multi_message_batch_for_distinct_preimages() {
            reset_vote_verify_batch_metrics_for_tests();

            let handle = spawn_vote_verify_workers(None, 1, 8, 8);
            let work_tx = handle.work_txs[0].clone();

            let mut keypairs = Vec::new();
            let mut peers = Vec::new();
            for idx in 0..4u8 {
                let seed = vec![idx; 32];
                let kp = KeyPair::from_seed(seed, Algorithm::BlsNormal);
                peers.push(PeerId::from(kp.public_key().clone()));
                keypairs.push(kp);
            }
            let topology = Arc::new(super::network_topology::Topology::new(peers));
            let pops = Arc::new(BTreeMap::new());
            let chain_id: ChainId = "vote-batch-test".parse().expect("chain id");

            let mut works = Vec::new();
            for (idx, kp) in keypairs.iter().enumerate() {
                let mut hash_bytes = [0u8; Hash::LENGTH];
                hash_bytes[0] = idx as u8;
                hash_bytes[1] = 0xA5;
                hash_bytes[Hash::LENGTH - 1] = 0x5A ^ (idx as u8);
                let block_hash =
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(hash_bytes));
                let mut vote = crate::sumeragi::consensus::Vote {
                    phase: crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    post_state_root: Hash::prehashed([1u8; Hash::LENGTH]),
                    height: 1,
                    view: 0,
                    epoch: 0,
                    highest_qc: None,
                    signer: idx as u32,
                    bls_sig: Vec::new(),
                };
                let preimage = crate::sumeragi::consensus::vote_preimage(
                    &chain_id,
                    super::PERMISSIONED_TAG,
                    &vote,
                );
                let sig = Signature::new(kp.private_key(), &preimage);
                vote.bls_sig = sig.payload().to_vec();
                let key = VoteVerifyKey::from_vote(&vote);
                works.push(VoteVerifyWork {
                    id: idx as u64,
                    key,
                    vote,
                    signature_topology: Arc::clone(&topology),
                    pops: Arc::clone(&pops),
                    chain_id: chain_id.clone(),
                    mode_tag: super::PERMISSIONED_TAG,
                });
            }

            for work in works {
                work_tx.send(work).expect("send vote verify work");
            }

            for _ in 0..4 {
                handle
                    .result_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("vote verify result");
            }

            let (used, _fallback) = vote_verify_multi_batch_totals_for_tests();
            assert!(
                used >= 2,
                "expected multi-message batch verification to be used"
            );

            drop(handle.work_txs);
            for join in handle.join_handles {
                if let Err(err) = join.join() {
                    panic!("vote verify worker panicked: {err:?}");
                }
            }
        }
    }
}
