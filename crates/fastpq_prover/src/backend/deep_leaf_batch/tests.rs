//! SHA3 bounded batches, exact shape errors and deterministic CPU parity.
use super::*;
use crate::backend::secret_polynomial::SecretPolynomial;
#[test]
fn bounded_leaf_batches_match_canonical_hashes_for_every_oracle_and_worker_count() {
    let binding = Context::new(b"SHA3 bounded leaf batch").unwrap();
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for oracle in [
                    Oracle::Row,
                    Oracle::QuotientAndMask,
                    Oracle::Fri(0),
                    Oracle::Fri(1),
                    Oracle::Fri(2),
                    Oracle::Fri(3),
                    Oracle::Fri(4),
                    Oracle::Terminal,
                ] {
                    let (_, _, leaves, width) = oracle.shape().unwrap();
                    for count in [1, leaves.min(CAPACITY)] {
                        let indices = (0..count).rev().collect::<Vec<_>>();
                        let mut payload = SecretPolynomial::<u8>::zeroed(count * width).unwrap();
                        for (i, word) in payload.chunks_exact_mut(8).enumerate() {
                            word.copy_from_slice(&(i as u64).to_le_bytes());
                        }
                        let mut outputs = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
                        hash(
                            &binding,
                            oracle,
                            &indices,
                            &payload,
                            width,
                            &mut outputs,
                            DigestExecutionV1::Cpu,
                        )
                        .unwrap();
                        for ((&i, bytes), actual) in indices
                            .iter()
                            .zip(payload.chunks_exact(width))
                            .zip(outputs.iter())
                        {
                            assert_eq!(
                                *actual,
                                binding
                                    .hash_leaf(oracle, i as u32, bytes)
                                    .unwrap()
                                    .into_bytes()
                            );
                        }
                        assert!(
                            payload_bytes(&binding, oracle, width).unwrap()
                                > CAPACITY * (width + 32)
                        );
                    }
                }
            });
    }
}
#[test]
fn prepared_leaf_parent_and_shape_paths_have_the_same_canonical_hash() {
    let binding = Context::new(b"prepared SHA3 batch").unwrap();
    let left = Digest::from_bytes([0xff; 32]);
    let right = Digest::from_bytes([0x80; 32]);
    let frames = vec![
        binding
            .prepare_leaf(Oracle::QuotientAndMask, 3, &[0; 96])
            .unwrap(),
        binding
            .prepare_parent(Oracle::Row, 1, 7, left, right)
            .unwrap(),
    ];
    let mut output = SecretPolynomial::<[u8; 32]>::zeroed(2).unwrap();
    execute_prepared(&frames, &mut output, DigestExecutionV1::Cpu).unwrap();
    assert_eq!(
        output[0],
        binding
            .hash_leaf(Oracle::QuotientAndMask, 3, &[0; 96])
            .unwrap()
            .into_bytes()
    );
    assert_eq!(
        output[1],
        binding
            .hash_parent(Oracle::Row, 1, 7, left, right)
            .unwrap()
            .into_bytes()
    );
    assert!(execute_prepared(&[], &mut [], DigestExecutionV1::Cpu).is_err());
    assert!(execute_prepared(&frames, &mut output[..1], DigestExecutionV1::Cpu).is_err());
    for (indices, payload, width) in [
        (vec![], vec![], 96),
        (vec![0], vec![0; 95], 96),
        (vec![usize::MAX], vec![0; 96], 96),
        (vec![0], vec![], 0),
    ] {
        assert!(
            hash(
                &binding,
                Oracle::QuotientAndMask,
                &indices,
                &payload,
                width,
                &mut output[..1],
                DigestExecutionV1::Cpu
            )
            .is_err()
        );
    }
    let mut invalid = [0; 96];
    invalid[..8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(
        hash(
            &binding,
            Oracle::QuotientAndMask,
            &[0],
            &invalid,
            96,
            &mut output[..1],
            DigestExecutionV1::Cpu
        )
        .is_err()
    );
}
#[cfg(feature = "fastpq-gpu")]
#[test]
fn unavailable_cuda_cannot_silently_use_poseidon_or_scalar_sha3() {
    assert!(
        preflight_execution(DigestExecutionV1::Device(
            crate::Digest384GpuBackendV1::Cuda
        ))
        .is_err()
    );
}
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "requires actual canonical SHA3 Metal execution for every proof oracle"]
fn actual_metal_leaf_batches_match_canonical_sha3_for_every_oracle() {
    let _lane = crate::backend::acquire_gpu_lane();
    let binding = Context::new(b"actual canonical Metal SHA3").unwrap();
    for oracle in [
        Oracle::Row,
        Oracle::QuotientAndMask,
        Oracle::Fri(0),
        Oracle::Fri(1),
        Oracle::Fri(2),
        Oracle::Fri(3),
        Oracle::Fri(4),
        Oracle::Terminal,
    ] {
        let (_, _, leaves, width) = oracle.shape().unwrap();
        for count in [1, 63, 64, 257, CAPACITY].map(|n| n.min(leaves)) {
            let indices = (0..count).rev().collect::<Vec<_>>();
            let mut bytes = SecretPolynomial::<u8>::zeroed(count * width).unwrap();
            for (i, chunk) in bytes.chunks_exact_mut(8).enumerate() {
                chunk.copy_from_slice(&(i as u64).to_le_bytes());
            }
            let mut actual = SecretPolynomial::<[u8; 32]>::zeroed(count).unwrap();
            hash(
                &binding,
                oracle,
                &indices,
                &bytes,
                width,
                &mut actual,
                DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Metal),
            )
            .unwrap();
            for ((&index, body), actual) in indices
                .iter()
                .zip(bytes.chunks_exact(width))
                .zip(actual.iter())
            {
                assert_eq!(
                    *actual,
                    binding
                        .hash_leaf(oracle, index as u32, body)
                        .unwrap()
                        .into_bytes()
                );
            }
        }
    }
}
