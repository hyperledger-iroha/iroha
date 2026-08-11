//! Deterministic fixed-shape R1CS synthesis for the closed Vega relation.

use std::{collections::BTreeMap, sync::Arc};

use thiserror::Error;

use super::{
    VegaT256ScalarV1 as Scalar,
    r1cs::{R1csError, Shape, SparseMatrixRowBuilder},
};

/// Hard synthesis bound shared with the polynomial evaluation work cap.
pub(super) const MAX_CIRCUIT_ROWS: usize = 1 << 20;

/// Failure while synthesizing a deterministic Vega R1CS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum CircuitError {
    #[error("Vega circuit dimensions exceed the fixed synthesis profile")]
    InvalidDimension,
    #[error("Vega circuit assignment does not match its public-input shape")]
    InvalidAssignment,
    #[error("Vega circuit rows do not match the canonical synthesis shape")]
    ShapeMismatch,
    #[error(transparent)]
    R1cs(#[from] R1csError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Variable {
    Private(usize),
    Public(usize),
    One,
}

#[derive(Clone, Debug, Default)]
pub(super) struct LinearCombination {
    terms: Vec<(Variable, Scalar)>,
}

impl LinearCombination {
    pub(super) fn zero() -> Self {
        Self::default()
    }

    pub(super) fn one() -> Self {
        Self::constant(Scalar::one())
    }

    pub(super) fn constant(value: Scalar) -> Self {
        if value.is_zero() {
            Self::zero()
        } else {
            Self {
                terms: vec![(Variable::One, value)],
            }
        }
    }

    pub(super) fn variable(variable: Variable) -> Self {
        Self {
            terms: vec![(variable, Scalar::one())],
        }
    }

    pub(super) fn plus(mut self, rhs: &Self) -> Self {
        self.terms.extend_from_slice(&rhs.terms);
        self
    }

    pub(super) fn minus(mut self, rhs: &Self) -> Self {
        self.terms.extend(
            rhs.terms
                .iter()
                .map(|(variable, value)| (*variable, -*value)),
        );
        self
    }

    pub(super) fn scaled(mut self, factor: Scalar) -> Self {
        for (_, coefficient) in &mut self.terms {
            *coefficient *= factor;
        }
        self
    }

    pub(super) fn add_term(mut self, variable: Variable, coefficient: Scalar) -> Self {
        if !coefficient.is_zero() {
            self.terms.push((variable, coefficient));
        }
        self
    }

    fn canonical_terms(&self) -> Vec<(Variable, Scalar)> {
        let mut terms = BTreeMap::<Variable, Scalar>::new();
        for (variable, coefficient) in &self.terms {
            *terms.entry(*variable).or_insert_with(Scalar::zero) += *coefficient;
        }
        terms
            .into_iter()
            .filter(|(_, coefficient)| !coefficient.is_zero())
            .collect()
    }
}

impl From<Variable> for LinearCombination {
    fn from(variable: Variable) -> Self {
        Self::variable(variable)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Bit {
    pub(super) variable: Variable,
}

impl Bit {
    pub(super) fn lc(self) -> LinearCombination {
        self.variable.into()
    }

    pub(super) fn variable(self) -> Variable {
        self.variable
    }
}

struct Constraint {
    a: LinearCombination,
    b: LinearCombination,
    c: LinearCombination,
}

struct SecretCircuitValues(Vec<Scalar>);

impl SecretCircuitValues {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn push(&mut self, value: Scalar) {
        self.0.push(value);
    }

    fn resize(&mut self, length: usize, value: Scalar) {
        self.0.resize(length, value);
    }

    fn get(&self, index: usize) -> Option<Scalar> {
        self.0.get(index).copied()
    }

    fn as_slice(&self) -> &[Scalar] {
        &self.0
    }

    fn into_inner(mut self) -> Vec<Scalar> {
        core::mem::take(&mut self.0)
    }
}

impl Drop for SecretCircuitValues {
    fn drop(&mut self) {
        for value in &mut self.0 {
            value.clear_secret();
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
    }
}

#[derive(Clone, Copy)]
pub(super) struct CircuitDimensions {
    pub(super) variable_count: usize,
    pub(super) constraint_count: usize,
    pub(super) emitted_private_value_count: usize,
    pub(super) emitted_constraint_count: usize,
}

/// Canonical fixed-shape topology plus the exact unpadded synthesis counts.
///
/// The raw counts distinguish a relation's emitted rows and private values
/// from the empty power-of-two CSR padding. Construct this once alongside the
/// canonical shape, then reuse it for every witness-only synthesis.
pub(super) struct CircuitProfile {
    shape: Arc<Shape>,
    raw_private_value_count: usize,
    raw_constraint_count: usize,
}

impl CircuitProfile {
    pub(super) fn new(
        shape: Arc<Shape>,
        raw_private_value_count: usize,
        raw_constraint_count: usize,
    ) -> Result<Self, CircuitError> {
        if raw_private_value_count == 0
            || raw_constraint_count == 0
            || shape.variable_count() > MAX_CIRCUIT_ROWS
            || shape.constraint_count() > MAX_CIRCUIT_ROWS
            || raw_private_value_count
                .checked_next_power_of_two()
                .ok_or(CircuitError::InvalidDimension)?
                != shape.variable_count()
            || raw_constraint_count
                .checked_next_power_of_two()
                .ok_or(CircuitError::InvalidDimension)?
                != shape.constraint_count()
        {
            return Err(CircuitError::InvalidDimension);
        }
        if !shape.has_only_empty_rows_from(raw_constraint_count)? {
            return Err(CircuitError::ShapeMismatch);
        }
        Ok(Self {
            shape,
            raw_private_value_count,
            raw_constraint_count,
        })
    }

    pub(super) fn shape(&self) -> &Arc<Shape> {
        &self.shape
    }

    fn raw_private_value_count(&self) -> usize {
        self.raw_private_value_count
    }

    fn raw_constraint_count(&self) -> usize {
        self.raw_constraint_count
    }
}

enum CircuitBuilderMode {
    Shape(Vec<Constraint>),
    Count {
        emitted_constraint_count: usize,
    },
    Compile {
        variable_count: usize,
        constraint_count: usize,
        expected_private_value_count: usize,
        expected_constraint_count: usize,
        emitted_constraint_count: usize,
        a: SparseMatrixRowBuilder,
        b: SparseMatrixRowBuilder,
        c: SparseMatrixRowBuilder,
    },
    Witness {
        profile: Arc<CircuitProfile>,
        emitted_constraint_count: usize,
    },
}

pub(super) struct CircuitBuilder {
    public_inputs: Vec<Scalar>,
    private_values: SecretCircuitValues,
    mode: CircuitBuilderMode,
}

#[derive(Clone)]
pub(super) struct CircuitAssignment {
    pub(super) shape: Arc<Shape>,
    pub(super) witness: Vec<Scalar>,
    pub(super) public_inputs: Vec<Scalar>,
}

impl CircuitBuilder {
    pub(super) fn new(public_inputs: Vec<Scalar>) -> Result<Self, CircuitError> {
        if public_inputs.is_empty() {
            return Err(CircuitError::InvalidDimension);
        }
        Ok(Self {
            public_inputs,
            private_values: SecretCircuitValues::new(),
            mode: CircuitBuilderMode::Shape(Vec::new()),
        })
    }

    /// Count a deterministic circuit while retaining only scalar assignments.
    pub(super) fn new_counting(public_inputs: Vec<Scalar>) -> Result<Self, CircuitError> {
        if public_inputs.is_empty() {
            return Err(CircuitError::InvalidDimension);
        }
        Ok(Self {
            public_inputs,
            private_values: SecretCircuitValues::new(),
            mode: CircuitBuilderMode::Count {
                emitted_constraint_count: 0,
            },
        })
    }

    /// Compile a deterministic circuit directly into its final CSR matrices.
    pub(super) fn new_compiling(
        public_inputs: Vec<Scalar>,
        dimensions: CircuitDimensions,
    ) -> Result<Self, CircuitError> {
        if public_inputs.is_empty()
            || dimensions.variable_count == 0
            || dimensions.constraint_count == 0
        {
            return Err(CircuitError::InvalidDimension);
        }
        let columns = dimensions
            .variable_count
            .checked_add(1)
            .and_then(|value| value.checked_add(public_inputs.len()))
            .ok_or(CircuitError::InvalidDimension)?;
        Ok(Self {
            public_inputs,
            private_values: SecretCircuitValues::with_capacity(dimensions.variable_count),
            mode: CircuitBuilderMode::Compile {
                variable_count: dimensions.variable_count,
                constraint_count: dimensions.constraint_count,
                expected_private_value_count: dimensions.emitted_private_value_count,
                expected_constraint_count: dimensions.emitted_constraint_count,
                emitted_constraint_count: 0,
                a: SparseMatrixRowBuilder::new(dimensions.constraint_count, columns)?,
                b: SparseMatrixRowBuilder::new(dimensions.constraint_count, columns)?,
                c: SparseMatrixRowBuilder::new(dimensions.constraint_count, columns)?,
            },
        })
    }

    /// Synthesize one witness against an immutable canonical profile.
    /// Constraints are checked and discarded as they are emitted.
    pub(super) fn new_with_profile(
        public_inputs: Vec<Scalar>,
        profile: Arc<CircuitProfile>,
    ) -> Result<Self, CircuitError> {
        if public_inputs.is_empty() || public_inputs.len() != profile.shape().public_input_count() {
            return Err(CircuitError::InvalidDimension);
        }
        Ok(Self {
            public_inputs,
            private_values: SecretCircuitValues::with_capacity(profile.shape().variable_count()),
            mode: CircuitBuilderMode::Witness {
                profile,
                emitted_constraint_count: 0,
            },
        })
    }

    pub(super) fn public(&self, index: usize) -> Result<Variable, CircuitError> {
        (index < self.public_inputs.len())
            .then_some(Variable::Public(index))
            .ok_or(CircuitError::InvalidDimension)
    }

    pub(super) fn alloc(&mut self, value: Scalar) -> Result<Variable, CircuitError> {
        let limit = match &self.mode {
            CircuitBuilderMode::Compile {
                expected_private_value_count,
                ..
            } => *expected_private_value_count,
            CircuitBuilderMode::Witness { profile, .. } => profile.raw_private_value_count(),
            CircuitBuilderMode::Shape(_) | CircuitBuilderMode::Count { .. } => MAX_CIRCUIT_ROWS,
        };
        if self.private_values.len() >= limit {
            return Err(CircuitError::InvalidDimension);
        }
        let variable = Variable::Private(self.private_values.len());
        self.private_values.push(value);
        Ok(variable)
    }

    pub(super) fn alloc_bit(&mut self, value: bool) -> Result<Bit, CircuitError> {
        let variable = self.alloc(Scalar::from_u64(u64::from(value)))?;
        let bit = Bit { variable };
        self.enforce(
            bit.lc(),
            bit.lc().minus(&LinearCombination::one()),
            LinearCombination::zero(),
        )?;
        Ok(bit)
    }

    pub(super) fn enforce(
        &mut self,
        a: LinearCombination,
        b: LinearCombination,
        c: LinearCombination,
    ) -> Result<(), CircuitError> {
        let witness_mode = match &self.mode {
            CircuitBuilderMode::Witness {
                profile,
                emitted_constraint_count,
                ..
            } => Some((
                profile.shape().as_ref(),
                profile.raw_constraint_count(),
                *emitted_constraint_count,
            )),
            CircuitBuilderMode::Shape(_)
            | CircuitBuilderMode::Count { .. }
            | CircuitBuilderMode::Compile { .. } => None,
        };
        if let Some((shape, expected_constraint_count, row)) = witness_mode {
            if row >= MAX_CIRCUIT_ROWS || row >= expected_constraint_count {
                return Err(CircuitError::InvalidDimension);
            }
            let a_row = matrix_row_entries(shape.variable_count(), &a)?;
            let b_row = matrix_row_entries(shape.variable_count(), &b)?;
            let c_row = matrix_row_entries(shape.variable_count(), &c)?;
            if !shape.matches_canonical_constraint_row(row, &a_row, &b_row, &c_row)? {
                return Err(CircuitError::ShapeMismatch);
            }
            let CircuitBuilderMode::Witness {
                emitted_constraint_count,
                ..
            } = &mut self.mode
            else {
                unreachable!("witness mode was observed");
            };
            *emitted_constraint_count = emitted_constraint_count
                .checked_add(1)
                .ok_or(CircuitError::InvalidDimension)?;
            return Ok(());
        }

        let compile_mode = match &self.mode {
            CircuitBuilderMode::Compile {
                variable_count,
                expected_constraint_count,
                emitted_constraint_count,
                ..
            } => Some((
                *variable_count,
                *expected_constraint_count,
                *emitted_constraint_count,
            )),
            CircuitBuilderMode::Shape(_)
            | CircuitBuilderMode::Count { .. }
            | CircuitBuilderMode::Witness { .. } => None,
        };
        if let Some((variable_count, constraint_count, row)) = compile_mode {
            if row >= MAX_CIRCUIT_ROWS || row >= constraint_count {
                return Err(CircuitError::InvalidDimension);
            }
            let a_row = matrix_row_entries(variable_count, &a)?;
            let b_row = matrix_row_entries(variable_count, &b)?;
            let c_row = matrix_row_entries(variable_count, &c)?;
            let CircuitBuilderMode::Compile {
                emitted_constraint_count,
                a,
                b,
                c,
                ..
            } = &mut self.mode
            else {
                unreachable!("compile mode was observed");
            };
            a.append_canonical_row(a_row)?;
            b.append_canonical_row(b_row)?;
            c.append_canonical_row(c_row)?;
            *emitted_constraint_count = emitted_constraint_count
                .checked_add(1)
                .ok_or(CircuitError::InvalidDimension)?;
            return Ok(());
        }

        if let CircuitBuilderMode::Count {
            emitted_constraint_count,
        } = &mut self.mode
        {
            if *emitted_constraint_count >= MAX_CIRCUIT_ROWS {
                return Err(CircuitError::InvalidDimension);
            }
            *emitted_constraint_count = emitted_constraint_count
                .checked_add(1)
                .ok_or(CircuitError::InvalidDimension)?;
            return Ok(());
        }

        let CircuitBuilderMode::Shape(constraints) = &mut self.mode else {
            unreachable!("all streaming modes returned above");
        };
        if constraints.len() >= MAX_CIRCUIT_ROWS {
            return Err(CircuitError::InvalidDimension);
        }
        constraints.push(Constraint { a, b, c });
        Ok(())
    }

    pub(super) fn enforce_zero(&mut self, value: LinearCombination) -> Result<(), CircuitError> {
        self.enforce(LinearCombination::one(), value, LinearCombination::zero())
    }

    pub(super) fn enforce_equal(
        &mut self,
        left: LinearCombination,
        right: LinearCombination,
    ) -> Result<(), CircuitError> {
        self.enforce_zero(left.minus(&right))
    }

    pub(super) fn evaluate(&self, value: &LinearCombination) -> Scalar {
        value
            .terms
            .iter()
            .fold(Scalar::zero(), |sum, (variable, coefficient)| {
                let assigned = match variable {
                    Variable::Private(index) => {
                        self.private_values.get(*index).expect("allocated private")
                    }
                    Variable::Public(index) => self.public_inputs[*index],
                    Variable::One => Scalar::one(),
                };
                sum + assigned * *coefficient
            })
    }

    pub(super) fn multiply(
        &mut self,
        left: LinearCombination,
        right: LinearCombination,
    ) -> Result<Variable, CircuitError> {
        let value = self.evaluate(&left) * self.evaluate(&right);
        let output = self.alloc(value)?;
        self.enforce(left, right, output.into())?;
        Ok(output)
    }

    pub(super) fn select(
        &mut self,
        condition: Bit,
        when_true: LinearCombination,
        when_false: LinearCombination,
    ) -> Result<Variable, CircuitError> {
        let condition_value = self.evaluate(&condition.lc());
        let selected = if condition_value == Scalar::one() {
            self.evaluate(&when_true)
        } else {
            self.evaluate(&when_false)
        };
        let output = self.alloc(selected)?;
        self.enforce(
            condition.lc(),
            when_true.minus(&when_false),
            LinearCombination::from(output).minus(&when_false),
        )?;
        Ok(output)
    }

    /// Return a bit equal to one exactly when `value` is zero.
    pub(super) fn is_zero(&mut self, value: LinearCombination) -> Result<Bit, CircuitError> {
        self.inverse_or_zero(value).map(|(bit, _)| bit)
    }

    /// Return `(is_zero, inverse_or_zero)` with both branches constrained.
    pub(super) fn inverse_or_zero(
        &mut self,
        value: LinearCombination,
    ) -> Result<(Bit, Variable), CircuitError> {
        let assigned = self.evaluate(&value);
        let is_zero = assigned.is_zero();
        let bit = self.alloc_bit(is_zero)?;
        let inverse = self.alloc(if is_zero {
            Scalar::zero()
        } else {
            assigned
                .inverse()
                .map_err(|_| CircuitError::InvalidAssignment)?
        })?;
        self.enforce(
            value.clone(),
            inverse.into(),
            LinearCombination::one().minus(&bit.lc()),
        )?;
        self.enforce(value, bit.lc(), LinearCombination::zero())?;
        Ok((bit, inverse))
    }

    pub(super) fn and(&mut self, left: Bit, right: Bit) -> Result<Bit, CircuitError> {
        let value = self.evaluate(&left.lc()) == Scalar::one()
            && self.evaluate(&right.lc()) == Scalar::one();
        let output = self.alloc_bit(value)?;
        self.enforce(left.lc(), right.lc(), output.lc())?;
        Ok(output)
    }

    pub(super) fn or(&mut self, left: Bit, right: Bit) -> Result<Bit, CircuitError> {
        let both = self.and(left, right)?;
        let value = self.evaluate(&left.lc()) == Scalar::one()
            || self.evaluate(&right.lc()) == Scalar::one();
        let output = self.alloc_bit(value)?;
        self.enforce_equal(output.lc(), left.lc().plus(&right.lc()).minus(&both.lc()))?;
        Ok(output)
    }

    pub(super) fn xor(&mut self, left: Bit, right: Bit) -> Result<Bit, CircuitError> {
        let left_value = self.evaluate(&left.lc()) == Scalar::one();
        let right_value = self.evaluate(&right.lc()) == Scalar::one();
        let output = self.alloc_bit(left_value ^ right_value)?;
        let two = Scalar::from_u64(2);
        self.enforce(
            left.lc(),
            right.lc(),
            left.lc().plus(&right.lc()).minus(&output.lc()).scaled(
                two.inverse()
                    .expect("two is invertible in the T256 scalar field"),
            ),
        )?;
        Ok(output)
    }

    pub(super) fn not(&mut self, bit: Bit) -> Result<Bit, CircuitError> {
        let value = self.evaluate(&bit.lc()) == Scalar::zero();
        let output = self.alloc_bit(value)?;
        self.enforce_equal(output.lc(), LinearCombination::one().minus(&bit.lc()))?;
        Ok(output)
    }

    pub(super) fn finalize(self) -> Result<CircuitAssignment, CircuitError> {
        let Self {
            public_inputs,
            mut private_values,
            mode,
        } = self;
        let CircuitBuilderMode::Shape(constraints) = mode else {
            return Err(CircuitError::InvalidAssignment);
        };
        if private_values.is_empty() || constraints.is_empty() {
            return Err(CircuitError::InvalidDimension);
        }
        let variable_count = private_values
            .len()
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        let constraint_count = constraints
            .len()
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        if variable_count > MAX_CIRCUIT_ROWS || constraint_count > MAX_CIRCUIT_ROWS {
            return Err(CircuitError::InvalidDimension);
        }
        let columns = variable_count
            .checked_add(1)
            .and_then(|value| value.checked_add(public_inputs.len()))
            .ok_or(CircuitError::InvalidDimension)?;
        let mut a = SparseMatrixRowBuilder::new(constraint_count, columns)?;
        let mut b = SparseMatrixRowBuilder::new(constraint_count, columns)?;
        let mut c = SparseMatrixRowBuilder::new(constraint_count, columns)?;
        for Constraint {
            a: constraint_a,
            b: constraint_b,
            c: constraint_c,
        } in constraints
        {
            a.append_canonical_row(matrix_row_entries(variable_count, &constraint_a)?)?;
            b.append_canonical_row(matrix_row_entries(variable_count, &constraint_b)?)?;
            c.append_canonical_row(matrix_row_entries(variable_count, &constraint_c)?)?;
        }
        let shape = Arc::new(Shape::new(
            constraint_count,
            variable_count,
            public_inputs.len(),
            a.finish(),
            b.finish(),
            c.finish(),
        )?);
        private_values.resize(variable_count, Scalar::zero());
        Ok(CircuitAssignment {
            shape,
            witness: private_values.into_inner(),
            public_inputs,
        })
    }

    /// Return the power-of-two dimensions discovered by a count-only pass.
    pub(super) fn finish_counting(self) -> Result<CircuitDimensions, CircuitError> {
        let Self {
            private_values,
            mode,
            ..
        } = self;
        let CircuitBuilderMode::Count {
            emitted_constraint_count,
        } = mode
        else {
            return Err(CircuitError::InvalidAssignment);
        };
        if private_values.is_empty() || emitted_constraint_count == 0 {
            return Err(CircuitError::InvalidDimension);
        }
        let variable_count = private_values
            .len()
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        let constraint_count = emitted_constraint_count
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        if variable_count > MAX_CIRCUIT_ROWS || constraint_count > MAX_CIRCUIT_ROWS {
            return Err(CircuitError::InvalidDimension);
        }
        Ok(CircuitDimensions {
            variable_count,
            constraint_count,
            emitted_private_value_count: private_values.len(),
            emitted_constraint_count,
        })
    }

    /// Finish the append-only compile pass without validating dummy shape values.
    pub(super) fn finalize_compiled(self) -> Result<CircuitAssignment, CircuitError> {
        let Self {
            public_inputs,
            mut private_values,
            mode,
        } = self;
        let CircuitBuilderMode::Compile {
            variable_count,
            constraint_count,
            expected_private_value_count,
            expected_constraint_count,
            emitted_constraint_count,
            a,
            b,
            c,
        } = mode
        else {
            return Err(CircuitError::InvalidAssignment);
        };
        if private_values.is_empty()
            || emitted_constraint_count == 0
            || private_values.len() != expected_private_value_count
            || emitted_constraint_count != expected_constraint_count
        {
            return Err(CircuitError::InvalidDimension);
        }
        if private_values
            .len()
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?
            != variable_count
            || emitted_constraint_count
                .checked_next_power_of_two()
                .ok_or(CircuitError::InvalidDimension)?
                != constraint_count
        {
            return Err(CircuitError::InvalidDimension);
        }
        let shape = Arc::new(Shape::new(
            constraint_count,
            variable_count,
            public_inputs.len(),
            a.finish(),
            b.finish(),
            c.finish(),
        )?);
        private_values.resize(variable_count, Scalar::zero());
        Ok(CircuitAssignment {
            shape,
            witness: private_values.into_inner(),
            public_inputs,
        })
    }

    /// Finish a deterministic synthesis against a previously verified shape.
    ///
    /// Fixed relations use this after the canonical shape has been built once,
    /// so each additional witness does not rebuild the sparse matrices.
    pub(super) fn finalize_with_shape(self) -> Result<CircuitAssignment, CircuitError> {
        let Self {
            public_inputs,
            mut private_values,
            mode,
        } = self;
        let CircuitBuilderMode::Witness {
            profile,
            emitted_constraint_count,
        } = mode
        else {
            return Err(CircuitError::InvalidAssignment);
        };
        let shape = Arc::clone(profile.shape());
        if private_values.is_empty()
            || emitted_constraint_count == 0
            || private_values.len() != profile.raw_private_value_count()
            || emitted_constraint_count != profile.raw_constraint_count()
        {
            return Err(CircuitError::InvalidDimension);
        }
        let variable_count = private_values
            .len()
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        let constraint_count = emitted_constraint_count
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        if variable_count != shape.variable_count() || constraint_count != shape.constraint_count()
        {
            return Err(CircuitError::InvalidDimension);
        }
        private_values.resize(variable_count, Scalar::zero());
        shape.validate_strict_assignment(private_values.as_slice(), &public_inputs)?;
        Ok(CircuitAssignment {
            shape,
            witness: private_values.into_inner(),
            public_inputs,
        })
    }
}

fn matrix_row_entries(
    variable_count: usize,
    combination: &LinearCombination,
) -> Result<Vec<(usize, Scalar)>, CircuitError> {
    let mut row_entries = Vec::new();
    for (variable, coefficient) in combination.canonical_terms() {
        let column = match variable {
            Variable::Private(index) if index < variable_count => index,
            Variable::Private(_) => return Err(CircuitError::InvalidDimension),
            Variable::One => variable_count,
            Variable::Public(index) => variable_count
                .checked_add(1)
                .and_then(|value| value.checked_add(index))
                .ok_or(CircuitError::InvalidDimension)?,
        };
        row_entries.push((column, coefficient));
    }
    row_entries.sort_by_key(|(column, _)| *column);
    Ok(row_entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::r1cs::SparseMatrix;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn profile(
        shape: Arc<Shape>,
        raw_private_value_count: usize,
        raw_constraint_count: usize,
    ) -> Arc<CircuitProfile> {
        Arc::new(
            CircuitProfile::new(shape, raw_private_value_count, raw_constraint_count)
                .expect("canonical profile"),
        )
    }

    #[test]
    fn builder_synthesizes_one_strict_fixed_shape_relation() {
        let mut builder = CircuitBuilder::new(vec![s(12)]).expect("public");
        let public = builder.public(0).expect("index");
        let left = builder.alloc(s(3)).expect("left");
        let right = builder.alloc(s(4)).expect("right");
        builder
            .enforce(left.into(), right.into(), public.into())
            .expect("constraint");
        let assignment = builder.finalize().expect("shape");
        assignment
            .shape
            .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
            .expect("satisfying assignment");
    }

    #[test]
    fn boolean_zero_test_and_selection_are_constrained() {
        let mut builder = CircuitBuilder::new(vec![s(1)]).expect("public");
        let zero = builder.is_zero(LinearCombination::zero()).expect("zero");
        let nonzero = builder
            .is_zero(LinearCombination::constant(s(7)))
            .expect("nonzero");
        let selected = builder
            .select(
                zero,
                LinearCombination::constant(s(11)),
                LinearCombination::constant(s(13)),
            )
            .expect("select");
        builder
            .enforce_equal(selected.into(), LinearCombination::constant(s(11)))
            .expect("selected");
        builder
            .enforce_equal(nonzero.lc(), LinearCombination::zero())
            .expect("nonzero flag");
        let assignment = builder.finalize().expect("shape");
        assignment
            .shape
            .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
            .expect("satisfying assignment");
    }

    #[test]
    fn finalization_pads_csr_rows_without_padding_constraint_owners() {
        let mut builder = CircuitBuilder::new(vec![s(2)]).expect("public");
        let public = builder.public(0).expect("public index");
        let value = builder.alloc(s(2)).expect("value");
        for _ in 0..3 {
            builder
                .enforce(value.into(), LinearCombination::one(), public.into())
                .expect("constraint");
        }
        let assignment = builder.finalize().expect("shape");
        assert_eq!(assignment.shape.constraint_count(), 4);
        for matrix in [
            &assignment.shape.a,
            &assignment.shape.b,
            &assignment.shape.c,
        ] {
            assert_eq!(matrix.row_entries(3).expect("padded row").count(), 0);
        }
        assignment
            .shape
            .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
            .expect("padded relation remains strict");
    }

    fn synthesize_three_equal_rows(
        builder: &mut CircuitBuilder,
        value: Scalar,
    ) -> Result<(), CircuitError> {
        let public = builder.public(0)?;
        let value = builder.alloc(value)?;
        for _ in 0..3 {
            builder.enforce(value.into(), LinearCombination::one(), public.into())?;
        }
        Ok(())
    }

    #[test]
    fn count_then_compile_matches_materialized_shape_and_keeps_dummy_unsatisfied() {
        let mut materialized = CircuitBuilder::new(vec![s(2)]).expect("public");
        synthesize_three_equal_rows(&mut materialized, s(2)).expect("materialized rows");
        let materialized = materialized.finalize().expect("materialized shape");

        let mut counter = CircuitBuilder::new_counting(vec![s(2)]).expect("public");
        synthesize_three_equal_rows(&mut counter, s(2)).expect("counted rows");
        let dimensions = counter.finish_counting().expect("counted dimensions");
        assert_eq!(dimensions.emitted_private_value_count, 1);
        assert_eq!(dimensions.emitted_constraint_count, 3);

        let mut compiler = CircuitBuilder::new_compiling(vec![s(2)], dimensions).expect("compiler");
        synthesize_three_equal_rows(&mut compiler, s(2)).expect("compiled rows");
        let compiled = compiler.finalize_compiled().expect("compiled shape");
        assert_eq!(compiled.shape.as_ref(), materialized.shape.as_ref());
        assert_eq!(compiled.witness, materialized.witness);
        assert_eq!(compiled.public_inputs, materialized.public_inputs);

        let mut dummy_counter = CircuitBuilder::new_counting(vec![s(2)]).expect("public");
        synthesize_three_equal_rows(&mut dummy_counter, s(3)).expect("dummy rows");
        let dummy_dimensions = dummy_counter.finish_counting().expect("dummy dimensions");
        let mut dummy_compiler =
            CircuitBuilder::new_compiling(vec![s(2)], dummy_dimensions).expect("compiler");
        synthesize_three_equal_rows(&mut dummy_compiler, s(3)).expect("dummy rows");
        let dummy = dummy_compiler
            .finalize_compiled()
            .expect("shape compilation does not validate dummy values");
        assert!(matches!(
            dummy
                .shape
                .validate_strict_assignment(&dummy.witness, &dummy.public_inputs),
            Err(R1csError::Unsatisfied)
        ));
    }

    #[test]
    fn witness_mode_matches_the_canonical_shape_and_rejects_unsatisfied_rows() {
        let mut shape_builder = CircuitBuilder::new(vec![s(2)]).expect("public");
        let public = shape_builder.public(0).expect("public index");
        let value = shape_builder.alloc(s(2)).expect("value");
        shape_builder
            .enforce(value.into(), LinearCombination::one(), public.into())
            .expect("canonical constraint");
        let canonical = shape_builder.finalize().expect("canonical shape");
        let profile = profile(Arc::clone(&canonical.shape), 1, 1);

        let mut witness_builder =
            CircuitBuilder::new_with_profile(vec![s(2)], Arc::clone(&profile))
                .expect("shared shape");
        let public = witness_builder.public(0).expect("public index");
        let value = witness_builder.alloc(s(2)).expect("value");
        witness_builder
            .enforce(value.into(), LinearCombination::one(), public.into())
            .expect("matching witness row");
        let witness = witness_builder
            .finalize_with_shape()
            .expect("matching witness");
        assert!(Arc::ptr_eq(&witness.shape, &canonical.shape));
        assert_eq!(witness.witness, canonical.witness);

        let mut mismatched = CircuitBuilder::new_with_profile(vec![s(2)], Arc::clone(&profile))
            .expect("shared shape");
        let public = mismatched.public(0).expect("public index");
        let _ = mismatched.alloc(s(2)).expect("value");
        assert!(matches!(
            mismatched.enforce(
                LinearCombination::one(),
                LinearCombination::one(),
                public.into()
            ),
            Err(CircuitError::ShapeMismatch)
        ));

        let mut unsatisfied = CircuitBuilder::new_with_profile(vec![s(2)], Arc::clone(&profile))
            .expect("shared shape");
        let public = unsatisfied.public(0).expect("public index");
        let value = unsatisfied.alloc(s(3)).expect("value");
        unsatisfied
            .enforce(value.into(), LinearCombination::one(), public.into())
            .expect("matching topology");
        assert!(matches!(
            unsatisfied.finalize_with_shape(),
            Err(CircuitError::R1cs(R1csError::Unsatisfied))
        ));

        let mut short =
            CircuitBuilder::new_with_profile(vec![s(2)], profile).expect("shared shape");
        let _ = short.alloc(s(2)).expect("value");
        assert!(matches!(
            short.finalize_with_shape(),
            Err(CircuitError::InvalidDimension)
        ));
    }

    #[test]
    fn witness_mode_rejects_a_satisfying_nonempty_padded_tail() {
        let a = SparseMatrix::new(
            4,
            3,
            &[(0, 0, s(1)), (1, 0, s(1)), (2, 0, s(1)), (3, 1, s(1))],
        )
        .expect("canonical A");
        let b = SparseMatrix::new(
            4,
            3,
            &[(0, 1, s(1)), (1, 1, s(1)), (2, 1, s(1)), (3, 1, s(1))],
        )
        .expect("canonical B");
        let c = SparseMatrix::new(
            4,
            3,
            &[(0, 2, s(1)), (1, 2, s(1)), (2, 2, s(1)), (3, 1, s(1))],
        )
        .expect("canonical C");
        let shape = Arc::new(Shape::new(4, 1, 1, a, b, c).expect("shape"));
        shape
            .validate_strict_assignment(&[s(2)], &[s(2)])
            .expect("the nonempty tail itself is satisfying");
        assert!(matches!(
            CircuitProfile::new(shape, 1, 3),
            Err(CircuitError::ShapeMismatch)
        ));
    }

    #[test]
    fn shape_synthesis_does_not_retain_three_global_entry_vectors() {
        let source = include_str!("circuit.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production circuit source");
        assert!(production.contains("SparseMatrixRowBuilder::new"));
        assert!(production.contains("enum CircuitBuilderMode"));
        assert!(production.contains("Shape(Vec<Constraint>)"));
        assert!(production.contains("CircuitBuilderMode::Count"));
        assert!(production.contains("CircuitBuilderMode::Compile"));
        assert!(production.contains("CircuitBuilderMode::Witness"));
        assert!(production.contains("has_only_empty_rows_from"));
        assert!(production.contains("for Constraint"));
        assert!(!production.contains("constraints.resize"));
        assert!(!production.contains("&self.constraints"));
        assert!(!production.contains("collect::<Result<Vec<_>, _>>()"));
        assert!(!production.contains("collect::<Result<Vec<_>>>"));
        assert!(!production.contains("let mut a_entries = Vec::new()"));
        assert!(!production.contains("matrix_from_constraints"));
        let witness_constructor = production
            .split("pub(super) fn new_with_profile")
            .nth(1)
            .and_then(|tail| tail.split("pub(super) fn public").next())
            .expect("witness constructor");
        assert!(!witness_constructor.contains("Vec<Constraint>"));
        let witness_finalizer = production
            .split("pub(super) fn finalize_with_shape")
            .nth(1)
            .and_then(|tail| tail.split("fn matrix_row_entries").next())
            .expect("witness finalizer");
        assert!(!witness_finalizer.contains("has_only_empty_rows_from"));
    }
}
