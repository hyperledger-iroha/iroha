//! Independent small polynomial interpolation used only by degree-owner tests.

const P: u64 = 0xffff_ffff_0000_0001;

pub(crate) fn add(left: u64, right: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(P)) as u64
}

pub(crate) fn sub(left: u64, right: u64) -> u64 {
    ((u128::from(left) + u128::from(P) - u128::from(right)) % u128::from(P)) as u64
}

pub(crate) fn mul(left: u64, right: u64) -> u64 {
    (u128::from(left) * u128::from(right) % u128::from(P)) as u64
}

pub(crate) fn power(mut value: u64, mut exponent: usize) -> u64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = mul(result, value);
        }
        value = mul(value, value);
        exponent >>= 1;
    }
    result
}

pub(crate) fn horner(coefficients: &[u64], point: u64) -> u64 {
    coefficients
        .iter()
        .rev()
        .fold(0, |value, &coefficient| add(mul(value, point), coefficient))
}

pub(crate) fn polynomial(bound: usize, seed: usize, point: u64) -> u64 {
    add(
        seed as u64 + 1,
        mul(seed as u64 + 3, power(point, bound - 1)),
    )
}

pub(crate) fn product(left: &[u64], right: &[u64]) -> Vec<u64> {
    let mut out = vec![0; left.len() + right.len() - 1];
    for (i, &a) in left.iter().enumerate() {
        for (j, &b) in right.iter().enumerate() {
            out[i + j] = add(out[i + j], mul(a, b));
        }
    }
    out
}

pub(crate) fn degree_bound(coefficients: &[u64]) -> usize {
    coefficients
        .iter()
        .rposition(|&value| value != 0)
        .map_or(0, |i| i + 1)
}

pub(crate) fn interpolate_columns(samples: &[Vec<u64>]) -> Vec<Vec<u64>> {
    let count = samples.len();
    let width = samples[0].len();
    let mut out = vec![vec![0; count]; width];
    for (i, row) in samples.iter().enumerate() {
        assert_eq!(row.len(), width);
        let mut numerator = vec![1];
        let mut denominator = 1;
        for j in 0..count {
            if j != i {
                numerator = product(&numerator, &[sub(0, j as u64), 1]);
                denominator = mul(denominator, sub(i as u64, j as u64));
            }
        }
        // Test interpolation has at most twelve distinct base points. Use a u64
        // exponent directly so this reference does not depend on usize width.
        let mut base = denominator;
        let mut inverse = 1;
        let mut exponent = P - 2;
        while exponent != 0 {
            if exponent & 1 != 0 {
                inverse = mul(inverse, base);
            }
            base = mul(base, base);
            exponent >>= 1;
        }
        for (column, &value) in row.iter().enumerate() {
            for (degree, &coefficient) in numerator.iter().enumerate() {
                out[column][degree] =
                    add(out[column][degree], mul(mul(value, inverse), coefficient));
            }
        }
    }
    out
}
