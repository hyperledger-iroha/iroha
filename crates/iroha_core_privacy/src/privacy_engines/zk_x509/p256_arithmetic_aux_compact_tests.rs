//! Compact arithmetic projection parity, bound terminals and clearing failures.

use super::super::private_table::inspection;
use super::*;

// These synthetic rows exercise projection only. The ignored complete-native
// test below uses an independently bound, validated five-signature source.
fn projection_fixture_v1() -> (
    ZkX509P256ArithmeticTraceV1,
    P256ArithmeticStarkFixedProviderV1,
) {
    let topology = verifier_topology_v1(P256EcdsaRoleV1::CertificateOrCrl).unwrap();
    let operations = topology
        .linked_operations
        .iter()
        .take(16)
        .map(|operation| ZkX509P256ArithmeticTopologyV1 {
            kind: operation.kind,
            modulus: operation.modulus,
        })
        .collect::<Vec<_>>();
    let fixed = P256ArithmeticStarkFixedProviderV1::new_v1(
        &operations,
        P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1,
    )
    .unwrap();
    let trace = ZkX509P256ArithmeticTraceV1 {
        fixed: Vec::new(),
        base: (0..512)
            .map(|row| core::array::from_fn(|column| F(((row + 1) * (column + 3)) as u64)))
            .collect(),
    };
    (trace, fixed)
}

fn projection_stream_v1<'a>(
    trace: &'a ZkX509P256ArithmeticTraceV1,
    fixed: &'a P256ArithmeticStarkFixedProviderV1,
) -> P256ArithmeticAggregateAuxStreamV1<'a> {
    let token = super::tests::main_post_base_v1(101);
    P256ArithmeticAggregateAuxStreamV1 {
        rows: P256ArithmeticAggregateRowsV1 {
            trace: P256ArithmeticRowsStorageV1::Raw(trace),
            fixed: Cow::Borrowed(fixed),
        },
        scalar_challenges: token.p256_scalar(),
        scalar_running: [F(17); 4],
        scalar_terminal: [F(19); 4],
        arithmetic_copy_challenges: token.p256_arithmetic_copy(),
        arithmetic_copy_running: [F(23); 4],
        arithmetic_copy_terminal: [F(29); 4],
        next_row: 0,
        _not_copy: core::cell::Cell::new(()),
    }
}

#[test]
fn arithmetic_compact_projection_covers_every_range_and_identity_cell() {
    let dense = core::array::from_fn::<_, P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1, _>(|column| {
        if column == 0 {
            F::ZERO
        } else {
            F(column as u64 + 1)
        }
    });
    let running = [F(17), F(19), F(23), F(29)];
    let terminal = [F(31), F(37), F(41), F(43)];
    for width in 1..=8 {
        for first in 0..=dense.len() - width {
            let selection = P256ArithmeticAuxSelectionV1::new_v1(first, width).unwrap();
            assert_eq!((selection.first, selection.end), (first, first + width));
            let mut actual = [F::ZERO; 8];
            for range in [
                ARITHMETIC_SCALAR_AUX..ARITHMETIC_VALUE_COPY_AUX,
                ARITHMETIC_VALUE_COPY_AUX..dense.len(),
            ] {
                selection.copy_family_v1(range.start, &dense[range], &mut actual);
            }
            assert_eq!(&actual[..width], &dense[first..first + width]);
            assert!(actual[width..].iter().all(|&value| value == F::ZERO));

            let mut expected = [F::ZERO; P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1];
            let mut actual = [F::ZERO; 8];
            for (family, events) in [(ARITHMETIC_SCALAR_AUX, 8), (ARITHMETIC_VALUE_COPY_AUX, 3)] {
                let end = family + compact_aux_width_v1(events);
                fill_compact_identity_aux_row_v1(
                    events,
                    &running,
                    &terminal,
                    &mut expected[family..end],
                )
                .unwrap();
                selection.copy_identity_family_v1(family, events, &running, &terminal, &mut actual);
            }
            assert_eq!(&actual[..width], &expected[first..first + width]);
            assert!(actual[width..].iter().all(|&value| value == F::ZERO));
        }
    }
}

#[test]
fn arithmetic_compact_stream_matches_all_columns_and_family_seams() {
    let (trace, fixed) = projection_fixture_v1();
    let ranges = (0..72)
        .step_by(8)
        .map(|first| (first, 8))
        .chain([(0, 1), (1, 8), (47, 8), (48, 8), (49, 8), (71, 1)])
        .collect::<Vec<_>>();
    let mut dense = projection_stream_v1(&trace, &fixed);
    let mut selected = ranges
        .iter()
        .map(|_| projection_stream_v1(&trace, &fixed))
        .collect::<Vec<_>>();
    for row in 0..512 {
        let expected = zeroize::Zeroizing::new(dense.next_aux_row_v1().unwrap().unwrap());
        for ((first, width), stream) in ranges.iter().copied().zip(&mut selected) {
            let selection = P256ArithmeticAuxSelectionV1::new_v1(first, width).unwrap();
            let actual = zeroize::Zeroizing::new(
                stream
                    .next_selected_aux_columns_v1(selection)
                    .unwrap()
                    .unwrap(),
            );
            assert_eq!(
                &actual[..width],
                &expected[first..first + width],
                "row={row} first={first} width={width}"
            );
            assert!(actual[width..].iter().all(|&value| value == F::ZERO));
            if !selection.scalar {
                assert_eq!(stream.scalar_running, [F(17); 4]);
            }
            if !selection.value_copy {
                assert_eq!(stream.arithmetic_copy_running, [F(23); 4]);
            }
        }
    }
}

#[test]
fn arithmetic_compact_terminal_checks_cover_selected_families_and_native_end() {
    let (trace, fixed) = projection_fixture_v1();
    for (first, width) in [(0, 1), (1, 8), (47, 8), (49, 8), (64, 8)] {
        let selection = P256ArithmeticAuxSelectionV1::new_v1(first, width).unwrap();
        for corrupt in [None, Some(false), Some(true)] {
            let mut stream = projection_stream_v1(&trace, &fixed);
            stream.next_row = P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1 - 1;
            stream.scalar_terminal = stream.scalar_running;
            stream.arithmetic_copy_terminal = stream.arithmetic_copy_running;
            match corrupt {
                Some(false) => stream.scalar_terminal[0] = F(101),
                Some(true) => stream.arithmetic_copy_terminal[0] = F(103),
                None => {}
            }
            let should_fail = matches!(corrupt, Some(false)) && selection.scalar
                || matches!(corrupt, Some(true)) && selection.value_copy;
            let mut columns = vec![vec![F(97)]; width];
            let (result, observations) = inspection::observe_v1(|| {
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                fill_aggregate_aux_columns_v1::<8>(1, 0, &mut targets, || {
                    stream.next_selected_aux_columns_v1(selection)
                })
            });
            if should_fail {
                assert_eq!(result, Err(P256AggregateAdapterErrorV1::Constraint));
                assert!(columns.iter().flatten().all(|&value| value == F::ZERO));
            } else {
                result.unwrap();
                assert!(
                    stream
                        .next_selected_aux_columns_v1(selection)
                        .unwrap()
                        .is_none()
                );
            }
            assert!(observations.iter().all(|item| item.nonzero_after == 0));
        }
    }
    let selection = P256ArithmeticAuxSelectionV1::new_v1(0, 8).unwrap();
    let mut stream = projection_stream_v1(&trace, &fixed);
    stream.next_row = P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1 + 1;
    assert_eq!(
        stream.next_selected_aux_columns_v1(selection),
        Err(P256AggregateAdapterErrorV1::Topology)
    );
}

#[test]
fn arithmetic_compact_projection_clears_partial_family_and_failed_destination() {
    let (mut trace, fixed) = projection_fixture_v1();
    trace.base.truncate(417);
    let selection = P256ArithmeticAuxSelectionV1::new_v1(47, 8).unwrap();
    let mut columns = vec![vec![F(97); 512]; 8];
    let (result, observations) = inspection::observe_v1(|| {
        let mut stream = projection_stream_v1(&trace, &fixed);
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        fill_aggregate_aux_columns_v1::<8>(512, 0, &mut targets, || {
            stream.next_selected_aux_columns_v1(selection)
        })
    });
    assert_eq!(result, Err(P256AggregateAdapterErrorV1::Source));
    assert!(columns.iter().flatten().all(|&value| value == F::ZERO));
    assert!(
        observations
            .iter()
            .any(|item| item.cells == 8 && item.nonzero_before > 0)
    );
    assert!(observations.iter().all(|item| item.nonzero_after == 0));

    let topology = verifier_topology_v1(P256EcdsaRoleV1::CertificateOrCrl).unwrap();
    let operations = topology
        .linked_operations
        .iter()
        .take(8)
        .map(|operation| ZkX509P256ArithmeticTopologyV1 {
            kind: operation.kind,
            modulus: operation.modulus,
        })
        .collect::<Vec<_>>();
    let truncated_fixed = P256ArithmeticStarkFixedProviderV1::new_v1(&operations, 256).unwrap();
    let (result, observations) = inspection::observe_v1(|| {
        let mut stream = projection_stream_v1(&trace, &truncated_fixed);
        stream.next_row = 416;
        let result = stream.next_selected_aux_columns_v1(selection);
        assert_eq!(stream.next_row, 416);
        result
    });
    assert!(result.is_err());
    assert!(
        observations
            .iter()
            .any(|item| item.cells == 8 && item.nonzero_before > 0)
    );
    assert!(observations.iter().all(|item| item.nonzero_after == 0));
}

#[test]
#[ignore = "complete native arithmetic rows for every signature and all 72 columns"]
fn arithmetic_compact_full_native_rows_match_dense_and_bound_terminals() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let bound = base.bind_v1(&super::tests::main_post_base_v1(101)).unwrap();
    let retained = bound.allocated_payload_bytes_v1();
    for signature in 0..P256_X5S1_SIGNATURES_V1 {
        let registration =
            P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Arithmetic, 0).unwrap();
        let mut dense = bound.arithmetic_aux_stream_v1(registration).unwrap();
        let mut batches = (0..9)
            .map(|_| bound.arithmetic_aux_stream_v1(registration).unwrap())
            .collect::<Vec<_>>();
        for row in 0..P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1 {
            let expected = zeroize::Zeroizing::new(dense.next_aux_row_v1().unwrap().unwrap());
            for (batch, stream) in batches.iter_mut().enumerate() {
                let first = batch * 8;
                let selection = P256ArithmeticAuxSelectionV1::new_v1(first, 8).unwrap();
                let actual = zeroize::Zeroizing::new(
                    stream
                        .next_selected_aux_columns_v1(selection)
                        .unwrap()
                        .unwrap(),
                );
                assert_eq!(
                    &actual[..],
                    &expected[first..first + 8],
                    "signature={signature} row={row} first={first}"
                );
            }
        }
        assert!(dense.next_aux_row_v1().unwrap().is_none());
        for (batch, stream) in batches.iter_mut().enumerate() {
            let selection = P256ArithmeticAuxSelectionV1::new_v1(batch * 8, 8).unwrap();
            assert!(
                stream
                    .next_selected_aux_columns_v1(selection)
                    .unwrap()
                    .is_none()
            );
            if selection.scalar {
                assert_eq!(stream.scalar_running, dense.scalar_running);
                assert_eq!(stream.scalar_running, stream.scalar_terminal);
            }
            if selection.value_copy {
                assert_eq!(
                    stream.arithmetic_copy_running,
                    dense.arithmetic_copy_running
                );
                assert_eq!(
                    stream.arithmetic_copy_running,
                    stream.arithmetic_copy_terminal
                );
            }
        }
        assert_eq!(bound.allocated_payload_bytes_v1(), retained);
    }
}
