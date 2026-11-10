//! Operation census (gate 1c): R1CS constraints, witness widths and lookups of
//! the Send and Receive relations themselves, with the large gadgets
//! synthesized on real values and every constraint checked on insertion:
//!
//! - P-256 ECDSA verification in the Fp circuit: non-native field arithmetic
//!   (three 88-bit limbs, CRT check mod 2^264 and mod Fp, range checks by
//!   lookups), R recovery, half-size scalars (v = v' u2 mod n with |v|, |v'|
//!   ~ 2^129) and one Straus loop over v' R' - v Q - w G with 4-bit odd-digit
//!   windows, dynamic tables for R' and Q and one fixed table for (G, 2^128 G).
//! - SHA-256 of the signed 32-byte message, bit-level, with constant folding.
//! - RP57 permutations (243 constraints, measured in `gadgets.rs`), indexed
//!   Merkle tree paths and range checks, counted from the specification.
//!
//! Lookups use LogUp with committed inverse helpers in a second witness phase
//! (after the main rows are committed): one constraint `h (alpha - f) = 1` and
//! one full-width helper per lookup, one per table entry, one linear sum row.

use std::collections::BTreeMap;

use ff::{Field, PrimeField};
use iroha_pasta::Fp;
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_traits::{One, ToPrimitive, Zero};

use crate::p256::{self, Point, Signature};

type V = usize;

/// Limb width of non-native values (3 limbs, mod-2^264 carry check).
const L: usize = 88;

fn fp_modulus() -> BigUint {
    BigUint::parse_bytes(
        b"40000000000000000000000000000000224698fc094cf91b992d30ed00000001",
        16,
    )
    .unwrap()
}

fn fp_from_big(x: &BigInt) -> Fp {
    let m = BigInt::from(fp_modulus());
    let r = x.mod_floor(&m).to_biguint().unwrap();
    let mut bytes = r.to_bytes_le();
    bytes.resize(32, 0);
    Fp::from_repr(bytes.try_into().unwrap()).unwrap()
}

fn fp_u(x: &BigUint) -> Fp {
    fp_from_big(&BigInt::from(x.clone()))
}

fn pow2(k: usize) -> BigUint {
    BigUint::one() << k
}

#[derive(Clone, Debug, Default)]
pub struct Lc(Vec<(V, Fp)>);

impl Lc {
    fn var(v: V) -> Self {
        Lc(vec![(v, Fp::ONE)])
    }
    fn constant(c: Fp) -> Self {
        if c == Fp::ZERO {
            Lc(Vec::new())
        } else {
            Lc(vec![(0, c)])
        }
    }
    fn plus(&self, o: &Lc) -> Lc {
        let mut t = self.0.clone();
        t.extend_from_slice(&o.0);
        Lc(t).compact()
    }
    fn minus(&self, o: &Lc) -> Lc {
        self.plus(&o.scale(-Fp::ONE))
    }
    fn scale(&self, s: Fp) -> Lc {
        Lc(self.0.iter().map(|(v, c)| (*v, *c * s)).collect())
    }
    fn compact(mut self) -> Lc {
        self.0.sort_by_key(|(v, _)| *v);
        let mut out: Vec<(V, Fp)> = Vec::with_capacity(self.0.len());
        for (v, c) in self.0 {
            match out.last_mut() {
                Some((last, sum)) if *last == v => *sum += c,
                _ => out.push((v, c)),
            }
        }
        out.retain(|(_, c)| *c != Fp::ZERO);
        Lc(out)
    }
    fn nnz(&self) -> usize {
        self.0.len()
    }
}

/// Counting constraint system with width classes and lookup records.
pub struct OpCs {
    vals: Vec<Fp>,
    /// Width class (bits) of each allocated value; index 0 is the constant 1.
    pub widths: Vec<u16>,
    pub product_rows: usize,
    pub linear_rows: usize,
    /// Nonzeros in A, B, C over all rows (matrix density for the sumchecks).
    pub nnz: usize,
    /// Range chunks of `chunk` bits (one lookup) and partial chunks (two
    /// lookups: the chunk and its left shift into the table).
    pub full_chunks: usize,
    pub partial_chunks: usize,
    pub dyn_lookups: usize,
    pub dyn_entries: usize,
    pub fixed_lookups: usize,
    pub fixed_tables: BTreeMap<String, usize>,
    /// Lookups into per-wallet comb tables (rows proved once at Bootstrap,
    /// bound by their commitment) and those tables' entries.
    pub wallet_lookups: usize,
    pub wallet_tables: BTreeMap<String, usize>,
    pub chunk: usize,
    /// Counted (not synthesized) rows: comparison against the field modulus
    /// in canonical decompositions, as in `gadgets.rs`.
    pub counted_rows: usize,
    pub mul_checks: usize,
    /// Constraints, lookups and tables, when recording a real circuit.
    pub rec: Option<Box<Rec>>,
}

/// A recorded circuit: every row, every lookup (table name and tuple) and
/// the tables' entries.
#[derive(Default)]
pub struct Rec {
    pub rows: Vec<(Lc, Lc, Lc)>,
    pub lookups: Vec<(String, Vec<Lc>)>,
    pub tables: BTreeMap<String, Vec<Vec<Fp>>>,
    /// Gadget scope of each row and lookup (`blocks`).
    pub row_scope: Vec<u32>,
    pub lookup_scope: Vec<u32>,
}

impl OpCs {
    pub fn new(chunk: usize) -> Self {
        OpCs {
            vals: vec![Fp::ONE],
            widths: vec![0],
            product_rows: 0,
            linear_rows: 0,
            nnz: 0,
            full_chunks: 0,
            partial_chunks: 0,
            dyn_lookups: 0,
            dyn_entries: 0,
            fixed_lookups: 0,
            fixed_tables: BTreeMap::new(),
            wallet_lookups: 0,
            wallet_tables: BTreeMap::new(),
            chunk,
            counted_rows: 0,
            mul_checks: 0,
            rec: None,
        }
    }
    /// A system that also records its rows, lookups and tables.
    pub fn recording(chunk: usize) -> Self {
        let mut cs = OpCs::new(chunk);
        cs.rec = Some(Box::default());
        cs
    }
    fn record_lookup(
        &mut self,
        table: &str,
        tuple: Vec<Lc>,
        entries: impl FnOnce() -> Vec<Vec<Fp>>,
    ) {
        if let Some(rec) = self.rec.as_mut() {
            if !rec.tables.contains_key(table) {
                rec.tables.insert(table.to_string(), entries());
            }
            rec.lookups.push((table.to_string(), tuple));
            rec.lookup_scope.push(crate::blocks::current());
        }
    }
    pub fn vals(&self) -> &[Fp] {
        &self.vals
    }
    fn alloc(&mut self, v: Fp, bits: u16) -> V {
        self.vals.push(v);
        self.widths.push(bits);
        self.vals.len() - 1
    }
    fn val(&self, lc: &Lc) -> Fp {
        lc.0.iter().map(|(v, c)| self.vals[*v] * c).sum()
    }
    fn enforce(&mut self, a: &Lc, b: &Lc, c: &Lc) {
        assert!(
            self.val(a) * self.val(b) == self.val(c),
            "unsatisfied constraint"
        );
        if let Some(rec) = self.rec.as_mut() {
            rec.rows.push((a.clone(), b.clone(), c.clone()));
            rec.row_scope.push(crate::blocks::current());
        }
        self.product_rows += 1;
        self.nnz += a.nnz() + b.nnz() + c.nnz();
    }
    fn enforce_zero(&mut self, lc: &Lc) {
        assert!(self.val(lc) == Fp::ZERO, "unsatisfied linear constraint");
        if let Some(rec) = self.rec.as_mut() {
            rec.rows
                .push((lc.clone(), Lc::constant(Fp::ONE), Lc::default()));
            rec.row_scope.push(crate::blocks::current());
        }
        self.linear_rows += 1;
        self.nnz += lc.nnz() + 1;
    }
    fn mul(&mut self, a: &Lc, b: &Lc, bits: u16) -> Lc {
        let v = self.alloc(self.val(a) * self.val(b), bits);
        self.enforce(a, b, &Lc::var(v));
        Lc::var(v)
    }
    fn boolean(&mut self, bit: bool) -> Lc {
        let v = self.alloc(if bit { Fp::ONE } else { Fp::ZERO }, 1);
        self.enforce(&Lc::var(v), &Lc::var(v), &Lc::var(v));
        Lc::var(v)
    }
    /// Allocates `x < 2^bits` as range-checked chunks; returns the
    /// recomposition (no constraint: it is linear).
    fn chunks(&mut self, x: &BigUint, bits: usize) -> Lc {
        assert!(x.bits() as usize <= bits, "value fits its declared width");
        let b = self.chunk;
        let mut lc = Lc::default();
        let mut offset = 0;
        while offset < bits {
            let w = b.min(bits - offset);
            let c = (x >> offset) % pow2(w);
            let v = self.alloc(fp_u(&c), w as u16);
            if w == b {
                self.full_chunks += 1
            } else {
                self.partial_chunks += 1
            }
            if self.rec.is_some() {
                let name = format!("range{b}");
                let entries = || (0..1u64 << b).map(|x| vec![Fp::from(x)]).collect();
                self.record_lookup(&name, vec![Lc::var(v)], entries);
                if w < b {
                    self.record_lookup(
                        &name,
                        vec![Lc::var(v).scale(fp_u(&pow2(b - w)))],
                        || unreachable!(),
                    );
                }
            }
            lc = lc.plus(&Lc::var(v).scale(fp_u(&pow2(offset))));
            offset += w;
        }
        lc
    }
    fn dyn_lookup(&mut self) {
        self.dyn_lookups += 1;
    }
    fn fixed_lookup(&mut self) {
        self.fixed_lookups += 1;
    }
    pub fn range_lookups(&self) -> usize {
        self.full_chunks + 2 * self.partial_chunks
    }
}

// ---------------------------------------------------------------------------
// Non-native values: three signed 88-bit limbs with tracked bounds.

#[derive(Clone)]
struct Nn {
    limbs: [Lc; 3],
    lv: [BigInt; 3],
    max: [BigUint; 3],
}

impl Nn {
    fn value(&self) -> BigInt {
        &self.lv[0] + (&self.lv[1] << L) + (&self.lv[2] << (2 * L))
    }
    fn max_value(&self) -> BigUint {
        &self.max[0] + (&self.max[1] << L) + (&self.max[2] << (2 * L))
    }
}

fn limbs_of(x: &BigUint) -> [BigUint; 3] {
    assert!(x.bits() as usize <= 3 * L);
    [x % pow2(L), (x >> L) % pow2(L), x >> (2 * L)]
}

fn nn_const(x: &BigUint) -> Nn {
    let l = limbs_of(x);
    Nn {
        limbs: [
            Lc::constant(fp_u(&l[0])),
            Lc::constant(fp_u(&l[1])),
            Lc::constant(fp_u(&l[2])),
        ],
        lv: [
            BigInt::from(l[0].clone()),
            BigInt::from(l[1].clone()),
            BigInt::from(l[2].clone()),
        ],
        max: l,
    }
}

/// Allocates `0 <= x < 2^bits` (bits <= 264) as range-checked limbs.
fn nn_alloc(cs: &mut OpCs, x: &BigUint, bits: usize) -> Nn {
    let _scope = crate::blocks::scope("nn_alloc");
    let l = limbs_of(x);
    let widths = [
        L.min(bits),
        L.min(bits.saturating_sub(L)),
        bits.saturating_sub(2 * L),
    ];
    let mut limbs: [Lc; 3] = Default::default();
    for k in 0..3 {
        limbs[k] = if widths[k] == 0 {
            Lc::default()
        } else {
            cs.chunks(&l[k], widths[k])
        };
    }
    Nn {
        limbs,
        lv: [
            BigInt::from(l[0].clone()),
            BigInt::from(l[1].clone()),
            BigInt::from(l[2].clone()),
        ],
        max: widths.map(|w| {
            if w == 0 {
                BigUint::zero()
            } else {
                pow2(w) - 1u32
            }
        }),
    }
}

/// Little-endian bit LCs (already boolean-constrained) as a non-native value.
fn nn_from_bits(bits: &[(Lc, bool)]) -> Nn {
    assert!(bits.len() <= 3 * L);
    let mut limbs: [Lc; 3] = Default::default();
    let mut lv = [BigInt::zero(), BigInt::zero(), BigInt::zero()];
    let mut max = [BigUint::zero(), BigUint::zero(), BigUint::zero()];
    for (i, (lc, b)) in bits.iter().enumerate() {
        let (k, j) = (i / L, i % L);
        limbs[k] = limbs[k].plus(&lc.scale(fp_u(&pow2(j))));
        if *b {
            lv[k] += BigInt::from(pow2(j));
        }
        max[k] += pow2(j);
    }
    Nn { limbs, lv, max }
}

/// Small-integer linear combination of non-native values.
fn nn_lin(terms: &[(i64, &Nn)]) -> Nn {
    let mut limbs: [Lc; 3] = Default::default();
    let mut lv = [BigInt::zero(), BigInt::zero(), BigInt::zero()];
    let mut max = [BigUint::zero(), BigUint::zero(), BigUint::zero()];
    for (c, x) in terms {
        let cf = if *c < 0 {
            -Fp::from((-c) as u64)
        } else {
            Fp::from(*c as u64)
        };
        for k in 0..3 {
            limbs[k] = limbs[k].plus(&x.limbs[k].scale(cf));
            lv[k] += BigInt::from(*c) * &x.lv[k];
            max[k] += BigUint::from(c.unsigned_abs()) * &x.max[k];
        }
    }
    Nn { limbs, lv, max }
}

fn bits_of(x: &BigUint) -> u16 {
    x.bits() as u16
}

/// Checks `sum coef_i A_i B_i + lin = 0 (mod m)` (or `= 0` over the integers
/// when `m` is `None`): quotient and carries range-checked, five evaluation
/// products per product term, three carry rows and one native row.
fn mul_check(cs: &mut OpCs, prods: &[(i64, &Nn, &Nn)], lin: &Nn, m: Option<&BigUint>) {
    let _scope = crate::blocks::scope("mul_check");
    let fpm = fp_modulus();
    cs.mul_checks += 1;
    // Integer value and bound of the left-hand side.
    let mut pv = lin.value();
    let mut pmax = lin.max_value();
    for (c, a, b) in prods {
        pv += BigInt::from(*c) * a.value() * b.value();
        pmax += BigUint::from(c.unsigned_abs()) * a.max_value() * b.max_value();
    }
    // Quotient q' = q + 2^qb >= 0.
    let (q_nn, q0) = match m {
        Some(m) => {
            let mi = BigInt::from(m.clone());
            assert!(pv.mod_floor(&mi).is_zero(), "left-hand side is 0 mod m");
            let q = &pv / &mi;
            let qb = (&pmax / m + 1u32).bits() as usize;
            let q0 = pow2(qb);
            let qp = (q + BigInt::from(q0.clone()))
                .to_biguint()
                .expect("offset quotient >= 0");
            (Some(nn_alloc(cs, &qp, qb + 1)), q0)
        }
        None => {
            assert!(pv.is_zero(), "integer identity holds");
            (None, BigUint::zero())
        }
    };
    let m0 = m.cloned().unwrap_or_default();
    let ml = limbs_of(&m0);
    let constant = &q0 * &m0; // E = P + q0 m - q' m
    // Columns 0..=6: Lc and integer value and bound.
    let ncol = 7;
    let mut col_lc: Vec<Lc> = vec![Lc::default(); ncol];
    let mut col_v: Vec<BigInt> = vec![BigInt::zero(); ncol];
    let mut col_max: Vec<BigUint> = vec![BigUint::zero(); ncol];
    let inv2 = Fp::from(2).invert().unwrap();
    let inv3 = Fp::from(3).invert().unwrap();
    for (c, a, b) in prods {
        let cf = if *c < 0 {
            -Fp::from((-c) as u64)
        } else {
            Fp::from(*c as u64)
        };
        // Evaluations at 0, 1, -1, 2, infinity.
        let eval = |x: &Nn, t: i64| -> (Lc, BigInt) {
            match t {
                i64::MAX => (x.limbs[2].clone(), x.lv[2].clone()),
                _ => {
                    let f = |k: usize| BigInt::from(t).pow(k as u32);
                    let lc = (0..3).fold(Lc::default(), |acc, k| {
                        acc.plus(&x.limbs[k].scale(fp_from_big(&f(k))))
                    });
                    let v = (0..3).map(|k| f(k) * &x.lv[k]).sum::<BigInt>();
                    (lc, v)
                }
            }
        };
        let mut pts = Vec::new();
        for t in [0, 1, -1, 2, i64::MAX] {
            let (la, va) = eval(a, t);
            let (lb, vb) = eval(b, t);
            let prod = &va * &vb;
            let bits = bits_of(&prod.magnitude().clone()).max(1);
            if prod.sign() == num_bigint::Sign::Minus {
                // Offset encoding: a negative product is stored as
                // prod + 2^bits (bits + 1 wide), so its entry stays narrow.
                let off = pow2(bits as usize);
                let shifted = (&prod + BigInt::from(off.clone()))
                    .to_biguint()
                    .expect("offset product >= 0");
                let v = Lc::var(cs.alloc(fp_u(&shifted), bits + 1));
                let out = v.minus(&Lc::constant(fp_u(&off)));
                cs.enforce(&la, &lb, &out);
                pts.push(out);
            } else {
                pts.push(cs.mul(&la, &lb, bits));
            }
        }
        let (p0, p1, pm1, p2, pinf) = (&pts[0], &pts[1], &pts[2], &pts[3], &pts[4]);
        let c0 = p0.clone();
        let c4 = pinf.clone();
        let c2 = p1.plus(pm1).scale(inv2).minus(&c0).minus(&c4);
        let s = p1.minus(pm1).scale(inv2);
        let t = p2
            .minus(&c0)
            .minus(&c2.scale(Fp::from(4)))
            .minus(&c4.scale(Fp::from(16)))
            .scale(inv2);
        let c3 = t.minus(&s).scale(inv3);
        let c1 = s.minus(&c3);
        let coeffs = [c0, c1, c2, c3, c4];
        for k in 0..5 {
            let mut v = BigInt::zero();
            let mut mx = BigUint::zero();
            for i in 0..3 {
                for j in 0..3 {
                    if i + j == k {
                        v += &a.lv[i] * &b.lv[j];
                        mx += &a.max[i] * &b.max[j];
                    }
                }
            }
            assert!(
                cs.val(&coeffs[k]) == fp_from_big(&v),
                "interpolated coefficient"
            );
            col_lc[k] = col_lc[k].plus(&coeffs[k].scale(cf));
            col_v[k] += BigInt::from(*c) * v;
            col_max[k] += BigUint::from(c.unsigned_abs()) * mx;
        }
    }
    for k in 0..3 {
        col_lc[k] = col_lc[k].plus(&lin.limbs[k]);
        col_v[k] += &lin.lv[k];
        col_max[k] += &lin.max[k];
    }
    let mut cst = constant.clone();
    for k in 0..ncol {
        let ck = &cst % pow2(L);
        cst >>= L;
        col_lc[k] = col_lc[k].plus(&Lc::constant(fp_u(&ck)));
        col_v[k] += BigInt::from(ck.clone());
        col_max[k] += ck;
    }
    assert!(cst.is_zero(), "constant fits the columns");
    if let Some(q) = &q_nn {
        for i in 0..3 {
            for j in 0..3 {
                let mj = fp_u(&ml[j]);
                col_lc[i + j] = col_lc[i + j].minus(&q.limbs[i].scale(mj));
                col_v[i + j] -= &q.lv[i] * BigInt::from(ml[j].clone());
                col_max[i + j] += &q.max[i] * &ml[j];
            }
        }
    }
    // Carry chain mod 2^264 over columns 0..2.
    let mut carry_lc = Lc::default();
    let mut carry_v = BigInt::zero();
    let mut carry_max = BigUint::zero();
    let half = &fpm >> 1;
    for k in 0..3 {
        let acc = &col_v[k] + &carry_v;
        let (next, rem) = acc.div_rem(&BigInt::from(pow2(L)));
        assert!(rem.is_zero(), "column {k} divisible by 2^88");
        let next_max = (&col_max[k] + &carry_max) / pow2(L) + 1u32;
        assert!(
            &col_max[k] + &carry_max + (&next_max << L) < half,
            "carry row is an integer identity"
        );
        let cb = next_max.bits() as usize;
        let off = pow2(cb);
        let shifted = (next.clone() + BigInt::from(off.clone()))
            .to_biguint()
            .unwrap();
        let chunk_lc = cs.chunks(&shifted, cb + 1);
        let next_lc = chunk_lc.minus(&Lc::constant(fp_u(&off)));
        cs.enforce_zero(
            &col_lc[k]
                .plus(&carry_lc)
                .minus(&next_lc.scale(fp_u(&pow2(L)))),
        );
        carry_lc = next_lc;
        carry_v = next;
        carry_max = next_max;
    }
    // Native row: sum_k col_k 2^(88 k) = 0 in Fp.
    let mut native = Lc::default();
    let mut nv = BigInt::zero();
    for k in 0..ncol {
        native = native.plus(&col_lc[k].scale(fp_u(&pow2(L * k))));
        nv += &col_v[k] << (L * k);
    }
    assert!(nv.is_zero(), "E = 0 over the integers");
    cs.enforce_zero(&native);
    // CRT bound: |E| <= |P| + |q0 - q'| m <= pmax + 2^qb m < 2^264 * Fp.
    let emax: BigUint = &pmax + &q0 * &m0;
    let bound = (pow2(3 * L) * &fpm) >> 1;
    assert!(
        emax < bound,
        "CRT bound holds: |E| < 2^{} vs bound 2^{} (products {}, qb {:?})",
        emax.bits(),
        bound.bits(),
        prods.len(),
        q_nn.as_ref().map(|q| q.max_value().bits())
    );
}

// ---------------------------------------------------------------------------
// P-256 points over non-native coordinates.

#[derive(Clone)]
struct Ppt {
    x: Nn,
    y: Nn,
    nat: Point,
}

fn pt_alloc(cs: &mut OpCs, pt: &Point) -> Ppt {
    Ppt {
        x: nn_alloc(cs, &pt.x, 256),
        y: nn_alloc(cs, &pt.y, 256),
        nat: pt.clone(),
    }
}

fn pt_const(pt: &Point) -> Ppt {
    Ppt {
        x: nn_const(&pt.x),
        y: nn_const(&pt.y),
        nat: pt.clone(),
    }
}

fn same_point(a: &Ppt) {
    let p = p256::p();
    assert_eq!(p256::modp(&a.x.value(), &p), a.nat.x, "x matches native");
    assert_eq!(p256::modp(&a.y.value(), &p), a.nat.y, "y matches native");
}

/// Where a looked-up table lives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Table {
    /// Entries computed in this circuit (committed before the challenges).
    Dynamic,
    /// Circuit constants.
    Fixed,
    /// Per-wallet rows proved once at Bootstrap and bound by their commitment.
    Wallet,
}

/// Looks up a point from a table: its six limbs are fresh witnesses bound by
/// the lookup to a table row (whose limbs were range-checked, are constant,
/// or were proved at Bootstrap).
fn pt_select(cs: &mut OpCs, row: &Ppt, table: Table) -> Ppt {
    let _scope = crate::blocks::scope("pt_select");
    let fresh = |x: &Nn, cs: &mut OpCs| -> Nn {
        let mut limbs: [Lc; 3] = Default::default();
        for k in 0..3 {
            let bits = (x.max[k].bits() as u16).max(1);
            limbs[k] = Lc::var(cs.alloc(fp_from_big(&x.lv[k]), bits));
        }
        Nn {
            limbs,
            lv: x.lv.clone(),
            max: x.max.clone(),
        }
    };
    let out = Ppt {
        x: fresh(&row.x, cs),
        y: fresh(&row.y, cs),
        nat: row.nat.clone(),
    };
    match table {
        Table::Dynamic => cs.dyn_lookup(),
        Table::Fixed => cs.fixed_lookup(),
        Table::Wallet => cs.wallet_lookups += 1,
    }
    out
}

/// Chord (or tangent, when a = c) slope of two curve points.
fn slope(a: &Point, c: &Point) -> BigUint {
    let p = p256::p();
    if a == c {
        let num = (BigUint::from(3u32) * &a.x * &a.x + &p - 3u32) % &p;
        num * p256::inv(&(BigUint::from(2u32) * &a.y % &p), &p) % &p
    } else {
        let dx = (&c.x + &p - &a.x) % &p;
        let dy = (&c.y + &p - &a.y) % &p;
        dy * p256::inv(&dx, &p) % &p
    }
}

fn group_add(a: &Point, c: &Point) -> Point {
    if a == c {
        p256::double(a)
    } else {
        p256::add(a, c)
    }
}

/// Both chord-slope equations for curve points a, c:
///   lam (x_c - x_a) = y_c - y_a  and  lam (y_a + y_c) = x_a^2 + x_a x_c + x_c^2 - 3.
/// The first pins lam unless x_a = x_c; then it forces y_a = y_c and the
/// second pins lam to the tangent (y != 0 on P-256). So no slope is a free
/// 0/0 witness; only c = -a is unsatisfiable (and never reached honestly).
fn slope_checks(cs: &mut OpCs, l: &Nn, a: &Ppt, c: &Ppt) {
    let _scope = crate::blocks::scope("slope_checks");
    let p = p256::p();
    mul_check(
        cs,
        &[(1, l, &nn_lin(&[(1, &c.x), (-1, &a.x)]))],
        &nn_lin(&[(-1, &c.y), (1, &a.y)]),
        Some(&p),
    );
    mul_check(
        cs,
        &[
            (1, l, &nn_lin(&[(1, &a.y), (1, &c.y)])),
            (-1, &a.x, &a.x),
            (-1, &a.x, &c.x),
            (-1, &c.x, &c.x),
        ],
        &nn_const(&BigUint::from(3u32)),
        Some(&p),
    );
}

/// Addition with a caller-supplied slope (tests inject wrong slopes).
fn pt_add_with_slope(cs: &mut OpCs, a: &Ppt, c: &Ppt, lam: &BigUint) -> Ppt {
    let _scope = crate::blocks::scope("pt_add");
    let p = p256::p();
    let nat = group_add(&a.nat, &c.nat);
    let l = nn_alloc(cs, lam, 256);
    slope_checks(cs, &l, a, c);
    let x3v = (lam * lam % &p + &p * 2u32 - &a.nat.x - &c.nat.x) % &p;
    let x3 = nn_alloc(cs, &x3v, 256);
    mul_check(
        cs,
        &[(1, &l, &l)],
        &nn_lin(&[(-1, &a.x), (-1, &c.x), (-1, &x3)]),
        Some(&p),
    );
    let y3v = (lam * ((&a.nat.x + &p - &x3v) % &p) % &p + &p - &a.nat.y) % &p;
    let y3 = nn_alloc(cs, &y3v, 256);
    mul_check(
        cs,
        &[(1, &l, &nn_lin(&[(1, &a.x), (-1, &x3)]))],
        &nn_lin(&[(-1, &a.y), (-1, &y3)]),
        Some(&p),
    );
    let out = Ppt { x: x3, y: y3, nat };
    same_point(&out);
    out
}

fn pt_add(cs: &mut OpCs, a: &Ppt, c: &Ppt) -> Ppt {
    let lam = slope(&a.nat, &c.nat);
    pt_add_with_slope(cs, a, c, &lam)
}

fn pt_double(cs: &mut OpCs, a: &Ppt) -> Ppt {
    let _scope = crate::blocks::scope("pt_double");
    let p = p256::p();
    let nat = p256::double(&a.nat);
    let num = (BigUint::from(3u32) * &a.nat.x * &a.nat.x + &p - 3u32) % &p;
    let lam = num * p256::inv(&(BigUint::from(2u32) * &a.nat.y % &p), &p) % &p;
    let l = nn_alloc(cs, &lam, 256);
    // 2 lam y - 3 x^2 + 3 = 0 (a = -3)
    mul_check(
        cs,
        &[(2, &l, &a.y), (-3, &a.x, &a.x)],
        &nn_const(&BigUint::from(3u32)),
        Some(&p),
    );
    let x3 = nn_alloc(cs, &nat.x, 256);
    mul_check(
        cs,
        &[(1, &l, &l)],
        &nn_lin(&[(-2, &a.x), (-1, &x3)]),
        Some(&p),
    );
    let y3 = nn_alloc(cs, &nat.y, 256);
    mul_check(
        cs,
        &[(1, &l, &nn_lin(&[(1, &a.x), (-1, &x3)]))],
        &nn_lin(&[(-1, &a.y), (-1, &y3)]),
        Some(&p),
    );
    let out = Ppt { x: x3, y: y3, nat };
    same_point(&out);
    out
}

/// `2 acc + s` without the intermediate y (five checks).
fn pt_double_add(cs: &mut OpCs, acc: &Ppt, s: &Ppt) -> Ppt {
    let _scope = crate::blocks::scope("pt_double_add");
    let p = p256::p();
    let r = group_add(&acc.nat, &s.nat);
    let nat = p256::add(&r, &acc.nat);
    let l1v = slope(&acc.nat, &s.nat);
    let l1 = nn_alloc(cs, &l1v, 256);
    slope_checks(cs, &l1, acc, s);
    let xr = nn_alloc(cs, &r.x, 256);
    mul_check(
        cs,
        &[(1, &l1, &l1)],
        &nn_lin(&[(-1, &acc.x), (-1, &s.x), (-1, &xr)]),
        Some(&p),
    );
    // (l1 + l2)(x_acc - x_r) = 2 y_acc: pinned, since y_acc != 0.
    let den = (&acc.nat.x + &p - &r.x) % &p;
    let l2v = (BigUint::from(2u32) * &acc.nat.y % &p * p256::inv(&den, &p) + &p - &l1v) % &p;
    let l2 = nn_alloc(cs, &l2v, 256);
    mul_check(
        cs,
        &[(
            1,
            &nn_lin(&[(1, &l1), (1, &l2)]),
            &nn_lin(&[(1, &acc.x), (-1, &xr)]),
        )],
        &nn_lin(&[(-2, &acc.y)]),
        Some(&p),
    );
    let xo = nn_alloc(cs, &nat.x, 256);
    mul_check(
        cs,
        &[(1, &l2, &l2)],
        &nn_lin(&[(-1, &xr), (-1, &acc.x), (-1, &xo)]),
        Some(&p),
    );
    let yo = nn_alloc(cs, &nat.y, 256);
    mul_check(
        cs,
        &[(1, &l2, &nn_lin(&[(1, &acc.x), (-1, &xo)]))],
        &nn_lin(&[(-1, &acc.y), (-1, &yo)]),
        Some(&p),
    );
    let out = Ppt { x: xo, y: yo, nat };
    same_point(&out);
    out
}

fn pt_on_curve(cs: &mut OpCs, a: &Ppt) {
    let _scope = crate::blocks::scope("pt_on_curve");
    let p = p256::p();
    let t = nn_alloc(cs, &(&a.nat.x * &a.nat.x % &p), 256);
    mul_check(cs, &[(1, &a.x, &a.x)], &nn_lin(&[(-1, &t)]), Some(&p));
    // y^2 - x t + 3 x - b = 0
    let lin = nn_lin(&[(3, &a.x), (-1, &nn_const(&p256::b()))]);
    mul_check(cs, &[(1, &a.y, &a.y), (-1, &a.x, &t)], &lin, Some(&p));
}

/// `a = k (mod p)` for a constant point coordinate.
fn nn_eq_const(cs: &mut OpCs, a: &Nn, k: &BigUint) {
    let _scope = crate::blocks::scope("nn_eq_const");
    mul_check(
        cs,
        &[],
        &nn_lin(&[(1, a), (-1, &nn_const(k))]),
        Some(&p256::p()),
    );
}

/// Odd multiples {1, 3, ..., 15} P and their negatives, indexed by the nibble
/// c with digit d = 2 c - 15.
fn odd_table(cs: &mut OpCs, base: &Ppt, fixed: bool) -> Vec<Ppt> {
    let _scope = crate::blocks::scope("odd_table");
    let p = p256::p();
    let mut odd = vec![base.clone()];
    if fixed {
        let two = p256::double(&base.nat);
        for _ in 1..8 {
            let next = p256::add(&odd.last().unwrap().nat, &two);
            odd.push(pt_const(&next));
        }
    } else {
        let two = pt_double(cs, base);
        for _ in 1..8 {
            let next = pt_add(cs, odd.last().unwrap(), &two);
            odd.push(next);
        }
        cs.dyn_entries += 16;
    }
    (0..16)
        .map(|c| {
            let d = 2 * c as i64 - 15;
            let e = &odd[(d.unsigned_abs() as usize - 1) / 2];
            if d > 0 {
                e.clone()
            } else {
                let y = nn_lin(&[(1, &nn_const(&p)), (-1, &e.y)]);
                Ppt {
                    x: e.x.clone(),
                    y,
                    nat: p256::neg(&e.nat),
                }
            }
        })
        .collect()
}

const WINDOWS: usize = 33; // 4-bit windows over 132-bit odd scalars

/// Base-16 digits c_i of (t + 16^33 - 1) / 2 for odd |t| < 2^132.
fn odd_nibbles(t: &BigInt) -> Vec<usize> {
    assert!(t.is_odd());
    let k: BigInt = BigInt::from(pow2(4 * WINDOWS)) - BigInt::one();
    let sum: BigInt = t + &k;
    let c: BigUint = (sum / BigInt::from(2)).to_biguint().unwrap();
    (0..WINDOWS)
        .map(|i: usize| -> usize { ((&c >> (4 * i)) % BigUint::from(16u32)).to_usize().unwrap() })
        .collect()
}

/// Scalar t = 2 c - (16^33 - 1) from nibble witnesses as a non-native value.
fn nn_from_nibbles(cs: &mut OpCs, nib: &[usize]) -> (Vec<V>, Nn) {
    let _scope = crate::blocks::scope("nn_from_nibbles");
    let vars: Vec<V> = nib
        .iter()
        .map(|c| cs.alloc(Fp::from(*c as u64), 4))
        .collect();
    let mut limbs: [Lc; 3] = Default::default();
    let mut lv = [BigInt::zero(), BigInt::zero(), BigInt::zero()];
    let mut max = [BigUint::zero(), BigUint::zero(), BigUint::zero()];
    for (i, (v, c)) in vars.iter().zip(nib).enumerate() {
        let (k, j) = ((4 * i) / L, (4 * i) % L);
        limbs[k] = limbs[k].plus(&Lc::var(*v).scale(fp_u(&(pow2(j) * 2u32))));
        lv[k] += BigInt::from(*c as u64 * 2) << j;
        max[k] += pow2(j) * 30u32;
    }
    let kconst = limbs_of(&(pow2(4 * WINDOWS) - 1u32));
    for k in 0..3 {
        limbs[k] = limbs[k].minus(&Lc::constant(fp_u(&kconst[k])));
        lv[k] -= BigInt::from(kconst[k].clone());
        max[k] = max[k].clone().max(kconst[k].clone());
    }
    (vars, Nn { limbs, lv, max })
}

/// Unsigned nibbles of a 128-bit value (32 windows).
fn nibbles_128(x: &BigUint) -> Vec<usize> {
    (0..32usize)
        .map(|i: usize| -> usize { ((x >> (4 * i)) % BigUint::from(16u32)).to_usize().unwrap() })
        .collect()
}

/// ECDSA P-256 verification of (r, s) on digest z under key q.
/// `z_bits`: SHA-256 output bits (little-endian integer order) when the
/// digest is computed in-circuit; `fixed_key`: q is a circuit constant.
fn ecdsa(
    cs: &mut OpCs,
    q: &Point,
    z: &BigUint,
    z_bits: Option<&[(Lc, bool)]>,
    sig: &Signature,
    fixed_key: bool,
) {
    let _scope = crate::blocks::scope("ecdsa");
    let n = p256::n();
    let p = p256::p();
    assert!(p256::verify(q, z, sig), "native ECDSA accepts");
    let zn = match z_bits {
        Some(bits) => nn_from_bits(bits),
        None => nn_alloc(cs, z, 256),
    };
    assert_eq!(zn.value(), BigInt::from(z.clone()));
    let r = nn_alloc(cs, &sig.r, 256);
    let s = nn_alloc(cs, &sig.s, 256);
    // 1 <= r, s <= n - 1.
    for x in [&r, &s] {
        let d = nn_alloc(cs, &(&n - 1u32 - p256::modp(&x.value(), &n)), 256);
        mul_check(
            cs,
            &[],
            &nn_lin(&[(1, &d), (1, x), (-1, &nn_const(&(&n - 1u32)))]),
            None,
        );
    }
    let rinv = nn_alloc(cs, &p256::inv(&sig.r, &n), 256);
    mul_check(
        cs,
        &[(1, &r, &rinv)],
        &nn_lin(&[(-1, &nn_const(&BigUint::one()))]),
        Some(&n),
    );
    let w = p256::inv(&sig.s, &n);
    let u1v = z * &w % &n;
    let u2v = &sig.r * &w % &n;
    let u1 = nn_alloc(cs, &u1v, 256);
    let u2 = nn_alloc(cs, &u2v, 256);
    mul_check(cs, &[(1, &u1, &s)], &nn_lin(&[(-1, &zn)]), Some(&n));
    mul_check(cs, &[(1, &u2, &s)], &nn_lin(&[(-1, &r)]), Some(&n));
    // R' = u1 G + u2 Q, x(R') = r + flag n.
    let rp = p256::add_any(p256::mul(&p256::generator(), &u1v), p256::mul(q, &u2v)).unwrap();
    let flag = rp.x >= n;
    assert_eq!(
        &rp.x,
        &(&sig.r + if flag { n.clone() } else { BigUint::zero() })
    );
    // x(R') is a fresh 256-bit value with x = r + flag n over the integers.
    let fl = cs.boolean(flag);
    let nl = limbs_of(&n);
    let mut rn = r.clone();
    for k in 0..3 {
        rn.limbs[k] = rn.limbs[k].plus(&fl.scale(fp_u(&nl[k])));
        if flag {
            rn.lv[k] += BigInt::from(nl[k].clone());
        }
        rn.max[k] += &nl[k];
    }
    let xr = nn_alloc(cs, &rp.x, 256);
    mul_check(cs, &[], &nn_lin(&[(1, &xr), (-1, &rn)]), None);
    let yr = nn_alloc(cs, &rp.y, 256);
    let rpt = Ppt {
        x: xr,
        y: yr,
        nat: rp.clone(),
    };
    pt_on_curve(cs, &rpt);
    let d = nn_alloc(cs, &(&p - 1u32 - &rp.x), 256);
    mul_check(
        cs,
        &[],
        &nn_lin(&[(1, &d), (1, &rpt.x), (-1, &nn_const(&(&p - 1u32)))]),
        None,
    );
    let qpt = if fixed_key {
        pt_const(q)
    } else {
        let qa = pt_alloc(cs, q);
        pt_on_curve(cs, &qa);
        qa
    };
    // Half-size scalars: v = v' u2 (mod n), w = v' u1 (mod n);
    // check v' R' - v Q - w G = O.
    let (v, vp) = p256::half_scalars(&u2v);
    let wv = p256::modp(&(&vp * BigInt::from(u1v.clone())), &n);
    let t1 = vp.clone();
    let t2 = -v.clone();
    let (_, t1n) = nn_from_nibbles(cs, &odd_nibbles(&t1));
    let (_, t2n) = nn_from_nibbles(cs, &odd_nibbles(&t2));
    assert_eq!(t1n.value(), t1);
    assert_eq!(t2n.value(), t2);
    let (wlo, whi) = (&wv % pow2(128), &wv >> 128);
    let (enib, fnib) = (nibbles_128(&wlo), nibbles_128(&whi));
    let mut wbits_lc: [Lc; 3] = Default::default();
    let mut wl = [BigInt::zero(), BigInt::zero(), BigInt::zero()];
    let mut wm = [BigUint::zero(), BigUint::zero(), BigUint::zero()];
    for (i, c) in enib.iter().chain(fnib.iter()).enumerate() {
        let var = cs.alloc(Fp::from(*c as u64), 4);
        let (k, j) = ((4 * i) / L, (4 * i) % L);
        wbits_lc[k] = wbits_lc[k].plus(&Lc::var(var).scale(fp_u(&pow2(j))));
        wl[k] += BigInt::from(*c as u64) << j;
        wm[k] += pow2(j) * 15u32;
    }
    let wn = Nn {
        limbs: wbits_lc,
        lv: wl,
        max: wm,
    };
    assert_eq!(wn.value(), BigInt::from(wv.clone()));
    // v - v' u2 = -t2 - t1 u2 = 0 and w - v' u1 = w - t1 u1 = 0 (mod n).
    mul_check(cs, &[(-1, &t1n, &u2)], &nn_lin(&[(-1, &t2n)]), Some(&n));
    mul_check(cs, &[(-1, &t1n, &u1)], &nn_lin(&[(1, &wn)]), Some(&n));
    // Tables.
    let tr = odd_table(cs, &rpt, false);
    let tq = odd_table(cs, &qpt, fixed_key);
    // Fixed table: entry (e, f) = -((e + 16) G + f 2^128 G).
    let g = p256::generator();
    let g2 = p256::mul(&g, &pow2(128)).unwrap();
    let gg = |e: usize, f: usize| -> Ppt {
        let a = p256::mul(&g, &BigUint::from(e as u64 + 16)).unwrap();
        let b = p256::mul(&g2, &BigUint::from(f as u64));
        pt_const(&p256::neg(&p256::add_any(Some(a), b).unwrap()))
    };
    cs.fixed_tables.insert("p256_g_2^128g".into(), 256);
    if fixed_key {
        cs.fixed_tables.insert("p256_fixed_key_odd".into(), 16);
    }
    let n1 = odd_nibbles(&t1);
    let n2 = odd_nibbles(&t2);
    let top = WINDOWS - 1;
    let qtable = if fixed_key {
        Table::Fixed
    } else {
        Table::Dynamic
    };
    let a1 = pt_select(cs, &tr[n1[top]], Table::Dynamic);
    let a2 = pt_select(cs, &tq[n2[top]], qtable);
    let mut acc = pt_add(cs, &a1, &a2);
    acc = pt_add(cs, &acc, &gg(0, 0)); // top window of w is zero: a constant
    for i in (0..top).rev() {
        acc = pt_double(cs, &acc);
        acc = pt_double(cs, &acc);
        acc = pt_double(cs, &acc);
        let s1 = pt_select(cs, &tr[n1[i]], Table::Dynamic);
        acc = pt_double_add(cs, &acc, &s1);
        let s2 = pt_select(cs, &tq[n2[i]], qtable);
        acc = pt_add(cs, &acc, &s2);
        let (e, f) = if i < 32 { (enib[i], fnib[i]) } else { (0, 0) };
        let s3 = pt_select(cs, &gg(e, f), Table::Fixed);
        acc = pt_add(cs, &acc, &s3);
    }
    // acc = -K G with K = 16 (16^33 - 1) / 15.
    let kk = (pow2(4 * WINDOWS) - 1u32) * 16u32 / 15u32;
    let expected = p256::neg(&p256::mul(&g, &(&kk % &n)).unwrap());
    assert_eq!(acc.nat, expected, "v' R' - v Q - w G = O");
    nn_eq_const(cs, &acc.x, &expected.x);
    nn_eq_const(cs, &acc.y, &expected.y);
}

// ---------------------------------------------------------------------------
// ECDSA over precomputed comb tables (lever 1, synthesized).
//
// With the key's tables precomputed, R' recovery and half-size scalars are not
// needed: the circuit computes P = u1 G + u2 Q directly and checks
// x(P) = r + flag n. Each scalar u is written u = 2 B - (2^256 - 1) (mod n)
// with B = (u + 2^256 - 1) / 2 mod n, so u = sum_t (2 b_t - 1) 2^t with signed
// one-bit digits (no zero digit, no parity condition). The 256 digits split
// into m chunks of c = 256 / m bits over the bases B_k = 2^(c k) X, and each
// table combines 8 bases: entry(idx) = sum_k (2 bit_k(idx) - 1) B_(8j+k).
// One loop of c windows then costs a double-add and (2 m / 8 - 1) additions
// per window.
//
// G's tables are circuit constants. Q's tables are per-wallet rows, proved
// once at Bootstrap (`bootstrap_cost`) and bound by their row commitment,
// which each verifier of the proof compares with the wallet's state; a fixed
// root key uses constant tables like G.

/// Comb tables of X with `m` bases (8 bases per table, 256 entries each),
/// computed natively along a Gray code.
pub fn comb_tables(x: &Point, m: usize) -> Vec<Vec<Point>> {
    assert!(m % 8 == 0 && 256 % m == 0);
    let c = 256 / m;
    let mut bases = vec![x.clone()];
    for _ in 1..m {
        let mut b = bases.last().unwrap().clone();
        for _ in 0..c {
            b = p256::double(&b);
        }
        bases.push(b);
    }
    (0..m / 8)
        .map(|j| {
            let group = &bases[8 * j..8 * j + 8];
            let twice: Vec<Point> = group.iter().map(p256::double).collect();
            let mut table: Vec<Option<Point>> = vec![None; 256];
            let mut cur = group
                .iter()
                .fold(None, |acc, b| p256::add_any(acc, Some(p256::neg(b))))
                .expect("nonzero entry");
            table[0] = Some(cur.clone());
            let mut prev = 0usize;
            for t in 1..256usize {
                let gray = t ^ (t >> 1);
                let k = (gray ^ prev).trailing_zeros() as usize;
                let term = if gray & (1 << k) != 0 {
                    twice[k].clone()
                } else {
                    p256::neg(&twice[k])
                };
                cur = p256::add_any(Some(cur), Some(term)).expect("nonzero entry");
                table[gray] = Some(cur.clone());
                prev = gray;
            }
            table.into_iter().map(|e| e.unwrap()).collect()
        })
        .collect()
}

/// The fixed tables of G, cached per base count.
fn g_comb_tables(m: usize) -> std::sync::Arc<Vec<Vec<Point>>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<BTreeMap<usize, Arc<Vec<Vec<Point>>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = cache.lock().unwrap();
    guard
        .entry(m)
        .or_insert_with(|| Arc::new(comb_tables(&p256::generator(), m)))
        .clone()
}

/// Signed-digit bits b_t of u: u = sum_t (2 b_t - 1) 2^t (mod n).
fn comb_bits(u: &BigUint) -> Vec<bool> {
    let n = p256::n();
    let inv2 = (&n + 1u32) >> 1;
    let b = (u + pow2(256) - 1u32) % &n * &inv2 % &n;
    (0..256)
        .map(|t| ((&b >> t) % 2u32) == BigUint::one())
        .collect()
}

/// A per-wallet table row: 88-bit limb witnesses in the table's dedicated
/// rows (only their values and bounds matter for the lookup).
fn pt_row(pt: &Point) -> Ppt {
    let row = |v: &BigUint| Nn {
        limbs: Default::default(),
        lv: limbs_of(v).map(BigInt::from),
        max: [pow2(L) - 1u32, pow2(L) - 1u32, pow2(256 - 2 * L) - 1u32],
    };
    Ppt {
        x: row(&pt.x),
        y: row(&pt.y),
        nat: pt.clone(),
    }
}

/// How a signature's key is handled in-circuit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SigKey {
    /// A key known only at proving time (gate 1c's method).
    Variable,
    /// A circuit-constant key with gate 1c's method (fixed odd table).
    FixedOdd,
    /// The prover's own key over per-wallet comb tables of m bases.
    Own(usize),
    /// A circuit-constant key over fixed comb tables of m bases.
    FixedComb(usize),
}

/// ECDSA P-256 verification of (r, s) on z under q over comb tables:
/// P = u1 G + u2 Q in one loop, then x(P) = r + flag n.
fn ecdsa_comb(
    cs: &mut OpCs,
    q: &Point,
    z: &BigUint,
    z_bits: Option<&[(Lc, bool)]>,
    sig: &Signature,
    key: SigKey,
) {
    let _scope = crate::blocks::scope("ecdsa_comb");
    let (m, qkind) = match key {
        SigKey::Own(m) => (m, Table::Wallet),
        SigKey::FixedComb(m) => (m, Table::Fixed),
        _ => unreachable!("comb keys only"),
    };
    let n = p256::n();
    let p = p256::p();
    let zn = match z_bits {
        Some(bits) => nn_from_bits(bits),
        None => nn_alloc(cs, z, 256),
    };
    assert_eq!(zn.value(), BigInt::from(z.clone()));
    let r = nn_alloc(cs, &sig.r, 256);
    let s = nn_alloc(cs, &sig.s, 256);
    // r, s <= n - 1; r != 0 by its inverse; s != 0 follows from u2 s = r.
    for x in [&r, &s] {
        let d = nn_alloc(cs, &(&n - 1u32 - p256::modp(&x.value(), &n)), 256);
        mul_check(
            cs,
            &[],
            &nn_lin(&[(1, &d), (1, x), (-1, &nn_const(&(&n - 1u32)))]),
            None,
        );
    }
    let rinv = nn_alloc(cs, &p256::inv(&sig.r, &n), 256);
    mul_check(
        cs,
        &[(1, &r, &rinv)],
        &nn_lin(&[(-1, &nn_const(&BigUint::one()))]),
        Some(&n),
    );
    let w = p256::inv(&sig.s, &n);
    let u1v = z * &w % &n;
    let u2v = &sig.r * &w % &n;
    // Digit bits as booleans; u = 2 B - (2^256 - 1) is linear in them.
    let mut digits = |u: &BigUint| -> (Vec<bool>, Vec<Lc>, Nn) {
        let bits = comb_bits(u);
        let lcs: Vec<(Lc, bool)> = bits.iter().map(|b| (cs.boolean(*b), *b)).collect();
        let bn = nn_from_bits(&lcs);
        (
            bits,
            lcs.into_iter().map(|(l, _)| l).collect(),
            nn_lin(&[(2, &bn), (-1, &nn_const(&(pow2(256) - 1u32)))]),
        )
    };
    let (b1, l1, u1) = digits(&u1v);
    let (b2, l2, u2) = digits(&u2v);
    mul_check(cs, &[(1, &u1, &s)], &nn_lin(&[(-1, &zn)]), Some(&n));
    mul_check(cs, &[(1, &u2, &s)], &nn_lin(&[(-1, &r)]), Some(&n));
    // Tables.
    let gt = g_comb_tables(m);
    let qt = comb_tables(q, m);
    let c = 256 / m;
    for j in 0..m / 8 {
        cs.fixed_tables.insert(format!("p256_g_comb{m}_{j}"), 256);
        if qkind == Table::Wallet {
            cs.wallet_tables.insert(format!("own_key_comb{m}_{j}"), 256);
        } else {
            cs.fixed_tables.insert(format!("root_key_comb{m}_{j}"), 256);
        }
    }
    // Window i, table j: idx = sum_k b[c (8 j + k) + i] 2^k (linear in the bits).
    let index = |bits: &[bool], j: usize, i: usize| -> usize {
        (0..8)
            .filter(|k| bits[c * (8 * j + k) + i])
            .map(|k| 1usize << k)
            .sum()
    };
    // Recorded lookups: (idx, x limbs, y limbs) against the table's entries.
    let idx_lc = |lcs: &[Lc], j: usize, i: usize| -> Lc {
        (0..8).fold(Lc::default(), |acc, k| {
            acc.plus(&lcs[c * (8 * j + k) + i].scale(Fp::from(1u64 << k)))
        })
    };
    let entries_of = |table: &[Point]| -> Vec<Vec<Fp>> {
        table
            .iter()
            .enumerate()
            .map(|(idx, pt)| {
                let mut e = vec![Fp::from(idx as u64)];
                e.extend(limbs_of(&pt.x).iter().map(fp_u));
                e.extend(limbs_of(&pt.y).iter().map(fp_u));
                e
            })
            .collect()
    };
    let tuple = |idx: Lc, pt: &Ppt| -> Vec<Lc> {
        let mut t = vec![idx];
        t.extend(pt.x.limbs.iter().cloned());
        t.extend(pt.y.limbs.iter().cloned());
        t
    };
    let qname = |j: usize| {
        format!(
            "comb{m}_key_{:x}_{j}",
            (&q.x % 0xffff_ffffu32).to_u32().unwrap()
        )
    };
    let mut acc: Option<Ppt> = None;
    for i in (0..c).rev() {
        let _step = crate::blocks::scope("comb_step");
        let mut entries = Vec::with_capacity(m / 4);
        for j in 0..m / 8 {
            let e = &qt[j][index(&b2, j, i)];
            let row = if qkind == Table::Wallet {
                pt_row(e)
            } else {
                pt_const(e)
            };
            let sel = pt_select(cs, &row, qkind);
            if cs.rec.is_some() {
                let t = tuple(idx_lc(&l2, j, i), &sel);
                cs.record_lookup(&qname(j), t, || entries_of(&qt[j]));
            }
            entries.push(sel);
        }
        for j in 0..m / 8 {
            let sel = pt_select(cs, &pt_const(&gt[j][index(&b1, j, i)]), Table::Fixed);
            if cs.rec.is_some() {
                let t = tuple(idx_lc(&l1, j, i), &sel);
                cs.record_lookup(&format!("comb{m}_g_{j}"), t, || entries_of(&gt[j]));
            }
            entries.push(sel);
        }
        let mut it = entries.into_iter();
        let first = it.next().unwrap();
        let mut a = match acc {
            None => first,
            Some(prev) => pt_double_add(cs, &prev, &first),
        };
        for e in it {
            a = pt_add(cs, &a, &e);
        }
        acc = Some(a);
    }
    let acc = acc.unwrap();
    let expected = p256::add_any(p256::mul(&p256::generator(), &u1v), p256::mul(q, &u2v)).unwrap();
    assert_eq!(acc.nat, expected, "comb computes u1 G + u2 Q");
    // x(P) < p and x(P) = r + flag n over the integers.
    let d = nn_alloc(cs, &(&p - 1u32 - &acc.nat.x), 256);
    mul_check(
        cs,
        &[],
        &nn_lin(&[(1, &d), (1, &acc.x), (-1, &nn_const(&(&p - 1u32)))]),
        None,
    );
    let flag = acc.nat.x >= n;
    let fl = cs.boolean(flag);
    let nl = limbs_of(&n);
    let mut rn = r.clone();
    for k in 0..3 {
        rn.limbs[k] = rn.limbs[k].plus(&fl.scale(fp_u(&nl[k])));
        if flag {
            rn.lv[k] += BigInt::from(nl[k].clone());
        }
        rn.max[k] += &nl[k];
    }
    mul_check(cs, &[], &nn_lin(&[(1, &acc.x), (-1, &rn)]), None);
}

/// One-time Bootstrap proof of a wallet's comb tables: Q on the curve, the
/// bases 2^(c k) Q by doublings, each table along a Gray code.
pub fn bootstrap_cost(chunk: usize, m: usize) -> Cost {
    let mut cs = OpCs::new(chunk);
    let n = p256::n();
    let p = p256::p();
    let q = p256::mul(&p256::generator(), &p256::scalar_below("bootstrap-d", &n)).unwrap();
    let qa = pt_alloc(&mut cs, &q);
    pt_on_curve(&mut cs, &qa);
    let c = 256 / m;
    let mut bases = vec![qa];
    for _ in 1..m {
        let mut b = bases.last().unwrap().clone();
        for _ in 0..c {
            b = pt_double(&mut cs, &b);
        }
        bases.push(b);
    }
    let neg = |a: &Ppt| Ppt {
        x: a.x.clone(),
        y: nn_lin(&[(1, &nn_const(&p)), (-1, &a.y)]),
        nat: p256::neg(&a.nat),
    };
    let native = comb_tables(&q, m);
    for j in 0..m / 8 {
        let group: Vec<Ppt> = bases[8 * j..8 * j + 8].to_vec();
        let mut twice = Vec::with_capacity(8);
        for b in &group {
            twice.push(pt_double(&mut cs, b));
        }
        let mut cur = neg(&group[0]);
        for b in &group[1..] {
            cur = pt_add(&mut cs, &cur, &neg(b));
        }
        assert_eq!(cur.nat, native[j][0]);
        let mut prev = 0usize;
        for t in 1..256usize {
            let gray = t ^ (t >> 1);
            let k = (gray ^ prev).trailing_zeros() as usize;
            let term = if gray & (1 << k) != 0 {
                twice[k].clone()
            } else {
                neg(&twice[k])
            };
            cur = pt_add(&mut cs, &cur, &term);
            assert_eq!(cur.nat, native[j][gray]);
            prev = gray;
        }
    }
    Cost::from_cs(&cs)
}

// ---------------------------------------------------------------------------
// SHA-256, bit-level with constant folding.

#[derive(Clone)]
enum Bit {
    C(bool),
    V(Lc, bool),
}

impl Bit {
    fn value(&self) -> bool {
        match self {
            Bit::C(b) | Bit::V(_, b) => *b,
        }
    }
    fn lc(&self) -> Lc {
        match self {
            Bit::C(b) => Lc::constant(if *b { Fp::ONE } else { Fp::ZERO }),
            Bit::V(l, _) => l.clone(),
        }
    }
}

fn bnot(a: &Bit) -> Bit {
    match a {
        Bit::C(b) => Bit::C(!b),
        Bit::V(l, b) => Bit::V(Lc::constant(Fp::ONE).minus(l), !b),
    }
}

fn band(cs: &mut OpCs, a: &Bit, b: &Bit) -> Bit {
    let _scope = crate::blocks::scope("band");
    match (a, b) {
        (Bit::C(false), _) | (_, Bit::C(false)) => Bit::C(false),
        (Bit::C(true), x) | (x, Bit::C(true)) => x.clone(),
        (Bit::V(la, va), Bit::V(lb, vb)) => Bit::V(cs.mul(la, lb, 1), *va && *vb),
    }
}

fn bxor(cs: &mut OpCs, a: &Bit, b: &Bit) -> Bit {
    let _scope = crate::blocks::scope("bxor");
    match (a, b) {
        (Bit::C(x), Bit::C(y)) => Bit::C(x ^ y),
        (Bit::C(false), x) | (x, Bit::C(false)) => x.clone(),
        (Bit::C(true), x) | (x, Bit::C(true)) => bnot(x),
        (Bit::V(la, va), Bit::V(lb, vb)) => {
            let ab = cs.mul(la, lb, 1);
            Bit::V(la.plus(lb).minus(&ab.scale(Fp::from(2))), va ^ vb)
        }
    }
}

type Word = [Bit; 32];

fn word_const(w: u32) -> Word {
    std::array::from_fn(|i| Bit::C((w >> i) & 1 == 1))
}

fn word_value(w: &Word) -> u32 {
    w.iter()
        .enumerate()
        .fold(0, |acc, (i, b)| acc | ((b.value() as u32) << i))
}

fn rotr(w: &Word, r: usize) -> Word {
    std::array::from_fn(|i| w[(i + r) % 32].clone())
}

fn shr(w: &Word, r: usize) -> Word {
    std::array::from_fn(|i| {
        if i + r < 32 {
            w[i + r].clone()
        } else {
            Bit::C(false)
        }
    })
}

fn xor3(cs: &mut OpCs, a: &Word, b: &Word, c: &Word) -> Word {
    std::array::from_fn(|i| {
        let t = bxor(cs, &a[i], &b[i]);
        bxor(cs, &t, &c[i])
    })
}

/// Sum of words and a constant mod 2^32: output and carry bits are new
/// booleans, one linear recomposition row.
fn add_words(cs: &mut OpCs, terms: &[&Word], k: u32) -> Word {
    let total: u64 = terms.iter().map(|w| word_value(w) as u64).sum::<u64>() + k as u64;
    if terms
        .iter()
        .all(|w| w.iter().all(|b| matches!(b, Bit::C(_))))
    {
        return word_const(total as u32);
    }
    let max: u64 = terms.len() as u64 * 0xffff_ffff + k as u64;
    let carry_bits = 64 - (max >> 32).leading_zeros() as usize;
    let mut sum = Lc::constant(Fp::from(k as u64));
    for w in terms {
        for (i, b) in w.iter().enumerate() {
            sum = sum.plus(&b.lc().scale(Fp::from(1u64 << i)));
        }
    }
    let out: Word = std::array::from_fn(|i| {
        let v = (total >> i) & 1 == 1;
        Bit::V(cs.boolean(v), v)
    });
    let mut rec = Lc::default();
    for (i, b) in out.iter().enumerate() {
        rec = rec.plus(&b.lc().scale(Fp::from(1u64 << i)));
    }
    for j in 0..carry_bits {
        let v = (total >> (32 + j)) & 1 == 1;
        let c = cs.boolean(v);
        rec = rec.plus(&c.scale(Fp::from(1u64 << (32 + j))));
    }
    cs.enforce_zero(&sum.minus(&rec));
    out
}

/// SHA-256 of a 32-byte message given as 256 message bits (byte order, LSB
/// first within each byte). Returns the digest as 256 bits of the big-endian
/// integer, least significant first.
fn sha256_32(cs: &mut OpCs, msg: &[Bit]) -> Vec<(Lc, bool)> {
    assert_eq!(msg.len(), 256);
    let mut block: Vec<Word> = Vec::new();
    for j in 0..8 {
        // Word j = big-endian bytes 4j..4j+4; word bit i = byte (4j + 3 - i/8) bit i%8.
        block.push(std::array::from_fn(|i| {
            msg[8 * (4 * j + 3 - i / 8) + i % 8].clone()
        }));
    }
    block.push(word_const(0x8000_0000));
    for _ in 9..15 {
        block.push(word_const(0));
    }
    block.push(word_const(256));
    let mut w = block.clone();
    for t in 16..64 {
        let s0 = xor3(
            cs,
            &rotr(&w[t - 15], 7),
            &rotr(&w[t - 15], 18),
            &shr(&w[t - 15], 3),
        );
        let s1 = xor3(
            cs,
            &rotr(&w[t - 2], 17),
            &rotr(&w[t - 2], 19),
            &shr(&w[t - 2], 10),
        );
        let next = add_words(cs, &[&w[t - 16], &s0, &w[t - 7], &s1], 0);
        w.push(next);
    }
    let mut st: Vec<Word> = p256::SHA_IV.iter().map(|v| word_const(*v)).collect();
    for t in 0..64 {
        let (a, b, c, d, e, f, g, h) = (
            &st[0], &st[1], &st[2], &st[3], &st[4], &st[5], &st[6], &st[7],
        );
        let s1 = xor3(cs, &rotr(e, 6), &rotr(e, 11), &rotr(e, 25));
        // ch = g + e (f - g)
        let ch: Word = std::array::from_fn(|i| match (&e[i], &f[i], &g[i]) {
            (Bit::C(ev), _, _) => {
                if *ev {
                    f[i].clone()
                } else {
                    g[i].clone()
                }
            }
            _ => {
                let diff_v = f[i].value() as i64 - g[i].value() as i64;
                let prod = match (&f[i], &g[i]) {
                    (Bit::C(x), Bit::C(y)) if x == y => Lc::default(),
                    (Bit::C(_), Bit::C(_)) => e[i].lc().scale(fp_from_big(&BigInt::from(diff_v))),
                    _ => cs.mul(&e[i].lc(), &f[i].lc().minus(&g[i].lc()), 1),
                };
                let v = (g[i].value() as i64 + (e[i].value() as i64) * diff_v) == 1;
                Bit::V(g[i].lc().plus(&prod), v)
            }
        });
        let s0 = xor3(cs, &rotr(a, 2), &rotr(a, 13), &rotr(a, 22));
        // maj = a b + c (a xor b)
        let maj: Word = std::array::from_fn(|i| {
            let ab = band(cs, &a[i], &b[i]);
            let axb = match (&a[i], &b[i]) {
                (Bit::V(la, va), Bit::V(lb, vb)) => {
                    Bit::V(la.plus(lb).minus(&ab.lc().scale(Fp::from(2))), va ^ vb)
                }
                _ => bxor(cs, &a[i], &b[i]),
            };
            let cx = band(cs, &c[i], &axb);
            let v = (a[i].value() & b[i].value())
                ^ (a[i].value() & c[i].value())
                ^ (b[i].value() & c[i].value());
            match (&ab, &cx) {
                (Bit::C(x), Bit::C(y)) => Bit::C(*x || *y),
                _ => Bit::V(ab.lc().plus(&cx.lc()), v),
            }
        });
        let t1_terms: Vec<&Word> = vec![h, &s1, &ch, &w[t]];
        let new_e = {
            let mut terms = t1_terms.clone();
            terms.push(d);
            add_words(cs, &terms, p256::SHA_K[t])
        };
        let new_a = {
            let mut terms = t1_terms;
            terms.push(&s0);
            terms.push(&maj);
            add_words(cs, &terms, p256::SHA_K[t])
        };
        st = vec![
            new_a,
            a.clone(),
            b.clone(),
            c.clone(),
            new_e,
            e.clone(),
            f.clone(),
            g.clone(),
        ];
    }
    let out: Vec<Word> = st
        .iter()
        .zip(p256::SHA_IV)
        .map(|(s, iv)| add_words(cs, &[s], iv))
        .collect();
    // Digest integer z: word 0 is the most significant.
    let mut bits = Vec::with_capacity(256);
    for k in 0..256 {
        let (word, bit) = (7 - k / 32, k % 32);
        let b = &out[word][bit];
        bits.push((b.lc(), b.value()));
    }
    bits
}

// ---------------------------------------------------------------------------
// SHA-256 with spread lookups (lever 5, synthesized).
//
// A word carries its dense value and, once known, its spread form
// spread(w) = sum_i w_i 4^i. One fixed table of (d, spread(d)) for d < 2^11
// serves chunk lookups (dense and spread together) and range checks of
// narrower pieces (looking up d 2^(11 - b) with its spread). A sum of up to
// three spread words has base-4 digits in 0..3, so its split into even bits
// (XOR) and odd bits (AND / majority) is unique: one linear row and three chunk
// lookups per half decide both.
// - Sigma0, Sigma1, sigma0, sigma1: the word is decomposed at its rotation and
//   shift cuts; rotated and shifted copies are linear in the pieces' spreads;
//   the XOR is the even half of their sum.
// - Maj: the odd half of spread(a) + spread(b) + spread(c).
// - Ch: the odd halves of spread(e) + spread(f) and spread(!e) + spread(g),
//   added (they never overlap).
// Additions mod 2^32 allocate the result with boolean carry bits; a result is
// range-checked where it is next decomposed, and words used only in additions
// may carry multiples of 2^32 harmlessly (every later use is mod 2^32). Pieces
// of up to 3 bits are booleans. The message words are linear in the input's
// canonical bits; the output words are bits, for the ECDSA digest, as before.

static SHA_LOOKUP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Selects the SHA-256 gadget for signed messages (lever 5 when true).
pub fn set_sha_lookup(on: bool) {
    SHA_LOOKUP.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn sha_lookup() -> bool {
    SHA_LOOKUP.load(std::sync::atomic::Ordering::Relaxed)
}

const SPREAD: usize = 11;

fn spread_of(x: u64, bits: usize) -> u64 {
    (0..bits).fold(0_u64, |acc, i| acc | (((x >> i) & 1) << (2 * i)))
}

fn spread_entries() -> Vec<Vec<Fp>> {
    (0..1u64 << SPREAD)
        .map(|d| vec![Fp::from(d), Fp::from(spread_of(d, SPREAD))])
        .collect()
}

fn lc_is_constant(lc: &Lc) -> bool {
    lc.0.iter().all(|(v, _)| *v == 0)
}

fn fpu(x: u64) -> Fp {
    Fp::from(x)
}

#[derive(Clone)]
struct SWord {
    dense: Lc,
    val: u32,
    spread: Option<Lc>,
    /// The word's bits when it comes from the message (free pieces).
    bits: Option<Vec<Lc>>,
    constant: bool,
}

fn sword_const(v: u32) -> SWord {
    SWord {
        dense: Lc::constant(fpu(v as u64)),
        val: v,
        spread: Some(Lc::constant(fpu(spread_of(v as u64, 32)))),
        bits: None,
        constant: true,
    }
}

#[derive(Clone)]
struct SPiece {
    off: usize,
    b: usize,
    spread: Lc,
}

/// Decomposes a word at `cuts` (pieces above 11 bits split into 11-bit
/// parts): free for constants and message words, booleans up to 3 bits, else
/// a table lookup plus a range lookup below 11 bits; one recomposition row.
fn sha_pieces(cs: &mut OpCs, w: &SWord, cuts: &[usize]) -> Vec<SPiece> {
    let _scope = crate::blocks::scope("sha_pieces");
    let mut bounds = vec![0];
    bounds.extend_from_slice(cuts);
    bounds.push(32);
    let mut pieces = Vec::new();
    let mut rec = Lc::default();
    for win in bounds.windows(2) {
        let mut start = win[0];
        while start < win[1] {
            let b = (win[1] - start).min(SPREAD);
            let v = ((w.val as u64) >> start) & ((1_u64 << b) - 1);
            let (dense, spread) = if w.constant {
                (Lc::constant(fpu(v)), Lc::constant(fpu(spread_of(v, b))))
            } else if let Some(bits) = &w.bits {
                let mut d = Lc::default();
                let mut s = Lc::default();
                for i in 0..b {
                    d = d.plus(&bits[start + i].scale(fpu(1 << i)));
                    s = s.plus(&bits[start + i].scale(fpu(1 << (2 * i))));
                }
                (d, s)
            } else if b <= 3 {
                let mut d = Lc::default();
                let mut s = Lc::default();
                for i in 0..b {
                    let bit = cs.boolean((v >> i) & 1 == 1);
                    d = d.plus(&bit.scale(fpu(1 << i)));
                    s = s.plus(&bit.scale(fpu(1 << (2 * i))));
                }
                (d, s)
            } else {
                let d = Lc::var(cs.alloc(fpu(v), b as u16));
                let s = Lc::var(cs.alloc(fpu(spread_of(v, b)), (2 * b) as u16));
                cs.fixed_lookup();
                cs.record_lookup("spread11", vec![d.clone(), s.clone()], spread_entries);
                if b < SPREAD {
                    cs.fixed_lookup();
                    let shifted = vec![
                        d.scale(fpu(1 << (SPREAD - b))),
                        s.scale(fpu(1 << (2 * (SPREAD - b)))),
                    ];
                    cs.record_lookup("spread11", shifted, spread_entries);
                }
                (d, s)
            };
            rec = rec.plus(&dense.scale(fpu(1 << start)));
            pieces.push(SPiece {
                off: start,
                b,
                spread,
            });
            start += b;
        }
    }
    if !w.constant && w.bits.is_none() {
        cs.enforce_zero(&rec.minus(&w.dense));
    }
    pieces
}

fn spread_of_pieces(pieces: &[SPiece]) -> Lc {
    pieces.iter().fold(Lc::default(), |acc, p| {
        acc.plus(&p.spread.scale(fpu(1 << (2 * p.off))))
    })
}

fn rotr_spread(pieces: &[SPiece], r: usize) -> Lc {
    pieces.iter().fold(Lc::default(), |acc, p| {
        assert!(
            p.off >= r || p.off + p.b <= r,
            "piece aligned with the rotation"
        );
        acc.plus(&p.spread.scale(fpu(1 << (2 * ((p.off + 32 - r) % 32)))))
    })
}

fn shr_spread(pieces: &[SPiece], r: usize) -> Lc {
    pieces
        .iter()
        .filter(|p| p.off >= r)
        .fold(Lc::default(), |acc, p| {
            acc.plus(&p.spread.scale(fpu(1 << (2 * (p.off - r)))))
        })
}

/// Splits a spread sum (base-4 digits 0..3) into its even bits and odd bits.
fn spread_split(cs: &mut OpCs, s: &Lc, sval: u64) -> (SWord, SWord) {
    let _scope = crate::blocks::scope("spread_split");
    let even: u32 = (0..32).fold(0, |acc, i| acc | ((((sval >> (2 * i)) & 1) as u32) << i));
    let odd: u32 = (0..32).fold(0, |acc, i| {
        acc | ((((sval >> (2 * i + 1)) & 1) as u32) << i)
    });
    if lc_is_constant(s) {
        return (sword_const(even), sword_const(odd));
    }
    let mut half = |v: u32| -> (Lc, Lc) {
        let mut d = Lc::default();
        let mut sp = Lc::default();
        for (off, b) in [(0_usize, 11_usize), (11, 11), (22, 10)] {
            let c = ((v as u64) >> off) & ((1_u64 << b) - 1);
            let dv = Lc::var(cs.alloc(fpu(c), b as u16));
            let sv = Lc::var(cs.alloc(fpu(spread_of(c, b)), (2 * b) as u16));
            cs.fixed_lookup();
            cs.record_lookup("spread11", vec![dv.clone(), sv.clone()], spread_entries);
            d = d.plus(&dv.scale(fpu(1 << off)));
            sp = sp.plus(&sv.scale(fpu(1 << (2 * off))));
        }
        (d, sp)
    };
    let (ed, es) = half(even);
    let (od, os) = half(odd);
    cs.enforce_zero(&es.plus(&os.scale(fpu(2))).minus(s));
    (
        SWord {
            dense: ed,
            val: even,
            spread: Some(es),
            bits: None,
            constant: false,
        },
        SWord {
            dense: od,
            val: odd,
            spread: Some(os),
            bits: None,
            constant: false,
        },
    )
}

/// Sum mod 2^32: the result allocated (32-bit), carry bits boolean.
fn sha_add(cs: &mut OpCs, terms: &[&SWord], k: u32) -> SWord {
    let _scope = crate::blocks::scope("sha_add");
    let total: u64 = terms.iter().map(|w| w.val as u64).sum::<u64>() + k as u64;
    let val = total as u32;
    if terms.iter().all(|w| w.constant) {
        return sword_const(val);
    }
    let max = terms.len() as u64 * 0xffff_ffff + k as u64;
    let carry_bits = 64 - (max >> 32).leading_zeros() as usize;
    let mut sum = Lc::constant(fpu(k as u64));
    for w in terms {
        sum = sum.plus(&w.dense);
    }
    let out = Lc::var(cs.alloc(fpu(val as u64), 32));
    let mut rec = out.clone();
    for j in 0..carry_bits {
        let c = cs.boolean((total >> (32 + j)) & 1 == 1);
        rec = rec.plus(&c.scale(fpu(1 << (32 + j))));
    }
    cs.enforce_zero(&sum.minus(&rec));
    SWord {
        dense: out,
        val,
        spread: None,
        bits: None,
        constant: false,
    }
}

/// A word's spread, decomposing it at `cuts` if needed (cached in the word).
fn sha_spread(cs: &mut OpCs, w: &mut SWord, cuts: &[usize]) -> (Lc, Vec<SPiece>) {
    let _scope = crate::blocks::scope("sha_spread");
    let pieces = sha_pieces(cs, w, cuts);
    let s = spread_of_pieces(&pieces);
    if w.spread.is_none() {
        w.spread = Some(s.clone());
    }
    (s, pieces)
}

/// The XOR of rotated / shifted copies: (rotations, shift) over pieces.
fn sha_sigma(
    cs: &mut OpCs,
    pieces: &[SPiece],
    rot: &[usize],
    shr: Option<usize>,
    val: u32,
) -> SWord {
    let _scope = crate::blocks::scope("sha_sigma");
    let mut s = Lc::default();
    let mut sval: u64 = 0;
    for &r in rot {
        s = s.plus(&rotr_spread(pieces, r));
        sval += spread_of(val.rotate_right(r as u32) as u64, 32);
    }
    if let Some(r) = shr {
        s = s.plus(&shr_spread(pieces, r));
        sval += spread_of((val >> r) as u64, 32);
    }
    spread_split(cs, &s, sval).0
}

/// SHA-256 of the 32-byte message given as 256 bits (as `sha256_32`), with
/// spread lookups; returns the digest bits in the same order.
fn sha256_32_lookup(cs: &mut OpCs, msg: &[Bit]) -> Vec<(Lc, bool)> {
    let _scope = crate::blocks::scope("sha256");
    assert_eq!(msg.len(), 256);
    cs.fixed_tables.insert("sha_spread_11".into(), 1 << SPREAD);
    let mut w: Vec<SWord> = Vec::with_capacity(64);
    for j in 0..8 {
        let bits: Vec<Bit> = (0..32)
            .map(|i| msg[8 * (4 * j + 3 - i / 8) + i % 8].clone())
            .collect();
        let lcs: Vec<Lc> = bits.iter().map(|b| b.lc()).collect();
        let val = bits
            .iter()
            .enumerate()
            .fold(0_u32, |acc, (i, b)| acc | ((b.value() as u32) << i));
        let dense = lcs
            .iter()
            .enumerate()
            .fold(Lc::default(), |acc, (i, l)| acc.plus(&l.scale(fpu(1 << i))));
        let spread = lcs.iter().enumerate().fold(Lc::default(), |acc, (i, l)| {
            acc.plus(&l.scale(fpu(1 << (2 * i))))
        });
        w.push(SWord {
            dense,
            val,
            spread: Some(spread),
            bits: Some(lcs),
            constant: false,
        });
    }
    w.push(sword_const(0x8000_0000));
    for _ in 9..15 {
        w.push(sword_const(0));
    }
    w.push(sword_const(256));
    // Message schedule. Words 16..=48 feed both sigma0 and sigma1 (one
    // decomposition at the union of their cuts); 49..=61 only sigma1.
    let both = [3, 7, 10, 17, 18, 19];
    let s1_cuts = [10, 17, 19];
    let mut cache: Vec<Option<Vec<SPiece>>> = vec![None; 64];
    for t in 16..64 {
        let _step = crate::blocks::scope("sha_schedule_step");
        let mut pieces_of = |cs: &mut OpCs, w: &mut Vec<SWord>, i: usize| -> Vec<SPiece> {
            if let Some(p) = &cache[i] {
                return p.clone();
            }
            let cuts: &[usize] = if (1..=48).contains(&i) {
                &both
            } else {
                &s1_cuts
            };
            let p = sha_pieces(cs, &w[i], cuts);
            cache[i] = Some(p.clone());
            p
        };
        let p15 = pieces_of(cs, &mut w, t - 15);
        let p2 = pieces_of(cs, &mut w, t - 2);
        let s0 = sha_sigma(cs, &p15, &[7, 18], Some(3), w[t - 15].val);
        let s1 = sha_sigma(cs, &p2, &[17, 19], Some(10), w[t - 2].val);
        let next = sha_add(cs, &[&w[t - 16], &s0, &w[t - 7], &s1], 0);
        w.push(next);
    }
    // Compression.
    let mut st: Vec<SWord> = p256::SHA_IV.iter().map(|v| sword_const(*v)).collect();
    let ones = fpu(spread_of(0xffff_ffff, 32));
    for t in 0..64 {
        let _step = crate::blocks::scope("sha_round");
        let (mut a, b, c, d) = (st[0].clone(), st[1].clone(), st[2].clone(), st[3].clone());
        let (mut e, f, g, h) = (st[4].clone(), st[5].clone(), st[6].clone(), st[7].clone());
        let (es, pe) = sha_spread(cs, &mut e, &[6, 11, 25]);
        let (as_, pa) = sha_spread(cs, &mut a, &[2, 13, 22]);
        let s1 = sha_sigma(cs, &pe, &[6, 11, 25], None, e.val);
        let s0 = sha_sigma(cs, &pa, &[2, 13, 22], None, a.val);
        let (fs, gs) = (f.spread.clone().unwrap(), g.spread.clone().unwrap());
        let (bs, cs_) = (b.spread.clone().unwrap(), c.spread.clone().unwrap());
        let sp = |x: u32| spread_of(x as u64, 32);
        let (_, ef) = spread_split(cs, &es.plus(&fs), sp(e.val) + sp(f.val));
        let not_e = Lc::constant(ones).minus(&es);
        let (_, ng) = spread_split(cs, &not_e.plus(&gs), sp(!e.val) + sp(g.val));
        let ch = SWord {
            dense: ef.dense.plus(&ng.dense),
            val: ef.val | ng.val,
            spread: None,
            bits: None,
            constant: ef.constant && ng.constant,
        };
        let (_, maj) = spread_split(
            cs,
            &as_.plus(&bs).plus(&cs_),
            sp(a.val) + sp(b.val) + sp(c.val),
        );
        let new_e = sha_add(cs, &[&h, &s1, &ch, &w[t], &d], p256::SHA_K[t]);
        let new_a = sha_add(cs, &[&h, &s1, &ch, &w[t], &s0, &maj], p256::SHA_K[t]);
        st = vec![new_a, a, b, c, new_e, e, f, g];
    }
    // Output words as bits: state + IV with boolean output and carry bits.
    let mut out_words: Vec<Word> = Vec::with_capacity(8);
    for (s, iv) in st.iter().zip(p256::SHA_IV) {
        let total = s.val as u64 + iv as u64;
        let out: Word = std::array::from_fn(|i| {
            let v = (total >> i) & 1 == 1;
            Bit::V(cs.boolean(v), v)
        });
        let carry = cs.boolean(total >> 32 == 1);
        let mut rec = carry.scale(fpu(1 << 32));
        for (i, bit) in out.iter().enumerate() {
            rec = rec.plus(&bit.lc().scale(fpu(1 << i)));
        }
        cs.enforce_zero(&s.dense.plus(&Lc::constant(fpu(iv as u64))).minus(&rec));
        out_words.push(out);
    }
    let mut bits = Vec::with_capacity(256);
    for k in 0..256 {
        let (word, bit) = (7 - k / 32, k % 32);
        let b = &out_words[word][bit];
        bits.push((b.lc(), b.value()));
    }
    bits
}

/// Canonical bits of a field element (as in gadgets.rs): 255 booleans, one
/// recomposition row, and the counted less-than-modulus comparison.
fn canonical_bits(cs: &mut OpCs, x: Fp) -> Vec<Bit> {
    let _scope = crate::blocks::scope("canonical_bits");
    let repr = x.to_repr();
    let mut bits = Vec::with_capacity(256);
    let mut rec = Lc::default();
    for i in 0..255 {
        let v = (repr[i / 8] >> (i % 8)) & 1 == 1;
        let lc = cs.boolean(v);
        rec = rec.plus(&lc.scale(fp_u(&pow2(i))));
        bits.push(Bit::V(lc, v));
    }
    bits.push(Bit::C(false));
    let xv = Lc::var(cs.alloc(x, 255));
    cs.enforce_zero(&rec.minus(&xv));
    cs.counted_rows += 255;
    bits
}

/// One signed message: the 32-byte encoding of a Poseidon digest, SHA-256
/// in-circuit, then ECDSA under `key`.
fn signed_message(cs: &mut OpCs, label: &str, key: SigKey) {
    let n = p256::n();
    let d = p256::scalar_below(&format!("{label}-d"), &n);
    let q = p256::mul(&p256::generator(), &d).unwrap();
    let m = fp_u(&p256::scalar_below(&format!("{label}-m"), &fp_modulus()));
    let msg = canonical_bits(cs, m);
    let digest = if sha_lookup() {
        sha256_32_lookup(cs, &msg)
    } else {
        sha256_32(cs, &msg)
    };
    let native = p256::sha256_one_block(&m.to_repr());
    let z = BigUint::from_bytes_be(&native);
    let zin: BigUint = digest
        .iter()
        .enumerate()
        .filter(|(_, (_, b))| *b)
        .map(|(i, _)| pow2(i))
        .sum();
    assert_eq!(zin, z, "in-circuit SHA-256 matches native");
    let sig = p256::sign(&d, &z, &p256::scalar_below(&format!("{label}-k"), &n));
    match key {
        SigKey::Variable => ecdsa(cs, &q, &z, Some(&digest), &sig, false),
        SigKey::FixedOdd => ecdsa(cs, &q, &z, Some(&digest), &sig, true),
        SigKey::Own(_) | SigKey::FixedComb(_) => ecdsa_comb(cs, &q, &z, Some(&digest), &sig, key),
    }
}

// ---------------------------------------------------------------------------
// Per-component costs and the operation census.

#[derive(Clone, Debug, Default)]
pub struct Cost {
    pub product_rows: usize,
    pub linear_rows: usize,
    pub nnz: usize,
    /// Witness entries by width class (bits).
    pub widths: BTreeMap<u16, usize>,
    pub full_chunks: usize,
    pub partial_chunks: usize,
    pub dyn_lookups: usize,
    pub dyn_entries: usize,
    pub fixed_lookups: usize,
    pub fixed_tables: BTreeMap<String, usize>,
    pub wallet_lookups: usize,
    pub wallet_tables: BTreeMap<String, usize>,
    pub counted_rows: usize,
    pub chunk: usize,
    pub mul_checks: usize,
    /// Wiring between uniform blocks as lookups into a value table (copy
    /// constraints), and that table's entries.
    pub copy_lookups: usize,
    pub copy_entries: usize,
}

impl Cost {
    fn from_cs(cs: &OpCs) -> Cost {
        let mut widths = BTreeMap::new();
        for w in &cs.widths[1..] {
            *widths.entry(*w).or_insert(0) += 1;
        }
        Cost {
            product_rows: cs.product_rows,
            linear_rows: cs.linear_rows,
            nnz: cs.nnz,
            widths,
            full_chunks: cs.full_chunks,
            partial_chunks: cs.partial_chunks,
            dyn_lookups: cs.dyn_lookups,
            dyn_entries: cs.dyn_entries,
            fixed_lookups: cs.fixed_lookups,
            fixed_tables: cs.fixed_tables.clone(),
            wallet_lookups: cs.wallet_lookups,
            wallet_tables: cs.wallet_tables.clone(),
            counted_rows: cs.counted_rows,
            chunk: cs.chunk,
            mul_checks: cs.mul_checks,
            copy_lookups: 0,
            copy_entries: 0,
        }
    }
    pub fn times(&self, k: usize) -> Cost {
        Cost {
            product_rows: self.product_rows * k,
            linear_rows: self.linear_rows * k,
            nnz: self.nnz * k,
            widths: self.widths.iter().map(|(w, c)| (*w, c * k)).collect(),
            full_chunks: self.full_chunks * k,
            partial_chunks: self.partial_chunks * k,
            dyn_lookups: self.dyn_lookups * k,
            dyn_entries: self.dyn_entries * k,
            fixed_lookups: self.fixed_lookups * k,
            fixed_tables: if k == 0 {
                BTreeMap::new()
            } else {
                self.fixed_tables.clone()
            },
            wallet_lookups: self.wallet_lookups * k,
            wallet_tables: if k == 0 {
                BTreeMap::new()
            } else {
                self.wallet_tables.clone()
            },
            counted_rows: self.counted_rows * k,
            chunk: self.chunk,
            mul_checks: self.mul_checks * k,
            copy_lookups: self.copy_lookups * k,
            copy_entries: self.copy_entries * k,
        }
    }
    pub fn plus(&self, o: &Cost) -> Cost {
        let mut widths = self.widths.clone();
        for (w, c) in &o.widths {
            *widths.entry(*w).or_insert(0) += c;
        }
        let mut fixed_tables = self.fixed_tables.clone();
        for (t, e) in &o.fixed_tables {
            fixed_tables.insert(t.clone(), *e); // shared tables count once
        }
        let mut wallet_tables = self.wallet_tables.clone();
        for (t, e) in &o.wallet_tables {
            wallet_tables.insert(t.clone(), *e); // one table set per wallet
        }
        Cost {
            product_rows: self.product_rows + o.product_rows,
            linear_rows: self.linear_rows + o.linear_rows,
            nnz: self.nnz + o.nnz,
            widths,
            full_chunks: self.full_chunks + o.full_chunks,
            partial_chunks: self.partial_chunks + o.partial_chunks,
            dyn_lookups: self.dyn_lookups + o.dyn_lookups,
            dyn_entries: self.dyn_entries + o.dyn_entries,
            fixed_lookups: self.fixed_lookups + o.fixed_lookups,
            fixed_tables,
            wallet_lookups: self.wallet_lookups + o.wallet_lookups,
            wallet_tables,
            counted_rows: self.counted_rows + o.counted_rows,
            chunk: self.chunk.max(o.chunk),
            mul_checks: self.mul_checks + o.mul_checks,
            copy_lookups: self.copy_lookups + o.copy_lookups,
            copy_entries: self.copy_entries + o.copy_entries,
        }
    }
    pub fn range_lookups(&self) -> usize {
        self.full_chunks + 2 * self.partial_chunks
    }
    pub fn lookups(&self) -> usize {
        self.range_lookups()
            + self.dyn_lookups
            + self.fixed_lookups
            + self.wallet_lookups
            + self.copy_lookups
    }
    pub fn witness(&self) -> usize {
        self.widths.values().sum()
    }
    /// Rows before the lookup argument.
    pub fn rows(&self) -> usize {
        self.product_rows + self.linear_rows + self.counted_rows
    }
}

/// The operation's relation with its lookup argument (LogUp, committed
/// inverse helpers): one row and one full-width helper per lookup and per
/// table entry, plus the multiplicities and one sum row.
#[derive(Clone, Debug)]
pub struct WithLookups {
    pub constraints: usize,
    pub witness: BTreeMap<u16, usize>,
    pub nnz: usize,
    pub lookups: usize,
    pub table_entries: usize,
    /// Per-wallet table entries (6 limbs each in dedicated rows).
    pub wallet_entries: usize,
}

impl Cost {
    pub fn with_lookups(&self) -> WithLookups {
        let range_table = 1usize << self.chunk;
        let fixed: usize = self.fixed_tables.values().sum();
        let wallet: usize = self.wallet_tables.values().sum();
        let table_entries = range_table + self.dyn_entries + fixed + wallet + self.copy_entries;
        let lookups = self.lookups();
        let mut witness = self.widths.clone();
        *witness.entry(255).or_insert(0) += lookups + table_entries; // helpers
        *witness.entry(20).or_insert(0) += table_entries; // multiplicities
        // The per-wallet rows (6 limbs per entry) sit in dedicated rows that
        // the fixed point reserves; their commitment is fixed at Bootstrap, so
        // no commitment work per proof.
        WithLookups {
            constraints: self.rows() + lookups + table_entries + 1,
            witness,
            // h (alpha - f) = 1: ~3 + chunk/compressed-row terms; table rows similar.
            nnz: self.nnz
                + 4 * self.range_lookups()
                + 9 * (self.dyn_lookups + self.fixed_lookups)
                + 17 * self.wallet_lookups
                + 5 * self.copy_lookups
                + 9 * table_entries
                + lookups
                + table_entries,
            lookups,
            table_entries,
            wallet_entries: wallet,
        }
    }
}

/// Synthesized components at range-chunk width `chunk`.
pub struct Components {
    pub chunk: usize,
    /// One signed message with a variable key: canonical bits, SHA-256, ECDSA.
    pub signed_var: Cost,
    /// The same under a fixed (circuit-constant) key, gate 1c's method.
    pub signed_fixed: Cost,
    pub sha_only: Cost,
    pub ecdsa_only: Cost,
    pub mul_check: Cost,
}

impl Cost {
    /// Counts of `self` minus `o` (tables and lookups of `self` kept).
    fn minus_rows(&self, o: &Cost) -> Cost {
        Cost {
            product_rows: self.product_rows - o.product_rows,
            linear_rows: self.linear_rows - o.linear_rows,
            nnz: self.nnz - o.nnz,
            widths: self
                .widths
                .iter()
                .map(|(w, c)| (*w, c - o.widths.get(w).copied().unwrap_or(0)))
                .collect(),
            full_chunks: self.full_chunks - o.full_chunks,
            partial_chunks: self.partial_chunks - o.partial_chunks,
            counted_rows: self.counted_rows - o.counted_rows,
            mul_checks: self.mul_checks - o.mul_checks,
            ..self.clone()
        }
    }
}

pub fn components(chunk: usize) -> Components {
    let measure = |f: &dyn Fn(&mut OpCs)| {
        let mut cs = OpCs::new(chunk);
        f(&mut cs);
        Cost::from_cs(&cs)
    };
    let signed_var = measure(&|cs| signed_message(cs, "tau", SigKey::Variable));
    let signed_fixed = measure(&|cs| signed_message(cs, "root", SigKey::FixedOdd));
    let sha_only = measure(&|cs| {
        let m = fp_u(&p256::scalar_below("sha", &fp_modulus()));
        let msg = canonical_bits(cs, m);
        let _ = if sha_lookup() {
            sha256_32_lookup(cs, &msg)
        } else {
            sha256_32(cs, &msg)
        };
    });
    let ecdsa_only = signed_var.minus_rows(&sha_only);
    let mul_check = measure(&|cs| {
        let p = p256::p();
        let a = nn_alloc(cs, &p256::scalar_below("a", &p), 256);
        let b = nn_alloc(cs, &p256::scalar_below("b", &p), 256);
        let prod = BigInt::from(p256::modp(&(a.value() * b.value()), &p));
        let r = nn_alloc(cs, &prod.to_biguint().unwrap(), 256);
        mul_check(cs, &[(1, &a, &b)], &nn_lin(&[(-1, &r)]), Some(&p));
    });
    Components {
        chunk,
        signed_var,
        signed_fixed,
        sha_only,
        ecdsa_only,
        mul_check,
    }
}

/// Lever 1 components: own-key and root signatures over comb tables of `m`
/// bases (per-wallet tables for the own key, constants for G and the root).
pub struct CombComponents {
    pub m: usize,
    pub signed_own: Cost,
    pub signed_root: Cost,
    pub ecdsa_own_only: Cost,
}

pub fn comb_components(comp: &Components, m: usize) -> CombComponents {
    let measure = |key: SigKey, label: &str| {
        let mut cs = OpCs::new(comp.chunk);
        signed_message(&mut cs, label, key);
        Cost::from_cs(&cs)
    };
    let signed_own = measure(SigKey::Own(m), "tau");
    let signed_root = measure(SigKey::FixedComb(m), "root");
    let ecdsa_own_only = signed_own.minus_rows(&comp.sha_only);
    CombComponents {
        m,
        signed_own,
        signed_root,
        ecdsa_own_only,
    }
}

/// Counted (specification-derived) parts: RP57 permutations at the measured
/// 243 rows each, indexed-tree path selection, comparisons and u128 checks.
#[derive(Clone, Copy, Debug)]
pub struct Counted {
    pub permutations: usize,
    /// Indexed-tree insertions (4 depth-32 paths, 6 leaf hashes) and
    /// memberships (1 path, 1 leaf hash).
    pub insertions: usize,
    pub memberships: usize,
    /// Full-field order comparisons (canonical decompositions + difference).
    pub comparisons: usize,
    /// u128 values range-checked (balances, amounts, ordinals, sequence).
    pub u128_values: usize,
    /// Field elements re-encoded as canonical bytes for `P_bytes`.
    pub byte_elements: usize,
    /// Selectors, equalities and branch logic, as rows.
    pub logic_rows: usize,
}

pub const DEPTH: usize = 32;

impl Counted {
    pub fn cost(&self, chunk: usize, poseidon_rows: usize) -> Cost {
        let perms =
            self.permutations + self.insertions * (4 * DEPTH + 6) + self.memberships * (DEPTH + 1);
        let swaps = self.insertions * 4 * DEPTH + self.memberships * DEPTH;
        let path_bits = self.insertions * 2 * DEPTH + self.memberships * DEPTH;
        let siblings = self.insertions * 2 * DEPTH + self.memberships * DEPTH;
        let chunks_per = |bits: usize| bits.div_ceil(chunk);
        // A comparison: two canonical 255-bit decompositions, one 254-bit
        // difference, and two less-than-modulus checks on chunks.
        let cmp_chunks = 3 * chunks_per(255) + 2 * chunks_per(255);
        let u128_chunks = chunks_per(128);
        // Byte re-encoding: 32 byte chunks (8-bit, shifted lookups) + canonicity.
        let mut widths = BTreeMap::new();
        *widths.entry(255).or_insert(0) += perms * poseidon_rows + swaps + siblings;
        *widths.entry(1).or_insert(0) += path_bits;
        *widths.entry(chunk as u16).or_insert(0) += self.comparisons * cmp_chunks
            + self.u128_values * u128_chunks
            + self.byte_elements * chunks_per(255);
        *widths.entry(8).or_insert(0) += self.byte_elements * 32;
        Cost {
            product_rows: perms * poseidon_rows + swaps + path_bits + self.logic_rows / 2,
            linear_rows: self.comparisons * 5
                + self.u128_values
                + self.byte_elements * 2
                + self.logic_rows / 2,
            nnz: perms * poseidon_rows * 6
                + swaps * 5
                + path_bits * 3
                + self.logic_rows * 4
                + (self.comparisons * cmp_chunks + self.u128_values * u128_chunks) * 2
                + self.byte_elements * 40,
            widths,
            full_chunks: self.comparisons * cmp_chunks
                + self.u128_values * u128_chunks
                + self.byte_elements * chunks_per(255),
            partial_chunks: self.byte_elements * 32,
            dyn_lookups: 0,
            dyn_entries: 0,
            fixed_lookups: 0,
            fixed_tables: BTreeMap::new(),
            wallet_lookups: 0,
            wallet_tables: BTreeMap::new(),
            counted_rows: 0,
            chunk,
            mul_checks: 0,
            copy_lookups: 0,
            copy_entries: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Operation {
    pub label: &'static str,
    /// Signatures under the prover's own payment key.
    pub own: usize,
    /// Signatures under other keys known only at proving time (issuer).
    pub other: usize,
    /// Signatures under the fixed root key.
    pub root: usize,
    pub counted: Counted,
}

/// Send and Receive per kagemusha_single_design_proposal.md §§3.1-3.2 and
/// kagemusha_wallet_wire_v1.md (Request, Payment, receipt rows), as Path 2's
/// completed proof (step relation plus the lineage step's own checks).
pub fn operations() -> Vec<Operation> {
    // Shared Poseidon digests (permutations, rate 2):
    // predecessor and successor heads P(core(33) || rest digest): 17 each;
    // statement digest ~20 elements: 10; operation_id: 3;
    // Request terms m = P(19): 10, credit_id = P(24): 12, request_digest = P(5): 3;
    // chain append (8 elements): 4; receipt message (~12 elements): 6.
    let heads = 34;
    let request = 10 + 12 + 3;
    // + the credential opening behind the account-digest checks (proposal
    // 804-807): ~12 (b04a57's review).
    let common = heads + 10 + 3 + request + 4 + 6 + 12;
    vec![
        Operation {
            label: "Send",
            own: 1, // tau_send under the payment key
            other: 0,
            root: 0,
            counted: Counted {
                // rest-digest opening for the fee policy (~40 elements): 20;
                // fee schedule object: 5.
                permutations: common + 20 + 5,
                insertions: 1, // pending-outgoing descriptor
                memberships: 0,
                comparisons: 2,
                u128_values: 12,
                byte_elements: 0,
                logic_rows: 2_000,
            },
        },
        Operation {
            label: "Send+quota",
            own: 1,
            other: 0,
            root: 0,
            counted: Counted {
                // + quota windows: 4 touched windows x (open + update of a
                // depth-6 array, 2 x 7) + windows-root openings (4 x 7).
                permutations: common + 20 + 5 + 4 * 14 + 4 * 7,
                insertions: 1,
                memberships: 0,
                comparisons: 2,
                u128_values: 24,
                byte_elements: 0,
                logic_rows: 3_000,
            },
        },
        Operation {
            label: "Receive",
            own: 2, // tau_recv and the receiver's Request signature
            other: 0,
            root: 0,
            counted: Counted {
                // Payment digest P_bytes(163 B transcript): 4 + its 5 elements
                // re-encoded as bytes; package digest over the transcript
                // digest of the verified Send pair: 4.
                permutations: common + 4 + 4,
                insertions: 2, // consumed-credit and credit-digest trees
                memberships: 0,
                comparisons: 4,
                u128_values: 10,
                byte_elements: 5,
                logic_rows: 3_000,
            },
        },
        Operation {
            label: "Receive+renewed",
            own: 2,
            other: 1, // + the renewed receiver credential (issuer key)
            root: 1,  // + its issuer certificate under the fixed root
            counted: Counted {
                permutations: common + 4 + 4 + 8 + 8,
                insertions: 2,
                memberships: 0,
                comparisons: 4,
                u128_values: 10,
                byte_elements: 5,
                logic_rows: 3_500,
            },
        },
        // Renewal path, option 1: the issuer key over fixed comb tables
        // (circuit constants per issuer key), the certificate still verified.
        Operation {
            label: "Receive+renewed/fixed-issuer",
            own: 2,
            other: 0,
            root: 2, // credential (issuer comb tables) and certificate (root)
            counted: Counted {
                permutations: common + 4 + 4 + 8 + 8,
                insertions: 2,
                memberships: 0,
                comparisons: 4,
                u128_values: 10,
                byte_elements: 5,
                logic_rows: 3_500,
            },
        },
        // Renewal path, option 1b: the issuer keys are circuit constants
        // (rotation needs a circuit update), so the certificate is checked
        // when the circuit is built, not in-circuit.
        Operation {
            label: "Receive+renewed/issuer-constant",
            own: 2,
            other: 0,
            root: 1,
            counted: Counted {
                permutations: common + 4 + 4 + 8,
                insertions: 2,
                memberships: 0,
                comparisons: 4,
                u128_values: 10,
                byte_elements: 5,
                logic_rows: 3_500,
            },
        },
        // Renewal path, option 2 (spec change): credential continuity. The
        // quoted pre-renewal credential was verified in-circuit when it was
        // introduced (Bootstrap or RefreshCredential); Receive checks its
        // digest (8 permutations, as before) against the lineage's prior
        // credentials and binds wallet_id and payment_key by continuity, with
        // no signature. Three layouts:
        // (W) the last four prior digests in the core: each head hashes four
        //     more elements (+2 permutations per head), compare four (bounded:
        //     a Request quoted before five renewals stops being receivable);
        Operation {
            label: "Receive+renewed/window-core",
            own: 2,
            other: 0,
            root: 0,
            counted: Counted {
                permutations: common + 4 + 4 + 8 + 4,
                insertions: 2,
                memberships: 0,
                comparisons: 4,
                u128_values: 10,
                byte_elements: 5,
                logic_rows: 3_500 + 12,
            },
        },
        // (T) a depth-16 tree of prior digests indexed by renewal_sequence,
        //     its root in the core (+1 permutation per head), one path
        //     (17 permutations, 16 conditional swaps): unbounded;
        Operation {
            label: "Receive+renewed/tree-core",
            own: 2,
            other: 0,
            root: 0,
            counted: Counted {
                permutations: common + 4 + 4 + 8 + 17 + 2,
                insertions: 2,
                memberships: 0,
                comparisons: 4,
                u128_values: 10,
                byte_elements: 5,
                logic_rows: 3_500 + 3 * 16,
            },
        },
        // (R) the same tree with its root in the rest digest: the renewed
        //     case opens the rest digest (20 permutations, as Send's estimate).
        Operation {
            label: "Receive+renewed/tree-rest",
            own: 2,
            other: 0,
            root: 0,
            counted: Counted {
                permutations: common + 4 + 4 + 8 + 17 + 20,
                insertions: 2,
                memberships: 0,
                comparisons: 4,
                u128_values: 10,
                byte_elements: 5,
                logic_rows: 3_500 + 3 * 16,
            },
        },
    ]
}

/// Copy lookups per non-native check (block inputs fetched from the value
/// table: a point operation reads two points, 12 limbs, over 3-5 checks) and
/// per RP57 permutation (3 inputs, 3 outputs). Estimates.
pub const COPY_PER_MUL_CHECK: usize = 3;
pub const COPY_PER_PERMUTATION: usize = 6;

impl Operation {
    /// Gate 1c's methods (`comb` None: every non-root signature with a
    /// variable key) or lever 1 (own-key and root signatures over comb tables).
    pub fn cost(
        &self,
        comp: &Components,
        comb: Option<&CombComponents>,
        poseidon_rows: usize,
    ) -> Cost {
        let (own, root) = match comb {
            None => (&comp.signed_var, &comp.signed_fixed),
            Some(c) => (&c.signed_own, &c.signed_root),
        };
        let mut c = own
            .times(self.own)
            .plus(&comp.signed_var.times(self.other))
            .plus(&root.times(self.root))
            .plus(&self.counted.cost(comp.chunk, poseidon_rows));
        let perms = self.counted.permutations
            + self.counted.insertions * (4 * DEPTH + 6)
            + self.counted.memberships * (DEPTH + 1);
        c.copy_lookups = COPY_PER_MUL_CHECK * c.mul_checks + COPY_PER_PERMUTATION * perms;
        c.copy_entries = c.copy_lookups / 2;
        *c.widths.entry(255).or_insert(0) += 0; // copied values are existing witnesses
        c
    }
}

/// One step of the masked random linear combination over Fq computed in the
/// Fp circuit (the cross-circuit binding): acc' = acc rho + v (mod q), with
/// acc and rho non-native Fq values and v a native value below 2^128 whose
/// limbs come from existing bits.
pub fn binding_step(chunk: usize) -> Cost {
    let q = BigUint::parse_bytes(
        b"40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001",
        16,
    )
    .unwrap();
    let mut cs = OpCs::new(chunk);
    let acc = nn_alloc(&mut cs, &p256::scalar_below("acc", &q), 255);
    let rho = nn_alloc(&mut cs, &p256::scalar_below("rho", &q), 255);
    let v = p256::scalar_below("v", &pow2(128));
    let vn = nn_const(&v); // stands for a linear combination of existing bits
    let before = Cost::from_cs(&cs);
    let next = (&acc.value().to_biguint().unwrap() * &rho.value().to_biguint().unwrap() + &v) % &q;
    let nn = nn_alloc(&mut cs, &next, 255);
    mul_check(
        &mut cs,
        &[(1, &acc, &rho)],
        &nn_lin(&[(1, &vn), (-1, &nn)]),
        Some(&q),
    );
    let after = Cost::from_cs(&cs);
    Cost {
        product_rows: after.product_rows - before.product_rows,
        linear_rows: after.linear_rows - before.linear_rows,
        nnz: after.nnz - before.nnz,
        widths: after
            .widths
            .iter()
            .map(|(w, c)| (*w, c - before.widths.get(w).copied().unwrap_or(0)))
            .collect(),
        full_chunks: after.full_chunks - before.full_chunks,
        partial_chunks: after.partial_chunks - before.partial_chunks,
        dyn_lookups: 0,
        dyn_entries: 0,
        fixed_lookups: 0,
        fixed_tables: BTreeMap::new(),
        wallet_lookups: 0,
        wallet_tables: BTreeMap::new(),
        counted_rows: 0,
        chunk,
        mul_checks: 1,
        copy_lookups: COPY_PER_MUL_CHECK,
        copy_entries: 0,
    }
}

/// The binding's random linear combination in C_P (gate 1f): Horner over Fq,
/// acc' = acc rho + v, from the mask mu, with rho and every accumulator a
/// non-native Fq value and each shared value v below 2^128 (its limbs come from
/// existing cells). Returns the cost and R.
pub fn rlc_chain(chunk: usize, rho: &BigUint, mu: &BigUint, values: &[u128]) -> (Cost, BigUint) {
    let (cs, r) = rlc_chain_cs(OpCs::new(chunk), rho, mu, values);
    let mut cost = Cost::from_cs(&cs);
    cost.copy_lookups = COPY_PER_MUL_CHECK * values.len();
    (cost, r)
}

/// The same chain into a given (possibly recording) system.
pub fn rlc_chain_cs(mut cs: OpCs, rho: &BigUint, mu: &BigUint, values: &[u128]) -> (OpCs, BigUint) {
    let _scope = crate::blocks::scope("chain_p");
    let q = BigUint::parse_bytes(
        b"40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001",
        16,
    )
    .unwrap();
    let rn = nn_alloc(&mut cs, rho, 255);
    let mut acc = nn_alloc(&mut cs, mu, 255);
    let mut accv = mu.clone();
    for v in values {
        let _step = crate::blocks::scope("binding_step");
        let vb = BigUint::from(*v);
        let next = (&accv * rho + &vb) % &q;
        let nn = nn_alloc(&mut cs, &next, 255);
        mul_check(
            &mut cs,
            &[(1, &acc, &rn)],
            &nn_lin(&[(1, &nn_const(&vb)), (-1, &nn)]),
            Some(&q),
        );
        acc = nn;
        accv = next;
    }
    (cs, accv)
}

/// Count summary of one cost.
pub fn summary(c: &Cost) -> String {
    let wl = c.with_lookups();
    let full: usize = wl
        .witness
        .iter()
        .filter(|(w, _)| **w >= 128)
        .map(|(_, n)| n)
        .sum();
    let small: usize = wl
        .witness
        .iter()
        .filter(|(w, _)| **w < 128)
        .map(|(_, n)| n)
        .sum();
    format!(
        "rows {} (products {}, linear {}, counted {}) | checks {} | lookups {} (range {}, dynamic {}, fixed {}, wallet {}) | \
         with LogUp: constraints {}, witness {} ({} full-width, {} narrow), table entries {}",
        c.rows(),
        c.product_rows,
        c.linear_rows,
        c.counted_rows,
        c.mul_checks,
        c.lookups(),
        c.range_lookups(),
        c.dyn_lookups,
        c.fixed_lookups,
        c.wallet_lookups,
        wl.constraints,
        full + small,
        full,
        small,
        wl.table_entries,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spread-lookup SHA-256 matches native SHA-256 of the 32-byte
    /// encodings, and costs far fewer rows than the bit-level gadget.
    #[test]
    fn lookup_sha256_matches_native() {
        for label in ["sha-a", "sha-b", "sha-c"] {
            let m = fp_u(&p256::scalar_below(label, &fp_modulus()));
            let mut cs = OpCs::new(11);
            let msg = canonical_bits(&mut cs, m);
            let before = cs.product_rows + cs.linear_rows;
            let digest = sha256_32_lookup(&mut cs, &msg);
            let rows = cs.product_rows + cs.linear_rows - before;
            let native = BigUint::from_bytes_be(&p256::sha256_one_block(&m.to_repr()));
            let zin: BigUint = digest
                .iter()
                .enumerate()
                .filter(|(_, (_, b))| *b)
                .map(|(i, _)| pow2(i))
                .sum();
            assert_eq!(zin, native, "lookup SHA-256 matches native");
            assert!(
                rows < 6_000 && cs.fixed_lookups < 5_000,
                "rows {rows}, lookups {}",
                cs.fixed_lookups
            );
        }
    }

    fn own_key_signature(label: &str) -> (Point, BigUint, Signature) {
        let n = p256::n();
        let d = p256::scalar_below(&format!("{label}-d"), &n);
        let q = p256::mul(&p256::generator(), &d).unwrap();
        let z = p256::scalar_below(&format!("{label}-z"), &pow2(256));
        let sig = p256::sign(&d, &z, &p256::scalar_below(&format!("{label}-k"), &n));
        (q, z, sig)
    }

    /// The comb verifier accepts valid own-key and root signatures for every
    /// base count, and its loop has the expected number of checks.
    #[test]
    fn comb_ecdsa_accepts_valid_signatures() {
        for (m, key) in [
            (8, SigKey::Own(8)),
            (16, SigKey::Own(16)),
            (32, SigKey::Own(32)),
            (8, SigKey::FixedComb(8)),
        ] {
            for label in ["comb-a", "comb-b"] {
                let (q, z, sig) = own_key_signature(&format!("{label}-{m}"));
                let mut cs = OpCs::new(13);
                ecdsa_comb(&mut cs, &q, &z, None, &sig, key);
                let c = 256 / m;
                let per_window = 2 * m / 8;
                // first window: per_window - 1 additions; then a double-add
                // (6 checks) and per_window - 1 additions (4 each) per window;
                // plus 5 scalar checks and 2 final checks.
                let loop_checks = 4 * (per_window - 1) + (c - 1) * (6 + 4 * (per_window - 1));
                assert_eq!(cs.mul_checks, loop_checks + 7, "m = {m}");
                let lookups = if key == SigKey::Own(m) {
                    cs.wallet_lookups
                } else {
                    0
                };
                assert_eq!(cs.fixed_lookups + lookups, c * per_window);
            }
        }
    }

    /// A signature on another digest, or a wrong r, does not satisfy the
    /// comb verifier.
    #[test]
    fn comb_ecdsa_rejects_invalid_signatures() {
        let (q, z, sig) = own_key_signature("comb-bad");
        for (z2, r2) in [
            (&z + 1u32, sig.r.clone()),
            (z.clone(), (&sig.r + 1u32) % p256::n()),
        ] {
            let bad = Signature {
                r: r2,
                s: sig.s.clone(),
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut cs = OpCs::new(13);
                ecdsa_comb(&mut cs, &q, &z2, None, &bad, SigKey::Own(8));
            }));
            assert!(
                result.is_err(),
                "an invalid signature must not satisfy the comb checks"
            );
        }
    }

    /// A forced exceptional addition (a = c) computes the doubling, and a
    /// wrong slope is rejected: the slope is never a free 0/0 witness.
    #[test]
    fn addition_has_no_free_slope() {
        let g = p256::generator();
        let pt = p256::mul(&g, &BigUint::from(5u32)).unwrap();
        let mut cs = OpCs::new(13);
        let a = pt_alloc(&mut cs, &pt);
        let sum = pt_add(&mut cs, &a, &a.clone());
        assert_eq!(sum.nat, p256::double(&pt));
        for wrong in [slope(&pt, &pt) + 1u32, BigUint::from(7u32)] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut cs = OpCs::new(13);
                let a = pt_alloc(&mut cs, &pt);
                let _ = pt_add_with_slope(&mut cs, &a, &a.clone(), &(wrong.clone() % p256::p()));
            }));
            assert!(result.is_err(), "a wrong slope must not satisfy the checks");
        }
        // Distinct points: the chord slope passes, another slope fails.
        let q = p256::mul(&g, &BigUint::from(9u32)).unwrap();
        let mut cs = OpCs::new(13);
        let (a, c) = (pt_alloc(&mut cs, &pt), pt_alloc(&mut cs, &q));
        assert_eq!(pt_add(&mut cs, &a, &c).nat, p256::add(&pt, &q));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut cs = OpCs::new(13);
            let (a, c) = (pt_alloc(&mut cs, &pt), pt_alloc(&mut cs, &q));
            let _ = pt_add_with_slope(&mut cs, &a, &c, &(slope(&pt, &q) + 1u32));
        }));
        assert!(result.is_err());
    }

    /// A forced exceptional first-window addition inside double-add (acc = s)
    /// also computes the exact group law: 2 acc + acc = 3 acc.
    #[test]
    fn double_add_with_equal_inputs_is_exact() {
        let g = p256::generator();
        let pt = p256::mul(&g, &BigUint::from(11u32)).unwrap();
        let mut cs = OpCs::new(13);
        let a = pt_alloc(&mut cs, &pt);
        let out = pt_double_add(&mut cs, &a, &a.clone());
        assert_eq!(out.nat, p256::mul(&pt, &BigUint::from(3u32)).unwrap());
    }
}

// ---------------------------------------------------------------------------
// The real Receive relation, stage A: every census component as constraints
// with satisfying values (the KAGEMUSHA field layouts are stage B).

const NODE_DOMAIN: u64 = 0x6b67_7769_6d6e_6431; // "kgwimnd1"-like tag
const LEAF_DOMAIN: u64 = 0x6b67_7769_6d6c_6631;

/// RP57 over Fp in-circuit (243 rows).
fn perm(cs: &mut OpCs, mut st: [Lc; 3]) -> [Lc; 3] {
    let _scope = crate::blocks::scope("perm");
    use iroha_pasta::poseidon::{FULL_ROUNDS, PARTIAL_ROUNDS, PoseidonField, WIDTH};
    let params = <Fp as PoseidonField>::rp57();
    let half = FULL_ROUNDS / 2;
    for (round, rc) in params.round_constants().iter().enumerate() {
        let _step = crate::blocks::scope("perm_round");
        for (w, c) in st.iter_mut().zip(rc) {
            *w = w.plus(&Lc::constant(*c));
        }
        let full = round < half || round >= half + PARTIAL_ROUNDS;
        let lanes = if full { WIDTH } else { 1 };
        for w in st.iter_mut().take(lanes) {
            let x2 = cs.mul(w, w, 255);
            let x4 = cs.mul(&x2, &x2, 255);
            *w = cs.mul(&x4, w, 255);
        }
        let mds = params.mds();
        let next: Vec<Lc> = (0..WIDTH)
            .map(|i| {
                st.iter()
                    .zip(mds[i].iter())
                    .fold(Lc::default(), |acc, (w, m)| acc.plus(&w.scale(*m)))
            })
            .collect();
        st = [next[0].clone(), next[1].clone(), next[2].clone()];
    }
    st
}

fn native_perm(a: Fp, b: Fp, c: Fp) -> Fp {
    let mut st = [a, b, c];
    iroha_pasta::poseidon::permute(&mut st);
    st[0]
}

fn hash2(cs: &mut OpCs, a: &Lc, b: &Lc, domain: u64) -> Lc {
    perm(cs, [a.clone(), b.clone(), Lc::constant(Fp::from(domain))])[0].clone()
}

/// A sparse Merkle tree of the given depth over Fp (native, for consistent
/// openings).
struct Smt {
    depth: usize,
    nodes: std::collections::HashMap<(usize, u64), Fp>,
    empty: Vec<Fp>,
}

impl Smt {
    fn new(depth: usize) -> Self {
        let mut empty = vec![Fp::ZERO];
        for l in 0..depth {
            let e = empty[l];
            empty.push(native_perm(e, e, Fp::from(NODE_DOMAIN)));
        }
        Smt {
            depth,
            nodes: Default::default(),
            empty,
        }
    }
    fn get(&self, level: usize, idx: u64) -> Fp {
        self.nodes
            .get(&(level, idx))
            .copied()
            .unwrap_or(self.empty[level])
    }
    fn set_leaf(&mut self, mut idx: u64, leaf: Fp) {
        self.nodes.insert((0, idx), leaf);
        for l in 0..self.depth {
            let (left, right) = (self.get(l, idx & !1), self.get(l, idx | 1));
            idx >>= 1;
            self.nodes.insert(
                (l + 1, idx),
                native_perm(left, right, Fp::from(NODE_DOMAIN)),
            );
        }
    }
    fn path(&self, idx: u64) -> Vec<Fp> {
        (0..self.depth)
            .map(|l| self.get(l, (idx >> l) ^ 1))
            .collect()
    }
    fn root(&self) -> Fp {
        self.get(self.depth, 0)
    }
}

/// The root over a leaf and its siblings: per level a boolean, one product
/// (the conditional swap) and one permutation.
fn merkle_root(cs: &mut OpCs, leaf: &Lc, idx: u64, siblings: &[Fp]) -> Lc {
    let mut cur = leaf.clone();
    for (l, sib) in siblings.iter().enumerate() {
        let bit = cs.boolean((idx >> l) & 1 == 1);
        let s = Lc::var(cs.alloc(*sib, 255));
        let swap = cs.mul(&bit, &s.minus(&cur), 255);
        let left = cur.plus(&swap);
        let right = cur.plus(&s).minus(&left);
        cur = hash2(cs, &left, &right, NODE_DOMAIN);
    }
    cur
}

fn leaf_hash(cs: &mut OpCs, k: &Lc, v: &Lc, n: &Lc) -> Lc {
    perm(
        cs,
        [
            k.clone(),
            v.clone(),
            n.plus(&Lc::constant(Fp::from(LEAF_DOMAIN))),
        ],
    )[0]
    .clone()
}

fn native_leaf(k: Fp, v: Fp, n: Fp) -> Fp {
    native_perm(k, v, n + Fp::from(LEAF_DOMAIN))
}

/// An indexed-tree insertion: update the low leaf (k_low, v_low, next) to
/// point at k, and write (k, v, next) into an empty slot; four depth-32
/// paths and six leaf hashes, the intermediate root shared.
fn insertion(cs: &mut OpCs, smt: &mut Smt, seed: u64) {
    let _scope = crate::blocks::scope("smt_insertion");
    let (lo, hi) = (
        seed.wrapping_mul(0x9e37_79b9) & 0xffff_ffff,
        seed.wrapping_mul(0x85eb_ca6b).wrapping_add(7) & 0xffff_ffff,
    );
    let (k_low, v_low, next) = (
        Fp::from(seed * 1000 + 1),
        Fp::from(seed + 17),
        Fp::from(seed * 1000 + 999),
    );
    let (key, value) = (Fp::from(seed * 1000 + 500), Fp::from(seed + 99));
    smt.set_leaf(lo, native_leaf(k_low, v_low, next));
    let root0 = smt.root();
    let (kl, vl, nl) = (
        Lc::var(cs.alloc(k_low, 255)),
        Lc::var(cs.alloc(v_low, 255)),
        Lc::var(cs.alloc(next, 255)),
    );
    let (k, v) = (Lc::var(cs.alloc(key, 255)), Lc::var(cs.alloc(value, 255)));
    let old = leaf_hash(cs, &kl, &vl, &nl);
    let r0 = merkle_root(cs, &old, lo, &smt.path(lo));
    assert!(cs.val(&r0) == root0);
    smt.set_leaf(lo, native_leaf(k_low, v_low, key));
    let new_low = leaf_hash(cs, &kl, &vl, &k);
    let r1 = merkle_root(cs, &new_low, lo, &smt.path(lo));
    let r1b = merkle_root(cs, &Lc::constant(Fp::ZERO), hi, &smt.path(hi));
    cs.enforce_zero(&r1.minus(&r1b));
    smt.set_leaf(hi, native_leaf(key, value, next));
    let new_leaf = leaf_hash(cs, &k, &v, &nl);
    let r2 = merkle_root(cs, &new_leaf, hi, &smt.path(hi));
    assert!(cs.val(&r2) == smt.root());
    // The remaining leaf-level hashes (key and value preimages).
    for _ in 0..3 {
        let _ = leaf_hash(cs, &k, &v, &kl);
    }
}

/// a < b for canonical field elements: five range-checked chunk sets (a, b,
/// b − a − 1 and the two canonicity witnesses p − 1 − a, p − 1 − b).
fn comparison(cs: &mut OpCs, a: &BigUint, b: &BigUint) {
    let _scope = crate::blocks::scope("comparison");
    let p = fp_modulus();
    let av = Lc::var(cs.alloc(fp_u(a), 255));
    let bv = Lc::var(cs.alloc(fp_u(b), 255));
    let ra = cs.chunks(a, 255);
    cs.enforce_zero(&ra.minus(&av));
    let rb = cs.chunks(b, 255);
    cs.enforce_zero(&rb.minus(&bv));
    let d = b - a - 1u32;
    let rd = cs.chunks(&d, 255);
    cs.enforce_zero(&bv.minus(&av).minus(&Lc::constant(Fp::ONE)).minus(&rd));
    for (x, xv) in [(a, &av), (b, &bv)] {
        let c = &p - 1u32 - x;
        let rc = cs.chunks(&c, 255);
        cs.enforce_zero(&Lc::constant(fp_u(&(&p - 1u32))).minus(xv).minus(&rc));
    }
}

fn u128_check(cs: &mut OpCs, v: u128) {
    let _scope = crate::blocks::scope("u128_check");
    let x = Lc::var(cs.alloc(Fp::from_u128(v), 128));
    let r = cs.chunks(&BigUint::from(v), 128);
    cs.enforce_zero(&x.minus(&r));
}

/// A field element re-encoded as its 32 canonical bytes (8-bit chunks).
fn bytes_encode(cs: &mut OpCs, x: Fp) {
    let _scope = crate::blocks::scope("bytes_encode");
    let xv = Lc::var(cs.alloc(x, 255));
    let repr = x.to_repr();
    let mut rec = Lc::default();
    for (i, byte) in repr.iter().enumerate() {
        let b = cs.chunks(&BigUint::from(*byte), 8);
        rec = rec.plus(&b.scale(fp_u(&pow2(8 * i))));
    }
    cs.enforce_zero(&xv.minus(&rec));
}

/// Selector and branch logic: booleans and their products (n rows).
fn logic_rows(cs: &mut OpCs, n: usize, seed: u64) {
    let _scope = crate::blocks::scope("logic_rows");
    for i in 0..n / 3 {
        let a = cs.boolean((seed + i as u64) % 3 == 0);
        let b = cs.boolean((seed + i as u64) % 5 == 0);
        let _ = cs.mul(&a, &b, 1);
    }
}

/// One signed message under `key_label`'s key (both Receive signatures are
/// under the wallet's own key, so its comb tables are shared).
fn signed_message_with(cs: &mut OpCs, key_label: &str, msg_label: &str, key: SigKey) {
    let _scope = crate::blocks::scope("signed_message");
    let n = p256::n();
    let d = p256::scalar_below(&format!("{key_label}-d"), &n);
    let q = p256::mul(&p256::generator(), &d).unwrap();
    let m = fp_u(&p256::scalar_below(
        &format!("{msg_label}-m"),
        &fp_modulus(),
    ));
    let msg = canonical_bits(cs, m);
    let digest = if sha_lookup() {
        sha256_32_lookup(cs, &msg)
    } else {
        sha256_32(cs, &msg)
    };
    let z = BigUint::from_bytes_be(&p256::sha256_one_block(&m.to_repr()));
    let sig = p256::sign(&d, &z, &p256::scalar_below(&format!("{msg_label}-k"), &n));
    match key {
        SigKey::Variable => ecdsa(cs, &q, &z, Some(&digest), &sig, false),
        SigKey::FixedOdd => ecdsa(cs, &q, &z, Some(&digest), &sig, true),
        SigKey::Own(_) | SigKey::FixedComb(_) => ecdsa_comb(cs, &q, &z, Some(&digest), &sig, key),
    }
}

/// The Receive relation (stage A) as a recorded circuit: two own-key
/// signatures over comb tables with lookup SHA-256, the census's Poseidon
/// hashing, two indexed-tree insertions, four comparisons, ten u128 checks,
/// five byte re-encodings and the branch logic.
pub fn receive_relation(chunk: usize, m: usize) -> OpCs {
    let _scope = crate::blocks::scope("own");
    let op = operations()
        .into_iter()
        .find(|o| o.label == "Receive")
        .unwrap();
    let mut cs = OpCs::recording(chunk);
    set_sha_lookup(true);
    signed_message_with(&mut cs, "receiver", "tau_recv", SigKey::Own(m));
    signed_message_with(&mut cs, "receiver", "request", SigKey::Own(m));
    set_sha_lookup(false);
    // Heads, statement, Request terms, credit_id, chains, receipt and digests.
    let mut st = [
        Lc::constant(Fp::ZERO),
        Lc::constant(Fp::ZERO),
        Lc::constant(Fp::from(7u64)),
    ];
    for i in 0..op.counted.permutations {
        let a = Lc::var(cs.alloc(Fp::from(1000 + i as u64).square(), 255));
        let b = Lc::var(cs.alloc(Fp::from(2000 + i as u64).square(), 255));
        st[0] = st[0].plus(&a);
        st[1] = st[1].plus(&b);
        st = perm(&mut cs, st);
    }
    for t in 0..op.counted.insertions {
        let mut smt = Smt::new(DEPTH);
        insertion(&mut cs, &mut smt, 11 + t as u64);
    }
    for i in 0..op.counted.comparisons {
        let a = p256::scalar_below(&format!("cmp-a-{i}"), &(fp_modulus() >> 1));
        let b = &a + 1u32 + p256::scalar_below(&format!("cmp-d-{i}"), &pow2(200));
        comparison(&mut cs, &a, &b);
    }
    for i in 0..op.counted.u128_values {
        u128_check(
            &mut cs,
            0x0123_4567_89ab_cdef_u128.wrapping_mul(i as u128 + 3),
        );
    }
    for i in 0..op.counted.byte_elements {
        bytes_encode(&mut cs, Fp::from(31 + i as u64).square().square());
    }
    logic_rows(&mut cs, op.counted.logic_rows, 5);
    cs
}

/// Moves a recorded system into a circuit assembly.
pub fn to_assembly(cs: &OpCs, asm: &mut crate::sparse::Assembly<Fp>) {
    let rec = cs.rec.as_ref().expect("recorded");
    let map = asm.import_vars(&cs.vals, &cs.widths);
    let conv =
        |lc: &Lc| -> crate::sparse::SLc<Fp> { lc.0.iter().map(|(v, c)| (map[*v], *c)).collect() };
    let pad = |v: &mut Vec<u32>, len: usize| v.resize(len, 0);
    pad(&mut asm.row_scope, asm.rows.len());
    pad(&mut asm.lookup_scope, asm.lookups.len());
    for (i, (a, b, c)) in rec.rows.iter().enumerate() {
        asm.rows.push((conv(a), conv(b), conv(c)));
        asm.row_scope
            .push(rec.row_scope.get(i).copied().unwrap_or(0));
    }
    for (j, (name, tuple)) in rec.lookups.iter().enumerate() {
        let entries = &rec.tables[name];
        let tid = asm.table(name, || entries.clone());
        asm.lookup(tid, tuple.iter().map(conv).collect());
        asm.lookup_scope
            .push(rec.lookup_scope.get(j).copied().unwrap_or(0));
    }
    asm.filler += cs.counted_rows;
}
