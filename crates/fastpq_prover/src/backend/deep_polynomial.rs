//! Exact coefficient construction of the 606-component DEEP polynomial.
//!
//! Combine trace columns before the two linear divisions, then apply their
//! common `(1 + lambda X^2)` shift. Combine the quotient halves before their
//! linear division, then apply `(1 + lambda X)`. This takes linear coefficient
//! work and two N-cell allocations, with no per-LDE-point inversions or 606
//! materialized polynomials. Owned coefficient buffers use unconditional erasure;
//! borrowed inputs and incidental arithmetic copies remain caller-owned.
//!
//! The source borrows canonical coefficients of degree <N, and splits an exact
//! degree-<2N quotient at coefficient N. It neither computes nor authenticates
//! that AIR quotient. OOD answers are constructed before the caller supplies
//! lambda, but the caller owns transcript chronology and coefficient commitments.
//! TODO: Bind this unmasked arithmetic into the reviewed protocol transcript,
//! quotient and LDE owners before production admission.

#[cfg(test)]
use super::deep_composition::DeepComposition;
use super::{
    compact_public_columns::COMMITTED_COLUMN_COUNT, deep_composition::OodPair,
    polynomial_field::PolynomialField, polynomial_transform::validate_coefficients,
    secret_polynomial::SecretPolynomial,
};
use crate::{Error, Result, field::GoldilocksFp4V1 as F};

/// Exclusive degree bound and exact returned coefficient extent.
pub(super) const DEGREE_BOUND: usize = super::deep_geometry::TRACE_ROWS;
/// Simultaneous owned scratch and result payload; excludes borrowed inputs/OOD answers.
pub(super) const WORKSPACE_BYTES: usize = 2 * DEGREE_BOUND * core::mem::size_of::<F>();

/// Validated immutable source coefficients; absent trailing coefficients are zero.
#[derive(Clone, Copy)]
pub(super) struct DeepPolynomialSource<'a> {
    trace: &'a [&'a [u64]],
    quotient: &'a [F],
}

impl<'a> DeepPolynomialSource<'a> {
    /// Check the complete fixed width, degree extents and canonical coordinates.
    ///
    /// Empty slices denote zero. Extents over the exclusive bounds are rejected
    /// even when the excess coefficients are zero; no input is truncated.
    pub(super) fn new(trace: &'a [&'a [u64]], quotient: &'a [F]) -> Result<Self> {
        if trace.len() != COMMITTED_COLUMN_COUNT
            || trace.iter().any(|column| column.len() > DEGREE_BOUND)
            || quotient.len() > 2 * DEGREE_BOUND
        {
            return Err(invalid(
                "DEEP source requires 301 degree-<N columns and a degree-<2N quotient",
            ));
        }
        for (column, coefficients) in trace.iter().enumerate() {
            for (degree, &value) in coefficients.iter().enumerate() {
                value.validate("deep_polynomial_trace", &[column, degree])?;
            }
        }
        validate_coefficients(quotient, quotient.len(), "deep_polynomial_quotient")?;
        Ok(Self { trace, quotient })
    }

    /// Borrow Q0 and Q1 such that Q(X) = Q0(X) + X^N Q1(X), without copying.
    pub(super) fn quotient_halves(self) -> [&'a [F]; 2] {
        let (low, high) = self
            .quotient
            .split_at(self.quotient.len().min(DEGREE_BOUND));
        [low, high]
    }

    /// Construct full extension-field answers before the composition challenge.
    ///
    /// The checked pair retains the trace generator supplied by the geometry
    /// owner. This step does not establish commitment or AIR consistency.
    pub(super) fn prepare(self, points: OodPair) -> PreparedDeepPolynomial<'a> {
        let trace_answers = points.points().map(|point| {
            core::array::from_fn(|column| {
                self.trace[column]
                    .iter()
                    .rev()
                    .fold(F::ZERO, |value, &coefficient| {
                        value.mul(point).add(F::embed_base(coefficient))
                    })
            })
        });
        let quotient_answers = self.quotient_halves().map(|half| {
            half.iter().rev().fold(F::ZERO, |value, &coefficient| {
                value.mul(points.points()[0]).add(coefficient)
            })
        });
        PreparedDeepPolynomial {
            source: self,
            points,
            trace_answers,
            quotient_answers,
        }
    }
}

/// Source-derived OOD answers fixed before lambda; no mutation or external answer setter.
pub(super) struct PreparedDeepPolynomial<'a> {
    source: DeepPolynomialSource<'a>,
    points: OodPair,
    trace_answers: [[F; COMMITTED_COLUMN_COUNT]; 2],
    quotient_answers: [F; 2],
}

impl PreparedDeepPolynomial<'_> {
    /// Current and next trace answers, in the shared projection's exact order.
    pub(super) fn trace_answers(&self) -> &[[F; COMMITTED_COLUMN_COUNT]; 2] {
        &self.trace_answers
    }

    /// OOD answers for Q0 and Q1 at the first point.
    pub(super) fn quotient_answers(&self) -> &[F; 2] {
        &self.quotient_answers
    }

    /// Prepare the independently implemented bounded point evaluator.
    #[cfg(test)]
    pub(super) fn evaluator(&self) -> Result<DeepComposition> {
        DeepComposition::new(
            self.points,
            &self.trace_answers[0],
            &self.trace_answers[1],
            &self.quotient_answers,
        )
    }

    /// Build degree-<N coefficients after every exact division remainder is zero.
    ///
    /// The workspace cap covers simultaneous owned coefficient allocations, not
    /// borrowed source storage or the separately bounded OOD answer arrays. The
    /// returned N-cell polynomial can feed the existing polynomial transform;
    /// this method performs no LDE, transcript operation or proof activation.
    pub(super) fn compose(&self, lambda: F, max_workspace_bytes: usize) -> Result<DeepPolynomial> {
        lambda.validate("deep_polynomial_challenge", &[])?;
        if WORKSPACE_BYTES > max_workspace_bytes {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_deep_polynomial_workspace_bytes",
                actual: WORKSPACE_BYTES,
                max: max_workspace_bytes,
            });
        }
        let mut scratch = SecretPolynomial::<F>::zeroed(DEGREE_BOUND)?;
        let mut coefficients = SecretPolynomial::<F>::zeroed(DEGREE_BOUND)?;
        let step = lambda.mul(lambda);
        let mut weight = F::ONE;
        let mut answers = [F::ZERO; 2];
        let mut extent = 2;
        for (column, source) in self.source.trace.iter().enumerate() {
            extent = extent.max(source.len());
            for (destination, &value) in scratch.iter_mut().zip(*source) {
                *destination = destination.add(weight.mul_base(value));
            }
            for (row, answer) in answers.iter_mut().enumerate() {
                *answer = answer.add(weight.mul(self.trace_answers[row][column]));
            }
            weight = weight.mul(step);
        }
        let interpolation = self.points.interpolate(answers[0], answers[1])?;
        let constant = interpolation.value_at(F::ZERO)?;
        let slope = interpolation.value_at(F::ONE)?.sub(constant);
        scratch[0] = scratch[0].sub(constant);
        scratch[1] = scratch[1].sub(slope);
        for point in self.points.points() {
            divide_linear_exact(&mut scratch[..extent], point)?;
        }
        validate_coefficients(
            &scratch[..extent],
            extent - 2,
            "deep_polynomial_trace_division",
        )?;
        for degree in 0..extent - 2 {
            coefficients[degree] = coefficients[degree].add(scratch[degree]);
            coefficients[degree + 2] = coefficients[degree + 2].add(lambda.mul(scratch[degree]));
        }

        scratch.fill(F::ZERO);
        extent = 1;
        let mut answer = F::ZERO;
        // weight is lambda^(2*301), so the halves start at powers 602 and 604.
        for (part, half) in self.source.quotient_halves().into_iter().enumerate() {
            extent = extent.max(half.len());
            for (destination, &value) in scratch.iter_mut().zip(half) {
                *destination = destination.add(weight.mul(value));
            }
            answer = answer.add(weight.mul(self.quotient_answers[part]));
            weight = weight.mul(step);
        }
        scratch[0] = scratch[0].sub(answer);
        divide_linear_exact(&mut scratch[..extent], self.points.points()[0])?;
        validate_coefficients(
            &scratch[..extent],
            extent - 1,
            "deep_polynomial_quotient_division",
        )?;
        for degree in 0..extent - 1 {
            coefficients[degree] = coefficients[degree].add(scratch[degree]);
            coefficients[degree + 1] = coefficients[degree + 1].add(lambda.mul(scratch[degree]));
        }
        validate_coefficients(&coefficients, DEGREE_BOUND, "deep_polynomial_result")?;
        Ok(DeepPolynomial { coefficients })
    }
}

/// Checked exact composition coefficients in the existing zeroizing storage owner.
pub(super) struct DeepPolynomial {
    coefficients: SecretPolynomial<F>,
}

impl DeepPolynomial {
    /// Borrow the full N-cell coefficient vector, including canonical zero padding.
    pub(super) fn coefficients(&self) -> &[F] {
        &self.coefficients
    }
}

// Canonical internal inputs. Descending synthetic division overwrites in place,
// leaves a zero high coefficient, and rejects the complete scalar remainder.
// VanishingDivisionPlan divides by X^N-1, so it cannot serve these linear factors.
fn divide_linear_exact(coefficients: &mut [F], point: F) -> Result<()> {
    let mut carry = F::ZERO;
    for coefficient in coefficients.iter_mut().rev() {
        let source = *coefficient;
        *coefficient = carry;
        carry = source.add(point.mul(carry));
    }
    if carry != F::ZERO {
        return Err(invalid("DEEP coefficient division has a nonzero remainder"));
    }
    Ok(())
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_polynomial/tests.rs"]
mod tests;
