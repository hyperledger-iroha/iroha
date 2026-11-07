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
//! Activity is structural: a lookup is active on the usable rows where the fixed
//! guard (fixed and selector factors) of any of its inputs is nonzero. On every
//! other usable row its tuple is all zero; one such row stays active so that the
//! zero tuple's membership, which the original lookup asserts there, is checked
//! too.

use std::collections::HashMap;

use iroha_pasta::PastaField;
use iroha_plonk::cs::ConstraintSystem;

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
}

/// Lookups sharing one table expression.
#[derive(Clone, Debug)]
pub struct LookupGroup<F> {
    pub(crate) table: Vec<Program<F>>,
    pub(crate) members: Vec<Member<F>>,
    /// Reciprocal cells per slot (slot `s` is witness column `h_s` of the group).
    pub(crate) slots: Vec<Vec<HCell>>,
    /// Usable rows: the table side is constrained on each.
    pub(crate) usable: usize,
    /// Witness column of `m`; `h_s` follow, then `g`.
    pub(crate) first_column: usize,
    /// Constraint index of slot 0; the table constraint follows the last slot.
    pub(crate) first_constraint: usize,
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
    /// Constraint indices of the group: one per slot, then the table constraint.
    pub(crate) fn constraints(&self) -> usize {
        self.slots.len() + 1
    }
}

/// Compiles the lookups of `cs` into groups. `first_column` and `first_constraint`
/// are the first free witness column and constraint index.
pub(crate) fn compile<F: PastaField>(
    cs: &ConstraintSystem<F>,
    tables: &Tables<'_, F>,
    usable: usize,
    first_column: usize,
    first_constraint: usize,
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
    let point = Point {
        advice: &empty,
        instance: &empty,
    };
    let mut stack = Vec::with_capacity(64);
    let mut groups = Vec::with_capacity(grouped.len());
    let mut column = first_column;
    let mut constraint = first_constraint;
    for indices in grouped {
        let lookups = cs.lookups();
        let width = lookups[indices[0]].width();
        let table = lookups[indices[0]]
            .table_expressions()
            .iter()
            .map(Program::compile)
            .collect();
        let mut members = Vec::with_capacity(indices.len());
        // Per member: the usable rows on which it is active.
        let mut active_rows: Vec<Vec<bool>> = Vec::with_capacity(indices.len());
        for &index in &indices {
            let lookup = &lookups[index];
            let mut guards = Vec::new();
            let mut everywhere = false;
            for input in lookup.input_expressions() {
                let (guard, _, trivial) = split_guard(input);
                everywhere |= trivial || guard.is_one();
                guards.push(guard);
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
            members.push(Member {
                lookup: index,
                inputs: lookup
                    .input_expressions()
                    .iter()
                    .map(Program::compile)
                    .collect(),
            });
        }
        let per_cell = if width == 1 { CELL_INPUTS } else { 1 };
        let mut slots: Vec<Vec<HCell>> = Vec::new();
        for row in 0..usable {
            let here: Vec<u16> = active_rows
                .iter()
                .enumerate()
                .filter(|(_, active)| active[row])
                .map(|(m, _)| m)
                .map(|m| u16::try_from(m).expect("members fit u16"))
                .collect();
            for (slot, chunk) in here.chunks(per_cell).enumerate() {
                if slots.len() <= slot {
                    slots.push(Vec::new());
                }
                let mut cell = HCell {
                    row: u32::try_from(row).expect("rows fit u32"),
                    members: [0; CELL_INPUTS],
                    count: u8::try_from(chunk.len()).expect("at most three"),
                };
                cell.members[..chunk.len()].copy_from_slice(chunk);
                slots[slot].push(cell);
            }
        }
        let group = LookupGroup {
            table,
            members,
            slots,
            usable,
            first_column: column,
            first_constraint: constraint,
        };
        column += group.columns();
        constraint += group.constraints();
        groups.push(group);
    }
    (groups, theta_len)
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
        index
            .entry(key)
            .or_insert_with(|| u32::try_from(row).expect("rows fit u32"));
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
                denominators.push(
                    r + compress(
                        &member.inputs,
                        theta,
                        tables,
                        point,
                        cell.row as usize,
                        &mut stack,
                    ),
                );
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
