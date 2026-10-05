//! Coefficient-form polynomial arithmetic in `Z_m[X] / (X^n + 1)`.
//!
//! The schoolbook products here are the deterministic fallback for the
//! transform paths of [`crate::ntt`] and the reference those paths are tested
//! against. They apply the negacyclic rule `X^n = -1` directly: a term whose
//! exponent reaches `n` lands on exponent `k - n` with its sign flipped.
use crate::{
    accel,
    modular::{ModularArithmetic, WordModulus, reduce_i128_to_u64_mod, sub_mod_u64},
    rounding::center_lift,
};
use thiserror::Error;
use zeroize::Zeroizing;

/// Failure of a checked polynomial kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PolynomialError {
    /// Two operands that must have equal length do not.
    #[error("polynomial length mismatch: {lhs} != {rhs}")]
    LengthMismatch {
        /// Length of the left operand.
        lhs: usize,
        /// Length of the right operand.
        rhs: usize,
    },
    /// A coefficient product does not fit `i128`.
    #[error("raw product coefficient exceeds i128")]
    ProductOverflow,
    /// The sum of two coefficient indices does not fit `usize`.
    #[error("raw product exponent exceeds usize")]
    ExponentOverflow,
    /// A negacyclic wrap subtraction does not fit `i128`.
    #[error("raw negacyclic subtraction exceeds i128")]
    SubtractionOverflow,
    /// An accumulation does not fit `i128`.
    #[error("raw addition exceeds i128")]
    AdditionOverflow,
}

/// The zero polynomial with `degree` coefficients.
#[must_use]
pub fn zero(degree: usize) -> Vec<u64> {
    vec![0; degree]
}

/// Coefficient-wise `lhs + rhs` modulo `modulus` over the common prefix.
#[must_use]
pub fn add_mod(lhs: &[u64], rhs: &[u64], modulus: u64) -> Vec<u64> {
    let mut output = lhs[..lhs.len().min(rhs.len())].to_vec();
    accel::add_mod_assign(&mut output, rhs, modulus);
    output
}

/// Coefficient-wise `lhs - rhs` modulo `modulus` over the common prefix.
#[must_use]
pub fn sub_mod(lhs: &[u64], rhs: &[u64], modulus: u64) -> Vec<u64> {
    let mut output = lhs[..lhs.len().min(rhs.len())].to_vec();
    accel::sub_mod_assign(&mut output, rhs, modulus);
    output
}

/// Coefficient-wise negation modulo `modulus`; zero stays zero.
#[must_use]
pub fn neg_mod(poly: &[u64], modulus: u64) -> Vec<u64> {
    poly.iter()
        .map(|&coefficient| sub_mod_u64(0, coefficient, modulus))
        .collect()
}

/// Multiply every coefficient by `scalar` modulo `modulus`.
#[must_use]
pub fn scalar_mul_mod(poly: &[u64], scalar: u64, modulus: u64) -> Vec<u64> {
    let mut output = poly.to_vec();
    accel::mul_scalar_mod(&mut output, scalar, modulus);
    output
}

/// Coefficient-wise checked sum of two signed raw polynomials.
///
/// The sum is built in a clearing buffer, so a rejected sum leaves no partial
/// result behind.
///
/// # Errors
/// [`PolynomialError::LengthMismatch`] or [`PolynomialError::AdditionOverflow`].
pub fn add_centered_raw(lhs: &[i128], rhs: &[i128]) -> Result<Vec<i128>, PolynomialError> {
    if lhs.len() != rhs.len() {
        return Err(PolynomialError::LengthMismatch {
            lhs: lhs.len(),
            rhs: rhs.len(),
        });
    }
    let mut sum = Zeroizing::new(Vec::with_capacity(lhs.len()));
    for (&left, &right) in lhs.iter().zip(rhs) {
        sum.push(
            left.checked_add(right)
                .ok_or(PolynomialError::AdditionOverflow)?,
        );
    }
    Ok(std::mem::take(&mut *sum))
}

/// Least non-negative residues of signed raw coefficients.
#[must_use]
pub fn reduce_raw_mod(raw: &[i128], modulus: u64) -> Vec<u64> {
    raw.iter()
        .map(|&coefficient| reduce_i128_to_u64_mod(coefficient, modulus))
        .collect()
}

/// Schoolbook negacyclic product in `Z_m[X] / (X^n + 1)` over any modular arithmetic.
///
/// The ring degree `n` is `output.len()`, and `output` is overwritten with the
/// product. This is the one schoolbook residue product of the crate:
/// [`negacyclic_mul_mod_schoolbook`] runs it over [`WordModulus`], and a
/// caller with secret-dependent operands runs it over
/// [`crate::constant_time::FixedModulus`]. Operands must satisfy the operand
/// contract of the arithmetic (canonical residues for the fixed modulus). The
/// loop bounds and the index schedule depend only on the operand lengths, so
/// the product is constant-time whenever the arithmetic is.
///
/// # Returns
/// `None`, leaving `output` untouched, when an operand has more than
/// `output.len()` coefficients.
pub fn negacyclic_mul_schoolbook_into_with<A: ModularArithmetic>(
    lhs: &[u64],
    rhs: &[u64],
    arithmetic: &A,
    output: &mut [u64],
) -> Option<()> {
    let degree = output.len();
    if lhs.len() > degree || rhs.len() > degree {
        return None;
    }
    output.fill(0);
    for (lhs_index, &lhs_coefficient) in lhs.iter().enumerate() {
        for (rhs_index, &rhs_coefficient) in rhs.iter().enumerate() {
            let term = arithmetic.mul(lhs_coefficient, rhs_coefficient);
            let raw_index = lhs_index + rhs_index;
            if raw_index >= degree {
                let index = raw_index - degree;
                output[index] = arithmetic.sub(output[index], term);
            } else {
                output[raw_index] = arithmetic.add(output[raw_index], term);
            }
        }
    }
    Some(())
}

/// Schoolbook negacyclic product of residue vectors in `Z_m[X] / (X^degree + 1)`.
///
/// Operands may be unreduced; a zero modulus yields the zero polynomial.
///
/// # Panics
/// When an operand has more than `degree` coefficients.
#[must_use]
pub fn negacyclic_mul_mod_schoolbook(
    lhs: &[u64],
    rhs: &[u64],
    degree: usize,
    modulus: u64,
) -> Vec<u64> {
    let mut product = vec![0_u64; degree];
    negacyclic_mul_schoolbook_into_with(lhs, rhs, &WordModulus(modulus), &mut product)
        .expect("operands have at most `degree` coefficients");
    product
}

/// Schoolbook negacyclic product of word polynomials over the integers.
///
/// Coefficients are widened to `i128` in clearing buffers and nothing is
/// reduced. The caller guarantees `degree * max(lhs) * max(rhs) <= i128::MAX`.
///
/// # Panics
/// When an operand has more than `degree` coefficients.
#[must_use]
pub fn negacyclic_mul_raw_schoolbook(lhs: &[u64], rhs: &[u64], degree: usize) -> Vec<i128> {
    let lhs = Zeroizing::new(
        lhs.iter()
            .map(|&value| i128::from(value))
            .collect::<Vec<_>>(),
    );
    let rhs = Zeroizing::new(
        rhs.iter()
            .map(|&value| i128::from(value))
            .collect::<Vec<_>>(),
    );
    negacyclic_mul_raw_schoolbook_i128(&lhs, &rhs, degree)
}

/// Schoolbook negacyclic product of signed polynomials over the integers.
///
/// The accumulator clears on every exit. The caller guarantees that no
/// coefficient of the true product leaves `i128`.
///
/// # Panics
/// When an operand has more than `degree` coefficients.
#[must_use]
pub fn negacyclic_mul_raw_schoolbook_i128(lhs: &[i128], rhs: &[i128], degree: usize) -> Vec<i128> {
    let mut acc = Zeroizing::new(vec![0_i128; degree]);
    for (i, &left) in lhs.iter().enumerate() {
        for (j, &right) in rhs.iter().enumerate() {
            let index = i + j;
            let term = left * right;
            if index < degree {
                acc[index] += term;
            } else {
                acc[index - degree] -= term;
            }
        }
    }
    std::mem::take(&mut *acc)
}

/// Checked schoolbook negacyclic product of the centered lifts of two polynomials.
///
/// Each coefficient is lifted with [`center_lift`] relative to `modulus`
/// before multiplying, so `q - a` multiplies as `-a`. Every multiplication and
/// accumulation is checked, and the accumulator clears when the product is
/// rejected.
///
/// # Errors
/// The [`PolynomialError`] overflow variants.
///
/// # Panics
/// When an operand has more than `degree` coefficients.
pub fn negacyclic_mul_centered_raw(
    lhs: &[u64],
    rhs: &[u64],
    degree: usize,
    modulus: u64,
) -> Result<Vec<i128>, PolynomialError> {
    let mut out = Zeroizing::new(vec![0_i128; degree]);
    for (lhs_index, &lhs_coefficient) in lhs.iter().enumerate() {
        let lhs_centered = center_lift(lhs_coefficient, modulus);
        for (rhs_index, &rhs_coefficient) in rhs.iter().enumerate() {
            let rhs_centered = center_lift(rhs_coefficient, modulus);
            let product = lhs_centered
                .checked_mul(rhs_centered)
                .ok_or(PolynomialError::ProductOverflow)?;
            let exponent = lhs_index
                .checked_add(rhs_index)
                .ok_or(PolynomialError::ExponentOverflow)?;
            if exponent >= degree {
                let target = exponent - degree;
                out[target] = out[target]
                    .checked_sub(product)
                    .ok_or(PolynomialError::SubtractionOverflow)?;
            } else {
                out[exponent] = out[exponent]
                    .checked_add(product)
                    .ok_or(PolynomialError::AdditionOverflow)?;
            }
        }
    }
    Ok(std::mem::take(&mut *out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficientwise_operations_reduce_exactly() {
        assert_eq!(zero(3), [0, 0, 0]);
        assert_eq!(add_mod(&[16, 1, 0], &[1, 16, 0], 17), [0, 0, 0]);
        assert_eq!(sub_mod(&[0, 1, 5], &[1, 16, 5], 17), [16, 2, 0]);
        assert_eq!(neg_mod(&[0, 1, 16, 18], 17), [0, 16, 1, 16]);
        assert_eq!(scalar_mul_mod(&[0, 1, 16], 16, 17), [0, 16, 1]);
        assert_eq!(add_mod(&[1, 2, 3], &[1], 17), [2], "common prefix");
        assert_eq!(sub_mod(&[1], &[1, 2, 3], 17), [0], "common prefix");
        assert_eq!(reduce_raw_mod(&[-1, 17, -18, 0], 17), [16, 0, 16, 0]);
        // Max-width modulus: no intermediate overflow.
        let modulus = u64::MAX;
        assert_eq!(
            add_mod(&[modulus - 1], &[modulus - 2], modulus),
            [modulus - 3]
        );
        assert_eq!(scalar_mul_mod(&[modulus - 1], modulus - 2, modulus), [2]);
    }

    #[test]
    fn raw_sum_is_checked() {
        assert_eq!(add_centered_raw(&[1, -2], &[3, 4]), Ok(vec![4, 2]));
        assert_eq!(
            add_centered_raw(&[1], &[1, 2]),
            Err(PolynomialError::LengthMismatch { lhs: 1, rhs: 2 })
        );
        assert_eq!(
            add_centered_raw(&[i128::MAX], &[1]),
            Err(PolynomialError::AdditionOverflow)
        );
        assert_eq!(
            add_centered_raw(&[i128::MIN], &[-1]),
            Err(PolynomialError::AdditionOverflow)
        );
    }

    #[test]
    fn schoolbook_products_apply_the_negacyclic_sign_rule() {
        // (1 + 2X + 3X^2 + 4X^3)(5 + 6X + 7X^2 + 8X^3) mod X^4 + 1 over the integers:
        // linear product 5, 16, 34, 60, 61, 52, 32; fold: 5-61, 16-52, 34-32, 60.
        let lhs = [1_u64, 2, 3, 4];
        let rhs = [5_u64, 6, 7, 8];
        assert_eq!(
            negacyclic_mul_raw_schoolbook(&lhs, &rhs, 4),
            [-56, -36, 2, 60]
        );
        assert_eq!(
            negacyclic_mul_raw_schoolbook_i128(&[1, 2, 3, 4], &[5, 6, 7, 8], 4),
            [-56, -36, 2, 60]
        );
        assert_eq!(
            negacyclic_mul_mod_schoolbook(&lhs, &rhs, 4, 17),
            [12, 15, 2, 9]
        );
        // X^3 * X = -1.
        assert_eq!(
            negacyclic_mul_mod_schoolbook(&[0, 0, 0, 1], &[0, 1, 0, 0], 4, 17),
            [16, 0, 0, 0]
        );
        assert_eq!(
            negacyclic_mul_raw_schoolbook(&[0, 0, 0, 1], &[0, 1, 0, 0], 4),
            [-1, 0, 0, 0]
        );
        // Signed operands.
        assert_eq!(
            negacyclic_mul_raw_schoolbook_i128(&[-1, 0], &[0, 3], 2),
            [0, -3]
        );
        assert_eq!(
            negacyclic_mul_raw_schoolbook_i128(&[0, -1], &[0, 3], 2),
            [3, 0]
        );
        // Unreduced operands and a zero modulus stay total.
        assert_eq!(
            negacyclic_mul_mod_schoolbook(&[18, 2 + 17], &[5 + 34, 6], 2, 17),
            negacyclic_mul_mod_schoolbook(&[1, 2], &[5, 6], 2, 17)
        );
        assert_eq!(
            negacyclic_mul_mod_schoolbook(&[1, 2], &[5, 6], 2, 0),
            [0, 0]
        );
        // Shorter operands are padded with zero coefficients; an empty ring has an empty product.
        assert_eq!(
            negacyclic_mul_mod_schoolbook(&[2], &[3, 4], 4, 17),
            [6, 8, 0, 0]
        );
        assert_eq!(
            negacyclic_mul_mod_schoolbook(&[], &[], 0, 17),
            Vec::<u64>::new()
        );
    }

    #[test]
    #[should_panic(expected = "operands have at most `degree` coefficients")]
    fn word_schoolbook_product_panics_on_an_operand_longer_than_the_degree() {
        let _ = negacyclic_mul_mod_schoolbook(&[1, 2, 3], &[1], 2, 17);
    }

    /// The generic kernel returns the same words over word and fixed-modulus arithmetic, equals
    /// the integer schoolbook product reduced, and rejects oversized operands untouched.
    #[test]
    fn generic_schoolbook_product_agrees_across_arithmetics_and_with_the_integers() {
        use crate::constant_time::FixedModulus;
        fn splitmix64(state: &mut u64) -> u64 {
            *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut value = *state;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            value ^ (value >> 31)
        }
        let mut state = 0xB1_u64;
        for (modulus, degree) in [
            (12_289_u64, 64_usize),
            (12_289, 8),
            (30_593, 16),
            (1_125_899_906_840_833, 64),
            ((1 << 63) - 25, 4),
        ] {
            let fixed = FixedModulus::derive(modulus).expect("odd modulus below 2^63");
            let word = WordModulus(modulus);
            let boundary = [0, 1, modulus - 1, modulus / 2, modulus / 2 + 1];
            let operands: [Vec<u64>; 4] = [
                (0..degree)
                    .map(|_| splitmix64(&mut state) % modulus)
                    .collect(),
                (0..degree)
                    .map(|_| splitmix64(&mut state) % modulus)
                    .collect(),
                vec![modulus - 1; degree],
                (0..degree)
                    .map(|index| boundary[index % boundary.len()])
                    .collect(),
            ];
            for lhs in &operands {
                for rhs in &operands {
                    let mut by_word = vec![u64::MAX; degree];
                    let mut by_fixed = vec![u64::MAX; degree];
                    assert_eq!(
                        negacyclic_mul_schoolbook_into_with(lhs, rhs, &word, &mut by_word),
                        Some(())
                    );
                    assert_eq!(
                        negacyclic_mul_schoolbook_into_with(lhs, rhs, &fixed, &mut by_fixed),
                        Some(())
                    );
                    assert_eq!(by_word, by_fixed, "modulus {modulus} degree {degree}");
                    assert_eq!(
                        by_word,
                        negacyclic_mul_mod_schoolbook(lhs, rhs, degree, modulus)
                    );
                    // Independent reference: exact signed fold of the centered lifts, reduced.
                    if modulus < 1 << 51 {
                        let raw = negacyclic_mul_centered_raw(lhs, rhs, degree, modulus)
                            .expect("centered products fit i128");
                        assert_eq!(by_word, reduce_raw_mod(&raw, modulus));
                    }
                }
            }
        }
        // X^(n-1) * X = -1 over both arithmetics.
        let fixed = FixedModulus::derive(12_289).unwrap();
        let mut product = [7_u64; 4];
        assert_eq!(
            negacyclic_mul_schoolbook_into_with(&[0, 0, 0, 1], &[0, 1], &fixed, &mut product),
            Some(())
        );
        assert_eq!(product, [12_288, 0, 0, 0]);
        // Oversized operands are rejected before the output is written.
        let mut untouched = [9_u64; 2];
        assert_eq!(
            negacyclic_mul_schoolbook_into_with(&[1, 2, 3], &[1], &fixed, &mut untouched),
            None
        );
        assert_eq!(
            negacyclic_mul_schoolbook_into_with(&[1], &[1, 2, 3], &WordModulus(17), &mut untouched),
            None
        );
        assert_eq!(untouched, [9, 9]);
        let mut empty: [u64; 0] = [];
        assert_eq!(
            negacyclic_mul_schoolbook_into_with(&[], &[], &fixed, &mut empty),
            Some(())
        );
    }

    #[test]
    fn centered_product_lifts_before_multiplying_and_is_checked() {
        // Modulus 17: 16 is -1 and 9 is -8 (8 is the largest positive lift).
        assert_eq!(
            negacyclic_mul_centered_raw(&[16, 0], &[9, 0], 2, 17),
            Ok(vec![8, 0])
        );
        assert_eq!(
            negacyclic_mul_centered_raw(&[0, 16], &[0, 8], 2, 17),
            Ok(vec![8, 0])
        );
        assert_eq!(
            negacyclic_mul_centered_raw(&[8, 0], &[8, 0], 2, 17),
            Ok(vec![64, 0])
        );
        // Even modulus: the midpoint 8 of 16 stays positive, 9 is -7.
        assert_eq!(
            negacyclic_mul_centered_raw(&[8, 0], &[9, 0], 2, 16),
            Ok(vec![-56, 0])
        );
        // Max-width operands: (2^63 - 1)^2 fits, eight such terms do not.
        let modulus = u64::MAX;
        let half = modulus / 2;
        assert_eq!(
            negacyclic_mul_centered_raw(&[half], &[half], 1, modulus),
            Ok(vec![i128::from(half) * i128::from(half)])
        );
        let wide = vec![half; 8];
        let ones = {
            let mut poly = vec![0_u64; 8];
            poly[0] = half;
            poly
        };
        assert_eq!(
            negacyclic_mul_centered_raw(&wide, &ones, 8, modulus).map(|product| product.len()),
            Ok(8)
        );
        // Coefficient 2 receives three positive terms of (2^63 - 1)^2 > i128::MAX / 3.
        assert_eq!(
            negacyclic_mul_centered_raw(&[half; 8], &[half; 8], 8, modulus),
            Err(PolynomialError::AdditionOverflow)
        );
        // Only wrapped (negative) terms: X^5..X^7 squared lands three terms on coefficient 4.
        let top = [0, 0, 0, 0, 0, half, half, half];
        assert_eq!(
            negacyclic_mul_centered_raw(&top, &top, 8, modulus),
            Err(PolynomialError::SubtractionOverflow)
        );
    }
}
