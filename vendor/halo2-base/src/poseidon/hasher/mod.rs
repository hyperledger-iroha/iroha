use crate::{
    gates::{GateInstructions, RangeInstructions},
    poseidon::hasher::{spec::OptimizedPoseidonSpec, state::PoseidonState},
    safe_types::{SafeBool, SafeTypeChip},
    utils::BigPrimeField,
    AssignedValue, Context,
    QuantumCell::Constant,
    ScalarField,
};

use getset::{CopyGetters, Getters};
use num_bigint::BigUint;
use std::{cell::OnceCell, mem};

#[cfg(test)]
mod tests;

/// Module for maximum distance separable matrix operations.
pub mod mds;
/// Module for poseidon specification.
pub mod spec;
/// Module for poseidon states.
pub mod state;

/// Stateless Poseidon hasher.
#[derive(Clone, Debug, Getters)]
pub struct PoseidonHasher<F: ScalarField, const T: usize, const RATE: usize> {
    /// Spec, contains round constants and optimized matrices.
    #[getset(get = "pub")]
    spec: OptimizedPoseidonSpec<F, T, RATE>,
    consts: OnceCell<PoseidonHasherConsts<F, T, RATE>>,
}
#[derive(Clone, Debug, Getters)]
struct PoseidonHasherConsts<F: ScalarField, const T: usize, const RATE: usize> {
    #[getset(get = "pub")]
    init_state: PoseidonState<F, T, RATE>,
    // hash of an empty input("").
    #[getset(get = "pub")]
    empty_hash: AssignedValue<F>,
}

impl<F: ScalarField, const T: usize, const RATE: usize> PoseidonHasherConsts<F, T, RATE> {
    pub fn new(
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        spec: &OptimizedPoseidonSpec<F, T, RATE>,
    ) -> Self {
        let init_state = PoseidonState::default(ctx);
        let mut state = init_state.clone();
        let empty_hash = fix_len_array_squeeze(ctx, gate, &[], &mut state, spec);
        Self { init_state, empty_hash }
    }
}

/// 1 logical row of compact input for Poseidon hasher.
#[derive(Copy, Clone, Debug, Getters, CopyGetters)]
pub struct PoseidonCompactInput<F: ScalarField, const RATE: usize> {
    /// Right padded inputs. No constrains on paddings.
    #[getset(get = "pub")]
    inputs: [AssignedValue<F>; RATE],
    /// is_final = 1 triggers squeeze.
    #[getset(get_copy = "pub")]
    is_final: SafeBool<F>,
    /// Length of `inputs`.
    #[getset(get_copy = "pub")]
    len: AssignedValue<F>,
}

impl<F: ScalarField, const RATE: usize> PoseidonCompactInput<F, RATE> {
    /// Create a new PoseidonCompactInput.
    pub fn new(
        inputs: [AssignedValue<F>; RATE],
        is_final: SafeBool<F>,
        len: AssignedValue<F>,
    ) -> Self {
        Self { inputs, is_final, len }
    }

    /// Add data validation constraints.
    pub fn add_validation_constraints(
        &self,
        ctx: &mut Context<F>,
        range: &impl RangeInstructions<F>,
    ) {
        range.check_less_than_safe(ctx, self.len, (RATE + 1) as u64);
        // Valid rows either end a logical input or fill the entire absorb rate.
        let is_full: AssignedValue<F> =
            range.gate().is_equal(ctx, self.len, Constant(F::from(RATE as u64)));
        let valid_cond = range.gate().or(ctx, *self.is_final.as_ref(), is_full);
        range.gate().assert_is_const(ctx, &valid_cond, &F::ONE);
    }
}

/// A compact chunk input for Poseidon hasher. The end of a logical input could only be at the boundary of a chunk.
#[derive(Clone, Debug, Getters, CopyGetters)]
pub struct PoseidonCompactChunkInput<F: ScalarField, const RATE: usize> {
    /// Inputs of a chunk. All witnesses will be absorbed.
    #[getset(get = "pub")]
    inputs: Vec<[AssignedValue<F>; RATE]>,
    /// is_final = 1 triggers squeeze.
    #[getset(get_copy = "pub")]
    is_final: SafeBool<F>,
}

impl<F: ScalarField, const RATE: usize> PoseidonCompactChunkInput<F, RATE> {
    /// Create a new PoseidonCompactInput.
    pub fn new(inputs: Vec<[AssignedValue<F>; RATE]>, is_final: SafeBool<F>) -> Self {
        Self { inputs, is_final }
    }
}

/// 1 logical row of compact output for Poseidon hasher.
#[derive(Copy, Clone, Debug, CopyGetters)]
pub struct PoseidonCompactOutput<F: ScalarField> {
    /// hash of 1 logical input.
    #[getset(get_copy = "pub")]
    hash: AssignedValue<F>,
    /// is_final = 1 ==> this is the end of a logical input.
    #[getset(get_copy = "pub")]
    is_final: SafeBool<F>,
}

impl<F: ScalarField, const T: usize, const RATE: usize> PoseidonHasher<F, T, RATE> {
    /// Create a poseidon hasher from an existing spec.
    pub fn new(spec: OptimizedPoseidonSpec<F, T, RATE>) -> Self {
        Self { spec, consts: OnceCell::new() }
    }
    /// Initialize necessary consts of hasher. Must be called before any computation.
    pub fn initialize_consts(&mut self, ctx: &mut Context<F>, gate: &impl GateInstructions<F>) {
        self.consts.get_or_init(|| PoseidonHasherConsts::<F, T, RATE>::new(ctx, gate, &self.spec));
    }

    /// Clear all consts.
    pub fn clear(&mut self) {
        self.consts.take();
    }

    fn empty_hash(&self) -> &AssignedValue<F> {
        self.consts.get().unwrap().empty_hash()
    }
    fn init_state(&self) -> &PoseidonState<F, T, RATE> {
        self.consts.get().unwrap().init_state()
    }

    /// Constrains and returns hash of a witness array with a variable length.
    ///
    /// Assumes `len` is within [usize] and `len <= inputs.len()`.
    /// * inputs: An right-padded array of [AssignedValue]. Constraints on paddings are not required.
    /// * len: Length of `inputs`.
    ///
    /// Return hash of `inputs`.
    pub fn hash_var_len_array(
        &self,
        ctx: &mut Context<F>,
        range: &impl RangeInstructions<F>,
        inputs: &[AssignedValue<F>],
        len: AssignedValue<F>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        // TODO: rewrite this using hash_compact_input.
        let max_len = inputs.len();
        if max_len == 0 {
            return *self.empty_hash();
        };

        // len <= max_len --> num_of_bits(len) <= num_of_bits(max_len)
        let num_bits = (usize::BITS - max_len.leading_zeros()) as usize;
        // num_perm = len // RATE + 1, len_last_chunk = len % RATE
        let (mut num_perm, len_last_chunk) = range.div_mod(ctx, len, BigUint::from(RATE), num_bits);
        num_perm = range.gate().inc(ctx, num_perm);

        let mut state = self.init_state().clone();
        let mut result_state = state.clone();
        for (i, chunk) in inputs.chunks(RATE).enumerate() {
            let is_last_perm =
                range.gate().is_equal(ctx, num_perm, Constant(F::from((i + 1) as u64)));
            let len_chunk = range.gate().select(
                ctx,
                len_last_chunk,
                Constant(F::from(RATE as u64)),
                is_last_perm,
            );

            state.permutation(ctx, range.gate(), chunk, Some(len_chunk), &self.spec);
            result_state.select(
                ctx,
                range.gate(),
                SafeTypeChip::<F>::unsafe_to_bool(is_last_perm),
                &state,
            );
        }
        if max_len % RATE == 0 {
            let is_last_perm = range.gate().is_equal(
                ctx,
                num_perm,
                Constant(F::from((max_len / RATE + 1) as u64)),
            );
            let len_chunk = ctx.load_zero();
            state.permutation(ctx, range.gate(), &[], Some(len_chunk), &self.spec);
            result_state.select(
                ctx,
                range.gate(),
                SafeTypeChip::<F>::unsafe_to_bool(is_last_perm),
                &state,
            );
        }
        result_state.s[1]
    }

    /// Constrains and returns hash of a witness array.
    ///
    /// * inputs: An array of [AssignedValue].
    ///
    /// Return hash of `inputs`.
    pub fn hash_fix_len_array(
        &self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        inputs: &[AssignedValue<F>],
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        let mut state = self.init_state().clone();
        fix_len_array_squeeze(ctx, gate, inputs, &mut state, &self.spec)
    }

    /// Constrains and returns hashes of inputs in a compact format. Length of `compact_inputs` should be determined at compile time.
    pub fn hash_compact_input(
        &self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        compact_inputs: &[PoseidonCompactInput<F, RATE>],
    ) -> Vec<PoseidonCompactOutput<F>>
    where
        F: BigPrimeField,
    {
        let mut outputs = Vec::with_capacity(compact_inputs.len());
        let mut state = self.init_state().clone();
        for input in compact_inputs {
            // Assume this is the last row of a logical input:
            // Depending on if len == RATE.
            let is_full = gate.is_equal(ctx, input.len, Constant(F::from(RATE as u64)));
            // Case 1: if len != RATE.
            state.permutation(ctx, gate, &input.inputs, Some(input.len), &self.spec);
            // Case 2: if len == RATE, an extra permuation is needed for squeeze.
            let mut state_2 = state.clone();
            state_2.permutation(ctx, gate, &[], None, &self.spec);
            // Select the result of case 1/2 depending on if len == RATE.
            let hash = gate.select(ctx, state_2.s[1], state.s[1], is_full);
            outputs.push(PoseidonCompactOutput { hash, is_final: input.is_final });
            // Reset state to init_state if this is the end of a logical input.
            // TODO: skip this if this is the last row.
            state.select(ctx, gate, input.is_final, self.init_state());
        }
        outputs
    }

    /// Constrains and returns hashes of chunk inputs in a compact format. Length of `chunk_inputs` should be determined at compile time.
    pub fn hash_compact_chunk_inputs(
        &self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        chunk_inputs: &[PoseidonCompactChunkInput<F, RATE>],
    ) -> Vec<PoseidonCompactOutput<F>>
    where
        F: BigPrimeField,
    {
        let zero_witness = ctx.load_zero();
        let mut outputs = Vec::with_capacity(chunk_inputs.len());
        let mut state = self.init_state().clone();
        for chunk_input in chunk_inputs {
            let is_final = chunk_input.is_final;
            for absorb in &chunk_input.inputs {
                state.permutation(ctx, gate, absorb, None, &self.spec);
            }
            // Because the length of each absorb is always RATE. An extra permutation is needed for squeeze.
            let mut output_state = state.clone();
            output_state.permutation(ctx, gate, &[], None, &self.spec);
            let hash = gate.select(ctx, output_state.s[1], zero_witness, *is_final.as_ref());
            outputs.push(PoseidonCompactOutput { hash, is_final });
            // Reset state to init_state if this is the end of a logical input.
            state.select(ctx, gate, is_final, self.init_state());
        }
        outputs
    }
}

/// Poseidon sponge. This is stateful.
pub struct PoseidonSponge<F: ScalarField, const T: usize, const RATE: usize> {
    init_state: PoseidonState<F, T, RATE>,
    state: PoseidonState<F, T, RATE>,
    spec: OptimizedPoseidonSpec<F, T, RATE>,
    absorbing: Vec<AssignedValue<F>>,
}

impl<F: ScalarField, const T: usize, const RATE: usize> PoseidonSponge<F, T, RATE> {
    /// Create new Poseidon hasher.
    pub fn new<const R_F: usize, const R_P: usize, const SECURE_MDS: usize>(
        ctx: &mut Context<F>,
    ) -> Self {
        let init_state = PoseidonState::default(ctx);
        let state = init_state.clone();
        Self {
            init_state,
            state,
            spec: OptimizedPoseidonSpec::new::<R_F, R_P, SECURE_MDS>(),
            absorbing: Vec::new(),
        }
    }

    /// Initialize a poseidon hasher from an existing spec.
    pub fn from_spec(ctx: &mut Context<F>, spec: OptimizedPoseidonSpec<F, T, RATE>) -> Self {
        let init_state = PoseidonState::default(ctx);
        Self { spec, state: init_state.clone(), init_state, absorbing: Vec::new() }
    }

    /// Reset state to default and clear the buffer.
    pub fn clear(&mut self) {
        self.state = self.init_state.clone();
        self.absorbing.clear();
    }

    /// Store given `elements` into buffer.
    pub fn update(&mut self, elements: &[AssignedValue<F>]) {
        self.absorbing.extend_from_slice(elements);
    }

    /// Consume buffer and perform permutation, then output second element of
    /// state.
    pub fn squeeze(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
    ) -> AssignedValue<F> {
        let input_elements = mem::take(&mut self.absorbing);
        fix_len_array_squeeze(ctx, gate, &input_elements, &mut self.state, &self.spec)
    }
}

/// ATTETION: input_elements.len() needs to be fixed at compile time.
fn fix_len_array_squeeze<F: ScalarField, const T: usize, const RATE: usize>(
    ctx: &mut Context<F>,
    gate: &impl GateInstructions<F>,
    input_elements: &[AssignedValue<F>],
    state: &mut PoseidonState<F, T, RATE>,
    spec: &OptimizedPoseidonSpec<F, T, RATE>,
) -> AssignedValue<F> {
    let exact = input_elements.len() % RATE == 0;

    for chunk in input_elements.chunks(RATE) {
        state.permutation(ctx, gate, chunk, None, spec);
    }
    if exact {
        state.permutation(ctx, gate, &[], None, spec);
    }

    state.s[1]
}
