//! Private value recurrence checkpoints, deterministic chunks and atomic output.

use super::super::{
    p256_value_bus::P256_VALUE_AUX_REPLAY_CHUNK_ROWS_V1 as CHUNK, private_table::inspection,
};
use super::*;

#[test]
fn value_checkpoint_destinations_are_disjoint_and_clear_on_failure_or_unwind() {
    let rows = P256_VALUE_BUS_STARK_TRACE_SIZE_V1;
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for failure in 0..3 {
            let mut columns = vec![vec![F(97); rows]; 3];
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pool.install(|| {
                    let mut targets = columns
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>();
                    fill_aggregate_aux_chunks_v1(rows, &mut targets, |chunk, targets| {
                        for (column, target) in targets.iter_mut().enumerate() {
                            assert_eq!(target.len(), CHUNK);
                            for (offset, value) in target.iter_mut().enumerate() {
                                *value = F((column * rows + chunk * CHUNK + offset + 1) as u64);
                            }
                        }
                        match (failure, chunk) {
                            (1, 7) => Err(P256AggregateAdapterErrorV1::Constraint),
                            (2, 7) => panic!("injected checkpoint chunk unwind"),
                            _ => Ok(()),
                        }
                    })
                })
            }));
            match failure {
                0 => {
                    outcome.unwrap().unwrap();
                    for (column, values) in columns.iter().enumerate() {
                        for (row, &value) in values.iter().enumerate() {
                            assert_eq!(value, F((column * rows + row + 1) as u64));
                        }
                    }
                }
                1 => assert_eq!(
                    outcome.unwrap(),
                    Err(P256AggregateAdapterErrorV1::Constraint)
                ),
                _ => assert!(outcome.is_err()),
            }
            if failure != 0 {
                assert!(columns.iter().flatten().all(|&value| value == F::ZERO));
            }
        }
    }
    for (rows, count, length) in [
        (CHUNK, 1, CHUNK),
        (rows, 0, rows),
        (rows, 9, 1),
        (rows, 1, 7),
    ] {
        let mut columns = vec![vec![F(97); length]; count];
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        assert_eq!(
            fill_aggregate_aux_chunks_v1(rows, &mut targets, |_, _| panic!(
                "invalid shape reached replay"
            )),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(columns.iter().flatten().all(|&value| value == F(97)));
    }
    let chunks = rows / CHUNK;
    assert!(p256_value_aux_replay_scratch_v1() >= chunks * core::mem::size_of::<[&mut [F]; 8]>());
    assert!(
        p256_value_aux_replay_scratch_v1()
            < super::super::allocation_payload::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1
    );
}

#[test]
#[ignore = "all native value rows and original dense recurrences across five signatures"]
fn value_checkpoint_all_families_match_dense_original_rows_and_terminals() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let bound = base.bind_v1(&super::tests::main_post_base_v1(107)).unwrap();
    let retained = bound.allocated_payload_bytes_v1();
    assert!(retained <= P256MainBaseSourceV1::allocation_forecast_v1().unwrap());
    for signature in 0..P256_X5S1_SIGNATURES_V1 {
        let registration =
            P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::ValueBus, 0).unwrap();
        let value = bound
            .signature_v1(registration)
            .unwrap()
            .value
            .as_ref()
            .unwrap();
        let mut dense = P256ValueExecutionAggregateStreamV1::new_v1(value).unwrap();
        for chunk in 0..P256_VALUE_BUS_STARK_TRACE_SIZE_V1 / CHUNK {
            let mut replay = bound
                .value_execution_aux_stream_at_v1(registration, Some(chunk))
                .unwrap();
            for offset in 0..CHUNK {
                let actual = zeroize::Zeroizing::new(replay.next_aux_row_v1().unwrap().unwrap());
                let expected = zeroize::Zeroizing::new(dense.next_aux_row_v1().unwrap().unwrap());
                assert_eq!(
                    *actual,
                    *expected,
                    "signature={signature} row={}",
                    chunk * CHUNK + offset
                );
            }
            assert!(replay.next_aux_row_v1().unwrap().is_none());
        }
        assert!(dense.next_aux_row_v1().unwrap().is_none());
        assert_eq!(
            dense.arithmetic_copy_running,
            dense.arithmetic_copy_terminal
        );
        assert_eq!(bound.allocated_payload_bytes_v1(), retained);
    }
}

#[test]
#[ignore = "actual bound owner, parallel selected-column parity and checkpoint corruption"]
fn value_checkpoint_parallel_batches_match_serial_and_clear_after_corruption() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let mut bound = base.bind_v1(&super::tests::main_post_base_v1(109)).unwrap();
    for signature in [0, P256_X5S1_SIGNATURES_V1 - 1] {
        for endpoint in 0..=1 {
            let registration =
                P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::ValueBus, endpoint)
                    .unwrap();
            let shape = registration.shape_v1().unwrap();
            let starts = if endpoint == 0 {
                vec![
                    0,
                    VALUE_WRITER_AUX - 4,
                    VALUE_WRITER_AUX,
                    VALUE_ARITHMETIC_COPY_AUX - 4,
                    shape.aux_width - 8,
                ]
            } else {
                vec![0, shape.aux_width - 8]
            };
            for first in starts {
                let mut expected =
                    zeroize::Zeroizing::new(vec![vec![F::ZERO; shape.trace_size]; 8]);
                if endpoint == 0 {
                    let mut stream = bound.value_execution_aux_stream_v1(registration).unwrap();
                    let selection = P256ValueAuxSelectionV1::new_v1(first, 8).unwrap();
                    for row in 0..shape.trace_size {
                        let values = zeroize::Zeroizing::new(
                            stream
                                .next_selected_aux_columns_v1(selection)
                                .unwrap()
                                .unwrap(),
                        );
                        for column in 0..8 {
                            expected[column][row] = values[column];
                        }
                    }
                } else {
                    let value = bound
                        .signature_v1(registration)
                        .unwrap()
                        .value
                        .as_ref()
                        .unwrap();
                    let mut stream = value.sorted_aux_source_v1().unwrap();
                    for row in 0..shape.trace_size {
                        let values =
                            zeroize::Zeroizing::new(stream.next_aux_row_v1().unwrap().unwrap());
                        for column in 0..8 {
                            expected[column][row] = values[first + column];
                        }
                    }
                }
                for workers in [1, 4] {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(workers)
                        .build()
                        .unwrap();
                    let mut actual =
                        zeroize::Zeroizing::new(vec![vec![F(97); shape.trace_size]; 8]);
                    pool.install(|| {
                        let mut targets =
                            actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>();
                        bound
                            .fill_value_aux_columns_v1(registration, first, &mut targets)
                            .unwrap();
                    });
                    assert_eq!(
                        *actual, *expected,
                        "signature={signature} endpoint={endpoint} first={first} workers={workers}"
                    );
                }
            }
        }
    }
    let registration = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    for writer in [false, true] {
        let cache = bound.signatures.as_mut().unwrap()[0]
            .value_checkpoints
            .as_mut()
            .unwrap();
        let first = if writer {
            cache.writer.corrupt_boundary_for_test_v1(1);
            VALUE_WRITER_AUX
        } else {
            cache.arithmetic_copy[1][0] = cache.arithmetic_copy[1][0].add(F::ONE);
            VALUE_ARITHMETIC_COPY_AUX
        };
        let mut columns =
            zeroize::Zeroizing::new(vec![vec![F(97); P256_VALUE_BUS_STARK_TRACE_SIZE_V1]; 8]);
        let (result, observations) = pool.install(|| {
            inspection::observe_v1(|| {
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                bound.fill_value_aux_columns_v1(registration, first, &mut targets)
            })
        });
        assert_eq!(result, Err(P256AggregateAdapterErrorV1::Constraint));
        assert!(columns.iter().flatten().all(|&value| value == F::ZERO));
        assert!(
            observations
                .iter()
                .any(|item| item.cells == 8 && item.nonzero_before > 0)
        );
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
        if !writer {
            let cache = bound.signatures.as_mut().unwrap()[0]
                .value_checkpoints
                .as_mut()
                .unwrap();
            cache.arithmetic_copy[1][0] = cache.arithmetic_copy[1][0].sub(F::ONE);
        }
    }
}
