//! ValueBus row-pass coefficients and complete ten-registration setup cost.

use super::*;
use main_fixed_replay_batch_tests::with_maximum_fixed_source_v1;

// Preserved pre-change complete setup, including public per-column row replay,
// validation, final dense ownership and the same bounded inverse8 transaction.
fn original_fixed_set_v1(
    source: &MainLog19ProverConstraintSourceV1<'_, '_>,
    registration: RegisteredSegmentLayoutV1,
    transform_policy: main_bounded_transform::MainBoundedTransformPolicyV1,
    cache_plan: main_quotient_cache::MainQuotientCachePlanV1,
) -> Result<ZeroizingMainFixedPolynomialSetV1, ZkX509StarkErrorV1> {
    main_bounded_transform::check_completion_v1(
        fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1(),
    )?;
    let index = source.source.registration_index_v1(registration)?;
    let transform_policy = transform_policy.for_quotient_layout_v1(
        registration.segment.base_width,
        registration.segment.aux_width,
    )?;
    let root = goldilocks_primitive_root_v1(registration.segment.trace_log2)
        .map_err(map_transparent_error_v1)?;
    let arithmetic = if index >= 6 {
        let binding = source.source.p256_binding_v1(registration)?;
        (binding.p256.adapter_v1() == P256MainAdapterV1::Arithmetic
            && binding.p256.local_instance_v1() == 0)
            .then_some(binding.p256)
    } else {
        None
    };
    if registration.segment.adapter == SegmentAdapterIdV1::P256Arithmetic {
        let binding = source.source.p256_binding_v1(registration)?;
        if arithmetic != Some(binding.p256) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let (owner, cache_plan) =
            main_fixed_coset::arithmetic_basis::MainArithmeticFixedCoefficientsV1::from_source_v1(
                &source.source.p256,
                binding,
                transform_policy,
                cache_plan,
            )?;
        return Ok(ZeroizingMainFixedPolynomialSetV1 {
            registration,
            cache_plan,
            owner: MainFixedCoefficientOwnerV1::Arithmetic(owner),
        });
    }
    if arithmetic.is_some() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut columns = ZeroizingBaseColumnsV1(Vec::new());
    columns
        .0
        .try_reserve_exact(registration.segment.fixed_width)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    // Each producer owns its separately sized fixed row. Resource accounting
    // reserves both current/next rows at the maximum producer width plus16 words.
    const _: () =
        assert!(ZK_X509_SHA_BATCH_FIXED_WIDTH_V1 <= main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1);
    const _: () = assert!(
        P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1 <= main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1
    );
    if (2..=5).contains(&index) {
        // These final public columns were already retained together by the old
        // per-column path. Construct each complete SHA row once into that same
        // charged matrix before bounded interpolation in its original order.
        #[cfg(test)]
        let rows_timer = PhaseTimerV1::start_v1(PhaseV1::CompositionShaFixedRows);
        for _ in 0..registration.segment.fixed_width {
            columns.0.push(
                zeroed_main_trace_column_v1(registration.segment.trace_size())?.into_vec_v1(),
            );
        }
        source
            .source
            .sha_fixed
            .fill_fixed_matrix_v1(index - 2, &mut columns.0)
            .map_err(map_main_sha_source_error_v1)?;
        #[cfg(test)]
        rows_timer.complete_v1();
    } else {
        for column in 0..registration.segment.fixed_width {
            let values = source.source.native_fixed_column_v1(registration, column)?;
            columns.0.push(values.into_vec_v1());
        }
    }
    #[cfg(test)]
    let inverse_timer = if (2..=5).contains(&index) {
        Some(PhaseTimerV1::start_v1(
            PhaseV1::CompositionShaFixedInverseTransform,
        ))
    } else {
        None
    };
    main_initial_fixed_coefficients_native_with_v1(
        &mut columns.0,
        root,
        transform_policy,
        &mut main_native_transform::ExactNativeTransformV1,
        fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1,
    )?;
    #[cfg(test)]
    if let Some(timer) = inverse_timer {
        timer.complete_v1();
    }
    Ok(ZeroizingMainFixedPolynomialSetV1 {
        registration,
        cache_plan,
        owner: MainFixedCoefficientOwnerV1::Dense(columns),
    })
}

fn registrations_v1(
    source: &MainLog19ProverConstraintSourceV1<'_, '_>,
) -> [RegisteredSegmentLayoutV1; 10] {
    let registrations = source
        .source
        .registrations
        .iter()
        .copied()
        .filter(|registration| registration.segment.adapter == SegmentAdapterIdV1::P256ValueBus)
        .collect::<Vec<_>>();
    assert_eq!(registrations.len(), 10);
    assert_eq!(
        registrations
            .iter()
            .map(|registration| registration.segment.fixed_width)
            .sum::<usize>(),
        340
    );
    for registration in &registrations {
        assert_eq!(registration.segment.trace_log2, 19);
        let binding = source.source.p256_binding_v1(*registration).unwrap();
        assert_eq!(binding.p256.adapter_v1(), P256MainAdapterV1::ValueBus);
        assert!(matches!(binding.p256.local_instance_v1(), 0 | 1));
    }
    registrations.try_into().unwrap()
}

#[test]
fn value_fixed_full_geometry_keeps_existing_matrix_and_transform_admission() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let registrations = layout
        .registered_segments
        .iter()
        .filter(|registration| {
            registration.segment.trace_log2 == 19
                && registration.segment.adapter == SegmentAdapterIdV1::P256ValueBus
        })
        .collect::<Vec<_>>();
    assert_eq!(registrations.len(), 10);
    assert_eq!(
        registrations
            .iter()
            .filter(|registration| registration.segment.fixed_width == 46)
            .count(),
        5
    );
    assert_eq!(
        registrations
            .iter()
            .filter(|registration| registration.segment.fixed_width == 22)
            .count(),
        5
    );
    assert_eq!(
        registrations
            .iter()
            .map(|registration| registration
                .segment
                .fixed_width
                .div_ceil(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1))
            .sum::<usize>(),
        45
    );
    assert_eq!(registrations.iter().map(|registration|registration.segment.fixed_width*registration.segment.trace_size()).sum::<usize>(),178_257_920);
    assert_eq!(
        registrations
            .iter()
            .map(|registration| registration.segment.trace_size())
            .sum::<usize>(),
        5_242_880
    );
    assert!(
        registrations
            .iter()
            .all(|registration| registration.segment.fixed_width
                <= main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1)
    );
    assert_eq!(
        registrations
            .iter()
            .map(|registration| registration.segment.fixed_width
                * registration.segment.trace_size()
                * core::mem::size_of::<F>())
            .max(),
        Some(192_937_984)
    );
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    assert_eq!(
        plan.maximum_live_buffers,
        main_resources::CURRENT_FULL_PROFILE_PEAK_V1
    );
    assert_eq!(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1, 8);
}

#[test]
#[ignore = "complete maximum-credential ValueBus fixed coefficients against original per-column replay and scalar IFFT"]
fn maximum_value_fixed_sets_match_every_independent_coefficient_and_refusal() {
    with_maximum_fixed_source_v1(|layout, source, policy| {
        let prover = MainLog19ProverConstraintSourceV1::for_main_v1(layout, source).unwrap();
        let plan = main_resources::MainProverBufferPlanV1::new_v1(layout).unwrap();
        for registration in registrations_v1(&prover) {
            let cache = plan.quotient_cache_plan_v1(layout, registration).unwrap();
            let mut actual =
                main_log19_fixed_polynomial_set_v1(&prover, registration, policy, cache).unwrap();
            assert_eq!(actual.registration, registration);
            let root = goldilocks_primitive_root_v1(19).unwrap();
            for (column, coefficients) in actual
                .full_coefficients_for_test_v1()
                .unwrap()
                .iter()
                .enumerate()
            {
                let mut expected = source.native_fixed_column_v1(registration, column).unwrap();
                goldilocks_ifft_v1(&mut expected, root).unwrap();
                assert_eq!(
                    coefficients.as_slice(),
                    &*expected,
                    "registration={registration:?} column={column}"
                );
            }
            let mut invalid = registration;
            invalid.segment.fixed_width -= 1;
            assert!(main_log19_fixed_polynomial_set_v1(&prover, invalid, policy, cache).is_err());
            let mut invalid = registration;
            invalid.segment.trace_log2 = 18;
            assert!(main_log19_fixed_polynomial_set_v1(&prover, invalid, policy, cache).is_err());
        }
    });
}

fn references_v1(
    layout: &AggregateProofLayoutV1,
    source: &MainLog19ProverConstraintSourceV1<'_, '_>,
    policy: main_bounded_transform::MainBoundedTransformPolicyV1,
) -> Vec<(RegisteredSegmentLayoutV1, ZeroizingBaseColumnsV1)> {
    let plan = main_resources::MainProverBufferPlanV1::new_v1(layout).unwrap();
    let mut result = Vec::new();
    for registration in registrations_v1(source) {
        let mut old = original_fixed_set_v1(
            source,
            registration,
            policy,
            plan.quotient_cache_plan_v1(layout, registration).unwrap(),
        )
        .unwrap();
        result.push((
            registration,
            ZeroizingBaseColumnsV1(old.full_coefficients_for_test_v1().unwrap().to_vec()),
        ));
    }
    assert_eq!(
        result
            .iter()
            .flat_map(|(_, columns)| columns.0.iter())
            .map(|column| column.capacity() * core::mem::size_of::<F>())
            .sum::<usize>(),
        1_426_063_360
    );
    result
}

fn verify_complete_v1(
    layout: &AggregateProofLayoutV1,
    source: &MainLog19ProverConstraintSourceV1<'_, '_>,
    policy: main_bounded_transform::MainBoundedTransformPolicyV1,
    reference: &[(RegisteredSegmentLayoutV1, ZeroizingBaseColumnsV1)],
    original: bool,
) {
    let plan = main_resources::MainProverBufferPlanV1::new_v1(layout).unwrap();
    assert_eq!(reference.len(), 10);
    for (registration, expected) in reference {
        let cache = plan.quotient_cache_plan_v1(layout, *registration).unwrap();
        let mut actual = if original {
            original_fixed_set_v1(source, *registration, policy, cache)
        } else {
            main_log19_fixed_polynomial_set_v1(source, *registration, policy, cache)
        }
        .unwrap();
        assert_eq!(actual.registration, *registration);
        assert_eq!(
            actual.full_coefficients_for_test_v1().unwrap(),
            expected.0.as_slice()
        );
        // Original final owner Drop/complete erasure and deallocation occur
        // before the next registration and inside the caller's timer.
        drop(actual);
    }
}

#[test]
#[ignore = "balanced complete ten-registration ValueBus setup, full parity and clearing at20 workers"]
fn value_fixed_complete_ten_registration_cost_20_workers() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(20)
        .build()
        .unwrap();
    pool.install(|| assert_eq!(rayon::current_num_threads(), 20));
    with_maximum_fixed_source_v1(|layout, source, policy| {
        let prover = MainLog19ProverConstraintSourceV1::for_main_v1(layout, source).unwrap();
        let reference = pool.install(|| references_v1(layout, &prover, policy));
        for original in [true, false] {
            pool.install(|| verify_complete_v1(layout, &prover, policy, &reference, original));
        }
        eprintln!(
            "value_fixed_complete_cost_header workers=20 registrations=10 columns=340 rows=524288 inverse_batches=45 rounds=8 caller=pool_install fixture_and_oracle=outside_timing reference_field_bytes=1426063360 comparison_and_final_owner_drop=inside_timing source_policy=actual_assembly backend=production_auto_both no_end_to_end_claim=true"
        );
        let mut samples = [Vec::new(), Vec::new()];
        for round in 0..8 {
            for mode in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                let start = std::time::Instant::now();
                pool.install(|| verify_complete_v1(layout, &prover, policy, &reference, mode == 0));
                let seconds = start.elapsed().as_secs_f64();
                samples[mode].push(seconds);
                eprintln!(
                    "value_fixed_complete_cost_sample round={round} mode={} seconds={seconds:.9}",
                    if mode == 0 { "original" } else { "row_matrix" }
                );
            }
        }
        let [original, candidate] = samples.map(|mut values| {
            values.sort_by(f64::total_cmp);
            (values[3] + values[4]) / 2.0
        });
        eprintln!(
            "value_fixed_complete_cost_summary original_median_seconds={original:.9} row_matrix_median_seconds={candidate:.9} speedup={:.6}",
            original / candidate
        );
    });
}
