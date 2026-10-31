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
fn fixed_prefix_deducts_actual_private_capacities_and_respects_exact_byte_boundary() {
    const NATIVE: usize = 8;
    const MASKED: usize = 37;
    let fixed = vec![vec![F(7); NATIVE]; 5];
    let per_fixed = NATIVE * core::mem::size_of::<F>() + core::mem::size_of::<PrivateTableV1<F>>();
    let exact = budget(2, MASKED) + 3 * per_fixed;
    for (allowance, expected) in [(exact - 1, 2), (exact, 3), (exact + 1, 3)] {
        let plan = MainQuotientCachePlanV1::from_budget_v1(2, 0, MASKED, 4, allowance).unwrap();
        assert_eq!(
            plan.fixed_prefix_columns_v1(NATIVE, fixed.len(), 4)
                .unwrap(),
            expected
        );
        let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
            Ok(replay(kind, range, MASKED))
        })
        .unwrap()
        .with_fixed_coefficients_v1(&fixed, NATIVE, 4)
        .unwrap();
        assert_eq!(cache.fixed_coefficients_v1().len(), expected);
        assert_eq!(
            cache.allocated_payload_v1().unwrap(),
            budget(2, MASKED) + expected * per_fixed
        );
        assert!(cache.allocated_payload_v1().unwrap() <= allowance);
        for column in cache.fixed_coefficients_v1() {
            assert_eq!(&**column, &fixed[0]);
        }
    }
    let plan = MainQuotientCachePlanV1::from_budget_v1(2, 0, MASKED, 4, exact).unwrap();
    let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |_, range| {
        Ok(range
            .map(|_| {
                let mut column = Vec::with_capacity(MASKED + 1);
                column.resize(MASKED, F::ONE);
                ZeroizingMainTraceColumnV1(column)
            })
            .collect())
    })
    .unwrap()
    .with_fixed_coefficients_v1(&fixed, NATIVE, 4)
    .unwrap();
    assert_eq!(
        cache.fixed_coefficients_v1().len(),
        2,
        "actual spare capacity is not spent twice"
    );
    let mut cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
        Ok(replay(kind, range, MASKED))
    })
    .unwrap();
    cache.payload_limit = cache.allocated_payload_v1().unwrap() - 1;
    assert!(cache.with_fixed_coefficients_v1(&fixed, NATIVE, 4).is_err());
    let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
        Ok(replay(kind, range, MASKED))
    })
    .unwrap()
    .with_fixed_coefficients_v1(&fixed, NATIVE, 1)
    .unwrap();
    assert!(cache.fixed_coefficients_v1().is_empty());
}

#[test]
fn fixed_prefix_refuses_malformed_fields_and_clears_both_owners_on_unwind() {
    for malformed in 0..4 {
        let plan = MainQuotientCachePlanV1::from_budget_v1(2, 0, 37, 4, budget(8, 37)).unwrap();
        let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
            Ok(replay(kind, range, 37))
        })
        .unwrap();
        let mut fixed = vec![vec![F(7); 8]; 3];
        let mut stripes = 4;
        match malformed {
            0 => fixed.clear(),
            1 => {
                fixed[1].pop();
            }
            2 => fixed[1][0] = F(u64::MAX),
            _ => stripes = 3,
        }
        assert!(
            cache
                .with_fixed_coefficients_v1(&fixed, 8, stripes)
                .is_err()
        );
    }
    let (result, erased) = inspection::observe_v1(|| {
        std::panic::catch_unwind(|| {
            let plan = MainQuotientCachePlanV1::from_budget_v1(2, 0, 37, 4, budget(8, 37)).unwrap();
            let _cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
                Ok(replay(kind, range, 37))
            })
            .unwrap()
            .with_fixed_coefficients_v1(&vec![vec![F(7); 8]; 3], 8, 4)
            .unwrap();
            panic!("fixed/private cache unwind");
        })
    });
    assert!(result.is_err());
    assert!(
        erased
            .iter()
            .any(|entry| entry.cells == 37 && entry.nonzero_before > 0)
    );
    assert!(
        erased
            .iter()
            .any(|entry| entry.cells == 8 && entry.nonzero_before > 0)
    );
    assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
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

fn retained_columns(
    kind: MainTraceColumnKindV1,
    width: usize,
    coefficients: usize,
) -> Vec<PrivateTableV1<F>> {
    replay(kind, 0..width, coefficients)
        .into_iter()
        .map(|column| PrivateTableV1::new(column.into_vec_v1(), zeroize_fields_v1))
        .collect()
}

#[test]
fn borrowed_registration_coefficients_preserve_every_coset_without_private_copies() {
    const COEFFICIENTS: usize = 37;
    let base = retained_columns(MainTraceColumnKindV1::Base, 10, COEFFICIENTS);
    let aux = retained_columns(MainTraceColumnKindV1::Aux, 7, COEFFICIENTS);
    let original_pointers = base
        .iter()
        .chain(&aux)
        .map(|column| column.as_ptr())
        .collect::<Vec<_>>();
    let plan =
        MainQuotientCachePlanV1::from_budget_v1(10, 7, COEFFICIENTS, 4, budget(9, COEFFICIENTS))
            .unwrap();
    let cache = MainQuotientReplayCacheV1::from_retained_v1(plan, &base, &aux).unwrap();
    assert!(cache.columns.is_empty());
    assert_eq!(cache.columns.capacity(), 0);
    assert_eq!(
        cache.allocated_payload_v1().unwrap(),
        core::mem::size_of::<MainQuotientReplayCacheV1>()
    );
    for ordinal in 0..4 {
        let full_root = goldilocks_primitive_root_v1(6).unwrap();
        let stripe = main_quotient_stripes::MainQuotientStripeV1 {
            rows: 16,
            count: 4,
            ordinal,
            next_stride: 4,
            root: full_root.pow(4),
            shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
        };
        for (kind, retained) in [
            (MainTraceColumnKindV1::Base, &base),
            (MainTraceColumnKindV1::Aux, &aux),
        ] {
            let values = cache
                .evaluate_retained_v1(
                    kind,
                    retained.len(),
                    stripe,
                    MainBoundedTransformPolicyV1::cpu_v1(),
                    retained,
                )
                .unwrap();
            let mut calls = Vec::new();
            let staged = cache
                .evaluate_sources_with_v1(
                    kind,
                    retained.len(),
                    stripe,
                    MainBoundedTransformPolicyV1::for_test_v1(stripe.rows, 8),
                    Some(retained),
                    |_| panic!("borrowed originals may not replay"),
                    |words, root, direction| {
                        calls.push(words.len());
                        transform_goldilocks_columns_v1(
                            words,
                            root,
                            direction,
                            fastpq_prover::ExecutionMode::Cpu,
                        )
                    },
                    || false,
                )
                .unwrap();
            assert_eq!(values.0, staged.0);
            assert!(calls.iter().all(|&width| width <= 8));
            assert_eq!(calls.iter().sum::<usize>(), retained.len());
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
    assert_eq!(
        original_pointers,
        base.iter()
            .chain(&aux)
            .map(|column| column.as_ptr())
            .collect::<Vec<_>>()
    );
    for (kind, columns) in [
        (MainTraceColumnKindV1::Base, &base),
        (MainTraceColumnKindV1::Aux, &aux),
    ] {
        for (column, values) in columns.iter().enumerate() {
            for (degree, value) in values.iter().enumerate() {
                assert_eq!(*value, coefficient(kind, column, degree));
            }
        }
    }
}

#[test]
fn borrowed_private_cache_reassigns_only_existing_bytes_to_fixed_prefix() {
    const COEFFICIENTS: usize = 37;
    let base = retained_columns(MainTraceColumnKindV1::Base, 10, COEFFICIENTS);
    let aux = retained_columns(MainTraceColumnKindV1::Aux, 7, COEFFICIENTS);
    let fixed = vec![vec![F(19); 8]; 40];
    let allowance = budget(9, COEFFICIENTS);
    let plan = MainQuotientCachePlanV1::from_budget_v1(10, 7, COEFFICIENTS, 4, allowance).unwrap();
    let original = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, columns| {
        Ok(replay(kind, columns, COEFFICIENTS))
    })
    .unwrap()
    .with_fixed_coefficients_v1(&fixed, 8, 4)
    .unwrap();
    let borrowed = MainQuotientReplayCacheV1::from_retained_v1(plan, &base, &aux)
        .unwrap()
        .with_fixed_coefficients_v1(&fixed, 8, 4)
        .unwrap();
    assert!(borrowed.columns.is_empty());
    assert_eq!(borrowed.payload_limit, original.payload_limit);
    assert!(borrowed.allocated_payload_v1().unwrap() <= allowance);
    assert!(borrowed.fixed_coefficients_v1().len() > original.fixed_coefficients_v1().len());
    let per_fixed = 8 * core::mem::size_of::<F>() + core::mem::size_of::<PrivateTableV1<F>>();
    assert_eq!(
        borrowed.fixed_coefficients_v1().len(),
        (allowance - core::mem::size_of::<MainQuotientReplayCacheV1>()) / per_fixed
    );
}

#[test]
fn borrowed_quotient_failure_clears_only_outputs_and_preserves_original_custody() {
    const COEFFICIENTS: usize = 37;
    let base = retained_columns(MainTraceColumnKindV1::Base, 9, COEFFICIENTS);
    let aux = retained_columns(MainTraceColumnKindV1::Aux, 1, COEFFICIENTS);
    let plan =
        MainQuotientCachePlanV1::from_budget_v1(9, 1, COEFFICIENTS, 4, budget(9, COEFFICIENTS))
            .unwrap();
    let cache = MainQuotientReplayCacheV1::from_retained_v1(plan, &base, &aux).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(2, 4, 0).unwrap();
    for failure in 0..3 {
        let (result, cleared) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut calls = 0;
                cache.evaluate_sources_with_v1(
                    MainTraceColumnKindV1::Base,
                    9,
                    stripe,
                    MainBoundedTransformPolicyV1::for_test_v1(16, 8),
                    Some(&base),
                    |_| panic!("borrowed originals may not replay"),
                    |words, root, direction| {
                        calls += 1;
                        if calls == 2 {
                            match failure {
                                0 => return Err(TransformError::DeviceUnavailable),
                                1 => return Err(TransformError::CompletionUncertain),
                                _ => panic!("borrowed forward failure"),
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
        assert_eq!(
            cleared.iter().map(|item| item.cells).sum::<usize>(),
            9 * (16 + 16)
        );
        assert!(cleared.iter().all(|item| item.nonzero_after == 0));
        for (column, values) in base.iter().enumerate() {
            for (degree, value) in values.iter().enumerate() {
                assert_eq!(
                    *value,
                    coefficient(MainTraceColumnKindV1::Base, column, degree)
                );
            }
        }
    }
    let (_, cleared) = inspection::observe_v1(|| {
        drop(base);
        drop(aux);
    });
    assert_eq!(
        cleared.iter().map(|item| item.cells).sum::<usize>(),
        10 * COEFFICIENTS
    );
    assert!(cleared.iter().all(|item| item.nonzero_after == 0));
}

#[test]
fn borrowed_quotient_rejects_wrong_shapes_local_copies_and_noncanonical_originals() {
    const COEFFICIENTS: usize = 37;
    let mut base = retained_columns(MainTraceColumnKindV1::Base, 2, COEFFICIENTS);
    let aux = retained_columns(MainTraceColumnKindV1::Aux, 1, COEFFICIENTS);
    let plan =
        MainQuotientCachePlanV1::from_budget_v1(2, 1, COEFFICIENTS, 4, budget(3, COEFFICIENTS))
            .unwrap();
    assert!(MainQuotientReplayCacheV1::from_retained_v1(plan, &[], &aux).is_err());
    assert!(MainQuotientReplayCacheV1::from_retained_v1(plan, &base[..1], &aux).is_err());
    let cache = MainQuotientReplayCacheV1::from_retained_v1(plan, &base, &aux).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(2, 4, 0).unwrap();
    assert!(
        cache
            .evaluate_retained_v1(
                MainTraceColumnKindV1::Base,
                1,
                stripe,
                MainBoundedTransformPolicyV1::cpu_v1(),
                &base
            )
            .is_err()
    );
    let owned = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, columns| {
        Ok(replay(kind, columns, COEFFICIENTS))
    })
    .unwrap();
    assert!(
        owned
            .evaluate_retained_v1(
                MainTraceColumnKindV1::Base,
                2,
                stripe,
                MainBoundedTransformPolicyV1::cpu_v1(),
                &base
            )
            .is_err()
    );
    base[1][0] = F(u64::MAX);
    assert!(
        cache
            .evaluate_retained_v1(
                MainTraceColumnKindV1::Base,
                2,
                stripe,
                MainBoundedTransformPolicyV1::cpu_v1(),
                &base
            )
            .is_err()
    );
    base[1].pop();
    assert!(MainQuotientReplayCacheV1::from_retained_v1(plan, &base, &aux).is_err());
    assert!(
        cache
            .evaluate_retained_v1(
                MainTraceColumnKindV1::Base,
                2,
                stripe,
                MainBoundedTransformPolicyV1::cpu_v1(),
                &base
            )
            .is_err()
    );
    assert!(matches!(
        cache.evaluate_sources_with_v1(
            MainTraceColumnKindV1::Base,
            2,
            stripe,
            MainBoundedTransformPolicyV1::cpu_v1(),
            Some(&base),
            |_| panic!("uncertain source"),
            |_, _, _| panic!("uncertain transform"),
            || true
        ),
        Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
    ));
}

#[test]
fn prepared_forward_grouping_preserves_full_mask_tails_source_order_and_every_output() {
    const ROWS: usize = 8;
    const COEFFICIENTS: usize = ROWS + MASK_DEGREE + 1;
    let full_root = goldilocks_primitive_root_v1(5).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1 {
        rows: ROWS,
        count: 4,
        ordinal: 3,
        next_stride: 1,
        root: full_root.pow(4),
        shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(3)),
    };
    let kind = MainTraceColumnKindV1::Base;
    for width in [1usize, 8, 15, 16, 17, 33] {
        let expected = (0..width)
            .map(|column| {
                (0..ROWS)
                    .map(|row| {
                        let x = stripe.shift.mul(stripe.root.pow(row as u128));
                        (0..COEFFICIENTS).rev().fold(F::ZERO, |sum, degree| {
                            sum.mul(x).add(coefficient(kind, column, degree))
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut prefixes = vec![0, 1, 7, 8, 9, 15, 16, width];
        prefixes.retain(|&prefix| prefix <= width);
        prefixes.sort_unstable();
        prefixes.dedup();
        for cached in prefixes {
            let plan = MainQuotientCachePlanV1::from_budget_v1(
                width,
                0,
                COEFFICIENTS,
                4,
                budget(cached, COEFFICIENTS),
            )
            .unwrap();
            let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
                Ok(replay(kind, range, COEFFICIENTS))
            })
            .unwrap();
            for group in [8, 16] {
                let mut source_ranges = Vec::new();
                let mut dispatches = Vec::new();
                let values = cache
                    .evaluate_sources_using_v1(
                        kind,
                        width,
                        stripe,
                        None,
                        |range| {
                            source_ranges.push(range.clone());
                            Ok(replay(kind, range, COEFFICIENTS))
                        },
                        group,
                        |columns, root, uncertain| {
                            dispatches.push(columns.len());
                            MainBoundedTransformPolicyV1::cpu_v1()
                                .forward_exact_v1(columns, root, uncertain)
                        },
                        || false,
                    )
                    .unwrap();
                assert_eq!(values.0, expected);
                assert_eq!(
                    source_ranges,
                    (cached..width)
                        .step_by(8)
                        .map(|first| first..width.min(first + 8))
                        .collect::<Vec<_>>()
                );
                assert!(dispatches.iter().all(|&count| count > 0 && count <= group));
                assert_eq!(dispatches.iter().sum::<usize>(), width);
                if group == 16 {
                    assert_eq!(dispatches.len(), width.div_ceil(group));
                } else {
                    assert_eq!(
                        dispatches.len(),
                        cached.div_ceil(8) + (width - cached).div_ceil(8)
                    );
                }
                assert!(values.iter().all(|column| column.capacity() == ROWS));
                assert_eq!(values.0.capacity(), width);
            }
        }
    }
    // Selected worker counts exercise the same independently ordered folding.
    for workers in [1, 4, 20] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let plan = MainQuotientCachePlanV1::from_budget_v1(
                    17,
                    0,
                    COEFFICIENTS,
                    4,
                    budget(7, COEFFICIENTS),
                )
                .unwrap();
                let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
                    Ok(replay(kind, range, COEFFICIENTS))
                })
                .unwrap();
                let run = |group| {
                    cache
                        .evaluate_sources_using_v1(
                            kind,
                            17,
                            stripe,
                            None,
                            |range| Ok(replay(kind, range, COEFFICIENTS)),
                            group,
                            |columns, root, uncertain| {
                                MainBoundedTransformPolicyV1::cpu_v1()
                                    .forward_exact_v1(columns, root, uncertain)
                            },
                            || false,
                        )
                        .unwrap()
                };
                assert_eq!(run(8).0, run(16).0);
            });
    }
}

#[test]
fn prepared_forward_grouping_preserves_source_refusal_order_quarantine_and_full_rollback() {
    use core::cell::Cell;
    const ROWS: usize = 8;
    const COEFFICIENTS: usize = ROWS + MASK_DEGREE + 1;
    let plan =
        MainQuotientCachePlanV1::from_budget_v1(33, 0, COEFFICIENTS, 4, budget(0, COEFFICIENTS))
            .unwrap();
    let cache =
        MainQuotientReplayCacheV1::from_replay_v1(plan, |_, _| panic!("empty prefix")).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(2, 3, 0).unwrap();
    for group in [8, 16] {
        for quarantine in [false, true] {
            let latched = Cell::new(false);
            let mut dispatched = 0;
            let (result, erased) = inspection::observe_v1(|| {
                cache.evaluate_sources_using_v1(
                    MainTraceColumnKindV1::Base,
                    16,
                    stripe,
                    None,
                    |range| {
                        if range.start == 8 {
                            latched.set(quarantine);
                            return Err(ZkX509StarkErrorV1::ProofTooLarge);
                        }
                        Ok(replay(MainTraceColumnKindV1::Base, range, COEFFICIENTS))
                    },
                    group,
                    |_, _, _| {
                        dispatched += 1;
                        Err(ZkX509StarkErrorV1::ProfileMismatch)
                    },
                    || latched.get(),
                )
            });
            assert_eq!(dispatched, usize::from(group == 8));
            let expected = if group == 8 {
                ZkX509StarkErrorV1::ProfileMismatch
            } else if quarantine {
                ZkX509StarkErrorV1::AcceleratorCompletionUncertain
            } else {
                ZkX509StarkErrorV1::ProofTooLarge
            };
            assert!(matches!(result, Err(error) if error == expected));
            assert_eq!(
                erased.iter().map(|item| item.cells).sum::<usize>(),
                8 * (COEFFICIENTS + ROWS)
            );
            assert!(erased.iter().all(|item| item.nonzero_after == 0));
        }
    }
    // A later replay or device failure/unwind clears both published and pending
    // final columns; no completed prefix or original replay allocation escapes.
    for failure in 0..4 {
        let mut dispatches = 0;
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                cache.evaluate_sources_using_v1(
                    MainTraceColumnKindV1::Base,
                    33,
                    stripe,
                    None,
                    |range| {
                        if failure < 2 && range.start == 24 {
                            if failure == 0 {
                                return Err(ZkX509StarkErrorV1::ProofTooLarge);
                            }
                            std::panic::resume_unwind(Box::new(()));
                        }
                        Ok(replay(MainTraceColumnKindV1::Base, range, COEFFICIENTS))
                    },
                    16,
                    |columns, root, uncertain| {
                        dispatches += 1;
                        if failure >= 2 && dispatches == 2 {
                            if failure == 2 {
                                return Err(ZkX509StarkErrorV1::ProfileMismatch);
                            }
                            std::panic::resume_unwind(Box::new(()));
                        }
                        MainBoundedTransformPolicyV1::cpu_v1()
                            .forward_exact_v1(columns, root, uncertain)
                    },
                    || false,
                )
            }))
        });
        if failure % 2 == 0 {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        assert_eq!(dispatches, if failure < 2 { 1 } else { 2 });
        let initialized = if failure < 2 { 24 } else { 32 };
        assert_eq!(
            erased.iter().map(|item| item.cells).sum::<usize>(),
            initialized * (COEFFICIENTS + ROWS)
        );
        assert!(erased.iter().all(|item| item.nonzero_after == 0));
    }
    for group in [0, 1, 7, 9, 17] {
        assert!(
            cache
                .evaluate_sources_using_v1(
                    MainTraceColumnKindV1::Base,
                    1,
                    stripe,
                    None,
                    |_| panic!("bad public group before source"),
                    group,
                    |_, _, _| panic!("bad public group before dispatch"),
                    || false
                )
                .is_err()
        );
    }
    let mut output = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1::<Vec<F>>);
    assert_eq!(
        append_folded_batch_v1(&mut output, &[], stripe),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    );
    assert_eq!(
        append_folded_batch_v1(&mut output, &[&[F::ONE]], stripe),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    );
}

#[test]
fn fixed_owner_reservation_keeps_private_columns_and_charges_actual_fixed_prefix() {
    const ROWS: usize = 8;
    let per_column = ROWS * core::mem::size_of::<F>() + core::mem::size_of::<PrivateTableV1<F>>();
    let private = 5 * per_column + core::mem::size_of::<MainQuotientReplayCacheV1>();
    let original = MainQuotientCachePlanV1 {
        base_columns: 2,
        aux_columns: 3,
        coefficient_count: ROWS,
        payload_limit: private + 2 * per_column,
    };
    let narrowed = original.reserve_fixed_owner_v1(per_column).unwrap();
    assert_eq!((narrowed.base_columns, narrowed.aux_columns), (2, 3));
    assert_eq!(narrowed.payload_limit + per_column, original.payload_limit);
    assert_eq!(original.fixed_prefix_columns_v1(ROWS, 10, 4).unwrap(), 2);
    assert_eq!(narrowed.fixed_prefix_columns_v1(ROWS, 10, 4).unwrap(), 1);
    let fixed = vec![vec![F(7); ROWS]; 10];
    let cache = MainQuotientReplayCacheV1::from_replay_v1(narrowed, |_, columns| {
        Ok(columns
            .map(|_| ZeroizingMainTraceColumnV1(vec![F(3); ROWS]))
            .collect())
    })
    .unwrap()
    .with_fixed_coefficients_v1(&fixed, ROWS, 4)
    .unwrap();
    assert_eq!(cache.fixed_coefficients_v1().len(), 1);
    assert_eq!(
        cache.allocated_payload_v1().unwrap() + per_column,
        original.payload_limit
    );
    let exact = MainQuotientCachePlanV1 {
        payload_limit: private + per_column,
        ..original
    };
    assert_eq!(
        exact
            .reserve_fixed_owner_v1(per_column)
            .unwrap()
            .fixed_prefix_columns_v1(ROWS, 10, 4)
            .unwrap(),
        0
    );
    let insufficient = MainQuotientCachePlanV1 {
        payload_limit: private + per_column - 1,
        ..original
    };
    assert!(matches!(
        insufficient.reserve_fixed_owner_v1(per_column),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert!(matches!(
        original.reserve_fixed_owner_v1(usize::MAX),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
}
