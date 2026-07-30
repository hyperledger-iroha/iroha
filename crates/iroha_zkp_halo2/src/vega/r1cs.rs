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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SparseMatrix {
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

    pub(super) fn entry_count(&self) -> usize {
        self.coefficients.len()
    }

    pub(super) fn canonical_entries(&self) -> impl Iterator<Item = (usize, usize, Scalar)> + '_ {
        self.row_offsets
            .windows(2)
            .enumerate()
            .flat_map(move |(row, bounds)| {
                (bounds[0]..bounds[1])
                    .map(move |index| (row, self.column_indices[index], self.coefficients[index]))
            })
    }

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Shape {
    constraint_count: usize,
    variable_count: usize,
    public_input_count: usize,
    pub(super) a: SparseMatrix,
    pub(super) b: SparseMatrix,
    pub(super) c: SparseMatrix,
}

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
        let mut assignment = Vec::with_capacity(self.columns());
        assignment.extend_from_slice(witness);
        assignment.push(relaxation);
        assignment.extend_from_slice(public_inputs);
        let products = self.multiply(&assignment)?;
        if products
            .a
            .iter()
            .copied()
            .zip(products.b.iter().copied())
            .zip(products.c.iter().copied().zip(error.iter().copied()))
            .any(|((a, b), (c, error))| a * b != relaxation * c + error)
        {
            return Err(R1csError::Unsatisfied);
        }
        Ok(())
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
            .validate_relaxed_assignment(&[s(3)], s(1), &[s(9)], &[s(0)])
            .expect("strict satisfying assignment");
        shape
            .validate_relaxed_assignment(&[s(3)], s(2), &[s(4)], &[s(1)])
            .expect("9 = 2*4 + 1");
        assert_eq!(
            shape.validate_relaxed_assignment(&[s(3)], s(2), &[s(4)], &[s(2)]),
            Err(R1csError::Unsatisfied)
        );
        assert!(
            shape
                .validate_relaxed_assignment(&[], s(1), &[s(9)], &[s(0)])
                .is_err()
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
}
