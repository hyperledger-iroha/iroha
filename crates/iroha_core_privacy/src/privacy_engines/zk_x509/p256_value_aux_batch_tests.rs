//! Endpoint-specific grouped replay, exact phase binding and clearing failures.

use super::super::private_table::inspection;
use super::*;

fn clearing_failures<const WIDTH: usize>() {
    for failure in 0..4 {
        let mut columns = vec![vec![F(97); 8]; 8];
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let mut row = 0;
                fill_aggregate_aux_columns_v1::<WIDTH>(8, WIDTH - 8, &mut targets, || {
                    row += 1;
                    match (failure, row) {
                        (0, 4) => return Err(P256AggregateAdapterErrorV1::Constraint),
                        (1, 4) => return Ok(None),
                        (2, 4) => panic!("injected value auxiliary unwind"),
                        _ => {}
                    }
                    Ok(Some([F(83); WIDTH])) // case3 returns an unexpected ninth row
                })
            }))
        });
        assert!(matches!(result, Ok(Err(_))) || (failure == 2 && result.is_err()));
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
        assert_eq!(
            observations.iter().filter(|item| item.cells == 8).count(),
            8
        );
        let scratch = observations
            .iter()
            .filter(|item| item.cells == WIDTH)
            .collect::<Vec<_>>();
        assert!(scratch.iter().any(|item| item.nonzero_before == WIDTH));
        if failure == 3 {
            assert_eq!(scratch.len(), 9, "unexpected row must clear too");
        }
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn value_auxiliary_execution_and_sorted_rows_clear_on_failure_extra_row_and_unwind() {
    clearing_failures::<P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1>();
    clearing_failures::<P256_VALUE_BUS_STARK_AUX_WIDTH_V1>();
}

#[test]
fn value_auxiliary_batch_rejects_shape_phase_and_other_adapters() {
    let bound = P256MainBoundSourceV1 {
        signatures: None,
        fixed: None,
        post_base: None,
        terminal_claims: None,
    };
    for endpoint in 0..=1 {
        let registration =
            P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, endpoint).unwrap();
        let width = registration.shape_v1().unwrap().aux_width;
        for (count, first, len) in [
            (0, 0, 8),
            (9, 0, 8),
            (1, usize::MAX, 8),
            (1, width, 8),
            (1, 0, 8),
        ] {
            let mut columns = vec![vec![F(97); len]; count];
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            assert_eq!(
                bound.fill_value_aux_columns_v1(registration, first, &mut targets),
                Err(P256AggregateAdapterErrorV1::Topology)
            );
            assert!(columns.iter().flatten().all(|value| *value == F(97)));
        }
        let mut output =
            zeroize::Zeroizing::new(vec![F(97); P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1]);
        assert_eq!(
            bound.fill_value_aux_columns_v1(registration, 0, &mut [&mut output]),
            Err(P256AggregateAdapterErrorV1::Phase)
        );
        assert!(output.iter().all(|value| *value == F(97)));
    }
    for adapter in [
        P256MainAdapterV1::Arithmetic,
        P256MainAdapterV1::BindingSink,
        P256MainAdapterV1::ScalarBitBus,
    ] {
        let registration = P256MainRegistrationV1::new_v1(0, adapter, 0).unwrap();
        let mut output = vec![F(97); 8];
        assert_eq!(
            bound.fill_value_aux_columns_v1(registration, 0, &mut [&mut output]),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(output.iter().all(|value| *value == F(97)));
    }
    let stack = p256_value_aux_replay_scratch_v1();
    assert!(stack > core::mem::size_of::<P256AggregateAuxRowScratchV1<116>>());
    assert!(stack <= P256MainBaseSourceV1::replay_scratch_forecast_v1().unwrap());
    assert!(stack < super::super::allocation_payload::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1);
}

#[test]
#[ignore = "all five signatures, full native value columns and independently bound token"]
fn value_auxiliary_batches_match_every_endpoint_cell_and_scalar_columns() {
    for seed in [71, 79] {
        let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
        let token = super::tests::main_post_base_v1(seed);
        let mut bound = base.bind_v1(token).unwrap();
        assert!(base.private_is_zeroized_v1());
        let retained = bound.allocated_payload_bytes_v1();
        for signature in 0..P256_X5S1_SIGNATURES_V1 {
            for endpoint in 0..=1 {
                let registration = P256MainRegistrationV1::new_v1(
                    signature,
                    P256MainAdapterV1::ValueBus,
                    endpoint,
                )
                .unwrap();
                let shape = registration.shape_v1().unwrap();
                let value = bound
                    .signature_v1(registration)
                    .unwrap()
                    .value
                    .as_ref()
                    .unwrap();
                let starts = if seed == 71 {
                    (0..shape.aux_width).step_by(8).collect::<Vec<_>>()
                } else {
                    vec![shape.aux_width - 8]
                };
                for first in starts {
                    let count = (shape.aux_width - first).min(8);
                    let mut columns =
                        zeroize::Zeroizing::new(vec![vec![F(97); shape.trace_size]; count]);
                    let mut targets = columns
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>();
                    bound
                        .fill_value_aux_columns_v1(registration, first, &mut targets)
                        .unwrap();
                    drop(targets);
                    // Fresh endpoint-specific streams independently derive all
                    // terminals, including every padding and final native row.
                    if endpoint == 0 {
                        let mut reference =
                            P256ValueExecutionAggregateStreamV1::new_v1(value).unwrap();
                        for row in 0..shape.trace_size {
                            let expected = zeroize::Zeroizing::new(
                                reference.next_aux_row_v1().unwrap().unwrap(),
                            );
                            for offset in 0..count {
                                assert_eq!(columns[offset][row], expected[first + offset]);
                            }
                        }
                        assert!(reference.next_aux_row_v1().unwrap().is_none());
                    } else {
                        let mut reference = value.sorted_aux_source_v1().unwrap();
                        for row in 0..shape.trace_size {
                            let expected = zeroize::Zeroizing::new(
                                reference.next_aux_row_v1().unwrap().unwrap(),
                            );
                            for offset in 0..count {
                                assert_eq!(columns[offset][row], expected[first + offset]);
                            }
                        }
                        assert!(reference.next_aux_row_v1().unwrap().is_none());
                    }
                    // Compare the actual single-column API at initial, middle,
                    // and final columns as well as every cell of the raw streams.
                    for column in [0, shape.aux_width / 2, shape.aux_width - 1] {
                        if (first..first + count).contains(&column) {
                            let mut single =
                                zeroize::Zeroizing::new(vec![F::ZERO; shape.trace_size]);
                            bound
                                .fill_aux_column_v1(registration, column, &mut single)
                                .unwrap();
                            assert_eq!(columns[column - first].as_slice(), single.as_slice());
                        }
                    }
                    assert_eq!(bound.allocated_payload_bytes_v1(), retained);
                }
            }
        }
        bound.post_base = Some(super::tests::main_post_base_v1(seed + 1));
        for endpoint in 0..=1 {
            let registration =
                P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, endpoint).unwrap();
            let mut output =
                zeroize::Zeroizing::new(vec![F(97); P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1]);
            assert_eq!(
                bound.fill_value_aux_columns_v1(registration, 0, &mut [&mut output]),
                Err(P256AggregateAdapterErrorV1::Challenge)
            );
            assert!(output.iter().all(|value| *value == F(97)));
        }
        bound.post_base = Some(token);
    }
}
