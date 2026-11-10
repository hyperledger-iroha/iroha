//! Stage A2, step 4: SHA-256 as uniform arrays (the binding chain is in
//! `binding.rs`).
//!
//! One 64-byte block per signed message: the 32-byte encoding of a field
//! element, then the padding. Family `fam` holds one block per p:
//!
//! - message (8 words): each word decomposed at [3, 7, 8, 16, 18, 24], so its
//!   bytes and its sigma0 pieces are linear in its cells; a running sum gives
//!   the encoded element `m = sum_j 2^(32 j) bswap(w_j)`;
//! - schedule (48 steps): `w[t] = w[t-16] + sigma0(w[t-15]) + w[t-7] +
//!   sigma1(w[t-2])`, the result decomposed at [3, 7, 10, 17, 18, 19] for the
//!   later sigmas;
//! - rounds (64): round t decomposes its inputs a and e (the previous round's
//!   outputs) and computes Sigma0, Sigma1, Ch and Maj by spread splits. The
//!   shift register is links to rounds t-1..t-4; the IV and K_t are per-round
//!   constants;
//! - output (8 words): state + IV as 32 boolean bits and a carry.
//!
//! Spread lookups as stage A's lever 5: one fixed table of (d, spread(d)) for
//! d < 2^11 serves chunk lookups and range checks of narrower pieces (d
//! shifted to the top of the table). A sum of up to three spread words has
//! base-4 digits 0..3, so its split into even bits (XOR) and odd bits (AND,
//! majority) is unique: one linear row and three chunk lookups per half.

use ff::{Field, PrimeField};

use crate::p256;
use crate::uniform::{Array, Lin, constant, link, own, plus, scaled};

pub type Fp = iroha_pasta::Fp;

pub const SPREAD_TABLE: u32 = 2;
const SPREAD: usize = 11;

/// Array offsets from `first`.
pub const A_MSG: usize = 0;
pub const A_SCHED: usize = 1;
pub const A_ROUND: usize = 2;
pub const A_OUT: usize = 3;
pub const A_SPREAD: usize = 4;

const MSG_CUTS: [usize; 6] = [3, 7, 8, 16, 18, 24];
const SCHED_CUTS: [usize; 6] = [3, 7, 10, 17, 18, 19];
const A_CUTS: [usize; 3] = [2, 13, 22];
const E_CUTS: [usize; 3] = [6, 11, 25];

fn spread_of(x: u64, bits: usize) -> u64 {
    (0..bits).fold(0, |acc, i| acc | (((x >> i) & 1) << (2 * i)))
}

fn f(x: u64) -> Fp {
    Fp::from(x)
}

fn pow2(e: usize) -> Fp {
    Fp::from(2u64).pow_vartime([e as u64])
}

fn sigma0(x: u32) -> u32 {
    x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
}

fn sigma1(x: u32) -> u32 {
    x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
}

/// Spread sum of the rotated and shifted copies of `x`.
fn sigma_spread(x: u32, rot: &[u32], shr: Option<u32>) -> u64 {
    let mut s: u64 = rot
        .iter()
        .map(|r| spread_of(x.rotate_right(*r) as u64, 32))
        .sum();
    if let Some(r) = shr {
        s += spread_of((x >> r) as u64, 32);
    }
    s
}

/// The padding words 8..15 of a 32-byte message.
fn pad(j: usize) -> u32 {
    match j {
        8 => 0x8000_0000,
        15 => 256,
        _ => 0,
    }
}

/// Where a template reads a cell: its own instance, or another array (of the
/// family) through a map on r.
#[derive(Clone)]
enum At {
    Own,
    Link(usize, Vec<Option<usize>>),
}

impl At {
    fn cell(&self, k: usize, c: Fp, n: usize) -> Lin<Fp> {
        match self {
            At::Own => scaled(&own(k, n), c),
            At::Link(a, map) => link(*a, k, map.clone(), c),
        }
    }
}

/// A map on r of an array with `n` slots: `g(r)` where defined.
fn map_of(n: usize, g: impl Fn(usize) -> Option<usize>) -> Vec<Option<usize>> {
    (0..n).map(g).collect()
}

/// A per-r constant: `g(r)` (zero where `None`).
fn per_r(n: usize, g: impl Fn(usize) -> Option<u64>) -> Lin<Fp> {
    constant((0..n).map(|r| g(r).map_or(Fp::ZERO, f)).collect())
}

#[derive(Clone, Copy)]
struct Piece {
    off: usize,
    b: usize,
    cell: usize,
    boolean: bool,
}

/// A word's pieces at a cut set: booleans for segments of up to 3 bits, else
/// parts of up to 11 bits as (dense, spread) cell pairs.
#[derive(Clone)]
struct Pieces {
    list: Vec<Piece>,
    first: usize,
    cells: usize,
}

impl Pieces {
    fn new(cuts: &[usize], first: usize) -> Self {
        let mut bounds = vec![0];
        bounds.extend_from_slice(cuts);
        bounds.push(32);
        let mut list = Vec::new();
        let mut cell = first;
        for win in bounds.windows(2) {
            let mut start = win[0];
            while start < win[1] {
                let b = (win[1] - start).min(SPREAD);
                if b <= 3 {
                    for i in 0..b {
                        list.push(Piece {
                            off: start + i,
                            b: 1,
                            cell,
                            boolean: true,
                        });
                        cell += 1;
                    }
                } else {
                    list.push(Piece {
                        off: start,
                        b,
                        cell,
                        boolean: false,
                    });
                    cell += 2;
                }
                start += b;
            }
        }
        Pieces {
            list,
            first,
            cells: cell - first,
        }
    }

    fn spread_cell(p: &Piece) -> usize {
        if p.boolean { p.cell } else { p.cell + 1 }
    }

    fn lin(&self, at: &At, n: usize, spread: bool, coef: &dyn Fn(&Piece) -> Option<Fp>) -> Lin<Fp> {
        let mut out: Lin<Fp> = Vec::new();
        for p in &self.list {
            if let Some(c) = coef(p) {
                let k = if spread { Self::spread_cell(p) } else { p.cell };
                out = plus(&out, &at.cell(k, c, n));
            }
        }
        out
    }

    fn dense(&self, at: &At, n: usize) -> Lin<Fp> {
        self.lin(at, n, false, &|p| Some(f(1 << p.off)))
    }

    fn spread(&self, at: &At, n: usize) -> Lin<Fp> {
        self.lin(at, n, true, &|p| Some(f(1 << (2 * p.off))))
    }

    /// The spread of the word rotated right by `r` (pieces aligned with `r`).
    fn rotr(&self, at: &At, r: usize, n: usize) -> Lin<Fp> {
        self.lin(at, n, true, &|p| {
            assert!(
                p.off >= r || p.off + p.b <= r,
                "piece aligned with the rotation"
            );
            Some(f(1 << (2 * ((p.off + 32 - r) % 32))))
        })
    }

    /// The spread of the word shifted right by `r`.
    fn shr(&self, at: &At, r: usize, n: usize) -> Lin<Fp> {
        self.lin(at, n, true, &|p| {
            if p.off >= r {
                Some(f(1 << (2 * (p.off - r))))
            } else {
                assert!(p.off + p.b <= r, "piece aligned with the shift");
                None
            }
        })
    }

    /// The spread of `sum rotr(w, rot) + shr(w, shr)`.
    fn sigma(&self, at: &At, rot: &[usize], shr: Option<usize>, n: usize) -> Lin<Fp> {
        let mut out: Lin<Fp> = Vec::new();
        for &r in rot {
            out = plus(&out, &self.rotr(at, r, n));
        }
        if let Some(r) = shr {
            out = plus(&out, &self.shr(at, r, n));
        }
        out
    }

    /// Booleanity rows and spread lookups of the pieces (own cells).
    fn constraints(
        &self,
        n: usize,
        rows: &mut Vec<[Lin<Fp>; 3]>,
        lookups: &mut Vec<(u32, Vec<Lin<Fp>>)>,
    ) {
        for p in &self.list {
            if p.boolean {
                rows.push([own(p.cell, n), own(p.cell, n), own(p.cell, n)]);
            } else {
                lookups.push((SPREAD_TABLE, vec![own(p.cell, n), own(p.cell + 1, n)]));
                if p.b < SPREAD {
                    let s = SPREAD - p.b;
                    lookups.push((
                        SPREAD_TABLE,
                        vec![
                            scaled(&own(p.cell, n), f(1 << s)),
                            scaled(&own(p.cell + 1, n), f(1 << (2 * s))),
                        ],
                    ));
                }
            }
        }
    }

    fn widths(&self) -> Vec<u16> {
        let mut w = Vec::new();
        for p in &self.list {
            if p.boolean {
                w.push(1);
            } else {
                w.push(p.b as u16);
                w.push(2 * p.b as u16);
            }
        }
        w
    }

    fn fill(&self, x: u32, cells: &mut [Vec<Fp>], i: usize) {
        for p in &self.list {
            let d = ((x as u64) >> p.off) & ((1 << p.b) - 1);
            cells[p.cell][i] = f(d);
            if !p.boolean {
                cells[p.cell + 1][i] = f(spread_of(d, p.b));
            }
        }
    }

    fn end(&self) -> usize {
        self.first + self.cells
    }
}

/// A spread sum's even (XOR) and odd (AND, majority) halves: (dense, spread)
/// chunk pairs at bit offsets 0, 11 and 22.
#[derive(Clone, Copy)]
struct Split {
    first: usize,
}

const CHUNKS: [(usize, usize); 3] = [(0, 11), (11, 11), (22, 10)];

impl Split {
    const CELLS: usize = 12;

    fn half(&self, odd: bool, spread: bool, n: usize) -> Lin<Fp> {
        let mut out: Lin<Fp> = Vec::new();
        for (c, (off, _)) in CHUNKS.iter().enumerate() {
            let k = self.first + 6 * odd as usize + 2 * c + spread as usize;
            let coef = if spread {
                f(1 << (2 * off))
            } else {
                f(1 << off)
            };
            out = plus(&out, &scaled(&own(k, n), coef));
        }
        out
    }

    fn even(&self, n: usize) -> Lin<Fp> {
        self.half(false, false, n)
    }

    fn odd(&self, n: usize) -> Lin<Fp> {
        self.half(true, false, n)
    }

    /// `even spread + 2 odd spread = sum`, and the six chunk lookups (the
    /// 10-bit top chunks need no shifted lookup: an 11th bit would put the
    /// halves above the sum).
    fn constraints(
        &self,
        sum: &Lin<Fp>,
        n: usize,
        rows: &mut Vec<[Lin<Fp>; 3]>,
        lookups: &mut Vec<(u32, Vec<Lin<Fp>>)>,
    ) {
        let lhs = plus(
            &self.half(false, true, n),
            &scaled(&self.half(true, true, n), f(2)),
        );
        rows.push([
            plus(&lhs, &scaled(sum, -Fp::ONE)),
            constant(vec![Fp::ONE; n]),
            Vec::new(),
        ]);
        for h in 0..2 {
            for c in 0..3 {
                let k = self.first + 6 * h + 2 * c;
                lookups.push((SPREAD_TABLE, vec![own(k, n), own(k + 1, n)]));
            }
        }
    }

    fn widths() -> Vec<u16> {
        (0..2)
            .flat_map(|_| CHUNKS.iter().flat_map(|(_, w)| [*w as u16, 2 * *w as u16]))
            .collect()
    }

    /// Fills the halves of spread sum `s`; returns (even, odd).
    fn fill(&self, s: u64, cells: &mut [Vec<Fp>], i: usize) -> (u32, u32) {
        let even = (0..32).fold(0u32, |acc, b| acc | ((((s >> (2 * b)) & 1) as u32) << b));
        let odd = (0..32).fold(0u32, |acc, b| {
            acc | ((((s >> (2 * b + 1)) & 1) as u32) << b)
        });
        for (h, v) in [(0usize, even), (1, odd)] {
            for (c, (off, w)) in CHUNKS.iter().enumerate() {
                let d = ((v as u64) >> off) & ((1 << w) - 1);
                cells[self.first + 6 * h + 2 * c][i] = f(d);
                cells[self.first + 6 * h + 2 * c + 1][i] = f(spread_of(d, *w));
            }
        }
        (even, odd)
    }
}

/// Cell layouts.
struct Layout {
    msg: Pieces,
    msg_acc: usize,
    sched: Pieces,
    s_sig0: Split,
    s_sig1: Split,
    pa: Pieces,
    pe: Pieces,
    big1: Split,
    big0: Split,
    ch1: Split,
    ch2: Split,
    maj: Split,
}

fn layout() -> Layout {
    let msg = Pieces::new(&MSG_CUTS, 1);
    let msg_acc = msg.end();
    let sched = Pieces::new(&SCHED_CUTS, 3);
    let s_sig0 = Split { first: sched.end() };
    let s_sig1 = Split {
        first: sched.end() + Split::CELLS,
    };
    let pa = Pieces::new(&A_CUTS, 8);
    let pe = Pieces::new(&E_CUTS, pa.end());
    let s = pe.end();
    Layout {
        msg,
        msg_acc,
        sched,
        s_sig0,
        s_sig1,
        pa,
        pe,
        big1: Split { first: s },
        big0: Split {
            first: s + Split::CELLS,
        },
        ch1: Split {
            first: s + 2 * Split::CELLS,
        },
        ch2: Split {
            first: s + 3 * Split::CELLS,
        },
        maj: Split {
            first: s + 4 * Split::CELLS,
        },
    }
}

/// Round cells before the pieces: A, E, three E carries, three A carries.
const R_A: usize = 0;
const R_E: usize = 1;
const R_CE: usize = 2;
const R_CA: usize = 5;

/// The SHA-256 arrays and their native results.
pub struct Sha {
    pub arrays: Vec<Array<Fp>>,
    pub tables: Vec<(usize, u32)>,
    /// Per block: the digest and the encoded element m.
    pub digests: Vec<[u8; 32]>,
}

/// Builds the SHA-256 arrays from array index `first` for one message per
/// block (family `fam` with `2^pi` p slots; the spread table in global
/// family `gfam`).
pub fn sha(first: usize, fam: usize, pi: usize, gfam: usize, messages: &[Fp]) -> Sha {
    let at = |k: usize| first + k;
    let (msg_a, sched_a, round_a) = (at(A_MSG), at(A_SCHED), at(A_ROUND));
    let lay = layout();
    let slots = 1usize << pi;
    let (rho_m, rho_s, rho_r, rho_o) = (3usize, 6usize, 6usize, 3usize);
    let (nm, ns, nr, no) = (
        1usize << rho_m,
        1usize << rho_s,
        1usize << rho_r,
        1usize << rho_o,
    );
    let msg_cells = lay.msg_acc + 1;
    let sched_cells = lay.s_sig1.first + Split::CELLS;
    let round_cells = lay.maj.first + Split::CELLS;
    let mut mv = vec![vec![Fp::ZERO; slots << rho_m]; msg_cells];
    let mut sv = vec![vec![Fp::ZERO; slots << rho_s]; sched_cells];
    let mut rv = vec![vec![Fp::ZERO; slots << rho_r]; round_cells];
    let mut ov = vec![vec![Fp::ZERO; slots << rho_o]; 33];
    let iv = p256::SHA_IV;
    let kt = p256::SHA_K;
    let mut digests = Vec::new();
    let byte_pos = |off: usize| -> usize {
        // Bit `off` of word j sits at bit 32 j + byte_pos(off) of m (the word
        // is big-endian over m's little-endian bytes).
        match off {
            24.. => off - 24,
            16.. => 8 + off - 16,
            8.. => 16 + off - 8,
            _ => 24 + off,
        }
    };
    for (p, m) in messages.iter().enumerate() {
        let bytes = m.to_repr();
        let mut w = [0u32; 64];
        for j in 0..8 {
            w[j] = u32::from_be_bytes(bytes[4 * j..4 * j + 4].try_into().unwrap());
        }
        for (j, wj) in w.iter_mut().enumerate().take(16).skip(8) {
            *wj = pad(j);
        }
        // Message words and the running encoding.
        let mut acc = Fp::ZERO;
        for j in 0..8 {
            let i = (p << rho_m) | j;
            mv[0][i] = f(w[j] as u64);
            lay.msg.fill(w[j], &mut mv, i);
            for piece in &lay.msg.list {
                let d = ((w[j] as u64) >> piece.off) & ((1 << piece.b) - 1);
                acc += pow2(32 * j + byte_pos(piece.off)) * f(d);
            }
            mv[lay.msg_acc][i] = acc;
        }
        assert_eq!(acc, *m, "the running sum re-encodes m");
        // Schedule.
        for t in 16..64 {
            let r = t - 16;
            let i = (p << rho_s) | r;
            let total = w[t - 16] as u64
                + sigma0(w[t - 15]) as u64
                + w[t - 7] as u64
                + sigma1(w[t - 2]) as u64;
            w[t] = total as u32;
            sv[0][i] = f(w[t] as u64);
            sv[1][i] = f((total >> 32) & 1);
            sv[2][i] = f((total >> 33) & 1);
            lay.sched.fill(w[t], &mut sv, i);
            let (e0, _) = lay
                .s_sig0
                .fill(sigma_spread(w[t - 15], &[7, 18], Some(3)), &mut sv, i);
            let (e1, _) = lay
                .s_sig1
                .fill(sigma_spread(w[t - 2], &[17, 19], Some(10)), &mut sv, i);
            assert_eq!(
                (e0, e1),
                (sigma0(w[t - 15]), sigma1(w[t - 2])),
                "sigma splits"
            );
        }
        // Rounds.
        let mut st = iv;
        for t in 0..64 {
            let [a, b, c, d, e, ff, g, h] = st;
            let i = (p << rho_r) | t;
            lay.pa.fill(a, &mut rv, i);
            lay.pe.fill(e, &mut rv, i);
            let sp = |x: u32| spread_of(x as u64, 32);
            let (s1, _) = lay
                .big1
                .fill(sigma_spread(e, &[6, 11, 25], None), &mut rv, i);
            let (s0, _) = lay
                .big0
                .fill(sigma_spread(a, &[2, 13, 22], None), &mut rv, i);
            let (_, ef) = lay.ch1.fill(sp(e) + sp(ff), &mut rv, i);
            let (_, ng) = lay.ch2.fill(sp(!e) + sp(g), &mut rv, i);
            let (_, mj) = lay.maj.fill(sp(a) + sp(b) + sp(c), &mut rv, i);
            assert_eq!(ef | ng, (e & ff) ^ (!e & g), "Ch");
            assert_eq!(mj, (a & b) ^ (a & c) ^ (b & c), "Maj");
            let t1 = h as u64 + s1 as u64 + (ef | ng) as u64 + w[t] as u64 + kt[t] as u64;
            let te = t1 + d as u64;
            let ta = t1 + s0 as u64 + mj as u64;
            let (ne, na) = (te as u32, ta as u32);
            rv[R_A][i] = f(na as u64);
            rv[R_E][i] = f(ne as u64);
            for k in 0..3 {
                rv[R_CE + k][i] = f((te >> (32 + k)) & 1);
                rv[R_CA + k][i] = f((ta >> (32 + k)) & 1);
            }
            st = [na, a, b, c, ne, e, ff, g];
        }
        // Output words.
        let mut digest = [0u8; 32];
        for k in 0..8 {
            let i = (p << rho_o) | k;
            let total = st[k] as u64 + iv[k] as u64;
            for b in 0..32 {
                ov[b][i] = f((total >> b) & 1);
            }
            ov[32][i] = f(total >> 32);
            digest[4 * k..4 * k + 4].copy_from_slice(&(total as u32).to_be_bytes());
        }
        assert_eq!(
            digest,
            p256::sha256_one_block(&bytes),
            "round arrays reproduce SHA-256"
        );
        digests.push(digest);
    }

    // Templates.
    let one = |n: usize| constant(vec![Fp::ONE; n]);
    let neg = |l: &Lin<Fp>| scaled(l, -Fp::ONE);
    let linear = |l: Lin<Fp>, n: usize| -> [Lin<Fp>; 3] { [l, one(n), Vec::new()] };
    let spread32 = |x: u32| spread_of(x as u64, 32);

    // Message: booleanity, recomposition, running encoding.
    let (mut mr, mut ml) = (Vec::new(), Vec::new());
    lay.msg.constraints(nm, &mut mr, &mut ml);
    mr.push(linear(
        plus(&lay.msg.dense(&At::Own, nm), &neg(&own(0, nm))),
        nm,
    ));
    {
        let mut row = plus(
            &own(lay.msg_acc, nm),
            &neg(&link(
                msg_a,
                lay.msg_acc,
                map_of(nm, |r| r.checked_sub(1)),
                Fp::ONE,
            )),
        );
        for piece in &lay.msg.list {
            let coef: Vec<Fp> = (0..nm)
                .map(|j| -pow2(32 * j + byte_pos(piece.off)))
                .collect();
            row = plus(
                &row,
                &vec![crate::uniform::Term {
                    src: crate::uniform::Src::Own(piece.cell),
                    coef,
                }],
            );
        }
        mr.push(linear(row, nm));
    }

    // Schedule (r = t - 16).
    let (mut sr, mut sl) = (Vec::new(), Vec::new());
    sr.push([own(1, ns), own(1, ns), own(1, ns)]);
    sr.push([own(2, ns), own(2, ns), own(2, ns)]);
    lay.sched.constraints(ns, &mut sr, &mut sl);
    sr.push(linear(
        plus(&lay.sched.dense(&At::Own, ns), &neg(&own(0, ns))),
        ns,
    ));
    let msg_at = |g: &dyn Fn(usize) -> Option<usize>, n: usize| At::Link(msg_a, map_of(n, g));
    let sched_at = |g: &dyn Fn(usize) -> Option<usize>, n: usize| At::Link(sched_a, map_of(n, g));
    let round_at = |g: &dyn Fn(usize) -> Option<usize>, n: usize| At::Link(round_a, map_of(n, g));
    {
        // sigma0(w[r + 1]): message words 1..7, padding 8..15, schedule from 16.
        let src_m = msg_at(&|r| (r + 1 < 8).then_some(r + 1), ns);
        let src_s = sched_at(&|r| (r < 48 && r >= 15).then(|| r - 15), ns);
        let mut sum = plus(
            &lay.msg.sigma(&src_m, &[7, 18], Some(3), ns),
            &lay.sched.sigma(&src_s, &[7, 18], Some(3), ns),
        );
        sum = plus(
            &sum,
            &per_r(ns, |r| {
                (7..15)
                    .contains(&r)
                    .then(|| sigma_spread(pad(r + 1), &[7, 18], Some(3)))
            }),
        );
        lay.s_sig0.constraints(&sum, ns, &mut sr, &mut sl);
        // sigma1(w[r + 14]): padding 14, 15, schedule from 16.
        let src_s = sched_at(&|r| (r < 48 && r >= 2).then(|| r - 2), ns);
        let mut sum = lay.sched.sigma(&src_s, &[17, 19], Some(10), ns);
        sum = plus(
            &sum,
            &per_r(ns, |r| {
                (r < 2).then(|| sigma_spread(pad(r + 14), &[17, 19], Some(10)))
            }),
        );
        lay.s_sig1.constraints(&sum, ns, &mut sr, &mut sl);
        // W + 2^32 c0 + 2^33 c1 = w[r] + sigma0 + w[r + 9] + sigma1.
        let mut rhs = plus(&lay.s_sig0.even(ns), &lay.s_sig1.even(ns));
        rhs = plus(
            &rhs,
            &link(msg_a, 0, map_of(ns, |r| (r < 8).then_some(r)), Fp::ONE),
        );
        rhs = plus(
            &rhs,
            &link(
                sched_a,
                0,
                map_of(ns, |r| (16..48).contains(&r).then(|| r - 16)),
                Fp::ONE,
            ),
        );
        rhs = plus(
            &rhs,
            &link(
                sched_a,
                0,
                map_of(ns, |r| (7..48).contains(&r).then(|| r - 7)),
                Fp::ONE,
            ),
        );
        rhs = plus(
            &rhs,
            &per_r(ns, |r| {
                let mut c = 0u64;
                if (8..16).contains(&r) {
                    c += pad(r) as u64;
                }
                if r < 7 {
                    c += pad(r + 9) as u64;
                }
                (c != 0).then_some(c)
            }),
        );
        let lhs = plus(
            &plus(&own(0, ns), &scaled(&own(1, ns), pow2(32))),
            &scaled(&own(2, ns), pow2(33)),
        );
        sr.push(linear(plus(&lhs, &neg(&rhs)), ns));
    }

    // Rounds.
    let (mut rr, mut rl) = (Vec::new(), Vec::new());
    for k in 0..3 {
        rr.push([own(R_CE + k, nr), own(R_CE + k, nr), own(R_CE + k, nr)]);
        rr.push([own(R_CA + k, nr), own(R_CA + k, nr), own(R_CA + k, nr)]);
    }
    lay.pa.constraints(nr, &mut rr, &mut rl);
    lay.pe.constraints(nr, &mut rr, &mut rl);
    {
        let prev = |cell: usize, back: usize| {
            link(round_a, cell, map_of(nr, |r| r.checked_sub(back)), Fp::ONE)
        };
        // a_t = A_(t-1) (IV0 at t = 0), e_t = E_(t-1) (IV4).
        let a_t = plus(
            &prev(R_A, 1),
            &per_r(nr, |r| (r == 0).then_some(iv[0] as u64)),
        );
        let e_t = plus(
            &prev(R_E, 1),
            &per_r(nr, |r| (r == 0).then_some(iv[4] as u64)),
        );
        rr.push(linear(plus(&lay.pa.dense(&At::Own, nr), &neg(&a_t)), nr));
        rr.push(linear(plus(&lay.pe.dense(&At::Own, nr), &neg(&e_t)), nr));
        lay.big1.constraints(
            &lay.pe.sigma(&At::Own, &[6, 11, 25], None, nr),
            nr,
            &mut rr,
            &mut rl,
        );
        lay.big0.constraints(
            &lay.pa.sigma(&At::Own, &[2, 13, 22], None, nr),
            nr,
            &mut rr,
            &mut rl,
        );
        // Spreads of f = e_(t-1), g = e_(t-2), b = a_(t-1), c = a_(t-2), with
        // the IV words before round 0.
        let back =
            |pieces: &Pieces, k: usize| pieces.spread(&round_at(&|r| r.checked_sub(k), nr), nr);
        let iv_spread = |early: &[(usize, usize)]| {
            per_r(nr, |r| {
                early
                    .iter()
                    .find(|(t, _)| *t == r)
                    .map(|(_, w)| spread32(iv[*w]))
            })
        };
        let f_sp = plus(&back(&lay.pe, 1), &iv_spread(&[(0, 5)]));
        let g_sp = plus(&back(&lay.pe, 2), &iv_spread(&[(0, 6), (1, 5)]));
        let b_sp = plus(&back(&lay.pa, 1), &iv_spread(&[(0, 1)]));
        let c_sp = plus(&back(&lay.pa, 2), &iv_spread(&[(0, 2), (1, 1)]));
        let e_sp = lay.pe.spread(&At::Own, nr);
        lay.ch1
            .constraints(&plus(&e_sp, &f_sp), nr, &mut rr, &mut rl);
        let not_e = plus(&per_r(nr, |_| Some(spread32(u32::MAX))), &neg(&e_sp));
        lay.ch2
            .constraints(&plus(&not_e, &g_sp), nr, &mut rr, &mut rl);
        lay.maj.constraints(
            &plus(&plus(&lay.pa.spread(&At::Own, nr), &b_sp), &c_sp),
            nr,
            &mut rr,
            &mut rl,
        );
        // h = E_(t-4), d = A_(t-4), with IV7..IV4 and IV3..IV0 before.
        let h_t = plus(
            &prev(R_E, 4),
            &per_r(nr, |r| (r < 4).then(|| iv[7 - r] as u64)),
        );
        let d_t = plus(
            &prev(R_A, 4),
            &per_r(nr, |r| (r < 4).then(|| iv[3 - r] as u64)),
        );
        let w_t = plus(
            &plus(
                &link(msg_a, 0, map_of(nr, |r| (r < 8).then_some(r)), Fp::ONE),
                &link(
                    sched_a,
                    0,
                    map_of(nr, |r| (r >= 16).then(|| r - 16)),
                    Fp::ONE,
                ),
            ),
            &per_r(nr, |r| {
                ((8..16).contains(&r) && pad(r) != 0).then(|| pad(r) as u64)
            }),
        );
        let t1 = plus(
            &plus(
                &plus(&h_t, &lay.big1.even(nr)),
                &plus(&lay.ch1.odd(nr), &lay.ch2.odd(nr)),
            ),
            &plus(&w_t, &constant((0..nr).map(|r| f(kt[r] as u64)).collect())),
        );
        let carried = |out: usize, c: usize| -> Lin<Fp> {
            let mut l = own(out, nr);
            for k in 0..3 {
                l = plus(&l, &scaled(&own(c + k, nr), pow2(32 + k)));
            }
            l
        };
        rr.push(linear(
            plus(&carried(R_E, R_CE), &neg(&plus(&t1, &d_t))),
            nr,
        ));
        let t2 = plus(&lay.big0.even(nr), &lay.maj.odd(nr));
        rr.push(linear(plus(&carried(R_A, R_CA), &neg(&plus(&t1, &t2))), nr));
    }

    // Output: state + IV = bits + 2^32 carry.
    let mut orow = Vec::new();
    for b in 0..33 {
        orow.push([own(b, no), own(b, no), own(b, no)]);
    }
    {
        let mut bits: Lin<Fp> = Vec::new();
        for b in 0..33 {
            bits = plus(&bits, &scaled(&own(b, no), pow2(b)));
        }
        let state = plus(
            &link(
                round_a,
                R_A,
                map_of(no, |k| (k < 4).then(|| 63 - k)),
                Fp::ONE,
            ),
            &link(
                round_a,
                R_E,
                map_of(no, |k| (k >= 4).then(|| 67 - k)),
                Fp::ONE,
            ),
        );
        let rhs = plus(&state, &per_r(no, |k| Some(iv[k] as u64)));
        orow.push(linear(plus(&bits, &neg(&rhs)), no));
    }

    let mut mw = vec![32u16];
    mw.extend(lay.msg.widths());
    mw.push(255);
    let mut sw = vec![32u16, 1, 1];
    sw.extend(lay.sched.widths());
    sw.extend(Split::widths());
    sw.extend(Split::widths());
    let mut rw = vec![32u16, 32, 1, 1, 1, 1, 1, 1];
    rw.extend(lay.pa.widths());
    rw.extend(lay.pe.widths());
    for _ in 0..5 {
        rw.extend(Split::widths());
    }
    assert_eq!(
        (mw.len(), sw.len(), rw.len()),
        (msg_cells, sched_cells, round_cells),
        "widths per cell"
    );
    let arrays = vec![
        Array {
            name: "sha message",
            family: fam,
            rho: rho_m,
            r_count: 8,
            fixed: false,
            lookups: ml,
            widths: mw,
            rows: mr,
            values: mv,
        },
        Array {
            name: "sha schedule",
            family: fam,
            rho: rho_s,
            r_count: 48,
            fixed: false,
            lookups: sl,
            widths: sw,
            rows: sr,
            values: sv,
        },
        Array {
            name: "sha round",
            family: fam,
            rho: rho_r,
            r_count: 64,
            fixed: false,
            lookups: rl,
            widths: rw,
            rows: rr,
            values: rv,
        },
        Array {
            name: "sha output",
            family: fam,
            rho: rho_o,
            r_count: 8,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1; 33],
            rows: orow,
            values: ov,
        },
        Array {
            name: "spread table",
            family: gfam,
            rho: SPREAD,
            r_count: 1 << SPREAD,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![SPREAD as u16, 2 * SPREAD as u16],
            rows: Vec::new(),
            values: vec![
                (0..1u64 << SPREAD).map(f).collect(),
                (0..1u64 << SPREAD)
                    .map(|d| f(spread_of(d, SPREAD)))
                    .collect(),
            ],
        },
    ];
    Sha {
        arrays,
        tables: vec![(at(A_SPREAD), SPREAD_TABLE)],
        digests,
    }
}

/// Test and measurement messages: `count` field elements (stand-ins for the
/// Poseidon digests a wallet signs).
pub fn sample_messages(count: usize) -> Vec<Fp> {
    let mut rng = crate::fs::SpikeRng::new(b"sha messages");
    (0..count).map(|_| rng.scalar()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::r1cs::{Circuit, Witness};
    use crate::uniform::UniformCircuit;
    use iroha_pasta::Eq as Vesta;

    fn small() -> (
        UniformCircuit<Fp>,
        crate::r1cs::Instance<Fp>,
        Witness<Fp>,
        Vec<[u8; 32]>,
    ) {
        let messages = sample_messages(2);
        let Sha {
            arrays,
            tables,
            digests,
        } = sha(0, 0, 1, 1, &messages);
        let (c, inst, wit) = UniformCircuit::build(
            "sha-test",
            arrays,
            vec![1, 0],
            vec![2, 1],
            tables,
            13,
            4,
            b"sha",
        );
        (c, inst, wit, digests)
    }

    /// Proves a witness with one cell changed (array, cell, instance).
    fn refused(
        c: &UniformCircuit<Fp>,
        inst: &crate::r1cs::Instance<Fp>,
        wit: &Witness<Fp>,
        gens: &crate::ipa::Gens<Vesta>,
        a: usize,
        k: usize,
        i: usize,
    ) -> bool {
        let mut bad = wit.w.clone();
        bad[c.cell_index(a, k, i)] += Fp::ONE;
        let mut prng = SpikeRng::new(b"sha refuse");
        let (proof, _) =
            crate::spartan::prove_circuit(&c.shape, gens, inst, &Witness { w: bad }, &mut prng, c);
        crate::spartan::verify_circuit(&c.shape, gens, inst, &proof.to_bytes(), c).is_err()
    }

    #[test]
    fn sha_rows_hold_prove_and_refuse() {
        let (c, inst, wit, digests) = small();
        // The output bits are the native digest (asserted in `sha` too).
        for (p, d) in digests.iter().enumerate() {
            for k in 0..8 {
                let word = u32::from_be_bytes(d[4 * k..4 * k + 4].try_into().unwrap());
                for b in 0..32 {
                    let cell = wit.w[c.cell_index(A_OUT, b, (p << 3) | k)];
                    assert_eq!(cell, f(((word >> b) & 1) as u64), "digest bit");
                }
            }
        }
        let shape = c.shape;
        let mut rng = SpikeRng::new(b"sha ch");
        let ch: Vec<Fp> = vec![rng.scalar(), rng.scalar()];
        let mut w = wit.w.clone();
        let rel = c.instantiate(&shape, &ch, Some(&mut w));
        let z = crate::r1cs::z_vector(&shape, &inst, &Witness { w: w.clone() });
        let (az, bz, cz) = rel.abc(&shape, &z);
        let bad: Vec<usize> = (0..1 << shape.sx)
            .filter(|x| az[*x] * bz[*x] != cz[*x])
            .collect();
        assert!(
            bad.is_empty(),
            "{} rows fail, first {:?}",
            bad.len(),
            &bad[..bad.len().min(5)]
        );
        let rx: Vec<Fp> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fp> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (crate::grid::eq_table(&rx), crate::grid::eq_table(&ry));
        let mut slow = [Fp::ZERO; 3];
        let entries = c.entries(&ch);
        for &(mat, x, y, v) in &entries {
            slow[mat] += ex[x] * ey[y] * v;
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (slow[0], slow[1], slow[2]),
            "matrix evaluation matches the entries"
        );
        let rm: [Fp; 3] = [rng.scalar(), rng.scalar(), rng.scalar()];
        let mut m = vec![Fp::ZERO; 1 << shape.sz()];
        for &(mat, x, y, v) in &entries {
            m[y] += ex[x] * v * rm[mat];
        }
        assert!(
            rel.m_table(&shape, &ex, rm[0], rm[1], rm[2]) == m,
            "M(r_x, .) matches the entries"
        );
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let mut prng = SpikeRng::new(b"sha prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let lay = layout();
        // A changed round output (block 1, round 30), a changed schedule word
        // (block 0, t = 40), a changed message piece and running sum, a
        // flipped output bit and a changed Maj chunk.
        assert!(
            refused(&c, &inst, &wit, &gens, A_ROUND, R_E, (1 << 6) | 30),
            "changed E"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, A_SCHED, 0, 40 - 16),
            "changed schedule word"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, A_MSG, lay.msg.list[4].cell, 3),
            "changed message piece"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, A_MSG, lay.msg_acc, 5),
            "changed running sum"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, A_OUT, 7, (1 << 3) | 2),
            "flipped digest bit"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, A_ROUND, lay.maj.first + 6, 17),
            "changed Maj chunk"
        );
    }
}
