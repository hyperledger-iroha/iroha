//! Exact directed permutation-column serialization and the sole acceptance scan.

use super::*;
use permutation_column_codec::{
    PermutationColumnEncoding, PermutationColumnMode as Mode, canonical_permutation_column_encoding,
};

/// Source rows per scanner-derived bitmap rank interval.
pub(super) const RANK_ROWS: usize = 4096;

#[derive(Debug)]
pub(super) struct PermutationRecord {
    pub(super) mode: Mode,
    pub(super) exceptions: u32,
    pub(super) payload: CheckedRange,
    pub(super) targets: CheckedRange,
    pub(super) bitmap: CheckedRange,
    // Cumulative exception count at each 4096-row boundary, including the final boundary.
    // Capacity is charged separately from the enclosing record Vec and original VK/domain.
    pub(super) ranks: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ValidatedPermutationColumn {
    pub(super) encoding: PermutationColumnEncoding,
    pub(super) exceptions: u32,
}

pub(super) fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

pub(super) fn self_target(rows: usize, column: usize, row: usize) -> io::Result<u32> {
    column
        .checked_mul(rows)
        .and_then(|v| v.checked_add(row))
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| invalid("permutation identity target overflows u32"))
}

pub(super) fn validate_permutation_columns<F: PrimeField>(
    permutations: &[Polynomial<F, LagrangeCoeff>],
    index: &InverseIndex<F>,
    cells: usize,
) -> io::Result<(Vec<ValidatedPermutationColumn>, u64)> {
    let mut seen = Seen::new(cells)?;
    let mut records = reserved(permutations.len())?;
    let mut bytes = 0_u64;
    for (column, values) in permutations.iter().enumerate() {
        let mut exceptions = 0_u32;
        for (row, value) in values.iter().enumerate() {
            let target = index.target(*value)?;
            // Every directed target participates, including every implied identity.
            seen.mark(target)?;
            exceptions += u32::from(target != self_target(index.rows, column, row)?);
        }
        let encoding = canonical_permutation_column_encoding(index.rows as u64, exceptions as u64)?;
        bytes = bytes
            .checked_add(encoding.payload_bytes)
            .ok_or_else(|| invalid("permutation-column total size overflow"))?;
        records.push(ValidatedPermutationColumn {
            encoding,
            exceptions,
        });
    }
    Ok((records, bytes))
}

pub(super) fn write_permutation_column<F: PrimeField, W: Write>(
    writer: &mut W,
    values: &[F],
    column: usize,
    index: &InverseIndex<F>,
    validated: ValidatedPermutationColumn,
) -> io::Result<()> {
    writer.write_all(&[validated.encoding.mode as u8])?;
    match validated.encoding.mode {
        Mode::Identity => {}
        Mode::Sparse => {
            writer.write_all(&validated.exceptions.to_le_bytes())?;
            for (row, value) in values.iter().enumerate() {
                let target = index.target(*value)?;
                if target != self_target(index.rows, column, row)? {
                    writer.write_all(&(row as u32).to_le_bytes())?;
                    writer.write_all(&target.to_le_bytes())?;
                }
            }
        }
        Mode::Bitmap => {
            for (chunk, values) in values.chunks(8).enumerate() {
                let mut bits = 0;
                for (bit, value) in values.iter().enumerate() {
                    let row = chunk * 8 + bit;
                    bits |=
                        u8::from(index.target(*value)? != self_target(index.rows, column, row)?)
                            << bit;
                }
                writer.write_all(&[bits])?;
            }
            for (row, value) in values.iter().enumerate() {
                let target = index.target(*value)?;
                if target != self_target(index.rows, column, row)? {
                    writer.write_all(&target.to_le_bytes())?;
                }
            }
        }
        Mode::Dense => {
            for value in values {
                writer.write_all(&index.target(*value)?.to_le_bytes())?;
            }
        }
    }
    Ok(())
}

pub(super) fn scan_permutation_column<F, R, W, V>(
    frame: &mut io::Take<R>,
    frame_bytes: u64,
    rows: usize,
    column: usize,
    output: &mut W,
    seen: &mut Seen,
    values: &mut V,
) -> io::Result<PermutationRecord>
where
    F: PrimeField,
    R: Read,
    W: Write,
    V: StructuredValues<F>,
{
    let mut tag = [0];
    frame.read_exact(&mut tag)?;
    let mode = match tag[0] {
        0 => Mode::Identity,
        1 => Mode::Sparse,
        2 => Mode::Bitmap,
        3 => Mode::Dense,
        _ => return Err(invalid("unknown permutation-column mode")),
    };
    output.write_all(&tag)?;
    let offset = frame_position(frame, frame_bytes)?;
    let mut record = PermutationRecord {
        mode,
        exceptions: 0,
        payload: CheckedRange::new(offset, 0, frame_bytes)?,
        targets: CheckedRange::new(offset, 0, frame_bytes)?,
        bitmap: CheckedRange::new(offset, 0, frame_bytes)?,
        ranks: Vec::new(),
    };
    values.begin_permutation(rows)?;
    let mut accept = |row: usize, target: u32| -> io::Result<()> {
        seen.mark(target)?;
        values.permutation_target(target, rows)?;
        record.exceptions += u32::from(target != self_target(rows, column, row)?);
        Ok(())
    };
    match mode {
        Mode::Identity => {
            for row in 0..rows {
                accept(row, self_target(rows, column, row)?)?;
            }
        }
        Mode::Sparse => {
            let count = read_u32(frame)?;
            let encoding = canonical_permutation_column_encoding(rows as u64, count as u64)?;
            if encoding.mode != mode {
                return Err(invalid("nonminimal permutation-column mode"));
            }
            output.write_all(&count.to_le_bytes())?;
            record.targets = CheckedRange::new(offset + 4, u64::from(count) * 8, frame_bytes)?;
            let mut next_row = 0;
            for _ in 0..count {
                let row = read_u32(frame)? as usize;
                let target = read_u32(frame)?;
                if row >= rows || row < next_row || target == self_target(rows, column, row)? {
                    return Err(invalid("unordered, out-of-range or self sparse exception"));
                }
                while next_row < row {
                    accept(next_row, self_target(rows, column, next_row)?)?;
                    next_row += 1;
                }
                accept(row, target)?;
                output.write_all(&(row as u32).to_le_bytes())?;
                output.write_all(&target.to_le_bytes())?;
                next_row = row + 1;
            }
            while next_row < rows {
                accept(next_row, self_target(rows, column, next_row)?)?;
                next_row += 1;
            }
        }
        Mode::Bitmap => {
            // A streaming Read supplies the complete bitmap before its targets. Retain only
            // this one column's n/8 bitmap until the targets are consumed, then drop it.
            let bitmap_bytes = rows.div_ceil(8);
            let mut bitmap = reserved(bitmap_bytes)?;
            bitmap.resize(bitmap_bytes, 0);
            frame.read_exact(&mut bitmap)?;
            if rows % 8 != 0 && bitmap[bitmap_bytes - 1] >> (rows % 8) != 0 {
                return Err(invalid("nonzero permutation bitmap padding"));
            }
            let count: u32 = bitmap.iter().map(|b| b.count_ones()).sum();
            let encoding = canonical_permutation_column_encoding(rows as u64, count as u64)?;
            if encoding.mode != mode {
                return Err(invalid("nonminimal permutation-column mode"));
            }
            output.write_all(&bitmap)?;
            record.bitmap = CheckedRange::new(offset, bitmap_bytes as u64, frame_bytes)?;
            record.targets = CheckedRange::new(
                offset + bitmap_bytes as u64,
                u64::from(count) * 4,
                frame_bytes,
            )?;
            record.ranks = reserved(rows.div_ceil(RANK_ROWS) + 1)?;
            let mut rank = 0;
            record.ranks.push(rank);
            for chunk in bitmap.chunks(RANK_ROWS / 8) {
                rank += chunk.iter().map(|b| b.count_ones()).sum::<u32>();
                record.ranks.push(rank);
            }
            for row in 0..rows {
                let target = if bitmap[row / 8] >> (row % 8) & 1 != 0 {
                    let target = read_u32(frame)?;
                    if target == self_target(rows, column, row)? {
                        return Err(invalid("self target in permutation bitmap exception"));
                    }
                    output.write_all(&target.to_le_bytes())?;
                    target
                } else {
                    self_target(rows, column, row)?
                };
                accept(row, target)?;
            }
        }
        Mode::Dense => {
            record.targets = CheckedRange::new(offset, (rows as u64) * 4, frame_bytes)?;
            for row in 0..rows {
                let target = read_u32(frame)?;
                accept(row, target)?;
                output.write_all(&target.to_le_bytes())?;
            }
        }
    }
    // The borrow also proves record.exceptions is the actual decoded non-self count.
    drop(accept);
    let encoding = canonical_permutation_column_encoding(rows as u64, record.exceptions as u64)?;
    if encoding.mode != mode {
        return Err(invalid("nonminimal permutation-column mode"));
    }
    record.payload = CheckedRange::new(offset, encoding.payload_bytes, frame_bytes)?;
    if record.payload.offset + record.payload.length != frame_position(frame, frame_bytes)? {
        return Err(invalid("permutation-column payload length mismatch"));
    }
    Ok(record)
}
