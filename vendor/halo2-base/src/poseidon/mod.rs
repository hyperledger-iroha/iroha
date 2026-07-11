use crate::{
    gates::{RangeChip, RangeInstructions},
    poseidon::hasher::{spec::OptimizedPoseidonSpec, PoseidonHasher},
    safe_types::{FixLenBytes, VarLenBytes, VarLenBytesVec},
    utils::{BigPrimeField, ScalarField},
    AssignedValue, Context,
};

use itertools::Itertools;

/// Module for Poseidon hasher
pub mod hasher;

/// Chip for Poseidon hash.
pub struct PoseidonChip<'a, F: ScalarField, const T: usize, const RATE: usize> {
    range_chip: &'a RangeChip<F>,
    hasher: PoseidonHasher<F, T, RATE>,
}

impl<'a, F: ScalarField, const T: usize, const RATE: usize> PoseidonChip<'a, F, T, RATE> {
    /// Create a new PoseidonChip.
    pub fn new(
        ctx: &mut Context<F>,
        spec: OptimizedPoseidonSpec<F, T, RATE>,
        range_chip: &'a RangeChip<F>,
    ) -> Self {
        let mut hasher = PoseidonHasher::new(spec);
        hasher.initialize_consts(ctx, range_chip.gate());
        Self { range_chip, hasher }
    }
}

/// Trait for Poseidon instructions
pub trait PoseidonInstructions<F: ScalarField> {
    /// Return hash of a [VarLenBytes]
    fn hash_var_len_bytes<const MAX_LEN: usize>(
        &self,
        ctx: &mut Context<F>,
        inputs: &VarLenBytes<F, MAX_LEN>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField;

    /// Return hash of a [VarLenBytesVec]
    fn hash_var_len_bytes_vec(
        &self,
        ctx: &mut Context<F>,
        inputs: &VarLenBytesVec<F>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField;

    /// Return hash of a [FixLenBytes]
    fn hash_fix_len_bytes<const MAX_LEN: usize>(
        &self,
        ctx: &mut Context<F>,
        inputs: &FixLenBytes<F, MAX_LEN>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField;
}

impl<F: ScalarField, const T: usize, const RATE: usize> PoseidonInstructions<F>
    for PoseidonChip<'_, F, T, RATE>
{
    fn hash_var_len_bytes<const MAX_LEN: usize>(
        &self,
        ctx: &mut Context<F>,
        inputs: &VarLenBytes<F, MAX_LEN>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        let inputs_len = inputs.len();
        self.hasher.hash_var_len_array(
            ctx,
            self.range_chip,
            inputs.bytes().map(|sb| *sb.as_ref()).as_ref(),
            *inputs_len,
        )
    }

    fn hash_var_len_bytes_vec(
        &self,
        ctx: &mut Context<F>,
        inputs: &VarLenBytesVec<F>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        let inputs_len = inputs.len();
        self.hasher.hash_var_len_array(
            ctx,
            self.range_chip,
            &inputs.bytes().iter().map(|sb| *sb.as_ref()).collect_vec(),
            *inputs_len,
        )
    }

    fn hash_fix_len_bytes<const MAX_LEN: usize>(
        &self,
        ctx: &mut Context<F>,
        inputs: &FixLenBytes<F, MAX_LEN>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        self.hasher.hash_fix_len_array(
            ctx,
            self.range_chip.gate(),
            inputs.bytes().map(|sb| *sb.as_ref()).as_ref(),
        )
    }
}
