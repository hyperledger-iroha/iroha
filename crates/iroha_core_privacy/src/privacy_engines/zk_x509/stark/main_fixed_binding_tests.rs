//! Controls for the verifier-fixed table digests and their evaluator ties.

use super::*;
use iroha_data_model::privacy::{PrivacyConsensusLimitsV1, PrivacyRuntimeStatementV1};
use std::collections::BTreeMap;

fn sample_row_v1(index: usize) -> [F; 3] {
    let index = index as u64;
    [
        F(index),
        F(index.wrapping_mul(7) % 11),
        F(u64::from(index % 3 == 0)),
    ]
}

/// Rebuild the table framing from its definition, with separately assembled bytes.
fn independent_table_digest_v1(label: &[u8], trace_log2: u8, rows: &[[F; 3]]) -> [u8; 48] {
    let frame = |phase: &[u8], index: u64, fields: &[&[u8]]| {
        privacy_outer_digest_frame_v1(
            ZK_X509_FIXED_DIGEST_CONTEXT_V1,
            b"zk-x509-verifier-fixed-binding",
            phase,
            0,
            index,
            0,
            fields,
        )
        .unwrap()
        .to_bytes()
    };
    let mut blocks = Vec::new();
    for (index, chunk) in rows.chunks(4_096).enumerate() {
        let bytes = chunk
            .iter()
            .flatten()
            .flat_map(|value| value.0.to_be_bytes())
            .collect::<Vec<_>>();
        blocks.extend_from_slice(&frame(b"table-block", index as u64, &[label, &bytes]));
    }
    let mut header = vec![trace_log2];
    header.extend_from_slice(&3_u32.to_be_bytes());
    frame(
        b"table",
        0,
        &[
            ZK_X509_VERIFIER_FIXED_BINDING_DESCRIPTOR_V1,
            label,
            &header,
            &blocks,
        ],
    )
}

#[test]
fn table_digest_matches_an_independent_framing_and_observes_every_row_once() {
    for trace_log2 in [5_u8, 13] {
        let rows = (0..1_usize << trace_log2)
            .map(sample_row_v1)
            .collect::<Vec<_>>();
        let mut observed = Vec::new();
        let digest = table_digest_v1(
            b"sample",
            trace_log2,
            3,
            |index| Ok(sample_row_v1(index)),
            |index, values| {
                assert_eq!(values, sample_row_v1(index));
                observed.push(index);
            },
        )
        .unwrap();
        assert_eq!(
            digest,
            independent_table_digest_v1(b"sample", trace_log2, &rows)
        );
        assert_eq!(observed, (0..rows.len()).collect::<Vec<_>>());
    }
}

#[test]
fn table_digest_moves_with_any_cell_row_order_label_or_width() {
    let digest = |label: &[u8], changed: Option<usize>, swapped: bool| {
        table_digest_v1(
            label,
            13,
            3,
            |index| {
                let source = match (swapped, index) {
                    (true, 100) => 101,
                    (true, 101) => 100,
                    _ => index,
                };
                let mut row = sample_row_v1(source);
                if changed == Some(index) {
                    row[2] = row[2].add(F::ONE);
                }
                Ok(row)
            },
            |_, _| {},
        )
        .unwrap()
    };
    let canonical = digest(b"sample", None, false);
    for (index, other) in [
        digest(b"sample", Some(0), false),
        digest(b"sample", Some(4_095), false),
        digest(b"sample", Some(8_191), false),
        digest(b"sample", None, true),
        digest(b"samplf", None, false),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(other, canonical, "variant {index}");
    }
    let wider = table_digest_v1(
        b"sample",
        13,
        4,
        |index| {
            let row = sample_row_v1(index);
            Ok([row[0], row[1], row[2], F::ZERO])
        },
        |_, _| {},
    )
    .unwrap();
    assert_ne!(wider, canonical);
    assert_eq!(
        table_digest_v1(
            b"sample",
            13,
            4,
            |index| Ok(sample_row_v1(index)),
            |_, _| {}
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    );
}

#[test]
fn der_tie_accepts_the_native_table_and_rejects_one_changed_cell() {
    let weights = tie_weights_v1().unwrap();
    let der = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1);
    let native = der_digest_v1(&weights, |row| {
        der.fixed_row(row)
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)
    })
    .unwrap();
    assert_eq!(
        native,
        zk_x509_verifier_fixed_binding_digests_v1().unwrap()[0]
    );
    // A table the closed forms do not evaluate fails, whatever its digest.
    for (row, column) in [
        (0, FIX_PARSER),
        (300_000, FIX_PADDING),
        (524_287, DER_FIX_ACTIVE),
    ] {
        let result = der_digest_v1(&weights, |index| {
            let mut values = der
                .fixed_row(index)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            if index == row {
                values[column] = F::ONE.sub(values[column]);
            }
            Ok(values)
        });
        assert_eq!(
            result,
            Err(ZkX509StarkErrorV1::ProfileMismatch),
            "{row} {column}"
        );
    }
}

#[test]
fn affine_tie_accepts_a_reference_schedule_and_rejects_a_wrong_translate() {
    let weights = tie_weights_v1().unwrap();
    let (prefix, linear) = main_log19_weight_prefixes_v1(&weights[0]).unwrap();
    let statement = rfc_reference_statement_v1(4).unwrap();
    let rfc_fixed = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(
        ZkX509Rfc5280StarkShapeV1::from_statement(&statement).unwrap(),
    )
    .unwrap();
    let sha_fixed = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let schedule =
        MainLog19PublicFixedAffineScheduleV1::compile_v1(&rfc_fixed, &sha_fixed).unwrap();
    affine_tie_v1(&schedule, &weights, &prefix, &linear).unwrap();
    // Direct next-row values at z instead of omega * z must not pass.
    let current_only = [weights[0].clone(), weights[0].clone()];
    assert_eq!(
        affine_tie_v1(&schedule, &current_only, &prefix, &linear),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    );
}

#[test]
fn reference_statements_are_valid_and_cover_every_shape() {
    let mut eku_counts = Vec::new();
    let mut crl_numbers = Vec::new();
    for disclosures in 0..DISCLOSURE_SHAPES_V1 {
        let rfc = rfc_reference_statement_v1(disclosures).unwrap();
        assert_eq!(rfc.disclosed_attribute_indices.len(), disclosures);
        assert!(
            rfc.presentation_not_after_unix_seconds - rfc.presentation_not_before_unix_seconds
                <= 300
        );
        ZkX509Rfc5280StarkShapeV1::from_statement(&rfc).unwrap();
        eku_counts.push(rfc.leaf_extended_key_usages.len());
        crl_numbers.push(rfc.crl_number);
        let statement = reference_statement_v1(disclosures).unwrap();
        PrivacyRuntimeStatementV1::IrohaZkX509StarkP256V1(statement.clone())
            .validate(&PrivacyConsensusLimitsV1::taira_default())
            .unwrap();
        assert_eq!(statement.disclosed_attributes.len(), disclosures);
        assert_eq!(
            statement.extended_key_usages.len(),
            rfc.leaf_extended_key_usages.len()
        );
    }
    assert_eq!(eku_counts, [1, 1, 2, 2, 3]);
    crl_numbers.sort_unstable();
    crl_numbers.dedup();
    assert_eq!(crl_numbers.len(), DISCLOSURE_SHAPES_V1);
    assert!(rfc_reference_statement_v1(DISCLOSURE_SHAPES_V1).is_none());
}

#[test]
fn binding_digests_are_deterministic_distinct_and_cached() {
    let first = *zk_x509_verifier_fixed_binding_digests_v1().unwrap();
    assert_eq!(compute_verifier_fixed_binding_digests_v1().unwrap(), first);
    for (index, digest) in first.iter().enumerate() {
        assert!(
            first[index + 1..].iter().all(|other| other != digest),
            "table digest {index} repeats"
        );
    }
}

/// Add one to cell `(row, column)` of a canonical affine schedule, keeping it
/// sorted and non-overlapping.
pub(super) fn increment_affine_cell_v1(
    segments: &mut Vec<MainLog19PublicFixedAffineSegmentV1>,
    column: usize,
    row: usize,
) {
    let column = u16::try_from(column).expect("affine column fits u16");
    let row = u32::try_from(row).expect("affine row fits u32");
    let covering = segments
        .iter()
        .position(|segment| segment.column == column && segment.start <= row && row < segment.end);
    let Some(index) = covering else {
        let at =
            segments.partition_point(|segment| (segment.column, segment.start) < (column, row));
        segments.insert(
            at,
            MainLog19PublicFixedAffineSegmentV1 {
                column,
                start: row,
                end: row + 1,
                start_value: F::ONE,
                step: F::ZERO,
            },
        );
        return;
    };
    let segment = segments.remove(index);
    let value_at = |at: u32| {
        segment
            .start_value
            .add(segment.step.mul(F(u64::from(at - segment.start))))
    };
    let mut pieces = Vec::new();
    if segment.start < row {
        pieces.push(MainLog19PublicFixedAffineSegmentV1 {
            end: row,
            ..segment
        });
    }
    pieces.push(MainLog19PublicFixedAffineSegmentV1 {
        column,
        start: row,
        end: row + 1,
        start_value: value_at(row).add(F::ONE),
        step: F::ZERO,
    });
    if row + 1 < segment.end {
        pieces.push(MainLog19PublicFixedAffineSegmentV1 {
            start: row + 1,
            start_value: value_at(row + 1),
            ..segment
        });
    }
    segments.splice(index..index, pieces);
}

#[test]
fn every_verifier_fixed_source_is_bound_by_a_compiled_profile_field() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let mut fields = BTreeMap::<&str, usize>::new();
    let mut p256_members = Vec::new();
    for (position, registration) in layout.registered_segments.iter().copied().enumerate() {
        let evaluator = MainFp4AirEvaluatorV1::for_registration_v1(registration)
            .unwrap()
            .expect("prepare_main_deep_fixed_v1 evaluates every registration");
        // Exhaustive by design: a new fixed source does not compile here until
        // it names the compiled-profile field that binds it.
        let field = match evaluator {
            MainFp4AirEvaluatorV1::Sha(_) => "22-26 SHA atom schedules",
            MainFp4AirEvaluatorV1::P256(_)
                if registration.segment.trace_log2 == ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 =>
            {
                "28 P-256 log-19 atom schedule"
            }
            MainFp4AirEvaluatorV1::P256(_) => {
                p256_members.push(position);
                "34 P-256 log-5, log-8 and log-16 tables"
            }
            MainFp4AirEvaluatorV1::StrictDer(_) => "30 DER table and closed forms",
            MainFp4AirEvaluatorV1::Rfc5280(_) => "31 RFC 5280 affine schedules",
            MainFp4AirEvaluatorV1::ByteMemory(_) => "32 IO tables",
            MainFp4AirEvaluatorV1::Projection(_) => "33 Projection tables",
        };
        *fields.entry(field).or_default() += 1;
    }
    assert_eq!(
        fields.into_iter().collect::<Vec<_>>(),
        [
            ("22-26 SHA atom schedules", 4),
            ("28 P-256 log-19 atom schedule", 15),
            ("30 DER table and closed forms", 1),
            ("31 RFC 5280 affine schedules", 1),
            ("32 IO tables", 1),
            ("33 Projection tables", 1),
            ("34 P-256 log-5, log-8 and log-16 tables", 26),
        ]
    );
    // The P-256 family digests exactly the registrations the census found,
    // in layout order.
    let expected = p256_members
        .iter()
        .map(|position| p256_label_v1(*position, layout.registered_segments[*position]).unwrap())
        .collect::<Vec<_>>();
    let digested = p256_members_v1(&layout).unwrap();
    assert_eq!(digested.len(), expected.len());
    for ((label, _), expected) in digested.iter().zip(&expected) {
        assert_eq!(label, expected);
    }
    let controls = zk_x509_verifier_fixed_binding_cell_controls_v1().unwrap();
    assert!(expected.contains(&controls[4].1));
    // The CA subproof evaluates one constant table, field 35's.
    assert_eq!(compile_ca_accumulator_fixed_columns_v1().unwrap().len(), 80);
    assert_eq!(ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1, 12);
    // One control per bound table, in field order.
    assert_eq!(
        controls.iter().map(|control| control.0).collect::<Vec<_>>(),
        (0..ZK_X509_VERIFIER_FIXED_BINDING_DIGEST_COUNT_V1).collect::<Vec<_>>()
    );
}

#[test]
fn one_changed_cell_moves_every_bound_table_digest() {
    let current = *zk_x509_verifier_fixed_binding_digests_v1().unwrap();
    for (table, label, row, column) in zk_x509_verifier_fixed_binding_cell_controls_v1().unwrap() {
        let changed =
            zk_x509_verifier_fixed_binding_digest_with_changed_cell_v1(table, &label, row, column);
        if table == 0 {
            // The DER closed forms no longer evaluate the changed table.
            assert_eq!(changed, Err(ZkX509StarkErrorV1::ProfileMismatch));
            let der = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1);
            let rows = |shifted: bool| {
                table_digest_v1(
                    b"der",
                    ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
                    ZK_X509_DER_STARK_FIXED_WIDTH_V1,
                    |index| {
                        let mut values = der.fixed_row(index).unwrap();
                        if shifted && index == row {
                            values[column] = values[column].add(F::ONE);
                        }
                        Ok(values)
                    },
                    |_, _| {},
                )
                .unwrap()
            };
            assert_ne!(rows(true), rows(false));
            assert_eq!(family_v1(b"der", &[rows(false)]).unwrap(), current[0]);
        } else {
            assert_ne!(changed.unwrap(), current[table], "table {table}");
        }
    }
    // The hook is cleared: recomputing gives the cached digests again.
    assert_eq!(
        compute_verifier_fixed_binding_digests_v1().unwrap(),
        current
    );
}

#[test]
fn affine_cell_increment_changes_exactly_one_cell() {
    let weights = tie_weights_v1().unwrap();
    let (prefix, linear) = main_log19_weight_prefixes_v1(&weights[0]).unwrap();
    let statement = rfc_reference_statement_v1(1).unwrap();
    let rfc_fixed = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(
        ZkX509Rfc5280StarkShapeV1::from_statement(&statement).unwrap(),
    )
    .unwrap();
    let sha_fixed = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 1,
    })
    .unwrap();
    let original =
        MainLog19PublicFixedAffineScheduleV1::compile_v1(&rfc_fixed, &sha_fixed).unwrap();
    let value = |segments: &[MainLog19PublicFixedAffineSegmentV1], column: u16, row: u32| {
        segments
            .iter()
            .filter(|segment| segment.column == column && segment.start <= row && row < segment.end)
            .fold(F::ZERO, |sum, segment| {
                sum.add(
                    segment
                        .start_value
                        .add(segment.step.mul(F(u64::from(row - segment.start)))),
                )
            })
    };
    let covered = original.segments[original.segments.len() / 2];
    for (column, row) in [
        (covered.column, covered.start),
        (covered.column, covered.end - 1),
        (0, 524_287),
    ] {
        let mut changed = MainLog19PublicFixedAffineScheduleV1 {
            segments: original.segments.clone(),
        };
        increment_affine_cell_v1(
            &mut changed.segments,
            usize::from(column),
            usize::try_from(row).unwrap(),
        );
        changed.validate_v1().unwrap();
        assert_eq!(
            value(&changed.segments, column, row),
            value(&original.segments, column, row).add(F::ONE)
        );
        for probe in [row.saturating_sub(1), row.saturating_add(1).min(524_287)] {
            if probe != row {
                assert_eq!(
                    value(&changed.segments, column, probe),
                    value(&original.segments, column, probe)
                );
            }
        }
        affine_tie_v1(&changed, &weights, &prefix, &linear).unwrap();
    }
}

#[test]
#[ignore = "one-time cost diagnostic: prints per-table seconds for the verifier-fixed digests"]
fn verifier_fixed_binding_one_time_cost() {
    let start = std::time::Instant::now();
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let mut lap = std::time::Instant::now();
    let mut report = |name: &str| {
        eprintln!(
            "zk-x509-verifier-fixed-binding-seconds-{name}={:.3}",
            lap.elapsed().as_secs_f64()
        );
        lap = std::time::Instant::now();
    };
    let weights = tie_weights_v1().unwrap();
    report("tie-weights");
    let der = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1);
    let der = der_digest_v1(&weights, |row| {
        der.fixed_row(row)
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)
    })
    .unwrap();
    report("der");
    let rfc = rfc_digest_v1(&weights).unwrap();
    report("rfc5280");
    drop(weights);
    let io = io_digest_v1(&layout).unwrap();
    report("io");
    let projection = projection_digest_v1(&layout).unwrap();
    report("projection");
    let p256 = p256_digest_v1(&layout).unwrap();
    report("p256");
    let ca = ca_digest_v1().unwrap();
    report("ca");
    assert_eq!(
        [der, rfc, io, projection, p256, ca],
        compute_verifier_fixed_binding_digests_v1().unwrap()
    );
    eprintln!(
        "zk-x509-verifier-fixed-binding-seconds-total-including-recheck={:.3}",
        start.elapsed().as_secs_f64()
    );
    for (name, digest) in [
        ("der", der),
        ("rfc5280-public-affine", rfc),
        ("io", io),
        ("projection", projection),
        ("p256-log5-8-16", p256),
        ("ca", ca),
    ] {
        eprintln!(
            "zk-x509-verifier-fixed-binding-{name}={}",
            hex::encode(digest)
        );
    }
}
