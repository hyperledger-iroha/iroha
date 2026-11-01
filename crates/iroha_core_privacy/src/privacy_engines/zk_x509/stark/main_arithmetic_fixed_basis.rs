//! Sealed public P-256 arithmetic fixed coefficients and canonical coset reconstruction.
//! All134 output allocations remain owned; only43 declared affine-basis columns
//! are transformed. Additional descriptors consume one existing fixed cache column.

use super::super::main_bounded_transform::{MainBoundedTransformPolicyV1, check_completion_v1};
use super::super::main_quotient_cache::MainQuotientCachePlanV1;
use super::MainFixedCosetV1;
use super::*;
use crate::privacy_engines::zk_x509::{
    p256_aggregate_adapter::{P256MainAdapterV1, P256MainBoundSourceV1},
    p256_air::{P256_BASE_MODULUS_BE_V1, P256_SCALAR_MODULUS_BE_V1},
    private_table::{PrivateTableV1, zeroize_fields_v1},
};
use fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1;

const WIDTH: usize = 134;
pub(in super::super) const BASIS_WIDTH: usize = 43;
const NATIVE_LOG: u8 = 19;
const NATIVE_ROWS: usize = 1 << NATIVE_LOG;
const BASIS: [usize; BASIS_WIDTH] = [
    0, 1, 15, 17, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38,
    39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 83, 90, 91, 92, 122, 123, 126,
];
pub(in super::super) const DESCRIPTOR_RESERVATION: usize =
    NATIVE_ROWS * core::mem::size_of::<F>() + core::mem::size_of::<PrivateTableV1<F>>();
// Include both transition states, construction plus borrowed recipe arrays,
// all borrowed views and43 Vec slots. Recipes are public stack temporaries;
// neither closed owner carries an oversized inline enum variant. The full134
// field columns are already charged unchanged.
const DESCRIPTOR_PAYLOAD: usize = core::mem::size_of::<MainArithmeticFixedCoefficientsV1>()
    + core::mem::size_of::<MainArithmeticFixedCosetV1<'static>>()
    + BASIS_WIDTH * core::mem::size_of::<Vec<F>>()
    + core::mem::size_of::<[&mut [F]; WIDTH]>()
    + core::mem::size_of::<[&[F]; BASIS_WIDTH]>()
    + 2 * core::mem::size_of::<[Recipe; WIDTH]>();
const _: () = assert!(DESCRIPTOR_PAYLOAD < DESCRIPTOR_RESERVATION);

#[derive(Clone, Copy, Debug, Default)]
struct Recipe {
    constant: F,
    canonicality: F,
    count: usize,
    terms: [(usize, F); 3],
}

impl Recipe {
    fn add(&mut self, original_column: usize, factor: F) -> Result<(), ZkX509StarkErrorV1> {
        let basis = BASIS
            .iter()
            .position(|&column| column == original_column)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        *self
            .terms
            .get_mut(self.count)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)? = (basis, factor);
        self.count += 1;
        Ok(())
    }

    fn apply(
        &self,
        basis: &[&[F]; BASIS_WIDTH],
        canonicality: &[F],
        row: usize,
        coefficients: bool,
    ) -> F {
        let mut value = if coefficients && row != 0 {
            F::ZERO
        } else {
            self.constant
        };
        if self.canonicality != F::ZERO {
            value = value.add(canonicality[row].mul(self.canonicality));
        }
        for &(column, factor) in &self.terms[..self.count] {
            value = value.add(basis[column][row].mul(factor));
        }
        value
    }
}

fn recipes() -> Result<[Recipe; WIDTH], ZkX509StarkErrorV1> {
    let inverse_limb = F(65535)
        .inv()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    let mut result = [Recipe::default(); WIDTH];
    for column in 0..WIDTH {
        let recipe = &mut result[column];
        if BASIS.contains(&column) || column == 84 {
            continue;
        }
        match column {
            2 => {
                recipe.add(17, inverse_limb)?;
                recipe.add(0, F::ZERO.sub(F::ONE))?;
                recipe.add(1, F::ZERO.sub(F::ONE))?;
            }
            3..=18 => {
                let limb = column - 3;
                let offset = 30 - 2 * limb;
                let p = F(u64::from(u16::from_be_bytes([
                    P256_BASE_MODULUS_BE_V1[offset],
                    P256_BASE_MODULUS_BE_V1[offset + 1],
                ])));
                let n = F(u64::from(u16::from_be_bytes([
                    P256_SCALAR_MODULUS_BE_V1[offset],
                    P256_SCALAR_MODULUS_BE_V1[offset + 1],
                ])));
                recipe.add(17, n.mul(inverse_limb))?;
                recipe.add(15, p.sub(n))?;
            }
            51..=66 => {
                recipe.add(19 + column - 51, F::ONE)?;
                recipe.add(35 + column - 51, F::ONE)?;
            }
            67..=82 => recipe.add(19 + column - 67, F::ONE)?,
            85 => {
                recipe.add(19, F::ONE)?;
                recipe.add(35, F::ONE)?;
            }
            86 => {
                recipe.add(34, F::ONE)?;
                recipe.add(50, F::ONE)?;
            }
            87 => recipe.add(19, F::ONE)?,
            88 => recipe.add(50, F::ONE)?,
            89 => {
                recipe.constant = F::ONE;
                recipe.add(17, F::ZERO.sub(inverse_limb))?;
            }
            90..=121 => {
                let slot = (column - 90) / 4;
                match (column - 90) % 4 {
                    0 => recipe.add(90, F::ONE)?,
                    1 => recipe.add(91, F::ONE)?,
                    2 => {
                        recipe.add(92, F::ONE)?;
                        recipe.add(90, F::ZERO.sub(F((slot / 4) as u64)))?;
                    }
                    3 => recipe.add(90, F((4 - slot % 4) as u64))?,
                    _ => return Err(ZkX509StarkErrorV1::InternalInvariant),
                }
            }
            124 | 133 => {
                recipe.constant = F::ONE;
                recipe.add(123, F::ZERO.sub(F::ONE))?;
            }
            125 | 127 | 129 => recipe.canonicality = F::ONE,
            128 | 130 => {
                recipe.add(126, F::ONE)?;
                recipe.canonicality = F(((column - 126) / 2) as u64);
            }
            131 => recipe.add(122, F::ONE)?,
            132 => recipe.add(123, F::ONE)?,
            _ => return Err(ZkX509StarkErrorV1::InternalInvariant),
        }
    }
    Ok(result)
}

fn reconstruct(
    full: &mut [Vec<F>],
    coefficients: bool,
    recipes: &[Recipe; WIDTH],
) -> Result<(), ZkX509StarkErrorV1> {
    let rows = full.first().map_or(0, Vec::len);
    if full.len() != WIDTH
        || rows < 2
        || !rows.is_power_of_two()
        || rows > NATIVE_ROWS
        || full.iter().any(|column| column.len() != rows)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut views: [&mut [F]; WIDTH] = core::array::from_fn(|_| -> &mut [F] { &mut [] });
    for (view, column) in views.iter_mut().zip(full) {
        *view = column;
    }
    let basis: [&[F]; BASIS_WIDTH] = core::array::from_fn(|index| -> &[F] {
        core::mem::take::<&mut [F]>(&mut views[BASIS[index]])
    });
    let canonicality = core::mem::take(&mut views[84]);
    canonicality
        .par_iter_mut()
        .enumerate()
        .for_each(|(row, target)| {
            *target = basis[4..20]
                .iter()
                .fold(F::ZERO, |sum, column| sum.add(column[row]));
        });
    let canonicality: &[F] = canonicality;
    views
        .par_iter_mut()
        .enumerate()
        .for_each(|(column, output)| {
            let recipe = &recipes[column];
            for (row, target) in output.iter_mut().enumerate() {
                *target = recipe.apply(&basis, canonicality, row, coefficients);
            }
        });
    Ok(())
}

fn validate_binding_v1(
    binding: MainP256RegistrationBindingV1,
    uncertain: bool,
) -> Result<(), ZkX509StarkErrorV1> {
    check_completion_v1(uncertain)?;
    if binding.main.segment.adapter != SegmentAdapterIdV1::P256Arithmetic
        || binding.main.segment.trace_log2 != NATIVE_LOG
        || binding.main.segment.fixed_width != WIDTH
        || binding.p256.adapter_v1() != P256MainAdapterV1::Arithmetic
        || binding.p256.local_instance_v1() != 0
        || p256_main_registration_from_main_layout_v1(binding.main)? != binding.p256
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}

/// An arithmetic provider is the sole constructor; omitted buffers are native
/// rows, never incorrectly advertised as coefficient columns.
pub(in super::super) struct MainArithmeticFixedCoefficientsV1 {
    basis: ZeroizingBaseColumnsV1,
    full: ZeroizingBaseColumnsV1,
    basis_in_full: bool,
}

impl MainArithmeticFixedCoefficientsV1 {
    pub(in super::super) fn from_source_v1(
        source: &P256MainBoundSourceV1,
        binding: MainP256RegistrationBindingV1,
        policy: MainBoundedTransformPolicyV1,
        cache_plan: MainQuotientCachePlanV1,
    ) -> Result<(Self, MainQuotientCachePlanV1), ZkX509StarkErrorV1> {
        validate_binding_v1(binding, goldilocks_transform_completion_uncertain_v1())?;
        // Reserve before allocating any additional descriptor owner. Keep all
        // original private cache counts; refusal cannot select a dense retry.
        let cache_plan = cache_plan.reserve_fixed_owner_v1(DESCRIPTOR_RESERVATION)?;
        let mut owner = Self {
            basis: ZeroizingBaseColumnsV1(Vec::new()),
            full: ZeroizingBaseColumnsV1(Vec::new()),
            basis_in_full: false,
        };
        owner
            .full
            .0
            .try_reserve_exact(WIDTH)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        owner
            .basis
            .0
            .try_reserve_exact(BASIS_WIDTH)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if owner.full.0.capacity() != WIDTH || owner.basis.0.capacity() != BASIS_WIDTH {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        #[cfg(test)]
        let rows_timer = PhaseTimerV1::start_v1(PhaseV1::CompositionArithmeticFixedRows);
        for _ in 0..WIDTH {
            let column = zeroed_main_trace_column_v1(NATIVE_ROWS)?;
            if column.0.capacity() != NATIVE_ROWS {
                return Err(ZkX509StarkErrorV1::ProofTooLarge);
            }
            owner.full.0.push(column.into_vec_v1());
        }
        source.fill_arithmetic_fixed_matrix_v1(binding.p256, &mut owner.full.0)?;
        #[cfg(test)]
        rows_timer.complete_v1();
        check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        if owner.full.iter().any(|column| {
            column.len() != NATIVE_ROWS
                || column.iter().any(|value| F::canonical(value.0).is_none())
        }) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        for column in BASIS {
            owner
                .basis
                .0
                .push(core::mem::take(&mut owner.full.0[column]));
        }
        #[cfg(test)]
        let inverse_timer =
            PhaseTimerV1::start_v1(PhaseV1::CompositionArithmeticFixedInverseTransform);
        super::super::main_initial_fixed_coefficients_native_with_v1(
            &mut owner.basis.0,
            goldilocks_primitive_root_v1(NATIVE_LOG).map_err(map_transparent_error_v1)?,
            policy,
            &mut super::super::main_native_transform::ExactNativeTransformV1,
            goldilocks_transform_completion_uncertain_v1,
        )?;
        check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        #[cfg(test)]
        inverse_timer.complete_v1();
        Ok((owner, cache_plan))
    }

    pub(in super::super) fn cache_columns_v1(&self) -> Result<&[Vec<F>], ZkX509StarkErrorV1> {
        if self.basis_in_full || self.basis.len() != BASIS_WIDTH {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(&self.basis.0)
    }

    pub(in super::super) fn into_coset_v1<'a>(
        mut self,
        policy: MainBoundedTransformPolicyV1,
        prefix: &'a [PrivateTableV1<F>],
    ) -> Result<MainArithmeticFixedCosetV1<'a>, ZkX509StarkErrorV1> {
        check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        if self.basis_in_full {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        let basis = MainFixedCosetV1::new_v1(NATIVE_LOG, core::mem::take(&mut self.basis.0))?
            .with_transform_policy_v1(policy)?
            .with_coefficient_prefix_v1(prefix)?;
        Ok(MainArithmeticFixedCosetV1 {
            basis,
            full: ZeroizingBaseColumnsV1(core::mem::take(&mut self.full.0)),
            basis_in_full: false,
            poisoned: false,
        })
    }

    /// Preserve complete independent OODS/coefficient assertions without making
    /// production construct or cache the omitted coefficient columns.
    #[cfg(test)]
    pub(in super::super) fn full_coefficients_for_test_v1(
        &mut self,
    ) -> Result<&[Vec<F>], ZkX509StarkErrorV1> {
        check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        if !self.basis_in_full {
            swap_basis_v1(&mut self.full.0, &mut self.basis.0, true, NATIVE_ROWS)?;
            self.basis_in_full = true;
            let recipes = recipes()?;
            reconstruct(&mut self.full.0, true, &recipes)?;
        }
        Ok(&self.full.0)
    }
}

impl Drop for MainArithmeticFixedCoefficientsV1 {
    fn drop(&mut self) {
        erase_columns_v1(&mut self.full.0);
        erase_columns_v1(&mut self.basis.0);
    }
}

pub(in super::super) struct MainArithmeticFixedCosetV1<'a> {
    basis: MainFixedCosetV1<'a>,
    full: ZeroizingBaseColumnsV1,
    basis_in_full: bool,
    poisoned: bool,
}

impl MainArithmeticFixedCosetV1<'_> {
    pub(in super::super) fn evaluate_v1(
        &mut self,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
    ) -> Result<&[Vec<F>], ZkX509StarkErrorV1> {
        let already_poisoned = self.poisoned;
        self.poisoned = true;
        check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        if already_poisoned || stripe.rows != NATIVE_ROWS {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if self.basis_in_full {
            swap_basis_v1(
                &mut self.full.0,
                &mut self.basis.columns.0,
                false,
                NATIVE_ROWS,
            )?;
            self.basis_in_full = false;
        }
        self.basis.evaluate_v1(stripe)?;
        swap_basis_v1(
            &mut self.full.0,
            &mut self.basis.columns.0,
            true,
            NATIVE_ROWS,
        )?;
        self.basis_in_full = true;
        let recipes = recipes()?;
        reconstruct(&mut self.full.0, false, &recipes)?;
        check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        self.poisoned = false;
        Ok(&self.full.0)
    }
}

impl Drop for MainArithmeticFixedCosetV1<'_> {
    fn drop(&mut self) {
        erase_columns_v1(&mut self.full.0);
        erase_columns_v1(&mut self.basis.columns.0);
    }
}

fn erase_columns_v1(columns: &mut Vec<Vec<F>>) {
    for column in columns.iter_mut() {
        zeroize_fields_v1(column);
    }
    columns.clear();
}

fn swap_basis_v1(
    full: &mut [Vec<F>],
    basis: &mut [Vec<F>],
    into_full: bool,
    rows: usize,
) -> Result<(), ZkX509StarkErrorV1> {
    if rows < 2
        || !rows.is_power_of_two()
        || rows > NATIVE_ROWS
        || full.len() != WIDTH
        || basis.len() != BASIS_WIDTH
        || BASIS.iter().enumerate().any(|(index, &column)| {
            let (empty, occupied) = if into_full {
                (&full[column], &basis[index])
            } else {
                (&basis[index], &full[column])
            };
            !empty.is_empty()
                || empty.capacity() != 0
                || occupied.len() != rows
                || occupied.capacity() != rows
        })
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    for (index, column) in BASIS.into_iter().enumerate() {
        core::mem::swap(&mut full[column], &mut basis[index]);
    }
    Ok(())
}

#[cfg(test)]
#[path = "main_arithmetic_fixed_basis_tests.rs"]
mod tests;
