//! Private, non-authorizing packed affine P-256 ECDSA prototype.
//!
//! This privately declared module is source-settled evidence for a fail-closed
//! k=17 candidate, not a backend or GuardBundle eligibility path.
//! The circuit keeps the reviewed current-query proof shape (eight equality
//! advice columns, one direct instance column, four fixed queries, two lookup
//! arguments, degree seven, and 3,264 augmented IPA bytes) while replacing the
//! row-infeasible recursive-FMA transpose with a packed row machine. Production
//! row reporting remains closed until real synthesis, key generation, proof,
//! and verification evidence has been admitted by a separately governed path.
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

const K: u32 = 17;
const ADVICE_COLUMNS: usize = 8;
const PUBLIC_BYTES: usize = 65 + 32 + 64;
const LOOKUP_BITS: usize = 15;
const RANGE_CHUNK_BITS: [usize; 11] = [2, 4, 6, 8, 9, 10, 11, 12, 13, 14, 15];
const TABLE_ROWS: usize = 65_365;

/// Canonical fixed-table order: the disabled sentinel, then every typed width
/// in [`RANGE_CHUNK_BITS`] order with its values in ascending integer order.
fn typed_range_table_rows_v3() -> impl Iterator<Item = (u64, u64)> {
    std::iter::once((0_u64, 0_u64)).chain(RANGE_CHUNK_BITS.into_iter().flat_map(|bits| {
        let tag = u64::try_from(bits).expect("range width fits u64");
        (0_u64..(1_u64 << bits)).map(move |value| (tag * value, tag * tag * value))
    }))
}

const LIMB_BITS: usize = 86;
const LIMB_WIDTHS: [usize; 3] = [86, 86, 84];
const LIMBS: usize = 3;
const SLOPE_QUOTIENT_BITS: usize = 258;
const MAXIMUM_CARRY_BIAS_BITS: usize = 90;
// A coefficient accumulator contains at most nine gated 86-by-86-bit
// products, three 86-by-86-bit quotient products, and one 86-by-90-bit
// radix-carry product. Including signs and additions keeps its absolute value
// below 2^176. This witness-independent bound leaves more than 70 bits of
// headroom in either Pasta scalar field.
const PACKED_COEFFICIENT_BOUND_BITS: usize = 176;
const WINDOW_BITS: usize = 4;
const WINDOWS: usize = 256 / WINDOW_BITS;
const K17_MAX_ASSIGNED_ROWS: usize = (1 << K) - 9;
const P256_PACKED_AFFINE_V3_SEMANTIC_ROWS: usize = 108_877;
const P256_PACKED_AFFINE_V3_RESERVED_ROWS: usize = 16_384;
const P256_PACKED_AFFINE_V3_UPPER_ROWS: usize =
    P256_PACKED_AFFINE_V3_SEMANTIC_ROWS + P256_PACKED_AFFINE_V3_RESERVED_ROWS;
const P256_PACKED_AFFINE_V3_HEADROOM_ROWS: usize =
    K17_MAX_ASSIGNED_ROWS - P256_PACKED_AFFINE_V3_UPPER_ROWS;

const AGGREGATE_SLOPE_RELATIONS: usize = 398;
const X_RELATIONS: usize = 398;
const Y_RELATIONS: usize = 398;
const ADD_Y_SUM_RELATIONS: usize = 135;
const BASE_PRODUCT_RELATIONS: usize = 1;
const CURVE_RELATIONS: usize = 1;
const SCALAR_PRODUCT_RELATIONS: usize = 3;

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
pub(super) struct P256PackedAffineShapeV3 {
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

pub(super) const P256_PACKED_AFFINE_SHAPE_V3: P256PackedAffineShapeV3 = P256PackedAffineShapeV3 {
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
    proof_points: 59,
    proof_scalars: 42,
    raw_proof_bytes: 3_232,
    augmented_proof_bytes: 3_264,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct P256PackedAffineRowsV3 {
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
    pub(super) padding_rows: usize,
    pub(super) semantic_rows: usize,
    pub(super) reserved_rows: usize,
    pub(super) upper_rows: usize,
    pub(super) headroom_rows: usize,
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
    pub(super) relation_counts: [usize; ModularRelationKind::COUNT],
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum P256PackedAffineFailureV3 {
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
        rows: Box<P256PackedAffineRowsV3>,
        maximum: usize,
    },
    InstanceBindingMismatch {
        instances: usize,
        bindings: usize,
    },
}

/// Move-only exact-statement source. Implementations must either fill all 161
/// bytes or fail; there is no truncating or retrying parser path.
pub(super) trait P256PackedStatementSourceV3 {
    fn read_exact_statement(
        &mut self,
        destination: &mut [u8; PUBLIC_BYTES],
    ) -> Result<(), &'static str>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct P256PackedAffineEcdsaCircuitV3<F> {
    sec1_uncompressed: [u8; 65],
    digest: [u8; 32],
    signature: [u8; 64],
    _field: PhantomData<F>,
}

impl<F> Default for P256PackedAffineEcdsaCircuitV3<F> {
    fn default() -> Self {
        Self {
            sec1_uncompressed: [0; 65],
            digest: [0; 32],
            signature: [0; 64],
            _field: PhantomData,
        }
    }
}

impl<F: BigPrimeField> P256PackedAffineEcdsaCircuitV3<F> {
    pub(super) fn new(sec1_uncompressed: [u8; 65], digest: [u8; 32], signature: [u8; 64]) -> Self {
        Self {
            sec1_uncompressed,
            digest,
            signature,
            _field: PhantomData,
        }
    }

    pub(super) fn from_source(mut source: impl P256PackedStatementSourceV3) -> Result<Self, Error> {
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

    pub(super) fn row_report(&self) -> Result<P256PackedAffineRowsV3, Error> {
        // A source-level ledger is not synthesized evidence. This public-to-
        // the-parent-module diagnostic deliberately stays closed until a
        // separately reviewed backend admits both Pasta keygen/prove/verify
        // artifacts for this exact circuit identity.
        Err(Error::Synthesis)
    }

    #[cfg(test)]
    fn trace_diagnostic_for_test(
        &self,
    ) -> Result<P256PackedAffineRowsV3, P256PackedAffineFailureV3> {
        match self.build_trace_diagnostic() {
            Ok(trace) => Ok(trace.rows),
            Err(P256PackedAffineFailureV3::RowCapacityExceeded { rows, .. }) => Ok(*rows),
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    fn trace_and_topology_for_test(
        &self,
    ) -> Result<(P256PackedAffineRowsV3, CanonicalTraceTopologyV3), P256PackedAffineFailureV3> {
        let trace = self.build_trace_diagnostic()?;
        let rows = trace.rows;
        let topology = trace.canonical_topology_descriptor()?;
        Ok((rows, topology))
    }

    #[cfg(test)]
    fn instance_partition_for_test(&self) -> Result<(Vec<F>, Vec<F>), P256PackedAffineFailureV3> {
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

    fn build_trace_diagnostic(&self) -> Result<PackedTrace<F>, P256PackedAffineFailureV3> {
        self.build_builder_diagnostic()?.finish()
    }

    fn build_builder_diagnostic(&self) -> Result<PackedBuilder<F>, P256PackedAffineFailureV3> {
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
pub(super) struct P256PackedAffineConfigV3 {
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

impl P256PackedAffineConfigV3 {
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

        meta.set_minimum_degree(P256_PACKED_AFFINE_SHAPE_V3.degree);
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

impl<F: BigPrimeField> Circuit<F> for P256PackedAffineEcdsaCircuitV3<F> {
    type Config = P256PackedAffineConfigV3;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        P256PackedAffineConfigV3::configure(meta)
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
    bits: usize,
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

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalTopologyRowV3 {
    opcode: Opcode,
    range_bits: usize,
    equality_alias_classes: [Option<usize>; ADVICE_COLUMNS],
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalTraceTopologyV3 {
    rows: Vec<CanonicalTopologyRowV3>,
    equality_classes: usize,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct IntegerInterval {
    lower: BigInt,
    upper: BigInt,
}

impl IntegerInterval {
    fn singleton(value: BigInt) -> Self {
        Self {
            lower: value.clone(),
            upper: value,
        }
    }

    fn zero() -> Self {
        Self::singleton(BigInt::from(0))
    }

    fn hull(intervals: impl IntoIterator<Item = Self>) -> Self {
        intervals
            .into_iter()
            .reduce(|left, right| Self {
                lower: left.lower.min(right.lower),
                upper: left.upper.max(right.upper),
            })
            .expect("an interval hull is non-empty")
    }

    fn add(&self, right: &Self) -> Self {
        Self {
            lower: &self.lower + &right.lower,
            upper: &self.upper + &right.upper,
        }
    }

    fn subtract(&self, right: &Self) -> Self {
        Self {
            lower: &self.lower - &right.upper,
            upper: &self.upper - &right.lower,
        }
    }

    fn multiply(&self, right: &Self) -> Self {
        let products = [
            &self.lower * &right.lower,
            &self.lower * &right.upper,
            &self.upper * &right.lower,
            &self.upper * &right.upper,
        ];
        Self {
            lower: products.iter().min().expect("four products").clone(),
            upper: products.iter().max().expect("four products").clone(),
        }
    }

    fn span(&self) -> BigUint {
        (&self.upper - &self.lower)
            .to_biguint()
            .expect("ordered interval has a nonnegative span")
    }

    fn contains(&self, value: &BigInt) -> bool {
        value >= &self.lower && value <= &self.upper
    }

    fn signed_bits(&self) -> usize {
        usize::try_from(self.lower.magnitude().max(self.upper.magnitude()).bits())
            .unwrap_or(usize::MAX)
    }

    fn bias_bits(&self) -> usize {
        usize::try_from(self.span().bits()).unwrap_or(usize::MAX)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
enum ModularRelationKind {
    AggregateSlope = 0,
    X = 1,
    Y = 2,
    AddYSum = 3,
    BaseProduct = 4,
    Curve = 5,
    ScalarProduct = 6,
}

impl ModularRelationKind {
    const COUNT: usize = 7;

    const EXPECTED_COUNTS: [usize; Self::COUNT] = [
        AGGREGATE_SLOPE_RELATIONS,
        X_RELATIONS,
        Y_RELATIONS,
        ADD_Y_SUM_RELATIONS,
        BASE_PRODUCT_RELATIONS,
        CURVE_RELATIONS,
        SCALAR_PRODUCT_RELATIONS,
    ];
}

#[derive(Clone, Debug)]
struct PolyFactor<F> {
    cell: Option<CellVar<F>>,
    integer: BigInt,
    lower: BigInt,
    upper: BigInt,
}

impl<F: Copy> From<&BoundedCell<F>> for PolyFactor<F> {
    fn from(value: &BoundedCell<F>) -> Self {
        Self {
            cell: Some(value.cell),
            integer: bigint(&value.integer),
            lower: BigInt::from(0),
            upper: (BigInt::from(1) << value.bits) - 1,
        }
    }
}

impl<F: Copy> From<&BoolVar<F>> for PolyFactor<F> {
    fn from(value: &BoolVar<F>) -> Self {
        Self {
            cell: Some(value.cell),
            integer: BigInt::from(u8::from(value.value)),
            lower: BigInt::from(0),
            upper: BigInt::from(1),
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

    fn interval(&self) -> IntegerInterval {
        let mut interval = IntegerInterval::singleton(BigInt::from(self.coefficient));
        for factor in &self.factors {
            interval = interval.multiply(&IntegerInterval {
                lower: factor.lower.clone(),
                upper: factor.upper.clone(),
            });
        }
        interval
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
                    lower: bigint(&limb),
                    upper: bigint(&limb),
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

    fn coefficient_intervals(&self) -> [IntegerInterval; 2 * LIMBS - 1] {
        std::array::from_fn(|index| {
            self.coefficients[index]
                .iter()
                .map(PolyTerm::interval)
                .fold(IntegerInterval::zero(), |sum, term| sum.add(&term))
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
    derived_constants: HashMap<BigUint, CellVar<F>>,
    family_offset_constants:
        [Option<(BigInt, CellVar<F>)>; ModularRelationKind::COUNT * (2 * LIMBS - 1)],
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
    relation_counts: [usize; ModularRelationKind::COUNT],
}

impl<F: BigPrimeField> PackedBuilder<F> {
    fn new() -> Self {
        Self {
            next_id: 0,
            caller_instances: Vec::new(),
            constant_instances: Vec::new(),
            constants: HashMap::new(),
            derived_constants: HashMap::new(),
            family_offset_constants: std::array::from_fn(|_| None),
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
            relation_counts: [0; ModularRelationKind::COUNT],
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

    fn family_offset_constant(
        &mut self,
        kind: ModularRelationKind,
        coefficient: usize,
        value: BigInt,
    ) -> CellVar<F> {
        assert!(
            usize::try_from(value.magnitude().bits()).unwrap_or(usize::MAX)
                <= PACKED_COEFFICIENT_BOUND_BITS,
            "family offset must fit the reviewed native lift"
        );
        let index = kind as usize * (2 * LIMBS - 1) + coefficient;
        if let Some((expected, cell)) = &self.family_offset_constants[index] {
            assert_eq!(expected, &value, "family offset is topology-constant");
            return *cell;
        }
        let cell = self.witness_signed(&value);
        self.constant_instances.push(cell);
        self.family_offset_constants[index] = Some((value, cell));
        cell
    }

    fn constant_big(&mut self, value: impl Into<BigUint>) -> CellVar<F> {
        self.constant_signed(BigInt::from_biguint(Sign::Plus, value.into()))
    }

    fn constant_i64(&mut self, value: i64) -> CellVar<F> {
        self.constant_signed(BigInt::from(value))
    }

    fn derived_big(&mut self, value: &BigUint) -> CellVar<F> {
        if let Some(cell) = self.derived_constants.get(value) {
            return *cell;
        }
        if value == &BigUint::from(0_u8) {
            return self.zero();
        }
        if value == &BigUint::from(1_u8) {
            return self.one();
        }
        let zero = self.zero();
        let one = self.one();
        let mut accumulator = zero;
        for bit in (0..usize::try_from(value.bits()).unwrap_or(usize::MAX)).rev() {
            accumulator = self.add(accumulator, accumulator);
            if ((value >> bit) & BigUint::from(1_u8)) == BigUint::from(1_u8) {
                accumulator = self.add(accumulator, one);
            }
        }
        self.derived_constants.insert(value.clone(), accumulator);
        accumulator
    }

    fn derived_uint_widths(&mut self, value: BigUint, widths: [usize; LIMBS]) -> UintVar<F> {
        let one = self.one();
        let integers = decompose_limbs(&value);
        let limbs = std::array::from_fn(|index| {
            let integer = integers[index].clone();
            BoundedCell {
                cell: self.derived_big(&integer),
                integer,
                bits: widths[index],
                chunks: Vec::new(),
                active: one,
            }
        });
        UintVar { limbs, value }
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
    ) -> Result<BoundedCell<F>, P256PackedAffineFailureV3> {
        let actual_bits = usize::try_from(integer.bits()).unwrap_or(usize::MAX);
        if actual_bits > bits || bits > 6 * LOOKUP_BITS {
            return Err(P256PackedAffineFailureV3::IntegerBound {
                witness,
                actual_bits,
                maximum_bits: bits.min(6 * LOOKUP_BITS),
            });
        }
        if !active.value && integer != BigUint::from(0_u8) {
            return Err(P256PackedAffineFailureV3::Source(
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
            bits,
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
    ) -> Result<BoundedCell<F>, P256PackedAffineFailureV3> {
        let actual_bits = usize::try_from(integer.bits()).unwrap_or(usize::MAX);
        if actual_bits > bits || bits > 6 * LOOKUP_BITS {
            return Err(P256PackedAffineFailureV3::IntegerBound {
                witness,
                actual_bits,
                maximum_bits: bits.min(6 * LOOKUP_BITS),
            });
        }
        if !active.value && integer != BigUint::from(0_u8) {
            return Err(P256PackedAffineFailureV3::Source(
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
            bits,
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
    ) -> Result<UintVar<F>, P256PackedAffineFailureV3> {
        self.load_uint_widths(value, LIMB_WIDTHS, active, witness)
    }

    fn load_uint_widths(
        &mut self,
        value: BigUint,
        widths: [usize; LIMBS],
        active: &BoolVar<F>,
        witness: &'static str,
    ) -> Result<UintVar<F>, P256PackedAffineFailureV3> {
        if usize::try_from(value.bits()).unwrap_or(usize::MAX) > widths.into_iter().sum() {
            return Err(P256PackedAffineFailureV3::IntegerBound {
                witness,
                actual_bits: usize::try_from(value.bits()).unwrap_or(usize::MAX),
                maximum_bits: widths.into_iter().sum(),
            });
        }
        let limbs = decompose_limbs(&value)
            .into_iter()
            .zip(widths)
            .map(|(limb, bits)| self.bounded(limb, bits, active, witness))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .unwrap_or_else(|_| panic!("P-256 uses exactly {LIMBS} limbs"));
        Ok(UintVar { limbs, value })
    }

    fn constant_uint(&mut self, value: BigUint) -> UintVar<F> {
        self.constant_uint_widths(value, LIMB_WIDTHS)
    }

    fn constant_uint_widths(&mut self, value: BigUint, widths: [usize; LIMBS]) -> UintVar<F> {
        let one = self.one();
        let integers = decompose_limbs(&value);
        let limbs = std::array::from_fn(|index| {
            let integer = integers[index].clone();
            BoundedCell {
                cell: self.constant_big(integer.clone()),
                integer,
                bits: widths[index],
                chunks: Vec::new(),
                active: one,
            }
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
    ) -> Result<Vec<RealizedTerm<F>>, P256PackedAffineFailureV3> {
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
            return Err(P256PackedAffineFailureV3::Source(
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
    ) -> Result<(), P256PackedAffineFailureV3> {
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
                return Err(P256PackedAffineFailureV3::UnsafeNativeCoefficient { witness, bits });
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
                    return Err(P256PackedAffineFailureV3::UnsafeNativeCoefficient {
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

    fn finish(self) -> Result<PackedTrace<F>, P256PackedAffineFailureV3> {
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

#[derive(Clone, Debug)]
struct BiasedCarry<F> {
    encoded: Option<BoundedCell<F>>,
}

impl<F: Copy> BiasedCarry<F> {
    fn cell(&self, zero: CellVar<F>) -> CellVar<F> {
        self.encoded.as_ref().map_or(zero, |encoded| encoded.cell)
    }

    fn encoded_integer(&self) -> BigInt {
        self.encoded
            .as_ref()
            .map_or_else(|| BigInt::from(0), |encoded| bigint(&encoded.integer))
    }
}

#[derive(Clone, Debug)]
enum QuotientWitness<F> {
    Signed(SignedMagnitude<F>),
    Offset {
        encoded: UintVar<F>,
        lower: BigInt,
        active: BoolVar<F>,
    },
    Boolean(BoolVar<F>),
}

const EXPECTED_CARRY_SIGNED_BITS: [[usize; 4]; ModularRelationKind::COUNT] = [
    [88, 89, 89, 87],
    [86, 87, 87, 85],
    [87, 88, 87, 86],
    [1, 1, 0, 0],
    [86, 87, 87, 85],
    [87, 88, 87, 86],
    [86, 87, 87, 85],
];
const EXPECTED_CARRY_BIAS_BITS: [[usize; 4]; ModularRelationKind::COUNT] = [
    [89, 90, 90, 88],
    [87, 88, 87, 86],
    [88, 89, 88, 87],
    [2, 2, 0, 0],
    [87, 88, 87, 86],
    [88, 89, 88, 87],
    [87, 88, 88, 86],
];

// Exact output of the reviewed canonical-limb interval propagation. The
// propagation splits the aggregate slope into mutually exclusive chord and
// tangent branches, splits signed-magnitude quotient signs before taking a
// hull, carries the q=(Q+Lq) correlation for biased quotients, and uses the
// actual P-256 p/n radix limbs rather than a generic 256-bit cap.
const REVIEWED_CARRY_INTERVALS_I128: [[(i128, i128); 4]; ModularRelationKind::COUNT] = [
    [
        (
            -309_485_009_821_345_068_724_781_048,
            232_113_757_366_008_801_543_585_786,
        ),
        (
            -541_598_767_187_353_870_268_367_860,
            386_856_262_276_681_335_905_977_335,
        ),
        (
            -406_199_075_349_983_006_064_378_877,
            309_485_009_785_316_271_714_206_718,
        ),
        (
            -135_399_691_765_313_270_182_838_786,
            96_714_065_546_652_335_844_885_249,
        ),
    ],
    [
        (
            -77_371_252_455_336_267_181_195_263,
            77_371_252_455_336_267_181_195_263,
        ),
        (
            -77_371_252_455_336_267_181_196_288,
            154_742_504_910_672_534_362_390_525,
        ),
        (
            -38_685_626_218_660_934_337_954_815,
            116_056_878_673_997_201_519_149_055,
        ),
        (
            -19_342_813_109_330_467_168_977_151,
            38_685_626_218_660_934_337_953_792,
        ),
    ],
    [
        (
            -77_371_252_455_336_267_181_195_264,
            154_742_504_910_672_534_362_390_523,
        ),
        (
            -232_113_757_366_008_801_543_584_766,
            154_742_504_910_672_534_362_392_571,
        ),
        (
            -135_399_691_783_327_668_688_126_975,
            154_742_504_892_658_135_857_102_846,
        ),
        (
            -58_028_439_327_991_401_506_930_689,
            38_685_626_218_660_934_337_954_304,
        ),
    ],
    [(-1, 1), (-1, 1), (0, 0), (0, 0)],
    [
        (
            -77_371_252_455_336_267_181_195_263,
            77_371_252_455_336_267_181_195_262,
        ),
        (
            -77_371_252_455_336_267_181_196_286,
            154_742_504_910_672_534_362_390_525,
        ),
        (
            -38_685_626_218_660_934_337_954_815,
            116_056_878_673_997_201_519_149_055,
        ),
        (
            -19_342_813_109_330_467_168_977_151,
            38_685_626_218_660_934_337_953_792,
        ),
    ],
    [
        (
            -77_371_252_455_336_267_181_195_264,
            154_742_504_910_672_534_362_390_524,
        ),
        (
            -232_113_757_366_008_801_543_584_764,
            154_742_504_910_672_534_362_392_574,
        ),
        (
            -135_399_691_783_327_668_688_126_975,
            154_742_504_892_658_135_857_102_847,
        ),
        (
            -58_028_439_327_991_401_506_930_689,
            38_685_626_218_660_934_337_954_304,
        ),
    ],
    [
        (
            -28_553_880_287_938_765_337_601_361,
            77_371_252_455_336_267_181_195_262,
        ),
        (
            -105_925_132_743_273_879_788_444_653,
            154_742_504_910_672_534_362_390_525,
        ),
        (
            -103_852_535_634_988_218_373_043_423,
            116_056_878_673_997_201_519_149_053,
        ),
        (
            -38_685_626_218_660_646_155_365_865,
            38_685_626_218_660_934_337_953_790,
        ),
    ],
];

#[cfg(test)]
const REVIEWED_CHORD_CARRY_INTERVALS_I128: [(i128, i128); 4] = [
    (
        -154_742_504_910_672_534_362_390_525,
        154_742_504_910_672_534_362_390_525,
    ),
    (
        -232_113_757_366_008_801_543_586_811,
        232_113_757_366_008_801_543_586_811,
    ),
    (
        -154_742_504_892_658_135_857_103_871,
        154_742_504_892_658_135_857_103_871,
    ),
    (
        -58_028_439_327_991_401_506_930_944,
        58_028_439_327_991_401_506_930_944,
    ),
];

#[cfg(test)]
const MAXIMUM_NATIVE_COEFFICIENT_DECIMAL: &str =
    "83808349891103296941472259724068755535090837664824320";

fn widths_for_total_bits(bits: usize) -> [usize; LIMBS] {
    assert!(
        (2 * LIMB_BITS + 1..=3 * LIMB_BITS).contains(&bits),
        "packed three-limb width is 173 through 258 bits"
    );
    [LIMB_BITS, LIMB_BITS, bits - 2 * LIMB_BITS]
}

fn exact_cell_factor<F: Copy>(cell: CellVar<F>, integer: BigInt) -> PolyFactor<F> {
    PolyFactor {
        cell: Some(cell),
        lower: integer.clone(),
        upper: integer.clone(),
        integer,
    }
}

fn constant_factor<F>(integer: BigInt) -> PolyFactor<F> {
    PolyFactor {
        cell: None,
        lower: integer.clone(),
        upper: integer.clone(),
        integer,
    }
}

fn linear_term<F: Copy>(cell: CellVar<F>, integer: BigInt, coefficient: i64) -> PolyTerm<F> {
    PolyTerm {
        coefficient,
        factors: vec![exact_cell_factor(cell, integer)],
    }
}

fn scaled_term<F: Copy>(
    cell: CellVar<F>,
    integer: BigInt,
    scale: BigInt,
    coefficient: i64,
) -> PolyTerm<F> {
    PolyTerm {
        coefficient,
        factors: vec![exact_cell_factor(cell, integer), constant_factor(scale)],
    }
}

fn cell_product_term<F: Copy>(
    left: CellVar<F>,
    left_integer: BigInt,
    right: CellVar<F>,
    right_integer: BigInt,
    coefficient: i64,
) -> PolyTerm<F> {
    PolyTerm {
        coefficient,
        factors: vec![
            exact_cell_factor(left, left_integer),
            exact_cell_factor(right, right_integer),
        ],
    }
}

fn signed_radix_digits(value: &BigInt) -> [BigInt; LIMBS] {
    let sign = if value.sign() == Sign::Minus {
        -BigInt::from(1)
    } else {
        BigInt::from(1)
    };
    let digits = decompose_limbs(value.magnitude());
    digits.map(|digit| &sign * bigint(&digit))
}

fn quotient_interval(kind: ModularRelationKind, modulus: &BigUint) -> IntegerInterval {
    let modulus = bigint(modulus);
    match kind {
        ModularRelationKind::AggregateSlope => IntegerInterval {
            lower: -BigInt::from(3) * &modulus + 6,
            upper: BigInt::from(2) * &modulus - 4,
        },
        ModularRelationKind::X => IntegerInterval {
            lower: BigInt::from(-2),
            upper: &modulus - 2,
        },
        ModularRelationKind::Y => IntegerInterval {
            lower: -&modulus + 1,
            upper: &modulus - 2,
        },
        ModularRelationKind::AddYSum => IntegerInterval {
            lower: BigInt::from(0),
            upper: BigInt::from(1),
        },
        ModularRelationKind::BaseProduct | ModularRelationKind::ScalarProduct => IntegerInterval {
            lower: BigInt::from(0),
            upper: &modulus - 2,
        },
        ModularRelationKind::Curve => IntegerInterval {
            lower: -&modulus + 2,
            upper: modulus,
        },
    }
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
    maximum_bits: usize,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<SignedMagnitude<F>, P256PackedAffineFailureV3> {
    let negative = value.sign() == Sign::Minus;
    let magnitude_value = if active.value {
        value.magnitude().clone()
    } else {
        BigUint::from(0_u8)
    };
    let bits = usize::try_from(magnitude_value.bits()).unwrap_or(usize::MAX);
    if bits > maximum_bits {
        return Err(P256PackedAffineFailureV3::IntegerBound {
            witness,
            actual_bits: bits,
            maximum_bits,
        });
    }
    builder.maximum_quotient_bits = builder.maximum_quotient_bits.max(maximum_bits);
    let sign_cell = builder.witness_fe(F::from(u64::from(negative && active.value)));
    let sign = BoolVar {
        cell: sign_cell,
        value: negative && active.value,
    };
    let magnitude = builder.load_uint_widths(
        magnitude_value,
        widths_for_total_bits(maximum_bits),
        active,
        witness,
    )?;
    let signed_limbs = std::array::from_fn(|index| {
        let signed = builder.witness_fe(if sign.value {
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

fn constrain_exact_variable_sum<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    left: &UintVar<F>,
    right: &UintVar<F>,
    constant: &UintVar<F>,
) {
    builder.canonical_checks += 1;
    let radix = radix();
    let mut carry = builder.constant_bool(false);
    for index in 0..LIMBS {
        let integer_sum =
            &left.limbs[index].integer + &right.limbs[index].integer + u8::from(carry.value);
        let next_value = integer_sum >= radix;
        let next = if index + 1 == LIMBS {
            builder.constant_bool(false)
        } else {
            builder.boolean(next_value)
        };
        builder.wide_rows.push(WideRow {
            left: left.limbs[index].cell,
            right: right.limbs[index].cell,
            carry_in: carry.cell,
            carry_out: next.cell,
            constant: constant.limbs[index].cell,
        });
        carry = next;
    }
}

fn constrain_uint_at_most<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    value: &UintVar<F>,
    maximum: &BigUint,
    witness: &'static str,
) -> Result<(), P256PackedAffineFailureV3> {
    if value.value > *maximum {
        return Err(P256PackedAffineFailureV3::IntegerBound {
            witness,
            actual_bits: usize::try_from(value.value.bits()).unwrap_or(usize::MAX),
            maximum_bits: usize::try_from(maximum.bits()).unwrap_or(usize::MAX),
        });
    }
    let widths = value.limbs.clone().map(|limb| limb.bits);
    let always = builder.constant_bool(true);
    let slack = builder.load_uint_widths(maximum - &value.value, widths, &always, witness)?;
    let constant = builder.derived_uint_widths(maximum.clone(), widths);
    constrain_exact_variable_sum(builder, value, &slack, &constant);
    Ok(())
}

fn constrain_signed_conditional_interval<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    quotient: &SignedMagnitude<F>,
    interval: &IntegerInterval,
    witness: &'static str,
) -> Result<(), P256PackedAffineFailureV3> {
    let positive_maximum = interval
        .upper
        .to_biguint()
        .ok_or(P256PackedAffineFailureV3::Source(
            "signed quotient upper endpoint is negative",
        ))?;
    let negative_maximum =
        (-&interval.lower)
            .to_biguint()
            .ok_or(P256PackedAffineFailureV3::Source(
                "signed quotient lower endpoint is positive",
            ))?;
    let maximum_bits =
        usize::try_from(positive_maximum.bits().max(negative_maximum.bits())).unwrap_or(usize::MAX);
    let widths = widths_for_total_bits(maximum_bits);
    let positive = builder.derived_uint_widths(positive_maximum.clone(), widths);
    let negative = builder.derived_uint_widths(negative_maximum.clone(), widths);
    let selected = select_uint(builder, &positive, &quotient.sign, &negative);
    if quotient.magnitude.value > selected.value {
        return Err(P256PackedAffineFailureV3::IntegerBound {
            witness,
            actual_bits: usize::try_from(quotient.magnitude.value.bits()).unwrap_or(usize::MAX),
            maximum_bits,
        });
    }
    let always = builder.constant_bool(true);
    let slack = builder.load_uint_widths(
        &selected.value - &quotient.magnitude.value,
        widths,
        &always,
        witness,
    )?;
    constrain_exact_variable_sum(builder, &quotient.magnitude, &slack, &selected);
    Ok(())
}

fn biased_carry_witness<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    value: &BigInt,
    interval: IntegerInterval,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<BiasedCarry<F>, P256PackedAffineFailureV3> {
    let actual = if active.value {
        value.clone()
    } else {
        BigInt::from(0)
    };
    if active.value && !interval.contains(&actual) {
        return Err(P256PackedAffineFailureV3::Source(
            "derived modular carry escaped its proven exact interval",
        ));
    }
    if interval.lower == BigInt::from(0) && interval.upper == BigInt::from(0) {
        if actual != BigInt::from(0) {
            return Err(P256PackedAffineFailureV3::Source(
                "terminal modular carry was nonzero",
            ));
        }
        return Ok(BiasedCarry { encoded: None });
    }

    let encoded_integer = if active.value {
        (&actual - &interval.lower)
            .to_biguint()
            .expect("contained carry has a nonnegative bias")
    } else {
        BigUint::from(0_u8)
    };
    let span = interval.span();
    let bits = interval.bias_bits();
    if bits > MAXIMUM_CARRY_BIAS_BITS {
        return Err(P256PackedAffineFailureV3::IntegerBound {
            witness,
            actual_bits: bits,
            maximum_bits: MAXIMUM_CARRY_BIAS_BITS,
        });
    }
    builder.maximum_carry_bits = builder.maximum_carry_bits.max(bits);
    let encoded = builder.bounded(encoded_integer, bits, active, witness)?;
    let always = builder.constant_bool(true);
    let slack = builder.bounded(
        &span - &encoded.integer,
        bits,
        &always,
        "biased carry upper-bound slack",
    )?;
    let span_cell = builder.derived_big(&span);
    builder.emit_linear_equation(&[(encoded.cell, 1), (slack.cell, 1), (span_cell, -1)]);

    Ok(BiasedCarry {
        encoded: Some(encoded),
    })
}

fn offset_quotient_witness<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    quotient_value: &BigInt,
    interval: &IntegerInterval,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<UintVar<F>, P256PackedAffineFailureV3> {
    let encoded_value =
        if active.value {
            (quotient_value - &interval.lower).to_biguint().ok_or(
                P256PackedAffineFailureV3::Source("quotient bias underflowed its exact interval"),
            )?
        } else {
            BigUint::from(0_u8)
        };
    let span = interval.span();
    let bits = usize::try_from(span.bits()).unwrap_or(usize::MAX);
    builder.maximum_quotient_bits = builder.maximum_quotient_bits.max(bits);
    let encoded =
        builder.load_uint_widths(encoded_value, widths_for_total_bits(bits), active, witness)?;
    constrain_uint_at_most(builder, &encoded, &span, witness)?;
    Ok(encoded)
}

fn quotient_witness<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    quotient_value: &BigInt,
    interval: &IntegerInterval,
    kind: ModularRelationKind,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<QuotientWitness<F>, P256PackedAffineFailureV3> {
    if active.value && !interval.contains(quotient_value) {
        return Err(P256PackedAffineFailureV3::Source(
            "modular quotient escaped its exact family interval",
        ));
    }
    match kind {
        ModularRelationKind::AggregateSlope => {
            let signed = signed_uint_witness(
                builder,
                quotient_value,
                SLOPE_QUOTIENT_BITS,
                active,
                witness,
            )?;
            constrain_signed_conditional_interval(builder, &signed, interval, witness)?;
            Ok(QuotientWitness::Signed(signed))
        }
        ModularRelationKind::AddYSum => {
            if quotient_value != &BigInt::from(0) && quotient_value != &BigInt::from(1) {
                return Err(P256PackedAffineFailureV3::Source(
                    "addition y-sum quotient was not boolean",
                ));
            }
            Ok(QuotientWitness::Boolean(builder.boolean(
                active.value && quotient_value == &BigInt::from(1),
            )))
        }
        ModularRelationKind::X
        | ModularRelationKind::Y
        | ModularRelationKind::BaseProduct
        | ModularRelationKind::Curve
        | ModularRelationKind::ScalarProduct => {
            let encoded =
                offset_quotient_witness(builder, quotient_value, interval, active, witness)?;
            Ok(QuotientWitness::Offset {
                encoded,
                lower: interval.lower.clone(),
                active: active.clone(),
            })
        }
    }
}

fn quotient_coefficient_intervals<F>(
    quotient: &QuotientWitness<F>,
    modulus_limbs: &[BigUint; LIMBS],
) -> [IntegerInterval; 2 * LIMBS - 1] {
    std::array::from_fn(|coefficient| {
        (0..LIMBS)
            .filter_map(|left| {
                coefficient
                    .checked_sub(left)
                    .filter(|right| *right < LIMBS)
                    .map(|right| (left, right))
            })
            .fold(IntegerInterval::zero(), |sum, (left, right)| {
                let modulus = bigint(&modulus_limbs[right]);
                let term = match quotient {
                    QuotientWitness::Signed(signed) => {
                        let maximum = (BigInt::from(1) << signed.magnitude.limbs[left].bits) - 1;
                        IntegerInterval {
                            lower: -&maximum * &modulus,
                            upper: maximum * modulus,
                        }
                    }
                    QuotientWitness::Offset { encoded, lower, .. } => {
                        let maximum = (BigInt::from(1) << encoded.limbs[left].bits) - 1;
                        let variable = IntegerInterval {
                            lower: BigInt::from(0),
                            upper: maximum * &modulus,
                        };
                        let offset = &signed_radix_digits(lower)[left] * modulus;
                        variable.add(&IntegerInterval {
                            lower: offset.clone().min(BigInt::from(0)),
                            upper: offset.max(BigInt::from(0)),
                        })
                    }
                    QuotientWitness::Boolean(_) => IntegerInterval {
                        lower: BigInt::from(0),
                        upper: modulus,
                    },
                };
                sum.add(&term)
            })
    })
}

fn aggregate_slope_coefficient_intervals() -> [IntegerInterval; 2 * LIMBS - 1] {
    let limb_maxima = LIMB_WIDTHS.map(|bits| (BigInt::from(1) << bits) - 1);
    let products: [BigInt; 2 * LIMBS - 1] = std::array::from_fn(|coefficient| {
        (0..LIMBS)
            .filter_map(|left| {
                coefficient
                    .checked_sub(left)
                    .filter(|right| *right < LIMBS)
                    .map(|right| &limb_maxima[left] * &limb_maxima[right])
            })
            .fold(BigInt::from(0), |sum, term| sum + term)
    });
    std::array::from_fn(|coefficient| {
        let linear = limb_maxima
            .get(coefficient)
            .cloned()
            .unwrap_or_else(|| BigInt::from(0));
        let chord_radius = BigInt::from(2) * (&products[coefficient] + &linear);
        let chord = IntegerInterval {
            lower: -&chord_radius,
            upper: chord_radius,
        };
        let tangent_constant = if coefficient == 0 {
            BigInt::from(3)
        } else {
            BigInt::from(0)
        };
        let tangent = IntegerInterval {
            lower: -BigInt::from(3) * &products[coefficient] + &tangent_constant,
            upper: BigInt::from(2) * &products[coefficient] + tangent_constant,
        };
        IntegerInterval::hull([IntegerInterval::zero(), chord, tangent])
    })
}

fn div_floor(value: &BigInt, divisor: &BigInt) -> BigInt {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder.sign() == Sign::Minus {
        quotient - 1
    } else {
        quotient
    }
}

fn div_ceil(value: &BigInt, divisor: &BigInt) -> BigInt {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder.sign() == Sign::Plus {
        quotient + 1
    } else {
        quotient
    }
}

fn derive_carry_intervals<F: Copy>(
    expression: &RadixExpression<F>,
    quotient: &QuotientWitness<F>,
    modulus_limbs: &[BigUint; LIMBS],
    kind: ModularRelationKind,
) -> Result<[IntegerInterval; 4], P256PackedAffineFailureV3> {
    let expression_intervals = if kind == ModularRelationKind::AggregateSlope {
        aggregate_slope_coefficient_intervals()
    } else {
        expression.coefficient_intervals()
    };
    let quotient_intervals = quotient_coefficient_intervals(quotient, modulus_limbs);
    let radix_integer = BigInt::from_biguint(Sign::Plus, radix());
    let mut previous = IntegerInterval::zero();
    let coarse_intervals = std::array::from_fn(|coefficient| {
        let numerator = expression_intervals[coefficient]
            .subtract(&quotient_intervals[coefficient])
            .add(&previous);
        let carry = IntegerInterval {
            lower: div_ceil(&numerator.lower, &radix_integer),
            upper: div_floor(&numerator.upper, &radix_integer),
        };
        previous = carry.clone();
        carry
    });
    let intervals =
        REVIEWED_CARRY_INTERVALS_I128[kind as usize].map(|(lower, upper)| IntegerInterval {
            lower: BigInt::from(lower),
            upper: BigInt::from(upper),
        });
    for (index, interval) in intervals.iter().enumerate() {
        if coarse_intervals[index].lower > interval.lower
            || coarse_intervals[index].upper < interval.upper
        {
            return Err(P256PackedAffineFailureV3::Source(
                "correlated carry refinement escaped the independent coarse hull",
            ));
        }
        if interval.signed_bits() != EXPECTED_CARRY_SIGNED_BITS[kind as usize][index]
            || interval.bias_bits() != EXPECTED_CARRY_BIAS_BITS[kind as usize][index]
        {
            return Err(P256PackedAffineFailureV3::Source(
                "mechanical carry interval width disagrees with the reviewed schedule",
            ));
        }
    }
    Ok(intervals)
}

fn quotient_coefficient_values<F>(
    quotient: &QuotientWitness<F>,
    modulus_limbs: &[BigUint; LIMBS],
) -> [BigInt; 2 * LIMBS - 1] {
    let digits: [BigInt; LIMBS] = match quotient {
        QuotientWitness::Signed(signed) => std::array::from_fn(|index| {
            let magnitude = bigint(&signed.magnitude.limbs[index].integer);
            if signed.sign.value {
                -magnitude
            } else {
                magnitude
            }
        }),
        QuotientWitness::Offset {
            encoded,
            lower,
            active,
        } => {
            let lower_digits = signed_radix_digits(lower);
            std::array::from_fn(|index| {
                bigint(&encoded.limbs[index].integer)
                    + BigInt::from(u8::from(active.value)) * &lower_digits[index]
            })
        }
        QuotientWitness::Boolean(bit) => std::array::from_fn(|index| {
            if index == 0 {
                BigInt::from(u8::from(bit.value))
            } else {
                BigInt::from(0)
            }
        }),
    };
    std::array::from_fn(|coefficient| {
        (0..LIMBS)
            .filter_map(|left| {
                coefficient
                    .checked_sub(left)
                    .filter(|right| *right < LIMBS)
                    .map(|right| &digits[left] * bigint(&modulus_limbs[right]))
            })
            .fold(BigInt::from(0), |sum, term| sum + term)
    })
}

fn append_quotient_terms<F: Copy>(
    terms: &mut Vec<PolyTerm<F>>,
    quotient: &QuotientWitness<F>,
    modulus_limbs: &[BigUint; LIMBS],
    coefficient: usize,
) {
    for left in 0..LIMBS {
        let Some(right) = coefficient.checked_sub(left).filter(|right| *right < LIMBS) else {
            continue;
        };
        let modulus = bigint(&modulus_limbs[right]);
        match quotient {
            QuotientWitness::Signed(signed) => {
                let magnitude = bigint(&signed.magnitude.limbs[left].integer);
                let signed_integer = if signed.sign.value {
                    -magnitude
                } else {
                    magnitude
                };
                terms.push(scaled_term(
                    signed.signed_limbs[left],
                    signed_integer,
                    modulus,
                    -1,
                ));
            }
            QuotientWitness::Offset { encoded, .. } => {
                terms.push(scaled_term(
                    encoded.limbs[left].cell,
                    bigint(&encoded.limbs[left].integer),
                    modulus,
                    -1,
                ));
            }
            QuotientWitness::Boolean(bit) => {
                if left == 0 {
                    terms.push(scaled_term(
                        bit.cell,
                        BigInt::from(u8::from(bit.value)),
                        modulus,
                        -1,
                    ));
                }
            }
        }
    }
}

fn family_coefficient_offset<F>(
    quotient: &QuotientWitness<F>,
    modulus_limbs: &[BigUint; LIMBS],
    carry_intervals: &[IntegerInterval; 4],
    coefficient: usize,
) -> BigInt {
    let quotient_lower = match quotient {
        QuotientWitness::Offset { lower, .. } => {
            let lower_digits = signed_radix_digits(lower);
            (0..LIMBS)
                .filter_map(|left| {
                    coefficient
                        .checked_sub(left)
                        .filter(|right| *right < LIMBS)
                        .map(|right| &lower_digits[left] * bigint(&modulus_limbs[right]))
                })
                .fold(BigInt::from(0), |sum, term| sum + term)
        }
        QuotientWitness::Signed(_) | QuotientWitness::Boolean(_) => BigInt::from(0),
    };
    let carry_in = coefficient
        .checked_sub(1)
        .and_then(|index| carry_intervals.get(index))
        .map_or_else(|| BigInt::from(0), |interval| interval.lower.clone());
    let carry_out = carry_intervals
        .get(coefficient)
        .map_or_else(|| BigInt::from(0), |interval| interval.lower.clone());
    -quotient_lower + carry_in - BigInt::from_biguint(Sign::Plus, radix()) * carry_out
}

fn constrain_modular_expression<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    expression: &RadixExpression<F>,
    modulus: &BigUint,
    active: &BoolVar<F>,
    kind: ModularRelationKind,
    witness: &'static str,
) -> Result<(), P256PackedAffineFailureV3> {
    let expression_integer = expression.integer();
    let modulus_integer = bigint(modulus);
    let quotient_value = if active.value {
        &expression_integer / &modulus_integer
    } else {
        BigInt::from(0)
    };
    let interval = quotient_interval(kind, modulus);
    let quotient = quotient_witness(builder, &quotient_value, &interval, kind, active, witness)?;
    let modulus_limbs = decompose_limbs(modulus);
    let expression_coefficients = expression.integer_coefficients();
    let quotient_coefficients = quotient_coefficient_values(&quotient, &modulus_limbs);
    let carry_intervals = derive_carry_intervals(expression, &quotient, &modulus_limbs, kind)?;
    let radix_integer = BigInt::from_biguint(Sign::Plus, radix());

    let mut carry_in = BigInt::from(0);
    let mut carry_values = Vec::with_capacity(2 * LIMBS - 2);
    for coefficient in 0..2 * LIMBS - 2 {
        let numerator =
            &expression_coefficients[coefficient] - &quotient_coefficients[coefficient] + &carry_in;
        let carry_out = &numerator / &radix_integer;
        carry_values.push(carry_out.clone());
        carry_in = carry_out;
    }
    let carries = carry_values
        .iter()
        .zip(carry_intervals.iter().cloned())
        .map(|(carry, interval)| {
            biased_carry_witness(builder, carry, interval, active, "biased modular carry")
        })
        .collect::<Result<Vec<_>, _>>()?;

    let zero = builder.zero();
    let active_integer = BigInt::from(u8::from(active.value));
    // Exactly five coefficient equations are emitted. Coefficients zero
    // through three propagate C0..C3; coefficient four has no carry-out cell,
    // so it is the mandatory terminal equation c4=0.
    for coefficient in 0..2 * LIMBS - 1 {
        let mut terms = expression.coefficients[coefficient].clone();
        append_quotient_terms(&mut terms, &quotient, &modulus_limbs, coefficient);
        if coefficient > 0 {
            let carry = &carries[coefficient - 1];
            terms.push(linear_term(carry.cell(zero), carry.encoded_integer(), 1));
        }
        if coefficient < carries.len() {
            let carry = &carries[coefficient];
            terms.push(scaled_term(
                carry.cell(zero),
                carry.encoded_integer(),
                radix_integer.clone(),
                -1,
            ));
        }
        let offset =
            family_coefficient_offset(&quotient, &modulus_limbs, &carry_intervals, coefficient);
        if offset != BigInt::from(0) {
            let offset_cell = builder.family_offset_constant(kind, coefficient, offset.clone());
            terms.push(cell_product_term(
                active.cell,
                active_integer.clone(),
                offset_cell,
                offset,
                1,
            ));
        }
        builder.realize_zero_sum(&terms, witness)?;
    }
    builder.modular_relations += 1;
    builder.relation_counts[kind as usize] += 1;
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
    _active: &BoolVar<F>,
    witness: &'static str,
) -> Result<(), P256PackedAffineFailureV3> {
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
) -> Result<BoolVar<F>, P256PackedAffineFailureV3> {
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
    kind: ModularRelationKind,
    witness: &'static str,
) -> Result<UintVar<F>, P256PackedAffineFailureV3> {
    let output = builder.load_uint(output_value, active, witness)?;
    constrain_canonical(builder, &output, modulus, active, witness)?;
    let mut expression = RadixExpression::new();
    expression.add_product(left, right, Some(active), 1);
    expression.add_linear(&output, Some(active), -1);
    constrain_modular_expression(builder, &expression, modulus, active, kind, witness)?;
    Ok(output)
}

fn modular_linear_reduction<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    terms: &[(&UintVar<F>, i64)],
    output_value: BigUint,
    modulus: &BigUint,
    active: &BoolVar<F>,
    witness: &'static str,
) -> Result<UintVar<F>, P256PackedAffineFailureV3> {
    let output = builder.load_uint(output_value, active, witness)?;
    constrain_canonical(builder, &output, modulus, active, witness)?;
    let mut expression = RadixExpression::new();
    for (value, coefficient) in terms {
        expression.add_linear(value, Some(active), *coefficient);
    }
    expression.add_linear(&output, Some(active), -1);
    constrain_modular_expression(
        builder,
        &expression,
        modulus,
        active,
        ModularRelationKind::AddYSum,
        witness,
    )?;
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
        bits: left.limbs[index].bits.max(right.limbs[index].bits),
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
) -> Result<AffineVar<F>, P256PackedAffineFailureV3> {
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
        ModularRelationKind::AggregateSlope,
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
        ModularRelationKind::X,
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
        ModularRelationKind::Y,
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
) -> Result<AffineVar<F>, P256PackedAffineFailureV3> {
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
        ModularRelationKind::AggregateSlope,
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
        ModularRelationKind::X,
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
        ModularRelationKind::Y,
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
) -> Result<[AffineVar<F>; 16], P256PackedAffineFailureV3> {
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
                bits: pair[0].bits.max(pair[1].bits),
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
                    bits: LIMB_WIDTHS[limb],
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
                    bits: LIMB_WIDTHS[limb],
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
) -> Result<AffineVar<F>, P256PackedAffineFailureV3> {
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
) -> Result<BoolVar<F>, P256PackedAffineFailureV3> {
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
        ModularRelationKind::BaseProduct,
        "on-curve x-squared",
    )?;
    let mut equation = RadixExpression::new();
    equation.add_product(&point.y, &point.y, None, 1);
    equation.add_product(&x_squared, &point.x, None, -1);
    equation.add_linear(&point.x, None, 3);
    equation.add_constant(&curve_b(), -1);
    constrain_modular_expression(
        builder,
        &equation,
        &modulus,
        &active,
        ModularRelationKind::Curve,
        "on-curve quotient",
    )?;
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
) -> Result<(), P256PackedAffineFailureV3> {
    let mut expression = RadixExpression::new();
    expression.add_product(left, right, Some(active), 1);
    expression.add_linear(output, Some(active), -1);
    constrain_modular_expression(
        builder,
        &expression,
        &modulus_scalar(),
        active,
        ModularRelationKind::ScalarProduct,
        witness,
    )
}

fn bind_public_uint<F: BigPrimeField>(
    builder: &mut PackedBuilder<F>,
    bytes_be: &[BoundedCell<F>; 32],
    raw: &[u8; 32],
    witness: &'static str,
) -> Result<UintVar<F>, P256PackedAffineFailureV3> {
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
) -> Result<(), P256PackedAffineFailureV3> {
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
        ModularRelationKind::ScalarProduct,
        "u1 scalar product",
    )?;
    let u2 = modular_product(
        builder,
        &r,
        &s_inverse,
        u2_value,
        &scalar_modulus,
        &active,
        ModularRelationKind::ScalarProduct,
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
    rows: P256PackedAffineRowsV3,
}

impl<F: BigPrimeField> PackedTrace<F> {
    #[cfg(test)]
    fn canonical_topology_descriptor(
        &self,
    ) -> Result<CanonicalTraceTopologyV3, P256PackedAffineFailureV3> {
        if self.rows_data.len() != self.rows.total_rows {
            return Err(P256PackedAffineFailureV3::Source(
                "canonical topology descriptor omitted padded trace rows",
            ));
        }

        let mut canonical_by_variable = HashMap::<usize, usize>::new();
        let mut equality_classes = 0_usize;
        let mut rows = Vec::with_capacity(self.rows_data.len());
        for row in &self.rows_data {
            let mut variables_by_column = [None; ADVICE_COLUMNS];
            for (variable, column) in &row.aliases {
                let Some(slot) = variables_by_column.get_mut(*column) else {
                    return Err(P256PackedAffineFailureV3::Source(
                        "canonical topology descriptor saw an invalid advice column",
                    ));
                };
                if slot.replace(*variable).is_some() {
                    return Err(P256PackedAffineFailureV3::Source(
                        "canonical topology descriptor saw two aliases for one advice cell",
                    ));
                }
            }
            let equality_alias_classes = variables_by_column.map(|variable| {
                variable.map(|variable| {
                    if let Some(class) = canonical_by_variable.get(&variable) {
                        *class
                    } else {
                        let class = equality_classes;
                        equality_classes += 1;
                        canonical_by_variable.insert(variable, class);
                        class
                    }
                })
            });
            rows.push(CanonicalTopologyRowV3 {
                opcode: row.opcode,
                range_bits: row.range_bits,
                equality_alias_classes,
            });
        }
        Ok(CanonicalTraceTopologyV3 {
            rows,
            equality_classes,
        })
    }

    fn assign(
        &self,
        config: &P256PackedAffineConfigV3,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), Error> {
        if self.rows.total_rows > K17_MAX_ASSIGNED_ROWS {
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

                let mut assigned_table_rows = 0_usize;
                for (offset, (first, second)) in typed_range_table_rows_v3().enumerate() {
                    raw_assign_fixed(&mut region, config.table_first, offset, F::from(first));
                    raw_assign_fixed(&mut region, config.table_second, offset, F::from(second));
                    assigned_table_rows = offset + 1;
                }
                debug_assert_eq!(assigned_table_rows, TABLE_ROWS);
                Ok(())
            },
        )
    }
}

fn transpose_packed_trace<F: BigPrimeField>(
    mut builder: PackedBuilder<F>,
) -> Result<PackedTrace<F>, P256PackedAffineFailureV3> {
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
        family_offset_constants,
        relation_counts,
        ..
    } = builder;
    let family_offset_count = family_offset_constants
        .iter()
        .filter(|constant| constant.is_some())
        .count();
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
            return Err(P256PackedAffineFailureV3::Source(
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
                    return Err(P256PackedAffineFailureV3::Source(
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

    if relation_counts != ModularRelationKind::EXPECTED_COUNTS {
        return Err(P256PackedAffineFailureV3::Source(
            "modular relation family counts disagree with the reviewed schedule",
        ));
    }
    if family_offset_count != 33 {
        return Err(P256PackedAffineFailureV3::Source(
            "verifier-bound family offset count is not exactly 30 plus 3",
        ));
    }
    if constant_instances.len() != 228 {
        return Err(P256PackedAffineFailureV3::Source(
            "verifier-derived constant tail is not exactly 228 field elements",
        ));
    }

    let assigned_semantic_rows = rows_data.len();
    let padding_rows = P256_PACKED_AFFINE_V3_SEMANTIC_ROWS.saturating_sub(assigned_semantic_rows);
    let semantic_rows = assigned_semantic_rows + padding_rows;
    let total_rows = semantic_rows.max(TABLE_ROWS);
    if assigned_semantic_rows > P256_PACKED_AFFINE_V3_SEMANTIC_ROWS
        || P256_PACKED_AFFINE_V3_UPPER_ROWS > K17_MAX_ASSIGNED_ROWS
    {
        let rows = P256PackedAffineRowsV3 {
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
            padding_rows: 0,
            semantic_rows: assigned_semantic_rows,
            reserved_rows: P256_PACKED_AFFINE_V3_RESERVED_ROWS,
            upper_rows: assigned_semantic_rows + P256_PACKED_AFFINE_V3_RESERVED_ROWS,
            headroom_rows: K17_MAX_ASSIGNED_ROWS
                .saturating_sub(assigned_semantic_rows + P256_PACKED_AFFINE_V3_RESERVED_ROWS),
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
            relation_counts,
        };
        return Err(P256PackedAffineFailureV3::RowCapacityExceeded {
            rows: Box::new(rows),
            maximum: P256_PACKED_AFFINE_V3_SEMANTIC_ROWS,
        });
    }
    rows_data.resize_with(total_rows, || AssignedRow::zero(Opcode::Disabled));
    let rows = P256PackedAffineRowsV3 {
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
        padding_rows,
        semantic_rows,
        reserved_rows: P256_PACKED_AFFINE_V3_RESERVED_ROWS,
        upper_rows: P256_PACKED_AFFINE_V3_UPPER_ROWS,
        headroom_rows: P256_PACKED_AFFINE_V3_HEADROOM_ROWS,
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
        relation_counts,
    };
    if instances.len() != binding_rows {
        return Err(P256PackedAffineFailureV3::InstanceBindingMismatch {
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
#[path = "p256_packed_affine_v3_tests.rs"]
mod tests;
