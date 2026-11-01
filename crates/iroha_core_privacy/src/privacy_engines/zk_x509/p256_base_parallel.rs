//! Independent P-256 base rows fill bounded disjoint tiles of the original columns.
//!
//! No recurrence is split and no private matrix is allocated. Every active tile
//! visits rows in canonical order. All tiles join before the first row error is
//! selected or the complete destination guard clears a failure or unwind.

use super::*;

const WIDTH: usize = crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1;
const TILE_ROWS: usize = 1 << 14;
const MAX_ROWS: usize = P256_ARITHMETIC_AGGREGATE_TRACE_SIZE_V1;
const TILES: usize = MAX_ROWS / TILE_ROWS;
const MIN_PARALLEL_ROWS: usize = 1 << 16;
type Tile<'a> = [&'a mut [F]; WIDTH];

const _: () = assert!(P256_VALUE_BUS_STARK_TRACE_SIZE_V1 == MAX_ROWS && TILES == 32);

/// Charge every possible tile's complete serial source state, row and error
/// workspace, independently of the worker count. The bounded view array contains
/// only borrows into already charged outputs and never reallocates their owners.
pub(super) const fn scratch_payload_v1(serial: usize) -> usize {
    TILES
        * (serial
            + core::mem::size_of::<Option<(usize, P256AggregateAdapterErrorV1)>>()
            + 8 * core::mem::size_of::<usize>()
            + core::mem::size_of::<P256AggregateAuxRowScratchV1<8>>())
        + core::mem::size_of::<[Tile<'static>; TILES]>()
        + core::mem::size_of::<P256AggregateColumnsDestinationGuardV1<'static, 'static>>()
}

pub(super) fn fill_columns_v1<const ROW_WIDTH: usize>(
    rows: usize,
    first: usize,
    outputs: &mut [&mut [F]],
    base_row: impl Fn(usize) -> Result<[F; ROW_WIDTH], P256AggregateAdapterErrorV1> + Sync,
) -> Result<(), P256AggregateAdapterErrorV1> {
    fill_with_v1(rows, first, outputs, prefer_tiles_v1(rows), base_row)
}

/// Let compact arithmetic fill the sole clearing row owner directly.
pub(super) fn fill_cells_v1<const ROW_WIDTH: usize>(
    rows: usize,
    first: usize,
    outputs: &mut [&mut [F]],
    fill_row: impl Fn(usize, &mut [F; ROW_WIDTH]) -> Result<(), P256AggregateAdapterErrorV1> + Sync,
) -> Result<(), P256AggregateAdapterErrorV1> {
    fill_cells_with_v1(rows, first, outputs, prefer_tiles_v1(rows), fill_row)
}

fn prefer_tiles_v1(rows: usize) -> bool {
    rows >= MIN_PARALLEL_ROWS && rayon::current_num_threads() > 1
}

fn fill_with_v1<const ROW_WIDTH: usize>(
    rows: usize,
    first: usize,
    outputs: &mut [&mut [F]],
    parallel: bool,
    base_row: impl Fn(usize) -> Result<[F; ROW_WIDTH], P256AggregateAdapterErrorV1> + Sync,
) -> Result<(), P256AggregateAdapterErrorV1> {
    fill_cells_with_v1(rows, first, outputs, parallel, |row, cells| {
        *cells = base_row(row)?;
        Ok(())
    })
}

fn fill_cells_with_v1<const ROW_WIDTH: usize>(
    rows: usize,
    first: usize,
    outputs: &mut [&mut [F]],
    parallel: bool,
    fill_row: impl Fn(usize, &mut [F; ROW_WIDTH]) -> Result<(), P256AggregateAdapterErrorV1> + Sync,
) -> Result<(), P256AggregateAdapterErrorV1> {
    if outputs.is_empty()
        || outputs.len() > WIDTH
        || first
            .checked_add(outputs.len())
            .is_none_or(|end| end > ROW_WIDTH)
        || !rows.is_power_of_two()
        || rows > MAX_ROWS
        || outputs.iter().any(|output| output.len() != rows)
    {
        return Err(P256AggregateAdapterErrorV1::Topology);
    }
    let count = outputs.len();
    let mut destination = P256AggregateColumnsDestinationGuardV1 {
        outputs,
        committed: false,
    };
    let write = |start: usize, targets: &mut [&mut [F]]| {
        for row in 0..targets[0].len() {
            let mut values = P256AggregateAuxRowScratchV1([F::ZERO; ROW_WIDTH]);
            fill_row(start + row, &mut values.0)?;
            for (offset, target) in targets.iter_mut().enumerate() {
                target[row] = values.0[first + offset];
            }
        }
        Ok::<_, P256AggregateAdapterErrorV1>(())
    };
    if !parallel || rows <= TILE_ROWS {
        write(0, destination.outputs)?;
    } else {
        let tile_count = rows.div_ceil(TILE_ROWS);
        let mut tiles: [Tile<'_>; TILES] =
            core::array::from_fn(|_| core::array::from_fn(|_| -> &mut [F] { &mut [] }));
        for (column, output) in destination.outputs.iter_mut().enumerate() {
            for (tile, cells) in tiles.iter_mut().zip(output.chunks_mut(TILE_ROWS)) {
                tile[column] = cells;
            }
        }
        // Do not cancel on another tile's failure. Each tile returns its first
        // canonical row error; this reduction picks the earliest public tile.
        // Rayon joins outstanding work before propagating any panic to the
        // enclosing guard, so no worker can race complete destination erasure.
        let failed = tiles[..tile_count]
            .par_iter_mut()
            .enumerate()
            .map(|(index, targets)| {
                write(index * TILE_ROWS, &mut targets[..count])
                    .err()
                    .map(|error| (index, error))
            })
            .reduce(
                || None,
                |left, right| match (left, right) {
                    (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
                    (left, right) => left.or(right),
                },
            );
        if let Some((_, error)) = failed {
            return Err(error);
        }
    }
    destination.committed = true;
    Ok(())
}

#[cfg(test)]
#[path = "p256_base_parallel_tests.rs"]
mod tests;
