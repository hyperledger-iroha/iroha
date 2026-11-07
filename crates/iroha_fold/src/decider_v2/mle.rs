//! Multilinear primitives of the v2 decider.
//!
//! Row `i` of a column of length `n = 2^mu` is the hypercube point of its
//! little-endian bits, `x_j = (i >> j) & 1`, and a column is the table of its
//! multilinear extension. Sumchecks bind the lowest variable first.

use ff::{Field, PrimeField};

/// `eq(x, y) = prod_j (x_j y_j + (1 - x_j)(1 - y_j))`.
pub(crate) fn eq<F: Field>(x: &[F], y: &[F]) -> F {
    x.iter()
        .zip(y)
        .fold(F::ONE, |acc, (a, b)| acc * (*a * b + (F::ONE - a) * (F::ONE - b)))
}

/// The table of `eq(point, i)` over every row `i < 2^point.len()`.
pub(crate) fn eq_table<F: Field>(point: &[F]) -> Vec<F> {
    let mut table = Vec::with_capacity(1 << point.len());
    table.push(F::ONE);
    for x in point {
        // `table` spans the bits below this one; the new bit is the top one.
        let len = table.len();
        table.resize(2 * len, F::ZERO);
        for i in 0..len {
            let high = table[i] * x;
            table[i] -= high;
            table[i + len] = high;
        }
    }
    table
}

/// Binds the lowest variable of `table` to `r`: `T'[i] = T[2i] + r (T[2i+1] - T[2i])`.
pub(crate) fn fold<F: Field>(table: &mut Vec<F>, r: F) {
    let half = table.len() / 2;
    for i in 0..half {
        let (low, high) = (table[2 * i], table[2 * i + 1]);
        table[i] = low + r * (high - low);
    }
    table.truncate(half);
}

/// The multilinear extension of `table` at `point`.
pub(crate) fn evaluate<F: Field>(table: &[F], point: &[F]) -> F {
    debug_assert_eq!(table.len(), 1 << point.len());
    let weights = eq_table(point);
    table.iter().zip(&weights).fold(F::ZERO, |acc, (v, w)| acc + *v * w)
}

/// `pow(x) = prod_j (1 - x_j + x_j beta_j)`, on the hypercube the `pow` weight
/// `prod_j beta_j^{i_j}` of row `i`.
pub(crate) fn pow<F: Field>(beta: &[F], x: &[F]) -> F {
    beta.iter().zip(x).fold(F::ONE, |acc, (b, v)| acc * (F::ONE - v + *v * b))
}

/// `sum_{i < u} eq(x, i)`, the multilinear extension of the indicator of the
/// first `u` rows. A row below `u` agrees with `u` above the highest bit where
/// they differ, has a zero there where `u` has a one, and is free below.
pub(crate) fn prefix_indicator<F: Field>(u: usize, x: &[F]) -> F {
    if u >= 1 << x.len() {
        return F::ONE;
    }
    let mut total = F::ZERO;
    let mut above = F::ONE;
    for (j, v) in x.iter().enumerate().rev() {
        if (u >> j) & 1 == 1 {
            total += above * (F::ONE - v);
            above *= v;
        } else {
            above *= F::ONE - v;
        }
    }
    total
}

/// `offset + sum_j 2^j x_j`, the multilinear extension of `offset + i`.
pub(crate) fn index<F: PrimeField>(offset: u64, x: &[F]) -> F {
    let mut total = F::from(offset);
    let mut power = F::ONE;
    for v in x {
        total += power * v;
        power = power.double();
    }
    total
}

/// Row `i` at rotation `k` of an `n`-row column: `(i + k) mod n`.
pub(crate) fn rotate(i: usize, k: i32, n: usize) -> usize {
    let n = i64::try_from(n).expect("domain fits i64");
    let i = i64::try_from(i).expect("row fits i64");
    usize::try_from((i + i64::from(k)).rem_euclid(n)).expect("rotated row fits usize")
}

/// The column `v` at rotation `k`: row `i` holds `v[(i + k) mod n]`.
pub(crate) fn rotated<F: Copy>(column: &[F], k: i32) -> Vec<F> {
    let n = column.len();
    (0..n).map(|i| column[rotate(i, k, n)]).collect()
}

/// `sum_j eq(rho, j) eq(sigma, (j + k) mod n)`: the multilinear extension at
/// `sigma` of the weights `e[m] = eq(rho, (m - k) mod n)`, so that a column's
/// rotated value at `rho` is `sum_m v[m] e[m]`. Brute force over the `n` rows
/// (the native oracle; the succinct carry formula belongs to `pcs::mle`).
pub(crate) fn shifted_eq<F: Field>(rho: &[F], sigma: &[F], k: i32) -> F {
    let a = eq_table(rho);
    let b = eq_table(sigma);
    let n = a.len();
    (0..n).fold(F::ZERO, |acc, j| acc + a[j] * b[rotate(j, k, n)])
}
