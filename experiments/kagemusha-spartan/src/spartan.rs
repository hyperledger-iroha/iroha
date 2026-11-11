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
use iroha_pasta::PastaCurve;
use iroha_pasta::msm::{
    BoundedSecretScalars, MemoryBudget, SharedMemoryBudget, msm_public, msm_secret,
    msm_secret_bounded_cancellable,
};
use rayon::prelude::*;

use crate::fs::{Scalar, SpikeRng, Transcript};
use crate::grid::{
    Extender, boolean_positions, eq_table, grid_vars, grid_weights, lagrange3, pow3, pow4,
    slot_positions,
};
use crate::ipa::{self, Aff, Claim, F, Gens, IpaProof};
use crate::r1cs::{self, Circuit, Instance, Rel, Shape, ToyCircuit, Witness};

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

/// Applies a link after the rows; returns the squeezed word (d or rho) and R.
fn apply_link<C: PastaCurve>(t: &mut Transcript<C>, link: &Link) -> Option<([u8; 32], [u8; 32])> {
    match link {
        Link::None => None,
        Link::First { rho, r } => {
            let _ = t.challenge_full(b"pair/d");
            let d = t.last_word().to_repr();
            t.word_limbs(b"pair/rho", rho);
            t.word_limbs(b"pair/R", r);
            Some((d, *r))
        }
        Link::Second { d, rlc } => {
            t.word_limbs(b"pair/d", d);
            let _ = t.challenge_full(b"pair/rho");
            let rho = t.last_word().to_repr();
            let r = rlc(&rho);
            t.word_limbs(b"pair/R", &r);
            Some((rho, r))
        }
    }
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
    let rx: Vec<S> = ch.rx.concat();
    let ry: Vec<S> = ch.ry.concat();
    let (ma, mb, mc) = rel.matrix_evals(shape, &rx, &ry);
    let m_value = ch.rabc[0] * ma + ch.rabc[1] * mb + ch.rabc[2] * mc;
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

/// A round's grid values committed at its slot's base-4 positions.
fn commit_values<C: PastaCurve>(gens: &Gens<C>, offset: usize, values: &[F<C>], blind: F<C>) -> C {
    if ipa::old_kernels() {
        let bases: Vec<Aff<C>> = slot_positions(grid_vars(values.len()))
            .iter()
            .map(|p| gens.g[offset + p])
            .collect();
        return msm_secret::<C>(values, &bases, MemoryBudget::DEFAULT).expect("msm")
            + C::from(gens.h) * blind;
    }
    // The slot's fixed-base table holds its grid generators and H: one table
    // MSM gives the same group element.
    let table = gens.round_table(offset, values.len());
    let mut scalars = Vec::with_capacity(values.len() + 1);
    scalars.extend_from_slice(values);
    scalars.push(blind);
    table
        .msm_secret(&scalars, MemoryBudget::DEFAULT)
        .expect("table msm")
}

fn commit_row<C: PastaCurve>(
    gens: &Gens<C>,
    shape: &Shape,
    row: usize,
    values: &[F<C>],
    blind: F<C>,
) -> C {
    let split = values.len() - shape.row_full_tail(row);
    let shared = SharedMemoryBudget::process_default();
    // The width class is a prover-side speedup only; a value outside it (a
    // cheating prover in the tamper suite) is committed at full width.
    let mut sum =
        match BoundedSecretScalars::new(&values[..split], shape.row_bits_class(row), None) {
            Ok(bounded) => msm_secret_bounded_cancellable::<C>(
                &bounded,
                &gens.g[..split],
                MemoryBudget::DEFAULT,
                &shared,
                None,
            ),
            Err(_) => msm_secret::<C>(&values[..split], &gens.g[..split], MemoryBudget::DEFAULT),
        }
        .expect("msm");
    if split < values.len() {
        sum += msm_secret::<C>(
            &values[split..],
            &gens.g[split..values.len()],
            MemoryBudget::DEFAULT,
        )
        .expect("msm");
    }
    sum + C::from(gens.h) * blind
}

/// Seconds the row commits spent since the prover's last reset: checking each
/// row against its public bound, in the column MSM, and blinding plus the
/// affine conversion.
pub static COMMIT_SPLIT: std::sync::Mutex<[f64; 3]> = std::sync::Mutex::new([0.0; 3]);

/// Commits rows `range` of `w` in lanes (one single-threaded plan per row,
/// iroha_pasta's bounded columns API), each at its public bit bound.
/// Table windows (Pixel 6, b04a57 11-09): c14 for P on Vesta, c13 for S on
/// Pallas; `ZKS_TABLE_WINDOW_P` / `ZKS_TABLE_WINDOW_S` override them.
pub fn table_window<C: PastaCurve>() -> usize {
    let p = std::any::type_name::<C>() == std::any::type_name::<iroha_pasta::Eq>();
    let (var, default) = if p {
        ("ZKS_TABLE_WINDOW_P", 14)
    } else {
        ("ZKS_TABLE_WINDOW_S", 13)
    };
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Whether row commitments use the generators' fixed-base table (the
/// default) or the variable-base column MSM (`ZKS_COMMIT=variable`).
pub fn table_commits() -> bool {
    std::env::var("ZKS_COMMIT").map_or(true, |v| v != "variable")
}

/// Table build time spent inside commits (first use per shape), seconds.
pub static TABLE_BUILD: std::sync::Mutex<f64> = std::sync::Mutex::new(0.0);

/// Builds (or finds) the commitment table for `shape` before timing, and
/// returns its build time; zero when tables are off.
pub fn warm_table<C: PastaCurve>(gens: &Gens<C>, shape: &Shape) -> f64 {
    if !ipa::old_kernels() {
        // The round slots' tables are per-shape setup too.
        for slot in slots(shape) {
            let _ = gens.round_table(slot.offset, pow3(slot.k));
        }
    }
    if !table_commits() {
        return 0.0;
    }
    gens.table(shape.active_cols(), table_window::<C>()).1
}

/// `sum_j col[j] G_j` per bounded column, through the fixed-base table or the
/// variable-base column MSM; both give the exact same group elements.
pub(crate) fn column_sums<C: PastaCurve>(
    gens: &Gens<C>,
    active: usize,
    columns: &[BoundedSecretScalars<F<C>>],
    table: bool,
) -> Vec<C> {
    use iroha_pasta::msm::msm_secret_bounded_columns_cancellable;
    let shared = SharedMemoryBudget::process_default();
    if table {
        let (t, build) = gens.table(active, table_window::<C>());
        *TABLE_BUILD.lock().unwrap() += build;
        t.msm_secret_bounded_columns_cancellable(columns, MemoryBudget::DEFAULT, &shared, None)
            .expect("table msm")
    } else {
        msm_secret_bounded_columns_cancellable::<C>(
            columns,
            &gens.g[..active],
            MemoryBudget::DEFAULT,
            &shared,
            None,
        )
        .expect("msm")
    }
}

/// `sum_j row[j] G_j` for a row whose nonzero cells take few distinct values,
/// or `None` when they do not (fewer than three cells per value).
///
/// A LogUp helper row holds one value per table entry looked up
/// (`1 / (alpha - t)`), so thousands of cells share a few thousand values:
/// the real Receive's helper rows have 15,488 cells and 2,046 to 3,532
/// values. The generators of each value are summed first (one addition per
/// cell), then one variable-base MSM runs over the sums with the distinct
/// values. The result is the same group element as the direct sum, for about
/// half the work of the table commit (Mac: 28.0 to 14.5 ms for P-recv's five
/// helper rows).
///
/// Grouping depends on which cells are equal, as the bucket method's indices
/// depend on which digits are equal: it is as variable-time as the MSM. The
/// spike wipes none of its scratch; a wallet wipes the value index, too.
pub(crate) fn grouped_row_sum<C: PastaCurve>(gens: &Gens<C>, row: &[F<C>]) -> Option<C> {
    use std::collections::HashMap;
    let g_clock = Instant::now();
    let mut index: HashMap<[u8; 32], u32> = HashMap::new();
    let mut values: Vec<F<C>> = Vec::new();
    let mut members: Vec<(u32, u32)> = Vec::new();
    for (col, value) in row.iter().enumerate() {
        if bool::from(value.is_zero()) {
            continue;
        }
        let group = *index.entry(value.to_repr()).or_insert_with(|| {
            values.push(*value);
            (values.len() - 1) as u32
        });
        members.push((group, col as u32));
    }
    if std::env::var("ZKS_GROUPED").as_deref() == Ok("stats") {
        eprintln!(
            "grouped row: {} nonzero cells, {} distinct values",
            members.len(),
            values.len()
        );
    }
    // A value costs about 45 additions in the MSM over the sums and a cell
    // one, against about 21 per cell in the table commit: grouping pays from
    // about 2.3 cells per value; 3 leaves a margin.
    if members.len() < 256 || values.len() * 3 > members.len() {
        return None;
    }
    // Cells by value (a counting sort), then one sum per value.
    let mut starts = vec![0u32; values.len() + 1];
    for (group, _) in &members {
        starts[*group as usize + 1] += 1;
    }
    for g in 0..values.len() {
        starts[g + 1] += starts[g];
    }
    let mut next = starts.clone();
    let mut order = vec![0u32; members.len()];
    for (group, col) in &members {
        order[next[*group as usize] as usize] = *col;
        next[*group as usize] += 1;
    }
    crate::prof::add("g.hash", g_clock.elapsed().as_secs_f64());
    let g_clock = Instant::now();
    let sums: Vec<C> = (0..values.len())
        .into_par_iter()
        .with_min_len(16)
        .with_max_len(256)
        .map(|g| {
            let cells = &order[starts[g] as usize..starts[g + 1] as usize];
            cells[1..]
                .iter()
                .fold(C::from(gens.g[cells[0] as usize]), |acc, col| {
                    acc + gens.g[*col as usize]
                })
        })
        .collect();
    let points = iroha_pasta::curve::batch_normalize(&sums);
    crate::prof::add("g.sums", g_clock.elapsed().as_secs_f64());
    let g_clock = Instant::now();
    let sum = msm_secret::<C>(&values, &points, MemoryBudget::DEFAULT).expect("msm");
    crate::prof::add("g.msm", g_clock.elapsed().as_secs_f64());
    Some(sum)
}

/// A row at its public bit bound. The width class is a prover-side speedup: a
/// row outside its class (a cheating prover in the refusal tests) commits at
/// full width.
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
    let (cols, active) = (shape.cols(), shape.active_cols());
    let clock = Instant::now();
    // Second-phase rows (LogUp helpers): a row of few distinct values is
    // summed by value; the other rows go through the lanes as before.
    let grouped: Vec<Option<C>> =
        if helpers && !ipa::old_kernels() && std::env::var("ZKS_GROUPED").as_deref() != Ok("off") {
            range
                .clone()
                .into_par_iter()
                .with_max_len(1)
                .map(|r| grouped_row_sum(gens, &w[r * cols..r * cols + active]))
                .collect()
        } else {
            range.clone().map(|_| None).collect()
        };
    crate::prof::add("c.grouped", clock.elapsed().as_secs_f64());
    let lanes: Vec<(usize, &[F<C>])> = range
        .clone()
        .zip(&grouped)
        .filter(|(_, g)| g.is_none())
        .map(|(r, _)| (r, &w[r * cols..r * cols + active]))
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
    let lane_sums = column_sums(gens, active, &columns, table_commits());
    let mut sums: Vec<C> = grouped.iter().map(|g| g.unwrap_or(C::identity())).collect();
    for ((r, _), sum) in lanes.iter().zip(lane_sums) {
        sums[r - range.start] = sum;
    }
    let summed = clock.elapsed().as_secs_f64();
    let blinded = |(sum, r): (C, usize)| (sum + C::from(gens.h) * blinds[r]).to_affine();
    let out = if old {
        sums.into_iter().zip(range).map(blinded).collect()
    } else {
        sums.into_par_iter()
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
    let fixed = fixed_commitments(shape, gens, circuit);
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
        println!("    prof ms: {}", crate::prof::line());
        println!(
            "    proof blake3 {}",
            blake3::hash(&proof.to_bytes()).to_hex()
        );
    }
    (proof, timings)
}

/// The verification key's part for constant columns: one unblinded
/// commitment per trailing fixed row (computed once per circuit).
pub fn fixed_commitments<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    circuit: &dyn Circuit<F<C>>,
) -> Vec<Aff<C>> {
    let nf = circuit.fixed_rows(shape);
    if nf == 0 {
        return Vec::new();
    }
    let values = circuit.fixed_values(shape);
    let (cols, active) = (shape.cols(), shape.active_cols());
    assert_eq!(values.len(), nf * cols, "fixed rows are whole rows");
    (0..nf)
        .map(|k| {
            iroha_pasta::msm::msm_public::<C>(
                &values[k * cols..k * cols + active],
                &gens.g[..active],
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
    let fixed = fixed_commitments(shape, gens, circuit);
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

/// Verification with a precomputed key part for the fixed rows.
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
    shape.check();
    let (rows_n, cols, active) = (shape.rows(), shape.cols(), shape.active_cols());
    // Trailing fixed rows: committed in the key, blind 0, not in the proof.
    let rows_c = rows_n - fixed.len();
    let (outer_off, inner_off) = tail_offsets(shape);
    let mut timings = Timings::default();
    let mut t = Transcript::<C>::new(shape.label.as_bytes());
    header(&mut t, shape, &inst.io);
    let mut w = crate::prof::time("c.wclone", || wit.w.clone());

    // Hyrax rows: phase 1, then (after its challenges) phase 2.
    let clock = Instant::now();
    *COMMIT_SPLIT.lock().unwrap() = [0.0; 3];
    let row_blinds: Vec<F<C>> = (0..rows_n)
        .map(|r| {
            if r < rows_c {
                rng.scalar()
            } else {
                F::<C>::ZERO
            }
        })
        .collect();
    let bounds = circuit.row_bounds(shape);
    let commit = |w: &[F<C>], range: std::ops::Range<usize>, helpers: bool| -> Vec<Aff<C>> {
        match &bounds {
            Some(b) => commit_rows_lanes(gens, shape, w, range, b, &row_blinds, helpers),
            None => range
                .map(|r| {
                    commit_row(
                        gens,
                        shape,
                        r,
                        &w[r * cols..r * cols + active],
                        row_blinds[r],
                    )
                    .to_affine()
                })
                .collect(),
        }
    };
    let p1 = circuit.phase1_rows(shape);
    let mut rows: Vec<Aff<C>> = commit(&w, 0..p1, false);
    timings.phase1 = clock.elapsed().as_secs_f64();
    crate::prof::add("c.phase1", timings.phase1);
    for row in &rows {
        t.point(b"row", row);
    }
    let ch2: Vec<F<C>> = (0..circuit.phase2_challenges())
        .map(|_| t.challenge(b"phase2"))
        .collect();
    *crate::sparse::LAST_BUILD.lock().unwrap() = (0.0, 0.0);
    let clock_rel = Instant::now();
    let rel = circuit.instantiate(shape, &ch2, Some(&mut w));
    timings.instantiate = clock_rel.elapsed().as_secs_f64();
    // Late io: fixed by the second phase, absorbed before its rows.
    let late = circuit.late_values(&w);
    if !late.is_empty() {
        t.scalar(b"late", &crate::fs::io_digest(&late));
    }
    let inst_full = Instance {
        io: io_with_late(&inst.io, &circuit.late_slots(), &late),
    };
    let inst = &inst_full;
    (timings.helpers, timings.matrices) = *crate::sparse::LAST_BUILD.lock().unwrap();
    let rows2 = crate::prof::time("c.phase2", || {
        commit(&w, p1..rows_c, circuit.phase2_challenges() > 0)
    });
    for row in rows2.iter().chain(fixed) {
        t.point(b"row", row);
    }
    rows.extend(rows2);
    rows.extend(fixed.iter().copied());
    let link_out = apply_link(&mut t, link);
    timings.commit = clock.elapsed().as_secs_f64() - timings.instantiate;
    timings.split = *COMMIT_SPLIT.lock().unwrap();

    // Outer sumcheck, eq-factored, committed rounds.
    let clock = Instant::now();
    let tau: Vec<F<C>> = tau_challenges(&mut t, shape);
    // AdaptiveWitness: after tau, shift one witness column in every row by J
    // so that the tau-weighted error vanishes; carry J G[col] in round 0.
    let mut adaptive: Option<(usize, F<C>)> = None;
    let opened: Witness<F<C>> = match cheat {
        Cheat::AdaptiveWitness => {
            let (col, shift) = adaptive_shift(shape, &w, &tau);
            adaptive = Some((col, shift));
            Witness {
                w: shifted(shape, &w, col, shift),
            }
        }
        _ => Witness { w },
    };
    let wit = &opened;
    let z_full = crate::prof::time("o.zvec", || r1cs::z_vector(shape, inst, wit));
    let (mut a, mut b, mut c) = crate::prof::time("o.abc", || rel.abc(shape, &z_full));
    let mut cancel_d0: Option<C> = None;
    let mut tail_values: Vec<Vec<F<C>>> = Vec::new();
    let mut round_blinds: Vec<F<C>> = Vec::new();
    let mut outer = Vec::new();
    let mut rx: Vec<Vec<F<C>>> = Vec::new();
    let mut position = 0;
    let old_kernels = std::env::var("ZKS_KERN").as_deref() == Ok("old");
    // An honest prover's first outer round has zero Boolean points (the
    // relation holds on every row); `ZKS_OUTER=exact` computes them anyway.
    let assume_satisfied =
        cheat == Cheat::Honest && std::env::var("ZKS_OUTER").as_deref() != Ok("exact");
    for (i, &k) in shape.outer_blocks().iter().enumerate() {
        let extender = Extender::new(k);
        let basis = crate::kern::Basis::new(k);
        let e_low = crate::prof::time("o.eq", || eq_table(&tau[position + k..]));
        let values = crate::prof::time("o.round", || {
            if old_kernels {
                round_values(&extender, &a, &b, Some(&c), Some(&e_low))
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
    timings.outer = clock.elapsed().as_secs_f64();

    // Inner sumcheck over z, committed rounds.
    let clock = Instant::now();
    let ex = crate::prof::time("i.eq", || eq_table(&rx.concat()));
    let mut m = crate::prof::time("i.mtable", || {
        rel.m_table(shape, &ex, rabc[0], rabc[1], rabc[2])
    });
    drop(ex);
    let mut z = z_full;
    let mut inner = Vec::new();
    let mut ry = Vec::new();
    let mut start = 0;
    for (i, &k) in shape.inner_blocks().iter().enumerate() {
        let extender = Extender::new(k);
        let basis = crate::kern::Basis::new(k);
        // Which corners of this round's variables hold anything: the public
        // half of z is zero beyond the public inputs, and a padded witness
        // has empty rows.
        let support = (!old_kernels && m.len() >> k >= crate::kern::SEGMENT)
            .then(|| crate::prof::time("i.support", || crate::kern::support(k, &m, &z)));
        let values = crate::prof::time("i.round", || {
            if old_kernels {
                round_values(&extender, &m, &z, None, None)
            } else {
                crate::kern::inner_round(&basis, &m, &z, support.as_ref())
            }
        });
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
        } else {
            m = crate::kern::fold_top(&m, &r, support.as_ref());
            z = crate::kern::fold_top(&z, &r, support.as_ref());
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
    let (weights, kappa, eq_row, _, _) = relation(shape, &ch, &inst.io, rel.as_ref());
    timings.relation = clock_rel.elapsed().as_secs_f64();
    let powers = gamma_powers(gamma, tail_values.len());
    let x_clock = Instant::now();
    let mut x = vec![F::<C>::ZERO; cols];
    x[..active]
        .par_iter_mut()
        .enumerate()
        .for_each(|(col, value)| {
            *value = eq_row
                .iter()
                .enumerate()
                .map(|(r, e)| *e * wit.w[r * cols + col])
                .sum();
        });
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
    let fixed = fixed_commitments(shape, gens, circuit);
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
    let fixed = fixed_commitments(shape, gens, circuit);
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
    let ch2: Vec<F<C>> = (0..circuit.phase2_challenges())
        .map(|_| t.challenge(b"phase2"))
        .collect();
    let rel = circuit.instantiate(shape, &ch2, None);
    if !proof.late.is_empty() {
        t.scalar(b"late", &crate::fs::io_digest(&proof.late));
    }
    let io_full = io_with_late(&inst.io, &slots, &proof.late);
    for row in &proof.rows[p1..] {
        t.point(b"row", row);
    }
    let link_out = apply_link(&mut t, link);
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
mod table_tests {
    use super::*;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};

    /// Table commits equal variable-base commits on rows of every width
    /// class, including a full-width row and an all-zero row.
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
        let a = column_sums(&gens, active, &columns, true);
        let b = column_sums(&gens, active, &columns, false);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(
                x.to_affine(),
                y.to_affine(),
                "table and variable-base sums agree"
            );
        }
    }

    #[test]
    fn table_commits_equal_variable_base() {
        same::<Vesta>(1 << 12, b"table vesta");
        same::<Pallas>(1 << 11, b"table pallas");
    }
}
