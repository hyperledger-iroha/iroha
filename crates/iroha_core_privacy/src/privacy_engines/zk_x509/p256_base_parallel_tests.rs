//! Independent scalar parity, deterministic tile errors, custody and full-source costs.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture_row_v1(row: usize) -> [F; 34] {
    core::array::from_fn(|column| F(((row + 17) * (column + 29)) as u64))
}

#[test]
fn base_tiles_match_serial_for_selected_widths_boundaries_and_worker_counts() {
    let rows = MIN_PARALLEL_ROWS;
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            assert_eq!(prefer_tiles_v1(rows), workers > 1);
            assert!(!prefer_tiles_v1(TILE_ROWS));
            for width in [1, 4, 8] {
                let mut starts = vec![0, 1, 15, 16, 17, 26, 34 - width];
                starts.retain(|&first| first + width <= 34);
                starts.sort_unstable();
                starts.dedup();
                for first in starts {
                    let mut expected = zeroize::Zeroizing::new(vec![vec![F(97); rows]; width]);
                    let mut targets = expected
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>();
                    fill_aggregate_base_columns_v1(rows, first, &mut targets, |row| {
                        Ok(fixture_row_v1(row))
                    })
                    .unwrap();
                    drop(targets);
                    let mut actual = zeroize::Zeroizing::new(vec![vec![F(83); rows]; width]);
                    let allocations = actual
                        .iter()
                        .map(|c| (c.as_ptr(), c.len(), c.capacity()))
                        .collect::<Vec<_>>();
                    let mut targets = actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>();
                    fill_columns_v1(rows, first, &mut targets, |row| Ok(fixture_row_v1(row)))
                        .unwrap();
                    assert_eq!(
                        &*actual, &*expected,
                        "workers={workers} first={first} width={width}"
                    );
                    assert_eq!(
                        actual
                            .iter()
                            .map(|c| (c.as_ptr(), c.len(), c.capacity()))
                            .collect::<Vec<_>>(),
                        allocations
                    );
                }
            }
        });
    }
}

#[test]
fn base_tiles_select_the_earliest_error_after_join_and_clear_on_unwind() {
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            let visits: [AtomicUsize; TILES] = core::array::from_fn(|_| AtomicUsize::new(0));
            let mut columns = zeroize::Zeroizing::new(vec![vec![F(97); MAX_ROWS]; 4]);
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            let result = fill_with_v1(MAX_ROWS, 3, &mut targets, true, |row| {
                visits[row / TILE_ROWS].fetch_add(1, Ordering::Relaxed);
                if row == TILE_ROWS + 7 {
                    return Err(P256AggregateAdapterErrorV1::Source);
                }
                if row == 19 * TILE_ROWS + 3 {
                    return Err(P256AggregateAdapterErrorV1::Constraint);
                }
                Ok(fixture_row_v1(row))
            });
            assert_eq!(result, Err(P256AggregateAdapterErrorV1::Source));
            assert!(columns.iter().flatten().all(|&value| value == F::ZERO));
            for (tile, count) in visits.iter().enumerate() {
                assert_eq!(
                    count.load(Ordering::Relaxed),
                    match tile {
                        1 => 8,
                        19 => 4,
                        _ => TILE_ROWS,
                    }
                );
            }
            for fail_at in [0, TILE_ROWS + 7, MAX_ROWS - 1] {
                for column in columns.iter_mut() {
                    column.fill(F(97));
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut targets = columns
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>();
                    fill_with_v1(MAX_ROWS, 3, &mut targets, true, |row| {
                        assert_ne!(row, fail_at, "injected independent base row unwind");
                        Ok(fixture_row_v1(row))
                    })
                }));
                assert!(result.is_err());
                assert!(columns.iter().flatten().all(|&value| value == F::ZERO));
            }
        });
    }
}

#[test]
fn base_tiles_admit_only_bounded_shapes_before_source_or_mutation() {
    for (rows, first, count, actual_rows) in [
        (0, 0, 1, 0),
        (3, 0, 1, 3),
        (MAX_ROWS * 2, 0, 1, 8),
        (8, 0, 0, 8),
        (8, 0, 9, 8),
        (8, 34, 1, 8),
        (8, usize::MAX, 1, 8),
        (8, 30, 8, 8),
        (8, 0, 1, 7),
    ] {
        let mut columns = vec![vec![F(97); actual_rows]; count];
        let original = columns.clone();
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        assert_eq!(
            fill_with_v1::<34>(rows, first, &mut targets, true, |_| panic!(
                "malformed shape read private source"
            )),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert_eq!(columns, original);
    }
    let mut columns = vec![vec![F(97); 8], vec![F(97); 7]];
    let original = columns.clone();
    let mut targets = columns
        .iter_mut()
        .map(Vec::as_mut_slice)
        .collect::<Vec<_>>();
    assert_eq!(
        fill_with_v1::<34>(8, 0, &mut targets, true, |_| unreachable!()),
        Err(P256AggregateAdapterErrorV1::Topology)
    );
    assert_eq!(columns, original);
    let views = core::mem::size_of::<[Tile<'static>; TILES]>();
    assert_eq!(views, TILES * WIDTH * 2 * core::mem::size_of::<usize>());
    let charge = p256_base_replay_scratch_v1();
    assert!(charge >= TILES * core::mem::size_of::<P256AggregateAuxRowScratchV1<34>>() + views);
    assert!(charge < 64 * 1024);
    assert!(charge <= P256MainBaseSourceV1::replay_scratch_forecast_v1().unwrap());
    assert!(
        charge < super::super::super::allocation_payload::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1
    );
}

#[test]
fn arithmetic_base_tiles_preserve_every_cell_family_and_original_zero_padding() {
    let topology = verifier_topology_v1(P256EcdsaRoleV1::CertificateOrCrl).unwrap();
    let operations = topology
        .linked_operations
        .iter()
        .take(16)
        .map(|op| ZkX509P256ArithmeticTopologyV1 {
            kind: op.kind,
            modulus: op.modulus,
        })
        .collect::<Vec<_>>();
    let fixed = P256ArithmeticStarkFixedProviderV1::new_v1(&operations, MAX_ROWS).unwrap();
    let trace = ZkX509P256ArithmeticTraceV1 {
        fixed: Vec::new(),
        base: (0..512)
            .map(|row| core::array::from_fn(|col| F(((row + 1) * (col + 3)) as u64)))
            .collect(),
    };
    let source = P256ArithmeticAggregateRowsV1 {
        trace: P256ArithmeticRowsStorageV1::Raw(&trace),
        fixed: Cow::Borrowed(&fixed),
    };
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            for width in [1, 4, 8] {
                for first in [0, 63, 64, 127, 128, 177, 178, 179, 202, 203, 211 - width] {
                    if first + width > 211 {
                        continue;
                    }
                    let mut columns =
                        zeroize::Zeroizing::new(vec![vec![F(97); MIN_PARALLEL_ROWS]; width]);
                    let mut targets = columns
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>();
                    fill_cells_v1::<8>(MIN_PARALLEL_ROWS, 0, &mut targets, |row, cells| {
                        source.trace.fill_cells_v1(row, first, &mut cells[..width])
                    })
                    .unwrap();
                    for (offset, column) in columns.iter().enumerate() {
                        for (row, value) in column.iter().enumerate() {
                            let expected = if row < 512 {
                                F(((row + 1) * (first + offset + 3)) as u64)
                            } else {
                                F::ZERO
                            };
                            assert_eq!(
                                *value,
                                expected,
                                "workers={workers} row={row} column={}",
                                first + offset
                            );
                        }
                    }
                }
            }
        });
    }
}

fn scalar_source_v1(
    source: &P256MainBaseSourceV1,
    registration: P256MainRegistrationV1,
    first: usize,
    outputs: &mut [&mut [F]],
) -> Result<(), P256AggregateAdapterErrorV1> {
    let signature = source.signature_v1(registration)?;
    let width = outputs.len();
    if registration.adapter_v1() == P256MainAdapterV1::Arithmetic {
        let rows = signature
            .arithmetic
            .as_ref()
            .unwrap()
            .rows_v1(signature.role)?;
        // Exact original arithmetic row loop: one clearing eight-field row,
        // without an extra helper-owned scratch copy in the measured baseline.
        let mut destination = P256AggregateColumnsDestinationGuardV1 {
            outputs,
            committed: false,
        };
        for row in 0..MAX_ROWS {
            let mut cells = P256AggregateAuxRowScratchV1([F::ZERO; 8]);
            rows.trace
                .fill_cells_v1(row, first, &mut cells.0[..width])?;
            for (offset, output) in destination.outputs.iter_mut().enumerate() {
                output[row] = cells.0[offset];
            }
        }
        destination.committed = true;
        Ok(())
    } else {
        let value = signature.value.as_ref().unwrap();
        let endpoint = if registration.local_instance_v1() == 0 {
            P256ValueBusStarkEndpointV1::Execution
        } else {
            P256ValueBusStarkEndpointV1::Sorted
        };
        fill_aggregate_base_columns_v1::<34>(MAX_ROWS, first, outputs, |row| {
            Ok(value.base_row_v1(endpoint, row)?)
        })
    }
}

#[test]
fn base_tile_direct_cell_errors_and_unwind_clear_the_partial_row_owner() {
    use super::super::super::private_table::inspection;
    for unwind in [false, true] {
        let mut columns = vec![vec![F(97); 8]; 4];
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                fill_cells_with_v1::<8>(8, 0, &mut targets, false, |row, cells| {
                    cells.fill(F(83));
                    if row == 3 {
                        assert!(!unwind, "injected partially filled arithmetic row unwind");
                        return Err(P256AggregateAdapterErrorV1::Source);
                    }
                    Ok(())
                })
            }))
        });
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(P256AggregateAdapterErrorV1::Source));
        }
        assert!(columns.iter().flatten().all(|&value| value == F::ZERO));
        assert_eq!(erased.iter().filter(|item| item.cells == 8).count(), 8);
        assert!(erased.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
#[ignore = "full validated native19 base source parity and allocation/clear A/B; isolated optimized run"]
fn full_native_base_tiles_match_scalar_and_report_allocation_and_clear_costs() {
    use std::time::{Duration, Instant};
    assert!(!cfg!(debug_assertions), "use an optimized build");
    assert_eq!(
        rayon::current_num_threads(),
        20,
        "run with RAYON_NUM_THREADS=20"
    );
    assert_eq!(
        rayon::current_thread_index(),
        None,
        "measure external prover caller"
    );
    let source = p256_main_base_source_fixture_for_test_v1().unwrap();
    let payload = source.allocated_payload_bytes_v1();
    let cases = [
        (P256MainAdapterV1::Arithmetic, 0, 0),
        (P256MainAdapterV1::Arithmetic, 0, 64),
        (P256MainAdapterV1::Arithmetic, 0, 128),
        (P256MainAdapterV1::Arithmetic, 0, 203),
        (P256MainAdapterV1::ValueBus, 0, 0),
        (P256MainAdapterV1::ValueBus, 0, 26),
        (P256MainAdapterV1::ValueBus, 1, 0),
        (P256MainAdapterV1::ValueBus, 1, 26),
    ];
    for (adapter, local, first) in cases {
        let registration = P256MainRegistrationV1::new_v1(0, adapter, local).unwrap();
        let mut expected = zeroize::Zeroizing::new(vec![vec![F::ZERO; MAX_ROWS]; 8]);
        let mut targets = expected
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        scalar_source_v1(&source, registration, first, &mut targets).unwrap();
        drop(targets);
        for workers in [1, 4, 20] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            pool.install(|| {
                for width in [1, 4, 8] {
                    let mut actual = zeroize::Zeroizing::new(vec![vec![F::ZERO; MAX_ROWS]; width]);
                    let mut targets = actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>();
                    source.fill_base_columns_v1(registration, first, &mut targets).unwrap();
                    assert_eq!(actual.as_slice(), &expected[..width], "workers={workers} first={first} width={width} registration={registration:?}");
                }
            });
        }
        const ROUNDS: usize = 5;
        let mut times = [[Duration::ZERO; ROUNDS]; 2];
        let mut allocation = [Duration::ZERO; 2];
        let mut fill = [Duration::ZERO; 2];
        let mut clear = [Duration::ZERO; 2];
        for round in 0..=ROUNDS {
            for parallel in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                let mut actual = zeroize::Zeroizing::new(vec![vec![F::ZERO; MAX_ROWS]; 8]);
                let allocated = start.elapsed();
                let pointers = actual
                    .iter()
                    .map(|column| (column.as_ptr(), column.len(), column.capacity()))
                    .collect::<Vec<_>>();
                let mut targets = actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>();
                let start = Instant::now();
                if parallel {
                    source
                        .fill_base_columns_v1(registration, first, &mut targets)
                        .unwrap();
                } else {
                    scalar_source_v1(&source, registration, first, &mut targets).unwrap();
                }
                let filled = start.elapsed();
                drop(targets);
                assert_eq!(&*actual, &*expected);
                assert_eq!(
                    actual
                        .iter()
                        .map(|column| (column.as_ptr(), column.len(), column.capacity()))
                        .collect::<Vec<_>>(),
                    pointers
                );
                let start = Instant::now();
                drop(actual);
                let cleared = start.elapsed();
                if round != 0 {
                    let mode = usize::from(parallel);
                    allocation[mode] += allocated;
                    fill[mode] += filled;
                    clear[mode] += cleared;
                    times[mode][round - 1] = allocated + filled + cleared;
                }
            }
        }
        for parallel in [false, true] {
            let mode = usize::from(parallel);
            times[mode].sort_unstable();
            println!(
                "x509_base_tiles_cost registration={registration:?} first={first} width=8 rows={MAX_ROWS} workers=20 caller_context=external parallel={parallel} calls={ROUNDS} total_ns={} median_ns={} allocation_ns={} source_fill_and_row_clear_ns={} output_clear_ns={} retained_source_bytes={payload} scratch_bound_bytes={} parity=all_fields_exact allocations=unchanged",
                (allocation[mode] + fill[mode] + clear[mode]).as_nanos(),
                times[mode][ROUNDS / 2].as_nanos(),
                allocation[mode].as_nanos(),
                fill[mode].as_nanos(),
                clear[mode].as_nanos(),
                p256_base_replay_scratch_v1()
            );
        }
    }
    assert_eq!(source.allocated_payload_bytes_v1(), payload);
}
