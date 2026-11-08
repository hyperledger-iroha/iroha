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

/// Bounds across a column set: zero, one bit, narrow chunks, limbs and full width.
const BOUNDS: [usize; 6] = [0, 1, 13, 16, 88, 255];

/// A canonical integer below `2^bits` (`bits < 255`), built limb by limb from
/// the top: the bound's largest value, or a uniform one.
fn below_bound<F: PastaField>(rng: &mut ChaCha20Rng, bits: usize, largest: bool) -> F {
    use rand_core::Rng;
    let shift = F::from(u64::MAX) + F::ONE;
    let mut value = F::ZERO;
    for limb in (0..bits.div_ceil(64)).rev() {
        let kept = (bits - 64 * limb).min(64);
        let mask = if kept == 64 {
            u64::MAX
        } else {
            (1_u64 << kept) - 1
        };
        let word = if largest { mask } else { rng.next_u64() & mask };
        value = value * shift + F::from(word);
    }
    value
}

fn bounded_column<C: PastaCurve>(n: usize, bits: usize, seed: u64) -> Vec<C::ScalarExt> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let column: Vec<C::ScalarExt> = (0..n)
        .map(|i| match (bits, i % 5) {
            (0, _) | (_, 0) => C::ScalarExt::ZERO,
            (255, 1) => -C::ScalarExt::ONE,
            (255, _) => C::ScalarExt::random(&mut rng),
            (_, 1) => below_bound(&mut rng, bits, true),
            _ => below_bound(&mut rng, bits, false),
        })
        .collect();
    // Every value satisfies its column's public bound.
    assert!(BoundedSecretScalars::new(&column, bits, None).is_ok());
    column
}

fn exact_bounded_columns<C: PastaCurve>() {
    for n in [8, 9, 300, 4097] {
        let (_, bases) = mixed_inputs::<C>(n, 23);
        for count in [1, 2, 6, 7, 13] {
            let columns: Vec<(Vec<C::ScalarExt>, usize)> = (0..count)
                .map(|column| {
                    let bits = BOUNDS[column % BOUNDS.len()];
                    (bounded_column::<C>(n, bits, 200 + column as u64), bits)
                })
                .collect();
            let bounded: Vec<BoundedSecretScalars<'_, C::ScalarExt>> = columns
                .iter()
                .map(|(scalars, bits)| BoundedSecretScalars::new(scalars, *bits, None).unwrap())
                .collect();
            let expected: Vec<C> = columns
                .iter()
                .map(|(scalars, _)| msm_naive::<C>(scalars, &bases))
                .collect();
            for threads in [1, 3, 8] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let shared = SharedMemoryBudget::new(PROCESS_MSM_SCRATCH_BYTES);
                let sums = pool
                    .install(|| {
                        msm_secret_bounded_columns_cancellable::<C>(
                            &bounded,
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
fn bounded_columns_equal_independent_sums_on_both_curves() {
    exact_bounded_columns::<Ep>();
    exact_bounded_columns::<Eq>();
}

fn bounded_columns_admit_the_largest_plan<C: PastaCurve>() {
    let n = 4097;
    let (_, bases) = mixed_inputs::<C>(n, 29);
    let columns: Vec<(Vec<C::ScalarExt>, usize)> = (0..9)
        .map(|column| {
            let bits = [13, 255, 1][column % 3];
            (bounded_column::<C>(n, bits, 300 + column as u64), bits)
        })
        .collect();
    let bounded: Vec<BoundedSecretScalars<'_, C::ScalarExt>> = columns
        .iter()
        .map(|(scalars, bits)| BoundedSecretScalars::new(scalars, *bits, None).unwrap())
        .collect();
    let expected: Vec<C> = columns
        .iter()
        .map(|(scalars, _)| msm_naive::<C>(scalars, &bases))
        .collect();
    let largest = [13, 255, 1]
        .into_iter()
        .map(|bits| {
            pippenger::plan(n, bits, 1, MemoryBudget::DEFAULT.bytes())
                .unwrap()
                .bytes
        })
        .max()
        .unwrap();
    // Room for exactly two of the largest single-task kernels.
    let shared = SharedMemoryBudget::new(largest * 2 + largest / 2);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let sums = pool
        .install(|| {
            msm_secret_bounded_columns_cancellable::<C>(
                &bounded,
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
        shared.peak_bytes() >= largest * 2,
        "two lanes must be admitted together"
    );
    assert!(shared.peak_bytes() <= shared.limit_bytes());
}

#[test]
fn bounded_columns_never_run_more_kernels_than_admitted() {
    bounded_columns_admit_the_largest_plan::<Ep>();
    bounded_columns_admit_the_largest_plan::<Eq>();
}

#[test]
fn bounded_columns_check_lengths_before_work_and_cancel() {
    let n = 300;
    let (_, bases) = mixed_inputs::<Ep>(n, 31);
    let columns: Vec<Vec<Fq>> = (0..4)
        .map(|column| bounded_column::<Ep>(n, 16, 400 + column))
        .collect();
    let mut bounded: Vec<BoundedSecretScalars<'_, Fq>> = columns
        .iter()
        .map(|scalars| BoundedSecretScalars::new(scalars, 16, None).unwrap())
        .collect();
    bounded[3] = BoundedSecretScalars::new(&columns[3][..n - 1], 16, None).unwrap();
    let shared = SharedMemoryBudget::new(PROCESS_MSM_SCRATCH_BYTES);
    assert!(matches!(
        msm_secret_bounded_columns_cancellable::<Ep>(
            &bounded,
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            None
        ),
        Err(MsmError::LengthMismatch(_))
    ));
    assert_eq!(shared.peak_bytes(), 0);
    bounded[3] = BoundedSecretScalars::new(&columns[3], 16, None).unwrap();
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        msm_secret_bounded_columns_cancellable::<Ep>(
            &bounded,
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            Some(&token)
        ),
        Err(MsmError::Cancelled)
    );
    assert_eq!(shared.in_use_bytes(), 0);
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
