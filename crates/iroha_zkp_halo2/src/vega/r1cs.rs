//! Strict sparse relaxed-R1CS algebra for the Vega Neutron/Nova composition.
use super::{VegaT256ScalarV1 as Scalar, commitment::Commitment};
use std::collections::HashMap;
use thiserror::Error;
/// Failure while constructing or evaluating a Vega R1CS object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum R1csError {
    #[error("Vega R1CS dimensions do not match")]
    InvalidDimension,
    #[error("Vega sparse matrix storage exceeds the CSR index range")]
    CsrStorageOverflow,
    #[error("Vega sparse matrix storage allocation failed")]
    CsrStorageAllocation,
    #[error("Vega sparse matrix entries do not match the counted CSR profile")]
    CsrEntryCountMismatch,
    #[error("Vega sparse matrix entries are not canonical")]
    NonCanonicalMatrix,
    #[error("Vega R1CS assignment does not satisfy the relation")]
    Unsatisfied,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct SparseMatrix {
    rows: usize,
    columns: usize,
    row_offsets: Vec<u32>,
    column_indices: Vec<u32>,
    coefficient_ids: CoefficientIds,
    coefficient_dictionary: Vec<Scalar>,
}
/// Append-only CSR construction for a fixed number of canonical rows.
///
/// The builder retains the final CSR buffers plus a pre-sized dictionary
/// lookup that is discarded by [`Self::finish`]. Callers may supply one row at
/// a time; finishing pads any trailing rows with empty offsets.
pub(super) struct SparseMatrixRowBuilder {
    rows: usize,
    columns: usize,
    expected_nonzero_count: usize,
    expected_coefficient_count: usize,
    row_offsets: Vec<u32>,
    column_indices: Vec<u32>,
    coefficient_ids: CoefficientIds,
    coefficient_dictionary: Vec<Scalar>,
    coefficient_lookup: HashMap<[u8; 32], u32>,
}
/// Per-entry dictionary IDs at the narrowest width that represents exact `D`.
#[derive(Debug, PartialEq, Eq)]
enum CoefficientIds {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
}
/// Fallible unique-coefficient counter for one canonical matrix profile.
pub(super) struct CoefficientDictionaryCounter {
    coefficients: HashMap<[u8; 32], ()>,
}
struct FoldRowInputs<'a> {
    relaxed_witness: &'a [Scalar],
    strict_witness: &'a [Scalar],
    effective_relaxation: Scalar,
    relaxed_public_inputs: &'a [Scalar],
    strict_public_inputs: &'a [Scalar],
}
impl SparseMatrix {
    pub(super) fn new(
        rows: usize,
        columns: usize,
        entries: &[(usize, usize, Scalar)],
    ) -> Result<Self, R1csError> {
        validate_csr_dimensions(rows, columns, entries.len())?;
        let mut coefficient_counter = CoefficientDictionaryCounter::new();
        for (_, _, coefficient) in entries {
            coefficient_counter.observe(*coefficient)?;
        }
        let coefficient_count = coefficient_counter.len();
        drop(coefficient_counter);
        let mut builder =
            SparseMatrixRowBuilder::new(rows, columns, entries.len(), coefficient_count)?;
        let mut cursor = 0;
        for row in 0..rows {
            let row_start = cursor;
            while cursor < entries.len() && entries[cursor].0 == row {
                cursor += 1;
            }
            builder.append_canonical_row(
                entries[row_start..cursor]
                    .iter()
                    .map(|(_, column, coefficient)| (*column, *coefficient)),
            )?;
        }
        if cursor != entries.len() {
            return Err(R1csError::NonCanonicalMatrix);
        }
        builder.finish()
    }
    pub(super) fn rows(&self) -> usize {
        self.rows
    }
    pub(super) fn columns(&self) -> usize {
        self.columns
    }
    pub(super) fn nonzero_count(&self) -> usize {
        self.coefficient_ids.len()
    }
    pub(super) fn coefficient_count(&self) -> usize {
        self.coefficient_dictionary.len()
    }
    #[cfg(test)]
    pub(super) fn canonical_entries(&self) -> impl Iterator<Item = (usize, usize, Scalar)> + '_ {
        self.row_offsets
            .windows(2)
            .enumerate()
            .flat_map(move |(row, bounds)| {
                let start = usize::try_from(bounds[0]).expect("u32 CSR offset fits usize");
                let end = usize::try_from(bounds[1]).expect("u32 CSR offset fits usize");
                (start..end).map(move |index| {
                    (
                        row,
                        usize::try_from(self.column_indices[index])
                            .expect("u32 CSR column fits usize"),
                        self.coefficient(index),
                    )
                })
            })
    }
    pub(super) fn row_entries(
        &self,
        row: usize,
    ) -> Option<impl Iterator<Item = (usize, Scalar)> + '_> {
        let bounds = self.row_bounds(row)?;
        Some(bounds.map(move |index| {
            (
                usize::try_from(self.column_indices[index]).expect("u32 CSR column fits usize"),
                self.coefficient(index),
            )
        }))
    }
    #[cfg(test)]
    pub(super) fn multiply(&self, vector: &[Scalar]) -> Result<Vec<Scalar>, R1csError> {
        if vector.len() != self.columns {
            return Err(R1csError::InvalidDimension);
        }
        let mut output = vec![Scalar::zero(); self.rows];
        for (row, output) in output.iter_mut().enumerate() {
            for index in self.row_bounds(row).expect("bounded CSR row") {
                let column =
                    usize::try_from(self.column_indices[index]).expect("u32 CSR column fits usize");
                *output += self.coefficient(index) * vector[column];
            }
        }
        Ok(output)
    }
    #[cfg(test)]
    pub(super) fn bind_rows(&self, row_weights: &[Scalar]) -> Result<Vec<Scalar>, R1csError> {
        if row_weights.len() != self.rows {
            return Err(R1csError::InvalidDimension);
        }
        let mut output = vec![Scalar::zero(); self.columns];
        for (row, weight) in row_weights.iter().copied().enumerate() {
            for index in self.row_bounds(row).expect("bounded CSR row") {
                let column =
                    usize::try_from(self.column_indices[index]).expect("u32 CSR column fits usize");
                output[column] += weight * self.coefficient(index);
            }
        }
        Ok(output)
    }
    pub(super) fn evaluate(
        &self,
        row_weights: &[Scalar],
        column_weights: &[Scalar],
    ) -> Result<Scalar, R1csError> {
        if row_weights.len() < self.rows || column_weights.len() < self.columns {
            return Err(R1csError::InvalidDimension);
        }
        let mut result = Scalar::zero();
        for (row, row_weight) in row_weights.iter().copied().take(self.rows).enumerate() {
            for index in self.row_bounds(row).expect("bounded CSR row") {
                let column =
                    usize::try_from(self.column_indices[index]).expect("u32 CSR column fits usize");
                result += row_weight * self.coefficient(index) * column_weights[column];
            }
        }
        Ok(result)
    }
    fn row_bounds(&self, row: usize) -> Option<core::ops::Range<usize>> {
        let next_row = row.checked_add(1)?;
        let start =
            usize::try_from(*self.row_offsets.get(row)?).expect("u32 CSR offset fits usize");
        let end =
            usize::try_from(*self.row_offsets.get(next_row)?).expect("u32 CSR offset fits usize");
        Some(start..end)
    }
    fn coefficient(&self, index: usize) -> Scalar {
        let dictionary_index = self
            .coefficient_ids
            .get(index)
            .expect("CSR coefficient ID exists for every nonzero");
        *self
            .coefficient_dictionary
            .get(dictionary_index)
            .expect("CSR coefficient ID indexes the immutable dictionary")
    }
}
impl SparseMatrixRowBuilder {
    /// Start a fixed-row CSR matrix with exactly counted nonzero storage.
    pub(super) fn new(
        rows: usize,
        columns: usize,
        expected_nonzero_count: usize,
        expected_coefficient_count: usize,
    ) -> Result<Self, R1csError> {
        validate_csr_dimensions(rows, columns, expected_nonzero_count)?;
        if expected_coefficient_count > expected_nonzero_count {
            return Err(R1csError::CsrEntryCountMismatch);
        }
        let offset_count = rows.checked_add(1).ok_or(R1csError::CsrStorageOverflow)?;
        let mut row_offsets = try_vec_with_exact_capacity(offset_count)?;
        let column_indices = try_vec_with_exact_capacity(expected_nonzero_count)?;
        let coefficient_ids =
            CoefficientIds::with_capacity(expected_nonzero_count, expected_coefficient_count)?;
        let coefficient_dictionary = try_vec_with_exact_capacity(expected_coefficient_count)?;
        let mut coefficient_lookup = HashMap::new();
        coefficient_lookup
            .try_reserve(expected_coefficient_count)
            .map_err(|_| R1csError::CsrStorageAllocation)?;
        row_offsets.push(0);
        Ok(Self {
            rows,
            columns,
            expected_nonzero_count,
            expected_coefficient_count,
            row_offsets,
            column_indices,
            coefficient_ids,
            coefficient_dictionary,
            coefficient_lookup,
        })
    }
    /// Consume one column-sorted, nonzero CSR row.
    pub(super) fn append_canonical_row<I>(&mut self, entries: I) -> Result<(), R1csError>
    where
        I: IntoIterator<Item = (usize, Scalar)>,
        I::IntoIter: ExactSizeIterator,
    {
        if self.row_offsets.len() > self.rows {
            return Err(R1csError::InvalidDimension);
        }
        let entries = entries.into_iter();
        let new_nonzero_count = self
            .column_indices
            .len()
            .checked_add(entries.len())
            .ok_or(R1csError::CsrStorageOverflow)?;
        if new_nonzero_count > self.expected_nonzero_count {
            return Err(R1csError::CsrEntryCountMismatch);
        }
        let mut previous = None;
        for (column, coefficient) in entries {
            if self.column_indices.len() >= self.expected_nonzero_count {
                return Err(R1csError::CsrEntryCountMismatch);
            }
            if column >= self.columns
                || coefficient.is_zero()
                || previous.is_some_and(|prior| prior >= column)
            {
                return Err(R1csError::NonCanonicalMatrix);
            }
            previous = Some(column);
            let coefficient_key = coefficient.to_be_bytes();
            let coefficient_id = if let Some(id) = self.coefficient_lookup.get(&coefficient_key) {
                *id
            } else {
                if self.coefficient_dictionary.len() >= self.expected_coefficient_count {
                    return Err(R1csError::CsrEntryCountMismatch);
                }
                let id = u32::try_from(self.coefficient_dictionary.len())
                    .map_err(|_| R1csError::CsrStorageOverflow)?;
                self.coefficient_dictionary.push(coefficient);
                let previous = self.coefficient_lookup.insert(coefficient_key, id);
                debug_assert!(previous.is_none());
                id
            };
            self.column_indices
                .push(u32::try_from(column).map_err(|_| R1csError::CsrStorageOverflow)?);
            self.coefficient_ids.push(coefficient_id)?;
        }
        if self.column_indices.len() != new_nonzero_count
            || self.coefficient_ids.len() != new_nonzero_count
        {
            return Err(R1csError::CsrEntryCountMismatch);
        }
        self.row_offsets
            .push(u32::try_from(new_nonzero_count).map_err(|_| R1csError::CsrStorageOverflow)?);
        Ok(())
    }
    /// Finish the matrix, appending empty offsets through the fixed row count.
    pub(super) fn finish(mut self) -> Result<SparseMatrix, R1csError> {
        if self.column_indices.len() != self.expected_nonzero_count
            || self.coefficient_ids.len() != self.expected_nonzero_count
            || self.coefficient_dictionary.len() != self.expected_coefficient_count
            || self.coefficient_lookup.len() != self.expected_coefficient_count
        {
            return Err(R1csError::CsrEntryCountMismatch);
        }
        let final_offset = u32::try_from(self.expected_nonzero_count)
            .map_err(|_| R1csError::CsrStorageOverflow)?;
        while self.row_offsets.len() - 1 < self.rows {
            self.row_offsets.push(final_offset);
        }
        drop(self.coefficient_lookup);
        Ok(SparseMatrix {
            rows: self.rows,
            columns: self.columns,
            row_offsets: self.row_offsets,
            column_indices: self.column_indices,
            coefficient_ids: self.coefficient_ids,
            coefficient_dictionary: self.coefficient_dictionary,
        })
    }
    #[cfg(test)]
    fn storage_capacities(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.row_offsets.capacity(),
            self.column_indices.capacity(),
            self.coefficient_ids.capacity(),
            self.coefficient_dictionary.capacity(),
            self.coefficient_lookup.capacity(),
        )
    }
}
impl CoefficientIds {
    fn with_capacity(nonzero_count: usize, coefficient_count: usize) -> Result<Self, R1csError> {
        let largest_id = coefficient_count.saturating_sub(1);
        if u8::try_from(largest_id).is_ok() {
            Ok(Self::U8(try_vec_with_exact_capacity(nonzero_count)?))
        } else if u16::try_from(largest_id).is_ok() {
            Ok(Self::U16(try_vec_with_exact_capacity(nonzero_count)?))
        } else {
            Ok(Self::U32(try_vec_with_exact_capacity(nonzero_count)?))
        }
    }
    fn push(&mut self, id: u32) -> Result<(), R1csError> {
        match self {
            Self::U8(ids) => ids.push(u8::try_from(id).map_err(|_| R1csError::CsrStorageOverflow)?),
            Self::U16(ids) => {
                ids.push(u16::try_from(id).map_err(|_| R1csError::CsrStorageOverflow)?);
            }
            Self::U32(ids) => ids.push(id),
        }
        Ok(())
    }
    fn get(&self, index: usize) -> Option<usize> {
        match self {
            Self::U8(ids) => ids.get(index).copied().map(usize::from),
            Self::U16(ids) => ids.get(index).copied().map(usize::from),
            Self::U32(ids) => ids
                .get(index)
                .copied()
                .and_then(|id| usize::try_from(id).ok()),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::U8(ids) => ids.len(),
            Self::U16(ids) => ids.len(),
            Self::U32(ids) => ids.len(),
        }
    }
    #[cfg(test)]
    fn capacity(&self) -> usize {
        match self {
            Self::U8(ids) => ids.capacity(),
            Self::U16(ids) => ids.capacity(),
            Self::U32(ids) => ids.capacity(),
        }
    }
    #[cfg(test)]
    fn element_width(&self) -> usize {
        match self {
            Self::U8(_) => core::mem::size_of::<u8>(),
            Self::U16(_) => core::mem::size_of::<u16>(),
            Self::U32(_) => core::mem::size_of::<u32>(),
        }
    }
}
impl CoefficientDictionaryCounter {
    pub(super) fn new() -> Self {
        Self {
            coefficients: HashMap::new(),
        }
    }
    pub(super) fn observe(&mut self, coefficient: Scalar) -> Result<(), R1csError> {
        let coefficient = coefficient.to_be_bytes();
        if self.coefficients.contains_key(&coefficient) {
            return Ok(());
        }
        let next_len = self
            .coefficients
            .len()
            .checked_add(1)
            .ok_or(R1csError::CsrStorageOverflow)?;
        if u32::try_from(next_len).is_err() {
            return Err(R1csError::CsrStorageOverflow);
        }
        self.coefficients
            .try_reserve(1)
            .map_err(|_| R1csError::CsrStorageAllocation)?;
        let previous = self.coefficients.insert(coefficient, ());
        debug_assert!(previous.is_none());
        debug_assert_eq!(self.coefficients.len(), next_len);
        Ok(())
    }
    pub(super) fn len(&self) -> usize {
        self.coefficients.len()
    }
}
fn validate_csr_dimensions(
    rows: usize,
    columns: usize,
    nonzero_count: usize,
) -> Result<(), R1csError> {
    if rows == 0 || columns == 0 {
        return Err(R1csError::InvalidDimension);
    }
    if u32::try_from(rows).is_err()
        || u32::try_from(columns).is_err()
        || u32::try_from(nonzero_count).is_err()
    {
        return Err(R1csError::CsrStorageOverflow);
    }
    Ok(())
}
fn try_vec_with_exact_capacity<T>(capacity: usize) -> Result<Vec<T>, R1csError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| R1csError::CsrStorageAllocation)?;
    Ok(values)
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Shape {
    constraint_count: usize,
    variable_count: usize,
    public_input_count: usize,
    pub(super) a: SparseMatrix,
    pub(super) b: SparseMatrix,
    pub(super) c: SparseMatrix,
}
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MatrixProducts {
    pub(super) a: Vec<Scalar>,
    pub(super) b: Vec<Scalar>,
    pub(super) c: Vec<Scalar>,
}
impl Shape {
    pub(super) fn new(
        constraint_count: usize,
        variable_count: usize,
        public_input_count: usize,
        a: SparseMatrix,
        b: SparseMatrix,
        c: SparseMatrix,
    ) -> Result<Self, R1csError> {
        let columns = variable_count
            .checked_add(1)
            .and_then(|value| value.checked_add(public_input_count))
            .ok_or(R1csError::InvalidDimension)?;
        if constraint_count == 0
            || variable_count == 0
            || !constraint_count.is_power_of_two()
            || [(&a), (&b), (&c)]
                .into_iter()
                .any(|matrix| matrix.rows() != constraint_count || matrix.columns() != columns)
        {
            return Err(R1csError::InvalidDimension);
        }
        Ok(Self {
            constraint_count,
            variable_count,
            public_input_count,
            a,
            b,
            c,
        })
    }
    pub(super) fn constraint_count(&self) -> usize {
        self.constraint_count
    }
    pub(super) fn variable_count(&self) -> usize {
        self.variable_count
    }
    pub(super) fn public_input_count(&self) -> usize {
        self.public_input_count
    }
    pub(super) fn columns(&self) -> usize {
        self.variable_count + 1 + self.public_input_count
    }
    #[cfg(test)]
    pub(super) fn multiply(&self, assignment: &[Scalar]) -> Result<MatrixProducts, R1csError> {
        if assignment.len() != self.columns() {
            return Err(R1csError::InvalidDimension);
        }
        Ok(MatrixProducts {
            a: self.a.multiply(assignment)?,
            b: self.b.multiply(assignment)?,
            c: self.c.multiply(assignment)?,
        })
    }
    pub(super) fn validate_relaxed_assignment(
        &self,
        witness: &[Scalar],
        relaxation: Scalar,
        public_inputs: &[Scalar],
        error: &[Scalar],
    ) -> Result<(), R1csError> {
        if witness.len() != self.variable_count
            || public_inputs.len() != self.public_input_count
            || error.len() != self.constraint_count
        {
            return Err(R1csError::InvalidDimension);
        }
        for (row, error) in error.iter().copied().enumerate() {
            let a = self.evaluate_assignment_row(&self.a, row, witness, relaxation, public_inputs);
            let b = self.evaluate_assignment_row(&self.b, row, witness, relaxation, public_inputs);
            let c = self.evaluate_assignment_row(&self.c, row, witness, relaxation, public_inputs);
            if a * b != relaxation * c + error {
                return Err(R1csError::Unsatisfied);
            }
        }
        Ok(())
    }
    /// Validate `A z * B z = C z` without allocating a zero error vector or
    /// full matrix products.
    pub(super) fn validate_strict_assignment(
        &self,
        witness: &[Scalar],
        public_inputs: &[Scalar],
    ) -> Result<(), R1csError> {
        if witness.len() != self.variable_count || public_inputs.len() != self.public_input_count {
            return Err(R1csError::InvalidDimension);
        }
        for row in 0..self.constraint_count {
            let a =
                self.evaluate_assignment_row(&self.a, row, witness, Scalar::one(), public_inputs);
            let b =
                self.evaluate_assignment_row(&self.b, row, witness, Scalar::one(), public_inputs);
            let c =
                self.evaluate_assignment_row(&self.c, row, witness, Scalar::one(), public_inputs);
            if a * b != c {
                return Err(R1csError::Unsatisfied);
            }
        }
        Ok(())
    }
    /// Check whether one emitted circuit row exactly matches this immutable
    /// shape's canonical A/B/C row, without allocating matrix products.
    pub(super) fn matches_canonical_constraint_row(
        &self,
        row: usize,
        a: impl IntoIterator<Item = (usize, Scalar)>,
        b: impl IntoIterator<Item = (usize, Scalar)>,
        c: impl IntoIterator<Item = (usize, Scalar)>,
    ) -> Result<bool, R1csError> {
        if row >= self.constraint_count {
            return Err(R1csError::InvalidDimension);
        }
        Ok(self.row_matches(&self.a, row, a)
            && self.row_matches(&self.b, row, b)
            && self.row_matches(&self.c, row, c))
    }
    /// Return whether every A/B/C row from `start` through the fixed padded tail is empty.
    pub(super) fn has_only_empty_rows_from(&self, start: usize) -> Result<bool, R1csError> {
        if start > self.constraint_count {
            return Err(R1csError::InvalidDimension);
        }
        Ok((start..self.constraint_count).all(|row| {
            [&self.a, &self.b, &self.c].into_iter().all(|matrix| {
                matrix
                    .row_entries(row)
                    .expect("bounded row")
                    .next()
                    .is_none()
            })
        }))
    }
    /// Derive the sole relaxed error vector while streaming rows directly.
    pub(super) fn derive_relaxed_error(
        &self,
        witness: &[Scalar],
        relaxation: Scalar,
        public_inputs: &[Scalar],
    ) -> Result<Vec<Scalar>, R1csError> {
        if witness.len() != self.variable_count || public_inputs.len() != self.public_input_count {
            return Err(R1csError::InvalidDimension);
        }
        let mut error = Vec::with_capacity(self.constraint_count);
        for row in 0..self.constraint_count {
            let a = self.evaluate_assignment_row(&self.a, row, witness, relaxation, public_inputs);
            let b = self.evaluate_assignment_row(&self.b, row, witness, relaxation, public_inputs);
            let c = self.evaluate_assignment_row(&self.c, row, witness, relaxation, public_inputs);
            error.push(a * b - relaxation * c);
        }
        Ok(error)
    }
    /// Derive Nova's cross term without materializing a combined assignment
    /// or all three matrix products.
    pub(super) fn derive_fold_cross_term(
        &self,
        relaxed_witness: &[Scalar],
        relaxed_relaxation: Scalar,
        relaxed_public_inputs: &[Scalar],
        relaxed_error: &[Scalar],
        strict_witness: &[Scalar],
        strict_public_inputs: &[Scalar],
    ) -> Result<Vec<Scalar>, R1csError> {
        if relaxed_witness.len() != self.variable_count
            || strict_witness.len() != self.variable_count
            || relaxed_public_inputs.len() != self.public_input_count
            || strict_public_inputs.len() != self.public_input_count
            || relaxed_error.len() != self.constraint_count
        {
            return Err(R1csError::InvalidDimension);
        }
        let effective_relaxation = relaxed_relaxation + Scalar::one();
        let inputs = FoldRowInputs {
            relaxed_witness,
            strict_witness,
            effective_relaxation,
            relaxed_public_inputs,
            strict_public_inputs,
        };
        let mut cross_term = Vec::with_capacity(self.constraint_count);
        for (row, error) in relaxed_error.iter().copied().enumerate() {
            let a = self.evaluate_fold_row(&self.a, row, &inputs);
            let b = self.evaluate_fold_row(&self.b, row, &inputs);
            let c = self.evaluate_fold_row(&self.c, row, &inputs);
            cross_term.push(a * b - effective_relaxation * c - error);
        }
        Ok(cross_term)
    }
    fn row_matches(
        &self,
        matrix: &SparseMatrix,
        row: usize,
        expected: impl IntoIterator<Item = (usize, Scalar)>,
    ) -> bool {
        matrix
            .row_entries(row)
            .expect("bounded row was checked")
            .eq(expected)
    }
    fn evaluate_assignment_row(
        &self,
        matrix: &SparseMatrix,
        row: usize,
        witness: &[Scalar],
        relaxation: Scalar,
        public_inputs: &[Scalar],
    ) -> Scalar {
        matrix
            .row_entries(row)
            .expect("shape matrices have every in-range row")
            .fold(Scalar::zero(), |sum, (column, coefficient)| {
                let value = if column < self.variable_count {
                    witness[column]
                } else if column == self.variable_count {
                    relaxation
                } else {
                    public_inputs[column - self.variable_count - 1]
                };
                sum + coefficient * value
            })
    }
    fn evaluate_fold_row(
        &self,
        matrix: &SparseMatrix,
        row: usize,
        inputs: &FoldRowInputs<'_>,
    ) -> Scalar {
        matrix
            .row_entries(row)
            .expect("shape matrices have every in-range row")
            .fold(Scalar::zero(), |sum, (column, coefficient)| {
                let value = if column < self.variable_count {
                    inputs.relaxed_witness[column] + inputs.strict_witness[column]
                } else if column == self.variable_count {
                    inputs.effective_relaxation
                } else {
                    let public_index = column - self.variable_count - 1;
                    inputs.relaxed_public_inputs[public_index]
                        + inputs.strict_public_inputs[public_index]
                };
                sum + coefficient * value
            })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Instance {
    pub(super) witness_commitment: Commitment,
    pub(super) public_inputs: Vec<Scalar>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Witness {
    pub(super) values: Vec<Scalar>,
    pub(super) blindings: Vec<Scalar>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RelaxedInstance {
    pub(super) witness_commitment: Commitment,
    pub(super) error_commitment: Commitment,
    pub(super) public_inputs: Vec<Scalar>,
    pub(super) relaxation: Scalar,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RelaxedWitness {
    pub(super) values: Vec<Scalar>,
    pub(super) witness_blindings: Vec<Scalar>,
    pub(super) error: Vec<Scalar>,
    pub(super) error_blindings: Vec<Scalar>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::algebra::inner_product;
    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }
    fn multiplication_shape() -> Shape {
        // z = [x, ONE, y], constraint x * x = y.
        let a = SparseMatrix::new(1, 3, &[(0, 0, s(1))]).expect("canonical A");
        let b = SparseMatrix::new(1, 3, &[(0, 0, s(1))]).expect("canonical B");
        let c = SparseMatrix::new(1, 3, &[(0, 2, s(1))]).expect("canonical C");
        Shape::new(1, 1, 1, a, b, c).expect("valid shape")
    }
    #[test]
    fn regular_shape_admits_non_power_of_two_witness_width() {
        // The governed Microsoft verifier circuit has 1,504 witness values;
        // Spartan pads that width for its inner table instead of changing the
        // committed R1CS assignment.
        let columns = 3 + 1 + 1;
        let a = SparseMatrix::new(1, columns, &[(0, 0, s(1))]).expect("canonical A");
        let b = SparseMatrix::new(1, columns, &[(0, 1, s(1))]).expect("canonical B");
        let c = SparseMatrix::new(1, columns, &[(0, 4, s(1))]).expect("canonical C");
        let shape = Shape::new(1, 3, 1, a, b, c).expect("unpadded witness width");
        shape
            .validate_strict_assignment(&[s(2), s(3), s(5)], &[s(6)])
            .expect("2 * 3 = 6");
    }
    #[test]
    fn strict_and_relaxed_assignments_are_checked_exactly() {
        let shape = multiplication_shape();
        shape
            .validate_strict_assignment(&[s(3)], &[s(9)])
            .expect("strict satisfying assignment");
        shape
            .validate_relaxed_assignment(&[s(3)], s(2), &[s(4)], &[s(1)])
            .expect("9 = 2*4 + 1");
        assert_eq!(
            shape.validate_relaxed_assignment(&[s(3)], s(2), &[s(4)], &[s(2)]),
            Err(R1csError::Unsatisfied)
        );
        assert!(shape.validate_strict_assignment(&[], &[s(9)]).is_err());
    }
    #[test]
    fn assignment_validation_streams_rows_without_full_products() {
        let source = include_str!("r1cs.rs");
        let validation = source
            .split("pub(super) fn validate_relaxed_assignment")
            .nth(1)
            .and_then(|source| {
                source
                    .split("#[derive(Clone, Debug, PartialEq, Eq)]")
                    .next()
            })
            .expect("validation implementation");
        assert!(validation.contains("evaluate_assignment_row"));
        assert!(!validation.contains("self.multiply(&assignment)"));
        assert!(!validation.contains("Vec::with_capacity(self.columns())"));
    }
    #[test]
    fn row_streamed_error_and_cross_term_match_full_products() {
        let shape = multiplication_shape();
        let relaxed_witness = [s(3)];
        let relaxation = s(2);
        let relaxed_public = [s(4)];
        let relaxed_error = [s(1)];
        let mut relaxed_assignment = relaxed_witness.to_vec();
        relaxed_assignment.push(relaxation);
        relaxed_assignment.extend(relaxed_public);
        let relaxed_products = shape.multiply(&relaxed_assignment).expect("dimensions");
        let expected_error = relaxed_products
            .a
            .iter()
            .copied()
            .zip(relaxed_products.b.iter().copied())
            .zip(relaxed_products.c.iter().copied())
            .map(|((a, b), c)| a * b - relaxation * c)
            .collect::<Vec<_>>();
        assert_eq!(
            shape
                .derive_relaxed_error(&relaxed_witness, relaxation, &relaxed_public)
                .expect("dimensions"),
            expected_error
        );
        let strict_witness = [s(5)];
        let strict_public = [s(25)];
        let effective_relaxation = relaxation + Scalar::one();
        let mut combined_assignment = vec![relaxed_witness[0] + strict_witness[0]];
        combined_assignment.push(effective_relaxation);
        combined_assignment.push(relaxed_public[0] + strict_public[0]);
        let combined_products = shape.multiply(&combined_assignment).expect("dimensions");
        let expected_cross_term = combined_products
            .a
            .into_iter()
            .zip(combined_products.b)
            .zip(combined_products.c.into_iter().zip(relaxed_error))
            .map(|((a, b), (c, error))| a * b - effective_relaxation * c - error)
            .collect::<Vec<_>>();
        assert_eq!(
            shape
                .derive_fold_cross_term(
                    &relaxed_witness,
                    relaxation,
                    &relaxed_public,
                    &relaxed_error,
                    &strict_witness,
                    &strict_public,
                )
                .expect("dimensions"),
            expected_cross_term
        );
    }
    #[test]
    fn sparse_matrix_rejects_duplicates_ordering_zero_and_bounds() {
        assert!(SparseMatrix::new(1, 1, &[(0, 0, s(0))]).is_err());
        assert!(SparseMatrix::new(1, 1, &[(0, 1, s(1))]).is_err());
        assert!(SparseMatrix::new(1, 1, &[(1, 0, s(1))]).is_err());
        assert!(SparseMatrix::new(1, 2, &[(0, 1, s(1)), (0, 0, s(1))]).is_err());
        assert!(SparseMatrix::new(1, 1, &[(0, 0, s(1)), (0, 0, s(2))]).is_err());
    }
    #[test]
    fn sparse_matrix_exposes_the_complete_canonical_entry_order() {
        let entries = [(0, 1, s(3)), (1, 0, s(4)), (1, 2, s(5))];
        let matrix = SparseMatrix::new(2, 3, &entries).expect("canonical matrix");
        assert_eq!(matrix.nonzero_count(), entries.len());
        assert_eq!(matrix.coefficient_count(), entries.len());
        assert_eq!(matrix.canonical_entries().collect::<Vec<_>>(), entries);
    }
    #[test]
    fn coefficient_dictionary_deduplicates_in_first_csr_occurrence_order() {
        let entries = [(0, 0, s(2)), (0, 2, s(2)), (1, 1, s(3)), (2, 0, s(2))];
        let matrix = SparseMatrix::new(3, 3, &entries).expect("canonical matrix");
        assert_eq!(matrix.nonzero_count(), 4);
        assert_eq!(matrix.coefficient_count(), 2);
        assert_eq!(matrix.coefficient_dictionary, vec![s(2), s(3)]);
        assert_eq!(matrix.coefficient_ids, CoefficientIds::U8(vec![0, 0, 1, 0]));
        assert_eq!(matrix.canonical_entries().collect::<Vec<_>>(), entries);
        assert_eq!(
            matrix.multiply(&[s(5), s(7), s(11)]).expect("dimensions"),
            vec![s(32), s(21), s(10)]
        );
    }
    #[test]
    fn coefficient_ids_select_the_smallest_exact_width_at_boundaries() {
        for (coefficient_count, expected_width) in
            [(0, 1), (256, 1), (257, 2), (65_536, 2), (65_537, 4)]
        {
            let ids = CoefficientIds::with_capacity(0, coefficient_count)
                .expect("zero-entry width probe does not allocate a large buffer");
            assert_eq!(ids.element_width(), expected_width);
            assert_eq!(ids.len(), 0);
            assert_eq!(ids.capacity(), 0);
        }
        let mut u8_ids = CoefficientIds::U8(Vec::new());
        assert_eq!(u8_ids.push(256), Err(R1csError::CsrStorageOverflow));
        let mut u16_ids = CoefficientIds::U16(Vec::new());
        assert_eq!(u16_ids.push(65_536), Err(R1csError::CsrStorageOverflow));
    }
    #[test]
    fn retained_csr_payload_matches_the_adaptive_memory_equations() {
        fn payload_bytes(rows: usize, entries: usize, distinct: usize, id_width: usize) -> usize {
            core::mem::size_of::<u32>() * (rows + 1)
                + (core::mem::size_of::<u32>() + id_width) * entries
                + core::mem::size_of::<Scalar>() * distinct
        }
        let (rows, entries, distinct) = (8, 40, 7);
        assert_eq!(
            payload_bytes(rows, entries, distinct, 1),
            4 * (rows + 1) + 5 * entries + 32 * distinct
        );
        assert_eq!(
            payload_bytes(rows, entries, distinct, 2),
            4 * (rows + 1) + 6 * entries + 32 * distinct
        );
        assert_eq!(
            payload_bytes(rows, entries, distinct, 4),
            4 * (rows + 1) + 8 * entries + 32 * distinct
        );
    }
    #[test]
    fn u32_csr_row_iteration_matches_canonical_entry_order() {
        let entries = [(0, 1, s(3)), (2, 0, s(4)), (2, 3, s(5))];
        let matrix = SparseMatrix::new(4, 4, &entries).expect("canonical matrix");
        for row in 0..matrix.rows() {
            let expected = entries
                .iter()
                .filter(|(entry_row, _, _)| *entry_row == row)
                .map(|(_, column, coefficient)| (*column, *coefficient))
                .collect::<Vec<_>>();
            assert_eq!(
                matrix
                    .row_entries(row)
                    .expect("in-range row")
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert!(matrix.row_entries(usize::MAX).is_none());
    }
    #[test]
    fn row_builder_pads_trailing_empty_rows_and_preserves_algebra() {
        let entries = [(0, 0, s(2)), (1, 2, s(3))];
        let expected = SparseMatrix::new(4, 3, &entries).expect("canonical matrix");
        let mut builder = SparseMatrixRowBuilder::new(4, 3, 2, 2).expect("bounded dimensions");
        let capacities = builder.storage_capacities();
        assert!(capacities.0 >= 5);
        assert!(capacities.1 >= 2);
        assert!(capacities.2 >= 2);
        assert!(capacities.3 >= 2);
        assert!(capacities.4 >= 2);
        builder
            .append_canonical_row([(0, s(2))])
            .expect("first canonical row");
        builder
            .append_canonical_row([(2, s(3))])
            .expect("second canonical row");
        assert_eq!(builder.storage_capacities(), capacities);
        let actual = builder.finish().expect("exact entry total");
        assert_eq!(
            (
                actual.row_offsets.capacity(),
                actual.column_indices.capacity(),
                actual.coefficient_ids.capacity(),
                actual.coefficient_dictionary.capacity(),
            ),
            (capacities.0, capacities.1, capacities.2, capacities.3)
        );
        assert_eq!(actual, expected);
        assert_eq!(
            actual.multiply(&[s(5), s(7), s(11)]).expect("dimensions"),
            vec![s(10), s(33), s(0), s(0)]
        );
        assert_eq!(actual.row_entries(2).expect("trailing row").count(), 0);
        assert_eq!(actual.row_entries(3).expect("trailing row").count(), 0);
    }
    #[test]
    fn row_builder_rejects_overfilled_and_underfilled_exact_storage() {
        let mut overfilled = SparseMatrixRowBuilder::new(1, 2, 1, 1).expect("exact storage");
        let capacities = overfilled.storage_capacities();
        assert_eq!(
            overfilled.append_canonical_row([(0, s(1)), (1, s(2))]),
            Err(R1csError::CsrEntryCountMismatch)
        );
        assert_eq!(overfilled.storage_capacities(), capacities);
        let mut underfilled = SparseMatrixRowBuilder::new(2, 2, 2, 1).expect("exact storage");
        underfilled
            .append_canonical_row([(0, s(1))])
            .expect("first row fits");
        assert_eq!(underfilled.finish(), Err(R1csError::CsrEntryCountMismatch));
    }
    #[test]
    fn row_builder_rejects_under_and_over_counted_coefficient_dictionaries() {
        let mut undercounted = SparseMatrixRowBuilder::new(1, 2, 2, 1).expect("bounded storage");
        assert_eq!(
            undercounted.append_canonical_row([(0, s(1)), (1, s(2))]),
            Err(R1csError::CsrEntryCountMismatch)
        );
        let mut overcounted = SparseMatrixRowBuilder::new(2, 1, 2, 2).expect("bounded storage");
        overcounted
            .append_canonical_row([(0, s(1))])
            .expect("first row");
        overcounted
            .append_canonical_row([(0, s(1))])
            .expect("repeated coefficient");
        assert_eq!(overcounted.finish(), Err(R1csError::CsrEntryCountMismatch));
    }
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn csr_storage_rejects_values_outside_u32_without_allocating() {
        let outside_u32 = u32::MAX as usize + 1;
        assert!(matches!(
            SparseMatrixRowBuilder::new(outside_u32, 1, 0, 0),
            Err(R1csError::CsrStorageOverflow)
        ));
        assert!(matches!(
            SparseMatrixRowBuilder::new(1, outside_u32, 0, 0),
            Err(R1csError::CsrStorageOverflow)
        ));
        assert!(matches!(
            SparseMatrixRowBuilder::new(1, 1, outside_u32, 0),
            Err(R1csError::CsrStorageOverflow)
        ));
        assert_eq!(
            SparseMatrix::new(1, outside_u32, &[]),
            Err(R1csError::CsrStorageOverflow)
        );
    }
    #[test]
    fn csr_source_keeps_compact_indices_dictionary_ids_and_fallible_reservation() {
        let source = include_str!("r1cs.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production R1CS source");
        assert!(production.contains("row_offsets: Vec<u32>"));
        assert!(production.contains("column_indices: Vec<u32>"));
        assert!(production.contains("coefficient_ids: CoefficientIds"));
        assert!(production.contains("coefficient_dictionary: Vec<Scalar>"));
        assert!(production.contains("coefficient_lookup: HashMap<[u8; 32], u32>"));
        assert!(production.contains("coefficients: HashMap<[u8; 32], ()>"));
        assert!(production.contains("U8(Vec<u8>)"));
        assert!(production.contains("U16(Vec<u16>)"));
        assert!(production.contains("U32(Vec<u32>)"));
        assert!(production.contains("expected_nonzero_count: usize"));
        assert!(production.contains("expected_coefficient_count: usize"));
        assert!(production.contains("try_reserve_exact(capacity)"));
        assert!(production.contains(".try_reserve(expected_coefficient_count)"));
        assert!(production.contains("drop(self.coefficient_lookup)"));
        assert!(production.contains("self.coefficient(index)"));
        assert!(production.contains("CsrEntryCountMismatch"));
        assert!(!production.contains("row_offsets: Vec<usize>"));
        assert!(!production.contains("column_indices: Vec<usize>"));
        assert!(!production.contains("coefficients: Vec<Scalar>"));
        assert!(!production.contains("BTreeMap"));
        assert!(!production.contains("BTreeSet"));
        let retained_matrix = production
            .split("pub(super) struct SparseMatrix {")
            .nth(1)
            .and_then(|tail| tail.split('}').next())
            .expect("retained sparse-matrix fields");
        assert!(retained_matrix.contains("coefficient_ids: CoefficientIds"));
        assert!(retained_matrix.contains("coefficient_dictionary: Vec<Scalar>"));
        assert!(!retained_matrix.contains("HashMap"));
        let row_reader = production
            .split("pub(super) fn row_entries")
            .nth(1)
            .and_then(|tail| tail.split("#[cfg(test)]").next())
            .expect("row iterator implementation");
        assert!(!row_reader.contains("Vec::"));
        assert!(!row_reader.contains("collect"));
        assert!(!row_reader.contains("HashMap"));
        assert_eq!(core::mem::size_of::<Scalar>(), 32);
    }
    #[test]
    fn matrix_binding_matches_full_bilinear_evaluation() {
        let shape = multiplication_shape();
        let rows = [s(7)];
        let columns = [s(11), s(13), s(17)];
        assert_eq!(
            shape.a.evaluate(&rows, &columns).expect("dimensions"),
            inner_product(&shape.a.bind_rows(&rows).expect("dimensions"), &columns)
                .expect("aligned")
        );
    }
    #[test]
    fn release_shape_and_sparse_matrix_are_not_deep_cloneable() {
        let source = include_str!("r1cs.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production R1CS source");
        for owner in ["SparseMatrix", "Shape"] {
            let declaration = production
                .split(&format!("pub(super) struct {owner}"))
                .next()
                .expect("bounded owner declaration prefix");
            let derive = declaration
                .rsplit("#[derive(")
                .next()
                .expect("owner derive");
            assert!(
                !derive.contains("Clone"),
                "{owner} must remain behind shared immutable ownership"
            );
        }
    }
}
