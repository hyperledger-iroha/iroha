//! The variable-time secret MSMs against the naive sum, on random and edge
//! inputs: identity, repeated and negated bases, zero, one and minus-one
//! scalars, every public bound, equal and opposite operands, both curves,
//! one and several workers.

use super::*;
use crate::{Ep, Eq};
use ff::Field;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

/// `2^bits - 1` for `bits < 255`; the largest canonical scalar otherwise.
fn largest<F: PastaField>(bits: usize) -> F {
    if bits >= 255 {
        return -F::ONE;
    }
    (0..bits).fold(F::ONE, |value, _| value.double()) - F::ONE
}

/// A random scalar below `2^bits`.
fn below<F: PastaField>(rng: &mut ChaCha20Rng, bits: usize) -> F {
    let mut limbs = F::random(&mut *rng).to_canonical_limbs();
    for (index, limb) in limbs.iter_mut().enumerate() {
        let kept = bits.saturating_sub(index * 64).min(64);
        if kept < 64 {
            *limb &= (1_u64 << kept) - 1;
        }
    }
    F::from_canonical_limbs(limbs).unwrap()
}

/// Random bases with identities, a repeated generator and its negation.
fn bases<C: PastaCurve>(rng: &mut ChaCha20Rng, n: usize) -> Vec<C::AffineExt> {
    (0..n)
        .map(|i| match i % 7 {
            0 => C::AffineExt::identity(),
            1 => C::generator().to_affine(),
            2 => (-C::generator()).to_affine(),
            _ => C::random(&mut *rng).to_affine(),
        })
        .collect()
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
}

/// Full-width secret MSMs of many lengths equal the naive sum.
fn full_width<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    for n in [0_usize, 1, 2, 5, 9, 33, 131, 3 * 128 + 1, 2049] {
        let bases = bases::<C>(&mut rng, n);
        let scalars: Vec<C::ScalarExt> = (0..n)
            .map(|i| match i % 5 {
                0 => C::ScalarExt::ZERO,
                1 => C::ScalarExt::ONE,
                2 => -C::ScalarExt::ONE,
                _ => C::ScalarExt::random(&mut rng),
            })
            .collect();
        let expected = msm_naive::<C>(&scalars, &bases);
        for threads in [1, 4] {
            let shared = SharedMemoryBudget::new(1 << 22);
            let actual = pool(threads)
                .install(|| {
                    msm_secret_cancellable::<C>(
                        &scalars,
                        &bases,
                        MemoryBudget::DEFAULT,
                        &shared,
                        None,
                    )
                })
                .unwrap();
            assert_eq!(actual, expected, "n = {n}, threads = {threads}");
            assert_eq!(actual.to_bytes(), expected.to_bytes());
            assert_eq!(shared.in_use_bytes(), 0);
        }
    }
}

#[test]
fn secret_msm_full_width_equals_naive_on_both_curves() {
    full_width::<Ep>(101);
    full_width::<Eq>(103);
}

/// Bounded columns (and each bounded column alone) at many public bounds
/// equal their naive sums.
fn bounded_columns<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let bounds = [
        0_usize, 1, 2, 3, 4, 5, 7, 8, 11, 16, 63, 64, 65, 80, 128, 181, 254, 255,
    ];
    for n in [1_usize, 7, 40, 2 * 128 + 5, 1500] {
        let bases = bases::<C>(&mut rng, n);
        let rows: Vec<Vec<C::ScalarExt>> = bounds
            .iter()
            .map(|&bits| {
                (0..n)
                    .map(|i| match i % 4 {
                        0 => C::ScalarExt::ZERO,
                        1 => largest::<C::ScalarExt>(bits),
                        _ => below::<C::ScalarExt>(&mut rng, bits),
                    })
                    .collect()
            })
            .collect();
        let checked: Vec<BoundedSecretScalars<'_, C::ScalarExt>> = rows
            .iter()
            .zip(bounds)
            .map(|(row, bits)| BoundedSecretScalars::new(row, bits, None).unwrap())
            .collect();
        let expected: Vec<C> = rows.iter().map(|row| msm_naive::<C>(row, &bases)).collect();
        for threads in [1, 3] {
            let shared = SharedMemoryBudget::new(1 << 22);
            let workers = pool(threads);
            let columns = workers
                .install(|| {
                    msm_secret_bounded_columns_cancellable::<C>(
                        &checked,
                        &bases,
                        MemoryBudget::DEFAULT,
                        &shared,
                        None,
                    )
                })
                .unwrap();
            assert_eq!(columns, expected, "n = {n}, threads = {threads}");
            for (column, sum) in checked.iter().zip(&expected) {
                let single = workers.install(|| {
                    msm_secret_bounded_cancellable::<C>(
                        column,
                        &bases,
                        MemoryBudget::DEFAULT,
                        &shared,
                        None,
                    )
                });
                assert_eq!(single.unwrap(), *sum);
            }
            assert_eq!(shared.in_use_bytes(), 0);
        }
    }
}

#[test]
fn bounded_secret_columns_equal_naive_on_both_curves() {
    bounded_columns::<Ep>(105);
    bounded_columns::<Eq>(107);
}

/// Every public bound: the largest scalar below it, a random one, the
/// largest one bit narrower and zero reconstruct exactly.
fn every_bound<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let g = C::generator();
    let bases = [
        g.to_affine(),
        (-g).to_affine(),
        g.double().to_affine(),
        C::random(&mut rng).to_affine(),
    ];
    for bits in 0..=255 {
        let row = [
            largest::<C::ScalarExt>(bits),
            below::<C::ScalarExt>(&mut rng, bits),
            largest::<C::ScalarExt>(bits.saturating_sub(1)),
            C::ScalarExt::ZERO,
        ];
        let checked = BoundedSecretScalars::new(&row, bits, None).unwrap();
        let shared = SharedMemoryBudget::new(1 << 20);
        let sum = msm_secret_bounded_cancellable::<C>(
            &checked,
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            None,
        )
        .unwrap();
        assert_eq!(sum, msm_naive::<C>(&row, &bases), "public bound {bits}");
    }
}

#[test]
fn every_public_bound_equals_naive_on_both_curves() {
    every_bound::<Ep>(109);
    every_bound::<Eq>(111);
}

/// Equal and opposite operands: one repeated base with equal scalars, a base
/// and its negation with equal scalars, a scalar and its negation on one
/// base, and identities among them, at several bounds.
fn collisions<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let g = C::generator().to_affine();
    let minus = (-C::generator()).to_affine();
    let p = C::random(&mut rng).to_affine();
    let s = C::ScalarExt::random(&mut rng);
    let t = C::ScalarExt::random(&mut rng);
    let cases: Vec<(Vec<C::AffineExt>, Vec<C::ScalarExt>)> = vec![
        (vec![g; 64], vec![s; 64]),
        (vec![p; 37], vec![-C::ScalarExt::ONE; 37]),
        (
            (0..64)
                .map(|i| if i % 2 == 0 { g } else { minus })
                .collect(),
            vec![s; 64],
        ),
        (
            vec![p; 40],
            (0..40).map(|i| if i % 2 == 0 { t } else { -t }).collect(),
        ),
        (
            (0..50)
                .map(|i| match i % 4 {
                    0 => C::AffineExt::identity(),
                    1 | 2 => g,
                    _ => minus,
                })
                .collect(),
            (0..50)
                .map(|i| if i % 5 == 0 { C::ScalarExt::ZERO } else { s })
                .collect(),
        ),
        (vec![C::AffineExt::identity(); 9], vec![s; 9]),
        (vec![g; 9], vec![C::ScalarExt::ZERO; 9]),
    ];
    for (bases, scalars) in &cases {
        for bits in [255_usize, 254, 64, 9] {
            let cut: Vec<C::ScalarExt> = scalars
                .iter()
                .map(|x| {
                    let mut limbs = x.to_canonical_limbs();
                    for (index, limb) in limbs.iter_mut().enumerate() {
                        let kept = bits.saturating_sub(index * 64).min(64);
                        if kept < 64 {
                            *limb &= (1_u64 << kept) - 1;
                        }
                    }
                    C::ScalarExt::from_canonical_limbs(limbs).unwrap()
                })
                .collect();
            let want = msm_naive::<C>(&cut, bases);
            for threads in [1, 4] {
                let shared = SharedMemoryBudget::new(1 << 22);
                let (full, columns) = pool(threads).install(|| {
                    (
                        msm_secret_cancellable::<C>(
                            &cut,
                            bases,
                            MemoryBudget::DEFAULT,
                            &shared,
                            None,
                        )
                        .unwrap(),
                        msm_secret_bounded_columns_cancellable::<C>(
                            &[
                                BoundedSecretScalars::new(&cut, bits, None).unwrap(),
                                BoundedSecretScalars::new(&cut, 255, None).unwrap(),
                            ],
                            bases,
                            MemoryBudget::DEFAULT,
                            &shared,
                            None,
                        )
                        .unwrap(),
                    )
                });
                assert_eq!(full, want, "bits {bits} threads {threads}");
                assert_eq!(columns, vec![want, want], "bits {bits} threads {threads}");
            }
        }
    }
}

#[test]
fn secret_msms_handle_equal_opposite_and_identity_operands_on_both_curves() {
    collisions::<Ep>(113);
    collisions::<Eq>(115);
}
