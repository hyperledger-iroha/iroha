//! Bounded log16 auxiliary replay parity, source refusal and clearing controls.

use super::super::private_table::inspection;
use super::*;

fn clearing_cases<const WIDTH: usize>() {
    for failure in 0..4 {
        let mut columns = vec![vec![F(97); 16]; 8];
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut outputs = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let mut row = 0;
                fill_aggregate_aux_columns_v1::<WIDTH>(16, WIDTH - 8, &mut outputs, || {
                    row += 1;
                    match (failure, row) {
                        (0, 4) => return Err(P256AggregateAdapterErrorV1::Constraint),
                        (1, 4) => return Ok(None),
                        (2, 4) => panic!("injected log16 auxiliary unwind"),
                        _ => {}
                    }
                    Ok(Some([F(83); WIDTH]))
                })
            }))
        });
        assert!(matches!(result, Ok(Err(_))) || (failure == 2 && result.is_err()));
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
        assert_eq!(
            observations.iter().filter(|item| item.cells == 16).count(),
            8
        );
        assert!(
            observations
                .iter()
                .any(|item| item.cells == WIDTH && item.nonzero_before == WIDTH)
        );
        if failure == 3 {
            assert_eq!(
                observations
                    .iter()
                    .filter(|item| item.cells == WIDTH)
                    .count(),
                17
            );
        }
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn log16_auxiliary_rows_and_destinations_clear_on_error_short_extra_and_unwind() {
    clearing_cases::<P256_WINDOW_AGGREGATE_AUX_WIDTH_V1>();
    clearing_cases::<P256_CROSS_TRACE_SINK_AUX_WIDTH_V1>();
}

#[test]
fn log16_auxiliary_batch_rejects_shape_phase_and_unrelated_adapters_before_writing() {
    let bound = P256MainBoundSourceV1 {
        signatures: None,
        fixed: None,
        post_base: None,
        terminal_claims: None,
    };
    for adapter in [
        P256MainAdapterV1::WindowBatch,
        P256MainAdapterV1::BindingSink,
    ] {
        let registration = P256MainRegistrationV1::new_v1(0, adapter, 0).unwrap();
        let shape = registration.shape_v1().unwrap();
        for (count, first, rows) in [
            (0, 0, 8),
            (9, 0, 8),
            (1, usize::MAX, 8),
            (1, shape.aux_width, 8),
            (1, 0, 8),
        ] {
            let mut columns = vec![vec![F(97); rows]; count];
            let mut outputs = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            assert_eq!(
                bound.fill_log16_aux_columns_v1(registration, first, &mut outputs),
                Err(P256AggregateAdapterErrorV1::Topology)
            );
            assert!(columns.iter().flatten().all(|value| *value == F(97)));
        }
        let mut column = zeroize::Zeroizing::new(vec![F(97); shape.trace_size]);
        assert_eq!(
            bound.fill_log16_aux_columns_v1(registration, 0, &mut [&mut column]),
            Err(P256AggregateAdapterErrorV1::Phase)
        );
        assert!(column.iter().all(|value| *value == F(97)));
    }
    for adapter in [
        P256MainAdapterV1::Arithmetic,
        P256MainAdapterV1::ValueBus,
        P256MainAdapterV1::Reduction,
        P256MainAdapterV1::ScalarBitBus,
    ] {
        let registration = P256MainRegistrationV1::new_v1(0, adapter, 0).unwrap();
        let mut column = vec![F(97); 8];
        assert_eq!(
            bound.fill_log16_aux_columns_v1(registration, 0, &mut [&mut column]),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(column.iter().all(|value| *value == F(97)));
    }
    assert!(
        p256_log16_aux_replay_scratch_v1()
            <= P256MainBaseSourceV1::replay_scratch_forecast_v1().unwrap()
    );
}

#[test]
#[ignore = "full five-signature native log16 replay against the original per-column oracle"]
fn log16_auxiliary_batches_match_every_cell_and_boundary_column() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let bound = base.bind_v1(&super::tests::main_post_base_v1(83)).unwrap();
    let retained = bound.allocated_payload_bytes_v1();
    for signature in 0..P256_X5S1_SIGNATURES_V1 {
        for adapter in [
            P256MainAdapterV1::WindowBatch,
            P256MainAdapterV1::BindingSink,
        ] {
            let registration = P256MainRegistrationV1::new_v1(signature, adapter, 0).unwrap();
            let shape = registration.shape_v1().unwrap();
            let mut starts = (0..shape.aux_width).step_by(8).collect::<Vec<_>>();
            starts.extend([1, shape.aux_width - 8]);
            if adapter == P256MainAdapterV1::WindowBatch {
                starts.push(WINDOW_SCALAR_AUX - 4);
            }
            starts.sort_unstable();
            starts.dedup();
            for first in starts {
                let count = (shape.aux_width - first).min(8);
                let mut columns =
                    zeroize::Zeroizing::new(vec![vec![F::ZERO; shape.trace_size]; count]);
                let mut outputs = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                bound
                    .fill_log16_aux_columns_v1(registration, first, &mut outputs)
                    .unwrap();
                drop(outputs);
                for (offset, actual) in columns.iter().enumerate() {
                    let mut expected = zeroize::Zeroizing::new(vec![F::ZERO; shape.trace_size]);
                    bound
                        .fill_aux_column_v1(registration, first + offset, &mut expected)
                        .unwrap();
                    assert_eq!(
                        actual.as_slice(),
                        expected.as_slice(),
                        "signature={signature} adapter={adapter:?} column={}",
                        first + offset
                    );
                }
                assert_eq!(bound.allocated_payload_bytes_v1(), retained);
            }
        }
    }
}

#[test]
#[ignore = "full validated log16 source and populated native terminal failures"]
fn log16_auxiliary_batches_preserve_source_token_and_terminal_refusals() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let token = super::tests::main_post_base_v1(83);
    let mut bound = base.bind_v1(&token).unwrap();
    let window = P256MainRegistrationV1::new_v1(2, P256MainAdapterV1::WindowBatch, 0).unwrap();
    let sink = P256MainRegistrationV1::new_v1(2, P256MainAdapterV1::BindingSink, 0).unwrap();
    let rows = P256_WINDOW_AGGREGATE_TRACE_SIZE_V1;
    {
        let signature = bound.signature_v1(window).unwrap();
        for scalar in [false, true] {
            let mut stream = P256WindowAggregateAuxStreamV1::new_v1(
                signature.window.as_ref().unwrap(),
                bound
                    .cross_claim_v1(window, P256CrossTraceTerminalRoleV1::WindowBatch)
                    .unwrap()
                    .start,
                token.p256_cross(),
                token.p256_scalar(),
            )
            .unwrap();
            if scalar {
                stream.scalar_terminal[0] = stream.scalar_terminal[0].add(F::ONE);
            } else {
                stream.cross_terminal[0] = stream.cross_terminal[0].add(F::ONE);
            }
            let mut columns = zeroize::Zeroizing::new(vec![vec![F(97); rows]; 8]);
            let mut outputs = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            assert_eq!(
                stream.fill_aux_columns_v1(0, &mut outputs),
                Err(P256AggregateAdapterErrorV1::Constraint)
            );
            assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
        }
        let mut stream = P256BindingSinkAggregateStreamV1::new_with_optional_certificate_v1(
            signature.sink.as_ref().unwrap(),
            token.p256_cross(),
            true,
        )
        .unwrap();
        stream.terminal[0] = stream.terminal[0].add(F::ONE);
        let mut column = zeroize::Zeroizing::new(vec![F(97); rows]);
        assert_eq!(
            stream.fill_aux_columns_v1(0, &mut [&mut column]),
            Err(P256AggregateAdapterErrorV1::Constraint)
        );
        assert!(column.iter().all(|value| *value == F::ZERO));
    }
    // Construction retains the original full source validation before any writes.
    bound.signatures.as_mut().unwrap()[2]
        .window
        .as_mut()
        .unwrap()
        .aux[0][0] = F::ONE;
    let mut column = zeroize::Zeroizing::new(vec![F(97); rows]);
    assert_eq!(
        bound.fill_log16_aux_columns_v1(window, 0, &mut [&mut column]),
        Err(P256AggregateAdapterErrorV1::Topology)
    );
    assert!(column.iter().all(|value| *value == F(97)));
    bound.signatures.as_mut().unwrap()[2]
        .window
        .as_mut()
        .unwrap()
        .aux[0][0] = F::ZERO;
    let last = bound.signatures.as_mut().unwrap()[2]
        .sink
        .as_mut()
        .unwrap()
        .rows
        .pop()
        .unwrap();
    // The original sink constructor maps its malformed trace refusal through
    // the cross-trace error conversion, which is Constraint rather than the
    // adapter's own Topology error.
    assert_eq!(
        bound.fill_aux_column_v1(sink, 0, &mut column),
        Err(P256AggregateAdapterErrorV1::Constraint)
    );
    assert!(column.iter().all(|value| *value == F(97)));
    assert_eq!(
        bound.fill_log16_aux_columns_v1(sink, 0, &mut [&mut column]),
        Err(P256AggregateAdapterErrorV1::Constraint)
    );
    assert!(column.iter().all(|value| *value == F(97)));
    bound.signatures.as_mut().unwrap()[2]
        .sink
        .as_mut()
        .unwrap()
        .rows
        .push(last);
    bound.post_base = Some(super::tests::main_post_base_v1(89));
    for registration in [window, sink] {
        assert_eq!(
            bound.fill_log16_aux_columns_v1(registration, 0, &mut [&mut column]),
            Err(P256AggregateAdapterErrorV1::Challenge)
        );
        assert!(column.iter().all(|value| *value == F(97)));
    }
}

#[test]
#[ignore = "optimized full-shape log16 construction/replay cost diagnostic"]
fn log16_auxiliary_batch_cost_diagnostic() {
    assert!(!cfg!(debug_assertions), "run in release mode");
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let bound = base.bind_v1(&super::tests::main_post_base_v1(83)).unwrap();
    for adapter in [
        P256MainAdapterV1::WindowBatch,
        P256MainAdapterV1::BindingSink,
    ] {
        let registration = P256MainRegistrationV1::new_v1(2, adapter, 0).unwrap();
        let shape = registration.shape_v1().unwrap();
        let mut elapsed = [std::time::Duration::ZERO; 2];
        for round in 0..2 {
            for first in (0..shape.aux_width).step_by(8) {
                let count = (shape.aux_width - first).min(8);
                let mut scalar =
                    zeroize::Zeroizing::new(vec![vec![F::ZERO; shape.trace_size]; count]);
                let mut batch =
                    zeroize::Zeroizing::new(vec![vec![F::ZERO; shape.trace_size]; count]);
                for mode in [round, 1 - round] {
                    let start = std::time::Instant::now();
                    if mode == 0 {
                        for (offset, column) in scalar.iter_mut().enumerate() {
                            bound
                                .fill_aux_column_v1(registration, first + offset, column)
                                .unwrap();
                        }
                    } else {
                        let mut outputs =
                            batch.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>();
                        bound
                            .fill_log16_aux_columns_v1(registration, first, &mut outputs)
                            .unwrap();
                    }
                    elapsed[mode] += start.elapsed();
                }
                assert_eq!(*scalar, *batch);
            }
        }
        println!(
            "p256_log16_aux_batch adapter={adapter:?} scalar_seconds={:.6} batch_seconds={:.6} speedup={:.3} rows={} columns={} rounds=2",
            elapsed[0].as_secs_f64(),
            elapsed[1].as_secs_f64(),
            elapsed[0].as_secs_f64() / elapsed[1].as_secs_f64(),
            shape.trace_size,
            shape.aux_width
        );
    }
}
