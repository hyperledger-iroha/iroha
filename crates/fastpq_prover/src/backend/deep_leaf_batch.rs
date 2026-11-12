//! Bounded SHA3 leaf/parent execution with clearing canonical frames.
//!
//! CPU SIMD is feature-gated with deterministic scalar fallback. Required Metal
//! runs its own exact public SHA3 known answers before any private staging.
//!
//! A commitment hashes one stripe of leaves as one ordered run. CPU execution
//! splits a run into fixed-size sequential jobs that pack and hash their own
//! leaves, two at a time through the paired kernel where it is built in, so
//! the workers stay busy for the whole stripe and no payload matrix is staged.
//! Required-device execution stages the same run in bounded batches. Digests
//! land at their run offset in every mode; neither worker count nor
//! completion order enters a commitment.
//! TODO: Qualify native resource/performance limits and implement CUDA execution.
use super::{
    compact_sha3::{MAX_PREPARED_HASH_FRAME_BYTES, PreparedHashFrame},
    deep_binding::{BindingError, Context, Oracle},
    plan_arithmetic::{checked_add as add, checked_mul as mul},
    secret_polynomial::SecretPolynomial,
};
use crate::{DigestExecutionV1, Error, Result};
use fastpq_isi::keccak256::{Sha3_256V1, Sha3Digest256V1 as Digest};
use rayon::prelude::*;
/// Bound independent of worker count, witness, transcript or device availability.
pub(super) const CAPACITY: usize = 1024;
/// Leaves packed and hashed by one sequential scalar job of a run. A run's job
/// count is its length over this extent, independent of the worker count.
const RUN_JOB_LEAVES: usize = 256;
/// Longest run one call hashes: 2^17 leaves, the largest fixed stripe.
///
/// Such a run has at most `CAPACITY / 2` jobs. A job owns at most two clearing
/// payloads and two canonical frames at a time (paired execution hashes two
/// leaves together), so the batch charge of [`payload_bytes`] also bounds a
/// run. Staged execution holds one batch.
pub(super) const MAX_RUN_LEAVES: usize = CAPACITY / 2 * RUN_JOB_LEAVES;
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
/// Hash one ordered run of leaves whose payloads are produced on demand.
///
/// `index(offset)` is the natural tree index of the leaf at `offset` in the
/// run, and `pack(offset, payload)` writes its exact canonical payload of
/// `leaf_bytes` bytes. `output[offset]` receives that leaf's digest. Every job
/// finishes before the error of the earliest failing offset is returned.
///
/// CPU execution splits the run into fixed-size sequential jobs that pack and
/// hash their own leaves, two at a time through the paired kernel when it is
/// built in. Required-device execution stages the run in bounded batches.
pub(super) fn hash_run(
    binding: &Context,
    oracle: Oracle,
    leaf_bytes: usize,
    index: impl Fn(usize) -> usize + Sync,
    pack: impl Fn(usize, &mut [u8]) -> Result<()> + Sync,
    output: &mut [[u8; 32]],
    execution: DigestExecutionV1,
) -> Result<()> {
    preflight_execution(execution)?;
    if output.is_empty() || output.len() > MAX_RUN_LEAVES || leaf_bytes == 0 {
        return Err(invalid("compact leaf run has another exact bounded shape"));
    }
    #[cfg(feature = "fastpq-gpu")]
    if matches!(execution, DigestExecutionV1::Device(_)) {
        return hash_run_staged(
            binding, oracle, leaf_bytes, &index, &pack, output, execution,
        );
    }
    let job = |start: usize, digests: &mut [[u8; 32]]| {
        hash_run_job(binding, oracle, leaf_bytes, &index, &pack, start, digests)
    };
    // A run of one job gains nothing from the pool.
    if output.len() <= RUN_JOB_LEAVES {
        return job(0, output);
    }
    let results = output
        .par_chunks_mut(RUN_JOB_LEAVES)
        .enumerate()
        .map(|(ordinal, digests)| job(ordinal * RUN_JOB_LEAVES, digests))
        .collect::<Vec<Result<()>>>();
    results.into_iter().collect()
}
/// Natural `u32` tree index of the leaf at one run offset.
fn run_leaf_index(index: &impl Fn(usize) -> usize, offset: usize) -> Result<u32> {
    u32::try_from(index(offset)).map_err(|_| invalid("compact leaf index exceeds u32"))
}
/// One sequential scalar job: pack a leaf, hash it, move to the next.
#[cfg(not(feature = "simd"))]
fn hash_run_job(
    binding: &Context,
    oracle: Oracle,
    leaf_bytes: usize,
    index: &impl Fn(usize) -> usize,
    pack: &impl Fn(usize, &mut [u8]) -> Result<()>,
    start: usize,
    digests: &mut [[u8; 32]],
) -> Result<()> {
    let mut payload = SecretPolynomial::<u8>::zeroed(leaf_bytes)?;
    for (position, digest) in digests.iter_mut().enumerate() {
        let offset = start + position;
        let leaf = run_leaf_index(index, offset)?;
        pack(offset, &mut payload)?;
        *digest = binding
            .hash_leaf(oracle, leaf, &payload)
            .map_err(|e| binding_error(&e))?
            .into_bytes();
    }
    Ok(())
}
/// One sequential paired job: pack two consecutive leaves, prepare their
/// canonical frames and hash them together through the paired kernel. A final
/// odd leaf, and a host without the vector extension, use the scalar owner.
#[cfg(feature = "simd")]
fn hash_run_job(
    binding: &Context,
    oracle: Oracle,
    leaf_bytes: usize,
    index: &impl Fn(usize) -> usize,
    pack: &impl Fn(usize, &mut [u8]) -> Result<()>,
    start: usize,
    digests: &mut [[u8; 32]],
) -> Result<()> {
    let mut payloads = SecretPolynomial::<u8>::zeroed(mul(2, leaf_bytes)?)?;
    let (first, second) = payloads.split_at_mut(leaf_bytes);
    for (pair, digests) in digests.chunks_mut(2).enumerate() {
        let offset = start + 2 * pair;
        pack(offset, first)?;
        let left = binding.prepare_leaf(oracle, run_leaf_index(index, offset)?, first)?;
        if digests.len() == 2 {
            pack(offset + 1, second)?;
            let right = binding.prepare_leaf(oracle, run_leaf_index(index, offset + 1)?, second)?;
            crate::keccak_batch::hash_cpu_in_order(&[left.job(), right.job()], digests)?;
        } else {
            crate::keccak_batch::hash_cpu_in_order(&[left.job()], digests)?;
        }
    }
    Ok(())
}
/// Stage a run for a required device in batches of at most [`CAPACITY`].
#[cfg(feature = "fastpq-gpu")]
fn hash_run_staged(
    binding: &Context,
    oracle: Oracle,
    leaf_bytes: usize,
    index: &(impl Fn(usize) -> usize + Sync),
    pack: &(impl Fn(usize, &mut [u8]) -> Result<()> + Sync),
    output: &mut [[u8; 32]],
    execution: DigestExecutionV1,
) -> Result<()> {
    let mut payloads =
        SecretPolynomial::<u8>::zeroed(mul(output.len().min(CAPACITY), leaf_bytes)?)?;
    let mut indices = [0; CAPACITY];
    for (batch, digests) in output.chunks_mut(CAPACITY).enumerate() {
        let start = batch * CAPACITY;
        let staged = &mut payloads[..digests.len() * leaf_bytes];
        let results = staged
            .par_chunks_exact_mut(leaf_bytes)
            .enumerate()
            .map(|(position, payload)| pack(start + position, payload))
            .collect::<Vec<Result<()>>>();
        results.into_iter().collect::<Result<()>>()?;
        for (position, slot) in indices[..digests.len()].iter_mut().enumerate() {
            *slot = index(start + position);
        }
        hash(
            binding,
            oracle,
            &indices[..digests.len()],
            staged,
            leaf_bytes,
            digests,
            execution,
        )?;
    }
    Ok(())
}
fn binding_error(error: &BindingError) -> Error {
    Error::InvalidTraceShape {
        details: error.to_string(),
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
