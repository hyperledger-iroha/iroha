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
