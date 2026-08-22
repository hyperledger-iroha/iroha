//! Private, non-authorizing P-256 ECDSA circuit prototype.
//!
//! This module deliberately has no production registration or GuardBundle
//! integration.  It accepts one exact public byte string
//! `[SEC1-uncompressed key; SHA-256 prehash; P1363 signature]`, constructs the
//! ordinary `halo2-ecc` virtual arithmetic trace, and transposes that trace
//! into eight equality-enabled, current-rotation advice columns.  SHA-256 and
//! ASN.1/DER parsing remain outside this prototype.
//!
//! The exact RFC 6979 trace occupies 116,304 rows after compaction, exceeding
//! the k=16 maximum of 65,527. The configured constraint-system transcript is
//! 3,200 augmented bytes, but that shape is not row-feasible. It cannot authorize a helper proof;
//! a bespoke P-256 child is required.

use std::{
    cmp::min,
    collections::{BTreeMap, HashMap, HashSet},
    marker::PhantomData,
};

use der_parser::num_bigint::BigUint;
use halo2_base::{
    gates::{GateInstructions, RangeChip, RangeInstructions},
    halo2_proofs::{
        circuit::{Cell, Layouter, SimpleFloorPlanner, Value},
        halo2curves::{
            group::{cofactor::CofactorCurveAffine as _, Curve as _},
            secp256r1::{Fp as P256Base, Fq as P256Scalar, Secp256r1, Secp256r1Affine},
            CurveAffine,
        },
        plonk::{Advice, Assigned, Circuit, Column, ConstraintSystem, Error, Fixed, Instance},
        poly::Rotation,
    },
    utils::halo2::{raw_assign_advice, raw_assign_fixed, raw_constrain_equal},
    utils::{biguint_to_fe, decompose_biguint, modulus, BigPrimeField, CurveAffineExt as _},
    virtual_region::{copy_constraints::SharedCopyConstraintManager, lookups::LookupAnyManager},
    AssignedValue, Context, ContextCell,
    QuantumCell::{Constant, Existing},
    FIRST_PHASE_CELL_TYPE_ID,
};
use halo2_ecc::{
    bigint::{big_is_equal, big_less_than, ProperCrtUint},
    ecc::{
        ec_add_unequal, ec_select, ec_select_from_bits, ec_sub_strict, ec_sub_unequal,
        into_strict_point, EcPoint,
    },
    fields::{fp::FpChip, FieldChip},
};
const K: u32 = 16;
const LOOKUP_BITS: usize = 15;
const LIMB_BITS: usize = 90;
const LIMBS: usize = 3;
const WINDOW_BITS: usize = 4;
const P256_SCALAR_BITS: usize = 256;
const P256_VARIABLE_OFFSET_SCALAR: u64 = 0x243f_6a88_85a3_08d3;
const P256_FIXED_OFFSET_SCALAR: u64 = 0x1319_8a2e_0370_7344;
const P256_SUM_OFFSET_SCALAR: u64 = 0xa409_3822_299f_31d0;
const ADVICE_COLUMNS: usize = 8;
const PUBLIC_BYTES: usize = 65 + 32 + 64;
const TABLE_ROWS: usize = 1 << LOOKUP_BITS;
// Conservative relative to the Axiom backend's current-only blinding need.
const K16_MAX_ASSIGNED_ROWS: usize = (1 << K) - 9;

type P256AssignedPoint<F> = EcPoint<F, ProperCrtUint<F>>;

/// Exact caller-controlled byte ordering for the prototype instance prefix.
pub(super) const P256_COMPACT_PUBLIC_BYTES_V1: usize = PUBLIC_BYTES;

/// Static transcript accounting for the k=16 current-query prototype.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct P256CompactShapeReportV1 {
    /// Circuit degree, forced to seven to retain two permutation chunks.
    pub(super) degree: usize,
    /// Equality-enabled advice columns.
    pub(super) advice_columns: usize,
    /// Current-rotation advice queries.
    pub(super) advice_queries: usize,
    /// Instance columns and current-rotation queries.
    pub(super) instance_columns: usize,
    pub(super) instance_queries: usize,
    /// Fixed columns and current-rotation queries (`q_range`, `q_bind`, table).
    pub(super) fixed_columns: usize,
    pub(super) fixed_queries: usize,
    /// Halo2 selector columns.  Fixed Boolean columns replace selectors.
    pub(super) selectors: usize,
    /// Columns in the copy permutation and resulting chunks.
    pub(super) equality_columns: usize,
    pub(super) permutation_chunks: usize,
    /// Independent single-column lookup arguments.
    pub(super) lookup_arguments: usize,
    /// Quotient pieces and multi-opening point sets.
    pub(super) quotient_pieces: usize,
    pub(super) opening_point_sets: usize,
    /// Point and scalar elements written by the unaugmented IPA proof.
    pub(super) proof_points: usize,
    pub(super) proof_scalars: usize,
    /// Raw Halo2 proof bytes and raw proof plus the protocol's 32-byte suffix.
    pub(super) raw_proof_bytes: usize,
    pub(super) augmented_proof_bytes: usize,
}

/// Exact configured-CS/proof-size shape; row feasibility is separately and
/// deliberately rejected by the trace-cap diagnostic.
pub(super) const P256_COMPACT_SHAPE_V1: P256CompactShapeReportV1 = P256CompactShapeReportV1 {
    degree: 7,
    advice_columns: 8,
    advice_queries: 8,
    instance_columns: 1,
    instance_queries: 1,
    fixed_columns: 3,
    fixed_queries: 3,
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

/// Runtime virtual/physical row accounting used to pin the k=16 no-go result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct P256CompactRowsV1 {
    /// Caller bytes plus verifier-derived constant-instance rows.
    pub(super) binding_rows: usize,
    /// Rows carrying two range lookups and zero, one, or two arithmetic gates.
    pub(super) range_rows: usize,
    /// Rows carrying arithmetic gates without range lookups.
    pub(super) arithmetic_rows: usize,
    /// Equality-only tautology rows for otherwise unmaterialized virtual cells.
    pub(super) equality_rows: usize,
    /// Total physical rows occupied by the compact trace.
    pub(super) total_rows: usize,
    /// Complete virtual arithmetic-gate count before transposition.
    pub(super) virtual_gates: usize,
    /// Complete virtual range-lookup count before transposition.
    pub(super) virtual_lookups: usize,
    /// Range lookups co-located with a gate operand.
    pub(super) coalesced_lookups: usize,
}

/// Private build-stage diagnostics. Production callers still receive only
/// `Error::Synthesis`; tests can inspect this enum to distinguish capacity
/// from trace-integrity failures without weakening the circuit boundary.
#[allow(
    dead_code,
    reason = "private diagnostic payloads are consumed through focused test Debug output"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum P256CompactTraceFailureV1 {
    LookupManagerPoisoned,
    CopyManagerPoisoned,
    MissingPublicCell {
        index: usize,
    },
    NonTrivialPublicAssignedValue {
        index: usize,
        variant: &'static str,
    },
    MissingEqualityValue {
        cell: ContextCell,
    },
    RowCapacityExceeded {
        rows: P256CompactRowsV1,
        maximum: usize,
    },
    InstanceBindingMismatch {
        instances: usize,
        binding_rows: usize,
    },
}

/// Public witness container for the private circuit prototype.
///
/// `digest` is already a SHA-256 digest.  `signature` is fixed-width P1363
/// `r || s`; accepting DER here would create a second, non-canonical parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct P256CompactEcdsaCircuitV1<F> {
    sec1_uncompressed: [u8; 65],
    digest: [u8; 32],
    signature: [u8; 64],
    _field: PhantomData<F>,
}

impl<F> Default for P256CompactEcdsaCircuitV1<F> {
    fn default() -> Self {
        Self {
            sec1_uncompressed: [0; 65],
            digest: [0; 32],
            signature: [0; 64],
            _field: PhantomData,
        }
    }
}

impl<F> P256CompactEcdsaCircuitV1<F>
where
    F: BigPrimeField,
{
    /// Construct the exact SEC1/prehash/P1363 prototype witness.
    pub(super) fn new(sec1_uncompressed: [u8; 65], digest: [u8; 32], signature: [u8; 64]) -> Self {
        Self {
            sec1_uncompressed,
            digest,
            signature,
            _field: PhantomData,
        }
    }

    /// Return the complete instance vector.
    ///
    /// The first 161 elements are the exact caller ABI.  The remaining values
    /// are deterministic circuit constants exposed through the same instance
    /// polynomial so the compact topology does not add a fourth fixed query.
    /// A future production wrapper must derive, never accept, that tail.
    pub(super) fn instances(&self) -> Result<Vec<F>, Error> {
        Ok(self.build_compact_trace()?.instances)
    }

    /// Measure the exact transposed row inventory without producing a proof.
    pub(super) fn row_report(&self) -> Result<P256CompactRowsV1, Error> {
        Ok(self.build_compact_trace()?.rows)
    }

    #[cfg(test)]
    fn trace_diagnostic_for_test(&self) -> Result<P256CompactRowsV1, P256CompactTraceFailureV1> {
        Ok(self.build_compact_trace_diagnostic()?.rows)
    }

    fn input_bytes(&self) -> [u8; PUBLIC_BYTES] {
        let mut input = [0_u8; PUBLIC_BYTES];
        input[..65].copy_from_slice(&self.sec1_uncompressed);
        input[65..97].copy_from_slice(&self.digest);
        input[97..].copy_from_slice(&self.signature);
        input
    }

    fn build_compact_trace(&self) -> Result<CompactTrace<F>, Error> {
        self.build_compact_trace_diagnostic()
            .map_err(|_| Error::Synthesis)
    }

    fn build_compact_trace_diagnostic(&self) -> Result<CompactTrace<F>, P256CompactTraceFailureV1> {
        let copy_manager: SharedCopyConstraintManager<F> = Default::default();
        let lookup_managers: [_; 3] =
            std::array::from_fn(|_| LookupAnyManager::<F, 1>::new(false, copy_manager.clone()));
        let retained_lookup = lookup_managers[0].clone();
        let range = RangeChip::new(LOOKUP_BITS, lookup_managers);
        let mut ctx = Context::new(false, 0, FIRST_PHASE_CELL_TYPE_ID, 0, copy_manager.clone());

        let public_cells = assign_public_bytes(&mut ctx, &range, &self.input_bytes());
        constrain_p256_ecdsa(
            &mut ctx,
            &range,
            &public_cells,
            &self.sec1_uncompressed,
            &self.digest,
            &self.signature,
        );

        let virtual_values: HashMap<_, _> = (0..ctx.advice_len())
            .map(|offset| {
                let value = ctx.get(offset as isize);
                (
                    value.cell.expect("non-witness-only context has a cell"),
                    value.value,
                )
            })
            .collect();
        let mut gates = Vec::new();
        for offset in 0..ctx.selector.len() {
            if ctx.selector[offset] {
                let cells = std::array::from_fn(|delta| {
                    let value = ctx.get((offset + delta) as isize);
                    (
                        value.cell.expect("selected virtual cell has an identity"),
                        value.value,
                    )
                });
                gates.push(VirtualGate { cells });
            }
        }

        let lookup_cells = retained_lookup
            .cells_to_lookup
            .lock()
            .map_err(|_| P256CompactTraceFailureV1::LookupManagerPoisoned)?
            .values()
            .flat_map(|thread| thread.iter())
            .map(|[value]| {
                (
                    value.cell.expect("range lookup has a virtual cell"),
                    value.value,
                )
            })
            .collect::<Vec<_>>();

        let (equalities, constants) = {
            let manager = copy_manager
                .lock()
                .map_err(|_| P256CompactTraceFailureV1::CopyManagerPoisoned)?;
            let equalities = manager.advice_equalities.clone();
            let mut constants = BTreeMap::<F, Vec<ContextCell>>::new();
            for (constant, cell) in manager.constant_equalities.iter() {
                constants.entry(*constant).or_default().push(*cell);
            }
            (equalities, constants)
        };

        // The extracted trace is now self-contained.  Clearing avoids the
        // virtual managers' intentional "not assigned" diagnostic on drop.
        retained_lookup
            .cells_to_lookup
            .lock()
            .map_err(|_| P256CompactTraceFailureV1::LookupManagerPoisoned)?
            .clear();
        copy_manager
            .lock()
            .map_err(|_| P256CompactTraceFailureV1::CopyManagerPoisoned)?
            .clear();

        transpose_trace(
            virtual_values,
            gates,
            lookup_cells,
            equalities,
            constants,
            public_cells,
        )
    }
}

/// Eight current-query advice columns plus three current-query fixed columns.
#[derive(Clone, Debug)]
pub(super) struct P256CompactConfigV1 {
    advice: [Column<Advice>; ADVICE_COLUMNS],
    instance: Column<Instance>,
    q_range: Column<Fixed>,
    q_bind: Column<Fixed>,
    table: Column<Fixed>,
}

impl P256CompactConfigV1 {
    fn configure<F: BigPrimeField>(meta: &mut ConstraintSystem<F>) -> Self {
        let advice = std::array::from_fn(|_| {
            let column = meta.advice_column();
            meta.enable_equality(column);
            column
        });
        let instance = meta.instance_column();
        let q_range = meta.fixed_column();
        let q_bind = meta.fixed_column();
        let table = meta.fixed_column();

        meta.create_gate(
            "two compact current-row FMAs and exact instance bind",
            |meta| {
                let values = advice.map(|column| meta.query_advice(column, Rotation::cur()));
                let public = meta.query_instance(instance, Rotation::cur());
                let bind = meta.query_fixed(q_bind, Rotation::cur());
                vec![
                    values[0].clone() + values[1].clone() * values[2].clone() - values[3].clone(),
                    values[4].clone() + values[5].clone() * values[6].clone() - values[7].clone(),
                    bind * (values[3].clone() - public),
                ]
            },
        );
        meta.lookup_any("compact range lane zero", |meta| {
            let enabled = meta.query_fixed(q_range, Rotation::cur());
            let value = meta.query_advice(advice[1], Rotation::cur());
            let table = meta.query_fixed(table, Rotation::cur());
            vec![(enabled * value, table)]
        });
        meta.lookup_any("compact range lane one", |meta| {
            let enabled = meta.query_fixed(q_range, Rotation::cur());
            let value = meta.query_advice(advice[5], Rotation::cur());
            let table = meta.query_fixed(table, Rotation::cur());
            vec![(enabled * value, table)]
        });

        // Degree seven gives five equality columns per permutation chunk and
        // exactly six h pieces.  This is part of the 3,200-byte shape contract.
        meta.set_minimum_degree(P256_COMPACT_SHAPE_V1.degree);
        Self {
            advice,
            instance,
            q_range,
            q_bind,
            table,
        }
    }
}

impl<F> Circuit<F> for P256CompactEcdsaCircuitV1<F>
where
    F: BigPrimeField,
{
    type Config = P256CompactConfigV1;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        P256CompactConfigV1::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let trace = self.build_compact_trace()?;
        trace.assign(&config, &mut layouter)
    }
}

#[derive(Clone, Copy, Debug)]
struct VirtualGate<F> {
    cells: [(ContextCell, Assigned<F>); 4],
}

#[derive(Clone, Debug)]
struct CompactRow<F> {
    values: [Assigned<F>; ADVICE_COLUMNS],
    aliases: Vec<(ContextCell, usize)>,
    range: bool,
    bind: bool,
    arithmetic: bool,
}

impl<F: BigPrimeField> CompactRow<F> {
    fn empty() -> Self {
        Self {
            values: [Assigned::Trivial(F::ZERO); ADVICE_COLUMNS],
            aliases: Vec::new(),
            range: false,
            bind: false,
            arithmetic: false,
        }
    }

    fn bind(value: Assigned<F>, aliases: impl IntoIterator<Item = ContextCell>) -> Self {
        let mut row = Self::empty();
        row.values[0] = value;
        row.values[3] = value;
        row.aliases
            .extend(aliases.into_iter().map(|cell| (cell, 3)));
        row.bind = true;
        row
    }

    fn set_gate(&mut self, lane: usize, gate: VirtualGate<F>, swap_factors: bool) {
        let base = lane * 4;
        let order = if swap_factors {
            [0, 2, 1, 3]
        } else {
            [0, 1, 2, 3]
        };
        for (physical, virtual_index) in order.into_iter().enumerate() {
            let (cell, value) = gate.cells[virtual_index];
            self.values[base + physical] = value;
            self.aliases.push((cell, base + physical));
        }
        self.arithmetic = true;
    }

    fn set_lookup_only(&mut self, lane: usize, lookup: (ContextCell, Assigned<F>)) {
        let base = lane * 4;
        self.values[base + 1] = lookup.1;
        self.aliases.push((lookup.0, base + 1));
    }

    fn set_equality_only(&mut self, lane: usize, cell: ContextCell, value: Assigned<F>) {
        let base = lane * 4;
        self.values[base] = value;
        self.values[base + 3] = value;
        self.aliases.push((cell, base + 3));
    }
}

#[derive(Clone, Debug)]
struct CompactTrace<F> {
    rows_data: Vec<CompactRow<F>>,
    equalities: Vec<(ContextCell, ContextCell)>,
    instances: Vec<F>,
    rows: P256CompactRowsV1,
}

impl<F: BigPrimeField> CompactTrace<F> {
    fn assign(
        &self,
        config: &P256CompactConfigV1,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), Error> {
        if self.rows_data.len() > K16_MAX_ASSIGNED_ROWS {
            return Err(Error::Synthesis);
        }
        layouter.assign_region(
            || "compact P-256 current-query trace and range table",
            |mut region| {
                let mut physical = HashMap::<ContextCell, Cell>::new();
                for (row_offset, row) in self.rows_data.iter().enumerate() {
                    let cells: [Cell; ADVICE_COLUMNS] = std::array::from_fn(|column| {
                        raw_assign_advice(
                            &mut region,
                            config.advice[column],
                            row_offset,
                            Value::known(row.values[column]),
                        )
                        .cell()
                    });
                    raw_assign_fixed(
                        &mut region,
                        config.q_range,
                        row_offset,
                        F::from(row.range as u64),
                    );
                    raw_assign_fixed(
                        &mut region,
                        config.q_bind,
                        row_offset,
                        F::from(row.bind as u64),
                    );
                    for (virtual_cell, column) in &row.aliases {
                        match physical.insert(*virtual_cell, cells[*column]) {
                            Some(first) => raw_constrain_equal(&mut region, first, cells[*column]),
                            None => {}
                        }
                    }
                }
                for value in 0..TABLE_ROWS {
                    raw_assign_fixed(&mut region, config.table, value, F::from(value as u64));
                }
                for (left, right) in &self.equalities {
                    let left = *physical.get(left).ok_or(Error::Synthesis)?;
                    let right = *physical.get(right).ok_or(Error::Synthesis)?;
                    raw_constrain_equal(&mut region, left, right);
                }
                Ok(())
            },
        )
    }
}

fn transpose_trace<F: BigPrimeField>(
    virtual_values: HashMap<ContextCell, Assigned<F>>,
    gates: Vec<VirtualGate<F>>,
    lookups: Vec<(ContextCell, Assigned<F>)>,
    equalities: Vec<(ContextCell, ContextCell)>,
    constants: BTreeMap<F, Vec<ContextCell>>,
    public_cells: Vec<AssignedValue<F>>,
) -> Result<CompactTrace<F>, P256CompactTraceFailureV1> {
    let virtual_gate_count = gates.len();
    let virtual_lookup_count = lookups.len();
    let mut rows_data = Vec::new();
    let mut instances = Vec::new();
    let mut covered = HashSet::new();

    for (index, public) in public_cells.into_iter().enumerate() {
        let cell = public
            .cell
            .ok_or(P256CompactTraceFailureV1::MissingPublicCell { index })?;
        instances.push(assigned_trivial(public.value, index)?);
        covered.insert(cell);
        rows_data.push(CompactRow::bind(public.value, [cell]));
    }
    for (constant, cells) in constants {
        instances.push(constant);
        covered.extend(cells.iter().copied());
        rows_data.push(CompactRow::bind(
            Assigned::Trivial(constant),
            cells.into_iter(),
        ));
    }
    let binding_rows = rows_data.len();

    // A range decomposition's fresh limb is normally the multiplicand in a
    // vertical FMA.  Transposition may swap the two factors, so every such
    // lookup can occupy physical column a1/a5 without an extra row.
    let mut candidates = HashMap::<ContextCell, Vec<(usize, bool)>>::new();
    for (gate_index, gate) in gates.iter().enumerate() {
        candidates
            .entry(gate.cells[1].0)
            .or_default()
            .push((gate_index, false));
        candidates
            .entry(gate.cells[2].0)
            .or_default()
            .push((gate_index, true));
    }
    let mut gate_match = vec![None; gates.len()];
    let mut range_lanes = Vec::<RangeLane<F>>::new();
    for lookup in lookups {
        let matched = candidates.get_mut(&lookup.0).and_then(|candidates| {
            while let Some((gate_index, swap)) = candidates.pop() {
                if gate_match[gate_index].is_none() {
                    gate_match[gate_index] = Some(swap);
                    return Some((gate_index, swap));
                }
            }
            None
        });
        match matched {
            Some((gate_index, swap)) => range_lanes.push(RangeLane::Gate(gate_index, swap)),
            None => range_lanes.push(RangeLane::Lookup(lookup)),
        }
    }
    let coalesced_lookups = gate_match.iter().filter(|entry| entry.is_some()).count();

    // A binding row is already a tautological FMA in lane zero.  Use its
    // otherwise-empty second lane before allocating more rows.  When range is
    // enabled the unused first lookup is zero, which is a canonical table
    // value.  This keeps every instance row exact while recovering the space
    // that the public/constant tail would otherwise waste.
    let mut binding_lane_fill = 0_usize;
    let mut range_lanes = range_lanes.into_iter();
    while binding_lane_fill < binding_rows {
        let Some(item) = range_lanes.next() else {
            break;
        };
        let row = &mut rows_data[binding_lane_fill];
        row.range = true;
        match item {
            RangeLane::Gate(index, swap) => row.set_gate(1, gates[index], swap),
            RangeLane::Lookup(lookup) => row.set_lookup_only(1, lookup),
        }
        covered.extend(row.aliases.iter().map(|(cell, _)| *cell));
        binding_lane_fill += 1;
    }
    let remaining_range_lanes = range_lanes.collect::<Vec<_>>();
    for lanes in remaining_range_lanes.chunks(2) {
        let mut row = CompactRow::empty();
        row.range = true;
        for (lane, item) in lanes.iter().enumerate() {
            match item {
                RangeLane::Gate(index, swap) => row.set_gate(lane, gates[*index], *swap),
                RangeLane::Lookup(lookup) => row.set_lookup_only(lane, *lookup),
            }
        }
        covered.extend(row.aliases.iter().map(|(cell, _)| *cell));
        rows_data.push(row);
    }

    let mut unmatched_gates = gates
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| gate_match[*index].is_none())
        .map(|(_, gate)| gate);
    while binding_lane_fill < binding_rows {
        let Some(gate) = unmatched_gates.next() else {
            break;
        };
        let row = &mut rows_data[binding_lane_fill];
        row.set_gate(1, gate, false);
        covered.extend(row.aliases.iter().map(|(cell, _)| *cell));
        binding_lane_fill += 1;
    }
    let remaining_unmatched_gates = unmatched_gates.collect::<Vec<_>>();
    for pair in remaining_unmatched_gates.chunks(2) {
        let mut row = CompactRow::empty();
        for (lane, gate) in pair.iter().enumerate() {
            row.set_gate(lane, *gate, false);
        }
        covered.extend(row.aliases.iter().map(|(cell, _)| *cell));
        rows_data.push(row);
    }

    let mut required = HashSet::new();
    for (left, right) in &equalities {
        required.insert(*left);
        required.insert(*right);
    }
    let missing = required
        .difference(&covered)
        .copied()
        .collect::<Vec<ContextCell>>();
    let equality_start = rows_data.len();
    for pair in missing.chunks(2) {
        let mut row = CompactRow::empty();
        for (lane, cell) in pair.iter().enumerate() {
            let value = *virtual_values
                .get(cell)
                .ok_or(P256CompactTraceFailureV1::MissingEqualityValue { cell: *cell })?;
            row.set_equality_only(lane, *cell, value);
        }
        rows_data.push(row);
    }
    let equality_rows = rows_data.len() - equality_start;
    let range_rows = rows_data.iter().filter(|row| row.range).count();
    let arithmetic_rows = rows_data
        .iter()
        .filter(|row| row.arithmetic && !row.range)
        .count();

    let rows = P256CompactRowsV1 {
        binding_rows,
        range_rows,
        arithmetic_rows,
        equality_rows,
        total_rows: rows_data.len(),
        virtual_gates: virtual_gate_count,
        virtual_lookups: virtual_lookup_count,
        coalesced_lookups,
    };
    if rows_data.len() > K16_MAX_ASSIGNED_ROWS {
        return Err(P256CompactTraceFailureV1::RowCapacityExceeded {
            rows,
            maximum: K16_MAX_ASSIGNED_ROWS,
        });
    }
    if instances.len() != binding_rows {
        return Err(P256CompactTraceFailureV1::InstanceBindingMismatch {
            instances: instances.len(),
            binding_rows,
        });
    }
    Ok(CompactTrace {
        rows_data,
        equalities,
        instances,
        rows,
    })
}

#[derive(Clone, Copy, Debug)]
enum RangeLane<F> {
    Gate(usize, bool),
    Lookup((ContextCell, Assigned<F>)),
}

fn assigned_trivial<F: BigPrimeField>(
    value: Assigned<F>,
    index: usize,
) -> Result<F, P256CompactTraceFailureV1> {
    match value {
        Assigned::Trivial(value) => Ok(value),
        Assigned::Zero => Err(P256CompactTraceFailureV1::NonTrivialPublicAssignedValue {
            index,
            variant: "Zero",
        }),
        Assigned::Rational(_, _) => Err(P256CompactTraceFailureV1::NonTrivialPublicAssignedValue {
            index,
            variant: "Rational",
        }),
    }
}

fn assign_public_bytes<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8; PUBLIC_BYTES],
) -> Vec<AssignedValue<F>> {
    bytes
        .iter()
        .map(|byte| {
            let value = ctx.load_witness(F::from(u64::from(*byte)));
            range.range_check(ctx, value, 8);
            value
        })
        .collect()
}

fn constrain_p256_ecdsa<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    public: &[AssignedValue<F>],
    sec1: &[u8; 65],
    digest: &[u8; 32],
    signature: &[u8; 64],
) {
    assert_eq!(public.len(), PUBLIC_BYTES);
    let base_chip = FpChip::<F, P256Base>::new(range, LIMB_BITS, LIMBS);
    let scalar_chip = FpChip::<F, P256Scalar>::new(range, LIMB_BITS, LIMBS);
    let gate = range.gate();

    gate.assert_is_const(ctx, &public[0], &F::from(4));

    let x_bytes: &[AssignedValue<F>; 32] = public[1..33].try_into().expect("fixed SEC1 x");
    let y_bytes: &[AssignedValue<F>; 32] = public[33..65].try_into().expect("fixed SEC1 y");
    let digest_bytes: &[AssignedValue<F>; 32] = public[65..97].try_into().expect("fixed digest");
    let r_bytes: &[AssignedValue<F>; 32] = public[97..129].try_into().expect("fixed r");
    let s_bytes: &[AssignedValue<F>; 32] = public[129..161].try_into().expect("fixed s");

    let x = load_exact_canonical(
        ctx,
        range,
        &base_chip,
        x_bytes,
        &sec1[1..33].try_into().expect("fixed x bytes"),
    );
    let y = load_exact_canonical(
        ctx,
        range,
        &base_chip,
        y_bytes,
        &sec1[33..65].try_into().expect("fixed y bytes"),
    );
    let public_key = EcPoint::new(x, y);
    p256_assert_on_curve_and_nonidentity(&base_chip, ctx, &public_key);

    let r_raw: [u8; 32] = signature[..32].try_into().expect("fixed r bytes");
    let s_raw: [u8; 32] = signature[32..].try_into().expect("fixed s bytes");
    let r = load_exact_canonical(ctx, range, &scalar_chip, r_bytes, &r_raw);
    let s = load_exact_canonical(ctx, range, &scalar_chip, s_bytes, &s_raw);
    let r_valid = scalar_chip.is_soft_nonzero(ctx, r.clone());
    let s_valid = scalar_chip.is_soft_nonzero(ctx, s.clone());

    let n = modulus::<P256Scalar>();
    let low_s_bound = scalar_chip.load_constant_uint(ctx, (&n >> 1usize) + 1_u8);
    let low_s = big_less_than::assign(
        range,
        ctx,
        s.clone(),
        low_s_bound,
        LIMB_BITS,
        scalar_chip.limb_bases[1],
    );

    let digest_integer = BigUint::from_bytes_be(digest);
    let digest_limbs = bind_90_bit_limbs(ctx, range, digest_bytes);
    let z_value = &digest_integer % &n;
    let z = scalar_chip.load_private(ctx, biguint_to_fe::<P256Scalar>(&z_value));
    scalar_chip.enforce_less_than_p(ctx, z.clone());
    constrain_single_subtraction(ctx, range, &digest_limbs, &digest_integer, &z, &n);

    let u1 = scalar_chip.divide_unsafe(ctx, z, &s);
    let u2 = scalar_chip.divide_unsafe(ctx, &r, s);
    let n_constant = scalar_chip.load_constant_uint(ctx, n.clone());
    let u1_small = big_less_than::assign(
        range,
        ctx,
        u1.clone(),
        n_constant.clone(),
        LIMB_BITS,
        scalar_chip.limb_bases[1],
    );
    let u2_small = big_less_than::assign(
        range,
        ctx,
        u2.clone(),
        n_constant,
        LIMB_BITS,
        scalar_chip.limb_bases[1],
    );

    let u1_generator = p256_fixed_base_scalar_multiply(
        &base_chip,
        ctx,
        &Secp256r1Affine::generator(),
        u1.limbs().to_vec(),
    );
    let u2_public_key =
        p256_variable_scalar_multiply(&base_chip, ctx, public_key, u2.limbs().to_vec());

    let x_equal = base_chip.is_equal(ctx, &u1_generator.x, &u2_public_key.x);
    let x_unequal = gate.not(ctx, x_equal);
    let y_equal = base_chip.is_equal(ctx, &u1_generator.y, &u2_public_key.y);
    let not_opposites = gate.or(ctx, x_unequal, y_equal);

    let sum = p256_sum(&base_chip, ctx, [u1_generator, u2_public_key].into_iter());
    let sum_x = base_chip.enforce_less_than(ctx, sum.x);
    let sum_x_integer = sum_x.inner().value();
    let x_mod_n_value = &sum_x_integer % &n;
    let x_mod_n = scalar_chip.load_private(ctx, biguint_to_fe::<P256Scalar>(&x_mod_n_value));
    scalar_chip.enforce_less_than_p(ctx, x_mod_n.clone());
    constrain_single_subtraction(
        ctx,
        range,
        sum_x.inner().limbs(),
        &sum_x_integer,
        &x_mod_n,
        &n,
    );
    let r_matches = big_is_equal::assign(gate, ctx, x_mod_n, r);

    let result = [
        r_valid,
        s_valid,
        low_s,
        u1_small,
        u2_small,
        not_opposites,
        r_matches,
    ]
    .into_iter()
    .reduce(|left, right| gate.and(ctx, left, right))
    .expect("non-empty ECDSA result conjunction");
    gate.assert_is_const(ctx, &result, &F::ONE);
}

fn load_exact_canonical<F, T>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    chip: &FpChip<'_, F, T>,
    assigned_bytes: &[AssignedValue<F>; 32],
    raw_bytes: &[u8; 32],
) -> ProperCrtUint<F>
where
    F: BigPrimeField,
    T: BigPrimeField,
{
    let integer = BigUint::from_bytes_be(raw_bytes);
    let represented = &integer % modulus::<T>();
    let loaded = chip.load_private(ctx, biguint_to_fe::<T>(&represented));
    let raw_limbs = bind_90_bit_limbs(ctx, range, assigned_bytes);
    for (raw, loaded) in raw_limbs.into_iter().zip(loaded.limbs()) {
        ctx.constrain_equal(&raw, loaded);
    }
    chip.enforce_less_than_p(ctx, loaded.clone());
    loaded
}

fn bind_90_bit_limbs<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    big_endian: &[AssignedValue<F>; 32],
) -> [AssignedValue<F>; LIMBS] {
    let little_endian = big_endian.iter().rev().copied().collect::<Vec<_>>();
    let (byte_11_low_2, byte_11_high_6) = split_byte(ctx, range, little_endian[11], 2);
    let (byte_22_low_4, byte_22_high_4) = split_byte(ctx, range, little_endian[22], 4);
    let gate = range.gate();

    let mut limb_0 = little_endian[..11].to_vec();
    limb_0.push(byte_11_low_2);
    let limb_0_coefficients = (0..11)
        .map(|index| F::from(2).pow_vartime([(8 * index) as u64]))
        .chain([F::from(2).pow_vartime([88])]);
    let limb_0 = gate.inner_product(
        ctx,
        limb_0.into_iter().map(Existing),
        limb_0_coefficients.map(Constant),
    );

    let mut limb_1 = vec![byte_11_high_6];
    limb_1.extend_from_slice(&little_endian[12..22]);
    limb_1.push(byte_22_low_4);
    let limb_1_coefficients = std::iter::once(F::ONE)
        .chain((0..10).map(|index| F::from(2).pow_vartime([(6 + 8 * index) as u64])))
        .chain([F::from(2).pow_vartime([86])]);
    let limb_1 = gate.inner_product(
        ctx,
        limb_1.into_iter().map(Existing),
        limb_1_coefficients.map(Constant),
    );

    let mut limb_2 = vec![byte_22_high_4];
    limb_2.extend_from_slice(&little_endian[23..]);
    let limb_2_coefficients = std::iter::once(F::ONE)
        .chain((0..9).map(|index| F::from(2).pow_vartime([(4 + 8 * index) as u64])));
    let limb_2 = gate.inner_product(
        ctx,
        limb_2.into_iter().map(Existing),
        limb_2_coefficients.map(Constant),
    );
    [limb_0, limb_1, limb_2]
}

fn split_byte<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    byte: AssignedValue<F>,
    low_bits: usize,
) -> (AssignedValue<F>, AssignedValue<F>) {
    let encoded = byte.value().to_repr();
    let value = encoded.as_ref()[0];
    let low_mask = (1_u16 << low_bits) - 1;
    let low = ctx.load_witness(F::from(u64::from(u16::from(value) & low_mask)));
    let high = ctx.load_witness(F::from(u64::from(value >> low_bits)));
    range.range_check(ctx, low, low_bits);
    range.range_check(ctx, high, 8 - low_bits);
    let recomposed = range.gate().inner_product(
        ctx,
        [Existing(low), Existing(high)],
        [Constant(F::ONE), Constant(F::from(1_u64 << low_bits))],
    );
    ctx.constrain_equal(&byte, &recomposed);
    (low, high)
}

fn constrain_single_subtraction<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    raw_limbs: &[AssignedValue<F>],
    raw_value: &BigUint,
    reduced: &ProperCrtUint<F>,
    modulus_value: &BigUint,
) {
    assert_eq!(raw_limbs.len(), LIMBS);
    assert_eq!(reduced.limbs().len(), LIMBS);
    let gate = range.gate();
    let subtract = raw_value >= modulus_value;
    let subtract_cell = ctx.load_witness(F::from(subtract as u64));
    gate.assert_bit(ctx, subtract_cell);
    let modulus_limbs = decompose_biguint::<F>(modulus_value, LIMBS, LIMB_BITS);
    let radix = F::from(2).pow_vartime([LIMB_BITS as u64]);

    let raw_integer_limbs = decompose_biguint::<F>(raw_value, LIMBS, LIMB_BITS);
    let reduced_integer_limbs = decompose_biguint::<F>(&reduced.value(), LIMBS, LIMB_BITS);
    let radix_integer = BigUint::from(1_u8) << LIMB_BITS;
    let mask = &radix_integer - 1_u8;
    let modulus_integer_limbs = (0..LIMBS)
        .map(|index| (modulus_value >> (index * LIMB_BITS)) & &mask)
        .collect::<Vec<_>>();
    let reduced_big_limbs = (0..LIMBS)
        .map(|index| (reduced.value() >> (index * LIMB_BITS)) & &mask)
        .collect::<Vec<_>>();

    let mut carry_integer = BigUint::from(0_u8);
    let mut carry_cell: Option<AssignedValue<F>> = None;
    for index in 0..LIMBS {
        debug_assert_eq!(
            raw_integer_limbs[index],
            biguint_to_fe::<F>(&((raw_value >> (index * LIMB_BITS)) & &mask))
        );
        debug_assert_eq!(
            reduced_integer_limbs[index],
            biguint_to_fe::<F>(&reduced_big_limbs[index])
        );
        let sum = &reduced_big_limbs[index]
            + if subtract {
                modulus_integer_limbs[index].clone()
            } else {
                BigUint::from(0_u8)
            }
            + &carry_integer;
        let next_carry_integer = &sum >> LIMB_BITS;
        debug_assert!(next_carry_integer <= BigUint::from(1_u8));
        let next_carry = ctx.load_witness(F::from(next_carry_integer == BigUint::from(1_u8)));
        gate.assert_bit(ctx, next_carry);

        let lhs = gate.inner_product(
            ctx,
            [
                Existing(reduced.limbs()[index]),
                Existing(subtract_cell),
                carry_cell.map_or(Constant(F::ZERO), Existing),
            ],
            [
                Constant(F::ONE),
                Constant(modulus_limbs[index]),
                Constant(F::ONE),
            ],
        );
        let rhs = gate.inner_product(
            ctx,
            [Existing(raw_limbs[index]), Existing(next_carry)],
            [Constant(F::ONE), Constant(radix)],
        );
        ctx.constrain_equal(&lhs, &rhs);
        carry_integer = next_carry_integer;
        carry_cell = Some(next_carry);
    }
    gate.assert_is_const(
        ctx,
        &carry_cell.expect("three-limb reduction has a terminal carry"),
        &F::ZERO,
    );
}

fn p256_assert_on_curve_and_nonidentity<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: &P256AssignedPoint<F>,
) {
    debug_assert_eq!(
        Secp256r1Affine::a(),
        -P256Base::from(3),
        "P-256 coefficient must remain a=-3"
    );
    let y_squared = chip.mul_no_carry(ctx, &point.y, &point.y);
    let x_squared = chip.mul(ctx, &point.x, &point.x);
    let x_cubed = chip.mul_no_carry(ctx, x_squared, &point.x);
    let three_x = chip.scalar_mul_no_carry(ctx, &point.x, 3);
    let x_cubed_minus_three_x = chip.sub_no_carry(ctx, x_cubed, three_x);
    let rhs = chip.add_constant_no_carry(ctx, x_cubed_minus_three_x, Secp256r1Affine::b());
    let difference = chip.sub_no_carry(ctx, y_squared, rhs);
    chip.check_carry_mod_to_zero(ctx, difference);

    let x_zero = chip.is_zero(ctx, &point.x);
    let y_zero = chip.is_zero(ctx, &point.y);
    let identity = chip.gate().and(ctx, x_zero, y_zero);
    chip.gate().assert_is_const(ctx, &identity, &F::ZERO);
}

/// Correct P-256 doubling: lambda = (3*x^2 - 3) / (2*y).
fn p256_double<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: impl Into<P256AssignedPoint<F>>,
) -> P256AssignedPoint<F> {
    let point = point.into();
    let two_y = chip.scalar_mul_no_carry(ctx, &point.y, 2);
    let x_squared = chip.mul_no_carry(ctx, &point.x, &point.x);
    let three_x_squared = chip.scalar_mul_no_carry(ctx, x_squared, 3);
    let three = chip.load_constant(ctx, P256Base::from(3));
    let numerator = chip.sub_no_carry(ctx, three_x_squared, three);
    let numerator = chip.carry_mod(ctx, numerator);
    let denominator = chip.carry_mod(ctx, two_y);
    let lambda = chip.divide(ctx, numerator, denominator);

    let lambda_squared = chip.mul_no_carry(ctx, &lambda, &lambda);
    let two_x = chip.scalar_mul_no_carry(ctx, &point.x, 2);
    let x_3_no_carry = chip.sub_no_carry(ctx, lambda_squared, two_x);
    let x_3 = chip.carry_mod(ctx, x_3_no_carry);
    let x_delta = chip.sub_no_carry(ctx, point.x, &x_3);
    let lambda_delta = chip.mul_no_carry(ctx, lambda, x_delta);
    let y_3_no_carry = chip.sub_no_carry(ctx, lambda_delta, point.y);
    let y_3 = chip.carry_mod(ctx, y_3_no_carry);
    EcPoint::new(x_3, y_3)
}

fn p256_load_offset_point<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    offset_scalar: u64,
) -> P256AssignedPoint<F> {
    assert_ne!(offset_scalar, 0);
    let point = (Secp256r1Affine::generator() * P256Scalar::from(offset_scalar)).to_affine();
    let (x, y) = point.into_coordinates();
    let point = EcPoint::new(chip.load_private(ctx, x), chip.load_private(ctx, y));
    p256_assert_on_curve_and_nonidentity(chip, ctx, &point);
    point
}

fn p256_scalar_bits<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    scalar: Vec<AssignedValue<F>>,
) -> Vec<AssignedValue<F>> {
    assert!(LIMB_BITS * scalar.len() >= P256_SCALAR_BITS);
    let mut remaining = P256_SCALAR_BITS;
    let mut bits = Vec::with_capacity(P256_SCALAR_BITS);
    for limb in scalar {
        if remaining == 0 {
            break;
        }
        let limb_bits = min(LIMB_BITS, remaining);
        bits.extend(chip.gate().num_to_bits(ctx, limb, limb_bits));
        remaining -= limb_bits;
    }
    assert_eq!(remaining, 0);
    bits
}

fn p256_variable_scalar_multiply<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: P256AssignedPoint<F>,
    scalar: Vec<AssignedValue<F>>,
) -> P256AssignedPoint<F> {
    assert!(!scalar.is_empty());
    let total_bits = P256_SCALAR_BITS;
    let window_count = total_bits.div_ceil(WINDOW_BITS);
    let rounded_bits = window_count * WINDOW_BITS;
    let zero = ctx.load_zero();
    let bits = p256_scalar_bits(chip, ctx, scalar)
        .into_iter()
        .chain(std::iter::repeat(zero).take(rounded_bits - total_bits))
        .collect::<Vec<_>>();

    let offset = p256_load_offset_point(chip, ctx, P256_VARIABLE_OFFSET_SCALAR);
    let mut offsets = Vec::with_capacity(WINDOW_BITS + 1);
    offsets.push(offset);
    for index in 1..=WINDOW_BITS {
        offsets.push(p256_double(chip, ctx, &offsets[index - 1]));
    }

    let cache_size = 1 << WINDOW_BITS;
    let infinity = chip.is_zero(ctx, &point.y);
    let negative_offset = ec_sub_unequal(chip, ctx, &offsets[0], &offsets[WINDOW_BITS], true);
    let point = into_strict_point(chip, ctx, point);
    let mut cached = Vec::with_capacity(cache_size);
    cached.push(into_strict_point(chip, ctx, negative_offset));
    for _ in 1..cache_size {
        let previous = cached.last().expect("non-empty scalar cache").clone();
        let sum = ec_add_unequal(chip, ctx, &previous, &point, true);
        let selected = ec_select(chip, ctx, previous.clone().into(), sum, infinity);
        cached.push(into_strict_point(chip, ctx, selected));
    }

    let start = offsets[0].clone();
    let mut accumulator = start.clone();
    for window in 0..window_count {
        for _ in 0..WINDOW_BITS {
            accumulator = p256_double(chip, ctx, accumulator);
        }
        let selected = ec_select_from_bits(
            chip,
            ctx,
            &cached,
            &bits[rounded_bits - WINDOW_BITS * (window + 1)..rounded_bits - WINDOW_BITS * window],
        );
        accumulator = ec_add_unequal(chip, ctx, accumulator, selected, true);
    }
    ec_sub_strict(chip, ctx, accumulator, start)
}

fn p256_fixed_base_scalar_multiply<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: &Secp256r1Affine,
    scalar: Vec<AssignedValue<F>>,
) -> P256AssignedPoint<F> {
    assert!(!bool::from(point.is_identity()));
    assert!(!scalar.is_empty());
    let total_bits = P256_SCALAR_BITS;
    let window_count = total_bits.div_ceil(WINDOW_BITS);
    let mut increment = point.to_curve();
    let cached_jacobian = (0..window_count)
        .flat_map(|window| {
            let mut current = increment;
            let width = 1 << min(WINDOW_BITS, total_bits - window * WINDOW_BITS);
            let values = std::iter::once(increment)
                .chain((1..width).map(|_| {
                    let previous = current;
                    current += increment;
                    previous
                }))
                .collect::<Vec<_>>();
            increment = current;
            values
        })
        .collect::<Vec<_>>();
    let mut cached_affine = vec![Secp256r1Affine::default(); cached_jacobian.len()];
    Secp256r1::batch_normalize(&cached_jacobian, &mut cached_affine);
    let cached = cached_affine
        .into_iter()
        .map(|point| {
            let (x, y) = point.into_coordinates();
            EcPoint::new(chip.load_constant(ctx, x), chip.load_constant(ctx, y))
        })
        .collect::<Vec<_>>();
    let bits = p256_scalar_bits(chip, ctx, scalar);

    let offset = p256_load_offset_point(chip, ctx, P256_FIXED_OFFSET_SCALAR);
    let mut accumulator = offset.clone();
    for (window_points, window_bits) in cached
        .chunks(1 << WINDOW_BITS)
        .rev()
        .zip(bits.chunks(WINDOW_BITS).rev())
    {
        let bit_sum = chip.gate().sum(ctx, window_bits.iter().copied());
        let zero_window = chip.gate().is_zero(ctx, bit_sum);
        let selected = ec_select_from_bits(chip, ctx, window_points, window_bits);
        let sum = ec_add_unequal(chip, ctx, &accumulator, &selected, true);
        accumulator = ec_select(chip, ctx, accumulator, sum, zero_window);
    }
    ec_sub_strict(chip, ctx, accumulator, offset)
}

fn p256_sum<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    points: impl IntoIterator<Item = P256AssignedPoint<F>>,
) -> P256AssignedPoint<F> {
    let offset = p256_load_offset_point(chip, ctx, P256_SUM_OFFSET_SCALAR);
    let offset = into_strict_point(chip, ctx, offset);
    let mut accumulator = offset.clone();
    for point in points {
        let infinity = chip.is_zero(ctx, &point.y);
        let sum = ec_add_unequal(chip, ctx, accumulator.clone(), point, true);
        let selected = ec_select(chip, ctx, accumulator.into(), sum, infinity);
        accumulator = into_strict_point(chip, ctx, selected);
    }
    ec_sub_strict(chip, ctx, accumulator, offset)
}

#[cfg(test)]
#[path = "p256_compact_tests.rs"]
mod tests;
