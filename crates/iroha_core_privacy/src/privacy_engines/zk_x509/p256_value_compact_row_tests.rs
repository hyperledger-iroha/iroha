//! Compact execution projection retains all-family validation and clearing.

use super::super::private_table::inspection;
use super::*;

#[derive(Debug, PartialEq, Eq)]
struct WriterErasureV1 {
    cells: usize,
    nonzero_before: usize,
    nonzero_after: usize,
}

thread_local! {
    static WRITER_ERASURES: core::cell::RefCell<Option<Vec<WriterErasureV1>>> = const {
        core::cell::RefCell::new(None)
    };
}

fn writer_field_counts_v1(row: &P256CrossTraceWriterAuxRowV1) -> (usize, usize) {
    row.event_values
        .iter()
        .chain(row.powers.iter().flatten().flatten())
        .chain(row.selected_power.iter().flatten())
        .chain(row.product_before.iter().flatten())
        .chain(row.terminal.iter())
        .fold((0, 0), |(cells, nonzero), value| {
            (cells + 1, nonzero + usize::from(*value != F::ZERO))
        })
}

pub(super) fn writer_before_erase_v1(row: &P256CrossTraceWriterAuxRowV1) -> Option<(usize, usize)> {
    WRITER_ERASURES
        .with_borrow(|observations| observations.as_ref().map(|_| writer_field_counts_v1(row)))
}

pub(super) fn writer_after_erase_v1(
    before: Option<(usize, usize)>,
    row: &P256CrossTraceWriterAuxRowV1,
) {
    if let Some((cells, nonzero_before)) = before {
        let (after_cells, nonzero_after) = writer_field_counts_v1(row);
        assert_eq!(cells, after_cells);
        WRITER_ERASURES.with_borrow_mut(|observations| {
            observations.as_mut().unwrap().push(WriterErasureV1 {
                cells,
                nonzero_before,
                nonzero_after,
            });
        });
    }
}

fn observe_writer_erasure_v1<T>(operation: impl FnOnce() -> T) -> (T, Vec<WriterErasureV1>) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            WRITER_ERASURES.set(None);
        }
    }
    WRITER_ERASURES.with_borrow(|observations| assert!(observations.is_none()));
    WRITER_ERASURES.set(Some(Vec::new()));
    let _scope = Scope;
    let result = operation();
    let observations = WRITER_ERASURES.take().unwrap();
    (result, observations)
}

#[test]
fn compact_writer_owner_drop_erases_every_typed_field_normally_and_on_unwind() {
    for unwind in [false, true] {
        let reached = core::cell::Cell::new(false);
        let (result, observations) = observe_writer_erasure_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let writer =
                    zeroize::Zeroizing::new(P256ValueWriterAuxScratchV1(writer_fixture_v1()));
                assert_eq!(writer_field_counts_v1(&(*writer).0), (86, 86));
                reached.set(true);
                if unwind {
                    panic!("populated typed writer owner unwind");
                }
                drop(writer);
            }))
        });
        assert!(reached.get());
        assert_eq!(result.is_err(), unwind);
        assert_eq!(
            observations,
            [WriterErasureV1 {
                cells: 86,
                nonzero_before: 86,
                nonzero_after: 0,
            }]
        );
    }
}

fn writer_fixture_v1() -> P256CrossTraceWriterAuxRowV1 {
    let mut ordinal = 0_u64;
    let mut value = || {
        ordinal += 1;
        F(ordinal)
    };
    P256CrossTraceWriterAuxRowV1 {
        event_values: core::array::from_fn(|_| value()),
        powers: core::array::from_fn(|_| {
            core::array::from_fn(|_| core::array::from_fn(|_| value()))
        }),
        selected_power: core::array::from_fn(|_| core::array::from_fn(|_| value())),
        product_before: core::array::from_fn(|_| core::array::from_fn(|_| value())),
        terminal: core::array::from_fn(|_| value()),
    }
}

#[test]
fn compact_execution_projection_matches_every_width_offset_and_family_boundary() {
    let native: [F; P256_VALUE_BUS_STARK_AUX_WIDTH_V1] =
        core::array::from_fn(|column| F(u64::try_from(column + 101).unwrap()));
    let writer = writer_fixture_v1();
    let copy: [F; P256_VALUE_ARITHMETIC_COPY_AUX_WIDTH_V1] =
        core::array::from_fn(|column| F(u64::try_from(column + 201).unwrap()));
    let mut expected = [F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1];
    expected[VALUE_NATIVE_AUX..VALUE_WRITER_AUX].copy_from_slice(&native);
    expected[VALUE_WRITER_AUX..VALUE_ARITHMETIC_COPY_AUX]
        .copy_from_slice(&flatten_writer_aux_v1(writer));
    expected[VALUE_ARITHMETIC_COPY_AUX..].copy_from_slice(&copy);
    for width in 1..=8 {
        for first in 0..=expected.len() - width {
            let selection = P256ValueExecutionAuxSelectionV1::new_v1(first, width).unwrap();
            let mut actual = [F::ZERO; 8];
            selection
                .copy_flat_v1(VALUE_NATIVE_AUX, &native, &mut actual)
                .unwrap();
            selection.copy_writer_v1(&writer, &mut actual).unwrap();
            selection
                .copy_flat_v1(VALUE_ARITHMETIC_COPY_AUX, &copy, &mut actual)
                .unwrap();
            assert_eq!(&actual[..width], &expected[first..first + width]);
            assert!(actual[width..].iter().all(|value| *value == F::ZERO));
        }
    }
    for (column, value) in flatten_writer_aux_v1(writer).into_iter().enumerate() {
        assert_eq!(writer_aux_cell_v1(&writer, column).unwrap(), value);
    }
    for column in [P256_CROSS_TRACE_WRITER_AUX_WIDTH_V1, usize::MAX] {
        assert_eq!(
            writer_aux_cell_v1(&writer, column),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
    }
}

#[test]
fn compact_execution_projection_refuses_bad_ranges_before_output_changes() {
    for (first, width) in [(0, 0), (0, 9), (116, 1), (115, 2), (usize::MAX, 1)] {
        assert_eq!(
            P256ValueExecutionAuxSelectionV1::new_v1(first, width),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
    }
    let selection = P256ValueExecutionAuxSelectionV1::new_v1(0, 8).unwrap();
    for (first, length) in [(usize::MAX, 1), (116, 1), (115, 2)] {
        let mut output = [F(97); 8];
        assert_eq!(
            selection.copy_flat_v1(first, &vec![F(13); length], &mut output),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert_eq!(output, [F(97); 8]);
    }
}

#[test]
fn compact_execution_transit_fits_the_previous_full_row_charge() {
    let compact = 2 * core::mem::size_of::<P256AggregateAuxRowScratchV1<8>>()
        + core::mem::size_of::<P256ValueExecutionAuxSelectionV1>()
        + core::mem::size_of::<zeroize::Zeroizing<P256ValueWriterAuxScratchV1>>();
    assert!(compact <= core::mem::size_of::<P256AggregateAuxRowScratchV1<116>>());
    assert!(
        p256_value_aux_replay_scratch_v1()
            < super::super::allocation_payload::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1
    );
}

#[test]
#[ignore = "genuine bound MAIN source and populated compact row/destination erasure"]
fn compact_execution_real_rows_clear_on_failure_unwind_shortage_and_extra_row() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let bound = base.bind_v1(super::tests::main_post_base_v1(89)).unwrap();
    assert!(base.private_is_zeroized_v1());
    let registration = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap();
    let selection = P256ValueExecutionAuxSelectionV1::new_v1(0, 8).unwrap();
    for failure in 0..4 {
        let mut columns = vec![vec![F(97); 2]; 8];
        let reached = core::cell::Cell::new(false);
        let produced = core::cell::Cell::new(0);
        let ((result, observations), writer_observations) = observe_writer_erasure_v1(|| {
            inspection::observe_v1(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut replay = bound.value_execution_aux_stream_v1(registration).unwrap();
                    let mut targets = columns
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>();
                    fill_aggregate_aux_columns_v1::<8>(2, 0, &mut targets, || {
                        if failure == 2 && produced.get() == 1 {
                            reached.set(true);
                            return Ok(None);
                        }
                        replay.next_selected_aux_row_with_v1(selection, |row| {
                            assert!(row.iter().any(|value| *value != F::ZERO));
                            produced.set(produced.get() + 1);
                            if produced.get() == 2 && failure < 2 {
                                reached.set(true);
                                if failure == 1 {
                                    panic!("populated compact execution row unwind");
                                }
                                return Err(P256AggregateAdapterErrorV1::Constraint);
                            }
                            if failure == 3 && produced.get() == 3 {
                                reached.set(true);
                            }
                            Ok(())
                        })
                    })
                }))
            })
        });
        assert!(reached.get());
        assert_eq!(writer_observations.len(), produced.get());
        assert!(writer_observations.iter().all(|entry| entry.cells == 86
            && entry.nonzero_before > 0
            && entry.nonzero_after == 0));
        if failure == 1 {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err(_))));
        }
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
        let compact = observations
            .iter()
            .filter(|entry| entry.cells == 8)
            .collect::<Vec<_>>();
        assert!(compact.iter().any(|entry| entry.nonzero_before > 0));
        // Each successful actual row clears its producer and consumer. The
        // failure/unwind row clears its producer before reaching the consumer;
        // the forbidden extra row must still clear in both owners.
        let expected = match failure {
            0 | 1 => 3,
            2 => 2,
            3 => 6,
            _ => unreachable!(),
        };
        assert_eq!(compact.len(), expected);
        assert!(observations.iter().filter(|entry| entry.cells == 2).count() >= 8);
        assert!(
            observations
                .iter()
                .any(|entry| entry.cells == 12 && entry.nonzero_before > 0)
        );
        assert!(
            observations
                .iter()
                .any(|entry| entry.cells == 18 && entry.nonzero_before > 0)
        );
        assert!(observations.iter().all(|entry| entry.nonzero_after == 0));
    }
}
