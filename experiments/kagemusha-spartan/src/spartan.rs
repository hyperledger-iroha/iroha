//! Zero-knowledge Spartan with Hyrax row commitments and committed
//! k-variable sumcheck rounds.
//!
//! Wire: row commitments `T_r`; per outer round one hiding commitment to the
//! eq-factored round polynomial's grid values; `v_A, v_B, v_C` (masked by two
//! random multiplication gates); per inner round one commitment; one IPA.
//! Every round-consistency check, both final checks and the hidden witness
//! evaluation `w~(r_y)` are batched with powers of `alpha` into one linear
//! relation over `(sum_r eq(r_row, r) W_r || round values)`, which the single
//! opening IPA proves. No round polynomial, witness evaluation or blinding
//! value appears in the clear.

use std::time::Instant;

use ff::{Field, PrimeField};
use group::GroupEncoding;
use iroha_pasta::msm::{
    BoundedSecretScalars, MemoryBudget, MsmError, SharedMemoryBudget, msm_public,
    msm_secret_ct_bounded_cancellable, msm_secret_ct_bounded_columns_cancellable,
};
use zeroize::Zeroizing;
use iroha_pasta::{CancellationToken, PastaCurve};
use rayon::prelude::*;

use crate::fs::{Scalar, SpikeRng, Transcript};
use crate::grid::{
    Extender, boolean_positions, eq_table, grid_vars, grid_weights, lagrange3, pow3, pow4,
    slot_positions,
};
use crate::ipa::{self, Aff, Claim, F, Gens, IpaProof};
use crate::r1cs::{self, Circuit, Instance, Rel, Shape, ToyCircuit, Witness};

#[cfg(test)]
#[path = "phase1_tests.rs"]
pub(crate) mod phase1_tests;

#[cfg(test)]
#[path = "commitment_budget_tests.rs"]
mod commitment_budget_tests;

#[derive(Clone)]
pub struct Proof<C: PastaCurve> {
    pub rows: Vec<Aff<C>>,
    /// Late io values (the circuit's `late_slots`, in order).
    pub late: Vec<F<C>>,
    pub outer: Vec<Aff<C>>,
    pub v: [F<C>; 3],
    pub inner: Vec<Aff<C>>,
    pub ipa: IpaProof<C>,
}

impl<C: PastaCurve> Proof<C> {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.rows
            .iter()
            .for_each(|p| out.extend_from_slice(&p.to_bytes()));
        self.late
            .iter()
            .for_each(|s| out.extend_from_slice(&s.to_repr()));
        self.outer
            .iter()
            .for_each(|p| out.extend_from_slice(&p.to_bytes()));
        self.v
            .iter()
            .for_each(|s| out.extend_from_slice(&s.to_repr()));
        self.inner
            .iter()
            .for_each(|p| out.extend_from_slice(&p.to_bytes()));
        for (l, r) in &self.ipa.lr {
            out.extend_from_slice(&l.to_bytes());
            out.extend_from_slice(&r.to_bytes());
        }
        out.extend_from_slice(&self.ipa.g_final.to_bytes());
        out.extend_from_slice(&self.ipa.a.to_bytes());
        out.extend_from_slice(&self.ipa.z1.to_repr());
        out.extend_from_slice(&self.ipa.z2.to_repr());
        out
    }

    /// Canonical decoding of exactly `shape.proof_bytes()` bytes.
    pub fn from_bytes(shape: &Shape, bytes: &[u8]) -> Option<Self> {
        Self::from_bytes_rows(shape, shape.rows(), 0, bytes)
    }

    /// Decoding with `rows` committed rows (the shape's rows less its fixed
    /// rows, whose commitments come from the verification key) and `late`
    /// late io values.
    pub fn from_bytes_rows(shape: &Shape, rows: usize, late: usize, bytes: &[u8]) -> Option<Self> {
        if bytes.len() + 32 * (shape.rows() - rows) != shape.proof_bytes() + 32 * late {
            return None;
        }
        let mut reader = Reader(bytes.chunks_exact(32));
        let rows = reader.points::<C>(rows)?;
        let late = (0..late)
            .map(|_| reader.scalar::<C>())
            .collect::<Option<Vec<_>>>()?;
        let outer = reader.points::<C>(shape.outer_blocks().len())?;
        let v = [
            reader.scalar::<C>()?,
            reader.scalar::<C>()?,
            reader.scalar::<C>()?,
        ];
        let inner = reader.points::<C>(shape.inner_blocks().len())?;
        let lr = (0..shape.col_bits)
            .map(|_| Some((reader.point::<C>()?, reader.point::<C>()?)))
            .collect::<Option<Vec<_>>>()?;
        let g_final = reader.point::<C>()?;
        let a = reader.point::<C>()?;
        let z1 = reader.scalar::<C>()?;
        let z2 = reader.scalar::<C>()?;
        Some(Self {
            rows,
            late,
            outer,
            v,
            inner,
            ipa: IpaProof {
                lr,
                g_final,
                a,
                z1,
                z2,
            },
        })
    }
}

struct Reader<'a>(std::slice::ChunksExact<'a, u8>);

impl Reader<'_> {
    fn chunk(&mut self) -> Option<[u8; 32]> {
        self.0
            .next()
            .map(|c| <[u8; 32]>::try_from(c).expect("32-byte chunk"))
    }
    fn point<C: PastaCurve>(&mut self) -> Option<Aff<C>> {
        Option::from(<Aff<C> as GroupEncoding>::from_bytes(&self.chunk()?))
    }
    fn points<C: PastaCurve>(&mut self, count: usize) -> Option<Vec<Aff<C>>> {
        (0..count).map(|_| self.point::<C>()).collect()
    }
    fn scalar<C: PastaCurve>(&mut self) -> Option<F<C>> {
        Option::from(F::<C>::from_repr(self.chunk()?))
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Timings {
    /// Hyrax row commitments (both phases).
    pub commit: f64,
    /// Of which the first phase.
    pub phase1: f64,
    /// Relation instantiation after the phase-2 challenges (sparse matrices
    /// and LogUp helpers).
    pub instantiate: f64,
    /// Of which the LogUp helpers (per proof) and the sparse matrices (a
    /// one-time preprocessing in a deployed prover); general circuits only.
    pub helpers: f64,
    pub matrices: f64,
    pub outer: f64,
    pub inner: f64,
    pub open: f64,
    /// Of `open`: the batched relation (weights, including the matrix
    /// evaluation `A~, B~, C~` at `(r_x, r_y)`), before the IPA.
    pub relation: f64,
    /// The commits' split (lane commits only): public-bound checks, the
    /// column MSMs, blinding plus affine conversion.
    pub split: [f64; 3],
}

impl Timings {
    pub fn total(&self) -> f64 {
        self.commit + self.instantiate + self.outer + self.inner + self.open
    }
    /// Per-proof work: everything except building the sparse matrices.
    pub fn core(&self) -> f64 {
        self.total() - self.matrices
    }
}

pub(crate) struct Challenges<S> {
    pub tau: Vec<S>,
    pub rx: Vec<Vec<S>>,
    pub v: [S; 3],
    pub rabc: [S; 3],
    pub ry: Vec<Vec<S>>,
    pub alpha: S,
    pub gamma: S,
}

/// Pairing of a P-proof and an S-proof: the cross-circuit binding's challenge
/// schedule (gate 1f). After its rows (phase 1), the first proof squeezes its
/// phase-1 digest d, then absorbs rho and R. The second proof, after its rows,
/// absorbs d, squeezes rho, then absorbs R = rlc(rho). So rho follows both
/// proofs' phase-1 commitments, and one R must satisfy both transcripts.
pub enum Link<'a> {
    None,
    First {
        rho: [u8; 32],
        r: [u8; 32],
    },
    Second {
        d: [u8; 32],
        rlc: &'a dyn Fn(&[u8; 32]) -> [u8; 32],
    },
}

/// The link's word, right after the first phase's rows: the first proof
/// squeezes d and absorbs rho, the second absorbs d and squeezes rho. Returns
/// (the squeezed word, rho). A circuit's link phase follows it. (A proof whose
/// rows are all first-phase rows, as the toy's, takes the word after all of
/// them, as before the link phase existed.)
fn link_word<C: PastaCurve>(t: &mut Transcript<C>, link: &Link) -> Option<([u8; 32], [u8; 32])> {
    match link {
        Link::None => None,
        Link::First { rho, .. } => {
            let _ = t.challenge_full(b"pair/d");
            let d = t.last_word().to_repr();
            t.word_limbs(b"pair/rho", rho);
            Some((d, *rho))
        }
        Link::Second { d, .. } => {
            t.word_limbs(b"pair/d", d);
            let _ = t.challenge_full(b"pair/rho");
            let rho = t.last_word().to_repr();
            Some((rho, rho))
        }
    }
}

/// The link's R: the first proof is given R; the second takes its circuit's
/// binding value when its prover has one (`binding`), `rlc` of rho otherwise
/// (a verifier's `rlc` returns the R it was given). Both proofs read R as io
/// (`Circuit::link_io`), so a circuit whose chain does not end at R fails.
fn link_value(link: &Link, rho: &[u8; 32], binding: Option<[u8; 32]>) -> [u8; 32] {
    match link {
        Link::None => unreachable!("a link value without a link"),
        Link::First { r, .. } => *r,
        Link::Second { rlc, .. } => binding.unwrap_or_else(|| rlc(rho)),
    }
}

/// Absorbs the link's R after every row. Returns (the squeezed word, R).
fn link_r<C: PastaCurve>(
    t: &mut Transcript<C>,
    word: Option<([u8; 32], [u8; 32])>,
    r: Option<[u8; 32]>,
) -> Option<([u8; 32], [u8; 32])> {
    let (squeezed, _) = word?;
    let r = r.expect("a link word has its R");
    t.word_limbs(b"pair/R", &r);
    Some((squeezed, r))
}

/// The first proof's phase-1 digest d (header and rows), as its verifier
/// recomputes it before replaying the second proof.
pub fn phase1_digest<C: PastaCurve>(
    shape: &Shape,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
) -> [u8; 32] {
    let mut t = Transcript::<C>::new(shape.label.as_bytes());
    header(&mut t, shape, &inst.io);
    for row in &proof.rows {
        t.point(b"row", row);
    }
    let _ = t.challenge_full(b"pair/d");
    t.last_word().to_repr()
}

/// Malicious provers for the regression suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cheat {
    Honest,
    /// Consult 6(b): round-0 commitment `D_0 = rho H` commits to nothing; after
    /// `r_0` the prover picks `q_0` (false Boolean sum, true value at `r_0`) and
    /// sends `D_1 = Com_0(q_0) + Com_1(t_1) - D_0`.
    CancelRound,
    /// A round commitment carries witness-column terms chosen after `tau`, so
    /// the opened witness differs from the rows committed before `tau`.
    AdaptiveWitness,
}

/// The shape (constants) and one digest of the public inputs, computed in
/// their own field (the scalar circuit hashes them; the transcript absorbs the
/// digest).
/// The io with the late values at their slots.
pub fn io_with_late<S: Scalar>(io: &[S], slots: &[usize], late: &[S]) -> Vec<S> {
    let mut out = io.to_vec();
    for (&s, v) in slots.iter().zip(late) {
        if out.len() <= s {
            out.resize(s + 1, S::ZERO);
        }
        out[s] = *v;
    }
    out
}

pub fn header<C: PastaCurve>(t: &mut Transcript<C>, shape: &Shape, io: &[F<C>]) {
    for value in [shape.sx, shape.row_bits, shape.col_bits, shape.k, io.len()] {
        t.bytes(b"shape", &(value as u64).to_le_bytes());
    }
    t.scalar(b"io", &crate::fs::io_digest(io));
}

/// Inner-round challenges: the row bits ry[1..=row_bits] multiply the row
/// commitments in the verifier, so they are endoscalars.
fn inner_challenges<C: PastaCurve>(
    t: &mut Transcript<C>,
    shape: &Shape,
    start: usize,
    k: usize,
) -> Vec<F<C>> {
    if crate::fs::lean_schedule() && start > shape.row_bits {
        return crate::fs::kron3(t.challenge(b"ry"), k);
    }
    (start..start + k)
        .map(|g| {
            if (1..=shape.row_bits).contains(&g) {
                t.endo_challenge(b"ry")
            } else {
                t.challenge(b"ry")
            }
        })
        .collect()
}

/// Tail offsets (IPA columns) of every committed round, outer then inner:
/// one aligned slot of 4^k columns each, in round order after the active
/// columns.
pub(crate) fn tail_offsets(shape: &Shape) -> (Vec<usize>, Vec<usize>) {
    let size = pow4(shape.k);
    let mo = shape.outer_blocks().len();
    let rounds = mo + shape.inner_blocks().len();
    let offsets: Vec<usize> = (0..rounds)
        .map(|s| shape.active_cols() + s * size)
        .collect();
    assert_eq!(shape.active_cols() % size, 0, "slots aligned");
    (offsets[..mo].to_vec(), offsets[mo..].to_vec())
}

/// A committed round's slot: sumcheck (outer or inner), round index,
/// variables (at most the shape's k: a shorter round takes the slot's low
/// digits), IPA offset, and its first variable (tau index for outer rounds,
/// r_y index for inner ones).
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub outer: bool,
    pub round: usize,
    pub k: usize,
    pub offset: usize,
    pub first: usize,
}

/// Every round slot, outer then inner.
pub fn slots(shape: &Shape) -> Vec<Slot> {
    let (outer_off, inner_off) = tail_offsets(shape);
    let mut out = Vec::new();
    for (outer, blocks, offs) in [
        (true, shape.outer_blocks(), outer_off),
        (false, shape.inner_blocks(), inner_off),
    ] {
        let mut first = 0;
        for (round, (&k, &offset)) in blocks.iter().zip(&offs).enumerate() {
            out.push(Slot {
                outer,
                round,
                k,
                offset,
                first,
            });
            first += k;
        }
    }
    out
}

/// Each slot's Boolean and Lagrange weight factors `(fb, fg)` in the batched
/// relation: alpha powers, the slot's `gamma^-(s+1)` scale, and the sign of
/// the round's link (the next round's claim, or the final check).
pub(crate) fn slot_weights<S: Field>(shape: &Shape, alpha: S, gamma: S) -> Vec<(S, S)> {
    let (mo, mi) = (shape.outer_blocks().len(), shape.inner_blocks().len());
    let ginv = gamma.invert().expect("nonzero challenge");
    let apow = |e: usize| alpha.pow_vartime([e as u64]);
    let mut out = Vec::with_capacity(mo + mi);
    let mut scale = S::ONE;
    for s in 0..mo + mi {
        scale *= ginv;
        let (b, last) = if s < mo {
            (s, s + 1 == mo)
        } else {
            (s + 1, s + 1 == mo + mi)
        };
        let sign = if last { S::ONE } else { -S::ONE };
        out.push((apow(b) * scale, sign * apow(b + 1) * scale));
    }
    out
}

/// `(prod_j (1 - r_j + r_j u_j), sum_{c >= active} s(u)_c eq(r, c))`: the
/// whole eq-weighted s sum and its part over the tail columns, by one pass
/// from the least significant column bit (suffixes of c at or above active's).
pub(crate) fn tail_eq_sums<S: Field>(us: &[S], r: &[S], active: usize) -> (S, S) {
    let l = us.len();
    let (mut f, mut d) = (S::ONE, S::ONE);
    for j in (0..l).rev() {
        let ru = r[j] * us[j];
        d = if (active >> (l - 1 - j)) & 1 == 1 {
            ru * d
        } else {
            ru * f + (S::ONE - r[j]) * d
        };
        f *= S::ONE - r[j] + ru;
    }
    (f, d)
}

/// `<s(u), b>` in closed form. The head `-coefficient eq(r_col, .)` on the
/// active columns is the whole eq sum minus its tail part (tail_eq_sums).
/// A slot's weights factor over its base-4 digits: s(u) on the slot is the
/// product over its offset's high bits times, per digit, 1, u_lo, u_hi on
/// values 0, 1, 2 (u_hi, u_lo the digit's two column bits); the Boolean
/// weights (eq(tau, .) outer, 1 inner) and the Lagrange weights factor the
/// same way. A shorter round's leading digits are 0: factor 1.
pub(crate) fn b_final_slots<S: Scalar>(
    shape: &Shape,
    ch: &Challenges<S>,
    coefficient: S,
    r_col: &[S],
    us: &[S],
) -> S {
    let l = us.len();
    let (full, tail) = tail_eq_sums(us, r_col, shape.active_cols());
    let mut total = -coefficient * (full - tail);
    for (slot, (fb, fg)) in slots(shape)
        .iter()
        .zip(slot_weights(shape, ch.alpha, ch.gamma))
    {
        let high = l - 2 * shape.k;
        let lead = shape.k - slot.k;
        let s_high: S = (0..high)
            .filter(|j| (slot.offset >> (l - 1 - j)) & 1 == 1)
            .map(|j| us[j])
            .product();
        let (mut bp, mut gp) = (S::ONE, S::ONE);
        for j in 0..slot.k {
            let (uh, ul) = (us[high + 2 * (lead + j)], us[high + 2 * (lead + j) + 1]);
            let r = if slot.outer {
                ch.rx[slot.round][j]
            } else {
                ch.ry[slot.round][j]
            };
            bp *= if slot.outer {
                let t = ch.tau[slot.first + j];
                S::ONE - t + ul * t
            } else {
                S::ONE + ul
            };
            let [l0, l1, l2] = lagrange3(r);
            gp *= l0 + ul * l1 + uh * l2;
        }
        total += s_high * (fb * bp + fg * gp);
    }
    total
}

/// The batched linear relation `<x, b> = kappa` over the IPA columns, and
/// the row weights `eq(r_row, .)`.
pub(crate) fn relation<S: Scalar>(
    shape: &Shape,
    ch: &Challenges<S>,
    io: &[S],
    rel: &dyn Rel<S>,
) -> (Vec<S>, S, Vec<S>, S, Vec<S>) {
    let rx: Vec<S> = ch.rx.concat();
    let ry: Vec<S> = ch.ry.concat();
    let (ma, mb, mc) = rel.matrix_evals(shape, &rx, &ry);
    relation_at(
        shape,
        ch,
        io,
        ch.rabc[0] * ma + ch.rabc[1] * mb + ch.rabc[2] * mc,
    )
}

/// [`relation`] with the inner final's matrix value `M(r_x, r_y) = r_A A~ +
/// r_B B~ + r_C C~` given: the verifier evaluates its program; the prover's
/// folded `M(r_x, .)` table is that value after the last inner round.
pub(crate) fn relation_at<S: Scalar>(
    shape: &Shape,
    ch: &Challenges<S>,
    io: &[S],
    m_value: S,
) -> (Vec<S>, S, Vec<S>, S, Vec<S>) {
    let (outer_off, inner_off) = tail_offsets(shape);
    let (ob, ib) = (shape.outer_blocks(), shape.inner_blocks());
    let mut b = vec![S::ZERO; shape.cols()];
    let mut kappa = S::ZERO;
    let mut apow = S::ONE;

    // Outer: sum_{y in {0,1}^k} eq(tau_i, y) t_i(y) = t_{i-1}(r_{i-1}), t_0 := 0.
    let mut position = 0;
    for (i, &k) in ob.iter().enumerate() {
        let eq_tau = eq_table(&ch.tau[position..position + k]);
        let at = slot_positions(k);
        for (index, &grid) in boolean_positions(k).iter().enumerate() {
            b[outer_off[i] + at[grid]] += apow * eq_tau[index];
        }
        if i > 0 {
            let at = slot_positions(ob[i - 1]);
            for (grid, weight) in grid_weights(&ch.rx[i - 1]).iter().enumerate() {
                b[outer_off[i - 1] + at[grid]] -= apow * weight;
            }
        }
        apow *= ch.alpha;
        position += k;
    }
    // Outer final: t_last(r_last) = v_A v_B - v_C.
    let last = ob.len() - 1;
    let at = slot_positions(ob[last]);
    for (grid, weight) in grid_weights(&ch.rx[last]).iter().enumerate() {
        b[outer_off[last] + at[grid]] += apow * weight;
    }
    kappa += apow * (ch.v[0] * ch.v[1] - ch.v[2]);
    apow *= ch.alpha;

    // Inner: sum_{y in {0,1}^k} q_i(y) = q_{i-1}(r'_{i-1}), q_0 := claim.
    let claim = ch.rabc[0] * ch.v[0] + ch.rabc[1] * ch.v[1] + ch.rabc[2] * ch.v[2];
    for (i, &k) in ib.iter().enumerate() {
        let at = slot_positions(k);
        for &grid in &boolean_positions(k) {
            b[inner_off[i] + at[grid]] += apow;
        }
        if i > 0 {
            let at = slot_positions(ib[i - 1]);
            for (grid, weight) in grid_weights(&ch.ry[i - 1]).iter().enumerate() {
                b[inner_off[i - 1] + at[grid]] -= apow * weight;
            }
        } else {
            kappa += apow * claim;
        }
        apow *= ch.alpha;
    }
    // Inner final: q_last(r'_last) = M(rx, ry) ((1 - r_sel) w~(r_yw) + r_sel io~(r_yw)).
    let ry: Vec<S> = ch.ry.concat();
    let last = ib.len() - 1;
    let at = slot_positions(ib[last]);
    for (grid, weight) in grid_weights(&ch.ry[last]).iter().enumerate() {
        b[inner_off[last] + at[grid]] += apow * weight;
    }
    let r_sel = ry[0];
    let r_row = &ry[1..1 + shape.row_bits];
    let r_col = &ry[1 + shape.row_bits..];
    let eq_col = eq_table(r_col);
    let coefficient = apow * m_value * (S::ONE - r_sel);
    b[..shape.active_cols()]
        .par_iter_mut()
        .zip(eq_col.par_iter())
        .for_each(|(weight, e)| *weight -= coefficient * e);
    kappa += apow * m_value * r_sel * r1cs::io_eval(shape, io, &ry[1..]);
    // Round slot i is opened as gamma^(i+1) times its values (separation of
    // every commitment by a challenge drawn after all of them).
    let gamma_inv = ch.gamma.invert().expect("nonzero challenge");
    let mut scale = S::ONE;
    for (offset, &k) in outer_off.iter().chain(&inner_off).zip(ob.iter().chain(&ib)) {
        scale *= gamma_inv;
        b[*offset..*offset + pow4(k)]
            .iter_mut()
            .for_each(|w| *w *= scale);
    }
    (b, kappa, eq_table(r_row), coefficient, r_col.to_vec())
}

fn fold_top<S: Field>(table: &[S], r: &[S]) -> Vec<S> {
    let low = table.len() >> r.len();
    let eq_r = eq_table(r);
    (0..low)
        .into_par_iter()
        .map(|j| {
            eq_r.iter()
                .enumerate()
                .map(|(hi, e)| *e * table[hi * low + j])
                .sum()
        })
        .collect()
}

fn add_into<S: Field>(mut a: Vec<S>, b: Vec<S>) -> Vec<S> {
    a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
    a
}

/// Grid values of `t(Y) = sum_low e(low) (A B - C)(Y, low)` (outer, `e` given)
/// or `sum_low (A B)(Y, low)` (inner, `c` and `e` absent).
fn round_values<S: Field>(
    extender: &Extender,
    a: &[S],
    b: &[S],
    c: Option<&[S]>,
    e: Option<&[S]>,
) -> Vec<S> {
    let k = extender.k;
    let (cube, grid) = (1 << k, pow3(k));
    let low = a.len() >> k;
    (0..low)
        .into_par_iter()
        .with_min_len(64)
        .fold(
            || {
                (
                    vec![S::ZERO; grid],
                    vec![S::ZERO; 3 * cube],
                    vec![S::ZERO; 3 * grid],
                )
            },
            |(mut acc, mut input, mut ext), j| {
                for hi in 0..cube {
                    input[hi] = a[hi * low + j];
                    input[cube + hi] = b[hi * low + j];
                    if let Some(c) = c {
                        input[2 * cube + hi] = c[hi * low + j];
                    }
                }
                let (ga, rest) = ext.split_at_mut(grid);
                let (gb, gc) = rest.split_at_mut(grid);
                extender.extend(&input[..cube], ga);
                extender.extend(&input[cube..2 * cube], gb);
                match (c, e) {
                    (Some(_), Some(e)) => {
                        extender.extend(&input[2 * cube..], gc);
                        let weight = e[j];
                        for g in 0..grid {
                            acc[g] += weight * (ga[g] * gb[g] - gc[g]);
                        }
                    }
                    _ => {
                        for g in 0..grid {
                            acc[g] += ga[g] * gb[g];
                        }
                    }
                }
                (acc, input, ext)
            },
        )
        .map(|(acc, _, _)| acc)
        .reduce(|| vec![S::ZERO; grid], add_into)
}

/// A round's grid values committed at its slot's base-4 positions, with the
/// blind on `H`: one constant-time secret MSM over the slot's grid generators
/// then `H`. The round values and the blind are private, so no digit-bucketed
/// table MSM.
fn commit_values<C: PastaCurve>(gens: &Gens<C>, offset: usize, values: &[F<C>], blind: F<C>) -> C {
    let mut bases: Vec<Aff<C>> = slot_positions(grid_vars(values.len()))
        .iter()
        .map(|p| gens.g[offset + p])
        .collect();
    bases.push(gens.h);
    let mut scalars = Zeroizing::new(Vec::with_capacity(values.len() + 1));
    scalars.extend_from_slice(values);
    scalars.push(blind);
    ipa::secret_msm::<C>(&scalars, &bases)
}

fn commit_row<C: PastaCurve>(
    gens: &Gens<C>,
    shape: &Shape,
    row: usize,
    values: &[F<C>],
    blind: F<C>,
) -> C {
    let split = values.len() - shape.row_full_tail(row);
    // The public width class bounds the head; the masking tail is full width.
    // Both go through the constant-time secret MSM (see `bounded_row` for a
    // head outside its class). The unblinded sum is wiped once blinded.
    let head = bounded_row::<C>(&values[..split], shape.row_bits_class(row));
    let mut sum = Zeroizing::new(
        msm_secret_ct_bounded_cancellable::<C>(
            &head,
            &gens.g[..split],
            MemoryBudget::DEFAULT,
            &SharedMemoryBudget::process_default(),
            None,
        )
        .expect("constant-time row commitment"),
    );
    if split < values.len() {
        *sum += ipa::secret_msm::<C>(&values[split..], &gens.g[split..values.len()]);
    }
    *sum + C::from(gens.h) * blind
}

/// Seconds the row commits spent since the prover's last reset: checking each
/// row against its public bound, in the column MSM, and blinding plus the
/// affine conversion.
pub static COMMIT_SPLIT: std::sync::Mutex<[f64; 3]> = std::sync::Mutex::new([0.0; 3]);

/// The prover's private commitment kernels, for measurement lines. Every
/// private commitment (witness rows, round values, the opening's L and R)
/// runs a constant-time secret MSM over public bases; no fixed-base table is
/// built or read, so a shape has no commitment setup to warm or account.
pub const COMMIT_KERNELS: &str = "commits: constant-time secret MSMs (bounded columns for rows, full width for rounds and the opening), no fixed-base table";

/// `sum_j col[j] G_j` per bounded column through the constant-time secret
/// column MSM: one window plan from the columns' public bounds, tables of
/// public generator multiples shared by every column, whole-table
/// constant-time selection, and no digit-indexed bucket or value-based skip.
pub(crate) fn column_sums<C: PastaCurve>(
    gens: &Gens<C>,
    active: usize,
    columns: &[BoundedSecretScalars<F<C>>],
) -> Vec<C> {
    column_sums_with_budget(gens, active, columns, MemoryBudget::DEFAULT, None)
        .expect("constant-time bounded row commitment")
}

/// [`column_sums`] within an explicit per-kernel `budget`, cancellable.
///
/// A budget below the kernel's single-base plan is [`MsmError::Budget`];
/// shared contention only changes the plan.
fn column_sums_with_budget<C: PastaCurve>(
    gens: &Gens<C>,
    active: usize,
    columns: &[BoundedSecretScalars<F<C>>],
    budget: MemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<Vec<C>, MsmError> {
    msm_secret_ct_bounded_columns_cancellable::<C>(
        columns,
        &gens.g[..active],
        budget,
        &SharedMemoryBudget::process_default(),
        cancellation,
    )
}

/// A row at its public bit bound, checked in constant time without a
/// value-derived width. The bound is a prover-side speedup: a row outside it
/// (a cheating prover in the refusal tests) commits at full width. That
/// whole-row validity bit is the only value-derived decision; an honest
/// witness always passes, so it reveals nothing about an honest row.
fn bounded_row<'a, C: PastaCurve>(row: &'a [F<C>], bits: usize) -> BoundedSecretScalars<'a, F<C>> {
    BoundedSecretScalars::new(row, bits.clamp(1, 255), None)
        .or_else(|_| BoundedSecretScalars::new(row, 255, None))
        .expect("full width holds every scalar")
}

pub(crate) fn commit_rows_lanes<C: PastaCurve>(
    gens: &Gens<C>,
    shape: &Shape,
    w: &[F<C>],
    range: std::ops::Range<usize>,
    bounds: &[usize],
    blinds: &[F<C>],
    helpers: bool,
) -> Vec<Aff<C>> {
    let rows: Vec<&[F<C>]> = w.chunks_exact(shape.cols()).collect();
    commit_row_slices(gens, shape, &rows, range, bounds, blinds, helpers)
}

/// [`commit_rows_lanes`] of the witness given as its rows.
fn commit_row_slices<C: PastaCurve>(
    gens: &Gens<C>,
    shape: &Shape,
    w: &[&[F<C>]],
    range: std::ops::Range<usize>,
    bounds: &[usize],
    blinds: &[F<C>],
    _helpers: bool,
) -> Vec<Aff<C>> {
    let active = shape.active_cols();
    let clock = Instant::now();
    // A row given empty is one the caller never wrote (`kern::Rows`, public
    // build structure): zero, its sum the identity. Every other row, LogUp
    // helper rows included, is a constant-time bounded secret column.
    let lanes: Vec<(usize, &[F<C>])> = range
        .clone()
        .filter(|&r| !w[r].is_empty())
        .map(|r| (r, &w[r][..active]))
        .collect();
    let old = ipa::old_kernels();
    // The new kernels check the rows' bounds, and blind and normalise the
    // sums, one row per task instead of one row after another.
    let columns: Vec<BoundedSecretScalars<F<C>>> = if old {
        lanes
            .iter()
            .map(|(r, row)| bounded_row::<C>(row, bounds[*r]))
            .collect()
    } else {
        lanes
            .par_iter()
            .with_max_len(1)
            .map(|(r, row)| bounded_row::<C>(row, bounds[*r]))
            .collect()
    };
    let checked = clock.elapsed().as_secs_f64();
    // Unblinded sums are private until blinded: wiped on every exit.
    let lane_sums = Zeroizing::new(column_sums(gens, active, &columns));
    assert_eq!(lane_sums.len(), lanes.len(), "one sum per committed row");
    let mut lane_sum = lane_sums.iter();
    let sums: Zeroizing<Vec<C>> = Zeroizing::new(
        range
            .clone()
            .map(|r| {
                if w[r].is_empty() {
                    C::identity()
                } else {
                    *lane_sum.next().expect("one sum per committed row")
                }
            })
            .collect(),
    );
    let summed = clock.elapsed().as_secs_f64();
    let blinded = |(sum, r): (&C, usize)| (*sum + C::from(gens.h) * blinds[r]).to_affine();
    let out = if old {
        sums.iter().zip(range).map(blinded).collect()
    } else {
        sums.as_slice()
            .par_iter()
            .zip(range.into_par_iter())
            .with_max_len(1)
            .map(blinded)
            .collect()
    };
    let done = clock.elapsed().as_secs_f64();
    let mut split = COMMIT_SPLIT.lock().unwrap();
    split[0] += checked;
    split[1] += summed - checked;
    split[2] += done - summed;
    out
}

fn gamma_powers<S: Field>(gamma: S, count: usize) -> Vec<S> {
    let mut power = S::ONE;
    (0..count)
        .map(|_| {
            power *= gamma;
            power
        })
        .collect()
}

/// The IPA commitment: `sum_r eq(r_row, r) T_r + sum_i gamma^(i+1) D_i`.
pub(crate) fn combined_commitment<C: PastaCurve>(
    rows: &[Aff<C>],
    rounds: &[Aff<C>],
    eq_row: &[F<C>],
    gamma: F<C>,
) -> C {
    let mut scalars = eq_row.to_vec();
    scalars.extend(gamma_powers(gamma, rounds.len()));
    let bases: Vec<Aff<C>> = rows.iter().chain(rounds).copied().collect();
    msm_public::<C>(&scalars, &bases, MemoryBudget::DEFAULT).expect("msm")
}

pub fn prove<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
) -> (Proof<C>, Timings) {
    prove_with(shape, gens, inst, wit, rng, Cheat::Honest)
}

pub fn prove_with<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    cheat: Cheat,
) -> (Proof<C>, Timings) {
    let (proof, timings, _) = prove_inner(
        shape,
        gens,
        inst,
        wit,
        rng,
        cheat,
        true,
        &[],
        &Link::None,
        &ToyCircuit,
        &[],
    );
    (proof, timings)
}

/// An honest proof that also folds inherited generator claims into its opening.
pub fn prove_with_claims<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    claims: &[Claim<C>],
) -> Proof<C> {
    prove_inner(
        shape,
        gens,
        inst,
        wit,
        rng,
        Cheat::Honest,
        true,
        claims,
        &Link::None,
        &ToyCircuit,
        &[],
    )
    .0
}

/// An honest proof with inherited claims and a pairing link; also returns the
/// link's squeezed word (d for the first proof, rho for the second) and R.
#[allow(clippy::too_many_arguments)]
pub fn prove_linked<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    claims: &[Claim<C>],
    link: &Link,
) -> (Proof<C>, [u8; 32], [u8; 32]) {
    let (proof, _, out) = prove_inner(
        shape,
        gens,
        inst,
        wit,
        rng,
        Cheat::Honest,
        true,
        claims,
        link,
        &ToyCircuit,
        &[],
    );
    let (word, r) = out.expect("linked proof");
    (proof, word, r)
}

/// Proves a general circuit (sparse relation, optional LogUp phase).
pub fn prove_circuit<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    circuit: &dyn Circuit<F<C>>,
) -> (Proof<C>, Timings) {
    let fixed = fixed_for(shape, gens, circuit, &inst.io);
    let _ = crate::prof::take();
    let (proof, timings, _) = prove_inner(
        shape,
        gens,
        inst,
        wit,
        rng,
        Cheat::Honest,
        true,
        &[],
        &Link::None,
        circuit,
        &fixed,
    );
    if crate::prof::enabled() {
        println!(
            "    proof blake3 {}",
            blake3::hash(&proof.to_bytes()).to_hex()
        );
    }
    (proof, timings)
}

/// The verification key's part for constant columns: one unblinded
/// commitment per trailing fixed row that holds the key's constants
/// (computed once per circuit; the wallet rows, first of the fixed rows, are
/// the instance's: `fixed_for`).
pub fn fixed_commitments<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    circuit: &dyn Circuit<F<C>>,
) -> Vec<Aff<C>> {
    let nf = circuit.fixed_rows(shape) - circuit.wallet_rows();
    if nf == 0 {
        return Vec::new();
    }
    // Only public fixed rows may omit zero suffixes. A padding row is the
    // first generator itself, including when the other columns are absent.
    (0..nf)
        .map(|k| {
            let row = circuit.fixed_commitment_prefix(shape, k);
            assert!(row.len() <= shape.active_cols());
            if row.is_empty() {
                return C::identity().to_affine();
            }
            if row.len() == 1 && row[0] == F::<C>::ONE {
                return gens.g[0];
            }
            iroha_pasta::msm::msm_public::<C>(
                &row,
                &gens.g[..row.len()],
                MemoryBudget::DEFAULT,
            )
            .expect("msm")
            .to_affine()
        })
        .collect()
}

pub fn verify_circuit<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
    circuit: &dyn Circuit<F<C>>,
) -> Result<(), String> {
    let fixed = fixed_for_verifier(shape, gens, circuit, &inst.io)?;
    verify_inner(
        shape,
        gens,
        inst,
        bytes,
        true,
        &[],
        &Link::None,
        circuit,
        &fixed,
    )
    .map(|_| ())
}

/// Every fixed row's commitment for one instance, in row order: the wallet
/// rows' from the instance's io (`Circuit::wallet_io`), then the key's
/// (`fixed_commitments`). This is the trusted prover/builder convenience;
/// verification uses fallible decoding of externally supplied instance io.
pub fn fixed_for<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    circuit: &dyn Circuit<F<C>>,
    io: &[F<C>],
) -> Vec<Aff<C>> {
    let mut fixed = wallet_commitments::<C>(circuit, io)
        .expect("a trusted instance names valid wallet row commitments");
    fixed.extend(fixed_commitments(shape, gens, circuit));
    fixed
}

/// Fixed-row assembly at the verifier boundary. Check the wallet inputs
/// before computing the circuit's ordinary verification-key commitments.
fn fixed_for_verifier<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    circuit: &dyn Circuit<F<C>>,
    io: &[F<C>],
) -> Result<Vec<Aff<C>>, String> {
    let nf = circuit.fixed_rows(shape);
    if nf > shape.rows() || circuit.wallet_rows() > nf {
        return Err("invalid fixed-row layout".into());
    }
    let mut fixed = wallet_commitments::<C>(circuit, io)?;
    fixed.extend(fixed_commitments(shape, gens, circuit));
    Ok(fixed)
}

/// The wallet rows' commitments an instance names (four io limbs per row).
/// Malformed, truncated or noncanonical inputs are rejected without indexing
/// beyond the supplied io or accepting a non-point commitment.
pub fn wallet_commitments<C: PastaCurve>(
    circuit: &dyn Circuit<F<C>>,
    io: &[F<C>],
) -> Result<Vec<Aff<C>>, String> {
    let n = circuit.wallet_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let at = circuit.wallet_io().ok_or("missing wallet commitment io")?;
    let end = n
        .checked_mul(4)
        .and_then(|length| at.checked_add(length))
        .ok_or("wallet commitment io span overflow")?;
    let limbs = io.get(at..end).ok_or("truncated wallet commitment io")?;
    limbs
        .chunks_exact(4)
        .map(|limbs| {
            point_from_limbs::<C>(limbs).ok_or_else(|| "invalid wallet commitment point".into())
        })
        .collect()
}

/// A point from its coordinates' 128-bit limbs (x low, x high, y low, y
/// high), each a scalar below 2^128; `None` unless there are exactly four
/// limbs and they encode a canonical finite curve point.
pub fn point_from_limbs<C: PastaCurve>(limbs: &[F<C>]) -> Option<Aff<C>> {
    if limbs.len() != 4 {
        return None;
    }
    let base = |lo: &F<C>, hi: &F<C>| -> Option<C::Base> {
        let mut bytes = [0u8; 32];
        for (half, v) in [lo, hi].into_iter().enumerate() {
            let r = v.to_repr();
            let r = r.as_ref();
            if r[16..].iter().any(|b| *b != 0) {
                return None;
            }
            bytes[16 * half..16 * half + 16].copy_from_slice(&r[..16]);
        }
        let mut repr = <C::Base as ff::PrimeField>::Repr::default();
        repr.as_mut().copy_from_slice(&bytes);
        Option::from(<C::Base as ff::PrimeField>::from_repr(repr))
    };
    let x = base(&limbs[0], &limbs[1])?;
    let y = base(&limbs[2], &limbs[3])?;
    let point: Aff<C> = Option::from(<Aff<C> as iroha_pasta::PastaAffine>::from_xy(x, y))?;
    bool::from(iroha_pasta::PastaAffine::coordinates(&point).is_some()).then_some(point)
}

/// A point's coordinates as four 128-bit limbs (x low, x high, y low, y
/// high) in the scalar field.
pub fn limbs_of_point<C: PastaCurve>(p: &Aff<C>) -> [F<C>; 4] {
    let (x, y): (C::Base, C::Base) =
        Option::from(iroha_pasta::PastaAffine::coordinates(p)).expect("a finite point");
    let mut out = [F::<C>::ZERO; 4];
    for (k, c) in [x, y].into_iter().enumerate() {
        let r = ff::PrimeField::to_repr(&c);
        let r = r.as_ref();
        let lo = u128::from_le_bytes(r[..16].try_into().unwrap());
        let hi = u128::from_le_bytes(r[16..32].try_into().unwrap());
        out[2 * k] = F::<C>::from_u128(lo);
        out[2 * k + 1] = F::<C>::from_u128(hi);
    }
    out
}

/// The unblinded commitment of `values` on the first generators (a row's
/// leading columns; the rest zero).
pub fn commit_public<C: PastaCurve>(gens: &Gens<C>, values: &[F<C>]) -> Aff<C> {
    iroha_pasta::msm::msm_public::<C>(values, &gens.g[..values.len()], MemoryBudget::DEFAULT)
        .expect("msm")
        .to_affine()
}

/// One row's unblinded commitment, as the key's and the wallet rows' are.
pub fn row_commitment<C: PastaCurve>(shape: &Shape, gens: &Gens<C>, row: &[F<C>]) -> Aff<C> {
    let active = shape.active_cols();
    iroha_pasta::msm::msm_public::<C>(&row[..active], &gens.g[..active], MemoryBudget::DEFAULT)
        .expect("msm")
        .to_affine()
}

/// Verification with precomputed fixed-row commitments, including the
/// wallet prefix. The wallet prefix must agree with this instance's io;
/// cached commitments from another wallet are rejected.
pub fn verify_circuit_with<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
    circuit: &dyn Circuit<F<C>>,
    fixed: &[Aff<C>],
) -> Result<(), String> {
    verify_inner(
        shape,
        gens,
        inst,
        bytes,
        true,
        &[],
        &Link::None,
        circuit,
        fixed,
    )
    .map(|_| ())
}

/// The construction without per-commitment separation (`gamma = 1`), kept
/// only to show that the regression attacks succeed against it.
pub fn prove_unseparated<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    cheat: Cheat,
) -> Proof<C> {
    prove_inner(
        shape,
        gens,
        inst,
        wit,
        rng,
        cheat,
        false,
        &[],
        &Link::None,
        &ToyCircuit,
        &[],
    )
    .0
}

/// tau: per variable, or powers of one full-width challenge (lean schedule).
pub(crate) fn tau_challenges<C: PastaCurve>(t: &mut Transcript<C>, shape: &Shape) -> Vec<F<C>> {
    if crate::fs::lean_schedule() {
        crate::fs::powers2(t.challenge_full(b"tau"), shape.sx)
    } else {
        t.challenges(b"tau", shape.sx)
    }
}

/// An outer round's challenges: per variable, or Kronecker powers.
pub(crate) fn round_challenges<C: PastaCurve>(t: &mut Transcript<C>, k: usize) -> Vec<F<C>> {
    if crate::fs::lean_schedule() {
        crate::fs::kron3(t.challenge(b"rx"), k)
    } else {
        t.challenges(b"rx", k)
    }
}

pub(crate) fn rabc_challenges<C: PastaCurve>(t: &mut Transcript<C>) -> [F<C>; 3] {
    if crate::fs::lean_schedule() {
        let r = t.challenge(b"rABC");
        [r, r * r, r * r * r]
    } else {
        [t.challenge(b"rA"), t.challenge(b"rB"), t.challenge(b"rC")]
    }
}

fn gamma_challenge<C: PastaCurve>(t: &mut Transcript<C>, separated: bool) -> F<C> {
    let gamma = t.endo_challenge(b"gamma");
    if separated { gamma } else { F::<C>::ONE }
}

/// The first phase's row blinds, in row order: one draw per non-fixed row and
/// zero for the fixed tail. Mask randomization precedes it. This is the first
/// phase's whole draw from `rng`, whether or not the rows are committed by the
/// caller, so a proof that reuses its own commitments draws exactly the same.
fn first_phase_blinds<C: PastaCurve>(
    shape: &Shape,
    rng: &mut SpikeRng,
    circuit: &dyn Circuit<F<C>>,
) -> Vec<F<C>> {
    let rows_c = shape.rows() - circuit.fixed_rows(shape);
    (0..shape.rows())
        .map(|r| {
            if r < rows_c {
                rng.scalar()
            } else {
                F::<C>::ZERO
            }
        })
        .collect()
}

/// Shared first-phase schedule. Mask randomization precedes this function; every
/// non-fixed row's blind is drawn even when only the first rows are committed.
fn commit_first_phase<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    w: &[&[F<C>]],
    rng: &mut SpikeRng,
    circuit: &dyn Circuit<F<C>>,
) -> (Vec<F<C>>, Option<Vec<usize>>, Vec<Aff<C>>) {
    let rows_c = shape.rows() - circuit.fixed_rows(shape);
    let blinds = first_phase_blinds::<C>(shape, rng, circuit);
    let bounds = circuit.row_bounds(shape);
    let p1 = circuit.phase1_rows(shape);
    assert!(p1 <= rows_c, "first-phase rows precede the fixed tail");
    let rows = match &bounds {
        Some(b) => commit_row_slices(gens, shape, w, 0..p1, b, &blinds, false),
        None => (0..p1)
            .map(|r| {
                commit_row(gens, shape, r, &w[r][..shape.active_cols()], blinds[r]).to_affine()
            })
            .collect(),
    };
    (blinds, bounds, rows)
}

/// Compute the first linked proof's digest before its link/lookup phases.
/// Replay the same entropy stream for the final proof. This performs real
/// masked row commitments, without allocating matrix plans or proving a result
/// that would be discarded solely to learn this digest.
pub fn prepare_linked_phase1<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    circuit: &dyn Circuit<F<C>>,
) -> [u8; 32] {
    let fill = vec![r1cs::RowFill::Values; shape.rows()];
    prepare_linked_phase1_built(shape, gens, inst, wit, &fill, rng, circuit)
}

/// [`prepare_linked_phase1`] of a witness exactly as its build wrote it:
/// `fill` ([`crate::uniform::UniformCircuit::build_rows`]) names the rows the
/// build never wrote, which are never read.
pub fn prepare_linked_phase1_built<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    fill: &[r1cs::RowFill],
    rng: &mut SpikeRng,
    circuit: &dyn Circuit<F<C>>,
) -> [u8; 32] {
    linked_phase1_rows(shape, gens, inst, wit, fill, rng, circuit).1
}

/// The first linked proof's first phase over a witness as its build wrote
/// it: the masked, blinded first-phase row commitments and the digest d they
/// give. The prover's copies of the mask rows and the row blinds are wiped
/// before the transcript is touched; the commitments are public (a proof
/// carries them unchanged).
fn linked_phase1_rows<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    fill: &[r1cs::RowFill],
    rng: &mut SpikeRng,
    circuit: &dyn Circuit<F<C>>,
) -> (Vec<Aff<C>>, [u8; 32]) {
    use zeroize::Zeroize as _;
    shape.check();
    // Only the rows holding product masks are copied (and zeroized).
    let mut w = crate::kern::Rows::with_fill(&wit.w, shape.cols(), fill);
    circuit.randomize_masks_rows(&mut w, rng);
    let (mut blinds, _, rows) = commit_first_phase(shape, gens, &w.slices(), rng, circuit);
    w.zeroize_owned();
    blinds.zeroize();
    let mut transcript = Transcript::<C>::new(shape.label.as_bytes());
    header(&mut transcript, shape, &inst.io);
    for row in &rows {
        transcript.point(b"row", row);
    }
    let _ = transcript.challenge_full(b"pair/d");
    let d = transcript.last_word().to_repr();
    (rows, d)
}

/// The first linked proof's first phase, committed once and owned until its
/// proof is made ([`prepare_linked_phase1_retained`]).
///
/// It borrows the generators, instance, witness, row account and circuit it
/// was prepared over, and keeps its own copy of the entropy stream as the
/// caller's stood before the preparation drew from it.
/// [`LinkedPhase1::prove_first`] consumes it and proves over exactly those
/// borrows and that stream. No argument of either call can pair the retained
/// commitments with other masks, blinds, rows or generators, and they serve
/// at most one proof. Only the preparation constructs this value.
///
/// Held between the two calls: the first-phase row commitments and the digest
/// d, which are public, and the stream's reader, which is private and wiped
/// when this value is consumed, dropped or unwound. The prover's copies of the
/// mask rows and the row blinds are not held: the preparation wipes them as
/// [`prepare_linked_phase1_built`] does, and the proof regenerates them from
/// the stream, in the same order and before any later draw.
///
/// Memory: [`LinkedPhase1::retained_bytes`] is one point per first-phase row
/// and the reader, never a witness row. A caller allocates it before its next
/// stage's availability query, which therefore already charges it, as it does
/// the caller's witness and retained circuit. The proof's own workspace is
/// unchanged, less the first phase's commitment scratch.
#[must_use = "a retained first phase is only useful to the proof it prepares"]
pub struct LinkedPhase1<'a, C: PastaCurve> {
    shape: Shape,
    gens: &'a Gens<C>,
    inst: &'a Instance<F<C>>,
    wit: &'a Witness<F<C>>,
    fill: &'a [r1cs::RowFill],
    circuit: &'a dyn Circuit<F<C>>,
    /// The entropy stream at the position the preparation started from. Boxed:
    /// it stays at one address from the preparation to its wipe, and moving
    /// this value moves only the pointer.
    replay: Box<SpikeRng>,
    /// The first-phase row commitments, in row order.
    rows: Vec<Aff<C>>,
    d: [u8; 32],
}

/// [`prepare_linked_phase1_built`] that keeps its row commitments for the
/// proof: the same digest and the same draw from `rng`, which the caller's
/// stream is advanced by as before. The returned value proves once, over the
/// same arguments and the stream as it stood on entry, without committing the
/// first-phase rows again.
pub fn prepare_linked_phase1_retained<'a, C: PastaCurve>(
    shape: &Shape,
    gens: &'a Gens<C>,
    inst: &'a Instance<F<C>>,
    wit: &'a Witness<F<C>>,
    fill: &'a [r1cs::RowFill],
    rng: &mut SpikeRng,
    circuit: &'a dyn Circuit<F<C>>,
) -> LinkedPhase1<'a, C> {
    // The stream as the proof replays it, taken before anything is drawn.
    let replay = Box::new(rng.clone());
    let (rows, d) = linked_phase1_rows(shape, gens, inst, wit, fill, rng, circuit);
    LinkedPhase1 {
        shape: *shape,
        gens,
        inst,
        wit,
        fill,
        circuit,
        replay,
        rows,
        d,
    }
}

impl<C: PastaCurve> LinkedPhase1<'_, C> {
    /// The first proof's phase-1 digest d, exactly
    /// [`prepare_linked_phase1_built`]'s.
    pub fn digest(&self) -> [u8; 32] {
        self.d
    }

    /// Bytes held from the preparation to the proof: this value, its stream
    /// reader and its row commitments' storage.
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + std::mem::size_of::<SpikeRng>()
            + self.rows.capacity() * std::mem::size_of::<Aff<C>>()
    }

    /// The first linked proof, given the second proof's `rho` and `R`: byte
    /// for byte [`prove_linked_built`]'s over the prepared arguments, the
    /// replayed stream and `Link::First { rho, r }`, with the first-phase
    /// rows committed once. Returns the proof, the squeezed d and R.
    pub fn prove_first(mut self, rho: [u8; 32], r: [u8; 32]) -> (Proof<C>, [u8; 32], [u8; 32]) {
        if crate::prof::memory_enabled() {
            println!(
                "prover retained first phase {}: {} row commitments, {} B held since its preparation",
                self.shape.label,
                self.rows.len(),
                self.retained_bytes()
            );
        }
        let fixed = fixed_for(&self.shape, self.gens, self.circuit, &self.inst.io);
        let rows = std::mem::take(&mut self.rows);
        let (proof, _, out) = prove_rows(
            &self.shape,
            self.gens,
            self.inst,
            self.wit,
            Some(self.fill),
            &mut self.replay,
            Cheat::Honest,
            true,
            &[],
            &Link::First { rho, r },
            self.circuit,
            &fixed,
            Some(rows),
        );
        let (word, r) = out.expect("linked proof");
        (proof, word, r)
    }
}

#[allow(clippy::too_many_arguments)]
fn prove_inner<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    cheat: Cheat,
    separated: bool,
    claims: &[Claim<C>],
    link: &Link,
    circuit: &dyn Circuit<F<C>>,
    fixed: &[Aff<C>],
) -> (Proof<C>, Timings, Option<([u8; 32], [u8; 32])>) {
    prove_rows(
        shape, gens, inst, wit, None, rng, cheat, separated, claims, link, circuit, fixed, None,
    )
}

/// [`prove_inner`] of a witness in which `fill` (when given) names the rows
/// its build never wrote: the prover never reads those rows. Without it every
/// row is read, so a witness changed after its build is proved as it is.
///
/// `phase1` (only [`LinkedPhase1::prove_first`] gives it) holds this proof's
/// own first-phase row commitments, made by its preparation over this
/// witness, row account, generators and circuit from `rng`'s stream at this
/// position. The masks and blinds are then regenerated in the same order and
/// the first-phase rows are not committed again.
#[allow(clippy::too_many_arguments)]
fn prove_rows<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    fill: Option<&[r1cs::RowFill]>,
    rng: &mut SpikeRng,
    cheat: Cheat,
    separated: bool,
    claims: &[Claim<C>],
    link: &Link,
    circuit: &dyn Circuit<F<C>>,
    fixed: &[Aff<C>],
    phase1: Option<Vec<Aff<C>>>,
) -> (Proof<C>, Timings, Option<([u8; 32], [u8; 32])>) {
    shape.check();
    let fill: Vec<r1cs::RowFill> = match fill {
        Some(fill) => fill.to_vec(),
        None => vec![r1cs::RowFill::Values; shape.rows()],
    };
    if crate::prof::memory_enabled() {
        // The witness rows by the build's public account only; no private
        // value is read or counted.
        let rows_of = |kind: r1cs::RowFill| fill.iter().filter(|f| **f == kind).count();
        println!(
            "prover accounting {} sx {} sw {}: {}; witness rows: values {}, never written {}, padding {}, of {}",
            shape.label,
            shape.sx,
            shape.sw(),
            circuit.memory_note(),
            rows_of(r1cs::RowFill::Values),
            rows_of(r1cs::RowFill::Zero),
            rows_of(r1cs::RowFill::Unit),
            fill.len()
        );
    }
    let mem = crate::prof::MemTrace::start();
    let prove_clock = Instant::now();
    let mark = |name: &'static str| {
        if let Some(m) = &mem {
            m.mark(name);
        }
    };
    let (rows_n, cols, active) = (shape.rows(), shape.cols(), shape.active_cols());
    // Trailing fixed rows: committed in the key, blind 0, not in the proof.
    let rows_c = rows_n - fixed.len();
    let (outer_off, inner_off) = tail_offsets(shape);
    let mut timings = Timings::default();
    let mut t = Transcript::<C>::new(shape.label.as_bytes());
    header(&mut t, shape, &inst.io);
    // The witness half of z = (w || io) as Hyrax rows: the caller's, except
    // the rows the prover writes (masks, the link phase, the second phase),
    // which it copies. The public half is the io, zero beyond it.
    let half = 1usize << shape.sw();
    assert_eq!(wit.w.len(), half, "witness length");
    assert!(
        fill.iter()
            .enumerate()
            .all(|(r, f)| *f != r1cs::RowFill::Unit || r >= rows_c),
        "only fixed rows are padding rows"
    );
    let mut zw = crate::kern::Rows::with_fill(&wit.w, cols, &fill);
    crate::prof::time("c.masks", || circuit.randomize_masks_rows(&mut zw, rng));
    mark("z");

    // Hyrax rows: phase 1, then (after its challenges) phase 2.
    let clock = Instant::now();
    *COMMIT_SPLIT.lock().unwrap() = [0.0; 3];
    assert_eq!(fixed.len(), circuit.fixed_rows(shape));
    let (row_blinds, bounds, mut rows) = match phase1 {
        // This proof's preparation committed these rows over the same masked
        // witness and blinds: the blinds are drawn again in order (the masks
        // were, just above) and the commitments are not recomputed.
        Some(rows) => {
            let blinds = first_phase_blinds::<C>(shape, rng, circuit);
            assert_eq!(
                rows.len(),
                circuit.phase1_rows(shape),
                "one retained commitment per first-phase row"
            );
            (blinds, circuit.row_bounds(shape), rows)
        }
        None => commit_first_phase(shape, gens, &zw.slices(), rng, circuit),
    };
    mark("phase1");
    let commit = |w: &[&[F<C>]], range: std::ops::Range<usize>, helpers: bool| -> Vec<Aff<C>> {
        match &bounds {
            Some(b) => commit_row_slices(gens, shape, w, range, b, &row_blinds, helpers),
            None => range
                .map(|r| commit_row(gens, shape, r, &w[r][..active], row_blinds[r]).to_affine())
                .collect(),
        }
    };
    let p1 = circuit.phase1_rows(shape);
    timings.phase1 = clock.elapsed().as_secs_f64();
    crate::prof::add("c.phase1", timings.phase1);
    for row in &rows {
        t.point(b"row", row);
    }
    // The link's word, then the link phase (values that follow rho), R and
    // the link's io (rho and R).
    let word = link_word(&mut t, link);
    let pl = circuit.link_rows(shape);
    let mut r_link = None;
    let link_io = match word {
        None => Vec::new(),
        Some((_, rho)) => {
            if pl > 0 {
                circuit.fill_link_rows(&rho, &mut zw);
            }
            let r = link_value(link, &rho, circuit.binding_r_rows(&zw));
            r_link = Some(r);
            circuit.link_io(&rho, &r)
        }
    };
    if pl > 0 {
        assert!(word.is_some(), "a link phase needs a link");
        let rows_l = commit(&zw.slices(), p1..p1 + pl, false);
        for row in &rows_l {
            t.point(b"row", row);
        }
        rows.extend(rows_l);
    }
    mark("link");
    let ch2: Vec<F<C>> = (0..circuit.phase2_challenges())
        .map(|_| t.challenge_full(b"phase2"))
        .collect();
    *crate::sparse::LAST_BUILD.lock().unwrap() = (0.0, 0.0);
    let clock_rel = Instant::now();
    let mut rel = circuit.instantiate_rows(shape, &ch2, &mut zw);
    timings.instantiate = clock_rel.elapsed().as_secs_f64();
    mark("instantiate");
    // Late io: fixed by the second phase, absorbed before its rows.
    let late = circuit.late_values_rows(&zw);
    if !late.is_empty() {
        t.scalar(b"late", &crate::fs::io_digest(&late));
    }
    let inst_full = Instance {
        io: io_with_late(
            &r1cs::io_with_link(&inst.io, &link_io),
            &circuit.late_slots(),
            &late,
        ),
    };
    let inst = &inst_full;
    (timings.helpers, timings.matrices) = *crate::sparse::LAST_BUILD.lock().unwrap();
    let rows2 = crate::prof::time("c.phase2", || {
        commit(
            &zw.slices(),
            p1 + pl..rows_c,
            circuit.phase2_challenges() > 0,
        )
    });
    for row in rows2.iter().chain(fixed) {
        t.point(b"row", row);
    }
    rows.extend(rows2);
    rows.extend(fixed.iter().copied());
    mark("phase2");
    let link_out = link_r(&mut t, word, r_link);
    timings.commit = clock.elapsed().as_secs_f64() - timings.instantiate;
    timings.split = *COMMIT_SPLIT.lock().unwrap();

    // Outer sumcheck, eq-factored, committed rounds.
    let clock = Instant::now();
    let tau: Vec<F<C>> = tau_challenges(&mut t, shape);
    // AdaptiveWitness: after tau, shift one witness column in every row by J
    // so that the tau-weighted error vanishes; carry J G[col] in round 0.
    let mut adaptive: Option<(usize, F<C>)> = None;
    if cheat == Cheat::AdaptiveWitness {
        let full = zw.to_vec();
        let (col, shift) = adaptive_shift(shape, &full, &tau);
        adaptive = Some((col, shift));
        zw.write_back(&shifted(shape, &full, col, shift));
    }
    let z_tail = inst.io.clone();
    let zv = crate::kern::ZView {
        rows: &zw,
        tail: &z_tail,
    };
    let old_kernels = ipa::old_kernels();
    // An honest prover's first outer round has zero Boolean points (the
    // relation holds on every row); `ZKS_OUTER=exact` computes them anyway.
    let assume_satisfied = cheat == Cheat::Honest;
    // A satisfied relation that computes rows on demand never holds its
    // unfolded Az, Bz, Cz: the first round and its fold stream them a chunk
    // of rows at a time.
    let stream = assume_satisfied
        && !old_kernels
        && rel.abc_ranges(shape, &zv, &[], [false; 3], &mut Default::default());
    let (mut a, mut b, mut c) = if stream {
        (Vec::new(), Vec::new(), Vec::new())
    } else {
        crate::prof::time("o.abc", || rel.abc_rows(shape, &zv))
    };
    mark("abc");
    let mut cancel_d0: Option<C> = None;
    let mut tail_values: Vec<Vec<F<C>>> = Vec::new();
    let mut round_blinds: Vec<F<C>> = Vec::new();
    let mut outer = Vec::new();
    let mut rx: Vec<Vec<F<C>>> = Vec::new();
    let mut position = 0;
    for (i, &k) in shape.outer_blocks().iter().enumerate() {
        let extender = Extender::new(k);
        let basis = crate::kern::Basis::new(k);
        let e_low = crate::prof::time("o.eq", || eq_table(&tau[position + k..]));
        let streamed = stream && i == 0;
        let low = (1usize << shape.sx) >> k;
        let corners = |js: std::ops::Range<usize>, mats: [bool; 3], out: &mut [Vec<F<C>>; 3]| {
            let ranges: Vec<std::ops::Range<usize>> = (0..1usize << k)
                .map(|hi| hi * low + js.start..hi * low + js.end)
                .collect();
            rel.abc_ranges(shape, &zv, &ranges, mats, out);
        };
        let values = crate::prof::time("o.round", || {
            if old_kernels {
                round_values(&extender, &a, &b, Some(&c), Some(&e_low))
            } else if streamed {
                crate::kern::outer_round_streamed(&basis, low, &e_low, &corners)
            } else {
                crate::kern::outer_round(&basis, &a, &b, &c, &e_low, i == 0 && assume_satisfied)
            }
        });
        let mut blind: F<C> = rng.scalar();
        let mut commitment = crate::prof::time("o.commit", || {
            commit_values(gens, outer_off[i], &values, blind)
        });
        match (cheat, i) {
            (Cheat::CancelRound, 0) => {
                commitment = C::from(gens.h) * blind;
                cancel_d0 = Some(commitment);
            }
            (Cheat::CancelRound, 1) => {
                // Retroactive q_0 = t_0 + J: Boolean sum 0, q_0(r_0) = t_0(r_0).
                let q0 = forged_round(&tail_values[0], &tau[..shape.outer_blocks()[0]], &rx[0]);
                let d0 = cancel_d0.expect("round 0 sent");
                commitment = commitment + commit_values(gens, outer_off[0], &q0, F::<C>::ZERO) - d0;
                blind -= round_blinds[0];
                tail_values[0] = q0;
            }
            (Cheat::AdaptiveWitness, 0) => {
                let (col, shift) = adaptive.expect("shift chosen");
                commitment += C::from(gens.g[col]) * shift;
            }
            _ => {}
        }
        let commitment = commitment.to_affine();
        t.point(b"outer", &commitment);
        let r: Vec<F<C>> = round_challenges(&mut t, k);
        let fold_clock = Instant::now();
        if old_kernels {
            a = fold_top(&a, &r);
            b = fold_top(&b, &r);
            c = fold_top(&c, &r);
        } else if streamed {
            [a, b, c] = crate::kern::fold_top_streamed(k, low, &r, &corners);
        } else {
            a = crate::kern::fold_top(&a, &r, None);
            b = crate::kern::fold_top(&b, &r, None);
            c = crate::kern::fold_top(&c, &r, None);
        }
        crate::prof::add("o.fold", fold_clock.elapsed().as_secs_f64());
        outer.push(commitment);
        tail_values.push(values);
        round_blinds.push(blind);
        rx.push(r);
        position += k;
    }
    let v = [a[0], b[0], c[0]];
    for value in &v {
        t.scalar(b"v", value);
    }
    let rabc: [F<C>; 3] = rabc_challenges(&mut t);
    // The gather's plan and index served only the outer rounds.
    rel.outer_done();
    timings.outer = clock.elapsed().as_secs_f64();
    mark("outer");

    // Inner sumcheck over z, committed rounds.
    let clock = Instant::now();
    // M(r_x, .) and z enter the first round split: dense witness halves and
    // public halves kept only up to their last nonzero entry. z's dense half
    // is the masked witness the opening reads.
    let mut m_first = Some(crate::prof::time("i.mtable", || {
        rel.m_table_split_at(shape, &rx.concat(), rabc[0], rabc[1], rabc[2])
    }));
    // The opening uses the folded matrix value from here on.
    drop(rel);
    mark("mtable");
    let (mut m, mut z): (Vec<F<C>>, Vec<F<C>>) = (Vec::new(), Vec::new());
    if old_kernels {
        m = m_first.take().expect("first M table").to_full();
        z = (0..2 * half).map(|i| zv.get(i)).collect();
    }
    // After a fold: which runs of the folded z may hold anything, by its
    // public layout (rows the build never wrote, the public half beyond the
    // io). The folded M is public; a private value is never tested for zero.
    let mut z_live: Option<crate::kern::Live> = None;
    let mut inner = Vec::new();
    let mut ry = Vec::new();
    let mut start = 0;
    for (i, &k) in shape.inner_blocks().iter().enumerate() {
        let extender = Extender::new(k);
        let basis = crate::kern::Basis::new(k);
        let (values, support) = if old_kernels {
            let values =
                crate::prof::time("i.round", || round_values(&extender, &m, &z, None, None));
            (values, None)
        } else if let Some(m_split) = &m_first {
            inner_values(&basis, k, m_split, &zv)
        } else {
            let live = z_live.as_ref().expect("the folded witness's live runs");
            inner_values(
                &basis,
                k,
                &crate::kern::Public(&m),
                &crate::kern::Masked::new(&z, live),
            )
        };
        let blind: F<C> = rng.scalar();
        let commitment = crate::prof::time("i.commit", || {
            commit_values(gens, inner_off[i], &values, blind).to_affine()
        });
        t.point(b"inner", &commitment);
        let r: Vec<F<C>> = inner_challenges(&mut t, shape, start, k);
        start += k;
        let fold_clock = Instant::now();
        if old_kernels {
            m = fold_top(&m, &r);
            z = fold_top(&z, &r);
        } else if let Some(m_split) = m_first.take() {
            m = crate::kern::fold_top(&m_split, &r, support.as_ref());
            drop(m_split);
            z = crate::kern::fold_top(&zv, &r, support.as_ref());
        } else {
            m = crate::kern::fold_top(&m, &r, support.as_ref());
            z = crate::kern::fold_top(&z, &r, support.as_ref());
        }
        if !old_kernels {
            z_live = Some(support.as_ref().map_or_else(
                || crate::kern::Live::dense(z.len()),
                crate::kern::Support::folded_live,
            ));
        }
        crate::prof::add("i.fold", fold_clock.elapsed().as_secs_f64());
        inner.push(commitment);
        tail_values.push(values);
        round_blinds.push(blind);
        ry.push(r);
    }
    let alpha: F<C> = t.challenge(b"alpha");
    let gamma: F<C> = gamma_challenge(&mut t, separated);
    timings.inner = clock.elapsed().as_secs_f64();
    mark("inner");

    // One opening IPA for the batched relation.
    let clock = Instant::now();
    let ch = Challenges {
        tau,
        rx,
        v,
        rabc,
        ry,
        alpha,
        gamma,
    };
    let clock_rel = Instant::now();
    // M(r_x, r_y): the folded table's one value (no matrix program run).
    assert_eq!(m.len(), 1, "the inner rounds bind every variable");
    let (weights, kappa, eq_row, _, _) = relation_at(shape, &ch, &inst.io, m[0]);
    timings.relation = clock_rel.elapsed().as_secs_f64();
    let powers = gamma_powers(gamma, tail_values.len());
    let x_clock = Instant::now();
    let mut x = vec![F::<C>::ZERO; cols];
    // Rows the caller never wrote: a zero row adds nothing, a padding row its
    // weight at column 0.
    let read: Vec<(F<C>, &[F<C>])> = (0..rows_n)
        .filter(|r| !zw.is_zero_row(*r) && !zw.is_unit_row(*r))
        .map(|r| (eq_row[r], zw.row(r)))
        .collect();
    let unit: F<C> = (0..rows_n)
        .filter(|r| zw.is_unit_row(*r))
        .map(|r| eq_row[r])
        .sum();
    x[..active]
        .par_iter_mut()
        .enumerate()
        .for_each(|(col, value)| {
            *value = read.iter().map(|(e, row)| *e * row[col]).sum();
        });
    // The opening scalar vector now holds the last contribution from the
    // masked witness rows. Erase owned rows before allocating IPA scratch.
    let owned_rows = zw.owned();
    zw.zeroize_owned();
    drop(zw);
    x[0] += unit;
    crate::prof::add("p.xrow", x_clock.elapsed().as_secs_f64());
    for ((offset, values), power) in outer_off
        .iter()
        .chain(&inner_off)
        .zip(&tail_values)
        .zip(&powers)
    {
        for (p, value) in slot_positions(grid_vars(values.len())).iter().zip(values) {
            x[offset + p] = *power * value;
        }
    }
    let blind: F<C> = eq_row
        .iter()
        .zip(&row_blinds)
        .map(|(e, r)| *e * r)
        .sum::<F<C>>()
        + round_blinds
            .iter()
            .zip(&powers)
            .map(|(b, p)| *b * p)
            .sum::<F<C>>();
    let rounds: Vec<Aff<C>> = outer.iter().chain(&inner).copied().collect();
    let p = crate::prof::time("p.combined", || {
        combined_commitment::<C>(&rows, &rounds, &eq_row, gamma)
    });
    let ipa = crate::prof::time("p.ipa", || {
        ipa::prove(gens, &mut t, &p, x, weights, kappa, blind, rng, claims)
    });
    timings.open = clock.elapsed().as_secs_f64();
    mark("open");
    if crate::prof::enabled() {
        println!(
            "prover time {} sx {} rows {}: {:.3} s",
            shape.label,
            shape.sx,
            shape.rows(),
            prove_clock.elapsed().as_secs_f64()
        );
    }
    if let Some(m) = mem {
        println!(
            "{}",
            m.finish(&format!(
                "{} sx {} sw {} rows {}",
                shape.label,
                shape.sx,
                shape.sw(),
                shape.rows()
            ))
        );
        println!("    prof ms: {}", crate::prof::line());
        println!(
            "prover accounting after {} sx {}: {}; prover-owned witness rows {} of {}",
            shape.label,
            shape.sx,
            circuit.memory_note(),
            owned_rows,
            rows_n
        );
    }

    rows.truncate(rows_c);
    (
        Proof {
            rows,
            late,
            outer,
            v,
            inner,
            ipa,
        },
        timings,
        link_out,
    )
}

/// One inner round over `(m, z)`: its values, and the support (which corners
/// of the round's variables may hold anything, from public information only)
/// its fold reuses. The public half of z is zero beyond the public inputs,
/// rows the build never wrote are empty, and M is public; the witness's
/// values never decide the schedule.
fn inner_values<S: Field, M: crate::kern::Table<S> + ?Sized, Z: crate::kern::Table<S> + ?Sized>(
    basis: &crate::kern::Basis,
    k: usize,
    m: &M,
    z: &Z,
) -> (Vec<S>, Option<crate::kern::Support>) {
    let support = (m.len() >> k >= crate::kern::SEGMENT)
        .then(|| crate::prof::time("i.support", || crate::kern::support(k, m, z)));
    let values = crate::prof::time("i.round", || {
        crate::kern::inner_round(basis, m, z, support.as_ref())
    });
    (values, support)
}

/// Honest-prover sanity check of the batched relation (used by tests).
pub fn relation_holds<C: PastaCurve>(
    shape: &Shape,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
) -> bool {
    let (a, b, c) = r1cs::abc(shape, &wit.w);
    let _ = inst;
    a.iter().zip(&b).zip(&c).all(|((a, b), c)| *a * b == *c)
}

pub fn verify<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
) -> Result<(), String> {
    verify_inner(
        shape,
        gens,
        inst,
        bytes,
        true,
        &[],
        &Link::None,
        &ToyCircuit,
        &[],
    )
    .map(|_| ())
}

pub fn verify_with_claims<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
    claims: &[Claim<C>],
) -> Result<(), String> {
    verify_inner(
        shape,
        gens,
        inst,
        bytes,
        true,
        claims,
        &Link::None,
        &ToyCircuit,
        &[],
    )
    .map(|_| ())
}

/// [`prove_linked_circuit`] of a witness exactly as its build wrote it:
/// `fill` ([`crate::uniform::UniformCircuit::build_rows`]) names the rows the
/// build never wrote, which the prover never reads. The proof is the same as
/// [`prove_linked_circuit`]'s of that witness.
#[allow(clippy::too_many_arguments)]
pub fn prove_linked_built<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    fill: &[r1cs::RowFill],
    rng: &mut SpikeRng,
    link: &Link,
    circuit: &dyn Circuit<F<C>>,
) -> (Proof<C>, [u8; 32], [u8; 32]) {
    let fixed = fixed_for(shape, gens, circuit, &inst.io);
    let (proof, _, out) = prove_rows(
        shape,
        gens,
        inst,
        wit,
        Some(fill),
        rng,
        Cheat::Honest,
        true,
        &[],
        link,
        circuit,
        &fixed,
        None,
    );
    let (word, r) = out.expect("linked proof");
    (proof, word, r)
}

/// Verifies a linked proof; returns the link's squeezed word (d or rho).
/// A linked circuit proof (`Link::First` or `Second`): the proof and the
/// link's squeezed word (d or rho) and R.
pub fn prove_linked_circuit<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    wit: &Witness<F<C>>,
    rng: &mut SpikeRng,
    link: &Link,
    circuit: &dyn Circuit<F<C>>,
) -> (Proof<C>, [u8; 32], [u8; 32]) {
    let fixed = fixed_for(shape, gens, circuit, &inst.io);
    let (proof, _, out) = prove_inner(
        shape,
        gens,
        inst,
        wit,
        rng,
        Cheat::Honest,
        true,
        &[],
        link,
        circuit,
        &fixed,
    );
    let (word, r) = out.expect("linked proof");
    (proof, word, r)
}

/// Verifies a linked circuit proof; returns the link's squeezed word.
pub fn verify_linked_circuit<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
    link: &Link,
    circuit: &dyn Circuit<F<C>>,
) -> Result<[u8; 32], String> {
    let fixed = fixed_for_verifier(shape, gens, circuit, &inst.io)?;
    verify_inner(shape, gens, inst, bytes, true, &[], link, circuit, &fixed)
        .map(|o| o.expect("linked").0)
}

pub fn verify_linked<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
    claims: &[Claim<C>],
    link: &Link,
) -> Result<[u8; 32], String> {
    verify_inner(
        shape,
        gens,
        inst,
        bytes,
        true,
        claims,
        link,
        &ToyCircuit,
        &[],
    )
    .map(|o| o.expect("linked").0)
}

pub fn verify_unseparated<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
) -> Result<(), String> {
    verify_inner(
        shape,
        gens,
        inst,
        bytes,
        false,
        &[],
        &Link::None,
        &ToyCircuit,
        &[],
    )
    .map(|_| ())
}

fn verify_inner<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    bytes: &[u8],
    separated: bool,
    claims: &[Claim<C>],
    link: &Link,
    circuit: &dyn Circuit<F<C>>,
    fixed: &[Aff<C>],
) -> Result<Option<([u8; 32], [u8; 32])>, String> {
    shape.check();
    let nf = circuit.fixed_rows(shape);
    if nf > shape.rows() || fixed.len() != nf || circuit.wallet_rows() > nf {
        return Err("invalid fixed-row commitments".into());
    }
    let wallet = wallet_commitments::<C>(circuit, &inst.io)?;
    if fixed.get(..wallet.len()) != Some(wallet.as_slice()) {
        return Err("cached wallet commitments do not match instance io".into());
    }
    let slots = circuit.late_slots();
    let mut proof =
        Proof::<C>::from_bytes_rows(shape, shape.rows() - fixed.len(), slots.len(), bytes)
            .ok_or("encoding")?;
    proof.rows.extend(fixed.iter().copied());
    let mut t = Transcript::<C>::new(shape.label.as_bytes());
    header(&mut t, shape, &inst.io);
    let p1 = circuit.phase1_rows(shape);
    for row in &proof.rows[..p1] {
        t.point(b"row", row);
    }
    let word = link_word(&mut t, link);
    let r_link = word.map(|(_, rho)| link_value(link, &rho, None));
    let link_io = match (word, r_link) {
        (Some((_, rho)), Some(r)) => circuit.link_io(&rho, &r),
        _ => Vec::new(),
    };
    let pl = circuit.link_rows(shape);
    if pl > 0 && word.is_none() {
        return Err("a link phase needs a link".into());
    }
    for row in &proof.rows[p1..p1 + pl] {
        t.point(b"row", row);
    }
    let ch2: Vec<F<C>> = (0..circuit.phase2_challenges())
        .map(|_| t.challenge_full(b"phase2"))
        .collect();
    let rel = circuit.instantiate(shape, &ch2, None);
    if !proof.late.is_empty() {
        t.scalar(b"late", &crate::fs::io_digest(&proof.late));
    }
    let io_full = io_with_late(&r1cs::io_with_link(&inst.io, &link_io), &slots, &proof.late);
    for row in &proof.rows[p1 + pl..] {
        t.point(b"row", row);
    }
    let link_out = link_r(&mut t, word, r_link);
    let tau: Vec<F<C>> = tau_challenges(&mut t, shape);
    let mut rx = Vec::new();
    for (commitment, &k) in proof.outer.iter().zip(&shape.outer_blocks()) {
        t.point(b"outer", commitment);
        rx.push(round_challenges(&mut t, k));
    }
    for value in &proof.v {
        t.scalar(b"v", value);
    }
    let rabc: [F<C>; 3] = rabc_challenges(&mut t);
    let mut ry = Vec::new();
    let mut start = 0;
    for (commitment, &k) in proof.inner.iter().zip(&shape.inner_blocks()) {
        t.point(b"inner", commitment);
        ry.push(inner_challenges(&mut t, shape, start, k));
        start += k;
    }
    let alpha: F<C> = t.challenge(b"alpha");
    let gamma: F<C> = gamma_challenge(&mut t, separated);
    let ch = Challenges {
        tau,
        rx,
        v: proof.v,
        rabc,
        ry,
        alpha,
        gamma,
    };
    let (weights, kappa, eq_row, coefficient, r_col) = relation(shape, &ch, &io_full, rel.as_ref());
    let rounds: Vec<Aff<C>> = proof.outer.iter().chain(&proof.inner).copied().collect();
    let p = combined_commitment::<C>(&proof.rows, &rounds, &eq_row, gamma);
    let (us, b_final) = ipa::verify(gens, &mut t, &p, weights, kappa, &proof.ipa, claims)?;
    // The circuits evaluate the closed form; it must equal the O(C) fold.
    assert!(
        b_final_slots(shape, &ch, coefficient, &r_col, &us) == b_final,
        "closed-form b_final"
    );
    Ok(link_out)
}

/// `q_0 = t_0 + J` with `sum_y eq(tau_0, y) q_0(y) = 0` and `q_0(r_0) = t_0(r_0)`:
/// `J` sits on the all-zero and all-two grid points.
fn forged_round<S: ff::PrimeField>(t0: &[S], tau0: &[S], r0: &[S]) -> Vec<S> {
    let k = tau0.len();
    let eq_tau = eq_table(tau0);
    let sum: S = boolean_positions(k)
        .iter()
        .zip(&eq_tau)
        .map(|(&g, e)| *e * t0[g])
        .sum();
    let weights = grid_weights(r0);
    let (zero, two) = (0, pow3(k) - 1);
    let j_zero = -sum * eq_tau[0].invert().expect("eq weight");
    let j_two = -weights[zero] * j_zero * weights[two].invert().expect("lagrange weight");
    let mut q0 = t0.to_vec();
    q0[zero] += j_zero;
    q0[two] += j_two;
    q0
}

fn shifted<S: Scalar>(shape: &Shape, w: &[S], col: usize, shift: S) -> Vec<S> {
    let mut out = w.to_vec();
    for r in 0..shape.rows() {
        out[r * shape.cols() + col] += shift;
    }
    out
}

fn weighted_error<S: Scalar>(shape: &Shape, w: &[S], eq_tau: &[S]) -> S {
    let (a, b, c) = r1cs::abc(shape, w);
    a.iter()
        .zip(&b)
        .zip(&c)
        .zip(eq_tau)
        .map(|(((a, b), c), e)| *e * (*a * b - c))
        .sum()
}

/// A column and shift `J` making `sum_x eq(tau, x) (Az Bz - Cz)(x) = 0`.
fn adaptive_shift<S: Scalar>(shape: &Shape, w: &[S], tau: &[S]) -> (usize, S) {
    let eq_tau = eq_table(tau);
    let two = S::from(2);
    for col in 0..shape.active_cols() {
        let e: Vec<S> = (0..3)
            .map(|j| weighted_error(shape, &shifted(shape, w, col, S::from(j)), &eq_tau))
            .collect();
        let quadratic = (e[2] - e[1].double() + e[0]) * two.invert().unwrap();
        let linear = e[1] - e[0] - quadratic;
        let root = if bool::from(quadratic.is_zero()) {
            linear.invert().into_option().map(|inv| -e[0] * inv)
        } else {
            let disc = linear.square() - quadratic * e[0] * S::from(4);
            disc.sqrt()
                .into_option()
                .map(|s| (s - linear) * (quadratic.double()).invert().unwrap())
        };
        if let Some(shift) = root {
            let check = weighted_error(shape, &shifted(shape, w, col, shift), &eq_tau);
            assert!(bool::from(check.is_zero()), "shift solves the quadratic");
            return (col, shift);
        }
    }
    panic!("no adaptive shift found");
}

#[cfg(test)]
mod wallet_input_tests {
    use super::*;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};

    /// A small real proof relation whose final witness row is supplied by
    /// the instance as a wallet commitment. No lookup/certificate fixture
    /// is needed to exercise the verifier's public-input boundary.
    struct WalletToy;

    impl<S: Scalar> Circuit<S> for WalletToy {
        fn phase1_rows(&self, shape: &Shape) -> usize {
            shape.rows() - 1
        }

        fn fixed_rows(&self, _shape: &Shape) -> usize {
            1
        }

        fn wallet_rows(&self) -> usize {
            1
        }

        fn wallet_io(&self) -> Option<usize> {
            Some(r1cs::NUM_PUBLIC)
        }

        fn instantiate<'a>(
            &'a self,
            _shape: &Shape,
            _ch: &[S],
            _w: Option<&mut [S]>,
        ) -> Box<dyn Rel<S> + 'a> {
            Box::new(r1cs::Toy)
        }
    }

    fn malformed_limbs<C: PastaCurve>(point: &Aff<C>) -> Vec<(&'static str, Vec<F<C>>)> {
        let good = limbs_of_point::<C>(point);
        let mut noncanonical = good;
        let mut modulus = (-C::Base::ONE).to_repr();
        for byte in modulus.as_mut() {
            let (sum, carry) = byte.overflowing_add(1);
            *byte = sum;
            if !carry {
                break;
            }
        }
        let bytes = modulus.as_ref();
        noncanonical[0] = F::<C>::from_u128(u128::from_le_bytes(bytes[..16].try_into().unwrap()));
        noncanonical[1] = F::<C>::from_u128(u128::from_le_bytes(bytes[16..].try_into().unwrap()));
        let mut wide = good;
        wide[0] = F::<C>::from_u128(u128::MAX) + F::<C>::ONE;
        vec![
            ("noncanonical coordinate", noncanonical.to_vec()),
            ("wide limb", wide.to_vec()),
            ("identity", vec![F::<C>::ZERO; 4]),
            (
                "off-curve coordinate",
                vec![F::<C>::ZERO, F::<C>::ZERO, F::<C>::ONE, F::<C>::ZERO],
            ),
        ]
    }

    fn check_point_inputs<C: PastaCurve>() {
        let point = Gens::<C>::new(2).g[0];
        let limbs = limbs_of_point::<C>(&point);
        assert_eq!(point_from_limbs::<C>(&limbs), Some(point));
        for length in 0..4 {
            assert!(point_from_limbs::<C>(&limbs[..length]).is_none());
        }
        let mut extra = limbs.to_vec();
        extra.push(F::<C>::ZERO);
        assert!(point_from_limbs::<C>(&extra).is_none(), "excess limb");
        for (label, malformed) in malformed_limbs::<C>(&point) {
            assert!(point_from_limbs::<C>(&malformed).is_none(), "{label}");
        }
    }

    #[test]
    fn wallet_point_decoder_rejects_malformed_limbs() {
        check_point_inputs::<Pallas>();
        check_point_inputs::<Vesta>();
    }

    #[test]
    fn wallet_verifiers_reject_malformed_io_and_stale_caches() {
        let shape = Shape {
            label: "wallet-input-boundary",
            sx: 10,
            row_bits: 2,
            col_bits: 8,
            k: 2,
        };
        let gens = Gens::<Vesta>::new(shape.cols());
        let mut rng = SpikeRng::new(b"wallet input boundary");
        let (mut inst, wit) = r1cs::sample(&shape, &mut rng);
        let last = (shape.rows() - 1) * shape.cols();
        let commitment = row_commitment(&shape, &gens, &wit.w[last..]);
        inst.io.extend(limbs_of_point::<Vesta>(&commitment));
        let fixed = fixed_for(&shape, &gens, &WalletToy, &inst.io);
        let (proof, _) = prove_circuit(&shape, &gens, &inst, &wit, &mut rng, &WalletToy);
        let bytes = proof.to_bytes();
        assert!(verify_circuit(&shape, &gens, &inst, &bytes, &WalletToy).is_ok());
        assert!(verify_circuit_with(&shape, &gens, &inst, &bytes, &WalletToy, &fixed).is_ok());
        let link = Link::First {
            rho: [1; 32],
            r: [2; 32],
        };
        let (linked, expected_d, _) =
            prove_linked_circuit(&shape, &gens, &inst, &wit, &mut rng, &link, &WalletToy);
        let linked_bytes = linked.to_bytes();
        assert_eq!(
            verify_linked_circuit(&shape, &gens, &inst, &linked_bytes, &link, &WalletToy),
            Ok(expected_d)
        );

        let rejected = |bad: &Instance<F<Vesta>>, label: &str| {
            assert!(
                verify_circuit(&shape, &gens, bad, &bytes, &WalletToy).is_err(),
                "uncached verifier: {label}"
            );
            assert!(
                verify_circuit_with(&shape, &gens, bad, &bytes, &WalletToy, &fixed).is_err(),
                "cached verifier: {label}"
            );
            assert!(
                verify_linked_circuit(&shape, &gens, bad, &linked_bytes, &link, &WalletToy)
                    .is_err(),
                "linked verifier: {label}"
            );
        };
        for length in 0..inst.io.len() {
            rejected(
                &Instance {
                    io: inst.io[..length].to_vec(),
                },
                "truncated io",
            );
        }
        for (label, malformed) in malformed_limbs::<Vesta>(&commitment) {
            let mut bad = inst.clone();
            bad.io[r1cs::NUM_PUBLIC..].copy_from_slice(&malformed);
            rejected(&bad, label);
        }

        let other = gens.g[0];
        assert_ne!(other, commitment);
        let stale = [other];
        assert_eq!(
            verify_circuit_with(&shape, &gens, &inst, &bytes, &WalletToy, &stale),
            Err("cached wallet commitments do not match instance io".into())
        );
        let mut other_inst = inst.clone();
        other_inst.io[r1cs::NUM_PUBLIC..].copy_from_slice(&limbs_of_point::<Vesta>(&other));
        assert_eq!(
            verify_circuit_with(&shape, &gens, &other_inst, &bytes, &WalletToy, &fixed),
            Err("cached wallet commitments do not match instance io".into())
        );
        for bad_cache in [Vec::new(), vec![commitment; shape.rows() + 1]] {
            assert!(
                verify_circuit_with(&shape, &gens, &inst, &bytes, &WalletToy, &bad_cache).is_err()
            );
        }
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};

    /// A shape with four width classes and full-width masking tails.
    fn small_shape(label: &'static str) -> Shape {
        let shape = Shape {
            label,
            sx: 8,
            row_bits: 2,
            col_bits: 10,
            k: 2,
        };
        shape.check();
        shape
    }

    /// The fixed-base table window the retired bucket-table commits used
    /// (Pixel 6, b04a57 11-09: c14 for P on Vesta, c13 for S on Pallas); a
    /// reference for the algebra only.
    fn reference_window<C: PastaCurve>() -> usize {
        let p = std::any::type_name::<C>() == std::any::type_name::<iroha_pasta::Eq>();
        if p { 14 } else { 13 }
    }

    /// Constant-time column sums equal the public MSM and the bucket table
    /// they replaced, on rows of every width class, including full-width and
    /// all-zero rows.
    fn same<C: PastaCurve>(cols: usize, label: &[u8]) {
        let gens = Gens::<C>::new(cols);
        let active = cols - 37;
        let mut rng = SpikeRng::new(label);
        let bounds = [255usize, 88, 11, 1, 0, 255, 32, 255];
        let rows: Vec<Vec<F<C>>> = bounds
            .iter()
            .map(|b| {
                (0..active)
                    .map(|_| match b {
                        0 => F::<C>::ZERO,
                        255 => rng.scalar(),
                        b => rng.below::<F<C>>(*b as u32),
                    })
                    .collect()
            })
            .collect();
        let columns: Vec<BoundedSecretScalars<F<C>>> = rows
            .iter()
            .zip(&bounds)
            .map(|(r, b)| BoundedSecretScalars::new(r, *b, None).unwrap())
            .collect();
        let sums = column_sums(&gens, active, &columns);
        let table = iroha_pasta::msm::FixedBaseTable::<C>::with_window(
            &gens.g[..active],
            reference_window::<C>(),
            MemoryBudget::DEFAULT,
        )
        .unwrap();
        let buckets = table
            .msm_secret_bounded_columns_cancellable(
                &columns,
                MemoryBudget::DEFAULT,
                &SharedMemoryBudget::process_default(),
                None,
            )
            .unwrap();
        assert_eq!(sums.len(), rows.len());
        for ((sum, bucket), row) in sums.iter().zip(&buckets).zip(&rows) {
            let public = msm_public::<C>(row, &gens.g[..active], MemoryBudget::DEFAULT).unwrap();
            assert_eq!(sum.to_affine(), public.to_affine(), "public MSM agrees");
            assert_eq!(*sum, *bucket, "bucket table agrees");
        }
    }

    #[test]
    fn constant_time_column_sums_equal_public_and_table_sums() {
        same::<Vesta>(1 << 12, b"table vesta");
        same::<Pallas>(1 << 11, b"table pallas");
    }

    /// Round commitments over the slot's grid generators and `H` equal the
    /// retired round-slot table's public MSM (window 9) and the plain public
    /// MSM over the same bases.
    fn rounds<C: PastaCurve>(label: &[u8]) {
        let shape = small_shape("constant-time rounds");
        let gens = Gens::<C>::new(shape.cols());
        let mut rng = SpikeRng::new(label);
        for slot in slots(&shape) {
            let values: Vec<F<C>> = (0..pow3(slot.k)).map(|_| rng.scalar()).collect();
            let blind: F<C> = rng.scalar();
            let actual = commit_values(&gens, slot.offset, &values, blind);
            let mut scalars = values.clone();
            scalars.push(blind);
            let mut bases: Vec<Aff<C>> = slot_positions(grid_vars(values.len()))
                .iter()
                .map(|p| gens.g[slot.offset + p])
                .collect();
            bases.push(gens.h);
            let table =
                iroha_pasta::msm::FixedBaseTable::<C>::with_window(&bases, 9, MemoryBudget::DEFAULT)
                    .unwrap();
            assert_eq!(
                actual,
                table.msm_public(&scalars, MemoryBudget::DEFAULT).unwrap()
            );
            assert_eq!(
                actual,
                msm_public::<C>(&scalars, &bases, MemoryBudget::DEFAULT).unwrap()
            );
        }
    }

    #[test]
    fn constant_time_round_commitments_equal_the_table_algebra() {
        rounds::<Vesta>(b"rounds vesta");
        rounds::<Pallas>(b"rounds pallas");
    }

    /// Single-row commitments (circuits without row bounds): the class-bounded
    /// head, the full-width masking tail and the blind, for honest rows and for
    /// a row with a value outside its class, which commits at full width.
    fn rows_without_bounds<C: PastaCurve>(label: &[u8]) {
        let shape = small_shape("constant-time single rows");
        let gens = Gens::<C>::new(shape.cols());
        let active = shape.active_cols();
        let mut rng = SpikeRng::new(label);
        for row in 0..shape.rows() {
            let bits = shape.row_bits_class(row) as u32;
            let honest: Vec<F<C>> = (0..active).map(|_| rng.below::<F<C>>(bits)).collect();
            let mut cheating = honest.clone();
            cheating[0] = -F::<C>::ONE;
            for values in [honest, cheating] {
                let blind: F<C> = rng.scalar();
                let expected = msm_public::<C>(&values, &gens.g[..active], MemoryBudget::DEFAULT)
                    .unwrap()
                    + C::from(gens.h) * blind;
                assert_eq!(
                    commit_row(&gens, &shape, row, &values, blind),
                    expected,
                    "row {row}"
                );
            }
        }
    }

    #[test]
    fn constant_time_single_row_commitments_on_both_curves() {
        rows_without_bounds::<Vesta>(b"single rows vesta");
        rows_without_bounds::<Pallas>(b"single rows pallas");
    }

    /// Bounded row lanes: every written row gets exactly its own sum and an
    /// unwritten row the blinded identity.
    fn lanes<C: PastaCurve>(label: &[u8]) {
        let shape = small_shape("constant-time lanes");
        let gens = Gens::<C>::new(shape.cols());
        let (cols, active) = (shape.cols(), shape.active_cols());
        let mut rng = SpikeRng::new(label);
        let bounds: Vec<usize> = (0..shape.rows())
            .map(|r| shape.row_bits_class(r))
            .collect();
        let written: Vec<Vec<F<C>>> = bounds
            .iter()
            .map(|&bits| {
                (0..cols)
                    .map(|c| {
                        if c < active {
                            rng.below::<F<C>>(bits as u32)
                        } else {
                            F::<C>::ZERO
                        }
                    })
                    .collect()
            })
            .collect();
        let unwritten: &[F<C>] = &[];
        let rows: Vec<&[F<C>]> = written
            .iter()
            .enumerate()
            .map(|(r, row)| if r == 1 { unwritten } else { row.as_slice() })
            .collect();
        let blinds: Vec<F<C>> = (0..shape.rows()).map(|_| rng.scalar()).collect();
        let actual = commit_row_slices(&gens, &shape, &rows, 0..shape.rows(), &bounds, &blinds, false);
        assert_eq!(actual.len(), shape.rows());
        for (r, point) in actual.iter().enumerate() {
            let sum = if r == 1 {
                C::identity()
            } else {
                msm_public::<C>(&written[r][..active], &gens.g[..active], MemoryBudget::DEFAULT)
                    .unwrap()
            };
            assert_eq!(
                *point,
                (sum + C::from(gens.h) * blinds[r]).to_affine(),
                "row {r}"
            );
        }
    }

    #[test]
    fn constant_time_row_lanes_on_both_curves() {
        lanes::<Vesta>(b"lanes vesta");
        lanes::<Pallas>(b"lanes pallas");
    }

    /// The opening's private sums equal the public MSM.
    #[test]
    fn constant_time_opening_sums_equal_the_public_msm() {
        let gens = Gens::<Vesta>::new(64);
        let mut rng = SpikeRng::new(b"constant-time opening");
        let scalars: Vec<F<Vesta>> = (0..64).map(|_| rng.scalar()).collect();
        assert_eq!(
            ipa::secret_msm::<Vesta>(&scalars, &gens.g),
            msm_public::<Vesta>(&scalars, &gens.g, MemoryBudget::DEFAULT).unwrap()
        );
    }

    /// Budget refusal, cancellation and malformed lengths are errors, never
    /// identity sums.
    fn refusals<C: PastaCurve>() {
        let gens = Gens::<C>::new(64);
        let zeros = vec![F::<C>::ZERO; 64];
        let ones = vec![F::<C>::ONE; 64];
        let columns = [
            BoundedSecretScalars::new(&ones, 1, None).unwrap(),
            BoundedSecretScalars::new(&zeros, 0, None).unwrap(),
        ];
        assert!(matches!(
            column_sums_with_budget(&gens, 64, &columns, MemoryBudget::new(0), None),
            Err(MsmError::Budget(_))
        ));
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            column_sums_with_budget(&gens, 64, &columns, MemoryBudget::DEFAULT, Some(&cancelled)),
            Err(MsmError::Cancelled)
        ));
        assert!(matches!(
            column_sums_with_budget(&gens, 63, &columns, MemoryBudget::DEFAULT, None),
            Err(MsmError::LengthMismatch(_))
        ));
        assert_eq!(
            column_sums(&gens, 64, &columns),
            vec![
                msm_public::<C>(&ones, &gens.g, MemoryBudget::DEFAULT).unwrap(),
                C::identity()
            ]
        );
    }

    #[test]
    fn constant_time_commitment_refusals_on_both_curves() {
        refusals::<Vesta>();
        refusals::<Pallas>();
    }
}

#[cfg(test)]
mod retained_phase1_tests {
    use super::*;

    /// A witness's rows as their content shows them: all zero, or (in the
    /// fixed tail) a padding row's single 1 in column 0. A prover told so
    /// never reads those rows.
    fn content_fill<S: Scalar>(shape: &Shape, fixed_rows: usize, w: &[S]) -> Vec<r1cs::RowFill> {
        let first_fixed = shape.rows() - fixed_rows;
        w.chunks(shape.cols())
            .enumerate()
            .map(|(r, row)| {
                if row.iter().all(|v| *v == S::ZERO) {
                    r1cs::RowFill::Zero
                } else if r >= first_fixed
                    && row[0] == S::ONE
                    && row[1..].iter().all(|v| *v == S::ZERO)
                {
                    r1cs::RowFill::Unit
                } else {
                    r1cs::RowFill::Values
                }
            })
            .collect()
    }

    /// The retained first phase against the two-pass prover it replaces, on a
    /// relation with product masks, a link phase, lookups (second-phase rows)
    /// and a fixed tail. Every later draw (round blinds, the opening's blinds)
    /// enters the proof bytes, so equal bytes also show the stream was
    /// consumed in the same order to its end.
    fn check<C: PastaCurve>(account: bool) {
        let (c, inst, wit) = phase1_tests::phased_fixture::<F<C>>(15);
        let shape = c.shape;
        let gens = Gens::<C>::new(shape.cols());
        let fill = if account {
            content_fill(&shape, c.fixed_rows(&shape), &wit.w)
        } else {
            vec![r1cs::RowFill::Values; shape.rows()]
        };
        let seed: &[u8] = b"retained first phase";
        let original = wit.w.clone();

        // The digest-only preparation, as the engine used it before.
        let mut two_pass = SpikeRng::new(seed);
        let d = prepare_linked_phase1_built(&shape, &gens, &inst, &wit, &fill, &mut two_pass, &c);
        let mut rng = SpikeRng::new(seed);
        let retained =
            prepare_linked_phase1_retained(&shape, &gens, &inst, &wit, &fill, &mut rng, &c);
        assert_eq!(retained.digest(), d, "the same first-phase digest");
        assert_eq!(
            rng.scalar::<F<C>>(),
            two_pass.scalar::<F<C>>(),
            "the caller's stream advances by the same prefix"
        );
        assert_eq!(wit.w, original, "the preparation leaves the witness");
        assert!(
            retained.retained_bytes() < shape.cols() * std::mem::size_of::<F<C>>(),
            "commitments and a stream reader, never a witness row"
        );

        // The second proof's rho and R, as the first proof is given them.
        let fallback = |_: &[u8; 32]| panic!("the actual link circuit supplies R");
        let (_, rho, r) = prove_linked_circuit(
            &shape,
            &gens,
            &inst,
            &wit,
            &mut SpikeRng::new(b"retained first phase, secondary"),
            &Link::Second { d, rlc: &fallback },
            &c,
        );
        let link = Link::First { rho, r };

        // Two passes over the first-phase rows, then one.
        let (reference, reference_d, reference_r) = prove_linked_built(
            &shape,
            &gens,
            &inst,
            &wit,
            &fill,
            &mut SpikeRng::new(seed),
            &link,
            &c,
        );
        let (proof, actual_d, actual_r) = retained.prove_first(rho, r);
        let bytes = proof.to_bytes();
        assert_eq!(bytes, reference.to_bytes(), "byte-identical proof");
        assert_eq!((actual_d, actual_r), (reference_d, reference_r));
        assert_eq!((actual_d, actual_r), (d, r), "the prepared digest and R");
        assert_eq!(wit.w, original, "the proof leaves the witness");
        assert_eq!(
            verify_linked_circuit(&shape, &gens, &inst, &bytes, &link, &c).unwrap(),
            d
        );

        // Other entropy gives other commitments, and a preparation that is
        // never proved is simply dropped (its stream reader wiped).
        let other = prepare_linked_phase1_retained(
            &shape,
            &gens,
            &inst,
            &wit,
            &fill,
            &mut SpikeRng::new(b"retained first phase, other entropy"),
            &c,
        );
        assert_ne!(other.digest(), d, "fresh hiding masks and blinds");
        drop(other);
    }

    #[test]
    fn retained_first_phase_proves_the_same_bytes_on_both_curves() {
        for account in [false, true] {
            check::<iroha_pasta::Ep>(account);
            check::<iroha_pasta::Eq>(account);
        }
    }
}
