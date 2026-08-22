//! Private, non-authorizing packed affine P-256 ECDSA prototype.
//!
//! This file is intentionally undeclared. It is pre-settlement evidence for a
//! fail-closed k=16 lower-bound artifact, not a backend or GuardBundle
//! eligibility path. The circuit keeps the reviewed current-query proof shape (eight equality
//! advice columns, one direct instance column, four fixed queries, two lookup
//! arguments, degree seven, and 3,200 augmented IPA bytes) while replacing the
//! row-infeasible recursive-FMA transpose with a packed row machine.
//!
//! Range lookups use the field-sound tuple
//! `(s * value, s^2 * value) in {(t * u, t^2 * u)}`. Every active width has a
//! distinct, fixed, nonzero tag. If a query with tag `s` matches a table row
//! with tag `t`, then `t*u*(s-t)=0`; hence a nonzero table value forces `s=t`
//! and then `value=u`. A zero table value forces `value=0`. Tag zero is used
//! only for the explicit `(0, 0)` disabled sentinel. This two-coordinate
//! argument rules out the fractional cross-kind attack on one-coordinate
//! encodings.
//!
//! Inputs are exactly `[SEC1-uncompressed key; SHA-256 prehash; P1363 r||s]`.
//! SHA-256 and DER parsing remain separate governed children. All constants,
//! including opcode overlap seeds and fixed-generator coordinates, are bound
//! through the verifier-derived instance tail. Complete affine exceptional
//! cases, canonical coordinates, low-S, digest/x single reduction, inactive
//! witness zeroization, and an exact row-cap diagnostic are part of this
//! prototype.

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
use zeroize::Zeroizing;

const K: u32 = 16;
const ADVICE_COLUMNS: usize = 8;
const PUBLIC_BYTES: usize = 65 + 32 + 64;
const LOOKUP_BITS: usize = 15;
const RANGE_CHUNK_BITS: [usize; 6] = [2, 4, 6, 8, 11, 15];
const TABLE_ROWS: usize = 1 + (1 << 2) + (1 << 4) + (1 << 6) + (1 << 8) + (1 << 11) + (1 << 15);
const LIMB_BITS: usize = 86;
const LIMBS: usize = 3;
const QUOTIENT_BITS: usize = 3 * LIMB_BITS;
const CARRY_BITS: usize = 90;
// A coefficient accumulator contains at most nine gated 86-by-86-bit
// products, three 86-by-86-bit quotient products, and one 86-by-90-bit
// radix-carry product. Including signs and additions keeps its absolute value
// below 2^180. This witness-independent bound leaves more than 70 bits of
// headroom in either Pasta scalar field.
const PACKED_COEFFICIENT_BOUND_BITS: usize = 180;
const WINDOW_BITS: usize = 4;
const WINDOWS: usize = 256 / WINDOW_BITS;
const K16_MAX_ASSIGNED_ROWS: usize = (1 << K) - 9;

// Exact source-topology lower bound, before any Dense, Boolean-Sign,
// constant-tail Bind, or lookup-only rows. The disjoint mandatory categories
// alone exceed the k=16 usable-row cap, so V2 is permanently non-authorizing.
const V2_MINIMUM_CUBIC_SPARSE_ROWS: usize = 31_122;
const V2_MINIMUM_RANGE_ROWS: usize = 17_559;
const V2_MINIMUM_QUOTIENT_CARRY_SIGN_ROWS: usize = 8_004;
const V2_MINIMUM_SELECTION_ROWS: usize = 6_975;
const V2_MINIMUM_CANONICAL_WIDE_ROWS: usize = 4_020;
const V2_MINIMUM_CALLER_BIND_ROWS: usize = PUBLIC_BYTES;
const P256_PACKED_AFFINE_V2_STATIC_MINIMUM_ROWS: usize = V2_MINIMUM_CUBIC_SPARSE_ROWS
    + V2_MINIMUM_RANGE_ROWS
    + V2_MINIMUM_QUOTIENT_CARRY_SIGN_ROWS
    + V2_MINIMUM_SELECTION_ROWS
    + V2_MINIMUM_CANONICAL_WIDE_ROWS
    + V2_MINIMUM_CALLER_BIND_ROWS;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct P256PackedAffineShapeV2 {
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
    pub(super) proof_points: usize,
    pub(super) proof_scalars: usize,
    pub(super) raw_proof_bytes: usize,
    pub(super) augmented_proof_bytes: usize,
}

pub(super) const P256_PACKED_AFFINE_SHAPE_V2: P256PackedAffineShapeV2 = P256PackedAffineShapeV2 {
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
    proof_points: 57,
    proof_scalars: 42,
    raw_proof_bytes: 3_168,
    augmented_proof_bytes: 3_200,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct P256PackedAffineRowsV2 {
    pub(super) binding_rows: usize,
    pub(super) caller_instance_rows: usize,
    pub(super) constant_instance_rows: usize,
    pub(super) range_rows: usize,
    pub(super) sparse_rows: usize,
    pub(super) dense_rows: usize,
    pub(super) wide_rows: usize,
    pub(super) sign_rows: usize,
    pub(super) selection_rows: usize,
    pub(super) lookup_only_rows: usize,
    pub(super) semantic_rows: usize,
    pub(super) table_rows: usize,
    pub(super) total_rows: usize,
    pub(super) range_lookups: usize,
    pub(super) modular_relations: usize,
    pub(super) complete_doublings: usize,
    pub(super) complete_additions: usize,
    pub(super) canonical_checks: usize,
    pub(super) zero_tests: usize,
    pub(super) maximum_quotient_bits: usize,
    pub(super) maximum_carry_bits: usize,
    pub(super) maximum_coefficient_bits: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum P256PackedAffineFailureV2 {
    Source(&'static str),
    IntegerBound {
        witness: &'static str,
        actual_bits: usize,
        maximum_bits: usize,
    },
    UnsafeNativeCoefficient {
        witness: &'static str,
        bits: usize,
    },
    MissingCell {
        variable: usize,
    },
    RowCapacityExceeded {
        rows: Box<P256PackedAffineRowsV2>,
        maximum: usize,
    },
    InstanceBindingMismatch {
        instances: usize,
        bindings: usize,
    },
}

/// Move-only exact-statement source. Implementations must either fill all 161
/// bytes or fail; there is no truncating or retrying parser path.
pub(super) trait P256PackedStatementSourceV2 {
    fn read_exact_statement(
        &mut self,
        destination: &mut [u8; PUBLIC_BYTES],
    ) -> Result<(), &'static str>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct P256PackedAffineEcdsaCircuitV2<F> {
    sec1_uncompressed: [u8; 65],
    digest: [u8; 32],
    signature: [u8; 64],
    _field: PhantomData<F>,
}

impl<F> Default for P256PackedAffineEcdsaCircuitV2<F> {
    fn default() -> Self {
        Self {
            sec1_uncompressed: [0; 65],
            digest: [0; 32],
            signature: [0; 64],
            _field: PhantomData,
        }
    }
}

impl<F: BigPrimeField> P256PackedAffineEcdsaCircuitV2<F> {
    pub(super) fn new(sec1_uncompressed: [u8; 65], digest: [u8; 32], signature: [u8; 64]) -> Self {
        Self {
            sec1_uncompressed,
            digest,
            signature,
            _field: PhantomData,
        }
    }

    pub(super) fn from_source(mut source: impl P256PackedStatementSourceV2) -> Result<Self, Error> {
        let mut statement = Zeroizing::new([0_u8; PUBLIC_BYTES]);
        source
            .read_exact_statement(&mut statement)
            .map_err(|_| Error::Synthesis)?;
        let mut sec1 = [0_u8; 65];
        let mut digest = [0_u8; 32];
        let mut signature = [0_u8; 64];
        sec1.copy_from_slice(&statement[..65]);
        digest.copy_from_slice(&statement[65..97]);
        signature.copy_from_slice(&statement[97..]);
        Ok(Self::new(sec1, digest, signature))
    }

    fn input_bytes(&self) -> [u8; PUBLIC_BYTES] {
        let mut statement = [0_u8; PUBLIC_BYTES];
        statement[..65].copy_from_slice(&self.sec1_uncompressed);
        statement[65..97].copy_from_slice(&self.digest);
        statement[97..].copy_from_slice(&self.signature);
        statement
    }

    pub(super) fn instances(&self) -> Result<Vec<F>, Error> {
        self.build_trace().map(|trace| trace.instances)
    }

    pub(super) fn row_report(&self) -> Result<P256PackedAffineRowsV2, Error> {
        self.build_trace().map(|trace| trace.rows)
    }

    #[cfg(test)]
    fn trace_diagnostic_for_test(
        &self,
    ) -> Result<P256PackedAffineRowsV2, P256PackedAffineFailureV2> {
        match self.build_trace_diagnostic() {
            Ok(trace) => Ok(trace.rows),
            Err(P256PackedAffineFailureV2::RowCapacityExceeded { rows, .. }) => Ok(*rows),
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    fn instance_partition_for_test(&self) -> Result<(Vec<F>, Vec<F>), P256PackedAffineFailureV2> {
        let mut builder = self.build_builder_diagnostic()?;
        ensure_layout_constants(&mut builder);
        Ok((
            builder
                .caller_instances
                .iter()
                .map(|cell| cell.value)
                .collect(),
            builder
                .constant_instances
                .iter()
                .map(|cell| cell.value)
                .collect(),
        ))
    }

    fn build_trace(&self) -> Result<PackedTrace<F>, Error> {
        self.build_trace_diagnostic().map_err(|_| Error::Synthesis)
    }

    fn build_trace_diagnostic(&self) -> Result<PackedTrace<F>, P256PackedAffineFailureV2> {
        self.build_builder_diagnostic()?.finish()
    }

    fn build_builder_diagnostic(&self) -> Result<PackedBuilder<F>, P256PackedAffineFailureV2> {
        let mut builder = PackedBuilder::new();
        constrain_ecdsa(
            &mut builder,
            &self.input_bytes(),
            &self.sec1_uncompressed,
            &self.digest,
            &self.signature,
        )?;
        Ok(builder)
    }
}

#[derive(Clone, Debug)]
pub(super) struct P256PackedAffineConfigV2 {
    advice: [Column<Advice>; ADVICE_COLUMNS],
    instance: Column<Instance>,
    opcode: Column<Fixed>,
    range_tag: Column<Fixed>,
    table_first: Column<Fixed>,
    table_second: Column<Fixed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
enum Opcode {
    Disabled = 0,
    Bind = 1,
    Range = 2,
    Sparse = 3,
    Dense = 4,
    Select = 5,
    // Numeric opcode six is deliberately unused. Wide carry rows use the
    // Dense layout, which makes every active selector vanish at fixed-zero
    // unusable and blinding rows without raising the maximum degree.
    Sign = 7,
}

const Q_BIND_ROOTS: [u64; 6] = [0, 2, 3, 4, 5, 7];
const Q_SPARSE_ROOTS: [u64; 4] = [0, 2, 4, 5];
const Q_DENSE_ROOTS: [u64; 5] = [0, 2, 3, 5, 7];
const Q_SELECT_ROOTS: [u64; 5] = [0, 2, 3, 4, 7];
const Q_SIGN_ROOTS: [u64; 5] = [0, 2, 3, 4, 5];

fn roots_except<F: BigPrimeField>(opcode: Expression<F>, roots: &[u64]) -> Expression<F> {
    roots
        .iter()
        .fold(Expression::Constant(F::ONE), |value, root| {
            value * (opcode.clone() - Expression::Constant(F::from(*root)))
        })
}

impl P256PackedAffineConfigV2 {
    fn configure<F: BigPrimeField>(meta: &mut ConstraintSystem<F>) -> Self {
        let advice = std::array::from_fn(|_| {
            let column = meta.advice_column();
            meta.enable_equality(column);
            column
        });
        let instance = meta.instance_column();
        let opcode = meta.fixed_column();
        let range_tag = meta.fixed_column();
        let table_first = meta.fixed_column();
        let table_second = meta.fixed_column();

        meta.create_gate("packed affine P-256 current-row machine", |meta| {
            let v = advice.map(|column| meta.query_advice(column, Rotation::cur()));
            let public = meta.query_instance(instance, Rotation::cur());
            let op = meta.query_fixed(opcode, Rotation::cur());

            // Every selector has an explicit fixed-zero root. Numeric opcode
            // six is unused, allowing the lowest-degree overlap polynomials
            // below. On assigned rows their only deliberate overlaps are:
            //
            // * Bind: all semantic columns are zero (the instance is v7);
            // * Sign under q_sparse: sign*(-2)*m + m - signed = 0;
            // * Wide rows are encoded as Dense rows directly.
            //
            // q_range additionally vanishes on lookup-bearing Sparse rows by
            // its (opcode - 3) factor. Its tag factor makes it vanish on every
            // other semantic, padding, unusable, and blinding row.
            let q_bind = roots_except(op.clone(), &Q_BIND_ROOTS);
            let q_range = meta.query_fixed(range_tag, Rotation::cur())
                * (op.clone() - Expression::Constant(F::from(3)));
            let q_sparse = roots_except(op.clone(), &Q_SPARSE_ROOTS);
            let q_dense = roots_except(op.clone(), &Q_DENSE_ROOTS);
            let q_select = roots_except(op.clone(), &Q_SELECT_ROOTS);
            let q_sign = roots_except(op, &Q_SIGN_ROOTS);

            let range_recomposition = v[0].clone()
                + v[1].clone() * Expression::Constant(F::from(1_u64 << 15))
                + v[2].clone() * Expression::Constant(F::from(1_u64 << 30))
                + v[3].clone() * Expression::Constant(F::from(1_u64 << 45))
                + v[4].clone() * Expression::Constant(F::from(1_u64 << 60))
                + v[5].clone()
                    * Expression::Constant(biguint_to_fe::<F>(&(BigUint::from(1_u8) << 75_usize)))
                - v[6].clone();
            let sparse = v[3].clone() * v[1].clone() * v[2].clone() + v[6].clone() - v[5].clone();
            let dense = v[0].clone() * v[1].clone()
                + v[2].clone() * v[3].clone()
                + v[4].clone() * v[7].clone()
                + v[6].clone()
                - v[5].clone();
            let select_zero =
                v[0].clone() + v[6].clone() * (v[1].clone() - v[0].clone()) - v[2].clone();
            let select_one =
                v[3].clone() + v[6].clone() * (v[4].clone() - v[3].clone()) - v[5].clone();
            let sign_zero = v[5].clone()
                - v[2].clone()
                    * (Expression::Constant(F::ONE)
                        - Expression::Constant(F::from(2)) * v[3].clone());
            let sign_one = v[4].clone()
                - v[0].clone()
                    * (Expression::Constant(F::ONE)
                        - Expression::Constant(F::from(2)) * v[3].clone());

            vec![
                q_bind * (v[7].clone() - public),
                q_range.clone() * range_recomposition,
                q_range.clone() * (Expression::Constant(F::ONE) - v[7].clone()) * v[6].clone(),
                q_sparse * sparse,
                q_dense * dense,
                q_select.clone() * select_zero,
                q_select * select_one,
                q_sign.clone() * sign_zero,
                q_sign.clone() * sign_one,
                q_sign.clone() * v[3].clone() * (v[3].clone() - Expression::Constant(F::ONE)),
                q_sign.clone() * (Expression::Constant(F::ONE) - v[7].clone()) * v[2].clone(),
                q_sign.clone() * (Expression::Constant(F::ONE) - v[7].clone()) * v[0].clone(),
                q_sign * (Expression::Constant(F::ONE) - v[7].clone()) * v[3].clone(),
            ]
        });

        for (label, column) in [
            ("packed typed range lane zero", advice[0]),
            ("packed typed range lane one", advice[4]),
        ] {
            meta.lookup_any(label, |meta| {
                let tag = meta.query_fixed(range_tag, Rotation::cur());
                let value = meta.query_advice(column, Rotation::cur());
                let first = meta.query_fixed(table_first, Rotation::cur());
                let second = meta.query_fixed(table_second, Rotation::cur());
                vec![
                    (tag.clone() * value.clone(), first),
                    (tag.clone() * tag * value, second),
                ]
            });
        }

        meta.set_minimum_degree(P256_PACKED_AFFINE_SHAPE_V2.degree);
        Self {
            advice,
            instance,
            opcode,
            range_tag,
            table_first,
            table_second,
        }
    }
}

impl<F: BigPrimeField> Circuit<F> for P256PackedAffineEcdsaCircuitV2<F> {
    type Config = P256PackedAffineConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        P256PackedAffineConfigV2::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.build_trace()?.assign(&config, &mut layouter)
    }
}

#[derive(Clone, Copy, Debug)]
struct CellVar<F> {
    id: usize,
    value: F,
}

#[derive(Clone, Debug)]
struct RangeChunk<F> {
    cell: CellVar<F>,
    integer: BigUint,
    bits: usize,
}

#[derive(Clone, Debug)]
struct BoundedCell<F> {
    cell: CellVar<F>,
    integer: BigUint,
    chunks: Vec<RangeChunk<F>>,
    active: CellVar<F>,
}

#[derive(Clone, Debug)]
struct BoolVar<F> {
    cell: CellVar<F>,
    value: bool,
}

#[derive(Clone, Debug)]
struct UintVar<F> {
    limbs: [BoundedCell<F>; LIMBS],
    value: BigUint,
}

#[derive(Clone, Copy, Debug)]
struct SignLane<F> {
    magnitude: CellVar<F>,
    sign: CellVar<F>,
    signed: CellVar<F>,
    active: CellVar<F>,
}

#[derive(Clone, Copy, Debug)]
struct SelectLane<F> {
    left: CellVar<F>,
    bit: CellVar<F>,
    right: CellVar<F>,
    output: CellVar<F>,
}

#[derive(Clone, Debug)]
struct AssignedRow<F> {
    values: [F; ADVICE_COLUMNS],
    aliases: Vec<(usize, usize)>,
    opcode: Opcode,
    range_bits: usize,
}

impl<F: BigPrimeField> AssignedRow<F> {
    fn zero(opcode: Opcode) -> Self {
        Self {
            values: [F::ZERO; ADVICE_COLUMNS],
            aliases: Vec::new(),
            opcode,
            range_bits: 0,
        }
    }

    fn set(&mut self, column: usize, variable: CellVar<F>) {
        self.values[column] = variable.value;
        self.aliases.push((variable.id, column));
    }
}

#[derive(Clone, Debug)]
struct RangeRow<F> {
    bounded: BoundedCell<F>,
}

#[derive(Clone, Copy, Debug)]
struct SparseRow<F> {
    left: CellVar<F>,
    right: CellVar<F>,
    gate: CellVar<F>,
    accumulator: CellVar<F>,
    output: CellVar<F>,
}

#[derive(Clone, Copy, Debug)]
struct DenseRow<F> {
    products: [(CellVar<F>, CellVar<F>); 3],
    accumulator: CellVar<F>,
    output: CellVar<F>,
}

#[derive(Clone, Copy, Debug)]
struct WideRow<F> {
    left: CellVar<F>,
    right: CellVar<F>,
    carry_in: CellVar<F>,
    carry_out: CellVar<F>,
    constant: CellVar<F>,
}

#[derive(Clone, Debug)]
struct PolyFactor<F> {
    cell: Option<CellVar<F>>,
    integer: BigInt,
}

impl<F: Copy> From<&BoundedCell<F>> for PolyFactor<F> {
    fn from(value: &BoundedCell<F>) -> Self {
        Self {
            cell: Some(value.cell),
            integer: bigint(&value.integer),
        }
    }
}

impl<F: Copy> From<&BoolVar<F>> for PolyFactor<F> {
    fn from(value: &BoolVar<F>) -> Self {
        Self {
            cell: Some(value.cell),
            integer: BigInt::from(u8::from(value.value)),
        }
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
                value * &factor.integer
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
                    integer: bigint(&limb),
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

#[derive(Clone, Debug)]
struct RealizedTerm<F> {
    factors: Vec<CellVar<F>>,
    integer: BigInt,
    negative: bool,
}

#[derive(Clone, Debug)]
struct PackedBuilder<F> {
    next_id: usize,
    caller_instances: Vec<CellVar<F>>,
    constant_instances: Vec<CellVar<F>>,
    constants: HashMap<BigInt, CellVar<F>>,
    range_rows: Vec<RangeRow<F>>,
    sparse_rows: Vec<SparseRow<F>>,
    dense_rows: Vec<DenseRow<F>>,
    wide_rows: Vec<WideRow<F>>,
    sign_lanes: Vec<SignLane<F>>,
    selects: Vec<SelectLane<F>>,
    modular_relations: usize,
    complete_doublings: usize,
    complete_additions: usize,
    canonical_checks: usize,
    zero_tests: usize,
    maximum_quotient_bits: usize,
    maximum_carry_bits: usize,
    maximum_coefficient_bits: usize,
}

impl<F: BigPrimeField> PackedBuilder<F> {
    fn new() -> Self {
        Self {
            next_id: 0,
            caller_instances: Vec::new(),
            constant_instances: Vec::new(),
            constants: HashMap::new(),
            range_rows: Vec::new(),
            sparse_rows: Vec::new(),
            dense_rows: Vec::new(),
            wide_rows: Vec::new(),
            sign_lanes: Vec::new(),
            selects: Vec::new(),
            modular_relations: 0,
            complete_doublings: 0,
            complete_additions: 0,
            canonical_checks: 0,
            zero_tests: 0,
            maximum_quotient_bits: 0,
            maximum_carry_bits: 0,
            maximum_coefficient_bits: 0,
        }
    }

    fn witness_fe(&mut self, value: F) -> CellVar<F> {
        let cell = CellVar {
            id: self.next_id,
            value,
        };
        self.next_id += 1;
        cell
    }

    fn witness_big(&mut self, value: &BigUint) -> CellVar<F> {
        self.witness_fe(biguint_to_fe::<F>(value))
    }

    fn witness_signed(&mut self, value: &BigInt) -> CellVar<F> {
        let field = match value.sign() {
            Sign::Minus => F::ZERO - biguint_to_fe::<F>(value.magnitude()),
            Sign::NoSign => F::ZERO,
            Sign::Plus => biguint_to_fe::<F>(value.magnitude()),
        };
        self.witness_fe(field)
    }

    fn caller_instance(&mut self, value: u8) -> CellVar<F> {
        let cell = self.witness_fe(F::from(u64::from(value)));
        self.caller_instances.push(cell);
        cell
    }

    fn constant_signed(&mut self, value: BigInt) -> CellVar<F> {
        if let Some(cell) = self.constants.get(&value) {
            return *cell;
        }
        let cell = self.witness_signed(&value);
        self.constants.insert(value, cell);
        self.constant_instances.push(cell);
        cell
    }

    fn constant_big(&mut self, value: impl Into<BigUint>) -> CellVar<F> {
        self.constant_signed(BigInt::from_biguint(Sign::Plus, value.into()))
    }

    fn constant_i64(&mut self, value: i64) -> CellVar<F> {
        self.constant_signed(BigInt::from(value))
    }

    fn zero(&mut self) -> CellVar<F> {
        self.constant_big(0_u8)
    }

    fn one(&mut self) -> CellVar<F> {
        self.constant_big(1_u8)
    }

    fn constant_bool(&mut self, value: bool) -> BoolVar<F> {
        BoolVar {
            cell: self.constant_big(u8::from(value)),
            value,
        }
    }

    fn boolean(&mut self, value: bool) -> BoolVar<F> {
        let cell = self.witness_fe(F::from(u64::from(value)));
        let zero = self.zero();
        let one = self.one();
        self.sign_lanes.push(SignLane {
            magnitude: zero,
            sign: cell,
            signed: zero,
            active: one,
        });
        BoolVar { cell, value }
    }

    fn bounded(
        &mut self,
        integer: BigUint,
        bits: usize,
        active: &BoolVar<F>,
        witness: &'static str,
    ) -> Result<BoundedCell<F>, P256PackedAffineFailureV2> {
        let actual_bits = usize::try_from(integer.bits()).unwrap_or(usize::MAX);
        if actual_bits > bits || bits > 6 * LOOKUP_BITS {
            return Err(P256PackedAffineFailureV2::IntegerBound {
                witness,
                actual_bits,
                maximum_bits: bits.min(6 * LOOKUP_BITS),
            });
        }
        if !active.value && integer != BigUint::from(0_u8) {
            return Err(P256PackedAffineFailureV2::Source(
                "inactive bounded witness was not zeroized",
            ));
        }
        let cell = self.witness_big(&integer);
        let mask = (BigUint::from(1_u8) << LOOKUP_BITS) - 1_u8;
        let chunks = (0..bits.div_ceil(LOOKUP_BITS))
            .map(|index| {
                let shift = index * LOOKUP_BITS;
                let chunk_integer = (&integer >> shift) & &mask;
                let remaining = bits - shift;
                RangeChunk {
                    cell: self.witness_big(&chunk_integer),
                    integer: chunk_integer,
                    bits: remaining.min(LOOKUP_BITS),
                }
            })
            .collect::<Vec<_>>();
        let bounded = BoundedCell {
            cell,
            integer,
            chunks,
            active: active.cell,
        };
        self.range_rows.push(RangeRow {
            bounded: bounded.clone(),
        });
        Ok(bounded)
    }

    fn bounded_existing(
        &mut self,
        cell: CellVar<F>,
        integer: BigUint,
        bits: usize,
        active: &BoolVar<F>,
        witness: &'static str,
    ) -> Result<BoundedCell<F>, P256PackedAffineFailureV2> {
        let actual_bits = usize::try_from(integer.bits()).unwrap_or(usize::MAX);
        if actual_bits > bits || bits > 6 * LOOKUP_BITS {
            return Err(P256PackedAffineFailureV2::IntegerBound {
                witness,
                actual_bits,
                maximum_bits: bits.min(6 * LOOKUP_BITS),
            });
        }
        if !active.value && integer != BigUint::from(0_u8) {
            return Err(P256PackedAffineFailureV2::Source(
                "inactive existing bounded witness was not zeroized",
            ));
        }
        let mask = (BigUint::from(1_u8) << LOOKUP_BITS) - 1_u8;
        let chunks = (0..bits.div_ceil(LOOKUP_BITS))
            .map(|index| {
                let shift = index * LOOKUP_BITS;
                let chunk_integer = (&integer >> shift) & &mask;
                let remaining = bits - shift;
                RangeChunk {
                    cell: self.witness_big(&chunk_integer),
                    integer: chunk_integer,
                    bits: remaining.min(LOOKUP_BITS),
                }
            })
            .collect::<Vec<_>>();
        let bounded = BoundedCell {
            cell,
            integer,
            chunks,
            active: active.cell,
        };
        self.range_rows.push(RangeRow {
            bounded: bounded.clone(),
        });
        Ok(bounded)
    }

    fn load_uint(
        &mut self,
        value: BigUint,
        active: &BoolVar<F>,
        witness: &'static str,
    ) -> Result<UintVar<F>, P256PackedAffineFailureV2> {
        let limbs = decompose_limbs(&value)
            .into_iter()
            .map(|limb| self.bounded(limb, LIMB_BITS, active, witness))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .unwrap_or_else(|_| panic!("P-256 uses exactly {LIMBS} limbs"));
        Ok(UintVar { limbs, value })
    }

    fn constant_uint(&mut self, value: BigUint) -> UintVar<F> {
        let one = self.one();
        let limbs = decompose_limbs(&value).map(|integer| BoundedCell {
            cell: self.constant_big(integer.clone()),
            integer,
            chunks: Vec::new(),
            active: one,
        });
        UintVar { limbs, value }
    }

    fn bool_not(&mut self, value: &BoolVar<F>) -> BoolVar<F> {
        let output = self.boolean(!value.value);
        let one = self.one();
        self.emit_linear_equation(&[(value.cell, 1), (output.cell, 1), (one, -1)]);
        output
    }

    fn bool_and(&mut self, left: &BoolVar<F>, right: &BoolVar<F>) -> BoolVar<F> {
        let output = BoolVar {
            cell: self.witness_fe(left.cell.value * right.cell.value),
            value: left.value && right.value,
        };
        self.emit_product_equation(left.cell, right.cell, output.cell);
        output
    }

    fn bool_or_exclusive(&mut self, left: &BoolVar<F>, right: &BoolVar<F>) -> BoolVar<F> {
        let output = self.boolean(left.value || right.value);
        self.emit_linear_equation(&[(left.cell, 1), (right.cell, 1), (output.cell, -1)]);
        output
    }

    fn bool_or(&mut self, left: &BoolVar<F>, right: &BoolVar<F>) -> BoolVar<F> {
        let output = self.boolean(left.value || right.value);
        let product = self.mul(left.cell, right.cell);
        self.emit_linear_equation(&[
            (left.cell, 1),
            (right.cell, 1),
            (product, -1),
            (output.cell, -1),
        ]);
        output
    }

    fn fma(&mut self, add: CellVar<F>, left: CellVar<F>, right: CellVar<F>) -> CellVar<F> {
        let output = self.witness_fe(add.value + left.value * right.value);
        let zero = self.zero();
        self.dense_rows.push(DenseRow {
            products: [(left, right), (zero, zero), (zero, zero)],
            accumulator: add,
            output,
        });
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
        let zero = self.zero();
        self.dense_rows.push(DenseRow {
            products: [(right, one), (zero, zero), (zero, zero)],
            accumulator: output,
            output: left,
        });
        output
    }

    fn scale(&mut self, value: CellVar<F>, coefficient: impl Into<BigInt>) -> CellVar<F> {
        let coefficient = self.constant_signed(coefficient.into());
        self.mul(value, coefficient)
    }

    fn emit_product_equation(&mut self, left: CellVar<F>, right: CellVar<F>, output: CellVar<F>) {
        let zero = self.zero();
        self.dense_rows.push(DenseRow {
            products: [(left, right), (zero, zero), (zero, zero)],
            accumulator: zero,
            output,
        });
    }

    fn emit_linear_equation(&mut self, terms: &[(CellVar<F>, i64)]) {
        let pairs = terms
            .iter()
            .map(|(cell, coefficient)| (*cell, self.constant_i64(*coefficient)))
            .collect::<Vec<_>>();
        let zero = self.zero();
        let mut accumulator = zero;
        for (index, group) in pairs.chunks(3).enumerate() {
            let is_last = index + 1 == pairs.len().div_ceil(3);
            let mut products = [(zero, zero); 3];
            let mut next_value = accumulator.value;
            for (slot, (left, right)) in group.iter().copied().enumerate() {
                products[slot] = (left, right);
                next_value += left.value * right.value;
            }
            let output = if is_last {
                zero
            } else {
                self.witness_fe(next_value)
            };
            self.dense_rows.push(DenseRow {
                products,
                accumulator,
                output,
            });
            accumulator = output;
        }
    }

    fn assert_equal(&mut self, left: CellVar<F>, right: CellVar<F>) {
        self.emit_linear_equation(&[(left, 1), (right, -1)]);
    }

    fn assert_zero(&mut self, value: CellVar<F>) {
        let zero = self.zero();
        self.assert_equal(value, zero);
    }

    fn is_zero_cell(&mut self, value: CellVar<F>, is_zero: bool) -> BoolVar<F> {
        self.zero_tests += 1;
        let flag = self.boolean(is_zero);
        let inverse = self.witness_fe(if is_zero {
            F::ZERO
        } else {
            value.value.invert().unwrap()
        });
        let one = self.one();
        let zero = self.zero();
        // value * inverse + flag = 1
        self.dense_rows.push(DenseRow {
            products: [(value, inverse), (zero, zero), (zero, zero)],
            accumulator: flag.cell,
            output: one,
        });
        // value * flag = 0
        self.dense_rows.push(DenseRow {
            products: [(value, flag.cell), (zero, zero), (zero, zero)],
            accumulator: zero,
            output: zero,
        });
        flag
    }

    fn select(&mut self, left: CellVar<F>, bit: &BoolVar<F>, right: CellVar<F>) -> CellVar<F> {
        let output = self.witness_fe(if bit.value { right.value } else { left.value });
        self.selects.push(SelectLane {
            left,
            bit: bit.cell,
            right,
            output,
        });
        output
    }

    fn realize_term(
        &mut self,
        term: &PolyTerm<F>,
    ) -> Result<Vec<RealizedTerm<F>>, P256PackedAffineFailureV2> {
        let mut factors = term
            .factors
            .iter()
            .map(|factor| {
                factor
                    .cell
                    .unwrap_or_else(|| self.constant_signed(factor.integer.clone()))
            })
            .collect::<Vec<_>>();
        let magnitude = term.coefficient.unsigned_abs();
        let negative = term.coefficient < 0;
        if factors.len() < 2 {
            let coefficient = if negative {
                -BigInt::from(magnitude)
            } else {
                BigInt::from(magnitude)
            };
            factors.push(self.constant_signed(coefficient));
            return Ok(vec![RealizedTerm {
                factors,
                integer: term.integer(),
                negative: false,
            }]);
        }
        if factors.len() == 2
            && term
                .factors
                .iter()
                .position(|factor| factor.cell.is_none())
                .is_some()
        {
            let constant_index = term
                .factors
                .iter()
                .position(|factor| factor.cell.is_none())
                .expect("checked above");
            let scaled = &term.factors[constant_index].integer * BigInt::from(term.coefficient);
            factors[constant_index] = self.constant_signed(scaled);
            return Ok(vec![RealizedTerm {
                factors,
                integer: term.integer(),
                negative: false,
            }]);
        }
        if magnitude > 8 {
            return Err(P256PackedAffineFailureV2::Source(
                "nonlinear coefficient exceeded the packed duplication bound",
            ));
        }
        let unit_integer = term
            .factors
            .iter()
            .fold(BigInt::from(1), |value, factor| value * &factor.integer);
        Ok((0..magnitude)
            .map(|_| RealizedTerm {
                factors: factors.clone(),
                integer: if negative {
                    -unit_integer.clone()
                } else {
                    unit_integer.clone()
                },
                negative,
            })
            .collect())
    }

    fn realize_zero_sum(
        &mut self,
        terms: &[PolyTerm<F>],
        witness: &'static str,
    ) -> Result<(), P256PackedAffineFailureV2> {
        let realized = terms
            .iter()
            .map(|term| self.realize_term(term))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let zero = self.zero();
        let mut accumulator = zero;
        let mut accumulator_integer = BigInt::from(0);
        let cubic = realized
            .iter()
            .filter(|term| term.factors.len() == 3)
            .collect::<Vec<_>>();
        let positive_dense = realized
            .iter()
            .filter(|term| term.factors.len() <= 2 && !term.negative)
            .collect::<Vec<_>>();
        let negative_dense = realized
            .iter()
            .filter(|term| term.factors.len() <= 2 && term.negative)
            .collect::<Vec<_>>();
        let total_steps =
            cubic.len() + positive_dense.len().div_ceil(3) + negative_dense.len().div_ceil(3);
        let mut step = 0_usize;
        for term in cubic {
            let next_integer = &accumulator_integer + &term.integer;
            let bits = usize::try_from(next_integer.magnitude().bits()).unwrap_or(usize::MAX);
            self.maximum_coefficient_bits = self.maximum_coefficient_bits.max(bits);
            if bits > PACKED_COEFFICIENT_BOUND_BITS {
                return Err(P256PackedAffineFailureV2::UnsafeNativeCoefficient { witness, bits });
            }
            step += 1;
            let next = if step == total_steps {
                zero
            } else {
                self.witness_signed(&next_integer)
            };
            let (row_accumulator, row_output) = if term.negative {
                (next, accumulator)
            } else {
                (accumulator, next)
            };
            self.sparse_rows.push(SparseRow {
                left: term.factors[0],
                right: term.factors[1],
                gate: term.factors[2],
                accumulator: row_accumulator,
                output: row_output,
            });
            accumulator = next;
            accumulator_integer = next_integer;
        }

        for (negative, dense) in [(false, positive_dense), (true, negative_dense)] {
            for group in dense.chunks(3) {
                let group_integer = group
                    .iter()
                    .map(|term| term.integer.clone())
                    .fold(BigInt::from(0), |sum, term| sum + term);
                let next_integer = &accumulator_integer + &group_integer;
                let bits = usize::try_from(next_integer.magnitude().bits()).unwrap_or(usize::MAX);
                self.maximum_coefficient_bits = self.maximum_coefficient_bits.max(bits);
                if bits > PACKED_COEFFICIENT_BOUND_BITS {
                    return Err(P256PackedAffineFailureV2::UnsafeNativeCoefficient {
                        witness,
                        bits,
                    });
                }
                step += 1;
                let next = if step == total_steps {
                    zero
                } else {
                    self.witness_signed(&next_integer)
                };
                let mut products = [(zero, zero); 3];
                for (slot, term) in group.iter().enumerate() {
                    products[slot] = match term.factors.as_slice() {
                        [] => (zero, self.one()),
                        [factor] => (*factor, self.one()),
                        [left, right] => (*left, *right),
                        _ => unreachable!("dense term has at most two factors"),
                    };
                }
                let (row_accumulator, row_output) = if negative {
                    (next, accumulator)
                } else {
                    (accumulator, next)
                };
                self.dense_rows.push(DenseRow {
                    products,
                    accumulator: row_accumulator,
                    output: row_output,
                });
                accumulator = next;
                accumulator_integer = next_integer;
            }
        }
        debug_assert_eq!(accumulator.id, zero.id);
        Ok(())
    }

    fn finish(self) -> Result<PackedTrace<F>, P256PackedAffineFailureV2> {
        transpose_packed_trace(self)
    }
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

#[derive(Clone, Debug)]
struct SignedMagnitude<F> {
    sign: BoolVar<F>,
    magnitude: UintVar<F>,
    signed_limbs: [CellVar<F>; LIMBS],
}

fn uint_is_zero<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    value: &UintVar<F>,
) -> BoolVar<F> {
    let mut result = builder.constant_bool(true);
    for limb in &value.limbs {
        let zero = builder.is_zero_cell(limb.cell, limb.integer == BigUint::from(0_u8));
        result = builder.bool_and(&result, &zero);
    }
    result
}

fn uint_equal<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
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
    builder: &mut PackedBuilder<F>,
    gate: &BoolVar<F>,
    value: &UintVar<F>,
) {
    for limb in &value.limbs {
        let product = builder.mul(gate.cell, limb.cell);
        builder.assert_zero(product);
    }
}

fn signed_uint_witness<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    value: &BigInt,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<SignedMagnitude<F>, P256PackedAffineFailureV2> {
    let negative = value.sign() == Sign::Minus;
    let magnitude_value = value.magnitude().clone();
    let bits = usize::try_from(magnitude_value.bits()).unwrap_or(usize::MAX);
    if bits > QUOTIENT_BITS {
        return Err(P256PackedAffineFailureV2::IntegerBound {
            witness,
            actual_bits: bits,
            maximum_bits: QUOTIENT_BITS,
        });
    }
    builder.maximum_quotient_bits = builder.maximum_quotient_bits.max(bits);
    let sign_cell = builder.witness_fe(F::from(u64::from(negative)));
    let sign = BoolVar {
        cell: sign_cell,
        value: negative,
    };
    let magnitude = builder.load_uint(magnitude_value, active, witness)?;
    let signed_limbs = std::array::from_fn(|index| {
        let signed = builder.witness_fe(if negative {
            F::ZERO - magnitude.limbs[index].cell.value
        } else {
            magnitude.limbs[index].cell.value
        });
        builder.sign_lanes.push(SignLane {
            magnitude: magnitude.limbs[index].cell,
            sign: sign_cell,
            signed,
            active: active.cell,
        });
        signed
    });
    let zero = uint_is_zero(builder, &magnitude);
    let negative_zero = builder.mul(sign.cell, zero.cell);
    builder.assert_zero(negative_zero);
    Ok(SignedMagnitude {
        sign,
        magnitude,
        signed_limbs,
    })
}

#[derive(Clone, Debug)]
struct SignedCarry<F> {
    signed: CellVar<F>,
    _magnitude: BoundedCell<F>,
}

fn signed_carry_witness<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    value: &BigInt,
    active: &BoolVar<F>,
) -> Result<SignedCarry<F>, P256PackedAffineFailureV2> {
    let negative = value.sign() == Sign::Minus;
    let integer = value.magnitude().clone();
    let bits = usize::try_from(integer.bits()).unwrap_or(usize::MAX);
    if bits > CARRY_BITS {
        return Err(P256PackedAffineFailureV2::IntegerBound {
            witness: "signed modular carry",
            actual_bits: bits,
            maximum_bits: CARRY_BITS,
        });
    }
    builder.maximum_carry_bits = builder.maximum_carry_bits.max(bits);
    let magnitude = builder.bounded(integer, CARRY_BITS, active, "signed modular carry")?;
    let sign_cell = builder.witness_fe(F::from(u64::from(negative)));
    let signed = builder.witness_fe(if negative {
        F::ZERO - magnitude.cell.value
    } else {
        magnitude.cell.value
    });
    builder.sign_lanes.push(SignLane {
        magnitude: magnitude.cell,
        sign: sign_cell,
        signed,
        active: active.cell,
    });
    let zero = builder.is_zero_cell(magnitude.cell, magnitude.integer == BigUint::from(0_u8));
    let negative_zero = builder.mul(sign_cell, zero.cell);
    builder.assert_zero(negative_zero);
    Ok(SignedCarry {
        signed,
        _magnitude: magnitude,
    })
}

fn constrain_modular_expression<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    expression: &RadixExpression<F>,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<(), P256PackedAffineFailureV2> {
    let expression_integer = expression.integer();
    let modulus_integer = bigint(modulus);
    let quotient_value = &expression_integer / &modulus_integer;
    let quotient = signed_uint_witness(builder, &quotient_value, active, witness)?;
    let modulus_limbs = decompose_limbs(modulus);
    let expression_coefficients = expression.integer_coefficients();
    let signed_quotient_limbs = quotient
        .magnitude
        .limbs
        .iter()
        .map(|limb| {
            let value = bigint(&limb.integer);
            if quotient.sign.value { -value } else { value }
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

    for coefficient in 0..2 * LIMBS - 1 {
        let mut terms = expression.coefficients[coefficient].clone();
        for left in 0..LIMBS {
            let Some(right) = coefficient.checked_sub(left).filter(|right| *right < LIMBS) else {
                continue;
            };
            terms.push(PolyTerm {
                coefficient: -1,
                factors: vec![
                    PolyFactor {
                        cell: Some(quotient.signed_limbs[left]),
                        integer: signed_quotient_limbs[left].clone(),
                    },
                    PolyFactor {
                        cell: None,
                        integer: bigint(&modulus_limbs[right]),
                    },
                ],
            });
        }
        if coefficient > 0 {
            terms.push(PolyTerm {
                coefficient: 1,
                factors: vec![PolyFactor {
                    cell: Some(carries[coefficient - 1].signed),
                    integer: carry_values[coefficient - 1].clone(),
                }],
            });
        }
        if coefficient < carries.len() {
            terms.push(PolyTerm {
                coefficient: -1,
                factors: vec![
                    PolyFactor {
                        cell: Some(carries[coefficient].signed),
                        integer: carry_values[coefficient].clone(),
                    },
                    PolyFactor {
                        cell: None,
                        integer: bigint(&radix()),
                    },
                ],
            });
        }
        builder.realize_zero_sum(&terms, witness)?;
    }
    builder.modular_relations += 1;
    Ok(())
}

fn constrain_exact_constant_sum<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
    constant: &BigUint,
) {
    builder.canonical_checks += 1;
    let radix = radix();
    let constant_limbs = decompose_limbs(constant);
    let mut carry = builder.constant_bool(false);
    for (index, constant_limb) in constant_limbs.iter().enumerate() {
        let integer_sum =
            &left.limbs[index].integer + &right.limbs[index].integer + u8::from(carry.value);
        let next_value = integer_sum >= radix;
        let next = if index + 1 == LIMBS {
            builder.constant_bool(false)
        } else {
            builder.boolean(next_value)
        };
        let constant_cell = builder.constant_big(constant_limb.clone());
        builder.wide_rows.push(WideRow {
            left: left.limbs[index].cell,
            right: right.limbs[index].cell,
            carry_in: carry.cell,
            carry_out: next.cell,
            constant: constant_cell,
        });
        carry = next;
    }
}

fn constrain_canonical<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    value: &UintVar<F>,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<(), P256PackedAffineFailureV2> {
    let maximum = modulus - 1_u8;
    let slack_value = if value.value <= maximum {
        &maximum - &value.value
    } else {
        BigUint::from(0_u8)
    };
    let always = builder.constant_bool(true);
    let slack = builder.load_uint(slack_value, &always, witness)?;
    constrain_exact_constant_sum(builder, value, &slack, &maximum);
    Ok(())
}

fn constrain_at_most<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    value: &UintVar<F>,
    maximum: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<BoolVar<F>, P256PackedAffineFailureV2> {
    let valid = active.value && value.value <= *maximum;
    let slack_value = if valid {
        maximum - &value.value
    } else {
        BigUint::from(0_u8)
    };
    let always = builder.constant_bool(true);
    let slack = builder.load_uint(slack_value, &always, witness)?;
    constrain_exact_constant_sum(builder, value, &slack, maximum);
    Ok(builder.boolean(valid))
}

fn modular_product<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
    output_value: BigUint,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<UintVar<F>, P256PackedAffineFailureV2> {
    let output = builder.load_uint(output_value, active, witness)?;
    constrain_canonical(builder, &output, modulus, active, witness)?;
    let mut expression = RadixExpression::new();
    expression.add_product(left, right, Some(active), 1);
    expression.add_linear(&output, Some(active), -1);
    constrain_modular_expression(builder, &expression, modulus, active, witness)?;
    Ok(output)
}

fn modular_linear_reduction<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    terms: &[(&UintVar<F>, i64)],
    output_value: BigUint,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<UintVar<F>, P256PackedAffineFailureV2> {
    let output = builder.load_uint(output_value, active, witness)?;
    constrain_canonical(builder, &output, modulus, active, witness)?;
    let mut expression = RadixExpression::new();
    for (value, coefficient) in terms {
        expression.add_linear(value, Some(active), *coefficient);
    }
    expression.add_linear(&output, Some(active), -1);
    constrain_modular_expression(builder, &expression, modulus, active, witness)?;
    Ok(output)
}

fn constrain_single_reduction<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
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
        let integer_sum = &reduced.limbs[index].integer
            + if reduce_value {
                modulus_limbs[index].clone()
            } else {
                BigUint::from(0_u8)
            }
            + u8::from(carry.value);
        let next_value = integer_sum >= radix;
        let next = if index + 1 == LIMBS {
            builder.constant_bool(false)
        } else {
            builder.boolean(next_value)
        };
        let modulus_part = builder.scale(reduce.cell, bigint(&modulus_limbs[index]));
        let sum = builder.add(reduced.limbs[index].cell, modulus_part);
        let with_carry = builder.add(sum, carry.cell);
        let radix_next = builder.scale(next.cell, bigint(&radix));
        let expected = builder.add(raw.limbs[index].cell, radix_next);
        builder.assert_equal(with_carry, expected);
        carry = next;
    }
    reduce
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

#[derive(Clone, Debug)]
struct AffineVar<F> {
    x: UintVar<F>,
    y: UintVar<F>,
    infinity: BoolVar<F>,
    value: AffineValue,
}

fn identity_var<F: BigPrimeField>(builder: &mut PackedBuilder<F>) -> AffineVar<F> {
    let zero = builder.constant_uint(BigUint::from(0_u8));
    AffineVar {
        x: zero.clone(),
        y: zero,
        infinity: builder.constant_bool(true),
        value: AffineValue::identity(),
    }
}

fn constrain_identity_coordinates<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    point: &AffineVar<F>,
) {
    gate_uint_zero(builder, &point.infinity, &point.x);
    gate_uint_zero(builder, &point.infinity, &point.y);
}

fn select_uint<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    left: &UintVar<F>,
    bit: &BoolVar<F>,
    right: &UintVar<F>,
) -> UintVar<F> {
    let limbs = std::array::from_fn(|index| BoundedCell {
        cell: builder.select(left.limbs[index].cell, bit, right.limbs[index].cell),
        integer: if bit.value {
            right.limbs[index].integer.clone()
        } else {
            left.limbs[index].integer.clone()
        },
        chunks: Vec::new(),
        active: builder.one(),
    });
    UintVar {
        value: if bit.value {
            right.value.clone()
        } else {
            left.value.clone()
        },
        limbs,
    }
}

fn zero_uint<F: BigPrimeField>(builder: &mut PackedBuilder<F>) -> UintVar<F> {
    builder.constant_uint(BigUint::from(0_u8))
}

fn complete_double<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    point: &AffineVar<F>,
) -> Result<AffineVar<F>, P256PackedAffineFailureV2> {
    builder.complete_doublings += 1;
    let modulus = modulus_base();
    let finite = builder.bool_not(&point.infinity);
    let y_zero = uint_is_zero(builder, &point.y);
    let y_nonzero = builder.bool_not(&y_zero);
    let active = builder.bool_and(&finite, &y_nonzero);
    let (value, lambda_value) = affine_double_value(&point.value);
    let lambda = builder.load_uint(
        if active.value {
            lambda_value
        } else {
            BigUint::from(0_u8)
        },
        &active,
        "doubling lambda",
    )?;
    let x = builder.load_uint(
        if active.value {
            value.x.clone()
        } else {
            BigUint::from(0_u8)
        },
        &active,
        "doubling x",
    )?;
    let y = builder.load_uint(
        if active.value {
            value.y.clone()
        } else {
            BigUint::from(0_u8)
        },
        &active,
        "doubling y",
    )?;
    for (candidate, label) in [
        (&lambda, "doubling lambda slack"),
        (&x, "doubling x slack"),
        (&y, "doubling y slack"),
    ] {
        constrain_canonical(builder, candidate, &modulus, &active, label)?;
    }

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
    let infinity = builder.bool_or(&point.infinity, &y_zero);
    let output = AffineVar {
        x,
        y,
        infinity,
        value,
    };
    constrain_identity_coordinates(builder, &output);
    Ok(output)
}

fn complete_add<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    left: &AffineVar<F>,
    right: &AffineVar<F>,
) -> Result<AffineVar<F>, P256PackedAffineFailureV2> {
    builder.complete_additions += 1;
    let modulus = modulus_base();
    let x_equal = uint_equal(builder, &left.x, &right.x);
    let y_equal = uint_equal(builder, &left.y, &right.y);
    let left_finite = builder.bool_not(&left.infinity);
    let right_finite = builder.bool_not(&right.infinity);
    let finite = builder.bool_and(&left_finite, &right_finite);

    let y_sum_value = if finite.value {
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
    let branches = [
        chord.cell,
        tangent.cell,
        opposite.cell,
        take_left.cell,
        take_right.cell,
        both_identity.cell,
    ];
    let one = builder.one();
    let mut branch_terms = branches
        .into_iter()
        .map(|branch| (branch, 1_i64))
        .collect::<Vec<_>>();
    branch_terms.push((one, -1));
    builder.emit_linear_equation(&branch_terms);

    let (host_output, host_lambda) = affine_add_value(&left.value, &right.value);
    let lambda = builder.load_uint(
        if active.value {
            host_lambda
        } else {
            BigUint::from(0_u8)
        },
        &active,
        "complete-add lambda",
    )?;
    let candidate_x = builder.load_uint(
        if active.value {
            host_output.x.clone()
        } else {
            BigUint::from(0_u8)
        },
        &active,
        "complete-add x",
    )?;
    let candidate_y = builder.load_uint(
        if active.value {
            host_output.y.clone()
        } else {
            BigUint::from(0_u8)
        },
        &active,
        "complete-add y",
    )?;
    for (candidate, label) in [
        (&lambda, "complete-add lambda slack"),
        (&candidate_x, "complete-add x slack"),
        (&candidate_y, "complete-add y slack"),
    ] {
        constrain_canonical(builder, candidate, &modulus, &active, label)?;
    }

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
    builder: &mut PackedBuilder<F>,
    point: &AffineVar<F>,
) -> Result<[AffineVar<F>; 16], P256PackedAffineFailureV2> {
    let mut table = std::array::from_fn(|_| None);
    table[0] = Some(identity_var(builder));
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
    builder: &mut PackedBuilder<F>,
    mut level: Vec<BoundedCell<F>>,
    bits_lsb: &[BoolVar<F>; 4],
) -> BoundedCell<F> {
    for bit in bits_lsb {
        level = level
            .chunks_exact(2)
            .map(|pair| BoundedCell {
                cell: builder.select(pair[0].cell, bit, pair[1].cell),
                integer: if bit.value {
                    pair[1].integer.clone()
                } else {
                    pair[0].integer.clone()
                },
                chunks: Vec::new(),
                active: builder.one(),
            })
            .collect();
    }
    level.pop().expect("four-bit table is non-empty")
}

fn window_is_zero<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
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
    builder: &mut PackedBuilder<F>,
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
            value | (usize::from(enabled.value) << bit)
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
    builder: &mut PackedBuilder<F>,
    bits_lsb: &[BoolVar<F>; 4],
) -> AffineVar<F> {
    let digit = bits_lsb
        .iter()
        .enumerate()
        .fold(0_usize, |value, (bit, enabled)| {
            value | (usize::from(enabled.value) << bit)
        });
    let values = fixed_generator_values();
    let x_limbs: [BoundedCell<F>; LIMBS] = std::array::from_fn(|limb| {
        let level = values
            .iter()
            .map(|point| {
                let integer = decompose_limbs(&point.x)[limb].clone();
                BoundedCell {
                    cell: builder.constant_big(integer.clone()),
                    integer,
                    chunks: Vec::new(),
                    active: builder.one(),
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
                    cell: builder.constant_big(integer.clone()),
                    integer,
                    chunks: Vec::new(),
                    active: builder.one(),
                }
            })
            .collect();
        select_bounded_component(builder, level, bits_lsb)
    });
    let identity = window_is_zero(builder, bits_lsb);
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
        value: values[digit].clone(),
    }
}

fn scalar_bits<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
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
            let power = builder.constant_big(BigUint::from(1_u8) << bit);
            accumulator = builder.fma(accumulator, bit_var.cell, power);
            bits.push(bit_var);
        }
        builder.assert_equal(accumulator, limb.cell);
    }
    bits.try_into()
        .unwrap_or_else(|_| panic!("three 86-bit limbs expose exactly 256 bits"))
}

fn straus_two_scalar<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    public_key: &AffineVar<F>,
    u1: &UintVar<F>,
    u2: &UintVar<F>,
) -> Result<AffineVar<F>, P256PackedAffineFailureV2> {
    let variable_table = variable_window_table(builder, public_key)?;
    let u1_bits = scalar_bits(builder, u1);
    let u2_bits = scalar_bits(builder, u2);
    let mut accumulator = identity_var(builder);
    for window in (0..WINDOWS).rev() {
        for _ in 0..WINDOW_BITS {
            accumulator = complete_double(builder, &accumulator)?;
        }
        let start = window * WINDOW_BITS;
        let fixed_bits: [BoolVar<F>; 4] = std::array::from_fn(|bit| u1_bits[start + bit].clone());
        let variable_bits: [BoolVar<F>; 4] =
            std::array::from_fn(|bit| u2_bits[start + bit].clone());
        let fixed = select_fixed_window(builder, &fixed_bits);
        let variable = select_variable_window(builder, &variable_table, &variable_bits);
        accumulator = complete_add(builder, &accumulator, &fixed)?;
        accumulator = complete_add(builder, &accumulator, &variable)?;
    }
    Ok(accumulator)
}

fn constrain_on_curve<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    point: &AffineVar<F>,
) -> Result<BoolVar<F>, P256PackedAffineFailureV2> {
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
    builder.assert_equal(nonidentity.cell, one);
    Ok(nonidentity)
}

fn constrain_scalar_product<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
    output: &UintVar<F>,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<(), P256PackedAffineFailureV2> {
    let mut expression = RadixExpression::new();
    expression.add_product(left, right, Some(active), 1);
    expression.add_linear(output, Some(active), -1);
    constrain_modular_expression(builder, &expression, &modulus_scalar(), active, witness)
}

fn bind_public_uint<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    bytes_be: &[BoundedCell<F>; 32],
    raw: &[u8; 32],
    witness: &'static str,
) -> Result<UintVar<F>, P256PackedAffineFailureV2> {
    let active = builder.constant_bool(true);
    let value = BigUint::from_bytes_be(raw);
    let loaded = builder.load_uint(value, &active, witness)?;
    let bytes_le = std::array::from_fn::<_, 32, _>(|index| bytes_be[31 - index].clone());

    let byte10 = &bytes_le[10];
    let low6_integer = &byte10.integer & BigUint::from(0x3f_u8);
    let high2_integer = &byte10.integer >> 6_usize;
    let low6 = builder.bounded(low6_integer, 6, &active, witness)?;
    let high2 = builder.bounded(high2_integer, 2, &active, witness)?;
    let sixty_four = builder.constant_big(64_u8);
    let recomposed10 = builder.fma(low6.cell, high2.cell, sixty_four);
    builder.assert_equal(recomposed10, byte10.cell);

    let byte21 = &bytes_le[21];
    let low4_integer = &byte21.integer & BigUint::from(0x0f_u8);
    let high4_integer = &byte21.integer >> 4_usize;
    let low4 = builder.bounded(low4_integer, 4, &active, witness)?;
    let high4 = builder.bounded(high4_integer, 4, &active, witness)?;
    let sixteen = builder.constant_big(16_u8);
    let recomposed21 = builder.fma(low4.cell, high4.cell, sixteen);
    builder.assert_equal(recomposed21, byte21.cell);

    let zero = builder.zero();
    let mut limb0 = zero;
    for (index, byte) in bytes_le[..10].iter().enumerate() {
        let power = builder.constant_big(BigUint::from(1_u8) << (8 * index));
        limb0 = builder.fma(limb0, byte.cell, power);
    }
    let power80 = builder.constant_big(BigUint::from(1_u8) << 80);
    limb0 = builder.fma(limb0, low6.cell, power80);
    builder.assert_equal(limb0, loaded.limbs[0].cell);

    let mut limb1 = high2.cell;
    for (offset, byte) in bytes_le[11..21].iter().enumerate() {
        let power = builder.constant_big(BigUint::from(1_u8) << (2 + 8 * offset));
        limb1 = builder.fma(limb1, byte.cell, power);
    }
    let power82 = builder.constant_big(BigUint::from(1_u8) << 82);
    limb1 = builder.fma(limb1, low4.cell, power82);
    builder.assert_equal(limb1, loaded.limbs[1].cell);

    let mut limb2 = high4.cell;
    for (offset, byte) in bytes_le[22..].iter().enumerate() {
        let power = builder.constant_big(BigUint::from(1_u8) << (4 + 8 * offset));
        limb2 = builder.fma(limb2, byte.cell, power);
    }
    builder.assert_equal(limb2, loaded.limbs[2].cell);
    Ok(loaded)
}

fn constrain_ecdsa<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    input: &[u8; PUBLIC_BYTES],
    sec1: &[u8; 65],
    digest: &[u8; 32],
    signature: &[u8; 64],
) -> Result<(), P256PackedAffineFailureV2> {
    let active = builder.constant_bool(true);
    let public = input
        .iter()
        .map(|byte| {
            let cell = builder.caller_instance(*byte);
            builder.bounded_existing(cell, BigUint::from(*byte), 8, &active, "public byte")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let prefix_four = builder.constant_big(4_u8);
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
    constrain_canonical(
        builder,
        &x,
        &base_modulus,
        &active,
        "SEC1 x canonical slack",
    )?;
    constrain_canonical(
        builder,
        &y,
        &base_modulus,
        &active,
        "SEC1 y canonical slack",
    )?;
    constrain_canonical(builder, &r, &scalar_modulus, &active, "r canonical slack")?;
    constrain_canonical(builder, &s, &scalar_modulus, &active, "s canonical slack")?;
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
    let low_s = constrain_at_most(
        builder,
        &s,
        &(&scalar_modulus >> 1_usize),
        &active,
        "low-S slack",
    )?;

    let digest_integer = BigUint::from_bytes_be(digest);
    let z_value = &digest_integer % &scalar_modulus;
    let z = builder.load_uint(z_value, &active, "reduced digest")?;
    constrain_canonical(
        builder,
        &z,
        &scalar_modulus,
        &active,
        "digest canonical slack",
    )?;
    constrain_single_reduction(
        builder,
        &digest_uint,
        &z,
        &scalar_modulus,
        digest_integer >= scalar_modulus,
    );

    let s_inverse_value = modular_inverse(&s.value, &scalar_modulus);
    let s_inverse = builder.load_uint(s_inverse_value, &active, "scalar inverse")?;
    constrain_canonical(
        builder,
        &s_inverse,
        &scalar_modulus,
        &active,
        "scalar inverse canonical slack",
    )?;
    let one_uint = builder.constant_uint(BigUint::from(1_u8));
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
    let x_mod_n = builder.load_uint(x_mod_n_value, &active, "result x modulo n")?;
    constrain_canonical(
        builder,
        &x_mod_n,
        &scalar_modulus,
        &active,
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
    builder.assert_equal(result.cell, one);
    Ok(())
}

#[derive(Clone, Debug)]
struct PackedTrace<F> {
    rows_data: Vec<AssignedRow<F>>,
    instances: Vec<F>,
    rows: P256PackedAffineRowsV2,
}

impl<F: BigPrimeField> PackedTrace<F> {
    fn assign(
        &self,
        config: &P256PackedAffineConfigV2,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), Error> {
        if self.rows.total_rows > K16_MAX_ASSIGNED_ROWS {
            return Err(Error::Synthesis);
        }
        layouter.assign_region(
            || "packed affine P-256 trace and typed range table",
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
                    raw_assign_fixed(
                        &mut region,
                        config.opcode,
                        offset,
                        F::from(row.opcode as u64),
                    );
                    raw_assign_fixed(
                        &mut region,
                        config.range_tag,
                        offset,
                        F::from(u64::try_from(row.range_bits).expect("range tag fits u64")),
                    );
                    for (variable, column) in &row.aliases {
                        if let Some(first) = first_cells.insert(*variable, cells[*column]) {
                            raw_constrain_equal(&mut region, first, cells[*column]);
                        }
                    }
                }

                raw_assign_fixed(&mut region, config.table_first, 0, F::ZERO);
                raw_assign_fixed(&mut region, config.table_second, 0, F::ZERO);
                let mut offset = 1_usize;
                for bits in RANGE_CHUNK_BITS {
                    let tag = F::from(u64::try_from(bits).expect("range width fits u64"));
                    for value in 0..(1_usize << bits) {
                        let value = F::from(u64::try_from(value).expect("range value fits u64"));
                        raw_assign_fixed(&mut region, config.table_first, offset, tag * value);
                        raw_assign_fixed(
                            &mut region,
                            config.table_second,
                            offset,
                            tag * tag * value,
                        );
                        offset += 1;
                    }
                }
                debug_assert_eq!(offset, TABLE_ROWS);
                Ok(())
            },
        )
    }
}

fn transpose_packed_trace<F: BigPrimeField>(
    mut builder: PackedBuilder<F>,
) -> Result<PackedTrace<F>, P256PackedAffineFailureV2> {
    // These are layout constants, so force them into the verifier-bound tail
    // before destructuring the builder. They may already be cached.
    let (one, minus_two, negative_radix) = ensure_layout_constants(&mut builder);
    let zero = *builder
        .constants
        .get(&BigInt::from(0))
        .expect("zero is a verifier-bound constant");
    let PackedBuilder {
        caller_instances,
        constant_instances,
        range_rows,
        sparse_rows,
        dense_rows,
        wide_rows,
        sign_lanes,
        selects,
        modular_relations,
        complete_doublings,
        complete_additions,
        canonical_checks,
        zero_tests,
        maximum_quotient_bits,
        maximum_carry_bits,
        maximum_coefficient_bits,
        ..
    } = builder;
    let instance_variables = caller_instances
        .iter()
        .chain(&constant_instances)
        .copied()
        .collect::<Vec<_>>();
    let instances = instance_variables
        .iter()
        .map(|variable| variable.value)
        .collect::<Vec<_>>();
    let mut rows_data = Vec::new();
    let caller_instance_rows = caller_instances.len();
    let constant_instance_rows = constant_instances.len();
    for variable in &instance_variables {
        let mut row = AssignedRow::zero(Opcode::Bind);
        row.set(7, *variable);
        rows_data.push(row);
    }
    let binding_rows = rows_data.len();

    let mut pending = HashMap::<usize, Vec<CellVar<F>>>::new();
    let mut range_lookups = 0_usize;
    for range in range_rows {
        let mut row = AssignedRow::zero(Opcode::Range);
        let chunks = &range.bounded.chunks;
        if chunks
            .iter()
            .any(|chunk| !RANGE_CHUNK_BITS.contains(&chunk.bits))
        {
            return Err(P256PackedAffineFailureV2::Source(
                "range chunk width has no typed table tag",
            ));
        }
        for (column, chunk) in chunks.iter().take(6).enumerate() {
            row.set(column, chunk.cell);
        }
        row.set(6, range.bounded.cell);
        row.set(7, range.bounded.active);
        if let Some(first) = chunks.first() {
            row.range_bits = first.bits;
            range_lookups += 1;
            if let Some(second) = chunks.get(4) {
                if second.bits != first.bits {
                    return Err(P256PackedAffineFailureV2::Source(
                        "range lanes zeroed their recomposition selector",
                    ));
                }
                range_lookups += 1;
            }
        }
        for (index, chunk) in chunks.iter().enumerate() {
            if index == 0 || (index == 4 && row.range_bits != 0) {
                continue;
            }
            pending.entry(chunk.bits).or_default().push(chunk.cell);
        }
        rows_data.push(row);
    }
    let range_rows_count = rows_data.len() - binding_rows;

    let mut sparse_assigned = sparse_rows
        .into_iter()
        .map(|relation| {
            let mut row = AssignedRow::zero(Opcode::Sparse);
            row.set(1, relation.left);
            row.set(2, relation.right);
            row.set(3, relation.gate);
            row.set(5, relation.output);
            row.set(6, relation.accumulator);
            row
        })
        .collect::<Vec<_>>();
    let original_sparse_rows = sparse_assigned.len();
    let mut lookup_row = 0_usize;
    let mut widths = pending.keys().copied().collect::<Vec<_>>();
    widths.sort_unstable();
    for bits in widths {
        let cells = pending
            .remove(&bits)
            .expect("range width was collected from this map");
        range_lookups += cells.len();
        for pair in cells.chunks(2) {
            if lookup_row == sparse_assigned.len() {
                sparse_assigned.push(AssignedRow::zero(Opcode::Sparse));
            }
            let row = &mut sparse_assigned[lookup_row];
            row.range_bits = bits;
            row.set(0, pair[0]);
            if let Some(second) = pair.get(1) {
                row.set(4, *second);
            }
            lookup_row += 1;
        }
    }
    let lookup_only_rows = sparse_assigned.len() - original_sparse_rows;
    rows_data.extend(sparse_assigned);
    let sparse_rows_count = original_sparse_rows;

    for relation in dense_rows {
        let mut row = AssignedRow::zero(Opcode::Dense);
        row.set(0, relation.products[0].0);
        row.set(1, relation.products[0].1);
        row.set(2, relation.products[1].0);
        row.set(3, relation.products[1].1);
        row.set(4, relation.products[2].0);
        row.set(7, relation.products[2].1);
        row.set(5, relation.output);
        row.set(6, relation.accumulator);
        rows_data.push(row);
    }
    let dense_rows_count =
        rows_data.len() - binding_rows - range_rows_count - original_sparse_rows - lookup_only_rows;

    for relation in wide_rows {
        // Wide carry equality is a Dense row:
        // left*1 + right*1 + carry_out*(-B) + carry_in - constant = 0.
        let mut row = AssignedRow::zero(Opcode::Dense);
        row.set(0, relation.left);
        row.set(1, one);
        row.set(2, relation.right);
        row.set(3, one);
        row.set(4, relation.carry_out);
        row.set(5, relation.constant);
        row.set(6, relation.carry_in);
        row.set(7, negative_radix);
        rows_data.push(row);
    }
    let wide_rows_count = rows_data.len()
        - binding_rows
        - range_rows_count
        - original_sparse_rows
        - lookup_only_rows
        - dense_rows_count;

    let mut signs_by_key = HashMap::<(usize, usize), Vec<SignLane<F>>>::new();
    for lane in sign_lanes {
        signs_by_key
            .entry((lane.sign.id, lane.active.id))
            .or_default()
            .push(lane);
    }
    let mut sign_keys = signs_by_key.keys().copied().collect::<Vec<_>>();
    sign_keys.sort_unstable();
    let mut sign_rows_count = 0_usize;
    for sign_key in sign_keys {
        let lanes = signs_by_key
            .remove(&sign_key)
            .expect("sign key was collected from this map");
        for pair in lanes.chunks(2) {
            let mut row = AssignedRow::zero(Opcode::Sign);
            // q_sparse proves lane zero as
            // sign*(-2)*magnitude + magnitude - signed = 0. q_sign
            // independently proves both lane equations and sign booleanity.
            row.set(1, minus_two);
            row.set(2, pair[0].magnitude);
            row.set(3, pair[0].sign);
            row.set(5, pair[0].signed);
            row.set(6, pair[0].magnitude);
            row.set(7, pair[0].active);
            if let Some(second) = pair.get(1) {
                debug_assert_eq!(second.sign.id, pair[0].sign.id);
                debug_assert_eq!(second.active.id, pair[0].active.id);
                row.set(0, second.magnitude);
                row.set(4, second.signed);
            }
            rows_data.push(row);
            sign_rows_count += 1;
        }
    }

    let mut selects_by_bit = HashMap::<usize, Vec<SelectLane<F>>>::new();
    for lane in selects {
        selects_by_bit.entry(lane.bit.id).or_default().push(lane);
    }
    let mut bit_ids = selects_by_bit.keys().copied().collect::<Vec<_>>();
    bit_ids.sort_unstable();
    let mut selection_rows = 0_usize;
    for bit_id in bit_ids {
        let lanes = selects_by_bit
            .remove(&bit_id)
            .expect("bit id was collected from this map");
        for pair in lanes.chunks(2) {
            let mut row = AssignedRow::zero(Opcode::Select);
            row.set(0, pair[0].left);
            row.set(1, pair[0].right);
            row.set(2, pair[0].output);
            row.set(6, pair[0].bit);
            if let Some(second) = pair.get(1) {
                debug_assert_eq!(second.bit.id, pair[0].bit.id);
                row.set(3, second.left);
                row.set(4, second.right);
                row.set(5, second.output);
            }
            rows_data.push(row);
            selection_rows += 1;
        }
    }

    let semantic_rows = rows_data.len();
    let total_rows = semantic_rows.max(TABLE_ROWS);
    if total_rows > K16_MAX_ASSIGNED_ROWS {
        let rows = P256PackedAffineRowsV2 {
            binding_rows,
            caller_instance_rows,
            constant_instance_rows,
            range_rows: range_rows_count,
            sparse_rows: sparse_rows_count,
            dense_rows: dense_rows_count,
            wide_rows: wide_rows_count,
            sign_rows: sign_rows_count,
            selection_rows,
            lookup_only_rows,
            semantic_rows,
            table_rows: TABLE_ROWS,
            total_rows,
            range_lookups,
            modular_relations,
            complete_doublings,
            complete_additions,
            canonical_checks,
            zero_tests,
            maximum_quotient_bits,
            maximum_carry_bits,
            maximum_coefficient_bits,
        };
        return Err(P256PackedAffineFailureV2::RowCapacityExceeded {
            rows: Box::new(rows),
            maximum: K16_MAX_ASSIGNED_ROWS,
        });
    }
    rows_data.resize_with(total_rows, || AssignedRow::zero(Opcode::Disabled));
    let rows = P256PackedAffineRowsV2 {
        binding_rows,
        caller_instance_rows,
        constant_instance_rows,
        range_rows: range_rows_count,
        sparse_rows: sparse_rows_count,
        dense_rows: dense_rows_count,
        wide_rows: wide_rows_count,
        sign_rows: sign_rows_count,
        selection_rows,
        lookup_only_rows,
        semantic_rows,
        table_rows: TABLE_ROWS,
        total_rows,
        range_lookups,
        modular_relations,
        complete_doublings,
        complete_additions,
        canonical_checks,
        zero_tests,
        maximum_quotient_bits,
        maximum_carry_bits,
        maximum_coefficient_bits,
    };
    if instances.len() != binding_rows {
        return Err(P256PackedAffineFailureV2::InstanceBindingMismatch {
            instances: instances.len(),
            bindings: binding_rows,
        });
    }
    Ok(PackedTrace {
        rows_data,
        instances,
        rows,
    })
}

fn ensure_layout_constants<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
) -> (CellVar<F>, CellVar<F>, CellVar<F>) {
    let one = builder.one();
    let minus_two = builder.constant_i64(-2);
    let negative_radix = builder.constant_signed(-BigInt::from_biguint(Sign::Plus, radix()));
    (one, minus_two, negative_radix)
}

#[cfg(test)]
#[path = "p256_packed_affine_v2_tests.rs"]
mod tests;
