//! Stage A2, step 7: the verifier's points side as uniform arrays.
//!
//! `PointsBuilder::verify` replays one proof's points side (stage A's
//! `verifier::points_side`) natively and records it; `build` lays out every
//! recorded proof of the circuit in shared arrays:
//!
//! - the transcript sponge: permutations of the circuit's pool (full.rs),
//!   each absorbing up to two words per lane read (a point's coordinate, a
//!   shared limb, an embedded word, or zero);
//! - squeezes: `s0 = c1 + 2^126 c2 + 2^252 c3` (two challenges, range-checked
//!   halves) or `s0 = lo + 2^128 hi` (a full-width challenge), each with the
//!   canonicity identity `s0 + d = p - 1` over limbs with carries;
//! - points: the proof's points (on-curve) and P' (read back from its
//!   addition, for the transcript), coordinates exported as words and pairs;
//! - additions (complete, both slope equations), operands read from any point
//!   export, the second one's y times a sign column (the row fold subtracts);
//! - Halo endoscalings and the variable/fixed-base multiplications (ladders.rs
//!   families): bases read, scalars read against the squeezes or the shared
//!   limbs (the multiplication's running sum at bit 128 is the high limb),
//!   results exported;
//! - the shared values: the challenges bound to their squeezes; the absorbed
//!   limbs, the multiplications' scalars and the claims' coordinates read from
//!   the binding chain's shared array.
//!
//! Copies carry everything between these arrays (copy reads into ladder
//! families, and across the operation DAG); links bind the global arrays to
//! the binding chain.

use ff::{Field, PrimeField};
use group::Group;
use iroha_pasta::PastaCurve;
use iroha_pasta::poseidon::PoseidonField;

use crate::fs::{Scalar, Transcript, endo_scalar, lean_schedule};
use crate::full::{Pool, Src3, Wiring};
use crate::ipa::{Aff, Claim, F, Gens};
use crate::ladders;
#[path = "points_complete.rs"]
mod complete;
use crate::r1cs::Shape;
use crate::spartan::Proof;
use crate::uniform::{Array, Lin, Src, Term, constant, link, own, plus, scaled};
use crate::verifier::{Kind, Trace, shared_scalars};
use complete::coords;
#[path = "generator_claim_points.rs"]
mod generator_claim_points;
pub use generator_claim_points::{GeneratorEquality, GeneratorPoints};

/// A point the DAG produces: an input point, an addition, an endoscaling, a
/// multiplication.
#[derive(Clone, Copy, Debug)]
pub enum PtRef {
    Input(usize),
    Add(usize),
    Endo(usize),
    Mixed(usize),
}

/// A word the transcript absorbs.
#[derive(Clone, Copy, Debug)]
pub enum Word {
    X(usize),
    Y(usize),
    /// A shared value (binding index).
    V(usize),
    /// An embedded scalar `lo + 2^128 hi` (index into the embedded words).
    Embed(usize),
}

/// A challenge: a squeeze's first or second half, or a full-width squeeze's
/// low or high limb.
#[derive(Clone, Copy, Debug)]
pub enum ChRef {
    Half(usize, bool),
    Full(usize, bool),
}

struct AddSpec<F> {
    p: PtRef,
    q: PtRef,
    neg: bool,
    /// px, py, qx, qy (raw), then the row values.
    pv: (F, F),
    qv: (F, F),
}

struct EndoSpec<C: PastaCurve> {
    base: PtRef,
    point: C,
    bits: Vec<bool>,
    ch: ChRef,
}

struct MixedSpec<C: PastaCurve> {
    base: Option<PtRef>,
    point: C,
    fixed: bool,
    bits: Vec<bool>,
    limbs: [usize; 2],
}

struct SqueezeSpec<F> {
    perm: usize,
    full: bool,
    out: [F; 3],
}

/// A verifier slot that takes a proof of any of three circuit kinds of one
/// layout (Receive, Send, Bootstrap): the kinds' fixed-row commitments (their
/// keys' constants) and the binding index of the slot's kind bits c0 (then
/// c1 next), shared values with c0 c1 = 0. The fixed rows are pinned to
/// `key0 + c0 (key1 - key0) + c1 (key2 - key0)`, linear in the bits.
pub struct KeySelect<'a, C: PastaCurve> {
    pub fixed: Vec<&'a [Aff<C>]>,
    pub kind_at: usize,
}

/// A constant input's alternative keys: the three kinds' coordinates and the
/// kind bits' first binding index.
type KeyAlt<F> = (Vec<(F, F)>, usize);

/// Binding indices for one proof's shared values.
#[derive(Clone, Copy, Debug)]
pub struct SharedMap {
    /// The proof's first shared value.
    pub base: usize,
    /// The absorbed other word's limbs and R's limbs (paired proofs).
    pub other: Option<[usize; 2]>,
    pub r: Option<[usize; 2]>,
}

/// Exports the builder reserves up front (the pool's lane reads name them).
#[derive(Clone, Copy, Debug)]
pub struct Exports {
    pub x: usize,
    pub y: usize,
    pub pt: usize,
    pub add: usize,
    pub endo: usize,
    pub mixed: usize,
    pub c1: usize,
    pub c2: usize,
    pub lo: usize,
    pub hi: usize,
    pub pairs: usize,
    pub embed: usize,
    pub header: usize,
    pub valid: usize,
}

pub struct PointsBuilder<C: PastaCurve> {
    pub e: Exports,
    /// The pool's output export, the shared values' export, a zero word.
    e_out: usize,
    e_v: usize,
    e_zero: usize,
    inputs: Vec<(C::Base, C::Base, Option<PtRef>)>,
    adds: Vec<AddSpec<C::Base>>,
    endo: Vec<EndoSpec<C>>,
    mixed: Vec<MixedSpec<C>>,
    squeezes: Vec<SqueezeSpec<C::Base>>,
    equal: Vec<(usize, usize)>,
    soft_equal: Vec<(usize, usize, Option<usize>)>,
    total: bool,
    /// (binding index, the challenge it equals).
    v_from: Vec<(usize, ChRef)>,
    /// (input point, binding indices of x lo, x hi, y lo, y hi).
    claim_limbs: Vec<(usize, [usize; 4])>,
    embeds: Vec<usize>,
    headers: Vec<[C::Base; 3]>,
    /// Inputs that are constants (a circuit proof's fixed rows: the key's
    /// commitments), each with its alternative keys when a kind bit selects
    /// between two circuits' keys.
    consts: Vec<(usize, Option<KeyAlt<C::Base>>)>,
    finite: Vec<usize>,
    pub perms: usize,
}

/// The sponge replay's state.
struct Sp<F> {
    src: Src3,
    val: [F; 3],
    pending: usize,
    lanes: [Option<(Word, F)>; 2],
    spare: Option<ChRef>,
}

impl<C: PastaCurve> PointsBuilder<C>
where
    C::Base: Scalar + PoseidonField,
{
    pub fn new(w: &mut Wiring<C::Base>, e_out: usize, e_v: usize, e_zero: usize) -> Self {
        let mut r = || w.reserve();
        let e = Exports {
            x: r(),
            y: r(),
            pt: r(),
            add: r(),
            endo: r(),
            mixed: r(),
            c1: r(),
            c2: r(),
            lo: r(),
            hi: r(),
            pairs: r(),
            embed: r(),
            header: r(),
            valid: r(),
        };
        PointsBuilder {
            e,
            e_out,
            e_v,
            e_zero,
            inputs: Vec::new(),
            adds: Vec::new(),
            endo: Vec::new(),
            mixed: Vec::new(),
            squeezes: Vec::new(),
            equal: Vec::new(),
            soft_equal: Vec::new(),
            total: false,
            v_from: Vec::new(),
            claim_limbs: Vec::new(),
            embeds: Vec::new(),
            headers: Vec::new(),
            consts: Vec::new(),
            finite: Vec::new(),
            perms: 0,
        }
    }

    fn word_src(&self, wd: Word) -> (usize, usize) {
        match wd {
            Word::X(i) => (self.e.x, i),
            Word::Y(i) => (self.e.y, i),
            Word::V(v) => (self.e_v, v),
            Word::Embed(k) => (self.e.embed, k),
        }
    }

    fn permute(&mut self, pool: &mut Pool<C::Base>, sp: &mut Sp<C::Base>) -> usize {
        let ab = [
            sp.lanes[0].map_or(C::Base::ZERO, |l| l.1),
            sp.lanes[1].map_or(C::Base::ZERO, |l| l.1),
        ];
        let srcs =
            [0, 1].map(|k| Some(sp.lanes[k].map_or((self.e_zero, 0), |l| self.word_src(l.0))));
        let q = pool.chain(sp.src, sp.val, ab, srcs);
        sp.src = Src3::Out(q);
        sp.val = pool.out(q);
        sp.pending = 0;
        sp.lanes = [None, None];
        self.perms += 1;
        q
    }

    fn absorb(&mut self, pool: &mut Pool<C::Base>, sp: &mut Sp<C::Base>, wd: Word, v: C::Base) {
        if sp.pending == 2 {
            self.permute(pool, sp);
        }
        sp.lanes[sp.pending] = Some((wd, v));
        sp.pending += 1;
        sp.spare = None;
    }

    fn input(&mut self, pool: &mut Pool<C::Base>, sp: &mut Sp<C::Base>, p: &Aff<C>) -> (PtRef, C) {
        let pt = C::from(*p);
        let (x, y) = coords(&pt);
        let i = self.inputs.len();
        self.inputs.push((x, y, None));
        self.absorb(pool, sp, Word::X(i), x);
        self.absorb(pool, sp, Word::Y(i), y);
        (PtRef::Input(i), pt)
    }

    fn add(&mut self, p: (PtRef, C), q: (PtRef, C), neg: bool) -> (PtRef, C) {
        let r = if neg { p.1 - q.1 } else { p.1 + q.1 };
        self.adds.push(AddSpec {
            p: p.0,
            q: q.0,
            neg,
            pv: coords(&p.1),
            qv: coords(&q.1),
        });
        (PtRef::Add(self.adds.len() - 1), r)
    }

    fn endoscale(&mut self, base: (PtRef, C), ch: ChRef, bits: &[bool]) -> (PtRef, C) {
        let r = base.1 * endo_scalar::<F<C>>(bits);
        self.endo.push(EndoSpec {
            base: base.0,
            point: base.1,
            bits: bits.to_vec(),
            ch,
        });
        (PtRef::Endo(self.endo.len() - 1), r)
    }

    /// A multiplication: variable base `[2c + 2^255 + 1] P`, or fixed base
    /// `[k] B`, `scalar` given by its 255 bits; `limbs` its shared limbs.
    fn multiply(
        &mut self,
        base: Option<(PtRef, C)>,
        fixed_base: Option<C>,
        scalar: &F<C>,
        limbs: [usize; 2],
    ) -> (PtRef, C) {
        let repr = scalar.to_repr();
        let bits: Vec<bool> = (0..255)
            .map(|i| (repr.as_ref()[i / 8] >> (i % 8)) & 1 == 1)
            .collect();
        let (point, fixed, r) = match (base, fixed_base) {
            (Some((_, p)), None) => {
                let two = F::<C>::from(2u64);
                let k = *scalar * two + two.pow_vartime([255u64]) + F::<C>::ONE;
                (p, false, p * k)
            }
            (None, Some(b)) => (b, true, b * *scalar),
            _ => unreachable!("a base or a fixed base"),
        };
        self.mixed.push(MixedSpec {
            base: base.map(|b| b.0),
            point,
            fixed,
            bits,
            limbs,
        });
        (PtRef::Mixed(self.mixed.len() - 1), r)
    }

    fn squeeze(&mut self, pool: &mut Pool<C::Base>, sp: &mut Sp<C::Base>, full: bool) -> usize {
        let q = self.permute(pool, sp);
        self.squeezes.push(SqueezeSpec {
            perm: q,
            full,
            out: pool.out(q),
        });
        self.squeezes.len() - 1
    }

    /// Replays one proof's points side (as `verifier::points_side`). With
    /// `keys` (a slot whose proof's kind a shared bit selects), its key's
    /// fixed rows are pinned to the selected key's commitments; otherwise to
    /// its own circuit's. Its wallet row (the first fixed row, when its
    /// circuit has one) is pinned to the shared values from `wallet` on: the
    /// table commitment its io names (x low, x high, y low, y high).
    #[allow(clippy::too_many_arguments)]
    pub fn verify_keyed(
        &mut self,
        pool: &mut Pool<C::Base>,
        shape: &Shape,
        gens: &Gens<C>,
        proof: &Proof<C>,
        claims: &[Claim<C>],
        tr: &Trace<C>,
        sm: SharedMap,
        keys: Option<&KeySelect<C>>,
        wallet: Option<usize>,
    ) -> TotalPoints {
        let start = self.inputs.len();
        self.verify_keyed_inner(
            pool, shape, gens, proof, claims, tr, sm, keys, wallet, false,
        );
        self.proof_ports(start, usize::MAX, proof, claims.len(), tr)
    }

    /// Total verification keeps transcript/group computation hard and returns
    /// the iff final Schnorr equality, including identity/opposite cases.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_keyed_total(
        &mut self,
        pool: &mut Pool<C::Base>,
        shape: &Shape,
        gens: &Gens<C>,
        proof: &Proof<C>,
        claims: &[Claim<C>],
        tr: &Trace<C>,
        sm: SharedMap,
        keys: Option<&KeySelect<C>>,
        wallet: Option<usize>,
    ) -> TotalPoints {
        self.total = true;
        let start = self.inputs.len();
        let verdict = self.soft_equal.len();
        self.verify_keyed_inner(pool, shape, gens, proof, claims, tr, sm, keys, wallet, true);
        self.proof_ports(start, verdict, proof, claims.len(), tr)
    }

    fn proof_ports(
        &self,
        start: usize,
        verdict: usize,
        proof: &Proof<C>,
        claims_len: usize,
        tr: &Trace<C>,
    ) -> TotalPoints {
        let fixed = tr.circuit.as_ref().map_or(0, |c| c.fixed.len());
        let rows = proof.rows.len();
        let rounds = proof.outer.len() + proof.inner.len();
        let mut wire_nodes: Vec<_> = (start..start + rows).collect();
        wire_nodes.extend(start + rows + fixed..start + rows + fixed + rounds);
        let ipa = start + rows + fixed + rounds + claims_len + 1;
        wire_nodes.extend(ipa..ipa + 2 * proof.ipa.lr.len() + 2);
        assert_eq!(wire_nodes.len(), rows + rounds + 2 * proof.ipa.lr.len() + 2);
        TotalPoints {
            verdict,
            wire_nodes,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn verify_keyed_inner(
        &mut self,
        pool: &mut Pool<C::Base>,
        shape: &Shape,
        gens: &Gens<C>,
        proof: &Proof<C>,
        claims: &[Claim<C>],
        tr: &Trace<C>,
        sm: SharedMap,
        keys: Option<&KeySelect<C>>,
        wallet: Option<usize>,
        total: bool,
    ) {
        let lean = lean_schedule();
        let embed = Transcript::<C>::scalars_embed();
        let sh = shared_scalars(tr, proof);
        let mut ch_at = Vec::with_capacity(tr.challenges.len());
        let mut at = sm.base;
        for (_, kind, _) in &tr.challenges {
            ch_at.push(at);
            at += if *kind == Kind::Full { 2 } else { 1 };
        }
        let full_base = at;
        let claims_base = full_base + 18;
        let k_header = self.headers.len();
        self.headers.push(tr.header.0);
        let mut sp = Sp {
            src: Src3::Entry(self.e.header, k_header),
            val: tr.header.0,
            pending: tr.header.1,
            lanes: [None, None],
            spare: None,
        };
        let mut next = 0usize;
        // The challenges in transcript order, bound to the shared values.
        let mut challenge =
            |me: &mut Self, pool: &mut Pool<C::Base>, sp: &mut Sp<C::Base>| -> (ChRef, Vec<bool>) {
                let (want, kind, fresh) = tr.challenges[next].clone();
                let ch = if kind == Kind::Full {
                    let s = me.squeeze(pool, sp, true);
                    sp.spare = None;
                    ChRef::Full(s, false)
                } else if fresh {
                    let s = me.squeeze(pool, sp, false);
                    sp.spare = Some(ChRef::Half(s, true));
                    ChRef::Half(s, false)
                } else {
                    sp.spare.take().expect("a spare half")
                };
                let v = ch_at[next];
                match ch {
                    ChRef::Full(s, _) => {
                        me.v_from.push((v, ChRef::Full(s, false)));
                        me.v_from.push((v + 1, ChRef::Full(s, true)));
                    }
                    half => me.v_from.push((v, half)),
                }
                next += 1;
                (ch, want)
            };
        let lw = |l: usize| -> Vec<Word> {
            if embed {
                Vec::new()
            } else {
                vec![Word::V(l), Word::V(l + 1)]
            }
        };
        let words = |me: &mut Self, l: usize| -> Vec<Word> {
            if embed {
                me.embeds.push(l);
                vec![Word::Embed(me.embeds.len() - 1)]
            } else {
                lw(l)
            }
        };
        // Absorbed shared words: their values from the shared value list.
        let shared = crate::verifier::shared_values(tr, proof, claims);
        let sv = |v: usize| -> u128 { shared[v - sm.base] };
        let absorb_words = |me: &mut Self,
                            pool: &mut Pool<C::Base>,
                            sp: &mut Sp<C::Base>,
                            wds: Vec<Word>,
                            other: Option<[u128; 2]>| {
            for (k, wd) in wds.into_iter().enumerate() {
                let v = match wd {
                    Word::V(v) if v >= sm.base && v < sm.base + shared.len() => {
                        C::Base::from_u128(sv(v))
                    }
                    Word::V(_) => C::Base::from_u128(other.expect("an outside word")[k]),
                    Word::Embed(k2) => {
                        let l = me.embeds[k2];
                        C::Base::from_u128(sv(l))
                            + C::Base::from(2u64).pow_vartime([128u64])
                                * C::Base::from_u128(sv(l + 1))
                    }
                    _ => unreachable!(),
                };
                me.absorb(pool, sp, wd, v);
            }
        };
        let limbs_of = |repr: [u8; 32]| -> [u128; 2] {
            [
                u128::from_le_bytes(repr[..16].try_into().unwrap()),
                u128::from_le_bytes(repr[16..].try_into().unwrap()),
            ]
        };
        // The pair link's word, right after the first phase's rows (all of a
        // toy proof's): the first proof squeezes d and absorbs rho, the second
        // absorbs d and squeezes rho. The absorbed word arrives as two shared
        // limbs.
        let mut link_word = |me: &mut Self, pool: &mut Pool<C::Base>, sp: &mut Sp<C::Base>| {
            if let Some((first, _, other, _)) = tr.link {
                if first {
                    let _ = challenge(me, pool, sp);
                }
                let o = sm.other.expect("the other word's limbs");
                absorb_words(
                    me,
                    pool,
                    sp,
                    vec![Word::V(o[0]), Word::V(o[1])],
                    Some(limbs_of(other)),
                );
                if !first {
                    let _ = challenge(me, pool, sp);
                }
            }
        };
        let io_w = words(self, full_base);
        absorb_words(self, pool, &mut sp, io_w, None);
        let rows: Vec<(PtRef, C)> = match &tr.circuit {
            None => {
                let out = proof
                    .rows
                    .iter()
                    .map(|r| self.input(pool, &mut sp, r))
                    .collect();
                link_word(self, pool, &mut sp);
                out
            }
            Some(ct) => {
                // A circuit proof: the first phase's rows, the pair link's
                // word, the link phase's rows, the second-phase challenges,
                // the late io's digest (two shared limbs), then the second
                // phase's rows and the fixed rows (constants).
                let all: Vec<Aff<C>> = proof.rows.iter().chain(&ct.fixed).copied().collect();
                let mut out: Vec<(PtRef, C)> = all[..ct.p1]
                    .iter()
                    .map(|r| self.input(pool, &mut sp, r))
                    .collect();
                link_word(self, pool, &mut sp);
                for r in &all[ct.p1..ct.p1 + ct.pl] {
                    let pt = self.input(pool, &mut sp, r);
                    out.push(pt);
                }
                for _ in 0..ct.phase2.len() {
                    let _ = challenge(self, pool, &mut sp);
                }
                if ct.late_digest.is_some() {
                    let late_base = claims_base + 4 * claims.len();
                    let wd = words(self, late_base);
                    absorb_words(self, pool, &mut sp, wd, None);
                }
                let first_fixed = all.len() - ct.fixed.len();
                let nw = ct.wallet_rows;
                assert!(nw <= 1, "one wallet row at most");
                assert_eq!(
                    wallet.is_some(),
                    nw == 1,
                    "a wallet row's commitment comes from shared limbs"
                );
                if let Some(ks) = keys {
                    assert!(
                        (2..=4).contains(&ks.fixed.len()),
                        "two to four circuit kinds"
                    );
                    assert!(
                        ks.fixed.iter().all(|f| f.len() == ct.fixed.len() - nw),
                        "every kind's key has the proof's key rows"
                    );
                    assert!(
                        ks.fixed.iter().any(|f| *f == &ct.fixed[nw..]),
                        "the proof's key is one of the slot's"
                    );
                }
                for (k, r) in all.iter().enumerate().skip(ct.p1 + ct.pl) {
                    let pt = self.input(pool, &mut sp, r);
                    if k >= first_fixed {
                        let f = k - first_fixed;
                        match (pt, wallet) {
                            ((PtRef::Input(i), _), Some(b)) if f < nw => {
                                self.claim_limbs.push((i, [b, b + 1, b + 2, b + 3]));
                            }
                            ((PtRef::Input(i), _), _) => {
                                let alt = keys.map(|ks| {
                                    let at = |kind: usize| coords(&C::from(ks.fixed[kind][f - nw]));
                                    ((0..ks.fixed.len()).map(at).collect(), ks.kind_at)
                                });
                                self.consts.push((i, alt));
                            }
                            _ => assert!(f >= nw, "a wallet row is an input"),
                        }
                    }
                    out.push(pt);
                }
                out
            }
        };
        // The link's R, after every row.
        if let Some((_, _, _, r)) = tr.link {
            let rl = sm.r.expect("R's limbs");
            absorb_words(
                self,
                pool,
                &mut sp,
                vec![Word::V(rl[0]), Word::V(rl[1])],
                Some(limbs_of(r)),
            );
        }
        let n_tau = if lean { 1 } else { shape.sx };
        for _ in 0..n_tau {
            let _ = challenge(self, pool, &mut sp);
        }
        let mut rounds: Vec<(PtRef, C)> = Vec::new();
        for (commitment, &k) in proof.outer.iter().zip(&shape.outer_blocks()) {
            rounds.push(self.input(pool, &mut sp, commitment));
            for _ in 0..if lean { 1 } else { k } {
                let _ = challenge(self, pool, &mut sp);
            }
        }
        for i in 0..3 {
            let wd = words(self, full_base + 2 * (1 + i));
            absorb_words(self, pool, &mut sp, wd, None);
        }
        for _ in 0..if lean { 1 } else { 3 } {
            let _ = challenge(self, pool, &mut sp);
        }
        let mut row_bits: Vec<(ChRef, Vec<bool>)> = Vec::new();
        let mut g = 0;
        for (commitment, &k) in proof.inner.iter().zip(&shape.inner_blocks()) {
            rounds.push(self.input(pool, &mut sp, commitment));
            if lean && g > shape.row_bits {
                let _ = challenge(self, pool, &mut sp);
                g += k;
                continue;
            }
            for _ in 0..k {
                let ch = challenge(self, pool, &mut sp);
                if (1..=shape.row_bits).contains(&g) {
                    row_bits.push(ch);
                }
                g += 1;
            }
        }
        let _alpha = challenge(self, pool, &mut sp);
        let gamma = challenge(self, pool, &mut sp);
        // Hyrax row fold: T' = T_lo + rho (T_hi - T_lo).
        let mut level = rows;
        for (ch, bits) in &row_bits {
            let half = level.len() / 2;
            let mut next_level = Vec::with_capacity(half);
            for i in 0..half {
                let diff = self.add(level[half + i], level[i], true);
                let scaled = self.endoscale(diff, *ch, bits);
                next_level.push(self.add(level[i], scaled, false));
            }
            level = next_level;
        }
        assert_eq!(level.len(), 1, "rows fold to one");
        let mut acc = *rounds.last().unwrap();
        for d in rounds.iter().rev().skip(1) {
            let scaled = self.endoscale(acc, gamma.0, &gamma.1);
            acc = self.add(*d, scaled, false);
        }
        let round_sum = self.endoscale(acc, gamma.0, &gamma.1);
        let p = self.add(level[0], round_sum, false);
        assert_eq!(coords(&p.1), coords(&tr.p), "combined commitment P");
        let mut p2 = p;
        if !claims.is_empty() {
            let mut pts = Vec::new();
            for (j, c) in claims.iter().enumerate() {
                let pt = self.input(pool, &mut sp, &c.g);
                if let PtRef::Input(i) = pt.0 {
                    let b = claims_base + 4 * j;
                    self.claim_limbs.push((i, [b, b + 1, b + 2, b + 3]));
                }
                pts.push(pt);
            }
            let xi = challenge(self, pool, &mut sp);
            let mut acc = *pts.last().unwrap();
            for gp in pts.iter().rev().skip(1) {
                let scaled = self.endoscale(acc, xi.0, &xi.1);
                acc = self.add(*gp, scaled, false);
            }
            let scaled = self.endoscale(acc, xi.0, &xi.1);
            p2 = self.add(p2, scaled, false);
        }
        assert_eq!(
            coords(&p2.1),
            coords(&tr.p_claims),
            "P with inherited claims"
        );
        // P' enters the transcript: a points instance read back from its addition.
        let (x2, y2) = coords(&p2.1);
        let ip = self.inputs.len();
        self.inputs.push((x2, y2, Some(p2.0)));
        self.absorb(pool, &mut sp, Word::X(ip), x2);
        self.absorb(pool, &mut sp, Word::Y(ip), y2);
        let kw = words(self, full_base + 2 * 4);
        absorb_words(self, pool, &mut sp, kw, None);
        let _zeta = challenge(self, pool, &mut sp);
        let u_pt = C::from(gens.u);
        let ku = self.multiply(
            None,
            Some(u_pt),
            &sh.kappa_zeta,
            [full_base + 10, full_base + 11],
        );
        let mut folded = self.add(p2, ku, false);
        assert_eq!(coords(&folded.1), coords(&tr.folded[0]), "IPA start");
        for (j, (l, r)) in proof.ipa.lr.iter().enumerate() {
            let lp = self.input(pool, &mut sp, l);
            let rp = self.input(pool, &mut sp, r);
            let u = challenge(self, pool, &mut sp);
            let ul = self.endoscale(lp, u.0, &u.1);
            let inner = self.add(folded, ul, false);
            let scaled = self.endoscale(inner, u.0, &u.1);
            folded = self.add(scaled, rp, false);
            assert_eq!(coords(&folded.1), coords(&tr.folded[j + 1]), "IPA round");
        }
        let gf = self.input(pool, &mut sp, &proof.ipa.g_final);
        let a = self.input(pool, &mut sp, &proof.ipa.a);
        let c = challenge(self, pool, &mut sp);
        assert_eq!(next, tr.challenges.len(), "every challenge replayed");
        let t1 = self.multiply(Some(gf), None, &sh.c_z1, [full_base + 12, full_base + 13]);
        let t2 = self.multiply(
            None,
            Some(u_pt),
            &sh.z1_bf_zeta,
            [full_base + 14, full_base + 15],
        );
        let t3 = self.multiply(
            None,
            Some(C::from(gens.h)),
            &proof.ipa.z2,
            [full_base + 16, full_base + 17],
        );
        let t12 = self.add(t1, t2, false);
        let lhs = self.add(t12, t3, false);
        let cf = self.endoscale(folded, c.0, &c.1);
        let rhs = self.add(a, cf, false);
        if !total {
            assert!(!bool::from(a.1.is_identity()), "Schnorr A must be finite");
            let PtRef::Input(ai) = a.0 else {
                unreachable!()
            };
            self.finite.push(ai);
            assert_eq!(coords(&lhs.1), coords(&rhs.1), "the final Schnorr check");
        }
        if let (PtRef::Add(l), PtRef::Add(r)) = (lhs.0, rhs.0) {
            if total {
                let PtRef::Input(ai) = a.0 else {
                    unreachable!()
                };
                self.soft_equal.push((l, r, Some(ai)));
            } else {
                self.equal.push((l, r));
            }
        }
    }
}

// ------------------------------------------------------------------ layout

/// Sealed result of the total verifier. The raw proof point sources exclude
/// key constants, inherited claims and intermediate transcript commitments.
pub struct TotalPoints {
    verdict: usize,
    wire_nodes: Vec<usize>,
}
impl TotalPoints {
    pub fn validity(&self, built: &PointsIdx) -> (usize, usize, usize) {
        (
            built.validity.expect("total verifier verdict array"),
            0,
            self.verdict,
        )
    }
    /// The exact raw proof's own folded generator, before any correction.
    pub fn own_generator(&self, built: &PointsIdx) -> [(usize, usize, usize); 2] {
        let i = self.wire_nodes[self.wire_nodes.len() - 2];
        [(built.points, 0, i), (built.points, 1, i)]
    }
    pub fn wire_points(&self, built: &PointsIdx) -> Vec<[(usize, usize, usize); 2]> {
        self.wire_nodes
            .iter()
            .map(|i| [(built.points, 0, *i), (built.points, 1, *i)])
            .collect()
    }
}

/// Where the points side's arrays sit.
#[derive(Clone, Copy, Debug)]
pub struct PointsIdx {
    pub points: usize,
    pub adds: usize,
    pub endo_base: usize,
    pub endo_step: usize,
    pub mixed_base: usize,
    pub mixed_end: usize,
    pub squeeze: usize,
    pub squeeze_full: usize,
    pub validity: Option<usize>,
}

const CH: usize = crate::nonnative::CHUNK;

fn chunk_ws(width: usize) -> Vec<usize> {
    let mut w = Vec::new();
    let mut left = width;
    while left > 0 {
        let c = CH.min(left);
        w.push(c);
        left -= c;
    }
    w
}

fn p2<Fb: PrimeField>(e: usize) -> Fb {
    Fb::from(2u64).pow_vartime([e as u64])
}

fn big<Fb: PrimeField>(x: &num_bigint::BigUint) -> Fb {
    let mut r = Fb::Repr::default();
    let b = x.to_bytes_le();
    r.as_mut()[..b.len()].copy_from_slice(&b);
    Option::from(Fb::from_repr(r)).expect("below the modulus")
}

fn modulus<Fb: PrimeField>() -> num_bigint::BigUint {
    num_bigint::BigUint::from_bytes_le((-Fb::ONE).to_repr().as_ref()) + 1u32
}

fn to_big<Fb: PrimeField>(x: Fb) -> num_bigint::BigUint {
    num_bigint::BigUint::from_bytes_le(x.to_repr().as_ref())
}

/// `sum_c 2^shift chunk_c` over chunk cells from `first`.
fn rec<Fb: PrimeField>(first: usize, widths: &[usize], n: usize) -> Lin<Fb> {
    let mut out = Vec::new();
    let mut shift = 0;
    for (c, w) in widths.iter().enumerate() {
        out = plus(&out, &scaled(&own(first + c, n), p2::<Fb>(shift)));
        shift += w;
    }
    out
}

fn chunk_vals<Fb: PrimeField>(x: &num_bigint::BigUint, widths: &[usize]) -> Vec<Fb> {
    let mut shift = 0;
    widths
        .iter()
        .map(|w| {
            let v = big::<Fb>(&((x >> shift) & ((num_bigint::BigUint::from(1u32) << *w) - 1u32)));
            shift += w;
            v
        })
        .collect()
}

/// Range lookups of a chunk cell of width `w` (shifted when narrow).
fn range_lk<Fb: PrimeField>(
    lookups: &mut Vec<(u32, Vec<Lin<Fb>>)>,
    cell: usize,
    w: usize,
    n: usize,
) {
    lookups.push((crate::nonnative::RANGE_TABLE, vec![own(cell, n)]));
    if w < CH {
        lookups.push((
            crate::nonnative::RANGE_TABLE,
            vec![scaled(&own(cell, n), p2::<Fb>(CH - w))],
        ));
    }
}

fn bits_for(n: usize) -> usize {
    n.next_power_of_two().trailing_zeros() as usize
}

fn lin_row<Fb: PrimeField>(l: Lin<Fb>, n: usize) -> [Lin<Fb>; 3] {
    [l, constant(vec![Fb::ONE; n]), Vec::new()]
}

fn neg<Fb: PrimeField>(l: &Lin<Fb>) -> Lin<Fb> {
    scaled(l, -Fb::ONE)
}

/// A global array with no cells and one export of per-instance tuples.
fn link_entries<Fb: Scalar>(
    w: &mut Wiring<Fb>,
    gfam: usize,
    e: usize,
    name: &'static str,
    n: usize,
    tuple: &dyn Fn(usize) -> Vec<Lin<Fb>>,
) {
    let rho = bits_for(n.max(1));
    let a = w.push(Array {
        name,
        family: gfam,
        rho,
        r_count: n.max(1),
        fixed: false,
        lookups: Vec::new(),
        widths: Vec::new(),
        rows: Vec::new(),
        values: Vec::new(),
    });
    w.set_export(e, a, tuple(1usize << rho));
}

impl<C: PastaCurve> PointsBuilder<C>
where
    C::Base: Scalar + PoseidonField,
{
    fn pt_src(&self, p: PtRef) -> (usize, usize) {
        match p {
            PtRef::Input(i) => (self.e.pt, i),
            PtRef::Add(i) => (self.e.add, i),
            PtRef::Endo(i) => (self.e.endo, i),
            PtRef::Mixed(i) => (self.e.mixed, i),
        }
    }

    /// Lays out every recorded proof into `w`: global arrays in `gfam`, the
    /// two ladder families, the reads of the binding chain's shared array
    /// `shared_a` (cell 0). `range` adds the 11-bit range table (when the
    /// circuit has none yet).
    pub fn build(
        self,
        w: &mut Wiring<C::Base>,
        gfam: usize,
        shared_a: usize,
        range: bool,
    ) -> PointsIdx {
        type Fb<C> = <C as PastaCurve>::Base;
        let one = Fb::<C>::ONE;
        let shared_link = |map: Vec<Option<usize>>, c: Fb<C>| link::<Fb<C>>(shared_a, 0, map, c);
        // Header states.
        let headers = self.headers.clone();
        link_entries(
            w,
            gfam,
            self.e.header,
            "transcript headers",
            headers.len(),
            &|n| {
                (0..3)
                    .map(|k| {
                        constant(
                            (0..n)
                                .map(|r| headers.get(r).map_or(Fb::<C>::ZERO, |h| h[k]))
                                .collect(),
                        )
                    })
                    .collect()
            },
        );
        // Points: on-curve, words and pairs exported, P' read back, claims'
        // coordinates bound to their shared limbs.
        let np = self.inputs.len();
        let rho_p = bits_for(np);
        let n_p = 1usize << rho_p;
        let mut pv = vec![vec![Fb::<C>::ZERO; n_p]; 5];
        for (i, (x, y, _)) in self.inputs.iter().enumerate() {
            pv[0][i] = *x;
            pv[1][i] = *y;
            pv[2][i] = x.square();
            pv[3][i] = x.square() * x;
        }
        let o = |k: usize| own::<Fb<C>>(k, n_p);
        let mut prow = vec![
            [o(0), o(0), o(2)],
            [o(2), o(0), o(3)],
            [o(1), o(1), plus(&o(3), &constant(vec![C::b(); n_p]))],
        ];
        if self.total {
            pv.extend((0..5).map(|_| vec![Fb::<C>::ZERO; n_p]));
            for i in 0..np {
                let x = pv[0][i];
                let y = pv[1][i];
                pv[5][i] = if x == Fb::<C>::ZERO {
                    one
                } else {
                    Fb::<C>::ZERO
                };
                pv[6][i] = Option::<Fb<C>>::from(x.invert()).unwrap_or(Fb::<C>::ZERO);
                pv[7][i] = if y == Fb::<C>::ZERO {
                    one
                } else {
                    Fb::<C>::ZERO
                };
                pv[8][i] = Option::<Fb<C>>::from(y.invert()).unwrap_or(Fb::<C>::ZERO);
                pv[9][i] = pv[5][i] * pv[7][i];
            }
            prow[2][2] = plus(&prow[2][2], &scaled(&o(9), -C::b()));
            for (v, z, inv) in [(0, 5, 6), (1, 7, 8)] {
                prow.push([o(v), o(z), vec![]]);
                prow.push([
                    o(v),
                    o(inv),
                    plus(&constant(vec![one; n_p]), &scaled(&o(z), -one)),
                ]);
                prow.push([o(inv), o(z), vec![]]);
                prow.push([o(z), o(z), o(z)]);
            }
            prow.push([o(5), o(7), o(9)]);
            let mut finite_mask = vec![Fb::<C>::ZERO; n_p];
            for i in &self.finite {
                finite_mask[*i] = one;
            }
            prow.push([o(9), constant(finite_mask), vec![]]);
        }
        // Constant inputs (fixed rows' commitments): x and y equal the key's;
        // in a slot that takes any kind, key0 + c0 (key1 - key0) + c1 (key2 -
        // key0) with the slot's kind bits c0, c1 (shared values).
        if !self.consts.is_empty() {
            for cell in [0usize, 1] {
                let pick = |p: (Fb<C>, Fb<C>)| if cell == 0 { p.0 } else { p.1 };
                let mut mask = vec![Fb::<C>::ZERO; n_p];
                let mut val = vec![Fb::<C>::ZERO; n_p];
                let mut kind_map = [vec![None; n_p], vec![None; n_p]];
                let mut kind_coef = [vec![Fb::<C>::ZERO; n_p], vec![Fb::<C>::ZERO; n_p]];
                let mut product_coef = vec![Fb::<C>::ZERO; n_p];
                for (i, alt) in &self.consts {
                    let i = *i;
                    mask[i] = one;
                    match alt {
                        None => val[i] = -pick((self.inputs[i].0, self.inputs[i].1)),
                        Some((keys, at)) => {
                            val[i] = -pick(keys[0]);
                            for b in 0..2 {
                                kind_map[b][i] = Some(*at + b);
                                kind_coef[b][i] =
                                    pick(keys[0]) - pick(*keys.get(b + 1).unwrap_or(&keys[0]));
                            }
                            if keys.len() == 4 {
                                product_coef[i] = -(pick(keys[3]) - pick(keys[1]) - pick(keys[2])
                                    + pick(keys[0]));
                            }
                        }
                    }
                }
                let mut row = plus(
                    &vec![Term {
                        src: Src::Own(cell),
                        coef: mask,
                    }],
                    &constant(val),
                );
                row = plus(
                    &row,
                    &vec![Term {
                        src: Src::Own(4),
                        coef: product_coef,
                    }],
                );
                for (map, coef) in kind_map.into_iter().zip(kind_coef) {
                    if map.iter().any(Option::is_some) {
                        row = plus(
                            &row,
                            &vec![Term {
                                src: Src::Link {
                                    array: shared_a,
                                    cell: 0,
                                    map,
                                },
                                coef,
                            }],
                        );
                    }
                }
                prow.push(lin_row(row, n_p));
            }
        }
        // The fourth kind uses the bilinear basis c0*c1. Three-kind slots
        // still reject 11; four-kind slots authenticate the fourth key exactly.
        let mut kind_maps = [vec![None; n_p], vec![None; n_p]];
        let mut forbidden = vec![Fb::<C>::ZERO; n_p];
        let mut two_kind = vec![Fb::<C>::ZERO; n_p];
        for (i, alt) in &self.consts {
            if let Some((keys, at)) = alt {
                kind_maps[0][*i] = Some(*at);
                kind_maps[1][*i] = Some(*at + 1);
                pv[4][*i] =
                    w.arrays[shared_a].values[0][*at] * w.arrays[shared_a].values[0][*at + 1];
                if keys.len() < 4 {
                    forbidden[*i] = one;
                }
                if keys.len() == 2 {
                    two_kind[*i] = one;
                }
            }
        }
        let c0 = shared_link(kind_maps[0].clone(), one);
        let c1 = shared_link(kind_maps[1].clone(), one);
        prow.push([c0.clone(), c1.clone(), o(4)]);
        prow.push([o(4), constant(forbidden), vec![]]);
        prow.push([c1.clone(), constant(two_kind), vec![]]);
        // These selectors are authenticated shared values, and are locally
        // constrained to the same Boolean domain as the scalar-side selector.
        prow.push([c0.clone(), c0.clone(), c0]);
        prow.push([c1.clone(), c1.clone(), c1]);
        if !self.claim_limbs.is_empty() {
            for (k, cell) in [(0usize, 0usize), (2, 1)] {
                let mut mask = vec![Fb::<C>::ZERO; n_p];
                let (mut lo, mut hi) = (vec![None; n_p], vec![None; n_p]);
                for (i, l) in &self.claim_limbs {
                    mask[*i] = one;
                    lo[*i] = Some(l[k]);
                    hi[*i] = Some(l[k + 1]);
                }
                let row = plus(
                    &plus(
                        &vec![Term {
                            src: Src::Own(cell),
                            coef: mask,
                        }],
                        &shared_link(lo, -one),
                    ),
                    &shared_link(hi, -p2::<Fb<C>>(128)),
                );
                prow.push(lin_row(row, n_p));
            }
        }
        let points = w.push(Array {
            name: "points",
            family: gfam,
            rho: rho_p,
            r_count: np,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; pv.len()],
            rows: prow,
            values: pv,
        });
        w.set_export(self.e.x, points, vec![o(0)]);
        w.set_export(self.e.y, points, vec![o(1)]);
        w.set_export(self.e.pt, points, vec![o(0), o(1)]);
        let back: Vec<Option<(usize, usize)>> = (0..n_p)
            .map(|i| self.inputs.get(i).and_then(|p| p.2).map(|r| self.pt_src(r)))
            .collect();
        w.import(points, vec![o(0), o(1)], back);
        self.build_claim_canonical(w, gfam, shared_a);
        // Additions.
        let na = self.adds.len();
        let rho_a = bits_for(na);
        let n_a = 1usize << rho_a;
        let mut av = vec![vec![Fb::<C>::ZERO; n_a]; 10];
        let mut sign: Vec<Fb<C>> = (0..n_a)
            .map(|i| if i < na { one } else { Fb::<C>::ZERO })
            .collect();
        for (i, s) in self.adds.iter().enumerate() {
            let qy = if s.neg { -s.qv.1 } else { s.qv.1 };
            let [l, sq, pq, x3, y3] = if self.total {
                [Fb::<C>::ZERO; 5]
            } else {
                ladders::n_add(s.pv, (s.qv.0, qy))
            };
            for (k, v) in [s.pv.0, s.pv.1, s.qv.0, s.qv.1, qy, l, sq, pq, x3, y3]
                .into_iter()
                .enumerate()
            {
                av[k][i] = v;
            }
            if s.neg {
                sign[i] = -one;
            }
        }
        let sign_a = w.push(Array {
            name: "add sign",
            family: gfam,
            rho: rho_a,
            r_count: na,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![255],
            rows: Vec::new(),
            values: vec![sign],
        });
        let adds_a = w.arrays.len();
        let a = |k: usize| own::<Fb<C>>(k, n_a);
        let ident: Vec<Option<usize>> = (0..n_a).map(Some).collect();
        let (px, py, qx, qyr, qy, l, sq, pq, x3, y3) =
            (a(0), a(1), a(2), a(3), a(4), a(5), a(6), a(7), a(8), a(9));
        let s_pq = plus(&px, &qx);
        let mut arows = vec![
            [link(sign_a, 0, ident.clone(), one), qyr.clone(), qy.clone()],
            [l.clone(), plus(&qx, &neg(&px)), plus(&qy, &neg(&py))],
            [s_pq.clone(), s_pq, sq.clone()],
            [px.clone(), qx.clone(), pq.clone()],
            [l.clone(), plus(&py, &qy), plus(&sq, &neg(&pq))],
            [l.clone(), l.clone(), plus(&plus(&x3, &px), &qx)],
            [l.clone(), plus(&px, &neg(&x3)), plus(&y3, &py)],
        ];
        if self.total {
            let completed = complete::additions(
                gfam,
                rho_a,
                &self.adds.iter().map(|a| a.pv).collect::<Vec<_>>(),
                &self.adds.iter().map(|a| a.qv).collect::<Vec<_>>(),
                &self.adds.iter().map(|a| a.neg).collect::<Vec<_>>(),
            );
            av = completed.values;
            arows = completed.rows;
        }
        if !self.equal.is_empty() {
            for cell in [8usize, 9] {
                let mut mask = vec![Fb::<C>::ZERO; n_a];
                let mut map = vec![None; n_a];
                for (lh, rh) in &self.equal {
                    mask[*lh] = one;
                    map[*lh] = Some(*rh);
                }
                arows.push(lin_row(
                    plus(
                        &vec![Term {
                            src: Src::Own(cell),
                            coef: mask,
                        }],
                        &link(adds_a, cell, map, -one),
                    ),
                    n_a,
                ));
            }
        }
        let adds = w.push(Array {
            name: "additions",
            family: gfam,
            rho: rho_a,
            r_count: na,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; av.len()],
            rows: arows,
            values: av,
        });
        assert_eq!(adds, adds_a);
        w.set_export(self.e.add, adds, vec![x3.clone(), y3.clone()]);
        let pfrom: Vec<Option<(usize, usize)>> = (0..n_a)
            .map(|i| self.adds.get(i).map(|s| self.pt_src(s.p)))
            .collect();
        let qfrom: Vec<Option<(usize, usize)>> = (0..n_a)
            .map(|i| self.adds.get(i).map(|s| self.pt_src(s.q)))
            .collect();
        w.import(adds, vec![px, py], pfrom);
        w.import(adds, vec![qx, qyr], qfrom);
        // Squeezes.
        let (mut vsq, mut fsq) = (Vec::new(), Vec::new());
        let mut inst = Vec::with_capacity(self.squeezes.len());
        for (s, sq) in self.squeezes.iter().enumerate() {
            if sq.full {
                inst.push(fsq.len());
                fsq.push(s);
            } else {
                inst.push(vsq.len());
                vsq.push(s);
            }
        }
        let pm1 = modulus::<Fb<C>>() - 1u32;
        let squeeze = self.build_squeeze(w, gfam, &vsq, false, &pm1);
        let squeeze_full = self.build_squeeze(w, gfam, &fsq, true, &pm1);
        let ch_src = |c: ChRef| -> (usize, usize) {
            match c {
                ChRef::Half(s, second) => (if second { self.e.c2 } else { self.e.c1 }, inst[s]),
                ChRef::Full(s, high) => (if high { self.e.hi } else { self.e.lo }, inst[s]),
            }
        };
        // The challenges' shared values equal their squeezes.
        let ns = w.slots(shared_a);
        let mut vfrom = vec![None; ns];
        for (v, c) in &self.v_from {
            vfrom[*v] = Some(ch_src(*c));
        }
        w.import(
            shared_a,
            vec![own(0, 1usize << w.arrays[shared_a].rho)],
            vfrom,
        );
        // Endoscalings.
        let ne = self.endo.len();
        let pi_e = bits_for(ne);
        let fam_e = w.family(pi_e, ne);
        let first_e = w.arrays.len();
        let inputs: Vec<(C, Vec<bool>)> = self
            .endo
            .iter()
            .map(|e| (e.point, e.bits.clone()))
            .collect();
        let lad = if self.total {
            complete::endoscale::<C>(first_e, fam_e, pi_e, &inputs)
        } else {
            ladders::endoscale::<C>(first_e, fam_e, pi_e, &inputs)
        };
        w.arrays.extend(lad.arrays);
        let (eb, es) = (first_e, first_e + 2);
        let last = self.endo[0].bits.len() / 2 - 1;
        let at = |a: usize, k: usize, r: usize| link::<Fb<C>>(a, k, vec![Some(r)], one);
        let slots_e = 1usize << pi_e;
        w.import(
            eb,
            vec![own(0, 1), own(1, 1)],
            (0..slots_e)
                .map(|p| self.endo.get(p).map(|e| self.pt_src(e.base)))
                .collect(),
        );
        w.import(
            eb,
            vec![at(es, 9, last)],
            (0..slots_e)
                .map(|p| self.endo.get(p).map(|e| ch_src(e.ch)))
                .collect(),
        );
        w.set_export(self.e.endo, eb, vec![at(es, 7, last), at(es, 8, last)]);
        // Variable- and fixed-base multiplications.
        let nm = self.mixed.len();
        let pi_m = bits_for(nm);
        let fam_m = w.family(pi_m, nm);
        let first_m = w.arrays.len();
        let inputs: Vec<(C, Vec<bool>, bool)> = self
            .mixed
            .iter()
            .map(|m| (m.point, m.bits.clone(), m.fixed))
            .collect();
        let lad = if self.total {
            complete::mixed_mul::<C>(first_m, fam_m, pi_m, &inputs)
        } else {
            ladders::mixed_mul::<C>(first_m, fam_m, pi_m, &inputs)
        };
        let results = lad.results.clone();
        w.arrays.extend(lad.arrays);
        let (mb, mstep, mtail, msel, mend) =
            (first_m, first_m + 2, first_m + 3, first_m + 5, first_m + 8);
        {
            let end = &mut w.arrays[mend];
            for (k, cell) in [(0usize, 9usize), (1, 10)] {
                let mut col = vec![Fb::<C>::ZERO; 1usize << pi_m];
                for (p, r) in results.iter().enumerate() {
                    col[p] = if k == 0 { r.0 } else { r.1 };
                }
                if self.total {
                    end.values[5 + k] = col;
                } else {
                    end.values.push(col);
                    end.widths.push(255);
                }
                let res = own::<Fb<C>>(5 + k, 1);
                let tail = at(mtail, cell, 2);
                end.rows.push([
                    at(msel, 0, 0),
                    plus(&own(3 + k, 1), &neg(&tail)),
                    plus(&res, &neg(&tail)),
                ]);
            }
        }
        w.set_export(self.e.mixed, mend, vec![own(5, 1), own(6, 1)]);
        let slots_m = 1usize << pi_m;
        w.import(
            mb,
            vec![own(0, 1), own(1, 1)],
            (0..slots_m)
                .map(|p| {
                    self.mixed
                        .get(p)
                        .and_then(|m| m.base)
                        .map(|b| self.pt_src(b))
                })
                .collect(),
        );
        let hi = at(mstep, 7, 126);
        let lo = plus(&at(mtail, 11, 2), &scaled(&hi, -p2::<Fb<C>>(128)));
        w.import(
            mb,
            vec![lo, hi],
            (0..slots_m)
                .map(|p| self.mixed.get(p).map(|_| (self.e.pairs, p)))
                .collect(),
        );
        let mixed = &self.mixed;
        link_entries(w, gfam, self.e.pairs, "limb pairs", nm, &|n| {
            vec![
                shared_link(
                    (0..n).map(|r| mixed.get(r).map(|m| m.limbs[0])).collect(),
                    one,
                ),
                shared_link(
                    (0..n).map(|r| mixed.get(r).map(|m| m.limbs[1])).collect(),
                    one,
                ),
            ]
        });
        let embeds = &self.embeds;
        link_entries(
            w,
            gfam,
            self.e.embed,
            "embedded words",
            embeds.len(),
            &|n| {
                vec![plus(
                    &shared_link((0..n).map(|r| embeds.get(r).copied()).collect(), one),
                    &shared_link(
                        (0..n).map(|r| embeds.get(r).map(|l| l + 1)).collect(),
                        p2::<Fb<C>>(128),
                    ),
                )]
            },
        );
        if range {
            let t = w.push(Array {
                name: "range table",
                family: gfam,
                rho: CH,
                r_count: 1 << CH,
                fixed: true,
                lookups: Vec::new(),
                widths: vec![CH as u16],
                rows: Vec::new(),
                values: vec![(0..1u64 << CH).map(Fb::<C>::from).collect()],
            });
            w.tables.push((t, crate::nonnative::RANGE_TABLE));
        }
        let validity = if self.soft_equal.is_empty() {
            None
        } else {
            let count = self.soft_equal.len();
            let rho = bits_for(count);
            let n = 1usize << rho;
            let mut values = vec![vec![Fb::<C>::ZERO; n]; 6];
            let left: Vec<_> = (0..n)
                .map(|r| self.soft_equal.get(r).map(|(l, _, _)| *l))
                .collect();
            let right: Vec<_> = (0..n)
                .map(|r| self.soft_equal.get(r).map(|(_, r, _)| *r))
                .collect();
            for (i, (l, r, ai)) in self.soft_equal.iter().enumerate() {
                for (cell, z, inv) in [(8, 1, 2), (9, 3, 4)] {
                    let d = w.arrays[adds].values[cell][*l] - w.arrays[adds].values[cell][*r];
                    values[z][i] = if d == Fb::<C>::ZERO {
                        one
                    } else {
                        Fb::<C>::ZERO
                    };
                    values[inv][i] = Option::<Fb<C>>::from(d.invert()).unwrap_or(Fb::<C>::ZERO);
                }
                values[5][i] = values[1][i] * values[3][i];
                values[0][i] = values[5][i]
                    * (one - ai.map_or(Fb::<C>::ZERO, |a| w.arrays[points].values[9][a]));
            }
            let mut rows = vec![];
            for (cell, z, inv) in [(8, 1, 2), (9, 3, 4)] {
                let d = plus(
                    &link(adds, cell, left.clone(), one),
                    &link(adds, cell, right.clone(), -one),
                );
                rows.push([d.clone(), own(z, n), vec![]]);
                rows.push([
                    d,
                    own(inv, n),
                    plus(&constant(vec![one; n]), &scaled(&own(z, n), -one)),
                ]);
                rows.push([own(z, n), own(inv, n), vec![]]);
            }
            rows.push([own(1, n), own(3, n), own(5, n)]);
            let a_map = (0..n)
                .map(|i| self.soft_equal.get(i).and_then(|(_, _, a)| *a))
                .collect();
            rows.push([
                own(5, n),
                plus(&constant(vec![one; n]), &link(points, 9, a_map, -one)),
                own(0, n),
            ]);
            let array = w.push(Array {
                name: "total verifier Schnorr iff verdicts",
                family: gfam,
                rho,
                r_count: count,
                fixed: false,
                lookups: vec![],
                widths: vec![1, 1, 255, 1, 255, 1],
                rows,
                values,
            });
            w.set_export(self.e.valid, array, vec![own(0, n)]);
            Some(array)
        };
        PointsIdx {
            points,
            adds,
            endo_base: eb,
            endo_step: es,
            mixed_base: mb,
            mixed_end: mend,
            squeeze,
            squeeze_full,
            validity,
        }
    }

    /// Canonical coordinates are essential for Receive's iff comparison of
    /// original/corrected inherited claims. A field join alone admits x+p and
    /// could incorrectly turn a valid incoming claim into a failed comparison.
    fn build_claim_canonical(&self, w: &mut Wiring<C::Base>, gfam: usize, shared: usize) {
        type Fb<C> = <C as PastaCurve>::Base;
        if self.claim_limbs.is_empty() {
            return;
        }
        let count = 2 * self.claim_limbs.len();
        let rho = bits_for(count);
        let n = 1usize << rho;
        let widths = [128usize, 127, 128, 127];
        let chunks = widths.map(chunk_ws);
        let mut starts = [0usize; 4];
        let mut cells = 5;
        for j in 0..4 {
            starts[j] = cells;
            cells += chunks[j].len();
        }
        let mut values = vec![vec![Fb::<C>::ZERO; n]; cells];
        let pm1 = modulus::<Fb<C>>() - 1u32;
        let low_mask = (num_bigint::BigUint::from(1u32) << 128) - 1u32;
        let pm_low = &pm1 & &low_mask;
        let pm_high = &pm1 >> 128;
        let mut lo_map = vec![None; n];
        let mut hi_map = vec![None; n];
        for (j, (point, limbs)) in self.claim_limbs.iter().enumerate() {
            for coordinate in 0..2 {
                let i = 2 * j + coordinate;
                let value = if coordinate == 0 {
                    self.inputs[*point].0
                } else {
                    self.inputs[*point].1
                };
                let x = to_big(value);
                let d = &pm1 - &x;
                let parts = [&x & &low_mask, &x >> 128, &d & &low_mask, &d >> 128];
                for k in 0..4 {
                    values[k][i] = big(&parts[k]);
                    for (c, v) in chunk_vals::<Fb<C>>(&parts[k], &chunks[k])
                        .into_iter()
                        .enumerate()
                    {
                        values[starts[k] + c][i] = v;
                    }
                }
                values[4][i] = big(&((&parts[0] + &parts[2]) >> 128));
                lo_map[i] = Some(limbs[2 * coordinate]);
                hi_map[i] = Some(limbs[2 * coordinate + 1]);
            }
        }
        let o = |k| own::<Fb<C>>(k, n);
        let mut rows = vec![
            lin_row(plus(&o(0), &link(shared, 0, lo_map, -Fb::<C>::ONE)), n),
            lin_row(plus(&o(1), &link(shared, 0, hi_map, -Fb::<C>::ONE)), n),
            lin_row(
                plus(
                    &plus(&o(0), &o(2)),
                    &plus(
                        &constant(vec![-big::<Fb<C>>(&pm_low); n]),
                        &scaled(&o(4), -p2::<Fb<C>>(128)),
                    ),
                ),
                n,
            ),
            lin_row(
                plus(
                    &plus(&o(1), &o(3)),
                    &plus(&o(4), &constant(vec![-big::<Fb<C>>(&pm_high); n])),
                ),
                n,
            ),
            [o(4), o(4), o(4)],
        ];
        let mut lookups = Vec::new();
        let mut cell_widths = vec![128, 127, 128, 127, 1];
        for k in 0..4 {
            rows.push(lin_row(
                plus(
                    &o(k),
                    &scaled(&rec(starts[k], &chunks[k], n), -Fb::<C>::ONE),
                ),
                n,
            ));
            for (j, width) in chunks[k].iter().enumerate() {
                range_lk(&mut lookups, starts[k] + j, *width, n);
                cell_widths.push(*width as u16);
            }
        }
        w.push(Array {
            name: "canonical claim coordinate limbs",
            family: gfam,
            rho,
            r_count: count,
            fixed: false,
            lookups,
            widths: cell_widths,
            rows,
            values,
        });
    }

    /// One squeeze array: `s0 = c1 + 2^126 c2 + 2^252 c3` (or `lo + 2^128 hi`
    /// when `full`), range-checked parts, and `s0 + d = p - 1` over the same
    /// limbs with boolean carries (canonical s0). s0 is read from the pool.
    fn build_squeeze(
        &self,
        w: &mut Wiring<C::Base>,
        gfam: usize,
        which: &[usize],
        full: bool,
        pm1: &num_bigint::BigUint,
    ) -> usize {
        type Fb<C> = <C as PastaCurve>::Base;
        let cnt = which.len();
        let rho = bits_for(cnt.max(1));
        let n = 1usize << rho;
        // Limb widths: three limbs (126, 126, 3), or two (128, 127).
        let lw: Vec<usize> = if full {
            vec![128, 127]
        } else {
            vec![126, 126, 3]
        };
        let nl = lw.len();
        let shifts: Vec<usize> = lw
            .iter()
            .scan(0, |acc, w| {
                let s = *acc;
                *acc += w;
                Some(s)
            })
            .collect();
        let cws: Vec<Vec<usize>> = lw.iter().map(|w| chunk_ws(*w)).collect();
        // Cells: s0, j1, j2, the limbs c_k, the limbs d_k, carries e_k (nl - 1),
        // then the chunks of every c_k and d_k.
        let (s0c, cbase, dbase, ebase) = (0usize, 3usize, 3 + nl, 3 + 2 * nl);
        // Chunk cells for multi-chunk limbs (a single-chunk limb checks itself).
        let chunk0 = ebase + nl - 1;
        let mut chunk_at = Vec::new();
        let mut next = chunk0;
        for _ in 0..2 {
            for cw in &cws {
                chunk_at.push(next);
                if cw.len() > 1 {
                    next += cw.len();
                }
            }
        }
        let cells = next;
        let mut v = vec![vec![Fb::<C>::ZERO; n]; cells];
        let mask = |k: usize| (num_bigint::BigUint::from(1u32) << lw[k]) - 1u32;
        let pm1_l: Vec<num_bigint::BigUint> =
            (0..nl).map(|k| (pm1 >> shifts[k]) & mask(k)).collect();
        // An empty array keeps one unread instance: s0 = 0, d = p - 1.
        let dummy = [SqueezeSpec {
            perm: 0,
            full,
            out: [Fb::<C>::ZERO; 3],
        }];
        let specs: Vec<&SqueezeSpec<Fb<C>>> = if which.is_empty() {
            dummy.iter().collect()
        } else {
            which.iter().map(|s| &self.squeezes[*s]).collect()
        };
        for (i, sq) in specs.into_iter().enumerate() {
            let s0 = to_big(sq.out[0]);
            let d = pm1 - &s0;
            let cl: Vec<num_bigint::BigUint> =
                (0..nl).map(|k| (&s0 >> shifts[k]) & mask(k)).collect();
            let dl: Vec<num_bigint::BigUint> =
                (0..nl).map(|k| (&d >> shifts[k]) & mask(k)).collect();
            v[s0c][i] = sq.out[0];
            v[1][i] = sq.out[1];
            v[2][i] = sq.out[2];
            let mut carry = num_bigint::BigUint::from(0u32);
            for k in 0..nl {
                v[cbase + k][i] = big(&cl[k]);
                v[dbase + k][i] = big(&dl[k]);
                let tot = &cl[k] + &dl[k] + &carry;
                let e = &tot >> lw[k];
                assert_eq!(&tot - (&e << lw[k]), pm1_l[k], "canonicity limb");
                if k + 1 < nl {
                    v[ebase + k][i] = big(&e);
                }
                carry = e;
                if cws[k].len() > 1 {
                    for (c, val) in chunk_vals::<Fb<C>>(&cl[k], &cws[k]).into_iter().enumerate() {
                        v[chunk_at[k] + c][i] = val;
                    }
                    for (c, val) in chunk_vals::<Fb<C>>(&dl[k], &cws[k]).into_iter().enumerate() {
                        v[chunk_at[nl + k] + c][i] = val;
                    }
                }
            }
        }
        // Rows.
        let o = |k: usize| own::<Fb<C>>(k, n);
        let mut rows = Vec::new();
        let mut srow = o(s0c);
        for k in 0..nl {
            srow = plus(&srow, &scaled(&o(cbase + k), -p2::<Fb<C>>(shifts[k])));
        }
        rows.push(lin_row(srow, n));
        for k in 0..nl {
            for (base, ci) in [(cbase, k), (dbase, nl + k)] {
                if cws[k].len() > 1 {
                    rows.push(lin_row(
                        plus(&o(base + k), &neg(&rec(chunk_at[ci], &cws[k], n))),
                        n,
                    ));
                }
            }
        }
        for k in 0..nl {
            let mut r = plus(&o(cbase + k), &o(dbase + k));
            r = plus(&r, &constant(vec![-big::<Fb<C>>(&pm1_l[k]); n]));
            if k > 0 {
                r = plus(&r, &o(ebase + k - 1));
            }
            if k + 1 < nl {
                r = plus(&r, &scaled(&o(ebase + k), -p2::<Fb<C>>(lw[k])));
            }
            rows.push(lin_row(r, n));
        }
        for k in 0..nl - 1 {
            rows.push([o(ebase + k), o(ebase + k), o(ebase + k)]);
        }
        let mut lookups = Vec::new();
        for k in 0..nl {
            for (ci, base) in [(k, cbase), (nl + k, dbase)] {
                if cws[k].len() > 1 {
                    for (c, wd) in cws[k].iter().enumerate() {
                        range_lk(&mut lookups, chunk_at[ci] + c, *wd, n);
                    }
                } else {
                    range_lk(&mut lookups, base + k, cws[k][0], n);
                }
            }
        }
        let mut widths = vec![255u16; 3];
        widths.extend(lw.iter().map(|x| *x as u16));
        widths.extend(lw.iter().map(|x| *x as u16));
        widths.extend(vec![1u16; nl - 1]);
        for _ in 0..2 {
            for cw in cws.iter().filter(|cw| cw.len() > 1) {
                widths.extend(cw.iter().map(|x| *x as u16));
            }
        }
        assert_eq!(widths.len(), cells, "a width per cell");
        let name = if full { "squeeze full" } else { "squeeze" };
        let a = w.push(Array {
            name,
            family: gfam,
            rho,
            r_count: cnt.max(1),
            fixed: false,
            lookups,
            widths,
            rows,
            values: v,
        });
        let from: Vec<Option<(usize, usize)>> = (0..n)
            .map(|i| which.get(i).map(|s| (self.e_out, self.squeezes[*s].perm)))
            .collect();
        w.import(a, vec![o(s0c), o(1), o(2)], from);
        if full {
            w.set_export(self.e.lo, a, vec![o(cbase)]);
            w.set_export(self.e.hi, a, vec![o(cbase + 1)]);
        } else {
            w.set_export(self.e.c1, a, vec![o(cbase)]);
            w.set_export(self.e.c2, a, vec![o(cbase + 1)]);
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::full::{NODE_DOMAIN, constant_entries};
    use crate::r1cs::{Circuit, Instance, Witness};
    use crate::uniform::UniformCircuit;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta, Fp};

    fn accepted(
        c: &UniformCircuit<Fp>,
        inst: &Instance<Fp>,
        gens: &Gens<Vesta>,
        bad: Vec<Fp>,
    ) -> bool {
        let mut prng = SpikeRng::new(b"points tamper");
        let (proof, _) =
            crate::spartan::prove_circuit(&c.shape, gens, inst, &Witness { w: bad }, &mut prng, c);
        crate::spartan::verify_circuit(&c.shape, gens, inst, &proof.to_bytes(), c).is_ok()
    }

    /// One Pallas proof's points side in an Fp circuit, with the binding chain
    /// over its shared values.
    #[test]
    fn points_side_holds_proves_and_refuses() {
        let shape = Shape {
            label: "points-test",
            sx: 13,
            row_bits: 2,
            col_bits: 11,
            k: 3,
        };
        let gens = Gens::<Pallas>::new(shape.cols());
        let io_len = crate::census::STATEMENT_IO + 2 * (2 + shape.col_bits) - 6;
        let (inst, wit, claims) =
            crate::verifier::instance::<Pallas>(shape, &gens, 2, io_len, b"points test");
        let proof = crate::spartan::prove_with_claims(
            &shape,
            &gens,
            &inst,
            &wit,
            &mut SpikeRng::new(b"points test prove"),
            &claims,
        );
        let tr = crate::verifier::trace(
            &shape,
            &gens,
            &inst,
            &proof,
            &claims,
            &crate::spartan::Link::None,
        );
        let values = crate::verifier::shared_values(&tr, &proof, &claims);
        let mut w = Wiring::<Fp>::default();
        let gfam = w.family(0, 1);
        let e_out = w.reserve();
        let e_v = w.reserve();
        let e_zero = constant_entries(&mut w, gfam, "zero word", &[vec![Fp::ZERO]]);
        let mut pool = Pool::new(Fp::from(NODE_DOMAIN));
        let mut pb = PointsBuilder::<Pallas>::new(&mut w, e_out, e_v, e_zero);
        pb.verify_keyed(
            &mut pool,
            &shape,
            &gens,
            &proof,
            &claims,
            &tr,
            SharedMap {
                base: 0,
                other: None,
                r: None,
            },
            None,
            None,
        );
        let (rho, mu, _) = crate::nonnative::sample_binding(0);
        let first = w.arrays.len();
        let b = crate::nonnative::binding_chain(first, gfam, gfam, &rho, &mu, &values);
        w.arrays.extend(b.arrays);
        w.tables.extend(b.tables);
        let shared_a = first + crate::nonnative::B_SHARED;
        let n_sh = 1usize << w.arrays[shared_a].rho;
        w.set_export(e_v, shared_a, vec![own(0, n_sh)]);
        let perms = pb.perms;
        let mut rng = SpikeRng::new(b"points pool");
        let pidx = pool.build(&mut w, e_out, &mut rng);
        let idx = pb.build(&mut w, gfam, shared_a, false);
        let (arrays, pi, counts, tables, copies) = w.finish();
        let (c, uinst, uwit) = UniformCircuit::build_with_copies(
            "points-test",
            arrays,
            pi,
            counts,
            tables,
            copies,
            13,
            4,
            b"points test",
        );
        let us = c.shape;
        assert_eq!(
            pidx.perms, perms,
            "every transcript permutation in the pool"
        );
        let mut prng = SpikeRng::new(b"points ch");
        let ch: Vec<Fp> = vec![prng.scalar(), prng.scalar()];
        let mut w2 = uwit.w.clone();
        let rel = c.instantiate(&us, &ch, Some(&mut w2));
        let z = crate::r1cs::z_vector(&us, &uinst, &Witness { w: w2 });
        let (az, bz, cz) = rel.abc(&us, &z);
        let bad: Vec<usize> = (0..1 << us.sx)
            .filter(|x| az[*x] * bz[*x] != cz[*x])
            .collect();
        let owners: Vec<String> = bad
            .iter()
            .take(5)
            .map(|x| {
                c.row_owner(*x).map_or("?".into(), |(a, j, i)| {
                    format!("{} row {j} instance {i}", c.arrays[a].name)
                })
            })
            .collect();
        assert!(bad.is_empty(), "{} rows fail: {:?}", bad.len(), owners);
        let rx: Vec<Fp> = (0..us.sx).map(|_| prng.scalar()).collect();
        let ry: Vec<Fp> = (0..us.sz()).map(|_| prng.scalar()).collect();
        let (ex, ey) = (crate::grid::eq_table(&rx), crate::grid::eq_table(&ry));
        let mut evals = [Fp::ZERO; 3];
        for (mat, x, y, v) in c.entries(&ch) {
            evals[mat] += v * ex[x] * ey[y];
        }
        assert_eq!(
            rel.matrix_evals(&us, &rx, &ry),
            (evals[0], evals[1], evals[2]),
            "compiled evaluation"
        );
        let ugens = Gens::<Vesta>::new(us.cols());
        let (uproof, _) = crate::spartan::prove_circuit(&us, &ugens, &uinst, &uwit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&us, &ugens, &uinst, &uproof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let tamper = |a: usize, k: usize, i: usize| -> bool {
            let mut bad = uwit.w.clone();
            bad[c.cell_index(a, k, i)] += Fp::ONE;
            accepted(&c, &uinst, &ugens, bad)
        };
        // A proof point, an addition operand, an endoscaling base, a squeeze
        // half, the multiplications' high limb read.
        assert!(!tamper(idx.points, 0, 3), "changed proof point refused");
        assert!(!tamper(idx.adds, 2, 5), "changed addition operand refused");
        assert!(
            !tamper(idx.endo_base, 0, 4),
            "changed endoscaling base refused"
        );
        assert!(!tamper(idx.squeeze, 3, 1), "changed challenge half refused");
        assert!(
            !tamper(idx.mixed_base, 1, 0),
            "changed multiplication base refused"
        );
    }
}

#[cfg(test)]
#[path = "points_total_tests.rs"]
mod total_tests;
