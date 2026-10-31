//! Tiled native RFC base construction into the existing retained allocation.

use super::*;

const WIDTH: usize = ZK_X509_RFC5280_STARK_BASE_WIDTH_V1;
const TILE: usize = 16;

struct OutputV1<'a, 'b> {
    columns: &'a mut [&'b mut [F]],
    committed: bool,
}
impl Drop for OutputV1<'_, '_> {
    fn drop(&mut self) {
        if !self.committed {
            for column in self.columns.iter_mut() {
                zeroize_fields_v1(column);
            }
        }
    }
}

pub(super) const fn scratch_payload_bytes_v1() -> usize {
    core::mem::size_of::<[[F; WIDTH]; TILE]>()
        + core::mem::size_of::<ZkX509Rfc5280StarkBaseRowV1>()
        + core::mem::size_of::<ZkX509Rfc5280StarkFixedRowV1>()
        + core::mem::size_of::<OutputV1<'static, 'static>>()
}

pub(super) fn fill_base_v1(
    material: &ZkX509Rfc5280StarkBaseMaterialV1,
    columns: &mut [&mut [F]],
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    fill_with_v1(ZK_X509_RFC5280_STARK_TRACE_SIZE_V1, columns, |row| {
        material.base_row(row)
    })
}

fn fill_with_v1(
    rows: usize,
    columns: &mut [&mut [F]],
    mut row_at: impl FnMut(usize) -> Result<ZkX509Rfc5280StarkBaseRowV1, ZkX509Rfc5280StarkErrorV1>,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if rows == 0
        || rows > ZK_X509_RFC5280_STARK_TRACE_SIZE_V1
        || columns.len() != WIDTH
        || columns.iter().any(|column| column.len() != rows)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let mut output = OutputV1 {
        columns,
        committed: false,
    };
    let mut tile = zeroize::Zeroizing::new([[F::ZERO; WIDTH]; TILE]);
    for first in (0..rows).step_by(TILE) {
        let count = TILE.min(rows - first);
        for (offset, target) in tile.iter_mut().take(count).enumerate() {
            let row = zeroize::Zeroizing::new(row_at(first + offset)?);
            if row.iter().any(|value| F::canonical(value.0).is_none()) {
                return Err(ZkX509Rfc5280StarkErrorV1::Shape);
            }
            target.copy_from_slice(&*row);
        }
        for (column, target) in output.columns.iter_mut().enumerate() {
            for (offset, row) in tile.iter().take(count).enumerate() {
                target[first + offset] = row[column];
            }
        }
    }
    output.committed = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_base_matrix_keeps_all_rows_columns_and_partial_tile() {
        let rows = 19;
        let mut columns = vec![vec![F::ZERO; rows]; WIDTH];
        let mut visited = Vec::new();
        fill_with_v1(
            rows,
            &mut columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>(),
            |row| {
                visited.push(row);
                Ok(core::array::from_fn(|column| {
                    F((row * WIDTH + column) as u64)
                }))
            },
        )
        .unwrap();
        assert_eq!(visited, (0..rows).collect::<Vec<_>>());
        for (column, values) in columns.iter().enumerate() {
            for (row, value) in values.iter().enumerate() {
                assert_eq!(*value, F((row * WIDTH + column) as u64));
            }
        }
    }
    #[test]
    fn complete_base_matrix_refuses_bad_shapes_and_clears_late_errors_or_unwind() {
        for mode in 0..3 {
            let mut columns = vec![vec![F(77); 19]; WIDTH];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fill_with_v1(
                    19,
                    &mut columns
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>(),
                    |row| {
                        if row == 17 {
                            match mode {
                                0 => return Err(ZkX509Rfc5280StarkErrorV1::Shape),
                                1 => panic!("base row source unwind"),
                                _ => {
                                    let mut row = [F::ONE; WIDTH];
                                    row[WIDTH - 1] = F(u64::MAX);
                                    return Ok(row);
                                }
                            }
                        }
                        Ok([F::ONE; WIDTH])
                    },
                )
            }));
            assert!(result.map_or(mode == 1, |outcome| outcome.is_err()));
            assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
        }
        let mut columns = vec![vec![F(77); 19]; WIDTH - 1];
        assert!(
            fill_with_v1(
                19,
                &mut columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>(),
                |_| panic!("invalid shape read source")
            )
            .is_err()
        );
        assert!(columns.iter().flatten().all(|value| *value == F(77)));
    }
}
