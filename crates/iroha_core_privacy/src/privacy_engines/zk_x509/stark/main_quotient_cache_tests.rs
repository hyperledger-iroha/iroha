//! Independent polynomial and live-allocation controls for quotient reuse.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection;

fn coefficient(kind: MainTraceColumnKindV1, column: usize, degree: usize) -> F {
    let offset = match kind {
        MainTraceColumnKindV1::Base => 1,
        MainTraceColumnKindV1::Aux => 101,
    };
    F::reduce((offset + 17 * column + degree * degree) as u128)
}

fn replay(
    kind: MainTraceColumnKindV1,
    columns: core::ops::Range<usize>,
    rows: usize,
) -> Vec<ZeroizingMainTraceColumnV1> {
    columns
        .map(|column| {
            ZeroizingMainTraceColumnV1(
                (0..rows)
                    .map(|degree| coefficient(kind, column, degree))
                    .collect(),
            )
        })
        .collect()
}

fn budget(columns: usize, coefficients: usize) -> usize {
    core::mem::size_of::<MainQuotientReplayCacheV1>()
        + columns
            * (coefficients * core::mem::size_of::<F>() + core::mem::size_of::<PrivateTableV1<F>>())
}

#[test]
fn bounded_cache_preserves_every_column_and_interleaved_coset_value() {
    const COEFFICIENTS: usize = 37;
    let root = goldilocks_primitive_root_v1(6).unwrap();
    for cached in [0, 1, 9, 10, 12, 17] {
        let plan = MainQuotientCachePlanV1::from_budget_v1(
            10,
            7,
            COEFFICIENTS,
            4,
            budget(cached, COEFFICIENTS),
        )
        .unwrap();
        assert_eq!(plan.base_columns + plan.aux_columns, cached);
        let mut source_columns = 0;
        let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, columns| {
            assert!(columns.len() <= 8);
            source_columns += columns.len();
            Ok(replay(kind, columns, COEFFICIENTS))
        })
        .unwrap();
        for ordinal in 0..4 {
            let stripe = main_quotient_stripes::MainQuotientStripeV1 {
                rows: 16,
                count: 4,
                ordinal,
                next_stride: 2,
                root: root.pow(4),
                shift: F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(ordinal as u128)),
            };
            for (kind, width) in [
                (MainTraceColumnKindV1::Base, 10),
                (MainTraceColumnKindV1::Aux, 7),
            ] {
                let values = cache
                    .evaluate_v1(
                        kind,
                        width,
                        stripe,
                        MainBoundedTransformPolicyV1::cpu_v1(),
                        |columns| {
                            assert!(columns.len() <= 8);
                            source_columns += columns.len();
                            Ok(replay(kind, columns, COEFFICIENTS))
                        },
                    )
                    .unwrap();
                assert_eq!(values.len(), width);
                for column in 0..width {
                    for row in 0..stripe.rows {
                        let x =
                            F(GOLDILOCKS_GENERATOR_V1).mul(root.pow((ordinal + 4 * row) as u128));
                        let expected = (0..COEFFICIENTS).rev().fold(F::ZERO, |sum, degree| {
                            sum.mul(x).add(coefficient(kind, column, degree))
                        });
                        assert_eq!(values[column][row], expected);
                    }
                }
            }
        }
        assert_eq!(source_columns, cached + 4 * (17 - cached));
    }
}

#[test]
fn cache_admission_uses_public_capacity_and_rejects_overflow_or_bad_replay() {
    for (base, aux, coefficients, stripes, bytes) in [
        (1, 0, 0, 4, 1000),
        (1, 0, 8, 0, 1000),
        (usize::MAX, 1, 8, 4, 1000),
        (1, 0, usize::MAX, 4, 1000),
        (1, 0, 8, 4, 0),
    ] {
        assert!(
            MainQuotientCachePlanV1::from_budget_v1(base, aux, coefficients, stripes, bytes)
                .is_err()
        );
    }
    let one_pass = MainQuotientCachePlanV1::from_budget_v1(10, 7, 8, 1, budget(17, 8)).unwrap();
    assert_eq!(one_pass.base_columns + one_pass.aux_columns, 0);
    MainQuotientReplayCacheV1::from_replay_v1(one_pass, |_, _| {
        panic!("single pass must not replay early")
    })
    .unwrap();
    let plan = MainQuotientCachePlanV1::from_budget_v1(2, 0, 8, 4, budget(2, 8)).unwrap();
    assert!(
        MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| Ok(replay(kind, range, 7)))
            .is_err()
    );
    assert!(MainQuotientReplayCacheV1::from_replay_v1(plan, |_, _| Ok(Vec::new())).is_err());
    // Initialized length alone is insufficient: retained allocation capacity is charged.
    assert!(
        MainQuotientReplayCacheV1::from_replay_v1(plan, |_, range| {
            Ok(range
                .map(|_| {
                    let mut values = Vec::with_capacity(100);
                    values.resize(8, F::ONE);
                    ZeroizingMainTraceColumnV1(values)
                })
                .collect())
        })
        .is_err()
    );
}

#[test]
fn cached_private_cells_clear_after_source_error_unwind_and_success() {
    for failure in [0, 1, 2] {
        let plan = MainQuotientCachePlanV1::from_budget_v1(9, 0, 8, 4, budget(9, 8)).unwrap();
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
                    if range.start == 8 && failure != 0 {
                        assert!(failure != 2, "injected cache replay unwind");
                        return Err(ZkX509StarkErrorV1::InternalInvariant);
                    }
                    Ok(replay(kind, range, 8))
                })?;
                drop(cache);
                Ok::<_, ZkX509StarkErrorV1>(())
            }))
        });
        match failure {
            0 => assert!(result.unwrap().is_ok()),
            1 => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
        let cells = if failure == 0 { 72 } else { 64 };
        assert_eq!(
            observations.iter().map(|item| item.cells).sum::<usize>(),
            cells
        );
        assert_eq!(
            observations
                .iter()
                .map(|item| item.nonzero_before)
                .sum::<usize>(),
            cells
        );
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn cached_and_replayed_private_batches_use_one_forward_adapter_and_full_coefficients() {
    const COEFFICIENTS: usize = 113;
    let plan =
        MainQuotientCachePlanV1::from_budget_v1(10, 7, COEFFICIENTS, 4, budget(3, COEFFICIENTS))
            .unwrap();
    let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
        Ok(replay(kind, range, COEFFICIENTS))
    })
    .unwrap();
    let root = goldilocks_primitive_root_v1(7).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1 {
        rows: 32,
        count: 4,
        ordinal: 3,
        next_stride: 2,
        root: root.pow(4),
        shift: F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(3)),
    };
    for kind in [MainTraceColumnKindV1::Base, MainTraceColumnKindV1::Aux] {
        let width = match kind {
            MainTraceColumnKindV1::Base => 10,
            MainTraceColumnKindV1::Aux => 7,
        };
        let mut transformed = 0;
        let values = cache
            .evaluate_with_v1(
                kind,
                width,
                stripe,
                MainBoundedTransformPolicyV1::for_test_v1(32, 2),
                |range| Ok(replay(kind, range, COEFFICIENTS)),
                |words, root, direction| {
                    assert_eq!(direction, Direction::Forward);
                    assert!(words.len() <= 2);
                    transformed += words.len();
                    transform_goldilocks_columns_v1(
                        words,
                        root,
                        direction,
                        fastpq_prover::ExecutionMode::Cpu,
                    )?;
                    Ok(Backend::Metal) // Injected receipt, not hardware qualification.
                },
                || false,
            )
            .unwrap();
        assert_eq!(transformed, width);
        for (column, values) in values.iter().enumerate() {
            for (row, value) in values.iter().enumerate() {
                let x = stripe.shift.mul(stripe.root.pow(row as u128));
                let expected = (0..COEFFICIENTS).rev().fold(F::ZERO, |sum, degree| {
                    sum.mul(x).add(coefficient(kind, column, degree))
                });
                assert_eq!(*value, expected);
            }
        }
    }
}

#[test]
fn private_cache_rejects_excess_replay_capacity_before_device_staging() {
    let plan = MainQuotientCachePlanV1::from_budget_v1(2, 0, 37, 1, budget(2, 37)).unwrap();
    let cache =
        MainQuotientReplayCacheV1::from_replay_v1(plan, |_, _| panic!("uncached plan")).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(2, 4, 0).unwrap();
    for failure in 0..4 {
        let (result, erased) = inspection::observe_v1(|| {
            cache.evaluate_with_v1(
                MainTraceColumnKindV1::Base,
                2,
                stripe,
                MainBoundedTransformPolicyV1::for_test_v1(16, 2),
                |range| {
                    let mut batch = replay(MainTraceColumnKindV1::Base, range, 37);
                    match failure {
                        0 => {
                            batch[0].0.reserve_exact(37);
                        }
                        1 => {
                            batch.reserve_exact(2);
                        }
                        2 => {
                            let _ = batch[0].0.pop();
                        }
                        _ => {
                            let _ = batch.pop();
                        }
                    }
                    Ok(batch)
                },
                |_, _, _| panic!("excess replay may not dispatch"),
                || false,
            )
        });
        assert!(result.is_err());
        assert!(erased.iter().any(|item| item.cells > 0));
        assert!(erased.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn private_cache_clears_output_and_replay_after_failed_or_unwound_forward_batch() {
    for failure in [0, 1, 2] {
        let plan = MainQuotientCachePlanV1::from_budget_v1(9, 0, 37, 1, budget(9, 37)).unwrap();
        let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |_, _| panic!("uncached plan"))
            .unwrap();
        let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(2, 4, 0).unwrap();
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut calls = 0;
                cache.evaluate_with_v1(
                    MainTraceColumnKindV1::Base,
                    9,
                    stripe,
                    MainBoundedTransformPolicyV1::for_test_v1(16, 8),
                    |range| Ok(replay(MainTraceColumnKindV1::Base, range, 37)),
                    |words, root, direction| {
                        calls += 1;
                        if calls == 2 {
                            match failure {
                                0 => return Err(TransformError::DeviceUnavailable),
                                1 => return Err(TransformError::CompletionUncertain),
                                _ => panic!("injected late private FFT unwind"),
                            }
                        }
                        transform_goldilocks_columns_v1(
                            words,
                            root,
                            direction,
                            fastpq_prover::ExecutionMode::Cpu,
                        )
                    },
                    || false,
                )
            }))
        });
        if failure == 2 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        // Source coefficients, both field-output batches, and word staging all
        // clear despite a completed earlier batch. No partial matrix returns.
        assert_eq!(
            erased.iter().map(|item| item.cells).sum::<usize>(),
            9 * (37 + 16 + 16)
        );
        assert!(erased.iter().all(|item| item.nonzero_after == 0));
    }
    let plan = MainQuotientCachePlanV1::from_budget_v1(1, 0, 37, 1, budget(1, 37)).unwrap();
    let cache =
        MainQuotientReplayCacheV1::from_replay_v1(plan, |_, _| panic!("uncached plan")).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(2, 4, 0).unwrap();
    assert!(matches!(
        cache.evaluate_with_v1(
            MainTraceColumnKindV1::Base,
            1,
            stripe,
            MainBoundedTransformPolicyV1::cpu_v1(),
            |_| panic!("uncertain completion must precede private source replay"),
            |_, _, _| panic!("uncertain dispatch"),
            || true,
        ),
        Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
    ));
}

#[test]
fn added_joint_owners_reduce_real_cache_without_changing_source_coordinates() {
    const COEFFICIENTS: usize = 37;
    let original =
        MainQuotientCachePlanV1::from_budget_v1(10, 7, COEFFICIENTS, 4, budget(17, COEFFICIENTS))
            .unwrap();
    let per_column = budget(1, COEFFICIENTS) - budget(0, COEFFICIENTS);
    for retained in [0, 1, 9, 10, 12, 17] {
        let reserved = (17 - retained) * per_column;
        let narrowed = original.reserve_additional_v1(reserved).unwrap();
        assert_eq!(narrowed.base_columns, retained.min(10));
        assert_eq!(narrowed.aux_columns, retained.saturating_sub(10));
        assert_eq!(narrowed.payload_limit + reserved, original.payload_limit);
        let mut generated = Vec::new();
        let cache = MainQuotientReplayCacheV1::from_replay_v1(narrowed, |kind, range| {
            for column in range.clone() {
                generated.push((kind, column));
            }
            Ok(replay(kind, range, COEFFICIENTS))
        })
        .unwrap();
        assert_eq!(generated.len(), retained);
        for (ordinal, (kind, column)) in generated.iter().enumerate() {
            let expected_kind = if ordinal < 10 {
                MainTraceColumnKindV1::Base
            } else {
                MainTraceColumnKindV1::Aux
            };
            assert_eq!(
                core::mem::discriminant(kind),
                core::mem::discriminant(&expected_kind)
            );
            assert_eq!(*column, if ordinal < 10 { ordinal } else { ordinal - 10 });
            for degree in [0, 7, COEFFICIENTS - 1] {
                assert_eq!(
                    cache.columns[ordinal][degree],
                    coefficient(expected_kind, *column, degree)
                );
            }
        }
        assert!(
            cache.columns.capacity() * core::mem::size_of::<PrivateTableV1<F>>()
                + cache
                    .columns
                    .iter()
                    .map(|c| c.capacity() * 8)
                    .sum::<usize>()
                + core::mem::size_of::<MainQuotientReplayCacheV1>()
                <= narrowed.payload_limit
        );
    }
    for reserved in [17 * per_column + 1, original.payload_limit, usize::MAX] {
        assert!(original.reserve_additional_v1(reserved).is_err());
    }
    let one_pass =
        MainQuotientCachePlanV1::from_budget_v1(10, 7, COEFFICIENTS, 1, budget(17, COEFFICIENTS))
            .unwrap()
            .reserve_additional_v1(per_column)
            .unwrap();
    assert_eq!((one_pass.base_columns, one_pass.aux_columns), (0, 0));
    MainQuotientReplayCacheV1::from_replay_v1(one_pass, |_, _| panic!("no single-stripe cache"))
        .unwrap();
}

#[test]
fn arithmetic_priority_preserves_capacity_and_every_original_column() {
    const COEFFICIENTS: usize = 37;
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let registration = *layout
        .registered_segments
        .iter()
        .find(|item| item.segment.adapter == SegmentAdapterIdV1::P256Arithmetic)
        .unwrap();
    assert_eq!(
        (
            registration.segment.base_width,
            registration.segment.aux_width
        ),
        (211, 72)
    );
    for retained in [0, 1, 8, 16, 18, 56, 64, 72, 73, 267, 277, 283] {
        let original = MainQuotientCachePlanV1::from_budget_v1(
            211,
            72,
            COEFFICIENTS,
            4,
            budget(retained, COEFFICIENTS),
        )
        .unwrap();
        let priority = original.prioritize_registration_v1(registration).unwrap();
        assert_eq!(priority.base_columns, retained.saturating_sub(72));
        assert_eq!(priority.aux_columns, retained.min(72));
        assert_eq!(priority.payload_limit, original.payload_limit);
        assert_eq!(priority.coefficient_count, original.coefficient_count);
        assert_eq!(priority.base_columns + priority.aux_columns, retained);
    }
    // The extra CA owners must reduce the count, then reapply the same public
    // priority. Neither an old base-first prefix nor two owners spend this slack.
    let ordinary = MainQuotientCachePlanV1::from_budget_v1(
        211,
        72,
        COEFFICIENTS,
        4,
        budget(277, COEFFICIENTS),
    )
    .unwrap()
    .prioritize_registration_v1(registration)
    .unwrap();
    let per_column = budget(1, COEFFICIENTS) - budget(0, COEFFICIENTS);
    for retained in 0..=277 {
        let reserved = (277 - retained) * per_column;
        let narrowed = ordinary
            .reserve_additional_v1(reserved)
            .unwrap()
            .prioritize_registration_v1(registration)
            .unwrap();
        assert_eq!(narrowed.base_columns, retained.saturating_sub(72));
        assert_eq!(narrowed.aux_columns, retained.min(72));
        assert_eq!(narrowed.payload_limit + reserved, ordinary.payload_limit);
    }
    let plan = ordinary
        .reserve_additional_v1(10 * per_column)
        .unwrap()
        .prioritize_registration_v1(registration)
        .unwrap();
    assert_eq!((ordinary.base_columns, ordinary.aux_columns), (205, 72));
    assert_eq!((plan.base_columns, plan.aux_columns), (195, 72));
    assert_eq!(plan.payload_limit + 10 * per_column, ordinary.payload_limit);
    let mut generated = 0;
    let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
        generated += range.len();
        assert!(range.len() <= 8);
        Ok(replay(kind, range, COEFFICIENTS))
    })
    .unwrap();
    let root = goldilocks_primitive_root_v1(6).unwrap();
    for ordinal in 0..4 {
        let stripe = main_quotient_stripes::MainQuotientStripeV1 {
            rows: 16,
            count: 4,
            ordinal,
            next_stride: 2,
            root: root.pow(4),
            shift: F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(ordinal as u128)),
        };
        for (kind, width) in [
            (MainTraceColumnKindV1::Base, 211),
            (MainTraceColumnKindV1::Aux, 72),
        ] {
            let values = cache
                .evaluate_v1(
                    kind,
                    width,
                    stripe,
                    MainBoundedTransformPolicyV1::cpu_v1(),
                    |range| {
                        assert!(matches!(kind, MainTraceColumnKindV1::Base));
                        assert!(range.start >= 195 && range.end <= 211 && range.len() <= 8);
                        generated += range.len();
                        Ok(replay(kind, range, COEFFICIENTS))
                    },
                )
                .unwrap();
            for (column, values) in values.iter().enumerate() {
                for (row, value) in values.iter().enumerate() {
                    let x = stripe.shift.mul(stripe.root.pow(row as u128));
                    let expected = (0..COEFFICIENTS).rev().fold(F::ZERO, |sum, degree| {
                        sum.mul(x).add(coefficient(kind, column, degree))
                    });
                    assert_eq!(*value, expected);
                }
            }
        }
    }
    assert_eq!(generated, 267 + 4 * 16);
    for item in &layout.registered_segments {
        if item.segment.adapter != SegmentAdapterIdV1::P256Arithmetic {
            let old = MainQuotientCachePlanV1::from_budget_v1(
                item.segment.base_width,
                item.segment.aux_width,
                COEFFICIENTS,
                4,
                budget(18, COEFFICIENTS),
            )
            .unwrap();
            let unchanged = old.prioritize_registration_v1(*item).unwrap();
            assert_eq!(
                (unchanged.base_columns, unchanged.aux_columns),
                (old.base_columns, old.aux_columns)
            );
            assert_eq!(unchanged.payload_limit, old.payload_limit);
        }
    }
    let single = MainQuotientCachePlanV1::from_budget_v1(
        211,
        72,
        COEFFICIENTS,
        1,
        budget(283, COEFFICIENTS),
    )
    .unwrap()
    .prioritize_registration_v1(registration)
    .unwrap();
    assert_eq!((single.base_columns, single.aux_columns), (0, 0));
    let mut foreign = plan;
    foreign.aux_columns = 73;
    assert!(foreign.prioritize_registration_v1(registration).is_err());
    foreign = plan;
    foreign.base_columns = 212;
    assert!(foreign.prioritize_registration_v1(registration).is_err());
}

#[test]
fn arithmetic_priority_clears_auxiliary_cache_on_success_error_and_unwind() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let registration = *layout
        .registered_segments
        .iter()
        .find(|item| item.segment.adapter == SegmentAdapterIdV1::P256Arithmetic)
        .unwrap();
    for failure in [0, 1, 2] {
        let plan = MainQuotientCachePlanV1::from_budget_v1(211, 72, 8, 4, budget(9, 8))
            .unwrap()
            .prioritize_registration_v1(registration)
            .unwrap();
        assert_eq!((plan.base_columns, plan.aux_columns), (0, 9));
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
                    assert!(matches!(kind, MainTraceColumnKindV1::Aux));
                    if range.start == 8 && failure != 0 {
                        assert!(failure != 2, "injected prioritized auxiliary unwind");
                        return Err(ZkX509StarkErrorV1::InternalInvariant);
                    }
                    Ok(replay(kind, range, 8))
                })?;
                drop(cache);
                Ok::<_, ZkX509StarkErrorV1>(())
            }))
        });
        match failure {
            0 => assert!(result.unwrap().is_ok()),
            1 => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
        let cells = if failure == 0 { 72 } else { 64 };
        assert_eq!(erased.iter().map(|item| item.cells).sum::<usize>(), cells);
        assert_eq!(
            erased.iter().map(|item| item.nonzero_before).sum::<usize>(),
            cells
        );
        assert!(erased.iter().all(|item| item.nonzero_after == 0));
    }
}
