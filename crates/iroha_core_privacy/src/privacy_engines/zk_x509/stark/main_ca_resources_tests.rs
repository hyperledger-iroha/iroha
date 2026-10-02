//! Simultaneous original owners, unchanged ceiling and every registration cache.

use super::*;

#[test]
fn joint_ca_owners_fit_only_after_registration_and_keep_the_existing_ceiling() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = MainCaJointBufferPlanV1::new_v1(&layout).unwrap();
    assert_eq!(
        plan.common,
        MainProverBufferPlanV1::new_v1(&layout).unwrap()
    );
    assert_eq!(plan.common.maximum_live_buffers, 3_697_993_184);
    assert_eq!(
        plan.ca_originals,
        823 * (6196 * core::mem::size_of::<F>() + core::mem::size_of::<Vec<F>>())
            + core::mem::size_of::<CaAwaitingMainAuxiliaryV1>()
    );
    assert_eq!(
        plan.main_originals,
        MainCaOriginalAuxiliaryV1::payload_bound_v1()
    );
    assert_eq!(plan.metadata, 1 << 20);
    assert_eq!(plan.ca_work, 640 << 20);
    let request = ca::ca_accumulator_resource_request_v1(2, 1, 136).unwrap();
    let envelope = ca::checked_ca_accumulator_resource_envelope_v1(request).unwrap();
    assert_eq!(envelope.fri_degree_cap, 9_215);
    assert_eq!(envelope.fri_degree_cap + 1, 9_216);
    // The inclusive degree needs 9,216 coefficients in each of four lanes.
    // Include the final coefficient of every lane and both complete CA
    // envelopes, each with its ten-byte header.
    assert_eq!(ca::ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1, 1_498_826);
    let ca_work = envelope.adapter_resident_payload_bytes
        + 7 * 2 * (1 << 16) * core::mem::size_of::<PrivacyOuterDigestV1>()
        + 4 * (1 << 16) * core::mem::size_of::<E>()
        + 4 * 9_216 * core::mem::size_of::<E>()
        + plan.metadata
        + 2 * ca::ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1;
    assert_eq!(ca_work, 586_488_364);
    assert!(ca_work < plan.ca_work);
    assert!(MainCaJointBufferPlanV1::metadata_required_v1().unwrap() < plan.metadata);
    assert!(plan.common.selected_replay <= 64 << 20);
    assert_eq!(super::super::main_key_joins::OPENINGS_V1, 31);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        core::mem::size_of::<aggregate::AggregateSupplementalDeepOpeningV1>(),
        120
    );
    let mut maximum = 0;
    for phase in [
        MainCaBufferPhaseV1::OriginalCommitments,
        MainCaBufferPhaseV1::Registration,
        MainCaBufferPhaseV1::PrivateLinks,
        MainCaBufferPhaseV1::Finalization,
    ] {
        let bytes = plan.required_v1(phase).unwrap();
        assert!(bytes <= plan.common.maximum_live_buffers);
        maximum = maximum.max(bytes);
    }
    assert_eq!(
        maximum,
        plan.required_v1(MainCaBufferPhaseV1::Registration).unwrap()
    );
    assert!(
        plan.required_v1(MainCaBufferPhaseV1::Registration).unwrap() + plan.main_originals
            > plan.common.maximum_live_buffers,
        "creating all24 original MAIN columns during a registration exceeds the unchanged ceiling"
    );
}

#[test]
fn every_joint_registration_reserves_ca_and_metadata_without_spending_cut_slack_twice() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = MainCaJointBufferPlanV1::new_v1(&layout).unwrap();
    let mut changed = 0;
    for registration in &layout.registered_segments {
        let ordinary = plan
            .common
            .quotient_cache_plan_v1(&layout, *registration)
            .unwrap();
        let joint = plan.quotient_cache_plan_v1(&layout, *registration).unwrap();
        let expected = ordinary
            .reserve_additional_v1(plan.ca_originals + plan.metadata)
            .unwrap()
            .prioritize_registration_v1(*registration)
            .unwrap();
        assert_eq!(
            (joint.base_columns, joint.aux_columns),
            (expected.base_columns, expected.aux_columns)
        );
        assert!(joint.base_columns <= ordinary.base_columns);
        assert!(
            joint.base_columns + joint.aux_columns <= ordinary.base_columns + ordinary.aux_columns
        );
        changed += usize::from(
            joint.base_columns + joint.aux_columns < ordinary.base_columns + ordinary.aux_columns,
        );
    }
    assert!(
        changed > 0,
        "joint owners must actually displace retained ordinary coefficients"
    );
    let mut foreign = plan;
    foreign.common.retained_cuts = 0;
    assert!(
        foreign
            .quotient_cache_plan_v1(&layout, layout.registered_segments[0])
            .is_err()
    );
}

#[test]
fn joint_phase_arithmetic_fails_closed_on_overflow_and_oversized_owners() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let original = MainCaJointBufferPlanV1::new_v1(&layout).unwrap();
    for phase in [
        MainCaBufferPhaseV1::OriginalCommitments,
        MainCaBufferPhaseV1::Registration,
        MainCaBufferPhaseV1::PrivateLinks,
        MainCaBufferPhaseV1::Finalization,
    ] {
        for mutation in 0..4 {
            let mut bad = original;
            match mutation {
                0 => bad.ca_originals = usize::MAX,
                1 => bad.metadata = usize::MAX,
                2 => bad.common.retained_cuts = usize::MAX,
                _ => bad.common.maximum_live_buffers = original.required_v1(phase).unwrap() - 1,
            }
            assert!(bad.required_v1(phase).is_err());
        }
    }
    for payload in [original.common.maximum_live_buffers, usize::MAX] {
        let mut bad = original;
        bad.main_originals = payload;
        assert!(bad.required_v1(MainCaBufferPhaseV1::PrivateLinks).is_err());
        assert!(bad.required_v1(MainCaBufferPhaseV1::Finalization).is_err());
        let mut bad = original;
        bad.ca_work = payload;
        assert!(bad.required_v1(MainCaBufferPhaseV1::Finalization).is_err());
    }
    assert!(sum_v1(&[usize::MAX, 1]).is_err());
}

#[test]
fn phase_ownership_requires_actual_capacity_and_disallows_early_main24() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = MainCaJointBufferPlanV1::new_v1(&layout).unwrap();
    for phase in [
        MainCaBufferPhaseV1::OriginalCommitments,
        MainCaBufferPhaseV1::Registration,
        MainCaBufferPhaseV1::PrivateLinks,
        MainCaBufferPhaseV1::Finalization,
    ] {
        let late = matches!(
            phase,
            MainCaBufferPhaseV1::PrivateLinks | MainCaBufferPhaseV1::Finalization
        );
        let correct = late.then_some(plan.main_originals);
        plan.check_original_payloads_v1(phase, plan.ca_originals, correct)
            .unwrap();
        let wrong_phase = (!late).then_some(plan.main_originals);
        assert!(
            plan.check_original_payloads_v1(phase, plan.ca_originals, wrong_phase)
                .is_err()
        );
        for bad_ca in [0, plan.ca_originals - 1, plan.ca_originals + 1, usize::MAX] {
            assert!(
                plan.check_original_payloads_v1(phase, bad_ca, correct)
                    .is_err()
            );
        }
        for bad_main in [
            0,
            plan.main_originals - 1,
            plan.main_originals + 1,
            usize::MAX,
        ] {
            assert!(
                plan.check_original_payloads_v1(phase, plan.ca_originals, Some(bad_main))
                    .is_err()
            );
        }
    }
}

#[test]
fn complete_joint_ordinary_registration_work_counts_original_ca_reservations() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = MainCaJointBufferPlanV1::new_v1(&layout).unwrap();
    assert_eq!(plan.common.maximum_live_buffers, 3_697_993_184);
    assert_eq!(plan.common.retained_cuts, 25_165_984);
    // Independent floor arithmetic is constant throughout this inline-owner
    // interval. The native type must remain in that interval for these pins.
    assert!(core::mem::size_of::<CaAwaitingMainAuxiliaryV1>() <= 65_536);
    assert_eq!(
        plan.ca_originals - core::mem::size_of::<CaAwaitingMainAuxiliaryV1>(),
        40_814_216
    );
    assert_eq!(plan.metadata, 1_048_576);
    let mut cached_columns = 0_u64;
    let mut ordinary_cached_columns = 0_u64;
    let mut native_iffts = 0_u64;
    let mut native_butterflies = 0_u64;
    let mut changed = 0;
    let mut columns = 0_u64;
    let mut native_cells = 0_u64;
    let mut masked_cells = 0_u64;
    let mut quotient_rows = 0_u64;
    for registration in &layout.registered_segments {
        let segment = registration.segment;
        let retained = registered_retained_prover_plan_v1(segment, layout.common_lde_log2).unwrap();
        let n = segment.trace_size() as u64;
        let m = retained.quotient_coset_rows as u64;
        let width = (segment.base_width + segment.aux_width) as u64;
        let stripe_rows = m.min(1 << 19);
        assert!(n <= stripe_rows);
        let stripes = m / stripe_rows;
        let ordinary = plan
            .common
            .quotient_cache_plan_v1(&layout, *registration)
            .unwrap();
        let cache = plan.quotient_cache_plan_v1(&layout, *registration).unwrap();
        let cached = (cache.base_columns + cache.aux_columns) as u64;
        if segment.adapter == SegmentAdapterIdV1::P256Arithmetic {
            assert_eq!((ordinary.base_columns, ordinary.aux_columns), (205, 72));
            assert_eq!((cache.base_columns, cache.aux_columns), (195, 72));
        }
        if segment.base_width == 285 && segment.aux_width == 280 {
            assert_eq!((ordinary.base_columns, ordinary.aux_columns), (28, 0));
            assert_eq!((cache.base_columns, cache.aux_columns), (18, 0));
        }
        let ordinary_cached = (ordinary.base_columns + ordinary.aux_columns) as u64;
        assert!(cache.base_columns <= segment.base_width);
        assert!(cache.aux_columns <= segment.aux_width);
        if segment.adapter == SegmentAdapterIdV1::P256Arithmetic {
            assert!(cache.base_columns == 0 || cache.aux_columns == segment.aux_width);
        } else {
            assert!(cache.aux_columns == 0 || cache.base_columns == segment.base_width);
        }
        assert!(cached <= ordinary_cached);
        if stripes == 1 {
            assert_eq!(cached, 0);
        }
        if cached != ordinary_cached {
            assert_eq!(ordinary_cached - cached, 10);
            assert_eq!(stripes, 4);
            changed += 1;
        }
        cached_columns += cached;
        ordinary_cached_columns += ordinary_cached;
        let replays = cached + (width - cached) * stripes;
        native_iffts += replays;
        native_butterflies += replays * (n / 2) * u64::from(segment.trace_log2);
        columns += width;
        native_cells += width * n;
        masked_cells += width * (n + MASK_DEGREE as u64 + 1);
        quotient_rows += m;
    }
    assert_eq!(layout.registered_segments.len(), 49);
    assert_eq!(columns, 5_811);
    assert_eq!(native_cells, 2_116_723_200);
    assert_eq!(masked_cells, 2_127_275_976);
    assert_eq!(quotient_rows, 53_215_232);
    assert_eq!(ordinary_cached_columns, 3_400);
    assert_eq!(changed, 6);
    // Five arithmetic registrations and RFC each lose ten cache columns;
    // their four stripes add60*(4-1)=180 real native inverse transforms.
    assert_eq!(cached_columns, 3_340);
    assert_eq!(native_iffts, 7_692);
    assert_eq!(native_butterflies, 29_386_060_288);
}
