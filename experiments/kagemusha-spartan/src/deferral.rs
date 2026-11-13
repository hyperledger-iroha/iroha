//! Stage A2, step 8: deferred matrix claims.
//!
//! A verified proof's matrix evaluation is not computed in-circuit. The
//! verifier's final check uses one combination of the three matrices at the
//! sumcheck point, `P(r, r_x, r_y) = r A(r_x, r_y) + r^2 B(r_x, r_y) +
//! r^3 C(r_x, r_y)` with the proof's own challenge `r` (the lean schedule's
//! `r_A, r_B, r_C`), so that value is the claim, and nothing else about the
//! three matrices is. Claims about the circuit kinds a circuit verifies live
//! on one polynomial; with two kinds,
//!
//! `P(c, rho, x, y) = (1 - c) P_0(rho, x, y) + c P_1(rho, x, y)`,
//! `P_k = pad_k(x, y) (rho A_k + rho^2 B_k + rho^3 C_k)`,
//!
//! where `pad_k` zeroes the variables kind `k` does not have. With three
//! kinds (Receive, Send, Bootstrap) the selector has two bits, `c0` (first
//! coordinate) and `c1` (last): kind `k` sits at the corner
//! `(k & 1, k >> 1)` and `P = sum_k eq((c0, c1), corner_k) P_k`. The fourth
//! corner `(1, 1)` selects no kind, so `P` vanishes there whatever the other
//! coordinates: that is the empty claim a chain starts from
//! (`MatPoly::empty_claim`). Every proof
//! carries one accumulated claim on it; a native verifier checks that claim
//! with one matrix evaluation per kind (the compiled program, 0.2 ms on the
//! Mac for a uniform circuit).
//!
//! A circuit folds four claims (the two verified proofs' accumulators and
//! their two fresh claims) into its own accumulator in one step: the cubic
//! curve `gamma` through the four points at `t = 0..3`, `q(t) = P(gamma(t))`
//! committed by its coefficients in the first phase, `q(t) = v_t` checked at
//! `t = 0..3`, and the new claim `(gamma(beta), q(beta))` at the second-phase
//! challenge `beta` (the LogUp challenge). If an input claim is false, the
//! committed `q` differs from `P(gamma(t))`, and `q(beta)` is false except
//! with probability `deg q / |F|`, `deg q = 3 (b + X + Y) + 9` with `b`
//! selector bits (plus the challenges' degrees for uniform kinds). The fresh
//! claims' points are fixed by the verified proof's transcript before the
//! first phase; so is `r`, which ties the claim to the value the check uses.

use ff::PrimeField;

use crate::fs::Scalar;
use crate::full::Wiring;
use crate::uniform::{Array, Factor, Lin, Src, Term, constant, link, own, plus, scaled};

/// One circuit kind: its constraint and `z` variable counts, its matrices'
/// degrees in the LogUp challenges `(alpha, beta)` (a uniform circuit's LogUp
/// rows carry `alpha - t` and powers of beta; a toy has none), and its matrix
/// evaluation `(A, B, C)` at `(alpha, beta; r_x, r_y)`.
pub struct Kind<F> {
    pub sx: usize,
    pub sz: usize,
    pub deg: (usize, usize),
    #[allow(clippy::type_complexity)]
    pub eval: Box<dyn Fn(&[F], &[F], &[F]) -> (F, F, F) + Send + Sync>,
    /// The evaluation at many points `(alpha, beta; r_x, r_y)` at once, for
    /// a kind with a batched form (a uniform circuit's compiled program over
    /// lanes); `None` runs `eval` per point, in parallel.
    #[allow(clippy::type_complexity)]
    pub eval_many: Option<Box<dyn Fn(&[(&[F], &[F], &[F])]) -> Vec<(F, F, F)> + Send + Sync>>,
}

/// The combined matrix polynomial of two or three circuit kinds, over
/// `(c0, rho, alpha, beta, x, y)`, with a second selector bit `c1` after `y`
/// when there are three kinds.
pub struct MatPoly<F> {
    pub kinds: Vec<Kind<F>>,
    pub x: usize,
    pub y: usize,
}

/// Kind `kind`'s selector corner `(c0, c1)`: Receive (0, 0), Send (1, 0),
/// Bootstrap (0, 1). The corner (1, 1) is no kind's (the empty claim's).
pub fn kind_bits(kind: usize) -> (u64, u64) {
    assert!(kind < 4, "four kinds at most");
    ((kind & 1) as u64, (kind >> 1) as u64)
}

/// A claim `P(point) = value`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim<F> {
    pub point: Vec<F>,
    pub value: F,
}

impl<F: PrimeField> MatPoly<F> {
    pub fn new<const K: usize>(kinds: [Kind<F>; K]) -> Self {
        Self::with_min(kinds, 0, 0)
    }
    /// With at least `x_min` and `y_min` matrix variables (so that the
    /// accumulators of consecutive hops share one dimension).
    pub fn with_min<const K: usize>(kinds: [Kind<F>; K], x_min: usize, y_min: usize) -> Self {
        Self::with_min_vec(kinds.into(), x_min, y_min)
    }

    pub fn with_min_vec(kinds: Vec<Kind<F>>, x_min: usize, y_min: usize) -> Self {
        assert!((2..=4).contains(&kinds.len()), "two to four kinds");
        let x = kinds.iter().map(|k| k.sx).max().unwrap().max(x_min);
        let y = kinds.iter().map(|k| k.sz).max().unwrap().max(y_min);
        MatPoly { kinds, x, y }
    }
    /// The selector's bits: `c0`, and `c1` with three kinds.
    pub fn selector_bits(&self) -> usize {
        if self.kinds.len() > 2 { 2 } else { 1 }
    }
    /// Variables: c0, rho, alpha, beta, then x and y, then c1 (three kinds).
    pub fn n(&self) -> usize {
        3 + self.selector_bits() + self.x + self.y
    }
    /// The coordinate of the second selector bit `c1` (three kinds).
    pub fn c1_at(&self) -> Option<usize> {
        (self.kinds.len() > 2).then_some(4 + self.x + self.y)
    }
    /// The degree of `P` along a cubic curve: 3 per multilinear variable
    /// (the selector's bits among them), 9 for rho (cubic), and 3 times the
    /// challenges' degrees.
    pub fn degree(&self) -> usize {
        let da = self.kinds.iter().map(|k| k.deg.0).max().unwrap();
        let db = self.kinds.iter().map(|k| k.deg.1).max().unwrap();
        3 * (self.selector_bits() + self.x + self.y) + 9 + 3 * da + 3 * db
    }
    /// The x and y coordinates of `point`.
    fn xy<'p>(&self, point: &'p [F]) -> (&'p [F], &'p [F]) {
        let xs = &point[4..4 + self.x];
        let ys = &point[4 + self.x..4 + self.x + self.y];
        (xs, ys)
    }
    /// Kind `k`'s inputs at `point`: its challenges, `r_x` and `r_y`.
    fn kind_inputs<'p>(&self, k: usize, point: &'p [F]) -> (&'p [F], &'p [F], &'p [F]) {
        assert_eq!(point.len(), self.n(), "a point of the combined space");
        let (xs, ys) = self.xy(point);
        (
            &point[2..4],
            &xs[..self.kinds[k].sx],
            &ys[..self.kinds[k].sz],
        )
    }
    /// Kind `k`'s selector at `point`: `eq((c0, c1), corner_k)`, with
    /// `c1 = 0` when there are two kinds.
    fn selector(&self, k: usize, point: &[F]) -> F {
        let (b0, b1) = kind_bits(k);
        let c1 = self.c1_at().map_or(F::ZERO, |j| point[j]);
        let at = |c: F, b: u64| if b == 1 { c } else { F::ONE - c };
        at(point[0], b0) * at(c1, b1)
    }
    /// Kind `k`'s term at `point` from its matrices' values there:
    /// `sel_k(c) pad_k (rho A + rho^2 B + rho^3 C)`.
    fn kind_term(&self, k: usize, point: &[F], (a, b, cc): (F, F, F)) -> F {
        let kind = &self.kinds[k];
        let rho = point[1];
        let (xs, ys) = self.xy(point);
        let sel = self.selector(k, point);
        let pad = xs[kind.sx..]
            .iter()
            .chain(&ys[kind.sz..])
            .fold(F::ONE, |acc, v| acc * (F::ONE - v));
        let rho2 = rho.square();
        sel * pad * (rho * a + rho2 * b + rho2 * rho * cc)
    }
    /// The empty claim: selector `(1, 1)` with rho zero. With three kinds
    /// the corner selects no kind; with four kinds rho zero still makes
    /// every `rho A + rho^2 B + rho^3 C` exactly zero. A
    /// Bootstrap proof's accumulator: a chain starts with nothing deferred.
    pub fn empty_claim(&self) -> Claim<F> {
        let c1 = self
            .c1_at()
            .expect("the empty corner needs three kinds' two selector bits");
        let mut point = vec![F::ZERO; self.n()];
        point[0] = F::ONE;
        point[c1] = F::ONE;
        Claim {
            point,
            value: F::ZERO,
        }
    }
    pub fn eval(&self, point: &[F]) -> F {
        (0..self.kinds.len())
            .map(|k| {
                let (ch, rx, ry) = self.kind_inputs(k, point);
                self.kind_term(k, point, (self.kinds[k].eval)(ch, rx, ry))
            })
            .sum()
    }
    /// The degree of kind `k`'s matrices along a cubic curve: 3 per variable
    /// it has and 3 per degree in the challenges (no selector, `rho` or
    /// padding, which `degree` adds).
    pub fn kind_degree(&self, k: usize) -> usize {
        let kind = &self.kinds[k];
        3 * (kind.sx + kind.sz + kind.deg.0 + kind.deg.1)
    }
    /// Kind `k`'s `(A, B, C)` at many points at once.
    fn kind_values(&self, k: usize, points: &[Vec<F>]) -> Vec<(F, F, F)> {
        use rayon::prelude::*;
        let kind = &self.kinds[k];
        let inputs: Vec<(&[F], &[F], &[F])> =
            points.iter().map(|p| self.kind_inputs(k, p)).collect();
        let abc: Vec<(F, F, F)> = match &kind.eval_many {
            Some(many) => many(&inputs),
            None => inputs
                .par_iter()
                .map(|(ch, rx, ry)| (kind.eval)(ch, rx, ry))
                .collect(),
        };
        assert_eq!(abc.len(), points.len(), "one evaluation per point");
        abc
    }
    /// `eval` at many points: each kind's matrices over all of them at once.
    pub fn eval_many(&self, points: &[Vec<F>]) -> Vec<F> {
        let mut total = vec![F::ZERO; points.len()];
        for k in 0..self.kinds.len() {
            for ((t, p), v) in total
                .iter_mut()
                .zip(points)
                .zip(self.kind_values(k, points))
            {
                *t += self.kind_term(k, p, v);
            }
        }
        total
    }
    /// A verified proof's fresh point: the kind's selector bits (c0 first,
    /// c1 last), r, its second-phase challenges (zeros for a proof without
    /// any), its r_x and r_y padded with zeros.
    pub fn fresh_point(&self, kind: usize, r: F, ch: &[F], rx: &[F], ry: &[F]) -> Vec<F> {
        assert!(kind < self.kinds.len(), "a kind of the polynomial");
        assert_eq!(
            (rx.len(), ry.len()),
            (self.kinds[kind].sx, self.kinds[kind].sz),
            "the kind's variables"
        );
        let (b0, b1) = kind_bits(kind);
        let mut p = vec![
            F::from(b0),
            r,
            ch.first().copied().unwrap_or(F::ZERO),
            ch.get(1).copied().unwrap_or(F::ZERO),
        ];
        p.extend_from_slice(rx);
        p.resize(4 + self.x, F::ZERO);
        p.extend_from_slice(ry);
        p.resize(4 + self.x + self.y, F::ZERO);
        if self.c1_at().is_some() {
            p.push(F::from(b1));
        }
        assert_eq!(p.len(), self.n(), "a point of the combined space");
        p
    }
    /// Which coordinates of kind `kind`'s fresh points are constants: the
    /// selector bits, the challenges of a kind without them, and the padding
    /// (`Some(value)`), against those its transcript sets.
    pub fn fixed_coords(&self, kind: usize) -> Vec<Option<F>> {
        let k = &self.kinds[kind];
        let no_ch = k.deg == (0, 0);
        let (b0, b1) = kind_bits(kind);
        (0..self.n())
            .map(|j| {
                if j == 0 {
                    Some(F::from(b0))
                } else if Some(j) == self.c1_at() {
                    Some(F::from(b1))
                } else if j == 1 {
                    None
                } else if j < 4 {
                    no_ch.then_some(F::ZERO)
                } else if j < 4 + self.x {
                    (j - 4 >= k.sx).then_some(F::ZERO)
                } else {
                    (j - 4 - self.x >= k.sz).then_some(F::ZERO)
                }
            })
            .collect()
    }
}

/// The Lagrange basis on the nodes 0, 1, 2, 3 by monomial coefficients:
/// `lagrange4()[i][e]` is the coefficient of `t^e` in `L_i`.
pub fn lagrange4<F: PrimeField>() -> [[F; 4]; 4] {
    let mut out = [[F::ZERO; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        let mut poly = vec![F::ONE];
        let mut denom = F::ONE;
        for j in (0..4).filter(|j| *j != i) {
            // poly *= (t - j)
            let mut next = vec![F::ZERO; poly.len() + 1];
            for (e, c) in poly.iter().enumerate() {
                next[e + 1] += *c;
                next[e] -= *c * F::from(j as u64);
            }
            poly = next;
            denom *= F::from(i as u64) - F::from(j as u64);
        }
        let inv = denom.invert().unwrap();
        for (e, c) in poly.iter().enumerate() {
            row[e] = *c * inv;
        }
    }
    out
}

/// `sum_e coefs[e] t^e`.
pub fn horner<F: PrimeField>(coefs: &[F], t: F) -> F {
    coefs.iter().rev().fold(F::ZERO, |acc, c| acc * t + c)
}

/// The curve through the four claims' points at `t`.
pub fn curve<F: PrimeField>(claims: &[Claim<F>; 4], t: F) -> Vec<F> {
    curve_with(&lagrange4::<F>(), claims, t)
}

/// `curve` with the Lagrange basis given.
fn curve_with<F: PrimeField>(l: &[[F; 4]; 4], claims: &[Claim<F>; 4], t: F) -> Vec<F> {
    let w: Vec<F> = (0..4).map(|i| horner(&l[i], t)).collect();
    (0..claims[0].point.len())
        .map(|j| (0..4).map(|i| w[i] * claims[i].point[j]).sum())
        .collect()
}

/// `1 / 1, ..., 1 / (n - 1)` (and 1 for index 0) with one inversion.
fn small_inverses<F: PrimeField>(n: usize) -> Vec<F> {
    let mut prefix = vec![F::ONE; n];
    for j in 2..n {
        prefix[j] = prefix[j - 1] * F::from(j as u64);
    }
    let mut inv = vec![F::ONE; n];
    if n > 1 {
        // acc = 1 / (n - 1)!, then peel one factor per step down.
        let mut acc = prefix[n - 1].invert().unwrap();
        for j in (2..n).rev() {
            inv[j] = acc * prefix[j - 1];
            acc *= F::from(j as u64);
        }
    }
    inv
}

/// The coefficients of the polynomial through `values` at the nodes
/// `0..values.len()`.
pub fn interpolate<F: PrimeField>(values: &[F]) -> Vec<F> {
    let n = values.len();
    let invs: Vec<F> = small_inverses(n);
    // Newton's divided differences on integer nodes.
    let mut dd = values.to_vec();
    for j in 1..n {
        for i in (j..n).rev() {
            dd[i] = (dd[i] - dd[i - 1]) * invs[j];
        }
    }
    // sum_k dd[k] prod_{m<k} (t - m), by Horner from the top.
    let mut poly = vec![F::ZERO; n];
    for k in (0..n).rev() {
        let kk = F::from(k as u64);
        for i in (1..n).rev() {
            poly[i] = poly[i - 1] - poly[i] * kk;
        }
        poly[0] = -poly[0] * kk + dd[k];
    }
    poly
}

/// The values at the nodes `m..n` of the polynomials of degree below `m`
/// with values `vals` at `0..m` (three at once), from the Lagrange weights
/// of equispaced nodes: `L_j(x) = N(x) / ((x - j) j! (m - 1 - j)! (-1)^(m - 1 - j))`,
/// `N(x) = prod_k (x - k)`; `inv` holds `1 / i` for `i < n`.
fn extend<F: PrimeField>(vals: &[(F, F, F)], n: usize, inv: &[F]) -> Vec<(F, F, F)> {
    let m = vals.len();
    if m >= n {
        return vals[..n].to_vec();
    }
    let mut inv_fact = vec![F::ONE; m];
    for i in 1..m {
        inv_fact[i] = inv_fact[i - 1] * inv[i];
    }
    let mut out = vals.to_vec();
    for x in m..n {
        let nx: F = (0..m).map(|k| F::from((x - k) as u64)).product();
        let mut acc = (F::ZERO, F::ZERO, F::ZERO);
        for (j, (a, b, c)) in vals.iter().enumerate() {
            let w = nx * inv[x - j] * inv_fact[j] * inv_fact[m - 1 - j];
            let w = if (m - 1 - j) % 2 == 1 { -w } else { w };
            acc = (acc.0 + w * a, acc.1 + w * b, acc.2 + w * c);
        }
        out.push(acc);
    }
    out
}

/// The honest `q = P(gamma(t))` by its `deg q + 1` coefficients, from the
/// curve's points at `t = 0..=deg q` evaluated together: each kind's
/// matrices only at `kind_degree + 1` of them (their degree along the
/// curve; no selector, `rho` or padding), extended to the rest.
pub fn restrict<F: PrimeField>(poly: &MatPoly<F>, claims: &[Claim<F>; 4]) -> Vec<F> {
    let d = poly.degree();
    let l = lagrange4::<F>();
    let points: Vec<Vec<F>> = (0..=d)
        .map(|t| curve_with(&l, claims, F::from(t as u64)))
        .collect();
    let inv = small_inverses::<F>(d + 1);
    let mut values = vec![F::ZERO; d + 1];
    for k in 0..poly.kinds.len() {
        let m = (poly.kind_degree(k) + 1).min(d + 1);
        let abc = extend(&poly.kind_values(k, &points[..m]), d + 1, &inv);
        for ((v, p), e) in values.iter_mut().zip(&points).zip(abc) {
            *v += poly.kind_term(k, p, e);
        }
    }
    interpolate(&values)
}

/// `restrict` one point at a time (the reference for the batched form).
pub fn restrict_pointwise<F: PrimeField>(poly: &MatPoly<F>, claims: &[Claim<F>; 4]) -> Vec<F> {
    let values: Vec<F> = (0..=poly.degree())
        .map(|t| poly.eval(&curve(claims, F::from(t as u64))))
        .collect();
    interpolate(&values)
}

/// The folded claim at challenge `beta`.
pub fn fold_output<F: PrimeField>(claims: &[Claim<F>; 4], q: &[F], beta: F) -> Claim<F> {
    Claim {
        point: curve(claims, beta),
        value: horner(q, beta),
    }
}

/// Where the fold's cells sit.
#[derive(Clone, Copy, Debug)]
pub struct FoldArrays {
    /// Instance j per coordinate: the four claims' coordinate j, then the
    /// folded point's (second phase).
    pub coords: usize,
    /// Instance e per coefficient of q: the coefficient, the Horner sums at
    /// 2 and 3, and at beta (second phase).
    pub q: usize,
    /// One instance: the four claims' values.
    pub values: usize,
    pub n: usize,
    pub degree: usize,
}

pub const C_P: [usize; 4] = [0, 1, 2, 3];
pub const C_G: usize = 4;
pub const Q_C: usize = 0;
pub const Q_H2: usize = 1;
pub const Q_H3: usize = 2;
pub const Q_HT: usize = 3;
pub const V_V: [usize; 4] = [0, 1, 2, 3];

fn bits_for(n: usize) -> usize {
    n.next_power_of_two().trailing_zeros() as usize
}

fn row_zero<F: PrimeField>(lin: Lin<F>, n: usize) -> [Lin<F>; 3] {
    [lin, constant(vec![F::ONE; n]), Vec::new()]
}

/// The fold of four claims in family `fam` (one instance, `p` range 1): the
/// arrays, their first-phase rows (the four values against q at 0..3), the
/// challenge rows (the folded point and q(beta)) and the fill of the
/// second-phase cells. The claims' coordinates and values are cells; the
/// caller ties them to their sources (`fix_coords` for constant ones).
pub fn fold_arrays<F: Scalar>(
    w: &mut Wiring<F>,
    fam: usize,
    claims: &[Claim<F>; 4],
    q: &[F],
) -> FoldArrays {
    let n = claims[0].point.len();
    let degree = q.len() - 1;
    let one = F::ONE;
    let zero = F::ZERO;
    for (t, cl) in claims.iter().enumerate() {
        assert_eq!(cl.point.len(), n, "one dimension");
        assert!(
            horner(q, F::from(t as u64)) == cl.value,
            "q passes through claim {t}"
        );
    }
    // Coordinates.
    let rc = bits_for(n);
    let nc = 1usize << rc;
    let mut cv = vec![vec![zero; nc]; 5];
    for (i, cl) in claims.iter().enumerate() {
        for (j, v) in cl.point.iter().enumerate() {
            cv[C_P[i]][j] = *v;
        }
    }
    let coords = w.push(Array {
        name: "defer coords",
        family: fam,
        rho: rc,
        r_count: n,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 5],
        rows: Vec::new(),
        values: cv,
    });
    // Coefficients of q and Horner sums at 2 and 3.
    let rq = bits_for(degree + 1);
    let nq = 1usize << rq;
    let mut qv = vec![vec![zero; nq]; 4];
    let (mut h2, mut h3) = (zero, zero);
    for e in (0..=degree).rev() {
        h2 = h2 * F::from(2) + q[e];
        h3 = h3 * F::from(3) + q[e];
        qv[Q_C][e] = q[e];
        qv[Q_H2][e] = h2;
        qv[Q_H3][e] = h3;
    }
    let up: Vec<Option<usize>> = (0..nq).map(|e| (e < degree).then_some(e + 1)).collect();
    let qa = w.arrays.len();
    let horner_row = |cell: usize, k: u64| -> [Lin<F>; 3] {
        let lin = plus(
            &plus(&own(cell, nq), &scaled(&own(Q_C, nq), -one)),
            &link(qa, cell, up.clone(), -F::from(k)),
        );
        row_zero(lin, nq)
    };
    let q_rows = vec![horner_row(Q_H2, 2), horner_row(Q_H3, 3)];
    let qa2 = w.push(Array {
        name: "defer q",
        family: fam,
        rho: rq,
        r_count: degree + 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 4],
        rows: q_rows,
        values: qv,
    });
    assert_eq!(qa, qa2);
    // Values: q(0) = C_0, q(1) = sum of C, q(2) = H2_0, q(3) = H3_0.
    let at0 = vec![Some(0)];
    let vrows = vec![
        row_zero(plus(&own(V_V[0], 1), &link(qa, Q_C, at0.clone(), -one)), 1),
        row_zero(
            plus(
                &own(V_V[1], 1),
                &vec![Term {
                    src: Src::All {
                        array: qa,
                        cell: Q_C,
                    },
                    coef: vec![-one],
                }],
            ),
            1,
        ),
        row_zero(plus(&own(V_V[2], 1), &link(qa, Q_H2, at0.clone(), -one)), 1),
        row_zero(plus(&own(V_V[3], 1), &link(qa, Q_H3, at0, -one)), 1),
    ];
    let vv: Vec<Vec<F>> = claims.iter().map(|cl| vec![cl.value]).collect();
    let values = w.push(Array {
        name: "defer values",
        family: fam,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 4],
        rows: vrows,
        values: vv,
    });
    // Challenge rows: G = sum_i L_i(beta) P_i; HT_e = C_e + beta HT_(e+1).
    // NegBeta(e) is -beta^e.
    let l = lagrange4::<F>();
    let term = |src: Src, c: F, len: usize| Term {
        src,
        coef: vec![c; len],
    };
    let mut g_a: Vec<(Term<F>, Factor)> = Vec::new();
    for (i, li) in l.iter().enumerate() {
        for (e, c) in li.iter().enumerate() {
            if bool::from(c.is_zero()) {
                continue;
            }
            if e == 0 {
                g_a.push((term(Src::Own(C_P[i]), *c, nc), Factor::One));
            } else {
                g_a.push((term(Src::Own(C_P[i]), -*c, nc), Factor::NegBeta(e)));
            }
        }
    }
    let g_row = [
        g_a,
        vec![(term(Src::One, one, nc), Factor::One)],
        vec![(term(Src::Own(C_G), one, nc), Factor::One)],
    ];
    let ht_row = [
        vec![
            (term(Src::Own(Q_HT), one, nq), Factor::One),
            (term(Src::Own(Q_C), -one, nq), Factor::One),
            (
                Term {
                    src: Src::Link {
                        array: qa,
                        cell: Q_HT,
                        map: up,
                    },
                    coef: vec![one; nq],
                },
                Factor::NegBeta(1),
            ),
        ],
        vec![(term(Src::One, one, nq), Factor::One)],
        Vec::new(),
    ];
    w.challenge.rows.push((coords, g_row));
    w.challenge.rows.push((qa, ht_row));
    w.challenge.cells.push((coords, C_G));
    w.challenge.cells.push((qa, Q_HT));
    assert!(w.challenge.fill.is_none(), "one fold per circuit");
    let (points, qc) = (
        claims.iter().map(|c| c.point.clone()).collect::<Vec<_>>(),
        q.to_vec(),
    );
    w.challenge.fill = Some(Box::new(move |ch: &[F]| {
        let beta = ch[1];
        let lw: Vec<F> = (0..4).map(|i| horner(&l[i], beta)).collect();
        let mut out: Vec<(usize, usize, usize, F)> = (0..n)
            .map(|j| (coords, C_G, j, (0..4).map(|i| lw[i] * points[i][j]).sum()))
            .collect();
        let mut h = F::ZERO;
        for e in (0..=degree).rev() {
            h = h * beta + qc[e];
            out.push((qa, Q_HT, e, h));
        }
        out
    }));
    FoldArrays {
        coords,
        q: qa,
        values,
        n,
        degree,
    }
}

/// Ties claim `node`'s constant coordinates (`Some`) to their values: one row
/// over the coordinates array, `fixed_j (P_node - value_j) = 0`.
pub fn fix_coords<F: Scalar>(
    w: &mut Wiring<F>,
    fold: &FoldArrays,
    node: usize,
    fixed: &[Option<F>],
) {
    let nc = 1usize << w.arrays[fold.coords].rho;
    let on: Vec<F> = (0..nc)
        .map(|j| {
            if fixed.get(j).is_some_and(|f| f.is_some()) {
                F::ONE
            } else {
                F::ZERO
            }
        })
        .collect();
    let val: Vec<F> = (0..nc)
        .map(|j| -fixed.get(j).copied().flatten().unwrap_or(F::ZERO))
        .collect();
    let lin = plus(
        &vec![Term {
            src: Src::Own(C_P[node]),
            coef: on,
        }],
        &vec![Term {
            src: Src::One,
            coef: val,
        }],
    );
    w.arrays[fold.coords].rows.push(row_zero(lin, nc));
}

/// Exposes the folded claim as late io: a one-cell io array at io indices
/// `offset..` holds the folded point, then its value; rows tie it to the
/// fold's second-phase cells, and the prover fills it after the challenges
/// (the transcript absorbs its digest before the second-phase rows).
/// Returns the io array.
pub fn expose<F: Scalar>(w: &mut Wiring<F>, fam: usize, fold: &FoldArrays, offset: usize) -> usize {
    let n = fold.n;
    let rl = bits_for(n + 1);
    assert_eq!(offset % (1usize << rl), 0, "late io aligned");
    let one = F::ONE;
    let late = w.push(Array {
        name: "defer late io",
        family: fam,
        rho: rl,
        r_count: n + 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: Vec::new(),
        values: vec![vec![F::ZERO; 1usize << rl]],
    });
    let nc = 1usize << w.arrays[fold.coords].rho;
    let to_late: Vec<Option<usize>> = (0..nc).map(|j| (j < n).then_some(j)).collect();
    let on: Vec<F> = (0..nc).map(|j| if j < n { one } else { F::ZERO }).collect();
    let row = plus(
        &vec![Term {
            src: Src::Own(C_G),
            coef: on,
        }],
        &link(late, 0, to_late, -one),
    );
    w.arrays[fold.coords].rows.push(row_zero(row, nc));
    let vrow = plus(
        &link(fold.q, Q_HT, vec![Some(0)], one),
        &link(late, 0, vec![Some(n)], -one),
    );
    w.arrays[fold.values].rows.push(row_zero(vrow, 1));
    w.io_arrays.push((late, offset));
    for j in 0..n {
        w.late_io.push((offset + j, fold.coords, C_G, j));
    }
    w.late_io.push((offset + n, fold.q, Q_HT, 0));
    late
}

/// Exposes a constant claim as late io where `expose` puts a folded one: a
/// circuit that folds nothing (Bootstrap, which verifies no proof) still
/// carries an accumulator where every verifier slot reads one. The io array
/// is `expose`'s; a one-cell array of `n + 1` instances holds the claim's
/// coordinates, then its value, each pinned to its constant and tied to its
/// io slot. Returns (the io array, the pinned array).
pub fn expose_constant<F: Scalar>(
    w: &mut Wiring<F>,
    fam: usize,
    claim: &Claim<F>,
    offset: usize,
) -> (usize, usize) {
    let n = claim.point.len();
    let rl = bits_for(n + 1);
    let nn = 1usize << rl;
    assert_eq!(offset % nn, 0, "late io aligned");
    let one = F::ONE;
    let late = w.push(Array {
        name: "constant late io",
        family: fam,
        rho: rl,
        r_count: n + 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: Vec::new(),
        values: vec![vec![F::ZERO; nn]],
    });
    let mut vals = claim.point.clone();
    vals.push(claim.value);
    vals.resize(nn, F::ZERO);
    let on: Vec<F> = (0..nn)
        .map(|j| if j <= n { one } else { F::ZERO })
        .collect();
    let own_on = vec![Term {
        src: Src::Own(0),
        coef: on,
    }];
    let pin = plus(&own_on, &constant(vals.iter().map(|v| -*v).collect()));
    let tie = plus(
        &own_on,
        &link(
            late,
            0,
            (0..nn).map(|j| (j <= n).then_some(j)).collect(),
            -one,
        ),
    );
    let held = w.push(Array {
        name: "constant claim",
        family: fam,
        rho: rl,
        r_count: n + 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: vec![row_zero(pin, nn), row_zero(tie, nn)],
        values: vec![vals],
    });
    w.io_arrays.push((late, offset));
    for j in 0..=n {
        w.late_io.push((offset + j, held, 0, j));
    }
    (late, held)
}

/// A proof's late io as a claim (point, then value).
pub fn claim_of<F: Clone>(late: &[F]) -> Claim<F> {
    let (point, value) = late.split_at(late.len() - 1);
    Claim {
        point: point.to_vec(),
        value: value[0].clone(),
    }
}

/// The folded claim in a witness filled for challenges `ch`.
pub fn read_output<F: Scalar>(
    c: &crate::uniform::UniformCircuit<F>,
    fold: &FoldArrays,
    w: &[F],
) -> Claim<F> {
    Claim {
        point: (0..fold.n)
            .map(|j| w[c.cell_index(fold.coords, C_G, j)])
            .collect(),
        value: w[c.cell_index(fold.q, Q_HT, 0)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::r1cs::{Circuit, Instance, Shape, Witness};
    use crate::uniform::UniformCircuit;
    use ff::Field;
    use ff::PrimeField as _;
    use iroha_pasta::{Eq as Vesta, Fp};

    fn toy_poly() -> MatPoly<Fp> {
        let kind = |sx: usize, row_bits: usize, col_bits: usize| {
            let shape = Shape {
                label: "toy",
                sx,
                row_bits,
                col_bits,
                k: 3,
            };
            Kind {
                sx,
                sz: shape.sz(),
                deg: (0, 0),
                eval: Box::new(move |_: &[Fp], rx: &[Fp], ry: &[Fp]| {
                    crate::r1cs::matrix_evals(&shape, rx, ry)
                }),
                eval_many: None,
            }
        };
        MatPoly::new([kind(9, 2, 6), kind(8, 2, 5)])
    }

    fn claims(poly: &MatPoly<Fp>, rng: &mut SpikeRng) -> [Claim<Fp>; 4] {
        let random = |rng: &mut SpikeRng| -> Claim<Fp> {
            let point: Vec<Fp> = (0..poly.n()).map(|_| rng.scalar()).collect();
            Claim {
                value: poly.eval(&point),
                point,
            }
        };
        let fresh = |rng: &mut SpikeRng, kind: usize| -> Claim<Fp> {
            let k = &poly.kinds[kind];
            let rx: Vec<Fp> = (0..k.sx).map(|_| rng.scalar()).collect();
            let ry: Vec<Fp> = (0..k.sz).map(|_| rng.scalar()).collect();
            let point = poly.fresh_point(kind, rng.scalar(), &[], &rx, &ry);
            Claim {
                value: poly.eval(&point),
                point,
            }
        };
        [random(rng), fresh(rng, 0), random(rng), fresh(rng, 1)]
    }

    fn build(
        poly: &MatPoly<Fp>,
        cl: &[Claim<Fp>; 4],
        q: &[Fp],
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, FoldArrays) {
        let mut w: Wiring<Fp> = Wiring::default();
        let fam = w.family(0, 1);
        let fold = fold_arrays(&mut w, fam, cl, q);
        fix_coords(&mut w, &fold, 1, &poly.fixed_coords(0));
        fix_coords(&mut w, &fold, 3, &poly.fixed_coords(1));
        expose(&mut w, fam, &fold, 32);
        let (arrays, pi, counts, tables, copies) = w.finish();
        let (c, inst, wit) = UniformCircuit::build_with_copies(
            "defer-test",
            arrays,
            pi,
            counts,
            tables,
            copies,
            10,
            4,
            b"defer",
        );
        (c, inst, wit, fold)
    }

    fn holds(c: &UniformCircuit<Fp>, inst: &Instance<Fp>, w: Vec<Fp>, ch: &[Fp]) -> bool {
        let shape = c.shape;
        let rel = c.instantiate(&shape, ch, None);
        // The late io as the honest prover sets it from w.
        let io = crate::spartan::io_with_late(&inst.io, &c.late_slots(), &c.late_values(&w));
        let z = crate::r1cs::z_vector(&shape, &Instance { io }, &Witness { w });
        let (az, bz, cz) = rel.abc(&shape, &z);
        (0..1usize << shape.sx).all(|x| az[x] * bz[x] == cz[x])
    }

    #[test]
    fn interpolation_and_curve() {
        let mut rng = SpikeRng::new(b"defer interp");
        let coefs: Vec<Fp> = (0..12).map(|_| rng.scalar()).collect();
        let values: Vec<Fp> = (0..12)
            .map(|t| horner(&coefs, Fp::from(t as u64)))
            .collect();
        assert_eq!(
            interpolate(&values),
            coefs,
            "interpolation inverts evaluation"
        );
        let poly = toy_poly();
        let cl = claims(&poly, &mut rng);
        for (t, c) in cl.iter().enumerate() {
            assert_eq!(
                curve(&cl, Fp::from(t as u64)),
                c.point,
                "the curve passes through point {t}"
            );
        }
        let q = restrict(&poly, &cl);
        let tau: Fp = rng.scalar();
        assert_eq!(
            horner(&q, tau),
            poly.eval(&curve(&cl, tau)),
            "q is P along the curve"
        );
        assert_eq!(
            q,
            restrict_pointwise(&poly, &cl),
            "the batched q is the pointwise q"
        );
        let inv = small_inverses::<Fp>(40);
        assert!(
            (1..40).all(|j| inv[j] * Fp::from(j as u64) == Fp::ONE),
            "small inverses"
        );
    }

    /// Three kinds: each kind's fresh point selects that kind alone (its
    /// corner of the two selector bits), the fourth corner holds the empty
    /// claim whatever the other coordinates, and a fold through the empty
    /// claim, fresh claims of every kind and a random claim is P along the
    /// curve.
    #[test]
    fn three_kinds_select_corners_and_the_empty_claim() {
        let mut rng = SpikeRng::new(b"defer three kinds");
        let kind = |sx: usize, row_bits: usize, col_bits: usize| {
            let shape = Shape {
                label: "toy",
                sx,
                row_bits,
                col_bits,
                k: 3,
            };
            Kind {
                sx,
                sz: shape.sz(),
                deg: (0, 0),
                eval: Box::new(move |_: &[Fp], rx: &[Fp], ry: &[Fp]| {
                    crate::r1cs::matrix_evals(&shape, rx, ry)
                }),
                eval_many: None,
            }
        };
        let poly = MatPoly::new([kind(9, 2, 6), kind(8, 2, 5), kind(7, 2, 5)]);
        let c1 = poly.c1_at().expect("three kinds have c1");
        assert_eq!(
            (poly.selector_bits(), poly.n(), c1),
            (2, 5 + poly.x + poly.y, poly.n() - 1),
            "c0 first, c1 last"
        );
        let mut fresh = Vec::new();
        for k in 0..3 {
            let kk = &poly.kinds[k];
            let rx: Vec<Fp> = (0..kk.sx).map(|_| rng.scalar()).collect();
            let ry: Vec<Fp> = (0..kk.sz).map(|_| rng.scalar()).collect();
            let r: Fp = rng.scalar();
            let point = poly.fresh_point(k, r, &[], &rx, &ry);
            let (a, b, c) = (kk.eval)(&[], &rx, &ry);
            let own = r * a + r.square() * b + r.square() * r * c;
            assert_eq!(poly.eval(&point), own, "kind {k}'s corner selects it alone");
            let fixed = poly.fixed_coords(k);
            let (b0, b1) = kind_bits(k);
            assert_eq!(
                (fixed[0], fixed[c1]),
                (Some(Fp::from(b0)), Some(Fp::from(b1))),
                "kind {k}'s selector bits are fixed coordinates"
            );
            assert!(
                fixed
                    .iter()
                    .zip(&point)
                    .all(|(f, v)| f.is_none_or(|f| f == *v)),
                "kind {k}'s fresh point has its fixed coordinates"
            );
            fresh.push(Claim { point, value: own });
        }
        let empty = poly.empty_claim();
        assert_eq!(
            poly.eval(&empty.point),
            empty.value,
            "the empty claim holds"
        );
        let mut corner: Vec<Fp> = (0..poly.n()).map(|_| rng.scalar()).collect();
        corner[0] = Fp::ONE;
        corner[c1] = Fp::ONE;
        assert_eq!(
            poly.eval(&corner),
            Fp::ZERO,
            "P vanishes on the corner (1, 1)"
        );
        let random: Vec<Fp> = (0..poly.n()).map(|_| rng.scalar()).collect();
        let cl = [
            empty,
            fresh[2].clone(),
            Claim {
                value: poly.eval(&random),
                point: random,
            },
            fresh[1].clone(),
        ];
        let q = restrict(&poly, &cl);
        assert_eq!(q.len(), poly.degree() + 1, "q has the degree bound");
        for (t, c) in cl.iter().enumerate() {
            assert_eq!(
                horner(&q, Fp::from(t as u64)),
                c.value,
                "q passes claim {t}"
            );
        }
        let tau: Fp = rng.scalar();
        assert_eq!(
            horner(&q, tau),
            poly.eval(&curve(&cl, tau)),
            "q is P along the curve"
        );
        assert_eq!(q, restrict_pointwise(&poly, &cl), "batched = pointwise");
    }

    fn four_kind_poly() -> MatPoly<Fp> {
        // Deliberately different dimensions and matrix coefficients ensure a
        // missing fourth selector cannot accidentally agree with another kind.
        let kind = |index: usize| {
            let sx = 5 + index;
            let sz = 4 + index % 2;
            Kind {
                sx,
                sz,
                deg: (1, 1),
                eval: Box::new(move |ch: &[Fp], rx: &[Fp], ry: &[Fp]| {
                    let x = rx.iter().copied().sum::<Fp>();
                    let y = ry.iter().copied().sum::<Fp>();
                    let tag = Fp::from((index + 1) as u64);
                    (
                        tag + x + ch[0] * y,
                        tag.double() + x * y,
                        tag * tag + ch[1] * x + y,
                    )
                }),
                eval_many: None,
            }
        };
        MatPoly::new([kind(0), kind(1), kind(2), kind(3)])
    }

    #[test]
    fn four_kinds_select_all_corners_and_empty_requires_zero_rho() {
        let poly = four_kind_poly();
        let mut rng = SpikeRng::new(b"four kind selector and empty regression");
        let c1 = poly.c1_at().unwrap();
        assert_eq!(poly.selector_bits(), 2);
        let mut fresh = Vec::new();
        for k in 0..4 {
            let kk = &poly.kinds[k];
            let rx: Vec<Fp> = (0..kk.sx).map(|_| rng.scalar()).collect();
            let ry: Vec<Fp> = (0..kk.sz).map(|_| rng.scalar()).collect();
            let ch = [rng.scalar(), rng.scalar()];
            let rho: Fp = rng.scalar();
            let point = poly.fresh_point(k, rho, &ch, &rx, &ry);
            let (a, b, c) = (kk.eval)(&ch, &rx, &ry);
            let value = rho * a + rho.square() * b + rho.square() * rho * c;
            assert_eq!(poly.eval(&point), value, "kind{k} selects its exact matrix");
            let (b0, b1) = kind_bits(k);
            assert_eq!((point[0], point[c1]), (Fp::from(b0), Fp::from(b1)));
            for (j, fixed) in poly.fixed_coords(k).into_iter().enumerate() {
                if let Some(v) = fixed {
                    assert_eq!(point[j], v, "kind{k} fixed coordinate{j}");
                }
            }
            fresh.push(Claim { point, value });
        }
        let empty = poly.empty_claim();
        assert_eq!(empty.point[1], Fp::ZERO);
        assert_eq!((empty.point[0], empty.point[c1]), (Fp::ONE, Fp::ONE));
        assert_eq!(poly.eval(&empty.point), Fp::ZERO);
        // (1,1) now selects a real Load matrix. Reusing the old assertion that
        // this corner is identically empty would waive all Load accumulation.
        let mut formerly_empty = empty.point.clone();
        formerly_empty[1] = Fp::ONE;
        assert_eq!(poly.eval(&formerly_empty), Fp::from(28));
        for _ in 0..8 {
            let mut point: Vec<Fp> = (0..poly.n()).map(|_| rng.scalar()).collect();
            point[1] = Fp::ZERO;
            assert_eq!(
                poly.eval(&point),
                Fp::ZERO,
                "rho0 remains empty even away from any selector corner"
            );
        }
        let claims: [Claim<Fp>; 4] = fresh.try_into().unwrap();
        let q = restrict(&poly, &claims);
        assert_eq!(q, restrict_pointwise(&poly, &claims));
        for _ in 0..4 {
            let beta: Fp = rng.scalar();
            assert_eq!(horner(&q, beta), poly.eval(&curve(&claims, beta)));
        }
        let mut with_bootstrap_empty = claims.clone();
        with_bootstrap_empty[2] = empty;
        let q = restrict(&poly, &with_bootstrap_empty);
        let beta: Fp = rng.scalar();
        let out = fold_output(&with_bootstrap_empty, &q, beta);
        assert_eq!(
            poly.eval(&out.point),
            out.value,
            "zero Bootstrap accumulator followed by real Load remains sound"
        );
    }

    #[test]
    fn four_kind_fold_actual_proof_matches_native_and_binds_load_selector() {
        let poly = four_kind_poly();
        let mut rng = SpikeRng::new(b"four kind actual folding proof");
        let claims: [Claim<Fp>; 4] = std::array::from_fn(|k| {
            let kk = &poly.kinds[k];
            let rx: Vec<Fp> = (0..kk.sx).map(|_| rng.scalar()).collect();
            let ry: Vec<Fp> = (0..kk.sz).map(|_| rng.scalar()).collect();
            let ch = [rng.scalar(), rng.scalar()];
            let point = poly.fresh_point(k, rng.scalar(), &ch, &rx, &ry);
            Claim {
                value: poly.eval(&point),
                point,
            }
        });
        let q = restrict(&poly, &claims);
        let mut wiring = Wiring::default();
        let family = wiring.family(0, 1);
        let fold = fold_arrays(&mut wiring, family, &claims, &q);
        for k in 0..4 {
            fix_coords(&mut wiring, &fold, k, &poly.fixed_coords(k));
        }
        expose(&mut wiring, family, &fold, 32);
        let (arrays, pi, counts, tables, copies) = wiring.finish();
        let (c, inst, wit) = UniformCircuit::build_with_copies(
            "four-kind-fold",
            arrays,
            pi,
            counts,
            tables,
            copies,
            10,
            4,
            b"four-kind-fold",
        );
        let ch = [rng.scalar(), rng.scalar()];
        let mut filled = wit.w.clone();
        let _ = c.instantiate(&c.shape, &ch, Some(&mut filled));
        assert!(holds(&c, &inst, filled.clone(), &ch));
        let output = read_output(&c, &fold, &filled);
        assert_eq!(output, fold_output(&claims, &q, ch[1]));
        assert_eq!(poly.eval(&output.point), output.value);
        // Both selector bits matter for the fourth kind under this same VK.
        for coordinate in [0, poly.c1_at().unwrap()] {
            let mut changed = filled.clone();
            changed[c.cell_index(fold.coords, C_P[3], coordinate)] = Fp::ZERO;
            assert!(
                !holds(&c, &inst, changed, &ch),
                "Load selector bit{coordinate} cannot be discarded"
            );
        }
        let gens = crate::ipa::Gens::<Vesta>::new(c.shape.cols());
        let (proof, _) = crate::spartan::prove_circuit(&c.shape, &gens, &inst, &wit, &mut rng, &c);
        crate::spartan::verify_circuit(&c.shape, &gens, &inst, &proof.to_bytes(), &c).unwrap();
        let accumulated = claim_of(&proof.late);
        assert_eq!(poly.eval(&accumulated.point), accumulated.value);
        let fixed = crate::spartan::fixed_commitments(&c.shape, &gens, &c);
        let trace = crate::verifier::trace_circuit(
            &c.shape,
            &gens,
            &inst,
            &proof,
            &[],
            &crate::spartan::Link::None,
            &c,
            &fixed,
        );
        assert!(
            trace.accepts,
            "native verifier trace agrees with real four-kind fold proof"
        );
        for late in [1, poly.c1_at().unwrap(), poly.n()] {
            let mut bytes = proof.to_bytes();
            let at = 32 * (c.shape.rows() - c.fixed_rows(&c.shape) + late);
            let value = Fp::from_repr(bytes[at..at + 32].try_into().unwrap()).unwrap() + Fp::ONE;
            bytes[at..at + 32].copy_from_slice(&value.to_repr());
            assert!(
                crate::spartan::verify_circuit(&c.shape, &gens, &inst, &bytes, &c).is_err(),
                "tampered rho/selector/value lateIO{late}"
            );
        }
    }

    /// A uniform kind's batched q: the fold circuit's own compiled program as
    /// kind 0 (with LogUp challenges), against one evaluation per point.
    #[test]
    fn batched_q_matches_pointwise() {
        let mut rng = SpikeRng::new(b"defer batched");
        let toy = toy_poly();
        let cl = claims(&toy, &mut rng);
        let q = restrict(&toy, &cl);
        let (c, ..) = build(&toy, &cl, &q);
        let c = std::sync::Arc::new(c);
        let (c1, c2) = (c.clone(), c.clone());
        let uniform = Kind {
            sx: c.shape.sx,
            sz: c.shape.sz(),
            deg: c.challenge_degrees(),
            eval: Box::new(move |ch: &[Fp], rx: &[Fp], ry: &[Fp]| c1.evals(rx, ry, ch)),
            eval_many: Some(Box::new(move |pts: &[(&[Fp], &[Fp], &[Fp])]| {
                c2.evals_many(pts)
            })),
        };
        let shape = Shape {
            label: "toy",
            sx: 8,
            row_bits: 2,
            col_bits: 5,
            k: 3,
        };
        let toy1 = Kind {
            sx: 8,
            sz: shape.sz(),
            deg: (0, 0),
            eval: Box::new(move |_: &[Fp], rx: &[Fp], ry: &[Fp]| {
                crate::r1cs::matrix_evals(&shape, rx, ry)
            }),
            eval_many: None,
        };
        let poly = MatPoly::new([uniform, toy1]);
        let random = |rng: &mut SpikeRng| -> Claim<Fp> {
            let point: Vec<Fp> = (0..poly.n()).map(|_| rng.scalar()).collect();
            Claim {
                value: poly.eval(&point),
                point,
            }
        };
        let cl = [
            random(&mut rng),
            random(&mut rng),
            random(&mut rng),
            random(&mut rng),
        ];
        // 1 + degree points: not a multiple of the lanes, so the last group
        // is partial.
        assert_eq!(
            restrict(&poly, &cl),
            restrict_pointwise(&poly, &cl),
            "the batched q is the pointwise q"
        );
        let (regs, slots) = c.plan_registers();
        assert!(
            regs < slots,
            "registers are reused ({regs} for {slots} slots)"
        );
    }

    #[test]
    fn degree_extension_matches_polynomials_on_both_fields() {
        fn check<F: Scalar>() {
            let mut rng = SpikeRng::new(b"degree extension boundaries");
            for degree in [0, 1, 2, 7, 16] {
                let polys: [Vec<F>; 3] =
                    std::array::from_fn(|_| (0..=degree).map(|_| rng.scalar()).collect());
                let at = |x: usize| {
                    let x = F::from(x as u64);
                    (
                        horner(&polys[0], x),
                        horner(&polys[1], x),
                        horner(&polys[2], x),
                    )
                };
                let values: Vec<_> = (0..=degree).map(at).collect();
                for count in [0, 1, degree + 1, degree + 2, 2 * degree + 5] {
                    let expected: Vec<_> = (0..count).map(at).collect();
                    assert_eq!(extend(&values, count, &small_inverses(count)), expected);
                }
            }
        }
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }

    /// Exercise the complete degree bound, differing kind dimensions and
    /// repeated curve points, with a direct polynomial as the oracle.
    #[test]
    fn batched_q_preserves_high_challenge_degrees_and_padding() {
        fn check<F: Scalar>() {
            let kind = |sx: usize, sz: usize, deg: (usize, usize)| {
                let eval = move |ch: &[F], rx: &[F], ry: &[F]| {
                    let alpha = (0..deg.0).fold(F::ONE, |a, _| a * ch[0]);
                    let beta = (0..deg.1).fold(F::ONE, |a, _| a * ch[1]);
                    let xs: F = rx.iter().copied().product();
                    let ys: F = ry.iter().copied().product();
                    (alpha * beta * xs * ys, beta * xs, F::ONE)
                };
                Kind {
                    sx,
                    sz,
                    deg,
                    eval: Box::new(eval),
                    eval_many: None,
                }
            };
            let mut rng = SpikeRng::new(b"q polynomial degree bounds");
            for dims in [(0, 0, 0, 0), (1, 3, 4, 1)] {
                let poly = MatPoly::with_min(
                    [kind(dims.0, dims.1, (1, 9)), kind(dims.2, dims.3, (2, 3))],
                    5,
                    4,
                );
                for repeated in [false, true] {
                    let mut cl: [Claim<F>; 4] = std::array::from_fn(|_| {
                        let point: Vec<_> = (0..poly.n()).map(|_| rng.scalar()).collect();
                        Claim {
                            value: poly.eval(&point),
                            point,
                        }
                    });
                    if repeated {
                        cl = std::array::from_fn(|_| cl[0].clone());
                    }
                    let q = restrict(&poly, &cl);
                    assert_eq!(q, restrict_pointwise(&poly, &cl));
                    for _ in 0..4 {
                        let t = rng.scalar();
                        assert_eq!(
                            horner(&q, t),
                            poly.eval(&curve(&cl, t)),
                            "off-grid evaluation"
                        );
                    }
                }
                assert!(
                    poly.eval_many(&[]).is_empty(),
                    "an empty batch has no values"
                );
            }
        }
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }

    #[test]
    fn deferred_fold_holds_proves_and_refuses() {
        let poly = toy_poly();
        let mut rng = SpikeRng::new(b"defer claims");
        let cl = claims(&poly, &mut rng);
        let q = restrict(&poly, &cl);
        let (c, inst, wit, fold) = build(&poly, &cl, &q);
        let shape = c.shape;
        let ch: Vec<Fp> = vec![rng.scalar(), rng.scalar()];
        // Honest: the relation holds, and so does the folded claim.
        let mut w = wit.w.clone();
        let _ = c.instantiate(&shape, &ch, Some(&mut w));
        assert!(
            holds(&c, &inst, w.clone(), &ch),
            "honest fold satisfies the relation"
        );
        let out = read_output(&c, &fold, &w);
        assert_eq!(
            out,
            fold_output(&cl, &q, ch[1]),
            "circuit output equals the native fold"
        );
        assert_eq!(poly.eval(&out.point), out.value, "folded claim holds");
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let mut prng = SpikeRng::new(b"defer prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        let bytes = proof.to_bytes();
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &bytes, &c).is_ok(),
            "honest proof"
        );
        // The native trace of a circuit proof replays the same transcript.
        let fixed = crate::spartan::fixed_commitments(&shape, &gens, &c);
        let tr = crate::verifier::trace_circuit(
            &shape,
            &gens,
            &inst,
            &proof,
            &[],
            &crate::spartan::Link::None,
            &c,
            &fixed,
        );
        assert!(tr.accepts, "the circuit trace accepts the proof");
        let ct = tr.circuit.as_ref().expect("circuit extras");
        assert_eq!(
            (ct.phase2.len(), ct.late_digest.is_some()),
            (2, true),
            "two phase-2 challenges and a late digest"
        );
        // The proof's late io is its folded claim, and that claim holds.
        let late = claim_of(&proof.late);
        assert_eq!(late.point.len(), poly.n(), "late io: point and value");
        assert_eq!(
            poly.eval(&late.point),
            late.value,
            "the proof's public accumulator holds"
        );
        // A late io value changed in the proof bytes: refused.
        let rows_c = shape.rows() - c.fixed_rows(&shape);
        for k in [0, poly.n()] {
            let mut bad = bytes.clone();
            let at = 32 * rows_c + 32 * k;
            let v = Fp::from_repr(bad[at..at + 32].try_into().unwrap()).unwrap() + Fp::ONE;
            bad[at..at + 32].copy_from_slice(&v.to_repr());
            assert!(
                crate::spartan::verify_circuit(&shape, &gens, &inst, &bad, &c).is_err(),
                "changed late io {k} refused"
            );
        }
        // A false fresh claim with q bent to pass through it: the circuit
        // accepts it, and the folded claim is false.
        let mut bad = cl.clone();
        bad[1].value += Fp::ONE;
        let l1 = lagrange4::<Fp>()[1];
        let mut qb = q.clone();
        for (e, c1) in l1.iter().enumerate() {
            qb[e] += *c1;
        }
        let (cb, ib, wb, fb) = build(&poly, &bad, &qb);
        let mut w = wb.w.clone();
        let _ = cb.instantiate(&cb.shape, &ch, Some(&mut w));
        assert!(
            holds(&cb, &ib, w.clone(), &ch),
            "a false claim with a matching q satisfies the relation"
        );
        let out = read_output(&cb, &fb, &w);
        assert!(
            poly.eval(&out.point) != out.value,
            "but the folded claim is false"
        );
        // A coefficient of q off (values kept), a folded coordinate off, and a
        // fresh claim's constant coordinate (its kind, a padding zero) off:
        // each breaks a row.
        let filled = |c: &UniformCircuit<Fp>, wit: &Witness<Fp>| -> Vec<Fp> {
            let mut w = wit.w.clone();
            let _ = c.instantiate(&c.shape, &ch, Some(&mut w));
            w
        };
        let mut w = filled(&c, &wit);
        w[c.cell_index(fold.q, Q_C, 7)] += Fp::ONE;
        assert!(!holds(&c, &inst, w, &ch), "changed coefficient refused");
        let mut w = filled(&c, &wit);
        w[c.cell_index(fold.coords, C_G, 3)] += Fp::ONE;
        assert!(
            !holds(&c, &inst, w, &ch),
            "changed folded coordinate refused"
        );
        let mut w = filled(&c, &wit);
        w[c.cell_index(fold.q, Q_HT, 0)] += Fp::ONE;
        assert!(!holds(&c, &inst, w, &ch), "changed folded value refused");
        let mut w = filled(&c, &wit);
        w[c.cell_index(fold.coords, C_P[3], 0)] += Fp::ONE;
        assert!(!holds(&c, &inst, w, &ch), "changed fresh kind refused");
        let pad = 4 + poly.kinds[1].sx;
        assert!(pad < 4 + poly.x, "the second kind has a padded x variable");
        let mut w = filled(&c, &wit);
        w[c.cell_index(fold.coords, C_P[3], pad)] += Fp::ONE;
        assert!(
            !holds(&c, &inst, w, &ch),
            "changed padding coordinate refused"
        );
        // The same refusals as proofs.
        let accepted = |bad: Vec<Fp>| -> bool {
            let mut prng = SpikeRng::new(b"defer tamper");
            let (proof, _) = crate::spartan::prove_circuit(
                &shape,
                &gens,
                &inst,
                &Witness { w: bad },
                &mut prng,
                &c,
            );
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok()
        };
        let mut b = wit.w.clone();
        b[c.cell_index(fold.values, V_V[2], 0)] += Fp::ONE;
        assert!(!accepted(b), "changed claim value refused by the verifier");
    }
}
