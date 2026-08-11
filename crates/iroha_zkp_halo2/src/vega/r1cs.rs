//! Strict sparse relaxed-R1CS algebra for the Vega Neutron/Nova composition.

use thiserror::Error;

use super::{VegaT256ScalarV1 as Scalar, commitment::Commitment};

/// Failure while constructing or evaluating a Vega R1CS object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum R1csError {
    #[error("Vega R1CS dimensions do not match")]
    InvalidDimension,
    #[error("Vega sparse matrix entries are not canonical")]
    NonCanonicalMatrix,
    #[error("Vega R1CS assignment does not satisfy the relation")]
    Unsatisfied,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SparseMatrix {
    rows: usize,
    columns: usize,
    row_offsets: Vec<usize>,
    column_indices: Vec<usize>,
    coefficients: Vec<Scalar>,
}

/// Append-only CSR construction for a fixed number of canonical rows.
///
/// The builder retains only the final CSR buffers. Callers may supply one row
/// at a time and [`Self::finish`] pads any trailing rows with empty offsets.
pub(super) struct SparseMatrixRowBuilder {
    rows: usize,
    columns: usize,
    row_offsets: Vec<usize>,
    column_indices: Vec<usize>,
    coefficients: Vec<Scalar>,
}

impl SparseMatrix {
    pub(super) fn new(
        rows: usize,
        columns: usize,
        entries: &[(usize, usize, Scalar)],
    ) -> Result<Self, R1csError> {
        if rows == 0 || columns == 0 {
            return Err(R1csError::InvalidDimension);
        }
        let mut previous = None;
        let mut row_offsets = Vec::with_capacity(rows + 1);
        let mut column_indices = Vec::with_capacity(entries.len());
        let mut coefficients = Vec::with_capacity(entries.len());
        let mut cursor = 0;
        row_offsets.push(0);
        for row in 0..rows {
            while cursor < entries.len() && entries[cursor].0 == row {
                let (entry_row, column, coefficient) = entries[cursor];
                if entry_row >= rows
                    || column >= columns
                    || coefficient.is_zero()
                    || previous.is_some_and(|prior| prior >= (entry_row, column))
                {
                    return Err(R1csError::NonCanonicalMatrix);
                }
                previous = Some((entry_row, column));
                column_indices.push(column);
                coefficients.push(coefficient);
                cursor += 1;
            }
            row_offsets.push(cursor);
        }
        if cursor != entries.len() {
            return Err(R1csError::NonCanonicalMatrix);
        }
        Ok(Self {
            rows,
            columns,
            row_offsets,
            column_indices,
            coefficients,
        })
    }

    pub(super) fn rows(&self) -> usize {
        self.rows
    }

    pub(super) fn columns(&self) -> usize {
        self.columns
    }

    #[cfg(test)]
    pub(super) fn entry_count(&self) -> usize {
        self.coefficients.len()
    }

    #[cfg(test)]
    pub(super) fn canonical_entries(&self) -> impl Iterator<Item = (usize, usize, Scalar)> + '_ {
        self.row_offsets
            .windows(2)
            .enumerate()
            .flat_map(move |(row, bounds)| {
                (bounds[0]..bounds[1])
                    .map(move |index| (row, self.column_indices[index], self.coefficients[index]))
            })
    }

    pub(super) fn row_entries(
        &self,
        row: usize,
    ) -> Option<impl Iterator<Item = (usize, Scalar)> + '_> {
        let bounds = self.row_offsets.get(row..=row + 1)?;
        Some(
            (bounds[0]..bounds[1])
                .map(move |index| (self.column_indices[index], self.coefficients[index])),
        )
    }

    #[cfg(test)]
    pub(super) fn multiply(&self, vector: &[Scalar]) -> Result<Vec<Scalar>, R1csError> {
        if vector.len() != self.columns {
            return Err(R1csError::InvalidDimension);
        }
        let mut output = vec![Scalar::zero(); self.rows];
        for (row, output) in output.iter_mut().enumerate() {
            for index in self.row_offsets[row]..self.row_offsets[row + 1] {
                *output += self.coefficients[index] * vector[self.column_indices[index]];
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
            for index in self.row_offsets[row]..self.row_offsets[row + 1] {
                output[self.column_indices[index]] += weight * self.coefficients[index];
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
            for index in self.row_offsets[row]..self.row_offsets[row + 1] {
                result += row_weight
                    * self.coefficients[index]
                    * column_weights[self.column_indices[index]];
            }
        }
        Ok(result)
    }
}

impl SparseMatrixRowBuilder {
    /// Start a fixed-row CSR matrix without staging all of its rows.
    pub(super) fn new(rows: usize, columns: usize) -> Result<Self, R1csError> {
        if rows == 0 || columns == 0 {
            return Err(R1csError::InvalidDimension);
        }
        let mut row_offsets = Vec::with_capacity(rows + 1);
        row_offsets.push(0);
        Ok(Self {
            rows,
            columns,
            row_offsets,
            column_indices: Vec::new(),
            coefficients: Vec::new(),
        })
    }

    /// Consume one column-sorted, nonzero CSR row.
    pub(super) fn append_canonical_row(
        &mut self,
        entries: impl IntoIterator<Item = (usize, Scalar)>,
    ) -> Result<(), R1csError> {
        if self.row_offsets.len() - 1 >= self.rows {
            return Err(R1csError::InvalidDimension);
        }
        let mut previous = None;
        for (column, coefficient) in entries {
            if column >= self.columns
                || coefficient.is_zero()
                || previous.is_some_and(|prior| prior >= column)
            {
                return Err(R1csError::NonCanonicalMatrix);
            }
            previous = Some(column);
            self.column_indices.push(column);
            self.coefficients.push(coefficient);
        }
        self.row_offsets.push(self.column_indices.len());
        Ok(())
    }

    /// Finish the matrix, appending empty offsets through the fixed row count.
    pub(super) fn finish(mut self) -> SparseMatrix {
        while self.row_offsets.len() - 1 < self.rows {
            self.row_offsets.push(self.column_indices.len());
        }
        SparseMatrix {
            rows: self.rows,
            columns: self.columns,
            row_offsets: self.row_offsets,
            column_indices: self.column_indices,
            coefficients: self.coefficients,
        }
    }
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
            || !variable_count.is_power_of_two()
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
        a: &[(usize, Scalar)],
        b: &[(usize, Scalar)],
        c: &[(usize, Scalar)],
    ) -> Result<bool, R1csError> {
        if row >= self.constraint_count {
            return Err(R1csError::InvalidDimension);
        }
        Ok(self.row_matches(&self.a, row, a)
            && self.row_matches(&self.b, row, b)
            && self.row_matches(&self.c, row, c))
    }

    /// Return whether every A/B/C row from `start` through the fixed padded
    /// tail is empty.
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
        let mut cross_term = Vec::with_capacity(self.constraint_count);
        for (row, error) in relaxed_error.iter().copied().enumerate() {
            let a = self.evaluate_fold_row(
                &self.a,
                row,
                relaxed_witness,
                strict_witness,
                effective_relaxation,
                relaxed_public_inputs,
                strict_public_inputs,
            );
            let b = self.evaluate_fold_row(
                &self.b,
                row,
                relaxed_witness,
                strict_witness,
                effective_relaxation,
                relaxed_public_inputs,
                strict_public_inputs,
            );
            let c = self.evaluate_fold_row(
                &self.c,
                row,
                relaxed_witness,
                strict_witness,
                effective_relaxation,
                relaxed_public_inputs,
                strict_public_inputs,
            );
            cross_term.push(a * b - effective_relaxation * c - error);
        }
        Ok(cross_term)
    }

    fn row_matches(&self, matrix: &SparseMatrix, row: usize, expected: &[(usize, Scalar)]) -> bool {
        matrix
            .row_entries(row)
            .expect("bounded row was checked")
            .eq(expected.iter().copied())
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
        relaxed_witness: &[Scalar],
        strict_witness: &[Scalar],
        effective_relaxation: Scalar,
        relaxed_public_inputs: &[Scalar],
        strict_public_inputs: &[Scalar],
    ) -> Scalar {
        matrix
            .row_entries(row)
            .expect("shape matrices have every in-range row")
            .fold(Scalar::zero(), |sum, (column, coefficient)| {
                let value = if column < self.variable_count {
                    relaxed_witness[column] + strict_witness[column]
                } else if column == self.variable_count {
                    effective_relaxation
                } else {
                    let public_index = column - self.variable_count - 1;
                    relaxed_public_inputs[public_index] + strict_public_inputs[public_index]
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
        assert_eq!(matrix.entry_count(), entries.len());
        assert_eq!(matrix.canonical_entries().collect::<Vec<_>>(), entries);
    }

    #[test]
    fn row_builder_pads_trailing_empty_rows_and_preserves_algebra() {
        let entries = [(0, 0, s(2)), (1, 2, s(3))];
        let expected = SparseMatrix::new(4, 3, &entries).expect("canonical matrix");
        let mut builder = SparseMatrixRowBuilder::new(4, 3).expect("bounded dimensions");
        builder
            .append_canonical_row([(0, s(2))])
            .expect("first canonical row");
        builder
            .append_canonical_row([(2, s(3))])
            .expect("second canonical row");
        let actual = builder.finish();
        assert_eq!(actual, expected);
        assert_eq!(
            actual.multiply(&[s(5), s(7), s(11)]).expect("dimensions"),
            vec![s(10), s(33), s(0), s(0)]
        );
        assert_eq!(actual.row_entries(2).expect("trailing row").count(), 0);
        assert_eq!(actual.row_entries(3).expect("trailing row").count(), 0);
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
