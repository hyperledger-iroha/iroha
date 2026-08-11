//! Deterministic fixed-shape R1CS synthesis for the closed Vega relation.

use std::collections::BTreeMap;

use thiserror::Error;

use super::{
    VegaT256ScalarV1 as Scalar,
    r1cs::{R1csError, Shape, SparseMatrix},
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

#[derive(Clone)]
struct Constraint {
    a: LinearCombination,
    b: LinearCombination,
    c: LinearCombination,
}

pub(super) struct CircuitBuilder {
    public_inputs: Vec<Scalar>,
    private_values: Vec<Scalar>,
    constraints: Vec<Constraint>,
}

#[derive(Clone)]
pub(super) struct CircuitAssignment {
    pub(super) shape: Shape,
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
            private_values: Vec::new(),
            constraints: Vec::new(),
        })
    }

    pub(super) fn public(&self, index: usize) -> Result<Variable, CircuitError> {
        (index < self.public_inputs.len())
            .then_some(Variable::Public(index))
            .ok_or(CircuitError::InvalidDimension)
    }

    pub(super) fn alloc(&mut self, value: Scalar) -> Result<Variable, CircuitError> {
        if self.private_values.len() >= MAX_CIRCUIT_ROWS {
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
        if self.constraints.len() >= MAX_CIRCUIT_ROWS {
            return Err(CircuitError::InvalidDimension);
        }
        self.constraints.push(Constraint { a, b, c });
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
                    Variable::Private(index) => self.private_values[*index],
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

    pub(super) fn finalize(mut self) -> Result<CircuitAssignment, CircuitError> {
        if self.private_values.is_empty() || self.constraints.is_empty() {
            return Err(CircuitError::InvalidDimension);
        }
        let variable_count = self
            .private_values
            .len()
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        let constraint_count = self
            .constraints
            .len()
            .checked_next_power_of_two()
            .ok_or(CircuitError::InvalidDimension)?;
        if variable_count > MAX_CIRCUIT_ROWS || constraint_count > MAX_CIRCUIT_ROWS {
            return Err(CircuitError::InvalidDimension);
        }
        self.private_values.resize(variable_count, Scalar::zero());
        self.constraints.resize(
            constraint_count,
            Constraint {
                a: LinearCombination::zero(),
                b: LinearCombination::zero(),
                c: LinearCombination::zero(),
            },
        );

        let columns = variable_count
            .checked_add(1)
            .and_then(|value| value.checked_add(self.public_inputs.len()))
            .ok_or(CircuitError::InvalidDimension)?;
        let mut a_entries = Vec::new();
        let mut b_entries = Vec::new();
        let mut c_entries = Vec::new();
        for (row, constraint) in self.constraints.iter().enumerate() {
            append_matrix_entries(row, variable_count, &constraint.a, &mut a_entries)?;
            append_matrix_entries(row, variable_count, &constraint.b, &mut b_entries)?;
            append_matrix_entries(row, variable_count, &constraint.c, &mut c_entries)?;
        }
        let a = SparseMatrix::new(constraint_count, columns, &a_entries)?;
        let b = SparseMatrix::new(constraint_count, columns, &b_entries)?;
        let c = SparseMatrix::new(constraint_count, columns, &c_entries)?;
        let shape = Shape::new(
            constraint_count,
            variable_count,
            self.public_inputs.len(),
            a,
            b,
            c,
        )?;
        Ok(CircuitAssignment {
            shape,
            witness: self.private_values,
            public_inputs: self.public_inputs,
        })
    }
}

fn append_matrix_entries(
    row: usize,
    variable_count: usize,
    combination: &LinearCombination,
    output: &mut Vec<(usize, usize, Scalar)>,
) -> Result<(), CircuitError> {
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
        row_entries.push((row, column, coefficient));
    }
    row_entries.sort_by_key(|(_, column, _)| *column);
    output.extend(row_entries);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
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
            .validate_relaxed_assignment(
                &assignment.witness,
                Scalar::one(),
                &assignment.public_inputs,
                &vec![Scalar::zero(); assignment.shape.constraint_count()],
            )
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
            .validate_relaxed_assignment(
                &assignment.witness,
                Scalar::one(),
                &assignment.public_inputs,
                &vec![Scalar::zero(); assignment.shape.constraint_count()],
            )
            .expect("satisfying assignment");
    }
}
