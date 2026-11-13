//! Complete native-field point arithmetic for total recursive verification.
//! Identity has the unique coordinate encoding (0,0). Every branch selector
//! and zero inverse is a hard, deterministic function of its exact inputs.
use crate::{
    ladders::Ladder,
    uniform::{Array, Lin, constant, link, own, plus, scaled},
};
use ff::{Field, PrimeField};
use group::{Curve, Group};
use iroha_pasta::{PastaAffine, PastaCurve};
pub fn coords<C: PastaCurve>(p: &C) -> (C::Base, C::Base) {
    Option::from(p.to_affine().coordinates()).unwrap_or((C::Base::ZERO, C::Base::ZERO))
}
#[derive(Clone)]
struct E<F: PrimeField> {
    lin: Lin<F>,
    values: Vec<F>,
}
impl<F: PrimeField> E<F> {
    fn add(&self, b: &Self) -> Self {
        Self {
            lin: plus(&self.lin, &b.lin),
            values: self
                .values
                .iter()
                .zip(&b.values)
                .map(|(a, b)| *a + *b)
                .collect(),
        }
    }
    fn scale(&self, f: F) -> Self {
        Self {
            lin: scaled(&self.lin, f),
            values: self.values.iter().map(|v| *v * f).collect(),
        }
    }
    fn sub(&self, b: &Self) -> Self {
        self.add(&b.scale(-F::ONE))
    }
}
struct B<F: PrimeField> {
    a: Array<F>,
    n: usize,
    slots: usize,
}
impl<F: PrimeField> B<F> {
    fn new(name: &'static str, family: usize, rho: usize, slots: usize, reserved: usize) -> Self {
        let n = 1 << rho;
        Self {
            a: Array {
                name,
                family,
                rho,
                r_count: n,
                fixed: false,
                lookups: vec![],
                rows: vec![],
                widths: vec![255; reserved],
                values: vec![vec![F::ZERO; slots * n]; reserved],
            },
            n,
            slots,
        }
    }
    fn constant(&self, v: F) -> E<F> {
        E {
            lin: constant(vec![v; self.n]),
            values: vec![v; self.slots * self.n],
        }
    }
    fn source(&self, lin: Lin<F>, values: Vec<F>) -> E<F> {
        assert_eq!(values.len(), self.slots * self.n);
        E { lin, values }
    }
    fn own(&self, k: usize) -> E<F> {
        E {
            lin: own(k, self.n),
            values: self.a.values[k].clone(),
        }
    }
    fn allocate(&mut self, v: Vec<F>, width: u16) -> E<F> {
        let k = self.a.values.len();
        self.a.values.push(v);
        self.a.widths.push(width);
        self.own(k)
    }
    fn equal(&mut self, a: &E<F>, b: &E<F>) {
        self.a
            .rows
            .push([a.sub(b).lin, constant(vec![F::ONE; self.n]), vec![]]);
    }
    fn put(&mut self, k: usize, v: &E<F>) {
        self.a.values[k] = v.values.clone();
        self.equal(&self.own(k), v);
    }
    fn mul(&mut self, a: &E<F>, b: &E<F>) -> E<F> {
        let out = self.allocate(
            a.values
                .iter()
                .zip(&b.values)
                .map(|(a, b)| *a * *b)
                .collect(),
            255,
        );
        self.a
            .rows
            .push([a.lin.clone(), b.lin.clone(), out.lin.clone()]);
        out
    }
    fn zero(&mut self, a: &E<F>) -> E<F> {
        let z = self.allocate(
            a.values
                .iter()
                .map(|v| if *v == F::ZERO { F::ONE } else { F::ZERO })
                .collect(),
            1,
        );
        let inv = self.allocate(
            a.values
                .iter()
                .map(|v| Option::<F>::from(v.invert()).unwrap_or(F::ZERO))
                .collect(),
            255,
        );
        self.a.rows.push([a.lin.clone(), z.lin.clone(), vec![]]);
        self.a.rows.push([
            a.lin.clone(),
            inv.lin.clone(),
            self.constant(F::ONE).sub(&z).lin,
        ]);
        self.a.rows.push([inv.lin.clone(), z.lin.clone(), vec![]]);
        self.a
            .rows
            .push([z.lin.clone(), z.lin.clone(), z.lin.clone()]);
        z
    }
    fn select(&mut self, z: &E<F>, yes: &E<F>, no: &E<F>) -> E<F> {
        self.mul(z, &yes.sub(no)).add(no)
    }
    fn add_point(&mut self, p: [E<F>; 2], q: [E<F>; 2]) -> [E<F>; 2] {
        let px = self.zero(&p[0]);
        let py = self.zero(&p[1]);
        let pinf = self.mul(&px, &py);
        let qx = self.zero(&q[0]);
        let qy = self.zero(&q[1]);
        let qinf = self.mul(&qx, &qy);
        let dx = q[0].sub(&p[0]);
        let dy = q[1].sub(&p[1]);
        let samex = self.zero(&dx);
        let samey = self.zero(&dy);
        let xx = self.mul(&p[0], &p[0]);
        let num = self.select(&samex, &xx.scale(F::from(3)), &dy);
        let den = self.select(&samex, &p[1].scale(F::from(2)), &dx);
        let dz = self.zero(&den);
        let inverse = self.allocate(
            den.values
                .iter()
                .map(|v| Option::<F>::from(v.invert()).unwrap_or(F::ZERO))
                .collect(),
            255,
        );
        self.a.rows.push([
            den.lin.clone(),
            inverse.lin.clone(),
            self.constant(F::ONE).sub(&dz).lin.clone(),
        ]);
        self.a
            .rows
            .push([dz.lin.clone(), inverse.lin.clone(), vec![]]);
        let slope = self.mul(&num, &inverse);
        let square = self.mul(&slope, &slope);
        let x = square.sub(&p[0]).sub(&q[0]);
        let y = self.mul(&slope, &p[0].sub(&x)).sub(&p[1]);
        let opposite = self.mul(&samex, &self.constant(F::ONE).sub(&samey));
        let overlap = self.mul(&dz, &opposite);
        let exceptional = dz.add(&opposite).sub(&overlap);
        let zero = self.constant(F::ZERO);
        let x = self.select(&exceptional, &zero, &x);
        let y = self.select(&exceptional, &zero, &y);
        let x = self.select(&qinf, &p[0], &x);
        let y = self.select(&qinf, &p[1], &y);
        [self.select(&pinf, &q[0], &x), self.select(&pinf, &q[1], &y)]
    }
}
fn bits_for(n: usize) -> usize {
    n.max(1).next_power_of_two().trailing_zeros() as usize
}
fn values<C: PastaCurve>(points: &[C], slots: usize) -> Vec<Vec<C::Base>> {
    let mut out = vec![vec![C::Base::ZERO; slots]; 2];
    for (i, p) in points.iter().enumerate() {
        let (x, y) = coords(p);
        out[0][i] = x;
        out[1][i] = y;
    }
    out
}
fn base_array<F: PrimeField>(
    name: &'static str,
    family: usize,
    values: Vec<Vec<F>>,
    fixed: bool,
) -> Array<F> {
    Array {
        name,
        family,
        rho: 0,
        r_count: 1,
        fixed,
        lookups: vec![],
        widths: vec![255; values.len()],
        rows: vec![],
        values,
    }
}

/// Inactive array slots have no constraints or witness positions. Complete
/// zero tests produce 1 on the dummy identity, so clear those padding-only
/// helper values before placement; every present arithmetic row is retained.
fn zero_absent<F: PrimeField>(array: &mut Array<F>, families: usize) {
    let n = 1usize << array.rho;
    for column in &mut array.values {
        for (i, value) in column.iter_mut().enumerate() {
            if i / n >= families || i % n >= array.r_count {
                *value = F::ZERO;
            }
        }
    }
}

/// Upgrade the DAG addition array while preserving its existing x/y ports.
pub fn additions<F: PrimeField>(
    family: usize,
    rho: usize,
    p: &[(F, F)],
    q: &[(F, F)],
    negative: &[bool],
) -> Array<F> {
    let n = 1 << rho;
    let mut b = B::new("complete total verifier additions", family, rho, 1, 10);
    for i in 0..p.len() {
        b.a.values[0][i] = p[i].0;
        b.a.values[1][i] = p[i].1;
        b.a.values[2][i] = q[i].0;
        b.a.values[3][i] = q[i].1;
    }
    let sign: E<F> = b.source(
        constant(
            (0..n)
                .map(|i| {
                    if negative.get(i).copied().unwrap_or(false) {
                        -F::ONE
                    } else {
                        F::ONE
                    }
                })
                .collect(),
        ),
        (0..n)
            .map(|i| {
                if negative.get(i).copied().unwrap_or(false) {
                    -F::ONE
                } else {
                    F::ONE
                }
            })
            .collect(),
    );
    let qy = b.mul(&b.own(3), &sign);
    b.put(4, &qy);
    let out = b.add_point([b.own(0), b.own(1)], [b.own(2), b.own(4)]);
    b.put(8, &out[0]);
    b.put(9, &out[1]);
    for k in [5, 6, 7] {
        b.equal(&b.own(k), &b.constant(F::ZERO));
    }
    b.a.r_count = p.len();
    zero_absent(&mut b.a, 1);
    b.a
}

pub fn endoscale<C: PastaCurve>(
    first: usize,
    family: usize,
    pi: usize,
    inputs: &[(C, Vec<bool>)],
) -> Ladder<C::Base> {
    type F<C> = <C as PastaCurve>::Base;
    let slots = 1 << pi;
    let steps = inputs[0].1.len() / 2;
    let rho = bits_for(steps);
    let n = 1 << rho;
    let beta = crate::ladders::beta::<C>();
    let one = F::<C>::ONE;
    let base = values(&inputs.iter().map(|(p, _)| *p).collect::<Vec<_>>(), slots);
    let mut init = B::new("complete endo init", family, 0, slots, 7);
    let p = [0, 1].map(|k| init.source(link(first, k, vec![Some(0)], one), base[k].clone()));
    let sum = init.add_point([p[0].scale(beta), p[1].clone()], p.clone());
    let start = init.add_point(sum.clone(), sum);
    init.put(5, &start[0]);
    init.put(6, &start[1]);
    for k in 0..5 {
        init.equal(&init.own(k), &init.constant(F::<C>::ZERO));
    }
    let mut step = B::new("complete endo steps", family, rho, slots, 10);
    let mut prev = vec![vec![F::<C>::ZERO; slots * n]; 2];
    let mut scalars = vec![];
    let mut results = vec![];
    for p in 0..slots {
        let (point, bits) = inputs
            .get(p)
            .cloned()
            .unwrap_or((C::identity(), vec![false; 2 * steps]));
        let mut acc = (point.endo() + point).double();
        let mut t = F::<C>::ZERO;
        for r in 0..n {
            let (x, y) = coords(&acc);
            prev[0][p * n + r] = x;
            prev[1][p * n + r] = y;
            let (s, e) = if r < steps {
                (bits[2 * (steps - 1 - r)], bits[2 * (steps - 1 - r) + 1])
            } else {
                (false, false)
            };
            step.a.values[0][p * n + r] = F::<C>::from(s as u64);
            step.a.values[1][p * n + r] = F::<C>::from(e as u64);
            let mut chosen = if s { point } else { -point };
            if e {
                chosen = chosen.endo();
            }
            acc = acc.double() + chosen;
            t = t * F::<C>::from(4) + F::<C>::from(s as u64) + F::<C>::from(2 * (e as u64));
            step.a.values[9][p * n + r] = t;
            if r + 1 == steps && p < inputs.len() {
                results.push(coords(&acc));
                scalars.push(t);
            }
        }
    }
    let previous: Vec<_> = (0..n).map(|r| r.checked_sub(1)).collect();
    let initial: Vec<_> = (0..n).map(|r| (r == 0).then_some(0)).collect();
    let acc = std::array::from_fn(|k| {
        step.source(
            plus(
                &link(first + 1, 5 + k, initial.clone(), one),
                &link(first + 2, 7 + k, previous.clone(), one),
            ),
            prev[k].clone(),
        )
    });
    let point = std::array::from_fn::<_, 2, _>(|k| {
        step.source(
            link(first, k, vec![Some(0); n], one),
            base[k]
                .iter()
                .flat_map(|v| std::iter::repeat_n(*v, n))
                .collect(),
        )
    });
    for k in [0, 1] {
        let bit = step.own(k);
        step.a
            .rows
            .push([bit.lin.clone(), bit.lin.clone(), bit.lin]);
        step.a.widths[k] = 1;
    }
    let x = step
        .mul(&step.own(1), &point[0].scale(beta - one))
        .add(&point[0]);
    step.put(2, &x);
    let y = step.mul(
        &step.own(0).scale(F::<C>::from(2)).sub(&step.constant(one)),
        &point[1],
    );
    step.put(3, &y);
    let doubled = step.add_point(acc.clone(), acc);
    let out = step.add_point(doubled, [step.own(2), step.own(3)]);
    step.put(7, &out[0]);
    step.put(8, &out[1]);
    let t = plus(
        &scaled(&link(first + 2, 9, previous, F::<C>::ONE), F::<C>::from(4)),
        &plus(&own(0, n), &scaled(&own(1, n), F::<C>::from(2))),
    );
    step.a.rows.push([
        plus(&own(9, n), &scaled(&t, -one)),
        constant(vec![one; n]),
        vec![],
    ]);
    for k in [4, 5, 6] {
        step.equal(&step.own(k), &step.constant(F::<C>::ZERO));
    }
    let mut arrays = vec![
        base_array("complete endo base", family, base, false),
        init.a,
        step.a,
    ];
    for array in &mut arrays {
        zero_absent(array, inputs.len());
    }
    Ladder {
        arrays,
        results,
        scalars,
    }
}

/// Same external offsets and scalar ports as ladders::mixed_mul; every step
/// and correction uses complete arithmetic, including a zero scalar result.
pub fn mixed_mul<C: PastaCurve>(
    first: usize,
    family: usize,
    pi: usize,
    inputs: &[(C, Vec<bool>, bool)],
) -> Ladder<C::Base> {
    type F<C> = <C as PastaCurve>::Base;
    let slots = 1 << pi;
    let bits = inputs[0].1.len();
    let rho = bits_for(bits);
    let n = 1 << rho;
    let one = F::<C>::ONE;
    let half = C::ScalarExt::from(2).invert().unwrap();
    let bases: Vec<_> = inputs
        .iter()
        .map(|(p, _, fixed)| if *fixed { *p * half } else { *p })
        .collect();
    let base = values(&bases, slots);
    let mut init = B::new("complete multiplication init", family, 0, slots, 4);
    let p = [0, 1].map(|k| init.source(link(first, k, vec![Some(0)], one), base[k].clone()));
    let out = init.add_point(p.clone(), p);
    init.put(2, &out[0]);
    init.put(3, &out[1]);
    for k in 0..2 {
        init.equal(&init.own(k), &init.constant(F::<C>::ZERO));
    }
    let mut step = B::new("complete multiplication steps", family, rho, slots, 8);
    let mut previous_values = vec![vec![F::<C>::ZERO; slots * n]; 2];
    let mut raw_results = vec![];
    let mut scalars = vec![];
    for p in 0..slots {
        let point = bases.get(p).copied().unwrap_or(C::identity());
        let mut acc = point.double();
        let mut t = F::<C>::ZERO;
        for r in 0..n {
            let (x, y) = coords(&acc);
            previous_values[0][p * n + r] = x;
            previous_values[1][p * n + r] = y;
            let bit = r < bits && inputs.get(p).is_some_and(|(_, b, _)| b[bits - 1 - r]);
            let v = F::<C>::from(bit as u64);
            step.a.values[0][p * n + r] = v;
            acc = acc.double() + if bit { point } else { -point };
            t = t.double() + v;
            step.a.values[7][p * n + r] = t;
            if r + 1 == bits && p < inputs.len() {
                raw_results.push(coords(&acc));
                scalars.push(t);
            }
        }
    }
    let previous: Vec<_> = (0..n).map(|r| r.checked_sub(1)).collect();
    let initial: Vec<_> = (0..n).map(|r| (r == 0).then_some(0)).collect();
    let acc = std::array::from_fn(|k| {
        step.source(
            plus(
                &link(first + 1, 2 + k, initial.clone(), one),
                &link(first + 2, 5 + k, previous.clone(), one),
            ),
            previous_values[k].clone(),
        )
    });
    let point = std::array::from_fn::<_, 2, _>(|k| {
        step.source(
            link(first, k, vec![Some(0); n], one),
            base[k]
                .iter()
                .flat_map(|v| std::iter::repeat_n(*v, n))
                .collect(),
        )
    });
    let bit = step.own(0);
    step.a.widths[0] = 1;
    step.a
        .rows
        .push([bit.lin.clone(), bit.lin.clone(), bit.lin]);
    let y = step.mul(
        &step.own(0).scale(F::<C>::from(2)).sub(&step.constant(one)),
        &point[1],
    );
    step.put(1, &y);
    let doubled = step.add_point(acc.clone(), acc);
    let out = step.add_point(doubled, [point[0].clone(), step.own(1)]);
    step.put(5, &out[0]);
    step.put(6, &out[1]);
    let t = plus(
        &scaled(&link(first + 2, 7, previous, one), F::<C>::from(2)),
        &own(0, n),
    );
    step.a.rows.push([
        plus(&own(7, n), &scaled(&t, -one)),
        constant(vec![one; n]),
        vec![],
    ]);
    for k in [2, 3, 4] {
        step.equal(&step.own(k), &step.constant(F::<C>::ZERO));
    }
    let mut tail = B::new("complete multiplication final ports", family, 2, slots, 12);
    for k in 0..12 {
        let source = match k {
            9 => Some(5),
            10 => Some(6),
            11 => Some(7),
            _ => None,
        };
        if let Some(source) = source {
            let v = (0..slots)
                .flat_map(|p| std::iter::repeat_n(step.a.values[source][p * n + bits - 1], 4))
                .collect();
            let e = tail.source(link(first + 2, source, vec![Some(bits - 1); 4], one), v);
            tail.put(k, &e);
        } else {
            tail.equal(&tail.own(k), &tail.constant(F::<C>::ZERO));
        }
    }
    let factor = C::ScalarExt::from(2).pow_vartime([bits as u64]) + C::ScalarExt::ONE;
    let mut fixed = vec![vec![F::<C>::ZERO; slots]; 2];
    let mut select = vec![vec![F::<C>::ZERO; slots]];
    let mut correction = vec![vec![F::<C>::ZERO; slots]; 2];
    for (p, (point, _, is_fixed)) in inputs.iter().enumerate() {
        if *is_fixed {
            fixed[0][p] = base[0][p];
            fixed[1][p] = base[1][p];
            select[0][p] = one;
            let (x, y) = coords(&(-(*point * (factor * half))));
            correction[0][p] = x;
            correction[1][p] = y;
        }
    }
    let mut check = base_array(
        "complete fixed multiplication binding",
        family,
        vec![],
        false,
    );
    for k in 0..2 {
        check.rows.push([
            plus(
                &link(first, k, vec![Some(0)], one),
                &link(first + 4, k, vec![Some(0)], -one),
            ),
            link(first + 5, 0, vec![Some(0)], one),
            vec![],
        ]);
    }
    let mut end = B::new("complete multiplication correction", family, 0, slots, 7);
    let point = std::array::from_fn(|k| {
        end.source(
            link(first + 3, 9 + k, vec![Some(2)], one),
            (0..slots)
                .map(|p| step.a.values[5 + k][p * n + bits - 1])
                .collect(),
        )
    });
    let corr = std::array::from_fn(|k| {
        end.source(
            link(first + 7, k, vec![Some(0)], one),
            correction[k].clone(),
        )
    });
    let out = end.add_point(point, corr);
    end.put(3, &out[0]);
    end.put(4, &out[1]);
    for k in 0..3 {
        end.equal(&end.own(k), &end.constant(F::<C>::ZERO));
    }
    let results = inputs
        .iter()
        .enumerate()
        .map(|(p, (_, _, fixed))| {
            if *fixed {
                (end.a.values[3][p], end.a.values[4][p])
            } else {
                raw_results[p]
            }
        })
        .collect();
    let mut arrays = vec![
        base_array("complete multiplication base", family, base, false),
        init.a,
        step.a,
        tail.a,
        base_array("complete multiplication fixed base", family, fixed, true),
        base_array("complete multiplication selector", family, select, true),
        check,
        base_array(
            "complete multiplication correction point",
            family,
            correction,
            true,
        ),
        end.a,
    ];
    for array in &mut arrays {
        zero_absent(array, inputs.len());
    }
    Ladder {
        arrays,
        results,
        scalars,
    }
}
