//! Checked four-lane polynomial transforms on an explicitly supplied Fp4 coset.
//!
//! The existing cyclotomic FFT remains the arithmetic owner. This adapter checks
//! dimensions/coefficients and applies full Fp4 coefficient twists; it never
//! projects points. The larger DEEP domain has an explicit constructor taking
//! its independently validated geometry, not modified replay parameters.

use fastpq_isi::FASTPQ_FINAL_V1;

use super::{
    FriDomain, field_pow, fixed_domain::FixedTraceDomain, polynomial_field::PolynomialField,
    secret_polynomial::SecretPolynomial,
};
use crate::{Error, Result, cyclotomic, field::GoldilocksFp4V1 as F};

/// Exact source-root subgroup and an explicit nonzero quartic-extension offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PolynomialDomain {
    rows: usize,
    generator: u64,
    offset: F,
    inverse_offset: F,
    workspace_bytes: usize,
}

impl PolynomialDomain {
    /// Bind the exact larger DEEP coset to the same checked four-lane FFT owner.
    ///
    /// This entry takes a validated fixed geometry, never arbitrary root/order
    /// metadata. The caller's cap is checked without allocating the 8M lanes.
    pub(super) fn for_deep(
        geometry: &super::deep_geometry::DeepGeometry,
        max_workspace_bytes: usize,
    ) -> Result<Self> {
        let rows = super::deep_geometry::LDE_ROWS;
        let workspace_bytes = rows
            .checked_mul(2 * core::mem::size_of::<F>())
            .ok_or_else(|| invalid("DEEP transform payload size overflow"))?;
        limit(
            "max_polynomial_transform_bytes",
            workspace_bytes,
            max_workspace_bytes,
        )?;
        limit(
            "max_polynomial_transform_addressable_bytes",
            workspace_bytes,
            isize::MAX as usize,
        )?;
        let domain = geometry.domain();
        let offset = F::embed_base(domain.offset);
        let inverse_offset = offset
            .inverse()
            .ok_or_else(|| invalid("DEEP transform requires a nonzero fixed offset"))?;
        Ok(Self {
            rows,
            generator: domain.generator,
            offset,
            inverse_offset,
            workspace_bytes,
        })
    }

    /// Validate geometry and the maximum simultaneous lane/result payloads.
    ///
    /// Subgroup transforms use the explicit offset one. A numerator caller must
    /// separately require a coset disjoint from its execution subgroup.
    pub(super) fn new(
        rows: usize,
        offset: F,
        max_rows: usize,
        max_workspace_bytes: usize,
    ) -> Result<Self> {
        if !rows.is_power_of_two() || rows.ilog2() > FASTPQ_FINAL_V1.lde_log_size {
            return Err(invalid(
                "polynomial transform needs an existing source-root subgroup",
            ));
        }
        limit("max_polynomial_transform_rows", rows, max_rows)?;
        let workspace_bytes = rows
            .checked_mul(2 * core::mem::size_of::<F>())
            .ok_or_else(|| invalid("polynomial transform payload size overflow"))?;
        limit(
            "max_polynomial_transform_bytes",
            workspace_bytes,
            max_workspace_bytes,
        )?;
        limit(
            "max_polynomial_transform_addressable_bytes",
            workspace_bytes,
            isize::MAX as usize,
        )?;
        offset.validate("polynomial_transform_offset", &[])?;
        let inverse_offset = offset
            .inverse()
            .ok_or_else(|| invalid("polynomial transform offset must be nonzero"))?;
        // Reuse the canonical root/order/coherence checks before deriving a
        // different *subgroup* of that same existing root. No params are changed.
        FixedTraceDomain::new(&FASTPQ_FINAL_V1, 1)?;
        let domain = FriDomain::from_lde_parameters(
            FASTPQ_FINAL_V1.lde_root,
            FASTPQ_FINAL_V1.lde_log_size,
            rows,
            1,
        )?;
        Ok(Self {
            rows,
            generator: domain.generator,
            offset,
            inverse_offset,
            workspace_bytes,
        })
    }

    /// Exact transform size, independent of a coefficient degree declaration.
    pub(super) const fn rows(self) -> usize {
        self.rows
    }

    /// Exact canonical base generator of this domain.
    pub(super) const fn generator(self) -> u64 {
        self.generator
    }

    /// Conservative simultaneous lane/result payload for one conversion.
    #[cfg(test)]
    pub(super) const fn workspace_bytes(self) -> usize {
        self.workspace_bytes
    }

    /// Checked current/next relation for a disjoint numerator coset.
    pub(super) fn numerator_rotation(self, trace_rows: usize) -> Result<usize> {
        if !trace_rows.is_power_of_two() || !self.rows.is_multiple_of(trace_rows) {
            return Err(invalid(
                "numerator domain must contain the exact trace subgroup",
            ));
        }
        let expected = FixedTraceDomain::new(&FASTPQ_FINAL_V1, trace_rows)?.generator;
        let stride = self.rows / trace_rows;
        if field_pow(self.generator, stride as u64) != expected {
            return Err(invalid(
                "numerator domain has an inconsistent trace rotation",
            ));
        }
        if self.offset.power(self.rows as u64) == F::ONE {
            return Err(invalid(
                "numerator interpolation coset intersects the trace subgroup",
            ));
        }
        Ok(stride)
    }

    /// Full Fp4 point at an exact bounded natural-order index.
    pub(super) fn point(self, index: usize) -> Result<F> {
        if index >= self.rows {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.rows,
            });
        }
        Ok(self
            .offset
            .mul_base(field_pow(self.generator, index as u64)))
    }

    /// Evaluate full coefficients, after canonicality and degree-padding checks.
    pub(super) fn evaluate(
        self,
        coefficients: &[F],
        degree_bound: usize,
    ) -> Result<PolynomialLanes> {
        if coefficients.is_empty()
            || coefficients.len() > self.rows
            || degree_bound > coefficients.len()
        {
            return Err(invalid(
                "polynomial coefficients exceed their exact extent or transform",
            ));
        }
        validate_coefficients(
            coefficients,
            degree_bound,
            "polynomial_transform_coefficients",
        )?;
        let mut lanes = PolynomialLanes::zeroed(self)?;
        let mut twist = F::ONE;
        for (index, &coefficient) in coefficients.iter().enumerate() {
            let value = coefficient.mul(twist).coefficients();
            for (lane, &word) in lanes.values.iter_mut().zip(&value) {
                lane[index] = word;
            }
            twist = twist.mul(self.offset);
        }
        for lane in &mut lanes.values {
            cyclotomic::fft(lane, self.cyclotomic());
        }
        Ok(lanes)
    }

    /// Interpolate exact canonical values; the returned coefficients include full padding.
    pub(super) fn interpolate(self, values: &[F]) -> Result<SecretPolynomial<F>> {
        if values.len() != self.rows {
            return Err(invalid(
                "polynomial interpolation requires its exact domain extent",
            ));
        }
        for (index, &value) in values.iter().enumerate() {
            value.validate("polynomial_transform_evaluations", &[index])?;
        }
        let mut lanes = PolynomialLanes::zeroed(self)?;
        for (index, value) in values.iter().enumerate() {
            for (lane, &word) in lanes.values.iter_mut().zip(&value.coefficients()) {
                lane[index] = word;
            }
        }
        self.interpolate_lanes(lanes)
    }

    /// Inverse FFT and full Fp4 untwist of a privately constructed lane matrix.
    pub(super) fn interpolate_lanes(
        self,
        mut lanes: PolynomialLanes,
    ) -> Result<SecretPolynomial<F>> {
        if lanes.domain != self {
            return Err(invalid(
                "polynomial lanes belong to a different exact transform domain",
            ));
        }
        for lane in &mut lanes.values {
            cyclotomic::ifft(lane, self.cyclotomic());
        }
        let mut coefficients = SecretPolynomial::zeroed(self.rows)?;
        let mut untwist = F::ONE;
        for index in 0..self.rows {
            coefficients[index] = lanes.value(index)?.mul(untwist);
            untwist = untwist.mul(self.inverse_offset);
        }
        Ok(coefficients)
    }

    fn cyclotomic(self) -> cyclotomic::Domain {
        cyclotomic::Domain {
            log_size: self.rows.ilog2(),
            generator: self.generator,
        }
    }
}

/// Canonical four-lane values with no duplicate Fp4 evaluation matrix.
pub(super) struct PolynomialLanes {
    domain: PolynomialDomain,
    values: [SecretPolynomial<u64>; 4],
}

impl PolynomialLanes {
    fn zeroed(domain: PolynomialDomain) -> Result<Self> {
        let rows = domain.rows;
        let values = [
            SecretPolynomial::zeroed(rows)?,
            SecretPolynomial::zeroed(rows)?,
            SecretPolynomial::zeroed(rows)?,
            SecretPolynomial::zeroed(rows)?,
        ];
        Ok(Self { domain, values })
    }

    /// Recover the complete canonical Fp4 value at a checked index.
    pub(super) fn value(&self, index: usize) -> Result<F> {
        if index >= self.domain.rows {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.domain.rows,
            });
        }
        Ok(
            F::new(core::array::from_fn(|lane| self.values[lane][index]))
                .expect("existing base-field FFT outputs are canonical"),
        )
    }
}

/// Validate full coefficients before interpreting their declared zero padding.
pub(super) fn validate_coefficients(
    values: &[F],
    degree_bound: usize,
    context: &'static str,
) -> Result<()> {
    if degree_bound > values.len() {
        return Err(invalid(
            "polynomial degree bound exceeds its exact coefficient extent",
        ));
    }
    for (index, &value) in values.iter().enumerate() {
        value.validate(context, &[index])?;
        if index >= degree_bound && value != F::ZERO {
            return Err(invalid(
                "polynomial coefficient above the declared degree is nonzero",
            ));
        }
    }
    Ok(())
}

/// Reserve a checked public-data or owner-handle vector before population.
/// Private field contents use SecretPolynomial instead of a growable Vec.
pub(super) fn reserved<T>(length: usize) -> Result<Vec<T>> {
    let bytes = length
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| invalid("polynomial buffer byte size overflow"))?;
    limit(
        "max_polynomial_addressable_buffer_bytes",
        bytes,
        isize::MAX as usize,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| invalid("polynomial buffer allocation failed"))?;
    Ok(values)
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
#[path = "polynomial_transform/tests.rs"]
mod tests;
