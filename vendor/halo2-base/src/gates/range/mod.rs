use crate::{
    gates::flex_gate::{FlexGateConfig, GateInstructions, MAX_PHASE},
    halo2_proofs::{
        circuit::{Layouter, Value},
        plonk::{
            Advice, Column, ConstraintSystem, Error, SecondPhase, Selector, TableColumn, ThirdPhase,
        },
        poly::Rotation,
    },
    utils::{
        biguint_to_fe, bit_length, decompose_fe_to_u64_limbs, fe_to_biguint, modulus,
        BigPrimeField, ScalarField,
    },
    virtual_region::lookups::LookupAnyManager,
    AssignedValue, Context,
    QuantumCell::{self, Constant, Existing, Witness},
};

use super::flex_gate::{FlexGateConfigParams, GateChip};

use getset::Getters;
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{One, Zero};
use std::{cmp::Ordering, ops::Shl};

fn assert_div_mod_no_wrap<F: BigPrimeField>(b: &BigUint, a_num_bits: usize) {
    assert!(!b.is_zero());
    assert!(a_num_bits <= F::CAPACITY as usize);

    let native_modulus = modulus::<F>();
    assert!(b < &native_modulus);

    let div_bound = BigUint::one().shl(a_num_bits as u32) / b + BigUint::one();
    let max_div = div_bound - BigUint::one();
    let max_lhs = b * max_div + (b - BigUint::one());
    assert!(max_lhs < native_modulus);
}

/// Configuration for Range Chip
#[derive(Clone, Debug)]
pub struct RangeConfig<F: ScalarField> {
    /// Underlying Gate Configuration
    pub gate: FlexGateConfig<F>,
    /// Special advice (witness) Columns used only for lookup tables.
    ///
    /// Each phase of a halo2 circuit has a distinct lookup_advice column.
    ///
    /// * If `gate` has only 1 advice column, lookups are enabled for that column, in which case `lookup_advice` is empty
    /// * If `gate` has more than 1 advice column some number of user-specified `lookup_advice` columns are added
    ///     * In this case, we don't need a selector so `q_lookup` is empty
    pub lookup_advice: Vec<Vec<Column<Advice>>>,
    /// Selector values for the lookup table.
    pub q_lookup: Vec<Option<Selector>>,
    /// Column for lookup table values.
    pub lookup: TableColumn,
    /// Defines the number of bits represented in the lookup table [0,2^<sup>lookup_bits</sup>).
    lookup_bits: usize,
}

impl<F: ScalarField> RangeConfig<F> {
    /// Generates a new [RangeConfig] with the specified parameters.
    ///
    /// If `num_columns` is 0, then we assume you do not want to perform any lookups in that phase.
    ///
    /// Panics if `lookup_bits` > 28.
    /// * `meta`: [ConstraintSystem] of the circuit
    /// * `gate_params`: see [FlexGateConfigParams]
    /// * `num_lookup_advice`: Number of `lookup_advice` [Column]s in each phase
    /// * `lookup_bits`: Number of bits represented in the LookUp table [0,2^lookup_bits)
    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        gate_params: FlexGateConfigParams,
        num_lookup_advice: &[usize],
        lookup_bits: usize,
    ) -> Self {
        assert!(lookup_bits <= F::S as usize);
        // sanity check: only create lookup table if there are lookup_advice columns
        assert!(!num_lookup_advice.is_empty(), "You are creating a RangeConfig but don't seem to need a lookup table, please double-check if you're using lookups correctly. Consider setting lookup_bits = None in BaseConfigParams");

        let lookup = meta.lookup_table_column();

        let gate = FlexGateConfig::configure(meta, gate_params.clone());

        // For now, we apply the same range lookup table to each phase
        let mut q_lookup = Vec::new();
        let mut lookup_advice = Vec::new();
        for (phase, &num_columns) in num_lookup_advice.iter().enumerate() {
            let num_advice = *gate_params.num_advice_per_phase.get(phase).unwrap_or(&0);
            let mut columns = Vec::new();
            // If num_columns is set to 0, then we assume you do not want to perform any lookups in that phase.
            // Disable this optimization in phase > 0 because you might set selectors based a cell from other columns.
            if phase == 0 && num_advice == 1 && num_columns != 0 {
                q_lookup.push(Some(meta.complex_selector()));
            } else {
                q_lookup.push(None);
                for _ in 0..num_columns {
                    let a = match phase {
                        0 => meta.advice_column(),
                        1 => meta.advice_column_in(SecondPhase),
                        2 => meta.advice_column_in(ThirdPhase),
                        _ => panic!("Currently RangeConfig only supports {MAX_PHASE} phases"),
                    };
                    meta.enable_equality(a);
                    columns.push(a);
                }
            }
            lookup_advice.push(columns);
        }

        let mut config = Self { lookup_advice, q_lookup, lookup, lookup_bits, gate };
        config.create_lookup(meta);

        log::debug!("Poisoned rows after RangeConfig::configure {}", meta.minimum_rows());
        config.gate.max_rows = (1 << gate_params.k) - meta.minimum_rows();
        assert!(
            (1 << lookup_bits) <= config.gate.max_rows,
            "lookup table is too large for the circuit degree plus blinding factors!"
        );

        config
    }

    /// Returns the number of bits represented in the lookup table [0,2^<sup>lookup_bits</sup>).
    pub fn lookup_bits(&self) -> usize {
        self.lookup_bits
    }

    /// Instantiates the lookup table of the circuit.
    /// * `meta`: [ConstraintSystem] of the circuit
    fn create_lookup(&self, meta: &mut ConstraintSystem<F>) {
        for (phase, q_l) in self.q_lookup.iter().enumerate() {
            if let Some(q) = q_l {
                meta.lookup("lookup", |meta| {
                    let q = meta.query_selector(*q);
                    // there should only be 1 advice column in phase `phase`
                    let a =
                        meta.query_advice(self.gate.basic_gates[phase][0].value, Rotation::cur());
                    vec![(q * a, self.lookup)]
                });
            }
        }
        //if multiple columns
        for la in self.lookup_advice.iter().flat_map(|advices| advices.iter()) {
            meta.lookup("lookup wo selector", |meta| {
                let a = meta.query_advice(*la, Rotation::cur());
                vec![(a, self.lookup)]
            });
        }
    }

    /// Loads the lookup table into the circuit using the provided `layouter`.
    /// * `layouter`: layouter for the circuit
    pub fn load_lookup_table(&self, layouter: &mut impl Layouter<F>) -> Result<(), Error> {
        layouter.assign_table(
            || format!("{} bit lookup", self.lookup_bits),
            |mut table| {
                for idx in 0..(1u32 << self.lookup_bits) {
                    table.assign_cell(
                        || "lookup table",
                        self.lookup,
                        idx as usize,
                        || Value::known(F::from(idx as u64)),
                    )?;
                }
                Ok(())
            },
        )?;
        Ok(())
    }
}

/// Trait that implements methods to constrain a field element number `x` is within a range of bits.
pub trait RangeInstructions<F: ScalarField> {
    /// The type of Gate used within the instructions.
    type Gate: GateInstructions<F>;

    /// Returns the type of gate used.
    fn gate(&self) -> &Self::Gate;

    /// Returns the number of bits the lookup table represents.
    fn lookup_bits(&self) -> usize;

    /// Checks and constrains that `a` lies in the range [0, 2<sup>range_bits</sup>).
    ///
    /// Inputs:
    /// * `a`: [AssignedValue] value to be range checked
    /// * `range_bits`: number of bits in the range
    fn range_check(&self, ctx: &mut Context<F>, a: AssignedValue<F>, range_bits: usize);

    /// Constrains that 'a' is less than 'b'.
    ///
    /// Assumes that `a` and `b` have bit length <= num_bits bits.
    ///
    /// Note: This may fail silently if a or b have more than num_bits.
    /// * a: [QuantumCell] value to check
    /// * b: upper bound expressed as a [QuantumCell]
    /// * num_bits: number of bits used to represent the values of `a` and `b`
    fn check_less_than(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        num_bits: usize,
    );

    /// Performs a range check that `a` has at most `ceil(b.bits() / lookup_bits) * lookup_bits` bits and then constrains that `a` is less than `b`.
    ///
    /// * a: [AssignedValue] value to check
    /// * b: upper bound expressed as a [u64] value
    ///
    /// ## Assumptions
    /// * `ceil(b.bits() / lookup_bits) * lookup_bits < F::CAPACITY`
    fn check_less_than_safe(&self, ctx: &mut Context<F>, a: AssignedValue<F>, b: u64) {
        let range_bits = bit_length(b).div_ceil(self.lookup_bits()) * self.lookup_bits();

        self.range_check(ctx, a, range_bits);
        self.check_less_than(ctx, a, Constant(F::from(b)), range_bits)
    }

    /// Performs a range check that `a` has at most `ceil(b.bits() / lookup_bits) * lookup_bits` bits and then constrains that `a` is less than `b`.
    ///
    /// * a: [AssignedValue] value to check
    /// * b: upper bound expressed as a [BigUint] value
    ///
    /// ## Assumptions
    /// * `ceil(b.bits() / lookup_bits) * lookup_bits < F::CAPACITY`
    fn check_big_less_than_safe(&self, ctx: &mut Context<F>, a: AssignedValue<F>, b: BigUint)
    where
        F: BigPrimeField,
    {
        let range_bits = (b.bits() as usize).div_ceil(self.lookup_bits()) * self.lookup_bits();

        self.range_check(ctx, a, range_bits);
        self.check_less_than(ctx, a, Constant(biguint_to_fe(&b)), range_bits)
    }

    /// Constrains whether `a` is in `[0, b)`, and returns 1 if `a` < `b`, otherwise 0.
    ///
    /// Assumes that`a` and `b` are known to have <= num_bits bits.
    /// * a: first [QuantumCell] to compare
    /// * b: second [QuantumCell] to compare
    /// * num_bits: number of bits to represent the values
    fn is_less_than(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        num_bits: usize,
    ) -> AssignedValue<F>;

    /// Performs a range check that `a` has at most `ceil(bit_length(b) / lookup_bits) * lookup_bits` and then returns whether `a` is in `[0,b)`.
    ///
    /// Returns 1 if `a` < `b`, otherwise 0.
    ///
    /// * a: [AssignedValue] value to check
    /// * b: upper bound as [u64] value
    fn is_less_than_safe(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        b: u64,
    ) -> AssignedValue<F> {
        let range_bits = bit_length(b).div_ceil(self.lookup_bits()) * self.lookup_bits();

        self.range_check(ctx, a, range_bits);
        self.is_less_than(ctx, a, Constant(F::from(b)), range_bits)
    }

    /// Performs a range check that `a` has at most `ceil(b.bits() / lookup_bits) * lookup_bits` bits and then returns whether `a` is in `[0,b)`.
    ///
    /// Returns 1 if `a` < `b`, otherwise 0.
    ///
    /// * a: [AssignedValue] value to check
    /// * b: upper bound as [BigUint] value
    ///
    /// For the current implementation using `is_less_than`, we require `(ceil(b.bits() / lookup_bits) + 1) * lookup_bits <= F::CAPACITY`
    fn is_big_less_than_safe(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        b: BigUint,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        let range_bits = (b.bits() as usize).div_ceil(self.lookup_bits()) * self.lookup_bits();

        self.range_check(ctx, a, range_bits);
        self.is_less_than(ctx, a, Constant(biguint_to_fe(&b)), range_bits)
    }

    /// Constrains and returns `(c, r)` such that `a = b * c + r`.
    ///
    /// * a: [QuantumCell] value to divide
    /// * b: [BigUint] value to divide by
    /// * a_num_bits: number of bits needed to represent the value of `a`
    ///
    /// ## Assumptions
    /// * `b != 0` and that `a` has <= `a_num_bits` bits.
    /// * `a_num_bits <= F::CAPACITY = F::NUM_BITS - 1`
    /// * the bounds on `b`, quotient, and remainder imply `b * quotient + remainder < F::MODULUS`
    fn div_mod(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<BigUint>,
        a_num_bits: usize,
    ) -> (AssignedValue<F>, AssignedValue<F>)
    where
        F: BigPrimeField,
    {
        let a = a.into();
        let b = b.into();
        assert_div_mod_no_wrap::<F>(&b, a_num_bits);
        let a_val = fe_to_biguint(a.value());
        let (div, rem) = a_val.div_mod_floor(&b);
        let [div, rem] = [div, rem].map(|v| biguint_to_fe(&v));
        ctx.assign_region([Witness(rem), Constant(biguint_to_fe(&b)), Witness(div), a], [0]);
        let rem = ctx.get(-4);
        let div = ctx.get(-2);
        // Constrain that a_num_bits fulfills `div < 2 ** a_num_bits / b`.
        self.check_big_less_than_safe(
            ctx,
            div,
            BigUint::one().shl(a_num_bits as u32) / &b + BigUint::one(),
        );
        // Constrain that remainder is less than divisor (i.e. `r < b`).
        self.check_big_less_than_safe(ctx, rem, b);
        (div, rem)
    }

    /// Constrains and returns `(c, r)` such that `a = b * c + r`.
    ///
    /// Assumes:
    /// that `b != 0`.
    /// that `a` has <= `a_num_bits` bits.
    /// that `b` has <= `b_num_bits` bits.
    ///
    /// Note:
    /// Let `X = 2 ** b_num_bits`
    /// Write `a = a1 * X + a0` and `c = c1 * X + c0`
    /// If we write `b * c0 + r = d1 * X + d0` then
    ///     `b * c + r = (b * c1 + d1) * X + d0`
    /// * a: [QuantumCell] value to divide
    /// * b: [QuantumCell] value to divide by
    /// * a_num_bits: number of bits needed to represent the value of `a`
    /// * b_num_bits: number of bits needed to represent the value of `b`
    ///
    /// ## Assumptions
    /// * `b != 0`
    /// * `a_num_bits <= F::CAPACITY = F::NUM_BITS - 1`
    /// * `b_num_bits <= F::CAPACITY = F::NUM_BITS - 1`
    /// * the native products used by the split division do not wrap in `F`
    fn div_mod_var(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        a_num_bits: usize,
        b_num_bits: usize,
    ) -> (AssignedValue<F>, AssignedValue<F>)
    where
        F: BigPrimeField,
    {
        assert!(a_num_bits <= F::CAPACITY as usize);
        assert!(b_num_bits > 0 && b_num_bits <= F::CAPACITY as usize);

        let native_modulus = modulus::<F>();
        let x = BigUint::one().shl(b_num_bits as u32);
        let max_bcr0 = &x * (&x - BigUint::one());
        assert!(max_bcr0 < native_modulus.clone());
        if a_num_bits > b_num_bits {
            let max_b = &x - BigUint::one();
            let max_div_hi = BigUint::one().shl((a_num_bits - b_num_bits) as u32) - BigUint::one();
            let max_bcr0_hi = BigUint::one().shl((a_num_bits - b_num_bits) as u32);
            let max_bcr_hi = max_b * max_div_hi + max_bcr0_hi;
            assert!(max_bcr_hi < native_modulus);
        }

        let a = a.into();
        let b = b.into();
        ctx.assign_cell(b);
        let b = ctx.get(-1);
        self.range_check(ctx, b, b_num_bits);
        let a_val = fe_to_biguint(a.value());
        let b_val = fe_to_biguint(b.value());
        assert!(!b_val.is_zero());
        let (div, rem) = a_val.div_mod_floor(&b_val);
        let (div_hi, div_lo) = div.div_mod_floor(&x);

        let x_fe = self.gate().pow_of_two()[b_num_bits];
        let [div, div_hi, div_lo, rem] = [div, div_hi, div_lo, rem].map(|v| biguint_to_fe(&v));
        ctx.assign_region(
            [Witness(div_lo), Witness(div_hi), Constant(x_fe), Witness(div), Witness(rem)],
            [0],
        );
        let [div_lo, div_hi, div, rem] = [-5, -4, -2, -1].map(|i| ctx.get(i));
        self.range_check(ctx, div_lo, b_num_bits);
        if a_num_bits <= b_num_bits {
            self.gate().assert_is_const(ctx, &div_hi, &F::ZERO);
        } else {
            self.range_check(ctx, div_hi, a_num_bits - b_num_bits);
        }

        let (bcr0_hi, bcr0_lo) = {
            let bcr0 = self.gate().mul_add(ctx, Existing(b), Existing(div_lo), Existing(rem));
            self.div_mod(ctx, Existing(bcr0), x.clone(), a_num_bits)
        };
        let bcr_hi = self.gate().mul_add(ctx, Existing(b), Existing(div_hi), Existing(bcr0_hi));

        let (a_hi, a_lo) = self.div_mod(ctx, a, x, a_num_bits);
        ctx.constrain_equal(&bcr_hi, &a_hi);
        ctx.constrain_equal(&bcr0_lo, &a_lo);

        self.range_check(ctx, rem, b_num_bits);
        self.check_less_than(ctx, Existing(rem), Existing(b), b_num_bits);
        (div, rem)
    }

    /// Constrains and returns the last bit of the value of `a`.
    ///
    /// Assume `a` has been range checked already to `limb_bits` bits.
    /// * a: [AssignedValue] value to get the last bit of
    /// * limb_bits: number of bits in a limb
    fn get_last_bit(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        limb_bits: usize,
    ) -> AssignedValue<F> {
        let a_big = fe_to_biguint(a.value());
        let bit_v = F::from(a_big.bit(0));
        let two = F::from(2u64);
        let h_v = F::from_bytes_le(&(a_big >> 1usize).to_bytes_le());

        ctx.assign_region([Witness(bit_v), Witness(h_v), Constant(two), Existing(a)], [0]);
        let half = ctx.get(-3);
        let bit = ctx.get(-4);

        self.range_check(ctx, half, limb_bits - 1);
        self.gate().assert_bit(ctx, bit);
        bit
    }
}

/// # RangeChip
/// This chip provides methods that rely on "range checking" that a field element `x` is within a range of bits.
/// Range checks are done using a lookup table with the numbers [0, 2<sup>lookup_bits</sup>).
#[derive(Clone, Debug, Getters)]
pub struct RangeChip<F: ScalarField> {
    /// Underlying [GateChip] for this chip.
    pub gate: GateChip<F>,
    /// Lookup manager for each phase, lazily initiated using the [`SharedCopyConstraintManager`](crate::virtual_region::copy_constraints::SharedCopyConstraintManager) from the [Context]
    /// that first calls it.
    ///
    /// The lookup manager is used to store the cells that need to be looked up in the range check lookup table.
    #[getset(get = "pub")]
    lookup_manager: [LookupAnyManager<F, 1>; MAX_PHASE],
    /// Defines the number of bits represented in the lookup table [0,2<sup>lookup_bits</sup>).
    lookup_bits: usize,
    /// [Vec] of powers of `2 ** lookup_bits` represented as [QuantumCell::Constant].
    /// These are precomputed and cached as a performance optimization for later limb decompositions. We precompute up to the higher power that fits in `F`, which is `2 ** ((F::CAPACITY / lookup_bits) * lookup_bits)`.
    pub limb_bases: Vec<QuantumCell<F>>,
}

impl<F: ScalarField> RangeChip<F> {
    /// Creates a new [RangeChip] with the given strategy and lookup_bits.
    /// * `lookup_bits`: number of bits represented in the lookup table [0,2<sup>lookup_bits</sup>)
    /// * `lookup_manager`: a [LookupAnyManager] for each phase.
    ///
    /// **IMPORTANT:** It is **critical** that all `LookupAnyManager`s use the same [`SharedCopyConstraintManager`](crate::virtual_region::copy_constraints::SharedCopyConstraintManager)
    /// as in your primary circuit builder.
    ///
    /// It is not advised to call this function directly. Instead you should call `BaseCircuitBuilder::range_chip`.
    pub fn new(lookup_bits: usize, lookup_manager: [LookupAnyManager<F, 1>; MAX_PHASE]) -> Self {
        let limb_base = F::from(1u64 << lookup_bits);
        let mut running_base = limb_base;
        let num_bases = F::CAPACITY as usize / lookup_bits;
        let mut limb_bases = Vec::with_capacity(num_bases + 1);
        limb_bases.extend([Constant(F::ONE), Constant(running_base)]);
        for _ in 2..=num_bases {
            running_base *= &limb_base;
            limb_bases.push(Constant(running_base));
        }
        let gate = GateChip::new();

        Self { gate, lookup_bits, lookup_manager, limb_bases }
    }

    fn add_cell_to_lookup(&self, ctx: &Context<F>, a: AssignedValue<F>) {
        let phase = ctx.phase();
        let manager = &self.lookup_manager[phase];
        manager.add_lookup(ctx.tag(), [a]);
    }

    /// Checks and constrains that `a` lies in the range [0, 2<sup>range_bits</sup>).
    ///
    /// This is done by decomposing `a` into `num_limbs` limbs, where `num_limbs = ceil(range_bits / lookup_bits)`.
    /// Each limb is constrained to be within the range [0, 2<sup>lookup_bits</sup>).
    /// The limbs are then combined to form `a` again with the last limb having `rem_bits` number of bits.
    ///
    /// Returns the last (highest) limb.
    ///
    /// Inputs:
    /// * `a`: [AssignedValue] value to be range checked
    /// * `range_bits`: number of bits in the range
    /// * `lookup_bits`: number of bits in the lookup table
    ///
    /// # Assumptions
    /// * `ceil(range_bits / lookup_bits) * lookup_bits <= F::CAPACITY`
    fn _range_check(
        &self,
        ctx: &mut Context<F>,
        a: AssignedValue<F>,
        range_bits: usize,
    ) -> AssignedValue<F> {
        if range_bits == 0 {
            self.gate.assert_is_const(ctx, &a, &F::ZERO);
            return a;
        }
        // the number of limbs
        let num_limbs = range_bits.div_ceil(self.lookup_bits);
        // println!("range check {} bits {} len", range_bits, k);
        let rem_bits = range_bits % self.lookup_bits;

        debug_assert!(self.limb_bases.len() >= num_limbs);

        let last_limb = if num_limbs == 1 {
            self.add_cell_to_lookup(ctx, a);
            a
        } else {
            let limbs = decompose_fe_to_u64_limbs(a.value(), num_limbs, self.lookup_bits)
                .into_iter()
                .map(|x| Witness(F::from(x)));
            let row_offset = ctx.advice.len() as isize;
            let acc = self.gate.inner_product(ctx, limbs, self.limb_bases[..num_limbs].to_vec());
            // the inner product above must equal `a`
            ctx.constrain_equal(&a, &acc);
            // we fetch the cells to lookup by getting the indices where `limbs` were assigned in `inner_product`. Because `limb_bases[0]` is 1, the progression of indices is 0,1,4,...,4+3*i
            self.add_cell_to_lookup(ctx, ctx.get(row_offset));
            for i in 0..num_limbs - 1 {
                self.add_cell_to_lookup(ctx, ctx.get(row_offset + 1 + 3 * i as isize));
            }
            ctx.get(row_offset + 1 + 3 * (num_limbs - 2) as isize)
        };

        // additional constraints for the last limb if rem_bits != 0
        match rem_bits.cmp(&1) {
            // we want to check x := limbs[num_limbs-1] is boolean
            // we constrain x*(x-1) = 0 + x * x - x == 0
            // | 0 | x | x | x |
            Ordering::Equal => {
                self.gate.assert_bit(ctx, last_limb);
            }
            Ordering::Greater => {
                let mult_val = self.gate.pow_of_two[self.lookup_bits - rem_bits];
                let check = self.gate.mul(ctx, last_limb, Constant(mult_val));
                self.add_cell_to_lookup(ctx, check);
            }
            _ => {}
        }
        last_limb
    }
}

impl<F: ScalarField> RangeInstructions<F> for RangeChip<F> {
    type Gate = GateChip<F>;

    /// The type of Gate used in this chip.
    fn gate(&self) -> &Self::Gate {
        &self.gate
    }

    /// Returns the number of bits represented in the lookup table [0,2<sup>lookup_bits</sup>).
    fn lookup_bits(&self) -> usize {
        self.lookup_bits
    }

    /// Checks and constrains that `a` lies in the range [0, 2<sup>range_bits</sup>).
    ///
    /// This is done by decomposing `a` into `num_limbs` limbs, where `num_limbs = ceil(range_bits / lookup_bits)`.
    /// Each limb is constrained to be within the range [0, 2<sup>lookup_bits</sup>).
    /// The limbs are then combined to form `a` again with the last limb having `rem_bits` number of bits.
    ///
    /// Inputs:
    /// * `a`: [AssignedValue] value to be range checked
    /// * `range_bits`: number of bits in the range
    ///
    /// # Assumptions
    /// * `ceil(range_bits / lookup_bits) * lookup_bits <= F::CAPACITY`
    fn range_check(&self, ctx: &mut Context<F>, a: AssignedValue<F>, range_bits: usize) {
        self._range_check(ctx, a, range_bits);
    }

    /// Constrains that 'a' is less than 'b'.
    ///
    /// Assumes that`a` and `b` are known to have <= num_bits bits.
    ///
    /// Note: This may fail silently if a or b have more than num_bits
    /// * a: [QuantumCell] value to check
    /// * b: upper bound expressed as a [QuantumCell]
    /// * num_bits: number of bits to represent the values
    fn check_less_than(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        num_bits: usize,
    ) {
        assert!(
            num_bits < F::CAPACITY as usize,
            "num_bits must leave one native-field bit of headroom for check_less_than"
        );
        let a = a.into();
        let b = b.into();
        let pow_of_two = self.gate.pow_of_two[num_bits];
        let check_cell = {
            let shift_a_val = pow_of_two + a.value();
            // | a + 2^(num_bits) - b | b | 1 | a + 2^(num_bits) | - 2^(num_bits) | 1 | a |
            let cells = [
                Witness(shift_a_val - b.value()),
                b,
                Constant(F::ONE),
                Witness(shift_a_val),
                Constant(-pow_of_two),
                Constant(F::ONE),
                a,
            ];
            ctx.assign_region(cells, [0, 3]);
            ctx.get(-7)
        };

        self.range_check(ctx, check_cell, num_bits);
    }

    /// Constrains whether `a` is in `[0, b)`, and returns 1 if `a` < `b`, otherwise 0.
    ///
    /// * a: first [QuantumCell] to compare
    /// * b: second [QuantumCell] to compare
    /// * num_bits: number of bits to represent the values
    ///
    /// # Assumptions
    /// * `a` and `b` are known to have `<= num_bits` bits.
    /// * (`ceil(num_bits / lookup_bits) + 1) * lookup_bits <= F::CAPACITY`
    fn is_less_than(
        &self,
        ctx: &mut Context<F>,
        a: impl Into<QuantumCell<F>>,
        b: impl Into<QuantumCell<F>>,
        num_bits: usize,
    ) -> AssignedValue<F> {
        let a = a.into();
        let b = b.into();

        let k = num_bits.div_ceil(self.lookup_bits);
        let padded_bits = k * self.lookup_bits;
        assert!(
            padded_bits + self.lookup_bits <= F::CAPACITY as usize,
            "num_bits is too large for this is_less_than implementation"
        );
        let pow_padded = self.gate.pow_of_two[padded_bits];

        let shift_a_val = pow_padded + a.value();
        let shifted_val = shift_a_val - b.value();
        let shifted_cell = {
            ctx.assign_region(
                [
                    Witness(shifted_val),
                    b,
                    Constant(F::ONE),
                    Witness(shift_a_val),
                    Constant(-pow_padded),
                    Constant(F::ONE),
                    a,
                ],
                [0, 3],
            );
            ctx.get(-7)
        };

        // check whether a - b + 2^padded_bits < 2^padded_bits ?
        // since assuming a, b < 2^padded_bits we are guaranteed a - b + 2^padded_bits < 2^{padded_bits + 1}
        let last_limb = self._range_check(ctx, shifted_cell, padded_bits + self.lookup_bits);
        // last_limb will have the (k + 1)-th limb of `a - b + 2^{k * limb_bits}`, which is zero iff `a < b`
        self.gate.is_zero(ctx, last_limb)
    }
}
