//! One bounded public fixed-polynomial matrix reused across quotient cosets.
//!
//! Fixed polynomials have degree below their native domain. Unlike masked
//! witness polynomials, they therefore fit in every quotient stripe without
//! folding. A public coefficient prefix borrows explicitly unspent registration
//! cache space. The uncached suffix recovers shifted coefficients in place.

use super::main_bounded_transform::{MainBoundedTransformPolicyV1, check_completion_v1};
use super::*;
use crate::privacy_engines::transparent_stark::{
    goldilocks_fft_coarse_v1, goldilocks_ifft_coarse_v1,
};
use fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1;
#[cfg(test)]
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as TransformError, transform_goldilocks_columns_v1,
};

/// Closed fixed transition seam: exact prepared publication in production,
/// with the original word transaction retained only by test implementations.
trait MainFixedTransformV1 {
    fn columns_v1(&self, policy: MainBoundedTransformPolicyV1, rows: usize) -> usize;
    fn apply_v1(
        &mut self,
        policy: MainBoundedTransformPolicyV1,
        columns: &mut [Vec<F>],
        root: F,
        diagonal: F,
        recovery: bool,
        uncertain: &mut dyn FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1>;
}

struct ExactFixedTransformV1;
impl MainFixedTransformV1 for ExactFixedTransformV1 {
    fn columns_v1(&self, policy: MainBoundedTransformPolicyV1, rows: usize) -> usize {
        policy.forward_columns_v1(rows)
    }

    fn apply_v1(
        &mut self,
        policy: MainBoundedTransformPolicyV1,
        columns: &mut [Vec<F>],
        root: F,
        diagonal: F,
        recovery: bool,
        uncertain: &mut dyn FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        policy.apply_exact_v1(columns, root, diagonal, recovery, uncertain)
    }
}

#[cfg(test)]
struct WordFixedTransformV1<T>(T);
#[cfg(test)]
impl<T> MainFixedTransformV1 for WordFixedTransformV1<T>
where
    T: FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
{
    fn columns_v1(&self, policy: MainBoundedTransformPolicyV1, rows: usize) -> usize {
        policy.columns_v1(rows)
    }

    fn apply_v1(
        &mut self,
        policy: MainBoundedTransformPolicyV1,
        columns: &mut [Vec<F>],
        root: F,
        diagonal: F,
        recovery: bool,
        uncertain: &mut dyn FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        policy.apply_with_v1(columns, root, diagonal, recovery, &mut self.0, uncertain)
    }
}

pub(super) struct MainFixedCosetV1<'a> {
    columns: ZeroizingBaseColumnsV1,
    native_log2: u8,
    previous: Option<main_quotient_stripes::MainQuotientStripeV1>,
    poisoned: bool,
    transform_policy: MainBoundedTransformPolicyV1,
    coefficient_prefix: &'a [super::super::super::private_table::PrivateTableV1<F>],
}

impl<'a> MainFixedCosetV1<'a> {
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
            coefficient_prefix: &[],
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

    /// Bind the immutable, separately charged prefix to these original public
    /// coefficients before any stripe is evaluated. The lifetime prevents the
    /// cache from being replaced or dropped while an evaluation can borrow it.
    pub(super) fn with_coefficient_prefix_v1(
        mut self,
        prefix: &'a [super::super::super::private_table::PrivateTableV1<F>],
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if self.previous.is_some()
            || self.poisoned
            || !self.coefficient_prefix.is_empty()
            || prefix.len() > self.columns.len()
            || prefix
                .iter()
                .zip(&self.columns.0)
                .any(|(original, column)| **original != column.as_slice())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        self.coefficient_prefix = prefix;
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
        let root = goldilocks_primitive_root_v1(
            u8::try_from(stripe.rows.ilog2()).expect("usize logarithm fits u8"),
        )
        .map_err(map_transparent_error_v1)?;
        let full_root = goldilocks_primitive_root_v1(
            u8::try_from(full_rows.ilog2()).expect("usize logarithm fits u8"),
        )
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
        let result = self.evaluate_transform_v1(
            stripe,
            &mut ExactFixedTransformV1,
            goldilocks_transform_completion_uncertain_v1,
        );
        #[cfg(test)]
        if result.is_err() {
            super::super::super::prover_observation::failed_fixed_coset_v1();
        }
        result
    }

    #[cfg(test)]
    fn evaluate_with_v1(
        &mut self,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
        transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        uncertain: impl FnMut() -> bool,
    ) -> Result<&[Vec<F>], ZkX509StarkErrorV1> {
        self.evaluate_transform_v1(stripe, &mut WordFixedTransformV1(transform), uncertain)
    }

    fn evaluate_transform_v1(
        &mut self,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
        transform: &mut impl MainFixedTransformV1,
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
        // charges one bounded word batch. The immutable prefix remains in its
        // separately charged cache; output allocations are reused, and no
        // output is exposed until every column has completed.
        for column in &mut self.columns.0 {
            if column.len() < stripe.rows {
                column
                    .try_reserve_exact(stripe.rows - column.len())
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
                column.resize(stripe.rows, F::ZERO);
            }
        }
        let device_columns = transform.columns_v1(self.transform_policy, stripe.rows);
        let batch_columns = if device_columns == 0 {
            aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
        } else {
            device_columns
        };
        let has_recovery = previous.is_some() && self.coefficient_prefix.len() < self.columns.len();
        let (forward_powers, inverse_powers) = if device_columns == 0 {
            self.transform_policy
                .cpu_power_pair_v1(stripe.rows, stripe.root, has_recovery)?
        } else {
            (None, None)
        };
        let (retained, recovered) = self.columns.0.split_at_mut(self.coefficient_prefix.len());
        for (columns, recovery, diagonal, prefix) in [
            (retained, false, stripe.shift, self.coefficient_prefix),
            (recovered, previous.is_some(), diagonal, &[][..]),
        ] {
            for (batch_index, batch) in columns.chunks_mut(batch_columns).enumerate() {
                check_completion_v1(uncertain())?;
                if !prefix.is_empty() {
                    let first = batch_index * batch_columns;
                    for (column, coefficients) in batch.iter_mut().zip(&prefix[first..]) {
                        column.fill(F::ZERO);
                        column[..coefficients.len()].copy_from_slice(coefficients);
                    }
                }
                if device_columns == 0 {
                    // The bounded outer batch owns parallelism on the CPU path.
                    batch.par_iter_mut().try_for_each(|column| {
                    if recovery {
                        if let Some(powers) = &inverse_powers {
                            crate::privacy_engines::transparent_stark::goldilocks_ifft_coarse_with_powers_v1(column, stripe.root, powers)
                        } else { goldilocks_ifft_coarse_v1(column, stripe.root) }
                        .map_err(map_transparent_error_v1)?;
                    }
                    let mut power = F::ONE;
                    for value in &mut *column {
                        *value = value.mul(power);
                        power = power.mul(diagonal);
                    }
                    if let Some(powers) = &forward_powers {
                        crate::privacy_engines::transparent_stark::goldilocks_fft_coarse_with_powers_v1(column, stripe.root, powers)
                    } else { goldilocks_fft_coarse_v1(column, stripe.root) }
                    .map_err(map_transparent_error_v1)
                })?;
                    #[cfg(test)]
                    {
                        super::super::super::prover_observation::completed_fixed_backend_v1(
                            false,
                            false,
                            batch.len(),
                        );
                        if recovery {
                            super::super::super::prover_observation::completed_fixed_backend_v1(
                                false,
                                true,
                                batch.len(),
                            );
                        }
                    }
                } else {
                    transform.apply_v1(
                        self.transform_policy,
                        batch,
                        stripe.root,
                        diagonal,
                        recovery,
                        &mut uncertain,
                    )?;
                }
                #[cfg(test)]
                super::super::super::prover_observation::completed_fixed_coset_v1(
                    batch.len(),
                    stripe.rows,
                    recovery,
                );
            }
        }
        check_completion_v1(uncertain())?;
        self.previous = Some(stripe);
        self.poisoned = false;
        Ok(&self.columns)
    }
}

impl Drop for MainFixedCosetV1<'_> {
    fn drop(&mut self) {
        for column in &mut self.columns.0 {
            super::super::super::private_table::zeroize_fields_v1(column);
        }
        // Release the already-cleared allocations now. The nested clearing
        // owner remains the unwind fallback if any earlier cleanup fails, but
        // its normal Drop must not erase this same complete matrix twice.
        self.columns.0.clear();
    }
}

#[cfg(test)]
#[path = "main_fixed_coset_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "main_fixed_basis_tests.rs"]
mod basis_tests;

#[cfg(test)]
#[path = "main_fixed_coset_device_tests.rs"]
mod device_tests;

#[cfg(test)]
#[path = "main_fixed_prepared_tests.rs"]
mod prepared_tests;

#[path = "main_arithmetic_fixed_basis.rs"]
pub(super) mod arithmetic_basis;
