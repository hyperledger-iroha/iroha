//! Private, non-authorizing compact affine P-256 ECDSA prototype.
//!
//! This private module is declared only for source-settled prototype tests. It
//! is a bounded prerequisite, not a production verifier. It implements the exact
//! three-by-86-bit non-native arithmetic relation, complete affine exceptional
//! cases, and the fixed-topology four-bit Straus schedule.  Eligibility is
//! fail-closed: both the assigned-row cap and the configured 3,200-byte proof
//! shape must hold before this circuit can be considered for a recursive child.
//! SHA-256, ASN.1/DER, KeyMint parsing, GuardBundle recursion, and backend
//! registration remain outside this file. Every constant instance tail,
//! including Boolean seeds, must be verifier-derived; caller-supplied constant
//! tails cannot authorize a helper proof.

use std::{collections::HashMap, marker::PhantomData};

use der_parser::num_bigint::{BigInt, BigUint, Sign};
use halo2_base::{
    halo2_proofs::{
        circuit::{Cell, Layouter, SimpleFloorPlanner, Value},
        plonk::{
            Advice, Assigned, Circuit, Column, ConstraintSystem, Error, Expression, Fixed, Instance,
        },
        poly::Rotation,
    },
    utils::{
        BigPrimeField, biguint_to_fe,
        halo2::{raw_assign_advice, raw_assign_fixed, raw_constrain_equal},
    },
};

const K: u32 = 16;
const ADVICE_COLUMNS: usize = 8;
const PUBLIC_BYTES: usize = 65 + 32 + 64;
const LOOKUP_BITS: usize = 15;
const RANGE_CHUNK_BITS: [usize; 6] = [2, 4, 6, 8, 11, 15];
const TABLE_ROWS: usize = (1 << 2) + (1 << 4) + (1 << 6) + (1 << 8) + (1 << 11) + (1 << 15);
const LIMB_BITS: usize = 86;
const LIMBS: usize = 3;
const QUOTIENT_BITS: usize = 3 * LIMB_BITS;
const CARRY_BITS: usize = 90;
const WINDOW_BITS: usize = 4;
const WINDOWS: usize = 256 / WINDOW_BITS;
const K16_MAX_ASSIGNED_ROWS: usize = (1 << K) - 9;
#[cfg(test)]
const P256_AFFINE_COMPACT_CALLER_INSTANCES_V1: usize = PUBLIC_BYTES;
#[cfg(test)]
const P256_AFFINE_COMPACT_CONSTANT_TAIL_INSTANCES_V1: usize = 193;
#[cfg(test)]
const P256_AFFINE_COMPACT_TOTAL_INSTANCES_V1: usize = 354;
// Commits, in insertion order, to every exact verifier-derived tail value as
// its 32-byte little-endian field representation after the domain and length.
#[cfg(test)]
const P256_AFFINE_COMPACT_CONSTANT_TAIL_DIGEST_V1: [u8; 32] = [
    0xc9, 0x70, 0xd2, 0x44, 0xe0, 0xaa, 0x9a, 0xda, 0xab, 0x2e, 0xd0, 0x9e, 0x2e, 0x2e, 0x4c, 0xe0,
    0xc3, 0xed, 0xd3, 0xf4, 0xb8, 0x72, 0xb4, 0xe7, 0x13, 0xd2, 0x16, 0xe9, 0x19, 0xd7, 0x64, 0xa6,
];

const P256_BASE_MODULUS_BE: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];
const P256_SCALAR_MODULUS_BE: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
];
const P256_B_BE: [u8; 32] = [
    0x5a, 0xc6, 0x35, 0xd8, 0xaa, 0x3a, 0x93, 0xe7, 0xb3, 0xeb, 0xbd, 0x55, 0x76, 0x98, 0x86, 0xbc,
    0x65, 0x1d, 0x06, 0xb0, 0xcc, 0x53, 0xb0, 0xf6, 0x3b, 0xce, 0x3c, 0x3e, 0x27, 0xd2, 0x60, 0x4b,
];
const P256_GENERATOR_X_BE: [u8; 32] = [
    0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40, 0xf2,
    0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
];
const P256_GENERATOR_Y_BE: [u8; 32] = [
    0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16,
    0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
];

/// Exact configured constraint-system and augmented proof shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct P256AffineCompactShapeV1 {
    pub(super) degree: usize,
    pub(super) advice_columns: usize,
    pub(super) advice_queries: usize,
    pub(super) instance_columns: usize,
    pub(super) instance_queries: usize,
    pub(super) fixed_columns: usize,
    pub(super) fixed_queries: usize,
    pub(super) selectors: usize,
    pub(super) equality_columns: usize,
    pub(super) permutation_chunks: usize,
    pub(super) lookup_arguments: usize,
    pub(super) quotient_pieces: usize,
    pub(super) opening_point_sets: usize,
    pub(super) proof_points: usize,
    pub(super) proof_scalars: usize,
    pub(super) raw_proof_bytes: usize,
    pub(super) augmented_proof_bytes: usize,
}

pub(super) const P256_AFFINE_COMPACT_SHAPE_V1: P256AffineCompactShapeV1 =
    P256AffineCompactShapeV1 {
        degree: 7,
        advice_columns: 8,
        advice_queries: 8,
        instance_columns: 1,
        instance_queries: 1,
        fixed_columns: 4,
        fixed_queries: 4,
        selectors: 0,
        equality_columns: 8,
        permutation_chunks: 2,
        lookup_arguments: 2,
        quotient_pieces: 6,
        opening_point_sets: 4,
        proof_points: 57,
        proof_scalars: 42,
        raw_proof_bytes: 3_168,
        augmented_proof_bytes: 3_200,
    };

/// Exact pre-cap physical-row inventory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct P256AffineCompactRowsV1 {
    pub(super) binding_rows: usize,
    pub(super) range_rows: usize,
    pub(super) arithmetic_rows: usize,
    pub(super) selection_rows: usize,
    pub(super) fixed_selection_rows: usize,
    pub(super) total_rows: usize,
    pub(super) range_lookups: usize,
    pub(super) fma_relations: usize,
    pub(super) select_relations: usize,
    pub(super) modular_relations: usize,
    pub(super) complete_doublings: usize,
    pub(super) complete_additions: usize,
    pub(super) maximum_quotient_bits: usize,
    pub(super) maximum_carry_bits: usize,
}

#[cfg(test)]
const P256_AFFINE_COMPACT_RFC6979_ROWS_V1: P256AffineCompactRowsV1 = P256AffineCompactRowsV1 {
    binding_rows: 354,
    range_rows: 52_262,
    arithmetic_rows: 178_648,
    selection_rows: 6_975,
    fixed_selection_rows: 0,
    total_rows: 238_239,
    range_lookups: 104_521,
    fma_relations: 357_296,
    select_relations: 13_950,
    modular_relations: 1_334,
    complete_doublings: 263,
    complete_additions: 135,
    maximum_quotient_bits: 258,
    maximum_carry_bits: 89,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum P256AffineCompactFailureV1 {
    IntegerBound {
        witness: &'static str,
        actual_bits: usize,
        maximum_bits: usize,
    },
    RowCapacityExceeded {
        rows: Box<P256AffineCompactRowsV1>,
        maximum: usize,
    },
    InstanceBindingMismatch {
        instances: usize,
        binding_rows: usize,
    },
    MissingCell {
        variable: usize,
    },
}

/// Exact public byte ordering is `[SEC1; SHA-256 digest; P1363 r || s]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct P256AffineCompactEcdsaCircuitV1<F> {
    sec1_uncompressed: [u8; 65],
    digest: [u8; 32],
    signature: [u8; 64],
    _field: PhantomData<F>,
}

impl<F> Default for P256AffineCompactEcdsaCircuitV1<F> {
    fn default() -> Self {
        Self {
            sec1_uncompressed: [0; 65],
            digest: [0; 32],
            signature: [0; 64],
            _field: PhantomData,
        }
    }
}

impl<F: BigPrimeField> P256AffineCompactEcdsaCircuitV1<F> {
    pub(super) fn new(sec1_uncompressed: [u8; 65], digest: [u8; 32], signature: [u8; 64]) -> Self {
        Self {
            sec1_uncompressed,
            digest,
            signature,
            _field: PhantomData,
        }
    }

    pub(super) fn instances(&self) -> Result<Vec<F>, Error> {
        self.build_trace().map(|trace| trace.instances)
    }

    pub(super) fn row_report(&self) -> Result<P256AffineCompactRowsV1, Error> {
        self.build_trace().map(|trace| trace.rows)
    }

    #[cfg(test)]
    fn trace_diagnostic_for_test(
        &self,
    ) -> Result<P256AffineCompactRowsV1, P256AffineCompactFailureV1> {
        self.build_trace_diagnostic().map(|trace| trace.rows)
    }

    #[cfg(test)]
    fn instance_contract_for_test(&self) -> Result<(Vec<F>, Vec<F>), P256AffineCompactFailureV1> {
        let builder = self.build_trace_builder_diagnostic()?;
        Ok((
            builder
                .caller_instances
                .iter()
                .map(|variable| variable.value)
                .collect(),
            builder
                .constant_instances
                .iter()
                .map(|variable| variable.value)
                .collect(),
        ))
    }

    fn input_bytes(&self) -> [u8; PUBLIC_BYTES] {
        let mut bytes = [0_u8; PUBLIC_BYTES];
        bytes[..65].copy_from_slice(&self.sec1_uncompressed);
        bytes[65..97].copy_from_slice(&self.digest);
        bytes[97..].copy_from_slice(&self.signature);
        bytes
    }

    fn build_trace(&self) -> Result<AffineTrace<F>, Error> {
        self.build_trace_diagnostic().map_err(|_| Error::Synthesis)
    }

    fn build_trace_builder_diagnostic(
        &self,
    ) -> Result<TraceBuilder<F>, P256AffineCompactFailureV1> {
        let mut builder = TraceBuilder::new();
        constrain_ecdsa(
            &mut builder,
            &self.input_bytes(),
            &self.sec1_uncompressed,
            &self.digest,
            &self.signature,
        )?;
        Ok(builder)
    }

    fn build_trace_diagnostic(&self) -> Result<AffineTrace<F>, P256AffineCompactFailureV1> {
        self.build_trace_builder_diagnostic()?.finish()
    }
}

/// Eight equality-enabled current-query columns. No rotation other than current is configured.
#[derive(Clone, Debug)]
pub(super) struct P256AffineCompactConfigV1 {
    advice: [Column<Advice>; ADVICE_COLUMNS],
    instance: Column<Instance>,
    mode: Column<Fixed>,
    range_kind: Column<Fixed>,
    table_kind: Column<Fixed>,
    table: Column<Fixed>,
}

impl P256AffineCompactConfigV1 {
    fn configure<F: BigPrimeField>(meta: &mut ConstraintSystem<F>) -> Self {
        let advice = std::array::from_fn(|_| {
            let column = meta.advice_column();
            meta.enable_equality(column);
            column
        });
        let instance = meta.instance_column();
        let mode = meta.fixed_column();
        let range_kind = meta.fixed_column();
        let table_kind = meta.fixed_column();
        let table = meta.fixed_column();
        meta.create_gate("compact affine P-256 current-row machine", |meta| {
            let values = advice.map(|column| meta.query_advice(column, Rotation::cur()));
            let public = meta.query_instance(instance, Rotation::cur());
            let mode_value = meta.query_fixed(mode, Rotation::cur());
            let fma = mode_lagrange(mode_value.clone(), 1);
            let bind = mode_lagrange(mode_value.clone(), 2);
            let select = mode_lagrange(mode_value, 3);
            vec![
                fma.clone()
                    * (values[0].clone() + values[1].clone() * values[2].clone()
                        - values[3].clone()),
                fma * (values[4].clone() + values[5].clone() * values[6].clone()
                    - values[7].clone()),
                bind * (values[3].clone() - public),
                select.clone()
                    * (values[0].clone()
                        + values[1].clone() * (values[2].clone() - values[0].clone())
                        - values[3].clone()),
                select
                    * (values[4].clone()
                        + values[5].clone() * (values[6].clone() - values[4].clone())
                        - values[7].clone()),
            ]
        });
        meta.lookup_any("compact affine range lane zero", |meta| {
            let kind = meta.query_fixed(range_kind, Rotation::cur());
            let value = meta.query_advice(advice[1], Rotation::cur());
            let expected_kind = meta.query_fixed(table_kind, Rotation::cur());
            let table = meta.query_fixed(table, Rotation::cur());
            vec![
                (kind.clone(), expected_kind.clone()),
                (kind * value, expected_kind * table),
            ]
        });
        meta.lookup_any("compact affine range lane one", |meta| {
            let kind = meta.query_fixed(range_kind, Rotation::cur());
            let value = meta.query_advice(advice[5], Rotation::cur());
            let expected_kind = meta.query_fixed(table_kind, Rotation::cur());
            let table = meta.query_fixed(table, Rotation::cur());
            vec![
                (kind.clone(), expected_kind.clone()),
                (kind * value, expected_kind * table),
            ]
        });
        meta.set_minimum_degree(P256_AFFINE_COMPACT_SHAPE_V1.degree);
        Self {
            advice,
            instance,
            mode,
            range_kind,
            table_kind,
            table,
        }
    }
}

impl<F: BigPrimeField> Circuit<F> for P256AffineCompactEcdsaCircuitV1<F> {
    type Config = P256AffineCompactConfigV1;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        P256AffineCompactConfigV1::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.build_trace()?.assign(&config, &mut layouter)
    }
}

fn mode_lagrange<F: BigPrimeField>(mode: Expression<F>, target: u64) -> Expression<F> {
    let mut numerator = Expression::Constant(F::ONE);
    let mut denominator = F::ONE;
    for other in 0_u64..4 {
        if other == target {
            continue;
        }
        numerator = numerator * (mode.clone() - Expression::Constant(F::from(other)));
        denominator *= F::from(target) - F::from(other);
    }
    numerator * Expression::Constant(denominator.invert().unwrap())
}

#[derive(Clone, Copy, Debug)]
struct CellVar<F> {
    id: usize,
    value: F,
}

#[derive(Clone, Debug)]
struct BoundedCell<F> {
    cell: CellVar<F>,
    integer: BigUint,
}

#[derive(Clone, Debug)]
struct BoolVar<F> {
    bounded: BoundedCell<F>,
}

impl<F: Copy> BoolVar<F> {
    fn cell(&self) -> CellVar<F> {
        self.bounded.cell
    }

    fn value(&self) -> bool {
        self.bounded.integer == BigUint::from(1_u8)
    }
}

#[derive(Clone, Debug)]
struct UintVar<F> {
    limbs: [BoundedCell<F>; LIMBS],
    value: BigUint,
}

#[derive(Clone, Copy, Debug)]
struct FmaRelation<F> {
    cells: [CellVar<F>; 4],
}

#[derive(Clone, Copy, Debug)]
struct SelectRelation<F> {
    cells: [CellVar<F>; 4],
}

#[derive(Clone, Copy, Debug)]
struct RangeRelation<F> {
    gate: FmaRelation<F>,
    bits: usize,
}

#[derive(Clone, Debug)]
struct TraceBuilder<F> {
    next_id: usize,
    caller_instances: Vec<CellVar<F>>,
    constant_instances: Vec<CellVar<F>>,
    constants: HashMap<BigUint, CellVar<F>>,
    fmas: Vec<FmaRelation<F>>,
    ranges: Vec<RangeRelation<F>>,
    selects: Vec<SelectRelation<F>>,
    modular_relations: usize,
    complete_doublings: usize,
    complete_additions: usize,
    maximum_quotient_bits: usize,
    maximum_carry_bits: usize,
}

impl<F: BigPrimeField> TraceBuilder<F> {
    fn new() -> Self {
        Self {
            next_id: 0,
            caller_instances: Vec::new(),
            constant_instances: Vec::new(),
            constants: HashMap::new(),
            fmas: Vec::new(),
            ranges: Vec::new(),
            selects: Vec::new(),
            modular_relations: 0,
            complete_doublings: 0,
            complete_additions: 0,
            maximum_quotient_bits: 0,
            maximum_carry_bits: 0,
        }
    }

    fn witness_fe(&mut self, value: F) -> CellVar<F> {
        let variable = CellVar {
            id: self.next_id,
            value,
        };
        self.next_id += 1;
        variable
    }

    fn witness_big(&mut self, value: &BigUint) -> CellVar<F> {
        self.witness_fe(biguint_to_fe::<F>(value))
    }

    fn caller_instance(&mut self, value: u8) -> BoundedCell<F> {
        let integer = BigUint::from(value);
        let cell = self.witness_big(&integer);
        self.caller_instances.push(cell);
        BoundedCell { cell, integer }
    }

    fn constant(&mut self, value: impl Into<BigUint>) -> CellVar<F> {
        let value = value.into();
        if let Some(constant) = self.constants.get(&value) {
            return *constant;
        }
        let constant = self.witness_big(&value);
        self.constants.insert(value, constant);
        self.constant_instances.push(constant);
        constant
    }

    fn zero(&mut self) -> CellVar<F> {
        self.constant(0_u8)
    }

    fn one(&mut self) -> CellVar<F> {
        self.constant(1_u8)
    }

    fn push_fma(&mut self, cells: [CellVar<F>; 4]) {
        self.fmas.push(FmaRelation { cells });
    }

    fn fma(&mut self, add: CellVar<F>, left: CellVar<F>, right: CellVar<F>) -> CellVar<F> {
        let output = self.witness_fe(add.value + left.value * right.value);
        self.push_fma([add, left, right, output]);
        output
    }

    fn mul(&mut self, left: CellVar<F>, right: CellVar<F>) -> CellVar<F> {
        let zero = self.zero();
        self.fma(zero, left, right)
    }

    fn add(&mut self, left: CellVar<F>, right: CellVar<F>) -> CellVar<F> {
        let one = self.one();
        self.fma(left, right, one)
    }

    fn subtract(&mut self, left: CellVar<F>, right: CellVar<F>) -> CellVar<F> {
        let output = self.witness_fe(left.value - right.value);
        let one = self.one();
        self.push_fma([output, right, one, left]);
        output
    }

    fn scale(&mut self, value: CellVar<F>, coefficient: impl Into<BigUint>) -> CellVar<F> {
        let coefficient = self.constant(coefficient);
        self.mul(value, coefficient)
    }

    fn assert_equal(&mut self, left: CellVar<F>, right: CellVar<F>) {
        let zero = self.zero();
        self.push_fma([left, zero, zero, right]);
    }

    fn assert_zero(&mut self, value: CellVar<F>) {
        let zero = self.zero();
        self.assert_equal(value, zero);
    }

    fn boolean(&mut self, value: bool) -> BoolVar<F> {
        let integer = BigUint::from(u8::from(value));
        let cell = self.witness_big(&integer);
        let zero = self.zero();
        self.push_fma([zero, cell, cell, cell]);
        BoolVar {
            bounded: BoundedCell { cell, integer },
        }
    }

    /// A verifier-derived Boolean constant, not a prover-selectable predicate.
    fn constant_bool(&mut self, value: bool) -> BoolVar<F> {
        let integer = BigUint::from(u8::from(value));
        let cell = self.constant(integer.clone());
        BoolVar {
            bounded: BoundedCell { cell, integer },
        }
    }

    fn bool_not(&mut self, value: &BoolVar<F>) -> BoolVar<F> {
        let one = self.one();
        let cell = self.subtract(one, value.cell());
        BoolVar {
            bounded: BoundedCell {
                cell,
                integer: BigUint::from(u8::from(!value.value())),
            },
        }
    }

    fn bool_and(&mut self, left: &BoolVar<F>, right: &BoolVar<F>) -> BoolVar<F> {
        let cell = self.mul(left.cell(), right.cell());
        BoolVar {
            bounded: BoundedCell {
                cell,
                integer: BigUint::from(u8::from(left.value() && right.value())),
            },
        }
    }

    fn bool_or_exclusive(&mut self, left: &BoolVar<F>, right: &BoolVar<F>) -> BoolVar<F> {
        let cell = self.add(left.cell(), right.cell());
        let value = left.value() || right.value();
        let result = BoolVar {
            bounded: BoundedCell {
                cell,
                integer: BigUint::from(u8::from(value)),
            },
        };
        let zero = self.zero();
        self.push_fma([zero, cell, cell, cell]);
        result
    }

    fn select(&mut self, left: CellVar<F>, bit: &BoolVar<F>, right: CellVar<F>) -> CellVar<F> {
        let output = self.witness_fe(if bit.value() { right.value } else { left.value });
        self.selects.push(SelectRelation {
            cells: [left, bit.cell(), right, output],
        });
        output
    }

    fn is_zero_cell(&mut self, value: CellVar<F>, is_zero: bool) -> BoolVar<F> {
        let zero = self.zero();
        let one = self.one();
        let flag = self.boolean(is_zero);
        let inverse = self.witness_fe(if is_zero {
            F::ZERO
        } else {
            value.value.invert().unwrap()
        });
        let product = self.mul(value, inverse);
        let one_minus_flag = self.subtract(one, flag.cell());
        self.assert_equal(product, one_minus_flag);
        let gated = self.mul(value, flag.cell());
        self.assert_equal(gated, zero);
        flag
    }

    fn range_bounded(
        &mut self,
        bounded: &BoundedCell<F>,
        bits: usize,
        witness: &'static str,
    ) -> Result<(), P256AffineCompactFailureV1> {
        let actual_bits = usize::try_from(bounded.integer.bits()).unwrap_or(usize::MAX);
        if actual_bits > bits {
            return Err(P256AffineCompactFailureV1::IntegerBound {
                witness,
                actual_bits,
                maximum_bits: bits,
            });
        }
        let chunks = bits.div_ceil(LOOKUP_BITS);
        let mask = (BigUint::from(1_u8) << LOOKUP_BITS) - 1_u8;
        let mut accumulator = self.zero();
        for chunk_index in 0..chunks {
            let shift = chunk_index * LOOKUP_BITS;
            let integer = (&bounded.integer >> shift) & &mask;
            let chunk = self.witness_big(&integer);
            let remaining = bits - shift;
            let chunk_bits = remaining.min(LOOKUP_BITS);
            let power = self.constant(BigUint::from(1_u8) << shift);
            let output = if chunk_index + 1 == chunks {
                bounded.cell
            } else {
                self.witness_fe(accumulator.value + chunk.value * power.value)
            };
            self.ranges.push(RangeRelation {
                gate: FmaRelation {
                    cells: [accumulator, chunk, power, output],
                },
                bits: chunk_bits,
            });
            accumulator = output;
        }
        Ok(())
    }

    fn load_uint(
        &mut self,
        value: BigUint,
        witness: &'static str,
    ) -> Result<UintVar<F>, P256AffineCompactFailureV1> {
        let radix = radix();
        let mask = &radix - 1_u8;
        let limbs = std::array::from_fn(|index| {
            let integer = (&value >> (index * LIMB_BITS)) & &mask;
            let cell = self.witness_big(&integer);
            BoundedCell { cell, integer }
        });
        for limb in &limbs {
            self.range_bounded(limb, LIMB_BITS, witness)?;
        }
        Ok(UintVar { limbs, value })
    }

    fn finish(self) -> Result<AffineTrace<F>, P256AffineCompactFailureV1> {
        transpose_affine_trace(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowMode {
    // Fixed columns default to zero on unusable/blinding rows, so zero must
    // disable every custom-gate relation.
    Disabled = 0,
    Fma = 1,
    Bind = 2,
    Select = 3,
}

#[derive(Clone, Debug)]
struct AffineRow<F> {
    values: [F; ADVICE_COLUMNS],
    aliases: Vec<(usize, usize)>,
    mode: RowMode,
    range_kind: usize,
}

impl<F: BigPrimeField> AffineRow<F> {
    fn zero() -> Self {
        Self {
            values: [F::ZERO; ADVICE_COLUMNS],
            aliases: Vec::new(),
            mode: RowMode::Disabled,
            range_kind: 0,
        }
    }

    fn set_lane(&mut self, lane: usize, relation: FmaRelation<F>) {
        let base = 4 * lane;
        for (offset, variable) in relation.cells.into_iter().enumerate() {
            self.values[base + offset] = variable.value;
            self.aliases.push((variable.id, base + offset));
        }
    }

    fn set_select_lane(&mut self, lane: usize, relation: SelectRelation<F>) {
        let base = 4 * lane;
        for (offset, variable) in relation.cells.into_iter().enumerate() {
            self.values[base + offset] = variable.value;
            self.aliases.push((variable.id, base + offset));
        }
    }
}

#[derive(Clone, Debug)]
struct AffineTrace<F> {
    rows_data: Vec<AffineRow<F>>,
    instances: Vec<F>,
    rows: P256AffineCompactRowsV1,
}

impl<F: BigPrimeField> AffineTrace<F> {
    fn assign(
        &self,
        config: &P256AffineCompactConfigV1,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), Error> {
        if self.rows_data.len() > K16_MAX_ASSIGNED_ROWS {
            return Err(Error::Synthesis);
        }
        layouter.assign_region(
            || "compact affine P-256 trace and overlapping range table",
            |mut region| {
                let mut first_cells = HashMap::<usize, Cell>::new();
                for (offset, row) in self.rows_data.iter().enumerate() {
                    let cells: [Cell; ADVICE_COLUMNS] = std::array::from_fn(|column| {
                        raw_assign_advice(
                            &mut region,
                            config.advice[column],
                            offset,
                            Value::known(Assigned::Trivial(row.values[column])),
                        )
                        .cell()
                    });
                    raw_assign_fixed(&mut region, config.mode, offset, F::from(row.mode as u64));
                    raw_assign_fixed(
                        &mut region,
                        config.range_kind,
                        offset,
                        F::from(row.range_kind as u64),
                    );
                    for (variable, column) in &row.aliases {
                        if let Some(first) = first_cells.insert(*variable, cells[*column]) {
                            raw_constrain_equal(&mut region, first, cells[*column]);
                        }
                    }
                }
                let mut offset = 0_usize;
                for bits in RANGE_CHUNK_BITS {
                    for value in 0..(1_usize << bits) {
                        raw_assign_fixed(
                            &mut region,
                            config.table_kind,
                            offset,
                            F::from(bits as u64),
                        );
                        raw_assign_fixed(&mut region, config.table, offset, F::from(value as u64));
                        offset += 1;
                    }
                }
                debug_assert_eq!(offset, TABLE_ROWS);
                Ok(())
            },
        )
    }
}

fn transpose_affine_trace<F: BigPrimeField>(
    builder: TraceBuilder<F>,
) -> Result<AffineTrace<F>, P256AffineCompactFailureV1> {
    let TraceBuilder {
        caller_instances,
        constant_instances,
        fmas,
        ranges,
        selects,
        modular_relations,
        complete_doublings,
        complete_additions,
        maximum_quotient_bits,
        maximum_carry_bits,
        ..
    } = builder;
    let mut rows_data = Vec::new();
    let instances = caller_instances
        .iter()
        .chain(&constant_instances)
        .map(|variable| variable.value)
        .collect::<Vec<_>>();
    for variable in caller_instances.iter().chain(&constant_instances) {
        let mut row = AffineRow::zero();
        row.mode = RowMode::Bind;
        row.values[0] = variable.value;
        row.values[3] = variable.value;
        row.aliases.push((variable.id, 0));
        row.aliases.push((variable.id, 3));
        rows_data.push(row);
    }
    let binding_rows = rows_data.len();

    let mut ranges_by_bits = ranges.into_iter().fold(
        HashMap::<usize, Vec<FmaRelation<F>>>::new(),
        |mut grouped, relation| {
            grouped
                .entry(relation.bits)
                .or_default()
                .push(relation.gate);
            grouped
        },
    );
    let mut bit_widths = ranges_by_bits.keys().copied().collect::<Vec<_>>();
    bit_widths.sort_unstable();
    let mut range_lookups = 0_usize;
    for bits in bit_widths {
        debug_assert!(RANGE_CHUNK_BITS.contains(&bits));
        let relations = ranges_by_bits
            .remove(&bits)
            .expect("range bit width was collected from this map");
        range_lookups += relations.len();
        for pair in relations.chunks(2) {
            let mut row = AffineRow::zero();
            row.mode = RowMode::Fma;
            row.range_kind = bits;
            for (lane, relation) in pair.iter().copied().enumerate() {
                row.set_lane(lane, relation);
            }
            rows_data.push(row);
        }
    }
    let range_rows = rows_data.len() - binding_rows;

    for pair in fmas.chunks(2) {
        let mut row = AffineRow::zero();
        row.mode = RowMode::Fma;
        for (lane, relation) in pair.iter().copied().enumerate() {
            row.set_lane(lane, relation);
        }
        rows_data.push(row);
    }
    let arithmetic_rows = rows_data.len() - binding_rows - range_rows;

    for pair in selects.chunks(2) {
        let mut row = AffineRow::zero();
        row.mode = RowMode::Select;
        for (lane, relation) in pair.iter().copied().enumerate() {
            row.set_select_lane(lane, relation);
        }
        rows_data.push(row);
    }
    let selection_rows = rows_data.len() - binding_rows - range_rows - arithmetic_rows;

    let fixed_selection_rows = 0;
    let rows = P256AffineCompactRowsV1 {
        binding_rows,
        range_rows,
        arithmetic_rows,
        selection_rows,
        fixed_selection_rows,
        total_rows: rows_data.len(),
        range_lookups,
        fma_relations: fmas.len(),
        select_relations: selects.len(),
        modular_relations,
        complete_doublings,
        complete_additions,
        maximum_quotient_bits,
        maximum_carry_bits,
    };
    if rows.total_rows > K16_MAX_ASSIGNED_ROWS {
        return Err(P256AffineCompactFailureV1::RowCapacityExceeded {
            rows: Box::new(rows),
            maximum: K16_MAX_ASSIGNED_ROWS,
        });
    }
    if instances.len() != binding_rows {
        return Err(P256AffineCompactFailureV1::InstanceBindingMismatch {
            instances: instances.len(),
            binding_rows,
        });
    }
    Ok(AffineTrace {
        rows_data,
        instances,
        rows,
    })
}

fn radix() -> BigUint {
    BigUint::from(1_u8) << LIMB_BITS
}

fn modulus_base() -> BigUint {
    BigUint::from_bytes_be(&P256_BASE_MODULUS_BE)
}

fn modulus_scalar() -> BigUint {
    BigUint::from_bytes_be(&P256_SCALAR_MODULUS_BE)
}

fn curve_b() -> BigUint {
    BigUint::from_bytes_be(&P256_B_BE)
}

fn decompose_limbs(value: &BigUint) -> [BigUint; LIMBS] {
    let mask = radix() - 1_u8;
    std::array::from_fn(|index| (value >> (index * LIMB_BITS)) & &mask)
}

fn compose_limbs(limbs: &[BigUint; LIMBS]) -> BigUint {
    limbs
        .iter()
        .enumerate()
        .fold(BigUint::from(0_u8), |sum, (index, limb)| {
            sum + (limb << (index * LIMB_BITS))
        })
}

fn bigint(value: &BigUint) -> BigInt {
    BigInt::from_biguint(Sign::Plus, value.clone())
}

fn modular_sub(left: &BigUint, right: &BigUint, modulus: &BigUint) -> BigUint {
    (left + modulus - (right % modulus)) % modulus
}

fn modular_inverse(value: &BigUint, modulus: &BigUint) -> BigUint {
    if *value == BigUint::from(0_u8) {
        BigUint::from(0_u8)
    } else {
        value.modpow(&(modulus - 2_u8), modulus)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AffineValue {
    x: BigUint,
    y: BigUint,
    infinity: bool,
}

impl AffineValue {
    fn identity() -> Self {
        Self {
            x: BigUint::from(0_u8),
            y: BigUint::from(0_u8),
            infinity: true,
        }
    }

    fn generator() -> Self {
        Self {
            x: BigUint::from_bytes_be(&P256_GENERATOR_X_BE),
            y: BigUint::from_bytes_be(&P256_GENERATOR_Y_BE),
            infinity: false,
        }
    }
}

fn affine_double_value(point: &AffineValue) -> (AffineValue, BigUint) {
    if point.infinity {
        return (AffineValue::identity(), BigUint::from(0_u8));
    }
    let modulus = modulus_base();
    let denominator = (&point.y << 1_usize) % &modulus;
    if denominator == BigUint::from(0_u8) {
        return (AffineValue::identity(), BigUint::from(0_u8));
    }
    let x_squared = (&point.x * &point.x) % &modulus;
    let numerator = modular_sub(
        &((&x_squared * 3_u8) % &modulus),
        &BigUint::from(3_u8),
        &modulus,
    );
    let lambda = (numerator * modular_inverse(&denominator, &modulus)) % &modulus;
    let x = modular_sub(
        &modular_sub(&((&lambda * &lambda) % &modulus), &point.x, &modulus),
        &point.x,
        &modulus,
    );
    let y = modular_sub(
        &((&lambda * modular_sub(&point.x, &x, &modulus)) % &modulus),
        &point.y,
        &modulus,
    );
    (
        AffineValue {
            x,
            y,
            infinity: false,
        },
        lambda,
    )
}

fn affine_add_value(left: &AffineValue, right: &AffineValue) -> (AffineValue, BigUint) {
    if left.infinity {
        return (right.clone(), BigUint::from(0_u8));
    }
    if right.infinity {
        return (left.clone(), BigUint::from(0_u8));
    }
    let modulus = modulus_base();
    if left.x == right.x {
        if (&left.y + &right.y) % &modulus == BigUint::from(0_u8) {
            return (AffineValue::identity(), BigUint::from(0_u8));
        }
        if left.y == right.y {
            return affine_double_value(left);
        }
        return (AffineValue::identity(), BigUint::from(0_u8));
    }
    let numerator = modular_sub(&right.y, &left.y, &modulus);
    let denominator = modular_sub(&right.x, &left.x, &modulus);
    let lambda = (numerator * modular_inverse(&denominator, &modulus)) % &modulus;
    let x = modular_sub(
        &modular_sub(&((&lambda * &lambda) % &modulus), &left.x, &modulus),
        &right.x,
        &modulus,
    );
    let y = modular_sub(
        &((&lambda * modular_sub(&left.x, &x, &modulus)) % &modulus),
        &left.y,
        &modulus,
    );
    (
        AffineValue {
            x,
            y,
            infinity: false,
        },
        lambda,
    )
}

fn fixed_generator_values() -> [AffineValue; 16] {
    let generator = AffineValue::generator();
    let mut values = std::array::from_fn(|_| AffineValue::identity());
    values[1] = generator.clone();
    for index in 2..16 {
        values[index] = affine_add_value(&values[index - 1], &generator).0;
    }
    values
}

fn constrain_exact_constant_sum<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
    constant: &BigUint,
) {
    let radix = radix();
    let constant_limbs = decompose_limbs(constant);
    let mut carry = builder.constant_bool(false);
    for (index, constant_limb) in constant_limbs.iter().enumerate() {
        let integer_sum =
            &left.limbs[index].integer + &right.limbs[index].integer + &carry.bounded.integer;
        let next_value = integer_sum >= radix;
        let next = builder.boolean(next_value);
        let sum = builder.add(left.limbs[index].cell, right.limbs[index].cell);
        let with_carry = builder.add(sum, carry.cell());
        let radix_next = builder.scale(next.cell(), radix.clone());
        let constant_limb = builder.constant(constant_limb.clone());
        let expected = builder.add(constant_limb, radix_next);
        builder.assert_equal(with_carry, expected);
        carry = next;
    }
    builder.assert_zero(carry.cell());
}

fn constrain_canonical<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    value: &UintVar<F>,
    modulus: &BigUint,
    witness: &'static str,
) -> Result<(), P256AffineCompactFailureV1> {
    let maximum = modulus - 1_u8;
    let slack_value = if value.value <= maximum {
        &maximum - &value.value
    } else {
        BigUint::from(0_u8)
    };
    let slack = builder.load_uint(slack_value, witness)?;
    constrain_exact_constant_sum(builder, value, &slack, &maximum);
    Ok(())
}

fn constrain_at_most<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    value: &UintVar<F>,
    maximum: &BigUint,
    witness: &'static str,
) -> Result<BoolVar<F>, P256AffineCompactFailureV1> {
    let valid = value.value <= *maximum;
    let slack_value = if valid {
        maximum - &value.value
    } else {
        BigUint::from(0_u8)
    };
    let slack = builder.load_uint(slack_value, witness)?;
    constrain_exact_constant_sum(builder, value, &slack, maximum);
    Ok(builder.boolean(valid))
}

fn constrain_uint_equal<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
) {
    for (left, right) in left.limbs.iter().zip(&right.limbs) {
        builder.assert_equal(left.cell, right.cell);
    }
}

fn uint_is_zero<F: BigPrimeField>(builder: &mut TraceBuilder<F>, value: &UintVar<F>) -> BoolVar<F> {
    let mut result = builder.constant_bool(true);
    for limb in &value.limbs {
        let zero = builder.is_zero_cell(limb.cell, limb.integer == BigUint::from(0_u8));
        result = builder.bool_and(&result, &zero);
    }
    result
}

fn uint_equal<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
) -> BoolVar<F> {
    let mut result = builder.constant_bool(true);
    for (left, right) in left.limbs.iter().zip(&right.limbs) {
        let difference = builder.subtract(left.cell, right.cell);
        let equal = builder.is_zero_cell(difference, left.integer == right.integer);
        result = builder.bool_and(&result, &equal);
    }
    result
}

fn gate_uint_zero<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    gate: &BoolVar<F>,
    value: &UintVar<F>,
) {
    for limb in &value.limbs {
        let product = builder.mul(gate.cell(), limb.cell);
        builder.assert_zero(product);
    }
}

fn bind_public_uint<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    bytes_be: &[BoundedCell<F>; 32],
    raw: &[u8; 32],
    witness: &'static str,
) -> Result<UintVar<F>, P256AffineCompactFailureV1> {
    let value = BigUint::from_bytes_be(raw);
    let loaded = builder.load_uint(value, witness)?;
    let bytes_le = std::array::from_fn::<_, 32, _>(|index| bytes_be[31 - index].clone());

    let byte10 = &bytes_le[10];
    let low6_integer = &byte10.integer & BigUint::from(0x3f_u8);
    let high2_integer = &byte10.integer >> 6_usize;
    let low6 = BoundedCell {
        cell: builder.witness_big(&low6_integer),
        integer: low6_integer,
    };
    let high2 = BoundedCell {
        cell: builder.witness_big(&high2_integer),
        integer: high2_integer,
    };
    builder.range_bounded(&low6, 6, witness)?;
    builder.range_bounded(&high2, 2, witness)?;
    let sixty_four = builder.constant(64_u8);
    let recomposed10 = builder.fma(low6.cell, high2.cell, sixty_four);
    builder.assert_equal(recomposed10, byte10.cell);

    let byte21 = &bytes_le[21];
    let low4_integer = &byte21.integer & BigUint::from(0x0f_u8);
    let high4_integer = &byte21.integer >> 4_usize;
    let low4 = BoundedCell {
        cell: builder.witness_big(&low4_integer),
        integer: low4_integer,
    };
    let high4 = BoundedCell {
        cell: builder.witness_big(&high4_integer),
        integer: high4_integer,
    };
    builder.range_bounded(&low4, 4, witness)?;
    builder.range_bounded(&high4, 4, witness)?;
    let sixteen = builder.constant(16_u8);
    let recomposed21 = builder.fma(low4.cell, high4.cell, sixteen);
    builder.assert_equal(recomposed21, byte21.cell);

    let zero = builder.zero();
    let mut limb0 = zero;
    for (index, byte) in bytes_le[..10].iter().enumerate() {
        let power = builder.constant(BigUint::from(1_u8) << (8 * index));
        limb0 = builder.fma(limb0, byte.cell, power);
    }
    let power80 = builder.constant(BigUint::from(1_u8) << 80);
    limb0 = builder.fma(limb0, low6.cell, power80);
    builder.assert_equal(limb0, loaded.limbs[0].cell);

    let mut limb1 = high2.cell;
    for (offset, byte) in bytes_le[11..21].iter().enumerate() {
        let power = builder.constant(BigUint::from(1_u8) << (2 + 8 * offset));
        limb1 = builder.fma(limb1, byte.cell, power);
    }
    let power82 = builder.constant(BigUint::from(1_u8) << 82);
    limb1 = builder.fma(limb1, low4.cell, power82);
    builder.assert_equal(limb1, loaded.limbs[1].cell);

    let mut limb2 = high4.cell;
    for (offset, byte) in bytes_le[22..].iter().enumerate() {
        let power = builder.constant(BigUint::from(1_u8) << (4 + 8 * offset));
        limb2 = builder.fma(limb2, byte.cell, power);
    }
    builder.assert_equal(limb2, loaded.limbs[2].cell);
    Ok(loaded)
}

#[derive(Clone, Debug)]
struct PolyFactor<F> {
    cell: Option<CellVar<F>>,
    integer: BigUint,
}

impl<F> From<&BoundedCell<F>> for PolyFactor<F>
where
    F: Copy,
{
    fn from(value: &BoundedCell<F>) -> Self {
        Self {
            cell: Some(value.cell),
            integer: value.integer.clone(),
        }
    }
}

impl<F> From<&BoolVar<F>> for PolyFactor<F>
where
    F: Copy,
{
    fn from(value: &BoolVar<F>) -> Self {
        Self::from(&value.bounded)
    }
}

#[derive(Clone, Debug)]
struct PolyTerm<F> {
    coefficient: i64,
    factors: Vec<PolyFactor<F>>,
}

impl<F: Copy> PolyTerm<F> {
    fn integer(&self) -> BigInt {
        self.factors
            .iter()
            .fold(BigInt::from(self.coefficient), |value, factor| {
                value * bigint(&factor.integer)
            })
    }
}

#[derive(Clone, Debug)]
struct RadixExpression<F> {
    coefficients: [Vec<PolyTerm<F>>; 2 * LIMBS - 1],
}

impl<F: Copy> RadixExpression<F> {
    fn new() -> Self {
        Self {
            coefficients: std::array::from_fn(|_| Vec::new()),
        }
    }

    fn add_product(
        &mut self,
        left: &UintVar<F>,
        right: &UintVar<F>,
        gate: Option<&BoolVar<F>>,
        coefficient: i64,
    ) {
        for (left_index, left_limb) in left.limbs.iter().enumerate() {
            for (right_index, right_limb) in right.limbs.iter().enumerate() {
                let mut factors = vec![PolyFactor::from(left_limb), PolyFactor::from(right_limb)];
                if let Some(gate) = gate {
                    factors.push(PolyFactor::from(gate));
                }
                self.coefficients[left_index + right_index].push(PolyTerm {
                    coefficient,
                    factors,
                });
            }
        }
    }

    fn add_linear(&mut self, value: &UintVar<F>, gate: Option<&BoolVar<F>>, coefficient: i64) {
        for (index, limb) in value.limbs.iter().enumerate() {
            let mut factors = vec![PolyFactor::from(limb)];
            if let Some(gate) = gate {
                factors.push(PolyFactor::from(gate));
            }
            self.coefficients[index].push(PolyTerm {
                coefficient,
                factors,
            });
        }
    }

    fn add_constant(&mut self, value: &BigUint, coefficient: i64) {
        for (index, limb) in decompose_limbs(value).into_iter().enumerate() {
            self.coefficients[index].push(PolyTerm {
                coefficient,
                factors: vec![PolyFactor {
                    cell: None,
                    integer: limb,
                }],
            });
        }
    }

    fn add_small_gated_constant(
        &mut self,
        value: u64,
        gate: Option<&BoolVar<F>>,
        coefficient: i64,
    ) {
        let factors = gate
            .map(|gate| vec![PolyFactor::from(gate)])
            .unwrap_or_default();
        self.coefficients[0].push(PolyTerm {
            coefficient: coefficient * i64::try_from(value).expect("small formula constant"),
            factors,
        });
    }

    fn integer_coefficients(&self) -> [BigInt; 2 * LIMBS - 1] {
        std::array::from_fn(|index| {
            self.coefficients[index]
                .iter()
                .map(PolyTerm::integer)
                .fold(BigInt::from(0), |sum, term| sum + term)
        })
    }

    fn integer(&self) -> BigInt {
        let radix = BigInt::from_biguint(Sign::Plus, radix());
        self.integer_coefficients().into_iter().enumerate().fold(
            BigInt::from(0),
            |sum, (index, coefficient)| {
                sum + coefficient * radix.pow(u32::try_from(index).expect("five coefficients"))
            },
        )
    }
}

fn realize_poly_term<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    term: &PolyTerm<F>,
) -> CellVar<F> {
    let mut product = builder.one();
    for factor in &term.factors {
        let cell = factor
            .cell
            .unwrap_or_else(|| builder.constant(factor.integer.clone()));
        product = builder.mul(product, cell);
    }
    let magnitude = term.coefficient.unsigned_abs();
    if magnitude != 1 {
        product = builder.scale(product, magnitude);
    }
    product
}

fn realize_coefficient<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    terms: &[PolyTerm<F>],
) -> CellVar<F> {
    let mut accumulator = builder.zero();
    for term in terms {
        let value = realize_poly_term(builder, term);
        accumulator = if term.coefficient < 0 {
            builder.subtract(accumulator, value)
        } else {
            builder.add(accumulator, value)
        };
    }
    accumulator
}

#[derive(Clone, Debug)]
struct SignedMagnitude<F> {
    sign: BoolVar<F>,
    magnitude: UintVar<F>,
    signed_limbs: [CellVar<F>; LIMBS],
}

fn signed_uint_witness<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    value: &BigInt,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<SignedMagnitude<F>, P256AffineCompactFailureV1> {
    let negative = value.sign() == Sign::Minus;
    let magnitude_value = value.magnitude().clone();
    let bits = usize::try_from(magnitude_value.bits()).unwrap_or(usize::MAX);
    if bits > QUOTIENT_BITS {
        return Err(P256AffineCompactFailureV1::IntegerBound {
            witness,
            actual_bits: bits,
            maximum_bits: QUOTIENT_BITS,
        });
    }
    builder.maximum_quotient_bits = builder.maximum_quotient_bits.max(bits);
    let sign = builder.boolean(negative);
    let magnitude = builder.load_uint(magnitude_value, witness)?;
    let zero = uint_is_zero(builder, &magnitude);
    let negative_zero = builder.mul(sign.cell(), zero.cell());
    builder.assert_zero(negative_zero);
    let inactive = builder.bool_not(active);
    gate_uint_zero(builder, &inactive, &magnitude);
    let signed_limbs = std::array::from_fn(|index| {
        let signed_part = builder.mul(sign.cell(), magnitude.limbs[index].cell);
        let twice = builder.scale(signed_part, 2_u8);
        builder.subtract(magnitude.limbs[index].cell, twice)
    });
    Ok(SignedMagnitude {
        sign,
        magnitude,
        signed_limbs,
    })
}

#[derive(Clone, Debug)]
struct SignedCarry<F> {
    sign: BoolVar<F>,
    magnitude: BoundedCell<F>,
    signed: CellVar<F>,
}

fn signed_carry_witness<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    value: &BigInt,
    active: &BoolVar<F>,
) -> Result<SignedCarry<F>, P256AffineCompactFailureV1> {
    let negative = value.sign() == Sign::Minus;
    let integer = value.magnitude().clone();
    let bits = usize::try_from(integer.bits()).unwrap_or(usize::MAX);
    if bits > CARRY_BITS {
        return Err(P256AffineCompactFailureV1::IntegerBound {
            witness: "signed modular carry",
            actual_bits: bits,
            maximum_bits: CARRY_BITS,
        });
    }
    builder.maximum_carry_bits = builder.maximum_carry_bits.max(bits);
    let magnitude = BoundedCell {
        cell: builder.witness_big(&integer),
        integer,
    };
    builder.range_bounded(&magnitude, CARRY_BITS, "signed modular carry")?;
    let sign = builder.boolean(negative);
    let zero = builder.is_zero_cell(magnitude.cell, magnitude.integer == BigUint::from(0_u8));
    let negative_zero = builder.mul(sign.cell(), zero.cell());
    builder.assert_zero(negative_zero);
    let inactive = builder.bool_not(active);
    let inactive_magnitude = builder.mul(inactive.cell(), magnitude.cell);
    builder.assert_zero(inactive_magnitude);
    let sign_part = builder.mul(sign.cell(), magnitude.cell);
    let twice = builder.scale(sign_part, 2_u8);
    let signed = builder.subtract(magnitude.cell, twice);
    Ok(SignedCarry {
        sign,
        magnitude,
        signed,
    })
}

fn constrain_modular_expression<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    expression: &RadixExpression<F>,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<(), P256AffineCompactFailureV1> {
    let quotient = expression.integer() / bigint(modulus);
    let quotient = signed_uint_witness(builder, &quotient, active, witness)?;
    let modulus_limbs = decompose_limbs(modulus);
    let expression_coefficients = expression.integer_coefficients();
    let signed_quotient_limbs = quotient
        .magnitude
        .limbs
        .iter()
        .map(|limb| {
            let value = bigint(&limb.integer);
            if quotient.sign.value() { -value } else { value }
        })
        .collect::<Vec<_>>();
    let quotient_coefficients: [BigInt; 2 * LIMBS - 1] = std::array::from_fn(|coefficient| {
        (0..LIMBS)
            .filter_map(|left| {
                coefficient
                    .checked_sub(left)
                    .filter(|right| *right < LIMBS)
                    .map(|right| (left, right))
            })
            .fold(BigInt::from(0), |sum, (left, right)| {
                sum + &signed_quotient_limbs[left] * bigint(&modulus_limbs[right])
            })
    });
    let radix_integer = BigInt::from_biguint(Sign::Plus, radix());
    let mut carry_in = BigInt::from(0);
    let mut carry_values = Vec::with_capacity(2 * LIMBS - 2);
    for coefficient in 0..2 * LIMBS - 2 {
        let numerator =
            &expression_coefficients[coefficient] - &quotient_coefficients[coefficient] + &carry_in;
        let carry_out = numerator / &radix_integer;
        carry_values.push(carry_out.clone());
        carry_in = carry_out;
    }
    let carries = carry_values
        .iter()
        .map(|carry| signed_carry_witness(builder, carry, active))
        .collect::<Result<Vec<_>, _>>()?;
    let zero = builder.zero();
    let radix_cell = builder.constant(radix());
    for coefficient in 0..2 * LIMBS - 1 {
        let expression_cell = realize_coefficient(builder, &expression.coefficients[coefficient]);
        let mut quotient_cell = zero;
        for left in 0..LIMBS {
            let Some(right) = coefficient.checked_sub(left).filter(|right| *right < LIMBS) else {
                continue;
            };
            let scaled = builder.scale(quotient.signed_limbs[left], modulus_limbs[right].clone());
            quotient_cell = builder.add(quotient_cell, scaled);
        }
        let carry_in_cell = coefficient
            .checked_sub(1)
            .and_then(|index| carries.get(index))
            .map_or(zero, |carry| carry.signed);
        let carry_out_cell = carries.get(coefficient).map_or(zero, |carry| carry.signed);
        let difference = builder.subtract(expression_cell, quotient_cell);
        let with_carry = builder.add(difference, carry_in_cell);
        let radix_carry = builder.mul(radix_cell, carry_out_cell);
        let residue = builder.subtract(with_carry, radix_carry);
        builder.assert_zero(residue);
    }
    builder.modular_relations += 1;
    Ok(())
}

fn modular_product<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
    output_value: BigUint,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<UintVar<F>, P256AffineCompactFailureV1> {
    let output = builder.load_uint(output_value, witness)?;
    constrain_canonical(builder, &output, modulus, witness)?;
    let mut expression = RadixExpression::new();
    expression.add_product(left, right, Some(active), 1);
    expression.add_linear(&output, Some(active), -1);
    constrain_modular_expression(builder, &expression, modulus, active, witness)?;
    let inactive = builder.bool_not(active);
    gate_uint_zero(builder, &inactive, &output);
    Ok(output)
}

fn modular_linear_reduction<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    terms: &[(&UintVar<F>, i64)],
    output_value: BigUint,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<UintVar<F>, P256AffineCompactFailureV1> {
    let output = builder.load_uint(output_value, witness)?;
    constrain_canonical(builder, &output, modulus, witness)?;
    let mut expression = RadixExpression::new();
    for (value, coefficient) in terms {
        expression.add_linear(value, Some(active), *coefficient);
    }
    expression.add_linear(&output, Some(active), -1);
    constrain_modular_expression(builder, &expression, modulus, active, witness)?;
    let inactive = builder.bool_not(active);
    gate_uint_zero(builder, &inactive, &output);
    Ok(output)
}

fn constrain_single_reduction<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    raw: &UintVar<F>,
    reduced: &UintVar<F>,
    modulus: &BigUint,
    reduce_value: bool,
) -> BoolVar<F> {
    let reduce = builder.boolean(reduce_value);
    let modulus_limbs = decompose_limbs(modulus);
    let radix = radix();
    let mut carry = builder.constant_bool(false);
    for index in 0..LIMBS {
        let modulus_part = builder.scale(reduce.cell(), modulus_limbs[index].clone());
        let sum = builder.add(reduced.limbs[index].cell, modulus_part);
        let with_carry = builder.add(sum, carry.cell());
        let integer_sum = &reduced.limbs[index].integer
            + if reduce_value {
                modulus_limbs[index].clone()
            } else {
                BigUint::from(0_u8)
            }
            + &carry.bounded.integer;
        let next = builder.boolean(integer_sum >= radix);
        let radix_next = builder.scale(next.cell(), radix.clone());
        let expected = builder.add(raw.limbs[index].cell, radix_next);
        builder.assert_equal(with_carry, expected);
        carry = next;
    }
    builder.assert_zero(carry.cell());
    reduce
}

#[derive(Clone, Debug)]
struct AffineVar<F> {
    x: UintVar<F>,
    y: UintVar<F>,
    infinity: BoolVar<F>,
    value: AffineValue,
}

fn identity_var<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
) -> Result<AffineVar<F>, P256AffineCompactFailureV1> {
    let zero = builder.load_uint(BigUint::from(0_u8), "identity coordinate")?;
    Ok(AffineVar {
        x: zero.clone(),
        y: zero,
        infinity: builder.constant_bool(true),
        value: AffineValue::identity(),
    })
}

fn constrain_identity_coordinates<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    point: &AffineVar<F>,
) {
    gate_uint_zero(builder, &point.infinity, &point.x);
    gate_uint_zero(builder, &point.infinity, &point.y);
}

fn select_uint<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    left: &UintVar<F>,
    bit: &BoolVar<F>,
    right: &UintVar<F>,
) -> UintVar<F> {
    let limbs = std::array::from_fn(|index| BoundedCell {
        cell: builder.select(left.limbs[index].cell, bit, right.limbs[index].cell),
        integer: if bit.value() {
            right.limbs[index].integer.clone()
        } else {
            left.limbs[index].integer.clone()
        },
    });
    UintVar {
        value: if bit.value() {
            right.value.clone()
        } else {
            left.value.clone()
        },
        limbs,
    }
}

fn complete_double<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    point: &AffineVar<F>,
) -> Result<AffineVar<F>, P256AffineCompactFailureV1> {
    builder.complete_doublings += 1;
    let modulus = modulus_base();
    let active = builder.bool_not(&point.infinity);
    let (value, lambda_value) = affine_double_value(&point.value);
    let lambda_value = if active.value() {
        lambda_value
    } else {
        BigUint::from(0_u8)
    };
    let x_value = if active.value() {
        value.x.clone()
    } else {
        BigUint::from(0_u8)
    };
    let y_value = if active.value() {
        value.y.clone()
    } else {
        BigUint::from(0_u8)
    };
    let lambda = builder.load_uint(lambda_value, "doubling lambda")?;
    let x = builder.load_uint(x_value, "doubling x")?;
    let y = builder.load_uint(y_value, "doubling y")?;
    for (candidate, label) in [
        (&lambda, "doubling lambda slack"),
        (&x, "doubling x slack"),
        (&y, "doubling y slack"),
    ] {
        constrain_canonical(builder, candidate, &modulus, label)?;
    }
    let inactive = builder.bool_not(&active);
    gate_uint_zero(builder, &inactive, &lambda);
    gate_uint_zero(builder, &inactive, &x);
    gate_uint_zero(builder, &inactive, &y);

    let mut slope = RadixExpression::new();
    slope.add_product(&lambda, &point.y, Some(&active), 2);
    slope.add_product(&point.x, &point.x, Some(&active), -3);
    slope.add_small_gated_constant(3, Some(&active), 1);
    constrain_modular_expression(
        builder,
        &slope,
        &modulus,
        &active,
        "doubling slope quotient",
    )?;
    let mut x_relation = RadixExpression::new();
    x_relation.add_product(&lambda, &lambda, Some(&active), 1);
    x_relation.add_linear(&point.x, Some(&active), -2);
    x_relation.add_linear(&x, Some(&active), -1);
    constrain_modular_expression(
        builder,
        &x_relation,
        &modulus,
        &active,
        "doubling x quotient",
    )?;
    let mut y_relation = RadixExpression::new();
    y_relation.add_product(&lambda, &point.x, Some(&active), 1);
    y_relation.add_product(&lambda, &x, Some(&active), -1);
    y_relation.add_linear(&point.y, Some(&active), -1);
    y_relation.add_linear(&y, Some(&active), -1);
    constrain_modular_expression(
        builder,
        &y_relation,
        &modulus,
        &active,
        "doubling y quotient",
    )?;
    let output = AffineVar {
        x,
        y,
        infinity: point.infinity.clone(),
        value,
    };
    constrain_identity_coordinates(builder, &output);
    Ok(output)
}

fn zero_uint<F: BigPrimeField>(builder: &mut TraceBuilder<F>) -> UintVar<F> {
    let zero = builder.zero();
    UintVar {
        limbs: std::array::from_fn(|_| BoundedCell {
            cell: zero,
            integer: BigUint::from(0_u8),
        }),
        value: BigUint::from(0_u8),
    }
}

fn complete_add<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    left: &AffineVar<F>,
    right: &AffineVar<F>,
) -> Result<AffineVar<F>, P256AffineCompactFailureV1> {
    builder.complete_additions += 1;
    let modulus = modulus_base();
    let x_equal = uint_equal(builder, &left.x, &right.x);
    let y_equal = uint_equal(builder, &left.y, &right.y);
    let left_finite = builder.bool_not(&left.infinity);
    let right_finite = builder.bool_not(&right.infinity);
    let finite = builder.bool_and(&left_finite, &right_finite);

    let y_sum_value = if finite.value() {
        (&left.y.value + &right.y.value) % &modulus
    } else {
        BigUint::from(0_u8)
    };
    let y_sum = modular_linear_reduction(
        builder,
        &[(&left.y, 1), (&right.y, 1)],
        y_sum_value,
        &modulus,
        &finite,
        "complete-add y-sum",
    )?;
    let y_negative = uint_is_zero(builder, &y_sum);
    let x_unequal = builder.bool_not(&x_equal);
    let y_not_negative = builder.bool_not(&y_negative);
    let chord = builder.bool_and(&finite, &x_unequal);
    let equal_xy = builder.bool_and(&x_equal, &y_equal);
    let tangent_candidate = builder.bool_and(&finite, &equal_xy);
    let tangent = builder.bool_and(&tangent_candidate, &y_not_negative);
    let same_x_negative = builder.bool_and(&x_equal, &y_negative);
    let opposite = builder.bool_and(&finite, &same_x_negative);
    let active = builder.bool_or_exclusive(&chord, &tangent);

    let take_left = builder.bool_and(&left_finite, &right.infinity);
    let take_right = builder.bool_and(&left.infinity, &right_finite);
    let both_identity = builder.bool_and(&left.infinity, &right.infinity);
    let branch_sum = [
        &chord,
        &tangent,
        &opposite,
        &take_left,
        &take_right,
        &both_identity,
    ]
    .into_iter()
    .fold(builder.zero(), |sum, branch| {
        builder.add(sum, branch.cell())
    });
    let one = builder.one();
    builder.assert_equal(branch_sum, one);

    let (host_output, host_lambda) = affine_add_value(&left.value, &right.value);
    let lambda_value = if active.value() {
        host_lambda
    } else {
        BigUint::from(0_u8)
    };
    let candidate_x_value = if active.value() {
        host_output.x.clone()
    } else {
        BigUint::from(0_u8)
    };
    let candidate_y_value = if active.value() {
        host_output.y.clone()
    } else {
        BigUint::from(0_u8)
    };
    let lambda = builder.load_uint(lambda_value, "complete-add lambda")?;
    let candidate_x = builder.load_uint(candidate_x_value, "complete-add x")?;
    let candidate_y = builder.load_uint(candidate_y_value, "complete-add y")?;
    for (candidate, label) in [
        (&lambda, "complete-add lambda slack"),
        (&candidate_x, "complete-add x slack"),
        (&candidate_y, "complete-add y slack"),
    ] {
        constrain_canonical(builder, candidate, &modulus, label)?;
    }
    let inactive = builder.bool_not(&active);
    gate_uint_zero(builder, &inactive, &lambda);
    gate_uint_zero(builder, &inactive, &candidate_x);
    gate_uint_zero(builder, &inactive, &candidate_y);

    let mut slope = RadixExpression::new();
    slope.add_product(&lambda, &right.x, Some(&chord), 1);
    slope.add_product(&lambda, &left.x, Some(&chord), -1);
    slope.add_linear(&right.y, Some(&chord), -1);
    slope.add_linear(&left.y, Some(&chord), 1);
    slope.add_product(&lambda, &left.y, Some(&tangent), 2);
    slope.add_product(&left.x, &left.x, Some(&tangent), -3);
    slope.add_small_gated_constant(3, Some(&tangent), 1);
    constrain_modular_expression(
        builder,
        &slope,
        &modulus,
        &active,
        "complete-add slope quotient",
    )?;
    let mut x_relation = RadixExpression::new();
    x_relation.add_product(&lambda, &lambda, Some(&active), 1);
    x_relation.add_linear(&left.x, Some(&active), -1);
    x_relation.add_linear(&right.x, Some(&active), -1);
    x_relation.add_linear(&candidate_x, Some(&active), -1);
    constrain_modular_expression(
        builder,
        &x_relation,
        &modulus,
        &active,
        "complete-add x quotient",
    )?;
    let mut y_relation = RadixExpression::new();
    y_relation.add_product(&lambda, &left.x, Some(&active), 1);
    y_relation.add_product(&lambda, &candidate_x, Some(&active), -1);
    y_relation.add_linear(&left.y, Some(&active), -1);
    y_relation.add_linear(&candidate_y, Some(&active), -1);
    constrain_modular_expression(
        builder,
        &y_relation,
        &modulus,
        &active,
        "complete-add y quotient",
    )?;

    let zero = zero_uint(builder);
    let selected_left = select_uint(builder, &zero, &take_left, &left.x);
    let selected_right = select_uint(builder, &selected_left, &take_right, &right.x);
    let x = select_uint(builder, &selected_right, &active, &candidate_x);
    let selected_left = select_uint(builder, &zero, &take_left, &left.y);
    let selected_right = select_uint(builder, &selected_left, &take_right, &right.y);
    let y = select_uint(builder, &selected_right, &active, &candidate_y);
    let infinity = builder.bool_or_exclusive(&opposite, &both_identity);
    let output = AffineVar {
        x,
        y,
        infinity,
        value: host_output,
    };
    constrain_identity_coordinates(builder, &output);
    Ok(output)
}

fn variable_window_table<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    point: &AffineVar<F>,
) -> Result<[AffineVar<F>; 16], P256AffineCompactFailureV1> {
    let mut table = std::array::from_fn(|_| None);
    table[0] = Some(identity_var(builder)?);
    table[1] = Some(point.clone());
    for multiple in 2_usize..16 {
        let value = if multiple % 2 == 0 {
            complete_double(
                builder,
                table[multiple / 2]
                    .as_ref()
                    .expect("lower even multiple is initialized"),
            )?
        } else {
            complete_add(
                builder,
                table[multiple - 1]
                    .as_ref()
                    .expect("previous multiple is initialized"),
                point,
            )?
        };
        table[multiple] = Some(value);
    }
    Ok(table.map(|entry| entry.expect("all sixteen window entries are initialized")))
}

fn select_bounded_component<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    mut level: Vec<BoundedCell<F>>,
    bits_lsb: &[BoolVar<F>; 4],
) -> BoundedCell<F> {
    for bit in bits_lsb {
        level = level
            .chunks_exact(2)
            .map(|pair| BoundedCell {
                cell: builder.select(pair[0].cell, bit, pair[1].cell),
                integer: if bit.value() {
                    pair[1].integer.clone()
                } else {
                    pair[0].integer.clone()
                },
            })
            .collect();
    }
    level.pop().expect("four-bit table is non-empty")
}

fn window_is_zero<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    bits_lsb: &[BoolVar<F>; 4],
) -> BoolVar<F> {
    let mut identity = builder.constant_bool(true);
    for bit in bits_lsb {
        let not_bit = builder.bool_not(bit);
        identity = builder.bool_and(&identity, &not_bit);
    }
    identity
}

fn select_variable_window<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    table: &[AffineVar<F>; 16],
    bits_lsb: &[BoolVar<F>; 4],
) -> AffineVar<F> {
    let x_limbs: [BoundedCell<F>; LIMBS] = std::array::from_fn(|limb| {
        select_bounded_component(
            builder,
            table
                .iter()
                .map(|point| point.x.limbs[limb].clone())
                .collect(),
            bits_lsb,
        )
    });
    let y_limbs: [BoundedCell<F>; LIMBS] = std::array::from_fn(|limb| {
        select_bounded_component(
            builder,
            table
                .iter()
                .map(|point| point.y.limbs[limb].clone())
                .collect(),
            bits_lsb,
        )
    });
    let identity = window_is_zero(builder, bits_lsb);
    let digit = bits_lsb
        .iter()
        .enumerate()
        .fold(0_usize, |value, (bit, enabled)| {
            value | (usize::from(enabled.value()) << bit)
        });
    AffineVar {
        x: UintVar {
            value: compose_limbs(&x_limbs.clone().map(|limb| limb.integer)),
            limbs: x_limbs,
        },
        y: UintVar {
            value: compose_limbs(&y_limbs.clone().map(|limb| limb.integer)),
            limbs: y_limbs,
        },
        infinity: identity,
        value: table[digit].value.clone(),
    }
}

fn select_fixed_window<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    bits_lsb: &[BoolVar<F>; 4],
) -> Result<AffineVar<F>, P256AffineCompactFailureV1> {
    let digit = bits_lsb
        .iter()
        .enumerate()
        .fold(0_usize, |value, (bit, enabled)| {
            value | (usize::from(enabled.value()) << bit)
        });
    let values = fixed_generator_values();
    let value = values[digit].clone();
    let x_limbs: [BoundedCell<F>; LIMBS] = std::array::from_fn(|limb| {
        let level = values
            .iter()
            .map(|point| {
                let integer = decompose_limbs(&point.x)[limb].clone();
                BoundedCell {
                    cell: builder.constant(integer.clone()),
                    integer,
                }
            })
            .collect();
        select_bounded_component(builder, level, bits_lsb)
    });
    let y_limbs: [BoundedCell<F>; LIMBS] = std::array::from_fn(|limb| {
        let level = values
            .iter()
            .map(|point| {
                let integer = decompose_limbs(&point.y)[limb].clone();
                BoundedCell {
                    cell: builder.constant(integer.clone()),
                    integer,
                }
            })
            .collect();
        select_bounded_component(builder, level, bits_lsb)
    });
    let identity = window_is_zero(builder, bits_lsb);
    Ok(AffineVar {
        x: UintVar {
            value: compose_limbs(&x_limbs.clone().map(|limb| limb.integer)),
            limbs: x_limbs,
        },
        y: UintVar {
            value: compose_limbs(&y_limbs.clone().map(|limb| limb.integer)),
            limbs: y_limbs,
        },
        infinity: identity,
        value,
    })
}

fn scalar_bits<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    scalar: &UintVar<F>,
) -> [BoolVar<F>; 256] {
    let mut bits = Vec::with_capacity(256);
    for (limb_index, limb) in scalar.limbs.iter().enumerate() {
        let limb_bits = if limb_index + 1 == LIMBS {
            84
        } else {
            LIMB_BITS
        };
        let mut accumulator = builder.zero();
        for bit in 0..limb_bits {
            let enabled = ((&limb.integer >> bit) & BigUint::from(1_u8)) == BigUint::from(1_u8);
            let bit_var = builder.boolean(enabled);
            let power = builder.constant(BigUint::from(1_u8) << bit);
            accumulator = builder.fma(accumulator, bit_var.cell(), power);
            bits.push(bit_var);
        }
        builder.assert_equal(accumulator, limb.cell);
    }
    bits.try_into()
        .unwrap_or_else(|_| panic!("three 86-bit limbs expose exactly 256 bits"))
}

fn straus_two_scalar<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    public_key: &AffineVar<F>,
    u1: &UintVar<F>,
    u2: &UintVar<F>,
) -> Result<AffineVar<F>, P256AffineCompactFailureV1> {
    let variable_table = variable_window_table(builder, public_key)?;
    let u1_bits = scalar_bits(builder, u1);
    let u2_bits = scalar_bits(builder, u2);
    let mut accumulator = identity_var(builder)?;
    for window in (0..WINDOWS).rev() {
        for _ in 0..WINDOW_BITS {
            accumulator = complete_double(builder, &accumulator)?;
        }
        let start = window * WINDOW_BITS;
        let fixed_bits: [BoolVar<F>; 4] = std::array::from_fn(|bit| u1_bits[start + bit].clone());
        let variable_bits: [BoolVar<F>; 4] =
            std::array::from_fn(|bit| u2_bits[start + bit].clone());
        let fixed = select_fixed_window(builder, &fixed_bits)?;
        let variable = select_variable_window(builder, &variable_table, &variable_bits);
        accumulator = complete_add(builder, &accumulator, &fixed)?;
        accumulator = complete_add(builder, &accumulator, &variable)?;
    }
    Ok(accumulator)
}

fn constrain_on_curve<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    point: &AffineVar<F>,
) -> Result<BoolVar<F>, P256AffineCompactFailureV1> {
    let modulus = modulus_base();
    let active = builder.constant_bool(true);
    let x_squared_value = (&point.x.value * &point.x.value) % &modulus;
    let x_squared = modular_product(
        builder,
        &point.x,
        &point.x,
        x_squared_value,
        &modulus,
        &active,
        "on-curve x-squared",
    )?;
    let mut equation = RadixExpression::new();
    equation.add_product(&point.y, &point.y, None, 1);
    equation.add_product(&x_squared, &point.x, None, -1);
    equation.add_linear(&point.x, None, 3);
    equation.add_constant(&curve_b(), -1);
    constrain_modular_expression(builder, &equation, &modulus, &active, "on-curve quotient")?;
    let x_zero = uint_is_zero(builder, &point.x);
    let y_zero = uint_is_zero(builder, &point.y);
    let both_zero = builder.bool_and(&x_zero, &y_zero);
    let nonidentity = builder.bool_not(&both_zero);
    let one = builder.one();
    builder.assert_equal(nonidentity.cell(), one);
    Ok(nonidentity)
}

fn constrain_scalar_product<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
    output: &UintVar<F>,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<(), P256AffineCompactFailureV1> {
    let mut expression = RadixExpression::new();
    expression.add_product(left, right, Some(active), 1);
    expression.add_linear(output, Some(active), -1);
    constrain_modular_expression(builder, &expression, &modulus_scalar(), active, witness)
}

fn constrain_ecdsa<F: BigPrimeField>(
    builder: &mut TraceBuilder<F>,
    input: &[u8; PUBLIC_BYTES],
    sec1: &[u8; 65],
    digest: &[u8; 32],
    signature: &[u8; 64],
) -> Result<(), P256AffineCompactFailureV1> {
    let public = input
        .iter()
        .map(|byte| {
            let cell = builder.caller_instance(*byte);
            builder.range_bounded(&cell, 8, "public byte")?;
            Ok(cell)
        })
        .collect::<Result<Vec<_>, P256AffineCompactFailureV1>>()?;
    let prefix_four = builder.constant(4_u8);
    let prefix_difference = builder.subtract(public[0].cell, prefix_four);
    let prefix_valid = builder.is_zero_cell(prefix_difference, input[0] == 4);

    let x_bytes: &[BoundedCell<F>; 32] = public[1..33].try_into().expect("fixed SEC1 x byte count");
    let y_bytes: &[BoundedCell<F>; 32] =
        public[33..65].try_into().expect("fixed SEC1 y byte count");
    let digest_bytes: &[BoundedCell<F>; 32] =
        public[65..97].try_into().expect("fixed digest byte count");
    let r_bytes: &[BoundedCell<F>; 32] = public[97..129].try_into().expect("fixed r byte count");
    let s_bytes: &[BoundedCell<F>; 32] = public[129..161].try_into().expect("fixed s byte count");
    let x_raw: [u8; 32] = sec1[1..33].try_into().expect("fixed SEC1 x");
    let y_raw: [u8; 32] = sec1[33..65].try_into().expect("fixed SEC1 y");
    let r_raw: [u8; 32] = signature[..32].try_into().expect("fixed signature r");
    let s_raw: [u8; 32] = signature[32..].try_into().expect("fixed signature s");
    let x = bind_public_uint(builder, x_bytes, &x_raw, "SEC1 x")?;
    let y = bind_public_uint(builder, y_bytes, &y_raw, "SEC1 y")?;
    let digest_uint = bind_public_uint(builder, digest_bytes, digest, "SHA-256 digest")?;
    let r = bind_public_uint(builder, r_bytes, &r_raw, "signature r")?;
    let s = bind_public_uint(builder, s_bytes, &s_raw, "signature s")?;

    let base_modulus = modulus_base();
    let scalar_modulus = modulus_scalar();
    constrain_canonical(builder, &x, &base_modulus, "SEC1 x canonical slack")?;
    constrain_canonical(builder, &y, &base_modulus, "SEC1 y canonical slack")?;
    constrain_canonical(builder, &r, &scalar_modulus, "r canonical slack")?;
    constrain_canonical(builder, &s, &scalar_modulus, "s canonical slack")?;
    let public_key = AffineVar {
        x,
        y,
        infinity: builder.constant_bool(false),
        value: AffineValue {
            x: BigUint::from_bytes_be(&x_raw),
            y: BigUint::from_bytes_be(&y_raw),
            infinity: false,
        },
    };
    let key_nonidentity = constrain_on_curve(builder, &public_key)?;
    let r_zero = uint_is_zero(builder, &r);
    let s_zero = uint_is_zero(builder, &s);
    let r_nonzero = builder.bool_not(&r_zero);
    let s_nonzero = builder.bool_not(&s_zero);
    let low_s = constrain_at_most(builder, &s, &(&scalar_modulus >> 1_usize), "low-S slack")?;

    let digest_integer = BigUint::from_bytes_be(digest);
    let z_value = &digest_integer % &scalar_modulus;
    let z = builder.load_uint(z_value, "reduced digest")?;
    constrain_canonical(builder, &z, &scalar_modulus, "digest canonical slack")?;
    constrain_single_reduction(
        builder,
        &digest_uint,
        &z,
        &scalar_modulus,
        digest_integer >= scalar_modulus,
    );

    let active = builder.constant_bool(true);
    let s_inverse_value = modular_inverse(&s.value, &scalar_modulus);
    let s_inverse = builder.load_uint(s_inverse_value, "scalar inverse")?;
    constrain_canonical(
        builder,
        &s_inverse,
        &scalar_modulus,
        "scalar inverse canonical slack",
    )?;
    let one_uint = builder.load_uint(BigUint::from(1_u8), "scalar one")?;
    constrain_canonical(builder, &one_uint, &scalar_modulus, "scalar one slack")?;
    constrain_scalar_product(
        builder,
        &s,
        &s_inverse,
        &one_uint,
        &active,
        "scalar inverse quotient",
    )?;
    let u1_value = (&z.value * &s_inverse.value) % &scalar_modulus;
    let u2_value = (&r.value * &s_inverse.value) % &scalar_modulus;
    let u1 = modular_product(
        builder,
        &z,
        &s_inverse,
        u1_value,
        &scalar_modulus,
        &active,
        "u1 scalar product",
    )?;
    let u2 = modular_product(
        builder,
        &r,
        &s_inverse,
        u2_value,
        &scalar_modulus,
        &active,
        "u2 scalar product",
    )?;
    let result_point = straus_two_scalar(builder, &public_key, &u1, &u2)?;
    let result_nonidentity = builder.bool_not(&result_point.infinity);
    let x_mod_n_value = &result_point.x.value % &scalar_modulus;
    let x_mod_n = builder.load_uint(x_mod_n_value, "result x modulo n")?;
    constrain_canonical(
        builder,
        &x_mod_n,
        &scalar_modulus,
        "result x canonical slack",
    )?;
    constrain_single_reduction(
        builder,
        &result_point.x,
        &x_mod_n,
        &scalar_modulus,
        result_point.x.value >= scalar_modulus,
    );
    let r_matches = uint_equal(builder, &x_mod_n, &r);
    let result = [
        prefix_valid,
        key_nonidentity,
        r_nonzero,
        s_nonzero,
        low_s,
        result_nonidentity,
        r_matches,
    ]
    .into_iter()
    .reduce(|left, right| builder.bool_and(&left, &right))
    .expect("ECDSA result conjunction is non-empty");
    let one = builder.one();
    builder.assert_equal(result.cell(), one);
    Ok(())
}

#[cfg(test)]
#[path = "p256_affine_compact_tests.rs"]
mod tests;
