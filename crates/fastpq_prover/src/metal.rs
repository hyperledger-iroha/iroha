#![cfg(all(feature = "fastpq-gpu", target_os = "macos"))]

//! Metal GPU bindings for FASTPQ.

#[cfg(test)]
use std::sync::Once;
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    env,
    ffi::c_void,
    iter::FusedIterator,
    mem,
    ops::Range,
    path::Path,
    process::{Command, Stdio},
    ptr,
    sync::{
        Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant},
    vec::Vec,
};

use fastpq_isi::poseidon::STATE_WIDTH;
use halo2curves::{bn256::Fr as Bn254Fr, ff::PrimeField};
use iroha_zkp_halo2::{Bn254Scalar, IpaScalar};
use metal::{
    Buffer, CommandBuffer, CommandQueue, ComputeCommandEncoderRef, ComputePipelineState, Device,
    Library, MTLCommandBufferStatus, MTLDeviceLocation, MTLResourceOptions, MTLSize, NSRange,
};
use norito::json::{self, Value};
use smallvec::SmallVec;
use tracing::{debug, warn};

use crate::{
    backend::GpuBackend,
    gpu::GpuError,
    metal_config::{self, DeviceHints},
    overrides,
    poseidon::FIELD_MODULUS,
    poseidon_manifest::poseidon_manifest,
    trace::{PoseidonColumnBatch, PoseidonColumnSlice},
};

type MetalResult<T> = Result<T, GpuError>;

const POSEIDON_PERMUTE_KERNEL: &str = "poseidon_permute";
const POSEIDON_HASH_KERNEL: &str = "poseidon_hash_columns";
const POSEIDON_TRACE_FUSED_KERNEL: &str = "poseidon_trace_fused";
const FFT_KERNEL: &str = "fastpq_fft_columns";
const LDE_KERNEL: &str = "fastpq_lde_columns";
const POST_TILE_KERNEL: &str = "fastpq_fft_post_tiling";
const BN254_FFT_KERNEL: &str = "bn254_fft_columns";
const BN254_LDE_KERNEL: &str = "bn254_lde_columns";
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
const FFT_TILE_STAGE_LIMIT: u32 = 32;
/// Must match `FFT_TILE_STAGE_CAP` in `metal/kernels/ntt_stage.metal`.
const LDE_TILE_STAGE_ENV: &str = "FASTPQ_METAL_LDE_TILE_STAGES";
const POSEIDON_THREADGROUP_CAPACITY: u32 = 256;
const POSEIDON_TARGET_THREADS: u32 = 8_192;
const MIN_POSEIDON_STATES_PER_BATCH: u32 = 1;
const QUEUE_FANOUT_ENV: &str = "FASTPQ_METAL_QUEUE_FANOUT";
const QUEUE_COLUMN_THRESHOLD_ENV: &str = "FASTPQ_METAL_COLUMN_THRESHOLD";
const MIN_QUEUE_FANOUT: usize = 1;
const MAX_QUEUE_FANOUT: usize = 4;
const DISCRETE_QUEUE_FANOUT: usize = 2;
const MIN_QUEUE_COLUMN_THRESHOLD: u32 = 1;
const DEFAULT_QUEUE_COLUMN_THRESHOLD: u32 = 16;
const MAX_BUFFER_POOL_BUFFERS: usize = 8;
const DEFAULT_MAX_COMMAND_BUFFERS: usize = 4;
const BN254_LIMBS: usize = 4;
const COLUMN_STAGING_PIPE_DEPTH: usize = 2;
const ADAPTIVE_TARGET_MS: f64 = 2.0;
const ADAPTIVE_BACKOFF_RATIO: f64 = 1.3;

fn debug_env_var(name: &str) -> Option<String> {
    overrides::guard_env_override(|| overrides::debug_env_string(name))
}

fn debug_env_bool(name: &str) -> Option<bool> {
    overrides::guard_env_override(|| overrides::debug_env_bool(name))
}

// BN254 Metal pipelines are bundled to keep the host in sync with the metallib;
// dispatchers stage twiddles/cosets and run a smoke test via `bn254_status()`
// to ensure the kernels are reachable before enabling GPU paths.
static METAL_CONTEXT: OnceLock<MetalResult<MetalPipelines>> = OnceLock::new();
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

/// Placeholder BN254 Metal entrypoint until kernels are implemented.
///
/// Returns `GpuError::Unsupported` when Metal is unavailable; otherwise loads
/// the BN254 FFT/LDE pipelines and runs a minimal smoke test to prove the
/// kernels are reachable.
pub(crate) fn bn254_status() -> MetalResult<()> {
    if Device::system_default().is_none() {
        return Err(GpuError::Unsupported(GpuBackend::Metal));
    }
    let ctx = metal_context()?;
    let _ = (&ctx.bn254_fft, &ctx.bn254_lde);
    bn254_smoke_test()?;
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
    let expected = 1usize << log_size;
    if element_extent != expected {
        return Err(GpuError::InvalidInput(
            "BN254 FFT columns must match the requested log size",
        ));
    }
    let limb_extent = expected
        .checked_mul(BN254_LIMBS)
        .ok_or(GpuError::InvalidInput(
            "BN254 FFT column length exceeds platform limits",
        ))?;
    let column_len_u64 = u64::try_from(expected)
        .map_err(|_| GpuError::InvalidInput("BN254 FFT column length exceeds 64-bit range"))?;
    let context = metal_context()?;
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
    for (batch_index, (offset, batch_columns)) in batches.into_iter().enumerate() {
        let slot_index = batch_index % pipe_depth;
        if let Some(ticket) = slots[slot_index].take() {
            ticket.wait(columns, limb_extent, true)?;
        }
        let start = usize::try_from(offset).expect("column offset fits usize");
        let width = usize::try_from(batch_columns).expect("batch column count fits usize");
        let range = start..start + width;
        let mut buffer = flatten_with_stats(&columns[range.clone()], ColumnStagingPhase::Fft);
        let metal_buffer = shared_buffer(&context.device, buffer.as_mut_slice());
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
    let pending_batches: Vec<ColumnBatchTicket> = slots.into_iter().flatten().collect();
    Ok(PendingColumns::new(
        columns,
        limb_extent,
        twiddle_buffer,
        pending_batches,
    ))
}

fn dispatch_bn254_lde_columns(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    coset: [u64; BN254_LIMBS],
) -> MetalResult<PendingLde> {
    if blowup_log == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 LDE requires a positive blowup factor",
        ));
    }
    if trace_log == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 LDE requires a trace log greater than zero",
        ));
    }
    let trace_extent = bn254_column_extent(coeffs)?;
    if trace_extent == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 LDE requires at least one coefficient",
        ));
    }
    let expected_trace = 1usize << trace_log;
    if trace_extent != expected_trace {
        return Err(GpuError::InvalidInput(
            "BN254 LDE coefficients must match the trace log size",
        ));
    }
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE log size exceeds 32-bit representation",
        ))?;
    let eval_len = 1usize << eval_log;
    let context = metal_context()?;
    let stage_twiddle_buffer = context.bn254_lde_twiddle_buffer(trace_log, blowup_log)?;
    let mut coeff_buffer = flatten_with_stats(coeffs, ColumnStagingPhase::Lde);
    let stats_enabled = LDE_STATS_ENABLED.load(Ordering::Acquire);
    let zero_timer = stats_enabled.then(|| Instant::now());
    let eval_limbs = coeffs
        .len()
        .checked_mul(eval_len)
        .and_then(|len| len.checked_mul(BN254_LIMBS))
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE output length exceeds limits",
        ))?;
    let mut eval_buffer = PooledBuffer::zeroed(eval_limbs);
    let host_stats = zero_timer.map(|start| LdeHostStats {
        zero_fill_bytes: eval_buffer.as_slice().len() * mem::size_of::<u64>(),
        zero_fill_ms: elapsed_ms(start.elapsed()),
        queue_delta: None,
    });
    let coeff_metal = shared_buffer(&context.device, coeff_buffer.as_mut_slice());
    let eval_metal = shared_buffer(&context.device, eval_buffer.as_mut_slice());
    let coset_scalar = bn254_scalar_from_canonical_limbs(&coset)?;
    let coset_limbs = bn254_scalar_to_canonical_limbs(&coset_scalar);
    let coset_buffer = upload_bn254_coset(&context.device, &coset_limbs)?;
    let column_count = coeffs.len();
    let column_count_u32 = u32::try_from(column_count)
        .map_err(|_| GpuError::InvalidInput("BN254 column count exceeds 32-bit range"))?;
    let eval_len_u64 = u64::try_from(eval_len)
        .map_err(|_| GpuError::InvalidInput("BN254 eval length exceeds 64-bit representation"))?;
    let mut tickets = Vec::with_capacity(column_count);
    let trace_len_u64 = u64::try_from(trace_extent)
        .map_err(|_| GpuError::InvalidInput("BN254 trace length exceeds 64-bit range"))?;
    for column in 0..column_count {
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

    fn cpu_fft(columns: &mut [Vec<Bn254Scalar>], log_size: u32, twiddles: &[Bn254Scalar]) {
        let n = 1usize << log_size;
        for column in columns {
            for stage in 0..log_size {
                let len = 1usize << (stage + 1);
                let half = len / 2;
                let stage_offset = stage as usize * (n / 2);
                for block in (0..n).step_by(len) {
                    for pair in 0..half {
                        let idx = block + pair;
                        let twiddle = twiddles[stage_offset + pair];
                        let u = column[idx];
                        let v = column[idx + half].mul(twiddle);
                        column[idx] = u.add(v);
                        column[idx + half] = u.sub(v);
                    }
                }
            }
        }
    }

    fn cpu_lde(
        coeffs: &[Vec<Bn254Scalar>],
        trace_log: u32,
        blowup_log: u32,
        twiddles: &[Bn254Scalar],
        coset: Bn254Scalar,
    ) -> Vec<Vec<Bn254Scalar>> {
        let eval_log = trace_log + blowup_log;
        let trace_len = 1usize << trace_log;
        let eval_len = 1usize << eval_log;
        let mut outputs = Vec::with_capacity(coeffs.len());
        for column in coeffs {
            let mut data = vec![Bn254Scalar::zero(); eval_len];
            data[..trace_len].copy_from_slice(column);
            // Scale coefficients by coset^i so the FFT evaluates over the coset domain.
            let mut coset_power = Bn254Scalar::one();
            for coeff in data.iter_mut().take(trace_len) {
                *coeff = coeff.mul(coset_power);
                coset_power = coset_power.mul(coset);
            }
            let mut column_fft = vec![data];
            cpu_fft(&mut column_fft, eval_log, twiddles);
            outputs.push(column_fft.pop().expect("single column present"));
        }
        outputs
    }

    #[test]
    fn fft_matches_cpu_reference() {
        ensure_multi_queue_env();
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
    store.lock().ok().map(|mut guard| guard.drain(..).collect())
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
    store.lock().ok().map(|mut guard| guard.drain(..).collect())
}

fn record_post_tile_sample(kind: KernelKind, log_len: u32, stage_start: u32, columns: u32) {
    if !POST_TILE_STATS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let store = POST_TILE_STATS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = store.lock() {
        guard.push(PostTileSample {
            kind,
            log_len,
            stage_start,
            columns,
        });
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
        self.samples_mut(phase).push(ColumnStagingSample {
            batch,
            flatten_ms,
            wait_ms,
        });
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
                buffer, executes tiled FFT stages with the requested coset, and leaves the final \
                stages to the post-tiling kernel when necessary.",
    },
    MetalKernelDescriptor {
        entry_point: "poseidon_permute",
        kind: KernelKind::Poseidon,
        threadgroup_cap: Some(POSEIDON_THREADGROUP_CAPACITY),
        tile_stage_cap: None,
        notes: "High-occupancy Poseidon2 permutation over STATE_WIDTH=3 words. \
                Threadgroups cache the round constants/MDS matrix in threadgroup memory, \
                each lane walks multiple states (tunable via FASTPQ_METAL_POSEIDON_BATCH) \
                with unrolled rounds, and FASTPQ_METAL_POSEIDON_LANES pins the launch width \
                so dispatches always launch ≥4k logical threads without recompiling the metallib.",
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
        guard.push(context.sample(duration));
    }
}

struct ColumnBatchTicket {
    range: Range<usize>,
    buffer: PooledBuffer,
    metal_buffer: Buffer,
    tickets: SmallVec<[DispatchTicket; 2]>,
}

impl ColumnBatchTicket {
    fn wait(self, columns: &mut [Vec<u64>], extent: usize, record_wait: bool) -> MetalResult<()> {
        let ColumnBatchTicket {
            range,
            buffer,
            metal_buffer: _metal,
            tickets,
        } = self;
        let wait_start = Instant::now();
        wait_for_tickets(tickets)?;
        if record_wait {
            record_staging_wait(ColumnStagingPhase::Fft, wait_start.elapsed());
        }
        restore_range(columns, range, buffer.as_slice(), extent);
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
        let wait_start = Instant::now();
        wait_for_ticket(ticket)?;
        if record_wait {
            record_staging_wait(ColumnStagingPhase::Poseidon, wait_start.elapsed());
        }
        states[range].copy_from_slice(buffer.as_slice());
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
        for (index, chunk) in states.as_slice().chunks_exact(STATE_WIDTH).enumerate() {
            result[column_offset + index] = chunk[0];
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
    _twiddle_buffer: Buffer,
    completed: bool,
}

impl<'a> PendingColumns<'a> {
    fn new(
        columns: &'a mut [Vec<u64>],
        extent: usize,
        twiddle_buffer: Buffer,
        pending_batches: Vec<ColumnBatchTicket>,
    ) -> Self {
        Self {
            columns,
            extent,
            pending_batches,
            _twiddle_buffer: twiddle_buffer,
            completed: false,
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
            batch.wait(self.columns, self.extent, false)?;
        }
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
        let result = self.finish()?;
        self.completed = true;
        Ok(result)
    }

    fn finish(&mut self) -> MetalResult<Option<Vec<Vec<u64>>>> {
        if self.completed {
            return Ok(None);
        }
        let tickets = mem::take(&mut self.tickets);
        wait_for_tickets(tickets)?;
        let mut result = Vec::with_capacity(self.column_count);
        let chunk_len = self.eval_len.saturating_mul(self.limbs_per_elem);
        for chunk in self.eval_buffer.as_slice().chunks(chunk_len) {
            result.push(chunk.to_vec());
        }
        if let Some(stats) = self.host_stats.take() {
            record_lde_stats(stats);
        }
        self.completed = true;
        Ok(Some(result))
    }
}

impl Drop for PendingLde {
    fn drop(&mut self) {
        if self.completed || self.tickets.is_empty() {
            return;
        }
        if let Err(error) = self.finish() {
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
struct PoseidonFusedArgs {
    state_count: u32,
    states_per_lane: u32,
    block_count: u32,
    leaf_offset: u32,
    parent_offset: u32,
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

struct QueuePool {
    queues: Vec<CommandQueue>,
    policy: QueuePolicy,
}

impl QueuePool {
    fn new(device: &Device, policy: QueuePolicy) -> Self {
        let mut queues = Vec::with_capacity(policy.fanout());
        for _ in 0..policy.fanout() {
            queues.push(device.new_command_queue());
        }
        Self { queues, policy }
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

    fn resolve(&mut self, device: &Device, log_len: u32, inverse: bool) -> Buffer {
        let key = TwiddleCacheKey { log_len, inverse };
        if let Some(entry) = self.buffers.get(&key) {
            record_twiddle_cache_sample(entry.build_cost_ms, true);
            return entry.buffer.clone();
        }
        let started = Instant::now();
        let stage_twiddles = compute_stage_twiddles(log_len, inverse);
        let buffer = device.new_buffer_with_data(
            stage_twiddles.as_ptr().cast::<c_void>(),
            (stage_twiddles.len() * mem::size_of::<u64>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );
        let build_cost_ms = elapsed_ms(started.elapsed());
        self.buffers.insert(
            key,
            TwiddleCacheEntry {
                buffer: buffer.clone(),
                build_cost_ms,
            },
        );
        record_twiddle_cache_sample(build_cost_ms, false);
        buffer
    }
}

struct MetalPipelines {
    device: Device,
    queues: QueuePool,
    poseidon_permute: ComputePipelineState,
    poseidon_hash: ComputePipelineState,
    poseidon_trace_fused: ComputePipelineState,
    fft: ComputePipelineState,
    lde: ComputePipelineState,
    post_tile: ComputePipelineState,
    bn254_fft: ComputePipelineState,
    bn254_lde: ComputePipelineState,
    twiddle_cache: Mutex<TwiddleCache>,
    bn254_fft_twiddles: Mutex<HashMap<u32, Buffer>>,
    bn254_lde_twiddles: Mutex<HashMap<(u32, u32), Buffer>>,
}

fn metal_context() -> MetalResult<&'static MetalPipelines> {
    match METAL_CONTEXT.get_or_init(build_metal_context) {
        Ok(context) => Ok(context),
        Err(err) => Err(err.clone()),
    }
}

impl MetalPipelines {
    fn stage_twiddle_buffer(&self, log_len: u32, inverse: bool) -> Buffer {
        let mut cache = self
            .twiddle_cache
            .lock()
            .expect("Metal twiddle cache poisoned");
        cache.resolve(&self.device, log_len, inverse)
    }

    fn bn254_fft_twiddle_buffer(&self, log_size: u32) -> MetalResult<Buffer> {
        let mut cache = self
            .bn254_fft_twiddles
            .lock()
            .expect("BN254 FFT twiddle cache poisoned");
        if let Some(buffer) = cache.get(&log_size) {
            return Ok(buffer.clone());
        }
        let buffer = stage_bn254_twiddles(&self.device, log_size)?;
        cache.insert(log_size, buffer.clone());
        Ok(buffer)
    }

    fn bn254_lde_twiddle_buffer(&self, trace_log: u32, blowup_log: u32) -> MetalResult<Buffer> {
        let key = (trace_log, blowup_log);
        let mut cache = self
            .bn254_lde_twiddles
            .lock()
            .expect("BN254 LDE twiddle cache poisoned");
        if let Some(buffer) = cache.get(&key) {
            return Ok(buffer.clone());
        }
        let eval_log = trace_log
            .checked_add(blowup_log)
            .ok_or(GpuError::InvalidInput(
                "BN254 LDE log size exceeds 32-bit representation",
            ))?;
        let buffer = stage_bn254_twiddles(&self.device, eval_log)?;
        cache.insert(key, buffer.clone());
        Ok(buffer)
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

/// Upload a flattened BN254 twiddle buffer (Montgomery limbs) for Metal FFT/LDE kernels.
///
/// Layout: stage-major, contiguous per stage with `(n/2)` twiddles, indexed as
/// `stage * (n/2) + offset`. Each twiddle must be four `u64` limbs in BN254
/// Montgomery form derived from the CPU domain to preserve parity.
pub(crate) fn upload_bn254_twiddles(device: &Device, twiddles: &[u64]) -> MetalResult<Buffer> {
    if twiddles.len() % 4 != 0 {
        return Err(GpuError::InvalidInput(
            "BN254 twiddle buffer must be a multiple of 4 limbs",
        ));
    }
    let byte_len = (twiddles.len() * mem::size_of::<u64>()) as u64;
    let buffer = device.new_buffer_with_data(
        twiddles.as_ptr().cast::<c_void>(),
        byte_len,
        MTLResourceOptions::StorageModeShared,
    );
    Ok(buffer)
}

/// Flatten an array of BN254 twiddles (each four limbs) into a `[u64]` buffer.
pub(crate) fn flatten_bn254_twiddles(twiddles: &[[u64; 4]]) -> Vec<u64> {
    let mut flat = Vec::with_capacity(twiddles.len() * 4);
    for t in twiddles {
        flat.extend_from_slice(t);
    }
    flat
}

/// Convenience: derive stage-major BN254 twiddles on CPU then upload to Metal.
pub(crate) fn stage_bn254_twiddles(device: &Device, log_size: u32) -> MetalResult<Buffer> {
    let twiddles = bn254_stage_twiddles_limbs(log_size)?;
    validate_bn254_twiddles_shape(log_size, &twiddles)?;
    let flat = flatten_bn254_twiddles(&twiddles);
    upload_bn254_twiddles(device, &flat)
}

/// Validate BN254 twiddle layout against the expected stage-major shape.
///
/// For `log_size`, the twiddle count must equal `log_size * (n/2)` where `n = 1 << log_size`.
pub(crate) fn validate_bn254_twiddles_shape(
    log_size: u32,
    twiddles: &[[u64; 4]],
) -> MetalResult<()> {
    if log_size == 0 {
        return Err(GpuError::InvalidInput(
            "BN254 twiddles require log_size > 0",
        ));
    }
    let n = 1usize << log_size;
    let expected = (log_size as usize) * (n / 2);
    if twiddles.len() != expected {
        return Err(GpuError::InvalidInput(
            "BN254 twiddles shape mismatch for stage-major layout",
        ));
    }
    Ok(())
}

/// Expected twiddle count for BN254 FFT (radix-2) given `log_size`.
pub(crate) fn bn254_fft_twiddle_len(log_size: u32) -> MetalResult<usize> {
    if log_size == 0 {
        return Err(GpuError::InvalidInput("BN254 FFT requires log_size > 0"));
    }
    let n = 1usize << log_size;
    Ok((log_size as usize) * (n / 2))
}

/// Expected twiddle count for BN254 LDE (radix-2) given trace/eval logs.
pub(crate) fn bn254_lde_twiddle_len(trace_log: u32, blowup_log: u32) -> MetalResult<usize> {
    if trace_log == 0 {
        return Err(GpuError::InvalidInput("BN254 LDE requires trace_log > 0"));
    }
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE log size exceeds 32-bit representation",
        ))?;
    let eval_len = 1usize << eval_log;
    Ok((eval_log as usize) * (eval_len / 2))
}

/// Upload a BN254 coset element (4 Montgomery limbs) for LDE kernels.
pub(crate) fn upload_bn254_coset(device: &Device, coset: &[u64]) -> MetalResult<Buffer> {
    if coset.len() != 4 {
        return Err(GpuError::InvalidInput(
            "BN254 coset must contain exactly 4 limbs",
        ));
    }
    let byte_len = (coset.len() * mem::size_of::<u64>()) as u64;
    let buffer = device.new_buffer_with_data(
        coset.as_ptr().cast::<c_void>(),
        byte_len,
        MTLResourceOptions::StorageModeShared,
    );
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

fn build_metal_context() -> MetalResult<MetalPipelines> {
    let Some(device) = Device::system_default() else {
        return Err(GpuError::Unsupported(GpuBackend::Metal));
    };
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
    let library_path =
        resolve_metal_library_path().ok_or_else(|| GpuError::Unsupported(GpuBackend::Metal))?;

    let library =
        device
            .new_library_with_file(&library_path)
            .map_err(|err| GpuError::Execution {
                backend: GpuBackend::Metal,
                message: format!("failed to load Metal library: {err}"),
            })?;

    let poseidon_permute = load_pipeline(&device, &library, POSEIDON_PERMUTE_KERNEL)?;
    let poseidon_hash = load_pipeline(&device, &library, POSEIDON_HASH_KERNEL)?;
    let poseidon_trace_fused = load_pipeline(&device, &library, POSEIDON_TRACE_FUSED_KERNEL)?;
    let fft = load_pipeline(&device, &library, FFT_KERNEL)?;
    let lde = load_pipeline(&device, &library, LDE_KERNEL)?;
    let post_tile = load_pipeline(&device, &library, POST_TILE_KERNEL)?;
    // BN254 kernels are loaded to ensure the metallib stays in sync with the host,
    // but remain gated behind parity checks before use.
    let bn254_fft = load_pipeline(&device, &library, BN254_FFT_KERNEL)?;
    let bn254_lde = load_pipeline(&device, &library, BN254_LDE_KERNEL)?;
    let queue_policy = resolve_queue_policy(&device);
    let queues = QueuePool::new(&device, queue_policy);
    let manifest_sha = poseidon_manifest().sha256_hex();
    debug!(
        target: "fastpq::metal",
        manifest_sha = manifest_sha,
        "loaded Poseidon manifest for GPU parity checks"
    );

    // Pre-stage minimal BN254 twiddle buffers from the CPU domain builder to ensure
    // the GPU layout stays aligned with host fixtures before runtime dispatches.
    let mut bn254_fft_twiddles = HashMap::new();
    let mut bn254_lde_twiddles = HashMap::new();
    let fft_min_log = 1u32;
    let lde_min = (1u32, 1u32); // smallest valid trace/log combination
    let fft_buffer = stage_bn254_twiddles(&device, fft_min_log)?;
    bn254_fft_twiddles.insert(fft_min_log, fft_buffer);
    let lde_eval_log = lde_min
        .0
        .checked_add(lde_min.1)
        .ok_or(GpuError::InvalidInput(
            "BN254 LDE log size exceeds 32-bit representation",
        ))?;
    let lde_buffer = stage_bn254_twiddles(&device, lde_eval_log)?;
    bn254_lde_twiddles.insert(lde_min, lde_buffer);

    Ok(MetalPipelines {
        device,
        queues,
        poseidon_permute,
        poseidon_hash,
        poseidon_trace_fused,
        fft,
        lde,
        post_tile,
        bn254_fft,
        bn254_lde,
        twiddle_cache: Mutex::new(TwiddleCache::new()),
        bn254_fft_twiddles: Mutex::new(bn254_fft_twiddles),
        bn254_lde_twiddles: Mutex::new(bn254_lde_twiddles),
    })
}

fn resolve_metal_library_path() -> Option<String> {
    debug_env_var("FASTPQ_METAL_LIB").and_then(|path| {
        if !path.is_empty() && Path::new(&path).exists() {
            Some(path)
        } else {
            None
        }
    })
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
pub fn fft_columns(columns: &mut [Vec<u64>], log_size: u32) -> MetalResult<()> {
    if columns.is_empty() {
        return Ok(());
    }

    fft_columns_async(columns, log_size)?.wait()
}

/// Dispatches an FFT over the provided columns and returns a pending handle.
pub(crate) fn fft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
) -> MetalResult<PendingColumns<'a>> {
    dispatch_fft_columns(columns, log_size, false)
}

fn dispatch_fft_columns<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
    inverse: bool,
) -> MetalResult<PendingColumns<'a>> {
    let extent = 1usize << log_size;
    if columns.iter().any(|column| column.len() != extent) {
        return Err(GpuError::InvalidInput("columns must share length"));
    }

    let column_len = u64::try_from(extent)
        .map_err(|_| GpuError::InvalidInput("column length exceeds u64::MAX"))?;
    let column_count = u32::try_from(columns.len())
        .map_err(|_| GpuError::InvalidInput("column count exceeds u32::MAX"))?;

    let context = metal_context()?;
    let limits = pipeline_limits(&context.fft);
    let tuning = metal_config::fft_tuning(log_size, limits.exec_width, limits.max_threads);
    let twiddle_buffer = context.stage_twiddle_buffer(log_size, inverse);

    let base_args = FftArgs {
        column_len,
        log_len: log_size,
        column_count: 0,
        inverse: inverse as u32,
        local_stage_limit: tuning.tile_stage_limit,
        threadgroup_lanes: tuning.threadgroup_lanes,
        column_offset: 0,
        _padding: 0,
    };

    let post_stage_start = post_tile_stage_start(log_size, tuning.tile_stage_limit);
    let fft_selection = select_fft_batch(tuning.threadgroup_lanes);
    let batch_size = fft_selection.columns();
    let batches = column_batch_ranges(column_count, batch_size);
    let pipe_depth = COLUMN_STAGING_PIPE_DEPTH.max(1);
    let mut slots: Vec<Option<ColumnBatchTicket>> = Vec::with_capacity(pipe_depth);
    slots.resize_with(pipe_depth, || None);
    let queue_total_columns =
        queue_total_columns_hint(column_count, inverse, context.queues.policy());

    for (batch_index, (offset, batch_columns)) in batches.into_iter().enumerate() {
        let slot_index = batch_index % pipe_depth;
        if let Some(batch) = slots[slot_index].take() {
            batch.wait(columns, extent, true)?;
        }
        let start = usize::try_from(offset).expect("column offset fits usize");
        let width = usize::try_from(batch_columns).expect("batch column count fits usize");
        let range = start..start + width;
        let mut buffer = flatten_with_stats(&columns[range.clone()], ColumnStagingPhase::Fft);
        let metal_buffer = shared_buffer(&context.device, buffer.as_mut_slice());
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

    let pending_batches: Vec<ColumnBatchTicket> = slots.into_iter().flatten().collect();

    Ok(PendingColumns::new(
        columns,
        extent,
        twiddle_buffer,
        pending_batches,
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
    let mut logical_threads = if per_lane == 0 {
        0
    } else {
        states.div_ceil(per_lane)
    };
    logical_threads = logical_threads.max(u64::from(POSEIDON_TARGET_THREADS));
    let threadgroups = logical_threads.div_ceil(lanes).max(1);
    let group_width = lanes.max(1);
    let group_count = threadgroups.max(1);
    (
        MTLSize::new(group_count, 1, 1),
        MTLSize::new(group_width, 1, 1),
        logical_threads,
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
    debug_assert!(
        state_count > 0,
        "poseidon batch requires positive state count"
    );
    let recommended = poseidon_recommended_states_per_batch(state_count, tuning);
    adaptive_scheduler()
        .select_poseidon(recommended, state_count.max(MIN_POSEIDON_STATES_PER_BATCH))
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
        let per_batch = self.batch_size;
        let batches = remaining.saturating_add(per_batch - 1) / per_batch;
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
        return Err("tile depth out of supported range (1–32 stages)");
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
pub fn ifft_columns(columns: &mut [Vec<u64>], log_size: u32) -> MetalResult<()> {
    if columns.is_empty() {
        return Ok(());
    }

    ifft_columns_async(columns, log_size)?.wait()
}

/// Dispatches an inverse FFT and returns a pending handle for the caller to await.
pub(crate) fn ifft_columns_async<'a>(
    columns: &'a mut [Vec<u64>],
    log_size: u32,
) -> MetalResult<PendingColumns<'a>> {
    dispatch_fft_columns(columns, log_size, true)
}

/// Returns the resolved FFT tuning (threadgroup lanes/tile stages) for the current Metal device.
pub fn fft_tuning_snapshot(log_size: u32) -> MetalResult<metal_config::FftTuning> {
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
    Ok(metal_config::poseidon_tuning(
        limits.exec_width,
        limits.max_threads,
    ))
}

#[allow(dead_code)] // Metal LDE entry point is unused when Metal is not available
pub fn lde_columns(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    coset: u64,
) -> MetalResult<Option<Vec<Vec<u64>>>> {
    if coeffs.is_empty() {
        return Ok(Some(Vec::new()));
    }

    lde_columns_async(coeffs, trace_log, blowup_log, coset)?.wait()
}

/// Dispatches an LDE kernel and returns a pending handle so callers can wait later.
pub(crate) fn lde_columns_async(
    coeffs: &[Vec<u64>],
    trace_log: u32,
    blowup_log: u32,
    coset: u64,
) -> MetalResult<PendingLde> {
    let trace_len = 1usize << trace_log;
    if coeffs.iter().any(|column| column.len() != trace_len) {
        return Err(GpuError::InvalidInput(
            "coefficient columns must share length",
        ));
    }

    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or(GpuError::InvalidInput(
            "lde log size exceeds 32-bit representation",
        ))?;
    let eval_len = 1usize << eval_log;

    let trace_len_u64 = u64::try_from(trace_len)
        .map_err(|_| GpuError::InvalidInput("trace length exceeds u64::MAX"))?;
    let eval_len_u64 = u64::try_from(eval_len)
        .map_err(|_| GpuError::InvalidInput("lde length exceeds u64::MAX"))?;
    let column_count = u32::try_from(coeffs.len())
        .map_err(|_| GpuError::InvalidInput("column count exceeds u32::MAX"))?;

    let mut coeff_buffer = flatten_with_stats(coeffs, ColumnStagingPhase::Lde);
    // Pre-zero the evaluation buffer so the Metal kernel can assume padded slots are zeroed.
    let stats_enabled = LDE_STATS_ENABLED.load(Ordering::Acquire);
    let queue_before = snapshot_queue_depth_stats();
    let zero_timer = stats_enabled.then(|| Instant::now());
    let mut eval_buffer = PooledBuffer::zeroed(coeffs.len() * eval_len);
    let queue_after = snapshot_queue_depth_stats();
    let queue_delta = match (queue_before, queue_after) {
        (Some(before), Some(after)) => Some(after.delta_since(&before)),
        _ => None,
    };
    let host_stats = zero_timer.map(|start| LdeHostStats {
        zero_fill_bytes: eval_buffer.as_slice().len() * mem::size_of::<u64>(),
        zero_fill_ms: elapsed_ms(start.elapsed()),
        queue_delta,
    });
    let context = metal_context()?;
    let coeff_metal = shared_buffer(&context.device, coeff_buffer.as_mut_slice());
    let eval_metal = shared_buffer(&context.device, eval_buffer.as_mut_slice());
    let stage_twiddle_buffer = context.stage_twiddle_buffer(eval_log, false);
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

    let mut tickets = Vec::new();
    let post_stage_start = post_tile_stage_start(eval_log, local_stage_limit);
    let lde_selection = select_lde_batch(eval_log, tuning.threadgroup_lanes);
    let batch_size = lde_selection.columns();
    let batches = column_batch_ranges(column_count, batch_size);
    for (batch_index, (offset, batch_columns)) in batches.into_iter().enumerate() {
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
            let post_args = PostTileArgs {
                column_len: eval_len_u64,
                log_len: eval_log,
                column_count,
                column_offset: offset,
                stage_start,
                inverse: 0,
                threadgroup_lanes: args.threadgroup_lanes,
                coset,
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
    let tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    let poseidon_selection = select_poseidon_batch(state_count, tuning);
    let batch_states = poseidon_selection.columns();
    let batches = column_batch_ranges(state_count, batch_states);
    let pipe_depth = COLUMN_STAGING_PIPE_DEPTH.max(1);
    let mut slots: Vec<Option<PoseidonBatchTicket>> = (0..pipe_depth).map(|_| None).collect();

    for (batch_index, (offset, count)) in batches.into_iter().enumerate() {
        let slot_index = batch_index % pipe_depth;
        if let Some(ticket) = slots[slot_index].take() {
            ticket.wait(states, true)?;
        }
        let element_range = poseidon_element_range(offset, count)?;
        let mut buffer =
            clone_slice_with_stats(&states[element_range.clone()], ColumnStagingPhase::Poseidon);
        let metal_buffer = shared_buffer(&context.device, buffer.as_mut_slice());
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
}

pub fn poseidon_hash_columns(batch: &PoseidonColumnBatch) -> MetalResult<Vec<u64>> {
    if batch.is_empty() {
        return Ok(Vec::new());
    }
    if batch.block_count() == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    let padded_len = batch.padded_len();
    if padded_len == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    let context = metal_context()?;
    let column_count = u32::try_from(batch.columns())
        .map_err(|_| GpuError::InvalidInput("poseidon column count exceeds u32::MAX"))?;
    let block_count = u32::try_from(batch.block_count())
        .map_err(|_| GpuError::InvalidInput("poseidon block count exceeds u32::MAX"))?;
    let padded_len_u32 = u32::try_from(padded_len)
        .map_err(|_| GpuError::InvalidInput("poseidon padded length exceeds u32::MAX"))?;
    let limits = pipeline_limits(&context.poseidon_hash);
    let tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    let selection = select_poseidon_batch(column_count, tuning);
    let batches = column_batch_ranges(column_count, selection.columns());
    let mut result = vec![0u64; batch.columns()];
    let payloads = batch.payloads();
    let pipe_depth = COLUMN_STAGING_PIPE_DEPTH.max(1);
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
        );
        let payload_buffer = shared_buffer(&context.device, payload_chunk.as_mut_slice());
        let count_usize = usize::try_from(count)
            .map_err(|_| GpuError::InvalidInput("poseidon batch count exceeds usize bounds"))?;
        let mut state_chunk = PooledBuffer::zeroed(count_usize * STATE_WIDTH);
        let state_buffer = shared_buffer(&context.device, state_chunk.as_mut_slice());
        let mut slice_chunk = batch
            .rebased_slices(column_offset, count_usize)
            .ok_or_else(|| GpuError::InvalidInput("poseidon descriptor rebasing failed"))?;
        let slice_buffer = shared_buffer(&context.device, slice_chunk.as_mut_slice());
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
            &context.poseidon_hash,
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

pub fn poseidon_hash_columns_fused(batch: &PoseidonColumnBatch) -> MetalResult<Vec<u64>> {
    if batch.is_empty() {
        return Ok(Vec::new());
    }
    if batch.block_count() == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    let padded_len = batch.padded_len();
    if padded_len == 0 {
        return Ok(vec![0; batch.columns()]);
    }
    let context = metal_context()?;
    let column_count = u32::try_from(batch.columns())
        .map_err(|_| GpuError::InvalidInput("poseidon column count exceeds u32::MAX"))?;
    let parent_count = u32::try_from((batch.columns() + 1) / 2)
        .map_err(|_| GpuError::InvalidInput("poseidon parent count exceeds u32::MAX"))?;
    let block_count = u32::try_from(batch.block_count())
        .map_err(|_| GpuError::InvalidInput("poseidon block count exceeds u32::MAX"))?;
    let padded_len_u32 = u32::try_from(padded_len)
        .map_err(|_| GpuError::InvalidInput("poseidon padded length exceeds u32::MAX"))?;
    let limits = pipeline_limits(&context.poseidon_hash);
    let tuning = metal_config::poseidon_tuning(limits.exec_width, limits.max_threads);
    let (threadgroups, threadgroup, logical_threads, states_per_lane) =
        poseidon_dispatch_geometry(column_count, tuning, &limits);
    let mut payload_chunk = clone_slice_with_stats(batch.payloads(), ColumnStagingPhase::Poseidon);
    let payload_buffer = shared_buffer(&context.device, payload_chunk.as_mut_slice());
    let mut slice_chunk = batch
        .rebased_slices(0, batch.columns())
        .ok_or_else(|| GpuError::InvalidInput("poseidon descriptor rebasing failed"))?;
    let slice_buffer = shared_buffer(&context.device, slice_chunk.as_mut_slice());
    let mut hash_chunk = PooledBuffer::zeroed(batch.columns() + parent_count as usize);
    let hash_buffer = shared_buffer(&context.device, hash_chunk.as_mut_slice());
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
    let mut ticket = submit_compute_with_geometry(
        queue,
        queue_index,
        &context.poseidon_trace_fused,
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
    Ok(hash_chunk.as_slice().to_vec())
}

fn shared_buffer<T>(device: &Device, data: &mut [T]) -> Buffer {
    let byte_len = u64::try_from(mem::size_of_val(data))
        .expect("Metal shared buffer length must fit into u64");
    let buffer = device.new_buffer_with_bytes_no_copy(
        data.as_mut_ptr().cast(),
        byte_len,
        MTLResourceOptions::StorageModeShared,
        None,
    );
    buffer.did_modify_range(NSRange {
        location: 0,
        length: byte_len,
    });
    buffer
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
    let mut permit = CommandPermit::new(queue_index);
    let (threadgroups, threadgroup, logical_threads) = match geometry {
        Some((groups, group, logical)) => (groups, group, logical),
        None => {
            let (groups, group) = dispatch_sizes(pipeline, logical_threads);
            (groups, group, logical_threads)
        }
    };

    let kernel_context = profile.map(|params| {
        let groups = threadgroups.width.max(1);
        let width = threadgroup.width.max(1);
        KernelDispatchContext::from_pipeline(params, logical_threads, groups, width, pipeline)
    });

    let command_buffer = queue.new_command_buffer();
    let owned_buffer = command_buffer.to_owned();
    let encoder = command_buffer.new_compute_command_encoder();
    encoder.set_compute_pipeline_state(pipeline);
    configure(encoder);

    let trace_enabled = dispatch_trace_enabled();
    let tracing_start = if trace_enabled {
        trace_dispatch_start(pipeline, logical_threads, &threadgroups, &threadgroup);
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

    command_buffer.commit();
    permit.mark_launched();
    let trace_label = if trace_enabled {
        Some(pipeline.label().to_string())
    } else {
        None
    };
    Ok(DispatchTicket {
        command: owned_buffer,
        trace_label,
        timing_start,
        kernel_context,
        permit,
        adaptive_sample: None,
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

fn clamp_u128_to_u64(value: u128) -> u64 {
    if value > u128::from(u64::MAX) {
        u64::MAX
    } else {
        value as u64
    }
}

fn wait_for_ticket(mut ticket: DispatchTicket) -> MetalResult<()> {
    let trace_label = ticket.trace_label.clone();
    let timing_start = ticket.timing_start;
    ticket.command.wait_until_completed();
    let status = ticket.command.status();
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

fn flatten(columns: &[Vec<u64>]) -> PooledBuffer {
    PooledBuffer::from_columns(columns)
}

fn flatten_with_stats(columns: &[Vec<u64>], phase: ColumnStagingPhase) -> PooledBuffer {
    let start = Instant::now();
    let buffer = PooledBuffer::from_columns(columns);
    record_staging_flatten(phase, start.elapsed());
    buffer
}

fn clone_slice_with_stats(elements: &[u64], phase: ColumnStagingPhase) -> PooledBuffer {
    let start = Instant::now();
    let buffer = PooledBuffer::from_slice(elements);
    record_staging_flatten(phase, start.elapsed());
    buffer
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

fn acquire_buffer(min_capacity: usize) -> Vec<u64> {
    if min_capacity == 0 {
        return Vec::new();
    }
    buffer_pool()
        .lock()
        .expect("buffer pool poisoned")
        .take(min_capacity)
}

#[derive(Default)]
struct BufferPool {
    spare: Vec<Vec<u64>>,
}

impl BufferPool {
    fn take(&mut self, min_capacity: usize) -> Vec<u64> {
        let mut candidate = None;
        let mut best_capacity = usize::MAX;
        for (idx, buffer) in self.spare.iter().enumerate() {
            let capacity = buffer.capacity();
            if capacity >= min_capacity && capacity < best_capacity {
                candidate = Some(idx);
                best_capacity = capacity;
            }
        }
        match candidate {
            Some(idx) => self.spare.swap_remove(idx),
            None => Vec::with_capacity(min_capacity),
        }
    }

    fn recycle(&mut self, mut buffer: Vec<u64>) {
        if buffer.capacity() == 0 {
            return;
        }
        buffer.clear();
        self.spare.push(buffer);
        if self.spare.len() > MAX_BUFFER_POOL_BUFFERS {
            self.spare.sort_unstable_by_key(|buf| buf.capacity());
            self.spare.truncate(MAX_BUFFER_POOL_BUFFERS);
        }
    }

    #[cfg(test)]
    fn len_for_tests(&self) -> usize {
        self.spare.len()
    }
}

struct PooledBuffer {
    data: Vec<u64>,
}

impl PooledBuffer {
    fn from_columns(columns: &[Vec<u64>]) -> Self {
        let len = columns.first().map_or(0, Vec::len);
        let total_len = len.saturating_mul(columns.len());
        let mut data = acquire_buffer(total_len);
        data.clear();
        for column in columns {
            data.extend_from_slice(column);
        }
        Self { data }
    }

    fn from_slice(elements: &[u64]) -> Self {
        let mut data = acquire_buffer(elements.len());
        data.clear();
        data.extend_from_slice(elements);
        Self { data }
    }

    fn zeroed(len: usize) -> Self {
        let mut data = acquire_buffer(len);
        data.resize(len, 0);
        Self { data }
    }

    fn as_slice(&self) -> &[u64] {
        &self.data
    }

    fn as_mut_slice(&mut self) -> &mut [u64] {
        self.data.as_mut_slice()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if self.data.capacity() == 0 {
            self.data.clear();
            return;
        }
        let buffer = mem::take(&mut self.data);
        buffer_pool()
            .lock()
            .expect("buffer pool poisoned")
            .recycle(buffer);
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

    fn acquire(&self) {
        let mut guard = self.state.lock().expect("command semaphore poisoned");
        while *guard >= self.limit {
            guard = self
                .condvar
                .wait(guard)
                .expect("command semaphore wait failed");
        }
        *guard += 1;
    }

    fn release(&self) {
        let mut guard = self.state.lock().expect("command semaphore poisoned");
        if *guard == 0 {
            return;
        }
        *guard -= 1;
        self.condvar.notify_one();
    }

    fn limit(&self) -> usize {
        self.limit
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
    semaphore: &'static CommandSemaphore,
    queue_index: usize,
    released: bool,
    launched: bool,
}

impl CommandPermit {
    fn new(queue_index: usize) -> Self {
        let semaphore = command_semaphore();
        semaphore.acquire();
        Self {
            semaphore,
            queue_index,
            released: false,
            launched: false,
        }
    }

    fn mark_launched(&mut self) {
        if !self.launched {
            record_queue_launch(self.queue_index);
            self.launched = true;
        }
    }

    fn complete(&mut self) {
        if self.launched {
            record_queue_completion(self.queue_index);
            self.launched = false;
        }
        if !self.released {
            self.semaphore.release();
            self.released = true;
        }
    }
}

impl Drop for CommandPermit {
    fn drop(&mut self) {
        if self.launched || !self.released {
            self.complete();
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
        pipeline = pipeline.label(),
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

fn restore_range(columns: &mut [Vec<u64>], range: Range<usize>, buffer: &[u64], extent: usize) {
    if range.is_empty() {
        return;
    }
    restore(&mut columns[range], buffer, extent);
}

fn restore(columns: &mut [Vec<u64>], buffer: &[u64], extent: usize) {
    for (column, chunk) in columns.iter_mut().zip(buffer.chunks_exact(extent)) {
        column.copy_from_slice(chunk);
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
    bn254_validate_log(log_size)?;
    let n = 1usize << log_size;
    let stage_span = n / 2;
    let mut twiddles = vec![Bn254Scalar::zero(); (log_size as usize) * stage_span];
    let max_log = bn254_two_adicity();
    let mut omega = Bn254Scalar::from(Bn254Fr::ROOT_OF_UNITY);
    let exponent = 1u64 << (max_log - log_size);
    omega = omega.pow_u64(exponent);
    for stage in 0..log_size {
        let len = 1usize << (stage + 1);
        let half = len / 2;
        let stride = n / len;
        let stage_offset = stage as usize * stage_span;
        let stride_twiddle = omega.pow_u64(stride as u64);
        let mut value = Bn254Scalar::one();
        for pair in 0..half {
            if pair == 0 {
                value = Bn254Scalar::one();
            } else {
                value = value.mul(stride_twiddle);
            }
            twiddles[stage_offset + pair] = value;
        }
        if half < stage_span {
            for idx in half..stage_span {
                twiddles[stage_offset + idx] = twiddles[stage_offset + idx % half];
            }
        }
    }
    Ok(twiddles)
}

fn bn254_stage_twiddles_limbs(log_size: u32) -> MetalResult<Vec<[u64; BN254_LIMBS]>> {
    let scalars = bn254_stage_twiddles_scalars(log_size)?;
    let twiddles: Vec<[u64; BN254_LIMBS]> = scalars
        .into_iter()
        .map(|scalar| bn254_scalar_to_canonical_limbs(&scalar))
        .collect();
    validate_bn254_twiddles_shape(log_size, &twiddles)?;
    Ok(twiddles)
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

fn compute_stage_twiddles(log_len: u32, inverse: bool) -> Vec<u64> {
    if log_len == 0 {
        return Vec::new();
    }

    let len = 1u64 << log_len;
    let base_exponent = (FIELD_MODULUS - 1) >> log_len;
    let mut omega = goldilocks_pow(GOLDILOCKS_GENERATOR, base_exponent);
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
        MAX_QUEUE_FANOUT, QueuePolicy, STATE_WIDTH, default_queue_column_threshold,
        lde_tile_stage_limit, parse_queue_fanout_override, parse_queue_threshold_override,
        poseidon_element_range, poseidon_recommended_states_per_batch, post_tile_stage_start,
        queue_total_columns_hint, select_poseidon_batch,
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
        assert_eq!(lde_tile_stage_limit(5), 5);
        assert_eq!(lde_tile_stage_limit(18), 12);
        assert_eq!(lde_tile_stage_limit(64), 8);
    }

    #[test]
    fn lde_tile_stage_limit_respects_device_hints() {
        metal_config::set_device_hints_for_tests(Some(DeviceHints::new(
            false,
            true,
            true,
            24 * 1024 * 1024 * 1024,
        )));
        assert_eq!(lde_tile_stage_limit(18), 14);
        metal_config::set_device_hints_for_tests(None);
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
        let expected = tuning
            .threadgroup_lanes
            .saturating_mul(tuning.states_per_lane)
            .saturating_mul(metal_config::poseidon_batch_multiplier());
        assert_eq!(
            poseidon_recommended_states_per_batch(expected * 2, tuning),
            expected
        );
    }

    #[test]
    fn poseidon_batch_selection_respects_remaining_states() {
        let tuning = metal_config::PoseidonTuning {
            threadgroup_lanes: 64,
            states_per_lane: 4,
        };
        let total_states = 32;
        let selection = select_poseidon_batch(total_states, tuning);
        assert_eq!(selection.columns(), total_states);
        let sample = selection.sample_for(total_states);
        assert!(sample.is_some(), "adaptive sample expected");
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
    use metal::Device;

    use super::*;

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
    fn flatten_bn254_twiddles_concatenates_limbs() {
        let inputs = [[1u64, 2, 3, 4], [5, 6, 7, 8]];
        let flat = super::flatten_bn254_twiddles(&inputs);
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
        let ok = super::validate_bn254_twiddles_shape(2, &[[0u64; 4]; 2]).is_ok();
        assert!(ok, "expected shape to be valid");

        let err = super::validate_bn254_twiddles_shape(2, &[[0u64; 4]; 1])
            .expect_err("expected shape error");
        assert!(matches!(err, GpuError::InvalidInput(_)));
    }

    #[test]
    fn bn254_twiddle_len_helpers_match_shape() {
        assert_eq!(super::bn254_fft_twiddle_len(2).unwrap(), 4);
        assert!(super::bn254_fft_twiddle_len(0).is_err());

        assert_eq!(super::bn254_lde_twiddle_len(2, 1).unwrap(), 12);
        assert!(super::bn254_lde_twiddle_len(0, 1).is_err());
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
        match super::bn254_status() {
            Ok(()) => {}
            Err(GpuError::Unsupported(_)) => return,
            Err(err) => panic!("BN254 status smoke test failed: {err}"),
        }
    }
}

#[cfg(all(test, feature = "fastpq-gpu", target_os = "macos"))]
mod tests {
    use std::{thread, time::Duration};

    use fastpq_isi::{CANONICAL_PARAMETER_SETS, poseidon as cpu_poseidon};

    use super::{ensure_multi_queue_env, unwrap_or_skip, *};
    use crate::fft::Planner;

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
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let scenarios = [(3, 2), (10, 2), (14, 1)];

        for (log_size, column_count) in scenarios {
            let mut cpu_columns = sample_fft_columns(log_size, column_count);
            let mut metal_columns = cpu_columns.clone();

            planner.fft_columns(&mut cpu_columns);
            if unwrap_or_skip(super::fft_columns(&mut metal_columns, log_size), "fft").is_none() {
                return;
            }
            assert_eq!(cpu_columns, metal_columns);

            planner.ifft_columns(&mut cpu_columns);
            if unwrap_or_skip(super::ifft_columns(&mut metal_columns, log_size), "ifft").is_none() {
                return;
            }
            assert_eq!(cpu_columns, metal_columns);
        }
    }

    #[test]
    fn lde_matches_cpu_reference() {
        ensure_multi_queue_env();
        let params = CANONICAL_PARAMETER_SETS[0];
        let planner = Planner::new(&params);
        let trace_log = 3;
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
        let Some(gpu_eval) = unwrap_or_skip(
            super::lde_columns(&coeffs, trace_log, planner.blowup_log(), params.omega_coset),
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
    fn poseidon_dispatch_geometry_meets_minimum_threads() {
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
        assert!(logical_threads >= u64::from(super::POSEIDON_TARGET_THREADS));
        assert_eq!(states_per_lane, 4);
        assert_eq!(group.width, 32);
        assert!(groups.width >= 1);
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
        let twiddles = super::compute_stage_twiddles(5, false);
        assert_eq!(twiddles, expected);

        let inverse_twiddles = super::compute_stage_twiddles(5, true);
        for (forward, inverse) in expected.iter().zip(inverse_twiddles.iter()) {
            assert_eq!(*inverse, super::goldilocks_inv(*forward));
        }
    }
    #[test]
    fn buffer_pool_recycles_vectors() {
        let mut pool = BufferPool::default();
        assert_eq!(pool.len_for_tests(), 0);

        let buffer = pool.take(16);
        assert!(buffer.capacity() >= 16);
        pool.recycle(buffer);
        assert_eq!(pool.len_for_tests(), 1);

        let buffer = pool.take(8);
        assert!(buffer.capacity() >= 8);
        assert_eq!(pool.len_for_tests(), 0);
    }

    #[test]
    fn pooled_buffer_zeroed_is_preinitialized() {
        let buffer = PooledBuffer::zeroed(4);
        assert_eq!(buffer.as_slice(), &[0, 0, 0, 0]);
    }

    #[test]
    fn queue_stats_capture_overlap() {
        super::enable_queue_depth_stats(true);
        {
            let mut first = super::CommandPermit::new(0);
            first.mark_launched();
            thread::sleep(Duration::from_millis(1));
            let mut second = super::CommandPermit::new(0);
            second.mark_launched();
            thread::sleep(Duration::from_millis(1));
            second.complete();
            first.complete();
        }
        let stats = super::take_queue_depth_stats().expect("stats captured");
        super::enable_queue_depth_stats(false);
        assert!(stats.dispatch_count >= 2);
        assert!(stats.max_in_flight >= 1);
        assert!(stats.overlap_ms > 0.0);
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
        assert_eq!(descriptors.len(), 5);
        for name in [
            "fastpq_fft_columns",
            "fastpq_fft_post_tiling",
            "fastpq_lde_columns",
            "poseidon_permute",
            "poseidon_hash_columns",
        ] {
            assert!(
                descriptors
                    .iter()
                    .any(|descriptor| descriptor.entry_point == name),
                "missing descriptor for {name}"
            );
        }
    }
}
