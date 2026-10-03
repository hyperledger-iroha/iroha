//! Original-source interval decoding with sparse searches and bounded bitmap rank windows.

use super::*;

fn label<F: PrimeField>(target: u32, rows: usize, omega: F) -> F {
    let target = target as usize;
    F::DELTA.pow_vartime([(target / rows) as u64]) * omega.pow_vartime([(target % rows) as u64])
}

fn sparse_pair<R: Read + Seek>(
    reader: &mut R,
    record: &PermutationRecord,
    at: usize,
    rows: usize,
    column: usize,
    cells: usize,
    frame: u64,
) -> io::Result<(usize, u32)> {
    if at >= record.exceptions as usize {
        return Err(invalid("indexed sparse pair is invalid"));
    }
    let offset = subrange(record.targets, (at as u64) * 8, 8, frame)?;
    seek(reader, offset)?;
    let mut pair = [0; 8];
    reader.read_exact(&mut pair)?;
    let row = u32::from_le_bytes(pair[..4].try_into().unwrap()) as usize;
    let target = u32::from_le_bytes(pair[4..].try_into().unwrap());
    if row >= rows || target as usize >= cells || target == self_target(rows, column, row)? {
        return Err(invalid("invalid indexed sparse exception"));
    }
    Ok((row, target))
}

impl<C: SerdeCurveAffine> IndexedStructuredProvingKeyV1<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    pub(super) fn copy_permutation_interval<R: Read + Seek>(
        &self,
        reader: &mut R,
        column: usize,
        start: usize,
        output: &mut [C::Scalar],
    ) -> io::Result<()> {
        let n = self.metadata.rows;
        let record = self
            .metadata
            .permutations
            .get(column)
            .ok_or_else(|| invalid("indexed permutation column is invalid"))?;
        let cells = n
            .checked_mul(self.metadata.permutation_columns)
            .filter(|cells| u32::try_from(*cells).is_ok())
            .ok_or_else(|| invalid("indexed permutation shape overflow"))?;
        let end = start
            .checked_add(output.len())
            .filter(|end| *end <= n)
            .ok_or_else(|| invalid("indexed permutation range is invalid"))?;
        if start == end {
            return Ok(());
        }
        let omega = self.vk.domain.get_omega();
        if record.mode != Mode::Dense {
            let mut value =
                C::Scalar::DELTA.pow_vartime([column as u64]) * omega.pow_vartime([start as u64]);
            for destination in output.iter_mut() {
                *destination = value;
                value *= omega;
            }
        }
        match record.mode {
            Mode::Identity => {}
            Mode::Dense => {
                let at = subrange(
                    record.targets,
                    (start as u64) * 4,
                    (output.len() as u64) * 4,
                    self.frame_bytes(),
                )?;
                seek(reader, at)?;
                let mut previous: Option<(usize, C::Scalar)> = None;
                for destination in output.iter_mut() {
                    let target = read_u32(reader)?;
                    if target as usize >= cells {
                        return Err(invalid("indexed permutation target is invalid"));
                    }
                    let id = target as usize;
                    *destination = match previous {
                        Some((prior, value))
                            if prior.checked_add(1) == Some(id) && prior / n == id / n =>
                        {
                            value * omega
                        }
                        _ => label(target, n, omega),
                    };
                    previous = Some((id, *destination));
                }
            }
            Mode::Sparse => {
                // Binary search exact pairs in the original reader, never a retained target map.
                // Each encountered row must also respect the search's strict ordering bounds.
                let (mut lo, mut hi) = (0, record.exceptions as usize);
                let (mut lower, mut upper) = (None, None);
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    let (row, _) =
                        sparse_pair(reader, record, mid, n, column, cells, self.frame_bytes())?;
                    if lower.is_some_and(|lower| row <= lower)
                        || upper.is_some_and(|upper| row >= upper)
                    {
                        return Err(invalid("unordered indexed sparse search"));
                    }
                    if row < start {
                        lo = mid + 1;
                        lower = Some(row);
                    } else {
                        hi = mid;
                        upper = Some(row);
                    }
                }
                let mut previous = if lo > 0 {
                    let (row, _) =
                        sparse_pair(reader, record, lo - 1, n, column, cells, self.frame_bytes())?;
                    if row >= start {
                        return Err(invalid("indexed sparse predecessor exceeds interval start"));
                    }
                    Some(row)
                } else {
                    None
                };
                for at in lo..record.exceptions as usize {
                    let (row, target) =
                        sparse_pair(reader, record, at, n, column, cells, self.frame_bytes())?;
                    if row < start || previous.is_some_and(|previous| row <= previous) {
                        return Err(invalid("unordered indexed sparse interval"));
                    }
                    if row >= end {
                        break;
                    }
                    output[row - start] = label(target, n, omega);
                    previous = Some(row);
                }
            }
            Mode::Bitmap => {
                // Re-read complete scanner-ranked windows, at most 512 bytes on the stack.
                // A changed window popcount cannot borrow a rank from the original bitmap.
                for chunk in start / RANK_ROWS..end.div_ceil(RANK_ROWS) {
                    let first_row = chunk * RANK_ROWS;
                    let last_row = (first_row + RANK_ROWS).min(n);
                    let bytes = (last_row - first_row).div_ceil(8);
                    let mut bitmap = [0_u8; RANK_ROWS / 8];
                    let at = subrange(
                        record.bitmap,
                        (first_row / 8) as u64,
                        bytes as u64,
                        self.frame_bytes(),
                    )?;
                    seek(reader, at)?;
                    reader.read_exact(&mut bitmap[..bytes])?;
                    if last_row == n && n % 8 != 0 && bitmap[bytes - 1] >> (n % 8) != 0 {
                        return Err(invalid("nonzero indexed permutation bitmap padding"));
                    }
                    let before = *record
                        .ranks
                        .get(chunk)
                        .ok_or_else(|| invalid("missing indexed bitmap rank"))?;
                    let after = *record
                        .ranks
                        .get(chunk + 1)
                        .ok_or_else(|| invalid("missing indexed bitmap rank end"))?;
                    let population: u32 = bitmap[..bytes].iter().map(|b| b.count_ones()).sum();
                    if before.checked_add(population) != Some(after) || after > record.exceptions {
                        return Err(invalid(
                            "indexed permutation bitmap rank disagrees with original",
                        ));
                    }
                    let from = start.max(first_row);
                    let to = end.min(last_row);
                    let mut rank = before;
                    for row in first_row..from {
                        rank += u32::from(bitmap[(row - first_row) / 8] >> (row % 8) & 1 != 0);
                    }
                    let count: u32 = (from..to)
                        .map(|row| u32::from(bitmap[(row - first_row) / 8] >> (row % 8) & 1 != 0))
                        .sum();
                    if count != 0 {
                        let at = subrange(
                            record.targets,
                            u64::from(rank) * 4,
                            u64::from(count) * 4,
                            self.frame_bytes(),
                        )?;
                        seek(reader, at)?;
                        for row in from..to {
                            if bitmap[(row - first_row) / 8] >> (row % 8) & 1 != 0 {
                                let target = read_u32(reader)?;
                                if target as usize >= cells
                                    || target == self_target(n, column, row)?
                                {
                                    return Err(invalid("invalid indexed bitmap exception target"));
                                }
                                output[row - start] = label(target, n, omega);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
