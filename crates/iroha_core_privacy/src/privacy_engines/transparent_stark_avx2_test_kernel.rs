//! Test-only four-lane Goldilocks butterflies; no production dispatch selects this code.

#![allow(unsafe_code)]

use super::{GoldilocksFieldV1 as F, *};
use core::arch::x86_64::*;

use super::goldilocks_avx2::{add, multiply, subtract};

/// The caller admits AVX2, canonical inputs, equal halves and sufficient public powers.
#[target_feature(enable = "avx2")]
unsafe fn window(
    left: &mut [F],
    right: &mut [F],
    powers: &[F],
    first: usize,
    stride: usize,
) -> usize {
    assert_eq!(left.len(), right.len());
    let mut offset = 0;
    while offset + 4 <= left.len() {
        let index = (first + offset) * stride;
        let twiddle = _mm256_set_epi64x(
            powers[index + 3 * stride].0.cast_signed(),
            powers[index + 2 * stride].0.cast_signed(),
            powers[index + stride].0.cast_signed(),
            powers[index].0.cast_signed(),
        );
        // SAFETY: each checked group contains four repr(transparent) u64 fields;
        // mutable halves are disjoint and loadu/storeu impose no SIMD alignment.
        unsafe {
            let even = _mm256_loadu_si256(left.as_ptr().add(offset).cast());
            let odd = _mm256_loadu_si256(right.as_ptr().add(offset).cast());
            let scaled = multiply(odd, twiddle);
            _mm256_storeu_si256(left.as_mut_ptr().add(offset).cast(), add(even, scaled));
            _mm256_storeu_si256(
                right.as_mut_ptr().add(offset).cast(),
                subtract(even, scaled),
            );
        }
        offset += 4;
    }
    let blocks = offset / 4;
    for index in offset..left.len() {
        let even = left[index];
        let scaled = right[index].mul(powers[(first + index) * stride]);
        left[index] = even.add(scaled);
        right[index] = even.sub(scaled);
    }
    blocks
}

/// Run a test-only coarse FFT, retaining validation, public windows and caller ownership.
pub(crate) fn transform(
    values: &mut [F],
    root: F,
    powers: &GoldilocksFftPowersV1,
    inverse: bool,
) -> Result<usize, TransparentStarkErrorV1> {
    assert!(
        std::is_x86_feature_detected!("avx2"),
        "AVX2 diagnostic admission required"
    );
    if root.0 >= GOLDILOCKS_MODULUS_V1 {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let transform_root = if inverse {
        root.inv().ok_or(TransparentStarkErrorV1::DivisionByZero)?
    } else {
        root
    };
    let size = values.len();
    if size == 0
        || !size.is_power_of_two()
        || transform_root.pow(size as u128) != F::ONE
        || (size > 1 && transform_root.pow((size / 2) as u128) == F::ONE)
        || powers.size != size
        || powers.root != transform_root
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if values.iter().any(|value| value.0 >= GOLDILOCKS_MODULUS_V1) {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let mut reversed = 0_usize;
    for index in 1..size {
        let mut bit = size >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            values.swap(index, reversed);
        }
    }
    let mut blocks = 0;
    let mut width = 2;
    while width <= size {
        let grain = if size < FFT_PARALLEL_MIN_VALUES_V1 || rayon::current_num_threads() == 1 {
            width / 2
        } else {
            FFT_BUTTERFLIES_PER_TASK_V1
        };
        for chunk in values.chunks_exact_mut(width) {
            let (left, right) = chunk.split_at_mut(width / 2);
            // Preserve the original coarse public window geometry. No Rayon
            // tasks, twiddle allocations, or private scratch are added here.
            for (index, (left, right)) in left
                .chunks_mut(grain)
                .zip(right.chunks_mut(grain))
                .enumerate()
            {
                // SAFETY: feature admission above precedes all intrinsics;
                // validated size/root and stage geometry bound every power.
                blocks +=
                    unsafe { window(left, right, &powers.values, index * grain, size / width) };
            }
        }
        width <<= 1;
    }
    if inverse {
        let inverse_size = F::reduce(size as u128)
            .inv()
            .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
        for value in values {
            *value = value.mul(inverse_size);
        }
    }
    Ok(blocks)
}

#[test]
#[ignore = "mandatory AVX2 edge/tail control for the explicit native diagnostic"]
fn avx2_butterfly_edges_and_tails_match_independent_modular_arithmetic() {
    assert!(std::is_x86_feature_detected!("avx2"));
    let edges = [
        0,
        1,
        2,
        (1 << 32) - 1,
        1 << 32,
        (1 << 32) + 1,
        (1 << 63) - 1,
        1 << 63,
        (1 << 63) + 1,
        GOLDILOCKS_MODULUS_V1 - 2,
        GOLDILOCKS_MODULUS_V1 - 1,
    ];
    let mut blocks = 0;
    for size in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 17] {
        for first in [0, 1, 3] {
            for stride in [1, 2, 7] {
                for a in edges {
                    for b in edges {
                        let powers = (0..((first + size + 1) * stride))
                            .map(|i| F(edges[i % edges.len()]))
                            .collect::<Vec<_>>();
                        // Offset one field deliberately exercises unaligned loads.
                        let mut left = vec![F(a); size + 1];
                        let mut right = vec![F(b); size + 1];
                        // SAFETY: admitted AVX2, complete public powers and disjoint slices.
                        blocks += unsafe {
                            window(&mut left[1..], &mut right[1..], &powers, first, stride)
                        };
                        for i in 0..size {
                            let p = u128::from(GOLDILOCKS_MODULUS_V1);
                            let product =
                                u128::from(b) * u128::from(powers[(first + i) * stride].0) % p;
                            assert_eq!(left[i + 1].0 as u128, (u128::from(a) + product) % p);
                            assert_eq!(right[i + 1].0 as u128, (u128::from(a) + p - product) % p);
                        }
                        assert_eq!(left[0], F(a));
                        assert_eq!(right[0], F(b));
                    }
                }
            }
        }
    }
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        F(seed % GOLDILOCKS_MODULUS_V1)
    };
    let mut left = (0..16_387).map(|_| next()).collect::<Vec<_>>();
    let mut right = (0..left.len()).map(|_| next()).collect::<Vec<_>>();
    let powers = (0..left.len()).map(|_| next()).collect::<Vec<_>>();
    let original = left
        .iter()
        .zip(&right)
        .map(|(a, b)| (*a, *b))
        .collect::<Vec<_>>();
    // SAFETY: mandatory feature admission above, disjoint equal slices and complete powers.
    blocks += unsafe { window(&mut left, &mut right, &powers, 0, 1) };
    for (i, (a, b)) in original.into_iter().enumerate() {
        let p = u128::from(GOLDILOCKS_MODULUS_V1);
        let product = u128::from(b.0) * u128::from(powers[i].0) % p;
        assert_eq!(left[i].0 as u128, (u128::from(a.0) + product) % p);
        assert_eq!(right[i].0 as u128, (u128::from(a.0) + p - product) % p);
    }
    assert!(blocks > 0, "scalar tails cannot qualify AVX2");
    println!("avx2_butterfly_edges vector_blocks={blocks} parity=exact");
}

#[test]
#[ignore = "mandatory AVX2 complete FFT/oracle control for the explicit native diagnostic"]
fn avx2_fft_preserves_scalar_oracle_outputs_and_validation() {
    use fastpq_prover::goldilocks_transform::{
        GoldilocksTransformBackendV1, GoldilocksTransformDirectionV1,
        transform_goldilocks_columns_v1,
    };
    assert!(std::is_x86_feature_detected!("avx2"));
    let mut total_blocks = 0;
    for log in 0..=10 {
        let rows = 1usize << log;
        let root = goldilocks_primitive_root_v1(log).unwrap();
        for inverse in [false, true] {
            let powers = GoldilocksFftPowersV1::new_v1(
                rows,
                if inverse { root.inv().unwrap() } else { root },
                GoldilocksFftPowersV1::required_payload_bytes_v1(rows).unwrap(),
            )
            .unwrap();
            let original = (0..rows)
                .map(|i| match i % 4 {
                    0 => F::ZERO,
                    1 => F(GOLDILOCKS_MODULUS_V1 - 1),
                    _ => F::reduce((i as u128 + 7).pow(5)),
                })
                .collect::<Vec<_>>();
            let mut expected = original.clone();
            let mut actual = original.clone();
            let scalar = if inverse {
                goldilocks_ifft_coarse_with_powers_v1
            } else {
                goldilocks_fft_coarse_with_powers_v1
            };
            scalar(&mut expected, root, &powers).unwrap();
            total_blocks += transform(&mut actual, root, &powers, inverse).unwrap();
            assert_eq!(actual, expected);
            let mut oracle = vec![original.iter().map(|v| v.0).collect::<Vec<_>>()];
            if rows == 1 {
                assert_eq!(actual, original);
            } else {
                assert_eq!(
                    transform_goldilocks_columns_v1(
                        &mut oracle,
                        root.0,
                        if inverse {
                            GoldilocksTransformDirectionV1::Inverse
                        } else {
                            GoldilocksTransformDirectionV1::Forward
                        },
                        fastpq_prover::ExecutionMode::Cpu
                    )
                    .unwrap(),
                    GoldilocksTransformBackendV1::Cpu
                );
            }
            assert!(actual.iter().zip(&oracle[0]).all(|(a, b)| a.0 == *b));
            for bad_root in [F::ZERO, F(GOLDILOCKS_MODULUS_V1)] {
                let mut expected = original.clone();
                let mut actual = original.clone();
                assert_eq!(
                    transform(&mut actual, bad_root, &powers, inverse).map(|_| ()),
                    scalar(&mut expected, bad_root, &powers)
                );
                assert_eq!(actual, original);
            }
            let mut actual = original;
            actual[0] = F(GOLDILOCKS_MODULUS_V1);
            let before = actual.clone();
            assert_eq!(
                transform(&mut actual, root, &powers, inverse),
                Err(TransparentStarkErrorV1::NonCanonicalField)
            );
            assert_eq!(actual, before);
        }
    }
    assert!(total_blocks > 0);
    println!("avx2_fft_oracle vector_blocks={total_blocks} parity=exact");
}

#[path = "transparent_stark_avx2_dif_test_kernel.rs"]
pub(crate) mod dif;
