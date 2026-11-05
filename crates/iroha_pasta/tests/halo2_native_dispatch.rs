//! Deterministic arithmetic checks for the vendored native MSM and FFT dispatch.

use halo2curves::{
    bn256::{Fr, G1, G1Affine},
    ff::{Field, PrimeField},
    fft::best_fft,
    group::{Curve, Group},
    msm::msm_best,
};

fn inputs(length: usize, include_identity: bool) -> (Vec<Fr>, Vec<G1Affine>, G1) {
    let generator = G1::generator();
    let mut point = generator;
    let mut scalars = Vec::with_capacity(length);
    let mut points = Vec::with_capacity(length);
    let mut expected_scalar = Fr::ZERO;
    for index in 0..length {
        let magnitude = Fr::from(u64::try_from(index % 19 + 1).unwrap());
        let scalar = if index % 5 == 0 {
            -magnitude
        } else {
            magnitude
        };
        scalars.push(scalar);
        if include_identity && index % 13 == 0 {
            points.push(G1::identity().to_affine());
        } else {
            points.push(point.to_affine());
            expected_scalar += scalar * Fr::from(u64::try_from(index + 1).unwrap());
        }
        point += generator;
    }
    (scalars, points, generator * expected_scalar)
}

#[test]
fn msm_matches_algebraic_reference_across_window_threshold_and_thread_counts() {
    // 8192 enters the scheduled/windowed branch, including its indexed reverse iteration.
    for length in [0, 1, 4, 32, 8192] {
        let (scalars, points, expected) = inputs(length, false);
        for threads in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            assert_eq!(pool.install(|| msm_best(&scalars, &points)), expected);
        }
    }
}

#[test]
fn large_msm_accepts_identity_bases_without_unwrapping_missing_coordinates() {
    let (scalars, points, expected) = inputs(8192, true);
    for threads in [1, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        assert_eq!(pool.install(|| msm_best(&scalars, &points)), expected);
        let identities = vec![G1::identity().to_affine(); scalars.len()];
        assert_eq!(
            pool.install(|| msm_best(&scalars, &identities)),
            G1::identity()
        );
    }
}

fn omega(log_n: u32) -> Fr {
    Fr::ROOT_OF_UNITY.pow_vartime([1_u64 << (Fr::S - log_n)])
}

#[test]
fn fft_matches_direct_evaluation_and_inverse_across_thread_counts() {
    for log_n in [0, 1, 4, 8, 12] {
        let length = 1_usize << log_n;
        let original: Vec<_> = (0..length)
            .map(|index| Fr::from(u64::try_from(index * 17 + 3).unwrap()))
            .collect();
        let root = omega(log_n);
        let mut reference = None;
        for threads in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut values = original.clone();
            pool.install(|| best_fft(&mut values, root, log_n));
            if let Some(reference) = &reference {
                assert_eq!(&values, reference);
            } else {
                reference = Some(values.clone());
            }
            if length <= 16 {
                for (index, actual) in values.iter().enumerate() {
                    let at = root.pow_vartime([u64::try_from(index).unwrap()]);
                    let expected = original
                        .iter()
                        .rev()
                        .fold(Fr::ZERO, |sum, coefficient| sum * at + coefficient);
                    assert_eq!(*actual, expected);
                }
            }
            pool.install(|| best_fft(&mut values, root.invert().unwrap(), log_n));
            let inverse_length = Fr::from(u64::try_from(length).unwrap()).invert().unwrap();
            for value in &mut values {
                *value *= inverse_length;
            }
            assert_eq!(values, original);
        }
    }
}

#[test]
#[ignore = "opt-in timing measurement; run with --ignored --nocapture"]
fn benchmark_native_dispatch() {
    use std::{hint::black_box, time::Instant};

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    for length in [64, 8192] {
        let (scalars, points, expected) = inputs(length, false);
        assert_eq!(pool.install(|| msm_best(&scalars, &points)), expected);
        let mut samples = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..3 {
                black_box(pool.install(|| msm_best(black_box(&scalars), black_box(&points))));
            }
            samples.push(start.elapsed().as_micros() / 3);
        }
        samples.sort_unstable();
        println!(
            "MSM n={length} median_us={} samples_us={samples:?}",
            samples[3]
        );
    }
    for log_n in [12, 16] {
        let length = 1_usize << log_n;
        let original: Vec<_> = (0..length)
            .map(|index| Fr::from(u64::try_from(index + 1).unwrap()))
            .collect();
        let root = omega(log_n);
        let mut samples = Vec::new();
        for _ in 0..7 {
            let mut values = original.clone();
            let start = Instant::now();
            pool.install(|| best_fft(black_box(&mut values), black_box(root), log_n));
            samples.push(start.elapsed().as_micros());
            black_box(values);
        }
        samples.sort_unstable();
        println!(
            "FFT n={length} median_us={} samples_us={samples:?}",
            samples[3]
        );
    }
}
