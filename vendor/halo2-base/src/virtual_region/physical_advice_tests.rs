//! Independent HashMap oracle for exact coordinate storage and copy-edge replay.

use std::{
    collections::HashMap,
    panic::{catch_unwind, AssertUnwindSafe},
};

use super::PhysicalAdviceMap;
use crate::{
    halo2_proofs::{
        circuit::Cell,
        halo2curves::bn256::Fr,
        plonk::{Any, Column, ConstraintSystem, FirstPhase, SecondPhase, ThirdPhase},
    },
    ContextCell, EXTERNAL_CELL_TYPE_ID, FIRST_PHASE_CELL_TYPE_ID, SECOND_PHASE_CELL_TYPE_ID,
    THIRD_PHASE_CELL_TYPE_ID,
};

fn columns() -> [Column<Any>; 5] {
    let mut meta = ConstraintSystem::<Fr>::default();
    [
        meta.advice_column_in(FirstPhase).into(),
        meta.advice_column_in(SecondPhase).into(),
        meta.advice_column_in(ThirdPhase).into(),
        meta.fixed_column().into(),
        meta.instance_column().into(),
    ]
}

fn coordinate(cell: Cell) -> (Column<Any>, usize) {
    (cell.column, cell.row_offset)
}
fn same(left: Option<Cell>, right: Option<Cell>) {
    assert_eq!(left.map(coordinate), right.map(coordinate));
}
fn id(phase: usize, context: usize, offset: usize) -> ContextCell {
    ContextCell::new(
        [
            FIRST_PHASE_CELL_TYPE_ID,
            SECOND_PHASE_CELL_TYPE_ID,
            THIRD_PHASE_CELL_TYPE_ID,
            EXTERNAL_CELL_TYPE_ID,
        ][phase],
        context,
        offset,
    )
}
fn check(map: &PhysicalAdviceMap, oracle: &HashMap<ContextCell, Cell>) {
    assert_eq!(map.len(), oracle.len());
    assert_eq!(map.is_empty(), oracle.is_empty());
    assert!(map.run_count() <= map.len());
    assert!(map.checked_run_capacity_bytes().is_some());
    for (key, expected) in oracle {
        same(map.resolve(key), Some(*expected));
        assert!(map.contains_key(key));
    }
    for phase in 0..4 {
        assert!(!map.contains_key(&id(phase, 999, 0)));
    }
}
fn insert(
    map: &mut PhysicalAdviceMap,
    oracle: &mut HashMap<ContextCell, Cell>,
    key: ContextCell,
    physical: Cell,
) {
    same(map.insert(key, physical), oracle.insert(key, physical));
}

#[test]
fn coordinate_runs_map_every_cell_across_contexts_phases_and_overlap_boundaries() {
    let cols = columns();
    let mut map = PhysicalAdviceMap::default();
    let mut oracle = HashMap::new();
    let mut original_boundary = Vec::new();
    for phase in 0..3 {
        let mut physical_column = 0;
        let mut row = 0;
        for (context, count) in [(0, 97), (7, 0), (13, 55), (29, 83)] {
            for offset in 0..count {
                let key = id(phase, context, offset);
                let cell = Cell {
                    column: cols[physical_column % 3],
                    row_offset: row,
                };
                insert(&mut map, &mut oracle, key, cell);
                if row == 62 {
                    // Base assigns the overlap again at the next column's row zero,
                    // but stores the original first physical cell for this virtual identity.
                    original_boundary.push((key, cell));
                    physical_column += 1;
                    row = 0;
                }
                row += 1;
            }
        }
    }
    check(&map, &oracle);
    assert_eq!(map.len(), 3 * (97 + 55 + 83));
    assert_eq!(map.context_count(), 9);
    assert!(map.run_count() <= 3 * (3 + 4));
    for (key, expected) in original_boundary {
        same(map.resolve(&key), Some(expected));
    }
    let runs = map.run_count();
    for (&key, &cell) in &oracle {
        same(map.insert(key, cell), Some(cell));
    }
    assert_eq!(map.run_count(), runs);
}

#[test]
fn coordinate_runs_preserve_gaps_out_of_order_overwrite_and_coalescing() {
    let cols = columns();
    let mut map = PhysicalAdviceMap::default();
    let mut oracle = HashMap::new();
    for offset in [9, 0, 5, 2, 8, 1, 7, 4, 6, 3] {
        insert(
            &mut map,
            &mut oracle,
            id(0, 0, offset),
            Cell {
                column: cols[0],
                row_offset: offset + 21,
            },
        );
        check(&map, &oracle);
        for hole in 0..10 {
            same(
                map.resolve(&id(0, 0, hole)),
                oracle.get(&id(0, 0, hole)).copied(),
            );
        }
    }
    assert_eq!(map.run_count(), 1);
    for offset in [0, 9, 5, 1, 8, 4] {
        insert(
            &mut map,
            &mut oracle,
            id(0, 0, offset),
            Cell {
                column: cols[3],
                row_offset: 50 - offset,
            },
        );
        check(&map, &oracle);
    }
    for offset in [4, 8, 1, 5, 9, 0] {
        insert(
            &mut map,
            &mut oracle,
            id(0, 0, offset),
            Cell {
                column: cols[0],
                row_offset: offset + 21,
            },
        );
        check(&map, &oracle);
    }
    assert_eq!(map.run_count(), 1);
}

#[test]
fn coordinate_runs_match_original_hashmap_for_deterministic_irregular_replay() {
    let cols = columns();
    let mut map = PhysicalAdviceMap::default();
    let mut oracle = HashMap::new();
    // Fixed arithmetic selects operations, not production randomness or map iteration.
    let mut state = 0x1327_598b_u64;
    for step in 0..12_000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let phase = (state >> 8) as usize % 4;
        let context = (state >> 15) as usize % 7;
        let offset = (state >> 21) as usize % 137;
        let key = id(phase, context, offset);
        let cell = Cell {
            column: cols[(state >> 34) as usize % cols.len()],
            row_offset: (state >> 40) as usize % 257,
        };
        insert(&mut map, &mut oracle, key, cell);
        if step % 311 == 0 {
            check(&map, &oracle);
        }
    }
    check(&map, &oracle);
    assert!(
        map.run_count() > map.len() / 2,
        "fixture exercises explicit irregular runs"
    );
}

#[test]
fn coordinate_runs_reset_clone_and_overwrite_errors_match_original_map() {
    let cols = columns();
    let key = id(2, 17, 6);
    let initial = Cell {
        column: cols[2],
        row_offset: 37,
    };
    let conflicting = Cell {
        column: cols[1],
        row_offset: 91,
    };
    let mut map = PhysicalAdviceMap::default();
    let mut oracle = HashMap::new();
    insert(&mut map, &mut oracle, key, initial);
    let cloned = map.clone();
    let reject = |old: Option<Cell>, new: Cell| {
        if let Some(old) = old {
            assert_eq!(coordinate(old), coordinate(new), "inconsistent replay");
        }
    };
    let run_error = catch_unwind(AssertUnwindSafe(|| {
        reject(map.insert(key, conflicting), conflicting)
    }));
    let old_error = catch_unwind(AssertUnwindSafe(|| {
        reject(oracle.insert(key, conflicting), conflicting)
    }));
    assert!(run_error.is_err() && old_error.is_err());
    // Like the old map, insertion happens before the assignment caller rejects it.
    check(&map, &oracle);
    same(cloned.resolve(&key), Some(initial));
    map.clear();
    oracle.clear();
    check(&map, &oracle);
    assert_eq!(map.context_count(), 0);
    assert_eq!(map.run_count(), 0);
    assert_eq!(map.checked_run_capacity_bytes(), Some(0));
    assert!(map.resolve(&key).is_none());
    insert(&mut map, &mut oracle, key, initial);
    check(&map, &oracle);
}

#[test]
fn coordinate_runs_preserve_maximum_offsets_rows_and_column_kind() {
    let cols = columns();
    let mut map = PhysicalAdviceMap::default();
    let mut oracle = HashMap::new();
    let maximum = u32::MAX as usize;
    for (offset, column, row) in [
        (maximum - 2, cols[0], usize::MAX - 2),
        (maximum - 1, cols[0], usize::MAX - 1),
        (maximum, cols[0], usize::MAX),
        (0, cols[0], usize::MAX),
        (1, cols[0], 0),
        (2, cols[3], 1),
        (3, cols[4], 2),
    ] {
        insert(
            &mut map,
            &mut oracle,
            id(3, (1 << 29) - 1, offset),
            Cell {
                column,
                row_offset: row,
            },
        );
    }
    check(&map, &oracle);
    assert_eq!(
        map.run_count(),
        5,
        "overflow and different column kinds cannot merge"
    );
    insert(
        &mut map,
        &mut oracle,
        id(3, (1 << 29) - 1, maximum - 1),
        Cell {
            column: cols[1],
            row_offset: 4,
        },
    );
    check(&map, &oracle);
    insert(
        &mut map,
        &mut oracle,
        id(3, (1 << 29) - 1, maximum - 1),
        Cell {
            column: cols[0],
            row_offset: usize::MAX - 1,
        },
    );
    check(&map, &oracle);
    assert_eq!(map.run_count(), 5);
}

#[test]
fn coordinate_runs_preserve_cross_context_lookup_and_ordered_copy_endpoints() {
    let cols = columns();
    let mut map = PhysicalAdviceMap::default();
    let mut oracle = HashMap::new();
    for phase in 0..4 {
        for context in [0, 7, 13] {
            for offset in 0..70 {
                insert(
                    &mut map,
                    &mut oracle,
                    id(phase, context, offset),
                    Cell {
                        column: cols[phase % 3],
                        row_offset: context * 71 + offset,
                    },
                );
            }
        }
    }
    let mut equalities = vec![
        (id(2, 7, 64), id(0, 13, 19)),
        (id(0, 13, 0), id(1, 7, 69)),
        (id(3, 0, 31), id(0, 0, 63)),
        (id(0, 13, 0), id(1, 7, 69)),
    ];
    equalities.sort_unstable(); // Same virtual ordering, including duplicate edges.
    let old_edges = equalities
        .iter()
        .map(|(a, b)| (coordinate(oracle[a]), coordinate(oracle[b])))
        .collect::<Vec<_>>();
    let run_edges = equalities
        .iter()
        .map(|(a, b)| {
            (
                coordinate(map.resolve(a).unwrap()),
                coordinate(map.resolve(b).unwrap()),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(run_edges, old_edges);
    // A lookup queue deliberately crosses contexts and revisits a cell; its order stays external.
    let lookups = [
        id(2, 13, 61),
        id(0, 0, 0),
        id(3, 7, 32),
        id(2, 13, 61),
        id(1, 7, 5),
    ];
    assert_eq!(
        lookups.map(|key| coordinate(map.resolve(&key).unwrap())),
        lookups.map(|key| coordinate(oracle[&key]))
    );
    check(&map, &oracle);
}

#[cfg(all(feature = "halo2-axiom", feature = "test-utils"))]
#[path = "physical_advice_proof_tests.rs"]
mod proof_tests;

#[cfg(target_pointer_width = "64")]
fn historical_geometry_benchmark<const REFERENCE: bool>() {
    use std::{hint::black_box, time::Instant};
    const CELLS: usize = 5_302_980;
    const MAX_ROWS: usize = 65_530;
    let mut meta = ConstraintSystem::<Fr>::default();
    let cols = (0..81)
        .map(|_| meta.advice_column().into())
        .collect::<Vec<Column<Any>>>();
    let mut runs = PhysicalAdviceMap::default();
    let mut reference = HashMap::<ContextCell, Cell>::new();
    let start = Instant::now();
    let mut column = 0;
    let mut row = 0;
    for offset in 0..CELLS {
        let key = id(0, 0, offset);
        let cell = Cell {
            column: cols[column],
            row_offset: row,
        };
        let previous = if REFERENCE {
            reference.insert(key, cell)
        } else {
            runs.insert(key, cell)
        };
        assert!(previous.is_none());
        if row == MAX_ROWS - 1 {
            column += 1;
            row = 0;
        }
        row += 1;
    }
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
    for offset in 0..CELLS {
        let key = id(0, 0, offset);
        let cell = if REFERENCE {
            reference[&key]
        } else {
            runs.resolve(&key).unwrap()
        };
        for word in [
            offset as u64,
            cell.column.index() as u64,
            cell.row_offset as u64,
        ] {
            for byte in word.to_le_bytes() {
                checksum = (checksum ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    assert_eq!(column + 1, 81);
    if REFERENCE {
        assert_eq!(reference.len(), CELLS);
    } else {
        assert_eq!(runs.len(), CELLS);
        assert_eq!(runs.run_count(), 81);
    }
    println!("BASE_MAP_BENCH reference={REFERENCE} cells={CELLS} contexts=1 columns=81 runs={} run_capacity_bytes={} hashmap_capacity={} elapsed_ms={} checksum={checksum:016x}",
        runs.run_count(), runs.checked_run_capacity_bytes().unwrap(), reference.capacity(), start.elapsed().as_millis());
    black_box((&runs, &reference));
}

#[cfg(target_pointer_width = "64")]
#[test]
#[ignore = "separate-process historical coordinate-storage benchmark; not a full Claim proof or RSS qualification"]
fn coordinate_map_benchmark_reference_historical_phase19_cells() {
    historical_geometry_benchmark::<true>();
}

#[cfg(target_pointer_width = "64")]
#[test]
#[ignore = "separate-process historical coordinate-storage benchmark; not a full Claim proof or RSS qualification"]
fn coordinate_map_benchmark_runs_historical_phase19_cells() {
    historical_geometry_benchmark::<false>();
}
