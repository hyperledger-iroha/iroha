//! Exact column order for compact hash and SMT trace openings.
//!
//! These mappings preserve every field cell. In particular, off-subgroup LDE
//! evaluations are full Goldilocks elements even where base trace rows contain
//! u32 limbs or bits. Decoding checks field canonicality, never narrows to u32,
//! and establishes no polynomial or statement validity by itself.

use super::{
    compact_blake2b_air::{COLUMN_COUNT as HASH_COLUMNS, CompactRow},
    compact_smt_air::{COLUMN_COUNT as SMT_COLUMNS, SmtRow},
};
use crate::{Error, Result};

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;

/// Encode the fixed 310-cell hash schema without dropping or reducing any cell.
#[must_use]
pub fn hash_row_cells<F: Copy>(row: &CompactRow<F>) -> [F; HASH_COLUMNS] {
    core::array::from_fn(|column| match column {
        0..32 => row.working[column],
        32..64 => row.message[column - 32],
        64..80 => row.chaining[column - 64],
        80..272 => row.bits[(column - 80) / 64][(column - 80) % 64],
        272..276 => row.carries[column - 272],
        276..300 => row.present[column - 276],
        300 => row.byte_len,
        301 => row.prefix_count,
        302..310 => row.digest[column - 302],
        _ => unreachable!("fixed hash schema width"),
    })
}

/// Reconstruct the exact hash schema from a fixed array of arbitrary field cells.
///
/// This generic mapping intentionally performs no range or validity check;
/// callers decoding untrusted base-field openings use [`decode_hash_row`].
#[must_use]
pub fn hash_row_from_cells<F: Copy>(cells: &[F; HASH_COLUMNS]) -> CompactRow<F> {
    CompactRow {
        working: core::array::from_fn(|i| cells[i]),
        message: core::array::from_fn(|i| cells[32 + i]),
        chaining: core::array::from_fn(|i| cells[64 + i]),
        bits: core::array::from_fn(|slot| core::array::from_fn(|i| cells[80 + 64 * slot + i])),
        carries: core::array::from_fn(|i| cells[272 + i]),
        present: core::array::from_fn(|i| cells[276 + i]),
        byte_len: cells[300],
        prefix_count: cells[301],
        digest: core::array::from_fn(|i| cells[302 + i]),
    }
}

/// Encode all 342 hash and full-width SMT state cells in fixed schema order.
#[must_use]
pub fn smt_row_cells<F: Copy>(row: &SmtRow<F>) -> [F; SMT_COLUMNS] {
    let hash = hash_row_cells(&row.hash);
    core::array::from_fn(|column| match column {
        0..310 => hash[column],
        310..318 => row.old_child[column - 310],
        318..326 => row.new_child[column - 318],
        326..334 => row.sibling[column - 326],
        334..342 => row.starting_root[column - 334],
        _ => unreachable!("fixed SMT schema width"),
    })
}

/// Reconstruct hash state and all four SMT ports without narrowing field values.
///
/// This generic mapping does not validate cells; untrusted base-field slices
/// must enter through [`decode_smt_row`].
#[must_use]
pub fn smt_row_from_cells<F: Copy>(cells: &[F; SMT_COLUMNS]) -> SmtRow<F> {
    SmtRow {
        hash: hash_row_from_cells(&core::array::from_fn(|i| cells[i])),
        old_child: core::array::from_fn(|i| cells[310 + i]),
        new_child: core::array::from_fn(|i| cells[318 + i]),
        sibling: core::array::from_fn(|i| cells[326 + i]),
        starting_root: core::array::from_fn(|i| cells[334 + i]),
    }
}

/// Decode a canonical base-field hash opening of exactly the fixed width.
pub fn decode_hash_row(cells: &[u64]) -> Result<CompactRow> {
    let cells = canonical_cells::<HASH_COLUMNS>(cells, "compact_hash_opening")?;
    Ok(hash_row_from_cells(cells))
}

/// Decode a canonical base-field SMT opening of exactly the fixed width.
pub fn decode_smt_row(cells: &[u64]) -> Result<SmtRow> {
    let cells = canonical_cells::<SMT_COLUMNS>(cells, "compact_smt_opening")?;
    Ok(smt_row_from_cells(cells))
}

fn canonical_cells<'a, const WIDTH: usize>(
    cells: &'a [u64],
    context: &'static str,
) -> Result<&'a [u64; WIDTH]> {
    let cells: &[u64; WIDTH] = cells.try_into().map_err(|_| Error::InvalidTraceShape {
        details: format!("{context} needs exactly {WIDTH} cells"),
    })?;
    for (column, &value) in cells.iter().enumerate() {
        if value >= GOLDILOCKS_MODULUS {
            return Err(Error::NonCanonicalGoldilocksElement {
                context,
                indices: vec![column],
            });
        }
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GoldilocksFp4V1;

    #[test]
    fn column_order_pins_every_hash_and_smt_field_boundary() {
        let cells = core::array::from_fn(|i| i as u64);
        let row = smt_row_from_cells(&cells);
        assert_eq!(smt_row_cells(&row), cells);
        assert_eq!(HASH_COLUMNS, 310);
        assert_eq!(SMT_COLUMNS, 342);
        assert_eq!((row.hash.working[0], row.hash.working[31]), (0, 31));
        assert_eq!((row.hash.message[0], row.hash.message[31]), (32, 63));
        assert_eq!((row.hash.chaining[0], row.hash.chaining[15]), (64, 79));
        for slot in 0..3 {
            assert_eq!(row.hash.bits[slot][0], 80 + 64 * slot as u64);
            assert_eq!(row.hash.bits[slot][63], 143 + 64 * slot as u64);
        }
        assert_eq!(row.hash.carries, [272, 273, 274, 275]);
        assert_eq!((row.hash.present[0], row.hash.present[23]), (276, 299));
        assert_eq!((row.hash.byte_len, row.hash.prefix_count), (300, 301));
        assert_eq!((row.hash.digest[0], row.hash.digest[7]), (302, 309));
        for (port, offset) in [
            (&row.old_child, 310),
            (&row.new_child, 318),
            (&row.sibling, 326),
            (&row.starting_root, 334),
        ] {
            for (i, &cell) in port.iter().enumerate() {
                assert_eq!(cell, offset + i as u64);
            }
        }
        let hash_cells = core::array::from_fn(|i| cells[i]);
        assert_eq!(hash_row_cells(&row.hash), hash_cells);
        assert_eq!(hash_row_from_cells(&hash_cells), row.hash);
    }

    #[test]
    fn canonical_decoding_preserves_full_field_lde_values() {
        let cells =
            core::array::from_fn::<_, SMT_COLUMNS, _>(|i| GOLDILOCKS_MODULUS - 1 - i as u64);
        assert_eq!(smt_row_cells(&decode_smt_row(&cells).unwrap()), cells);
        assert_eq!(
            hash_row_cells(&decode_hash_row(&cells[..HASH_COLUMNS]).unwrap()),
            cells[..HASH_COLUMNS]
        );
        for length in [
            0,
            HASH_COLUMNS - 1,
            HASH_COLUMNS + 1,
            SMT_COLUMNS - 1,
            SMT_COLUMNS + 1,
        ] {
            let malformed = vec![0; length];
            assert!(decode_hash_row(&malformed).is_err());
            assert!(decode_smt_row(&malformed).is_err());
        }
    }

    #[test]
    fn every_noncanonical_column_is_rejected_with_its_exact_position() {
        for column in 0..SMT_COLUMNS {
            for bad in [GOLDILOCKS_MODULUS, u64::MAX] {
                let mut cells = [0; SMT_COLUMNS];
                cells[column] = bad;
                assert!(matches!(decode_smt_row(&cells),
                    Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [column]));
                if column < HASH_COLUMNS {
                    assert!(matches!(decode_hash_row(&cells[..HASH_COLUMNS]),
                        Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [column]));
                }
            }
        }
    }

    #[test]
    fn generic_mapping_preserves_all_four_extension_coordinates() {
        let cells = core::array::from_fn(|i| {
            GoldilocksFp4V1::new([
                i as u64,
                i as u64 + 1,
                i as u64 + 2,
                GOLDILOCKS_MODULUS - 1 - i as u64,
            ])
            .unwrap()
        });
        let row = smt_row_from_cells(&cells);
        assert_eq!(smt_row_cells(&row), cells);
        let hash_cells = core::array::from_fn(|i| cells[i]);
        assert_eq!(
            hash_row_cells(&hash_row_from_cells(&hash_cells)),
            hash_cells
        );
    }
}
