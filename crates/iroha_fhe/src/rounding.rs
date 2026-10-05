//! Explicit integer rounding: centered lifting, nearest division and scale-and-round.
//!
//! No floating point participates anywhere in this crate. The rules are:
//!
//! - **Centered lift** ([`center_lift`]): a residue `x` modulo `m` lifts to `x`
//!   when `x <= floor(m / 2)` and to `x - m` otherwise. The range is
//!   `[-floor((m - 1) / 2), floor(m / 2)]`; for even `m` the midpoint `m / 2`
//!   is positive.
//! - **Nearest division** ([`div_round_nearest_i128`]): `round(v / d)` to the
//!   nearest integer, ties away from zero. Computed as
//!   `sign(v) * floor((|v| + floor(d / 2)) / d)`. A tie exists only for even
//!   `d`.
//! - **Ceiling division** ([`ceil_div_u128`]): `ceil(n / d)`.
//! - **Scale and round** ([`scale_round_centered`]): `round(c * a / b)` to the
//!   nearest integer, ties away from zero, computed as
//!   `sign(c) * floor((|c| * a + floor(b / 2)) / b)`. A tie
//!   (`2 * |c| * a = (2k + 1) * b`) exists only for even `b`.
//! - **Modulus switching** ([`modulus_switch_round`]): centered lift modulo the
//!   source modulus, scale and round by `to / from` with the rule above, then
//!   least non-negative residue modulo the target. It is plain rounding; a
//!   scheme that must also preserve a plaintext residue adds its own
//!   correction term on top.
use crate::modular::reduce_i128_to_u64_mod;
use thiserror::Error;

/// Failure of a rounding kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RoundingError {
    /// A divisor that must be positive is zero or negative.
    #[error("divisor must be positive")]
    NonPositiveDivisor,
    /// A denominator is zero.
    #[error("denominator must be non-zero")]
    ZeroDenominator,
    /// Adding the rounding offset left the integer range.
    #[error("rounding offset exceeds the integer range")]
    OffsetOverflow,
    /// The scaled numerator `|c| * a + floor(b / 2)` does not fit `u128`.
    #[error("scaled numerator exceeds u128")]
    ScalingOverflow,
    /// The rounded quotient does not fit `i128`.
    #[error("rounded quotient exceeds i128")]
    RoundedExceedsI128,
}

/// Centered representative of a residue: `coefficient - modulus` above `floor(modulus / 2)`.
#[must_use]
pub fn center_lift(coefficient: u64, modulus: u64) -> i128 {
    let coefficient = i128::from(coefficient);
    let modulus = i128::from(modulus);
    if coefficient > modulus / 2 {
        coefficient - modulus
    } else {
        coefficient
    }
}

/// `round(value / divisor)` to the nearest integer, ties away from zero.
///
/// # Errors
/// [`RoundingError::NonPositiveDivisor`] or [`RoundingError::OffsetOverflow`].
pub fn div_round_nearest_i128(value: i128, divisor: i128) -> Result<i128, RoundingError> {
    if divisor <= 0 {
        return Err(RoundingError::NonPositiveDivisor);
    }
    let half = divisor / 2;
    if value >= 0 {
        value.checked_add(half).map(|value| value / divisor)
    } else {
        value
            .checked_abs()
            .and_then(|abs| abs.checked_add(half))
            .map(|rounded_abs| -(rounded_abs / divisor))
    }
    .ok_or(RoundingError::OffsetOverflow)
}

/// `ceil(numerator / denominator)`.
///
/// # Errors
/// [`RoundingError::ZeroDenominator`] or [`RoundingError::OffsetOverflow`].
pub fn ceil_div_u128(numerator: u128, denominator: u128) -> Result<u128, RoundingError> {
    if denominator == 0 {
        return Err(RoundingError::ZeroDenominator);
    }
    numerator
        .checked_add(denominator - 1)
        .map(|value| value / denominator)
        .ok_or(RoundingError::OffsetOverflow)
}

/// `round(coefficient * numerator / denominator)` to the nearest integer, ties away from zero.
///
/// # Errors
/// [`RoundingError::ZeroDenominator`], [`RoundingError::ScalingOverflow`] or
/// [`RoundingError::RoundedExceedsI128`].
pub fn scale_round_centered(
    coefficient: i128,
    numerator: u64,
    denominator: u64,
) -> Result<i128, RoundingError> {
    if denominator == 0 {
        return Err(RoundingError::ZeroDenominator);
    }
    let scaled = coefficient
        .unsigned_abs()
        .checked_mul(u128::from(numerator))
        .and_then(|value| value.checked_add(u128::from(denominator / 2)))
        .ok_or(RoundingError::ScalingOverflow)?;
    let rounded = i128::try_from(scaled / u128::from(denominator))
        .map_err(|_| RoundingError::RoundedExceedsI128)?;
    Ok(if coefficient < 0 { -rounded } else { rounded })
}

/// Switch a residue from `from_modulus` to `to_modulus` by rounding `value * to / from`.
///
/// The residue is lifted with [`center_lift`], scaled with
/// [`scale_round_centered`] and reduced to `[0, to_modulus)`.
///
/// # Errors
/// [`RoundingError::ZeroDenominator`] when either modulus is zero, or the
/// errors of [`scale_round_centered`].
pub fn modulus_switch_round(
    value: u64,
    from_modulus: u64,
    to_modulus: u64,
) -> Result<u64, RoundingError> {
    if to_modulus == 0 {
        return Err(RoundingError::ZeroDenominator);
    }
    let rounded = scale_round_centered(center_lift(value, from_modulus), to_modulus, from_modulus)?;
    Ok(reduce_i128_to_u64_mod(rounded, to_modulus))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent statement of "nearest, ties away from zero": the unique integer `r` with
    /// `|c * a - r * b| * 2 <= b`, preferring the larger magnitude on equality.
    fn reference_round(coefficient: i128, numerator: i128, denominator: i128) -> i128 {
        let exact = coefficient * numerator;
        let floor = exact.div_euclid(denominator);
        let remainder = exact.rem_euclid(denominator);
        match (2 * remainder).cmp(&denominator) {
            core::cmp::Ordering::Less => floor,
            core::cmp::Ordering::Greater => floor + 1,
            // Tie: away from zero.
            core::cmp::Ordering::Equal => {
                if exact >= 0 {
                    floor + 1
                } else {
                    floor
                }
            }
        }
    }

    #[test]
    fn center_lift_keeps_the_even_midpoint_positive() {
        assert_eq!(center_lift(0, 16), 0);
        assert_eq!(center_lift(8, 16), 8, "even midpoint is positive");
        assert_eq!(center_lift(9, 16), -7);
        assert_eq!(center_lift(15, 16), -1);
        assert_eq!(
            center_lift(8, 17),
            8,
            "floor(17 / 2) is the largest positive lift"
        );
        assert_eq!(center_lift(9, 17), -8);
        assert_eq!(center_lift(u64::MAX - 1, u64::MAX), -1);
        assert_eq!(
            center_lift(u64::MAX / 2, u64::MAX),
            i128::from(u64::MAX / 2)
        );
        assert_eq!(
            center_lift(u64::MAX / 2 + 1, u64::MAX),
            -i128::from(u64::MAX / 2)
        );
        assert_eq!(
            center_lift(20, 16),
            4,
            "an unreduced residue above the modulus lifts by one modulus"
        );
    }

    #[test]
    fn nearest_division_rounds_ties_away_from_zero() {
        // Even divisor: exact halves exist.
        for (value, divisor, expected) in [
            (0_i128, 4_i128, 0_i128),
            (1, 4, 0),
            (2, 4, 1),
            (3, 4, 1),
            (5, 4, 1),
            (6, 4, 2),
            (-1, 4, 0),
            (-2, 4, -1),
            (-6, 4, -2),
            (-5, 4, -1),
            // Odd divisor: no tie.
            (7, 5, 1),
            (8, 5, 2),
            (-7, 5, -1),
            (-8, 5, -2),
            (i128::MAX - 1, 1, i128::MAX - 1),
        ] {
            assert_eq!(
                div_round_nearest_i128(value, divisor),
                Ok(expected),
                "{value} / {divisor}"
            );
            assert_eq!(reference_round(value, 1, divisor), expected);
        }
        assert_eq!(
            div_round_nearest_i128(1, 0),
            Err(RoundingError::NonPositiveDivisor)
        );
        assert_eq!(
            div_round_nearest_i128(1, -3),
            Err(RoundingError::NonPositiveDivisor)
        );
        assert_eq!(
            div_round_nearest_i128(i128::MAX, 2),
            Err(RoundingError::OffsetOverflow)
        );
        assert_eq!(
            div_round_nearest_i128(i128::MIN, 2),
            Err(RoundingError::OffsetOverflow)
        );
        assert_eq!(
            div_round_nearest_i128(i128::MIN + 1, 2),
            Err(RoundingError::OffsetOverflow)
        );
        assert_eq!(div_round_nearest_i128(i128::MIN + 1, 1), Ok(i128::MIN + 1));
    }

    #[test]
    fn ceiling_division_is_exact_at_multiples_and_checked() {
        assert_eq!(ceil_div_u128(0, 7), Ok(0));
        assert_eq!(ceil_div_u128(7, 7), Ok(1));
        assert_eq!(ceil_div_u128(8, 7), Ok(2));
        assert_eq!(ceil_div_u128(14, 7), Ok(2));
        assert_eq!(ceil_div_u128(u128::MAX, 1), Ok(u128::MAX));
        assert_eq!(ceil_div_u128(1, 0), Err(RoundingError::ZeroDenominator));
        assert_eq!(
            ceil_div_u128(u128::MAX, 2),
            Err(RoundingError::OffsetOverflow)
        );
    }

    #[test]
    fn scale_and_round_matches_the_independent_rule_at_halves_and_boundaries() {
        // t / q with an even q: 2 * |c| * t = (2k + 1) * q has solutions.
        let (numerator, denominator) = (4_u64, 8_u64);
        for coefficient in -20_i128..=20 {
            assert_eq!(
                scale_round_centered(coefficient, numerator, denominator),
                Ok(reference_round(
                    coefficient,
                    i128::from(numerator),
                    i128::from(denominator)
                )),
                "coefficient {coefficient}"
            );
        }
        // Exact halves: 1 * 4 / 8 = 0.5 -> 1 and -0.5 -> -1; 3 * 4 / 8 = 1.5 -> 2.
        assert_eq!(scale_round_centered(1, 4, 8), Ok(1));
        assert_eq!(scale_round_centered(-1, 4, 8), Ok(-1));
        assert_eq!(scale_round_centered(3, 4, 8), Ok(2));
        assert_eq!(scale_round_centered(-3, 4, 8), Ok(-2));
        // Odd denominator: no tie; just below and just above a half.
        assert_eq!(scale_round_centered(3, 1, 7), Ok(0));
        assert_eq!(scale_round_centered(4, 1, 7), Ok(1));
        assert_eq!(scale_round_centered(-3, 1, 7), Ok(0));
        assert_eq!(scale_round_centered(-4, 1, 7), Ok(-1));
        // The registered RAM-LFE BFV moduli: t = 257, q = 257 * 2^48. Scaling by t / q divides
        // by 2^48, so 2^47 is exactly one half.
        let (t, q) = (257_u64, 257_u64 << 48);
        let half = 1_i128 << 47;
        for coefficient in [
            0_i128,
            1,
            half - 1,
            half,
            half + 1,
            2 * half,
            3 * half,
            -half,
            -half - 1,
            -half + 1,
            i128::from(q) * 3 + half,
            -(i128::from(q) * 3 + half),
        ] {
            assert_eq!(
                scale_round_centered(coefficient, t, q),
                Ok(reference_round(coefficient, i128::from(t), i128::from(q))),
                "coefficient {coefficient}"
            );
        }
        assert_eq!(
            scale_round_centered(half, t, q),
            Ok(1),
            "exactly one half rounds away from zero"
        );
        assert_eq!(scale_round_centered(half - 1, t, q), Ok(0));
        assert_eq!(scale_round_centered(-half, t, q), Ok(-1));
        assert_eq!(scale_round_centered(-half + 1, t, q), Ok(0));
        assert_eq!(scale_round_centered(3 * half, t, q), Ok(2));
        // A small pair of the same shape: t = 257, q = 257 * 2^20, half point 2^19.
        let (t, q) = (257_u64, 269_484_032_u64);
        for coefficient in [
            0_i128,
            1,
            524_287,
            524_288,
            524_289,
            1_048_576,
            -524_288,
            -524_289,
            i128::from(q) * 3 + 524_288,
        ] {
            assert_eq!(
                scale_round_centered(coefficient, t, q),
                Ok(reference_round(coefficient, i128::from(t), i128::from(q)))
            );
        }
        assert_eq!(
            scale_round_centered(524_288, t, q),
            Ok(1),
            "exactly one half rounds away from zero"
        );
        assert_eq!(scale_round_centered(524_287, t, q), Ok(0));
        assert_eq!(scale_round_centered(-524_288, t, q), Ok(-1));
        // Errors.
        assert_eq!(
            scale_round_centered(1, 1, 0),
            Err(RoundingError::ZeroDenominator)
        );
        assert_eq!(
            scale_round_centered(i128::MAX, u64::MAX, 3),
            Err(RoundingError::ScalingOverflow)
        );
        assert_eq!(
            scale_round_centered(i128::MIN, 2, 3),
            Err(RoundingError::ScalingOverflow)
        );
        assert_eq!(
            scale_round_centered(i128::MIN, 1, 1),
            Err(RoundingError::RoundedExceedsI128)
        );
        assert_eq!(scale_round_centered(i128::MAX, 1, 1), Ok(i128::MAX));
        assert_eq!(scale_round_centered(i128::MIN + 1, 1, 1), Ok(i128::MIN + 1));
    }

    #[test]
    fn modulus_switching_rounds_the_centered_lift() {
        // 16 -> 4: the scale is 1/4.
        let expected = [
            (0_u64, 0_u64),
            (1, 0),
            (2, 1), // 0.5 rounds away from zero
            (3, 1),
            (5, 1),
            (6, 2),  // 1.5 rounds to 2
            (8, 2),  // the even midpoint lifts to +8 -> 2
            (9, 2),  // lifts to -7 -> -1.75 -> -2 = 2 mod 4
            (10, 2), // lifts to -6 -> -1.5 -> -2
            (11, 3), // lifts to -5 -> -1.25 -> -1
            (14, 3), // lifts to -2 -> -0.5 -> -1
            (15, 0), // lifts to -1 -> -0.25 -> 0
        ];
        for (value, switched) in expected {
            assert_eq!(
                modulus_switch_round(value, 16, 4),
                Ok(switched),
                "value {value}"
            );
        }
        // Odd moduli and switching up.
        for (from, to) in [
            (35_969_u64, 30_593_u64),
            (30_593, 35_969),
            (269_484_032, 30_593),
            (17, 4_293_918_721),
            // The registered ciphertext modulus 257 * 2^48, down to a limb and to the plaintext.
            (257 << 48, 30_593),
            (257 << 48, 257),
        ] {
            for value in [
                0,
                1,
                from / 2,
                from / 2 + 1,
                from - 1,
                from / 3,
                from / 4 + 1,
            ] {
                let lifted = center_lift(value, from);
                let rounded = reference_round(lifted, i128::from(to), i128::from(from));
                assert_eq!(
                    modulus_switch_round(value, from, to),
                    Ok(u64::try_from(rounded.rem_euclid(i128::from(to))).unwrap()),
                    "value {value} from {from} to {to}"
                );
            }
        }
        assert_eq!(
            modulus_switch_round(1, 0, 4),
            Err(RoundingError::ZeroDenominator)
        );
        assert_eq!(
            modulus_switch_round(1, 4, 0),
            Err(RoundingError::ZeroDenominator)
        );
        // Same modulus is the identity.
        for value in 0..17 {
            assert_eq!(modulus_switch_round(value, 17, 17), Ok(value));
        }
    }
}
