//! Checked exact division of full Fp4 coefficients by the trace vanishing polynomial.
//!
//! This owner never replaces the numerator by its subgroup interpolant. Borrowed
//! input coefficients remain caller-owned and unchanged. Owned scratch and result
//! buffers are guarded by the unconditional zeroization owner. Caller inputs and
//! incidental arithmetic copies remain outside that owned-allocation guarantee.

use super::{polynomial_transform::validate_coefficients, secret_polynomial::SecretPolynomial};
use crate::{Error, Result, field::GoldilocksFp4V1 as F};

/// Validated exact coefficient extents and conditional full-polynomial degree bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VanishingDivisionPlan {
    trace_rows: usize,
    numerator_extent: usize,
    numerator_degree_bound: usize,
    quotient_extent: usize,
    quotient_degree_bound: usize,
    payload_bytes: usize,
    work_units: usize,
}

impl VanishingDivisionPlan {
    /// Check degree, extent and structural resource obligations before reading coefficients.
    pub(super) fn new(
        trace_rows: usize,
        numerator_extent: usize,
        numerator_degree_bound: usize,
        quotient_extent: usize,
        max_payload_bytes: usize,
        max_work_units: usize,
    ) -> Result<Self> {
        if trace_rows == 0
            || trace_rows > numerator_extent
            || numerator_degree_bound > numerator_extent
        {
            return Err(invalid(
                "vanishing division requires exact full numerator extents",
            ));
        }
        let quotient_degree_bound = numerator_degree_bound.saturating_sub(trace_rows);
        if quotient_extent < quotient_degree_bound {
            return Err(invalid(
                "quotient extent cannot hold the full conditional degree bound",
            ));
        }
        // Borrowed numerator, full remainder scratch and exact quotient result.
        let cells = numerator_extent
            .checked_mul(2)
            .and_then(|n| n.checked_add(quotient_extent))
            .ok_or_else(|| invalid("vanishing division coefficient count overflow"))?;
        let payload_bytes = cells
            .checked_mul(core::mem::size_of::<F>())
            .ok_or_else(|| invalid("vanishing division byte count overflow"))?;
        // Four-coordinate inspections; padding, copy and initialization; two
        // quotient/remainder updates per descending coefficient; final remainder.
        // Include four-coordinate erasure of owned remainder and quotient boxes
        // (the result may be dropped later by its caller).
        let work_units = numerator_extent
            .checked_mul(11)
            .and_then(|work| {
                quotient_extent
                    .checked_mul(10)
                    .and_then(|q| q.checked_add(work))
            })
            .and_then(|work| {
                quotient_degree_bound
                    .checked_mul(5)
                    .and_then(|steps| work.checked_add(steps))
            })
            .and_then(|work| work.checked_add(trace_rows))
            .ok_or_else(|| invalid("vanishing division work count overflow"))?;
        limit(
            "max_vanishing_division_payload_bytes",
            payload_bytes,
            max_payload_bytes,
        )?;
        limit(
            "max_vanishing_division_work_units",
            work_units,
            max_work_units,
        )?;
        limit(
            "max_vanishing_division_addressable_bytes",
            payload_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            trace_rows,
            numerator_extent,
            numerator_degree_bound,
            quotient_extent,
            quotient_degree_bound,
            payload_bytes,
            work_units,
        })
    }

    /// Exclusive quotient bound; no divisibility is established by constructing a plan.
    pub(super) const fn quotient_degree_bound(self) -> usize {
        self.quotient_degree_bound
    }

    /// Exact output extent including required zero padding.
    pub(super) const fn quotient_extent(self) -> usize {
        self.quotient_extent
    }

    /// Conservative payload bytes including the borrowed full numerator.
    pub(super) const fn payload_bytes(self) -> usize {
        self.payload_bytes
    }

    /// Declared structural work, distinct from time or machine instruction count.
    pub(super) const fn work_units(self) -> usize {
        self.work_units
    }

    /// Divide the actual polynomial and reject every nonzero remainder coefficient.
    ///
    /// The caller retains the supplied numerator. This owner allocates/drops the
    /// full remainder scratch and transfers the private quotient buffer on success.
    pub(super) fn divide(self, numerator: &[F]) -> Result<ExactQuotient> {
        if numerator.len() != self.numerator_extent {
            return Err(invalid(
                "vanishing division numerator has the wrong exact extent",
            ));
        }
        validate_coefficients(
            numerator,
            self.numerator_degree_bound,
            "vanishing_division_numerator",
        )?;
        let mut remainder = SecretPolynomial::from_slice(numerator)?;
        let mut coefficients = SecretPolynomial::zeroed(self.quotient_extent)?;
        for degree in (self.trace_rows..self.numerator_degree_bound).rev() {
            let coefficient = remainder[degree];
            coefficients[degree - self.trace_rows] = coefficient;
            remainder[degree] = F::ZERO;
            remainder[degree - self.trace_rows] =
                remainder[degree - self.trace_rows].add(coefficient);
        }
        if remainder[..self.trace_rows]
            .iter()
            .any(|&value| value != F::ZERO)
        {
            return Err(invalid(
                "full AIR numerator has a nonzero vanishing-polynomial remainder",
            ));
        }
        validate_coefficients(
            &coefficients,
            self.quotient_degree_bound,
            "vanishing_division_quotient",
        )?;
        Ok(ExactQuotient {
            coefficients,
            degree_bound: self.quotient_degree_bound,
        })
    }
}

/// Owned exact quotient; secret coefficient values are not formatted by Debug.
pub(super) struct ExactQuotient {
    coefficients: SecretPolynomial<F>,
    degree_bound: usize,
}

impl ExactQuotient {
    /// Borrow complete quotient coefficients including the caller-declared zero padding.
    pub(super) fn coefficients(&self) -> &[F] {
        &self.coefficients
    }

    /// Exclusive upper bound that passed full-input and zero-remainder checks.
    pub(super) const fn degree_bound(&self) -> usize {
        self.degree_bound
    }
}

fn limit(name: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded {
            limit: name,
            actual,
            max,
        })
    } else {
        Ok(())
    }
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "polynomial_division/tests.rs"]
mod tests;
