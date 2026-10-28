//! Endpoint-specific grouped replay, exact phase binding and clearing failures.

use super::super::private_table::inspection;
use super::*;

fn clearing_failures<const WIDTH: usize>() {
    // Keep destination extents distinct from every tested scratch width.
    let rows = 16;
    for failure in 0..4 {
        let mut columns = vec![vec![F(97); rows]; 8];
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let mut row = 0;
                fill_aggregate_aux_columns_v1::<WIDTH>(rows, WIDTH - 8, &mut targets, || {
                    row += 1;
                    match (failure, row) {
                        (0, 4) => return Err(P256AggregateAdapterErrorV1::Constraint),
                        (1, 4) => return Ok(None),
                        (2, 4) => panic!("injected value auxiliary unwind"),
                        _ => {}
                    }
                    Ok(Some([F(83); WIDTH])) // case3 returns an unexpected extra row
                })
            }))
        });
        assert!(matches!(result, Ok(Err(_))) || (failure == 2 && result.is_err()));
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
        assert_eq!(
            observations
                .iter()
                .filter(|item| item.cells == rows)
                .count(),
            8
        );
        let scratch = observations
            .iter()
            .filter(|item| item.cells == WIDTH)
            .collect::<Vec<_>>();
        assert!(scratch.iter().any(|item| item.nonzero_before == WIDTH));
        if failure == 3 {
            assert_eq!(scratch.len(), rows + 1, "unexpected row must clear too");
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
        let mut bound = base.bind_v1(&token).unwrap();
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

#[test]
fn value_auxiliary_public_selection_covers_every_bounded_range_and_boundary() {
    let width = P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1;
    for count in 1..=crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1 {
        for first in 0..=width - count {
            let selection = P256ValueAuxSelectionV1::new_v1(first, count).unwrap();
            let selected = first..first + count;
            assert_eq!(
                selection.native,
                selected.clone().any(|c| c < VALUE_WRITER_AUX)
            );
            assert_eq!(
                selection.writer,
                selected
                    .clone()
                    .any(|c| (VALUE_WRITER_AUX..VALUE_ARITHMETIC_COPY_AUX).contains(&c))
            );
            assert_eq!(
                selection.arithmetic_copy,
                selected.clone().any(|c| c >= VALUE_ARITHMETIC_COPY_AUX)
            );
            assert_eq!(selection.first, first);
            assert_eq!(selection.end, first + count);
        }
    }
    for (first, count) in [(0, 0), (0, 9), (width, 1), (width - 1, 2), (usize::MAX, 1)] {
        assert_eq!(
            P256ValueAuxSelectionV1::new_v1(first, count),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
    }
}

#[test]
fn value_auxiliary_compact_projection_matches_every_range_and_clears_padding() {
    let dense =
        core::array::from_fn::<_, P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1, _>(|column| {
            F(column as u64 + 1)
        });
    for count in 1..=P256_VALUE_AUX_REPLAY_WIDTH_V1 {
        for first in 0..=dense.len() - count {
            let selection = P256ValueAuxSelectionV1::new_v1(first, count).unwrap();
            let mut compact = [F::ZERO; P256_VALUE_AUX_REPLAY_WIDTH_V1];
            for range in [
                VALUE_NATIVE_AUX..VALUE_WRITER_AUX,
                VALUE_WRITER_AUX..VALUE_ARITHMETIC_COPY_AUX,
                VALUE_ARITHMETIC_COPY_AUX..dense.len(),
            ] {
                selection.copy_family_v1(range.start, &dense[range], &mut compact);
            }
            assert_eq!(&compact[..count], &dense[first..first + count]);
            assert!(compact[count..].iter().all(|&value| value == F::ZERO));
        }
    }
    clearing_failures::<P256_VALUE_AUX_REPLAY_WIDTH_V1>();
}

/// Original full-width selected replay retained only as a performance control.
fn full_width_selected_reference(
    stream: &mut P256ValueExecutionAggregateStreamV1<'_>,
    selection: P256ValueAuxSelectionV1,
) -> Result<Option<[F; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1]>, P256AggregateAdapterErrorV1> {
    if stream.next_row == P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1 {
        return Ok(None);
    }
    if stream.next_row > P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1 {
        return Err(P256AggregateAdapterErrorV1::Topology);
    }
    let mut aux =
        P256AggregateAuxRowScratchV1([F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1]);
    if selection.native {
        aux.0[VALUE_NATIVE_AUX..VALUE_WRITER_AUX].copy_from_slice(
            &stream
                .value_aux
                .as_mut()
                .ok_or(P256AggregateAdapterErrorV1::Challenge)?
                .next_aux_row_v1()?
                .ok_or(P256AggregateAdapterErrorV1::Topology)?,
        );
    }
    if selection.writer {
        let writer = stream
            .writer
            .as_mut()
            .ok_or(P256AggregateAdapterErrorV1::Challenge)?
            .next_row_v1()?
            .ok_or(P256AggregateAdapterErrorV1::Topology)?;
        aux.0[VALUE_WRITER_AUX..VALUE_ARITHMETIC_COPY_AUX]
            .copy_from_slice(&flatten_writer_aux_v1(&writer));
    }
    if selection.arithmetic_copy {
        let events =
            value_arithmetic_copy_events_v1(stream.next_row, stream.arithmetic_operations)?;
        let sources = p256_value_bus_opened_values_v1(&stream.base.base_row_v1(stream.next_row)?);
        stream.arithmetic_copy_running = build_compact_arithmetic_copy_aux_row_v1(
            &events,
            &sources,
            stream.arithmetic_copy_running,
            stream.arithmetic_copy_terminal,
            stream.arithmetic_copy_challenges,
            &mut aux.0[VALUE_ARITHMETIC_COPY_AUX..],
        )?;
    }
    stream.next_row += 1;
    if selection.arithmetic_copy
        && stream.next_row == P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1
        && stream.arithmetic_copy_running != stream.arithmetic_copy_terminal
    {
        return Err(P256AggregateAdapterErrorV1::Constraint);
    }
    Ok(Some(aux.0))
}

#[test]
#[ignore = "optimized native replay cost comparison; full source construction and all value columns"]
fn value_auxiliary_compact_projection_cost_diagnostic() {
    assert!(
        !cfg!(debug_assertions),
        "run the replay diagnostic in release mode"
    );
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let bound = base.bind_v1(&super::tests::main_post_base_v1(83)).unwrap();
    let registration = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap();
    let shape = registration.shape_v1().unwrap();
    let mut elapsed = [std::time::Duration::ZERO; 2];
    // Alternate order to avoid assigning all warm-cache runs to one path.
    for round in 0..2 {
        for first in (0..shape.aux_width).step_by(P256_VALUE_AUX_REPLAY_WIDTH_V1) {
            let count = (shape.aux_width - first).min(P256_VALUE_AUX_REPLAY_WIDTH_V1);
            let selection = P256ValueAuxSelectionV1::new_v1(first, count).unwrap();
            let mut reference =
                zeroize::Zeroizing::new(vec![vec![F::ZERO; shape.trace_size]; count]);
            let mut compact = zeroize::Zeroizing::new(vec![vec![F::ZERO; shape.trace_size]; count]);
            for mode in [round, 1 - round] {
                let mut stream = bound.value_execution_aux_stream_v1(registration).unwrap();
                let columns = if mode == 0 {
                    &mut reference
                } else {
                    &mut compact
                };
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let start = std::time::Instant::now();
                if mode == 0 {
                    fill_aggregate_aux_columns_v1::<P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1>(
                        shape.trace_size,
                        first,
                        &mut targets,
                        || full_width_selected_reference(&mut stream, selection),
                    )
                    .unwrap();
                } else {
                    fill_aggregate_aux_columns_v1::<P256_VALUE_AUX_REPLAY_WIDTH_V1>(
                        shape.trace_size,
                        0,
                        &mut targets,
                        || stream.next_selected_aux_columns_v1(selection),
                    )
                    .unwrap();
                }
                elapsed[mode] += start.elapsed();
            }
            assert_eq!(*reference, *compact);
        }
    }
    println!(
        "p256_value_aux_projection full_width_seconds={:.6} compact_seconds={:.6} speedup={:.3} native_rows={} columns={} rounds=2",
        elapsed[0].as_secs_f64(),
        elapsed[1].as_secs_f64(),
        elapsed[0].as_secs_f64() / elapsed[1].as_secs_f64(),
        shape.trace_size,
        shape.aux_width,
    );
}

#[test]
#[ignore = "full native value source, dense oracle and selected terminal failure"]
fn value_auxiliary_selected_replay_preserves_dense_cells_and_terminal_refusal() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let token = super::tests::main_post_base_v1(83);
    let bound = base.bind_v1(&token).unwrap();
    let registration = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap();
    let value = bound
        .signature_v1(registration)
        .unwrap()
        .value
        .as_ref()
        .unwrap();
    // These windows include both pure-family ranges and both family boundaries.
    for first in [
        0,
        VALUE_WRITER_AUX - 4,
        VALUE_WRITER_AUX,
        VALUE_ARITHMETIC_COPY_AUX - 4,
        VALUE_ARITHMETIC_COPY_AUX,
        P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1 - 8,
    ] {
        let selection = P256ValueAuxSelectionV1::new_v1(first, 8).unwrap();
        let mut selected = bound.value_execution_aux_stream_v1(registration).unwrap();
        let mut dense = P256ValueExecutionAggregateStreamV1::new_v1(value).unwrap();
        for _ in 0..P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1 {
            let actual = zeroize::Zeroizing::new(
                selected
                    .next_selected_aux_columns_v1(selection)
                    .unwrap()
                    .unwrap(),
            );
            let expected = zeroize::Zeroizing::new(dense.next_aux_row_v1().unwrap().unwrap());
            assert_eq!(&actual[..], &expected[first..first + 8]);
        }
        assert!(
            selected
                .next_selected_aux_columns_v1(selection)
                .unwrap()
                .is_none()
        );
        assert!(dense.next_aux_row_v1().unwrap().is_none());
    }
    let first = VALUE_ARITHMETIC_COPY_AUX;
    let selection = P256ValueAuxSelectionV1::new_v1(first, 1).unwrap();
    let mut selected = bound.value_execution_aux_stream_v1(registration).unwrap();
    selected.arithmetic_copy_terminal[0] = selected.arithmetic_copy_terminal[0].add(F::ONE);
    let mut column = zeroize::Zeroizing::new(vec![F(97); P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1]);
    assert_eq!(
        fill_aggregate_aux_columns_v1::<P256_VALUE_AUX_REPLAY_WIDTH_V1>(
            P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1,
            0,
            &mut [&mut column],
            || selected.next_selected_aux_columns_v1(selection)
        ),
        Err(P256AggregateAdapterErrorV1::Constraint)
    );
    assert!(column.iter().all(|v| *v == F::ZERO));

    // Fail between two selected families after the compact scratch contains
    // native products. Both that scratch and every destination must erase.
    let selection = P256ValueAuxSelectionV1::new_v1(VALUE_WRITER_AUX - 4, 8).unwrap();
    let mut selected = bound.value_execution_aux_stream_v1(registration).unwrap();
    selected.writer = None;
    let mut columns =
        zeroize::Zeroizing::new(vec![vec![F(97); P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1]; 8]);
    let (result, observations) = inspection::observe_v1(|| {
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        fill_aggregate_aux_columns_v1::<P256_VALUE_AUX_REPLAY_WIDTH_V1>(
            P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1,
            0,
            &mut targets,
            || selected.next_selected_aux_columns_v1(selection),
        )
    });
    assert_eq!(result, Err(P256AggregateAdapterErrorV1::Challenge));
    assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
    assert!(
        observations.iter().any(|item| {
            item.cells == P256_VALUE_AUX_REPLAY_WIDTH_V1 && item.nonzero_before > 0
        })
    );
    assert!(observations.iter().all(|item| item.nonzero_after == 0));
}
