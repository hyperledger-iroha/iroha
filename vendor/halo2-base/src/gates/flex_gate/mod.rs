use crate::{
    halo2_proofs::{
        plonk::{
            Advice, Assigned, Column, ConstraintSystem, FirstPhase, Fixed, SecondPhase, Selector,
            ThirdPhase,
        },
        poly::Rotation,
    },
    utils::ScalarField,
    AssignedValue, Context,
    QuantumCell::{self, Constant, Existing, Witness, WitnessFraction},
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    iter::{self},
    marker::PhantomData,
};

pub mod threads;

/// Vector of thread advice column break points
pub type ThreadBreakPoints = Vec<usize>;
/// Vector of vectors tracking the thread break points across different halo2 phases
pub type MultiPhaseThreadBreakPoints = Vec<ThreadBreakPoints>;

/// The maximum number of phases in halo2.
pub(super) const MAX_PHASE: usize = 3;

/// # Vertical Gate Strategy:
/// `q_0 * (a + b * c - d) = 0`
/// where
/// * `a = value[0], b = value[1], c = value[2], d = value[3]`
/// * `q = q_enable[0]`
/// * `q` is either 0 or 1 so this is just a simple selector
///
/// We chose `a + b * c` instead of `a * b + c` to allow "chaining" of gates, i.e., the output of one gate because `a` in the next gate.
///
/// A configuration for a basic gate chip describing the selector, and advice column values.
#[derive(Clone, Debug)]
pub struct BasicGateConfig<F: ScalarField> {
    /// [Selector] column that stores selector values that are used to activate gates in the advice column.
    // `q_enable` will have either length 1 or 2, depending on the strategy
    pub q_enable: Selector,
    /// [Column] that stores the advice values of the gate.
    pub value: Column<Advice>,
    /// Marker for the field type.
    _marker: PhantomData<F>,
}

impl<F: ScalarField> BasicGateConfig<F> {
    /// Constructor
    pub fn new(q_enable: Selector, value: Column<Advice>) -> Self {
        Self { q_enable, value, _marker: PhantomData }
    }

    /// Instantiates a new [BasicGateConfig].
    ///
    /// Assumes `phase` is in the range [0, MAX_PHASE).
    /// * `meta`: [ConstraintSystem] used for the gate
    /// * `phase`: The phase to add the gate to
    pub fn configure(meta: &mut ConstraintSystem<F>, phase: u8) -> Self {
        let value = match phase {
            0 => meta.advice_column_in(FirstPhase),
            1 => meta.advice_column_in(SecondPhase),
            2 => meta.advice_column_in(ThirdPhase),
            _ => panic!("Currently BasicGate only supports {MAX_PHASE} phases"),
        };
        meta.enable_equality(value);

        let q_enable = meta.selector();

        let config = Self { q_enable, value, _marker: PhantomData };
        config.create_gate(meta);
        config
    }

    /// Wrapper for [ConstraintSystem].create_gate(name, meta) creates a gate form [q * (a + b * c - out)].
    /// * `meta`: [ConstraintSystem] used for the gate
    fn create_gate(&self, meta: &mut ConstraintSystem<F>) {
        meta.create_gate("1 column a + b * c = out", |meta| {
            let q = meta.query_selector(self.q_enable);

            let a = meta.query_advice(self.value, Rotation::cur());
            let b = meta.query_advice(self.value, Rotation::next());
            let c = meta.query_advice(self.value, Rotation(2));
            let out = meta.query_advice(self.value, Rotation(3));

            vec![q * (a + b * c - out)]
        })
    }
}

/// A Config struct defining the parameters for [FlexGateConfig]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct FlexGateConfigParams {
    /// Specifies the number of rows in the circuit to be 2<sup>k</sup>
    pub k: usize,
    /// The number of advice columns per phase
    pub num_advice_per_phase: Vec<usize>,
    /// The number of fixed columns
    pub num_fixed: usize,
}

/// Defines a configuration for a flex gate chip describing the selector, and advice column values for the chip.
#[derive(Clone, Debug)]
pub struct FlexGateConfig<F: ScalarField> {
    /// A [Vec] of [BasicGateConfig] that define gates for each halo2 phase.
    pub basic_gates: Vec<Vec<BasicGateConfig<F>>>,
    /// A [Vec] of [Fixed] [Column]s for allocating constant values.
    pub constants: Vec<Column<Fixed>>,
    /// Max number of usable rows in the circuit.
    pub max_rows: usize,
}

impl<F: ScalarField> FlexGateConfig<F> {
    /// Generates a new [FlexGateConfig]
    ///
    /// * `meta`: [ConstraintSystem] of the circuit
    /// * `params`: see [FlexGateConfigParams]
    pub fn configure(meta: &mut ConstraintSystem<F>, params: FlexGateConfigParams) -> Self {
        // create fixed (constant) columns and enable equality constraints
        let mut constants = Vec::with_capacity(params.num_fixed);
        for _i in 0..params.num_fixed {
            let c = meta.fixed_column();
            meta.enable_equality(c);
            // meta.enable_constant(c);
            constants.push(c);
        }

        let mut basic_gates = vec![];
        for (phase, &num_columns) in params.num_advice_per_phase.iter().enumerate() {
            let config =
                (0..num_columns).map(|_| BasicGateConfig::configure(meta, phase as u8)).collect();
            basic_gates.push(config);
        }
        log::debug!("Poisoned rows after FlexGateConfig::configure {}", meta.minimum_rows());
        Self {
            basic_gates,
            constants,
            // Warning: this needs to be updated if you create more advice columns after this `FlexGateConfig` is created
            max_rows: (1 << params.k) - meta.minimum_rows(),
        }
    }
}

/// Trait that defines basic arithmetic operations for a gate.
pub trait GateInstructions<F: ScalarField> {
    /// Returns a slice of the [ScalarField] field elements 2^i for i in 0..F::NUM_BITS.
    fn pow_of_two(&self) -> &[F];

    /// Constrains and returns `a + b * 1 = out`.
    ///
    /// Defines a vertical gate of form | a | b | 1 | a + b | where (a + b) = out.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `b`: [QuantumCell] value to add to 'a`
    fn add(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let out_val = *a.value() + b.value();
        ctx.assign_region_last([a, b, Constant(F::ONE), Witness(out_val)], [0])
    }

    /// Constrains and returns `out = a + 1`.
    ///
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    fn inc(&self, ctx: &mut Context<F>, a: impl Into<QuantumCell<F>>) -> AssignedValue<F> {
        self.add(ctx, a, Constant(F::ONE))
    }

    /// Constrains and returns `a + b * (-1) = out`.
    ///
    /// Defines a vertical gate of form | a - b | b | 1 | a |, where (a - b) = out.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `b`: [QuantumCell] value to subtract from 'a'
    fn sub(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let out_val = *a.value() - b.value();
        // slightly better to not have to compute -F::ONE since F::ONE is cached
        ctx.assign_region([Witness(out_val), b, Constant(F::ONE), a], [0]);
        ctx.get(-4)
    }

    /// Constrains and returns `out = a - 1`.
    ///
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    fn dec(&self, ctx: &mut Context<F>, a: impl Into<QuantumCell<F>>) -> AssignedValue<F> {
        self.sub(ctx, a, Constant(F::ONE))
    }

    /// Constrains and returns  `a - b * c = out`.
    ///
    /// Defines a vertical gate of form | a - b * c | b | c | a |, where (a - b * c) = out.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value to subtract 'b * c' from
    /// * `b`: [QuantumCell] value
    /// * `c`: [QuantumCell] value
    fn sub_mul(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        c: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let c = c.into();
        let out_val = *a.value() - *b.value() * c.value();
        ctx.assign_region_last([Witness(out_val), b, c, a], [0]);
        ctx.get(-4)
    }

    /// Constrains and returns `a * (-1) = out`.
    ///
    /// Defines a vertical gate of form | a | -a | 1 | 0 |, where (-a) = out.
    /// * `ctx`: the [Context] to add the constraints to
    /// * `a`: [QuantumCell] value to negate
    fn neg(&self, ctx: &mut Context<F>, a: impl Into<QuantumCell<F>>) -> AssignedValue<F> {
        let a = a.into();
        let out_val = -*a.value();
        ctx.assign_region([a, Witness(out_val), Constant(F::ONE), Constant(F::ZERO)], [0]);
        ctx.get(-3)
    }

    /// Constrains and returns  `0 + a * b = out`.
    ///
    /// Defines a vertical gate of form | 0 | a | b | a * b |, where (a * b) = out.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `b`: [QuantumCell] value to multiply 'a' by
    fn mul(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let out_val = *a.value() * b.value();
        ctx.assign_region_last([Constant(F::ZERO), a, b, Witness(out_val)], [0])
    }

    /// Constrains and returns  `a * b + c = out`.
    ///
    /// Defines a vertical gate of form | c | a | b | a * b + c |, where (a * b + c) = out.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `b`: [QuantumCell] value to multiply 'a' by
    /// * `c`: [QuantumCell] value to add to 'a * b'
    fn mul_add(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        c: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let c = c.into();
        let out_val = *a.value() * b.value() + c.value();
        ctx.assign_region_last([c, a, b, Witness(out_val)], [0])
    }

    /// Constrains and returns `(1 - a) * b = b - a * b`.
    ///
    /// Defines a vertical gate of form | (1 - a) * b | a | b | b |, where (1 - a) * b = out.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `b`: [QuantumCell] value to multiply 'a' by
    fn mul_not(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let out_val = (F::ONE - a.value()) * b.value();
        ctx.assign_region_smart([Witness(out_val), a, b, b], [0], [(2, 3)], []);
        ctx.get(-4)
    }

    /// Constrains that x is boolean (e.g. 0 or 1).
    ///
    /// Defines a vertical gate of form | 0 | x | x | x |.
    /// * `ctx`: [Context] to add the constraints to
    /// * `x`: [QuantumCell] value to constrain
    fn assert_bit(&self, ctx: &mut Context<F>, x: AssignedValue<F>) {
        ctx.assign_region([Constant(F::ZERO), Existing(x), Existing(x), Existing(x)], [0]);
    }

    /// Constrains and returns a / b = out.
    ///
    /// Defines a vertical gate of form | 0 | a / b | b | a |, where a / b = out.
    ///
    /// Assumes `b != 0`.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `b`: [QuantumCell] value to divide 'a' by
    fn div_unsafe(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        // TODO: if really necessary, make `c` of type `Assigned<F>`
        // this would require the API using `Assigned<F>` instead of `F` everywhere, so leave as last resort
        let c = b.value().invert().unwrap() * a.value();
        ctx.assign_region([Constant(F::ZERO), Witness(c), b, a], [0]);
        ctx.get(-3)
    }

    /// Constrains that `a` is equal to `constant` value.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `constant`: constant value to constrain `a` to be equal to
    fn assert_is_const(&self, ctx: &mut Context<F>, a: &AssignedValue<F>, constant: &F) {
        if !ctx.witness_gen_only {
            ctx.copy_manager.lock().unwrap().constant_equalities.push((*constant, a.cell.unwrap()));
        }
    }

    /// Constrains and returns the inner product of `<a, b>`.
    ///
    /// Assumes 'a' and 'b' are the same length.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell] values
    /// * `b`: Iterator of [QuantumCell] values to take inner product of `a` by
    fn inner_product<QA>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> AssignedValue<F>
    where
        QA: Into<QuantumCell<F>>;

    /// Returns the inner product of `<a, b>` and the last element of `a` after it has been assigned.
    ///
    /// **NOT** encouraged for general usage.
    /// This is a low-level function, where you want to avoid first assigning `a` and then copying the last element into the
    /// correct cell for this computation.
    ///
    /// Assumes 'a' and 'b' are the same length.
    /// * `ctx`: [Context] of the circuit
    /// * `a`: Iterator of [QuantumCell]s
    /// * `b`: Iterator of [QuantumCell]s to take inner product of `a` by
    fn inner_product_left_last<QA>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> (AssignedValue<F>, AssignedValue<F>)
    where
        QA: Into<QuantumCell<F>>;

    /// Returns `(<a,b>, a_assigned)`. See `inner_product` for more details.
    ///
    /// **NOT** encouraged for general usage.
    /// This is a low-level function, useful for when you want to simultaneously compute an inner product while assigning
    /// private witnesses for the first time. This avoids first assigning `a` and then copying into the correct cells
    /// for this computation. We do not return the assignments of `a` in `inner_product` as an optimization to avoid
    /// the memory allocation of having to collect the vectors.
    ///
    /// Assumes 'a' and 'b' are the same length.
    fn inner_product_left<QA>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> (AssignedValue<F>, Vec<AssignedValue<F>>)
    where
        QA: Into<QuantumCell<F>>;

    /// Calculates and constrains the inner product.
    ///
    /// Returns the assignment trace where `output[i]` has the running sum `sum_{j=0..=i} a[j] * b[j]`.
    ///
    /// Assumes 'a' and 'b' are the same length.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell] values
    /// * `b`: Iterator of [QuantumCell] values to calculate the partial sums of the inner product of `a` by.
    fn inner_product_with_sums<'thread, QA>(
        &self,
        ctx: &'thread mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> Box<dyn Iterator<Item = AssignedValue<F>> + 'thread>
    where
        QA: Into<QuantumCell<F>>;

    /// Constrains and returns the sum of [QuantumCell]'s in iterator `a`.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell] values to sum
    fn sum<Q>(&self, ctx: &mut Context<F>, a: impl IntoIterator<Item = Q>) -> AssignedValue<F>
    where
        Q: Into<QuantumCell<F>>,
    {
        let mut a = a.into_iter().peekable();
        let start = a.next();
        if start.is_none() {
            return ctx.load_zero();
        }
        let start = start.unwrap().into();
        if a.peek().is_none() {
            return ctx.assign_region_last([start], []);
        }
        let (len, hi) = a.size_hint();
        assert_eq!(Some(len), hi);

        let mut sum = *start.value();
        let cells = iter::once(start).chain(a.flat_map(|a| {
            let a = a.into();
            sum += a.value();
            [a, Constant(F::ONE), Witness(sum)]
        }));
        ctx.assign_region_last(cells, (0..len).map(|i| 3 * i as isize))
    }

    /// Calculates and constrains the sum of the elements of `a`.
    ///
    /// Returns the assignment trace where `output[i]` has the running sum `sum_{j=0..=i} a[j]`.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell] values to sum
    fn partial_sums<'thread, Q>(
        &self,
        ctx: &'thread mut Context<F>,
        a: impl IntoIterator<Item = Q>,
    ) -> Box<dyn Iterator<Item = AssignedValue<F>> + 'thread>
    where
        Q: Into<QuantumCell<F>>,
    {
        let mut a = a.into_iter().peekable();
        let start = a.next();
        if start.is_none() {
            return Box::new(iter::once(ctx.load_zero()));
        }
        let start = start.unwrap().into();
        if a.peek().is_none() {
            return Box::new(iter::once(ctx.assign_region_last([start], [])));
        }
        let (len, hi) = a.size_hint();
        assert_eq!(Some(len), hi);

        let mut sum = *start.value();
        let cells = iter::once(start).chain(a.flat_map(|a| {
            let a = a.into();
            sum += a.value();
            [a, Constant(F::ONE), Witness(sum)]
        }));
        ctx.assign_region(cells, (0..len).map(|i| 3 * i as isize));
        Box::new((0..=len).rev().map(|i| ctx.get(-1 - 3 * (i as isize))))
    }

    /// Calculates and constrains the accumulated product of 'a' and 'b' i.e. `x_i = b_1 * (a_1...a_{i - 1})
    ///     + b_2 * (a_2...a_{i - 1})
    ///     + ...
    ///     + b_i`
    ///
    /// Returns the assignment trace where `output[i]` is the running accumulated product x_i.
    ///
    /// Assumes 'a' and 'b' are the same length.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell] values
    /// * `b`: Iterator of [QuantumCell] values to take the accumulated product of `a` by
    fn accumulated_product<QA, QB>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QB>,
    ) -> Vec<AssignedValue<F>>
    where
        QA: Into<QuantumCell<F>>,
        QB: Into<QuantumCell<F>>,
    {
        let mut b = b.into_iter();
        let mut a = a.into_iter();
        let b_first = b.next();
        if let Some(b_first) = b_first {
            let b_first = ctx.assign_region_last([b_first], []);
            std::iter::successors(Some(b_first), |x| {
                a.next().zip(b.next()).map(|(a, b)| self.mul_add(ctx, Existing(*x), a, b))
            })
            .collect()
        } else {
            vec![]
        }
    }

    /// Constrains and returns the sum of products of `coeff * (a * b)` defined in `values` plus a variable `var` e.g.
    /// `x = var + values[0].0 * (values[0].1 * values[0].2) + values[1].0 * (values[1].1 * values[1].2) + ... + values[n].0 * (values[n].1 * values[n].2)`.
    /// * `ctx`: [Context] to add the constraints to.
    /// * `values`: Iterator of tuples `(coeff, a, b)` where `coeff` is a field element, `a` and `b` are [QuantumCell]'s.
    /// * `var`: [QuantumCell] that represents the value of a variable added to the sum.
    fn sum_products_with_coeff_and_var(
        &self,
        ctx: &mut Context<F>,
        values: impl IntoIterator<Item = (F, QuantumCell<F>, QuantumCell<F>)>,
        var: QuantumCell<F>,
    ) -> AssignedValue<F>;

    /// Constrains and returns `a || b`, assuming `a` and `b` are boolean.
    ///
    /// Defines a vertical gate of form `| 1 - b | 1 | b | 1 | b | a | 1 - b | out |`, where `out = a + b - a * b`.
    /// * `ctx`: [Context] to add the constraints to.
    /// * `a`: [QuantumCell] that contains a boolean value.
    /// * `b`: [QuantumCell] that contains a boolean value.
    fn or(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let not_b_val = F::ONE - b.value();
        let out_val = *a.value() + b.value() - *a.value() * b.value();
        let cells = [
            Witness(not_b_val),
            Constant(F::ONE),
            b,
            Constant(F::ONE),
            b,
            a,
            Witness(not_b_val),
            Witness(out_val),
        ];
        ctx.assign_region_smart(cells, [0, 4], [(0, 6), (2, 4)], []);
        ctx.last().unwrap()
    }

    /// Constrains and returns `a & b`, assumeing `a` and `b` are boolean.
    ///
    /// Defines a vertical gate of form | 0 | a | b | out |, where out = a * b.
    /// * `ctx`: [Context] to add the constraints to.
    /// * `a`: [QuantumCell] that contains a boolean value.
    /// * `b`: [QuantumCell] that contains a boolean value.
    fn and(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        self.mul(ctx, a, b)
    }

    /// Constrains and returns `!a` assumeing `a` is boolean.
    ///
    /// Defines a vertical gate of form | 1 - a | a | 1 | 1 |, where 1 - a = out.
    /// * `ctx`: [Context] to add the constraints to.
    /// * `a`: [QuantumCell] that contains a boolean value.
    fn not(&self, ctx: &mut Context<F>, a: impl Into<QuantumCell<F>>) -> AssignedValue<F> {
        self.sub(ctx, Constant(F::ONE), a)
    }

    /// Constrains and returns `sel ? a : b` assuming `sel` is boolean.
    ///
    /// Defines a vertical gate of form `| 1 - sel | sel | 1 | a | 1 - sel | sel | 1 | b | out |`, where out = sel * a + (1 - sel) * b.
    /// * `ctx`: [Context] to add the constraints to.
    /// * `a`: [QuantumCell] that contains a boolean value.
    /// * `b`: [QuantumCell] that contains a boolean value.
    /// * `sel`: [QuantumCell] that contains a boolean value.
    fn select(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        sel: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F>;

    /// Constains and returns `a || (b && c)`, assuming `a`, `b` and `c` are boolean.
    ///
    /// Defines a vertical gate of form `| 1 - b c | b | c | 1 | a - 1 | 1 - b c | out | a - 1 | 1 | 1 | a |`, where out = a + b * c - a * b * c.
    /// * `ctx`: [Context] to add the constraints to.
    /// * `a`: [QuantumCell] that contains a boolean value.
    /// * `b`: [QuantumCell] that contains a boolean value.
    /// * `c`: [QuantumCell] that contains a boolean value.
    fn or_and(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        c: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F>;

    /// Constrains and returns an indicator vector from a slice of boolean values, where `output[idx] = 1` iff idx = (the number represented by `bits` in binary little endian), otherwise `output[idx] = 0`.
    /// * `ctx`: [Context] to add the constraints to
    /// * `bits`: slice of [QuantumCell]'s that contains boolean values
    ///
    /// # Assumptions
    /// * `bits` is non-empty
    fn bits_to_indicator(
        &self,
        ctx: &mut Context<F>,
        bits: &[AssignedValue<F>],
    ) -> Vec<AssignedValue<F>> {
        let k = bits.len();
        assert!(k > 0, "bits_to_indicator: bits must be non-empty");

        // (inv_last_bit, last_bit) = (1, 0) if bits[k - 1] = 0
        let (inv_last_bit, last_bit) = {
            ctx.assign_region(
                [
                    Witness(F::ONE - bits[k - 1].value()),
                    Existing(bits[k - 1]),
                    Constant(F::ONE),
                    Constant(F::ONE),
                ],
                [0],
            );
            (ctx.get(-4), ctx.get(-3))
        };
        let mut indicator = Vec::with_capacity(2 * (1 << k) - 2);
        let mut offset = 0;
        indicator.push(inv_last_bit);
        indicator.push(last_bit);
        for (idx, bit) in bits.iter().rev().enumerate().skip(1) {
            for old_idx in 0..(1 << idx) {
                // inv_prod_val = (1 - bit) * indicator[offset + old_idx]
                let inv_prod_val = (F::ONE - bit.value()) * indicator[offset + old_idx].value();
                ctx.assign_region(
                    [
                        Witness(inv_prod_val),
                        Existing(indicator[offset + old_idx]),
                        Existing(*bit),
                        Existing(indicator[offset + old_idx]),
                    ],
                    [0],
                );
                indicator.push(ctx.get(-4));

                // prod = bit * indicator[offset + old_idx]
                let prod = self.mul(ctx, Existing(indicator[offset + old_idx]), Existing(*bit));
                indicator.push(prod);
            }
            offset += 1 << idx;
        }
        indicator.split_off((1 << k) - 2)
    }

    /// Constrains and returns a [Vec] `indicator` of length `len`, where `indicator[i] == 1 if i == idx otherwise 0`, if `idx >= len` then `indicator` is all zeros.
    ///
    /// Assumes `len` is greater than 0.
    /// * `ctx`: [Context] to add the constraints to
    /// * `idx`: [QuantumCell]  index of the indicator vector to be set to 1
    /// * `len`: length of the `indicator` vector
    fn idx_to_indicator(
        &self,
        ctx: &mut Context<F>,
        idx: impl Into<QuantumCell<F>>,
        len: usize,
    ) -> Vec<AssignedValue<F>> {
        let mut idx = idx.into();
        (0..len)
            .map(|i| {
                // need to use assigned idx after i > 0 so equality constraint holds
                if i == 0 {
                    // unroll `is_zero` to make sure if `idx == Witness(_)` it is replaced by `Existing(_)` in later iterations
                    let x = idx.value();
                    let (is_zero, inv) = if x.is_zero_vartime() {
                        (F::ONE, Assigned::Trivial(F::ONE))
                    } else {
                        (F::ZERO, Assigned::Rational(F::ONE, *x))
                    };
                    let cells = [
                        Witness(is_zero),
                        idx,
                        WitnessFraction(inv),
                        Constant(F::ONE),
                        Constant(F::ZERO),
                        idx,
                        Witness(is_zero),
                        Constant(F::ZERO),
                    ];
                    ctx.assign_region_smart(cells, [0, 4], [(0, 6), (1, 5)], []); // note the two `idx` need to be constrained equal: (1, 5)
                    idx = Existing(ctx.get(-3)); // replacing `idx` with Existing cell so future loop iterations constrain equality of all `idx`s
                    ctx.get(-2)
                } else {
                    self.is_equal(ctx, idx, Constant(F::from(i as u64)))
                }
            })
            .collect()
    }

    /// Constrains the inner product of `a` and `indicator` and returns `a[idx]` (e.g. the value of `a` at `idx`).
    ///
    /// Assumes that `a` and `indicator` are non-empty iterators of the same length, the values of `indicator` are boolean,
    /// and that `indicator` has at most one `1` bit.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell]'s that contains field elements
    /// * `indicator`: Iterator of [AssignedValue]'s where `indicator[i] == 1` if `i == idx`, otherwise `0`
    fn select_by_indicator<Q>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = Q>,
        indicator: impl IntoIterator<Item = AssignedValue<F>>,
    ) -> AssignedValue<F>
    where
        Q: Into<QuantumCell<F>>,
    {
        let mut sum = F::ZERO;
        let a = a.into_iter();
        let (len, hi) = a.size_hint();
        assert_eq!(Some(len), hi);

        let cells =
            std::iter::once(Constant(F::ZERO)).chain(a.zip(indicator).flat_map(|(a, ind)| {
                let a = a.into();
                sum = if ind.value().is_zero_vartime() { sum } else { *a.value() };
                [a, Existing(ind), Witness(sum)]
            }));
        ctx.assign_region_last(cells, (0..len).map(|i| 3 * i as isize))
    }

    /// Constrains and returns `cells[idx]` if `idx < cells.len()`, otherwise return 0.
    ///
    /// Assumes that `cells` and `idx` are non-empty iterators of the same length.
    /// * `ctx`: [Context] to add the constraints to
    /// * `cells`: Iterator of [QuantumCell]s to select from
    /// * `idx`: [QuantumCell] with value `idx` where `idx` is the index of the cell to be selected
    fn select_from_idx<Q>(
        &self,
        ctx: &mut Context<F>,
        cells: impl IntoIterator<Item = Q>,
        idx: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F>
    where
        Q: Into<QuantumCell<F>>,
    {
        let cells = cells.into_iter();
        let (len, hi) = cells.size_hint();
        assert_eq!(Some(len), hi);

        let ind = self.idx_to_indicator(ctx, idx, len);
        self.select_by_indicator(ctx, cells, ind)
    }

    /// `array2d` is an array of fixed length arrays.
    /// Assumes:
    /// * `array2d.len() == indicator.len()`
    /// * `array2d[i].len() == array2d[j].len()` for all `i,j`.
    /// * the values of `indicator` are boolean and that `indicator` has at most one `1` bit.
    /// * the lengths of `array2d` and `indicator` are the same.
    ///
    /// Returns the "dot product" of `array2d` with `indicator` as a fixed length (1d) array of length `array2d[0].len()`.
    fn select_array_by_indicator<AR, AV>(
        &self,
        ctx: &mut Context<F>,
        array2d: &[AR],
        indicator: &[AssignedValue<F>],
    ) -> Vec<AssignedValue<F>>
    where
        AR: AsRef<[AV]>,
        AV: AsRef<AssignedValue<F>>,
    {
        (0..array2d[0].as_ref().len())
            .map(|j| {
                self.select_by_indicator(
                    ctx,
                    array2d.iter().map(|array_i| *array_i.as_ref()[j].as_ref()),
                    indicator.iter().copied(),
                )
            })
            .collect()
    }

    /// Constrains that a cell is equal to 0 and returns `1` if `a = 0`, otherwise `0`.
    ///
    /// Defines a vertical gate of form `| out | a | inv | 1 | 0 | a | out | 0 |`, where out = 1 if a = 0, otherwise out = 0.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value to be constrained
    fn is_zero(&self, ctx: &mut Context<F>, a: AssignedValue<F>) -> AssignedValue<F> {
        let x = a.value();
        let (is_zero, inv) = if x.is_zero_vartime() {
            (F::ONE, Assigned::Trivial(F::ONE))
        } else {
            (F::ZERO, Assigned::Rational(F::ONE, *x))
        };

        let cells = [
            Witness(is_zero),
            Existing(a),
            WitnessFraction(inv),
            Constant(F::ONE),
            Constant(F::ZERO),
            Existing(a),
            Witness(is_zero),
            Constant(F::ZERO),
        ];
        ctx.assign_region_smart(cells, [0, 4], [(0, 6)], []);
        ctx.get(-2)
    }

    /// Constrains that the value of two cells are equal: b - a = 0, returns `1` if `a = b`, otherwise `0`.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] value
    /// * `b`: [QuantumCell] value to compare to `a`
    fn is_equal(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let diff = self.sub(ctx, a, b);
        self.is_zero(ctx, diff)
    }

    /// Constrains and returns little-endian bit vector representation of `a`.
    ///
    /// Assumes `range_bits >= bit_length(a)`.
    /// * `a`: [QuantumCell] of the value to convert
    /// * `range_bits`: range of bits needed to represent `a`
    fn num_to_bits(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        range_bits: usize,
    ) -> Vec<AssignedValue<F>>;

    /// Constrains and computes `a`<sup>`exp`</sup> where both `a, exp` are witnesses. The exponent is computed in the native field `F`.
    ///
    /// Constrains that `exp` has at most `max_bits` bits.
    fn pow_var(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        exp: AssignedValue<F>,
        max_bits: usize,
    ) -> AssignedValue<F>;

    /// Performs and constrains Lagrange interpolation on `coords` and evaluates the resulting polynomial at `x`.
    ///
    /// Given pairs `coords[i] = (x_i, y_i)`, let `f` be the unique degree `len(coords) - 1` polynomial such that `f(x_i) = y_i` for all `i`.
    ///
    /// Returns:
    /// (f(x), Prod_i(x - x_i))
    /// * `ctx`: [Context] to add the constraints to
    /// * `coords`: immutable reference to a slice of tuples of [AssignedValue]s representing the points to interpolate over such that `coords[i] = (x_i, y_i)`
    /// * `x`: x-coordinate of the point to evaluate `f` at
    ///
    /// # Assumptions
    /// * `coords` is non-empty
    fn lagrange_and_eval(
        &self,
        ctx: &mut Context<F>,
        coords: &[(AssignedValue<F>, AssignedValue<F>)],
        x: AssignedValue<F>,
    ) -> (AssignedValue<F>, AssignedValue<F>) {
        assert!(!coords.is_empty(), "coords should not be empty");
        let mut z = self.sub(ctx, Existing(x), Existing(coords[0].0));
        for coord in coords.iter().skip(1) {
            let sub = self.sub(ctx, Existing(x), Existing(coord.0));
            z = self.mul(ctx, Existing(z), Existing(sub));
        }
        let mut eval = None;
        for i in 0..coords.len() {
            // compute (x - x_i) * Prod_{j != i} (x_i - x_j)
            let mut denom = self.sub(ctx, Existing(x), Existing(coords[i].0));
            for j in 0..coords.len() {
                if i == j {
                    continue;
                }
                let sub = self.sub(ctx, coords[i].0, coords[j].0);
                denom = self.mul(ctx, denom, sub);
            }
            // TODO: batch inversion
            let is_zero = self.is_zero(ctx, denom);
            self.assert_is_const(ctx, &is_zero, &F::ZERO);

            // y_i / denom
            let quot = self.div_unsafe(ctx, coords[i].1, denom);
            eval = if let Some(eval) = eval {
                let eval = self.add(ctx, eval, quot);
                Some(eval)
            } else {
                Some(quot)
            };
        }
        let out = self.mul(ctx, eval.unwrap(), z);
        (out, z)
    }
}

/// A chip that implements the [GateInstructions] trait supporting basic arithmetic operations.
#[derive(Clone, Debug)]
pub struct GateChip<F: ScalarField> {
    /// The field elements 2^i for i in 0..F::NUM_BITS.
    pub pow_of_two: Vec<F>,
    /// To avoid Montgomery conversion in `F::from` for common small numbers, we keep a cache of field elements.
    pub field_element_cache: Vec<F>,
}

impl<F: ScalarField> Default for GateChip<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: ScalarField> GateChip<F> {
    /// Returns a new [GateChip] with some precomputed values. This can be called out of circuit and has no extra dependencies.
    pub fn new() -> Self {
        let mut pow_of_two = Vec::with_capacity(F::NUM_BITS as usize);
        let two = F::from(2);
        pow_of_two.push(F::ONE);
        pow_of_two.push(two);
        for _ in 2..F::NUM_BITS {
            pow_of_two.push(two * pow_of_two.last().unwrap());
        }
        let field_element_cache = (0..1024).map(|i| F::from(i)).collect();

        Self { pow_of_two, field_element_cache }
    }

    /// Calculates and constrains the inner product of `<a, b>`.
    /// If the first element of `b` is `Constant(F::ONE)`, then an optimization is performed to save 3 cells.
    ///
    /// Returns `true` if `b` start with `Constant(F::ONE)`, and `false` otherwise.
    ///
    /// Assumes `a` and `b` are the same length.
    /// * `ctx`: [Context] of the circuit
    /// * `a`: Iterator of [QuantumCell] values
    /// * `b`: Iterator of [QuantumCell] values to take inner product of `a` by
    fn inner_product_simple<QA>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> bool
    where
        QA: Into<QuantumCell<F>>,
    {
        let mut sum;
        let mut a = a.into_iter();
        let mut b = b.into_iter().peekable();

        let b_starts_with_one = matches!(b.peek(), Some(Constant(c)) if c == &F::ONE);
        let cells = if b_starts_with_one {
            b.next();
            let start_a = a.next().unwrap().into();
            sum = *start_a.value();
            iter::once(start_a)
        } else {
            sum = F::ZERO;
            iter::once(Constant(F::ZERO))
        }
        .chain(a.zip(b).flat_map(|(a, b)| {
            let a = a.into();
            sum += *a.value() * b.value();
            [a, b, Witness(sum)]
        }));

        if ctx.witness_gen_only() {
            ctx.assign_region(cells, vec![]);
        } else {
            let cells = cells.collect::<Vec<_>>();
            let lo = cells.len();
            let len = lo / 3;
            ctx.assign_region(cells, (0..len).map(|i| 3 * i as isize));
        };
        b_starts_with_one
    }
}

impl<F: ScalarField> GateInstructions<F> for GateChip<F> {
    /// Returns a slice of the [ScalarField] elements 2<sup>i</sup> for i in 0..F::NUM_BITS.
    fn pow_of_two(&self) -> &[F] {
        &self.pow_of_two
    }

    /// Constrains and returns the inner product of `<a, b>`.
    /// If the first element of `b` is `Constant(F::ONE)`, then an optimization is performed to save 3 cells.
    ///
    /// Assumes 'a' and 'b' are the same length.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell] values
    /// * `b`: Iterator of [QuantumCell] values to take inner product of `a` by
    fn inner_product<QA>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> AssignedValue<F>
    where
        QA: Into<QuantumCell<F>>,
    {
        self.inner_product_simple(ctx, a, b);
        ctx.last().unwrap()
    }

    /// Returns the inner product of `<a, b>` and the last element of `a` after it has been assigned.
    ///
    /// **NOT** encouraged for general usage.
    /// This is a low-level function, where you want to avoid first assigning `a` and then copying the last element into the
    /// correct cell for this computation.
    ///
    /// Assumes 'a' and 'b' are the same length.
    /// * `ctx`: [Context] of the circuit
    /// * `a`: Iterator of [QuantumCell]s
    /// * `b`: Iterator of [QuantumCell]s to take inner product of `a` by
    fn inner_product_left_last<QA>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> (AssignedValue<F>, AssignedValue<F>)
    where
        QA: Into<QuantumCell<F>>,
    {
        let a = a.into_iter();
        let (len, hi) = a.size_hint();
        assert_eq!(Some(len), hi);
        let row_offset = ctx.advice.len();
        let b_starts_with_one = self.inner_product_simple(ctx, a, b);
        let a_last = if b_starts_with_one {
            if len == 1 {
                ctx.get(row_offset as isize)
            } else {
                ctx.get((row_offset + 1 + 3 * (len - 2)) as isize)
            }
        } else {
            ctx.get((row_offset + 1 + 3 * (len - 1)) as isize)
        };
        (ctx.last().unwrap(), a_last)
    }

    /// Returns `(<a,b>, a_assigned)`. See `inner_product` for more details.
    ///
    /// **NOT** encouraged for general usage.
    /// This is a low-level function, useful for when you want to simultaneously compute an inner product while assigning
    /// private witnesses for the first time. This avoids first assigning `a` and then copying into the correct cells
    /// for this computation. We do not return the assignments of `a` in `inner_product` as an optimization to avoid
    /// the memory allocation of having to collect the vectors.
    ///
    /// We do not return `b_assigned` because if `b` starts with `Constant(F::ONE)`, the first element of `b` is not assigned.
    ///
    /// Assumes 'a' and 'b' are the same length.
    fn inner_product_left<QA>(
        &self,
        ctx: &mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> (AssignedValue<F>, Vec<AssignedValue<F>>)
    where
        QA: Into<QuantumCell<F>>,
    {
        let a = a.into_iter().collect_vec();
        let len = a.len();
        let row_offset = ctx.advice.len();
        let b_starts_with_one = self.inner_product_simple(ctx, a, b);
        let a_assigned = (0..len)
            .map(|i| {
                if b_starts_with_one {
                    if i == 0 {
                        ctx.get(row_offset as isize)
                    } else {
                        ctx.get((row_offset + 1 + 3 * (i - 1)) as isize)
                    }
                } else {
                    ctx.get((row_offset + 1 + 3 * i) as isize)
                }
            })
            .collect_vec();
        (ctx.last().unwrap(), a_assigned)
    }

    /// Calculates and constrains the inner product.
    ///
    /// Returns the assignment trace where `output[i]` has the running sum `sum_{j=0..=i} a[j] * b[j]`.
    ///
    /// Assumes 'a' and 'b' are the same length.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: Iterator of [QuantumCell] values
    /// * `b`: Iterator of [QuantumCell] values to calculate the partial sums of the inner product of `a` by
    fn inner_product_with_sums<'thread, QA>(
        &self,
        ctx: &'thread mut Context<F>,
        a: impl IntoIterator<Item = QA>,
        b: impl IntoIterator<Item = QuantumCell<F>>,
    ) -> Box<dyn Iterator<Item = AssignedValue<F>> + 'thread>
    where
        QA: Into<QuantumCell<F>>,
    {
        let row_offset = ctx.advice.len();
        let b_starts_with_one = self.inner_product_simple(ctx, a, b);
        if b_starts_with_one {
            Box::new((row_offset..ctx.advice.len()).step_by(3).map(|i| ctx.get(i as isize)))
        } else {
            // in this case the first assignment is 0 so we skip it
            Box::new((row_offset..ctx.advice.len()).step_by(3).skip(1).map(|i| ctx.get(i as isize)))
        }
    }

    /// Constrains and returns the sum of products of `coeff * (a * b)` defined in `values` plus a variable `var` e.g.
    /// `x = var + values[0].0 * (values[0].1 * values[0].2) + values[1].0 * (values[1].1 * values[1].2) + ... + values[n].0 * (values[n].1 * values[n].2)`.
    /// * `ctx`: [Context] to add the constraints to
    /// * `values`: Iterator of tuples `(coeff, a, b)` where `coeff` is a field element, `a` and `b` are [QuantumCell]'s
    /// * `var`: [QuantumCell] that represents the value of a variable added to the sum
    fn sum_products_with_coeff_and_var(
        &self,
        ctx: &mut Context<F>,
        values: impl IntoIterator<Item = (F, QuantumCell<F>, QuantumCell<F>)>,
        var: QuantumCell<F>,
    ) -> AssignedValue<F> {
        // Create an iterator starting with `var` and
        let (a, b): (Vec<_>, Vec<_>) = std::iter::once((var, Constant(F::ONE)))
            .chain(values.into_iter().filter_map(|(c, va, vb)| {
                if c == F::ONE {
                    Some((va, vb))
                } else if c != F::ZERO {
                    let prod = self.mul(ctx, va, vb);
                    Some((QuantumCell::Existing(prod), Constant(c)))
                } else {
                    None
                }
            }))
            .unzip();
        self.inner_product(ctx, a, b)
    }

    /// Constrains and returns `sel ? a : b` assuming `sel` is boolean.
    ///
    /// Defines a vertical gate of form `| 1 - sel | sel | 1 | a | 1 - sel | sel | 1 | b | out |`, where out = sel * a + (1 - sel) * b.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] that contains a boolean value
    /// * `b`: [QuantumCell] that contains a boolean value
    /// * `sel`: [QuantumCell] that contains a boolean value
    fn select(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        sel: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let sel = sel.into();
        let diff_val = *a.value() - b.value();
        let out_val = diff_val * sel.value() + b.value();
        // | a - b | 1 | b | a |
        // | b | sel | a - b | out |
        let cells = [
            Witness(diff_val),
            Constant(F::ONE),
            b,
            a,
            b,
            sel,
            Witness(diff_val),
            Witness(out_val),
        ];
        ctx.assign_region_smart(cells, [0, 4], [(0, 6), (2, 4)], []);
        ctx.last().unwrap()
    }

    /// Constains and returns `a || (b && c)`, assuming `a`, `b` and `c` are boolean.
    ///
    /// Defines a vertical gate of form `| 1 - b c | b | c | 1 | a - 1 | 1 - b c | out | a - 1 | 1 | 1 | a |`, where out = a + b * c - a * b * c.
    /// * `ctx`: [Context] to add the constraints to
    /// * `a`: [QuantumCell] that contains a boolean value
    /// * `b`: [QuantumCell] that contains a boolean value
    /// * `c`: [QuantumCell] that contains a boolean value
    fn or_and(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        c: impl Into<QuantumCell<F>>,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();
        let c = c.into();
        let bc_val = *b.value() * c.value();
        let not_bc_val = F::ONE - bc_val;
        let not_a_val = *a.value() - F::ONE;
        let out_val = bc_val + a.value() - bc_val * a.value();
        let cells = [
            Witness(not_bc_val),
            b,
            c,
            Constant(F::ONE),
            Witness(not_a_val),
            Witness(not_bc_val),
            Witness(out_val),
            Witness(not_a_val),
            Constant(F::ONE),
            Constant(F::ONE),
            a,
        ];
        ctx.assign_region_smart(cells, [0, 3, 7], [(4, 7), (0, 5)], []);
        ctx.get(-5)
    }

    /// Constrains and returns little-endian bit vector representation of `a`.
    ///
    /// Assumes `range_bits >= number of bits in a`.
    /// * `a`: [QuantumCell] of the value to convert
    /// * `range_bits`: range of bits needed to represent `a`. Assumes `range_bits > 0`.
    fn num_to_bits(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        range_bits: usize,
    ) -> Vec<AssignedValue<F>> {
        let bits = a.value().to_u64_limbs(range_bits, 1).into_iter().map(|x| Witness(F::from(x)));

        let mut bit_cells = Vec::with_capacity(range_bits);
        let row_offset = ctx.advice.len();
        let acc = self.inner_product(
            ctx,
            bits,
            self.pow_of_two[..range_bits].iter().map(|c| Constant(*c)),
        );
        ctx.constrain_equal(&a, &acc);
        debug_assert!(range_bits > 0);
        bit_cells.push(ctx.get(row_offset as isize));
        for i in 1..range_bits {
            bit_cells.push(ctx.get((row_offset + 1 + 3 * (i - 1)) as isize));
        }

        for bit_cell in &bit_cells {
            self.assert_bit(ctx, *bit_cell);
        }
        bit_cells
    }

    /// Constrains and computes `a^exp` where both `a, exp` are witnesses. The exponent is computed in the native field `F`.
    ///
    /// Constrains that `exp` has at most `max_bits` bits.
    fn pow_var(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        exp: AssignedValue<F>,
        max_bits: usize,
    ) -> AssignedValue<F> {
        let exp_bits = self.num_to_bits(ctx, exp, max_bits);
        // standard square-and-mul approach
        let mut acc = ctx.load_constant(F::ONE);
        for (i, bit) in exp_bits.into_iter().rev().enumerate() {
            if i > 0 {
                // square
                acc = self.mul(ctx, acc, acc);
            }
            let mul = self.mul(ctx, acc, a);
            acc = self.select(ctx, mul, acc, bit);
        }
        acc
    }
}
