use std::iter;

use itertools::Itertools;

use crate::{
    gates::GateInstructions,
    poseidon::hasher::{mds::SparseMDSMatrix, spec::OptimizedPoseidonSpec},
    safe_types::SafeBool,
    utils::ScalarField,
    AssignedValue, Context,
    QuantumCell::{Constant, Existing},
};

#[derive(Clone, Debug)]
pub(crate) struct PoseidonState<F: ScalarField, const T: usize, const RATE: usize> {
    pub(crate) s: [AssignedValue<F>; T],
}

impl<F: ScalarField, const T: usize, const RATE: usize> PoseidonState<F, T, RATE> {
    pub fn default(ctx: &mut Context<F>) -> Self {
        let mut default_state = [F::ZERO; T];
        // from Section 4.2 of https://eprint.iacr.org/2019/458.pdf
        // • Variable-Input-Length Hashing. The capacity value is 2^64 + (o−1) where o the output length.
        // for our transcript use cases, o = 1
        default_state[0] = F::from_u128(1u128 << 64);
        Self { s: default_state.map(|f| ctx.load_constant(f)) }
    }

    /// Perform permutation on this state.
    ///
    /// ATTETION: inputs.len() needs to be fixed at compile time.
    /// Assume len <= inputs.len().
    /// `inputs` is right padded.
    /// If `len` is `None`, treat `inputs` as a fixed length array.
    pub fn permutation(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        inputs: &[AssignedValue<F>],
        len: Option<AssignedValue<F>>,
        spec: &OptimizedPoseidonSpec<F, T, RATE>,
    ) {
        let r_f = spec.r_f / 2;
        let mds = &spec.mds_matrices.mds.0;
        let pre_sparse_mds = &spec.mds_matrices.pre_sparse_mds.0;
        let sparse_matrices = &spec.mds_matrices.sparse_matrices;

        // First half of the full round
        let constants = &spec.constants.start;
        if let Some(len) = len {
            // Note: this doesn't mean `padded_inputs` is 0 padded because there is no constraints on `inputs[len..]`
            let padded_inputs: [AssignedValue<F>; RATE] =
                core::array::from_fn(
                    |i| if i < inputs.len() { inputs[i] } else { ctx.load_zero() },
                );
            self.absorb_var_len_with_pre_constants(ctx, gate, padded_inputs, len, &constants[0]);
        } else {
            self.absorb_with_pre_constants(ctx, gate, inputs, &constants[0]);
        }
        for constants in constants.iter().skip(1).take(r_f - 1) {
            self.sbox_full(ctx, gate, constants);
            self.apply_mds(ctx, gate, mds);
        }
        self.sbox_full(ctx, gate, constants.last().unwrap());
        self.apply_mds(ctx, gate, pre_sparse_mds);

        // Partial rounds
        let constants = &spec.constants.partial;
        for (constant, sparse_mds) in constants.iter().zip(sparse_matrices.iter()) {
            self.sbox_part(ctx, gate, constant);
            self.apply_sparse_mds(ctx, gate, sparse_mds);
        }

        // Second half of the full rounds
        let constants = &spec.constants.end;
        for constants in constants.iter() {
            self.sbox_full(ctx, gate, constants);
            self.apply_mds(ctx, gate, mds);
        }
        self.sbox_full(ctx, gate, &[F::ZERO; T]);
        self.apply_mds(ctx, gate, mds);
    }

    /// Constrains and set self to a specific state if `selector` is true.
    pub fn select(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        selector: SafeBool<F>,
        set_to: &Self,
    ) {
        for i in 0..T {
            self.s[i] = gate.select(ctx, set_to.s[i], self.s[i], *selector.as_ref());
        }
    }

    fn x_power5_with_constant(
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        x: AssignedValue<F>,
        constant: &F,
    ) -> AssignedValue<F> {
        let x2 = gate.mul(ctx, x, x);
        let x4 = gate.mul(ctx, x2, x2);
        gate.mul_add(ctx, x, x4, Constant(*constant))
    }

    fn sbox_full(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        constants: &[F; T],
    ) {
        for (x, constant) in self.s.iter_mut().zip(constants.iter()) {
            *x = Self::x_power5_with_constant(ctx, gate, *x, constant);
        }
    }

    fn sbox_part(&mut self, ctx: &mut Context<F>, gate: &impl GateInstructions<F>, constant: &F) {
        let x = &mut self.s[0];
        *x = Self::x_power5_with_constant(ctx, gate, *x, constant);
    }

    fn absorb_with_pre_constants(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        inputs: &[AssignedValue<F>],
        pre_constants: &[F; T],
    ) {
        assert!(inputs.len() < T);

        // Explanation of what's going on: before each round of the poseidon permutation,
        // two things have to be added to the state: inputs (the absorbed elements) and
        // preconstants. Imagine the state as a list of T elements, the first of which is
        // the capacity:  |--cap--|--el1--|--el2--|--elR--|
        // - A preconstant is added to each of all T elements (which is different for each)
        // - The inputs are added to all elements starting from el1 (so, not to the capacity),
        //   to as many elements as inputs are available.
        // - To the first element for which no input is left (if any), an extra 1 is added.

        // adding preconstant to the distinguished capacity element (only one)
        self.s[0] = gate.add(ctx, self.s[0], Constant(pre_constants[0]));

        // adding pre-constants and inputs to the elements for which both are available
        for ((x, constant), input) in
            self.s.iter_mut().zip(pre_constants.iter()).skip(1).zip(inputs.iter())
        {
            *x = gate.sum(ctx, [Existing(*x), Existing(*input), Constant(*constant)]);
        }

        let offset = inputs.len() + 1;
        // adding only pre-constants when no input is left
        for (i, (x, constant)) in
            self.s.iter_mut().zip(pre_constants.iter()).skip(offset).enumerate()
        {
            *x = gate.add(ctx, *x, Constant(if i == 0 { F::ONE + constant } else { *constant }));
            // the if idx == 0 { F::one() } else { F::zero() } is to pad the input with a single 1 and then 0s
            // this is the padding suggested in pg 31 of https://eprint.iacr.org/2019/458.pdf and in Section 4.2 (Variable-Input-Length Hashing. The padding consists of one field element being 1, and the remaining elements being 0.)
        }
    }

    /// Absorb inputs with a variable length.
    ///
    /// `inputs` is right padded.
    fn absorb_var_len_with_pre_constants(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        inputs: [AssignedValue<F>; RATE],
        len: AssignedValue<F>,
        pre_constants: &[F; T],
    ) {
        // Explanation of what's going on: before each round of the poseidon permutation,
        // two things have to be added to the state: inputs (the absorbed elements) and
        // preconstants. Imagine the state as a list of T elements, the first of which is
        // the capacity:  |--cap--|--el1--|--el2--|--elR--|
        // - A preconstant is added to each of all T elements (which is different for each)
        // - The inputs are added to all elements starting from el1 (so, not to the capacity),
        //   to as many elements as inputs are available.
        // - To the first element for which no input is left (if any), an extra 1 is added.

        // Adding preconstants to the current state.
        for (i, pre_const) in pre_constants.iter().enumerate() {
            self.s[i] = gate.add(ctx, self.s[i], Constant(*pre_const));
        }

        // Generate a mask array where a[i] = i < len for i = 0..RATE.
        let idx = gate.dec(ctx, len);
        let len_indicator = gate.idx_to_indicator(ctx, idx, RATE);
        // inputs_mask[i] = sum(len_indicator[i..])
        let mut inputs_mask =
            gate.partial_sums(ctx, len_indicator.clone().into_iter().rev()).collect_vec();
        inputs_mask.reverse();

        let padded_inputs = inputs
            .iter()
            .zip(inputs_mask.iter())
            .map(|(input, mask)| gate.mul(ctx, *input, *mask))
            .collect_vec();
        for i in 0..RATE {
            // Add all inputs.
            self.s[i + 1] = gate.add(ctx, self.s[i + 1], padded_inputs[i]);
            // Add the extra 1 after inputs.
            if i + 2 < T {
                self.s[i + 2] = gate.add(ctx, self.s[i + 2], len_indicator[i]);
            }
        }
        // If len == 0, inputs_mask is all 0. Then the extra 1 should be added into s[1].
        let empty_extra_one = gate.not(ctx, inputs_mask[0]);
        self.s[1] = gate.add(ctx, self.s[1], empty_extra_one);
    }

    fn apply_mds(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        mds: &[[F; T]; T],
    ) {
        let res = mds
            .iter()
            .map(|row| {
                gate.inner_product(ctx, self.s.iter().copied(), row.iter().map(|c| Constant(*c)))
            })
            .collect::<Vec<_>>();

        self.s = res.try_into().unwrap();
    }

    fn apply_sparse_mds(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        mds: &SparseMDSMatrix<F, T, RATE>,
    ) {
        self.s = iter::once(gate.inner_product(
            ctx,
            self.s.iter().copied(),
            mds.row.iter().map(|c| Constant(*c)),
        ))
        .chain(
            mds.col_hat
                .iter()
                .zip(self.s.iter().skip(1))
                .map(|(coeff, state)| gate.mul_add(ctx, self.s[0], Constant(*coeff), *state)),
        )
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    }
}
