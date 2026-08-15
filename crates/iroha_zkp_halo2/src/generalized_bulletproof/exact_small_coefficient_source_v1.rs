//! Closed verifier-side constraint source for exact small coefficients.

use super::{
    ArithmeticCircuitStatement, GeneralizedBulletproofErrorV1, ProofGeneratorView, ProofScalar,
    ProofSuite, ScalarVector, SecretScalar, VerifierConstraintSourceV1, VerifierTranscript,
};

/// Exact coefficient set represented by the closed constraint source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactSmallCoefficientBoundV1 {
    /// Coefficients are in `{-1, 0, 1}`.
    One,
    /// Coefficients are in `{-2, -1, 0, 1, 2}`.
    Two,
}

impl ExactSmallCoefficientBoundV1 {
    const fn gates_per_coefficient(self) -> usize {
        match self {
            Self::One => 2,
            Self::Two => 3,
        }
    }

    const fn constraints_per_coefficient(self) -> usize {
        match self {
            Self::One => 5,
            Self::Two => 7,
        }
    }
}

/// Checked, field-private description of one canonical exact-small circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactSmallCoefficientConstraintSourceV1 {
    coefficient_count: usize,
    padded_gates: usize,
    constraint_count: usize,
    bound: ExactSmallCoefficientBoundV1,
}

impl ExactSmallCoefficientConstraintSourceV1 {
    /// Construct the unique canonical circuit shape for `coefficient_count`.
    pub(crate) fn new(
        coefficient_count: usize,
        bound: ExactSmallCoefficientBoundV1,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        if coefficient_count == 0 {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let actual_gates = coefficient_count
            .checked_mul(bound.gates_per_coefficient())
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let padded_gates = actual_gates
            .checked_next_power_of_two()
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let visible_constraints = coefficient_count
            .checked_mul(bound.constraints_per_coefficient())
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let tail_constraints = padded_gates
            .checked_sub(coefficient_count)
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        let constraint_count = visible_constraints
            .checked_add(tail_constraints)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        Ok(Self {
            coefficient_count,
            padded_gates,
            constraint_count,
            bound,
        })
    }

    fn validate_statement_shape(
        self,
        generator_count: usize,
        vector_commitment_count: usize,
        scalar_commitment_count: usize,
    ) -> Result<(), GeneralizedBulletproofErrorV1> {
        if generator_count != self.padded_gates
            || vector_commitment_count != 1
            || scalar_commitment_count != 0
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        Ok(())
    }

    /// Aggregate every canonical row once under consecutive powers of `z_one`.
    pub(super) fn aggregate<F: ProofScalar>(
        self,
        z_one: F,
    ) -> Result<ExactSmallCoefficientAggregatesV1<F>, GeneralizedBulletproofErrorV1> {
        let mut running = RunningExactSmallCoefficientAggregateV1::new(self.padded_gates, z_one);
        for coefficient_index in 0..self.coefficient_count {
            let first_gate = coefficient_index
                .checked_mul(self.bound.gates_per_coefficient())
                .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
            running.emit_boolean_gate(first_gate)?;
            running.emit_boolean_gate(
                first_gate
                    .checked_add(1)
                    .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?,
            )?;
            match self.bound {
                ExactSmallCoefficientBoundV1::One => {
                    running.add_l(first_gate, F::ONE)?;
                    running.add_l(first_gate + 1, -F::ONE)?;
                }
                ExactSmallCoefficientBoundV1::Two => {
                    running.emit_boolean_gate(
                        first_gate
                            .checked_add(2)
                            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?,
                    )?;
                    running.add_l(first_gate, F::ONE)?;
                    running.add_l(first_gate + 1, F::ONE)?;
                    running.add_l(first_gate + 2, -F::from_u64(2))?;
                }
            }
            running.add_cg(coefficient_index, -F::ONE)?;
            running.finish_row()?;
        }
        for padded_index in self.coefficient_count..self.padded_gates {
            running.add_cg(padded_index, F::ONE)?;
            running.finish_row()?;
        }
        running.finish(self.constraint_count)
    }

    #[cfg(test)]
    /// Return the checked padded-gate and constraint counts to unit tests.
    pub(super) const fn test_shape(self) -> (usize, usize) {
        (self.padded_gates, self.constraint_count)
    }
}

/// Validated verifier statement which cannot select an arbitrary row source.
pub(crate) struct ExactSmallCoefficientVerifierStatementV1<'a, S: ProofSuite> {
    statement: ArithmeticCircuitStatement<'a, S>,
    source: ExactSmallCoefficientConstraintSourceV1,
}

impl<'a, S: ProofSuite> ExactSmallCoefficientVerifierStatementV1<'a, S> {
    /// Validate the canonical basis, sole commitment, and exact source shape.
    pub(crate) fn new(
        generators: ProofGeneratorView<'a, S>,
        source: ExactSmallCoefficientConstraintSourceV1,
        vector_commitment: S::Point,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let statement = ArithmeticCircuitStatement::new(
            generators,
            Vec::new(),
            vec![vector_commitment],
            Vec::new(),
        )?;
        source.validate_statement_shape(
            statement.generators.g_bold.len(),
            statement.vector_commitments.len(),
            statement.scalar_commitments.len(),
        )?;
        Ok(Self { statement, source })
    }

    /// Consume the statement through the shared generalized verifier core.
    pub(crate) fn verify<T>(self, transcript: &mut T) -> Result<(), GeneralizedBulletproofErrorV1>
    where
        T: VerifierTranscript<S>,
    {
        self.statement.verify_with_constraint_source(
            VerifierConstraintSourceV1::ExactSmallCoefficient(self.source),
            transcript,
        )
    }
}

/// Owned verifier weights produced by one complete canonical row pass.
pub(super) struct ExactSmallCoefficientAggregatesV1<F: ProofScalar> {
    /// Aggregate left-wire weights.
    pub(super) l_weights: ScalarVector<F>,
    /// Aggregate right-wire weights.
    pub(super) r_weights: ScalarVector<F>,
    /// Aggregate output-wire weights.
    pub(super) o_weights: ScalarVector<F>,
    /// Aggregate weights for the sole vector commitment.
    pub(super) vector_commitment_weights: ScalarVector<F>,
    /// Empty scalar-commitment weight vector fixed by this source.
    pub(super) scalar_commitment_weights: ScalarVector<F>,
    /// Zero constant product fixed by this source.
    pub(super) constraint_product: SecretScalar<F>,
}

struct RunningExactSmallCoefficientAggregateV1<F: ProofScalar> {
    aggregates: ExactSmallCoefficientAggregatesV1<F>,
    z_one: F,
    running_z: F,
    emitted_rows: usize,
}

impl<F: ProofScalar> RunningExactSmallCoefficientAggregateV1<F> {
    fn new(padded_gates: usize, z_one: F) -> Self {
        Self {
            aggregates: ExactSmallCoefficientAggregatesV1 {
                l_weights: ScalarVector::zero(padded_gates),
                r_weights: ScalarVector::zero(padded_gates),
                o_weights: ScalarVector::zero(padded_gates),
                vector_commitment_weights: ScalarVector::zero(padded_gates),
                scalar_commitment_weights: ScalarVector::zero(0),
                constraint_product: SecretScalar::new(F::ZERO),
            },
            z_one,
            running_z: z_one,
            emitted_rows: 0,
        }
    }

    fn add_l(&mut self, index: usize, coefficient: F) -> Result<(), GeneralizedBulletproofErrorV1> {
        let value = self
            .aggregates
            .l_weights
            .0
            .get_mut(index)
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        *value += coefficient * self.running_z;
        Ok(())
    }

    fn add_r(&mut self, index: usize, coefficient: F) -> Result<(), GeneralizedBulletproofErrorV1> {
        let value = self
            .aggregates
            .r_weights
            .0
            .get_mut(index)
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        *value += coefficient * self.running_z;
        Ok(())
    }

    fn add_o(&mut self, index: usize, coefficient: F) -> Result<(), GeneralizedBulletproofErrorV1> {
        let value = self
            .aggregates
            .o_weights
            .0
            .get_mut(index)
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        *value += coefficient * self.running_z;
        Ok(())
    }

    fn add_cg(
        &mut self,
        index: usize,
        coefficient: F,
    ) -> Result<(), GeneralizedBulletproofErrorV1> {
        let value = self
            .aggregates
            .vector_commitment_weights
            .0
            .get_mut(index)
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        *value += coefficient * self.running_z;
        Ok(())
    }

    fn emit_boolean_gate(&mut self, gate: usize) -> Result<(), GeneralizedBulletproofErrorV1> {
        self.add_l(gate, F::ONE)?;
        self.add_r(gate, -F::ONE)?;
        self.finish_row()?;
        self.add_o(gate, F::ONE)?;
        self.add_l(gate, -F::ONE)?;
        self.finish_row()
    }

    fn finish_row(&mut self) -> Result<(), GeneralizedBulletproofErrorV1> {
        self.emitted_rows = self
            .emitted_rows
            .checked_add(1)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        self.running_z *= self.z_one;
        Ok(())
    }

    fn finish(
        self,
        expected_rows: usize,
    ) -> Result<ExactSmallCoefficientAggregatesV1<F>, GeneralizedBulletproofErrorV1> {
        if self.emitted_rows != expected_rows {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        Ok(self.aggregates)
    }
}
