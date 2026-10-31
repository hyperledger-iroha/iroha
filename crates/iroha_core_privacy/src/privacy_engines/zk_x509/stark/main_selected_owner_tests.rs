//! External-caller query owner lifetimes, fallback release and exact admission.

use super::*;
use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;

const ROWS: usize = 1 << 22;

fn evaluator_v1() -> MainTraceCosetEvaluatorV1 {
    MainTraceCosetEvaluatorV1 {
        evaluation_rows: ROWS,
        device_columns: 1,
        receipt: MainTransformReceiptV1::default(),
        coset_powers: Some(PublicCosetPowersV1::new_v1(MAX_PUBLIC_COSET_POWERS_V1).unwrap()),
        selected_scratch: selected_coset::SelectedScratchV1::default(),
    }
}

fn columns_v1(native: u8) -> Vec<Column> {
    (0..8usize)
        .map(|column| {
            Column::from_vec_v1(
                (0..(1usize << native) + MASK_DEGREE + 1)
                    .map(|degree| F((degree * 137 + column * 53 + 19) as u64))
                    .collect(),
            )
        })
        .collect()
}

fn check_values_v1(columns: &[Column], selected: &[usize], outputs: &[Column]) {
    assert_eq!(columns.len(), outputs.len());
    let root = goldilocks_primitive_root_v1(22).unwrap();
    for (input, output) in columns.iter().zip(outputs) {
        assert_eq!(output.len(), selected.len());
        for (&row, &value) in selected.iter().zip(output.iter()) {
            let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
            let expected = input
                .iter()
                .rev()
                .fold(F::ZERO, |sum, &coefficient| sum.mul(point).add(coefficient));
            assert_eq!(value, expected);
        }
    }
}

#[test]
fn selected_owner_external_caller_reuses_across_source_batches_and_clears_at_phase_end() {
    assert!(
        rayon::current_thread_index().is_none(),
        "exercise the external proof caller"
    );
    let (_, erased) = selected_coset::observe_batch_clear_v1(|| {
        let mut evaluator = evaluator_v1();
        let mut identity = None;
        for (native, selected) in [
            (15, vec![0, ROWS - 1]),
            (16, vec![1, 7, ROWS / 2]),
            (15, vec![0]),
        ] {
            // Source allocations coexist with the retained selected matrix,
            // as they do between callbacks of commit_joined_batches_v1.
            let columns = columns_v1(native);
            let actual = evaluator
                .evaluate_selected_v1(&columns, native, 22, &selected)
                .unwrap();
            check_values_v1(&columns, &selected, &actual);
            let current = evaluator.selected_scratch.retained_identity_v1().unwrap();
            assert_eq!(current.1, 8 * ROWS);
            assert_eq!(*identity.get_or_insert(current), current);
            assert_eq!(selected_coset::observed_batch_clear_count_v1(), Some(0));
        }
    });
    assert_eq!(erased.len(), 1);
    assert_eq!(erased[0].cells, 8 * ROWS);
    assert!(erased[0].nonzero_before > 0);
    assert_eq!(erased[0].nonzero_after, 0);
}

#[test]
fn selected_owner_external_partial_fallback_releases_before_dispatch_success_error_or_unwind() {
    assert!(rayon::current_thread_index().is_none());
    let columns = columns_v1(15);
    let selected = [0, 7, ROWS - 1];
    for outcome in 0..3 {
        let mut evaluator = evaluator_v1();
        evaluator
            .evaluate_selected_v1(&columns, 15, 22, &selected)
            .unwrap();
        let (result, erased) = selected_coset::observe_batch_clear_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                evaluator.evaluate_selected_with_v1(
                    &columns[..1],
                    15,
                    22,
                    Some(&selected),
                    |words, root| {
                        // The prior full matrix has already been erased before the
                        // fallback's original dispatch receives its working words.
                        assert_eq!(selected_coset::observed_batch_clear_count_v1(), Some(1));
                        match outcome {
                            0 => transform_goldilocks_columns_v1(
                                words,
                                root,
                                Direction::Forward,
                                fastpq_prover::ExecutionMode::Cpu,
                            )
                            .map_err(|_| AggregateStarkErrorV1::InternalInvariant),
                            1 => Err(AggregateStarkErrorV1::InternalInvariant),
                            _ => panic!("injected fallback dispatch unwind"),
                        }
                    },
                )
            }))
        });
        match outcome {
            0 => check_values_v1(&columns[..1], &selected, &result.unwrap().unwrap()),
            1 => assert_eq!(
                result.unwrap().err().unwrap(),
                AggregateStarkErrorV1::InternalInvariant
            ),
            _ => assert!(result.is_err()),
        }
        assert!(evaluator.selected_scratch.retained_identity_v1().is_none());
        assert_eq!(erased.len(), 1);
        assert_eq!(erased[0].cells, 8 * ROWS);
        assert_eq!(erased[0].nonzero_after, 0);
    }
}

#[test]
fn selected_owner_external_validation_failure_and_small_cpu_fallback_clear_prior_custody() {
    assert!(rayon::current_thread_index().is_none());
    let columns = columns_v1(15);
    for fault in 0..3 {
        let mut evaluator = evaluator_v1();
        evaluator
            .evaluate_selected_v1(&columns, 15, 22, &[0])
            .unwrap();
        let (result, erased) = selected_coset::observe_batch_clear_v1(|| {
            if fault == 0 {
                evaluator.evaluate_selected_v1(&columns, 15, 22, &[])
            } else if fault == 1 {
                let mut invalid = columns_v1(15);
                invalid[7] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1)]);
                evaluator.evaluate_selected_v1(&invalid, 15, 22, &[0])
            } else {
                let small = columns_v1(5);
                let actual = evaluator.evaluate_selected_v1(&small, 5, 22, &[0, ROWS - 1])?;
                check_values_v1(&small, &[0, ROWS - 1], &actual);
                Ok(actual)
            }
        });
        if fault < 2 {
            assert_eq!(
                result.err().unwrap(),
                if fault == 0 {
                    AggregateStarkErrorV1::InvalidLayout
                } else {
                    AggregateStarkErrorV1::NonCanonicalField
                }
            );
        } else {
            assert!(result.is_ok());
        }
        assert!(evaluator.selected_scratch.retained_identity_v1().is_none());
        assert_eq!(erased.len(), 1);
        assert_eq!(erased[0].cells, 8 * ROWS);
        assert_eq!(erased[0].nonzero_after, 0);
    }
}

#[test]
fn selected_owner_headers_and_source_overlap_fit_the_existing_replay_allowance() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let native_coefficients = 8 * ((1usize << 19) + MASK_DEGREE + 1) * size_of::<F>();
    let retained_scratch = 8 * ROWS * size_of::<F>();
    let native_overlap = 2 * (1usize << 19) * size_of::<F>();
    assert_eq!(
        plan.replay_batch,
        native_coefficients + retained_scratch + native_overlap
    );
    assert_eq!(retained_scratch, 268_435_456);
    assert_eq!(plan.replay_batch, 310_494_720);
    assert_eq!(
        main_resources::MAIN_QUERY_REUSE_METADATA_BYTES_V1,
        4 * size_of::<selected_coset::SelectedScratchV1>()
    );
    assert_eq!(
        PUBLIC_COSET_POWERS_ALLOWANCE_V1,
        MAX_PUBLIC_COSET_POWERS_V1 * size_of::<F>() + size_of::<MainTraceCosetEvaluatorV1>()
    );
    let no_sources = plan.check_source_payloads_v1(&[], 0).unwrap();
    assert_eq!(
        no_sources,
        plan.maximum_live_buffers
            + main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1
            + main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1
            + PUBLIC_COSET_POWERS_ALLOWANCE_V1
    );
}
