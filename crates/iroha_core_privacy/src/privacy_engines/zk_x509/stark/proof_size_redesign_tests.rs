//! Source-derived byte accounting for the gated complete-relation redesign.

use super::*;

fn complete_oods_bytes(layout: &AggregateProofLayoutV1) -> usize {
    let parameters = layout.parameters_v1();
    let shared = layout.as_shared().unwrap();
    let full_rows =
        aggregate::AggregateProofLayoutV1::new(parameters, shared.trace_groups().to_vec()).unwrap();
    let before =
        aggregate::maximum_encoded_proof_with_deep_bytes_v1(parameters, &full_rows).unwrap();
    let q = parameters.query_count;
    let groups = layout.trace_groups.len();
    let width = layout
        .trace_groups
        .iter()
        .map(|group| group.base_width + group.aux_width)
        .sum::<usize>();
    let frontier = |opened| {
        aggregate::maximum_multiproof_frontier_len_v1(layout.common_lde_size(), opened).unwrap()
    };
    let independently_counted = before
        - q * width * 8
        - 2 * groups * (frontier(2 * q) - frontier(q)) * 48
        - (groups - 1) * 2 * (frontier(q) + 1) * 48;
    let current = aggregate::maximum_encoded_proof_with_deep_bytes_v1(parameters, &shared).unwrap();
    assert_eq!(current, independently_counted);
    assert_eq!(
        aggregate::exact_deep_opening_bytes_v1(parameters, &shared).unwrap(),
        aggregate::exact_deep_opening_bytes_v1(parameters, &full_rows).unwrap()
    );
    current
}

#[test]
fn paired_fri_and_complete_relation_wire_bounds_are_source_derived() {
    let main = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let ca = AggregateProofLayoutV1::for_accumulators_v1().unwrap();
    assert_eq!(main.registered_segments.len(), 49);
    assert_eq!(main.trace_groups.len(), 6);
    assert_eq!(ca.trace_groups.len(), 1);
    let mut implemented_total = 0;
    for (layout, expected, saving) in [(&main, 7_908_768, 656_640), (&ca, 1_498_816, 210_816)] {
        let parameters = layout.parameters_v1();
        assert_eq!(
            parameters.fri_commitment_layout,
            aggregate::AggregateFriCommitmentLayoutV1::Paired
        );
        let shared = layout.as_shared().unwrap();
        let implemented =
            aggregate::maximum_encoded_proof_with_deep_bytes_v1(parameters, &shared).unwrap();
        let scalar = aggregate::maximum_encoded_proof_with_deep_bytes_v1(
            aggregate::AggregateStarkParametersV1 {
                fri_commitment_layout: aggregate::AggregateFriCommitmentLayoutV1::Scalar,
                ..parameters
            },
            &shared,
        )
        .unwrap();
        assert_eq!(implemented, expected);
        assert_eq!(scalar - implemented, saving);
        implemented_total += implemented;
    }
    let framing = super::super::profile::ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1 as usize
        + super::super::profile::ZK_X509_CA_CLAIM_ENVELOPE_BYTES_V1 as usize
        + super::super::credential_stark::ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1;
    assert_eq!(framing, 5_822);
    assert_eq!(
        implemented_total + framing,
        super::super::profile::ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize
    );
    assert_eq!(complete_oods_bytes(&main), 7_908_768);
    assert_eq!(complete_oods_bytes(&ca), 1_498_816);
    let candidate = complete_oods_bytes(&main) + complete_oods_bytes(&ca) + framing;
    assert_eq!(candidate, 9_413_406);
    assert_eq!(ZK_X509_MAX_PROOF_BYTES_V1 as usize - candidate, 23_778);
    assert_eq!(super::super::profile::validate_profile_v1(), Ok(()));
    // Fitting bytes is not independent crypto/resource qualification.
    assert!(!super::super::profile::zk_x509_activation_readiness_v1().is_complete());
}

#[test]
fn sha_polynomial_selector_degree_fits_the_unchanged_profile() {
    let main = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let sha = main
        .registered_segments
        .iter()
        .filter(|registration| registration.segment.adapter == SegmentAdapterIdV1::Sha256CallBus)
        .collect::<Vec<_>>();
    assert_eq!(sha.len(), 4);
    assert_eq!(ZK_X509_MAX_CONSTRAINT_DEGREE_V1, 7);
    assert_eq!(MASK_DEGREE, 1_815);
    assert_eq!(COMPOSITION_DEGREE_CHUNKS, 6);
    assert_eq!(main.parameters_v1().query_count, 136);
    for registration in sha {
        let segment = registration.segment;
        assert_eq!(segment.constraint_degree, 6);
        assert_eq!(segment.trace_log2, 19);
        let (quotient_degree, fri_degree) = checked_segment_degree_capacity_v1(
            segment.trace_log2,
            ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
            segment.constraint_degree,
        )
        .unwrap();
        assert_eq!(quotient_degree, 2_632_330);
        assert_eq!(fri_degree, 589_823);
        assert!(quotient_degree < COMPOSITION_DEGREE_CHUNKS * (fri_degree + 1));
    }
    assert_eq!(
        super::super::profile::ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1,
        9_413_406
    );
    assert_eq!(ZK_X509_MAX_PROOF_BYTES_V1, 9_437_184);
}

#[test]
fn binding_sink_optional_selection_degree_fits_the_unchanged_profile() {
    let main = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let mut count = 0;
    for registration in main.registered_segments {
        if registration.segment.adapter != SegmentAdapterIdV1::P256ValueBus
            || !matches!(
                p256_instance_parts_v1(registration.segment.instance),
                Some((_, 2))
            )
        {
            continue;
        }
        let segment = registration.segment;
        assert_eq!(segment.constraint_degree, 3);
        assert_eq!(segment.trace_log2, 16);
        let (quotient_degree, fri_degree) = checked_segment_degree_capacity_v1(
            segment.trace_log2,
            ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
            segment.constraint_degree,
        )
        .unwrap();
        assert_eq!(quotient_degree, 136_517);
        assert_eq!(fri_degree, 589_823);
        count += 1;
    }
    assert_eq!(count, 5);
    assert_eq!(ZK_X509_MAX_CONSTRAINT_DEGREE_V1, 7);
    assert_eq!(ZK_X509_MAX_PROOF_BYTES_V1, 9_437_184);
}

#[test]
fn joined_main_plan_retains_every_registered_column_and_native_group_slice() {
    use aggregate::joined_trace::{JoinedTraceColumnKindV1, JoinedTraceCommitmentPlanV1};
    let main = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let shared = main.as_shared().unwrap();
    let mut total = 0;
    for kind in [JoinedTraceColumnKindV1::Base, JoinedTraceColumnKindV1::Aux] {
        let plan =
            JoinedTraceCommitmentPlanV1::new_v1(main.parameters_v1(), &shared, kind).unwrap();
        let mut offset = 0;
        for (index, group) in main.trace_groups.iter().enumerate() {
            let width = match kind {
                JoinedTraceColumnKindV1::Base => group.base_width,
                JoinedTraceColumnKindV1::Aux => group.aux_width,
            };
            assert_eq!(plan.group_range_v1(index).unwrap(), offset..offset + width);
            offset += width;
        }
        assert_eq!(plan.width_v1(), offset);
        assert!(plan.group_range_v1(6).is_err());
        total += offset;
    }
    assert_eq!(total, 5_811);
    // The physical commitment is joined; all six logical groups remain exact.
    assert_eq!(shared.trace_groups().len(), 6);
    assert_eq!(shared.trace_commitment_count_v1(), 1);
    assert_eq!(
        aggregate::maximum_encoded_proof_with_deep_bytes_v1(main.parameters_v1(), &shared).unwrap(),
        7_908_768
    );
}

#[test]
fn private_terminal_frame_removal_is_exact_without_changing_proof_limits() {
    // Eight DER scalars, eight addressed RFC records, and the complete
    // addressed five-signature P-256 frame leave the public envelope.
    let removed_der = 8 * 8;
    let removed_rfc = 8 * (2 + 2 + 2 + 2 + 8);
    let removed_p256 = 12 + 348 * (2 + 2 + 2 + 2 + 8);
    assert_eq!(8 + 8 + 348, 364);
    assert_eq!(4 + 208, 212);
    assert_eq!(32 * 16, 512);
    assert_eq!(80 * 16, 1_280);
    assert_eq!(60 * 16, 960);
    assert_eq!(removed_der + removed_rfc + removed_p256, 5_772);
    assert_eq!(
        11_952 - removed_der - removed_rfc - removed_p256 - 60 * 16 - 32 * 16 - 80 * 16 + 31 * 32,
        super::super::profile::ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1 as usize
    );
    assert_eq!(ZK_X509_MAX_PROOF_BYTES_V1, 9_437_184);
    assert_eq!(COMPOSITION_DEGREE_CHUNKS, 6);
    assert_eq!(
        AggregateProofLayoutV1::for_full_profile_v1()
            .unwrap()
            .parameters_v1()
            .query_count,
        136
    );
}
