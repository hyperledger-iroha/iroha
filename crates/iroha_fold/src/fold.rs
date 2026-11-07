//! `ProtoGalaxy` folding (k = 1) of fresh instances into an accumulator.
//!
//! An accumulator `(phi, beta, e)` with witness `w` satisfies
//! `sum_i pow_i(beta) f_i(w, phi) = e` over every constraint `f_i` of the relation,
//! plus the affine relations (copies, `LogUp` sums, commitment openings) that every
//! instance satisfies. A fresh instance satisfies `f_i = 0` for all `i`.
//!
//! One fold (`ProtoGalaxy`, ePrint 2023/1106, section 4):
//!
//! 1. `delta` from the transcript; the prover sends `F(X) = sum_i pow_i(beta + X
//!    delta) f_i(w_acc)` without its constant term (which is `e`);
//! 2. `alpha`; `beta* = beta + alpha delta` and `F(alpha)`;
//! 3. the prover sends `K(X)` with `G(X) = F(alpha) L0(X) + Z(X) K(X)`, where
//!    `G(X) = sum_i pow_i(beta*) f_i(L0(X) w_acc + L1(X) w_new)`, `L0 = 1 - X`,
//!    `L1 = X`, `Z = X (X - 1)`;
//! 4. `gamma` (128 bits); everything folds as `acc + gamma (new - acc)` and
//!    `e* = F(alpha) L0(gamma) + Z(gamma) K(gamma)`.
//!
//! Here `delta` stands for the vector `(delta, delta^2, delta^4, ...)`. The native
//! verifier recomputes every challenge, including the fresh instance's `LogUp`
//! challenges, and the folded instance; it rejects nothing by itself: a dishonest
//! fold yields an accumulator the decider rejects.

use ff::Field;
use iroha_pasta::PastaCurve;
use rand_core::{CryptoRng, Rng};

use crate::{
    Error,
    commit::{CommitKey, Witness, fold_point, fold_scalar},
    logup::{self, compress},
    program::{MAX_POINTS, Point},
    relation::FoldedRelation,
    transcript::{TAG_ACCUMULATOR, TAG_FOLD, TAG_INSTANCE, Transcript},
};

/// A folding instance: commitments to every witness column and the folded public
/// variables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance<C: PastaCurve> {
    /// One commitment per witness column, in relation order.
    pub commitments: Vec<C::AffineExt>,
    /// Instance (public input) columns of the circuit.
    pub public: Vec<Vec<C::ScalarExt>>,
    /// `LogUp` tuple-compression powers `theta^1 .. theta^L`.
    pub theta: Vec<C::ScalarExt>,
    /// One `LogUp` challenge per group.
    pub r: Vec<C::ScalarExt>,
}

/// A relaxed accumulator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accumulator<C: PastaCurve> {
    /// The folded instance.
    pub instance: Instance<C>,
    /// `ProtoGalaxy` `pow` challenges, one per constraint-index bit.
    pub beta: Vec<C::ScalarExt>,
    /// The accumulated error.
    pub e: C::ScalarExt,
}

/// The output of [`fold`]: the new accumulator, its witness and the prover's messages.
pub type FoldOutput<C> = (
    Accumulator<C>,
    Witness<<C as PastaCurve>::ScalarExt>,
    FoldProof<<C as PastaCurve>::ScalarExt>,
);

/// The prover's messages of one fold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoldProof<F> {
    /// Coefficients 1..=t of `F(X)` (the constant term is the accumulator's `e`).
    pub f: Vec<F>,
    /// Coefficients of `K(X)`, degree `d - 2`.
    pub k: Vec<F>,
}

/// Every witness column and public variable of one point.
#[derive(Clone, Copy)]
pub(crate) struct View<'a, F> {
    pub(crate) columns: &'a [Vec<F>],
    pub(crate) public: &'a [Vec<F>],
    pub(crate) theta: &'a [F],
    pub(crate) r: &'a [F],
}

impl<'a, F> View<'a, F> {
    pub(crate) fn of(
        witness: &'a Witness<F>,
        public: &'a [Vec<F>],
        theta: &'a [F],
        r: &'a [F],
    ) -> Self {
        Self {
            columns: &witness.columns,
            public,
            theta,
            r,
        }
    }
    fn point(&self, advice: usize) -> Point<'a, F> {
        Point {
            advice: &self.columns[..advice],
            instance: self.public,
        }
    }
}

fn absorb_instance<C: PastaCurve>(
    transcript: &mut Transcript<C::ScalarExt>,
    instance: &Instance<C>,
) {
    transcript.absorb_u64(instance.commitments.len() as u64);
    for point in &instance.commitments {
        transcript.absorb_point(point);
    }
    absorb_public::<C>(transcript, &instance.public);
    transcript.absorb_scalars(&instance.theta);
    transcript.absorb_scalars(&instance.r);
}

fn absorb_public<C: PastaCurve>(
    transcript: &mut Transcript<C::ScalarExt>,
    public: &[Vec<C::ScalarExt>],
) {
    transcript.absorb_u64(public.len() as u64);
    for column in public {
        transcript.absorb_u64(column.len() as u64);
        transcript.absorb_scalars(column);
    }
}

/// The round-one challenges of a fresh instance: `theta` powers and one `r` per
/// group, from the public columns and the advice and multiplicity commitments.
fn round_one_challenges<C: PastaCurve>(
    relation: &FoldedRelation<C::ScalarExt>,
    domain: &[u8; 32],
    public: &[Vec<C::ScalarExt>],
    commitments: &[C::AffineExt],
) -> (Vec<C::ScalarExt>, Vec<C::ScalarExt>) {
    let mut transcript = Transcript::<C::ScalarExt>::new(TAG_INSTANCE, domain);
    absorb_public::<C>(&mut transcript, public);
    for point in &commitments[..relation.advice_columns] {
        transcript.absorb_point(point);
    }
    for group in &relation.groups {
        transcript.absorb_point(&commitments[group.m_column()]);
    }
    let theta = transcript.challenge();
    let mut powers = Vec::with_capacity(relation.theta_len);
    let mut power = theta;
    for _ in 0..relation.theta_len {
        powers.push(power);
        power *= theta;
    }
    let r = relation
        .groups
        .iter()
        .map(|_| transcript.challenge())
        .collect();
    (powers, r)
}

impl<C: PastaCurve> Instance<C> {
    /// Whether the `LogUp` challenges are the transcript's for this instance's
    /// round-one data (required of every fresh instance before it is used).
    #[must_use]
    pub fn has_fresh_challenges(
        &self,
        relation: &FoldedRelation<C::ScalarExt>,
        domain: &[u8; 32],
    ) -> bool {
        if self.commitments.len() != relation.columns() {
            return false;
        }
        let (theta, r) =
            round_one_challenges::<C>(relation, domain, &self.public, &self.commitments);
        theta == self.theta && r == self.r
    }

    fn same_shape(&self, other: &Self) -> bool {
        self.commitments.len() == other.commitments.len()
            && self.theta.len() == other.theta.len()
            && self.r.len() == other.r.len()
            && self.public.len() == other.public.len()
            && self
                .public
                .iter()
                .zip(&other.public)
                .all(|(a, b)| a.len() == b.len())
    }
}

/// Builds a fresh instance and its witness from a stage's advice and instance
/// columns: commits the advice and the `LogUp` multiplicities, derives `theta` and
/// `r`, then commits the reciprocal columns. Blinding scalars come from `rng`.
///
/// # Errors
/// [`Error::Shape`] on shape mismatch or a zero `LogUp` denominator,
/// [`Error::LookupInput`] when an input tuple is not in its table, [`Error::Msm`].
pub fn fresh_instance<C: PastaCurve, R: Rng + CryptoRng>(
    relation: &FoldedRelation<C::ScalarExt>,
    key: &CommitKey<C>,
    domain: &[u8; 32],
    advice: Vec<Vec<C::ScalarExt>>,
    public: Vec<Vec<C::ScalarExt>>,
    rng: &mut R,
) -> Result<(Instance<C>, Witness<C::ScalarExt>), Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    if advice.len() != relation.advice_columns || advice.iter().any(|c| c.len() != relation.n) {
        return Err(Error::Shape("advice columns"));
    }
    if public
        .iter()
        .map(Vec::len)
        .ne(relation.instance_lengths.iter().copied())
    {
        return Err(Error::Shape("instance columns"));
    }
    if key.len() < relation.n {
        return Err(Error::Shape("commitment basis shorter than the domain"));
    }
    let tables = relation.tables();
    let columns_total = relation.columns();
    let mut columns: Vec<Vec<S<C>>> = advice;
    columns.resize(columns_total, Vec::new());
    let point = Point {
        advice: &columns[..relation.advice_columns],
        instance: &public,
    };
    let mut ms = Vec::with_capacity(relation.groups.len());
    for group in &relation.groups {
        ms.push(logup::multiplicities(group, &tables, point)?);
    }
    for (group, m) in relation.groups.iter().zip(ms) {
        columns[group.m_column()] = m;
    }
    let blinds: Vec<S<C>> = (0..columns_total)
        .map(|_| S::<C>::random(&mut *rng))
        .collect();
    let mut commitments = vec![C::AffineExt::default(); columns_total];
    for c in 0..relation.advice_columns {
        commitments[c] = key.commit(&columns[c], &blinds[c])?;
    }
    for group in &relation.groups {
        let c = group.m_column();
        commitments[c] = key.commit(&columns[c], &blinds[c])?;
    }
    let (theta, r) = round_one_challenges::<C>(relation, domain, &public, &commitments);
    for (group, rg) in relation.groups.iter().zip(&r) {
        let point = Point {
            advice: &columns[..relation.advice_columns],
            instance: &public,
        };
        let (h, g) = logup::reciprocals(
            group,
            &tables,
            point,
            &columns[group.m_column()],
            &theta,
            *rg,
        )?;
        for (slot, column) in h.into_iter().enumerate() {
            columns[group.h_column(slot)] = column;
        }
        columns[group.g_column()] = g;
        for c in (group.m_column() + 1)..=group.g_column() {
            commitments[c] = key.commit(&columns[c], &blinds[c])?;
        }
    }
    Ok((
        Instance {
            commitments,
            public,
            theta,
            r,
        },
        Witness { columns, blinds },
    ))
}

impl<C: PastaCurve> Accumulator<C> {
    /// Starts an accumulator from a fresh instance: `beta` from the transcript after
    /// the instance, `e = 0`. The instance's own challenges are checked.
    ///
    /// # Errors
    /// [`Error::Rejected`] when the instance's `LogUp` challenges are not its own.
    pub fn from_fresh(
        relation: &FoldedRelation<C::ScalarExt>,
        domain: &[u8; 32],
        instance: Instance<C>,
    ) -> Result<Self, Error> {
        if !instance.has_fresh_challenges(relation, domain) {
            return Err(Error::Rejected("fresh instance challenges"));
        }
        let mut transcript = Transcript::<C::ScalarExt>::new(TAG_ACCUMULATOR, domain);
        absorb_instance(&mut transcript, &instance);
        let beta = (0..relation.index_bits())
            .map(|_| transcript.challenge())
            .collect();
        Ok(Self {
            instance,
            beta,
            e: <C::ScalarExt as Field>::ZERO,
        })
    }
}

/// Calls `sink(row, index, value)` for every constraint of `relation` at `view`.
/// Table constraints whose `g` and `m` both vanish are exactly zero and skipped.
pub(crate) fn constraints_at<F: iroha_pasta::PastaField>(
    relation: &FoldedRelation<F>,
    view: View<'_, F>,
    mut sink: impl FnMut(usize, usize, F),
) {
    let tables = relation.tables();
    let point = view.point(relation.advice_columns);
    let mut stack = Vec::with_capacity(64);
    for (index, gate) in relation.gates.iter().enumerate() {
        for (row, guard) in gate.rows.iter().zip(&gate.guards) {
            let row = *row as usize;
            sink(
                row,
                index,
                gate.body.eval(&tables, point, row, &mut stack) * guard,
            );
        }
    }
    for (gi, group) in relation.groups.iter().enumerate() {
        let r = view.r[gi];
        for (slot, cells) in group.slots.iter().enumerate() {
            let h_column = &view.columns[group.h_column(slot)];
            for cell in cells {
                let row = cell.row as usize;
                let mut terms = [F::ZERO; logup::CELL_INPUTS];
                let members = cell.members();
                for (term, member) in terms.iter_mut().zip(members) {
                    let member = &group.members[usize::from(*member)];
                    *term =
                        r + compress(&member.inputs, view.theta, &tables, point, row, &mut stack);
                }
                sink(
                    row,
                    group.first_constraint + slot,
                    cell_constraint(h_column[row], &terms[..members.len()]),
                );
            }
        }
        let (m, g) = (
            &view.columns[group.m_column()],
            &view.columns[group.g_column()],
        );
        for row in 0..group.usable {
            if bool::from(m[row].is_zero()) && bool::from(g[row].is_zero()) {
                continue;
            }
            let s = compress(&group.table, view.theta, &tables, point, row, &mut stack);
            sink(
                row,
                group.first_constraint + group.slots.len(),
                g[row] * (r + s) - m[row],
            );
        }
    }
}

/// `h prod(t_k) - sum_k prod_{j != k} t_j` for one reciprocal cell.
fn cell_constraint<F: Field>(h: F, terms: &[F]) -> F {
    match terms {
        [a] => h * a - F::ONE,
        [a, b] => h * a * b - (*a + *b),
        [a, b, c] => h * a * b * c - (*b * c + *a * c + *a * b),
        _ => unreachable!("one to three inputs per cell"),
    }
}

/// As [`constraints_at`] along the line `acc + x (new - acc)` for every `x` in
/// `xs` (at most [`MAX_POINTS`]).
fn constraints_on_line<F: iroha_pasta::PastaField>(
    relation: &FoldedRelation<F>,
    acc: View<'_, F>,
    new: View<'_, F>,
    xs: &[F],
    mut sink: impl FnMut(usize, usize, &[F; MAX_POINTS]),
) {
    let tables = relation.tables();
    let (pa, pn) = (
        acc.point(relation.advice_columns),
        new.point(relation.advice_columns),
    );
    let m = xs.len();
    let line = |a: F, b: F| {
        let mut out = [F::ZERO; MAX_POINTS];
        for (o, x) in out.iter_mut().zip(xs) {
            *o = a + *x * (b - a);
        }
        out
    };
    let mut stack = Vec::with_capacity(64);
    for (index, gate) in relation.gates.iter().enumerate() {
        for (row, guard) in gate.rows.iter().zip(&gate.guards) {
            let row = *row as usize;
            let mut v = gate.body.eval_line(&tables, pa, pn, xs, row, &mut stack);
            for x in v.iter_mut().take(m) {
                *x *= guard;
            }
            sink(row, index, &v);
        }
    }
    let thetas: Vec<[F; MAX_POINTS]> = acc
        .theta
        .iter()
        .zip(new.theta)
        .map(|(a, b)| line(*a, *b))
        .collect();
    let compress_line =
        |programs: &[crate::program::Program<F>], row: usize, stack: &mut Vec<[F; MAX_POINTS]>| {
            let mut out = [F::ZERO; MAX_POINTS];
            for (j, program) in programs.iter().enumerate() {
                let v = program.eval_line(&tables, pa, pn, xs, row, stack);
                for i in 0..m {
                    out[i] += if j == 0 {
                        v[i]
                    } else {
                        thetas[j - 1][i] * v[i]
                    };
                }
            }
            out
        };
    for (gi, group) in relation.groups.iter().enumerate() {
        let r = line(acc.r[gi], new.r[gi]);
        for (slot, cells) in group.slots.iter().enumerate() {
            let column = group.h_column(slot);
            for cell in cells {
                let row = cell.row as usize;
                let h = line(acc.columns[column][row], new.columns[column][row]);
                let members = cell.members();
                let mut terms = [[F::ZERO; MAX_POINTS]; logup::CELL_INPUTS];
                for (term, member) in terms.iter_mut().zip(members) {
                    let member = &group.members[usize::from(*member)];
                    let a = compress_line(&member.inputs, row, &mut stack);
                    for i in 0..m {
                        term[i] = r[i] + a[i];
                    }
                }
                let mut out = [F::ZERO; MAX_POINTS];
                for i in 0..m {
                    let t: Vec<F> = terms[..members.len()].iter().map(|t| t[i]).collect();
                    out[i] = cell_constraint(h[i], &t);
                }
                sink(row, group.first_constraint + slot, &out);
            }
        }
        let (mc, gc) = (group.m_column(), group.g_column());
        for row in 0..group.usable {
            let zero = |c: usize| {
                bool::from(acc.columns[c][row].is_zero())
                    && bool::from(new.columns[c][row].is_zero())
            };
            if zero(mc) && zero(gc) {
                continue;
            }
            let s = compress_line(&group.table, row, &mut stack);
            let (mv, gv) = (
                line(acc.columns[mc][row], new.columns[mc][row]),
                line(acc.columns[gc][row], new.columns[gc][row]),
            );
            let mut out = [F::ZERO; MAX_POINTS];
            for i in 0..m {
                out[i] = gv[i] * (r[i] + s[i]) - mv[i];
            }
            sink(row, group.first_constraint + group.slots.len(), &out);
        }
    }
}

/// `pow_i(beta)` for `i < 2^beta.len()`, little-endian bits.
pub(crate) fn pow_vector<F: Field>(beta: &[F]) -> Vec<F> {
    let mut out = Vec::with_capacity(1 << beta.len());
    out.push(F::ONE);
    for b in beta {
        let len = out.len();
        for i in 0..len {
            let v = out[i] * b;
            out.push(v);
        }
    }
    out
}

fn mul_linear<F: Field>(poly: &[F], a: F, b: F) -> Vec<F> {
    let mut out = vec![F::ZERO; poly.len() + 1];
    for (i, c) in poly.iter().enumerate() {
        out[i] += *c * a;
        out[i + 1] += *c * b;
    }
    out
}

fn add_into<F: Field>(left: &mut Vec<F>, right: &[F]) {
    if left.len() < right.len() {
        left.resize(right.len(), F::ZERO);
    }
    for (l, r) in left.iter_mut().zip(right) {
        *l += *r;
    }
}

fn eval_poly<F: Field>(poly: &[F], x: F) -> F {
    poly.iter().rev().fold(F::ZERO, |acc, c| acc * x + c)
}

/// `(delta, delta^2, delta^4, ...)`.
fn delta_powers<F: Field>(delta: F, t: usize) -> Vec<F> {
    let mut out = Vec::with_capacity(t);
    let mut d = delta;
    for _ in 0..t {
        out.push(d);
        d = d.square();
    }
    out
}

/// `F(X) = sum_i pow_i(beta + X delta) f_i(w_acc)`, all `t + 1` coefficients.
fn f_polynomial<F: iroha_pasta::PastaField>(
    relation: &FoldedRelation<F>,
    acc: View<'_, F>,
    beta: &[F],
    delta: &[F],
) -> Vec<F> {
    let pb = relation.constraint_bits;
    let inner: Vec<Vec<F>> = (0..(1usize << pb))
        .map(|p| {
            let mut poly = vec![F::ONE];
            for bit in 0..pb {
                if p >> bit & 1 == 1 {
                    poly = mul_linear(&poly, beta[bit], delta[bit]);
                }
            }
            poly
        })
        .collect();
    let mut rows: Vec<Vec<F>> = vec![Vec::new(); relation.n];
    constraints_at(relation, acc, |row, index, value| {
        if !bool::from(value.is_zero()) {
            let target = &mut rows[row];
            let p = &inner[index];
            if target.len() < p.len() {
                target.resize(p.len(), F::ZERO);
            }
            for (t, c) in target.iter_mut().zip(p) {
                *t += *c * value;
            }
        }
    });
    let mut stack: Vec<(usize, Vec<F>)> = Vec::new();
    for leaf in rows {
        let mut node = (0usize, leaf);
        while stack.last().is_some_and(|(level, _)| *level == node.0) {
            let (level, mut left) = stack.pop().expect("checked");
            if !node.1.is_empty() {
                let lifted = mul_linear(&node.1, beta[pb + level], delta[pb + level]);
                add_into(&mut left, &lifted);
            }
            node = (level + 1, left);
        }
        stack.push(node);
    }
    let mut f = stack.pop().map(|(_, p)| p).unwrap_or_default();
    f.resize(relation.index_bits() + 1, F::ZERO);
    f
}

/// Coefficients of the polynomial of degree `< xs.len()` through `(xs, ys)`.
fn interpolate<F: Field>(xs: &[F], ys: &[F]) -> Vec<F> {
    let mut out = vec![F::ZERO; xs.len()];
    for (i, (xi, yi)) in xs.iter().zip(ys).enumerate() {
        let mut basis = vec![F::ONE];
        let mut denominator = F::ONE;
        for (j, xj) in xs.iter().enumerate() {
            if i != j {
                basis = mul_linear(&basis, -*xj, F::ONE);
                denominator *= *xi - xj;
            }
        }
        let scale = *yi * denominator.invert().expect("distinct points");
        for (o, b) in out.iter_mut().zip(basis) {
            *o += b * scale;
        }
    }
    out
}

struct Challenges<F> {
    beta_star: Vec<F>,
    f_alpha: F,
    gamma: F,
}

fn fold_transcript<C: PastaCurve>(
    domain: &[u8; 32],
    acc: &Accumulator<C>,
    fresh: &Instance<C>,
) -> Transcript<C::ScalarExt> {
    let mut transcript = Transcript::<C::ScalarExt>::new(TAG_FOLD, domain);
    absorb_instance(&mut transcript, &acc.instance);
    transcript.absorb_scalars(&acc.beta);
    transcript.absorb_scalars(&[acc.e]);
    absorb_instance(&mut transcript, fresh);
    transcript
}

fn fold_instance<C: PastaCurve>(
    acc: &Accumulator<C>,
    fresh: &Instance<C>,
    proof: &FoldProof<C::ScalarExt>,
    challenges: &Challenges<C::ScalarExt>,
) -> Accumulator<C> {
    let gamma = challenges.gamma;
    let one = <C::ScalarExt as Field>::ONE;
    let fold_vec = |a: &[C::ScalarExt], b: &[C::ScalarExt]| -> Vec<C::ScalarExt> {
        a.iter()
            .zip(b)
            .map(|(x, y)| fold_scalar(x, y, &gamma))
            .collect()
    };
    let instance = Instance {
        commitments: acc
            .instance
            .commitments
            .iter()
            .zip(&fresh.commitments)
            .map(|(a, b)| fold_point::<C>(a, b, &gamma))
            .collect(),
        public: acc
            .instance
            .public
            .iter()
            .zip(&fresh.public)
            .map(|(a, b)| fold_vec(a, b))
            .collect(),
        theta: fold_vec(&acc.instance.theta, &fresh.theta),
        r: fold_vec(&acc.instance.r, &fresh.r),
    };
    let e = challenges.f_alpha * (one - gamma) + gamma * (gamma - one) * eval_poly(&proof.k, gamma);
    Accumulator {
        instance,
        beta: challenges.beta_star.clone(),
        e,
    }
}

/// Folds `fresh` into `acc`, returning the new accumulator, its witness and the
/// prover's messages.
///
/// The single-letter names follow the `ProtoGalaxy` notation (`t`, `d`, `f`, `g`, `k`).
///
/// # Errors
/// [`Error::Shape`] when the instances or witnesses do not match the relation;
/// [`Error::Rejected`] when `fresh` does not carry its own `LogUp` challenges.
#[allow(clippy::many_single_char_names)]
pub fn fold<C: PastaCurve>(
    relation: &FoldedRelation<C::ScalarExt>,
    domain: &[u8; 32],
    acc: &Accumulator<C>,
    acc_witness: &Witness<C::ScalarExt>,
    fresh: &Instance<C>,
    fresh_witness: &Witness<C::ScalarExt>,
) -> Result<FoldOutput<C>, Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let columns = relation.columns();
    let shape = |w: &Witness<S<C>>| {
        w.columns.len() == columns
            && w.blinds.len() == columns
            && w.columns.iter().all(|c| c.len() == relation.n)
    };
    if !shape(acc_witness) || !shape(fresh_witness) || !acc.instance.same_shape(fresh) {
        return Err(Error::Shape("fold inputs"));
    }
    if acc.beta.len() != relation.index_bits() {
        return Err(Error::Shape("accumulator beta"));
    }
    if !fresh.has_fresh_challenges(relation, domain) {
        return Err(Error::Rejected("fresh instance challenges"));
    }
    let mut transcript = fold_transcript(domain, acc, fresh);
    let t = relation.index_bits();
    let delta = delta_powers(transcript.challenge(), t);
    let acc_view = View::of(
        acc_witness,
        &acc.instance.public,
        &acc.instance.theta,
        &acc.instance.r,
    );
    let new_view = View::of(fresh_witness, &fresh.public, &fresh.theta, &fresh.r);
    let f = f_polynomial(relation, acc_view, &acc.beta, &delta);
    debug_assert_eq!(f[0], acc.e, "the accumulator must satisfy its relation");
    transcript.absorb_scalars(&f[1..]);
    let alpha = transcript.challenge();
    let mut f_with_e = f.clone();
    f_with_e[0] = acc.e;
    let f_alpha = eval_poly(&f_with_e, alpha);
    let beta_star: Vec<S<C>> = acc
        .beta
        .iter()
        .zip(&delta)
        .map(|(b, d)| *b + alpha * d)
        .collect();
    let pb = relation.constraint_bits;
    let pow_lo = pow_vector(&beta_star[..pb]);
    let pow_hi = pow_vector(&beta_star[pb..]);
    let d = relation.degree;
    let xs: Vec<S<C>> = (2..=d).map(|x| S::<C>::from(x as u64)).collect();
    let mut rows = vec![[S::<C>::ZERO; MAX_POINTS]; relation.n];
    constraints_on_line(relation, acc_view, new_view, &xs, |row, index, values| {
        let w = pow_lo[index];
        let slot = &mut rows[row];
        for i in 0..xs.len() {
            slot[i] += w * values[i];
        }
    });
    let mut g = vec![S::<C>::ZERO; xs.len()];
    for (row, values) in rows.iter().enumerate() {
        for i in 0..xs.len() {
            g[i] += pow_hi[row] * values[i];
        }
    }
    let one = S::<C>::ONE;
    let k_values: Vec<S<C>> = xs
        .iter()
        .zip(&g)
        .map(|(x, gx)| (*gx - f_alpha * (one - x)) * (*x * (*x - one)).invert().expect("x >= 2"))
        .collect();
    let k = interpolate(&xs, &k_values);
    transcript.absorb_scalars(&k);
    let gamma = transcript.challenge_128();
    let proof = FoldProof {
        f: f[1..].to_vec(),
        k,
    };
    let challenges = Challenges {
        beta_star,
        f_alpha,
        gamma,
    };
    let folded = fold_instance(acc, fresh, &proof, &challenges);
    let witness = Witness {
        columns: acc_witness
            .columns
            .iter()
            .zip(&fresh_witness.columns)
            .map(|(a, b)| {
                a.iter()
                    .zip(b)
                    .map(|(x, y)| fold_scalar(x, y, &gamma))
                    .collect()
            })
            .collect(),
        blinds: acc_witness
            .blinds
            .iter()
            .zip(&fresh_witness.blinds)
            .map(|(a, b)| fold_scalar(a, b, &gamma))
            .collect(),
    };
    Ok((folded, witness, proof))
}

/// The native fold verifier: recomputes every challenge (and the fresh instance's
/// `LogUp` challenges) and returns the folded accumulator.
///
/// # Errors
/// [`Error::Shape`] for mismatched shapes or proof lengths; [`Error::Rejected`]
/// when `fresh` does not carry its own `LogUp` challenges.
pub fn verify_fold<C: PastaCurve>(
    relation: &FoldedRelation<C::ScalarExt>,
    domain: &[u8; 32],
    acc: &Accumulator<C>,
    fresh: &Instance<C>,
    proof: &FoldProof<C::ScalarExt>,
) -> Result<Accumulator<C>, Error> {
    let t = relation.index_bits();
    if !acc.instance.same_shape(fresh) || acc.beta.len() != t {
        return Err(Error::Shape("fold inputs"));
    }
    if proof.f.len() != t || proof.k.len() != relation.degree - 1 {
        return Err(Error::Shape("fold proof"));
    }
    if !fresh.has_fresh_challenges(relation, domain) {
        return Err(Error::Rejected("fresh instance challenges"));
    }
    let mut transcript = fold_transcript(domain, acc, fresh);
    let delta = delta_powers(transcript.challenge(), t);
    transcript.absorb_scalars(&proof.f);
    let alpha = transcript.challenge();
    let mut f = Vec::with_capacity(t + 1);
    f.push(acc.e);
    f.extend_from_slice(&proof.f);
    let f_alpha = eval_poly(&f, alpha);
    let beta_star = acc
        .beta
        .iter()
        .zip(&delta)
        .map(|(b, d)| *b + alpha * d)
        .collect();
    transcript.absorb_scalars(&proof.k);
    let gamma = transcript.challenge_128();
    Ok(fold_instance(
        acc,
        fresh,
        proof,
        &Challenges {
            beta_star,
            f_alpha,
            gamma,
        },
    ))
}
