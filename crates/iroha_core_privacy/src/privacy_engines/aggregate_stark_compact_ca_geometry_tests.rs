//! The higher native-to-LDE ratio is confined to the exact private CA profile.

use super::*;

fn parameters_v1() -> AggregateStarkParametersV1 {
    AggregateStarkParametersV1 {
        proof_magic: *b"X5C2",
        proof_version: 1,
        fri_commitment_layout: AggregateFriCommitmentLayoutV1::Paired,
        security_lanes: 1,
        query_count: 136,
        blowup_log2: 4,
        terminal_log2: 10,
        terminal_degree_bound: 143,
        composition_degree_chunks: 4,
        minimum_trace_log2: 12,
        maximum_trace_log2: 12,
        maximum_trace_groups: 1,
        maximum_segment_instances: 13,
        maximum_base_columns_per_instance: 64,
        maximum_aux_columns_per_instance: 64,
        maximum_proof_bytes: 1_498_816,
    }
}
fn group_v1() -> AggregateTraceGroupLayoutV1 {
    AggregateTraceGroupLayoutV1 {
        native_trace_log2: 12,
        segment_instances: 13,
        base_width: 695,
        aux_width: 128,
    }
}
#[test]
fn compact_ca_private_padding_preserves_effective_rate_complete_wire_and_fri_limits() {
    let parameters = parameters_v1();
    assert!(parameters.compact_ca_private_padding_v1());
    let layout = AggregateProofLayoutV1::new_with_trace_layout_v1(
        parameters,
        vec![group_v1()],
        AggregateTraceLayoutV1::GroupedCurrent,
    )
    .unwrap();
    assert_eq!(layout.common_lde_size(), 65_536);
    assert_eq!(layout.common_lde_log2(), 16);
    assert_eq!(group_v1().next_stride(16).unwrap(), 16);
    assert_eq!(layout.fri_rounds(parameters).unwrap(), 6);
    assert_eq!(layout.fri_degree_cap(parameters).unwrap(), 9_216);
    assert_eq!(
        layout.fri_degree_cap(parameters).unwrap() * 64,
        layout.common_lde_size() * 9
    );
    assert_eq!(
        layout.fri_mask_coefficient_count(parameters).unwrap(),
        9_215
    );
    assert_eq!(
        maximum_encoded_proof_with_deep_bytes_v1(parameters, &layout).unwrap(),
        1_498_816
    );
    assert_eq!(parameters.maximum_segment_instances, 13); // Trace commitment chunks.
    assert_eq!(parameters.composition_degree_chunks, 4); // Quotient chunks.
}
#[test]
fn every_changed_ca_parameter_rejects_the_higher_blowup_exception() {
    for index in 0..17 {
        let mut parameters = parameters_v1();
        match index {
            0 => parameters.proof_magic = *b"X5T1",
            1 => parameters.proof_version = 2,
            2 => parameters.fri_commitment_layout = AggregateFriCommitmentLayoutV1::Scalar,
            3 => parameters.security_lanes = 2,
            4 => parameters.query_count = 137,
            5 => parameters.blowup_log2 = 3,
            6 => parameters.terminal_log2 = 11,
            7 => parameters.terminal_degree_bound = 144,
            8 => parameters.composition_degree_chunks = 5,
            9 => parameters.minimum_trace_log2 = 11,
            10 => parameters.maximum_trace_log2 = 13,
            11 => parameters.maximum_trace_groups = 2,
            12 => parameters.maximum_segment_instances = 14,
            13 => parameters.maximum_base_columns_per_instance = 65,
            14 => parameters.maximum_aux_columns_per_instance = 65,
            15 => parameters.maximum_proof_bytes += 1,
            _ => parameters.blowup_log2 = 5,
        }
        assert!(!parameters.compact_ca_private_padding_v1());
        assert_eq!(
            parameters.validate(),
            Err(AggregateStarkErrorV1::InvalidLayout),
            "parameter mutation {index}"
        );
    }
}
#[test]
fn every_altered_ca_original_column_layout_or_query_form_is_rejected() {
    let parameters = parameters_v1();
    for index in 0..5 {
        let mut group = group_v1();
        match index {
            0 => group.native_trace_log2 = 13,
            1 => group.segment_instances = 12,
            2 => group.base_width = 694,
            3 => group.aux_width = 127,
            _ => group.aux_width = 129,
        }
        assert!(
            AggregateProofLayoutV1::new_with_trace_layout_v1(
                parameters,
                vec![group],
                AggregateTraceLayoutV1::GroupedCurrent
            )
            .is_err()
        );
    }
    for trace_layout in [
        AggregateTraceLayoutV1::GroupedCurrentNext,
        AggregateTraceLayoutV1::JoinedCurrent,
    ] {
        assert!(
            AggregateProofLayoutV1::new_with_trace_layout_v1(
                parameters,
                vec![group_v1()],
                trace_layout
            )
            .is_err()
        );
    }
    assert!(
        AggregateProofLayoutV1::new_with_trace_layout_v1(
            parameters,
            vec![group_v1(), group_v1()],
            AggregateTraceLayoutV1::GroupedCurrent
        )
        .is_err()
    );
    let mut layout = AggregateProofLayoutV1::new_with_trace_layout_v1(
        parameters,
        vec![group_v1()],
        AggregateTraceLayoutV1::GroupedCurrent,
    )
    .unwrap();
    layout.common_lde_log2 = 17;
    assert!(layout.validate(parameters).is_err());
}
