//! Explicit telescoping randomization of a coefficient quotient split.
//!
//! Given Q = Q0 + X^s Q1 and caller-supplied T, return Q0 + X^s T and
//! Q1 - T. This is the two-chunk algebra of Haböck–Al Kindi, ePrint
//! 2024/1037, section 4.1; it also permits an unequal final chunk. All
//! coefficients remain caller-owned or in fixed zeroizing allocations.
//!
//! TODO: Prove the complete candidate's opening simulation, transcript/FRI
//! reduction, entropy sourcing and resource envelope before using this in a
//! producer. This validation-only owner chooses no mask length or randomness,
//! changes no proof bytes, and supplies no hiding or admission authority.

use super::{polynomial_field::PolynomialField, secret_polynomial::SecretPolynomial};
use crate::{Error, GoldilocksFp4V1 as F, Result};

/// Exact input extents and exclusive degree bounds supplied by the caller.
#[derive(Clone, Copy, Debug)]
pub(super) struct PairMaskingShape {
    /// Exponent of the existing coefficient split, not a new subgroup choice.
    pub(super) split: usize,
    /// Exact supplied quotient coefficient extent, including zero padding.
    pub(super) quotient_coefficients: usize,
    /// Coefficients at or above this index must be zero.
    pub(super) quotient_degree_bound: usize,
    /// Exact supplied independent mask extent, including zero padding.
    pub(super) mask_coefficients: usize,
    /// Mask coefficients at or above this index must be zero.
    pub(super) mask_degree_bound: usize,
}

/// Explicit bounded arithmetic policy with no production defaults.
#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::struct_field_names,
    reason = "every field is an inclusive cap and `max_` separates it from the measured \
              `PairMaskingPlan` payload bytes and work units that it bounds"
)]
pub(super) struct PairMaskingLimits {
    /// Maximum exclusive degree of either resulting chunk.
    pub(super) max_chunk_degree_bound: usize,
    /// Borrowed input plus both output payload bytes, excluding allocator metadata.
    pub(super) max_payload_bytes: usize,
    /// Conservative coordinate inspections, writes, field updates and erasure work.
    pub(super) max_work_units: usize,
}

/// Immutable arithmetic plan checked before inspecting or allocating private data.
pub(super) struct PairMaskingPlan {
    shape: PairMaskingShape,
    extents: [usize; 2],
    #[cfg(test)]
    degree_bounds: [usize; 2],
    payload_bytes: usize,
    work_units: usize,
}

impl PairMaskingPlan {
    /// Check all dimensions and simultaneous borrowed/owned payload accounting.
    pub(super) fn new(shape: PairMaskingShape, limits: PairMaskingLimits) -> Result<Self> {
        if shape.split == 0
            || shape.quotient_coefficients == 0
            || shape.quotient_degree_bound == 0
            || shape.quotient_degree_bound > shape.quotient_coefficients
            || shape.mask_coefficients == 0
            || shape.mask_degree_bound == 0
            || shape.mask_degree_bound > shape.mask_coefficients
        {
            return Err(invalid(
                "quotient pair requires explicit nonempty bounded coefficients",
            ));
        }
        let extents = [
            add(shape.split, shape.mask_coefficients)?,
            shape
                .quotient_coefficients
                .saturating_sub(shape.split)
                .max(shape.mask_coefficients),
        ];
        let degree_bounds = [
            add(shape.split, shape.mask_degree_bound)?,
            shape
                .quotient_degree_bound
                .saturating_sub(shape.split)
                .max(shape.mask_degree_bound),
        ];
        let inputs = add(shape.quotient_coefficients, shape.mask_coefficients)?;
        let outputs = add(extents[0], extents[1])?;
        let payload_bytes = mul(add(inputs, outputs)?, F::BYTES)?;
        // Four base-coordinate inspections + padding comparison for each input;
        // initialization and eventual four-coordinate erasure for each output;
        // one copy per quotient coefficient and two updates per mask coefficient.
        let work_units = add(
            mul(add(inputs, outputs)?, 5)?,
            add(
                shape.quotient_coefficients,
                mul(shape.mask_coefficients, 2)?,
            )?,
        )?;
        check(
            "max_quotient_pair_degree",
            degree_bounds[0].max(degree_bounds[1]),
            limits.max_chunk_degree_bound,
        )?;
        check(
            "max_quotient_pair_payload_bytes",
            payload_bytes,
            limits.max_payload_bytes,
        )?;
        check(
            "max_quotient_pair_work_units",
            work_units,
            limits.max_work_units,
        )?;
        check(
            "max_quotient_pair_addressable_bytes",
            payload_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            shape,
            extents,
            #[cfg(test)]
            degree_bounds,
            payload_bytes,
            work_units,
        })
    }

    /// Simultaneous declared input/output field payload; this is not measured RSS.
    pub(super) const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Preflight work charge, not a machine instruction or timing measurement.
    pub(super) const fn work_units(&self) -> usize {
        self.work_units
    }

    /// Exclusive degree bounds of the randomized low and high chunks.
    #[cfg(test)]
    pub(super) const fn degree_bounds(&self) -> [usize; 2] {
        self.degree_bounds
    }

    /// Validate all inputs before allocation; preserve the original full quotient.
    ///
    /// All-zero masks are valid algebra, not evidence of entropy or privacy.
    /// No input is truncated when the final chunk is longer than the split.
    pub(super) fn apply(&self, quotient: &[F], mask: &[F]) -> Result<[SecretPolynomial<F>; 2]> {
        if quotient.len() != self.shape.quotient_coefficients
            || mask.len() != self.shape.mask_coefficients
        {
            return Err(invalid("quotient pair inputs differ from declared extents"));
        }
        validate(
            quotient,
            self.shape.quotient_degree_bound,
            "quotient_pair_input",
        )?;
        validate(mask, self.shape.mask_degree_bound, "quotient_pair_mask")?;
        let mut low = SecretPolynomial::zeroed(self.extents[0])?;
        let mut high = SecretPolynomial::zeroed(self.extents[1])?;
        let split = self.shape.split.min(quotient.len());
        low[..split].copy_from_slice(&quotient[..split]);
        high[..quotient.len() - split].copy_from_slice(&quotient[split..]);
        for (index, &value) in mask.iter().enumerate() {
            low[self.shape.split + index] = value;
            high[index] = high[index].sub(value);
        }
        Ok([low, high])
    }
}

fn validate(values: &[F], bound: usize, context: &'static str) -> Result<()> {
    for (index, &value) in values.iter().enumerate() {
        value.validate(context, &[index])?;
        if index >= bound && value != F::ZERO {
            return Err(invalid("quotient pair has nonzero declared degree padding"));
        }
    }
    Ok(())
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("quotient pair dimension overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("quotient pair dimension overflow"))
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}
fn check(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded { limit, actual, max })
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "quotient_pair_masking/tests.rs"]
mod tests;
