//! RBC planning, chunking, and hydration helpers.
#![allow(
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unused_self
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    num::NonZeroUsize,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant, SystemTime},
};

use eyre::Result;
use iroha_config::parameters::actual as config_actual;
use iroha_crypto::{Hash, HashOf, MerkleTree, Signature};
use iroha_data_model::{
    ChainId, Encode as _,
    block::{BlockHeader, BlockPayload, SignedBlock, consensus::RbcEncoding},
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
};
use iroha_logger::prelude::*;
use iroha_primitives::erasure::rs16 as erasure_rs16;
use norito::codec::Decode;
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use sha2::{Digest, Sha256};

use super::{
    Actor, BlockPayloadDedupKey, DataspaceAllocation, InvalidSigKind, InvalidSigOutcome,
    LaneAllocation, MissingBlockClearReason, MissingBlockFetchDecision, PipelinePhase,
    RBC_MAX_TOTAL_CHUNKS, RbcPayloadLayout, RbcRosterSource, RbcSession, RbcSessionError,
    pending_rbc::{
        PendingChunkOutcome, PendingRbcDropReason, PendingRbcMessages, rbc_deliver_stash_bytes,
        rbc_ready_stash_bytes,
    },
    proposals::block_payload_bytes,
};
use crate::{
    queue::{Queue, RoutingDecision},
    sumeragi::{
        BackgroundRequest, CryptoHash,
        consensus::{
            RbcChunk, RbcDeliver, RbcInit, RbcReady, rbc_deliver_preimage, rbc_ready_preimage,
        },
        message::{BlockMessage, BlockMessageWire},
        network_topology::commit_quorum_from_len,
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
    pub(super) roster: Vec<PeerId>,
}

#[derive(Clone, Debug)]
pub(super) struct RbcPlan {
    pub(super) primary: RbcSessionPlan,
    pub(super) duplicate: Option<RbcSessionPlan>,
}

const RBC_PERSIST_WORK_QUEUE_CAP: usize = 8;
const RBC_PERSIST_RESULT_QUEUE_CAP: usize = 8;
const RBC_SEED_WORKER_THREADS: usize = 4;
const RBC_SEED_WORK_QUEUE_CAP: usize = 4 * RBC_SEED_WORKER_THREADS;
const RBC_SEED_RESULT_QUEUE_CAP: usize = 4 * RBC_SEED_WORKER_THREADS;

pub(super) fn rbc_roster_hash(roster: &[PeerId]) -> Hash {
    Hash::new(roster.to_vec().encode())
}

fn roster_has_duplicates(roster: &[PeerId]) -> bool {
    let mut seen = BTreeSet::new();
    for peer in roster {
        if !seen.insert(peer.clone()) {
            return true;
        }
    }
    false
}

fn normalize_rbc_block_header(mut header: BlockHeader) -> BlockHeader {
    // Result merkle root is excluded from consensus hashing, so ignore it for RBC header matching.
    header.result_merkle_root = None;
    header
}

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

#[derive(Debug)]
pub(super) struct RbcSeedWork {
    pub(super) key: SessionKey,
    pub(super) payload_hash: Hash,
    pub(super) payload_bytes: Vec<u8>,
    pub(super) chunking: RbcChunkingSpec,
    pub(super) epoch: u64,
}

#[derive(Debug)]
pub(super) struct RbcSeedResult {
    pub(super) key: SessionKey,
    pub(super) payload_hash: Hash,
    pub(super) outcome: Result<RbcSession>,
}

#[derive(Debug)]
pub(super) struct RbcSeedWorkerHandle {
    pub(super) work_tx: mpsc::SyncSender<RbcSeedWork>,
    pub(super) result_rx: mpsc::Receiver<RbcSeedResult>,
    pub(super) join_handle: std::thread::JoinHandle<()>,
}

#[derive(Debug)]
pub(super) enum RbcError {
    TransactionPayloadTooLarge { len: usize },
    ChunkSizeOverflow { chunk_size: usize },
    ChunkCountOverflow { count: usize },
    ChunkCountExceedsCap { count: u32, cap: u32 },
    ChunkRootUnavailable,
    MissingLeaderSignature,
    ChunkIndexOverflow { idx: usize },
    SessionInit(RbcSessionError),
}

impl std::fmt::Display for RbcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransactionPayloadTooLarge { len } => {
                write!(
                    f,
                    "transaction payload length {len} exceeds addressable range"
                )
            }
            Self::ChunkSizeOverflow { chunk_size } => {
                write!(
                    f,
                    "RBC chunk size {chunk_size} exceeds the protocol metadata range"
                )
            }
            Self::ChunkCountOverflow { count } => {
                write!(f, "RBC payload requires {count} chunks")
            }
            Self::ChunkCountExceedsCap { count, cap } => {
                write!(
                    f,
                    "RBC payload requires {count} chunks, exceeding hard cap {cap}"
                )
            }
            Self::ChunkRootUnavailable => write!(f, "failed to compute chunk root"),
            Self::MissingLeaderSignature => write!(f, "missing leader signature for RBC init"),
            Self::ChunkIndexOverflow { idx } => {
                write!(f, "RBC chunk index {idx} exceeds u32 range")
            }
            Self::SessionInit(err) => write!(f, "RBC session init failed: {err}"),
        }
    }
}

impl std::error::Error for RbcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SessionInit(err) => Some(err),
            _ => None,
        }
    }
}

impl From<RbcSessionError> for RbcError {
    fn from(err: RbcSessionError) -> Self {
        Self::SessionInit(err)
    }
}

fn spawn_rbc_persist_worker(
    cfg: crate::sumeragi::RbcStoreConfig,
    wake_tx: Option<mpsc::SyncSender<()>>,
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
    let join_handle =
        crate::sumeragi::sumeragi_thread_builder("sumeragi-rbc-persist").spawn(move || {
            while let Ok(work) = work_rx.recv() {
                let key = work.key;
                let outcome = store.persist_snapshot(&work.persisted);
                if result_tx.send(RbcPersistResult { key, outcome }).is_err() {
                    break;
                }
                if let Some(wake) = wake_tx.as_ref() {
                    let _ = wake.try_send(());
                }
            }
        })?;
    Ok(RbcPersistWorkerHandle {
        work_tx,
        result_rx,
        join_handle,
    })
}

fn spawn_rbc_seed_worker(wake_tx: Option<mpsc::SyncSender<()>>) -> io::Result<RbcSeedWorkerHandle> {
    let (work_tx, work_rx) = mpsc::sync_channel::<RbcSeedWork>(RBC_SEED_WORK_QUEUE_CAP);
    let (result_tx, result_rx) = mpsc::sync_channel::<RbcSeedResult>(RBC_SEED_RESULT_QUEUE_CAP);
    let work_rx = Arc::new(Mutex::new(work_rx));
    let mut worker_handles = Vec::with_capacity(RBC_SEED_WORKER_THREADS);
    for idx in 0..RBC_SEED_WORKER_THREADS {
        let work_rx = Arc::clone(&work_rx);
        let result_tx = result_tx.clone();
        let wake_tx = wake_tx.clone();
        let thread_name = format!("sumeragi-rbc-seed-{idx}");
        let handle = crate::sumeragi::sumeragi_thread_builder(thread_name).spawn(move || {
            loop {
                let work = {
                    let guard = match work_rx.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    guard.recv()
                };
                let Ok(work) = work else {
                    break;
                };
                let key = work.key;
                let payload_hash = work.payload_hash;
                let outcome = Actor::build_rbc_session_from_payload_with_chunking(
                    &work.payload_bytes,
                    payload_hash,
                    work.chunking,
                    work.epoch,
                )
                .map_err(eyre::Report::from);
                if result_tx
                    .send(RbcSeedResult {
                        key,
                        payload_hash,
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
        })?;
        worker_handles.push(handle);
    }
    drop(result_tx);
    let join_handle = crate::sumeragi::sumeragi_thread_builder("sumeragi-rbc-seed-supervisor")
        .spawn(move || {
            for handle in worker_handles {
                if handle.join().is_err() {
                    warn!("RBC seed worker thread panicked");
                }
            }
        })?;
    Ok(RbcSeedWorkerHandle {
        work_tx,
        result_rx,
        join_handle,
    })
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RbcChunkingSpec {
    pub(super) encoding: RbcEncoding,
    pub(super) chunk_size_bytes: usize,
    pub(super) data_shards: u16,
    pub(super) parity_shards: u16,
}

impl RbcChunkingSpec {
    pub(super) const fn plain(chunk_size_bytes: usize) -> Self {
        Self {
            encoding: RbcEncoding::Plain,
            chunk_size_bytes,
            data_shards: 0,
            parity_shards: 0,
        }
    }

    pub(super) fn from_config(rbc: &config_actual::SumeragiRbc) -> Self {
        let (data_shards, parity_shards) = match rbc.encoding {
            RbcEncoding::Plain => (0, 0),
            RbcEncoding::Rs16 => (rbc.data_shards, rbc.parity_shards),
        };
        Self {
            encoding: rbc.encoding,
            chunk_size_bytes: rbc.chunk_max_bytes,
            data_shards,
            parity_shards,
        }
    }

    fn from_layout(layout: RbcPayloadLayout) -> Self {
        if !layout.payload_size_known() {
            return Self::plain(layout.chunk_size().max(1));
        }
        Self {
            encoding: layout.encoding,
            chunk_size_bytes: layout.chunk_size(),
            data_shards: layout.data_shards,
            parity_shards: layout.parity_shards,
        }
    }

    fn chunk_size_u32(self) -> std::result::Result<u32, RbcError> {
        u32::try_from(self.chunk_size_bytes).map_err(|_| RbcError::ChunkSizeOverflow {
            chunk_size: self.chunk_size_bytes,
        })
    }

    fn layout_for_payload(
        self,
        payload_len: usize,
    ) -> std::result::Result<RbcPayloadLayout, RbcError> {
        let chunk_size_bytes = self.chunk_size_u32()?;
        RbcPayloadLayout::new(
            self.encoding,
            chunk_size_bytes,
            u64::try_from(payload_len).unwrap_or(u64::MAX),
            self.data_shards,
            self.parity_shards,
        )
        .map_err(RbcError::from)
    }
}

#[derive(Clone, Debug)]
struct EncodedRbcPayload {
    layout: RbcPayloadLayout,
    chunks: Vec<Vec<u8>>,
    digests: Vec<[u8; 32]>,
    chunk_root: Hash,
}

pub(super) fn chunk_payload_bytes(payload: &[u8], chunk_size: usize) -> Vec<Vec<u8>> {
    let effective = chunk_size.max(1);
    let chunk_total = chunk_count(payload.len(), chunk_size);
    let mut chunks = Vec::with_capacity(chunk_total);
    if payload.is_empty() {
        // Represent empty payloads as a single empty chunk to avoid zero-length sessions.
        chunks.push(Vec::new());
        return chunks;
    }
    for chunk in payload.chunks(effective) {
        chunks.push(chunk.to_vec());
    }
    chunks
}

#[inline]
pub(super) fn chunk_count(payload_len: usize, chunk_size: usize) -> usize {
    let effective = chunk_size.max(1);
    if payload_len == 0 {
        return 1;
    }
    payload_len.div_ceil(effective)
}

fn encode_rbc_payload(
    payload_bytes: &[u8],
    chunking: RbcChunkingSpec,
) -> std::result::Result<EncodedRbcPayload, RbcError> {
    let layout = chunking.layout_for_payload(payload_bytes.len())?;
    let chunk_bytes = match chunking.encoding {
        RbcEncoding::Plain => chunk_payload_bytes(payload_bytes, chunking.chunk_size_bytes),
        RbcEncoding::Rs16 => {
            let data_shards = usize::from(chunking.data_shards);
            let parity_shards = usize::from(chunking.parity_shards);
            let stripe_width = data_shards.saturating_add(parity_shards);
            let data_chunks = chunk_payload_bytes(payload_bytes, chunking.chunk_size_bytes);
            let stripe_count = data_chunks.len().div_ceil(data_shards);
            let symbol_count = chunking.chunk_size_bytes / 2;
            let mut encoded = Vec::with_capacity(stripe_count.saturating_mul(stripe_width));
            for stripe_idx in 0..stripe_count {
                let start = stripe_idx.saturating_mul(data_shards);
                let end = start.saturating_add(data_shards).min(data_chunks.len());
                let mut stripe = Vec::with_capacity(data_shards);
                let mut symbols = Vec::with_capacity(data_shards);
                for chunk in &data_chunks[start..end] {
                    stripe.push(chunk.clone());
                    symbols.push(erasure_rs16::symbols_from_chunk(symbol_count, chunk));
                }
                while stripe.len() < data_shards {
                    stripe.push(Vec::new());
                    symbols.push(erasure_rs16::symbols_from_chunk(symbol_count, &[]));
                }
                let parity = erasure_rs16::encode_parity(&symbols, parity_shards)
                    .map_err(|_| RbcError::ChunkRootUnavailable)?;
                encoded.extend(stripe);
                for shard in parity {
                    let bytes = erasure_rs16::chunk_from_symbols(&shard, chunking.chunk_size_bytes)
                        .map_err(|_| RbcError::ChunkRootUnavailable)?;
                    encoded.push(bytes);
                }
            }
            encoded
        }
    };

    let total_chunks =
        u32::try_from(chunk_bytes.len()).map_err(|_| RbcError::ChunkCountOverflow {
            count: chunk_bytes.len(),
        })?;
    if total_chunks > RBC_MAX_TOTAL_CHUNKS {
        return Err(RbcError::ChunkCountExceedsCap {
            count: total_chunks,
            cap: RBC_MAX_TOTAL_CHUNKS,
        });
    }

    let mut digests = Vec::with_capacity(chunk_bytes.len());
    for chunk in &chunk_bytes {
        let digest = Sha256::digest(chunk);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&digest);
        digests.push(arr);
    }
    let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests.clone())
        .root()
        .map(Hash::from)
        .ok_or(RbcError::ChunkRootUnavailable)?;

    Ok(EncodedRbcPayload {
        layout,
        chunks: chunk_bytes,
        digests,
        chunk_root,
    })
}

fn rbc_layout_from_init(init: &RbcInit) -> std::result::Result<RbcPayloadLayout, RbcSessionError> {
    if init.chunk_size_bytes == 0 {
        return Ok(RbcPayloadLayout::legacy_plain());
    }
    RbcPayloadLayout::new(
        init.encoding,
        init.chunk_size_bytes,
        init.payload_size_bytes,
        init.data_shards,
        init.parity_shards,
    )
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub(super) struct HydrationOutcome {
    pub(super) updated: bool,
    pub(super) all_chunks_present: bool,
    pub(super) payload_hash_mismatch: bool,
    pub(super) chunk_root_mismatch: bool,
    pub(super) chunk_digest_mismatch: bool,
    pub(super) layout_mismatch: bool,
    pub(super) observed_chunks: Option<u32>,
    pub(super) observed_chunk_root: Option<Hash>,
}

pub(super) fn apply_hydrated_payload(
    session: &mut RbcSession,
    payload_bytes: &[u8],
    payload_hash: Hash,
    chunk_max_bytes: usize,
) -> HydrationOutcome {
    let mut outcome = HydrationOutcome::default();
    if payload_bytes.is_empty() {
        session.invalid = true;
        outcome.updated = true;
        outcome.layout_mismatch = true;
        outcome.observed_chunks = Some(0);
        return outcome;
    }
    let chunking = if session.layout().payload_size_known() {
        RbcChunkingSpec::from_layout(session.layout())
    } else {
        RbcChunkingSpec::plain(chunk_max_bytes)
    };
    let encoded = match encode_rbc_payload(payload_bytes, chunking) {
        Ok(encoded) => encoded,
        Err(_) => {
            session.invalid = true;
            outcome.updated = true;
            outcome.layout_mismatch = true;
            return outcome;
        }
    };
    if session.layout().payload_size_known() && session.layout() != encoded.layout {
        session.invalid = true;
        outcome.updated = true;
        outcome.layout_mismatch = true;
        return outcome;
    }
    if !session.layout().payload_size_known() {
        session.layout = encoded.layout;
        outcome.updated = true;
    }
    let EncodedRbcPayload {
        chunks,
        digests,
        chunk_root,
        ..
    } = encoded;

    let chunk_count = if let Ok(count) = u32::try_from(chunks.len()) {
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

    if let Some(expected) = session.expected_chunk_digests.as_ref() {
        if expected != &digests {
            session.invalid = true;
            outcome.chunk_digest_mismatch = true;
        }
    } else {
        session.expected_chunk_digests = Some(digests.clone());
        outcome.updated = true;
    }

    outcome.observed_chunk_root = Some(chunk_root);

    let dropped = session.drop_mismatched_chunks();
    if dropped > 0 {
        outcome.updated = true;
    }

    for (idx, chunk) in chunks.iter().enumerate() {
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
        let observed_root = outcome.observed_chunk_root.or_else(|| session.chunk_root());
        if let Some(observed_root) = observed_root {
            if let Some(expected_root) = session.expected_chunk_root {
                if expected_root != observed_root {
                    session.invalid = true;
                    outcome.chunk_root_mismatch = true;
                }
            } else {
                session.expected_chunk_root = Some(observed_root);
                outcome.updated = true;
            }
        }
    }

    if outcome.payload_hash_mismatch || outcome.chunk_root_mismatch || outcome.chunk_digest_mismatch
    {
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

pub(super) fn rbc_chunk_target_count(roster_len: usize, fanout_cap: Option<NonZeroUsize>) -> usize {
    let peers = roster_len.saturating_sub(1);
    if peers == 0 {
        return 0;
    }
    let commit_quorum = commit_quorum_from_len(roster_len);
    // Ensure a commit quorum of peers receives chunks even if the leader is excluded.
    let min_targets = commit_quorum.min(peers);
    // Default to the full roster so RBC can reach READY quorum even with up to f faulty peers.
    let desired = fanout_cap.map_or(peers, |cap| cap.get().min(peers));
    desired.max(min_targets)
}

pub(super) fn select_rbc_chunk_targets(
    roster: &[PeerId],
    local_peer_id: &PeerId,
    seed: u64,
    target_count: usize,
) -> Vec<(usize, PeerId)> {
    if target_count == 0 {
        return Vec::new();
    }
    let mut candidates: Vec<(usize, PeerId)> = roster
        .iter()
        .enumerate()
        .filter(|(_, peer)| *peer != local_peer_id)
        .map(|(idx, peer)| (idx, peer.clone()))
        .collect();
    if candidates.is_empty() {
        return candidates;
    }
    if target_count < candidates.len() {
        let mut rng = StdRng::seed_from_u64(seed);
        candidates.shuffle(&mut rng);
        candidates.truncate(target_count);
    }
    candidates
}

#[allow(dead_code)]
fn rbc_rebroadcasters_count(roster_len: usize) -> usize {
    if roster_len == 0 {
        return 0;
    }
    let max_faults = roster_len.saturating_sub(1) / 3;
    max_faults.saturating_add(1).min(roster_len)
}

#[allow(dead_code)]
fn rbc_rebroadcast_indices_with_count(
    roster_len: usize,
    seed: u64,
    rebroadcaster_count: usize,
) -> Vec<usize> {
    if rebroadcaster_count == 0 {
        return Vec::new();
    }
    if rebroadcaster_count >= roster_len {
        return (0..roster_len).collect();
    }

    let leader_idx = 0;
    let mut selected = Vec::with_capacity(rebroadcaster_count);
    selected.push(leader_idx);

    let mut candidates: Vec<usize> = (1..roster_len).collect();
    let mut rng = StdRng::seed_from_u64(seed);
    candidates.shuffle(&mut rng);
    selected.extend(
        candidates
            .into_iter()
            .take(rebroadcaster_count.saturating_sub(1)),
    );
    selected
}

#[allow(dead_code)]
fn rbc_rebroadcast_indices(roster_len: usize, seed: u64) -> Vec<usize> {
    rbc_rebroadcast_indices_with_count(roster_len, seed, rbc_rebroadcasters_count(roster_len))
}

#[allow(dead_code)]
fn rbc_ready_rebroadcasters_count(roster_len: usize) -> usize {
    if roster_len == 0 {
        return 0;
    }
    commit_quorum_from_len(roster_len).min(roster_len)
}

#[allow(dead_code)]
fn rbc_ready_rebroadcast_indices(roster_len: usize, seed: u64) -> Vec<usize> {
    rbc_rebroadcast_indices_with_count(roster_len, seed, rbc_ready_rebroadcasters_count(roster_len))
}

#[allow(dead_code)]
pub(super) fn is_payload_rebroadcaster(
    roster: &[PeerId],
    local_peer_id: &PeerId,
    seed: u64,
) -> bool {
    let Some(local_idx) = roster.iter().position(|peer| peer == local_peer_id) else {
        return false;
    };
    rbc_rebroadcast_indices(roster.len(), seed).contains(&local_idx)
}

#[allow(dead_code)]
pub(super) fn is_ready_rebroadcaster(roster: &[PeerId], local_peer_id: &PeerId, seed: u64) -> bool {
    let Some(local_idx) = roster.iter().position(|peer| peer == local_peer_id) else {
        return false;
    };
    rbc_ready_rebroadcast_indices(roster.len(), seed).contains(&local_idx)
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

fn distribute_allocation_weights(total_chunks: u32, weights: &[u128]) -> Vec<u32> {
    let mut allocations = distribute_chunks(total_chunks, weights);
    if total_chunks == 0 {
        return allocations;
    }
    for (allocation, weight) in allocations.iter_mut().zip(weights.iter()) {
        if *allocation == 0 && *weight > 0 {
            *allocation = 1;
        }
    }
    let assigned_total: u32 = allocations.iter().copied().sum();
    if assigned_total > total_chunks {
        let mut excess = assigned_total - total_chunks;
        for allocation in allocations.iter_mut().rev() {
            if excess == 0 {
                break;
            }
            if *allocation > 0 {
                *allocation -= 1;
                excess -= 1;
            }
        }
    }
    allocations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::network_topology::commit_quorum_from_len;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::peer::PeerId;

    fn sample_roster(len: usize) -> Vec<PeerId> {
        (0..len)
            .map(|idx| {
                let seed = format!("rbc-roster-{idx}");
                let keypair = KeyPair::from_seed(seed.into_bytes(), Algorithm::Ed25519);
                PeerId::new(keypair.public_key().clone())
            })
            .collect()
    }

    #[test]
    fn rbc_chunk_target_count_defaults_to_full_roster() {
        let roster_len: usize = 7;
        let peers = roster_len.saturating_sub(1);
        let commit_quorum = commit_quorum_from_len(roster_len);
        let expected_min = commit_quorum.min(peers);
        assert_eq!(rbc_chunk_target_count(roster_len, None), peers);
        assert_eq!(
            rbc_chunk_target_count(roster_len, NonZeroUsize::new(1)),
            expected_min
        );
        assert_eq!(
            rbc_chunk_target_count(roster_len, NonZeroUsize::new(32)),
            peers
        );
    }

    #[test]
    fn select_rbc_chunk_targets_is_deterministic() {
        let roster = sample_roster(5);
        let local = roster[0].clone();
        let targets = select_rbc_chunk_targets(&roster, &local, 42, 2);
        let targets_repeat = select_rbc_chunk_targets(&roster, &local, 42, 2);
        assert_eq!(targets, targets_repeat);
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().all(|(_, peer)| peer != &local));
    }

    #[test]
    fn rbc_rebroadcaster_count_tracks_fault_tolerance() {
        assert_eq!(rbc_rebroadcasters_count(0), 0);
        assert_eq!(rbc_rebroadcasters_count(1), 1);
        assert_eq!(rbc_rebroadcasters_count(2), 1);
        assert_eq!(rbc_rebroadcasters_count(3), 1);
        assert_eq!(rbc_rebroadcasters_count(4), 2);
        assert_eq!(rbc_rebroadcasters_count(7), 3);
    }

    #[test]
    fn rbc_ready_rebroadcaster_count_tracks_commit_quorum() {
        for roster_len in 0..10 {
            let expected = commit_quorum_from_len(roster_len).min(roster_len);
            assert_eq!(rbc_ready_rebroadcasters_count(roster_len), expected);
        }
    }

    #[test]
    fn rbc_rebroadcast_subset_includes_leader_and_is_deterministic() {
        let roster = sample_roster(4);
        let seed = 99;
        let indices = rbc_rebroadcast_indices(roster.len(), seed);
        assert_eq!(indices.len(), rbc_rebroadcasters_count(roster.len()));
        assert!(indices.contains(&0));
        let repeat = rbc_rebroadcast_indices(roster.len(), seed);
        assert_eq!(indices, repeat);
    }

    #[test]
    fn rbc_rebroadcaster_selection_is_deterministic() {
        let roster = sample_roster(5);
        let local = roster[2].clone();
        let seed = 73;
        let first = is_payload_rebroadcaster(&roster, &local, seed);
        let second = is_payload_rebroadcaster(&roster, &local, seed);
        let ready_first = is_ready_rebroadcaster(&roster, &local, seed);
        let ready_second = is_ready_rebroadcaster(&roster, &local, seed);
        assert_eq!(first, second);
        assert_eq!(ready_first, ready_second);

        let payload_rebroadcasters = roster
            .iter()
            .filter(|peer| is_payload_rebroadcaster(&roster, peer, seed))
            .count();
        assert_eq!(
            payload_rebroadcasters,
            rbc_rebroadcasters_count(roster.len())
        );

        let ready_rebroadcasters = roster
            .iter()
            .filter(|peer| is_ready_rebroadcaster(&roster, peer, seed))
            .count();
        assert_eq!(
            ready_rebroadcasters,
            rbc_ready_rebroadcasters_count(roster.len())
        );
        assert!(ready_rebroadcasters >= payload_rebroadcasters);

        let outsider = PeerId::new(
            KeyPair::from_seed(b"rbc-outsider".to_vec(), Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        assert!(
            !roster.contains(&outsider),
            "outsider peer must not be part of the roster"
        );
        assert!(!is_payload_rebroadcaster(&roster, &outsider, seed));
        assert!(!is_ready_rebroadcaster(&roster, &outsider, seed));
    }

    #[test]
    fn distribute_allocation_weights_enforces_minimums() {
        let allocations = distribute_allocation_weights(5, &[1, 100, 1]);
        assert_eq!(allocations, vec![1, 4, 0]);
    }

    #[test]
    fn distribute_allocation_weights_returns_zero_for_empty_total() {
        let allocations = distribute_allocation_weights(0, &[1, 1, 1]);
        assert_eq!(allocations, vec![0, 0, 0]);
    }

    #[test]
    fn rbc_error_wraps_session_init_error() {
        let err = RbcError::from(RbcSessionError::TooManyChunks {
            total_chunks: 10,
            max_chunks: 2,
        });
        assert!(matches!(
            err,
            RbcError::SessionInit(RbcSessionError::TooManyChunks { .. })
        ));
        assert!(std::error::Error::source(&err).is_some());
        let rendered = format!("{err}");
        assert!(
            rendered.contains("RBC session init failed"),
            "unexpected error message: {rendered}"
        );
    }

    #[test]
    fn rbc_seed_worker_exits_on_channel_close() {
        let RbcSeedWorkerHandle {
            work_tx,
            result_rx: _result_rx,
            join_handle,
        } = spawn_rbc_seed_worker(None).expect("spawn rbc seed worker");
        drop(work_tx);
        assert!(join_handle.join().is_ok());
    }
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
    evaluate_deliver_acceptance_with_policy(session, required_ready, false)
}

pub(super) fn evaluate_deliver_acceptance_with_policy(
    session: &RbcSession,
    required_ready: usize,
    allow_missing_chunks: bool,
) -> DeliverAcceptance {
    if required_ready != 0 && session.ready_signatures.len() < required_ready {
        return DeliverAcceptance::DeferReady {
            ready_count: session.ready_signatures.len(),
            required: required_ready,
        };
    }

    let received = session.received_chunks();
    let total = session.total_chunks();
    if !allow_missing_chunks && total != 0 && received < total {
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
    pub(super) fn release_block_payload_dedup(&self, key: &BlockPayloadDedupKey) {
        let mut guard = self
            .block_payload_dedup
            .lock()
            .expect("block payload dedup cache poisoned");
        guard.remove(key);
    }

    pub(super) fn release_pending_rbc_dedup(&self, pending: &PendingRbcMessages) {
        if pending.chunks.is_empty() && pending.ready.is_empty() && pending.deliver.is_empty() {
            return;
        }
        let mut guard = self
            .block_payload_dedup
            .lock()
            .expect("block payload dedup cache poisoned");
        for entry in &pending.ready {
            let signature_hash = CryptoHash::new(&entry.signature);
            let key = BlockPayloadDedupKey::RbcReady {
                height: entry.height,
                view: entry.view,
                block_hash: entry.block_hash,
                sender: entry.sender,
                signature_hash,
            };
            guard.remove(&key);
        }
        for entry in &pending.deliver {
            let signature_hash = CryptoHash::new(&entry.signature);
            let key = BlockPayloadDedupKey::RbcDeliver {
                height: entry.height,
                view: entry.view,
                block_hash: entry.block_hash,
                sender: entry.sender,
                signature_hash,
            };
            guard.remove(&key);
        }
        for entry in &pending.chunks {
            let bytes_hash = CryptoHash::new(&entry.chunk.bytes);
            let key = BlockPayloadDedupKey::RbcChunk {
                height: entry.chunk.height,
                view: entry.chunk.view,
                epoch: entry.chunk.epoch,
                block_hash: entry.chunk.block_hash,
                idx: entry.chunk.idx,
                bytes_hash,
            };
            guard.remove(&key);
        }
    }

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

        debug_assert_eq!(
            transactions.len(),
            routing.len(),
            "routing decisions must align with transactions"
        );

        let encoded = encode_rbc_payload(payload, RbcChunkingSpec::from_config(&self.config.rbc))?;
        let total_chunks = u32::try_from(encoded.chunks.len()).expect("encoded chunk count fits");

        let (lane_allocations, dataspace_allocations) =
            self.derive_rbc_allocations(transactions, routing, total_chunks)?;

        let block_hash = signed_block.hash();
        let leader_signature = signed_block
            .signatures()
            .find(|signature| signature.index() == u64::from(local_validator_index))
            .cloned()
            .ok_or(RbcError::MissingLeaderSignature)?;
        let block_header = signed_block.header();
        let mut session = RbcSession::new_with_layout(
            encoded.layout,
            total_chunks,
            Some(payload_hash),
            Some(encoded.chunk_root),
            Some(encoded.digests.clone()),
            epoch,
        )
        .map_err(RbcError::from)?;
        session.block_header = Some(block_header);
        session.leader_signature = Some(leader_signature.clone());
        for (idx, chunk) in encoded.chunks.iter().enumerate() {
            let chunk_index = u32::try_from(idx).expect("chunk index fits within a 32-bit range");
            session.ingest_chunk(chunk_index, chunk.clone(), Some(local_validator_index));
        }

        session.set_allocations(lane_allocations, dataspace_allocations);

        let primary_key = Self::session_key(&block_hash, height, view);
        let roster = self.rbc_roster_for_session(primary_key);
        if roster.is_empty() {
            warn!(
                height,
                view, "skipping RBC plan: empty commit roster snapshot"
            );
            return Ok(None);
        }
        let roster_hash = rbc_roster_hash(&roster);

        let drop_every = self
            .config
            .debug
            .rbc
            .drop_every_nth_chunk
            .and_then(|value| usize::try_from(value.get()).ok());
        let (order, dropped) = compute_chunk_broadcast_order(
            encoded.chunks.len(),
            self.config.debug.rbc.shuffle_chunks,
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
                bytes: encoded.chunks[idx].clone(),
            });
        }

        let init = crate::sumeragi::consensus::RbcInit {
            block_hash,
            height,
            view,
            epoch,
            roster: roster.clone(),
            roster_hash,
            total_chunks,
            encoding: encoded.layout.encoding,
            chunk_size_bytes: encoded.layout.chunk_size_bytes,
            payload_size_bytes: encoded.layout.payload_size_bytes,
            data_shards: encoded.layout.data_shards,
            parity_shards: encoded.layout.parity_shards,
            chunk_digests: encoded.digests.clone(),
            payload_hash,
            chunk_root: encoded.chunk_root,
            block_header,
            leader_signature: leader_signature.clone(),
        };

        let primary_plan = RbcSessionPlan {
            key: primary_key,
            session,
            init,
            chunks,
            roster,
        };

        let duplicate_plan = if self.config.debug.rbc.duplicate_inits {
            if view == u64::MAX {
                None
            } else {
                let dup_view = view + 1;
                let dup_session = primary_plan.session.clone();
                let dup_key = Self::session_key(&block_hash, height, dup_view);
                let dup_roster = self.rbc_roster_for_session(dup_key);
                if dup_roster.is_empty() {
                    warn!(
                        height,
                        view = dup_view,
                        "skipping duplicate RBC init: empty commit roster snapshot"
                    );
                    return Ok(Some(RbcPlan {
                        primary: primary_plan,
                        duplicate: None,
                    }));
                }
                let dup_roster_hash = rbc_roster_hash(&dup_roster);
                let (dup_order, dup_dropped) = compute_chunk_broadcast_order(
                    encoded.chunks.len(),
                    self.config.debug.rbc.shuffle_chunks,
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
                        bytes: encoded.chunks[idx].clone(),
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
                        roster: dup_roster.clone(),
                        roster_hash: dup_roster_hash,
                        total_chunks,
                        encoding: encoded.layout.encoding,
                        chunk_size_bytes: encoded.layout.chunk_size_bytes,
                        payload_size_bytes: encoded.layout.payload_size_bytes,
                        data_shards: encoded.layout.data_shards,
                        parity_shards: encoded.layout.parity_shards,
                        chunk_digests: encoded.digests.clone(),
                        payload_hash,
                        chunk_root: encoded.chunk_root,
                        block_header,
                        leader_signature: leader_signature.clone(),
                    },
                    chunks: dup_chunks,
                    roster: dup_roster.clone(),
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

    /// Derive per-lane and per-dataspace chunk allocations for the RBC plan.
    #[allow(clippy::too_many_lines)]
    pub(super) fn derive_rbc_allocations(
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
                .map_err(|_| RbcError::TransactionPayloadTooLarge { len: encoded.len() })?;
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
        let lane_chunks = distribute_allocation_weights(total_chunks, &lane_weights);
        for (alloc, chunk_count) in lane_allocations.iter_mut().zip(lane_chunks.into_iter()) {
            alloc.total_chunks = chunk_count;
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
            let allocated = distribute_allocation_weights(lane_total_chunks, &weights);
            for (alloc, chunk_count) in dataspace_allocations[idx..end]
                .iter_mut()
                .zip(allocated.into_iter())
            {
                alloc.total_chunks = chunk_count;
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
        Self::mask_includes(self.config.debug.rbc.drop_validator_mask, validator_idx)
    }

    fn should_equivocate_rbc_chunk(&self, validator_idx: u32, chunk_idx: u32) -> bool {
        Self::mask_includes(
            self.config.debug.rbc.equivocate_validator_mask,
            validator_idx,
        ) && chunk_idx < 64
            && Self::mask_includes(self.config.debug.rbc.equivocate_chunk_mask, chunk_idx)
    }

    fn should_withhold_chunk(&self, chunk_idx: u32) -> bool {
        chunk_idx < 64 && Self::mask_includes(self.config.debug.rbc.partial_chunk_mask, chunk_idx)
    }

    pub(super) fn should_fork_ready(&self, validator_idx: u32) -> bool {
        Self::mask_includes(self.config.debug.rbc.conflicting_ready_mask, validator_idx)
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

    pub(super) fn schedule_rbc_chunk_posts(
        &mut self,
        chunk: &super::OutboundRbcChunk,
        targets: &[(usize, PeerId)],
    ) {
        if targets.is_empty() {
            return;
        }
        let (block_hash, height, view, epoch, chunk_idx, bytes) = match chunk.message.as_ref() {
            BlockMessage::RbcChunk(inner) => (
                inner.block_hash,
                inner.height,
                inner.view,
                inner.epoch,
                inner.idx,
                inner.bytes.as_slice(),
            ),
            BlockMessage::RbcChunkCompact(inner) => (
                inner.block_hash,
                u64::from(inner.height),
                u64::from(inner.view),
                u64::from(inner.epoch),
                inner.idx,
                inner.bytes.as_slice(),
            ),
            other => {
                debug!(
                    kind = ?other,
                    "skipping outbound RBC chunk post: unexpected message variant"
                );
                return;
            }
        };
        let local_peer_id = self.common_config.peer.id().clone();
        for (target_idx, peer_id) in targets {
            if peer_id == &local_peer_id {
                continue;
            }
            let Ok(validator_idx) = u32::try_from(*target_idx) else {
                continue;
            };

            if self.should_drop_rbc_for(validator_idx) {
                debug!(
                    height,
                    view, chunk_idx, validator_idx, "dropping RBC chunk due to debug mask"
                );
                continue;
            }

            if self.should_equivocate_rbc_chunk(validator_idx, chunk_idx) {
                let mut bytes = bytes.to_vec();
                Self::mutate_chunk_for_equivocation(&mut bytes);
                let message_chunk =
                    BlockMessage::from_rbc_chunk(crate::sumeragi::consensus::RbcChunk {
                        block_hash,
                        height,
                        view,
                        epoch,
                        idx: chunk_idx,
                        bytes,
                    });
                debug!(
                    height,
                    view, chunk_idx, validator_idx, "equivocating RBC chunk due to debug mask"
                );
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer_id.clone(),
                    msg: BlockMessageWire::new(message_chunk),
                });
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer_id.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&chunk.message),
                    Arc::clone(&chunk.encoded),
                ),
            });
        }
    }

    pub(super) fn install_rbc_session_plan(&mut self, plan: &RbcSessionPlan) -> Result<()> {
        let key = plan.key;
        if self.retire_exact_frontier_rbc_runtime(key, "plan_install") {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "skipping RBC session installation for near-tip locally known block"
            );
            return Ok(());
        }
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
        self.record_rbc_session_roster(key, plan.roster.clone(), RbcRosterSource::Derived);
        self.persist_rbc_session(key, &plan.session);
        Ok(())
    }

    pub(super) fn broadcast_rbc_session_plan(&mut self, plan: RbcSessionPlan) -> Result<()> {
        let key = plan.key;
        if self.retire_exact_frontier_rbc_runtime(key, "plan_broadcast") {
            return Ok(());
        }
        let topology_peers = plan.roster;
        if topology_peers.is_empty() {
            return Ok(());
        }
        let local_peer_id = self.common_config.peer.id().clone();
        let init_message = Arc::new(BlockMessage::RbcInit(plan.init));
        let init_encoded = Arc::new(BlockMessageWire::encode_message(init_message.as_ref()));
        let initial_chunk_count = plan.chunks.len();
        let dispatch = self.dispatch_rbc_outbound_chunks(
            key,
            initial_chunk_count,
            Some((plan.chunks, &topology_peers)),
        );
        for peer in &topology_peers {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&init_message),
                    Arc::clone(&init_encoded),
                ),
            });
        }
        if dispatch.sent != 0 {
            self.subsystems
                .da_rbc
                .rbc
                .payload_rebroadcast_last_sent
                .insert(key, Instant::now());
        }
        self.maybe_emit_rbc_ready(key)?;
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn build_rbc_session_from_payload(
        payload_bytes: &[u8],
        payload_hash: Hash,
        chunk_size: usize,
        epoch: u64,
    ) -> std::result::Result<RbcSession, RbcError> {
        Self::build_rbc_session_from_payload_with_chunking(
            payload_bytes,
            payload_hash,
            RbcChunkingSpec::plain(chunk_size),
            epoch,
        )
    }

    pub(super) fn build_rbc_session_from_payload_with_chunking(
        payload_bytes: &[u8],
        payload_hash: Hash,
        chunking: RbcChunkingSpec,
        epoch: u64,
    ) -> std::result::Result<RbcSession, RbcError> {
        let encoded = encode_rbc_payload(payload_bytes, chunking)?;
        let total_chunks = u32::try_from(encoded.chunks.len()).expect("encoded chunk count fits");
        let mut session = RbcSession::new_with_layout(
            encoded.layout,
            total_chunks,
            Some(payload_hash),
            Some(encoded.chunk_root),
            Some(encoded.digests),
            epoch,
        )
        .map_err(RbcError::from)?;

        for (idx, chunk) in encoded.chunks.into_iter().enumerate() {
            let idx = u32::try_from(idx).map_err(|_| RbcError::ChunkIndexOverflow { idx })?;
            session.ingest_chunk(idx, chunk, None);
        }

        Ok(session)
    }

    pub(super) fn seed_rbc_session_from_block(
        &mut self,
        key: SessionKey,
        block: &SignedBlock,
        payload_hash: Hash,
        rebroadcast_missing_init: bool,
    ) -> Result<()> {
        if self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            return Ok(());
        }

        let should_rebroadcast =
            rebroadcast_missing_init || self.subsystems.da_rbc.rbc.pending.contains_key(&key);
        let payload_bytes = block_payload_bytes(block);
        let mut session = match Self::build_rbc_session_from_payload_with_chunking(
            &payload_bytes,
            payload_hash,
            RbcChunkingSpec::from_config(&self.config.rbc),
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

        session.block_header = Some(block.header());
        let roster = self.rbc_roster_for_session(key);
        if !roster.is_empty() {
            let mut topology = super::network_topology::Topology::new(roster.clone());
            match self.leader_index_for(&mut topology, key.1, key.2) {
                Ok(leader_index) => {
                    let leader_index = u64::try_from(leader_index).unwrap_or(u64::MAX);
                    if let Some(signature) = block
                        .signatures()
                        .find(|signature| signature.index() == leader_index)
                    {
                        session.leader_signature = Some(signature.clone());
                    } else {
                        debug!(
                            height = key.1,
                            view = key.2,
                            expected = leader_index,
                            "leader signature missing while seeding RBC session"
                        );
                    }
                }
                Err(err) => {
                    debug!(
                        height = key.1,
                        view = key.2,
                        ?err,
                        "failed to derive leader index while seeding RBC session"
                    );
                }
            }
        }

        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        if roster.is_empty() {
            self.ensure_rbc_session_roster(key);
        } else {
            self.record_rbc_session_roster(key, roster, RbcRosterSource::Derived);
        }
        self.flush_pending_rbc(key)?;
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
        }
        self.publish_rbc_backlog_snapshot();
        self.maybe_emit_rbc_ready(key)?;
        if should_rebroadcast {
            if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                self.rebroadcast_rbc_payload_for_missing_init(key, &session);
            }
        }
        Ok(())
    }

    pub(super) fn insert_stub_rbc_session_from_block(
        &mut self,
        key: SessionKey,
        block: &SignedBlock,
        payload_hash: Hash,
        payload_len: usize,
    ) -> Result<bool> {
        if self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            return Ok(false);
        }

        let chunking = RbcChunkingSpec::from_config(&self.config.rbc);
        let layout = chunking
            .layout_for_payload(payload_len)
            .map_err(eyre::Report::from)?;
        let total_chunks = layout
            .total_chunks()
            .expect("layout built from a known payload size must report chunk count");
        let total_chunks =
            u32::try_from(total_chunks).map_err(|_| RbcError::ChunkCountOverflow {
                count: total_chunks,
            })?;
        if total_chunks > RBC_MAX_TOTAL_CHUNKS {
            return Err(RbcError::ChunkCountExceedsCap {
                count: total_chunks,
                cap: RBC_MAX_TOTAL_CHUNKS,
            }
            .into());
        }

        let mut session = RbcSession::new_with_layout(
            layout,
            total_chunks,
            Some(payload_hash),
            None,
            None,
            self.epoch_for_height(key.1),
        )
        .map_err(RbcError::from)?;
        session.block_header = Some(block.header());
        let roster = self.rbc_roster_for_session(key);
        if !roster.is_empty() {
            let mut topology = super::network_topology::Topology::new(roster.clone());
            match self.leader_index_for(&mut topology, key.1, key.2) {
                Ok(leader_index) => {
                    let leader_index = u64::try_from(leader_index).unwrap_or(u64::MAX);
                    if let Some(signature) = block
                        .signatures()
                        .find(|signature| signature.index() == leader_index)
                    {
                        session.leader_signature = Some(signature.clone());
                    } else {
                        debug!(
                            height = key.1,
                            view = key.2,
                            expected = leader_index,
                            "leader signature missing while inserting stub RBC session"
                        );
                    }
                }
                Err(err) => {
                    debug!(
                        height = key.1,
                        view = key.2,
                        ?err,
                        "failed to derive leader index while inserting stub RBC session"
                    );
                }
            }
        }
        if session.leader_signature.is_none() {
            session.leader_signature = block.signatures().next().cloned();
        }
        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        if roster.is_empty() {
            self.ensure_rbc_session_roster(key);
        } else {
            self.record_rbc_session_roster(key, roster, RbcRosterSource::Derived);
        }
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
        }
        self.publish_rbc_backlog_snapshot();
        Ok(true)
    }

    pub(super) fn populate_rbc_session_metadata_from_block(
        &mut self,
        key: SessionKey,
        block: &SignedBlock,
    ) -> bool {
        if !self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            return false;
        }

        let roster = self.ensure_rbc_session_roster(key);
        let leader_signature = if roster.is_empty() {
            None
        } else {
            let mut topology = super::network_topology::Topology::new(roster);
            match self.leader_index_for(&mut topology, key.1, key.2) {
                Ok(leader_index) => {
                    let leader_index = u64::try_from(leader_index).unwrap_or(u64::MAX);
                    block
                        .signatures()
                        .find(|signature| signature.index() == leader_index)
                        .cloned()
                }
                Err(err) => {
                    debug!(
                        height = key.1,
                        view = key.2,
                        ?err,
                        "failed to derive leader index while hydrating RBC metadata from BlockCreated"
                    );
                    None
                }
            }
        };

        let mut changed = false;
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get_mut(&key) {
            let header = block.header();
            if session.block_header != Some(header) {
                session.block_header = Some(header);
                changed = true;
            }
            if let Some(signature) = leader_signature
                && session.leader_signature.as_ref() != Some(&signature)
            {
                session.leader_signature = Some(signature);
                changed = true;
            }
        }

        if changed && let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
            self.publish_rbc_backlog_snapshot();
        }

        changed
    }

    pub(super) fn ensure_rbc_session_from_pending_block(
        &mut self,
        key: SessionKey,
        rebroadcast_missing_init: bool,
    ) -> Result<bool> {
        if self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            return Ok(true);
        }
        if let Some(intent) = self.subsystems.da_rbc.rbc.seed_inflight.get_mut(&key) {
            if rebroadcast_missing_init {
                intent.rebroadcast_missing_init = true;
            }
            return Ok(self.subsystems.da_rbc.rbc.sessions.contains_key(&key));
        }
        let (block, payload_hash) = {
            let Some(pending) = self.pending.pending_blocks.get(&key.0) else {
                return Ok(false);
            };
            if pending.height != key.1 || pending.view != key.2 {
                return Ok(false);
            }
            if matches!(
                pending.validation_status,
                super::pending_block::ValidationStatus::Invalid
            ) {
                return Ok(false);
            }
            (pending.block.clone(), pending.payload_hash)
        };
        if let Some(seed_tx) = self.subsystems.da_rbc.rbc.seed_tx.as_ref() {
            let payload_bytes = block_payload_bytes(&block);
            let payload_len = payload_bytes.len();
            let payload_bytes_for_hydrate = payload_bytes.clone();
            let work = RbcSeedWork {
                key,
                payload_hash,
                payload_bytes,
                chunking: RbcChunkingSpec::from_config(&self.config.rbc),
                epoch: self.epoch_for_height(key.1),
            };
            match seed_tx.try_send(work) {
                Ok(()) => {
                    self.subsystems.da_rbc.rbc.seed_inflight.insert(
                        key,
                        super::RbcSeedIntent {
                            rebroadcast_missing_init,
                        },
                    );
                    if let Err(err) = self.insert_stub_rbc_session_from_block(
                        key,
                        &block,
                        payload_hash,
                        payload_len,
                    ) {
                        warn!(
                            height = key.1,
                            view = key.2,
                            block = %key.0,
                            error = %err,
                            "failed to insert stub RBC session after seed enqueue"
                        );
                        self.subsystems.da_rbc.rbc.seed_inflight.remove(&key);
                        return Ok(false);
                    }
                    let hydrate_result = self.hydrate_rbc_session_from_block(
                        key,
                        &payload_bytes_for_hydrate,
                        payload_hash,
                        None,
                    );
                    // Pending block already includes full payload; keep READY/DELIVER on the
                    // synchronous path and ignore delayed background seed completion.
                    self.subsystems.da_rbc.rbc.seed_inflight.remove(&key);
                    hydrate_result?;
                    if rebroadcast_missing_init
                        && let Some(session) =
                            self.subsystems.da_rbc.rbc.sessions.get(&key).cloned()
                    {
                        self.rebroadcast_rbc_payload_for_missing_init(key, &session);
                    }
                    return Ok(self.subsystems.da_rbc.rbc.sessions.contains_key(&key));
                }
                Err(mpsc::TrySendError::Full(_work)) => {
                    debug!(?key, "RBC seed queue full; falling back to sync seeding");
                }
                Err(mpsc::TrySendError::Disconnected(_work)) => {
                    warn!(
                        ?key,
                        "RBC seed worker disconnected; falling back to sync seeding"
                    );
                    self.subsystems.da_rbc.rbc.seed_tx = None;
                    self.subsystems.da_rbc.rbc.seed_rx = None;
                    self.subsystems.da_rbc.rbc.seed_inflight.clear();
                }
            }
        }
        self.seed_rbc_session_from_block(key, &block, payload_hash, rebroadcast_missing_init)?;
        Ok(self.subsystems.da_rbc.rbc.sessions.contains_key(&key))
    }

    pub(super) fn maybe_hydrate_rbc_session_from_local_payload(
        &mut self,
        key: SessionKey,
        sender: Option<&PeerId>,
    ) -> Result<bool> {
        let should_hydrate = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .is_some_and(|session| {
                !session.is_invalid() && session.received_chunks() != session.total_chunks()
            });
        if !should_hydrate {
            return Ok(false);
        }
        let Some((height, view, payload_bytes, payload_hash)) = self
            .with_local_payload_for_progress(key.0, |height, view, payload_bytes, payload_hash| {
                (height, view, payload_bytes.to_vec(), payload_hash)
            })
        else {
            return Ok(false);
        };
        if height != key.1 || view != key.2 {
            return Ok(false);
        }
        self.hydrate_rbc_session_from_block(key, &payload_bytes, payload_hash, sender)?;
        Ok(true)
    }

    pub(super) fn rbc_missing_block_preferred_signers(
        &self,
        key: SessionKey,
    ) -> BTreeSet<crate::sumeragi::consensus::ValidatorIndex> {
        let roster = {
            let existing = self.rbc_session_roster(key);
            if existing.is_empty() {
                self.rbc_roster_for_session(key)
            } else {
                existing
            }
        };
        if roster.is_empty() {
            return BTreeSet::new();
        }

        let canonical_topology = crate::sumeragi::network_topology::Topology::new(roster);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
        let signature_topology =
            super::topology_for_view(&canonical_topology, key.1, key.2, mode_tag, prf_seed);
        let signers = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .and_then(|session| session.leader_signature.as_ref())
            .and_then(|signature| {
                crate::sumeragi::consensus::ValidatorIndex::try_from(signature.index()).ok()
            })
            .map_or_else(
                || BTreeSet::from([crate::sumeragi::consensus::ValidatorIndex::from(0_u16)]),
                |view_signer| BTreeSet::from([view_signer]),
            );
        super::normalize_signer_indices_to_canonical(
            &signers,
            &signature_topology,
            &canonical_topology,
        )
    }

    /// After INIT, try to hydrate from locally available payload bytes and fall back to targeted
    /// `BlockCreated` recovery only when the RBC session still cannot reconstruct a signed block
    /// after payload delivery.
    pub(super) fn rbc_session_needs_block_created_recovery_from_session(
        &self,
        key: SessionKey,
        session: Option<&RbcSession>,
    ) -> bool {
        if self.block_known_locally(key.0) {
            return false;
        }
        let Some(session) = session.or_else(|| self.subsystems.da_rbc.rbc.sessions.get(&key))
        else {
            return true;
        };
        session.payload_hash().is_none()
            || session.block_header.is_none()
            || session.leader_signature.is_none()
            || session.is_invalid()
    }

    pub(super) fn rbc_session_needs_block_created_recovery(&self, key: SessionKey) -> bool {
        self.rbc_session_needs_block_created_recovery_from_session(key, None)
    }

    pub(super) fn rbc_session_should_force_frontier_authoritative_body_fetch(
        &self,
        key: SessionKey,
        session: &RbcSession,
    ) -> bool {
        if session.is_invalid() || self.authoritative_block_payload_available(key.0) {
            return false;
        }
        let frontier_height = self.committed_height_snapshot().saturating_add(1);
        key.1 <= frontier_height.saturating_add(1)
            && session.total_chunks() == 1
            && session.received_chunks() == 0
    }

    /// After INIT, try to hydrate from locally available payload bytes and fall back to targeted
    /// `BlockCreated` recovery only when the RBC session cannot later rebuild the signed block
    /// from delivered payload bytes plus INIT metadata.
    fn post_rbc_init_payload_recovery(
        &mut self,
        key: SessionKey,
        sender: Option<&PeerId>,
        needs_missing_roster_recovery: bool,
    ) -> Result<()> {
        self.maybe_hydrate_rbc_session_from_local_payload(key, sender)?;
        let frontier_height = self.committed_height_snapshot().saturating_add(1);
        let near_frontier = key.1 <= frontier_height.saturating_add(1);
        if !near_frontier {
            return Ok(());
        }
        let (needs_missing_payload_recovery, force_authoritative_body_fetch) = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .map_or((false, false), |session| {
                (
                    !session.is_invalid() && session.received_chunks() == 0,
                    self.rbc_session_should_force_frontier_authoritative_body_fetch(key, session),
                )
            });
        if force_authoritative_body_fetch {
            self.request_missing_block_for_pending_rbc(
                key,
                "rbc_init_single_chunk_frontier_body",
                None,
            );
        } else if needs_missing_payload_recovery
            && self.rbc_session_needs_block_created_recovery(key)
        {
            self.request_missing_block_for_pending_rbc(key, "rbc_init_missing_payload", None);
        } else if needs_missing_roster_recovery {
            self.request_missing_block_for_pending_rbc(key, "rbc_init_missing_roster", None);
        }
        Ok(())
    }

    /// When a full block payload is available alongside a `BlockCreated` message, ingest it into
    /// the cached RBC session so availability tracking can proceed once RBC evidence arrives (READY/DELIVER).
    pub(super) fn hydrate_rbc_session_from_block(
        &mut self,
        key: SessionKey,
        payload_bytes: &[u8],
        payload_hash: Hash,
        sender: Option<&PeerId>,
    ) -> Result<()> {
        let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) else {
            return Ok(());
        };
        if session.delivered_payload_matches(&payload_hash) {
            let delivered_bytes = session.take_delivered_payload_bytes_for_telemetry();
            // Restore the session before returning so future RBC traffic can reuse it.
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            if let (Some(bytes), Some(telemetry)) = (delivered_bytes, self.telemetry_handle()) {
                telemetry.add_rbc_payload_bytes_delivered(bytes);
            }
            return Ok(());
        }

        let outcome = apply_hydrated_payload(
            &mut session,
            payload_bytes,
            payload_hash,
            self.config.rbc.chunk_max_bytes,
        );

        if outcome.payload_hash_mismatch {
            let log_outcome = sender.map(|peer| {
                self.record_rbc_mismatch(peer, status::RbcMismatchKind::PayloadHash, key.1, key.2)
            });
            if log_outcome.is_none_or(super::RbcMismatchLogOutcome::should_log) {
                warn!(
                    height = key.1,
                    view = key.2,
                    sender = ?sender,
                    expected = ?session.payload_hash(),
                    observed = ?payload_hash,
                    "hydrated payload hash mismatches RBC session; marking invalid"
                );
            } else {
                debug!(
                    height = key.1,
                    view = key.2,
                    sender = ?sender,
                    expected = ?session.payload_hash(),
                    observed = ?payload_hash,
                    "suppressing repeated hydrated payload hash mismatch log"
                );
            }
        }
        if outcome.chunk_digest_mismatch {
            let log_outcome = sender.map(|peer| {
                self.record_rbc_mismatch(peer, status::RbcMismatchKind::ChunkDigest, key.1, key.2)
            });
            if log_outcome.is_none_or(super::RbcMismatchLogOutcome::should_log) {
                warn!(
                    height = key.1,
                    view = key.2,
                    sender = ?sender,
                    "hydrated payload chunk-digest list mismatches INIT; marking invalid"
                );
            } else {
                debug!(
                    height = key.1,
                    view = key.2,
                    sender = ?sender,
                    "suppressing repeated hydrated chunk-digest mismatch log"
                );
            }
        }
        if outcome.chunk_root_mismatch {
            let log_outcome = sender.map(|peer| {
                self.record_rbc_mismatch(peer, status::RbcMismatchKind::ChunkRoot, key.1, key.2)
            });
            let expected_root = session.expected_chunk_root;
            let observed_root = outcome.observed_chunk_root.or_else(|| session.chunk_root());
            if log_outcome.is_none_or(super::RbcMismatchLogOutcome::should_log) {
                warn!(
                    height = key.1,
                    view = key.2,
                    sender = ?sender,
                    ?expected_root,
                    ?observed_root,
                    "hydrated payload chunk-root mismatches INIT; marking invalid"
                );
            } else {
                debug!(
                    height = key.1,
                    view = key.2,
                    sender = ?sender,
                    ?expected_root,
                    ?observed_root,
                    "suppressing repeated hydrated chunk-root mismatch log"
                );
            }
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
            || outcome.chunk_digest_mismatch
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
        let delivered_bytes = session.take_delivered_payload_bytes_for_telemetry();
        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        if let (Some(bytes), Some(telemetry)) = (delivered_bytes, self.telemetry_handle()) {
            telemetry.add_rbc_payload_bytes_delivered(bytes);
        }
        if should_update_status {
            self.publish_rbc_backlog_snapshot();
        }
        let promoted_roster = self.promote_rbc_session_roster_and_retry(key);
        if promoted_roster {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "promoted derived RBC roster and retried pending READY/DELIVER after payload hydration"
            );
        }
        if !invalidated {
            self.maybe_emit_rbc_ready(key)?;
            let authoritative_after =
                self.subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&key)
                    .is_some_and(|session| {
                        self.rbc_session_has_authoritative_payload_for_progress(key, session)
                    });
            if authoritative_after {
                self.recover_block_from_rbc_session(key);
                self.request_commit_pipeline_for_round(
                    key.1,
                    key.2,
                    super::status::RoundPhaseTrace::WaitValidation,
                    super::status::RoundEventCauseTrace::BlockAvailable,
                    None,
                );
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

    /// Attempt to reconstruct the full block payload for an RBC session once chunk ownership is
    /// complete and the local payload is authoritative.
    fn rbc_session_payload_bytes(&self, key: &SessionKey) -> Option<Vec<u8>> {
        let session = self.subsystems.da_rbc.rbc.sessions.get(key)?;
        if session.received_chunks() != session.total_chunks()
            || !self.rbc_session_has_authoritative_payload_for_progress(*key, session)
        {
            return None;
        }
        session.payload_bytes()
    }

    /// Attempt to reconstruct `BlockCreated` once the full RBC payload is available.
    /// Falls back to requesting a missing `BlockCreated` if the signed header is unavailable.
    pub(super) fn recover_block_from_rbc_session(&mut self, key: SessionKey) {
        if self.pending.pending_blocks.contains_key(&key.0) {
            return;
        }
        if self.kura.get_block_height_by_hash(key.0).is_some() {
            return;
        }
        if self.suppress_far_future_rbc_missing_block_fetch(key, "rbc_recover_future_window") {
            return;
        }
        let payload = self.rbc_session_payload_bytes(&key);
        if let Some(payload) = payload {
            let payload_hash = Hash::new(&payload);
            let mut payload_mismatch: Option<Hash> = None;
            {
                if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get_mut(&key) {
                    if let Some(expected) = session.payload_hash() {
                        if expected != payload_hash {
                            session.invalid = true;
                            payload_mismatch = Some(expected);
                        }
                    }
                }
            }
            if let Some(expected) = payload_mismatch {
                warn!(
                    height = key.1,
                    view = key.2,
                    expected = ?expected,
                    observed = ?payload_hash,
                    "reconstructed RBC payload hash mismatches session; marking invalid"
                );
                self.clear_pending_rbc(&key);
                if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                    self.update_rbc_status_entry(key, &session, false);
                    self.persist_rbc_session(key, &session);
                }
                self.publish_rbc_backlog_snapshot();
            }
            if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
                if let (Some(block_header), Some(leader_signature)) =
                    (session.block_header, session.leader_signature.clone())
                {
                    if block_header.hash() == key.0 {
                        match BlockPayload::decode(&mut &payload[..]) {
                            Ok(mut payload) => {
                                let mut expected_header = block_header;
                                expected_header.result_merkle_root = None;
                                if payload.header == expected_header {
                                    payload.header = block_header;
                                    let block = SignedBlock::presigned_with_payload(
                                        leader_signature,
                                        payload,
                                    );
                                    let block_created =
                                        self.frontier_block_created_for_wire(&block);
                                    if let Err(err) = self.handle_block_created(block_created, None)
                                    {
                                        warn!(
                                            ?err,
                                            height = key.1,
                                            view = key.2,
                                            block = %key.0,
                                            "failed to handle recovered BlockCreated from RBC payload"
                                        );
                                    } else {
                                        debug!(
                                            height = key.1,
                                            view = key.2,
                                            block = %key.0,
                                            "recovered BlockCreated from RBC payload"
                                        );
                                        return;
                                    }
                                } else {
                                    warn!(
                                        height = key.1,
                                        view = key.2,
                                        block = %key.0,
                                        "skipping RBC payload recovery: header mismatch"
                                    );
                                }
                            }
                            Err(err) => {
                                warn!(
                                ?err,
                                height = key.1,
                                view = key.2,
                                block = %key.0,
                                    "skipping RBC payload recovery: decode failed"
                                );
                            }
                        }
                    } else {
                        warn!(
                            height = key.1,
                            view = key.2,
                            expected = ?key.0,
                            observed = ?block_header.hash(),
                            "skipping RBC payload recovery: block header hash mismatch"
                        );
                    }
                }
            }
        }
        let mut roster = self.ensure_rbc_session_roster(key);
        if roster.is_empty() {
            roster = self.effective_commit_topology();
            if roster.is_empty() {
                debug!(
                    height = key.1,
                    view = key.2,
                    block = %key.0,
                    "skipping missing BlockCreated fetch: empty roster"
                );
                return;
            }
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "using fallback commit topology for missing BlockCreated fetch"
            );
        }
        let now = Instant::now();
        if self.keep_exact_frontier_rbc_repair_in_slot(key, &roster, now) {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "keeping RBC delivery recovery inside exact frontier slot"
            );
            return;
        }
        let topology = crate::sumeragi::network_topology::Topology::new(roster);
        let retry_window = self.control_plane_rebroadcast_cooldown();
        let defer_view_change = self.should_defer_missing_block_view_change(&key.0, key.1, key.2);
        let view_change_window = if defer_view_change {
            None
        } else {
            Some(self.quorum_timeout(self.runtime_da_enabled()))
        };
        let mut signers = self.rbc_missing_block_preferred_signers(key);
        if signers.is_empty() {
            let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
            let signature_topology =
                super::topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
            signers = super::normalize_signer_indices_to_canonical(
                &BTreeSet::from([crate::sumeragi::consensus::ValidatorIndex::from(0_u16)]),
                &signature_topology,
                &topology,
            );
        }
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let decision = super::plan_missing_block_fetch_with_mode(
            &mut requests,
            key.0,
            key.1,
            key.2,
            crate::sumeragi::consensus::Phase::Commit,
            super::MissingBlockPriority::Consensus,
            &signers,
            &topology,
            now,
            retry_window,
            view_change_window,
            self.recovery_signer_fallback_attempts(),
            super::MissingBlockFetchMode::StrictSigners,
            false,
        );
        self.pending.missing_block_requests = requests;
        if defer_view_change {
            self.clear_missing_block_view_change(&key.0);
        }
        let dwell = self
            .pending
            .missing_block_requests
            .get(&key.0)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let targets_len = match &decision {
            MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        let dwell_ms = dwell.as_millis().try_into().unwrap_or(u64::MAX);
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
        super::status::record_missing_block_fetch(targets_len, dwell_ms);

        match decision {
            MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(
                    key.0,
                    key.1,
                    key.2,
                    super::MissingBlockPriority::Consensus,
                    &targets,
                );
                info!(
                    height = key.1,
                    view = key.2,
                    block = %key.0,
                    targets = ?targets,
                    target_kind = target_kind.label(),
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "requested missing BlockCreated after RBC delivery"
                );
            }
            MissingBlockFetchDecision::NoTargets => {
                warn!(
                    height = key.1,
                    view = key.2,
                    block = %key.0,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "missing BlockCreated fetch deferred: no targets available"
                );
            }
            MissingBlockFetchDecision::Backoff => {}
        }
    }

    pub(super) fn request_missing_block_after_rbc_drop(
        &mut self,
        key: SessionKey,
        reason: PendingRbcDropReason,
        context: &'static str,
    ) {
        self.request_missing_block_for_pending_rbc(key, context, Some(reason));
    }

    fn keep_exact_frontier_rbc_repair_in_slot(
        &mut self,
        key: SessionKey,
        roster: &[PeerId],
        now: Instant,
    ) -> bool {
        let committed_height = self.committed_height_snapshot();
        let frontier_height = committed_height.saturating_add(1);
        if key.1 <= committed_height {
            self.clear_missing_block_request(&key.0, MissingBlockClearReason::Obsolete);
            self.clear_missing_block_view_change(&key.0);
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                committed_height,
                "retiring superseded RBC recovery without generic missing-BlockCreated fetch"
            );
            return true;
        }
        if key.1 == frontier_height.saturating_add(1) {
            self.next_slot_prefetch = Some(super::FrontierPrefetchSlot {
                height: key.1,
                view: key.2,
                block_hash: key.0,
            });
            self.clear_missing_block_request(&key.0, MissingBlockClearReason::Obsolete);
            self.clear_missing_block_view_change(&key.0);
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "staging next-slot prefetch without generic missing-BlockCreated fetch"
            );
            return true;
        }
        if key.1 != frontier_height {
            return false;
        }
        if self.frontier_block_materialized_locally(key.0) && !self.block_known_locally(key.0) {
            return false;
        }
        if roster.is_empty() {
            if self.update_frontier_slot(
                key.0,
                key.1,
                key.2,
                None,
                BTreeSet::new(),
                /*block_created_seen*/ false,
                /*exact_fetch_armed*/ true,
                self.frontier_block_materialized_locally(key.0),
                None,
                None,
                now,
            ) {
                self.clear_missing_block_request(&key.0, MissingBlockClearReason::Obsolete);
                self.clear_missing_block_view_change(&key.0);
            }
            return true;
        }

        let topology = crate::sumeragi::network_topology::Topology::new(roster.to_vec());
        let mut signers = self.rbc_missing_block_preferred_signers(key);
        if signers.is_empty() {
            let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
            let signature_topology =
                super::topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
            signers = super::normalize_signer_indices_to_canonical(
                &BTreeSet::from([crate::sumeragi::consensus::ValidatorIndex::from(0_u16)]),
                &signature_topology,
                &topology,
            );
        }

        let _ = self.handle_frontier_body_gap_with_topology(
            key.0, key.1, key.2, &signers, &topology, /*exact_fetch_armed*/ true, now,
        );
        self.clear_missing_block_request(&key.0, MissingBlockClearReason::Obsolete);
        self.clear_missing_block_view_change(&key.0);
        true
    }

    fn suppress_far_future_rbc_missing_block_fetch(
        &mut self,
        key: SessionKey,
        context: &'static str,
    ) -> bool {
        let frontier_height = self.committed_height_snapshot().saturating_add(1);
        if key.1 <= frontier_height.saturating_add(1) {
            return false;
        }

        let now = Instant::now();
        if self.pending.missing_block_requests.contains_key(&key.0) {
            self.clear_missing_block_request(&key.0, MissingBlockClearReason::Obsolete);
        }
        self.clear_missing_block_recovery_for_height(key.1, now);

        let has_rbc_state = self.subsystems.da_rbc.rbc.sessions.contains_key(&key)
            || self.subsystems.da_rbc.rbc.pending.contains_key(&key)
            || self
                .subsystems
                .da_rbc
                .rbc
                .session_rosters
                .contains_key(&key)
            || self
                .subsystems
                .da_rbc
                .rbc
                .payload_rebroadcast_last_sent
                .contains_key(&key)
            || self
                .subsystems
                .da_rbc
                .rbc
                .ready_rebroadcast_last_sent
                .contains_key(&key)
            || self
                .subsystems
                .da_rbc
                .rbc
                .deliver_rebroadcast_last_sent
                .contains_key(&key)
            || self.subsystems.da_rbc.rbc.ready_deferral.contains_key(&key)
            || self
                .subsystems
                .da_rbc
                .rbc
                .deliver_deferral
                .contains_key(&key)
            || self
                .subsystems
                .da_rbc
                .rbc
                .outbound_chunks
                .contains_key(&key)
            || self
                .subsystems
                .da_rbc
                .rbc
                .persisted_full_sessions
                .contains(&key)
            || self.subsystems.da_rbc.rbc.persist_inflight.contains(&key);
        if has_rbc_state {
            self.purge_rbc_state(key, key.0, key.1, key.2);
        }

        let reanchor_requested = self.request_range_pull_from_anchor_with_tier(
            frontier_height,
            "rbc_far_future_missing_block",
            now,
            Some(super::RangePullCandidateTier::CommitTopology),
        );
        debug!(
            height = key.1,
            view = key.2,
            frontier_height,
            block = %key.0,
            context,
            had_rbc_state = has_rbc_state,
            reanchor_requested,
            "suppressing far-future RBC missing-block fetch beyond contiguous frontier"
        );
        true
    }

    pub(super) fn request_missing_block_for_pending_rbc(
        &mut self,
        key: SessionKey,
        context: &'static str,
        reason: Option<PendingRbcDropReason>,
    ) {
        if self.block_known_locally(key.0) {
            return;
        }
        if self.suppress_far_future_rbc_missing_block_fetch(key, context) {
            return;
        }
        let mut roster = self.ensure_rbc_session_roster(key);
        if roster.is_empty() {
            roster = self.effective_commit_topology();
            if roster.is_empty() {
                match reason {
                    Some(reason) => {
                        debug!(
                            height = key.1,
                            view = key.2,
                            block = %key.0,
                            drop_reason = reason.as_str(),
                            context,
                            "skipping missing BlockCreated fetch after RBC drop: empty roster"
                        );
                    }
                    None => {
                        debug!(
                            height = key.1,
                            view = key.2,
                            block = %key.0,
                            context,
                            "skipping missing BlockCreated fetch for pending RBC: empty roster"
                        );
                    }
                }
                return;
            }
            match reason {
                Some(reason) => {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        drop_reason = reason.as_str(),
                        context,
                        "using fallback commit topology for missing BlockCreated fetch after RBC drop"
                    );
                }
                None => {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        context,
                        "using fallback commit topology for missing BlockCreated fetch while awaiting RBC INIT"
                    );
                }
            }
        }
        let now = Instant::now();
        if self.keep_exact_frontier_rbc_repair_in_slot(key, &roster, now) {
            match reason {
                Some(reason) => {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        drop_reason = reason.as_str(),
                        context,
                        "keeping pending RBC recovery inside exact frontier slot"
                    );
                }
                None => {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        context,
                        "keeping RBC INIT recovery inside exact frontier slot"
                    );
                }
            }
            return;
        }
        let topology = crate::sumeragi::network_topology::Topology::new(roster);
        let retry_window = self.control_plane_rebroadcast_cooldown();
        let defer_view_change = self.should_defer_missing_block_view_change(&key.0, key.1, key.2);
        let view_change_window = if defer_view_change {
            None
        } else {
            Some(self.quorum_timeout(self.runtime_da_enabled()))
        };
        let fetch_gate = self.missing_block_ingress_fetch_gate(
            key.0,
            key.1,
            key.2,
            crate::sumeragi::consensus::Phase::Commit,
            super::MissingBlockPriority::Consensus,
            now,
            retry_window,
            view_change_window,
        );
        if matches!(fetch_gate, super::MissingBlockIngressFetchGate::Hold) {
            if defer_view_change {
                self.clear_missing_block_view_change(&key.0);
            }
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                context,
                grace_ms = self.authoritative_body_ingress_fetch_grace().as_millis(),
                "holding missing BlockCreated recovery within authoritative body ingress grace"
            );
            return;
        }
        let mut signers = self.rbc_missing_block_preferred_signers(key);
        if signers.is_empty() {
            let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
            let signature_topology =
                super::topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
            signers = super::normalize_signer_indices_to_canonical(
                &BTreeSet::from([crate::sumeragi::consensus::ValidatorIndex::from(0_u16)]),
                &signature_topology,
                &topology,
            );
        }
        let fetch_mode = self.pending_rbc_missing_block_fetch_mode(key, reason);
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let decision = super::plan_missing_block_fetch_with_mode(
            &mut requests,
            key.0,
            key.1,
            key.2,
            crate::sumeragi::consensus::Phase::Commit,
            super::MissingBlockPriority::Consensus,
            &signers,
            &topology,
            now,
            retry_window,
            view_change_window,
            self.recovery_signer_fallback_attempts(),
            fetch_mode,
            matches!(
                fetch_gate,
                super::MissingBlockIngressFetchGate::Fetch {
                    force_retry_now: true
                }
            ),
        );
        self.pending.missing_block_requests = requests;
        if defer_view_change {
            self.clear_missing_block_view_change(&key.0);
        }
        let dwell = self
            .pending
            .missing_block_requests
            .get(&key.0)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let targets_len = match &decision {
            MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        let dwell_ms = dwell.as_millis().try_into().unwrap_or(u64::MAX);
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
        super::status::record_missing_block_fetch(targets_len, dwell_ms);

        match decision {
            MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(
                    key.0,
                    key.1,
                    key.2,
                    super::MissingBlockPriority::Consensus,
                    &targets,
                );
                match reason {
                    Some(reason) => {
                        info!(
                            height = key.1,
                            view = key.2,
                            block = %key.0,
                            drop_reason = reason.as_str(),
                            context,
                            targets = ?targets,
                            target_kind = target_kind.label(),
                            retry_window_ms = retry_window.as_millis(),
                            dwell_ms = dwell.as_millis(),
                            "requested missing BlockCreated after RBC drop"
                        );
                    }
                    None => {
                        info!(
                            height = key.1,
                            view = key.2,
                            block = %key.0,
                            context,
                            targets = ?targets,
                            target_kind = target_kind.label(),
                            retry_window_ms = retry_window.as_millis(),
                            dwell_ms = dwell.as_millis(),
                            "requested missing BlockCreated while awaiting RBC INIT"
                        );
                    }
                }
            }
            MissingBlockFetchDecision::NoTargets => match reason {
                Some(reason) => {
                    warn!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        drop_reason = reason.as_str(),
                        context,
                        retry_window_ms = retry_window.as_millis(),
                        dwell_ms = dwell.as_millis(),
                        "missing BlockCreated fetch deferred after RBC drop: no targets available"
                    );
                }
                None => {
                    warn!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        context,
                        retry_window_ms = retry_window.as_millis(),
                        dwell_ms = dwell.as_millis(),
                        "missing BlockCreated fetch deferred while awaiting RBC INIT: no targets available"
                    );
                }
            },
            MissingBlockFetchDecision::Backoff => {}
        }
    }

    pub(super) fn pending_rbc_missing_block_fetch_mode(
        &self,
        key: SessionKey,
        reason: Option<PendingRbcDropReason>,
    ) -> super::MissingBlockFetchMode {
        let _ = (key, reason);
        super::MissingBlockFetchMode::StrictSigners
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
        let has_active_pending = self.pending.pending_blocks.contains_key(block_hash);
        let pending_aborted_payload =
            self.pending
                .pending_blocks
                .get(block_hash)
                .is_some_and(|pending| {
                    pending.aborted
                        && !matches!(
                            pending.validation_status,
                            super::pending_block::ValidationStatus::Invalid
                        )
                });
        let committed_height = self.last_committed_height;
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
        if self.runtime_da_enabled()
            && kind != "RbcInit"
            && !self.rbc_session_roster(key).is_empty()
        {
            debug!(
                height,
                view,
                local_view,
                committed_height,
                block = %block_hash,
                kind,
                "accepting RBC message for stale view with tracked session roster"
            );
            return false;
        }
        if self.runtime_da_enabled()
            && (pending_aborted_payload
                || (has_active_pending && !self.block_payload_available_locally(*block_hash)))
        {
            if height > committed_height {
                let reason = if pending_aborted_payload {
                    "pending block aborted"
                } else {
                    "payload missing"
                };
                debug!(
                    height,
                    view,
                    local_view,
                    committed_height,
                    block = %block_hash,
                    kind,
                    reason,
                    "accepting RBC message for stale view while availability is unresolved"
                );
                return false;
            }
        }
        if self.runtime_da_enabled() && self.rbc_rebroadcast_active(key) {
            debug!(
                height,
                view,
                local_view,
                block = %block_hash,
                kind,
                "accepting RBC message for stale view on active block"
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
    pub(super) fn handle_rbc_init(&mut self, init: RbcInit, sender: Option<PeerId>) -> Result<()> {
        if self.should_drop_stale_rbc_message(init.height, init.view, &init.block_hash, "RbcInit") {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Committed,
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadTooLarge,
            );
            return Ok(());
        }
        let expected_len = usize::try_from(init.total_chunks).unwrap_or(usize::MAX);
        if init.chunk_digests.len() != expected_len {
            warn!(
                height = init.height,
                view = init.view,
                total_chunks = init.total_chunks,
                digests = init.chunk_digests.len(),
                "rejecting RBC init with mismatched chunk digest count"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ChunkDigestMismatch,
            );
            return Ok(());
        }

        // 1. Basic Roster Validation
        if init.roster.is_empty() {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                "rejecting RBC init with empty roster snapshot"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::RosterMissing,
            );
            return Ok(());
        }
        if roster_has_duplicates(&init.roster) {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                "rejecting RBC init with duplicate roster entries"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let roster_hash = rbc_roster_hash(&init.roster);
        if roster_hash != init.roster_hash {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                expected = ?roster_hash,
                observed = ?init.roster_hash,
                "rejecting RBC init with mismatched roster hash"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::RosterHashMismatch,
            );
            return Ok(());
        }

        // 2. Consistency Checks (Session & Roster)
        // Check against derived roster *before* signature verification to ensure we use the correct leader derivation context if available.
        let derived_roster = self.rbc_roster_for_session(key);
        let derived_roster_missing = derived_roster.is_empty();
        if !derived_roster_missing && derived_roster != init.roster {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                "rejecting RBC init with roster snapshot that mismatches local commit topology"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::RosterHashMismatch,
            );
            return Ok(());
        }

        // Check against existing session to fail fast on conflicts.
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
            if let Some(expected_hash) = session.payload_hash() {
                if expected_hash != init.payload_hash {
                    warn!(
                        ?expected_hash,
                        observed = ?init.payload_hash,
                        height = init.height,
                        view = init.view,
                        "dropping RBC INIT that mismatches existing payload hash"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::PayloadMismatch,
                    );
                    return Ok(());
                }
            }
            if let Some(existing) = session.block_header {
                let existing = normalize_rbc_block_header(existing);
                let incoming = normalize_rbc_block_header(init.block_header);
                if existing != incoming {
                    warn!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        "dropping RBC INIT that mismatches existing block header"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::PayloadMismatch,
                    );
                    return Ok(());
                }
            }
            if let Some(existing) = session.leader_signature.as_ref() {
                if existing != &init.leader_signature {
                    warn!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        "dropping RBC INIT that mismatches existing leader signature"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::InvalidSignature,
                    );
                    return Ok(());
                }
            }
            if session.total_chunks != init.total_chunks {
                warn!(
                    height = init.height,
                    view = init.view,
                    expected = session.total_chunks,
                    observed = init.total_chunks,
                    "dropping RBC INIT that mismatches existing chunk count"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcInit,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::PayloadMismatch,
                );
                return Ok(());
            }
            if let Some(expected) = session.expected_chunk_digests.as_ref() {
                if expected != &init.chunk_digests {
                    warn!(
                        height = init.height,
                        view = init.view,
                        "dropping RBC INIT that mismatches existing chunk digests"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::ChunkDigestMismatch,
                    );
                    return Ok(());
                }
            }
            let inferred_chunk_root_only = session.expected_chunk_root.is_some()
                && session.expected_chunk_digests.is_none()
                && session.payload_hash().is_none();
            if let Some(root) = session.expected_chunk_root {
                if !inferred_chunk_root_only && root != init.chunk_root {
                    warn!(
                        height = init.height,
                        view = init.view,
                        ?root,
                        observed = ?init.chunk_root,
                        "dropping RBC INIT that mismatches existing chunk root"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::ChunkRootMismatch,
                    );
                    return Ok(());
                }
            }
        }

        // 3. Header & Authority Checks
        let header_hash = init.block_header.hash();
        if header_hash != init.block_hash {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                expected = ?init.block_hash,
                observed = ?header_hash,
                "rejecting RBC init with mismatched block header hash"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadMismatch,
            );
            return Ok(());
        }
        if init.block_header.height().get() != init.height
            || init.block_header.view_change_index() != init.view
        {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                "rejecting RBC init with header height/view mismatch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let mut signature_topology =
            crate::sumeragi::network_topology::Topology::new(init.roster.clone());
        let expected_leader_index =
            match self.leader_index_for(&mut signature_topology, init.height, init.view) {
                Ok(idx) => idx,
                Err(err) => {
                    warn!(
                        ?err,
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        "rejecting RBC init: failed to derive leader index"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::InvalidPayload,
                    );
                    return Ok(());
                }
            };
        let Some(leader_peer) = signature_topology.as_ref().get(expected_leader_index) else {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                "rejecting RBC init: leader index out of range"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        };
        let expected_leader_index = u64::try_from(expected_leader_index).unwrap_or(u64::MAX);
        if init.leader_signature.index() != expected_leader_index {
            warn!(
                height = init.height,
                view = init.view,
                block = %init.block_hash,
                expected = expected_leader_index,
                observed = init.leader_signature.index(),
                "rejecting RBC init: leader signature index mismatch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            return Ok(());
        }
        if init
            .leader_signature
            .signature()
            .verify_hash(leader_peer.public_key(), header_hash)
            .is_err()
        {
            if let Some(sender) = sender {
                // If the leader signature is invalid, penalize the sender (who might be the leader, or relaying bad data).
                // Note: If the sender != leader, they are relaying bad data which is also penalizable (spam).
                let sender_index = signature_topology
                    .position(sender.public_key())
                    .and_then(|idx| u64::try_from(idx).ok())
                    .unwrap_or(u64::MAX);
                let outcome = self.record_invalid_signature(
                    InvalidSigKind::RbcInit,
                    init.height,
                    init.view,
                    sender_index,
                );
                if matches!(outcome, InvalidSigOutcome::Logged) {
                    warn!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        sender = %sender,
                        sender_index,
                        "rejecting RBC init: leader signature invalid"
                    );
                } else {
                    debug!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        sender = %sender,
                        sender_index,
                        "suppressing repeated invalid RBC init signature log"
                    );
                }
            } else {
                warn!(
                    height = init.height,
                    view = init.view,
                    block = %init.block_hash,
                    "rejecting RBC init: leader signature invalid (unknown sender)"
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            return Ok(());
        }

        // 4. Integrity Checks (Expensive)
        // Perform the expensive Merkle root calculation only after verifying the leader's signature and consistency.
        let computed_root =
            MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(init.chunk_digests.clone())
                .root()
                .map(Hash::from);
        if computed_root != Some(init.chunk_root) {
            warn!(
                height = init.height,
                view = init.view,
                expected = ?init.chunk_root,
                observed = ?computed_root,
                "rejecting RBC init with mismatched chunk digests root"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcInit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ChunkRootMismatch,
            );
            return Ok(());
        }
        let init_layout = match rbc_layout_from_init(&init) {
            Ok(layout) => layout,
            Err(err) => {
                warn!(
                    height = init.height,
                    view = init.view,
                    block = %init.block_hash,
                    "rejecting RBC init with invalid layout metadata: {err}"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcInit,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidPayload,
                );
                return Ok(());
            }
        };

        let (roster, roster_source) = if derived_roster_missing {
            let committed_height = self.last_committed_height;
            let committed_epoch = self.epoch_for_height(committed_height);
            let (consensus_mode, _, _) = self.consensus_context_for_height(init.height);
            let fallback_roster = {
                let world = self.state.world_view();
                let commit_topology = self.state.commit_topology_snapshot();
                let height = self.last_committed_height;
                let roster = self.active_topology_with_genesis_fallback_from_world(
                    &world,
                    commit_topology.as_slice(),
                    height,
                    consensus_mode,
                );
                super::roster::canonicalize_roster_for_mode(roster, consensus_mode)
            };
            if !fallback_roster.is_empty() && fallback_roster == init.roster {
                debug!(
                    height = init.height,
                    view = init.view,
                    block = %init.block_hash,
                    roster_len = init.roster.len(),
                    committed_height,
                    committed_epoch,
                    session_epoch = expected_epoch,
                    "using active topology for RBC session; derived roster unavailable but matches init roster"
                );
                (init.roster.clone(), RbcRosterSource::Derived)
            } else {
                debug!(
                    height = init.height,
                    view = init.view,
                    block = %init.block_hash,
                    roster_len = init.roster.len(),
                    committed_height,
                    committed_epoch,
                    session_epoch = expected_epoch,
                    "using init roster for RBC session; derived roster unavailable"
                );
                (init.roster.clone(), RbcRosterSource::Init)
            }
        } else {
            (derived_roster, RbcRosterSource::Derived)
        };
        if let Some(existing_roster) = self.subsystems.da_rbc.rbc.session_rosters.get(&key) {
            let existing_hash = rbc_roster_hash(existing_roster);
            if existing_hash != init.roster_hash {
                let source = self
                    .rbc_session_roster_source(key)
                    .unwrap_or(RbcRosterSource::Init);
                if source.is_authoritative() {
                    warn!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        expected = ?existing_hash,
                        observed = ?init.roster_hash,
                        "rejecting RBC init with conflicting roster snapshot"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::RosterHashMismatch,
                    );
                    return Ok(());
                }
                if !roster_source.is_authoritative() {
                    warn!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        expected = ?existing_hash,
                        observed = ?init.roster_hash,
                        "ignoring RBC init with conflicting unverified roster snapshot"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::RosterHashMismatch,
                    );
                    return Ok(());
                }
            }
        }
        self.cache_vote_roster(init.block_hash, init.height, init.view, roster.clone());
        self.record_rbc_session_roster(key, roster, roster_source);
        let frontier_height = self.committed_height_snapshot().saturating_add(1);
        let near_frontier = key.1 <= frontier_height.saturating_add(1);
        let needs_missing_roster_recovery = near_frontier && !roster_source.is_authoritative();
        if let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) {
            if session.layout().payload_size_known() && init_layout.payload_size_known() {
                if session.layout() != init_layout {
                    warn!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        "dropping RBC init that mismatches existing payload layout"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcInit,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::PayloadMismatch,
                    );
                    self.subsystems.da_rbc.rbc.sessions.insert(key, session);
                    return Ok(());
                }
            } else if init_layout.payload_size_known() {
                session.layout = init_layout;
            }
            if session.payload_hash().is_none() {
                session.payload_hash = Some(init.payload_hash);
            }
            if session.expected_chunk_digests.is_none() {
                session.expected_chunk_digests = Some(init.chunk_digests.clone());
            }
            let dropped = session.drop_mismatched_chunks();
            if dropped > 0 {
                warn!(
                    height = init.height,
                    view = init.view,
                    dropped,
                    "dropping cached RBC chunks that mismatch INIT digests"
                );
            }
            if session.expected_chunk_root.is_none() {
                session.expected_chunk_root = Some(init.chunk_root);
            }
            session.epoch = init.epoch;
            let preserved_result_merkle_root = session
                .block_header
                .as_ref()
                .and_then(|header| header.result_merkle_root);
            let mut merged_header = init.block_header;
            if merged_header.result_merkle_root.is_none() {
                merged_header.result_merkle_root = preserved_result_merkle_root;
            }
            session.block_header = Some(merged_header);
            session.leader_signature = Some(init.leader_signature);
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            self.post_rbc_init_payload_recovery(
                key,
                sender.as_ref(),
                needs_missing_roster_recovery,
            )?;
            self.flush_pending_rbc(key)?;
            if !self.should_defer_rbc_ready_after_init(key) {
                self.maybe_emit_rbc_ready(key)?;
            }
            if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                self.update_rbc_status_entry(key, &session, false);
                self.persist_rbc_session(key, &session);
            }
            self.publish_rbc_backlog_snapshot();
            self.request_commit_pipeline_for_round(
                key.1,
                key.2,
                super::status::RoundPhaseTrace::WaitBlock,
                super::status::RoundEventCauseTrace::BlockAvailable,
                None,
            );
            return Ok(());
        }
        let session = match RbcSession::new_with_layout(
            init_layout,
            init.total_chunks,
            Some(init.payload_hash),
            Some(init.chunk_root),
            Some(init.chunk_digests.clone()),
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
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcInit,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidPayload,
                );
                return Ok(());
            }
        };
        let mut session = session;
        let mut merged_header = init.block_header;
        if merged_header.result_merkle_root.is_none() {
            merged_header.result_merkle_root = session
                .block_header
                .and_then(|header| header.result_merkle_root);
        }
        session.block_header = Some(merged_header);
        session.leader_signature = Some(init.leader_signature);
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
        self.post_rbc_init_payload_recovery(key, sender.as_ref(), needs_missing_roster_recovery)?;
        // Apply any messages that arrived before INIT once the session exists.
        self.flush_pending_rbc(key)?;
        if !self.should_defer_rbc_ready_after_init(key) {
            self.maybe_emit_rbc_ready(key)?;
        }
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &session, false);
            self.persist_rbc_session(key, &session);
        }
        self.publish_rbc_backlog_snapshot();
        self.request_commit_pipeline_for_round(
            key.1,
            key.2,
            super::status::RoundPhaseTrace::WaitBlock,
            super::status::RoundEventCauseTrace::BlockAvailable,
            None,
        );
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_rbc_chunk(
        &mut self,
        chunk: RbcChunk,
        sender: Option<PeerId>,
    ) -> Result<()> {
        let chunk_height = chunk.height;
        let chunk_view = chunk.view;
        let chunk_idx = chunk.idx;
        let chunk_block = chunk.block_hash;
        let chunk_bytes_len = chunk.bytes.len();
        debug!(
            height = chunk_height,
            view = chunk_view,
            idx = chunk_idx,
            bytes = chunk_bytes_len,
            block = %chunk_block,
            sender = ?sender,
            "received RBC chunk"
        );
        if self.should_drop_stale_rbc_message(chunk_height, chunk_view, &chunk_block, "RbcChunk") {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcChunk,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(chunk_height);
        if chunk.epoch != expected_epoch {
            warn!(
                height = chunk_height,
                view = chunk_view,
                block = %chunk_block,
                expected = expected_epoch,
                observed = chunk.epoch,
                "dropping RBC chunk with mismatched epoch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcChunk,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
            );
            return Ok(());
        }
        let max_chunk_bytes = self.config.rbc.chunk_max_bytes.max(1);
        if chunk_bytes_len > max_chunk_bytes {
            warn!(
                height = chunk_height,
                view = chunk_view,
                idx = chunk_idx,
                chunk_len = chunk_bytes_len,
                max = max_chunk_bytes,
                "dropping RBC chunk that exceeds configured size cap"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcChunk,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadTooLarge,
            );
            return Ok(());
        }
        if self.rbc_message_stale(&chunk_block, chunk_height) {
            debug!(
                height = chunk_height,
                view = chunk_view,
                block = %chunk_block,
                "dropping RBC chunk for committed block"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcChunk,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Committed,
            );
            return Ok(());
        }
        let key = Self::session_key(&chunk_block, chunk_height, chunk_view);
        let had_session = self.subsystems.da_rbc.rbc.sessions.contains_key(&key);
        if !had_session {
            let rebuilt = self.ensure_rbc_session_from_pending_block(key, true)?;
            if rebuilt {
                debug!(
                    height = chunk_height,
                    view = chunk_view,
                    block = %chunk_block,
                    "reconstructed RBC session from pending block for incoming chunk"
                );
            }
        }
        let mut chunk_digest_mismatch = false;
        let (ready_sent_before, became_complete, accepted_chunk) = if let Some(session) =
            self.subsystems.da_rbc.rbc.sessions.get_mut(&key)
        {
            let ready_sent_before = session.sent_ready;
            let was_complete =
                session.total_chunks() != 0 && session.received_chunks() == session.total_chunks();
            let outcome = session.ingest_chunk_with_outcome(chunk.idx, chunk.bytes, None);
            if matches!(outcome, super::ChunkIngestOutcome::DigestMismatch) {
                chunk_digest_mismatch = true;
            }
            let accepted_chunk = matches!(outcome, super::ChunkIngestOutcome::Accepted);
            let is_complete =
                session.total_chunks() != 0 && session.received_chunks() == session.total_chunks();
            debug!(
                height = chunk_height,
                view = chunk_view,
                idx = chunk_idx,
                block = %chunk_block,
                sender = ?sender,
                outcome = ?outcome,
                accepted_chunk,
                received_chunks = session.received_chunks(),
                total_chunks = session.total_chunks(),
                "ingested RBC chunk"
            );
            (
                ready_sent_before,
                !was_complete && is_complete,
                accepted_chunk,
            )
        } else {
            let (max_chunks, max_bytes) = self.pending_rbc_caps();
            let chunk_dedup_key = BlockPayloadDedupKey::RbcChunk {
                height: chunk_height,
                view: chunk_view,
                epoch: chunk.epoch,
                block_hash: chunk_block,
                idx: chunk_idx,
                bytes_hash: CryptoHash::new(&chunk.bytes),
            };
            let Some(pending) = self.pending_rbc_slot(key) else {
                let dropped_bytes = u64::try_from(chunk_bytes_len).unwrap_or(u64::MAX);
                Self::record_pending_drop_counts(
                    self.telemetry_handle(),
                    PendingRbcDropReason::SessionLimit,
                    1,
                    dropped_bytes,
                );
                warn!(
                    ?key,
                    limit = self.config.rbc.pending_session_limit,
                    dropped_bytes,
                    "dropping pending RBC chunk: stash session limit reached"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcChunk,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StashSessionLimit,
                );
                self.request_missing_block_after_rbc_drop(
                    key,
                    PendingRbcDropReason::SessionLimit,
                    "rbc_chunk_stash_limit",
                );
                self.release_block_payload_dedup(&chunk_dedup_key);
                return Ok(());
            };
            debug!(
                ?key,
                sender = ?sender,
                idx = chunk_idx,
                "stashing RBC chunk before session init"
            );
            let outcome =
                pending.push_chunk_capped(chunk, sender, max_chunks, max_bytes, Instant::now());
            match outcome {
                PendingChunkOutcome::Inserted {
                    pending_chunks,
                    pending_bytes,
                    evicted_chunks,
                    evicted_bytes,
                } => {
                    status::inc_pending_rbc_stash(status::PendingRbcStashKind::Chunk, None);
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcChunk,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::InitMissing,
                    );
                    if evicted_chunks > 0 {
                        Self::record_pending_drop_counts(
                            self.telemetry_handle(),
                            PendingRbcDropReason::Cap,
                            evicted_chunks,
                            evicted_bytes,
                        );
                        self.request_missing_block_after_rbc_drop(
                            key,
                            PendingRbcDropReason::Cap,
                            "rbc_chunk_stash_evicted",
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
                    if evicted_chunks == 0 {
                        self.request_missing_block_for_pending_rbc(key, "rbc_chunk_stash", None);
                    }
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
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcChunk,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::StashCap,
                    );
                    self.request_missing_block_after_rbc_drop(
                        key,
                        PendingRbcDropReason::Cap,
                        "rbc_chunk_stash_cap",
                    );
                    self.release_block_payload_dedup(&chunk_dedup_key);
                }
            }
            self.publish_rbc_backlog_snapshot();
            return Ok(());
        };
        if accepted_chunk {
            let now = Instant::now();
            self.touch_pending_progress(chunk_block, chunk_height, chunk_view, now);
            let _ = self.note_missing_block_request_dependency_progress(
                chunk_block,
                chunk_height,
                chunk_view,
                now,
                true,
            );
        }
        if chunk_digest_mismatch {
            let log_outcome = sender.as_ref().map(|peer| {
                self.record_rbc_mismatch(
                    peer,
                    status::RbcMismatchKind::ChunkDigest,
                    chunk_height,
                    chunk_view,
                )
            });
            if log_outcome.is_none_or(super::RbcMismatchLogOutcome::should_log) {
                warn!(
                    height = chunk_height,
                    view = chunk_view,
                    idx = chunk_idx,
                    block = %chunk_block,
                    sender = ?sender,
                    "dropping RBC chunk with mismatched digest"
                );
            } else {
                debug!(
                    height = chunk_height,
                    view = chunk_view,
                    idx = chunk_idx,
                    block = %chunk_block,
                    sender = ?sender,
                    "suppressing repeated RBC chunk digest mismatch log"
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcChunk,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ChunkDigestMismatch,
            );
        }
        self.maybe_emit_rbc_ready(key)?;
        if accepted_chunk {
            let delivered_bytes = self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .get_mut(&key)
                .and_then(super::RbcSession::take_delivered_payload_bytes_for_telemetry);
            if let (Some(bytes), Some(telemetry)) = (delivered_bytes, self.telemetry_handle()) {
                telemetry.add_rbc_payload_bytes_delivered(bytes);
            }
        }
        let authoritative_after =
            self.subsystems
                .da_rbc
                .rbc
                .sessions
                .get(&key)
                .is_some_and(|session| {
                    self.rbc_session_has_authoritative_payload_for_progress(key, session)
                });
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
        if authoritative_after {
            self.recover_block_from_rbc_session(key);
            self.request_commit_pipeline_for_round(
                key.1,
                key.2,
                super::status::RoundPhaseTrace::WaitValidation,
                super::status::RoundEventCauseTrace::BlockAvailable,
                None,
            );
        } else if became_complete || (!ready_sent_before && ready_sent_after) {
            self.request_commit_pipeline_for_round(
                key.1,
                key.2,
                super::status::RoundPhaseTrace::WaitDa,
                super::status::RoundEventCauseTrace::BlockAvailable,
                None,
            );
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcReady,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
            return Ok(());
        }
        if self.invalid_sig_penalty.is_suppressed(
            InvalidSigKind::RbcReady,
            u64::from(ready.sender),
            Instant::now(),
        ) {
            debug!(
                height = ready.height,
                view = ready.view,
                sender = ready.sender,
                block = %ready.block_hash,
                "dropping RBC READY from penalized signer"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcReady,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PenalizedSender,
            );
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcReady,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcReady,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Committed,
            );
            return Ok(());
        }
        let key = Self::session_key(&ready.block_hash, ready.height, ready.view);
        let ready_sender = ready.sender;
        let ready_view = ready.view;
        if !self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            if self.ensure_rbc_session_from_pending_block(key, true)? {
                // Session reconstructed from pending block; continue to process READY normally.
            } else {
                let max_bytes = self.pending_rbc_caps().1;
                let ready_dedup_key = BlockPayloadDedupKey::RbcReady {
                    height: ready.height,
                    view: ready.view,
                    block_hash: ready.block_hash,
                    sender: ready.sender,
                    signature_hash: CryptoHash::new(&ready.signature),
                };
                let ready_bytes = rbc_ready_stash_bytes(&ready);
                let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                    let Some(pending) = self.pending_rbc_slot(key) else {
                        let dropped_bytes = u64::try_from(ready_bytes).unwrap_or(u64::MAX);
                        Self::record_pending_drop_counts(
                            self.telemetry_handle(),
                            PendingRbcDropReason::SessionLimit,
                            1,
                            dropped_bytes,
                        );
                        warn!(
                            ?key,
                            limit = self.config.rbc.pending_session_limit,
                            ready_sender,
                            ready_view,
                            dropped_bytes,
                            "dropping pending RBC READY: stash session limit reached before INIT"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcReady,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::StashSessionLimit,
                        );
                        self.request_missing_block_after_rbc_drop(
                            key,
                            PendingRbcDropReason::SessionLimit,
                            "rbc_ready_stash_limit",
                        );
                        self.release_block_payload_dedup(&ready_dedup_key);
                        return Ok(());
                    };
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
                    status::inc_pending_rbc_stash(
                        status::PendingRbcStashKind::Ready,
                        Some(status::PendingRbcStashReason::InitMissing),
                    );
                    info!(
                        ?key,
                        pending_chunks,
                        pending_bytes,
                        local_peer = %self.common_config.peer.id(),
                        ready_sender,
                        ready_view,
                        "stashed RBC READY until INIT arrives"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcReady,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::InitMissing,
                    );
                    self.request_missing_block_for_pending_rbc(key, "rbc_ready_stash", None);
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
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcReady,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::StashCap,
                    );
                    self.request_missing_block_after_rbc_drop(
                        key,
                        PendingRbcDropReason::Cap,
                        "rbc_ready_stash_cap",
                    );
                    self.release_block_payload_dedup(&ready_dedup_key);
                }
                self.publish_rbc_backlog_snapshot();
                return Ok(());
            }
        }
        let mut topology_peers = self.rbc_session_roster(key);
        let mut roster_source = self
            .rbc_session_roster_source(key)
            .unwrap_or(RbcRosterSource::Init);
        let prior_roster_source = roster_source;
        if !roster_source.is_authoritative() {
            if let Some((refreshed, _updated)) = self.refresh_derived_rbc_session_roster(key) {
                if !refreshed.is_empty() {
                    topology_peers = refreshed;
                }
                roster_source = self
                    .rbc_session_roster_source(key)
                    .unwrap_or(RbcRosterSource::Init);
            }
        }
        if prior_roster_source != roster_source && roster_source.is_authoritative() {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                sender = ready_sender,
                "refreshed RBC roster to derived snapshot"
            );
        }
        fn stash_ready(
            actor: &mut Actor,
            key: SessionKey,
            ready: RbcReady,
            reason: &'static str,
            handling_reason: super::status::ConsensusMessageReason,
            stash_reason: status::PendingRbcStashReason,
        ) -> Result<()> {
            let ready_sender = ready.sender;
            let ready_view = ready.view;
            let max_bytes = actor.pending_rbc_caps().1;
            let ready_dedup_key = BlockPayloadDedupKey::RbcReady {
                height: ready.height,
                view: ready.view,
                block_hash: ready.block_hash,
                sender: ready.sender,
                signature_hash: CryptoHash::new(&ready.signature),
            };
            let ready_bytes = rbc_ready_stash_bytes(&ready);
            let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                let Some(pending) = actor.pending_rbc_slot(key) else {
                    let dropped_bytes = u64::try_from(ready_bytes).unwrap_or(u64::MAX);
                    Actor::record_pending_drop_counts(
                        actor.telemetry_handle(),
                        PendingRbcDropReason::SessionLimit,
                        1,
                        dropped_bytes,
                    );
                    warn!(
                        ?key,
                        limit = actor.config.rbc.pending_session_limit,
                        reason,
                        ready_sender,
                        ready_view,
                        dropped_bytes,
                        "dropping pending RBC READY: stash session limit reached"
                    );
                    actor.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcReady,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::StashSessionLimit,
                    );
                    actor.request_missing_block_after_rbc_drop(
                        key,
                        PendingRbcDropReason::SessionLimit,
                        "rbc_ready_stash_limit",
                    );
                    actor.release_block_payload_dedup(&ready_dedup_key);
                    return Ok(());
                };
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
                status::inc_pending_rbc_stash(
                    status::PendingRbcStashKind::Ready,
                    Some(stash_reason),
                );
                info!(
                    ?key,
                    pending_chunks,
                    pending_bytes,
                    local_peer = %actor.common_config.peer.id(),
                    ready_sender,
                    ready_view,
                    reason,
                    "stashed RBC READY while awaiting roster resolution"
                );
                actor.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcReady,
                    super::status::ConsensusMessageOutcome::Deferred,
                    handling_reason,
                );
            } else {
                warn!(
                    ?key,
                    max_bytes, reason, "dropping pending RBC READY: stash limits exceeded"
                );
                Actor::record_pending_drop_counts(
                    actor.telemetry_handle(),
                    PendingRbcDropReason::Cap,
                    1,
                    u64::try_from(dropped_bytes).unwrap_or(u64::MAX),
                );
                actor.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcReady,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StashCap,
                );
                actor.request_missing_block_after_rbc_drop(
                    key,
                    PendingRbcDropReason::Cap,
                    "rbc_ready_stash_cap",
                );
                actor.release_block_payload_dedup(&ready_dedup_key);
            }
            actor.publish_rbc_backlog_snapshot();
            Ok(())
        }
        if topology_peers.is_empty() {
            let committed_height = self.last_committed_height;
            let committed_epoch = self.epoch_for_height(committed_height);
            let session_epoch = self.epoch_for_height(key.1);
            let payload_known = self.block_known_locally(key.0);
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                local_peer = %self.common_config.peer.id(),
                sender = ready_sender,
                roster_source = ?roster_source,
                committed_height,
                committed_epoch,
                session_epoch,
                payload_known,
                "deferring RBC READY: commit roster unavailable"
            );
            stash_ready(
                self,
                key,
                ready,
                "commit roster missing",
                super::status::ConsensusMessageReason::RosterMissingDeferred,
                status::PendingRbcStashReason::RosterMissing,
            )?;
            self.request_missing_block_for_pending_rbc(key, "rbc_ready_roster_missing", None);
            return Ok(());
        }
        let mut roster_hash = rbc_roster_hash(&topology_peers);
        if roster_hash != ready.roster_hash && !roster_source.is_authoritative() {
            if let Some((refreshed, _updated)) = self.refresh_derived_rbc_session_roster(key) {
                topology_peers = refreshed;
                roster_hash = rbc_roster_hash(&topology_peers);
                roster_source = self
                    .rbc_session_roster_source(key)
                    .unwrap_or(RbcRosterSource::Init);
            }
        }
        if roster_hash != ready.roster_hash {
            if roster_source.is_authoritative() {
                warn!(
                    height = ready.height,
                    view = ready.view,
                    sender = ready.sender,
                    expected = ?roster_hash,
                    observed = ?ready.roster_hash,
                    "dropping RBC READY: roster hash mismatch"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcReady,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::RosterHashMismatch,
                );
                return Ok(());
            }
            debug!(
                height = ready.height,
                view = ready.view,
                local_peer = %self.common_config.peer.id(),
                sender = ready.sender,
                roster_source = ?roster_source,
                expected = ?roster_hash,
                observed = ?ready.roster_hash,
                "deferring RBC READY: roster hash mismatch on derived roster"
            );
            stash_ready(
                self,
                key,
                ready,
                "commit roster hash mismatch",
                super::status::ConsensusMessageReason::RosterHashMismatchDeferred,
                status::PendingRbcStashReason::RosterHashMismatch,
            )?;
            return Ok(());
        }
        let allow_unverified = self.allow_unverified_rbc_roster(key);
        if !roster_source.is_authoritative() && !allow_unverified {
            let derived_len = self.rbc_roster_for_session(key).len();
            debug!(
                height = ready.height,
                view = ready.view,
                local_peer = %self.common_config.peer.id(),
                sender = ready.sender,
                roster_source = ?roster_source,
                roster_len = topology_peers.len(),
                derived_len,
                allow_unverified,
                "deferring RBC READY: roster not authoritative"
            );
            stash_ready(
                self,
                key,
                ready,
                "commit roster unverified",
                super::status::ConsensusMessageReason::RosterUnverifiedDeferred,
                status::PendingRbcStashReason::RosterUnverified,
            )?;
            self.request_missing_block_for_pending_rbc(key, "rbc_ready_unverified_roster", None);
            return Ok(());
        }
        let topology = crate::sumeragi::network_topology::Topology::new(topology_peers);
        let deliver_quorum = self.rbc_deliver_quorum(&topology);
        let authoritative_known_payload = roster_source.is_authoritative()
            && self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .get(&key)
                .is_some_and(|session| {
                    self.rbc_session_has_authoritative_payload_for_progress(key, session)
                });
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(ready.height);
        let signature_topology =
            super::topology_for_view(&topology, ready.height, ready.view, mode_tag, prf_seed);
        let local_idx = self.local_validator_index_for_topology(&signature_topology);
        let ready_peer = usize::try_from(ready.sender)
            .ok()
            .and_then(|idx| signature_topology.as_ref().get(idx));
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
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcReady,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::Duplicate,
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
                    peer = ?ready_peer,
                    topology_len = signature_topology.as_ref().len(),
                    "dropping RBC READY with invalid signature"
                );
            } else {
                debug!(
                    height = ready.height,
                    view = ready.view,
                    sender = ready.sender,
                    peer = ?ready_peer,
                    topology_len = signature_topology.as_ref().len(),
                    "suppressing repeated invalid RBC READY signature log"
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcReady,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            return Ok(());
        }
        let mut inferred_chunk_root: Option<Hash> = None;
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
            match session.expected_chunk_root {
                Some(expected) => {
                    if expected != ready.chunk_root {
                        let log_outcome = ready_peer.map(|peer| {
                            self.record_rbc_mismatch(
                                peer,
                                status::RbcMismatchKind::ChunkRoot,
                                ready.height,
                                ready.view,
                            )
                        });
                        if log_outcome.is_none_or(super::RbcMismatchLogOutcome::should_log) {
                            warn!(
                                height = ready.height,
                                view = ready.view,
                                sender = ready.sender,
                                ?expected,
                                observed = ?ready.chunk_root,
                                "dropping RBC READY with mismatched chunk root"
                            );
                        } else {
                            debug!(
                                height = ready.height,
                                view = ready.view,
                                sender = ready.sender,
                                ?expected,
                                observed = ?ready.chunk_root,
                                "suppressing repeated RBC READY chunk-root mismatch log"
                            );
                        }
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcReady,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::ChunkRootMismatch,
                        );
                        return Ok(());
                    }
                }
                None => {
                    if let Some(computed) = session.chunk_root() {
                        if computed != ready.chunk_root {
                            let log_outcome = ready_peer.map(|peer| {
                                self.record_rbc_mismatch(
                                    peer,
                                    status::RbcMismatchKind::ChunkRoot,
                                    ready.height,
                                    ready.view,
                                )
                            });
                            if log_outcome.is_none_or(super::RbcMismatchLogOutcome::should_log) {
                                warn!(
                                    height = ready.height,
                                    view = ready.view,
                                    sender = ready.sender,
                                    ?computed,
                                    observed = ?ready.chunk_root,
                                    "dropping RBC READY with mismatched computed chunk root"
                                );
                            } else {
                                debug!(
                                    height = ready.height,
                                    view = ready.view,
                                    sender = ready.sender,
                                    ?computed,
                                    observed = ?ready.chunk_root,
                                    "suppressing repeated RBC READY computed chunk-root mismatch log"
                                );
                            }
                            self.record_consensus_message_handling(
                                super::status::ConsensusMessageKind::RbcReady,
                                super::status::ConsensusMessageOutcome::Dropped,
                                super::status::ConsensusMessageReason::ChunkRootMismatch,
                            );
                            return Ok(());
                        }
                        inferred_chunk_root = Some(computed);
                    } else {
                        inferred_chunk_root = Some(ready.chunk_root);
                    }
                }
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
            if session.expected_chunk_root.is_none() {
                if let Some(root) = inferred_chunk_root {
                    session.expected_chunk_root = Some(root);
                }
            }
            let was_valid = !session.is_invalid();
            let recorded_ready = session.record_ready_with_roster_hash(
                ready.sender,
                ready.signature.clone(),
                ready.roster_hash,
            );
            ready_count_after = session.ready_signatures.len();
            ready_senders_after = session
                .ready_signatures
                .iter()
                .map(|entry| entry.sender)
                .collect();
            if was_valid && session.is_invalid() {
                conflict_detected = true;
            }
            let _ = session
                .sync_progress_observations(authoritative_known_payload, Some(deliver_quorum));
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
        self.maybe_emit_rbc_ready(key)?;
        let delivered_after = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .is_some_and(|session| session.delivered);
        let deliver_emitted = !delivered_before && delivered_after;
        if recorded_ready {
            let now = Instant::now();
            self.touch_pending_progress(ready.block_hash, ready.height, ready.view, now);
            let _ = self.note_missing_block_request_dependency_progress(
                ready.block_hash,
                ready.height,
                ready.view,
                now,
                true,
            );
        }
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
                let msg = Arc::new(BlockMessage::RbcReady(ready.clone()));
                let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                for peer in gossip_targets {
                    self.schedule_background(BackgroundRequest::Post {
                        peer,
                        msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
                    });
                }
            }
        }
        if should_process_commit_after_ready(recorded_ready, clear_pending, deliver_emitted) {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            let consensus_queue_backlog = queue_depths.vote_rx > 0
                || queue_depths.block_payload_rx > 0
                || queue_depths.rbc_chunk_rx > 0
                || queue_depths.block_rx > 0;
            if consensus_queue_backlog {
                debug!(
                    height = key.1,
                    view = key.2,
                    vote_rx_depth = queue_depths.vote_rx,
                    block_payload_rx_depth = queue_depths.block_payload_rx,
                    rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                    block_rx_depth = queue_depths.block_rx,
                    "consensus queue backlog detected while handling RBC READY"
                );
            }
            self.request_commit_pipeline_for_round(
                key.1,
                key.2,
                super::status::RoundPhaseTrace::WaitDa,
                super::status::RoundEventCauseTrace::BlockAvailable,
                None,
            );
        }
        Ok(())
    }

    fn evaluate_rbc_deliver_outcome(
        deliver_quorum: usize,
        session: &mut RbcSession,
        key: SessionKey,
        deliver: &crate::sumeragi::consensus::RbcDeliver,
        allow_missing_chunks: bool,
    ) -> (
        bool,
        bool,
        Option<u64>,
        bool,
        bool,
        Option<String>,
        Option<super::status::ConsensusMessageReason>,
    ) {
        let mut ignored = false;
        let mut first_deliver = false;
        let mut delivered_bytes: Option<u64> = None;
        let mut invalidate = false;
        let mut chunk_root_mismatch = false;
        let mut defer_reason: Option<String> = None;
        let mut defer_kind: Option<super::status::ConsensusMessageReason> = None;
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
                chunk_root_mismatch,
                defer_reason,
                defer_kind,
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
                chunk_root_mismatch,
                defer_reason,
                defer_kind,
            );
        }
        if !session.is_invalid() {
            let acceptance = if allow_missing_chunks {
                evaluate_deliver_acceptance_with_policy(session, deliver_quorum, true)
            } else {
                evaluate_deliver_acceptance(session, deliver_quorum)
            };
            match acceptance {
                DeliverAcceptance::DeferReady {
                    ready_count,
                    required,
                } => {
                    status::inc_rbc_deliver_defer_ready();
                    defer_reason = Some(format!(
                        "deferring RBC DELIVER until READY quorum is satisfied (ready_count={ready_count}, required={required})"
                    ));
                    defer_kind = Some(super::status::ConsensusMessageReason::ReadyQuorumMissing);
                }
                DeliverAcceptance::DeferChunks { received, total } => {
                    status::inc_rbc_deliver_defer_chunks();
                    defer_reason = Some(format!(
                        "deferring RBC DELIVER until all chunks are present (received={received}, total={total})"
                    ));
                    defer_kind = Some(super::status::ConsensusMessageReason::ChunksMissing);
                }
                DeliverAcceptance::InvalidChunkRoot => {
                    session.invalid = true;
                    invalidate = true;
                    ignored = true;
                    chunk_root_mismatch = true;
                }
                DeliverAcceptance::Accept => {
                    first_deliver =
                        session.record_deliver(deliver.sender, deliver.signature.clone());
                    if first_deliver {
                        status::record_round_gap_deliver(key.1, key.2, key.0);
                        delivered_bytes = session.take_delivered_payload_bytes_for_telemetry();
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
            chunk_root_mismatch,
            defer_reason,
            defer_kind,
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcDeliver,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
            return Ok(());
        }
        let now = Instant::now();
        if self.invalid_sig_penalty.is_suppressed(
            InvalidSigKind::RbcDeliver,
            u64::from(deliver.sender),
            now,
        ) {
            debug!(
                height = deliver.height,
                view = deliver.view,
                sender = deliver.sender,
                block = %deliver.block_hash,
                "dropping RBC DELIVER from penalized signer"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcDeliver,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PenalizedSender,
            );
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcDeliver,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcDeliver,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Committed,
            );
            return Ok(());
        }
        let key = Self::session_key(&deliver.block_hash, deliver.height, deliver.view);
        if !self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            if self.ensure_rbc_session_from_pending_block(key, true)? {
                // Session reconstructed from pending block; continue to process DELIVER normally.
            } else {
                let max_bytes = self.pending_rbc_caps().1;
                let deliver_dedup_key = BlockPayloadDedupKey::RbcDeliver {
                    height: deliver.height,
                    view: deliver.view,
                    block_hash: deliver.block_hash,
                    sender: deliver.sender,
                    signature_hash: CryptoHash::new(&deliver.signature),
                };
                let deliver_bytes = rbc_deliver_stash_bytes(&deliver);
                let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                    let Some(pending) = self.pending_rbc_slot(key) else {
                        let dropped_bytes = u64::try_from(deliver_bytes).unwrap_or(u64::MAX);
                        Self::record_pending_drop_counts(
                            self.telemetry_handle(),
                            PendingRbcDropReason::SessionLimit,
                            1,
                            dropped_bytes,
                        );
                        warn!(
                            ?key,
                            limit = self.config.rbc.pending_session_limit,
                            dropped_bytes,
                            "dropping pending RBC DELIVER: stash session limit reached before INIT"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcDeliver,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::StashSessionLimit,
                        );
                        self.request_missing_block_after_rbc_drop(
                            key,
                            PendingRbcDropReason::SessionLimit,
                            "rbc_deliver_stash_limit",
                        );
                        self.release_block_payload_dedup(&deliver_dedup_key);
                        return Ok(());
                    };
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
                    status::inc_pending_rbc_stash(
                        status::PendingRbcStashKind::Deliver,
                        Some(status::PendingRbcStashReason::InitMissing),
                    );
                    debug!(
                        ?key,
                        pending_chunks, pending_bytes, "stashed RBC DELIVER until INIT arrives"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcDeliver,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::InitMissing,
                    );
                    self.request_missing_block_for_pending_rbc(key, "rbc_deliver_stash", None);
                } else {
                    warn!(
                        ?key,
                        max_bytes,
                        "dropping pending RBC DELIVER: stash limits exceeded before INIT"
                    );
                    Self::record_pending_drop_counts(
                        self.telemetry_handle(),
                        PendingRbcDropReason::Cap,
                        1,
                        u64::try_from(dropped_bytes).unwrap_or(u64::MAX),
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcDeliver,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::StashCap,
                    );
                    self.request_missing_block_after_rbc_drop(
                        key,
                        PendingRbcDropReason::Cap,
                        "rbc_deliver_stash_cap",
                    );
                    self.release_block_payload_dedup(&deliver_dedup_key);
                }
                self.publish_rbc_backlog_snapshot();
                return Ok(());
            }
        }
        let mut topology_peers = self.rbc_session_roster(key);
        let mut roster_source = self
            .rbc_session_roster_source(key)
            .unwrap_or(RbcRosterSource::Init);
        let mut roster_updated = false;
        let prior_roster_source = roster_source;
        if !roster_source.is_authoritative() {
            if let Some((refreshed, updated)) = self.refresh_derived_rbc_session_roster(key) {
                if !refreshed.is_empty() {
                    topology_peers = refreshed;
                }
                roster_source = self
                    .rbc_session_roster_source(key)
                    .unwrap_or(RbcRosterSource::Init);
                roster_updated |= updated;
            }
        }
        if prior_roster_source != roster_source && roster_source.is_authoritative() {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                sender = deliver.sender,
                "refreshed RBC roster to derived snapshot"
            );
        }
        fn stash_deliver(
            actor: &mut Actor,
            key: SessionKey,
            deliver: RbcDeliver,
            reason: &'static str,
            stash_reason: status::PendingRbcStashReason,
        ) -> Result<()> {
            let handling_reason = match stash_reason {
                status::PendingRbcStashReason::RosterMissing => {
                    super::status::ConsensusMessageReason::RosterMissingDeferred
                }
                status::PendingRbcStashReason::RosterHashMismatch => {
                    super::status::ConsensusMessageReason::RosterHashMismatchDeferred
                }
                status::PendingRbcStashReason::RosterUnverified => {
                    super::status::ConsensusMessageReason::RosterUnverifiedDeferred
                }
                status::PendingRbcStashReason::InitMissing => {
                    super::status::ConsensusMessageReason::InitMissing
                }
            };
            let max_bytes = actor.pending_rbc_caps().1;
            let deliver_dedup_key = BlockPayloadDedupKey::RbcDeliver {
                height: deliver.height,
                view: deliver.view,
                block_hash: deliver.block_hash,
                sender: deliver.sender,
                signature_hash: CryptoHash::new(&deliver.signature),
            };
            let deliver_bytes = rbc_deliver_stash_bytes(&deliver);
            let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                let Some(pending) = actor.pending_rbc_slot(key) else {
                    let dropped_bytes = u64::try_from(deliver_bytes).unwrap_or(u64::MAX);
                    Actor::record_pending_drop_counts(
                        actor.telemetry_handle(),
                        PendingRbcDropReason::SessionLimit,
                        1,
                        dropped_bytes,
                    );
                    warn!(
                        ?key,
                        limit = actor.config.rbc.pending_session_limit,
                        reason,
                        dropped_bytes,
                        "dropping pending RBC DELIVER: stash session limit reached"
                    );
                    actor.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcDeliver,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::StashSessionLimit,
                    );
                    actor.request_missing_block_after_rbc_drop(
                        key,
                        PendingRbcDropReason::SessionLimit,
                        "rbc_deliver_stash_limit",
                    );
                    actor.release_block_payload_dedup(&deliver_dedup_key);
                    return Ok(());
                };
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
                status::inc_pending_rbc_stash(
                    status::PendingRbcStashKind::Deliver,
                    Some(stash_reason),
                );
                debug!(
                    ?key,
                    pending_chunks,
                    pending_bytes,
                    reason,
                    "stashed RBC DELIVER while awaiting roster resolution"
                );
                actor.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcDeliver,
                    super::status::ConsensusMessageOutcome::Deferred,
                    handling_reason,
                );
            } else {
                warn!(
                    ?key,
                    max_bytes, reason, "dropping pending RBC DELIVER: stash limits exceeded"
                );
                Actor::record_pending_drop_counts(
                    actor.telemetry_handle(),
                    PendingRbcDropReason::Cap,
                    1,
                    u64::try_from(dropped_bytes).unwrap_or(u64::MAX),
                );
                actor.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcDeliver,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StashCap,
                );
                actor.request_missing_block_after_rbc_drop(
                    key,
                    PendingRbcDropReason::Cap,
                    "rbc_deliver_stash_cap",
                );
                actor.release_block_payload_dedup(&deliver_dedup_key);
            }
            actor.publish_rbc_backlog_snapshot();
            Ok(())
        }
        if topology_peers.is_empty() {
            let committed_height = self.last_committed_height;
            let committed_epoch = self.epoch_for_height(committed_height);
            let session_epoch = self.epoch_for_height(key.1);
            let payload_known = self.block_known_locally(key.0);
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                sender = deliver.sender,
                roster_source = ?roster_source,
                committed_height,
                committed_epoch,
                session_epoch,
                payload_known,
                "deferring RBC DELIVER: commit roster unavailable"
            );
            stash_deliver(
                self,
                key,
                deliver,
                "commit roster missing",
                status::PendingRbcStashReason::RosterMissing,
            )?;
            self.request_missing_block_for_pending_rbc(key, "rbc_deliver_roster_missing", None);
            return Ok(());
        }
        let mut roster_hash = rbc_roster_hash(&topology_peers);
        if roster_hash != deliver.roster_hash && !roster_source.is_authoritative() {
            if let Some((refreshed, updated)) = self.refresh_derived_rbc_session_roster(key) {
                topology_peers = refreshed;
                roster_updated |= updated;
                roster_hash = rbc_roster_hash(&topology_peers);
                roster_source = self
                    .rbc_session_roster_source(key)
                    .unwrap_or(RbcRosterSource::Init);
            }
        }
        if roster_hash != deliver.roster_hash {
            if roster_source.is_authoritative() {
                warn!(
                    height = deliver.height,
                    view = deliver.view,
                    sender = deliver.sender,
                    expected = ?roster_hash,
                    observed = ?deliver.roster_hash,
                    "dropping RBC DELIVER: roster hash mismatch"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcDeliver,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::RosterHashMismatch,
                );
                return Ok(());
            }
            stash_deliver(
                self,
                key,
                deliver,
                "commit roster hash mismatch",
                status::PendingRbcStashReason::RosterHashMismatch,
            )?;
            return Ok(());
        }
        let allow_unverified = self.allow_unverified_rbc_roster(key);
        if !roster_source.is_authoritative() && !allow_unverified {
            let derived_len = self.rbc_roster_for_session(key).len();
            debug!(
                height = deliver.height,
                view = deliver.view,
                sender = deliver.sender,
                roster_source = ?roster_source,
                roster_len = topology_peers.len(),
                derived_len,
                "deferring RBC DELIVER: roster not authoritative"
            );
            stash_deliver(
                self,
                key,
                deliver,
                "commit roster unverified",
                status::PendingRbcStashReason::RosterUnverified,
            )?;
            self.request_missing_block_for_pending_rbc(key, "rbc_deliver_unverified_roster", None);
            return Ok(());
        }
        let topology = crate::sumeragi::network_topology::Topology::new(topology_peers);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(deliver.height);
        let signature_topology =
            super::topology_for_view(&topology, deliver.height, deliver.view, mode_tag, prf_seed);
        let deliver_peer = usize::try_from(deliver.sender)
            .ok()
            .and_then(|idx| signature_topology.as_ref().get(idx));
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
                    peer = ?deliver_peer,
                    "dropping RBC DELIVER with invalid signature"
                );
            } else {
                debug!(
                    height = deliver.height,
                    view = deliver.view,
                    sender = deliver.sender,
                    peer = ?deliver_peer,
                    "suppressing repeated invalid RBC DELIVER signature log"
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcDeliver,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            return Ok(());
        }
        let mut inferred_chunk_root: Option<Hash> = None;
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
            match session.expected_chunk_root {
                Some(expected) => {
                    if expected != deliver.chunk_root {
                        warn!(
                            height = deliver.height,
                            view = deliver.view,
                            sender = deliver.sender,
                            peer = ?deliver_peer,
                            ?expected,
                            observed = ?deliver.chunk_root,
                            "dropping RBC DELIVER with mismatched chunk root"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcDeliver,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::ChunkRootMismatch,
                        );
                        return Ok(());
                    }
                }
                None => {
                    if let Some(computed) = session.chunk_root() {
                        if computed != deliver.chunk_root {
                            warn!(
                                height = deliver.height,
                                view = deliver.view,
                                sender = deliver.sender,
                                peer = ?deliver_peer,
                                ?computed,
                                observed = ?deliver.chunk_root,
                                "dropping RBC DELIVER with mismatched computed chunk root"
                            );
                            self.record_consensus_message_handling(
                                super::status::ConsensusMessageKind::RbcDeliver,
                                super::status::ConsensusMessageOutcome::Dropped,
                                super::status::ConsensusMessageReason::ChunkRootMismatch,
                            );
                            return Ok(());
                        }
                        inferred_chunk_root = Some(computed);
                    } else {
                        inferred_chunk_root = Some(deliver.chunk_root);
                    }
                }
            }
        }
        let mut ready_to_record: Vec<(u32, Vec<u8>)> = Vec::new();
        let mut invalid_ready_senders: Vec<u32> = Vec::new();
        let mut suppressed_ready_senders: Vec<u32> = Vec::new();
        if !deliver.ready_signatures.is_empty() {
            let max_ready = signature_topology.as_ref().len();
            if deliver.ready_signatures.len() > max_ready {
                debug!(
                    height = deliver.height,
                    view = deliver.view,
                    sender = deliver.sender,
                    ready_entries = deliver.ready_signatures.len(),
                    max_ready,
                    "truncating oversized RBC DELIVER READY bundle"
                );
            }
            for entry in deliver.ready_signatures.iter().take(max_ready) {
                let ready = RbcReady {
                    block_hash: deliver.block_hash,
                    height: deliver.height,
                    view: deliver.view,
                    epoch: deliver.epoch,
                    roster_hash: deliver.roster_hash,
                    chunk_root: deliver.chunk_root,
                    sender: entry.sender,
                    signature: entry.signature.clone(),
                };
                if self.invalid_sig_penalty.is_suppressed(
                    InvalidSigKind::RbcReady,
                    u64::from(entry.sender),
                    now,
                ) {
                    suppressed_ready_senders.push(entry.sender);
                    continue;
                }
                if !rbc_ready_signature_valid(
                    &ready,
                    &signature_topology,
                    &self.common_config.chain,
                    mode_tag,
                ) {
                    invalid_ready_senders.push(entry.sender);
                    continue;
                }
                ready_to_record.push((entry.sender, entry.signature.clone()));
            }
        }
        let deliver_quorum = self.rbc_deliver_quorum(&topology);
        let authoritative_known_payload = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .is_some_and(|session| {
                self.rbc_session_has_authoritative_payload_for_progress(key, session)
            });
        let allow_missing_chunks = authoritative_known_payload;
        let (
            ignored,
            first_deliver,
            delivered_bytes,
            invalidate,
            chunk_root_mismatch,
            defer_reason,
            defer_kind,
        ) = {
            let session = self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .get_mut(&key)
                .expect("session presence checked before validation");
            if session.expected_chunk_root.is_none() {
                if let Some(root) = inferred_chunk_root {
                    session.expected_chunk_root = Some(root);
                }
            }
            for (sender, signature) in ready_to_record {
                session.record_ready_with_roster_hash(sender, signature, deliver.roster_hash);
            }
            let _ = session
                .sync_progress_observations(authoritative_known_payload, Some(deliver_quorum));
            Self::evaluate_rbc_deliver_outcome(0, session, key, &deliver, allow_missing_chunks)
        };
        if chunk_root_mismatch {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::RbcDeliver,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ChunkRootMismatch,
            );
            let peer = usize::try_from(deliver.sender)
                .ok()
                .and_then(|idx| signature_topology.as_ref().get(idx));
            if let Some(peer) = peer {
                let log_outcome = self.record_rbc_mismatch(
                    peer,
                    status::RbcMismatchKind::ChunkRoot,
                    deliver.height,
                    deliver.view,
                );
                if log_outcome.should_log() {
                    warn!(
                        height = deliver.height,
                        view = deliver.view,
                        sender = deliver.sender,
                        "ignoring RBC DELIVER: chunk root mismatch detected"
                    );
                } else {
                    debug!(
                        height = deliver.height,
                        view = deliver.view,
                        sender = deliver.sender,
                        "suppressing repeated RBC DELIVER chunk-root mismatch log"
                    );
                }
            } else {
                warn!(
                    height = deliver.height,
                    view = deliver.view,
                    sender = deliver.sender,
                    "ignoring RBC DELIVER: chunk root mismatch from unknown sender"
                );
            }
        }
        if !invalid_ready_senders.is_empty() {
            for sender in invalid_ready_senders {
                let outcome = self.record_invalid_signature(
                    InvalidSigKind::RbcReady,
                    deliver.height,
                    deliver.view,
                    u64::from(sender),
                );
                if matches!(outcome, InvalidSigOutcome::Logged) {
                    warn!(
                        height = deliver.height,
                        view = deliver.view,
                        sender,
                        topology_len = signature_topology.as_ref().len(),
                        "dropping RBC READY from DELIVER bundle with invalid signature"
                    );
                } else {
                    debug!(
                        height = deliver.height,
                        view = deliver.view,
                        sender,
                        topology_len = signature_topology.as_ref().len(),
                        "suppressing repeated invalid RBC READY signature log"
                    );
                }
            }
        }
        if !suppressed_ready_senders.is_empty() {
            debug!(
                height = deliver.height,
                view = deliver.view,
                suppressed = suppressed_ready_senders.len(),
                "dropping RBC READY entries from penalized senders"
            );
        }
        self.maybe_emit_rbc_ready(key)?;
        if let Some(reason) = defer_reason {
            if let Some(defer_kind) = defer_kind {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcDeliver,
                    super::status::ConsensusMessageOutcome::Deferred,
                    defer_kind,
                );
            }
            if matches!(
                defer_kind,
                Some(super::status::ConsensusMessageReason::ReadyQuorumMissing)
            ) {
                if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
                    let ready_senders: BTreeSet<_> = session
                        .ready_signatures
                        .iter()
                        .map(|entry| entry.sender)
                        .collect();
                    let mut missing_ready_total = 0usize;
                    let mut missing_ready = Vec::new();
                    let mut missing_ready_peers = Vec::new();
                    for (idx, peer) in signature_topology.as_ref().iter().enumerate() {
                        let idx = match super::ValidatorIndex::try_from(idx) {
                            Ok(idx) => idx,
                            Err(_) => continue,
                        };
                        if ready_senders.contains(&idx) {
                            continue;
                        }
                        missing_ready_total = missing_ready_total.saturating_add(1);
                        if missing_ready.len() < super::READY_MISSING_LOG_LIMIT {
                            missing_ready.push(idx);
                            missing_ready_peers.push(peer.clone());
                        }
                    }
                    let (pending_ready_total, pending_ready, pending_ready_peers) = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .pending
                        .get(&key)
                        .map(|pending| {
                            let senders: BTreeSet<_> =
                                pending.ready.iter().map(|ready| ready.sender).collect();
                            let pending_ready_total = senders.len();
                            let mut pending_ready = Vec::new();
                            let mut pending_ready_peers = Vec::new();
                            for sender in
                                senders.iter().take(super::READY_MISSING_LOG_LIMIT).copied()
                            {
                                pending_ready.push(sender);
                                if let Ok(idx) = usize::try_from(sender) {
                                    if let Some(peer) = signature_topology.as_ref().get(idx) {
                                        pending_ready_peers.push(peer.clone());
                                    }
                                }
                            }
                            (pending_ready_total, pending_ready, pending_ready_peers)
                        })
                        .unwrap_or_else(|| (0, Vec::new(), Vec::new()));
                    let local_ready_sender =
                        self.local_validator_index_for_topology(&signature_topology);
                    let local_ready_sent =
                        local_ready_sender.is_some_and(|idx| ready_senders.contains(&idx));
                    let local_ready_deferral =
                        self.subsystems.da_rbc.rbc.ready_deferral.get(&key).map(
                            |entry| match entry.reason {
                                super::RbcReadyDeferralReason::CommitRosterMissing => {
                                    "commit_roster_missing"
                                }
                                super::RbcReadyDeferralReason::CommitRosterUnverified => {
                                    "commit_roster_unverified"
                                }
                                super::RbcReadyDeferralReason::MissingPayload => "missing_payload",
                                super::RbcReadyDeferralReason::ChunkRootMissing => {
                                    "chunk_root_missing"
                                }
                                super::RbcReadyDeferralReason::LocalNotInCommitTopology => {
                                    "local_not_in_commit_topology"
                                }
                            },
                        );
                    iroha_logger::debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        local_peer = %self.common_config.peer.id(),
                        sender = deliver.sender,
                        peer = ?deliver_peer,
                        roster_source = ?roster_source,
                        local_ready_sender = ?local_ready_sender,
                        local_ready_sent,
                        local_ready_deferral = ?local_ready_deferral,
                        ready = ready_senders.len(),
                        required = deliver_quorum,
                        missing_ready_total,
                        missing_ready = ?missing_ready,
                        missing_ready_peers = ?missing_ready_peers,
                        pending_ready_total,
                        pending_ready = ?pending_ready,
                        pending_ready_peers = ?pending_ready_peers,
                        senders = ?ready_senders.iter().copied().collect::<Vec<_>>(),
                        "deferring RBC DELIVER: READY quorum not yet satisfied"
                    );
                }
            }
            let sender = deliver.sender;
            let max_bytes = self.pending_rbc_caps().1;
            let deliver_dedup_key = BlockPayloadDedupKey::RbcDeliver {
                height: deliver.height,
                view: deliver.view,
                block_hash: deliver.block_hash,
                sender: deliver.sender,
                signature_hash: CryptoHash::new(&deliver.signature),
            };
            let deliver_bytes = rbc_deliver_stash_bytes(&deliver);
            let (accepted, dropped_bytes, pending_chunks, pending_bytes) = {
                let Some(pending) = self.pending_rbc_slot(key) else {
                    let dropped_bytes = u64::try_from(deliver_bytes).unwrap_or(u64::MAX);
                    Self::record_pending_drop_counts(
                        self.telemetry_handle(),
                        PendingRbcDropReason::SessionLimit,
                        1,
                        dropped_bytes,
                    );
                    warn!(
                        height = key.1,
                        view = key.2,
                        sender,
                        limit = self.config.rbc.pending_session_limit,
                        dropped_bytes,
                        "dropping deferred RBC DELIVER: stash session limit reached"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::RbcDeliver,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::StashSessionLimit,
                    );
                    self.request_missing_block_after_rbc_drop(
                        key,
                        PendingRbcDropReason::SessionLimit,
                        "rbc_deliver_defer_limit",
                    );
                    self.release_block_payload_dedup(&deliver_dedup_key);
                    return Ok(());
                };
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
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::RbcDeliver,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StashCap,
                );
                self.request_missing_block_after_rbc_drop(
                    key,
                    PendingRbcDropReason::Cap,
                    "rbc_deliver_defer_cap",
                );
                self.release_block_payload_dedup(&deliver_dedup_key);
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
        if self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .is_some_and(|session| session.delivered || session.is_invalid())
        {
            self.subsystems.da_rbc.rbc.ready_deferral.remove(&key);
            self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
        }
        if roster_updated && self.subsystems.da_rbc.rbc.pending.contains_key(&key) {
            self.flush_pending_rbc(key)?;
        }
        self.publish_rbc_backlog_snapshot();
        if ignored {
            return Ok(());
        }
        self.touch_pending_progress(deliver.block_hash, deliver.height, deliver.view, now);
        let _ = self.note_missing_block_request_dependency_progress(
            deliver.block_hash,
            deliver.height,
            deliver.view,
            now,
            true,
        );
        if first_deliver {
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.inc_rbc_deliver_broadcasts();
                if let Some(bytes) = delivered_bytes {
                    telemetry.add_rbc_payload_bytes_delivered(bytes);
                }
            }
        }
        if self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
            self.record_phase_sample(PipelinePhase::Commit, deliver.height, deliver.view);
        }
        // If the block header never arrived (missed `BlockCreated`), request it from peers once
        // the full RBC payload is delivered so validation and votes can proceed.
        self.recover_block_from_rbc_session(key);
        let _ = should_process_commit_after_deliver(first_deliver);
        self.request_commit_pipeline_for_round(
            deliver.height,
            deliver.view,
            super::status::RoundPhaseTrace::WaitValidation,
            super::status::RoundEventCauseTrace::RbcDelivered,
            None,
        );
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
                    let session_keys: Vec<_> = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .sessions
                        .keys()
                        .copied()
                        .collect();
                    for session_key in session_keys {
                        let roster_source = self
                            .rbc_session_roster_source(session_key)
                            .unwrap_or(RbcRosterSource::Init);
                        if !roster_source.is_authoritative() {
                            continue;
                        }
                        let session_roster = self.rbc_session_roster(session_key);
                        if session_roster.is_empty() {
                            continue;
                        }
                        let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&session_key)
                        else {
                            continue;
                        };
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

    pub(crate) fn attach_rbc_persist_worker(&mut self) -> Option<std::thread::JoinHandle<()>> {
        if self.subsystems.da_rbc.rbc.persist_tx.is_some() {
            return None;
        }
        let cfg = self.subsystems.da_rbc.rbc.store_cfg.clone()?;
        if cfg.max_sessions == 0 || cfg.max_bytes == 0 {
            return None;
        }
        match spawn_rbc_persist_worker(cfg, self.wake_tx.clone()) {
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

    pub(crate) fn attach_rbc_seed_worker(&mut self) -> Option<std::thread::JoinHandle<()>> {
        if self.subsystems.da_rbc.rbc.seed_tx.is_some() {
            return None;
        }
        match spawn_rbc_seed_worker(self.wake_tx.clone()) {
            Ok(handle) => {
                self.subsystems.da_rbc.rbc.seed_tx = Some(handle.work_tx);
                self.subsystems.da_rbc.rbc.seed_rx = Some(handle.result_rx);
                self.subsystems.da_rbc.rbc.seed_inflight.clear();
                Some(handle.join_handle)
            }
            Err(err) => {
                warn!(?err, "failed to spawn RBC seed worker thread");
                None
            }
        }
    }

    pub(in crate::sumeragi) fn poll_rbc_persist_results_inner(&mut self) -> bool {
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
            self.subsystems
                .da_rbc
                .rbc
                .persist_inflight
                .remove(&result.key);
            match result.outcome {
                Ok(outcome) => {
                    self.handle_rbc_store_evictions(&outcome.removed);
                    self.update_rbc_store_pressure(outcome.pressure);
                    if self
                        .subsystems
                        .da_rbc
                        .rbc
                        .sessions
                        .contains_key(&result.key)
                    {
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

    pub(in crate::sumeragi) fn poll_rbc_seed_results_inner(&mut self) -> bool {
        let Some(rx) = self.subsystems.da_rbc.rbc.seed_rx.as_ref() else {
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
            self.subsystems.da_rbc.rbc.seed_tx = None;
            self.subsystems.da_rbc.rbc.seed_rx = None;
            self.subsystems.da_rbc.rbc.seed_inflight.clear();
        }
        if drained.is_empty() {
            return false;
        }
        let mut progress = false;
        for result in drained {
            let Some(intent) = self.subsystems.da_rbc.rbc.seed_inflight.remove(&result.key) else {
                continue;
            };
            progress = true;
            let key = result.key;
            match result.outcome {
                Ok(mut seeded) => {
                    if seeded.payload_hash.is_none() {
                        seeded.payload_hash = Some(result.payload_hash);
                    }
                    let mut session = if let Some(existing) =
                        self.subsystems.da_rbc.rbc.sessions.remove(&key)
                    {
                        let mut mismatch = existing.total_chunks != seeded.total_chunks;
                        if existing.epoch != seeded.epoch {
                            mismatch = true;
                        }
                        if let Some(existing_hash) = existing.payload_hash() {
                            if seeded.payload_hash != Some(existing_hash) {
                                mismatch = true;
                            }
                        }
                        if let (Some(expected), Some(seed_digests)) = (
                            existing.expected_chunk_digests.as_ref(),
                            seeded.expected_chunk_digests.as_ref(),
                        ) {
                            if expected != seed_digests {
                                mismatch = true;
                            }
                        }
                        if let (Some(expected), Some(seed_root)) =
                            (existing.expected_chunk_root, seeded.expected_chunk_root)
                        {
                            if expected != seed_root {
                                mismatch = true;
                            }
                        }
                        if mismatch {
                            warn!(
                                height = key.1,
                                view = key.2,
                                block = %key.0,
                                "seeded RBC session mismatched existing metadata; marking invalid"
                            );
                            let mut existing = existing;
                            existing.invalid = true;
                            existing
                        } else {
                            let mut existing = existing;
                            if existing.payload_hash.is_none() {
                                existing.payload_hash = seeded.payload_hash;
                            }
                            if existing.expected_chunk_digests.is_none() {
                                existing.expected_chunk_digests = seeded.expected_chunk_digests;
                            }
                            if existing.expected_chunk_root.is_none() {
                                existing.expected_chunk_root = seeded.expected_chunk_root;
                            }
                            if existing.received_chunks < existing.total_chunks {
                                existing.chunks = seeded.chunks;
                                existing.received_chunks = seeded.received_chunks;
                            }
                            existing
                        }
                    } else {
                        seeded
                    };

                    if session.block_header.is_none() || session.leader_signature.is_none() {
                        if let Some(pending) = self.pending.pending_blocks.get(&key.0) {
                            if session.block_header.is_none() {
                                session.block_header = Some(pending.block.header());
                            }
                            if session.leader_signature.is_none() {
                                let roster = self.rbc_roster_for_session(key);
                                if !roster.is_empty() {
                                    let mut topology =
                                        super::network_topology::Topology::new(roster);
                                    match self.leader_index_for(&mut topology, key.1, key.2) {
                                        Ok(leader_index) => {
                                            let leader_index =
                                                u64::try_from(leader_index).unwrap_or(u64::MAX);
                                            if let Some(signature) = pending
                                                .block
                                                .signatures()
                                                .find(|signature| signature.index() == leader_index)
                                            {
                                                session.leader_signature = Some(signature.clone());
                                            } else {
                                                debug!(
                                                    height = key.1,
                                                    view = key.2,
                                                    expected = leader_index,
                                                    "leader signature missing after RBC seed"
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            debug!(
                                                height = key.1,
                                                view = key.2,
                                                ?err,
                                                "failed to derive leader index after RBC seed"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }

                    self.subsystems.da_rbc.rbc.sessions.insert(key, session);
                    let roster = self.rbc_roster_for_session(key);
                    if roster.is_empty() {
                        self.ensure_rbc_session_roster(key);
                    } else {
                        self.record_rbc_session_roster(key, roster, RbcRosterSource::Derived);
                    }

                    let invalid = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .sessions
                        .get(&key)
                        .is_some_and(RbcSession::is_invalid);
                    if invalid {
                        self.clear_pending_rbc(&key);
                    } else if let Err(err) = self.flush_pending_rbc(key) {
                        warn!(
                            ?err,
                            ?key,
                            "failed to flush pending RBC after seed completion"
                        );
                    }

                    if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                        self.update_rbc_status_entry(key, &session, false);
                        self.persist_rbc_session(key, &session);
                    }
                    self.publish_rbc_backlog_snapshot();
                    if let Err(err) = self.maybe_emit_rbc_ready(key) {
                        debug!(
                            height = key.1,
                            view = key.2,
                            ?err,
                            "failed to emit RBC READY after seed completion"
                        );
                    }
                    if intent.rebroadcast_missing_init {
                        if let Some(session) =
                            self.subsystems.da_rbc.rbc.sessions.get(&key).cloned()
                        {
                            self.rebroadcast_rbc_payload_for_missing_init(key, &session);
                        }
                    }
                }
                Err(err) => {
                    warn!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        error = %err,
                        "failed to seed RBC session from payload"
                    );
                }
            }
        }
        progress
    }

    pub(super) fn persist_rbc_session(&mut self, key: SessionKey, session: &RbcSession) {
        let total_chunks = session.total_chunks();
        if total_chunks == 0 {
            return;
        }
        if session.received_chunks() < total_chunks {
            return;
        }
        let roster_source = self
            .rbc_session_roster_source(key)
            .unwrap_or(RbcRosterSource::Init);
        if !roster_source.is_authoritative() {
            return;
        }
        let session_roster = self.rbc_session_roster(key);
        if session_roster.is_empty() {
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
        if self.subsystems.da_rbc.rbc.persist_inflight.contains(&key) {
            return;
        }
        if self.subsystems.da_rbc.rbc.persist_tx.is_some() {
            let persisted = session.to_persisted(
                key,
                self.chain_hash,
                &self.subsystems.da_rbc.rbc.manifest,
                session_roster.as_slice(),
            );
            if let Some(tx) = self.subsystems.da_rbc.rbc.persist_tx.as_ref() {
                match tx.try_send(RbcPersistWork { key, persisted }) {
                    Ok(()) => {
                        self.subsystems.da_rbc.rbc.persist_inflight.insert(key);
                    }
                    Err(mpsc::TrySendError::Full(_work)) => {
                        debug!(
                            ?key,
                            "RBC persist queue full; deferring session persistence"
                        );
                        status::inc_rbc_store_persist_drops();
                        if let Some(telemetry) = self.telemetry_handle() {
                            telemetry.inc_rbc_persist_drops();
                        }
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
                .deliver_rebroadcast_last_sent
                .remove(key);
            self.subsystems.da_rbc.rbc.ready_deferral.remove(key);
            self.subsystems.da_rbc.rbc.deliver_deferral.remove(key);
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

    pub(super) fn prune_stale_rbc_sessions(&mut self, now: SystemTime) -> bool {
        let ttl = self.config.rbc.session_ttl;
        if ttl == Duration::ZERO || self.subsystems.da_rbc.rbc.sessions.is_empty() {
            return false;
        }
        let stale_keys = self
            .subsystems
            .da_rbc
            .rbc
            .status_handle
            .stale_keys(ttl, now);
        if stale_keys.is_empty() {
            return false;
        }

        for key in &stale_keys {
            self.subsystems.da_rbc.rbc.sessions.remove(key);
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
                .deliver_rebroadcast_last_sent
                .remove(key);
            self.subsystems.da_rbc.rbc.ready_deferral.remove(key);
            self.subsystems.da_rbc.rbc.deliver_deferral.remove(key);
            self.subsystems
                .da_rbc
                .rbc
                .persisted_full_sessions
                .remove(key);
            self.subsystems.da_rbc.rbc.persist_inflight.remove(key);
        }

        let chunk_store = if self.ensure_rbc_chunk_store() {
            self.subsystems.da_rbc.rbc.chunk_store.as_ref()
        } else {
            None
        };
        if let Some(store) = chunk_store {
            for key in &stale_keys {
                if let Err(err) = store.remove(key) {
                    warn!(?err, ?key, "failed to remove stale RBC session from store");
                }
            }
        }

        self.publish_rbc_backlog_snapshot();
        true
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
            encoding: session.layout().encoding,
            data_shards: session.layout().data_shards,
            parity_shards: session.layout().parity_shards,
            received_chunks: session.received_chunks(),
            ready_count,
            delivered: session.delivered,
            payload_hash: session.payload_hash(),
            recovered_from_disk: recovered || session.recovered_from_disk(),
            invalid: session.is_invalid(),
            reconstructed_stripes: session.reconstructed_stripes(),
            reconstructable_stripes: session.reconstructable_stripes(),
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
        let ttl = self.config.rbc.pending_ttl;
        let session_limit = self.config.rbc.pending_session_limit;
        Self::update_rbc_backlog_snapshot(
            &self.subsystems.da_rbc.rbc.sessions,
            &self.subsystems.da_rbc.rbc.pending,
            caps,
            ttl,
            session_limit,
            telemetry_ref,
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn update_rbc_backlog_snapshot(
        sessions: &BTreeMap<SessionKey, RbcSession>,
        pending: &BTreeMap<SessionKey, PendingRbcMessages>,
        pending_caps: (usize, usize),
        pending_ttl: Duration,
        pending_session_limit: usize,
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
        pending_snapshot.session_cap = u64::try_from(pending_session_limit).unwrap_or(u64::MAX);
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
