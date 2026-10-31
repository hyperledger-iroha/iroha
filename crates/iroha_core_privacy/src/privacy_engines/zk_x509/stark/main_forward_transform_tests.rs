//! Exact quotient forward admission, transaction failures and complete-cycle costs.

use super::super::main_bounded_transform::with_forward_group_limit_for_testing_v1;
use super::*;
use crate::privacy_engines::transparent_stark::{GOLDILOCKS_MODULUS_V1, goldilocks_fft_v1};
use crate::privacy_engines::zk_x509::private_table::inspection;
#[cfg(target_os = "macos")]
use fastpq_prover::goldilocks_transform::transform_goldilocks_columns_v1;

fn folded_columns_v1(log: u8, width: usize) -> ZeroizingBaseColumnsV1 {
    let rows = 1usize << log;
    let full_root = goldilocks_primitive_root_v1(log + 2).unwrap();
    let stripe = super::super::main_quotient_stripes::MainQuotientStripeV1 {
        rows,
        count: 4,
        ordinal: 1,
        next_stride: 1,
        root: full_root.pow(4),
        shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root),
    };
    ZeroizingBaseColumnsV1(
        (0..width)
            .map(|column| {
                let coefficients = ZeroizingMainTraceColumnV1(
                    (0..rows + MASK_DEGREE + 1)
                        .map(|index| {
                            F(GOLDILOCKS_MODULUS_V1
                                - 1
                                - ((index * 31 + column * 17) % 7919) as u64)
                        })
                        .collect(),
                );
                let mut output = vec![F::ZERO; rows];
                stripe.fold_into_v1(&coefficients, &mut output).unwrap();
                output
            })
            .collect(),
    )
}

#[test]
fn forward_transaction_admits_sixteen_with_full_staging_and_views_without_widening_native() {
    assert_eq!(
        forward_metadata_payload_v1(),
        metadata_payload_v1() + 16 * size_of::<&[u64]>()
    );
    for rows in [2usize, 32, 256, 1 << 16, 1 << 19] {
        for width in [1, 2, 4, 8, 16] {
            assert_eq!(
                required_forward_payload_v1(rows, width).unwrap(),
                metal_goldilocks_forward_extra_payload_v1(rows, width).unwrap()
                    + forward_metadata_payload_v1()
            );
            assert!(
                prepared_goldilocks_forward_payload_bound_v1(rows, width).unwrap()
                    < metal_goldilocks_forward_extra_payload_v1(rows, width).unwrap()
            );
            let policy = MainBoundedTransformPolicyV1::for_forward_test_v1(rows, width);
            assert_eq!(policy.forward_columns_v1(rows), width);
            with_forward_group_limit_for_testing_v1(8, || {
                assert_eq!(policy.forward_columns_v1(rows), width.min(8))
            });
            assert!(policy.columns_v1(rows) <= 8);
        }
    }
    for (rows, width) in [(0, 1), (3, 1), (1 << 20, 1), (32, 0), (32, 17)] {
        assert!(required_forward_payload_v1(rows, width).is_err());
    }
    let policy = MainBoundedTransformPolicyV1::for_forward_test_v1(1 << 19, 16);
    assert_eq!(policy.forward_columns_v1(1 << 19), 16);
    let result = std::panic::catch_unwind(|| {
        with_forward_group_limit_for_testing_v1(8, || {
            assert_eq!(policy.forward_columns_v1(1 << 19), 8);
            with_forward_group_limit_for_testing_v1(16, || {
                assert_eq!(policy.forward_columns_v1(1 << 19), 16)
            });
            assert_eq!(policy.forward_columns_v1(1 << 19), 8);
            std::panic::resume_unwind(Box::new(()));
        })
    });
    assert!(result.is_err());
    assert_eq!(policy.forward_columns_v1(1 << 19), 16);
    let original = MainBoundedTransformPolicyV1::for_test_v1(1 << 19, 8);
    assert_eq!(original.forward_columns_v1(1 << 19), 8);
}

#[test]
fn forward_exact_cpu_preserves_complete_mask_tail_outputs_and_allocations() {
    for workers in [1, 4, 20] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                for width in [1, 4, 8, 16] {
                    let root = goldilocks_primitive_root_v1(6).unwrap();
                    let mut actual = folded_columns_v1(6, width);
                    let mut expected = ZeroizingBaseColumnsV1(actual.0.clone());
                    for column in &mut expected.0 {
                        goldilocks_fft_v1(column, root).unwrap();
                    }
                    let pointers = actual
                        .iter()
                        .map(|c| (c.as_ptr(), c.capacity()))
                        .collect::<Vec<_>>();
                    MainBoundedTransformPolicyV1::cpu_v1()
                        .forward_exact_v1(&mut actual.0, root, || false)
                        .unwrap();
                    assert_eq!(&*actual, &*expected);
                    assert_eq!(
                        actual
                            .iter()
                            .map(|c| (c.as_ptr(), c.capacity()))
                            .collect::<Vec<_>>(),
                        pointers
                    );
                }
            });
    }
    for rows in [2usize, 32] {
        let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, 1);
        assert_eq!(policy.forward_columns_v1(rows), 0);
        let root = goldilocks_primitive_root_v1(rows.ilog2() as u8).unwrap();
        let mut actual = ZeroizingBaseColumnsV1(vec![vec![F(7); rows]]);
        let mut expected = actual.0[0].clone();
        goldilocks_fft_v1(&mut expected, root).unwrap();
        policy
            .forward_exact_v1(&mut actual.0, root, || false)
            .unwrap();
        assert_eq!(actual.0[0], expected);
    }
}

#[test]
fn forward_transaction_refusals_preserve_all_sixteen_originals_and_uncertainty_priority() {
    let root = goldilocks_primitive_root_v1(5).unwrap();
    let original = folded_columns_v1(5, 16);
    let available = required_forward_payload_v1(32, 16).unwrap();
    for error in [
        TransformError::InvalidShape,
        TransformError::InvalidRoot,
        TransformError::NonCanonicalInput,
        TransformError::NonCanonicalOutput,
        TransformError::DeviceUnavailable,
        TransformError::DeviceFailure("drained".into()),
        TransformError::CompletionUncertain,
    ] {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let pointers = actual
            .iter()
            .map(|c| (c.as_ptr(), c.len(), c.capacity()))
            .collect::<Vec<_>>();
        let expected = map_error_v1(error.clone());
        let (result, erased) = inspection::observe_v1(|| {
            prepare_forward_and_publish_with_v1(
                &mut actual.0,
                root,
                available,
                &mut || false,
                |views, actual_root| {
                    assert_eq!(actual_root, root.0);
                    assert_eq!(views.len(), 16);
                    for (view, (pointer, length, _)) in views.iter().zip(&pointers) {
                        assert_eq!(view.as_ptr(), pointer.cast::<u64>());
                        assert_eq!(view.len(), *length);
                    }
                    Err(error)
                },
            )
        });
        assert!(matches!(result, Err(error) if error == expected));
        assert!(erased.is_empty());
        assert_eq!(&*actual, &*original);
        assert_eq!(
            actual
                .iter()
                .map(|c| (c.as_ptr(), c.len(), c.capacity()))
                .collect::<Vec<_>>(),
            pointers
        );
    }
    for mode in 0..4 {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let uncertain = core::cell::Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepare_forward_and_publish_with_v1(
                &mut actual.0,
                root,
                if mode == 0 { available - 1 } else { available },
                &mut || uncertain.get(),
                |_, _| {
                    assert_ne!(mode, 0, "budget must refuse before preparation");
                    if mode == 1 {
                        std::panic::resume_unwind(Box::new(()));
                    }
                    if mode == 2 {
                        uncertain.set(true);
                        return Err(TransformError::DeviceFailure("drained".into()));
                    }
                    Ok(Outcome::Cpu)
                },
            )
        }));
        match mode {
            1 => assert!(result.is_err()),
            2 => assert!(matches!(
                result.unwrap(),
                Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
            )),
            _ => assert!(matches!(result.unwrap(), Ok(MainNativePreparedV1::Cpu))),
        }
        assert_eq!(&*actual, &*original);
    }
    for fault in 0..7 {
        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
        let mut actual_root = root;
        match fault {
            0 => actual.0.clear(),
            1 => actual.0.push(vec![F::ZERO; 32]),
            2 => {
                actual.0[15].pop();
            }
            3 => actual.0[15][31] = F(GOLDILOCKS_MODULUS_V1),
            4 => actual_root = F::ONE,
            5 => actual_root = F(GOLDILOCKS_MODULUS_V1),
            _ => {}
        }
        let before = actual.0.clone();
        assert!(
            MainBoundedTransformPolicyV1::cpu_v1()
                .forward_exact_v1(&mut actual.0, actual_root, || fault == 6)
                .is_err()
        );
        assert_eq!(actual.0, before);
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "isolated real Metal quotient forward words8/prepared8/prepared16 full-cycle A/B; run fresh with first mode0/1/2"]
fn native19_forward_prepared_grouping_preserves_all_outputs_and_reports_full_costs() {
    use crate::privacy_engines::zk_x509::prover_observation::ObservationV1;
    use fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1;
    use std::time::{Duration, Instant};
    assert!(!cfg!(debug_assertions));
    assert_eq!(rayon::current_num_threads(), 20);
    assert_eq!(rayon::current_thread_index(), None);
    assert_eq!(
        fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal)
    );
    let first = std::env::var("X509_FORWARD_AB_FIRST_MODE")
        .map_or(0, |value| value.parse::<usize>().unwrap());
    assert!(first < 3);
    struct StopTelemetry;
    impl Drop for StopTelemetry {
        fn drop(&mut self) {
            fastpq_prover::enable_queue_depth_stats(false);
        }
    }
    fastpq_prover::enable_queue_depth_stats(true);
    let _telemetry = StopTelemetry;
    const ROUNDS: usize = 12;
    let rows = 1 << 19;
    let root = goldilocks_primitive_root_v1(19).unwrap();
    let original = folded_columns_v1(19, 16);
    let mut expected = ZeroizingBaseColumnsV1(original.0.clone());
    for column in &mut expected.0 {
        goldilocks_fft_v1(column, root).unwrap();
    }
    let policy = MainBoundedTransformPolicyV1::for_forward_test_v1(rows, 16);
    assert_eq!(policy.columns_v1(rows), 8);
    let mut elapsed = [[Duration::ZERO; ROUNDS]; 3];
    let mut allocation = [Duration::ZERO; 3];
    let mut transform = [Duration::ZERO; 3];
    let mut clear = [Duration::ZERO; 3];
    let mut stats = [fastpq_prover::ColumnStagingPhaseStats::default(); 3];
    for round in 0..=ROUNDS {
        let order = if round % 2 == 0 { [0, 1, 2] } else { [2, 1, 0] };
        for offset in order {
            let mode = (first + offset) % 3;
            fastpq_prover::take_column_staging_stats().unwrap();
            let observation = ObservationV1::begin_v1();
            let start = Instant::now();
            let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
            let allocated = start.elapsed();
            let pointers = actual
                .iter()
                .map(|c| (c.as_ptr(), c.capacity()))
                .collect::<Vec<_>>();
            let start = Instant::now();
            if mode == 0 {
                for batch in actual.0.chunks_mut(8) {
                    policy
                        .forward_with_v1(
                            batch,
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
            } else {
                with_forward_group_limit_for_testing_v1(if mode == 1 { 8 } else { 16 }, || {
                    policy
                        .forward_exact_v1(
                            &mut actual.0,
                            root,
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap()
                });
            }
            let transformed = start.elapsed();
            let captured = fastpq_prover::take_column_staging_stats().unwrap().fft();
            let receipt = observation.finish_v1().public_text_v1();
            assert!(receipt.contains("quotient_stripe_metal_forward_columns=16"));
            assert!(receipt.contains("quotient_stripe_cpu_forward_columns=0"));
            assert_eq!(captured.batches, if mode == 2 { 1 } else { 2 });
            assert_eq!(&*actual, &*expected);
            assert_eq!(
                actual
                    .iter()
                    .map(|c| (c.as_ptr(), c.capacity()))
                    .collect::<Vec<_>>(),
                pointers
            );
            let start = Instant::now();
            drop(actual);
            let cleared = start.elapsed();
            if round == 0 {
                println!(
                    "x509_forward_first_call mode={mode} first_mode={first} process_first={} total_ns={} transform_ns={} batches={} wait_ms={:.6} gpu_execution_ms={:.6} driver_ms={:.6} queue_ms={:.6} parity=all_fields_exact",
                    mode == first,
                    (allocated + transformed + cleared).as_nanos(),
                    transformed.as_nanos(),
                    captured.batches,
                    captured.wait_ms,
                    captured.gpu_execution_ms,
                    captured.cpu_driver_scheduling_ms,
                    captured.pre_gpu_queue_ms
                );
                continue;
            }
            elapsed[mode][round - 1] = allocated + transformed + cleared;
            allocation[mode] += allocated;
            transform[mode] += transformed;
            clear[mode] += cleared;
            stats[mode].batches += captured.batches;
            stats[mode].flatten_ms += captured.flatten_ms;
            stats[mode].wait_ms += captured.wait_ms;
            stats[mode].timestamped_commands += captured.timestamped_commands;
            stats[mode].gpu_execution_ms += captured.gpu_execution_ms;
            stats[mode].cpu_driver_scheduling_ms += captured.cpu_driver_scheduling_ms;
            stats[mode].pre_gpu_queue_ms += captured.pre_gpu_queue_ms;
        }
    }
    for mode in 0..3 {
        elapsed[mode].sort_unstable();
        println!(
            "x509_forward_transaction_cost mode={mode} first_mode={first} rows={rows} columns=16 group={} rounds={ROUNDS} median_ns={} total_ns={} allocation_ns={} transform_all_staging_clear_ns={} result_clear_ns={} batches={} flatten_ms={:.6} wait_ms={:.6} timestamped_commands={} gpu_execution_ms={:.6} driver_ms={:.6} queue_ms={:.6} parity=all_fields_exact pointers=unchanged caller_context=external timing_scope=successful-command-intervals-not-additive",
            if mode == 2 { 16 } else { 8 },
            elapsed[mode][ROUNDS / 2].as_nanos(),
            (allocation[mode] + transform[mode] + clear[mode]).as_nanos(),
            allocation[mode].as_nanos(),
            transform[mode].as_nanos(),
            clear[mode].as_nanos(),
            stats[mode].batches,
            stats[mode].flatten_ms,
            stats[mode].wait_ms,
            stats[mode].timestamped_commands,
            stats[mode].gpu_execution_ms,
            stats[mode].cpu_driver_scheduling_ms,
            stats[mode].pre_gpu_queue_ms
        );
    }
}
