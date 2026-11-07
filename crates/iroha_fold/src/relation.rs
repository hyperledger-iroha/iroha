//! The folded relation of one `iroha_plonk` circuit.
//!
//! Compiled from the circuit's constraint system and its fixed tables (selectors,
//! fixed columns, copy cycles), never from witness values, so a key-generation
//! synthesis and a witness synthesis of one circuit yield the same relation.
//!
//! Constraint indices are row-major: constraint `c` on row `r` has index
//! `r * 2^constraint_bits + c`. `ProtoGalaxy`'s `pow` weights use that index.
//! Gate polynomials and `LogUp` slot constraints whose row sets never meet (their
//! fixed guards are never nonzero on the same row) share one constraint index, a
//! *constraint class*: on every row at most one member of a class is constrained,
//! so the class constraint is that member's constraint or exactly zero. Each
//! group's table constraint, whose activity depends on the witness, has its own
//! index after the classes. Classes keep the index space, hence `F(X)` and a
//! decider's public weights, small.

use iroha_pasta::PastaField;
use iroha_plonk::{
    cs::{Any, Expression},
    frontend::Synthesized,
};

use crate::{
    Error,
    logup::{self, LookupGroup},
    program::{MAX_POINTS, Point, Program, Tables, folded_degree, split_guard},
};

/// One gate polynomial with the rows on which its constant guard is nonzero.
#[derive(Clone, Debug)]
pub(crate) struct GateConstraint<F> {
    pub(crate) degree: usize,
    pub(crate) body: Program<F>,
    pub(crate) rows: Vec<u32>,
    pub(crate) guards: Vec<F>,
    /// Constraint index (class).
    pub(crate) class: usize,
    /// The whole polynomial (guard and body), retained for deciders.
    pub(crate) expression: Expression<F>,
}

/// One cell of a copy constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Cell {
    Advice(u32, u32),
    Fixed(u32, u32),
    Instance(u32, u32),
}

/// The advice columns and the instance columns of one stage.
pub type StageColumns<F> = (Vec<Vec<F>>, Vec<Vec<F>>);

/// Sizes of a [`FoldedRelation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationSummary {
    /// Gate polynomials.
    pub gate_polynomials: usize,
    /// Largest folded degree of a gate polynomial.
    pub max_gate_degree: usize,
    /// Guarded (polynomial, row) pairs.
    pub gate_rows: usize,
    /// Usable rows of the domain.
    pub usable_rows: usize,
    /// `LogUp` groups.
    pub lookup_groups: usize,
    /// Reciprocal cells over all groups and slots.
    pub reciprocal_cells: usize,
    /// Copy-constraint edges.
    pub copies: usize,
    /// Committed witness columns.
    pub columns: usize,
    /// Largest folded degree of any constraint.
    pub degree: usize,
    /// Constraint classes (gate polynomials and slots sharing an index).
    pub constraint_classes: usize,
}

/// The column layout of one `LogUp` group: its multiplicity column, then its
/// reciprocal columns, then its table column `g_column`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupShape {
    /// Witness column of the multiplicities (committed in round one).
    pub m_column: usize,
    /// Witness column of the table-side reciprocals; the slot columns lie between.
    pub g_column: usize,
}

/// What a fold verifier needs to know of a relation: the committed columns and
/// their rounds, the public column lengths, the challenge counts and the degree.
/// Relations with equal shapes fold through the same verifier circuit, and a
/// circuit's shape does not depend on how many rows its layout uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoldShape {
    /// Committed witness columns.
    pub columns: usize,
    /// Advice columns, committed first in round one.
    pub advice_columns: usize,
    /// The `LogUp` groups in relation order.
    pub groups: Vec<GroupShape>,
    /// Declared instance column lengths.
    pub instance_lengths: Vec<usize>,
    /// `theta` powers of an instance.
    pub theta_len: usize,
    /// Constraint-index bits: the `beta` length and the degree of `F(X)`.
    pub index_bits: usize,
    /// Largest folded degree of any constraint (`K(X)` has `degree - 1` coefficients).
    pub degree: usize,
}

/// The folded relation of one circuit (one side and operation kind).
#[derive(Clone, Debug)]
pub struct FoldedRelation<F> {
    pub(crate) n: usize,
    pub(crate) usable: usize,
    pub(crate) advice_columns: usize,
    pub(crate) instance_lengths: Vec<usize>,
    pub(crate) fixed: Vec<Vec<F>>,
    pub(crate) selectors: Vec<Vec<bool>>,
    pub(crate) gates: Vec<GateConstraint<F>>,
    pub(crate) groups: Vec<LookupGroup<F>>,
    pub(crate) copies: Vec<(Cell, Cell)>,
    pub(crate) degree: usize,
    pub(crate) constraint_bits: usize,
    pub(crate) theta_len: usize,
    pub(crate) classes: usize,
}

/// Greedy colouring of gate polynomials and `LogUp` slots by row-set
/// disjointness (largest row sets first); sets every member's class and each
/// group's table constraint (after the classes). Returns the number of classes.
fn assign_classes<F>(n: usize, gates: &mut [GateConstraint<F>], groups: &mut [LookupGroup<F>]) -> usize {
    let words = n.div_ceil(64);
    // (gate index or usize::MAX, group, slot) with its rows.
    let mut items: Vec<(Option<usize>, usize, usize, Vec<u32>)> = Vec::new();
    for (index, gate) in gates.iter().enumerate() {
        items.push((Some(index), 0, 0, gate.rows.clone()));
    }
    for (g, group) in groups.iter().enumerate() {
        for (slot, cells) in group.slots.iter().enumerate() {
            items.push((None, g, slot, cells.iter().map(|cell| cell.row).collect()));
        }
    }
    items.sort_by_key(|item| core::cmp::Reverse(item.3.len()));
    let mut occupied: Vec<Vec<u64>> = Vec::new();
    for (gate, g, slot, rows) in items {
        let fits = |bits: &Vec<u64>| rows.iter().all(|r| bits[*r as usize / 64] >> (*r % 64) & 1 == 0);
        let class = match occupied.iter().position(fits) {
            Some(class) => class,
            None => {
                occupied.push(vec![0; words]);
                occupied.len() - 1
            }
        };
        for r in &rows {
            occupied[class][*r as usize / 64] |= 1 << (*r % 64);
        }
        match gate {
            Some(index) => gates[index].class = class,
            None => groups[g].slot_constraints[slot] = class,
        }
    }
    let classes = occupied.len();
    for (g, group) in groups.iter_mut().enumerate() {
        group.table_constraint = classes + g;
    }
    classes
}

fn u32_of(value: usize) -> u32 {
    u32::try_from(value).expect("indices and rows fit u32")
}

impl<F: PastaField> FoldedRelation<F> {
    /// Compiles the folded relation of a synthesized circuit (with or without
    /// advice values).
    ///
    /// # Errors
    /// [`Error::Degree`] when a constraint's folded degree exceeds what one
    /// multi-point walk evaluates.
    pub fn from_synthesized(synthesized: &Synthesized<F>) -> Result<Self, Error> {
        let cs = &synthesized.cs;
        let assigned = &synthesized.tables;
        let n = assigned.n();
        let usable = assigned.usable_rows();
        let fixed = assigned.fixed().to_vec();
        let selectors = assigned.selectors().to_vec();
        let tables = Tables { fixed: &fixed, selectors: &selectors, n };
        let empty: [Vec<F>; 0] = [];
        let point = Point { advice: &empty, instance: &empty };
        let mut stack = Vec::with_capacity(64);
        let mut gates = Vec::new();
        let mut degree = 2;
        for gate in cs.gates() {
            for polynomial in gate.polynomials() {
                let (guard, body, _) = split_guard(polynomial);
                let mut rows = Vec::new();
                let mut guards = Vec::new();
                for row in 0..n {
                    let value = guard.eval(&tables, point, row, &mut stack);
                    if !bool::from(value.is_zero()) {
                        rows.push(u32_of(row));
                        guards.push(value);
                    }
                }
                let poly_degree = folded_degree(polynomial);
                degree = degree.max(poly_degree);
                gates.push(GateConstraint {
                    degree: poly_degree,
                    body,
                    rows,
                    guards,
                    class: 0,
                    expression: polynomial.clone(),
                });
            }
        }
        let first_column = cs.num_advice_columns();
        let (mut groups, theta_len) = logup::compile(cs, &tables, usable, first_column);
        for group in &groups {
            let width = group.members.first().map_or(1, |m| m.inputs.len());
            let input_degree = group
                .members
                .iter()
                .map(|m| {
                    cs.lookups()[m.lookup]
                        .input_expressions()
                        .iter()
                        .map(folded_degree)
                        .max()
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
            // A compressed component theta_j * a has degree deg(a) + 1 (j > 0).
            let compressed = if width > 1 { input_degree + 1 } else { input_degree };
            let per_cell = group.slots.iter().flatten().map(|c| c.members().len()).max().unwrap_or(1);
            degree = degree.max(1 + per_cell * compressed.max(1));
            let table_degree = cs.lookups()[group.members[0].lookup]
                .table_expressions()
                .iter()
                .map(folded_degree)
                .max()
                .unwrap_or(0);
            let table_compressed = if width > 1 { table_degree + 1 } else { table_degree };
            degree = degree.max(1 + table_compressed.max(1));
        }
        if degree > MAX_POINTS + 1 {
            return Err(Error::Degree(degree));
        }
        let classes = assign_classes(n, &mut gates, &mut groups);
        let constraints = classes + groups.len();
        let constraint_bits = constraints.max(1).next_power_of_two().trailing_zeros() as usize;
        let permutation = assigned.permutation();
        let columns = permutation.columns();
        let cell = |position: usize, row: usize| {
            let column = columns[position];
            let (index, row) = (u32_of(column.index()), u32_of(row));
            match column.column_type() {
                Any::Advice => Cell::Advice(index, row),
                Any::Fixed => Cell::Fixed(index, row),
                Any::Instance => Cell::Instance(index, row),
            }
        };
        let mut copies = Vec::new();
        for position in 0..columns.len() {
            for row in 0..n {
                if let Some(target) = permutation.mapping(position, row)
                    && target != (position, row)
                {
                    copies.push((cell(position, row), cell(target.0, target.1)));
                }
            }
        }
        Ok(Self {
            n,
            usable,
            advice_columns: cs.num_advice_columns(),
            instance_lengths: cs.instance_lengths().to_vec(),
            fixed,
            selectors,
            gates,
            groups,
            copies,
            degree,
            constraint_bits,
            theta_len,
            classes,
        })
    }

    /// Sizes of the relation.
    #[must_use]
    pub fn summary(&self) -> RelationSummary {
        RelationSummary {
            gate_polynomials: self.gates.len(),
            max_gate_degree: self.gates.iter().map(|g| g.degree).max().unwrap_or(0),
            gate_rows: self.gates.iter().map(|g| g.rows.len()).sum(),
            usable_rows: self.usable,
            lookup_groups: self.groups.len(),
            reciprocal_cells: self.groups.iter().flat_map(|g| g.slots.iter()).map(Vec::len).sum(),
            copies: self.copies.len(),
            columns: self.columns(),
            degree: self.degree,
            constraint_classes: self.classes,
        }
    }

    /// The shape a fold verifier of this relation needs.
    #[must_use]
    pub fn shape(&self) -> FoldShape {
        FoldShape {
            columns: self.columns(),
            advice_columns: self.advice_columns,
            groups: self
                .groups
                .iter()
                .map(|g| GroupShape { m_column: g.m_column(), g_column: g.g_column() })
                .collect(),
            instance_lengths: self.instance_lengths.clone(),
            theta_len: self.theta_len,
            index_bits: self.index_bits(),
            degree: self.degree,
        }
    }

    /// Domain size `2^k`.
    #[must_use]
    pub fn n(&self) -> usize {
        self.n
    }

    /// Maximum folded degree of any constraint.
    #[must_use]
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Committed witness columns: advice, then per `LogUp` group `m`, every `h_s`, `g`.
    #[must_use]
    pub fn columns(&self) -> usize {
        self.advice_columns + self.groups.iter().map(LookupGroup::columns).sum::<usize>()
    }

    /// Number of `LogUp` groups (one challenge `r` each).
    #[must_use]
    pub fn groups(&self) -> usize {
        self.groups.len()
    }

    /// Number of `theta` powers an instance carries.
    #[must_use]
    pub fn theta_len(&self) -> usize {
        self.theta_len
    }

    /// Bits of the row-major constraint index: `constraint_bits + log2(n)`.
    #[must_use]
    pub fn index_bits(&self) -> usize {
        self.constraint_bits + self.n.trailing_zeros() as usize
    }

    /// Declared instance column lengths.
    #[must_use]
    pub fn instance_lengths(&self) -> &[usize] {
        &self.instance_lengths
    }

    pub(crate) fn tables(&self) -> Tables<'_, F> {
        Tables { fixed: &self.fixed, selectors: &self.selectors, n: self.n }
    }

    /// The advice and instance columns of a synthesized stage of this relation.
    ///
    /// # Errors
    /// [`Error::MissingWitness`] for a key-generation synthesis; [`Error::Shape`]
    /// when the stage does not match the relation.
    pub fn stage_witness(&self, synthesized: &Synthesized<F>) -> Result<StageColumns<F>, Error> {
        let advice = synthesized.tables.advice().ok_or(Error::MissingWitness)?.to_vec();
        let instance = synthesized.tables.instance().ok_or(Error::MissingWitness)?.to_vec();
        if advice.len() != self.advice_columns || advice.iter().any(|c| c.len() != self.n) {
            return Err(Error::Shape("advice columns"));
        }
        if instance.iter().map(Vec::len).ne(self.instance_lengths.iter().copied()) {
            return Err(Error::Shape("instance columns"));
        }
        Ok((advice, instance))
    }
}
