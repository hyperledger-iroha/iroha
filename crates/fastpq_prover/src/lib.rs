//! FASTPQ lane prover.
//!
//! This crate provides the production FASTPQ-ISI prover and verifier
//! implementation.  It exposes deterministic commitments, wiring to the
//! sole V1 parameter set, and the backend that drives the
//! end-to-end STARK pipeline.  Downstream callers interact with the canonical
//! constructor which initialises the production backend.
//!
//! The public API is intentionally narrow and uses Norito-friendly types so
//! callers can persist artifacts without pulling in Serde.

#![deny(unsafe_code)]
#![deny(missing_docs)]
#![allow(unexpected_cfgs)]
mod axt_binding;
mod backend;
mod batch;
#[cfg(any(test, feature = "dev-tools", feature = "fastpq-gpu"))]
mod bn254;
mod bn254_poseidon;
#[cfg(feature = "fastpq-gpu")]
mod bn254_poseidon_params;
mod cyclotomic;
mod digest;
mod error;
#[cfg(any(test, feature = "dev-tools", feature = "fastpq-gpu"))]
mod fastpq_cuda;
mod fft;
pub mod gadgets;
#[cfg(feature = "fastpq-gpu")]
#[path = "gpu.rs"]
mod gpu;
#[cfg(not(feature = "fastpq-gpu"))]
#[path = "gpu_stub.rs"]
mod gpu;
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
mod metal;
mod metal_config;
mod ordering;
pub(crate) mod overrides;
mod packing;
mod poseidon;
mod poseidon_manifest;
mod proof;
mod semantics;
mod trace;
pub use axt_binding::{
    AXT_FASTPQ_BATCH_SEAL_METADATA_KEY, AXT_FASTPQ_BINDING_METADATA_KEY,
    AXT_FASTPQ_COMMITTED_AMOUNT_METADATA_KEY, AXT_FASTPQ_DA_COMMITMENT_METADATA_KEY,
    AXT_FASTPQ_EXPIRY_SLOT_METADATA_KEY, AXT_FASTPQ_MANIFEST_ROOT_METADATA_KEY,
    AXT_FASTPQ_REMOTE_SPEND_CLAIMS_METADATA_KEY, AxtFastpqProofPayload, AxtVerifiedProof,
    DEFAULT_PARAMETER as AXT_DEFAULT_PARAMETER, MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES,
    axt_proof_blob_from_bound_batch, axt_proof_envelope_from_bound_batch, batch_manifest_sha256,
    bind_axt_batch, bind_axt_batch_with_committed_amount, bind_axt_batch_with_proof_metadata,
    canonicalize_binding, embedded_axt_binding, encode_axt_fastpq_payload,
    set_axt_remote_spend_claims, transition_batch_from_model, transition_batch_to_model,
    validate_axt_transfer_claim_binding, verify_axt_bound_batch, verify_axt_proof_blob,
    verify_axt_proof_envelope, verify_axt_proof_envelope_with_outer_metadata,
};
pub use backend::{
    ExecutionMode, PoseidonExecutionMode, clear_execution_mode_observer,
    set_execution_mode_observer,
};
#[cfg(feature = "dev-tools")]
#[doc(hidden)]
pub use backend::{hash_lde_leaves, lde_chunk_size, merkle_paths_for_queries};
pub use batch::{
    OperationKind, PublicInputs, StateTransition, TRANSITION_BATCH_SCHEMA_NAME, TransitionBatch,
};
pub use bn254_poseidon::{
    Bn254PoseidonBatchSlice, PendingBn254PoseidonWordBatch, preflight_bn254_poseidon_word_batches,
    try_hash_bn254_poseidon_word_batches, try_submit_bn254_poseidon_word_batches,
};
pub use digest::trace_commitment;
pub use error::{Error, Result};
#[cfg(feature = "dev-tools")]
#[doc(hidden)]
pub use fastpq_cuda::{CudaBackendError, fastpq_bn254_fft, fastpq_bn254_lde};
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
pub use packing::{LIMB_BYTES, PackedBytes, pack_bytes};
#[cfg(feature = "fastpq-gpu")]
pub use poseidon::preflight_gpu_backend as preflight_poseidon_gpu_backend;
pub use poseidon::{FIELD_MODULUS, PoseidonSponge, hash_field_elements};
#[cfg(any(test, feature = "dev-tools"))]
pub use proof::verify_raw_statement;
pub use proof::{Proof, Prover, VerifyLimits, verify, verify_with_limits};
pub use semantics::{ProofSemantics, validate_batch_semantics};
#[cfg(feature = "dev-tools")]
#[doc(hidden)]
pub use trace::merkle_root;
#[cfg(all(feature = "dev-tools", feature = "fastpq-gpu"))]
#[doc(hidden)]
pub use trace::{
    ColumnDigests, PoseidonColumnBatch, hash_columns_cpu_batch_inputs, hash_columns_gpu_batch,
    hash_columns_gpu_with_first_level,
};
pub use trace::{
    PoseidonPipelinePolicy, RowUsage, Trace, TraceColumn, build_trace,
    clear_poseidon_gpu_event_observer, clear_poseidon_pipeline_observer,
    set_poseidon_gpu_event_observer, set_poseidon_pipeline_observer,
};
#[cfg(all(feature = "dev-tools", feature = "fastpq-gpu"))]
#[doc(hidden)]
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
pub use poseidon_manifest::{
    PoseidonManifest, poseidon_manifest, poseidon_manifest_sha256, poseidon_profile_id,
    poseidon_profile_sha256,
};
