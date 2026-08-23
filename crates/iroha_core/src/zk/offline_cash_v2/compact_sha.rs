//! Conditional compact SHA-256 circuit for the undeclared Offline Cash V2 tranche.
//!
//! This source is intentionally not declared by `offline_cash_v2.rs`.  It contains a
//! current-row-only Halo2 machine and assignment path, but has no compiler or artifact evidence.
//! The governed nine-job batch is deliberately unavailable: the bit-level implementation counts
//! 932,944 assigned rows, while the `k = 17` profile exposes only 131,063.  Circuit synthesis and
//! fixed-trace construction reject that mismatch before trace assignment; the infallible typed
//! constructor merely retains an already validated ABI.  No V1 role, proof, key, transcript, or
//! wire contract is changed by this candidate.

use std::{collections::HashMap, marker::PhantomData};

use halo2_base::{
    halo2_proofs::{
        circuit::{Cell, Layouter, SimpleFloorPlanner, Value},
        plonk::{
            Advice, Assigned, Circuit, Column, ConstraintSystem, Error, Expression, Fixed, Instance,
        },
        poly::Rotation,
    },
    utils::{
        BigPrimeField,
        halo2::{raw_assign_advice, raw_assign_fixed, raw_constrain_equal},
    },
};

use super::compact_sha_abi::{
    COMPACT_SHA_BATCH_INSTANCE_CELLS_V2, COMPACT_SHA_FIXED_BLOCKS_V2,
    COMPACT_SHA_FIXED_MESSAGE_BLOCKS_V2, COMPACT_SHA_FIXED_MESSAGE_BYTES_V2,
    COMPACT_SHA_HELPER_WORDS_V2, COMPACT_SHA_KEY_WORDS_V2, COMPACT_SHA_USABLE_ROWS_V2,
    CompactShaAbiErrorV2, CompactShaBatchPublicAbiV2,
};
use crate::zk::kagemusha_sha256_table16_v4::{IV, ROUND_CONSTANTS};

const ADVICE_COLUMNS: usize = 8;
const INSTANCE_COLUMNS: usize = 1;
const FIXED_COLUMNS: usize = 4;
const LOOKUP_ARGUMENTS: usize = 2;
const MAX_DEGREE: usize = 7;
const PERMUTATION_CHUNKS: usize = 2;
const CURRENT_ROTATIONS: usize = 1;

/// Exact typed dense/spread inventory retained for the V2 candidate.
///
/// The current bit-level machine consumes only width one; widths 2..=16 are collision-audited
/// inventory and do not contribute to the SHA relation or the 932,944-row no-go.
pub(super) const COMPACT_SHA_SPREAD_WIDTHS_V2: [usize; 12] =
    [1, 2, 3, 4, 6, 7, 8, 10, 11, 13, 14, 16];
pub(super) const COMPACT_SHA_TYPED_SPREAD_ROWS_V2: usize = 93_662;
const CARRY_ROWS: usize = 5;
const LOGIC_MODES: usize = 6;
const LOGIC_ROWS: usize = LOGIC_MODES * 8;
pub(super) const COMPACT_SHA_TABLE_ROWS_V2: usize =
    1 + COMPACT_SHA_TYPED_SPREAD_ROWS_V2 + CARRY_ROWS + LOGIC_ROWS;

const TYPE_KEY_SHIFT: u64 = 16;
const CARRY_KEY_BASE: u64 = 1 << 22;
const LOGIC_KEY_BASE: u64 = 1 << 23;

pub(super) const COMPACT_SHA_SCHEDULE_ROWS_PER_BLOCK_V2: usize = 48 * (16 + 16 + 32);
pub(super) const COMPACT_SHA_COMPRESSION_ROWS_PER_BLOCK_V2: usize = 64 * (4 * 16 + 4 * 32);
pub(super) const COMPACT_SHA_FEED_FORWARD_ROWS_PER_BLOCK_V2: usize = 8 * 32;
pub(super) const COMPACT_SHA_ROWS_PER_BLOCK_V2: usize = COMPACT_SHA_SCHEDULE_ROWS_PER_BLOCK_V2
    + COMPACT_SHA_COMPRESSION_ROWS_PER_BLOCK_V2
    + COMPACT_SHA_FEED_FORWARD_ROWS_PER_BLOCK_V2;

// Thirty-two direct instance bindings, seven Horner limbs for each instance cell, fifty-two
// rows for each real ABI word (four byte Horner rows, thirty-two bit Horner rows, and sixteen
// two-lane Boolean lookup rows), plus two seed and six radix-construction rows.
pub(super) const COMPACT_SHA_ABI_BINDING_ROWS_V2: usize = COMPACT_SHA_BATCH_INSTANCE_CELLS_V2
    + COMPACT_SHA_BATCH_INSTANCE_CELLS_V2 * 7
    + (COMPACT_SHA_HELPER_WORDS_V2 + 2 * COMPACT_SHA_KEY_WORDS_V2) * 52
    + 2
    + 6;
pub(super) const COMPACT_SHA_BATCH_SHA_ROWS_V2: usize =
    COMPACT_SHA_FIXED_BLOCKS_V2 * COMPACT_SHA_ROWS_PER_BLOCK_V2;
pub(super) const COMPACT_SHA_BATCH_REQUIRED_ROWS_V2: usize =
    COMPACT_SHA_ABI_BINDING_ROWS_V2 + COMPACT_SHA_BATCH_SHA_ROWS_V2;
pub(super) const COMPACT_SHA_BATCH_ROW_EXCESS_V2: usize =
    COMPACT_SHA_BATCH_REQUIRED_ROWS_V2 - COMPACT_SHA_USABLE_ROWS_V2;

/// Rejected paper projection retained so reviewers cannot mistake it for source evidence.
///
/// Its 2,127 rows per block assumed that the reviewed Table16 schedule/compression layout could
/// be flattened into eight current-row advice columns while retaining all dense/spread halves,
/// carry witnesses, and state aliases at zero copy cost.  It also assumed that rotation removal,
/// lookup-tuple typing, and Boolean reconstruction could share existing Table16 rows.  No gate or
/// assignment was found that realizes those assumptions under two lookup arguments.  The 4,690
/// non-block rows were therefore irrelevant: the projected 130,183 total was never a sound source
/// bound and is not used by any eligibility decision.
pub(super) const COMPACT_SHA_REJECTED_PACKED_ROWS_PER_BLOCK_V2: usize = 2_127;
pub(super) const COMPACT_SHA_REJECTED_PACKED_BLOCK_ROWS_V2: usize =
    COMPACT_SHA_FIXED_BLOCKS_V2 * COMPACT_SHA_REJECTED_PACKED_ROWS_PER_BLOCK_V2;
pub(super) const COMPACT_SHA_REJECTED_NON_BLOCK_ROWS_V2: usize = 4_690;
pub(super) const COMPACT_SHA_REJECTED_SOURCE_BOUND_V2: usize =
    COMPACT_SHA_REJECTED_PACKED_BLOCK_ROWS_V2 + COMPACT_SHA_REJECTED_NON_BLOCK_ROWS_V2;

const _: () = assert!(COMPACT_SHA_TYPED_SPREAD_ROWS_V2 == 93_662);
const _: () = assert!(COMPACT_SHA_TABLE_ROWS_V2 == 93_716);
const _: () = assert!(COMPACT_SHA_SCHEDULE_ROWS_PER_BLOCK_V2 == 3_072);
const _: () = assert!(COMPACT_SHA_COMPRESSION_ROWS_PER_BLOCK_V2 == 12_288);
const _: () = assert!(COMPACT_SHA_FEED_FORWARD_ROWS_PER_BLOCK_V2 == 256);
const _: () = assert!(COMPACT_SHA_ROWS_PER_BLOCK_V2 == 15_616);
const _: () = assert!(COMPACT_SHA_ABI_BINDING_ROWS_V2 == 11_600);
const _: () = assert!(COMPACT_SHA_BATCH_SHA_ROWS_V2 == 921_344);
const _: () = assert!(COMPACT_SHA_BATCH_REQUIRED_ROWS_V2 == 932_944);
const _: () = assert!(COMPACT_SHA_BATCH_ROW_EXCESS_V2 == 801_881);
const _: () = assert!(COMPACT_SHA_REJECTED_PACKED_BLOCK_ROWS_V2 == 125_493);
const _: () = assert!(COMPACT_SHA_REJECTED_SOURCE_BOUND_V2 == 130_183);
const _: () = assert!(COMPACT_SHA_TABLE_ROWS_V2 <= COMPACT_SHA_USABLE_ROWS_V2);
const _: () = assert!(COMPACT_SHA_BATCH_REQUIRED_ROWS_V2 > COMPACT_SHA_USABLE_ROWS_V2);

/// The reviewed augmented Pasta IPA shape.  It is a configured source target, not compiler,
/// proof, key, or artifact evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CompactShaProofShapeV2 {
    pub(super) k: u32,
    pub(super) advice_columns: usize,
    pub(super) advice_queries: usize,
    pub(super) instance_columns: usize,
    pub(super) instance_queries: usize,
    pub(super) fixed_columns: usize,
    pub(super) fixed_queries: usize,
    pub(super) selector_columns: usize,
    pub(super) lookup_arguments: usize,
    pub(super) maximum_degree: usize,
    pub(super) rotations: usize,
    pub(super) equality_columns: usize,
    pub(super) permutation_chunks: usize,
    pub(super) point_sets: usize,
    pub(super) point_elements: usize,
    pub(super) scalar_elements: usize,
    pub(super) raw_bytes: usize,
    pub(super) augmented_bytes: usize,
}

pub(super) const COMPACT_SHA_PROOF_SHAPE_V2: CompactShaProofShapeV2 = CompactShaProofShapeV2 {
    k: 17,
    advice_columns: ADVICE_COLUMNS,
    advice_queries: ADVICE_COLUMNS,
    instance_columns: INSTANCE_COLUMNS,
    instance_queries: INSTANCE_COLUMNS,
    fixed_columns: FIXED_COLUMNS,
    fixed_queries: FIXED_COLUMNS,
    selector_columns: 0,
    lookup_arguments: LOOKUP_ARGUMENTS,
    maximum_degree: MAX_DEGREE,
    rotations: CURRENT_ROTATIONS,
    equality_columns: ADVICE_COLUMNS,
    permutation_chunks: PERMUTATION_CHUNKS,
    point_sets: 4,
    point_elements: 59,
    scalar_elements: 42,
    raw_bytes: 3_232,
    augmented_bytes: 3_264,
};

const _: () = assert!(COMPACT_SHA_PROOF_SHAPE_V2.point_elements * 32 == 1_888);
const _: () = assert!(COMPACT_SHA_PROOF_SHAPE_V2.scalar_elements * 32 == 1_344);
const _: () = assert!(COMPACT_SHA_PROOF_SHAPE_V2.raw_bytes == 1_888 + 1_344);
const _: () = assert!(COMPACT_SHA_PROOF_SHAPE_V2.augmented_bytes == 3_232 + 32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CompactShaRowsV2 {
    pub(super) public_bindings: usize,
    pub(super) packed_instance_horner: usize,
    pub(super) word_and_bit_decomposition: usize,
    pub(super) seeds_and_radices: usize,
    pub(super) schedule: usize,
    pub(super) compression: usize,
    pub(super) feed_forward: usize,
    pub(super) lookup_table: usize,
    pub(super) assigned: usize,
    pub(super) available: usize,
}

impl CompactShaRowsV2 {
    pub(super) const fn fixed_batch() -> Self {
        Self {
            public_bindings: COMPACT_SHA_BATCH_INSTANCE_CELLS_V2,
            packed_instance_horner: COMPACT_SHA_BATCH_INSTANCE_CELLS_V2 * 7,
            word_and_bit_decomposition: (COMPACT_SHA_HELPER_WORDS_V2
                + 2 * COMPACT_SHA_KEY_WORDS_V2)
                * 52,
            seeds_and_radices: 8,
            schedule: COMPACT_SHA_FIXED_BLOCKS_V2 * COMPACT_SHA_SCHEDULE_ROWS_PER_BLOCK_V2,
            compression: COMPACT_SHA_FIXED_BLOCKS_V2 * COMPACT_SHA_COMPRESSION_ROWS_PER_BLOCK_V2,
            feed_forward: COMPACT_SHA_FIXED_BLOCKS_V2 * COMPACT_SHA_FEED_FORWARD_ROWS_PER_BLOCK_V2,
            lookup_table: COMPACT_SHA_TABLE_ROWS_V2,
            assigned: COMPACT_SHA_BATCH_REQUIRED_ROWS_V2,
            available: COMPACT_SHA_USABLE_ROWS_V2,
        }
    }

    pub(super) const fn fits(self) -> bool {
        self.assigned <= self.available && self.lookup_table <= self.available
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CompactShaFailureV2 {
    Abi(CompactShaAbiErrorV2),
    RowLimit { required: usize, available: usize },
    CounterSpill { attempted: usize, available: usize },
    MessageGeometry,
}

impl From<CompactShaAbiErrorV2> for CompactShaFailureV2 {
    fn from(error: CompactShaAbiErrorV2) -> Self {
        Self::Abi(error)
    }
}

#[derive(Clone, Debug)]
pub(super) struct CompactShaCircuitV2<F: BigPrimeField> {
    abi: CompactShaBatchPublicAbiV2,
    _marker: PhantomData<F>,
}

impl<F: BigPrimeField> CompactShaCircuitV2<F> {
    pub(super) const fn new(abi: CompactShaBatchPublicAbiV2) -> Self {
        Self {
            abi,
            _marker: PhantomData,
        }
    }

    pub(super) const fn rows() -> CompactShaRowsV2 {
        CompactShaRowsV2::fixed_batch()
    }

    pub(super) fn public_instances(&self) -> Vec<F> {
        self.abi.field_instances::<F>().to_vec()
    }

    pub(super) fn preflight(&self) -> Result<(), CompactShaFailureV2> {
        let rows = Self::rows();
        if rows.fits() {
            Ok(())
        } else {
            Err(CompactShaFailureV2::RowLimit {
                required: rows.assigned,
                available: rows.available,
            })
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct CompactShaConfigV2 {
    advice: [Column<Advice>; ADVICE_COLUMNS],
    instance: Column<Instance>,
    opcode: Column<Fixed>,
    control: Column<Fixed>,
    table_key: Column<Fixed>,
    table_value: Column<Fixed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
enum Opcode {
    Disabled = 0,
    Public = 1,
    Fma = 2,
    Logic = 3,
    Add = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
enum LogicMode {
    Xor3 = 0,
    Choose = 1,
    Majority = 2,
    Range = 3,
    ForceZero = 4,
    ForceOne = 5,
}

fn mode_lagrange<F: BigPrimeField>(opcode: Expression<F>, target: u64) -> Expression<F> {
    let mut numerator = Expression::Constant(F::ONE);
    let mut denominator = F::ONE;
    for root in 0_u64..=Opcode::Add as u64 {
        if root != target {
            numerator = numerator * (opcode.clone() - Expression::Constant(F::from(root)));
            denominator *= F::from(target) - F::from(root);
        }
    }
    let inverse = Option::<F>::from(denominator.invert())
        .expect("distinct compact-SHA opcode roots are invertible");
    numerator * Expression::Constant(inverse)
}

impl CompactShaConfigV2 {
    pub(super) fn configure<F: BigPrimeField>(meta: &mut ConstraintSystem<F>) -> Self {
        let advice = std::array::from_fn(|_| {
            let column = meta.advice_column();
            meta.enable_equality(column);
            column
        });
        let instance = meta.instance_column();
        let opcode = meta.fixed_column();
        let control = meta.fixed_column();
        let table_key = meta.fixed_column();
        let table_value = meta.fixed_column();

        meta.create_gate("offline cash V2 compact SHA current-row machine", |meta| {
            let value = advice.map(|column| meta.query_advice(column, Rotation::cur()));
            let public = meta.query_instance(instance, Rotation::cur());
            let op = meta.query_fixed(opcode, Rotation::cur());
            let q_public = mode_lagrange(op.clone(), Opcode::Public as u64);
            let q_fma = mode_lagrange(op.clone(), Opcode::Fma as u64);
            let q_logic = mode_lagrange(op.clone(), Opcode::Logic as u64);
            let q_add = mode_lagrange(op, Opcode::Add as u64);
            let one = Expression::Constant(F::ONE);
            let two = Expression::Constant(F::from(2));
            let add_sum = value[..5]
                .iter()
                .cloned()
                .fold(Expression::Constant(F::ZERO), |sum, item| sum + item)
                + value[5].clone()
                - value[6].clone()
                - two * value[7].clone();
            let mut constraints = vec![
                q_public * (value[7].clone() - public),
                q_fma.clone()
                    * (value[0].clone() + value[1].clone() * value[2].clone() - value[3].clone()),
                q_fma * (value[4].clone() + value[5].clone() * value[6].clone() - value[7].clone()),
                q_add.clone() * add_sum,
            ];
            for index in [0, 1, 2, 3, 4, 6] {
                constraints.push(
                    q_add.clone() * value[index].clone() * (value[index].clone() - one.clone()),
                );
            }
            for item in value {
                constraints
                    .push(q_logic.clone() * item.clone() * (item - Expression::Constant(F::ONE)));
            }
            constraints
        });

        for (label, lane) in [
            ("offline cash V2 compact SHA lookup lane zero", 0_usize),
            ("offline cash V2 compact SHA lookup lane one", 1_usize),
        ] {
            meta.lookup_any(label, |meta| {
                let value = advice.map(|column| meta.query_advice(column, Rotation::cur()));
                let op = meta.query_fixed(opcode, Rotation::cur());
                let logic_control = meta.query_fixed(control, Rotation::cur());
                let q_logic = mode_lagrange(op.clone(), Opcode::Logic as u64);
                let q_add = mode_lagrange(op, Opcode::Add as u64);
                let table_key_expression = meta.query_fixed(table_key, Rotation::cur());
                let table_value_expression = meta.query_fixed(table_value, Rotation::cur());
                let base = 4 * lane;
                // Logic membership is the sound tuple
                //   (2^23 + 8*mode + x + 2*y + 4*z, output).
                // The gate above independently makes x/y/z/output Boolean, so fractional or
                // cross-input encodings cannot masquerade as another truth-table row.
                let logic_key = Expression::Constant(F::from(LOGIC_KEY_BASE))
                    + logic_control * Expression::Constant(F::from(8))
                    + value[base].clone()
                    + value[base + 1].clone() * Expression::Constant(F::from(2))
                    + value[base + 2].clone() * Expression::Constant(F::from(4));
                let (add_key, add_value) = if lane == 0 {
                    // Lane zero checks (typed_key(1, output), spread(output)); for width one the
                    // spread value is exactly the Boolean output.
                    (
                        Expression::Constant(F::from(typed_key(1, 0))) + value[6].clone(),
                        value[6].clone(),
                    )
                } else {
                    // Lane one checks (2^22 + carry_out, carry_out), restricting the exact carry
                    // to 0..=4. Together with Boolean inputs and
                    // sum(inputs)+carry_in = output+2*carry_out, this is an exact <=5-word add.
                    (
                        Expression::Constant(F::from(CARRY_KEY_BASE)) + value[7].clone(),
                        value[7].clone(),
                    )
                };
                vec![
                    (
                        q_logic.clone() * logic_key + q_add.clone() * add_key,
                        table_key_expression,
                    ),
                    (
                        q_logic * value[base + 3].clone() + q_add * add_value,
                        table_value_expression,
                    ),
                ]
            });
        }

        meta.set_minimum_degree(MAX_DEGREE);
        Self {
            advice,
            instance,
            opcode,
            control,
            table_key,
            table_value,
        }
    }
}

pub(super) fn compact_sha_padded_blocks_v2(
    message_bytes: usize,
) -> Result<usize, CompactShaFailureV2> {
    u64::try_from(message_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_mul(8))
        .ok_or(CompactShaFailureV2::MessageGeometry)?;
    message_bytes
        .checked_add(9)
        .map(|bytes| bytes.div_ceil(64))
        .ok_or(CompactShaFailureV2::MessageGeometry)
}

impl<F: BigPrimeField> Circuit<F> for CompactShaCircuitV2<F> {
    type Config = CompactShaConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        CompactShaConfigV2::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.preflight().map_err(|_| Error::Synthesis)?;
        let trace = build_fixed_batch_trace::<F>(&self.abi, COMPACT_SHA_USABLE_ROWS_V2)
            .map_err(|_| Error::Synthesis)?;
        trace.assign(&config, &mut layouter)
    }
}

#[derive(Clone, Copy, Debug)]
struct Variable<F> {
    id: usize,
    value: F,
}

#[derive(Clone, Copy, Debug)]
struct Bit<F> {
    variable: Variable<F>,
    value: bool,
}

#[derive(Clone, Copy, Debug)]
struct Carry<F> {
    variable: Variable<F>,
    value: u8,
}

#[derive(Clone, Copy, Debug)]
struct ByteBits<F> {
    value: u8,
    bits: [Bit<F>; 8],
}

type Word<F> = [Bit<F>; 32];

#[derive(Clone, Debug)]
struct TraceRow<F> {
    values: [F; ADVICE_COLUMNS],
    aliases: Vec<(usize, usize)>,
    opcode: Opcode,
    control: u64,
}

impl<F: BigPrimeField> TraceRow<F> {
    fn zero() -> Self {
        Self {
            values: [F::ZERO; ADVICE_COLUMNS],
            aliases: Vec::new(),
            opcode: Opcode::Disabled,
            control: 0,
        }
    }

    fn set(&mut self, column: usize, variable: Variable<F>) {
        self.values[column] = variable.value;
        self.aliases.push((variable.id, column));
    }
}

#[derive(Clone, Copy, Debug)]
struct RowCounter {
    available: usize,
    used: usize,
}

impl RowCounter {
    const fn new(available: usize) -> Self {
        Self { available, used: 0 }
    }

    fn allocate(&mut self) -> Result<(), CompactShaFailureV2> {
        let attempted = self
            .used
            .checked_add(1)
            .ok_or(CompactShaFailureV2::CounterSpill {
                attempted: usize::MAX,
                available: self.available,
            })?;
        if attempted > self.available {
            return Err(CompactShaFailureV2::CounterSpill {
                attempted,
                available: self.available,
            });
        }
        self.used = attempted;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct TraceBuilder<F: BigPrimeField> {
    rows: Vec<TraceRow<F>>,
    equalities: Vec<(usize, usize)>,
    next_id: usize,
    counter: RowCounter,
    zero: Option<Bit<F>>,
    one: Option<Bit<F>>,
}

impl<F: BigPrimeField> TraceBuilder<F> {
    fn new(available: usize) -> Self {
        Self {
            rows: Vec::new(),
            equalities: Vec::new(),
            next_id: 0,
            counter: RowCounter::new(available),
            zero: None,
            one: None,
        }
    }

    fn variable(&mut self, value: F) -> Variable<F> {
        let variable = Variable {
            id: self.next_id,
            value,
        };
        self.next_id += 1;
        variable
    }

    fn bit(&mut self, value: bool) -> Bit<F> {
        Bit {
            variable: self.variable(F::from(u64::from(value))),
            value,
        }
    }

    fn push(&mut self, row: TraceRow<F>) -> Result<(), CompactShaFailureV2> {
        self.counter.allocate()?;
        self.rows.push(row);
        Ok(())
    }

    fn bind_public(&mut self, value: F) -> Result<Variable<F>, CompactShaFailureV2> {
        let variable = self.variable(value);
        let mut row = TraceRow::zero();
        row.opcode = Opcode::Public;
        row.set(7, variable);
        self.push(row)?;
        Ok(variable)
    }

    fn seed_bits(&mut self) -> Result<(Bit<F>, Bit<F>), CompactShaFailureV2> {
        let zero = self.bit(false);
        let mut zero_row = TraceRow::zero();
        zero_row.opcode = Opcode::Logic;
        zero_row.control = LogicMode::ForceZero as u64;
        for column in 0..ADVICE_COLUMNS {
            zero_row.set(column, zero.variable);
        }
        self.push(zero_row)?;
        self.zero = Some(zero);

        let one = self.bit(true);
        let mut one_row = TraceRow::zero();
        one_row.opcode = Opcode::Logic;
        one_row.control = LogicMode::ForceOne as u64;
        for lane in 0..2 {
            let base = 4 * lane;
            one_row.set(base, zero.variable);
            one_row.set(base + 1, zero.variable);
            one_row.set(base + 2, zero.variable);
            one_row.set(base + 3, one.variable);
        }
        self.push(one_row)?;
        self.one = Some(one);
        Ok((zero, one))
    }

    fn constants(&self) -> (Bit<F>, Bit<F>) {
        (
            self.zero
                .expect("zero seed precedes compact-SHA arithmetic"),
            self.one.expect("one seed precedes compact-SHA arithmetic"),
        )
    }

    fn fma_to(
        &mut self,
        left: Variable<F>,
        multiplier: Variable<F>,
        right: Variable<F>,
        output: Variable<F>,
    ) -> Result<(), CompactShaFailureV2> {
        let zero = self.constants().0.variable;
        let mut row = TraceRow::zero();
        row.opcode = Opcode::Fma;
        row.set(0, left);
        row.set(1, multiplier);
        row.set(2, right);
        row.set(3, output);
        for column in 4..8 {
            row.set(column, zero);
        }
        self.push(row)
    }

    fn fma(
        &mut self,
        left: Variable<F>,
        multiplier: Variable<F>,
        right: Variable<F>,
    ) -> Result<Variable<F>, CompactShaFailureV2> {
        let output = self.variable(left.value + multiplier.value * right.value);
        self.fma_to(left, multiplier, right, output)?;
        Ok(output)
    }

    fn radices(&mut self) -> Result<(Variable<F>, Variable<F>, Variable<F>), CompactShaFailureV2> {
        let (zero, one) = self.constants();
        let two = self.fma(one.variable, one.variable, one.variable)?;
        let four = self.fma(zero.variable, two, two)?;
        let sixteen = self.fma(zero.variable, four, four)?;
        let radix_256 = self.fma(zero.variable, sixteen, sixteen)?;
        let radix_65_536 = self.fma(zero.variable, radix_256, radix_256)?;
        let radix_2_32 = self.fma(zero.variable, radix_65_536, radix_65_536)?;
        Ok((two, radix_256, radix_2_32))
    }

    fn range_bits(&mut self, bits: &[Bit<F>]) -> Result<(), CompactShaFailureV2> {
        let zero = self.constants().0;
        for pair in bits.chunks(2) {
            let lanes = [pair[0], pair.get(1).copied().unwrap_or(zero)];
            let mut row = TraceRow::zero();
            row.opcode = Opcode::Logic;
            row.control = LogicMode::Range as u64;
            for (lane, bit) in lanes.into_iter().enumerate() {
                let base = 4 * lane;
                row.set(base, bit.variable);
                row.set(base + 1, zero.variable);
                row.set(base + 2, zero.variable);
                row.set(base + 3, bit.variable);
            }
            self.push(row)?;
        }
        Ok(())
    }

    fn decompose_byte(
        &mut self,
        byte: Variable<F>,
        value: u8,
        radix_two: Variable<F>,
    ) -> Result<ByteBits<F>, CompactShaFailureV2> {
        let bits = std::array::from_fn(|index| self.bit((value >> index) & 1 == 1));
        self.range_bits(&bits)?;
        let zero = self.constants().0.variable;
        let mut accumulator = zero;
        for index in (0..8).rev() {
            let output = if index == 0 {
                byte
            } else {
                self.variable(bits[index].variable.value + radix_two.value * accumulator.value)
            };
            self.fma_to(bits[index].variable, radix_two, accumulator, output)?;
            accumulator = output;
        }
        Ok(ByteBits { value, bits })
    }

    fn decompose_word(
        &mut self,
        word: Variable<F>,
        value: u32,
        radix_two: Variable<F>,
        radix_256: Variable<F>,
    ) -> Result<[ByteBits<F>; 4], CompactShaFailureV2> {
        let values = value.to_le_bytes();
        let byte_variables = values.map(|byte| self.variable(F::from(u64::from(byte))));
        let zero = self.constants().0.variable;
        let mut accumulator = zero;
        for index in (0..4).rev() {
            let output = if index == 0 {
                word
            } else {
                self.variable(byte_variables[index].value + radix_256.value * accumulator.value)
            };
            self.fma_to(byte_variables[index], radix_256, accumulator, output)?;
            accumulator = output;
        }
        let mut bytes = Vec::with_capacity(4);
        for (variable, value) in byte_variables.into_iter().zip(values) {
            bytes.push(self.decompose_byte(variable, value, radix_two)?);
        }
        bytes
            .try_into()
            .map_err(|_| CompactShaFailureV2::MessageGeometry)
    }

    fn constant_byte(&self, value: u8) -> ByteBits<F> {
        let (zero, one) = self.constants();
        ByteBits {
            value,
            bits: std::array::from_fn(|index| if (value >> index) & 1 == 1 { one } else { zero }),
        }
    }

    fn constant_word(&self, value: u32) -> Word<F> {
        let (zero, one) = self.constants();
        std::array::from_fn(|index| if (value >> index) & 1 == 1 { one } else { zero })
    }

    fn logic_word(
        &mut self,
        mode: LogicMode,
        first: &Word<F>,
        second: &Word<F>,
        third: &Word<F>,
    ) -> Result<Word<F>, CompactShaFailureV2> {
        let mut output = Vec::with_capacity(32);
        for index in 0..32 {
            let value = logic_value(
                mode,
                first[index].value,
                second[index].value,
                third[index].value,
            );
            output.push(self.bit(value));
        }
        for pair in (0..32).step_by(2) {
            let mut row = TraceRow::zero();
            row.opcode = Opcode::Logic;
            row.control = mode as u64;
            for lane in 0..2 {
                let index = pair + lane;
                let base = 4 * lane;
                row.set(base, first[index].variable);
                row.set(base + 1, second[index].variable);
                row.set(base + 2, third[index].variable);
                row.set(base + 3, output[index].variable);
            }
            self.push(row)?;
        }
        output
            .try_into()
            .map_err(|_| CompactShaFailureV2::MessageGeometry)
    }

    fn add_words(&mut self, words: &[Word<F>]) -> Result<Word<F>, CompactShaFailureV2> {
        if words.is_empty() || words.len() > 5 {
            return Err(CompactShaFailureV2::MessageGeometry);
        }
        let zero = self.constants().0;
        let mut carry = Carry {
            variable: zero.variable,
            value: 0,
        };
        let mut output = Vec::with_capacity(32);
        for index in 0..32 {
            let sum = words
                .iter()
                .map(|word| u8::from(word[index].value))
                .sum::<u8>()
                + carry.value;
            let bit = self.bit(sum & 1 == 1);
            let next_carry = Carry {
                variable: self.variable(F::from(u64::from(sum >> 1))),
                value: sum >> 1,
            };
            let mut row = TraceRow::zero();
            row.opcode = Opcode::Add;
            for column in 0..5 {
                row.set(
                    column,
                    words
                        .get(column)
                        .map_or(zero.variable, |word| word[index].variable),
                );
            }
            row.set(5, carry.variable);
            row.set(6, bit.variable);
            row.set(7, next_carry.variable);
            self.push(row)?;
            output.push(bit);
            carry = next_carry;
        }
        output
            .try_into()
            .map_err(|_| CompactShaFailureV2::MessageGeometry)
    }

    fn assert_equal(&mut self, left: Variable<F>, right: Variable<F>) {
        self.equalities.push((left.id, right.id));
    }

    fn constrain_byte_constant(
        &mut self,
        byte: &ByteBits<F>,
        expected: u8,
    ) -> Result<(), CompactShaFailureV2> {
        if byte.value != expected {
            return Err(CompactShaFailureV2::MessageGeometry);
        }
        let expected = self.constant_byte(expected);
        for index in 0..8 {
            self.assert_equal(byte.bits[index].variable, expected.bits[index].variable);
        }
        Ok(())
    }

    fn finish(self) -> CompactShaTrace<F> {
        CompactShaTrace {
            rows: self.rows,
            equalities: self.equalities,
        }
    }
}

fn logic_value(mode: LogicMode, x: bool, y: bool, z: bool) -> bool {
    match mode {
        LogicMode::Xor3 => x ^ y ^ z,
        LogicMode::Choose => (x & y) ^ (!x & z),
        LogicMode::Majority => (x & y) ^ (x & z) ^ (y & z),
        LogicMode::Range => x,
        LogicMode::ForceZero => false,
        LogicMode::ForceOne => true,
    }
}

fn rotate_right<F: BigPrimeField>(word: &Word<F>, amount: usize) -> Word<F> {
    std::array::from_fn(|index| word[(index + amount) % 32])
}

fn shift_right<F: BigPrimeField>(word: &Word<F>, amount: usize, zero: Bit<F>) -> Word<F> {
    std::array::from_fn(|index| word.get(index + amount).copied().unwrap_or(zero))
}

fn small_sigma_zero<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    word: &Word<F>,
) -> Result<Word<F>, CompactShaFailureV2> {
    let zero = builder.constants().0;
    builder.logic_word(
        LogicMode::Xor3,
        &rotate_right(word, 7),
        &rotate_right(word, 18),
        &shift_right(word, 3, zero),
    )
}

fn small_sigma_one<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    word: &Word<F>,
) -> Result<Word<F>, CompactShaFailureV2> {
    let zero = builder.constants().0;
    builder.logic_word(
        LogicMode::Xor3,
        &rotate_right(word, 17),
        &rotate_right(word, 19),
        &shift_right(word, 10, zero),
    )
}

fn big_sigma_zero<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    word: &Word<F>,
) -> Result<Word<F>, CompactShaFailureV2> {
    builder.logic_word(
        LogicMode::Xor3,
        &rotate_right(word, 2),
        &rotate_right(word, 13),
        &rotate_right(word, 22),
    )
}

fn big_sigma_one<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    word: &Word<F>,
) -> Result<Word<F>, CompactShaFailureV2> {
    builder.logic_word(
        LogicMode::Xor3,
        &rotate_right(word, 6),
        &rotate_right(word, 11),
        &rotate_right(word, 25),
    )
}

fn padded_blocks<F: BigPrimeField>(
    builder: &TraceBuilder<F>,
    message: &[ByteBits<F>],
) -> Result<Vec<[Word<F>; 16]>, CompactShaFailureV2> {
    let bit_length = u64::try_from(message.len())
        .ok()
        .and_then(|length| length.checked_mul(8))
        .ok_or(CompactShaFailureV2::MessageGeometry)?;
    let mut padded = message.to_vec();
    padded.push(builder.constant_byte(0x80));
    while padded.len() % 64 != 56 {
        padded.push(builder.constant_byte(0));
    }
    padded.extend(
        bit_length
            .to_be_bytes()
            .into_iter()
            .map(|byte| builder.constant_byte(byte)),
    );
    let mut blocks = Vec::with_capacity(padded.len() / 64);
    for block in padded.chunks_exact(64) {
        let mut words = Vec::with_capacity(16);
        for bytes in block.chunks_exact(4) {
            words.push(std::array::from_fn(|index| {
                let byte = 3 - index / 8;
                bytes[byte].bits[index % 8]
            }));
        }
        blocks.push(
            words
                .try_into()
                .map_err(|_| CompactShaFailureV2::MessageGeometry)?,
        );
    }
    Ok(blocks)
}

fn constrain_sha256<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    message: &[ByteBits<F>],
    expected: &[ByteBits<F>; 32],
) -> Result<(), CompactShaFailureV2> {
    let blocks = padded_blocks(builder, message)?;
    let mut state: [Word<F>; 8] = IV.map(|word| builder.constant_word(word));
    for block in blocks {
        let mut schedule = block.to_vec();
        for index in 16..64 {
            let sigma_zero = small_sigma_zero(builder, &schedule[index - 15])?;
            let sigma_one = small_sigma_one(builder, &schedule[index - 2])?;
            schedule.push(builder.add_words(&[
                schedule[index - 16],
                sigma_zero,
                schedule[index - 7],
                sigma_one,
            ])?);
        }

        let initial = state;
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for round in 0..64 {
            let sigma_one = big_sigma_one(builder, &e)?;
            let choose = builder.logic_word(LogicMode::Choose, &e, &f, &g)?;
            let round_constant = builder.constant_word(ROUND_CONSTANTS[round]);
            let first =
                builder.add_words(&[h, sigma_one, choose, round_constant, schedule[round]])?;
            let sigma_zero = big_sigma_zero(builder, &a)?;
            let majority = builder.logic_word(LogicMode::Majority, &a, &b, &c)?;
            let second = builder.add_words(&[sigma_zero, majority])?;
            let next_e = builder.add_words(&[d, first])?;
            let next_a = builder.add_words(&[first, second])?;
            h = g;
            g = f;
            f = e;
            e = next_e;
            d = c;
            c = b;
            b = a;
            a = next_a;
        }
        let working = [a, b, c, d, e, f, g, h];
        for index in 0..8 {
            state[index] = builder.add_words(&[initial[index], working[index]])?;
        }
    }

    for (byte_index, expected_byte) in expected.iter().enumerate() {
        let word = byte_index / 4;
        let word_byte = 3 - byte_index % 4;
        for bit in 0..8 {
            builder.assert_equal(
                state[word][word_byte * 8 + bit].variable,
                expected_byte.bits[bit].variable,
            );
        }
    }
    Ok(())
}

fn flatten_words<F: BigPrimeField>(
    words: &[[ByteBits<F>; 4]],
    start: usize,
    count: usize,
) -> Vec<ByteBits<F>> {
    words[start..start + count]
        .iter()
        .flat_map(|word| word.iter().copied())
        .collect()
}

fn digest_at<F: BigPrimeField>(words: &[[ByteBits<F>; 4]], start: usize) -> [ByteBits<F>; 32] {
    flatten_words(words, start, 8)
        .try_into()
        .expect("eight ABI words are one SHA-256 digest")
}

fn framed_bits<F: BigPrimeField>(
    builder: &TraceBuilder<F>,
    domain: &[u8],
    fields: &[&[ByteBits<F>]],
) -> Result<Vec<ByteBits<F>>, CompactShaFailureV2> {
    let mut output = Vec::new();
    let domain_length =
        u64::try_from(domain.len()).map_err(|_| CompactShaFailureV2::MessageGeometry)?;
    output.extend(
        domain_length
            .to_le_bytes()
            .map(|byte| builder.constant_byte(byte)),
    );
    output.extend(domain.iter().map(|byte| builder.constant_byte(*byte)));
    for field in fields {
        let field_length =
            u64::try_from(field.len()).map_err(|_| CompactShaFailureV2::MessageGeometry)?;
        output.extend(
            field_length
                .to_le_bytes()
                .map(|byte| builder.constant_byte(byte)),
        );
        output.extend_from_slice(field);
    }
    Ok(output)
}

fn fixed_jobs<F: BigPrimeField>(
    builder: &TraceBuilder<F>,
    words: &[[ByteBits<F>; 4]],
) -> Result<([Vec<ByteBits<F>>; 9], [[ByteBits<F>; 32]; 9]), CompactShaFailureV2> {
    const CURRENT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:current-guard";
    const NEXT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:next-guard";
    const PLATFORM_MESSAGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-message";
    const GUARD_USE_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-use-claim";
    const PLATFORM_BIND_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-bind-claim";
    const ANDROID_KEY_CERT_CLAIM_DOMAIN: &[u8] =
        b"iroha:offline-cash:v1:helper:android-key-cert-claim";
    const GUARD_BUNDLE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-bundle";

    let operation = vec![words[5][0]];
    let android_present = vec![words[6][0]];
    let from = flatten_words(words, 8, 2);
    let to = flatten_words(words, 10, 2);
    let release = digest_at(words, 24).to_vec();
    let context = digest_at(words, 32).to_vec();
    let current_head = digest_at(words, 40).to_vec();
    let current_lineage = digest_at(words, 48).to_vec();
    let transition = digest_at(words, 56).to_vec();
    let wallet = digest_at(words, 64).to_vec();
    let policy = digest_at(words, 72).to_vec();
    let device = digest_at(words, 80).to_vec();
    let current_guard = digest_at(words, 88).to_vec();
    let next_guard = digest_at(words, 96).to_vec();
    let platform_key_digest = digest_at(words, 104).to_vec();
    let platform_message = digest_at(words, 112).to_vec();
    let guard_use = digest_at(words, 120).to_vec();
    let platform_bind = digest_at(words, 128).to_vec();
    let certificate = digest_at(words, 136).to_vec();
    let tbs = digest_at(words, 144).to_vec();
    let issuer = digest_at(words, 152).to_vec();
    let attestation = digest_at(words, 160).to_vec();
    let android_claim = digest_at(words, 168).to_vec();
    let platform_key =
        flatten_words(words, COMPACT_SHA_HELPER_WORDS_V2, COMPACT_SHA_KEY_WORDS_V2)[..65].to_vec();
    let issuer_key = flatten_words(
        words,
        COMPACT_SHA_HELPER_WORDS_V2 + COMPACT_SHA_KEY_WORDS_V2,
        COMPACT_SHA_KEY_WORDS_V2,
    )[..65]
        .to_vec();
    let algorithm = b"ecdsa-p256-sha256"
        .iter()
        .map(|byte| builder.constant_byte(*byte))
        .collect::<Vec<_>>();
    let origin = b"generated-in-keymint-hardware"
        .iter()
        .map(|byte| builder.constant_byte(*byte))
        .collect::<Vec<_>>();
    let purpose = b"sign"
        .iter()
        .map(|byte| builder.constant_byte(*byte))
        .collect::<Vec<_>>();
    let digest_mode = b"sha-256"
        .iter()
        .map(|byte| builder.constant_byte(*byte))
        .collect::<Vec<_>>();
    let usage_limit = 1_u32
        .to_le_bytes()
        .map(|byte| builder.constant_byte(byte))
        .to_vec();

    let messages = [
        framed_bits(
            builder,
            CURRENT_GUARD_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &wallet,
                &policy,
                &device,
                &from,
            ],
        )?,
        framed_bits(
            builder,
            NEXT_GUARD_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &to,
            ],
        )?,
        framed_bits(
            builder,
            PLATFORM_MESSAGE_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &next_guard,
                &from,
                &to,
            ],
        )?,
        platform_key,
        framed_bits(
            builder,
            GUARD_USE_CLAIM_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &next_guard,
                &from,
                &to,
                &platform_message,
            ],
        )?,
        framed_bits(
            builder,
            PLATFORM_BIND_CLAIM_DOMAIN,
            &[
                &release,
                &policy,
                &wallet,
                &device,
                &platform_key_digest,
                &platform_message,
                &current_guard,
                &next_guard,
            ],
        )?,
        issuer_key,
        framed_bits(
            builder,
            ANDROID_KEY_CERT_CLAIM_DOMAIN,
            &[
                &release,
                &policy,
                &device,
                &platform_key_digest,
                &certificate,
                &tbs,
                &issuer,
                &attestation,
                &algorithm,
                &origin,
                &purpose,
                &digest_mode,
                &usage_limit,
            ],
        )?,
        framed_bits(
            builder,
            GUARD_BUNDLE_DOMAIN,
            &[
                &operation,
                &android_present,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &next_guard,
                &from,
                &to,
                &guard_use,
                &platform_bind,
                &android_claim,
            ],
        )?,
    ];
    if messages.each_ref().map(Vec::len) != COMPACT_SHA_FIXED_MESSAGE_BYTES_V2 {
        return Err(CompactShaFailureV2::MessageGeometry);
    }
    let expected = [
        digest_at(words, 88),
        digest_at(words, 96),
        digest_at(words, 112),
        digest_at(words, 104),
        digest_at(words, 120),
        digest_at(words, 128),
        digest_at(words, 152),
        digest_at(words, 168),
        digest_at(words, 176),
    ];
    Ok((messages, expected))
}

struct FixedBatchPrefix<F: BigPrimeField> {
    builder: TraceBuilder<F>,
    messages: [Vec<ByteBits<F>>; 9],
    expected: [[ByteBits<F>; 32]; 9],
}

fn build_fixed_batch_prefix<F: BigPrimeField>(
    abi: &CompactShaBatchPublicAbiV2,
    available: usize,
) -> Result<FixedBatchPrefix<F>, CompactShaFailureV2> {
    let host_messages = abi.fixed_messages()?;
    if host_messages.each_ref().map(Vec::len) != COMPACT_SHA_FIXED_MESSAGE_BYTES_V2 {
        return Err(CompactShaFailureV2::MessageGeometry);
    }
    let mut builder = TraceBuilder::<F>::new(available);
    let instance_values = abi.field_instances::<F>();
    let mut public = Vec::with_capacity(COMPACT_SHA_BATCH_INSTANCE_CELLS_V2);
    for value in instance_values {
        public.push(builder.bind_public(value)?);
    }
    let (zero, _) = builder.seed_bits()?;
    let (radix_two, radix_256, radix_2_32) = builder.radices()?;
    let host_words = abi.words();
    let word_variables = host_words
        .iter()
        .map(|word| builder.variable(F::from(u64::from(*word))))
        .collect::<Vec<_>>();
    for (cell, bound) in public.into_iter().enumerate() {
        let mut accumulator = zero.variable;
        for slot in (0..7).rev() {
            let index = cell * 7 + slot;
            let word = word_variables.get(index).copied().unwrap_or(zero.variable);
            let output = if slot == 0 {
                bound
            } else {
                builder.variable(word.value + radix_2_32.value * accumulator.value)
            };
            builder.fma_to(word, radix_2_32, accumulator, output)?;
            accumulator = output;
        }
    }
    let mut words = Vec::with_capacity(host_words.len());
    for (variable, value) in word_variables.into_iter().zip(host_words) {
        words.push(builder.decompose_word(variable, value, radix_two, radix_256)?);
    }

    // Direct-instance verifiers must not be able to smuggle ignored bytes through either
    // 17-word SEC1 encoding. The 65-byte payload starts with uncompressed tag 0x04; bytes 65..68
    // in the final word are canonical zero padding. These equalities cost no additional rows.
    for start in [
        COMPACT_SHA_HELPER_WORDS_V2,
        COMPACT_SHA_HELPER_WORDS_V2 + COMPACT_SHA_KEY_WORDS_V2,
    ] {
        let prefix = words[start][0];
        builder.constrain_byte_constant(&prefix, 0x04)?;
        for byte_index in 1..4 {
            let padding = words[start + COMPACT_SHA_KEY_WORDS_V2 - 1][byte_index];
            builder.constrain_byte_constant(&padding, 0)?;
        }
    }

    let (messages, expected) = fixed_jobs(&builder, &words)?;
    for index in 0..9 {
        if messages[index]
            .iter()
            .map(|byte| byte.value)
            .ne(host_messages[index].iter().copied())
        {
            return Err(CompactShaFailureV2::MessageGeometry);
        }
    }
    Ok(FixedBatchPrefix {
        builder,
        messages,
        expected,
    })
}

fn build_fixed_batch_trace<F: BigPrimeField>(
    abi: &CompactShaBatchPublicAbiV2,
    available: usize,
) -> Result<CompactShaTrace<F>, CompactShaFailureV2> {
    if COMPACT_SHA_BATCH_REQUIRED_ROWS_V2 > available {
        return Err(CompactShaFailureV2::RowLimit {
            required: COMPACT_SHA_BATCH_REQUIRED_ROWS_V2,
            available,
        });
    }
    let FixedBatchPrefix {
        mut builder,
        messages,
        expected,
    } = build_fixed_batch_prefix::<F>(abi, available)?;
    for index in 0..9 {
        let actual_blocks = compact_sha_padded_blocks_v2(messages[index].len())?;
        if actual_blocks != COMPACT_SHA_FIXED_MESSAGE_BLOCKS_V2[index] {
            return Err(CompactShaFailureV2::MessageGeometry);
        }
        constrain_sha256(&mut builder, &messages[index], &expected[index])?;
    }
    Ok(builder.finish())
}

#[derive(Clone, Debug)]
struct CompactShaTrace<F: BigPrimeField> {
    rows: Vec<TraceRow<F>>,
    equalities: Vec<(usize, usize)>,
}

impl<F: BigPrimeField> CompactShaTrace<F> {
    fn assign(
        &self,
        config: &CompactShaConfigV2,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), Error> {
        let assigned_rows = self.rows.len().max(COMPACT_SHA_TABLE_ROWS_V2);
        if assigned_rows > COMPACT_SHA_USABLE_ROWS_V2 {
            return Err(Error::Synthesis);
        }
        let table = table_entries();
        layouter.assign_region(
            || "offline cash V2 compact SHA trace and typed spread table",
            |mut region| {
                let mut first_cells = HashMap::<usize, Cell>::new();
                for offset in 0..assigned_rows {
                    let row = self.rows.get(offset);
                    let values = row.map_or([F::ZERO; ADVICE_COLUMNS], |row| row.values);
                    let cells: [Cell; ADVICE_COLUMNS] = std::array::from_fn(|column| {
                        raw_assign_advice(
                            &mut region,
                            config.advice[column],
                            offset,
                            Value::known(Assigned::Trivial(values[column])),
                        )
                        .cell()
                    });
                    raw_assign_fixed(
                        &mut region,
                        config.opcode,
                        offset,
                        F::from(row.map_or(Opcode::Disabled as u64, |row| row.opcode as u64)),
                    );
                    raw_assign_fixed(
                        &mut region,
                        config.control,
                        offset,
                        F::from(row.map_or(0, |row| row.control)),
                    );
                    let (key, table_value) = table.get(offset).copied().unwrap_or((0, 0));
                    raw_assign_fixed(&mut region, config.table_key, offset, F::from(key));
                    raw_assign_fixed(
                        &mut region,
                        config.table_value,
                        offset,
                        F::from(table_value),
                    );
                    if let Some(row) = row {
                        for (variable, column) in &row.aliases {
                            if let Some(first) = first_cells.insert(*variable, cells[*column]) {
                                raw_constrain_equal(&mut region, first, cells[*column]);
                            }
                        }
                    }
                }
                for (left, right) in &self.equalities {
                    let left = first_cells.get(left).ok_or(Error::Synthesis)?;
                    let right = first_cells.get(right).ok_or(Error::Synthesis)?;
                    raw_constrain_equal(&mut region, *left, *right);
                }
                Ok(())
            },
        )
    }
}

const fn typed_key(width: usize, value: u64) -> u64 {
    ((width as u64) << TYPE_KEY_SHIFT) | value
}

fn spread(value: u64, width: usize) -> u64 {
    (0..width).fold(0, |spread, bit| {
        spread | (((value >> bit) & 1) << (2 * bit))
    })
}

fn table_entries() -> Vec<(u64, u64)> {
    let mut table = Vec::with_capacity(COMPACT_SHA_TABLE_ROWS_V2);
    table.push((0, 0));
    for width in COMPACT_SHA_SPREAD_WIDTHS_V2 {
        for value in 0..(1_u64 << width) {
            table.push((typed_key(width, value), spread(value, width)));
        }
    }
    for carry in 0..CARRY_ROWS as u64 {
        table.push((CARRY_KEY_BASE + carry, carry));
    }
    for mode in 0..LOGIC_MODES as u64 {
        for packed in 0..8_u64 {
            let x = packed & 1 != 0;
            let y = packed & 2 != 0;
            let z = packed & 4 != 0;
            let mode = match mode {
                0 => LogicMode::Xor3,
                1 => LogicMode::Choose,
                2 => LogicMode::Majority,
                3 => LogicMode::Range,
                4 => LogicMode::ForceZero,
                5 => LogicMode::ForceOne,
                _ => unreachable!("logic table mode is bounded"),
            };
            table.push((
                LOGIC_KEY_BASE + mode as u64 * 8 + packed,
                u64::from(logic_value(mode, x, y, z)),
            ));
        }
    }
    debug_assert_eq!(table.len(), COMPACT_SHA_TABLE_ROWS_V2);
    table
}

/// Test-only audit that materializes only the 11,600-row ABI prefix, exercises the exact
/// circuit-side fixed-job routing, and counts (without allocating) every SHA operation row.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompactShaCountingAuditV2 {
    pub(super) circuit_messages: [Vec<u8>; 9],
    pub(super) circuit_expected_digests: [[u8; 32]; 9],
    pub(super) materialized_abi_rows: usize,
    pub(super) counted_schedule_rows: usize,
    pub(super) counted_compression_rows: usize,
    pub(super) counted_feed_forward_rows: usize,
    pub(super) counted_assigned_rows: usize,
    pub(super) lookup_table_rows: usize,
}

#[cfg(test)]
fn count_compact_sha_message_operations_v2(
    message_bytes: usize,
) -> Result<(usize, usize, usize), CompactShaFailureV2> {
    fn add_rows(target: &mut usize, rows: usize) -> Result<(), CompactShaFailureV2> {
        *target = (*target)
            .checked_add(rows)
            .ok_or(CompactShaFailureV2::MessageGeometry)?;
        Ok(())
    }

    let blocks = compact_sha_padded_blocks_v2(message_bytes)?;
    let mut schedule = 0_usize;
    let mut compression = 0_usize;
    let mut feed_forward = 0_usize;
    for _ in 0..blocks {
        for _ in 16..64 {
            // small_sigma_zero, small_sigma_one, then one four-word schedule addition.
            for rows in [16, 16, 32] {
                add_rows(&mut schedule, rows)?;
            }
        }
        for _ in 0..64 {
            // big_sigma_one, choose, first, big_sigma_zero, majority, second, next_e, next_a.
            for rows in [16, 16, 32, 16, 16, 32, 32, 32] {
                add_rows(&mut compression, rows)?;
            }
        }
        for _ in 0..8 {
            add_rows(&mut feed_forward, 32)?;
        }
    }
    Ok((schedule, compression, feed_forward))
}

#[cfg(test)]
pub(super) fn compact_sha_counting_audit_v2<F: BigPrimeField>(
    abi: &CompactShaBatchPublicAbiV2,
) -> Result<CompactShaCountingAuditV2, CompactShaFailureV2> {
    let FixedBatchPrefix {
        builder,
        messages,
        expected,
    } = build_fixed_batch_prefix::<F>(abi, COMPACT_SHA_ABI_BINDING_ROWS_V2)?;
    let materialized_abi_rows = builder.counter.used;
    if materialized_abi_rows != COMPACT_SHA_ABI_BINDING_ROWS_V2
        || builder.rows.len() != materialized_abi_rows
    {
        return Err(CompactShaFailureV2::MessageGeometry);
    }
    let circuit_messages = messages.map(|message| {
        message
            .into_iter()
            .map(|byte| byte.value)
            .collect::<Vec<_>>()
    });
    let circuit_expected_digests = expected.map(|digest| digest.map(|byte| byte.value));

    let mut counted_schedule_rows = 0_usize;
    let mut counted_compression_rows = 0_usize;
    let mut counted_feed_forward_rows = 0_usize;
    for (index, message) in circuit_messages.iter().enumerate() {
        let blocks = compact_sha_padded_blocks_v2(message.len())?;
        if blocks != COMPACT_SHA_FIXED_MESSAGE_BLOCKS_V2[index] {
            return Err(CompactShaFailureV2::MessageGeometry);
        }
        let (schedule, compression, feed_forward) =
            count_compact_sha_message_operations_v2(message.len())?;
        counted_schedule_rows = counted_schedule_rows
            .checked_add(schedule)
            .ok_or(CompactShaFailureV2::MessageGeometry)?;
        counted_compression_rows = counted_compression_rows
            .checked_add(compression)
            .ok_or(CompactShaFailureV2::MessageGeometry)?;
        counted_feed_forward_rows = counted_feed_forward_rows
            .checked_add(feed_forward)
            .ok_or(CompactShaFailureV2::MessageGeometry)?;
    }
    let counted_assigned_rows = materialized_abi_rows
        .checked_add(counted_schedule_rows)
        .and_then(|rows| rows.checked_add(counted_compression_rows))
        .and_then(|rows| rows.checked_add(counted_feed_forward_rows))
        .ok_or(CompactShaFailureV2::MessageGeometry)?;
    if counted_assigned_rows != COMPACT_SHA_BATCH_REQUIRED_ROWS_V2 {
        return Err(CompactShaFailureV2::MessageGeometry);
    }
    Ok(CompactShaCountingAuditV2 {
        circuit_messages,
        circuit_expected_digests,
        materialized_abi_rows,
        counted_schedule_rows,
        counted_compression_rows,
        counted_feed_forward_rows,
        counted_assigned_rows,
        lookup_table_rows: COMPACT_SHA_TABLE_ROWS_V2,
    })
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) struct CompactShaDiagnosticCircuitV2<F: BigPrimeField> {
    message: Vec<u8>,
    expected: [u8; 32],
    _marker: PhantomData<F>,
}

#[cfg(test)]
impl<F: BigPrimeField> CompactShaDiagnosticCircuitV2<F> {
    pub(super) fn new(message: Vec<u8>, expected: [u8; 32]) -> Self {
        Self {
            message,
            expected,
            _marker: PhantomData,
        }
    }

    pub(super) fn public_instances(&self) -> Vec<F> {
        self.message
            .iter()
            .chain(&self.expected)
            .map(|byte| F::from(u64::from(*byte)))
            .collect()
    }
}

#[cfg(test)]
impl<F: BigPrimeField> Circuit<F> for CompactShaDiagnosticCircuitV2<F> {
    type Config = CompactShaConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        CompactShaConfigV2::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        diagnostic_trace::<F>(&self.message, self.expected)
            .map_err(|_| Error::Synthesis)?
            .assign(&config, &mut layouter)
    }
}

#[cfg(test)]
fn diagnostic_trace<F: BigPrimeField>(
    message: &[u8],
    expected: [u8; 32],
) -> Result<CompactShaTrace<F>, CompactShaFailureV2> {
    let mut builder = TraceBuilder::<F>::new(COMPACT_SHA_USABLE_ROWS_V2);
    let values = message.iter().chain(&expected).copied().collect::<Vec<_>>();
    let mut public = Vec::with_capacity(values.len());
    for value in &values {
        public.push(builder.bind_public(F::from(u64::from(*value)))?);
    }
    builder.seed_bits()?;
    let (radix_two, _, _) = builder.radices()?;
    let mut bytes = Vec::with_capacity(values.len());
    for (variable, value) in public.into_iter().zip(values) {
        bytes.push(builder.decompose_byte(variable, value, radix_two)?);
    }
    let (message_bits, expected_bits) = bytes.split_at(message.len());
    let expected_bits: [ByteBits<F>; 32] = expected_bits
        .try_into()
        .map_err(|_| CompactShaFailureV2::MessageGeometry)?;
    constrain_sha256(&mut builder, message_bits, &expected_bits)?;
    Ok(builder.finish())
}

#[cfg(test)]
pub(super) fn compact_sha_table_entries_v2() -> Vec<(u64, u64)> {
    table_entries()
}
