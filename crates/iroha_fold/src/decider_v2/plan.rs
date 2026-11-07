//! The fixed layout of a v2 decider: the tables its zero-check reads, its
//! weighted terms, its key polynomials and its copy permutation.

use std::collections::HashMap;

use ff::Field;
use iroha_pasta::{PastaCurve, PastaField};
use iroha_plonk::cs::Expression;

use super::eval::{MAX_DEGREE, Poly};
use crate::{
    Error,
    commit::CommitKey,
    program::folded_degree,
    relation::{Cell, FoldedRelation},
};

/// A table of the zero-check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Source {
    /// An accumulator witness column at a rotation.
    Witness(usize, i32),
    /// A key polynomial at a rotation.
    Key(usize, i32),
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
    pub(crate) sources: Vec<Source>,
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
    named: HashMap<KeyName, usize>,
    sparse: HashMap<Vec<(u32, [u64; 4])>, usize>,
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
    polys.into_iter().reduce(Poly::mul).unwrap_or_else(|| Poly::constant(F::ONE))
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

    fn named_key(&mut self, name: KeyName) -> usize {
        if let Some(index) = self.named.get(&name) {
            return *index;
        }
        let relation = self.relation;
        let values = match name {
            KeyName::Fixed(c) => relation.fixed[c].clone(),
            KeyName::Selector(s) => {
                relation.selectors[s].iter().map(|on| if *on { F::ONE } else { F::ZERO }).collect()
            }
        };
        let index = self.key_polys.len();
        self.key_polys.push(values);
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
        let index = self.key_polys.len();
        self.key_polys.push(column);
        self.sparse.insert(content, index);
        index
    }

    fn key_table(&mut self, name: KeyName, rotation: i32) -> usize {
        let key = self.named_key(name);
        self.table(Source::Key(key, rotation))
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
            Expression::Selector(selector) => Poly::table(self.key_table(KeyName::Selector(selector.index()), 0)),
            Expression::Fixed(query) => {
                Poly::table(self.key_table(KeyName::Fixed(query.column_index), query.rotation.0))
            }
            Expression::Advice(query) => Poly::table(self.table(Source::Witness(query.column_index, query.rotation.0))),
            Expression::Instance(query) => {
                Poly::table(self.table(Source::Instance(query.column_index, query.rotation.0)))
            }
            Expression::InstanceCell(query) => Poly::scalar(self.cell(query.column_index, query.row)),
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
            out = out.add(if j == 0 { term } else { Poly::scalar(j - 1).mul(term) });
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
    /// `eq` tensor). Commits the key polynomials.
    ///
    /// # Errors
    /// [`Error::Shape`] for a basis shorter than the domain, [`Error::Degree`]
    /// for a term of degree above 15, [`Error::Msm`].
    #[allow(clippy::too_many_lines)]
    pub fn new(relation: FoldedRelation<C::ScalarExt>, key: &CommitKey<C>) -> Result<Self, Error> {
        type S<C> = <C as PastaCurve>::ScalarExt;
        let n = relation.n;
        if key.len() < n {
            return Err(Error::Shape("commitment basis shorter than the domain"));
        }
        let mu = n.trailing_zeros() as usize;
        let scalars = Scalars { theta: relation.theta_len, groups: relation.groups.len() };
        let mut b = Builder {
            relation: &relation,
            sources: Vec::new(),
            source_index: HashMap::new(),
            key_polys: Vec::new(),
            named: HashMap::new(),
            sparse: HashMap::new(),
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
            let guard = b.table(Source::Key(guard, 0));
            let poly = Poly::table(pow).mul(Poly::table(guard)).mul(product(body));
            terms.push(Term { weight: Weight::Index(gate.class), poly, support: Support::Rows(gate.rows.clone()) });
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
                let activity = b.table(Source::Key(activity, 0));
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
                    cell = cell.sub(product(parts.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, p)| p.clone())));
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
            terms.push(Term { weight: Weight::Index(group.table_constraint), poly, support: Support::Prefix(relation.usable) });
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
                    columns.push(if key.0 { CopyColumn::Witness(key.1) } else { CopyColumn::Fixed(key.1) });
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
        let mut members: Vec<u64> = relation.copies.iter().flat_map(|(a, c)| [id_of(a), id_of(c)]).collect();
        members.sort_unstable();
        members.dedup();
        for id in members {
            let r = root(&mut parent, id);
            cycles.entry(r).or_default().push(id);
        }
        let mut sigma_columns: Vec<Vec<u64>> =
            (0..columns.len()).map(|j| (0..n64).map(|i| j as u64 * n64 + i).collect()).collect();
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
        for (j, column) in columns.iter().enumerate() {
            moved[j].sort_unstable();
            let s = b.key_polys.len();
            b.key_polys.push(sigma_columns[j].iter().map(|v| S::<C>::from(*v)).collect());
            sigma.push(s);
            let value = match column {
                CopyColumn::Witness(c) => b.table(Source::Witness(*c, 0)),
                CopyColumn::Fixed(c) => b.key_table(KeyName::Fixed(*c), 0),
            };
            let sigma_table = b.table(Source::Key(s, 0));
            let helper = b.table(Source::Helper(j));
            let id = Poly::constant(S::<C>::from(j as u64 * n64)).add(Poly::table(index));
            let left = gamma.clone().add(Poly::table(value)).add(eta.clone().mul(id.clone()));
            let right = gamma.clone().add(Poly::table(value)).add(eta.clone().mul(Poly::table(sigma_table)));
            let row = Poly::table(helper)
                .mul(left)
                .mul(right)
                .sub(eta.clone().mul(Poly::table(sigma_table).sub(id)));
            terms.push(Term {
                weight: Weight::AlphaKappa(groups + 1, j),
                poly: Poly::table(eq_tau).mul(row),
                support: Support::Rows(moved[j].clone()),
            });
            terms.push(Term {
                weight: Weight::Alpha(groups + 2),
                poly: Poly::table(helper),
                support: Support::Rows(moved[j].clone()),
            });
        }

        let degree = terms.iter().map(|t| t.poly.degree()).max().unwrap_or(1).max(1);
        if degree > MAX_DEGREE {
            return Err(Error::Degree(degree));
        }
        let key_commitments =
            b.key_polys.iter().map(|poly| key.commit(poly, &S::<C>::ZERO)).collect::<Result<Vec<_>, _>>()?;
        let copies = CopyPlan { columns, sigma, rows: moved, instance_cells };
        let Builder { sources, key_polys, cells, .. } = b;
        let mut committed: Vec<Committed> = sources.iter().filter_map(|s| s.committed().map(|(p, _)| p)).collect();
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
            sources,
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

    /// The degree of the zero-check's round polynomials.
    #[must_use]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    /// Sizes of the plan: zero-check tables, key polynomials, copy helpers,
    /// terms.
    #[must_use]
    pub fn sizes(&self) -> (usize, usize, usize, usize) {
        (self.sources.len(), self.key_polys.len(), self.copies.columns.len(), self.terms.len())
    }
}
