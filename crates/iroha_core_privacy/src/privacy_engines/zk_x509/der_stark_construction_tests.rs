//! Producer state snapshots must satisfy the unchanged numeric DER relation.

use super::*;

fn construction_challenges() -> ZkX509DerStarkChallengesV1 {
    ZkX509DerStarkChallengesV1 {
        tuple: core::array::from_fn(|lane| {
            core::array::from_fn(|column| F((1_000 + lane * 100 + column) as u64))
        }),
        byte_lookup: [F(9_001), F(9_002), F(9_003), F(9_004)],
    }
}

fn assert_complete_numeric_construction(documents: &[&[u8]]) {
    let base = build_zk_x509_der_stark_base_v1(documents).expect("valid numeric base");
    validate_zk_x509_der_stark_base_trace_v1(&base).expect("every base transition");
    let challenges = construction_challenges();
    let trace = build_zk_x509_der_stark_trace_v1(base, challenges).expect("complete bus closure");
    let claims = zk_x509_der_stark_terminal_claims_v1(&trace).unwrap();
    let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1).unwrap();
    let comparator_end =
        ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + trace.base.private_shape.comparator_rows;
    let indices = (0..=trace.base.private_shape.parser_rows)
        .chain(ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1..=comparator_end)
        .chain([
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
            ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
        ]);
    let mut residues = Vec::with_capacity(ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1);
    for index in indices {
        let next = (index + 1) % ZK_X509_DER_STARK_TRACE_SIZE_V1;
        evaluate_zk_x509_der_stark_residues_into_v1(
            &zk_x509_der_stark_aggregate_base_row_v1(&trace.base, index).unwrap(),
            &zk_x509_der_stark_aggregate_base_row_v1(&trace.base, next).unwrap(),
            &zk_x509_der_stark_aggregate_aux_row_v1(&trace, index).unwrap(),
            &zk_x509_der_stark_aggregate_aux_row_v1(&trace, next).unwrap(),
            &schedule.fixed_row(index).unwrap(),
            &schedule.fixed_row(next).unwrap(),
            challenges,
            ZkX509DerStarkPublicTerminalsV1,
            claims,
            &mut residues,
        )
        .unwrap();
        assert_eq!(residues.len(), ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1);
        assert!(
            residues.iter().all(|value| *value == F::ZERO),
            "synthetic fixture row {index} violates the complete DER AIR"
        );
    }
}

#[test]
fn long_length_rows_snapshot_state_before_consuming_the_length_octet() {
    for length in [127_usize, 128, 255, 256, 4_092] {
        for tag in [&[0x04][..], &[0x9f, 0x1f][..]] {
            // The high-tag variant uses one extra byte, so keep it inside the
            // same unchanged 4096-byte proof-facing document cap.
            if length == 4_092 && tag.len() == 2 {
                continue;
            }
            let mut document = tag.to_vec();
            match length {
                0..=127 => document.push(length as u8),
                128..=255 => document.extend_from_slice(&[0x81, length as u8]),
                _ => document.extend_from_slice(&[0x82, (length >> 8) as u8, length as u8]),
            }
            document.resize(document.len() + length, 0x5a);
            assert_complete_numeric_construction(&[&document]);
            if length >= 128 {
                let mut broken = build_zk_x509_der_stark_base_v1(&[&document]).unwrap();
                let first_length = tag.len();
                assert!(
                    broken.rows[first_length][BASE_PAYLOAD..BASE_PAYLOAD + 4]
                        .iter()
                        .all(|value| *value == F::ZERO)
                );
                // Recreate the former producer bug; do not relax its rejection.
                let count = if length <= 255 { 1 } else { 2 };
                write_bits_v1(&mut broken.rows[first_length], BASE_PAYLOAD, 2, count);
                broken.rows[first_length][BASE_PAYLOAD + 2] = F(u64::from(count == 2));
                assert_eq!(
                    validate_zk_x509_der_stark_base_trace_v1(&broken),
                    Err(ZkX509DerStarkErrorV1::Transition)
                );
            }
        }
    }
}

#[test]
fn oid_and_bit_string_flags_end_before_the_boundary_row() {
    for primitive in [
        vec![0x06, 0x03, 0x2a, 0x86, 0x48],
        vec![0x03, 0x02, 0x03, 0xa8],
    ] {
        for nested in [false, true] {
            let document = if nested {
                let mut encoded = vec![0x30, primitive.len() as u8];
                encoded.extend_from_slice(&primitive);
                encoded
            } else {
                primitive.clone()
            };
            assert_complete_numeric_construction(&[&document]);
            let base = build_zk_x509_der_stark_base_v1(&[&document]).unwrap();
            let boundary = base
                .rows
                .windows(2)
                .position(|pair| {
                    pack_bits_v1(&pair[0][BASE_PHASE_BITS..BASE_PHASE_BITS + 3])
                        == F(PHASE_PRIMITIVE_CONTENT as u64)
                        && pack_bits_v1(&pair[1][BASE_PHASE_BITS..BASE_PHASE_BITS + 3])
                            == F(PHASE_BOUNDARY as u64)
                })
                .unwrap()
                + 1;
            for column in [BASE_PRIMITIVE_FIRST, BASE_OID_START, BASE_UNUSED_BITS] {
                assert_eq!(base.rows[boundary][column], F::ZERO);
                let mut broken = base.clone();
                broken.rows[boundary][column] = F::ONE;
                assert_eq!(
                    validate_zk_x509_der_stark_base_trace_v1(&broken),
                    Err(ZkX509DerStarkErrorV1::Transition)
                );
            }
        }
    }
}

#[test]
fn ordinary_and_maximum_release_documents_satisfy_complete_numeric_der_air() {
    use super::super::relation::release_fixture::{
        build_zk_x509_release_fixture_v1, reference_statement_context_v1,
    };
    for maximum in [false, true] {
        let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), maximum)
            .expect("actual release fixture");
        if maximum {
            assert_eq!(fixture.witness.certificate_chain_der.len(), 3);
            assert_eq!(fixture.crl_entry_count, 64);
            assert_eq!(fixture.resource_shape.maximum_serial_bytes, 20);
            assert_eq!(fixture.statement.disclosed_attributes.len(), 4);
        }
        let mut documents = fixture
            .witness
            .certificate_chain_der
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        documents.push(&fixture.witness.crl_der);
        assert_complete_numeric_construction(&documents);
    }
}

#[test]
fn inactive_local_auxiliary_template_matches_generic_witness_equations() {
    let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1).unwrap();
    let null = [0x05, 0x00];
    for count in 1..=ZK_X509_DER_STARK_MAX_DOCUMENTS_V1 {
        let documents = vec![null.as_slice(); count];
        let base = build_zk_x509_der_stark_base_v1(&documents).unwrap();
        let inactive =
            zk_x509_der_stark_aggregate_base_row_v1(&base, base.private_shape.parser_rows).unwrap();
        assert_eq!(inactive[BASE_ROW_ACTIVE], F::ZERO);
        assert_eq!(inactive[BASE_FINAL_DOCUMENT], F((count - 1) as u64));
        // Exercise every distinct public fixed-row boundary, including parser,
        // comparator and aggregate padding. Private inactivity must dominate
        // those public selectors in the unchanged witness equations.
        for index in [
            0,
            1,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
            ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
        ] {
            let mut expected = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
            populate_low_degree_auxiliaries_v1(
                &inactive,
                &schedule.fixed_row(index).unwrap(),
                &mut expected,
            )
            .unwrap();
            assert_eq!(
                DER_INACTIVE_LOW_DEGREE_AUXILIARIES_V1, expected,
                "document count {count}, fixed boundary {index}"
            );
        }
    }
    for (value, column) in [(64, AUX_BYTE_64_INVERSE), (128, AUX_BYTE_128_INVERSE)] {
        assert_eq!(
            F::ZERO
                .sub(F(value))
                .mul(DER_INACTIVE_LOW_DEGREE_AUXILIARIES_V1[column]),
            F::ONE
        );
    }
}

#[test]
fn inactive_auxiliary_reconstruction_preserves_active_rows_and_both_carry_domains() {
    for document in [&[0x05, 0x00][..], &[0x31, 0x04, 0x05, 0x00, 0x05, 0x00][..]] {
        let base = build_zk_x509_der_stark_base_v1(&[document]).unwrap();
        let mut trace = build_zk_x509_der_stark_trace_v1(base, construction_challenges()).unwrap();
        let parser_end = trace.base.private_shape.parser_rows;
        let comparator_rows = trace.base.private_shape.comparator_rows;
        assert_eq!(comparator_rows > 0, document[0] == 0x31);
        // Distinct public synthetic values expose any lost or cross-domain carry.
        // This tests the adapter's cell copying, not validity of a modified trace.
        for (index, row) in trace.aux_rows.iter_mut().enumerate() {
            for (column, cell) in row.iter_mut().enumerate() {
                *cell = F((1 + index * ZK_X509_DER_STARK_AUX_WIDTH_V1 + column) as u64);
            }
        }
        for index in 0..parser_end {
            assert_eq!(
                zk_x509_der_stark_aggregate_aux_row_v1(&trace, index).unwrap(),
                trace.aux_rows[index]
            );
        }
        for index in 0..comparator_rows {
            assert_eq!(
                zk_x509_der_stark_aggregate_aux_row_v1(
                    &trace,
                    ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + index
                )
                .unwrap(),
                trace.aux_rows[parser_end + index]
            );
        }
        let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1).unwrap();
        for index in [
            parser_end,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + comparator_rows,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
            ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
        ] {
            let carry = if index < ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 {
                &trace.aux_rows[parser_end - 1]
            } else {
                trace.aux_rows.last().unwrap()
            };
            let mut expected = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
            populate_low_degree_auxiliaries_v1(
                &zk_x509_der_stark_aggregate_base_row_v1(&trace.base, index).unwrap(),
                &schedule.fixed_row(index).unwrap(),
                &mut expected,
            )
            .unwrap();
            for (before, after) in [
                (AUX_STACK_PUSH_BEFORE, AUX_STACK_PUSH_AFTER),
                (AUX_STACK_POP_BEFORE, AUX_STACK_POP_AFTER),
                (AUX_DOCUMENT_BEFORE, AUX_DOCUMENT_AFTER),
                (AUX_NODE_BEFORE, AUX_NODE_AFTER),
                (AUX_PAIR_PRODUCER_BEFORE, AUX_PAIR_PRODUCER_AFTER),
                (AUX_PAIR_CONSUMER_BEFORE, AUX_PAIR_CONSUMER_AFTER),
                (AUX_BYTE_TABLE_SUM_BEFORE, AUX_BYTE_TABLE_SUM_AFTER),
                (AUX_BYTE_QUERY_SUM_BEFORE, AUX_BYTE_QUERY_SUM_AFTER),
                (
                    AUX_BYTE_TABLE_ZERO_COUNT_BEFORE,
                    AUX_BYTE_TABLE_ZERO_COUNT_AFTER,
                ),
                (
                    AUX_BYTE_QUERY_ZERO_COUNT_BEFORE,
                    AUX_BYTE_QUERY_ZERO_COUNT_AFTER,
                ),
                (AUX_INPUT_BYTE_BEFORE, AUX_INPUT_BYTE_AFTER),
            ] {
                for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
                    expected[before + lane] = carry[after + lane];
                    expected[after + lane] = carry[after + lane];
                }
            }
            assert_eq!(
                zk_x509_der_stark_aggregate_aux_row_v1(&trace, index).unwrap(),
                expected,
                "native padding index {index}"
            );
        }
    }
}

#[test]
fn inactive_auxiliary_template_keeps_shape_row_and_resource_errors() {
    let document = [0x05, 0x00];
    let base = build_zk_x509_der_stark_base_v1(&[&document]).unwrap();
    let trace = build_zk_x509_der_stark_trace_v1(base, construction_challenges()).unwrap();
    let padding_index = trace.base.private_shape.parser_rows;
    assert_eq!(
        zk_x509_der_stark_aggregate_aux_row_v1(&trace, ZK_X509_DER_STARK_TRACE_SIZE_V1),
        Err(ZkX509DerStarkErrorV1::Resource)
    );
    let mut missing_document = trace.clone();
    missing_document.base.private_shape.document_lengths.clear();
    assert_eq!(
        zk_x509_der_stark_aggregate_aux_row_v1(&missing_document, padding_index),
        Err(ZkX509DerStarkErrorV1::Shape)
    );
    let mut missing_parser = trace.clone();
    missing_parser.base.private_shape.parser_rows = 0;
    assert_eq!(
        zk_x509_der_stark_aggregate_aux_row_v1(&missing_parser, 0),
        Err(ZkX509DerStarkErrorV1::Shape)
    );
    let mut missing_carry = trace.clone();
    missing_carry.aux_rows.clear();
    assert_eq!(
        zk_x509_der_stark_aggregate_aux_row_v1(&missing_carry, padding_index),
        Err(ZkX509DerStarkErrorV1::Row)
    );
    assert_eq!(
        zk_x509_der_stark_aggregate_aux_row_v1(
            &missing_carry,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1
        ),
        Err(ZkX509DerStarkErrorV1::Shape)
    );
    assert_eq!(
        zk_x509_der_stark_aggregate_aux_row_v1(&missing_carry, 0),
        Err(ZkX509DerStarkErrorV1::Row)
    );
}
