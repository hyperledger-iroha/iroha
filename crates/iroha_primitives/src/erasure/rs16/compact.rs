//! Compact terminal stripes over the existing RS16 field and SIMD kernels.
//! No signed context, consensus resource cap, custody decision or protocol signature lives here.
//! The caller owns every job buffer. Existing process-wide field tables remain separate.
#[path = "compact/funded.rs"]
mod funded;
pub use funded::{
    CodecAllocationError, Encoded, Reconstructed, encode_funded, reconstruct_funded,
    required_backing_bytes,
};

use super::{Backend, Rs16Error, choose_backend, gf_inv, gf_mul, gf_pow, mul_add_row};
use std::ops::Range;

/// Checked byte geometry; one stripe has k data rows followed by m parity rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactShape {
    payload: usize,
    k: usize,
    m: usize,
    maximum_row: usize,
    stripes: usize,
    terminal_row: usize,
    chunks: usize,
    encoded: usize,
    workspace: usize,
}
impl CompactShape {
    /// Calculate exact geometry and caller-owned scratch without allocating.
    /// All final data padding is zero; the terminal row is 2*ceil(remaining/(2*k)).
    pub fn new(payload: usize, k: usize, m: usize, maximum_row: usize) -> Result<Self, Rs16Error> {
        let n = k.checked_add(m).ok_or(Rs16Error)?;
        if payload == 0
            || k == 0
            || m == 0
            || n > u16::MAX as usize
            || maximum_row < 2
            || !maximum_row.is_multiple_of(2)
        {
            return Err(Rs16Error);
        }
        let capacity = k.checked_mul(maximum_row).ok_or(Rs16Error)?;
        let stripes = payload.div_ceil(capacity);
        let remaining = payload - (stripes - 1) * capacity;
        let terminal_row = remaining.div_ceil(2 * k).checked_mul(2).ok_or(Rs16Error)?;
        let chunks = stripes.checked_mul(n).ok_or(Rs16Error)?;
        let encoded = (stripes - 1)
            .checked_mul(maximum_row)
            .and_then(|v| v.checked_add(terminal_row))
            .and_then(|v| v.checked_mul(n))
            .ok_or(Rs16Error)?;
        let active_row = if stripes == 1 {
            terminal_row
        } else {
            maximum_row
        };
        let workspace = k
            .checked_mul(k)
            .and_then(|v| v.checked_mul(2))
            .and_then(|v| v.checked_add(k.checked_mul(m)?))
            .and_then(|v| v.checked_add(k))
            .and_then(|v| v.checked_add(n.checked_add(k)?.checked_mul(active_row / 2)?))
            .ok_or(Rs16Error)?;
        Ok(Self {
            payload,
            k,
            m,
            maximum_row,
            stripes,
            terminal_row,
            chunks,
            encoded,
            workspace,
        })
    }
    /// Number of stripes in this exact compact codeword.
    pub fn stripe_count(self) -> usize {
        self.stripes
    }
    /// Exact row length of the terminal stripe.
    pub fn terminal_row_bytes(self) -> usize {
        self.terminal_row
    }
    /// Canonical original payload length.
    pub fn payload_bytes(self) -> usize {
        self.payload
    }
    /// Exact complete codeword bytes, excluding any wire/authentication metadata.
    pub fn encoded_bytes(self) -> usize {
        self.encoded
    }
    /// Number of ordered chunks.
    pub fn chunk_count(self) -> usize {
        self.chunks
    }
    /// Required caller-owned u16 workspace elements.
    pub fn workspace_words(self) -> usize {
        self.workspace
    }
    /// Exact byte range of one row in the complete ordered codeword.
    pub fn chunk_range(self, index: usize) -> Option<Range<usize>> {
        if index >= self.chunks {
            return None;
        }
        let n = self.k + self.m;
        let stripe = index / n;
        let row_bytes = self.row_bytes(stripe);
        let start = stripe * n * self.maximum_row + (index % n) * row_bytes;
        Some(start..start + row_bytes)
    }
    fn maximum_active_row(self) -> usize {
        if self.stripes == 1 {
            self.terminal_row
        } else {
            self.maximum_row
        }
    }
    fn row_bytes(self, stripe: usize) -> usize {
        if stripe + 1 == self.stripes {
            self.terminal_row
        } else {
            self.maximum_row
        }
    }
    /// Encode exact canonical bytes without allocating any per-job backing.
    pub fn encode_into(
        self,
        payload: &[u8],
        codeword: &mut [u8],
        workspace: &mut [u16],
    ) -> Result<(), Rs16Error> {
        self.encode_with_backend(payload, codeword, workspace, choose_backend())
    }
    fn encode_with_backend(
        self,
        payload: &[u8],
        codeword: &mut [u8],
        workspace: &mut [u16],
        backend: Backend,
    ) -> Result<(), Rs16Error> {
        if payload.len() != self.payload
            || codeword.len() != self.encoded
            || workspace.len() < self.workspace
        {
            return Err(Rs16Error);
        }
        let work = Work::new(self, workspace);
        prepare(self.k, self.m, work.matrix, work.inverse, work.parity)?;
        for stripe in 0..self.stripes {
            let row_bytes = self.row_bytes(stripe);
            let symbols = row_bytes / 2;
            let data = &mut work.rows[..self.k * symbols];
            data.fill(0);
            let offset = stripe * self.k * self.maximum_row;
            let remaining = (self.payload - offset).min(self.k * row_bytes);
            for (index, byte) in payload[offset..offset + remaining].iter().enumerate() {
                data[index / 2] |= u16::from(*byte) << (8 * (index % 2));
            }
            encode_rows(self.k, self.m, symbols, work.parity, work.rows, backend);
            write_rows(self, stripe, symbols, work.rows, codeword);
        }
        Ok(())
    }
    /// Recover from any k received rows per stripe and check every provided row,
    /// canonical zero padding, and every regenerated row through the caller's
    /// commitment predicate. This predicate must bind the original manifest;
    /// this codec does not authenticate a manifest or choose an authority.
    /// On error, destinations may be partly written and must not be treated as output.
    pub fn reconstruct_into(
        self,
        received: &[Option<&[u8]>],
        payload: &mut [u8],
        codeword: &mut [u8],
        workspace: &mut [u16],
        verify_original_row: impl FnMut(usize, &[u8]) -> bool,
    ) -> Result<(), Rs16Error> {
        self.reconstruct_with_backend(
            received,
            payload,
            codeword,
            workspace,
            verify_original_row,
            choose_backend(),
        )
    }
    fn reconstruct_with_backend(
        self,
        received: &[Option<&[u8]>],
        payload: &mut [u8],
        codeword: &mut [u8],
        workspace: &mut [u16],
        mut verify_original_row: impl FnMut(usize, &[u8]) -> bool,
        backend: Backend,
    ) -> Result<(), Rs16Error> {
        if received.len() != self.chunks
            || payload.len() != self.payload
            || codeword.len() != self.encoded
            || workspace.len() < self.workspace
        {
            return Err(Rs16Error);
        }
        let work = Work::new(self, workspace);
        prepare(self.k, self.m, work.matrix, work.inverse, work.parity)?;
        let n = self.k + self.m;
        for stripe in 0..self.stripes {
            let row_bytes = self.row_bytes(stripe);
            let symbols = row_bytes / 2;
            let mut selected = 0;
            for (index, row) in received[stripe * n..(stripe + 1) * n].iter().enumerate() {
                let Some(row) = row else {
                    continue;
                };
                if row.len() != row_bytes {
                    return Err(Rs16Error);
                }
                if selected < self.k {
                    work.indices[selected] = index as u16;
                    for (position, pair) in row.chunks_exact(2).enumerate() {
                        work.selected[selected * symbols + position] =
                            u16::from_le_bytes([pair[0], pair[1]]);
                    }
                    selected += 1;
                }
            }
            if selected != self.k {
                return Err(Rs16Error);
            }
            work.matrix.fill(0);
            for (row, index) in work.indices.iter().map(|v| usize::from(*v)).enumerate() {
                if index < self.k {
                    work.matrix[row * self.k + index] = 1;
                } else {
                    work.matrix[row * self.k..(row + 1) * self.k].copy_from_slice(
                        &work.parity[(index - self.k) * self.k..(index - self.k + 1) * self.k],
                    );
                }
            }
            invert(self.k, work.matrix, work.inverse)?;
            work.rows[..self.k * symbols].fill(0);
            for row in 0..self.k {
                for source in 0..self.k {
                    mul_add_row(
                        work.inverse[row * self.k + source],
                        &work.selected[source * symbols..(source + 1) * symbols],
                        &mut work.rows[row * symbols..(row + 1) * symbols],
                        backend,
                    );
                }
            }
            encode_rows(self.k, self.m, symbols, work.parity, work.rows, backend);
            write_rows(self, stripe, symbols, work.rows, codeword);
            for index in stripe * n..(stripe + 1) * n {
                let row = &codeword[self.chunk_range(index).ok_or(Rs16Error)?];
                if received[index].is_some_and(|received| received != row)
                    || !verify_original_row(index, row)
                {
                    return Err(Rs16Error);
                }
            }
            let offset = stripe * self.k * self.maximum_row;
            let remaining = (self.payload - offset).min(self.k * row_bytes);
            for position in 0..self.k * row_bytes {
                let byte = (work.rows[position / 2] >> (8 * (position % 2))) as u8;
                if position < remaining {
                    payload[offset + position] = byte;
                } else if byte != 0 {
                    return Err(Rs16Error);
                }
            }
        }
        Ok(())
    }
}
struct Work<'a> {
    matrix: &'a mut [u16],
    inverse: &'a mut [u16],
    parity: &'a mut [u16],
    indices: &'a mut [u16],
    selected: &'a mut [u16],
    rows: &'a mut [u16],
}
impl<'a> Work<'a> {
    fn new(shape: CompactShape, workspace: &'a mut [u16]) -> Self {
        let (matrix, rest) = workspace.split_at_mut(shape.k * shape.k);
        let (inverse, rest) = rest.split_at_mut(shape.k * shape.k);
        let (parity, rest) = rest.split_at_mut(shape.k * shape.m);
        let (indices, rest) = rest.split_at_mut(shape.k);
        let (selected, rows) = rest.split_at_mut(shape.k * (shape.maximum_active_row() / 2));
        Self {
            matrix,
            inverse,
            parity,
            indices,
            selected,
            rows,
        }
    }
}
fn invert(k: usize, matrix: &mut [u16], inverse: &mut [u16]) -> Result<(), Rs16Error> {
    inverse.fill(0);
    for row in 0..k {
        inverse[row * k + row] = 1;
    }
    for column in 0..k {
        let pivot = (column..k)
            .find(|row| matrix[row * k + column] != 0)
            .ok_or(Rs16Error)?;
        for index in 0..k {
            matrix.swap(column * k + index, pivot * k + index);
            inverse.swap(column * k + index, pivot * k + index);
        }
        let reciprocal = gf_inv(matrix[column * k + column]).ok_or(Rs16Error)?;
        for index in 0..k {
            matrix[column * k + index] = gf_mul(matrix[column * k + index], reciprocal);
            inverse[column * k + index] = gf_mul(inverse[column * k + index], reciprocal);
        }
        for row in 0..k {
            if row == column {
                continue;
            }
            let factor = matrix[row * k + column];
            for index in 0..k {
                matrix[row * k + index] ^= gf_mul(factor, matrix[column * k + index]);
                inverse[row * k + index] ^= gf_mul(factor, inverse[column * k + index]);
            }
        }
    }
    Ok(())
}
fn prepare(
    k: usize,
    m: usize,
    matrix: &mut [u16],
    inverse: &mut [u16],
    parity: &mut [u16],
) -> Result<(), Rs16Error> {
    for row in 0..k {
        for column in 0..k {
            matrix[row * k + column] = if row == 0 || column == 0 {
                1
            } else {
                gf_pow(row * column)
            };
        }
    }
    invert(k, matrix, inverse)?;
    for row in 0..m {
        for column in 0..k {
            let mut value = 0;
            for source in 0..k {
                let coefficient = if source == 0 {
                    1
                } else {
                    gf_pow((k + row) * source)
                };
                value ^= gf_mul(coefficient, inverse[source * k + column]);
            }
            parity[row * k + column] = value;
        }
    }
    Ok(())
}
fn encode_rows(
    k: usize,
    m: usize,
    symbols: usize,
    coefficients: &[u16],
    rows: &mut [u16],
    backend: Backend,
) {
    let (data, parity) = rows.split_at_mut(k * symbols);
    parity[..m * symbols].fill(0);
    for row in 0..m {
        for source in 0..k {
            mul_add_row(
                coefficients[row * k + source],
                &data[source * symbols..(source + 1) * symbols],
                &mut parity[row * symbols..(row + 1) * symbols],
                backend,
            );
        }
    }
}
fn write_rows(shape: CompactShape, stripe: usize, symbols: usize, rows: &[u16], output: &mut [u8]) {
    let n = shape.k + shape.m;
    for row in 0..n {
        let range = shape
            .chunk_range(stripe * n + row)
            .expect("checked ordered stripe");
        for (destination, symbol) in output[range]
            .chunks_exact_mut(2)
            .zip(&rows[row * symbols..(row + 1) * symbols])
        {
            destination.copy_from_slice(&symbol.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_bytes_match_existing_rs16_parity_for_each_stripe() {
        for (k, m) in [(1, 1), (3, 3), (4, 2), (16, 16)] {
            for length in [1, 7, 8, 9, 63, 64, 65, 255, 256, 257, 4093] {
                let shape = CompactShape::new(length, k, m, 64).unwrap();
                let payload = (0..length)
                    .map(|i| (i.wrapping_mul(17) % 251) as u8)
                    .collect::<Vec<_>>();
                let mut out = vec![0; shape.encoded_bytes()];
                let mut workspace = vec![0; shape.workspace_words()];
                shape
                    .encode_with_backend(&payload, &mut out, &mut workspace, Backend::Scalar)
                    .unwrap();
                for stripe in 0..shape.stripes {
                    let row_bytes = shape.row_bytes(stripe);
                    let data = (0..k)
                        .map(|row| {
                            let offset = (stripe * k * 64 + row * row_bytes).min(payload.len());
                            let len = (payload.len() - offset).min(row_bytes);
                            super::super::symbols_from_payload(row_bytes, &payload, offset, len)
                                .unwrap()
                        })
                        .collect::<Vec<_>>();
                    let parity =
                        super::super::encode_parity_with_backend(&data, m, Backend::Scalar)
                            .unwrap();
                    for (row, symbols) in data.iter().chain(&parity).enumerate() {
                        assert_eq!(
                            &out[shape.chunk_range(stripe * (k + m) + row).unwrap()],
                            super::super::chunk_from_symbols(symbols, row_bytes).unwrap()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn scalar_and_host_accelerated_boundaries_match_with_erasure_recovery() {
        let accelerated = choose_backend();
        eprintln!("compact host backend: {accelerated:?}");
        for (k, m) in [(1, 1), (4, 2), (16, 16)] {
            for length in [1, 7, 9, 63, 65, 255, 257, 4093, 23572] {
                let shape = CompactShape::new(length, k, m, 256 * 1024).unwrap();
                let original = (0..length).map(|i| (i % 251) as u8).collect::<Vec<_>>();
                let mut scalar = vec![0; shape.encoded_bytes()];
                let mut host = scalar.clone();
                let mut work = vec![0; shape.workspace_words()];
                shape
                    .encode_with_backend(&original, &mut scalar, &mut work, Backend::Scalar)
                    .unwrap();
                shape
                    .encode_with_backend(&original, &mut host, &mut work, accelerated)
                    .unwrap();
                assert_eq!(scalar, host);
                let rows = (0..shape.chunk_count())
                    .map(|index| {
                        if index % (k + m) < m.min(k) {
                            None
                        } else {
                            Some(&scalar[shape.chunk_range(index).unwrap()])
                        }
                    })
                    .collect::<Vec<_>>();
                let mut payload = vec![0; length];
                let mut reconstructed = vec![0; shape.encoded_bytes()];
                for backend in [Backend::Scalar, accelerated] {
                    shape
                        .reconstruct_with_backend(
                            &rows,
                            &mut payload,
                            &mut reconstructed,
                            &mut work,
                            |i, row| row == &scalar[shape.chunk_range(i).unwrap()],
                            backend,
                        )
                        .unwrap();
                    assert_eq!(payload, original);
                    assert_eq!(reconstructed, scalar);
                }
            }
        }
    }
    #[test]
    fn consistent_nonzero_terminal_padding_rejects_even_with_matching_rows() {
        let longer = CompactShape::new(16, 4, 2, 64).unwrap();
        let target = CompactShape::new(9, 4, 2, 64).unwrap();
        assert_eq!(longer.encoded_bytes(), target.encoded_bytes());
        let payload = vec![1; 16];
        let mut codeword = vec![0; longer.encoded_bytes()];
        let mut work = vec![0; longer.workspace_words()];
        longer
            .encode_into(&payload, &mut codeword, &mut work)
            .unwrap();
        let rows = (0..target.chunk_count())
            .map(|i| Some(&codeword[target.chunk_range(i).unwrap()]))
            .collect::<Vec<_>>();
        let mut recovered = vec![0; 9];
        let mut regenerated = vec![0; target.encoded_bytes()];
        assert!(
            target
                .reconstruct_into(
                    &rows,
                    &mut recovered,
                    &mut regenerated,
                    &mut work,
                    |i, row| row == &codeword[target.chunk_range(i).unwrap()]
                )
                .is_err()
        );
    }
}
