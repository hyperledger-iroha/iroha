// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! # Sparse Matrices
//!
//! This module defines a custom implementation of CSR/CSC sparse matrices.
//! Specifically, we implement sparse matrix / dense vector multiplication
//! to compute the `A z`, `B z`, and `C z` in Spartan.
use ff::PrimeField;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::errors::VegaError;

/// Threshold below which we use sequential (non-rayon) iteration.
const PARALLEL_THRESHOLD: usize = 4096;

/// Precomputed SpMV accelerator for a fixed sparse matrix.
///
/// Classifies entries by coefficient magnitude to avoid expensive field
/// multiplications for the common cases in R1CS:
/// - +/-1: just add/subtract (no multiplication)
/// - small +/-k (|k| <= 7): repeated addition (cheaper than field mul)
/// - general: full field multiplication
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecomputedSparseMatrix<F: PrimeField> {
    pub(crate) num_rows: usize,
    pub(crate) num_cols: usize,
    // Cumulative offsets into entry arrays (length = num_rows + 1).
    // Row i's entries span offsets[i]..offsets[i+1].
    // Uses u32 to halve memory vs usize (max 4B entries).
    pub(crate) off_unit_pos: Vec<u32>,
    pub(crate) off_unit_neg: Vec<u32>,
    pub(crate) off_small: Vec<u32>,
    pub(crate) off_general: Vec<u32>,
    pub(crate) unit_pos_cols: Vec<u32>,
    pub(crate) unit_neg_cols: Vec<u32>,
    pub(crate) small_cols: Vec<u32>,
    pub(crate) small_coeffs: Vec<i8>,
    pub(crate) general_cols: Vec<u32>,
    pub(crate) general_vals: Vec<F>,
}

impl<F: PrimeField> PrecomputedSparseMatrix<F> {
    /// Build from a CSR SparseMatrix by classifying entries.
    pub fn from_sparse(m: &SparseMatrix<F>) -> Self {
        let num_rows = m.indptr.len() - 1;
        let one = F::ONE;
        let neg_one = -F::ONE;

        let small_pos: Vec<F> = (2u64..=7).map(F::from).collect();
        let small_neg: Vec<F> = (2u64..=7).map(|k| -F::from(k)).collect();

        let mut off_unit_pos = Vec::with_capacity(num_rows + 1);
        let mut off_unit_neg = Vec::with_capacity(num_rows + 1);
        let mut off_small = Vec::with_capacity(num_rows + 1);
        let mut off_general = Vec::with_capacity(num_rows + 1);
        let mut unit_pos_cols: Vec<u32> = Vec::new();
        let mut unit_neg_cols: Vec<u32> = Vec::new();
        let mut small_cols: Vec<u32> = Vec::new();
        let mut small_coeffs: Vec<i8> = Vec::new();
        let mut general_cols: Vec<u32> = Vec::new();
        let mut general_vals = Vec::new();

        for ptrs in m.indptr.windows(2) {
            off_unit_pos.push(unit_pos_cols.len() as u32);
            off_unit_neg.push(unit_neg_cols.len() as u32);
            off_small.push(small_cols.len() as u32);
            off_general.push(general_cols.len() as u32);

            for (&val, &col) in m.data[ptrs[0]..ptrs[1]]
                .iter()
                .zip(&m.indices[ptrs[0]..ptrs[1]])
            {
                if val == one {
                    unit_pos_cols.push(col as u32);
                } else if val == neg_one {
                    unit_neg_cols.push(col as u32);
                } else if let Some(k) = small_pos.iter().position(|&v| v == val) {
                    small_cols.push(col as u32);
                    small_coeffs.push((k as i8) + 2);
                } else if let Some(k) = small_neg.iter().position(|&v| v == val) {
                    small_cols.push(col as u32);
                    small_coeffs.push(-((k as i8) + 2));
                } else {
                    general_cols.push(col as u32);
                    general_vals.push(val);
                }
            }
        }
        // Sentinel for last row's end
        debug_assert!(
            unit_pos_cols.len() <= u32::MAX as usize,
            "unit_pos_cols exceeds u32 range"
        );
        debug_assert!(
            unit_neg_cols.len() <= u32::MAX as usize,
            "unit_neg_cols exceeds u32 range"
        );
        debug_assert!(
            small_cols.len() <= u32::MAX as usize,
            "small_cols exceeds u32 range"
        );
        debug_assert!(
            general_cols.len() <= u32::MAX as usize,
            "general_cols exceeds u32 range"
        );
        debug_assert!(
            m.cols <= u32::MAX as usize,
            "column count exceeds u32 range"
        );
        off_unit_pos.push(unit_pos_cols.len() as u32);
        off_unit_neg.push(unit_neg_cols.len() as u32);
        off_small.push(small_cols.len() as u32);
        off_general.push(general_cols.len() as u32);

        Self {
            num_rows,
            num_cols: m.cols,
            off_unit_pos,
            off_unit_neg,
            off_small,
            off_general,
            unit_pos_cols,
            unit_neg_cols,
            small_cols,
            small_coeffs,
            general_cols,
            general_vals,
        }
    }

    #[inline(always)]
    pub(crate) fn small_mul(coeff: i8, x: F) -> F {
        let abs = coeff.unsigned_abs();
        let result = match abs {
            2 => x.double(),
            3 => x.double() + x,
            4 => x.double().double(),
            5 => x.double().double() + x,
            6 => {
                let d = x.double();
                d.double() + d
            }
            7 => {
                let d = x.double();
                d.double() + d + x
            }
            _ => unreachable!(),
        };
        if coeff < 0 { -result } else { result }
    }

    /// Get the range of unit_pos entries for a given row.
    #[inline(always)]
    pub(crate) fn range_unit_pos(&self, row: usize) -> (usize, usize) {
        (
            self.off_unit_pos[row] as usize,
            self.off_unit_pos[row + 1] as usize,
        )
    }

    /// Get the range of unit_neg entries for a given row.
    #[inline(always)]
    pub(crate) fn range_unit_neg(&self, row: usize) -> (usize, usize) {
        (
            self.off_unit_neg[row] as usize,
            self.off_unit_neg[row + 1] as usize,
        )
    }

    /// Get the range of small entries for a given row.
    #[inline(always)]
    pub(crate) fn range_small(&self, row: usize) -> (usize, usize) {
        (
            self.off_small[row] as usize,
            self.off_small[row + 1] as usize,
        )
    }

    /// Get the range of general entries for a given row.
    #[inline(always)]
    pub(crate) fn range_general(&self, row: usize) -> (usize, usize) {
        (
            self.off_general[row] as usize,
            self.off_general[row + 1] as usize,
        )
    }

    #[inline(always)]
    fn compute_row_single(&self, row: usize, v: &[F]) -> F {
        let mut sum = F::ZERO;

        let (start, end) = self.range_unit_pos(row);
        for i in start..end {
            sum += v[self.unit_pos_cols[i] as usize];
        }

        let (start, end) = self.range_unit_neg(row);
        for i in start..end {
            sum -= v[self.unit_neg_cols[i] as usize];
        }

        let (start, end) = self.range_small(row);
        for i in start..end {
            sum += Self::small_mul(self.small_coeffs[i], v[self.small_cols[i] as usize]);
        }

        let (start, end) = self.range_general(row);
        for i in start..end {
            sum += self.general_vals[i] * v[self.general_cols[i] as usize];
        }

        sum
    }

    /// Fast SpMV using precomputed coefficient classification.
    pub fn multiply_vec(&self, vector: &[F]) -> Vec<F> {
        assert_eq!(self.num_cols, vector.len(), "invalid shape");
        if self.num_rows <= PARALLEL_THRESHOLD || rayon::current_num_threads() <= 1 {
            (0..self.num_rows)
                .map(|r| self.compute_row_single(r, vector))
                .collect()
        } else {
            (0..self.num_rows)
                .into_par_iter()
                .map(|r| self.compute_row_single(r, vector))
                .collect()
        }
    }

    /// Batched SpMV: multiply the same matrix by multiple vectors in a single pass.
    /// Uses sub-batches of BATCH_SIZE to keep working set in cache.
    pub fn multiply_vec_batched(&self, vectors: &[&[F]]) -> Vec<Vec<F>> {
        let n_vecs = vectors.len();
        if n_vecs == 0 {
            return vec![];
        }
        for v in vectors {
            assert_eq!(self.num_cols, v.len(), "invalid shape");
        }

        // For small numbers of vectors or small matrices, just call single-vector multiply
        const BATCH_SIZE: usize = 4;
        if n_vecs <= 1 || self.num_rows <= 256 {
            return vectors.iter().map(|v| self.multiply_vec(v)).collect();
        }

        let mut results: Vec<Vec<F>> = (0..n_vecs).map(|_| vec![F::ZERO; self.num_rows]).collect();

        // Process vectors in sub-batches to keep working set in L3 cache
        for batch_start in (0..n_vecs).step_by(BATCH_SIZE) {
            let batch_end = (batch_start + BATCH_SIZE).min(n_vecs);
            let batch_len = batch_end - batch_start;

            #[allow(clippy::needless_range_loop)]
            for row in 0..self.num_rows {
                // unit_pos: coefficient = +1
                let (start, end) = self.range_unit_pos(row);
                for idx in start..end {
                    let col = self.unit_pos_cols[idx] as usize;
                    for b in 0..batch_len {
                        results[batch_start + b][row] += vectors[batch_start + b][col];
                    }
                }

                // unit_neg: coefficient = -1
                let (start, end) = self.range_unit_neg(row);
                for idx in start..end {
                    let col = self.unit_neg_cols[idx] as usize;
                    for b in 0..batch_len {
                        results[batch_start + b][row] -= vectors[batch_start + b][col];
                    }
                }

                // small coefficients
                let (start, end) = self.range_small(row);
                for idx in start..end {
                    let col = self.small_cols[idx] as usize;
                    let coeff = self.small_coeffs[idx];
                    for b in 0..batch_len {
                        results[batch_start + b][row] +=
                            Self::small_mul(coeff, vectors[batch_start + b][col]);
                    }
                }

                // general: full field multiply
                let (start, end) = self.range_general(row);
                for idx in start..end {
                    let col = self.general_cols[idx] as usize;
                    let val = self.general_vals[idx];
                    for b in 0..batch_len {
                        results[batch_start + b][row] += val * vectors[batch_start + b][col];
                    }
                }
            }
        }

        results
    }

    /// Build a FilteredSpmv containing only entries with col >= col_min.
    pub fn build_filtered(&self, col_min: usize, num_rows_used: usize) -> FilteredSpmv<F> {
        let num_rows = num_rows_used.min(self.num_rows);
        let one = F::ONE;
        let neg_one = -F::ONE;

        let mut rows = Vec::new();
        let mut cols = Vec::new();
        let mut vals = Vec::new();

        for row in 0..num_rows {
            let (start, end) = self.range_unit_pos(row);
            for i in start..end {
                let col = self.unit_pos_cols[i] as usize;
                if col >= col_min {
                    rows.push(row as u32);
                    cols.push(col as u32);
                    vals.push(one);
                }
            }

            let (start, end) = self.range_unit_neg(row);
            for i in start..end {
                let col = self.unit_neg_cols[i] as usize;
                if col >= col_min {
                    rows.push(row as u32);
                    cols.push(col as u32);
                    vals.push(neg_one);
                }
            }

            let (start, end) = self.range_small(row);
            for i in start..end {
                let col = self.small_cols[i] as usize;
                if col >= col_min {
                    let coeff = self.small_coeffs[i];
                    let val = Self::small_mul(coeff, one);
                    rows.push(row as u32);
                    cols.push(col as u32);
                    vals.push(val);
                }
            }

            let (start, end) = self.range_general(row);
            for i in start..end {
                let col = self.general_cols[i] as usize;
                if col >= col_min {
                    rows.push(row as u32);
                    cols.push(col as u32);
                    vals.push(self.general_vals[i]);
                }
            }
        }

        FilteredSpmv { rows, cols, vals }
    }
}

/// Compact representation of matrix entries filtered by column threshold.
/// Stores only entries with col >= col_min, avoiding iteration over the
/// full matrix during incremental SpMV.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilteredSpmv<F: PrimeField> {
    pub(crate) rows: Vec<u32>,
    pub(crate) cols: Vec<u32>,
    pub(crate) vals: Vec<F>,
}

impl<F: PrimeField> FilteredSpmv<F> {
    /// Accumulate filtered entries into output: out[row] += val * vector[col]
    #[inline]
    pub fn multiply_vec_add(&self, vector: &[F], out: &mut [F]) {
        for i in 0..self.rows.len() {
            out[self.rows[i] as usize] += self.vals[i] * vector[self.cols[i] as usize];
        }
    }
}

/// CSR format sparse matrix, We follow the names used by scipy.
/// Detailed explanation here: https://stackoverflow.com/questions/52299420/scipy-csr-matrix-understand-indptr
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseMatrix<F: PrimeField> {
    /// all non-zero values in the matrix
    pub data: Vec<F>,
    /// column indices
    pub indices: Vec<usize>,
    /// row information
    pub indptr: Vec<usize>,
    /// number of columns
    pub cols: usize,
}

impl<F: PrimeField> SparseMatrix<F> {
    /// Write raw bytes for digest computation (much faster than bincode Serialize).
    pub fn write_digest_bytes<W: std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        // Write lengths and cols as fixed-size le bytes
        w.write_all(&(self.data.len() as u64).to_le_bytes())?;
        w.write_all(&(self.indices.len() as u64).to_le_bytes())?;
        w.write_all(&(self.indptr.len() as u64).to_le_bytes())?;
        w.write_all(&(self.cols as u64).to_le_bytes())?;
        // Write field elements as raw repr bytes
        for d in &self.data {
            w.write_all(d.to_repr().as_ref())?;
        }
        // Write indices as raw bytes
        for idx in &self.indices {
            w.write_all(&(*idx as u64).to_le_bytes())?;
        }
        // Write indptr as raw bytes
        for ptr in &self.indptr {
            w.write_all(&(*ptr as u64).to_le_bytes())?;
        }
        Ok(())
    }

    /// 0x0 empty matrix
    pub fn empty() -> Self {
        SparseMatrix {
            data: vec![],
            indices: vec![],
            indptr: vec![0],
            cols: 0,
        }
    }

    /// Construct from the COO representation; Vec<usize(row), usize(col), F>.
    /// We assume that the rows are sorted during construction.
    #[cfg(test)]
    pub fn new(matrix: &[(usize, usize, F)], rows: usize, cols: usize) -> Self {
        let mut new_matrix = vec![vec![]; rows];
        for (row, col, val) in matrix {
            new_matrix[*row].push((*col, *val));
        }

        for row in new_matrix.iter() {
            assert!(row.windows(2).all(|w| w[0].0 < w[1].0));
        }

        let mut indptr = vec![0; rows + 1];
        for (i, col) in new_matrix.iter().enumerate() {
            indptr[i + 1] = indptr[i] + col.len();
        }

        let mut indices = vec![];
        let mut data = vec![];
        for col in new_matrix {
            let (idx, val): (Vec<_>, Vec<_>) = col.into_iter().unzip();
            indices.extend(idx);
            data.extend(val);
        }

        SparseMatrix {
            data,
            indices,
            indptr,
            cols,
        }
    }

    /// Retrieves the data for row slice [i..j] from `ptrs`.
    /// We assume that `ptrs` is indexed from `indptrs` and do not check if the
    /// returned slice is actually a valid row.
    pub fn get_row_unchecked(&self, ptrs: &[usize; 2]) -> impl Iterator<Item = (&F, &usize)> {
        self.data[ptrs[0]..ptrs[1]]
            .iter()
            .zip(&self.indices[ptrs[0]..ptrs[1]])
    }

    /// Multiply by a dense vector; uses rayon/gpu.
    ///
    /// # Errors
    /// Returns `VegaError::InvalidInputLength` if the vector length doesn't match the matrix dimensions.
    pub fn multiply_vec(&self, vector: &[F]) -> Result<Vec<F>, VegaError> {
        if self.cols != vector.len() {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SparseMatrix multiply_vec: Expected {} elements in vector, got {}",
                    self.cols,
                    vector.len()
                ),
            });
        }

        Ok(self.multiply_vec_unchecked(vector))
    }

    /// Multiply by a dense vector; uses rayon/gpu.
    /// This does not check that the shape of the matrix/vector are compatible.
    pub fn multiply_vec_unchecked(&self, vector: &[F]) -> Vec<F> {
        let num_rows = self.indptr.len() - 1;
        if num_rows <= PARALLEL_THRESHOLD {
            self.indptr
                .windows(2)
                .map(|ptrs| {
                    let row_ptrs = [ptrs[0], ptrs[1]];
                    self.get_row_unchecked(&row_ptrs)
                        .map(|(val, col_idx)| *val * vector[*col_idx])
                        .sum()
                })
                .collect()
        } else {
            self.indptr
                .par_windows(2)
                .map(|ptrs| {
                    let row_ptrs = [ptrs[0], ptrs[1]];
                    self.get_row_unchecked(&row_ptrs)
                        .map(|(val, col_idx)| *val * vector[*col_idx])
                        .sum()
                })
                .collect()
        }
    }

    /// returns a custom iterator
    pub fn iter(&self) -> Iter<'_, F> {
        let mut row = 0;
        while row + 1 < self.indptr.len() && self.indptr[row + 1] == 0 {
            row += 1;
        }
        let nnz = if self.indptr.is_empty() {
            0
        } else {
            self.indptr[self.indptr.len() - 1]
        };
        Iter {
            matrix: self,
            row,
            i: 0,
            nnz,
        }
    }
}

/// Iterator for sparse matrix
pub struct Iter<'a, F: PrimeField> {
    matrix: &'a SparseMatrix<F>,
    row: usize,
    i: usize,
    nnz: usize,
}

impl<'a, F: PrimeField> Iterator for Iter<'a, F> {
    type Item = (usize, usize, F);

    fn next(&mut self) -> Option<Self::Item> {
        // are we at the end?
        if self.i == self.nnz {
            return None;
        }

        // compute current item
        let curr_item = (
            self.row,
            self.matrix.indices[self.i],
            self.matrix.data[self.i],
        );

        // advance the iterator
        self.i += 1;
        // edge case at the end
        if self.i == self.nnz {
            return Some(curr_item);
        }
        // if `i` has moved to next row
        while self.i >= self.matrix.indptr[self.row + 1] {
            self.row += 1;
        }

        Some(curr_item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        provider::PallasHyraxEngine,
        traits::{Engine, Group},
    };
    use ff::PrimeField;
    use proptest::{
        prelude::*,
        strategy::{BoxedStrategy, Just, Strategy},
    };

    type G = <PallasHyraxEngine as Engine>::GE;
    type Fr = <G as Group>::Scalar;

    /// Wrapper struct around a field element that implements additional traits
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FWrap<F: PrimeField>(pub F);

    impl<F: PrimeField> Copy for FWrap<F> {}

    #[cfg(not(target_arch = "wasm32"))]
    /// Trait implementation for generating `FWrap<F>` instances with proptest
    impl<F: PrimeField> Arbitrary for FWrap<F> {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
            use rand::rngs::StdRng;
            use rand_core::SeedableRng;

            let strategy = any::<[u8; 32]>()
                .prop_map(|seed| FWrap(F::random(StdRng::from_seed(seed))))
                .no_shrink();
            strategy.boxed()
        }
    }

    #[test]
    fn test_matrix_creation() {
        let matrix_data = vec![
            (0, 1, Fr::from(2)),
            (1, 2, Fr::from(3)),
            (2, 0, Fr::from(4)),
        ];
        let sparse_matrix = SparseMatrix::<Fr>::new(&matrix_data, 3, 3);

        assert_eq!(
            sparse_matrix.data,
            vec![Fr::from(2), Fr::from(3), Fr::from(4)]
        );
        assert_eq!(sparse_matrix.indices, vec![1, 2, 0]);
        assert_eq!(sparse_matrix.indptr, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_matrix_vector_multiplication() {
        let matrix_data = vec![
            (0, 1, Fr::from(2)),
            (0, 2, Fr::from(7)),
            (1, 2, Fr::from(3)),
            (2, 0, Fr::from(4)),
        ];
        let sparse_matrix = SparseMatrix::<Fr>::new(&matrix_data, 3, 3);
        let vector = vec![Fr::from(1), Fr::from(2), Fr::from(3)];

        let result = sparse_matrix.multiply_vec(&vector);

        assert_eq!(
            result.unwrap(),
            vec![Fr::from(25), Fr::from(9), Fr::from(4)]
        );
    }

    fn coo_strategy() -> BoxedStrategy<Vec<(usize, usize, FWrap<Fr>)>> {
        let coo_strategy =
            any::<FWrap<Fr>>().prop_flat_map(|f| (0usize..100, 0usize..100, Just(f)));
        proptest::collection::vec(coo_strategy, 10).boxed()
    }

    proptest! {
        #[test]
        fn test_matrix_iter(mut coo_matrix in coo_strategy()) {
          // process the randomly generated coo matrix
          coo_matrix.sort_by_key(|(row, col, _val)| (*row, *col));
          coo_matrix.dedup_by_key(|(row, col, _val)| (*row, *col));
          let coo_matrix = coo_matrix.into_iter().map(|(row, col, val)| { (row, col, val.0) }).collect::<Vec<_>>();

          let matrix = SparseMatrix::new(&coo_matrix, 100, 100);

          prop_assert_eq!(coo_matrix, matrix.iter().collect::<Vec<_>>());
      }
    }
}
