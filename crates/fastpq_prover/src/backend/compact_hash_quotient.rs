//! Shared fixed-coset constraint ledger for the 512-row compact hash schedule.
//!
//! The exact invocation is active: compilation fixes the gadget's `active` input
//! to one. Rows 0..407 retain the existing gadget's constraint order, zero-filled
//! to 597 local and 83 transition slots. Rows 408..511 use the first 310 local
//! slots to require zero hash cells. Hash transitions are absent at export,
//! padding and cyclic wrap; the surrounding SMT program must constrain its own
//! semantic carry across those edges.
//!
//! Compilation calls the generic reference gadget once per phase, interns its
//! arithmetic in a deterministic shared DAG, and groups identical slot
//! expressions under summed fixed phase masks. Evaluation executes each shared
//! node once; it never selects an opcode from an LDE index or evaluates every
//! phase's full residue vector at every point. The compiled immutable graph is
//! shared across verifier/prover instances, independently of witness contents.
//!
//! TODO: Combine these slots with the separate SMT/public-boundary ledger, bind
//! the complete schema and degree metadata before challenges, and commit/open
//! every column. Measure complete proof resources before changing admission.
//! This module provides numerators, not a standalone proof or authenticated
//! public ports, and does not remove mandatory replay.

use std::{cell::RefCell, collections::BTreeMap, sync::OnceLock};

use fastpq_isi::StarkParameterSet;

use super::{
    FriDomain, GOLDILOCKS_MODULUS, GoldilocksFp4V1, add_mod, fixed_schedule::PeriodicSelectors,
    mul_mod, sub_mod,
};
use crate::{
    Error, Result,
    gadgets::{
        compact_blake2b_air::{self as hash, CompactRow},
        compact_trace_columns::{hash_row_cells, hash_row_from_cells},
        transfer_integer_air::IntegerAirField,
    },
};

/// Fixed physical rows per invocation, including 104 zero hash scratch rows.
pub(super) const PERIOD: usize = 512;
/// Stable local slots, including the longest import phase and padding equations.
pub(super) const LOCAL_SLOTS: usize = 597;
/// Stable adjacent-row slots; excluded edges contribute exact zero polynomials.
pub(super) const TRANSITION_SLOTS: usize = 83;
const INPUT_CELLS: usize = 2 * hash::COLUMN_COUNT;
#[cfg(test)]
const MAX_PROVER_MASK_CYCLE: usize = 4096;

/// Canonical base or extension values accepted by this ledger's opening boundary.
pub(super) trait LedgerField: IntegerAirField {
    /// First noncanonical base coefficient, if any.
    fn noncanonical_coefficient(self) -> Option<usize>;
    /// Embed a canonical base constant without losing extension coordinates.
    fn embed_base(value: u64) -> Self;
    /// Scale every coordinate by a canonical base-field selector value.
    fn scale_base(self, value: u64) -> Self;
}

impl LedgerField for u64 {
    fn noncanonical_coefficient(self) -> Option<usize> {
        (self >= GOLDILOCKS_MODULUS).then_some(0)
    }

    fn embed_base(value: u64) -> Self {
        value
    }

    fn scale_base(self, value: u64) -> Self {
        mul_mod(self, value)
    }
}

impl LedgerField for GoldilocksFp4V1 {
    fn noncanonical_coefficient(self) -> Option<usize> {
        self.coefficients()
            .iter()
            .position(|&value| value >= GOLDILOCKS_MODULUS)
    }

    fn embed_base(value: u64) -> Self {
        Self::from_base(value).expect("compiled constants and fixed selectors are canonical")
    }

    fn scale_base(self, value: u64) -> Self {
        self.mul_base(value)
    }
}

/// Fixed-slot full-field numerators, before alpha mixing and row-zerofier division.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HashNumerators<F> {
    /// Every local equation in stable schema order.
    pub(super) local: [F; LOCAL_SLOTS],
    /// Every internal hash edge equation in stable schema order.
    pub(super) transitions: [F; TRANSITION_SLOTS],
}

/// Owned arithmetic workspace with exactly one cell per fixed compiled DAG node.
///
/// Allocate one workspace per proof operation or Rayon job and drop it when that
/// operation ends. Every cell is overwritten on each successful evaluation;
/// previous witness values never enter the next result. The workspace holds no
/// masks or domain parameters, and its immutable graph identity is checked before
/// use. It neither grows with the trace nor retains witness data in global state.
#[cfg(test)]
pub(super) struct EvaluationScratch<F> {
    compiled: &'static CompiledLedger,
    values: Box<[F]>,
}

/// Exact graph costs and conservative polynomial degrees for the chosen domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
pub(super) struct LedgerMetrics {
    /// Shared DAG nodes, including canonical constants and the 620 input cells.
    pub(super) nodes: usize,
    /// Add/subtract/multiply DAG nodes evaluated once per point.
    pub(super) arithmetic_nodes: usize,
    /// Distinct known selector masks shared by output slots.
    pub(super) selector_masks: usize,
    /// Contiguous phase runs used to evaluate those masks from 513 prefix sums.
    pub(super) selector_runs: usize,
    /// Mask-weighted shared expressions across all stable output slots.
    pub(super) output_terms: usize,
    /// Number of unshared reference residues, including padding, across one period.
    pub(super) reference_residues_per_period: usize,
    /// Maximum degree in current/next trace variables before fixed selectors.
    pub(super) relation_degree: usize,
    /// Upper bound on the degree of every fixed phase mask.
    pub(super) selector_degree: usize,
    /// Upper bound after composing degree-below-N columns with fixed selectors.
    pub(super) numerator_degree: usize,
    /// Exclusive quotient bound after a satisfied numerator divides by X^N-1.
    pub(super) quotient_degree_bound: usize,
}

/// Validated periodic selector geometry and one immutable compiled hash ledger.
pub(super) struct CompactHashQuotient {
    selectors: PeriodicSelectors,
    #[cfg(test)]
    trace_rows: usize,
    compiled: &'static CompiledLedger,
    #[cfg(test)]
    lde_domain: FriDomain,
    #[cfg(test)]
    lde_rows: usize,
    #[cfg(test)]
    mask_cycle_rows: usize,
}

impl CompactHashQuotient {
    /// Construct the fixed active-invocation ledger on a supported multiple of 512.
    pub(super) fn new(params: &StarkParameterSet, trace_rows: usize) -> Result<Self> {
        let selectors = PeriodicSelectors::new(params, trace_rows, PERIOD)?;
        // Selector construction already checked these exact domain sizes.
        let blowup = params.fri.blowup_factor as usize;
        let lde_rows = trace_rows * blowup;
        let _lde_domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            lde_rows,
            params.omega_coset,
        )?;
        static COMPILED: OnceLock<CompiledLedger> = OnceLock::new();
        Ok(Self {
            selectors,
            #[cfg(test)]
            trace_rows,
            compiled: COMPILED.get_or_init(CompiledLedger::compile),
            #[cfg(test)]
            lde_domain: _lde_domain,
            #[cfg(test)]
            lde_rows,
            #[cfg(test)]
            mask_cycle_rows: PERIOD * blowup,
        })
    }

    /// Evaluate the full stable ledger using verifier-derived phase polynomials.
    ///
    /// `next` is the separately authenticated row polynomial at `g*x`; its index
    /// and Merkle binding belong to the surrounding proof. Every current/next
    /// field coefficient is checked before arithmetic. No active witness input
    /// exists: each execution phase is the exact active single-block invocation.
    pub(super) fn evaluate<F: LedgerField>(
        &self,
        point: u64,
        current: &CompactRow<F>,
        next: &CompactRow<F>,
    ) -> Result<HashNumerators<F>> {
        let inputs = canonical_inputs(current, next)?;
        let phases = self.selectors.evaluate(point)?;
        Ok(self.compiled.evaluate(&phases, &inputs))
    }

    /// Allocate fixed-size arithmetic storage for reuse within one proof operation.
    #[cfg(test)]
    pub(super) fn evaluation_scratch<F: LedgerField>(&self) -> EvaluationScratch<F> {
        EvaluationScratch {
            compiled: self.compiled,
            values: vec![F::ZERO; self.compiled.nodes.len()].into_boxed_slice(),
        }
    }

    /// Prepare only the periodic selector cycle for sequential prover evaluation.
    ///
    /// At canonical blowup eight the fixed masks repeat every 4096 LDE indices,
    /// independently of N. This optional prover-only preparation refuses larger
    /// cycles instead of allocating a full/custom LDE-sized table. The borrowed
    /// view retains its exact owner and cannot be relabelled to another domain.
    #[cfg(test)]
    pub(super) fn prepare_prover_masks(&self) -> Result<ProverMaskCycle<'_>> {
        if self.mask_cycle_rows > MAX_PROVER_MASK_CYCLE {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_compact_hash_prover_mask_cycle",
                actual: self.mask_cycle_rows,
                max: MAX_PROVER_MASK_CYCLE,
            });
        }
        let mut values = Vec::with_capacity(self.mask_cycle_rows * self.compiled.masks.len());
        for index in 0..self.mask_cycle_rows {
            let phases = self.selectors.evaluate(self.lde_domain.point(index))?;
            values.extend(self.compiled.mask_values(&phases));
        }
        Ok(ProverMaskCycle {
            ledger: self,
            values: values.into_boxed_slice(),
        })
    }

    /// Report fixed graph cost and the selector-aware quotient degree bound.
    ///
    /// The masks have degree at most N-N/512. Quadratic relations therefore have
    /// numerator degree at most 3N-N/512-2, hence a quotient below 2N when every
    /// required subgroup relation vanishes. Degree metadata does not establish
    /// vanishing or replace column/quotient degree proofs.
    #[cfg(test)]
    pub(super) fn metrics(&self) -> LedgerMetrics {
        let relation_degree = self.compiled.max_degree;
        let selector_degree = self.trace_rows - self.trace_rows / PERIOD;
        LedgerMetrics {
            nodes: self.compiled.nodes.len(),
            arithmetic_nodes: self
                .compiled
                .nodes
                .iter()
                .filter(|node| node.is_arithmetic())
                .count(),
            selector_masks: self.compiled.masks.len(),
            selector_runs: self.compiled.masks.iter().map(|mask| mask.runs.len()).sum(),
            output_terms: self
                .compiled
                .local
                .iter()
                .chain(&self.compiled.transitions)
                .map(Vec::len)
                .sum(),
            reference_residues_per_period: self.compiled.reference_residues,
            relation_degree,
            selector_degree,
            numerator_degree: relation_degree * (self.trace_rows - 1) + selector_degree,
            quotient_degree_bound: 2 * self.trace_rows,
        }
    }
}

/// Prover-only mask cycle tied by borrow to one exact ledger and LDE geometry.
#[cfg(test)]
pub(super) struct ProverMaskCycle<'a> {
    ledger: &'a CompactHashQuotient,
    values: Box<[u64]>,
}

#[cfg(test)]
impl ProverMaskCycle<'_> {
    /// Evaluate shared arithmetic at one bounded LDE index with cached fixed masks.
    pub(super) fn evaluate<F: LedgerField>(
        &self,
        index: usize,
        current: &CompactRow<F>,
        next: &CompactRow<F>,
    ) -> Result<HashNumerators<F>> {
        if index >= self.ledger.lde_rows {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.ledger.lde_rows,
            });
        }
        let inputs = canonical_inputs(current, next)?;
        let width = self.ledger.compiled.masks.len();
        let start = (index % self.ledger.mask_cycle_rows) * width;
        Ok(self
            .ledger
            .compiled
            .evaluate_masks(&self.values[start..start + width], &inputs))
    }

    /// Evaluate with cached masks and caller-owned fixed-size arithmetic storage.
    ///
    /// This performs the same index and full-field canonicality checks as
    /// `evaluate`, then overwrites the DAG in dependency order without allocating
    /// arithmetic or output vectors. Masks remain tied to this exact ledger's
    /// domain; only the domain-independent arithmetic storage is reusable.
    pub(super) fn evaluate_with_scratch<F: LedgerField>(
        &self,
        index: usize,
        current: &CompactRow<F>,
        next: &CompactRow<F>,
        scratch: &mut EvaluationScratch<F>,
    ) -> Result<HashNumerators<F>> {
        if index >= self.ledger.lde_rows {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.ledger.lde_rows,
            });
        }
        let inputs = canonical_inputs(current, next)?;
        if !core::ptr::eq(scratch.compiled, self.ledger.compiled)
            || scratch.values.len() != self.ledger.compiled.nodes.len()
        {
            return Err(Error::InvalidTraceShape {
                details: "compact hash evaluation scratch belongs to a different graph".to_owned(),
            });
        }
        let width = self.ledger.compiled.masks.len();
        let start = (index % self.ledger.mask_cycle_rows) * width;
        Ok(self.ledger.compiled.evaluate_masks_with_scratch(
            &self.values[start..start + width],
            &inputs,
            &mut scratch.values,
        ))
    }
}

fn canonical_inputs<F: LedgerField>(
    current: &CompactRow<F>,
    next: &CompactRow<F>,
) -> Result<[F; INPUT_CELLS]> {
    let current = hash_row_cells(current);
    let next = hash_row_cells(next);
    for (row, cells) in [&current, &next].into_iter().enumerate() {
        for (column, &value) in cells.iter().enumerate() {
            if let Some(coefficient) = value.noncanonical_coefficient() {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context: "compact_hash_ledger_row",
                    indices: vec![row, column, coefficient],
                });
            }
        }
    }
    Ok(core::array::from_fn(|index| {
        if index < hash::COLUMN_COUNT {
            current[index]
        } else {
            next[index - hash::COLUMN_COUNT]
        }
    }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Constant(u64),
    Input(usize),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
}

impl Node {
    #[cfg(test)]
    fn is_arithmetic(self) -> bool {
        matches!(self, Self::Add(..) | Self::Sub(..) | Self::Mul(..))
    }
}

#[derive(Default)]
struct Builder {
    nodes: Vec<Node>,
    degrees: Vec<usize>,
    interned: BTreeMap<Node, usize>,
}

impl Builder {
    fn intern(&mut self, node: Node) -> usize {
        if let Some(&index) = self.interned.get(&node) {
            return index;
        }
        let degree = match node {
            Node::Constant(_) => 0,
            Node::Input(_) => 1,
            Node::Add(left, right) | Node::Sub(left, right) => {
                self.degrees[left].max(self.degrees[right])
            }
            Node::Mul(left, right) => self.degrees[left] + self.degrees[right],
        };
        let index = self.nodes.len();
        self.nodes.push(node);
        self.degrees.push(degree);
        self.interned.insert(node, index);
        index
    }
}

/// Copy handles borrow one local compilation arena; constants need no arena.
#[derive(Clone, Copy)]
enum Expression<'a> {
    Constant(u64),
    Node(&'a RefCell<Builder>, usize),
}

#[derive(Clone, Copy)]
enum Operation {
    Add,
    Sub,
    Mul,
}

impl<'a> Expression<'a> {
    fn id(self, arena: &'a RefCell<Builder>) -> usize {
        match self {
            Self::Constant(value) => arena.borrow_mut().intern(Node::Constant(value)),
            Self::Node(owner, index) => {
                assert!(core::ptr::eq(owner, arena), "one fixed compilation arena");
                index
            }
        }
    }

    fn binary(self, other: Self, operation: Operation) -> Self {
        if let (Self::Constant(left), Self::Constant(right)) = (self, other) {
            return Self::Constant(match operation {
                Operation::Add => add_mod(left, right),
                Operation::Sub => sub_mod(left, right),
                Operation::Mul => mul_mod(left, right),
            });
        }
        match (operation, self, other) {
            (Operation::Add | Operation::Sub, _, Self::Constant(0))
            | (Operation::Mul, _, Self::Constant(1)) => return self,
            (Operation::Add, Self::Constant(0), _) | (Operation::Mul, Self::Constant(1), _) => {
                return other;
            }
            (Operation::Mul, Self::Constant(0), _) | (Operation::Mul, _, Self::Constant(0)) => {
                return Self::ZERO;
            }
            (Operation::Sub, Self::Node(left_arena, left), Self::Node(right_arena, right))
                if core::ptr::eq(left_arena, right_arena) && left == right =>
            {
                return Self::ZERO;
            }
            _ => {}
        }
        let arena = match (self, other) {
            (Self::Node(arena, _), _) | (_, Self::Node(arena, _)) => arena,
            _ => unreachable!("constant arithmetic handled above"),
        };
        let mut left = self.id(arena);
        let mut right = other.id(arena);
        if matches!(operation, Operation::Add | Operation::Mul) && right < left {
            core::mem::swap(&mut left, &mut right);
        }
        let node = match operation {
            Operation::Add => Node::Add(left, right),
            Operation::Sub => Node::Sub(left, right),
            Operation::Mul => Node::Mul(left, right),
        };
        let index = arena.borrow_mut().intern(node);
        Self::Node(arena, index)
    }
}

impl IntegerAirField for Expression<'_> {
    const ZERO: Self = Self::Constant(0);
    const ONE: Self = Self::Constant(1);

    fn from_u32(value: u32) -> Self {
        Self::Constant(u64::from(value))
    }

    fn add(self, other: Self) -> Self {
        self.binary(other, Operation::Add)
    }

    fn sub(self, other: Self) -> Self {
        self.binary(other, Operation::Sub)
    }

    fn mul(self, other: Self) -> Self {
        self.binary(other, Operation::Mul)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct PhaseSet([u64; PERIOD / 64]);

impl PhaseSet {
    fn insert(&mut self, phase: usize) {
        self.0[phase / 64] |= 1_u64 << (phase % 64);
    }

    fn contains(self, phase: usize) -> bool {
        self.0[phase / 64] & (1_u64 << (phase % 64)) != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PhaseMask {
    // A mask is a sum of disjoint fixed E_r polynomials, represented compactly
    // by maximal half-open phase runs. Prefix sums share the work across masks.
    runs: Vec<(usize, usize)>,
}

impl PhaseMask {
    fn from_set(set: PhaseSet) -> Self {
        let mut runs = Vec::new();
        let mut phase = 0;
        while phase < PERIOD {
            if !set.contains(phase) {
                phase += 1;
                continue;
            }
            let start = phase;
            while phase < PERIOD && set.contains(phase) {
                phase += 1;
            }
            runs.push((start, phase));
        }
        Self { runs }
    }

    fn evaluate(&self, prefixes: &[u64; PERIOD + 1]) -> u64 {
        self.runs.iter().fold(0, |sum, &(start, end)| {
            add_mod(sum, sub_mod(prefixes[end], prefixes[start]))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Term {
    expression: usize,
    mask: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompiledLedger {
    nodes: Vec<Node>,
    masks: Vec<PhaseMask>,
    local: [Vec<Term>; LOCAL_SLOTS],
    transitions: [Vec<Term>; TRANSITION_SLOTS],
    #[cfg(test)]
    max_degree: usize,
    #[cfg(test)]
    reference_residues: usize,
}

impl CompiledLedger {
    fn compile() -> Self {
        let arena = RefCell::new(Builder::default());
        let input: [Expression<'_>; INPUT_CELLS] = core::array::from_fn(|index| {
            let node = arena.borrow_mut().intern(Node::Input(index));
            Expression::Node(&arena, node)
        });
        let current = hash_row_from_cells(&core::array::from_fn(|index| input[index]));
        let next = hash_row_from_cells(&core::array::from_fn(|index| {
            input[hash::COLUMN_COUNT + index]
        }));
        let mut local: [BTreeMap<usize, PhaseSet>; LOCAL_SLOTS] =
            core::array::from_fn(|_| BTreeMap::new());
        let mut transitions: [BTreeMap<usize, PhaseSet>; TRANSITION_SLOTS] =
            core::array::from_fn(|_| BTreeMap::new());
        let mut maximum_local = 0;
        let mut maximum_transition = 0;
        let mut _reference_residues = 0;
        for phase in 0..PERIOD {
            let residues = if let Some(index) = hash::RowIndex::new(phase) {
                hash::local_residues(Expression::ONE, index, &current)
            } else {
                hash_row_cells(&current).to_vec()
            };
            maximum_local = maximum_local.max(residues.len());
            _reference_residues += residues.len();
            group_phase(&arena, phase, &residues, &mut local);
            if let Some(index) = hash::RowIndex::new(phase) {
                if let Some(residues) =
                    hash::transition_residues(Expression::ONE, index, &current, &next)
                {
                    maximum_transition = maximum_transition.max(residues.len());
                    _reference_residues += residues.len();
                    group_phase(&arena, phase, &residues, &mut transitions);
                }
            }
        }
        assert_eq!(
            maximum_local, LOCAL_SLOTS,
            "update the authenticated stable local ledger when the gadget changes"
        );
        assert_eq!(
            maximum_transition, TRANSITION_SLOTS,
            "update the authenticated stable transition ledger when the gadget changes"
        );
        let mut masks = Vec::new();
        let mut mask_ids = BTreeMap::new();
        let mut convert = |groups: BTreeMap<usize, PhaseSet>| {
            groups
                .into_iter()
                .map(|(expression, set)| {
                    let mask = *mask_ids.entry(set).or_insert_with(|| {
                        let index = masks.len();
                        masks.push(PhaseMask::from_set(set));
                        index
                    });
                    Term { expression, mask }
                })
                .collect()
        };
        let local = local.map(&mut convert);
        let transitions = transitions.map(convert);
        let builder = arena.into_inner();
        let max_degree = builder.degrees.into_iter().max().unwrap_or(0);
        assert_eq!(
            max_degree, 2,
            "fixed active hash numerators must remain quadratic"
        );
        Self {
            nodes: builder.nodes,
            masks,
            local,
            transitions,
            #[cfg(test)]
            max_degree,
            #[cfg(test)]
            reference_residues: _reference_residues,
        }
    }

    fn evaluate<F: LedgerField>(
        &self,
        phases: &[u64],
        input: &[F; INPUT_CELLS],
    ) -> HashNumerators<F> {
        let masks = self.mask_values(phases);
        self.evaluate_masks(&masks, input)
    }

    fn mask_values(&self, phases: &[u64]) -> Vec<u64> {
        assert_eq!(
            phases.len(),
            PERIOD,
            "internally derived fixed selector width"
        );
        let mut prefixes = [0; PERIOD + 1];
        for (phase, &value) in phases.iter().enumerate() {
            prefixes[phase + 1] = add_mod(prefixes[phase], value);
        }
        self.masks
            .iter()
            .map(|mask| mask.evaluate(&prefixes))
            .collect()
    }

    fn evaluate_masks<F: LedgerField>(
        &self,
        masks: &[u64],
        input: &[F; INPUT_CELLS],
    ) -> HashNumerators<F> {
        let mut values = vec![F::ZERO; self.nodes.len()];
        self.evaluate_masks_with_scratch(masks, input, &mut values)
    }

    fn evaluate_masks_with_scratch<F: LedgerField>(
        &self,
        masks: &[u64],
        input: &[F; INPUT_CELLS],
        values: &mut [F],
    ) -> HashNumerators<F> {
        assert_eq!(
            masks.len(),
            self.masks.len(),
            "internally derived fixed mask width"
        );
        assert_eq!(
            values.len(),
            self.nodes.len(),
            "fixed compiled scratch width"
        );
        // Interning emits operands before their consumers. Overwrite every node,
        // including constants and inputs, before any output slot reads the DAG.
        for (index, &node) in self.nodes.iter().enumerate() {
            let value = match node {
                Node::Constant(value) => F::embed_base(value),
                Node::Input(index) => input[index],
                Node::Add(left, right) => values[left].add(values[right]),
                Node::Sub(left, right) => values[left].sub(values[right]),
                Node::Mul(left, right) => match (self.nodes[left], self.nodes[right]) {
                    (Node::Constant(value), _) => values[right].scale_base(value),
                    (_, Node::Constant(value)) => values[left].scale_base(value),
                    _ => values[left].mul(values[right]),
                },
            };
            values[index] = value;
        }
        let evaluate_slot = |terms: &Vec<Term>| {
            terms.iter().fold(F::ZERO, |sum, term| {
                sum.add(values[term.expression].scale_base(masks[term.mask]))
            })
        };
        HashNumerators {
            local: core::array::from_fn(|slot| evaluate_slot(&self.local[slot])),
            transitions: core::array::from_fn(|slot| evaluate_slot(&self.transitions[slot])),
        }
    }
}

fn group_phase<'a, const SLOTS: usize>(
    arena: &'a RefCell<Builder>,
    phase: usize,
    residues: &[Expression<'a>],
    groups: &mut [BTreeMap<usize, PhaseSet>; SLOTS],
) {
    assert!(
        residues.len() <= SLOTS,
        "fixed source residue count fits the ledger"
    );
    for (slot, &expression) in residues.iter().enumerate() {
        if matches!(expression, Expression::Constant(0)) {
            continue;
        }
        groups[slot]
            .entry(expression.id(arena))
            .or_default()
            .insert(phase);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backend::{field_pow, fixed_domain::FixedTraceDomain},
        fft::Planner,
    };
    use fastpq_isi::FASTPQ_FINAL_V1;

    fn reference<F: IntegerAirField>(
        phase: usize,
        current: &CompactRow<F>,
        next: &CompactRow<F>,
    ) -> HashNumerators<F> {
        let local = hash::RowIndex::new(phase).map_or_else(
            || hash_row_cells(current).to_vec(),
            |index| hash::local_residues(F::ONE, index, current),
        );
        let transitions = hash::RowIndex::new(phase)
            .and_then(|index| hash::transition_residues(F::ONE, index, current, next))
            .unwrap_or_default();
        HashNumerators {
            local: core::array::from_fn(|slot| local.get(slot).copied().unwrap_or(F::ZERO)),
            transitions: core::array::from_fn(|slot| {
                transitions.get(slot).copied().unwrap_or(F::ZERO)
            }),
        }
    }

    fn seeded_row(seed: u64) -> CompactRow {
        hash_row_from_cells(&core::array::from_fn(|index| {
            let value = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add((index as u64 + 1).wrapping_mul(1442695040888963407_u64));
            value % GOLDILOCKS_MODULUS
        }))
    }

    fn horner(coefficients: &[u64], point: u64) -> u64 {
        coefficients.iter().rev().fold(0, |sum, &coefficient| {
            add_mod(mul_mod(sum, point), coefficient)
        })
    }

    #[test]
    fn compilation_is_deterministic_bounded_and_reports_selector_aware_degree() {
        let first = CompiledLedger::compile();
        let second = CompiledLedger::compile();
        assert_eq!(first, second);
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let metrics = ledger.metrics();
        assert_eq!(
            metrics.reference_residues_per_period,
            166_786 + 104 * hash::COLUMN_COUNT
        );
        assert_eq!(metrics.relation_degree, 2);
        assert_eq!(metrics.selector_degree, PERIOD - 1);
        assert_eq!(metrics.numerator_degree, 3 * PERIOD - 3);
        assert_eq!(metrics.quotient_degree_bound, 2 * PERIOD);
        // These are fixed-source operation budgets, not proof-controlled counts.
        // They detect accidental loss of expression/mask sharing or expansion.
        assert!(metrics.nodes < 32_768, "{metrics:?}");
        assert!(metrics.output_terms < 16_384, "{metrics:?}");
        assert!(metrics.selector_runs < 16_384, "{metrics:?}");
        assert!(metrics.selector_masks < 2_048, "{metrics:?}");
        assert!(
            metrics.arithmetic_nodes + metrics.output_terms
                < metrics.reference_residues_per_period / 4,
            "{metrics:?}"
        );
        let large = CompactHashQuotient::new(&FASTPQ_FINAL_V1, 1 << FASTPQ_FINAL_V1.trace_log_size)
            .unwrap();
        assert!(core::ptr::eq(ledger.compiled, large.compiled));
        assert_eq!(large.metrics().nodes, metrics.nodes);
        assert_eq!(large.metrics().selector_degree, 65_536 - 128);
        assert_eq!(large.metrics().numerator_degree, 3 * 65_536 - 130);
        for rows in [0, 256, 513, 131_072] {
            assert!(CompactHashQuotient::new(&FASTPQ_FINAL_V1, rows).is_err());
        }
    }

    #[test]
    fn every_physical_phase_matches_reference_slot_order_and_zero_filling() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, PERIOD)
            .unwrap()
            .generator;
        let current = seeded_row(17);
        let next = seeded_row(93);
        let mut point = 1;
        for phase in 0..PERIOD {
            assert_eq!(
                ledger.evaluate(point, &current, &next).unwrap(),
                reference(phase, &current, &next),
                "phase={phase}"
            );
            point = mul_mod(point, generator);
        }
        assert_eq!(point, 1);
    }

    #[test]
    fn constant_row_coset_evaluation_matches_interpolated_reference_constraints() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let current = seeded_row(313);
        let next = seeded_row(991);
        let mut columns = vec![vec![0; PERIOD]; LOCAL_SLOTS + TRANSITION_SLOTS];
        for phase in 0..PERIOD {
            let expected = reference(phase, &current, &next);
            for (column, value) in columns
                .iter_mut()
                .zip(expected.local.into_iter().chain(expected.transitions))
            {
                column[phase] = value;
            }
        }
        Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut columns);
        for point in [0, 7, FASTPQ_FINAL_V1.omega_coset, GOLDILOCKS_MODULUS - 2] {
            let actual = ledger.evaluate(point, &current, &next).unwrap();
            for (slot, (value, coefficients)) in actual
                .local
                .into_iter()
                .chain(actual.transitions)
                .zip(&columns)
                .enumerate()
            {
                assert_eq!(value, horner(coefficients, point), "slot={slot}, x={point}");
            }
        }
    }

    #[test]
    fn valid_hash_and_every_padding_cell_obey_the_complete_physical_schedule() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, PERIOD)
            .unwrap()
            .generator;
        let bytes: Vec<_> = (0..83).map(|index| (index * 71 + 13) as u8).collect();
        let witness = hash::CompactHashWitness::from_bytes(&bytes).unwrap();
        let zero = CompactRow::zero();
        let mut point = 1;
        for phase in 0..PERIOD {
            let row = witness.rows().get(phase).unwrap_or(&zero);
            let next = witness.rows().get((phase + 1) % PERIOD).unwrap_or(&zero);
            let result = ledger.evaluate(point, row, next).unwrap();
            assert!(
                result
                    .local
                    .into_iter()
                    .chain(result.transitions)
                    .all(|value| value == 0),
                "phase={phase}"
            );
            point = mul_mod(point, generator);
        }
        let padding_point = field_pow(generator, 408);
        for column in 0..hash::COLUMN_COUNT {
            let mut cells = [0; hash::COLUMN_COUNT];
            cells[column] = 1;
            let row = hash_row_from_cells(&cells);
            let result = ledger.evaluate(padding_point, &row, &zero).unwrap();
            assert_eq!(result.local[column], 1, "padding column={column}");
            assert_eq!(result.local.iter().filter(|&&value| value != 0).count(), 1);
            assert!(result.transitions.iter().all(|&value| value == 0));
        }
        let arbitrary = seeded_row(123);
        for phase in [407, 408, 510, 511] {
            let result = ledger
                .evaluate(field_pow(generator, phase as u64), &arbitrary, &arbitrary)
                .unwrap();
            assert!(
                result.transitions.iter().all(|&value| value == 0),
                "excluded edge={phase}"
            );
        }
        // A next-row mutation on an included copy edge must occupy its stable slot.
        let mut next = witness.rows()[1];
        next.message[0] = add_mod(next.message[0], 1);
        assert_ne!(
            ledger
                .evaluate(1, &witness.rows()[0], &next)
                .unwrap()
                .transitions[0],
            0
        );
    }

    #[test]
    fn full_extension_evaluation_matches_uncompiled_phase_polynomials() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let make_row = |seed: u64| {
            hash_row_from_cells(&core::array::from_fn(|column| {
                GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                    seed + column as u64 * 17 + lane as u64 * 23
                }))
                .unwrap()
            }))
        };
        let current = make_row(1);
        let next = make_row(73);
        let point = FASTPQ_FINAL_V1.omega_coset;
        let phases = ledger.selectors.evaluate(point).unwrap();
        let mut expected = HashNumerators {
            local: [GoldilocksFp4V1::ZERO; LOCAL_SLOTS],
            transitions: [GoldilocksFp4V1::ZERO; TRANSITION_SLOTS],
        };
        for (phase, weight) in phases.into_iter().enumerate() {
            let residue = reference(phase, &current, &next);
            for (value, term) in expected.local.iter_mut().zip(residue.local) {
                *value = value.add(term.mul_base(weight));
            }
            for (value, term) in expected.transitions.iter_mut().zip(residue.transitions) {
                *value = value.add(term.mul_base(weight));
            }
        }
        let actual = ledger.evaluate(point, &current, &next).unwrap();
        assert_eq!(actual, expected);
        for lane in 1..4 {
            assert!(
                actual
                    .local
                    .iter()
                    .any(|value| value.coefficients()[lane] != 0)
            );
        }
        let base_current = seeded_row(51);
        let base_next = seeded_row(199);
        let embed = |row: &CompactRow| {
            hash_row_from_cells(&hash_row_cells(row).map(GoldilocksFp4V1::embed_base))
        };
        let base = ledger.evaluate(point, &base_current, &base_next).unwrap();
        let extension = ledger
            .evaluate(point, &embed(&base_current), &embed(&base_next))
            .unwrap();
        assert_eq!(extension.local, base.local.map(GoldilocksFp4V1::embed_base));
        assert_eq!(
            extension.transitions,
            base.transitions.map(GoldilocksFp4V1::embed_base)
        );
    }

    #[test]
    fn composed_column_degree_is_not_truncated_to_subgroup_interpolation() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let mut execution_mask = vec![vec![0; PERIOD]];
        execution_mask[0][..hash::ROW_COUNT].fill(1);
        Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut execution_mask);
        // Slot1 is bits[0][0]*(bits[0][0]-1) in every execution phase.
        // Set that trace column to X^(N-1), all other columns to zero. Padding
        // slot1 is working[1]=0. Construct the exact full numerator by shifting
        // the independently interpolated selector coefficients, not by reducing
        // the higher-degree product modulo X^N-1.
        let mut numerator = vec![0; 3 * PERIOD - 2];
        for (degree, &coefficient) in execution_mask[0].iter().enumerate() {
            numerator[degree + 2 * PERIOD - 2] =
                add_mod(numerator[degree + 2 * PERIOD - 2], coefficient);
            numerator[degree + PERIOD - 1] = sub_mod(numerator[degree + PERIOD - 1], coefficient);
        }
        assert_ne!(*numerator.last().unwrap(), 0);
        assert_eq!(numerator.len() - 1, ledger.metrics().numerator_degree);
        let mut remainder = numerator.clone();
        let mut quotient = vec![0; numerator.len() - PERIOD];
        for degree in (PERIOD..remainder.len()).rev() {
            let coefficient = remainder[degree];
            quotient[degree - PERIOD] = coefficient;
            remainder[degree] = 0;
            remainder[degree - PERIOD] = add_mod(remainder[degree - PERIOD], coefficient);
        }
        assert_ne!(*quotient.last().unwrap(), 0);
        assert!(quotient.len() <= ledger.metrics().quotient_degree_bound);
        let zero = CompactRow::zero();
        for point in [7, FASTPQ_FINAL_V1.omega_coset] {
            let mut row = zero;
            row.bits[0][0] = field_pow(point, (PERIOD - 1) as u64);
            let actual = ledger.evaluate(point, &row, &zero).unwrap().local[1];
            assert_eq!(actual, horner(&numerator, point));
            assert_eq!(
                actual,
                add_mod(
                    mul_mod(
                        sub_mod(field_pow(point, PERIOD as u64), 1),
                        horner(&quotient, point)
                    ),
                    horner(&remainder[..PERIOD], point)
                )
            );
        }
    }

    #[test]
    fn all_current_next_columns_and_extension_coordinates_reject_noncanonical_values() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let zero = CompactRow::<u64>::zero();
        for side in 0..2 {
            for column in 0..hash::COLUMN_COUNT {
                let mut cells = [0; hash::COLUMN_COUNT];
                cells[column] = GOLDILOCKS_MODULUS;
                let bad = hash_row_from_cells(&cells);
                let (current, next) = if side == 0 {
                    (&bad, &zero)
                } else {
                    (&zero, &bad)
                };
                assert!(
                    matches!(ledger.evaluate(7, current, next), Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [side, column, 0])
                );
                for lane in 0..4 {
                    let mut cells = [GoldilocksFp4V1::ZERO; hash::COLUMN_COUNT];
                    let mut coefficients = [0; 4];
                    coefficients[lane] = GOLDILOCKS_MODULUS;
                    cells[column] =
                        GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
                    let bad = hash_row_from_cells(&cells);
                    let zero = CompactRow::<GoldilocksFp4V1>::zero();
                    let (current, next) = if side == 0 {
                        (&bad, &zero)
                    } else {
                        (&zero, &bad)
                    };
                    assert!(
                        matches!(ledger.evaluate(7, current, next), Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [side, column, lane])
                    );
                }
            }
        }
        for point in [GOLDILOCKS_MODULUS, u64::MAX] {
            assert!(ledger.evaluate(point, &zero, &zero).is_err());
        }
    }

    #[test]
    fn fixed_scratch_reuse_matches_direct_evaluation_on_small_and_shifted_domains() {
        for trace_rows in [PERIOD, 2 * PERIOD] {
            let mut params = FASTPQ_FINAL_V1;
            if trace_rows == 2 * PERIOD {
                // A one-step rotation is a different valid representative of
                // the same disjoint LDE coset. Its masks must follow that exact
                // owner geometry, including the second periodic cycle.
                let original = CompactHashQuotient::new(&params, trace_rows).unwrap();
                params.omega_coset = original.lde_domain.point(1);
                assert_ne!(params.omega_coset, FASTPQ_FINAL_V1.omega_coset);
            }
            let ledger = CompactHashQuotient::new(&params, trace_rows).unwrap();
            let cycle = ledger.prepare_prover_masks().unwrap();
            assert!(core::ptr::eq(cycle.ledger, &ledger));
            let mut base_scratch = ledger.evaluation_scratch::<u64>();
            let mut extension_scratch = ledger.evaluation_scratch::<GoldilocksFp4V1>();
            let base_pointer = base_scratch.values.as_ptr();
            let extension_pointer = extension_scratch.values.as_ptr();
            assert_eq!(base_scratch.values.len(), ledger.metrics().nodes);
            assert_eq!(extension_scratch.values.len(), ledger.metrics().nodes);
            let first = seeded_row(31);
            let second = seeded_row(73);
            let zero = CompactRow::<u64>::zero();
            let extension_row = |seed: u64| {
                hash_row_from_cells(&core::array::from_fn(|column| {
                    GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                        seed + 17 * column as u64 + 23 * lane as u64
                    }))
                    .unwrap()
                }))
            };
            let extension_first = extension_row(19);
            let extension_second = extension_row(61);
            let extension_zero = CompactRow::<GoldilocksFp4V1>::zero();
            let mut indices = vec![0, 1, 407, 408, 4095, ledger.lde_rows - 1, 0];
            if ledger.lde_rows > ledger.mask_cycle_rows {
                indices.extend([ledger.mask_cycle_rows, ledger.mask_cycle_rows + 1]);
            }
            for index in indices {
                let point = ledger.lde_domain.point(index);
                // Poison all scratch cells. Topological evaluation must replace
                // even constants/inputs before any arithmetic consumes them.
                base_scratch.values.fill(GOLDILOCKS_MODULUS);
                extension_scratch.values.fill(
                    GoldilocksFp4V1::from_coefficients_unchecked_for_test([
                        0,
                        0,
                        GOLDILOCKS_MODULUS,
                        0,
                    ]),
                );
                for (current, next) in [
                    (&first, &second),
                    (&second, &first),
                    (&zero, &zero),
                    (&first, &second),
                ] {
                    assert_eq!(
                        cycle
                            .evaluate_with_scratch(index, current, next, &mut base_scratch)
                            .unwrap(),
                        ledger.evaluate(point, current, next).unwrap(),
                        "base trace_rows={trace_rows}, index={index}"
                    );
                }
                for (current, next) in [
                    (&extension_first, &extension_second),
                    (&extension_second, &extension_first),
                    (&extension_zero, &extension_zero),
                    (&extension_first, &extension_second),
                ] {
                    assert_eq!(
                        cycle
                            .evaluate_with_scratch(index, current, next, &mut extension_scratch)
                            .unwrap(),
                        ledger.evaluate(point, current, next).unwrap(),
                        "extension trace_rows={trace_rows}, index={index}"
                    );
                }
                // Exercise a trace column of maximum allowed degree N-1. The
                // quadratic slot must evaluate its full composition, including
                // the phase mask, without subgroup-degree truncation.
                let mut high_degree = zero;
                let value = field_pow(point, (trace_rows - 1) as u64);
                high_degree.bits[0][0] = value;
                let numerator = cycle
                    .evaluate_with_scratch(index, &high_degree, &zero, &mut base_scratch)
                    .unwrap();
                let phases = ledger.selectors.evaluate(point).unwrap();
                let execution_mask = phases[..hash::ROW_COUNT].iter().copied().fold(0, add_mod);
                assert_eq!(
                    numerator.local[1],
                    mul_mod(execution_mask, mul_mod(value, sub_mod(value, 1)))
                );
                assert_eq!(
                    numerator,
                    ledger.evaluate(point, &high_degree, &zero).unwrap()
                );
                assert_eq!(base_scratch.values.as_ptr(), base_pointer);
                assert_eq!(extension_scratch.values.as_ptr(), extension_pointer);
                assert_eq!(base_scratch.values.len(), ledger.metrics().nodes);
                assert_eq!(extension_scratch.values.len(), ledger.metrics().nodes);
            }
        }
    }

    #[test]
    fn scratch_evaluation_preserves_checked_errors_and_recovers_after_rejection() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, PERIOD).unwrap();
        let cycle = ledger.prepare_prover_masks().unwrap();
        let current = seeded_row(5);
        let next = seeded_row(11);
        let mut scratch = ledger.evaluation_scratch::<u64>();
        let expected = cycle.evaluate(0, &current, &next).unwrap();
        let mut bad_current = current;
        bad_current.message[0] = GOLDILOCKS_MODULUS;
        let mut bad_next = next;
        bad_next.digest[7] = GOLDILOCKS_MODULUS;
        for (row, next_row) in [(&bad_current, &next), (&current, &bad_next)] {
            let direct = cycle.evaluate(0, row, next_row).unwrap_err();
            let reused = cycle
                .evaluate_with_scratch(0, row, next_row, &mut scratch)
                .unwrap_err();
            assert_eq!(format!("{reused}"), format!("{direct}"));
        }
        for index in [ledger.lde_rows, usize::MAX] {
            assert!(matches!(
                cycle.evaluate_with_scratch(index, &bad_current, &bad_next, &mut scratch),
                Err(Error::QueryIndexOutOfRange { index: found, len })
                    if found == index && len == ledger.lde_rows
            ));
        }
        assert_eq!(
            cycle
                .evaluate_with_scratch(0, &current, &next, &mut scratch)
                .unwrap(),
            expected
        );
        let mut extension_scratch = ledger.evaluation_scratch::<GoldilocksFp4V1>();
        let zero = CompactRow::<GoldilocksFp4V1>::zero();
        for lane in 0..4 {
            let mut coefficients = [0; 4];
            coefficients[lane] = GOLDILOCKS_MODULUS;
            let mut invalid = zero;
            invalid.digest[7] = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
            for (current, next) in [(&invalid, &zero), (&zero, &invalid)] {
                let direct = cycle.evaluate(0, current, next).unwrap_err();
                let reused = cycle
                    .evaluate_with_scratch(0, current, next, &mut extension_scratch)
                    .unwrap_err();
                assert_eq!(format!("{reused}"), format!("{direct}"));
            }
        }
        assert_eq!(
            cycle
                .evaluate_with_scratch(0, &zero, &zero, &mut extension_scratch)
                .unwrap(),
            cycle.evaluate(0, &zero, &zero).unwrap()
        );
        // Production callers cannot mutate this private size; exercise the
        // defensive graph-width check without allocating a second compiled DAG.
        scratch.values = Box::new([]);
        assert!(matches!(
            cycle.evaluate_with_scratch(0, &current, &next, &mut scratch),
            Err(Error::InvalidTraceShape { .. })
        ));
    }

    #[test]
    fn bounded_prover_mask_cycle_matches_queries_and_reports_actual_cost() {
        let ledger = CompactHashQuotient::new(&FASTPQ_FINAL_V1, 65_536).unwrap();
        let preparation_start = std::time::Instant::now();
        let cycle = ledger.prepare_prover_masks().unwrap();
        let preparation_elapsed = preparation_start.elapsed();
        assert!(core::ptr::eq(cycle.ledger, &ledger));
        assert_eq!(ledger.mask_cycle_rows, 4096);
        assert_eq!(cycle.values.len(), 4096 * ledger.compiled.masks.len());
        let current = seeded_row(31);
        let next = seeded_row(73);
        for index in [
            0,
            1,
            407,
            408,
            4095,
            4096,
            4097,
            65_535,
            ledger.lde_rows - 1,
        ] {
            let direct = ledger
                .evaluate(ledger.lde_domain.point(index), &current, &next)
                .unwrap();
            assert_eq!(
                cycle.evaluate(index, &current, &next).unwrap(),
                direct,
                "LDE index={index}"
            );
        }
        for index in [ledger.lde_rows, usize::MAX] {
            assert!(cycle.evaluate(index, &current, &next).is_err());
        }
        let mut invalid = current;
        invalid.digest[7] = GOLDILOCKS_MODULUS;
        assert!(cycle.evaluate(0, &invalid, &next).is_err());
        assert!(cycle.evaluate(0, &current, &invalid).is_err());
        let mut custom = FASTPQ_FINAL_V1;
        custom.fri.blowup_factor = 16;
        let larger_cycle = CompactHashQuotient::new(&custom, 512).unwrap();
        assert!(matches!(
            larger_cycle.prepare_prover_masks(),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_hash_prover_mask_cycle",
                actual: 8192,
                max: 4096
            })
        ));

        let extension_row = |seed: u64| {
            hash_row_from_cells(&core::array::from_fn(|column| {
                GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                    seed + 7 * column as u64 + 13 * lane as u64
                }))
                .unwrap()
            }))
        };
        let extension_current = extension_row(43);
        let extension_next = extension_row(91);
        assert_eq!(
            cycle
                .evaluate(4097, &extension_current, &extension_next)
                .unwrap(),
            ledger
                .evaluate(
                    ledger.lde_domain.point(4097),
                    &extension_current,
                    &extension_next
                )
                .unwrap()
        );

        // Timings are evidence only: no machine-speed assertion affects validity.
        let base_start = std::time::Instant::now();
        for index in 0..100 {
            std::hint::black_box(
                ledger
                    .evaluate(
                        ledger.lde_domain.point(index),
                        std::hint::black_box(&current),
                        &next,
                    )
                    .unwrap(),
            );
        }
        let base_elapsed = base_start.elapsed();
        let extension_start = std::time::Instant::now();
        for index in 0..100 {
            std::hint::black_box(
                ledger
                    .evaluate(
                        ledger.lde_domain.point(index),
                        std::hint::black_box(&extension_current),
                        &extension_next,
                    )
                    .unwrap(),
            );
        }
        let extension_elapsed = extension_start.elapsed();
        let cached_base_start = std::time::Instant::now();
        for index in 0..100 {
            std::hint::black_box(
                cycle
                    .evaluate(index, std::hint::black_box(&current), &next)
                    .unwrap(),
            );
        }
        let cached_base_elapsed = cached_base_start.elapsed();
        let cached_extension_start = std::time::Instant::now();
        for index in 0..100 {
            std::hint::black_box(
                cycle
                    .evaluate(
                        index,
                        std::hint::black_box(&extension_current),
                        &extension_next,
                    )
                    .unwrap(),
            );
        }
        let cached_extension_elapsed = cached_extension_start.elapsed();
        let mut base_scratch = ledger.evaluation_scratch::<u64>();
        let scratch_base_start = std::time::Instant::now();
        for index in 0..100 {
            std::hint::black_box(
                cycle
                    .evaluate_with_scratch(
                        index,
                        std::hint::black_box(&current),
                        &next,
                        &mut base_scratch,
                    )
                    .unwrap(),
            );
        }
        let scratch_base_elapsed = scratch_base_start.elapsed();
        let mut extension_scratch = ledger.evaluation_scratch::<GoldilocksFp4V1>();
        let scratch_extension_start = std::time::Instant::now();
        for index in 0..100 {
            std::hint::black_box(
                cycle
                    .evaluate_with_scratch(
                        index,
                        std::hint::black_box(&extension_current),
                        &extension_next,
                        &mut extension_scratch,
                    )
                    .unwrap(),
            );
        }
        let scratch_extension_elapsed = scratch_extension_start.elapsed();
        eprintln!(
            "compact_hash_ledger_metrics={:?}; mask_cycle_rows={}; mask_cycle_bytes={}; preparation={preparation_elapsed:?}; base_100={base_elapsed:?}; fp4_100={extension_elapsed:?}; cached_base_100={cached_base_elapsed:?}; cached_fp4_100={cached_extension_elapsed:?}; scratch_base_100={scratch_base_elapsed:?}; scratch_fp4_100={scratch_extension_elapsed:?}; base_scratch_bytes={}; fp4_scratch_bytes={}",
            ledger.metrics(),
            ledger.mask_cycle_rows,
            core::mem::size_of_val(&*cycle.values),
            core::mem::size_of_val(&*base_scratch.values),
            core::mem::size_of_val(&*extension_scratch.values)
        );
    }
}
