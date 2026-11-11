//! Grid evaluations of k-variate polynomials of individual degree at most 2.
//!
//! A committed sumcheck round sends one hiding commitment to the round
//! polynomial's values on `{0,1,2}^k` (coordinate 0 most significant). The
//! verifier's checks on those values are linear: Boolean-cube sums and
//! Lagrange evaluation at the round challenges.
//!
//! In the IPA vector every round owns an aligned slot of 4^k columns (k the
//! shape's variables per round): grid point `d` sits at the slot offset whose
//! base-4 digits are `d` (digit 3 stays empty; a shorter last round uses the
//! low digits, its leading ones 0), so each coordinate owns two column bits
//! and the IPA's `<s(u), b>` over a slot factors into one short sum per
//! coordinate.

use ff::{Field, PrimeField};

pub fn pow3(k: usize) -> usize {
    3_usize.pow(k as u32)
}

/// Columns of a round slot.
pub fn pow4(k: usize) -> usize {
    1 << (2 * k)
}

/// Slot offsets of the grid points: grid index `g` (base 3, coordinate 0 most
/// significant) at the offset with the same digits in base 4.
pub fn slot_positions(k: usize) -> Vec<usize> {
    (0..pow3(k))
        .map(|g| digits(g, k).iter().fold(0, |acc, &d| acc * 4 + d))
        .collect()
}

/// The variables of a round from its grid size `3^k`.
pub fn grid_vars(len: usize) -> usize {
    let mut k = 0;
    while pow3(k) < len {
        k += 1;
    }
    assert_eq!(pow3(k), len, "a grid of 3^k values");
    k
}

fn grid_index(digits: &[usize]) -> usize {
    digits.iter().fold(0, |acc, &digit| acc * 3 + digit)
}

fn digits(mut index: usize, k: usize) -> Vec<usize> {
    let mut out = vec![0; k];
    for slot in out.iter_mut().rev() {
        *slot = index % 3;
        index /= 3;
    }
    out
}

/// Grid positions of the Boolean points `b in {0,1}^k`, indexed by `b` read
/// most significant bit first.
pub fn boolean_positions(k: usize) -> Vec<usize> {
    (0..1_usize << k)
        .map(|b| (0..k).fold(0, |acc, j| acc * 3 + ((b >> (k - 1 - j)) & 1)))
        .collect()
}

/// Extends multilinear values on `{0,1}^k` to `{0,1,2}^k` in place, one
/// dimension at a time (`f(2) = 2 f(1) - f(0)`), with precomputed steps.
pub struct Extender {
    pub k: usize,
    inputs: Vec<usize>,
    steps: Vec<(usize, usize, usize)>,
}

impl Extender {
    pub fn new(k: usize) -> Self {
        let mut steps = Vec::new();
        for j in 0..k {
            for index in 0..pow3(k) {
                let point = digits(index, k);
                if point[j] != 2 || point[j + 1..].iter().any(|&digit| digit == 2) {
                    continue;
                }
                let mut one = point.clone();
                one[j] = 1;
                let mut zero = point;
                zero[j] = 0;
                steps.push((index, grid_index(&one), grid_index(&zero)));
            }
        }
        let extender = Self {
            k,
            inputs: boolean_positions(k),
            steps,
        };
        assert_eq!(extender.inputs.len() + extender.steps.len(), pow3(k));
        extender
    }

    #[inline]
    pub fn extend<F: Field>(&self, input: &[F], out: &mut [F]) {
        for (value, &position) in input.iter().zip(&self.inputs) {
            out[position] = *value;
        }
        for &(target, one, zero) in &self.steps {
            out[target] = out[one].double() - out[zero];
        }
    }
}

/// Lagrange basis of the nodes 0, 1, 2 at `t`.
pub fn lagrange3<F: PrimeField>(t: F) -> [F; 3] {
    let half = F::from(2).invert().expect("2 is invertible");
    let one = F::ONE;
    let two = F::from(2);
    [
        (t - one) * (t - two) * half,
        t * (two - t),
        t * (t - one) * half,
    ]
}

/// Weights `w` with `p(r) = sum_g w[g] p(g)` over the grid `{0,1,2}^k`.
pub fn grid_weights<F: PrimeField>(r: &[F]) -> Vec<F> {
    let mut weights = vec![F::ONE];
    for &t in r {
        let basis = lagrange3(t);
        let mut next = Vec::with_capacity(weights.len() * 3);
        for weight in &weights {
            for value in basis {
                next.push(*weight * value);
            }
        }
        weights = next;
    }
    weights
}

/// `eq(r, x)` for every `x in {0,1}^len(r)`, `r[0]` the most significant bit.
///
/// Large tables are the product of a table over the leading variables and one
/// over the last ten, filled in parallel: every entry is the same exact
/// product of factors as in the serial table.
pub fn eq_table<F: Field>(r: &[F]) -> Vec<F> {
    use rayon::prelude::*;
    const LOW: usize = 10;
    if r.len() < LOW + 4 || std::env::var("ZKS_KERN").as_deref() == Ok("old") {
        return eq_table_serial(r);
    }
    let (high, low) = r.split_at(r.len() - LOW);
    let (th, tl) = (eq_table(high), eq_table_serial(low));
    let mask = (1usize << LOW) - 1;
    (0..1usize << r.len())
        .into_par_iter()
        .with_min_len(1 << LOW)
        .with_max_len(1 << 13)
        .map(|i| th[i >> LOW] * tl[i & mask])
        .collect()
}

fn eq_table_serial<F: Field>(r: &[F]) -> Vec<F> {
    let mut table = Vec::with_capacity(1 << r.len());
    table.push(F::ONE);
    for &ri in r {
        let mut next = Vec::with_capacity(table.len() * 2);
        for value in &table {
            let high = *value * ri;
            next.push(*value - high);
            next.push(high);
        }
        table = next;
    }
    table
}

pub fn eq_eval<F: Field>(a: &[F], b: &[F]) -> F {
    a.iter()
        .zip(b)
        .map(|(x, y)| *x * y + (F::ONE - x) * (F::ONE - y))
        .product()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_pasta::Fp;

    #[test]
    fn extension_matches_lagrange_evaluation() {
        for k in 1..=5 {
            let extender = Extender::new(k);
            let input: Vec<Fp> = (0..1_u64 << k).map(|i| Fp::from(i * i + 7)).collect();
            let mut grid = vec![Fp::ZERO; pow3(k)];
            extender.extend(&input, &mut grid);
            let r: Vec<Fp> = (0..k as u64).map(|i| Fp::from(1000 + 13 * i)).collect();
            let via_grid: Fp = grid_weights(&r)
                .iter()
                .zip(&grid)
                .map(|(w, v)| *w * v)
                .sum();
            let via_eq: Fp = eq_table(&r).iter().zip(&input).map(|(w, v)| *w * v).sum();
            assert_eq!(via_grid, via_eq, "k = {k}");
        }
    }
}
