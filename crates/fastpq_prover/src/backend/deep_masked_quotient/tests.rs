//! Independent full-polynomial quotient identities and exact source-plan refusals.

use super::*;
use crate::{
    backend::{deep_masked_replay::ReplayLimits, field_pow},
    gadgets::compact_smt_air::{PublicStatement, PublicUpdate},
};
use fastpq_isi::FASTPQ_FINAL_V1;

fn policy() -> QuotientLimits {
    QuotientLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
    }
}
fn replay_plan() -> MaskedReplayPlan {
    MaskedReplayPlan::new(ReplayLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: 4,
    })
    .unwrap()
}
fn digest(seed: u8) -> [u32; 8] {
    let digest = iroha_crypto::Hash::new([seed; 33]);
    let bytes: &[u8; 32] = digest.as_ref();
    core::array::from_fn(|index| {
        u32::from_le_bytes(bytes[4 * index..4 * index + 4].try_into().unwrap())
    })
}
fn air() -> CompactTransferAir {
    CompactTransferAir::new(
        &PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: digest(1),
                    new_leaf: digest(2),
                    path: 7,
                },
                PublicUpdate {
                    old_leaf: digest(3),
                    new_leaf: digest(4),
                    path: 11,
                },
            ],
            old_root: digest(5),
            new_root: digest(6),
        },
        None,
    )
    .unwrap()
}
fn horner(values: &[F], x: F) -> F {
    values
        .iter()
        .rev()
        .fold(F::ZERO, |value, &coefficient| value.mul(x).add(coefficient))
}

#[test]
fn four_stripe_full_numerator_matches_independent_convolution_and_exact_division() {
    const N: usize = 8;
    let generator = field_pow(FASTPQ_FINAL_V1.trace_root, (TRACE_ROWS / N) as u64);
    let source_coefficients = [
        F::embed_base(13),
        F::embed_base(17),
        F::embed_base(19),
        F::embed_base(23),
    ];
    let source = (0..N)
        .map(|i| {
            horner(
                &source_coefficients,
                F::embed_base(field_pow(generator, i as u64)),
            )
            .coefficients()[0]
        })
        .collect::<Vec<_>>();
    let mut replay = MaskedTraceReplay::arithmetic_fixture(&[&source], N, 2).unwrap();
    let mask = (N..replay.coefficient_extent())
        .map(|degree| F::embed_base(replay.coefficient(0, degree)))
        .collect::<Vec<_>>();
    let shifted = mask
        .iter()
        .enumerate()
        .map(|(degree, &value)| value.mul_base(field_pow(generator, degree as u64)))
        .collect::<Vec<_>>();
    let alpha = F::new([7, 11, 13, 17]).unwrap();
    let mut expected = vec![F::ZERO; 2 * N];
    for (degree, (&a, &b)) in mask.iter().zip(&shifted).enumerate() {
        expected[degree] = a.add(alpha.mul(b));
    }
    // Numerator = (w-C) + alpha*(w(gX)-C(gX)) + (w-C)*(w(gX)-C(gX)).
    // Its quotient is r + alpha*r(gX) + (X^N-1)*r*r(gX).
    for (i, &a) in mask.iter().enumerate() {
        for (j, &b) in shifted.iter().enumerate() {
            expected[i + j] = expected[i + j].sub(a.mul(b));
            expected[N + i + j] = expected[N + i + j].add(a.mul(b));
        }
    }
    let domain =
        PolynomialDomain::new(4 * N, F::embed_base(COSET_OFFSET), 4 * N, usize::MAX).unwrap();
    let evaluate = |_: usize, x: F, row: &[u64], next: &[u64], (): &mut ()| {
        let first = F::embed_base(row[0]).sub(horner(&source_coefficients, x));
        let second =
            F::embed_base(next[0]).sub(horner(&source_coefficients, x.mul_base(generator)));
        Ok(first.add(alpha.mul(second)).add(first.mul(second)))
    };
    // A job count other than the fixed one is refused before any replay pass.
    assert!(
        interpolate_numerator(&mut replay, domain, &mut [(); NUMERATOR_JOBS - 1], evaluate)
            .is_err()
    );
    let mut workspaces = [(); NUMERATOR_JOBS];
    let numerator = interpolate_numerator(&mut replay, domain, &mut workspaces, evaluate).unwrap();
    let mut expected_numerator = vec![F::ZERO; 4 * N];
    for (degree, &coefficient) in expected.iter().enumerate() {
        expected_numerator[degree] = expected_numerator[degree].sub(coefficient);
        expected_numerator[degree + N] = expected_numerator[degree + N].add(coefficient);
    }
    assert_eq!(&*numerator, expected_numerator);
    let division =
        VanishingDivisionPlan::new(N, 4 * N, 3 * N, 2 * N, usize::MAX, usize::MAX).unwrap();
    let quotient = division.divide(&numerator).unwrap();
    assert_eq!(quotient.coefficients(), expected);
    let pair = PairMaskingPlan::new(
        PairMaskingShape {
            split: N,
            quotient_coefficients: 2 * N,
            quotient_degree_bound: 2 * N,
            mask_coefficients: 2,
            mask_degree_bound: 2,
        },
        PairMaskingLimits {
            max_chunk_degree_bound: 2 * N,
            max_payload_bytes: usize::MAX,
            max_work_units: usize::MAX,
        },
    )
    .unwrap();
    let chunks = pair
        .apply(quotient.coefficients(), replay.quotient_mask())
        .unwrap();
    let result = DeepMaskedQuotient {
        chunks,
        degree_bounds: pair.degree_bounds(),
    };
    assert_eq!(result.degree_bounds(), [N + 2, N]);
    for x in [F::ZERO, F::ONE, F::new([3, 5, 7, 11]).unwrap()] {
        assert_eq!(
            horner(result.chunks()[0], x).add(x.power(N as u64).mul(horner(result.chunks()[1], x))),
            horner(&expected, x)
        );
    }
    let bad = interpolate_numerator(
        &mut replay,
        domain,
        &mut workspaces,
        |index, x, row, next, workspace| {
            evaluate(index, x, row, next, workspace).map(|value| value.add(F::ONE))
        },
    )
    .unwrap();
    assert!(
        division.divide(&bad).is_err(),
        "nonzero source residual must never be divided pointwise and accepted"
    );
}

#[test]
fn full_relation_plan_uses_actual_degrees_and_refuses_budget_and_source_mismatch() {
    let air = air();
    let plan = DeepQuotientPlan::new(&air, replay_plan(), policy()).unwrap();
    assert_eq!(plan.domain.rows(), 262_144);
    assert_eq!(plan.numerator_bound, 196_803);
    assert_eq!(plan.division.quotient_degree_bound(), 131_267);
    assert_eq!(plan.pair.degree_bounds(), [65_614, 65_731]);
    assert_eq!(plan.cycle, 2048);
    assert!(plan.payload_bytes > replay_plan().payload_bytes);
    assert!(plan.payload_bytes < 2 * (1 << 30));
    eprintln!(
        "DEEP full numerator payload={} structural_work={}",
        plan.payload_bytes, plan.work_units
    );
    assert!(plan.work_units > replay_plan().work_units);
    assert!(
        DeepQuotientPlan::new(
            &air,
            replay_plan(),
            QuotientLimits {
                max_payload_bytes: plan.payload_bytes - 1,
                ..policy()
            }
        )
        .is_err()
    );
    assert!(
        DeepQuotientPlan::new(
            &air,
            replay_plan(),
            QuotientLimits {
                max_work_units: plan.work_units - 1,
                ..policy()
            }
        )
        .is_err()
    );
    let mut small = MaskedTraceReplay::arithmetic_fixture(&[&[1, 2, 3, 4]], 4, 1).unwrap();
    assert!(DeepQuotientPlan::new(&air, small.plan(), policy()).is_err());
    assert!(plan.build(&mut small, &vec![F::ZERO; SLOT_COUNT]).is_err());
}

#[test]
fn public_reconstruction_keeps_all_301_private_and_41_known_coordinates() {
    let retained = core::array::from_fn::<_, COMMITTED_COLUMN_COUNT, _>(|column| column as u64 + 1);
    let known = core::array::from_fn::<_, PUBLIC_COLUMN_COUNT, _>(|column| 1001 + column as u64);
    let mut output = [0; COLUMN_COUNT];
    reconstruct(&retained, &known, &mut output).unwrap();
    for (&column, &value) in COMMITTED_COLUMNS.iter().zip(&retained) {
        assert_eq!(output[column], value);
    }
    for (&column, &value) in PUBLIC_COLUMNS.iter().zip(&known) {
        assert_eq!(output[column], value);
    }
    assert!(reconstruct(&retained[..300], &known, &mut output).is_err());
    assert!(reconstruct(&retained, &known, &mut output[..341]).is_err());
    let public = PublicColumnReconstruction::new(&DeepGeometry::polynomial_parameters()).unwrap();
    let domain = PolynomialDomain::new(
        4 * TRACE_ROWS,
        F::embed_base(COSET_OFFSET),
        4 * TRACE_ROWS,
        usize::MAX,
    )
    .unwrap();
    for index in [0, 1, 2047, 262_143] {
        assert_eq!(
            public.evaluate(domain.point(index).unwrap()).unwrap(),
            public
                .evaluate(domain.point(index % 2048).unwrap())
                .unwrap()
        );
        // The numerator evaluates these columns in the base field: on its
        // base-field coset that is the same value, embedded.
        let base = public.evaluate(domain.base_point(index).unwrap()).unwrap();
        assert_eq!(
            public.evaluate(domain.point(index).unwrap()).unwrap(),
            base.map(F::embed_base)
        );
    }
}

#[test]
fn parallel_rows_preserve_serial_field_results_and_range_ownership() {
    let weighted = |index: usize, slot: usize| {
        F::new([index as u64 + 1, slot as u64 + 3, 5, 7])
            .unwrap()
            .mul(F::new([11, 13, slot as u64 + 17, 19]).unwrap())
    };
    for threads in [1, 2, 6] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        for length in [1, 31, 32, 33, 65, 257] {
            let mut values = SecretPolynomial::zeroed(length).unwrap();
            let mut workspaces = (0..NUMERATOR_JOBS)
                .map(|_| (None, SecretPolynomial::zeroed(SLOT_COUNT).unwrap()))
                .collect::<Vec<(Option<usize>, SecretPolynomial<F>)>>();
            pool.install(|| {
                evaluate_parallel_rows(&mut values, &mut workspaces, |index, workspace| {
                    if let Some(previous) = workspace.0 {
                        assert_eq!(index, previous + 1);
                    }
                    workspace.0 = Some(index);
                    for (slot, value) in workspace.1.iter_mut().enumerate() {
                        *value = weighted(index, slot);
                    }
                    Ok(workspace.1.iter().copied().fold(F::ZERO, F::add))
                })
            })
            .unwrap();
            for (index, &value) in values.iter().enumerate() {
                assert_eq!(
                    value,
                    (0..SLOT_COUNT).fold(F::ZERO, |sum, slot| sum.add(weighted(index, slot)))
                );
            }
            let chunk = length.div_ceil(NUMERATOR_JOBS);
            let jobs = length.div_ceil(chunk);
            assert_eq!(
                workspaces
                    .iter()
                    .filter(|workspace| workspace.0.is_some())
                    .count(),
                jobs
            );
        }
    }
}

#[test]
fn parallel_failures_join_all_ranges_and_return_the_earliest_row_error() {
    use std::sync::atomic::{AtomicBool, Ordering};
    for threads in [1, 2, 6] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let visited = (0..65).map(|_| AtomicBool::new(false)).collect::<Vec<_>>();
        let mut values = SecretPolynomial::zeroed(65).unwrap();
        let mut workspaces = [(); NUMERATOR_JOBS];
        let result = pool.install(|| {
            evaluate_parallel_rows(&mut values, &mut workspaces, |index, ()| {
                visited[index].store(true, Ordering::SeqCst);
                if [3, 7, 11].contains(&index) {
                    return Err(Error::QueryIndexOutOfRange { index, len: 65 });
                }
                Ok(F::embed_base(index as u64))
            })
        });
        assert!(matches!(
            result,
            Err(Error::QueryIndexOutOfRange { index: 3, len: 65 })
        ));
        assert!(visited[64].load(Ordering::SeqCst));
        assert_eq!(values[64], F::embed_base(64));
    }
    let mut output = [F::ONE];
    assert!(
        evaluate_parallel_rows(&mut output, &mut [(); NUMERATOR_JOBS - 1], |_, ()| panic!(
            "invalid shape executed"
        ))
        .is_err()
    );
    assert!(
        evaluate_parallel_rows(&mut [], &mut [(); NUMERATOR_JOBS], |_, ()| panic!(
            "empty shape executed"
        ))
        .is_err()
    );
    assert_eq!(output, [F::ONE]);
}

#[test]
fn parallel_owned_workspaces_erase_on_failure_and_unwind() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use zeroize::Zeroize;
    #[derive(Default)]
    struct Tracked {
        value: u64,
        cleared: Option<Arc<AtomicUsize>>,
    }
    impl Zeroize for Tracked {
        fn zeroize(&mut self) {
            self.value.zeroize();
            if let Some(cleared) = &self.cleared {
                assert_eq!(self.value, 0);
                cleared.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    for panic in [false, true] {
        let cleared = Arc::new(AtomicUsize::new(0));
        let outcome = std::panic::catch_unwind(|| {
            let mut workspaces = (0..NUMERATOR_JOBS)
                .map(|_| {
                    let mut scratch = SecretPolynomial::<Tracked>::zeroed(2).unwrap();
                    for value in scratch.iter_mut() {
                        value.value = 71;
                        value.cleared = Some(Arc::clone(&cleared));
                    }
                    scratch
                })
                .collect::<Vec<_>>();
            let mut values = SecretPolynomial::zeroed(65).unwrap();
            let result = evaluate_parallel_rows(&mut values, &mut workspaces, |index, _| {
                if index == 0 {
                    assert!(!panic, "test-only parallel failure");
                    return Err(invalid("test-only row failure"));
                }
                Ok(F::ONE)
            });
            assert!(result.is_err());
        });
        assert_eq!(outcome.is_err(), panic);
        assert_eq!(cleared.load(Ordering::SeqCst), 2 * NUMERATOR_JOBS);
    }
}

#[test]
fn every_numerator_job_is_charged_before_allocation_independently_of_pool_size() {
    let air = air();
    let plan = DeepQuotientPlan::new(&air, replay_plan(), policy()).unwrap();
    let public = air.polynomial_preparation_cost(plan.domain).unwrap();
    let rows = plan.domain.rows();
    // The public preparation holds one graph workspace; the other jobs each
    // hold another, with both complete rows, both retained rows and residues.
    let job_cells = NUMERATOR_JOBS * (SLOT_COUNT + 2 * COLUMN_COUNT + 2 * COMMITTED_COLUMN_COUNT)
        + (NUMERATOR_JOBS - 1) * public.scratch_cells;
    let job_owners =
        NUMERATOR_JOBS * (size_of::<NumeratorWorkspace<'_, '_>>() + size_of::<Result<()>>());
    assert_eq!(
        plan.payload_bytes,
        replay_plan().payload_bytes
            + public.payload_bytes
            + (plan.cycle * PUBLIC_COLUMN_COUNT + 3 * PHYSICAL_HASH_ROWS) * 8
            + (3 * rows + TRACE_ROWS) * F::BYTES
            + job_cells * 8
            + job_owners
            + plan.division.payload_bytes()
            + plan.pair.payload_bytes()
    );
    // Base-field caches: the fixed columns alone are 49 one-word lanes.
    assert!(public.payload_bytes > 49 * rows * 8);
    assert!(public.payload_bytes < 49 * rows * F::BYTES);
    let expected = (plan.payload_bytes, plan.work_units);
    for threads in [1, 6] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        assert_eq!(
            pool.install(|| {
                let plan = DeepQuotientPlan::new(&air, replay_plan(), policy()).unwrap();
                (plan.payload_bytes, plan.work_units)
            }),
            expected
        );
    }
}
