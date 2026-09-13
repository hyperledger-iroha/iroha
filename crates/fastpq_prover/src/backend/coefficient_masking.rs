//! Checked coefficient transformation by an explicitly supplied vanishing mask.
//!
//! This computes C'(X) = C(X) + (X^N - 1) M(X), with no reduction modulo
//! X^N - 1. All dimensions, exclusive degree bounds and resource limits come
//! from the caller. Input coefficients are borrowed and remain unchanged.
//!
//! TODO: The final protocol must supply authenticated geometry, independently
//! sampled private masks, their distribution/entropy and all disclosure/degree
//! obligations. This algebra supplies no randomness, witness-hiding guarantee,
//! proof acceptance, profile registration or production parameter choice.

use core::{marker::PhantomData, mem::size_of};

use super::{GOLDILOCKS_MODULUS, polynomial_field::PolynomialField};
use crate::{Error, Result};

/// Explicit trusted polynomial dimensions; bounds are exclusive degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MaskingShape {
    /// Exact trace coefficient extent N and order of the vanishing subgroup.
    pub(super) trace_coefficients: usize,
    /// Coefficients at or above this index must be zero; lies in 1..=N.
    pub(super) trace_degree_bound: usize,
    /// Exact supplied mask coefficient extent, including declared zero padding.
    pub(super) mask_coefficients: usize,
    /// Mask coefficients at or above this index must be zero.
    pub(super) mask_degree_bound: usize,
}

/// Explicit resource policy without any production default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MaskingLimits {
    /// Maximum exact trace coefficient extent.
    pub(super) max_trace_coefficients: usize,
    /// Maximum exact private mask coefficient extent.
    pub(super) max_mask_coefficients: usize,
    /// Maximum exact output coefficient extent N + mask_coefficients.
    pub(super) max_output_coefficients: usize,
    /// Maximum output element payload bytes; excludes allocator bookkeeping.
    pub(super) max_output_bytes: usize,
    /// Maximum declared coordinate inspections and field-element operations.
    ///
    /// This bounds canonical-coordinate inspections, padding comparisons, output
    /// initialization/copies and two field updates per supplied mask coefficient.
    /// It is not a machine instruction, timing or peak RSS measurement.
    pub(super) max_work_units: usize,
}

/// Validated immutable dimensions and work bound for one canonical field type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MaskingPlan<F> {
    shape: MaskingShape,
    output_coefficients: usize,
    masked_degree_bound: usize,
    work_units: usize,
    field: PhantomData<F>,
}

impl<F: PolynomialField> MaskingPlan<F> {
    /// Check all trusted dimensions and policy limits without reading coefficients.
    ///
    /// N must describe a power-of-two subgroup of the existing base field. Mask
    /// extent may exceed N: overlap is handled by the same coefficient addition.
    /// No mask degree, nonzero leading coefficient or distribution is selected.
    pub(super) fn new(shape: MaskingShape, limits: MaskingLimits) -> Result<Self> {
        let n = shape.trace_coefficients;
        let n_u64 = u64::try_from(n).map_err(|_| invalid("masking trace extent exceeds u64"))?;
        if !n.is_power_of_two()
            || !(GOLDILOCKS_MODULUS - 1).is_multiple_of(n_u64)
            || shape.trace_degree_bound == 0
            || shape.trace_degree_bound > n
            || shape.mask_coefficients == 0
            || shape.mask_degree_bound == 0
            || shape.mask_degree_bound > shape.mask_coefficients
        {
            return Err(invalid(
                "masking requires exact subgroup and coefficient degree bounds",
            ));
        }
        let k = shape.mask_coefficients;
        let output_coefficients = checked_add(n, k)?;
        let masked_degree_bound = checked_add(n, shape.mask_degree_bound)?;
        let output_bytes = checked_mul(output_coefficients, size_of::<F>())?;
        let coordinate_inspections = checked_mul(output_coefficients, F::COEFFICIENTS)?;
        let work_units = checked_add(
            checked_add(coordinate_inspections, checked_mul(output_coefficients, 2)?)?,
            checked_add(n, checked_mul(k, 2)?)?,
        )?;
        limit(
            "max_masking_trace_coefficients",
            n,
            limits.max_trace_coefficients,
        )?;
        limit(
            "max_masking_mask_coefficients",
            k,
            limits.max_mask_coefficients,
        )?;
        limit(
            "max_masking_output_coefficients",
            output_coefficients,
            limits.max_output_coefficients,
        )?;
        limit(
            "max_masking_output_bytes",
            output_bytes,
            limits.max_output_bytes,
        )?;
        limit("max_masking_work_units", work_units, limits.max_work_units)?;
        // Vec allocations cannot exceed isize::MAX even when a larger caller cap exists.
        limit(
            "max_masking_addressable_output_bytes",
            output_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            shape,
            output_coefficients,
            masked_degree_bound,
            work_units,
            field: PhantomData,
        })
    }

    /// Exact returned coefficient extent, including retained high zero padding.
    pub(super) const fn output_coefficients(&self) -> usize {
        self.output_coefficients
    }

    /// Exclusive output degree bound; the leading allowed coefficient may be zero.
    pub(super) const fn masked_degree_bound(&self) -> usize {
        self.masked_degree_bound
    }

    /// Conservative declared work charged before any coefficient is read.
    pub(super) const fn work_units(&self) -> usize {
        self.work_units
    }

    /// Apply the exact full coefficient transform after complete input preflight.
    ///
    /// Explicit all-zero masks are valid algebra, and confer no hiding guarantee.
    /// Empty or omitted masks are never synthesized. Both coefficient arrays must
    /// have their exact declared extents and canonical zero padding. No output is
    /// allocated or field arithmetic performed until all input checks succeed.
    pub(super) fn apply(&self, trace: &[F], mask: &[F]) -> Result<Vec<F>> {
        self.validate_inputs(trace, mask)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.output_coefficients)
            .map_err(|_| invalid("masking output coefficient allocation failed"))?;
        output.resize(self.output_coefficients, F::ZERO);
        self.write_validated(trace, mask, &mut output);
        Ok(output)
    }

    /// Write into an exact caller-owned buffer after complete input preflight.
    ///
    /// The masked quotient owner supplies a fixed zeroizing allocation. Neither
    /// inputs nor output change on a preflight error; no second arithmetic path
    /// or caller-selected mask value is synthesized by this storage boundary.
    pub(super) fn apply_into(&self, trace: &[F], mask: &[F], output: &mut [F]) -> Result<()> {
        if output.len() != self.output_coefficients {
            return Err(invalid(
                "masking output differs from its exact declared extent",
            ));
        }
        self.validate_inputs(trace, mask)?;
        output.fill(F::ZERO);
        self.write_validated(trace, mask, output);
        Ok(())
    }

    fn validate_inputs(&self, trace: &[F], mask: &[F]) -> Result<()> {
        if trace.len() != self.shape.trace_coefficients
            || mask.len() != self.shape.mask_coefficients
        {
            return Err(invalid(
                "masking coefficient slices differ from the exact declared extents",
            ));
        }
        validate_coefficients(
            trace,
            self.shape.trace_degree_bound,
            "masking_trace_coefficient",
        )?;
        validate_coefficients(
            mask,
            self.shape.mask_degree_bound,
            "masking_mask_coefficient",
        )?;
        Ok(())
    }

    fn write_validated(&self, trace: &[F], mask: &[F], output: &mut [F]) {
        output[..trace.len()].copy_from_slice(trace);
        for (index, &coefficient) in mask.iter().enumerate() {
            output[index] = output[index].sub(coefficient);
            let high = self.shape.trace_coefficients + index;
            output[high] = output[high].add(coefficient);
        }
    }
}

fn validate_coefficients<F: PolynomialField>(
    coefficients: &[F],
    degree_bound: usize,
    context: &'static str,
) -> Result<()> {
    for (index, &coefficient) in coefficients.iter().enumerate() {
        coefficient.validate(context, &[index])?;
        if index >= degree_bound && coefficient != F::ZERO {
            return Err(invalid(
                "masking coefficient padding is not zero at the declared degree bound",
            ));
        }
    }
    Ok(())
}

fn checked_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invalid("masking coefficient dimension or work overflow"))
}

fn checked_mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("masking coefficient dimension or work overflow"))
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
#[path = "coefficient_masking/tests.rs"]
mod tests;
