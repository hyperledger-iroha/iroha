//! Bounded independent Merkle parents with canonical CPU/device parity.
//!
//! A stripe run reuses its clearing right-child allocation for output. The shared
//! leaf executor owns device results and bounds all canonical framing scratch.
//!
//! One tree level of one stripe is hashed as one ordered run. CPU execution
//! splits it into fixed-size sequential jobs, pairing parents through the
//! paired kernel where it is built in; required-device execution stages the
//! same run in bounded batches.

use rayon::prelude::*;

use super::deep_binding::{Context, Oracle};
use crate::{DigestExecutionV1, Error, Result};
use fastpq_isi::keccak256::Sha3Digest256V1 as Digest;

/// Stage one bounded batch of parents for a required device.
///
/// Shared leaf payload accounting with a body extent of at least 96 bytes
/// covers frames, jobs and returned digests; borrowed left/right slices remain
/// charged by their existing clearing owners.
#[cfg(feature = "fastpq-gpu")]
fn hash_staged_batch(
    binding: &Context,
    oracle: Oracle,
    level: u32,
    indices: &[usize],
    left: &[[u8; 32]],
    right: &mut [[u8; 32]],
    execution: DigestExecutionV1,
) -> Result<()> {
    let frames = indices
        .par_iter()
        .zip(left.par_iter())
        .zip(right.par_iter())
        .map(|((&index, &left), &right)| {
            let index =
                u32::try_from(index).map_err(|_| invalid("DEEP parent batch index exceeds u32"))?;
            binding.prepare_parent(oracle, level, index, digest(left), digest(right))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    super::deep_leaf_batch::execute_prepared(&frames, right, execution)
}

/// Parents hashed by one sequential CPU job of a level run; a run's job count
/// is its length over this extent, independent of the worker count.
const RUN_JOB_PARENTS: usize = 1024;

/// Hash one ordered level run in place.
///
/// `right[offset]` becomes the parent of `left[offset]` and `right[offset]`,
/// addressed at `first_index + offset * index_step` on `level`. A stripe's
/// parents on one level form exactly such an arithmetic progression. The error
/// of the earliest failing offset is returned after every job finishes.
///
/// CPU execution splits the run into fixed-size sequential jobs, hashing two
/// parents at a time through the paired kernel when it is built in.
/// Required-device execution stages the run in bounded batches.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per canonical parent coordinate and child run"
)]
pub(super) fn hash_run_in_place(
    binding: &Context,
    oracle: Oracle,
    level: usize,
    first_index: usize,
    index_step: usize,
    left: &[[u8; 32]],
    right: &mut [[u8; 32]],
    execution: DigestExecutionV1,
) -> Result<()> {
    if right.is_empty()
        || right.len() > super::deep_leaf_batch::MAX_RUN_LEAVES
        || left.len() != right.len()
    {
        return Err(invalid("DEEP parent run has another exact bounded shape"));
    }
    super::deep_leaf_batch::preflight_execution(execution)?;
    let level = u32::try_from(level).map_err(|_| invalid("DEEP parent batch level exceeds u32"))?;
    let index_at = |offset: usize| {
        offset
            .checked_mul(index_step)
            .and_then(|distance| distance.checked_add(first_index))
            .ok_or_else(|| invalid("DEEP parent run index overflow"))
    };
    #[cfg(feature = "fastpq-gpu")]
    if matches!(execution, DigestExecutionV1::Device(_)) {
        const BATCH: usize = super::deep_leaf_batch::CAPACITY;
        let mut indices = [0; BATCH];
        for (batch, (right, left)) in right.chunks_mut(BATCH).zip(left.chunks(BATCH)).enumerate() {
            for (position, slot) in indices[..right.len()].iter_mut().enumerate() {
                *slot = index_at(batch * BATCH + position)?;
            }
            hash_staged_batch(
                binding,
                oracle,
                level,
                &indices[..right.len()],
                left,
                right,
                execution,
            )?;
        }
        return Ok(());
    }
    let job = |start: usize, left: &[[u8; 32]], right: &mut [[u8; 32]]| {
        hash_run_job(binding, oracle, level, &index_at, start, left, right)
    };
    // A run of one job gains nothing from the pool.
    if right.len() <= RUN_JOB_PARENTS {
        return job(0, left, right);
    }
    let results = right
        .par_chunks_mut(RUN_JOB_PARENTS)
        .zip(left.par_chunks(RUN_JOB_PARENTS))
        .enumerate()
        .map(|(ordinal, (right, left))| job(ordinal * RUN_JOB_PARENTS, left, right))
        .collect::<Vec<Result<()>>>();
    results.into_iter().collect()
}

/// Natural `u32` tree index of the parent at one run offset.
fn run_parent_index(index_at: &impl Fn(usize) -> Result<usize>, offset: usize) -> Result<u32> {
    u32::try_from(index_at(offset)?).map_err(|_| invalid("DEEP parent batch index exceeds u32"))
}

/// One sequential scalar job over consecutive parents.
#[cfg(not(feature = "simd"))]
fn hash_run_job(
    binding: &Context,
    oracle: Oracle,
    level: u32,
    index_at: &impl Fn(usize) -> Result<usize>,
    start: usize,
    left: &[[u8; 32]],
    right: &mut [[u8; 32]],
) -> Result<()> {
    for (position, (&left, right)) in left.iter().zip(right.iter_mut()).enumerate() {
        let index = run_parent_index(index_at, start + position)?;
        *right = binding
            .hash_parent(oracle, level, index, digest(left), digest(*right))
            .map_err(|error| Error::InvalidTraceShape {
                details: format!("DEEP parent batch: {error}"),
            })?
            .into_bytes();
    }
    Ok(())
}

/// One sequential paired job: prepare the canonical frames of two consecutive
/// parents and hash them together through the paired kernel. A final odd
/// parent, and a host without the vector extension, use the scalar owner.
#[cfg(feature = "simd")]
fn hash_run_job(
    binding: &Context,
    oracle: Oracle,
    level: u32,
    index_at: &impl Fn(usize) -> Result<usize>,
    start: usize,
    left: &[[u8; 32]],
    right: &mut [[u8; 32]],
) -> Result<()> {
    for (pair, (left, right)) in left.chunks(2).zip(right.chunks_mut(2)).enumerate() {
        let offset = start + 2 * pair;
        let first = binding.prepare_parent(
            oracle,
            level,
            run_parent_index(index_at, offset)?,
            digest(left[0]),
            digest(right[0]),
        )?;
        if right.len() == 2 {
            let second = binding.prepare_parent(
                oracle,
                level,
                run_parent_index(index_at, offset + 1)?,
                digest(left[1]),
                digest(right[1]),
            )?;
            crate::keccak_batch::hash_cpu_in_order(&[first.job(), second.job()], right)?;
        } else {
            crate::keccak_batch::hash_cpu_in_order(&[first.job()], right)?;
        }
    }
    Ok(())
}

fn digest(words: [u8; 32]) -> Digest {
    Digest::from_bytes(words)
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::secret_polynomial::SecretPolynomial;

    #[test]
    fn parent_runs_match_canonical_hashes_across_job_boundaries_and_worker_counts() {
        let binding = Context::new(b"ordered parent level run").unwrap();
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    // Both 2^23-leaf trees have 2^22 parents on level one: runs
                    // of one parent, one job, a ragged tail and several jobs fit.
                    for (oracle, count, first, step) in [
                        (Oracle::QuotientAndMask, 1, 5, 1),
                        (Oracle::Row, 1024, 0, 64),
                        (Oracle::QuotientAndMask, 1031, 3, 2),
                        (Oracle::Row, 4097, 7, 4),
                        // The first FRI tree has 2^18 parents there, the last 64.
                        (Oracle::Fri(0), 2049, 1, 3),
                        (Oracle::Fri(4), 64, 0, 1),
                    ] {
                        let mut left = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
                        let mut right = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
                        for (index, (left, right)) in
                            left.iter_mut().zip(right.iter_mut()).enumerate()
                        {
                            *left = core::array::from_fn(|lane| {
                                (5 * index + lane + 1).to_le_bytes()[0]
                            });
                            *right = core::array::from_fn(|lane| {
                                (9 * index + lane + 13).to_le_bytes()[0]
                            });
                        }
                        let expected = left
                            .iter()
                            .zip(right.iter())
                            .enumerate()
                            .map(|(offset, (&left, &right))| {
                                binding
                                    .hash_parent(
                                        oracle,
                                        1,
                                        u32::try_from(first + offset * step).unwrap(),
                                        digest(left),
                                        digest(right),
                                    )
                                    .unwrap()
                                    .into_bytes()
                            })
                            .collect::<Vec<_>>();
                        hash_run_in_place(
                            &binding,
                            oracle,
                            1,
                            first,
                            step,
                            &left,
                            &mut right,
                            DigestExecutionV1::Cpu,
                        )
                        .unwrap();
                        assert_eq!(&right[..], expected, "{oracle:?} run of {count}");
                    }
                });
        }
    }

    #[test]
    fn malformed_parent_runs_fail_with_the_earliest_offset_error() {
        let binding = Context::new(b"parent run rejection").unwrap();
        let cpu = DigestExecutionV1::Cpu;
        let mut right = [[1; 32]; 2];
        // Empty and unequal child runs, level zero and an index progression
        // that overflows are all refused.
        assert!(hash_run_in_place(&binding, Oracle::Row, 1, 0, 1, &[], &mut [], cpu).is_err());
        assert!(
            hash_run_in_place(
                &binding,
                Oracle::Row,
                1,
                0,
                1,
                &[[1; 32]; 1],
                &mut right,
                cpu
            )
            .is_err()
        );
        // Level zero holds leaves and level 24 lies above the 23-level root.
        for level in [0, 24] {
            assert!(
                hash_run_in_place(
                    &binding,
                    Oracle::Row,
                    level,
                    0,
                    1,
                    &[[1; 32]; 2],
                    &mut right,
                    cpu
                )
                .is_err()
            );
        }
        assert!(
            hash_run_in_place(
                &binding,
                Oracle::Row,
                1,
                usize::MAX,
                1,
                &[[1; 32]; 2],
                &mut right,
                cpu
            )
            .is_err()
        );
        // Fri(4) has 64 parents on level one. A run stepping past them fails at
        // its first out-of-range offset in every worker schedule.
        let count = 3000;
        let left = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let mut right = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
                    assert!(
                        hash_run_in_place(
                            &binding,
                            Oracle::Fri(4),
                            1,
                            0,
                            1,
                            &left,
                            &mut right,
                            cpu
                        )
                        .is_err()
                    );
                    // Offsets below the bound were hashed; none above it was.
                    assert_ne!(right[0], [0; 32]);
                    assert_ne!(right[63], [0; 32]);
                    assert_eq!(right[64], [0; 32]);
                });
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn unavailable_cuda_parent_batches_fail_before_private_hashing() {
        let binding = Context::new(b"device unavailable").unwrap();
        let mut right = [[0xff; 32]; 1];
        assert!(
            hash_run_in_place(
                &binding,
                Oracle::Row,
                1,
                0,
                1,
                &[[0xff; 32]],
                &mut right,
                DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Cuda)
            )
            .is_err()
        );
        assert_eq!(right, [[0xff; 32]]);
    }
}
