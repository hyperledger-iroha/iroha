//! Coefficient replay/folding against independent Horner and inverse-fiber transforms.

use super::*;
use crate::backend::{GOLDILOCKS_MODULUS, fri_fold::FriFoldPlan};
fn limits(passes: usize) -> CoefficientLimits {
    CoefficientLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: passes,
    }
}
fn dense(seed: usize) -> F {
    F::new([
        seed as u64 + 1,
        seed as u64 + 7,
        seed as u64 + 11,
        seed as u64 + 19,
    ])
    .unwrap()
}
fn horner(coefficients: &[F], point: u64) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &value| sum.mul_base(point).add(value))
}

#[test]
fn fp4_replay_three_oracles_match_horner_in_every_lane_and_natural_position() {
    let plan = CoefficientReplayPlan::with_shape(1024, 16, 3, 1, limits(1)).unwrap();
    let first = (0..16).map(dense).collect::<Vec<_>>();
    let second = (0..7).map(|i| dense(i + 31)).collect::<Vec<_>>();
    let sources = [&first[..], &second[..], &[][..]];
    let mut replay = CoefficientReplay::new(plan, &sources).unwrap();
    assert_eq!(replay.plan(), plan);
    let mut visited = vec![false; plan.rows()];
    replay
        .visit_all(|stripe| {
            for row in 0..stripe.rows() {
                let index = stripe.global_index(row);
                assert!(!visited[index]);
                visited[index] = true;
                for (column, source) in sources.iter().enumerate() {
                    assert_eq!(
                        stripe.value(column, row)?,
                        horner(source, plan.domain().point(index))
                    );
                }
            }
            assert!(stripe.value(3, 0).is_err());
            assert!(stripe.value(0, stripe.rows()).is_err());
            assert!(stripe.fiber(0, &mut [F::ZERO]).is_err());
            Ok(())
        })
        .unwrap();
    assert!(visited.into_iter().all(|value| value));
    assert!(
        replay
            .visit_all(|_| panic!("exhausted replay callback"))
            .is_err()
    );
}

#[test]
fn every_fri_coefficient_fold_matches_existing_inverse_fiber_transform() {
    for round in 0..5 {
        let plan = CoefficientReplayPlan::fri(round, limits(1)).unwrap();
        assert_eq!(plan.stripes(), 64);
        assert_eq!(plan.arity(), FRI_ARITIES[round]);
        let mut coefficients = vec![F::ZERO; plan.degree()];
        for (i, value) in coefficients[..32.min(plan.degree())].iter_mut().enumerate() {
            *value = dense(i + 53);
        }
        coefficients[plan.degree() - 1] = dense(97);
        let beta = dense(101 + round);
        let folded = fold_coefficients(round, &coefficients, beta, usize::MAX).unwrap();
        assert_eq!(folded.len(), FRI_DEGREES[round + 1]);
        let kernel = FriFoldPlan::new(
            plan.arity(),
            plan.domain().coset_generator(plan.rows() / plan.arity()),
        )
        .unwrap();
        for index in [0, 1, plan.rows() / plan.arity() - 1] {
            let fiber = (0..plan.arity())
                .map(|k| {
                    horner(
                        &coefficients,
                        plan.domain()
                            .point(index + k * (plan.rows() / plan.arity())),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                kernel
                    .fold_coset(&fiber, beta, plan.domain().point(index))
                    .unwrap(),
                horner(&folded, plan.domain().folded(plan.arity()).point(index))
            );
        }
        let exact = (coefficients.len() + folded.len()) * F::BYTES;
        assert!(fold_coefficients(round, &coefficients, beta, exact - 1).is_err());
    }
}

#[test]
fn stripe_fibers_keep_verifier_order_and_complete_terminal_coset() {
    for (rows, degree, arity) in [(1024, 16, 16), (1024, 16, 8), (512, 8, 4), (128, 2, 1)] {
        let plan = CoefficientReplayPlan::with_shape(rows, degree, 1, arity, limits(1)).unwrap();
        let coefficients = (0..degree).map(|i| dense(i + 107)).collect::<Vec<_>>();
        let sources = [&coefficients[..]];
        let mut replay = CoefficientReplay::new(plan, &sources).unwrap();
        let mut seen = vec![false; rows / arity];
        replay
            .visit_all(|stripe| {
                for row in 0..stripe.fiber_rows() {
                    let index = stripe.global_index(row);
                    let mut fiber = vec![F::ZERO; arity];
                    stripe.fiber(row, &mut fiber)?;
                    assert!(!seen[index]);
                    seen[index] = true;
                    for (k, value) in fiber.into_iter().enumerate() {
                        assert_eq!(
                            value,
                            horner(
                                &coefficients,
                                plan.domain().point(index + k * (rows / arity))
                            )
                        );
                    }
                }
                assert!(
                    stripe
                        .fiber(stripe.fiber_rows(), &mut vec![F::ZERO; arity])
                        .is_err()
                );
                assert!(stripe.fiber(0, &mut []).is_err());
                Ok(())
            })
            .unwrap();
        assert!(seen.into_iter().all(|value| value));
    }
    let terminal = CoefficientReplayPlan::terminal(limits(1)).unwrap();
    assert_eq!(terminal.rows(), 128);
    assert_eq!(terminal.degree(), 2);
    assert_eq!(
        terminal.domain(),
        CoefficientReplayPlan::fri(4, limits(1))
            .unwrap()
            .domain()
            .folded(4)
    );
}

#[test]
fn coefficient_preflight_rejects_incomplete_noncanonical_and_unbounded_work() {
    let plan = CoefficientReplayPlan::quotient_and_mask(limits(2)).unwrap();
    assert_eq!(plan.stripe_bytes, 12_582_912);
    assert_eq!(plan.payload_bytes, 25_165_824);
    assert_eq!(plan.maximum_column_transforms, 3 * 4 * 64 * 2);
    assert!(
        CoefficientReplayPlan::quotient_and_mask(CoefficientLimits {
            max_payload_bytes: plan.payload_bytes - 1,
            ..limits(2)
        })
        .is_err()
    );
    assert!(
        CoefficientReplayPlan::quotient_and_mask(CoefficientLimits {
            max_work_units: plan.work_units - 1,
            ..limits(2)
        })
        .is_err()
    );
    assert!(CoefficientReplayPlan::fri(5, limits(1)).is_err());
    assert!(CoefficientReplayPlan::terminal(limits(0)).is_err());
    assert!(CoefficientReplayPlan::terminal(limits(usize::MAX)).is_err());
    assert!(CoefficientReplay::new(plan, &[&[]]).is_err());
    let over = vec![F::ZERO; plan.degree() + 1];
    assert!(CoefficientReplay::new(plan, &[&[], &[], &over]).is_err());
    for coordinate in 0..4 {
        let mut value = [0; 4];
        value[coordinate] = GOLDILOCKS_MODULUS;
        let bad = [F::from_coefficients_unchecked_for_test(value)];
        assert!(CoefficientReplay::new(plan, &[&[], &[], &bad]).is_err());
        assert!(fold_coefficients(4, &[F::ZERO; 8], bad[0], usize::MAX).is_err());
    }
    assert!(fold_coefficients(5, &[], F::ONE, usize::MAX).is_err());
    assert!(fold_coefficients(4, &[F::ZERO; 7], F::ONE, usize::MAX).is_err());
}

#[test]
fn selected_group_stripes_keep_all_fiber_coordinates_and_reject_bad_indices_early() {
    assert_eq!(super::super::deep_geometry::QUERY_COUNT, 77);
    let plan = CoefficientReplayPlan::with_shape(1024, 16, 1, 4, limits(1)).unwrap();
    let coefficients = (0..16).map(dense).collect::<Vec<_>>();
    let sources = [&coefficients[..]];
    let mut replay = CoefficientReplay::new(plan, &sources).unwrap();
    for invalid in [
        vec![],
        vec![0, 0],
        vec![3, 1],
        vec![256],
        (0..=2 * super::super::deep_geometry::QUERY_COUNT).collect(),
    ] {
        assert!(
            replay
                .visit_selected_stripes(&invalid, |_| panic!("invalid group visited"))
                .is_err()
        );
        assert!(replay.ensure_pass_available().is_ok());
    }
    let indices = [0, 1, 63, 64, 127, 128, 255];
    let mut seen = [false; 64];
    replay
        .visit_selected_stripes(&indices, |stripe| {
            let s = stripe.stripe_index();
            assert!(!seen[s]);
            seen[s] = true;
            for &index in &indices {
                if index % 64 == s {
                    let mut fiber = [F::ZERO; 4];
                    stripe.fiber(index / 64, &mut fiber)?;
                    for (position, &value) in fiber.iter().enumerate() {
                        assert_eq!(
                            value,
                            horner(&coefficients, plan.domain().point(index + position * 256))
                        );
                    }
                }
            }
            Ok(())
        })
        .unwrap();
    for (s, &visited) in seen.iter().enumerate() {
        assert_eq!(visited, indices.iter().any(|i| i % 64 == s));
    }
    assert!(replay.ensure_pass_available().is_err());
}

#[test]
fn selected_group_stripes_accept_the_exact_query_plus_sibling_boundary() {
    let plan = CoefficientReplayPlan::with_shape(1024, 16, 1, 4, limits(1)).unwrap();
    let coefficients = (0..16).map(dense).collect::<Vec<_>>();
    let sources = [&coefficients[..]];
    let mut replay = CoefficientReplay::new(plan, &sources).unwrap();
    let indices = (0..2 * super::super::deep_geometry::QUERY_COUNT).collect::<Vec<_>>();
    assert_eq!(indices.len(), 154);
    let mut checked = 0;
    replay
        .visit_selected_stripes(&indices, |stripe| {
            for &index in &indices {
                if index % 64 == stripe.stripe_index() {
                    let mut fiber = [F::ZERO; 4];
                    stripe.fiber(index / 64, &mut fiber)?;
                    for (position, &value) in fiber.iter().enumerate() {
                        assert_eq!(
                            value,
                            horner(&coefficients, plan.domain().point(index + position * 256))
                        );
                        checked += 1;
                    }
                }
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(checked, 154 * 4);
    assert!(replay.ensure_pass_available().is_err());
}
