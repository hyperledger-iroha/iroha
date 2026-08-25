//! Eight-column current-row compiler for compact Offline Cash wrappers.
//!
//! Halo2-base records every arithmetic relation as a virtual four-cell
//! `a + b * c = out` gate.  This module transposes that virtual graph into one
//! current-row packed gate, preserving every virtual equality, constant
//! binding, range lookup, and direct public-instance binding.  The physical
//! profile is deliberately fixed to eight equality-enabled advice columns,
//! two direct instance columns, one opcode fixed column, and one two-column
//! typed lookup table.  At degree ten this is the exact 3,072-byte ordinary
//! Pasta-IPA shape; changing any column/query geometry changes the
//! authenticated protocol before artifact generation.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use halo2_base::{
    AssignedValue, Context, ContextCell,
    gates::circuit::builder::BaseCircuitBuilder,
    halo2_proofs::{
        circuit::{Cell, Layouter, Value},
        plonk::{Advice, Assigned, Column, ConstraintSystem, Error, Expression, Fixed, Instance},
        poly::Rotation,
    },
    utils::{
        BigPrimeField, fe_to_biguint,
        halo2::{raw_assign_advice, raw_assign_fixed, raw_constrain_equal},
    },
};

use iroha_data_model::offline::OFFLINE_CASH_HALO2_K_V1;
use sha2::{Digest as _, Sha256};

use crate::zk::kagemusha_sha256_v4::{
    KagemushaConstrainedSha256V1, KagemushaSha256ByteBindingV1, KagemushaSha256ByteV4,
};

const ADVICE_COLUMNS: usize = 8;
const DIRECT_INSTANCE_COLUMNS: usize = 2;
const LOOKUP_BITS: usize = 15;
const OP_PADDING: u64 = 0;
const OP_ARITHMETIC: u64 = OP_PADDING + 1;
const OP_RANGE_OR_BYTE: u64 = 2;
const OP_CONSTANT: u64 = 3;
const OP_XOR_OR_ROTATE: u64 = 4;
const OP_CHOICE: u64 = 5;
const OP_MAJORITY: u64 = 6;
const OP_ADD: u64 = 7;
const OP_SPLIT_ADD: u64 = 8;
const OP_INSTANCE: u64 = 9;
const LOOKUP_OPCODES: [u64; 7] = [
    OP_RANGE_OR_BYTE,
    OP_CONSTANT,
    OP_XOR_OR_ROTATE,
    OP_CHOICE,
    OP_MAJORITY,
    OP_ADD,
    OP_SPLIT_ADD,
];
const TUPLE_RADIX: u64 = 1 << 16;
const MINIMUM_UNUSABLE_ROWS: usize = 9;
const USABLE_ROWS: usize = (1 << OFFLINE_CASH_HALO2_K_V1) - MINIMUM_UNUSABLE_ROWS;
const SHA256_DIGEST_WORDS: usize = 8;

#[derive(Clone, Debug)]
struct OfflineCashPackedSha256JobV1<F: BigPrimeField> {
    message: Vec<KagemushaSha256ByteBindingV1<F>>,
    output_words: [AssignedValue<F>; SHA256_DIGEST_WORDS],
}

/// Source-authoritative SHA-256 jobs compiled into the same eight-column
/// current-row trace as the Base graph. Host hashing supplies witnesses only;
/// every byte, compression round, carry, and digest word is reconstructed by
/// the typed table before it can be equal to the Base output cells.
#[derive(Clone, Debug, Default)]
pub(super) struct OfflineCashPackedSha256JobsV1<F: BigPrimeField> {
    jobs: Vec<OfflineCashPackedSha256JobV1<F>>,
}

impl<F: BigPrimeField> OfflineCashPackedSha256JobsV1<F> {
    fn block_count_v1(&self) -> usize {
        self.jobs
            .iter()
            .map(|job| {
                job.message
                    .len()
                    .checked_add(9)
                    .expect("bounded packed SHA-256 message length")
                    .div_ceil(64)
            })
            .sum()
    }
}

impl<F> KagemushaConstrainedSha256V1<F> for OfflineCashPackedSha256JobsV1<F>
where
    F: BigPrimeField,
{
    fn digest_constrained_v1(
        &mut self,
        ctx: &mut Context<F>,
        message: &[KagemushaSha256ByteV4<F>],
    ) -> Result<[AssignedValue<F>; SHA256_DIGEST_WORDS], String> {
        let bindings = message
            .iter()
            .copied()
            .map(KagemushaSha256ByteV4::binding_v1)
            .collect::<Vec<_>>();
        let bytes = bindings
            .iter()
            .enumerate()
            .map(|(index, binding)| match binding {
                KagemushaSha256ByteBindingV1::Constant(byte) => Ok(*byte),
                KagemushaSha256ByteBindingV1::Assigned(value) => {
                    if value.cell.is_none() {
                        return Err(format!(
                            "packed SHA-256 message cell {index} has no virtual identity"
                        ));
                    }
                    u8::try_from(fe_to_biguint(value.value())).map_err(|_| {
                        format!("packed SHA-256 message cell {index} is not a canonical byte")
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let digest = Sha256::digest(&bytes);
        let output_words = std::array::from_fn(|index| {
            let start = index * 4;
            let word = u32::from_be_bytes(
                digest[start..start + 4]
                    .try_into()
                    .expect("SHA-256 word width"),
            );
            ctx.load_witness(F::from(u64::from(word)))
        });
        self.jobs.push(OfflineCashPackedSha256JobV1 {
            message: bindings,
            output_words,
        });
        Ok(output_words)
    }
}

#[derive(Clone, Debug)]
pub(super) struct OfflineCashPackedBaseConfigV1<const INSTANCE_COLUMNS: usize = 2> {
    advice: [Column<Advice>; ADVICE_COLUMNS],
    instances: [Column<Instance>; INSTANCE_COLUMNS],
    opcode: Column<Fixed>,
    table_tag: Column<Fixed>,
    table_value: Column<Fixed>,
}

impl<const INSTANCE_COLUMNS: usize> OfflineCashPackedBaseConfigV1<INSTANCE_COLUMNS> {
    pub(super) fn configure<F: BigPrimeField>(meta: &mut ConstraintSystem<F>) -> Self {
        assert!(INSTANCE_COLUMNS >= DIRECT_INSTANCE_COLUMNS);
        let advice = std::array::from_fn(|_| {
            let column = meta.advice_column();
            meta.enable_equality(column);
            column
        });
        let instances = std::array::from_fn(|_| meta.instance_column());
        for instance in &instances[DIRECT_INSTANCE_COLUMNS..] {
            meta.enable_equality(*instance);
        }
        let opcode = meta.fixed_column();
        let table_tag = meta.fixed_column();
        let table_value = meta.fixed_column();

        meta.create_gate("Offline Cash packed current-row Base graph", |meta| {
            let values = advice.map(|column| meta.query_advice(column, Rotation::cur()));
            let public_zero = meta.query_instance(instances[0], Rotation::cur());
            let public_one = meta.query_instance(instances[1], Rotation::cur());
            let op = meta.query_fixed(opcode, Rotation::cur());
            // Opcode zero is reserved for unassigned/blinded rows.  Instance
            // rows are deliberately arithmetic-shaped, so this degree-eight
            // selector may enable both opcodes one and nine while vanishing
            // on padding and every typed-lookup row.
            let arithmetic = (OP_RANGE_OR_BYTE..=OP_SPLIT_ADD)
                .fold(Expression::Constant(F::ONE), |product, root| {
                    product * (op.clone() - Expression::Constant(F::from(root)))
                });
            let bind = (OP_ARITHMETIC..OP_INSTANCE)
                .fold(Expression::Constant(F::ONE), |product, root| {
                    product * (op.clone() - Expression::Constant(F::from(root)))
                });
            let constant = lagrange_opcode_selector_v1::<F>(op.clone(), OP_CONSTANT);
            let fixed_value = meta.query_fixed(table_value, Rotation::cur());
            let constant_lookup_scale = Expression::Constant(lookup_scale_v1::<F>(OP_CONSTANT));
            vec![
                op.clone()
                    * (arithmetic.clone()
                        * (values[0].clone() + values[1].clone() * values[2].clone()
                            - values[3].clone())),
                op.clone()
                    * (arithmetic
                        * (values[4].clone() + values[5].clone() * values[6].clone()
                            - values[7].clone())),
                op.clone() * (bind.clone() * (values[0].clone() - public_zero)),
                op.clone() * (bind * (values[4].clone() - public_one)),
                op * (constant * (values[0].clone() * constant_lookup_scale - fixed_value)),
            ]
        });

        for (label, lane) in [
            ("Offline Cash packed typed lookup lane zero", 0_usize),
            ("Offline Cash packed typed lookup lane one", 4_usize),
        ] {
            meta.lookup_any(label, |meta| {
                let op = meta.query_fixed(opcode, Rotation::cur());
                // This public cubic is zero on padding, arithmetic, and
                // instance rows and non-zero on every typed opcode.  Scaling
                // the tuple and its fixed table value by the same factor keeps
                // exact membership without a high-degree Lagrange enable.
                let lookup_scale = op.clone()
                    * (op.clone() - Expression::Constant(F::from(OP_ARITHMETIC)))
                    * (op.clone() - Expression::Constant(F::from(OP_INSTANCE)));
                let tuple = (0..4).fold(Expression::Constant(F::ZERO), |sum, component| {
                    sum + meta.query_advice(advice[lane + component], Rotation::cur())
                        * Expression::Constant(F::from(
                            TUPLE_RADIX.pow(u32::try_from(component).expect("tuple width")),
                        ))
                });
                let table_tag = meta.query_fixed(table_tag, Rotation::cur());
                let table_value = meta.query_fixed(table_value, Rotation::cur());
                vec![(op, table_tag), (lookup_scale * tuple, table_value)]
            });
        }
        meta.set_minimum_degree(10);
        Self {
            advice,
            instances,
            opcode,
            table_tag,
            table_value,
        }
    }
}

fn lagrange_opcode_selector_v1<F: BigPrimeField>(op: Expression<F>, target: u64) -> Expression<F> {
    let mut numerator = Expression::Constant(F::ONE);
    let mut denominator = F::ONE;
    for point in OP_ARITHMETIC..=OP_INSTANCE {
        if point == target {
            continue;
        }
        numerator = numerator * (op.clone() - Expression::Constant(F::from(point)));
        denominator *= F::from(target) - F::from(point);
    }
    numerator
        * Expression::Constant(
            Option::<F>::from(denominator.invert()).expect("distinct opcode domain"),
        )
}

fn lookup_scale_v1<F: BigPrimeField>(opcode: u64) -> F {
    F::from(opcode)
        * (F::from(opcode) - F::from(OP_ARITHMETIC))
        * (F::from(opcode) - F::from(OP_INSTANCE))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PhysicalCellV1 {
    row: usize,
    column: usize,
}

#[derive(Clone, Debug)]
struct PackedRowV1<F> {
    advice: [Assigned<F>; ADVICE_COLUMNS],
    opcode: u64,
    fixed_constant: Option<F>,
}

impl<F: BigPrimeField> PackedRowV1<F> {
    fn arithmetic(first: [Assigned<F>; 4], second: Option<[Assigned<F>; 4]>) -> Self {
        let zero = Assigned::Trivial(F::ZERO);
        let second = second.unwrap_or([zero; 4]);
        Self {
            advice: [
                first[0], first[1], first[2], first[3], second[0], second[1], second[2], second[3],
            ],
            opcode: OP_ARITHMETIC,
            fixed_constant: None,
        }
    }

    fn lookup(opcode: u64, first: [Assigned<F>; 4], second: Option<[Assigned<F>; 4]>) -> Self {
        let zero = Assigned::Trivial(F::ZERO);
        let second = second.unwrap_or([zero; 4]);
        Self {
            advice: [
                first[0], first[1], first[2], first[3], second[0], second[1], second[2], second[3],
            ],
            opcode,
            fixed_constant: None,
        }
    }

    fn constant(value: F) -> Self {
        let zero = Assigned::Trivial(F::ZERO);
        Self {
            advice: [
                Assigned::Trivial(value),
                zero,
                zero,
                zero,
                zero,
                zero,
                zero,
                zero,
            ],
            opcode: OP_CONSTANT,
            fixed_constant: Some(value),
        }
    }

    fn free(value: Assigned<F>) -> Self {
        let zero = Assigned::Trivial(F::ZERO);
        Self {
            advice: [value, zero, zero, value, zero, zero, zero, zero],
            opcode: OP_ARITHMETIC,
            fixed_constant: None,
        }
    }

    fn instance(first: Assigned<F>, second: Assigned<F>) -> Self {
        let zero = Assigned::Trivial(F::ZERO);
        Self {
            advice: [first, zero, zero, first, second, zero, zero, second],
            opcode: OP_INSTANCE,
            fixed_constant: None,
        }
    }
}

type LogicalWireV1 = usize;
type NibbleWordV1 = [LogicalWireV1; 8];

#[derive(Clone, Debug)]
struct LogicalWireValueV1<F> {
    value: Assigned<F>,
}

#[derive(Clone, Copy, Debug)]
struct LogicalTupleV1 {
    opcode: u64,
    wires: [LogicalWireV1; 4],
}

#[derive(Clone, Debug)]
struct OfflineCashPackedShaTraceCompilerV1<F> {
    wires: Vec<LogicalWireValueV1<F>>,
    constants: BTreeMap<F, LogicalWireV1>,
    arithmetic: Vec<[LogicalWireV1; 4]>,
    tuples: Vec<LogicalTupleV1>,
    external_bindings: Vec<(LogicalWireV1, ContextCell)>,
}

impl<F: BigPrimeField> OfflineCashPackedShaTraceCompilerV1<F> {
    fn compile(jobs: &OfflineCashPackedSha256JobsV1<F>) -> Result<Self, String> {
        let mut compiler = Self {
            wires: Vec::new(),
            constants: BTreeMap::new(),
            arithmetic: Vec::new(),
            tuples: Vec::new(),
            external_bindings: Vec::new(),
        };
        for job in &jobs.jobs {
            compiler.compile_job(job)?;
        }
        Ok(compiler)
    }

    fn external_cells(&self) -> impl Iterator<Item = ContextCell> + '_ {
        self.external_bindings.iter().map(|(_, cell)| *cell)
    }

    fn new_wire(&mut self, value: u64) -> LogicalWireV1 {
        self.new_assigned_wire(Assigned::Trivial(F::from(value)))
    }

    fn new_assigned_wire(&mut self, value: Assigned<F>) -> LogicalWireV1 {
        let wire = self.wires.len();
        self.wires.push(LogicalWireValueV1 { value });
        wire
    }

    fn constant(&mut self, value: u64) -> LogicalWireV1 {
        let value = F::from(value);
        if let Some(wire) = self.constants.get(&value) {
            return *wire;
        }
        let wire = self.new_assigned_wire(Assigned::Trivial(value));
        self.constants.insert(value, wire);
        wire
    }

    fn bind_external(
        &mut self,
        wire: LogicalWireV1,
        value: AssignedValue<F>,
    ) -> Result<(), String> {
        let cell = value
            .cell
            .ok_or_else(|| "packed SHA-256 binding lost its virtual-cell identity".to_owned())?;
        self.external_bindings.push((wire, cell));
        Ok(())
    }

    fn source_byte(
        &mut self,
        binding: KagemushaSha256ByteBindingV1<F>,
    ) -> Result<LogicalWireV1, String> {
        match binding {
            KagemushaSha256ByteBindingV1::Constant(byte) => Ok(self.constant(u64::from(byte))),
            KagemushaSha256ByteBindingV1::Assigned(value) => {
                let byte = u8::try_from(fe_to_biguint(value.value()))
                    .map_err(|_| "packed SHA-256 source is not a byte".to_owned())?;
                let wire = self.new_wire(u64::from(byte));
                self.bind_external(wire, value)?;
                Ok(wire)
            }
        }
    }

    fn wire_u64(&self, wire: LogicalWireV1) -> Result<u64, String> {
        u64::try_from(fe_to_biguint(&self.wires[wire].value.evaluate())).map_err(|_| {
            "packed SHA-256 internal wire exceeded its authenticated tuple domain".to_owned()
        })
    }

    fn wire_u8(&self, wire: LogicalWireV1) -> Result<u8, String> {
        u8::try_from(self.wire_u64(wire)?).map_err(|_| {
            "packed SHA-256 internal wire exceeded its authenticated tuple domain".to_owned()
        })
    }

    fn push_tuple(&mut self, opcode: u64, wires: [LogicalWireV1; 4]) {
        self.tuples.push(LogicalTupleV1 { opcode, wires });
    }

    fn split_byte(&mut self, byte: LogicalWireV1) -> Result<[LogicalWireV1; 2], String> {
        let value = self.wire_u8(byte)?;
        let low = self.new_wire(u64::from(value & 0x0f));
        let high = self.new_wire(u64::from(value >> 4));
        let width = self.constant(2);
        self.push_tuple(OP_RANGE_OR_BYTE, [byte, low, high, width]);
        Ok([low, high])
    }

    fn constant_word(&mut self, value: u32) -> NibbleWordV1 {
        std::array::from_fn(|index| self.constant(u64::from((value >> (index * 4)) & 0x0f)))
    }

    fn xor_nibble(
        &mut self,
        left: LogicalWireV1,
        right: LogicalWireV1,
    ) -> Result<LogicalWireV1, String> {
        let output = self.new_wire(u64::from(self.wire_u8(left)? ^ self.wire_u8(right)?));
        let shift_zero = self.constant(0);
        self.push_tuple(OP_XOR_OR_ROTATE, [left, right, output, shift_zero]);
        Ok(output)
    }

    fn xor_word(
        &mut self,
        left: &NibbleWordV1,
        right: &NibbleWordV1,
    ) -> Result<NibbleWordV1, String> {
        (0..8)
            .map(|index| self.xor_nibble(left[index], right[index]))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "packed SHA-256 XOR width mismatch".to_owned())
    }

    fn xor_three_words(
        &mut self,
        first: &NibbleWordV1,
        second: &NibbleWordV1,
        third: &NibbleWordV1,
    ) -> Result<NibbleWordV1, String> {
        let partial = self.xor_word(first, second)?;
        self.xor_word(&partial, third)
    }

    fn rotate_or_shift_word(
        &mut self,
        input: &NibbleWordV1,
        distance: usize,
        rotate: bool,
    ) -> Result<NibbleWordV1, String> {
        let whole = distance / 4;
        let shift = distance % 4;
        let zero = self.constant(0);
        let mut output = [zero; 8];
        for (index, slot) in output.iter_mut().enumerate() {
            let low_index = index + whole;
            let low = if rotate {
                input[low_index % 8]
            } else if low_index < 8 {
                input[low_index]
            } else {
                zero
            };
            if shift == 0 {
                *slot = low;
                continue;
            }
            let high_index = low_index + 1;
            let high = if rotate {
                input[high_index % 8]
            } else if high_index < 8 {
                input[high_index]
            } else {
                zero
            };
            let low_value = self.wire_u8(low)?;
            let high_value = self.wire_u8(high)?;
            let value = (low_value >> shift) | ((high_value << (4 - shift)) & 0x0f);
            let result = self.new_wire(u64::from(value));
            let shift_wire = self.constant(shift as u64);
            self.push_tuple(OP_XOR_OR_ROTATE, [low, high, result, shift_wire]);
            *slot = result;
        }
        Ok(output)
    }

    fn choice_word(
        &mut self,
        choose: &NibbleWordV1,
        when_one: &NibbleWordV1,
        when_zero: &NibbleWordV1,
    ) -> Result<NibbleWordV1, String> {
        let mut output = [self.constant(0); 8];
        for index in 0..8 {
            let e = self.wire_u8(choose[index])?;
            let f = self.wire_u8(when_one[index])?;
            let g = self.wire_u8(when_zero[index])?;
            output[index] = self.new_wire(u64::from((e & f) ^ ((!e & 0x0f) & g)));
            self.push_tuple(
                OP_CHOICE,
                [
                    choose[index],
                    when_one[index],
                    when_zero[index],
                    output[index],
                ],
            );
        }
        Ok(output)
    }

    fn majority_word(
        &mut self,
        first: &NibbleWordV1,
        second: &NibbleWordV1,
        third: &NibbleWordV1,
    ) -> Result<NibbleWordV1, String> {
        let mut output = [self.constant(0); 8];
        for index in 0..8 {
            let a = self.wire_u8(first[index])?;
            let b = self.wire_u8(second[index])?;
            let c = self.wire_u8(third[index])?;
            output[index] = self.new_wire(u64::from((a & b) ^ (a & c) ^ (b & c)));
            self.push_tuple(
                OP_MAJORITY,
                [first[index], second[index], third[index], output[index]],
            );
        }
        Ok(output)
    }

    fn add_words(
        &mut self,
        left: &NibbleWordV1,
        right: &NibbleWordV1,
    ) -> Result<NibbleWordV1, String> {
        let zero = self.constant(0);
        let mut carry = zero;
        let mut output = [zero; 8];
        for index in 0..8 {
            let sum = u16::from(self.wire_u8(left[index])?)
                + u16::from(self.wire_u8(right[index])?)
                + u16::from(self.wire_u8(carry)?);
            let extended = self.new_wire(u64::from(sum));
            self.push_tuple(OP_ADD, [left[index], right[index], carry, extended]);
            output[index] = self.new_wire(u64::from(sum & 0x0f));
            let next_carry = self.new_wire(u64::from(sum >> 4));
            self.push_tuple(OP_SPLIT_ADD, [extended, output[index], next_carry, zero]);
            carry = next_carry;
        }
        Ok(output)
    }

    fn add_many(&mut self, words: &[NibbleWordV1]) -> Result<NibbleWordV1, String> {
        let (first, rest) = words
            .split_first()
            .ok_or_else(|| "packed SHA-256 addition has no operands".to_owned())?;
        rest.iter()
            .try_fold(*first, |sum, word| self.add_words(&sum, word))
    }

    fn pack_and_bind_word(
        &mut self,
        word: &NibbleWordV1,
        output: AssignedValue<F>,
    ) -> Result<(), String> {
        let radix = self.constant(16);
        let mut accumulator = word[7];
        for nibble in word[..7].iter().rev() {
            let value = u64::from(self.wire_u8(*nibble)?) + self.wire_u64(accumulator)? * 16;
            let next = self.new_wire(value);
            self.arithmetic.push([*nibble, accumulator, radix, next]);
            accumulator = next;
        }
        self.bind_external(accumulator, output)
    }

    fn compile_job(&mut self, job: &OfflineCashPackedSha256JobV1<F>) -> Result<(), String> {
        let mut bytes = job
            .message
            .iter()
            .copied()
            .map(|binding| self.source_byte(binding))
            .collect::<Result<Vec<_>, _>>()?;
        let bit_len = u64::try_from(bytes.len())
            .ok()
            .and_then(|len| len.checked_mul(8))
            .ok_or_else(|| "packed SHA-256 message length overflow".to_owned())?;
        bytes.push(self.constant(0x80));
        while bytes.len() % 64 != 56 {
            bytes.push(self.constant(0));
        }
        bytes.extend(
            bit_len
                .to_be_bytes()
                .into_iter()
                .map(|byte| self.constant(u64::from(byte))),
        );

        let mut state = SHA256_IV.map(|word| self.constant_word(word));
        for block in bytes.chunks_exact(64) {
            let byte_nibbles = block
                .iter()
                .copied()
                .map(|byte| self.split_byte(byte))
                .collect::<Result<Vec<_>, _>>()?;
            let mut schedule = Vec::<NibbleWordV1>::with_capacity(64);
            for word in byte_nibbles.chunks_exact(4) {
                schedule.push([
                    word[3][0], word[3][1], word[2][0], word[2][1], word[1][0], word[1][1],
                    word[0][0], word[0][1],
                ]);
            }
            for index in 16..64 {
                let s0_rot7 = self.rotate_or_shift_word(&schedule[index - 15], 7, true)?;
                let s0_rot18 = self.rotate_or_shift_word(&schedule[index - 15], 18, true)?;
                let s0_shift3 = self.rotate_or_shift_word(&schedule[index - 15], 3, false)?;
                let s0 = self.xor_three_words(&s0_rot7, &s0_rot18, &s0_shift3)?;
                let s1_rot17 = self.rotate_or_shift_word(&schedule[index - 2], 17, true)?;
                let s1_rot19 = self.rotate_or_shift_word(&schedule[index - 2], 19, true)?;
                let s1_shift10 = self.rotate_or_shift_word(&schedule[index - 2], 10, false)?;
                let s1 = self.xor_three_words(&s1_rot17, &s1_rot19, &s1_shift10)?;
                schedule.push(self.add_many(&[
                    schedule[index - 16],
                    s0,
                    schedule[index - 7],
                    s1,
                ])?);
            }

            let initial = state;
            let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
            for index in 0..64 {
                let s1_rot6 = self.rotate_or_shift_word(&e, 6, true)?;
                let s1_rot11 = self.rotate_or_shift_word(&e, 11, true)?;
                let s1_rot25 = self.rotate_or_shift_word(&e, 25, true)?;
                let upper_sigma_one = self.xor_three_words(&s1_rot6, &s1_rot11, &s1_rot25)?;
                let choice = self.choice_word(&e, &f, &g)?;
                let round_constant = self.constant_word(SHA256_K[index]);
                let temp_one =
                    self.add_many(&[h, upper_sigma_one, choice, round_constant, schedule[index]])?;
                let s0_rot2 = self.rotate_or_shift_word(&a, 2, true)?;
                let s0_rot13 = self.rotate_or_shift_word(&a, 13, true)?;
                let s0_rot22 = self.rotate_or_shift_word(&a, 22, true)?;
                let upper_sigma_zero = self.xor_three_words(&s0_rot2, &s0_rot13, &s0_rot22)?;
                let majority = self.majority_word(&a, &b, &c)?;
                let temp_two = self.add_words(&upper_sigma_zero, &majority)?;
                h = g;
                g = f;
                f = e;
                e = self.add_words(&d, &temp_one)?;
                d = c;
                c = b;
                b = a;
                a = self.add_words(&temp_one, &temp_two)?;
            }
            state = [a, b, c, d, e, f, g, h]
                .into_iter()
                .zip(initial)
                .map(|(word, initial)| self.add_words(&word, &initial))
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| "packed SHA-256 state width mismatch".to_owned())?;
        }
        for (word, output) in state.iter().zip(job.output_words) {
            self.pack_and_bind_word(word, output)?;
        }
        Ok(())
    }

    fn append_rows(
        self,
        rows: &mut Vec<PackedRowV1<F>>,
        first: &HashMap<ContextCell, PhysicalCellV1>,
        equalities: &mut Vec<(PhysicalCellV1, PhysicalCellV1)>,
    ) -> Result<(), String> {
        let mut wire_first = vec![None::<PhysicalCellV1>; self.wires.len()];
        let mut remember_wire = |wire: LogicalWireV1, physical: PhysicalCellV1| {
            if let Some(previous) = wire_first[wire].replace(physical) {
                equalities.push((previous, physical));
            }
        };

        for (value, wire) in &self.constants {
            let row = rows.len();
            rows.push(PackedRowV1::constant(*value));
            remember_wire(*wire, PhysicalCellV1 { row, column: 0 });
        }
        for gates in self.arithmetic.chunks(2) {
            let row = rows.len();
            let assigned = |gate: &[LogicalWireV1; 4]| gate.map(|wire| self.wires[wire].value);
            rows.push(PackedRowV1::arithmetic(
                assigned(&gates[0]),
                gates.get(1).map(assigned),
            ));
            for (lane, gate) in gates.iter().enumerate() {
                for (column, wire) in gate.iter().copied().enumerate() {
                    remember_wire(
                        wire,
                        PhysicalCellV1 {
                            row,
                            column: lane * 4 + column,
                        },
                    );
                }
            }
        }
        let mut by_opcode = BTreeMap::<u64, Vec<LogicalTupleV1>>::new();
        for tuple in self.tuples {
            by_opcode.entry(tuple.opcode).or_default().push(tuple);
        }
        for (opcode, tuples) in by_opcode {
            for pair in tuples.chunks(2) {
                let row = rows.len();
                let assigned =
                    |tuple: &LogicalTupleV1| tuple.wires.map(|wire| self.wires[wire].value);
                rows.push(PackedRowV1::lookup(
                    opcode,
                    assigned(&pair[0]),
                    pair.get(1).map(assigned),
                ));
                for (lane, tuple) in pair.iter().enumerate() {
                    for (column, wire) in tuple.wires.iter().copied().enumerate() {
                        remember_wire(
                            wire,
                            PhysicalCellV1 {
                                row,
                                column: lane * 4 + column,
                            },
                        );
                    }
                }
            }
        }
        drop(remember_wire);
        for (wire, external) in self.external_bindings {
            equalities.push((
                wire_first[wire].ok_or_else(|| {
                    "packed SHA-256 logical wire was never materialized".to_owned()
                })?,
                *first.get(&external).ok_or_else(|| {
                    "packed SHA-256 external Base cell was never materialized".to_owned()
                })?,
            ));
        }
        Ok(())
    }
}

const SHA256_IV: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

#[rustfmt::skip]
const SHA256_K: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5,
    0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
    0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
    0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
    0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85,
    0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
    0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
    0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
];

fn packed_tuple_value_v1<F: BigPrimeField>(components: [u64; 4]) -> F {
    components
        .into_iter()
        .enumerate()
        .fold(F::ZERO, |sum, (index, component)| {
            sum + F::from(component)
                * F::from(TUPLE_RADIX.pow(u32::try_from(index).expect("tuple width")))
        })
}

fn extend_sha256_typed_table_v1<F: BigPrimeField>(table: &mut BTreeSet<(F, F)>) {
    for byte in 0_u64..=u64::from(u8::MAX) {
        table.insert((
            F::from(OP_RANGE_OR_BYTE),
            packed_tuple_value_v1([byte, byte & 0x0f, byte >> 4, 2]),
        ));
    }
    for left in 0_u64..16 {
        for right in 0_u64..16 {
            table.insert((
                F::from(OP_XOR_OR_ROTATE),
                packed_tuple_value_v1([left, right, left ^ right, 0]),
            ));
            for shift in 1_u64..=3 {
                table.insert((
                    F::from(OP_XOR_OR_ROTATE),
                    packed_tuple_value_v1([
                        left,
                        right,
                        (left >> shift) | ((right << (4 - shift)) & 0x0f),
                        shift,
                    ]),
                ));
            }
            for carry in 0_u64..=1 {
                table.insert((
                    F::from(OP_ADD),
                    packed_tuple_value_v1([left, right, carry, left + right + carry]),
                ));
            }
            for third in 0_u64..16 {
                table.insert((
                    F::from(OP_CHOICE),
                    packed_tuple_value_v1([
                        left,
                        right,
                        third,
                        (left & right) ^ ((!left & 0x0f) & third),
                    ]),
                ));
                table.insert((
                    F::from(OP_MAJORITY),
                    packed_tuple_value_v1([
                        left,
                        right,
                        third,
                        (left & right) ^ (left & third) ^ (right & third),
                    ]),
                ));
            }
        }
    }
    for extended in 0_u64..=31 {
        table.insert((
            F::from(OP_SPLIT_ADD),
            packed_tuple_value_v1([extended, extended & 0x0f, extended >> 4, 0]),
        ));
    }
}

/// Fully materialized compact trace. Raw proof witnesses are owned by the
/// circuit and replaced with unknown values by `without_witnesses`.
#[derive(Clone, Debug)]
pub(super) struct OfflineCashPackedBaseTraceV1<F, const INSTANCE_COLUMNS: usize = 2> {
    rows: Vec<PackedRowV1<F>>,
    equalities: Vec<(PhysicalCellV1, PhysicalCellV1)>,
    table: Vec<(F, F)>,
    extra_instance_bindings: Vec<(PhysicalCellV1, usize, usize)>,
    sha_jobs: usize,
    sha_blocks: usize,
    unknown: bool,
}

impl<F: BigPrimeField, const INSTANCE_COLUMNS: usize>
    OfflineCashPackedBaseTraceV1<F, INSTANCE_COLUMNS>
{
    pub(super) fn from_builder(
        builder: &BaseCircuitBuilder<F>,
        sha_jobs: &OfflineCashPackedSha256JobsV1<F>,
    ) -> Result<Self, String> {
        if builder.witness_gen_only() {
            return Err(
                "packed Base compiler requires the constraint-bearing builder stage".to_owned(),
            );
        }
        if builder.assigned_instances.len() != INSTANCE_COLUMNS {
            return Err("packed Base compiler requires exactly two instance columns".to_owned());
        }
        if builder
            .core()
            .phase_manager
            .iter()
            .skip(1)
            .any(|phase| phase.total_advice() != 0)
        {
            return Err("packed Base compiler does not admit later challenge phases".to_owned());
        }
        let sha_trace = OfflineCashPackedShaTraceCompilerV1::compile(sha_jobs)?;
        let sha_job_count = sha_jobs.jobs.len();
        let sha_block_count = sha_jobs.block_count_v1();

        // Instance rows are reserved first.  Their opcode binds column zero of
        // each four-cell lane directly to the corresponding public column;
        // missing tail cells are canonical zero padding.
        let public_rows = builder
            .assigned_instances
            .iter()
            .take(DIRECT_INSTANCE_COLUMNS)
            .map(Vec::len)
            .max()
            .unwrap_or(0);
        let mut rows = (0..public_rows)
            .map(|row| {
                PackedRowV1::instance(
                    builder.assigned_instances[0]
                        .get(row)
                        .map_or(Assigned::Trivial(F::ZERO), |value| value.value),
                    builder.assigned_instances[1]
                        .get(row)
                        .map_or(Assigned::Trivial(F::ZERO), |value| value.value),
                )
            })
            .collect::<Vec<_>>();
        let mut first = HashMap::<ContextCell, PhysicalCellV1>::new();
        let mut equalities = Vec::<(PhysicalCellV1, PhysicalCellV1)>::new();
        let mut extra_instance_bindings = Vec::<(PhysicalCellV1, usize, usize)>::new();
        let remember =
            |cell: ContextCell,
             physical: PhysicalCellV1,
             first: &mut HashMap<ContextCell, PhysicalCellV1>,
             equalities: &mut Vec<(PhysicalCellV1, PhysicalCellV1)>| {
                if let Some(previous) = first.insert(cell, physical) {
                    equalities.push((previous, physical));
                }
            };
        for (column, instances) in builder
            .assigned_instances
            .iter()
            .take(DIRECT_INSTANCE_COLUMNS)
            .enumerate()
        {
            for (row, value) in instances.iter().enumerate() {
                let cell = value.cell.ok_or_else(|| {
                    "packed Base public instance has no virtual identity".to_owned()
                })?;
                remember(
                    cell,
                    PhysicalCellV1 {
                        row,
                        column: column * 4,
                    },
                    &mut first,
                    &mut equalities,
                );
            }
        }
        for (column, instances) in builder
            .assigned_instances
            .iter()
            .enumerate()
            .skip(DIRECT_INSTANCE_COLUMNS)
        {
            for (instance_row, value) in instances.iter().enumerate() {
                let row = rows.len();
                rows.push(PackedRowV1::free(value.value));
                let physical = PhysicalCellV1 { row, column: 0 };
                let cell = value.cell.ok_or_else(|| {
                    "packed Base extra public instance has no virtual identity".to_owned()
                })?;
                remember(cell, physical, &mut first, &mut equalities);
                extra_instance_bindings.push((physical, column, instance_row));
            }
        }

        let phase_zero = builder
            .core()
            .phase_manager
            .first()
            .ok_or_else(|| "packed Base compiler is missing phase zero".to_owned())?;
        let mut virtual_gates = Vec::<([Assigned<F>; 4], [ContextCell; 4])>::new();
        for context in &phase_zero.threads {
            if context.selector.len() != context.advice_len() {
                return Err("packed Base virtual selector/advice lengths differ".to_owned());
            }
            for (offset, enabled) in context.selector.iter().copied().enumerate() {
                if !enabled {
                    continue;
                }
                if offset + 4 > context.advice_len() {
                    return Err("packed Base virtual gate is truncated".to_owned());
                }
                virtual_gates.push((
                    std::array::from_fn(|column| context.get((offset + column) as isize).value),
                    std::array::from_fn(|column| {
                        ContextCell::new(context.type_id(), context.id(), offset + column)
                    }),
                ));
            }
        }
        for gates in virtual_gates.chunks(2) {
            let row = rows.len();
            rows.push(PackedRowV1::arithmetic(
                gates[0].0,
                gates.get(1).map(|gate| gate.0),
            ));
            for (lane, (_, cells)) in gates.iter().enumerate() {
                for (column, cell) in cells.iter().copied().enumerate() {
                    remember(
                        cell,
                        PhysicalCellV1 {
                            row,
                            column: lane * 4 + column,
                        },
                        &mut first,
                        &mut equalities,
                    );
                }
            }
        }

        let lookup_cells = builder.lookup_manager()[0]
            .cells_to_lookup
            .lock()
            .map_err(|_| "packed Base range lookup lock is poisoned".to_owned())?
            .values()
            .flatten()
            .map(|entry| entry[0])
            .collect::<Vec<_>>();
        for chunk in lookup_cells.chunks(2) {
            let row = rows.len();
            let tuple = |value: Assigned<F>| {
                [
                    value,
                    Assigned::Trivial(F::ZERO),
                    Assigned::Trivial(F::ZERO),
                    Assigned::Trivial(F::ZERO),
                ]
            };
            rows.push(PackedRowV1::lookup(
                OP_RANGE_OR_BYTE,
                tuple(chunk[0].value),
                chunk.get(1).map(|value| tuple(value.value)),
            ));
            for (lane, value) in chunk.iter().enumerate() {
                let cell = value
                    .cell
                    .ok_or_else(|| "packed Base range cell has no virtual identity".to_owned())?;
                remember(
                    cell,
                    PhysicalCellV1 {
                        row,
                        column: lane * 4,
                    },
                    &mut first,
                    &mut equalities,
                );
            }
        }

        let copy = builder
            .core()
            .copy_manager
            .lock()
            .map_err(|_| "packed Base copy-constraint lock is poisoned".to_owned())?;
        let mut needed = BTreeSet::<ContextCell>::new();
        for (left, right) in &copy.advice_equalities {
            needed.insert(*left);
            needed.insert(*right);
        }
        for (_, cell) in copy.constant_equalities.iter() {
            needed.insert(*cell);
        }
        needed.extend(sha_trace.external_cells());
        for cell in needed {
            if first.contains_key(&cell) {
                continue;
            }
            let context = phase_zero
                .threads
                .iter()
                .find(|context| {
                    context.type_id() == cell.type_id() && context.id() == cell.context_id()
                })
                .ok_or_else(|| "packed Base equality references an unknown context".to_owned())?;
            if cell.offset() >= context.advice_len() {
                return Err("packed Base equality references an unknown cell".to_owned());
            }
            let value = context.get(cell.offset() as isize).value;
            let row = rows.len();
            rows.push(PackedRowV1::free(value));
            remember(
                cell,
                PhysicalCellV1 { row, column: 0 },
                &mut first,
                &mut equalities,
            );
        }
        for (left, right) in &copy.advice_equalities {
            equalities.push((
                *first
                    .get(left)
                    .ok_or_else(|| "packed Base left equality cell is unmapped".to_owned())?,
                *first
                    .get(right)
                    .ok_or_else(|| "packed Base right equality cell is unmapped".to_owned())?,
            ));
        }

        let mut constant_cells = BTreeMap::<F, PhysicalCellV1>::new();
        for (constant, _) in copy.constant_equalities.iter() {
            if constant_cells.contains_key(constant) {
                continue;
            }
            let row = rows.len();
            rows.push(PackedRowV1::constant(*constant));
            constant_cells.insert(*constant, PhysicalCellV1 { row, column: 0 });
        }
        for (constant, cell) in copy.constant_equalities.iter() {
            equalities.push((
                constant_cells[constant],
                *first
                    .get(cell)
                    .ok_or_else(|| "packed Base constant cell is unmapped".to_owned())?,
            ));
        }
        drop(copy);

        sha_trace.append_rows(&mut rows, &first, &mut equalities)?;

        if rows.len() > USABLE_ROWS {
            return Err(format!(
                "packed Base trace requires {} rows, exceeding {USABLE_ROWS}",
                rows.len()
            ));
        }

        let mut required_table = (0_u64..(1_u64 << LOOKUP_BITS))
            .map(|value| (F::from(OP_RANGE_OR_BYTE), F::from(value)))
            .collect::<BTreeSet<_>>();
        for opcode in LOOKUP_OPCODES {
            required_table.insert((F::from(opcode), F::ZERO));
        }
        required_table.insert((F::from(OP_ARITHMETIC), F::ZERO));
        required_table.insert((F::from(OP_INSTANCE), F::ZERO));
        extend_sha256_typed_table_v1(&mut required_table);
        required_table.extend(
            constant_cells
                .keys()
                .copied()
                .map(|constant| (F::from(OP_CONSTANT), constant)),
        );
        // Disabled arithmetic and instance rows query their own typed tag with
        // a scaled zero value. Opcode-zero padding is supplied canonically by
        // Halo2's unassigned fixed rows and never needs a witness row.
        for row in &rows {
            if let Some(value) = row.fixed_constant {
                required_table.remove(&(F::from(OP_CONSTANT), value));
            }
        }

        let constant_rows = rows
            .iter()
            .filter(|row| row.fixed_constant.is_some())
            .count();
        let table_len = rows
            .len()
            .max(required_table.len().saturating_add(constant_rows));
        let mut table = vec![(F::ZERO, F::ZERO); table_len];
        for (offset, row) in rows.iter().enumerate() {
            if let Some(value) = row.fixed_constant {
                table[offset] = (
                    F::from(OP_CONSTANT),
                    value * lookup_scale_v1::<F>(OP_CONSTANT),
                );
            }
        }
        let mut remaining = required_table.into_iter();
        for (offset, row) in rows
            .iter()
            .map(Some)
            .chain(core::iter::repeat(None))
            .take(table.len())
            .enumerate()
        {
            if row.is_some_and(|row| row.fixed_constant.is_some()) {
                continue;
            }
            if let Some((tag, value)) = remaining.next() {
                let opcode = (OP_ARITHMETIC..=OP_INSTANCE)
                    .find(|opcode| tag == F::from(*opcode))
                    .ok_or_else(|| "packed Base typed table has an unknown opcode".to_owned())?;
                table[offset] = (tag, value * lookup_scale_v1::<F>(opcode));
            } else {
                break;
            }
        }
        if remaining.next().is_some() {
            return Err("packed Base typed table placement overflow".to_owned());
        }
        if table.len() > USABLE_ROWS {
            return Err("packed Base typed table exceeds the k=16 row cap".to_owned());
        }
        Ok(Self {
            rows,
            equalities,
            table,
            extra_instance_bindings,
            sha_jobs: sha_job_count,
            sha_blocks: sha_block_count,
            unknown: false,
        })
    }

    pub(super) fn without_witnesses(&self) -> Self {
        let mut clone = self.clone();
        clone.unknown = true;
        clone
    }

    pub(super) fn assigned_rows(&self) -> usize {
        self.rows.len()
    }

    pub(super) const fn sha_inventory(&self) -> (usize, usize) {
        (self.sha_jobs, self.sha_blocks)
    }

    pub(super) fn synthesize(
        &self,
        config: &OfflineCashPackedBaseConfigV1<INSTANCE_COLUMNS>,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), Error> {
        let cells = layouter.assign_region(
            || "Offline Cash packed Base trace and typed table",
            |mut region| {
                let mut cells = Vec::<[Cell; ADVICE_COLUMNS]>::with_capacity(self.rows.len());
                for (offset, row) in self.rows.iter().enumerate() {
                    let assigned = std::array::from_fn(|column| {
                        raw_assign_advice(
                            &mut region,
                            config.advice[column],
                            offset,
                            if self.unknown {
                                Value::unknown()
                            } else {
                                Value::known(row.advice[column])
                            },
                        )
                        .cell()
                    });
                    raw_assign_fixed(&mut region, config.opcode, offset, F::from(row.opcode));
                    cells.push(assigned);
                }
                for (left, right) in &self.equalities {
                    raw_constrain_equal(
                        &mut region,
                        cells[left.row][left.column],
                        cells[right.row][right.column],
                    );
                }
                for (offset, (tag, value)) in self.table.iter().copied().enumerate() {
                    raw_assign_fixed(&mut region, config.table_tag, offset, tag);
                    raw_assign_fixed(&mut region, config.table_value, offset, value);
                }
                Ok(cells)
            },
        )?;
        for (physical, instance_column, instance_row) in &self.extra_instance_bindings {
            layouter.constrain_instance(
                cells[physical.row][physical.column],
                config.instances[*instance_column],
                *instance_row,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::pasta_ipa_recursion::{
        PastaIpaInstanceQueryV1, pasta_ipa_augmented_proof_shape_v1,
    };
    use ff::Field as _;
    use halo2_base::utils::ScalarField;
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::Circuit,
    };

    #[derive(Clone, Debug)]
    struct PackedShaTestCircuitV1<F: BigPrimeField> {
        trace: OfflineCashPackedBaseTraceV1<F>,
    }

    impl<F: BigPrimeField> Circuit<F> for PackedShaTestCircuitV1<F> {
        type Config = OfflineCashPackedBaseConfigV1<2>;
        type FloorPlanner = V1;
        type Params = ();

        fn params(&self) -> Self::Params {}

        fn without_witnesses(&self) -> Self {
            Self {
                trace: self.trace.without_witnesses(),
            }
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            OfflineCashPackedBaseConfigV1::<2>::configure(meta)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            self.trace.synthesize(&config, &mut layouter)
        }
    }

    fn packed_sha_test_v1<F: BigPrimeField + ScalarField>(
        message: &[u8],
        expected_words: [u32; SHA256_DIGEST_WORDS],
    ) -> (PackedShaTestCircuitV1<F>, Vec<Vec<F>>) {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(OFFLINE_CASH_HALO2_K_V1 as usize)
            .use_lookup_bits(LOOKUP_BITS);
        let mut jobs = OfflineCashPackedSha256JobsV1::default();
        let message = message
            .iter()
            .copied()
            .map(KagemushaSha256ByteV4::constant)
            .collect::<Vec<_>>();
        let digest = jobs
            .digest_constrained_v1(builder.main(0), &message)
            .expect("collect packed SHA job");
        let zero = builder.main(0).load_witness(F::ZERO);
        builder.assigned_instances = vec![digest.to_vec(), vec![zero]];
        let trace = OfflineCashPackedBaseTraceV1::from_builder(&builder, &jobs)
            .expect("compile packed SHA trace");
        let instances = vec![
            expected_words
                .into_iter()
                .map(|word| F::from(u64::from(word)))
                .collect(),
            vec![F::ZERO],
        ];
        (PackedShaTestCircuitV1 { trace }, instances)
    }

    fn assert_exact_shape<F: BigPrimeField>() {
        let mut meta = ConstraintSystem::<F>::default();
        let _ = OfflineCashPackedBaseConfigV1::<2>::configure(&mut meta);
        assert_eq!(meta.degree(), 10);
        assert_eq!(meta.num_advice_columns(), 8);
        assert_eq!(meta.advice_queries().len(), 8);
        assert_eq!(meta.num_instance_columns(), 2);
        assert_eq!(meta.instance_queries().len(), 2);
        assert_eq!(meta.num_fixed_columns(), 3);
        assert_eq!(meta.fixed_queries().len(), 3);
        assert_eq!(meta.num_selectors(), 0);
        assert_eq!(meta.permutation().get_columns().len(), 8);
        assert_eq!(meta.lookups().len(), 2);
        assert!(
            meta.advice_queries()
                .iter()
                .all(|(_, rotation)| *rotation == Rotation::cur())
        );
        let shape = pasta_ipa_augmented_proof_shape_v1(
            &meta,
            OFFLINE_CASH_HALO2_K_V1,
            PastaIpaInstanceQueryV1::Direct,
        )
        .expect("packed final-State shape");
        assert_eq!(shape.commitments(), 59);
        assert_eq!(shape.evaluations(), 37);
        assert_eq!(shape.ordinary_proof_bytes(), 3_072);
    }

    #[test]
    fn exact_eq_and_ep_shape_is_3072_ordinary_bytes() {
        assert_exact_shape::<Fp>();
        assert_exact_shape::<Fq>();
    }

    #[test]
    fn packed_sha256_matches_standard_empty_and_abc_vectors_and_binds_output() {
        let vectors = [
            (
                b"".as_slice(),
                [
                    0xe3b0_c442,
                    0x98fc_1c14,
                    0x9afb_f4c8,
                    0x996f_b924,
                    0x27ae_41e4,
                    0x649b_934c,
                    0xa495_991b,
                    0x7852_b855,
                ],
            ),
            (
                b"abc".as_slice(),
                [
                    0xba78_16bf,
                    0x8f01_cfea,
                    0x4141_40de,
                    0x5dae_2223,
                    0xb003_61a3,
                    0x9617_7a9c,
                    0xb410_ff61,
                    0xf200_15ad,
                ],
            ),
        ];
        for (message, expected) in vectors {
            let (circuit, instances) = packed_sha_test_v1::<Fp>(message, expected);
            eprintln!(
                "packed SHA-256 message_bytes={} assigned_rows={} sha_inventory={:?}",
                message.len(),
                circuit.trace.assigned_rows(),
                circuit.trace.sha_inventory(),
            );
            MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, instances.clone())
                .expect("packed SHA standard vector prover")
                .assert_satisfied();

            let mut tampered = instances;
            tampered[0][3] += Fp::ONE;
            assert!(
                MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, tampered)
                    .expect("packed SHA output-tamper prover")
                    .verify()
                    .is_err()
            );
        }
    }

    #[test]
    fn packed_sha_typed_table_rejects_each_input_output_mode_and_carry_mutation() {
        let mut table = BTreeSet::<(Fp, Fp)>::new();
        extend_sha256_typed_table_v1(&mut table);
        let cases = [
            (OP_RANGE_OR_BYTE, [0xab, 0x0b, 0x0a, 2]),
            (OP_XOR_OR_ROTATE, [3, 5, 6, 0]),
            (OP_XOR_OR_ROTATE, [3, 5, 9, 1]),
            (OP_CHOICE, [1, 0, 1, 0]),
            (OP_MAJORITY, [0, 1, 2, 0]),
            (OP_ADD, [3, 5, 1, 9]),
            (OP_SPLIT_ADD, [27, 11, 1, 0]),
        ];
        for (opcode, tuple) in cases {
            assert!(table.contains(&(Fp::from(opcode), packed_tuple_value_v1::<Fp>(tuple))));
            for component in 0..4 {
                let mut mutated = tuple;
                mutated[component] += 1;
                assert!(
                    !table.contains(&(Fp::from(opcode), packed_tuple_value_v1::<Fp>(mutated))),
                    "opcode {opcode} component {component} mutation unexpectedly remained valid"
                );
            }
        }
    }
}
