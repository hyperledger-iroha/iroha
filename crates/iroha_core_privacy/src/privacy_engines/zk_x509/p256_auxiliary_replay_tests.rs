//! Bound arithmetic terminal reuse, bounded replay, and destination clearing.

use super::super::private_table::inspection;
use super::*;

std::thread_local! {
    static TERMINAL_DERIVATIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static ARITHMETIC_ROWS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

pub(super) fn record_terminal_derivation_v1() {
    TERMINAL_DERIVATIONS.with(|count| count.set(count.get() + 1));
}
pub(super) fn record_arithmetic_row_v1() {
    ARITHMETIC_ROWS.with(|count| count.set(count.get() + 1));
}
fn counts_v1() -> (usize, usize) {
    (
        TERMINAL_DERIVATIONS.with(core::cell::Cell::get),
        ARITHMETIC_ROWS.with(core::cell::Cell::get),
    )
}

#[test]
fn arithmetic_auxiliary_batch_projects_every_width_and_nonzero_offset() {
    for width in 1..=crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1 {
        for first in [0, 1, 72 - width] {
            let mut columns = vec![vec![F(99); 8]; width];
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            let mut row = 0;
            fill_aggregate_aux_columns_v1::<72>(8, first, &mut targets, || {
                if row == 8 {
                    return Ok(None);
                }
                let values = core::array::from_fn(|column| F((row * 72 + column + 1) as u64));
                row += 1;
                Ok(Some(values))
            })
            .unwrap();
            for (column, values) in columns.iter().enumerate() {
                for (row, value) in values.iter().enumerate() {
                    assert_eq!(*value, F((row * 72 + first + column + 1) as u64));
                }
            }
        }
    }
}

#[test]
fn arithmetic_auxiliary_batch_rejects_shape_before_source_or_destination_changes() {
    for (width, first, rows, len) in [
        (0, 0, 8, 8),
        (9, 0, 8, 8),
        (8, 65, 8, 8),
        (1, usize::MAX, 8, 8),
        (1, 0, 0, 0),
        (1, 0, 7, 7),
        (2, 0, 8, 7),
    ] {
        let mut columns = vec![vec![F(97); len]; width];
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        assert_eq!(
            fill_aggregate_aux_columns_v1::<72>(
                rows,
                first,
                &mut targets,
                || -> Result<Option<[F; 72]>, _> { panic!("invalid shape reached source") }
            ),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(columns.iter().flatten().all(|value| *value == F(97)));
    }
}

#[test]
fn arithmetic_auxiliary_batch_clears_all_destinations_on_late_failure_and_unwind() {
    for failure in 0..4 {
        let mut columns = vec![vec![F(97); 8]; 8];
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let mut row = 0;
                fill_aggregate_aux_columns_v1::<72>(8, 4, &mut targets, || {
                    row += 1;
                    match (failure, row) {
                        (0, 4) => return Err(P256AggregateAdapterErrorV1::Constraint),
                        (1, 4) => return Ok(None),
                        (2, 4) => panic!("injected late arithmetic replay unwind"),
                        _ => {}
                    }
                    Ok(Some([F(83); 72])) // case 3 emits a forbidden extra row
                })
            }))
        });
        assert!(matches!(result, Ok(Err(_))) || (failure == 2 && result.is_err()));
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
        assert_eq!(
            observations.iter().filter(|item| item.cells == 8).count(),
            8
        );
        assert!(
            observations
                .iter()
                .any(|item| item.cells == 72 && item.nonzero_before == 72)
        );
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn arithmetic_auxiliary_factory_rejects_missing_phase_and_other_adapters() {
    let bound = P256MainBoundSourceV1 {
        signatures: None,
        fixed: None,
        post_base: None,
        terminal_claims: None,
    };
    let arithmetic = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0).unwrap();
    assert!(matches!(
        bound.arithmetic_aux_stream_v1(arithmetic),
        Err(P256AggregateAdapterErrorV1::Phase)
    ));
    let other = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap();
    assert!(matches!(
        bound.arithmetic_aux_stream_v1(other),
        Err(P256AggregateAdapterErrorV1::Topology)
    ));
    assert!(core::mem::size_of::<P256ArithmeticAggregateAuxStreamV1<'static>>() <= 512);
    assert_eq!(
        core::mem::size_of::<P256AggregateAuxRowScratchV1<72>>(),
        576
    );
    let scratch = p256_arithmetic_aux_replay_scratch_v1();
    assert!(scratch <= P256MainBaseSourceV1::replay_scratch_forecast_v1().unwrap());
    assert!(scratch < super::super::allocation_payload::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1);
    eprintln!(
        "arithmetic auxiliary replay stack-owned payload: {scratch} bytes; extra retained payload=0; native eight-column payload=33554432"
    );
}

#[test]
#[ignore = "actual five-signature bind and every arithmetic auxiliary cell; optimized qualification"]
fn arithmetic_auxiliary_bound_terminals_and_batches_match_checked_raw_streams() {
    for seed in [71, 79] {
        let before = counts_v1();
        let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
        let token = super::tests::main_post_base_v1(seed);
        let mut bound = base.bind_v1(token).unwrap();
        assert_eq!(counts_v1().0, before.0 + P256_X5S1_SIGNATURES_V1);
        assert!(base.private_is_zeroized_v1());
        let retained = bound.allocated_payload_bytes_v1();
        for signature in [0, P256_X5S1_SIGNATURES_V1 - 1] {
            let registration =
                P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Arithmetic, 0)
                    .unwrap();
            let owner = bound
                .signature_v1(registration)
                .unwrap()
                .arithmetic
                .as_ref()
                .unwrap();
            let reference = P256ArithmeticAggregateAuxStreamV1::new_v1(
                registration.role_v1(),
                owner.trace_v1(),
                token.p256_scalar(),
                token.p256_arithmetic_copy(),
            )
            .unwrap();
            let count = counts_v1();
            let cached = bound.arithmetic_aux_stream_v1(registration).unwrap();
            assert_eq!(cached.terminal_v1(), reference.terminal_v1());
            assert_eq!(
                cached.arithmetic_copy_terminal_v1(),
                reference.arithmetic_copy_terminal_v1()
            );
            assert_eq!(counts_v1(), count);
            drop((cached, reference));
            // The first token checks every cell; the second independently
            // bound token also checks one full native batch per role.
            let starts = if seed == 71 {
                (0..72).step_by(8).collect::<Vec<_>>()
            } else {
                vec![48]
            };
            for first in starts {
                let mut columns = vec![vec![F(97); P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1]; 8];
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                let before = counts_v1();
                bound
                    .fill_arithmetic_aux_columns_v1(registration, first, &mut targets)
                    .unwrap();
                assert_eq!(
                    counts_v1(),
                    (before.0, before.1 + P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1)
                );
                let mut reference = P256ArithmeticAggregateAuxStreamV1::new_v1(
                    registration.role_v1(),
                    owner.trace_v1(),
                    token.p256_scalar(),
                    token.p256_arithmetic_copy(),
                )
                .unwrap();
                for row in 0..P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1 {
                    let expected = reference.next_aux_row_v1().unwrap().unwrap();
                    for offset in 0..8 {
                        assert_eq!(columns[offset][row], expected[first + offset]);
                    }
                }
                assert!(reference.next_aux_row_v1().unwrap().is_none());
                assert_eq!(bound.allocated_payload_bytes_v1(), retained);
            }
        }
        // Exact token association is checked against both bound child owners.
        bound.post_base = Some(super::tests::main_post_base_v1(seed + 1));
        let registration =
            P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0).unwrap();
        assert!(matches!(
            bound.arithmetic_aux_stream_v1(registration),
            Err(P256AggregateAdapterErrorV1::Challenge)
        ));
        bound.post_base = Some(token);
        assert!(bound.arithmetic_aux_stream_v1(registration).is_ok());
    }
}

#[test]
fn arithmetic_selected_families_follow_every_public_range_and_reject_overflow() {
    for width in 1..=8 {
        for first in 0..=72 - width {
            let selected = P256ArithmeticAuxSelectionV1::new_v1(first, width).unwrap();
            assert_eq!(
                selected.scalar,
                (first..first + width).any(|column| (1..49).contains(&column))
            );
            assert_eq!(
                selected.value_copy,
                (first..first + width).any(|column| (49..72).contains(&column))
            );
        }
    }
    for (first, width) in [(0, 0), (0, 9), (72, 1), (71, 2), (usize::MAX, 1)] {
        assert_eq!(
            P256ArithmeticAuxSelectionV1::new_v1(first, width),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
    }
    let zero = P256ArithmeticAuxSelectionV1::new_v1(0, 1).unwrap();
    assert!(!zero.scalar && !zero.value_copy);
}

#[test]
fn arithmetic_identity_rows_match_dense_factor_evaluation_with_nonzero_sources() {
    for seed in [17, 29, 83] {
        let token = super::tests::main_post_base_v1(seed);
        let before = core::array::from_fn(|lane| F((lane as u64 + 7) * 11));
        let terminal = core::array::from_fn(|lane| F((lane as u64 + 5) * 37));
        for slots in [3, 8] {
            let mut actual = vec![F(91); compact_aux_width_v1(slots)];
            let mut expected = actual.clone();
            fill_compact_identity_aux_row_v1(slots, &before, &terminal, &mut actual).unwrap();
            let sources = vec![F(321); slots];
            let after = if slots == 8 {
                build_compact_scalar_aux_row_v1(
                    &[P256ScalarSourceEventFixedV1::inactive_v1(); 8],
                    &sources,
                    before,
                    terminal,
                    token.p256_scalar(),
                    &mut expected,
                )
                .unwrap()
            } else {
                build_compact_arithmetic_copy_aux_row_v1(
                    &[P256ArithmeticCopyEventFixedV1::inactive_v1(); 3],
                    &sources,
                    before,
                    terminal,
                    token.p256_arithmetic_copy(),
                    &mut expected,
                )
                .unwrap()
            };
            assert_eq!(after, before);
            assert_eq!(actual, expected);
        }
    }
    for (slots, width) in [(0, 8), (4, 28), (3, 22), (8, 47), (usize::MAX, 1)] {
        let mut target = vec![F(97); width];
        assert_eq!(
            fill_compact_identity_aux_row_v1(slots, &[F::ONE; 4], &[F(13); 4], &mut target),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(target.iter().all(|value| *value == F(97)));
    }
}

#[test]
fn arithmetic_selected_public_activity_matches_every_fixed_native_row() {
    let logical = P256_ARITHMETIC_OPERATIONS_V1 * P256_ARITHMETIC_ROWS_PER_OPERATION_V1;
    let mut scalar_count = 0;
    let mut copy_count = 0;
    for row in 0..P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1 {
        let scalar = matches!(row / P256_ARITHMETIC_ROWS_PER_OPERATION_V1, 13 | 14);
        let copy = row < logical && row % P256_ARITHMETIC_ROWS_PER_OPERATION_V1 < 16;
        let scalar_events = arithmetic_scalar_events_v1(row).unwrap();
        let copy_events = arithmetic_value_copy_events_v1(row, logical).unwrap();
        assert_eq!(
            scalar_events.iter().all(|event| event.active == F::ONE),
            scalar
        );
        assert_eq!(copy_events.iter().all(|event| event.active == F::ONE), copy);
        if !scalar {
            assert!(
                scalar_events
                    .iter()
                    .all(|event| *event == P256ScalarSourceEventFixedV1::inactive_v1())
            );
        }
        if !copy {
            assert!(
                copy_events
                    .iter()
                    .all(|event| *event == P256ArithmeticCopyEventFixedV1::inactive_v1())
            );
        }
        scalar_count += usize::from(scalar);
        copy_count += usize::from(copy);
    }
    assert_eq!(scalar_count, 64);
    assert_eq!(copy_count, 14_828 * 16);
}

// This projection-only fixture deliberately has no arithmetic validity claim.
// Real validated owners and all padded rows are checked separately below.
fn selected_projection_stream_v1<'a>(
    trace: &'a ZkX509P256ArithmeticTraceV1,
    fixed: &'a P256ArithmeticStarkFixedProviderV1,
) -> P256ArithmeticAggregateAuxStreamV1<'a> {
    let token = super::tests::main_post_base_v1(97);
    P256ArithmeticAggregateAuxStreamV1 {
        rows: P256ArithmeticAggregateRowsV1 {
            trace,
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
fn selected_projection_trace_v1() -> (
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

#[test]
fn arithmetic_selected_stream_matches_dense_at_scalar_copy_and_reserved_seams() {
    let (trace, fixed) = selected_projection_trace_v1();
    let ranges = [
        (0, 1),
        (0, 8),
        (1, 8),
        (8, 8),
        (16, 8),
        (24, 8),
        (32, 8),
        (40, 8),
        (47, 8),
        (48, 8),
        (49, 8),
        (56, 8),
        (64, 8),
        (71, 1),
    ];
    let mut dense = selected_projection_stream_v1(&trace, &fixed);
    let mut selected = ranges
        .iter()
        .map(|_| selected_projection_stream_v1(&trace, &fixed))
        .collect::<Vec<_>>();
    for row in 0..512 {
        let expected = dense.next_aux_row_v1().unwrap().unwrap();
        for ((first, width), stream) in ranges.iter().copied().zip(&mut selected) {
            let selection = P256ArithmeticAuxSelectionV1::new_v1(first, width).unwrap();
            let actual = stream.next_selected_aux_row_v1(selection).unwrap().unwrap();
            assert_eq!(
                &actual[first..first + width],
                &expected[first..first + width],
                "row={row} first={first} width={width}"
            );
            assert_eq!(actual[0], F::ZERO);
            if !selection.scalar {
                assert_eq!(stream.scalar_running, [F(17); 4]);
                assert!(actual[1..49].iter().all(|value| *value == F::ZERO));
            }
            if !selection.value_copy {
                assert_eq!(stream.arithmetic_copy_running, [F(23); 4]);
                assert!(actual[49..].iter().all(|value| *value == F::ZERO));
            }
        }
    }
    assert!(
        p256_arithmetic_aux_replay_scratch_v1()
            < super::super::allocation_payload::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1
    );
}

#[test]
fn arithmetic_selected_stream_clears_partial_rows_and_destinations_on_source_failure() {
    let (mut trace, fixed) = selected_projection_trace_v1();
    // Scalar row416 exists, the same public active schedule next reaches a
    // missing private row417. The caller's complete destination must clear.
    trace.base.truncate(417);
    let selection = P256ArithmeticAuxSelectionV1::new_v1(47, 8).unwrap();
    let mut columns = vec![vec![F(97); 512]; 8];
    let (result, observations) = inspection::observe_v1(|| {
        let mut stream = selected_projection_stream_v1(&trace, &fixed);
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        fill_aggregate_aux_columns_v1::<72>(512, 47, &mut targets, || {
            stream.next_selected_aux_row_v1(selection)
        })
    });
    assert_eq!(result, Err(P256AggregateAdapterErrorV1::Source));
    assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
    assert_eq!(
        observations.iter().filter(|item| item.cells == 512).count(),
        8
    );
    assert!(
        observations
            .iter()
            .any(|item| item.cells == 72 && item.nonzero_before > 0)
    );
    assert!(
        observations
            .iter()
            .any(|item| item.cells == 8 && item.nonzero_before > 0)
    );
    assert!(observations.iter().all(|item| item.nonzero_after == 0));
}

#[test]
fn arithmetic_selected_partial_scalar_row_is_erased_if_fixed_replay_fails() {
    let (trace, _) = selected_projection_trace_v1();
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
    let selection = P256ArithmeticAuxSelectionV1::new_v1(47, 8).unwrap();
    let (result, observations) = inspection::observe_v1(|| {
        let mut stream = selected_projection_stream_v1(&trace, &truncated_fixed);
        stream.next_row = 416;
        let result = stream.next_selected_aux_row_v1(selection);
        assert_eq!(stream.next_row, 416);
        result
    });
    assert!(result.is_err());
    assert!(
        observations
            .iter()
            .any(|item| item.cells == 72 && item.nonzero_before > 0)
    );
    assert!(
        observations
            .iter()
            .any(|item| item.cells == 8 && item.nonzero_before > 0)
    );
    assert!(observations.iter().all(|item| item.nonzero_after == 0));
}

#[test]
#[ignore = "actual five-signature bound owner, two tokens and complete native arithmetic parity"]
fn arithmetic_selected_full_native_rows_match_dense_for_all_signatures_and_tokens() {
    for seed in [71, 79] {
        let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
        let bound = base.bind_v1(super::tests::main_post_base_v1(seed)).unwrap();
        assert!(base.private_is_zeroized_v1());
        let retained = bound.allocated_payload_bytes_v1();
        for signature in 0..P256_X5S1_SIGNATURES_V1 {
            let registration =
                P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Arithmetic, 0)
                    .unwrap();
            let mut dense = bound.arithmetic_aux_stream_v1(registration).unwrap();
            let mut batches = (0..9)
                .map(|_| bound.arithmetic_aux_stream_v1(registration).unwrap())
                .collect::<Vec<_>>();
            for row in 0..P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1 {
                let expected = dense.next_aux_row_v1().unwrap().unwrap();
                for (batch, stream) in batches.iter_mut().enumerate() {
                    let first = batch * 8;
                    let selection = P256ArithmeticAuxSelectionV1::new_v1(first, 8).unwrap();
                    let actual = stream.next_selected_aux_row_v1(selection).unwrap().unwrap();
                    assert_eq!(
                        &actual[first..first + 8],
                        &expected[first..first + 8],
                        "seed={seed} signature={signature} row={row} batch={batch}"
                    );
                }
            }
            assert!(dense.next_aux_row_v1().unwrap().is_none());
            for (batch, stream) in batches.iter_mut().enumerate() {
                let selection = P256ArithmeticAuxSelectionV1::new_v1(batch * 8, 8).unwrap();
                assert!(
                    stream
                        .next_selected_aux_row_v1(selection)
                        .unwrap()
                        .is_none()
                );
                if selection.scalar {
                    assert_eq!(stream.scalar_running, dense.scalar_running);
                }
                if selection.value_copy {
                    assert_eq!(
                        stream.arithmetic_copy_running,
                        dense.arithmetic_copy_running
                    );
                }
                assert_eq!(stream.scalar_terminal, dense.scalar_terminal);
                assert_eq!(
                    stream.arithmetic_copy_terminal,
                    dense.arithmetic_copy_terminal
                );
            }
            assert_eq!(bound.allocated_payload_bytes_v1(), retained);
        }
    }
}
