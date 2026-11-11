//! Stage A2, step 2: the elliptic-curve ladders as uniform arrays.
//!
//! The verifier's ladders (Halo endoscaling, variable-base and fixed-base
//! multiplication) become step arrays over instances `(p, r)`: `p` is one
//! ladder, `r` its step. Every step owns its bits (booleanity in the step)
//! and extends a running recomposition sum, so a ladder hands one scalar to
//! the outside instead of every bit. The base point is read by a broadcast
//! link (every `r` reads the ladder's one base instance), and the accumulator
//! by a shift link (step `r` reads step `r - 1`). The point formulas are the
//! verifier gadgets' rows, unchanged.

use ff::{Field, PrimeField};
use group::{Curve, Group};
use iroha_pasta::{PastaAffine, PastaCurve};

use crate::uniform::{Array, Lin, constant, link, own, plus, scaled};

fn sub<F: PrimeField>(a: &Lin<F>, b: &Lin<F>) -> Lin<F> {
    plus(a, &scaled(b, -F::ONE))
}

fn plus3<F: PrimeField>(a: &Lin<F>, b: &Lin<F>, c: &Lin<F>) -> Lin<F> {
    plus(&plus(a, b), c)
}

fn cst<F: PrimeField>(v: F, n: usize) -> Lin<F> {
    constant(vec![v; n])
}

/// A point as two linear combinations.
#[derive(Clone)]
struct Pt<F> {
    x: Lin<F>,
    y: Lin<F>,
}

type Row<F> = [Lin<F>; 3];

fn r_bool<F: PrimeField>(b: usize, n: usize) -> Row<F> {
    [own(b, n), own(b, n), own(b, n)]
}

/// `lin = 0` (the gadgets' `enforce_zero` form).
fn r_zero<F: PrimeField>(lin: Lin<F>, n: usize) -> Row<F> {
    [lin, cst(F::ONE, n), Vec::new()]
}

/// Chord addition, inputs provably distinct (verifier `add_incomplete`).
fn r_add_incomplete<F: PrimeField>(
    p: &Pt<F>,
    q: &Pt<F>,
    [l, x3, y3]: [usize; 3],
    n: usize,
) -> Vec<Row<F>> {
    vec![
        [own(l, n), sub(&q.x, &p.x), sub(&q.y, &p.y)],
        [own(l, n), own(l, n), plus3(&own(x3, n), &p.x, &q.x)],
        [own(l, n), sub(&p.x, &own(x3, n)), plus(&own(y3, n), &p.y)],
    ]
}

/// Doubling (verifier `double`).
fn r_double<F: PrimeField>(p: &Pt<F>, [xx, l, x3, y3]: [usize; 4], n: usize) -> Vec<Row<F>> {
    vec![
        [p.x.clone(), p.x.clone(), own(xx, n)],
        [
            own(l, n),
            scaled(&p.y, F::from(2)),
            scaled(&own(xx, n), F::from(3)),
        ],
        [
            own(l, n),
            own(l, n),
            plus(&own(x3, n), &scaled(&p.x, F::from(2))),
        ],
        [own(l, n), sub(&p.x, &own(x3, n)), plus(&own(y3, n), &p.y)],
    ]
}

/// Complete addition checking both slope equations (verifier `add`).
fn r_add<F: PrimeField>(
    p: &Pt<F>,
    q: &Pt<F>,
    [l, sq, pq, x3, y3]: [usize; 5],
    n: usize,
) -> Vec<Row<F>> {
    let s = plus(&p.x, &q.x);
    vec![
        [own(l, n), sub(&q.x, &p.x), sub(&q.y, &p.y)],
        [s.clone(), s, own(sq, n)],
        [p.x.clone(), q.x.clone(), own(pq, n)],
        [own(l, n), plus(&p.y, &q.y), sub(&own(sq, n), &own(pq, n))],
        [own(l, n), own(l, n), plus3(&own(x3, n), &p.x, &q.x)],
        [own(l, n), sub(&p.x, &own(x3, n)), plus(&own(y3, n), &p.y)],
    ]
}

/// `2 acc + s`, inputs provably distinct (verifier `double_add`).
fn r_double_add<F: PrimeField>(
    acc: &Pt<F>,
    s: &Pt<F>,
    [l1, xr, l2, xo, yo]: [usize; 5],
    n: usize,
) -> Vec<Row<F>> {
    vec![
        [own(l1, n), sub(&s.x, &acc.x), sub(&s.y, &acc.y)],
        [own(l1, n), own(l1, n), plus3(&own(xr, n), &acc.x, &s.x)],
        [
            plus(&own(l1, n), &own(l2, n)),
            sub(&acc.x, &own(xr, n)),
            scaled(&acc.y, F::from(2)),
        ],
        [
            own(l2, n),
            own(l2, n),
            plus3(&own(xo, n), &own(xr, n), &acc.x),
        ],
        [
            own(l2, n),
            sub(&acc.x, &own(xo, n)),
            plus(&own(yo, n), &acc.y),
        ],
    ]
}

// Native counterparts (the witness), same formulas.

fn n_add_incomplete<F: PrimeField>((xp, yp): (F, F), (xq, yq): (F, F)) -> [F; 3] {
    let l = (yq - yp) * (xq - xp).invert().expect("distinct x");
    let x3 = l.square() - xp - xq;
    [l, x3, l * (xp - x3) - yp]
}

fn n_double<F: PrimeField>((x, y): (F, F)) -> [F; 4] {
    let xx = x.square();
    let l = xx * F::from(3) * y.double().invert().expect("y != 0");
    let x3 = l.square() - x.double();
    [xx, l, x3, l * (x - x3) - y]
}

pub(crate) fn n_add<F: PrimeField>((xp, yp): (F, F), (xq, yq): (F, F)) -> [F; 5] {
    let l = if xp != xq {
        (yq - yp) * (xq - xp).invert().unwrap()
    } else {
        assert!(yp == yq, "p = -q is unsatisfiable");
        xp.square() * F::from(3) * yp.double().invert().unwrap()
    };
    let s = xp + xq;
    let x3 = l.square() - xp - xq;
    [l, s.square(), xp * xq, x3, l * (xp - x3) - yp]
}

fn n_double_add<F: PrimeField>((xa, ya): (F, F), (xs, ys): (F, F)) -> [F; 5] {
    let l1 = (ys - ya) * (xs - xa).invert().expect("distinct x");
    let xr = l1.square() - xa - xs;
    let l2 = ya.double() * (xa - xr).invert().expect("distinct x") - l1;
    let xo = l2.square() - xr - xa;
    [l1, xr, l2, xo, l2 * (xa - xo) - ya]
}

pub fn coords<C: PastaCurve>(p: &C) -> (C::Base, C::Base) {
    Option::from(p.to_affine().coordinates()).expect("finite point")
}

pub fn beta<C: PastaCurve>() -> C::Base {
    let g = C::generator();
    coords(&g.endo()).0 * coords(&g).0.invert().unwrap()
}

fn bits_for(len: usize) -> usize {
    len.next_power_of_two().trailing_zeros() as usize
}

/// A ladder's arrays and every ladder's result point and recomposed scalar.
pub struct Ladder<F> {
    pub arrays: Vec<Array<F>>,
    pub results: Vec<(F, F)>,
    pub scalars: Vec<F>,
}

fn grid<F: PrimeField>(cells: usize, slots: usize) -> Vec<Vec<F>> {
    vec![vec![F::ZERO; slots]; cells]
}

/// Halo endoscaling `[endo(bits)] P` for each `(P, bits)` (same bit count,
/// pairs `(sign, endo)` from the top), as arrays `first..first + 3` of family
/// `fam` with `2^pi >= inputs` ladders:
///
/// - base (p): x, y;
/// - init (p): `phi(P) + P` (3 rows), then its doubling (4 rows);
/// - step (p, r): bits s and e, the selected point (2 rows), `2 acc + s`
///   (5 rows), booleanity (2 rows) and the running scalar
///   `t = 4 t' + s + 2 e` (1 row).
pub fn endoscale<C: PastaCurve>(
    first: usize,
    fam: usize,
    pi: usize,
    inputs: &[(C, Vec<bool>)],
) -> Ladder<C::Base> {
    let (base_a, init_a, step_a) = (first, first + 1, first + 2);
    let steps = inputs[0].1.len() / 2;
    let rho = bits_for(steps);
    let n = 1usize << rho;
    let beta = beta::<C>();
    let one = C::Base::ONE;
    // Witness.
    let mut base = grid(2, 1 << pi);
    let mut init = grid(7, 1 << pi);
    let mut step = grid(10, 1 << (pi + rho));
    let (mut results, mut scalars) = (Vec::new(), Vec::new());
    for (p, (point, bits)) in inputs.iter().enumerate() {
        assert_eq!(bits.len(), 2 * steps, "one bit count per array");
        let (x, y) = coords(point);
        base[0][p] = x;
        base[1][p] = y;
        let [l, x3, y3] = n_add_incomplete((beta * x, y), (x, y));
        let [xx, l2, x4, y4] = n_double((x3, y3));
        for (k, v) in [l, x3, y3, xx, l2, x4, y4].into_iter().enumerate() {
            init[k][p] = v;
        }
        let mut acc = (x4, y4);
        let mut t = C::Base::ZERO;
        let mut native = (point.endo() + *point).double();
        for r in 0..n {
            let (sv, ev) = if r < steps {
                (bits[2 * (steps - 1 - r)], bits[2 * (steps - 1 - r) + 1])
            } else {
                (false, false)
            };
            let xs = if ev { beta * x } else { x };
            let ys = if sv { y } else { -y };
            let [l1, xr, l2, xo, yo] = n_double_add(acc, (xs, ys));
            let sf = if sv { one } else { C::Base::ZERO };
            let ef = if ev { one } else { C::Base::ZERO };
            t = t * C::Base::from(4) + sf + ef.double();
            for (k, v) in [sf, ef, xs, ys, l1, xr, l2, xo, yo, t]
                .into_iter()
                .enumerate()
            {
                step[k][(p << rho) | r] = v;
            }
            acc = (xo, yo);
            if r < steps {
                let mut s = if sv { *point } else { -*point };
                if ev {
                    s = s.endo();
                }
                native = native.double() + s;
                if r + 1 == steps {
                    assert_eq!(
                        acc,
                        coords(&native),
                        "endoscale steps reproduce the group arithmetic"
                    );
                    results.push(acc);
                    scalars.push(t);
                }
            }
        }
    }
    // Templates.
    let all = |k: usize| -> Vec<Option<usize>> { vec![Some(k); n] };
    let base_pt = Pt {
        x: link(base_a, 0, vec![Some(0)], one),
        y: link(base_a, 1, vec![Some(0)], one),
    };
    let phi = Pt {
        x: scaled(&base_pt.x, beta),
        y: base_pt.y.clone(),
    };
    let mut init_rows = r_add_incomplete(&phi, &base_pt, [0, 1, 2], 1);
    init_rows.extend(r_double(
        &Pt {
            x: own(1, 1),
            y: own(2, 1),
        },
        [3, 4, 5, 6],
        1,
    ));
    let prev: Vec<Option<usize>> = (0..n)
        .map(|r| if r == 0 { None } else { Some(r - 1) })
        .collect();
    let at0: Vec<Option<usize>> = (0..n)
        .map(|r| if r == 0 { Some(0) } else { None })
        .collect();
    let acc = Pt {
        x: plus(
            &link(init_a, 5, at0.clone(), one),
            &link(step_a, 7, prev.clone(), one),
        ),
        y: plus(
            &link(init_a, 6, at0, one),
            &link(step_a, 8, prev.clone(), one),
        ),
    };
    let bx = link(base_a, 0, all(0), one);
    let by = link(base_a, 1, all(0), one);
    let (sv, ev, xs, ys) = (0, 1, 2, 3);
    let mut step_rows = vec![r_bool(sv, n), r_bool(ev, n)];
    step_rows.push([own(ev, n), scaled(&bx, beta - one), sub(&own(xs, n), &bx)]);
    step_rows.push([
        sub(&scaled(&own(sv, n), C::Base::from(2)), &cst(one, n)),
        by,
        own(ys, n),
    ]);
    step_rows.extend(r_double_add(
        &acc,
        &Pt {
            x: own(xs, n),
            y: own(ys, n),
        },
        [4, 5, 6, 7, 8],
        n,
    ));
    let recompose = sub(
        &sub(
            &sub(
                &own(9, n),
                &scaled(&link(step_a, 9, prev, one), C::Base::from(4)),
            ),
            &own(sv, n),
        ),
        &scaled(&own(ev, n), C::Base::from(2)),
    );
    step_rows.push(r_zero(recompose, n));
    let arrays = vec![
        Array {
            name: "endo base",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 2],
            rows: Vec::new(),
            values: base,
        },
        Array {
            name: "endo init",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 7],
            rows: init_rows,
            values: init,
        },
        Array {
            name: "endo step",
            family: fam,
            rho,
            r_count: n,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1, 1, 255, 255, 255, 255, 255, 255, 255, 255],
            rows: step_rows,
            values: step,
        },
    ];
    Ladder {
        arrays,
        results,
        scalars,
    }
}

/// `[2c + 2^m + 1] P` from `c`'s `m` bits (verifier `var_mul`): `acc = 2P`, then
/// `acc = 2 acc + (2 c_i - 1) P` from the top bit, the last three with a
/// complete doubling and addition. Arrays `first..first + 4`:
///
/// - base (p);
/// - init (p): `2P` (4 rows);
/// - step (p, r), r < m - 3: bit, select, `2 acc + s` (5 rows), booleanity,
///   running scalar `t = 2 t' + c` (8 rows);
/// - tail (p, r), r < 3: bit, select, doubling (4 rows), complete addition
///   (6 rows), booleanity, running scalar (13 rows).
pub fn var_mul<C: PastaCurve>(
    first: usize,
    fam: usize,
    pi: usize,
    inputs: &[(C, Vec<bool>)],
) -> Ladder<C::Base> {
    let (base_a, init_a, step_a, tail_a) = (first, first + 1, first + 2, first + 3);
    let m = inputs[0].1.len();
    let steps = m - 3;
    let rho = bits_for(steps);
    let n = 1usize << rho;
    let one = C::Base::ONE;
    let two = C::Base::from(2);
    let mut base = grid(2, 1 << pi);
    let mut init = grid(4, 1 << pi);
    let mut step = grid(8, 1 << (pi + rho));
    let mut tail = grid(12, 1 << (pi + 2));
    let (mut results, mut scalars) = (Vec::new(), Vec::new());
    for (p, (point, bits)) in inputs.iter().enumerate() {
        assert_eq!(bits.len(), m, "one bit count per array");
        let (x, y) = coords(point);
        base[0][p] = x;
        base[1][p] = y;
        let d = n_double((x, y));
        for (k, v) in d.into_iter().enumerate() {
            init[k][p] = v;
        }
        let mut acc = (d[2], d[3]);
        let mut t = C::Base::ZERO;
        let mut native = point.double();
        let mut dummy = (C::Base::ZERO, C::Base::ZERO, C::Base::ZERO);
        for (r, i) in (0..m).rev().enumerate() {
            let c = bits[i];
            let cf = if c { one } else { C::Base::ZERO };
            let ys = if c { y } else { -y };
            t = t.double() + cf;
            native = native.double() + if c { *point } else { -*point };
            if r < steps {
                let [l1, xr, l2, xo, yo] = n_double_add(acc, (x, ys));
                for (k, v) in [cf, ys, l1, xr, l2, xo, yo, t].into_iter().enumerate() {
                    step[k][(p << rho) | r] = v;
                }
                acc = (xo, yo);
                if r + 1 == steps {
                    // Dummy steps (bit 0) fill the step range after the last real step.
                    let (mut da, mut dt) = (acc, t);
                    for rr in steps..n {
                        let [l1, xr, l2, xo, yo] = n_double_add(da, (x, -y));
                        dt = dt.double();
                        for (k, v) in [C::Base::ZERO, -y, l1, xr, l2, xo, yo, dt]
                            .into_iter()
                            .enumerate()
                        {
                            step[k][(p << rho) | rr] = v;
                        }
                        da = (xo, yo);
                    }
                    dummy = (da.0, da.1, dt);
                }
            } else {
                let [xx, l, x3, y3] = n_double(acc);
                let [l2, sq, pq, x4, y4] = n_add((x3, y3), (x, ys));
                for (k, v) in [cf, ys, xx, l, x3, y3, l2, sq, pq, x4, y4, t]
                    .into_iter()
                    .enumerate()
                {
                    tail[k][(p << 2) | (r - steps)] = v;
                }
                acc = (x4, y4);
            }
        }
        let _ = dummy;
        assert_eq!(
            acc,
            coords(&native),
            "var_mul steps reproduce the group arithmetic"
        );
        {
            let [xx, l, x3, y3] = n_double(acc);
            let [l2, sq, pq, x4, y4] = n_add((x3, y3), (x, -y));
            let td = t.double();
            for (k, v) in [C::Base::ZERO, -y, xx, l, x3, y3, l2, sq, pq, x4, y4, td]
                .into_iter()
                .enumerate()
            {
                tail[k][(p << 2) | 3] = v;
            }
        }
        results.push(acc);
        scalars.push(t);
    }
    let base_pt = |nn: usize| Pt {
        x: link(base_a, 0, vec![Some(0); nn], one),
        y: link(base_a, 1, vec![Some(0); nn], one),
    };
    let init_rows = r_double(&base_pt(1), [0, 1, 2, 3], 1);
    let prev: Vec<Option<usize>> = (0..n)
        .map(|r| if r == 0 { None } else { Some(r - 1) })
        .collect();
    let at0: Vec<Option<usize>> = (0..n)
        .map(|r| if r == 0 { Some(0) } else { None })
        .collect();
    let acc = Pt {
        x: plus(
            &link(init_a, 2, at0.clone(), one),
            &link(step_a, 5, prev.clone(), one),
        ),
        y: plus(
            &link(init_a, 3, at0, one),
            &link(step_a, 6, prev.clone(), one),
        ),
    };
    let b = base_pt(n);
    let mut step_rows = vec![r_bool(0, n)];
    step_rows.push([
        sub(&scaled(&own(0, n), two), &cst(one, n)),
        b.y.clone(),
        own(1, n),
    ]);
    step_rows.extend(r_double_add(
        &acc,
        &Pt {
            x: b.x.clone(),
            y: own(1, n),
        },
        [2, 3, 4, 5, 6],
        n,
    ));
    step_rows.push(r_zero(
        sub(
            &sub(&own(7, n), &scaled(&link(step_a, 7, prev, one), two)),
            &own(0, n),
        ),
        n,
    ));
    // Tail: r = 0 reads the last step, r = 1, 2 read the previous tail.
    let last = steps - 1;
    let from_step: Vec<Option<usize>> = (0..4)
        .map(|r| if r == 0 { Some(last) } else { None })
        .collect();
    let from_tail: Vec<Option<usize>> = (0..4)
        .map(|r| if r == 0 { None } else { Some(r - 1) })
        .collect();
    let tacc = Pt {
        x: plus(
            &link(step_a, 5, from_step.clone(), one),
            &link(tail_a, 9, from_tail.clone(), one),
        ),
        y: plus(
            &link(step_a, 6, from_step.clone(), one),
            &link(tail_a, 10, from_tail.clone(), one),
        ),
    };
    let tb = base_pt(4);
    let mut tail_rows = vec![r_bool(0, 4)];
    tail_rows.push([
        sub(&scaled(&own(0, 4), two), &cst(one, 4)),
        tb.y.clone(),
        own(1, 4),
    ]);
    tail_rows.extend(r_double(&tacc, [2, 3, 4, 5], 4));
    tail_rows.extend(r_add(
        &Pt {
            x: own(4, 4),
            y: own(5, 4),
        },
        &Pt {
            x: tb.x.clone(),
            y: own(1, 4),
        },
        [6, 7, 8, 9, 10],
        4,
    ));
    let t_prev = plus(
        &link(step_a, 7, from_step, one),
        &link(tail_a, 11, from_tail, one),
    );
    tail_rows.push(r_zero(
        sub(&sub(&own(11, 4), &scaled(&t_prev, two)), &own(0, 4)),
        4,
    ));
    let arrays = vec![
        Array {
            name: "var base",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 2],
            rows: Vec::new(),
            values: base,
        },
        Array {
            name: "var init",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 4],
            rows: init_rows,
            values: init,
        },
        Array {
            name: "var step",
            family: fam,
            rho,
            r_count: n,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1, 255, 255, 255, 255, 255, 255, 255],
            rows: step_rows,
            values: step,
        },
        Array {
            name: "var tail",
            family: fam,
            rho: 2,
            r_count: 4,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
            rows: tail_rows,
            values: tail,
        },
    ];
    Ladder {
        arrays,
        results,
        scalars,
    }
}

/// Variable- and fixed-base multiplications in one family, sharing
/// `var_mul`'s templates. Each input is `(point, bits, fixed)`:
///
/// - a variable base `P` gives `[2c + 2^m + 1] P` at the tail, as `var_mul`;
/// - a fixed base `B` gives `[k] B` at the end: the ladder runs over
///   `B' = [1/2] B` and the end adds `Q = -[(2^m + 1)/2] B`.
///
/// Arrays `first..first + 9`: base, init, step, tail (witness, as `var_mul`);
/// the constant columns `B'` and a selector; a check row
/// `(base - B') * selector = 0` (2 rows), so a fixed instance's base is its
/// constant and a variable one's is free; the constant `Q` (any point for a
/// variable instance, whose end the consumers ignore); and the end (6 rows).
pub fn mixed_mul<C: PastaCurve>(
    first: usize,
    fam: usize,
    pi: usize,
    inputs: &[(C, Vec<bool>, bool)],
) -> Ladder<C::Base> {
    let half = C::ScalarExt::from(2).invert().unwrap();
    let m = inputs[0].1.len();
    let two_m1 = (0..m).fold(C::ScalarExt::ONE, |acc, _| acc.double()) + C::ScalarExt::ONE;
    let bases: Vec<(C, Vec<bool>)> = inputs
        .iter()
        .map(|(b, bits, fixed)| (if *fixed { *b * half } else { *b }, bits.clone()))
        .collect();
    let mut ladder = var_mul::<C>(first, fam, pi, &bases);
    let (base_a, tail_a, fixb_a, sel_a, corr_a) =
        (first, first + 3, first + 4, first + 5, first + 7);
    let one = C::Base::ONE;
    let mut fixb = grid(2, 1 << pi);
    let mut sel = grid(1, 1 << pi);
    let mut corr = grid(2, 1 << pi);
    let mut end = grid(5, 1 << pi);
    let mut results = Vec::new();
    for (p, (b, bits, fixed)) in inputs.iter().enumerate() {
        let q = if *fixed {
            coords(&(-(*b * (two_m1 * half))))
        } else {
            coords(&C::generator())
        };
        if *fixed {
            let bp = coords(&(*b * half));
            fixb[0][p] = bp.0;
            fixb[1][p] = bp.1;
            sel[0][p] = one;
        }
        corr[0][p] = q.0;
        corr[1][p] = q.1;
        let vals = n_add(ladder.results[p], q);
        for (k, v) in vals.into_iter().enumerate() {
            end[k][p] = v;
        }
        if *fixed {
            let k = bits.iter().rev().fold(C::ScalarExt::ZERO, |acc, bit| {
                acc.double()
                    + if *bit {
                        C::ScalarExt::ONE
                    } else {
                        C::ScalarExt::ZERO
                    }
            });
            assert_eq!(
                (vals[3], vals[4]),
                coords(&(*b * k)),
                "fixed-base ladder reproduces [k] B"
            );
            results.push((vals[3], vals[4]));
        } else {
            results.push(ladder.results[p]);
        }
    }
    let at = |a: usize, k: usize| link(a, k, vec![Some(0)], one);
    let check_rows = vec![
        [
            sub(&at(base_a, 0), &at(fixb_a, 0)),
            at(sel_a, 0),
            Vec::new(),
        ],
        [
            sub(&at(base_a, 1), &at(fixb_a, 1)),
            at(sel_a, 0),
            Vec::new(),
        ],
    ];
    let acc = Pt {
        x: link(tail_a, 9, vec![Some(2)], one),
        y: link(tail_a, 10, vec![Some(2)], one),
    };
    let q = Pt {
        x: at(corr_a, 0),
        y: at(corr_a, 1),
    };
    let end_rows = r_add(&acc, &q, [0, 1, 2, 3, 4], 1);
    let mk = |name: &'static str,
              fixed: bool,
              widths: Vec<u16>,
              rows: Vec<Row<C::Base>>,
              values: Vec<Vec<C::Base>>| Array {
        name,
        family: fam,
        rho: 0,
        r_count: 1,
        fixed,
        lookups: Vec::new(),
        widths,
        rows,
        values,
    };
    ladder
        .arrays
        .push(mk("mul base const", true, vec![255; 2], Vec::new(), fixb));
    ladder
        .arrays
        .push(mk("mul selector", true, vec![1], Vec::new(), sel));
    ladder.arrays.push(mk(
        "mul base check",
        false,
        Vec::new(),
        check_rows,
        Vec::new(),
    ));
    ladder
        .arrays
        .push(mk("mul corr", true, vec![255; 2], Vec::new(), corr));
    ladder
        .arrays
        .push(mk("mul end", false, vec![255; 5], end_rows, end));
    ladder.results = results;
    ladder
}

/// `[k] B` for fixed bases `B` (one per ladder) from `k`'s `m` bits, as a
/// variable-base ladder over the constant column `B' = [1/2] B`:
/// `var_mul` gives `[2k + 2^m + 1] B' = [k] B + [(2^m + 1)/2] B`, and an end
/// step adds the constant `Q = -[(2^m + 1)/2] B` with a complete addition.
/// The bases and corrections are constant columns, so no window table enters
/// the matrices. Arrays `first..first + 6`: the four of `var_mul` (its base a
/// constant column), the correction (constant), and the end (6 rows).
pub fn fixed_base_mul<C: PastaCurve>(
    first: usize,
    fam: usize,
    pi: usize,
    inputs: &[(C, Vec<bool>)],
) -> Ladder<C::Base> {
    let half = C::ScalarExt::from(2).invert().unwrap();
    let m = inputs[0].1.len();
    let two_m1 = (0..m).fold(C::ScalarExt::ONE, |acc, _| acc.double()) + C::ScalarExt::ONE;
    let primed: Vec<(C, Vec<bool>)> = inputs
        .iter()
        .map(|(b, bits)| (*b * half, bits.clone()))
        .collect();
    let mut ladder = var_mul::<C>(first, fam, pi, &primed);
    ladder.arrays[0].fixed = true;
    ladder.arrays[0].name = "fixed base";
    let (corr_a, tail_a) = (first + 4, first + 3);
    let one = C::Base::ONE;
    let mut corr = grid(2, 1 << pi);
    let mut end = grid(5, 1 << pi);
    let mut results = Vec::new();
    for (p, (b, bits)) in inputs.iter().enumerate() {
        let q = coords(&(-(*b * (two_m1 * half))));
        corr[0][p] = q.0;
        corr[1][p] = q.1;
        let vals = n_add(ladder.results[p], q);
        for (k, v) in vals.into_iter().enumerate() {
            end[k][p] = v;
        }
        let k = bits.iter().rev().fold(C::ScalarExt::ZERO, |acc, bit| {
            acc.double()
                + if *bit {
                    C::ScalarExt::ONE
                } else {
                    C::ScalarExt::ZERO
                }
        });
        assert_eq!(
            (vals[3], vals[4]),
            coords(&(*b * k)),
            "fixed-base ladder reproduces [k] B"
        );
        results.push((vals[3], vals[4]));
    }
    let acc = Pt {
        x: link(tail_a, 9, vec![Some(2)], one),
        y: link(tail_a, 10, vec![Some(2)], one),
    };
    let q = Pt {
        x: link(corr_a, 0, vec![Some(0)], one),
        y: link(corr_a, 1, vec![Some(0)], one),
    };
    let end_rows = r_add(&acc, &q, [0, 1, 2, 3, 4], 1);
    ladder.arrays.push(Array {
        name: "fixed corr",
        family: fam,
        rho: 0,
        r_count: 1,
        fixed: true,
        lookups: Vec::new(),
        widths: vec![255; 2],
        rows: Vec::new(),
        values: corr,
    });
    ladder.arrays.push(Array {
        name: "fixed end",
        family: fam,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 5],
        rows: end_rows,
        values: end,
    });
    ladder.results = results;
    ladder
}

/// `[k] B` for a fixed base `B` (verifier `fixed_mul`): 2-bit windows over
/// `(d + 4) 4^j B`, chord additions in the middle, complete additions for the
/// last two windows and the offset removal. The window tables are per-r
/// coefficients, so a family holds one base. Arrays `first..first + 4`:
///
/// - first (p): window 0: bits, their product, booleanity, scalar (4 rows);
/// - step (p, r): window r + 1: bits, product, chord addition (3 rows),
///   booleanity, scalar `t = t' + 4^j (b0 + 2 b1)` (7 rows);
/// - tail (p, r), r < 2: the last two windows with complete additions
///   (10 rows);
/// - end (p): adds the constant `-offset` (6 rows).
pub fn fixed_mul<C: PastaCurve>(
    first: usize,
    fam: usize,
    pi: usize,
    base_point: &C,
    bits: &[Vec<bool>],
) -> Ladder<C::Base> {
    let (first_a, step_a, tail_a, end_a) = (first, first + 1, first + 2, first + 3);
    let nbits = bits[0].len();
    let windows = nbits.div_ceil(2);
    let steps = windows - 3;
    let rho = bits_for(steps);
    let n = 1usize << rho;
    let one = C::Base::ONE;
    // Window tables and the offset, as the gadget builds them.
    let mut tables: Vec<[(C::Base, C::Base); 4]> = Vec::new();
    let mut weights: Vec<C::Base> = Vec::new();
    let mut offset = C::identity();
    let mut stepb = *base_point;
    let mut w = one;
    for j in 0..windows.max(n + 1) {
        tables.push(std::array::from_fn(|d| {
            coords(&(stepb * C::ScalarExt::from(d as u64 + 4)))
        }));
        if j < windows {
            offset += stepb * C::ScalarExt::from(4);
        }
        stepb = stepb.double().double();
        weights.push(w);
        w = w * C::Base::from(4);
    }
    let neg_offset = coords(&(-offset));
    let mut first_v = grid(4, 1 << pi);
    let mut step = grid(9, 1 << (pi + rho));
    let mut tail = grid(9, 1 << (pi + 1));
    let mut end = grid(5, 1 << pi);
    let (mut results, mut scalars) = (Vec::new(), Vec::new());
    let pick =
        |t: &[(C::Base, C::Base); 4], b0: bool, b1: bool| t[usize::from(b0) + 2 * usize::from(b1)];
    for (p, kb) in bits.iter().enumerate() {
        assert_eq!(kb.len(), nbits, "one bit count per array");
        let bit = |i: usize| kb.get(i).copied().unwrap_or(false);
        let f = |b: bool| if b { one } else { C::Base::ZERO };
        let (b0, b1) = (bit(0), bit(1));
        let mut t = f(b0) + f(b1).double();
        for (k, v) in [f(b0), f(b1), f(b0 && b1), t].into_iter().enumerate() {
            first_v[k][p] = v;
        }
        let mut acc = pick(&tables[0], b0, b1);
        for j in 1..windows {
            let (b0, b1) = (bit(2 * j), bit(2 * j + 1));
            let sel = pick(&tables[j], b0, b1);
            t += weights[j] * (f(b0) + f(b1).double());
            if j + 2 < windows {
                let [l, x3, y3] = n_add_incomplete(acc, sel);
                for (k, v) in [f(b0), f(b1), f(b0 && b1), sel.0, sel.1, l, x3, y3, t]
                    .into_iter()
                    .enumerate()
                {
                    step[k][(p << rho) | (j - 1)] = v;
                }
                acc = (x3, y3);
                if j + 3 == windows {
                    // Dummy windows (bits 0) fill the step range.
                    let mut da = acc;
                    for r in steps..n {
                        let sel = tables[r + 1][0];
                        let [l, x3, y3] = n_add_incomplete(da, sel);
                        for (k, v) in [
                            C::Base::ZERO,
                            C::Base::ZERO,
                            C::Base::ZERO,
                            sel.0,
                            sel.1,
                            l,
                            x3,
                            y3,
                            t,
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            step[k][(p << rho) | r] = v;
                        }
                        da = (x3, y3);
                    }
                }
            } else {
                let [l, sq, pq, x3, y3] = n_add(acc, sel);
                for (k, v) in [f(b0), f(b1), f(b0 && b1), l, sq, pq, x3, y3, t]
                    .into_iter()
                    .enumerate()
                {
                    tail[k][(p << 1) | (j + 2 - windows)] = v;
                }
                acc = (x3, y3);
            }
        }
        let [l, sq, pq, x3, y3] = n_add(acc, neg_offset);
        for (k, v) in [l, sq, pq, x3, y3].into_iter().enumerate() {
            end[k][p] = v;
        }
        let k_scalar = kb.iter().rev().fold(C::ScalarExt::ZERO, |acc, b| {
            acc.double()
                + if *b {
                    C::ScalarExt::ONE
                } else {
                    C::ScalarExt::ZERO
                }
        });
        assert_eq!(
            (x3, y3),
            coords(&(*base_point * k_scalar)),
            "fixed_mul windows reproduce the group arithmetic"
        );
        results.push((x3, y3));
        scalars.push(t);
    }
    // The selected point of window j from (b0, b1, both), as per-r constants.
    let select = |b0: Lin<C::Base>,
                  b1: Lin<C::Base>,
                  both: Lin<C::Base>,
                  win: &dyn Fn(usize) -> Option<usize>,
                  len: usize|
     -> Pt<C::Base> {
        let coef = |f: &dyn Fn(&[(C::Base, C::Base); 4]) -> C::Base| -> Vec<C::Base> {
            (0..len)
                .map(|r| win(r).map_or(C::Base::ZERO, |j| f(&tables[j])))
                .collect()
        };
        let axis = |pick_xy: &dyn Fn((C::Base, C::Base)) -> C::Base| -> Lin<C::Base> {
            let c0 = constant(coef(&|t| pick_xy(t[0])));
            let c1 = coef(&|t| pick_xy(t[1]) - pick_xy(t[0]));
            let c2 = coef(&|t| pick_xy(t[2]) - pick_xy(t[0]));
            let c3 = coef(&|t| pick_xy(t[3]) - pick_xy(t[2]) - pick_xy(t[1]) + pick_xy(t[0]));
            let times = |lin: &Lin<C::Base>, c: &[C::Base]| -> Lin<C::Base> {
                lin.iter()
                    .map(|term| crate::uniform::Term {
                        src: term.src.clone(),
                        coef: term.coef.iter().zip(c).map(|(a, b)| *a * b).collect(),
                    })
                    .collect()
            };
            plus(
                &plus(&plus(&c0, &times(&b0, &c1)), &times(&b1, &c2)),
                &times(&both, &c3),
            )
        };
        Pt {
            x: axis(&|p| p.0),
            y: axis(&|p| p.1),
        }
    };
    // First window: the selection is the accumulator.
    let first_rows = vec![
        r_bool(0, 1),
        r_bool(1, 1),
        [own(0, 1), own(1, 1), own(2, 1)],
        r_zero(
            sub(
                &sub(&own(3, 1), &own(0, 1)),
                &scaled(&own(1, 1), C::Base::from(2)),
            ),
            1,
        ),
    ];
    // Middle windows j = r + 1: acc from the first window (r = 0) or the
    // previous step.
    let prev: Vec<Option<usize>> = (0..n)
        .map(|r| if r == 0 { None } else { Some(r - 1) })
        .collect();
    let at0: Vec<Option<usize>> = (0..n)
        .map(|r| if r == 0 { Some(0) } else { None })
        .collect();
    let first_pt_at0 = select(
        link(first_a, 0, at0.clone(), one),
        link(first_a, 1, at0.clone(), one),
        link(first_a, 2, at0.clone(), one),
        &|r| if r == 0 { Some(0) } else { None },
        n,
    );
    let acc = Pt {
        x: plus(&first_pt_at0.x, &link(step_a, 6, prev.clone(), one)),
        y: plus(&first_pt_at0.y, &link(step_a, 7, prev.clone(), one)),
    };
    let sel = select(own(0, n), own(1, n), own(2, n), &|r| Some(r + 1), n);
    let mut step_rows = vec![
        r_bool(0, n),
        r_bool(1, n),
        [own(0, n), own(1, n), own(2, n)],
    ];
    step_rows.push(r_zero(sub(&own(3, n), &sel.x), n));
    step_rows.push(r_zero(sub(&own(4, n), &sel.y), n));
    step_rows.extend(r_add_incomplete(
        &acc,
        &Pt {
            x: own(3, n),
            y: own(4, n),
        },
        [5, 6, 7],
        n,
    ));
    let weight: Vec<C::Base> = (0..n).map(|r| weights[r + 1]).collect();
    let windowed = |k: usize, factor: C::Base| crate::uniform::Term {
        src: crate::uniform::Src::Own(k),
        coef: weight.iter().map(|w| *w * factor).collect(),
    };
    let t_prev = plus(&link(first_a, 3, at0, one), &link(step_a, 8, prev, one));
    let mut rec = sub(&own(8, n), &t_prev);
    rec = plus(
        &rec,
        &vec![windowed(0, -one), windowed(1, -C::Base::from(2))],
    );
    step_rows.push(r_zero(rec, n));
    // Tail windows j = windows - 2 + r with complete additions.
    let from_step: Vec<Option<usize>> = (0..2)
        .map(|r| if r == 0 { Some(steps - 1) } else { None })
        .collect();
    let from_tail: Vec<Option<usize>> = (0..2)
        .map(|r| if r == 0 { None } else { Some(r - 1) })
        .collect();
    let tacc = Pt {
        x: plus(
            &link(step_a, 6, from_step.clone(), one),
            &link(tail_a, 6, from_tail.clone(), one),
        ),
        y: plus(
            &link(step_a, 7, from_step.clone(), one),
            &link(tail_a, 7, from_tail.clone(), one),
        ),
    };
    let tsel = select(
        own(0, 2),
        own(1, 2),
        own(2, 2),
        &|r| Some(windows - 2 + r),
        2,
    );
    let mut tail_rows = vec![
        r_bool(0, 2),
        r_bool(1, 2),
        [own(0, 2), own(1, 2), own(2, 2)],
    ];
    tail_rows.extend(r_add(&tacc, &tsel, [3, 4, 5, 6, 7], 2));
    let tweight: Vec<C::Base> = (0..2).map(|r| weights[windows - 2 + r]).collect();
    let twin = |k: usize, factor: C::Base| crate::uniform::Term {
        src: crate::uniform::Src::Own(k),
        coef: tweight.iter().map(|w| *w * factor).collect(),
    };
    let tt_prev = plus(
        &link(step_a, 8, from_step, one),
        &link(tail_a, 8, from_tail, one),
    );
    let mut trec = sub(&own(8, 2), &tt_prev);
    trec = plus(&trec, &vec![twin(0, -one), twin(1, -C::Base::from(2))]);
    tail_rows.push(r_zero(trec, 2));
    // End: acc + (-offset).
    let eacc = Pt {
        x: link(tail_a, 6, vec![Some(1)], one),
        y: link(tail_a, 7, vec![Some(1)], one),
    };
    let off = Pt {
        x: cst(neg_offset.0, 1),
        y: cst(neg_offset.1, 1),
    };
    let end_rows = r_add(&eacc, &off, [0, 1, 2, 3, 4], 1);
    let arrays = vec![
        Array {
            name: "fixed first",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1, 1, 1, 255],
            rows: first_rows,
            values: first_v,
        },
        Array {
            name: "fixed step",
            family: fam,
            rho,
            r_count: n,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1, 1, 1, 255, 255, 255, 255, 255, 255],
            rows: step_rows,
            values: step,
        },
        Array {
            name: "fixed tail",
            family: fam,
            rho: 1,
            r_count: 2,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![1, 1, 1, 255, 255, 255, 255, 255, 255],
            rows: tail_rows,
            values: tail,
        },
        Array {
            name: "fixed end",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 5],
            rows: end_rows,
            values: end,
        },
    ];
    Ladder {
        arrays,
        results,
        scalars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::{SpikeRng, endo_scalar};
    use crate::grid::eq_table;
    use crate::r1cs::{Circuit, Witness};
    use crate::uniform::UniformCircuit;
    use iroha_pasta::Ep as Pallas;

    type Fb = <Pallas as PastaCurve>::Base;
    type Fs = <Pallas as PastaCurve>::ScalarExt;

    fn random_bits(rng: &mut SpikeRng, n: usize) -> Vec<bool> {
        (0..n).map(|_| rng.below::<Fb>(1) == Fb::ONE).collect()
    }

    /// Three endoscalings (family of 4 slots, one absent), two variable-base
    /// and two fixed-base multiplications, with mask gates.
    fn circuit(
        endo_steps: usize,
        mul_bits: usize,
    ) -> (
        UniformCircuit<Fb>,
        crate::r1cs::Instance<Fb>,
        Witness<Fb>,
        Vec<Ladder<Fb>>,
    ) {
        let mut rng = SpikeRng::new(b"ladders");
        let point = |rng: &mut SpikeRng| Pallas::generator() * rng.scalar::<Fs>();
        let endo_in: Vec<(Pallas, Vec<bool>)> = (0..3)
            .map(|_| (point(&mut rng), random_bits(&mut rng, 2 * endo_steps)))
            .collect();
        let var_in: Vec<(Pallas, Vec<bool>)> = (0..2)
            .map(|_| (point(&mut rng), random_bits(&mut rng, mul_bits)))
            .collect();
        let (u, h) = (point(&mut rng), point(&mut rng));
        let fixed_in: Vec<(Pallas, Vec<bool>)> = vec![
            (u, random_bits(&mut rng, mul_bits)),
            (u, random_bits(&mut rng, mul_bits)),
            (h, random_bits(&mut rng, mul_bits)),
        ];
        let e = endoscale::<Pallas>(0, 0, 2, &endo_in);
        let v = var_mul::<Pallas>(3, 1, 1, &var_in);
        let f = fixed_base_mul::<Pallas>(7, 2, 2, &fixed_in);
        let mixed_in: Vec<(Pallas, Vec<bool>, bool)> = var_in
            .iter()
            .map(|(p, b)| (*p, b.clone(), false))
            .chain(fixed_in.iter().map(|(p, b)| (*p, b.clone(), true)))
            .collect();
        let x = mixed_mul::<Pallas>(13, 4, 3, &mixed_in);
        for (k, (pt, bits)) in endo_in.iter().enumerate() {
            assert_eq!(e.results[k], coords(&(*pt * endo_scalar::<Fs>(bits))));
        }
        let mut arrays = Vec::new();
        for l in [&e, &v, &f, &x] {
            arrays.extend(l.arrays.iter().map(|a| Array {
                name: a.name,
                family: a.family,
                rho: a.rho,
                r_count: a.r_count,
                fixed: a.fixed,
                lookups: a.lookups.clone(),
                widths: a.widths.clone(),
                rows: a.rows.clone(),
                values: a.values.clone(),
            }));
        }
        let mask_a: Vec<Fb> = vec![rng.scalar(), rng.scalar()];
        let mask_b: Vec<Fb> = vec![rng.scalar(), rng.scalar()];
        let mask_c: Vec<Fb> = mask_a.iter().zip(&mask_b).map(|(x, y)| *x * y).collect();
        arrays.push(Array {
            name: "mask",
            family: 3,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 3],
            rows: vec![[own(0, 1), own(1, 1), own(2, 1)]],
            values: vec![mask_a, mask_b, mask_c],
        });
        let (c, inst, wit) = UniformCircuit::build(
            "ladders",
            arrays,
            vec![2, 1, 2, 1, 3],
            vec![3, 2, 3, 2, 5],
            Vec::new(),
            12,
            4,
            b"ladders",
        );
        (c, inst, wit, vec![e, v, f, x])
    }

    #[test]
    fn ladders_match_group_arithmetic_and_their_expansion() {
        let (c, inst, wit, _) = circuit(9, 12);
        let shape = c.shape;
        let rel = c.instantiate(&shape, &[], None);
        let z = crate::r1cs::z_vector(&shape, &inst, &wit);
        let (az, bz, cz) = rel.abc(&shape, &z);
        for x in 0..1 << shape.sx {
            assert_eq!(az[x] * bz[x], cz[x], "row {x} holds");
        }
        // A fixed instance whose base cell leaves its constant breaks its check
        // row (mixed family: arrays 13.., instance 2 is the first fixed base).
        let mut bad = z.clone();
        bad[c.cell_index(13, 0, 2)] += Fb::ONE;
        let (ba, bb, bc) = rel.abc(&shape, &bad);
        let check = c.row_index(19, 0, 2);
        assert_ne!(
            ba[check] * bb[check],
            bc[check],
            "the selector pins a fixed base"
        );
        let entries = c.entries(&[]);
        let mut rng = SpikeRng::new(b"ladder points");
        let rx: Vec<Fb> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fb> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (eq_table(&rx), eq_table(&ry));
        let mut evals = [Fb::ZERO; 3];
        let mut m = vec![Fb::ZERO; 1 << shape.sz()];
        let (ra, rb, rc) = (rng.scalar::<Fb>(), rng.scalar::<Fb>(), rng.scalar::<Fb>());
        for &(mat, x, y, v) in &entries {
            evals[mat] += v * ex[x] * ey[y];
            m[y] += ex[x] * v * [ra, rb, rc][mat];
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (evals[0], evals[1], evals[2])
        );
        assert_eq!(rel.m_table(&shape, &ex, ra, rb, rc), m);
    }

    #[test]
    fn ladders_prove_and_refuse() {
        let (c, inst, wit, _) = circuit(9, 12);
        let shape = c.shape;
        // The ladders run over Fp = Pallas base = Vesta scalar: Vesta commits.
        let gens = crate::ipa::Gens::<iroha_pasta::Eq>::new(shape.cols());
        let mut rng = SpikeRng::new(b"ladders-prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut rng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let mut bad = wit.w.clone();
        let target = bad
            .iter()
            .position(|v| !bool::from(v.is_zero()) && *v != Fb::ONE)
            .unwrap();
        bad[target] += Fb::ONE;
        let (proof, _) =
            crate::spartan::prove_circuit(&shape, &gens, &inst, &Witness { w: bad }, &mut rng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_err(),
            "changed cell refused"
        );
        // A prover that changes a constant column (a fixed base) is refused: the
        // key's commitment to the fixed row no longer matches its opening.
        assert!(
            c.fixed_rows(&shape) > 0,
            "the fixed bases sit in fixed rows"
        );
        let fixed_start = (shape.rows() - c.fixed_rows(&shape)) * shape.cols();
        let mut bad = wit.w.clone();
        let target = fixed_start
            + bad[fixed_start..]
                .iter()
                .position(|v| !bool::from(v.is_zero()))
                .unwrap();
        bad[target] += Fb::ONE;
        let (proof, _) =
            crate::spartan::prove_circuit(&shape, &gens, &inst, &Witness { w: bad }, &mut rng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_err(),
            "changed constant refused"
        );
        assert_eq!(
            proof.to_bytes().len() + 32 * c.fixed_rows(&shape),
            shape.proof_bytes(),
            "fixed rows are not in the proof"
        );
    }
}
