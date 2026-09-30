//! One bounded public fixed-polynomial matrix reused across quotient cosets.
//!
//! Fixed polynomials have degree below their native domain. Unlike masked
//! witness polynomials, they therefore fit in every quotient stripe without
//! folding. Recovering the shifted coefficients in place permits moving to the
//! next stripe without retaining separate coefficient and evaluation matrices.

use super::*;
use crate::privacy_engines::transparent_stark::{goldilocks_fft_v1, goldilocks_ifft_v1};

pub(super) struct MainFixedCosetV1 {
    columns: ZeroizingBaseColumnsV1,
    native_log2: u8,
    previous: Option<main_quotient_stripes::MainQuotientStripeV1>,
    poisoned: bool,
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
        let valid = self.validate_stripe_v1(stripe);
        self.poisoned = true;
        valid?;
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
        // allocation for inverse/diagonal/forward operations. No value matrix
        // is copied, and no output is exposed until every column has completed.
        for column in &mut self.columns.0 {
            if column.len() < stripe.rows {
                column
                    .try_reserve_exact(stripe.rows - column.len())
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
                column.resize(stripe.rows, F::ZERO);
            }
        }
        for batch in self
            .columns
            .0
            .chunks_mut(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
        {
            batch.par_iter_mut().try_for_each(|column| {
                if previous.is_some() {
                    goldilocks_ifft_v1(column, stripe.root).map_err(map_transparent_error_v1)?;
                }
                let mut power = F::ONE;
                for value in &mut *column {
                    *value = value.mul(power);
                    power = power.mul(diagonal);
                }
                goldilocks_fft_v1(column, stripe.root).map_err(map_transparent_error_v1)
            })?;
            #[cfg(test)]
            super::super::super::prover_observation::completed_fixed_coset_v1(
                batch.len(),
                stripe.rows,
                previous.is_some(),
            );
        }
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
