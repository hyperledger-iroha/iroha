//! Prover kernels of the committed sumchecks (review lane ebb983).
//!
//! Same round values and folded tables as `spartan::round_values` and
//! `spartan::fold_top`, as exact field elements, with less work per row:
//!
//! - Round polynomials are accumulated on `{0, 1, inf}^k` (the value at
//!   infinity is the leading coefficient along that axis) and converted to the
//!   committed grid `{0, 1, 2}^k` once per round. Extending a multilinear
//!   table costs one subtraction per new point instead of two operations, and
//!   a product's value at infinity is the product of the two slopes.
//! - Outer rounds weigh `A` by `eq` once per corner (8 products for k = 3)
//!   instead of once per grid point (27). `C` is multilinear, so it has no
//!   leading coefficient: it only enters the Boolean points.
//! - In the first outer round of a satisfied relation every Boolean point is
//!   zero, so an honest prover skips those points and `C` altogether.
//! - Inner rounds read which corners of the round's variables hold anything,
//!   per run of 256 rows ([`support`]): the public half of `z` and of
//!   `M(r_x, .)` is zero beyond the public inputs, and a padded witness has
//!   empty rows. A variable whose upper (or lower) half is empty in both
//!   tables contributes the factor `(1 - Y)^2` (or `Y^2`), so the round runs
//!   on the remaining sub-cube (9 products instead of 27 for the public half
//!   alone, 3 where half the rows are empty too), and so does its fold.
//! - Tables fold by a tree (`2^k - 1` products per output instead of `2^k`).
//! - Every loop is cut into leaves of bounded length, so a slow core never
//!   holds a large share of a pass (unequal cores: a Pixel 6's 2 + 2 + 4).

use ff::Field;
use rayon::prelude::*;

use crate::grid::{boolean_positions, pow3};

/// Longest leaf of a parallel pass, in rows of the folded table.
const LEAF: usize = 1 << 10;

/// Grid bookkeeping for `k` variables: indices are base 3, coordinate 0 most
/// significant; digit 2 is "infinity" while accumulating and the node 2 after
/// [`Basis::to_grid`].
pub struct Basis {
    pub k: usize,
    cube: usize,
    grid: usize,
    /// Grid positions of the Boolean points, by corner index.
    inputs: Vec<usize>,
    /// Extension steps `(target, one, zero)`: `out[target] = out[one] - out[zero]`.
    steps: Vec<(usize, usize, usize)>,
    /// Grid positions with at least one coordinate at infinity.
    upper: Vec<usize>,
}

fn digits(mut index: usize, k: usize) -> Vec<usize> {
    let mut out = vec![0; k];
    for slot in out.iter_mut().rev() {
        *slot = index % 3;
        index /= 3;
    }
    out
}

fn index_of(digits: &[usize]) -> usize {
    digits.iter().fold(0, |acc, &d| acc * 3 + d)
}

impl Basis {
    pub fn new(k: usize) -> Self {
        let grid = pow3(k);
        let mut steps = Vec::new();
        for j in 0..k {
            for index in 0..grid {
                let point = digits(index, k);
                if point[j] != 2 || point[j + 1..].iter().any(|&d| d == 2) {
                    continue;
                }
                let mut one = point.clone();
                one[j] = 1;
                let mut zero = point;
                zero[j] = 0;
                steps.push((index, index_of(&one), index_of(&zero)));
            }
        }
        let upper = (0..grid).filter(|g| digits(*g, k).contains(&2)).collect();
        Self {
            k,
            cube: 1 << k,
            grid,
            inputs: boolean_positions(k),
            steps,
            upper,
        }
    }

    /// Extends corner values to `{0, 1, inf}^k`.
    #[inline]
    fn extend<F: Field>(&self, corners: &[F], out: &mut [F]) {
        for (value, &position) in corners.iter().zip(&self.inputs) {
            out[position] = *value;
        }
        for &(target, one, zero) in &self.steps {
            out[target] = out[one] - out[zero];
        }
    }

    /// Converts accumulated values on `{0, 1, inf}^k` to the grid
    /// `{0, 1, 2}^k`: along each axis `t(2) = 2 (t(1) + t(inf)) - t(0)` for a
    /// polynomial of degree at most 2 in that variable.
    pub fn to_grid<F: Field>(&self, acc: &mut [F]) {
        let mut stride = 1;
        for _ in 0..self.k {
            for base in 0..self.grid {
                if (base / stride) % 3 != 0 {
                    continue;
                }
                let (t0, t1, tinf) = (acc[base], acc[base + stride], acc[base + 2 * stride]);
                acc[base + 2 * stride] = (t1 + tinf).double() - t0;
            }
            stride *= 3;
        }
    }
}

fn add_into<F: Field>(mut a: Vec<F>, b: Vec<F>) -> Vec<F> {
    a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
    a
}

/// Grid values of `t(Y) = sum_low e(low) (A B - C)(Y, low)`.
///
/// `satisfied`: the tables are `Az, Bz, Cz` of a satisfied relation before any
/// fold, so every Boolean point is zero and is not computed.
pub fn outer_round<F: Field>(
    basis: &Basis,
    a: &[F],
    b: &[F],
    c: &[F],
    e: &[F],
    satisfied: bool,
) -> Vec<F> {
    let (cube, grid) = (basis.cube, basis.grid);
    let low = a.len() >> basis.k;
    assert!(b.len() == a.len() && c.len() == a.len() && e.len() == low);
    let mut acc = (0..low)
        .into_par_iter()
        .with_min_len(64)
        .with_max_len(LEAF)
        .fold(
            || {
                (
                    vec![F::ZERO; grid],
                    vec![F::ZERO; 2 * cube],
                    vec![F::ZERO; 2 * grid],
                )
            },
            |(mut acc, mut corners, mut ext), j| {
                let weight = e[j];
                let (ca, cb) = corners.split_at_mut(cube);
                for hi in 0..cube {
                    ca[hi] = weight * a[hi * low + j];
                    cb[hi] = b[hi * low + j];
                }
                let (ga, gb) = ext.split_at_mut(grid);
                basis.extend(ca, ga);
                basis.extend(cb, gb);
                for &g in &basis.upper {
                    acc[g] += ga[g] * gb[g];
                }
                if !satisfied {
                    for (hi, &g) in basis.inputs.iter().enumerate() {
                        acc[g] += ga[g] * gb[g] - weight * c[hi * low + j];
                    }
                }
                (acc, corners, ext)
            },
        )
        .map(|(acc, _, _)| acc)
        .reduce(|| vec![F::ZERO; grid], add_into);
    basis.to_grid(&mut acc);
    acc
}

/// Rows per run of the corner support.
pub const SEGMENT: usize = 256;

/// Which corners of a round's `k` variables hold anything: per run of
/// [`SEGMENT`] rows `j`, bit `hi` is set when `m` or `z` is nonzero at corner
/// `hi` somewhere in the run. Zero runs are public structure almost always
/// (the public half beyond the inputs, rows without cells); a run of cells
/// that are all zero by value is treated the same way, and the result is the
/// same either way.
pub struct Support {
    k: usize,
    seg: usize,
    masks: Vec<u32>,
}

/// Scans both tables (a run stops at its first nonzero entry).
pub fn support<F: Field>(k: usize, m: &[F], z: &[F]) -> Support {
    assert!(k <= 5 && z.len() == m.len());
    let low = m.len() >> k;
    let seg = SEGMENT.min(low).max(1);
    let masks = (0..low.div_ceil(seg))
        .into_par_iter()
        .with_max_len(16)
        .map(|s| {
            let (j0, j1) = (s * seg, ((s + 1) * seg).min(low));
            (0..1usize << k).fold(0u32, |mask, hi| {
                let run = hi * low + j0..hi * low + j1;
                let any = m[run.clone()].iter().any(|x| !x.is_zero_vartime())
                    || z[run].iter().any(|x| !x.is_zero_vartime());
                mask | (u32::from(any) << hi)
            })
        })
        .collect();
    Support { k, seg, masks }
}

/// How one corner mask reduces a round: the variables that stay free, the
/// corner of each sub-cube point, and the variables whose upper (`low`) or
/// lower (`high`) half is empty.
struct Reduction {
    free: Vec<usize>,
    corners: Vec<usize>,
    low: Vec<usize>,
    high: Vec<usize>,
}

impl Reduction {
    /// `mask` is nonzero.
    fn new(k: usize, mask: u32) -> Self {
        let (mut free, mut low, mut high) = (Vec::new(), Vec::new(), Vec::new());
        let mut base = 0usize;
        for t in 0..k {
            let bit = k - 1 - t;
            let upper = (0..1usize << k).any(|hi| (hi >> bit) & 1 == 1 && (mask >> hi) & 1 == 1);
            let lower = (0..1usize << k).any(|hi| (hi >> bit) & 1 == 0 && (mask >> hi) & 1 == 1);
            match (lower, upper) {
                (true, true) => free.push(t),
                (true, false) => low.push(t),
                _ => {
                    high.push(t);
                    base |= 1 << bit;
                }
            }
        }
        let f = free.len();
        let corners = (0..1usize << f)
            .map(|s| {
                free.iter().enumerate().fold(base, |hi, (i, t)| {
                    hi | (((s >> (f - 1 - i)) & 1) << (k - 1 - t))
                })
            })
            .collect();
        Reduction {
            free,
            corners,
            low,
            high,
        }
    }

    /// Adds the sub-cube's values on `{0, 1, inf}^f` into the round's: a
    /// variable with an empty upper half has `(1 - Y)^2 P` (P at 0 and as
    /// the leading coefficient, 0 at 1), one with an empty lower half
    /// `Y^2 P` (P at 1 and as the leading coefficient, 0 at 0).
    fn scatter<F: Field>(&self, k: usize, sub: &[F], acc: &mut [F]) {
        let f = self.free.len();
        let frozen = self.low.len() + self.high.len();
        for (g, value) in sub.iter().enumerate() {
            let d = digits(g, f);
            let mut point = vec![0usize; k];
            for (i, t) in self.free.iter().enumerate() {
                point[*t] = d[i];
            }
            for choice in 0..1usize << frozen {
                for (c, t) in self.low.iter().enumerate() {
                    point[*t] = if (choice >> c) & 1 == 1 { 2 } else { 0 };
                }
                for (c, t) in self.high.iter().enumerate() {
                    point[*t] = if (choice >> (self.low.len() + c)) & 1 == 1 {
                        2
                    } else {
                        1
                    };
                }
                acc[index_of(&point)] += value;
            }
        }
    }
}

/// The distinct masks of a support with their reductions, and each run's
/// index into them (`u16::MAX` for an empty run).
fn reductions(support: &Support) -> (Vec<(u32, Reduction)>, Vec<u16>) {
    let mut list: Vec<(u32, Reduction)> = Vec::new();
    let runs = support
        .masks
        .iter()
        .map(|&mask| {
            if mask == 0 {
                return u16::MAX;
            }
            match list.iter().position(|(m, _)| *m == mask) {
                Some(i) => i as u16,
                None => {
                    list.push((mask, Reduction::new(support.k, mask)));
                    (list.len() - 1) as u16
                }
            }
        })
        .collect();
    (list, runs)
}

/// Grid values of `q(Y) = sum_low (M Z)(Y, low)`.
///
/// With a [`Support`] every run of rows works on the sub-cube its corners
/// span; without one every row uses the whole cube.
pub fn inner_round<F: Field>(basis: &Basis, m: &[F], z: &[F], support: Option<&Support>) -> Vec<F> {
    let (k, cube, grid) = (basis.k, basis.cube, basis.grid);
    let low = m.len() >> k;
    assert_eq!(z.len(), m.len());
    let Some(support) = support else {
        let mut acc = (0..low)
            .into_par_iter()
            .with_min_len(64)
            .with_max_len(LEAF)
            .fold(
                || {
                    (
                        vec![F::ZERO; grid],
                        vec![F::ZERO; 2 * cube],
                        vec![F::ZERO; 2 * grid],
                    )
                },
                |(mut acc, mut corners, mut ext), j| {
                    let (cm, cz) = corners.split_at_mut(cube);
                    for hi in 0..cube {
                        cm[hi] = m[hi * low + j];
                        cz[hi] = z[hi * low + j];
                    }
                    let (gm, gz) = ext.split_at_mut(grid);
                    basis.extend(cm, gm);
                    basis.extend(cz, gz);
                    for g in 0..grid {
                        acc[g] += gm[g] * gz[g];
                    }
                    (acc, corners, ext)
                },
            )
            .map(|(acc, _, _)| acc)
            .reduce(|| vec![F::ZERO; grid], add_into);
        basis.to_grid(&mut acc);
        return acc;
    };
    assert_eq!(support.k, k);
    let (list, runs) = reductions(support);
    let subs: Vec<Basis> = (0..=k).map(Basis::new).collect();
    let fresh = || {
        (
            list.iter()
                .map(|(_, r)| vec![F::ZERO; pow3(r.free.len())])
                .collect::<Vec<_>>(),
            vec![F::ZERO; 2 * cube],
            vec![F::ZERO; 2 * grid],
        )
    };
    let (parts, _, _) = (0..runs.len())
        .into_par_iter()
        .with_max_len(4)
        .fold(fresh, |(mut parts, mut corners, mut ext), s| {
            if runs[s] == u16::MAX {
                return (parts, corners, ext);
            }
            let reduction = &list[runs[s] as usize].1;
            let sub = &subs[reduction.free.len()];
            let (n, sgrid) = (reduction.corners.len(), sub.grid);
            let part = &mut parts[runs[s] as usize];
            for j in s * support.seg..((s + 1) * support.seg).min(low) {
                let (cm, cz) = corners.split_at_mut(cube);
                for (c, hi) in reduction.corners.iter().enumerate() {
                    cm[c] = m[hi * low + j];
                    cz[c] = z[hi * low + j];
                }
                let (gm, gz) = ext.split_at_mut(grid);
                sub.extend(&cm[..n], &mut gm[..sgrid]);
                sub.extend(&cz[..n], &mut gz[..sgrid]);
                for g in 0..sgrid {
                    part[g] += gm[g] * gz[g];
                }
            }
            (parts, corners, ext)
        })
        .reduce(fresh, |(x, c, e), (y, _, _)| {
            (
                x.into_iter().zip(y).map(|(a, b)| add_into(a, b)).collect(),
                c,
                e,
            )
        });
    let mut acc = vec![F::ZERO; grid];
    for ((_, reduction), part) in list.iter().zip(&parts) {
        reduction.scatter(k, part, &mut acc);
    }
    basis.to_grid(&mut acc);
    acc
}

/// One output of the fold: the multilinear extension of `corners` (first
/// variable most significant) at `r`, by a tree from the last variable.
#[inline]
fn fold_corners<F: Field>(corners: &mut [F], r: &[F]) -> F {
    let mut len = corners.len();
    for t in (0..r.len()).rev() {
        len /= 2;
        for h in 0..len {
            let (v0, v1) = (corners[2 * h], corners[2 * h + 1]);
            corners[h] = v0 + r[t] * (v1 - v0);
        }
    }
    corners[0]
}

/// Binds the top `r.len()` variables of `table` to `r`.
///
/// With a [`Support`] (of this table and its partner) a run folds its
/// sub-cube and multiplies by `1 - r` (or `r`) for every variable whose
/// upper (or lower) half is empty; an empty run folds to zero unread.
pub fn fold_top<F: Field>(table: &[F], r: &[F], support: Option<&Support>) -> Vec<F> {
    let k = r.len();
    let cube = 1usize << k;
    let low = table.len() >> k;
    let Some(support) = support else {
        return (0..low)
            .into_par_iter()
            .with_min_len(64)
            .with_max_len(4 * LEAF)
            .map_init(
                || vec![F::ZERO; cube],
                |corners, j| {
                    for hi in 0..cube {
                        corners[hi] = table[hi * low + j];
                    }
                    fold_corners(corners, r)
                },
            )
            .collect();
    };
    assert_eq!(support.k, k);
    let (list, runs) = reductions(support);
    // Per reduction: the challenges of its free variables and the factor of
    // its other variables.
    let bound: Vec<(Vec<F>, Option<F>)> = list
        .iter()
        .map(|(_, reduction)| {
            let free = reduction.free.iter().map(|t| r[*t]).collect();
            let factor = (reduction.low.len() + reduction.high.len() > 0).then(|| {
                reduction
                    .low
                    .iter()
                    .map(|t| F::ONE - r[*t])
                    .chain(reduction.high.iter().map(|t| r[*t]))
                    .product()
            });
            (free, factor)
        })
        .collect();
    (0..low)
        .into_par_iter()
        .with_min_len(64)
        .with_max_len(4 * LEAF)
        .map_init(
            || vec![F::ZERO; cube],
            |corners, j| {
                let run = runs[j / support.seg];
                if run == u16::MAX {
                    return F::ZERO;
                }
                let (reduction, (free, factor)) = (&list[run as usize].1, &bound[run as usize]);
                for (c, hi) in reduction.corners.iter().enumerate() {
                    corners[c] = table[hi * low + j];
                }
                let value = fold_corners(&mut corners[..reduction.corners.len()], free);
                factor.map_or(value, |f| f * value)
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::grid::{Extender, eq_table};
    use iroha_pasta::Fp;

    /// The round values the spike's first kernels compute.
    fn reference_round(
        k: usize,
        a: &[Fp],
        b: &[Fp],
        c: Option<&[Fp]>,
        e: Option<&[Fp]>,
    ) -> Vec<Fp> {
        let extender = Extender::new(k);
        let (cube, grid) = (1 << k, pow3(k));
        let low = a.len() >> k;
        let mut acc = vec![Fp::ZERO; grid];
        for j in 0..low {
            let pick = |t: &[Fp]| -> Vec<Fp> { (0..cube).map(|hi| t[hi * low + j]).collect() };
            let (mut ga, mut gb, mut gc) = (
                vec![Fp::ZERO; grid],
                vec![Fp::ZERO; grid],
                vec![Fp::ZERO; grid],
            );
            extender.extend(&pick(a), &mut ga);
            extender.extend(&pick(b), &mut gb);
            match (c, e) {
                (Some(c), Some(e)) => {
                    extender.extend(&pick(c), &mut gc);
                    for g in 0..grid {
                        acc[g] += e[j] * (ga[g] * gb[g] - gc[g]);
                    }
                }
                _ => {
                    for g in 0..grid {
                        acc[g] += ga[g] * gb[g];
                    }
                }
            }
        }
        acc
    }

    fn reference_fold(table: &[Fp], r: &[Fp]) -> Vec<Fp> {
        let low = table.len() >> r.len();
        let eq_r = eq_table(r);
        (0..low)
            .map(|j| {
                eq_r.iter()
                    .enumerate()
                    .map(|(hi, e)| *e * table[hi * low + j])
                    .sum()
            })
            .collect()
    }

    #[test]
    fn rounds_and_folds_equal_the_reference_kernels() {
        let mut rng = SpikeRng::new(b"kern");
        for k in 1..=4 {
            for log in [k, k + 1, k + 5, k + 10] {
                let n = 1usize << log;
                let low = n >> k;
                let random =
                    |rng: &mut SpikeRng| -> Vec<Fp> { (0..n).map(|_| rng.scalar()).collect() };
                let (a, b, c) = (random(&mut rng), random(&mut rng), random(&mut rng));
                let e: Vec<Fp> = (0..low).map(|_| rng.scalar()).collect();
                let basis = Basis::new(k);
                assert_eq!(
                    outer_round(&basis, &a, &b, &c, &e, false),
                    reference_round(k, &a, &b, Some(&c), Some(&e)),
                    "outer k {k} log {log}"
                );
                // A satisfied relation: c = a b on every row.
                let ab: Vec<Fp> = a.iter().zip(&b).map(|(x, y)| *x * y).collect();
                let expected = reference_round(k, &a, &b, Some(&ab), Some(&e));
                assert_eq!(
                    outer_round(&basis, &a, &b, &ab, &e, true),
                    expected,
                    "satisfied outer k {k} log {log}"
                );
                assert_eq!(
                    outer_round(&basis, &a, &b, &ab, &e, false),
                    expected,
                    "exact outer k {k} log {log}"
                );
                assert_eq!(
                    inner_round(&basis, &a, &b, None),
                    reference_round(k, &a, &b, None, None),
                    "inner k {k} log {log}"
                );
                let r: Vec<Fp> = (0..k).map(|_| rng.scalar()).collect();
                assert_eq!(
                    fold_top(&a, &r, None),
                    reference_fold(&a, &r),
                    "fold k {k} log {log}"
                );
                // Padded tables: per run of rows, corners emptied in both
                // tables by a pattern (the public half beyond a few rows,
                // empty quarters, single corners, everything), with one
                // stray value in m to keep a corner's run alive.
                let (mut m, mut z) = (a.clone(), b.clone());
                let seg = SEGMENT.min(low);
                for run in 0..low.div_ceil(seg) {
                    let pattern = (run * 7 + log) % 6;
                    for hi in 0..1usize << k {
                        let top = hi >> (k - 1) == 1;
                        let empty = match pattern {
                            0 => top,
                            1 => top || (k >= 2 && (hi >> (k - 2)) & 1 == 1),
                            2 => !top,
                            3 => hi != (1 << k) - 1 && hi != 0,
                            4 => true,
                            _ => false,
                        };
                        if empty {
                            for j in run * seg..((run + 1) * seg).min(low) {
                                m[hi * low + j] = Fp::ZERO;
                                z[hi * low + j] = Fp::ZERO;
                            }
                        }
                    }
                }
                if n >= 4 {
                    m[n - 1] = Fp::from(5u64);
                }
                let sup = support(k, &m, &z);
                assert_eq!(
                    inner_round(&basis, &m, &z, Some(&sup)),
                    reference_round(k, &m, &z, None, None),
                    "padded inner k {k} log {log}"
                );
                assert_eq!(
                    inner_round(&basis, &m, &z, None),
                    reference_round(k, &m, &z, None, None),
                    "dense inner on padded k {k} log {log}"
                );
                assert_eq!(
                    fold_top(&m, &r, Some(&sup)),
                    reference_fold(&m, &r),
                    "padded fold of m k {k} log {log}"
                );
                assert_eq!(
                    fold_top(&z, &r, Some(&sup)),
                    reference_fold(&z, &r),
                    "padded fold of z k {k} log {log}"
                );
            }
        }
    }
}
