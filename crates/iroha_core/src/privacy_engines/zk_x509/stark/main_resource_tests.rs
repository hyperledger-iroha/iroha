//! Whole-profile retained allocation bounds, without materializing private traces.

use super::*;

#[test]
fn small_native_source_forecast_uses_full_widths_and_rejects_oversized_extents() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let shape = main_resources::MainSmallSourceShapeV1 {
        der_active_rows: ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        io_active_rows: 1 << 18,
        io_declarations_payload: 4096,
        projection_rows: 1 << 15,
    };
    let (retained, scratch) = shape.allocation_forecast_v1(&layout).unwrap();
    let matrices = 327_680 * (76 + 196) * 8 + (1 << 18) * (28 + 17 + 39) * 8 + (1 << 15) * 32 * 8;
    assert_eq!(matrices, 897_581_056);
    assert!(retained >= matrices + 4096);
    assert!(scratch < main_resources::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1);
    let mut changed = shape;
    changed.der_active_rows += 1;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.io_active_rows += 1;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.projection_rows -= 1;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.io_declarations_payload = usize::MAX;
    assert!(changed.allocation_forecast_v1(&layout).is_err());
    changed = shape;
    changed.io_active_rows = 1;
    assert_eq!(
        changed.allocation_forecast_v1(&layout).unwrap(),
        (retained, scratch)
    );
}

#[test]
fn canonical_source_shape_forecasts_fit_the_preconstruction_allowances() {
    use super::super::super::sha_call_bus_stark::ZkX509ShaCallPublicShapeV1;
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let (small, small_scratch) = main_resources::MainSmallSourceShapeV1 {
        der_active_rows: ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        io_active_rows: 1 << 18,
        io_declarations_payload: 1 << 20,
        projection_rows: 1 << 15,
    }
    .allocation_forecast_v1(&layout)
    .unwrap();
    let sha_shape = ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    };
    let p256 = P256MainBaseSourceV1::allocation_forecast_v1().unwrap();
    let p256_scratch = P256MainBaseSourceV1::replay_scratch_forecast_v1().unwrap();
    let sha_fixed = ZkX509ShaBatchFixedProviderV1::allocation_forecast_v1(sha_shape).unwrap();
    let sha_scratch = ZkX509ShaBatchFixedProviderV1::replay_scratch_forecast_v1(sha_shape).unwrap();
    let retained = small + p256 + sha_fixed;
    assert!(retained <= main_resources::MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1);
    let serial_scratch = small_scratch.max(sha_scratch).max(p256_scratch);
    assert!(serial_scratch <= main_resources::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1);
    eprintln!(
        "MAIN source shape forecasts: retained={retained}, small={small}, p256={p256}, sha_fixed={sha_fixed}, serial_scratch={}",
        serial_scratch
    );
    // These construction bounds do not materialize or qualify the separately
    // borrowed maximum-profile assembly, nor measure allocator/process RSS.
}

#[test]
fn source_payload_admission_includes_scratch_reserve_and_capacity_overflow() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let reserve = main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1;
    let scratch = 13_579;
    let available = plan.remaining_source_and_runtime_envelope - reserve - scratch;
    let first = available / 3;
    let second = available - first;
    assert_eq!(
        plan.check_source_payloads_v1(&[first, second], scratch)
            .unwrap() as u64,
        super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
    );
    assert!(matches!(
        plan.check_source_payloads_v1(&[first, second + 1], scratch),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert!(matches!(
        plan.check_source_payloads_v1(&[first, second], scratch + 1),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert!(plan.check_source_payloads_v1(&[usize::MAX, 1], 0).is_err());
    assert!(plan.check_source_payloads_v1(&[0], usize::MAX).is_err());
    assert_eq!(
        plan.check_source_payloads_v1(&[], 0).unwrap(),
        plan.maximum_live_buffers + reserve
    );
}

#[test]
fn native_source_budget_is_reserved_before_construction_and_rechecked_after_binding() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let source_limit = main_resources::MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1;
    let scratch = main_resources::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1;
    let reserve = main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1;
    let assembly_limit =
        plan.remaining_source_and_runtime_envelope - source_limit - scratch - reserve;
    assert_eq!(
        plan.check_before_sources_v1(assembly_limit).unwrap() as u64,
        super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
    );
    assert!(matches!(
        plan.check_before_sources_v1(assembly_limit + 1),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert_eq!(
        plan.check_native_sources_v1(assembly_limit, &[source_limit / 2, source_limit / 2])
            .unwrap(),
        plan.check_before_sources_v1(assembly_limit).unwrap()
    );
    assert!(matches!(
        plan.check_native_sources_v1(0, &[source_limit, 1]),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
    assert!(plan.check_native_sources_v1(0, &[usize::MAX, 1]).is_err());
    // The source transition reuses the same retained assembly charge, rather
    // than treating a borrowed assembly as already freed after base commitment.
    assert!(matches!(
        plan.check_native_sources_v1(assembly_limit + 1, &[source_limit]),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
}

#[test]
fn replay_buffer_plan_charges_live_owners_and_leaves_an_explicit_source_envelope() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    assert_eq!(plan.masks, 81_690_944);
    assert_eq!(
        plan.joined_streams,
        layout.common_lde_size()
            * core::mem::size_of::<
                crate::privacy_engines::privacy_outer_hash::PrivacyOuterLastFieldStreamV1,
            >()
    );
    assert_eq!(
        plan.maximum_live_buffers,
        plan.masks
            + (plan.quotient_stage + plan.replay_batch).max(
                plan.joined_streams
                    + plan.replay_batch
                    + plan.composition
                    + plan.fri_stage
                    + plan.openings
            )
    );
    assert_eq!(
        (plan.maximum_live_buffers + plan.remaining_source_and_runtime_envelope) as u64,
        super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
    );
    assert!(plan.remaining_source_and_runtime_envelope > 0);
    // This remainder must also cover borrowed sources and process overhead;
    // passing this buffer check is not whole-prover or RSS qualification.
    eprintln!("MAIN replay transform buffer plan: {plan:?}");
    let mut invalid = layout;
    invalid.trace_groups[5].base_width += 1;
    assert!(main_resources::MainProverBufferPlanV1::new_v1(&invalid).is_err());
}

#[test]
fn whole_main_retaining_every_masked_coefficient_exceeds_the_release_memory_ceiling() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let geometry = layout
        .trace_groups
        .iter()
        .map(|group| (group.native_trace_log2, group.base_width + group.aux_width))
        .collect::<Vec<_>>();
    assert_eq!(
        geometry,
        [
            (5, 800),
            (8, 190),
            (15, 49),
            (16, 805),
            (18, 67),
            (19, 3712)
        ]
    );
    let masks = MASK_DEGREE + 1;
    assert_eq!(masks, 1816);
    let coefficient_bytes = geometry
        .iter()
        .map(|(log, width)| ((1_u64 << log) + masks as u64) * *width as u64 * 8)
        .sum::<u64>();
    let mask_bytes = geometry
        .iter()
        .map(|(_, width)| *width as u64 * masks as u64 * 8)
        .sum::<u64>();
    assert_eq!(coefficient_bytes, 16_226_947_392);
    assert_eq!(mask_bytes, 81_690_944);
    assert!(coefficient_bytes > super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1);
    // This is a payload lower bound, excluding borrowed assembly/source traces,
    // quotient matrices, public fixed polynomials, FFT scratch and allocator overhead.
    assert_eq!(((1_u64 << 19) + masks as u64) * 3712 * 8, 15_623_184_384);
}
