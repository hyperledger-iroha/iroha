//! FASTPQ lane prover scaffolding.
//!
//! This crate provides the production FASTPQ-ISI prover and verifier
//! implementation.  It exposes deterministic commitments, wiring to the
//! canonical parameter catalogue, and the Stage 6 backend that drives the
//! end-to-end STARK pipeline.  Downstream callers interact with the canonical
//! constructor which initialises the production backend.
//!
//! The public API is intentionally narrow and uses Norito-friendly types so
//! callers can persist artifacts without pulling in Serde.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(unexpected_cfgs)]

mod backend;
mod batch;
mod cyclotomic;
mod digest;
mod error;
mod fastpq_cuda;
pub mod gadgets;
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
mod metal;
mod metal_config;
pub mod ordering;
pub(crate) mod overrides;
pub mod packing;
mod poseidon;
mod poseidon_manifest;
mod proof;
pub mod trace;

#[cfg(feature = "fastpq-gpu")]
#[path = "gpu.rs"]
mod gpu;
#[cfg(not(feature = "fastpq-gpu"))]
#[path = "gpu_stub.rs"]
mod gpu;

mod fft;

pub use backend::{
    Backend, BackendConfig, ExecutionMode, PoseidonExecutionMode, clear_execution_mode_observer,
    set_execution_mode_observer,
};
#[doc(hidden)]
pub use backend::{
    compute_lookup_grand_product, hash_lde_leaves, lde_chunk_size, merkle_paths_for_queries,
};
pub use batch::{OperationKind, PublicInputs, StateTransition, TransitionBatch};
pub use digest::trace_commitment;
pub use error::{Error, Result};
pub use fastpq_cuda::{CudaBackendError, fastpq_fft, fastpq_ifft, fastpq_lde};
pub use fft::Planner;
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
pub use metal::{
    AdaptiveScheduleSnapshot, BatchHeuristicSnapshot, ColumnStagingPhase, ColumnStagingPhaseStats,
    ColumnStagingSample, ColumnStagingStats, CommandLimitSnapshot, CommandLimitSource, KernelKind,
    KernelStatsSample, LdeHostStats, MetalKernelDescriptor, PostTileSample, QueueDepthStats,
    QueueLaneStats, TwiddleCacheStats, adaptive_schedule_snapshot, enable_kernel_stats,
    enable_lde_host_stats, enable_post_tile_stats, enable_queue_depth_stats,
    enable_twiddle_cache_stats, fft_tuning_snapshot, metal_kernel_descriptors,
    poseidon_tuning_snapshot, set_metal_queue_policy, snapshot_queue_depth_stats,
    take_column_staging_stats, take_kernel_stats, take_lde_host_stats, take_post_tile_stats,
    take_queue_depth_stats, take_twiddle_cache_stats,
};
pub use metal_config::{FftTuning, PoseidonTuning};
pub use ordering::ordering_hash;
pub use overrides::{MetalOverrides, apply_metal_overrides};
pub use packing::{LIMB_BYTES, PackedBytes, pack_bytes, unpack_bytes};
pub use poseidon::{FIELD_MODULUS, PoseidonSponge, hash_field_elements};
pub use proof::{Proof, Prover, verify};
pub use trace::{
    ColumnDigests, PoseidonPipelinePolicy, RowUsage, Trace, TraceColumn, build_trace,
    clear_poseidon_pipeline_observer, column_hashes, merkle_root, merkle_root_with_first_level,
    set_poseidon_pipeline_observer,
};
#[cfg(feature = "fastpq-gpu")]
pub use trace::{
    PoseidonPipelineStats, enable_poseidon_pipeline_stats, take_poseidon_pipeline_stats,
};

#[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
/// No-op when the Metal backend is unavailable.
///
/// # Errors
/// This stub never errors because the Metal backend is disabled at compile time and
/// runtime overrides are ignored on unsupported targets.
#[allow(clippy::unnecessary_wraps)]
pub fn set_metal_queue_policy(
    _fanout: Option<usize>,
    _column_threshold: Option<u32>,
) -> std::result::Result<(), &'static str> {
    Ok(())
}
pub use poseidon_manifest::{PoseidonManifest, poseidon_manifest, poseidon_manifest_sha256};
