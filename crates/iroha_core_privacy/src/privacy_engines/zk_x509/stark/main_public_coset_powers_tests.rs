//! Shared public coset table parity, bounded ownership and native timing controls.
use super::*;

#[test]
fn public_coset_table_is_exact_bounded_and_reserved_with_every_source_owner() {
    assert!(PublicCosetPowersV1::new_v1(0).is_err());
    assert!(PublicCosetPowersV1::new_v1(MAX_PUBLIC_COSET_POWERS_V1 + 1).is_err());
    let table = PublicCosetPowersV1::new_v1(MAX_PUBLIC_COSET_POWERS_V1).unwrap();
    assert_eq!(table.values.len(), 526_104);
    assert_eq!(table.values.capacity(), table.values.len());
    for index in [0, 1, 2, 63, 64, 255, 256, 32_767, 524_287, 526_103] {
        assert_eq!(
            table.values[index],
            F(GOLDILOCKS_GENERATOR_V1).pow(index as u128)
        );
    }
    assert_eq!(
        table.values.capacity() * core::mem::size_of::<F>()
            + core::mem::size_of::<MainTraceCosetEvaluatorV1>(),
        PUBLIC_COSET_POWERS_ALLOWANCE_V1
    );
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let zero_source = plan.check_source_payloads_v1(&[], 0).unwrap();
    assert_eq!(
        zero_source,
        plan.maximum_live_buffers
            + main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1
            + main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1
            + PUBLIC_COSET_POWERS_ALLOWANCE_V1
    );
    let source_budget = plan.remaining_source_and_runtime_envelope
        - main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1
        - main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1
        - PUBLIC_COSET_POWERS_ALLOWANCE_V1;
    assert!(plan.check_source_payloads_v1(&[source_budget], 0).is_ok());
    assert!(
        plan.check_source_payloads_v1(&[source_budget + 1], 0)
            .is_err()
    );
    // Every device width is selected from the remainder AFTER public ownership.
    let rows = layout.common_lde_size();
    for width in [1, 2, 4] {
        let device =
            fastpq_prover::goldilocks_transform::metal_goldilocks_transform_extra_payload_v1(
                rows, width,
            )
            .unwrap();
        let with_table = device + PUBLIC_COSET_POWERS_ALLOWANCE_V1;
        assert_eq!(
            select_metal_columns_v1(rows, with_table - PUBLIC_COSET_POWERS_ALLOWANCE_V1),
            width
        );
        assert!(
            select_metal_columns_v1(rows, with_table - PUBLIC_COSET_POWERS_ALLOWANCE_V1 - 1)
                < width
        );
    }
}

#[test]
fn public_power_reuse_preserves_serial_products_padding_and_one_owner_across_calls() {
    use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    for width in [1, 3, 8] {
        let columns: Vec<_> = (0..width)
            .map(|column| {
                Column::from_vec_v1(
                    (0..21 - column)
                        .map(|index| {
                            if index % 3 == 0 {
                                F::ZERO
                            } else {
                                F(GOLDILOCKS_MODULUS_V1 - 1 - index as u64)
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        let table = PublicCosetPowersV1::new_v1(64).unwrap();
        let mut words = PrivateTableV1::new(vec![vec![0; 64]; width], erase_word_rows_v1);
        for workers in [1, 2, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    populate_coset_words_v1(&mut words, &columns, &table.values).unwrap();
                });
            for (row, coefficients) in words.iter().zip(&columns) {
                let mut power = F::ONE;
                for (index, coefficient) in coefficients.iter().enumerate() {
                    assert_eq!(row[index], coefficient.mul(power).0);
                    power = power.mul(F(GOLDILOCKS_GENERATOR_V1));
                }
                assert!(row[coefficients.len()..].iter().all(|word| *word == 0));
            }
        }
        let mut evaluator = MainTraceCosetEvaluatorV1 {
            evaluation_rows: 64,
            device_columns: 2,
            receipt: MainTransformReceiptV1::default(),
            coset_powers: Some(table),
        };
        let address = evaluator.coset_powers.as_ref().unwrap().values.as_ptr();
        for _ in 0..3 {
            let actual = evaluator
                .evaluate_with_v1(&columns, 4, 6, |values, root| {
                    transform_goldilocks_columns_v1(
                        values,
                        root,
                        Direction::Forward,
                        fastpq_prover::ExecutionMode::Cpu,
                    )
                    .map_err(|_| AggregateStarkErrorV1::InvalidLayout)
                })
                .unwrap();
            assert_eq!(
                evaluator.coset_powers.as_ref().unwrap().values.as_ptr(),
                address
            );
            for (values, coefficients) in actual.iter().zip(&columns) {
                assert_eq!(
                    &**values,
                    masked_trace_coefficients_on_coset_v1(coefficients, 4, 6).unwrap()
                );
            }
        }
    }
    let columns = vec![Column::from_vec_v1(vec![F(3); 21])];
    let mut short = vec![vec![9; 20]];
    assert!(populate_coset_words_v1(&mut short, &columns, &[F::ONE; 21]).is_err());
    assert!(short[0].iter().all(|word| *word == 9));
    let mut words = vec![vec![9; 64]];
    assert!(populate_coset_words_v1(&mut words, &columns, &[F::ONE; 20]).is_err());
    assert!(words[0].iter().all(|word| *word == 9));
    for table in [None, Some(PublicCosetPowersV1::new_v1(20).unwrap())] {
        let mut evaluator = MainTraceCosetEvaluatorV1 {
            evaluation_rows: 64,
            device_columns: 1,
            receipt: MainTransformReceiptV1::default(),
            coset_powers: table,
        };
        assert!(
            evaluator
                .evaluate_with_v1(&columns, 4, 6, |_, _| panic!("invalid table dispatched"))
                .is_err()
        );
    }
}

#[test]
#[ignore = "component host preparation timing only; root owns native execution and full proof qualification"]
fn shared_coset_preparation_matches_original_products_at_native19_width8() {
    use std::{hint::black_box, time::Instant};
    let length = MAX_PUBLIC_COSET_POWERS_V1;
    let table_start = Instant::now();
    let table = PublicCosetPowersV1::new_v1(length).unwrap();
    let table_elapsed = table_start.elapsed();
    let columns: Vec<_> = (0..8)
        .map(|column| {
            Column::from_vec_v1(
                (0..length)
                    .map(|index| F(((index + 1) * 37 + column * 101) as u64))
                    .collect(),
            )
        })
        .collect();
    let mut original = PrivateTableV1::new(vec![vec![0; length]; 8], erase_word_rows_v1);
    let mut reused = PrivateTableV1::new(vec![vec![0; length]; 8], erase_word_rows_v1);
    let mut original_elapsed = std::time::Duration::ZERO;
    let mut reused_elapsed = std::time::Duration::ZERO;
    // Alternate execution order so neither implementation is always measured
    // with the preceding implementation's warmed input and public-power cache.
    for iteration in 0..6 {
        for use_shared in [iteration % 2 == 0, iteration % 2 != 0] {
            let start = Instant::now();
            if use_shared {
                populate_coset_words_v1(&mut reused, &columns, &table.values).unwrap();
                reused_elapsed += start.elapsed();
                black_box(&reused);
            } else {
                for (output, coefficients) in original.iter_mut().zip(&columns) {
                    let mut power = F::ONE;
                    for (value, coefficient) in output.iter_mut().zip(coefficients.iter()) {
                        *value = coefficient.mul(power).0;
                        power = power.mul(F(GOLDILOCKS_GENERATOR_V1));
                    }
                }
                original_elapsed += start.elapsed();
                black_box(&original);
            }
        }
        assert_eq!(&**original, &**reused);
    }
    eprintln!(
        "shared_public_coset_preparation rows={length} columns=8 repeats=6 alternating_order=true table_ns={} original_ns={} shared_ns={} complete_proof_qualified=false",
        table_elapsed.as_nanos(),
        original_elapsed.as_nanos(),
        reused_elapsed.as_nanos()
    );
}

#[test]
#[ignore = "isolated real Metal four/eight-column joined batching comparison; component evidence only"]
fn joined_full_output_metal_four_and_eight_column_batches_match_exactly() {
    use crate::privacy_engines::transparent_stark::masked_trace_coefficients_with_mask_v1;
    use fastpq_prover::goldilocks_transform::metal_goldilocks_transform_extra_payload_v1;
    use std::time::Instant;

    assert_eq!(
        fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal),
    );
    let common = 22;
    let rows = 1usize << common;
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let cap = super::super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1 as usize;
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let available = cap - plan.joined_transform_admission_v1(288_345_698).unwrap();
    assert!(metal_goldilocks_transform_extra_payload_v1(rows, 8).unwrap() <= available);
    for native in [5, 19] {
        let columns = (0..8)
            .map(|column| {
                let values = (0..1usize << native)
                    .map(|row| F((17 * row + 13 * column + 1) as u64))
                    .collect::<Vec<_>>();
                let mask = (0..MASK_DEGREE + 1)
                    .map(|degree| F((29 * degree + 7 * column + 3) as u64))
                    .collect::<Vec<_>>();
                Column::from_vec_v1(
                    masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let expected = columns
            .iter()
            .map(|column| {
                Column::from_vec_v1(
                    masked_trace_coefficients_on_coset_v1(column, native, common).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let mut evaluator = MainTraceCosetEvaluatorV1::new_v1(&layout, 288_345_698).unwrap();
        assert!(evaluator.coset_powers.is_some());
        for round in 0..3 {
            for device_columns in if round % 2 == 0 { [4, 8] } else { [8, 4] } {
                // Test only: both shapes passed the exact joined admission above.
                evaluator.device_columns = device_columns;
                let started = Instant::now();
                let before = evaluator.receipt.metal_columns;
                let output = evaluator
                    .evaluate_with_v1(&columns, native, common, |words, root| {
                        let backend = transform_goldilocks_columns_v1(
                            words,
                            root,
                            Direction::Forward,
                            fastpq_prover::ExecutionMode::Gpu,
                        )
                        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
                        assert_eq!(backend, Backend::Metal);
                        Ok(backend)
                    })
                    .unwrap();
                let transform_ns = started.elapsed().as_nanos();
                assert_eq!(evaluator.receipt.metal_columns - before, 8);
                for (actual, expected) in output.iter().zip(&expected) {
                    assert_eq!(actual.as_ref(), expected.as_ref());
                }
                let clear_started = Instant::now();
                drop(output);
                let clear_ns = clear_started.elapsed().as_nanos();
                println!(
                    "x509_joined_device_batch native_log={native} common_log={common} width=8 device_columns={device_columns} workers={} round={round} transform_gather_scratch_clear_ns={transform_ns} output_clear_ns={clear_ns} parity=exact",
                    rayon::current_num_threads(),
                );
            }
        }
    }
}

#[test]
fn semantic_scope_receives_actual_full_selected_and_split_backend_work() {
    use crate::privacy_engines::zk_x509::prover_observation::transform::{FamilyV1, KindV1};
    use crate::privacy_engines::zk_x509::prover_observation::{
        ObservationV1, PhaseTimerV1, PhaseV1,
    };
    let columns = (0..5)
        .map(|column| {
            Column::from_vec_v1(
                (0..21)
                    .map(|degree| F((column * 19 + degree * 7 + 1) as u64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    for selected in [false, true] {
        let observation = ObservationV1::begin_v1();
        let timer = PhaseTimerV1::start_v1(PhaseV1::InitialJoinedTransform);
        let child = timer
            .transform_context_v1()
            .enter(KindV1::Base, Ok(FamilyV1::Projection));
        let mut evaluator = MainTraceCosetEvaluatorV1 {
            evaluation_rows: 64,
            device_columns: if selected { 0 } else { 2 },
            receipt: MainTransformReceiptV1::default(),
            coset_powers: if selected {
                None
            } else {
                Some(PublicCosetPowersV1::new_v1(64).unwrap())
            },
        };
        let mut backend_calls = 0;
        let indices = [0, 17, 63];
        let output = if selected {
            evaluator
                .evaluate_selected_v1(&columns, 4, 6, &indices)
                .unwrap()
        } else {
            evaluator
                .evaluate_with_v1(&columns, 4, 6, |words, root| {
                    transform_goldilocks_columns_v1(
                        words,
                        root,
                        Direction::Forward,
                        fastpq_prover::ExecutionMode::Cpu,
                    )
                    .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
                    backend_calls += 1;
                    // Inject only the receipt identity after real CPU arithmetic.
                    // This tests mixed submission accounting, not Metal hardware.
                    Ok(if backend_calls == 2 {
                        Backend::Metal
                    } else {
                        Backend::Cpu
                    })
                })
                .unwrap()
        };
        child.complete();
        timer.complete_v1();
        let receipt = observation.finish_v1();
        for (actual, column) in output.iter().zip(&columns) {
            let full = masked_trace_coefficients_on_coset_v1(column, 4, 6).unwrap();
            let expected = if selected {
                indices.iter().map(|&index| full[index]).collect::<Vec<_>>()
            } else {
                full
            };
            assert_eq!(&**actual, expected.as_slice());
        }
        let record = receipt.measurement_v1().unwrap();
        let (node_index, node) = record
            .phase_tree
            .nodes
            .iter()
            .enumerate()
            .find(|(_, node)| node.label == FamilyV1::Projection.label(KindV1::Base))
            .unwrap();
        assert_eq!((node.calls, node.completed, node.interrupted), (1, 1, 0));
        let work = record
            .work_counters
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.label.as_str(),
                    "transform_cpu_columns" | "transform_metal_columns"
                )
            })
            .collect::<Vec<_>>();
        assert!(
            work.iter()
                .all(|entry| usize::try_from(entry.phase).ok() == Some(node_index))
        );
        assert_eq!(work.iter().map(|entry| entry.total_units).sum::<u64>(), 5);
        assert_eq!(
            work.iter().map(|entry| entry.count).sum::<u64>(),
            if selected { 1 } else { 3 }
        );
        assert_eq!(backend_calls, if selected { 0 } else { 3 });
        assert_eq!(evaluator.receipt.cpu_columns, if selected { 5 } else { 3 });
        assert_eq!(
            evaluator.receipt.metal_columns,
            if selected { 0 } else { 2 }
        );
    }
}

#[test]
fn semantic_scope_keeps_actual_evaluator_refusal_unwind_and_populated_cleanup() {
    use crate::privacy_engines::zk_x509::private_table::inspection;
    use crate::privacy_engines::zk_x509::prover_observation::transform::{FamilyV1, KindV1};
    use crate::privacy_engines::zk_x509::prover_observation::{
        ObservationV1, PhaseTimerV1, PhaseV1,
    };
    for unwind in [false, true] {
        let observation = ObservationV1::begin_v1();
        let (result, cleared) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let timer = PhaseTimerV1::start_v1(PhaseV1::InitialJoinedTransform);
                let child = timer
                    .transform_context_v1()
                    .enter(KindV1::Aux, Ok(FamilyV1::MixedRelations));
                let columns = vec![Column::from_vec_v1(vec![F(3); 21])];
                let mut evaluator = MainTraceCosetEvaluatorV1 {
                    evaluation_rows: 64,
                    device_columns: 1,
                    receipt: MainTransformReceiptV1::default(),
                    coset_powers: Some(PublicCosetPowersV1::new_v1(64).unwrap()),
                };
                let output = evaluator.evaluate_with_v1(&columns, 4, 6, |words, _| {
                    assert!(words.iter().flatten().any(|&word| word != 0));
                    if unwind {
                        panic!("injected original backend unwind");
                    }
                    Err(AggregateStarkErrorV1::InternalInvariant)
                })?;
                child.complete();
                timer.complete_v1();
                Ok::<_, AggregateStarkErrorV1>(output)
            }))
        });
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(AggregateStarkErrorV1::InternalInvariant))
            ));
        }
        assert!(
            cleared
                .iter()
                .any(|entry| entry.cells >= 64 && entry.nonzero_before > 0)
        );
        assert!(cleared.iter().all(|entry| entry.nonzero_after == 0));
        let receipt = observation.finish_v1();
        for label in [
            "InitialJoinedTransform",
            FamilyV1::MixedRelations.label(KindV1::Aux),
        ] {
            let node = receipt
                .measurement_v1()
                .unwrap()
                .phase_tree
                .nodes
                .iter()
                .find(|node| node.label == label)
                .unwrap();
            assert_eq!(
                (node.calls, node.completed, node.interrupted, node.unwound),
                (1, 0, 1, u64::from(unwind))
            );
        }
    }
}
