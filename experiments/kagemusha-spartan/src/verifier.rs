//! Verifier synthesis (gate 1d): the in-circuit verification of one spike
//! proof, satisfied by real proofs from the sponge-transcript prover.
//!
//! The work splits by field, as CENSUS.md choice 1:
//! - points side (the circuit where the proof's points are native): the RP57
//!   transcript and its challenge decompositions, on-curve checks, the Hyrax
//!   row fold, the round-commitment Horner in gamma, the inherited claims, the
//!   IPA fold and the final Schnorr check;
//! - scalar side (the circuit where its scalars are native): endoscalar values,
//!   the batched relation (per-slot round weights, kappa), the closed-form
//!   `b_final` and each inherited claim's value (per-slot products over the
//!   base-4 round slots), the matrix MLE, the io digest and the Schnorr scalars.
//!
//! Every constraint is checked on insertion, and every intermediate point and
//! scalar is asserted equal to the native verifier's.
//!
//! Additions between independently chosen points check both chord-slope
//! equations (no free slope; p = -q is unsatisfiable). Incomplete additions
//! stay only where the inputs provably differ: inside Halo endoscaling (one
//! base, Halo's lemma), inside the variable-base multiplication (partial sums
//! in [2^i + 1, 3 2^i - 1], complete for the last three steps) and in the
//! fixed-base windows (digit offset 4, complete for the last two windows).

use std::collections::{BTreeMap, HashMap};

use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use group::{Curve, Group};
use iroha_pasta::poseidon::{FULL_ROUNDS, PARTIAL_ROUNDS, PoseidonField, RATE, WIDTH};
use iroha_pasta::{PastaAffine, PastaCurve};

use crate::fs::{
    CHALLENGE_BITS, Transcript, endo_scalar, full_scalar, io_digest, kron3, lean_schedule, powers2,
    raw_scalar,
};
use crate::ipa::{Aff, Claim, F, Gens, s_vector};
use crate::r1cs::{self, Instance, Shape};
use crate::spartan::{self, Link, Proof};

type V = usize;

// ---------------------------------------------------------------------------
// Constraint system with per-section counts.

#[derive(Clone, Debug, Default)]
pub struct Lc<S>(Vec<(V, S)>);

impl<S: Field> Lc<S> {
    fn var(v: V) -> Self {
        Lc(vec![(v, S::ONE)])
    }
    fn constant(c: S) -> Self {
        if c == S::ZERO {
            Lc(Vec::new())
        } else {
            Lc(vec![(0, c)])
        }
    }
    fn plus(&self, o: &Lc<S>) -> Lc<S> {
        let mut t = self.0.clone();
        t.extend_from_slice(&o.0);
        Lc(t).compact()
    }
    fn minus(&self, o: &Lc<S>) -> Lc<S> {
        self.plus(&o.scale(-S::ONE))
    }
    fn scale(&self, s: S) -> Lc<S> {
        Lc(self.0.iter().map(|(v, c)| (*v, *c * s)).collect()).compact()
    }
    fn compact(mut self) -> Lc<S> {
        self.0.sort_by_key(|(v, _)| *v);
        let mut out: Vec<(V, S)> = Vec::with_capacity(self.0.len());
        for (v, c) in self.0 {
            match out.last_mut() {
                Some((last, sum)) if *last == v => *sum += c,
                _ => out.push((v, c)),
            }
        }
        out.retain(|(_, c)| *c != S::ZERO);
        Lc(out)
    }
    fn is_constant(&self) -> bool {
        self.0.iter().all(|(v, _)| *v == 0)
    }
}

/// Records every constraint and each variable's section so that a run can
/// perturb each witness cell alone and check some constraint fails (the
/// assigned-cell mutation test).
pub static MUTATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Exports each finished side (values, widths, rows, counted rows) for the
/// real-circuit assembly.
pub static EXPORT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SINK: std::sync::Mutex<Vec<Box<dyn std::any::Any + Send>>> =
    std::sync::Mutex::new(Vec::new());

pub struct Exported<S> {
    pub vals: Vec<S>,
    pub widths: Vec<u16>,
    pub rows: Vec<(Vec<(usize, S)>, Vec<(usize, S)>, Vec<(usize, S)>)>,
    pub counted: usize,
    /// Gadget scope of each row (`blocks`).
    pub row_scope: Vec<u32>,
}

pub fn take_exports() -> Vec<Box<dyn std::any::Any + Send>> {
    std::mem::take(&mut SINK.lock().unwrap())
}

pub struct Cs<S> {
    vals: Vec<S>,
    var_width: Vec<u16>,
    counted_rows: usize,
    var_section: Vec<&'static str>,
    rows: Vec<(Lc<S>, Lc<S>, Lc<S>)>,
    row_scope: Vec<u32>,
    record: bool,
    section: &'static str,
    pub counts: BTreeMap<&'static str, usize>,
    /// One-bit witness entries (committed at 1-bit width).
    pub bits: usize,
    pub vars: usize,
}

impl<S: PrimeField<Repr = [u8; 32]>> Cs<S> {
    fn new() -> Self {
        let record = MUTATE.load(std::sync::atomic::Ordering::Relaxed)
            || EXPORT.load(std::sync::atomic::Ordering::Relaxed);
        Cs {
            vals: vec![S::ONE],
            var_width: vec![0],
            counted_rows: 0,
            var_section: vec!["one"],
            rows: Vec::new(),
            row_scope: Vec::new(),
            record,
            section: "other",
            counts: BTreeMap::new(),
            bits: 0,
            vars: 0,
        }
    }
    fn section(&mut self, s: &'static str) {
        self.section = s;
    }
    fn count(&mut self, n: usize) {
        *self.counts.entry(self.section).or_insert(0) += n;
    }
    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }
    fn alloc(&mut self, v: S) -> Lc<S> {
        self.vals.push(v);
        self.var_width.push(255);
        self.var_section.push(self.section);
        self.vars += 1;
        Lc::var(self.vals.len() - 1)
    }
    fn val(&self, lc: &Lc<S>) -> S {
        lc.0.iter().map(|(v, c)| self.vals[*v] * c).sum()
    }
    fn enforce(&mut self, a: &Lc<S>, b: &Lc<S>, c: &Lc<S>) {
        assert!(
            self.val(a) * self.val(b) == self.val(c),
            "unsatisfied constraint in {}",
            self.section
        );
        if self.record {
            self.rows.push((a.clone(), b.clone(), c.clone()));
            self.row_scope.push(crate::blocks::current());
        }
        self.count(1);
    }
    fn enforce_zero(&mut self, a: &Lc<S>) {
        assert!(
            self.val(a) == S::ZERO,
            "unsatisfied linear constraint in {}",
            self.section
        );
        if self.record {
            self.rows
                .push((a.clone(), Lc::constant(S::ONE), Lc(Vec::new())));
            self.row_scope.push(crate::blocks::current());
        }
        self.count(1);
    }

    /// Perturbs each variable alone by a random nonzero amount and checks
    /// that some recorded constraint containing it fails; returns the cells
    /// that stay satisfied, by section.
    pub fn mutation_test(&mut self) -> BTreeMap<&'static str, usize> {
        let mut index: Vec<Vec<usize>> = vec![Vec::new(); self.vals.len()];
        for (i, (a, b, c)) in self.rows.iter().enumerate() {
            for lc in [a, b, c] {
                for (v, _) in &lc.0 {
                    if index[*v].last() != Some(&i) {
                        index[*v].push(i);
                    }
                }
            }
        }
        let delta = S::from(0x9e37_79b9_7f4a_7c15_u64).square() + S::from(7);
        let mut free = BTreeMap::new();
        for v in 1..self.vals.len() {
            let saved = self.vals[v];
            self.vals[v] = saved + delta;
            let caught = index[v].iter().any(|&i| {
                let (a, b, c) = &self.rows[i];
                self.val(a) * self.val(b) != self.val(c)
            });
            self.vals[v] = saved;
            if !caught {
                *free.entry(self.var_section[v]).or_insert(0) += 1;
            }
        }
        free
    }
    fn mul(&mut self, a: &Lc<S>, b: &Lc<S>) -> Lc<S> {
        if a.is_constant() {
            return b.scale(self.val(a));
        }
        if b.is_constant() {
            return a.scale(self.val(b));
        }
        let v = self.alloc(self.val(a) * self.val(b));
        self.enforce(a, b, &v);
        v
    }
    fn inv(&mut self, a: &Lc<S>) -> Lc<S> {
        let v = self.alloc(self.val(a).invert().expect("nonzero"));
        self.enforce(a, &v, &Lc::constant(S::ONE));
        v
    }
    fn boolean(&mut self, bit: bool) -> Lc<S> {
        let v = self.alloc(if bit { S::ONE } else { S::ZERO });
        *self.var_width.last_mut().unwrap() = 1;
        self.enforce(&v, &v, &v);
        self.bits += 1;
        v
    }
    fn counted(&mut self, n: usize) {
        self.count(n);
        self.counted_rows += n;
    }
    /// Canonical little-endian bits: booleans, recomposition, and the
    /// comparison with the modulus counted at one row per bit.
    fn canonical_bits(&mut self, x: &Lc<S>) -> Vec<Lc<S>> {
        let _scope = crate::blocks::scope("canonical_bits");
        let repr = self.val(x).to_repr();
        let n = S::NUM_BITS as usize;
        let bits: Vec<Lc<S>> = (0..n)
            .map(|i| self.boolean((repr[i / 8] >> (i % 8)) & 1 == 1))
            .collect();
        let sum = recompose(&bits);
        self.enforce_zero(&sum.minus(x));
        self.counted(n);
        bits
    }
}

fn recompose<S: Field>(bits: &[Lc<S>]) -> Lc<S> {
    let mut sum = Lc(Vec::new());
    let mut power = S::ONE;
    for b in bits {
        sum = sum.plus(&b.scale(power));
        power = power.double();
    }
    sum
}

fn pow2<S: PrimeField>(k: usize) -> S {
    S::from(2).pow_vartime([k as u64])
}

/// RP57 permutation on a state of linear combinations (243 rows).
fn permute_gadget<S: PoseidonField + PrimeField<Repr = [u8; 32]>>(
    cs: &mut Cs<S>,
    state: &mut [Lc<S>; WIDTH],
) {
    let _scope = crate::blocks::scope("permute");
    let params = S::rp57();
    let half = FULL_ROUNDS / 2;
    for (round, rc) in params.round_constants().iter().enumerate() {
        let _step = crate::blocks::scope("permute_round");
        for (w, c) in state.iter_mut().zip(rc) {
            *w = w.plus(&Lc::constant(*c));
        }
        let full = round < half || round >= half + PARTIAL_ROUNDS;
        let lanes = if full { WIDTH } else { 1 };
        for w in state.iter_mut().take(lanes) {
            let x2 = cs.mul(w, w);
            let x4 = cs.mul(&x2, &x2);
            *w = cs.mul(&x4, w);
        }
        let mds = params.mds();
        let next: Vec<Lc<S>> = (0..WIDTH)
            .map(|i| {
                state
                    .iter()
                    .zip(mds[i].iter())
                    .fold(Lc(Vec::new()), |acc, (w, m)| acc.plus(&w.scale(*m)))
            })
            .collect();
        *state = [next[0].clone(), next[1].clone(), next[2].clone()];
    }
}

/// A challenge as the points side holds it: its bits (decomposed squeeze),
/// or only its value (squeeze range-checked by lookups).
#[derive(Clone)]
enum Ch<S> {
    Bits(Vec<Lc<S>>),
    Value(Lc<S>),
    /// A full-width word as two 128-bit limbs.
    Full(Lc<S>, Lc<S>),
}

/// The transcript's duplex sponge in-circuit (same schedule as fs::Transcript).
struct Sponge<S> {
    state: [Lc<S>; WIDTH],
    pending: usize,
    spare: Option<Ch<S>>,
    permutations: usize,
    squeezes: usize,
    lookup_squeezes: usize,
}

impl<S: PoseidonField + PrimeField<Repr = [u8; 32]>> Sponge<S> {
    fn new(snapshot: ([S; WIDTH], usize)) -> Self {
        let (st, pending) = snapshot;
        Sponge {
            state: [
                Lc::constant(st[0]),
                Lc::constant(st[1]),
                Lc::constant(st[2]),
            ],
            pending,
            spare: None,
            permutations: 0,
            squeezes: 0,
            lookup_squeezes: 0,
        }
    }
    fn permute(&mut self, cs: &mut Cs<S>) {
        permute_gadget(cs, &mut self.state);
        self.permutations += 1;
    }
    fn absorb(&mut self, cs: &mut Cs<S>, x: &Lc<S>) {
        if self.pending == RATE {
            self.permute(cs);
            self.pending = 0;
        }
        self.state[self.pending] = self.state[self.pending].plus(x);
        self.pending += 1;
        self.spare = None;
    }
    /// The next 126-bit challenge. A fresh squeeze is decomposed into bits
    /// when `bits` (some half is an endoscalar), else split as
    /// c1 + 2^126 c2 + 2^252 c3 with c1, c2 range-checked by lookups (counted:
    /// ten 13-bit lookups each), c3 by three booleans, and the comparison with
    /// the modulus counted at 20 rows.
    fn challenge(&mut self, cs: &mut Cs<S>, bits: bool) -> Ch<S> {
        let _scope = crate::blocks::scope("challenge");
        if let Some(ch) = self.spare.take() {
            return ch;
        }
        self.permute(cs);
        self.pending = 0;
        self.squeezes += 1;
        let s0 = self.state[0].clone();
        if bits {
            let bits = cs.canonical_bits(&s0);
            self.spare = Some(Ch::Bits(bits[CHALLENGE_BITS..2 * CHALLENGE_BITS].to_vec()));
            return Ch::Bits(bits[..CHALLENGE_BITS].to_vec());
        }
        self.lookup_squeezes += 1;
        let repr = cs.val(&s0).to_repr();
        let part = |lo: usize, hi: usize| -> u128 {
            (lo..hi).fold(0_u128, |acc, i| {
                acc | ((((repr[i / 8] >> (i % 8)) & 1) as u128) << (i - lo))
            })
        };
        let c1 = cs.alloc(S::from_u128(part(0, 126)));
        let c2 = cs.alloc(S::from_u128(part(126, 252)));
        let c3: Vec<Lc<S>> = (252..255)
            .map(|i| cs.boolean((repr[i / 8] >> (i % 8)) & 1 == 1))
            .collect();
        let rec = c1
            .plus(&c2.scale(pow2(126)))
            .plus(&recompose(&c3).scale(pow2(252)));
        cs.enforce_zero(&rec.minus(&s0));
        cs.counted(20 + 20);
        self.spare = Some(Ch::Value(c2));
        Ch::Value(c1)
    }

    /// A full-width challenge: the squeezed word as two 128-bit limbs, range-
    /// checked by lookups (counted, 20) with the modulus comparison (20).
    fn challenge_full(&mut self, cs: &mut Cs<S>) -> Ch<S> {
        let _scope = crate::blocks::scope("challenge_full");
        self.permute(cs);
        self.pending = 0;
        self.squeezes += 1;
        self.lookup_squeezes += 1;
        self.spare = None;
        let s0 = self.state[0].clone();
        let repr = cs.val(&s0).to_repr();
        let lo = cs.alloc(S::from_u128(u128::from_le_bytes(
            repr[..16].try_into().unwrap(),
        )));
        let hi = cs.alloc(S::from_u128(u128::from_le_bytes(
            repr[16..].try_into().unwrap(),
        )));
        cs.enforce_zero(&lo.plus(&hi.scale(pow2(128))).minus(&s0));
        cs.counted(20 + 20);
        Ch::Full(lo, hi)
    }
}

// ---------------------------------------------------------------------------
// Points side gadgets (native curve arithmetic over C::Base).

#[derive(Clone)]
struct Pt<S> {
    x: Lc<S>,
    y: Lc<S>,
}

fn coords<C: PastaCurve>(p: &C) -> (C::Base, C::Base) {
    Option::from(p.to_affine().coordinates()).expect("finite point")
}

fn beta<C: PastaCurve>() -> C::Base {
    let g = C::generator();
    let (x, _) = coords(&g);
    let (xe, _) = coords(&g.endo());
    xe * x.invert().unwrap()
}

/// A proof point: coordinates and the on-curve check (3 rows).
fn pt_input<C: PastaCurve>(cs: &mut Cs<C::Base>, p: &Aff<C>) -> Pt<C::Base> {
    let (x, y): (C::Base, C::Base) =
        Option::from(p.coordinates()).expect("non-identity proof point");
    let (xv, yv) = (cs.alloc(x), cs.alloc(y));
    let xx = cs.mul(&xv, &xv);
    let xxx = cs.mul(&xx, &xv);
    cs.enforce(&yv, &yv, &xxx.plus(&Lc::constant(C::b())));
    Pt { x: xv, y: yv }
}

fn pt_const<C: PastaCurve>(p: &C) -> Pt<C::Base> {
    let (x, y) = coords(p);
    Pt {
        x: Lc::constant(x),
        y: Lc::constant(y),
    }
}

fn pt_check<C: PastaCurve>(cs: &Cs<C::Base>, pt: &Pt<C::Base>, native: &C, what: &str) {
    let (x, y) = coords(native);
    assert!(
        cs.val(&pt.x) == x && cs.val(&pt.y) == y,
        "{what} matches the native verifier"
    );
}

fn neg<S: Field>(p: &Pt<S>) -> Pt<S> {
    Pt {
        x: p.x.clone(),
        y: p.y.scale(-S::ONE),
    }
}

/// Chord addition (3 rows) where the inputs provably differ.
fn add_incomplete<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, p: &Pt<S>, q: &Pt<S>) -> Pt<S> {
    let _scope = crate::blocks::scope("add_incomplete");
    let (xp, yp, xq, yq) = (cs.val(&p.x), cs.val(&p.y), cs.val(&q.x), cs.val(&q.y));
    let lam = (yq - yp) * (xq - xp).invert().expect("incomplete addition: distinct x");
    let l = cs.alloc(lam);
    cs.enforce(&l, &q.x.minus(&p.x), &q.y.minus(&p.y));
    let x3 = cs.alloc(lam.square() - xp - xq);
    cs.enforce(&l, &l, &x3.plus(&p.x).plus(&q.x));
    let y3 = cs.alloc(lam * (xp - cs.val(&x3)) - yp);
    cs.enforce(&l, &p.x.minus(&x3), &y3.plus(&p.y));
    Pt { x: x3, y: y3 }
}

/// Addition checking both chord-slope equations (a = 0, 6 rows):
/// lam (xq - xp) = yq - yp and lam (yp + yq) = xp^2 + xp xq + xq^2.
fn add<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, p: &Pt<S>, q: &Pt<S>) -> Pt<S> {
    let _scope = crate::blocks::scope("add");
    let (xp, yp, xq, yq) = (cs.val(&p.x), cs.val(&p.y), cs.val(&q.x), cs.val(&q.y));
    let lam = if xp != xq {
        (yq - yp) * (xq - xp).invert().unwrap()
    } else {
        assert!(yp == yq, "p = -q is unsatisfiable");
        xp.square() * S::from(3) * (yp.double()).invert().unwrap()
    };
    let l = cs.alloc(lam);
    cs.enforce(&l, &q.x.minus(&p.x), &q.y.minus(&p.y));
    let s = p.x.plus(&q.x);
    let sq = cs.mul(&s, &s);
    let pq = cs.mul(&p.x, &q.x);
    cs.enforce(&l, &p.y.plus(&q.y), &sq.minus(&pq));
    let x3 = cs.alloc(lam.square() - xp - xq);
    cs.enforce(&l, &l, &x3.plus(&p.x).plus(&q.x));
    let y3 = cs.alloc(lam * (xp - cs.val(&x3)) - yp);
    cs.enforce(&l, &p.x.minus(&x3), &y3.plus(&p.y));
    Pt { x: x3, y: y3 }
}

fn double<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, p: &Pt<S>) -> Pt<S> {
    let _scope = crate::blocks::scope("double");
    let (x, y) = (cs.val(&p.x), cs.val(&p.y));
    let xx = cs.mul(&p.x, &p.x);
    let lam = (x.square() * S::from(3)) * (y.double()).invert().expect("y != 0");
    let l = cs.alloc(lam);
    cs.enforce(&l, &p.y.scale(S::from(2)), &xx.scale(S::from(3)));
    let x3 = cs.alloc(lam.square() - x.double());
    cs.enforce(&l, &l, &x3.plus(&p.x.scale(S::from(2))));
    let y3 = cs.alloc(lam * (x - cs.val(&x3)) - y);
    cs.enforce(&l, &p.x.minus(&x3), &y3.plus(&p.y));
    Pt { x: x3, y: y3 }
}

/// `2 acc + s` with five rows (incomplete; inputs provably differ).
fn double_add<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, acc: &Pt<S>, s: &Pt<S>) -> Pt<S> {
    let _scope = crate::blocks::scope("double_add");
    let (xa, ya, xs, ys) = (cs.val(&acc.x), cs.val(&acc.y), cs.val(&s.x), cs.val(&s.y));
    let l1 = (ys - ya) * (xs - xa).invert().expect("distinct x");
    let l1v = cs.alloc(l1);
    cs.enforce(&l1v, &s.x.minus(&acc.x), &s.y.minus(&acc.y));
    let xr = l1.square() - xa - xs;
    let xrv = cs.alloc(xr);
    cs.enforce(&l1v, &l1v, &xrv.plus(&acc.x).plus(&s.x));
    let l2 = ya.double() * (xa - xr).invert().expect("distinct x") - l1;
    let l2v = cs.alloc(l2);
    cs.enforce(
        &l1v.plus(&l2v),
        &acc.x.minus(&xrv),
        &acc.y.scale(S::from(2)),
    );
    let xo = l2.square() - xr - xa;
    let xov = cs.alloc(xo);
    cs.enforce(&l2v, &l2v, &xov.plus(&xrv).plus(&acc.x));
    let yov = cs.alloc(l2 * (xa - xo) - ya);
    cs.enforce(&l2v, &acc.x.minus(&xov), &yov.plus(&acc.y));
    Pt { x: xov, y: yov }
}

/// Halo endoscaling [r] P with existing challenge bits (pairs: sign, endo).
fn endoscale<C: PastaCurve>(
    cs: &mut Cs<C::Base>,
    p: &Pt<C::Base>,
    bits: &[Lc<C::Base>],
    beta: C::Base,
) -> Pt<C::Base> {
    let _scope = crate::blocks::scope("endoscale");
    let one = C::Base::ONE;
    let phi = Pt {
        x: p.x.scale(beta),
        y: p.y.clone(),
    };
    let sum = add_incomplete(cs, &phi, p);
    let mut acc = double(cs, &sum);
    let (x, y) = (cs.val(&p.x), cs.val(&p.y));
    for i in (0..bits.len() / 2).rev() {
        let _step = crate::blocks::scope("endoscale_step");
        let (sv, ev) = (&bits[2 * i], &bits[2 * i + 1]);
        let (sign, endo) = (cs.val(sv) == one, cs.val(ev) == one);
        let xs = cs.alloc(if endo { x * beta } else { x });
        cs.enforce(ev, &p.x.scale(beta - one), &xs.minus(&p.x));
        let ys = cs.alloc(if sign { y } else { -y });
        cs.enforce(
            &sv.scale(C::Base::from(2)).minus(&Lc::constant(one)),
            &p.y,
            &ys,
        );
        acc = double_add(cs, &acc, &Pt { x: xs, y: ys });
    }
    acc
}

/// z P for z = 2 c + 2^n + 1 (mod order) from c's n bits: acc = 2 P, then
/// acc = 2 acc + (2 c_i - 1) P from the top bit; complete for the last three.
fn var_mul<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, p: &Pt<S>, cbits: &[Lc<S>]) -> Pt<S> {
    let _scope = crate::blocks::scope("var_mul");
    let y = cs.val(&p.y);
    let mut acc = double(cs, p);
    for i in (0..cbits.len()).rev() {
        let _step = crate::blocks::scope("var_mul_step");
        let positive = cs.val(&cbits[i]) == S::ONE;
        let ys = cs.alloc(if positive { y } else { -y });
        cs.enforce(
            &cbits[i].scale(S::from(2)).minus(&Lc::constant(S::ONE)),
            &p.y,
            &ys,
        );
        let s = Pt {
            x: p.x.clone(),
            y: ys,
        };
        acc = if i >= 3 {
            double_add(cs, &acc, &s)
        } else {
            let d = double(cs, &acc);
            add(cs, &d, &s)
        };
    }
    acc
}

/// k B for a constant base from k's bits: 2-bit windows over (d + 4) 4^j B;
/// complete additions for the last two windows and the offset removal.
fn fixed_mul<C: PastaCurve>(cs: &mut Cs<C::Base>, base: &C, bits: &[Lc<C::Base>]) -> Pt<C::Base> {
    let _scope = crate::blocks::scope("fixed_mul");
    let windows = bits.len().div_ceil(2);
    let zero = Lc(Vec::new());
    let mut acc: Option<Pt<C::Base>> = None;
    let mut offset = C::identity();
    let mut step = *base;
    for j in 0..windows {
        let _step = crate::blocks::scope("fixed_mul_step");
        let table: Vec<(C::Base, C::Base)> = (0..4_u64)
            .map(|d| coords(&(step * C::ScalarExt::from(d + 4))))
            .collect();
        offset += step * C::ScalarExt::from(4);
        let b0 = bits[2 * j].clone();
        let b1 = bits.get(2 * j + 1).cloned().unwrap_or(zero.clone());
        let both = cs.mul(&b0, &b1);
        let pick = |c: [C::Base; 4]| -> Lc<C::Base> {
            Lc::constant(c[0])
                .plus(&b0.scale(c[1] - c[0]))
                .plus(&b1.scale(c[2] - c[0]))
                .plus(&both.scale(c[3] - c[2] - c[1] + c[0]))
        };
        let sel = Pt {
            x: pick([table[0].0, table[1].0, table[2].0, table[3].0]),
            y: pick([table[0].1, table[1].1, table[2].1, table[3].1]),
        };
        acc = Some(match acc {
            None => sel,
            Some(a) if j + 2 >= windows => add(cs, &a, &sel),
            Some(a) => add_incomplete(cs, &a, &sel),
        });
        step = step.double().double();
    }
    add(cs, &acc.unwrap(), &pt_const(&(-offset)))
}

/// A full scalar of the other field arrives as two 128-bit limbs (bound by
/// the cross-circuit RLC); its bits are decomposed here when it multiplies a
/// point (booleans, two recompositions, counted comparison with the order).
struct Limbs<S> {
    lo: Lc<S>,
    hi: Lc<S>,
    repr: [u8; 32],
}

fn limbs_in<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, repr: [u8; 32]) -> Limbs<S> {
    let _scope = crate::blocks::scope("limbs_in");
    let lo = u128::from_le_bytes(repr[..16].try_into().unwrap());
    let hi = u128::from_le_bytes(repr[16..].try_into().unwrap());
    Limbs {
        lo: cs.alloc(S::from_u128(lo)),
        hi: cs.alloc(S::from_u128(hi)),
        repr,
    }
}

fn limb_bits<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, l: &Limbs<S>) -> Vec<Lc<S>> {
    let _scope = crate::blocks::scope("limb_bits");
    let bits: Vec<Lc<S>> = (0..255)
        .map(|i| cs.boolean((l.repr[i / 8] >> (i % 8)) & 1 == 1))
        .collect();
    cs.enforce_zero(&recompose(&bits[..128]).minus(&l.lo));
    cs.enforce_zero(&recompose(&bits[128..]).minus(&l.hi));
    cs.counted(255);
    bits
}

// ---------------------------------------------------------------------------
// Native replay of the verifier with every intermediate value.

/// How a challenge is used: a 126-bit integer, a Halo endoscalar, or the whole
/// squeezed word (lean tau).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Raw,
    Endo,
    Full,
}

pub struct Trace<C: PastaCurve> {
    /// Challenge bits in transcript order (255 word bits for Full), with
    /// their kind and whether each starts a fresh squeeze.
    pub challenges: Vec<(Vec<bool>, Kind, bool)>,
    pub header: ([C::Base; WIDTH], usize),
    pub io_digest: F<C>,
    pub tau: Vec<F<C>>,
    pub rx: Vec<Vec<F<C>>>,
    pub rabc: [F<C>; 3],
    pub ry: Vec<Vec<F<C>>>,
    pub alpha: F<C>,
    pub gamma: F<C>,
    pub p: C,
    pub xi: F<C>,
    pub p_claims: C,
    pub kappa: F<C>,
    pub claim_values: Vec<F<C>>,
    pub kappa_claims: F<C>,
    pub zeta: F<C>,
    pub folded: Vec<C>,
    pub us: Vec<F<C>>,
    pub c: F<C>,
    pub b_final: F<C>,
    pub coefficient: F<C>,
    pub r_col: Vec<F<C>>,
    /// Whether the native final opening holds (a tampered proof fails it).
    pub accepts: bool,
    /// Pairing link: (first proof?, squeezed word d or rho, absorbed word rho
    /// or d, R).
    pub link: Option<(bool, [u8; 32], [u8; 32], [u8; 32])>,
    /// A circuit proof's extras (`trace_circuit`).
    pub circuit: Option<CircuitTrace<C>>,
}

/// What a circuit proof's transcript adds: the first phase's rows, the pair
/// link's word, the link phase's rows, then the second-phase challenges, the
/// late io's digest, then the second phase's rows and the fixed rows (the
/// verification key's commitments, last); the link's R after them.
pub struct CircuitTrace<C: PastaCurve> {
    pub p1: usize,
    /// The link phase's rows (after the link's word).
    pub pl: usize,
    /// The link's io (slots rho fills, set from the transcript).
    pub link_io: Vec<(usize, F<C>)>,
    pub phase2: Vec<F<C>>,
    pub late_digest: Option<F<C>>,
    pub fixed: Vec<Aff<C>>,
    /// The io with the link's and the late values at their slots.
    pub io_full: Vec<F<C>>,
}

/// The pair link's word in a trace (see `spartan::link_word`): (first proof?,
/// the squeezed word, the absorbed word).
fn traced_link_word<C: PastaCurve>(
    t: &mut Transcript<C>,
    ch: &mut impl FnMut(&mut Transcript<C>, Kind) -> F<C>,
    link: &Link,
) -> Option<(bool, [u8; 32], [u8; 32])> {
    match link {
        Link::None => None,
        Link::First { rho, .. } => {
            let _ = ch(t, Kind::Full);
            let d = t.last_word().to_repr();
            t.word_limbs(b"pair/rho", rho);
            Some((true, d, *rho))
        }
        Link::Second { d, .. } => {
            t.word_limbs(b"pair/d", d);
            let _ = ch(t, Kind::Full);
            let rho = t.last_word().to_repr();
            Some((false, rho, *d))
        }
    }
}

/// The link's R in a trace: the first proof's given value, the second's
/// `rlc` of its squeezed rho (a verifier's `rlc` returns the R it was given).
fn traced_link_r(link: &Link, squeezed: &[u8; 32]) -> [u8; 32] {
    match link {
        Link::None => unreachable!("a link word without a link"),
        Link::First { r, .. } => *r,
        Link::Second { rlc, .. } => rlc(squeezed),
    }
}

pub fn trace<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    link: &Link,
) -> Trace<C> {
    trace_with(shape, gens, inst, proof, claims, link, None)
}

/// The trace of a circuit proof (uniform or sparse): its second phase, late io
/// and fixed rows (`fixed`, the key's commitments) as `verify_inner` reads them.
pub fn trace_circuit<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    link: &Link,
    circuit: &dyn r1cs::Circuit<F<C>>,
    fixed: &[Aff<C>],
) -> Trace<C> {
    trace_with(
        shape,
        gens,
        inst,
        proof,
        claims,
        link,
        Some((circuit, fixed)),
    )
}

#[allow(clippy::too_many_arguments)]
fn trace_with<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    link: &Link,
    circuit: Option<(&dyn r1cs::Circuit<F<C>>, &[Aff<C>])>,
) -> Trace<C> {
    let mut t = Transcript::<C>::new(shape.label.as_bytes());
    for value in [
        shape.sx,
        shape.row_bits,
        shape.col_bits,
        shape.k,
        inst.io.len(),
    ] {
        t.bytes(b"shape", &(value as u64).to_le_bytes());
    }
    let header = t.snapshot();
    let digest = io_digest(&inst.io);
    t.scalar(b"io", &digest);
    let mut log: Vec<(Vec<bool>, Kind, bool)> = Vec::new();
    let mut ch = |t: &mut Transcript<C>, kind: Kind| -> F<C> {
        let fresh = kind == Kind::Full || !t.has_spare();
        if kind == Kind::Full {
            let v = t.challenge_full(b"full");
            let repr = t.last_word().to_repr();
            log.push((
                (0..255)
                    .map(|i| (repr[i / 8] >> (i % 8)) & 1 == 1)
                    .collect(),
                kind,
                true,
            ));
            return v;
        }
        let bits = t.challenge_bits();
        let v = if kind == Kind::Endo {
            endo_scalar(&bits)
        } else {
            raw_scalar(&bits)
        };
        log.push((bits, kind, fresh));
        v
    };
    let lean = lean_schedule();
    let mut rows: Vec<Aff<C>> = proof.rows.clone();
    let mut io_full = inst.io.clone();
    let mut rel: Option<Box<dyn r1cs::Rel<F<C>> + '_>> = None;
    let mut ctrace = None;
    let word = match circuit {
        None => {
            for row in &rows {
                t.point(b"row", row);
            }
            traced_link_word(&mut t, &mut ch, link)
        }
        Some((c, fixed)) => {
            rows.extend(fixed.iter().copied());
            let p1 = c.phase1_rows(shape);
            for row in &rows[..p1] {
                t.point(b"row", row);
            }
            let word = traced_link_word(&mut t, &mut ch, link);
            // rho: absorbed by the first proof, squeezed by the second; R:
            // the link's.
            let link_io = word.map_or(Vec::new(), |(first, squeezed, absorbed)| {
                let rho = if first { absorbed } else { squeezed };
                c.link_io(&rho, &traced_link_r(link, &squeezed))
            });
            let pl = c.link_rows(shape);
            for row in &rows[p1..p1 + pl] {
                t.point(b"row", row);
            }
            let phase2: Vec<F<C>> = (0..c.phase2_challenges())
                .map(|_| ch(&mut t, Kind::Raw))
                .collect();
            rel = Some(c.instantiate(shape, &phase2, None));
            let late_digest = (!proof.late.is_empty()).then(|| io_digest(&proof.late));
            if let Some(d) = late_digest {
                t.scalar(b"late", &d);
            }
            io_full = spartan::io_with_late(
                &r1cs::io_with_link(&inst.io, &link_io),
                &c.late_slots(),
                &proof.late,
            );
            for row in &rows[p1 + pl..] {
                t.point(b"row", row);
            }
            ctrace = Some(CircuitTrace {
                p1,
                pl,
                link_io,
                phase2,
                late_digest,
                fixed: fixed.to_vec(),
                io_full: io_full.clone(),
            });
            word
        }
    };
    // The link's R, after every row.
    let link_words = word.map(|(first, squeezed, absorbed)| {
        let r = traced_link_r(link, &squeezed);
        t.word_limbs(b"pair/R", &r);
        (first, squeezed, absorbed, r)
    });
    let tau: Vec<F<C>> = if lean {
        powers2(ch(&mut t, Kind::Full), shape.sx)
    } else {
        (0..shape.sx).map(|_| ch(&mut t, Kind::Raw)).collect()
    };
    let mut rx = Vec::new();
    for (commitment, &k) in proof.outer.iter().zip(&shape.outer_blocks()) {
        t.point(b"outer", commitment);
        rx.push(if lean {
            kron3(ch(&mut t, Kind::Raw), k)
        } else {
            (0..k).map(|_| ch(&mut t, Kind::Raw)).collect()
        });
    }
    for value in &proof.v {
        t.scalar(b"v", value);
    }
    let rabc = if lean {
        let r = ch(&mut t, Kind::Raw);
        [r, r * r, r * r * r]
    } else {
        [
            ch(&mut t, Kind::Raw),
            ch(&mut t, Kind::Raw),
            ch(&mut t, Kind::Raw),
        ]
    };
    let mut ry = Vec::new();
    let mut g = 0;
    for (commitment, &k) in proof.inner.iter().zip(&shape.inner_blocks()) {
        t.point(b"inner", commitment);
        if lean && g > shape.row_bits {
            ry.push(kron3(ch(&mut t, Kind::Raw), k));
            g += k;
            continue;
        }
        let mut r = Vec::with_capacity(k);
        for _ in 0..k {
            r.push(ch(
                &mut t,
                if (1..=shape.row_bits).contains(&g) {
                    Kind::Endo
                } else {
                    Kind::Raw
                },
            ));
            g += 1;
        }
        ry.push(r);
    }
    let alpha = ch(&mut t, Kind::Raw);
    let gamma = ch(&mut t, Kind::Endo);
    let chs = spartan::Challenges {
        tau: tau.clone(),
        rx: rx.clone(),
        v: proof.v,
        rabc,
        ry: ry.clone(),
        alpha,
        gamma,
    };
    let (weights, kappa, eq_row, coefficient, r_col) = match &rel {
        Some(r) => spartan::relation(shape, &chs, &io_full, r.as_ref()),
        None => spartan::relation(shape, &chs, &inst.io, &r1cs::Toy),
    };
    let rounds: Vec<Aff<C>> = proof.outer.iter().chain(&proof.inner).copied().collect();
    let p = spartan::combined_commitment::<C>(&rows, &rounds, &eq_row, gamma);
    // Inherited claims, folded in clear.
    let (mut p_claims, mut kappa_claims, mut xi) = (p, kappa, F::<C>::ZERO);
    let mut claim_values = Vec::new();
    if !claims.is_empty() {
        for claim in claims {
            t.point(b"ipa/claim", &claim.g);
        }
        xi = ch(&mut t, Kind::Endo);
        let mut power = F::<C>::ONE;
        for claim in claims {
            power *= xi;
            let s = s_vector(&claim.us);
            let value: F<C> = s.iter().zip(&weights).map(|(a, b)| *a * b).sum();
            claim_values.push(value);
            kappa_claims += power * value;
            p_claims += C::from(claim.g) * power;
        }
    }
    t.point(b"ipa/commitment", &p_claims.to_affine());
    t.scalar(b"ipa/value", &kappa_claims);
    let zeta = ch(&mut t, Kind::Endo);
    let u_point = C::from(gens.u) * zeta;
    let mut folded = p_claims + u_point * kappa_claims;
    let mut trail = vec![folded];
    let mut us = Vec::new();
    for (l, r) in &proof.ipa.lr {
        t.point(b"ipa/L", l);
        t.point(b"ipa/R", r);
        let u = ch(&mut t, Kind::Endo);
        folded = folded * u + C::from(*l) * u.square() + C::from(*r);
        trail.push(folded);
        us.push(u);
    }
    let b_final = spartan::b_final_slots(shape, &chs, coefficient, &r_col, &us);
    t.point(b"ipa/G", &proof.ipa.g_final);
    t.point(b"ipa/A", &proof.ipa.a);
    let c = ch(&mut t, Kind::Endo);
    let lhs = (C::from(proof.ipa.g_final) + u_point * b_final) * proof.ipa.z1
        + C::from(gens.h) * proof.ipa.z2;
    let accepts = lhs == C::from(proof.ipa.a) + folded * c;
    Trace {
        challenges: log,
        header,
        io_digest: digest,
        tau,
        rx,
        rabc,
        ry,
        alpha,
        gamma,
        p,
        xi,
        p_claims,
        kappa,
        claim_values,
        kappa_claims,
        zeta,
        folded: trail,
        us,
        c,
        b_final,
        coefficient,
        r_col,
        accepts,
        link: link_words,
        circuit: ctrace,
    }
}

// ---------------------------------------------------------------------------
// Points side.

#[derive(Clone, Debug, Default)]
pub struct SideCount {
    pub counts: BTreeMap<&'static str, usize>,
    pub total: usize,
    pub bits: usize,
    pub vars: usize,
    pub permutations: usize,
    pub squeezes: usize,
    pub lookup_squeezes: usize,
    pub endoscales: usize,
    pub adds: usize,
    /// Cells a single perturbation leaves satisfied, by section (mutation run).
    pub free_cells: Option<BTreeMap<&'static str, usize>>,
}

fn finish<S: PrimeField<Repr = [u8; 32]> + Send + 'static>(cs: &mut Cs<S>) -> SideCount {
    if EXPORT.load(std::sync::atomic::Ordering::Relaxed) {
        SINK.lock().unwrap().push(Box::new(Exported {
            vals: cs.vals.clone(),
            widths: cs.var_width.clone(),
            rows: cs
                .rows
                .iter()
                .map(|(a, b, c)| (a.0.clone(), b.0.clone(), c.0.clone()))
                .collect(),
            counted: cs.counted_rows,
            row_scope: cs.row_scope.clone(),
        }));
    }
    let free_cells = if MUTATE.load(std::sync::atomic::Ordering::Relaxed) {
        Some(cs.mutation_test())
    } else {
        None
    };
    SideCount {
        counts: cs.counts.clone(),
        total: cs.total(),
        bits: cs.bits,
        vars: cs.vars,
        free_cells,
        ..Default::default()
    }
}

/// The shared full scalars the points side needs from the scalar side.
pub struct SharedScalars<C: PastaCurve> {
    pub io_digest: F<C>,
    pub kappa_zeta: F<C>,
    pub c_z1: F<C>,
    pub z1_bf_zeta: F<C>,
}

pub fn shared_scalars<C: PastaCurve>(tr: &Trace<C>, proof: &Proof<C>) -> SharedScalars<C> {
    // z1 = 2 c + 2^255 + 1 (mod order).
    let n = 255;
    let c_z1 = (proof.ipa.z1 - pow2::<F<C>>(n) - F::<C>::ONE) * F::<C>::from(2).invert().unwrap();
    SharedScalars {
        io_digest: tr.io_digest,
        kappa_zeta: tr.kappa_claims * tr.zeta,
        c_z1,
        z1_bf_zeta: proof.ipa.z1 * tr.b_final * tr.zeta,
    }
}

pub fn points_side<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    tr: &Trace<C>,
) -> SideCount {
    let _scope = crate::blocks::scope("points_side");
    let mut cs = Cs::<C::Base>::new();
    let b = beta::<C>();
    let sh = shared_scalars(tr, proof);
    let mut sp = Sponge::<C::Base>::new(tr.header);
    let mut next = 0usize;
    let mut endoscales = 0usize;
    let mut adds = 0usize;
    // Absorption words of a shared scalar.
    let words = |l: &Limbs<C::Base>| -> Vec<Lc<C::Base>> {
        if Transcript::<C>::scalars_embed() {
            vec![l.lo.plus(&l.hi.scale(pow2(128)))]
        } else {
            vec![l.lo.clone(), l.hi.clone()]
        }
    };
    // A fresh squeeze is decomposed into bits unless (lean schedule) every
    // challenge drawn from it is raw.
    let lean = lean_schedule();
    macro_rules! challenge {
        () => {{
            cs.section("transcript");
            let (want, kind, fresh) = tr.challenges[next].clone();
            let ch = if kind == Kind::Full {
                sp.challenge_full(&mut cs)
            } else {
                let spare_used = tr
                    .challenges
                    .get(next + 1)
                    .is_some_and(|(_, k, f)| !f && *k != Kind::Full);
                let endo_in =
                    kind == Kind::Endo || (spare_used && tr.challenges[next + 1].1 == Kind::Endo);
                sp.challenge(&mut cs, !lean || endo_in || !fresh)
            };
            match &ch {
                Ch::Bits(bits) => {
                    let values: Vec<bool> =
                        bits.iter().map(|b| cs.val(b) == C::Base::ONE).collect();
                    assert_eq!(
                        values, want,
                        "in-circuit challenge {next} matches the native transcript"
                    );
                }
                Ch::Value(v) => {
                    assert!(
                        cs.val(v) == raw_scalar::<C::Base>(&want),
                        "in-circuit challenge {next} matches"
                    );
                }
                Ch::Full(lo, hi) => {
                    let w = cs.val(&lo.plus(&hi.scale(pow2(128))));
                    let bits: Vec<bool> = (0..255)
                        .map(|i| (w.to_repr()[i / 8] >> (i % 8)) & 1 == 1)
                        .collect();
                    assert_eq!(bits, want, "in-circuit full challenge matches");
                }
            }
            next += 1;
            ch
        }};
    }
    let bits_of = |c: Ch<C::Base>| -> Vec<Lc<C::Base>> {
        match c {
            Ch::Bits(b) => b,
            _ => panic!("an endoscalar challenge has bits"),
        }
    };
    macro_rules! absorb_point {
        ($p:expr) => {{
            cs.section("on-curve");
            let pt = pt_input::<C>(&mut cs, $p);
            cs.section("transcript");
            sp.absorb(&mut cs, &pt.x);
            sp.absorb(&mut cs, &pt.y);
            pt
        }};
    }
    cs.section("transcript");
    let io = limbs_in(&mut cs, sh.io_digest.to_repr());
    for w in words(&io) {
        sp.absorb(&mut cs, &w);
    }
    let rows: Vec<Pt<C::Base>> = proof.rows.iter().map(|r| absorb_point!(r)).collect();
    if let Some((first, _, other, r)) = tr.link {
        // First: squeeze d, absorb rho and R; second: absorb d, squeeze rho,
        // absorb R. The absorbed words arrive as 128-bit limbs (shared).
        if first {
            let _ = challenge!();
        }
        cs.section("transcript");
        let o = limbs_in(&mut cs, other);
        sp.absorb(&mut cs, &o.lo);
        sp.absorb(&mut cs, &o.hi);
        if !first {
            let _ = challenge!();
        }
        cs.section("transcript");
        let rl = limbs_in(&mut cs, r);
        sp.absorb(&mut cs, &rl.lo);
        sp.absorb(&mut cs, &rl.hi);
    }
    let n_tau = if lean { 1 } else { shape.sx };
    for _ in 0..n_tau {
        let _ = challenge!();
    }
    let mut rounds: Vec<Pt<C::Base>> = Vec::new();
    for (commitment, &k) in proof.outer.iter().zip(&shape.outer_blocks()) {
        rounds.push(absorb_point!(commitment));
        let n_round = if lean { 1 } else { k };
        for _ in 0..n_round {
            let _ = challenge!();
        }
    }
    cs.section("transcript");
    for value in &proof.v {
        let l = limbs_in(&mut cs, value.to_repr());
        for w in words(&l) {
            sp.absorb(&mut cs, &w);
        }
    }
    let n_rabc = if lean { 1 } else { 3 };
    for _ in 0..n_rabc {
        let _ = challenge!();
    }
    let mut row_bits: Vec<Vec<Lc<C::Base>>> = Vec::new();
    let mut g = 0;
    for (commitment, &k) in proof.inner.iter().zip(&shape.inner_blocks()) {
        rounds.push(absorb_point!(commitment));
        if lean && g > shape.row_bits {
            let _ = challenge!();
            g += k;
            continue;
        }
        for _ in 0..k {
            let ch = challenge!();
            if (1..=shape.row_bits).contains(&g) {
                row_bits.push(bits_of(ch));
            }
            g += 1;
        }
    }
    let _alpha = challenge!();
    let gamma = bits_of(challenge!());
    // Hyrax row fold: T' = T_lo + rho (T_hi - T_lo), rho_0 the top row bit.
    cs.section("group");
    let mut level = rows;
    for bits in &row_bits {
        let half = level.len() / 2;
        let mut next_level = Vec::with_capacity(half);
        for i in 0..half {
            let diff = add(&mut cs, &level[half + i], &neg(&level[i]));
            let scaled = endoscale::<C>(&mut cs, &diff, bits, b);
            next_level.push(add(&mut cs, &level[i], &scaled));
            endoscales += 1;
            adds += 2;
        }
        level = next_level;
    }
    assert_eq!(level.len(), 1);
    // sum_i gamma^(i+1) D_i by Horner.
    let mut acc = rounds.last().unwrap().clone();
    for d in rounds.iter().rev().skip(1) {
        let scaled = endoscale::<C>(&mut cs, &acc, &gamma, b);
        acc = add(&mut cs, d, &scaled);
        endoscales += 1;
        adds += 1;
    }
    let round_sum = endoscale::<C>(&mut cs, &acc, &gamma, b);
    let p = add(&mut cs, &level[0], &round_sum);
    endoscales += 1;
    adds += 1;
    pt_check(&cs, &p, &tr.p, "combined commitment P");
    // Inherited claims: P' = P + sum_j xi^(j+1) G_j.
    let mut p2 = p;
    if !claims.is_empty() {
        let pts: Vec<Pt<C::Base>> = claims.iter().map(|c| absorb_point!(&c.g)).collect();
        let xi = bits_of(challenge!());
        cs.section("group");
        let mut acc = pts.last().unwrap().clone();
        for gpt in pts.iter().rev().skip(1) {
            let scaled = endoscale::<C>(&mut cs, &acc, &xi, b);
            acc = add(&mut cs, gpt, &scaled);
            endoscales += 1;
            adds += 1;
        }
        let scaled = endoscale::<C>(&mut cs, &acc, &xi, b);
        p2 = add(&mut cs, &p2, &scaled);
        endoscales += 1;
        adds += 1;
    }
    pt_check(&cs, &p2, &tr.p_claims, "P with inherited claims");
    cs.section("transcript");
    sp.absorb(&mut cs, &p2.x);
    sp.absorb(&mut cs, &p2.y);
    let kappa_limbs = limbs_in(&mut cs, tr.kappa_claims.to_repr());
    for w in words(&kappa_limbs) {
        sp.absorb(&mut cs, &w);
    }
    let _zeta = challenge!();
    // folded = P' + (kappa' zeta) U.
    cs.section("scalar bits");
    let kz = limbs_in(&mut cs, sh.kappa_zeta.to_repr());
    let kz_bits = limb_bits(&mut cs, &kz);
    cs.section("group");
    let ku = fixed_mul::<C>(&mut cs, &C::from(gens.u), &kz_bits);
    let mut folded = add(&mut cs, &p2, &ku);
    adds += 1;
    pt_check(&cs, &folded, &tr.folded[0], "IPA start");
    for (j, (l, r)) in proof.ipa.lr.iter().enumerate() {
        let lp = absorb_point!(l);
        let rp = absorb_point!(r);
        let u = bits_of(challenge!());
        cs.section("group");
        let ul = endoscale::<C>(&mut cs, &lp, &u, b);
        let inner = add(&mut cs, &folded, &ul);
        let scaled = endoscale::<C>(&mut cs, &inner, &u, b);
        folded = add(&mut cs, &scaled, &rp);
        endoscales += 2;
        adds += 2;
        pt_check(&cs, &folded, &tr.folded[j + 1], "IPA round");
    }
    let gf = absorb_point!(&proof.ipa.g_final);
    let a = absorb_point!(&proof.ipa.a);
    let c = bits_of(challenge!());
    // z1 G_f + (z1 b_f zeta) U + z2 H = A + c folded.
    cs.section("scalar bits");
    let cz = limbs_in(&mut cs, sh.c_z1.to_repr());
    let cz_bits = limb_bits(&mut cs, &cz);
    let zb = limbs_in(&mut cs, sh.z1_bf_zeta.to_repr());
    let zb_bits = limb_bits(&mut cs, &zb);
    let z2 = limbs_in(&mut cs, proof.ipa.z2.to_repr());
    let z2_bits = limb_bits(&mut cs, &z2);
    cs.section("group");
    let t1 = var_mul(&mut cs, &gf, &cz_bits);
    let t2 = fixed_mul::<C>(&mut cs, &C::from(gens.u), &zb_bits);
    let t3 = fixed_mul::<C>(&mut cs, &C::from(gens.h), &z2_bits);
    let t12 = add(&mut cs, &t1, &t2);
    let lhs = add(&mut cs, &t12, &t3);
    let cf = endoscale::<C>(&mut cs, &folded, &c, b);
    let rhs = add(&mut cs, &a, &cf);
    endoscales += 1;
    adds += 3;
    cs.enforce_zero(&lhs.x.minus(&rhs.x));
    cs.enforce_zero(&lhs.y.minus(&rhs.y));
    assert_eq!(next, tr.challenges.len(), "every challenge replayed");
    let mut out = finish(&mut cs);
    out.permutations = sp.permutations;
    out.squeezes = sp.squeezes;
    out.lookup_squeezes = sp.lookup_squeezes;
    out.endoscales = endoscales;
    out.adds = adds;
    out
}

// ---------------------------------------------------------------------------
// Scalar side (native arithmetic over F<C>).

/// The endoscalar a zeta + b of 126 challenge bits: booleans, the shared
/// 126-bit value, one product per pair.
fn endo_value<S: PrimeField<Repr = [u8; 32]> + WithSmallOrderMulGroup<3>>(
    cs: &mut Cs<S>,
    bits: &[bool],
) -> Lc<S> {
    let _scope = crate::blocks::scope("endo_value");
    let b: Vec<Lc<S>> = bits.iter().map(|x| cs.boolean(*x)).collect();
    let shared = cs.alloc(raw_scalar(bits));
    cs.enforce_zero(&recompose(&b).minus(&shared));
    let mut a = Lc::constant(S::from_u128(1 << 64));
    let mut bb = a.clone();
    for i in 0..bits.len() / 2 {
        let (s, e) = (&b[2 * i], &b[2 * i + 1]);
        let es = cs.mul(e, s);
        let w = S::from_u128(1 << i);
        a = a.plus(&es.scale(w.double())).minus(&e.scale(w));
        bb = bb
            .plus(&s.scale(w.double()))
            .minus(&Lc::constant(w))
            .minus(&es.scale(w.double()))
            .plus(&e.scale(w));
    }
    let v = a.scale(S::ZETA).plus(&bb);
    assert!(
        cs.val(&v) == endo_scalar::<S>(bits),
        "endoscalar value matches"
    );
    v
}

fn eq_table_c<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, r: &[Lc<S>]) -> Vec<Lc<S>> {
    let _scope = crate::blocks::scope("eq_table");
    let mut table = vec![Lc::constant(S::ONE)];
    for ri in r {
        let mut next = Vec::with_capacity(table.len() * 2);
        for value in &table {
            let high = cs.mul(value, ri);
            next.push(value.minus(&high));
            next.push(high);
        }
        table = next;
    }
    table
}

/// Lagrange basis at nodes 0, 1, 2 from t and t^2 (linear, no rows).
fn lagrange_sq<S: PrimeField>(t: &Lc<S>, tt: &Lc<S>) -> [Lc<S>; 3] {
    let half = S::from(2).invert().unwrap();
    [
        tt.minus(&t.scale(S::from(3)))
            .plus(&Lc::constant(S::from(2)))
            .scale(half),
        t.scale(S::from(2)).minus(tt),
        tt.minus(t).scale(half),
    ]
}

fn product_c<S: PrimeField<Repr = [u8; 32]>>(cs: &mut Cs<S>, xs: &[Lc<S>]) -> Lc<S> {
    let _scope = crate::blocks::scope("product");
    let mut acc = Lc::constant(S::ONE);
    for x in xs {
        acc = cs.mul(&acc, x);
    }
    acc
}

fn bit_c<S: PrimeField>(r: &Lc<S>, b: usize) -> Lc<S> {
    if b == 1 {
        r.clone()
    } else {
        Lc::constant(S::ONE).minus(r)
    }
}

/// What the closed form reads per round slot (shared by every opening): the
/// weight factors, tau per digit (outer slots) and the Lagrange bases of the
/// round's challenges.
struct SlotC<S> {
    slot: spartan::Slot,
    fb: Lc<S>,
    fg: Lc<S>,
    tau: Vec<Lc<S>>,
    lag: Vec<[Lc<S>; 3]>,
}

/// `<s(u), b>` in closed form (spartan::b_final_slots): the head by one pass
/// over the column bits, then one product per slot digit factor.
fn closed_form_c<S: PrimeField<Repr = [u8; 32]>>(
    cs: &mut Cs<S>,
    us: &[Lc<S>],
    coefficient: &Lc<S>,
    r_col: &[Lc<S>],
    active: usize,
    slots: &[SlotC<S>],
) -> Lc<S> {
    let _scope = crate::blocks::scope("closed_form");
    let l = us.len();
    let kk = slots.iter().map(|sc| sc.slot.k).max().unwrap_or(0);
    let one = Lc::constant(S::ONE);
    let (mut f, mut d) = (one.clone(), one.clone());
    for j in (0..l).rev() {
        let ru = cs.mul(&r_col[j], &us[j]);
        let nr = one.minus(&r_col[j]);
        d = if (active >> (l - 1 - j)) & 1 == 1 {
            cs.mul(&ru, &d)
        } else {
            let a = cs.mul(&ru, &f);
            a.plus(&cs.mul(&nr, &d))
        };
        f = cs.mul(&f, &nr.plus(&ru));
    }
    let mut total = Lc(Vec::new()).minus(&cs.mul(coefficient, &f.minus(&d)));
    // s(u) over a slot's high bits, memoized by prefix; the inner Boolean
    // factor depends on the slot size only.
    let mut memo: HashMap<Vec<usize>, Lc<S>> = HashMap::new();
    let mut inner_b: HashMap<usize, Lc<S>> = HashMap::new();
    for sc in slots {
        let k = sc.slot.k;
        let high = l - 2 * kk;
        let lead = kk - k;
        let set: Vec<usize> = (0..high)
            .filter(|j| (sc.slot.offset >> (l - 1 - j)) & 1 == 1)
            .collect();
        let mut s_high = one.clone();
        for n in 1..=set.len() {
            s_high = match memo.get(&set[..n]) {
                Some(v) => v.clone(),
                None => {
                    let v = cs.mul(&s_high, &us[set[n - 1]]);
                    memo.insert(set[..n].to_vec(), v.clone());
                    v
                }
            };
        }
        let bp = if sc.slot.outer {
            let mut bp = one.clone();
            for j in 0..k {
                let (ul, t) = (&us[high + 2 * (lead + j) + 1], &sc.tau[j]);
                let bf = one.minus(t).plus(&cs.mul(ul, t));
                bp = cs.mul(&bp, &bf);
            }
            bp
        } else {
            match inner_b.get(&k) {
                Some(v) => v.clone(),
                None => {
                    let mut bp = one.clone();
                    for j in 0..k {
                        bp = cs.mul(&bp, &one.plus(&us[high + 2 * (lead + j) + 1]));
                    }
                    inner_b.insert(k, bp.clone());
                    bp
                }
            }
        };
        let mut gp = one.clone();
        for j in 0..k {
            let (uh, ul) = (&us[high + 2 * (lead + j)], &us[high + 2 * (lead + j) + 1]);
            let [l0, l1, l2] = &sc.lag[j];
            let gf = l0.plus(&cs.mul(ul, l1)).plus(&cs.mul(uh, l2));
            gp = cs.mul(&gp, &gf);
        }
        let w = cs.mul(&sc.fb, &bp).plus(&cs.mul(&sc.fg, &gp));
        total = total.plus(&cs.mul(&s_high, &w));
    }
    total
}

/// Rate-2 sponge over the public inputs in-circuit.
fn io_digest_c<S: PoseidonField + PrimeField<Repr = [u8; 32]>>(
    cs: &mut Cs<S>,
    io: &[Lc<S>],
) -> (Lc<S>, usize) {
    let _scope = crate::blocks::scope("io_digest");
    let mut state = [
        Lc(Vec::new()),
        Lc(Vec::new()),
        Lc::constant(S::from(io.len() as u64)),
    ];
    let mut perms = 0;
    for pair in io.chunks(RATE) {
        for (slot, value) in state.iter_mut().zip(pair) {
            *slot = slot.plus(value);
        }
        permute_gadget(cs, &mut state);
        perms += 1;
    }
    (state[0].clone(), perms)
}

pub fn scalar_side<C: PastaCurve>(
    shape: &Shape,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    tr: &Trace<C>,
) -> SideCount {
    let _scope = crate::blocks::scope("scalar_side");
    let mut cs = Cs::<F<C>>::new();
    // Challenges in transcript order: endoscalars rebuilt from bits, raw
    // challenges as shared values.
    cs.section("challenge values");
    // The pairing link's squeezed word (d or rho) is not used here: it crosses
    // between the two points sides only (the mutation test flagged its
    // allocation as two dead cells).
    let skip = usize::from(tr.link.is_some());
    let mut vals: Vec<Lc<F<C>>> = Vec::with_capacity(tr.challenges.len());
    for (bits, kind, _) in &tr.challenges[skip..] {
        vals.push(match kind {
            Kind::Endo => endo_value(&mut cs, bits),
            Kind::Raw => cs.alloc(raw_scalar(bits)),
            Kind::Full => {
                // Two 128-bit limbs cross; the word is lo + 2^128 hi here.
                let w = bits.iter().enumerate().fold([0_u8; 32], |mut acc, (i, b)| {
                    acc[i / 8] |= (*b as u8) << (i % 8);
                    acc
                });
                let lo = cs.alloc(F::<C>::from_u128(u128::from_le_bytes(
                    w[..16].try_into().unwrap(),
                )));
                let hi = cs.alloc(F::<C>::from_u128(u128::from_le_bytes(
                    w[16..].try_into().unwrap(),
                )));
                let t = lo.plus(&hi.scale(pow2(128)));
                assert!(cs.val(&t) == full_scalar::<F<C>>(&w));
                t
            }
        });
    }
    let lean = lean_schedule();
    let mut it = vals.into_iter();
    let mut take =
        |n: usize| -> Vec<Lc<F<C>>> { (0..n).map(|_| it.next().expect("challenge")).collect() };
    // Powers for the lean schedule: squarings for tau, cubes per round.
    let squarings = |cs: &mut Cs<F<C>>, t: Lc<F<C>>, n: usize| -> Vec<Lc<F<C>>> {
        let mut out = vec![t];
        for _ in 1..n {
            let last = out.last().unwrap().clone();
            out.push(cs.mul(&last, &last));
        }
        out
    };
    // A round's powers r, r^3, r^9, ... and the squares the chain computes
    // (the Lagrange bases reuse them).
    type Sq<T> = Vec<Option<Lc<T>>>;
    let cubes = |cs: &mut Cs<F<C>>, r: Lc<F<C>>, k: usize| -> (Vec<Lc<F<C>>>, Sq<F<C>>) {
        let mut out = vec![r];
        let mut sqs = vec![None; k];
        for j in 1..k {
            let last = out.last().unwrap().clone();
            let sq = cs.mul(&last, &last);
            out.push(cs.mul(&sq, &last));
            sqs[j - 1] = Some(sq);
        }
        (out, sqs)
    };
    let plain = |v: Vec<Lc<F<C>>>| -> (Vec<Lc<F<C>>>, Sq<F<C>>) {
        let n = v.len();
        (v, vec![None; n])
    };
    cs.section("challenge powers");
    let tau = if lean {
        let t = take(1).remove(0);
        squarings(&mut cs, t, shape.sx)
    } else {
        take(shape.sx)
    };
    let (ob, ib) = (shape.outer_blocks(), shape.inner_blocks());
    let (rx, rx_sq): (Vec<Vec<Lc<F<C>>>>, Vec<Sq<F<C>>>) = ob
        .iter()
        .map(|&k| {
            if lean {
                let r = take(1).remove(0);
                cubes(&mut cs, r, k)
            } else {
                plain(take(k))
            }
        })
        .unzip();
    let rabc = if lean {
        let r = take(1).remove(0);
        let r2 = cs.mul(&r, &r);
        let r3 = cs.mul(&r2, &r);
        vec![r, r2, r3]
    } else {
        take(3)
    };
    let mut ry: Vec<Vec<Lc<F<C>>>> = Vec::new();
    let mut ry_sq: Vec<Sq<F<C>>> = Vec::new();
    let mut g = 0;
    for &k in &ib {
        let (r, sq) = if lean && g > shape.row_bits {
            let r = take(1).remove(0);
            cubes(&mut cs, r, k)
        } else {
            plain(take(k))
        };
        ry.push(r);
        ry_sq.push(sq);
        g += k;
    }
    for (a, b) in tau.iter().zip(&tr.tau) {
        assert!(cs.val(a) == *b, "tau matches");
    }
    for (a, b) in ry.concat().iter().zip(tr.ry.concat()) {
        assert!(cs.val(a) == b, "r_y matches");
    }
    cs.section("challenge values");
    let alpha = take(1).remove(0);
    let gamma = take(1).remove(0);
    let xi = if claims.is_empty() {
        None
    } else {
        Some(take(1).remove(0))
    };
    let zeta = take(1).remove(0);
    let us = take(shape.col_bits);
    let _c = take(1).remove(0);
    assert!(cs.val(&tau[0]) == tr.tau[0] && cs.val(&gamma) == tr.gamma && cs.val(&zeta) == tr.zeta);
    // Proof scalars and public inputs (native here).
    let v: Vec<Lc<F<C>>> = proof.v.iter().map(|x| cs.alloc(*x)).collect();
    let z1 = cs.alloc(proof.ipa.z1);
    let io: Vec<Lc<F<C>>> = inst.io.iter().map(|x| cs.alloc(*x)).collect();
    cs.section("io digest");
    let (digest, _) = io_digest_c(&mut cs, &io);
    assert!(cs.val(&digest) == tr.io_digest);
    // alpha powers (one row each).
    cs.section("relation");
    let (mo, mi) = (ob.len(), ib.len());
    let mut apow = vec![Lc::constant(F::<C>::ONE)];
    for _ in 0..mo + mi + 2 {
        let next = cs.mul(apow.last().unwrap(), &alpha);
        apow.push(next);
    }
    // Slot scales gamma^-(s+1).
    let ginv = cs.inv(&gamma);
    let mut gpow = vec![ginv.clone()];
    for _ in 1..mo + mi {
        let next = cs.mul(gpow.last().unwrap(), &ginv);
        gpow.push(next);
    }
    // Per slot: weight factors and the Lagrange bases of its challenges.
    let mut sc: Vec<SlotC<F<C>>> = Vec::new();
    for (s, slot) in spartan::slots(shape).into_iter().enumerate() {
        let (b, last) = if slot.outer {
            (s, s + 1 == mo)
        } else {
            (s + 1, s + 1 == mo + mi)
        };
        let fb = cs.mul(&apow[b], &gpow[s]);
        let g = cs.mul(&apow[b + 1], &gpow[s]);
        let fg = if last { g } else { g.scale(-F::<C>::ONE) };
        let (rs, sqs) = if slot.outer {
            (&rx[slot.round], &rx_sq[slot.round])
        } else {
            (&ry[slot.round], &ry_sq[slot.round])
        };
        let mut lag = Vec::with_capacity(slot.k);
        for (t, sq) in rs.iter().zip(sqs) {
            let tt = match sq {
                Some(x) => x.clone(),
                None => cs.mul(t, t),
            };
            lag.push(lagrange_sq(t, &tt));
        }
        let tau_s = if slot.outer {
            tau[slot.first..slot.first + slot.k].to_vec()
        } else {
            Vec::new()
        };
        sc.push(SlotC {
            slot,
            fb,
            fg,
            tau: tau_s,
            lag,
        });
    }
    for (c, (nb, ng)) in sc
        .iter()
        .zip(spartan::slot_weights(shape, tr.alpha, tr.gamma))
    {
        assert!(
            cs.val(&c.fb) == nb && cs.val(&c.fg) == ng,
            "slot weights match"
        );
    }
    // Matrix MLE (the spike's uniform toy; the real circuit is budgeted).
    cs.section("matrix MLE (toy)");
    let rxa: Vec<Lc<F<C>>> = rx.concat();
    let rya: Vec<Lc<F<C>>> = ry.concat();
    let sw = shape.sw();
    let (rx_hi, rx_w) = rxa.split_at(shape.sx - sw);
    let (rx_q, rx_rest) = rx_w.split_at(2);
    let r_sel = rya[0].clone();
    let ry_w = &rya[1..];
    let (ry_q, ry_rest) = ry_w.split_at(2);
    let one = Lc::constant(F::<C>::ONE);
    let z_hi = {
        let f: Vec<_> = rx_hi.iter().map(|r| one.minus(r)).collect();
        product_c(&mut cs, &f)
    };
    let e = {
        let mut f = Vec::new();
        for (a, b) in rx_rest.iter().zip(ry_rest) {
            let ab = cs.mul(a, b);
            f.push(ab.scale(F::<C>::from(2)).minus(a).minus(b).plus(&one));
        }
        product_c(&mut cs, &f)
    };
    let ws = one.minus(&r_sel);
    let io0 = {
        let f: Vec<_> = ry_w.iter().map(|r| one.minus(r)).collect();
        product_c(&mut cs, &f)
    };
    let eqq = |cs: &mut Cs<F<C>>, r: &[Lc<F<C>>], q: usize| -> Lc<F<C>> {
        cs.mul(&bit_c(&r[0], (q >> 1) & 1), &bit_c(&r[1], q & 1))
    };
    let exq: Vec<Lc<F<C>>> = (0..4).map(|q| eqq(&mut cs, rx_q, q)).collect();
    let eyq: Vec<Lc<F<C>>> = (0..4).map(|q| eqq(&mut cs, ry_q, q)).collect();
    let mut diag = Lc(Vec::new());
    for q in 1..4 {
        diag = diag.plus(&cs.mul(&exq[q], &eyq[q]));
    }
    let zwe = {
        let t = cs.mul(&z_hi, &ws);
        cs.mul(&t, &e)
    };
    let a00 = cs.mul(&exq[0], &eyq[0]);
    let ma = cs.mul(&zwe, &a00.plus(&diag));
    let b01 = cs.mul(&exq[0], &eyq[1]);
    let b33 = cs.mul(&exq[3], &eyq[3]);
    let bw = cs.mul(&zwe, &b01.plus(&b33));
    let rio = cs.mul(&r_sel, &io0);
    let bio = cs.mul(&rio, &exq[1].plus(&exq[2]));
    let bio = cs.mul(&z_hi, &bio);
    let mb = bw.plus(&bio);
    let c02 = cs.mul(&exq[0], &eyq[2]);
    let mc = cs.mul(&zwe, &c02.plus(&diag));
    let m_value = {
        let a = cs.mul(&rabc[0], &ma);
        let b = cs.mul(&rabc[1], &mb);
        let c = cs.mul(&rabc[2], &mc);
        a.plus(&b).plus(&c)
    };
    // io~(r) over the sw witness-index variables.
    cs.section("io eval");
    let io_eval = {
        let mut sum = Lc(Vec::new());
        let bits_needed = io.len().next_power_of_two().trailing_zeros() as usize;
        let common: Vec<_> = ry_w[..sw - bits_needed]
            .iter()
            .map(|r| one.minus(r))
            .collect();
        let pre = product_c(&mut cs, &common);
        let eq_low = eq_table_c(&mut cs, &ry_w[sw - bits_needed..sw]);
        for (i, x) in io.iter().enumerate() {
            let w = cs.mul(&pre, &eq_low[i]);
            sum = sum.plus(&cs.mul(&w, x));
        }
        sum
    };
    // kappa.
    cs.section("relation");
    let vv = cs.mul(&v[0], &v[1]);
    let mut kappa = cs.mul(&apow[mo], &vv.minus(&v[2]));
    let claim = {
        let a = cs.mul(&rabc[0], &v[0]);
        let b = cs.mul(&rabc[1], &v[1]);
        let c = cs.mul(&rabc[2], &v[2]);
        a.plus(&b).plus(&c)
    };
    kappa = kappa.plus(&cs.mul(&apow[mo + 1], &claim));
    let am = cs.mul(&apow[mo + 1 + mi], &m_value);
    let coefficient = cs.mul(&am, &ws);
    let ams = cs.mul(&am, &r_sel);
    kappa = kappa.plus(&cs.mul(&ams, &io_eval));
    assert!(cs.val(&kappa) == tr.kappa, "kappa matches");
    assert!(
        cs.val(&coefficient) == tr.coefficient,
        "coefficient matches"
    );
    // b_final and inherited claim values in closed form.
    cs.section("closed form");
    let r_col: Vec<Lc<F<C>>> = rya[1 + shape.row_bits..].to_vec();
    let active = shape.active_cols();
    let b_final = closed_form_c(&mut cs, &us, &coefficient, &r_col, active, &sc);
    assert!(cs.val(&b_final) == tr.b_final, "b_final matches");
    let mut kappa2 = kappa.clone();
    if let Some(xi) = &xi {
        let mut power = Lc::constant(F::<C>::ONE);
        for (j, claim) in claims.iter().enumerate() {
            power = cs.mul(&power, xi);
            let uo: Vec<Lc<F<C>>> = claim.us.iter().map(|u| cs.alloc(*u)).collect();
            let value = closed_form_c(&mut cs, &uo, &coefficient, &r_col, active, &sc);
            assert!(
                cs.val(&value) == tr.claim_values[j],
                "inherited claim value matches"
            );
            kappa2 = kappa2.plus(&cs.mul(&power, &value));
        }
    }
    assert!(
        cs.val(&kappa2) == tr.kappa_claims,
        "kappa with claims matches"
    );
    // Schnorr scalars for the points side and z1's signed-digit form.
    cs.section("schnorr scalars");
    let sh = shared_scalars(tr, proof);
    let kz = cs.mul(&kappa2, &zeta);
    assert!(cs.val(&kz) == sh.kappa_zeta);
    let bz = cs.mul(&b_final, &zeta);
    let zbz = cs.mul(&z1, &bz);
    assert!(cs.val(&zbz) == sh.z1_bf_zeta);
    let cz = cs.alloc(sh.c_z1);
    cs.enforce_zero(
        &cz.scale(F::<C>::from(2))
            .plus(&Lc::constant(pow2::<F<C>>(255) + F::<C>::ONE))
            .minus(&z1),
    );
    finish(&mut cs)
}

// ---------------------------------------------------------------------------
// Driver.

pub struct Synthesized {
    pub points: SideCount,
    pub scalar: SideCount,
    /// Values crossing between the two circuits, as 128-bit limbs.
    pub shared: usize,
    /// Raw challenges among them (range-checked where the binding is non-native).
    pub raw_challenges: usize,
    pub challenges: usize,
    /// The shared values themselves (for the binding's R chains).
    pub values: Vec<u128>,
}

/// Proves one spike proof at `shape` with `claims` inherited claims and `io`
/// public inputs, verifies it natively, and synthesizes both sides.
pub fn synthesize<C: PastaCurve>(
    shape: Shape,
    gens: &Gens<C>,
    inherited: usize,
    io_len: usize,
    seed: &[u8],
) -> Synthesized {
    let (inst, wit, claims) = instance::<C>(shape, gens, inherited, io_len, seed);
    let proof = spartan::prove_with_claims(
        &shape,
        gens,
        &inst,
        &wit,
        &mut crate::fs::SpikeRng::new(&[seed, b"/prove"].concat()),
        &claims,
    );
    synthesize_proof(&shape, gens, &inst, &proof, &claims, &Link::None)
}

/// A sampled instance with `io_len` public inputs and `inherited` claims
/// (earlier openings' challenges and folded generators).
pub fn instance<C: PastaCurve>(
    shape: Shape,
    gens: &Gens<C>,
    inherited: usize,
    io_len: usize,
    seed: &[u8],
) -> (Instance<F<C>>, r1cs::Witness<F<C>>, Vec<Claim<C>>) {
    use crate::fs::SpikeRng;
    let mut rng = SpikeRng::new(seed);
    let (mut inst, wit) = r1cs::sample::<F<C>>(&shape, &mut rng);
    while inst.io.len() < io_len {
        inst.io.push(rng.below(64));
    }
    // Inherited claims: earlier openings' challenges and folded generators.
    let claims: Vec<Claim<C>> = (0..inherited)
        .map(|_| {
            let us: Vec<F<C>> = (0..shape.col_bits)
                .map(|_| {
                    let bits: Vec<bool> = (0..CHALLENGE_BITS)
                        .map(|_| rng.below::<F<C>>(1) == F::<C>::ONE)
                        .collect();
                    endo_scalar(&bits)
                })
                .collect();
            let g = iroha_pasta::msm::msm_public::<C>(
                &s_vector(&us),
                &gens.g,
                iroha_pasta::msm::MemoryBudget::DEFAULT,
            )
            .expect("msm")
            .to_affine();
            Claim { us, g }
        })
        .collect();
    (inst, wit, claims)
}

/// Both sides of one proof's in-circuit verification (with its link, if paired).
pub fn synthesize_proof<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    link: &Link,
) -> Synthesized {
    let inherited = claims.len();
    let native = match link {
        Link::None => spartan::verify_with_claims(shape, gens, inst, &proof.to_bytes(), claims),
        _ => spartan::verify_linked(shape, gens, inst, &proof.to_bytes(), claims, link).map(|_| ()),
    };
    native.expect("native verifier accepts");
    let tr = trace(shape, gens, inst, proof, claims, link);
    assert!(tr.accepts, "native final opening");
    let points = points_side(shape, gens, proof, claims, &tr);
    let scalar = scalar_side(shape, inst, proof, claims, &tr);
    let raw = tr
        .challenges
        .iter()
        .filter(|(_, k, _)| *k != Kind::Endo)
        .count();
    let crossing: usize = tr
        .challenges
        .iter()
        .map(|(_, k, _)| if *k == Kind::Full { 2 } else { 1 })
        .sum();
    // Shared: every challenge (126 bits), nine full scalars as two limbs (io
    // digest, v_A, v_B, v_C, kappa', kappa' zeta, c(z1), z1 b_f zeta, z2), and
    // four limbs per inherited claim point.
    // R crosses once per pair (charged to the first proof).
    let r_limbs = if tr.link.is_some_and(|l| l.0) { 2 } else { 0 };
    let shared = crossing + 2 * 9 + 4 * inherited + r_limbs;
    let values = shared_values(&tr, proof, claims);
    Synthesized {
        points,
        scalar,
        shared,
        raw_challenges: raw,
        challenges: tr.challenges.len(),
        values,
    }
}

/// The 128-bit values one proof's verification shares between the circuits:
/// every challenge (two limbs for full-width words), the nine full scalars,
/// the inherited claim points, and R for the first proof of a pair.
pub fn shared_values<C: PastaCurve>(
    tr: &Trace<C>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
) -> Vec<u128> {
    let limbs = |repr: [u8; 32]| -> [u128; 2] {
        [
            u128::from_le_bytes(repr[..16].try_into().unwrap()),
            u128::from_le_bytes(repr[16..].try_into().unwrap()),
        ]
    };
    let mut out = Vec::new();
    for (bits, kind, _) in &tr.challenges {
        if *kind == Kind::Full {
            let w = bits.iter().enumerate().fold([0_u8; 32], |mut acc, (i, b)| {
                acc[i / 8] |= (*b as u8) << (i % 8);
                acc
            });
            out.extend(limbs(w));
        } else {
            out.push(
                bits.iter()
                    .enumerate()
                    .fold(0_u128, |acc, (i, b)| acc | ((*b as u128) << i)),
            );
        }
    }
    let sh = shared_scalars(tr, proof);
    for v in [
        sh.io_digest,
        proof.v[0],
        proof.v[1],
        proof.v[2],
        tr.kappa_claims,
        sh.kappa_zeta,
        sh.c_z1,
        sh.z1_bf_zeta,
        proof.ipa.z2,
    ] {
        out.extend(limbs(v.to_repr()));
    }
    for c in claims {
        let (x, y): (C::Base, C::Base) = Option::from(c.g.coordinates()).expect("finite claim");
        out.extend(limbs(x.to_repr()));
        out.extend(limbs(y.to_repr()));
    }
    // A circuit proof's late io digest (absorbed after its second-phase
    // challenges), before the link's R (R stays last).
    if let Some(d) = tr.circuit.as_ref().and_then(|c| c.late_digest) {
        out.extend(limbs(d.to_repr()));
    }
    if let Some((true, _, _, r)) = tr.link {
        out.extend(limbs(r));
    }
    out
}

// ---------------------------------------------------------------------------
// The cross-circuit binding (gate 1f).
//
// A P-proof (Vesta, circuit C_P) and an S-proof (Pallas, circuit C_S) of one
// operation share values: every challenge and scalar their two halves of the
// predecessor's verification exchange. Each circuit computes the masked
// random linear combination R = mu rho^n + sum_j v_j rho^(n-1-j) over Fq of
// its copies (natively in C_S, non-natively in C_P; mu is a fresh mask both
// commit). rho follows both proofs' phase-1 commitments (spartan::Link): P
// squeezes its phase-1 digest d, S absorbs d and its rows and squeezes rho, P
// absorbs rho; both then absorb R. Whoever verifies the pair (the next
// operation's circuits, or the payee natively) replays both transcripts with
// one R. Differing copies give differing R except with probability n / q;
// a rho known before the copies are committed lets a forger fix two copies
// with a short lattice vector (shown in the tests).

pub type Fq = <iroha_pasta::Eq as PastaCurve>::Base;

pub struct PairSpec<'a> {
    pub shape_p: Shape,
    pub gens_p: &'a Gens<iroha_pasta::Eq>,
    pub inst_p: Instance<F<iroha_pasta::Eq>>,
    pub wit_p: r1cs::Witness<F<iroha_pasta::Eq>>,
    pub claims_p: Vec<Claim<iroha_pasta::Eq>>,
    pub shape_s: Shape,
    pub gens_s: &'a Gens<iroha_pasta::Ep>,
    pub inst_s: Instance<F<iroha_pasta::Ep>>,
    pub wit_s: r1cs::Witness<F<iroha_pasta::Ep>>,
    pub claims_s: Vec<Claim<iroha_pasta::Ep>>,
}

pub struct Pair {
    pub p: Proof<iroha_pasta::Eq>,
    pub s: Proof<iroha_pasta::Ep>,
    pub d: [u8; 32],
    pub rho: [u8; 32],
    pub r: [u8; 32],
}

/// R over Fq: mu rho^n + sum_j v_j rho^(n-1-j).
pub fn rlc_value(rho: &[u8; 32], mu: Fq, values: &[u128]) -> Fq {
    let rho = rho_fq(rho);
    values
        .iter()
        .fold(mu, |acc, v| acc * rho + Fq::from_u128(*v))
}

/// rho as an Fq element (S's squeezed Fp word, below q).
pub fn rho_fq(rho: &[u8; 32]) -> Fq {
    Option::from(Fq::from_repr(*rho)).expect("Fp word embeds in Fq")
}

/// Proves a linked pair; `rlc` gives R from rho (honest provers: the RLC of
/// the shared values both circuits hold).
pub fn prove_pair(spec: &PairSpec, seed: &[u8], rlc: &dyn Fn(&[u8; 32]) -> [u8; 32]) -> Pair {
    use crate::fs::SpikeRng;
    let rng_p = SpikeRng::new(&[seed, b"/P"].concat());
    let mut rng_s = SpikeRng::new(&[seed, b"/S"].concat());
    // P's phase-1 digest from a pass with the same randomness (same rows).
    let (_, d, _) = spartan::prove_linked(
        &spec.shape_p,
        spec.gens_p,
        &spec.inst_p,
        &spec.wit_p,
        &mut rng_p.clone(),
        &spec.claims_p,
        &Link::First {
            rho: [0; 32],
            r: [0; 32],
        },
    );
    let (s, rho, r) = spartan::prove_linked(
        &spec.shape_s,
        spec.gens_s,
        &spec.inst_s,
        &spec.wit_s,
        &mut rng_s,
        &spec.claims_s,
        &Link::Second { d, rlc },
    );
    let (p, d2, _) = spartan::prove_linked(
        &spec.shape_p,
        spec.gens_p,
        &spec.inst_p,
        &spec.wit_p,
        &mut rng_p.clone(),
        &spec.claims_p,
        &Link::First { rho, r },
    );
    assert_eq!(d, d2, "same rows, same digest");
    Pair { p, s, d, rho, r }
}

/// Verifies a pair natively: d from P's rows, S replayed with d (giving rho),
/// P replayed with rho, one R for both.
pub fn verify_pair(
    spec: &PairSpec,
    p: &Proof<iroha_pasta::Eq>,
    s: &Proof<iroha_pasta::Ep>,
    r: [u8; 32],
) -> Result<(), String> {
    let d = spartan::phase1_digest(&spec.shape_p, &spec.inst_p, p);
    let fixed = move |_: &[u8; 32]| r;
    let rho = spartan::verify_linked(
        &spec.shape_s,
        spec.gens_s,
        &spec.inst_s,
        &s.to_bytes(),
        &spec.claims_s,
        &Link::Second { d, rlc: &fixed },
    )?;
    let d2 = spartan::verify_linked(
        &spec.shape_p,
        spec.gens_p,
        &spec.inst_p,
        &p.to_bytes(),
        &spec.claims_p,
        &Link::First { rho, r },
    )?;
    if d2 != d {
        return Err("pair: phase-1 digest".into());
    }
    Ok(())
}

/// The binding's R chain in C_S (Fq native): one row per shared value.
pub fn rlc_chain_native(rho: &[u8; 32], mu: Fq, values: &[u128]) -> (SideCount, Fq) {
    let _scope = crate::blocks::scope("chain_s");
    let mut cs = Cs::<Fq>::new();
    cs.section("binding R chain");
    let rv = cs.alloc(rho_fq(rho));
    let mut acc = cs.alloc(mu);
    for v in values {
        let vv = cs.alloc(Fq::from_u128(*v)); // an existing cell in the real circuit
        let next = cs.alloc(cs.val(&acc) * rho_fq(rho) + Fq::from_u128(*v));
        cs.enforce(&acc, &rv, &next.minus(&vv));
        acc = next;
    }
    let r = cs.val(&acc);
    (finish(&mut cs), r)
}

/// A short (delta, eps) with delta rho + eps = 0 (mod q): 2D Gauss reduction.
pub fn short_relation(rho: &[u8; 32]) -> (num_bigint::BigInt, num_bigint::BigInt) {
    use num_bigint::{BigInt, BigUint};
    use num_integer::Integer;
    let q = BigInt::from(
        BigUint::parse_bytes(
            b"40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001",
            16,
        )
        .unwrap(),
    );
    let r = BigInt::from(BigUint::from_bytes_le(rho));
    let norm = |v: &(BigInt, BigInt)| &v.0 * &v.0 + &v.1 * &v.1;
    let (mut a, mut b) = (
        (BigInt::from(1), (-&r).mod_floor(&q)),
        (BigInt::from(0), q.clone()),
    );
    if norm(&a) > norm(&b) {
        std::mem::swap(&mut a, &mut b);
    }
    loop {
        let dot = &a.0 * &b.0 + &a.1 * &b.1;
        let na = norm(&a);
        // m = round(dot / na)
        let num: BigInt = &dot * BigInt::from(2) + &na;
        let den: BigInt = &na * BigInt::from(2);
        let m: BigInt = num.div_floor(&den);
        b = (&b.0 - &m * &a.0, &b.1 - &m * &a.1);
        if norm(&b) >= norm(&a) {
            return a;
        }
        std::mem::swap(&mut a, &mut b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};
    use num_traits::ToPrimitive;

    /// The endoscalar mapping equals Halo endoscaling of a point.
    #[test]
    fn endoscalar_matches_endoscaling() {
        let mut rng = crate::fs::SpikeRng::new(b"endo");
        for _ in 0..4 {
            let bits: Vec<bool> = (0..CHALLENGE_BITS)
                .map(|_| rng.below::<F<Vesta>>(1) == F::<Vesta>::ONE)
                .collect();
            let p = Vesta::generator() * rng.scalar::<F<Vesta>>();
            let mut native = (p.endo() + p).double();
            for i in (0..bits.len() / 2).rev() {
                let mut s = if bits[2 * i] { p } else { -p };
                if bits[2 * i + 1] {
                    s = s.endo();
                }
                native = native.double() + s;
            }
            assert_eq!(native, p * endo_scalar::<F<Vesta>>(&bits));
        }
    }

    /// Runs both sides on a (possibly tampered) proof; true when some circuit
    /// constraint is unsatisfied.
    fn unsatisfied<C: PastaCurve>(
        shape: &Shape,
        gens: &Gens<C>,
        inst: &Instance<F<C>>,
        proof: &Proof<C>,
        claims: &[Claim<C>],
    ) -> bool {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let tr = trace(shape, gens, inst, proof, claims, &Link::None);
            let _ = points_side(shape, gens, proof, claims, &tr);
            let _ = scalar_side(shape, inst, proof, claims, &tr);
        }));
        match result {
            Ok(()) => false,
            Err(e) => {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                assert!(
                    msg.contains("unsatisfied"),
                    "rejected by a circuit constraint, not by a harness error: {msg}"
                );
                true
            }
        }
    }

    /// Refusal cases on real proofs: altered proof points and scalars, wrong
    /// public inputs, omitted / duplicated / reordered / fabricated inherited
    /// claims, and the CancelRound and AdaptiveWitness forgeries each leave a
    /// circuit constraint unsatisfied.
    #[test]
    fn synthesis_refuses_tampering() {
        use crate::fs::SpikeRng;
        let shape = Shape {
            label: "verifier-refusal",
            sx: 13,
            row_bits: 2,
            col_bits: 11,
            k: 3,
        };
        let gens = Gens::<Vesta>::new(1 << shape.col_bits);
        let mut rng = SpikeRng::new(b"refusal");
        let (inst, wit) = r1cs::sample::<F<Vesta>>(&shape, &mut rng);
        let claim = |rng: &mut SpikeRng| {
            let us: Vec<F<Vesta>> = (0..shape.col_bits)
                .map(|_| {
                    endo_scalar(
                        &(0..CHALLENGE_BITS)
                            .map(|_| rng.below::<F<Vesta>>(1) == F::<Vesta>::ONE)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            let g = iroha_pasta::msm::msm_public::<Vesta>(
                &s_vector(&us),
                &gens.g,
                iroha_pasta::msm::MemoryBudget::DEFAULT,
            )
            .unwrap()
            .to_affine();
            Claim { us, g }
        };
        let claims = vec![claim(&mut rng), claim(&mut rng)];
        let proof = spartan::prove_with_claims(&shape, &gens, &inst, &wit, &mut rng, &claims);
        assert!(
            !unsatisfied(&shape, &gens, &inst, &proof, &claims),
            "honest proof satisfies"
        );
        let reparse = |p: &Proof<Vesta>| Proof::<Vesta>::from_bytes(&shape, &p.to_bytes()).unwrap();
        let g = Vesta::generator();
        let bump = |a: &Aff<Vesta>| (Vesta::from(*a) + g).to_affine();
        let mut cases: Vec<(&str, Proof<Vesta>)> = Vec::new();
        let mut p = reparse(&proof);
        p.rows[1] = bump(&p.rows[1]);
        cases.push(("row commitment", p));
        let mut p = reparse(&proof);
        p.outer[0] = bump(&p.outer[0]);
        cases.push(("outer round", p));
        let mut p = reparse(&proof);
        let last = p.inner.len() - 1;
        p.inner[last] = bump(&p.inner[last]);
        cases.push(("inner round", p));
        let mut p = reparse(&proof);
        p.v[0] += F::<Vesta>::ONE;
        cases.push(("v_A", p));
        let mut p = reparse(&proof);
        p.ipa.lr[3].0 = bump(&p.ipa.lr[3].0);
        cases.push(("IPA L", p));
        let mut p = reparse(&proof);
        p.ipa.lr[7].1 = bump(&p.ipa.lr[7].1);
        cases.push(("IPA R", p));
        let mut p = reparse(&proof);
        p.ipa.g_final = bump(&p.ipa.g_final);
        cases.push(("folded generator", p));
        let mut p = reparse(&proof);
        p.ipa.a = bump(&p.ipa.a);
        cases.push(("Schnorr A", p));
        let mut p = reparse(&proof);
        p.ipa.z1 += F::<Vesta>::ONE;
        cases.push(("z1", p));
        let mut p = reparse(&proof);
        p.ipa.z2 += F::<Vesta>::ONE;
        cases.push(("z2", p));
        for (what, p) in &cases {
            assert!(
                unsatisfied(&shape, &gens, &inst, p, &claims),
                "{what} tampering refused"
            );
        }
        let mut io = inst.io.clone();
        io[1] += F::<Vesta>::ONE;
        assert!(
            unsatisfied(&shape, &gens, &Instance { io }, &proof, &claims),
            "wrong io refused"
        );
        assert!(
            unsatisfied(&shape, &gens, &inst, &proof, &claims[..1]),
            "omitted claim refused"
        );
        let dup = vec![claims[0].clone(), claims[0].clone()];
        assert!(
            unsatisfied(&shape, &gens, &inst, &proof, &dup),
            "duplicated claim refused"
        );
        let swapped = vec![claims[1].clone(), claims[0].clone()];
        assert!(
            unsatisfied(&shape, &gens, &inst, &proof, &swapped),
            "reordered claims refused"
        );
        // A fabricated claim (G_old not <s(u_old), G>), proved honestly otherwise.
        let mut fake = claims.clone();
        fake[1].g = bump(&fake[1].g);
        let forged = spartan::prove_with_claims(&shape, &gens, &inst, &wit, &mut rng, &fake);
        assert!(
            unsatisfied(&shape, &gens, &inst, &forged, &fake),
            "fabricated claim refused"
        );
        // The tamper suite's forgeries on an unsatisfied witness.
        let mut bad = wit.w.clone();
        bad[(3 * shape.rows() / 4) * shape.cols()] = F::<Vesta>::from(2);
        let bad_wit = r1cs::Witness { w: bad };
        for cheat in [spartan::Cheat::CancelRound, spartan::Cheat::AdaptiveWitness] {
            let (forged, _) = spartan::prove_with(&shape, &gens, &inst, &bad_wit, &mut rng, cheat);
            assert!(
                unsatisfied(&shape, &gens, &inst, &forged, &[]),
                "{cheat:?} refused"
            );
        }
    }

    fn small_pair_spec<'a>(gv: &'a Gens<Vesta>, gp: &'a Gens<Pallas>, seed: &[u8]) -> PairSpec<'a> {
        let shape = Shape {
            label: "pair-test",
            sx: 13,
            row_bits: 2,
            col_bits: 11,
            k: 3,
        };
        let (inst_p, wit_p, claims_p) =
            instance::<Vesta>(shape, gv, 2, 16, &[seed, b"/p"].concat());
        let (inst_s, wit_s, claims_s) =
            instance::<Pallas>(shape, gp, 2, 16, &[seed, b"/s"].concat());
        PairSpec {
            shape_p: shape,
            gens_p: gv,
            inst_p,
            wit_p,
            claims_p,
            shape_s: shape,
            gens_s: gp,
            inst_s,
            wit_s,
            claims_s,
        }
    }

    /// The pairing link: honest pairs verify; a P or S from another pair, a
    /// different R, or a rho drawn before S's rows are fixed is refused; and
    /// the link's in-circuit replay is satisfied by real pairs.
    #[test]
    fn pairing_link_binds_partners() {
        let gv = Gens::<Vesta>::new(1 << 11);
        let gp = Gens::<Pallas>::new(1 << 11);
        let a = small_pair_spec(&gv, &gp, b"pair-a");
        let b = small_pair_spec(&gv, &gp, b"pair-b");
        let values: Vec<u128> = (0..40)
            .map(|i| (i as u128 + 1) * 0x1234_5678_9abc_def1)
            .collect();
        let mu = Fq::from(99);
        let rlc = |rho: &[u8; 32]| rlc_value(rho, mu, &values).to_repr();
        let pa = prove_pair(&a, b"a", &rlc);
        let pb = prove_pair(&b, b"b", &rlc);
        assert!(
            verify_pair(&a, &pa.p, &pa.s, pa.r).is_ok(),
            "honest pair verifies"
        );
        assert!(verify_pair(&b, &pb.p, &pb.s, pb.r).is_ok());
        assert!(
            verify_pair(&a, &pa.p, &pb.s, pa.r).is_err(),
            "S from another pair refused"
        );
        assert!(
            verify_pair(&a, &pa.p, &pb.s, pb.r).is_err(),
            "S from another pair with its own R refused"
        );
        assert!(
            verify_pair(&b, &pa.p, &pb.s, pb.r).is_err(),
            "P from another pair refused"
        );
        let mut r2 = pa.r;
        r2[0] ^= 1;
        assert!(
            verify_pair(&a, &pa.p, &pa.s, r2).is_err(),
            "a different R refused"
        );
        // rho follows S's rows: another S witness gives another rho.
        let mut c = small_pair_spec(&gv, &gp, b"pair-a");
        c.wit_s = small_pair_spec(&gv, &gp, b"pair-c").wit_s;
        c.inst_s = small_pair_spec(&gv, &gp, b"pair-c").inst_s;
        let pc = prove_pair(&c, b"a", &rlc);
        assert_eq!(pc.d, pa.d, "same P rows, same digest");
        assert_ne!(pc.rho, pa.rho, "rho depends on S's phase-1 rows");
        // In-circuit replay of both linked proofs.
        let fixed = move |_: &[u8; 32]| pa.r;
        let sp = synthesize_proof(
            &a.shape_p,
            &gv,
            &a.inst_p,
            &pa.p,
            &a.claims_p,
            &Link::First {
                rho: pa.rho,
                r: pa.r,
            },
        );
        let ss = synthesize_proof(
            &a.shape_s,
            &gp,
            &a.inst_s,
            &pa.s,
            &a.claims_s,
            &Link::Second {
                d: pa.d,
                rlc: &fixed,
            },
        );
        assert!(sp.points.total > 0 && ss.points.total > 0);
    }

    /// The binding's RLC: both chains (native in C_S, non-native in C_P) give
    /// the same R; a differing, reordered, omitted or duplicated copy changes
    /// R; a rho known before the copies are committed admits a forgery with a
    /// short lattice vector, which a fresh rho refuses.
    #[test]
    fn binding_rlc_refusals() {
        let values: Vec<u128> = (0..60)
            .map(|i| (0xfedc_ba98_7654_3210_u128 << 64 | i as u128) ^ ((i as u128) << 100))
            .collect();
        let mu = Fq::from(12345);
        let rho = Fq::from(0x5eed_u64).square().square().double().to_repr();
        let r = rlc_value(&rho, mu, &values);
        let (_, rn) = rlc_chain_native(&rho, mu, &values);
        assert_eq!(rn, r, "native chain");
        let mu_big = num_bigint::BigUint::from_bytes_le(&mu.to_repr());
        let (_, rp) = crate::operation::rlc_chain(
            11,
            &num_bigint::BigUint::from_bytes_le(&rho),
            &mu_big,
            &values,
        );
        assert_eq!(
            rp,
            num_bigint::BigUint::from_bytes_le(&r.to_repr()),
            "non-native chain"
        );
        let mut altered = values.clone();
        altered[17] ^= 1;
        assert_ne!(rlc_value(&rho, mu, &altered), r, "differing copy");
        let mut swapped = values.clone();
        swapped.swap(3, 4);
        assert_ne!(rlc_value(&rho, mu, &swapped), r, "reordered copies");
        assert_ne!(rlc_value(&rho, mu, &values[1..]), r, "omitted copy");
        let mut dup = values.clone();
        dup.insert(5, values[5]);
        assert_ne!(rlc_value(&rho, mu, &dup), r, "duplicated copy");
        // rho known early: delta rho + eps = 0 (mod q) with |delta|, |eps| < 2^128.
        let (delta, eps) = short_relation(&rho);
        assert!(
            delta.bits() <= 128 && eps.bits() <= 128,
            "short vector: {} / {} bits",
            delta.bits(),
            eps.bits()
        );
        let fits = |v: u128, d: &num_bigint::BigInt| -> Option<u128> {
            let x = num_bigint::BigInt::from(v) + d;
            (x.sign() != num_bigint::Sign::Minus && x.bits() <= 128).then(|| x.to_u128().unwrap())
        };
        let mut forged = None;
        'search: for sign in [1, -1] {
            let (d, e) = (&delta * sign, &eps * sign);
            for j in 0..values.len() - 1 {
                if let (Some(a), Some(b)) = (fits(values[j], &d), fits(values[j + 1], &e)) {
                    let mut f = values.clone();
                    f[j] = a;
                    f[j + 1] = b;
                    forged = Some(f);
                    break 'search;
                }
            }
        }
        let forged = forged.expect("a forgeable adjacent pair");
        assert_ne!(forged, values);
        assert_eq!(
            rlc_value(&rho, mu, &forged),
            r,
            "rho known early: forged copies, same R"
        );
        let fresh = Fq::from(0xfeed_u64).square().square().to_repr();
        assert_ne!(
            rlc_value(&fresh, mu, &forged),
            rlc_value(&fresh, mu, &values),
            "a fresh rho refuses them"
        );
    }

    /// Assigned-cell mutation: every witness cell of both verifier sides, on
    /// both curves, is pinned by some constraint.
    #[test]
    fn verifier_cells_are_constrained() {
        MUTATE.store(true, std::sync::atomic::Ordering::Relaxed);
        let shape = Shape {
            label: "mutation-test",
            sx: 13,
            row_bits: 2,
            col_bits: 11,
            k: 3,
        };
        let gv = Gens::<Vesta>::new(1 << shape.col_bits);
        let sv = synthesize::<Vesta>(shape, &gv, 2, 16, b"mv");
        let gp = Gens::<Pallas>::new(1 << shape.col_bits);
        let sp = synthesize::<Pallas>(shape, &gp, 1, 16, b"mp");
        MUTATE.store(false, std::sync::atomic::Ordering::Relaxed);
        for side in [&sv.points, &sv.scalar, &sp.points, &sp.scalar] {
            let free = side.free_cells.as_ref().expect("mutation run");
            assert!(free.is_empty(), "unconstrained cells: {free:?}");
        }
    }

    /// Small shapes on both curves: every constraint satisfied by a real proof
    /// with inherited claims.
    #[test]
    fn synthesis_accepts_real_proofs() {
        for k in [2, 3] {
            let shape = Shape {
                label: "verifier-test",
                sx: 13,
                row_bits: 2,
                col_bits: 11,
                k,
            };
            let gv = Gens::<Vesta>::new(1 << shape.col_bits);
            let s = synthesize::<Vesta>(shape, &gv, 2, 16, b"vt");
            assert!(s.points.total > 0 && s.scalar.total > 0);
            let gp = Gens::<Pallas>::new(1 << shape.col_bits);
            let s = synthesize::<Pallas>(shape, &gp, 1, 16, b"pt");
            assert!(s.points.total > 0 && s.scalar.total > 0);
        }
    }
}
