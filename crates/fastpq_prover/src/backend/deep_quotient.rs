//! Unmasked 301-column input to the shared full-polynomial AIR quotient owner.
//!
//! Only committed base coefficients are caller inputs. All 41 omitted columns
//! are derived from the public-column owner's physical-period values, interpolated
//! on its 512-point subgroup, and lifted by X -> X^(N/512). The resulting complete
//! trace feeds the existing 923-slot evaluator on a disjoint 4N coset, then the
//! existing exact X^N-1 division. No AIR equations or zero-mask adapter are added.
//! The DEEP producer binds row/quotient commitments, transcript chronology and
//! the later 8M transforms. TODO: Qualify the integrated protocol before node
//! admission; a hiding construction remains outside this unmasked owner.

use super::{
    compact_public_columns::{
        COMMITTED_COLUMN_COUNT, PUBLIC_COLUMN_COUNT, PUBLIC_COLUMNS, PUBLIC_POLYNOMIAL_DEGREE,
        base_values,
    },
    compact_transfer_air::CompactTransferAir,
    deep_geometry::{COSET_OFFSET, TRACE_ROWS},
    masked_quotient::{
        MaskedQuotientLimits, MaskedQuotientPlan, checked_add, checked_mul, transform_work,
    },
    polynomial_field::PolynomialField,
    polynomial_transform::{PolynomialDomain, reserved, validate_coefficients},
    secret_polynomial::SecretPolynomial,
};
use crate::{
    Error, Result,
    field::GoldilocksFp4V1 as F,
    gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_HASH_ROWS, PhysicalRowIndex},
};

/// Exact interpolation extent for the source-derived unmasked numerator degree.
pub(super) const NUMERATOR_ROWS: usize = 4 * TRACE_ROWS;
/// Exact returned quotient coefficient extent; both N-cell halves are retained.
pub(super) const QUOTIENT_COEFFICIENTS: usize = 2 * TRACE_ROWS;

/// Explicit arithmetic limits, separate from proof-wire or admission limits.
#[derive(Clone, Copy, Debug)]
pub(super) struct DeepQuotientLimits {
    /// Simultaneous active-phase payload bytes, including that phase's input slices.
    pub(super) max_payload_bytes: usize,
    /// Structural arithmetic/inspection/cleanup work per preparation or quotient phase.
    pub(super) max_work_units: usize,
}

impl DeepQuotientLimits {
    fn shared(self) -> MaskedQuotientLimits {
        MaskedQuotientLimits {
            max_payload_bytes: self.max_payload_bytes,
            max_work_units: self.max_work_units,
            max_interpolation_rows: NUMERATOR_ROWS,
            #[cfg(test)]
            max_mask_coefficients: 0,
            #[cfg(test)]
            max_masked_coefficients: TRACE_ROWS,
        }
    }

    fn check(self, bytes: usize, work: usize) -> Result<()> {
        for (limit, actual, max) in [
            (
                "max_deep_quotient_preparation_bytes",
                bytes,
                self.max_payload_bytes,
            ),
            (
                "max_deep_quotient_preparation_work",
                work,
                self.max_work_units,
            ),
            (
                "max_deep_quotient_addressable_bytes",
                bytes,
                isize::MAX as usize,
            ),
        ] {
            if actual > max {
                return Err(Error::VerifierLimitExceeded { limit, actual, max });
            }
        }
        Ok(())
    }
}

/// Complete immutable coefficients with verifier-owned public columns.
///
/// Owned copies stay under the shared erasure guard. The caller may release its
/// base coefficient slices after preparation; this owner does not retain them.
pub(super) struct PreparedDeepTrace {
    columns: Vec<SecretPolynomial<F>>,
    degree_bounds: [usize; COLUMN_COUNT],
}

impl PreparedDeepTrace {
    /// Validate all 301 source columns and preparation resources before any FFT.
    ///
    /// Sources have degree <N; empty slices mean zero. Public columns cannot be
    /// supplied or overridden by a caller, including through the source width.
    pub(super) fn prepare(projected: &[&[u64]], limits: DeepQuotientLimits) -> Result<Self> {
        let (bytes, work) = preparation_cost(projected)?;
        limits.check(bytes, work)?;
        for (column, values) in projected.iter().enumerate() {
            for (degree, &value) in values.iter().enumerate() {
                value.validate("deep_quotient_source", &[column, degree])?;
            }
        }
        let period = PolynomialDomain::new(
            PHYSICAL_HASH_ROWS,
            F::ONE,
            PHYSICAL_HASH_ROWS,
            limits.max_payload_bytes,
        )?;
        let mut columns = reserved(COLUMN_COUNT)?;
        let mut degree_bounds = [1; COLUMN_COUNT];
        let mut public = 0;
        let mut committed = 0;
        for (reference, degree_bound) in degree_bounds.iter_mut().enumerate() {
            if public < PUBLIC_COLUMN_COUNT && PUBLIC_COLUMNS[public] == reference {
                columns.push(public_coefficients(public, period)?);
                *degree_bound = PUBLIC_POLYNOMIAL_DEGREE + 1;
                public += 1;
            } else {
                let input = projected[committed];
                let mut coefficients = SecretPolynomial::zeroed(input.len().max(1))?;
                for (destination, &value) in coefficients.iter_mut().zip(input) {
                    *destination = F::embed_base(value);
                }
                *degree_bound = coefficients.len();
                columns.push(coefficients);
                committed += 1;
            }
        }
        Ok(Self {
            columns,
            degree_bounds,
        })
    }

    /// Borrow a complete reference column without exposing mutable coefficients.
    #[cfg(test)]
    pub(super) fn column(&self, index: usize) -> Result<&[F]> {
        self.columns
            .get(index)
            .map(|values| &**values)
            .ok_or(Error::QueryIndexOutOfRange {
                index,
                len: COLUMN_COUNT,
            })
    }

    /// Exclusive checked input bounds passed to the unchanged AIR degree owner.
    #[cfg(test)]
    pub(super) fn degree_bounds(&self) -> &[usize; COLUMN_COUNT] {
        &self.degree_bounds
    }

    /// Preflight the complete 4N evaluation and quotient before transform allocation.
    pub(super) fn plan<'a>(
        &'a self,
        air: &'a CompactTransferAir,
        limits: DeepQuotientLimits,
    ) -> Result<MaskedQuotientPlan<'a>> {
        let columns: [&[F]; COLUMN_COUNT] = core::array::from_fn(|index| &*self.columns[index]);
        MaskedQuotientPlan::from_coefficients(
            air,
            &columns,
            &self.degree_bounds,
            NUMERATOR_ROWS,
            F::embed_base(COSET_OFFSET),
            QUOTIENT_COEFFICIENTS,
            limits.shared(),
        )
    }

    /// Evaluate all 923 weighted slots and return the exact guarded quotient.
    ///
    /// The full numerator is dropped after its conditional degree and zero
    /// remainder checks. No trace-point residual interpolation or truncation is
    /// used. Alpha must already be derived after the trace commitment.
    pub(super) fn build(
        &self,
        air: &CompactTransferAir,
        alpha: &[F],
        limits: DeepQuotientLimits,
    ) -> Result<SecretPolynomial<F>> {
        let result = self.plan(air, limits)?.build(alpha)?;
        if result.quotient().degree_bound() > QUOTIENT_COEFFICIENTS {
            return Err(invalid(
                "unmasked quotient exceeds the fixed two-half degree bound",
            ));
        }
        Ok(result.into_quotient().into_coefficients())
    }
}

fn preparation_cost(projected: &[&[u64]]) -> Result<(usize, usize)> {
    if projected.len() != COMMITTED_COLUMN_COUNT
        || projected.iter().any(|column| column.len() > TRACE_ROWS)
    {
        return Err(invalid(
            "unmasked quotient needs exactly 301 degree-<N base columns",
        ));
    }
    let mut input_cells = 0;
    let mut owned_cells = checked_mul(PUBLIC_COLUMN_COUNT, TRACE_ROWS)?;
    for column in projected {
        input_cells = checked_add(input_cells, column.len())?;
        owned_cells = checked_add(owned_cells, column.len().max(1))?;
    }
    // All retained complete coefficients plus one period's values, IFFT lanes
    // and result. Handles and allocator metadata are outside payload accounting.
    let cells = checked_add(owned_cells, checked_mul(3, PHYSICAL_HASH_ROWS)?)?;
    let bytes = checked_add(checked_mul(cells, F::BYTES)?, checked_mul(input_cells, 8)?)?;
    let work = checked_add(
        checked_add(input_cells, checked_mul(owned_cells, 16)?)?,
        checked_mul(PUBLIC_COLUMN_COUNT, transform_work(PHYSICAL_HASH_ROWS)?)?,
    )?;
    Ok((bytes, work))
}

fn public_coefficients(index: usize, period: PolynomialDomain) -> Result<SecretPolynomial<F>> {
    if index >= PUBLIC_COLUMN_COUNT || period.rows() != PHYSICAL_HASH_ROWS {
        return Err(invalid(
            "public coefficient interpolation needs its exact 512-period column",
        ));
    }
    let mut values = SecretPolynomial::zeroed(PHYSICAL_HASH_ROWS)?;
    for (phase, value) in values.iter_mut().enumerate() {
        let row = PhysicalRowIndex::new(phase).expect("fixed period is within the physical trace");
        *value = F::embed_base(base_values(row)[index]);
    }
    let periodic = period.interpolate(&values)?;
    let mut full = SecretPolynomial::zeroed(TRACE_ROWS)?;
    let stride = TRACE_ROWS / PHYSICAL_HASH_ROWS;
    for (degree, &value) in periodic.iter().enumerate() {
        full[degree * stride] = value;
    }
    validate_coefficients(
        &full,
        PUBLIC_POLYNOMIAL_DEGREE + 1,
        "deep_quotient_public_coefficients",
    )?;
    Ok(full)
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_quotient/tests.rs"]
pub(super) mod tests;
