//! `LogUp` re-arithmetization of `iroha_plonk` lookups for folding.
//!
//! Lookups whose table expressions are equal share one *group*: one table side
//! (multiplicities `m` and reciprocals `g = m / (r + S)`) and one challenge `r`.
//! An input tuple is compressed as `A = a_0 + theta_1 a_1 + ...`; the powers
//! `theta_j` are separate instance variables so the compressed input has folded
//! degree two. Inputs of single-column lookups on one row share reciprocal cells,
//! at most three per cell (`h (r+a)(r+b)(r+c) = sum of pair products`, degree
//! four); tuple lookups use one cell per input (`h (r + A) = 1`, degree three).
//!
//! Activity is structural: an input that is a sum of guarded terms (each a product
//! with a factor of fixed and selector columns only, as the running-sum range
//! check `step (z - 2^k z') + limb z`) is active on the usable rows where the guard
//! of any of its terms is nonzero; an input with an unguarded term is active on
//! every usable row. A lookup is active where any of its inputs is. On every
//! other usable row each term vanishes, so its tuple is exactly zero whatever the
//! witness; one such row stays active so that the zero tuple's membership, which
//! the original lookup asserts there, is checked too.

use std::collections::HashMap;

use iroha_pasta::PastaField;
use iroha_plonk::cs::{ConstraintSystem, Expression};

use crate::{
    Error,
    program::{Point, Program, Tables, split_guard},
};

/// Inputs sharing one reciprocal cell, at most this many.
pub const CELL_INPUTS: usize = 3;

/// One reciprocal cell: its row and the group members whose inputs it sums.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HCell {
    pub(crate) row: u32,
    pub(crate) members: [u16; CELL_INPUTS],
    pub(crate) count: u8,
}

impl HCell {
    pub(crate) fn members(&self) -> &[u16] {
        &self.members[..usize::from(self.count)]
    }
}

/// One lookup of a group.
#[derive(Clone, Debug)]
pub(crate) struct Member<F> {
    pub(crate) lookup: usize,
    pub(crate) inputs: Vec<Program<F>>,
    /// The input expressions, retained for deciders.
    pub(crate) expressions: Vec<Expression<F>>,
}

/// Lookups sharing one table expression.
#[derive(Clone, Debug)]
pub struct LookupGroup<F> {
    pub(crate) table: Vec<Program<F>>,
    /// The table expressions, retained for deciders.
    pub(crate) table_expressions: Vec<Expression<F>>,
    pub(crate) members: Vec<Member<F>>,
    /// The fixed members of each slot (slot `s` is witness column `h_s`).
    pub(crate) slot_members: Vec<Vec<u16>>,
    /// Reciprocal cells per slot: every row where any of its members is active.
    pub(crate) slots: Vec<Vec<HCell>>,
    /// Usable rows: the table side is constrained on each.
    pub(crate) usable: usize,
    /// Witness column of `m`; `h_s` follow, then `g`.
    pub(crate) first_column: usize,
    /// Constraint index of each slot (its constraint class).
    pub(crate) slot_constraints: Vec<usize>,
    /// Constraint index of the table constraint.
    pub(crate) table_constraint: usize,
}

impl<F> LookupGroup<F> {
    /// Witness columns of the group: `m`, every `h_s`, `g`.
    pub(crate) fn columns(&self) -> usize {
        self.slots.len() + 2
    }
    pub(crate) fn m_column(&self) -> usize {
        self.first_column
    }
    pub(crate) fn h_column(&self, slot: usize) -> usize {
        self.first_column + 1 + slot
    }
    pub(crate) fn g_column(&self) -> usize {
        self.first_column + 1 + self.slots.len()
    }
}

/// Compiles the lookups of `cs` into groups; `first_column` is the first free
/// witness column. Constraint indices are assigned by the caller (constraint
/// classes).
pub(crate) fn compile<F: PastaField>(
    cs: &ConstraintSystem<F>,
    tables: &Tables<'_, F>,
    usable: usize,
    first_column: usize,
) -> (Vec<LookupGroup<F>>, usize) {
    let mut keys: Vec<String> = Vec::new();
    let mut grouped: Vec<Vec<usize>> = Vec::new();
    let mut theta_len = 0usize;
    for (index, lookup) in cs.lookups().iter().enumerate() {
        let key = format!("{:?}", lookup.table_expressions());
        theta_len = theta_len.max(lookup.width().saturating_sub(1));
        if let Some(group) = keys.iter().position(|k| *k == key) {
            grouped[group].push(index);
        } else {
            keys.push(key);
            grouped.push(vec![index]);
        }
    }
    let empty: [Vec<F>; 0] = [];
    let point = Point { advice: &empty, instance: &empty };
    let mut stack = Vec::with_capacity(64);
    let mut groups = Vec::with_capacity(grouped.len());
    let mut column = first_column;
    for indices in grouped {
        let lookups = cs.lookups();
        let width = lookups[indices[0]].width();
        let table_expressions = lookups[indices[0]].table_expressions().to_vec();
        let table = table_expressions.iter().map(Program::compile).collect();
        let mut members = Vec::with_capacity(indices.len());
        // Per member: the usable rows on which it is active.
        let mut active_rows: Vec<Vec<bool>> = Vec::with_capacity(indices.len());
        for &index in &indices {
            let lookup = &lookups[index];
            let mut guards = Vec::new();
            let mut everywhere = false;
            for input in lookup.input_expressions() {
                match term_guards(input) {
                    Some(terms) => guards.extend(terms),
                    None => everywhere = true,
                }
            }
            let mut active = vec![everywhere; usable];
            if !everywhere {
                for (row, slot) in active.iter_mut().enumerate() {
                    *slot = guards
                        .iter()
                        .any(|g| !bool::from(g.eval(tables, point, row, &mut stack).is_zero()));
                }
                // Keep the all-zero tuple of an inactive row checked once.
                if let Some(row) = active.iter().position(|a| !*a) {
                    active[row] = true;
                }
            }
            active_rows.push(active);
            let expressions = lookup.input_expressions().to_vec();
            members.push(Member {
                lookup: index,
                inputs: expressions.iter().map(Program::compile).collect(),
                expressions,
            });
        }
        // Fixed slots: members with similar activity share a slot (at most three
        // single-column members, or one tuple member). A slot's cells are every
        // row where any of its members is active; a member inactive on such a row
        // contributes its exactly zero tuple, whose membership the original lookup
        // asserts on that row too.
        let per_cell = if width == 1 { CELL_INPUTS } else { 1 };
        let counts: Vec<usize> = active_rows.iter().map(|a| a.iter().filter(|x| **x).count()).collect();
        let firsts: Vec<usize> = active_rows.iter().map(|a| a.iter().position(|x| *x).unwrap_or(usable)).collect();
        let mut order: Vec<usize> = (0..members.len()).collect();
        order.sort_by_key(|m| (core::cmp::Reverse(counts[*m]), firsts[*m], *m));
        let slot_members: Vec<Vec<u16>> = order
            .chunks(per_cell)
            .map(|chunk| chunk.iter().map(|m| u16::try_from(*m).expect("members fit u16")).collect())
            .collect();
        let slots = slot_members
            .iter()
            .map(|chosen| {
                let mut cell = HCell {
                    row: 0,
                    members: [0; CELL_INPUTS],
                    count: u8::try_from(chosen.len()).expect("at most three"),
                };
                cell.members[..chosen.len()].copy_from_slice(chosen);
                (0..usable)
                    .filter(|row| chosen.iter().any(|m| active_rows[usize::from(*m)][*row]))
                    .map(|row| HCell { row: u32::try_from(row).expect("rows fit u32"), ..cell })
                    .collect()
            })
            .collect::<Vec<Vec<HCell>>>();
        let group = LookupGroup {
            table,
            table_expressions,
            members,
            slot_constraints: vec![0; slot_members.len()],
            slot_members,
            slots,
            usable,
            first_column: column,
            table_constraint: 0,
        };
        column += group.columns();
        groups.push(group);
    }
    (groups, theta_len)
}

/// The guards of the terms of `input` (a sum of guarded products), or `None` when
/// a term has no guard and the input may be nonzero on every row.
fn term_guards<F: PastaField>(input: &Expression<F>) -> Option<Vec<Program<F>>> {
    match input {
        Expression::Sum(left, right) => {
            let mut guards = term_guards(left)?;
            guards.extend(term_guards(right)?);
            Some(guards)
        }
        Expression::Negated(inner) | Expression::Scaled(inner, _) => term_guards(inner),
        Expression::Constant(value) if bool::from(value.is_zero()) => Some(Vec::new()),
        _ => {
            let (guard, _, trivial) = split_guard(input);
            (!trivial && !guard.is_one()).then(|| vec![guard])
        }
    }
}

/// Compressed tuple `a_0 + theta_1 a_1 + ...` of `programs` at one point.
pub(crate) fn compress<F: PastaField>(
    programs: &[Program<F>],
    theta: &[F],
    tables: &Tables<'_, F>,
    point: Point<'_, F>,
    row: usize,
    stack: &mut Vec<F>,
) -> F {
    let mut acc = F::ZERO;
    for (j, program) in programs.iter().enumerate() {
        let value = program.eval(tables, point, row, stack);
        acc += if j == 0 { value } else { theta[j - 1] * value };
    }
    acc
}

fn tuple<F: PastaField>(
    programs: &[Program<F>],
    tables: &Tables<'_, F>,
    point: Point<'_, F>,
    row: usize,
    stack: &mut Vec<F>,
) -> Vec<[u64; 4]> {
    programs
        .iter()
        .map(|p| p.eval(tables, point, row, stack).to_canonical_limbs())
        .collect()
}

/// Round one: multiplicity column of `group` (length `n`), counted against the
/// first table row holding each input tuple.
pub(crate) fn multiplicities<F: PastaField>(
    group: &LookupGroup<F>,
    tables: &Tables<'_, F>,
    point: Point<'_, F>,
) -> Result<Vec<F>, Error> {
    let mut stack = Vec::with_capacity(64);
    let mut index: HashMap<Vec<[u64; 4]>, u32> = HashMap::new();
    for row in 0..group.usable {
        let key = tuple(&group.table, tables, point, row, &mut stack);
        index.entry(key).or_insert_with(|| u32::try_from(row).expect("rows fit u32"));
    }
    let mut counts = vec![0u64; tables.n];
    for slot in &group.slots {
        for cell in slot {
            for member in cell.members() {
                let member = &group.members[usize::from(*member)];
                let key = tuple(&member.inputs, tables, point, cell.row as usize, &mut stack);
                let row = index.get(&key).ok_or(Error::LookupInput {
                    lookup: member.lookup,
                    row: cell.row as usize,
                })?;
                counts[*row as usize] += 1;
            }
        }
    }
    Ok(counts.into_iter().map(F::from).collect())
}

/// Montgomery batch inversion of nonzero values.
pub(crate) fn batch_invert<F: PastaField>(values: &mut [F]) {
    let mut prefix = Vec::with_capacity(values.len());
    let mut acc = F::ONE;
    for v in values.iter() {
        prefix.push(acc);
        acc *= v;
    }
    let mut inverse = acc.invert().expect("nonzero denominators");
    for (v, p) in values.iter_mut().zip(prefix).rev() {
        let next = inverse * *v;
        *v = inverse * p;
        inverse = next;
    }
}

/// Round two: the `h_s` columns and the `g` column of `group` for challenges
/// `theta` (powers `theta_1..`) and `r`. A zero denominator `r + A` is a prover
/// failure with negligible probability; the caller resamples by a fresh attempt.
#[allow(clippy::many_single_char_names)]
pub(crate) fn reciprocals<F: PastaField>(
    group: &LookupGroup<F>,
    tables: &Tables<'_, F>,
    point: Point<'_, F>,
    m: &[F],
    theta: &[F],
    r: F,
) -> Result<(Vec<Vec<F>>, Vec<F>), Error> {
    let n = tables.n;
    let mut stack = Vec::with_capacity(64);
    let mut denominators = Vec::new();
    for slot in &group.slots {
        for cell in slot {
            for member in cell.members() {
                let member = &group.members[usize::from(*member)];
                denominators.push(r + compress(&member.inputs, theta, tables, point, cell.row as usize, &mut stack));
            }
        }
    }
    let mut table_rows = Vec::new();
    for (row, multiplicity) in m.iter().enumerate().take(group.usable) {
        if !bool::from(multiplicity.is_zero()) {
            table_rows.push(row);
            denominators.push(r + compress(&group.table, theta, tables, point, row, &mut stack));
        }
    }
    if denominators.iter().any(|d| bool::from(d.is_zero())) {
        return Err(Error::Shape("LogUp denominator is zero"));
    }
    batch_invert(&mut denominators);
    let mut next = denominators.into_iter();
    let mut h = Vec::with_capacity(group.slots.len());
    for slot in &group.slots {
        let mut column = vec![F::ZERO; n];
        for cell in slot {
            let mut sum = F::ZERO;
            for _ in cell.members() {
                sum += next.next().expect("one inverse per input");
            }
            column[cell.row as usize] = sum;
        }
        h.push(column);
    }
    let mut g = vec![F::ZERO; n];
    for row in table_rows {
        g[row] = m[row] * next.next().expect("one inverse per used table row");
    }
    Ok((h, g))
}
