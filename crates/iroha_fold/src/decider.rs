//! The relaxed-PLONK decider of a folded relation (v1; output `IVC.md` section 7,
//! `DESIGN.md` section 9).
//!
//! An accumulator `(phi, beta, e)` with witness `w` is accepted when `w` opens
//! every commitment, satisfies every copy and `LogUp` sum, and
//!
//! ```text
//! e = sum_r P(r) Phi(r),   Phi(r) = sum_c W_c f_c(r)
//! ```
//!
//! where `P(r)` is the `pow` weight of row `r` (the row bits of `beta`), `W_c`
//! the weight of constraint index `c` (its constraint bits) and `f_c(r)` the class
//! constraint of index `c` on row `r` ([`crate::relation`]). [`Decider`] is a
//! PLONK circuit (`iroha_plonk` frontend) that proves exactly this, so a
//! PIPA-R proof of it decides the accumulator succinctly:
//!
//! - the relation's fixed columns and selectors (as fixed columns) and its
//!   committed witness columns, in [`crate::Witness`] order (advice, then per
//!   `LogUp` group `m`, `h_s`, `g`), with the relation's copy constraints;
//! - `P`: `P(0) = 1`, `P(r + 1) = P(r) R(r)`, where every `R(r)` is copied from
//!   the public ratio of `r`'s trailing-ones count;
//! - `S`: `S(0) = 0`, `S(r + 1) = S(r) + P(r) Phi(r)`, `e = S(u-1) + P(u-1) Phi(u-1)`
//!   over the `u` usable rows: a relation whose constraints all lie on usable rows
//!   ([`DeciderPlan::new`] checks it) has `Phi = 0` past them;
//! - `T_g` per `LogUp` group: the running sum of `sum_s a_s h_s - g`, zero at
//!   both ends, where the fixed `a_s` marks slot `s`'s cells (the only rows
//!   where `h_s` is constrained, so no free `h` value enters the sum);
//! - each public scalar (the weights, the folded `theta` and `r`, `e`) as a
//!   constant advice column: equal on consecutive rows and copied from its
//!   instance cell once.
//!
//! The public inputs are the folded public columns of the accumulator followed by
//! one column of scalars ([`DeciderPlan::scalars`]). The decider proves nothing
//! about the commitments: a wrap must check that the decider proof's commitments
//! to the witness columns are the accumulator's (a committed-advice proof keeps
//! them identical), and recompute the scalars from the accumulator.

use std::sync::Arc;

use ff::Field;
use iroha_pasta::{PastaCurve, PastaField};
use iroha_plonk::{
    cs::{Advice, Column, ConstraintSystem, Expression, Fixed, Instance, Rotation, VirtualCells},
    frontend::{Cell as PlonkCell, Circuit, Error, Layouter, SimpleFloorPlanner, Value},
};

use crate::{
    commit::Witness,
    fold::{Accumulator, View, constraints_at, pow_vector},
    relation::{Cell, FoldedRelation},
};

/// The fixed layout of a decider: its relation and the public scalar column.
#[derive(Clone, Debug)]
pub struct DeciderPlan<F> {
    relation: FoldedRelation<F>,
}

/// Index ranges of the scalar instance column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalarLayout {
    /// Weights of the constraint indices (classes, then table constraints).
    pub weights: usize,
    /// Folded `theta` powers.
    pub theta: usize,
    /// Folded `LogUp` challenges.
    pub r: usize,
    /// Row-weight ratios, one per row bit.
    pub ratios: usize,
}

impl ScalarLayout {
    /// Position of the first `theta` power.
    #[must_use]
    pub const fn theta_start(&self) -> usize {
        self.weights
    }
    /// Position of the first `r`.
    #[must_use]
    pub const fn r_start(&self) -> usize {
        self.weights + self.theta
    }
    /// Position of the first ratio.
    #[must_use]
    pub const fn ratio_start(&self) -> usize {
        self.weights + self.theta + self.r
    }
    /// Position of `e`.
    #[must_use]
    pub const fn e(&self) -> usize {
        self.weights + self.theta + self.r + self.ratios
    }
    /// Length of the scalar column.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.e() + 1
    }
    /// Whether the column is empty (never: it always holds `e`).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        false
    }
    /// The scalars that are broadcast as constant advice columns (all but the ratios).
    fn broadcast(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.ratio_start()).chain(core::iter::once(self.e()))
    }
}

impl<F: PastaField> DeciderPlan<F> {
    /// The decider plan of `relation`.
    ///
    /// # Errors
    /// [`crate::Error::Shape`] when a gate or `LogUp` constraint is active past the
    /// usable rows, or a fixed or selector value is nonzero there (the decider
    /// sums the usable rows only).
    pub fn new(relation: FoldedRelation<F>) -> Result<Self, crate::Error> {
        let usable = relation.usable;
        let past = |row: u32| row as usize >= usable;
        if usable < 2 {
            return Err(crate::Error::Shape("decider: fewer than two usable rows"));
        }
        if relation.gates.iter().any(|g| g.rows.iter().any(|r| past(*r))) {
            return Err(crate::Error::Shape("decider: a gate constrains rows past the usable ones"));
        }
        if relation.groups.iter().any(|g| g.slots.iter().flatten().any(|c| past(c.row))) {
            return Err(crate::Error::Shape("decider: a LogUp cell lies past the usable rows"));
        }
        if relation.fixed.iter().any(|c| c[usable..].iter().any(|v| !bool::from(v.is_zero())))
            || relation.selectors.iter().any(|c| c[usable..].iter().any(|v| *v))
        {
            return Err(crate::Error::Shape("decider: fixed values past the usable rows"));
        }
        Ok(Self { relation })
    }

    /// The decided relation.
    #[must_use]
    pub const fn relation(&self) -> &FoldedRelation<F> {
        &self.relation
    }

    /// The layout of the scalar instance column.
    #[must_use]
    pub fn scalars(&self) -> ScalarLayout {
        let r = &self.relation;
        ScalarLayout {
            weights: r.classes + r.groups.len(),
            theta: r.theta_len,
            r: r.groups.len(),
            ratios: r.n.trailing_zeros() as usize,
        }
    }
}

/// The decider's instance columns for `acc`: its folded public columns, then the
/// scalar column (weights, `theta`, `r`, ratios, `e`).
#[must_use]
pub fn decider_instances<C: PastaCurve>(
    plan: &DeciderPlan<C::ScalarExt>,
    acc: &Accumulator<C>,
) -> Vec<Vec<C::ScalarExt>> {
    let relation = &plan.relation;
    let layout = plan.scalars();
    let pb = relation.constraint_bits;
    let pow_lo = pow_vector(&acc.beta[..pb]);
    let mut scalars = Vec::with_capacity(layout.len());
    scalars.extend_from_slice(&pow_lo[..layout.weights]);
    scalars.extend_from_slice(&acc.instance.theta);
    scalars.extend_from_slice(&acc.instance.r);
    let rows = &acc.beta[pb..];
    for j in 0..layout.ratios {
        // r -> r + 1 with j trailing ones: bit j is set, bits 0..j are cleared.
        let mut ratio = rows[j];
        for b in &rows[..j] {
            ratio *= b.invert().expect("pow challenges are nonzero");
        }
        scalars.push(ratio);
    }
    scalars.push(acc.e);
    let mut out = acc.instance.public.clone();
    out.push(scalars);
    out
}

/// The decider's private columns, computed from an accumulator's witness.
#[derive(Clone, Debug)]
pub struct DeciderWitness<F> {
    columns: Vec<Vec<F>>,
    p: Vec<F>,
    s: Vec<F>,
    t: Vec<Vec<F>>,
}

/// The decider witness of `acc` with witness `witness`.
///
/// # Panics
/// For a witness of another shape (the caller decides shape first).
#[must_use]
pub fn decider_witness<C: PastaCurve>(
    plan: &DeciderPlan<C::ScalarExt>,
    acc: &Accumulator<C>,
    witness: &Witness<C::ScalarExt>,
) -> DeciderWitness<C::ScalarExt> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let relation = &plan.relation;
    let n = relation.n;
    let usable = relation.usable;
    let pb = relation.constraint_bits;
    let pow_lo = pow_vector(&acc.beta[..pb]);
    let mut p = pow_vector(&acc.beta[pb..]);
    p.truncate(usable);
    let mut phi = vec![S::<C>::ZERO; n];
    let view = View::of(witness, &acc.instance.public, &acc.instance.theta, &acc.instance.r);
    constraints_at(relation, view, |row, index, value| phi[row] += pow_lo[index] * value);
    let mut s = Vec::with_capacity(usable);
    let mut sum = S::<C>::ZERO;
    for row in 0..usable {
        s.push(sum);
        sum += p[row] * phi[row];
    }
    let t = relation
        .groups
        .iter()
        .map(|group| {
            // Reciprocal cells count on their slot's cells only, as natively.
            let mut reciprocals = vec![S::<C>::ZERO; usable];
            for (slot, cells) in group.slots.iter().enumerate() {
                let column = &witness.columns[group.h_column(slot)];
                for cell in cells {
                    reciprocals[cell.row as usize] += column[cell.row as usize];
                }
            }
            let table = &witness.columns[group.g_column()];
            let mut running = Vec::with_capacity(usable);
            let mut sum = S::<C>::ZERO;
            for row in 0..usable {
                running.push(sum);
                sum += reciprocals[row] - table[row];
            }
            running
        })
        .collect();
    DeciderWitness { columns: witness.columns.clone(), p, s, t }
}

/// The columns of a decider.
#[derive(Clone, Debug)]
pub struct DeciderConfig {
    instances: Vec<Column<Instance>>,
    scalars: Column<Instance>,
    fixed: Vec<Column<Fixed>>,
    selectors: Vec<Column<Fixed>>,
    witness: Vec<Column<Advice>>,
    broadcast: Vec<(usize, Column<Advice>)>,
    ratio: Column<Advice>,
    p: Column<Advice>,
    s: Column<Advice>,
    t: Vec<Column<Advice>>,
    q_main: Column<Fixed>,
    q_first: Column<Fixed>,
    q_last: Column<Fixed>,
    activity: Vec<Vec<Column<Fixed>>>,
}

/// A decider circuit for one accumulator (or a key-generation layout).
#[derive(Clone, Debug)]
pub struct Decider<F> {
    plan: Arc<DeciderPlan<F>>,
    witness: Option<Arc<DeciderWitness<F>>>,
}

impl<F: PastaField> Decider<F> {
    /// A key-generation layout.
    #[must_use]
    pub const fn layout(plan: Arc<DeciderPlan<F>>) -> Self {
        Self { plan, witness: None }
    }

    /// A witness layout.
    #[must_use]
    pub fn new(plan: Arc<DeciderPlan<F>>, witness: DeciderWitness<F>) -> Self {
        Self { plan, witness: Some(Arc::new(witness)) }
    }
}

/// Rewrites a relation expression onto the decider's columns.
struct Remap<'a> {
    fixed: &'a [Column<Fixed>],
    selectors: &'a [Column<Fixed>],
    witness: &'a [Column<Advice>],
    instances: &'a [Column<Instance>],
}

impl Remap<'_> {
    fn expression<F: PastaField>(&self, cells: &mut VirtualCells<'_, F>, expression: &Expression<F>) -> Expression<F> {
        match expression {
            Expression::Constant(value) => Expression::Constant(*value),
            Expression::Selector(selector) => cells.query_fixed(self.selectors[selector.index()], Rotation::cur()),
            Expression::Fixed(query) => cells.query_fixed(self.fixed[query.column_index], query.rotation),
            Expression::Advice(query) => cells.query_advice(self.witness[query.column_index], query.rotation),
            Expression::Instance(query) => cells.query_instance(self.instances[query.column_index], query.rotation),
            Expression::Negated(inner) => -self.expression(cells, inner),
            Expression::Sum(left, right) => self.expression(cells, left) + self.expression(cells, right),
            Expression::Product(left, right) => self.expression(cells, left) * self.expression(cells, right),
            Expression::Scaled(inner, factor) => self.expression(cells, inner) * *factor,
        }
    }
}

fn product<F: PastaField>(factors: impl IntoIterator<Item = Expression<F>>) -> Expression<F> {
    factors
        .into_iter()
        .reduce(|acc, factor| acc * factor)
        .unwrap_or(Expression::Constant(F::ONE))
}

impl<F: PastaField> Circuit<F> for Decider<F> {
    type Config = DeciderConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = Option<Arc<DeciderPlan<F>>>;

    fn params(&self) -> Self::Params {
        Some(Arc::clone(&self.plan))
    }

    fn without_witnesses(&self) -> Self {
        Self::layout(Arc::clone(&self.plan))
    }

    fn configure(_meta: &mut ConstraintSystem<F>) -> DeciderConfig {
        unreachable!("a decider is configured with its plan")
    }

    #[allow(clippy::too_many_lines)]
    fn configure_with_params(meta: &mut ConstraintSystem<F>, plan: Self::Params) -> DeciderConfig {
        let plan = plan.expect("a decider is configured with its plan");
        let relation = &plan.relation;
        let layout = plan.scalars();
        let instances: Vec<Column<Instance>> =
            relation.instance_lengths.iter().map(|len| meta.instance_column(*len)).collect();
        let scalars = meta.instance_column(layout.len());
        meta.enable_equality(scalars);
        let fixed: Vec<Column<Fixed>> = relation.fixed.iter().map(|_| meta.fixed_column()).collect();
        let selectors: Vec<Column<Fixed>> = relation.selectors.iter().map(|_| meta.fixed_column()).collect();
        let witness: Vec<Column<Advice>> = (0..relation.columns()).map(|_| meta.advice_column()).collect();
        // Every column a relation copy touches takes part in the permutation.
        for (a, b) in &relation.copies {
            for cell in [a, b] {
                match *cell {
                    Cell::Advice(c, _) => meta.enable_equality(witness[c as usize]),
                    Cell::Fixed(c, _) => meta.enable_equality(fixed[c as usize]),
                    Cell::Instance(c, _) => meta.enable_equality(instances[c as usize]),
                }
            }
        }
        let broadcast: Vec<(usize, Column<Advice>)> = layout
            .broadcast()
            .map(|index| {
                let column = meta.advice_column();
                meta.enable_equality(column);
                (index, column)
            })
            .collect();
        let ratio = meta.advice_column();
        meta.enable_equality(ratio);
        let p = meta.advice_column();
        let s = meta.advice_column();
        let t: Vec<Column<Advice>> = relation.groups.iter().map(|_| meta.advice_column()).collect();
        let q_main = meta.fixed_column();
        let q_first = meta.fixed_column();
        let q_last = meta.fixed_column();
        let activity: Vec<Vec<Column<Fixed>>> =
            relation.groups.iter().map(|g| g.slots.iter().map(|_| meta.fixed_column()).collect()).collect();
        let config = DeciderConfig {
            instances,
            scalars,
            fixed,
            selectors,
            witness,
            broadcast,
            ratio,
            p,
            s,
            t,
            q_main,
            q_first,
            q_last,
            activity,
        };
        let c = config.clone();
        let plan = Arc::clone(&plan);
        meta.create_gate("relaxed decider", move |cells| {
            let relation = &plan.relation;
            let layout = plan.scalars();
            let remap = Remap { fixed: &c.fixed, selectors: &c.selectors, witness: &c.witness, instances: &c.instances };
            let scalar = |cells: &mut VirtualCells<'_, F>, index: usize| {
                let column = c.broadcast.iter().find(|(i, _)| *i == index).expect("broadcast scalar").1;
                cells.query_advice(column, Rotation::cur())
            };
            let cur = Rotation::cur();
            let next = Rotation::next();
            // Phi = sum over constraint indices of weight * class constraint.
            let mut phi = Expression::Constant(F::ZERO);
            for gate in &relation.gates {
                let weight = scalar(cells, gate.class);
                phi = phi + weight * remap.expression(cells, &gate.expression);
            }
            let theta: Vec<Expression<F>> =
                (0..layout.theta).map(|j| scalar(cells, layout.theta_start() + j)).collect();
            let compress = |cells: &mut VirtualCells<'_, F>, expressions: &[Expression<F>]| {
                let mut out = Expression::Constant(F::ZERO);
                for (j, e) in expressions.iter().enumerate() {
                    let term = remap.expression(cells, e);
                    out = out + if j == 0 { term } else { theta[j - 1].clone() * term };
                }
                out
            };
            let mut logup_sums = Vec::with_capacity(relation.groups.len());
            for (g, group) in relation.groups.iter().enumerate() {
                let r = scalar(cells, layout.r_start() + g);
                let mut h_sum = Expression::Constant(F::ZERO);
                for (slot, chosen) in group.slot_members.iter().enumerate() {
                    let h = cells.query_advice(c.witness[group.h_column(slot)], cur);
                    // h_s is constrained, hence counted, on its slot's cells only.
                    let active = cells.query_fixed(c.activity[g][slot], cur);
                    h_sum = h_sum + active.clone() * h.clone();
                    let terms: Vec<Expression<F>> = chosen
                        .iter()
                        .map(|m| r.clone() + compress(cells, &group.members[usize::from(*m)].expressions))
                        .collect();
                    let mut cell = h * product(terms.iter().cloned());
                    for k in 0..terms.len() {
                        cell = cell
                            - product(terms.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, t)| t.clone()));
                    }
                    let weight = scalar(cells, group.slot_constraints[slot]);
                    phi = phi + weight * active * cell;
                }
                let m = cells.query_advice(c.witness[group.m_column()], cur);
                let gg = cells.query_advice(c.witness[group.g_column()], cur);
                let table = compress(cells, &group.table_expressions);
                let weight = scalar(cells, group.table_constraint);
                // On rows past the usable ones m = g = 0, so the constraint is zero there.
                phi = phi + weight * (gg.clone() * (r + table) - m);
                logup_sums.push(h_sum - gg);
            }
            let q_main = cells.query_fixed(c.q_main, cur);
            let q_first = cells.query_fixed(c.q_first, cur);
            let q_last = cells.query_fixed(c.q_last, cur);
            let p_cur = cells.query_advice(c.p, cur);
            let p_next = cells.query_advice(c.p, next);
            let s_cur = cells.query_advice(c.s, cur);
            let s_next = cells.query_advice(c.s, next);
            let ratio = cells.query_advice(c.ratio, cur);
            let e = scalar(cells, layout.e());
            let weighted = p_cur.clone() * phi;
            let mut constraints = vec![
                q_main.clone() * (s_next - s_cur.clone() - weighted.clone()),
                q_last.clone() * (e - s_cur.clone() - weighted),
                q_first.clone() * s_cur,
                q_first.clone() * (p_cur.clone() - Expression::Constant(F::ONE)),
                q_main.clone() * (p_next - p_cur * ratio),
            ];
            for (_, column) in &c.broadcast {
                let now = cells.query_advice(*column, cur);
                let after = cells.query_advice(*column, next);
                constraints.push(q_main.clone() * (after - now));
            }
            for (g, sum) in logup_sums.into_iter().enumerate() {
                let t_cur = cells.query_advice(c.t[g], cur);
                let t_next = cells.query_advice(c.t[g], next);
                constraints.push(q_main.clone() * (t_next - t_cur.clone() - sum.clone()));
                constraints.push(q_first.clone() * t_cur.clone());
                constraints.push(q_last.clone() * (t_cur + sum));
            }
            constraints
        });
        config
    }

    #[allow(clippy::too_many_lines)]
    fn synthesize(&self, config: DeciderConfig, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        let relation = &self.plan.relation;
        let layout = self.plan.scalars();
        let n = relation.usable;
        let witness = self.witness.as_deref();
        let value = |f: &dyn Fn(&DeciderWitness<F>) -> F| witness.map_or_else(Value::unknown, |w| Value::known(f(w)));
        // Cells a relation copy needs, keyed by (kind, column, row).
        let mut needed = std::collections::BTreeSet::new();
        for (a, b) in &relation.copies {
            for cell in [a, b] {
                needed.insert(*cell);
            }
        }
        let instance_copies = layouter.assign_region(
            || "decider",
            |mut region| {
                let mut handles: std::collections::BTreeMap<Cell, PlonkCell> = std::collections::BTreeMap::new();
                for (c, column) in config.fixed.iter().enumerate() {
                    for row in 0..n {
                        let cell = region.assign_fixed(*column, row, relation.fixed[c][row])?;
                        let key = Cell::Fixed(u32::try_from(c).expect("fits"), u32::try_from(row).expect("fits"));
                        if needed.contains(&key) {
                            handles.insert(key, cell);
                        }
                    }
                }
                for (s, column) in config.selectors.iter().enumerate() {
                    for row in 0..n {
                        let bit = if relation.selectors[s][row] { F::ONE } else { F::ZERO };
                        region.assign_fixed(*column, row, bit)?;
                    }
                }
                for row in 0..n {
                    let main = if row + 1 < n { F::ONE } else { F::ZERO };
                    region.assign_fixed(config.q_main, row, main)?;
                    region.assign_fixed(config.q_first, row, if row == 0 { F::ONE } else { F::ZERO })?;
                    region.assign_fixed(config.q_last, row, if row + 1 == n { F::ONE } else { F::ZERO })?;
                }
                for (g, group) in relation.groups.iter().enumerate() {
                    for (slot, cells) in group.slots.iter().enumerate() {
                        let mut active = vec![F::ZERO; n];
                        for cell in cells {
                            active[cell.row as usize] = F::ONE;
                        }
                        for (row, a) in active.into_iter().enumerate() {
                            region.assign_fixed(config.activity[g][slot], row, a)?;
                        }
                    }
                }
                for (c, column) in config.witness.iter().enumerate() {
                    for row in 0..n {
                        let cell = region.assign_advice(*column, row, value(&|w| w.columns[c][row]))?;
                        let key = Cell::Advice(u32::try_from(c).expect("fits"), u32::try_from(row).expect("fits"));
                        if needed.contains(&key) {
                            handles.insert(key, cell.cell());
                        }
                    }
                }
                for row in 0..n {
                    region.assign_advice(config.p, row, value(&|w| w.p[row]))?;
                    region.assign_advice(config.s, row, value(&|w| w.s[row]))?;
                    for (g, column) in config.t.iter().enumerate() {
                        region.assign_advice(*column, row, value(&|w| w.t[g][row]))?;
                    }
                }
                // Broadcast scalars: constant columns, row 0 copied from the instance.
                for (index, column) in &config.broadcast {
                    let first = region.assign_advice_from_instance(|| "scalar", config.scalars, *index, *column, 0)?;
                    let v = first.value().copied();
                    for row in 1..n {
                        region.assign_advice(*column, row, v)?;
                    }
                }
                // Ratios: every row but the last copies the ratio of its trailing ones.
                for row in 0..n {
                    let ones = row.trailing_ones() as usize;
                    if row + 1 < n {
                        region.assign_advice_from_instance(
                            || "ratio",
                            config.scalars,
                            layout.ratio_start() + ones,
                            config.ratio,
                            row,
                        )?;
                    } else {
                        region.assign_advice(config.ratio, row, Value::known(F::ZERO))?;
                    }
                }
                // The relation's copies.
                let mut instance_copies = Vec::new();
                for (a, b) in &relation.copies {
                    match (a, b) {
                        (Cell::Instance(c, r), other) | (other, Cell::Instance(c, r)) => {
                            let handle = *handles.get(other).ok_or(Error::Synthesis)?;
                            instance_copies.push((handle, *c as usize, *r as usize));
                        }
                        _ => {
                            let left = *handles.get(a).ok_or(Error::Synthesis)?;
                            let right = *handles.get(b).ok_or(Error::Synthesis)?;
                            region.constrain_equal(left, right)?;
                        }
                    }
                }
                Ok(instance_copies)
            },
        )?;
        for (cell, column, row) in instance_copies {
            layouter.constrain_instance(cell, config.instances[column], row)?;
        }
        Ok(())
    }
}
