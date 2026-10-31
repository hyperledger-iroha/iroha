//! Initial immutable output parity, mask refusal, clearing and complete-ledger checks.

use super::*;
use aggregate::initial_evaluated_column::{
    InitialEvaluatedColumnV1 as Evaluated, InitialEvaluationV1,
};
use aggregate::joined_trace::{
    JoinedTraceColumnKindV1 as Kind, JoinedTraceCommitmentPlanV1 as Plan,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

const COMMON_LOG: u8 = 12;
const ROWS: usize = 1 << COMMON_LOG;

fn evaluator(device_columns: usize) -> MainTraceCosetEvaluatorV1 {
    MainTraceCosetEvaluatorV1 {
        evaluation_rows: ROWS,
        device_columns,
        receipt: MainTransformReceiptV1::default(),
        coset_powers: (device_columns != 0)
            .then(|| PublicCosetPowersV1::new_v1((1 << 9) + MASK_DEGREE + 1).unwrap()),
        selected_scratch: selected_coset::SelectedScratchV1::default(),
    }
}

fn fixture(width: usize) -> (Plan, Plan, aggregate::AggregateStarkDomainsV1) {
    let parameters = aggregate::AggregateStarkParametersV1 {
        proof_magic: *b"ROI1",
        proof_version: 1,
        fri_commitment_layout: aggregate::AggregateFriCommitmentLayoutV1::Paired,
        security_lanes: 2,
        query_count: fastpq_isi::FASTPQ_QUERY_COUNT_V1 as usize,
        blowup_log2: 3,
        terminal_log2: 3,
        terminal_degree_bound: 7,
        composition_degree_chunks: 3,
        minimum_trace_log2: 8,
        maximum_trace_log2: 9,
        maximum_trace_groups: 2,
        maximum_segment_instances: 2,
        maximum_base_columns_per_instance: 16,
        maximum_aux_columns_per_instance: 16,
        maximum_proof_bytes: 1 << 20,
    };
    let layout = aggregate::AggregateProofLayoutV1::new(
        parameters,
        vec![
            aggregate::AggregateTraceGroupLayoutV1 {
                native_trace_log2: 8,
                segment_instances: 1,
                base_width: width,
                aux_width: width,
            },
            aggregate::AggregateTraceGroupLayoutV1 {
                native_trace_log2: 9,
                segment_instances: 1,
                base_width: 1,
                aux_width: 1,
            },
        ],
    )
    .unwrap();
    assert_eq!(layout.common_lde_log2(), COMMON_LOG);
    let domains = aggregate::AggregateStarkDomainsV1 {
        digest_context:
            crate::privacy_engines::transparent_stark::TransparentStarkDigestContextV1::new(
                iroha_data_model::privacy::PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
                b"initial-readonly-tests-v1",
            ),
        base_leaf: b"readonly-base-leaf",
        base_node: b"readonly-base-node",
        aux_leaf: b"readonly-aux-leaf",
        aux_node: b"readonly-aux-node",
        composition_leaf: b"readonly-composition-leaf",
        composition_node: b"readonly-composition-node",
        fri_leaf: b"readonly-fri-leaf",
        fri_node: b"readonly-fri-node",
        layout_label: b"readonly-layout",
        base_root_label: b"readonly-base-root",
        aux_root_label: b"readonly-aux-root",
        composition_root_label: b"readonly-composition-root",
        fri_root_label: b"readonly-fri-root",
        fri_beta_label: b"readonly-fri-beta",
        query_seed: b"readonly-query",
    };
    (
        Plan::new_v1(parameters, &layout, Kind::Base).unwrap(),
        Plan::new_v1(parameters, &layout, Kind::Aux).unwrap(),
        domains,
    )
}

fn coefficients(group: usize, column: usize) -> Vec<F> {
    let count = (1usize << (8 + group)) + MASK_DEGREE + 1;
    // Every original hiding coefficient, including the final one, is nonzero.
    (0..count)
        .map(|index| F((1 + 1_000_003 * group + 10_007 * column + 17 * index) as u64))
        .collect()
}

fn cpu(columns: &[Column], native: u8, common: u8) -> Result<Vec<Column>, AggregateStarkErrorV1> {
    columns
        .iter()
        .map(|column| {
            masked_trace_coefficients_on_coset_v1(column, native, common)
                .map(Column::from_vec_v1)
                .map_err(aggregate::map_transparent_error_v1)
        })
        .collect()
}

#[test]
fn initial_readonly_full_masked_roots_frontiers_rows_match_original_cpu() {
    let indices = [0, 1, 15, 16, 1023, 1024, ROWS - 2, ROWS - 1];
    for width in [1, 4, 8, 16] {
        let (base, aux, domains) = fixture(width);
        for plan in [base, aux] {
            let expected = plan
                .commit_replayed_v1(
                    &domains,
                    &indices,
                    |group, column| Ok(coefficients(group, column)),
                    cpu,
                )
                .unwrap();
            for device_columns in [0, 4, 8] {
                // Zero is required CPU; four/eight explicitly exercise Auto,
                // which may use either the original CPU fallback or Metal.
                let mut initial = evaluator(device_columns);
                let (actual, cut) = plan
                    .commit_retained_replayed_v1(
                        &domains,
                        &[],
                        None,
                        |group, column| Ok(coefficients(group, column)),
                        |columns, native, common, selected| {
                            assert!(selected.is_none());
                            initial.evaluate_initial_v1(columns, native, common)
                        },
                    )
                    .unwrap();
                assert_eq!(
                    actual.commitment.root, expected.commitment.root,
                    "width={width} device_columns={device_columns}"
                );
                assert!(actual.opened_rows.is_empty());
                assert_eq!(
                    initial.receipt.cpu_columns + initial.receipt.metal_columns,
                    width + 1
                );
                if device_columns == 0 {
                    assert_eq!(initial.receipt.metal_columns, 0);
                }
                let cut = cut.unwrap();
                let mut replay = evaluator(0);
                let (opened, replacement_cut) = plan
                    .commit_retained_replayed_v1(
                        &domains,
                        &indices,
                        Some(&cut),
                        |group, column| Ok(coefficients(group, column)),
                        |columns, native, common, selected| {
                            replay
                                .evaluate_selected_v1(columns, native, common, selected.unwrap())
                                .map(|columns| {
                                    columns.into_iter().map(Evaluated::from_field_v1).collect()
                                })
                        },
                    )
                    .unwrap();
                assert!(replacement_cut.is_none());
                assert_eq!(opened, expected);
                // Independent polynomial evaluation checks the complete row,
                // including the full mask tail and mixed native-group order.
                for &index in &indices {
                    let root = goldilocks_primitive_root_v1(COMMON_LOG).unwrap();
                    let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(index as u128));
                    let expected_row = (0..width)
                        .map(|column| (0, column))
                        .chain([(1, 0)])
                        .map(|(group, column)| {
                            coefficients(group, column)
                                .iter()
                                .rev()
                                .fold(F::ZERO, |value, &coefficient| value.mul(x).add(coefficient))
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(opened.opened_rows[&index], expected_row);
                }
            }
        }
    }
}

#[test]
fn initial_readonly_subpage_geometry_requires_original_cpu_fields() {
    let columns = (0..4)
        .map(|column| {
            Column::from_vec_v1(
                (0..137)
                    .map(|index| F((column * 1009 + index + 1) as u64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let expected = cpu(&columns, 7, 10).unwrap();
    for device_columns in [4, 8] {
        let mut evaluator = evaluator(device_columns);
        evaluator.evaluation_rows = 1 << 10;
        let actual = evaluator.evaluate_initial_v1(&columns, 7, 10).unwrap();
        assert_eq!(evaluator.receipt.cpu_columns, columns.len());
        assert_eq!(evaluator.receipt.metal_columns, 0);
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.as_ref(), expected.as_ref());
            assert!(actual.exact_initial_extent_v1(1 << 10));
        }
    }
}

#[test]
fn initial_readonly_rejects_each_final_mask_cell_without_source_mutation() {
    for group in 0..2 {
        for bad_column in 0..8 {
            let mut original = (0..8)
                .map(|column| coefficients(group, column))
                .collect::<Vec<_>>();
            *original[bad_column].last_mut().unwrap() = F(u64::MAX);
            let columns = original
                .iter()
                .cloned()
                .map(Column::from_vec_v1)
                .collect::<Vec<_>>();
            for device_columns in [0, 4, 8] {
                let mut evaluator = evaluator(device_columns);
                let result = evaluator.evaluate_initial_v1(&columns, 8 + group as u8, COMMON_LOG);
                assert!(matches!(
                    result,
                    Err(AggregateStarkErrorV1::NonCanonicalField)
                ));
                assert_eq!(evaluator.receipt, MainTransformReceiptV1::default());
                for (actual, expected) in columns.iter().zip(&original) {
                    assert_eq!(actual.as_ref(), expected.as_slice());
                }
            }
        }
    }
}

#[test]
fn initial_readonly_cpu_field_transfer_preserves_pointer_and_clears_on_unwind() {
    for unwind in [false, true] {
        let (result, observations) = aggregate::field_column_erasure::observe_v1(|| {
            catch_unwind(AssertUnwindSafe(|| {
                let values = vec![F(19); 32];
                let pointer = values.as_ptr();
                let column = Evaluated::from_field_v1(Column::from_vec_v1(values));
                assert_eq!(column.as_ref().as_ptr(), pointer);
                assert!(column.exact_initial_extent_v1(32));
                assert!(!column.exact_initial_extent_v1(31));
                assert!(!unwind, "injected immutable CPU owner unwind");
                drop(column);
            }))
        });
        assert_eq!(result.is_err(), unwind);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].cells, 32);
        assert_eq!(observations[0].nonzero_before, 32);
        assert_eq!(observations[0].nonzero_after, 0);
    }
    let mut oversized = Vec::with_capacity(64);
    oversized.resize(32, F(23));
    let column = Evaluated::from_field_v1(Column::from_vec_v1(oversized));
    assert!(!column.exact_initial_extent_v1(32));
}

#[test]
fn initial_readonly_pending_batch_clears_after_source_error_and_unwind() {
    let (plan, _, domains) = fixture(16);
    for unwind in [false, true] {
        let (outcome, observed) = aggregate::field_column_erasure::observe_v1(|| {
            catch_unwind(AssertUnwindSafe(|| {
                let mut initial = evaluator(0);
                plan.commit_retained_replayed_v1(
                    &domains,
                    &[],
                    None,
                    |group, column| {
                        if group == 0 && column == 8 {
                            assert!(!unwind, "injected next source callback unwind");
                            return Err(AggregateStarkErrorV1::InvalidLayout);
                        }
                        Ok(coefficients(group, column))
                    },
                    |columns, native, common, selected| {
                        assert!(selected.is_none());
                        initial.evaluate_initial_v1(columns, native, common)
                    },
                )
            }))
        });
        if unwind {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(
                outcome,
                Ok(Err(AggregateStarkErrorV1::InvalidLayout))
            ));
        }
        // The first eight complete outputs were pending for sixteen-column
        // absorption when the next callback failed. Their exact owners clear.
        assert_eq!(observed.iter().filter(|item| item.cells == ROWS).count(), 8);
        assert_eq!(
            observed
                .iter()
                .filter(|item| item.cells == (1 << 8) + MASK_DEGREE + 1)
                .count(),
            8
        );
        assert!(observed.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn initial_readonly_coalescer_refuses_bad_outputs_before_next_source_callback() {
    let (plan, _, domains) = fixture(16);
    for mutation in 0..3 {
        let mut source_calls = 0;
        let (result, observed) = aggregate::field_column_erasure::observe_v1(|| {
            plan.commit_retained_replayed_v1(
                &domains,
                &[],
                None,
                |group, column| {
                    source_calls += 1;
                    Ok(coefficients(group, column))
                },
                |columns, _, _, selected| {
                    assert!(selected.is_none());
                    Ok(columns
                        .iter()
                        .enumerate()
                        .map(|(column, _)| {
                            let mut values = if column == 7 && mutation == 1 {
                                let mut values = Vec::with_capacity(ROWS + 1);
                                values.resize(ROWS, F(29));
                                values
                            } else {
                                vec![F(29); ROWS]
                            };
                            if column == 7 {
                                match mutation {
                                    0 => *values.last_mut().unwrap() = F(u64::MAX),
                                    1 => (),
                                    2 => {
                                        values.pop();
                                    }
                                    _ => unreachable!(),
                                }
                            }
                            Evaluated::from_field_v1(Column::from_vec_v1(values))
                        })
                        .collect::<Vec<_>>())
                },
            )
        });
        assert_eq!(source_calls, 8, "mutation={mutation}");
        let expected = if mutation == 0 {
            AggregateStarkErrorV1::NonCanonicalField
        } else {
            AggregateStarkErrorV1::InvalidLayout
        };
        assert!(matches!(result, Err(error) if error == expected));
        assert!(observed.iter().all(|item| item.nonzero_after == 0));
        assert_eq!(observed.len(), 16);
    }
}

#[test]
fn initial_readonly_complete_profile_metadata_is_inside_existing_reserve() {
    use super::super::main_ca_resources::{MainCaBufferPhaseV1 as Phase, MainCaJointBufferPlanV1};
    use main_resources::*;
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let joint = MainCaJointBufferPlanV1::from_common_v1(plan).unwrap();
    let metadata = aggregate::initial_evaluated_column::metadata_payload_v1();
    assert_eq!(metadata, MAIN_READONLY_OUTPUT_METADATA_BYTES_V1);
    let original_descriptors = 16
        * (256
            + core::mem::size_of::<
                fastpq_prover::goldilocks_transform::CompletedGoldilocksColumnsV1,
            >()
            + 2 * core::mem::size_of::<usize>()
            + core::mem::size_of::<Evaluated>())
        + 4 * (core::mem::size_of::<Vec<Evaluated>>()
            + core::mem::size_of::<std::vec::IntoIter<Evaluated>>())
        + 16 * core::mem::size_of::<Evaluated>()
        + 16 * core::mem::size_of::<aggregate::ZeroizingFieldColumnV1>()
        + 16 * core::mem::size_of::<&[u64]>();
    let preflight = aggregate::initial_evaluated_column::PREFLIGHT_METADATA_BYTES_V1;
    #[cfg(target_pointer_width = "64")]
    assert_eq!(preflight, 56);
    assert_eq!(metadata, original_descriptors + preflight);

    assert!(
        metadata
            >= 16 * core::mem::size_of::<Evaluated>()
                + 16 * core::mem::size_of::<
                    fastpq_prover::goldilocks_transform::CompletedGoldilocksColumnsV1,
                >()
    );
    let explicit = MAIN_COMPOSITION_RESIDUE_SCRATCH_BYTES_V1
        + MAIN_COMPOSITION_SHA_FOLD_BYTES_V1
        + MAIN_QUERY_REUSE_METADATA_BYTES_V1
        + MAIN_ARITHMETIC_CONVOLUTION_SCRATCH_BYTES_V1
        + metadata;
    let residual = MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1
        .checked_sub(explicit)
        .unwrap();
    assert_eq!(explicit + residual, 1 << 30);
    assert_eq!(plan.maximum_live_buffers, CURRENT_FULL_PROFILE_PEAK_V1);
    for phase in [
        Phase::OriginalCommitments,
        Phase::Registration,
        Phase::ShaUnion,
        Phase::PrivateLinks,
        Phase::Finalization,
    ] {
        assert!(joint.required_v1(phase).unwrap() <= plan.maximum_live_buffers);
    }
    let ceiling = usize::try_from(
        crate::privacy_engines::zk_x509::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1,
    )
    .unwrap();
    assert_eq!(ceiling, 12 << 30);
    let retained = 288_345_698 + MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1;
    let charged = plan
        .check_source_payloads_v1(&[retained], MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1)
        .unwrap();
    assert!(charged <= ceiling);
    assert_eq!(
        charged,
        plan.maximum_live_buffers
            + retained
            + MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1
            + MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1
            + super::super::main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1
            + PUBLIC_COSET_POWERS_ALLOWANCE_V1
    );
    assert_eq!(plan.check_before_sources_v1(288_345_698).unwrap(), charged);
    assert!(plan.initial_commit_batch >= 16 * layout.common_lde_size() * core::mem::size_of::<F>());
}
