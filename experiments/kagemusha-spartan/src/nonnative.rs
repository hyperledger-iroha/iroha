//! Stage A2, step 3: the own-key comb ECDSA as uniform arrays (and, step 4,
//! the cross-circuit binding chain, at the end).
//!
//! - One canonical check template per product shape proves
//!   `sum_i c_i A_i B_i + lin = 0 (mod m)` for non-native values in three
//!   88-bit limbs, the modulus `m` (p or n) a per-instance coefficient. Its
//!   bounds come from the template's worst case (every port at its largest
//!   limbs), so no shape depends on data. As stage A's `mul_check`: five
//!   offset-encoded evaluation products per product (at 0, 1, -1, 2 and
//!   infinity), the offset quotient and three offset carries as range-checked
//!   11-bit chunks, three carry rows and one native row close the CRT
//!   argument.
//! - Every value the comb allocates (slopes, coordinates, the signature's
//!   scalars) is an instance of the value array: three limbs recomposed from
//!   range-checked chunks.
//! - The digits array holds both scalars' signed-digit bits (boolean, 16 per
//!   scalar and step) and a Horner accumulator, giving `u = 2 B - (2^256 - 1)`
//!   in three limbs; the four table selections per step are lookups whose
//!   index is read from these bits: the two of the wallet key's tables into
//!   the wallet table (a witness table of 512 rows, the circuit's wallet row,
//!   whose commitment the instance names), G's two into G's table (constant
//!   columns).
//! - Instances are indexed slot-major, `r = slot * 16 + step`, so a check's
//!   inputs are links whose maps factor into a slot table and a step relation
//!   (the same step, or the previous step's final point).
//! - Every step is a double-add and three additions: the accumulator starts
//!   at a fixed offset point `O` (constant terms) and ends at
//!   `2^16 O + u1 G + u2 Q`; one final addition of `C = -2^16 O` (both slope
//!   equations, so equal points double) gives `P = u1 G + u2 Q`.
//! - Per signature: `r rinv = 1`, `u1 s = z`, `u2 s = r` (mod n) as extra
//!   one-product checks; `r, s <= n - 1`, `x(P) <= p - 1` and
//!   `x(P) = r + flag n` as integer additions with boolean carries.

use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_traits::{One, Zero};

use crate::p256::{self, Point};
use crate::uniform::{Array, Lin, Src as USrc, Term, compact, constant, link, own, plus, scaled};

pub type Fp = iroha_pasta::Fp;

const L: usize = 88;
pub(crate) const CHUNK: usize = 11;
pub const STEPS: usize = 16;
pub const RANGE_TABLE: u32 = 0;
/// G's comb tables (constant columns).
pub const POINT_TABLE: u32 = 1;
/// The wallet key's comb tables: a witness table, the wallet row.
pub const WALLET_TABLE: u32 = 7;

/// The lookup table holding one signature family's public key multiples.
/// Families for different keys must use different table ids, distinct from
/// every other table in their circuit. The range and generator tables remain
/// shared.
#[derive(Clone, Copy, Debug)]
pub enum KeyTable {
    /// Witness values. The caller must authenticate the table commitment,
    /// as the wallet row does; this choice alone does not authenticate a key.
    Witness { id: u32 },
    /// Circuit constants, authenticated by the circuit's verification key.
    /// Changing this public key changes that circuit; this is not a mechanism
    /// for accepting a newly certified issuer in an existing circuit.
    Fixed { id: u32 },
}

pub(crate) fn pow2(k: usize) -> BigUint {
    BigUint::one() << k
}

fn fp_modulus() -> BigUint {
    use ff::PrimeField;
    BigUint::from_bytes_le((-Fp::from(1u64)).to_repr().as_ref()) + 1u32
}

pub(crate) fn fp_u(x: &BigUint) -> Fp {
    use ff::PrimeField;
    let mut repr = [0u8; 32];
    let bytes = (x % fp_modulus()).to_bytes_le();
    repr[..bytes.len()].copy_from_slice(&bytes);
    Fp::from_repr(repr).unwrap()
}

fn fp_i(x: &BigInt) -> Fp {
    let mag = fp_u(x.magnitude());
    if x.sign() == Sign::Minus { -mag } else { mag }
}

fn limbs(x: &BigUint) -> [BigUint; 3] {
    [x % pow2(L), (x >> L) % pow2(L), x >> (2 * L)]
}

fn bits(x: &BigUint) -> usize {
    (x.bits() as usize).max(1)
}

/// Chunk widths of a `width`-bit integer: full 11-bit chunks and a partial
/// top chunk (checked twice: as is, and shifted to the top of the table).
pub(crate) fn chunk_widths(width: usize) -> Vec<usize> {
    let mut w = Vec::new();
    let mut left = width;
    while left > 0 {
        let c = CHUNK.min(left);
        w.push(c);
        left -= c;
    }
    w
}

/// Range lookups for a chunk cell of width `w`.
pub(crate) fn range_lookups(
    lookups: &mut Vec<(u32, Vec<Lin<Fp>>)>,
    cell: usize,
    w: usize,
    n: usize,
) {
    lookups.push((RANGE_TABLE, vec![own(cell, n)]));
    if w < CHUNK {
        lookups.push((
            RANGE_TABLE,
            vec![scaled(&own(cell, n), fp_u(&pow2(CHUNK - w)))],
        ));
    }
}

/// `sum_c 2^(shift_c) chunk_c` over consecutive chunk cells from `first`.
pub(crate) fn recompose(first: usize, widths: &[usize], n: usize) -> Lin<Fp> {
    let mut out: Lin<Fp> = Vec::new();
    let mut shift = 0;
    for (c, w) in widths.iter().enumerate() {
        out = plus(&out, &scaled(&own(first + c, n), fp_u(&pow2(shift))));
        shift += w;
    }
    out
}

/// Chunk values of `x` for `widths`.
pub(crate) fn chunk_values(x: &BigUint, widths: &[usize]) -> Vec<Fp> {
    let mut shift = 0;
    widths
        .iter()
        .map(|w| {
            let v = fp_u(&((x >> shift) % pow2(*w)));
            shift += w;
            v
        })
        .collect()
}

/// A linear combination with each term's coefficient multiplied per r.
fn scaled_r(lin: &Lin<Fp>, s: &[Fp]) -> Lin<Fp> {
    compact(
        lin.iter()
            .map(|t| Term {
                src: t.src.clone(),
                coef: t.coef.iter().zip(s).map(|(c, x)| *c * x).collect(),
            })
            .collect(),
    )
}

/// A non-native input of a check, per reading `r`: links to limbs plus
/// constant limbs.
#[derive(Clone)]
pub struct Port {
    /// (array, first limb cell, map, coefficient per r).
    pub links: Vec<(usize, usize, Vec<Option<usize>>, Vec<Fp>)>,
    /// Constant limbs per r.
    pub consts: [Vec<Fp>; 3],
}

impl Port {
    fn limb(&self, k: usize) -> Lin<Fp> {
        let mut out: Lin<Fp> = Vec::new();
        for (array, first, map, coef) in &self.links {
            out.push(Term {
                src: USrc::Link {
                    array: *array,
                    cell: first + k,
                    map: map.clone(),
                },
                coef: coef.clone(),
            });
        }
        if self.consts[k]
            .iter()
            .any(|c| !bool::from(ff::Field::is_zero(c)))
        {
            out.push(Term {
                src: USrc::One,
                coef: self.consts[k].clone(),
            });
        }
        compact(out)
    }
}

/// Absolute limb bounds of a port.
pub type Bound = [BigUint; 3];

/// `count` values below 2^256 with coefficients up to `coef`.
fn sources(count: u64, coef: u64) -> Bound {
    [pow2(L) - 1u32, pow2(L) - 1u32, pow2(256 - 2 * L) - 1u32]
        .map(|x| x * BigUint::from(count * coef))
}

fn max_bound(a: &Bound, b: &Bound) -> Bound {
    [0, 1, 2].map(|k| a[k].clone().max(b[k].clone()))
}

/// Digit accumulator limb widths: Horner doubling leaves limb 0 unnormalized.
const ACC_BITS: [usize; 3] = [96, 88, 80];

/// Limbs of `u = 2 acc - (2^256 - 1)` are below these in absolute value.
fn u_bound() -> Bound {
    [
        pow2(ACC_BITS[0] + 1),
        pow2(ACC_BITS[1] + 1),
        pow2(ACC_BITS[2] + 1),
    ]
}

/// Evaluation points; `i64::MAX` stands for infinity (the top coefficient).
const EVAL: [i64; 5] = [0, 1, -1, 2, i64::MAX];

/// The canonical shape of a check: offsets, quotient and carry widths, and
/// the cell layout `[evaluation products | quotient chunks | carry chunks]`.
#[derive(Clone)]
pub struct CheckShape {
    pub products: usize,
    off: Vec<[BigUint; 5]>,
    off_bits: Vec<[usize; 5]>,
    q0: BigUint,
    qbits: usize,
    c_off: [BigUint; 3],
    c_bits: [usize; 3],
    pub moduli: Vec<BigUint>,
    /// `q0 m` spread over the seven columns, per modulus.
    constant_cols: Vec<Vec<BigUint>>,
    q_chunks: [Vec<usize>; 3],
    c_chunks: [Vec<usize>; 3],
    pub cells: usize,
}

impl CheckShape {
    /// `ports[i]`: limb bounds of product `i`'s two inputs (coefficients
    /// +-1); `lin`: the linear part's limb bounds; `cst_max`: the largest
    /// small constant; `moduli`: the moduli instances may use.
    pub fn new(ports: &[(Bound, Bound)], lin: &Bound, cst_max: u64, moduli: &[BigUint]) -> Self {
        let eval_bound = |b: &Bound, t: i64| -> BigUint {
            if t == i64::MAX {
                return b[2].clone();
            }
            let at = BigUint::from(t.unsigned_abs());
            &b[0] + &at * &b[1] + &at * &at * &b[2]
        };
        let mut off = Vec::new();
        let mut off_bits = Vec::new();
        let mut col_max = vec![BigUint::zero(); 7];
        for (ba, bb) in ports {
            let mut o: [BigUint; 5] = Default::default();
            let mut ob = [0usize; 5];
            for (slot, t) in EVAL.iter().copied().enumerate() {
                let m = eval_bound(ba, t) * eval_bound(bb, t);
                ob[slot] = bits(&m);
                o[slot] = pow2(ob[slot]);
            }
            off.push(o);
            off_bits.push(ob);
            for i in 0..3 {
                for j in 0..3 {
                    col_max[i + j] += &ba[i] * &bb[j];
                }
            }
        }
        for k in 0..3 {
            col_max[k] += &lin[k];
        }
        col_max[0] += BigUint::from(cst_max);
        // The left-hand side's integer bound and the offset quotient (the
        // smallest modulus has the largest quotient).
        let lhs_max: BigUint = (0..7).map(|k| &col_max[k] << (L * k)).sum();
        let mmin = moduli.iter().min().expect("a modulus").clone();
        let qb = bits(&(&lhs_max / &mmin + 1u32));
        let q0 = pow2(qb);
        let qbits = qb + 1;
        let mut constant_cols = Vec::new();
        for m in moduli {
            let mut cst = &q0 * m;
            let mut cols = Vec::new();
            for _ in 0..7 {
                cols.push(&cst % pow2(L));
                cst >>= L;
            }
            assert!(cst.is_zero(), "q0 m fits the columns");
            constant_cols.push(cols);
        }
        for k in 0..7 {
            col_max[k] += constant_cols.iter().map(|c| c[k].clone()).max().unwrap();
        }
        let qmax = limbs(&(pow2(qbits) - 1u32));
        for i in 0..3 {
            for j in 0..3 {
                let mj = moduli.iter().map(|m| limbs(m)[j].clone()).max().unwrap();
                col_max[i + j] += &qmax[i] * mj;
            }
        }
        // Carries over columns 0..2, offset-encoded.
        let half = fp_modulus() >> 1;
        let mut c_off: [BigUint; 3] = Default::default();
        let mut c_bits = [0usize; 3];
        let mut carry_max = BigUint::zero();
        for k in 0..3 {
            let next = (&col_max[k] + &carry_max) / pow2(L) + 1u32;
            assert!(
                &col_max[k] + &carry_max + (&next << L) < half,
                "carry row {k} is an integer identity"
            );
            let cb = bits(&next);
            c_off[k] = pow2(cb);
            c_bits[k] = cb + 1;
            carry_max = next;
        }
        // CRT: |E| <= lhs_max + q0 m < 2^264 Fp / 2 for every modulus.
        let bound = (pow2(3 * L) * fp_modulus()) >> 1;
        for m in moduli {
            let emax = &lhs_max + &q0 * m;
            assert!(
                emax < bound,
                "CRT bound: |E| < 2^{} vs 2^{}",
                emax.bits(),
                bound.bits()
            );
        }
        let q_chunks = [L, L, qbits - 2 * L].map(chunk_widths);
        let c_chunks = c_bits.map(chunk_widths);
        let cells = 5 * ports.len()
            + q_chunks
                .iter()
                .chain(&c_chunks)
                .map(|c| c.len())
                .sum::<usize>();
        CheckShape {
            products: ports.len(),
            off,
            off_bits,
            q0,
            qbits,
            c_off,
            c_bits,
            moduli: moduli.to_vec(),
            constant_cols,
            q_chunks,
            c_chunks,
            cells,
        }
    }

    fn q_first(&self, i: usize) -> usize {
        5 * self.products + self.q_chunks[..i].iter().map(|c| c.len()).sum::<usize>()
    }

    fn c_first(&self, k: usize) -> usize {
        self.q_first(3) + self.c_chunks[..k].iter().map(|c| c.len()).sum::<usize>()
    }

    /// Cell widths for the commitment's row bounds.
    pub fn widths(&self) -> Vec<u16> {
        let mut w: Vec<u16> = self
            .off_bits
            .iter()
            .flat_map(|ob| ob.iter().map(|b| (*b + 1) as u16))
            .collect();
        w.extend(
            self.q_chunks
                .iter()
                .chain(&self.c_chunks)
                .flatten()
                .map(|c| *c as u16),
        );
        w
    }

    /// Range lookups per instance.
    pub fn lookups(&self) -> usize {
        self.q_chunks
            .iter()
            .chain(&self.c_chunks)
            .flatten()
            .map(|w| if *w < CHUNK { 2 } else { 1 })
            .sum()
    }

    /// Template rows and range lookups for inputs `a[i]`, `b[i]`, product
    /// coefficients `pc[i]` (per r), the linear part, a small constant per r
    /// and the modulus index per r, in an array of `n` slots.
    pub fn template(
        &self,
        a: &[Port],
        b: &[Port],
        pc: &[Vec<Fp>],
        lin: &Port,
        cst: &[Fp],
        mod_of: &[usize],
        n: usize,
    ) -> (Vec<[Lin<Fp>; 3]>, Vec<(u32, Vec<Lin<Fp>>)>) {
        self.template_lin(
            a,
            b,
            pc,
            &[lin.limb(0), lin.limb(1), lin.limb(2)],
            cst,
            mod_of,
            n,
        )
    }

    /// As [`Self::template`], with the linear part given limb by limb.
    pub fn template_lin(
        &self,
        a: &[Port],
        b: &[Port],
        pc: &[Vec<Fp>],
        lin: &[Lin<Fp>; 3],
        cst: &[Fp],
        mod_of: &[usize],
        n: usize,
    ) -> (Vec<[Lin<Fp>; 3]>, Vec<(u32, Vec<Lin<Fp>>)>) {
        let one = Fp::from(1u64);
        let mut rows: Vec<[Lin<Fp>; 3]> = Vec::new();
        let mut lookups: Vec<(u32, Vec<Lin<Fp>>)> = Vec::new();
        let ev = |port: &Port, t: i64| -> Lin<Fp> {
            if t == i64::MAX {
                return port.limb(2);
            }
            let mut out: Lin<Fp> = Vec::new();
            for k in 0..3 {
                out = plus(
                    &out,
                    &scaled(&port.limb(k), fp_i(&BigInt::from(t).pow(k as u32))),
                );
            }
            out
        };
        let inv2 = ff::Field::invert(&Fp::from(2u64)).unwrap();
        let inv3 = ff::Field::invert(&Fp::from(3u64)).unwrap();
        let mut cols: Vec<Lin<Fp>> = vec![Vec::new(); 7];
        for i in 0..self.products {
            let mut pts: Vec<Lin<Fp>> = Vec::new();
            for (slot, t) in EVAL.iter().copied().enumerate() {
                let value = plus(
                    &own(5 * i + slot, n),
                    &constant(vec![-fp_u(&self.off[i][slot]); n]),
                );
                rows.push([ev(&a[i], t), ev(&b[i], t), value.clone()]);
                pts.push(value);
            }
            let (p0, p1, pm1, p2, pinf) = (&pts[0], &pts[1], &pts[2], &pts[3], &pts[4]);
            let c0 = p0.clone();
            let c4 = pinf.clone();
            let c2 = plus(
                &plus(&scaled(&plus(p1, pm1), inv2), &scaled(&c0, -one)),
                &scaled(&c4, -one),
            );
            let s = scaled(&plus(p1, &scaled(pm1, -one)), inv2);
            let t = scaled(
                &plus(
                    &plus(&plus(p2, &scaled(&c0, -one)), &scaled(&c2, -Fp::from(4u64))),
                    &scaled(&c4, -Fp::from(16u64)),
                ),
                inv2,
            );
            let c3 = scaled(&plus(&t, &scaled(&s, -one)), inv3);
            let c1 = plus(&s, &scaled(&c3, -one));
            for (k, ck) in [c0, c1, c2, c3, c4].into_iter().enumerate() {
                cols[k] = plus(&cols[k], &scaled_r(&ck, &pc[i]));
            }
        }
        for k in 0..3 {
            cols[k] = plus(&cols[k], &lin[k]);
        }
        cols[0] = plus(&cols[0], &constant(cst.to_vec()));
        for k in 0..7 {
            cols[k] = plus(
                &cols[k],
                &constant(
                    (0..n)
                        .map(|r| fp_u(&self.constant_cols[mod_of[r]][k]))
                        .collect(),
                ),
            );
        }
        // The offset quotient q' = q + q0 from its chunks: columns lose q' m.
        let ml: Vec<[BigUint; 3]> = self.moduli.iter().map(limbs).collect();
        for i in 0..3 {
            let qi = recompose(self.q_first(i), &self.q_chunks[i], n);
            for (c, w) in self.q_chunks[i].iter().enumerate() {
                range_lookups(&mut lookups, self.q_first(i) + c, *w, n);
            }
            for j in 0..3 {
                let mj: Vec<Fp> = (0..n).map(|r| -fp_u(&ml[mod_of[r]][j])).collect();
                cols[i + j] = plus(&cols[i + j], &scaled_r(&qi, &mj));
            }
        }
        // Carry chain: col_k + carry_k = 2^88 carry_(k+1).
        let mut carry: Lin<Fp> = Vec::new();
        for k in 0..3 {
            for (c, w) in self.c_chunks[k].iter().enumerate() {
                range_lookups(&mut lookups, self.c_first(k) + c, *w, n);
            }
            let next = plus(
                &recompose(self.c_first(k), &self.c_chunks[k], n),
                &constant(vec![-fp_u(&self.c_off[k]); n]),
            );
            let row = plus(&plus(&cols[k], &carry), &scaled(&next, -fp_u(&pow2(L))));
            rows.push([row, constant(vec![one; n]), Vec::new()]);
            carry = next;
        }
        // Native row: sum_k col_k 2^(88 k) = 0 in Fp.
        let mut native: Lin<Fp> = Vec::new();
        for k in 0..7 {
            native = plus(&native, &scaled(&cols[k], fp_u(&pow2(L * k))));
        }
        rows.push([native, constant(vec![one; n]), Vec::new()]);
        (rows, lookups)
    }

    /// One instance's cells for input limbs `a[i]`, `b[i]` (coefficients
    /// applied), product coefficients, linear limbs, constant and modulus.
    pub fn witness(
        &self,
        a: &[[BigInt; 3]],
        b: &[[BigInt; 3]],
        pc: &[i64],
        lin: &[BigInt; 3],
        cst: i64,
        modulus: usize,
    ) -> Vec<Fp> {
        let m = &self.moduli[modulus];
        let ml = limbs(m);
        let mut cells = vec![Fp::from(0u64); self.cells];
        let val = |x: &[BigInt; 3]| -> BigInt { &x[0] + (&x[1] << L) + (&x[2] << (2 * L)) };
        let ev = |x: &[BigInt; 3], t: i64| -> BigInt {
            if t == i64::MAX {
                return x[2].clone();
            }
            let t = BigInt::from(t);
            &x[0] + &t * &x[1] + &t * &t * &x[2]
        };
        let mut lhs = val(lin) + BigInt::from(cst);
        let mut cols: Vec<BigInt> = vec![BigInt::zero(); 7];
        for i in 0..self.products {
            for (slot, t) in EVAL.iter().copied().enumerate() {
                let prod = ev(&a[i], t) * ev(&b[i], t);
                let stored = (prod + BigInt::from(self.off[i][slot].clone()))
                    .to_biguint()
                    .expect("offset product >= 0");
                assert!(
                    bits(&stored) <= self.off_bits[i][slot] + 1,
                    "product within its canonical width"
                );
                cells[5 * i + slot] = fp_u(&stored);
            }
            lhs += BigInt::from(pc[i]) * val(&a[i]) * val(&b[i]);
            for x in 0..3 {
                for y in 0..3 {
                    cols[x + y] += BigInt::from(pc[i]) * &a[i][x] * &b[i][y];
                }
            }
        }
        for k in 0..3 {
            cols[k] += &lin[k];
        }
        cols[0] += BigInt::from(cst);
        let mi = BigInt::from(m.clone());
        assert!(lhs.mod_floor(&mi).is_zero(), "the check holds mod m");
        let q = &lhs / &mi;
        let qp = (q + BigInt::from(self.q0.clone()))
            .to_biguint()
            .expect("offset quotient >= 0");
        assert!(
            bits(&qp) <= self.qbits,
            "quotient within its canonical width"
        );
        let ql = limbs(&qp);
        for i in 0..3 {
            for (c, v) in chunk_values(&ql[i], &self.q_chunks[i])
                .into_iter()
                .enumerate()
            {
                cells[self.q_first(i) + c] = v;
            }
        }
        for k in 0..7 {
            cols[k] += BigInt::from(self.constant_cols[modulus][k].clone());
        }
        for i in 0..3 {
            for j in 0..3 {
                cols[i + j] -= BigInt::from(ql[i].clone()) * BigInt::from(ml[j].clone());
            }
        }
        let mut carry = BigInt::zero();
        for k in 0..3 {
            let (next, rem) = (&cols[k] + &carry).div_rem(&BigInt::from(pow2(L)));
            assert!(rem.is_zero(), "column {k} divisible by 2^88");
            let shifted = (&next + BigInt::from(self.c_off[k].clone()))
                .to_biguint()
                .expect("offset carry >= 0");
            assert!(
                bits(&shifted) <= self.c_bits[k],
                "carry within its canonical width"
            );
            for (c, v) in chunk_values(&shifted, &self.c_chunks[k])
                .into_iter()
                .enumerate()
            {
                cells[self.c_first(k) + c] = v;
            }
            carry = next;
        }
        let total: BigInt = (0..7).map(|k| &cols[k] << (L * k)).sum();
        assert!(total.is_zero(), "E = 0 over the integers");
        cells
    }
}

/// Value array cells: three limbs, then their chunks (88 + 88 + 80 bits).
fn value_chunks() -> [Vec<usize>; 3] {
    [L, L, 256 - 2 * L].map(chunk_widths)
}

pub fn value_cells_count() -> usize {
    3 + value_chunks().iter().map(|c| c.len()).sum::<usize>()
}

fn value_template(n: usize) -> (Vec<[Lin<Fp>; 3]>, Vec<(u32, Vec<Lin<Fp>>)>) {
    let w = value_chunks();
    let one = Fp::from(1u64);
    let mut rows = Vec::new();
    let mut lookups = Vec::new();
    let mut first = 3;
    for (k, wk) in w.iter().enumerate() {
        let rec = plus(&own(k, n), &scaled(&recompose(first, wk, n), -one));
        for (c, width) in wk.iter().enumerate() {
            range_lookups(&mut lookups, first + c, *width, n);
        }
        rows.push([rec, constant(vec![one; n]), Vec::new()]);
        first += wk.len();
    }
    (rows, lookups)
}

fn value_cells(x: &BigUint) -> Vec<Fp> {
    let l = limbs(x);
    let mut out: Vec<Fp> = l.iter().map(fp_u).collect();
    for (k, wk) in value_chunks().iter().enumerate() {
        out.extend(chunk_values(&l[k], wk));
    }
    out
}

fn value_widths() -> Vec<u16> {
    [L as u16, L as u16, (256 - 2 * L) as u16]
        .into_iter()
        .chain(value_chunks().iter().flatten().map(|w| *w as u16))
        .collect()
}

/// Signed-digit bits `b_t` of `u`: `u = sum_t (2 b_t - 1) 2^t (mod n)`.
pub fn comb_bits(u: &BigUint) -> Vec<bool> {
    let n = p256::n();
    let inv2 = (&n + 1u32) >> 1;
    let b = (u + pow2(256) - 1u32) % &n * &inv2 % &n;
    (0..256)
        .map(|t| ((&b >> t) % 2u32) == BigUint::one())
        .collect()
}

/// Slope of the addition a + c: the chord, or the tangent when equal.
fn slope(a: &Point, c: &Point) -> BigUint {
    let p = p256::p();
    if a.x == c.x {
        assert_eq!(a.y, c.y, "opposite points have no sum");
        let num = (BigUint::from(3u32) * &a.x * &a.x + &p - 3u32) % &p;
        return num * p256::inv(&(BigUint::from(2u32) * &a.y % &p), &p) % &p;
    }
    let dx = (&c.x + &p - &a.x) % &p;
    let dy = (&c.y + &p - &a.y) % &p;
    dy * p256::inv(&dx, &p) % &p
}

/// Where a check input reads from.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Kind {
    /// A value slot of this step.
    Value,
    /// Selected point `slot`'s x (0) or y (1) of this step.
    Sel(usize),
    /// The accumulator's x (slot 0) or y (slot 1): the previous step's final
    /// point, or the offset point at step 0.
    Acc,
    /// The comb's final accumulator, x (slot 0) or y (slot 1).
    Final,
    /// A per-signature value (slot = its index in [`EXTRA`]).
    Extra,
    /// The scalar `u` of slot 0 (u1) or 1 (u2) from the digits array.
    Digits,
    /// A constant.
    Const(BigUint),
}

#[derive(Clone, Debug)]
struct Src {
    kind: Kind,
    slot: usize,
    coef: i64,
}

#[derive(Clone)]
struct CheckSpec {
    a: Vec<Vec<Src>>,
    b: Vec<Vec<Src>>,
    pc: Vec<i64>,
    lin: Vec<Src>,
    cst: i64,
    /// Modulus index: 0 = p, 1 = n.
    modulus: usize,
}

fn v(slot: usize, coef: i64) -> Src {
    Src {
        kind: Kind::Value,
        slot,
        coef,
    }
}

type PtSrc = (Src, Src);

/// The value slots of one step: double_add(acc, e0) -> l1 v0, xr v1, l2 v2,
/// xo v3, yo v4; then three additions with e1, e2, e3 -> (l, x3, y3) at v5,
/// v8, v11 (the step's final point at v12, v13).
pub const VALUE_SLOTS: usize = 14;
const FINAL_X: usize = 12;
/// Per-signature values, in value slot 14.
const EXTRA_SLOT: usize = 14;
pub const EXTRA: [&str; 10] = [
    "r", "s", "z", "rinv", "n-1-r", "n-1-s", "p-1-x", "lambda", "x", "t2",
];
const R_: usize = 0;
const S_: usize = 1;
const Z_: usize = 2;
const RINV_: usize = 3;
/// The value array's instance (within a signature) holding z; its cells 0-2
/// are z's 88-bit limbs.
pub const Z_INSTANCE: usize = (EXTRA_SLOT << 4) | Z_;
const DR_: usize = 4;
const DS_: usize = 5;
const DX_: usize = 6;
const LAM_: usize = 7;
const XP_: usize = 8;
const T2_: usize = 9;

/// The checks of one comb step: (one-product checks, four-product checks).
fn step_checks() -> (Vec<CheckSpec>, Vec<CheckSpec>) {
    let mut one = Vec::new();
    let mut four = Vec::new();
    let neg = |s: &Src| Src {
        coef: -s.coef,
        ..s.clone()
    };
    let slope_checks =
        |l: usize, a: &PtSrc, c: &PtSrc, one: &mut Vec<CheckSpec>, four: &mut Vec<CheckSpec>| {
            // l (c.x - a.x) - (c.y - a.y) = 0.
            one.push(CheckSpec {
                a: vec![vec![v(l, 1)]],
                b: vec![vec![c.0.clone(), neg(&a.0)]],
                pc: vec![1],
                lin: vec![neg(&c.1), a.1.clone()],
                cst: 0,
                modulus: 0,
            });
            // l (a.y + c.y) - a.x^2 - a.x c.x - c.x^2 + 3 = 0 (both on the curve).
            four.push(CheckSpec {
                a: vec![
                    vec![v(l, 1)],
                    vec![a.0.clone()],
                    vec![a.0.clone()],
                    vec![c.0.clone()],
                ],
                b: vec![
                    vec![a.1.clone(), c.1.clone()],
                    vec![a.0.clone()],
                    vec![c.0.clone()],
                    vec![c.0.clone()],
                ],
                pc: vec![1, -1, -1, -1],
                lin: vec![],
                cst: 3,
                modulus: 0,
            });
        };
    let acc: PtSrc = (
        Src {
            kind: Kind::Acc,
            slot: 0,
            coef: 1,
        },
        Src {
            kind: Kind::Acc,
            slot: 1,
            coef: 1,
        },
    );
    let e = |j: usize| -> PtSrc {
        (
            Src {
                kind: Kind::Sel(0),
                slot: j,
                coef: 1,
            },
            Src {
                kind: Kind::Sel(1),
                slot: j,
                coef: 1,
            },
        )
    };
    let (l1, xr, l2, xo, yo) = (0, 1, 2, 3, 4);
    slope_checks(l1, &acc, &e(0), &mut one, &mut four);
    // r = acc + e0; out = r + acc.
    one.push(CheckSpec {
        a: vec![vec![v(l1, 1)]],
        b: vec![vec![v(l1, 1)]],
        pc: vec![1],
        lin: vec![neg(&acc.0), neg(&e(0).0), v(xr, -1)],
        cst: 0,
        modulus: 0,
    });
    one.push(CheckSpec {
        a: vec![vec![v(l1, 1), v(l2, 1)]],
        b: vec![vec![acc.0.clone(), v(xr, -1)]],
        pc: vec![1],
        lin: vec![Src {
            coef: -2,
            ..acc.1.clone()
        }],
        cst: 0,
        modulus: 0,
    });
    one.push(CheckSpec {
        a: vec![vec![v(l2, 1)]],
        b: vec![vec![v(l2, 1)]],
        pc: vec![1],
        lin: vec![v(xr, -1), neg(&acc.0), v(xo, -1)],
        cst: 0,
        modulus: 0,
    });
    one.push(CheckSpec {
        a: vec![vec![v(l2, 1)]],
        b: vec![vec![acc.0.clone(), v(xo, -1)]],
        pc: vec![1],
        lin: vec![neg(&acc.1), v(yo, -1)],
        cst: 0,
        modulus: 0,
    });
    let mut a: PtSrc = (v(xo, 1), v(yo, 1));
    for (k, j) in [1usize, 2, 3].into_iter().enumerate() {
        let (l, x3, y3) = (5 + 3 * k, 6 + 3 * k, 7 + 3 * k);
        let c = e(j);
        slope_checks(l, &a, &c, &mut one, &mut four);
        one.push(CheckSpec {
            a: vec![vec![v(l, 1)]],
            b: vec![vec![v(l, 1)]],
            pc: vec![1],
            lin: vec![neg(&a.0), neg(&c.0), v(x3, -1)],
            cst: 0,
            modulus: 0,
        });
        one.push(CheckSpec {
            a: vec![vec![v(l, 1)]],
            b: vec![vec![a.0.clone(), v(x3, -1)]],
            pc: vec![1],
            lin: vec![neg(&a.1), v(y3, -1)],
            cst: 0,
            modulus: 0,
        });
        a = (v(x3, 1), v(y3, 1));
    }
    assert_eq!(a.0.slot, FINAL_X);
    (one, four)
}

/// The per-signature one-product checks, after the steps' (one instance
/// each): `r rinv = 1`, `u1 s = z`, `u2 s = r` (mod n); then `P = acc + C`
/// with both slope equations (`t2 = acc.x (acc.x + C.x)` carries the second
/// one's two products).
fn extra_checks(c: &Point) -> Vec<CheckSpec> {
    let p = p256::p();
    let ex = |slot: usize, coef: i64| Src {
        kind: Kind::Extra,
        slot,
        coef,
    };
    let fin = |slot: usize, coef: i64| Src {
        kind: Kind::Final,
        slot,
        coef,
    };
    let dig = |slot: usize| Src {
        kind: Kind::Digits,
        slot,
        coef: 1,
    };
    let cst = |x: &BigUint, coef: i64| Src {
        kind: Kind::Const(x.clone()),
        slot: 0,
        coef,
    };
    let k3 = (BigUint::from(3u32) + &p - (&c.x * &c.x % &p)) % &p;
    vec![
        CheckSpec {
            a: vec![vec![ex(R_, 1)]],
            b: vec![vec![ex(RINV_, 1)]],
            pc: vec![1],
            lin: vec![],
            cst: -1,
            modulus: 1,
        },
        CheckSpec {
            a: vec![vec![dig(0)]],
            b: vec![vec![ex(S_, 1)]],
            pc: vec![1],
            lin: vec![ex(Z_, -1)],
            cst: 0,
            modulus: 1,
        },
        CheckSpec {
            a: vec![vec![dig(1)]],
            b: vec![vec![ex(S_, 1)]],
            pc: vec![1],
            lin: vec![ex(R_, -1)],
            cst: 0,
            modulus: 1,
        },
        // lambda (C.x - acc.x) - (C.y - acc.y) = 0.
        CheckSpec {
            a: vec![vec![ex(LAM_, 1)]],
            b: vec![vec![cst(&c.x, 1), fin(0, -1)]],
            pc: vec![1],
            lin: vec![cst(&c.y, -1), fin(1, 1)],
            cst: 0,
            modulus: 0,
        },
        // lambda^2 - acc.x - C.x - x = 0.
        CheckSpec {
            a: vec![vec![ex(LAM_, 1)]],
            b: vec![vec![ex(LAM_, 1)]],
            pc: vec![1],
            lin: vec![fin(0, -1), cst(&c.x, -1), ex(XP_, -1)],
            cst: 0,
            modulus: 0,
        },
        // acc.x (acc.x + C.x) - t2 = 0.
        CheckSpec {
            a: vec![vec![fin(0, 1)]],
            b: vec![vec![fin(0, 1), cst(&c.x, 1)]],
            pc: vec![1],
            lin: vec![ex(T2_, -1)],
            cst: 0,
            modulus: 0,
        },
        // lambda (acc.y + C.y) - t2 + 3 - C.x^2 = 0.
        CheckSpec {
            a: vec![vec![ex(LAM_, 1)]],
            b: vec![vec![fin(1, 1), cst(&c.y, 1)]],
            pc: vec![1],
            lin: vec![ex(T2_, -1), cst(&k3, 1)],
            cst: 0,
            modulus: 0,
        },
    ]
}

/// The comb's arrays (family `fam`, one instance per signature): value, the
/// public key's selections, one-product checks, four-product checks, digits
/// and integer additions; then the range table and G's point table (global
/// family `gfam`), G's selections, and the public-key table (global; a
/// witness table for the wallet row or explicitly fixed circuit material).
pub struct Comb {
    pub arrays: Vec<Array<Fp>>,
    pub tables: Vec<(usize, u32)>,
    /// `P = u1 G + u2 Q` per signature.
    pub results: Vec<Point>,
    pub offset: Point,
    pub check_counts: (usize, usize, usize),
    pub shapes: (CheckShape, CheckShape),
}

pub const OFFSET_SEED: u64 = 0x0bad_cafe;

/// Array offsets from `first`.
pub const A_VALUE: usize = 0;
pub const A_SEL: usize = 1;
pub const A_CHK1: usize = 2;
pub const A_CHK4: usize = 3;
pub const A_DIGITS: usize = 4;
pub const A_ADD: usize = 5;
pub const A_RANGE: usize = 6;
pub const A_TABLE: usize = 7;
pub const A_SELG: usize = 8;
/// The family's public-key table; the default comb uses the wallet row.
pub const A_WTABLE: usize = 9;

/// The wallet table's cells per row: the row (`256 t + index`), then x's and
/// y's limbs.
pub const WALLET_CELLS: usize = 7;
/// The wallet table's rows (two tables of 256 entries).
pub const WALLET_ROWS: usize = 512;

/// The wallet table's values, cell-major (`WALLET_CELLS` vectors of
/// `WALLET_ROWS`): the wallet key's two comb tables (m = 16).
pub fn wallet_table_values(q: &Point) -> Vec<Vec<Fp>> {
    let qt = crate::operation::comb_tables(q, 16);
    let mut vals = vec![vec![Fp::from(0u64); WALLET_ROWS]; WALLET_CELLS];
    for (t, entries) in qt.iter().enumerate() {
        for (idx, pt) in entries.iter().enumerate() {
            let row = t * 256 + idx;
            vals[0][row] = Fp::from(row as u64);
            let (xl, yl) = (limbs(&pt.x), limbs(&pt.y));
            for k in 0..3 {
                vals[1 + k][row] = fp_u(&xl[k]);
                vals[4 + k][row] = fp_u(&yl[k]);
            }
        }
    }
    vals
}

/// Digit bit `K` of a step sits in accumulator limb `ACC_LIMB[K]` with
/// weight `2^(16 K - 88 ACC_LIMB[K])`.
fn acc_limb(k: usize) -> usize {
    (16 * k) / L
}

/// Builds the wallet comb from array index `first` for `(Q, z, r, s)` per
/// signature, using the wallet witness table (`WALLET_TABLE`). The caller
/// authenticates its commitment. Every input in this family has the same
/// public key.
pub fn comb(
    first: usize,
    fam: usize,
    pi: usize,
    gfam: usize,
    inputs: &[(Point, BigUint, BigUint, BigUint)],
) -> Comb {
    comb_with_key_table(
        first,
        fam,
        pi,
        gfam,
        inputs,
        KeyTable::Witness { id: WALLET_TABLE },
    )
}

/// Builds one signature family with its own public-key table. Compose
/// separate families with distinct table ids to check signatures from
/// different keys in the same circuit. This does not check a signer
/// certificate or bind a witness table to an authority; the caller must do
/// so before using a witness table for issuer or root signatures.
pub fn comb_with_key_table(
    first: usize,
    fam: usize,
    pi: usize,
    gfam: usize,
    inputs: &[(Point, BigUint, BigUint, BigUint)],
    key_table: KeyTable,
) -> Comb {
    assert!(
        !inputs.is_empty(),
        "a comb family needs at least one signature"
    );
    let (key_table_id, key_table_fixed) = match key_table {
        KeyTable::Witness { id } => (id, false),
        KeyTable::Fixed { id } => (id, true),
    };
    assert!(
        key_table_id != RANGE_TABLE
            && key_table_id != POINT_TABLE
            && key_table_id < crate::uniform::COPY_TABLE,
        "a public-key table must not alias a shared or reserved table"
    );
    let p = p256::p();
    let nn = p256::n();
    let m = 16usize;
    let c = 256 / m;
    assert_eq!(c, STEPS);
    let (one_specs, four_specs) = step_checks();
    let offset = p256::mul(&p256::generator(), &BigUint::from(OFFSET_SEED)).unwrap();
    let cpt = p256::neg(&p256::mul(&offset, &pow2(STEPS)).unwrap());
    let extra_specs = extra_checks(&cpt);
    let at = |k: usize| first + k;
    let (value_a, sel_a, digits_a) = (at(A_VALUE), at(A_SEL), at(A_DIGITS));
    let moduli = vec![p.clone(), nn.clone()];
    let s1 = CheckShape::new(
        &[(max_bound(&sources(2, 1), &u_bound()), sources(2, 1))],
        &sources(3, 2),
        3,
        &moduli,
    );
    let s4 = CheckShape::new(
        &[
            (sources(1, 1), sources(2, 1)),
            (sources(1, 1), sources(1, 1)),
            (sources(1, 1), sources(1, 1)),
            (sources(1, 1), sources(1, 1)),
        ],
        &sources(3, 2),
        3,
        &moduli[..1],
    );
    let slots = 1usize << pi;
    let gt = crate::operation::comb_tables(&p256::generator(), m);
    let qt = crate::operation::comb_tables(&inputs[0].0, m);
    for (q, _, _, _) in inputs {
        assert_eq!(q, &inputs[0].0, "one public key per comb family");
    }
    let tables: Vec<&Vec<Point>> = vec![&qt[0], &qt[1], &gt[0], &gt[1]];
    let selg_a = at(A_SELG);
    // Selection j reads scalar u2 (digit slot 1) for the key's tables and u1
    // (slot 0) for G's; its index bits are the digits' K = 8 (j % 2) + k.
    let scalar_of = |j: usize| if j < 2 { 1usize } else { 0 };
    let (rho_v, rho_s, rho_1, rho_4, rho_d, rho_a) =
        (8usize, 5usize, 8usize, 6usize, 5usize, 2usize);
    let n1 = one_specs.len() * STEPS + extra_specs.len();
    let vcount = value_cells_count();
    let mut value = vec![vec![Fp::from(0u64); slots << rho_v]; vcount];
    let mut sel = vec![vec![Fp::from(0u64); slots << rho_s]; 6];
    let mut selg = vec![vec![Fp::from(0u64); slots << rho_s]; 6];
    let mut chk1 = vec![vec![Fp::from(0u64); slots << rho_1]; s1.cells];
    let mut chk4 = vec![vec![Fp::from(0u64); slots << rho_4]; s4.cells];
    let mut digits = vec![vec![Fp::from(0u64); slots << rho_d]; 16 + 3];
    let mut add = vec![vec![Fp::from(0u64); slots << rho_a]; 3];
    let mut results = Vec::new();
    let lim = |x: &BigUint| -> [BigInt; 3] { limbs(x).map(BigInt::from) };
    let all_ones = limbs(&(pow2(256) - 1u32));
    for (sig, (q, z, r, s)) in inputs.iter().enumerate() {
        assert!(
            p256::verify(
                q,
                z,
                &p256::Signature {
                    r: r.clone(),
                    s: s.clone()
                }
            ),
            "a valid signature"
        );
        let w = p256::inv(s, &nn);
        let (u1, u2) = (z * &w % &nn, r * &w % &nn);
        // Digits: bit t = 16 K + i of u at (scalar, step = 15 - i), cell K,
        // and the Horner limbs acc(step) = 2 acc(step - 1) + sum_K b 2^(..).
        let mut uacc: Vec<[BigUint; 3]> = Vec::new();
        for (scalar, u) in [&u1, &u2].into_iter().enumerate() {
            let b = comb_bits(u);
            let mut acc: [BigUint; 3] = Default::default();
            for step in 0..STEPS {
                let i = STEPS - 1 - step;
                let rr = (sig << rho_d) | (scalar << 4) | step;
                for limb in acc.iter_mut() {
                    *limb <<= 1;
                }
                for k in 0..16 {
                    let bit = b[16 * k + i];
                    digits[k][rr] = Fp::from(bit as u64);
                    if bit {
                        acc[acc_limb(k)] += pow2(16 * k - L * acc_limb(k));
                    }
                }
                for l in 0..3 {
                    assert!(
                        bits(&acc[l]) <= ACC_BITS[l],
                        "digit accumulator within its width"
                    );
                    digits[16 + l][rr] = fp_u(&acc[l]);
                }
            }
            let bsum: BigUint = (0..3).map(|l| &acc[l] << (L * l)).sum();
            let bval: BigUint = (0..256).filter(|t| b[*t]).map(pow2).sum();
            assert_eq!(bsum, bval, "Horner limbs recompose B");
            uacc.push(acc);
        }
        let (b1, b2) = (comb_bits(&u1), comb_bits(&u2));
        let index = |bits: &[bool], j: usize, i: usize| -> usize {
            (0..8)
                .filter(|k| bits[c * (8 * j + k) + i])
                .map(|k| 1usize << k)
                .sum()
        };
        let mut accp = offset.clone();
        for (step, i) in (0..c).rev().enumerate() {
            let idx = [
                index(&b2, 0, i),
                index(&b2, 1, i),
                index(&b1, 0, i),
                index(&b1, 1, i),
            ];
            let e: Vec<Point> = (0..4).map(|j| tables[j][idx[j]].clone()).collect();
            for j in 0..4 {
                let rr = (sig << rho_s) | ((j % 2) << 4) | step;
                let store = if j < 2 { &mut sel } else { &mut selg };
                let (xl, yl) = (limbs(&e[j].x), limbs(&e[j].y));
                for k in 0..3 {
                    store[k][rr] = fp_u(&xl[k]);
                    store[3 + k][rr] = fp_u(&yl[k]);
                }
            }
            // double_add(acc, e0) = 2 acc + e0, then + e1, e2, e3.
            let l1 = slope(&accp, &e[0]);
            let rp = p256::add(&accp, &e[0]);
            let l2 = slope(&rp, &accp);
            let out = p256::add(&rp, &accp);
            let mut vals: Vec<BigUint> = vec![l1, rp.x.clone(), l2, out.x.clone(), out.y.clone()];
            let mut a = out;
            for ej in &e[1..] {
                let l = slope(&a, ej);
                let sum = p256::add(&a, ej);
                vals.extend([l, sum.x.clone(), sum.y.clone()]);
                a = sum;
            }
            assert_eq!(vals.len(), VALUE_SLOTS);
            for (slot, x) in vals.iter().enumerate() {
                let rr = (sig << rho_v) | (slot << 4) | step;
                for (k, cell) in value_cells(x).into_iter().enumerate() {
                    value[k][rr] = cell;
                }
            }
            let src_val = |s: &Src| -> [BigInt; 3] {
                let l = match &s.kind {
                    Kind::Value => lim(&vals[s.slot]),
                    Kind::Sel(xy) => lim(if *xy == 0 { &e[s.slot].x } else { &e[s.slot].y }),
                    Kind::Acc => lim(if s.slot == 0 { &accp.x } else { &accp.y }),
                    other => unreachable!("{other:?} in a step check"),
                };
                l.map(|x| x * BigInt::from(s.coef))
            };
            let combine = |srcs: &[Src]| -> [BigInt; 3] {
                let mut out: [BigInt; 3] = Default::default();
                for s in srcs {
                    let x = src_val(s);
                    for k in 0..3 {
                        out[k] += &x[k];
                    }
                }
                out
            };
            for (shape, specs, store, rho) in [
                (&s1, &one_specs, &mut chk1, rho_1),
                (&s4, &four_specs, &mut chk4, rho_4),
            ] {
                for (slot, spec) in specs.iter().enumerate() {
                    let av: Vec<[BigInt; 3]> = spec.a.iter().map(|s| combine(s)).collect();
                    let bv: Vec<[BigInt; 3]> = spec.b.iter().map(|s| combine(s)).collect();
                    let cells = shape.witness(
                        &av,
                        &bv,
                        &spec.pc,
                        &combine(&spec.lin),
                        spec.cst,
                        spec.modulus,
                    );
                    let rr = (sig << rho) | (slot << 4) | step;
                    for (k, cell) in cells.into_iter().enumerate() {
                        store[k][rr] = cell;
                    }
                }
            }
            accp = a;
        }
        // The final addition P = acc + C and the per-signature values.
        let lam = slope(&accp, &cpt);
        let pt =
            p256::add_any(Some(accp.clone()), Some(cpt.clone())).expect("P is not the identity");
        let expect = p256::add_any(p256::mul(&p256::generator(), &u1), p256::mul(q, &u2)).unwrap();
        assert_eq!(pt, expect, "comb computes u1 G + u2 Q");
        assert_eq!(&pt.x % &nn, *r, "x(P) = r (mod n)");
        let t2 = &accp.x * ((&accp.x + &cpt.x) % &p) % &p;
        let extra: [BigUint; 10] = [
            r.clone(),
            s.clone(),
            z.clone(),
            p256::inv(r, &nn),
            &nn - 1u32 - r,
            &nn - 1u32 - s,
            &p - 1u32 - &pt.x,
            lam,
            pt.x.clone(),
            t2,
        ];
        for (k, x) in extra.iter().enumerate() {
            let rr = (sig << rho_v) | (EXTRA_SLOT << 4) | k;
            for (cell, val) in value_cells(x).into_iter().enumerate() {
                value[cell][rr] = val;
            }
        }
        let xsrc = |s: &Src| -> [BigInt; 3] {
            let l = match &s.kind {
                Kind::Extra => lim(&extra[s.slot]),
                Kind::Final => lim(if s.slot == 0 { &accp.x } else { &accp.y }),
                Kind::Digits => {
                    let acc = &uacc[s.slot];
                    [0, 1, 2].map(|k| {
                        BigInt::from(2u32) * BigInt::from(acc[k].clone())
                            - BigInt::from(all_ones[k].clone())
                    })
                }
                Kind::Const(x) => lim(x),
                other => unreachable!("{other:?} in a signature check"),
            };
            l.map(|x| x * BigInt::from(s.coef))
        };
        let xcombine = |srcs: &[Src]| -> [BigInt; 3] {
            let mut out: [BigInt; 3] = Default::default();
            for s in srcs {
                let x = xsrc(s);
                for k in 0..3 {
                    out[k] += &x[k];
                }
            }
            out
        };
        for (e_ix, spec) in extra_specs.iter().enumerate() {
            let av: Vec<[BigInt; 3]> = spec.a.iter().map(|s| xcombine(s)).collect();
            let bv: Vec<[BigInt; 3]> = spec.b.iter().map(|s| xcombine(s)).collect();
            let cells = s1.witness(
                &av,
                &bv,
                &spec.pc,
                &xcombine(&spec.lin),
                spec.cst,
                spec.modulus,
            );
            let rr = (sig << rho_1) | (one_specs.len() * STEPS + e_ix);
            for (k, cell) in cells.into_iter().enumerate() {
                chk1[k][rr] = cell;
            }
        }
        // Integer additions a + b = c: carries k1, k2 and the flag.
        let flag = pt.x >= nn;
        let adds: [(BigUint, BigUint, BigUint); 4] = [
            (r.clone(), &nn - 1u32 - r, &nn - 1u32),
            (s.clone(), &nn - 1u32 - s, &nn - 1u32),
            (pt.x.clone(), &p - 1u32 - &pt.x, &p - 1u32),
            (
                r.clone(),
                if flag { nn.clone() } else { BigUint::zero() },
                pt.x.clone(),
            ),
        ];
        for (ix, (a, b, cc)) in adds.iter().enumerate() {
            assert_eq!(a + b, cc.clone(), "integer addition {ix}");
            let (al, bl, cl) = (limbs(a), limbs(b), limbs(cc));
            let k1 = (&al[0] + &bl[0]) >> L;
            let k2 = (&al[1] + &bl[1] + &k1) >> L;
            let rr = (sig << rho_a) | ix;
            add[0][rr] = fp_u(&k1);
            add[1][rr] = fp_u(&k2);
            add[2][rr] = Fp::from((ix == 3 && flag) as u64);
            assert_eq!(&al[2] + &bl[2] + &k2, cl[2], "top limb without carry");
        }
        results.push(pt);
    }
    // Check instances: (r, spec, step) for the steps' checks, then the extra
    // checks at r = 14 * 16 + e (no step).
    let instances =
        |specs: &[CheckSpec], extra: &[CheckSpec]| -> Vec<(usize, CheckSpec, Option<usize>)> {
            let mut out = Vec::new();
            for (slot, spec) in specs.iter().enumerate() {
                for step in 0..STEPS {
                    out.push(((slot << 4) | step, spec.clone(), Some(step)));
                }
            }
            for (e_ix, spec) in extra.iter().enumerate() {
                out.push((specs.len() * STEPS + e_ix, spec.clone(), None));
            }
            out
        };
    // A port: per source position, one link per kind and the constants.
    let port = |inst: &[(usize, CheckSpec, Option<usize>)],
                get: &dyn Fn(&CheckSpec) -> Vec<Src>,
                rho: usize|
     -> Port {
        let n = 1usize << rho;
        let zero = Fp::from(0u64);
        let width = inst
            .iter()
            .map(|(_, sp, _)| get(sp).len())
            .max()
            .unwrap_or(0);
        let mut out = Port {
            links: Vec::new(),
            consts: [vec![zero; n], vec![zero; n], vec![zero; n]],
        };
        for q in 0..width {
            for class in 0..9 {
                let target = match class {
                    0 | 3 | 4 | 5 => (value_a, 0),
                    1 => (sel_a, 0),
                    2 => (sel_a, 3),
                    7 => (selg_a, 0),
                    8 => (selg_a, 3),
                    _ => (digits_a, 16),
                };
                let mut map = vec![None; n];
                let mut coef = vec![zero; n];
                for (r, sp, step) in inst {
                    let list = get(sp);
                    let Some(s) = list.get(q) else { continue };
                    let to = match (&s.kind, class, step) {
                        (Kind::Value, 0, Some(st)) => (s.slot << 4) | st,
                        (Kind::Sel(0), 1, Some(st)) | (Kind::Sel(1), 2, Some(st)) if s.slot < 2 => {
                            (s.slot << 4) | st
                        }
                        (Kind::Sel(0), 7, Some(st)) | (Kind::Sel(1), 8, Some(st))
                            if s.slot >= 2 =>
                        {
                            ((s.slot - 2) << 4) | st
                        }
                        (Kind::Acc, 3, Some(st)) if *st > 0 => ((FINAL_X + s.slot) << 4) | (st - 1),
                        (Kind::Final, 4, None) => ((FINAL_X + s.slot) << 4) | (STEPS - 1),
                        (Kind::Extra, 5, None) => (EXTRA_SLOT << 4) | s.slot,
                        (Kind::Digits, 6, None) => (s.slot << 4) | (STEPS - 1),
                        _ => continue,
                    };
                    map[*r] = Some(to);
                    coef[*r] = fp_i(&BigInt::from(if class == 6 { 2 * s.coef } else { s.coef }));
                }
                if map.iter().any(|m| m.is_some()) {
                    out.links.push((target.0, target.1, map, coef));
                }
            }
            for (r, sp, step) in inst {
                let list = get(sp);
                let Some(s) = list.get(q) else { continue };
                let cl: Option<[BigInt; 3]> = match (&s.kind, step) {
                    (Kind::Acc, Some(0)) => {
                        Some(lim(if s.slot == 0 { &offset.x } else { &offset.y }))
                    }
                    (Kind::Const(x), _) => Some(lim(x)),
                    (Kind::Digits, _) => Some(all_ones.clone().map(|x| -BigInt::from(x))),
                    _ => None,
                };
                if let Some(cl) = cl {
                    for k in 0..3 {
                        out.consts[k][*r] += fp_i(&(&cl[k] * BigInt::from(s.coef)));
                    }
                }
            }
        }
        out
    };
    let build_checks = |shape: &CheckShape,
                        inst: &[(usize, CheckSpec, Option<usize>)],
                        rho: usize|
     -> (Vec<[Lin<Fp>; 3]>, Vec<(u32, Vec<Lin<Fp>>)>) {
        let n = 1usize << rho;
        let a: Vec<Port> = (0..shape.products)
            .map(|i| port(inst, &|sp| sp.a[i].clone(), rho))
            .collect();
        let b: Vec<Port> = (0..shape.products)
            .map(|i| port(inst, &|sp| sp.b[i].clone(), rho))
            .collect();
        let lin = port(inst, &|sp| sp.lin.clone(), rho);
        let mut pc = vec![vec![Fp::from(0u64); n]; shape.products];
        let mut cst = vec![Fp::from(0u64); n];
        let mut mod_of = vec![0usize; n];
        for (r, sp, _) in inst {
            for i in 0..shape.products {
                pc[i][*r] = fp_i(&BigInt::from(sp.pc[i]));
            }
            cst[*r] = fp_i(&BigInt::from(sp.cst));
            mod_of[*r] = sp.modulus;
        }
        shape.template(&a, &b, &pc, &lin, &cst, &mod_of, n)
    };
    let (r1, l1) = build_checks(&s1, &instances(&one_specs, &extra_specs), rho_1);
    let (r4, l4) = build_checks(&s4, &instances(&four_specs, &[]), rho_4);
    let (vr, vl) = value_template(1 << rho_v);
    // Selections: (the table's row 256 t + index from the digit bits, limbs),
    // the wallet key's tables (j = 0, 1) in the wallet table, G's (j = 2, 3)
    // in G's table; instance (t << 4) | step with t = j % 2.
    let sel_lookup = |half: usize, table: u32| {
        let ns = 1usize << rho_s;
        let mut index: Lin<Fp> = Vec::new();
        for k in 0..16 {
            let mut map = vec![None; ns];
            for tt in 0..2 {
                let j = 2 * half + tt;
                if k / 8 == tt {
                    for step in 0..STEPS {
                        map[(tt << 4) | step] = Some((scalar_of(j) << 4) | step);
                    }
                }
            }
            index.push(Term {
                src: USrc::Link {
                    array: digits_a,
                    cell: k,
                    map,
                },
                coef: vec![Fp::from(1u64 << (k % 8)); ns],
            });
        }
        let mut t = vec![plus(
            &constant((0..ns).map(|r| Fp::from(256 * (r >> 4) as u64)).collect()),
            &compact(index),
        )];
        for k in 0..6 {
            t.push(own(k, ns));
        }
        (table, t)
    };
    // Digits: booleanity and the Horner rows.
    let (dr, dwidths) = {
        let nd = 1usize << rho_d;
        let one = Fp::from(1u64);
        let mut rows: Vec<[Lin<Fp>; 3]> = (0..16)
            .map(|k| [own(k, nd), own(k, nd), own(k, nd)])
            .collect();
        let prev: Vec<Option<usize>> = (0..nd)
            .map(|r| if r & 15 > 0 { Some(r - 1) } else { None })
            .collect();
        for l in 0..3 {
            let mut row = plus(
                &own(16 + l, nd),
                &vec![Term {
                    src: USrc::Link {
                        array: digits_a,
                        cell: 16 + l,
                        map: prev.clone(),
                    },
                    coef: vec![-Fp::from(2u64); nd],
                }],
            );
            for k in (0..16).filter(|k| acc_limb(*k) == l) {
                row = plus(&row, &scaled(&own(k, nd), -fp_u(&pow2(16 * k - L * l))));
            }
            rows.push([row, constant(vec![one; nd]), Vec::new()]);
        }
        let widths: Vec<u16> = std::iter::repeat_n(1u16, 16)
            .chain(ACC_BITS.iter().map(|b| *b as u16))
            .collect();
        (rows, widths)
    };
    // Integer additions a + b = c with boolean carries; the last one adds
    // flag * n and checks x(P).
    let ar = {
        let na = 1usize << rho_a;
        let one = Fp::from(1u64);
        let vlink = |cells: [Option<usize>; 4], k: usize| -> Lin<Fp> {
            let map: Vec<Option<usize>> = cells
                .iter()
                .map(|c| c.map(|e| (EXTRA_SLOT << 4) | e))
                .collect();
            vec![Term {
                src: USrc::Link {
                    array: value_a,
                    cell: k,
                    map,
                },
                coef: vec![one; na],
            }]
        };
        let (nl, pl) = (limbs(&(&nn - 1u32)), limbs(&(&p - 1u32)));
        let nlimbs = limbs(&nn);
        let side = |k: usize| -> (Lin<Fp>, Lin<Fp>, Lin<Fp>) {
            let a = vlink([Some(R_), Some(S_), Some(XP_), Some(R_)], k);
            let b = plus(
                &vlink([Some(DR_), Some(DS_), Some(DX_), None], k),
                &vec![Term {
                    src: USrc::Own(2),
                    coef: vec![
                        Fp::from(0u64),
                        Fp::from(0u64),
                        Fp::from(0u64),
                        fp_u(&nlimbs[k]),
                    ],
                }],
            );
            let c = plus(
                &constant(vec![
                    fp_u(&nl[k]),
                    fp_u(&nl[k]),
                    fp_u(&pl[k]),
                    Fp::from(0u64),
                ]),
                &vlink([None, None, None, Some(XP_)], k),
            );
            (a, b, c)
        };
        let shift = fp_u(&pow2(L));
        let mut rows: Vec<[Lin<Fp>; 3]> = Vec::new();
        for k in 0..3 {
            let (a, b, c) = side(k);
            let mut row = plus(&plus(&a, &b), &scaled(&c, -one));
            if k > 0 {
                row = plus(&row, &own(k - 1, na));
            }
            if k < 2 {
                row = plus(&row, &scaled(&own(k, na), -shift));
            }
            rows.push([row, constant(vec![one; na]), Vec::new()]);
        }
        for cell in 0..3 {
            rows.push([own(cell, na), own(cell, na), own(cell, na)]);
        }
        rows
    };
    // G's table: rows 256 t + index for G's two tables.
    let mut table_vals = vec![vec![Fp::from(0u64); WALLET_ROWS]; WALLET_CELLS];
    for (t, entries) in tables[2..].iter().enumerate() {
        for (idx, pt) in entries.iter().enumerate() {
            let row = t * 256 + idx;
            table_vals[0][row] = Fp::from(row as u64);
            let (xl, yl) = (limbs(&pt.x), limbs(&pt.y));
            for k in 0..3 {
                table_vals[1 + k][row] = fp_u(&xl[k]);
                table_vals[4 + k][row] = fp_u(&yl[k]);
            }
        }
    }
    let arrays = vec![
        Array {
            name: "comb value",
            family: fam,
            rho: rho_v,
            r_count: (EXTRA_SLOT << 4) + EXTRA.len(),
            fixed: false,
            lookups: vl,
            widths: value_widths(),
            rows: vr,
            values: value,
        },
        Array {
            name: "comb selection",
            family: fam,
            rho: rho_s,
            r_count: 2 * STEPS,
            fixed: false,
            lookups: vec![sel_lookup(0, key_table_id)],
            widths: vec![88, 88, 80, 88, 88, 80],
            rows: Vec::new(),
            values: sel,
        },
        Array {
            name: "comb check 1",
            family: fam,
            rho: rho_1,
            r_count: n1,
            fixed: false,
            lookups: l1,
            widths: s1.widths(),
            rows: r1,
            values: chk1,
        },
        Array {
            name: "comb check 4",
            family: fam,
            rho: rho_4,
            r_count: four_specs.len() * STEPS,
            fixed: false,
            lookups: l4,
            widths: s4.widths(),
            rows: r4,
            values: chk4,
        },
        Array {
            name: "comb digits",
            family: fam,
            rho: rho_d,
            r_count: 2 * STEPS,
            fixed: false,
            lookups: Vec::new(),
            widths: dwidths,
            rows: dr,
            values: digits,
        },
        Array {
            name: "comb integer add",
            family: fam,
            rho: rho_a,
            r_count: 4,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1, 1, 1],
            rows: ar,
            values: add,
        },
        Array {
            name: "range table",
            family: gfam,
            rho: CHUNK,
            r_count: 1 << CHUNK,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![CHUNK as u16],
            rows: Vec::new(),
            values: vec![(0..1u64 << CHUNK).map(Fp::from).collect()],
        },
        Array {
            name: "point table",
            family: gfam,
            rho: 9,
            r_count: WALLET_ROWS,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![9, 88, 88, 80, 88, 88, 80],
            rows: Vec::new(),
            values: table_vals,
        },
        Array {
            name: "comb selection G",
            family: fam,
            rho: rho_s,
            r_count: 2 * STEPS,
            fixed: false,
            lookups: vec![sel_lookup(1, POINT_TABLE)],
            widths: vec![88, 88, 80, 88, 88, 80],
            rows: Vec::new(),
            values: selg,
        },
        Array {
            name: if key_table_id == WALLET_TABLE && !key_table_fixed {
                "wallet table"
            } else {
                "public-key table"
            },
            family: gfam,
            rho: 9,
            r_count: WALLET_ROWS,
            fixed: key_table_fixed,
            lookups: Vec::new(),
            widths: vec![9, 88, 88, 80, 88, 88, 80],
            rows: Vec::new(),
            values: wallet_table_values(&inputs[0].0),
        },
    ];
    Comb {
        arrays,
        tables: vec![
            (at(A_RANGE), RANGE_TABLE),
            (at(A_TABLE), POINT_TABLE),
            (at(A_WTABLE), key_table_id),
        ],
        results,
        offset,
        check_counts: (one_specs.len(), four_specs.len(), extra_specs.len()),
        shapes: (s1, s4),
    }
}

/// Test and measurement inputs: one wallet key, `count` real signatures
/// `(Q, z, r, s)` on 256-bit digests.
pub fn sample_inputs(count: usize) -> Vec<(Point, BigUint, BigUint, BigUint)> {
    let n = p256::n();
    let d = p256::scalar_below("comb-wallet-key", &n);
    let q = p256::mul(&p256::generator(), &d).unwrap();
    (0..count)
        .map(|k| {
            let z = p256::scalar_below(&format!("comb-digest-{k}"), &pow2(256));
            let nonce = p256::scalar_below(&format!("comb-nonce-{k}"), &n);
            let sig = p256::sign(&d, &z, &nonce);
            (q.clone(), z, sig.r, sig.s)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Stage A2, step 4: the cross-circuit binding chain, Horner over Fq in the Fp
// circuit: `acc' = acc rho + v (mod q)` from the mask `acc_0 = mu`, with acc
// and rho non-native Fq values and each shared value v a native value below
// 2^128 (bound to its source by a copy lookup in step 6).

/// The Pallas scalar modulus q (Fq).
pub fn fq_modulus() -> BigUint {
    BigUint::parse_bytes(
        b"40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001",
        16,
    )
    .unwrap()
}

/// Array offsets from `first`.
pub const B_VALUE: usize = 0;
pub const B_CHECK: usize = 1;
pub const B_RANGE: usize = 2;
pub const B_SHARED: usize = 3;

/// Limb widths of the accumulators and rho (values below 2^255).
const FQ_LIMBS: [usize; 3] = [L, L, 255 - 2 * L];

pub struct Binding {
    pub arrays: Vec<Array<Fp>>,
    pub tables: Vec<(usize, u32)>,
    /// R = acc_n.
    pub result: BigUint,
    pub shape: CheckShape,
}

/// The chain from array index `first`: family `fam` holds the one chain
/// (value instances acc_0 = mu .. acc_n, then rho; one check per shared
/// value; the shared values themselves, below 2^128 by range-checked chunks,
/// as the non-native check's bound assumes), the range table is in global
/// family `gfam`.
pub fn binding_chain(
    first: usize,
    fam: usize,
    gfam: usize,
    rho: &BigUint,
    mu: &BigUint,
    values: &[u128],
) -> Binding {
    let q = fq_modulus();
    let n = values.len();
    let rho_bits = (n + 2).next_power_of_two().trailing_zeros() as usize;
    let nn = 1usize << rho_bits;
    let value_a = first + B_VALUE;
    let one = Fp::from(1u64);
    let zero = Fp::from(0u64);
    let chunks = FQ_LIMBS.map(chunk_widths);
    let vcount = 3 + chunks.iter().map(|c| c.len()).sum::<usize>();
    let mut accs = vec![mu.clone()];
    for v in values {
        let next = (accs.last().unwrap() * rho + BigUint::from(*v)) % &q;
        accs.push(next);
    }
    // Values: three limbs and their chunks.
    let mut value = vec![vec![zero; nn]; vcount];
    let fill = |value: &mut Vec<Vec<Fp>>, r: usize, x: &BigUint| {
        assert!(x.bits() <= 255, "an Fq value below 2^255");
        let l = limbs(x);
        let mut k = 3;
        for i in 0..3 {
            value[i][r] = fp_u(&l[i]);
            for c in chunk_values(&l[i], &chunks[i]) {
                value[k][r] = c;
                k += 1;
            }
        }
    };
    for (r, x) in accs.iter().enumerate() {
        fill(&mut value, r, x);
    }
    fill(&mut value, n + 1, rho);
    let (mut vr, mut vl) = (Vec::new(), Vec::new());
    let mut first_chunk = 3;
    for (k, wk) in chunks.iter().enumerate() {
        vr.push([
            plus(&own(k, nn), &scaled(&recompose(first_chunk, wk, nn), -one)),
            constant(vec![one; nn]),
            Vec::new(),
        ]);
        for (c, w) in wk.iter().enumerate() {
            range_lookups(&mut vl, first_chunk + c, *w, nn);
        }
        first_chunk += wk.len();
    }
    // Checks: acc_r rho + v_r - acc_(r+1) = 0 (mod q), v_r read from the
    // shared array.
    let lb = |k: usize| pow2(FQ_LIMBS[k]) - 1u32;
    let acc_bound: Bound = [lb(0), lb(1), lb(2)];
    let lin_bound: Bound = [lb(0) + pow2(128), lb(1), lb(2)];
    let shape = CheckShape::new(
        &[(acc_bound.clone(), acc_bound)],
        &lin_bound,
        0,
        std::slice::from_ref(&q),
    );
    let port = |g: &dyn Fn(usize) -> usize| Port {
        links: vec![(
            value_a,
            0,
            (0..nn).map(|r| (r < n).then(|| g(r))).collect(),
            vec![one; nn],
        )],
        consts: [vec![zero; nn], vec![zero; nn], vec![zero; nn]],
    };
    let next = |k: usize| {
        link(
            value_a,
            k,
            (0..nn).map(|r| (r < n).then_some(r + 1)).collect(),
            -one,
        )
    };
    let shared = link(
        first + B_SHARED,
        0,
        (0..nn).map(|r| (r < n).then_some(r)).collect(),
        one,
    );
    let lin = [plus(&shared, &next(0)), next(1), next(2)];
    let (cr, cl) = shape.template_lin(
        &[port(&|r| r)],
        &[port(&|_| n + 1)],
        &[vec![one; nn]],
        &lin,
        &vec![zero; nn],
        &vec![0; nn],
        nn,
    );
    let mut check = vec![vec![zero; nn]; shape.cells];
    let lim = |x: &BigUint| -> [BigInt; 3] { limbs(x).map(BigInt::from) };
    for (r, v) in values.iter().enumerate() {
        let nxt = lim(&accs[r + 1]);
        let linv = [BigInt::from(*v) - &nxt[0], -nxt[1].clone(), -nxt[2].clone()];
        for (k, cell) in shape
            .witness(&[lim(&accs[r])], &[lim(rho)], &[1], &linv, 0, 0)
            .into_iter()
            .enumerate()
        {
            check[k][r] = cell;
        }
    }
    // Shared values: v and its 128-bit chunks.
    let vchunks = chunk_widths(128);
    let mut shv = vec![vec![zero; nn]; 1 + vchunks.len()];
    for (r, v) in values.iter().enumerate() {
        let x = BigUint::from(*v);
        shv[0][r] = fp_u(&x);
        for (c, val) in chunk_values(&x, &vchunks).into_iter().enumerate() {
            shv[1 + c][r] = val;
        }
    }
    let srows = vec![[
        plus(&own(0, nn), &scaled(&recompose(1, &vchunks, nn), -one)),
        constant(vec![one; nn]),
        Vec::new(),
    ]];
    let mut sl = Vec::new();
    for (c, w) in vchunks.iter().enumerate() {
        range_lookups(&mut sl, 1 + c, *w, nn);
    }
    let mut sw: Vec<u16> = vec![128];
    sw.extend(vchunks.iter().map(|w| *w as u16));
    let mut vw: Vec<u16> = FQ_LIMBS.iter().map(|w| *w as u16).collect();
    vw.extend(chunks.iter().flatten().map(|w| *w as u16));
    let cw = shape.widths();
    let arrays = vec![
        Array {
            name: "binding value",
            family: fam,
            rho: rho_bits,
            r_count: n + 2,
            fixed: false,
            lookups: vl,
            widths: vw,
            rows: vr,
            values: value,
        },
        Array {
            name: "binding check",
            family: fam,
            rho: rho_bits,
            r_count: n,
            fixed: false,
            lookups: cl,
            widths: cw,
            rows: cr,
            values: check,
        },
        Array {
            name: "range table",
            family: gfam,
            rho: CHUNK,
            r_count: 1 << CHUNK,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![CHUNK as u16],
            rows: Vec::new(),
            values: vec![(0..1u64 << CHUNK).map(Fp::from).collect()],
        },
        Array {
            name: "binding shared",
            family: fam,
            rho: rho_bits,
            r_count: n,
            fixed: false,
            lookups: sl,
            widths: sw,
            rows: srows,
            values: shv,
        },
    ];
    Binding {
        arrays,
        tables: vec![(first + B_RANGE, RANGE_TABLE)],
        result: accs[n].clone(),
        shape,
    }
}

/// The link range table's id: the range table again, for the link phase's
/// lookups. Their values follow rho, so their multiplicities are link-phase
/// cells too, committed after the first phase's (whose range table's
/// multiplicities the first phase commits).
pub const LINK_RANGE_TABLE: u32 = 4;

/// An Fq value (below q) as its 32-byte little-endian word.
pub fn fq_bytes(x: &BigUint) -> [u8; 32] {
    let b = x.to_bytes_le();
    assert!(b.len() <= 32, "a value below 2^256");
    let mut out = [0u8; 32];
    out[..b.len()].copy_from_slice(&b);
    out
}

/// Array offsets from `first` of the linked chain.
pub const BL_SHARED: usize = 0;
pub const BL_MU: usize = 1;
pub const BL_VALUE: usize = 2;
pub const BL_CHECK: usize = 3;
pub const BL_WORD: usize = 4;
pub const BL_RANGE: usize = 5;
pub const BL_LINK_RANGE: usize = 6;
/// The link's io arrays from `first + BL_IO`: rho's limbs (lo, hi), R's.
pub const BL_IO: usize = 7;

/// Widths of a word gadget's split of the middle limb: l1 = t + 2^40 u, so
/// that lo = l0 + 2^88 t and hi = u + 2^48 l2 (lo + 2^128 hi = the value).
const WORD_T: usize = 128 - L;
const WORD_U: usize = L - WORD_T;

/// A link phase's cells: (array, cell, instance, value).
pub type LinkCells = Vec<(usize, usize, usize, Fp)>;

/// The binding chain of `binding_chain` split by phase (the pair link's rho
/// follows the first phase's commitments): first-phase arrays hold the shared
/// values and the mask mu (three limbs); link-phase arrays hold the chain's
/// values (acc_0 = mu .. acc_n, then rho, as limbs and chunks), its checks,
/// and two word gadgets that read rho and R = acc_n as the link's io limbs
/// (lo, hi). The link phase's lookups use `LINK_RANGE_TABLE`.
pub struct LinkedBinding {
    pub arrays: Vec<Array<Fp>>,
    pub tables: Vec<(usize, u32)>,
    /// The link phase's arrays (absolute indices).
    pub link_arrays: Vec<usize>,
    /// The link's io arrays (absolute) and their io slots.
    pub io: Vec<(usize, usize)>,
    /// The link phase's cells from rho (a 32-byte word below 2^255).
    pub fill: Box<dyn Fn(&[u8; 32]) -> LinkCells + Send + Sync>,
    /// R's limb cells: acc_n's three limbs (88 bits each, low first).
    pub binding: Vec<(usize, usize, usize)>,
}

/// The linked chain's link-phase columns for one rho.
struct LinkedColumns {
    value: Vec<Vec<Fp>>,
    check: Vec<Vec<Fp>>,
    word: Vec<Vec<Fp>>,
}

#[allow(clippy::too_many_arguments)]
fn linked_columns(
    shape: &CheckShape,
    q: &BigUint,
    mu: &BigUint,
    values: &[u128],
    rho: &BigUint,
    nn: usize,
    chunks: &[Vec<usize>; 3],
    word_chunks: &[Vec<usize>; 2],
) -> LinkedColumns {
    let n = values.len();
    let zero = Fp::from(0u64);
    assert!(rho.bits() <= 255, "rho below 2^255");
    let mut accs = vec![mu.clone()];
    for v in values {
        let next = (accs.last().unwrap() * rho + BigUint::from(*v)) % q;
        accs.push(next);
    }
    let vcount = 3 + chunks.iter().map(|c| c.len()).sum::<usize>();
    let mut value = vec![vec![zero; nn]; vcount];
    let fill = |value: &mut Vec<Vec<Fp>>, r: usize, x: &BigUint| {
        let l = limbs(x);
        let mut k = 3;
        for i in 0..3 {
            value[i][r] = fp_u(&l[i]);
            for c in chunk_values(&l[i], &chunks[i]) {
                value[k][r] = c;
                k += 1;
            }
        }
    };
    for (r, x) in accs.iter().enumerate() {
        fill(&mut value, r, x);
    }
    fill(&mut value, n + 1, rho);
    let lim = |x: &BigUint| -> [BigInt; 3] { limbs(x).map(BigInt::from) };
    let mut check = vec![vec![zero; nn]; shape.cells];
    for (r, v) in values.iter().enumerate() {
        let nxt = lim(&accs[r + 1]);
        let linv = [BigInt::from(*v) - &nxt[0], -nxt[1].clone(), -nxt[2].clone()];
        for (k, cell) in shape
            .witness(&[lim(&accs[r])], &[lim(rho)], &[1], &linv, 0, 0)
            .into_iter()
            .enumerate()
        {
            check[k][r] = cell;
        }
    }
    let wcount = 2 + word_chunks.iter().map(|c| c.len()).sum::<usize>();
    let mut word = vec![vec![zero; 2]; wcount];
    for (i, x) in [rho, &accs[n]].into_iter().enumerate() {
        let l1 = &limbs(x)[1];
        let (t, u) = (l1 % pow2(WORD_T), l1 >> WORD_T);
        word[0][i] = fp_u(&t);
        word[1][i] = fp_u(&u);
        let mut k = 2;
        for (y, wc) in [(&t, &word_chunks[0]), (&u, &word_chunks[1])] {
            for c in chunk_values(y, wc) {
                word[k][i] = c;
                k += 1;
            }
        }
    }
    LinkedColumns { value, check, word }
}

/// The linked binding chain from array index `first` (family `fam`; the
/// range tables in global family `gfam`; the link's io arrays at io slots
/// `rho_io`, `rho_io + 1` (rho's lo, hi) and `r_io`, `r_io + 1` (R's)).
pub fn binding_chain_linked(
    first: usize,
    fam: usize,
    gfam: usize,
    mu: &BigUint,
    values: &[u128],
    (rho_io, r_io): (usize, usize),
) -> LinkedBinding {
    let q = fq_modulus();
    assert!(mu < &q, "mu below q");
    let n = values.len();
    assert!(n > 0, "a chain over shared values");
    let rho_bits = (n + 2).next_power_of_two().trailing_zeros() as usize;
    let nn = 1usize << rho_bits;
    let value_a = first + BL_VALUE;
    let (mu_a, word_a, io_a) = (first + BL_MU, first + BL_WORD, first + BL_IO);
    let one = Fp::from(1u64);
    let zero = Fp::from(0u64);
    let chunks = FQ_LIMBS.map(chunk_widths);
    let word_chunks = [chunk_widths(WORD_T), chunk_widths(WORD_U)];
    let link_table = |lookups: &mut Vec<(u32, Vec<Lin<Fp>>)>| {
        for (t, _) in lookups.iter_mut() {
            assert_eq!(*t, RANGE_TABLE, "the chain's lookups are range lookups");
            *t = LINK_RANGE_TABLE;
        }
    };
    // Values: limb recomposition, and instance 0 is the first phase's mu.
    let (mut vr, mut vl) = (Vec::new(), Vec::new());
    let mut first_chunk = 3;
    for (k, wk) in chunks.iter().enumerate() {
        vr.push([
            plus(&own(k, nn), &scaled(&recompose(first_chunk, wk, nn), -one)),
            constant(vec![one; nn]),
            Vec::new(),
        ]);
        for (c, w) in wk.iter().enumerate() {
            range_lookups(&mut vl, first_chunk + c, *w, nn);
        }
        first_chunk += wk.len();
    }
    let at0: Vec<Fp> = (0..nn).map(|r| if r == 0 { one } else { zero }).collect();
    for k in 0..3 {
        vr.push([
            plus(
                &vec![Term {
                    src: USrc::Own(k),
                    coef: at0.clone(),
                }],
                &link(
                    mu_a,
                    k,
                    (0..nn).map(|r| (r == 0).then_some(0)).collect(),
                    -one,
                ),
            ),
            constant(vec![one; nn]),
            Vec::new(),
        ]);
    }
    link_table(&mut vl);
    // Checks: acc_r rho + v_r - acc_(r+1) = 0 (mod q), v_r read from the
    // shared array.
    let lb = |k: usize| pow2(FQ_LIMBS[k]) - 1u32;
    let acc_bound: Bound = [lb(0), lb(1), lb(2)];
    let lin_bound: Bound = [lb(0) + pow2(128), lb(1), lb(2)];
    let shape = CheckShape::new(
        &[(acc_bound.clone(), acc_bound)],
        &lin_bound,
        0,
        std::slice::from_ref(&q),
    );
    let port = |g: &dyn Fn(usize) -> usize| Port {
        links: vec![(
            value_a,
            0,
            (0..nn).map(|r| (r < n).then(|| g(r))).collect(),
            vec![one; nn],
        )],
        consts: [vec![zero; nn], vec![zero; nn], vec![zero; nn]],
    };
    let next = |k: usize| {
        link(
            value_a,
            k,
            (0..nn).map(|r| (r < n).then_some(r + 1)).collect(),
            -one,
        )
    };
    let shared = link(
        first + BL_SHARED,
        0,
        (0..nn).map(|r| (r < n).then_some(r)).collect(),
        one,
    );
    let lin = [plus(&shared, &next(0)), next(1), next(2)];
    let (cr, mut cl) = shape.template_lin(
        &[port(&|r| r)],
        &[port(&|_| n + 1)],
        &[vec![one; nn]],
        &lin,
        &vec![zero; nn],
        &vec![0; nn],
        nn,
    );
    link_table(&mut cl);
    // Word gadgets: instance 0 reads rho (value instance n + 1) as the
    // link's io limbs (lo, hi), instance 1 R = acc_n (value instance n).
    let src = [Some(n + 1), Some(n)];
    let limb = |k: usize| link(value_a, k, src.to_vec(), one);
    let io_of = |j: usize| {
        plus(
            &link(io_a + j, 0, vec![Some(0), None], one),
            &link(io_a + 2 + j, 0, vec![None, Some(0)], one),
        )
    };
    let t_first = 2;
    let u_first = 2 + word_chunks[0].len();
    let mut wr = vec![
        [
            plus(
                &own(0, 2),
                &scaled(&recompose(t_first, &word_chunks[0], 2), -one),
            ),
            constant(vec![one; 2]),
            Vec::new(),
        ],
        [
            plus(
                &own(1, 2),
                &scaled(&recompose(u_first, &word_chunks[1], 2), -one),
            ),
            constant(vec![one; 2]),
            Vec::new(),
        ],
    ];
    // l1 = t + 2^40 u; lo = l0 + 2^88 t; hi = u + 2^48 l2.
    wr.push([
        plus(
            &limb(1),
            &plus(
                &scaled(&own(0, 2), -one),
                &scaled(&own(1, 2), -fp_u(&pow2(WORD_T))),
            ),
        ),
        constant(vec![one; 2]),
        Vec::new(),
    ]);
    wr.push([
        plus(
            &io_of(0),
            &plus(
                &scaled(&limb(0), -one),
                &scaled(&own(0, 2), -fp_u(&pow2(L))),
            ),
        ),
        constant(vec![one; 2]),
        Vec::new(),
    ]);
    wr.push([
        plus(
            &io_of(1),
            &plus(
                &scaled(&own(1, 2), -one),
                &scaled(&limb(2), -fp_u(&pow2(WORD_U))),
            ),
        ),
        constant(vec![one; 2]),
        Vec::new(),
    ]);
    let mut wl = Vec::new();
    for (c, w) in word_chunks.iter().flatten().enumerate() {
        range_lookups(&mut wl, 2 + c, *w, 2);
    }
    link_table(&mut wl);
    // Shared values: v and its 128-bit chunks (first phase).
    let vchunks = chunk_widths(128);
    let mut shv = vec![vec![zero; nn]; 1 + vchunks.len()];
    for (r, v) in values.iter().enumerate() {
        let x = BigUint::from(*v);
        shv[0][r] = fp_u(&x);
        for (c, val) in chunk_values(&x, &vchunks).into_iter().enumerate() {
            shv[1 + c][r] = val;
        }
    }
    let srows = vec![[
        plus(&own(0, nn), &scaled(&recompose(1, &vchunks, nn), -one)),
        constant(vec![one; nn]),
        Vec::new(),
    ]];
    let mut sl = Vec::new();
    for (c, w) in vchunks.iter().enumerate() {
        range_lookups(&mut sl, 1 + c, *w, nn);
    }
    let mut sw: Vec<u16> = vec![128];
    sw.extend(vchunks.iter().map(|w| *w as u16));
    let mut vw: Vec<u16> = FQ_LIMBS.iter().map(|w| *w as u16).collect();
    vw.extend(chunks.iter().flatten().map(|w| *w as u16));
    let mut ww: Vec<u16> = vec![WORD_T as u16, WORD_U as u16];
    ww.extend(word_chunks.iter().flatten().map(|w| *w as u16));
    // The build's columns (rho = 0); the prover fills the link phase from
    // the link's rho.
    let cols = linked_columns(
        &shape,
        &q,
        mu,
        values,
        &BigUint::zero(),
        nn,
        &chunks,
        &word_chunks,
    );
    let range = |name: &'static str| Array {
        name,
        family: gfam,
        rho: CHUNK,
        r_count: 1 << CHUNK,
        fixed: true,
        lookups: Vec::new(),
        widths: vec![CHUNK as u16],
        rows: Vec::new(),
        values: vec![(0..1u64 << CHUNK).map(Fp::from).collect()],
    };
    let io_cell = |name: &'static str| Array {
        name,
        family: gfam,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![128],
        rows: Vec::new(),
        values: vec![vec![zero]],
    };
    let mut arrays = vec![
        Array {
            name: "binding shared",
            family: fam,
            rho: rho_bits,
            r_count: n,
            fixed: false,
            lookups: sl,
            widths: sw,
            rows: srows,
            values: shv,
        },
        Array {
            name: "binding mu",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: FQ_LIMBS.iter().map(|w| *w as u16).collect(),
            rows: Vec::new(),
            values: limbs(mu).iter().map(|l| vec![fp_u(l)]).collect(),
        },
        Array {
            name: "binding value",
            family: fam,
            rho: rho_bits,
            r_count: n + 2,
            fixed: false,
            lookups: vl,
            widths: vw,
            rows: vr,
            values: cols.value,
        },
        Array {
            name: "binding check",
            family: fam,
            rho: rho_bits,
            r_count: n,
            fixed: false,
            lookups: cl,
            widths: shape.widths(),
            rows: cr,
            values: cols.check,
        },
        Array {
            name: "binding word",
            family: fam,
            rho: 1,
            r_count: 2,
            fixed: false,
            lookups: wl,
            widths: ww,
            rows: wr,
            values: cols.word,
        },
        range("range table"),
        range("link range table"),
    ];
    arrays.extend(
        [
            "binding rho lo",
            "binding rho hi",
            "binding R lo",
            "binding R hi",
        ]
        .map(io_cell),
    );
    let check_a = first + BL_CHECK;
    let (mu_v, vals) = (mu.clone(), values.to_vec());
    let fill = move |rho: &[u8; 32]| -> LinkCells {
        let rho = BigUint::from_bytes_le(rho);
        let cols = linked_columns(&shape, &q, &mu_v, &vals, &rho, nn, &chunks, &word_chunks);
        let n = vals.len();
        let mut out = Vec::new();
        for (k, col) in cols.value.iter().enumerate() {
            out.extend((0..n + 2).map(|r| (value_a, k, r, col[r])));
        }
        for (k, col) in cols.check.iter().enumerate() {
            out.extend((0..n).map(|r| (check_a, k, r, col[r])));
        }
        for (k, col) in cols.word.iter().enumerate() {
            out.extend((0..2).map(|i| (word_a, k, i, col[i])));
        }
        out
    };
    LinkedBinding {
        arrays,
        tables: vec![
            (first + BL_RANGE, RANGE_TABLE),
            (first + BL_LINK_RANGE, LINK_RANGE_TABLE),
        ],
        link_arrays: vec![value_a, check_a, word_a],
        io: vec![
            (io_a, rho_io),
            (io_a + 1, rho_io + 1),
            (io_a + 2, r_io),
            (io_a + 3, r_io + 1),
        ],
        fill: Box::new(fill),
        binding: (0..3).map(|k| (value_a, k, n)).collect(),
    }
}

/// Test and measurement inputs: rho, mu and `count` shared values below 2^128.
pub fn sample_binding(count: usize) -> (BigUint, BigUint, Vec<u128>) {
    let q = fq_modulus();
    let u128_of = |x: &BigUint| {
        x.to_u64_digits()
            .iter()
            .rev()
            .fold(0u128, |acc, d| (acc << 64) | *d as u128)
    };
    let values = (0..count)
        .map(|k| u128_of(&p256::scalar_below(&format!("binding-v{k}"), &pow2(128))))
        .collect();
    (
        p256::scalar_below("binding-rho", &q),
        p256::scalar_below("binding-mu", &q),
        values,
    )
}

// ------------------------------------------------- Bootstrap's key tables

/// The table id of Bootstrap's key points, a witness table: the bases
/// `B_k` (rows 0..16) and their doubles `2 B_k` (rows 16..32).
pub const KEY_POINTS_TABLE: u32 = 6;

/// Array offsets of the key-table derivation from `first`.
pub const KT_DVAL: usize = 0;
pub const KT_D1: usize = 1;
pub const KT_D2: usize = 2;
pub const KT_POINTS: usize = 3;
pub const KT_IVAL: usize = 4;
pub const KT_I1: usize = 5;
pub const KT_I4: usize = 6;
pub const KT_GSEL: usize = 7;
pub const KT_GVAL: usize = 8;
pub const KT_G1: usize = 9;
pub const KT_G4: usize = 10;

/// The doubling chain `P_(s + 1) = 2 P_s` from `P_0 = Q`: the bases
/// `B_k = 2^(16 k) Q = P_(16 k)` and their doubles `2 B_k = P_(16 k + 1)`
/// all lie on it, the last one at `s = 241`.
pub const KT_DOUBLINGS: usize = 241;

/// Bootstrap's derivation of a wallet's comb tables from its key (Lever 1).
pub struct KeyTables {
    pub arrays: Vec<Array<Fp>>,
    pub tables: Vec<(usize, u32)>,
    /// The derived tables: `operation::comb_tables(Q, 16)`.
    pub entries: Vec<Vec<Point>>,
    /// Per table `j`, the Gray step whose sum is entry `h` (the Gray
    /// values' instance `(slot << 9) | (j << 8) | step[h]`).
    pub step_of: Vec<Vec<usize>>,
    /// Doublings, additions, then the one-, two- and four-product checks.
    pub counts: [usize; 5],
}

/// A read of a non-native value by a key-table check: (array, first limb
/// cell, instance, coefficient).
type KRead = (usize, usize, usize, i64);

/// One input of a key-table check: reads plus constant values, each with
/// its coefficient.
#[derive(Clone, Default)]
struct KPort {
    reads: Vec<KRead>,
    consts: Vec<(BigUint, i64)>,
}

fn kport(reads: &[KRead]) -> KPort {
    KPort {
        reads: reads.to_vec(),
        consts: Vec::new(),
    }
}

/// One key-table check: `sum_i pc_i a_i b_i + lin + cst = 0 (mod p)`.
#[derive(Clone)]
struct KCheck {
    a: Vec<KPort>,
    b: Vec<KPort>,
    pc: Vec<i64>,
    lin: KPort,
    cst: i64,
}

/// A reader of one coordinate of a key-table point: its read at a
/// coefficient.
type Rd<'a> = &'a dyn Fn(i64) -> KRead;

/// The four checks of an addition `o = a + c` with slope `l`: the chord,
/// the sum's x and y, then the second slope equation (both points on the
/// curve), which leaves equal points only the tangent.
#[allow(clippy::too_many_arguments)]
fn kt_addition(
    l: Rd<'_>,
    ax: Rd<'_>,
    ay: Rd<'_>,
    cx: Rd<'_>,
    cy: Rd<'_>,
    ox: Rd<'_>,
    oy: Rd<'_>,
) -> [KCheck; 4] {
    [
        KCheck {
            a: vec![kport(&[l(1)])],
            b: vec![kport(&[cx(1), ax(-1)])],
            pc: vec![1],
            lin: kport(&[cy(-1), ay(1)]),
            cst: 0,
        },
        KCheck {
            a: vec![kport(&[l(1)])],
            b: vec![kport(&[l(1)])],
            pc: vec![1],
            lin: kport(&[ax(-1), cx(-1), ox(-1)]),
            cst: 0,
        },
        KCheck {
            a: vec![kport(&[l(1)])],
            b: vec![kport(&[ax(1), ox(-1)])],
            pc: vec![1],
            lin: kport(&[ay(-1), oy(-1)]),
            cst: 0,
        },
        KCheck {
            a: vec![
                kport(&[l(1)]),
                kport(&[ax(1)]),
                kport(&[ax(1)]),
                kport(&[cx(1)]),
            ],
            b: vec![
                kport(&[ay(1), cy(1)]),
                kport(&[ax(1)]),
                kport(&[cx(1)]),
                kport(&[cx(1)]),
            ],
            pc: vec![1, -1, -1, -1],
            lin: KPort::default(),
            cst: 3,
        },
    ]
}

/// The smallest `rho` with `count <= 2^rho`.
fn rho_of(count: usize) -> usize {
    count.max(1).next_power_of_two().trailing_zeros() as usize
}

/// A value array (three limbs from range-checked chunks per instance); idle
/// instances (`None`) hold zero.
fn kt_value_array(name: &'static str, fam: usize, vals: &[Option<BigUint>]) -> Array<Fp> {
    let rho = rho_of(vals.len());
    let n = 1usize << rho;
    let (rows, lookups) = value_template(n);
    let mut values = vec![vec![Fp::from(0u64); n]; value_cells_count()];
    for (r, v) in vals.iter().enumerate() {
        if let Some(v) = v {
            for (k, cell) in value_cells(v).into_iter().enumerate() {
                values[k][r] = cell;
            }
        }
    }
    Array {
        name,
        family: fam,
        rho,
        r_count: vals.len(),
        fixed: false,
        lookups,
        widths: value_widths(),
        rows,
        values,
    }
}

/// A check array of `shape` over `checks`, an idle instance (`None`) having
/// every input zero, which holds trivially. A port's reads are links grouped
/// by (array, first limb cell, position in the port); `value_of(array,
/// first, instance)` gives a read's native value.
fn kt_check_array(
    name: &'static str,
    fam: usize,
    shape: &CheckShape,
    checks: &[Option<KCheck>],
    value_of: &dyn Fn(usize, usize, usize) -> BigUint,
) -> Array<Fp> {
    let rho = rho_of(checks.len());
    let n = 1usize << rho;
    let zero = Fp::from(0u64);
    let port = |get: &dyn Fn(&KCheck) -> &KPort| -> Port {
        let width = checks
            .iter()
            .flatten()
            .map(|c| get(c).reads.len())
            .max()
            .unwrap_or(0);
        let mut links = Vec::new();
        for q in 0..width {
            let mut groups: Vec<((usize, usize), Vec<Option<usize>>, Vec<Fp>)> = Vec::new();
            for (r, c) in checks.iter().enumerate() {
                let Some(c) = c else { continue };
                let Some(&(a, first, inst, coef)) = get(c).reads.get(q) else {
                    continue;
                };
                let g = match groups.iter().position(|g| g.0 == (a, first)) {
                    Some(i) => i,
                    None => {
                        groups.push(((a, first), vec![None; n], vec![zero; n]));
                        groups.len() - 1
                    }
                };
                groups[g].1[r] = Some(inst);
                groups[g].2[r] = fp_i(&BigInt::from(coef));
            }
            for ((a, first), map, coef) in groups {
                links.push((a, first, map, coef));
            }
        }
        let mut consts = [vec![zero; n], vec![zero; n], vec![zero; n]];
        for (r, c) in checks.iter().enumerate() {
            let Some(c) = c else { continue };
            for (x, coef) in &get(c).consts {
                let l = limbs(x);
                for k in 0..3 {
                    consts[k][r] += fp_i(&(BigInt::from(l[k].clone()) * BigInt::from(*coef)));
                }
            }
        }
        Port { links, consts }
    };
    let products = shape.products;
    let a: Vec<Port> = (0..products).map(|i| port(&|c| &c.a[i])).collect();
    let b: Vec<Port> = (0..products).map(|i| port(&|c| &c.b[i])).collect();
    let lin = port(&|c| &c.lin);
    let mut pc = vec![vec![zero; n]; products];
    let mut cst = vec![zero; n];
    for (r, c) in checks.iter().enumerate() {
        let Some(c) = c else { continue };
        assert_eq!(c.pc.len(), products, "{name}: one coefficient per product");
        for i in 0..products {
            pc[i][r] = fp_i(&BigInt::from(c.pc[i]));
        }
        cst[r] = fp_i(&BigInt::from(c.cst));
    }
    let (rows, lookups) = shape.template(&a, &b, &pc, &lin, &cst, &vec![0; n], n);
    let eval = |pt: &KPort| -> [BigInt; 3] {
        let mut out: [BigInt; 3] = Default::default();
        for &(a, first, inst, coef) in &pt.reads {
            let l = limbs(&value_of(a, first, inst));
            for k in 0..3 {
                out[k] += BigInt::from(l[k].clone()) * BigInt::from(coef);
            }
        }
        for (x, coef) in &pt.consts {
            let l = limbs(x);
            for k in 0..3 {
                out[k] += BigInt::from(l[k].clone()) * BigInt::from(*coef);
            }
        }
        out
    };
    let mut values = vec![vec![zero; n]; shape.cells];
    let zeros: Vec<[BigInt; 3]> = vec![Default::default(); products];
    for r in 0..checks.len() {
        let cells = match &checks[r] {
            Some(c) => {
                let av: Vec<[BigInt; 3]> = c.a.iter().map(eval).collect();
                let bv: Vec<[BigInt; 3]> = c.b.iter().map(eval).collect();
                shape.witness(&av, &bv, &c.pc, &eval(&c.lin), c.cst, 0)
            }
            None => shape.witness(
                &zeros,
                &zeros,
                &vec![0; products],
                &Default::default(),
                0,
                0,
            ),
        };
        for (k, cell) in cells.into_iter().enumerate() {
            values[k][r] = cell;
        }
    }
    Array {
        name,
        family: fam,
        rho,
        r_count: checks.len(),
        fixed: false,
        lookups,
        widths: shape.widths(),
        rows,
        values,
    }
}

/// Bootstrap's derivation of the wallet's comb tables (m = 16) from its key
/// `Q`, as uniform arrays from index `first` in family `fam` (range lookups
/// into the circuit's range table):
///
/// - the doubling chain (`KT_DOUBLINGS` doublings), with `Q` on the curve:
///   value slots per step `s` are the slope (`t = x^2` at `s = 0`), `x`, `y`;
///   each doubling checks `l (2 y) - 3 x^2 + 3 = 0`, `x' = l^2 - 2 x` and
///   `y' = l (x - x') - y`;
/// - the key points, a witness table of the bases and their doubles, tied to
///   the chain;
/// - per table `j` (bases `8 j..8 j + 8`), the sum `S_j` of its bases by
///   seven additions (six in the initial array, the seventh at the Gray
///   walk's step 0): `S_j` is entry 255;
/// - the Gray walk, steps 1..255: step `t` flips bit `k_t` of the index
///   (`h_t = gray(t) ^ 255`) and adds `+-2 B_(8 j + k_t)`, so that every
///   entry `sum_k (2 bit_k(h) - 1) B_(8 j + k)` is one step's sum. The addend
///   is looked up in the key points by a per-step index; its sign is a
///   per-step coefficient on its `y`.
///
/// Every addition checks both slope equations (the chord and, with both
/// points on the curve, `l (a.y + c.y) = a.x^2 + a.x c.x + c.x^2 - 3`), so
/// equal points can only double.
pub fn key_tables(first: usize, fam: usize, q: &Point) -> KeyTables {
    let p = p256::p();
    let at = |k: usize| first + k;
    // The native chain.
    let mut dp: Vec<Point> = vec![q.clone()];
    let mut dl: Vec<BigUint> = vec![&q.x * &q.x % &p];
    for s in 1..=KT_DOUBLINGS {
        let prev = dp[s - 1].clone();
        dl.push(slope(&prev, &prev));
        dp.push(p256::double(&prev));
    }
    let step_of_point = |i: usize| if i < 16 { 16 * i } else { 16 * (i - 16) + 1 };
    let key_point = |i: usize| dp[step_of_point(i)].clone();
    // The additions: (the addend c before its sign, the sign, slope, sum),
    // per table.
    struct Add {
        c: Point,
        sign: i64,
        lam: BigUint,
        out: Point,
    }
    let add = |a: &Point, c: &Point, sign: i64| -> Add {
        let cs = if sign < 0 { p256::neg(c) } else { c.clone() };
        let lam = slope(a, &cs);
        let out =
            p256::add_any(Some(a.clone()), Some(cs)).expect("a table entry is not the identity");
        Add {
            c: c.clone(),
            sign,
            lam,
            out,
        }
    };
    let mut init: Vec<Vec<Add>> = Vec::new();
    let mut gray: Vec<Vec<Add>> = Vec::new();
    let mut gray_row: Vec<Vec<usize>> = Vec::new();
    let mut entries: Vec<Vec<Point>> = Vec::new();
    let mut step_of: Vec<Vec<usize>> = Vec::new();
    for j in 0..2 {
        let mut cur = key_point(8 * j);
        let mut iv = Vec::new();
        for s in 0..6 {
            let x = add(&cur, &key_point(8 * j + s + 1), 1);
            cur = x.out.clone();
            iv.push(x);
        }
        let (mut gv, mut rows) = (Vec::new(), Vec::new());
        let mut table: Vec<Option<Point>> = vec![None; 256];
        let mut steps = vec![0usize; 256];
        let (mut h, mut prev_gray) = (255usize, 0usize);
        for t in 0..256usize {
            let (row, sign) = if t == 0 {
                (8 * j + 7, 1)
            } else {
                let g = t ^ (t >> 1);
                let k = (g ^ prev_gray).trailing_zeros() as usize;
                prev_gray = g;
                h ^= 1 << k;
                (16 + 8 * j + k, if h & (1 << k) != 0 { 1 } else { -1 })
            };
            let x = add(&cur, &key_point(row), sign);
            cur = x.out.clone();
            assert!(table[h].is_none(), "each entry once");
            table[h] = Some(cur.clone());
            steps[h] = t;
            gv.push(x);
            rows.push(row);
        }
        init.push(iv);
        gray.push(gv);
        gray_row.push(rows);
        entries.push(table.into_iter().map(|e| e.expect("every entry")).collect());
        step_of.push(steps);
    }
    assert_eq!(
        entries,
        crate::operation::comb_tables(q, 16),
        "the walk derives the comb tables"
    );
    // Native values by (array, first limb cell, instance).
    let mut native: std::collections::HashMap<(usize, usize, usize), BigUint> =
        std::collections::HashMap::new();
    let n_d = 256usize;
    let mut dvals: Vec<Option<BigUint>> = vec![None; 3 * n_d];
    for s in 0..=KT_DOUBLINGS {
        for (slot, x) in [&dl[s], &dp[s].x, &dp[s].y].into_iter().enumerate() {
            dvals[(slot << 8) | s] = Some(x.clone());
            native.insert((at(KT_DVAL), 0, (slot << 8) | s), x.clone());
        }
    }
    let mut ivals: Vec<Option<BigUint>> = vec![None; 48];
    for (j, iv) in init.iter().enumerate() {
        for (s, x) in iv.iter().enumerate() {
            for (slot, v) in [&x.lam, &x.out.x, &x.out.y].into_iter().enumerate() {
                let r = (slot << 4) | (j << 3) | s;
                ivals[r] = Some(v.clone());
                native.insert((at(KT_IVAL), 0, r), v.clone());
            }
        }
    }
    let mut gvals: Vec<Option<BigUint>> = vec![None; 3 << 9];
    let mut gsel = vec![vec![Fp::from(0u64); 512]; 6];
    for (j, gv) in gray.iter().enumerate() {
        for (t, x) in gv.iter().enumerate() {
            for (slot, v) in [&x.lam, &x.out.x, &x.out.y].into_iter().enumerate() {
                let r = (slot << 9) | (j << 8) | t;
                gvals[r] = Some(v.clone());
                native.insert((at(KT_GVAL), 0, r), v.clone());
            }
            let r = (j << 8) | t;
            let (xl, yl) = (limbs(&x.c.x), limbs(&x.c.y));
            for k in 0..3 {
                gsel[k][r] = fp_u(&xl[k]);
                gsel[3 + k][r] = fp_u(&yl[k]);
            }
            native.insert((at(KT_GSEL), 0, r), x.c.x.clone());
            native.insert((at(KT_GSEL), 3, r), x.c.y.clone());
        }
    }
    let value_of = |a: usize, first: usize, inst: usize| -> BigUint {
        native
            .get(&(a, first, inst))
            .unwrap_or_else(|| panic!("a native value at ({a}, {first}, {inst})"))
            .clone()
    };
    // Reads.
    let dv = |slot: usize, s: usize, coef: i64| (at(KT_DVAL), 0, (slot << 8) | s, coef);
    let iv = |slot: usize, j: usize, s: usize, coef: i64| {
        (at(KT_IVAL), 0, (slot << 4) | (j << 3) | s, coef)
    };
    let gv = |slot: usize, j: usize, t: usize, coef: i64| {
        (at(KT_GVAL), 0, (slot << 9) | (j << 8) | t, coef)
    };
    let gs = |cell: usize, j: usize, t: usize, coef: i64| (at(KT_GSEL), cell, (j << 8) | t, coef);
    // The doubling chain's checks.
    let mut d1: Vec<Option<KCheck>> = vec![None; 2 * n_d];
    let mut d2: Vec<Option<KCheck>> = vec![None; n_d];
    d1[0] = Some(KCheck {
        a: vec![kport(&[dv(1, 0, 1)])],
        b: vec![kport(&[dv(1, 0, 1)])],
        pc: vec![1],
        lin: kport(&[dv(0, 0, -1)]),
        cst: 0,
    });
    d2[0] = Some(KCheck {
        a: vec![kport(&[dv(2, 0, 1)]), kport(&[dv(1, 0, 1)])],
        b: vec![kport(&[dv(2, 0, 1)]), kport(&[dv(0, 0, 1)])],
        pc: vec![1, -1],
        lin: KPort {
            reads: vec![dv(1, 0, 3)],
            consts: vec![(p256::b(), -1)],
        },
        cst: 0,
    });
    for s in 1..=KT_DOUBLINGS {
        let l = dv(0, s, 1);
        d1[s] = Some(KCheck {
            a: vec![kport(&[l])],
            b: vec![kport(&[l])],
            pc: vec![1],
            lin: kport(&[dv(1, s - 1, -2), dv(1, s, -1)]),
            cst: 0,
        });
        d1[n_d + s] = Some(KCheck {
            a: vec![kport(&[l])],
            b: vec![kport(&[dv(1, s - 1, 1), dv(1, s, -1)])],
            pc: vec![1],
            lin: kport(&[dv(2, s - 1, -1), dv(2, s, -1)]),
            cst: 0,
        });
        d2[s] = Some(KCheck {
            a: vec![kport(&[l]), kport(&[dv(1, s - 1, 1)])],
            b: vec![kport(&[dv(2, s - 1, 2)]), kport(&[dv(1, s - 1, 3)])],
            pc: vec![1, -1],
            lin: KPort::default(),
            cst: 3,
        });
    }
    // The initial sums: a = B_(8 j) at s = 0, else the previous sum; c =
    // B_(8 j + s + 1), read from the chain.
    let mut i1: Vec<Option<KCheck>> = vec![None; 48];
    let mut i4: Vec<Option<KCheck>> = vec![None; 16];
    for j in 0..2 {
        for s in 0..6 {
            let s0 = step_of_point(8 * j);
            let a_at = |slot: usize, c: i64| {
                if s == 0 {
                    dv(slot, s0, c)
                } else {
                    iv(slot, j, s - 1, c)
                }
            };
            let st = step_of_point(8 * j + s + 1);
            let checks = kt_addition(
                &|c| iv(0, j, s, c),
                &|c| a_at(1, c),
                &|c| a_at(2, c),
                &|c| dv(1, st, c),
                &|c| dv(2, st, c),
                &|c| iv(1, j, s, c),
                &|c| iv(2, j, s, c),
            );
            let [c0, c1, c2, c3] = checks;
            for (spec, c) in [c0, c1, c2].into_iter().enumerate() {
                i1[(spec << 4) | (j << 3) | s] = Some(c);
            }
            i4[(j << 3) | s] = Some(c3);
        }
    }
    // The Gray walk: a = the last initial sum at t = 0, else the previous
    // step's sum; c = the selected key point, its y signed per step.
    let mut g1: Vec<Option<KCheck>> = vec![None; 3 << 9];
    let mut g4: Vec<Option<KCheck>> = vec![None; 512];
    for j in 0..2 {
        for t in 0..256 {
            let a_at = |slot: usize, c: i64| {
                if t == 0 {
                    iv(slot, j, 5, c)
                } else {
                    gv(slot, j, t - 1, c)
                }
            };
            let sign = gray[j][t].sign;
            let checks = kt_addition(
                &|c| gv(0, j, t, c),
                &|c| a_at(1, c),
                &|c| a_at(2, c),
                &|c| gs(0, j, t, c),
                &|c| gs(3, j, t, c * sign),
                &|c| gv(1, j, t, c),
                &|c| gv(2, j, t, c),
            );
            let [c0, c1, c2, c3] = checks;
            for (spec, c) in [c0, c1, c2].into_iter().enumerate() {
                g1[(spec << 9) | (j << 8) | t] = Some(c);
            }
            g4[(j << 8) | t] = Some(c3);
        }
    }
    // Shapes (modulus p only).
    let moduli = vec![p.clone()];
    let s1 = CheckShape::new(
        &[(sources(1, 1), sources(2, 1))],
        &sources(3, 2),
        3,
        &moduli,
    );
    let s2 = CheckShape::new(
        &[
            (sources(1, 1), sources(1, 2)),
            (sources(1, 1), sources(1, 3)),
        ],
        &sources(2, 3),
        3,
        &moduli,
    );
    let s4 = CheckShape::new(
        &[
            (sources(1, 1), sources(2, 1)),
            (sources(1, 1), sources(1, 1)),
            (sources(1, 1), sources(1, 1)),
            (sources(1, 1), sources(1, 1)),
        ],
        &sources(3, 2),
        3,
        &moduli,
    );
    // The key points: a witness table (index, x limbs, y limbs), tied to the
    // chain's values.
    let one = Fp::from(1u64);
    let n_k = 32usize;
    let mut kvals = vec![vec![Fp::from(0u64); n_k]; 7];
    for i in 0..n_k {
        let pt = key_point(i);
        kvals[0][i] = Fp::from(i as u64);
        let (xl, yl) = (limbs(&pt.x), limbs(&pt.y));
        for k in 0..3 {
            kvals[1 + k][i] = fp_u(&xl[k]);
            kvals[4 + k][i] = fp_u(&yl[k]);
        }
    }
    let chain_map = |slot: usize| -> Vec<Option<usize>> {
        (0..n_k)
            .map(|i| Some((slot << 8) | step_of_point(i)))
            .collect()
    };
    let mut krows: Vec<[Lin<Fp>; 3]> = vec![[
        plus(
            &own(0, n_k),
            &constant((0..n_k).map(|i| -Fp::from(i as u64)).collect()),
        ),
        constant(vec![one; n_k]),
        Vec::new(),
    ]];
    for (base, slot) in [(1usize, 1usize), (4, 2)] {
        for k in 0..3 {
            krows.push([
                plus(
                    &own(base + k, n_k),
                    &link(at(KT_DVAL), k, chain_map(slot), -one),
                ),
                constant(vec![one; n_k]),
                Vec::new(),
            ]);
        }
    }
    // The Gray walk's addends: looked up in the key points by row.
    let gs_lookup = {
        let mut t = vec![constant(
            (0..512)
                .map(|r| Fp::from(gray_row[r >> 8][r & 255] as u64))
                .collect(),
        )];
        for k in 0..6 {
            t.push(own(k, 512));
        }
        (KEY_POINTS_TABLE, t)
    };
    let arrays = vec![
        kt_value_array("key-table chain value", fam, &dvals),
        kt_check_array("key-table chain check 1", fam, &s1, &d1, &value_of),
        kt_check_array("key-table chain check 2", fam, &s2, &d2, &value_of),
        Array {
            name: "key points",
            family: fam,
            rho: rho_of(n_k),
            r_count: n_k,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![5, 88, 88, 80, 88, 88, 80],
            rows: krows,
            values: kvals,
        },
        kt_value_array("key-table sum value", fam, &ivals),
        kt_check_array("key-table sum check 1", fam, &s1, &i1, &value_of),
        kt_check_array("key-table sum check 4", fam, &s4, &i4, &value_of),
        Array {
            name: "key-table walk addend",
            family: fam,
            rho: 9,
            r_count: 512,
            fixed: false,
            lookups: vec![gs_lookup],
            widths: vec![88, 88, 80, 88, 88, 80],
            rows: Vec::new(),
            values: gsel,
        },
        kt_value_array("key-table walk value", fam, &gvals),
        kt_check_array("key-table walk check 1", fam, &s1, &g1, &value_of),
        kt_check_array("key-table walk check 4", fam, &s4, &g4, &value_of),
    ];
    let count = |v: &[Option<KCheck>]| v.iter().flatten().count();
    let counts = [
        KT_DOUBLINGS,
        2 * (6 + 256),
        count(&d1) + count(&i1) + count(&g1),
        count(&d2),
        count(&i4) + count(&g4),
    ];
    KeyTables {
        arrays,
        tables: vec![(at(KT_POINTS), KEY_POINTS_TABLE)],
        entries,
        step_of,
        counts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::r1cs::{Circuit, Witness};
    use crate::uniform::UniformCircuit;
    use ff::Field;
    use iroha_pasta::Eq as Vesta;

    fn small() -> (
        UniformCircuit<Fp>,
        crate::r1cs::Instance<Fp>,
        Witness<Fp>,
        Vec<Point>,
        Vec<(Point, BigUint, BigUint, BigUint)>,
    ) {
        let inputs = sample_inputs(2);
        let Comb {
            arrays,
            tables,
            results,
            ..
        } = comb(0, 0, 1, 1, &inputs);
        let (c, inst, wit) = UniformCircuit::build(
            "comb-test",
            arrays,
            vec![1, 0],
            vec![2, 1],
            tables,
            14,
            4,
            b"comb",
        );
        (c, inst, wit, results, inputs)
    }

    #[test]
    fn check_shapes_are_canonical() {
        let moduli = vec![p256::p(), p256::n()];
        let s1 = CheckShape::new(
            &[(max_bound(&sources(2, 1), &u_bound()), sources(2, 1))],
            &sources(3, 2),
            3,
            &moduli,
        );
        let s4 = CheckShape::new(
            &[
                (sources(1, 1), sources(2, 1)),
                (sources(1, 1), sources(1, 1)),
                (sources(1, 1), sources(1, 1)),
                (sources(1, 1), sources(1, 1)),
            ],
            &sources(3, 2),
            3,
            &moduli[..1],
        );
        assert_eq!(s1.widths().len(), s1.cells);
        assert_eq!(s4.widths().len(), s4.cells);
    }

    fn keyed_input(label: &str) -> (Point, BigUint, BigUint, BigUint) {
        let n = p256::n();
        let d = p256::scalar_below(label, &n);
        let q = p256::mul(&p256::generator(), &d).unwrap();
        let z = p256::scalar_below(&format!("{label}-message"), &pow2(256));
        let nonce = p256::scalar_below(&format!("{label}-nonce"), &n);
        let sig = p256::sign(&d, &z, &nonce);
        (q, z, sig.r, sig.s)
    }

    #[test]
    fn custom_witness_key_table_routes_its_lookups() {
        let input = keyed_input("witness-key-table");
        let id = 8;
        let family = comb_with_key_table(10, 0, 1, 1, &[input], KeyTable::Witness { id });
        assert_eq!(family.arrays[A_SEL].lookups[0].0, id);
        assert!(family.tables.contains(&(10 + A_WTABLE, id)));
        assert!(!family.arrays[A_WTABLE].fixed);
        assert_eq!(family.arrays[A_SELG].lookups[0].0, POINT_TABLE);
    }

    #[test]
    #[should_panic(expected = "must not alias a shared or reserved table")]
    fn key_table_rejects_a_shared_table_id() {
        let inputs = sample_inputs(1);
        let _ = comb_with_key_table(0, 0, 1, 1, &inputs, KeyTable::Fixed { id: POINT_TABLE });
    }

    /// Two different authority keys use distinct lookup families. Their
    /// fixed tables are authenticated by the circuit key: an honest proof
    /// cannot be verified after swapping the two keys' tables. This tests
    /// the generic fixed-table mechanism, not a certificate trust chain.
    #[test]
    fn distinct_fixed_key_tables_prove_and_reject_swaps() {
        let inputs = [keyed_input("issuer-comb"), keyed_input("root-comb")];
        assert_ne!(inputs[0].0, inputs[1].0);
        let build = |swap_tables: bool| {
            let mut arrays = Vec::new();
            let mut tables = Vec::new();
            for fam in 0..inputs.len() {
                let input = &inputs[if swap_tables { 1 - fam } else { fam }];
                let first = arrays.len();
                let id = 8 + fam as u32;
                let family = comb_with_key_table(
                    first,
                    fam,
                    1,
                    2,
                    std::slice::from_ref(input),
                    KeyTable::Fixed { id },
                );
                assert_eq!(family.arrays[A_SEL].lookups[0].0, id);
                assert!(family.arrays[A_WTABLE].fixed);
                arrays.extend(family.arrays);
                for table in family.tables {
                    if !tables.iter().any(|&(_, id)| id == table.1) {
                        tables.push(table);
                    }
                }
            }
            let (c, inst, wit) = UniformCircuit::build(
                "distinct-key-tables",
                arrays,
                vec![1, 1, 0],
                vec![1, 1, 1],
                tables,
                14,
                4,
                b"distinct-key-tables",
            );
            (c, inst, wit)
        };
        let (c, inst, wit) = build(false);
        let gens = crate::ipa::Gens::<Vesta>::new(c.shape.cols());
        let mut rng = SpikeRng::new(b"distinct key tables prove");
        let (proof, _) = crate::spartan::prove_circuit(&c.shape, &gens, &inst, &wit, &mut rng, &c);
        assert!(
            crate::spartan::verify_circuit(&c.shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "signatures under both distinct fixed keys verify"
        );
        let (swapped, swapped_inst, _) = build(true);
        assert_eq!(c.shape.sx, swapped.shape.sx);
        assert_eq!(c.shape.rows(), swapped.shape.rows());
        assert_eq!(c.shape.cols(), swapped.shape.cols());
        assert!(
            crate::spartan::verify_circuit(
                &swapped.shape,
                &gens,
                &swapped_inst,
                &proof.to_bytes(),
                &swapped
            )
            .is_err(),
            "swapping the issuer and root tables changes the authenticated circuit"
        );
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
        delta: Fp,
    ) -> bool {
        let mut bad = wit.w.clone();
        bad[c.cell_index(a, k, i)] += delta;
        let mut prng = SpikeRng::new(b"comb refuse");
        let (proof, _) =
            crate::spartan::prove_circuit(&c.shape, gens, inst, &Witness { w: bad }, &mut prng, c);
        crate::spartan::verify_circuit(&c.shape, gens, inst, &proof.to_bytes(), c).is_err()
    }

    #[test]
    fn binding_chain_holds_proves_and_refuses() {
        let (rho, mu, values) = sample_binding(20);
        let q = fq_modulus();
        let Binding {
            arrays,
            tables,
            result,
            shape: bshape,
        } = binding_chain(0, 0, 1, &rho, &mu, &values);
        let mut acc = mu.clone();
        for v in &values {
            acc = (acc * &rho + BigUint::from(*v)) % &q;
        }
        assert_eq!(result, acc, "R equals native Horner");
        let (c, inst, wit) = UniformCircuit::build(
            "binding-test",
            arrays,
            vec![0, 0],
            vec![1, 1],
            tables,
            12,
            4,
            b"binding",
        );
        let shape = c.shape;
        let mut rng = SpikeRng::new(b"binding ch");
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
        let mut prng = SpikeRng::new(b"binding prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let one = Fp::ONE;
        // A changed accumulator limb, a changed shared value, a changed rho chunk.
        assert!(
            refused(&c, &inst, &wit, &gens, B_VALUE, 1, 7, one),
            "changed accumulator"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, B_SHARED, 0, 3, one),
            "changed shared value"
        );
        // A shared value re-chunked as v + 1 (its own row holds): the check
        // refuses it.
        let mut bad = wit.w.clone();
        let x = BigUint::from(values[4]) + 1u32;
        bad[c.cell_index(B_SHARED, 0, 4)] = fp_u(&x);
        for (k, val) in chunk_values(&x, &chunk_widths(128)).into_iter().enumerate() {
            bad[c.cell_index(B_SHARED, 1 + k, 4)] = val;
        }
        let mut prng = SpikeRng::new(b"binding rechunk");
        let (proof, _) =
            crate::spartan::prove_circuit(&shape, &gens, &inst, &Witness { w: bad }, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_err(),
            "re-chunked shared value refused"
        );
        let _ = bshape;
        assert!(
            refused(&c, &inst, &wit, &gens, B_VALUE, 5, values.len() + 1, one),
            "changed rho chunk"
        );
    }

    #[test]
    fn comb_rows_hold_prove_and_refuse() {
        let (c, inst, wit, results, inputs) = small();
        for (pt, (q, z, r, s)) in results.iter().zip(&inputs) {
            let n = p256::n();
            let w = p256::inv(s, &n);
            let expect = p256::add_any(
                p256::mul(&p256::generator(), &(z * &w % &n)),
                p256::mul(q, &(r * &w % &n)),
            )
            .unwrap();
            assert_eq!(pt, &expect, "u1 G + u2 Q");
            assert_eq!(&pt.x % &n, *r);
        }
        let shape = c.shape;
        let mut rng = SpikeRng::new(b"comb ch");
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
        // The matrix evaluation (slot x step factors) agrees with the entries.
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
        let mut prng = SpikeRng::new(b"comb prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let one = Fp::ONE;
        let extra = |e: usize| (EXTRA_SLOT << 4) | e;
        // A changed slope (step 3).
        assert!(
            refused(&c, &inst, &wit, &gens, A_VALUE, 0, 3, one),
            "changed slope"
        );
        // A flipped digit bit (signature 1, u2, step 5, bit 9): breaks the
        // booleanity, Horner and selection rows.
        assert!(
            refused(
                &c,
                &inst,
                &wit,
                &gens,
                A_DIGITS,
                9,
                (1 << 5) | (1 << 4) | 5,
                one
            ),
            "flipped digit"
        );
        // A changed r limb (signature 0).
        assert!(
            refused(&c, &inst, &wit, &gens, A_VALUE, 1, extra(R_), one),
            "changed r limb"
        );
        // The final x(P) and the flag.
        assert!(
            refused(
                &c,
                &inst,
                &wit,
                &gens,
                A_VALUE,
                0,
                (1 << 8) | extra(XP_),
                one
            ),
            "changed x(P)"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, A_ADD, 2, 3, one),
            "flipped flag"
        );
    }

    /// Bootstrap's key tables (m = 16) derived from a key in uniform arrays:
    /// the walk's sums are the comb tables and sit in the witness, every row
    /// holds, the matrix evaluation matches the entries, the circuit proves
    /// and verifies, and a changed walk slope, chain value, addend or key
    /// point is refused.
    #[test]
    fn key_tables_derive_the_comb_tables() {
        let n = p256::n();
        let q = p256::mul(
            &p256::generator(),
            &p256::scalar_below("key-tables-test", &n),
        )
        .unwrap();
        let first = 1;
        let kt = key_tables(first, 0, &q);
        assert_eq!(
            kt.entries,
            crate::operation::comb_tables(&q, 16),
            "the derived tables"
        );
        assert_eq!(
            kt.counts,
            [
                KT_DOUBLINGS,
                524,
                2 * KT_DOUBLINGS + 1 + 3 * 524,
                KT_DOUBLINGS + 1,
                524
            ],
            "doublings, additions and checks"
        );
        let mut arrays = vec![Array {
            name: "range table",
            family: 0,
            rho: CHUNK,
            r_count: 1 << CHUNK,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![CHUNK as u16],
            rows: Vec::new(),
            values: vec![(0..1u64 << CHUNK).map(Fp::from).collect()],
        }];
        let mut tables = vec![(0usize, RANGE_TABLE)];
        tables.extend(kt.tables.iter().copied());
        arrays.extend(kt.arrays);
        let (c, inst, wit) = UniformCircuit::build(
            "key-tables-test",
            arrays,
            vec![0],
            vec![1],
            tables,
            14,
            4,
            b"key tables",
        );
        for j in 0..2 {
            for h in [0usize, 1, 128, 200, 255] {
                let t = kt.step_of[j][h];
                let e = &kt.entries[j][h];
                for (slot, v) in [(1usize, &e.x), (2, &e.y)] {
                    let l = limbs(v);
                    for k in 0..3 {
                        assert_eq!(
                            wit.w[c.cell_index(first + KT_GVAL, k, (slot << 9) | (j << 8) | t)],
                            fp_u(&l[k]),
                            "entry ({j}, {h}) in the witness"
                        );
                    }
                }
            }
        }
        let shape = c.shape;
        let mut rng = SpikeRng::new(b"key tables ch");
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
        for &(mat, x, y, v) in &c.entries(&ch) {
            slow[mat] += ex[x] * ey[y] * v;
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (slow[0], slow[1], slow[2]),
            "matrix evaluation matches the entries"
        );
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let mut prng = SpikeRng::new(b"key tables prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let one = Fp::ONE;
        assert!(
            refused(
                &c,
                &inst,
                &wit,
                &gens,
                first + KT_GVAL,
                0,
                (1 << 8) | 77,
                one
            ),
            "changed walk slope"
        );
        assert!(
            refused(
                &c,
                &inst,
                &wit,
                &gens,
                first + KT_DVAL,
                1,
                (1 << 8) | 40,
                one
            ),
            "changed chain x"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, first + KT_GSEL, 4, 9, one),
            "changed addend"
        );
        assert!(
            refused(&c, &inst, &wit, &gens, first + KT_POINTS, 2, 20, one),
            "changed key point"
        );
    }
}
