// Exact integer-conversion helper and boundary tests for sandbox balances.
fn numeric_to_u64(n: &Numeric) -> Result<u64, iroha_primitives::TryFromNumericError> {
    let mantissa = n
        .try_mantissa_u128()
        .ok_or(iroha_primitives::TryFromNumericError)?;
    if n.scale() == 0 {
        return mantissa
            .try_into()
            .map_err(|_| iroha_primitives::TryFromNumericError);
    }
    let scale = 10u128
        .checked_pow(n.scale())
        .ok_or(iroha_primitives::TryFromNumericError)?;
    if mantissa % scale != 0 {
        return Err(iroha_primitives::TryFromNumericError);
    }
    mantissa
        .checked_div(scale)
        .ok_or(iroha_primitives::TryFromNumericError)?
        .try_into()
        .map_err(|_| iroha_primitives::TryFromNumericError)
}
mod numeric_to_u64_tests {
    use iroha_primitives::numeric::Numeric;
    use super::numeric_to_u64;
    #[test]
    fn accepts_scaled_whole_numbers() {
        let scaled = Numeric::try_new(120_i32, 1).expect("numeric");
        assert_eq!(numeric_to_u64(&scaled).unwrap(), 12);
    }
    #[test]
    fn rejects_fractional_balances() {
        let fractional = Numeric::try_new(1_i32, 1).expect("numeric");
        assert!(numeric_to_u64(&fractional).is_err());
    }
    #[test]
    fn rejects_values_outside_u64_range() {
        // Any value that cannot be represented as u64 should error.
        let large = Numeric::try_new(i128::MAX, 0).expect("numeric");
        assert!(numeric_to_u64(&large).is_err());
        let overflowing = Numeric::try_new(i128::MIN, 0).expect("numeric");
        assert!(numeric_to_u64(&overflowing).is_err());
    }
}
