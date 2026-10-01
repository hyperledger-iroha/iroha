//! Bounded independent Merkle parents with canonical CPU/device parity.
//!
//! A stripe run reuses its clearing right-child allocation for output. The shared
//! leaf executor owns device results and bounds all canonical framing scratch.

use rayon::prelude::*;

use super::deep_binding::{Context, Oracle};
use crate::{DigestExecutionV1, Error, Result};
use fastpq_isi::keccak256::Sha3Digest256V1 as Digest;

/// Hash one fixed tree level in input order. Shared leaf payload accounting with
/// a body extent of at least 96 bytes covers frames, jobs and returned digests;
/// borrowed left/right slices remain charged by their existing clearing owners.
pub(super) fn hash_in_place(
    binding: &Context,
    oracle: Oracle,
    level: usize,
    indices: &[usize],
    left: &[[u8; 32]],
    right: &mut [[u8; 32]],
    execution: DigestExecutionV1,
) -> Result<()> {
    if indices.is_empty()
        || indices.len() > super::deep_leaf_batch::CAPACITY
        || indices.len() != left.len()
        || indices.len() != right.len()
    {
        return Err(invalid("DEEP parent batch has another exact bounded shape"));
    }
    super::deep_leaf_batch::preflight_execution(execution)?;
    let level = u32::try_from(level).map_err(|_| invalid("DEEP parent batch level exceeds u32"))?;
    #[cfg(any(feature = "fastpq-gpu", feature = "simd"))]
    {
        let frames = indices
            .par_iter()
            .zip(left.par_iter())
            .zip(right.par_iter())
            .map(|((&index, &left), &right)| {
                let index = u32::try_from(index)
                    .map_err(|_| invalid("DEEP parent batch index exceeds u32"))?;
                binding.prepare_parent(oracle, level, index, digest(left)?, digest(right)?)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        return super::deep_leaf_batch::execute_prepared(&frames, right, execution);
    }
    #[cfg(not(any(feature = "fastpq-gpu", feature = "simd")))]
    {
        let _ = execution;
        let results = indices
            .par_iter()
            .zip(left.par_iter())
            .zip(right.par_iter_mut())
            .map(|((&index, &left), right)| {
                let index = u32::try_from(index)
                    .map_err(|_| invalid("DEEP parent batch index exceeds u32"))?;
                *right = binding
                    .hash_parent(oracle, level, index, digest(left)?, digest(*right)?)
                    .map_err(|error| Error::InvalidTraceShape {
                        details: format!("DEEP parent batch: {error}"),
                    })?
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

fn digest(words: [u8; 32]) -> Result<Digest> {
    Ok(Digest::from_bytes(words))
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

    fn check_parity(execution: DigestExecutionV1) {
        let binding = Context::new(b"bounded parent executor parity").unwrap();
        for oracle in [
            Oracle::Row,
            Oracle::QuotientAndMask,
            Oracle::Fri(0),
            Oracle::Fri(4),
        ] {
            let (_, _, leaves, _) = oracle.shape().unwrap();
            let count = (leaves / 2).min(super::super::deep_leaf_batch::CAPACITY);
            let indices = (0..count).rev().collect::<Vec<_>>();
            let mut left = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
            let mut right = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
            for (index, (left, right)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
                *left = core::array::from_fn(|lane| (3 * index + lane + 1) as u8);
                *right = core::array::from_fn(|lane| (7 * index + lane + 11) as u8);
            }
            let expected = indices
                .iter()
                .zip(left.iter())
                .zip(right.iter())
                .map(|((&index, &left), &right)| {
                    binding
                        .hash_parent(
                            oracle,
                            1,
                            u32::try_from(index).unwrap(),
                            digest(left).unwrap(),
                            digest(right).unwrap(),
                        )
                        .unwrap()
                        .into_bytes()
                })
                .collect::<Vec<_>>();
            hash_in_place(&binding, oracle, 1, &indices, &left, &mut right, execution).unwrap();
            assert_eq!(&right[..], expected);
        }
    }

    #[test]
    fn parent_batches_match_canonical_hashes_under_different_worker_counts() {
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    check_parity(DigestExecutionV1::Cpu);
                });
        }
    }

    #[test]
    fn malformed_parent_batches_fail_before_returning_commitments() {
        let binding = Context::new(b"parent batch rejection").unwrap();
        let mut right = [[1; 32]; 2];
        for (level, indices, left) in [
            (1, &[][..], &[][..]),
            (1, &[0][..], &[[1; 32]; 2][..]),
            (0, &[0, 1][..], &[[1; 32]; 2][..]),
            (24, &[0, 1][..], &[[1; 32]; 2][..]),
            (1, &[0, usize::MAX][..], &[[1; 32]; 2][..]),
        ] {
            assert!(
                hash_in_place(
                    &binding,
                    Oracle::Row,
                    level,
                    indices,
                    left,
                    &mut right,
                    DigestExecutionV1::Cpu
                )
                .is_err()
            );
        }
        assert_eq!(digest([0xff; 32]).unwrap().into_bytes(), [0xff; 32]);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn unavailable_cuda_parent_batches_fail_before_private_hashing() {
        let binding = Context::new(b"device unavailable").unwrap();
        let mut right = [[0xff; 32]; 1];
        assert!(
            hash_in_place(
                &binding,
                Oracle::Row,
                1,
                &[0],
                &[[0xff; 32]],
                &mut right,
                DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Cuda)
            )
            .is_err()
        );
        assert_eq!(right, [[0xff; 32]]);
    }
}
