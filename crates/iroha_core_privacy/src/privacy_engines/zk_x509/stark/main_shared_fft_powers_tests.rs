//! Exact public-power sharing, resource, and original clearing-owner controls.

use super::*;
use crate::privacy_engines::transparent_stark::{
    GOLDILOCKS_MODULUS_V1, TransparentStarkErrorV1, goldilocks_fft_coarse_v1,
    goldilocks_fft_coarse_with_powers_v1, goldilocks_ifft_coarse_v1,
    goldilocks_ifft_coarse_with_powers_v1,
};
use crate::privacy_engines::zk_x509::private_table::{inspection, zeroize_field_rows_v1};

fn policy() -> MainBoundedTransformPolicyV1 {
    // Synthetic public admission; the existing whole-layout boundary control
    // independently checks for_assembly_v1 and its exact fixed reservation.
    MainBoundedTransformPolicyV1 {
        available: 0,
        backend: None,
        public_powers_reserved: true,
    }
}
fn no_device(_: &mut [Vec<u64>], _: u64, _: Direction) -> Result<Backend, TransformError> {
    panic!("shared public powers stay on the CPU route")
}
fn columns(rows: usize, width: usize) -> PrivateTableV1<Vec<F>> {
    PrivateTableV1::new(
        (0..width)
            .map(|column| {
                (0..rows)
                    .map(|row| F::reduce((row as u128 + 31 * column as u128 + 17).pow(3)))
                    .collect()
            })
            .collect(),
        zeroize_field_rows_v1,
    )
}

#[test]
fn shared_public_fft_powers_preserve_domain_errors_and_independent_small_evaluations() {
    for log in 0..=5 {
        let rows = 1usize << log;
        let root = goldilocks_primitive_root_v1(log).unwrap();
        let forward = PublicPowers::new_v1(rows, root, usize::MAX).unwrap();
        let inverse = PublicPowers::new_v1(rows, root.inv().unwrap(), usize::MAX).unwrap();
        let original = columns(rows, 1);
        let mut values = original[0].clone();
        goldilocks_fft_coarse_with_powers_v1(&mut values, root, &forward).unwrap();
        for (index, actual) in values.iter().enumerate() {
            let x = root.pow(index as u128);
            let expected = original[0]
                .iter()
                .rev()
                .fold(F::ZERO, |a, b| a.mul(x).add(*b));
            assert_eq!(*actual, expected);
        }
        goldilocks_ifft_coarse_with_powers_v1(&mut values, root, &inverse).unwrap();
        assert_eq!(values, original[0]);
    }
    let root = goldilocks_primitive_root_v1(4).unwrap();
    let forward = PublicPowers::new_v1(16, root, usize::MAX).unwrap();
    let inverse = PublicPowers::new_v1(16, root.inv().unwrap(), usize::MAX).unwrap();
    for rows in [0, 3, 16, 32] {
        for candidate in [F::ZERO, F::ONE, F(GOLDILOCKS_MODULUS_V1), root] {
            for backwards in [false, true] {
                for malformed in [false, true] {
                    let mut expected = vec![F(7); rows];
                    if malformed && rows > 0 {
                        expected[rows - 1] = F(GOLDILOCKS_MODULUS_V1);
                    }
                    let mut actual = expected.clone();
                    let expected_result = if backwards {
                        goldilocks_ifft_coarse_v1(&mut expected, candidate)
                    } else {
                        goldilocks_fft_coarse_v1(&mut expected, candidate)
                    };
                    let actual_result = if backwards {
                        goldilocks_ifft_coarse_with_powers_v1(&mut actual, candidate, &inverse)
                    } else {
                        goldilocks_fft_coarse_with_powers_v1(&mut actual, candidate, &forward)
                    };
                    assert_eq!(actual_result, expected_result);
                    assert_eq!(actual, expected);
                }
            }
        }
    }
    let mut wrong_owner = vec![F(7); 16];
    assert_eq!(
        goldilocks_fft_coarse_with_powers_v1(&mut wrong_owner, root, &inverse),
        Err(TransparentStarkErrorV1::InvalidDomain)
    );
    assert_eq!(wrong_owner, vec![F(7); 16]);
}

#[test]
fn shared_public_fft_powers_charge_exact_single_and_paired_capacity_before_dispatch() {
    for rows in [1usize, 2, 32, 1 << 14, 1 << 19] {
        let root = goldilocks_primitive_root_v1(rows.trailing_zeros() as u8).unwrap();
        let minimum = PublicPowers::required_payload_bytes_v1(rows).unwrap();
        assert_eq!(
            minimum,
            rows / 2 * core::mem::size_of::<F>() + core::mem::size_of::<PublicPowers>()
        );
        assert!(PublicPowers::new_v1(rows, root, minimum - 1).is_err());
        let owner = PublicPowers::new_v1(rows, root, minimum).unwrap();
        assert_eq!(owner.allocated_payload_bytes_v1().unwrap(), minimum);
        assert!(
            MainBoundedTransformPolicyV1::cpu_v1()
                .cpu_powers_v1(rows, root)
                .unwrap()
                .is_none()
        );
        let pair = policy().cpu_power_pair_v1(rows, root, true).unwrap();
        if rows < SHARED_POWERS_MIN_ROWS_V1 {
            assert!(pair.0.is_none() && pair.1.is_none());
        } else {
            assert_eq!(
                pair.0.unwrap().allocated_payload_bytes_v1().unwrap()
                    + pair.1.unwrap().allocated_payload_bytes_v1().unwrap(),
                2 * minimum
            );
            let forward = policy().cpu_power_pair_v1(rows, root, false).unwrap();
            assert_eq!(
                forward.0.unwrap().allocated_payload_bytes_v1().unwrap(),
                minimum
            );
            assert!(forward.1.is_none());
        }
    }
    assert_eq!(SHARED_POWERS_ALLOWANCE_V1, 4_194_384);
    for available in [0, SHARED_POWERS_ALLOWANCE_V1 - 1] {
        // An explicit reference/public caller with no reserved table allowance
        // remains valid and allocation-free even at small public budgets.
        let reference = MainBoundedTransformPolicyV1 {
            available,
            ..MainBoundedTransformPolicyV1::cpu_v1()
        };
        assert!(
            reference
                .cpu_powers_v1(256, goldilocks_primitive_root_v1(8).unwrap())
                .unwrap()
                .is_none()
        );
    }
    for available in [0, 1, 1 << 20, usize::MAX - SHARED_POWERS_ALLOWANCE_V1] {
        let admitted = MainBoundedTransformPolicyV1 {
            available,
            ..policy()
        };
        assert_eq!(admitted.available, available);
        // Spending all remaining caller/device slack cannot consume the
        // separate table reservation or change the public selection.
        let exhausted = admitted.reserve_additional_v1(available).unwrap();
        for rows in [32usize, 128, 256, 1 << 19] {
            let root = goldilocks_primitive_root_v1(rows.trailing_zeros() as u8).unwrap();
            for selected in [admitted, exhausted] {
                let pair = selected.cpu_power_pair_v1(rows, root, true).unwrap();
                assert_eq!(pair.0.is_some(), rows >= SHARED_POWERS_MIN_ROWS_V1);
                assert_eq!(pair.1.is_some(), rows >= SHARED_POWERS_MIN_ROWS_V1);
                if let (Some(forward), Some(inverse)) = pair {
                    assert!(
                        forward.allocated_payload_bytes_v1().unwrap()
                            + inverse.allocated_payload_bytes_v1().unwrap()
                            <= SHARED_POWERS_ALLOWANCE_V1
                    );
                }
            }
        }
    }
    assert!(
        policy()
            .cpu_powers_v1(1 << 20, goldilocks_primitive_root_v1(20).unwrap())
            .is_err()
    );
    for malformed in [0, 3, usize::MAX] {
        assert!(PublicPowers::required_payload_bytes_v1(malformed).is_err());
    }
}

#[test]
fn shared_public_fft_powers_preserve_all_workers_fixed_transitions_and_private_cleanup() {
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for log in [5, 8, 14, 15, 16, 18, 19] {
            let rows = 1usize << log;
            let root = goldilocks_primitive_root_v1(log).unwrap();
            for width in [1, 8] {
                for direction in [Direction::Forward, Direction::Inverse] {
                    let mut actual = columns(rows, width);
                    let mut expected = PrivateTableV1::new(actual.to_vec(), zeroize_field_rows_v1);
                    let identities: Vec<_> = actual
                        .iter()
                        .map(|column| (column.as_ptr(), column.len(), column.capacity()))
                        .collect();
                    pool.install(|| {
                        for column in &mut *expected {
                            if direction == Direction::Inverse {
                                goldilocks_ifft_coarse_v1(column, root)
                            } else {
                                goldilocks_fft_coarse_v1(column, root)
                            }
                            .unwrap();
                        }
                        if direction == Direction::Inverse {
                            policy().inverse_with_v1(&mut actual, root, no_device, || false)
                        } else {
                            policy().forward_with_v1(&mut actual, root, no_device, || false)
                        }
                        .unwrap();
                    });
                    assert_eq!(&*actual, &*expected);
                    assert_eq!(
                        identities,
                        actual
                            .iter()
                            .map(|column| (column.as_ptr(), column.len(), column.capacity()))
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    for rows in [32usize, 256] {
        for width in [1, 8, 11] {
            let original = columns(8, width);
            let mut baseline =
                super::super::main_fixed_coset::MainFixedCosetV1::new_v1(3, original.to_vec())
                    .unwrap();
            let mut actual =
                super::super::main_fixed_coset::MainFixedCosetV1::new_v1(3, original.to_vec())
                    .unwrap()
                    .with_transform_policy_v1(policy())
                    .unwrap();
            let full_root =
                goldilocks_primitive_root_v1((rows.trailing_zeros() + 2) as u8).unwrap();
            for ordinal in 0..4 {
                let stripe = main_quotient_stripes::MainQuotientStripeV1 {
                    rows,
                    count: 4,
                    ordinal,
                    next_stride: rows / 8,
                    root: goldilocks_primitive_root_v1(rows.trailing_zeros() as u8).unwrap(),
                    shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
                };
                let expected = baseline.evaluate_v1(stripe).unwrap();
                let result = actual.evaluate_v1(stripe).unwrap();
                assert_eq!(result, expected);
                for (column, values) in result.iter().enumerate() {
                    for (row, value) in values.iter().enumerate() {
                        let x = stripe.shift.mul(stripe.root.pow(row as u128));
                        assert_eq!(
                            *value,
                            original[column]
                                .iter()
                                .rev()
                                .fold(F::ZERO, |sum, coefficient| sum.mul(x).add(*coefficient))
                        );
                    }
                }
            }
        }
    }
    for rows in [32usize, 256] {
        for malformed in [false, true] {
            for unwind in [false, true] {
                let mut returned_error = None;
                let (outcome, erased) = inspection::observe_v1(|| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut values = columns(rows, 2);
                        if malformed {
                            values[1][rows - 1] = F(GOLDILOCKS_MODULUS_V1);
                        }
                        let result = policy().forward_with_v1(
                            &mut values,
                            goldilocks_primitive_root_v1(rows.trailing_zeros() as u8).unwrap(),
                            no_device,
                            || false,
                        );
                        returned_error = Some(result.is_err());
                        if unwind {
                            panic!("shared public table cleanup");
                        }
                    }))
                });
                assert_eq!(returned_error, Some(malformed));
                assert_eq!(outcome.is_err(), unwind);
                assert_eq!(erased.iter().map(|row| row.cells).sum::<usize>(), 2 * rows);
                assert!(erased.iter().any(|row| row.nonzero_before > 0));
                assert!(erased.iter().all(|row| row.nonzero_after == 0));
            }
        }
    }
}

#[test]
#[ignore = "registered full-work shared public table cost diagnostic"]
fn registered_shared_public_fft_powers_include_construction_in_alternating_full_work_cost() {
    use fastpq_prover::goldilocks_transform::transform_goldilocks_columns_v1;
    assert_eq!(rayon::current_num_threads(), 20);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let native_logs = layout
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(native_logs, [5, 8, 15, 16, 18, 19].into_iter().collect());
    let mut stripe_logs = std::collections::BTreeSet::new();
    let mut transition_logs = std::collections::BTreeSet::new();
    for registration in &layout.registered_segments {
        let plan = registered_retained_prover_plan_v1(registration.segment, layout.common_lde_log2)
            .unwrap();
        let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(
            registration.segment.trace_log2,
            plan.quotient_coset_log2,
            0,
        )
        .unwrap();
        stripe_logs.insert(stripe.rows.ilog2() as u8);
        if stripe.count > 1 {
            transition_logs.insert((stripe.rows.ilog2() as u8, plan.quotient_coset_log2));
        }
    }
    let mut cases = Vec::new();
    for log in native_logs {
        cases.push((0, log, log));
    }
    for log in stripe_logs {
        cases.push((1, log, log));
    }
    for (log, full) in transition_logs {
        cases.push((2, log, full));
    }
    assert_eq!(cases.len(), 13);
    println!(
        "shared_power_header cases=13 widths=1,8 rounds=3 modes=coarse,shared workers=20 expected_records=156 public_allocation_construction_and_drop_timed=true private_allocation_copy_oracle_clear_timed=false wholeproof_claim=false"
    );
    let mut records = 0;
    for (case, (phase, log, full)) in cases.into_iter().enumerate() {
        let rows = 1usize << log;
        let root = goldilocks_primitive_root_v1(log).unwrap();
        let diagonal = goldilocks_primitive_root_v1(full).unwrap();
        for width in [1, 8] {
            let source = columns(rows, width);
            let mut expected = Words::new(
                source
                    .iter()
                    .map(|column| column.iter().map(|value| value.0).collect())
                    .collect(),
                erase_words_v1,
            );
            if phase != 1 {
                assert_eq!(
                    transform_goldilocks_columns_v1(
                        &mut expected,
                        root.0,
                        Direction::Inverse,
                        fastpq_prover::ExecutionMode::Cpu
                    )
                    .unwrap(),
                    Backend::Cpu
                );
            }
            if phase == 2 {
                for column in &mut *expected {
                    let mut power = F::ONE;
                    for value in column {
                        *value = F(*value).mul(power).0;
                        power = power.mul(diagonal);
                    }
                }
            }
            if phase != 0 {
                assert_eq!(
                    transform_goldilocks_columns_v1(
                        &mut expected,
                        root.0,
                        Direction::Forward,
                        fastpq_prover::ExecutionMode::Cpu
                    )
                    .unwrap(),
                    Backend::Cpu
                );
            }
            let mut coarse = PrivateTableV1::new(source.to_vec(), zeroize_field_rows_v1);
            let mut shared = PrivateTableV1::new(source.to_vec(), zeroize_field_rows_v1);
            for round in 0..3 {
                for use_shared in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let target = if use_shared { &mut shared } else { &mut coarse };
                    for (target, original) in target.iter_mut().zip(source.iter()) {
                        target.copy_from_slice(original);
                    }
                    let started = std::time::Instant::now();
                    let (forward, inverse) = if use_shared {
                        if phase == 0 {
                            (
                                None,
                                policy().cpu_powers_v1(rows, root.inv().unwrap()).unwrap(),
                            )
                        } else {
                            policy().cpu_power_pair_v1(rows, root, phase == 2).unwrap()
                        }
                    } else {
                        (None, None)
                    };
                    let construction_ns = started.elapsed().as_nanos();
                    let table_payload_bytes = forward
                        .as_ref()
                        .map_or(0, |table| table.allocated_payload_bytes_v1().unwrap())
                        + inverse
                            .as_ref()
                            .map_or(0, |table| table.allocated_payload_bytes_v1().unwrap());
                    let table_selected = forward.is_some() || inverse.is_some();
                    assert_eq!(
                        table_selected,
                        use_shared && rows >= SHARED_POWERS_MIN_ROWS_V1
                    );
                    if table_selected {
                        assert_eq!(
                            table_payload_bytes,
                            PublicPowers::required_payload_bytes_v1(rows).unwrap()
                                * if phase == 2 { 2 } else { 1 }
                        );
                    } else {
                        assert_eq!(table_payload_bytes, 0);
                    }
                    target
                        .par_iter_mut()
                        .try_for_each(|column| -> Result<(), TransparentStarkErrorV1> {
                            if phase != 1 {
                                if let Some(table) = &inverse {
                                    goldilocks_ifft_coarse_with_powers_v1(column, root, table)?;
                                } else {
                                    goldilocks_ifft_coarse_v1(column, root)?;
                                }
                            }
                            if phase == 2 {
                                let mut power = F::ONE;
                                for value in column.iter_mut() {
                                    *value = value.mul(power);
                                    power = power.mul(diagonal);
                                }
                            }
                            if phase != 0 {
                                if let Some(table) = &forward {
                                    goldilocks_fft_coarse_with_powers_v1(column, root, table)?;
                                } else {
                                    goldilocks_fft_coarse_v1(column, root)?;
                                }
                            }
                            Ok(())
                        })
                        .unwrap();
                    drop(forward);
                    drop(inverse);
                    let elapsed_ns = started.elapsed().as_nanos();
                    for (column, expected) in target.iter().zip(expected.iter()) {
                        assert!(
                            column
                                .iter()
                                .zip(expected)
                                .all(|(actual, expected)| actual.0 == *expected)
                        );
                    }
                    println!(
                        "shared_power_cost case={case} phase={phase} log={log} full_log={full} width={width} round={round} shared={use_shared} table_selected={table_selected} construction_ns={construction_ns} elapsed_ns={elapsed_ns} table_payload_bytes={table_payload_bytes} named_private_payload_bytes={}",
                        4 * width * rows * core::mem::size_of::<F>()
                    );
                    records += 1;
                }
            }
        }
    }
    assert_eq!(records, 156);
}
