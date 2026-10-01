//! Fixed-matrix adapter parity, poisoned ownership and required-device controls.

use super::*;
use crate::privacy_engines::zk_x509::{
    private_table::inspection, prover_observation::ObservationV1,
};

fn stripe(
    native_log: u8,
    stripe_log: u8,
    full_log: u8,
    ordinal: usize,
) -> main_quotient_stripes::MainQuotientStripeV1 {
    main_quotient_stripes::MainQuotientStripeV1 {
        rows: 1 << stripe_log,
        count: 1 << (full_log - stripe_log),
        ordinal,
        next_stride: 1 << (stripe_log - native_log),
        root: goldilocks_primitive_root_v1(stripe_log).unwrap(),
        shift: F(GOLDILOCKS_GENERATOR_V1).mul(
            goldilocks_primitive_root_v1(full_log)
                .unwrap()
                .pow(ordinal as u128),
        ),
    }
}

fn coefficients(log: u8, width: usize) -> Vec<Vec<F>> {
    (0..width)
        .map(|column| {
            (0..1_usize << log)
                .map(|i| F((i * i + 7 * column + 3) as u64))
                .collect()
        })
        .collect()
}

fn transform_preserving_caller_allocation(
    words: &mut [Vec<u64>],
    root: u64,
    direction: Direction,
    execution: fastpq_prover::ExecutionMode,
) -> Result<Backend, TransformError> {
    let allocations: [Option<(*const u64, usize, usize)>; 8] =
        core::array::from_fn(|i| words.get(i).map(|c| (c.as_ptr(), c.len(), c.capacity())));
    let result = transform_goldilocks_columns_v1(words, root, direction, execution);
    let after: [Option<(*const u64, usize, usize)>; 8] =
        core::array::from_fn(|i| words.get(i).map(|c| (c.as_ptr(), c.len(), c.capacity())));
    assert_eq!(allocations, after);
    result
}

#[test]
fn packed_fixed_batches_match_cpu_horner_order_padding_and_actual_fallback_counters() {
    for width in [1, 8, 11] {
        for admitted in [1, 2, 4, 8] {
            let original = coefficients(3, width);
            let mut cpu = MainFixedCosetV1::new_v1(3, original.clone()).unwrap();
            let mut packed = MainFixedCosetV1::new_v1(3, original.clone())
                .unwrap()
                .with_transform_policy_v1(MainBoundedTransformPolicyV1::for_test_v1(16, admitted))
                .unwrap();
            let observation = ObservationV1::begin_v1();
            for ordinal in 0..4 {
                let coordinates = stripe(3, 4, 6, ordinal);
                let expected = cpu.evaluate_v1(coordinates).unwrap();
                let actual = packed
                    .evaluate_with_v1(
                        coordinates,
                        |words, root, direction| {
                            assert!(words.len() <= admitted);
                            transform_preserving_caller_allocation(
                                words,
                                root,
                                direction,
                                fastpq_prover::ExecutionMode::Cpu,
                            )
                        },
                        || false,
                    )
                    .unwrap();
                assert_eq!(actual, expected);
                for (column, values) in actual.iter().enumerate() {
                    for (row, value) in values.iter().enumerate() {
                        let x = coordinates.shift.mul(coordinates.root.pow(row as u128));
                        let expected = original[column]
                            .iter()
                            .rev()
                            .fold(F::ZERO, |sum, c| sum.mul(x).add(*c));
                        assert_eq!(value.0.to_le_bytes(), expected.0.to_le_bytes());
                    }
                    let mut recovered = values.clone();
                    goldilocks_ifft_v1(&mut recovered, coordinates.root).unwrap();
                    let mut power = F::ONE;
                    for value in &mut recovered {
                        *value = value.mul(power);
                        power = power.mul(coordinates.shift.inv().unwrap());
                    }
                    assert_eq!(&recovered[..8], original[column]);
                    assert!(recovered[8..].iter().all(|v| *v == F::ZERO));
                }
            }
            let receipt = observation.finish_v1().public_text_v1();
            assert!(receipt.contains(&format!("fixed_coset_cpu_forward_columns={}", 8 * width)));
            assert!(receipt.contains(&format!("fixed_coset_cpu_inverse_columns={}", 6 * width)));
            assert!(receipt.contains("fixed_coset_metal_forward_columns=0"));
            assert!(receipt.contains("fixed_coset_metal_inverse_columns=0"));
        }
    }
}

#[test]
fn failed_inverse_forward_later_batch_and_unwind_poison_and_clear_real_words() {
    for failing_call in [0, 1, 2, 3, 5] {
        for unwind in [false, true] {
            let mut owner = MainFixedCosetV1::new_v1(3, coefficients(3, 5))
                .unwrap()
                .with_transform_policy_v1(MainBoundedTransformPolicyV1::for_test_v1(16, 2))
                .unwrap();
            owner
                .evaluate_with_v1(
                    stripe(3, 4, 6, 0),
                    |words, root, direction| {
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
            let (result, erased) = inspection::observe_v1(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut calls = 0;
                    owner
                        .evaluate_with_v1(
                            stripe(3, 4, 6, 1),
                            |words, root, direction| {
                                let call = calls;
                                calls += 1;
                                if call == failing_call {
                                    words[0][0] = 99;
                                    if unwind {
                                        panic!("injected fixed device stage unwind");
                                    }
                                    return Err(TransformError::DeviceUnavailable);
                                }
                                transform_goldilocks_columns_v1(
                                    words,
                                    root,
                                    direction,
                                    fastpq_prover::ExecutionMode::Cpu,
                                )
                            },
                            || false,
                        )
                        .map(|_| ())
                }))
            });
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(owner.poisoned);
            assert!(owner.evaluate_v1(stripe(3, 4, 6, 1)).is_err());
            assert!(erased.iter().any(|v| v.nonzero_before > 0));
            assert!(erased.iter().all(|v| v.nonzero_after == 0));
        }
    }
}

#[test]
fn malformed_coordinates_and_terminal_uncertainty_poison_before_dispatch_or_padding() {
    for cpu in [false, true] {
        for uncertain in [false, true] {
            let original = coefficients(3, 3);
            let mut owner = MainFixedCosetV1::new_v1(3, original.clone()).unwrap();
            if !cpu {
                owner = owner
                    .with_transform_policy_v1(MainBoundedTransformPolicyV1::for_test_v1(16, 2))
                    .unwrap();
            }
            let mut coordinates = stripe(3, 4, 6, 0);
            if !uncertain {
                coordinates.shift = F::ZERO;
            }
            let result = owner.evaluate_with_v1(
                coordinates,
                |_, _, _| panic!("rejected call dispatched"),
                || uncertain,
            );
            if uncertain {
                assert!(matches!(
                    result,
                    Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
                ));
            } else {
                assert!(result.is_err());
            }
            assert_eq!(owner.columns.0, original);
            assert!(owner.poisoned);
        }
    }
}

#[test]
fn completed_owner_cannot_replace_its_original_admission() {
    let mut owner = MainFixedCosetV1::new_v1(3, coefficients(3, 1)).unwrap();
    owner.evaluate_v1(stripe(3, 4, 6, 0)).unwrap();
    assert!(
        owner
            .with_transform_policy_v1(MainBoundedTransformPolicyV1::cpu_v1())
            .is_err()
    );
}

#[test]
#[ignore = "requires actual Metal and full native19 repeated fixed stripes; absence is a failure"]
fn required_metal_repeated_native19_fixed_stripes_match_every_cpu_byte() {
    use fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1;
    assert_eq!(
        available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal),
        "required actual Metal backend"
    );
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let policy = MainBoundedTransformPolicyV1::for_assembly_v1(&layout, 288_345_698).unwrap();
    assert_eq!(policy.columns_v1(1 << 19), 8);
    for native_log in [8, 19] {
        // Eleven columns exercise a full eight-column submission and its tail.
        let original = coefficients(native_log, 11);
        let mut cpu = MainFixedCosetV1::new_v1(native_log, original.clone()).unwrap();
        let mut metal = MainFixedCosetV1::new_v1(native_log, original)
            .unwrap()
            .with_transform_policy_v1(policy)
            .unwrap();
        let observation = ObservationV1::begin_v1();
        for ordinal in 0..4 {
            let coordinates = stripe(native_log, native_log, native_log + 2, ordinal);
            let expected = cpu.evaluate_v1(coordinates).unwrap();
            let actual = metal
                .evaluate_with_v1(
                    coordinates,
                    |words, root, direction| {
                        let backend = transform_preserving_caller_allocation(
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
            for (actual, expected) in actual.iter().flatten().zip(expected.iter().flatten()) {
                assert_eq!(actual.0.to_le_bytes(), expected.0.to_le_bytes());
            }
        }
        let receipt = observation.finish_v1().public_text_v1();
        assert!(receipt.contains("fixed_coset_metal_forward_columns=44"));
        assert!(receipt.contains("fixed_coset_metal_inverse_columns=33"));
        assert!(receipt.contains("fixed_coset_cpu_forward_columns=44"));
        assert!(receipt.contains("fixed_coset_cpu_inverse_columns=33"));
        println!("native_log={native_log}; repeated_fixed_stripes=4; {receipt}");
    }
}
