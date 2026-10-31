//! Native transaction admission, original-owner custody, parity and full-cycle costs.

use super::*;
use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
use crate::privacy_engines::zk_x509::private_table::inspection;
#[cfg(target_os = "macos")]
use fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1;
use fastpq_prover::goldilocks_transform::transform_goldilocks_columns_v1;

fn native_columns_v1(rows: usize, width: usize) -> ZeroizingBaseColumnsV1 {
    ZeroizingBaseColumnsV1(
        (0..width)
            .map(|column| {
                (0..rows)
                    .map(|row| F((31 * row + 17 * column + 1) as u64))
                    .collect()
            })
            .collect(),
    )
}

#[test]
fn native_transaction_charges_complete_backend_and_borrowed_metadata_without_widening() {
    let metadata = metadata_payload_v1();
    assert_eq!(
        metadata,
        size_of::<PreparedGoldilocksColumnsV1>()
            + size_of::<Outcome>()
            + size_of::<[&[u64]; 8]>()
            + size_of::<[&mut [u64]; 8]>()
            + 16 * size_of::<usize>()
    );
    for rows in [2, 32, 256, 1 << 16, 1 << 19] {
        for width in [1, 2, 4, 8] {
            let backend = metal_goldilocks_transform_extra_payload_v1(rows, width).unwrap();
            assert_eq!(
                required_payload_v1(rows, width).unwrap(),
                backend + metadata
            );
            assert!(prepared_goldilocks_columns_payload_bound_v1(rows, width).unwrap() < backend);
            let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, width);
            assert_eq!(policy.columns_v1(rows), width);
        }
    }
    for (rows, width) in [(0, 1), (1, 1), (3, 1), (1 << 20, 1), (32, 0), (32, 9)] {
        assert!(required_payload_v1(rows, width).is_err());
    }
    let mut actual = native_columns_v1(32, 1);
    let original = actual.0.clone();
    let root = goldilocks_primitive_root_v1(5).unwrap();
    let available = required_payload_v1(32, 1).unwrap();
    let result =
        prepare_and_publish_with_v1(&mut actual.0, root, available - 1, &mut || false, |_, _| {
            panic!("underfunded transaction prepared a private allocation")
        });
    assert!(matches!(result, Ok(MainNativePreparedV1::Cpu)));
    assert_eq!(actual.0, original);
}

#[test]
fn native_public_metadata_budget_falls_back_before_prepare_on_exact_old_tiny_budgets() {
    for rows in [2_usize, 32] {
        let width = 1;
        let root = goldilocks_primitive_root_v1(rows.ilog2() as u8).unwrap();
        let old_budget = metal_goldilocks_transform_extra_payload_v1(rows, width).unwrap()
            + rows * size_of::<u64>()
            + size_of::<Vec<u64>>()
            + size_of::<crate::privacy_engines::zk_x509::private_table::PrivateTableV1<Vec<u64>>>();
        let new_budget = required_payload_v1(rows, width).unwrap();
        assert!(old_budget < new_budget);
        for budget in [old_budget, new_budget - 1] {
            let mut actual = native_columns_v1(rows, width);
            let original = actual.0.clone();
            let result =
                prepare_and_publish_with_v1(&mut actual.0, root, budget, &mut || false, |_, _| {
                    panic!("public metadata refusal must precede preparation")
                });
            assert!(matches!(result, Ok(MainNativePreparedV1::Cpu)));
            assert_eq!(actual.0, original);
        }
        let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, width);
        assert_eq!(policy.columns_v1(rows), width);
        let original = native_columns_v1(rows, width);
        let mut old_mode = ZeroizingBaseColumnsV1(original.0.clone());
        let mut calls = 0;
        policy
            .inverse_with_v1(
                &mut old_mode.0,
                root,
                |words, root, direction| {
                    calls += 1;
                    transform_goldilocks_columns_v1(
                        words,
                        root,
                        direction,
                        fastpq_prover::ExecutionMode::Cpu,
                    )
                },
                || false,
            )
            .unwrap();
        assert_eq!(calls, 1);
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        policy
            .inverse_native_with_v1(&mut actual.0, root, &mut ExactNativeTransformV1, || false)
            .unwrap();
        assert_eq!(&*actual, &*old_mode);
    }
}

#[test]
fn native_preparation_errors_unwind_and_cpu_selection_preserve_every_original_owner() {
    let rows = 32;
    let root = goldilocks_primitive_root_v1(5).unwrap();
    let available = required_payload_v1(rows, 2).unwrap();
    let original = native_columns_v1(rows, 2);
    for error in [
        TransformError::InvalidShape,
        TransformError::InvalidRoot,
        TransformError::NonCanonicalInput,
        TransformError::NonCanonicalOutput,
        TransformError::DeviceUnavailable,
        TransformError::DeviceFailure("ordinary drained failure".into()),
        TransformError::CompletionUncertain,
    ] {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let pointers = actual
            .iter()
            .map(|column| (column.as_ptr(), column.len(), column.capacity()))
            .collect::<Vec<_>>();
        let expected_error = map_error_v1(error.clone());
        let (result, erased) = inspection::observe_v1(|| {
            prepare_and_publish_with_v1(
                &mut actual.0,
                root,
                available,
                &mut || false,
                |views, actual_root| {
                    assert_eq!(actual_root, root.0);
                    assert_eq!(views.len(), 2);
                    for (view, (pointer, length, _)) in views.iter().zip(&pointers) {
                        assert_eq!(view.as_ptr(), pointer.cast::<u64>());
                        assert_eq!(view.len(), *length);
                    }
                    Err(error)
                },
            )
        });
        assert!(matches!(result, Err(error) if error == expected_error));
        assert!(
            erased.is_empty(),
            "core must not allocate a shadow word matrix"
        );
        assert_eq!(&*actual, &*original);
        assert_eq!(
            actual
                .iter()
                .map(|column| (column.as_ptr(), column.len(), column.capacity()))
                .collect::<Vec<_>>(),
            pointers
        );
    }
    for unwind in [false, true] {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepare_and_publish_with_v1(&mut actual.0, root, available, &mut || false, |_, _| {
                assert!(!unwind, "prepared callback unwind before publication");
                Ok(Outcome::Cpu)
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result.unwrap(), Ok(MainNativePreparedV1::Cpu)));
        }
        assert_eq!(&*actual, &*original);
    }
    for uncertain_at in [0, 1] {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let mut checks = 0;
        let mut prepares = 0;
        let result = prepare_and_publish_with_v1(
            &mut actual.0,
            root,
            available,
            &mut || {
                let uncertain = checks == uncertain_at;
                checks += 1;
                uncertain
            },
            |_, _| {
                prepares += 1;
                Ok(Outcome::Cpu)
            },
        );
        assert!(matches!(
            result,
            Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
        ));
        assert_eq!(prepares, uncertain_at);
        assert_eq!(&*actual, &*original);
    }
    // Cleanup may latch uncertainty while the dispatch reports an ordinary
    // error. Match the original adapter's terminal-status precedence.
    let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
    let uncertain = core::cell::Cell::new(false);
    let result = prepare_and_publish_with_v1(
        &mut actual.0,
        root,
        available,
        &mut || uncertain.get(),
        |_, _| {
            uncertain.set(true);
            Err(TransformError::DeviceFailure(
                "drained error with latched uncertainty".into(),
            ))
        },
    );
    assert!(matches!(
        result,
        Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
    ));
    assert_eq!(&*actual, &*original);
}

#[test]
fn native_exact_cpu_path_and_word_injection_keep_original_inverse_and_validation() {
    let root = goldilocks_primitive_root_v1(5).unwrap();
    let original = native_columns_v1(32, 8);
    let mut expected = ZeroizingBaseColumnsV1(original.0.clone());
    MainBoundedTransformPolicyV1::cpu_v1()
        .inverse_with_v1(&mut expected.0, root, |_, _, _| unreachable!(), || false)
        .unwrap();
    let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
    let pointers = actual.iter().map(Vec::as_ptr).collect::<Vec<_>>();
    MainBoundedTransformPolicyV1::cpu_v1()
        .inverse_native_with_v1(&mut actual.0, root, &mut ExactNativeTransformV1, || false)
        .unwrap();
    assert_eq!(&*actual, &*expected);
    assert_eq!(actual.iter().map(Vec::as_ptr).collect::<Vec<_>>(), pointers);
    for width in [1, 2, 4, 8] {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let mut calls = 0;
        let mut injection = WordInjectionV1(|words: &mut [Vec<u64>], root, direction| {
            calls += 1;
            assert_eq!(words.len(), width);
            assert_eq!(direction, Direction::Inverse);
            transform_goldilocks_columns_v1(
                words,
                root,
                direction,
                fastpq_prover::ExecutionMode::Cpu,
            )
        });
        MainBoundedTransformPolicyV1::for_test_v1(32, width)
            .inverse_native_with_v1(&mut actual.0, root, &mut injection, || false)
            .unwrap();
        assert_eq!(calls, 8 / width);
        assert_eq!(&*actual, &*expected);
    }
    for failure in 0..7 {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let mut actual_root = root;
        match failure {
            0 => actual.0.clear(),
            1 => actual.0.push(vec![F::ZERO; 32]),
            2 => {
                actual.0[1].pop();
            }
            3 => actual.0[7][31] = F(GOLDILOCKS_MODULUS_V1),
            4 => actual_root = F::ONE,
            5 => actual_root = F(GOLDILOCKS_MODULUS_V1),
            _ => {}
        }
        let before = actual.0.clone();
        let result = MainBoundedTransformPolicyV1::cpu_v1().inverse_native_with_v1(
            &mut actual.0,
            actual_root,
            &mut ExactNativeTransformV1,
            || failure == 6,
        );
        assert!(result.is_err());
        assert_eq!(actual.0, before);
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "isolated native19 actual Metal transaction/word adapter allocation-and-clear A/B"]
fn native19_transactional_metal_preserves_all_fields_and_reports_complete_costs() {
    use crate::privacy_engines::zk_x509::prover_observation::ObservationV1;
    use std::time::{Duration, Instant};

    assert!(!cfg!(debug_assertions), "use an optimized build");
    assert_eq!(
        fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal)
    );
    assert_eq!(
        rayon::current_num_threads(),
        20,
        "run with RAYON_NUM_THREADS=20"
    );
    assert_eq!(
        rayon::current_thread_index(),
        None,
        "measure the external prover caller"
    );
    struct StopTelemetry;
    impl Drop for StopTelemetry {
        fn drop(&mut self) {
            fastpq_prover::enable_queue_depth_stats(false);
        }
    }
    fastpq_prover::enable_queue_depth_stats(true);
    let _telemetry = StopTelemetry;
    let rows = 1 << 19;
    let root = goldilocks_primitive_root_v1(19).unwrap();
    const ROUNDS: usize = 16;
    for width in [1, 4, 8] {
        let original = native_columns_v1(rows, width);
        let mut expected = ZeroizingBaseColumnsV1(original.0.clone());
        MainBoundedTransformPolicyV1::cpu_v1()
            .inverse_with_v1(&mut expected.0, root, |_, _, _| unreachable!(), || false)
            .unwrap();
        let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, width);
        assert_eq!(policy.columns_v1(rows), width);
        let mut elapsed = [[Duration::ZERO; ROUNDS]; 2];
        let mut allocated = [Duration::ZERO; 2];
        let mut transformed = [Duration::ZERO; 2];
        let mut cleared = [Duration::ZERO; 2];
        let mut staging = [fastpq_prover::ColumnStagingPhaseStats::default(); 2];
        for round in 0..=ROUNDS {
            for transactional in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                fastpq_prover::take_column_staging_stats().unwrap();
                let observation = ObservationV1::begin_v1();
                let allocated_at = Instant::now();
                let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
                let allocation = allocated_at.elapsed();
                let pointers = actual
                    .iter()
                    .map(|column| (column.as_ptr(), column.capacity()))
                    .collect::<Vec<_>>();
                let transformed_at = Instant::now();
                if transactional {
                    policy
                        .inverse_native_with_v1(
                            &mut actual.0,
                            root,
                            &mut ExactNativeTransformV1,
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap();
                } else {
                    policy
                        .inverse_with_v1(
                            &mut actual.0,
                            root,
                            |words, root, direction| {
                                let backend = transform_goldilocks_columns_v1(
                                    words,
                                    root,
                                    direction,
                                    fastpq_prover::ExecutionMode::Gpu,
                                )?;
                                assert_eq!(backend, Backend::Metal);
                                Ok(backend)
                            },
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap();
                }
                let transform = transformed_at.elapsed();
                let captured = fastpq_prover::take_column_staging_stats().unwrap().fft();
                let receipt = observation.finish_v1().public_text_v1();
                assert!(receipt.contains(&format!("native_replay_metal_inverse_columns={width}")));
                assert!(receipt.contains("native_replay_cpu_inverse_columns=0"));
                assert_eq!(&*actual, &*expected);
                assert_eq!(
                    actual
                        .iter()
                        .map(|column| (column.as_ptr(), column.capacity()))
                        .collect::<Vec<_>>(),
                    pointers
                );
                let cleared_at = Instant::now();
                drop(actual);
                let clear = cleared_at.elapsed();
                if round != 0 {
                    let mode = usize::from(transactional);
                    allocated[mode] += allocation;
                    transformed[mode] += transform;
                    cleared[mode] += clear;
                    elapsed[mode][round - 1] = allocation + transform + clear;
                    staging[mode].batches += captured.batches;
                    staging[mode].flatten_ms += captured.flatten_ms;
                    staging[mode].wait_ms += captured.wait_ms;
                    staging[mode].timestamped_commands += captured.timestamped_commands;
                    staging[mode].gpu_execution_ms += captured.gpu_execution_ms;
                    staging[mode].cpu_driver_scheduling_ms += captured.cpu_driver_scheduling_ms;
                    staging[mode].pre_gpu_queue_ms += captured.pre_gpu_queue_ms;
                }
            }
        }
        for transactional in [false, true] {
            let mode = usize::from(transactional);
            elapsed[mode].sort_unstable();
            println!(
                "x509_native_transaction_cost rows={rows} width={width} workers=20 caller_context=external transactional={transactional} calls={ROUNDS} total_ns={} median_ns={} allocation_ns={} transform_all_staging_clear_ns={} result_clear_ns={} staging_batches={} flatten_ms={:.6} wait_ms={:.6} timestamped_commands={} gpu_execution_ms={:.6} cpu_driver_scheduling_ms={:.6} pre_gpu_queue_ms={:.6} timing_scope=successful-command-intervals-not-additive parity=all_fields_exact pointers=unchanged",
                (allocated[mode] + transformed[mode] + cleared[mode]).as_nanos(),
                elapsed[mode][ROUNDS / 2].as_nanos(),
                allocated[mode].as_nanos(),
                transformed[mode].as_nanos(),
                cleared[mode].as_nanos(),
                staging[mode].batches,
                staging[mode].flatten_ms,
                staging[mode].wait_ms,
                staging[mode].timestamped_commands,
                staging[mode].gpu_execution_ms,
                staging[mode].cpu_driver_scheduling_ms,
                staging[mode].pre_gpu_queue_ms,
            );
        }
    }
}
