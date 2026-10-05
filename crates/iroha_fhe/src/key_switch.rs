//! Digit decomposition and the digit/key inner product of key switching.
//!
//! Relinearization, Galois key switching and collective key switching share
//! one arithmetic shape: split the switching component into small base-`B`
//! digits, multiply each digit polynomial by the matching pair of key rows and
//! accumulate the two ciphertext components. Key formats, key generation and
//! the choice of ring product stay with each scheme; only that shape lives
//! here.
use zeroize::Zeroizing;

/// Split every coefficient into `digits` base-`base` digits, least significant first.
///
/// Returns one polynomial of `degree` coefficients per digit. Coefficient
/// digits beyond `digits` are dropped, so callers choose `digits` to cover the
/// coefficient range.
///
/// # Returns
/// `None` when `base < 2` or `poly` has more than `degree` coefficients.
#[must_use]
pub fn decompose_digits(
    poly: &[u64],
    degree: usize,
    base: u64,
    digits: usize,
) -> Option<Vec<Vec<u64>>> {
    if base < 2 || poly.len() > degree {
        return None;
    }
    let mut output = vec![vec![0_u64; degree]; digits];
    for (coefficient_index, &coefficient) in poly.iter().enumerate() {
        let mut value = coefficient;
        for digit_poly in &mut output {
            digit_poly[coefficient_index] = value % base;
            value /= base;
        }
    }
    Some(output)
}

/// Accumulate both components of a key switch in one pass over the digits.
///
/// Computes `initial_k + sum_i multiply(digit_i, row_i_k)` for `k = 0, 1`.
/// Digits and key rows are paired positionally and the shorter sequence ends
/// the sum. Each key row is a pair `(row_0, row_1)`; for every digit the first
/// component is multiplied and added before the second, so a failing step is
/// reported in digit order, first component first. `multiply` and `add` are
/// the caller's ring product and sum, so the same accumulation serves
/// coefficient-form and RNS corridors.
///
/// The accumulators and each contribution live in clearing buffers: a
/// superseded accumulator is cleared when the next one replaces it, and a
/// failing step leaves no partial sum behind.
///
/// # Errors
/// The first error returned by `multiply` or `add`.
pub fn digit_inner_product_pair<D, R, E>(
    initial: (Vec<u64>, Vec<u64>),
    digits: &[D],
    rows: impl IntoIterator<Item = (R, R)>,
    mut multiply: impl FnMut(&D, R) -> Result<Vec<u64>, E>,
    mut add: impl FnMut(&[u64], &[u64]) -> Result<Vec<u64>, E>,
) -> Result<(Vec<u64>, Vec<u64>), E> {
    let mut first = Zeroizing::new(initial.0);
    let mut second = Zeroizing::new(initial.1);
    for (digit, (first_row, second_row)) in digits.iter().zip(rows) {
        let contribution = Zeroizing::new(multiply(digit, first_row)?);
        first = Zeroizing::new(add(&first, &contribution)?);
        let contribution = Zeroizing::new(multiply(digit, second_row)?);
        second = Zeroizing::new(add(&second, &contribution)?);
    }
    Ok((std::mem::take(&mut *first), std::mem::take(&mut *second)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_recompose_to_the_coefficients() {
        let poly = [0_u64, 1, 4_095, 4_096, 269_484_031, u64::MAX];
        let base = 1_u64 << 12;
        let digits = decompose_digits(&poly, 8, base, 6).expect("digits");
        assert_eq!(digits.len(), 6);
        assert!(digits.iter().all(|digit| digit.len() == 8));
        assert!(digits.iter().flatten().all(|&digit| digit < base));
        for (index, &coefficient) in poly.iter().enumerate() {
            let recomposed = digits.iter().rev().fold(0_u128, |value, digit| {
                value * u128::from(base) + u128::from(digit[index])
            });
            // Six 12-bit digits cover 72 bits, more than a word.
            assert_eq!(recomposed, u128::from(coefficient));
        }
        assert!(digits.iter().all(|digit| digit[6] == 0 && digit[7] == 0));
        // Too few digits drop the high part.
        let short = decompose_digits(&[4_097], 1, base, 1).expect("digits");
        assert_eq!(short, vec![vec![1]]);
        assert_eq!(decompose_digits(&[1], 1, base, 0), Some(Vec::new()));
    }

    #[test]
    fn decomposition_rejects_a_degenerate_base_and_oversized_polynomials() {
        assert_eq!(decompose_digits(&[1], 1, 0, 2), None);
        assert_eq!(decompose_digits(&[1], 1, 1, 2), None);
        assert_eq!(decompose_digits(&[1, 2], 1, 4, 2), None);
    }

    #[test]
    fn inner_product_accumulates_in_order_and_stops_at_the_shorter_sequence() {
        let modulus = 17_u64;
        let digits = [vec![1_u64, 2], vec![3, 4], vec![5, 6]];
        let rows = [vec![2_u64, 0], vec![1, 1]];
        let multiply = |digit: &Vec<u64>, row: &Vec<u64>| -> Result<Vec<u64>, ()> {
            Ok(crate::polynomial::negacyclic_mul_mod_schoolbook(
                digit, row, 2, modulus,
            ))
        };
        let add = |lhs: &[u64], rhs: &[u64]| -> Result<Vec<u64>, ()> {
            Ok(crate::polynomial::add_mod(lhs, rhs, modulus))
        };
        // The two components are interleaved digit by digit.
        // first: [7, 7] + (1 + 2X) * 2 + (3 + 4X)(1 + X) = [7, 7] + [2, 4] + [3 - 4, 7] = [8, 1].
        let mut order = Vec::new();
        let traced = |digit: &Vec<u64>, row: &Vec<u64>| -> Result<Vec<u64>, ()> {
            order.push((digit[0], row[0]));
            Ok(crate::polynomial::negacyclic_mul_mod_schoolbook(
                digit, row, 2, modulus,
            ))
        };
        let pairs = [(&rows[0], &rows[1]), (&rows[1], &rows[0])];
        assert_eq!(
            digit_inner_product_pair((vec![7, 7], vec![0, 0]), &digits, pairs, traced, add),
            // second: (1 + 2X)(1 + X) + (3 + 4X) * 2 = [1 - 2, 3] + [6, 8] = [5, 11].
            Ok((vec![8, 1], vec![5, 11]))
        );
        assert_eq!(order, [(1, 2), (1, 1), (3, 1), (3, 2)]);
        let mut calls = 0_u32;
        let fail_second = |_: &Vec<u64>, _: &Vec<u64>| -> Result<Vec<u64>, u32> {
            calls += 1;
            if calls == 2 {
                Err(calls)
            } else {
                Ok(vec![0, 0])
            }
        };
        assert_eq!(
            digit_inner_product_pair(
                (vec![0, 0], vec![0, 0]),
                &digits,
                pairs,
                fail_second,
                |lhs, _| Ok(lhs.to_vec())
            ),
            Err(2)
        );
        let no_digits: [Vec<u64>; 0] = [];
        assert_eq!(
            digit_inner_product_pair((vec![9], vec![4]), &no_digits, pairs, multiply, add),
            Ok((vec![9], vec![4]))
        );
    }
}
