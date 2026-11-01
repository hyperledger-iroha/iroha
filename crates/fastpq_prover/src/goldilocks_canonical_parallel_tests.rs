//! Production canonical input scans at the real parallel threshold.

use super::*;
use zeroize::{Zeroize as _, Zeroizing};

/// Bound these two large regression fixtures to one simultaneous allocation set.
pub(crate) static LARGE_SCAN_FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn fixture_word(index: usize) -> u64 {
    [0, 1, FIELD_MODULUS - 1, 37][index % 4]
}

fn assert_production_scan(columns: &[&[u64]], expected: bool) {
    let words = columns.iter().map(|column| column.len()).sum();
    let scalar = columns
        .iter()
        .flat_map(|column| column.iter())
        .all(|&word| word < FIELD_MODULUS);
    assert_eq!(scalar, expected);
    assert_eq!(canonical_word_columns_v1(columns, words), scalar);
}

#[test]
fn production_input_parallel_scan_matches_scalar_at_real_threshold_without_mutation() {
    let _fixture_guard = LARGE_SCAN_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1, 1 << 24);
    let threshold = CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1;
    let rows = threshold / 2;
    let tile = CANONICAL_SCAN_TILE_WORDS_V1;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    pool.install(|| {
        assert_eq!(rayon::current_num_threads(), 2);
        assert!(!parallel_canonical_scan_v1(threshold - 1));
        assert!(parallel_canonical_scan_v1(threshold));
        // One 128 MiB fixture; no copied oracle matrix. Drop clears on assertion unwind too.
        let mut columns = Zeroizing::new([Vec::new(), Vec::new()]);
        for (column, values) in columns.iter_mut().enumerate() {
            values.try_reserve_exact(rows).unwrap();
            assert_eq!(values.capacity(), rows);
            values.extend((0..rows).map(|row| fixture_word(column * rows + row)));
        }
        let identities = columns
            .each_ref()
            .map(|column| (column.as_ptr(), column.len(), column.capacity()));
        assert_production_scan(&[&columns[0], &columns[1][..rows - 1]], true);
        assert_production_scan(&[&columns[0], &columns[1]], true);
        // One invalid residue at each side of a tile/column boundary and at the full tail.
        for (column, row, invalid) in [
            (0, 0, FIELD_MODULUS),
            (0, tile - 1, u64::MAX),
            (0, tile, FIELD_MODULUS),
            (0, rows - 1, u64::MAX),
            (1, 0, FIELD_MODULUS),
            (1, rows - 1, u64::MAX),
        ] {
            let original = columns[column][row];
            columns[column][row] = invalid;
            assert_production_scan(&[&columns[0], &columns[1]], false);
            assert_eq!(columns[column][row], invalid);
            assert_eq!(
                columns
                    .each_ref()
                    .map(|column| (column.as_ptr(), column.len(), column.capacity())),
                identities
            );
            columns[column][row] = original;
        }
        // The just-below-threshold path scans its real slice extent, including its tail.
        columns[1][rows - 1] = u64::MAX;
        assert_production_scan(&[&columns[0], &columns[1][..rows - 1]], true);
        columns[1][rows - 2] = FIELD_MODULUS;
        assert_production_scan(&[&columns[0], &columns[1][..rows - 1]], false);
        columns[1][rows - 2] = fixture_word(threshold - 2);
        columns[1][rows - 1] = fixture_word(threshold - 1);
        for (column, values) in columns.iter().enumerate() {
            assert!(
                values
                    .iter()
                    .enumerate()
                    .all(|(row, &value)| value == fixture_word(column * rows + row))
            );
        }
        assert_eq!(
            columns
                .each_ref()
                .map(|column| (column.as_ptr(), column.len(), column.capacity())),
            identities
        );
        for values in columns.iter_mut() {
            values.as_mut_slice().zeroize();
            assert!(values.iter().all(|&value| value == 0));
        }
    });
}
