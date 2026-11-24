//! Equivalence, public planning and resource refusal of the constant-time
//! secret MSM.

use super::*;
use crate::msm::{BudgetExceeded, msm_naive, msm_public, msm_secret_bounded_columns_cancellable};
use crate::{Ep, Eq, EqAffine, Fq};
use ff::Field;
use group::{Curve, Group};
use rand_chacha::ChaCha20Rng;
use rand_core::{Rng as _, SeedableRng};

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
    // The smallest plan, a few chunks, and room for every loop.
    for budget in [least, 2 * least, 64 << 10, MemoryBudget::DEFAULT.bytes()] {
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

/// The columns through the reference complete-addition chunk kernel
/// ([`add_chunk`]), chunk by chunk on the calling thread.
fn complete_reference<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    bases: &[C::AffineExt],
    window: usize,
    chunk: usize,
) -> Vec<C> {
    let mut tables = vec![C::identity(); chunk * table_entries(window)];
    let mut limbs = vec![[0_u64; 4]; chunk];
    let mut sums = vec![C::identity(); columns.len()];
    for (index, part) in bases.chunks(chunk).enumerate() {
        let mut scratch = Chunk {
            tables: &mut tables[..],
            limbs: &mut limbs[..],
        };
        add_chunk::<C>(
            columns,
            part,
            index * chunk,
            window,
            &mut scratch,
            &mut sums,
            None,
        )
        .unwrap();
    }
    sums
}

/// The columns through the batch-affine chunk kernel ([`add_chunk_batch`]),
/// chunk by chunk on the calling thread, at an explicit window and chunk.
fn batch_affine<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    bases: &[C::AffineExt],
    window: usize,
    chunk: usize,
) -> Vec<C> {
    let mut tables = vec![C::identity(); chunk * table_entries(window)];
    let mut limbs = vec![[0_u64; 4]; chunk];
    let mut batch = BatchScratch::<C>::new(chunk, window);
    let mut sums = vec![C::identity(); columns.len()];
    for (index, part) in bases.chunks(chunk).enumerate() {
        let mut scratch = Chunk {
            tables: &mut tables[..],
            limbs: &mut limbs[..],
        };
        add_chunk_batch::<C>(
            columns,
            None,
            part,
            index * chunk,
            window,
            &mut scratch,
            &mut batch,
            &mut sums,
            None,
        )
        .unwrap();
    }
    sums
}

/// Random columns at many public bounds over bases with identities and a
/// repeated, negated generator: the batch-affine kernel returns the
/// reference kernel's exact group elements at every window and chunk size.
fn batch_equals_complete<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let bounds = [
        0_usize, 1, 2, 3, 4, 5, 7, 8, 11, 16, 63, 64, 65, 128, 181, 254, 255,
    ];
    for n in [1_usize, 2, 3, 17, 2 * CT_CHUNK_BASES + 5] {
        let bases = bases::<C>(&mut rng, n);
        let rows: Vec<Vec<C::ScalarExt>> = bounds
            .iter()
            .map(|&bits| {
                (0..n)
                    .map(|i| match i % 6 {
                        0 => C::ScalarExt::ZERO,
                        1 => largest::<C::ScalarExt>(bits),
                        _ => below::<C::ScalarExt>(&mut rng, bits),
                    })
                    .collect()
            })
            .collect();
        let columns: Vec<(&[C::ScalarExt], usize)> = rows
            .iter()
            .zip(bounds)
            .map(|(row, bits)| (row.as_slice(), bits))
            .collect();
        let naive: Vec<C> = rows.iter().map(|row| msm_naive::<C>(row, &bases)).collect();
        for window in 1..=MAX_CT_WINDOW {
            for chunk in [1_usize, 2, 3, 5, 64, CT_CHUNK_BASES] {
                let reference = complete_reference::<C>(&columns, &bases, window, chunk);
                let batch = batch_affine::<C>(&columns, &bases, window, chunk);
                assert_eq!(reference, naive, "n {n} window {window} chunk {chunk}");
                assert_eq!(batch, reference, "n {n} window {window} chunk {chunk}");
                for (b, r) in batch.iter().zip(&reference) {
                    assert_eq!(b.to_bytes(), r.to_bytes());
                }
            }
        }
    }
}

#[test]
fn batch_affine_sums_equal_the_complete_kernel_on_both_curves() {
    batch_equals_complete::<Ep>(81);
    batch_equals_complete::<Eq>(83);
}

/// Equal and opposite operands at every tree level: one repeated base with
/// equal digits doubles at every level; a base and its negation with equal
/// digits cancel; a scalar and its negation on one base cancel; identity
/// bases and zero digits mix with all of them.
fn forced_collisions<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let g = C::generator().to_affine();
    let minus = (-C::generator()).to_affine();
    let p = C::random(&mut rng).to_affine();
    let s = C::ScalarExt::random(&mut rng);
    let t = C::ScalarExt::random(&mut rng);
    let cases: Vec<(Vec<C::AffineExt>, Vec<C::ScalarExt>)> = vec![
        // Every pair equal (tangent) at every level.
        (vec![g; 64], vec![s; 64]),
        (vec![p; 37], vec![-C::ScalarExt::ONE; 37]),
        // Opposite pairs: G and -G with equal scalars, then sums cancel.
        (
            (0..64)
                .map(|i| if i % 2 == 0 { g } else { minus })
                .collect(),
            vec![s; 64],
        ),
        // A scalar and its negation on one base.
        (
            vec![p; 40],
            (0..40).map(|i| if i % 2 == 0 { t } else { -t }).collect(),
        ),
        // Pairs that become equal only at higher levels.
        (
            (0..48)
                .map(|i| if i % 3 == 0 { minus } else { g })
                .collect(),
            (0..48).map(|i| if i % 3 == 0 { -s } else { s }).collect(),
        ),
        // Identity bases and zero scalars among collisions.
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
        // Only identities, and only zero digits.
        (vec![C::AffineExt::identity(); 9], vec![s; 9]),
        (vec![g; 9], vec![C::ScalarExt::ZERO; 9]),
    ];
    for (bases, scalars) in &cases {
        let expected = msm_naive::<C>(scalars, bases);
        for bits in [FULL_BITS, 254, 64, 9] {
            // Cut the scalars to the bound so it holds.
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
            let want = if bits == FULL_BITS {
                expected
            } else {
                msm_naive::<C>(&cut, bases)
            };
            let columns = [(cut.as_slice(), bits)];
            for window in 1..=MAX_CT_WINDOW {
                for chunk in [1_usize, 2, 7, 16, CT_CHUNK_BASES] {
                    assert_eq!(
                        batch_affine::<C>(&columns, bases, window, chunk),
                        vec![want],
                        "bits {bits} window {window} chunk {chunk}"
                    );
                }
            }
            for threads in [1, 4] {
                let got = pool(threads).install(|| {
                    msm_secret_ct_bounded_columns_cancellable::<C>(
                        &[BoundedSecretScalars::new(&cut, bits, None).unwrap()],
                        bases,
                        MemoryBudget::DEFAULT,
                        &SharedMemoryBudget::new(1 << 22),
                        None,
                    )
                });
                assert_eq!(got.unwrap(), vec![want], "bits {bits} threads {threads}");
            }
        }
    }
}

#[test]
fn batch_affine_handles_equal_opposite_and_identity_operands_on_both_curves() {
    forced_collisions::<Ep>(85);
    forced_collisions::<Eq>(87);
}

/// Every public bound at every window: the carries and the shorter top
/// windows of the batch-affine kernel reconstruct each scalar.
fn batch_window_edges<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let g = C::generator();
    let bases = [
        g.to_affine(),
        (-g).to_affine(),
        g.double().to_affine(),
        C::random(&mut rng).to_affine(),
    ];
    let rows: Vec<_> = (0..=FULL_BITS)
        .map(|bits| {
            [
                largest::<C::ScalarExt>(bits),
                below::<C::ScalarExt>(&mut rng, bits),
                largest::<C::ScalarExt>(bits.saturating_sub(1)),
                C::ScalarExt::ZERO,
            ]
        })
        .collect();
    let columns: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(bits, row)| (row.as_slice(), bits))
        .collect();
    let naive: Vec<C> = rows.iter().map(|row| msm_naive::<C>(row, &bases)).collect();
    for window in 1..=MAX_CT_WINDOW {
        for chunk in [1_usize, 3, 4] {
            let sums = batch_affine::<C>(&columns, &bases, window, chunk);
            for (bits, (sum, want)) in sums.iter().zip(&naive).enumerate() {
                assert_eq!(sum, want, "bound {bits} window {window} chunk {chunk}");
            }
        }
    }
}

#[test]
fn batch_affine_window_and_digit_edges_on_both_curves() {
    batch_window_edges::<Ep>(89);
    batch_window_edges::<Eq>(91);
}

/// One complete affine addition against the projective group law, for
/// random, equal, opposite and identity operands; and the batch inversion
/// against per-element inversion.
fn affine_pairs<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let p = C::random(&mut rng).to_affine();
    let q = C::random(&mut rng).to_affine();
    let o = C::AffineExt::identity();
    for (a, b) in [
        (p, q),
        (p, p),
        (p, -p),
        (o, q),
        (p, o),
        (o, o),
        (q, p),
        (-q, q),
    ] {
        let (den, num, flags) = affine_pair_terms::<C>(
            &a,
            &b,
            a.is_identity().unwrap_u8(),
            b.is_identity().unwrap_u8(),
        );
        assert!(!bool::from(den.is_zero()), "nonzero denominator");
        let lambda = num * den.invert().unwrap();
        let (sum, identity) = affine_pair_sum::<C>(&a, &b, lambda, flags);
        let want = (a.to_curve() + b).to_affine();
        assert_eq!(sum, want);
        assert_eq!(identity, want.is_identity().unwrap_u8());
    }
    let values: Vec<C::Base> = (0..33).map(|_| C::Base::random(&mut rng)).collect();
    let mut inverted = values.clone();
    let mut prefixes = vec![C::Base::ZERO; values.len()];
    invert_nonzero_batch(&mut inverted, &mut prefixes);
    for (value, inverse) in values.iter().zip(&inverted) {
        assert_eq!(*value * inverse, C::Base::ONE);
    }
}

#[test]
fn complete_affine_pairs_and_batch_inversion_on_both_curves() {
    affine_pairs::<Ep>(93);
    affine_pairs::<Eq>(95);
}

/// The batch-affine scratch fits the admitted plan's accounted bytes.
#[test]
fn batch_scratch_is_accounted_in_the_plan() {
    for window in 1..=MAX_CT_WINDOW {
        for chunk in [1_usize, 2, 3, 64, CT_CHUNK_BASES] {
            let scratch = BatchScratch::<Eq>::new(chunk, window);
            let bytes = scratch.affine.len() * size_of::<EqAffine>()
                + scratch.table_identity.len()
                + scratch.selected.len() * size_of::<EqAffine>()
                + scratch.selected_identity.len()
                + scratch.lane_lengths.len() * size_of::<usize>()
                + (scratch.denominators.len() + scratch.numerators.len() + scratch.prefixes.len())
                    * size_of::<Fq>()
                + scratch.pair_flags.len();
            assert_eq!(Some(bytes), batch_scratch_bytes::<Eq>(chunk, window));
            assert!(scratch.selected.len() >= column_windows(FULL_BITS, window) * chunk);
        }
    }
}

/// Public supports: a column reads only the bases its support names. With
/// every skipped scalar zero, the sums equal the unsupported kernel's and the
/// naive sums, for empty, full, sparse and run-shaped supports, through every
/// plan size and pool size.
fn supported_columns<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let n = 3 * CT_CHUNK_BASES + 11;
    let words = n.div_ceil(64) + 1;
    let bases = bases::<C>(&mut rng, n);
    let bounds = [255_usize, 11, 181, 64, 1, 0, 255];
    let supports: Vec<Vec<u64>> = (0..bounds.len())
        .map(|c| {
            let mut bits = vec![0_u64; words];
            for i in 0..n {
                let set = match c {
                    0 => true,
                    1 => false,
                    2 => i % 3 != 1,
                    3 => (i / 100) % 2 == 0,
                    4 => i % 64 < 5,
                    _ => rng.next_u64() % 2 == 0,
                };
                if set {
                    bits[i / 64] |= 1 << (i % 64);
                }
            }
            bits
        })
        .collect();
    let rows: Vec<Vec<C::ScalarExt>> = bounds
        .iter()
        .zip(&supports)
        .map(|(&bits, support)| {
            (0..n)
                .map(|i| {
                    if support[i / 64] >> (i % 64) & 1 == 1 {
                        below::<C::ScalarExt>(&mut rng, bits)
                    } else {
                        C::ScalarExt::ZERO
                    }
                })
                .collect()
        })
        .collect();
    let checked: Vec<BoundedSecretScalars<'_, C::ScalarExt>> = rows
        .iter()
        .zip(bounds)
        .map(|(row, bits)| BoundedSecretScalars::new(row, bits, None).unwrap())
        .collect();
    let views: Vec<&[u64]> = supports.iter().map(Vec::as_slice).collect();
    let expected: Vec<C> = rows.iter().map(|row| msm_naive::<C>(row, &bases)).collect();
    for budget in [64 << 10, MemoryBudget::DEFAULT.bytes()] {
        for threads in [1, 4] {
            let shared = SharedMemoryBudget::new(1 << 22);
            let (supported, plain) = pool(threads).install(|| {
                (
                    msm_secret_ct_bounded_columns_supported_cancellable::<C>(
                        &checked,
                        &views,
                        &bases,
                        MemoryBudget::new(budget),
                        &shared,
                        None,
                    )
                    .unwrap(),
                    msm_secret_ct_bounded_columns_cancellable::<C>(
                        &checked,
                        &bases,
                        MemoryBudget::new(budget),
                        &shared,
                        None,
                    )
                    .unwrap(),
                )
            });
            assert_eq!(supported, expected, "budget {budget} threads {threads}");
            assert_eq!(plain, expected);
            assert_eq!(shared.in_use_bytes(), 0);
        }
    }
    // One support per column, each covering every base.
    assert!(matches!(
        msm_secret_ct_bounded_columns_supported_cancellable::<C>(
            &checked,
            &views[..1],
            &bases,
            MemoryBudget::DEFAULT,
            &SharedMemoryBudget::new(1 << 22),
            None,
        ),
        Err(MsmError::LengthMismatch(_))
    ));
    let short: Vec<&[u64]> = views.iter().map(|v| &v[..(n / 64) - 1]).collect();
    assert!(matches!(
        msm_secret_ct_bounded_columns_supported_cancellable::<C>(
            &checked,
            &short,
            &bases,
            MemoryBudget::DEFAULT,
            &SharedMemoryBudget::new(1 << 22),
            None,
        ),
        Err(MsmError::LengthMismatch(_))
    ));
}

#[test]
fn public_supports_skip_only_zero_positions_on_both_curves() {
    supported_columns::<Ep>(97);
    supported_columns::<Eq>(99);
}
