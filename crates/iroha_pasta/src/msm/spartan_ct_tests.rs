//! Equivalence, public planning and resource refusal of the constant-time
//! secret MSM.

use super::*;
use crate::msm::{BudgetExceeded, msm_naive, msm_public, msm_secret_bounded_columns_cancellable};
use crate::{Ep, Eq, Fq};
use ff::Field;
use group::{Curve, Group};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

/// `2^bits - 1` for `bits < 255`; the largest canonical scalar otherwise.
fn largest<F: PastaField>(bits: usize) -> F {
    if bits >= FULL_BITS {
        return -F::ONE;
    }
    (0..bits).fold(F::ONE, |value, _| value.double()) - F::ONE
}

/// A random scalar below `2^bits`: a random canonical value with every bit
/// from `bits` upwards cleared.
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

/// Bases with identities, a repeated generator and its negation among random
/// points: the cases a batch-affine kernel must special-case.
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

fn full_width<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    for n in [
        0_usize,
        1,
        2,
        5,
        9,
        33,
        CT_CHUNK_BASES + 3,
        3 * CT_CHUNK_BASES + 1,
    ] {
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
        assert_eq!(
            msm_public::<C>(&scalars, &bases, MemoryBudget::DEFAULT).unwrap(),
            expected
        );
        for threads in [1, 4] {
            let shared = SharedMemoryBudget::new(1 << 20);
            let actual = pool(threads)
                .install(|| {
                    msm_secret_ct_cancellable::<C>(
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
            assert!(shared.peak_bytes() <= shared.limit_bytes());
        }
    }
}

#[test]
fn full_width_sums_equal_independent_multiplications_on_both_curves() {
    full_width::<Ep>(1);
    full_width::<Eq>(2);
    // One concrete product against the complete scalar multiplier.
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let point = Ep::random(&mut rng).to_affine();
    let scalar = Fq::random(&mut rng);
    assert_eq!(
        msm_secret_ct_cancellable::<Ep>(
            &[scalar],
            &[point],
            MemoryBudget::DEFAULT,
            &SharedMemoryBudget::new(1 << 16),
            None,
        )
        .unwrap(),
        point * scalar
    );
}

fn bounded_columns<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let bounds = [0_usize, 1, 2, 3, 8, 11, 16, 64, 80, 128, 254, 255];
    for n in [1_usize, 7, 40, 2 * CT_CHUNK_BASES + 5] {
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
            let shared = SharedMemoryBudget::new(256 << 10);
            let workers = pool(threads);
            let (constant, buckets) = workers.install(|| {
                (
                    msm_secret_ct_bounded_columns_cancellable::<C>(
                        &checked,
                        &bases,
                        MemoryBudget::DEFAULT,
                        &shared,
                        None,
                    )
                    .unwrap(),
                    msm_secret_bounded_columns_cancellable::<C>(
                        &checked,
                        &bases,
                        MemoryBudget::DEFAULT,
                        &shared,
                        None,
                    )
                    .unwrap(),
                )
            });
            assert_eq!(constant, expected, "n = {n}, threads = {threads}");
            assert_eq!(buckets, expected, "n = {n}, threads = {threads}");
            for (column, sum) in checked.iter().zip(&expected) {
                let single = workers.install(|| {
                    msm_secret_ct_bounded_cancellable::<C>(
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
fn bounded_columns_equal_naive_and_bucket_sums_on_both_curves() {
    bounded_columns::<Ep>(4);
    bounded_columns::<Eq>(5);
}

/// Signed radix-16 has one carry digit when the public bound is a
/// multiple of four. Every residue class and every limb boundary must
/// reconstruct the original scalar, including a full carry chain.
fn signed_window_bounds<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let g = C::generator();
    let bases = [g.to_affine(), (-g).to_affine(), g.double().to_affine()];
    let rows: Vec<_> = (0..=FULL_BITS)
        .map(|bits| {
            [
                largest::<C::ScalarExt>(bits),
                below::<C::ScalarExt>(&mut rng, bits),
                largest::<C::ScalarExt>(bits.saturating_sub(1)),
            ]
        })
        .collect();
    let columns: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(bits, row)| (row.as_slice(), bits))
        .collect();
    let mut sums = vec![C::identity(); columns.len()];
    run_stack::<C>(&columns, &bases, 4, None, &mut sums).unwrap();
    for (bits, (row, sum)) in rows.iter().zip(sums).enumerate() {
        assert_eq!(sum, msm_naive::<C>(row, &bases), "public bound {bits}");
    }
}

#[test]
fn signed_radix_carries_cover_every_public_width_on_both_curves() {
    signed_window_bounds::<Ep>(71);
    signed_window_bounds::<Eq>(73);
}

/// Different budgets and pool sizes change the chunking and loop count only;
/// every plan returns the same exact group elements.
fn plans_agree<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let n = 3 * CT_CHUNK_BASES + 7;
    let bases = bases::<C>(&mut rng, n);
    let bounds = [1_usize, 64, 255];
    let rows: Vec<Vec<C::ScalarExt>> = bounds
        .iter()
        .map(|&bits| {
            (0..n)
                .map(|_| below::<C::ScalarExt>(&mut rng, bits))
                .collect()
        })
        .collect();
    let checked: Vec<BoundedSecretScalars<'_, C::ScalarExt>> = rows
        .iter()
        .zip(bounds)
        .map(|(row, bits)| BoundedSecretScalars::new(row, bits, None).unwrap())
        .collect();
    let expected: Vec<C> = rows.iter().map(|row| msm_naive::<C>(row, &bases)).collect();
    let least = plan_bytes::<C>(1, ct_window(bounds), bounds.len(), 1).unwrap();
    for budget in [least, 4 << 10, 64 << 10, MemoryBudget::DEFAULT.bytes()] {
        for threads in [1, 4] {
            let shared = SharedMemoryBudget::new(1 << 20);
            let sums = pool(threads)
                .install(|| {
                    msm_secret_ct_bounded_columns_cancellable::<C>(
                        &checked,
                        &bases,
                        MemoryBudget::new(budget),
                        &shared,
                        None,
                    )
                })
                .unwrap();
            assert_eq!(sums, expected, "budget = {budget}, threads = {threads}");
            assert!(shared.peak_bytes() <= budget);
            assert_eq!(shared.in_use_bytes(), 0);
        }
    }
}

#[test]
fn every_admitted_plan_gives_the_same_sums_on_both_curves() {
    plans_agree::<Ep>(6);
    plans_agree::<Eq>(7);
}

#[test]
fn window_is_a_function_of_public_bounds() {
    assert_eq!(ct_window([255]), 4);
    assert_eq!(ct_window([64]), 3);
    assert_eq!(ct_window([16]), 2);
    assert_eq!(ct_window([1]), 1);
    assert_eq!(ct_window([0]), 1);
    assert_eq!(ct_window(core::iter::empty::<usize>()), 1);
    assert_eq!(ct_window(vec![1; 64]), 1);
    for bits in 0..=FULL_BITS {
        let window = ct_window([bits]);
        assert!((1..=MAX_CT_WINDOW).contains(&window), "bits = {bits}");
        assert_eq!(window, ct_window([bits]));
    }
}

#[test]
fn admitted_plans_stay_within_budget_and_reserve_exactly() {
    let shared = SharedMemoryBudget::new(1 << 20);
    for (n, columns) in [(1_usize, 1_usize), (300, 1), (5000, 9)] {
        for window in 1..=MAX_CT_WINDOW {
            let least = plan_bytes::<Ep>(1, window, columns, 1).unwrap();
            for budget in [least, least + 1000, 64 << 10, 1 << 20] {
                for threads in [1, 4, 16] {
                    let admitted = admit::<Ep>(
                        n,
                        window,
                        columns,
                        threads,
                        MemoryBudget::new(budget),
                        &shared,
                    )
                    .unwrap();
                    // Other tests share the process ceiling: contention may
                    // refuse a plan here, which the kernel turns into the
                    // allocation-free path.
                    let Some((plan, scratch)) = admitted else {
                        continue;
                    };
                    assert_eq!(scratch.bytes(), plan.bytes);
                    assert!(plan.bytes <= budget);
                    assert_eq!(
                        Some(plan.bytes),
                        plan_bytes::<Ep>(plan.chunk, window, columns, plan.loops)
                    );
                    assert_eq!(plan.window, window);
                    assert!((1..=CT_CHUNK_BASES.min(n)).contains(&plan.chunk));
                    assert!(plan.loops >= 1 && plan.loops <= threads);
                    assert!(plan.loops <= n.div_ceil(plan.chunk));
                }
            }
            let refused = admit::<Ep>(n, window, columns, 4, MemoryBudget::new(least - 1), &shared);
            assert!(matches!(
                refused,
                Err(MsmError::Budget(BudgetExceeded { required, budget }))
                    if required == least && budget == least - 1
            ));
        }
    }
    assert_eq!(shared.in_use_bytes(), 0);
}

fn refusals<C: PastaCurve>() {
    let n = 40;
    let bases = vec![C::generator().to_affine(); n];
    let scalars = vec![-C::ScalarExt::ONE; n];
    let expected = msm_naive::<C>(&scalars, &bases);
    let least = plan_bytes::<C>(1, ct_window([FULL_BITS]), 1, 1).unwrap();
    let shared = SharedMemoryBudget::new(1 << 20);

    // A per-kernel budget below one base on one loop is refused before any
    // allocation; exactly that plan is admitted.
    for budget in [0, least - 1] {
        assert_eq!(
            msm_secret_ct_cancellable::<C>(
                &scalars,
                &bases,
                MemoryBudget::new(budget),
                &shared,
                None
            ),
            Err(MsmError::Budget(BudgetExceeded {
                required: least,
                budget
            }))
        );
    }
    assert_eq!(
        msm_secret_ct_cancellable::<C>(&scalars, &bases, MemoryBudget::new(least), &shared, None)
            .unwrap(),
        expected
    );
    assert!(shared.peak_bytes() <= least);

    // A refused shared reservation is contention, not an error: the sum runs
    // without heap scratch.
    let full = SharedMemoryBudget::new(0);
    assert_eq!(
        msm_secret_ct_cancellable::<C>(&scalars, &bases, MemoryBudget::DEFAULT, &full, None)
            .unwrap(),
        expected
    );
    assert_eq!(full.peak_bytes(), 0);

    // Zero public bounds need no scratch and read no scalar.
    let zeros = vec![C::ScalarExt::ZERO; n];
    assert_eq!(
        msm_secret_ct_bounded_columns_cancellable::<C>(
            &[BoundedSecretScalars::new(&zeros, 0, None).unwrap()],
            &bases,
            MemoryBudget::new(0),
            &shared,
            None,
        )
        .unwrap(),
        vec![C::identity()]
    );
    let one = BoundedSecretScalars::new(&scalars, FULL_BITS, None).unwrap();
    assert!(matches!(
        msm_secret_ct_bounded_columns_cancellable::<C>(
            &[one],
            &bases,
            MemoryBudget::new(0),
            &shared,
            None,
        ),
        Err(MsmError::Budget(_))
    ));

    // Malformed inputs fail before any work.
    assert!(matches!(
        msm_secret_ct_cancellable::<C>(
            &scalars[..n - 1],
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            None
        ),
        Err(MsmError::LengthMismatch(_))
    ));
    let mut sums = [C::identity()];
    assert_eq!(
        columns_into::<C>(
            &[(&scalars[..], FULL_BITS + 1)],
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            None,
            &mut sums,
        ),
        Err(MsmError::ScalarBound)
    );
    let mut too_few: [C; 0] = [];
    assert!(matches!(
        columns_into::<C>(
            &[(&scalars[..], FULL_BITS)],
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            None,
            &mut too_few,
        ),
        Err(MsmError::LengthMismatch(_))
    ));

    // Cancellation is reported, never a partial sum, and all scratch is released.
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(
        msm_secret_ct_cancellable::<C>(
            &scalars,
            &bases,
            MemoryBudget::DEFAULT,
            &shared,
            Some(&cancelled)
        ),
        Err(MsmError::Cancelled)
    );
    assert_eq!(shared.in_use_bytes(), 0);
}

#[test]
fn resource_refusal_contention_and_malformed_inputs_on_both_curves() {
    refusals::<Ep>();
    refusals::<Eq>();
}
