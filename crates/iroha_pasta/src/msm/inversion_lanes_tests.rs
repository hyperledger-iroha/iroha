//! Fixed-width inversion scheduling experiment over public seeded inputs.
//! This module does not change the production bucket kernel.

use super::*;
use crate::{Fp, Fq};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};
use std::{hint::black_box, time::Instant};

fn serial<F: PastaField>(den: &mut [F], prefix: &mut [F]) {
    let mut acc = F::ONE;
    for (d, p) in den.iter().zip(prefix.iter_mut()) {
        *p = acc;
        acc *= d;
    }
    let mut inv = acc.invert().unwrap_or(F::ZERO);
    for (d, p) in den.iter_mut().zip(prefix.iter()).rev() {
        let value = inv * p;
        inv *= *d;
        *d = value;
    }
}

fn interleaved<F: PastaField>(den: &mut [F], prefix: &mut [F]) {
    let mut acc = [F::ONE; 4];
    for (ds, ps) in den.chunks(4).zip(prefix.chunks_mut(4)) {
        for ((p, a), d) in ps.iter_mut().zip(acc.iter_mut()).zip(ds) {
            *p = *a;
            *a *= d;
        }
    }
    let lo = acc[0] * acc[1];
    let hi = acc[2] * acc[3];
    let inverse = (lo * hi).invert().unwrap_or(F::ZERO);
    let lo_inverse = inverse * hi;
    let hi_inverse = inverse * lo;
    let mut inv = [
        lo_inverse * acc[1],
        lo_inverse * acc[0],
        hi_inverse * acc[3],
        hi_inverse * acc[2],
    ];
    for (ds, ps) in den.chunks_mut(4).zip(prefix.chunks(4)).rev() {
        for j in 0..ds.len() {
            let value = inv[j] * ps[j];
            inv[j] *= ds[j];
            ds[j] = value;
        }
    }
}

fn inputs<F: PastaField>(n: usize) -> Vec<F> {
    let mut rng = ChaCha20Rng::seed_from_u64(0x696e_766c_616e_6573);
    (0..n)
        .map(|_| {
            let value = F::random(&mut rng);
            if bool::from(value.is_zero()) {
                F::ONE
            } else {
                value
            }
        })
        .collect()
}

fn parity<F: PastaField>() {
    for n in [0, 1, 2, 3, 4, 5, 31, 32, 33, 1023, 1024] {
        let original = inputs::<F>(n);
        let mut expected = original.clone();
        let mut actual = original.clone();
        let mut prefix = vec![F::ZERO; n];
        serial(&mut expected, &mut prefix);
        interleaved(&mut actual, &mut prefix);
        assert_eq!(actual, expected, "n={n}");
        let mut production_secret = original.clone();
        let mut production_public = original.clone();
        invert_queued::<F, true>(&mut production_secret, &mut prefix);
        invert_queued::<F, false>(&mut production_public, &mut prefix);
        assert_eq!(production_secret, expected, "secret n={n}");
        assert_eq!(production_public, expected, "public n={n}");
        for (value, inverse) in original.iter().zip(actual.iter()) {
            assert_eq!(*value * inverse, F::ONE);
        }
    }
}

#[test]
fn four_inversion_lanes_match_serial_on_both_fields() {
    parity::<Fp>();
    parity::<Fq>();
}

fn queued_bucket_boundaries<C: PastaCurve, const SECRET: bool>() {
    let generator = C::generator();
    let weights = [1_i64, 2, -1, 3, 4];
    let scalars: Vec<_> = weights
        .iter()
        .map(|weight| {
            let value = C::ScalarExt::from(weight.unsigned_abs());
            if *weight < 0 { -value } else { value }
        })
        .collect();
    let bases: Vec<_> = scalars
        .iter()
        .map(|scalar| (generator * scalar).to_affine())
        .collect();
    for count in [
        0, 1, 2, 3, 4, 5, 15, 16, 17, 63, 64, 65, 255, 256, 257, 511, 512, 513, 1023, 1024,
    ] {
        let mut buckets = Buckets::<C, SECRET>::new(&bases, (4 * count).max(64));
        let mut expected = C::ScalarExt::ZERO;
        for bucket in 0..count {
            let initial = bucket % bases.len();
            buckets.insert(bucket, initial, false);
            expected += scalars[initial] * C::ScalarExt::from((bucket + 1) as u64);
        }
        for bucket in 0..count {
            // Mix doubling, cancellation and ordinary affine addition within
            // every lane and within the partial final four-element group.
            let initial = bucket % bases.len();
            let (point, negative) = match bucket % 3 {
                0 => (initial, false),
                1 => (initial, true),
                _ => ((initial + 1) % bases.len(), false),
            };
            buckets.insert(bucket, point, negative);
            let value = if negative {
                -scalars[point]
            } else {
                scalars[point]
            };
            expected += value * C::ScalarExt::from((bucket + 1) as u64);
            if bucket % 7 == 0 {
                buckets.insert(bucket, 3, false);
                expected += scalars[3] * C::ScalarExt::from((bucket + 1) as u64);
            }
        }
        buckets.flush();
        assert_eq!(
            buckets.reduce(0, count),
            generator * expected,
            "count={count}"
        );
        // A flush with an empty queue must preserve the same exact sum.
        buckets.flush();
        assert_eq!(buckets.reduce(0, count), generator * expected);
    }
}

#[test]
fn production_flush_handles_partial_lanes_and_exceptional_points() {
    queued_bucket_boundaries::<crate::Ep, false>();
    queued_bucket_boundaries::<crate::Ep, true>();
    queued_bucket_boundaries::<crate::Eq, false>();
    queued_bucket_boundaries::<crate::Eq, true>();
}

fn measure<F: PastaField>(field: &str, n: usize) {
    const REPEATS: usize = 500;
    let original = inputs::<F>(n);
    let mut values = original.clone();
    let mut prefix = vec![F::ZERO; n];
    let mut samples = [Vec::new(), Vec::new()];
    for round in 0..10 {
        for arm in [round % 2, 1 - round % 2] {
            values.copy_from_slice(&original);
            let start = Instant::now();
            for _ in 0..REPEATS {
                match arm {
                    0 => serial(black_box(&mut values), black_box(&mut prefix)),
                    _ => interleaved(black_box(&mut values), black_box(&mut prefix)),
                }
            }
            samples[arm].push(start.elapsed().as_nanos());
            assert_eq!(values, original);
        }
    }
    println!(
        "MSM_INVERSION_LANES field={field} n={n} repeats={REPEATS} serial_ns={:?} interleaved_ns={:?}",
        samples[0], samples[1]
    );
}

#[test]
#[ignore = "explicit fixed-width arithmetic scheduling measurement"]
fn benchmark_four_inversion_lanes() {
    for n in [16, 64, 256, 512, 1024] {
        measure::<Fp>("fp", n);
        measure::<Fq>("fq", n);
    }
}
