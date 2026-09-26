//! Checked physical 342-column trace to guarded 301-column base coefficients.
//!
//! Every physical row passes the canonical public-column projection owner before
//! any coefficient allocation or inverse FFT. Only the exact retained columns
//! are copied. Their natural-order subgroup values use the unchanged checked
//! trace generator and existing deterministic CPU IFFT, parallel by column.
//! This establishes shape/canonical projection, not AIR satisfaction or public
//! authority. Owned coefficient buffers erase on drop; borrowed inputs and
//! incidental arithmetic copies remain caller-owned. No witness is rebuilt.

use rayon::prelude::*;

use super::{
    compact_public_columns::{COMMITTED_COLUMN_COUNT, COMMITTED_COLUMNS, project_base_row},
    deep_geometry::{DeepGeometry, TRACE_ROWS},
    fixed_domain::FixedTraceDomain,
    masked_quotient::{checked_add, checked_mul, transform_work},
    polynomial_transform::reserved,
    secret_polynomial::SecretPolynomial,
};
use crate::{
    Error, Result, cyclotomic,
    gadgets::compact_smt_air::{COLUMN_COUNT, PhysicalRowIndex},
};

/// Explicit caller ceilings for physical projection and coefficient conversion.
#[derive(Clone, Copy, Debug)]
pub(super) struct DeepCoefficientLimits {
    /// Charged borrowed/owned array payload; excludes allocator/runtime overhead.
    pub(super) max_payload_bytes: usize,
    /// Conservative structural transform, inspection, copying and erasure work.
    pub(super) max_work_units: usize,
}

/// Fixed conversion charges available before physical witness expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CoefficientCharge {
    /// Input plus output arrays, ownership/view descriptors and fixed row scratch.
    pub(super) payload_bytes: usize,
    /// Conservative work accounting units, not elapsed time or CPU instructions.
    pub(super) work_units: usize,
}

/// Canonical degree-<N retained coefficients, with fixed width and erased storage.
pub(super) struct DeepTraceCoefficients {
    columns: [SecretPolynomial<u64>; COMMITTED_COLUMN_COUNT],
}

impl DeepTraceCoefficients {
    /// Count complete fixed input/output payloads before either can be allocated.
    ///
    /// Borrowed columns are charged at full distinct size even if they alias.
    /// Input views plus three retained-column descriptor arrays cover temporary
    /// ownership, fixed ownership and output views; owned field buffers never grow or leave the erasure guard.
    pub(super) fn required_resources() -> Result<CoefficientCharge> {
        let input_cells = checked_mul(COLUMN_COUNT, TRACE_ROWS)?;
        let output_cells = checked_mul(COMMITTED_COLUMN_COUNT, TRACE_ROWS)?;
        let cells = checked_add(input_cells, output_cells)?;
        let scratch = checked_add(COLUMN_COUNT, COMMITTED_COLUMN_COUNT)?;
        let descriptors = checked_add(
            checked_mul(COLUMN_COUNT, core::mem::size_of::<&[u64]>())?,
            checked_mul(
                3 * COMMITTED_COLUMN_COUNT,
                core::mem::size_of::<SecretPolynomial<u64>>(),
            )?,
        )?;
        let payload_bytes = checked_add(
            checked_mul(checked_add(cells, scratch)?, core::mem::size_of::<u64>())?,
            descriptors,
        )?;
        // Reuse the existing conservative four-lane transform allowance for
        // each cheaper single-lane IFFT. Add explicit whole-source validation
        // and output initialization/copy/inspection/erasure allowance.
        let work_units = checked_add(
            checked_add(checked_mul(input_cells, 8)?, checked_mul(output_cells, 16)?)?,
            checked_mul(COMMITTED_COLUMN_COUNT, transform_work(TRACE_ROWS)?)?,
        )?;
        Ok(CoefficientCharge {
            payload_bytes,
            work_units,
        })
    }

    /// Validate all 342×N physical cells, then project and interpolate exactly 301.
    ///
    /// This directly borrows the normal quantity producer's column layout.
    /// Width/length/resource checks precede the full canonical scan; the entire
    /// scan completes before any retained coefficient is allocated or transformed.
    pub(super) fn from_columns(
        columns: &[impl AsRef<[u64]>],
        limits: DeepCoefficientLimits,
    ) -> Result<Self> {
        if columns.len() != COLUMN_COUNT {
            return Err(invalid(
                "DEEP coefficient input requires exactly 342 columns",
            ));
        }
        let complete =
            core::array::from_fn::<_, COLUMN_COUNT, _>(|column| columns[column].as_ref());
        if complete.iter().any(|column| column.len() != TRACE_ROWS) {
            return Err(invalid(
                "DEEP coefficient input requires exactly 65536 physical rows",
            ));
        }
        let charge = Self::required_resources()?;
        check_limit(
            "max_deep_coefficient_payload_bytes",
            charge.payload_bytes,
            limits.max_payload_bytes,
        )?;
        check_limit(
            "max_deep_coefficient_work_units",
            charge.work_units,
            limits.max_work_units,
        )?;
        check_limit(
            "max_deep_coefficient_addressable_bytes",
            charge.payload_bytes,
            isize::MAX as usize,
        )?;
        let trace = FixedTraceDomain::new(&DeepGeometry::polynomial_parameters(), TRACE_ROWS)?;
        validate_complete_rows(&complete)?;
        let mut columns = copy_retained(&complete)?;
        interpolate_columns(
            &mut columns,
            cyclotomic::Domain {
                log_size: TRACE_ROWS.ilog2(),
                generator: trace.generator,
            },
        );
        Ok(Self { columns })
    }

    /// Borrow all fixed-width coefficient slices in canonical commitment order.
    /// The returned view has no ownership of, and cannot mutate, guarded storage.
    pub(super) fn coefficients(&self) -> [&[u64]; COMMITTED_COLUMN_COUNT] {
        core::array::from_fn(|column| &*self.columns[column])
    }
}

// Shape is checked before this full scan. Every cell, including omitted cells,
// reaches the existing canonical projector before owned output copies or FFTs.
fn validate_complete_rows(complete: &[&[u64]; COLUMN_COUNT]) -> Result<()> {
    for row in 0..TRACE_ROWS {
        let cells = core::array::from_fn::<_, COLUMN_COUNT, _>(|column| complete[column][row]);
        let index = PhysicalRowIndex::new(row).expect("fixed physical trace extent");
        project_base_row(index, &cells)?;
    }
    Ok(())
}

// The only production caller supplies a fully validated fixed matrix. Copy by
// retained column avoids a second private row transpose or a 342-column IFFT.
fn copy_retained(
    complete: &[&[u64]; COLUMN_COUNT],
) -> Result<[SecretPolynomial<u64>; COMMITTED_COLUMN_COUNT]> {
    let mut columns = reserved(COMMITTED_COLUMN_COUNT)?;
    for reference in COMMITTED_COLUMNS {
        columns.push(SecretPolynomial::from_slice(complete[reference])?);
    }
    columns
        .try_into()
        .map_err(|_| invalid("DEEP retained column count differs from its fixed map"))
}

// Same column-parallel deterministic cyclotomic IFFT as Planner::ifft_columns,
// applied directly to guarded slices without converting them to unguarded Vecs.
// The checked caller fixes every length/root before invoking this in-place kernel.
fn interpolate_columns(columns: &mut [SecretPolynomial<u64>], domain: cyclotomic::Domain) {
    columns
        .par_iter_mut()
        .for_each(|column| cyclotomic::ifft(column, domain));
}

fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        return Err(Error::VerifierLimitExceeded { limit, actual, max });
    }
    Ok(())
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_coefficients/tests.rs"]
mod tests;
