//! Exact-output, malformed-input and scheduling regressions for zero-padded FFTs.

use super::*;

fn dense_coset(
    coefficients: &[GoldilocksFieldV1],
    size: usize,
    root: GoldilocksFieldV1,
    shift: GoldilocksFieldV1,
) -> Vec<GoldilocksFieldV1> {
    let mut result = vec![GoldilocksFieldV1::ZERO; size];
    let mut power = GoldilocksFieldV1::ONE;
    for (target, value) in result.iter_mut().zip(coefficients) {
        *target = value.mul(power);
        power = power.mul(shift);
    }
    goldilocks_fft_v1(&mut result, root).unwrap();
    result
}

fn coefficients(length: usize, salt: u64) -> Vec<GoldilocksFieldV1> {
    (0..length)
        .map(|index| match index % 4 {
            0 => GoldilocksFieldV1::ZERO,
            1 => GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1 - 1),
            2 => GoldilocksFieldV1::ONE,
            _ => GoldilocksFieldV1::reduce(u128::from(salt) + index as u128 * 0x1_0000_0001),
        })
        .collect()
}

#[test]
fn every_small_prefix_matches_independent_horner_and_dense_fft() {
    for log in 0..=6 {
        let size = 1 << log;
        let root = goldilocks_primitive_root_v1(log).unwrap();
        for length in 0..=size {
            let input = coefficients(length, 97);
            for shift in [
                GoldilocksFieldV1::ONE,
                GoldilocksFieldV1(7),
                GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1 - 1),
            ] {
                let expected = dense_coset(&input, size, root, shift);
                let actual = goldilocks_evaluate_coset_v1(&input, size, root, shift).unwrap();
                assert_eq!(actual, expected, "log={log}, prefix={length}");
                // Exercise the primitive directly below the production threshold,
                // including full prefixes that production deliberately leaves dense.
                let mut direct = vec![GoldilocksFieldV1::ZERO; size];
                let mut power = GoldilocksFieldV1::ONE;
                for (target, value) in direct.iter_mut().zip(&input) {
                    *target = value.mul(power);
                    power = power.mul(shift);
                }
                goldilocks_fft_zero_padded_v1::<true>(&mut direct, root, length);
                assert_eq!(direct, expected);
                let mut point = shift;
                for value in &actual {
                    let evaluated = input
                        .iter()
                        .rev()
                        .fold(GoldilocksFieldV1::ZERO, |sum, c| sum.mul(point).add(*c));
                    assert_eq!(*value, evaluated);
                    point = point.mul(root);
                }
            }
        }
    }
}

#[test]
fn padded_fft_preserves_public_extent_mask_tail_and_worker_parity() {
    let log = 14;
    let size = 1 << log;
    let root = goldilocks_primitive_root_v1(log).unwrap();
    let shift = GoldilocksFieldV1(7);
    // Include empty/full prefixes, both sides of every smaller power of two,
    // and a nonzero masking tail above a native domain boundary.
    let mut lengths = BTreeSet::from([0, 1, size, (1 << 12) + 1816]);
    for exponent in 1..log {
        let middle = 1 << exponent;
        lengths.extend([middle - 1, middle, middle + 1]);
    }
    for workers in [1, 2, 4, 8, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            for &length in &lengths {
                let mut input = coefficients(length, 193);
                if let Some(last) = input.last_mut() {
                    *last = GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1 - 1);
                }
                let expected = dense_coset(&input, size, root, shift);
                let mut direct = vec![GoldilocksFieldV1::ZERO; size];
                let mut power = GoldilocksFieldV1::ONE;
                for (target, value) in direct.iter_mut().zip(&input) {
                    *target = value.mul(power);
                    power = power.mul(shift);
                }
                goldilocks_fft_zero_padded_v1::<true>(&mut direct, root, length);
                assert_eq!(direct, expected);
                for actual in [
                    goldilocks_evaluate_coset_with_inner_parallelism_v1::<true>(
                        &input, size, root, shift,
                    )
                    .unwrap(),
                    goldilocks_evaluate_coset_with_inner_parallelism_v1::<false>(
                        &input, size, root, shift,
                    )
                    .unwrap(),
                ] {
                    assert_eq!(actual, expected, "workers={workers}, prefix={length}");
                }
            }
            for length in [
                0,
                1,
                (1 << 16) - 1,
                1 << 16,
                (1 << 16) + 1,
                (1 << 18) - 1,
                1 << 18,
            ] {
                let input = coefficients(length, 71);
                let root = goldilocks_primitive_root_v1(18).unwrap();
                let expected = dense_coset(&input, 1 << 18, root, shift);
                for actual in [
                    goldilocks_evaluate_coset_with_inner_parallelism_v1::<true>(
                        &input,
                        1 << 18,
                        root,
                        shift,
                    )
                    .unwrap(),
                    goldilocks_evaluate_coset_with_inner_parallelism_v1::<false>(
                        &input,
                        1 << 18,
                        root,
                        shift,
                    )
                    .unwrap(),
                ] {
                    assert_eq!(actual, expected);
                }
            }
            for width in [1, 3, 8] {
                let inputs = (0..width)
                    .map(|index| coefficients(8193, index))
                    .collect::<Vec<_>>();
                let outputs = inputs
                    .par_iter()
                    .map(|input| goldilocks_evaluate_coset_v1(input, size, root, shift).unwrap())
                    .collect::<Vec<_>>();
                for (input, actual) in inputs.iter().zip(outputs) {
                    assert_eq!(actual, dense_coset(input, size, root, shift));
                }
            }
        });
    }
}

#[test]
fn padded_coset_retains_domain_and_coefficient_error_precedence() {
    let root = goldilocks_primitive_root_v1(4).unwrap();
    let invalid = GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1);
    for input in [
        vec![],
        vec![GoldilocksFieldV1::ONE],
        vec![GoldilocksFieldV1::ONE; 16],
    ] {
        for bad_root in [
            GoldilocksFieldV1::ZERO,
            GoldilocksFieldV1::ONE,
            invalid,
            root.mul(root),
        ] {
            assert_eq!(
                goldilocks_evaluate_coset_v1(&input, 16, bad_root, GoldilocksFieldV1(7)),
                Err(TransparentStarkErrorV1::InvalidDomain)
            );
        }
    }
    for size in [16, 1 << 18] {
        assert_eq!(
            goldilocks_evaluate_coset_v1(
                &[GoldilocksFieldV1::ONE],
                size,
                invalid,
                GoldilocksFieldV1(7)
            ),
            Err(TransparentStarkErrorV1::InvalidDomain),
        );
        assert_eq!(
            goldilocks_evaluate_coset_v1(&[invalid], size, invalid, GoldilocksFieldV1(7)),
            Err(TransparentStarkErrorV1::NonCanonicalField),
        );
    }
    assert_eq!(
        goldilocks_evaluate_coset_v1(&[invalid], 16, invalid, GoldilocksFieldV1(7)),
        Err(TransparentStarkErrorV1::NonCanonicalField)
    );
    for (size, shift, input) in [
        (0, GoldilocksFieldV1(7), vec![]),
        (3, GoldilocksFieldV1(7), vec![]),
        (16, GoldilocksFieldV1::ZERO, vec![]),
        (16, invalid, vec![invalid]),
        (16, GoldilocksFieldV1(7), vec![GoldilocksFieldV1::ONE; 17]),
    ] {
        assert_eq!(
            goldilocks_evaluate_coset_v1(&input, size, root, shift),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
    }
}

#[test]
#[ignore = "release-only common22 dense versus prefix FFT timing and full-output parity"]
fn common22_prefix_fft_reports_full_output_parity_and_timings() {
    use std::time::Instant;
    let size = 1 << 22;
    let root = goldilocks_primitive_root_v1(22).unwrap();
    let shift = GoldilocksFieldV1(7);
    for workers in [1, 8, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            for native_log in [5, 8, 15, 16, 18, 19] {
                for width in [1, 8] {
                    let inputs = (0..width).map(|index| coefficients((1 << native_log) + 1816, index)).collect::<Vec<_>>();
                    let before = Instant::now();
                    let expected = inputs.par_iter().map(|input| dense_coset(input, size, root, shift)).collect::<Vec<_>>();
                    let dense_seconds = before.elapsed().as_secs_f64();
                    let before = Instant::now();
                    let actual = inputs.par_iter().map(|input| goldilocks_evaluate_coset_v1(input, size, root, shift).unwrap()).collect::<Vec<_>>();
                    let prefix_seconds = before.elapsed().as_secs_f64();
                    assert_eq!(actual, expected);
                    eprintln!("X509_PREFIX_FFT_V1 native_log={native_log} common_log=22 width={width} workers={workers} dense_seconds={dense_seconds:.6} prefix_seconds={prefix_seconds:.6} speedup={:.3} parity=exact", dense_seconds / prefix_seconds);
                }
            }
        });
    }
}
