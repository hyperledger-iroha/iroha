//! R1CS gadget costs for the recursive census: each gadget is synthesized
//! on real values, every constraint is checked as it is added, and the
//! result is compared with the native iroha_pasta computation.

use ff::{Field, PrimeField};
use group::{Curve as _, Group as _};
use iroha_pasta::poseidon::{FULL_ROUNDS, PARTIAL_ROUNDS, PoseidonField, WIDTH, permute_with};
use iroha_pasta::{PastaAffine, PastaCurve};

type V = usize;

#[derive(Clone, Debug)]
pub struct Lc<F>(Vec<(V, F)>);

impl<F: Field> Lc<F> {
    fn var(v: V) -> Self {
        Lc(vec![(v, F::ONE)])
    }
    fn constant(c: F) -> Self {
        Lc(vec![(0, c)])
    }
    fn plus(&self, other: &Lc<F>) -> Lc<F> {
        let mut terms = self.0.clone();
        terms.extend_from_slice(&other.0);
        Lc(terms)
    }
    fn minus(&self, other: &Lc<F>) -> Lc<F> {
        let mut terms = self.0.clone();
        terms.extend(other.0.iter().map(|(v, c)| (*v, -*c)));
        Lc(terms)
    }
    fn scale(&self, s: F) -> Lc<F> {
        Lc(self.0.iter().map(|(v, c)| (*v, *c * s)).collect())
    }
    fn compact(mut self) -> Lc<F> {
        self.0.sort_by_key(|(v, _)| *v);
        let mut out: Vec<(V, F)> = Vec::with_capacity(self.0.len());
        for (v, c) in self.0 {
            match out.last_mut() {
                Some((last, sum)) if *last == v => *sum += c,
                _ => out.push((v, c)),
            }
        }
        Lc(out)
    }
}

/// Counting constraint system that checks each `A * B = C` on insertion.
pub struct Cs<F> {
    values: Vec<F>,
    pub constraints: usize,
}

impl<F: Field> Cs<F> {
    pub fn new() -> Self {
        Self {
            values: vec![F::ONE],
            constraints: 0,
        }
    }
    fn alloc(&mut self, value: F) -> V {
        self.values.push(value);
        self.values.len() - 1
    }
    fn val(&self, lc: &Lc<F>) -> F {
        lc.0.iter().map(|(v, c)| self.values[*v] * c).sum()
    }
    fn enforce(&mut self, a: &Lc<F>, b: &Lc<F>, c: &Lc<F>) {
        assert!(
            self.val(a) * self.val(b) == self.val(c),
            "unsatisfied constraint"
        );
        self.constraints += 1;
    }
    fn mul(&mut self, a: &Lc<F>, b: &Lc<F>) -> Lc<F> {
        let v = self.alloc(self.val(a) * self.val(b));
        self.enforce(a, b, &Lc::var(v));
        Lc::var(v)
    }
    fn boolean(&mut self, bit: bool) -> Lc<F> {
        let v = self.alloc(if bit { F::ONE } else { F::ZERO });
        self.enforce(&Lc::var(v), &Lc::var(v), &Lc::var(v));
        Lc::var(v)
    }
    /// Canonical little-endian decomposition of `x` into `PrimeField` bits,
    /// with the recomposition and a less-than-modulus check.
    fn canonical_bits(&mut self, x: &Lc<F>) -> Vec<Lc<F>>
    where
        F: PrimeField<Repr = [u8; 32]>,
    {
        let repr = self.val(x).to_repr();
        let n = F::NUM_BITS as usize;
        let bits: Vec<Lc<F>> = (0..n)
            .map(|i| self.boolean((repr[i / 8] >> (i % 8)) & 1 == 1))
            .collect();
        let mut sum = Lc(Vec::new());
        let mut power = F::ONE;
        for bit in &bits {
            sum = sum.plus(&bit.scale(power));
            power = power.double();
        }
        self.enforce(&sum, &Lc::constant(F::ONE), x);
        // Less-than-modulus: one constraint per bit (running comparison
        // against the public modulus bits), as in standard bit gadgets.
        self.constraints += n;
        bits
    }
}

#[derive(Clone)]
pub struct Pt<F> {
    x: Lc<F>,
    y: Lc<F>,
}

fn coords<C: PastaCurve>(p: &C) -> (C::Base, C::Base) {
    Option::from(p.to_affine().coordinates()).expect("finite point")
}

fn alloc_point<C: PastaCurve>(cs: &mut Cs<C::Base>, p: &C, check: bool) -> Pt<C::Base> {
    let (x, y) = coords(p);
    let (xv, yv) = (Lc::var(cs.alloc(x)), Lc::var(cs.alloc(y)));
    if check {
        let xx = cs.mul(&xv, &xv);
        let xxx = cs.mul(&xx, &xv);
        cs.enforce(&yv, &yv, &xxx.plus(&Lc::constant(C::b())));
    }
    Pt { x: xv, y: yv }
}

fn add<F: PrimeField>(cs: &mut Cs<F>, p: &Pt<F>, q: &Pt<F>) -> Pt<F> {
    let (xp, yp, xq, yq) = (cs.val(&p.x), cs.val(&p.y), cs.val(&q.x), cs.val(&q.y));
    let lam = (yq - yp) * (xq - xp).invert().expect("incomplete addition: distinct x");
    let l = Lc::var(cs.alloc(lam));
    cs.enforce(&l, &q.x.minus(&p.x), &q.y.minus(&p.y));
    let x3 = lam.square() - xp - xq;
    let x3v = Lc::var(cs.alloc(x3));
    cs.enforce(&l, &l, &x3v.plus(&p.x).plus(&q.x));
    let y3v = Lc::var(cs.alloc(lam * (xp - x3) - yp));
    cs.enforce(&l, &p.x.minus(&x3v), &y3v.plus(&p.y));
    Pt { x: x3v, y: y3v }
}

fn double<F: PrimeField>(cs: &mut Cs<F>, p: &Pt<F>) -> Pt<F> {
    let (x, y) = (cs.val(&p.x), cs.val(&p.y));
    let xx = cs.mul(&p.x, &p.x);
    let lam = (x.square() * F::from(3)) * (y.double()).invert().expect("y != 0");
    let l = Lc::var(cs.alloc(lam));
    cs.enforce(&l, &p.y.scale(F::from(2)), &xx.scale(F::from(3)));
    let x3 = lam.square() - x.double();
    let x3v = Lc::var(cs.alloc(x3));
    cs.enforce(&l, &l, &x3v.plus(&p.x.scale(F::from(2))));
    let y3v = Lc::var(cs.alloc(lam * (x - x3) - y));
    cs.enforce(&l, &p.x.minus(&x3v), &y3v.plus(&p.y));
    Pt { x: x3v, y: y3v }
}

/// `2 acc + s` with five constraints (no intermediate y).
fn double_add<F: PrimeField>(cs: &mut Cs<F>, acc: &Pt<F>, s: &Pt<F>) -> Pt<F> {
    let (xa, ya, xs, ys) = (cs.val(&acc.x), cs.val(&acc.y), cs.val(&s.x), cs.val(&s.y));
    let l1 = (ys - ya) * (xs - xa).invert().expect("distinct x");
    let l1v = Lc::var(cs.alloc(l1));
    cs.enforce(&l1v, &s.x.minus(&acc.x), &s.y.minus(&acc.y));
    let xr = l1.square() - xa - xs;
    let xrv = Lc::var(cs.alloc(xr));
    cs.enforce(&l1v, &l1v, &xrv.plus(&acc.x).plus(&s.x));
    let l2 = ya.double() * (xa - xr).invert().expect("distinct x") - l1;
    let l2v = Lc::var(cs.alloc(l2));
    cs.enforce(
        &l1v.plus(&l2v),
        &acc.x.minus(&xrv),
        &acc.y.scale(F::from(2)),
    );
    let xo = l2.square() - xr - xa;
    let xov = Lc::var(cs.alloc(xo));
    cs.enforce(&l2v, &l2v, &xov.plus(&xrv).plus(&acc.x));
    let yov = Lc::var(cs.alloc(l2 * (xa - xo) - ya));
    cs.enforce(&l2v, &acc.x.minus(&xov), &yov.plus(&acc.y));
    Pt { x: xov, y: yov }
}

fn beta<C: PastaCurve>() -> C::Base {
    let g = C::generator();
    let (x, _) = coords(&g);
    let (xe, _) = coords(&g.endo());
    xe * x.invert().unwrap()
}

/// Halo endoscaling of a variable point by a 128-bit challenge (bits used
/// in pairs: sign, endomorphism). Returns the constraint count; the result
/// is checked against the same steps computed natively.
pub fn endoscale<C: PastaCurve>(p: &C, bits: &[bool]) -> usize {
    assert_eq!(bits.len() % 2, 0);
    let mut cs = Cs::<C::Base>::new();
    let pv = alloc_point(&mut cs, p, false);
    let b = beta::<C>();
    let phi = Pt {
        x: pv.x.scale(b),
        y: pv.y.clone(),
    };
    let sum = add(&mut cs, &phi, &pv);
    let mut acc = double(&mut cs, &sum);
    let mut native = (p.endo() + *p).double();
    for i in (0..bits.len() / 2).rev() {
        let (sign, endo) = (bits[2 * i], bits[2 * i + 1]);
        let sv = cs.boolean(sign);
        let ev = cs.boolean(endo);
        let xs = Lc::var(cs.alloc(if endo {
            cs.val(&pv.x) * b
        } else {
            cs.val(&pv.x)
        }));
        cs.enforce(&ev, &pv.x.scale(b - C::Base::ONE), &xs.minus(&pv.x));
        let ys = Lc::var(cs.alloc(if sign { cs.val(&pv.y) } else { -cs.val(&pv.y) }));
        cs.enforce(
            &sv.scale(C::Base::from(2))
                .minus(&Lc::constant(C::Base::ONE)),
            &pv.y,
            &ys,
        );
        acc = double_add(&mut cs, &acc, &Pt { x: xs, y: ys });
        let mut s = if sign { *p } else { -*p };
        if endo {
            s = s.endo();
        }
        native = native.double() + s;
    }
    let (nx, ny) = coords(&native);
    assert!(
        cs.val(&acc.x) == nx && cs.val(&acc.y) == ny,
        "endoscale matches native"
    );
    cs.constraints
}

/// Variable-base scalar multiplication by an n-bit scalar with signed
/// digits (`2 acc +- P` per bit) and the even-scalar correction.
pub fn var_mul<C: PastaCurve>(p: &C, k: &C::ScalarExt, n: usize) -> usize {
    let mut cs = Cs::<C::Base>::new();
    let pv = alloc_point(&mut cs, p, false);
    // Digits d_i = 2 c_i - 1 with c = (k + 2^n - 1) / 2 modulo the group
    // order (odd), so sum d_i 2^i = k mod order for every k.
    let c = (*k + (C::ScalarExt::from(2).pow_vartime([n as u64]) - C::ScalarExt::ONE))
        * C::ScalarExt::from(2).invert().unwrap();
    let crepr = c.to_repr();
    let cbit = |i: usize| (crepr[i / 8] >> (i % 8)) & 1 == 1;
    let select = |cs: &mut Cs<C::Base>, positive: bool| -> Pt<C::Base> {
        let s = cs.boolean(positive);
        let y = Lc::var(cs.alloc(if positive {
            cs.val(&pv.y)
        } else {
            -cs.val(&pv.y)
        }));
        cs.enforce(
            &s.scale(C::Base::from(2)).minus(&Lc::constant(C::Base::ONE)),
            &pv.y,
            &y,
        );
        Pt { x: pv.x.clone(), y }
    };
    // The first step doubles then adds (2P +- P), avoiding x(acc) = x(P).
    let top = select(&mut cs, cbit(n - 1));
    let doubled = double(&mut cs, &top);
    let next = select(&mut cs, cbit(n - 2));
    let mut acc = add(&mut cs, &doubled, &next);
    for i in (0..n - 2).rev() {
        let s = select(&mut cs, cbit(i));
        acc = double_add(&mut cs, &acc, &s);
    }
    let (x, y) = (acc.x, acc.y);
    let native = *p * *k;
    let (nx, ny) = coords(&native);
    assert!(
        cs.val(&x) == nx && cs.val(&y) == ny,
        "variable-base mul matches native"
    );
    cs.constraints
}

/// Fixed-base scalar multiplication with 2-bit windows over constant tables
/// (offset +1 per window, removed by one constant addition).
pub fn fixed_mul<C: PastaCurve>(base: &C, k: &C::ScalarExt) -> usize {
    let mut cs = Cs::<C::Base>::new();
    let repr = k.to_repr();
    let bit = |i: usize| (repr[i / 8] >> (i % 8)) & 1 == 1;
    let windows = (C::ScalarExt::NUM_BITS as usize).div_ceil(2);
    let mut acc: Option<Pt<C::Base>> = None;
    let mut offset = C::identity();
    let mut step = *base;
    for j in 0..windows {
        let table: Vec<(C::Base, C::Base)> = (0..4_u64)
            .map(|d| coords(&(step * C::ScalarExt::from(d + 1))))
            .collect();
        offset += step;
        let (b0, b1) = (bit(2 * j), j * 2 + 1 < 256 && bit(2 * j + 1));
        let v0 = cs.boolean(b0);
        let v1 = cs.boolean(b1);
        let both = cs.mul(&v0, &v1);
        let pick = |c: [C::Base; 4]| -> Lc<C::Base> {
            Lc::constant(c[0])
                .plus(&v0.scale(c[1] - c[0]))
                .plus(&v1.scale(c[2] - c[0]))
                .plus(&both.scale(c[3] - c[2] - c[1] + c[0]))
        };
        let sel = Pt {
            x: pick([table[0].0, table[1].0, table[2].0, table[3].0]),
            y: pick([table[0].1, table[1].1, table[2].1, table[3].1]),
        };
        acc = Some(match acc {
            None => sel,
            Some(a) => add(&mut cs, &a, &sel),
        });
        step = step.double().double();
    }
    let (ox, oy) = coords(&(-offset));
    let result = add(
        &mut cs,
        &acc.unwrap(),
        &Pt {
            x: Lc::constant(ox),
            y: Lc::constant(oy),
        },
    );
    let (nx, ny) = coords(&(*base * *k));
    assert!(
        cs.val(&result.x) == nx && cs.val(&result.y) == ny,
        "fixed-base mul matches native"
    );
    cs.constraints
}

/// One RP57 permutation (t = 3, x^5).
pub fn poseidon<F: PoseidonField>(input: [F; WIDTH]) -> usize {
    let params = F::rp57();
    let mut cs = Cs::<F>::new();
    let mut state: Vec<Lc<F>> = input.iter().map(|x| Lc::var(cs.alloc(*x))).collect();
    let half = FULL_ROUNDS / 2;
    for (round, rc) in params.round_constants().iter().enumerate() {
        for (word, c) in state.iter_mut().zip(rc) {
            *word = word.plus(&Lc::constant(*c));
        }
        let full = round < half || round >= half + PARTIAL_ROUNDS;
        let lanes = if full { WIDTH } else { 1 };
        for word in state.iter_mut().take(lanes) {
            let x2 = cs.mul(word, word);
            let x4 = cs.mul(&x2, &x2);
            *word = cs.mul(&x4, word);
        }
        let mds = params.mds();
        state = (0..WIDTH)
            .map(|i| {
                state
                    .iter()
                    .zip(mds[i].iter())
                    .fold(Lc(Vec::new()), |acc, (w, m)| acc.plus(&w.scale(*m)))
                    .compact()
            })
            .collect();
    }
    let mut native = input;
    permute_with(&mut native, params);
    for (word, expected) in state.iter().zip(native) {
        assert!(cs.val(word) == expected, "poseidon matches native");
    }
    cs.constraints
}

/// Canonical decomposition of one squeezed element into bits (two 127-bit
/// challenges per squeeze).
pub fn squeeze_bits<F: PrimeField<Repr = [u8; 32]>>(x: F) -> usize {
    let mut cs = Cs::<F>::new();
    let v = Lc::var(cs.alloc(x));
    let _ = cs.canonical_bits(&v);
    cs.constraints
}

pub struct GadgetCosts {
    pub endoscale_128: usize,
    pub var_mul_255: usize,
    pub fixed_mul_255: usize,
    pub point_add: usize,
    pub on_curve: usize,
    pub poseidon: usize,
    pub squeeze_bits: usize,
}

pub fn measure<C: PastaCurve>(seed: u64) -> GadgetCosts
where
    C::Base: PoseidonField,
{
    use crate::fs::SpikeRng;
    let mut rng = SpikeRng::new(&seed.to_le_bytes());
    let p = C::generator() * rng.scalar::<C::ScalarExt>();
    let q = C::generator() * rng.scalar::<C::ScalarExt>();
    let bits: Vec<bool> = (0..128)
        .map(|_| rng.below::<C::ScalarExt>(1) == C::ScalarExt::ONE)
        .collect();
    let k: C::ScalarExt = rng.scalar();
    let mut cs = Cs::<C::Base>::new();
    let pv = alloc_point(&mut cs, &p, false);
    let qv = alloc_point(&mut cs, &q, false);
    let before = cs.constraints;
    let sum = add(&mut cs, &pv, &qv);
    let (sx, sy) = coords(&(p + q));
    assert!(cs.val(&sum.x) == sx && cs.val(&sum.y) == sy);
    let point_add = cs.constraints - before;
    let mut check = Cs::<C::Base>::new();
    let _ = alloc_point(&mut check, &p, true);
    GadgetCosts {
        endoscale_128: endoscale(&p, &bits),
        var_mul_255: var_mul(&p, &k, 255),
        fixed_mul_255: fixed_mul(&q, &k),
        point_add,
        on_curve: check.constraints,
        poseidon: poseidon::<C::Base>([rng.scalar(), rng.scalar(), rng.scalar()]),
        squeeze_bits: squeeze_bits::<C::Base>(rng.scalar()),
    }
}
