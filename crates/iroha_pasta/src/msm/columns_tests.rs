//! Batch column commitments: exact sums, admitted lanes and the windowed fallback.

use super::*;
use crate::{Ep, Eq, field::Fq};
use ff::Field;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

/// Scalars and bases with zeros, units, repeats, opposite points and identities.
fn mixed_inputs<C: PastaCurve>(n: usize, seed: u64) -> (Vec<C::ScalarExt>, Vec<C::AffineExt>) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let generator = C::generator();
    let scalars = (0..n)
        .map(|i| match i % 6 {
            0 => C::ScalarExt::ZERO,
            1 => -C::ScalarExt::ONE,
            2 => C::ScalarExt::ONE,
            3 => C::ScalarExt::from(1_u64 << (i % 63)),
            _ => C::ScalarExt::random(&mut rng),
        })
        .collect();
    let bases = (0..n)
        .map(|i| match i % 7 {
            0 => C::AffineExt::identity(),
            1 | 4 => generator.to_affine(),
            2 => (-generator).to_affine(),
            _ => (generator * C::ScalarExt::random(&mut rng)).to_affine(),
        })
        .collect();
    (scalars, bases)
}

fn columns_of<C: PastaCurve>(count: usize, n: usize) -> Vec<Vec<C::ScalarExt>> {
    (0..count)
        .map(|column| mixed_inputs::<C>(n, 100 + column as u64).0)
        .collect()
}

fn exact_columns<C: PastaCurve>() {
    for n in [0, 1, 8, 9, 300, 4097] {
        let (_, bases) = mixed_inputs::<C>(n, 7);
        for count in [0, 1, 2, 3, 7, 9] {
            let columns = columns_of::<C>(count, n);
            let borrowed: Vec<&[C::ScalarExt]> = columns.iter().map(Vec::as_slice).collect();
            let expected: Vec<C> = columns
                .iter()
                .map(|column| msm_naive::<C>(column, &bases))
                .collect();
            for threads in [1, 3, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let shared = SharedMemoryBudget::new(PROCESS_MSM_SCRATCH_BYTES);
                let sums = pool
                    .install(|| {
                        msm_secret_columns_cancellable::<C>(
                            &borrowed,
                            &bases,
                            MemoryBudget::DEFAULT,
                            &shared,
                            None,
                        )
                    })
                    .unwrap();
                assert_eq!(sums, expected, "n={n} columns={count} threads={threads}");
                assert_eq!(shared.in_use_bytes(), 0);
                assert!(shared.peak_bytes() <= shared.limit_bytes());
            }
        }
    }
}

#[test]
fn columns_equal_independent_sums_on_both_curves() {
    exact_columns::<Ep>();
    exact_columns::<Eq>();
}

fn bounded_lanes<C: PastaCurve>() {
    let n = 4097;
    let (_, bases) = mixed_inputs::<C>(n, 11);
    let columns = columns_of::<C>(11, n);
    let borrowed: Vec<&[C::ScalarExt]> = columns.iter().map(Vec::as_slice).collect();
    let expected: Vec<C> = columns
        .iter()
        .map(|column| msm_naive::<C>(column, &bases))
        .collect();
    let member = pippenger::plan(n, 255, 1, MemoryBudget::DEFAULT.bytes()).unwrap();
    // Room for exactly two single-task kernels: four workers must not run more.
    let shared = SharedMemoryBudget::new(member.bytes * 2 + member.bytes / 2);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let sums = pool
        .install(|| {
            msm_secret_columns_cancellable::<C>(
                &borrowed,
                &bases,
                MemoryBudget::DEFAULT,
                &shared,
                None,
            )
        })
        .unwrap();
    assert_eq!(sums, expected);
    assert_eq!(shared.in_use_bytes(), 0);
    assert!(
        shared.peak_bytes() >= member.bytes * 2,
        "two lanes must be admitted together"
    );
    assert!(shared.peak_bytes() <= shared.limit_bytes());
}

#[test]
fn columns_never_run_more_kernels_than_admitted() {
    bounded_lanes::<Ep>();
    bounded_lanes::<Eq>();
}

fn no_lane_fits<C: PastaCurve>() {
    let n = 300;
    let (_, bases) = mixed_inputs::<C>(n, 13);
    let columns = columns_of::<C>(5, n);
    let borrowed: Vec<&[C::ScalarExt]> = columns.iter().map(Vec::as_slice).collect();
    let expected: Vec<C> = columns
        .iter()
        .map(|column| msm_naive::<C>(column, &bases))
        .collect();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    // Zero scratch: every column runs alone through the allocation-free path.
    let shared = SharedMemoryBudget::new(0);
    let sums = pool
        .install(|| {
            msm_secret_columns_cancellable::<C>(
                &borrowed,
                &bases,
                MemoryBudget::DEFAULT,
                &shared,
                None,
            )
        })
        .unwrap();
    assert_eq!(sums, expected);
    assert_eq!(shared.peak_bytes(), 0);
}

#[test]
fn columns_without_admitted_lanes_stay_exact_and_unreserved() {
    no_lane_fits::<Ep>();
    no_lane_fits::<Eq>();
}

#[test]
fn columns_check_every_length_before_work_and_cancel() {
    let n = 300;
    let (_, bases) = mixed_inputs::<Ep>(n, 17);
    let columns = columns_of::<Ep>(4, n);
    let mut borrowed: Vec<&[Fq]> = columns.iter().map(Vec::as_slice).collect();
    borrowed[3] = &columns[3][..n - 1];
    let shared = SharedMemoryBudget::new(PROCESS_MSM_SCRATCH_BYTES);
    assert!(matches!(
        msm_secret_columns_cancellable::<Ep>(
            &borrowed,
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            None
        ),
        Err(MsmError::LengthMismatch(_))
    ));
    assert_eq!(shared.peak_bytes(), 0);
    borrowed[3] = &columns[3];
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        msm_secret_columns_cancellable::<Ep>(
            &borrowed,
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            Some(&token)
        ),
        Err(MsmError::Cancelled)
    );
    assert_eq!(shared.in_use_bytes(), 0);
    // A kernel too large for the per-kernel budget refuses exactly as one column does.
    assert!(matches!(
        msm_secret_columns_cancellable::<Ep>(
            &borrowed,
            &bases,
            MemoryBudget::new(16),
            &shared,
            None
        ),
        Err(MsmError::Budget(_))
    ));
}

fn windowed<C: PastaCurve>() {
    for n in [STACK_WINDOWED_MIN + 1, 100, 257, 1031] {
        let (scalars, bases) = mixed_inputs::<C>(n, 19 + n as u64);
        let expected = msm_naive::<C>(&scalars, &bases);
        assert_eq!(
            windowed_msm::<C, true>(&scalars, &bases, None).unwrap(),
            expected
        );
        assert_eq!(
            windowed_msm::<C, false>(&scalars, &bases, None).unwrap(),
            expected
        );
        assert_eq!(
            stack_msm::<C, true>(&scalars, &bases, None).unwrap(),
            expected
        );
    }
    // One bucket receives P, P (a doubling) and -P (a cancellation); the top
    // window of -1 is nonzero; an all-identity input sums to the identity.
    let p = C::generator() * C::ScalarExt::from(5_u64);
    let bases = vec![
        p.to_affine(),
        p.to_affine(),
        (-p).to_affine(),
        p.to_affine(),
    ];
    let scalars = vec![C::ScalarExt::from(63_u64); 3]
        .into_iter()
        .chain([-C::ScalarExt::ONE])
        .collect::<Vec<_>>();
    assert_eq!(
        windowed_msm::<C, true>(&scalars, &bases, None).unwrap(),
        msm_naive::<C>(&scalars, &bases)
    );
    let identities = vec![C::AffineExt::identity(); 40];
    let ones = vec![C::ScalarExt::ONE; 40];
    assert!(bool::from(
        windowed_msm::<C, true>(&ones, &identities, None)
            .unwrap()
            .is_identity()
    ));
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        windowed_msm::<C, true>(&ones, &identities, Some(&token)),
        Err(MsmError::Cancelled)
    );
}

#[test]
fn windowed_fallback_is_exact_for_exceptional_points() {
    windowed::<Ep>();
    windowed::<Eq>();
}
