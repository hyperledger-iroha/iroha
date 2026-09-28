//! SCCP v1 amount rules (spec §0, §3.2).
//!
//! Taira XOR amounts are `Numeric` values of scale at most 9. Every destination token has 9
//! decimals, so one token unit equals one Taira unit and no scaling happens anywhere:
//! `taira_units(q) = mantissa(q) × 10^(9 − scale(q))`, which MUST be an exact integer with
//! `0 < taira_units(q) < 2^128`. Amounts moving to or from TON are further bounded by `2^96`.

use iroha_data_model::{bridge::SccpNetworkV1, prelude::Numeric};

use super::constants::{TON_AMOUNT_BOUND, XOR_DECIMALS};

unit_error! {
    /// Errors of the amount conversions and bounds.
    pub enum AmountError {
        /// The amount is zero.
        Zero => "SCCP amount must be nonzero",
        /// The amount is negative.
        Negative => "SCCP amount must not be negative",
        /// The canonical scale exceeds 9, so the value is not a whole number of Taira units.
        ScaleTooLarge => "SCCP amount has more than 9 fractional digits",
        /// The amount is `2^128` Taira units or more.
        TooLarge => "SCCP amount must be below 2^128 Taira units",
        /// The amount is `2^96` units or more on a TON lane.
        TonBoundExceeded => "SCCP amount must be below 2^96 units when TON is an endpoint",
    }
}

/// `taira_units(q)`: the exact number of Taira units (10^-9 XOR) in `q`.
///
/// The value is canonicalized first (trailing fractional zeros removed), so `1.000000000000`
/// converts like `1`.
///
/// # Errors
///
/// Returns [`AmountError`] when `q` is zero, negative, has more than 9 significant fractional
/// digits, or is `2^128` units or more.
pub fn taira_units(q: &Numeric) -> Result<u128, AmountError> {
    let canonical = q.clone().trim_trailing_zeros();
    if canonical.mantissa().is_negative() {
        return Err(AmountError::Negative);
    }
    if canonical.mantissa().is_zero() {
        return Err(AmountError::Zero);
    }
    let scale = canonical.scale();
    if scale > XOR_DECIMALS {
        return Err(AmountError::ScaleTooLarge);
    }
    let mantissa = canonical.try_mantissa_u128().ok_or(AmountError::TooLarge)?;
    let factor = 10_u128.pow(XOR_DECIMALS - scale);
    mantissa.checked_mul(factor).ok_or(AmountError::TooLarge)
}

/// The canonical XOR `Numeric` of `units` Taira units (the inverse of [`taira_units`]).
///
/// # Errors
///
/// Returns [`AmountError::Zero`] for zero units.
pub fn numeric_from_units(units: u128) -> Result<Numeric, AmountError> {
    if units == 0 {
        return Err(AmountError::Zero);
    }
    Numeric::try_new(units, XOR_DECIMALS).map_err(|_| AmountError::TooLarge)
}

/// Check the lane-specific destination bound: nonzero, and `< 2^96` when TON is an endpoint.
///
/// # Errors
///
/// Returns [`AmountError::Zero`] or [`AmountError::TonBoundExceeded`].
pub fn check_lane_amount(
    units: u128,
    source: SccpNetworkV1,
    target: SccpNetworkV1,
) -> Result<(), AmountError> {
    if units == 0 {
        return Err(AmountError::Zero);
    }
    let ton =
        matches!(source, SccpNetworkV1::TonMainnet) || matches!(target, SccpNetworkV1::TonMainnet);
    if ton && units >= TON_AMOUNT_BOUND {
        return Err(AmountError::TonBoundExceeded);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric(mantissa: i128, scale: u32) -> Numeric {
        Numeric::try_new(mantissa, scale).expect("numeric")
    }

    #[test]
    fn taira_units_scales_exactly() {
        assert_eq!(taira_units(&numeric(1, 0)), Ok(1_000_000_000));
        assert_eq!(taira_units(&numeric(1, 9)), Ok(1));
        assert_eq!(taira_units(&numeric(15, 1)), Ok(1_500_000_000));
        assert_eq!(taira_units(&numeric(123_456_789, 9)), Ok(123_456_789));
        // Trailing zeros beyond scale 9 are canonicalized away.
        assert_eq!(taira_units(&numeric(1_000, 12)), Ok(1));
    }

    #[test]
    fn taira_units_rejects_inexact_and_out_of_range() {
        assert_eq!(
            taira_units(&numeric(1, 10)),
            Err(AmountError::ScaleTooLarge)
        );
        assert_eq!(taira_units(&numeric(0, 0)), Err(AmountError::Zero));
        assert_eq!(taira_units(&numeric(-1, 0)), Err(AmountError::Negative));
        // (2^128 - 1) units is the maximum; one more unit overflows.
        let max_units = u128::MAX;
        assert_eq!(
            taira_units(&Numeric::try_new(max_units, 9).expect("numeric")),
            Ok(max_units)
        );
        let too_large = Numeric::try_new(max_units, 0).expect("numeric");
        assert_eq!(taira_units(&too_large), Err(AmountError::TooLarge));
    }

    #[test]
    fn numeric_from_units_roundtrips() {
        for units in [1_u128, 9, 10, 1_000_000_000, 1_500_000_000, u128::MAX] {
            let value = numeric_from_units(units).expect("numeric");
            assert_eq!(taira_units(&value), Ok(units));
        }
        let one = numeric_from_units(1_000_000_000).unwrap();
        assert_eq!(one.scale(), 0);
        assert_eq!(numeric_from_units(0), Err(AmountError::Zero));
    }

    #[test]
    fn lane_amount_bounds() {
        let taira = SccpNetworkV1::SoraTaira;
        let ton = SccpNetworkV1::TonMainnet;
        let eth = SccpNetworkV1::EthereumMainnet;
        assert_eq!(check_lane_amount(0, taira, eth), Err(AmountError::Zero));
        assert_eq!(check_lane_amount(TON_AMOUNT_BOUND, taira, eth), Ok(()));
        assert_eq!(check_lane_amount(u128::MAX, eth, taira), Ok(()));
        assert_eq!(check_lane_amount(TON_AMOUNT_BOUND - 1, taira, ton), Ok(()));
        assert_eq!(
            check_lane_amount(TON_AMOUNT_BOUND, taira, ton),
            Err(AmountError::TonBoundExceeded)
        );
        assert_eq!(
            check_lane_amount(TON_AMOUNT_BOUND, ton, taira),
            Err(AmountError::TonBoundExceeded)
        );
    }
}
