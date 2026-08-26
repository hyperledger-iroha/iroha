#![cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#![allow(
    dead_code,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::clone_on_copy,
    clippy::collapsible_if,
    clippy::derivable_impls,
    clippy::elidable_lifetime_names,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::manual_div_ceil,
    clippy::manual_inspect,
    clippy::manual_is_multiple_of,
    clippy::manual_range_contains,
    clippy::manual_slice_size_calculation,
    clippy::map_unwrap_or,
    clippy::missing_errors_doc,
    clippy::needless_collect,
    clippy::needless_pass_by_value,
    clippy::needless_return,
    clippy::option_if_let_else,
    clippy::or_fun_call,
    clippy::redundant_closure,
    clippy::redundant_closure_for_method_calls,
    clippy::redundant_pub_crate,
    clippy::return_self_not_must_use,
    clippy::single_match_else,
    clippy::suboptimal_flops,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref,
    clippy::uninlined_format_args,
    clippy::unnecessary_lazy_evaluations,
    clippy::useless_conversion
)]
//! Metal GPU bindings for FASTPQ.
use crate::{
    backend::GpuBackend,
    bn254,
    bn254_poseidon::Bn254PoseidonBatchSlice,
    bn254_poseidon_params::{
        BN254_LIMBS, BN254_POSEIDON_WIDTH, Bn254PoseidonWidth3Params, bn254_limbs_to_bytes,
        bn254_poseidon_width3_params,
    },
    gpu::GpuError,
    metal_config::{self, DeviceHints},
    overrides,
    poseidon::FIELD_MODULUS,
    poseidon_manifest::poseidon_manifest,
    trace::{PoseidonColumnBatch, PoseidonColumnSlice},
};
use block::{Block, ConcreteBlock};
use fastpq_isi::poseidon::STATE_WIDTH;
use halo2curves::{bn256::Fr as Bn254Fr, ff::PrimeField};
use iroha_zkp_halo2::{Bn254Scalar, IpaScalar};
use metal::{
    Buffer, CommandBuffer, CommandBufferRef, CommandQueue, CommandQueueRef, CompileOptions,
    ComputeCommandEncoderRef, ComputePipelineState, Device, DeviceRef, Library,
    MTLCommandBufferStatus, MTLDeviceLocation, MTLLanguageVersion, MTLResourceOptions, MTLSize,
    NSUInteger,
    foreign_types::{ForeignType, ForeignTypeRef},
    objc::{msg_send, rc::autoreleasepool, runtime::Object, sel, sel_impl},
};
use norito::json::{self, Value};
use smallvec::SmallVec;
#[cfg(test)]
use std::sync::Once;
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    ffi::c_void,
    iter::FusedIterator,
    mem,
    ops::Range,
    path::Path,
    process::{Command, Stdio},
    ptr,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant},
    vec::Vec,
};
use tracing::{debug, warn};
type MetalResult<T> = Result<T, GpuError>;
const POSEIDON_PERMUTE_KERNEL: &str = "poseidon_permute";
const POSEIDON_HASH_KERNEL: &str = "poseidon_hash_columns";
const POSEIDON_HASH_ROWS_KERNEL: &str = "poseidon_hash_rows";
const POSEIDON_TRACE_FUSED_KERNEL: &str = "poseidon_trace_fused";
const POSEIDON_TRACE_PARENTS_KERNEL: &str = "poseidon_trace_parents";
const FFT_KERNEL: &str = "fastpq_fft_columns";
const LDE_KERNEL: &str = "fastpq_lde_columns";
const POST_TILE_KERNEL: &str = "fastpq_fft_post_tiling";
const BN254_FFT_KERNEL: &str = "bn254_fft_columns";
const BN254_LDE_KERNEL: &str = "bn254_lde_columns";
const BN254_POSEIDON_HASH_KERNEL: &str = "bn254_poseidon_hash_words";
#[cfg(test)]
const GOLDILOCKS_GENERATOR: u64 = 7;
const MIN_FFT_COLUMNS_PER_BATCH: u32 = 1;
const MAX_FFT_COLUMNS_PER_BATCH: u32 = 64;
const FFT_COLUMNS_TARGET_THREADS: u32 = 4_096;
const FFT_COLUMNS_ENV: &str = "FASTPQ_METAL_FFT_COLUMNS";
const MIN_LDE_COLUMNS_PER_BATCH: u32 = 1;
const MAX_LDE_COLUMNS_PER_BATCH: u32 = 64;
const LDE_COLUMNS_ENV: &str = "FASTPQ_METAL_LDE_COLUMNS";
const LDE_COLUMNS_TARGET_THREADS: u32 = 4_096;
const DEFAULT_LDE_COLUMNS_PER_BATCH: u32 = 2;
const FFT_THREADGROUP_CAPACITY: u32 = 256;
const FFT_TILE_STAGE_LIMIT: u32 = metal_config::FFT_TILE_STAGE_LIMIT_MAX;
/// Must match `FFT_TILE_STAGE_CAP` in `metal/kernels/ntt_stage.metal`.
const LDE_TILE_STAGE_ENV: &str = "FASTPQ_METAL_LDE_TILE_STAGES";
const POSEIDON_THREADGROUP_CAPACITY: u32 = 256;
const POSEIDON_DISPATCH_PIPE_DEPTH: usize = 2;
const BN254_POSEIDON_THREADGROUP_CAPACITY: u32 = 128;
const MIN_POSEIDON_STATES_PER_BATCH: u32 = 1;
const QUEUE_FANOUT_ENV: &str = "FASTPQ_METAL_QUEUE_FANOUT";
const QUEUE_COLUMN_THRESHOLD_ENV: &str = "FASTPQ_METAL_COLUMN_THRESHOLD";
const MIN_QUEUE_FANOUT: usize = 1;
const MAX_QUEUE_FANOUT: usize = 4;
const DISCRETE_QUEUE_FANOUT: usize = 2;
const MIN_QUEUE_COLUMN_THRESHOLD: u32 = 1;
const DEFAULT_QUEUE_COLUMN_THRESHOLD: u32 = 16;
const MAX_BUFFER_POOL_BUFFERS: usize = 8;
const MAX_BUFFER_POOL_PAGES_PER_BUFFER: usize = 1_024;
const MAX_BUFFER_POOL_CACHED_PAGES: usize = 4_096;
const MAX_RETAINED_DISPATCH_TICKETS: usize = 16;
const MAX_RETAINED_TELEMETRY_SAMPLES: usize = 4_096;
const BN254_TWIDDLE_CACHE_MAX_BYTES: u64 = 256 * 1024 * 1024;
const GOLDILOCKS_TWIDDLE_CACHE_MAX_ENTRIES: usize = 64;
// Metal's bytes-no-copy API requires both ends of the wrapped region to be
// page-aligned. A 16 KiB region satisfies both 4 KiB Intel and 16 KiB Apple
// Silicon macOS page sizes.
const METAL_BUFFER_PAGE_BYTES: usize = 16 * 1024;
const METAL_BUFFER_PAGE_WORDS: usize = METAL_BUFFER_PAGE_BYTES / mem::size_of::<u64>();
const GOLDILOCKS_TWO_ADICITY: u32 = 32;
const DEFAULT_MAX_COMMAND_BUFFERS: usize = 4;
const COLUMN_STAGING_PIPE_DEPTH: usize = 2;
const ADAPTIVE_TARGET_MS: f64 = 2.0;
const ADAPTIVE_BACKOFF_RATIO: f64 = 1.3;
const METAL_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
// Permit acquisition is host-side backpressure, so it must not fail before a
// submitted command is considered hung.
const METAL_COMMAND_PERMIT_TIMEOUT: Duration = METAL_COMMAND_TIMEOUT;
fn debug_env_var(name: &str) -> Option<String> {
    overrides::guard_env_override(|| overrides::debug_env_string(name))
}
fn debug_env_bool(name: &str) -> Option<bool> {
    overrides::guard_env_override(|| overrides::debug_env_bool(name))
}
// BN254 Metal pipelines are bundled to keep the host in sync with the metallib.
// Transcript hashing uses a narrow context so it does not have to compile the
// full prover pipeline set before validating word-batch acceleration.
static METAL_CONTEXT: OnceLock<MetalResult<MetalPipelines>> = OnceLock::new();
static BN254_POSEIDON_CONTEXT: OnceLock<MetalResult<Bn254PoseidonMetalPipelines>> = OnceLock::new();
static FFT_BATCH_OVERRIDE: OnceLock<Option<u32>> = OnceLock::new();
static LDE_BATCH_OVERRIDE: OnceLock<Option<u32>> = OnceLock::new();
static LDE_TILE_OVERRIDE: OnceLock<Option<u32>> = OnceLock::new();
static BUFFER_POOL: OnceLock<Mutex<BufferPool>> = OnceLock::new();
static LDE_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
static LDE_STATS: OnceLock<Mutex<Option<LdeHostStats>>> = OnceLock::new();
static KERNEL_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
static KERNEL_STATS: OnceLock<Mutex<Vec<KernelStatsSample>>> = OnceLock::new();
static COMMAND_SEMAPHORE: OnceLock<CommandSemaphore> = OnceLock::new();
static COMMAND_SEMAPHORE_STATE: OnceLock<CommandSemaphoreState> = OnceLock::new();
static QUEUE_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
static QUEUE_STATS: OnceLock<Mutex<QueueStatsState>> = OnceLock::new();
static COLUMN_STAGING_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
static COLUMN_STAGING_STATS: OnceLock<Mutex<ColumnStagingStats>> = OnceLock::new();
static POST_TILE_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
static POST_TILE_STATS: OnceLock<Mutex<Vec<PostTileSample>>> = OnceLock::new();
static RUNTIME_QUEUE_FANOUT_OVERRIDE: OnceLock<usize> = OnceLock::new();
static RUNTIME_QUEUE_THRESHOLD_OVERRIDE: OnceLock<u32> = OnceLock::new();
static TWIDDLE_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
static TWIDDLE_STATS: OnceLock<Mutex<TwiddleCacheStats>> = OnceLock::new();
static ADAPTIVE_SCHEDULER: OnceLock<AdaptiveScheduler> = OnceLock::new();
static GPU_CORE_COUNT: OnceLock<Option<usize>> = OnceLock::new();
static LAST_LDE_TILE_LIMIT: AtomicU32 = AtomicU32::new(0);
static MAX_IN_FLIGHT_ENV_OVERRIDE: OnceLock<Option<usize>> = OnceLock::new();
static THREADGROUP_ENV_OVERRIDE: OnceLock<Option<u64>> = OnceLock::new();
static DISPATCH_TRACE_ENV: OnceLock<bool> = OnceLock::new();
/// Return `GpuError::Unsupported` when Metal is unavailable; otherwise load
/// the BN254 Poseidon word-batch pipeline used by FASTPQ transcript hashing.
pub(crate) fn bn254_status() -> MetalResult<()> {
    if select_metal_device().is_none() {
        return Err(GpuError::Unsupported(GpuBackend::Metal));
    }
    let ctx = bn254_poseidon_context()?;
    let _ = &ctx.bn254_poseidon_hash;
    Ok(())
}
/// Pending BN254 Metal FFT dispatch.
///
/// WP2-C will replace this with a real kernel-backed guard when the Metal
/// pipelines are implemented.
pub(crate) struct PendingBn254Fft<'a> {
    pending: Option<PendingColumns<'a>>,
}
impl<'a> PendingBn254Fft<'a> {
    fn empty() -> Self {
        Self { pending: None }
    }
    fn new(pending: PendingColumns<'a>) -> Self {
        Self {
            pending: Some(pending),
        }
    }
    /// Wait for the dispatch to complete.
    pub fn wait(mut self) -> MetalResult<()> {
        if let Some(pending) = self.pending.take() {
            pending.wait()?;
        }
        Ok(())
    }
}
pub fn bn254_fft_columns(columns: &mut [Vec<u64>], log_size: u32) -> MetalResult<()> {
    bn254_validate_log(log_size)?;
    if columns.is_empty() {
        return Ok(());
    }
    bn254_fft_columns_async(columns, log_size)?.wait()
}
/// Enqueue a BN254 FFT on the Metal backend.
pub(crate) fn bn254_fft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
) -> MetalResult<PendingBn254Fft<'a>> {
    bn254_validate_log(log_size)?;
    if columns.is_empty() {
        return Ok(PendingBn254Fft::empty());
    }
    let pending = dispatch_bn254_fft_columns(columns, log_size)?;
    Ok(PendingBn254Fft::new(pending))
}
/// Pending BN254 Metal LDE dispatch.
///
/// Replaced by real pipeline once BN254 kernels ship.
pub(crate) struct PendingBn254Lde {
    pending: Option<PendingLde>,
}
impl PendingBn254Lde {
    fn empty() -> Self {
        Self { pending: None }
    }
    fn new(pending: PendingLde) -> Self {
        Self {
            pending: Some(pending),
        }
    }
    /// Wait for the dispatch to complete and collect the evaluated columns.
    pub fn wait(mut self) -> MetalResult<Option<Vec<Vec<u64>>>> {
        if let Some(pending) = self.pending.take() {
            pending.wait()
        } else {
            Ok(Some(Vec::new()))
        }
    }
}
pub fn bn254_lde_columns(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    coset: [u64; BN254_LIMBS],
) -> MetalResult<Option<Vec<Vec<u64>>>> {
    let _ = bn254_lde_domain_lengths(trace_log, blowup_log)?;
    if coeffs.is_empty() {
        return Ok(Some(Vec::new()));
    }
    bn254_lde_columns_async(coeffs, trace_log, blowup_log, coset)?.wait()
}
fn bn254_smoke_test() -> MetalResult<()> {
    // Minimal FFT check to prove BN254 kernels are reachable.
    const FFT_LOG: u32 = 3;
    let mut fft_columns = sample_bn254_columns(FFT_LOG, 1);
    bn254_fft_columns(&mut fft_columns, FFT_LOG)?;
    // LDE smoke test with a small trace and coset to validate staging/layout.
    const TRACE_LOG: u32 = 2;
    const BLOWUP_LOG: u32 = 1;
    let coeffs = sample_bn254_columns(TRACE_LOG, 1);
    let coset = sample_bn254_coset();
    if let Some(eval_columns) = bn254_lde_columns(&coeffs, TRACE_LOG, BLOWUP_LOG, coset)? {
        let expected_len = (1usize << (TRACE_LOG + BLOWUP_LOG)) * BN254_LIMBS;
        if eval_columns
            .iter()
            .any(|column| column.len() != expected_len)
        {
            return Err(GpuError::Execution {
                backend: GpuBackend::Metal,
                message: "BN254 LDE output length mismatch during smoke test".into(),
            });
        }
    }
    Ok(())
}
/// Enqueue a BN254 LDE on the Metal backend.
pub(crate) fn bn254_lde_columns_async(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    coset: [u64; BN254_LIMBS],
) -> MetalResult<PendingBn254Lde> {
    let _ = bn254_lde_domain_lengths(trace_log, blowup_log)?;
    if coeffs.is_empty() {
        return Ok(PendingBn254Lde::empty());
    }
    let pending = dispatch_bn254_lde_columns(coeffs, trace_log, blowup_log, coset)?;
    Ok(PendingBn254Lde::new(pending))
}
fn dispatch_bn254_fft_columns<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
) -> MetalResult<PendingColumns<'a>> {
    bn254_validate_log(log_size)?;
    let element_extent = bn254_column_extent(columns)?;
    if element_extent == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 FFT requires at least one coefficient",
        ));
    }
    let expected = bn254_domain_len(log_size)?;
    if element_extent != expected {
        return Err(GpuError::InvalidInput(
            "BN254 FFT columns must match the requested log size",
        ));
    }
    for column in columns.iter() {
        bn254::validate_canonical_limbs(column).map_err(GpuError::InvalidInput)?;
    }
    let limb_extent = expected
        .checked_mul(BN254_LIMBS)
        .ok_or(GpuError::InvalidInput(
            "BN254 FFT column length exceeds platform limits",
        ))?;
    let column_len_u64 = u64::try_from(expected)
        .map_err(|_| GpuError::InvalidInput("BN254 FFT column length exceeds 64-bit range"))?;
    let context = metal_context()?;
    validate_metal_pooled_word_len(&context.device, limb_extent)?;
    let twiddle_buffer = context.bn254_fft_twiddle_buffer(log_size)?;
    let limits = pipeline_limits(&context.bn254_fft);
    let tuning = metal_config::fft_tuning(log_size, limits.exec_width, limits.max_threads);
    let column_count = columns.len();
    let column_count_u32 = u32::try_from(column_count)
        .map_err(|_| GpuError::InvalidInput("BN254 column count exceeds 32-bit range"))?;
    let pipe_depth = COLUMN_STAGING_PIPE_DEPTH.max(1);
    let mut slots: Vec<Option<ColumnBatchTicket>> = Vec::with_capacity(pipe_depth);
    slots.resize_with(pipe_depth, || None);
    let selection = select_fft_batch(tuning.threadgroup_lanes);
    let batches = column_batch_ranges(column_count_u32, 1);
    let profile = KernelProfileParams {
        kind: KernelKind::Fft,
        bytes: fft_bytes_per_batch(column_len_u64, 1),
        elements: column_len_u64,
        columns: 1,
    };
    let mut rollback = ColumnMutationRollback::capture(columns)?;
    let dispatch_result = (|| -> MetalResult<()> {
        for (batch_index, (offset, batch_columns)) in batches.into_iter().enumerate() {
            let slot_index = batch_index % pipe_depth;
            if let Some(ticket) = slots[slot_index].take() {
                ticket.wait(columns, limb_extent, true)?;
            }
            let start = usize::try_from(offset).expect("column offset fits usize");
            let width = usize::try_from(batch_columns).expect("batch column count fits usize");
            let range = start..start + width;
            let mut buffer = flatten_with_stats(&columns[range.clone()], ColumnStagingPhase::Fft)?;
            let metal_buffer = shared_pooled_buffer(&context.device, &mut buffer)?;
            let (queue, queue_index) = context.queues.select(column_count_u32, batch_index);
            let (threadgroups, threadgroup) =
                bn254_threadgroup_geometry(&context.bn254_fft, column_len_u64);
            let sample_request = selection.sample_for(1);
            let mut ticket = submit_compute_with_geometry(
                queue,
                queue_index,
                &context.bn254_fft,
                Some((threadgroups, threadgroup, column_len_u64)),
                column_len_u64,
                Some(profile),
                sample_request.is_some(),
                |encoder: &ComputeCommandEncoderRef| {
                    encoder.set_buffer(0, Some(&metal_buffer), 0);
                    encoder.set_bytes(
                        1,
                        mem::size_of::<u32>() as u64,
                        ptr::from_ref(&log_size).cast(),
                    );
                    encoder.set_buffer(2, Some(&twiddle_buffer), 0);
                },
            )?;
            if let Some(sample) = sample_request {
                ticket = ticket.with_adaptive_sample(sample);
            }
            let mut tickets = SmallVec::<[DispatchTicket; 2]>::new();
            tickets.push(ticket);
            slots[slot_index] = Some(ColumnBatchTicket {
                range,
                buffer,
                metal_buffer,
                tickets,
            });
        }
        Ok(())
    })();
    rollback_columns_on_error(dispatch_result, columns, &mut rollback)?;
    let pending_batches: Vec<ColumnBatchTicket> = slots.into_iter().flatten().collect();
    Ok(PendingColumns::new(
        columns,
        limb_extent,
        twiddle_buffer,
        pending_batches,
        rollback,
    ))
}
fn dispatch_bn254_lde_columns(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    coset: [u64; BN254_LIMBS],
) -> MetalResult<PendingLde> {
    let (expected_trace, _, eval_len) = bn254_lde_domain_lengths(trace_log, blowup_log)?;
    let trace_extent = bn254_column_extent(coeffs)?;
    if trace_extent == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 LDE requires at least one coefficient",
        ));
    }
    if trace_extent != expected_trace {
        return Err(GpuError::InvalidInput(
            "BN254 LDE coefficients must match the trace log size",
        ));
    }
    for column in coeffs {
        bn254::validate_canonical_limbs(column).map_err(GpuError::InvalidInput)?;
    }
    let coset_scalar = bn254_scalar_from_canonical_limbs(&coset)?;
    let coset_limbs = bn254_scalar_to_canonical_limbs(&coset_scalar);
    let eval_limbs = coeffs
        .len()
        .checked_mul(eval_len)
        .and_then(|len| len.checked_mul(BN254_LIMBS))
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE output length exceeds limits",
        ))?;
    let coeff_limbs = coeffs
        .len()
        .checked_mul(coeffs[0].len())
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE coefficient length exceeds limits",
        ))?;
    let context = metal_context()?;
    validate_metal_pooled_word_len(&context.device, coeff_limbs)?;
    validate_metal_pooled_word_len(&context.device, eval_limbs)?;
    let stage_twiddle_buffer = context.bn254_lde_twiddle_buffer(trace_log, blowup_log)?;
    let mut coeff_buffer = flatten_with_stats(coeffs, ColumnStagingPhase::Lde)?;
    let stats_enabled = LDE_STATS_ENABLED.load(Ordering::Acquire);
    let zero_timer = stats_enabled.then(|| Instant::now());
    let mut eval_buffer = PooledBuffer::zeroed(eval_limbs)?;
    let host_stats = zero_timer.map(|start| LdeHostStats {
        zero_fill_bytes: eval_buffer.len().saturating_mul(mem::size_of::<u64>()),
        zero_fill_ms: elapsed_ms(start.elapsed()),
        queue_delta: None,
    });
    let coeff_metal = shared_pooled_buffer(&context.device, &mut coeff_buffer)?;
    let eval_metal = shared_pooled_buffer(&context.device, &mut eval_buffer)?;
    let coset_buffer = upload_bn254_coset(&context.device, &coset_limbs)?;
    let column_count = coeffs.len();
    let column_count_u32 = u32::try_from(column_count)
        .map_err(|_| GpuError::InvalidInput("BN254 column count exceeds 32-bit range"))?;
    let eval_len_u64 = u64::try_from(eval_len)
        .map_err(|_| GpuError::InvalidInput("BN254 eval length exceeds 64-bit representation"))?;
    let (mut tickets, ticket_window) = pending_ticket_window::<DispatchTicket>()?;
    let trace_len_u64 = u64::try_from(trace_extent)
        .map_err(|_| GpuError::InvalidInput("BN254 trace length exceeds 64-bit range"))?;
    for column in 0..column_count {
        if let Some(ticket) = pop_oldest_ticket_if_full(&mut tickets, ticket_window) {
            wait_for_ticket(ticket)?;
        }
        let coeff_offset = column
            .checked_mul(trace_extent)
            .and_then(|v| v.checked_mul(BN254_LIMBS))
            .and_then(|v| v.checked_mul(mem::size_of::<u64>()))
            .ok_or(GpuError::InvalidInput(
                "BN254 coefficient offset exceeds limits",
            ))?;
        let eval_offset = column
            .checked_mul(eval_len)
            .and_then(|v| v.checked_mul(BN254_LIMBS))
            .and_then(|v| v.checked_mul(mem::size_of::<u64>()))
            .ok_or(GpuError::InvalidInput(
                "BN254 evaluation offset exceeds limits",
            ))?;
        let coeff_offset_bytes = u64::try_from(coeff_offset)
            .map_err(|_| GpuError::InvalidInput("BN254 coefficient offset exceeds 64-bit range"))?;
        let eval_offset_bytes = u64::try_from(eval_offset)
            .map_err(|_| GpuError::InvalidInput("BN254 evaluation offset exceeds 64-bit range"))?;
        let (queue, queue_index) = context.queues.select(column_count_u32, column);
        let (threadgroups, threadgroup) =
            bn254_threadgroup_geometry(&context.bn254_lde, eval_len_u64);
        let profile = KernelProfileParams {
            kind: KernelKind::Lde,
            bytes: lde_bytes_per_batch(trace_len_u64, eval_len_u64, 1),
            elements: eval_len_u64,
            columns: 1,
        };
        let ticket = submit_compute_with_geometry(
            queue,
            queue_index,
            &context.bn254_lde,
            Some((threadgroups, threadgroup, eval_len_u64)),
            eval_len_u64,
            Some(profile),
            false,
            |encoder: &ComputeCommandEncoderRef| {
                encoder.set_buffer(0, Some(&coeff_metal), coeff_offset_bytes);
                encoder.set_buffer(1, Some(&eval_metal), eval_offset_bytes);
                encoder.set_bytes(
                    2,
                    mem::size_of::<u32>() as u64,
                    ptr::from_ref(&trace_log).cast(),
                );
                encoder.set_bytes(
                    3,
                    mem::size_of::<u32>() as u64,
                    ptr::from_ref(&blowup_log).cast(),
                );
                encoder.set_buffer(4, Some(&coset_buffer), 0);
                encoder.set_buffer(5, Some(&stage_twiddle_buffer), 0);
            },
        )?;
        tickets.push(ticket);
    }
    Ok(PendingLde::new(
        column_count,
        eval_len,
        BN254_LIMBS,
        coeff_buffer,
        eval_buffer,
        coeff_metal,
        eval_metal,
        stage_twiddle_buffer,
        tickets,
        host_stats,
    ))
}
#[cfg(all(test, feature = "fastpq-gpu", target_os = "macos"))]
fn ensure_multi_queue_env() {
    static INIT: Once = Once::new();
    INIT.call_once(|| set_queue_policy_override_for_tests(2, 1));
}
#[cfg(all(test, feature = "fastpq-gpu", target_os = "macos"))]
fn unwrap_or_skip<T>(result: MetalResult<T>, context: &str) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(GpuError::Unsupported(_)) => {
            eprintln!("skipping Metal {context} test: backend unavailable");
            None
        }
        Err(err) => panic!("Metal {context} failed: {err}"),
    }
}
#[cfg(test)]
mod bn254_parity {
    use crate::bn254::{cpu_fft, cpu_lde};

    use super::{ensure_multi_queue_env, unwrap_or_skip, *};
    fn sample_columns(log_size: u32, column_count: usize) -> Vec<Vec<u64>> {
        let len = 1usize << log_size;
        let mut columns = Vec::with_capacity(column_count);
        for column in 0..column_count {
            let mut data = Vec::with_capacity(len * BN254_LIMBS);
            for row in 0..len {
                let value = Bn254Scalar::from(((column as u64 + 1) * 31).wrapping_add(row as u64));
                data.extend_from_slice(&bn254_scalar_to_canonical_limbs(&value));
            }
            columns.push(data);
        }
        columns
    }
    fn canonical_to_scalars(column: &[u64]) -> Vec<Bn254Scalar> {
        column
            .chunks_exact(BN254_LIMBS)
            .map(|chunk| bn254_limbs_slice_to_scalar(chunk).expect("valid scalar"))
            .collect()
    }
    fn scalars_to_canonical(columns: &[Vec<Bn254Scalar>]) -> Vec<Vec<u64>> {
        columns
            .iter()
            .map(|column| {
                let mut out = Vec::with_capacity(column.len() * BN254_LIMBS);
                for value in column {
                    out.extend_from_slice(&bn254_scalar_to_canonical_limbs(value));
                }
                out
            })
            .collect()
    }
    #[test]
    fn fft_matches_cpu_reference() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let log_size = 4;
        let column_count = 2;
        let mut gpu_columns = sample_columns(log_size, column_count);
        let mut cpu_columns: Vec<Vec<Bn254Scalar>> = gpu_columns
            .iter()
            .map(|column| canonical_to_scalars(column))
            .collect();
        let twiddles = bn254_stage_twiddles_scalars(log_size).expect("twiddles");
        cpu_fft(&mut cpu_columns, log_size, &twiddles);
        let cpu_expected = scalars_to_canonical(&cpu_columns);
        if unwrap_or_skip(
            super::bn254_fft_columns(&mut gpu_columns, log_size),
            "bn254_fft",
        )
        .is_none()
        {
            return;
        }
        assert_eq!(gpu_columns, cpu_expected);
    }
    #[test]
    fn lde_matches_cpu_reference() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let trace_log = 3;
        let blowup_log = 2;
        let coset = Bn254Scalar::from(5u64);
        let coeffs = sample_columns(trace_log, 2);
        let coeff_scalars: Vec<Vec<Bn254Scalar>> = coeffs
            .iter()
            .map(|column| canonical_to_scalars(column))
            .collect();
        let twiddles = bn254_stage_twiddles_scalars(trace_log + blowup_log).expect("lde twiddles");
        let cpu_eval = cpu_lde(&coeff_scalars, trace_log, blowup_log, &twiddles, coset);
        let cpu_expected = scalars_to_canonical(&cpu_eval);
        let coset_limbs = bn254_scalar_to_canonical_limbs(&coset);
        let gpu_eval = match unwrap_or_skip(
            super::bn254_lde_columns(&coeffs, trace_log, blowup_log, coset_limbs),
            "bn254_lde",
        ) {
            Some(value) => value.expect("Metal BN254 backend declined workload"),
            None => return,
        };
        assert_eq!(gpu_eval, cpu_expected);
    }
}
static TEST_QUEUE_FANOUT_OVERRIDE: OnceLock<usize> = OnceLock::new();
#[cfg(test)]
static TEST_QUEUE_THRESHOLD_OVERRIDE: OnceLock<u32> = OnceLock::new();
#[cfg(test)]
fn set_queue_policy_override_for_tests(fanout: usize, threshold: u32) {
    let _ = TEST_QUEUE_FANOUT_OVERRIDE.set(fanout);
    let _ = TEST_QUEUE_THRESHOLD_OVERRIDE.set(threshold);
}
struct PipelineLimits {
    exec_width: u32,
    max_threads: u32,
}
struct DispatchTicket {
    command: CommandBuffer,
    trace_label: Option<String>,
    timing_start: Option<Instant>,
    kernel_context: Option<KernelDispatchContext>,
    permit: CommandPermit,
    adaptive_sample: Option<AdaptiveSample>,
}
impl DispatchTicket {
    fn with_adaptive_sample(mut self, sample: AdaptiveSample) -> Self {
        self.adaptive_sample = Some(sample);
        self
    }
}
/// Host-side metrics captured while preparing GPU LDE buffers.
#[derive(Clone, Debug, Default)]
pub struct LdeHostStats {
    /// Number of bytes zeroed on the host before launching the GPU LDE kernel.
    pub zero_fill_bytes: usize,
    /// Milliseconds spent zeroing the evaluation buffer.
    pub zero_fill_ms: f64,
    /// Queue-depth delta observed while zero-fill was running.
    pub queue_delta: Option<QueueDepthStats>,
}
/// Enable or disable capture of [`LdeHostStats`] for subsequent `lde_columns` calls.
pub fn enable_lde_host_stats(enabled: bool) {
    LDE_STATS_ENABLED.store(enabled, Ordering::Release);
    if !enabled {
        if let Some(store) = LDE_STATS.get() {
            if let Ok(mut guard) = store.lock() {
                guard.take();
            }
        }
    }
}
/// Returns the most recent [`LdeHostStats`] sample recorded by `lde_columns`, clearing it.
pub fn take_lde_host_stats() -> Option<LdeHostStats> {
    if !LDE_STATS_ENABLED.load(Ordering::Acquire) {
        return None;
    }
    LDE_STATS
        .get()
        .and_then(|store| store.lock().ok())
        .and_then(|mut guard| guard.take())
}
/// Cache hit/miss totals for stage twiddle buffers.
#[derive(Clone, Copy, Debug, Default)]
pub struct TwiddleCacheStats {
    /// Number of cache hits recorded.
    pub hits: u64,
    /// Number of cache misses recorded.
    pub misses: u64,
    /// Estimated milliseconds spent uploading twiddles before caching.
    pub before_ms: f64,
    /// Milliseconds spent uploading twiddles after caching (actual work).
    pub after_ms: f64,
}
/// Enable or disable capture of [`TwiddleCacheStats`] for subsequent twiddle lookups.
pub fn enable_twiddle_cache_stats(enabled: bool) {
    TWIDDLE_STATS_ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        if let Some(store) = TWIDDLE_STATS.get() {
            if let Ok(mut guard) = store.lock() {
                *guard = TwiddleCacheStats::default();
            }
        }
    }
}
/// Returns the accumulated [`TwiddleCacheStats`], clearing the snapshot.
pub fn take_twiddle_cache_stats() -> Option<TwiddleCacheStats> {
    if !TWIDDLE_STATS_ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let store = TWIDDLE_STATS.get_or_init(|| Mutex::new(TwiddleCacheStats::default()));
    store.lock().ok().map(|mut guard| {
        let stats = *guard;
        *guard = TwiddleCacheStats::default();
        stats
    })
}
fn record_twiddle_cache_sample(duration_ms: f64, hit: bool) {
    if !TWIDDLE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store = TWIDDLE_STATS.get_or_init(|| Mutex::new(TwiddleCacheStats::default()));
    if let Ok(mut guard) = store.lock() {
        guard.before_ms += duration_ms;
        if hit {
            guard.hits = guard.hits.saturating_add(1);
        } else {
            guard.misses = guard.misses.saturating_add(1);
            guard.after_ms += duration_ms;
        }
    }
}
/// Enable or disable queue depth instrumentation for subsequent Metal dispatches.
pub fn enable_queue_depth_stats(enabled: bool) {
    QUEUE_STATS_ENABLED.store(enabled, Ordering::Release);
    let store = QUEUE_STATS.get_or_init(|| Mutex::new(QueueStatsState::default()));
    if let Ok(mut guard) = store.lock() {
        guard.reset();
    }
    COLUMN_STAGING_STATS_ENABLED.store(enabled, Ordering::Release);
    if enabled {
        reset_column_staging_stats();
    }
}
/// Returns the most recent queue depth snapshot recorded since statistics were enabled.
pub fn take_queue_depth_stats() -> Option<QueueDepthStats> {
    if !QUEUE_STATS_ENABLED.load(Ordering::Acquire) {
        return None;
    }
    let store = QUEUE_STATS.get_or_init(|| Mutex::new(QueueStatsState::default()));
    let mut guard = store.lock().ok()?;
    guard.advance(Instant::now());
    let snapshot = guard.snapshot(command_semaphore().limit());
    guard.reset();
    Some(snapshot)
}
/// Returns the accumulated column staging telemetry, clearing the current snapshot.
pub fn take_column_staging_stats() -> Option<ColumnStagingStats> {
    if !COLUMN_STAGING_STATS_ENABLED.load(Ordering::Acquire) {
        return None;
    }
    let store = COLUMN_STAGING_STATS.get_or_init(|| Mutex::new(ColumnStagingStats::default()));
    store.lock().ok().map(|mut guard| mem::take(&mut *guard))
}
/// Returns a snapshot of the adaptive scheduling heuristics, if initialised.
pub fn adaptive_schedule_snapshot() -> Option<AdaptiveScheduleSnapshot> {
    let scheduler = ADAPTIVE_SCHEDULER.get();
    let (mut fft_snapshot, mut lde_snapshot, poseidon_snapshot) =
        scheduler.map_or((None, None, None), |sched| sched.snapshot());
    if fft_snapshot.is_none() {
        if let Some(value) = fft_batch_override() {
            fft_snapshot = Some(BatchHeuristicSnapshot {
                columns: value,
                recommended: value,
                max_columns: value,
                target_ms: ADAPTIVE_TARGET_MS,
                last_duration_ms: None,
                samples: 0,
                override_active: true,
            });
        }
    }
    if lde_snapshot.is_none() {
        if let Some(value) = lde_batch_override() {
            lde_snapshot = Some(BatchHeuristicSnapshot {
                columns: value,
                recommended: value,
                max_columns: value,
                target_ms: ADAPTIVE_TARGET_MS,
                last_duration_ms: None,
                samples: 0,
                override_active: true,
            });
        }
    }
    if fft_snapshot.is_none()
        && lde_snapshot.is_none()
        && poseidon_snapshot.is_none()
        && COMMAND_SEMAPHORE_STATE.get().is_none()
    {
        return None;
    }
    let lde_tile = LAST_LDE_TILE_LIMIT.load(Ordering::Acquire);
    let poseidon_multiplier = metal_config::poseidon_batch_multiplier();
    Some(AdaptiveScheduleSnapshot {
        max_in_flight: command_limit_snapshot(),
        fft: fft_snapshot,
        lde: lde_snapshot,
        poseidon: poseidon_snapshot,
        poseidon_batch_multiplier: Some(poseidon_multiplier),
        lde_tile_stage_limit: (lde_tile > 0).then_some(lde_tile),
    })
}
/// Returns a snapshot of the current queue depth metrics without resetting the counters.
pub fn snapshot_queue_depth_stats() -> Option<QueueDepthStats> {
    if !QUEUE_STATS_ENABLED.load(Ordering::Acquire) {
        return None;
    }
    let store = QUEUE_STATS.get_or_init(|| Mutex::new(QueueStatsState::default()));
    let mut guard = store.lock().ok()?;
    guard.advance(Instant::now());
    Some(guard.snapshot(command_semaphore().limit()))
}
fn reset_column_staging_stats() {
    let store = COLUMN_STAGING_STATS.get_or_init(|| Mutex::new(ColumnStagingStats::default()));
    if let Ok(mut guard) = store.lock() {
        *guard = ColumnStagingStats::default();
    }
}
fn record_staging_flatten(phase: ColumnStagingPhase, duration: Duration) {
    if !COLUMN_STAGING_STATS_ENABLED.load(Ordering::Acquire) {
        return;
    }
    let store = COLUMN_STAGING_STATS.get_or_init(|| Mutex::new(ColumnStagingStats::default()));
    if let Ok(mut guard) = store.lock() {
        let delta = elapsed_ms(duration);
        guard.record_flatten_sample(phase, delta);
    }
}
fn record_staging_wait(phase: ColumnStagingPhase, duration: Duration) {
    if !COLUMN_STAGING_STATS_ENABLED.load(Ordering::Acquire) {
        return;
    }
    let store = COLUMN_STAGING_STATS.get_or_init(|| Mutex::new(ColumnStagingStats::default()));
    if let Ok(mut guard) = store.lock() {
        let delta = elapsed_ms(duration);
        guard.record_wait_sample(phase, delta);
    }
}
/// Enable or disable capture of per-dispatch Metal kernel statistics.
pub fn enable_kernel_stats(enabled: bool) {
    KERNEL_STATS_ENABLED.store(enabled, Ordering::Relaxed);
    let store = KERNEL_STATS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = store.lock() {
        guard.clear();
    }
}
/// Returns the collected Metal kernel statistics, clearing the current snapshot.
pub fn take_kernel_stats() -> Option<Vec<KernelStatsSample>> {
    if !KERNEL_STATS_ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let store = KERNEL_STATS.get_or_init(|| Mutex::new(Vec::new()));
    store.lock().ok().map(|mut guard| mem::take(&mut *guard))
}
/// Enable or disable capture of post-tiling dispatch samples.
pub fn enable_post_tile_stats(enabled: bool) {
    POST_TILE_STATS_ENABLED.store(enabled, Ordering::Relaxed);
    let store = POST_TILE_STATS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = store.lock() {
        guard.clear();
    }
}
/// Returns the recorded post-tiling dispatch samples, clearing the snapshot.
pub fn take_post_tile_stats() -> Option<Vec<PostTileSample>> {
    if !POST_TILE_STATS_ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let store = POST_TILE_STATS.get_or_init(|| Mutex::new(Vec::new()));
    store.lock().ok().map(|mut guard| mem::take(&mut *guard))
}
fn record_post_tile_sample(kind: KernelKind, log_len: u32, stage_start: u32, columns: u32) {
    if !POST_TILE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store = POST_TILE_STATS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = store.lock() {
        push_bounded_telemetry_sample(
            &mut guard,
            PostTileSample {
                kind,
                log_len,
                stage_start,
                columns,
            },
        );
    }
}
/// Aggregated queue depth metrics captured while GPU dispatches were in flight.
#[derive(Clone, Debug, Default)]
pub struct QueueDepthStats {
    /// Configured maximum number of concurrent Metal command buffers.
    pub limit: u32,
    /// Total number of command buffers launched while statistics were enabled.
    pub dispatch_count: u32,
    /// Maximum simultaneous command buffers observed.
    pub max_in_flight: u32,
    /// Milliseconds spent with at least one command buffer in flight.
    pub busy_ms: f64,
    /// Milliseconds spent with more than one command buffer in flight.
    pub overlap_ms: f64,
    /// Wall-clock milliseconds spanned by the measurement window.
    pub window_ms: f64,
    /// Per-queue occupancy metrics, indexed by queue slot.
    pub queues: Vec<QueueLaneStats>,
}
/// Per-queue occupancy data captured alongside [`QueueDepthStats`].
#[derive(Clone, Copy, Debug, Default)]
pub struct QueueLaneStats {
    /// Index of the Metal command queue.
    pub index: u32,
    /// Total number of command buffers launched on this queue.
    pub dispatch_count: u32,
    /// Maximum simultaneous command buffers observed on this queue.
    pub max_in_flight: u32,
    /// Milliseconds spent with at least one command buffer in flight on this queue.
    pub busy_ms: f64,
    /// Milliseconds spent with two or more command buffers in flight on this queue.
    pub overlap_ms: f64,
}
/// Staging buckets emitted by [`ColumnStagingStats`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnStagingPhase {
    /// FFT/IFFT staging activity.
    Fft,
    /// LDE staging activity.
    Lde,
    /// Poseidon staging activity (permute/hash).
    Poseidon,
}
/// Host-side staging telemetry recorded while batches are prepared.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnStagingPhaseStats {
    /// Number of staging events captured for this phase.
    pub batches: u32,
    /// Total milliseconds spent flattening host columns into staging buffers.
    pub flatten_ms: f64,
    /// Milliseconds spent waiting for GPU completions before a staging buffer could be reused.
    pub wait_ms: f64,
}
impl ColumnStagingPhaseStats {
    fn record_flatten(&mut self, duration_ms: f64) {
        self.batches = self.batches.saturating_add(1);
        self.flatten_ms += duration_ms;
    }
    fn record_wait(&mut self, duration_ms: f64) {
        self.wait_ms += duration_ms;
    }
}
/// Per-batch sample describing how long the host spent waiting vs flattening.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnStagingSample {
    /// Zero-based batch index within the phase.
    pub batch: u32,
    /// Milliseconds spent flattening the host slice into a GPU staging buffer.
    pub flatten_ms: f64,
    /// Milliseconds spent waiting for the GPU before staging could continue.
    pub wait_ms: f64,
}
impl ColumnStagingSample {
    /// Ratio of time spent waiting relative to the total host staging time for this batch.
    pub fn wait_ratio(&self) -> f64 {
        let total = self.flatten_ms + self.wait_ms;
        if total <= f64::EPSILON {
            0.0
        } else {
            self.wait_ms / total
        }
    }
}
/// Host-side staging telemetry recorded while FFT/LDE/Poseidon batches are prepared.
#[derive(Clone, Debug, Default)]
pub struct ColumnStagingStats {
    total: ColumnStagingPhaseStats,
    fft: ColumnStagingPhaseStats,
    lde: ColumnStagingPhaseStats,
    poseidon: ColumnStagingPhaseStats,
    fft_samples: Vec<ColumnStagingSample>,
    lde_samples: Vec<ColumnStagingSample>,
    poseidon_samples: Vec<ColumnStagingSample>,
    pending_wait: ColumnStagingPending,
}
impl ColumnStagingStats {
    /// Aggregate telemetry across all phases.
    pub fn total(&self) -> ColumnStagingPhaseStats {
        self.total
    }
    /// FFT/IFFT telemetry.
    pub fn fft(&self) -> ColumnStagingPhaseStats {
        self.fft
    }
    /// LDE telemetry.
    pub fn lde(&self) -> ColumnStagingPhaseStats {
        self.lde
    }
    /// Poseidon telemetry.
    pub fn poseidon(&self) -> ColumnStagingPhaseStats {
        self.poseidon
    }
    /// Returns the recorded FFT batch samples.
    pub fn fft_samples(&self) -> &[ColumnStagingSample] {
        &self.fft_samples
    }
    /// Returns the recorded LDE batch samples.
    pub fn lde_samples(&self) -> &[ColumnStagingSample] {
        &self.lde_samples
    }
    /// Returns the recorded Poseidon batch samples.
    pub fn poseidon_samples(&self) -> &[ColumnStagingSample] {
        &self.poseidon_samples
    }
    fn phase_mut(&mut self, phase: ColumnStagingPhase) -> &mut ColumnStagingPhaseStats {
        match phase {
            ColumnStagingPhase::Fft => &mut self.fft,
            ColumnStagingPhase::Lde => &mut self.lde,
            ColumnStagingPhase::Poseidon => &mut self.poseidon,
        }
    }
    fn samples_mut(&mut self, phase: ColumnStagingPhase) -> &mut Vec<ColumnStagingSample> {
        match phase {
            ColumnStagingPhase::Fft => &mut self.fft_samples,
            ColumnStagingPhase::Lde => &mut self.lde_samples,
            ColumnStagingPhase::Poseidon => &mut self.poseidon_samples,
        }
    }
    fn record_flatten_sample(&mut self, phase: ColumnStagingPhase, flatten_ms: f64) {
        self.total.record_flatten(flatten_ms);
        let wait_ms = self.pending_wait.take(phase);
        let phase_stats = self.phase_mut(phase);
        phase_stats.record_flatten(flatten_ms);
        let batch = phase_stats.batches.saturating_sub(1);
        push_bounded_telemetry_sample(
            self.samples_mut(phase),
            ColumnStagingSample {
                batch,
                flatten_ms,
                wait_ms,
            },
        );
    }
    fn record_wait_sample(&mut self, phase: ColumnStagingPhase, wait_ms: f64) {
        self.total.record_wait(wait_ms);
        self.phase_mut(phase).record_wait(wait_ms);
        self.pending_wait.add(phase, wait_ms);
    }
}
#[derive(Clone, Debug, Default)]
struct ColumnStagingPending {
    fft: f64,
    lde: f64,
    poseidon: f64,
}
impl ColumnStagingPending {
    fn phase_mut(&mut self, phase: ColumnStagingPhase) -> &mut f64 {
        match phase {
            ColumnStagingPhase::Fft => &mut self.fft,
            ColumnStagingPhase::Lde => &mut self.lde,
            ColumnStagingPhase::Poseidon => &mut self.poseidon,
        }
    }
    fn add(&mut self, phase: ColumnStagingPhase, wait_ms: f64) {
        *self.phase_mut(phase) += wait_ms;
    }
    fn take(&mut self, phase: ColumnStagingPhase) -> f64 {
        let slot = self.phase_mut(phase);
        let captured = *slot;
        *slot = 0.0;
        captured
    }
}
/// Snapshot describing the command semaphore heuristics.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommandLimitSnapshot {
    /// Final limit applied to the Metal command semaphore.
    pub limit: u32,
    /// Minimum limit allowed by the queue fan-out policy.
    pub queue_floor: u32,
    /// Automatically derived limit before overrides were applied.
    pub auto_limit: u32,
    /// Source used to derive the automatic limit.
    pub source: CommandLimitSource,
    /// GPU core count reported by the system profiler (if available).
    pub gpu_cores: Option<u32>,
    /// Host CPU parallelism used as a fallback (if available).
    pub cpu_parallelism: Option<u32>,
    /// User-provided override applied via `FASTPQ_METAL_MAX_IN_FLIGHT`, if any.
    pub override_limit: Option<u32>,
}
/// Origin of the automatically derived command buffer limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandLimitSource {
    /// Resolved from the GPU core count reported by macOS.
    GpuCores,
    /// Resolved from CPU parallelism as a fallback.
    CpuParallelism,
    /// Fallback constant used when no system telemetry was available.
    Fallback,
}
impl Default for CommandLimitSource {
    fn default() -> Self {
        Self::Fallback
    }
}
impl CommandLimitSource {
    /// Returns a stable string identifier for serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GpuCores => "gpu_cores",
            Self::CpuParallelism => "cpu_parallelism",
            Self::Fallback => "fallback",
        }
    }
}
/// Snapshot describing the resolved FFT/LDE batch heuristics.
#[derive(Clone, Copy, Debug, Default)]
pub struct BatchHeuristicSnapshot {
    /// Final columns per batch that will be used for upcoming dispatches.
    pub columns: u32,
    /// Baseline heuristic before adaptive scaling.
    pub recommended: u32,
    /// Maximum columns permitted by the current domain/device limits.
    pub max_columns: u32,
    /// Target kernel duration in milliseconds.
    pub target_ms: f64,
    /// Last measured kernel duration for a full-width batch (if available).
    pub last_duration_ms: Option<f64>,
    /// Number of adaptive samples recorded so far.
    pub samples: u32,
    /// Indicates whether a fixed override disabled the adaptive scheduler.
    pub override_active: bool,
}
/// Combined snapshot of the adaptive scheduling heuristics.
#[derive(Clone, Copy, Debug, Default)]
pub struct AdaptiveScheduleSnapshot {
    /// Command semaphore limit metadata.
    pub max_in_flight: Option<CommandLimitSnapshot>,
    /// FFT column batching heuristics.
    pub fft: Option<BatchHeuristicSnapshot>,
    /// LDE column batching heuristics.
    pub lde: Option<BatchHeuristicSnapshot>,
    /// Poseidon state batching heuristics.
    pub poseidon: Option<BatchHeuristicSnapshot>,
    /// Poseidon batch multiplier derived from device hints.
    pub poseidon_batch_multiplier: Option<u32>,
    /// LDE tile depth chosen for the current run.
    pub lde_tile_stage_limit: Option<u32>,
}
#[derive(Clone, Copy)]
enum AdaptiveStateId {
    Fft,
    Lde,
    Poseidon,
}
impl AdaptiveStateId {
    fn state(self) -> &'static AdaptiveBatchState {
        let scheduler = adaptive_scheduler();
        match self {
            AdaptiveStateId::Fft => &scheduler.fft,
            AdaptiveStateId::Lde => &scheduler.lde,
            AdaptiveStateId::Poseidon => &scheduler.poseidon,
        }
    }
}
struct BatchSelection {
    columns: u32,
    max_columns: u32,
    adaptive_state: Option<AdaptiveStateId>,
}
impl BatchSelection {
    fn fixed(columns: u32) -> Self {
        Self {
            columns,
            max_columns: columns,
            adaptive_state: None,
        }
    }
    fn adaptive(columns: u32, max_columns: u32, state: AdaptiveStateId) -> Self {
        Self {
            columns,
            max_columns: max_columns.max(1),
            adaptive_state: Some(state),
        }
    }
    fn columns(&self) -> u32 {
        self.columns.max(1)
    }
    fn sample_for(&self, actual_columns: u32) -> Option<AdaptiveSample> {
        if self.adaptive_state.is_none() || actual_columns != self.columns {
            return None;
        }
        Some(AdaptiveSample {
            state: self.adaptive_state.expect("adaptive state present"),
            columns: actual_columns,
            max_columns: self.max_columns,
        })
    }
}
#[derive(Clone, Copy)]
struct AdaptiveSample {
    state: AdaptiveStateId,
    columns: u32,
    max_columns: u32,
}
impl AdaptiveSample {
    fn record(&self, duration: Duration) {
        let duration_ms = elapsed_ms(duration);
        self.state
            .state()
            .record_sample(self.columns, self.max_columns, duration_ms);
    }
}
struct AdaptiveScheduler {
    fft: AdaptiveBatchState,
    lde: AdaptiveBatchState,
    poseidon: AdaptiveBatchState,
}
impl AdaptiveScheduler {
    fn new() -> Self {
        Self {
            fft: AdaptiveBatchState::new(MIN_FFT_COLUMNS_PER_BATCH, ADAPTIVE_TARGET_MS),
            lde: AdaptiveBatchState::new(MIN_LDE_COLUMNS_PER_BATCH, ADAPTIVE_TARGET_MS),
            poseidon: AdaptiveBatchState::new(MIN_POSEIDON_STATES_PER_BATCH, ADAPTIVE_TARGET_MS),
        }
    }
    fn select_fft(&self, recommended: u32, max_columns: u32) -> BatchSelection {
        self.fft
            .select(recommended, max_columns, AdaptiveStateId::Fft)
    }
    fn select_lde(&self, recommended: u32, max_columns: u32) -> BatchSelection {
        self.lde
            .select(recommended, max_columns, AdaptiveStateId::Lde)
    }
    fn select_poseidon(&self, recommended: u32, max_states: u32) -> BatchSelection {
        self.poseidon
            .select(recommended, max_states, AdaptiveStateId::Poseidon)
    }
    fn snapshot(
        &self,
    ) -> (
        Option<BatchHeuristicSnapshot>,
        Option<BatchHeuristicSnapshot>,
        Option<BatchHeuristicSnapshot>,
    ) {
        let fft_override = fft_batch_override();
        let lde_override = lde_batch_override();
        let fft = self.fft.snapshot(fft_override);
        let lde = self.lde.snapshot(lde_override);
        let poseidon = self.poseidon.snapshot(None);
        (fft, lde, poseidon)
    }
}
fn adaptive_scheduler() -> &'static AdaptiveScheduler {
    ADAPTIVE_SCHEDULER.get_or_init(AdaptiveScheduler::new)
}
struct AdaptiveBatchState {
    target_ms: f64,
    min_columns: u32,
    data: Mutex<AdaptiveBatchData>,
}
impl AdaptiveBatchState {
    fn new(min_columns: u32, target_ms: f64) -> Self {
        Self {
            target_ms,
            min_columns: min_columns.max(1),
            data: Mutex::new(AdaptiveBatchData::default()),
        }
    }
    fn select(&self, recommended: u32, max_columns: u32, kind: AdaptiveStateId) -> BatchSelection {
        let max_columns = max_columns.max(self.min_columns);
        let mut data = self.data.lock().expect("adaptive scheduler poisoned");
        data.recommended_columns = recommended.max(self.min_columns);
        data.max_columns = max_columns;
        let base = if data.current_columns == 0 {
            data.recommended_columns
        } else {
            data.current_columns
        };
        let resolved = base.clamp(self.min_columns, max_columns);
        data.current_columns = resolved;
        drop(data);
        BatchSelection::adaptive(resolved, max_columns, kind)
    }
    fn record_sample(&self, columns: u32, max_columns: u32, duration_ms: f64) {
        let mut data = self.data.lock().expect("adaptive scheduler poisoned");
        data.last_duration_ms = Some(duration_ms);
        data.samples = data.samples.saturating_add(1);
        let max_columns = max_columns.max(self.min_columns);
        let mut next = columns.clamp(self.min_columns, max_columns);
        if duration_ms + f64::EPSILON < self.target_ms && columns < max_columns {
            next = (columns.saturating_mul(2))
                .min(max_columns)
                .max(self.min_columns);
        } else if duration_ms > self.target_ms * ADAPTIVE_BACKOFF_RATIO
            && columns > self.min_columns
        {
            let halved = (columns + 1) / 2;
            next = halved.max(self.min_columns).min(max_columns);
        }
        data.current_columns = next;
    }
    fn snapshot(&self, override_value: Option<u32>) -> Option<BatchHeuristicSnapshot> {
        if let Some(value) = override_value {
            return Some(BatchHeuristicSnapshot {
                columns: value,
                recommended: value,
                max_columns: value,
                target_ms: self.target_ms,
                last_duration_ms: None,
                samples: 0,
                override_active: true,
            });
        }
        let data = self.data.lock().ok()?;
        if data.current_columns == 0 && data.recommended_columns == 0 {
            return None;
        }
        Some(BatchHeuristicSnapshot {
            columns: data.current_columns.max(self.min_columns),
            recommended: data
                .recommended_columns
                .max(self.min_columns)
                .min(data.max_columns.max(self.min_columns)),
            max_columns: data.max_columns.max(self.min_columns),
            target_ms: self.target_ms,
            last_duration_ms: data.last_duration_ms,
            samples: data.samples,
            override_active: false,
        })
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct AdaptiveBatchData {
    current_columns: u32,
    recommended_columns: u32,
    max_columns: u32,
    last_duration_ms: Option<f64>,
    samples: u32,
}
impl QueueDepthStats {
    /// Returns the delta between two queue depth snapshots, saturating at zero.
    pub fn delta_since(&self, previous: &QueueDepthStats) -> QueueDepthStats {
        let queues = self
            .queues
            .iter()
            .map(|lane| {
                let previous_lane = previous
                    .queues
                    .iter()
                    .find(|candidate| candidate.index == lane.index);
                lane.delta_since(previous_lane)
            })
            .collect();
        QueueDepthStats {
            limit: self.limit,
            dispatch_count: self.dispatch_count.saturating_sub(previous.dispatch_count),
            max_in_flight: self.max_in_flight.saturating_sub(previous.max_in_flight),
            busy_ms: saturating_sub_ms(self.busy_ms, previous.busy_ms),
            overlap_ms: saturating_sub_ms(self.overlap_ms, previous.overlap_ms),
            window_ms: saturating_sub_ms(self.window_ms, previous.window_ms),
            queues,
        }
    }
    /// Accumulates another queue depth delta into this snapshot.
    pub fn accumulate_delta(&mut self, delta: &QueueDepthStats) {
        self.limit = delta.limit;
        self.dispatch_count = self.dispatch_count.saturating_add(delta.dispatch_count);
        self.max_in_flight = self.max_in_flight.max(delta.max_in_flight);
        self.busy_ms += delta.busy_ms;
        self.overlap_ms += delta.overlap_ms;
        self.window_ms += delta.window_ms;
        for lane in &delta.queues {
            match self
                .queues
                .iter_mut()
                .find(|candidate| candidate.index == lane.index)
            {
                Some(existing) => existing.accumulate_delta(lane),
                None => self.queues.push(lane.clone()),
            }
        }
    }
}
impl QueueLaneStats {
    fn delta_since(&self, previous: Option<&QueueLaneStats>) -> QueueLaneStats {
        if let Some(prev) = previous {
            QueueLaneStats {
                index: self.index,
                dispatch_count: self.dispatch_count.saturating_sub(prev.dispatch_count),
                max_in_flight: self.max_in_flight,
                busy_ms: saturating_sub_ms(self.busy_ms, prev.busy_ms),
                overlap_ms: saturating_sub_ms(self.overlap_ms, prev.overlap_ms),
            }
        } else {
            self.clone()
        }
    }
    fn accumulate_delta(&mut self, delta: &QueueLaneStats) {
        self.dispatch_count = self.dispatch_count.saturating_add(delta.dispatch_count);
        self.max_in_flight = self.max_in_flight.max(delta.max_in_flight);
        self.busy_ms += delta.busy_ms;
        self.overlap_ms += delta.overlap_ms;
    }
}
fn saturating_sub_ms(current: f64, previous: f64) -> f64 {
    let delta = current - previous;
    if delta <= 0.0 { 0.0 } else { delta }
}
/// Kernel categories profiled by the Metal backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KernelKind {
    /// Forward FFT (column batches).
    Fft,
    /// Inverse FFT (column batches).
    Ifft,
    /// Low-degree extension columns.
    Lde,
    /// Poseidon permutation batches.
    Poseidon,
}
impl KernelKind {
    /// Returns a stable string identifier for this kernel category.
    pub fn as_str(&self) -> &'static str {
        match self {
            KernelKind::Fft => "fft",
            KernelKind::Ifft => "ifft",
            KernelKind::Lde => "lde",
            KernelKind::Poseidon => "poseidon",
        }
    }
}
/// Sample describing a post-tiling dispatch.
#[derive(Clone, Copy, Debug)]
pub struct PostTileSample {
    /// Kernel category associated with the dispatch.
    pub kind: KernelKind,
    /// Log₂ domain length for the dispatch.
    pub log_len: u32,
    /// Stage index where the post-tiling phase begins.
    pub stage_start: u32,
    /// Number of columns processed in the batch.
    pub columns: u32,
}
/// Descriptor capturing the characteristics of an exported Metal kernel.
#[derive(Clone, Copy, Debug)]
pub struct MetalKernelDescriptor {
    /// Public entry point compiled into `fastpq.metallib`.
    pub entry_point: &'static str,
    /// Logical operation executed by the kernel.
    pub kind: KernelKind,
    /// Maximum threads per threadgroup supported by the implementation (if applicable).
    pub threadgroup_cap: Option<u32>,
    /// Maximum number of FFT stages executed inside the shared-memory tile.
    pub tile_stage_cap: Option<u32>,
    /// Free-form description covering coset handling, inputs, and determinism notes.
    pub notes: &'static str,
}
const METAL_KERNEL_DESCRIPTORS: &[MetalKernelDescriptor] = &[
    MetalKernelDescriptor {
        entry_point: "fastpq_fft_columns",
        kind: KernelKind::Fft,
        threadgroup_cap: Some(FFT_THREADGROUP_CAPACITY),
        tile_stage_cap: Some(FFT_TILE_STAGE_LIMIT),
        notes: "Forward FFT over trace columns. Uses shared-memory tiles up to FFT_TILE_STAGE_LIMIT \
                stages with coset=1 and applies inverse scaling when requested.",
    },
    MetalKernelDescriptor {
        entry_point: "fastpq_fft_post_tiling",
        kind: KernelKind::Fft,
        threadgroup_cap: Some(FFT_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "Completes FFT/IFFT/LDE passes once the shared-memory tile limit is reached. \
                Runs butterflies entirely out of device memory and applies inverse scaling.",
    },
    MetalKernelDescriptor {
        entry_point: "fastpq_lde_columns",
        kind: KernelKind::Lde,
        threadgroup_cap: Some(FFT_THREADGROUP_CAPACITY),
        tile_stage_cap: Some(FFT_TILE_STAGE_LIMIT),
        notes: "Performs low-degree extension in place: copies coefficients into the evaluation \
                buffer with coefficient-wise coset scaling, executes tiled FFT stages over the \
                base domain, and leaves the final stages to the post-tiling kernel when necessary.",
    },
    MetalKernelDescriptor {
        entry_point: "poseidon_permute",
        kind: KernelKind::Poseidon,
        threadgroup_cap: Some(POSEIDON_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "Dense-MDS Poseidon permutation over STATE_WIDTH=3 words. Threadgroups cache the \
                round constants/MDS matrix in threadgroup memory, while production Goldilocks \
                dispatches assign one independent state per lane and launch only the threads \
                required by the actual state count.",
    },
    MetalKernelDescriptor {
        entry_point: "poseidon_hash_columns",
        kind: KernelKind::Poseidon,
        threadgroup_cap: Some(POSEIDON_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "Consumes flattened Poseidon column payloads (domain seed + padded rate blocks), \
                absorbs them entirely on-device, and writes the resulting STATE_WIDTH=3 states \
                back to device memory so hosts can read column hashes without rerunning the \
                absorb loop on the CPU.",
    },
    MetalKernelDescriptor {
        entry_point: "poseidon_hash_rows",
        kind: KernelKind::Poseidon,
        threadgroup_cap: Some(POSEIDON_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "Consumes column-major trace values and hashes independent row messages \
                (row index, column count, row values) in one batched dispatch window. \
                Output row digests are written in row order with scalar Poseidon padding.",
    },
    MetalKernelDescriptor {
        entry_point: "poseidon_trace_fused",
        kind: KernelKind::Poseidon,
        threadgroup_cap: Some(POSEIDON_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "Consumes flattened Poseidon column payloads and writes the trace commitment leaf \
                hashes into a combined leaf/parent output buffer. A follow-up parent kernel hashes \
                the depth-1 Merkle layer after all leaves are globally visible.",
    },
    MetalKernelDescriptor {
        entry_point: "poseidon_trace_parents",
        kind: KernelKind::Poseidon,
        threadgroup_cap: Some(POSEIDON_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "Hashes adjacent trace commitment leaves into the fused depth-1 Merkle parent \
                layer. Odd leaf counts duplicate the final leaf exactly like the CPU builder.",
    },
    MetalKernelDescriptor {
        entry_point: "bn254_fft_columns",
        kind: KernelKind::Fft,
        threadgroup_cap: None,
        tile_stage_cap: None,
        notes: "Cooperative single-threadgroup BN254 FFT over one canonical-limb column. Packed \
                stage twiddles use n-1 field elements and all Montgomery arithmetic remains \
                deterministic across Metal devices.",
    },
    MetalKernelDescriptor {
        entry_point: "bn254_lde_columns",
        kind: KernelKind::Lde,
        threadgroup_cap: None,
        tile_stage_cap: None,
        notes: "Cooperative single-threadgroup BN254 coset LDE over one canonical-limb column. \
                The host launches one command per column through a bounded completion window.",
    },
    MetalKernelDescriptor {
        entry_point: "bn254_poseidon_hash_words",
        kind: KernelKind::Poseidon,
        threadgroup_cap: Some(BN254_POSEIDON_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "Hashes flattened BN254 Poseidon word batches for FASTPQ transcript digests. \
                Inputs, parameters, and outputs are staged as raw canonical limb buffers, \
                converted to Montgomery form inside the kernel, and returned as canonical \
                BN254 field bytes.",
    },
];
/// Returns metadata describing every exported Metal kernel.
pub fn metal_kernel_descriptors() -> &'static [MetalKernelDescriptor] {
    METAL_KERNEL_DESCRIPTORS
}
/// Snapshot of a single Metal kernel dispatch.
#[derive(Clone, Copy, Debug)]
pub struct KernelStatsSample {
    /// Kernel category.
    pub kind: KernelKind,
    /// Bytes processed (host estimate, includes read + write paths).
    pub bytes: u64,
    /// Field elements touched by the dispatch.
    pub elements: u64,
    /// Columns in this batch (if applicable).
    pub column_count: u32,
    /// Logical threads launched for the dispatch.
    pub logical_threads: u64,
    /// Threads per threadgroup.
    pub threadgroup_width: u64,
    /// Number of threadgroups issued.
    pub threadgroups: u64,
    /// Pipeline execution width reported by Metal.
    pub execution_width: u32,
    /// Maximum threads per threadgroup supported by the pipeline.
    pub max_threads_per_group: u32,
    /// Kernel duration in milliseconds.
    pub duration_ms: f64,
}
#[derive(Clone, Copy, Debug)]
struct KernelProfileParams {
    kind: KernelKind,
    bytes: u64,
    elements: u64,
    columns: u32,
}
#[derive(Clone, Copy, Debug)]
struct KernelDispatchContext {
    profile: KernelProfileParams,
    logical_threads: u64,
    threadgroups: u64,
    threadgroup_width: u64,
    execution_width: u32,
    max_threads_per_group: u32,
}
impl KernelDispatchContext {
    fn from_pipeline(
        profile: KernelProfileParams,
        logical_threads: u64,
        threadgroups: u64,
        threadgroup_width: u64,
        pipeline: &ComputePipelineState,
    ) -> Self {
        let execution_width = pipeline.thread_execution_width().max(1);
        let max_threads = pipeline.max_total_threads_per_threadgroup().max(1);
        Self {
            profile,
            logical_threads,
            threadgroups,
            threadgroup_width,
            execution_width: execution_width.try_into().unwrap_or(u32::MAX),
            max_threads_per_group: max_threads.try_into().unwrap_or(u32::MAX),
        }
    }
    fn sample(&self, duration: Duration) -> KernelStatsSample {
        KernelStatsSample {
            kind: self.profile.kind,
            bytes: self.profile.bytes,
            elements: self.profile.elements,
            column_count: self.profile.columns,
            logical_threads: self.logical_threads,
            threadgroup_width: self.threadgroup_width,
            threadgroups: self.threadgroups,
            execution_width: self.execution_width,
            max_threads_per_group: self.max_threads_per_group,
            duration_ms: duration.as_secs_f64() * 1_000.0,
        }
    }
}
fn record_kernel_stats(context: &KernelDispatchContext, duration: Duration) {
    if !KERNEL_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store = KERNEL_STATS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = store.lock() {
        push_bounded_telemetry_sample(&mut guard, context.sample(duration));
    }
}
fn push_bounded_telemetry_sample<T>(samples: &mut Vec<T>, sample: T) {
    if samples.len() >= MAX_RETAINED_TELEMETRY_SAMPLES {
        return;
    }
    if samples.try_reserve(1).is_ok() {
        samples.push(sample);
    }
}
struct ColumnBatchTicket {
    range: Range<usize>,
    buffer: PooledBuffer,
    metal_buffer: Buffer,
    tickets: SmallVec<[DispatchTicket; 2]>,
}

struct ColumnMutationRollback {
    original: Option<Vec<Vec<u64>>>,
}

impl ColumnMutationRollback {
    fn capture(columns: &[Vec<u64>]) -> MetalResult<Self> {
        let mut original = Vec::new();
        original.try_reserve_exact(columns.len()).map_err(|_| {
            GpuError::InvalidInput("Metal rollback column list exceeds available host memory")
        })?;
        for column in columns {
            let mut snapshot = Vec::new();
            snapshot.try_reserve_exact(column.len()).map_err(|_| {
                GpuError::InvalidInput("Metal rollback column data exceeds available host memory")
            })?;
            snapshot.extend_from_slice(column);
            original.push(snapshot);
        }
        Ok(Self {
            original: Some(original),
        })
    }

    fn disarmed() -> Self {
        Self { original: None }
    }

    fn restore(&mut self, columns: &mut [Vec<u64>]) {
        let Some(original) = self.original.take() else {
            return;
        };
        debug_assert_eq!(columns.len(), original.len());
        for (column, original_column) in columns.iter_mut().zip(original) {
            *column = original_column;
        }
    }

    fn commit(&mut self) {
        self.original = None;
    }
}

fn rollback_columns_on_error<T>(
    result: MetalResult<T>,
    columns: &mut [Vec<u64>],
    rollback: &mut ColumnMutationRollback,
) -> MetalResult<T> {
    if result.is_err() {
        rollback.restore(columns);
    }
    result
}

fn try_clone_metal_words(words: &[u64], error: &'static str) -> MetalResult<Vec<u64>> {
    let mut snapshot = Vec::new();
    snapshot
        .try_reserve_exact(words.len())
        .map_err(|_| GpuError::InvalidInput(error))?;
    snapshot.extend_from_slice(words);
    Ok(snapshot)
}

fn try_zeroed_metal_words(len: usize, error: &'static str) -> MetalResult<Vec<u64>> {
    let mut words = Vec::new();
    words
        .try_reserve_exact(len)
        .map_err(|_| GpuError::InvalidInput(error))?;
    words.resize(len, 0);
    Ok(words)
}

#[cfg(test)]
std::thread_local! {
    static COLUMN_BATCH_WAIT_FAILURE: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
struct ColumnBatchWaitFailureGuard;

#[cfg(test)]
impl Drop for ColumnBatchWaitFailureGuard {
    fn drop(&mut self) {
        COLUMN_BATCH_WAIT_FAILURE.with(|remaining| remaining.set(None));
    }
}

#[cfg(test)]
fn fail_column_batch_wait_after(successful_waits: usize) -> ColumnBatchWaitFailureGuard {
    COLUMN_BATCH_WAIT_FAILURE.with(|remaining| remaining.set(Some(successful_waits)));
    ColumnBatchWaitFailureGuard
}

#[cfg(test)]
fn injected_column_batch_wait_failure() -> bool {
    COLUMN_BATCH_WAIT_FAILURE.with(|remaining| match remaining.get() {
        Some(0) => {
            remaining.set(None);
            true
        }
        Some(count) => {
            remaining.set(Some(count - 1));
            false
        }
        None => false,
    })
}

#[cfg(test)]
std::thread_local! {
    static POSEIDON_BATCH_WAIT_FAILURE: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
struct PoseidonBatchWaitFailureGuard;

#[cfg(test)]
impl Drop for PoseidonBatchWaitFailureGuard {
    fn drop(&mut self) {
        POSEIDON_BATCH_WAIT_FAILURE.with(|remaining| remaining.set(None));
    }
}

#[cfg(test)]
fn fail_poseidon_batch_wait_after(successful_waits: usize) -> PoseidonBatchWaitFailureGuard {
    POSEIDON_BATCH_WAIT_FAILURE.with(|remaining| remaining.set(Some(successful_waits)));
    PoseidonBatchWaitFailureGuard
}

#[cfg(test)]
fn injected_poseidon_batch_wait_failure() -> bool {
    POSEIDON_BATCH_WAIT_FAILURE.with(|remaining| match remaining.get() {
        Some(0) => {
            remaining.set(None);
            true
        }
        Some(count) => {
            remaining.set(Some(count - 1));
            false
        }
        None => false,
    })
}

impl ColumnBatchTicket {
    fn wait(self, columns: &mut [Vec<u64>], extent: usize, record_wait: bool) -> MetalResult<()> {
        let ColumnBatchTicket {
            range,
            buffer,
            metal_buffer: _metal,
            tickets,
        } = self;
        #[cfg(test)]
        if injected_column_batch_wait_failure() {
            return Err(GpuError::Execution {
                backend: GpuBackend::Metal,
                message: "injected column batch wait failure".to_owned(),
            });
        }
        let wait_start = Instant::now();
        wait_for_tickets(tickets)?;
        if record_wait {
            record_staging_wait(ColumnStagingPhase::Fft, wait_start.elapsed());
        }
        restore_range(columns, range, &buffer, extent);
        Ok(())
    }
}
struct PoseidonBatchTicket {
    range: Range<usize>,
    buffer: PooledBuffer,
    metal_buffer: Buffer,
    ticket: DispatchTicket,
}
impl PoseidonBatchTicket {
    fn wait(self, states: &mut [u64], record_wait: bool) -> MetalResult<()> {
        let PoseidonBatchTicket {
            range,
            buffer,
            metal_buffer: _,
            ticket,
        } = self;
        #[cfg(test)]
        if injected_poseidon_batch_wait_failure() {
            return Err(GpuError::Execution {
                backend: GpuBackend::Metal,
                message: "injected Poseidon batch wait failure".to_owned(),
            });
        }
        let wait_start = Instant::now();
        wait_for_ticket(ticket)?;
        if record_wait {
            record_staging_wait(ColumnStagingPhase::Poseidon, wait_start.elapsed());
        }
        buffer.copy_to_slice(&mut states[range]);
        Ok(())
    }
}
struct PoseidonHashTicket {
    column_offset: usize,
    payload: PooledBuffer,
    slices: Vec<PoseidonColumnSlice>,
    states: PooledBuffer,
    payload_buffer: Buffer,
    slice_buffer: Buffer,
    state_buffer: Buffer,
    ticket: DispatchTicket,
}
impl PoseidonHashTicket {
    fn wait(self, result: &mut [u64], record_wait: bool) -> MetalResult<()> {
        let PoseidonHashTicket {
            column_offset,
            payload: _,
            slices: _,
            states,
            payload_buffer: _,
            slice_buffer: _,
            state_buffer: _,
            ticket,
        } = self;
        let wait_start = Instant::now();
        wait_for_ticket(ticket)?;
        if record_wait {
            record_staging_wait(ColumnStagingPhase::Poseidon, wait_start.elapsed());
        }
        for index in 0..states.len() / STATE_WIDTH {
            result[column_offset + index] = states.word(index * STATE_WIDTH);
        }
        Ok(())
    }
}
/// Pending Metal column kernel dispatch.
///
/// The guard keeps the shared buffers and command tickets alive until
/// [`wait`](Self::wait) is invoked, allowing callers to defer completion while
/// overlapping CPU work.
pub(crate) struct PendingColumns<'a> {
    columns: &'a mut [Vec<u64>],
    extent: usize,
    pending_batches: Vec<ColumnBatchTicket>,
    _twiddle_buffer: Option<Buffer>,
    rollback: ColumnMutationRollback,
    completed: bool,
}
impl<'a> PendingColumns<'a> {
    fn new(
        columns: &'a mut [Vec<u64>],
        extent: usize,
        twiddle_buffer: Buffer,
        pending_batches: Vec<ColumnBatchTicket>,
        rollback: ColumnMutationRollback,
    ) -> Self {
        Self {
            columns,
            extent,
            pending_batches,
            _twiddle_buffer: Some(twiddle_buffer),
            rollback,
            completed: false,
        }
    }
    fn identity(columns: &'a mut [Vec<u64>], extent: usize) -> Self {
        Self {
            columns,
            extent,
            pending_batches: Vec::new(),
            _twiddle_buffer: None,
            rollback: ColumnMutationRollback::disarmed(),
            completed: true,
        }
    }
    /// Wait for the GPU kernel to finish and restore the column slices.
    pub(crate) fn wait(mut self) -> MetalResult<()> {
        self.finish()?;
        self.completed = true;
        Ok(())
    }
    fn finish(&mut self) -> MetalResult<()> {
        if self.completed {
            return Ok(());
        }
        let batches = mem::take(&mut self.pending_batches);
        for batch in batches {
            if let Err(error) = batch.wait(self.columns, self.extent, false) {
                self.rollback.restore(self.columns);
                self.completed = true;
                return Err(error);
            }
        }
        self.rollback.commit();
        self.completed = true;
        Ok(())
    }
}
impl Drop for PendingColumns<'_> {
    fn drop(&mut self) {
        if self.completed || self.pending_batches.is_empty() {
            return;
        }
        if let Err(error) = self.finish() {
            warn!(
                target: "fastpq::metal",
                %error,
                "pending column dispatch dropped without awaiting completion"
            );
        }
    }
}
/// Pending Metal LDE kernel dispatch.
pub(crate) struct PendingLde {
    column_count: usize,
    eval_len: usize,
    limbs_per_elem: usize,
    _coeff_buffer: PooledBuffer,
    eval_buffer: PooledBuffer,
    _coeff_metal: Buffer,
    _eval_metal: Buffer,
    _stage_twiddle_buffer: Buffer,
    tickets: Vec<DispatchTicket>,
    host_stats: Option<LdeHostStats>,
    completed: bool,
}
impl PendingLde {
    fn new(
        column_count: usize,
        eval_len: usize,
        limbs_per_elem: usize,
        coeff_buffer: PooledBuffer,
        eval_buffer: PooledBuffer,
        coeff_metal: Buffer,
        eval_metal: Buffer,
        stage_twiddle_buffer: Buffer,
        tickets: Vec<DispatchTicket>,
        host_stats: Option<LdeHostStats>,
    ) -> Self {
        Self {
            column_count,
            eval_len,
            limbs_per_elem,
            _coeff_buffer: coeff_buffer,
            eval_buffer,
            _coeff_metal: coeff_metal,
            _eval_metal: eval_metal,
            _stage_twiddle_buffer: stage_twiddle_buffer,
            tickets,
            host_stats,
            completed: false,
        }
    }
    /// Wait for the Metal LDE to finish and collect the evaluated columns.
    pub(crate) fn wait(mut self) -> MetalResult<Option<Vec<Vec<u64>>>> {
        if self.completed {
            return Ok(None);
        }
        self.complete_dispatch()?;
        let mut result = Vec::new();
        result.try_reserve_exact(self.column_count).map_err(|_| {
            GpuError::InvalidInput("Metal LDE result list exceeds available host memory")
        })?;
        let chunk_len =
            self.eval_len
                .checked_mul(self.limbs_per_elem)
                .ok_or(GpuError::InvalidInput(
                    "Metal LDE output chunk length exceeds platform limits",
                ))?;
        for column in 0..self.column_count {
            let mut chunk = Vec::new();
            chunk.try_reserve_exact(chunk_len).map_err(|_| {
                GpuError::InvalidInput("Metal LDE result column exceeds available host memory")
            })?;
            chunk.resize(chunk_len, 0);
            let offset = column.checked_mul(chunk_len).ok_or(GpuError::InvalidInput(
                "Metal LDE output offset exceeds platform limits",
            ))?;
            self.eval_buffer.copy_range_to_slice(offset, &mut chunk);
            result.push(chunk);
        }
        Ok(Some(result))
    }
    fn complete_dispatch(&mut self) -> MetalResult<()> {
        if self.completed {
            return Ok(());
        }
        let tickets = mem::take(&mut self.tickets);
        let wait_result = wait_for_tickets(tickets);
        self.completed = true;
        wait_result?;
        if let Some(stats) = self.host_stats.take() {
            record_lde_stats(stats);
        }
        Ok(())
    }
}
impl Drop for PendingLde {
    fn drop(&mut self) {
        if self.completed || self.tickets.is_empty() {
            return;
        }
        if let Err(error) = self.complete_dispatch() {
            warn!(
                target: "fastpq::metal",
                %error,
                "pending LDE dispatch dropped without awaiting completion"
            );
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
struct FftArgs {
    column_len: u64,
    log_len: u32,
    column_count: u32,
    inverse: u32,
    local_stage_limit: u32,
    threadgroup_lanes: u32,
    column_offset: u32,
    _padding: u32,
    _padding2: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct LdeArgs {
    trace_len: u64,
    eval_len: u64,
    trace_log: u32,
    blowup_log: u32,
    column_count: u32,
    column_offset: u32,
    threadgroup_lanes: u32,
    local_stage_limit: u32,
    coset: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct PostTileArgs {
    column_len: u64,
    log_len: u32,
    column_count: u32,
    column_offset: u32,
    stage_start: u32,
    inverse: u32,
    threadgroup_lanes: u32,
    coset: u64,
}
const _: [(); 40] = [(); mem::size_of::<FftArgs>()];
const _: [(); 48] = [(); mem::size_of::<LdeArgs>()];
const _: [(); 40] = [(); mem::size_of::<PostTileArgs>()];
#[repr(C)]
#[derive(Clone, Copy)]
struct PoseidonArgs {
    state_count: u32,
    states_per_lane: u32,
    block_count: u32,
    _reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct PoseidonFusedArgs {
    state_count: u32,
    states_per_lane: u32,
    block_count: u32,
    leaf_offset: u32,
    parent_offset: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct PoseidonRowArgs {
    row_count: u32,
    column_count: u32,
    row_offset: u32,
    batch_count: u32,
    states_per_lane: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Bn254PoseidonArgs {
    batch_count: u32,
    states_per_lane: u32,
    round_count: u32,
    _reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Bn254PoseidonMetalSlice {
    offset: u32,
    len: u32,
}
#[derive(Clone, Copy, Debug)]
struct PoseidonRowDispatchEvidence {
    batch_count: u32,
    batch_rows: u32,
    row_count: u32,
    column_count: u32,
    logical_threads: u64,
    threadgroups: u64,
    threadgroup_width: u64,
    states_per_lane: u32,
    queue_index: usize,
    byte_estimate: u64,
}
impl PoseidonRowDispatchEvidence {
    fn contextualize_error(self, error: GpuError) -> GpuError {
        warn!(
            target: "fastpq::metal",
            batch_count = self.batch_count,
            batch_rows = self.batch_rows,
            row_count = self.row_count,
            column_count = self.column_count,
            logical_threads = self.logical_threads,
            threadgroups = self.threadgroups,
            threadgroup_width = self.threadgroup_width,
            states_per_lane = self.states_per_lane,
            queue_index = self.queue_index,
            byte_estimate = self.byte_estimate,
            %error,
            "Poseidon row-hash Metal runtime dispatch failed"
        );
        match error {
            GpuError::Execution { backend, message } => GpuError::Execution {
                backend,
                message: format!(
                    "{message}; poseidon rows batch_count={} batch_rows={} row_count={} column_count={} \
                     logical_threads={} threadgroups={} threadgroup_width={} states_per_lane={} \
                     queue_index={} byte_estimate={}",
                    self.batch_count,
                    self.batch_rows,
                    self.row_count,
                    self.column_count,
                    self.logical_threads,
                    self.threadgroups,
                    self.threadgroup_width,
                    self.states_per_lane,
                    self.queue_index,
                    self.byte_estimate,
                ),
            },
            other => other,
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Bn254PoseidonDispatchEvidence {
    batch_count: u32,
    word_count: usize,
    logical_threads: u64,
    threadgroups: u64,
    threadgroup_width: u64,
    states_per_lane: u32,
    queue_index: usize,
    byte_estimate: u64,
}
impl Bn254PoseidonDispatchEvidence {
    fn contextualize_error(self, error: GpuError) -> GpuError {
        match error {
            GpuError::Execution { backend, message } => GpuError::Execution {
                backend,
                message: format!(
                    "{message}; BN254 Poseidon dispatch context: batch_count={}, word_count={}, \
                     logical_threads={}, threadgroups={}, threadgroup_width={}, \
                     states_per_lane={}, queue_index={}, byte_estimate={}",
                    self.batch_count,
                    self.word_count,
                    self.logical_threads,
                    self.threadgroups,
                    self.threadgroup_width,
                    self.states_per_lane,
                    self.queue_index,
                    self.byte_estimate
                ),
            },
            other => other,
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct QueuePolicy {
    fanout: usize,
    column_threshold: u32,
}
impl QueuePolicy {
    fn new(fanout: usize, column_threshold: u32) -> Self {
        let validated_fanout = fanout.clamp(1, MAX_QUEUE_FANOUT);
        let threshold = column_threshold.max(MIN_QUEUE_COLUMN_THRESHOLD);
        Self {
            fanout: validated_fanout,
            column_threshold: threshold,
        }
    }
    fn fanout(&self) -> usize {
        self.fanout
    }
    fn column_threshold(&self) -> u32 {
        self.column_threshold
    }
    fn should_fan_out(&self, total_columns: u32) -> bool {
        self.fanout > 1 && total_columns >= self.column_threshold
    }
    fn select_index(&self, total_columns: u32, batch_index: usize) -> usize {
        if !self.should_fan_out(total_columns) {
            return 0;
        }
        batch_index % self.fanout
    }
}
fn queue_total_columns_hint(column_count: u32, inverse: bool, policy: &QueuePolicy) -> u32 {
    if inverse && policy.should_fan_out(column_count) && column_count <= policy.column_threshold() {
        policy.column_threshold().saturating_sub(1)
    } else {
        column_count
    }
}
fn metal_nil_error(operation: &'static str) -> GpuError {
    GpuError::Execution {
        backend: GpuBackend::Metal,
        message: format!("Metal returned nil from {operation}"),
    }
}

#[allow(unsafe_code)]
fn try_new_command_queue(device: &DeviceRef) -> MetalResult<CommandQueue> {
    // SAFETY: `newCommandQueue` returns an owned Objective-C object. The SDK permits nil under
    // resource pressure, so check the raw pointer before transferring ownership to metal-rs.
    let raw: *mut Object = unsafe { msg_send![device, newCommandQueue] };
    if raw.is_null() {
        return Err(metal_nil_error("-[MTLDevice newCommandQueue]"));
    }
    // SAFETY: the non-null `new...` result carries +1 ownership.
    Ok(unsafe { CommandQueue::from_ptr(raw.cast()) })
}
struct QueuePool {
    queues: Vec<CommandQueue>,
    policy: QueuePolicy,
}
impl QueuePool {
    fn new(device: &Device, policy: QueuePolicy) -> MetalResult<Self> {
        let mut queues = Vec::new();
        queues.try_reserve_exact(policy.fanout()).map_err(|_| {
            GpuError::InvalidInput("Metal command queue list exceeds available host memory")
        })?;
        for _ in 0..policy.fanout() {
            queues.push(try_new_command_queue(device)?);
        }
        Ok(Self { queues, policy })
    }
    fn select(&self, total_columns: u32, batch_index: usize) -> (&CommandQueue, usize) {
        let index = self.policy.select_index(total_columns, batch_index);
        (&self.queues[index], index)
    }
    #[allow(dead_code)]
    fn primary(&self) -> (&CommandQueue, usize) {
        (&self.queues[0], 0)
    }
    fn policy(&self) -> &QueuePolicy {
        &self.policy
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct TwiddleCacheKey {
    log_len: u32,
    root: u64,
    inverse: bool,
}
struct TwiddleCacheEntry {
    buffer: Buffer,
    build_cost_ms: f64,
}
struct TwiddleCache {
    buffers: HashMap<TwiddleCacheKey, TwiddleCacheEntry>,
}
impl TwiddleCache {
    fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }
    fn resolve(
        &mut self,
        device: &Device,
        log_len: u32,
        root: u64,
        inverse: bool,
    ) -> MetalResult<Buffer> {
        if log_len == 0 {
            return Err(GpuError::InvalidInput(
                "Metal twiddle buffers require a non-zero domain log",
            ));
        }
        let _ = goldilocks_domain_len(log_len)?;
        let key = TwiddleCacheKey {
            log_len,
            root,
            inverse,
        };
        if let Some(entry) = self.buffers.get(&key) {
            record_twiddle_cache_sample(entry.build_cost_ms, true);
            return Ok(entry.buffer.clone());
        }
        let started = Instant::now();
        let stage_twiddles = compute_stage_twiddles(log_len, root, inverse);
        let byte_len =
            u64::try_from(mem::size_of_val(stage_twiddles.as_slice())).map_err(|_| {
                GpuError::InvalidInput("Metal twiddle buffer length exceeds 64-bit representation")
            })?;
        validate_metal_buffer_byte_len(device, byte_len)?;
        let buffer = try_new_buffer_with_data(
            device,
            stage_twiddles.as_ptr().cast::<c_void>(),
            byte_len,
            MTLResourceOptions::StorageModeShared,
        )?;
        let build_cost_ms = elapsed_ms(started.elapsed());
        if self.buffers.len() >= GOLDILOCKS_TWIDDLE_CACHE_MAX_ENTRIES {
            self.buffers.clear();
        }
        self.buffers.insert(
            key,
            TwiddleCacheEntry {
                buffer: buffer.clone(),
                build_cost_ms,
            },
        );
        record_twiddle_cache_sample(build_cost_ms, false);
        Ok(buffer)
    }
}
struct Bn254TwiddleCache {
    buffers: HashMap<u32, (Buffer, u64)>,
    bytes: u64,
}
impl Bn254TwiddleCache {
    fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            bytes: 0,
        }
    }

    fn resolve(&mut self, device: &Device, log_size: u32) -> MetalResult<Buffer> {
        if let Some((buffer, _)) = self.buffers.get(&log_size) {
            return Ok(buffer.clone());
        }
        let byte_len = bn254::staged_twiddle_byte_len(log_size).map_err(GpuError::InvalidInput)?;
        let buffer = stage_bn254_twiddles(device, log_size)?;
        if byte_len <= BN254_TWIDDLE_CACHE_MAX_BYTES {
            if self.bytes.saturating_add(byte_len) > BN254_TWIDDLE_CACHE_MAX_BYTES {
                self.buffers.clear();
                self.bytes = 0;
            }
            self.buffers.insert(log_size, (buffer.clone(), byte_len));
            self.bytes = self.bytes.saturating_add(byte_len);
        }
        Ok(buffer)
    }
}
struct MetalPipelines {
    device: Device,
    queues: QueuePool,
    poseidon_permute: ComputePipelineState,
    poseidon_hash: ComputePipelineState,
    poseidon_hash_rows: ComputePipelineState,
    poseidon_trace_fused: ComputePipelineState,
    #[allow(dead_code)]
    poseidon_trace_parents: ComputePipelineState,
    fft: ComputePipelineState,
    lde: ComputePipelineState,
    post_tile: ComputePipelineState,
    bn254_fft: ComputePipelineState,
    bn254_lde: ComputePipelineState,
    bn254_poseidon_hash: ComputePipelineState,
    twiddle_cache: Mutex<TwiddleCache>,
    bn254_twiddles: Mutex<Bn254TwiddleCache>,
}
struct Bn254PoseidonMetalPipelines {
    device: Device,
    queues: QueuePool,
    bn254_poseidon_hash: ComputePipelineState,
}
fn metal_context() -> MetalResult<&'static MetalPipelines> {
    match METAL_CONTEXT.get_or_init(build_metal_context) {
        Ok(context) => Ok(context),
        Err(err) => Err(err.clone()),
    }
}
fn bn254_poseidon_context() -> MetalResult<&'static Bn254PoseidonMetalPipelines> {
    match BN254_POSEIDON_CONTEXT.get_or_init(build_bn254_poseidon_context) {
        Ok(context) => Ok(context),
        Err(err) => Err(err.clone()),
    }
}
impl MetalPipelines {
    fn stage_twiddle_buffer(&self, log_len: u32, root: u64, inverse: bool) -> MetalResult<Buffer> {
        let mut cache = self
            .twiddle_cache
            .lock()
            .expect("Metal twiddle cache poisoned");
        cache.resolve(&self.device, log_len, root, inverse)
    }
    fn bn254_fft_twiddle_buffer(&self, log_size: u32) -> MetalResult<Buffer> {
        self.bn254_twiddles
            .lock()
            .expect("BN254 twiddle cache poisoned")
            .resolve(&self.device, log_size)
    }
    fn bn254_lde_twiddle_buffer(&self, trace_log: u32, blowup_log: u32) -> MetalResult<Buffer> {
        let eval_log = trace_log
            .checked_add(blowup_log)
            .ok_or(GpuError::InvalidInput(
                "BN254 LDE log size exceeds 32-bit representation",
            ))?;
        self.bn254_fft_twiddle_buffer(eval_log)
    }
}
fn pipeline_limits(pipeline: &ComputePipelineState) -> PipelineLimits {
    let exec_width = u32::try_from(pipeline.thread_execution_width()).unwrap_or(u32::MAX);
    let max_threads =
        u32::try_from(pipeline.max_total_threads_per_threadgroup()).unwrap_or(u32::MAX);
    PipelineLimits {
        exec_width: exec_width.max(1),
        max_threads: max_threads.max(1),
    }
}
/// Upload a flattened BN254 twiddle buffer (canonical limbs) for Metal FFT/LDE kernels.
///
/// Layout: packed stage-major, with `2^stage` entries at offset `2^stage - 1`.
/// Each twiddle must be four `u64` limbs in BN254
/// canonical form; the shader converts each value into Montgomery form.
#[cfg(test)]
pub(crate) fn upload_bn254_twiddles(device: &Device, twiddles: &[u64]) -> MetalResult<Buffer> {
    if twiddles.is_empty() || twiddles.len() % 4 != 0 {
        return Err(GpuError::InvalidInput(
            "BN254 twiddle buffer must contain a non-zero multiple of 4 limbs",
        ));
    }
    let byte_len = u64::try_from(mem::size_of_val(twiddles)).map_err(|_| {
        GpuError::InvalidInput("BN254 twiddle buffer length exceeds 64-bit representation")
    })?;
    validate_metal_buffer_byte_len(device, byte_len)?;
    let buffer = try_new_buffer_with_data(
        device,
        twiddles.as_ptr().cast::<c_void>(),
        byte_len,
        MTLResourceOptions::StorageModeShared,
    )?;
    Ok(buffer)
}
fn upload_bn254_twiddle_values(
    device: &Device,
    twiddles: &[[u64; BN254_LIMBS]],
) -> MetalResult<Buffer> {
    if twiddles.is_empty() {
        return Err(GpuError::InvalidInput(
            "BN254 twiddle buffer requires at least one value",
        ));
    }
    let byte_len = u64::try_from(mem::size_of_val(twiddles)).map_err(|_| {
        GpuError::InvalidInput("BN254 twiddle buffer length exceeds 64-bit representation")
    })?;
    validate_metal_buffer_byte_len(device, byte_len)?;
    try_new_buffer_with_data(
        device,
        twiddles.as_ptr().cast::<c_void>(),
        byte_len,
        MTLResourceOptions::StorageModeShared,
    )
}
/// Flatten an array of BN254 twiddles (each four limbs) into a `[u64]` buffer.
#[cfg(test)]
pub(crate) fn flatten_bn254_twiddles(twiddles: &[[u64; 4]]) -> MetalResult<Vec<u64>> {
    let limb_len = twiddles
        .len()
        .checked_mul(BN254_LIMBS)
        .ok_or(GpuError::InvalidInput(
            "BN254 flattened twiddle length exceeds platform limits",
        ))?;
    let mut flat = Vec::new();
    flat.try_reserve_exact(limb_len).map_err(|_| {
        GpuError::InvalidInput("BN254 flattened twiddles exceed available host memory")
    })?;
    for t in twiddles {
        flat.extend_from_slice(t);
    }
    Ok(flat)
}
/// Convenience: derive stage-major BN254 twiddles on CPU then upload to Metal.
pub(crate) fn stage_bn254_twiddles(device: &Device, log_size: u32) -> MetalResult<Buffer> {
    bn254::validate_staged_twiddle_resources(log_size).map_err(GpuError::InvalidInput)?;
    let byte_len = bn254::staged_twiddle_byte_len(log_size).map_err(GpuError::InvalidInput)?;
    validate_metal_buffer_byte_len(device, byte_len)?;
    let twiddles = bn254_stage_twiddles_limbs(log_size)?;
    validate_bn254_twiddles_shape(log_size, &twiddles)?;
    upload_bn254_twiddle_values(device, &twiddles)
}
/// Validate BN254 twiddle layout against the expected packed stage-major shape.
///
/// For `log_size`, the twiddle count must equal `n - 1` where `n = 1 << log_size`.
pub(crate) fn validate_bn254_twiddles_shape(
    log_size: u32,
    twiddles: &[[u64; 4]],
) -> MetalResult<()> {
    let expected = bn254::fft_twiddle_len(log_size).map_err(GpuError::InvalidInput)?;
    if twiddles.len() != expected {
        return Err(GpuError::InvalidInput(
            "BN254 twiddles shape mismatch for stage-major layout",
        ));
    }
    Ok(())
}
/// Expected twiddle count for BN254 FFT (radix-2) given `log_size`.
pub(crate) fn bn254_fft_twiddle_len(log_size: u32) -> MetalResult<usize> {
    bn254::fft_twiddle_len(log_size).map_err(GpuError::InvalidInput)
}
/// Expected twiddle count for BN254 LDE (radix-2) given trace/eval logs.
pub(crate) fn bn254_lde_twiddle_len(trace_log: u32, blowup_log: u32) -> MetalResult<usize> {
    if blowup_log == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 LDE requires a positive blowup factor",
        ));
    }
    bn254_validate_log(trace_log)?;
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE log size exceeds 32-bit representation",
        ))?;
    bn254::fft_twiddle_len(eval_log).map_err(GpuError::InvalidInput)
}
/// Upload a BN254 coset element (4 canonical limbs) for LDE kernels.
pub(crate) fn upload_bn254_coset(device: &Device, coset: &[u64]) -> MetalResult<Buffer> {
    if coset.len() != 4 {
        return Err(GpuError::InvalidInput(
            "BN254 coset must contain exactly 4 limbs",
        ));
    }
    let byte_len = u64::try_from(mem::size_of_val(coset)).map_err(|_| {
        GpuError::InvalidInput("BN254 coset buffer length exceeds 64-bit representation")
    })?;
    validate_metal_buffer_byte_len(device, byte_len)?;
    let buffer = try_new_buffer_with_data(
        device,
        coset.as_ptr().cast::<c_void>(),
        byte_len,
        MTLResourceOptions::StorageModeShared,
    )?;
    Ok(buffer)
}
fn load_pipeline(
    device: &Device,
    library: &Library,
    name: &str,
) -> MetalResult<ComputePipelineState> {
    let function = library
        .get_function(name, None)
        .map_err(|err| GpuError::Execution {
            backend: GpuBackend::Metal,
            message: format!("kernel {name} missing: {err}"),
        })?;
    device
        .new_compute_pipeline_state_with_function(&function)
        .map_err(|err| GpuError::Execution {
            backend: GpuBackend::Metal,
            message: format!("failed to create pipeline for {name}: {err}"),
        })
}
fn register_metal_device_hints(device: &Device) {
    let location = device.location();
    let is_discrete = !device.is_low_power()
        || device.is_headless()
        || matches!(
            location,
            MTLDeviceLocation::Slot | MTLDeviceLocation::External
        );
    let mut working_set = device.recommended_max_working_set_size();
    if working_set == 0 {
        const GIB: u64 = 1024 * 1024 * 1024;
        working_set = if is_discrete { 64 * GIB } else { 16 * GIB };
    }
    metal_config::register_device_hints(DeviceHints::new(
        device.is_low_power(),
        device.is_headless(),
        matches!(
            location,
            MTLDeviceLocation::Slot | MTLDeviceLocation::External
        ),
        working_set,
    ));
}
fn load_metal_library(device: &Device) -> MetalResult<Library> {
    if let Some(library_path) = resolve_metal_library_path() {
        return device
            .new_library_with_file(&library_path)
            .map_err(|err| GpuError::Execution {
                backend: GpuBackend::Metal,
                message: format!("failed to load Metal library {}: {err}", library_path),
            });
    }
    debug!(
        target: "fastpq::metal",
        "offline fastpq.metallib unavailable; compiling embedded Metal source"
    );
    compile_embedded_metal_library(device)
}
fn compile_embedded_metal_library(device: &Device) -> MetalResult<Library> {
    let options = CompileOptions::new();
    options.set_language_version(MTLLanguageVersion::V2_4);
    options.set_fast_math_enabled(false);
    device
        .new_library_with_source(&embedded_metal_library_source(), &options)
        .map_err(|err| GpuError::Execution {
            backend: GpuBackend::Metal,
            message: format!("failed to compile embedded Metal library: {err}"),
        })
}
fn embedded_metal_library_source() -> String {
    const PRELUDE: &str = "#include <metal_stdlib>\nusing namespace metal;\n";
    const PARAMS: &str = include_str!("../metal/include/params.h");
    const FIELD: &str = include_str!("../metal/kernels/field.metal");
    const NTT: &str = include_str!("../metal/kernels/ntt_stage.metal");
    const POSEIDON: &str = include_str!("../metal/kernels/poseidon2.metal");
    const BN254: &str = include_str!("../metal/kernels/bn254.metal");

    let mut source = String::with_capacity(
        PRELUDE.len() + PARAMS.len() + FIELD.len() + NTT.len() + POSEIDON.len() + BN254.len(),
    );
    source.push_str(PRELUDE);
    source.push_str(PARAMS);
    source.push('\n');
    source.push_str(FIELD);
    source.push('\n');
    append_embedded_translation_unit(&mut source, NTT);
    append_embedded_translation_unit(&mut source, POSEIDON);
    append_embedded_translation_unit(&mut source, BN254);
    source
}
fn append_embedded_translation_unit(destination: &mut String, translation_unit: &str) {
    for line in translation_unit.lines() {
        // Quoted includes are repository-local files already embedded above.
        // System includes remain in the source for the runtime compiler.
        if line.trim_start().starts_with("#include \"") {
            continue;
        }
        destination.push_str(line);
        destination.push('\n');
    }
}
fn build_bn254_poseidon_context() -> MetalResult<Bn254PoseidonMetalPipelines> {
    let Some(device) = select_metal_device() else {
        return Err(GpuError::Unsupported(GpuBackend::Metal));
    };
    register_metal_device_hints(&device);
    let library = load_metal_library(&device)?;
    let bn254_poseidon_hash = load_pipeline(&device, &library, BN254_POSEIDON_HASH_KERNEL)?;
    let queue_policy = resolve_queue_policy(&device);
    let queues = QueuePool::new(&device, queue_policy)?;
    let manifest_sha = poseidon_manifest().sha256_hex();
    debug!(
        target: "fastpq::metal",
        manifest_sha = manifest_sha,
        "loaded BN254 Poseidon word-batch Metal pipeline"
    );
    Ok(Bn254PoseidonMetalPipelines {
        device,
        queues,
        bn254_poseidon_hash,
    })
}
fn build_metal_context() -> MetalResult<MetalPipelines> {
    let Some(device) = select_metal_device() else {
        return Err(GpuError::Unsupported(GpuBackend::Metal));
    };
    register_metal_device_hints(&device);
    let library = load_metal_library(&device)?;
    let poseidon_permute = load_pipeline(&device, &library, POSEIDON_PERMUTE_KERNEL)?;
    let poseidon_hash = load_pipeline(&device, &library, POSEIDON_HASH_KERNEL)?;
    let poseidon_hash_rows = load_pipeline(&device, &library, POSEIDON_HASH_ROWS_KERNEL)?;
    let poseidon_trace_fused = load_pipeline(&device, &library, POSEIDON_TRACE_FUSED_KERNEL)?;
    let poseidon_trace_parents = load_pipeline(&device, &library, POSEIDON_TRACE_PARENTS_KERNEL)?;
    let fft = load_pipeline(&device, &library, FFT_KERNEL)?;
    let lde = load_pipeline(&device, &library, LDE_KERNEL)?;
    let post_tile = load_pipeline(&device, &library, POST_TILE_KERNEL)?;
    // BN254 kernels are loaded to ensure the metallib stays in sync with the host,
    // but remain gated behind parity checks before use.
    let bn254_fft = load_pipeline(&device, &library, BN254_FFT_KERNEL)?;
    let bn254_lde = load_pipeline(&device, &library, BN254_LDE_KERNEL)?;
    let bn254_poseidon_hash = load_pipeline(&device, &library, BN254_POSEIDON_HASH_KERNEL)?;
    let queue_policy = resolve_queue_policy(&device);
    let queues = QueuePool::new(&device, queue_policy)?;
    let manifest_sha = poseidon_manifest().sha256_hex();
    debug!(
        target: "fastpq::metal",
        manifest_sha = manifest_sha,
        "loaded Poseidon manifest for GPU parity checks"
    );
    // Pre-stage minimal BN254 twiddle buffers from the CPU domain builder to ensure
    // the GPU layout stays aligned with host fixtures before runtime dispatches.
    let mut bn254_twiddles = Bn254TwiddleCache::new();
    let fft_min_log = 1u32;
    let lde_eval_log = 2u32; // smallest valid trace/log combination is (1, 1)
    let _ = bn254_twiddles.resolve(&device, fft_min_log)?;
    let _ = bn254_twiddles.resolve(&device, lde_eval_log)?;
    Ok(MetalPipelines {
        device,
        queues,
        poseidon_permute,
        poseidon_hash,
        poseidon_hash_rows,
        poseidon_trace_fused,
        poseidon_trace_parents,
        fft,
        lde,
        post_tile,
        bn254_fft,
        bn254_lde,
        bn254_poseidon_hash,
        twiddle_cache: Mutex::new(TwiddleCache::new()),
        bn254_twiddles: Mutex::new(bn254_twiddles),
    })
}
fn resolve_metal_library_path() -> Option<String> {
    resolve_metal_library_path_candidates(
        debug_env_var("FASTPQ_METAL_LIB"),
        option_env!("FASTPQ_METAL_LIB"),
    )
}
fn resolve_metal_library_path_candidates(
    runtime_override: Option<String>,
    build_path: Option<&str>,
) -> Option<String> {
    runtime_override
        .filter(|path| !path.is_empty())
        .or_else(|| {
            build_path
                // Build-script paths live under Cargo's output directory and may
                // disappear when a binary is packaged or moved. Unlike an explicit
                // runtime override, a stale embedded path should use the source
                // fallback instead of making otherwise valid Metal hardware unusable.
                .filter(|path| !path.is_empty() && Path::new(path).is_file())
                .map(str::to_owned)
        })
}
fn select_metal_device() -> Option<Device> {
    Device::system_default().or_else(|| Device::all().into_iter().next())
}
fn resolve_queue_policy(device: &Device) -> QueuePolicy {
    let fanout_override = queue_fanout_override();
    let fanout = fanout_override.unwrap_or_else(|| default_queue_fanout(device));
    let threshold_override = queue_threshold_override();
    let auto_threshold = default_queue_column_threshold(fanout);
    let threshold = threshold_override.unwrap_or(auto_threshold);
    let policy = QueuePolicy::new(fanout, threshold);
    let device_name = device.name();
    debug!(
        target: "fastpq::metal",
        fanout = policy.fanout(),
        column_threshold = policy.column_threshold(),
        auto_threshold = auto_threshold,
        fanout_override = fanout_override,
        threshold_override = threshold_override,
        device_low_power = device.is_low_power(),
        device_removable = device.is_removable(),
        %device_name,
        "configured Metal queue fan-out policy"
    );
    policy
}
fn default_queue_fanout(device: &Device) -> usize {
    if is_discrete_gpu(device) {
        DISCRETE_QUEUE_FANOUT
    } else {
        MIN_QUEUE_FANOUT
    }
}
fn is_discrete_gpu(device: &Device) -> bool {
    !device.is_low_power()
        || device.is_headless()
        || matches!(
            device.location(),
            MTLDeviceLocation::Slot | MTLDeviceLocation::External
        )
}
fn queue_fanout_override() -> Option<usize> {
    if let Some(value) = RUNTIME_QUEUE_FANOUT_OVERRIDE.get().copied() {
        return Some(value);
    }
    #[cfg(test)]
    if let Some(value) = TEST_QUEUE_FANOUT_OVERRIDE.get().copied() {
        return Some(value);
    }
    debug_env_var(QUEUE_FANOUT_ENV).and_then(|raw| match parse_queue_fanout_override(raw.trim()) {
        Ok(value) => {
            debug!(
                target: "fastpq::metal",
                fanout = value,
                "overriding Metal queue fan-out via {QUEUE_FANOUT_ENV}"
            );
            Some(value)
        }
        Err(error) => {
            warn!(
                target: "fastpq::metal",
                raw,
                %error,
                "invalid {QUEUE_FANOUT_ENV} override; keeping heuristic fan-out"
            );
            None
        }
    })
}
fn parse_queue_fanout_override(raw: &str) -> Result<usize, &'static str> {
    let value: usize = raw.parse().map_err(|_| "not an integer")?;
    if value == 0 {
        return Err("fan-out must be greater than zero");
    }
    Ok(value)
}
fn queue_threshold_override() -> Option<u32> {
    if let Some(value) = RUNTIME_QUEUE_THRESHOLD_OVERRIDE.get().copied() {
        return Some(value);
    }
    #[cfg(test)]
    if let Some(value) = TEST_QUEUE_THRESHOLD_OVERRIDE.get().copied() {
        return Some(value);
    }
    debug_env_var(QUEUE_COLUMN_THRESHOLD_ENV).and_then(|raw| {
        match parse_queue_threshold_override(raw.trim()) {
            Ok(value) => {
                debug!(
                    target: "fastpq::metal",
                    columns = value,
                    "overriding queue column threshold via {QUEUE_COLUMN_THRESHOLD_ENV}"
                );
                Some(value)
            }
            Err(error) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    %error,
                    default_threshold = DEFAULT_QUEUE_COLUMN_THRESHOLD,
                    "invalid {QUEUE_COLUMN_THRESHOLD_ENV} override; keeping heuristic threshold"
                );
                None
            }
        }
    })
}
fn parse_queue_threshold_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if value == 0 {
        return Err("column threshold must be greater than zero");
    }
    Ok(value)
}
/// Apply runtime overrides for the Metal queue fan-out and column threshold heuristics.
///
/// When a value is provided, it must respect the same constraints enforced by the
/// environment variables:
/// - `fanout`: range `[1, MAX_QUEUE_FANOUT]`
/// - `column_threshold`: greater than zero
///
/// Overrides must be configured before the Metal context is initialised. Calling this
/// routine more than once for a given value returns an error.
pub fn set_metal_queue_policy(
    fanout: Option<usize>,
    column_threshold: Option<u32>,
) -> Result<(), &'static str> {
    if let Some(value) = fanout {
        if value < MIN_QUEUE_FANOUT || value > MAX_QUEUE_FANOUT {
            return Err("FASTPQ Metal queue fan-out must be between 1 and 4");
        }
        RUNTIME_QUEUE_FANOUT_OVERRIDE
            .set(value)
            .map_err(|_| "FASTPQ Metal queue fan-out override already configured")?;
    }
    if let Some(value) = column_threshold {
        if value == 0 {
            return Err("FASTPQ Metal queue column threshold must be greater than zero");
        }
        RUNTIME_QUEUE_THRESHOLD_OVERRIDE
            .set(value)
            .map_err(|_| "FASTPQ Metal queue column-threshold override already configured")?;
    }
    Ok(())
}
fn default_queue_column_threshold(fanout: usize) -> u32 {
    if fanout <= 1 {
        return u32::MAX;
    }
    let scaled = (fanout as u32).saturating_mul(8);
    DEFAULT_QUEUE_COLUMN_THRESHOLD.max(scaled)
}
#[allow(dead_code)] // Metal FFT entry point is unused when CUDA-only builds run tests
pub fn fft_columns(columns: &mut [Vec<u64>], log_size: u32, root: u64) -> MetalResult<()> {
    let _ = goldilocks_domain_len(log_size)?;
    if columns.is_empty() {
        return Ok(());
    }
    fft_columns_async(columns, log_size, root)?.wait()
}
/// Dispatches an FFT over the provided columns and returns a pending handle.
pub(crate) fn fft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
) -> MetalResult<PendingColumns<'a>> {
    dispatch_fft_columns(columns, log_size, root, false)
}
fn dispatch_fft_columns<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
    inverse: bool,
) -> MetalResult<PendingColumns<'a>> {
    let extent = goldilocks_domain_len(log_size)?;
    if columns.iter().any(|column| column.len() != extent) {
        return Err(GpuError::InvalidInput("columns must share length"));
    }
    if columns.is_empty() || log_size == 0 {
        return Ok(PendingColumns::identity(columns, extent));
    }
    let column_len = u64::try_from(extent)
        .map_err(|_| GpuError::InvalidInput("column length exceeds u64::MAX"))?;
    let column_count = u32::try_from(columns.len())
        .map_err(|_| GpuError::InvalidInput("column count exceeds u32::MAX"))?;
    let context = metal_context()?;
    let limits = pipeline_limits(&context.fft);
    let tuning = metal_config::fft_tuning(log_size, limits.exec_width, limits.max_threads);
    let twiddle_buffer = context.stage_twiddle_buffer(log_size, root, inverse)?;
    let base_args = FftArgs {
        column_len,
        log_len: log_size,
        column_count: 0,
        inverse: inverse as u32,
        local_stage_limit: tuning.tile_stage_limit,
        threadgroup_lanes: tuning.threadgroup_lanes,
        column_offset: 0,
        _padding: 0,
        _padding2: 0,
    };
    let post_stage_start = post_tile_stage_start(log_size, tuning.tile_stage_limit);
    let fft_selection = select_fft_batch(tuning.threadgroup_lanes);
    let batch_size = fft_selection.columns();
    let max_batch_columns = usize::try_from(column_count.min(batch_size))
        .map_err(|_| GpuError::InvalidInput("FFT batch column count exceeds platform limits"))?;
    let max_batch_words = extent
        .checked_mul(max_batch_columns)
        .ok_or(GpuError::InvalidInput(
            "FFT batch buffer length exceeds platform limits",
        ))?;
    validate_metal_pooled_word_len(&context.device, max_batch_words)?;
    let batches = column_batch_ranges(column_count, batch_size);
    let pipe_depth = COLUMN_STAGING_PIPE_DEPTH.max(1);
    let mut slots: Vec<Option<ColumnBatchTicket>> = Vec::with_capacity(pipe_depth);
    slots.resize_with(pipe_depth, || None);
    let queue_total_columns =
        queue_total_columns_hint(column_count, inverse, context.queues.policy());
    let mut rollback = ColumnMutationRollback::capture(columns)?;
    let dispatch_result = (|| -> MetalResult<()> {
        for (batch_index, (offset, batch_columns)) in batches.into_iter().enumerate() {
            let slot_index = batch_index % pipe_depth;
            if let Some(batch) = slots[slot_index].take() {
                batch.wait(columns, extent, true)?;
            }
            let start = usize::try_from(offset).expect("column offset fits usize");
            let width = usize::try_from(batch_columns).expect("batch column count fits usize");
            let range = start..start + width;
            let mut buffer = flatten_with_stats(&columns[range.clone()], ColumnStagingPhase::Fft)?;
            let metal_buffer = shared_pooled_buffer(&context.device, &mut buffer)?;
            let (queue, queue_index) = context.queues.select(queue_total_columns, batch_index);
            let mut args = base_args;
            args.column_count = batch_columns;
            let (threadgroups, threadgroup, logical_threads) =
                fft_dispatch_geometry(batch_columns, tuning.threadgroup_lanes);
            let profile = KernelProfileParams {
                kind: if inverse {
                    KernelKind::Ifft
                } else {
                    KernelKind::Fft
                },
                bytes: fft_bytes_per_batch(column_len, batch_columns),
                elements: column_len.saturating_mul(u64::from(batch_columns)),
                columns: batch_columns,
            };
            let mut tickets = SmallVec::<[DispatchTicket; 2]>::new();
            let sample_request = fft_selection.sample_for(batch_columns);
            let mut ticket = submit_compute_with_geometry(
                queue,
                queue_index,
                &context.fft,
                Some((threadgroups, threadgroup, logical_threads)),
                logical_threads,
                Some(profile),
                sample_request.is_some(),
                |encoder: &ComputeCommandEncoderRef| {
                    encoder.set_buffer(0, Some(&metal_buffer), 0);
                    encoder.set_buffer(1, Some(&twiddle_buffer), 0);
                    encoder.set_bytes(
                        2,
                        mem::size_of::<FftArgs>() as u64,
                        ptr::from_ref(&args).cast(),
                    );
                },
            )?;
            if let Some(sample) = sample_request {
                ticket = ticket.with_adaptive_sample(sample);
            }
            tickets.push(ticket);
            if let Some(stage_start) = post_stage_start {
                let post_args = PostTileArgs {
                    column_len,
                    log_len: log_size,
                    column_count: batch_columns,
                    column_offset: 0,
                    stage_start,
                    inverse: args.inverse,
                    threadgroup_lanes: args.threadgroup_lanes,
                    coset: 1,
                };
                tickets.push(submit_post_tile_dispatch(
                    context,
                    queue,
                    queue_index,
                    &metal_buffer,
                    &twiddle_buffer,
                    post_args,
                    batch_columns,
                    profile,
                )?);
            }
            slots[slot_index] = Some(ColumnBatchTicket {
                range,
                buffer,
                metal_buffer,
                tickets,
            });
        }
        Ok(())
    })();
    rollback_columns_on_error(dispatch_result, columns, &mut rollback)?;
    let pending_batches: Vec<ColumnBatchTicket> = slots.into_iter().flatten().collect();
    Ok(PendingColumns::new(
        columns,
        extent,
        twiddle_buffer,
        pending_batches,
        rollback,
    ))
}
fn fft_dispatch_geometry(column_count: u32, threadgroup_lanes: u32) -> (MTLSize, MTLSize, u64) {
    let lanes = u64::from(threadgroup_lanes.max(1));
    let groups = MTLSize::new(u64::from(column_count), 1, 1);
    let threads = MTLSize::new(lanes, 1, 1);
    let logical = lanes * u64::from(column_count);
    (groups, threads, logical)
}
fn poseidon_dispatch_geometry(
    state_count: u32,
    tuning: metal_config::PoseidonTuning,
    limits: &PipelineLimits,
) -> (MTLSize, MTLSize, u64, u32) {
    let states_per_lane = tuning.states_per_lane.max(1);
    let default_lanes = tuning.threadgroup_lanes.max(1);
    let override_width = threadgroup_override().unwrap_or(u64::from(default_lanes));
    let max_threads = u64::from(limits.max_threads.max(1));
    let lanes = override_width.min(max_threads).max(1);
    let states = u64::from(state_count);
    let per_lane = u64::from(states_per_lane);
    let logical_threads = states.div_ceil(per_lane).max(1);
    let group_width = lanes.min(logical_threads).max(1);
    let threadgroups = logical_threads.div_ceil(group_width).max(1);
    let group_count = threadgroups.max(1);
    (
        MTLSize::new(group_count, 1, 1),
        MTLSize::new(group_width, 1, 1),
        logical_threads,
        states_per_lane,
    )
}
fn bn254_poseidon_dispatch_geometry(
    batch_count: u32,
    tuning: metal_config::PoseidonTuning,
    limits: &PipelineLimits,
) -> (MTLSize, MTLSize, u64, u32) {
    let states_per_lane = tuning.states_per_lane.max(1);
    let states = u64::from(batch_count);
    let logical_threads = states.div_ceil(u64::from(states_per_lane)).max(1);
    let default_lanes = tuning.threadgroup_lanes.max(1);
    let override_width = threadgroup_override().unwrap_or(u64::from(default_lanes));
    let max_threads = u64::from(limits.max_threads.max(1));
    let threadgroup_width = override_width
        .min(max_threads)
        .min(u64::from(BN254_POSEIDON_THREADGROUP_CAPACITY))
        .min(logical_threads.max(1))
        .max(1);
    let threadgroups = logical_threads.div_ceil(threadgroup_width).max(1);
    (
        MTLSize::new(threadgroups, 1, 1),
        MTLSize::new(threadgroup_width, 1, 1),
        logical_threads.max(1),
        states_per_lane,
    )
}
fn poseidon_recommended_states_per_batch(
    state_count: u32,
    tuning: metal_config::PoseidonTuning,
) -> u32 {
    if state_count == 0 {
        return 0;
    }
    let lanes = tuning.threadgroup_lanes.max(1);
    let per_lane = tuning.states_per_lane.max(1);
    let base = lanes.saturating_mul(per_lane).max(1);
    let multiplier = metal_config::poseidon_batch_multiplier().max(1);
    let target = base.saturating_mul(multiplier).max(base);
    state_count.min(target)
}
fn select_poseidon_batch(state_count: u32, tuning: metal_config::PoseidonTuning) -> BatchSelection {
    select_poseidon_batch_with_scheduler(adaptive_scheduler(), state_count, tuning)
}
fn select_poseidon_batch_with_scheduler(
    scheduler: &AdaptiveScheduler,
    state_count: u32,
    tuning: metal_config::PoseidonTuning,
) -> BatchSelection {
    debug_assert!(
        state_count > 0,
        "poseidon batch requires positive state count"
    );
    let recommended = poseidon_recommended_states_per_batch(state_count, tuning);
    scheduler.select_poseidon(recommended, recommended.max(MIN_POSEIDON_STATES_PER_BATCH))
}
fn poseidon_element_range(offset: u32, count: u32) -> MetalResult<Range<usize>> {
    let start_state = usize::try_from(offset)
        .map_err(|_| GpuError::InvalidInput("poseidon offset exceeds usize"))?;
    let count_states = usize::try_from(count)
        .map_err(|_| GpuError::InvalidInput("poseidon batch exceeds usize"))?;
    let start = start_state
        .checked_mul(STATE_WIDTH)
        .ok_or(GpuError::InvalidInput(
            "poseidon range start exceeds usize bounds",
        ))?;
    let len = count_states
        .checked_mul(STATE_WIDTH)
        .ok_or(GpuError::InvalidInput(
            "poseidon batch length exceeds usize bounds",
        ))?;
    Ok(start..start + len)
}
fn poseidon_payload_range(offset: u32, count: u32, padded_len: usize) -> MetalResult<Range<usize>> {
    let start_state = usize::try_from(offset)
        .map_err(|_| GpuError::InvalidInput("poseidon payload offset exceeds usize"))?;
    let count_states = usize::try_from(count)
        .map_err(|_| GpuError::InvalidInput("poseidon payload count exceeds usize"))?;
    let start = start_state
        .checked_mul(padded_len)
        .ok_or(GpuError::InvalidInput(
            "poseidon payload start exceeds usize bounds",
        ))?;
    let len = count_states
        .checked_mul(padded_len)
        .ok_or(GpuError::InvalidInput(
            "poseidon payload length exceeds usize bounds",
        ))?;
    Ok(start..start + len)
}
#[derive(Clone, Debug)]
struct ColumnBatchIter {
    total: u32,
    batch_size: u32,
    offset: u32,
}
impl ColumnBatchIter {
    fn new(total: u32, batch_size: u32) -> Self {
        Self {
            total,
            batch_size: batch_size.max(1),
            offset: 0,
        }
    }
    fn remaining(&self) -> u32 {
        self.total.saturating_sub(self.offset)
    }
    fn remaining_batches(&self) -> usize {
        let remaining = self.remaining();
        if remaining == 0 {
            return 0;
        }
        let batches = remaining.div_ceil(self.batch_size);
        usize::try_from(batches).unwrap_or(usize::MAX)
    }
}
impl Iterator for ColumnBatchIter {
    type Item = (u32, u32);
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.total {
            return None;
        }
        let remaining = self.total - self.offset;
        let chunk = self.batch_size.min(remaining);
        let start = self.offset;
        self.offset = self.offset.saturating_add(chunk);
        Some((start, chunk))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining_batches();
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for ColumnBatchIter {
    fn len(&self) -> usize {
        self.remaining_batches()
    }
}
impl FusedIterator for ColumnBatchIter {}
fn column_batch_ranges(total: u32, batch_size: u32) -> ColumnBatchIter {
    ColumnBatchIter::new(total, batch_size)
}
fn lde_tile_stage_limit(eval_log: u32) -> u32 {
    if eval_log == 0 {
        return 0;
    }
    let hints = metal_config::device_hint_snapshot();
    let limit = lde_tile_stage_override()
        .unwrap_or_else(|| metal_config::lde_tile_stage_target(eval_log, hints));
    let clamped = limit
        .clamp(
            metal_config::LDE_TILE_STAGE_LIMIT_MIN,
            metal_config::LDE_TILE_STAGE_LIMIT_MAX,
        )
        .min(eval_log);
    LAST_LDE_TILE_LIMIT.store(clamped, Ordering::Release);
    clamped
}
fn lde_tile_stage_override() -> Option<u32> {
    *LDE_TILE_OVERRIDE.get_or_init(|| {
        debug_env_var(LDE_TILE_STAGE_ENV).and_then(|raw| match parse_lde_tile_stage_override(raw.trim()) {
            Ok(value) => {
                debug!(
                    target: "fastpq::metal",
                    stages = value,
                    "overriding Metal LDE tile stages via {LDE_TILE_STAGE_ENV}"
                );
                Some(value)
            }
            Err(error) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    %error,
                    default_stages = metal_config::LDE_TILE_STAGE_LIMIT_MAX,
                    "ignoring invalid {LDE_TILE_STAGE_ENV} override; keeping heuristic tile depth"
                );
                None
            }
        })
    })
}
fn parse_lde_tile_stage_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if !(metal_config::LDE_TILE_STAGE_LIMIT_MIN..=metal_config::LDE_TILE_STAGE_LIMIT_MAX)
        .contains(&value)
    {
        return Err("tile depth out of supported range (1–8 stages)");
    }
    Ok(value)
}
fn post_tile_stage_start(total_log: u32, local_stage_limit: u32) -> Option<u32> {
    let stage_start = total_log.min(local_stage_limit);
    (stage_start < total_log).then_some(stage_start)
}
fn submit_post_tile_dispatch(
    context: &MetalPipelines,
    queue: &CommandQueue,
    queue_index: usize,
    column_buffer: &Buffer,
    twiddle_buffer: &Buffer,
    args: PostTileArgs,
    batch_columns: u32,
    profile: KernelProfileParams,
) -> MetalResult<DispatchTicket> {
    let (threadgroups, threadgroup, logical_threads) =
        fft_dispatch_geometry(batch_columns, args.threadgroup_lanes);
    let ticket = submit_compute_with_geometry(
        queue,
        queue_index,
        &context.post_tile,
        Some((threadgroups, threadgroup, logical_threads)),
        logical_threads,
        Some(profile),
        false,
        |encoder: &ComputeCommandEncoderRef| {
            encoder.set_buffer(0, Some(column_buffer), 0);
            encoder.set_buffer(1, Some(twiddle_buffer), 0);
            encoder.set_bytes(
                2,
                mem::size_of::<PostTileArgs>() as u64,
                ptr::from_ref(&args).cast(),
            );
        },
    )?;
    record_post_tile_sample(profile.kind, args.log_len, args.stage_start, batch_columns);
    Ok(ticket)
}
#[allow(dead_code)] // Metal IFFT entry point is unused in non-macOS test environments
pub fn ifft_columns(columns: &mut [Vec<u64>], log_size: u32, root: u64) -> MetalResult<()> {
    let _ = goldilocks_domain_len(log_size)?;
    if columns.is_empty() {
        return Ok(());
    }
    ifft_columns_async(columns, log_size, root)?.wait()
}
/// Dispatches an inverse FFT and returns a pending handle for the caller to await.
pub(crate) fn ifft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    root: u64,
) -> MetalResult<PendingColumns<'a>> {
    dispatch_fft_columns(columns, log_size, root, true)
}
/// Returns the resolved FFT tuning (threadgroup lanes/tile stages) for the current Metal device.
pub fn fft_tuning_snapshot(log_size: u32) -> MetalResult<metal_config::FftTuning> {
    let _ = goldilocks_domain_len(log_size)?;
    let context = metal_context()?;
    let limits = pipeline_limits(&context.fft);
    Ok(metal_config::fft_tuning(
        log_size,
        limits.exec_width,
        limits.max_threads,
    ))
}
/// Returns the resolved Poseidon tuning (threadgroup lanes/states per lane) for the current device.
pub fn poseidon_tuning_snapshot() -> MetalResult<metal_config::PoseidonTuning> {
    let context = metal_context()?;
    let limits = pipeline_limits(&context.poseidon_permute);
    let mut tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    tuning.states_per_lane = 1;
    Ok(tuning)
}
#[allow(dead_code)] // Metal LDE entry point is unused when Metal is not available
pub fn lde_columns(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    lde_root: u64,
    coset: u64,
) -> MetalResult<Option<Vec<Vec<u64>>>> {
    let _ = goldilocks_lde_domain_lengths(trace_log, blowup_log)?;
    if coeffs.is_empty() {
        return Ok(Some(Vec::new()));
    }
    lde_columns_async(coeffs, trace_log, blowup_log, lde_root, coset)?.wait()
}
/// Dispatches an LDE kernel and returns a pending handle so callers can wait later.
pub(crate) fn lde_columns_async(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    lde_root: u64,
    coset: u64,
) -> MetalResult<PendingLde> {
    let (trace_len, eval_log, eval_len) = goldilocks_lde_domain_lengths(trace_log, blowup_log)?;
    if coeffs.iter().any(|column| column.len() != trace_len) {
        return Err(GpuError::InvalidInput(
            "coefficient columns must share length",
        ));
    }
    let trace_len_u64 = u64::try_from(trace_len)
        .map_err(|_| GpuError::InvalidInput("trace length exceeds u64::MAX"))?;
    let eval_len_u64 = u64::try_from(eval_len)
        .map_err(|_| GpuError::InvalidInput("lde length exceeds u64::MAX"))?;
    let column_count = u32::try_from(coeffs.len())
        .map_err(|_| GpuError::InvalidInput("column count exceeds u32::MAX"))?;
    let coeff_elements = coeffs
        .len()
        .checked_mul(trace_len)
        .ok_or(GpuError::InvalidInput(
            "LDE coefficient length exceeds platform limits",
        ))?;
    let eval_elements = coeffs
        .len()
        .checked_mul(eval_len)
        .ok_or(GpuError::InvalidInput(
            "LDE output length exceeds platform limits",
        ))?;
    let context = metal_context()?;
    validate_metal_pooled_word_len(&context.device, coeff_elements)?;
    validate_metal_pooled_word_len(&context.device, eval_elements)?;
    let mut coeff_buffer = flatten_with_stats(coeffs, ColumnStagingPhase::Lde)?;
    // Pre-zero the evaluation buffer so the Metal kernel can assume padded slots are zeroed.
    let stats_enabled = LDE_STATS_ENABLED.load(Ordering::Acquire);
    let queue_before = snapshot_queue_depth_stats();
    let zero_timer = stats_enabled.then(|| Instant::now());
    let mut eval_buffer = PooledBuffer::zeroed(eval_elements)?;
    let queue_after = snapshot_queue_depth_stats();
    let queue_delta = match (queue_before, queue_after) {
        (Some(before), Some(after)) => Some(after.delta_since(&before)),
        _ => None,
    };
    let host_stats = zero_timer.map(|start| LdeHostStats {
        zero_fill_bytes: eval_buffer.len().saturating_mul(mem::size_of::<u64>()),
        zero_fill_ms: elapsed_ms(start.elapsed()),
        queue_delta,
    });
    let coeff_metal = shared_pooled_buffer(&context.device, &mut coeff_buffer)?;
    let eval_metal = shared_pooled_buffer(&context.device, &mut eval_buffer)?;
    let stage_twiddle_buffer = context.stage_twiddle_buffer(eval_log, lde_root, false)?;
    let limits = pipeline_limits(&context.lde);
    let tuning = metal_config::fft_tuning(eval_log, limits.exec_width, limits.max_threads);
    let local_stage_limit = lde_tile_stage_limit(eval_log);
    let base_args = LdeArgs {
        trace_len: trace_len_u64,
        eval_len: eval_len_u64,
        trace_log,
        blowup_log,
        column_count,
        column_offset: 0,
        threadgroup_lanes: tuning.threadgroup_lanes,
        local_stage_limit,
        coset,
    };
    let (mut tickets, ticket_window) = pending_ticket_window::<DispatchTicket>()?;
    let post_stage_start = post_tile_stage_start(eval_log, local_stage_limit);
    let lde_selection = select_lde_batch(eval_log, tuning.threadgroup_lanes);
    let batch_size = lde_selection.columns();
    let batches = column_batch_ranges(column_count, batch_size);
    for (batch_index, (offset, batch_columns)) in batches.into_iter().enumerate() {
        if let Some(ticket) = pop_oldest_ticket_if_full(&mut tickets, ticket_window) {
            wait_for_ticket(ticket)?;
        }
        let (queue, queue_index) = context.queues.select(column_count, batch_index);
        let mut args = base_args;
        args.column_offset = offset;
        let (threadgroups, threadgroup, logical_threads) =
            fft_dispatch_geometry(batch_columns, tuning.threadgroup_lanes);
        let profile = KernelProfileParams {
            kind: KernelKind::Lde,
            bytes: lde_bytes_per_batch(trace_len_u64, eval_len_u64, batch_columns),
            elements: eval_len_u64.saturating_mul(u64::from(batch_columns)),
            columns: batch_columns,
        };
        let sample_request = lde_selection.sample_for(batch_columns);
        let mut ticket = submit_compute_with_geometry(
            queue,
            queue_index,
            &context.lde,
            Some((threadgroups, threadgroup, logical_threads)),
            logical_threads,
            Some(profile),
            sample_request.is_some(),
            |encoder: &ComputeCommandEncoderRef| {
                encoder.set_buffer(0, Some(&coeff_metal), 0);
                encoder.set_buffer(1, Some(&eval_metal), 0);
                encoder.set_buffer(2, Some(&stage_twiddle_buffer), 0);
                encoder.set_bytes(
                    3,
                    mem::size_of::<LdeArgs>() as u64,
                    ptr::from_ref(&args).cast(),
                );
            },
        )?;
        if let Some(sample) = sample_request {
            ticket = ticket.with_adaptive_sample(sample);
        }
        tickets.push(ticket);
        if let Some(stage_start) = post_stage_start {
            if let Some(ticket) = pop_oldest_ticket_if_full(&mut tickets, ticket_window) {
                wait_for_ticket(ticket)?;
            }
            let post_args = PostTileArgs {
                column_len: eval_len_u64,
                log_len: eval_log,
                column_count,
                column_offset: offset,
                stage_start,
                inverse: 0,
                threadgroup_lanes: args.threadgroup_lanes,
                coset: 1,
            };
            tickets.push(submit_post_tile_dispatch(
                context,
                queue,
                queue_index,
                &eval_metal,
                &stage_twiddle_buffer,
                post_args,
                batch_columns,
                profile,
            )?);
        }
    }
    Ok(PendingLde::new(
        coeffs.len(),
        eval_len,
        1,
        coeff_buffer,
        eval_buffer,
        coeff_metal,
        eval_metal,
        stage_twiddle_buffer,
        tickets,
        host_stats,
    ))
}
pub fn poseidon_permute(states: &mut [u64]) -> MetalResult<()> {
    if states.is_empty() {
        return Ok(());
    }
    if !states.len().is_multiple_of(STATE_WIDTH) {
        return Err(GpuError::InvalidInput(
            "poseidon states must be a multiple of STATE_WIDTH",
        ));
    }
    let context = metal_context()?;
    let state_count = u32::try_from(states.len() / STATE_WIDTH)
        .map_err(|_| GpuError::InvalidInput("poseidon batch exceeds u32::MAX states"))?;
    let limits = pipeline_limits(&context.poseidon_permute);
    let mut tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    // `poseidon_permute` backs the sponge/preflight path where each input state is
    // independent. Keep that kernel on one state per lane; the trace kernels keep
    // their multi-state batching and have separate parity coverage.
    tuning.states_per_lane = 1;
    let poseidon_selection = select_poseidon_batch(state_count, tuning);
    let batch_states = poseidon_selection.columns();
    let max_batch_words = usize::try_from(state_count.min(batch_states))
        .ok()
        .and_then(|count| count.checked_mul(STATE_WIDTH))
        .ok_or(GpuError::InvalidInput(
            "Metal Poseidon batch buffer length exceeds platform limits",
        ))?;
    validate_metal_pooled_word_len(&context.device, max_batch_words)?;
    let batches = column_batch_ranges(state_count, batch_states);
    let pipe_depth = POSEIDON_DISPATCH_PIPE_DEPTH;
    let mut slots: Vec<Option<PoseidonBatchTicket>> = (0..pipe_depth).map(|_| None).collect();
    let original = try_clone_metal_words(
        states,
        "Metal Poseidon rollback data exceeds available host memory",
    )?;
    let dispatch_result = (|| -> MetalResult<()> {
        for (batch_index, (offset, count)) in batches.into_iter().enumerate() {
            let slot_index = batch_index % pipe_depth;
            if let Some(ticket) = slots[slot_index].take() {
                ticket.wait(states, true)?;
            }
            let element_range = poseidon_element_range(offset, count)?;
            let mut buffer = clone_slice_with_stats(
                &states[element_range.clone()],
                ColumnStagingPhase::Poseidon,
            )?;
            let metal_buffer = shared_pooled_buffer(&context.device, &mut buffer)?;
            let (threadgroups, threadgroup, logical_threads, states_per_lane) =
                poseidon_dispatch_geometry(count, tuning, &limits);
            let args = PoseidonArgs {
                state_count: count,
                states_per_lane,
                block_count: 0,
                _reserved: 0,
            };
            let profile = KernelProfileParams {
                kind: KernelKind::Poseidon,
                bytes: poseidon_bytes_per_batch(count),
                elements: u64::from(count)
                    .saturating_mul(u64::try_from(STATE_WIDTH).unwrap_or(u64::MAX)),
                columns: count,
            };
            let (queue, queue_index) = context.queues.select(state_count, batch_index);
            let sample_request = poseidon_selection.sample_for(count);
            let mut ticket = submit_compute_with_geometry(
                queue,
                queue_index,
                &context.poseidon_permute,
                Some((threadgroups, threadgroup, logical_threads)),
                logical_threads,
                Some(profile),
                sample_request.is_some(),
                |encoder: &ComputeCommandEncoderRef| {
                    encoder.set_buffer(0, Some(&metal_buffer), 0);
                    encoder.set_bytes(
                        1,
                        mem::size_of::<PoseidonArgs>() as u64,
                        ptr::from_ref(&args).cast(),
                    );
                },
            )?;
            if let Some(sample) = sample_request {
                ticket = ticket.with_adaptive_sample(sample);
            }
            slots[slot_index] = Some(PoseidonBatchTicket {
                range: element_range,
                buffer,
                metal_buffer,
                ticket,
            });
        }
        for ticket in slots.into_iter().flatten() {
            ticket.wait(states, false)?;
        }
        Ok(())
    })();
    if dispatch_result.is_err() {
        states.copy_from_slice(&original);
    }
    dispatch_result
}
pub fn poseidon_hash_columns(batch: &PoseidonColumnBatch) -> MetalResult<Vec<u64>> {
    if batch.is_empty() {
        return Ok(Vec::new());
    }
    if batch.block_count() == 0 {
        return try_zeroed_metal_words(
            batch.columns(),
            "Metal Poseidon zero-block output exceeds available host memory",
        );
    }
    let padded_len = batch.padded_len();
    if padded_len == 0 {
        return try_zeroed_metal_words(
            batch.columns(),
            "Metal Poseidon empty-payload output exceeds available host memory",
        );
    }
    let context = metal_context()?;
    let column_count = u32::try_from(batch.columns())
        .map_err(|_| GpuError::InvalidInput("poseidon column count exceeds u32::MAX"))?;
    let block_count = u32::try_from(batch.block_count())
        .map_err(|_| GpuError::InvalidInput("poseidon block count exceeds u32::MAX"))?;
    let padded_len_u32 = u32::try_from(padded_len)
        .map_err(|_| GpuError::InvalidInput("poseidon padded length exceeds u32::MAX"))?;
    // Keep tuning and submission tied to the same function-specific pipeline limits.
    let pipeline = &context.poseidon_hash;
    let limits = pipeline_limits(pipeline);
    let mut tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    // Cross-column command batching is parity-covered, but packed multiple
    // sponge states per Metal lane diverges for non-leading lanes on current
    // Apple drivers. Keep one state per lane and batch across lanes/commands.
    tuning.states_per_lane = 1;
    let selection = select_poseidon_batch(column_count, tuning);
    let columns_per_batch = selection.columns();
    let max_batch_columns = usize::try_from(column_count.min(columns_per_batch)).map_err(|_| {
        GpuError::InvalidInput("Metal Poseidon batch column count exceeds platform limits")
    })?;
    let max_payload_words =
        max_batch_columns
            .checked_mul(padded_len)
            .ok_or(GpuError::InvalidInput(
                "Metal Poseidon payload buffer length exceeds platform limits",
            ))?;
    let max_state_words =
        max_batch_columns
            .checked_mul(STATE_WIDTH)
            .ok_or(GpuError::InvalidInput(
                "Metal Poseidon state buffer length exceeds platform limits",
            ))?;
    validate_metal_pooled_word_len(&context.device, max_payload_words)?;
    validate_metal_pooled_word_len(&context.device, max_state_words)?;
    let batches = column_batch_ranges(column_count, columns_per_batch);
    let mut result = try_zeroed_metal_words(
        batch.columns(),
        "Metal Poseidon result exceeds available host memory",
    )?;
    let payloads = batch.payloads();
    let pipe_depth = POSEIDON_DISPATCH_PIPE_DEPTH;
    let mut slots: Vec<Option<PoseidonHashTicket>> = (0..pipe_depth).map(|_| None).collect();
    for (batch_index, (offset, count)) in batches.into_iter().enumerate() {
        let slot_index = batch_index % pipe_depth;
        if let Some(ticket) = slots[slot_index].take() {
            ticket.wait(&mut result, true)?;
        }
        let payload_range = poseidon_payload_range(offset, count, padded_len)?;
        let column_offset = usize::try_from(offset)
            .map_err(|_| GpuError::InvalidInput("poseidon offset exceeds usize"))?;
        let mut payload_chunk = clone_slice_with_stats(
            &payloads[payload_range.clone()],
            ColumnStagingPhase::Poseidon,
        )?;
        let payload_buffer = shared_pooled_buffer(&context.device, &mut payload_chunk)?;
        let count_usize = usize::try_from(count)
            .map_err(|_| GpuError::InvalidInput("poseidon batch count exceeds usize bounds"))?;
        let state_words = count_usize
            .checked_mul(STATE_WIDTH)
            .ok_or(GpuError::InvalidInput(
                "poseidon state buffer length exceeds platform limits",
            ))?;
        let mut state_chunk = PooledBuffer::zeroed(state_words)?;
        let state_buffer = shared_pooled_buffer(&context.device, &mut state_chunk)?;
        let slice_chunk = batch
            .rebased_slices(column_offset, count_usize)
            .ok_or_else(|| GpuError::InvalidInput("poseidon descriptor rebasing failed"))?;
        let slice_buffer = copied_buffer(&context.device, &slice_chunk)?;
        let (threadgroups, threadgroup, logical_threads, states_per_lane) =
            poseidon_dispatch_geometry(count, tuning, &limits);
        let args = PoseidonArgs {
            state_count: count,
            states_per_lane,
            block_count,
            _reserved: 0,
        };
        let profile = KernelProfileParams {
            kind: KernelKind::Poseidon,
            bytes: poseidon_hash_bytes_per_batch(count, padded_len_u32),
            elements: u64::from(count)
                .saturating_mul(u64::try_from(STATE_WIDTH).unwrap_or(u64::MAX)),
            columns: count,
        };
        let (queue, queue_index) = context.queues.select(column_count, batch_index);
        let sample_request = selection.sample_for(count);
        let mut ticket = submit_compute_with_geometry(
            queue,
            queue_index,
            pipeline,
            Some((threadgroups, threadgroup, logical_threads)),
            logical_threads,
            Some(profile),
            sample_request.is_some(),
            |encoder: &ComputeCommandEncoderRef| {
                encoder.set_buffer(0, Some(&payload_buffer), 0);
                encoder.set_buffer(1, Some(&slice_buffer), 0);
                encoder.set_buffer(2, Some(&state_buffer), 0);
                encoder.set_bytes(
                    3,
                    mem::size_of::<PoseidonArgs>() as u64,
                    ptr::from_ref(&args).cast(),
                );
            },
        )?;
        if let Some(sample) = sample_request {
            ticket = ticket.with_adaptive_sample(sample);
        }
        slots[slot_index] = Some(PoseidonHashTicket {
            column_offset,
            payload: payload_chunk,
            slices: slice_chunk,
            states: state_chunk,
            payload_buffer,
            slice_buffer,
            state_buffer,
            ticket,
        });
    }
    for ticket in slots.into_iter().flatten() {
        ticket.wait(&mut result, false)?;
    }
    Ok(result)
}
pub fn poseidon_hash_rows(columns: &[Vec<u64>]) -> MetalResult<Vec<u64>> {
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let row_len = columns[0].len();
    if columns.iter().any(|column| column.len() != row_len) {
        return Err(GpuError::InvalidInput(
            "poseidon row columns must share length",
        ));
    }
    if row_len == 0 {
        return Ok(Vec::new());
    }
    let row_count = u32::try_from(row_len)
        .map_err(|_| GpuError::InvalidInput("poseidon row count exceeds u32::MAX"))?;
    let column_count = u32::try_from(columns.len())
        .map_err(|_| GpuError::InvalidInput("poseidon row column count exceeds u32::MAX"))?;
    let context = metal_context()?;
    let column_words = columns
        .len()
        .checked_mul(row_len)
        .ok_or(GpuError::InvalidInput(
            "Metal Poseidon row input length exceeds platform limits",
        ))?;
    validate_metal_pooled_word_len(&context.device, column_words)?;
    validate_metal_pooled_word_len(&context.device, row_len)?;
    let mut column_chunk = flatten_with_stats(columns, ColumnStagingPhase::Poseidon)?;
    let column_buffer = shared_pooled_buffer(&context.device, &mut column_chunk)?;
    let mut result = PooledBuffer::zeroed(row_len)?;
    let result_buffer = shared_pooled_buffer(&context.device, &mut result)?;
    let limits = pipeline_limits(&context.poseidon_hash_rows);
    let mut tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    // Row hashing absorbs values one at a time with sponge padding. Keep each
    // row on its own lane; multi-row lane packing has separate vector state and
    // row-index bookkeeping, and parity is more important than the small packing
    // win for v1 proof commitments.
    tuning.states_per_lane = 1;
    let selection = select_poseidon_batch(row_count, tuning);
    let batch_size = selection.columns();
    let total_batches =
        u32::try_from(column_batch_ranges(row_count, batch_size).len()).unwrap_or(u32::MAX);
    let (mut tickets, ticket_window) =
        pending_ticket_window::<(DispatchTicket, PoseidonRowDispatchEvidence)>()?;
    for (batch_index, (offset, count)) in column_batch_ranges(row_count, batch_size)
        .into_iter()
        .enumerate()
    {
        if let Some((ticket, evidence)) = pop_oldest_ticket_if_full(&mut tickets, ticket_window) {
            wait_for_ticket(ticket).map_err(|error| evidence.contextualize_error(error))?;
        }
        let (threadgroups, threadgroup, logical_threads, states_per_lane) =
            poseidon_dispatch_geometry(count, tuning, &limits);
        let args = PoseidonRowArgs {
            row_count,
            column_count,
            row_offset: offset,
            batch_count: count,
            states_per_lane,
        };
        let byte_estimate = poseidon_row_hash_bytes_per_batch(count, column_count);
        let profile = KernelProfileParams {
            kind: KernelKind::Poseidon,
            bytes: byte_estimate,
            elements: u64::from(count),
            columns: count,
        };
        let (queue, queue_index) = context.queues.select(row_count, batch_index);
        let evidence = PoseidonRowDispatchEvidence {
            batch_count: total_batches,
            batch_rows: count,
            row_count,
            column_count,
            logical_threads,
            threadgroups: threadgroups.width,
            threadgroup_width: threadgroup.width,
            states_per_lane,
            queue_index,
            byte_estimate,
        };
        let sample_request = selection.sample_for(count);
        let mut ticket = submit_compute_with_geometry(
            queue,
            queue_index,
            &context.poseidon_hash_rows,
            Some((threadgroups, threadgroup, logical_threads)),
            logical_threads,
            Some(profile),
            sample_request.is_some(),
            |encoder: &ComputeCommandEncoderRef| {
                encoder.set_buffer(0, Some(&column_buffer), 0);
                encoder.set_buffer(1, Some(&result_buffer), 0);
                encoder.set_bytes(
                    2,
                    mem::size_of::<PoseidonRowArgs>() as u64,
                    ptr::from_ref(&args).cast(),
                );
            },
        )
        .map_err(|error| evidence.contextualize_error(error))?;
        if let Some(sample) = sample_request {
            ticket = ticket.with_adaptive_sample(sample);
        }
        tickets.push((ticket, evidence));
    }
    for (ticket, evidence) in tickets {
        wait_for_ticket(ticket).map_err(|error| evidence.contextualize_error(error))?;
    }
    result.to_vec()
}
pub fn bn254_poseidon_hash_words(
    words: &[u64],
    slices: &[Bn254PoseidonBatchSlice],
) -> MetalResult<Vec<[u8; 32]>> {
    if slices.is_empty() {
        return Ok(Vec::new());
    }
    bn254_poseidon_hash_words_async(words, slices)?.wait()
}
/// Pending BN254 Poseidon word-batch dispatch.
///
/// The guard owns the staged Metal buffers until [`wait`](Self::wait), allowing
/// callers to overlap command completion with independent host work.
pub(crate) struct PendingBn254PoseidonWords {
    _word_chunk: PooledBuffer,
    _word_buffer: Buffer,
    _slice_chunk: Vec<Bn254PoseidonMetalSlice>,
    _slice_buffer: Buffer,
    _round_constants: PooledBuffer,
    _round_buffer: Buffer,
    _mds: PooledBuffer,
    _mds_buffer: Buffer,
    output: PooledBuffer,
    _output_buffer: Buffer,
    ticket: Option<DispatchTicket>,
    evidence: Bn254PoseidonDispatchEvidence,
    completed: bool,
}
impl PendingBn254PoseidonWords {
    /// Wait for the dispatch and collect canonical BN254 digest bytes.
    pub(crate) fn wait(mut self) -> MetalResult<Vec<[u8; 32]>> {
        self.finish()?;
        if !self.output.len().is_multiple_of(BN254_LIMBS) {
            return Err(GpuError::Execution {
                backend: GpuBackend::Metal,
                message: "BN254 Poseidon output was not limb aligned".to_owned(),
            });
        }
        let digest_count = self.output.len() / BN254_LIMBS;
        let mut digests = Vec::new();
        digests.try_reserve_exact(digest_count).map_err(|_| {
            GpuError::InvalidInput("BN254 Poseidon digest list exceeds available host memory")
        })?;
        for index in 0..digest_count {
            let mut limbs = [0u64; BN254_LIMBS];
            self.output
                .copy_range_to_slice(index * BN254_LIMBS, &mut limbs);
            digests.push(bn254_limbs_to_bytes(&limbs));
        }
        Ok(digests)
    }
    fn finish(&mut self) -> MetalResult<()> {
        if self.completed {
            return Ok(());
        }
        let ticket = self
            .ticket
            .take()
            .expect("pending BN254 Poseidon dispatch missing ticket");
        let result = wait_for_ticket(ticket).map_err(|error| {
            warn!(
                target: "fastpq::metal",
                batch_count = self.evidence.batch_count,
                word_count = self.evidence.word_count,
                logical_threads = self.evidence.logical_threads,
                threadgroups = self.evidence.threadgroups,
                threadgroup_width = self.evidence.threadgroup_width,
                states_per_lane = self.evidence.states_per_lane,
                queue_index = self.evidence.queue_index,
                byte_estimate = self.evidence.byte_estimate,
                %error,
                "BN254 Poseidon Metal runtime dispatch failed"
            );
            self.evidence.contextualize_error(error)
        });
        self.completed = true;
        result
    }
}
impl Drop for PendingBn254PoseidonWords {
    fn drop(&mut self) {
        if self.completed || self.ticket.is_none() {
            return;
        }
        if let Err(error) = self.finish() {
            warn!(
                target: "fastpq::metal",
                %error,
                "pending BN254 Poseidon word dispatch dropped without awaiting completion"
            );
        }
    }
}
pub(crate) fn bn254_poseidon_hash_words_async(
    words: &[u64],
    slices: &[Bn254PoseidonBatchSlice],
) -> MetalResult<PendingBn254PoseidonWords> {
    if slices.is_empty() {
        return Err(GpuError::InvalidInput(
            "BN254 Poseidon async dispatch requires at least one input",
        ));
    }
    let context = bn254_poseidon_context()?;
    let batch_count = u32::try_from(slices.len())
        .map_err(|_| GpuError::InvalidInput("BN254 Poseidon batch exceeds u32::MAX inputs"))?;
    let mut metal_slices = Vec::new();
    metal_slices.try_reserve_exact(slices.len()).map_err(|_| {
        GpuError::InvalidInput("BN254 Poseidon slice list exceeds available host memory")
    })?;
    for slice in slices {
        let end = slice
            .offset()
            .checked_add(slice.len())
            .ok_or(GpuError::InvalidInput(
                "BN254 Poseidon slice range overflows",
            ))?;
        if end > words.len() {
            return Err(GpuError::InvalidInput(
                "BN254 Poseidon slice exceeds flattened word buffer",
            ));
        }
        metal_slices.push(Bn254PoseidonMetalSlice {
            offset: u32::try_from(slice.offset()).map_err(|_| {
                GpuError::InvalidInput("BN254 Poseidon word offset exceeds u32::MAX")
            })?,
            len: u32::try_from(slice.len()).map_err(|_| {
                GpuError::InvalidInput("BN254 Poseidon word length exceeds u32::MAX")
            })?,
        });
    }
    let params = bn254_poseidon_width3_params();
    let staged_words = if words.is_empty() { &[0u64][..] } else { words };
    let output_len = slices
        .len()
        .checked_mul(BN254_LIMBS)
        .ok_or(GpuError::InvalidInput(
            "BN254 Poseidon output length overflows",
        ))?;
    validate_metal_pooled_word_len(&context.device, staged_words.len())?;
    validate_metal_pooled_word_len(&context.device, params.round_constants.len())?;
    validate_metal_pooled_word_len(&context.device, params.mds.len())?;
    validate_metal_pooled_word_len(&context.device, output_len)?;
    let mut word_chunk = PooledBuffer::from_slice(staged_words)?;
    let word_buffer = shared_pooled_buffer(&context.device, &mut word_chunk)?;
    let slice_chunk = metal_slices;
    let slice_buffer = copied_buffer(&context.device, &slice_chunk)?;
    let mut round_constants = PooledBuffer::from_slice(&params.round_constants)?;
    let round_buffer = shared_pooled_buffer(&context.device, &mut round_constants)?;
    let mut mds = PooledBuffer::from_slice(&params.mds)?;
    let mds_buffer = shared_pooled_buffer(&context.device, &mut mds)?;
    let mut output = PooledBuffer::zeroed(output_len)?;
    let output_buffer = shared_pooled_buffer(&context.device, &mut output)?;
    let limits = pipeline_limits(&context.bn254_poseidon_hash);
    let tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    let (threadgroups, threadgroup, logical_threads, states_per_lane) =
        bn254_poseidon_dispatch_geometry(batch_count, tuning, &limits);
    let args = Bn254PoseidonArgs {
        batch_count,
        states_per_lane,
        round_count: params.round_count,
        _reserved: 0,
    };
    let byte_estimate = bn254_poseidon_hash_bytes(words.len(), slices.len(), params);
    let profile = KernelProfileParams {
        kind: KernelKind::Poseidon,
        bytes: byte_estimate,
        elements: u64::from(batch_count)
            .saturating_mul(u64::try_from(BN254_POSEIDON_WIDTH).unwrap_or(u64::MAX)),
        columns: batch_count,
    };
    let (queue, queue_index) = context.queues.select(batch_count, 0);
    let evidence = Bn254PoseidonDispatchEvidence {
        batch_count,
        word_count: words.len(),
        logical_threads,
        threadgroups: threadgroups.width,
        threadgroup_width: threadgroup.width,
        states_per_lane,
        queue_index,
        byte_estimate,
    };
    let ticket = submit_compute_with_geometry(
        queue,
        queue_index,
        &context.bn254_poseidon_hash,
        Some((threadgroups, threadgroup, logical_threads)),
        logical_threads,
        Some(profile),
        false,
        |encoder: &ComputeCommandEncoderRef| {
            encoder.set_buffer(0, Some(&word_buffer), 0);
            encoder.set_buffer(1, Some(&slice_buffer), 0);
            encoder.set_buffer(2, Some(&output_buffer), 0);
            encoder.set_buffer(3, Some(&round_buffer), 0);
            encoder.set_buffer(4, Some(&mds_buffer), 0);
            encoder.set_bytes(
                5,
                mem::size_of::<Bn254PoseidonArgs>() as u64,
                ptr::from_ref(&args).cast(),
            );
        },
    )?;
    Ok(PendingBn254PoseidonWords {
        _word_chunk: word_chunk,
        _word_buffer: word_buffer,
        _slice_chunk: slice_chunk,
        _slice_buffer: slice_buffer,
        _round_constants: round_constants,
        _round_buffer: round_buffer,
        _mds: mds,
        _mds_buffer: mds_buffer,
        output,
        _output_buffer: output_buffer,
        ticket: Some(ticket),
        evidence,
        completed: false,
    })
}
/// Dispatch the low-level leaf-plus-parent kernel used by backend parity tests.
///
/// Production trace commitments use `trace::hash_columns_gpu_fused`, which
/// composes the parity-checked column and Merkle-pair batch paths. This entry
/// point stays available so the fused Metal kernels keep direct CPU parity
/// coverage without becoming the default commitment path.
#[allow(dead_code)]
pub fn poseidon_hash_columns_fused(batch: &PoseidonColumnBatch) -> MetalResult<Vec<u64>> {
    if batch.is_empty() {
        return Ok(Vec::new());
    }
    if batch.block_count() == 0 {
        return try_zeroed_metal_words(
            batch.columns(),
            "Metal fused Poseidon zero-block output exceeds available host memory",
        );
    }
    let padded_len = batch.padded_len();
    if padded_len == 0 {
        return try_zeroed_metal_words(
            batch.columns(),
            "Metal fused Poseidon empty-payload output exceeds available host memory",
        );
    }
    let context = metal_context()?;
    let column_count = u32::try_from(batch.columns())
        .map_err(|_| GpuError::InvalidInput("poseidon column count exceeds u32::MAX"))?;
    let parent_count_usize = batch.columns().div_ceil(2);
    let parent_count = u32::try_from(parent_count_usize)
        .map_err(|_| GpuError::InvalidInput("poseidon parent count exceeds u32::MAX"))?;
    let block_count = u32::try_from(batch.block_count())
        .map_err(|_| GpuError::InvalidInput("poseidon block count exceeds u32::MAX"))?;
    let padded_len_u32 = u32::try_from(padded_len)
        .map_err(|_| GpuError::InvalidInput("poseidon padded length exceeds u32::MAX"))?;
    let leaf_pipeline = &context.poseidon_trace_fused;
    let limits = pipeline_limits(leaf_pipeline);
    let mut tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    tuning.states_per_lane = 1;
    let (threadgroups, threadgroup, logical_threads, states_per_lane) =
        poseidon_dispatch_geometry(column_count, tuning, &limits);
    validate_metal_pooled_word_len(&context.device, batch.payloads().len())?;
    let mut payload_chunk = clone_slice_with_stats(batch.payloads(), ColumnStagingPhase::Poseidon)?;
    let payload_buffer = shared_pooled_buffer(&context.device, &mut payload_chunk)?;
    let slice_chunk = batch
        .rebased_slices(0, batch.columns())
        .ok_or_else(|| GpuError::InvalidInput("poseidon descriptor rebasing failed"))?;
    let slice_buffer = copied_buffer(&context.device, &slice_chunk)?;
    let hash_words =
        batch
            .columns()
            .checked_add(parent_count_usize)
            .ok_or(GpuError::InvalidInput(
                "poseidon fused output length exceeds platform limits",
            ))?;
    validate_metal_pooled_word_len(&context.device, hash_words)?;
    let mut hash_chunk = PooledBuffer::zeroed(hash_words)?;
    let hash_buffer = shared_pooled_buffer(&context.device, &mut hash_chunk)?;
    let args = PoseidonFusedArgs {
        state_count: column_count,
        states_per_lane,
        block_count,
        leaf_offset: 0,
        parent_offset: column_count,
    };
    let profile = KernelProfileParams {
        kind: KernelKind::Poseidon,
        bytes: poseidon_hash_bytes_per_batch(column_count, padded_len_u32).saturating_add(
            u64::from(parent_count)
                .saturating_mul(u64::try_from(mem::size_of::<u64>()).unwrap_or(u64::MAX)),
        ),
        elements: u64::from(column_count),
        columns: column_count,
    };
    let (queue, queue_index) = context.queues.select(column_count, 0);
    let ticket = submit_compute_with_geometry(
        queue,
        queue_index,
        leaf_pipeline,
        Some((threadgroups, threadgroup, logical_threads)),
        logical_threads,
        Some(profile),
        false,
        |encoder: &ComputeCommandEncoderRef| {
            encoder.set_buffer(0, Some(&payload_buffer), 0);
            encoder.set_buffer(1, Some(&slice_buffer), 0);
            encoder.set_buffer(2, Some(&hash_buffer), 0);
            encoder.set_bytes(
                3,
                mem::size_of::<PoseidonFusedArgs>() as u64,
                ptr::from_ref(&args).cast(),
            );
        },
    )?;
    wait_for_ticket(ticket)?;
    let parent_limits = pipeline_limits(&context.poseidon_trace_parents);
    let mut parent_tuning =
        metal_config::poseidon_tuning(parent_limits.exec_width, parent_limits.max_threads);
    parent_tuning.states_per_lane = 1;
    let (parent_threadgroups, parent_threadgroup, parent_logical_threads, parent_states_per_lane) =
        poseidon_dispatch_geometry(parent_count, parent_tuning, &parent_limits);
    let parent_args = PoseidonFusedArgs {
        state_count: column_count,
        states_per_lane: parent_states_per_lane,
        block_count: 0,
        leaf_offset: 0,
        parent_offset: column_count,
    };
    let parent_profile = KernelProfileParams {
        kind: KernelKind::Poseidon,
        bytes: u64::from(parent_count)
            .saturating_mul(u64::try_from(3 * mem::size_of::<u64>()).unwrap_or(u64::MAX)),
        elements: u64::from(parent_count)
            .saturating_mul(u64::try_from(STATE_WIDTH).unwrap_or(u64::MAX)),
        columns: parent_count,
    };
    let (parent_queue, parent_queue_index) = context.queues.select(parent_count, 1);
    let parent_ticket = submit_compute_with_geometry(
        parent_queue,
        parent_queue_index,
        &context.poseidon_trace_parents,
        Some((
            parent_threadgroups,
            parent_threadgroup,
            parent_logical_threads,
        )),
        parent_logical_threads,
        Some(parent_profile),
        false,
        |encoder: &ComputeCommandEncoderRef| {
            encoder.set_buffer(0, Some(&hash_buffer), 0);
            encoder.set_bytes(
                1,
                mem::size_of::<PoseidonFusedArgs>() as u64,
                ptr::from_ref(&parent_args).cast(),
            );
        },
    )?;
    wait_for_ticket(parent_ticket)?;
    hash_chunk.to_vec()
}
struct MetalBufferBackingRetention {
    backing: Mutex<Option<Arc<PooledBufferBacking>>>,
}
impl MetalBufferBackingRetention {
    fn new(backing: Arc<PooledBufferBacking>) -> Self {
        Self {
            backing: Mutex::new(Some(backing)),
        }
    }
    fn release(&self) {
        match self.backing.lock() {
            Ok(mut backing) => {
                let _ = backing.take();
            }
            Err(poisoned) => {
                let _ = poisoned.into_inner().take();
            }
        }
    }
}
fn validate_metal_buffer_byte_len(device: &Device, byte_len: u64) -> MetalResult<()> {
    if byte_len == 0 {
        return Err(GpuError::InvalidInput(
            "Metal buffers require at least one byte",
        ));
    }
    let max_buffer_length = u64::try_from(device.max_buffer_length()).unwrap_or(u64::MAX);
    if byte_len > max_buffer_length {
        return Err(GpuError::InvalidInput(
            "Metal buffer exceeds the device max_buffer_length",
        ));
    }
    Ok(())
}

#[allow(unsafe_code)]
fn try_new_buffer_with_data(
    device: &DeviceRef,
    data: *const c_void,
    byte_len: u64,
    options: MTLResourceOptions,
) -> MetalResult<Buffer> {
    // SAFETY: the caller supplies a readable region of `byte_len` bytes. `newBufferWithBytes`
    // copies it before returning; the nullable SDK result is checked before metal-rs wraps it.
    let raw: *mut Object = unsafe {
        msg_send![device,
            newBufferWithBytes: data
            length: byte_len
            options: options
        ]
    };
    if raw.is_null() {
        return Err(metal_nil_error(
            "-[MTLDevice newBufferWithBytes:length:options:]",
        ));
    }
    // SAFETY: the non-null `new...` result carries +1 ownership.
    Ok(unsafe { Buffer::from_ptr(raw.cast()) })
}

fn validate_metal_pooled_word_len(device: &Device, word_len: usize) -> MetalResult<()> {
    let byte_len = metal_buffer_page_count(word_len)
        .checked_mul(mem::size_of::<MetalBufferPage>())
        .and_then(|len| u64::try_from(len).ok())
        .ok_or(GpuError::InvalidInput(
            "Metal pooled buffer length exceeds platform limits",
        ))?;
    validate_metal_buffer_byte_len(device, byte_len)
}

#[allow(unsafe_code)]
fn shared_pooled_buffer(device: &Device, data: &mut PooledBuffer) -> MetalResult<Buffer> {
    let (data_ptr, byte_len) = data.metal_region();
    validate_metal_buffer_byte_len(device, byte_len)?;
    let retention = Arc::new(MetalBufferBackingRetention::new(data.backing()));
    let completion_retention = Arc::clone(&retention);
    let deallocator = ConcreteBlock::new(move |_: *const c_void, _: NSUInteger| {
        completion_retention.release();
    })
    .copy();
    let deallocator_block: &Block<(*const c_void, NSUInteger), ()> = &deallocator;
    let device_ref: &DeviceRef = device;
    // SAFETY: the page-aligned backing remains retained until Metal invokes the copied
    // deallocator block. Check the SDK-nullable result before transferring +1 ownership.
    let raw: *mut Object = unsafe {
        msg_send![device_ref,
            newBufferWithBytesNoCopy: data_ptr
            length: byte_len
            options: MTLResourceOptions::StorageModeShared
            deallocator: Some(deallocator_block)
        ]
    };
    if raw.is_null() {
        return Err(metal_nil_error(
            "-[MTLDevice newBufferWithBytesNoCopy:length:options:deallocator:]",
        ));
    }
    // SAFETY: the non-null `new...` result carries +1 ownership.
    Ok(unsafe { Buffer::from_ptr(raw.cast()) })
}
fn copied_buffer<T>(device: &Device, data: &[T]) -> MetalResult<Buffer> {
    let byte_len = u64::try_from(mem::size_of_val(data)).map_err(|_| {
        GpuError::InvalidInput("Metal copied buffer length exceeds 64-bit representation")
    })?;
    if byte_len == 0 {
        return Err(GpuError::InvalidInput(
            "Metal copied buffers require at least one byte",
        ));
    }
    validate_metal_buffer_byte_len(device, byte_len)?;
    try_new_buffer_with_data(
        device,
        data.as_ptr().cast(),
        byte_len,
        MTLResourceOptions::StorageModeShared,
    )
}

#[allow(unsafe_code)]
fn try_command_buffer(queue: &CommandQueueRef) -> MetalResult<CommandBuffer> {
    // SAFETY: `commandBuffer` is a +0/autoreleased Objective-C result. Check for nil before
    // borrowing it through metal-rs, then retain an owned reference for the dispatch ticket.
    let raw: *mut Object = unsafe { msg_send![queue, commandBuffer] };
    if raw.is_null() {
        return Err(metal_nil_error("-[MTLCommandQueue commandBuffer]"));
    }
    // SAFETY: the raw pointer was checked and remains live in the surrounding autorelease pool.
    let borrowed = unsafe { CommandBufferRef::from_ptr(raw.cast()) };
    Ok(borrowed.to_owned())
}

#[allow(unsafe_code)]
fn try_compute_encoder(command: &CommandBufferRef) -> MetalResult<&ComputeCommandEncoderRef> {
    // SAFETY: `computeCommandEncoder` is a +0/autoreleased result whose lifetime is bounded by
    // the command buffer and surrounding autorelease pool. Validate nil before wrapping it.
    let raw: *mut Object = unsafe { msg_send![command, computeCommandEncoder] };
    if raw.is_null() {
        return Err(metal_nil_error("-[MTLCommandBuffer computeCommandEncoder]"));
    }
    // SAFETY: the non-null encoder remains live for this encoding scope.
    Ok(unsafe { ComputeCommandEncoderRef::from_ptr(raw.cast()) })
}
fn submit_compute<F>(
    queue: &CommandQueue,
    queue_index: usize,
    pipeline: &ComputePipelineState,
    thread_count: u64,
    profile: Option<KernelProfileParams>,
    collect_timing: bool,
    configure: F,
) -> MetalResult<DispatchTicket>
where
    F: FnOnce(&ComputeCommandEncoderRef),
{
    submit_compute_with_geometry(
        queue,
        queue_index,
        pipeline,
        None,
        thread_count,
        profile,
        collect_timing,
        configure,
    )
}
fn submit_compute_with_geometry<F>(
    queue: &CommandQueue,
    queue_index: usize,
    pipeline: &ComputePipelineState,
    geometry: Option<(MTLSize, MTLSize, u64)>,
    logical_threads: u64,
    profile: Option<KernelProfileParams>,
    collect_timing: bool,
    configure: F,
) -> MetalResult<DispatchTicket>
where
    F: FnOnce(&ComputeCommandEncoderRef),
{
    debug_assert!(logical_threads > 0, "Metal dispatch requires threads > 0");
    let mut permit = CommandPermit::try_new(queue_index)?;
    let (threadgroups, threadgroup, logical_threads) = match geometry {
        Some((groups, group, logical)) => (groups, group, logical),
        None => {
            let (groups, group) = dispatch_sizes(pipeline, logical_threads);
            (groups, group, logical_threads)
        }
    };
    let trace_label = profile
        .map(|params| params.kind.as_str())
        .unwrap_or("metal");
    let kernel_context = profile.map(|params| {
        let groups = threadgroups.width.max(1);
        let width = threadgroup.width.max(1);
        KernelDispatchContext::from_pipeline(params, logical_threads, groups, width, pipeline)
    });
    autoreleasepool(|| {
        let command_buffer = try_command_buffer(queue)?;
        let encoder = try_compute_encoder(&command_buffer)?;
        encoder.set_compute_pipeline_state(pipeline);
        configure(encoder);
        let trace_enabled = dispatch_trace_enabled();
        let tracing_start = if trace_enabled {
            trace_dispatch_start(
                trace_label,
                pipeline,
                logical_threads,
                &threadgroups,
                &threadgroup,
            );
            Some(Instant::now())
        } else {
            None
        };
        let timing_needed = collect_timing || trace_enabled || kernel_context.is_some();
        let timing_start = if timing_needed {
            Some(tracing_start.unwrap_or_else(Instant::now))
        } else {
            None
        };
        encoder.dispatch_thread_groups(threadgroups, threadgroup);
        encoder.end_encoding();
        let completion = permit.completion();
        let completion_handler = ConcreteBlock::new(move |_| {
            completion.complete();
        })
        .copy();
        command_buffer.add_completed_handler(&completion_handler);
        permit.mark_launched();
        command_buffer.commit();
        let trace_label = trace_enabled.then(|| trace_label.to_owned());
        Ok(DispatchTicket {
            command: command_buffer,
            trace_label,
            timing_start,
            kernel_context,
            permit,
            adaptive_sample: None,
        })
    })
}
fn dispatch_sizes(pipeline: &ComputePipelineState, threads: u64) -> (MTLSize, MTLSize) {
    let execution_width = pipeline.thread_execution_width().max(1);
    let max_threads = pipeline.max_total_threads_per_threadgroup().max(1);
    let base_width = execution_width.min(max_threads).min(threads.max(1));
    let threadgroup_width = threadgroup_override().map_or(base_width, |override_width| {
        override_width.min(max_threads).max(1).min(threads.max(1))
    });
    let threadgroup = MTLSize::new(threadgroup_width, 1, 1);
    let groups = threads.div_ceil(threadgroup.width.max(1)).max(1);
    (MTLSize::new(groups, 1, 1), threadgroup)
}
fn bn254_threadgroup_geometry(
    pipeline: &ComputePipelineState,
    elements: u64,
) -> (MTLSize, MTLSize) {
    let limits = pipeline_limits(pipeline);
    let max_threads = u64::from(limits.max_threads.max(1));
    let mut width = elements.min(max_threads).max(1);
    if let Some(override_width) = threadgroup_override() {
        width = width
            .min(override_width.max(1))
            .min(max_threads)
            .min(elements.max(1));
    }
    let threadgroup = MTLSize::new(width, 1, 1);
    (MTLSize::new(1, 1, 1), threadgroup)
}
fn fft_bytes_per_batch(column_len: u64, columns: u32) -> u64 {
    let element_bytes = u64::try_from(mem::size_of::<u64>()).unwrap_or(u64::MAX);
    let io_bytes = element_bytes.saturating_mul(2);
    let per_column = u128::from(column_len).saturating_mul(u128::from(io_bytes));
    clamp_u128_to_u64(per_column.saturating_mul(u128::from(columns)))
}
fn lde_bytes_per_batch(trace_len: u64, eval_len: u64, columns: u32) -> u64 {
    let element_bytes = u64::try_from(mem::size_of::<u64>()).unwrap_or(u64::MAX);
    let per_column = u128::from(trace_len + eval_len).saturating_mul(u128::from(element_bytes));
    clamp_u128_to_u64(per_column.saturating_mul(u128::from(columns)))
}
fn poseidon_bytes_per_batch(states: u32) -> u64 {
    let width = u64::try_from(STATE_WIDTH).unwrap_or(u64::MAX);
    let element_bytes = u64::try_from(mem::size_of::<u64>()).unwrap_or(u64::MAX);
    let per_state = u128::from(width).saturating_mul(u128::from(element_bytes));
    clamp_u128_to_u64(per_state.saturating_mul(u128::from(states)))
}
fn poseidon_hash_bytes_per_batch(states: u32, padded_len: u32) -> u64 {
    let element_bytes = u64::try_from(mem::size_of::<u64>()).unwrap_or(u64::MAX);
    let payload = u128::from(padded_len).saturating_mul(u128::from(element_bytes));
    let width = u64::try_from(STATE_WIDTH).unwrap_or(u64::MAX);
    let state = u128::from(width).saturating_mul(u128::from(element_bytes));
    let descriptor = u64::try_from(mem::size_of::<PoseidonColumnSlice>()).unwrap_or(u64::MAX);
    let descriptor_total = u128::from(descriptor);
    let per_column = payload
        .saturating_add(state)
        .saturating_add(descriptor_total);
    clamp_u128_to_u64(per_column.saturating_mul(u128::from(states)))
}
fn poseidon_row_hash_bytes_per_batch(rows: u32, columns: u32) -> u64 {
    let element_bytes = u64::try_from(mem::size_of::<u64>()).unwrap_or(u64::MAX);
    let input = u128::from(rows)
        .saturating_mul(u128::from(columns))
        .saturating_mul(u128::from(element_bytes));
    let output = u128::from(rows).saturating_mul(u128::from(element_bytes));
    clamp_u128_to_u64(input.saturating_add(output))
}
fn bn254_poseidon_hash_bytes(
    word_count: usize,
    slice_count: usize,
    params: &Bn254PoseidonWidth3Params,
) -> u64 {
    let element_bytes = u128::from(u64::try_from(mem::size_of::<u64>()).unwrap_or(u64::MAX));
    let input = u128::try_from(word_count)
        .unwrap_or(u128::MAX)
        .saturating_mul(element_bytes);
    let output = u128::try_from(slice_count.saturating_mul(BN254_LIMBS))
        .unwrap_or(u128::MAX)
        .saturating_mul(element_bytes);
    let descriptors = u128::try_from(slice_count)
        .unwrap_or(u128::MAX)
        .saturating_mul(u128::from(
            u64::try_from(mem::size_of::<Bn254PoseidonMetalSlice>()).unwrap_or(u64::MAX),
        ));
    let constants = u128::try_from(
        params
            .round_constants
            .len()
            .saturating_add(params.mds.len()),
    )
    .unwrap_or(u128::MAX)
    .saturating_mul(element_bytes);
    clamp_u128_to_u64(
        input
            .saturating_add(output)
            .saturating_add(descriptors)
            .saturating_add(constants),
    )
}
fn clamp_u128_to_u64(value: u128) -> u64 {
    if value > u128::from(u64::MAX) {
        u64::MAX
    } else {
        value as u64
    }
}
fn pending_ticket_window<T>() -> MetalResult<(Vec<T>, usize)> {
    let depth = command_semaphore()
        .limit()
        .clamp(1, MAX_RETAINED_DISPATCH_TICKETS);
    let mut tickets = Vec::new();
    tickets.try_reserve_exact(depth).map_err(|_| {
        GpuError::InvalidInput("Metal pending command window exceeds available host memory")
    })?;
    Ok((tickets, depth))
}
fn pop_oldest_ticket_if_full<T>(tickets: &mut Vec<T>, depth: usize) -> Option<T> {
    if tickets.len() < depth.max(1) {
        None
    } else {
        Some(tickets.remove(0))
    }
}
fn wait_for_ticket(mut ticket: DispatchTicket) -> MetalResult<()> {
    let trace_label = ticket.trace_label.clone();
    let timing_start = ticket.timing_start;
    let wait_start = Instant::now();
    let mut polls = 0usize;
    let status = loop {
        let status = ticket.command.status();
        if matches!(
            status,
            MTLCommandBufferStatus::Completed | MTLCommandBufferStatus::Error
        ) {
            break status;
        }
        if wait_start.elapsed() >= METAL_COMMAND_TIMEOUT {
            let duration = timing_start.map(|start| start.elapsed());
            if let Some(label) = trace_label {
                trace_dispatch_end_label(Some(label), duration.unwrap_or_default(), false);
            }
            return Err(GpuError::Execution {
                backend: GpuBackend::Metal,
                message: format!("command buffer timed out after {METAL_COMMAND_TIMEOUT:?}"),
            });
        }
        polls = polls.saturating_add(1);
        if polls <= 64 {
            thread::yield_now();
        } else if polls <= 256 {
            thread::sleep(Duration::from_micros(50));
        } else {
            thread::sleep(Duration::from_millis(1));
        }
    };
    let duration = timing_start.map(|start| start.elapsed());
    if let Some(label) = trace_label {
        trace_dispatch_end_label(
            Some(label),
            duration.unwrap_or_default(),
            status == MTLCommandBufferStatus::Completed,
        );
    }
    if status == MTLCommandBufferStatus::Completed {
        if let (Some(context), Some(elapsed)) = (ticket.kernel_context.as_ref(), duration) {
            record_kernel_stats(context, elapsed);
        }
        if let (Some(sample), Some(elapsed)) = (ticket.adaptive_sample.as_ref(), duration) {
            sample.record(elapsed);
        }
        ticket.permit.complete();
        Ok(())
    } else {
        ticket.permit.complete();
        Err(GpuError::Execution {
            backend: GpuBackend::Metal,
            message: format!("command buffer finished with status {:?}", status),
        })
    }
}
fn wait_for_tickets<T>(tickets: T) -> MetalResult<()>
where
    T: IntoIterator<Item = DispatchTicket>,
{
    for ticket in tickets {
        wait_for_ticket(ticket)?;
    }
    Ok(())
}
fn record_lde_stats(stats: LdeHostStats) {
    if !LDE_STATS_ENABLED.load(Ordering::Acquire) {
        return;
    }
    let store = LDE_STATS.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = store.lock() {
        *guard = Some(stats);
    }
}
fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
fn flatten_with_stats(
    columns: &[Vec<u64>],
    phase: ColumnStagingPhase,
) -> MetalResult<PooledBuffer> {
    let start = Instant::now();
    let buffer = PooledBuffer::from_columns(columns)?;
    record_staging_flatten(phase, start.elapsed());
    Ok(buffer)
}
fn clone_slice_with_stats(
    elements: &[u64],
    phase: ColumnStagingPhase,
) -> MetalResult<PooledBuffer> {
    let start = Instant::now();
    let buffer = PooledBuffer::from_slice(elements)?;
    record_staging_flatten(phase, start.elapsed());
    Ok(buffer)
}
fn buffer_pool() -> &'static Mutex<BufferPool> {
    BUFFER_POOL.get_or_init(|| Mutex::new(BufferPool::default()))
}
fn select_fft_batch(threadgroup_lanes: u32) -> BatchSelection {
    if let Some(columns) = fft_batch_override() {
        return BatchSelection::fixed(columns);
    }
    let recommended = default_fft_columns_per_batch(threadgroup_lanes);
    adaptive_scheduler().select_fft(recommended, MAX_FFT_COLUMNS_PER_BATCH)
}
fn default_fft_columns_per_batch(threadgroup_lanes: u32) -> u32 {
    let lanes = threadgroup_lanes.max(1);
    let target_threads = FFT_COLUMNS_TARGET_THREADS.max(lanes);
    let columns = target_threads / lanes;
    columns.clamp(MIN_FFT_COLUMNS_PER_BATCH, MAX_FFT_COLUMNS_PER_BATCH)
}
fn fft_batch_override() -> Option<u32> {
    *FFT_BATCH_OVERRIDE.get_or_init(|| {
        debug_env_var(FFT_COLUMNS_ENV).and_then(|raw| match parse_fft_batch_override(raw.trim()) {
            Ok(value) => {
                debug!(
                    target: "fastpq::metal",
                    columns = value,
                    "overriding Metal FFT batch columns via {FFT_COLUMNS_ENV}"
                );
                Some(value)
            }
            Err(error) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    %error,
                    default_batch = FFT_COLUMNS_TARGET_THREADS / FFT_THREADGROUP_CAPACITY,
                    "invalid {FFT_COLUMNS_ENV} override; keeping heuristic batch sizing"
                );
                None
            }
        })
    })
}
fn parse_fft_batch_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if !(MIN_FFT_COLUMNS_PER_BATCH..=MAX_FFT_COLUMNS_PER_BATCH).contains(&value) {
        return Err("batch size out of supported range (1–64 columns)");
    }
    Ok(value)
}
fn select_lde_batch(eval_log: u32, threadgroup_lanes: u32) -> BatchSelection {
    if let Some(columns) = lde_batch_override() {
        return BatchSelection::fixed(columns);
    }
    let recommended = default_lde_columns_per_batch(eval_log, threadgroup_lanes);
    let domain_cap = lde_domain_cap(eval_log).max(MIN_LDE_COLUMNS_PER_BATCH);
    adaptive_scheduler().select_lde(recommended, domain_cap)
}
fn default_lde_columns_per_batch(eval_log: u32, threadgroup_lanes: u32) -> u32 {
    let domain_cap = lde_domain_cap(eval_log);
    let lanes = threadgroup_lanes.max(1);
    let mut columns = LDE_COLUMNS_TARGET_THREADS
        .checked_div(lanes)
        .unwrap_or(0)
        .max(1);
    columns = columns.clamp(MIN_LDE_COLUMNS_PER_BATCH, MAX_LDE_COLUMNS_PER_BATCH);
    columns.min(domain_cap).max(MIN_LDE_COLUMNS_PER_BATCH)
}
fn lde_domain_cap(eval_log: u32) -> u32 {
    match eval_log {
        n if n >= 22 => 1,
        n if n >= 18 => 2,
        n if n >= 16 => 4,
        _ => 64,
    }
}
fn lde_batch_override() -> Option<u32> {
    *LDE_BATCH_OVERRIDE.get_or_init(|| {
        debug_env_var(LDE_COLUMNS_ENV).and_then(|raw| match parse_lde_batch_override(raw.trim()) {
            Ok(value) => {
                debug!(
                    target: "fastpq::metal",
                    columns = value,
                    "overriding Metal LDE batch columns via {LDE_COLUMNS_ENV}"
                );
                Some(value)
            }
            Err(error) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    %error,
                    default_batch = DEFAULT_LDE_COLUMNS_PER_BATCH,
                    "invalid {LDE_COLUMNS_ENV} override; keeping heuristic batch sizing"
                );
                None
            }
        })
    })
}
fn parse_lde_batch_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if !(MIN_LDE_COLUMNS_PER_BATCH..=MAX_LDE_COLUMNS_PER_BATCH).contains(&value) {
        return Err("batch size out of supported range (1–64 columns)");
    }
    Ok(value)
}
#[repr(C, align(16384))]
struct MetalBufferPage {
    words: [u64; METAL_BUFFER_PAGE_WORDS],
}
impl MetalBufferPage {
    fn zeroed() -> Self {
        Self {
            words: [0; METAL_BUFFER_PAGE_WORDS],
        }
    }
}
fn metal_buffer_page_count(word_len: usize) -> usize {
    word_len.div_ceil(METAL_BUFFER_PAGE_WORDS).max(1)
}
fn acquire_buffer(word_len: usize) -> MetalResult<Vec<MetalBufferPage>> {
    let page_count = metal_buffer_page_count(word_len);
    let mut pages = buffer_pool()
        .lock()
        .map_err(|_| GpuError::Execution {
            backend: GpuBackend::Metal,
            message: "Metal buffer pool lock poisoned".to_owned(),
        })?
        .take(page_count)?;
    pages.resize_with(page_count, MetalBufferPage::zeroed);
    Ok(pages)
}
#[derive(Default)]
struct BufferPool {
    spare: Vec<Vec<MetalBufferPage>>,
}
fn buffer_pool_capacity_is_cacheable(page_capacity: usize) -> bool {
    (1..=MAX_BUFFER_POOL_PAGES_PER_BUFFER).contains(&page_capacity)
}
impl BufferPool {
    fn take(&mut self, min_pages: usize) -> MetalResult<Vec<MetalBufferPage>> {
        let mut candidate = None;
        let mut best_capacity = usize::MAX;
        for (idx, buffer) in self.spare.iter().enumerate() {
            let capacity = buffer.capacity();
            if capacity >= min_pages && capacity < best_capacity {
                candidate = Some(idx);
                best_capacity = capacity;
            }
        }
        match candidate {
            Some(idx) => Ok(self.spare.swap_remove(idx)),
            None => {
                let mut buffer = Vec::new();
                buffer.try_reserve_exact(min_pages).map_err(|_| {
                    GpuError::InvalidInput("Metal pooled buffer pages exceed available host memory")
                })?;
                Ok(buffer)
            }
        }
    }
    fn recycle(&mut self, mut buffer: Vec<MetalBufferPage>) {
        if !buffer_pool_capacity_is_cacheable(buffer.capacity()) {
            return;
        }
        buffer.clear();
        self.spare.push(buffer);
        self.spare.sort_unstable_by_key(|buf| buf.capacity());
        if self.spare.len() > MAX_BUFFER_POOL_BUFFERS {
            self.spare.truncate(MAX_BUFFER_POOL_BUFFERS);
        }
        while self.spare.iter().fold(0usize, |pages, buffer| {
            pages.saturating_add(buffer.capacity())
        }) > MAX_BUFFER_POOL_CACHED_PAGES
        {
            let _ = self.spare.pop();
        }
    }
    #[cfg(test)]
    fn len_for_tests(&self) -> usize {
        self.spare.len()
    }
}
struct PooledBufferBacking {
    pages: Vec<MetalBufferPage>,
    logical_len: usize,
}
impl Drop for PooledBufferBacking {
    fn drop(&mut self) {
        let pages = mem::take(&mut self.pages);
        if pages.capacity() == 0 {
            return;
        }
        if let Ok(mut pool) = buffer_pool().lock() {
            pool.recycle(pages);
        }
    }
}
struct PooledBuffer {
    backing: Arc<PooledBufferBacking>,
}
impl PooledBuffer {
    fn from_pages(pages: Vec<MetalBufferPage>, logical_len: usize) -> Self {
        Self {
            backing: Arc::new(PooledBufferBacking { pages, logical_len }),
        }
    }
    fn from_columns(columns: &[Vec<u64>]) -> MetalResult<Self> {
        let total_len = columns.iter().try_fold(0usize, |total, column| {
            total
                .checked_add(column.len())
                .ok_or(GpuError::InvalidInput(
                    "Metal pooled column buffer length exceeds platform limits",
                ))
        })?;
        let mut buffer = Self::zeroed(total_len)?;
        let mut offset = 0usize;
        for column in columns {
            buffer.copy_from_slice_at(offset, column);
            offset += column.len();
        }
        Ok(buffer)
    }
    fn from_slice(elements: &[u64]) -> MetalResult<Self> {
        let mut buffer = Self::zeroed(elements.len())?;
        buffer.copy_from_slice_at(0, elements);
        Ok(buffer)
    }
    fn zeroed(len: usize) -> MetalResult<Self> {
        Ok(Self::from_pages(acquire_buffer(len)?, len))
    }
    fn len(&self) -> usize {
        self.backing.logical_len
    }
    fn copy_from_slice_at(&mut self, offset: usize, source: &[u64]) {
        let backing = Arc::get_mut(&mut self.backing)
            .expect("pooled buffer cannot be mutated after Metal retains its backing");
        let end = offset
            .checked_add(source.len())
            .expect("pooled buffer write range overflow");
        assert!(
            end <= backing.logical_len,
            "pooled buffer write out of bounds"
        );
        let mut source_offset = 0usize;
        let mut target_offset = offset;
        while source_offset < source.len() {
            let page_index = target_offset / METAL_BUFFER_PAGE_WORDS;
            let page_offset = target_offset % METAL_BUFFER_PAGE_WORDS;
            let copy_len =
                (source.len() - source_offset).min(METAL_BUFFER_PAGE_WORDS - page_offset);
            backing.pages[page_index].words[page_offset..page_offset + copy_len]
                .copy_from_slice(&source[source_offset..source_offset + copy_len]);
            source_offset += copy_len;
            target_offset += copy_len;
        }
    }
    fn copy_range_to_slice(&self, offset: usize, destination: &mut [u64]) {
        let end = offset
            .checked_add(destination.len())
            .expect("pooled buffer read range overflow");
        assert!(
            end <= self.backing.logical_len,
            "pooled buffer read out of bounds"
        );
        let mut destination_offset = 0usize;
        let mut source_offset = offset;
        while destination_offset < destination.len() {
            let page_index = source_offset / METAL_BUFFER_PAGE_WORDS;
            let page_offset = source_offset % METAL_BUFFER_PAGE_WORDS;
            let copy_len =
                (destination.len() - destination_offset).min(METAL_BUFFER_PAGE_WORDS - page_offset);
            destination[destination_offset..destination_offset + copy_len].copy_from_slice(
                &self.backing.pages[page_index].words[page_offset..page_offset + copy_len],
            );
            destination_offset += copy_len;
            source_offset += copy_len;
        }
    }
    fn copy_to_slice(&self, destination: &mut [u64]) {
        assert_eq!(
            destination.len(),
            self.backing.logical_len,
            "pooled buffer destination length mismatch"
        );
        self.copy_range_to_slice(0, destination);
    }
    fn to_vec(&self) -> MetalResult<Vec<u64>> {
        let mut words = Vec::new();
        words
            .try_reserve_exact(self.backing.logical_len)
            .map_err(|_| {
                GpuError::InvalidInput("Metal output copy exceeds available host memory")
            })?;
        words.resize(self.backing.logical_len, 0);
        self.copy_to_slice(&mut words);
        Ok(words)
    }
    fn word(&self, index: usize) -> u64 {
        assert!(
            index < self.backing.logical_len,
            "pooled buffer read out of bounds"
        );
        let page_index = index / METAL_BUFFER_PAGE_WORDS;
        let page_offset = index % METAL_BUFFER_PAGE_WORDS;
        self.backing.pages[page_index].words[page_offset]
    }
    fn metal_region(&mut self) -> (*const c_void, u64) {
        let backing = Arc::get_mut(&mut self.backing)
            .expect("pooled buffer cannot be shared with Metal more than once");
        let byte_len = backing
            .pages
            .len()
            .checked_mul(mem::size_of::<MetalBufferPage>())
            .and_then(|len| u64::try_from(len).ok())
            .expect("Metal shared buffer length must fit into u64");
        (backing.pages.as_mut_ptr().cast(), byte_len)
    }
    fn backing(&self) -> Arc<PooledBufferBacking> {
        Arc::clone(&self.backing)
    }
    #[cfg(test)]
    fn weak_backing_for_tests(&self) -> std::sync::Weak<PooledBufferBacking> {
        Arc::downgrade(&self.backing)
    }
}
struct CommandSemaphoreState {
    limit: usize,
    queue_floor: usize,
    auto_limit: usize,
    override_limit: Option<usize>,
    source: CommandLimitSource,
    gpu_cores: Option<usize>,
    cpu_parallelism: Option<usize>,
}
struct CommandLimitComputation {
    limit: usize,
    source: CommandLimitSource,
    gpu_cores: Option<usize>,
    cpu_parallelism: Option<usize>,
}
fn command_semaphore() -> &'static CommandSemaphore {
    COMMAND_SEMAPHORE.get_or_init(|| {
        let queue_floor = resolved_queue_floor();
        let auto = auto_in_flight_limit(queue_floor);
        let mut override_limit = max_in_flight_override();
        let limit = match override_limit {
            Some(value) if value < queue_floor => {
                warn!(
                    target: "fastpq::metal",
                    override_limit = value,
                    queue_floor,
                    "FASTPQ_METAL_MAX_IN_FLIGHT override below queue floor; clamping"
                );
                override_limit = Some(queue_floor);
                queue_floor
            }
            Some(value) => value,
            None => auto.limit,
        };
        let _ = COMMAND_SEMAPHORE_STATE.get_or_init(|| CommandSemaphoreState {
            limit,
            queue_floor,
            auto_limit: auto.limit,
            override_limit,
            source: auto.source,
            gpu_cores: auto.gpu_cores,
            cpu_parallelism: auto.cpu_parallelism,
        });
        CommandSemaphore::new(limit.max(1))
    })
}
fn command_limit_snapshot() -> Option<CommandLimitSnapshot> {
    COMMAND_SEMAPHORE_STATE
        .get()
        .map(|state| CommandLimitSnapshot {
            limit: state.limit.try_into().unwrap_or(u32::MAX),
            queue_floor: state.queue_floor.try_into().unwrap_or(u32::MAX),
            auto_limit: state.auto_limit.try_into().unwrap_or(u32::MAX),
            source: state.source,
            gpu_cores: state.gpu_cores.and_then(|v| v.try_into().ok()),
            cpu_parallelism: state.cpu_parallelism.and_then(|v| v.try_into().ok()),
            override_limit: state.override_limit.and_then(|v| v.try_into().ok()),
        })
}
fn max_in_flight_override() -> Option<usize> {
    if let Some(value) = overrides::metal_max_in_flight_override() {
        return Some(value);
    }
    *MAX_IN_FLIGHT_ENV_OVERRIDE.get_or_init(|| {
        debug_env_var("FASTPQ_METAL_MAX_IN_FLIGHT").and_then(|raw| match raw.trim().parse::<usize>() {
            Ok(value) if value > 0 => {
                debug!(
                    target: "fastpq::metal",
                    max_in_flight = value,
                    "overriding Metal command buffer in-flight limit (prefer fastpq.metal_max_in_flight)"
                );
                Some(value)
            }
            Ok(_) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    "FASTPQ_METAL_MAX_IN_FLIGHT must be greater than zero; ignoring override"
                );
                None
            }
            Err(error) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    %error,
                    "failed to parse FASTPQ_METAL_MAX_IN_FLIGHT; ignoring override"
                );
                None
            }
        })
    })
}
fn auto_in_flight_limit(queue_floor: usize) -> CommandLimitComputation {
    if let Some(cores) = gpu_core_count() {
        let limit = limit_from_gpu_cores(cores).max(queue_floor);
        debug!(
            target: "fastpq::metal",
            max_in_flight = limit,
            gpu_cores = cores,
            queue_floor,
            "auto Metal command buffer limit resolved from GPU core count"
        );
        return CommandLimitComputation {
            limit,
            source: CommandLimitSource::GpuCores,
            gpu_cores: Some(cores),
            cpu_parallelism: None,
        };
    }
    let (cpus, cpu_parallelism, source) = match thread::available_parallelism().map(|n| n.get()) {
        Ok(value) => (value, Some(value), CommandLimitSource::CpuParallelism),
        Err(_) => (
            DEFAULT_MAX_COMMAND_BUFFERS,
            None,
            CommandLimitSource::Fallback,
        ),
    };
    let limit = default_in_flight_limit_for_parallelism(cpus).max(queue_floor);
    debug!(
        target: "fastpq::metal",
        max_in_flight = limit,
        cpus,
        queue_floor,
        "auto Metal command buffer limit resolved from host parallelism"
    );
    CommandLimitComputation {
        limit,
        source,
        gpu_cores: None,
        cpu_parallelism,
    }
}
fn limit_from_gpu_cores(cores: usize) -> usize {
    let safe = cores.max(1);
    let half = (safe + 1) / 2;
    half.clamp(2, 16)
}
fn gpu_core_count() -> Option<usize> {
    *GPU_CORE_COUNT.get_or_init(detect_gpu_core_count)
}
fn detect_gpu_core_count() -> Option<usize> {
    let output = Command::new("system_profiler")
        .arg("SPDisplaysDataType")
        .arg("-json")
        .stderr(Stdio::null())
        .output()
        .map_err(|err| {
            debug!(
                target: "fastpq::metal",
                %err,
                "system_profiler unavailable; skipping GPU core detection"
            )
        })
        .ok()?;
    if !output.status.success() {
        debug!(
            target: "fastpq::metal",
            status = ?output.status.code(),
            "system_profiler reported failure; skipping GPU core detection"
        );
        return None;
    }
    let payload = String::from_utf8(output.stdout).ok()?;
    let cores = parse_gpu_core_count(&payload)?;
    debug!(
        target: "fastpq::metal",
        gpu_cores = cores,
        "detected GPU core count from system_profiler"
    );
    Some(cores)
}
fn parse_gpu_core_count(payload: &str) -> Option<usize> {
    let value: Value = json::from_str(payload).ok()?;
    let entries = value.get("SPDisplaysDataType").and_then(Value::as_array)?;
    for entry in entries {
        if let Some(count) = entry
            .get("sppci_cores")
            .and_then(value_to_usize)
            .or_else(|| entry.get("spdisplays_cores").and_then(value_to_usize))
        {
            return Some(count);
        }
    }
    None
}
fn value_to_usize(value: &Value) -> Option<usize> {
    match value {
        Value::Number(num) => num.as_u64().and_then(|n| n.try_into().ok()),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}
fn default_in_flight_limit_for_parallelism(cpus: usize) -> usize {
    let safe_cpus = cpus.max(1);
    let half = (safe_cpus + 1) / 2;
    half.clamp(2, 16)
}
struct CommandSemaphore {
    limit: usize,
    state: Mutex<usize>,
    condvar: Condvar,
}
impl CommandSemaphore {
    fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            state: Mutex::new(0),
            condvar: Condvar::new(),
        }
    }
    fn acquire_timeout(&self, timeout: Duration) -> bool {
        let started = Instant::now();
        let mut guard = self.state.lock().expect("command semaphore poisoned");
        while *guard >= self.limit {
            let elapsed = started.elapsed();
            let Some(remaining) = timeout.checked_sub(elapsed) else {
                return false;
            };
            let (next_guard, result) = self
                .condvar
                .wait_timeout(guard, remaining)
                .expect("command semaphore wait failed");
            guard = next_guard;
            if result.timed_out() && *guard >= self.limit {
                return false;
            }
        }
        *guard += 1;
        true
    }
    fn release(&self) {
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *guard == 0 {
            return;
        }
        *guard -= 1;
        self.condvar.notify_one();
    }
    fn limit(&self) -> usize {
        self.limit
    }
    #[cfg(test)]
    fn in_flight_for_tests(&self) -> usize {
        *self.state.lock().expect("command semaphore poisoned")
    }
}
fn resolved_queue_floor() -> usize {
    let fanout = context_queue_fanout().unwrap_or(1);
    fanout.saturating_mul(2).max(2)
}
fn context_queue_fanout() -> Option<usize> {
    METAL_CONTEXT
        .get()
        .and_then(|result| result.as_ref().ok())
        .map(|context| context.queues.policy().fanout())
}
struct CommandPermit {
    completion: Arc<CommandPermitCompletion>,
}
impl CommandPermit {
    fn try_new(queue_index: usize) -> MetalResult<Self> {
        let semaphore = command_semaphore();
        if !semaphore.acquire_timeout(METAL_COMMAND_PERMIT_TIMEOUT) {
            let snapshot = command_limit_snapshot();
            return Err(GpuError::Execution {
                backend: GpuBackend::Metal,
                message: format!(
                    "timed out waiting {METAL_COMMAND_PERMIT_TIMEOUT:?} for Metal command permit \
                     on queue {queue_index}; limit={}",
                    snapshot.as_ref().map_or_else(
                        || semaphore.limit().to_string(),
                        |state| { state.limit.to_string() }
                    )
                ),
            });
        }
        Ok(Self {
            completion: Arc::new(CommandPermitCompletion::new(semaphore, queue_index)),
        })
    }
    fn completion(&self) -> Arc<CommandPermitCompletion> {
        Arc::clone(&self.completion)
    }
    fn mark_launched(&mut self) {
        self.completion.mark_launched();
    }
    fn complete(&mut self) {
        self.completion.complete();
    }
}
impl Drop for CommandPermit {
    fn drop(&mut self) {
        // A committed command buffer owns a completion-handler clone. Releasing its permit here
        // would let a timed-out or partially submitted batch exceed the configured in-flight cap
        // while Metal is still executing it. Unlaunched permits have no callback and must be
        // returned immediately.
        if !self.completion.is_launched() {
            self.complete();
        }
    }
}
struct CommandPermitCompletion {
    semaphore: &'static CommandSemaphore,
    queue_index: usize,
    launched: AtomicBool,
    released: AtomicBool,
}
impl CommandPermitCompletion {
    fn new(semaphore: &'static CommandSemaphore, queue_index: usize) -> Self {
        Self {
            semaphore,
            queue_index,
            launched: AtomicBool::new(false),
            released: AtomicBool::new(false),
        }
    }
    fn mark_launched(&self) {
        if self
            .launched
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            record_queue_launch(self.queue_index);
        }
    }
    fn is_launched(&self) -> bool {
        self.launched.load(Ordering::Acquire)
    }
    fn complete(&self) {
        if self.launched.swap(false, Ordering::AcqRel) {
            record_queue_completion(self.queue_index);
        }
        if !self.released.swap(true, Ordering::AcqRel) {
            self.semaphore.release();
        }
    }
}
#[derive(Default)]
struct QueueStatsState {
    in_flight: u32,
    max_in_flight: u32,
    dispatch_count: u32,
    last_event: Option<Instant>,
    busy: Duration,
    overlap: Duration,
    window: Duration,
    lanes: Vec<QueueLaneState>,
}
#[derive(Clone, Default)]
struct QueueLaneState {
    in_flight: u32,
    max_in_flight: u32,
    dispatch_count: u32,
    busy: Duration,
    overlap: Duration,
}
impl QueueStatsState {
    fn reset(&mut self) {
        self.in_flight = 0;
        self.max_in_flight = 0;
        self.dispatch_count = 0;
        self.last_event = None;
        self.busy = Duration::default();
        self.overlap = Duration::default();
        self.window = Duration::default();
        for lane in &mut self.lanes {
            lane.in_flight = 0;
            lane.max_in_flight = 0;
            lane.dispatch_count = 0;
            lane.busy = Duration::default();
            lane.overlap = Duration::default();
        }
    }
    fn advance(&mut self, now: Instant) {
        if let Some(previous) = self.last_event {
            let delta = now.saturating_duration_since(previous);
            self.window += delta;
            if self.in_flight > 0 {
                self.busy += delta;
                if self.in_flight > 1 {
                    self.overlap += delta;
                }
            }
            for lane in &mut self.lanes {
                if lane.in_flight > 0 {
                    lane.busy += delta;
                    if lane.in_flight > 1 {
                        lane.overlap += delta;
                    }
                }
            }
        }
        self.last_event = Some(now);
    }
    fn record_launch(&mut self, queue_index: usize, now: Instant) {
        self.advance(now);
        self.in_flight = self.in_flight.saturating_add(1);
        self.dispatch_count = self.dispatch_count.saturating_add(1);
        if self.in_flight > self.max_in_flight {
            self.max_in_flight = self.in_flight;
        }
        let lane = self.lane_mut(queue_index);
        lane.in_flight = lane.in_flight.saturating_add(1);
        lane.dispatch_count = lane.dispatch_count.saturating_add(1);
        if lane.in_flight > lane.max_in_flight {
            lane.max_in_flight = lane.in_flight;
        }
    }
    fn record_completion(&mut self, queue_index: usize, now: Instant) {
        self.advance(now);
        if self.in_flight > 0 {
            self.in_flight -= 1;
        }
        let lane = self.lane_mut(queue_index);
        if lane.in_flight > 0 {
            lane.in_flight -= 1;
        }
    }
    fn snapshot(&self, limit: usize) -> QueueDepthStats {
        let queues = self
            .lanes
            .iter()
            .enumerate()
            .map(|(index, lane)| QueueLaneStats {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                dispatch_count: lane.dispatch_count,
                max_in_flight: lane.max_in_flight,
                busy_ms: elapsed_ms(lane.busy),
                overlap_ms: elapsed_ms(lane.overlap),
            })
            .collect();
        QueueDepthStats {
            limit: u32::try_from(limit).unwrap_or(u32::MAX),
            dispatch_count: self.dispatch_count,
            max_in_flight: self.max_in_flight,
            busy_ms: elapsed_ms(self.busy),
            overlap_ms: elapsed_ms(self.overlap),
            window_ms: elapsed_ms(self.window),
            queues,
        }
    }
    fn lane_mut(&mut self, index: usize) -> &mut QueueLaneState {
        if index >= self.lanes.len() {
            self.lanes.resize_with(index + 1, QueueLaneState::default);
        }
        &mut self.lanes[index]
    }
}
fn record_queue_launch(queue_index: usize) {
    if !QUEUE_STATS_ENABLED.load(Ordering::Acquire) {
        return;
    }
    let store = QUEUE_STATS.get_or_init(|| Mutex::new(QueueStatsState::default()));
    if let Ok(mut guard) = store.lock() {
        guard.record_launch(queue_index, Instant::now());
    }
}
fn record_queue_completion(queue_index: usize) {
    if !QUEUE_STATS_ENABLED.load(Ordering::Acquire) {
        return;
    }
    let store = QUEUE_STATS.get_or_init(|| Mutex::new(QueueStatsState::default()));
    if let Ok(mut guard) = store.lock() {
        guard.record_completion(queue_index, Instant::now());
    }
}
fn threadgroup_override() -> Option<u64> {
    if let Some(value) = overrides::metal_threadgroup_override() {
        return Some(value);
    }
    *THREADGROUP_ENV_OVERRIDE.get_or_init(|| {
        debug_env_var("FASTPQ_METAL_THREADGROUP").and_then(|raw| match raw.trim().parse::<u64>() {
            Ok(value) if value > 0 => {
                debug!(
                    target: "fastpq::metal",
                    threadgroup = value,
                    "overriding Metal threadgroup width (prefer fastpq.metal_threadgroup_size)"
                );
                Some(value)
            }
            Ok(_) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    "FASTPQ_METAL_THREADGROUP must be > 0; ignoring override"
                );
                None
            }
            Err(error) => {
                warn!(
                    target: "fastpq::metal",
                    raw,
                    %error,
                    "failed to parse FASTPQ_METAL_THREADGROUP; ignoring override"
                );
                None
            }
        })
    })
}
fn dispatch_trace_enabled() -> bool {
    if let Some(enabled) = overrides::metal_dispatch_trace_override() {
        return enabled;
    }
    *DISPATCH_TRACE_ENV.get_or_init(|| {
        debug_env_bool("FASTPQ_METAL_TRACE")
            .map(|enabled| {
                if enabled {
                    debug!(
                        target: "fastpq::metal",
                        "FASTPQ_METAL_TRACE enabled; prefer fastpq.metal_trace for production runs"
                    );
                }
                enabled
            })
            .unwrap_or(false)
    })
}
fn trace_dispatch_start(
    pipeline_label: &str,
    pipeline: &ComputePipelineState,
    threads: u64,
    threadgroups: &MTLSize,
    threadgroup: &MTLSize,
) {
    if !dispatch_trace_enabled() {
        return;
    }
    let execution_width = pipeline.thread_execution_width();
    let max_threads = pipeline.max_total_threads_per_threadgroup();
    debug!(
        target: "fastpq::metal",
        pipeline = pipeline_label,
        threads,
        threadgroup = threadgroup.width,
        groups = threadgroups.width,
        execution_width,
        max_threads,
        "dispatching Metal kernel"
    );
}
fn trace_dispatch_end_label(label: Option<String>, duration: Duration, success: bool) {
    if !dispatch_trace_enabled() {
        return;
    }
    debug!(
        target: "fastpq::metal",
        pipeline = label,
        duration_us = duration.as_micros(),
        success,
        "Metal kernel completed"
    );
}
fn restore_range(
    columns: &mut [Vec<u64>],
    range: Range<usize>,
    buffer: &PooledBuffer,
    extent: usize,
) {
    if range.is_empty() {
        return;
    }
    for (batch_offset, column) in columns[range].iter_mut().enumerate() {
        buffer.copy_range_to_slice(batch_offset * extent, column);
    }
}
fn bn254_two_adicity() -> u32 {
    Bn254Fr::S
}
fn bn254_validate_log(log_size: u32) -> MetalResult<()> {
    if log_size == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 FFT requires log_size greater than zero",
        ));
    }
    if log_size > bn254_two_adicity() {
        return Err(GpuError::InvalidInput(
            "BN254 FFT exceeds supported two-adicity",
        ));
    }
    Ok(())
}
fn bn254_domain_len(log_size: u32) -> MetalResult<usize> {
    bn254_validate_log(log_size)?;
    1usize.checked_shl(log_size).ok_or(GpuError::InvalidInput(
        "BN254 domain length exceeds platform limits",
    ))
}
fn bn254_lde_domain_lengths(trace_log: u32, blowup_log: u32) -> MetalResult<(usize, u32, usize)> {
    if blowup_log == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 LDE requires a positive blowup factor",
        ));
    }
    let trace_len = bn254_domain_len(trace_log)?;
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE log size exceeds 32-bit representation",
        ))?;
    let eval_len = bn254_domain_len(eval_log)?;
    Ok((trace_len, eval_log, eval_len))
}
fn goldilocks_domain_len(log_size: u32) -> MetalResult<usize> {
    if log_size > GOLDILOCKS_TWO_ADICITY {
        return Err(GpuError::InvalidInput(
            "Goldilocks domain log exceeds two-adicity",
        ));
    }
    1usize.checked_shl(log_size).ok_or(GpuError::InvalidInput(
        "Goldilocks domain length exceeds platform limits",
    ))
}
fn goldilocks_lde_domain_lengths(
    trace_log: u32,
    blowup_log: u32,
) -> MetalResult<(usize, u32, usize)> {
    if blowup_log == 0 {
        return Err(GpuError::InvalidInput(
            "LDE requires a positive blowup factor",
        ));
    }
    let trace_len = goldilocks_domain_len(trace_log)?;
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(GpuError::InvalidInput(
            "LDE log size exceeds 32-bit representation",
        ))?;
    let eval_len = goldilocks_domain_len(eval_log)?;
    Ok((trace_len, eval_log, eval_len))
}
fn bn254_scalar_to_canonical_limbs(value: &Bn254Scalar) -> [u64; BN254_LIMBS] {
    let bytes = value.to_bytes();
    let mut limbs = [0u64; BN254_LIMBS];
    for (index, limb) in limbs.iter_mut().enumerate() {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[index * 8..(index + 1) * 8]);
        *limb = u64::from_le_bytes(buf);
    }
    limbs
}
fn bn254_scalar_from_canonical_limbs(limbs: &[u64; BN254_LIMBS]) -> MetalResult<Bn254Scalar> {
    let mut bytes = [0u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        bytes[index * 8..(index + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    Bn254Scalar::from_bytes(&bytes).map_err(|_| {
        GpuError::InvalidInput("BN254 canonical limbs decode produced invalid field element")
    })
}
fn bn254_limbs_slice_to_scalar(slice: &[u64]) -> MetalResult<Bn254Scalar> {
    let limbs: [u64; BN254_LIMBS] = slice
        .try_into()
        .expect("slice length should equal BN254 limb count");
    bn254_scalar_from_canonical_limbs(&limbs)
}
fn bn254_stage_twiddles_scalars(log_size: u32) -> MetalResult<Vec<Bn254Scalar>> {
    bn254::stage_twiddles_scalars(log_size).map_err(GpuError::InvalidInput)
}
fn bn254_stage_twiddles_limbs(log_size: u32) -> MetalResult<Vec<[u64; BN254_LIMBS]>> {
    bn254::stage_twiddles_limbs(log_size).map_err(GpuError::InvalidInput)
}
fn sample_bn254_columns(log_size: u32, column_count: usize) -> Vec<Vec<u64>> {
    let len = 1usize << log_size;
    let mut columns = Vec::with_capacity(column_count);
    for column in 0..column_count {
        let mut data = Vec::with_capacity(len * BN254_LIMBS);
        for row in 0..len {
            let value = Bn254Scalar::from(((column as u64 + 1) * 31).wrapping_add(row as u64 + 1));
            data.extend_from_slice(&bn254_scalar_to_canonical_limbs(&value));
        }
        columns.push(data);
    }
    columns
}
fn sample_bn254_coset() -> [u64; BN254_LIMBS] {
    bn254_scalar_to_canonical_limbs(&Bn254Scalar::from(5u64))
}
fn bn254_column_extent(columns: &[Vec<u64>]) -> MetalResult<usize> {
    if columns.is_empty() {
        return Ok(0);
    }
    let limb_len = columns[0].len();
    if limb_len % BN254_LIMBS != 0 {
        return Err(GpuError::InvalidInput(
            "BN254 column length must be a multiple of four limbs",
        ));
    }
    if columns.iter().any(|column| column.len() != limb_len) {
        return Err(GpuError::InvalidInput(
            "BN254 columns must share the same limb length",
        ));
    }
    Ok(limb_len / BN254_LIMBS)
}
fn goldilocks_mul(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let reduced = product % u128::from(FIELD_MODULUS);
    u64::try_from(reduced).expect("Goldilocks reduction fits in u64")
}
fn goldilocks_pow(mut base: u64, mut exponent: u64) -> u64 {
    let mut result = 1u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = goldilocks_mul(result, base);
        }
        base = goldilocks_mul(base, base);
        exponent >>= 1;
    }
    result
}
fn goldilocks_inv(value: u64) -> u64 {
    goldilocks_pow(value, FIELD_MODULUS - 2)
}
fn compute_stage_twiddles(log_len: u32, root: u64, inverse: bool) -> Vec<u64> {
    if log_len == 0 {
        return Vec::new();
    }
    let len = 1u64 << log_len;
    let mut omega = root;
    if inverse {
        omega = goldilocks_inv(omega);
    }
    let mut twiddles = Vec::with_capacity(log_len as usize);
    for stage in 0..log_len {
        let size = 1u64 << (stage + 1);
        let step = len / size;
        twiddles.push(goldilocks_pow(omega, step));
    }
    twiddles
}
#[cfg(test)]
mod helper_tests {
    use super::{
        AdaptiveScheduler, MAX_QUEUE_FANOUT, QueuePolicy, STATE_WIDTH,
        default_queue_column_threshold, lde_tile_stage_limit, parse_queue_fanout_override,
        parse_queue_threshold_override, poseidon_element_range,
        poseidon_recommended_states_per_batch, post_tile_stage_start, queue_total_columns_hint,
        select_poseidon_batch, select_poseidon_batch_with_scheduler,
    };
    use crate::metal_config::{self, DeviceHints};
    #[test]
    fn post_tile_stage_start_only_dispatches_when_needed() {
        assert_eq!(post_tile_stage_start(10, 4), Some(4));
        assert_eq!(post_tile_stage_start(8, 16), None);
        assert_eq!(post_tile_stage_start(0, 4), None);
    }
    #[test]
    fn lde_tile_stage_limit_scales_with_log_size() {
        let _hint_guard = metal_config::device_hints_test_guard();
        assert_eq!(lde_tile_stage_limit(5), 5);
        assert_eq!(lde_tile_stage_limit(18), 8);
        assert_eq!(lde_tile_stage_limit(64), 8);
    }
    #[test]
    fn lde_tile_stage_limit_respects_device_hints() {
        let _hint_guard = metal_config::device_hints_test_guard();
        metal_config::set_device_hints_for_tests(Some(DeviceHints::new(
            false,
            true,
            true,
            24 * 1024 * 1024 * 1024,
        )));
        assert_eq!(lde_tile_stage_limit(18), 8);
    }
    #[test]
    fn queue_policy_round_robins_above_threshold() {
        let policy = QueuePolicy::new(3, 8);
        let below = policy.select_index(4, 5);
        assert_eq!(below, 0, "fan-out should not engage below threshold");
        let indices: Vec<_> = (0..6).map(|idx| policy.select_index(16, idx)).collect();
        assert_eq!(indices, vec![0, 1, 2, 0, 1, 2]);
    }
    #[test]
    fn queue_policy_clamps_requested_values() {
        let policy = QueuePolicy::new(0, 0);
        assert_eq!(policy.fanout(), 1);
        assert_eq!(policy.column_threshold(), 1);
        let capped = QueuePolicy::new(MAX_QUEUE_FANOUT + 10, 4);
        assert_eq!(capped.fanout(), MAX_QUEUE_FANOUT);
        assert_eq!(capped.column_threshold(), 4);
    }
    #[test]
    fn queue_fanout_override_validation() {
        assert_eq!(parse_queue_fanout_override("2").unwrap(), 2);
        assert!(parse_queue_fanout_override("0").is_err());
        assert!(parse_queue_fanout_override("abc").is_err());
    }
    #[test]
    fn queue_threshold_override_validation() {
        assert_eq!(parse_queue_threshold_override("12").unwrap(), 12);
        assert!(parse_queue_threshold_override("0").is_err());
        assert!(parse_queue_threshold_override("abc").is_err());
    }
    #[test]
    fn default_queue_threshold_scales_with_fanout() {
        assert_eq!(default_queue_column_threshold(1), u32::MAX);
        assert_eq!(default_queue_column_threshold(2), 16);
        assert_eq!(default_queue_column_threshold(3), 24);
    }
    #[test]
    fn inverse_fft_hint_disables_threshold_fanout() {
        let policy = QueuePolicy::new(2, 16);
        assert_eq!(queue_total_columns_hint(16, true, &policy), 15);
        assert_eq!(queue_total_columns_hint(15, true, &policy), 15);
        assert_eq!(queue_total_columns_hint(32, true, &policy), 32);
        assert_eq!(queue_total_columns_hint(16, false, &policy), 16);
    }
    #[test]
    fn poseidon_recommended_batch_respects_caps() {
        let tuning = metal_config::PoseidonTuning {
            threadgroup_lanes: 64,
            states_per_lane: 4,
        };
        assert_eq!(poseidon_recommended_states_per_batch(0, tuning), 0);
        assert_eq!(poseidon_recommended_states_per_batch(1, tuning), 1);
        let target = tuning
            .threadgroup_lanes
            .saturating_mul(tuning.states_per_lane)
            .saturating_mul(metal_config::poseidon_batch_multiplier());
        let recommended = poseidon_recommended_states_per_batch(target * 2, tuning);
        let base = tuning
            .threadgroup_lanes
            .saturating_mul(tuning.states_per_lane);
        let max_expected = base.saturating_mul(4);
        assert!(recommended >= base);
        assert!(recommended <= max_expected);
    }
    #[test]
    fn poseidon_batch_selection_respects_remaining_states() {
        let tuning = metal_config::PoseidonTuning {
            threadgroup_lanes: 64,
            states_per_lane: 4,
        };
        let total_states = 32;
        let selection = select_poseidon_batch(total_states, tuning);
        assert!((1..=total_states).contains(&selection.columns()));
        let sample = selection.sample_for(selection.columns());
        assert!(sample.is_some(), "adaptive sample expected");
    }
    #[test]
    fn poseidon_batch_selection_clamps_shared_state_to_current_safe_cap() {
        let scheduler = AdaptiveScheduler::new();
        let seeded = scheduler.select_poseidon(4_096, 4_096);
        assert_eq!(seeded.columns(), 4_096);
        let tuning = metal_config::PoseidonTuning {
            threadgroup_lanes: 2,
            states_per_lane: 1,
        };
        let state_count = 4_096;
        let recommended = poseidon_recommended_states_per_batch(state_count, tuning);
        let selection = select_poseidon_batch_with_scheduler(&scheduler, state_count, tuning);
        assert_eq!(selection.max_columns, recommended);
        assert!(selection.columns() <= recommended);
    }
    #[test]
    fn poseidon_element_range_scales_with_state_width() {
        let range = poseidon_element_range(2, 3).expect("range");
        assert_eq!(range.start, 2 * STATE_WIDTH);
        assert_eq!(range.end, 5 * STATE_WIDTH);
    }
}
#[cfg(test)]
mod bn254_helper_tests {
    use super::*;
    use metal::Device;
    #[test]
    fn upload_bn254_twiddles_rejects_non_limb_multiple() {
        if Device::system_default().is_none() {
            return;
        }
        let device = Device::system_default().expect("device");
        let err = upload_bn254_twiddles(&device, &[1u64, 2, 3]).expect_err("expected invalid");
        assert!(matches!(err, GpuError::InvalidInput(_)));
    }
    #[test]
    fn upload_bn254_twiddles_rejects_an_empty_metal_buffer() {
        let Some(device) = Device::system_default() else {
            return;
        };
        let err = upload_bn254_twiddles(&device, &[]).expect_err("expected empty rejection");
        assert!(matches!(err, GpuError::InvalidInput(_)));
    }
    #[test]
    fn flatten_bn254_twiddles_concatenates_limbs() {
        let inputs = [[1u64, 2, 3, 4], [5, 6, 7, 8]];
        let flat = super::flatten_bn254_twiddles(&inputs).expect("flatten twiddles");
        assert_eq!(flat, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
    #[test]
    fn upload_bn254_coset_requires_four_limbs() {
        if Device::system_default().is_none() {
            return;
        }
        let device = Device::system_default().expect("device");
        let err = upload_bn254_coset(&device, &[1u64, 2, 3]).expect_err("expected invalid");
        assert!(matches!(err, GpuError::InvalidInput(_)));
    }
    #[test]
    fn validate_bn254_twiddles_shape_checks_length() {
        let ok = super::validate_bn254_twiddles_shape(2, &[[0u64; 4]; 3]).is_ok();
        assert!(ok, "expected shape to be valid");
        let err = super::validate_bn254_twiddles_shape(2, &[[0u64; 4]; 4])
            .expect_err("expected shape error");
        assert!(matches!(err, GpuError::InvalidInput(_)));
    }
    #[test]
    fn bn254_twiddle_len_helpers_match_shape() {
        assert_eq!(super::bn254_fft_twiddle_len(2).unwrap(), 3);
        assert!(super::bn254_fft_twiddle_len(0).is_err());
        assert_eq!(super::bn254_lde_twiddle_len(2, 1).unwrap(), 7);
        assert!(super::bn254_lde_twiddle_len(0, 1).is_err());
        assert!(super::bn254_lde_twiddle_len(2, 0).is_err());
    }
    #[test]
    fn bn254_twiddle_len_helpers_reject_oversized_logs_without_panicking() {
        assert!(matches!(
            super::bn254_fft_twiddle_len(u32::MAX),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            super::bn254_lde_twiddle_len(u32::MAX, 1),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            super::bn254_lde_twiddle_len(1, u32::MAX),
            Err(GpuError::InvalidInput(_))
        ));
    }
    #[test]
    fn stage_bn254_twiddles_rejects_zero_log() {
        if Device::system_default().is_none() {
            return;
        }
        let device = Device::system_default().expect("device");
        let err = super::stage_bn254_twiddles(&device, 0).expect_err("expected log_size rejection");
        assert!(matches!(err, GpuError::InvalidInput(_)));
    }
    #[test]
    fn stage_bn254_twiddles_matches_expected_size() {
        if Device::system_default().is_none() {
            return;
        }
        let device = Device::system_default().expect("device");
        let log_size = 3;
        let buffer = super::stage_bn254_twiddles(&device, log_size).expect("twiddles");
        let expected_twiddles = super::bn254_fft_twiddle_len(log_size).unwrap();
        let expected_bytes = expected_twiddles * BN254_LIMBS * std::mem::size_of::<u64>();
        assert_eq!(buffer.length() as usize, expected_bytes);
    }
    #[test]
    fn bn254_status_runs_smoke_checks() {
        if Device::system_default().is_none() {
            return;
        }
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        match super::bn254_status() {
            Ok(()) => {}
            Err(GpuError::Unsupported(_)) => return,
            Err(err) => panic!("BN254 status smoke test failed: {err}"),
        }
    }
}
#[cfg(all(test, feature = "fastpq-gpu", target_os = "macos"))]
mod tests {
    use super::{ensure_multi_queue_env, unwrap_or_skip, *};
    use crate::fft::Planner;
    use fastpq_isi::{CANONICAL_PARAMETER_SETS, poseidon as cpu_poseidon};
    use iroha_crypto::Hash;
    use std::{thread, time::Duration};
    const TRACE_NODE_DOMAIN_FOR_TESTS: &[u8] = b"fastpq:v1:trace:node";
    const REQUIRED_PIPELINES: &[&str] = &[
        POSEIDON_PERMUTE_KERNEL,
        POSEIDON_HASH_KERNEL,
        POSEIDON_HASH_ROWS_KERNEL,
        POSEIDON_TRACE_FUSED_KERNEL,
        POSEIDON_TRACE_PARENTS_KERNEL,
        FFT_KERNEL,
        LDE_KERNEL,
        POST_TILE_KERNEL,
        BN254_FFT_KERNEL,
        BN254_LDE_KERNEL,
        BN254_POSEIDON_HASH_KERNEL,
    ];
    #[test]
    fn embedded_metal_source_is_self_contained() {
        let source = embedded_metal_library_source();
        assert!(
            !source
                .lines()
                .any(|line| line.trim_start().starts_with("#include \"")),
            "runtime Metal source must not depend on repository-relative includes"
        );
        for name in REQUIRED_PIPELINES {
            assert!(
                source.contains(&format!("kernel void {name}")),
                "runtime Metal source is missing {name}"
            );
        }
    }
    #[test]
    fn metal_library_resolution_fails_closed_only_for_explicit_override() {
        let missing = "/definitely/missing/fastpq.metallib";
        assert_eq!(
            resolve_metal_library_path_candidates(Some(missing.to_owned()), None).as_deref(),
            Some(missing),
            "an invalid explicit override must reach the loader and report an error"
        );
        assert_eq!(
            resolve_metal_library_path_candidates(None, Some(missing)),
            None,
            "a stale build-time path must select embedded source fallback"
        );
    }
    #[test]
    fn embedded_metal_source_builds_every_required_pipeline() {
        let Some(device) = select_metal_device() else {
            return;
        };
        let library = compile_embedded_metal_library(&device)
            .expect("embedded Metal source should compile on a visible device");
        for name in REQUIRED_PIPELINES {
            load_pipeline(&device, &library, name)
                .unwrap_or_else(|error| panic!("embedded Metal pipeline {name} failed: {error}"));
        }
    }
    #[test]
    fn zero_log_goldilocks_fft_and_ifft_are_identity_without_dispatch() {
        let original = vec![vec![3], vec![7]];
        let mut columns = original.clone();
        fft_columns_async(&mut columns, 0, 1)
            .expect("length-one FFT should be accepted")
            .wait()
            .expect("identity FFT wait should succeed");
        assert_eq!(columns, original);

        ifft_columns_async(&mut columns, 0, 1)
            .expect("length-one IFFT should be accepted")
            .wait()
            .expect("identity IFFT wait should succeed");
        assert_eq!(columns, original);
    }
    #[test]
    fn oversized_metal_domain_logs_return_invalid_input_before_device_setup() {
        let mut columns = vec![vec![1]];
        assert!(matches!(
            fft_columns_async(&mut columns, u32::MAX, 1),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            ifft_columns_async(&mut columns, u32::MAX, 1),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            fft_tuning_snapshot(u32::MAX),
            Err(GpuError::InvalidInput(_))
        ));

        let coeffs = vec![vec![1]];
        assert!(matches!(
            lde_columns_async(&coeffs, u32::MAX, 1, 1, 1),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            lde_columns_async(&coeffs, 0, u32::MAX, 1, 1),
            Err(GpuError::InvalidInput(_))
        ));

        let mut bn254_columns = vec![vec![0; BN254_LIMBS]];
        assert!(matches!(
            bn254_fft_columns_async(&mut bn254_columns, u32::MAX),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            bn254_lde_columns_async(&bn254_columns, u32::MAX, 1, [0; BN254_LIMBS]),
            Err(GpuError::InvalidInput(_))
        ));
    }
    #[test]
    fn empty_metal_inputs_still_validate_domain_parameters() {
        let mut columns = Vec::<Vec<u64>>::new();
        assert!(matches!(
            fft_columns(&mut columns, u32::MAX, 1),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            ifft_columns(&mut columns, u32::MAX, 1),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            lde_columns(&columns, u32::MAX, 1, 1, 1),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            lde_columns(&columns, 0, 0, 1, 1),
            Err(GpuError::InvalidInput(_))
        ));

        assert!(matches!(
            bn254_fft_columns(&mut columns, u32::MAX),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            bn254_fft_columns_async(&mut columns, 0),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            bn254_lde_columns(&columns, 1, 0, [0; BN254_LIMBS]),
            Err(GpuError::InvalidInput(_))
        ));
        assert!(matches!(
            bn254_lde_columns_async(&columns, u32::MAX, 1, [0; BN254_LIMBS]),
            Err(GpuError::InvalidInput(_))
        ));
    }
    #[test]
    fn valid_empty_metal_inputs_complete_without_device_setup() {
        let mut columns = Vec::<Vec<u64>>::new();
        fft_columns(&mut columns, 0, 1).expect("empty Goldilocks FFT should be a no-op");
        ifft_columns(&mut columns, 0, 1).expect("empty Goldilocks IFFT should be a no-op");
        assert_eq!(
            lde_columns(&columns, 0, 1, 1, 1).expect("empty Goldilocks LDE should succeed"),
            Some(Vec::new())
        );

        bn254_fft_columns(&mut columns, 1).expect("empty BN254 FFT should be a no-op");
        bn254_fft_columns_async(&mut columns, 1)
            .expect("empty BN254 async FFT should be accepted")
            .wait()
            .expect("empty BN254 async FFT wait should succeed");
        assert_eq!(
            bn254_lde_columns(&columns, 1, 1, [0; BN254_LIMBS])
                .expect("empty BN254 LDE should succeed"),
            Some(Vec::new())
        );
        assert_eq!(
            bn254_lde_columns_async(&columns, 1, 1, [0; BN254_LIMBS])
                .expect("empty BN254 async LDE should be accepted")
                .wait()
                .expect("empty BN254 async LDE wait should succeed"),
            Some(Vec::new())
        );
    }
    #[test]
    fn goldilocks_lde_rejects_zero_blowup_before_device_setup() {
        let coeffs = vec![vec![1, 2]];
        assert!(matches!(
            lde_columns_async(&coeffs, 1, 0, 1, 1),
            Err(GpuError::InvalidInput(_))
        ));
    }
    #[test]
    fn bn254_transforms_reject_noncanonical_coefficients_before_device_setup() {
        let mut fft_columns = vec![vec![u64::MAX; BN254_LIMBS * 2]];
        assert!(matches!(
            bn254_fft_columns_async(&mut fft_columns, 1),
            Err(GpuError::InvalidInput(_))
        ));

        let lde_columns = vec![vec![u64::MAX; BN254_LIMBS * 2]];
        assert!(matches!(
            bn254_lde_columns_async(&lde_columns, 1, 1, sample_bn254_coset()),
            Err(GpuError::InvalidInput(_))
        ));
    }
    #[test]
    fn bn254_fft_late_batch_failure_restores_every_input_column() {
        if select_metal_device().is_none() {
            return;
        }
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let mut columns = sample_bn254_columns(3, 4);
        let original = columns.clone();
        // Four one-column batches fill both staging slots, commit two prefixes,
        // then inject the failure during PendingColumns::finish.
        let _failure = fail_column_batch_wait_after(2);
        let error = bn254_fft_columns(&mut columns, 3).expect_err("injected failure expected");
        assert!(
            error
                .to_string()
                .contains("injected column batch wait failure")
        );
        assert_eq!(columns, original);
    }
    #[test]
    fn bn254_fft_dispatch_loop_failure_restores_every_input_column() {
        if select_metal_device().is_none() {
            return;
        }
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let mut columns = sample_bn254_columns(3, 4);
        let original = columns.clone();
        // The third batch drains the first staging slot successfully; the
        // fourth drains the second and fails while dispatches are still built.
        let _failure = fail_column_batch_wait_after(1);
        let error = bn254_fft_columns(&mut columns, 3).expect_err("injected failure expected");
        assert!(
            error
                .to_string()
                .contains("injected column batch wait failure")
        );
        assert_eq!(columns, original);
    }
    #[test]
    fn poseidon_late_batch_failure_restores_every_input_state() {
        if select_metal_device().is_none() {
            return;
        }
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let mut states = (0..4_096 * STATE_WIDTH)
            .map(|index| index as u64 % FIELD_MODULUS)
            .collect::<Vec<_>>();
        let original = states.clone();
        let _failure = fail_poseidon_batch_wait_after(1);
        let error = poseidon_permute(&mut states).expect_err("injected failure expected");
        assert!(
            error
                .to_string()
                .contains("injected Poseidon batch wait failure")
        );
        assert_eq!(states, original);
    }
    fn sample_fft_columns(log_size: u32, column_count: usize) -> Vec<Vec<u64>> {
        let len = 1usize << log_size;
        (0..column_count)
            .map(|col| {
                (0..len)
                    .map(|idx| {
                        let seed = ((col as u64 + 1) * 0x9e37_79b9)
                            ^ ((idx as u64).wrapping_mul(0x2545_f491_4f6c_dd1d));
                        seed % cpu_poseidon::FIELD_MODULUS
                    })
                    .collect::<Vec<u64>>()
            })
            .collect()
    }
    fn test_domain_seed(domain: &[u8]) -> u64 {
        let digest = Hash::new(domain);
        let bytes = digest.as_ref();
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&bytes[..8]);
        u64::try_from(u128::from(u64::from_le_bytes(chunk)) % u128::from(FIELD_MODULUS))
            .expect("Goldilocks reduction fits u64")
    }
    fn hash_with_domain_for_tests(domain: &[u8], values: &[u64]) -> u64 {
        let mut sponge = cpu_poseidon::PoseidonSponge::new();
        sponge.absorb(test_domain_seed(domain));
        sponge.absorb_slice(values);
        sponge.squeeze()
    }
    #[test]
    fn fft_dispatch_geometry_scales_with_columns() {
        let lanes = 32;
        let (groups, threads, logical) = super::fft_dispatch_geometry(4, lanes);
        assert_eq!(groups.width, 4);
        assert_eq!(threads.width, u64::from(lanes));
        assert_eq!(logical, u64::from(lanes * 4));
    }
    #[test]
    fn fft_and_ifft_match_cpu_reference() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let scenarios = [(3, 2), (10, 2), (14, 1), (18, 1)];
        for (log_size, column_count) in scenarios {
            let mut cpu_columns = sample_fft_columns(log_size, column_count);
            let mut metal_columns = cpu_columns.clone();
            let root = goldilocks_pow(GOLDILOCKS_GENERATOR, (FIELD_MODULUS - 1) >> log_size);
            let domain = crate::cyclotomic::Domain {
                log_size,
                generator: root,
            };
            for column in &mut cpu_columns {
                crate::cyclotomic::fft(column, domain);
            }
            if unwrap_or_skip(
                super::fft_columns(&mut metal_columns, log_size, root),
                "fft",
            )
            .is_none()
            {
                return;
            }
            assert_eq!(cpu_columns, metal_columns);
            for column in &mut cpu_columns {
                crate::cyclotomic::ifft(column, domain);
            }
            if unwrap_or_skip(
                super::ifft_columns(&mut metal_columns, log_size, root),
                "ifft",
            )
            .is_none()
            {
                return;
            }
            assert_eq!(cpu_columns, metal_columns);
        }
    }
    #[test]
    fn lde_matches_cpu_reference() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        // Balanced parameters use blowup_log=3, so this crosses the 256-word
        // threadgroup tile boundary and exercises the post-tile stage.
        let trace_log = 6;
        let trace_len = 1usize << trace_log;
        let coeffs = vec![
            (0..trace_len)
                .map(|idx| {
                    (idx as u64).wrapping_mul(13).wrapping_add(3) % cpu_poseidon::FIELD_MODULUS
                })
                .collect::<Vec<u64>>(),
            (0..trace_len)
                .map(|idx| {
                    (idx as u64).wrapping_mul(23).wrapping_add(17) % cpu_poseidon::FIELD_MODULUS
                })
                .collect::<Vec<u64>>(),
        ];
        let cpu_eval = planner.lde_columns(&coeffs);
        let lde_root = planner
            .lde_domain(trace_log + planner.blowup_log())
            .generator;
        let Some(gpu_eval) = unwrap_or_skip(
            super::lde_columns(
                &coeffs,
                trace_log,
                planner.blowup_log(),
                lde_root,
                params.omega_coset,
            ),
            "lde",
        ) else {
            return;
        };
        let gpu_eval = gpu_eval.expect("Metal backend declined workload");
        assert_eq!(cpu_eval, gpu_eval);
    }
    #[test]
    fn poseidon_matches_cpu_permutation() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let mut cpu_states = Vec::new();
        for idx in 0u64..4 {
            cpu_states.push(idx * 11);
            cpu_states.push(idx * 7 + 3);
            cpu_states.push(idx * 5 + 1);
        }
        let mut metal_states = cpu_states.clone();
        for chunk in cpu_states.chunks_exact_mut(cpu_poseidon::STATE_WIDTH) {
            let mut state = [0u64; cpu_poseidon::STATE_WIDTH];
            state.copy_from_slice(chunk);
            cpu_poseidon::permute_state(&mut state);
            chunk.copy_from_slice(&state);
        }
        if unwrap_or_skip(super::poseidon_permute(&mut metal_states), "poseidon").is_none() {
            return;
        }
        assert_eq!(cpu_states, metal_states);
    }
    #[test]
    fn poseidon_hash_rows_matches_cpu_reference() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let row_count = 64usize;
        let columns = (0..5usize)
            .map(|column| {
                (0..row_count)
                    .map(|row| {
                        ((column as u64 + 3) * 97 + (row as u64 * 13)) % cpu_poseidon::FIELD_MODULUS
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let expected = (0..row_count)
            .map(|row| {
                let mut limbs = Vec::with_capacity(columns.len() + 2);
                limbs.push(row as u64);
                limbs.push(columns.len() as u64);
                for column in &columns {
                    limbs.push(column[row]);
                }
                cpu_poseidon::hash_field_elements(&limbs)
            })
            .collect::<Vec<_>>();
        let Some(actual) =
            unwrap_or_skip(super::poseidon_hash_rows(&columns), "poseidon_hash_rows")
        else {
            return;
        };
        assert_eq!(actual, expected);
    }
    #[test]
    fn poseidon_hash_columns_batches_multi_block_columns() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let domain_names = (0..8usize)
            .map(|idx| format!("fastpq:v1:trace:column:vectorized:{idx}"))
            .collect::<Vec<_>>();
        let domains = domain_names.iter().map(String::as_str).collect::<Vec<_>>();
        let columns = (0..domains.len())
            .map(|column| {
                (0..9usize)
                    .map(|row| {
                        ((column as u64 + 5) * 101 + (row as u64 * 17))
                            % cpu_poseidon::FIELD_MODULUS
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let batch =
            PoseidonColumnBatch::from_domains_and_columns(&domains, &columns).expect("batch");
        assert!(
            batch.block_count() > 1,
            "test batch must exercise multi-block sponge absorption"
        );
        let expected =
            crate::trace::hash_columns_cpu_batch_inputs(&domains, &columns).expect("cpu reference");
        super::adaptive_scheduler()
            .poseidon
            .record_sample(4, domains.len() as u32, 0.0);
        super::enable_kernel_stats(true);
        let Some(actual) = unwrap_or_skip(
            super::poseidon_hash_columns(&batch),
            "poseidon_hash_columns vectorized",
        ) else {
            super::enable_kernel_stats(false);
            return;
        };
        let stats = super::take_kernel_stats().expect("kernel stats enabled");
        super::enable_kernel_stats(false);
        assert_eq!(actual, expected);
        let sample = stats
            .iter()
            .find(|sample| sample.kind.as_str() == "poseidon" && sample.column_count > 1)
            .unwrap_or_else(|| panic!("expected a vectorized Poseidon dispatch, got {stats:?}"));
        let actual_limits = super::PipelineLimits {
            exec_width: sample.execution_width,
            max_threads: sample.max_threads_per_group,
        };
        let mut expected_tuning = crate::metal_config::poseidon_tuning(
            actual_limits.exec_width,
            actual_limits.max_threads,
        );
        expected_tuning.states_per_lane = 1;
        let (_, expected_threadgroup, _, _) =
            super::poseidon_dispatch_geometry(sample.column_count, expected_tuning, &actual_limits);
        assert_eq!(
            sample.threadgroup_width, expected_threadgroup.width,
            "Poseidon column geometry must use the limits of the pipeline that was dispatched"
        );
    }
    #[test]
    fn poseidon_hash_columns_batches_merkle_pairs() {
        ensure_multi_queue_env();
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let pairs = (0..16usize)
            .map(|idx| {
                let left =
                    (idx as u64).wrapping_mul(0xd1b5_4a32_d192_ed03) % cpu_poseidon::FIELD_MODULUS;
                let right = (idx as u64)
                    .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                    .wrapping_add(7)
                    % cpu_poseidon::FIELD_MODULUS;
                [left, right]
            })
            .collect::<Vec<_>>();
        let batch = PoseidonColumnBatch::from_domain_and_pairs(TRACE_NODE_DOMAIN_FOR_TESTS, &pairs)
            .expect("batch");
        let expected = pairs
            .iter()
            .map(|pair| hash_with_domain_for_tests(TRACE_NODE_DOMAIN_FOR_TESTS, pair))
            .collect::<Vec<_>>();
        super::adaptive_scheduler()
            .poseidon
            .record_sample(8, pairs.len() as u32, 0.0);
        super::enable_kernel_stats(true);
        let Some(actual) = unwrap_or_skip(
            super::poseidon_hash_columns(&batch),
            "poseidon_hash_columns merkle pairs",
        ) else {
            super::enable_kernel_stats(false);
            return;
        };
        let stats = super::take_kernel_stats().expect("kernel stats enabled");
        super::enable_kernel_stats(false);
        assert_eq!(actual, expected);
        assert!(
            stats
                .iter()
                .any(|sample| sample.kind.as_str() == "poseidon" && sample.column_count > 1),
            "expected Merkle pair hashing to use a vectorized dispatch, got {stats:?}"
        );
    }
    #[test]
    fn poseidon_dispatch_geometry_uses_actual_work() {
        let limits = super::PipelineLimits {
            exec_width: 32,
            max_threads: 64,
        };
        let tuning = super::metal_config::PoseidonTuning {
            threadgroup_lanes: 32,
            states_per_lane: 4,
        };
        let (groups, group, logical_threads, states_per_lane) =
            super::poseidon_dispatch_geometry(16, tuning, &limits);
        assert_eq!(logical_threads, 4);
        assert_eq!(states_per_lane, 4);
        assert_eq!(group.width, 4);
        assert_eq!(groups.width, 1);
    }
    #[test]
    fn poseidon_tuning_snapshot_reports_effective_parity_shape() {
        if super::select_metal_device().is_none() {
            return;
        }
        let tuning = super::poseidon_tuning_snapshot().expect("Metal Poseidon tuning");
        assert_eq!(tuning.states_per_lane, 1);
    }
    #[test]
    fn bn254_poseidon_dispatch_geometry_uses_actual_work() {
        let limits = super::PipelineLimits {
            exec_width: 32,
            max_threads: 64,
        };
        let tuning = super::metal_config::PoseidonTuning {
            threadgroup_lanes: 32,
            states_per_lane: 4,
        };
        let (groups, group, logical_threads, states_per_lane) =
            super::bn254_poseidon_dispatch_geometry(64, tuning, &limits);
        assert_eq!(logical_threads, 16);
        assert_eq!(states_per_lane, 4);
        assert_eq!(group.width, 16);
        assert_eq!(groups.width, 1);
        let (groups, group, logical_threads, states_per_lane) =
            super::bn254_poseidon_dispatch_geometry(513, tuning, &limits);
        assert_eq!(logical_threads, 129);
        assert_eq!(states_per_lane, 4);
        assert_eq!(group.width, 32);
        assert_eq!(groups.width, 5);
        let wide_tuning = super::metal_config::PoseidonTuning {
            threadgroup_lanes: 256,
            states_per_lane: 2,
        };
        let wide_limits = super::PipelineLimits {
            exec_width: 32,
            max_threads: 512,
        };
        let (groups, group, logical_threads, states_per_lane) =
            super::bn254_poseidon_dispatch_geometry(512, wide_tuning, &wide_limits);
        assert_eq!(logical_threads, 256);
        assert_eq!(states_per_lane, 2);
        assert_eq!(
            group.width,
            u64::from(super::BN254_POSEIDON_THREADGROUP_CAPACITY)
        );
        assert_eq!(groups.width, 2);
    }
    #[test]
    fn column_batch_iterator_chunks_columns() {
        let batches: Vec<_> = super::column_batch_ranges(10, 4).collect();
        assert_eq!(batches, vec![(0, 4), (4, 4), (8, 2)]);
    }
    #[test]
    fn column_batch_iterator_handles_zero_total_and_batch_size() {
        let empty: Vec<_> = super::column_batch_ranges(0, 8).collect();
        assert!(empty.is_empty());
        let singletons: Vec<_> = super::column_batch_ranges(3, 0).collect();
        assert_eq!(singletons, vec![(0, 1), (1, 1), (2, 1)]);
    }
    #[test]
    fn column_batch_iterator_exact_size_handles_u32_max() {
        let mut batches = super::ColumnBatchIter::new(u32::MAX, 2);
        let expected = usize::try_from(u32::MAX.div_ceil(2)).expect("batch count fits usize");
        assert_eq!(batches.len(), expected);
        assert_eq!(batches.size_hint(), (expected, Some(expected)));
        assert_eq!(batches.next(), Some((0, 2)));
        assert_eq!(batches.len(), expected - 1);
    }
    #[test]
    fn column_batch_iterator_reports_exact_len() {
        let mut iter = super::column_batch_ranges(9, 4);
        assert_eq!(iter.len(), 3);
        iter.next();
        assert_eq!(iter.len(), 2);
        let _: Vec<_> = iter.collect();
    }
    #[test]
    fn stage_twiddles_match_reference_values() {
        let expected = vec![
            0xffff_ffff_0000_0000,
            0x0001_0000_0000_0000,
            0xffff_fffe_ff00_0001,
            0xefff_ffff_0000_0001,
            0x0000_0000_3fff_ffff_c000,
        ];
        let root = super::goldilocks_pow(super::GOLDILOCKS_GENERATOR, (FIELD_MODULUS - 1) >> 5);
        let twiddles = super::compute_stage_twiddles(5, root, false);
        assert_eq!(twiddles, expected);
        let inverse_twiddles = super::compute_stage_twiddles(5, root, true);
        for (forward, inverse) in expected.iter().zip(inverse_twiddles.iter()) {
            assert_eq!(*inverse, super::goldilocks_inv(*forward));
        }
    }
    #[test]
    fn buffer_pool_recycles_aligned_page_vectors() {
        let mut pool = BufferPool::default();
        assert_eq!(pool.len_for_tests(), 0);
        let buffer = pool.take(2).expect("allocate pages");
        assert!(buffer.capacity() >= 2);
        pool.recycle(buffer);
        assert_eq!(pool.len_for_tests(), 1);
        let buffer = pool.take(1).expect("reuse pages");
        assert!(buffer.capacity() >= 1);
        assert_eq!(pool.len_for_tests(), 0);
    }
    #[test]
    fn buffer_pool_rejects_oversized_cached_allocations() {
        assert!(buffer_pool_capacity_is_cacheable(1));
        assert!(buffer_pool_capacity_is_cacheable(
            MAX_BUFFER_POOL_PAGES_PER_BUFFER
        ));
        assert!(!buffer_pool_capacity_is_cacheable(0));
        assert!(!buffer_pool_capacity_is_cacheable(
            MAX_BUFFER_POOL_PAGES_PER_BUFFER + 1
        ));
    }
    #[test]
    fn pooled_buffer_zeroed_is_preinitialized() {
        let buffer = PooledBuffer::zeroed(4).expect("allocate pooled buffer");
        assert_eq!(buffer.to_vec().expect("copy pooled buffer"), [0, 0, 0, 0]);
    }
    #[test]
    fn pooled_buffer_copy_roundtrips_across_page_boundaries() {
        let words = (0..METAL_BUFFER_PAGE_WORDS + 3)
            .map(|index| index as u64)
            .collect::<Vec<_>>();
        let buffer = PooledBuffer::from_slice(&words).expect("allocate pooled buffer");
        assert_eq!(buffer.to_vec().expect("copy pooled buffer"), words);

        let mut boundary = [0; 4];
        buffer.copy_range_to_slice(METAL_BUFFER_PAGE_WORDS - 2, &mut boundary);
        assert_eq!(
            boundary,
            [
                (METAL_BUFFER_PAGE_WORDS - 2) as u64,
                (METAL_BUFFER_PAGE_WORDS - 1) as u64,
                METAL_BUFFER_PAGE_WORDS as u64,
                (METAL_BUFFER_PAGE_WORDS + 1) as u64,
            ]
        );
    }
    #[test]
    fn pooled_buffer_region_is_page_aligned_and_page_rounded() {
        assert_eq!(mem::align_of::<MetalBufferPage>(), METAL_BUFFER_PAGE_BYTES);
        assert_eq!(mem::size_of::<MetalBufferPage>(), METAL_BUFFER_PAGE_BYTES);
        for logical_words in [
            0,
            1,
            METAL_BUFFER_PAGE_WORDS - 1,
            METAL_BUFFER_PAGE_WORDS,
            METAL_BUFFER_PAGE_WORDS + 1,
        ] {
            let mut buffer = PooledBuffer::zeroed(logical_words).expect("allocate pooled buffer");
            let (pointer, byte_len) = buffer.metal_region();
            assert_eq!(pointer as usize % METAL_BUFFER_PAGE_BYTES, 0);
            assert_eq!(byte_len as usize % METAL_BUFFER_PAGE_BYTES, 0);
            assert!(byte_len as usize >= logical_words * mem::size_of::<u64>());
            assert_eq!(
                byte_len as usize,
                metal_buffer_page_count(logical_words) * METAL_BUFFER_PAGE_BYTES
            );
        }
    }
    #[test]
    fn aligned_pooled_buffer_can_back_a_metal_buffer_until_deallocation() {
        let Some(device) = select_metal_device() else {
            return;
        };
        let mut buffer = PooledBuffer::from_slice(&[1, 2, 3, 4]).expect("allocate pooled buffer");
        let metal_buffer = shared_pooled_buffer(&device, &mut buffer)
            .expect("aligned shared buffer should fit the Metal device limit");
        let weak_backing = buffer.weak_backing_for_tests();

        drop(buffer);
        assert!(weak_backing.upgrade().is_some());
        drop(metal_buffer);
        for _ in 0..64 {
            if weak_backing.upgrade().is_none() {
                break;
            }
            thread::yield_now();
        }
        assert!(
            weak_backing.upgrade().is_none(),
            "Metal buffer deallocation must release its aligned backing"
        );
    }
    #[test]
    fn partial_batch_abort_retains_each_backing_until_metal_deallocation() {
        let buffers = [
            PooledBuffer::zeroed(4).expect("allocate first pooled buffer"),
            PooledBuffer::zeroed(8).expect("allocate second pooled buffer"),
        ];
        let weak_backings = buffers
            .iter()
            .map(PooledBuffer::weak_backing_for_tests)
            .collect::<Vec<_>>();
        let retentions = buffers
            .iter()
            .map(|buffer| MetalBufferBackingRetention::new(buffer.backing()))
            .collect::<Vec<_>>();

        drop(buffers);
        assert!(
            weak_backings
                .iter()
                .all(|backing| backing.upgrade().is_some())
        );

        retentions[0].release();
        assert!(weak_backings[0].upgrade().is_none());
        assert!(weak_backings[1].upgrade().is_some());
        retentions[1].release();
        assert!(weak_backings[1].upgrade().is_none());
    }
    #[test]
    fn callback_release_paths_recover_poisoned_locks() {
        let buffer = PooledBuffer::zeroed(4).expect("allocate pooled buffer");
        let weak_backing = buffer.weak_backing_for_tests();
        let retention = MetalBufferBackingRetention::new(buffer.backing());
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = retention.backing.lock().expect("retention lock");
            panic!("poison retention lock for callback regression");
        }));
        assert!(poisoned.is_err());
        drop(buffer);
        retention.release();
        assert!(weak_backing.upgrade().is_none());

        let semaphore = CommandSemaphore::new(1);
        *semaphore.state.lock().expect("semaphore lock") = 1;
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = semaphore.state.lock().expect("semaphore lock");
            panic!("poison semaphore lock for callback regression");
        }));
        assert!(poisoned.is_err());
        semaphore.release();
        let in_flight = *semaphore
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(in_flight, 0);
    }
    #[test]
    fn queue_stats_capture_overlap() {
        let start = Instant::now();
        let mut state = super::QueueStatsState::default();
        state.record_launch(0, start);
        state.record_launch(0, start + Duration::from_millis(1));
        state.record_completion(0, start + Duration::from_millis(2));
        state.record_completion(0, start + Duration::from_millis(3));
        let stats = state.snapshot(2);
        assert_eq!(stats.dispatch_count, 2);
        assert_eq!(stats.max_in_flight, 2);
        assert_eq!(stats.overlap_ms, 1.0);
    }
    #[test]
    fn bounded_ticket_window_drains_in_fifo_order() {
        let mut tickets = Vec::with_capacity(2);
        tickets.extend([11, 22]);
        assert_eq!(super::pop_oldest_ticket_if_full(&mut tickets, 2), Some(11));
        tickets.push(33);
        assert_eq!(tickets, [22, 33]);
        assert_eq!(super::pop_oldest_ticket_if_full(&mut tickets, 3), None);
    }
    #[test]
    fn telemetry_sample_retention_is_bounded() {
        let mut samples = Vec::new();
        for sample in 0..=super::MAX_RETAINED_TELEMETRY_SAMPLES {
            super::push_bounded_telemetry_sample(&mut samples, sample);
        }
        assert_eq!(samples.len(), super::MAX_RETAINED_TELEMETRY_SAMPLES);
        assert_eq!(samples.first(), Some(&0));
        assert_eq!(
            samples.last(),
            Some(&(super::MAX_RETAINED_TELEMETRY_SAMPLES - 1))
        );
    }
    #[test]
    fn command_completion_releases_permit_and_queue_stats_once() {
        super::enable_queue_depth_stats(true);
        let semaphore = Box::leak(Box::new(super::CommandSemaphore::new(1)));
        assert!(semaphore.acquire_timeout(Duration::from_millis(1)));
        assert_eq!(semaphore.in_flight_for_tests(), 1);
        let completion = super::CommandPermitCompletion::new(semaphore, 0);
        completion.mark_launched();
        completion.mark_launched();
        completion.complete();
        completion.complete();
        assert_eq!(semaphore.in_flight_for_tests(), 0);
        let stats = super::take_queue_depth_stats().expect("stats captured");
        super::enable_queue_depth_stats(false);
        assert_eq!(stats.dispatch_count, 1);
        assert_eq!(stats.queues[0].dispatch_count, 1);
    }
    #[test]
    fn launched_permit_drop_defers_release_to_completion_handler() {
        let semaphore = Box::leak(Box::new(super::CommandSemaphore::new(1)));
        assert!(semaphore.acquire_timeout(Duration::from_millis(1)));
        let completion = Arc::new(super::CommandPermitCompletion::new(semaphore, 0));
        let mut permit = super::CommandPermit {
            completion: Arc::clone(&completion),
        };
        permit.mark_launched();

        drop(permit);
        assert_eq!(
            semaphore.in_flight_for_tests(),
            1,
            "a timed-out/dropped launched ticket must keep its permit"
        );
        completion.complete();
        assert_eq!(semaphore.in_flight_for_tests(), 0);
    }
    #[test]
    fn unlaunched_permit_drop_releases_immediately() {
        let semaphore = Box::leak(Box::new(super::CommandSemaphore::new(1)));
        assert!(semaphore.acquire_timeout(Duration::from_millis(1)));
        let permit = super::CommandPermit {
            completion: Arc::new(super::CommandPermitCompletion::new(semaphore, 0)),
        };

        drop(permit);
        assert_eq!(semaphore.in_flight_for_tests(), 0);
    }
    #[test]
    fn poseidon_dispatch_staging_uses_deeper_completion_backed_pipe() {
        assert!(super::POSEIDON_DISPATCH_PIPE_DEPTH > 1);
        assert!(super::POSEIDON_DISPATCH_PIPE_DEPTH <= super::DEFAULT_MAX_COMMAND_BUFFERS);
    }
    #[test]
    fn column_staging_stats_capture_events() {
        super::enable_queue_depth_stats(true);
        super::record_staging_wait(super::ColumnStagingPhase::Fft, Duration::from_millis(2));
        super::record_staging_flatten(super::ColumnStagingPhase::Fft, Duration::from_millis(5));
        super::record_staging_flatten(
            super::ColumnStagingPhase::Poseidon,
            Duration::from_millis(3),
        );
        let stats = super::take_column_staging_stats().expect("staging stats captured");
        super::enable_queue_depth_stats(false);
        let total = stats.total();
        assert_eq!(total.batches, 2);
        assert!((total.flatten_ms - 8.0).abs() < f64::EPSILON);
        assert!((total.wait_ms - 2.0).abs() < f64::EPSILON);
        assert_eq!(stats.fft().batches, 1);
        assert!((stats.fft().flatten_ms - 5.0).abs() < f64::EPSILON);
        assert!((stats.fft().wait_ms - 2.0).abs() < f64::EPSILON);
        assert_eq!(stats.poseidon().batches, 1);
        assert!((stats.poseidon().flatten_ms - 3.0).abs() < f64::EPSILON);
        assert_eq!(stats.poseidon().wait_ms, 0.0);
        assert_eq!(stats.lde().batches, 0);
        let fft_samples = stats.fft_samples();
        assert_eq!(fft_samples.len(), 1);
        assert_eq!(fft_samples[0].batch, 0);
        assert!((fft_samples[0].flatten_ms - 5.0).abs() < f64::EPSILON);
        assert!((fft_samples[0].wait_ms - 2.0).abs() < f64::EPSILON);
        let poseidon_samples = stats.poseidon_samples();
        assert_eq!(poseidon_samples.len(), 1);
        assert_eq!(poseidon_samples[0].batch, 0);
        assert!((poseidon_samples[0].flatten_ms - 3.0).abs() < f64::EPSILON);
        assert_eq!(poseidon_samples[0].wait_ms, 0.0);
        assert!(stats.lde_samples().is_empty());
    }
    #[test]
    fn queue_depth_delta_handles_accumulation() {
        let before = QueueDepthStats {
            limit: 4,
            dispatch_count: 2,
            max_in_flight: 1,
            busy_ms: 0.5,
            overlap_ms: 0.125,
            window_ms: 0.5,
            queues: vec![
                QueueLaneStats {
                    index: 0,
                    dispatch_count: 1,
                    max_in_flight: 1,
                    busy_ms: 0.25,
                    overlap_ms: 0.0,
                },
                QueueLaneStats {
                    index: 1,
                    dispatch_count: 1,
                    max_in_flight: 1,
                    busy_ms: 0.25,
                    overlap_ms: 0.125,
                },
            ],
        };
        let after = QueueDepthStats {
            limit: 4,
            dispatch_count: 5,
            max_in_flight: 3,
            busy_ms: 1.5,
            overlap_ms: 0.625,
            window_ms: 1.5,
            queues: vec![
                QueueLaneStats {
                    index: 0,
                    dispatch_count: 3,
                    max_in_flight: 2,
                    busy_ms: 1.0,
                    overlap_ms: 0.25,
                },
                QueueLaneStats {
                    index: 1,
                    dispatch_count: 3,
                    max_in_flight: 2,
                    busy_ms: 0.5,
                    overlap_ms: 0.375,
                },
            ],
        };
        let delta = after.delta_since(&before);
        assert_eq!(delta.limit, 4);
        assert_eq!(delta.dispatch_count, 3);
        assert_eq!(delta.max_in_flight, 2);
        assert!((delta.busy_ms - 1.0).abs() < f64::EPSILON);
        assert!((delta.overlap_ms - 0.5).abs() < f64::EPSILON);
        assert!((delta.window_ms - 1.0).abs() < f64::EPSILON);
        assert_eq!(delta.queues.len(), 2);
        assert_eq!(delta.queues[0].dispatch_count, 2);
        assert!((delta.queues[0].busy_ms - 0.75).abs() < f64::EPSILON);
        assert!((delta.queues[1].overlap_ms - 0.25).abs() < f64::EPSILON);
        let mut total = QueueDepthStats::default();
        total.accumulate_delta(&delta);
        assert_eq!(total.dispatch_count, 3);
        assert_eq!(total.max_in_flight, 2);
        assert!((total.busy_ms - 1.0).abs() < f64::EPSILON);
        assert!((total.overlap_ms - 0.5).abs() < f64::EPSILON);
        assert!((total.window_ms - 1.0).abs() < f64::EPSILON);
        assert_eq!(total.queues.len(), 2);
        assert_eq!(total.queues[0].max_in_flight, 2);
        let next = QueueDepthStats {
            limit: 4,
            dispatch_count: 1,
            max_in_flight: 1,
            busy_ms: 0.25,
            overlap_ms: 0.125,
            window_ms: 0.25,
            queues: vec![QueueLaneStats {
                index: 0,
                dispatch_count: 1,
                max_in_flight: 1,
                busy_ms: 0.25,
                overlap_ms: 0.125,
            }],
        };
        total.accumulate_delta(&next);
        assert_eq!(total.dispatch_count, 4);
        assert_eq!(total.max_in_flight, 2);
        assert!((total.busy_ms - 1.25).abs() < f64::EPSILON);
        assert!((total.overlap_ms - 0.625).abs() < f64::EPSILON);
        assert!((total.window_ms - 1.25).abs() < f64::EPSILON);
        assert_eq!(total.queues.len(), 2);
        assert_eq!(total.queues[0].dispatch_count, 3);
        assert_eq!(total.queues[1].dispatch_count, 2);
    }
    #[test]
    fn lde_batch_size_scales_with_domain() {
        assert_eq!(default_lde_columns_per_batch(10, 32), 64);
        assert_eq!(default_lde_columns_per_batch(12, 32), 64);
        assert_eq!(default_lde_columns_per_batch(15, 32), 64);
        assert_eq!(default_lde_columns_per_batch(17, 32), 4);
        assert_eq!(default_lde_columns_per_batch(18, 32), 2);
        assert_eq!(
            default_lde_columns_per_batch(22, 32),
            MIN_LDE_COLUMNS_PER_BATCH
        );
    }
    #[test]
    fn lde_batch_size_scales_with_lane_width() {
        assert_eq!(default_lde_columns_per_batch(10, 32), 64);
        assert_eq!(default_lde_columns_per_batch(10, 128), 32);
        assert_eq!(default_lde_columns_per_batch(10, 256), 16);
        assert_eq!(
            default_lde_columns_per_batch(20, 256),
            DEFAULT_LDE_COLUMNS_PER_BATCH
        );
    }
    #[test]
    fn fft_batch_size_scales_with_lane_width() {
        assert_eq!(default_fft_columns_per_batch(32), MAX_FFT_COLUMNS_PER_BATCH);
        assert_eq!(default_fft_columns_per_batch(64), MAX_FFT_COLUMNS_PER_BATCH);
        assert_eq!(default_fft_columns_per_batch(128), 32);
        assert_eq!(default_fft_columns_per_batch(256), 16);
    }
    #[test]
    fn fft_batch_override_validation() {
        assert_eq!(parse_fft_batch_override("2").unwrap(), 2);
        assert!(parse_fft_batch_override("0").is_err());
        assert!(parse_fft_batch_override("65").is_err());
        assert!(parse_fft_batch_override("abc").is_err());
    }
    #[test]
    fn lde_batch_override_validation() {
        assert_eq!(parse_lde_batch_override("4").unwrap(), 4);
        assert!(parse_lde_batch_override("0").is_err());
        assert!(parse_lde_batch_override("65").is_err());
        assert!(parse_lde_batch_override("abc").is_err());
    }
    #[test]
    fn default_in_flight_limit_scales_with_parallelism() {
        assert_eq!(default_in_flight_limit_for_parallelism(1), 2);
        assert_eq!(default_in_flight_limit_for_parallelism(2), 2);
        assert_eq!(default_in_flight_limit_for_parallelism(4), 2);
        assert_eq!(default_in_flight_limit_for_parallelism(6), 3);
        assert_eq!(default_in_flight_limit_for_parallelism(8), 4);
        assert_eq!(default_in_flight_limit_for_parallelism(12), 6);
        assert_eq!(default_in_flight_limit_for_parallelism(32), 16);
    }
    #[test]
    fn adaptive_batch_doubles_until_target() {
        let state = AdaptiveBatchState::new(1, 2.0);
        let selection = state.select(2, 16, AdaptiveStateId::Fft);
        assert_eq!(selection.columns(), 2);
        state.record_sample(2, 16, 1.0);
        let next = state.select(2, 16, AdaptiveStateId::Fft);
        assert_eq!(next.columns(), 4);
    }
    #[test]
    fn adaptive_batch_backs_off_after_slow_sample() {
        let state = AdaptiveBatchState::new(1, 2.0);
        let selection = state.select(2, 32, AdaptiveStateId::Fft);
        assert_eq!(selection.columns(), 2);
        state.record_sample(2, 32, 1.0);
        let grown = state.select(2, 32, AdaptiveStateId::Fft);
        assert_eq!(grown.columns(), 4);
        state.record_sample(4, 32, 2.0 * ADAPTIVE_BACKOFF_RATIO + 0.1);
        let backoff = state.select(2, 32, AdaptiveStateId::Fft);
        assert_eq!(backoff.columns(), 2);
    }
    #[test]
    fn adaptive_batch_backoff_respects_minimum_floor() {
        let state = AdaptiveBatchState::new(3, 2.0);
        let selection = state.select(4, 64, AdaptiveStateId::Fft);
        assert_eq!(selection.columns(), 4);
        state.record_sample(4, 64, 2.0 * ADAPTIVE_BACKOFF_RATIO + 0.1);
        let next = state.select(4, 64, AdaptiveStateId::Fft);
        assert_eq!(next.columns(), 3);
    }
    #[test]
    fn parse_gpu_core_count_reads_fields() {
        let payload = r#"{"SPDisplaysDataType":[{"sppci_cores":10}]}"#;
        assert_eq!(super::parse_gpu_core_count(payload), Some(10));
        let payload = r#"{"SPDisplaysDataType":[{"spdisplays_cores":"8"}]}"#;
        assert_eq!(super::parse_gpu_core_count(payload), Some(8));
    }
    #[test]
    fn kernel_descriptors_cover_entry_points() {
        let descriptors = super::metal_kernel_descriptors();
        assert_eq!(descriptors.len(), 11);
        for name in [
            "fastpq_fft_columns",
            "fastpq_fft_post_tiling",
            "fastpq_lde_columns",
            "poseidon_permute",
            "poseidon_hash_columns",
            "poseidon_hash_rows",
            "poseidon_trace_fused",
            "poseidon_trace_parents",
            "bn254_fft_columns",
            "bn254_lde_columns",
            "bn254_poseidon_hash_words",
        ] {
            assert!(
                descriptors
                    .iter()
                    .any(|descriptor| descriptor.entry_point == name),
                "missing descriptor for {name}"
            );
        }
        let bn254_poseidon = descriptors
            .iter()
            .find(|descriptor| descriptor.entry_point == "bn254_poseidon_hash_words")
            .expect("BN254 Poseidon descriptor");
        assert_eq!(
            bn254_poseidon.threadgroup_cap,
            Some(super::BN254_POSEIDON_THREADGROUP_CAPACITY)
        );
    }
}
