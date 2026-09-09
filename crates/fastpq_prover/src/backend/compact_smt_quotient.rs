//! Fixed-coset ledger for the compact SMT program's semantic constraints.
//!
//! This module adds no BLAKE2b equations. It binds exact node-domain and input
//! ports, public leaves/roots, and all carried SMT state around 512-row hashes.
//! The schema is exactly 65,536 rows by 342 cells with active=1. Stable residue
//! slots do not depend on a queried row label: verifier-known polynomials are
//! evaluated at the actual canonical field point.
//!
//! Every combined mask is interpolated directly, or formed by linear sums of
//! fixed polynomials. No two selectors are multiplied. Each fixed polynomial
//! has degree <N, and every residue is linear in the trace cells times a fixed
//! coefficient: numerator degree <=2N-2 and quotient degree <N after division
//! by X^N-1. The separate hash ledger still needs its conservative <2N bound.
//!
//! There are 49 sparse fixed columns at exactly 704 positions, plus 512 periodic
//! phases. Sparse field payload is 275,968 bytes; verifier storage/work depends
//! on these public constants, not the private trace/LDE size. Provers can obtain
//! the sparse rows and precompute their LDEs once. Verifiers use `evaluate_fixed`
//! without constructing a trace or performing an FFT. Calling that public-table
//! evaluator separately for every prover LDE point would be needlessly costly.
//!
//! TODO: Bind this schema, full authenticated public claims, profile, and public
//! inputs before challenges; combine it with the reviewed hash ledger, trace
//! commitments and joint degree proof. Fixed-value constructors are not proof
//! authentication. This module does not change admission or remove replay.

use std::collections::BTreeMap;

use super::{
    GOLDILOCKS_MODULUS, fixed_schedule::PeriodicSelectors, public_table::PublicTablePolynomial,
};
use crate::{
    Error, Result,
    gadgets::{
        compact_smt_air::{
            HASH_COUNT, PHYSICAL_HASH_ROWS, PHYSICAL_ROW_COUNT, PHYSICAL_ROWS_PER_UPDATE,
            PublicStatement, SmtRow,
        },
        transfer_integer_air::IntegerAirField,
    },
};
use fastpq_isi::StarkParameterSet;

/// Exact stable SMT-only residue slots, excluding all hash constraints.
pub(super) const RESIDUE_COUNT: usize = 243;
/// Sparse verifier-known coefficient columns in this fixed schema.
pub(super) const FIXED_COLUMN_COUNT: usize = 49;
/// Union of four import rows, one export row per hash, and new-hash padding ends.
pub(super) const FIXED_ROW_COUNT: usize = 704;
const NODE_DOMAIN: &[u8; 19] = b"fastpq:v1:smt:node|";
const EXPORT_PHASE: usize = 407;
const LAST_PHASE: usize = PHYSICAL_HASH_ROWS - 1;
const PORT_MASKS: usize = 24;
const OLD_EXPORT: usize = PORT_MASKS;
const NEW_EXPORT: usize = 25;
const SIBLING_FREE: usize = 26;
const UPDATE_RESET: usize = 27;
const FINAL_ROW: usize = 28;
const LEAF_ROWS: usize = 29;
const OLD_ROOT_ROWS: usize = 30;
const FINAL_EXPORT: usize = 31;
const FIRST_ROW: usize = 32;
const OLD_LEAVES: usize = 33;
const NEW_LEAVES: usize = 41;

const DOMAIN_SLOT: usize = 1;
const MARKER_SLOT: usize = 153;
const PORT_SLOT: usize = 155;
const LEAF_SLOT: usize = 171;
const SOURCE_ROOT_SLOT: usize = 187;
const OLD_ROOT_SLOT: usize = 195;
const FINAL_ROOT_SLOT: usize = 203;
const START_CARRY_SLOT: usize = 211;
const OLD_CARRY_SLOT: usize = 219;
const NEW_CARRY_SLOT: usize = 227;
const SIBLING_CARRY_SLOT: usize = 235;

fn port_mask(port: usize, phase: usize, source: usize) -> usize {
    12 * port + 3 * phase + source
}

/// Bounded public fixed columns, derived from a complete externally authenticated statement.
pub(super) struct CompactSmtFixedColumns {
    statement: PublicStatement,
    positions: Vec<usize>,
    rows: Vec<Vec<u64>>,
}

impl CompactSmtFixedColumns {
    /// Prepare exact sparse masks; this does not authenticate the statement's origin.
    pub(super) fn new(statement: &PublicStatement) -> Result<Self> {
        for digest in [
            statement.old_root,
            statement.new_root,
            statement.updates[0].old_leaf,
            statement.updates[0].new_leaf,
            statement.updates[1].old_leaf,
            statement.updates[1].new_leaf,
        ] {
            if digest[7] & (1 << 24) == 0 {
                return Err(shape_error(
                    "compact SMT public digest marker is not canonical",
                ));
            }
        }
        let mut sparse = BTreeMap::<usize, Vec<u64>>::new();
        for hash in 0..HASH_COUNT {
            let update = hash / 64;
            let level = (hash / 2) % 32;
            let child_port = ((statement.updates[update].path >> level) & 1) as usize;
            for phase in 0..4 {
                let row = sparse
                    .entry(hash * PHYSICAL_HASH_ROWS + phase)
                    .or_insert_with(|| vec![0; FIXED_COLUMN_COUNT]);
                for port in 0..2 {
                    let source = if port == child_port { hash % 2 } else { 2 };
                    row[port_mask(port, phase, source)] = 1;
                }
            }
            let export = sparse
                .entry(hash * PHYSICAL_HASH_ROWS + EXPORT_PHASE)
                .or_insert_with(|| vec![0; FIXED_COLUMN_COUNT]);
            export[if hash % 2 == 0 {
                OLD_EXPORT
            } else {
                NEW_EXPORT
            }] = 1;
            if hash % 64 == 63 {
                export[OLD_ROOT_ROWS] = 1;
            }
            if hash == HASH_COUNT - 1 {
                export[FINAL_EXPORT] = 1;
            }
            if hash % 2 == 1 {
                let padding = sparse
                    .entry(hash * PHYSICAL_HASH_ROWS + LAST_PHASE)
                    .or_insert_with(|| vec![0; FIXED_COLUMN_COUNT]);
                padding[SIBLING_FREE] = 1;
                if hash % 64 == 63 {
                    padding[OLD_ROOT_ROWS] = 1;
                }
                if hash == 63 {
                    padding[UPDATE_RESET] = 1;
                }
                if hash == HASH_COUNT - 1 {
                    padding[FINAL_ROW] = 1;
                }
            }
        }
        for update in 0..2 {
            let row = sparse
                .get_mut(&(update * PHYSICAL_ROWS_PER_UPDATE))
                .expect("every first import row was created");
            row[LEAF_ROWS] = 1;
            if update == 0 {
                row[FIRST_ROW] = 1;
            }
            for limb in 0..8 {
                row[OLD_LEAVES + limb] = u64::from(statement.updates[update].old_leaf[limb]);
                row[NEW_LEAVES + limb] = u64::from(statement.updates[update].new_leaf[limb]);
            }
        }
        let (positions, rows) = sparse.into_iter().unzip();
        Ok(Self {
            statement: *statement,
            positions,
            rows,
        })
    }

    /// Exact ordered subgroup positions for optional prover-side fixed-column FFTs.
    pub(super) fn positions(&self) -> &[usize] {
        &self.positions
    }
    /// All 49 canonical fixed values per position; omitted subgroup rows are zero.
    pub(super) fn rows(&self) -> &[Vec<u64>] {
        &self.rows
    }
}

/// Separately cacheable fixed values at one canonical evaluation point.
pub(super) struct CompactSmtFixedValues {
    phases: [u64; PHYSICAL_HASH_ROWS],
    sparse: [u64; FIXED_COLUMN_COUNT],
}

impl CompactSmtFixedValues {
    /// Check field encoding of trusted fixed evaluations, not proof-supplied masks.
    ///
    /// A prover may call this after evaluating the declared fixed columns by FFT.
    /// A verifier must derive these values itself using `evaluate_fixed`.
    pub(super) fn new(
        phases: [u64; PHYSICAL_HASH_ROWS],
        sparse: [u64; FIXED_COLUMN_COUNT],
    ) -> Result<Self> {
        for (column, value) in phases.iter().chain(sparse.iter()).copied().enumerate() {
            if value >= GOLDILOCKS_MODULUS {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context: "compact_smt_fixed_evaluation",
                    indices: vec![column],
                });
            }
        }
        Ok(Self { phases, sparse })
    }
}

/// Prepared verifier geometry borrowing the bounded public coefficient table.
pub(super) struct CompactSmtQuotient<'a> {
    fixed: &'a CompactSmtFixedColumns,
    selectors: PeriodicSelectors,
    table: PublicTablePolynomial<'a>,
}

impl<'a> CompactSmtQuotient<'a> {
    /// Validate exact trace/LDE geometry once without constructing a private trace.
    pub(super) fn new(
        params: &StarkParameterSet,
        fixed: &'a CompactSmtFixedColumns,
    ) -> Result<Self> {
        Ok(Self {
            fixed,
            selectors: PeriodicSelectors::new(params, PHYSICAL_ROW_COUNT, PHYSICAL_HASH_ROWS)?,
            table: PublicTablePolynomial::new_sparse(
                params,
                PHYSICAL_ROW_COUNT,
                FIXED_COLUMN_COUNT,
                &fixed.rows,
                &fixed.positions,
                FIXED_ROW_COUNT,
            )?,
        })
    }

    /// Evaluate known coefficients with O(704×49+512+log N) public field work.
    pub(super) fn evaluate_fixed(&self, point: u64) -> Result<CompactSmtFixedValues> {
        let phases = self
            .selectors
            .evaluate(point)?
            .try_into()
            .ok()
            .expect("fixed selector period");
        let sparse = self
            .table
            .evaluate(point)?
            .try_into()
            .ok()
            .expect("fixed public coefficient width");
        CompactSmtFixedValues::new(phases, sparse)
    }

    /// Evaluate the stable ledger from current/next openings and precomputed fixed values.
    ///
    /// The next opening is at the effective trace generator times the current
    /// point. Every untrusted trace cell must already have canonical field encoding.
    pub(super) fn residues<F: IntegerAirField>(
        &self,
        values: &CompactSmtFixedValues,
        current: &SmtRow<F>,
        next: &SmtRow<F>,
    ) -> [F; RESIDUE_COUNT] {
        numerators(
            &values.phases.map(lift_base),
            &values.sparse.map(lift_base),
            current,
            next,
            &self.fixed.statement,
        )
    }
}

fn lift_base<F: IntegerAirField>(value: u64) -> F {
    let two32 = F::from_u32(65_536).mul(F::from_u32(65_536));
    F::from_u32(value as u32).add(F::from_u32((value >> 32) as u32).mul(two32))
}

fn input_bit<F: Copy>(row: &SmtRow<F>, byte: usize, bit: usize) -> F {
    row.hash.bits[byte / 8][8 * (byte % 8) + bit]
}

fn input_limb<F: IntegerAirField>(current: &SmtRow<F>, next: &SmtRow<F>, offset: usize) -> F {
    let mut packed = F::ZERO;
    let mut weight = F::ONE;
    for bit in 0..32 {
        let byte = offset + bit / 8;
        let source = if byte < 24 { current } else { next };
        packed = packed.add(input_bit(source, byte % 24, bit % 8).mul(weight));
        weight = weight.add(weight);
    }
    packed
}

fn numerators<F: IntegerAirField>(
    phases: &[F; PHYSICAL_HASH_ROWS],
    known: &[F; FIXED_COLUMN_COUNT],
    current: &SmtRow<F>,
    next: &SmtRow<F>,
    statement: &PublicStatement,
) -> [F; RESIDUE_COUNT] {
    let mut out = [F::ZERO; RESIDUE_COUNT];
    let executing = phases[..=EXPORT_PHASE]
        .iter()
        .copied()
        .fold(F::ZERO, F::add);
    out[0] = executing.mul(current.hash.byte_len.sub(F::from_u32(83)));
    for (byte, value) in NODE_DOMAIN.iter().copied().enumerate() {
        for bit in 0..8 {
            out[DOMAIN_SLOT + 8 * byte + bit] = phases[0]
                .mul(input_bit(current, byte, bit).sub(F::from_u32(u32::from((value >> bit) & 1))));
        }
    }
    for (slot, marker) in [50_usize, 82].into_iter().enumerate() {
        out[MARKER_SLOT + slot] =
            phases[marker / 24].mul(input_bit(current, marker % 24, 0).sub(F::ONE));
    }
    for port in 0..2 {
        for limb in 0..8 {
            let start = 19 + 32 * port + 4 * limb;
            let phase = start / 24;
            out[PORT_SLOT + 8 * port + limb] = phases[phase]
                .mul(input_limb(current, next, start % 24))
                .sub(known[port_mask(port, phase, 0)].mul(current.old_child[limb]))
                .sub(known[port_mask(port, phase, 1)].mul(current.new_child[limb]))
                .sub(known[port_mask(port, phase, 2)].mul(current.sibling[limb]));
        }
    }
    let edge = F::ONE.sub(known[FINAL_ROW]);
    let ordinary = edge.sub(known[UPDATE_RESET]);
    for limb in 0..8 {
        out[LEAF_SLOT + limb] = known[LEAF_ROWS]
            .mul(current.old_child[limb])
            .sub(known[OLD_LEAVES + limb]);
        out[LEAF_SLOT + 8 + limb] = known[LEAF_ROWS]
            .mul(current.new_child[limb])
            .sub(known[NEW_LEAVES + limb]);
        out[SOURCE_ROOT_SLOT + limb] = known[FIRST_ROW]
            .mul(current.starting_root[limb].sub(F::from_u32(statement.old_root[limb])));
        out[OLD_ROOT_SLOT + limb] =
            known[OLD_ROOT_ROWS].mul(current.old_child[limb].sub(current.starting_root[limb]));
        out[FINAL_ROOT_SLOT + limb] = known[FINAL_EXPORT]
            .mul(current.hash.digest[limb])
            .add(known[FINAL_ROW].mul(current.new_child[limb]))
            .sub(
                known[FINAL_EXPORT]
                    .add(known[FINAL_ROW])
                    .mul(F::from_u32(statement.new_root[limb])),
            );
        out[START_CARRY_SLOT + limb] = edge
            .mul(next.starting_root[limb])
            .sub(ordinary.mul(current.starting_root[limb]))
            .sub(known[UPDATE_RESET].mul(current.new_child[limb]));
        out[OLD_CARRY_SLOT + limb] = ordinary
            .mul(next.old_child[limb].sub(current.old_child[limb]))
            .add(known[OLD_EXPORT].mul(current.old_child[limb].sub(current.hash.digest[limb])));
        out[NEW_CARRY_SLOT + limb] = ordinary
            .mul(next.new_child[limb].sub(current.new_child[limb]))
            .add(known[NEW_EXPORT].mul(current.new_child[limb].sub(current.hash.digest[limb])));
        out[SIBLING_CARRY_SLOT + limb] = F::ONE
            .sub(known[SIBLING_FREE])
            .mul(next.sibling[limb].sub(current.sibling[limb]));
    }
    out
}

fn shape_error(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GoldilocksFp4V1,
        backend::{add_mod, field_pow, fixed_domain::FixedTraceDomain, mul_mod},
        fft::Planner,
        gadgets::{
            compact_blake2b_air,
            compact_smt_air::{
                PhysicalRowIndex, PublicUpdate, physical_local_residues,
                physical_transition_residues,
            },
            compact_trace_columns::{smt_row_cells, smt_row_from_cells},
        },
    };
    use fastpq_isi::FASTPQ_FINAL_V1;
    use iroha_crypto::Hash;

    fn digest(seed: u8) -> [u32; 8] {
        let hash = Hash::new([seed; 33]);
        let bytes = hash.as_ref();
        core::array::from_fn(|i| u32::from_le_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap()))
    }

    fn statement() -> PublicStatement {
        PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: digest(1),
                    new_leaf: digest(2),
                    path: 0xa59c_71e3,
                },
                PublicUpdate {
                    old_leaf: digest(3),
                    new_leaf: digest(4),
                    path: 0x6a35_8e1c,
                },
            ],
            old_root: digest(5),
            new_root: digest(6),
        }
    }

    fn fixed_at_row(fixed: &CompactSmtFixedColumns, index: usize) -> CompactSmtFixedValues {
        let mut phases = [0; PHYSICAL_HASH_ROWS];
        phases[index % PHYSICAL_HASH_ROWS] = 1;
        let mut sparse = [0; FIXED_COLUMN_COUNT];
        if let Ok(position) = fixed.positions.binary_search(&index) {
            sparse.copy_from_slice(&fixed.rows[position]);
        }
        CompactSmtFixedValues::new(phases, sparse).unwrap()
    }

    fn arbitrary_row(seed: u64) -> SmtRow {
        smt_row_from_cells(&core::array::from_fn(|i| {
            (seed + 97 * i as u64 + 3).wrapping_mul(0x7abc_def0_1234_5678) % GOLDILOCKS_MODULUS
        }))
    }

    // Read the physical reference's semantic suffixes, preserving all actual
    // numerators. Only their variable local ordering is remapped to stable slots;
    // no SMT equation is recomputed by this differential adapter.
    fn reference<F: IntegerAirField>(
        index: usize,
        current: &SmtRow<F>,
        next: &SmtRow<F>,
        public: &PublicStatement,
    ) -> [F; RESIDUE_COUNT] {
        let position = PhysicalRowIndex::new(index).unwrap();
        let phase = position.phase();
        let mut out = [F::ZERO; RESIDUE_COUNT];
        let hash_local_count = position.logical_index().map_or(310, |logical| {
            compact_blake2b_air::local_residues(F::ONE, logical.hash_index(), &current.hash).len()
        });
        let mut local = physical_local_residues(F::ONE, position, current, public)
            .into_iter()
            .skip(hash_local_count);
        if !position.is_padding() {
            out[0] = local.next().unwrap();
            if phase == 0 {
                for value in &mut out[DOMAIN_SLOT..MARKER_SLOT] {
                    *value = local.next().unwrap();
                }
            }
            for (slot, marker) in [50_usize, 82].into_iter().enumerate() {
                if phase == marker / 24 {
                    out[MARKER_SLOT + slot] = local.next().unwrap();
                }
            }
            for port in 0..2 {
                for limb in 0..8 {
                    let start = 19 + 32 * port + 4 * limb;
                    if start / 24 == phase && start % 24 <= 20 {
                        out[PORT_SLOT + 8 * port + limb] = local.next().unwrap();
                    }
                }
            }
            if index % PHYSICAL_ROWS_PER_UPDATE == 0 {
                for limb in 0..8 {
                    out[LEAF_SLOT + limb] = local.next().unwrap();
                    out[LEAF_SLOT + 8 + limb] = local.next().unwrap();
                    if index == 0 {
                        out[SOURCE_ROOT_SLOT + limb] = local.next().unwrap();
                    }
                }
            }
        }
        let update_root =
            position.hash_ordinal() % 64 == 63 && (phase == EXPORT_PHASE || phase == LAST_PHASE);
        if update_root {
            for value in &mut out[OLD_ROOT_SLOT..FINAL_ROOT_SLOT] {
                *value = local.next().unwrap();
            }
        }
        if position.hash_ordinal() == HASH_COUNT - 1
            && (phase == EXPORT_PHASE || phase == LAST_PHASE)
        {
            for value in &mut out[FINAL_ROOT_SLOT..START_CARRY_SLOT] {
                *value = local.next().unwrap();
            }
        }
        assert!(local.next().is_none());
        if index == PHYSICAL_ROW_COUNT - 1 {
            return out;
        }
        let hash_transition_count = if phase < EXPORT_PHASE {
            compact_blake2b_air::transition_residues(
                F::ONE,
                position.logical_index().unwrap().hash_index(),
                &current.hash,
                &next.hash,
            )
            .unwrap()
            .len()
        } else {
            0
        };
        let mut transition = physical_transition_residues(F::ONE, position, current, next, public)
            .unwrap()
            .into_iter()
            .skip(hash_transition_count);
        if phase < EXPORT_PHASE {
            for port in 0..2 {
                for limb in 0..8 {
                    let start = 19 + 32 * port + 4 * limb;
                    if start / 24 == phase && start % 24 > 20 {
                        out[PORT_SLOT + 8 * port + limb] = transition.next().unwrap();
                    }
                }
            }
        }
        for limb in 0..8 {
            out[START_CARRY_SLOT + limb] = transition.next().unwrap();
            if index == PHYSICAL_ROWS_PER_UPDATE - 1 {
                continue;
            }
            out[OLD_CARRY_SLOT + limb] = transition.next().unwrap();
            out[NEW_CARRY_SLOT + limb] = transition.next().unwrap();
            if phase != LAST_PHASE || !position.is_new() {
                out[SIBLING_CARRY_SLOT + limb] = transition.next().unwrap();
            }
        }
        assert!(transition.next().is_none());
        out
    }

    #[test]
    fn every_phase_and_special_boundary_matches_physical_semantic_suffixes() {
        let public = statement();
        let fixed = CompactSmtFixedColumns::new(&public).unwrap();
        let ledger = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &fixed).unwrap();
        for hash in [0, 1, 2, 63, 64, 65, 126, 127] {
            for phase in 0..PHYSICAL_HASH_ROWS {
                let index = hash * PHYSICAL_HASH_ROWS + phase;
                let current = arbitrary_row(index as u64);
                let next = arbitrary_row(index as u64 + 3);
                assert_eq!(
                    ledger.residues(&fixed_at_row(&fixed, index), &current, &next),
                    reference(index, &current, &next, &public),
                    "physical row {index}"
                );
            }
        }
        // Exercise every publicly fixed path bit, including both child roles.
        for hash in 0..HASH_COUNT {
            for phase in 0..4 {
                let index = hash * PHYSICAL_HASH_ROWS + phase;
                let current = arbitrary_row(index as u64);
                let next = arbitrary_row(index as u64 + 3);
                assert_eq!(
                    ledger.residues(&fixed_at_row(&fixed, index), &current, &next),
                    reference(index, &current, &next, &public)
                );
            }
        }
    }

    #[test]
    fn every_cell_mutation_matches_reference_at_all_boundary_kinds() {
        let public = statement();
        let fixed = CompactSmtFixedColumns::new(&public).unwrap();
        let ledger = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &fixed).unwrap();
        for index in [
            0, 1, 2, 3, 407, 408, 511, 919, 1023, 32663, 32767, 32768, 65431, 65535,
        ] {
            let current = arbitrary_row(index as u64);
            let next = arbitrary_row(index as u64 + 3);
            let values = fixed_at_row(&fixed, index);
            for cell in 0..342 {
                for mutate_next in [false, true] {
                    let mut cells = smt_row_cells(if mutate_next { &next } else { &current });
                    cells[cell] = add_mod(cells[cell], 1);
                    let changed = smt_row_from_cells(&cells);
                    let (a, b) = if mutate_next {
                        (&current, &changed)
                    } else {
                        (&changed, &next)
                    };
                    assert_eq!(
                        ledger.residues(&values, a, b),
                        reference(index, a, b, &public),
                        "row {index}, cell {cell}, next {mutate_next}"
                    );
                }
            }
        }
    }

    #[test]
    fn fixed_subgroup_and_coset_values_match_independent_ifft_horner() {
        let fixed = CompactSmtFixedColumns::new(&statement()).unwrap();
        let ledger = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &fixed).unwrap();
        let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, PHYSICAL_ROW_COUNT)
            .unwrap()
            .generator;
        for index in [
            0, 3, 4, 407, 408, 511, 512, 1023, 32767, 32768, 65431, 65535,
        ] {
            let expected = fixed_at_row(&fixed, index);
            let actual = ledger
                .evaluate_fixed(field_pow(generator, index as u64))
                .unwrap();
            assert_eq!(actual.phases, expected.phases);
            assert_eq!(actual.sparse, expected.sparse);
        }
        let selected = [0, OLD_EXPORT, OLD_LEAVES + 7, FINAL_ROW];
        let mut columns = vec![vec![0; PHYSICAL_ROW_COUNT]; selected.len()];
        for (&position, row) in fixed.positions().iter().zip(fixed.rows()) {
            for (column, &source) in columns.iter_mut().zip(&selected) {
                column[position] = row[source];
            }
        }
        Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut columns);
        for point in [0, 1, 7, FASTPQ_FINAL_V1.omega_coset, GOLDILOCKS_MODULUS - 1] {
            let actual = ledger.evaluate_fixed(point).unwrap();
            for (&source, coefficients) in selected.iter().zip(&columns) {
                let expected = coefficients.iter().rev().fold(0, |sum, &coefficient| {
                    add_mod(mul_mod(sum, point), coefficient)
                });
                assert_eq!(
                    actual.sparse[source], expected,
                    "fixed column {source}, x {point}"
                );
            }
        }
        assert!(ledger.evaluate_fixed(GOLDILOCKS_MODULUS).is_err());
    }

    #[test]
    fn extension_evaluation_preserves_all_reference_equations_and_coset_coefficients() {
        let public = statement();
        let fixed = CompactSmtFixedColumns::new(&public).unwrap();
        let ledger = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &fixed).unwrap();
        let extension = smt_row_from_cells(&core::array::from_fn(|i| {
            GoldilocksFp4V1::new([i as u64 + 1, i as u64 + 2, 3, GOLDILOCKS_MODULUS - 1]).unwrap()
        }));
        for index in [0, 1, 2, 3, 407, 408, 511, 1023, 32767, 32768, 65431, 65535] {
            assert_eq!(
                ledger.residues(&fixed_at_row(&fixed, index), &extension, &extension),
                reference(index, &extension, &extension, &public)
            );
        }
        let embed = |value| GoldilocksFp4V1::from_base(value).unwrap();
        let current = arbitrary_row(1);
        let next = arbitrary_row(2);
        let values = ledger.evaluate_fixed(FASTPQ_FINAL_V1.omega_coset).unwrap();
        assert_eq!(
            ledger.residues(&values, &current, &next).map(embed),
            ledger.residues(
                &values,
                &smt_row_from_cells(&smt_row_cells(&current).map(embed)),
                &smt_row_from_cells(&smt_row_cells(&next).map(embed))
            )
        );
        for base in [0, 1, 1 << 32, GOLDILOCKS_MODULUS - 1] {
            assert_eq!(lift_base::<GoldilocksFp4V1>(base), embed(base));
        }
    }

    #[derive(Clone, Copy)]
    struct Degree(usize);
    impl IntegerAirField for Degree {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(0);
        fn from_u32(_: u32) -> Self {
            Self(0)
        }
        fn add(self, rhs: Self) -> Self {
            Self(self.0.max(rhs.0))
        }
        fn sub(self, rhs: Self) -> Self {
            Self(self.0.max(rhs.0))
        }
        fn mul(self, rhs: Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }

    #[test]
    fn complete_fixed_and_trace_degree_and_resource_bounds_are_explicit() {
        let public = statement();
        let fixed = CompactSmtFixedColumns::new(&public).unwrap();
        assert_eq!(fixed.rows().len(), FIXED_ROW_COUNT);
        assert_eq!(fixed.positions().len(), FIXED_ROW_COUNT);
        assert!(
            fixed
                .rows()
                .iter()
                .all(|row| row.len() == FIXED_COLUMN_COUNT)
        );
        assert!(fixed.positions().windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(FIXED_ROW_COUNT * FIXED_COLUMN_COUNT * 8, 275_968);
        assert_eq!(PHYSICAL_ROW_COUNT, 65_536);
        let trace_degree = PHYSICAL_ROW_COUNT - 1;
        let row = smt_row_from_cells(&[Degree(trace_degree); 342]);
        let residues = numerators(
            &[Degree(trace_degree); PHYSICAL_HASH_ROWS],
            &[Degree(trace_degree); FIXED_COLUMN_COUNT],
            &row,
            &row,
            &public,
        );
        let degree = residues.iter().map(|value| value.0).max().unwrap();
        assert_eq!(degree, 2 * PHYSICAL_ROW_COUNT - 2);
        assert!(degree - PHYSICAL_ROW_COUNT < PHYSICAL_ROW_COUNT);
        assert_eq!(RESIDUE_COUNT, SIBLING_CARRY_SLOT + 8);
        assert!(core::mem::size_of::<CompactSmtFixedValues>() < 5 * 1024);
    }

    #[test]
    fn malformed_fixed_coordinates_and_public_markers_are_rejected() {
        for column in 0..PHYSICAL_HASH_ROWS + FIXED_COLUMN_COUNT {
            for bad in [GOLDILOCKS_MODULUS, u64::MAX] {
                let mut phases = [0; PHYSICAL_HASH_ROWS];
                let mut sparse = [0; FIXED_COLUMN_COUNT];
                if column < PHYSICAL_HASH_ROWS {
                    phases[column] = bad;
                } else {
                    sparse[column - PHYSICAL_HASH_ROWS] = bad;
                }
                assert!(matches!(CompactSmtFixedValues::new(phases, sparse),
                    Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [column]));
            }
        }
        for digest in 0..6 {
            let mut public = statement();
            let selected = match digest {
                0 => &mut public.old_root,
                1 => &mut public.new_root,
                2 => &mut public.updates[0].old_leaf,
                3 => &mut public.updates[0].new_leaf,
                4 => &mut public.updates[1].old_leaf,
                _ => &mut public.updates[1].new_leaf,
            };
            selected[7] &= !(1 << 24);
            assert!(CompactSmtFixedColumns::new(&public).is_err());
        }
    }
}
