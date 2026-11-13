//! Scalar half of the private exact generator restructuring proof.
use super::{Fold, SOURCE_COUNT, Trace};
use crate::{
    fs::Scalar,
    full::Wiring,
    ipa::F,
    uniform::{Array, Lin, constant, link, own, plus, scaled},
};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use iroha_pasta::PastaCurve;

pub type Port = (usize, usize, usize);
pub struct ScalarPorts {
    pub sources: Vec<Vec<Port>>,
    pub output: Vec<Port>,
}
#[derive(Clone)]
struct E<S> {
    v: S,
    l: Lin<S>,
}
impl<S: Field> E<S> {
    fn add(&self, o: &Self) -> Self {
        Self {
            v: self.v + o.v,
            l: plus(&self.l, &o.l),
        }
    }
    fn scale(&self, c: S) -> Self {
        Self {
            v: self.v * c,
            l: scaled(&self.l, c),
        }
    }
    fn sub(&self, o: &Self) -> Self {
        self.add(&o.scale(-S::ONE))
    }
}
struct B<S> {
    a: Array<S>,
    shared: usize,
    base: usize,
    values: Vec<u128>,
}
impl<S: PrimeField> B<S> {
    fn new(family: usize, shared: usize, base: usize, values: Vec<u128>) -> Self {
        Self {
            a: Array {
                name: "exact generator restructuring scalar equations",
                family,
                rho: 0,
                r_count: 1,
                fixed: false,
                lookups: vec![],
                widths: vec![],
                rows: vec![],
                values: vec![],
            },
            shared,
            base,
            values,
        }
    }
    fn constant(&self, v: S) -> E<S> {
        E {
            v,
            l: constant(vec![v]),
        }
    }
    fn shared(&self, i: usize) -> E<S> {
        E {
            v: S::from_u128(self.values[i]),
            l: link(self.shared, 0, vec![Some(self.base + i)], S::ONE),
        }
    }
    fn alloc(&mut self, v: S, width: u16) -> E<S> {
        let k = self.a.values.len();
        self.a.values.push(vec![v]);
        self.a.widths.push(width);
        E { v, l: own(k, 1) }
    }
    fn equal(&mut self, a: &E<S>, b: &E<S>) {
        assert!(a.v == b.v, "generator scalar equality");
        self.a
            .rows
            .push([a.sub(b).l, constant(vec![S::ONE]), vec![]]);
    }
    fn copy(&mut self, e: &E<S>) -> (E<S>, usize) {
        let k = self.a.values.len();
        let o = self.alloc(e.v, 255);
        self.equal(&o, e);
        (o, k)
    }
    fn scalar(&mut self, i: usize) -> (E<S>, usize) {
        let p = S::from(2).pow_vartime([128]);
        let e = self.shared(i).add(&self.shared(i + 1).scale(p));
        self.copy(&e)
    }
    fn mul(&mut self, a: &E<S>, b: &E<S>) -> E<S> {
        let o = self.alloc(a.v * b.v, 255);
        self.a.rows.push([a.l.clone(), b.l.clone(), o.l.clone()]);
        o
    }
    fn endo(&mut self, i: usize) -> E<S>
    where
        S: WithSmallOrderMulGroup<3>,
    {
        let raw = self.shared(i);
        let integer = self.values[i];
        let one = self.constant(S::ONE);
        let mut bits = vec![];
        let mut sum = self.constant(S::ZERO);
        let mut power = S::ONE;
        for bit in 0..126 {
            let b = self.alloc(S::from(((integer >> bit) & 1) as u64), 1);
            self.a.rows.push([b.l.clone(), b.sub(&one).l, vec![]]);
            sum = sum.add(&b.scale(power));
            power = power.double();
            bits.push(b);
        }
        self.equal(&sum, &raw);
        let mut a = self.constant(S::from(2));
        let mut b = a.clone();
        for j in (0..63).rev() {
            let sign = bits[2 * j].scale(S::from(2)).sub(&one);
            let se = self.mul(&sign, &bits[2 * j + 1]);
            a = a.scale(S::from(2)).add(&se);
            b = b.scale(S::from(2)).add(&sign.sub(&se));
        }
        let value = a.scale(S::ZETA).add(&b);
        self.copy(&value).0
    }
    fn closed(&mut self, us: &[E<S>], rs: &[E<S>]) -> E<S> {
        let one = self.constant(S::ONE);
        let mut out = one.clone();
        for (u, r) in us.iter().zip(rs) {
            let ru = self.mul(r, u);
            let factor = one.sub(r).add(&ru);
            out = self.mul(&out, &factor);
        }
        out
    }
}

/// `shared` is the same range-checked cross-field stream consumed by the
/// points builder. All returned sources must be bound to the exact preceding
/// proof challenge cells / inherited public IO; output cells feed late IO.
pub fn append_scalar<C: PastaCurve>(
    w: &mut Wiring<F<C>>,
    family: usize,
    shared: usize,
    base: usize,
    fold: &Fold<C>,
    tr: &Trace<C>,
) -> ScalarPorts
where
    F<C>: Scalar,
{
    let l = &tr.layout;
    let mut b = B::new(family, shared, base, tr.shared.clone());
    let a = w.arrays.len();
    let zero = b.constant(F::<C>::ZERO);
    b.equal(&b.shared(l.zero), &zero);
    let mut source_values = vec![];
    let mut sources = vec![];
    for j in 0..SOURCE_COUNT {
        let mut vals = vec![];
        let mut ports = vec![];
        for bit in 0..l.k {
            let (v, c) = b.scalar(l.source(j) + 4 + 2 * bit);
            vals.push(v);
            ports.push((a, c, 0));
        }
        source_values.push(vals);
        sources.push(ports);
    }
    let mut masks = vec![];
    for j in 0..1 + 2 * l.k {
        masks.push(b.scalar(l.masks + 2 * j).0);
    }
    let mu = b.endo(l.challenges);
    let rs = (0..l.k).map(|j| b.shared(l.r(j))).collect::<Vec<_>>();
    let zeta = b.endo(l.zeta());
    let mut us = vec![];
    let mut output = vec![];
    for j in 0..l.k {
        let u = b.endo(l.u(j));
        let (v, c) = b.scalar(l.output + 4 + 2 * j);
        b.equal(&v, &u);
        us.push(u);
        output.push((a, c, 0));
    }
    let mut weight = b.constant(F::<C>::ONE);
    let mut value = b.constant(F::<C>::ZERO);
    for source in &source_values {
        let v = b.closed(source, &rs);
        let weighted = b.mul(&weight, &v);
        value = value.add(&weighted);
        weight = b.mul(&weight, &mu);
    }
    let kz = b.mul(&value, &zeta);
    let kz_shared = b.scalar(l.kz).0;
    b.equal(&kz_shared, &kz);
    let bf = b.closed(&us, &rs);
    let c = b.scalar(l.c).0;
    assert_eq!(c.v, fold.c);
    let cb = b.mul(&c, &bf);
    let cbz = b.mul(&cb, &zeta);
    let cbz_shared = b.scalar(l.cbz).0;
    b.equal(&cbz_shared, &cbz);
    let mut gamma = b.constant(F::<C>::ZERO);
    for (i, u) in us.iter().enumerate() {
        let gu = b.mul(&gamma, u);
        let uu = b.mul(u, u);
        let lam = b.mul(&masks[1 + 2 * i], &uu);
        gamma = gu.add(&lam).add(&masks[2 + 2 * i]);
    }
    let gamma_shared = b.scalar(l.gamma).0;
    b.equal(&gamma_shared, &gamma);
    let adjusted = b.scalar(l.adjusted_c).0;
    let offset = b.constant(F::<C>::from(2).pow_vartime([255]) + F::<C>::ONE);
    b.equal(&adjusted.scale(F::<C>::from(2)).add(&offset), &c);
    w.push(b.a);
    ScalarPorts { sources, output }
}
