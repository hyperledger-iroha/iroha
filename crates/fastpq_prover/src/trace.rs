//! Stage 1 FASTPQ trace builder.
//!
//! The builder canonicalises transition batches into the row/column layout
//! expected by the FASTPQ AIR. Rows are ordered lexicographically by key,
//! operation rank, and original insertion index. Columns are padded to the
//! next power-of-two trace length and exposed as Goldilocks field elements.

use core::{cmp::max, convert::TryFrom};
#[cfg(feature = "fastpq-gpu")]
use std::sync::Mutex;
#[cfg(feature = "fastpq-gpu")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, OnceLock, RwLock},
};

#[cfg(feature = "fastpq-gpu")]
use crate::gpu;
use crate::{
    Error, Result, StateTransition, TransitionBatch,
    backend::{self, ExecutionMode, PoseidonExecutionMode},
    fft::Planner,
    gadgets::transfer::{self, TransferRowKey},
    pack_bytes, poseidon,
};
#[cfg(feature = "fastpq-gpu")]
use fastpq_isi::poseidon::RATE;
use fastpq_isi::{StarkParameterSet, poseidon::PoseidonSponge as CpuPoseidonSponge};
use iroha_crypto::Hash;
use iroha_data_model::fastpq::TRANSFER_TRANSCRIPTS_METADATA_KEY;
use rayon::prelude::*;

/// Goldilocks modulus used by the FASTPQ AIR.
const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;

/// Sparse Merkle tree height used by the stage 1 trace layout.
const SMT_HEIGHT: usize = transfer::TRANSFER_MERKLE_HEIGHT;

/// Domain tag for hashing metadata payloads.
const METADATA_DOMAIN: &[u8] = b"fastpq:v1:metadata";

/// Domain tag for hashing DS identifiers.
const DSID_DOMAIN: &[u8] = b"fastpq:v1:dsid";

/// Domain tag used for column hashes.
const TRACE_COLUMN_DOMAIN_PREFIX: &str = "fastpq:v1:trace:column:";

/// Domain tag used for Merkle interior nodes.
const TRACE_NODE_DOMAIN: &[u8] = b"fastpq:v1:trace:node";

#[cfg(feature = "fastpq-gpu")]
static POSEIDON_PIPELINE_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_PIPELINE_STATS: OnceLock<Mutex<PoseidonPipelineStats>> = OnceLock::new();
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_MERKLE_GPU_DISABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_COLUMN_GPU_DISABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_MERKLE_GPU_SELF_TEST: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_COLUMN_GPU_SELF_TEST: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "fastpq-gpu")]
const POSEIDON_MERKLE_GPU_MIN_PAIRS: usize = 512;

type PoseidonPipelineObserver = dyn Fn(PoseidonPipelinePolicy, &'static str, Option<backend::GpuBackend>)
    + Send
    + Sync
    + 'static;
static POSEIDON_PIPELINE_OBSERVER: OnceLock<RwLock<Option<Arc<PoseidonPipelineObserver>>>> =
    OnceLock::new();
type PoseidonGpuEventObserver = dyn Fn(&'static str, &'static str, &'static str, Option<backend::GpuBackend>)
    + Send
    + Sync
    + 'static;
static POSEIDON_GPU_EVENT_OBSERVER: OnceLock<RwLock<Option<Arc<PoseidonGpuEventObserver>>>> =
    OnceLock::new();

/// Poseidon pipeline execution policy derived from configuration and runtime detection.
#[derive(Clone, Copy, Debug)]
pub struct PoseidonPipelinePolicy {
    requested: PoseidonExecutionMode,
    resolved: ExecutionMode,
}

impl PoseidonPipelinePolicy {
    /// Construct a policy from the requested override and the resolved backend execution mode.
    #[must_use]
    pub fn new(requested: PoseidonExecutionMode, fallback: ExecutionMode) -> Self {
        let resolved = match requested {
            PoseidonExecutionMode::Auto => fallback,
            PoseidonExecutionMode::Cpu => ExecutionMode::Cpu,
            PoseidonExecutionMode::Gpu => {
                if matches!(fallback, ExecutionMode::Gpu) {
                    ExecutionMode::Gpu
                } else {
                    ExecutionMode::Cpu
                }
            }
        };
        Self {
            requested,
            resolved,
        }
    }

    /// Convenience helper for tests that already operate on a concrete execution mode.
    #[must_use]
    pub fn for_mode(mode: ExecutionMode) -> Self {
        let requested = match mode {
            ExecutionMode::Auto => PoseidonExecutionMode::Auto,
            ExecutionMode::Cpu => PoseidonExecutionMode::Cpu,
            ExecutionMode::Gpu => PoseidonExecutionMode::Gpu,
        };
        Self::new(requested, mode)
    }

    /// Requested override from configuration/CLI.
    #[must_use]
    pub const fn requested(self) -> PoseidonExecutionMode {
        self.requested
    }

    /// Resolved execution mode used by the pipeline.
    #[must_use]
    pub const fn resolved(self) -> ExecutionMode {
        self.resolved
    }

    fn cpu_label(self) -> &'static str {
        if matches!(self.requested, PoseidonExecutionMode::Cpu) {
            "cpu_forced"
        } else {
            "cpu_fallback"
        }
    }
}

fn poseidon_observer_slot() -> &'static RwLock<Option<Arc<PoseidonPipelineObserver>>> {
    POSEIDON_PIPELINE_OBSERVER.get_or_init(|| RwLock::new(None))
}

fn poseidon_gpu_event_observer_slot() -> &'static RwLock<Option<Arc<PoseidonGpuEventObserver>>> {
    POSEIDON_GPU_EVENT_OBSERVER.get_or_init(|| RwLock::new(None))
}

fn notify_poseidon_pipeline_observer(
    policy: PoseidonPipelinePolicy,
    path: &'static str,
    backend: Option<backend::GpuBackend>,
) {
    if let Ok(guard) = poseidon_observer_slot().read()
        && let Some(callback) = guard.clone()
    {
        callback(policy, path, backend);
    }
}

#[cfg(feature = "fastpq-gpu")]
fn notify_poseidon_gpu_event_observer(
    accelerator: &'static str,
    event: &'static str,
    reason: &'static str,
    backend: Option<backend::GpuBackend>,
) {
    if let Ok(guard) = poseidon_gpu_event_observer_slot().read()
        && let Some(callback) = guard.clone()
    {
        callback(accelerator, event, reason, backend);
    }
}

/// Install a callback invoked whenever the Poseidon pipeline resolves to CPU/GPU execution.
pub fn set_poseidon_pipeline_observer<F>(observer: F)
where
    F: Fn(PoseidonPipelinePolicy, &'static str, Option<backend::GpuBackend>)
        + Send
        + Sync
        + 'static,
{
    if let Ok(mut guard) = poseidon_observer_slot().write() {
        *guard = Some(Arc::new(observer));
    }
}

/// Install a callback invoked when a FASTPQ GPU accelerator is disabled or a sampled parity check fails.
pub fn set_poseidon_gpu_event_observer<F>(observer: F)
where
    F: Fn(&'static str, &'static str, &'static str, Option<backend::GpuBackend>)
        + Send
        + Sync
        + 'static,
{
    if let Ok(mut guard) = poseidon_gpu_event_observer_slot().write() {
        *guard = Some(Arc::new(observer));
    }
}

/// Remove the previously registered Poseidon pipeline observer, if any.
pub fn clear_poseidon_pipeline_observer() {
    if let Ok(mut guard) = poseidon_observer_slot().write() {
        guard.take();
    }
}

/// Remove the previously registered FASTPQ GPU accelerator event observer, if any.
pub fn clear_poseidon_gpu_event_observer() {
    if let Ok(mut guard) = poseidon_gpu_event_observer_slot().write() {
        guard.take();
    }
}

/// Representation of a fully padded FASTPQ trace.
#[derive(Debug, Clone)]
pub struct Trace {
    /// Number of real (non-padding) rows.
    pub rows: usize,
    /// Padded trace length (power of two).
    pub padded_len: usize,
    /// Trace columns exposed as Goldilocks field elements.
    pub columns: Vec<TraceColumn>,
    /// Validated transfer gadget witnesses extracted from the batch metadata.
    pub transfer_witnesses: Vec<transfer::TransferGadgetInput>,
    /// Per-selector row counts (excluding padded rows).
    pub row_usage: RowUsage,
}

/// Single trace column with deterministic name and field elements.
#[derive(Debug, Clone)]
pub struct TraceColumn {
    /// Column name used for hashing and debugging.
    pub name: String,
    /// Column values (length equals [`Trace::padded_len`]).
    pub values: Vec<u64>,
}

/// Column digest set containing the leaf hashes plus optional fused parents.
#[derive(Clone, Debug)]
pub struct ColumnDigests {
    /// Poseidon hash for each column (leaf nodes).
    pub leaves: Vec<u64>,
    /// Optional GPU-computed depth-1 parents (same slicing as described in the Stage7-P2 ABI).
    pub fused_parents: Option<Vec<u64>>,
}

impl ColumnDigests {
    /// Create a new digest set from leaves and optional parents.
    pub fn new(leaves: Vec<u64>, fused_parents: Option<Vec<u64>>) -> Self {
        Self {
            leaves,
            fused_parents,
        }
    }

    /// Borrow the leaf hashes.
    #[must_use]
    pub fn leaves(&self) -> &[u64] {
        &self.leaves
    }

    /// Borrow the fused parent hashes, when available.
    #[must_use]
    pub fn fused_parents(&self) -> Option<&[u64]> {
        self.fused_parents.as_deref()
    }
}

/// Intermediate per-row representation before column transposition.
struct RowData {
    key_limbs: Vec<u64>,
    value_old_limbs: Vec<u64>,
    value_new_limbs: Vec<u64>,
    asset_limbs: Vec<u64>,
    delta: u64,
    running_asset_delta: u64,
    supply_counter: u64,
    metadata_hash: u64,
    perm_hash: u64,
    neighbour_leaf: u64,
    selectors: Selectors,
    path_bits: [u64; SMT_HEIGHT],
    siblings: [u64; SMT_HEIGHT],
    node_in: [u64; SMT_HEIGHT],
    node_out: [u64; SMT_HEIGHT],
    dsid: u64,
    slot: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct Selectors {
    active: u64,
    transfer: u64,
    mint: u64,
    burn: u64,
    role_grant: u64,
    role_revoke: u64,
    meta_set: u64,
    perm: u64,
}

impl RowData {
    fn padding(metadata_hash: u64, dsid: u64, slot: u64) -> Self {
        Self {
            key_limbs: Vec::new(),
            value_old_limbs: Vec::new(),
            value_new_limbs: Vec::new(),
            asset_limbs: Vec::new(),
            delta: 0,
            running_asset_delta: 0,
            supply_counter: 0,
            metadata_hash,
            perm_hash: 0,
            neighbour_leaf: 0,
            selectors: Selectors::default(),
            path_bits: [0; SMT_HEIGHT],
            siblings: [0; SMT_HEIGHT],
            node_in: [0; SMT_HEIGHT],
            node_out: [0; SMT_HEIGHT],
            dsid,
            slot,
        }
    }
}

/// Telemetry snapshot for the GPU Poseidon pipelined hashing path.
#[cfg(feature = "fastpq-gpu")]
#[derive(Clone, Copy, Debug, Default)]
pub struct PoseidonPipelineStats {
    /// Whether the streaming pipeline executed.
    pub enabled: bool,
    /// Columns prepped per chunk while streaming.
    pub chunk_columns: u32,
    /// Max buffered chunk count allowed.
    pub pipe_depth: u32,
    /// Number of chunks dispatched to the GPU worker.
    pub batches: u32,
    /// Number of times the pipeline aborted and fell back.
    pub fallbacks: u32,
    /// Number of Merkle parent batches hashed through the GPU pair path.
    pub merkle_pair_gpu_batches: u32,
    /// Number of Merkle parent batches hashed through the scalar pair path.
    pub merkle_pair_cpu_batches: u32,
    /// Number of Merkle parent GPU batches that failed and fell back.
    pub merkle_pair_fallbacks: u32,
    /// Largest Merkle parent pair batch observed while telemetry was enabled.
    pub merkle_pair_max_pairs: u32,
}

/// Enable or disable collection of Poseidon pipeline telemetry.
#[cfg(feature = "fastpq-gpu")]
pub fn enable_poseidon_pipeline_stats(enabled: bool) {
    POSEIDON_PIPELINE_STATS_ENABLED.store(enabled, Ordering::Relaxed);
    if enabled {
        reset_poseidon_pipeline_stats();
    }
}

/// Drain the accumulated Poseidon pipeline stats, if telemetry collection is active.
#[cfg(feature = "fastpq-gpu")]
pub fn take_poseidon_pipeline_stats() -> Option<PoseidonPipelineStats> {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    store.lock().ok().map(|mut guard| {
        let snapshot = *guard;
        *guard = PoseidonPipelineStats::default();
        snapshot
    })
}

#[cfg(feature = "fastpq-gpu")]
fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(feature = "fastpq-gpu")]
fn record_poseidon_pipeline_start(chunk_columns: usize, depth: usize) {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.enabled = true;
        guard.chunk_columns = saturating_u32(chunk_columns);
        guard.pipe_depth = saturating_u32(depth);
        guard.batches = 0;
        guard.fallbacks = 0;
    }
}

#[cfg(feature = "fastpq-gpu")]
fn record_poseidon_pipeline_batch() {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.batches = guard.batches.saturating_add(1);
    }
}

#[cfg(feature = "fastpq-gpu")]
fn record_poseidon_pipeline_fallback() {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.fallbacks = guard.fallbacks.saturating_add(1);
    }
}

#[cfg(feature = "fastpq-gpu")]
fn record_poseidon_merkle_pair_gpu_batch(pair_count: usize) {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.merkle_pair_gpu_batches = guard.merkle_pair_gpu_batches.saturating_add(1);
        guard.merkle_pair_max_pairs = guard.merkle_pair_max_pairs.max(saturating_u32(pair_count));
    }
}

#[cfg(feature = "fastpq-gpu")]
fn record_poseidon_merkle_pair_cpu_batch(pair_count: usize) {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.merkle_pair_cpu_batches = guard.merkle_pair_cpu_batches.saturating_add(1);
        guard.merkle_pair_max_pairs = guard.merkle_pair_max_pairs.max(saturating_u32(pair_count));
    }
}

#[cfg(feature = "fastpq-gpu")]
fn record_poseidon_merkle_pair_fallback() {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.merkle_pair_fallbacks = guard.merkle_pair_fallbacks.saturating_add(1);
    }
}

#[cfg(feature = "fastpq-gpu")]
fn reset_poseidon_pipeline_stats() {
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        *guard = PoseidonPipelineStats::default();
    }
}

/// Row usage counts for each selector.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowUsage {
    /// Total number of real (non-padding) rows in the trace.
    pub total_rows: usize,
    /// Rows tagged with `OperationKind::Transfer`.
    pub transfer_rows: usize,
    /// Rows tagged with `OperationKind::Mint`.
    pub mint_rows: usize,
    /// Rows tagged with `OperationKind::Burn`.
    pub burn_rows: usize,
    /// Rows tagged with `OperationKind::RoleGrant`.
    pub role_grant_rows: usize,
    /// Rows tagged with `OperationKind::RoleRevoke`.
    pub role_revoke_rows: usize,
    /// Rows tagged with `OperationKind::MetaSet`.
    pub meta_set_rows: usize,
    /// Rows tagged with permission selectors (mirrors role grant/revoke rows).
    pub permission_rows: usize,
}

impl RowUsage {
    fn from_rows(rows: &[RowData], real_rows: usize) -> Self {
        let mut usage = Self {
            total_rows: real_rows,
            ..Self::default()
        };
        for row in rows.iter().take(real_rows) {
            usage.transfer_rows = usage
                .transfer_rows
                .saturating_add(selector_count(row.selectors.transfer));
            usage.mint_rows = usage
                .mint_rows
                .saturating_add(selector_count(row.selectors.mint));
            usage.burn_rows = usage
                .burn_rows
                .saturating_add(selector_count(row.selectors.burn));
            usage.role_grant_rows = usage
                .role_grant_rows
                .saturating_add(selector_count(row.selectors.role_grant));
            usage.role_revoke_rows = usage
                .role_revoke_rows
                .saturating_add(selector_count(row.selectors.role_revoke));
            usage.meta_set_rows = usage
                .meta_set_rows
                .saturating_add(selector_count(row.selectors.meta_set));
            usage.permission_rows = usage
                .permission_rows
                .saturating_add(selector_count(row.selectors.perm));
        }
        usage
    }

    /// Rows tagged with anything other than transfers.
    #[must_use]
    pub fn non_transfer_rows(&self) -> usize {
        self.total_rows.saturating_sub(self.transfer_rows)
    }
}

fn selector_count(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn populate_merkle_columns(
    row: &mut RowData,
    key: &[u8],
    balance_before: u64,
    balance_after: u64,
    proof: &transfer::TransferMerkleProof,
) {
    let mut current_before = transfer::leaf_hash(key, balance_before);
    let mut current_after = transfer::leaf_hash(key, balance_after);
    for level in 0..SMT_HEIGHT {
        let bit = proof.bit(level);
        row.path_bits[level] = bit;
        let sibling_hash = Hash::prehashed(proof.sibling(level));
        row.siblings[level] = hash_to_field(&sibling_hash);
        row.node_in[level] = hash_to_field(&current_before);
        row.node_out[level] = hash_to_field(&current_after);
        let (before_left, before_right) = if bit == 0 {
            (current_before, sibling_hash)
        } else {
            (sibling_hash, current_before)
        };
        current_before = transfer::internal_hash(&before_left, &before_right);
        let (after_left, after_right) = if bit == 0 {
            (current_after, sibling_hash)
        } else {
            (sibling_hash, current_after)
        };
        current_after = transfer::internal_hash(&after_left, &after_right);
    }
}

fn hash_to_field(hash: &Hash) -> u64 {
    let mut limb = [0u8; 8];
    limb.copy_from_slice(&hash.as_ref()[..8]);
    u64::from_le_bytes(limb) % GOLDILOCKS_MODULUS
}

/// Build the stage 1 trace columns for a transition batch.
///
/// # Errors
///
/// Returns [`Error`] when value widths or permission witnesses are malformed.
#[allow(clippy::too_many_lines)]
pub fn build_trace(batch: &TransitionBatch) -> Result<Trace> {
    let mut canonical = batch.clone();
    canonical.sort();

    let transfer_witnesses = extract_transfer_witnesses(
        &canonical.metadata,
        &canonical.transitions,
        &canonical.public_inputs,
    )?;
    let transfer_proof_index = transfer::index_row_proofs(&transfer_witnesses);

    let metadata_hash = metadata_hash(&canonical.metadata)?;
    let dsid_hash = hash_with_domain(DSID_DOMAIN, &canonical.public_inputs.dsid)?;
    let slot_value = canonical.public_inputs.slot;

    let mut rows: Vec<RowData> = Vec::with_capacity(canonical.transitions.len());
    let mut running_per_asset: HashMap<Vec<u8>, i128> = HashMap::new();
    let mut supply_counters: HashMap<Vec<u8>, i128> = HashMap::new();

    for transition in &canonical.transitions {
        let mut selectors = Selectors {
            active: 1,
            ..Selectors::default()
        };
        let (asset_id_bytes, perm_hash) = match &transition.operation {
            crate::OperationKind::Transfer => {
                selectors.transfer = 1;
                (extract_asset_id(&transition.key), 0)
            }
            crate::OperationKind::Mint => {
                selectors.mint = 1;
                (extract_asset_id(&transition.key), 0)
            }
            crate::OperationKind::Burn => {
                selectors.burn = 1;
                (extract_asset_id(&transition.key), 0)
            }
            crate::OperationKind::RoleGrant {
                role_id,
                permission_id,
                epoch,
            }
            | crate::OperationKind::RoleRevoke {
                role_id,
                permission_id,
                epoch,
            } => {
                let perm = permission_hash(role_id, permission_id, *epoch)?;
                selectors.perm = 1;
                if matches!(
                    &transition.operation,
                    crate::OperationKind::RoleGrant { .. }
                ) {
                    selectors.role_grant = 1;
                } else {
                    selectors.role_revoke = 1;
                }
                (extract_asset_id(&transition.key), perm)
            }
            crate::OperationKind::MetaSet => {
                selectors.meta_set = 1;
                (transition.key.clone(), 0)
            }
        };

        let key_limbs = pack_bytes(&transition.key).limbs;
        let value_old_limbs = pack_bytes(&transition.pre_value).limbs;
        let value_new_limbs = pack_bytes(&transition.post_value).limbs;
        let asset_limbs = pack_bytes(&asset_id_bytes).limbs;

        let numeric_values = matches!(
            transition.operation,
            crate::OperationKind::Transfer
                | crate::OperationKind::Mint
                | crate::OperationKind::Burn
        );
        let (_value_old, _value_new, delta_signed, pre_value_u64) = if numeric_values {
            let pre_value_u64 = decode_u64_le(&transition.pre_value)?;
            let post_value_u64 = decode_u64_le(&transition.post_value)?;
            let value_old = i128::from(pre_value_u64);
            let value_new = i128::from(post_value_u64);
            (value_old, value_new, value_new - value_old, pre_value_u64)
        } else {
            (0, 0, 0, 0)
        };
        let delta = field_from_i128(delta_signed);

        let asset_key = asset_id_bytes.clone();
        let running_prev = running_per_asset.get(&asset_key).copied().unwrap_or(0);
        let running_next = if numeric_values {
            let running_next = running_prev + delta_signed;
            running_per_asset.insert(asset_key.clone(), running_next);
            running_next
        } else {
            running_prev
        };

        let supply_prev = supply_counters.get(&asset_key).copied().unwrap_or(0);
        let supply_next = if numeric_values {
            let mut supply_next = supply_prev;
            if matches!(
                &transition.operation,
                crate::OperationKind::Mint | crate::OperationKind::Burn
            ) {
                supply_next += delta_signed;
            }
            supply_counters.insert(asset_key.clone(), supply_next);
            supply_next
        } else {
            supply_prev
        };

        let mut row = RowData {
            key_limbs,
            value_old_limbs,
            value_new_limbs,
            asset_limbs,
            delta,
            running_asset_delta: field_from_i128(running_next),
            supply_counter: field_from_i128(supply_next),
            metadata_hash,
            perm_hash,
            neighbour_leaf: 0,
            selectors,
            path_bits: [0; SMT_HEIGHT],
            siblings: [0; SMT_HEIGHT],
            node_in: [0; SMT_HEIGHT],
            node_out: [0; SMT_HEIGHT],
            dsid: dsid_hash,
            slot: slot_value,
        };

        if matches!(transition.operation, crate::OperationKind::Transfer) {
            let proof = transfer_proof_index
                .get(&TransferRowKey::from_transition(transition))
                .cloned()
                .ok_or_else(|| Error::TransferInvariant {
                    details: "transfer row is missing its canonical SMT proof witness".into(),
                });
            let proof = proof?;
            populate_merkle_columns(
                &mut row,
                transition.key.as_slice(),
                pre_value_u64,
                decode_u64_le(&transition.post_value)?,
                &proof,
            );
        }

        rows.push(row);
    }

    let n_rows = rows.len();
    let padded_len = pow2_ceil(max(1, n_rows));
    let row_usage = RowUsage::from_rows(&rows, n_rows);
    while rows.len() < padded_len {
        rows.push(RowData::padding(metadata_hash, dsid_hash, slot_value));
    }

    let max_key_limbs = rows
        .iter()
        .map(|row| row.key_limbs.len())
        .max()
        .unwrap_or_default();
    let max_value_old = rows
        .iter()
        .map(|row| row.value_old_limbs.len())
        .max()
        .unwrap_or_default();
    let max_value_new = rows
        .iter()
        .map(|row| row.value_new_limbs.len())
        .max()
        .unwrap_or_default();
    let max_asset_limbs = rows
        .iter()
        .map(|row| row.asset_limbs.len())
        .max()
        .unwrap_or_default();

    let mut columns = vec![
        TraceColumn::new("s_active", rows.iter().map(|row| row.selectors.active)),
        TraceColumn::new("s_transfer", rows.iter().map(|row| row.selectors.transfer)),
        TraceColumn::new("s_mint", rows.iter().map(|row| row.selectors.mint)),
        TraceColumn::new("s_burn", rows.iter().map(|row| row.selectors.burn)),
        TraceColumn::new(
            "s_role_grant",
            rows.iter().map(|row| row.selectors.role_grant),
        ),
        TraceColumn::new(
            "s_role_revoke",
            rows.iter().map(|row| row.selectors.role_revoke),
        ),
        TraceColumn::new("s_meta_set", rows.iter().map(|row| row.selectors.meta_set)),
        TraceColumn::new("s_perm", rows.iter().map(|row| row.selectors.perm)),
    ];

    for idx in 0..max_key_limbs {
        columns.push(TraceColumn::new(
            format!("key_limb_{idx}"),
            rows.iter()
                .map(|row| row.key_limbs.get(idx).copied().unwrap_or(0)),
        ));
    }
    for idx in 0..max_value_old {
        columns.push(TraceColumn::new(
            format!("value_old_limb_{idx}"),
            rows.iter()
                .map(|row| row.value_old_limbs.get(idx).copied().unwrap_or(0)),
        ));
    }
    for idx in 0..max_value_new {
        columns.push(TraceColumn::new(
            format!("value_new_limb_{idx}"),
            rows.iter()
                .map(|row| row.value_new_limbs.get(idx).copied().unwrap_or(0)),
        ));
    }
    for idx in 0..max_asset_limbs {
        columns.push(TraceColumn::new(
            format!("asset_id_limb_{idx}"),
            rows.iter()
                .map(|row| row.asset_limbs.get(idx).copied().unwrap_or(0)),
        ));
    }

    columns.push(TraceColumn::new("delta", rows.iter().map(|row| row.delta)));
    columns.push(TraceColumn::new(
        "running_asset_delta",
        rows.iter().map(|row| row.running_asset_delta),
    ));
    columns.push(TraceColumn::new(
        "metadata_hash",
        rows.iter().map(|row| row.metadata_hash),
    ));
    columns.push(TraceColumn::new(
        "supply_counter",
        rows.iter().map(|row| row.supply_counter),
    ));
    columns.push(TraceColumn::new(
        "perm_hash",
        rows.iter().map(|row| row.perm_hash),
    ));
    columns.push(TraceColumn::new(
        "neighbour_leaf",
        rows.iter().map(|row| row.neighbour_leaf),
    ));
    columns.push(TraceColumn::new("dsid", rows.iter().map(|row| row.dsid)));
    columns.push(TraceColumn::new("slot", rows.iter().map(|row| row.slot)));

    for level in 0..SMT_HEIGHT {
        columns.push(TraceColumn::new(
            format!("path_bit_{level}"),
            rows.iter().map(|row| row.path_bits[level]),
        ));
        columns.push(TraceColumn::new(
            format!("sibling_{level}"),
            rows.iter().map(|row| row.siblings[level]),
        ));
        columns.push(TraceColumn::new(
            format!("node_in_{level}"),
            rows.iter().map(|row| row.node_in[level]),
        ));
        columns.push(TraceColumn::new(
            format!("node_out_{level}"),
            rows.iter().map(|row| row.node_out[level]),
        ));
    }

    Ok(Trace {
        rows: n_rows,
        padded_len,
        columns,
        transfer_witnesses,
        row_usage,
    })
}

/// Return the canonical FASTPQ column layout for a transition batch without materialising rows.
///
#[must_use]
pub(crate) fn column_names_for_batch(batch: &TransitionBatch) -> Vec<String> {
    let mut canonical = batch.clone();
    canonical.sort();

    let mut max_key_limbs = 0usize;
    let mut max_value_old = 0usize;
    let mut max_value_new = 0usize;
    let mut max_asset_limbs = 0usize;
    for transition in &canonical.transitions {
        let asset_id_bytes = match &transition.operation {
            crate::OperationKind::MetaSet => transition.key.clone(),
            _ => extract_asset_id(&transition.key),
        };
        max_key_limbs = max_key_limbs.max(pack_bytes(&transition.key).limbs.len());
        max_value_old = max_value_old.max(pack_bytes(&transition.pre_value).limbs.len());
        max_value_new = max_value_new.max(pack_bytes(&transition.post_value).limbs.len());
        max_asset_limbs = max_asset_limbs.max(pack_bytes(&asset_id_bytes).limbs.len());
    }

    let mut columns = [
        "s_active",
        "s_transfer",
        "s_mint",
        "s_burn",
        "s_role_grant",
        "s_role_revoke",
        "s_meta_set",
        "s_perm",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    columns.extend((0..max_key_limbs).map(|idx| format!("key_limb_{idx}")));
    columns.extend((0..max_value_old).map(|idx| format!("value_old_limb_{idx}")));
    columns.extend((0..max_value_new).map(|idx| format!("value_new_limb_{idx}")));
    columns.extend((0..max_asset_limbs).map(|idx| format!("asset_id_limb_{idx}")));
    columns.extend(
        [
            "delta",
            "running_asset_delta",
            "metadata_hash",
            "supply_counter",
            "perm_hash",
            "neighbour_leaf",
            "dsid",
            "slot",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    for level in 0..SMT_HEIGHT {
        columns.push(format!("path_bit_{level}"));
        columns.push(format!("sibling_{level}"));
        columns.push(format!("node_in_{level}"));
        columns.push(format!("node_out_{level}"));
    }

    columns
}

impl TraceColumn {
    fn new(name: impl Into<String>, values: impl Iterator<Item = u64>) -> Self {
        Self {
            name: name.into(),
            values: values.collect(),
        }
    }
}

fn metadata_hash(metadata: &BTreeMap<String, Vec<u8>>) -> Result<u64> {
    if metadata.is_empty() {
        return Ok(0);
    }
    let encoded = norito::core::to_bytes(metadata)?;
    hash_with_domain(METADATA_DOMAIN, &encoded)
}

fn extract_transfer_witnesses(
    metadata: &BTreeMap<String, Vec<u8>>,
    transitions: &[StateTransition],
    public_inputs: &crate::PublicInputs,
) -> Result<Vec<transfer::TransferGadgetInput>> {
    let has_transfer = transitions
        .iter()
        .any(|transition| matches!(transition.operation, crate::OperationKind::Transfer));
    let Some(transcripts) = transfer::decode_transcripts(metadata)? else {
        if has_transfer {
            return Err(Error::MissingMetadata {
                key: TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            });
        }
        return Ok(Vec::new());
    };
    if has_transfer && transcripts.is_empty() {
        return Err(Error::TransferInvariant {
            details: "transfer transcript metadata is empty".into(),
        });
    }
    transfer::verify_transcripts(transitions, &transcripts)?;
    transfer::transcripts_to_witnesses(
        &transcripts,
        &public_inputs.old_root,
        &public_inputs.new_root,
    )
}

pub(crate) fn permission_hash(role_id: &[u8], permission_id: &[u8], epoch: u64) -> Result<u64> {
    if role_id.len() != 32 {
        return Err(Error::InvalidRoleIdLength {
            length: role_id.len(),
        });
    }
    if permission_id.len() != 32 {
        return Err(Error::InvalidPermissionIdLength {
            length: permission_id.len(),
        });
    }
    let mut payload = Vec::with_capacity(32 + 32 + 8);
    payload.extend_from_slice(role_id);
    payload.extend_from_slice(permission_id);
    payload.extend_from_slice(&epoch.to_le_bytes());
    Ok(poseidon::hash_field_elements_cpu(
        &pack_bytes(&payload).limbs,
    ))
}

fn hash_with_domain(domain: &[u8], payload: &[u8]) -> Result<u64> {
    let domain_packed = pack_bytes(domain);
    let payload_packed = pack_bytes(payload);

    let mut limbs = Vec::with_capacity(domain_packed.limbs.len() + payload_packed.limbs.len() + 2);
    let domain_len = u64::try_from(domain_packed.length).map_err(|_| Error::ValueWidth {
        length: domain_packed.length,
    })?;
    limbs.push(domain_len);
    limbs.extend(domain_packed.limbs);
    let payload_len = u64::try_from(payload_packed.length).map_err(|_| Error::ValueWidth {
        length: payload_packed.length,
    })?;
    limbs.push(payload_len);
    limbs.extend(payload_packed.limbs);

    Ok(poseidon::hash_field_elements_cpu(&limbs))
}

fn domain_seed(domain: &[u8]) -> u64 {
    let digest = Hash::new(domain);
    let bytes = digest.as_ref();
    let mut chunk = [0u8; 8];
    chunk.copy_from_slice(&bytes[..8]);
    let raw = u64::from_le_bytes(chunk);
    let reduced = u128::from(raw) % u128::from(GOLDILOCKS_MODULUS);
    u64::try_from(reduced).expect("modulus reduction fits u64")
}

fn hash_field_with_domain_cpu(domain: &[u8], values: &[u64]) -> u64 {
    let mut sponge = CpuPoseidonSponge::new();
    sponge.absorb(domain_seed(domain));
    sponge.absorb_slice(values);
    sponge.squeeze()
}

#[cfg(feature = "fastpq-gpu")]
/// Flattened Poseidon column payloads used by GPU hashing backends.
#[derive(Debug, Clone)]
pub struct PoseidonColumnBatch {
    payloads: Arc<[u64]>,
    payload_start: usize,
    payload_len: usize,
    offsets: Vec<PoseidonColumnSlice>,
    block_count: usize,
    padded_len: usize,
}

#[cfg(feature = "fastpq-gpu")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Offset metadata describing where a column resides inside the flattened payload buffer.
pub struct PoseidonColumnSlice {
    offset: u32,
    len: u32,
}

#[cfg(feature = "fastpq-gpu")]
impl PoseidonColumnSlice {
    fn new(offset: usize, len: usize) -> Option<Self> {
        let offset = u32::try_from(offset).ok()?;
        let len = u32::try_from(len).ok()?;
        Some(Self { offset, len })
    }

    /// Return the starting index of this column within the flattened payload slice.
    pub fn offset(self) -> usize {
        self.offset as usize
    }

    /// Return the number of limbs reserved for this column payload (including padding).
    pub fn len(self) -> usize {
        self.len as usize
    }

    /// Return true when this descriptor does not cover any payload limbs.
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    fn rebased(self, base: usize) -> Option<Self> {
        let offset = self.offset().checked_sub(base)?;
        Self::new(offset, self.len())
    }
}

#[cfg(feature = "fastpq-gpu")]
pub(crate) fn poseidon_limb_padded_len(limb_len: usize) -> Option<usize> {
    let payload = limb_len.checked_add(1)?;
    let remainder = payload % RATE;
    if remainder == 0 {
        Some(payload)
    } else {
        payload.checked_add(RATE - remainder)
    }
}

#[cfg(feature = "fastpq-gpu")]
impl PoseidonColumnBatch {
    fn empty() -> Self {
        Self {
            payloads: Arc::<[u64]>::from(Vec::<u64>::new()),
            payload_start: 0,
            payload_len: 0,
            offsets: Vec::new(),
            block_count: 0,
            padded_len: 0,
        }
    }

    /// Construct a flattened batch from the supplied domains and coefficient columns.
    pub fn from_domains_and_columns(domains: &[&str], columns: &[Vec<u64>]) -> Option<Self> {
        if domains.len() != columns.len() {
            tracing::warn!(
                target: "fastpq::poseidon",
                domain_count = domains.len(),
                column_count = columns.len(),
                "domain/column mismatch prevented gpu hashing"
            );
            return None;
        }
        if columns.is_empty() {
            return Some(Self::empty());
        }
        let column_len = columns[0].len();
        if !columns.iter().all(|column| column.len() == column_len) {
            tracing::warn!(
                target: "fastpq::poseidon",
                "column length mismatch prevented gpu hashing"
            );
            return None;
        }
        let padded_len = {
            let payload = column_len + 2;
            let remainder = payload % RATE;
            if remainder == 0 {
                payload
            } else {
                payload + (RATE - remainder)
            }
        };
        let mut payloads =
            Vec::with_capacity(columns.len().saturating_mul(padded_len).max(padded_len));
        let mut offsets = Vec::with_capacity(columns.len());
        let mut block_count = None;

        for (domain, values) in domains.iter().zip(columns.iter()) {
            let start = payloads.len();
            payloads.push(domain_seed(domain.as_bytes()));
            payloads.extend_from_slice(values);
            payloads.push(1);
            let mut column_total = payloads.len() - start;
            let remainder = column_total % RATE;
            if remainder != 0 {
                let padding = RATE - remainder;
                payloads.extend(std::iter::repeat_n(0, padding));
                column_total += padding;
            }
            let blocks = column_total / RATE;
            if let Some(expected) = block_count {
                debug_assert_eq!(
                    expected, blocks,
                    "poseidon columns must share the same block length"
                );
            } else {
                block_count = Some(blocks);
            }
            if let Some(slice) = PoseidonColumnSlice::new(start, column_total) {
                offsets.push(slice);
            } else {
                tracing::warn!(
                    target: "fastpq::poseidon",
                    offset = start,
                    len = column_total,
                    "poseidon column descriptor exceeded GPU bounds"
                );
                return None;
            }
        }

        Some(Self {
            payloads: payloads.into(),
            payload_start: 0,
            payload_len: offsets
                .last()
                .map_or(0, |slice| slice.offset() + slice.len()),
            offsets,
            block_count: block_count.unwrap_or(0),
            padded_len,
        })
    }

    /// Construct a flattened batch for domain-separated Merkle parent pairs.
    pub fn from_domain_and_pairs(domain: &[u8], pairs: &[[u64; 2]]) -> Option<Self> {
        if pairs.is_empty() {
            return Some(Self::empty());
        }
        let padded_len = {
            let payload = 4usize;
            let remainder = payload % RATE;
            if remainder == 0 {
                payload
            } else {
                payload + (RATE - remainder)
            }
        };
        let domain = domain_seed(domain);
        let mut payloads =
            Vec::with_capacity(pairs.len().saturating_mul(padded_len).max(padded_len));
        let mut offsets = Vec::with_capacity(pairs.len());
        let mut block_count = None;

        for pair in pairs {
            let start = payloads.len();
            payloads.push(domain);
            payloads.push(pair[0]);
            payloads.push(pair[1]);
            payloads.push(1);
            let mut column_total = payloads.len() - start;
            let remainder = column_total % RATE;
            if remainder != 0 {
                let padding = RATE - remainder;
                payloads.extend(std::iter::repeat_n(0, padding));
                column_total += padding;
            }
            let blocks = column_total / RATE;
            if let Some(expected) = block_count {
                debug_assert_eq!(
                    expected, blocks,
                    "poseidon pair batches must share the same block length"
                );
            } else {
                block_count = Some(blocks);
            }
            if let Some(slice) = PoseidonColumnSlice::new(start, column_total) {
                offsets.push(slice);
            } else {
                tracing::warn!(
                    target: "fastpq::poseidon",
                    offset = start,
                    len = column_total,
                    "poseidon pair descriptor exceeded GPU bounds"
                );
                return None;
            }
        }

        Some(Self {
            payloads: payloads.into(),
            payload_start: 0,
            payload_len: offsets
                .last()
                .map_or(0, |slice| slice.offset() + slice.len()),
            offsets,
            block_count: block_count.unwrap_or(0),
            padded_len,
        })
    }

    /// Construct a flattened batch from already domain-separated limb messages.
    pub fn from_limb_slices(messages: &[Vec<u64>]) -> Option<Self> {
        if messages.is_empty() {
            return Some(Self::empty());
        }
        let Some(padded_len) = messages
            .first()
            .and_then(|message| poseidon_limb_padded_len(message.len()))
        else {
            tracing::warn!(
                target: "fastpq::poseidon",
                "poseidon limb batch length exceeded host bounds"
            );
            return None;
        };
        if !messages
            .iter()
            .all(|message| poseidon_limb_padded_len(message.len()) == Some(padded_len))
        {
            tracing::warn!(
                target: "fastpq::poseidon",
                "mixed Poseidon limb padded lengths require separate gpu batches"
            );
            return None;
        }
        let mut payloads =
            Vec::with_capacity(messages.len().saturating_mul(padded_len).max(padded_len));
        let mut offsets = Vec::with_capacity(messages.len());
        let block_count = padded_len / RATE;

        for values in messages {
            let start = payloads.len();
            payloads.extend_from_slice(values);
            payloads.push(1);
            let padding = padded_len.saturating_sub(values.len() + 1);
            payloads.extend(std::iter::repeat_n(0, padding));
            if let Some(slice) = PoseidonColumnSlice::new(start, padded_len) {
                offsets.push(slice);
            } else {
                tracing::warn!(
                    target: "fastpq::poseidon",
                    offset = start,
                    len = padded_len,
                    "poseidon limb batch descriptor exceeded GPU bounds"
                );
                return None;
            }
        }

        Some(Self {
            payloads: payloads.into(),
            payload_start: 0,
            payload_len: offsets
                .last()
                .map_or(0, |slice| slice.offset() + slice.len()),
            offsets,
            block_count,
            padded_len,
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    pub(crate) fn columns(&self) -> usize {
        self.offsets.len()
    }

    pub(crate) fn block_count(&self) -> usize {
        self.block_count
    }

    pub(crate) fn payloads(&self) -> &[u64] {
        let start = self.payload_start.min(self.payloads.len());
        let end = (self.payload_start + self.payload_len).min(self.payloads.len());
        &self.payloads[start..end]
    }

    pub(crate) fn offsets(&self) -> &[PoseidonColumnSlice] {
        &self.offsets
    }

    pub(crate) fn padded_len(&self) -> usize {
        self.padded_len
    }

    pub(crate) fn rebased_slices(
        &self,
        offset: usize,
        count: usize,
    ) -> Option<Vec<PoseidonColumnSlice>> {
        if count == 0 {
            return Some(Vec::new());
        }
        let end = offset.checked_add(count)?;
        let base = self.offsets.get(offset)?.offset();
        let mut slices = Vec::with_capacity(count);
        for slice in self.offsets.get(offset..end)? {
            slices.push(slice.rebased(base)?);
        }
        Some(slices)
    }

    /// Materialise a batch containing a contiguous window of columns.
    ///
    /// The returned batch copies only the payload region that covers the
    /// requested columns and re-bases the column offsets so GPU kernels can
    /// ingest the flattened buffer directly.
    pub fn column_window(&self, offset: usize, count: usize) -> Option<Self> {
        if count == 0 {
            return Some(Self::empty());
        }
        let end = offset.checked_add(count)?;
        let base_slice = self.offsets.get(offset)?;
        let last_slice = self.offsets.get(end.checked_sub(1)?)?;
        let absolute_base = self.payload_start.checked_add(base_slice.offset())?;
        let window_end = self
            .payload_start
            .checked_add(last_slice.offset())?
            .checked_add(last_slice.len())?;
        if window_end > self.payload_start + self.payload_len || absolute_base >= window_end {
            return None;
        }

        let offsets = self.rebased_slices(offset, count)?;

        Some(Self {
            payloads: Arc::clone(&self.payloads),
            payload_start: absolute_base,
            payload_len: window_end - absolute_base,
            offsets,
            block_count: self.block_count,
            padded_len: self.padded_len,
        })
    }
}

#[cfg(feature = "fastpq-gpu")]
/// Attempt to hash the supplied columns using the active GPU backend.
///
/// Returns `None` when no accelerator is available or the GPU path encounters
/// an execution error, allowing callers to fall back to the CPU sponge.
pub fn hash_columns_gpu_batch(batch: &PoseidonColumnBatch) -> Option<Vec<u64>> {
    let backend = backend::current_gpu_backend()?;
    if POSEIDON_COLUMN_GPU_DISABLED.load(Ordering::Acquire) {
        record_poseidon_pipeline_fallback();
        return None;
    }
    if !poseidon_column_gpu_self_test(backend) {
        POSEIDON_COLUMN_GPU_DISABLED.store(true, Ordering::Release);
        record_poseidon_pipeline_fallback();
        return None;
    }
    if !batch.is_empty() {
        record_poseidon_pipeline_start(batch.columns(), 1);
    }
    match gpu::poseidon_hash_columns(batch, backend) {
        Ok(result) => {
            if !batch.is_empty() {
                record_poseidon_pipeline_batch();
            }
            Some(result)
        }
        Err(error) => {
            disable_poseidon_column_gpu_with_warning(
                backend,
                "dispatch error",
                batch.columns(),
                Some(&error),
            );
            record_poseidon_pipeline_fallback();
            None
        }
    }
}

#[cfg(feature = "fastpq-gpu")]
pub(crate) fn disable_poseidon_column_gpu_after_parity_mismatch(
    backend: backend::GpuBackend,
    operation: &'static str,
    item_count: usize,
) {
    disable_poseidon_column_gpu_with_warning(backend, operation, item_count, None);
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_column_disable_reason(
    operation: &'static str,
    error: Option<&gpu::GpuError>,
) -> &'static str {
    if error.is_some() {
        return match operation {
            "self-test error" => "self_test_error",
            _ => "dispatch_error",
        };
    }
    match operation {
        "self-test mismatch" => "self_test_mismatch",
        "limb batch count mismatch" => "count_mismatch",
        "limb batch CPU parity mismatch" | "runtime CPU parity mismatch" => "cpu_parity_mismatch",
        _ => operation,
    }
}

#[cfg(feature = "fastpq-gpu")]
fn disable_poseidon_column_gpu_with_warning(
    backend: backend::GpuBackend,
    operation: &'static str,
    item_count: usize,
    error: Option<&gpu::GpuError>,
) {
    if POSEIDON_COLUMN_GPU_DISABLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let reason = poseidon_column_disable_reason(operation, error);
    if error.is_none() {
        notify_poseidon_gpu_event_observer(
            "poseidon_columns",
            "sampled_parity_failure",
            reason,
            Some(backend),
        );
    }
    notify_poseidon_gpu_event_observer("poseidon_columns", "disabled", reason, Some(backend));
    if let Some(error) = error {
        tracing::warn!(
            target: "fastpq::poseidon",
            backend = ?backend,
            operation,
            item_count,
            %error,
            "gpu Poseidon column accelerator disabled; falling back to deterministic CPU hashing"
        );
    } else {
        tracing::warn!(
            target: "fastpq::poseidon",
            backend = ?backend,
            operation,
            item_count,
            "gpu Poseidon column accelerator disabled after CPU parity mismatch"
        );
    }
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_column_gpu_self_test(backend: backend::GpuBackend) -> bool {
    *POSEIDON_COLUMN_GPU_SELF_TEST.get_or_init(|| {
        let domains = [
            "fastpq:v1:trace:column:selftest:a",
            "fastpq:v1:trace:column:selftest:b",
        ];
        let columns = vec![vec![1u64, 2, 3, 4], vec![5u64, 6, 7, 8]];
        let Some(batch) = PoseidonColumnBatch::from_domains_and_columns(&domains, &columns) else {
            tracing::warn!(
                target: "fastpq::poseidon",
                "gpu poseidon column self-test could not build batch"
            );
            return false;
        };
        let expected =
            hash_columns_cpu_batch_inputs(&domains, &columns).expect("self-test batch is valid");
        match gpu::poseidon_hash_columns(&batch, backend) {
            Ok(actual) if actual == expected => true,
            Ok(actual) => {
                disable_poseidon_column_gpu_with_warning(
                    backend,
                    "self-test mismatch",
                    columns.iter().map(Vec::len).sum(),
                    None,
                );
                tracing::warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    expected = ?expected,
                    actual = ?actual,
                    "gpu poseidon column self-test mismatch; falling back to cpu hashing"
                );
                false
            }
            Err(error) => {
                disable_poseidon_column_gpu_with_warning(
                    backend,
                    "self-test error",
                    columns.iter().map(Vec::len).sum(),
                    Some(&error),
                );
                tracing::warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    %error,
                    "gpu poseidon column self-test failed; falling back to cpu hashing"
                );
                false
            }
        }
    })
}

#[cfg(feature = "fastpq-gpu")]
/// Hash the supplied domains and coefficient columns through the canonical CPU Poseidon path.
///
/// Returns `None` when the domain and column shapes do not match, mirroring the
/// validation performed by [`PoseidonColumnBatch::from_domains_and_columns`].
pub fn hash_columns_cpu_batch_inputs(domains: &[&str], columns: &[Vec<u64>]) -> Option<Vec<u64>> {
    if domains.len() != columns.len() {
        return None;
    }
    if columns.is_empty() {
        return Some(Vec::new());
    }
    let column_len = columns[0].len();
    if !columns.iter().all(|column| column.len() == column_len) {
        return None;
    }
    Some(
        domains
            .iter()
            .zip(columns.iter())
            .map(|(domain, values)| hash_field_with_domain_cpu(domain.as_bytes(), values))
            .collect(),
    )
}

#[cfg(feature = "fastpq-gpu")]
/// Hash the supplied Poseidon column batch on the GPU, returning leaf digests
/// alongside the fused depth-1 parent layer when acceleration succeeds.
///
/// The public contract is a fused digest result; internally the implementation
/// uses the parity-proven column batch kernel for leaves and the Merkle pair
/// batch helper for parents.
///
/// Returns `None` when GPU acceleration is unavailable, disabled via
/// [`ExecutionMode`], or a batch dispatch encounters an error so callers
/// can fall back to the scalar sponge.
pub fn hash_columns_gpu_fused(
    batch: &PoseidonColumnBatch,
    mode: ExecutionMode,
) -> Option<ColumnDigests> {
    let _backend = backend::current_gpu_backend()?;
    if !matches!(mode, ExecutionMode::Gpu | ExecutionMode::Auto) {
        return None;
    }
    if batch.is_empty() {
        return Some(ColumnDigests::new(Vec::new(), Some(Vec::new())));
    }
    if batch.block_count() == 0 || batch.padded_len() == 0 {
        let leaves = vec![0u64; batch.columns()];
        let parents = vec![0u64; batch.columns().div_ceil(2)];
        return Some(ColumnDigests::new(leaves, Some(parents)));
    }

    let leaves = hash_columns_gpu_batch(batch)?;
    let parent_pairs = merkle_pairs(&leaves);
    let parents = hash_trace_merkle_pairs_gpu(&parent_pairs).unwrap_or_else(|| {
        record_poseidon_merkle_pair_cpu_batch(parent_pairs.len());
        hash_trace_merkle_pairs_cpu(&parent_pairs)
    });
    Some(ColumnDigests::new(leaves, Some(parents)))
}

fn extract_asset_id(key: &[u8]) -> Vec<u8> {
    key.strip_prefix(b"asset/").map_or_else(
        || key.to_vec(),
        |rest| {
            rest.iter()
                .position(|&b| b == b'/')
                .map_or_else(|| rest.to_vec(), |end| rest[..end].to_vec())
        },
    )
}

fn decode_u64_le(bytes: &[u8]) -> Result<u64> {
    if bytes.len() > core::mem::size_of::<u64>() {
        return Err(Error::ValueWidth {
            length: bytes.len(),
        });
    }
    let mut chunk = [0u8; 8];
    chunk[..bytes.len()].copy_from_slice(bytes);
    Ok(u64::from_le_bytes(chunk))
}

fn field_from_i128(value: i128) -> u64 {
    let modulus = i128::from(GOLDILOCKS_MODULUS);
    let mut reduced = value % modulus;
    if reduced < 0 {
        reduced += modulus;
    }
    u64::try_from(reduced).expect("canonical reduction fits u64")
}

fn pow2_ceil(value: usize) -> usize {
    value.next_power_of_two()
}

/// Compute column hashes for a trace suitable for Poseidon Merkle commitment.
///
/// # Errors
///
/// Returns [`Error::ValueWidth`] when metadata payloads exceed the field limb
/// width or when Norito encoding fails.
#[allow(clippy::unnecessary_wraps)]
pub fn column_hashes(trace: &Trace, params: &StarkParameterSet) -> Result<ColumnDigests> {
    if trace.columns.is_empty() {
        return Ok(ColumnDigests::new(Vec::new(), None));
    }

    let planner = Planner::new(params);
    let mode = ExecutionMode::Cpu;
    let coefficients = trace_coefficients(trace, &planner, ExecutionMode::Cpu);

    Ok(hash_columns_from_coefficients(
        trace,
        &coefficients,
        &planner,
        mode,
        PoseidonPipelinePolicy::for_mode(mode),
    ))
}

pub(crate) fn trace_coefficients(
    trace: &Trace,
    planner: &Planner,
    mode: ExecutionMode,
) -> Vec<Vec<u64>> {
    let columns: Vec<Vec<u64>> = trace
        .columns
        .iter()
        .map(|column| column.values.clone())
        .collect();
    if columns.is_empty() {
        return columns;
    }

    match mode {
        ExecutionMode::Gpu => {
            #[cfg(test)]
            let mut cpu_columns = columns.clone();
            let mut gpu_columns = columns;
            planner.ifft_gpu(&mut gpu_columns);
            #[cfg(test)]
            {
                planner.ifft_columns(&mut cpu_columns);
                assert_eq!(
                    cpu_columns, gpu_columns,
                    "ifft gpu output diverged from cpu reference"
                );
            }
            gpu_columns
        }
        ExecutionMode::Cpu | ExecutionMode::Auto => {
            let mut cpu_columns = columns;
            planner.ifft_columns(&mut cpu_columns);
            cpu_columns
        }
    }
}

#[cfg_attr(not(feature = "fastpq-gpu"), allow(unused_variables))]
pub(crate) fn hash_columns_from_coefficients(
    trace: &Trace,
    coefficients: &[Vec<u64>],
    _planner: &Planner,
    _mode: ExecutionMode,
    poseidon_policy: PoseidonPipelinePolicy,
) -> ColumnDigests {
    assert_eq!(
        trace.columns.len(),
        coefficients.len(),
        "coefficient set must match trace columns"
    );

    #[cfg(feature = "fastpq-gpu")]
    let poseidon_backend = backend::current_gpu_backend();
    #[cfg(not(feature = "fastpq-gpu"))]
    let poseidon_backend: Option<backend::GpuBackend> = None;
    #[cfg(feature = "fastpq-gpu")]
    let mut poseidon_recorded = false;
    #[cfg(not(feature = "fastpq-gpu"))]
    let poseidon_recorded = false;

    #[cfg(feature = "fastpq-gpu")]
    {
        let domain_names: Vec<String> = trace
            .columns
            .iter()
            .map(|column| format!("{TRACE_COLUMN_DOMAIN_PREFIX}{}", column.name))
            .collect();
        let domain_refs: Vec<&str> = domain_names.iter().map(String::as_str).collect();
        if let Some(batch) =
            PoseidonColumnBatch::from_domains_and_columns(&domain_refs, coefficients)
        {
            if matches!(poseidon_policy.resolved(), ExecutionMode::Gpu) {
                if let Some(fused) = hash_columns_gpu_fused(&batch, ExecutionMode::Gpu) {
                    notify_poseidon_pipeline_observer(
                        poseidon_policy,
                        "gpu_fused",
                        poseidon_backend,
                    );
                    return fused;
                }
                if poseidon_backend.is_some()
                    && let Some(result) = hash_columns_gpu_batch(&batch)
                {
                    notify_poseidon_pipeline_observer(
                        poseidon_policy,
                        "gpu_batch",
                        poseidon_backend,
                    );
                    return ColumnDigests::new(result, None);
                }
                notify_poseidon_pipeline_observer(
                    poseidon_policy,
                    poseidon_policy.cpu_label(),
                    poseidon_backend,
                );
                poseidon_recorded = true;
            } else {
                notify_poseidon_pipeline_observer(
                    poseidon_policy,
                    poseidon_policy.cpu_label(),
                    poseidon_backend,
                );
                poseidon_recorded = true;
            }
        }
    }

    if !poseidon_recorded {
        notify_poseidon_pipeline_observer(
            poseidon_policy,
            poseidon_policy.cpu_label(),
            poseidon_backend,
        );
    }

    let leaves: Vec<u64> = trace
        .columns
        .par_iter()
        .zip(coefficients.par_iter())
        .map(|(column, coeffs)| {
            let domain = format!("{TRACE_COLUMN_DOMAIN_PREFIX}{}", column.name);
            hash_field_with_domain_cpu(domain.as_bytes(), coeffs)
        })
        .collect();

    ColumnDigests::new(leaves, None)
}

pub(crate) struct TracePolynomialData {
    pub coefficients: Vec<Vec<u64>>,
    lde_state: LdeColumnsState,
    transfer_plan: transfer::TransferGadgetPlan,
}

enum LdeColumnsState {
    Ready(Vec<Vec<u64>>),
}

impl TracePolynomialData {
    pub(crate) fn lde_columns(&mut self) -> &Vec<Vec<u64>> {
        match &self.lde_state {
            LdeColumnsState::Ready(columns) => columns,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn transfer_witnesses(&self) -> &[transfer::TransferGadgetInput] {
        self.transfer_plan.witnesses()
    }

    pub(crate) fn transfer_plan(&self) -> &transfer::TransferGadgetPlan {
        &self.transfer_plan
    }
}

pub(crate) fn derive_polynomial_data(
    trace: &Trace,
    planner: &Planner,
    _mode: ExecutionMode,
) -> TracePolynomialData {
    let coefficients = trace_coefficients(trace, planner, ExecutionMode::Cpu);
    let lde_state = if coefficients.is_empty() {
        LdeColumnsState::Ready(Vec::new())
    } else {
        // Proof LDE columns must be byte-identical across CPU and accelerator builds.
        // Metal LDE remains available through the planner APIs, but proof construction
        // materializes these columns on CPU and reserves GPU proof work for batched
        // Poseidon paths with parity gates.
        LdeColumnsState::Ready(planner.lde_columns(&coefficients))
    };
    TracePolynomialData {
        coefficients,
        lde_state,
        transfer_plan: transfer::TransferGadgetPlan::from_inputs(&trace.transfer_witnesses),
    }
}

pub(crate) fn column_index(trace: &Trace, name: &str) -> Option<usize> {
    trace.columns.iter().position(|column| column.name == name)
}

/// Compute a Poseidon2 Merkle root over column hashes using an optional fused first level.
pub fn merkle_root_with_first_level(leaves: &[u64], first_level: Option<&[u64]>) -> u64 {
    if leaves.is_empty() {
        return 0;
    }
    let mut current = first_level.map_or_else(
        || compute_merkle_level(leaves),
        |parents| {
            if parents.is_empty() && leaves.len() > 1 {
                compute_merkle_level(leaves)
            } else {
                parents.to_vec()
            }
        },
    );
    if current.is_empty() {
        return leaves[0];
    }
    while current.len() > 1 {
        current = compute_merkle_level(&current);
    }
    current[0]
}

/// Compute the traditional Merkle root using scalar-equivalent Poseidon hashes.
pub fn merkle_root(leaves: &[u64]) -> u64 {
    merkle_root_with_first_level(leaves, None)
}

fn compute_merkle_level(input: &[u64]) -> Vec<u64> {
    let pairs = merkle_pairs(input);
    hash_trace_merkle_pairs_batched(&pairs)
}

fn merkle_pairs(input: &[u64]) -> Vec<[u64; 2]> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut pairs = Vec::with_capacity(input.len().div_ceil(2));
    for chunk in input.chunks(2) {
        let left = chunk[0];
        let right = *chunk.get(1).unwrap_or(&left);
        pairs.push([left, right]);
    }
    pairs
}

fn hash_trace_merkle_pairs_cpu(pairs: &[[u64; 2]]) -> Vec<u64> {
    pairs
        .iter()
        .map(|pair| hash_field_with_domain_cpu(TRACE_NODE_DOMAIN, pair))
        .collect()
}

pub(crate) fn hash_trace_merkle_pairs_batched(pairs: &[[u64; 2]]) -> Vec<u64> {
    let mode = if backend::current_gpu_backend().is_some() {
        backend::ExecutionMode::Gpu
    } else {
        backend::ExecutionMode::Cpu
    };
    hash_trace_merkle_pairs_with_mode(pairs, mode)
}

pub(crate) fn hash_trace_merkle_pairs_with_mode(
    pairs: &[[u64; 2]],
    mode: backend::ExecutionMode,
) -> Vec<u64> {
    #[cfg(not(feature = "fastpq-gpu"))]
    let _ = mode;
    #[cfg(feature = "fastpq-gpu")]
    if mode == backend::ExecutionMode::Gpu
        && let Some(hashes) = hash_trace_merkle_pairs_gpu(pairs)
    {
        return hashes;
    }
    #[cfg(feature = "fastpq-gpu")]
    record_poseidon_merkle_pair_cpu_batch(pairs.len());
    hash_trace_merkle_pairs_cpu(pairs)
}

#[cfg(feature = "fastpq-gpu")]
fn hash_trace_merkle_pairs_gpu(pairs: &[[u64; 2]]) -> Option<Vec<u64>> {
    if pairs.is_empty() {
        return Some(Vec::new());
    }
    if pairs.len() < POSEIDON_MERKLE_GPU_MIN_PAIRS
        || POSEIDON_MERKLE_GPU_DISABLED.load(Ordering::Acquire)
    {
        return None;
    }
    let backend = backend::current_gpu_backend()?;
    if !poseidon_merkle_pair_gpu_preflight(backend) {
        disable_poseidon_merkle_gpu_with_warning(backend, pairs.len(), "preflight_failure", None);
        return None;
    }
    let Some(batch) = PoseidonColumnBatch::from_domain_and_pairs(TRACE_NODE_DOMAIN, pairs) else {
        record_poseidon_merkle_pair_fallback();
        return None;
    };
    match gpu::poseidon_hash_columns(&batch, backend) {
        Ok(result) => {
            if result.len() == pairs.len()
                && trace_merkle_pair_gpu_matches_cpu_sample(pairs, &result)
            {
                record_poseidon_merkle_pair_gpu_batch(pairs.len());
                Some(result)
            } else {
                disable_poseidon_merkle_gpu_with_warning(
                    backend,
                    pairs.len(),
                    "runtime CPU parity mismatch",
                    None,
                );
                tracing::warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    pair_count = pairs.len(),
                    actual = result.len(),
                    "gpu trace Merkle Poseidon pair batch diverged from CPU parity sample; falling back"
                );
                record_poseidon_merkle_pair_fallback();
                None
            }
        }
        Err(error) => {
            disable_poseidon_merkle_gpu_with_warning(
                backend,
                pairs.len(),
                "dispatch error",
                Some(&error),
            );
            record_poseidon_merkle_pair_fallback();
            None
        }
    }
}

#[cfg(feature = "fastpq-gpu")]
fn trace_merkle_pair_gpu_matches_cpu_sample(pairs: &[[u64; 2]], hashes: &[u64]) -> bool {
    if pairs.len() != hashes.len() {
        return false;
    }
    if pairs.is_empty() {
        return true;
    }

    #[cfg(any(test, debug_assertions))]
    let sample_indices = 0..pairs.len();
    #[cfg(not(any(test, debug_assertions)))]
    let sample_indices = trace_merkle_pair_sample_indices(pairs.len());

    for index in sample_indices {
        let expected = hash_field_with_domain_cpu(TRACE_NODE_DOMAIN, &pairs[index]);
        let actual = hashes[index];
        if actual != expected {
            tracing::warn!(
                target: "fastpq::poseidon",
                index,
                actual,
                expected,
                left = pairs[index][0],
                right = pairs[index][1],
                "gpu trace Merkle Poseidon pair parity mismatch"
            );
            return false;
        }
    }
    true
}

#[cfg(all(feature = "fastpq-gpu", not(any(test, debug_assertions))))]
fn trace_merkle_pair_sample_indices(len: usize) -> Vec<usize> {
    const SAMPLE_COUNT: usize = 16;
    if len <= SAMPLE_COUNT {
        return (0..len).collect();
    }

    let last = len - 1;
    let mut indices = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let index = sample * last / (SAMPLE_COUNT - 1);
        if indices.last().copied() != Some(index) {
            indices.push(index);
        }
    }
    indices
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_merkle_pair_gpu_preflight(backend: backend::GpuBackend) -> bool {
    *POSEIDON_MERKLE_GPU_SELF_TEST.get_or_init(|| {
        let pairs = [
            [0u64, 0u64],
            [1u64, 2u64],
            [GOLDILOCKS_MODULUS - 1, 42u64],
            [0xd1b5_4a32_d192_ed03, 0x9e37_79b9_7f4a_7c15],
        ];
        let Some(batch) = PoseidonColumnBatch::from_domain_and_pairs(TRACE_NODE_DOMAIN, &pairs)
        else {
            tracing::warn!(
                target: "fastpq::poseidon",
                backend = ?backend,
                "gpu trace Merkle Poseidon pair preflight could not build batch; falling back"
            );
            return false;
        };
        match gpu::poseidon_hash_columns(&batch, backend) {
            Ok(actual) => {
                let expected = hash_trace_merkle_pairs_cpu(&pairs);
                if actual == expected {
                    return true;
                }
                let mismatch = actual
                    .iter()
                    .zip(expected.iter())
                    .enumerate()
                    .find_map(|(index, (actual, expected))| {
                        (actual != expected).then_some((index, *actual, *expected))
                    })
                    .or_else(|| {
                        (actual.len() != expected.len()).then_some((
                            actual.len().min(expected.len()),
                            actual.get(expected.len()).copied().unwrap_or(0),
                            expected.get(actual.len()).copied().unwrap_or(0),
                        ))
                    });
                let (mismatch_index, actual_value, expected_value) =
                    mismatch.unwrap_or((0, 0, 0));
                tracing::warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    mismatch_index,
                    actual = actual_value,
                    expected = expected_value,
                    "gpu trace Merkle Poseidon pair preflight diverged; falling back to scalar hashing"
                );
                false
            }
            Err(error) => {
                tracing::warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    %error,
                    "gpu trace Merkle Poseidon pair preflight failed; falling back to scalar hashing"
                );
                false
            }
        }
    })
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_merkle_disable_reason(
    reason: &'static str,
    error: Option<&gpu::GpuError>,
) -> &'static str {
    if error.is_some() {
        return "dispatch_error";
    }
    match reason {
        "runtime CPU parity mismatch" => "cpu_parity_mismatch",
        "preflight_failure" => "preflight_failure",
        _ => reason,
    }
}

#[cfg(feature = "fastpq-gpu")]
fn disable_poseidon_merkle_gpu_with_warning(
    backend: backend::GpuBackend,
    pair_count: usize,
    reason: &'static str,
    error: Option<&gpu::GpuError>,
) {
    if POSEIDON_MERKLE_GPU_DISABLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
    {
        let reason_label = poseidon_merkle_disable_reason(reason, error);
        if error.is_none() && reason != "preflight_failure" {
            notify_poseidon_gpu_event_observer(
                "poseidon_merkle_pairs",
                "sampled_parity_failure",
                reason_label,
                Some(backend),
            );
        }
        notify_poseidon_gpu_event_observer(
            "poseidon_merkle_pairs",
            "disabled",
            reason_label,
            Some(backend),
        );
        if let Some(error) = error {
            tracing::warn!(
                target: "fastpq::poseidon",
                backend = ?backend,
                pair_count,
                min_pairs = POSEIDON_MERKLE_GPU_MIN_PAIRS,
                reason,
                %error,
                "gpu trace Merkle Poseidon pair accelerator disabled; falling back to scalar hashing"
            );
        } else {
            tracing::warn!(
                target: "fastpq::poseidon",
                backend = ?backend,
                pair_count,
                min_pairs = POSEIDON_MERKLE_GPU_MIN_PAIRS,
                reason,
                "gpu trace Merkle Poseidon pair accelerator disabled; falling back to scalar hashing"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use fastpq_isi::CANONICAL_PARAMETER_SETS;
    use iroha_crypto::Hash;
    use iroha_data_model::{
        DomainId,
        asset::id::AssetDefinitionId,
        fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::to_bytes;

    use super::*;
    use crate::{
        ExecutionMode, OperationKind, PoseidonExecutionMode, PublicInputs, StateTransition,
        TransitionBatch, gadgets::transfer,
    };
    #[cfg(feature = "fastpq-gpu")]
    use crate::{backend, gpu};

    fn sample_batch() -> TransitionBatch {
        let transcript = sample_transfer_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let mut batch = TransitionBatch::new(
            "fastpq-lane-balanced",
            PublicInputs {
                old_root,
                new_root,
                ..PublicInputs::default()
            },
        );
        for transition in sample_transitions(&transcript) {
            batch.push(transition);
        }
        let mint_key = format!(
            "asset/{}/{}",
            transcript.deltas[0].asset_definition, transcript.deltas[0].to_account
        );
        batch.push(StateTransition::new(
            mint_key.into_bytes(),
            20u64.to_le_bytes().to_vec(),
            40u64.to_le_bytes().to_vec(),
            OperationKind::Mint,
        ));
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&vec![transcript]).expect("encode transcripts"),
        );
        batch
    }

    #[test]
    fn poseidon_policy_defaults_to_fallback_mode() {
        let policy = PoseidonPipelinePolicy::new(PoseidonExecutionMode::Auto, ExecutionMode::Cpu);
        assert_eq!(policy.requested(), PoseidonExecutionMode::Auto);
        assert!(matches!(policy.resolved(), ExecutionMode::Cpu));
        assert_eq!(policy.cpu_label(), "cpu_fallback");

        let gpu_policy =
            PoseidonPipelinePolicy::new(PoseidonExecutionMode::Auto, ExecutionMode::Gpu);
        assert!(matches!(gpu_policy.resolved(), ExecutionMode::Gpu));
        assert_eq!(gpu_policy.cpu_label(), "cpu_fallback");
    }

    #[test]
    fn poseidon_policy_gpu_override_requires_gpu_backend() {
        let forced_gpu =
            PoseidonPipelinePolicy::new(PoseidonExecutionMode::Gpu, ExecutionMode::Gpu);
        assert!(matches!(forced_gpu.resolved(), ExecutionMode::Gpu));
        assert_eq!(forced_gpu.requested(), PoseidonExecutionMode::Gpu);

        let downgraded =
            PoseidonPipelinePolicy::new(PoseidonExecutionMode::Gpu, ExecutionMode::Cpu);
        assert!(matches!(downgraded.resolved(), ExecutionMode::Cpu));
        assert_eq!(downgraded.cpu_label(), "cpu_fallback");
    }

    #[test]
    fn poseidon_policy_respects_cpu_override() {
        let forced = PoseidonPipelinePolicy::new(PoseidonExecutionMode::Cpu, ExecutionMode::Gpu);
        assert!(matches!(forced.resolved(), ExecutionMode::Cpu));
        assert_eq!(forced.cpu_label(), "cpu_forced");
    }

    #[test]
    fn trace_has_power_of_two_length() {
        let trace = build_trace(&sample_batch()).expect("build");
        assert!(trace.padded_len.is_power_of_two());
        assert!(trace.padded_len >= trace.rows);
        assert!(
            trace
                .columns
                .iter()
                .all(|col| col.values.len() == trace.padded_len)
        );
    }

    #[test]
    fn column_names_for_batch_matches_trace_layout() {
        let batch = sample_batch();
        let trace = build_trace(&batch).expect("build trace");
        let expected = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();

        assert_eq!(column_names_for_batch(&batch), expected);
    }

    #[test]
    fn column_hashes_match_merkle_root() {
        let trace = build_trace(&sample_batch()).expect("build");
        let params = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-lane-balanced")
            .copied()
            .expect("canonical parameter set");
        let hashes = column_hashes(&trace, &params).expect("hashes");
        assert!(!hashes.leaves().is_empty());
        let root = merkle_root_with_first_level(hashes.leaves(), hashes.fused_parents());
        assert_ne!(root, 0);
    }

    #[test]
    fn column_hashes_reuse_coefficients() {
        let trace = build_trace(&sample_batch()).expect("build");
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let mut data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let via_coeffs = hash_columns_from_coefficients(
            &trace,
            &data.coefficients,
            &planner,
            ExecutionMode::Cpu,
            PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
        );
        let via_api = column_hashes(&trace, &params).expect("hash via api");
        assert_eq!(via_coeffs.leaves(), via_api.leaves());
        assert_eq!(
            merkle_root_with_first_level(via_coeffs.leaves(), via_coeffs.fused_parents()),
            merkle_root_with_first_level(via_api.leaves(), via_api.fused_parents())
        );
        assert_eq!(data.lde_columns().len(), trace.columns.len());
    }

    #[test]
    fn parallel_column_hashes_preserve_order() {
        let columns: Vec<TraceColumn> = (0_u64..8)
            .map(|index| TraceColumn {
                name: format!("col{index}"),
                values: vec![index, index + 1],
            })
            .collect();
        let coefficients: Vec<Vec<u64>> = columns.iter().map(|col| col.values.clone()).collect();
        let trace = Trace {
            rows: 2,
            padded_len: 2,
            columns: columns.clone(),
            transfer_witnesses: Vec::new(),
            row_usage: RowUsage::default(),
        };

        let sequential: Vec<u64> = columns
            .iter()
            .zip(coefficients.iter())
            .map(|(column, coeffs)| {
                let domain = format!("{TRACE_COLUMN_DOMAIN_PREFIX}{}", column.name);
                hash_field_with_domain_cpu(domain.as_bytes(), coeffs)
            })
            .collect();

        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let parallel = hash_columns_from_coefficients(
            &trace,
            &coefficients,
            &planner,
            ExecutionMode::Cpu,
            PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
        );
        assert_eq!(parallel.leaves(), sequential.as_slice());
        assert!(
            parallel.fused_parents().is_none(),
            "CPU hashing should not emit fused parents"
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_column_batch_flattens_inputs() {
        let domains = vec!["fastpq:v1:trace:column:a", "fastpq:v1:trace:column:b"];
        let columns = vec![vec![1u64, 2, 3], vec![4u64, 5, 6]];
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).expect("batch");
        assert_eq!(batch.columns(), domains.len());
        let offsets = batch.offsets();
        assert_eq!(offsets.len(), domains.len());
        assert!(offsets.iter().all(|slice| slice.len().is_multiple_of(RATE)));
        let payload_len = columns[0].len() + 2;
        let padded_len = if payload_len.is_multiple_of(RATE) {
            payload_len
        } else {
            payload_len + (RATE - (payload_len % RATE))
        };
        assert_eq!(batch.block_count(), padded_len / RATE);
        assert!(offsets.iter().all(|slice| slice.len() == padded_len));
        let payloads = batch.payloads();
        for (index, slice) in offsets.iter().enumerate() {
            let start = slice.offset();
            let end = slice.offset() + slice.len();
            let region = &payloads[start..end];
            assert_eq!(region[0], domain_seed(domains[index].as_bytes()));
            assert_eq!(&region[1..=columns[index].len()], columns[index].as_slice());
            assert_eq!(region[1 + columns[index].len()], 1);
            assert!(
                region[columns[index].len() + 2..]
                    .iter()
                    .all(|value| *value == 0)
            );
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_column_batch_rejects_mismatched_domain_and_column_counts() {
        let domains = vec!["fastpq:v1:trace:column:a"];
        let columns = vec![vec![1u64, 2, 3], vec![4u64, 5, 6]];
        assert!(
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).is_none(),
            "GPU batch construction must reject mismatched metadata before dispatch"
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_column_batch_flattens_merkle_pairs() {
        let pairs = vec![[1u64, 2u64], [3u64, 4u64], [5u64, 6u64]];
        let batch =
            PoseidonColumnBatch::from_domain_and_pairs(TRACE_NODE_DOMAIN, &pairs).expect("batch");
        assert_eq!(batch.columns(), pairs.len());
        let offsets = batch.offsets();
        assert_eq!(offsets.len(), pairs.len());
        assert!(offsets.iter().all(|slice| slice.len().is_multiple_of(RATE)));
        assert_eq!(batch.block_count(), offsets[0].len() / RATE);
        let payloads = batch.payloads();
        for (index, slice) in offsets.iter().enumerate() {
            let start = slice.offset();
            let end = slice.offset() + slice.len();
            let region = &payloads[start..end];
            assert_eq!(region[0], domain_seed(TRACE_NODE_DOMAIN));
            assert_eq!(&region[1..3], pairs[index].as_slice());
            assert_eq!(region[3], 1);
            assert!(region[4..].iter().all(|value| *value == 0));
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_column_batch_window_rejects_out_of_range_and_overflow() {
        let domains = vec![
            "fastpq:v1:trace:column:a",
            "fastpq:v1:trace:column:b",
            "fastpq:v1:trace:column:c",
        ];
        let columns = vec![vec![1u64, 2], vec![3u64, 4], vec![5u64, 6]];
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).expect("batch");
        assert!(batch.column_window(domains.len(), 1).is_none());
        assert!(batch.column_window(1, usize::MAX).is_none());
        assert!(batch.rebased_slices(1, usize::MAX).is_none());
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_column_batch_flattens_same_padded_limb_slices() {
        let messages = vec![Vec::<u64>::new(), vec![1u64]];
        let batch = PoseidonColumnBatch::from_limb_slices(&messages).expect("batch");
        assert_eq!(batch.columns(), messages.len());
        assert!(batch.padded_len().is_multiple_of(RATE));
        assert_eq!(batch.block_count(), batch.padded_len() / RATE);
        for (index, slice) in batch.offsets().iter().enumerate() {
            assert_eq!(slice.len(), batch.padded_len());
            let start = slice.offset();
            let end = slice.offset() + slice.len();
            let region = &batch.payloads()[start..end];
            assert_eq!(&region[..messages[index].len()], messages[index].as_slice());
            assert_eq!(region[messages[index].len()], 1);
            assert!(
                region[messages[index].len() + 1..]
                    .iter()
                    .all(|value| *value == 0)
            );
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_column_batch_rejects_mixed_padded_limb_slices() {
        let messages = vec![vec![1u64], vec![2u64, 3], vec![4u64, 5, 6, 7]];
        assert!(
            PoseidonColumnBatch::from_limb_slices(&messages).is_none(),
            "mixed canonical padded lengths must be grouped before GPU hashing"
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn fused_poseidon_respects_execution_mode() {
        let domains: Vec<&str> = Vec::new();
        let columns: Vec<Vec<u64>> = Vec::new();
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).expect("batch");
        assert!(hash_columns_gpu_fused(&batch, ExecutionMode::Cpu).is_none());
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn fused_poseidon_gpu_batch_executes() {
        let Some(backend) = backend::current_gpu_backend() else {
            return;
        };
        let domains = vec!["fastpq:v1:trace:column:a", "fastpq:v1:trace:column:b"];
        let columns = vec![vec![1u64, 2, 3, 4], vec![5u64, 6, 7, 8]];
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).expect("batch");
        match gpu::poseidon_hash_columns_fused(&batch, backend) {
            Ok(_) => {}
            Err(gpu::GpuError::Unsupported(_)) => {
                eprintln!("skipping fused poseidon gpu batch test: backend unavailable");
            }
            Err(error) => panic!("gpu fused failed: {error:?}"),
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn metal_poseidon_column_batch_matches_cpu_self_test_cases() {
        if !matches!(
            backend::current_gpu_backend(),
            Some(backend::GpuBackend::Metal)
        ) {
            eprintln!("skipping Metal Poseidon column parity test; backend unavailable");
            return;
        }
        let domains = [
            "fastpq:v1:trace:column:selftest:a",
            "fastpq:v1:trace:column:selftest:b",
        ];
        let columns = vec![vec![1u64, 2, 3, 4], vec![5u64, 6, 7, 8]];
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).expect("batch");
        let actual = gpu::poseidon_hash_columns(&batch, backend::GpuBackend::Metal)
            .expect("Metal Poseidon column batch should run");
        let expected = hash_columns_cpu_batch_inputs(&domains, &columns).expect("valid CPU batch");
        assert_eq!(actual, expected);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn metal_poseidon_merkle_pair_batch_matches_cpu_self_test_cases() {
        if !matches!(
            backend::current_gpu_backend(),
            Some(backend::GpuBackend::Metal)
        ) {
            eprintln!("skipping Metal Poseidon Merkle pair parity test; backend unavailable");
            return;
        }
        let pairs = [
            [0u64, 0u64],
            [1u64, 2u64],
            [GOLDILOCKS_MODULUS - 1, 42u64],
            [0xd1b5_4a32_d192_ed03, 0x9e37_79b9_7f4a_7c15],
        ];
        let batch = PoseidonColumnBatch::from_domain_and_pairs(TRACE_NODE_DOMAIN, &pairs)
            .expect("pair batch");
        let actual = gpu::poseidon_hash_columns(&batch, backend::GpuBackend::Metal)
            .expect("Metal Poseidon Merkle pair batch should run");
        let expected = hash_trace_merkle_pairs_cpu(&pairs);
        assert_eq!(actual, expected);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn public_gpu_poseidon_merkle_pair_batch_matches_cpu_self_test_cases() {
        let Some(backend) = backend::current_gpu_backend() else {
            eprintln!("skipping GPU Poseidon Merkle pair parity test; backend unavailable");
            return;
        };
        let pairs = [
            [0u64, 0u64],
            [1u64, 2u64],
            [GOLDILOCKS_MODULUS - 1, 42u64],
            [0xd1b5_4a32_d192_ed03, 0x9e37_79b9_7f4a_7c15],
        ];
        let batch = PoseidonColumnBatch::from_domain_and_pairs(TRACE_NODE_DOMAIN, &pairs)
            .expect("pair batch");
        let actual = gpu::poseidon_hash_columns(&batch, backend)
            .expect("GPU Poseidon pair batch should run");
        let expected = hash_trace_merkle_pairs_cpu(&pairs);
        assert_eq!(actual, expected);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_column_batch_windows_preserve_offsets() {
        let domains = vec![
            "fastpq:v1:trace:column:a",
            "fastpq:v1:trace:column:b",
            "fastpq:v1:trace:column:c",
        ];
        let columns = vec![vec![1u64, 2], vec![3u64, 4], vec![5u64, 6]];
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).expect("batch");
        let window = batch.column_window(1, 2).expect("slice");
        assert_eq!(window.columns(), 2);
        let offsets = window.offsets();
        assert_eq!(offsets.len(), 2);
        let payloads = window.payloads();
        for (index, slice) in offsets.iter().enumerate() {
            let start = slice.offset();
            let end = slice.offset() + slice.len();
            let region = &payloads[start..end];
            let domain = domains[index + 1].as_bytes();
            assert_eq!(region[0], domain_seed(domain));
            assert_eq!(
                &region[1..=columns[index + 1].len()],
                columns[index + 1].as_slice()
            );
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_gpu_hashes_match_cpu_when_backend_available() {
        if backend::current_gpu_backend().is_none() {
            eprintln!("skipping poseidon gpu parity test; backend unavailable");
            return;
        }
        let trace = build_trace(&sample_batch()).expect("build trace");
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let cpu_hashes = hash_columns_from_coefficients(
            &trace,
            &data.coefficients,
            &planner,
            ExecutionMode::Cpu,
            PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
        );
        let domain_names: Vec<String> = trace
            .columns
            .iter()
            .map(|column| format!("{TRACE_COLUMN_DOMAIN_PREFIX}{}", column.name))
            .collect();
        let domains: Vec<&str> = domain_names.iter().map(String::as_str).collect();
        let batch = PoseidonColumnBatch::from_domains_and_columns(&domains, &data.coefficients)
            .expect("gpu batch");
        let Some(gpu_hashes) = hash_columns_gpu_batch(&batch) else {
            eprintln!("skipping poseidon gpu parity test; dispatch declined");
            return;
        };
        assert_eq!(
            cpu_hashes.leaves(),
            gpu_hashes.as_slice(),
            "gpu hashes diverged from cpu"
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_cpu_batch_inputs_match_scalar_reference() {
        let domains = vec!["fastpq:v1:trace:column:a", "fastpq:v1:trace:column:b"];
        let columns = vec![vec![1u64, 2, 3, 4], vec![5u64, 6, 7, 8]];
        let hashes =
            hash_columns_cpu_batch_inputs(&domains, &columns).expect("cpu poseidon batch hashes");
        let expected: Vec<u64> = domains
            .iter()
            .zip(columns.iter())
            .map(|(domain, values)| hash_field_with_domain_cpu(domain.as_bytes(), values))
            .collect();
        assert_eq!(hashes, expected);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_gpu_repeated_dispatches_match_cpu_when_backend_available() {
        if backend::current_gpu_backend().is_none() {
            eprintln!("skipping repeated poseidon gpu parity test; backend unavailable");
            return;
        }
        let trace = build_trace(&sample_batch()).expect("build trace");
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let cpu_hashes = hash_columns_from_coefficients(
            &trace,
            &data.coefficients,
            &planner,
            ExecutionMode::Cpu,
            PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
        );
        let domain_names: Vec<String> = trace
            .columns
            .iter()
            .map(|column| format!("{TRACE_COLUMN_DOMAIN_PREFIX}{}", column.name))
            .collect();
        let domains: Vec<&str> = domain_names.iter().map(String::as_str).collect();
        let batch = PoseidonColumnBatch::from_domains_and_columns(&domains, &data.coefficients)
            .expect("gpu batch");
        let Some(first) = hash_columns_gpu_batch(&batch) else {
            eprintln!("skipping repeated poseidon gpu parity test; dispatch declined");
            return;
        };
        let second = hash_columns_gpu_batch(&batch).expect("second gpu hash dispatch");
        assert_eq!(
            first, second,
            "reused gpu workspace changed poseidon batch output between dispatches"
        );
        assert_eq!(
            cpu_hashes.leaves(),
            first.as_slice(),
            "reused gpu workspace diverged from cpu reference"
        );
    }

    #[test]
    fn merkle_root_with_first_level_matches_cpu_reference() {
        let leaves = vec![1u64, 2, 3, 4, 5];
        let full_root = merkle_root(&leaves);
        let first_level = compute_merkle_level(&leaves);
        let fused_root = merkle_root_with_first_level(&leaves, Some(&first_level));
        assert_eq!(
            fused_root, full_root,
            "providing the first level must not change the merkle root"
        );
    }

    #[test]
    fn merkle_levels_match_scalar_reference_for_mixed_shapes() {
        let shapes = [
            Vec::new(),
            vec![1u64],
            vec![1u64, 2],
            vec![1u64, 2, 3],
            (0_u64..17).collect::<Vec<_>>(),
            (0_u64..128)
                .map(|value| value.wrapping_mul(0x9e37_79b9_7f4a_7c15) % GOLDILOCKS_MODULUS)
                .collect::<Vec<_>>(),
            vec![GOLDILOCKS_MODULUS - 1, 0, 42, GOLDILOCKS_MODULUS - 2],
        ];
        for leaves in shapes {
            let pairs = merkle_pairs(&leaves);
            assert_eq!(
                compute_merkle_level(&leaves),
                hash_trace_merkle_pairs_cpu(&pairs),
                "merkle level diverged for {leaves:?}"
            );
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn trace_merkle_pair_parity_sample_rejects_truncated_or_tampered_gpu_output() {
        let pairs = [
            [0u64, 0u64],
            [1u64, 2u64],
            [GOLDILOCKS_MODULUS - 1, 42u64],
            [0xd1b5_4a32_d192_ed03, 0x9e37_79b9_7f4a_7c15],
        ];
        let expected = hash_trace_merkle_pairs_cpu(&pairs);
        assert!(
            !trace_merkle_pair_gpu_matches_cpu_sample(&pairs, &expected[..expected.len() - 1]),
            "truncated GPU output must fail CPU parity sampling"
        );

        let mut tampered = expected;
        tampered[2] = tampered[2].wrapping_add(1) % GOLDILOCKS_MODULUS;
        assert!(
            !trace_merkle_pair_gpu_matches_cpu_sample(&pairs, &tampered),
            "tampered GPU output must fail CPU parity sampling"
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_gpu_event_observer_records_disable_events() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let observed = Arc::new(AtomicUsize::new(0));
        let observed_for_callback = Arc::clone(&observed);
        set_poseidon_gpu_event_observer(move |accelerator, event, reason, backend| {
            assert_eq!(accelerator, "poseidon_merkle_pairs");
            assert_eq!(event, "disabled");
            assert_eq!(reason, "cpu_parity_mismatch");
            assert_eq!(backend, Some(backend::GpuBackend::Metal));
            observed_for_callback.fetch_add(1, Ordering::SeqCst);
        });
        notify_poseidon_gpu_event_observer(
            "poseidon_merkle_pairs",
            "disabled",
            "cpu_parity_mismatch",
            Some(backend::GpuBackend::Metal),
        );
        clear_poseidon_gpu_event_observer();
        assert_eq!(observed.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn merkle_pair_batch_stats_record_scalar_threshold_path() {
        enable_poseidon_pipeline_stats(true);
        let pair_count = POSEIDON_MERKLE_GPU_MIN_PAIRS - 1;
        let leaves = (0..pair_count * 2)
            .map(|value| (value as u64).wrapping_mul(0xd1b5_4a32_d192_ed03) % GOLDILOCKS_MODULUS)
            .collect::<Vec<_>>();
        let pairs = merkle_pairs(&leaves);
        let expected = hash_trace_merkle_pairs_cpu(&pairs);
        let actual = compute_merkle_level(&leaves);
        assert_eq!(actual, expected);
        let stats = take_poseidon_pipeline_stats().expect("stats enabled");
        enable_poseidon_pipeline_stats(false);
        assert!(
            stats.merkle_pair_cpu_batches > 0,
            "threshold Merkle level should be accounted for by pair-batch telemetry: {stats:?}"
        );
        assert_eq!(stats.merkle_pair_gpu_batches, 0);
        assert_eq!(
            stats.merkle_pair_max_pairs,
            u32::try_from(pair_count).expect("test pair count fits u32")
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn public_gpu_trace_merkle_pair_path_records_gpu_batch_when_backend_available() {
        if backend::current_gpu_backend().is_none() {
            eprintln!("skipping GPU Poseidon Merkle pair path test; backend unavailable");
            return;
        }
        enable_poseidon_pipeline_stats(true);
        let pair_count = POSEIDON_MERKLE_GPU_MIN_PAIRS;
        let pairs = (0..pair_count)
            .map(|value| {
                let left = (value as u64).wrapping_mul(0xd1b5_4a32_d192_ed03) % GOLDILOCKS_MODULUS;
                let right = (value as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) % GOLDILOCKS_MODULUS;
                [left, right]
            })
            .collect::<Vec<_>>();
        let expected = hash_trace_merkle_pairs_cpu(&pairs);
        let actual = hash_trace_merkle_pairs_with_mode(&pairs, backend::ExecutionMode::Gpu);
        assert_eq!(actual, expected);
        let stats = take_poseidon_pipeline_stats().expect("stats enabled");
        enable_poseidon_pipeline_stats(false);
        assert!(
            stats.merkle_pair_gpu_batches > 0,
            "GPU Merkle pair path should record a GPU batch: {stats:?}"
        );
        assert_eq!(
            stats.merkle_pair_fallbacks, 0,
            "GPU Merkle pair path should not fall back when parity passes"
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn poseidon_fused_gpu_matches_cpu_first_level() {
        if backend::current_gpu_backend().is_none() {
            eprintln!("skipping poseidon fused parity test; backend unavailable");
            return;
        }
        let trace = build_trace(&sample_batch()).expect("build trace");
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let coefficients = trace_coefficients(&trace, &planner, ExecutionMode::Cpu);
        let cpu_hashes = hash_columns_from_coefficients(
            &trace,
            &coefficients,
            &planner,
            ExecutionMode::Cpu,
            PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
        );
        let domain_names: Vec<String> = trace
            .columns
            .iter()
            .map(|column| format!("{TRACE_COLUMN_DOMAIN_PREFIX}{}", column.name))
            .collect();
        let domains: Vec<&str> = domain_names.iter().map(String::as_str).collect();
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &coefficients).expect("batch");
        let Some(fused) = hash_columns_gpu_fused(&batch, ExecutionMode::Gpu) else {
            eprintln!("skipping poseidon fused parity test; fused dispatch declined");
            return;
        };
        assert_eq!(
            fused.leaves(),
            cpu_hashes.leaves(),
            "fused gpu leaves diverged from cpu reference"
        );
        let expected_parents = compute_merkle_level(cpu_hashes.leaves());
        let fused_parents = fused
            .fused_parents()
            .expect("fused gpu path must return parents");
        assert_eq!(
            fused_parents,
            expected_parents.as_slice(),
            "fused gpu parents diverged from cpu reference"
        );
    }

    #[test]
    fn poseidon_policy_labels_cpu_fallbacks() {
        let policy = PoseidonPipelinePolicy::new(PoseidonExecutionMode::Cpu, ExecutionMode::Gpu);
        assert_eq!(policy.resolved(), ExecutionMode::Cpu);
        assert_eq!(policy.cpu_label(), "cpu_forced");
        let downgraded =
            PoseidonPipelinePolicy::new(PoseidonExecutionMode::Gpu, ExecutionMode::Cpu);
        assert_eq!(downgraded.resolved(), ExecutionMode::Cpu);
        assert_eq!(downgraded.cpu_label(), "cpu_fallback");
    }

    #[test]
    fn derive_polynomial_data_materializes_cpu_lde_for_gpu_mode() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace = build_trace(&sample_batch()).expect("build");
        let coefficients = trace_coefficients(&trace, &planner, ExecutionMode::Cpu);
        let expected = planner.lde_columns(&coefficients);

        let mut data = derive_polynomial_data(&trace, &planner, ExecutionMode::Gpu);

        assert_eq!(data.coefficients, coefficients);
        assert_eq!(data.lde_columns(), &expected);
    }

    #[test]
    fn transfer_witnesses_extracted_from_metadata() {
        let (batch, transcript) = batch_with_transfer_metadata();
        let trace = build_trace(&batch).expect("trace");
        let (old_root, new_root) = transcript_roots(&transcript);
        let expected = transfer::transcripts_to_witnesses(&[transcript], &old_root, &new_root)
            .expect("witness extraction");
        assert_eq!(trace.transfer_witnesses, expected);
        let proof_index = transfer::index_row_proofs(&expected);
        let mut canonical = batch.clone();
        canonical.sort();
        for (row, transition) in canonical.transitions.iter().enumerate() {
            if !matches!(transition.operation, OperationKind::Transfer) {
                continue;
            }
            let row_key = transfer::TransferRowKey::from_transition(transition);
            let proof = proof_index.get(&row_key).expect("transfer proof for row");
            let path_bit = trace
                .columns
                .iter()
                .find(|column| column.name == "path_bit_0")
                .expect("path_bit_0 column present")
                .values[row];
            assert_eq!(path_bit, proof.bit(0));
            let sibling_value = trace
                .columns
                .iter()
                .find(|column| column.name == "sibling_0")
                .expect("sibling_0 column present")
                .values[row];
            assert_eq!(
                sibling_value,
                hash_to_field(&Hash::prehashed(proof.sibling(0)))
            );
        }
    }

    #[test]
    fn build_trace_rejects_invalid_transfer_transcripts() {
        let (mut batch, mut transcript) = batch_with_transfer_metadata();
        transcript.deltas[0].from_balance_after = Numeric::from(1u32);
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&vec![transcript]).expect("encode transcripts"),
        );
        let err = build_trace(&batch).expect_err("invalid transcript must fail");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }

    #[test]
    fn build_trace_rejects_missing_transfer_transcripts() {
        let transcript = sample_transfer_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let mut batch = TransitionBatch::new(
            "fastpq-lane-balanced",
            PublicInputs {
                old_root,
                new_root,
                ..PublicInputs::default()
            },
        );
        for transition in sample_transitions(&transcript) {
            batch.push(transition);
        }
        let err = build_trace(&batch).expect_err("missing transcripts must fail");
        assert!(matches!(err, Error::MissingMetadata { .. }));
    }

    #[test]
    fn meta_set_accepts_non_numeric_values() {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        batch.push(StateTransition::new(
            b"metadata/domain/wonderland".to_vec(),
            br#"{"key":"old","value":1}"#.to_vec(),
            br#"{"key":"new","value":2}"#.to_vec(),
            OperationKind::MetaSet,
        ));
        let trace = build_trace(&batch).expect("build trace");
        assert_eq!(trace.rows, 1);
    }

    #[test]
    fn polynomial_data_exposes_transfer_witnesses() {
        let (batch, transcript) = batch_with_transfer_metadata();
        let trace = build_trace(&batch).expect("trace");
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let expected = transfer::transcripts_to_witnesses(
            &[transcript],
            &batch.public_inputs.old_root,
            &batch.public_inputs.new_root,
        )
        .expect("witness extraction");
        assert_eq!(data.transfer_plan().witnesses(), expected.as_slice());
    }

    #[test]
    fn row_usage_counts_per_selector() {
        let trace = build_trace(&sample_batch()).expect("build");
        assert_eq!(trace.row_usage.total_rows, trace.rows);
        assert_eq!(trace.row_usage.transfer_rows, 2);
        assert_eq!(trace.row_usage.mint_rows, 1);
        assert_eq!(
            trace.row_usage.non_transfer_rows(),
            trace.row_usage.total_rows - trace.row_usage.transfer_rows
        );
    }

    fn batch_with_transfer_metadata() -> (TransitionBatch, TransferTranscript) {
        let transcript = sample_transfer_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let mut batch = TransitionBatch::new(
            "fastpq-lane-balanced",
            PublicInputs {
                old_root,
                new_root,
                ..PublicInputs::default()
            },
        );
        for transition in sample_transitions(&transcript) {
            batch.push(transition);
        }
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&vec![transcript.clone()]).expect("encode transcripts"),
        );
        (batch, transcript)
    }

    fn sample_transfer_transcript() -> TransferTranscript {
        let mut delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            amount: Numeric::from(42u32),
            from_balance_before: Numeric::from(200u32),
            from_balance_after: Numeric::from(158u32),
            to_balance_before: Numeric::from(1u32),
            to_balance_after: Numeric::from(43u32),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        attach_delta_witnesses(&mut delta);
        let batch_hash = Hash::prehashed([0x22; 32]);
        let digest = transfer::compute_poseidon_digest(&delta, &batch_hash);
        TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::new(b"authority"),
            poseidon_preimage_digest: Some(digest),
        }
    }

    fn attach_delta_witnesses(delta: &mut TransferDeltaTranscript) {
        let scale = delta.normalized_scale();
        let sender_key =
            format!("asset/{}/{}", delta.asset_definition, delta.from_account).into_bytes();
        let receiver_key =
            format!("asset/{}/{}", delta.asset_definition, delta.to_account).into_bytes();
        let (from, to) = transfer::build_transfer_smt_witness_pair(
            &sender_key,
            numeric_to_u64(&delta.from_balance_before, scale),
            numeric_to_u64(&delta.from_balance_after, scale),
            &receiver_key,
            numeric_to_u64(&delta.to_balance_before, scale),
            numeric_to_u64(&delta.to_balance_after, scale),
        )
        .expect("transfer witness");
        delta.from_smt_witness = from;
        delta.to_smt_witness = to;
    }

    fn transcript_roots(transcript: &TransferTranscript) -> ([u8; 32], [u8; 32]) {
        let delta = transcript.deltas.first().expect("sample has delta");
        (
            delta.from_smt_witness.root_before,
            delta.to_smt_witness.root_after,
        )
    }

    fn numeric_to_u64(value: &Numeric, target_scale: u32) -> u64 {
        iroha_data_model::fastpq::normalized_numeric_to_u64(value, target_scale)
            .expect("numeric fits")
    }

    fn sample_transitions(transcript: &TransferTranscript) -> Vec<StateTransition> {
        transcript
            .deltas
            .iter()
            .flat_map(|delta| {
                let sender = StateTransition::new(
                    format!("asset/{}/{}", delta.asset_definition, delta.from_account).into_bytes(),
                    numeric_to_bytes(&delta.from_balance_before),
                    numeric_to_bytes(&delta.from_balance_after),
                    OperationKind::Transfer,
                );
                let receiver = StateTransition::new(
                    format!("asset/{}/{}", delta.asset_definition, delta.to_account).into_bytes(),
                    numeric_to_bytes(&delta.to_balance_before),
                    numeric_to_bytes(&delta.to_balance_after),
                    OperationKind::Transfer,
                );
                [sender, receiver]
            })
            .collect()
    }

    fn numeric_to_bytes(value: &Numeric) -> Vec<u8> {
        let amount: u64 = value.clone().try_into().expect("numeric fits u64");
        amount.to_le_bytes().to_vec()
    }
}
