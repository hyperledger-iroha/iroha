//! RBC planning, chunking, and hydration helpers.

use std::{
    collections::BTreeMap,
    io,
    sync::mpsc,
    time::{Duration, Instant, SystemTime},
};

use eyre::{Result, eyre};
use iroha_crypto::{Hash, HashOf, MerkleTree, Signature};
use iroha_data_model::{
    ChainId, Encode as _,
    block::{BlockHeader, SignedBlock},
    nexus::{DataSpaceId, LaneId},
};
use iroha_logger::prelude::*;
use iroha_version::codec::DecodeVersioned;
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use sha2::{Digest, Sha256};

use super::{
    Actor, DataspaceAllocation, InvalidSigKind, InvalidSigOutcome, LaneAllocation, PipelinePhase,
    RBC_MAX_TOTAL_CHUNKS, RbcSession,
    pending_rbc::{
        PENDING_RBC_STASH_LIMIT, PendingChunkOutcome, PendingRbcDropReason, PendingRbcMessages,
    },
    proposals::block_payload_bytes,
};
use crate::{
    queue::{Queue, RoutingDecision},
    sumeragi::{
        BackgroundRequest,
        consensus::{
            RbcChunk, RbcDeliver, RbcInit, RbcReady, rbc_deliver_preimage, rbc_ready_preimage,
        },
        message::{BlockCreated, BlockMessage},
        rbc_status,
        rbc_store::{ChunkStore, PersistOutcome, PersistedSession, SessionKey, StorePressure},
        status,
    },
    tx::AcceptedTransaction,
};

#[derive(Clone, Debug)]
pub(super) struct RbcSessionPlan {
    pub(super) key: SessionKey,
    pub(super) session: RbcSession,
    pub(super) init: RbcInit,
    pub(super) chunks: Vec<RbcChunk>,
}

#[derive(Clone, Debug)]
pub(super) struct RbcPlan {
    pub(super) primary: RbcSessionPlan,
    pub(super) duplicate: Option<RbcSessionPlan>,
}

const RBC_PERSIST_WORK_QUEUE_CAP: usize = 8;
const RBC_PERSIST_RESULT_QUEUE_CAP: usize = 8;

#[derive(Debug)]
pub(super) struct RbcPersistWork {
    pub(super) key: SessionKey,
    pub(super) persisted: PersistedSession,
}

#[derive(Debug)]
pub(super) struct RbcPersistResult {
    pub(super) key: SessionKey,
    pub(super) outcome: io::Result<PersistOutcome>,
}

#[derive(Debug)]
pub(super) struct RbcPersistWorkerHandle {
    pub(super) work_tx: mpsc::SyncSender<RbcPersistWork>,
    pub(super) result_rx: mpsc::Receiver<RbcPersistResult>,
    pub(super) join_handle: std::thread::JoinHandle<()>,
}

fn spawn_rbc_persist_worker(
    cfg: crate::sumeragi::RbcStoreConfig,
) -> io::Result<RbcPersistWorkerHandle> {
    let (work_tx, work_rx) = mpsc::sync_channel::<RbcPersistWork>(RBC_PERSIST_WORK_QUEUE_CAP);
    let (result_tx, result_rx) =
        mpsc::sync_channel::<RbcPersistResult>(RBC_PERSIST_RESULT_QUEUE_CAP);
    let store = ChunkStore::new(
        cfg.dir.clone(),
        cfg.ttl,
        cfg.soft_sessions,
        cfg.soft_bytes,
        cfg.max_sessions,
        cfg.max_bytes,
    )?;
    let join_handle = std::thread::Builder::new()
        .name("sumeragi-rbc-persist".to_owned())
        .spawn(move || {
            while let Ok(work) = work_rx.recv() {
                let key = work.key;
                let outcome = store.persist_snapshot(&work.persisted);
                if result_tx.send(RbcPersistResult { key, outcome }).is_err() {
                    break;
                }
            }
        })?;
    Ok(RbcPersistWorkerHandle {
        work_tx,
        result_rx,
        join_handle,
    })
}

pub(super) fn chunk_payload_bytes(payload: &[u8], chunk_size: usize) -> Vec<Vec<u8>> {
    let effective = chunk_size.max(1);
    if payload.is_empty() {
        return Vec::new();
    }
    payload.chunks(effective).map(<[_]>::to_vec).collect()
}

#[inline]
pub(super) fn chunk_count(payload_len: usize, chunk_size: usize) -> usize {
    let effective = chunk_size.max(1);
    payload_len.div_ceil(effective)
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub(super) struct HydrationOutcome {
    pub(super) updated: bool,
    pub(super) all_chunks_present: bool,
    pub(super) payload_hash_mismatch: bool,
    pub(super) chunk_root_mismatch: bool,
    pub(super) layout_mismatch: bool,
    pub(super) observed_chunks: Option<u32>,
}

pub(super) fn apply_hydrated_payload(
    session: &mut RbcSession,
    payload_bytes: &[u8],
    payload_hash: Hash,
    chunk_max_bytes: usize,
) -> HydrationOutcome {
    let mut outcome = HydrationOutcome::default();
    let chunk_bytes = chunk_payload_bytes(payload_bytes, chunk_max_bytes);

    let chunk_count = if let Ok(count) = u32::try_from(chunk_bytes.len()) {
        count
    } else {
        session.invalid = true;
        outcome.updated = true;
        outcome.layout_mismatch = true;
        return outcome;
    };
    outcome.observed_chunks = Some(chunk_count);
    let expected_chunks = session.total_chunks();
    if expected_chunks == 0 || chunk_count != expected_chunks {
        session.invalid = true;
        outcome.updated = true;
        outcome.layout_mismatch = true;
        return outcome;
    }

    for (idx, chunk) in chunk_bytes.iter().enumerate() {
        let Ok(idx_u32) = u32::try_from(idx) else {
            return outcome;
        };
        if session.chunk_bytes(idx_u32).is_none() {
            session.ingest_chunk(idx_u32, chunk.clone(), None);
            outcome.updated = true;
        }
    }

    if let Some(existing) = session.payload_hash() {
        if existing != payload_hash {
            session.invalid = true;
            outcome.payload_hash_mismatch = true;
        }
    } else {
        session.payload_hash = Some(payload_hash);
        outcome.updated = true;
    }

    let all_chunks_present = session.total_chunks() == 0
        || (session.total_chunks() != 0 && session.received_chunks() == session.total_chunks());
    if all_chunks_present {
        outcome.all_chunks_present = true;
        if let Some(computed_root) = session.chunk_root() {
            if let Some(expected_root) = session.expected_chunk_root {
                if expected_root != computed_root {
                    session.invalid = true;
                    outcome.chunk_root_mismatch = true;
                }
            } else {
                session.expected_chunk_root = Some(computed_root);
                outcome.updated = true;
            }
        }
    }

    if outcome.payload_hash_mismatch || outcome.chunk_root_mismatch {
        outcome.updated = true;
    }

    outcome
}

pub(super) fn shuffle_seed(block_hash: &HashOf<BlockHeader>, height: u64, view: u64) -> u64 {
    let raw = block_hash.as_ref().as_ref();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&raw[..8]);
    let mut seed = u64::from_le_bytes(bytes);
    seed ^= height;
    seed = seed.rotate_left(13) ^ view;
    seed
}

pub(super) fn compute_chunk_broadcast_order(
    chunk_count: usize,
    shuffle: bool,
    seed: u64,
    drop_every: Option<usize>,
) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..chunk_count).collect();
    if shuffle && chunk_count > 1 {
        let mut rng = StdRng::seed_from_u64(seed);
        order.shuffle(&mut rng);
    }
    let mut dropped = 0usize;
    if let Some(interval) = drop_every.filter(|value| *value > 0) {
        let mut filtered = Vec::with_capacity(order.len());
        for (pos, idx) in order.into_iter().enumerate() {
            if (pos + 1) % interval == 0 {
                dropped += 1;
                continue;
            }
            filtered.push(idx);
        }
        order = filtered;
    }
    (order, dropped)
}

pub(super) fn distribute_chunks(total_chunks: u32, weights: &[u128]) -> Vec<u32> {
    if weights.is_empty() {
        return Vec::new();
    }
    if total_chunks == 0 {
        return vec![0; weights.len()];
    }

    let total_weight: u128 = weights.iter().copied().sum();
    // Fallback to even distribution when all weights are zero.
    if total_weight == 0 {
        let mut allocations = vec![0u32; weights.len()];
        let mut remaining = total_chunks;
        for allocation in &mut allocations {
            if remaining == 0 {
                break;
            }
            *allocation = allocation.saturating_add(1);
            remaining -= 1;
        }
        return allocations;
    }

    let mut allocations = vec![0u32; weights.len()];
    let mut sum_assigned = 0u32;
    let mut fractional: Vec<(usize, u128)> = Vec::with_capacity(weights.len());

    for (idx, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            fractional.push((idx, 0));
            continue;
        }
        let exact = weight.saturating_mul(u128::from(total_chunks));
        let base = u32::try_from(exact / total_weight).unwrap_or(u32::MAX);
        let remainder = exact % total_weight;
        allocations[idx] = base;
        sum_assigned = sum_assigned.saturating_add(base);
        fractional.push((idx, remainder));
    }

    let mut leftover = total_chunks.saturating_sub(sum_assigned);
    if leftover > 0 {
        fractional.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (idx, _) in fractional {
            if leftover == 0 {
                break;
            }
            allocations[idx] = allocations[idx].saturating_add(1);
            leftover -= 1;
        }
    }

    allocations
}

pub(super) fn rbc_ready_signature_valid(
    ready: &RbcReady,
    topology: &crate::sumeragi::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
) -> bool {
    let Ok(idx) = usize::try_from(ready.sender) else {
        return false;
    };
    let Some(peer) = topology.as_ref().get(idx) else {
        return false;
    };
    let preimage = rbc_ready_preimage(chain_id, mode_tag, ready);
    let signature = Signature::from_bytes(&ready.signature);
    signature.verify(peer.public_key(), &preimage).is_ok()
}

pub(super) fn rbc_deliver_signature_valid(
    deliver: &RbcDeliver,
    topology: &crate::sumeragi::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
) -> bool {
    let Ok(idx) = usize::try_from(deliver.sender) else {
        return false;
    };
    let Some(peer) = topology.as_ref().get(idx) else {
        return false;
    };
    let preimage = rbc_deliver_preimage(chain_id, mode_tag, deliver);
    let signature = Signature::from_bytes(&deliver.signature);
    signature.verify(peer.public_key(), &preimage).is_ok()
}

pub(super) fn should_process_commit_after_ready(
    recorded_ready: bool,
    clear_pending: bool,
    deliver_emitted: bool,
) -> bool {
    (recorded_ready || clear_pending) && !deliver_emitted
}

pub(super) fn should_process_commit_after_deliver(first_deliver: bool) -> bool {
    first_deliver
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DeliverAcceptance {
    Accept,
    DeferReady { ready_count: usize, required: usize },
    DeferChunks { received: u32, total: u32 },
    InvalidChunkRoot,
}

pub(super) fn evaluate_deliver_acceptance(
    session: &RbcSession,
    required_ready: usize,
) -> DeliverAcceptance {
    if required_ready != 0 && session.ready_signatures.len() < required_ready {
        return DeliverAcceptance::DeferReady {
            ready_count: session.ready_signatures.len(),
            required: required_ready,
        };
    }

    let received = session.received_chunks();
    let total = session.total_chunks();
    if total != 0 && received < total {
        return DeliverAcceptance::DeferChunks { received, total };
    }

    if let (Some(expected), Some(computed)) = (session.expected_chunk_root, session.chunk_root()) {
        if expected != computed {
            return DeliverAcceptance::InvalidChunkRoot;
        }
    }

    DeliverAcceptance::Accept
}

pub(super) struct RbcPlanInputs<'a> {
    pub(super) signed_block: &'a SignedBlock,
    pub(super) transactions: &'a [AcceptedTransaction<'static>],
    pub(super) routing: &'a [RoutingDecision],
    pub(super) payload: &'a [u8],
    pub(super) payload_hash: Hash,
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) epoch: u64,
    pub(super) local_validator_index: u32,
}

impl Actor {
    #[allow(clippy::too_many_lines)]
    pub(super) fn prepare_rbc_plan(&self, inputs: RbcPlanInputs<'_>) -> Result<Option<RbcPlan>> {
        let RbcPlanInputs {
            signed_block,
            transactions,
            routing,
            payload,
            payload_hash,
            height,
            view,
            epoch,
            local_validator_index,
        } = inputs;
        let da_enabled = self.runtime_da_enabled();
        if !da_enabled {
            return Ok(None);
        }

        if payload.is_empty() {
            return Ok(None);
        }

        debug_assert_eq!(
            transactions.len(),
            routing.len(),
            "routing decisions must align with transactions"
        );

        let chunk_bytes = chunk_payload_bytes(payload, self.config.rbc_chunk_max_bytes);
        if chunk_bytes.is_empty() {
            return Ok(None);
        }

        let total_chunks = u32::try_from(chunk_bytes.len())
            .map_err(|_| eyre!("too many RBC chunks ({})", chunk_bytes.len()))?;
        if total_chunks > RBC_MAX_TOTAL_CHUNKS {
            return Err(eyre!(
                "RBC payload requires {total_chunks} chunks, exceeding hard cap {}",
                RBC_MAX_TOTAL_CHUNKS
            ));
        }

        let (lane_allocations, dataspace_allocations) =
            self.derive_rbc_allocations(transactions, routing, total_chunks)?;

        let mut digests = Vec::with_capacity(chunk_bytes.len());
        for chunk in &chunk_bytes {
            let digest = Sha256::digest(chunk);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&digest);
            digests.push(arr);
        }

        let merkle = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests);
        let chunk_root = merkle
            .root()
            .map(Hash::from)
            .ok_or_else(|| eyre!("failed to compute RBC chunk root"))?;

        let block_hash = signed_block.hash();
        let mut session =
            RbcSession::new(total_chunks, Some(payload_hash), Some(chunk_root), epoch)
                .map_err(|err| eyre!(err))?;
        for (idx, chunk) in chunk_bytes.iter().enumerate() {
            let chunk_index = u32::try_from(idx).expect("chunk index fits within a 32-bit range");
            session.ingest_chunk(chunk_index, chunk.clone(), Some(local_validator_index));
        }

        session.set_allocations(lane_allocations, dataspace_allocations);

        let drop_every = self
            .config
            .debug_rbc_drop_every_nth_chunk
            .and_then(|value| usize::try_from(value.get()).ok());
        let (order, dropped) = compute_chunk_broadcast_order(
            chunk_bytes.len(),
            self.config.debug_rbc_shuffle_chunks,
            shuffle_seed(&block_hash, height, view),
            drop_every,
        );
        if dropped > 0 {
            debug!(height, view, dropped, "RBC debug config dropped chunks");
        }

        let mut chunks = Vec::with_capacity(order.len());
        for idx in order {
            let chunk_index = u32::try_from(idx).expect("chunk index fits within a 32-bit range");
            if self.should_withhold_chunk(chunk_index) {
                debug!(
                    height,
                    view,
                    chunk_idx = idx,
                    "withholding RBC chunk due to debug mask"
                );
                continue;
            }
            chunks.push(crate::sumeragi::consensus::RbcChunk {
                block_hash,
                height,
                view,
                epoch,
                idx: chunk_index,
                bytes: chunk_bytes[idx].clone(),
            });
        }

        let init = crate::sumeragi::consensus::RbcInit {
            block_hash,
            height,
            view,
            epoch,
            total_chunks,
            payload_hash,
            chunk_root,
        };

        let primary_plan = RbcSessionPlan {
            key: Self::session_key(&block_hash, height, view),
            session,
            init,
            chunks,
        };

        let duplicate_plan = if self.config.debug_rbc_duplicate_inits {
            if view == u64::MAX {
                None
            } else {
                let dup_view = view + 1;
                let dup_session = primary_plan.session.clone();
                let (dup_order, dup_dropped) = compute_chunk_broadcast_order(
                    chunk_bytes.len(),
                    self.config.debug_rbc_shuffle_chunks,
                    shuffle_seed(&block_hash, height, dup_view),
                    drop_every,
                );
                if dup_dropped > 0 {
                    debug!(
                        height,
                        view = dup_view,
                        dup_dropped,
                        "RBC duplicate init dropped chunks due to debug config"
                    );
                }
                let mut dup_chunks = Vec::with_capacity(dup_order.len());
                for idx in dup_order {
                    let chunk_index =
                        u32::try_from(idx).expect("duplicate chunk index fits within 32 bits");
                    if self.should_withhold_chunk(chunk_index) {
                        debug!(
                            height,
                            view = dup_view,
                            chunk_idx = idx,
                            "withholding duplicate RBC chunk due to debug mask"
                        );
                        continue;
                    }
                    dup_chunks.push(crate::sumeragi::consensus::RbcChunk {
                        block_hash,
                        height,
                        view: dup_view,
                        epoch,
                        idx: chunk_index,
                        bytes: chunk_bytes[idx].clone(),
                    });
                }
                debug!(
                    height,
                    base_view = view,
                    duplicate_view = dup_view,
                    "broadcasting duplicate RBC init due to debug config"
                );
                Some(RbcSessionPlan {
                    key: Self::session_key(&block_hash, height, dup_view),
                    session: dup_session,
                    init: crate::sumeragi::consensus::RbcInit {
                        block_hash,
                        height,
                        view: dup_view,
                        epoch,
                        total_chunks,
                        payload_hash,
                        chunk_root,
                    },
                    chunks: dup_chunks,
                })
            }
        } else {
            None
        };

        Ok(Some(RbcPlan {
            primary: primary_plan,
            duplicate: duplicate_plan,
        }))
    }

    #[allow(clippy::too_many_lines)]
    fn derive_rbc_allocations(
        &self,
        transactions: &[AcceptedTransaction<'static>],
        routing: &[RoutingDecision],
        total_chunks: u32,
    ) -> Result<(Vec<LaneAllocation>, Vec<DataspaceAllocation>)> {
        use std::collections::BTreeMap as StdBTreeMap;

        let mut lane_map: StdBTreeMap<LaneId, LaneAllocation> = StdBTreeMap::new();
        let mut dataspace_map: StdBTreeMap<(LaneId, DataSpaceId), DataspaceAllocation> =
            StdBTreeMap::new();

        for (tx, decision) in transactions.iter().zip(routing.iter()) {
            let encoded = tx.as_ref().encode();
            let len = u64::try_from(encoded.len())
                .map_err(|_| eyre!("transaction payload exceeds addressable length"))?;
            let teu = Queue::estimate_teu(tx);

            lane_map
                .entry(decision.lane_id)
                .and_modify(|alloc| {
                    alloc.tx_count = alloc.tx_count.saturating_add(1);
                    alloc.rbc_bytes_total = alloc.rbc_bytes_total.saturating_add(len);
                    alloc.teu_total = alloc.teu_total.saturating_add(teu);
                })
                .or_insert(LaneAllocation {
                    lane_id: decision.lane_id,
                    tx_count: 1,
                    rbc_bytes_total: len,
                    teu_total: teu,
                    total_chunks: 0,
                });

            dataspace_map
                .entry((decision.lane_id, decision.dataspace_id))
                .and_modify(|alloc| {
                    alloc.tx_count = alloc.tx_count.saturating_add(1);
                    alloc.rbc_bytes_total = alloc.rbc_bytes_total.saturating_add(len);
                    alloc.teu_total = alloc.teu_total.saturating_add(teu);
                })
                .or_insert(DataspaceAllocation {
                    lane_id: decision.lane_id,
                    dataspace_id: decision.dataspace_id,
                    tx_count: 1,
                    rbc_bytes_total: len,
                    teu_total: teu,
                    total_chunks: 0,
                });
        }

        let mut lane_allocations: Vec<LaneAllocation> = lane_map.into_values().collect();
        lane_allocations.sort_by_key(|alloc| alloc.lane_id.as_u32());

        let mut dataspace_allocations: Vec<DataspaceAllocation> =
            dataspace_map.into_values().collect();
        dataspace_allocations.sort_by(|a, b| {
            a.lane_id
                .as_u32()
                .cmp(&b.lane_id.as_u32())
                .then_with(|| a.dataspace_id.as_u64().cmp(&b.dataspace_id.as_u64()))
        });

        let lane_weights: Vec<u128> = lane_allocations
            .iter()
            .map(|alloc| u128::from(alloc.rbc_bytes_total))
            .collect();
        let lane_chunks = distribute_chunks(total_chunks, &lane_weights);
        for (alloc, chunk_count) in lane_allocations.iter_mut().zip(lane_chunks.into_iter()) {
            alloc.total_chunks = chunk_count;
            if alloc.total_chunks == 0 && alloc.rbc_bytes_total > 0 && total_chunks > 0 {
                alloc.total_chunks = 1;
            }
        }

        // Ensure total chunks remain bounded after enforcing minimums.
        let assigned_total: u32 = lane_allocations
            .iter()
            .map(|alloc| alloc.total_chunks)
            .sum();
        if assigned_total > total_chunks && !lane_allocations.is_empty() {
            let mut excess = assigned_total - total_chunks;
            for alloc in lane_allocations.iter_mut().rev() {
                if excess == 0 {
                    break;
                }
                if alloc.total_chunks > 0 {
                    alloc.total_chunks -= 1;
                    excess -= 1;
                }
            }
        }

        let mut lane_chunk_map: StdBTreeMap<LaneId, u32> = StdBTreeMap::new();
        for alloc in &lane_allocations {
            lane_chunk_map.insert(alloc.lane_id, alloc.total_chunks);
        }

        let mut idx = 0usize;
        while idx < dataspace_allocations.len() {
            let lane_id = dataspace_allocations[idx].lane_id;
            let mut end = idx;
            while end < dataspace_allocations.len() && dataspace_allocations[end].lane_id == lane_id
            {
                end += 1;
            }
            let lane_total_chunks = lane_chunk_map.get(&lane_id).copied().unwrap_or(0);
            let weights: Vec<u128> = dataspace_allocations[idx..end]
                .iter()
                .map(|alloc| u128::from(alloc.rbc_bytes_total))
                .collect();
            let allocated = distribute_chunks(lane_total_chunks, &weights);
            for (alloc, chunk_count) in dataspace_allocations[idx..end]
                .iter_mut()
                .zip(allocated.into_iter())
            {
                alloc.total_chunks = chunk_count;
                if alloc.total_chunks == 0 && alloc.rbc_bytes_total > 0 && lane_total_chunks > 0 {
                    alloc.total_chunks = 1;
                }
            }

            let assigned: u32 = dataspace_allocations[idx..end]
                .iter()
                .map(|alloc| alloc.total_chunks)
                .sum();
            if assigned > lane_total_chunks {
                let mut excess = assigned - lane_total_chunks;
                for alloc in dataspace_allocations[idx..end].iter_mut().rev() {
                    if excess == 0 {
                        break;
                    }
                    if alloc.total_chunks > 0 {
                        alloc.total_chunks -= 1;
                        excess -= 1;
                    }
                }
            }

            idx = end;
        }

        // Scale chunk counts to maintain proportionality with actual chunk size rounding.
        Ok((lane_allocations, dataspace_allocations))
    }

    #[inline]
    fn mask_includes(mask: u64, idx: u32) -> bool {
        idx < 64 && (mask & (1u64 << idx)) != 0
    }

    fn should_drop_rbc_for(&self, validator_idx: u32) -> bool {
        Self::mask_includes(self.config.debug_rbc_drop_validator_mask, validator_idx)
    }

    fn should_equivocate_rbc_chunk(&self, validator_idx: u32, chunk_idx: u32) -> bool {
        Self::mask_includes(
            self.config.debug_rbc_equivocate_validator_mask,
            validator_idx,
        ) && chunk_idx < 64
            && Self::mask_includes(self.config.debug_rbc_equivocate_chunk_mask, chunk_idx)
    }

    fn should_withhold_chunk(&self, chunk_idx: u32) -> bool {
        chunk_idx < 64 && Self::mask_includes(self.config.debug_rbc_partial_chunk_mask, chunk_idx)
    }

    pub(super) fn should_fork_ready(&self, validator_idx: u32) -> bool {
        Self::mask_includes(self.config.debug_rbc_conflicting_ready_mask, validator_idx)
    }

    fn mutate_chunk_for_equivocation(bytes: &mut Vec<u8>) {
        if let Some(first) = bytes.first_mut() {
            *first ^= 0xFF;
        } else {
            bytes.push(0xFF);
        }
    }

    pub(super) fn fork_ready_message(mut ready: RbcReady) -> RbcReady {
        if let Some(first) = ready.signature.first_mut() {
            *first ^= 0xA5;
        } else {
            ready.signature.push(0xA5);
        }
        ready
    }

    pub(super) fn schedule_rbc_chunk_broadcast(
        &mut self,
        chunk: crate::sumeragi::consensus::RbcChunk,
    ) {
        let key = Self::session_key(&chunk.block_hash, chunk.height, chunk.view);
        let roster = self.rbc_session_roster(key);
        if roster.is_empty() {
            return;
        }
        let local_peer_id = self.common_config.peer.id().clone();

        for (idx, peer_id) in roster.iter().enumerate() {
            if peer_id == &local_peer_id {
                continue;
            }
            let Ok(validator_idx) = u32::try_from(idx) else {
                continue;
            };

            if self.should_drop_rbc_for(validator_idx) {
                debug!(
                    height = chunk.height,
                    view = chunk.view,
                    chunk_idx = chunk.idx,
                    validator_idx,
                    "dropping RBC chunk due to debug mask"
                );
                continue;
            }

            let mut message_chunk = chunk.clone();
            if self.should_equivocate_rbc_chunk(validator_idx, message_chunk.idx) {
                Self::mutate_chunk_for_equivocation(&mut message_chunk.bytes);
                debug!(
                    height = message_chunk.height,
                    view = message_chunk.view,
                    chunk_idx = message_chunk.idx,
                    validator_idx,
                    "equivocating RBC chunk due to debug mask"
                );
            }

            self.schedule_background(BackgroundRequest::Post {
                peer: peer_id.clone(),
                msg: BlockMessage::RbcChunk(message_chunk),
            });
        }
    }

    pub(super) fn install_rbc_session_plan(&mut self, plan: &RbcSessionPlan) -> Result<()> {
        let key = plan.key;
        if self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .insert(key, plan.session.clone())
            .is_some()
        {
            debug!(
                height = key.1,
                view = key.2,
                "overwriting existing RBC session entry"
            );
        }
        self.update_rbc_status_entry(key, &plan.session, false);
        self.persist_rbc_session(key, &plan.session);

        let roster = self.rbc_roster_for_session(key);
        self.record_rbc_session_roster(key, roster);
        Ok(())
    }

    pub(super) fn broadcast_rbc_session_plan(&mut self, plan: RbcSessionPlan) -> Result<()> {
        let key = plan.key;
        let topology_peers = self.rbc_session_roster(key);
        if topology_peers.is_empty() {
            return Ok(());
        }
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in &topology_peers {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessage::RbcInit(plan.init),
            });
        }
        for chunk in plan.chunks {
            self.schedule_rbc_chunk_broadcast(chunk);
        }
        self.maybe_emit_rbc_ready(key)?;
        Ok(())
    }

    pub(super) fn build_rbc_session_from_payload(
        payload_bytes: &[u8],
        payload_hash: Hash,
        chunk_size: usize,
        epoch: u64,
    ) -> Result<RbcSession> {
        let chunk_bytes = chunk_payload_bytes(payload_bytes, chunk_size);
        if chunk_bytes.is_empty() {
            return Err(eyre!("cannot seed RBC session from empty payload"));
        }

        let total_chunks = u32::try_from(chunk_bytes.len())
            .map_err(|_| eyre!("RBC payload requires {} chunks", chunk_bytes.len()))?;
        if total_chunks > RBC_MAX_TOTAL_CHUNKS {
            return Err(eyre!(
                "RBC payload requires {total_chunks} chunks, exceeding cap {RBC_MAX_TOTAL_CHUNKS}"
            ));
        }

        let mut digests = Vec::with_capacity(chunk_bytes.len());
        for chunk in &chunk_bytes {
            let digest = Sha256::digest(chunk);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&digest);
            digests.push(arr);
        }
        let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests)
            .root()
            .ok_or_else(|| eyre!("failed to compute chunk root"))?;

        let mut session = RbcSession::new(
            total_chunks,
            Some(payload_hash),
            Some(Hash::from(chunk_root)),
            epoch,
        )
        .map_err(|err| eyre!(err))?;

        for (idx, chunk) in chunk_bytes.into_iter().enumerate() {
            let idx =
                u32::try_from(idx).map_err(|_| eyre!("RBC chunk index {idx} exceeds u32 range"))?;
            session.ingest_chunk(idx, chunk, None);
        }

        Ok(session)
    }

    pub(super) fn seed_rbc_session_from_block(
        &mut self,
        key: SessionKey,
        block: &SignedBlock,
        payload_hash: Hash,
    ) -> Result<()> {
        if self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            return Ok(());
        }

        let payload_bytes = block_payload_bytes(block);
        let session = match Self::build_rbc_session_from_payload(
            &payload_bytes,
            payload_hash,
            self.config.rbc_chunk_max_bytes,
            self.epoch_for_height(key.1),
        ) {
            Ok(session) => session,
            Err(err) => {
                warn!(
                    height = key.1,
                    view = key.2,
                    error = %err,
                    "failed to seed RBC session from BlockCreated payload"
                );
                return Ok(());
            }
        };

        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        let roster = self.rbc_roster_for_session(key);
        self.record_rbc_session_roster(key, roster);
        self.flush_pending_rbc(key)?;
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
        }
        self.publish_rbc_backlog_snapshot();
        self.maybe_emit_rbc_ready(key)?;
        Ok(())
    }

    /// When a full block payload is available alongside a `BlockCreated` message, ingest it into
    /// the cached RBC session so availability tracking can proceed once RBC evidence arrives (READY/DELIVER).
    pub(super) fn hydrate_rbc_session_from_block(
        &mut self,
        key: SessionKey,
        payload_bytes: &[u8],
        payload_hash: Hash,
    ) -> Result<()> {
        let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) else {
            return Ok(());
        };
        if session.delivered_payload_matches(&payload_hash) {
            // Restore the session before returning so future RBC traffic can reuse it.
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }

        let outcome = apply_hydrated_payload(
            &mut session,
            payload_bytes,
            payload_hash,
            self.config.rbc_chunk_max_bytes,
        );

        if outcome.payload_hash_mismatch {
            warn!(
                height = key.1,
                view = key.2,
                expected = ?session.payload_hash(),
                observed = ?payload_hash,
                "hydrated payload hash mismatches RBC session; marking invalid"
            );
        }
        if outcome.chunk_root_mismatch {
            let expected_root = session.expected_chunk_root;
            let computed_root = session.chunk_root();
            warn!(
                height = key.1,
                view = key.2,
                ?expected_root,
                ?computed_root,
                "hydrated payload chunk-root mismatches INIT; marking invalid"
            );
        }
        if outcome.layout_mismatch {
            warn!(
                height = key.1,
                view = key.2,
                expected_chunks = session.total_chunks(),
                observed_chunks = outcome.observed_chunks.unwrap_or_default(),
                "hydrated payload chunk layout mismatches INIT; marking invalid"
            );
        }

        let invalidated = session.is_invalid();
        let should_update_status = outcome.updated
            || outcome.payload_hash_mismatch
            || outcome.chunk_root_mismatch
            || outcome.layout_mismatch
            || invalidated;
        if should_update_status {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
        }
        if invalidated {
            self.clear_pending_rbc(&key);
        }
        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        if should_update_status {
            self.publish_rbc_backlog_snapshot();
        }
        if !invalidated {
            self.maybe_emit_rbc_ready(key)?;
            if outcome.all_chunks_present {
                if let Err(err) = self.maybe_emit_rbc_deliver(key) {
                    debug!(
                        height = key.1,
                        view = key.2,
                        ?err,
                        "failed to emit RBC DELIVER after hydrating payload"
                    );
                }
            }
        }
        Ok(())
    }

    /// Attempt to reconstruct the full block payload for an RBC session once delivery is complete.
    /// Returns `None` if the session is incomplete or missing chunk data.
    fn rbc_session_payload_bytes(&self, key: &SessionKey) -> Option<Vec<u8>> {
        let session = self.subsystems.da_rbc.rbc.sessions.get(key)?;
        if !session.delivered || session.received_chunks() != session.total_chunks() {
            return None;
        }

        let mut payload = Vec::new();
        for idx in 0..session.total_chunks() {
            let chunk = session.chunk_bytes(idx)?;
            payload.extend_from_slice(chunk);
        }
        Some(payload)
    }

    /// Ingest a block reconstructed purely from an RBC payload when the original `BlockCreated`
    /// broadcast was missed. This keeps peers that only saw RBC traffic from stalling availability tracking.
    pub(super) fn recover_block_from_rbc_session(&mut self, key: SessionKey) {
        if self.pending.pending_blocks.contains_key(&key.0) {
            return;
        }
        if self.kura.get_block_height_by_hash(key.0).is_some() {
            return;
        }
        let Some(payload) = self.rbc_session_payload_bytes(&key) else {
            return;
        };
        let payload_hash = Hash::new(&payload);
        if let Some(expected) = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .and_then(RbcSession::payload_hash)
        {
            if expected != payload_hash {
                warn!(
                    height = key.1,
                    view = key.2,
                    expected = ?expected,
                    observed = ?payload_hash,
                    "discarding reconstructed RBC payload due to hash mismatch"
                );
                return;
            }
        }

        let cursor = payload.as_slice();
        let block = match SignedBlock::decode_all_versioned(cursor) {
            Ok(block) => block,
            Err(err) => {
                warn!(
                    ?err,
                    height = key.1,
                    view = key.2,
                    "failed to decode block from delivered RBC payload"
                );
                return;
            }
        };
        if block.hash() != key.0 {
            warn!(
                block = %block.hash(),
                expected = %key.0,
                height = key.1,
                view = key.2,
                "RBC payload block hash mismatch; rejecting reconstructed block"
            );
            return;
        }
        let header = block.header();
        let height = header.height().get();
        let view = u64::from(header.view_change_index());
        if height != key.1 || view != key.2 {
            warn!(
                block_height = height,
                block_view = view,
                session_height = key.1,
                session_view = key.2,
                "RBC payload metadata mismatches session key; rejecting reconstructed block"
            );
            return;
        }

        let block_msg = BlockCreated::from(&block);
        match self.handle_block_created(block_msg) {
            Ok(()) => {
                info!(
                    height,
                    view,
                    block = %key.0,
                    "ingested block from delivered RBC payload after missing BlockCreated"
                );
            }
            Err(err) => {
                warn!(
                    ?err,
                    height,
                    view,
                    block = %key.0,
                    "failed to ingest block reconstructed from RBC payload"
                );
            }
        }
    }

    pub(super) fn should_drop_stale_rbc_message(
        &self,
        height: crate::sumeragi::consensus::Height,
        view: crate::sumeragi::consensus::View,
        block_hash: &HashOf<BlockHeader>,
        kind: &'static str,
    ) -> bool {
        let Some(local_view) = self.stale_view(height, view) else {
            return false;
        };
        let key = Self::session_key(block_hash, height, view);
        if self.subsystems.da_rbc.rbc.sessions.contains_key(&key)
            || self.block_known_locally(*block_hash)
        {
            debug!(
                height,
                view,
                local_view,
                block = %block_hash,
                kind,
                "accepting RBC message for stale view on known block"
            );
            return false;
        }
        if self.runtime_da_enabled() {
            debug!(
                height,
                view,
                local_view,
                block = %block_hash,
                kind,
                "accepting RBC message for stale view while DA is enabled"
            );
            return false;
        }
        debug!(
            height,
            view,
            local_view,
            block = %block_hash,
            kind,
            "dropping RBC message for stale view"
        );
        true
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_rbc_init(&mut self, init: RbcInit) -> Result<()> {
        if self.should_drop_stale_rbc_message(init.height, init.view, &init.block_hash, "RbcInit") {
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(init.height);
        if init.epoch != expected_epoch {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                expected = expected_epoch,
                observed = init.epoch,
                "dropping RBC INIT with mismatched epoch"
            );
            return Ok(());
        }
        if self.rbc_message_stale(&init.block_hash, init.height) {
            debug!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                "dropping RBC INIT for committed block"
            );
            return Ok(());
        }
        let key = Self::session_key(&init.block_hash, init.height, init.view);
        if init.total_chunks == 0 {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                "rejecting RBC init with zero total chunks"
            );
            return Ok(());
        }
        if init.total_chunks > RBC_MAX_TOTAL_CHUNKS {
            warn!(
                height = init.height,
                view = init.view,
                total_chunks = init.total_chunks,
                max_chunks = RBC_MAX_TOTAL_CHUNKS,
                "rejecting RBC init that exceeds chunk cap"
            );
            return Ok(());
        }
        let roster = self.rbc_roster_for_session(key);
        self.record_rbc_session_roster(key, roster);
        if let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) {
            if let Some(expected_hash) = session.payload_hash() {
                if expected_hash != init.payload_hash {
                    warn!(
                        ?expected_hash,
                        observed = ?init.payload_hash,
                        height = init.height,
                        view = init.view,
                        "RBC INIT payload hash mismatches seeded session"
                    );
                    session.invalid = true;
                }
            } else {
                session.payload_hash = Some(init.payload_hash);
            }
            if session.total_chunks != init.total_chunks {
                warn!(
                    height = init.height,
                    view = init.view,
                    expected = session.total_chunks,
                    observed = init.total_chunks,
                    "RBC INIT chunk-count mismatches seeded session"
                );
                session.invalid = true;
            }
            match session.expected_chunk_root {
                Some(root) if root != init.chunk_root => {
                    warn!(
                        height = init.height,
                        view = init.view,
                        ?root,
                        observed = ?init.chunk_root,
                        "RBC INIT chunk-root mismatches seeded session"
                    );
                    session.invalid = true;
                }
                None => session.expected_chunk_root = Some(init.chunk_root),
                _ => {}
            }
            session.epoch = init.epoch;
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            self.flush_pending_rbc(key)?;
            self.maybe_emit_rbc_ready(key)?;
            if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                self.update_rbc_status_entry(key, &session, false);
                self.persist_rbc_session(key, &session);
            }
            self.publish_rbc_backlog_snapshot();
            self.process_commit_candidates();
            return Ok(());
        }
        let session = match RbcSession::new(
            init.total_chunks,
            Some(init.payload_hash),
            Some(init.chunk_root),
            init.epoch,
        ) {
            Ok(session) => session,
            Err(err) => {
                warn!(
                    height = init.height,
                    view = init.view,
                    total_chunks = init.total_chunks,
                    "failed to initialise RBC session: {err}"
                );
                return Ok(());
            }
        };
        if self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .insert(key, session)
            .is_some()
        {
            warn!(?key, "replacing existing RBC session on init");
        }
        // Apply any messages that arrived before INIT once the session exists.
        self.flush_pending_rbc(key)?;
        self.maybe_emit_rbc_ready(key)?;
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
        }
        self.publish_rbc_backlog_snapshot();
        self.process_commit_candidates();
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_rbc_chunk(&mut self, chunk: RbcChunk) -> Result<()> {
        if self.should_drop_stale_rbc_message(
            chunk.height,
            chunk.view,
            &chunk.block_hash,
            "RbcChunk",
        ) {
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(chunk.height);
        if chunk.epoch != expected_epoch {
            warn!(
                height = chunk.height,
                view = chunk.view,
                block = %chunk.block_hash,
                expected = expected_epoch,
                observed = chunk.epoch,
                "dropping RBC chunk with mismatched epoch"
            );
            return Ok(());
        }
        let max_chunk_bytes = self.config.rbc_chunk_max_bytes.max(1);
        if chunk.bytes.len() > max_chunk_bytes {
            warn!(
                height = chunk.height,
                view = chunk.view,
                idx = chunk.idx,
                chunk_len = chunk.bytes.len(),
                max = max_chunk_bytes,
                "dropping RBC chunk that exceeds configured size cap"
            );
            return Ok(());
        }
        if self.rbc_message_stale(&chunk.block_hash, chunk.height) {
            debug!(
                height = chunk.height,
                view = chunk.view,
                block = %chunk.block_hash,
                "dropping RBC chunk for committed block"
            );
            return Ok(());
        }
        let key = Self::session_key(&chunk.block_hash, chunk.height, chunk.view);
        let (ready_sent_before, became_complete) =
            if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get_mut(&key) {
                let ready_sent_before = session.sent_ready;
                let was_complete = session.total_chunks() != 0
                    && session.received_chunks() == session.total_chunks();
                session.ingest_chunk(chunk.idx, chunk.bytes, None);
                let is_complete = session.total_chunks() != 0
                    && session.received_chunks() == session.total_chunks();
                (ready_sent_before, !was_complete && is_complete)
            } else {
                let (max_chunks, max_bytes) = self.pending_rbc_caps();
                let outcome = {
                    let pending = self.pending_rbc_slot(key);
                    pending.push_chunk_capped(chunk, max_chunks, max_bytes, Instant::now())
                };
                match outcome {
                    PendingChunkOutcome::Inserted {
                        pending_chunks,
                        pending_bytes,
                        evicted_chunks,
                        evicted_bytes,
                    } => {
                        if evicted_chunks > 0 {
                            Self::record_pending_drop_counts(
                                self.telemetry_handle(),
                                PendingRbcDropReason::Cap,
                                evicted_chunks,
                                evicted_bytes,
                            );
                        }
                        debug!(
                            ?key,
                            pending_chunks,
                            pending_bytes,
                            evicted_chunks,
                            evicted_bytes,
                            "stashed RBC chunk until INIT arrives"
                        );
                    }
                    PendingChunkOutcome::Dropped {
                        dropped_bytes,
                        evicted_chunks,
                        evicted_bytes,
                    } => {
                        let dropped_chunks = evicted_chunks.saturating_add(1);
                        let dropped_bytes_total = dropped_bytes.saturating_add(evicted_bytes);
                        Self::record_pending_drop_counts(
                            self.telemetry_handle(),
                            PendingRbcDropReason::Cap,
                            dropped_chunks,
                            dropped_bytes_total,
                        );
                        warn!(
                            ?key,
                            max_chunks,
                            max_bytes,
                            dropped_bytes,
                            evicted_chunks,
                            evicted_bytes,
                            "dropping pending RBC chunk: stash limits exceeded before INIT"
                        );
                    }
                }
                self.publish_rbc_backlog_snapshot();
                return Ok(());
            };
        self.maybe_emit_rbc_ready(key)?;
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
            if session.total_chunks() != 0
                && session.received_chunks() == session.total_chunks()
                && self.subsystems.da_rbc.rbc.pending.contains_key(&key)
            {
                self.flush_pending_rbc(key)?;
            }
        }
        self.publish_rbc_backlog_snapshot();
        let ready_sent_after = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .map_or(ready_sent_before, |session| session.sent_ready);
        if became_complete || (!ready_sent_before && ready_sent_after) {
            self.process_commit_candidates();
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_rbc_ready(&mut self, ready: RbcReady) -> Result<()> {
        if self.should_drop_stale_rbc_message(
            ready.height,
            ready.view,
            &ready.block_hash,
            "RbcReady",
        ) {
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(ready.height);
        if ready.epoch != expected_epoch {
            warn!(
                height = ready.height,
                view = ready.view,
                sender = ready.sender,
                block = %ready.block_hash,
                expected = expected_epoch,
                observed = ready.epoch,
                "dropping RBC READY with mismatched epoch"
            );
            return Ok(());
        }
        if self.rbc_message_stale(&ready.block_hash, ready.height) {
            debug!(
                height = ready.height,
                view = ready.view,
                block = %ready.block_hash,
                sender = ready.sender,
                "dropping RBC READY for committed block"
            );
            return Ok(());
        }
        let key = Self::session_key(&ready.block_hash, ready.height, ready.view);
        let ready_sender = ready.sender;
        let ready_view = ready.view;
        if !self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            let max_bytes = self.pending_rbc_caps().1;
            let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                let pending = self.pending_rbc_slot(key);
                let (accepted, dropped_bytes) =
                    pending.push_ready_capped(ready, max_bytes, Instant::now());
                (
                    accepted,
                    dropped_bytes,
                    pending.pending_chunks(),
                    pending.pending_bytes(),
                )
            };
            if accepted {
                info!(
                    ?key,
                    pending_chunks,
                    pending_bytes,
                    ready_sender,
                    ready_view,
                    "stashed RBC READY until INIT arrives"
                );
            } else {
                warn!(
                    ?key,
                    max_bytes, "dropping pending RBC READY: stash limits exceeded before INIT"
                );
                Self::record_pending_drop_counts(
                    self.telemetry_handle(),
                    PendingRbcDropReason::Cap,
                    1,
                    u64::try_from(dropped_bytes).unwrap_or(u64::MAX),
                );
            }
            self.publish_rbc_backlog_snapshot();
            return Ok(());
        }
        let topology_peers = self.rbc_session_roster(key);
        if topology_peers.is_empty() {
            debug!(
                height = ready.height,
                view = ready.view,
                sender = ready.sender,
                "dropping RBC READY: empty commit topology"
            );
            return Ok(());
        }
        let topology = crate::sumeragi::network_topology::Topology::new(topology_peers);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(ready.height);
        let signature_topology =
            super::topology_for_view(&topology, ready.height, ready.view, mode_tag, prf_seed);
        let local_idx = self.local_validator_index_for_topology(&signature_topology);
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
            if let Some(existing) = session
                .ready_signatures
                .iter()
                .find(|entry| entry.sender == ready.sender)
            {
                if existing.signature == ready.signature {
                    trace!(
                        height = ready.height,
                        view = ready.view,
                        sender = ready.sender,
                        "ignoring duplicate RBC READY with identical signature"
                    );
                    return Ok(());
                }
            }
        }
        if !rbc_ready_signature_valid(
            &ready,
            &signature_topology,
            &self.common_config.chain,
            mode_tag,
        ) {
            let outcome = self.record_invalid_signature(
                InvalidSigKind::RbcReady,
                ready.height,
                ready.view,
                u64::from(ready.sender),
            );
            if matches!(outcome, InvalidSigOutcome::Logged) {
                warn!(
                    height = ready.height,
                    view = ready.view,
                    sender = ready.sender,
                    topology_len = signature_topology.as_ref().len(),
                    "dropping RBC READY with invalid signature"
                );
            } else {
                debug!(
                    height = ready.height,
                    view = ready.view,
                    sender = ready.sender,
                    topology_len = signature_topology.as_ref().len(),
                    "suppressing repeated invalid RBC READY signature log"
                );
            }
            return Ok(());
        }
        if let Some(expected) = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .and_then(|session| session.expected_chunk_root)
        {
            if expected != ready.chunk_root {
                warn!(
                    height = ready.height,
                    view = ready.view,
                    sender = ready.sender,
                    ?expected,
                    observed = ?ready.chunk_root,
                    "dropping RBC READY with mismatched chunk root"
                );
                return Ok(());
            }
        }
        let mut clear_pending = false;
        let mut conflict_detected = false;
        let ready_count_before;
        let ready_count_after;
        let ready_senders_after: Vec<u32>;
        let recorded_ready = {
            let session = self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .get_mut(&key)
                .expect("session presence checked before validation");
            ready_count_before = session.ready_signatures.len();
            let was_valid = !session.is_invalid();
            let recorded_ready = session.record_ready(ready.sender, ready.signature.clone());
            ready_count_after = session.ready_signatures.len();
            ready_senders_after = session
                .ready_signatures
                .iter()
                .map(|entry| entry.sender)
                .collect();
            if was_valid && session.is_invalid() {
                conflict_detected = true;
            }
            if session.is_invalid() {
                clear_pending = true;
            }
            recorded_ready
        };
        if clear_pending {
            self.clear_pending_rbc(&key);
        }
        if conflict_detected {
            warn!(
                height = key.1,
                view = key.2,
                sender = ready_sender,
                "RBC READY conflict detected; marking session invalid"
            );
        }
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
        }
        if self.subsystems.da_rbc.rbc.pending.contains_key(&key) {
            self.flush_pending_rbc(key)?;
        }
        let delivered_before = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .is_some_and(|session| session.delivered);
        self.maybe_emit_rbc_deliver(key)?;
        let delivered_after = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .is_some_and(|session| session.delivered);
        let deliver_emitted = !delivered_before && delivered_after;
        let telemetry_ref = self.telemetry_handle();
        self.publish_rbc_backlog_snapshot();
        if recorded_ready {
            if let Some(telemetry) = telemetry_ref {
                telemetry.inc_rbc_ready_broadcasts();
            }
            iroha_logger::info!(
                height = key.1,
                view = key.2,
                sender = ready_sender,
                ready_before = ready_count_before,
                ready_after = ready_count_after,
                senders = ?ready_senders_after,
                "accepted RBC READY"
            );
        } else if ready_count_before != ready_count_after {
            iroha_logger::info!(
                height = key.1,
                view = key.2,
                sender = ready_sender,
                ready_before = ready_count_before,
                ready_after = ready_count_after,
                senders = ?ready_senders_after,
                "processed RBC READY without recording (invalid session)"
            );
        } else {
            let session_invalid = self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .get(&key)
                .is_some_and(super::RbcSession::is_invalid);
            iroha_logger::info!(
                height = key.1,
                view = key.2,
                sender = ready_sender,
                ready_before = ready_count_before,
                ready_after = ready_count_after,
                senders = ?ready_senders_after,
                invalid_session = session_invalid,
                conflict_detected,
                "ignored duplicate or invalid RBC READY"
            );
        }
        if recorded_ready && !clear_pending && local_idx != Some(ready_sender) {
            let gossip_targets =
                self.new_view_gossip_targets(signature_topology.as_ref(), Some(ready_sender));
            if !gossip_targets.is_empty() {
                let msg = BlockMessage::RbcReady(ready.clone());
                for peer in gossip_targets {
                    self.schedule_background(BackgroundRequest::Post {
                        peer,
                        msg: msg.clone(),
                    });
                }
            }
        }
        if should_process_commit_after_ready(recorded_ready, clear_pending, deliver_emitted) {
            self.process_commit_candidates();
        }
        Ok(())
    }

    fn evaluate_rbc_deliver_outcome(
        deliver_quorum: usize,
        session: &mut RbcSession,
        key: SessionKey,
        deliver: &crate::sumeragi::consensus::RbcDeliver,
    ) -> (bool, bool, Option<u64>, bool, Option<String>) {
        let mut ignored = false;
        let mut first_deliver = false;
        let mut delivered_bytes: Option<u64> = None;
        let mut invalidate = false;
        let mut defer_reason: Option<String> = None;
        if session.is_invalid() {
            ignored = true;
            warn!(
                height = key.1,
                view = key.2,
                sender = deliver.sender,
                "ignoring RBC DELIVER due to invalid session state"
            );
            return (
                ignored,
                first_deliver,
                delivered_bytes,
                invalidate,
                defer_reason,
            );
        }
        if session.delivered {
            ignored = true;
            if session.deliver_sender == Some(deliver.sender)
                && session
                    .deliver_signature
                    .as_deref()
                    .is_some_and(|sig| sig != deliver.signature.as_slice())
            {
                session.invalid = true;
                invalidate = true;
                warn!(
                    height = key.1,
                    view = key.2,
                    sender = deliver.sender,
                    "conflicting RBC DELIVER signature detected; marking session invalid"
                );
            }
            return (
                ignored,
                first_deliver,
                delivered_bytes,
                invalidate,
                defer_reason,
            );
        }
        if let Some(expected) = session.expected_chunk_root {
            if expected != deliver.chunk_root {
                session.invalid = true;
                invalidate = true;
                ignored = true;
                warn!(
                    height = key.1,
                    view = key.2,
                    sender = deliver.sender,
                    ?expected,
                    observed = ?deliver.chunk_root,
                    "ignoring RBC DELIVER due to chunk-root mismatch"
                );
            }
        }
        if !session.is_invalid() {
            match evaluate_deliver_acceptance(session, deliver_quorum) {
                DeliverAcceptance::DeferReady {
                    ready_count,
                    required,
                } => {
                    status::inc_rbc_deliver_defer_ready();
                    defer_reason = Some(format!(
                        "deferring RBC DELIVER until READY quorum is satisfied (ready_count={ready_count}, required={required})"
                    ));
                }
                DeliverAcceptance::DeferChunks { received, total } => {
                    status::inc_rbc_deliver_defer_chunks();
                    defer_reason = Some(format!(
                        "deferring RBC DELIVER until all chunks are present (received={received}, total={total})"
                    ));
                }
                DeliverAcceptance::InvalidChunkRoot => {
                    session.invalid = true;
                    invalidate = true;
                    ignored = true;
                    warn!(
                        height = key.1,
                        view = key.2,
                        sender = deliver.sender,
                        "ignoring RBC DELIVER: chunk root mismatch detected"
                    );
                }
                DeliverAcceptance::Accept => {
                    first_deliver =
                        session.record_deliver(deliver.sender, deliver.signature.clone());
                    if first_deliver {
                        delivered_bytes = session.delivered_payload_bytes();
                    }
                }
            }
        }
        if session.is_invalid() {
            invalidate = true;
        }
        (
            ignored,
            first_deliver,
            delivered_bytes,
            invalidate,
            defer_reason,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_rbc_deliver(&mut self, deliver: RbcDeliver) -> Result<()> {
        if self.should_drop_stale_rbc_message(
            deliver.height,
            deliver.view,
            &deliver.block_hash,
            "RbcDeliver",
        ) {
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(deliver.height);
        if deliver.epoch != expected_epoch {
            warn!(
                height = deliver.height,
                view = deliver.view,
                sender = deliver.sender,
                block = %deliver.block_hash,
                expected = expected_epoch,
                observed = deliver.epoch,
                "dropping RBC DELIVER with mismatched epoch"
            );
            return Ok(());
        }
        if self.rbc_message_stale(&deliver.block_hash, deliver.height) {
            debug!(
                height = deliver.height,
                view = deliver.view,
                block = %deliver.block_hash,
                sender = deliver.sender,
                "dropping RBC DELIVER for committed block"
            );
            return Ok(());
        }
        let key = Self::session_key(&deliver.block_hash, deliver.height, deliver.view);
        if !self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            let max_bytes = self.pending_rbc_caps().1;
            let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                let pending = self.pending_rbc_slot(key);
                let (accepted, dropped_bytes) =
                    pending.push_deliver_capped(deliver, max_bytes, Instant::now());
                (
                    accepted,
                    dropped_bytes,
                    pending.pending_chunks(),
                    pending.pending_bytes(),
                )
            };
            if accepted {
                debug!(
                    ?key,
                    pending_chunks, pending_bytes, "stashed RBC DELIVER until INIT arrives"
                );
            } else {
                warn!(
                    ?key,
                    max_bytes, "dropping pending RBC DELIVER: stash limits exceeded before INIT"
                );
                Self::record_pending_drop_counts(
                    self.telemetry_handle(),
                    PendingRbcDropReason::Cap,
                    1,
                    u64::try_from(dropped_bytes).unwrap_or(u64::MAX),
                );
            }
            self.publish_rbc_backlog_snapshot();
            return Ok(());
        }
        let topology_peers = self.rbc_session_roster(key);
        if topology_peers.is_empty() {
            debug!(
                height = deliver.height,
                view = deliver.view,
                sender = deliver.sender,
                "dropping RBC DELIVER: empty commit topology"
            );
            return Ok(());
        }
        let topology = crate::sumeragi::network_topology::Topology::new(topology_peers);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(deliver.height);
        let signature_topology =
            super::topology_for_view(&topology, deliver.height, deliver.view, mode_tag, prf_seed);
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
            if session.delivered
                && session.deliver_sender == Some(deliver.sender)
                && session
                    .deliver_signature
                    .as_deref()
                    .is_some_and(|sig| sig == deliver.signature.as_slice())
            {
                trace!(
                    height = deliver.height,
                    view = deliver.view,
                    sender = deliver.sender,
                    "ignoring duplicate RBC DELIVER with identical signature"
                );
                return Ok(());
            }
        }
        if !rbc_deliver_signature_valid(
            &deliver,
            &signature_topology,
            &self.common_config.chain,
            mode_tag,
        ) {
            let outcome = self.record_invalid_signature(
                InvalidSigKind::RbcDeliver,
                deliver.height,
                deliver.view,
                u64::from(deliver.sender),
            );
            if matches!(outcome, InvalidSigOutcome::Logged) {
                warn!(
                    height = deliver.height,
                    view = deliver.view,
                    sender = deliver.sender,
                    "dropping RBC DELIVER with invalid signature"
                );
            } else {
                debug!(
                    height = deliver.height,
                    view = deliver.view,
                    sender = deliver.sender,
                    "suppressing repeated invalid RBC DELIVER signature log"
                );
            }
            return Ok(());
        }
        let deliver_quorum = self.rbc_deliver_quorum(&topology);
        let (ignored, first_deliver, delivered_bytes, invalidate, defer_reason) = {
            let session = self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .get_mut(&key)
                .expect("session presence checked before validation");
            Self::evaluate_rbc_deliver_outcome(deliver_quorum, session, key, &deliver)
        };
        if let Some(reason) = defer_reason {
            let sender = deliver.sender;
            let max_bytes = self.pending_rbc_caps().1;
            let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                let pending = self.pending_rbc_slot(key);
                let (accepted, dropped_bytes) =
                    pending.push_deliver_capped(deliver, max_bytes, Instant::now());
                (
                    accepted,
                    dropped_bytes,
                    pending.pending_chunks(),
                    pending.pending_bytes(),
                )
            };
            if accepted {
                debug!(
                    height = key.1,
                    view = key.2,
                    sender,
                    reason = reason.as_str(),
                    pending_chunks,
                    pending_bytes,
                    "deferring RBC DELIVER"
                );
            } else {
                warn!(
                    height = key.1,
                    view = key.2,
                    sender,
                    max_bytes,
                    "dropping pending RBC DELIVER: stash limits exceeded while deferring"
                );
                Self::record_pending_drop_counts(
                    self.telemetry_handle(),
                    PendingRbcDropReason::Cap,
                    1,
                    u64::try_from(dropped_bytes).unwrap_or(u64::MAX),
                );
            }
            self.publish_rbc_backlog_snapshot();
            return Ok(());
        }
        if invalidate {
            self.clear_pending_rbc(&key);
        }
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
        }
        let telemetry_ref = self.telemetry_handle();
        self.publish_rbc_backlog_snapshot();
        if ignored {
            return Ok(());
        }
        if first_deliver {
            if let Some(telemetry) = telemetry_ref {
                telemetry.inc_rbc_deliver_broadcasts();
                if let Some(bytes) = delivered_bytes {
                    telemetry.add_rbc_payload_bytes_delivered(bytes);
                }
            }
        }
        if self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            self.record_phase_sample(PipelinePhase::Commit, deliver.height, deliver.view);
        }
        // If the block header never arrived (missed `BlockCreated`), rebuild it from the delivered
        // RBC payload so availability tracking and votes can proceed.
        self.recover_block_from_rbc_session(key);
        if should_process_commit_after_deliver(first_deliver) {
            self.process_commit_candidates();
        }
        Ok(())
    }

    pub(super) fn ensure_rbc_chunk_store(&mut self) -> bool {
        if self.subsystems.da_rbc.rbc.chunk_store.is_none() {
            let cfg = match self.subsystems.da_rbc.rbc.store_cfg.as_ref() {
                Some(cfg) => cfg.clone(),
                None => return false,
            };
            if cfg.max_sessions == 0 || cfg.max_bytes == 0 {
                return false;
            }

            match ChunkStore::new(
                cfg.dir.clone(),
                cfg.ttl,
                cfg.soft_sessions,
                cfg.soft_bytes,
                cfg.max_sessions,
                cfg.max_bytes,
            ) {
                Ok(store) => {
                    debug!(
                        path = ?cfg.dir,
                        ttl = cfg.ttl.as_secs(),
                        max_sessions = cfg.max_sessions,
                        max_bytes = cfg.max_bytes,
                        "initialised RBC chunk store using deferred configuration"
                    );
                    self.subsystems.da_rbc.rbc.status_handle.configure(Some(
                        rbc_status::StoreConfig {
                            dir: cfg.dir.clone(),
                            ttl: cfg.ttl,
                            capacity: cfg.max_sessions,
                        },
                    ));

                    let mut last_pressure = StorePressure::Normal {
                        sessions: 0,
                        bytes: 0,
                    };
                    let mut removed_acc = Vec::new();
                    for (&session_key, session) in &self.subsystems.da_rbc.rbc.sessions {
                        let session_roster = self.rbc_session_roster(session_key);
                        match store.persist_session(
                            session_key,
                            session,
                            &self.chain_hash,
                            &self.subsystems.da_rbc.rbc.manifest,
                            session_roster.as_slice(),
                        ) {
                            Ok(outcome) => {
                                if !outcome.removed.is_empty() {
                                    removed_acc.extend(outcome.removed);
                                }
                                last_pressure = outcome.pressure;
                            }
                            Err(err) => {
                                warn!(
                                    ?err,
                                    ?session_key,
                                    "failed to persist in-memory RBC session during store recovery"
                                );
                            }
                        }
                    }

                    if !removed_acc.is_empty() {
                        self.handle_rbc_store_evictions(&removed_acc);
                    }
                    self.update_rbc_store_pressure(last_pressure);
                    self.publish_rbc_backlog_snapshot();
                    self.subsystems.da_rbc.rbc.chunk_store = Some(store);
                }
                Err(err) => {
                    trace!(
                        ?err,
                        path = ?cfg.dir,
                        "failed to initialise RBC chunk store from configuration"
                    );
                    return false;
                }
            }
        }

        self.subsystems.da_rbc.rbc.chunk_store.is_some()
    }

    pub(super) fn attach_rbc_persist_worker(&mut self) -> Option<std::thread::JoinHandle<()>> {
        if self.subsystems.da_rbc.rbc.persist_tx.is_some() {
            return None;
        }
        let cfg = self.subsystems.da_rbc.rbc.store_cfg.clone()?;
        if cfg.max_sessions == 0 || cfg.max_bytes == 0 {
            return None;
        }
        match spawn_rbc_persist_worker(cfg) {
            Ok(handle) => {
                self.subsystems.da_rbc.rbc.persist_tx = Some(handle.work_tx);
                self.subsystems.da_rbc.rbc.persist_rx = Some(handle.result_rx);
                self.subsystems.da_rbc.rbc.persist_inflight.clear();
                Some(handle.join_handle)
            }
            Err(err) => {
                warn!(?err, "failed to spawn RBC persist worker thread");
                None
            }
        }
    }

    pub(super) fn poll_rbc_persist_results(&mut self) -> bool {
        let Some(rx) = self.subsystems.da_rbc.rbc.persist_rx.as_ref() else {
            return false;
        };
        let mut drained = Vec::new();
        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(result) => drained.push(result),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected {
            self.subsystems.da_rbc.rbc.persist_tx = None;
            self.subsystems.da_rbc.rbc.persist_rx = None;
            self.subsystems.da_rbc.rbc.persist_inflight.clear();
        }
        if drained.is_empty() {
            return false;
        }
        for result in drained {
            self.subsystems.da_rbc.rbc.persist_inflight.remove(&result.key);
            match result.outcome {
                Ok(outcome) => {
                    self.handle_rbc_store_evictions(&outcome.removed);
                    self.update_rbc_store_pressure(outcome.pressure);
                    if self.subsystems.da_rbc.rbc.sessions.contains_key(&result.key) {
                        self.subsystems
                            .da_rbc
                            .rbc
                            .persisted_full_sessions
                            .insert(result.key);
                    }
                }
                Err(err) => {
                    warn!(?err, ?result.key, "failed to persist RBC session snapshot");
                }
            }
        }
        true
    }

    pub(super) fn persist_rbc_session(&mut self, key: SessionKey, session: &RbcSession) {
        let total_chunks = session.total_chunks();
        if total_chunks == 0 {
            return;
        }
        if session.received_chunks() < total_chunks {
            return;
        }
        if self
            .subsystems
            .da_rbc
            .rbc
            .persisted_full_sessions
            .contains(&key)
        {
            return;
        }
        if self
            .subsystems
            .da_rbc
            .rbc
            .persist_inflight
            .contains(&key)
        {
            return;
        }
        if let Some(tx) = self.subsystems.da_rbc.rbc.persist_tx.as_ref() {
            let session_roster = self.rbc_session_roster(key);
            let persisted = session.to_persisted(
                key,
                self.chain_hash,
                &self.subsystems.da_rbc.rbc.manifest,
                session_roster.as_slice(),
            );
            match tx.try_send(RbcPersistWork { key, persisted }) {
                Ok(()) => {
                    self.subsystems.da_rbc.rbc.persist_inflight.insert(key);
                }
                Err(mpsc::TrySendError::Full(_work)) => {
                    debug!(?key, "RBC persist queue full; deferring session persistence");
                }
                Err(mpsc::TrySendError::Disconnected(_work)) => {
                    warn!(
                        ?key,
                        "RBC persist worker disconnected; falling back to sync persistence"
                    );
                    self.subsystems.da_rbc.rbc.persist_tx = None;
                    self.subsystems.da_rbc.rbc.persist_inflight.clear();
                }
            }
            if self.subsystems.da_rbc.rbc.persist_tx.is_some() {
                return;
            }
        }
        if !self.ensure_rbc_chunk_store() {
            trace!(
                ?key,
                "RBC chunk store unavailable; skipping persistence for session"
            );
            return;
        }

        let store = self
            .subsystems
            .da_rbc
            .rbc
            .chunk_store
            .as_ref()
            .expect("chunk store should be initialised");

        let session_roster = self.rbc_session_roster(key);
        match store.persist_session(
            key,
            session,
            &self.chain_hash,
            &self.subsystems.da_rbc.rbc.manifest,
            session_roster.as_slice(),
        ) {
            Ok(outcome) => {
                self.handle_rbc_store_evictions(&outcome.removed);
                self.update_rbc_store_pressure(outcome.pressure);
                self.subsystems
                    .da_rbc
                    .rbc
                    .persisted_full_sessions
                    .insert(key);
            }
            Err(err) => {
                warn!(?err, "failed to persist RBC session snapshot");
            }
        }
    }

    pub(super) fn handle_rbc_store_evictions(&mut self, removed: &[SessionKey]) {
        if removed.is_empty() {
            return;
        }
        for key in removed {
            let existed = self.subsystems.da_rbc.rbc.sessions.remove(key).is_some();
            self.subsystems.da_rbc.rbc.pending.remove(key);
            self.clear_rbc_session_roster(key);
            self.subsystems.da_rbc.rbc.status_handle.remove(key);
            self.subsystems
                .da_rbc
                .rbc
                .payload_rebroadcast_last_sent
                .remove(key);
            self.subsystems
                .da_rbc
                .rbc
                .ready_rebroadcast_last_sent
                .remove(key);
            self.subsystems
                .da_rbc
                .rbc
                .persisted_full_sessions
                .remove(key);
            self.subsystems.da_rbc.rbc.persist_inflight.remove(key);
            if existed {
                debug!(
                    block_hash = ?key.0,
                    height = key.1,
                    view = key.2,
                    "evicted RBC session due to store limits"
                );
            }
        }
        status::record_rbc_store_evictions(removed);
        #[cfg(feature = "telemetry")]
        self.telemetry.inc_rbc_store_evictions(removed.len() as u64);
        self.publish_rbc_backlog_snapshot();
    }

    pub(super) fn update_rbc_store_pressure(&self, pressure: StorePressure) {
        let level = match pressure {
            StorePressure::HardLimit { .. } => 2,
            StorePressure::SoftLimit { .. } => 1,
            StorePressure::Normal { .. } => 0,
        };
        status::set_rbc_store_pressure(pressure.sessions() as u64, pressure.bytes() as u64, level);
        if pressure.is_soft() || pressure.is_hard() {
            status::inc_rbc_store_backpressure_deferrals();
        }
        #[cfg(feature = "telemetry")]
        {
            self.telemetry.set_rbc_store_pressure(pressure);
            if pressure.is_soft() || pressure.is_hard() {
                self.telemetry.inc_rbc_store_backpressure_deferrals();
            }
        }
    }

    pub(super) fn update_rbc_status_entry(
        &self,
        key: SessionKey,
        session: &RbcSession,
        recovered: bool,
    ) {
        let ready_count = u64::try_from(session.ready_signatures.len()).unwrap_or(u64::MAX);
        let summary = rbc_status::Summary {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            total_chunks: session.total_chunks(),
            received_chunks: session.received_chunks(),
            ready_count,
            delivered: session.delivered,
            payload_hash: session.payload_hash(),
            recovered_from_disk: recovered || session.recovered_from_disk(),
            invalid: session.is_invalid(),
            lane_backlog: session.lane_backlog_entries(),
            dataspace_backlog: session.dataspace_backlog_entries(),
        };
        self.subsystems
            .da_rbc
            .rbc
            .status_handle
            .update(summary, SystemTime::now());
    }

    pub(super) fn session_key(
        block_hash: &HashOf<BlockHeader>,
        height: crate::sumeragi::consensus::Height,
        view: crate::sumeragi::consensus::View,
    ) -> SessionKey {
        (*block_hash, height, view)
    }
}

impl Actor {
    pub(super) fn publish_rbc_backlog_snapshot(&self) {
        let telemetry_ref = self.telemetry_handle();
        let caps = self.pending_rbc_caps();
        let ttl = self.config.rbc_pending_ttl;
        Self::update_rbc_backlog_snapshot(
            &self.subsystems.da_rbc.rbc.sessions,
            &self.subsystems.da_rbc.rbc.pending,
            caps,
            ttl,
            telemetry_ref,
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn update_rbc_backlog_snapshot(
        sessions: &BTreeMap<SessionKey, RbcSession>,
        pending: &BTreeMap<SessionKey, PendingRbcMessages>,
        pending_caps: (usize, usize),
        pending_ttl: Duration,
        telemetry: Option<&crate::telemetry::Telemetry>,
    ) {
        let mut total_missing = 0u64;
        let mut max_missing = 0u64;
        let mut pending_sessions = 0u64;
        let mut lane_backlog: BTreeMap<u32, status::LaneRbcSnapshot> = BTreeMap::new();
        let mut dataspace_backlog: BTreeMap<(u32, u64), status::DataspaceRbcSnapshot> =
            BTreeMap::new();
        for session in sessions.values() {
            let missing = u64::from(
                session
                    .total_chunks()
                    .saturating_sub(session.received_chunks()),
            );
            if !session.delivered {
                total_missing = total_missing.saturating_add(missing);
                if missing > max_missing {
                    max_missing = missing;
                }
                pending_sessions = pending_sessions.saturating_add(1);
            }

            for entry in session.lane_backlog_entries() {
                let accum =
                    lane_backlog
                        .entry(entry.lane_id)
                        .or_insert_with(|| status::LaneRbcSnapshot {
                            lane_id: entry.lane_id,
                            ..status::LaneRbcSnapshot::default()
                        });
                accum.tx_count = accum.tx_count.saturating_add(entry.tx_count);
                accum.total_chunks = accum.total_chunks.saturating_add(entry.total_chunks);
                accum.pending_chunks = accum.pending_chunks.saturating_add(entry.pending_chunks);
                accum.rbc_bytes_total = accum.rbc_bytes_total.saturating_add(entry.rbc_bytes_total);
            }

            for entry in session.dataspace_backlog_entries() {
                let key = (entry.lane_id, entry.dataspace_id);
                let accum =
                    dataspace_backlog
                        .entry(key)
                        .or_insert_with(|| status::DataspaceRbcSnapshot {
                            lane_id: entry.lane_id,
                            dataspace_id: entry.dataspace_id,
                            ..status::DataspaceRbcSnapshot::default()
                        });
                accum.tx_count = accum.tx_count.saturating_add(entry.tx_count);
                accum.total_chunks = accum.total_chunks.saturating_add(entry.total_chunks);
                accum.pending_chunks = accum.pending_chunks.saturating_add(entry.pending_chunks);
                accum.rbc_bytes_total = accum.rbc_bytes_total.saturating_add(entry.rbc_bytes_total);
            }
        }
        let lane_backlog_vec: Vec<_> = lane_backlog.into_values().collect();
        let dataspace_backlog_vec: Vec<_> = dataspace_backlog.into_values().collect();

        let pending_stash_sessions = u64::try_from(pending.len()).unwrap_or(u64::MAX);
        let mut pending_stash_chunks = 0u64;
        let mut pending_stash_bytes = 0u64;
        let mut pending_entries = Vec::with_capacity(pending.len());
        let now = Instant::now();
        for (key, entry) in pending {
            pending_stash_chunks = pending_stash_chunks
                .saturating_add(u64::try_from(entry.pending_chunks()).unwrap_or(u64::MAX));
            pending_stash_bytes = pending_stash_bytes
                .saturating_add(u64::try_from(entry.pending_bytes()).unwrap_or(u64::MAX));
            let (dropped_chunks, dropped_ready, dropped_deliver, dropped_bytes) =
                entry.drop_breakdown();
            pending_entries.push(status::PendingRbcEntrySnapshot {
                block_hash: key.0,
                height: key.1,
                view: key.2,
                chunks: u64::try_from(entry.pending_chunks()).unwrap_or(u64::MAX),
                bytes: u64::try_from(entry.pending_bytes()).unwrap_or(u64::MAX),
                ready: u64::try_from(entry.ready.len()).unwrap_or(u64::MAX),
                deliver: u64::try_from(entry.deliver.len()).unwrap_or(u64::MAX),
                dropped_chunks,
                dropped_bytes,
                dropped_ready,
                dropped_deliver,
                age_ms: entry.age_ms(now),
            });
        }

        status::set_rbc_backlog_snapshot(total_missing, max_missing, pending_sessions);
        let (max_pending_chunks, max_pending_bytes) = pending_caps;
        let mut pending_snapshot = status::pending_rbc_snapshot();
        pending_snapshot.sessions = pending_stash_sessions;
        pending_snapshot.session_cap = u64::try_from(PENDING_RBC_STASH_LIMIT).unwrap_or(u64::MAX);
        pending_snapshot.chunks = pending_stash_chunks;
        pending_snapshot.bytes = pending_stash_bytes;
        pending_snapshot.max_chunks_per_session =
            u64::try_from(max_pending_chunks).unwrap_or(u64::MAX);
        pending_snapshot.max_bytes_per_session =
            u64::try_from(max_pending_bytes).unwrap_or(u64::MAX);
        pending_snapshot.ttl_ms =
            u64::try_from(pending_ttl.as_millis().min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
        pending_snapshot.entries = pending_entries;
        status::set_pending_rbc_snapshot(pending_snapshot);
        status::set_rbc_lane_backlog(lane_backlog_vec.clone());
        status::set_rbc_dataspace_backlog(dataspace_backlog_vec.clone());

        if let Some(telemetry) = telemetry {
            telemetry.set_rbc_backlog(total_missing, max_missing, pending_sessions);
            telemetry.set_rbc_pending(
                pending_stash_sessions,
                pending_stash_chunks,
                pending_stash_bytes,
            );
            telemetry.set_rbc_lane_backlog(&lane_backlog_vec);
            telemetry.set_rbc_dataspace_backlog(&dataspace_backlog_vec);
        }
    }
}
