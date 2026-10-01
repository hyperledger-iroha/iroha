//! Bounded SHA3 leaf/parent execution with clearing canonical frames.
//!
//! CPU SIMD is feature-gated with deterministic scalar fallback. Required Metal
//! runs its own exact public SHA3 known answers before any private staging.
//! TODO: Qualify native resource/performance limits and implement CUDA execution.
use super::{
    compact_sha3::{MAX_PREPARED_HASH_FRAME_BYTES, PreparedHashFrame},
    deep_binding::{BindingError, Context, Oracle},
    masked_quotient::{checked_add as add, checked_mul as mul},
};
use crate::{DigestExecutionV1, Error, Result};
use fastpq_isi::keccak256::{Sha3_256V1, Sha3Digest256V1 as Digest};
use rayon::prelude::*;
/// Bound independent of worker count, witness, transcript or device availability.
pub(super) const CAPACITY: usize = 1024;
/// Reject unsupported required-device mode before entropy or private-row callbacks.
#[cfg_attr(
    not(feature = "fastpq-gpu"),
    expect(
        clippy::unnecessary_wraps,
        reason = "required-device and quarantine failures exist with the hardware feature"
    )
)]
pub(super) fn preflight_execution(execution: DigestExecutionV1) -> Result<()> {
    #[cfg(feature = "fastpq-gpu")]
    if crate::gpu::transform_completion_uncertain_v1() {
        return Err(Error::NativeDigestExecution{details:"device completion uncertain; private staging may remain, restart process before proving".into()});
    }
    #[cfg(feature = "fastpq-gpu")]
    if let DigestExecutionV1::Device(backend) = execution {
        return crate::keccak_gpu::preflight(backend);
    }
    let _ = execution;
    Ok(())
}
/// Conservative fixed payload for source cells, exact encoded bodies, result slots,
/// retained per-job SHA3 owners and every concurrent Keccak permutation scratch.
/// Paired SIMD retains state25 + saved25 + scratch35 two-lane cells.
/// Charge that per job conservatively; scalar execution uses fewer cells.
pub(super) fn payload_bytes(binding: &Context, oracle: Oracle, leaf_bytes: usize) -> Result<usize> {
    let body = binding
        .tree_frame_bytes(oracle)
        .map_err(|e| binding_error(&e))?;
    if body > MAX_PREPARED_HASH_FRAME_BYTES {
        return Err(invalid("compact tree body exceeds its fixed bound"));
    }
    add(
        crate::keccak_batch::device_payload_bytes(CAPACITY, mul(CAPACITY, body)?)?,
        mul(
            CAPACITY,
            add(
                leaf_bytes,
                add(
                    body,
                    Digest::BYTES
                        + 2 * size_of::<usize>()
                        + size_of::<Result<()>>()
                        + size_of::<Result<PreparedHashFrame>>()
                        + size_of::<PreparedHashFrame>()
                        + Sha3_256V1::RETAINED_BYTES
                        + size_of::<crate::keccak_batch::Job<'_>>()
                        + 85 * 2 * size_of::<u64>(),
                )?,
            )?,
        )?,
    )
}
/// Hash already-owned canonical bodies in fixed index order.
#[cfg(any(test, feature = "fastpq-gpu", feature = "simd"))]
pub(super) fn execute_prepared(
    frames: &[PreparedHashFrame],
    output: &mut [[u8; 32]],
    execution: DigestExecutionV1,
) -> Result<()> {
    preflight_execution(execution)?;
    if frames.is_empty() || frames.len() > CAPACITY || frames.len() != output.len() {
        return Err(invalid(
            "compact prepared batch has another exact bounded shape",
        ));
    }
    let jobs = frames
        .iter()
        .map(PreparedHashFrame::job)
        .collect::<Vec<_>>();
    match execution {
        DigestExecutionV1::Cpu => crate::keccak_batch::hash_cpu(&jobs, output),
        #[cfg(feature = "fastpq-gpu")]
        DigestExecutionV1::Device(backend) => crate::keccak_gpu::hash(backend, &jobs, output),
    }
}
/// Hash a bounded prefix into caller-owned clearing digest storage. Every worker
/// finishes before its ordered error is returned; no completion order enters H.
pub(super) fn hash(
    binding: &Context,
    oracle: Oracle,
    indices: &[usize],
    payloads: &[u8],
    leaf_bytes: usize,
    output: &mut [[u8; 32]],
    execution: DigestExecutionV1,
) -> Result<()> {
    preflight_execution(execution)?;
    if indices.is_empty()
        || indices.len() > CAPACITY
        || output.len() != indices.len()
        || leaf_bytes == 0
        || payloads.len() != mul(indices.len(), leaf_bytes)?
    {
        return Err(invalid(
            "compact leaf batch has another exact bounded shape",
        ));
    }
    #[cfg(any(feature = "simd", feature = "fastpq-gpu"))]
    {
        let frames = indices
            .par_iter()
            .zip(payloads.par_chunks_exact(leaf_bytes))
            .map(|(&index, bytes)| {
                let index =
                    u32::try_from(index).map_err(|_| invalid("compact leaf index exceeds u32"))?;
                binding.prepare_leaf(oracle, index, bytes)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        execute_prepared(&frames, output, execution)
    }
    #[cfg(not(any(feature = "simd", feature = "fastpq-gpu")))]
    {
        let results = output
            .par_iter_mut()
            .zip(payloads.par_chunks_exact(leaf_bytes))
            .zip(indices.par_iter())
            .map(|((out, bytes), &index)| {
                let index =
                    u32::try_from(index).map_err(|_| invalid("compact leaf index exceeds u32"))?;
                *out = binding
                    .hash_leaf(oracle, index, bytes)
                    .map_err(|e| binding_error(&e))?
                    .into_bytes();
                Ok(())
            })
            .collect::<Vec<Result<()>>>();
        for result in results {
            result?;
        }
        Ok(())
    }
}
fn binding_error(error: &BindingError) -> Error {
    Error::InvalidTraceShape {
        details: format!("compact leaf batch: {error}"),
    }
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}
#[cfg(test)]
#[path = "deep_leaf_batch/tests.rs"]
mod tests;
