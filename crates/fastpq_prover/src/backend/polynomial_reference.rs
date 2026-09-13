//! Independent test-only polynomial evaluations without rational interpolation.

use super::{add_mod, field_inverse, field_pow, fixed_domain::FixedTraceDomain, mul_mod};
use crate::field::GoldilocksFp4V1 as F;
use fastpq_isi::FASTPQ_FINAL_V1;

/// Evaluate a coefficient vector directly with Horner, using no production evaluator.
pub(super) fn horner(coefficients: &[u64], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |value, &coefficient| {
            value.mul(point).add(F::from_base(coefficient).unwrap())
        })
}

/// Small independent inverse DFT; production FFT and selector formulas are unused.
pub(super) fn interpolate(values: &[u64]) -> Vec<u64> {
    let n = values.len();
    let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, n)
        .unwrap()
        .generator;
    let inverse = field_inverse(generator);
    (0..n)
        .map(|degree| {
            let mut power = 1;
            let step = field_pow(inverse, degree as u64);
            let value = values.iter().fold(0, |sum, &entry| {
                let term = mul_mod(entry, power);
                power = mul_mod(power, step);
                add_mod(sum, term)
            });
            mul_mod(value, field_inverse(n as u64))
        })
        .collect()
}

/// The polynomial 1 + X + ... + X^(n-1), with no inverse or division.
/// Its binary product expansion is independent of the production Lagrange quotient.
fn geometric(mut point: F, mut n: usize) -> F {
    assert!(n.is_power_of_two());
    let mut value = F::ONE;
    while n > 1 {
        value = value.mul(F::ONE.add(point));
        point = point.mul(point);
        n >>= 1;
    }
    value
}

/// Direct degree-(n-1) subgroup Lagrange polynomial at one fixed row.
pub(super) fn lagrange(n: usize, row: usize, point: F) -> F {
    let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, n)
        .unwrap()
        .generator;
    let inverse_root = field_pow(field_inverse(generator), row as u64);
    geometric(point.mul_base(inverse_root), n).mul_base(field_inverse(n as u64))
}

/// Independent periodic-selector polynomial, reduced to the period subgroup.
pub(super) fn periodic(n: usize, period: usize, phase: usize, point: F) -> F {
    let mut exponent = n / period;
    let mut reduced = F::ONE;
    let mut base = point;
    while exponent != 0 {
        if exponent & 1 != 0 {
            reduced = reduced.mul(base);
        }
        base = base.mul(base);
        exponent >>= 1;
    }
    lagrange(period, phase, reduced)
}

/// Base points plus every non-base coordinate and a dense extension point.
pub(super) fn points() -> Vec<F> {
    let mut points: Vec<_> = [
        0,
        1,
        7,
        FASTPQ_FINAL_V1.omega_coset,
        super::GOLDILOCKS_MODULUS - 1,
    ]
    .into_iter()
    .map(|v| F::from_base(v).unwrap())
    .collect();
    for lane in 1..4 {
        let mut words = [0; 4];
        words[0] = 19;
        words[lane] = 31;
        points.push(F::new(words).unwrap());
    }
    points.push(F::new([13, 17, 23, 29]).unwrap());
    points
}
