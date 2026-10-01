//! Native inverse-only replay parity, custody and completion controls.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection;
use rand::{RngCore, SeedableRng, rngs::StdRng};

fn native(log: u8, column: usize) -> ZeroizingMainTraceColumnV1 {
    ZeroizingMainTraceColumnV1(
        (0..1_usize << log)
            .map(|row| F::reduce(row as u128 * row as u128 + 17 * column as u128 + 3))
            .collect(),
    )
}

#[test]
fn native_device_replay_preserves_full_masked_coefficients_and_serial_source_order() {
    for log in [3, 11] {
        let mut rng = StdRng::from_seed([log + 121; 32]);
        let group = MainTraceMaskGroupV1::sample_v1(log, 13, 10, &mut rng, |column| {
            Ok(native(log, column))
        })
        .unwrap();
        let masks_before = group
            .masks
            .iter()
            .map(|mask| mask.coefficients().to_vec())
            .collect::<Vec<_>>();
        let mut rng_before = rng.clone();
        for width in [1, 3, 8] {
            let range = 2..2 + width;
            let expected = range
                .clone()
                .map(|column| group.replay_v1(column, &native(log, column)).unwrap())
                .collect::<Vec<_>>();
            for admitted in [1, 2, 4, 8] {
                for completed in [Backend::Cpu, Backend::Metal] {
                    let caller = std::thread::current().id();
                    let mut order = Vec::new();
                    let actual = group
                        .replay_batch_with_v1(
                            range.clone(),
                            MainBoundedTransformPolicyV1::for_test_v1(1 << log, admitted),
                            |column| {
                                assert_eq!(std::thread::current().id(), caller);
                                order.push(column);
                                Ok(native(log, column))
                            },
                            |words, root, direction| {
                                assert_eq!(direction, Direction::Inverse);
                                assert!(words.len() <= admitted);
                                transform_goldilocks_columns_v1(
                                    words,
                                    root,
                                    direction,
                                    fastpq_prover::ExecutionMode::Cpu,
                                )?;
                                Ok(completed) // Injection tests receipt handling, not hardware.
                            },
                            || false,
                        )
                        .unwrap();
                    assert_eq!(order, range.clone().collect::<Vec<_>>());
                    assert_eq!(actual, expected);
                }
            }
        }
        assert_eq!(
            group
                .masks
                .iter()
                .map(|mask| mask.coefficients().to_vec())
                .collect::<Vec<_>>(),
            masks_before
        );
        assert_eq!(rng.next_u64(), rng_before.next_u64());
    }
}

#[test]
fn native_replay_failed_late_inverse_clears_all_sources_and_word_batches_before_masking() {
    let mut rng = StdRng::from_seed([142; 32]);
    let group = MainTraceMaskGroupV1::sample_v1(3, 13, 8, &mut rng, |column| Ok(native(3, column)))
        .unwrap();
    for failure in 0..7 {
        let uncertain = std::cell::Cell::new(false);
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut calls = 0;
                group.replay_batch_with_v1(
                    0..8,
                    MainBoundedTransformPolicyV1::for_test_v1(8, 4),
                    |column| Ok(native(3, column)),
                    |words, root, direction| {
                        calls += 1;
                        assert_eq!(direction, Direction::Inverse);
                        if calls == 2 {
                            match failure {
                                0 => return Err(TransformError::DeviceUnavailable),
                                1 => return Err(TransformError::CompletionUncertain),
                                2 => {
                                    words[0][0] = u64::MAX;
                                    return Ok(Backend::Metal);
                                }
                                3 => {
                                    let _ = words[0].pop();
                                    return Ok(Backend::Metal);
                                }
                                4 => return Ok(Backend::Cuda),
                                5 => {
                                    uncertain.set(true);
                                    return Ok(Backend::Cpu);
                                }
                                _ => panic!("injected inverse replay unwind"),
                            }
                        }
                        transform_goldilocks_columns_v1(
                            words,
                            root,
                            direction,
                            fastpq_prover::ExecutionMode::Cpu,
                        )
                    },
                    || uncertain.get(),
                )
            }))
        });
        if failure == 6 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        // Eight native columns plus both four-column word batches. No mask
        // coefficient allocation was made after the failed inverse completion.
        assert_eq!(
            erased.iter().map(|item| item.cells).sum::<usize>(),
            2 * 8 * 8
        );
        assert!(erased.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn native_replay_rejects_excess_capacity_and_quarantine_before_dispatch_or_source() {
    let mut rng = StdRng::from_seed([143; 32]);
    let group = MainTraceMaskGroupV1::sample_v1(3, 13, 2, &mut rng, |column| Ok(native(3, column)))
        .unwrap();
    let (result, erased) = inspection::observe_v1(|| {
        group.replay_batch_with_v1(
            0..2,
            MainBoundedTransformPolicyV1::for_test_v1(8, 2),
            |column| {
                let mut values = native(3, column);
                values.0.reserve_exact(8);
                Ok(values)
            },
            |_, _, _| panic!("excess capacity must not dispatch"),
            || false,
        )
    });
    assert!(matches!(result, Err(ZkX509StarkErrorV1::ProofTooLarge)));
    assert_eq!(erased.iter().map(|item| item.cells).sum::<usize>(), 8);
    assert!(erased.iter().all(|item| item.nonzero_after == 0));
    for policy in [
        MainBoundedTransformPolicyV1::cpu_v1(),
        MainBoundedTransformPolicyV1::for_test_v1(8, 2),
    ] {
        assert!(matches!(
            group.replay_batch_with_v1(
                0..2,
                policy,
                |_| panic!("quarantine must precede source construction"),
                |_, _, _| panic!("quarantine must precede dispatch"),
                || true
            ),
            Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
        ));
    }
}

#[test]
#[ignore = "requires a normally built Metal privacy binary; full coefficient parity is component evidence only"]
fn native_replay_log19_width8_required_metal_matches_full_cpu_masked_output() {
    let mut rng = StdRng::from_seed([144; 32]);
    let group =
        MainTraceMaskGroupV1::sample_v1(19, 22, 8, &mut rng, |column| Ok(native(19, column)))
            .unwrap();
    let expected = group
        .replay_batch_v1(0..8, MainBoundedTransformPolicyV1::cpu_v1(), |column| {
            Ok(native(19, column))
        })
        .unwrap();
    let mut completed_columns = 0;
    let actual = group
        .replay_batch_with_v1(
            0..8,
            MainBoundedTransformPolicyV1::for_test_v1(1 << 19, 8),
            |column| Ok(native(19, column)),
            |words, root, direction| {
                assert_eq!(direction, Direction::Inverse);
                let backend = transform_goldilocks_columns_v1(
                    words,
                    root,
                    direction,
                    fastpq_prover::ExecutionMode::Gpu,
                )?;
                assert_eq!(backend, Backend::Metal, "required genuine Metal receipt");
                completed_columns += words.len();
                Ok(backend)
            },
            goldilocks_transform_completion_uncertain_v1,
        )
        .unwrap();
    assert_eq!(completed_columns, 8);
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .all(|column| column.len() == (1 << 19) + MASK_DEGREE + 1)
    );
}

#[test]
fn native_replay_final_short_batch_preserves_global_mask_indices_and_exact_capacity() {
    let mut rng = StdRng::from_seed([145; 32]);
    let group =
        MainTraceMaskGroupV1::sample_v1(3, 13, 10, &mut rng, |column| Ok(native(3, column)))
            .unwrap();
    let matrix = ZeroizingBaseColumnsV1(
        (0..10)
            .map(|column| native(3, column).into_vec_v1())
            .collect(),
    );
    let arrays = (0..8)
        .map(|row| {
            [
                F((row * row + 17 * 8 + 3) as u64),
                F((row * row + 17 * 9 + 3) as u64),
            ]
        })
        .collect::<Vec<_>>();
    let expected = (8..10)
        .map(|column| group.replay_v1(column, &matrix[column]).unwrap())
        .collect::<Vec<_>>();
    for builder in 0..3 {
        let mut order = Vec::new();
        let mut calls = 0;
        let actual = group
            .replay_batch_with_v1(
                8..10,
                MainBoundedTransformPolicyV1::for_test_v1(8, 4),
                |column| {
                    order.push(column);
                    let values = match builder {
                        0 => {
                            let mut values = zeroed_main_trace_column_v1(8)?;
                            values.copy_from_slice(&matrix[column]);
                            values
                        }
                        1 => copied_matrix_column_v1(&matrix, 10, 8, column)?,
                        _ => copied_array_column_v1(&arrays, column - 8)?,
                    };
                    assert_eq!(values.0.capacity(), 8);
                    Ok(values)
                },
                |words, root, direction| {
                    calls += 1;
                    assert_eq!(words.len(), 2);
                    assert_eq!(direction, Direction::Inverse);
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
        assert_eq!(order, vec![8, 9]);
        assert_eq!(actual.capacity(), 2);
        assert!(
            actual
                .iter()
                .all(|column| column.0.capacity() == 8 + MASK_DEGREE + 1)
        );
        assert_eq!(actual, expected);
    }
}
