//! Cross-SDK golden verification for Kotodama V1 numeric frames and envelopes.

#[allow(dead_code)]
#[path = "../examples/numeric_v1_fixture.rs"]
mod fixture_generator;

use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, NumericError},
    numeric_abi::{DecimalValueV1, IntValueV1, NumericAbiError, QuantityValueV1},
};
use ivm::{VMError, numeric::PointerAbiFaultV1, numeric_tlv};

#[test]
fn checked_in_fixture_is_generated_by_the_current_rust_wire_implementation() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/numeric_v1_golden.json");
    let actual = std::fs::read_to_string(&path).expect("read shared numeric fixture");
    assert_eq!(actual, fixture_generator::render_fixture());
}

#[test]
fn every_valid_and_adversarial_vector_has_the_pinned_rust_outcome() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/numeric_v1_golden.json");
    let text = std::fs::read_to_string(path).expect("read shared numeric fixture");
    let document = norito::json::parse_value(&text).expect("parse shared numeric fixture");

    for vector in document
        .get("valid")
        .and_then(norito::json::Value::as_array)
        .expect("valid vectors")
    {
        let kind = string(vector, "kind");
        let canonical = string(vector, "canonical");
        let frame = hex::decode(string(vector, "frame_hex")).expect("valid frame hex");
        let envelope = hex::decode(string(vector, "envelope_hex")).expect("valid envelope hex");
        let decoded = match kind {
            "int" => {
                assert_eq!(
                    IntValueV1::decode_frame(&frame)
                        .expect("decode valid int frame")
                        .as_int()
                        .to_string(),
                    canonical
                );
                numeric_tlv::decode_int_bytes(&envelope)
                    .expect("decode valid int envelope")
                    .to_string()
            }
            "decimal" => {
                assert_eq!(
                    DecimalValueV1::decode_frame(&frame)
                        .expect("decode valid decimal frame")
                        .as_numeric()
                        .to_string(),
                    canonical
                );
                numeric_tlv::decode_decimal_bytes(&envelope)
                    .expect("decode valid decimal envelope")
                    .to_string()
            }
            "quantity" => {
                assert_eq!(
                    QuantityValueV1::decode_frame(&frame)
                        .expect("decode valid quantity frame")
                        .as_quantity()
                        .to_string(),
                    canonical
                );
                numeric_tlv::decode_quantity_bytes(&envelope)
                    .expect("decode valid quantity envelope")
                    .to_string()
            }
            other => panic!("unknown numeric fixture kind {other}"),
        };
        assert_eq!(decoded, canonical, "{}", string(vector, "id"));
    }

    for vector in document
        .get("invalid")
        .and_then(norito::json::Value::as_array)
        .expect("invalid vectors")
    {
        let bytes = hex::decode(string(vector, "hex")).expect("invalid vector hex");
        let actual = match (string(vector, "input"), string(vector, "decode_as")) {
            ("frame", "int") => IntValueV1::decode_frame(&bytes)
                .expect_err("invalid int frame")
                .pipe(frame_error_category),
            ("frame", "decimal") => DecimalValueV1::decode_frame(&bytes)
                .expect_err("invalid decimal frame")
                .pipe(frame_error_category),
            ("frame", "quantity") => QuantityValueV1::decode_frame(&bytes)
                .expect_err("invalid quantity frame")
                .pipe(frame_error_category),
            ("envelope", "int") => numeric_tlv::decode_int_bytes(&bytes)
                .expect_err("invalid int envelope")
                .pipe(envelope_error_category),
            ("envelope", "decimal") => numeric_tlv::decode_decimal_bytes(&bytes)
                .expect_err("invalid decimal envelope")
                .pipe(envelope_error_category),
            ("envelope", "quantity") => numeric_tlv::decode_quantity_bytes(&bytes)
                .expect_err("invalid quantity envelope")
                .pipe(envelope_error_category),
            other => panic!("unknown fixture decoder {other:?}"),
        };
        assert_eq!(
            actual,
            string(vector, "expected"),
            "{}",
            string(vector, "id")
        );
    }

    for vector in document
        .get("invalid_text")
        .and_then(norito::json::Value::as_array)
        .expect("invalid text vectors")
    {
        let input = vector.get("input").expect("invalid text input");
        let actual = numeric_text_error_category(string(vector, "kind"), input)
            .expect_err("invalid text vector must fail");
        assert_eq!(
            actual,
            string(vector, "expected"),
            "{}",
            string(vector, "id")
        );
    }
}

fn canonical_integer_syntax(source: &str) -> bool {
    let magnitude = source.strip_prefix('-').unwrap_or(source);
    !magnitude.is_empty()
        && magnitude.bytes().all(|byte| byte.is_ascii_digit())
        && (magnitude == "0" || !magnitude.starts_with('0'))
        && source != "-0"
}

fn canonical_scaled_syntax(source: &str) -> bool {
    let unsigned = source.strip_prefix('-').unwrap_or(source);
    let mut parts = unsigned.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (whole != "0" && whole.starts_with('0'))
        || fraction.is_some_and(|digits| {
            digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return false;
    }
    source != "-0" && source != "-0.0"
}

fn numeric_text_error_category(
    kind: &str,
    input: &norito::json::Value,
) -> Result<(), &'static str> {
    let source = input.as_str().ok_or("invalid_text")?;
    match kind {
        "int" => {
            if !canonical_integer_syntax(source) {
                return Err("invalid_text");
            }
            let value = source.parse::<BigInt>().map_err(|_| "mantissa_overflow")?;
            IntValueV1::try_new(value)
                .map(|_| ())
                .map_err(|error| match error {
                    NumericAbiError::MantissaOverflow => "mantissa_overflow",
                    other => panic!("unexpected int text error: {other:?}"),
                })
        }
        "decimal" | "quantity" => {
            if !canonical_scaled_syntax(source) {
                return Err("invalid_text");
            }
            let value = source.parse::<Numeric>().map_err(|error| match error {
                NumericError::MantissaTooLarge => "mantissa_overflow",
                NumericError::ScaleTooLarge => "invalid_scale",
                NumericError::Malformed => "invalid_text",
            })?;
            if value.to_string() != source {
                return Err("invalid_text");
            }
            if kind == "quantity" && value.mantissa().is_negative() {
                return Err("negative_quantity");
            }
            Ok(())
        }
        other => panic!("unknown invalid-text kind {other}"),
    }
}

fn string<'a>(value: &'a norito::json::Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("fixture field {key} must be a string"))
}

fn frame_error_category(error: NumericAbiError) -> &'static str {
    match error {
        NumericAbiError::FrameTooShort => "frame_too_short",
        NumericAbiError::FrameTooLarge => "frame_too_large",
        NumericAbiError::InvalidHeader => "invalid_header",
        NumericAbiError::LengthMismatch => "length_mismatch",
        NumericAbiError::NonCanonicalMantissa => "noncanonical_mantissa",
        NumericAbiError::NonCanonicalDecimal => "noncanonical_decimal",
        NumericAbiError::InvalidScale => "invalid_scale",
        NumericAbiError::NegativeQuantity => "negative_quantity",
        NumericAbiError::SchemaMismatch => "schema_mismatch",
        NumericAbiError::CompressionNotAllowed => "compression_not_allowed",
        NumericAbiError::LayoutFlagsNotAllowed => "layout_flags_not_allowed",
        NumericAbiError::Norito(message) if message.contains("checksum") => "checksum_mismatch",
        other => panic!("unpinned numeric frame error: {other:?}"),
    }
}

fn envelope_error_category(error: VMError) -> &'static str {
    match error {
        VMError::PointerAbiFault(PointerAbiFaultV1::PayloadHashMismatch) => "payload_hash_mismatch",
        VMError::PointerAbiFault(PointerAbiFaultV1::WrongType) => "wrong_type",
        VMError::PointerAbiFault(PointerAbiFaultV1::TypeNotAllowed) => "type_not_allowed",
        VMError::PointerAbiFault(PointerAbiFaultV1::UnknownType) => "unknown_type",
        VMError::PointerAbiFault(PointerAbiFaultV1::InvalidEnvelopeVersion) => {
            "invalid_envelope_version"
        }
        VMError::PointerAbiFault(PointerAbiFaultV1::OversizedLength) => "oversized_length",
        VMError::PointerAbiFault(PointerAbiFaultV1::TruncatedEnvelope) => "truncated_envelope",
        other => panic!("unpinned numeric envelope error: {other:?}"),
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, function: impl FnOnce(Self) -> T) -> T {
        function(self)
    }
}

impl<T> Pipe for T {}
