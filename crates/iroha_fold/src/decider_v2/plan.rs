//! The fixed layout of a v2 decider: the tables its zero-check reads, its
//! weighted terms, its key polynomials and its copy permutation.
//!
//! Key polynomials are shared by equal contents. A key polynomial with at most
//! the plan's step bound of constant runs ([`STEP_RUNS`] unless chosen) is a
//! step function the verifier evaluates ([`Steps`]); every other one is
//! committed and opened. One copy helper covers [`COPY_GROUP`] copy columns.

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use ff::Field;
use iroha_pasta::{PastaCurve, PastaField};
use iroha_plonk::cs::Expression;

use super::{
    eval::{MAX_DEGREE, Poly},
    mle,
};
use crate::{
    Error,
    commit::CommitKey,
    program::folded_degree,
    relation::{Cell, FoldedRelation},
};

/// The step bound of [`DeciderV2Plan::new`]: a key polynomial with at most this
/// many maximal constant runs (counted cyclically) is evaluated by the verifier
/// as a step function instead of being committed and opened. Each change of
/// value costs one prefix indicator (`O(mu)` field operations, shared between
/// boundaries) against an opened commitment and its claimed values; a verifier
/// with field-side room for more takes a larger bound
/// ([`DeciderV2Plan::with_step_runs`]).
pub const STEP_RUNS: usize = 16;

/// Copy columns per copy helper. A helper sums the copy terms of its columns,
/// and its row constraint has degree `2 COPY_GROUP + 2` (six).
pub(crate) const COPY_GROUP: usize = 2;

/// A table of the zero-check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Source {
    /// An accumulator witness column at a rotation.
    Witness(usize, i32),
    /// A key polynomial at a rotation.
    Key(usize, i32),
    /// A key polynomial of at most [`STEP_RUNS`] runs at a rotation: the
    /// verifier evaluates it ([`DeciderV2Plan::steps`]).
    Steps(usize, i32),
    /// A folded public column at a rotation (the verifier evaluates it).
    Instance(usize, i32),
    /// A copy helper column, committed by the decider prover.
    Helper(usize),
    /// The `pow` weight of each row.
    Pow,
    /// `eq(tau, .)`: the weights of the decider's own row constraints.
    EqTau,
    /// The indicator of the usable rows.
    Usable,
    /// The row index.
    Index,
}

/// A polynomial the decider's claims open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Committed {
    /// An accumulator witness column.
    Witness(usize),
    /// A key polynomial.
    Key(usize),
    /// A copy helper column.
    Helper(usize),
}

impl Source {
    /// The committed polynomial and rotation this table reads, if any.
    pub(crate) const fn committed(self) -> Option<(Committed, i32)> {
        match self {
            Self::Witness(c, k) => Some((Committed::Witness(c), k)),
            Self::Key(p, k) => Some((Committed::Key(p), k)),
            Self::Helper(j) => Some((Committed::Helper(j), 0)),
            _ => None,
        }
    }
}

/// The coefficient of a term.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Weight {
    /// The `pow` weight of a constraint index (its constraint bits).
    Index(usize),
    /// `alpha^k`.
    Alpha(usize),
    /// `alpha^k kappa^j`.
    AlphaKappa(usize, usize),
}

/// The rows off which a term vanishes on the hypercube, so its pairs need no
/// evaluation (a prover optimization; the verifier ignores it).
#[derive(Clone, Debug)]
pub(crate) enum Support {
    /// The term vanishes on every row outside this ascending list.
    Rows(Vec<u32>),
    /// The term vanishes on every row at or past this count.
    Prefix(usize),
}

/// One weighted term of the zero-check sum.
#[derive(Clone, Debug)]
pub(crate) struct Term<F> {
    pub(crate) weight: Weight,
    pub(crate) poly: Poly<F>,
    pub(crate) support: Support,
}

/// A step function over the rows: `first` on row 0, changing by `delta` at each
/// listed row (ascending). Its multilinear extension at `x` is
/// `first + sum delta (1 - prefix_indicator(row, x))`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Steps<F> {
    pub(crate) first: F,
    pub(crate) changes: Vec<(usize, F)>,
}

impl<F: Field> Steps<F> {
    /// The step function of a column.
    pub(crate) fn of(column: &[F]) -> Self {
        let first = column.first().copied().unwrap_or(F::ZERO);
        let changes = column
            .windows(2)
            .enumerate()
            .filter(|(_, w)| w[1] != w[0])
            .map(|(i, w)| (i + 1, w[1] - w[0]))
            .collect();
        Self { first, changes }
    }

    /// Its multilinear extension at `x`.
    pub(crate) fn eval(&self, x: &[F]) -> F {
        self.changes.iter().fold(self.first, |total, (row, delta)| {
            total + *delta * (F::ONE - mle::prefix_indicator(*row, x))
        })
    }
}

/// The number of maximal constant runs of a column, counted cyclically (a
/// constant column has one).
fn cyclic_runs<F: Field>(column: &[F]) -> usize {
    let n = column.len();
    (0..n)
        .filter(|&i| column[i] != column[(i + n - 1) % n])
        .count()
        .max(1)
}

/// A column with copy cells.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CopyColumn {
    Witness(usize),
    Fixed(usize),
}

/// The copy permutation of a relation.
#[derive(Clone, Debug)]
pub(crate) struct CopyPlan {
    /// Columns with copy cells; column `j` holds the ids `j n .. (j + 1) n`.
    pub(crate) columns: Vec<CopyColumn>,
    /// The key polynomial of each column's `sigma`.
    pub(crate) sigma: Vec<usize>,
    /// The rows of each column that `sigma` moves.
    pub(crate) rows: Vec<Vec<u32>>,
    /// The columns of each copy helper ([`COPY_GROUP`] each, the last one
    /// possibly fewer).
    pub(crate) helpers: Vec<Vec<usize>>,
    /// Instance cells in copies: column, row, id and `sigma(id)`.
    pub(crate) instance_cells: Vec<(usize, usize, u64, u64)>,
}

/// One committed table of the zero-check: its source, polynomial and rotation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Read {
    pub(crate) source: usize,
    pub(crate) poly: usize,
    pub(crate) rotation: i32,
}

/// The fixed layout of the v2 decider of one folded relation.
#[derive(Clone, Debug)]
pub struct DeciderV2Plan<C: PastaCurve> {
    pub(crate) relation: FoldedRelation<C::ScalarExt>,
    pub(crate) key: CommitKey<C>,
    pub(crate) mu: usize,
    /// The step bound: key polynomials of at most this many runs are step
    /// tables.
    pub(crate) step_runs: usize,
    pub(crate) sources: Vec<Source>,
    /// The step function of each [`Source::Steps`] table (`None` for every
    /// other source), in source order.
    pub(crate) steps: Vec<Option<Steps<C::ScalarExt>>>,
    pub(crate) key_polys: Vec<Vec<C::ScalarExt>>,
    pub(crate) key_commitments: Vec<C::AffineExt>,
    pub(crate) terms: Vec<Term<C::ScalarExt>>,
    pub(crate) copies: CopyPlan,
    /// Instance cells read as scalars: column and row.
    pub(crate) cells: Vec<(usize, usize)>,
    pub(crate) degree: usize,
    /// The polynomials the claims open, in claim order.
    pub(crate) committed: Vec<Committed>,
    /// The committed tables, in `at_rho` order.
    pub(crate) reads: Vec<Read>,
}

/// Scalar layout: `theta` powers, one `r` per group, the copy challenges
/// `gamma` and `eta`, then the instance cells.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scalars {
    pub(crate) theta: usize,
    pub(crate) groups: usize,
}

impl Scalars {
    pub(crate) const fn r(&self, group: usize) -> usize {
        self.theta + group
    }
    pub(crate) const fn gamma(&self) -> usize {
        self.theta + self.groups
    }
    pub(crate) const fn eta(&self) -> usize {
        self.theta + self.groups + 1
    }
    pub(crate) const fn cell(&self, index: usize) -> usize {
        self.theta + self.groups + 2 + index
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum KeyName {
    Fixed(usize),
    Selector(usize),
}

struct Builder<'a, F> {
    relation: &'a FoldedRelation<F>,
    sources: Vec<Source>,
    source_index: HashMap<Source, usize>,
    key_polys: Vec<Vec<F>>,
    /// Whether each key polynomial is a step function the verifier evaluates.
    stepped: Vec<bool>,
    /// Key polynomials by a hash of their contents.
    contents: HashMap<u64, Vec<usize>>,
    named: HashMap<KeyName, usize>,
    sparse: HashMap<Vec<(u32, [u64; 4])>, usize>,
    step_runs: usize,
    cells: Vec<(usize, usize)>,
    cell_index: HashMap<(usize, usize), usize>,
    scalars: Scalars,
}

fn top_factors<F: Clone>(expression: &Expression<F>, out: &mut Vec<Expression<F>>) {
    match expression {
        Expression::Product(left, right) => {
            top_factors(left, out);
            top_factors(right, out);
        }
        other => out.push(other.clone()),
    }
}

fn product<F: PastaField>(polys: impl IntoIterator<Item = Poly<F>>) -> Poly<F> {
    polys
        .into_iter()
        .reduce(Poly::mul)
        .unwrap_or_else(|| Poly::constant(F::ONE))
}

impl<F: PastaField> Builder<'_, F> {
    fn table(&mut self, source: Source) -> usize {
        if let Some(index) = self.source_index.get(&source) {
            return *index;
        }
        let index = self.sources.len();
        self.sources.push(source);
        self.source_index.insert(source, index);
        index
    }

    /// The key polynomial `values`, shared by equal contents.
    fn add_key(&mut self, values: Vec<F>) -> usize {
        let mut hasher = DefaultHasher::new();
        for value in &values {
            value.to_canonical_limbs().hash(&mut hasher);
        }
        let candidates = self.contents.entry(hasher.finish()).or_default();
        if let Some(index) = candidates
            .iter()
            .find(|index| self.key_polys[**index] == values)
        {
            return *index;
        }
        let index = self.key_polys.len();
        candidates.push(index);
        self.stepped.push(cyclic_runs(&values) <= self.step_runs);
        self.key_polys.push(values);
        index
    }

    fn named_key(&mut self, name: KeyName) -> usize {
        if let Some(index) = self.named.get(&name) {
            return *index;
        }
        let relation = self.relation;
        let values = match name {
            KeyName::Fixed(c) => relation.fixed[c].clone(),
            KeyName::Selector(s) => relation.selectors[s]
                .iter()
                .map(|on| if *on { F::ONE } else { F::ZERO })
                .collect(),
        };
        let index = self.add_key(values);
        self.named.insert(name, index);
        index
    }

    /// A key polynomial with the given nonzero rows, shared by equal contents.
    fn sparse_key(&mut self, rows: &[u32], values: &[F]) -> usize {
        let content: Vec<(u32, [u64; 4])> = rows
            .iter()
            .zip(values)
            .filter(|(_, v)| !bool::from(v.is_zero()))
            .map(|(r, v)| (*r, v.to_canonical_limbs()))
            .collect();
        if let Some(index) = self.sparse.get(&content) {
            return *index;
        }
        let mut column = vec![F::ZERO; self.relation.n];
        for (row, value) in rows.iter().zip(values) {
            column[*row as usize] = *value;
        }
        let index = self.add_key(column);
        self.sparse.insert(content, index);
        index
    }

    /// The zero-check table of key polynomial `key` at `rotation`: a step
    /// function the verifier evaluates, or a committed table.
    fn key_source(&mut self, key: usize, rotation: i32) -> usize {
        self.table(if self.stepped[key] {
            Source::Steps(key, rotation)
        } else {
            Source::Key(key, rotation)
        })
    }

    fn key_table(&mut self, name: KeyName, rotation: i32) -> usize {
        let key = self.named_key(name);
        self.key_source(key, rotation)
    }

    fn cell(&mut self, column: usize, row: usize) -> usize {
        if let Some(index) = self.cell_index.get(&(column, row)) {
            return self.scalars.cell(*index);
        }
        let index = self.cells.len();
        self.cells.push((column, row));
        self.cell_index.insert((column, row), index);
        self.scalars.cell(index)
    }

    fn compile(&mut self, expression: &Expression<F>) -> Poly<F> {
        match expression {
            Expression::Constant(value) => Poly::constant(*value),
            Expression::Selector(selector) => {
                Poly::table(self.key_table(KeyName::Selector(selector.index()), 0))
            }
            Expression::Fixed(query) => {
                Poly::table(self.key_table(KeyName::Fixed(query.column_index), query.rotation.0))
            }
            Expression::Advice(query) => {
                Poly::table(self.table(Source::Witness(query.column_index, query.rotation.0)))
            }
            Expression::Instance(query) => {
                Poly::table(self.table(Source::Instance(query.column_index, query.rotation.0)))
            }
            Expression::InstanceCell(query) => {
                Poly::scalar(self.cell(query.column_index, query.row))
            }
            Expression::Negated(inner) => self.compile(inner).neg(),
            Expression::Sum(left, right) => {
                let left = self.compile(left);
                left.add(self.compile(right))
            }
            Expression::Product(left, right) => {
                let left = self.compile(left);
                left.mul(self.compile(right))
            }
            Expression::Scaled(inner, factor) => self.compile(inner).scale(*factor),
        }
    }

    /// `a_0 + theta_1 a_1 + ...` of `expressions`.
    fn compress(&mut self, expressions: &[Expression<F>]) -> Poly<F> {
        let mut out = Poly::constant(F::ZERO);
        for (j, expression) in expressions.iter().enumerate() {
            let term = self.compile(expression);
            out = out.add(if j == 0 {
                term
            } else {
                Poly::scalar(j - 1).mul(term)
            });
        }
        out
    }
}

/// Union-find root with path halving.
fn root(parent: &mut HashMap<u64, u64>, mut id: u64) -> u64 {
    loop {
        let p = *parent.get(&id).unwrap_or(&id);
        if p == id {
            return id;
        }
        let grand = *parent.get(&p).unwrap_or(&p);
        parent.insert(id, grand);
        id = grand;
    }
}

impl<C: PastaCurve> DeciderV2Plan<C> {
    /// The v2 decider plan of `relation`, whose accumulators commit under `key`
    /// (the monomial generators: a claim opens as an inner product with an
    /// `eq` tensor), with the step bound [`STEP_RUNS`]. Commits the key
    /// polynomials.
    ///
    /// # Errors
    /// As [`Self::with_step_runs`].
    pub fn new(relation: FoldedRelation<C::ScalarExt>, key: &CommitKey<C>) -> Result<Self, Error> {
        Self::with_step_runs(relation, key, STEP_RUNS)
    }

    /// [`Self::new`] with key polynomials of at most `step_runs` constant runs
    /// evaluated by the verifier. The decider's prover and verifier must use
    /// the same bound.
    ///
    /// # Errors
    /// [`Error::Shape`] for a basis shorter than the domain, [`Error::Degree`]
    /// for a term of degree above 15, [`Error::Msm`].
    #[allow(clippy::too_many_lines)]
    pub fn with_step_runs(
        relation: FoldedRelation<C::ScalarExt>,
        key: &CommitKey<C>,
        step_runs: usize,
    ) -> Result<Self, Error> {
        type S<C> = <C as PastaCurve>::ScalarExt;
        let n = relation.n;
        if key.len() < n {
            return Err(Error::Shape("commitment basis shorter than the domain"));
        }
        let mu = n.trailing_zeros() as usize;
        let scalars = Scalars {
            theta: relation.theta_len,
            groups: relation.groups.len(),
        };
        let mut b = Builder {
            relation: &relation,
            sources: Vec::new(),
            source_index: HashMap::new(),
            key_polys: Vec::new(),
            stepped: Vec::new(),
            contents: HashMap::new(),
            named: HashMap::new(),
            sparse: HashMap::new(),
            step_runs,
            cells: Vec::new(),
            cell_index: HashMap::new(),
            scalars,
        };
        let pow = b.table(Source::Pow);
        let usable = b.table(Source::Usable);
        let mut terms = Vec::new();

        // Gate polynomials: pow * guard * body, where the guard is the key
        // polynomial of the gate's guard values on its rows.
        for gate in &relation.gates {
            let mut factors = Vec::new();
            top_factors(&gate.expression, &mut factors);
            let body: Vec<Poly<S<C>>> = factors
                .iter()
                .filter(|f| folded_degree(f) != 0)
                .map(|f| b.compile(f))
                .collect::<Vec<_>>();
            let guard = b.sparse_key(&gate.rows, &gate.guards);
            let guard = b.key_source(guard, 0);
            let poly = Poly::table(pow).mul(Poly::table(guard)).mul(product(body));
            terms.push(Term {
                weight: Weight::Index(gate.class),
                poly,
                support: Support::Rows(gate.rows.clone()),
            });
        }

        // LogUp: slot cells, table rows, and the linear sums over exactly the
        // constrained cells.
        let groups = relation.groups.len();
        for (g, group) in relation.groups.iter().enumerate() {
            let r = Poly::scalar(scalars.r(g));
            for (slot, chosen) in group.slot_members.iter().enumerate() {
                let rows: Vec<u32> = group.slots[slot].iter().map(|cell| cell.row).collect();
                let ones = vec![S::<C>::ONE; rows.len()];
                let activity = b.sparse_key(&rows, &ones);
                let activity = b.key_source(activity, 0);
                let h = b.table(Source::Witness(group.h_column(slot), 0));
                let parts: Vec<Poly<S<C>>> = chosen
                    .iter()
                    .map(|m| {
                        let input = b.compress(&group.members[usize::from(*m)].expressions);
                        r.clone().add(input)
                    })
                    .collect();
                let mut cell = Poly::table(h).mul(product(parts.iter().cloned()));
                for k in 0..parts.len() {
                    cell = cell.sub(product(
                        parts
                            .iter()
                            .enumerate()
                            .filter(|(j, _)| *j != k)
                            .map(|(_, p)| p.clone()),
                    ));
                }
                let poly = Poly::table(pow).mul(Poly::table(activity)).mul(cell);
                terms.push(Term {
                    weight: Weight::Index(group.slot_constraints[slot]),
                    poly,
                    support: Support::Rows(rows.clone()),
                });
                terms.push(Term {
                    weight: Weight::Alpha(1 + g),
                    poly: Poly::table(activity).mul(Poly::table(h)),
                    support: Support::Rows(rows),
                });
            }
            let m = b.table(Source::Witness(group.m_column(), 0));
            let gc = b.table(Source::Witness(group.g_column(), 0));
            let table = b.compress(&group.table_expressions);
            let poly = Poly::table(pow)
                .mul(Poly::table(usable))
                .mul(Poly::table(gc).mul(r.add(table)).sub(Poly::table(m)));
            terms.push(Term {
                weight: Weight::Index(group.table_constraint),
                poly,
                support: Support::Prefix(relation.usable),
            });
            terms.push(Term {
                weight: Weight::Alpha(1 + g),
                poly: Poly::table(usable).mul(Poly::table(gc)).neg(),
                support: Support::Prefix(relation.usable),
            });
        }

        // Copies: a LogUp permutation argument. Column j holds the ids
        // j n .. (j + 1) n; instance column c follows all copy columns.
        let mut columns: Vec<CopyColumn> = Vec::new();
        let mut column_index: HashMap<(bool, usize), usize> = HashMap::new();
        for (a, bcell) in &relation.copies {
            for cell in [a, bcell] {
                let key = match *cell {
                    Cell::Advice(c, _) => (true, c as usize),
                    Cell::Fixed(c, _) => (false, c as usize),
                    Cell::Instance(..) => continue,
                };
                column_index.entry(key).or_insert_with(|| {
                    columns.push(if key.0 {
                        CopyColumn::Witness(key.1)
                    } else {
                        CopyColumn::Fixed(key.1)
                    });
                    columns.len() - 1
                });
            }
        }
        let n64 = n as u64;
        let copy_count = columns.len() as u64;
        let id_of = |cell: &Cell| -> u64 {
            match *cell {
                Cell::Advice(c, r) => column_index[&(true, c as usize)] as u64 * n64 + u64::from(r),
                Cell::Fixed(c, r) => column_index[&(false, c as usize)] as u64 * n64 + u64::from(r),
                Cell::Instance(c, r) => (copy_count + u64::from(c)) * n64 + u64::from(r),
            }
        };
        let mut parent: HashMap<u64, u64> = HashMap::new();
        for (a, bcell) in &relation.copies {
            let (x, y) = (root(&mut parent, id_of(a)), root(&mut parent, id_of(bcell)));
            if x != y {
                parent.insert(x.max(y), x.min(y));
            }
        }
        let mut cycles: HashMap<u64, Vec<u64>> = HashMap::new();
        let mut members: Vec<u64> = relation
            .copies
            .iter()
            .flat_map(|(a, c)| [id_of(a), id_of(c)])
            .collect();
        members.sort_unstable();
        members.dedup();
        for id in members {
            let r = root(&mut parent, id);
            cycles.entry(r).or_default().push(id);
        }
        let mut sigma_columns: Vec<Vec<u64>> = (0..columns.len())
            .map(|j| (0..n64).map(|i| j as u64 * n64 + i).collect())
            .collect();
        let mut moved: Vec<Vec<u32>> = vec![Vec::new(); columns.len()];
        let mut instance_cells = Vec::new();
        for cycle in cycles.values_mut() {
            cycle.sort_unstable();
            for (k, id) in cycle.iter().enumerate() {
                let next = cycle[(k + 1) % cycle.len()];
                let (column, row) = ((id / n64) as usize, (id % n64) as usize);
                if (column as u64) < copy_count {
                    sigma_columns[column][row] = next;
                    if next != *id {
                        moved[column].push(u32::try_from(row).expect("rows fit u32"));
                    }
                } else {
                    instance_cells.push((column - columns.len(), row, *id, next));
                }
            }
        }
        instance_cells.sort_unstable();
        let mut sigma = Vec::with_capacity(columns.len());
        let eq_tau = b.table(Source::EqTau);
        let index = b.table(Source::Index);
        let gamma = Poly::scalar(scalars.gamma());
        let eta = Poly::scalar(scalars.eta());
        // Per column: L = gamma + v + eta id, R = gamma + v + eta sigma and the
        // correction eta (sigma - id) = R - L.
        let mut parts = Vec::with_capacity(columns.len());
        for (j, column) in columns.iter().enumerate() {
            moved[j].sort_unstable();
            let s = b.add_key(sigma_columns[j].iter().map(|v| S::<C>::from(*v)).collect());
            sigma.push(s);
            let value = match column {
                CopyColumn::Witness(c) => b.table(Source::Witness(*c, 0)),
                CopyColumn::Fixed(c) => b.key_table(KeyName::Fixed(*c), 0),
            };
            let sigma_table = b.key_source(s, 0);
            let id = Poly::constant(S::<C>::from(j as u64 * n64)).add(Poly::table(index));
            let left = gamma
                .clone()
                .add(Poly::table(value))
                .add(eta.clone().mul(id.clone()));
            let right = gamma
                .clone()
                .add(Poly::table(value))
                .add(eta.clone().mul(Poly::table(sigma_table)));
            let correction = eta.clone().mul(Poly::table(sigma_table).sub(id));
            parts.push((left, right, correction));
        }
        // One helper per group of columns: z = sum_c (1 / L_c - 1 / R_c), by the
        // row constraint z prod_c L_c R_c = sum_c eta (sigma_c - id_c)
        // prod_{c' != c} L_c' R_c', which vanishes off the group's moved rows.
        let helpers: Vec<Vec<usize>> = (0..columns.len())
            .collect::<Vec<_>>()
            .chunks(COPY_GROUP)
            .map(<[usize]>::to_vec)
            .collect();
        for (q, members) in helpers.iter().enumerate() {
            let helper = b.table(Source::Helper(q));
            let mut rows: Vec<u32> = members
                .iter()
                .flat_map(|j| moved[*j].iter().copied())
                .collect();
            rows.sort_unstable();
            rows.dedup();
            let products: Vec<Poly<S<C>>> = members
                .iter()
                .map(|j| parts[*j].0.clone().mul(parts[*j].1.clone()))
                .collect();
            let mut row = Poly::table(helper).mul(product(products.iter().cloned()));
            for (k, j) in members.iter().enumerate() {
                let others: Vec<Poly<S<C>>> = products
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != k)
                    .map(|(_, p)| p.clone())
                    .collect();
                let correction = parts[*j].2.clone();
                row = row.sub(if others.is_empty() {
                    correction
                } else {
                    correction.mul(product(others))
                });
            }
            terms.push(Term {
                weight: Weight::AlphaKappa(groups + 1, q),
                poly: Poly::table(eq_tau).mul(row),
                support: Support::Rows(rows.clone()),
            });
            terms.push(Term {
                weight: Weight::Alpha(groups + 2),
                poly: Poly::table(helper),
                support: Support::Rows(rows),
            });
        }

        let degree = terms
            .iter()
            .map(|t| t.poly.degree())
            .max()
            .unwrap_or(1)
            .max(1);
        if degree > MAX_DEGREE {
            return Err(Error::Degree(degree));
        }
        let key_commitments = b
            .key_polys
            .iter()
            .map(|poly| key.commit(poly, &S::<C>::ZERO))
            .collect::<Result<Vec<_>, _>>()?;
        let copies = CopyPlan {
            columns,
            sigma,
            rows: moved,
            helpers,
            instance_cells,
        };
        let Builder {
            sources,
            key_polys,
            cells,
            ..
        } = b;
        let steps = sources
            .iter()
            .map(|source| match *source {
                Source::Steps(key, rotation) => {
                    Some(Steps::of(&mle::rotated(&key_polys[key], rotation)))
                }
                _ => None,
            })
            .collect();
        let mut committed: Vec<Committed> = sources
            .iter()
            .filter_map(|s| s.committed().map(|(p, _)| p))
            .collect();
        committed.sort_unstable();
        committed.dedup();
        let reads = sources
            .iter()
            .enumerate()
            .filter_map(|(source, s)| {
                s.committed().map(|(p, rotation)| Read {
                    source,
                    poly: committed.binary_search(&p).expect("listed"),
                    rotation,
                })
            })
            .collect();
        Ok(Self {
            relation,
            key: key.clone(),
            mu,
            step_runs,
            sources,
            steps,
            key_polys,
            key_commitments,
            terms,
            copies,
            cells,
            degree,
            committed,
            reads,
        })
    }

    /// The commitments of the key polynomials (fixed per relation).
    #[must_use]
    pub fn key_commitments(&self) -> &[C::AffineExt] {
        &self.key_commitments
    }

    /// The step bound: key polynomials of at most this many constant runs are
    /// evaluated by the verifier (the prover and the verifier of a decider must
    /// use the same).
    #[must_use]
    pub const fn step_runs(&self) -> usize {
        self.step_runs
    }

    /// The degree of the zero-check's round polynomials.
    #[must_use]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    /// The row bits `mu`: the domain is `2^mu` rows, and the batched opening
    /// has `mu` rounds.
    #[must_use]
    pub const fn mu(&self) -> usize {
        self.mu
    }

    /// Copy-helper commitments per proof of this plan (one per group of
    /// [`COPY_GROUP`] columns with copy cells).
    #[must_use]
    pub fn helper_count(&self) -> usize {
        self.copies.helpers.len()
    }

    /// The instance cells in copies, as (column, row, id, `sigma(id)`): the
    /// cells the instance copy term `I` sums over
    /// ([`super::instance_copy_terms`]).
    #[must_use]
    pub fn instance_copy_cells(&self) -> &[(usize, usize, u64, u64)] {
        &self.copies.instance_cells
    }

    /// Whether the relation reads public values beyond copies (instance
    /// columns queried by gates, or instance cells read as scalars). A
    /// field-side decider whose public columns stay on the commitment side
    /// (an imported `I`) requires that it does not.
    #[must_use]
    pub fn reads_public_beyond_copies(&self) -> bool {
        !self.cells.is_empty()
            || self
                .sources
                .iter()
                .any(|source| matches!(source, Source::Instance(..)))
    }

    /// The verifier's work beyond the opening: field multiplications of the
    /// final term evaluation (one per term weight included) and of the step
    /// keys (at most `2 mu + 1` per change of value, before sharing), scalars
    /// absorbed per proof, and the opened commitments as (witness, key,
    /// helper).
    #[must_use]
    pub fn verifier_profile(&self) -> (usize, usize, (usize, usize, usize)) {
        let steps: usize = self
            .steps
            .iter()
            .flatten()
            .map(|s| s.changes.len() * (2 * self.mu + 1))
            .sum();
        let multiplications = self
            .terms
            .iter()
            .map(|t| t.poly.multiplications() + 1)
            .sum::<usize>()
            + steps;
        let absorbed =
            self.mu * self.degree + self.reads.len() + 2 * self.mu + self.committed.len();
        let mut opened = (0, 0, 0);
        for poly in &self.committed {
            match poly {
                Committed::Witness(_) => opened.0 += 1,
                Committed::Key(_) => opened.1 += 1,
                Committed::Helper(_) => opened.2 += 1,
            }
        }
        (multiplications, absorbed, opened)
    }

    /// For verifier-cost planning of one batch of `plans` (in member order):
    /// the claims its opening proves (a key polynomial shared by members
    /// once), the field scalars its transcript absorbs per proof besides the
    /// digests (round values, reads at `rho`, shift rounds, claim values), and
    /// its round degree.
    #[must_use]
    pub fn batch_profile(plans: &[&Self]) -> (usize, usize, usize) {
        let claims = super::batch_claims(plans).opened.len();
        let mu = plans.first().map_or(0, |plan| plan.mu);
        let degree = plans.iter().map(|plan| plan.degree).max().unwrap_or(1);
        let reads: usize = plans.iter().map(|plan| plan.reads.len()).sum();
        (claims, mu * degree + reads + 2 * mu + claims, degree)
    }

    /// For verifier-cost planning: each key polynomial's number of maximal
    /// constant runs (cyclic, so a constant column has one) and the run
    /// boundaries (row indices where a value changes).
    #[must_use]
    pub fn key_runs(&self) -> Vec<(usize, Vec<usize>)> {
        self.key_polys
            .iter()
            .map(|poly| {
                let n = poly.len();
                let boundaries: Vec<usize> = (0..n)
                    .filter(|&i| poly[i] != poly[(i + n - 1) % n])
                    .collect();
                (boundaries.len().max(1), boundaries)
            })
            .collect()
    }

    /// Sizes of the plan: zero-check tables, key polynomials, copy helpers,
    /// terms.
    #[must_use]
    pub fn sizes(&self) -> (usize, usize, usize, usize) {
        (
            self.sources.len(),
            self.key_polys.len(),
            self.copies.helpers.len(),
            self.terms.len(),
        )
    }
}
