//! Resource-boundary, allocation custody and arithmetic adapter controls.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection;
use fastpq_prover::goldilocks_transform::transform_goldilocks_columns_v1;

#[test]
fn fixed_staging_charges_word_capacities_beyond_complete_device_allowance() {
    for rows in [2, 32, 1 << 19] {
        for count in [1, 2, 4, 8] {
            let extra = metal_goldilocks_transform_extra_payload_v1(rows, count).unwrap();
            let exact = extra
                + rows * count * 8
                + count * core::mem::size_of::<Vec<u64>>()
                + core::mem::size_of::<Words>();
            assert_eq!(required_v1(rows, count).unwrap(), exact);
            let policy = MainBoundedTransformPolicyV1 {
                available: exact,
                backend: Some(Backend::Metal),
                public_powers_reserved: false,
            };
            assert_eq!(policy.columns_v1(rows), count);
            assert!(
                MainBoundedTransformPolicyV1 {
                    available: exact - 1,
                    ..policy
                }
                .columns_v1(rows)
                    < count
            );
        }
    }
    for (rows, columns) in [
        (0, 1),
        (1, 1),
        (3, 1),
        (1 << 20, 1),
        (32, 0),
        (32, 9),
        (usize::MAX, 8),
    ] {
        assert!(required_v1(rows, columns).is_err());
    }
    for backend in [None, Some(Backend::Cpu), Some(Backend::Cuda)] {
        assert_eq!(
            MainBoundedTransformPolicyV1 {
                available: usize::MAX,
                backend,
                public_powers_reserved: false,
            }
            .columns_v1(32),
            0
        );
    }
}

#[test]
fn outer_assembly_limit_and_source_reserves_remain_unchanged() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let cap =
        usize::try_from(super::super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1)
            .expect("fixture value fits usize");
    let source_limit = cap - plan.check_before_sources_v1(0).unwrap();
    let mask_scratch = core::mem::size_of::<E>();
    let link_owner =
        super::super::main_terminal_links::MainTerminalLinkPlanV1::public_owner_charge_v1();
    // Three 192-link plans, fixed transcript bytes, one Vec header and 192
    // Fp4 link coefficients remain live in the bounded-transform owner.
    assert_eq!(mask_scratch, 32);
    assert_eq!(link_owner, 3 * 192 * 64 + 64 + 24 + 192 * 32);
    assert_eq!(
        source_limit,
        (4_usize << 30)
            - main_resources::CURRENT_FULL_PROFILE_PEAK_V1
            - SHARED_POWERS_ALLOWANCE_V1
            - main_transform::PUBLIC_COSET_POWERS_ALLOWANCE_V1
    );
    let key_owner = super::super::main_key_joins::MainKeyJoinPlanV1::public_owner_charge_v1();
    assert_eq!(key_owner, 8 * 1024 * 1024);
    let union_owner = super::super::main_sha_union::MainShaUnionPlanV1::public_owner_charge_v1();
    // Three plans each retain 116 (group, column) coordinates and six
    // points, plus the 24 quotient weights and the original framing reserve.
    assert_eq!(union_owner, 3 * (116 * 16 + 6 * 8) + 24 + 24 * 32 + 1024);
    let limit = source_limit - link_owner - key_owner - union_owner;
    assert_eq!(
        limit,
        (4_usize << 30)
            - main_resources::CURRENT_FULL_PROFILE_PEAK_V1
            - main_transform::PUBLIC_COSET_POWERS_ALLOWANCE_V1
            - link_owner
            - key_owner
            - union_owner
            - SHARED_POWERS_ALLOWANCE_V1
    );
    assert_eq!(
        plan.maximum_live_buffers,
        main_resources::CURRENT_FULL_PROFILE_PEAK_V1
    );
    let policy = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 288_345_698).unwrap();
    assert_eq!(
        policy.available,
        (4_usize << 30)
            - main_resources::CURRENT_FULL_PROFILE_PEAK_V1
            - main_transform::PUBLIC_COSET_POWERS_ALLOWANCE_V1
            - 288_345_698
            - link_owner
            - key_owner
            - union_owner
            - SHARED_POWERS_ALLOWANCE_V1
    );
    assert_eq!(
        MainBoundedTransformPolicyV1 {
            backend: Some(Backend::Metal),
            ..policy
        }
        .columns_v1(1 << 19),
        4
    );
    assert_eq!(
        MainBoundedTransformPolicyV1::for_assembly_v1(&layout, limit)
            .unwrap()
            .available,
        0
    );
    assert!(MainBoundedTransformPolicyV1::for_assembly_v1(&layout, limit + 1).is_err());
    let empty_slack = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, limit).unwrap();
    assert!(empty_slack.public_powers_reserved);
    let root = goldilocks_primitive_root_v1(19).unwrap();
    let (forward, inverse) = empty_slack.cpu_power_pair_v1(1 << 19, root, true).unwrap();
    assert_eq!(
        forward.unwrap().allocated_payload_bytes_v1().unwrap()
            + inverse.unwrap().allocated_payload_bytes_v1().unwrap(),
        SHARED_POWERS_ALLOWANCE_V1
    );
}

#[test]
fn measured_assembly_admits_sixteen_forward_columns_without_borrowing_other_reserves() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let policy = MainBoundedTransformPolicyV1 {
        backend: Some(Backend::Metal),
        ..MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 215_244_730).unwrap()
    };
    let rows = 1 << 19;
    for registration in &layout.registered_segments {
        let adjusted = policy
            .for_quotient_layout_v1(
                registration.segment.base_width,
                registration.segment.aux_width,
            )
            .unwrap();
        assert_eq!(adjusted.forward_columns_v1(rows), 16);
        assert_eq!(adjusted.columns_v1(rows), 8);
        let required =
            super::super::main_native_transform::required_forward_payload_v1(rows, 16).unwrap();
        let exact = MainBoundedTransformPolicyV1 {
            available: required,
            ..adjusted
        };
        assert_eq!(exact.forward_columns_v1(rows), 16);
        assert_eq!(
            exact
                .reserve_additional_v1(1)
                .unwrap()
                .forward_columns_v1(rows),
            8
        );
        assert!(required <= adjusted.available);
    }
}

#[test]
fn staging_refuses_underfunded_and_oversized_actual_allocations_before_input_copy() {
    let need = required_v1(32, 2).unwrap();
    let mut calls = 0;
    let result = allocate_words_with_v1(32, 2, need - 1, |_, _| {
        calls += 1;
        Ok(())
    });
    assert!(matches!(result, Err(ZkX509StarkErrorV1::ProofTooLarge)));
    assert_eq!(calls, 0);
    let result = allocate_words_with_v1(32, 2, need, |column, rows| {
        column.reserve_exact(rows * 4);
        Ok(())
    });
    assert!(matches!(result, Err(ZkX509StarkErrorV1::ProofTooLarge)));
    let result = allocate_words_with_v1(32, 2, need, |_, _| Ok(()));
    assert!(matches!(result, Err(ZkX509StarkErrorV1::ProofTooLarge)));
}

#[test]
fn partial_staging_allocation_failure_and_unwind_clear_actual_owners() {
    for unwind in [false, true] {
        let (outcome, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut calls = 0;
                allocate_words_with_v1(32, 2, required_v1(32, 2).unwrap(), |column, rows| {
                    calls += 1;
                    column.reserve_exact(rows);
                    column.resize(rows, 9);
                    if calls == 2 {
                        assert!(!unwind, "injected fixed staging allocation unwind");
                        return Err(ZkX509StarkErrorV1::AllocationFailure);
                    }
                    Ok(())
                })
            })
        });
        if unwind {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(
                outcome.unwrap(),
                Err(ZkX509StarkErrorV1::AllocationFailure)
            ));
        }
        assert_eq!(erased.iter().map(|e| e.cells).sum::<usize>(), 64);
        assert!(erased.iter().any(|e| e.nonzero_before > 0));
        assert!(erased.iter().all(|e| e.nonzero_after == 0));
    }
}

#[test]
fn malformed_batches_and_uncertain_cpu_admission_never_dispatch() {
    let policy = MainBoundedTransformPolicyV1::for_test_v1(32, 2);
    let root = goldilocks_primitive_root_v1(5).unwrap();
    for mut columns in [
        vec![],
        vec![vec![F::ONE; 32]; 3],
        vec![vec![F::ONE; 31]],
        vec![vec![F(u64::MAX); 32]],
    ] {
        assert!(
            policy
                .apply_with_v1(
                    &mut columns,
                    root,
                    F(7),
                    false,
                    |_, _, _| panic!("invalid dispatch"),
                    || false
                )
                .is_err()
        );
    }
    let mut columns = vec![vec![F::ONE; 32]];
    assert!(matches!(
        policy.apply_with_v1(
            &mut columns,
            root,
            F(7),
            false,
            |_, _, _| panic!("uncertain dispatch"),
            || true
        ),
        Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
    ));
    assert_eq!(columns, vec![vec![F::ONE; 32]]);
}

#[test]
fn transform_errors_bad_output_and_uncertain_completion_never_publish_batch() {
    let policy = MainBoundedTransformPolicyV1::for_test_v1(32, 2);
    let root = goldilocks_primitive_root_v1(5).unwrap();
    for recovery in [false, true] {
        for failure in 0..6 {
            let mut columns = vec![vec![F(19); 32]; 2];
            let original = columns.clone();
            let uncertain = std::cell::Cell::new(false);
            let (result, erased) = inspection::observe_v1(|| {
                policy.apply_with_v1(
                    &mut columns,
                    root,
                    F(7),
                    recovery,
                    |words, root, direction| match failure {
                        0 => {
                            words[0][0] = 77;
                            Err(TransformError::DeviceUnavailable)
                        }
                        1 => {
                            words[0][0] = u64::MAX;
                            Ok(Backend::Metal)
                        }
                        2 => Ok(Backend::Cuda),
                        3 => {
                            uncertain.set(true);
                            Ok(Backend::Metal)
                        }
                        4 => {
                            let _ = words[0].pop();
                            Ok(Backend::Metal)
                        }
                        _ => {
                            if direction == Direction::Forward {
                                Err(TransformError::CompletionUncertain)
                            } else {
                                transform_goldilocks_columns_v1(
                                    words,
                                    root,
                                    direction,
                                    fastpq_prover::ExecutionMode::Cpu,
                                )
                            }
                        }
                    },
                    || uncertain.get(),
                )
            });
            assert!(result.is_err());
            assert_eq!(columns, original);
            assert_eq!(erased.iter().map(|e| e.cells).sum::<usize>(), 64);
            assert!(erased.iter().all(|e| e.nonzero_after == 0));
        }
    }
}

#[test]
fn quotient_metadata_and_log19_device_payload_fit_only_unreserved_bytes() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let policy = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 288_345_698).unwrap();
    // Independent live-owner census: one host word matrix and one shared
    // staging matrix, worst-case page padding, the whole idle pool, all 64
    // factorized twiddle cache entries plus construction/returned owners, and
    // dispatch metadata. These are logical payloads, not a measured RSS bound.
    let words = (1_usize << 19) * 8 * 8;
    let factorized_twiddles = (64 + 2) * (4 * 256) * 8;
    let expected = 2 * words
        + 8 * (16 * 1024 - 1)
        + 64 * 1024 * 1024
        + factorized_twiddles
        + (1 << 20)
        + 8 * core::mem::size_of::<Vec<u64>>()
        + core::mem::size_of::<Words>();
    assert_eq!(expected, 135_938_264);
    assert_eq!(required_v1(1 << 19, 8).unwrap(), expected);
    assert_eq!(factorized_twiddles - (64 + 2) * 32 * 8, 523_776);
    for registration in &layout.registered_segments {
        let adjusted = MainBoundedTransformPolicyV1 {
            backend: Some(Backend::Metal),
            ..policy
        }
        .for_quotient_layout_v1(
            registration.segment.base_width,
            registration.segment.aux_width,
        )
        .unwrap();
        assert!(adjusted.available < policy.available);
        assert_eq!(adjusted.columns_v1(1 << 19), 4);
        assert!(required_v1(1 << 19, 4).unwrap() <= adjusted.available);
        assert!(required_v1(1 << 19, 8).unwrap() > adjusted.available);
        let metadata = policy.available - adjusted.available;
        assert!(
            metadata
                >= (registration.segment.base_width + registration.segment.aux_width)
                    * core::mem::size_of::<Vec<F>>()
        );
        assert!(
            MainBoundedTransformPolicyV1 {
                available: metadata - 1,
                ..policy
            }
            .for_quotient_layout_v1(
                registration.segment.base_width,
                registration.segment.aux_width
            )
            .is_err()
        );
    }
    assert!(policy.for_quotient_layout_v1(usize::MAX, 1).is_err());
}

#[test]
fn private_forward_adapter_matches_horner_and_preserves_separate_backend_counters() {
    use crate::privacy_engines::zk_x509::prover_observation::ObservationV1;
    let full_root = goldilocks_primitive_root_v1(7).unwrap();
    let stripe = main_quotient_stripes::MainQuotientStripeV1 {
        rows: 32,
        count: 4,
        ordinal: 3,
        next_stride: 2,
        root: full_root.pow(4),
        shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(3)),
    };
    for admitted in [0, 1, 2, 4, 8] {
        for completed in [Backend::Cpu, Backend::Metal] {
            let policy = if admitted == 0 {
                MainBoundedTransformPolicyV1::cpu_v1()
            } else {
                MainBoundedTransformPolicyV1::for_test_v1(32, admitted)
            };
            let coefficients = (0..8)
                .map(|column| {
                    (0..113)
                        .map(|degree| {
                            F::reduce(
                                u128::try_from(column + 3 * degree * degree + 7)
                                    .expect("positive fixture coefficient"),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut columns = vec![vec![F::ZERO; 32]; 8];
            for (target, source) in columns.iter_mut().zip(&coefficients) {
                stripe.fold_into_v1(source, target).unwrap();
            }
            let pointers = columns.iter().map(Vec::as_ptr).collect::<Vec<_>>();
            let observation = ObservationV1::begin_v1();
            let mut calls = 0;
            let (result, erased) = inspection::observe_v1(|| {
                policy.forward_with_v1(
                    &mut columns,
                    stripe.root,
                    |words, root, direction| {
                        assert!(admitted > 0 && words.len() <= admitted);
                        assert_eq!(direction, Direction::Forward);
                        calls += 1;
                        // Injection exercises adapter receipts; this is not a Metal claim.
                        transform_goldilocks_columns_v1(
                            words,
                            root,
                            direction,
                            fastpq_prover::ExecutionMode::Cpu,
                        )?;
                        Ok(completed)
                    },
                    || false,
                )
            });
            result.unwrap();
            let receipt = observation.finish_v1().public_text_v1();
            let metal = admitted > 0 && completed == Backend::Metal;
            assert!(receipt.contains(&format!(
                "quotient_stripe_metal_forward_columns={}",
                if metal { 8 } else { 0 }
            )));
            assert!(receipt.contains(&format!(
                "quotient_stripe_cpu_forward_columns={}",
                if metal { 0 } else { 8 }
            )));
            assert!(receipt.contains("fixed_coset_metal_forward_columns=0"));
            assert!(receipt.contains("fixed_coset_cpu_forward_columns=0"));
            assert_eq!(calls, if admitted == 0 { 0 } else { 8 / admitted });
            assert_eq!(
                erased.iter().map(|item| item.cells).sum::<usize>(),
                if admitted == 0 { 0 } else { 8 * 32 }
            );
            assert!(erased.iter().all(|item| item.nonzero_after == 0));
            assert_eq!(
                columns.iter().map(Vec::as_ptr).collect::<Vec<_>>(),
                pointers
            );
            for (column, source) in columns.iter().zip(&coefficients) {
                for (row, actual) in column.iter().enumerate() {
                    let x = stripe.shift.mul(stripe.root.pow(row as u128));
                    let expected = source
                        .iter()
                        .rev()
                        .fold(F::ZERO, |value, coefficient| value.mul(x).add(*coefficient));
                    assert_eq!(*actual, expected);
                }
            }
        }
    }
}

#[test]
fn private_forward_rejects_invalid_inputs_and_uncertain_cpu_fallback() {
    let root = goldilocks_primitive_root_v1(5).unwrap();
    for policy in [
        MainBoundedTransformPolicyV1::cpu_v1(),
        MainBoundedTransformPolicyV1::for_test_v1(32, 2),
    ] {
        for (mut columns, root) in [
            (vec![], root),
            (vec![vec![F::ONE; 32]; 9], root),
            (vec![vec![F::ONE; 31]], root),
            (vec![vec![F(u64::MAX); 32]], root),
            (vec![vec![F::ONE; 32]], F::ONE),
            (vec![vec![F::ONE; 32]], F(u64::MAX)),
        ] {
            let before = columns.clone();
            assert!(
                policy
                    .forward_with_v1(
                        &mut columns,
                        root,
                        |_, _, _| panic!("invalid dispatch"),
                        || false
                    )
                    .is_err()
            );
            assert_eq!(columns, before);
        }
        let mut columns = vec![vec![F(13); 32]; 2];
        assert!(matches!(
            policy.forward_with_v1(
                &mut columns,
                root,
                |_, _, _| panic!("uncertain dispatch"),
                || true
            ),
            Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
        ));
        assert_eq!(columns, vec![vec![F(13); 32]; 2]);
    }
}

#[test]
fn private_forward_failure_unwind_and_wrong_receipts_clear_staging_without_copyback() {
    let policy = MainBoundedTransformPolicyV1::for_test_v1(32, 2);
    let root = goldilocks_primitive_root_v1(5).unwrap();
    for failure in 0..8 {
        let mut columns = vec![vec![F(23); 32]; 2];
        let before = columns.clone();
        let uncertain = std::cell::Cell::new(false);
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                policy.forward_with_v1(
                    &mut columns,
                    root,
                    |words, _, _| {
                        words[0][0] = 29;
                        match failure {
                            0 => Err(TransformError::DeviceFailure("injected".to_owned())),
                            1 => {
                                words[0][0] = u64::MAX;
                                Ok(Backend::Metal)
                            }
                            2 => {
                                let _ = words[0].pop();
                                Ok(Backend::Metal)
                            }
                            3 => Ok(Backend::Cuda),
                            4 => {
                                uncertain.set(true);
                                Ok(Backend::Cpu)
                            }
                            5 => Err(TransformError::CompletionUncertain),
                            6 => {
                                words[0].reserve_exact(32);
                                Ok(Backend::Metal)
                            }
                            _ => panic!("injected private transform unwind"),
                        }
                    },
                    || uncertain.get(),
                )
            }))
        });
        if failure == 7 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(columns, before);
        assert_eq!(
            erased.iter().map(|item| item.cells).sum::<usize>(),
            if failure == 6 { 96 } else { 64 }
        );
        assert!(erased.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
#[ignore = "requires a normally built Metal privacy binary; component parity, not full-proof qualification"]
fn private_quotient_log19_width8_required_metal_matches_full_cpu_output_with_high_tail() {
    use fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1;
    let rows = 1_usize << 19;
    let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(19, 22, 7).unwrap();
    let mut expected = ZeroizingBaseColumnsV1(Vec::new());
    for column in 0..8 {
        let mut coefficients = ZeroizingMainTraceColumnV1(vec![F::ZERO; rows + 1816]);
        for (degree, value) in [(0, 3), (1, 5), (rows - 1, 7), (rows, 11), (rows + 1815, 13)] {
            coefficients[degree] = F(value + column);
        }
        let mut folded = ZeroizingMainTraceColumnV1(vec![F::ZERO; rows]);
        stripe.fold_into_v1(&coefficients, &mut folded).unwrap();
        expected.0.push(folded.into_vec_v1());
    }
    let mut actual = ZeroizingBaseColumnsV1(expected.0.clone());
    MainBoundedTransformPolicyV1::cpu_v1()
        .forward_with_v1(
            &mut expected.0,
            stripe.root,
            |_, _, _| panic!("explicit CPU path"),
            goldilocks_transform_completion_uncertain_v1,
        )
        .unwrap();
    let mut metal_columns = 0;
    MainBoundedTransformPolicyV1::for_test_v1(rows, 8)
        .forward_with_v1(
            &mut actual.0,
            stripe.root,
            |words, root, direction| {
                let completed = transform_goldilocks_columns_v1(
                    words,
                    root,
                    direction,
                    fastpq_prover::ExecutionMode::Gpu,
                )?;
                assert_eq!(
                    completed,
                    Backend::Metal,
                    "a required genuine Metal receipt is mandatory"
                );
                metal_columns += words.len();
                Ok(completed)
            },
            goldilocks_transform_completion_uncertain_v1,
        )
        .unwrap();
    assert_eq!(metal_columns, 8);
    assert_eq!(&*actual, &*expected);
}

#[test]
#[ignore = "isolated registered native inverse CPU/actual Metal timing; component evidence only"]
fn native_inverse_registered_cpu_and_metal_cost_keep_original_values() {
    use fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1;
    use std::time::Instant;

    assert_eq!(
        fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal),
    );
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let admitted = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 0)
        .unwrap()
        .for_native_replay_v1()
        .unwrap();
    for native in [5, 8, 15, 16, 18, 19] {
        let rows = 1usize << native;
        let root = goldilocks_primitive_root_v1(native).unwrap();
        for width in [1, 8] {
            let original = ZeroizingBaseColumnsV1(
                (0..width)
                    .map(|column| {
                        (0..rows)
                            .map(|row| F((17 * row + 13 * column + 1) as u64))
                            .collect()
                    })
                    .collect(),
            );
            let mut expected = ZeroizingBaseColumnsV1(original.0.clone());
            MainBoundedTransformPolicyV1::cpu_v1()
                .inverse_with_v1(
                    &mut expected.0,
                    root,
                    |_, _, _| panic!("reference is CPU-only"),
                    goldilocks_transform_completion_uncertain_v1,
                )
                .unwrap();
            for round in 0..3 {
                for device in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let policy = MainBoundedTransformPolicyV1 {
                        backend: if device { Some(Backend::Metal) } else { None },
                        ..admitted
                    };
                    let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
                    let started = Instant::now();
                    let mut metal_columns = 0;
                    policy
                        .inverse_with_v1(
                            &mut actual.0,
                            root,
                            |words, root, direction| {
                                assert!(device);
                                let backend = transform_goldilocks_columns_v1(
                                    words,
                                    root,
                                    direction,
                                    fastpq_prover::ExecutionMode::Gpu,
                                )?;
                                assert_eq!(backend, Backend::Metal);
                                metal_columns += words.len();
                                Ok(backend)
                            },
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap();
                    let transform_ns = started.elapsed().as_nanos();
                    assert_eq!(metal_columns, if device { width } else { 0 });
                    assert_eq!(&*actual, &*expected);
                    let clear_started = Instant::now();
                    drop(actual);
                    let clear_ns = clear_started.elapsed().as_nanos();
                    println!(
                        "x509_native_inverse_cost native_log={native} width={width} workers={} round={round} device={device} transform_scratch_clear_ns={transform_ns} result_clear_ns={clear_ns} parity=exact",
                        rayon::current_num_threads(),
                    );
                }
            }
        }
    }
}

#[test]
fn native_replay_metadata_respects_every_phase_residual_without_source_discharge() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let original = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 288_345_698).unwrap();
    let policy = MainBoundedTransformPolicyV1 {
        backend: Some(Backend::Metal),
        ..original
    };
    let adjusted = policy.for_native_replay_v1().unwrap();
    let metadata = policy.available - adjusted.available;
    assert!(metadata >= 5 * 8 * core::mem::size_of::<Vec<F>>());
    assert!(metadata < 4096);
    assert_eq!(adjusted.columns_v1(1 << 19), 4);
    assert!(
        MainBoundedTransformPolicyV1 {
            available: metadata - 1,
            ..policy
        }
        .for_native_replay_v1()
        .is_err()
    );
    assert_eq!(
        MainBoundedTransformPolicyV1 {
            available: metadata,
            ..policy
        }
        .for_native_replay_v1()
        .unwrap()
        .columns_v1(1 << 19),
        0
    );
    for registration in &layout.registered_segments {
        let quotient = policy
            .for_quotient_layout_v1(
                registration.segment.base_width,
                registration.segment.aux_width,
            )
            .unwrap();
        let native = quotient.for_native_replay_v1().unwrap();
        assert_eq!(quotient.available - native.available, metadata);
        assert_eq!(native.columns_v1(1 << 19), 4);
        assert!(required_v1(1 << 19, 4).unwrap() <= native.available);
        assert!(required_v1(1 << 19, 8).unwrap() > native.available);
    }
    assert_eq!(
        original.available,
        (4_usize << 30)
            - main_resources::CURRENT_FULL_PROFILE_PEAK_V1
            - main_transform::PUBLIC_COSET_POWERS_ALLOWANCE_V1
            - 288_345_698
            - super::super::main_terminal_links::MainTerminalLinkPlanV1::public_owner_charge_v1()
            - super::super::main_key_joins::MainKeyJoinPlanV1::public_owner_charge_v1()
            - super::super::main_sha_union::MainShaUnionPlanV1::public_owner_charge_v1()
            - SHARED_POWERS_ALLOWANCE_V1
    );
}

#[test]
fn inverse_only_adapter_matches_independent_dft_without_forward_or_diagonal() {
    let root = goldilocks_primitive_root_v1(5).unwrap();
    let inverse_root = root.inv().unwrap();
    let inverse_n = F(32).inv().unwrap();
    let original =
        (0..8)
            .map(|column| {
                (0..32)
                    .map(|row| {
                        F(u64::try_from(column * 101 + row * row + 3)
                            .expect("fixture value fits u64"))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
    let expected = original
        .iter()
        .map(|values| {
            (0..32)
                .map(|degree| {
                    values
                        .iter()
                        .enumerate()
                        .fold(F::ZERO, |sum, (row, &value)| {
                            sum.add(value.mul(inverse_root.pow((degree * row) as u128)))
                        })
                        .mul(inverse_n)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for admitted in [0, 1, 2, 4, 8] {
        let policy = if admitted == 0 {
            MainBoundedTransformPolicyV1::cpu_v1()
        } else {
            MainBoundedTransformPolicyV1::for_test_v1(32, admitted)
        };
        for completed in [Backend::Cpu, Backend::Metal] {
            let mut values = original.clone();
            let pointers = values.iter().map(Vec::as_ptr).collect::<Vec<_>>();
            let observation =
                crate::privacy_engines::zk_x509::prover_observation::ObservationV1::begin_v1();
            let mut calls = 0;
            policy
                .inverse_with_v1(
                    &mut values,
                    root,
                    |words, root, direction| {
                        calls += 1;
                        assert!(admitted > 0 && words.len() <= admitted);
                        assert_eq!(direction, Direction::Inverse);
                        transform_goldilocks_columns_v1(
                            words,
                            root,
                            direction,
                            fastpq_prover::ExecutionMode::Cpu,
                        )?;
                        Ok(completed) // Injected receipt only.
                    },
                    || false,
                )
                .unwrap();
            let receipt = observation.finish_v1().public_text_v1();
            assert_eq!(values, expected);
            assert_eq!(values.iter().map(Vec::as_ptr).collect::<Vec<_>>(), pointers);
            assert_eq!(calls, if admitted == 0 { 0 } else { 8 / admitted });
            let metal = admitted > 0 && completed == Backend::Metal;
            assert!(receipt.contains(&format!(
                "native_replay_cpu_inverse_columns={}",
                if metal { 0 } else { 8 }
            )));
            assert!(receipt.contains(&format!(
                "native_replay_metal_inverse_columns={}",
                if metal { 8 } else { 0 }
            )));
            assert!(receipt.contains("quotient_stripe_cpu_forward_columns=0"));
            assert!(receipt.contains("quotient_stripe_metal_forward_columns=0"));
            assert!(receipt.contains("fixed_coset_metal_inverse_columns=0"));
        }
    }
}

#[test]
#[ignore = "isolated registered forward CPU/shared-powers versus actual Metal median timings"]
fn native_forward_registered_cpu_and_metal_cost_keep_original_values() {
    use std::time::Instant;

    use fastpq_prover::goldilocks_transform::{
        available_goldilocks_transform_backend_v1, goldilocks_transform_completion_uncertain_v1,
    };

    assert!(!cfg!(debug_assertions), "use an optimized build");
    assert_eq!(
        available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal)
    );
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let admitted = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 215_244_730)
        .unwrap()
        .for_native_replay_v1()
        .unwrap();
    for native in [15, 16, 18, 19] {
        let rows = 1usize << native;
        let root = goldilocks_primitive_root_v1(native).unwrap();
        for width in [1, 8] {
            let admitted_device_batch_width = admitted.columns_v1(rows).min(width);
            assert_eq!(admitted_device_batch_width, width);
            let original = ZeroizingBaseColumnsV1(
                (0..width)
                    .map(|column| {
                        (0..rows)
                            .map(|row| F((17 * row + 13 * column + 1) as u64))
                            .collect()
                    })
                    .collect(),
            );
            let mut expected = ZeroizingBaseColumnsV1(original.0.clone());
            MainBoundedTransformPolicyV1::cpu_v1()
                .forward_with_v1(
                    &mut expected.0,
                    root,
                    |_, _, _| panic!("independent reference is CPU-only"),
                    goldilocks_transform_completion_uncertain_v1,
                )
                .unwrap();
            let mut elapsed = [[0_u128; 3]; 2];
            for round in 0..3 {
                for device in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let policy = MainBoundedTransformPolicyV1 {
                        backend: if device { Some(Backend::Metal) } else { None },
                        ..admitted
                    };
                    let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
                    let started = Instant::now();
                    let mut metal_columns = 0;
                    let mut actual_device_batch_width = 0;
                    policy
                        .forward_with_v1(
                            &mut actual.0,
                            root,
                            |words, root, direction| {
                                assert!(device);
                                assert_eq!(direction, Direction::Forward);
                                let backend = transform_goldilocks_columns_v1(
                                    words,
                                    root,
                                    direction,
                                    fastpq_prover::ExecutionMode::Gpu,
                                )?;
                                assert_eq!(backend, Backend::Metal);
                                metal_columns += words.len();
                                actual_device_batch_width =
                                    actual_device_batch_width.max(words.len());
                                Ok(backend)
                            },
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap();
                    elapsed[usize::from(device)][round] = started.elapsed().as_nanos();
                    assert_eq!(metal_columns, if device { width } else { 0 });
                    assert_eq!(
                        actual_device_batch_width,
                        if device {
                            admitted_device_batch_width
                        } else {
                            0
                        }
                    );
                    assert_eq!(&*actual, &*expected);
                    println!(
                        "x509_native_forward_cost native_log={native} width={width} workers={} round={round} device={device} admitted_device_batch_width={admitted_device_batch_width} actual_device_batch_width={actual_device_batch_width} transform_scratch_clear_ns={} parity=exact",
                        rayon::current_num_threads(),
                        elapsed[usize::from(device)][round],
                    );
                }
            }
            for mode in &mut elapsed {
                mode.sort_unstable();
            }
            println!(
                "x509_native_forward_median native_log={native} width={width} workers={} admitted_device_batch_width={admitted_device_batch_width} cpu_ns={} metal_ns={} cpu_speedup={:.3} parity=exact",
                rayon::current_num_threads(),
                elapsed[0][1],
                elapsed[1][1],
                elapsed[1][1] as f64 / elapsed[0][1] as f64,
            );
        }
    }
}

#[test]
fn host_tiles_preserve_copy_scaling_validation_and_allocations_across_worker_counts() {
    use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    // A partial final tile exercises the host helpers independently of FFT shape.
    let rows = HOST_PARALLEL_ROWS_V1 + 17;
    let original = (0..3)
        .map(|column| {
            (0..rows)
                .map(|row| F((column * rows + row) as u64 * 31 + 17))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            assert!(canonical_field_columns_v1(&original));
            for diagonal in [F::ONE, F(7), goldilocks_primitive_root_v1(19).unwrap()] {
                let mut words = vec![vec![0; rows]; original.len()];
                let word_storage = words
                    .iter()
                    .map(|column| (column.as_ptr(), column.capacity()))
                    .collect::<Vec<_>>();
                copy_fields_to_words_v1(&mut words, &original);
                assert!(canonical_word_columns_v1(&words));
                let mut expected = words.clone();
                for column in &mut expected {
                    let mut power = F::ONE;
                    for value in column {
                        *value = F(*value).mul(power).0;
                        power = power.mul(diagonal);
                    }
                }
                scale_word_columns_v1(&mut words, diagonal);
                assert_eq!(words, expected);
                assert_eq!(
                    words
                        .iter()
                        .map(|column| (column.as_ptr(), column.capacity()))
                        .collect::<Vec<_>>(),
                    word_storage
                );
                let mut fields = vec![vec![F::ZERO; rows]; original.len()];
                let field_storage = fields
                    .iter()
                    .map(|column| (column.as_ptr(), column.capacity()))
                    .collect::<Vec<_>>();
                copy_words_to_fields_v1(&mut fields, &words);
                assert_eq!(
                    fields
                        .iter()
                        .flatten()
                        .map(|value| value.0)
                        .collect::<Vec<_>>(),
                    expected.into_iter().flatten().collect::<Vec<_>>()
                );
                assert_eq!(
                    fields
                        .iter()
                        .map(|column| (column.as_ptr(), column.capacity()))
                        .collect::<Vec<_>>(),
                    field_storage
                );
                for row in [0, HOST_TILE_ROWS_V1 - 1, HOST_TILE_ROWS_V1, rows - 1] {
                    let field = fields[2][row];
                    fields[2][row] = F(GOLDILOCKS_MODULUS_V1);
                    assert!(!canonical_field_columns_v1(&fields));
                    fields[2][row] = field;
                    let word = words[2][row];
                    words[2][row] = u64::MAX;
                    assert!(!canonical_word_columns_v1(&words));
                    words[2][row] = word;
                }
            }
        });
    }
}

#[test]
fn tiled_transform_failure_keeps_original_columns_and_clears_complete_staging() {
    let rows = HOST_PARALLEL_ROWS_V1;
    let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, 2);
    let root = goldilocks_primitive_root_v1(16).unwrap();
    let mut columns = vec![vec![F(17); rows]; 2];
    let original = columns.clone();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let (result, erased) = pool.install(|| {
        inspection::observe_v1(|| {
            policy.apply_with_v1(
                &mut columns,
                root,
                F(7),
                false,
                |words, _, _| {
                    words[1][rows - 1] = u64::MAX;
                    Ok(Backend::Metal)
                },
                || false,
            )
        })
    });
    assert_eq!(result, Err(ZkX509StarkErrorV1::NonCanonicalField));
    assert_eq!(columns, original);
    assert_eq!(
        erased.iter().map(|entry| entry.cells).sum::<usize>(),
        2 * rows
    );
    assert!(erased.iter().any(|entry| entry.nonzero_before != 0));
    assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
}

#[test]
fn scoped_host_mode_restores_default_after_success_error_and_unwind() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    pool.install(|| {
        assert_eq!(HOST_TILES_OVERRIDE_V1.with(|value| value.get()), None);
        assert!(use_host_tiles_v1(HOST_PARALLEL_ROWS_V1));
        with_host_tiles_for_testing_v1(false, || {
            assert!(!use_host_tiles_v1(HOST_PARALLEL_ROWS_V1));
            with_host_tiles_for_testing_v1(true, || {
                assert!(use_host_tiles_v1(HOST_PARALLEL_ROWS_V1));
                assert!(!use_host_tiles_v1(HOST_PARALLEL_ROWS_V1 - 1));
            });
            assert!(!use_host_tiles_v1(HOST_PARALLEL_ROWS_V1));
        });
        assert_eq!(HOST_TILES_OVERRIDE_V1.with(|value| value.get()), None);
        let result: Result<(), ()> = with_host_tiles_for_testing_v1(false, || Err(()));
        assert!(result.is_err());
        assert_eq!(HOST_TILES_OVERRIDE_V1.with(|value| value.get()), None);
        assert!(
            std::panic::catch_unwind(|| {
                with_host_tiles_for_testing_v1(false, || panic!("diagnostic scope unwind"))
            })
            .is_err()
        );
        assert_eq!(HOST_TILES_OVERRIDE_V1.with(|value| value.get()), None);
        assert!(use_host_tiles_v1(HOST_PARALLEL_ROWS_V1));
    });
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| {
            with_host_tiles_for_testing_v1(true, || {
                assert!(!use_host_tiles_v1(HOST_PARALLEL_ROWS_V1));
            });
        });
}

#[test]
fn both_host_modes_keep_original_columns_and_clear_staging_after_error_and_unwind() {
    let rows = HOST_PARALLEL_ROWS_V1;
    let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, 2);
    let root = goldilocks_primitive_root_v1(16).unwrap();
    let original = ZeroizingBaseColumnsV1(vec![vec![F(17); rows]; 2]);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    pool.install(|| {
        for tiled in [false, true] {
            for unwind in [false, true] {
                let mut columns = ZeroizingBaseColumnsV1(original.0.clone());
                let (result, erased) = inspection::observe_v1(|| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        with_host_tiles_for_testing_v1(tiled, || {
                            policy.apply_with_v1(
                                &mut columns.0,
                                root,
                                F(7),
                                false,
                                |words, _, _| {
                                    words[1][rows - 1] = u64::MAX;
                                    assert!(!unwind, "diagnostic adapter unwind");
                                    Ok(Backend::Metal)
                                },
                                || false,
                            )
                        })
                    }))
                });
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), Err(ZkX509StarkErrorV1::NonCanonicalField));
                }
                assert_eq!(&*columns, &*original);
                assert_eq!(HOST_TILES_OVERRIDE_V1.with(|value| value.get()), None);
                assert_eq!(
                    erased.iter().map(|entry| entry.cells).sum::<usize>(),
                    2 * rows
                );
                assert!(erased.iter().any(|entry| entry.nonzero_before != 0));
                assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
            }
        }
    });
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "isolated actual Metal host-mode A/B with process-wide telemetry; run alone in release"]
fn native19_metal_host_modes_preserve_full_outputs_and_report_staging_costs() {
    native19_metal_host_modes_with_caller_v1(false);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "external-caller Metal host-mode A/B; run alone in a fresh release process with RAYON_NUM_THREADS=20"]
fn native19_metal_external_caller_host_modes_preserve_full_outputs_and_report_staging_costs() {
    native19_metal_host_modes_with_caller_v1(true);
}

#[cfg(target_os = "macos")]
fn native19_metal_host_modes_with_caller_v1(external_caller: bool) {
    use std::time::{Duration, Instant};

    use fastpq_prover::goldilocks_transform::{
        available_goldilocks_transform_backend_v1, goldilocks_transform_completion_uncertain_v1,
    };

    assert!(!cfg!(debug_assertions), "use an optimized build");
    if external_caller {
        assert_eq!(
            std::env::var("RAYON_NUM_THREADS").as_deref(),
            Ok("20"),
            "run the external-caller diagnostic alone in a fresh process with RAYON_NUM_THREADS=20"
        );
        assert_eq!(rayon::current_thread_index(), None);
        assert_eq!(rayon::current_num_threads(), 20);
    }
    assert_eq!(
        available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal),
        "actual Metal is required; do not substitute CPU timing"
    );
    // These public counters are process-wide. This ignored diagnostic must run
    // alone, and the guard disables capture on error or unwind as well.
    struct StopTelemetry;
    impl Drop for StopTelemetry {
        fn drop(&mut self) {
            fastpq_prover::enable_queue_depth_stats(false);
        }
    }
    fastpq_prover::enable_queue_depth_stats(true);
    let _telemetry = StopTelemetry;
    let run = || {
        const ROUNDS: usize = 32;
        let caller_context = if external_caller {
            "external_test_thread"
        } else {
            "rayon_worker"
        };
        assert_eq!(rayon::current_thread_index().is_none(), external_caller);
        assert_eq!(rayon::current_num_threads(), 20);
        println!(
            "x509_native19_metal_host_modes caller_context={caller_context} workers=20 calls_per_mode={ROUNDS} private_input=false"
        );
        let rows = 1 << 19;
        let root = goldilocks_primitive_root_v1(19).unwrap();
        for width in [1, 4, 8] {
            let original = ZeroizingBaseColumnsV1(
                (0..width)
                    .map(|column| {
                        (0..rows)
                            .map(|row| F((31 * row + 17 * column + 1) as u64))
                            .collect()
                    })
                    .collect(),
            );
            let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, width);
            assert_eq!(policy.columns_v1(rows), width);
            for fixed_forward in [false, true] {
                let operation = if fixed_forward {
                    "fixed_forward_diagonal"
                } else {
                    "native_inverse"
                };
                let mut expected = ZeroizingBaseColumnsV1(original.0.clone());
                if fixed_forward {
                    // Independent full scalar diagonal and CPU FFT reference.
                    for column in &mut expected.0 {
                        let mut power = F::ONE;
                        for value in column {
                            *value = value.mul(power);
                            power = power.mul(F(7));
                        }
                    }
                    MainBoundedTransformPolicyV1::cpu_v1()
                        .forward_with_v1(
                            &mut expected.0,
                            root,
                            |_, _, _| panic!("reference is CPU-only"),
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap();
                } else {
                    MainBoundedTransformPolicyV1::cpu_v1()
                        .inverse_with_v1(
                            &mut expected.0,
                            root,
                            |_, _, _| panic!("reference is CPU-only"),
                            goldilocks_transform_completion_uncertain_v1,
                        )
                        .unwrap();
                }
                let mut elapsed = [[Duration::ZERO; ROUNDS]; 2];
                let mut allocation = [Duration::ZERO; 2];
                let mut adapter = [Duration::ZERO; 2];
                let mut clearing = [Duration::ZERO; 2];
                let mut staging = [fastpq_prover::ColumnStagingPhaseStats::default(); 2];
                // One untimed warm-up for each mode precedes 32 complete calls
                // per mode. Alternate order to distribute thermal/load drift.
                for round in 0..=ROUNDS {
                    for tiled in if round % 2 == 0 {
                        [false, true]
                    } else {
                        [true, false]
                    } {
                        fastpq_prover::take_column_staging_stats().unwrap();
                        let allocated_at = Instant::now();
                        let mut actual = ZeroizingBaseColumnsV1(original.0.clone());
                        let allocation_elapsed = allocated_at.elapsed();
                        let transformed_at = Instant::now();
                        let mut metal_columns = 0;
                        with_host_tiles_for_testing_v1(tiled, || {
                            let mut transform = |words: &mut [Vec<u64>], root, direction| {
                                let backend = transform_goldilocks_columns_v1(
                                    words,
                                    root,
                                    direction,
                                    fastpq_prover::ExecutionMode::Gpu,
                                )?;
                                assert_eq!(backend, Backend::Metal);
                                metal_columns += words.len();
                                Ok(backend)
                            };
                            if fixed_forward {
                                policy.apply_with_v1(
                                    &mut actual.0,
                                    root,
                                    F(7),
                                    false,
                                    &mut transform,
                                    goldilocks_transform_completion_uncertain_v1,
                                )
                            } else {
                                policy.inverse_with_v1(
                                    &mut actual.0,
                                    root,
                                    &mut transform,
                                    goldilocks_transform_completion_uncertain_v1,
                                )
                            }
                        })
                        .unwrap();
                        // The unchanged adapter has already cleared its entire
                        // private staging allocation before this timestamp.
                        let adapter_elapsed = transformed_at.elapsed();
                        let captured = fastpq_prover::take_column_staging_stats().unwrap().fft();
                        assert_eq!(metal_columns, width);
                        assert_eq!(HOST_TILES_OVERRIDE_V1.with(|value| value.get()), None);
                        // Compare every field outside measured work, then time
                        // normal clearing and deallocation of the result owner.
                        assert_eq!(&*actual, &*expected);
                        let cleared_at = Instant::now();
                        drop(actual);
                        let clearing_elapsed = cleared_at.elapsed();
                        if round != 0 {
                            let mode = usize::from(tiled);
                            allocation[mode] += allocation_elapsed;
                            adapter[mode] += adapter_elapsed;
                            clearing[mode] += clearing_elapsed;
                            elapsed[mode][round - 1] =
                                allocation_elapsed + adapter_elapsed + clearing_elapsed;
                            staging[mode].batches += captured.batches;
                            staging[mode].flatten_ms += captured.flatten_ms;
                            staging[mode].wait_ms += captured.wait_ms;
                        }
                    }
                }
                for tiled in [false, true] {
                    let mode = usize::from(tiled);
                    elapsed[mode].sort_unstable();
                    let total = allocation[mode] + adapter[mode] + clearing[mode];
                    println!(
                        "x509_native19_metal_host_mode caller_context={caller_context} operation={operation} width={width} workers={} tiled={tiled} calls={ROUNDS} total_ns={} median_ns={} min_ns={} max_ns={} allocation_ns={} adapter_and_staging_clear_ns={} result_clear_ns={} staging_batches={} flatten_ms={:.6} wait_ms={:.6} parity=all_fields_exact",
                        rayon::current_num_threads(),
                        total.as_nanos(),
                        elapsed[mode][ROUNDS / 2].as_nanos(),
                        elapsed[mode][0].as_nanos(),
                        elapsed[mode][ROUNDS - 1].as_nanos(),
                        allocation[mode].as_nanos(),
                        adapter[mode].as_nanos(),
                        clearing[mode].as_nanos(),
                        staging[mode].batches,
                        staging[mode].flatten_ms,
                        staging[mode].wait_ms,
                    );
                }
            }
        }
    };
    if external_caller {
        run();
    } else {
        rayon::ThreadPoolBuilder::new()
            .num_threads(20)
            .build()
            .unwrap()
            .install(run);
    }
}

#[test]
#[ignore = "native19 host copy/validation/scaling cost only; no complete-proof performance claim"]
fn native19_host_tiles_cost_preserves_full_outputs() {
    use std::time::Instant;
    let rows = 1 << 19;
    let diagonal = F(7);
    let original = (0..8)
        .map(|column| {
            (0..rows)
                .map(|row| F((column * rows + row) as u64 * 31 + 17))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let _initialized_workers = rayon::current_num_threads();
    for width in [1, 4, 8] {
        for round in 0..4 {
            let mut reference_words = vec![vec![0; rows]; width];
            let mut reference_output = vec![vec![F::ZERO; rows]; width];
            let mut tiled_words = vec![vec![0; rows]; width];
            let mut tiled_output = vec![vec![F::ZERO; rows]; width];
            let mut timings = [std::time::Duration::ZERO; 2];
            for tiled in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                if tiled {
                    assert!(canonical_field_columns_v1(&original[..width]));
                    copy_fields_to_words_v1(&mut tiled_words, &original[..width]);
                    scale_word_columns_v1(&mut tiled_words, diagonal);
                    assert!(canonical_word_columns_v1(&tiled_words));
                    copy_words_to_fields_v1(&mut tiled_output, &tiled_words);
                } else {
                    assert!(
                        original[..width]
                            .iter()
                            .flatten()
                            .all(|value| F::canonical(value.0).is_some())
                    );
                    for (target, source) in reference_words.iter_mut().zip(&original[..width]) {
                        for (target, value) in target.iter_mut().zip(source) {
                            *target = value.0;
                        }
                    }
                    reference_words.par_iter_mut().for_each(|column| {
                        let mut power = F::ONE;
                        for word in column {
                            *word = F(*word).mul(power).0;
                            power = power.mul(diagonal);
                        }
                    });
                    assert!(
                        reference_words
                            .iter()
                            .flatten()
                            .all(|&word| F::canonical(word).is_some())
                    );
                    for (target, source) in reference_output.iter_mut().zip(&reference_words) {
                        for (target, &word) in target.iter_mut().zip(source) {
                            *target = F(word);
                        }
                    }
                }
                timings[usize::from(tiled)] = start.elapsed();
            }
            assert_eq!(tiled_words, reference_words);
            assert_eq!(tiled_output, reference_output);
            eprintln!(
                "native19_host_tiles rows={rows} width={width} round={round} workers={} serial_copy_column_parallel_scale_ns={} tiled_host_ns={} parity=exact",
                rayon::current_num_threads(),
                timings[0].as_nanos(),
                timings[1].as_nanos()
            );
        }
    }
}
