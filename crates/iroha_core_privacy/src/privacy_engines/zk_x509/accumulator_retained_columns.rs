//! Original masked CA coefficients retained across the joint commitment stages.
//!
//! No commitment-domain codeword is retained by this owner. Replay uses the
//! same coefficient vector and never resamples a trace mask. Explicit native,
//! coefficient and replay allocations clear on success, error and unwind.

use super::super::private_table::{PrivateTableV1, zeroize_fields_v1};
use super::*;
#[cfg(test)]
use crate::privacy_engines::transparent_stark::masked_trace_coefficients_on_coset_v1;
use crate::privacy_engines::transparent_stark::{
    goldilocks_fft_coarse_v1, masked_trace_coefficients_with_mask_v1, sample_trace_mask_v1,
};
use rand::TryRng;
use rayon::prelude::*;

/// The two original CA commitments have separate, fixed-width owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CaColumnFamilyV1 {
    /// Witness columns committed before shared bus challenges.
    Base,
    /// Bus and serialized-product columns committed after shared challenges.
    Auxiliary,
}
impl CaColumnFamilyV1 {
    /// Exact number of columns in this original commitment.
    pub(super) const fn width_v1(self) -> usize {
        match self {
            Self::Base => ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
            Self::Auxiliary => ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
        }
    }
}
fn clear_columns_v1(columns: &mut [Vec<F>]) {
    for column in columns {
        zeroize_fields_v1(column);
    }
}
/// Scoped private original coefficients; intentionally neither Copy nor Clone.
pub(super) struct CaOriginalMaskedColumnsV1 {
    family: CaColumnFamilyV1,
    columns: PrivateTableV1<Vec<F>>,
}
impl core::fmt::Debug for CaOriginalMaskedColumnsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CaOriginalMaskedColumnsV1")
            .field("family", &self.family)
            .field("coefficients", &"<redacted>")
            .finish_non_exhaustive()
    }
}
impl CaOriginalMaskedColumnsV1 {
    /// Exact coefficient and column-header forecast before private allocation.
    /// The containing phase accounts for this owner's inline storage separately.
    pub(super) fn payload_bound_v1(
        family: CaColumnFamilyV1,
    ) -> Result<usize, ZkX509CaAccumulatorProofErrorV1> {
        ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1
            .checked_add(CA_MASK_DEGREE_V1 + 1)
            .and_then(|count| count.checked_mul(core::mem::size_of::<F>()))
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<Vec<F>>()))
            .and_then(|bytes| bytes.checked_mul(family.width_v1()))
            .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)
    }

    /// Consume and clear native columns after sampling each original mask once.
    pub(super) fn sample_v1<R: TryRng>(
        family: CaColumnFamilyV1,
        native: Vec<Vec<F>>,
        rng: &mut R,
    ) -> Result<Self, ZkX509CaAccumulatorProofErrorV1> {
        // Adopt even malformed private input before the first fallible check.
        let native = PrivateTableV1::new(native, clear_columns_v1);
        let width = family.width_v1();
        let count = ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1
            .checked_add(CA_MASK_DEGREE_V1 + 1)
            .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)?;
        if native.len() != width
            || native.iter().any(|column| {
                column.len() != ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1
                    || column.iter().any(|value| F::canonical(value.0).is_none())
            })
        {
            return Err(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness);
        }
        let mut columns = PrivateTableV1::new(Vec::new(), clear_columns_v1);
        columns
            .try_reserve_exact(width)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
        if columns.capacity() != width {
            return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
        }
        for source in native.iter() {
            let mask = sample_trace_mask_v1(CA_MASK_DEGREE_V1, rng)
                .map_err(map_transparent_proof_error_v1)?;
            let coefficients = PrivateTableV1::new(
                masked_trace_coefficients_with_mask_v1(
                    source,
                    ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                    mask.coefficients(),
                )
                .map_err(map_transparent_proof_error_v1)?,
                zeroize_fields_v1,
            );
            if coefficients.len() != count || coefficients.capacity() != count {
                return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
            }
            columns.push(coefficients.into_vec());
        }
        let owner = Self { family, columns };
        if owner.allocated_payload_bytes_v1()? != Self::payload_bound_v1(family)? {
            return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
        }
        Ok(owner)
    }
    /// Fixed identity of this original base or auxiliary commitment.
    pub(super) const fn family_v1(&self) -> CaColumnFamilyV1 {
        self.family
    }
    /// Borrow one original coefficient vector without creating a secret copy.
    pub(super) fn column_v1(&self, index: usize) -> Result<&[F], ZkX509CaAccumulatorProofErrorV1> {
        self.columns
            .get(index)
            .map(Vec::as_slice)
            .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)
    }
    /// Exact allocated coefficient payload, including column-list capacity.
    pub(super) fn allocated_payload_bytes_v1(
        &self,
    ) -> Result<usize, ZkX509CaAccumulatorProofErrorV1> {
        self.columns.iter().try_fold(
            self.columns.capacity() * core::mem::size_of::<Vec<F>>(),
            |sum, column| {
                column
                    .capacity()
                    .checked_mul(core::mem::size_of::<F>())
                    .and_then(|bytes| sum.checked_add(bytes))
                    .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)
            },
        )
    }
    /// Replay the same original polynomial on its fixed local commitment domain.
    #[cfg(test)]
    pub(super) fn column_lde_v1(
        &self,
        index: usize,
    ) -> Result<PrivateTableV1<F>, ZkX509CaAccumulatorProofErrorV1> {
        let column = PrivateTableV1::new(
            masked_trace_coefficients_on_coset_v1(
                self.column_v1(index)?,
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
            )
            .map_err(map_transparent_proof_error_v1)?,
            zeroize_fields_v1,
        );
        if column.len() != 1 << ZK_X509_CA_FRI_LDE_LOG2_V1 || column.capacity() != column.len() {
            return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
        }
        Ok(column)
    }
    /// Materialize a scoped local codeword matrix; the returned guard clears it.
    /// The caller must charge this phase against simultaneous MAIN ownership.
    pub(super) fn local_lde_v1(
        &self,
    ) -> Result<PrivateTableV1<Vec<F>>, ZkX509CaAccumulatorProofErrorV1> {
        self.local_lde_with_v1(|_, coefficients, target, root, shift| {
            let mut power = F::ONE;
            for (target, coefficient) in target.iter_mut().zip(coefficients) {
                *target = coefficient.mul(power);
                power = power.mul(shift);
            }
            // The outer column tasks own parallelism. Each column keeps the
            // original radix-two arithmetic without per-stage worker joins.
            goldilocks_fft_coarse_v1(target, root).map_err(map_transparent_proof_error_v1)
        })
    }

    fn local_lde_with_v1(
        &self,
        fill: impl Fn(usize, &[F], &mut [F], F, F) -> Result<(), ZkX509CaAccumulatorProofErrorV1> + Sync,
    ) -> Result<PrivateTableV1<Vec<F>>, ZkX509CaAccumulatorProofErrorV1> {
        let width = self.family.width_v1();
        let rows = 1usize << ZK_X509_CA_FRI_LDE_LOG2_V1;
        let native_rows = 1usize << ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1;
        let root = goldilocks_primitive_root_v1(ZK_X509_CA_FRI_LDE_LOG2_V1)
            .map_err(map_transparent_proof_error_v1)?;
        let shift = F(GOLDILOCKS_GENERATOR_V1);
        if rows <= native_rows
            || shift.pow(native_rows as u128) == F::ONE
            || shift.pow(rows as u128) == F::ONE
        {
            return Err(map_transparent_proof_error_v1(
                TransparentStarkErrorV1::InvalidDomain,
            ));
        }
        // Preserve original per-column shape/canonicality error precedence
        // before scheduling any private output work or publishing a matrix.
        for index in 0..width {
            let coefficients = self.column_v1(index)?;
            if coefficients.is_empty() || coefficients.len() > rows {
                return Err(map_transparent_proof_error_v1(
                    TransparentStarkErrorV1::InvalidDomain,
                ));
            }
            if coefficients
                .iter()
                .any(|value| F::canonical(value.0).is_none())
            {
                return Err(map_transparent_proof_error_v1(
                    TransparentStarkErrorV1::NonCanonicalField,
                ));
            }
        }
        let mut result = PrivateTableV1::new(Vec::new(), clear_columns_v1);
        result
            .try_reserve_exact(width)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
        if result.capacity() != width {
            return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
        }
        // These are the final, already admitted matrix allocations, not worker
        // scratch. Install every clearing owner before any field is populated.
        for _ in 0..width {
            let mut column = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
            column
                .try_reserve_exact(rows)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
            if column.capacity() != rows {
                return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
            }
            column.resize(rows, F::ZERO);
            result.push(column.into_vec());
        }
        let failure = result
            .par_iter_mut()
            .enumerate()
            .map(|(index, target)| {
                fill(index, &self.columns[index], target, root, shift)
                    .err()
                    .map(|error| (index, error))
            })
            .reduce(
                || None,
                |left, right| match (left, right) {
                    (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
                    (left, right) => left.or(right),
                },
            );
        if let Some((_, error)) = failure {
            return Err(error);
        }
        if result.len() != width
            || result.capacity() != width
            || result
                .iter()
                .any(|column| column.len() != rows || column.capacity() != rows)
        {
            return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
        }
        Ok(result)
    }
    /// Open an original polynomial directly at an admitted shared/translated point.
    pub(super) fn open_v1(
        &self,
        index: usize,
        point: E,
    ) -> Result<E, ZkX509CaAccumulatorProofErrorV1> {
        if point == E::ZERO
            || !point.is_canonical()
            || point.pow(ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 as u128) == E::ONE
        {
            return Err(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening);
        }
        Ok(self
            .column_v1(index)?
            .iter()
            .rev()
            .fold(E::ZERO, |accumulator, coefficient| {
                accumulator.mul(point).add(E::from_base(*coefficient))
            }))
    }
}

#[cfg(test)]
#[path = "accumulator_retained_columns_tests.rs"]
mod tests;
