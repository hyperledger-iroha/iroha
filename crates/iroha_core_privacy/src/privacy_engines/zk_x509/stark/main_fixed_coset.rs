//! One bounded public fixed-polynomial matrix reused across quotient cosets.
//!
//! Fixed polynomials have degree below their native domain. Unlike masked
//! witness polynomials, they therefore fit in every quotient stripe without
//! folding. Recovering the shifted coefficients in place permits moving to the
//! next stripe without retaining separate coefficient and evaluation matrices.

use super::main_bounded_transform::{MainBoundedTransformPolicyV1, check_completion_v1};
use super::*;
use crate::privacy_engines::transparent_stark::{goldilocks_fft_v1, goldilocks_ifft_v1};
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as TransformError, goldilocks_transform_completion_uncertain_v1,
    transform_goldilocks_columns_v1,
};

pub(super) struct MainFixedCosetV1 {
    columns: ZeroizingBaseColumnsV1,
    native_log2: u8,
    previous: Option<main_quotient_stripes::MainQuotientStripeV1>,
    poisoned: bool,
    transform_policy: MainBoundedTransformPolicyV1,
}

impl MainFixedCosetV1 {
    pub(super) fn new_v1(
        native_log2: u8,
        coefficients: Vec<Vec<F>>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let owner = Self {
            columns: ZeroizingBaseColumnsV1(coefficients),
            native_log2,
            previous: None,
            poisoned: false,
            transform_policy: MainBoundedTransformPolicyV1::cpu_v1(),
        };
        let rows = 1_usize
            .checked_shl(u32::from(native_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if native_log2 > main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1
            || owner.columns.is_empty()
            || owner.columns.iter().any(|column| {
                column.len() != rows || column.iter().any(|value| F::canonical(value.0).is_none())
            })
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(owner)
    }

    /// Install the outer assembly's checked allowance before the first stripe.
    pub(super) fn with_transform_policy_v1(
        mut self,
        policy: MainBoundedTransformPolicyV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if self.previous.is_some() || self.poisoned {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        self.transform_policy = policy;
        Ok(self)
    }

    fn validate_stripe_v1(
        &self,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let native_rows = 1_usize << self.native_log2;
        let full_rows = stripe
            .rows
            .checked_mul(stripe.count)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if self.poisoned
            || !stripe.rows.is_power_of_two()
            || !stripe.count.is_power_of_two()
            || stripe.rows < native_rows
            || stripe.rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1
            || full_rows <= native_rows
            || full_rows.ilog2() > 32
            || stripe.ordinal >= stripe.count
            || stripe.next_stride != stripe.rows / native_rows
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root = goldilocks_primitive_root_v1(stripe.rows.ilog2() as u8)
            .map_err(map_transparent_error_v1)?;
        let full_root = goldilocks_primitive_root_v1(full_rows.ilog2() as u8)
            .map_err(map_transparent_error_v1)?;
        if stripe.root != root
            || stripe.shift != F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(stripe.ordinal as u128))
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        match self.previous {
            None if stripe.ordinal == 0 => Ok(()),
            Some(previous)
                if previous.rows == stripe.rows
                    && previous.count == stripe.count
                    && previous.root == stripe.root
                    && previous.ordinal.checked_add(1) == Some(stripe.ordinal) =>
            {
                Ok(())
            }
            _ => Err(ZkX509StarkErrorV1::ProfileMismatch),
        }
    }

    /// Return canonical row ordering for the next interleaved stripe. Any
    /// rejected call poisons this owner, including failures before mutation.
    pub(super) fn evaluate_v1(
        &mut self,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
    ) -> Result<&[Vec<F>], ZkX509StarkErrorV1> {
        let result = self.evaluate_with_v1(
            stripe,
            |words, root, direction| {
                transform_goldilocks_columns_v1(
                    words,
                    root,
                    direction,
                    fastpq_prover::ExecutionMode::Auto,
                )
            },
            goldilocks_transform_completion_uncertain_v1,
        );
        #[cfg(test)]
        if result.is_err() {
            super::super::super::prover_observation::failed_fixed_coset_v1();
        }
        result
    }

    fn evaluate_with_v1(
        &mut self,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
        mut transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<&[Vec<F>], ZkX509StarkErrorV1> {
        let valid = self.validate_stripe_v1(stripe);
        self.poisoned = true;
        valid?;
        // Explicit CPU dispatch does not itself check quarantined storage.
        // Admission must remain terminal even after selecting CPU fallback.
        check_completion_v1(uncertain())?;
        let previous = self.previous;
        let diagonal = match previous {
            Some(previous) => stripe.shift.mul(
                previous
                    .shift
                    .inv()
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            ),
            None => stripe.shift,
        };
        // The fixed matrix is public. Reserve before padding, then reuse each
        // allocation for inverse/diagonal/forward operations. A device path
        // charges one bounded word batch; no full second matrix is copied and
        // no output is exposed until every column has completed.
        for column in &mut self.columns.0 {
            if column.len() < stripe.rows {
                column
                    .try_reserve_exact(stripe.rows - column.len())
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
                column.resize(stripe.rows, F::ZERO);
            }
        }
        let device_columns = self.transform_policy.columns_v1(stripe.rows);
        let batch_columns = if device_columns == 0 {
            aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
        } else {
            device_columns
        };
        for batch in self.columns.0.chunks_mut(batch_columns) {
            check_completion_v1(uncertain())?;
            if device_columns == 0 {
                batch.par_iter_mut().try_for_each(|column| {
                    if previous.is_some() {
                        goldilocks_ifft_v1(column, stripe.root)
                            .map_err(map_transparent_error_v1)?;
                    }
                    let mut power = F::ONE;
                    for value in &mut *column {
                        *value = value.mul(power);
                        power = power.mul(diagonal);
                    }
                    goldilocks_fft_v1(column, stripe.root).map_err(map_transparent_error_v1)
                })?;
                #[cfg(test)]
                {
                    super::super::super::prover_observation::completed_fixed_backend_v1(
                        false,
                        false,
                        batch.len(),
                    );
                    if previous.is_some() {
                        super::super::super::prover_observation::completed_fixed_backend_v1(
                            false,
                            true,
                            batch.len(),
                        );
                    }
                }
            } else {
                self.transform_policy.apply_with_v1(
                    batch,
                    stripe.root,
                    diagonal,
                    previous.is_some(),
                    &mut transform,
                    &mut uncertain,
                )?;
            }
            #[cfg(test)]
            super::super::super::prover_observation::completed_fixed_coset_v1(
                batch.len(),
                stripe.rows,
                previous.is_some(),
            );
        }
        check_completion_v1(uncertain())?;
        self.previous = Some(stripe);
        self.poisoned = false;
        Ok(&self.columns)
    }
}

impl Drop for MainFixedCosetV1 {
    fn drop(&mut self) {
        for column in &mut self.columns.0 {
            super::super::super::private_table::zeroize_fields_v1(column);
        }
    }
}

#[cfg(test)]
#[path = "main_fixed_coset_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "main_fixed_coset_device_tests.rs"]
mod device_tests;
