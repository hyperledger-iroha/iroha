//! Complete native-field point arithmetic for total recursive verification.
//! Identity has the unique coordinate encoding (0,0). Every branch selector
//! and zero inverse is a hard, deterministic function of its exact inputs.
//!
//! A ladder step doubles and adds in complete projective coordinates (Renes,
//! Costello and Batina 2016, algorithms 7 and 9 for `a = 0`): products only,
//! no branch and no inverse, complete because both Pasta groups have odd
//! order. Each step still writes its affine sum, with the same zero inverse,
//! so the ports and the `(0, 0)` identity are unchanged.
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
        self.zero_and_inverse(a).0
    }
    /// The zero flag of `a` and its inverse, which is zero at zero.
    fn zero_and_inverse(&mut self, a: &E<F>) -> (E<F>, E<F>) {
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
        (z, inv)
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
    /// `a * b` as reserved cell `k`: one row and no further cell.
    fn product_into(&mut self, k: usize, a: &E<F>, b: &E<F>) -> E<F> {
        self.a.values[k] = a
            .values
            .iter()
            .zip(&b.values)
            .map(|(a, b)| *a * *b)
            .collect();
        let out = self.own(k);
        self.a
            .rows
            .push([a.lin.clone(), b.lin.clone(), out.lin.clone()]);
        out
    }
    /// Whether an affine point is the identity, encoded `(0, 0)`.
    fn infinity(&mut self, p: &[E<F>; 2]) -> E<F> {
        let x = self.zero(&p[0]);
        let y = self.zero(&p[1]);
        self.mul(&x, &y)
    }
    /// Projective coordinates of an affine point from its identity flag:
    /// `(x, y, 1)`, and `(0, 1, 0)` for the identity `(0, 0)`.
    fn lift(&self, p: &[E<F>; 2], infinity: &E<F>) -> [E<F>; 3] {
        [
            p[0].clone(),
            p[1].add(infinity),
            self.constant(F::ONE).sub(infinity),
        ]
    }
    /// Complete projective doubling on `y^2 = x^3 + b`, `b3 = 3b`: eight
    /// products. The first three take the reserved cells `spare`.
    fn double_projective(&mut self, p: &[E<F>; 3], b3: F, spare: [usize; 3]) -> [E<F>; 3] {
        let [x, y, z] = p;
        let yy = self.product_into(spare[0], y, y);
        let yz = self.product_into(spare[1], y, z);
        let zz = self.product_into(spare[2], z, z);
        let t2 = zz.scale(b3);
        let z8 = yy.scale(F::from(8));
        let t2z8 = self.mul(&t2, &z8);
        let z3 = self.mul(&yz, &z8);
        let t0 = yy.sub(&t2.scale(F::from(3)));
        let y3 = self.mul(&t0, &yy.add(&t2)).add(&t2z8);
        let xy = self.mul(x, y);
        let x3 = self.mul(&t0, &xy).scale(F::from(2));
        [x3, y3, z3]
    }
    /// Complete projective addition: twelve products. Also returns the cells
    /// of the six closing products; the sum is `(c0 - c1, c2 + c3, c4 + c5)`,
    /// which the next instance links.
    fn add_projective(&mut self, p: &[E<F>; 3], q: &[E<F>; 3], b3: F) -> ([E<F>; 3], [usize; 6]) {
        let [x1, y1, z1] = p;
        let [x2, y2, z2] = q;
        let t0 = self.mul(x1, x2);
        let t1 = self.mul(y1, y2);
        let t2 = self.mul(z1, z2);
        let t3 = self.mul(&x1.add(y1), &x2.add(y2)).sub(&t0).sub(&t1);
        let t4 = self.mul(&y1.add(z1), &y2.add(z2)).sub(&t1).sub(&t2);
        let t5 = self.mul(&x1.add(z1), &x2.add(z2)).sub(&t0).sub(&t2);
        let a0 = t0.scale(F::from(3));
        let b2 = t2.scale(b3);
        let z = t1.add(&b2);
        let d = t1.sub(&b2);
        let y5 = t5.scale(b3);
        let first = self.a.values.len();
        let closing = [
            self.mul(&t3, &d),
            self.mul(&t4, &y5),
            self.mul(&d, &z),
            self.mul(&y5, &a0),
            self.mul(&z, &t4),
            self.mul(&a0, &t3),
        ];
        (
            [
                closing[0].sub(&closing[1]),
                closing[2].add(&closing[3]),
                closing[4].add(&closing[5]),
            ],
            std::array::from_fn(|k| first + k),
        )
    }
    /// Affine coordinates of a projective point in the reserved cells `out`.
    /// The inverse is zero at `Z = 0`, so the identity comes out as `(0, 0)`.
    fn normalize_into(&mut self, p: &[E<F>; 3], out: [usize; 2]) -> [E<F>; 2] {
        let (_, inverse) = self.zero_and_inverse(&p[2]);
        let x = self.product_into(out[0], &p[0], &inverse);
        let y = self.product_into(out[1], &p[1], &inverse);
        [x, y]
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

/// Where a ladder step keeps its projective arithmetic: three reserved cells
/// for the doubling's first products, the affine sum's two reserved cells and
/// the six closing products of `add_projective`, which the next instance
/// links.
struct StepCells {
    spare: [usize; 3],
    out: [usize; 2],
    carry: [usize; 6],
}
const ENDO: StepCells = StepCells {
    spare: [4, 5, 6],
    out: [7, 8],
    carry: [21, 22, 23, 24, 25, 26],
};
const MIXED: StepCells = StepCells {
    spare: [2, 3, 4],
    out: [5, 6],
    carry: [19, 20, 21, 22, 23, 24],
};

/// `3b` of the Pasta curves `y^2 = x^3 + 5`, checked on the generator.
fn curve_b3<C: PastaCurve>() -> C::Base {
    let (x, y) = coords(&C::generator());
    assert!(
        y.square() == x.square() * x + C::Base::from(5),
        "the projective formulas assume y^2 = x^3 + 5"
    );
    C::Base::from(15)
}

/// The identity flag of an affine point encoded `(0, 0)` at the identity.
fn infinity_native<F: PrimeField>((x, y): (F, F)) -> F {
    F::from(u64::from(x == F::ZERO && y == F::ZERO))
}

/// `B::double_projective` on plain field elements: the witness a step reads.
fn double_native<F: PrimeField>([x, y, z]: [F; 3], b3: F) -> [F; 3] {
    let (yy, yz, t2) = (y * y, y * z, b3 * z * z);
    let z8 = yy * F::from(8);
    let t0 = yy - t2 * F::from(3);
    [(t0 * (x * y)).double(), t0 * (yy + t2) + t2 * z8, yz * z8]
}

/// `B::add_projective` on plain field elements.
fn add_native<F: PrimeField>([x1, y1, z1]: [F; 3], [x2, y2, z2]: [F; 3], b3: F) -> [F; 3] {
    let (t0, t1, t2) = (x1 * x2, y1 * y2, z1 * z2);
    let t3 = (x1 + y1) * (x2 + y2) - t0 - t1;
    let t4 = (y1 + z1) * (y2 + z2) - t1 - t2;
    let t5 = (x1 + z1) * (x2 + z2) - t0 - t2;
    let (a0, b2) = (t0 * F::from(3), t2 * b3);
    let (z, d, y5) = (t1 + b2, t1 - b2, t5 * b3);
    [t3 * d - t4 * y5, d * z + y5 * a0, z * t4 + a0 * t3]
}

/// The projective accumulator a ladder step reads. Its first instance lifts
/// the affine start in the init array's cells `start` with the base's
/// identity flag in cell `flag`; every later instance reads the previous
/// instance's sum from the step's carried cells. Arrays as in the ladders:
/// `first + 1` the init, `first + 2` the steps.
fn accumulator<F: PrimeField>(
    step: &B<F>,
    first: usize,
    start: [usize; 2],
    flag: usize,
    carry: [usize; 6],
    values: Vec<Vec<F>>,
) -> [E<F>; 3] {
    let (n, one) = (step.n, F::ONE);
    let previous: Vec<_> = (0..n).map(|r| r.checked_sub(1)).collect();
    let initial: Vec<_> = (0..n).map(|r| (r == 0).then_some(0)).collect();
    let at_start: Vec<_> = (0..n).map(|r| if r == 0 { one } else { F::ZERO }).collect();
    let opening = |cell: usize, sign: F| link(first + 1, cell, initial.clone(), sign);
    let carried = |cell: usize, sign: F| link(first + 2, cell, previous.clone(), sign);
    let [c0, c1, c2, c3, c4, c5] = carry;
    let lins = [
        plus(
            &opening(start[0], one),
            &plus(&carried(c0, one), &carried(c1, -one)),
        ),
        plus(
            &plus(&opening(start[1], one), &opening(flag, one)),
            &plus(&carried(c2, one), &carried(c3, one)),
        ),
        plus(
            &plus(&constant(at_start), &opening(flag, -one)),
            &plus(&carried(c4, one), &carried(c5, one)),
        ),
    ];
    let mut values = values.into_iter();
    lins.map(|lin| step.source(lin, values.next().expect("three coordinates")))
}

/// One step, `2 acc + chosen`, in complete projective arithmetic, with its
/// affine sum in the reserved cells. `chosen` is affine with the identity
/// flag `infinity` of the ladder's base.
fn double_and_add<F: PrimeField>(
    step: &mut B<F>,
    acc: &[E<F>; 3],
    chosen: &[E<F>; 2],
    infinity: &E<F>,
    b3: F,
    cells: &StepCells,
) {
    let chosen = step.lift(chosen, infinity);
    let doubled = step.double_projective(acc, b3, cells.spare);
    let (sum, carry) = step.add_projective(&doubled, &chosen, b3);
    assert_eq!(carry, cells.carry, "the cells the next instance links");
    step.normalize_into(&sum, cells.out);
    // The witness of those links: each instance's sum is what the next reads.
    let n = step.n;
    for i in (0..step.slots * n).filter(|i| (i + 1) % n != 0) {
        assert!(
            (0..3).all(|k| sum[k].values[i] == acc[k].values[i + 1]),
            "carried projective accumulator"
        );
    }
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
    let b3 = curve_b3::<C>();
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
    // The base's identity flag, which every step lifts with. The start
    // 2 (endo(P) + P) is the identity exactly when P is: a finite P has
    // endo(P) != -P, and the group has odd order.
    let infinity = init.infinity(&p);
    let flag = init.a.values.len() - 1;
    let mut step = B::new("complete endo steps", family, rho, slots, 10);
    let mut prev = vec![vec![F::<C>::ZERO; slots * n]; 3];
    let mut scalars = vec![];
    let mut results = vec![];
    for p in 0..slots {
        let (point, bits) = inputs
            .get(p)
            .cloned()
            .unwrap_or((C::identity(), vec![false; 2 * steps]));
        let inf = infinity_native(coords(&point));
        let mut group = (point.endo() + point).double();
        let (sx, sy) = coords(&group);
        let mut acc = [sx, sy + inf, one - inf];
        let mut t = F::<C>::ZERO;
        for r in 0..n {
            for (column, value) in prev.iter_mut().zip(acc) {
                column[p * n + r] = value;
            }
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
            let (cx, cy) = coords(&chosen);
            acc = add_native(double_native(acc, b3), [cx, cy + inf, one - inf], b3);
            group = group.double() + chosen;
            t = t * F::<C>::from(4) + F::<C>::from(s as u64) + F::<C>::from(2 * (e as u64));
            step.a.values[9][p * n + r] = t;
            if r + 1 == steps && p < inputs.len() {
                results.push(coords(&group));
                scalars.push(t);
            }
        }
    }
    let previous: Vec<_> = (0..n).map(|r| r.checked_sub(1)).collect();
    let acc = accumulator(&step, first, [5, 6], flag, ENDO.carry, prev);
    let point = std::array::from_fn::<_, 2, _>(|k| {
        step.source(
            link(first, k, vec![Some(0); n], one),
            base[k]
                .iter()
                .flat_map(|v| std::iter::repeat_n(*v, n))
                .collect(),
        )
    });
    let infinite = step.source(
        link(first + 1, flag, vec![Some(0); n], one),
        infinity
            .values
            .iter()
            .flat_map(|v| std::iter::repeat_n(*v, n))
            .collect(),
    );
    for k in [0, 1] {
        let bit = step.own(k);
        step.a
            .rows
            .push([bit.lin.clone(), bit.lin.clone(), bit.lin]);
        step.a.widths[k] = 1;
    }
    // Cells 2 and 3, the chosen point: x (1 + (beta - 1) e) and (2 s - 1) y.
    let factor = step.constant(one).add(&step.own(1).scale(beta - one));
    let x = step.product_into(2, &point[0], &factor);
    let sign = step.own(0).scale(F::<C>::from(2)).sub(&step.constant(one));
    let y = step.product_into(3, &sign, &point[1]);
    double_and_add(&mut step, &acc, &[x, y], &infinite, b3, &ENDO);
    for (p, result) in results.iter().enumerate() {
        let last = p * n + steps - 1;
        assert_eq!(
            *result,
            (step.a.values[7][last], step.a.values[8][last]),
            "affine port of the projective ladder"
        );
    }
    let t = plus(
        &scaled(&link(first + 2, 9, previous, F::<C>::ONE), F::<C>::from(4)),
        &plus(&own(0, n), &scaled(&own(1, n), F::<C>::from(2))),
    );
    step.a.rows.push([
        plus(&own(9, n), &scaled(&t, -one)),
        constant(vec![one; n]),
        vec![],
    ]);
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
    let b3 = curve_b3::<C>();
    let half = C::ScalarExt::from(2).invert().unwrap();
    let bases: Vec<_> = inputs
        .iter()
        .map(|(p, _, fixed)| if *fixed { *p * half } else { *p })
        .collect();
    let base = values(&bases, slots);
    let mut init = B::new("complete multiplication init", family, 0, slots, 4);
    let p = [0, 1].map(|k| init.source(link(first, k, vec![Some(0)], one), base[k].clone()));
    let out = init.add_point(p.clone(), p.clone());
    init.put(2, &out[0]);
    init.put(3, &out[1]);
    for k in 0..2 {
        init.equal(&init.own(k), &init.constant(F::<C>::ZERO));
    }
    // The base's identity flag; the start 2 P is the identity exactly when P
    // is, the group having odd order.
    let infinity = init.infinity(&p);
    let flag = init.a.values.len() - 1;
    let mut step = B::new("complete multiplication steps", family, rho, slots, 8);
    let mut previous_values = vec![vec![F::<C>::ZERO; slots * n]; 3];
    let mut raw_results = vec![];
    let mut scalars = vec![];
    for p in 0..slots {
        let point = bases.get(p).copied().unwrap_or(C::identity());
        let inf = infinity_native(coords(&point));
        let mut group = point.double();
        let (sx, sy) = coords(&group);
        let mut acc = [sx, sy + inf, one - inf];
        let mut t = F::<C>::ZERO;
        for r in 0..n {
            for (column, value) in previous_values.iter_mut().zip(acc) {
                column[p * n + r] = value;
            }
            let bit = r < bits && inputs.get(p).is_some_and(|(_, b, _)| b[bits - 1 - r]);
            let v = F::<C>::from(bit as u64);
            step.a.values[0][p * n + r] = v;
            let chosen = if bit { point } else { -point };
            let (cx, cy) = coords(&chosen);
            acc = add_native(double_native(acc, b3), [cx, cy + inf, one - inf], b3);
            group = group.double() + chosen;
            t = t.double() + v;
            step.a.values[7][p * n + r] = t;
            if r + 1 == bits && p < inputs.len() {
                raw_results.push(coords(&group));
                scalars.push(t);
            }
        }
    }
    let previous: Vec<_> = (0..n).map(|r| r.checked_sub(1)).collect();
    let acc = accumulator(&step, first, [2, 3], flag, MIXED.carry, previous_values);
    let point = std::array::from_fn::<_, 2, _>(|k| {
        step.source(
            link(first, k, vec![Some(0); n], one),
            base[k]
                .iter()
                .flat_map(|v| std::iter::repeat_n(*v, n))
                .collect(),
        )
    });
    let infinite = step.source(
        link(first + 1, flag, vec![Some(0); n], one),
        infinity
            .values
            .iter()
            .flat_map(|v| std::iter::repeat_n(*v, n))
            .collect(),
    );
    let bit = step.own(0);
    step.a.widths[0] = 1;
    step.a
        .rows
        .push([bit.lin.clone(), bit.lin.clone(), bit.lin]);
    // Cell 1, the chosen point's y: (2 bit - 1) y.
    let sign = step.own(0).scale(F::<C>::from(2)).sub(&step.constant(one));
    let y = step.product_into(1, &sign, &point[1]);
    let chosen = [point[0].clone(), y];
    double_and_add(&mut step, &acc, &chosen, &infinite, b3, &MIXED);
    for (p, result) in raw_results.iter().enumerate() {
        let last = p * n + bits - 1;
        assert_eq!(
            *result,
            (step.a.values[5][last], step.a.values[6][last]),
            "affine port of the projective ladder"
        );
    }
    let t = plus(
        &scaled(&link(first + 2, 7, previous, one), F::<C>::from(2)),
        &own(0, n),
    );
    step.a.rows.push([
        plus(&own(7, n), &scaled(&t, -one)),
        constant(vec![one; n]),
        vec![],
    ]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::endo_scalar;
    use crate::uniform::Src;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};

    /// A linear form at instance `i` of `arrays[a]`. Links name arrays from
    /// the ladder's `first` index and read the same ladder.
    fn value<F: PrimeField>(
        arrays: &[Array<F>],
        first: usize,
        a: usize,
        lin: &Lin<F>,
        i: usize,
    ) -> F {
        let arr = &arrays[a];
        let n = 1usize << arr.rho;
        lin.iter()
            .map(|t| {
                t.coef[i % n]
                    * match &t.src {
                        Src::One => F::ONE,
                        Src::Own(k) => arr.values[*k][i],
                        Src::Link { array, cell, map } => map[i % n].map_or(F::ZERO, |r| {
                            let target = &arrays[*array - first];
                            target.values[*cell][((i / n) << target.rho) + r]
                        }),
                        Src::All { .. } => unreachable!("no sum row in a ladder"),
                    }
            })
            .sum()
    }

    /// The first row that fails at a present instance of `count` ladders:
    /// its array, template row and instance.
    fn failing_row<F: PrimeField>(
        arrays: &[Array<F>],
        first: usize,
        count: usize,
    ) -> Option<(&'static str, usize, usize)> {
        for (a, arr) in arrays.iter().enumerate() {
            for (j, [ra, rb, rc]) in arr.rows.iter().enumerate() {
                for p in 0..count {
                    for r in 0..arr.r_count {
                        let i = (p << arr.rho) + r;
                        if value(arrays, first, a, ra, i) * value(arrays, first, a, rb, i)
                            != value(arrays, first, a, rc, i)
                        {
                            return Some((arr.name, j, i));
                        }
                    }
                }
            }
        }
        None
    }

    /// Three endoscalings and three multiplications in families of four
    /// slots, one base of each the identity and one multiplication
    /// fixed-base, with steps padded to sixteen. Every row holds, the ports
    /// equal group arithmetic, a step keeps its row count and a changed
    /// carried cell breaks a row.
    macro_rules! complete_ladders {
        ($name:ident, $curve:ty) => {
            #[test]
            fn $name() {
                type Fs = <$curve as PastaCurve>::ScalarExt;
                let point = |k: u64| <$curve>::generator() * Fs::from(k).square();
                let mut state = 0x9E37_79B9_7F4A_7C15_u64;
                let mut random = |n: usize| -> Vec<bool> {
                    (0..n)
                        .map(|_| {
                            state = state
                                .wrapping_mul(6_364_136_223_846_793_005)
                                .wrapping_add(1_442_695_040_888_963_407);
                            state >> 63 == 1
                        })
                        .collect()
                };
                let (steps, width) = (9, 12);
                let endo_in: Vec<($curve, Vec<bool>)> = vec![
                    (point(3), random(2 * steps)),
                    (<$curve>::identity(), random(2 * steps)),
                    (point(5), random(2 * steps)),
                ];
                let mixed_in: Vec<($curve, Vec<bool>, bool)> = vec![
                    (point(7), random(width), false),
                    (<$curve>::identity(), random(width), false),
                    (point(11), random(width), true),
                ];
                // Any first index: the links follow it.
                let mut e = endoscale::<$curve>(5, 0, 2, &endo_in);
                let m = mixed_mul::<$curve>(8, 1, 2, &mixed_in);
                assert_eq!(
                    (e.arrays[2].rows.len(), m.arrays[2].rows.len()),
                    (31, 29),
                    "rows of one endoscaling step and one multiplication step"
                );
                assert_eq!(failing_row(&e.arrays, 5, 3), None);
                assert_eq!(failing_row(&m.arrays, 8, 3), None);
                for (k, (pt, bits)) in endo_in.iter().enumerate() {
                    assert_eq!(e.results[k], coords(&(*pt * endo_scalar::<Fs>(bits))));
                }
                // A variable base leaves [2^width + 2t + 1] P for its caller's
                // correction; a fixed base is corrected here to [t] P.
                for (k, (pt, bits, fixed)) in mixed_in.iter().enumerate() {
                    let t = bits
                        .iter()
                        .rev()
                        .fold(Fs::ZERO, |t, b| t.double() + Fs::from(u64::from(*b)));
                    let scalar = if *fixed {
                        t
                    } else {
                        Fs::from(2).pow_vartime([width as u64]) + t.double() + Fs::ONE
                    };
                    assert_eq!(m.results[k], coords(&(*pt * scalar)));
                }
                // The first ladder's second step, a carried product.
                e.arrays[2].values[ENDO.carry[0]][1] += <$curve as PastaCurve>::Base::ONE;
                assert!(
                    failing_row(&e.arrays, 5, 3).is_some(),
                    "a changed carried cell breaks a row"
                );
            }
        };
    }
    complete_ladders!(pallas_ladders_hold_and_match_the_group, Pallas);
    complete_ladders!(vesta_ladders_hold_and_match_the_group, Vesta);
}
