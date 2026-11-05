//! Exact rendering and source contracts for address and numeric errors.

use std::{error::Error, fmt, fmt::Write as _};

use crate::{
    BigIntError, DecimalProductError, NumericAbiError, NumericError, NumericOperationError,
    NumericSpecError, NumericSpecParseError, TryFromNumericError, addr::ParseError,
};

fn assert_display_contract(error: &impl Error, expected: &str) {
    assert_eq!(error.to_string(), expected);
    assert_eq!(format!("{error:>120.3}"), expected);
    assert_eq!(format!("{error:🦀^120.3}"), expected);
    assert_eq!(format!("{error:+#0120.3}"), expected);

    struct LimitedWriter(usize);
    impl fmt::Write for LimitedWriter {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            self.0 = self.0.checked_sub(text.len()).ok_or(fmt::Error)?;
            Ok(())
        }
    }
    // Include failures after a prefix has already been written, so nested
    // payload formatting must also propagate its writer's error.
    for limit in [0, expected.len() / 2, expected.len() - 1] {
        assert!(write!(LimitedWriter(limit), "{error}").is_err());
    }
}

fn assert_without_source(error: &impl Error, expected: &str) {
    assert_display_contract(error, expected);
    assert!(error.source().is_none());
}

#[test]
fn address_error_display_preserves_all_messages() {
    for (error, expected) in [
        (
            ParseError::NotEnoughSegments,
            "Not enough segments in IP address",
        ),
        (
            ParseError::TooManySegments,
            "Too many segments in IP address",
        ),
        (
            ParseError::InvalidSegment,
            "Failed to parse segment in IP address",
        ),
        (
            ParseError::InvalidPort,
            "Failed to parse port number in socket address",
        ),
        (
            ParseError::NoPort,
            "Failed to find port number in socket address",
        ),
        (
            ParseError::UnexpectedAbbreviation,
            "Ipv6 address contains more than one '::' abbreviation",
        ),
    ] {
        assert_without_source(&error, expected);
    }
}

#[test]
fn bigint_error_display_preserves_all_messages() {
    for (error, expected) in [
        (BigIntError::Overflow, "Value exceeds configured bit cap"),
        (
            BigIntError::NonCanonical,
            "Two's-complement byte representation is not minimal",
        ),
        (BigIntError::DivisionByZero, "Division by zero"),
    ] {
        assert_without_source(&error, expected);
    }
}

#[test]
fn numeric_error_display_preserves_all_messages() {
    for (error, expected) in [
        (
            NumericError::MantissaTooLarge,
            "Mantissa exceeds allowed range",
        ),
        (NumericError::ScaleTooLarge, "Scale exceeds allowed range"),
        (
            NumericError::Malformed,
            "Malformed: expecting number with optional decimal point (10, 10.02)",
        ),
    ] {
        assert_without_source(&error, expected);
    }
}

#[test]
fn numeric_operation_error_display_preserves_all_messages_and_wrappers() {
    for (error, expected) in [
        (
            NumericOperationError::MantissaOverflow,
            "Canonical result mantissa is outside `-2^511..=2^511-1`",
        ),
        (
            NumericOperationError::ScaleOverflow,
            "Canonical exact result needs a scale greater than 28",
        ),
        (NumericOperationError::DivisionByZero, "Divisor is zero"),
        (
            NumericOperationError::RepeatingDecimal,
            "Exact quotient is a repeating decimal",
        ),
        (
            NumericOperationError::ExactDivisionScaleOverflow,
            "Exact terminating quotient needs more than 28 fractional digits",
        ),
        (
            NumericOperationError::InvalidScale,
            "Requested output scale is outside `0..=28`",
        ),
        (
            NumericOperationError::InexactConversion,
            "Conversion would discard a nonzero fractional part",
        ),
        (
            NumericOperationError::NonCanonical,
            "Decimal is not in its unique canonical representation",
        ),
        (
            NumericOperationError::NegativeQuantity,
            "Quantity cannot be negative",
        ),
        (
            NumericOperationError::QuantityUnderflow,
            "Quantity subtraction would produce a negative result",
        ),
        (
            NumericOperationError::NegativeSquareRoot,
            "Integer square root received a negative operand",
        ),
    ] {
        assert_without_source(&error, expected);
        let wrapped: DecimalProductError = error.into();
        assert_without_source(&wrapped, expected);
        assert!(matches!(wrapped, DecimalProductError::Numeric(actual) if actual == error));
    }
}

#[test]
fn numeric_conversion_error_display_preserves_the_struct_message() {
    assert_without_source(
        &TryFromNumericError,
        "The error type returned when a numeric conversion fails.",
    );
}

#[test]
fn numeric_spec_error_display_preserves_the_message() {
    assert_without_source(
        &NumericSpecError::ScaleTooHigh,
        "Given number has scale higher than allowed by spec.",
    );
}

#[test]
fn numeric_spec_parse_error_display_preserves_messages_and_original_source() {
    assert_without_source(
        &NumericSpecParseError::StartWithNumeric,
        "String representation should start with Numeric",
    );
    assert_without_source(
        &NumericSpecParseError::WrappedInBraces,
        "Numeric should be followed by optional scale wrapped in braces",
    );
    for (text, expected) in [
        (
            "",
            "Scale should be valid integer value: cannot parse integer from empty string",
        ),
        (
            "μ",
            "Scale should be valid integer value: invalid digit found in string",
        ),
        (
            "4294967296",
            "Scale should be valid integer value: number too large to fit in target type",
        ),
    ] {
        let error = NumericSpecParseError::InvalidScale(text.parse::<u32>().unwrap_err());
        assert_display_contract(&error, expected);
        let NumericSpecParseError::InvalidScale(original) = &error else {
            unreachable!("constructed InvalidScale");
        };
        let source = error
            .source()
            .unwrap()
            .downcast_ref::<std::num::ParseIntError>()
            .unwrap();
        assert!(std::ptr::eq(source, original));
        assert!(source.source().is_none());
    }
}

#[test]
fn numeric_abi_error_display_preserves_all_messages_and_string_payloads() {
    for (error, expected) in [
        (
            NumericAbiError::FrameTooShort,
            "Frame is shorter than the fixed Norito header",
        ),
        (
            NumericAbiError::FrameTooLarge,
            "Frame exceeds the maximum size for its declared numeric type",
        ),
        (
            NumericAbiError::InvalidHeader,
            "Frame does not use the canonical Norito V1 header",
        ),
        (
            NumericAbiError::SchemaMismatch,
            "Frame schema hash does not match the nominal numeric type",
        ),
        (
            NumericAbiError::CompressionNotAllowed,
            "Numeric frames must be uncompressed",
        ),
        (
            NumericAbiError::LayoutFlagsNotAllowed,
            "Numeric frames must use layout flags zero",
        ),
        (
            NumericAbiError::LengthMismatch,
            "Declared payload length is invalid or does not consume the frame exactly",
        ),
        (
            NumericAbiError::MantissaOverflow,
            "Mantissa encoding is outside the signed domain",
        ),
        (
            NumericAbiError::NonCanonicalMantissa,
            "Mantissa encoding is not minimal",
        ),
        (
            NumericAbiError::InvalidScale,
            "Decimal scale is outside `0..=28`",
        ),
        (
            NumericAbiError::NonCanonicalDecimal,
            "Decimal representation contains removable trailing zeroes",
        ),
        (
            NumericAbiError::NegativeQuantity,
            "Quantity mantissa is negative",
        ),
        (
            NumericAbiError::Norito(String::new()),
            "Norito frame validation failed: ",
        ),
        (
            NumericAbiError::Norito("μ\nquote\"\\".into()),
            "Norito frame validation failed: μ\nquote\"\\",
        ),
    ] {
        assert_without_source(&error, expected);
    }
}
