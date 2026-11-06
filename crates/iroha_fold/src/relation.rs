//! The folded relation of one `iroha_plonk` circuit.
//!
//! Compiled from the circuit's constraint system and its fixed tables (selectors,
//! fixed columns, copy cycles), never from witness values, so a key-generation
//! synthesis and a witness synthesis of one circuit yield the same relation.
//!
//! Constraint indices are row-major: constraint `p` on row `r` has index
//! `r * 2^constraint_bits + p`, where `p` runs over the gate polynomials, then each
//! `LogUp` group's slots and its table constraint. `ProtoGalaxy`'s `pow` weights use
//! that index.

use iroha_pasta::PastaField;
use iroha_plonk::{cs::Any, frontend::Synthesized};

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
}

/// One cell of a copy constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
                });
            }
        }
        let first_column = cs.num_advice_columns();
        let (groups, theta_len) = logup::compile(cs, &tables, usable, first_column, gates.len());
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
        let constraints = gates.len() + groups.iter().map(LookupGroup::constraints).sum::<usize>();
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
