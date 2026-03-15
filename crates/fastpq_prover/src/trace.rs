//! Stage 1 FASTPQ trace builder.
//!
//! The builder canonicalises transition batches into the row/column layout
//! expected by the FASTPQ AIR. Rows are ordered lexicographically by key,
//! operation rank, and original insertion index. Columns are padded to the
//! next power-of-two trace length and exposed as Goldilocks field elements.

use core::{cmp::max, convert::TryFrom};
#[cfg(feature = "fastpq-gpu")]
use std::sync::Mutex;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, OnceLock, RwLock},
};
#[cfg(feature = "fastpq-gpu")]
use std::{
    env,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use fastpq_isi::StarkParameterSet;
#[cfg(feature = "fastpq-gpu")]
use fastpq_isi::poseidon::RATE;
use iroha_crypto::Hash;
use iroha_data_model::fastpq::TRANSFER_TRANSCRIPTS_METADATA_KEY;
use rayon::prelude::*;
use tracing::warn;

#[cfg(feature = "fastpq-gpu")]
use crate::gpu;
#[cfg(feature = "fastpq-gpu")]
use crate::overrides;
use crate::{
    Error, Result, StateTransition, TransitionBatch,
    backend::{self, ExecutionMode, PoseidonExecutionMode},
    fft::{GpuLdeDispatch, Planner},
    gadgets::transfer::{self, ProofFlavor, TransferRowKey},
    pack_bytes,
    poseidon::{self, PoseidonSponge},
};

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
static DEBUG_FUSED_ENV: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "fastpq-gpu")]
const GPU_POSEIDON_PIPE_COLUMNS_MIN: usize = 16;
#[cfg(feature = "fastpq-gpu")]
const GPU_POSEIDON_PIPE_COLUMNS_MAX: usize = 256;
#[cfg(feature = "fastpq-gpu")]
const GPU_POSEIDON_PIPE_DEPTH_MIN: usize = 1;
#[cfg(feature = "fastpq-gpu")]
const GPU_POSEIDON_PIPE_DEPTH_MAX: usize = 8;
#[cfg(feature = "fastpq-gpu")]
const GPU_POSEIDON_PIPE_COLUMNS_ENV: &str = "FASTPQ_POSEIDON_PIPE_COLUMNS";
#[cfg(feature = "fastpq-gpu")]
const GPU_POSEIDON_PIPE_DEPTH_ENV: &str = "FASTPQ_POSEIDON_PIPE_DEPTH";
#[cfg(feature = "fastpq-gpu")]
static GPU_POSEIDON_PIPE_COLUMNS_OVERRIDE: OnceLock<Option<usize>> = OnceLock::new();
#[cfg(feature = "fastpq-gpu")]
static GPU_POSEIDON_PIPE_DEPTH_OVERRIDE: OnceLock<Option<usize>> = OnceLock::new();
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_PIPELINE_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_PIPELINE_STATS: OnceLock<Mutex<PoseidonPipelineStats>> = OnceLock::new();

type PoseidonPipelineObserver = dyn Fn(PoseidonPipelinePolicy, &'static str, Option<backend::GpuBackend>)
    + Send
    + Sync
    + 'static;
static POSEIDON_PIPELINE_OBSERVER: OnceLock<RwLock<Option<Arc<PoseidonPipelineObserver>>>> =
    OnceLock::new();

#[cfg(feature = "fastpq-gpu")]
fn fused_debug_enabled() -> bool {
    if let Some(enabled) = overrides::metal_debug_fused_override() {
        return enabled;
    }
    overrides::guard_env_override(|| {
        Some(*DEBUG_FUSED_ENV.get_or_init(|| env::var_os("FASTPQ_DEBUG_FUSED").is_some()))
    })
    .unwrap_or(false)
}

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

/// Remove the previously registered Poseidon pipeline observer, if any.
pub fn clear_poseidon_pipeline_observer() {
    if let Ok(mut guard) = poseidon_observer_slot().write() {
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
fn record_poseidon_pipeline_start(chunk_columns: usize, depth: usize) {
    if !POSEIDON_PIPELINE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store =
        POSEIDON_PIPELINE_STATS.get_or_init(|| Mutex::new(PoseidonPipelineStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.enabled = true;
        guard.chunk_columns = chunk_columns.min(u32::MAX as usize) as u32;
        guard.pipe_depth = depth.min(u32::MAX as usize) as u32;
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
    flavor: ProofFlavor,
    proof: &transfer::TransferMerkleProof,
) {
    let mut current = transfer::synthetic_leaf_hash(key, balance_before, flavor);
    for level in 0..SMT_HEIGHT {
        let bit = proof.bit(level);
        row.path_bits[level] = bit;
        let sibling_hash = proof.sibling(level);
        row.siblings[level] = hash_to_field(&sibling_hash);
        row.node_in[level] = hash_to_field(&current);
        let (left, right) = if bit == 0 {
            (sibling_hash, current)
        } else {
            (current, sibling_hash)
        };
        current = transfer::synthetic_internal_hash(&left, &right);
        row.node_out[level] = hash_to_field(&current);
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

    let transfer_witnesses =
        extract_transfer_witnesses(&canonical.metadata, &canonical.transitions)?;
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
            let proof_role = if delta_signed.is_negative() {
                ProofFlavor::Sender
            } else {
                ProofFlavor::Receiver
            };
            let proof = transfer_proof_index
                .get(&TransferRowKey::from_transition(transition))
                .cloned()
                .unwrap_or_else(|| {
                    transfer::synthesize_row_proof(
                        transition.key.as_slice(),
                        pre_value_u64,
                        proof_role,
                    )
                });
            populate_merkle_columns(
                &mut row,
                transition.key.as_slice(),
                pre_value_u64,
                proof_role,
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
        .unwrap_or(0);
    let max_value_old = rows
        .iter()
        .map(|row| row.value_old_limbs.len())
        .max()
        .unwrap_or(0);
    let max_value_new = rows
        .iter()
        .map(|row| row.value_new_limbs.len())
        .max()
        .unwrap_or(0);
    let max_asset_limbs = rows
        .iter()
        .map(|row| row.asset_limbs.len())
        .max()
        .unwrap_or(0);

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
    transfer::transcripts_to_witnesses(&transcripts)
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
    Ok(poseidon::hash_field_elements(&pack_bytes(&payload).limbs))
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

    Ok(poseidon::hash_field_elements(&limbs))
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

fn hash_field_with_domain(domain: &[u8], values: &[u64]) -> u64 {
    let mut sponge = PoseidonSponge::new();
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
    pub fn offset(&self) -> usize {
        self.offset as usize
    }

    /// Return the number of limbs reserved for this column payload (including padding).
    pub fn len(&self) -> usize {
        self.len as usize
    }

    fn rebased(&self, base: usize) -> Option<Self> {
        let offset = self.offset().checked_sub(base)?;
        Self::new(offset, self.len())
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
            warn!(
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
            warn!(
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
                payloads.extend(std::iter::repeat(0).take(padding));
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
            match PoseidonColumnSlice::new(start, column_total) {
                Some(slice) => offsets.push(slice),
                None => {
                    warn!(
                        target: "fastpq::poseidon",
                        offset = start,
                        len = column_total,
                        "poseidon column descriptor exceeded GPU bounds"
                    );
                    return None;
                }
            }
        }

        Some(Self {
            payloads: payloads.into(),
            payload_start: 0,
            payload_len: offsets
                .last()
                .map(|slice| slice.offset() + slice.len())
                .unwrap_or(0),
            offsets,
            block_count: block_count.unwrap_or(0),
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
    match gpu::poseidon_hash_columns(batch, backend) {
        Ok(result) => Some(result),
        Err(error) => {
            warn!(
                target: "fastpq::poseidon",
                backend = ?backend,
                %error,
                "gpu poseidon batch failed; falling back to cpu hashing"
            );
            None
        }
    }
}

#[cfg(feature = "fastpq-gpu")]
fn hash_columns_gpu_pipelined_batch(batch: &PoseidonColumnBatch) -> Option<Vec<u64>> {
    let backend = backend::current_gpu_backend()?;
    if batch.is_empty() {
        return Some(Vec::new());
    }
    let chunk_size = poseidon_pipe_columns();
    let pipe_depth = poseidon_pipe_depth();
    record_poseidon_pipeline_start(chunk_size, pipe_depth);
    let total_columns = batch.columns();
    let (task_tx, task_rx) = mpsc::sync_channel::<(usize, PoseidonColumnBatch)>(pipe_depth);
    let (result_tx, result_rx) =
        mpsc::channel::<std::result::Result<(usize, Vec<u64>), gpu::GpuError>>();

    let worker = thread::spawn(move || {
        while let Ok((offset, chunk)) = task_rx.recv() {
            let result = gpu::poseidon_hash_columns(&chunk, backend).map(|hashes| (offset, hashes));
            if result_tx.send(result).is_err() {
                break;
            }
        }
    });

    let mut dispatched = 0usize;
    for chunk_start in (0..total_columns).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(total_columns);
        let column_count = chunk_end - chunk_start;
        let Some(chunk) = batch.column_window(chunk_start, column_count) else {
            warn!(
                target: "fastpq::poseidon",
                chunk_start,
                chunk_end,
                "poseidon pipeline failed to materialise chunk; falling back to cpu hashing"
            );
            drop(task_tx);
            record_poseidon_pipeline_fallback();
            let _ = worker.join();
            return None;
        };
        if task_tx.send((chunk_start, chunk)).is_err() {
            warn!(
                target: "fastpq::poseidon",
                chunk_start,
                chunk_end,
                "poseidon pipeline worker unavailable; falling back to cpu hashing"
            );
            record_poseidon_pipeline_fallback();
            let _ = worker.join();
            return None;
        }
        record_poseidon_pipeline_batch();
        dispatched += 1;
    }
    drop(task_tx);

    let mut hashes = vec![0u64; total_columns];
    let mut errored = false;
    for _ in 0..dispatched {
        match result_rx.recv() {
            Ok(Ok((offset, chunk_hashes))) => {
                let end = offset + chunk_hashes.len();
                hashes[offset..end].copy_from_slice(&chunk_hashes);
            }
            Ok(Err(error)) => {
                warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    %error,
                    "poseidon pipeline chunk failed; falling back to cpu hashing"
                );
                errored = true;
                record_poseidon_pipeline_fallback();
                break;
            }
            Err(_) => {
                warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    "poseidon pipeline dropped results; falling back to cpu hashing"
                );
                errored = true;
                record_poseidon_pipeline_fallback();
                break;
            }
        }
    }
    let _ = worker.join();
    if errored { None } else { Some(hashes) }
}

#[cfg(feature = "fastpq-gpu")]
/// Hash the supplied Poseidon column batch on the GPU, returning leaf digests
/// alongside the fused depth-1 parent layer when acceleration succeeds.
///
/// Returns `None` when GPU acceleration is unavailable, disabled via
/// [`ExecutionMode`], or the streaming pipeline encounters an error so callers
/// can fall back to the scalar sponge.
pub fn hash_columns_gpu_fused(
    batch: &PoseidonColumnBatch,
    mode: ExecutionMode,
) -> Option<ColumnDigests> {
    let backend = backend::current_gpu_backend()?;
    if !matches!(mode, ExecutionMode::Gpu | ExecutionMode::Auto) {
        return None;
    }
    if batch.is_empty() {
        return Some(ColumnDigests::new(Vec::new(), Some(Vec::new())));
    }
    if batch.block_count() == 0 || batch.padded_len() == 0 {
        let leaves = vec![0u64; batch.columns()];
        let parents = vec![0u64; (batch.columns() + 1) / 2];
        return Some(ColumnDigests::new(leaves, Some(parents)));
    }

    let total_columns = batch.columns();
    let total_parents = (total_columns + 1) / 2;
    let mut chunk_columns = poseidon_pipe_columns().max(2);
    if chunk_columns % 2 != 0 {
        chunk_columns = chunk_columns.saturating_sub(1);
    }
    if chunk_columns < 2 {
        chunk_columns = 2;
    }
    let pipe_depth = poseidon_pipe_depth();
    record_poseidon_pipeline_start(chunk_columns, pipe_depth);
    let (task_tx, task_rx) =
        mpsc::sync_channel::<(usize, usize, usize, usize, PoseidonColumnBatch)>(pipe_depth);
    let (result_tx, result_rx) = mpsc::channel::<
        std::result::Result<(usize, usize, usize, usize, Vec<u64>), gpu::GpuError>,
    >();

    let worker_backend = backend;
    let worker = thread::spawn(move || {
        while let Ok((offset, parent_offset, column_count, parent_count, chunk)) = task_rx.recv() {
            let result = gpu::poseidon_hash_columns_fused(&chunk, worker_backend)
                .map(|hashes| (offset, parent_offset, column_count, parent_count, hashes));
            if result_tx.send(result).is_err() {
                break;
            }
        }
    });

    let mut dispatched = 0usize;
    let mut parent_cursor = 0usize;
    for chunk_start in (0..total_columns).step_by(chunk_columns) {
        let chunk_end = (chunk_start + chunk_columns).min(total_columns);
        let column_count = chunk_end - chunk_start;
        let chunk_parents = (column_count + 1) / 2;
        let Some(chunk) = batch.column_window(chunk_start, column_count) else {
            warn!(
                target: "fastpq::poseidon",
                chunk_start,
                chunk_end,
                "poseidon fused pipeline failed to materialise chunk; falling back"
            );
            drop(task_tx);
            record_poseidon_pipeline_fallback();
            let _ = worker.join();
            return None;
        };
        if task_tx
            .send((
                chunk_start,
                parent_cursor,
                column_count,
                chunk_parents,
                chunk,
            ))
            .is_err()
        {
            warn!(
                target: "fastpq::poseidon",
                chunk_start,
                chunk_end,
                "poseidon fused pipeline worker unavailable; falling back"
            );
            drop(task_tx);
            record_poseidon_pipeline_fallback();
            let _ = worker.join();
            return None;
        }
        record_poseidon_pipeline_batch();
        dispatched += 1;
        parent_cursor += chunk_parents;
    }
    drop(task_tx);

    let mut leaves = vec![0u64; total_columns];
    let mut parents = vec![0u64; total_parents];
    let mut errored = false;
    for _ in 0..dispatched {
        match result_rx.recv() {
            Ok(Ok((offset, parent_offset, column_count, parent_count, fused))) => {
                if fused.len() != column_count + parent_count {
                    warn!(
                        target: "fastpq::poseidon",
                        chunk_start = offset,
                        chunk_end = offset + column_count,
                        expected = column_count + parent_count,
                        actual = fused.len(),
                        "poseidon fused kernel returned unexpected digest count"
                    );
                    errored = true;
                    record_poseidon_pipeline_fallback();
                    break;
                }
                leaves[offset..offset + column_count].copy_from_slice(&fused[..column_count]);
                parents[parent_offset..parent_offset + parent_count]
                    .copy_from_slice(&fused[column_count..]);
            }
            Ok(Err(error)) => {
                warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    %error,
                    "poseidon fused pipeline chunk failed; falling back"
                );
                if fused_debug_enabled() {
                    eprintln!("poseidon fused chunk failed: {error:?}");
                }
                errored = true;
                record_poseidon_pipeline_fallback();
                break;
            }
            Err(_) => {
                warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    "poseidon fused pipeline dropped results; falling back"
                );
                errored = true;
                record_poseidon_pipeline_fallback();
                break;
            }
        }
    }
    let _ = worker.join();
    if errored {
        None
    } else {
        Some(ColumnDigests::new(leaves, Some(parents)))
    }
}

#[cfg(feature = "fastpq-gpu")]
fn hash_columns_gpu_overlap(trace: &Trace, planner: &Planner) -> Option<ColumnDigests> {
    let backend = backend::current_gpu_backend()?;
    if trace.columns.is_empty() {
        return Some(ColumnDigests::new(Vec::new(), None));
    }
    let chunk_size = poseidon_pipe_columns();
    let pipe_depth = poseidon_pipe_depth();
    record_poseidon_pipeline_start(chunk_size, pipe_depth);
    let (task_tx, task_rx) = mpsc::sync_channel::<(usize, PoseidonColumnBatch)>(pipe_depth);
    let (result_tx, result_rx) =
        mpsc::channel::<std::result::Result<(usize, Vec<u64>), gpu::GpuError>>();

    let worker = thread::spawn(move || {
        while let Ok((offset, batch)) = task_rx.recv() {
            let result = gpu::poseidon_hash_columns(&batch, backend).map(|hashes| (offset, hashes));
            if result_tx.send(result).is_err() {
                break;
            }
        }
    });

    let mut dispatched = 0usize;
    for chunk_start in (0..trace.columns.len()).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(trace.columns.len());
        let mut coeff_chunk: Vec<Vec<u64>> = trace.columns[chunk_start..chunk_end]
            .iter()
            .map(|column| column.values.clone())
            .collect();
        planner.ifft_columns(&mut coeff_chunk);
        let domain_refs: Vec<&str> = trace.columns[chunk_start..chunk_end]
            .iter()
            .map(|column| column.name.as_str())
            .collect();
        let Some(batch) = PoseidonColumnBatch::from_domains_and_columns(&domain_refs, &coeff_chunk)
        else {
            warn!(
                target: "fastpq::poseidon",
                chunk_start,
                chunk_end,
                "poseidon overlap pipeline failed to encode chunk; falling back to cpu hashing"
            );
            drop(task_tx);
            record_poseidon_pipeline_fallback();
            let _ = worker.join();
            return None;
        };
        if task_tx.send((chunk_start, batch)).is_err() {
            warn!(
                target: "fastpq::poseidon",
                chunk_start,
                chunk_end,
                "poseidon overlap worker unavailable; falling back to cpu hashing"
            );
            record_poseidon_pipeline_fallback();
            let _ = worker.join();
            return None;
        }
        record_poseidon_pipeline_batch();
        dispatched += 1;
    }
    drop(task_tx);

    let mut leaves = vec![0u64; trace.columns.len()];
    let mut errored = false;
    for _ in 0..dispatched {
        match result_rx.recv() {
            Ok(Ok((offset, chunk_hashes))) => {
                let end = offset + chunk_hashes.len();
                leaves[offset..end].copy_from_slice(&chunk_hashes);
            }
            Ok(Err(error)) => {
                warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    %error,
                    "poseidon overlap pipeline chunk failed; falling back to cpu hashing"
                );
                errored = true;
                record_poseidon_pipeline_fallback();
                break;
            }
            Err(_) => {
                warn!(
                    target: "fastpq::poseidon",
                    backend = ?backend,
                    "poseidon overlap pipeline dropped results; falling back to cpu hashing"
                );
                errored = true;
                record_poseidon_pipeline_fallback();
                break;
            }
        }
    }
    let _ = worker.join();
    if errored {
        None
    } else {
        Some(ColumnDigests::new(leaves, None))
    }
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_pipe_columns() -> usize {
    let default = match backend::current_gpu_backend() {
        // Smaller default batches to shorten Poseidon dispatches on large traces.
        Some(backend::GpuBackend::Metal) => 32,
        Some(backend::GpuBackend::Cuda) => 32,
        _ => 32,
    };
    poseidon_pipe_columns_override()
        .unwrap_or(default)
        .clamp(GPU_POSEIDON_PIPE_COLUMNS_MIN, GPU_POSEIDON_PIPE_COLUMNS_MAX)
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_pipe_columns_override() -> Option<usize> {
    *GPU_POSEIDON_PIPE_COLUMNS_OVERRIDE.get_or_init(|| {
        env::var(GPU_POSEIDON_PIPE_COLUMNS_ENV)
            .ok()
            .and_then(|raw| {
                parse_pipe_override(
                    &raw,
                    GPU_POSEIDON_PIPE_COLUMNS_ENV,
                    GPU_POSEIDON_PIPE_COLUMNS_MIN,
                    GPU_POSEIDON_PIPE_COLUMNS_MAX,
                )
            })
    })
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_pipe_depth() -> usize {
    // Shallower default depth reduces per-dispatch runtime on large traces where Poseidon dominates.
    poseidon_pipe_depth_override()
        .unwrap_or(1)
        .clamp(GPU_POSEIDON_PIPE_DEPTH_MIN, GPU_POSEIDON_PIPE_DEPTH_MAX)
}

#[cfg(feature = "fastpq-gpu")]
fn poseidon_pipe_depth_override() -> Option<usize> {
    *GPU_POSEIDON_PIPE_DEPTH_OVERRIDE.get_or_init(|| {
        env::var(GPU_POSEIDON_PIPE_DEPTH_ENV).ok().and_then(|raw| {
            parse_pipe_override(
                &raw,
                GPU_POSEIDON_PIPE_DEPTH_ENV,
                GPU_POSEIDON_PIPE_DEPTH_MIN,
                GPU_POSEIDON_PIPE_DEPTH_MAX,
            )
        })
    })
}

#[cfg(feature = "fastpq-gpu")]
fn parse_pipe_override(raw: &str, env_key: &str, min: usize, max: usize) -> Option<usize> {
    match raw.trim().parse::<usize>() {
        Ok(value) if (min..=max).contains(&value) => Some(value),
        Ok(value) => {
            warn!(
                target: "fastpq::poseidon",
                env = env_key,
                value,
                min,
                max,
                "ignoring out-of-range poseidon pipeline override"
            );
            None
        }
        Err(error) => {
            warn!(
                target: "fastpq::poseidon",
                env = env_key,
                %error,
                raw = raw.trim(),
                "failed to parse poseidon pipeline override; keeping default"
            );
            None
        }
    }
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
    let mode = if cfg!(feature = "fastpq-gpu") {
        ExecutionMode::Auto.resolve()
    } else {
        ExecutionMode::Cpu
    };

    #[cfg(feature = "fastpq-gpu")]
    {
        if mode != ExecutionMode::Cpu {
            if let Some(result) = hash_columns_gpu_overlap(trace, &planner) {
                return Ok(result);
            }
        }
    }

    let coefficients = trace_coefficients(trace, &planner, mode);

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
    planner: &Planner,
    mode: ExecutionMode,
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
        let domain_refs: Vec<&str> = trace
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect();
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
                if let Some(result) = hash_columns_gpu_pipelined_batch(&batch) {
                    notify_poseidon_pipeline_observer(
                        poseidon_policy,
                        "gpu_pipelined",
                        poseidon_backend,
                    );
                    return ColumnDigests::new(result, None);
                }
                if poseidon_backend.is_some() {
                    if let Some(result) = hash_columns_gpu_batch(&batch) {
                        notify_poseidon_pipeline_observer(
                            poseidon_policy,
                            "gpu_batch",
                            poseidon_backend,
                        );
                        return ColumnDigests::new(result, None);
                    }
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
            hash_field_with_domain(domain.as_bytes(), coeffs)
        })
        .collect();

    ColumnDigests::new(leaves, None)
}

pub(crate) struct TracePolynomialData {
    pub coefficients: Vec<Vec<u64>>,
    lde_state: LdeColumnsState,
    transfer_plan: transfer::TransferGadgetPlan,
}

struct PendingLdeState {
    dispatch: GpuLdeDispatch,
    planner: Planner,
}

enum LdeColumnsState {
    Ready(Vec<Vec<u64>>),
    Pending(Box<PendingLdeState>),
}

impl TracePolynomialData {
    pub(crate) fn lde_columns(&mut self) -> &Vec<Vec<u64>> {
        if !matches!(self.lde_state, LdeColumnsState::Ready(_)) {
            self.resolve_pending_lde();
        }
        match &self.lde_state {
            LdeColumnsState::Ready(columns) => columns,
            _ => unreachable!("pending state resolved before returning"),
        }
    }

    fn resolve_pending_lde(&mut self) {
        let PendingLdeState { dispatch, planner } =
            match std::mem::replace(&mut self.lde_state, LdeColumnsState::Ready(Vec::new())) {
                LdeColumnsState::Pending(pending) => *pending,
                LdeColumnsState::Ready(columns) => {
                    self.lde_state = LdeColumnsState::Ready(columns);
                    return;
                }
            };
        #[cfg(test)]
        let planner_for_check = planner.clone();
        let columns = match dispatch.wait() {
            Ok(Some(columns)) => columns,
            Ok(None) => {
                warn!(
                    target: "fastpq::trace",
                    "gpu lde pending dispatch yielded no columns; falling back to cpu"
                );
                planner.lde_columns(&self.coefficients)
            }
            Err(error) => {
                warn!(
                    target: "fastpq::trace",
                    %error,
                    "gpu lde pending dispatch failed; falling back to cpu"
                );
                planner.lde_columns(&self.coefficients)
            }
        };
        #[cfg(test)]
        {
            let cpu_columns = planner_for_check.lde_columns(&self.coefficients);
            assert_eq!(
                cpu_columns, columns,
                "lde gpu output diverged from cpu reference"
            );
        }
        self.lde_state = LdeColumnsState::Ready(columns);
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
    mode: ExecutionMode,
) -> TracePolynomialData {
    let coefficients = trace_coefficients(trace, planner, mode);
    let lde_state = if coefficients.is_empty() {
        LdeColumnsState::Ready(Vec::new())
    } else {
        match mode {
            ExecutionMode::Gpu => planner.lde_gpu_pending(&coefficients).map_or_else(
                || LdeColumnsState::Ready(planner.lde_columns(&coefficients)),
                |dispatch| {
                    LdeColumnsState::Pending(Box::new(PendingLdeState {
                        dispatch,
                        planner: planner.clone(),
                    }))
                },
            ),
            ExecutionMode::Cpu | ExecutionMode::Auto => {
                LdeColumnsState::Ready(planner.lde_columns(&coefficients))
            }
        }
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

/// Compute the traditional Merkle root by fully hashing all levels on the CPU.
pub fn merkle_root(leaves: &[u64]) -> u64 {
    merkle_root_with_first_level(leaves, None)
}

fn compute_merkle_level(input: &[u64]) -> Vec<u64> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut padded = input.to_vec();
    if padded.len() % 2 == 1 {
        let last = *padded.last().expect("non-empty vector");
        padded.push(last);
    }
    let mut next = Vec::with_capacity(padded.len() / 2);
    for pair in padded.chunks(2) {
        next.push(hash_field_with_domain(
            TRACE_NODE_DOMAIN,
            &[pair[0], pair[1]],
        ));
    }
    next
}

#[cfg(test)]
mod tests {
    use fastpq_isi::CANONICAL_PARAMETER_SETS;
    use iroha_crypto::Hash;
    use iroha_data_model::{
        asset::id::AssetDefinitionId,
        fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::to_bytes;

    use super::*;
    use crate::{
        ExecutionMode, OperationKind, PoseidonExecutionMode, PublicInputs, StateTransition,
        TransitionBatch, backend, gadgets::transfer, gpu,
    };

    fn sample_batch() -> TransitionBatch {
        let transcript = sample_transfer_transcript();
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
        assert_eq!(via_coeffs.fused_parents(), via_api.fused_parents());
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
                hash_field_with_domain(domain.as_bytes(), coeffs)
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
        assert!(offsets.iter().all(|slice| slice.len() % RATE == 0));
        let payload_len = columns[0].len() + 2;
        let padded_len = if payload_len % RATE == 0 {
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
            assert_eq!(
                &region[1..1 + columns[index].len()],
                columns[index].as_slice()
            );
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
                &region[1..1 + columns[index + 1].len()],
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
        let mut data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let cpu_hashes = hash_columns_from_coefficients(
            &trace,
            &data.coefficients,
            &planner,
            ExecutionMode::Cpu,
            PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
        );
        let domains: Vec<&str> = trace
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect();
        let batch = PoseidonColumnBatch::from_domains_and_columns(&domains, &data.coefficients)
            .expect("gpu batch");
        let gpu_hashes = match hash_columns_gpu_batch(&batch) {
            Some(values) => values,
            None => {
                eprintln!("skipping poseidon gpu parity test; dispatch declined");
                return;
            }
        };
        assert_eq!(
            cpu_hashes.leaves(),
            gpu_hashes.as_slice(),
            "gpu hashes diverged from cpu"
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
        let domains: Vec<&str> = trace
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect();
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &coefficients).expect("batch");
        let fused = match hash_columns_gpu_fused(&batch, ExecutionMode::Gpu) {
            Some(values) => values,
            None => {
                eprintln!("skipping poseidon fused parity test; fused dispatch declined");
                return;
            }
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

    fn sample_coefficients(planner: &Planner) -> Vec<Vec<u64>> {
        let trace = build_trace(&sample_batch()).expect("build");
        trace_coefficients(&trace, planner, ExecutionMode::Cpu)
    }

    #[test]
    fn pending_lde_falls_back_when_gpu_declines() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let coefficients = sample_coefficients(&planner);
        let expected = planner.lde_columns(&coefficients);

        let dispatch = GpuLdeDispatch::from_ready(gpu::LdeDispatch::ready(None));
        let pending = PendingLdeState {
            dispatch,
            planner: planner.clone(),
        };
        let mut data = TracePolynomialData {
            coefficients: coefficients.clone(),
            lde_state: LdeColumnsState::Pending(Box::new(pending)),
            transfer_plan: transfer::TransferGadgetPlan::default(),
        };

        assert_eq!(data.lde_columns(), &expected);
    }

    #[test]
    fn pending_lde_recovers_from_gpu_error() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let coefficients = sample_coefficients(&planner);
        let expected = planner.lde_columns(&coefficients);

        let error = gpu::GpuError::Execution {
            backend: backend::GpuBackend::Cuda,
            message: "simulated gpu failure".to_owned(),
        };
        let dispatch = GpuLdeDispatch::from_ready(gpu::LdeDispatch::from_error(error));
        let pending = PendingLdeState {
            dispatch,
            planner: planner.clone(),
        };
        let mut data = TracePolynomialData {
            coefficients: coefficients.clone(),
            lde_state: LdeColumnsState::Pending(Box::new(pending)),
            transfer_plan: transfer::TransferGadgetPlan::default(),
        };

        assert_eq!(data.lde_columns(), &expected);
    }

    #[test]
    fn transfer_witnesses_extracted_from_metadata() {
        let (batch, transcript) = batch_with_transfer_metadata();
        let trace = build_trace(&batch).expect("trace");
        let expected =
            transfer::transcripts_to_witnesses(&[transcript]).expect("witness extraction");
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
            assert_eq!(sibling_value, hash_to_field(&proof.sibling(0)));
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
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
        let expected =
            transfer::transcripts_to_witnesses(&[transcript]).expect("witness extraction");
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
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            ),
            amount: Numeric::from(42u32),
            from_balance_before: Numeric::from(200u32),
            from_balance_after: Numeric::from(158u32),
            to_balance_before: Numeric::from(1u32),
            to_balance_after: Numeric::from(43u32),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
        let batch_hash = Hash::prehashed([0x22; 32]);
        let digest = transfer::compute_poseidon_digest(&delta, &batch_hash);
        TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::new(b"authority"),
            poseidon_preimage_digest: Some(digest),
        }
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
