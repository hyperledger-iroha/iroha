//! Stage A2, step 5: the IPA verifier's scalar side as uniform arrays.
//!
//! With k = 3 and base-4 round slots (grid.rs), every closed-form opening
//! `<s(u), b>` is one pass over the column bits plus a short product per round
//! slot and digit (spartan::b_final_slots); no vector is left. The arrays:
//! - `slot data` (one instance per round slot): the round's challenge powers
//!   and squares, tau's squaring chain across slots, each digit's Boolean
//!   weights (1 - tau, tau outer; 1, 1 inner; 1, 0 for a shorter round's
//!   missing digit, whose challenge is 0), the slot's weight factors (a chain
//!   in alpha / gamma) and the io evaluation's eq prefix;
//! - `bits` (column bit j x opening q): r_col, the running eq product, the
//!   tail sum's suffix recursion and the high bits every slot shares in s(u);
//! - `slot terms` (slot x opening): per digit the Boolean and Lagrange factors
//!   against s(u)'s two bits of that digit, s(u) over the slot's varying high
//!   bits, the weighted term and a running sum over slots;
//! - `openings` (one instance per opening): its value from the bits and the
//!   slot terms, and its weight xi^q in kappa' (0 for padding openings);
//! - `alpha powers`, the io fold (one heap-ordered array: node n folds its
//!   children 2n and 2n + 1 with its level's challenge; the io values are the
//!   leaves) and one glue instance (gamma^-1, kappa, kappa' as kappa plus the
//!   openings' weighted sum, the Schnorr scalars).
//!
//! Slots are indexed by their 64-column block's low bits x (the bits that
//! differ between slots), so s(u)'s varying high bits are selectors by single
//! index bits; instances below the first slot are idle (zero weights).
//! Openings sit in the low bits (`r = x * Q + q`), so per-slot coefficients
//! split into a slot factor and links to the slot data (`r >> qb`) or to one
//! column bit (`j * Q + q`) keep closed forms. The slot-term template has
//! constant coefficients. Inputs (challenges, proof scalars, io, inherited
//! claims' u, the matrix MLE's values) are cells; step 6 wires them.

use ff::{Field, PrimeField};
use iroha_pasta::PastaCurve;

use crate::grid::lagrange3;
use crate::ipa::{Claim, F};
use crate::r1cs::{Instance, Shape};
use crate::spartan::{self, Proof, Slot};
use crate::uniform::{Array, Lin, Src, Term, constant, plus};
use crate::verifier::{Trace, shared_scalars};

/// Array offsets within the scalar side.
pub const S_SLOT: usize = 0;
pub const S_BITS: usize = 1;
pub const S_TERMS: usize = 2;
pub const S_GLUE: usize = 3;
pub const S_OPEN: usize = 4;
pub const S_ALPHA: usize = 5;
pub const S_IO: usize = 6;

// Opening cells: H (coefficient times the head's eq part), TT (the slots' sum
// times the shared high bits), VAL, XP (the weight: 1, xi^q, 0 padding), XV.
const OH: usize = 0;
const OTT: usize = 1;
pub const OVAL: usize = 2;
const OXP: usize = 3;
const OXV: usize = 4;

// Slot data cells.
pub const T: [usize; 3] = [0, 1, 2];
const SQ: [usize; 3] = [3, 4, 5];
pub const TAU: [usize; 3] = [6, 7, 8];
const W0: [usize; 3] = [9, 10, 11];
const W1: [usize; 3] = [12, 13, 14];
pub const FB: usize = 15;
const FG: usize = 16;
const PP1: usize = 17;
const PP: usize = 18;
const PRE: usize = 19;
const SLOT_CELLS: usize = 20;

// Bit cells.
pub const U: usize = 0;
const RC: usize = 1;
const RU: usize = 2;
const FF: usize = 3;
const Y: usize = 4;
const Z: usize = 5;
pub const D: usize = 6;
const CP: usize = 7;
const BIT_CELLS: usize = 8;

// Slot term cells (s(u) over the varying high bits, the term and the sum
// follow).
const P: [usize; 3] = [0, 1, 2];
const B01: usize = 3;
const BB: usize = 4;
const LU: [usize; 3] = [5, 6, 7];
const HU: [usize; 3] = [8, 9, 10];
const G01: usize = 11;
const GG: usize = 12;
const FBB: usize = 13;
pub const FGG: usize = 14;
const SV0: usize = 15;

// Glue cells.
pub const ALPHA: usize = 0;
pub const GAMMA: usize = 1;
const GINV: usize = 2;
const RHO1: usize = 3;
const RHO2: usize = 4;
pub const R1: usize = 5;
const R2: usize = 6;
const R3: usize = 7;
pub const V0: usize = 8;
const VV: usize = 11;
const K1: usize = 12;
const CA: usize = 13;
const K2: usize = 16;
const MA: usize = 17;
pub const M1: usize = 20;
const AM: usize = 23;
pub const COEF: usize = 24;
const AMS: usize = 25;
const IOE: usize = 26;
const K3: usize = 27;
pub const KAPPA: usize = 28;
pub const XI: usize = 29;
pub const ZETA: usize = 30;
pub const KZ: usize = 31;
const BZ: usize = 32;
pub const Z1: usize = 33;
pub const ZBZ: usize = 34;
pub const CZ: usize = 35;
pub const KAPPA2: usize = 36;
/// A circuit proof's second-phase challenges (alpha, beta): inputs for its
/// deferred matrix claim (zero for a toy proof).
pub const PH0: usize = 37;
const GLUE_CELLS: usize = 39;

/// The scalar side's arrays and the glue cells' expected values.
pub struct ScalarSide<S> {
    pub arrays: Vec<Array<S>>,
    /// Openings (1 + inherited claims), padded to `q_pad`.
    pub openings: usize,
    pub q_pad: usize,
    /// Slot instances (a power of two; the slots are the last ones).
    pub slot_instances: usize,
    /// Values equal to the native verifier's: (name, array, cell, instance).
    pub expected: Vec<(&'static str, usize, usize, usize, S)>,
}

/// One digit of a slot: its round variable (index within the round), or
/// missing (a shorter round's leading digit, value 0).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Digit {
    Var(usize),
    Missing,
}

fn digit(slot: &Slot, d: usize) -> Digit {
    let lead = 3 - slot.k;
    if d >= lead {
        Digit::Var(d - lead)
    } else {
        Digit::Missing
    }
}

fn lin<S: Field>(src: Src, coef: Vec<S>) -> Lin<S> {
    vec![Term { src, coef }]
}

fn lk<S: Field>(array: usize, cell: usize, map: Vec<Option<usize>>, coef: Vec<S>) -> Lin<S> {
    lin(Src::Link { array, cell, map }, coef)
}

fn sum<S: Field>(parts: &[Lin<S>]) -> Lin<S> {
    parts.iter().fold(Vec::new(), |acc, p| plus(&acc, p))
}

fn neg_c<S: Field>(v: &[S]) -> Vec<S> {
    v.iter().map(|c| -*c).collect()
}

fn half<S: PrimeField>() -> S {
    S::from(2u64).invert().unwrap()
}

/// Builds the scalar side of one proof's verification from its native
/// trace: arrays from `first` on, all in family `fam` (global, p = 0).
#[allow(clippy::too_many_arguments)]
pub fn scalar_side<C: PastaCurve>(
    first: usize,
    fam: usize,
    shape: &Shape,
    lean: bool,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    tr: &Trace<C>,
) -> ScalarSide<F<C>> {
    scalar_side_with(
        first,
        fam,
        shape,
        lean,
        inst,
        proof,
        claims,
        tr,
        &|rx, ry| crate::r1cs::matrix_evals(shape, rx, ry),
    )
}

/// The scalar side with the verified circuit's matrix evaluation `evals` at
/// `(r_x, r_y)` (its second-phase challenges bound in): a circuit proof's
/// io is read with its late values (`tr.circuit`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn scalar_side_with<C: PastaCurve>(
    first: usize,
    fam: usize,
    shape: &Shape,
    lean: bool,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
    tr: &Trace<C>,
    evals: &dyn Fn(&[F<C>], &[F<C>]) -> (F<C>, F<C>, F<C>),
) -> ScalarSide<F<C>> {
    let io_vals: &[F<C>] = tr.circuit.as_ref().map_or(&inst.io[..], |c| &c.io_full[..]);
    let one = F::<C>::ONE;
    let zero = F::<C>::ZERO;
    let slots = spartan::slots(shape);
    assert_eq!(shape.k, 3, "three-variable rounds, 64-column slots");
    let l = shape.col_bits;
    assert!(l >= 6, "a slot's six digit bits");
    let (ns, mo) = (slots.len(), shape.outer_blocks().len());
    let openings = 1 + claims.len();
    let q_pad = openings.next_power_of_two();
    let qb = q_pad.trailing_zeros() as usize;
    // Slot s sits in 64-column block b0 + s; x = the block's low w bits.
    let high = l - 6;
    let b0 = slots[0].offset >> 6;
    assert!(
        slots
            .iter()
            .enumerate()
            .all(|(s, sl)| sl.offset == (b0 + s) << 6),
        "consecutive slots"
    );
    let w = (usize::BITS - (b0 ^ (b0 + ns - 1)).leading_zeros()) as usize;
    let n_s = 1usize << w;
    let x0 = b0 & (n_s - 1);
    assert_eq!(x0 + ns, n_s, "the slots end the tail");
    let nc = high - w;
    let slot_at = |x: usize| -> Option<usize> { (x >= x0 && x < x0 + ns).then(|| x - x0) };
    let x_of = |s: usize| x0 + s;
    let x_last = n_s - 1;
    let rl = l.next_power_of_two().trailing_zeros() as usize;
    let (n_b, n_t) = (1usize << (rl + qb), 1usize << (w + qb));
    let a = |x: usize| first + x;
    let io_len = io_vals.len();
    let io_bits = io_len.next_power_of_two().trailing_zeros() as usize;
    let sw = shape.sw();
    let active = shape.active_cols();
    let rx_all: Vec<F<C>> = tr.rx.concat();
    let ry_all: Vec<F<C>> = tr.ry.concat();
    // The slot instance and digit holding r_y variable g.
    let ry_digit = |g: usize| -> (usize, usize) {
        let s = slots
            .iter()
            .position(|sl| !sl.outer && sl.first <= g && g < sl.first + sl.k)
            .expect("inner variable");
        (x_of(s), g - slots[s].first + 3 - slots[s].k)
    };
    let round_lean = |sl: &Slot| lean && (sl.outer || sl.first > shape.row_bits);
    let value_t = |sl: &Slot, d: usize| -> F<C> {
        match digit(sl, d) {
            Digit::Var(v) => {
                if sl.outer {
                    tr.rx[sl.round][v]
                } else {
                    tr.ry[sl.round][v]
                }
            }
            Digit::Missing => zero,
        }
    };
    // s(u)'s high bits (MSB first): the first nc are the block's shared bits,
    // the last w are x's bits.
    let block_bit = |j: usize| (b0 >> (high - 1 - j)) & 1;
    let common: Vec<usize> = (0..nc).map(block_bit).collect();
    let (tm, acc) = (SV0 + w, SV0 + w + 1);
    let term_cells = SV0 + w + 2;

    let bit = |b: bool| if b { one } else { zero };

    // ---------------------------------------------------------------- slot data
    let slot_down: Vec<Option<usize>> = (0..n_s).map(|x| x.checked_sub(1)).collect();
    let to_glue = |n: usize| -> Vec<Option<usize>> { vec![Some(0); n] };
    let ps = |f: &dyn Fn(usize) -> bool| -> Vec<F<C>> {
        (0..n_s).map(|x| bit(slot_at(x).is_some_and(f))).collect()
    };
    let all_s = vec![one; n_s];
    let mut rows_s: Vec<[Lin<F<C>>; 3]> = Vec::new();
    let own_s = |k: usize, c: Vec<F<C>>| lin(Src::Own(k), c);
    for d in 0..3 {
        rows_s.push([
            own_s(T[d], all_s.clone()),
            own_s(T[d], all_s.clone()),
            own_s(SQ[d], all_s.clone()),
        ]);
    }
    // t_(d+1) = t_d^2 t_d inside a lean round.
    let chain = |d: usize, s: usize| -> bool {
        let sl = &slots[s];
        round_lean(sl)
            && matches!(digit(sl, d), Digit::Var(_))
            && matches!(digit(sl, d + 1), Digit::Var(_))
    };
    for d in 0..2 {
        let c = ps(&|s| chain(d, s));
        rows_s.push([
            own_s(SQ[d], c.clone()),
            own_s(T[d], all_s.clone()),
            own_s(T[d + 1], c),
        ]);
    }
    // A missing digit's challenge is 0 (its Lagrange factor is then 1).
    for d in 0..3 {
        let miss = ps(&|s| digit(&slots[s], d) == Digit::Missing);
        if miss.iter().any(|c| !bool::from(c.is_zero())) {
            rows_s.push([own_s(T[d], miss), constant(all_s.clone()), Vec::new()]);
        }
    }
    // tau[v] = tau[v - 1]^2 (lean): the previous digit, or the previous
    // slot's last digit.
    let same = |d: usize, s: usize| -> bool {
        let sl = &slots[s];
        lean && sl.outer
            && d >= 1
            && matches!(digit(sl, d - 1), Digit::Var(_))
            && matches!(digit(sl, d), Digit::Var(_))
    };
    let cross = |d: usize, s: usize| -> bool {
        let sl = &slots[s];
        lean && sl.outer && s >= 1 && digit(sl, d) == Digit::Var(0)
    };
    for d in 0..3 {
        let (cs, cc) = (ps(&|s| same(d, s)), ps(&|s| cross(d, s)));
        let src = if d == 0 {
            lk(a(S_SLOT), TAU[2], slot_down.clone(), cc.clone())
        } else {
            sum(&[
                own_s(TAU[d - 1], cs.clone()),
                lk(a(S_SLOT), TAU[2], slot_down.clone(), cc.clone()),
            ])
        };
        let on: Vec<F<C>> = cs.iter().zip(&cc).map(|(x, y)| *x + y).collect();
        rows_s.push([src.clone(), src, own_s(TAU[d], on)]);
    }
    // Boolean weights per digit: w0 + u_lo w1 sums the digit's values 0, 1.
    let ov = |d: usize| ps(&|s| slots[s].outer && matches!(digit(&slots[s], d), Digit::Var(_)));
    let iv = |d: usize| ps(&|s| !slots[s].outer && matches!(digit(&slots[s], d), Digit::Var(_)));
    for d in 0..3 {
        rows_s.push([
            sum(&[
                own_s(W1[d], all_s.clone()),
                own_s(TAU[d], neg_c(&ov(d))),
                lin(Src::One, neg_c(&iv(d))),
            ]),
            constant(all_s.clone()),
            Vec::new(),
        ]);
        rows_s.push([
            sum(&[
                own_s(W0[d], all_s.clone()),
                own_s(TAU[d], ov(d)),
                lin(Src::One, neg_c(&all_s)),
            ]),
            constant(all_s.clone()),
            Vec::new(),
        ]);
    }
    // Weight factors: fb_s = fb_(s-1) alpha / gamma (alpha^2 / gamma into the
    // inner rounds), fb_0 = 1 / gamma; fg_s = fb_s alpha sign_s; idle
    // instances 0.
    let first_s = ps(&|s| s == 0);
    let rest_s = ps(&|s| s >= 1);
    let at_mo = ps(&|s| s == mo);
    rows_s.push([
        sum(&[
            lk(a(S_SLOT), FB, slot_down.clone(), rest_s.clone()),
            lin(Src::One, first_s.clone()),
        ]),
        sum(&[
            lk(
                a(S_GLUE),
                RHO1,
                to_glue(n_s),
                rest_s.iter().zip(&at_mo).map(|(x, y)| *x - y).collect(),
            ),
            lk(a(S_GLUE), RHO2, to_glue(n_s), at_mo.clone()),
            lk(a(S_GLUE), GINV, to_glue(n_s), first_s.clone()),
        ]),
        own_s(FB, all_s.clone()),
    ]);
    let last_of_sumcheck = |s: usize| s + 1 == mo || s + 1 == ns;
    let sign: Vec<F<C>> = (0..n_s)
        .map(|x| slot_at(x).map_or(zero, |s| if last_of_sumcheck(s) { one } else { -one }))
        .collect();
    rows_s.push([
        own_s(FB, all_s.clone()),
        lk(a(S_GLUE), ALPHA, to_glue(n_s), sign),
        own_s(FG, all_s.clone()),
    ]);
    // io's eq prefix: prod (1 - r_y[g]) over g in [1, 1 + sw - io_bits).
    let in_pre = |s: usize, d: usize| -> bool {
        let sl = &slots[s];
        match digit(sl, d) {
            Digit::Var(v) => !sl.outer && (1..1 + sw - io_bits).contains(&(sl.first + v)),
            Digit::Missing => false,
        }
    };
    let pre_f = |d: usize| -> Lin<F<C>> {
        sum(&[
            lin(Src::One, all_s.clone()),
            own_s(T[d], neg_c(&ps(&|s| in_pre(s, d)))),
        ])
    };
    rows_s.push([pre_f(0), pre_f(1), own_s(PP1, all_s.clone())]);
    rows_s.push([
        own_s(PP1, all_s.clone()),
        pre_f(2),
        own_s(PP, all_s.clone()),
    ]);
    rows_s.push([
        sum(&[
            lk(a(S_SLOT), PRE, slot_down.clone(), rest_s.clone()),
            lin(Src::One, first_s.clone()),
        ]),
        own_s(PP, all_s.clone()),
        own_s(PRE, all_s.clone()),
    ]);
    // Values (idle instances: zero challenges and weights).
    let ginv = tr.gamma.invert().unwrap();
    let (rho1, rho2) = (tr.alpha * ginv, tr.alpha * tr.alpha * ginv);
    let mut vs = vec![vec![zero; n_s]; SLOT_CELLS];
    for x in 0..n_s {
        for d in 0..3 {
            vs[W0[d]][x] = one;
            vs[PP1][x] = one;
            vs[PP][x] = one;
        }
    }
    let (mut fb, mut pre) = (one, one);
    for (s, sl) in slots.iter().enumerate() {
        let x = x_of(s);
        for d in 0..3 {
            let t = value_t(sl, d);
            vs[T[d]][x] = t;
            vs[SQ[d]][x] = t * t;
            match (sl.outer, digit(sl, d)) {
                (true, Digit::Var(v)) => {
                    let tau = tr.tau[sl.first + v];
                    vs[TAU[d]][x] = tau;
                    vs[W0[d]][x] = one - tau;
                    vs[W1[d]][x] = tau;
                }
                (false, Digit::Var(_)) => vs[W1[d]][x] = one,
                _ => {}
            }
        }
        fb = if s == 0 {
            ginv
        } else {
            fb * if s == mo { rho2 } else { rho1 }
        };
        vs[FB][x] = fb;
        vs[FG][x] = fb * tr.alpha * if last_of_sumcheck(s) { one } else { -one };
        let fs: Vec<F<C>> = (0..3)
            .map(|d| if in_pre(s, d) { one - vs[T[d]][x] } else { one })
            .collect();
        vs[PP1][x] = fs[0] * fs[1];
        vs[PP][x] = vs[PP1][x] * fs[2];
        pre *= vs[PP][x];
        vs[PRE][x] = pre;
    }
    for (s, (nb, ng)) in spartan::slot_weights(shape, tr.alpha, tr.gamma)
        .into_iter()
        .enumerate()
    {
        assert!(
            vs[FB][x_of(s)] == nb && vs[FG][x_of(s)] == ng,
            "slot weights"
        );
    }

    // --------------------------------------------------------------------- bits
    let per = |n: usize, count: usize, f: &dyn Fn(usize) -> F<C>| -> Vec<F<C>> {
        (0..n)
            .map(|r| if r < count { f(r) } else { zero })
            .collect()
    };
    let pb = |f: &dyn Fn(usize) -> F<C>| per(n_b, l * q_pad, &|r| f(r >> qb));
    let own_b = |k: usize, c: Vec<F<C>>| lin(Src::Own(k), c);
    let up: Vec<Option<usize>> = (0..n_b)
        .map(|r| (r < l * q_pad && (r >> qb) + 1 < l).then(|| r + q_pad))
        .collect();
    let down: Vec<Option<usize>> = (0..n_b)
        .map(|r| (r < l * q_pad && r >= q_pad).then(|| r - q_pad))
        .collect();
    let ones_b = pb(&|_| one);
    let not_last = pb(&|j| bit(j + 1 < l));
    let last = pb(&|j| bit(j + 1 == l));
    let a_bit = |j: usize| (active >> (l - 1 - j)) & 1 == 1;
    let rcd: Vec<(usize, usize)> = (0..l).map(|j| ry_digit(1 + shape.row_bits + j)).collect();
    let mut rows_b: Vec<[Lin<F<C>>; 3]> = Vec::new();
    let mut rc = own_b(RC, ones_b.clone());
    for d in 0..3 {
        let map: Vec<Option<usize>> = (0..n_b)
            .map(|r| (r < l * q_pad).then(|| rcd[r >> qb].0))
            .collect();
        rc = plus(
            &rc,
            &lk(a(S_SLOT), T[d], map, neg_c(&pb(&|j| bit(rcd[j].1 == d)))),
        );
    }
    rows_b.push([rc, constant(ones_b.clone()), Vec::new()]);
    rows_b.push([
        own_b(RC, ones_b.clone()),
        own_b(U, ones_b.clone()),
        own_b(RU, ones_b.clone()),
    ]);
    let f_up = sum(&[
        lk(a(S_BITS), FF, up.clone(), not_last.clone()),
        lin(Src::One, last.clone()),
    ]);
    let d_up = sum(&[
        lk(a(S_BITS), D, up.clone(), not_last.clone()),
        lin(Src::One, last.clone()),
    ]);
    let nrc = sum(&[lin(Src::One, ones_b.clone()), own_b(RC, neg_c(&ones_b))]);
    rows_b.push([
        f_up.clone(),
        sum(&[nrc.clone(), own_b(RU, ones_b.clone())]),
        own_b(FF, ones_b.clone()),
    ]);
    let wsel = sum(&[
        lk(
            a(S_BITS),
            D,
            up.clone(),
            pb(&|j| bit(j + 1 < l && a_bit(j))),
        ),
        lk(
            a(S_BITS),
            FF,
            up.clone(),
            pb(&|j| bit(j + 1 < l && !a_bit(j))),
        ),
        lin(Src::One, last.clone()),
    ]);
    rows_b.push([own_b(RU, ones_b.clone()), wsel, own_b(Y, ones_b.clone())]);
    rows_b.push([nrc, d_up, own_b(Z, ones_b.clone())]);
    rows_b.push([
        sum(&[
            own_b(D, ones_b.clone()),
            own_b(Y, neg_c(&ones_b)),
            own_b(Z, neg_c(&pb(&|j| bit(!a_bit(j))))),
        ]),
        constant(ones_b.clone()),
        Vec::new(),
    ]);
    if nc > 0 {
        let in_c = pb(&|j| bit(j < nc));
        rows_b.push([
            sum(&[
                lk(a(S_BITS), CP, down.clone(), pb(&|j| bit(j >= 1 && j < nc))),
                lin(Src::One, pb(&|j| bit(j == 0))),
            ]),
            sum(&[
                own_b(U, pb(&|j| bit(j < nc && common[j] == 1))),
                lin(Src::One, pb(&|j| bit(j < nc && common[j] == 0))),
            ]),
            own_b(CP, in_c),
        ]);
    }
    let mut vb = vec![vec![zero; n_b]; BIT_CELLS];
    for q in 0..q_pad {
        let us: Vec<F<C>> = match q {
            0 => tr.us.clone(),
            q if q <= claims.len() => claims[q - 1].us.clone(),
            _ => vec![zero; l],
        };
        let (mut f, mut dd) = (one, one);
        for j in (0..l).rev() {
            let r = j * q_pad + q;
            let rcv = vs[T[rcd[j].1]][rcd[j].0];
            assert!(rcv == tr.r_col[j], "r_col from the slot data");
            let ru = rcv * us[j];
            vb[U][r] = us[j];
            vb[RC][r] = rcv;
            vb[RU][r] = ru;
            let y = ru * if a_bit(j) { dd } else { f };
            let z = (one - rcv) * dd;
            vb[Y][r] = y;
            vb[Z][r] = z;
            dd = if a_bit(j) { y } else { y + z };
            f *= one - rcv + ru;
            vb[FF][r] = f;
            vb[D][r] = dd;
        }
        let mut cp = one;
        for (j, c) in common.iter().enumerate() {
            if *c == 1 {
                cp *= us[j];
            }
            vb[CP][j * q_pad + q] = cp;
        }
    }

    // --------------------------------------------------------------- slot terms
    let all_t = vec![one; n_t];
    let own_t = |k: usize, c: Vec<F<C>>| lin(Src::Own(k), c);
    let to_slot: Vec<Option<usize>> = (0..n_t).map(|r| Some(r >> qb)).collect();
    let sd = |cell: usize, c: Vec<F<C>>| lk(a(S_SLOT), cell, to_slot.clone(), c);
    let to_bit = |j: usize| -> Vec<Option<usize>> {
        (0..n_t)
            .map(|r| Some(j * q_pad + (r & (q_pad - 1))))
            .collect()
    };
    let ub = |j: usize, c: Vec<F<C>>| lk(a(S_BITS), U, to_bit(j), c);
    let (j_hi, j_lo) = (|d: usize| l - 6 + 2 * d, |d: usize| l - 5 + 2 * d);
    let h = half::<F<C>>();
    let cst = |c: F<C>| vec![c; n_t];
    let bf =
        |d: usize| -> Lin<F<C>> { sum(&[sd(W0[d], all_t.clone()), own_t(P[d], all_t.clone())]) };
    let gf = |d: usize| -> Lin<F<C>> {
        sum(&[
            lin(Src::One, all_t.clone()),
            sd(SQ[d], cst(h)),
            sd(T[d], cst(-h - one)),
            own_t(LU[d], all_t.clone()),
            own_t(HU[d], all_t.clone()),
        ])
    };
    let mut rows_t: Vec<[Lin<F<C>>; 3]> = Vec::new();
    for d in 0..3 {
        rows_t.push([
            ub(j_lo(d), all_t.clone()),
            sd(W1[d], all_t.clone()),
            own_t(P[d], all_t.clone()),
        ]);
    }
    rows_t.push([bf(0), bf(1), own_t(B01, all_t.clone())]);
    rows_t.push([own_t(B01, all_t.clone()), bf(2), own_t(BB, all_t.clone())]);
    for d in 0..3 {
        let l1 = sum(&[sd(T[d], cst(F::<C>::from(2))), sd(SQ[d], cst(-one))]);
        let l2 = sum(&[sd(SQ[d], cst(h)), sd(T[d], cst(-h))]);
        rows_t.push([ub(j_lo(d), all_t.clone()), l1, own_t(LU[d], all_t.clone())]);
        rows_t.push([ub(j_hi(d), all_t.clone()), l2, own_t(HU[d], all_t.clone())]);
    }
    rows_t.push([gf(0), gf(1), own_t(G01, all_t.clone())]);
    rows_t.push([own_t(G01, all_t.clone()), gf(2), own_t(GG, all_t.clone())]);
    rows_t.push([
        sd(FB, all_t.clone()),
        own_t(BB, all_t.clone()),
        own_t(FBB, all_t.clone()),
    ]);
    rows_t.push([
        sd(FG, all_t.clone()),
        own_t(GG, all_t.clone()),
        own_t(FGG, all_t.clone()),
    ]);
    // s(u) over x's bits (MSB first): column bit nc + t is x's bit w - 1 - t.
    let xbit = |t: usize| -> Vec<F<C>> {
        (0..n_t)
            .map(|r| bit(((r >> qb) >> (w - 1 - t)) & 1 == 1))
            .collect()
    };
    for t in 0..w {
        let prev = if t == 0 {
            lin(Src::One, all_t.clone())
        } else {
            own_t(SV0 + t - 1, all_t.clone())
        };
        let set = xbit(t);
        let sel = sum(&[
            ub(nc + t, set.clone()),
            lin(Src::One, set.iter().map(|c| one - c).collect()),
        ]);
        rows_t.push([prev, sel, own_t(SV0 + t, all_t.clone())]);
    }
    let svar = if w == 0 {
        lin(Src::One, all_t.clone())
    } else {
        own_t(SV0 + w - 1, all_t.clone())
    };
    rows_t.push([
        svar,
        sum(&[own_t(FBB, all_t.clone()), own_t(FGG, all_t.clone())]),
        own_t(tm, all_t.clone()),
    ]);
    let t_down: Vec<Option<usize>> = (0..n_t).map(|r| r.checked_sub(q_pad)).collect();
    rows_t.push([
        sum(&[
            own_t(acc, all_t.clone()),
            lk(
                a(S_TERMS),
                acc,
                t_down,
                neg_c(&(0..n_t).map(|r| bit(r >= q_pad)).collect::<Vec<_>>()),
            ),
            own_t(tm, neg_c(&all_t)),
        ]),
        constant(all_t.clone()),
        Vec::new(),
    ]);
    let mut vt = vec![vec![zero; n_t]; term_cells];
    let mut totals = vec![zero; q_pad];
    for x in 0..n_s {
        for q in 0..q_pad {
            let r = x * q_pad + q;
            let u = |j: usize| vb[U][j * q_pad + q];
            let mut bfs = [zero; 3];
            let mut gfs = [zero; 3];
            for d in 0..3 {
                let (uh, ul) = (u(j_hi(d)), u(j_lo(d)));
                let t = vs[T[d]][x];
                vt[P[d]][r] = ul * vs[W1[d]][x];
                bfs[d] = vs[W0[d]][x] + vt[P[d]][r];
                let [l0, l1, l2] = lagrange3(t);
                vt[LU[d]][r] = ul * l1;
                vt[HU[d]][r] = uh * l2;
                gfs[d] = l0 + vt[LU[d]][r] + vt[HU[d]][r];
            }
            vt[B01][r] = bfs[0] * bfs[1];
            vt[BB][r] = vt[B01][r] * bfs[2];
            vt[G01][r] = gfs[0] * gfs[1];
            vt[GG][r] = vt[G01][r] * gfs[2];
            vt[FBB][r] = vs[FB][x] * vt[BB][r];
            vt[FGG][r] = vs[FG][x] * vt[GG][r];
            let mut sv = one;
            for t in 0..w {
                if (x >> (w - 1 - t)) & 1 == 1 {
                    sv *= u(nc + t);
                }
                vt[SV0 + t][r] = sv;
            }
            vt[tm][r] = sv * (vt[FBB][r] + vt[FGG][r]);
            totals[q] += vt[tm][r];
            vt[acc][r] = totals[q];
        }
    }

    // ------------------------------------------------------------------ io fold
    // Heap order: leaves 2^L + i hold io_i (zero padding enforced); node n in
    // [1, 2^L) folds children 2n, 2n + 1 (io's bit L - 1 - level) with r_y's
    // variable for that bit; node 1 is sum_i eq(r, i) io_i. Node 0 is idle.
    let lv = io_bits;
    let n_io = 1usize << (lv + 1);
    let leaves = 1usize << lv;
    let child = |o: usize| -> Vec<Option<usize>> {
        (0..n_io).map(|n| (n < leaves).then(|| 2 * n + o)).collect()
    };
    let internal: Vec<F<C>> = (0..n_io).map(|n| bit(n < leaves)).collect();
    let mut r_side: Vec<Lin<F<C>>> = Vec::new();
    for level in 0..lv {
        let g = 1 + sw - io_bits + level;
        let (s, d) = ry_digit(g);
        let on: Vec<F<C>> = (0..n_io)
            .map(|n| bit(n >= 1 << level && n < 2 << level))
            .collect();
        r_side.push(lk(a(S_SLOT), T[d], vec![Some(s); n_io], on));
    }
    let mut rows_io = vec![[
        sum(&r_side),
        sum(&[
            lk(a(S_IO), 0, child(1), internal.clone()),
            lk(a(S_IO), 0, child(0), neg_c(&internal)),
        ]),
        sum(&[
            lin(Src::Own(0), internal.clone()),
            lk(a(S_IO), 0, child(0), neg_c(&internal)),
        ]),
    ]];
    if io_len < leaves {
        rows_io.push([
            lin(
                Src::Own(0),
                (0..n_io).map(|n| bit(n >= leaves + io_len)).collect(),
            ),
            constant(vec![one; n_io]),
            Vec::new(),
        ]);
    }
    let mut vio = vec![zero; n_io];
    vio[leaves..leaves + io_len].copy_from_slice(io_vals);
    for n in (1..leaves).rev() {
        let level = n.ilog2() as usize;
        let r = ry_all[1 + sw - io_bits + level];
        vio[n] = vio[2 * n] + r * (vio[2 * n + 1] - vio[2 * n]);
    }
    let io_root = vio[1];

    // --------------------------------------------------------------------- glue
    let mut vg = vec![zero; GLUE_CELLS];
    let g1 = |cell: usize| lin(Src::Own(cell), vec![one]);
    let gc = |cell: usize, c: F<C>| lin(Src::Own(cell), vec![c]);
    let pt1 = |arr: usize, cell: usize, at: usize, c: F<C>| lk(arr, cell, vec![Some(at)], vec![c]);
    let k1 = constant(vec![one]);
    let mut rows_g: Vec<[Lin<F<C>>; 3]> = Vec::new();
    let lin_row = |rows: &mut Vec<[Lin<F<C>>; 3]>, l: Lin<F<C>>| {
        rows.push([l, constant(vec![one]), Vec::new()])
    };
    // gamma^-1, alpha / gamma, alpha^2 / gamma.
    rows_g.push([g1(GAMMA), g1(GINV), k1.clone()]);
    rows_g.push([g1(ALPHA), g1(GINV), g1(RHO1)]);
    rows_g.push([g1(ALPHA), g1(RHO1), g1(RHO2)]);
    vg[ALPHA] = tr.alpha;
    vg[GAMMA] = tr.gamma;
    vg[GINV] = ginv;
    vg[RHO1] = rho1;
    vg[RHO2] = rho2;
    // alpha^e = the alpha powers' instance e - 1.
    let na = ns + 2;
    let ra = na.next_power_of_two().trailing_zeros() as usize;
    let n_a = 1usize << ra;
    let pa = |f: &dyn Fn(usize) -> bool| -> Vec<F<C>> {
        (0..n_a).map(|n| bit(n < na && f(n))).collect()
    };
    let a_down: Vec<Option<usize>> = (0..n_a)
        .map(|r| (r >= 1 && r < na).then(|| r - 1))
        .collect();
    let rows_a = vec![[
        sum(&[
            lk(a(S_ALPHA), 0, a_down, pa(&|n| n >= 1)),
            lin(Src::One, pa(&|n| n == 0)),
        ]),
        lk(a(S_GLUE), ALPHA, vec![Some(0); n_a], pa(&|_| true)),
        lin(Src::Own(0), pa(&|_| true)),
    ]];
    let mut va = vec![vec![zero; n_a]];
    let mut xa = one;
    for n in 0..na {
        xa *= tr.alpha;
        va[0][n] = xa;
    }
    let apow = |e: usize| pt1(a(S_ALPHA), 0, e - 1, one);
    let apv = |e: usize| va[0][e - 1];
    // r_A, r_B, r_C = r, r^2, r^3 (lean); three challenges otherwise.
    if lean {
        rows_g.push([g1(R1), g1(R1), g1(R2)]);
        rows_g.push([g1(R2), g1(R1), g1(R3)]);
    }
    vg[R1] = tr.rabc[0];
    vg[R2] = tr.rabc[1];
    vg[R3] = tr.rabc[2];
    // kappa = alpha^mo (v_A v_B - v_C) + alpha^(mo+1) sum r v + am r_sel io(r).
    for (i, v) in proof.v.iter().enumerate() {
        vg[V0 + i] = *v;
    }
    rows_g.push([g1(V0), g1(V0 + 1), g1(VV)]);
    vg[VV] = proof.v[0] * proof.v[1];
    rows_g.push([apow(mo), sum(&[g1(VV), gc(V0 + 2, -one)]), g1(K1)]);
    vg[K1] = apv(mo) * (vg[VV] - proof.v[2]);
    for i in 0..3 {
        rows_g.push([g1(R1 + i), g1(V0 + i), g1(CA + i)]);
        vg[CA + i] = vg[R1 + i] * proof.v[i];
    }
    rows_g.push([apow(mo + 1), sum(&[g1(CA), g1(CA + 1), g1(CA + 2)]), g1(K2)]);
    vg[K2] = apv(mo + 1) * (vg[CA] + vg[CA + 1] + vg[CA + 2]);
    let (ma, mb, mc) = evals(&rx_all, &ry_all);
    if let Some(ct) = &tr.circuit {
        for (k, v) in ct.phase2.iter().enumerate().take(2) {
            vg[PH0 + k] = *v;
        }
    }
    for (i, m) in [ma, mb, mc].into_iter().enumerate() {
        vg[MA + i] = m;
        rows_g.push([g1(R1 + i), g1(MA + i), g1(M1 + i)]);
        vg[M1 + i] = vg[R1 + i] * m;
    }
    rows_g.push([apow(ns + 1), sum(&[g1(M1), g1(M1 + 1), g1(M1 + 2)]), g1(AM)]);
    vg[AM] = apv(ns + 1) * (vg[M1] + vg[M1 + 1] + vg[M1 + 2]);
    let (s_sel, d_sel) = ry_digit(0);
    rows_g.push([
        g1(AM),
        sum(&[k1.clone(), pt1(a(S_SLOT), T[d_sel], s_sel, -one)]),
        g1(COEF),
    ]);
    rows_g.push([g1(AM), pt1(a(S_SLOT), T[d_sel], s_sel, one), g1(AMS)]);
    let r_sel = ry_all[0];
    vg[COEF] = vg[AM] * (one - r_sel);
    vg[AMS] = vg[AM] * r_sel;
    rows_g.push([
        pt1(a(S_SLOT), PRE, x_last, one),
        pt1(a(S_IO), 0, 1, one),
        g1(IOE),
    ]);
    vg[IOE] = vs[PRE][x_last] * io_root;
    assert!(
        vg[IOE] == crate::r1cs::io_eval(shape, io_vals, &ry_all[1..]),
        "io evaluation"
    );
    rows_g.push([g1(AMS), g1(IOE), g1(K3)]);
    vg[K3] = vg[AMS] * vg[IOE];
    lin_row(
        &mut rows_g,
        sum(&[g1(KAPPA), gc(K1, -one), gc(K2, -one), gc(K3, -one)]),
    );
    vg[KAPPA] = vg[K1] + vg[K2] + vg[K3];
    vg[XI] = tr.xi;
    // ----------------------------------------------------------------- openings
    // value_q = CP_q (sum of slot terms) - coefficient (F_0 - D_0); weight
    // xp_q: 1, xi, xi^2, ..., then 0 on padding; kappa' = kappa - value_0 +
    // sum_q xp_q value_q.
    let n_o = q_pad;
    let po = |f: &dyn Fn(usize) -> bool| -> Vec<F<C>> { (0..n_o).map(|q| bit(f(q))).collect() };
    let ones_o = po(&|_| true);
    let own_o = |k: usize, c: Vec<F<C>>| lin(Src::Own(k), c);
    let at_bits =
        |j: usize| -> Vec<Option<usize>> { (0..n_o).map(|q| Some(j * q_pad + q)).collect() };
    let at_sum: Vec<Option<usize>> = (0..n_o).map(|q| Some(x_last * q_pad + q)).collect();
    let o_down: Vec<Option<usize>> = (0..n_o).map(|q| q.checked_sub(1)).collect();
    let mut rows_o: Vec<[Lin<F<C>>; 3]> = vec![[
        lk(a(S_GLUE), COEF, vec![Some(0); n_o], ones_o.clone()),
        sum(&[
            lk(a(S_BITS), FF, at_bits(0), ones_o.clone()),
            lk(a(S_BITS), D, at_bits(0), neg_c(&ones_o)),
        ]),
        own_o(OH, ones_o.clone()),
    ]];
    let cp_side = if nc > 0 {
        lk(a(S_BITS), CP, at_bits(nc - 1), ones_o.clone())
    } else {
        lin(Src::One, ones_o.clone())
    };
    rows_o.push([
        cp_side,
        lk(a(S_TERMS), acc, at_sum, ones_o.clone()),
        own_o(OTT, ones_o.clone()),
    ]);
    rows_o.push([
        sum(&[
            own_o(OVAL, ones_o.clone()),
            own_o(OTT, neg_c(&ones_o)),
            own_o(OH, ones_o.clone()),
        ]),
        constant(ones_o.clone()),
        Vec::new(),
    ]);
    rows_o.push([
        sum(&[
            lk(a(S_OPEN), OXP, o_down, po(&|q| q >= 1)),
            lin(Src::One, po(&|q| q == 0)),
        ]),
        sum(&[
            lk(
                a(S_GLUE),
                XI,
                vec![Some(0); n_o],
                po(&|q| q >= 1 && q < openings),
            ),
            lin(Src::One, po(&|q| q == 0)),
        ]),
        own_o(OXP, ones_o.clone()),
    ]);
    rows_o.push([
        own_o(OXP, ones_o.clone()),
        own_o(OVAL, ones_o.clone()),
        own_o(OXV, ones_o.clone()),
    ]);
    let mut vo = vec![vec![zero; n_o]; 5];
    let mut xp = one;
    for q in 0..n_o {
        vo[OH][q] = vg[COEF] * (vb[FF][q] - vb[D][q]);
        vo[OTT][q] = if nc > 0 {
            vb[CP][(nc - 1) * q_pad + q]
        } else {
            one
        } * totals[q];
        vo[OVAL][q] = vo[OTT][q] - vo[OH][q];
        xp = if q == 0 {
            one
        } else if q < openings {
            xp * tr.xi
        } else {
            zero
        };
        vo[OXP][q] = xp;
        vo[OXV][q] = xp * vo[OVAL][q];
    }
    lin_row(
        &mut rows_g,
        vec![
            Term {
                src: Src::Own(KAPPA2),
                coef: vec![one],
            },
            Term {
                src: Src::Own(KAPPA),
                coef: vec![-one],
            },
            Term {
                src: Src::Link {
                    array: a(S_OPEN),
                    cell: OVAL,
                    map: vec![Some(0)],
                },
                coef: vec![one],
            },
            Term {
                src: Src::All {
                    array: a(S_OPEN),
                    cell: OXV,
                },
                coef: vec![-one],
            },
        ],
    );
    vg[KAPPA2] = vg[KAPPA] - vo[OVAL][0] + vo[OXV].iter().copied().sum::<F<C>>();
    // Schnorr scalars: kappa' zeta, z1 b_final zeta, and c with z1 = 2 c + 2^255 + 1.
    vg[ZETA] = tr.zeta;
    vg[Z1] = proof.ipa.z1;
    rows_g.push([g1(KAPPA2), g1(ZETA), g1(KZ)]);
    vg[KZ] = vg[KAPPA2] * tr.zeta;
    rows_g.push([pt1(a(S_OPEN), OVAL, 0, one), g1(ZETA), g1(BZ)]);
    vg[BZ] = vo[OVAL][0] * tr.zeta;
    rows_g.push([g1(Z1), g1(BZ), g1(ZBZ)]);
    vg[ZBZ] = proof.ipa.z1 * vg[BZ];
    let p255 = F::<C>::from(2).pow_vartime([255u64]);
    lin_row(
        &mut rows_g,
        sum(&[
            gc(CZ, F::<C>::from(2)),
            lin(Src::One, vec![p255 + one]),
            gc(Z1, -one),
        ]),
    );
    let sh = shared_scalars(tr, proof);
    vg[CZ] = sh.c_z1;

    // Expected native values.
    let mut expected = vec![
        ("b_final", S_OPEN, OVAL, 0, tr.b_final),
        ("kappa", S_GLUE, KAPPA, 0, tr.kappa),
        ("coefficient", S_GLUE, COEF, 0, tr.coefficient),
        ("kappa'", S_GLUE, KAPPA2, 0, tr.kappa_claims),
        ("kappa' zeta", S_GLUE, KZ, 0, sh.kappa_zeta),
        ("z1 b_final zeta", S_GLUE, ZBZ, 0, sh.z1_bf_zeta),
        ("c(z1)", S_GLUE, CZ, 0, sh.c_z1),
    ];
    for (j, v) in tr.claim_values.iter().enumerate() {
        expected.push(("claim value", S_OPEN, OVAL, j + 1, *v));
    }

    let arr = |name: &'static str,
               rho: usize,
               r_count: usize,
               rows: Vec<[Lin<F<C>>; 3]>,
               values: Vec<Vec<F<C>>>| Array {
        name,
        family: fam,
        rho,
        r_count,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; values.len()],
        rows,
        values,
    };
    let arrays = vec![
        arr("scalar slot data", w, n_s, rows_s, vs),
        arr("scalar bits", rl + qb, l * q_pad, rows_b, vb),
        arr("scalar slot terms", w + qb, n_t, rows_t, vt),
        arr(
            "scalar glue",
            0,
            1,
            rows_g,
            vg.into_iter().map(|v| vec![v]).collect(),
        ),
        arr("scalar openings", qb, n_o, rows_o, vo),
        arr("scalar alpha powers", ra, na, rows_a, va),
        arr("scalar io fold", lv + 1, n_io, rows_io, vec![vio]),
    ];
    for (name, ar, cell, at, v) in &expected {
        assert!(
            arrays[*ar].values[*cell][*at] == *v,
            "{name} equals the native verifier's"
        );
    }
    ScalarSide {
        arrays,
        openings,
        q_pad,
        slot_instances: n_s,
        expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::r1cs::{Circuit, Witness};
    use crate::uniform::UniformCircuit;
    use iroha_pasta::{Eq as Vesta, Fp};

    /// Proves a witness with one cell changed (array, cell, instance).
    fn refused(
        c: &UniformCircuit<Fp>,
        inst: &Instance<Fp>,
        wit: &Witness<Fp>,
        gens: &crate::ipa::Gens<Vesta>,
        a: usize,
        k: usize,
        i: usize,
    ) -> bool {
        let mut bad = wit.w.clone();
        bad[c.cell_index(a, k, i)] += Fp::ONE;
        let mut prng = SpikeRng::new(b"scalar refuse");
        let (proof, _) =
            crate::spartan::prove_circuit(&c.shape, gens, inst, &Witness { w: bad }, &mut prng, c);
        crate::spartan::verify_circuit(&c.shape, gens, inst, &proof.to_bytes(), c).is_err()
    }

    #[test]
    fn scalar_side_matches_native_proves_and_refuses() {
        let shape = Shape {
            label: "scalar-test",
            sx: 13,
            row_bits: 2,
            col_bits: 11,
            k: 3,
        };
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let io_len = crate::census::STATEMENT_IO + 2 * (2 + shape.col_bits) - 6;
        let (inst, wit, claims) =
            crate::verifier::instance::<Vesta>(shape, &gens, 2, io_len, b"scalar test");
        let proof = crate::spartan::prove_with_claims(
            &shape,
            &gens,
            &inst,
            &wit,
            &mut SpikeRng::new(b"scalar test prove"),
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
        let lean = crate::fs::lean_schedule();
        let side = scalar_side::<Vesta>(0, 0, &shape, lean, &inst, &proof, &claims, &tr);
        let n = side.arrays.len();
        let (c, uinst, uwit) = UniformCircuit::build(
            "scalar-test",
            side.arrays,
            vec![0],
            vec![1],
            Vec::new(),
            12,
            4,
            b"scalar",
        );
        let us = c.shape;
        let mut rng = SpikeRng::new(b"scalar ch");
        let ch: Vec<Fp> = Vec::new();
        let rel = c.instantiate(&us, &ch, None);
        let z = crate::r1cs::z_vector(&us, &uinst, &uwit);
        let (az, bz, cz) = rel.abc(&us, &z);
        let bad: Vec<usize> = (0..1 << us.sx)
            .filter(|x| az[*x] * bz[*x] != cz[*x])
            .collect();
        assert!(
            bad.is_empty(),
            "{} rows fail, first {:?}",
            bad.len(),
            &bad[..bad.len().min(5)]
        );
        let rx: Vec<Fp> = (0..us.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fp> = (0..us.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (crate::grid::eq_table(&rx), crate::grid::eq_table(&ry));
        let entries = c.entries(&ch);
        let mut slow = [Fp::ZERO; 3];
        for &(mat, x, y, v) in &entries {
            slow[mat] += ex[x] * ey[y] * v;
        }
        for a in 0..c.arrays.len() {
            let mut part = [Fp::ZERO; 3];
            for &(mat, x, y, v) in &c.entries_of(&ch, Some(a)) {
                part[mat] += ex[x] * ey[y] * v;
            }
            assert_eq!(
                c.evals_of(a, &rx, &ry, &ch),
                (part[0], part[1], part[2]),
                "{}: matrix evaluation matches its entries",
                c.arrays[a].name
            );
        }
        assert_eq!(
            rel.matrix_evals(&us, &rx, &ry),
            (slow[0], slow[1], slow[2]),
            "matrix evaluation matches the entries"
        );
        let rm: [Fp; 3] = [rng.scalar(), rng.scalar(), rng.scalar()];
        let mut m = vec![Fp::ZERO; 1 << us.sz()];
        for &(mat, x, y, v) in &entries {
            m[y] += ex[x] * v * rm[mat];
        }
        assert!(
            rel.m_table(&us, &ex, rm[0], rm[1], rm[2]) == m,
            "M(r_x, .) matches the entries"
        );
        let ugens = crate::ipa::Gens::<Vesta>::new(us.cols());
        let mut prng = SpikeRng::new(b"scalar prove");
        let (uproof, _) = crate::spartan::prove_circuit(&us, &ugens, &uinst, &uwit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&us, &ugens, &uinst, &uproof.to_bytes(), &c).is_ok(),
            "honest"
        );
        // A changed slot weight, tail-sum step, slot term, running sum, io
        // fold value and opening value.
        let xs = side.slot_instances;
        assert!(
            refused(&c, &uinst, &uwit, &ugens, S_SLOT, FB, xs - 2),
            "changed weight factor"
        );
        assert!(
            refused(&c, &uinst, &uwit, &ugens, S_BITS, D, 5 * side.q_pad + 1),
            "changed tail sum"
        );
        assert!(
            refused(
                &c,
                &uinst,
                &uwit,
                &ugens,
                S_TERMS,
                FGG,
                (xs - 3) * side.q_pad + 2
            ),
            "changed slot term"
        );
        assert!(
            refused(&c, &uinst, &uwit, &ugens, S_IO, 0, 5),
            "changed io fold value"
        );
        assert!(
            refused(&c, &uinst, &uwit, &ugens, S_OPEN, OVAL, 0),
            "changed b_final"
        );
        assert_eq!(n, S_IO + 1, "seven arrays");
    }
}
