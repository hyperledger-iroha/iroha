//! Ring automorphisms `X -> X^k` of `Z_m[X] / (X^n + 1)`.
//!
//! For `k` coprime to `2n`, coefficient `i` moves to exponent `i * k mod 2n`;
//! an exponent `e >= n` lands on `e - n` with its sign flipped because
//! `X^n = -1`. The map is a signed permutation of coefficients.
use crate::modular::{ModularArithmetic, WordModulus, gcd_u64};
use thiserror::Error;

/// Failure of an automorphism kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum AutomorphismError {
    /// `2 * degree` does not fit the index type.
    #[error("cyclotomic order exceeds the supported range")]
    OrderOverflow,
    /// The power does not fit `usize`.
    #[error("automorphism power exceeds platform usize")]
    PowerExceedsUsize,
    /// The power is zero or not below `2 * degree`.
    #[error("automorphism power must be in 1..{cyclotomic_order}")]
    PowerOutOfRange {
        /// `2 * degree`.
        cyclotomic_order: usize,
    },
    /// The power shares a factor with `2 * degree`.
    #[error("automorphism power must be coprime to 2 * degree")]
    PowerNotCoprime,
    /// An exponent product does not fit `usize`.
    #[error("automorphism exponent exceeds the supported range")]
    ExponentOverflow,
    /// The input has more coefficients than the output.
    #[error("automorphism input is longer than the ring degree")]
    InputTooLong,
    /// A power does not fit `u32`.
    #[error("automorphism power exceeds u32")]
    PowerExceedsU32,
}

/// Validate an automorphism power for a ring of `degree` coefficients.
///
/// # Errors
/// The [`AutomorphismError`] naming the first violated rule: the order
/// `2 * degree` must fit, and the power must fit `usize`, lie in
/// `1..2 * degree` and be coprime to `2 * degree`.
pub fn validate_power(degree: usize, automorphism_power: u32) -> Result<usize, AutomorphismError> {
    let cyclotomic_order = degree
        .checked_mul(2)
        .ok_or(AutomorphismError::OrderOverflow)?;
    let power =
        usize::try_from(automorphism_power).map_err(|_| AutomorphismError::PowerExceedsUsize)?;
    if power == 0 || power >= cyclotomic_order {
        return Err(AutomorphismError::PowerOutOfRange { cyclotomic_order });
    }
    let cyclotomic_order_u64 =
        u64::try_from(cyclotomic_order).map_err(|_| AutomorphismError::OrderOverflow)?;
    if gcd_u64(u64::from(automorphism_power), cyclotomic_order_u64) != 1 {
        return Err(AutomorphismError::PowerNotCoprime);
    }
    Ok(power)
}

/// Every automorphism power of a ring of `degree` coefficients, ascending.
///
/// These are the odd integers in `1..2 * degree` coprime to `2 * degree`.
///
/// # Errors
/// [`AutomorphismError::OrderOverflow`] or [`AutomorphismError::PowerExceedsU32`].
pub fn unit_powers(degree: usize) -> Result<Vec<u32>, AutomorphismError> {
    let cyclotomic_order = degree
        .checked_mul(2)
        .ok_or(AutomorphismError::OrderOverflow)?;
    let cyclotomic_order_u64 =
        u64::try_from(cyclotomic_order).map_err(|_| AutomorphismError::OrderOverflow)?;
    let mut powers = Vec::new();
    for power in (1..cyclotomic_order).step_by(2) {
        let power_u64 = u64::try_from(power).map_err(|_| AutomorphismError::OrderOverflow)?;
        if gcd_u64(power_u64, cyclotomic_order_u64) == 1 {
            powers.push(u32::try_from(power).map_err(|_| AutomorphismError::PowerExceedsU32)?);
        }
    }
    Ok(powers)
}

/// Write the image of `poly` under `X -> X^power` into `output` over any modular arithmetic.
///
/// The ring degree is `output.len()`. `output` is overwritten; the caller owns
/// its clearing, which lets secret polynomials stay in a clearing buffer. The
/// index schedule depends only on the length and the power, so the map is
/// constant-time in the coefficients whenever the arithmetic is. `power` must
/// already be validated with [`validate_power`].
///
/// # Errors
/// [`AutomorphismError::InputTooLong`], [`AutomorphismError::OrderOverflow`] or
/// [`AutomorphismError::ExponentOverflow`].
pub fn apply_into_with<A: ModularArithmetic>(
    poly: &[u64],
    power: usize,
    arithmetic: &A,
    output: &mut [u64],
) -> Result<(), AutomorphismError> {
    let degree = output.len();
    if poly.len() > degree {
        return Err(AutomorphismError::InputTooLong);
    }
    let cyclotomic_order = degree
        .checked_mul(2)
        .ok_or(AutomorphismError::OrderOverflow)?;
    output.fill(0);
    for (index, &coefficient) in poly.iter().enumerate() {
        let exponent = index
            .checked_mul(power)
            .map(|value| value % cyclotomic_order)
            .ok_or(AutomorphismError::ExponentOverflow)?;
        if exponent >= degree {
            let target_index = exponent - degree;
            output[target_index] = arithmetic.sub(output[target_index], coefficient);
        } else {
            output[exponent] = arithmetic.add(output[exponent], coefficient);
        }
    }
    Ok(())
}

/// Write the image of `poly` under `X -> X^power` into `output` modulo a word modulus.
///
/// See [`apply_into_with`]; unreduced coefficients are reduced exactly.
///
/// # Errors
/// As [`apply_into_with`].
pub fn apply_into(
    poly: &[u64],
    power: usize,
    modulus: u64,
    output: &mut [u64],
) -> Result<(), AutomorphismError> {
    apply_into_with(poly, power, &WordModulus(modulus), output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Evaluate a polynomial of `Z_m[X]/(X^n + 1)` at `X^power` by substitution and reduction.
    fn substitute(poly: &[u64], power: usize, modulus: u64) -> Vec<u64> {
        let degree = poly.len();
        let mut output = vec![0_i128; degree];
        for (index, &coefficient) in poly.iter().enumerate() {
            let exponent = index * power;
            let sign = if (exponent / degree).is_multiple_of(2) {
                1
            } else {
                -1
            };
            output[exponent % degree] += sign * i128::from(coefficient);
        }
        output
            .into_iter()
            .map(|value| u64::try_from(value.rem_euclid(i128::from(modulus))).unwrap())
            .collect()
    }

    #[test]
    fn power_validation_reports_each_rule() {
        assert_eq!(validate_power(64, 3), Ok(3));
        assert_eq!(validate_power(64, 127), Ok(127));
        assert_eq!(
            validate_power(64, 0),
            Err(AutomorphismError::PowerOutOfRange {
                cyclotomic_order: 128
            })
        );
        assert_eq!(
            validate_power(64, 128),
            Err(AutomorphismError::PowerOutOfRange {
                cyclotomic_order: 128
            })
        );
        assert_eq!(
            validate_power(64, 2),
            Err(AutomorphismError::PowerNotCoprime)
        );
        assert_eq!(
            validate_power(usize::MAX, 3),
            Err(AutomorphismError::OrderOverflow)
        );
        assert_eq!(
            validate_power(0, 1),
            Err(AutomorphismError::PowerOutOfRange {
                cyclotomic_order: 0
            })
        );
        // A degree that is not a power of two has odd non-units.
        assert_eq!(
            validate_power(3, 3),
            Err(AutomorphismError::PowerNotCoprime)
        );
        assert_eq!(validate_power(3, 5), Ok(5));
    }

    #[test]
    fn unit_powers_are_the_odd_units_in_ascending_order() {
        assert_eq!(unit_powers(4), Ok(vec![1, 3, 5, 7]));
        assert_eq!(unit_powers(3), Ok(vec![1, 5]));
        assert_eq!(unit_powers(0), Ok(Vec::new()));
        assert_eq!(unit_powers(64).map(|powers| powers.len()), Ok(64));
        assert_eq!(
            unit_powers(usize::MAX),
            Err(AutomorphismError::OrderOverflow)
        );
    }

    #[test]
    fn application_matches_substitution_for_every_unit_power() {
        let modulus = 269_484_032_u64;
        let poly: Vec<u64> = (0..8_u64)
            .map(|index| (index * 37_219_771 + 5) % modulus)
            .collect();
        for power in unit_powers(8).unwrap() {
            let power = validate_power(8, power).unwrap();
            let mut output = vec![u64::MAX; 8];
            apply_into(&poly, power, modulus, &mut output).expect("apply");
            assert_eq!(output, substitute(&poly, power, modulus), "power {power}");
        }
        // The identity power leaves the polynomial unchanged.
        let mut identity = vec![0_u64; 8];
        apply_into(&poly, 1, modulus, &mut identity).unwrap();
        assert_eq!(identity, poly);
        // X -> X^-1 (power 2n - 1): coefficient 0 stays, coefficient i becomes -c[n - i].
        let mut inverse = vec![0_u64; 8];
        apply_into(&poly, 15, modulus, &mut inverse).unwrap();
        assert_eq!(inverse[0], poly[0]);
        for index in 1..8 {
            assert_eq!(inverse[index], (modulus - poly[8 - index]) % modulus);
        }
    }

    #[test]
    fn application_rejects_oversized_input_and_overflow() {
        let mut output = [0_u64; 2];
        assert_eq!(
            apply_into(&[1, 2, 3], 1, 17, &mut output),
            Err(AutomorphismError::InputTooLong)
        );
        let mut wide_output = [0_u64; 4];
        assert_eq!(
            apply_into(&[1, 2, 3], usize::MAX, 17, &mut wide_output),
            Err(AutomorphismError::ExponentOverflow)
        );
        // The branch-free Montgomery arithmetic gives the same signed permutation.
        let fixed = crate::constant_time::FixedModulus::derive(1_125_899_906_843_221).unwrap();
        let secret = [5_u64, 1_125_899_906_843_220, 0, 7];
        let (mut by_fixed, mut by_word) = ([0_u64; 4], [0_u64; 4]);
        assert_eq!(apply_into_with(&secret, 7, &fixed, &mut by_fixed), Ok(()));
        assert_eq!(apply_into(&secret, 7, fixed.modulus, &mut by_word), Ok(()));
        assert_eq!(by_fixed, by_word);
        assert_eq!(by_fixed, [5, 1_125_899_906_843_214, 0, 1]);
        // A shorter input is zero-extended.
        let mut output = [9_u64; 4];
        assert_eq!(apply_into(&[1, 2], 3, 17, &mut output), Ok(()));
        assert_eq!(output, [1, 0, 0, 2]);
    }
}
