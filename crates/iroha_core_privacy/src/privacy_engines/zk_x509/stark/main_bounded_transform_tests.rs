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
    let cap = super::super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1 as usize;
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
        596_974_144 - mask_scratch - SHARED_POWERS_ALLOWANCE_V1
    );
    let key_owner = super::super::main_key_joins::MainKeyJoinPlanV1::public_owner_charge_v1();
    assert_eq!(key_owner, 8 * 1024 * 1024);
    let union_owner = super::super::main_sha_union::MainShaUnionPlanV1::public_owner_charge_v1();
    assert_eq!(union_owner, 6_416);
    let limit = source_limit - link_owner - key_owner - union_owner;
    assert_eq!(
        limit,
        596_974_144
            - mask_scratch
            - link_owner
            - key_owner
            - union_owner
            - SHARED_POWERS_ALLOWANCE_V1
    );
    assert_eq!(plan.maximum_live_buffers, 3_697_993_152 + mask_scratch);
    let policy = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 288_345_698).unwrap();
    assert_eq!(
        policy.available,
        308_628_446
            - mask_scratch
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
        8
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
                        if unwind {
                            panic!("injected fixed staging allocation unwind");
                        }
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
    // Independent live-owner census: one host word matrix, rollback and shared
    // staging matrices, worst-case page padding, the whole idle pool, all 64
    // factorized twiddle cache entries plus construction/returned owners, and
    // dispatch metadata. These are logical payloads, not a measured RSS bound.
    let words = (1_usize << 19) * 8 * 8;
    let factorized_twiddles = (64 + 2) * (4 * 256) * 8;
    let expected = 3 * words
        + 8 * (16 * 1024 - 1)
        + 64 * 1024 * 1024
        + factorized_twiddles
        + (1 << 20)
        + 8 * core::mem::size_of::<Vec<u64>>()
        + core::mem::size_of::<Words>();
    assert_eq!(expected, 169_492_696);
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
        assert_eq!(adjusted.columns_v1(1 << 19), 8);
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
                        .map(|degree| F::reduce((column + 3 * degree * degree + 7) as u128))
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
    assert_eq!(adjusted.columns_v1(1 << 19), 8);
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
        assert_eq!(native.columns_v1(1 << 19), 8);
    }
    assert_eq!(
        original.available,
        308_628_446
            - core::mem::size_of::<E>()
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
    let original = (0..8)
        .map(|column| {
            (0..32)
                .map(|row| F((column * 101 + row * row + 3) as u64))
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
