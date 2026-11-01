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

// This all-cached CPU experiment lives only in the existing cfg(test) child.
// Actual evaluate_with_v1 remains the primary timed baseline and parity oracle.
impl MainQuotientReplayCacheV1 {
    #[allow(clippy::too_many_arguments)]
    fn diagnostic_evaluate_cached_v1(
        &self,
        kind: MainTraceColumnKindV1,
        width: usize,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
        policy: MainBoundedTransformPolicyV1,
        mut uncertain: impl FnMut() -> bool,
        mut after_batch: impl FnMut(usize) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(ZeroizingBaseColumnsV1, usize, usize, usize), ZkX509StarkErrorV1> {
        check_completion_v1(uncertain())?;
        let cached = match kind {
            MainTraceColumnKindV1::Base => &self.columns[..self.base_columns],
            MainTraceColumnKindV1::Aux => &self.columns[self.base_columns..],
        };
        if width == 0 || cached.len() != width || policy.columns_v1(stripe.rows) != 0 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut output = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1::<Vec<F>>);
        output
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if output.capacity() != width {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let mut retained = None;
        let mut constructors = 0;
        let mut peak_public_payload = 0;
        let mut batches = 0;
        for originals in cached.chunks(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            check_completion_v1(uncertain())?;
            let start = output.len();
            for _ in originals {
                let mut column = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
                column
                    .try_reserve_exact(stripe.rows)
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
                if column.capacity() != stripe.rows {
                    return Err(ZkX509StarkErrorV1::ProofTooLarge);
                }
                column.resize(stripe.rows, F::ZERO);
                output.push(column.into_vec());
            }
            let batch = &mut output[start..];
            let mut coefficients: [&[F]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1] =
                [&[]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1];
            for (target, source) in coefficients.iter_mut().zip(originals) {
                *target = source;
            }
            batch
                .par_iter_mut()
                .zip(coefficients[..originals.len()].par_iter())
                .try_for_each(|(values, coefficients)| stripe.fold_into_v1(coefficients, values))?;
            policy.diagnostic_reused_forward_v1(
                batch,
                stripe.root,
                &mut retained,
                &mut constructors,
                &mut peak_public_payload,
                &mut uncertain,
            )?;
            batches += 1;
            after_batch(batches)?;
        }
        check_completion_v1(uncertain())?;
        // This public table drops before the next kind/stripe or any inverse.
        drop(retained);
        Ok((
            ZeroizingBaseColumnsV1(output.into_vec()),
            constructors,
            peak_public_payload,
            batches,
        ))
    }
}

#[test]
fn diagnostic_cached_reuse_matches_original_horner_and_clearing() {
    for rows in [32_usize, 256] {
        let root = goldilocks_primitive_root_v1(u8::try_from(rows.ilog2()).unwrap()).unwrap();
        let stripe = main_quotient_stripes::MainQuotientStripeV1 {
            rows,
            count: 4,
            ordinal: 1,
            next_stride: 2,
            root,
            shift: F(7),
        };
        let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
        let policy = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 0)
            .unwrap()
            .diagnostic_cpu_policy_v1();
        let plan =
            MainQuotientCachePlanV1::from_budget_v1(17, 0, rows + 5, 4, budget(17, rows + 5))
                .unwrap();
        for failure in 0..4 {
            let (result, erased) = inspection::observe_v1(|| {
                std::panic::catch_unwind(|| {
                    let mut cache =
                        MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
                            Ok(replay(kind, range, rows + 5))
                        })
                        .unwrap();
                    if failure == 3 {
                        cache.columns[0][0] = F(u64::MAX);
                    }
                    let actual = cache.diagnostic_evaluate_cached_v1(
                        MainTraceColumnKindV1::Base,
                        17,
                        stripe,
                        policy,
                        || false,
                        |_| {
                            if failure == 1 {
                                return Err(ZkX509StarkErrorV1::AllocationFailure);
                            }
                            assert_ne!(failure, 2, "injected all-cached owner unwind");
                            Ok(())
                        },
                    );
                    if failure != 0 {
                        assert!(actual.is_err());
                        return;
                    }
                    let (actual, constructors, _, batches) = actual.unwrap();
                    assert_eq!(constructors, usize::from(rows >= 256));
                    assert_eq!(batches, 3);
                    let reference = cache
                        .evaluate_with_v1(
                            MainTraceColumnKindV1::Base,
                            17,
                            stripe,
                            policy,
                            |_| panic!("all cached"),
                            |_, _, _| panic!("CPU"),
                            || false,
                        )
                        .unwrap();
                    assert_eq!(actual.0, reference.0);
                    for (column, values) in actual.0.iter().enumerate() {
                        for (row, value) in values.iter().enumerate() {
                            let point = stripe.shift.mul(stripe.root.pow(row as u128));
                            let expected = cache.columns[column]
                                .iter()
                                .rev()
                                .fold(F::ZERO, |sum, c| sum.mul(point).add(*c));
                            assert_eq!(*value, expected);
                        }
                    }
                })
            });
            assert_eq!(result.is_err(), failure == 2);
            assert!(erased.iter().any(|entry| entry.nonzero_before > 0));
            assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
        }
    }
}

fn diagnostic_output_payload_v1(output: &ZeroizingBaseColumnsV1) -> usize {
    core::mem::size_of::<ZeroizingBaseColumnsV1>()
        + output.0.capacity() * core::mem::size_of::<Vec<F>>()
        + output
            .0
            .iter()
            .map(|column| column.capacity() * core::mem::size_of::<F>())
            .sum::<usize>()
}

fn diagnostic_compare_cached_outputs_v1(
    cache: &MainQuotientReplayCacheV1,
    current: &ZeroizingBaseColumnsV1,
    candidate: &ZeroizingBaseColumnsV1,
    stripe: main_quotient_stripes::MainQuotientStripeV1,
    oracle_limit: usize,
) {
    assert!(
        diagnostic_output_payload_v1(current)
            .checked_add(diagnostic_output_payload_v1(candidate))
            .unwrap()
            <= oracle_limit
    );
    assert_eq!(current.0, candidate.0);
    let width = candidate.0.len();
    assert!(width > 0);
    for column in [0, width - 1] {
        for row in [0, 1, stripe.rows - 1] {
            let point = stripe.shift.mul(stripe.root.pow(row as u128));
            let expected = cache.columns[column]
                .iter()
                .rev()
                .fold(F::ZERO, |sum, c| sum.mul(point).add(*c));
            assert_eq!(candidate.0[column][row], expected);
        }
    }
}

#[test]
#[ignore = "bounded actual-source cached-prefix CPU power reuse diagnostic; optimized build and isolated timing"]
fn actual_cached_prefix_public_powers_report_paired_cost() {
    use crate::privacy_engines::transparent_stark::{
        masked_trace_coefficients_with_mask_v1, sample_trace_mask_v1,
    };
    use crate::privacy_engines::zk_x509::{
        main_assembly::build_zk_x509_main_trace_assembly_v1,
        prover_observation::ObservationV1,
        relation::{
            ZkX509GovernanceV1,
            release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
        },
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha12Rng;
    use std::time::Instant;
    crate::test_support::require_no_debug_assertions("use matched optimized dependencies");
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true).unwrap();
    let trust_anchor = fixture.authoritative_state.trust_anchor();
    let crl = fixture.authoritative_state.crl_record();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .unwrap();
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let buffers = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    buffers
        .check_before_sources_v1(assembly.allocated_payload_bytes_v1())
        .unwrap();
    let digest = |seed| PrivacyOuterDigestV1::from_bytes([seed; 48]);
    // Existing component binding construction; no authenticated proof transcript.
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            [0x81; 32],
            assembly.verifier_profile.compiled_profile_digest,
            core::array::from_fn(|index| digest(u8::try_from(index).unwrap() + 1)),
        ),
        digest(0x91),
        digest(0xa1),
        digest(0xb1),
    )
    .unwrap();
    let sha =
        main_log19_sha_base_sources_v1(&assembly.sha_schedule, &assembly.sha_witnesses).unwrap();
    let p256 = P256MainBaseSourceV1::new_v1(&assembly).unwrap();
    let bound = MainLog19BoundTraceGroupSourceV1::bind_from_phase_v1(
        &layout, &assembly, sha, p256, &binding,
    )
    .unwrap();
    let mut projection = MainProjectionTraceGroupSourceV1::for_main_v1(
        &layout,
        &fixture.statement,
        &assembly.projection_trace,
    )
    .unwrap();
    let mut io =
        MainIoTraceGroupSourceV1::for_main_v1(&layout, &fixture.statement, &assembly.io).unwrap();
    projection
        .bind_challenges_v1(&binding.main_post_base())
        .unwrap();
    io.bind_challenges_v1(&binding.main_post_base()).unwrap();
    buffers
        .check_native_sources_v1(
            assembly.allocated_payload_bytes_v1(),
            &[
                bound.allocated_payload_bytes_v1(),
                projection.allocated_payload_bytes_v1(),
                io.allocated_payload_bytes_v1(),
            ],
        )
        .unwrap();
    let sources = MainTraceReplaySourcesV1::Bound {
        log19: &bound,
        projection: &projection,
        io: &io,
    };
    let registration = *layout
        .registered_segments
        .iter()
        .find(|r| r.segment.adapter == SegmentAdapterIdV1::P256Arithmetic)
        .unwrap();
    assert_eq!(
        (
            registration.segment.base_width,
            registration.segment.aux_width
        ),
        (211, 72)
    );
    let retained_plan =
        registered_retained_prover_plan_v1(registration.segment, layout.common_lde_log2).unwrap();
    let first_stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(
        registration.segment.trace_log2,
        retained_plan.quotient_coset_log2,
        0,
    )
    .unwrap();
    assert_eq!(first_stripe.rows, 1 << 19);
    assert!(first_stripe.count > 1);
    let original_cache_plan = main_ca_resources::MainCaJointBufferPlanV1::new_v1(&layout)
        .unwrap()
        .quotient_cache_plan_v1(&layout, registration)
        .unwrap();
    assert_eq!(
        (
            original_cache_plan.base_columns,
            original_cache_plan.aux_columns
        ),
        (211, 72)
    );
    let policy = MainBoundedTransformPolicyV1::for_assembly_v1(
        &layout,
        assembly.allocated_payload_bytes_v1(),
    )
    .unwrap()
    .for_quotient_layout_v1(
        registration.segment.base_width,
        registration.segment.aux_width,
    )
    .unwrap()
    .diagnostic_cpu_policy_v1();
    assert_eq!(policy.columns_v1(first_stripe.rows), 0);
    assert_eq!(
        main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1,
        4_194_384
    );
    let mut records = 0;
    for width in [8_usize, 16, 17] {
        let exact = budget(width, original_cache_plan.coefficient_count);
        let plan = original_cache_plan
            .reserve_additional_v1(
                original_cache_plan
                    .payload_limit
                    .checked_sub(exact)
                    .unwrap(),
            )
            .unwrap()
            .prioritize_registration_v1(registration)
            .unwrap();
        assert_eq!((plan.base_columns, plan.aux_columns), (0, width));
        let mut rng = ChaCha12Rng::from_seed([157; 32]);
        let cache = MainQuotientReplayCacheV1::from_replay_v1(plan, |kind, range| {
            assert!(matches!(kind, MainTraceColumnKindV1::Aux));
            let native = sources.native_columns_v1(
                &layout,
                kind,
                registration.trace_group,
                registration.aux_start + range.start..registration.aux_start + range.end,
            )?;
            let mut output = Vec::new();
            output
                .try_reserve_exact(range.len())
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            if output.capacity() != range.len() {
                return Err(ZkX509StarkErrorV1::ProofTooLarge);
            }
            for column in native {
                if column.len() != registration.segment.trace_size()
                    || column.iter().any(|value| F::canonical(value.0).is_none())
                {
                    return Err(ZkX509StarkErrorV1::ProfileMismatch);
                }
                let mask = sample_trace_mask_v1(MASK_DEGREE, &mut rng)
                    .map_err(map_transparent_error_v1)?;
                let coefficients = masked_trace_coefficients_with_mask_v1(
                    &column,
                    registration.segment.trace_log2,
                    mask.coefficients(),
                )
                .map_err(map_transparent_error_v1)?;
                output.push(ZeroizingMainTraceColumnV1(coefficients));
            }
            Ok(output)
        })
        .unwrap();
        assert!(cache.allocated_payload_v1().unwrap() <= plan.payload_limit);
        let input_capacity = cache.allocated_payload_v1().unwrap();
        // Two bounded prefix results fit within the original auxiliary output
        // charge, including all actual vector headers; never a full mirror.
        let oracle_limit = registration
            .segment
            .aux_width
            .checked_mul(
                first_stripe.rows * core::mem::size_of::<F>() + core::mem::size_of::<Vec<F>>(),
            )
            .unwrap()
            + core::mem::size_of::<ZeroizingBaseColumnsV1>();
        let planned_pair_payload = width
            .checked_mul(
                first_stripe
                    .rows
                    .checked_mul(core::mem::size_of::<F>())
                    .unwrap()
                    + core::mem::size_of::<Vec<F>>(),
            )
            .unwrap()
            .checked_add(core::mem::size_of::<ZeroizingBaseColumnsV1>())
            .unwrap()
            .checked_mul(2)
            .unwrap();
        assert!(
            planned_pair_payload <= oracle_limit,
            "admit both bounded outputs before allocation"
        );
        for workers in [8_usize, 16] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            pool.install(|| {
                for ordinal in [0, first_stripe.count - 1] {
                    let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(
                        registration.segment.trace_log2,
                        retained_plan.quotient_coset_log2,
                        ordinal,
                    )
                    .unwrap();
                    // One ordinary production result is the common resident
                    // oracle for this whole case. Every measured arm holds only
                    // its own output beside it and clears that output before the
                    // next arm starts. The original single-output allowance
                    // admits these TWO bounded prefixes before either allocates.
                    let oracle = cache
                        .evaluate_with_v1(
                            MainTraceColumnKindV1::Aux,
                            width,
                            stripe,
                            policy,
                            |_| panic!("cached"),
                            |_, _, _| panic!("CPU"),
                            || false,
                        )
                        .unwrap();
                    let (warm_candidate, warm_count, _, warm_batches) = cache
                        .diagnostic_evaluate_cached_v1(
                            MainTraceColumnKindV1::Aux,
                            width,
                            stripe,
                            policy,
                            || false,
                            |_| Ok(()),
                        )
                        .unwrap();
                    assert_eq!((warm_count, warm_batches), (1, width.div_ceil(8)));
                    diagnostic_compare_cached_outputs_v1(
                        &cache,
                        &oracle,
                        &warm_candidate,
                        stripe,
                        oracle_limit,
                    );
                    drop(warm_candidate);
                    let oracle_payload = diagnostic_output_payload_v1(&oracle);
                    // Two rounds in each order, with identical oracle residency.
                    for round in 0..4 {
                        let mut work_ns = [0_u128; 2];
                        let mut constructors = [width.div_ceil(8), 0];
                        let mut peaks = [0; 2];
                        let mut cleanup_ns = [0_u128; 2];
                        let mut arm_payloads = [0; 2];
                        for candidate in if round % 2 == 0 {
                            [false, true]
                        } else {
                            [true, false]
                        } {
                            let index = usize::from(candidate);
                            let observer = ObservationV1::begin_v1();
                            let started = Instant::now();
                            let output = if candidate {
                                let (values, count, peak, batches) = cache
                                    .diagnostic_evaluate_cached_v1(
                                        MainTraceColumnKindV1::Aux,
                                        width,
                                        stripe,
                                        policy,
                                        || false,
                                        |_| Ok(()),
                                    )
                                    .unwrap();
                                assert_eq!(batches, width.div_ceil(8));
                                constructors[index] = count;
                                peaks[index] = peak;
                                values
                            } else {
                                cache
                                    .evaluate_with_v1(
                                        MainTraceColumnKindV1::Aux,
                                        width,
                                        stripe,
                                        policy,
                                        |_| panic!("cached"),
                                        |_, _, _| panic!("CPU"),
                                        || false,
                                    )
                                    .unwrap()
                            };
                            work_ns[index] = started.elapsed().as_nanos();
                            let observation = observer.finish_v1().public_text_v1();
                            assert!(observation.contains(&format!(
                                "quotient_stripe_cpu_forward_columns={width}\n"
                            )));
                            assert!(observation.contains("quotient_stripe_metal_forward_columns=0\n"));
                            // Exact output, representative Horner checks and
                            // actual-capacity admission are outside both timers.
                            diagnostic_compare_cached_outputs_v1(
                                &cache,
                                &oracle,
                                &output,
                                stripe,
                                oracle_limit,
                            );
                            arm_payloads[index] = diagnostic_output_payload_v1(&output);
                            let started = Instant::now();
                            drop(output);
                            cleanup_ns[index] = started.elapsed().as_nanos();
                        }
                        assert_eq!(constructors, [width.div_ceil(8), 1]);
                        // Setup follows both evaluated and cleared arms. The
                        // same oracle stays live; no timed arm retains a peer.
                        for candidate in if round % 2 == 0 {
                            [false, true]
                        } else {
                            [true, false]
                        } {
                            let index = usize::from(candidate);
                            let started = Instant::now();
                            let expected = if candidate { 1 } else { width.div_ceil(8) };
                            let mut setup_constructors = 0;
                            let mut setup_peak = 0;
                            for _ in 0..expected {
                                let powers = policy.cpu_powers_v1(stripe.rows, stripe.root).unwrap().unwrap();
                                setup_constructors += 1;
                                setup_peak = setup_peak.max(powers.allocated_payload_bytes_v1().unwrap());
                                std::hint::black_box(&powers);
                                drop(powers);
                            }
                            let setup_ns = started.elapsed().as_nanos();
                            assert_eq!(setup_constructors, expected);
                            assert!(setup_peak <= main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1);
                            assert!(peaks[index] <= main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1);
                            println!("quotient_power_cost workers={workers} rows={} original_aux_width={} width={width} coefficient_count={} stripe_count={} ordinal={ordinal} round={round} candidate={candidate} constructors={} constructor_count_scope={} setup_constructors_observed={setup_constructors} setup_ns={setup_ns} work_ns={} cleanup_ns={} full_ns={} candidate_public_peak_observed={} setup_peak_observed={setup_peak} oracle_limit={oracle_limit} oracle_payload={oracle_payload} arm_payload={} resident_oracle=common_warm_current arm_output=cleared_before_next parity=exact baseline=production_evaluate scope=all_cached_actual_source_prefix",
                                stripe.rows, registration.segment.aux_width, cache.coefficient_count, stripe.count, constructors[index], if candidate { "observed_successful" } else { "source_derived_expected" }, work_ns[index], cleanup_ns[index], work_ns[index] + cleanup_ns[index], peaks[index], arm_payloads[index]);
                            records += 1;
                        }
                        assert_eq!(diagnostic_output_payload_v1(&oracle), oracle_payload);
                        assert_eq!(cache.allocated_payload_v1().unwrap(), input_capacity);
                    }
                    // This shared warmup/oracle cleanup is outside both arms.
                    drop(oracle);
                }
            });
        }
    }
    assert_eq!(records, 96);
    println!(
        "quotient_power_cost_complete records={records} parity=exact production_baseline=true wholeproof_claim=false constructor_setup_separate=true baseline_batch_count=source_derived columns_observed=true balanced_order=true common_resident_oracle=true per_arm_clear=true"
    );
}
