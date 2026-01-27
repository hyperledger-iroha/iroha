//! Vote signature verification workers.

use std::{collections::BTreeMap, sync::mpsc};

use super::*;
use iroha_crypto::Algorithm;

const VOTE_VERIFY_BATCH_MAX: usize = 64;

/// Vote signature verification request payload.
#[derive(Debug)]
pub(super) struct VoteVerifyWork {
    pub(super) id: u64,
    pub(super) key: VoteVerifyKey,
    pub(super) vote: crate::sumeragi::consensus::Vote,
    pub(super) signature_topology: super::network_topology::Topology,
    pub(super) pops: BTreeMap<PublicKey, Vec<u8>>,
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

/// Spawn vote signature verification workers.
pub(super) fn spawn_vote_verify_workers(
    wake_tx: Option<mpsc::SyncSender<()>>,
    worker_threads: usize,
    work_queue_cap: usize,
    result_queue_cap: usize,
) -> VoteVerifyWorkerHandle {
    let threads = worker_threads.max(1);
    let work_queue_cap = work_queue_cap.max(1);
    let result_queue_cap = result_queue_cap.max(1);
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
                                results.push(VoteVerifyResult {
                                    id: work.id,
                                    key: work.key,
                                    signature_result: Err(VoteSignatureError::SignerIndexOverflow(
                                        u64::from(signer_raw),
                                    )),
                                });
                                continue;
                            }
                        };
                        let Some(peer) = work.signature_topology.as_ref().get(idx) else {
                            results.push(VoteVerifyResult {
                                id: work.id,
                                key: work.key,
                                signature_result: Err(VoteSignatureError::SignerOutOfRange {
                                    signer: idx.try_into().unwrap_or(u32::MAX),
                                    roster_len: work
                                        .signature_topology
                                        .as_ref()
                                        .len()
                                        .try_into()
                                        .unwrap_or(u32::MAX),
                                }),
                            });
                            continue;
                        };
                        if work.vote.bls_sig.is_empty() {
                            results.push(VoteVerifyResult {
                                id: work.id,
                                key: work.key,
                                signature_result: Err(VoteSignatureError::SignatureInvalid),
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
                        let prepared_vote = prepared[idx].take().expect("prepared vote");
                        results.push(VoteVerifyResult {
                            id: prepared_vote.work.id,
                            key: prepared_vote.work.key,
                            signature_result,
                        });
                    }

                    for ((algorithm, preimage), indices) in groups {
                        if indices.len() == 1 {
                            let idx = indices[0];
                            let prepared_vote = prepared[idx].as_ref().expect("prepared vote");
                            let signature_result = verify_single(prepared_vote);
                            let prepared_vote = prepared[idx].take().expect("prepared vote");
                            results.push(VoteVerifyResult {
                                id: prepared_vote.work.id,
                                key: prepared_vote.work.key,
                                signature_result,
                            });
                            continue;
                        }

                        let signatures: Vec<&[u8]> = indices
                            .iter()
                            .map(|idx| {
                                prepared[*idx]
                                    .as_ref()
                                    .expect("prepared vote")
                                    .work
                                    .vote
                                    .bls_sig
                                    .as_slice()
                            })
                            .collect();
                        let mut public_keys: Vec<&PublicKey> = Vec::with_capacity(indices.len());
                        let mut pops: Vec<&[u8]> = Vec::with_capacity(indices.len());
                        let mut missing_pop = false;
                        for idx in &indices {
                            let prepared_vote = prepared[*idx].as_ref().expect("prepared vote");
                            let Some(pop) = prepared_vote.pop.as_ref() else {
                                missing_pop = true;
                                break;
                            };
                            public_keys.push(&prepared_vote.public_key);
                            pops.push(pop.as_slice());
                        }
                        let use_batch = if missing_pop {
                            false
                        } else {
                            match algorithm {
                                Algorithm::BlsNormal => {
                                    iroha_crypto::bls_normal_verify_aggregate_same_message(
                                        &preimage,
                                        &signatures,
                                        &public_keys,
                                        &pops,
                                    )
                                    .is_ok()
                                }
                                Algorithm::BlsSmall => {
                                    iroha_crypto::bls_small_verify_aggregate_same_message(
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

                        for idx in indices {
                            let prepared_vote = prepared[idx].as_ref().expect("prepared vote");
                            let signature_result = if use_batch {
                                Ok(())
                            } else {
                                verify_single(prepared_vote)
                            };
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
        let Some(result_rx) = self.subsystems.vote_verify.result_rx.take() else {
            return false;
        };
        let mut progress = false;
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
                    let chain_id = self.common_config.chain.clone();
                    let context = inflight.context;
                    let evidence_context = super::evidence::EvidenceValidationContext {
                        topology: &context.topology,
                        chain_id: &chain_id,
                        mode_tag: context.mode_tag,
                        prf_seed: context.prf_seed,
                    };
                    if self.validate_and_record_vote_with_signature_result(
                        &inflight.vote,
                        &context.signature_topology,
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
        if keep_rx {
            self.subsystems.vote_verify.result_rx = Some(result_rx);
        } else {
            self.subsystems.vote_verify.work_txs.clear();
            self.subsystems.vote_verify.inflight.clear();
        }
        progress
    }
}
