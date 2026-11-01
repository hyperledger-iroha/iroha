//! Complete ten-registration setup compared with the original per-column pipeline.

use super::*;
use crate::privacy_engines::zk_x509::{
    main_assembly::build_zk_x509_main_trace_assembly_v1,
    relation::{
        ZkX509GovernanceV1,
        release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
    },
};

fn with_source_v1(
    consume: impl FnOnce(
        &AggregateProofLayoutV1,
        &mut P256MainBoundSourceV1,
        MainBoundedTransformPolicyV1,
    ),
) {
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
    let digest = |seed| PrivacyOuterDigestV1::from_bytes([seed; 48]);
    let pre_aux = ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
        [0x81; 32],
        assembly.verifier_profile.compiled_profile_digest,
        core::array::from_fn(|index| digest(u8::try_from(index).unwrap() + 1)),
    );
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        pre_aux,
        digest(0x91),
        digest(0xA1),
        digest(0xB1),
    )
    .unwrap();
    let mut base = P256MainBaseSourceV1::new_v1(&assembly).unwrap();
    let mut source = base.bind_v1(&binding.main_post_base()).unwrap();
    let policy = MainBoundedTransformPolicyV1::for_assembly_v1(
        &layout,
        assembly.allocated_payload_bytes_v1(),
    )
    .unwrap();
    consume(&layout, &mut source, policy);
}

// Exact pre-change pending-copy stream. Only its concrete source and native-log
// lookup are specialized; original per-column public replay and IFFT stay intact.
fn original_sets_v1(
    source: &MainP256Log16ProverConstraintSourceV1<'_>,
    mut cache_plan: impl FnMut(
        RegisteredSegmentLayoutV1,
    ) -> Result<
        main_quotient_cache::MainQuotientCachePlanV1,
        ZkX509StarkErrorV1,
    >,
    mut consume: impl FnMut(&mut ZeroizingMainFixedPolynomialSetV1) -> Result<(), ZkX509StarkErrorV1>,
) -> Result<(), ZkX509StarkErrorV1> {
    let mut pending: Option<ZeroizingMainFixedPolynomialSetV1> = None;
    source.stream_fixed_polynomials_v1(|registration, local_column, coefficients| {
        if registration.segment.trace_log2 != 16
            || coefficients.len() != registration.segment.trace_size()
            || coefficients
                .iter()
                .any(|value| F::canonical(value.0).is_none())
            || registration.segment.adapter == SegmentAdapterIdV1::P256Arithmetic
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if pending
            .as_ref()
            .is_some_and(|set| set.registration != registration)
        {
            let mut completed = pending
                .take()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
            if completed.cache_columns_v1()?.len() != completed.registration.segment.fixed_width {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            consume(&mut completed)?;
        }
        if pending.is_none() {
            pending = Some(ZeroizingMainFixedPolynomialSetV1 {
                registration,
                cache_plan: cache_plan(registration)?,
                owner: MainFixedCoefficientOwnerV1::Dense(ZeroizingBaseColumnsV1(Vec::new())),
            });
        }
        let set = pending
            .as_mut()
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if set.registration != registration
            || local_column != set.cache_columns_v1()?.len()
            || local_column >= registration.segment.fixed_width
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut retained = ZeroizingMainTraceColumnV1(Vec::new());
        retained
            .0
            .try_reserve_exact(coefficients.len())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        retained.0.extend_from_slice(coefficients);
        set.dense_columns_mut_v1()?.push(retained.into_vec_v1());
        Ok(())
    })?;
    let mut completed = pending.ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if completed.cache_columns_v1()?.len() != completed.registration.segment.fixed_width {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    consume(&mut completed)
}
fn references_v1(
    layout: &AggregateProofLayoutV1,
    source: &MainP256Log16ProverConstraintSourceV1<'_>,
) -> Vec<(RegisteredSegmentLayoutV1, ZeroizingBaseColumnsV1)> {
    let plan = main_resources::MainProverBufferPlanV1::new_v1(layout).unwrap();
    let mut references = Vec::new();
    original_sets_v1(
        source,
        |registration| plan.quotient_cache_plan_v1(layout, registration),
        |set| {
            let registration = set.registration;
            references.push((
                registration,
                ZeroizingBaseColumnsV1(set.full_coefficients_for_test_v1()?.to_vec()),
            ));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(references.len(), 10);
    assert_eq!(
        references
            .iter()
            .map(|(_, columns)| columns.len())
            .sum::<usize>(),
        415
    );
    references
}

fn verify_sets_v1(
    layout: &AggregateProofLayoutV1,
    provider: &MainProverConstraintProviderV1<'_, '_>,
    policy: MainBoundedTransformPolicyV1,
    references: &[(RegisteredSegmentLayoutV1, ZeroizingBaseColumnsV1)],
    original: bool,
) {
    let plan = main_resources::MainProverBufferPlanV1::new_v1(layout).unwrap();
    let mut index = 0;
    let mut compare = |set: &mut ZeroizingMainFixedPolynomialSetV1| {
        let (registration, expected) = &references[index];
        assert_eq!(set.registration, *registration);
        assert_eq!(set.full_coefficients_for_test_v1()?, expected.0.as_slice());
        index += 1;
        Ok(())
    };
    if original {
        let MainProverConstraintProviderV1::Log16(source) = provider else {
            panic!("Log16 fixture");
        };
        original_sets_v1(
            source,
            |registration| plan.quotient_cache_plan_v1(layout, registration),
            &mut compare,
        )
        .unwrap();
    } else {
        stream_main_fixed_polynomial_sets_v1(
            provider,
            policy,
            |registration| plan.quotient_cache_plan_v1(layout, registration),
            &mut compare,
        )
        .unwrap();
    }
    assert_eq!(index, 10);
}

#[test]
#[ignore = "complete maximum-credential Log16 coefficient parity and bound-source refusal order"]
fn maximum_log16_fixed_sets_preserve_every_coefficient_and_source_refusal() {
    with_source_v1(|layout, source, policy| {
        {
            let prover =
                MainP256Log16ProverConstraintSourceV1::for_main_v1(layout, source).unwrap();
            let references = references_v1(layout, &prover);
            let provider = MainProverConstraintProviderV1::Log16(prover);
            verify_sets_v1(layout, &provider, policy, &references, false);
            let plan = main_resources::MainProverBufferPlanV1::new_v1(layout).unwrap();
            let events = std::cell::RefCell::new(Vec::new());
            assert_eq!(
                stream_main_fixed_polynomial_sets_v1(
                    &provider,
                    policy,
                    |registration| {
                        events.borrow_mut().push((0, registration));
                        Err(ZkX509StarkErrorV1::ProofTooLarge)
                    },
                    |_| panic!("cache refusal precedes consume")
                ),
                Err(ZkX509StarkErrorV1::ProofTooLarge)
            );
            assert_eq!(events.borrow().len(), 1);
            events.borrow_mut().clear();
            assert_eq!(
                stream_main_fixed_polynomial_sets_v1(
                    &provider,
                    policy,
                    |registration| {
                        events.borrow_mut().push((0, registration));
                        plan.quotient_cache_plan_v1(layout, registration)
                    },
                    |set| {
                        events.borrow_mut().push((1, set.registration));
                        Err(ZkX509StarkErrorV1::InternalInvariant)
                    }
                ),
                Err(ZkX509StarkErrorV1::InternalInvariant)
            );
            let events = events.into_inner();
            assert_eq!(events.len(), 2);
            assert_eq!(events[0].0, 0);
            assert_eq!(events[1].0, 1);
            assert_eq!(events[0].1, events[1].1);
        }
        source.zeroize_private_v1();
        let registration =
            P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::WindowBatch, 0).unwrap();
        let mut columns = ZeroizingBaseColumnsV1(vec![
            vec![F(19); 1 << 16];
            P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1
        ]);
        assert_eq!(
            source.fill_log16_fixed_matrix_v1(registration, &mut columns.0),
            Err(P256AggregateAdapterErrorV1::Phase)
        );
        assert!(columns.0.iter().flatten().all(|value| *value == F(19)));
        columns.0[0].pop();
        assert_eq!(
            source.fill_log16_fixed_matrix_v1(registration, &mut columns.0),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(columns.0.iter().flatten().all(|value| *value == F(19)));
    });
}

fn median_v1(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2.0
}

#[test]
#[ignore = "balanced complete ten-registration fixed setup, full comparison and clearing at 20 workers"]
fn log16_fixed_complete_ten_registration_cost_20_workers() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(20)
        .build()
        .unwrap();
    pool.install(|| assert_eq!(rayon::current_num_threads(), 20));
    with_source_v1(|layout, source, policy| {
        let prover = MainP256Log16ProverConstraintSourceV1::for_main_v1(layout, source).unwrap();
        let references = pool.install(|| references_v1(layout, &prover));
        let provider = MainProverConstraintProviderV1::Log16(prover);
        for original in [true, false] {
            pool.install(|| verify_sets_v1(layout, &provider, policy, &references, original));
        }
        eprintln!(
            "log16_fixed_complete_cost_header workers=20 registrations=10 columns=415 rows=65536 rounds=8 caller=pool_install fixture_and_oracle=outside_timing reference_bytes={} comparison_and_final_owner_drop=inside_timing source_policy=actual_assembly backend=original_cpu_vs_production_auto no_end_to_end_claim=true",
            415 * (1usize << 16) * core::mem::size_of::<F>()
        );
        let mut samples = [Vec::new(), Vec::new()];
        for round in 0..8 {
            for mode in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                let start = std::time::Instant::now();
                pool.install(|| verify_sets_v1(layout, &provider, policy, &references, mode == 0));
                let seconds = start.elapsed().as_secs_f64();
                samples[mode].push(seconds);
                eprintln!(
                    "log16_fixed_complete_cost_sample round={round} mode={} seconds={seconds:.9}",
                    if mode == 0 { "original" } else { "row_matrix" }
                );
            }
        }
        let original = median_v1(&mut samples[0]);
        let candidate = median_v1(&mut samples[1]);
        eprintln!(
            "log16_fixed_complete_cost_summary original_median_seconds={original:.9} row_matrix_median_seconds={candidate:.9} speedup={:.6}",
            original / candidate
        );
    });
}
