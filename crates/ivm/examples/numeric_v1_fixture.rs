//! Generate the shared cross-SDK Kotodama V1 numeric wire fixture.

use std::{env, fs, path::Path};

use iroha_crypto::Hash;
use iroha_primitives::{
    bigint::BigInt,
    numeric::{MAX_MANTISSA_BITS, MAX_MANTISSA_BYTES, Numeric, Quantity},
    numeric_abi::{
        DECIMAL_SCHEMA_HASH_V1, DecimalValueV1, INT_SCHEMA_HASH_V1, IntValueV1,
        MAX_INT_FRAME_BYTES_V1, NUMERIC_FRAME_HEADER_BYTES_V1, QUANTITY_SCHEMA_HASH_V1,
        QuantityValueV1,
    },
};
use ivm::{PointerType, numeric_tlv};
use norito::json::{Map, Value};

fn object(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

fn valid(
    id: &'static str,
    kind: &'static str,
    canonical: String,
    mantissa: String,
    scale: Option<u32>,
    frame: Vec<u8>,
    envelope: Vec<u8>,
) -> Value {
    object([
        ("id", Value::from(id)),
        ("kind", Value::from(kind)),
        ("canonical", Value::from(canonical)),
        ("mantissa", Value::from(mantissa)),
        ("scale", scale.map_or(Value::Null, Value::from)),
        (
            "body_hex",
            Value::from(hex::encode(&frame[NUMERIC_FRAME_HEADER_BYTES_V1..])),
        ),
        ("frame_hex", Value::from(hex::encode(frame))),
        ("envelope_hex", Value::from(hex::encode(envelope))),
    ])
}

fn invalid(
    id: &'static str,
    input: &'static str,
    decode_as: &'static str,
    expected: &'static str,
    bytes: Vec<u8>,
) -> Value {
    object([
        ("id", Value::from(id)),
        ("input", Value::from(input)),
        ("decode_as", Value::from(decode_as)),
        ("expected", Value::from(expected)),
        ("hex", Value::from(hex::encode(bytes))),
    ])
}

fn invalid_text(
    id: &'static str,
    kind: &'static str,
    input: Value,
    expected: &'static str,
) -> Value {
    object([
        ("id", Value::from(id)),
        ("kind", Value::from(kind)),
        ("input", input),
        ("expected", Value::from(expected)),
    ])
}

fn int_vector(id: &'static str, value: BigInt) -> Value {
    let frame = IntValueV1::try_new(value.clone())
        .expect("bounded fixture integer")
        .encode_frame()
        .expect("fixture int frame");
    let envelope = numeric_tlv::encode_int(&value).expect("fixture int envelope");
    valid(
        id,
        "int",
        value.to_string(),
        value.to_string(),
        None,
        frame,
        envelope,
    )
}

fn decimal_vector(id: &'static str, value: Numeric) -> Value {
    value.validate_decimal().expect("canonical fixture decimal");
    let frame = DecimalValueV1::from_canonical_numeric(value.clone())
        .expect("canonical decimal")
        .encode_frame()
        .expect("fixture decimal frame");
    let envelope = numeric_tlv::encode_decimal(&value).expect("fixture decimal envelope");
    valid(
        id,
        "decimal",
        value.to_string(),
        value.mantissa().to_string(),
        Some(value.scale()),
        frame,
        envelope,
    )
}

fn quantity_vector(id: &'static str, value: Quantity) -> Value {
    let frame = QuantityValueV1::new(value.clone())
        .encode_frame()
        .expect("fixture quantity frame");
    let envelope = numeric_tlv::encode_quantity(&value).expect("fixture quantity envelope");
    valid(
        id,
        "quantity",
        value.to_string(),
        value.as_numeric().mantissa().to_string(),
        Some(value.as_numeric().scale()),
        frame,
        envelope,
    )
}

fn body(mantissa: &[u8], scale: Option<u8>) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + mantissa.len() + usize::from(scale.is_some()));
    body.extend_from_slice(
        &u32::try_from(mantissa.len())
            .expect("bounded fixture mantissa")
            .to_le_bytes(),
    );
    body.extend_from_slice(mantissa);
    if let Some(scale) = scale {
        body.push(scale);
    }
    body
}

fn frame(schema: [u8; 16], body: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(NUMERIC_FRAME_HEADER_BYTES_V1 + body.len());
    frame.extend_from_slice(&norito::core::MAGIC);
    frame.push(norito::core::VERSION_MAJOR);
    frame.push(norito::core::VERSION_MINOR);
    frame.extend_from_slice(&schema);
    frame.push(0);
    frame.extend_from_slice(
        &u64::try_from(body.len())
            .expect("bounded fixture body")
            .to_le_bytes(),
    );
    frame.extend_from_slice(&norito::crc64_fallback(body).to_le_bytes());
    frame.push(0);
    frame.extend_from_slice(body);
    frame
}

fn envelope(pointer_type: u16, version: u8, frame: &[u8]) -> Vec<u8> {
    let mut envelope = Vec::with_capacity(7 + frame.len() + Hash::LENGTH);
    envelope.extend_from_slice(&pointer_type.to_be_bytes());
    envelope.push(version);
    envelope.extend_from_slice(
        &u32::try_from(frame.len())
            .expect("bounded fixture frame")
            .to_be_bytes(),
    );
    envelope.extend_from_slice(frame);
    envelope.extend_from_slice(Hash::new(frame).as_ref());
    envelope
}

fn signed_endpoints() -> (BigInt, BigInt) {
    let maximum = vec![0xff_u8; MAX_MANTISSA_BYTES - 1]
        .into_iter()
        .chain([0x7f])
        .collect::<Vec<_>>();
    let minimum = vec![0_u8; MAX_MANTISSA_BYTES - 1]
        .into_iter()
        .chain([0x80])
        .collect::<Vec<_>>();
    (
        BigInt::from_twos_bytes(&minimum).expect("minimum endpoint"),
        BigInt::from_twos_bytes(&maximum).expect("maximum endpoint"),
    )
}

fn increment_unsigned_decimal(source: &str) -> String {
    assert!(
        !source.is_empty() && source.bytes().all(|byte| byte.is_ascii_digit()),
        "fixture decimal increment accepts unsigned digits"
    );
    let mut digits = source.as_bytes().to_vec();
    let mut carry = true;
    for digit in digits.iter_mut().rev() {
        if !carry {
            break;
        }
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            carry = false;
        }
    }
    if carry {
        digits.insert(0, b'1');
    }
    String::from_utf8(digits).expect("decimal digits are UTF-8")
}

/// Render the deterministic shared JSON fixture.
pub fn render_fixture() -> String {
    let (minimum, maximum) = signed_endpoints();
    let positive_overflow_text = increment_unsigned_decimal(&maximum.to_string());
    let negative_overflow_text = format!(
        "-{}",
        increment_unsigned_decimal(
            minimum
                .to_string()
                .strip_prefix('-')
                .expect("minimum endpoint is negative")
        )
    );
    let scale_29_text = format!("0.{}1", "0".repeat(28));
    let valid_values = vec![
        int_vector("int_zero", BigInt::zero()),
        int_vector("int_127", BigInt::from_i128(127)),
        int_vector("int_128", BigInt::from_i128(128)),
        int_vector("int_neg_128", BigInt::from_i128(-128)),
        int_vector("int_neg_129", BigInt::from_i128(-129)),
        int_vector(
            "int_2_pow_63_minus_1",
            BigInt::from_i128(i128::from(i64::MAX)),
        ),
        int_vector("int_2_pow_63", BigInt::from_i128(1_i128 << 63)),
        int_vector("int_min", minimum.clone()),
        int_vector("int_max", maximum.clone()),
        decimal_vector("decimal_zero", Numeric::new(0, 0)),
        decimal_vector("decimal_min", Numeric::new(minimum.clone(), 0)),
        decimal_vector("decimal_max", Numeric::new(maximum.clone(), 0)),
        decimal_vector("decimal_neg_1_25", Numeric::new(-125, 2)),
        decimal_vector("decimal_scale_28", Numeric::new(BigInt::one(), 28)),
        quantity_vector("quantity_zero", "0".parse().expect("zero quantity")),
        quantity_vector("quantity_1_25", "1.25".parse().expect("quantity")),
        quantity_vector(
            "quantity_max",
            Quantity::from_canonical_numeric(Numeric::new(maximum.clone(), 0))
                .expect("maximum is a quantity"),
        ),
        quantity_vector(
            "quantity_scale_28",
            Quantity::from_canonical_numeric(Numeric::new(BigInt::one(), 28))
                .expect("positive scale-28 value is a quantity"),
        ),
    ];

    let redundant_zero_frame = frame(INT_SCHEMA_HASH_V1, &body(&[0], None));
    let redundant_positive_frame = frame(INT_SCHEMA_HASH_V1, &body(&[1, 0], None));
    let redundant_negative_frame = frame(INT_SCHEMA_HASH_V1, &body(&[0xff, 0xff], None));
    let decimal_zero_scale_frame = frame(DECIMAL_SCHEMA_HASH_V1, &body(&[], Some(1)));
    let decimal_trailing_zero_frame = frame(DECIMAL_SCHEMA_HASH_V1, &body(&[10], Some(1)));
    let decimal_scale_29_frame = frame(DECIMAL_SCHEMA_HASH_V1, &body(&[1], Some(29)));
    let negative_quantity_frame = frame(QUANTITY_SCHEMA_HASH_V1, &body(&[0xff], Some(0)));
    let quantity_zero_scale_frame = frame(QUANTITY_SCHEMA_HASH_V1, &body(&[], Some(1)));
    let quantity_trailing_zero_frame = frame(QUANTITY_SCHEMA_HASH_V1, &body(&[10], Some(1)));
    let mut positive_overflow = vec![0_u8; MAX_MANTISSA_BYTES + 1];
    positive_overflow[MAX_MANTISSA_BYTES - 1] = 0x80;
    let positive_overflow_frame = frame(INT_SCHEMA_HASH_V1, &body(&positive_overflow, None));
    let mut negative_overflow = vec![0xff_u8; MAX_MANTISSA_BYTES + 1];
    negative_overflow[MAX_MANTISSA_BYTES - 1] = 0x7f;
    let negative_overflow_frame = frame(INT_SCHEMA_HASH_V1, &body(&negative_overflow, None));
    let canonical_int_frame = IntValueV1::try_new(BigInt::one())
        .expect("bounded attack integer")
        .encode_frame()
        .expect("canonical attack base");

    let mut wrong_schema_frame = canonical_int_frame.clone();
    wrong_schema_frame[6..22].copy_from_slice(&DECIMAL_SCHEMA_HASH_V1);

    let mut compressed_frame = canonical_int_frame.clone();
    compressed_frame[22] = 1;

    let mut layout_flags_frame = canonical_int_frame.clone();
    layout_flags_frame[39] = 1;

    let mut bad_crc_frame = canonical_int_frame.clone();
    let last = bad_crc_frame.len() - 1;
    bad_crc_frame[last] ^= 1;

    let canonical_int_envelope = numeric_tlv::encode_int(&BigInt::one()).expect("attack envelope");
    let frame_too_short = canonical_int_frame[..NUMERIC_FRAME_HEADER_BYTES_V1 - 1].to_vec();
    let mut invalid_header_frame = canonical_int_frame.clone();
    invalid_header_frame[0] ^= 1;
    let mut frame_length_mismatch = canonical_int_frame.clone();
    let declared_body_length = u64::from_le_bytes(
        frame_length_mismatch[23..31]
            .try_into()
            .expect("frame body length field"),
    );
    frame_length_mismatch[23..31]
        .copy_from_slice(&(declared_body_length + 1).to_le_bytes());
    let mut malformed_body = body(&[1], None);
    malformed_body[..4].copy_from_slice(&2_u32.to_le_bytes());
    let body_length_mismatch = frame(INT_SCHEMA_HASH_V1, &malformed_body);
    let mut bad_hash_envelope = canonical_int_envelope.clone();
    let last = bad_hash_envelope.len() - 1;
    bad_hash_envelope[last] ^= 1;

    let mut oversized_envelope = canonical_int_envelope.clone();
    oversized_envelope[3..7].copy_from_slice(
        &u32::try_from(MAX_INT_FRAME_BYTES_V1 + 1)
            .expect("numeric frame bound fits u32")
            .to_be_bytes(),
    );
    let mut envelope_length_mismatch = canonical_int_envelope.clone();
    let declared_frame_length = u32::from_be_bytes(
        envelope_length_mismatch[3..7]
            .try_into()
            .expect("envelope frame length field"),
    );
    envelope_length_mismatch[3..7]
        .copy_from_slice(&(declared_frame_length - 1).to_be_bytes());

    let invalid_values = vec![
        invalid(
            "redundant_zero",
            "frame",
            "int",
            "noncanonical_mantissa",
            redundant_zero_frame,
        ),
        invalid(
            "redundant_positive_sign",
            "frame",
            "int",
            "noncanonical_mantissa",
            redundant_positive_frame,
        ),
        invalid(
            "redundant_negative_sign",
            "frame",
            "int",
            "noncanonical_mantissa",
            redundant_negative_frame,
        ),
        invalid(
            "decimal_zero_nonzero_scale",
            "frame",
            "decimal",
            "noncanonical_decimal",
            decimal_zero_scale_frame,
        ),
        invalid(
            "decimal_removable_zero",
            "frame",
            "decimal",
            "noncanonical_decimal",
            decimal_trailing_zero_frame,
        ),
        invalid(
            "decimal_scale_29",
            "frame",
            "decimal",
            "invalid_scale",
            decimal_scale_29_frame,
        ),
        invalid(
            "negative_quantity",
            "frame",
            "quantity",
            "negative_quantity",
            negative_quantity_frame,
        ),
        invalid(
            "quantity_zero_nonzero_scale",
            "frame",
            "quantity",
            "noncanonical_decimal",
            quantity_zero_scale_frame,
        ),
        invalid(
            "quantity_removable_zero",
            "frame",
            "quantity",
            "noncanonical_decimal",
            quantity_trailing_zero_frame,
        ),
        invalid(
            "positive_mantissa_overflow",
            "frame",
            "int",
            "frame_too_large",
            positive_overflow_frame,
        ),
        invalid(
            "negative_mantissa_overflow",
            "frame",
            "int",
            "frame_too_large",
            negative_overflow_frame,
        ),
        invalid(
            "frame_too_short",
            "frame",
            "int",
            "frame_too_short",
            frame_too_short,
        ),
        invalid(
            "invalid_frame_header",
            "frame",
            "int",
            "invalid_header",
            invalid_header_frame,
        ),
        invalid(
            "declared_frame_length_mismatch",
            "frame",
            "int",
            "length_mismatch",
            frame_length_mismatch,
        ),
        invalid(
            "numeric_body_length_mismatch",
            "frame",
            "int",
            "length_mismatch",
            body_length_mismatch,
        ),
        invalid(
            "wrong_frame_schema",
            "frame",
            "int",
            "schema_mismatch",
            wrong_schema_frame,
        ),
        invalid(
            "compressed_frame",
            "frame",
            "int",
            "compression_not_allowed",
            compressed_frame,
        ),
        invalid(
            "layout_flags_frame",
            "frame",
            "int",
            "layout_flags_not_allowed",
            layout_flags_frame,
        ),
        invalid(
            "bad_frame_checksum",
            "frame",
            "int",
            "checksum_mismatch",
            bad_crc_frame,
        ),
        invalid(
            "bad_payload_hash",
            "envelope",
            "int",
            "payload_hash_mismatch",
            bad_hash_envelope,
        ),
        invalid(
            "cross_type_envelope",
            "envelope",
            "decimal",
            "wrong_type",
            canonical_int_envelope.clone(),
        ),
        invalid(
            "retired_amount_pointer_type",
            "envelope",
            "int",
            "type_not_allowed",
            envelope(PointerType::RetiredAmount as u16, 1, &canonical_int_frame),
        ),
        invalid(
            "unassigned_pointer_type",
            "envelope",
            "int",
            "unknown_type",
            envelope(0x0014, 2, &canonical_int_frame),
        ),
        invalid(
            "known_nonnumeric_pointer_precedes_version",
            "envelope",
            "int",
            "wrong_type",
            envelope(PointerType::AccountId as u16, 2, &canonical_int_frame),
        ),
        invalid(
            "unknown_pointer_type",
            "envelope",
            "int",
            "unknown_type",
            envelope(0xffff, 1, &canonical_int_frame),
        ),
        invalid(
            "envelope_version_2",
            "envelope",
            "int",
            "invalid_envelope_version",
            envelope(PointerType::Int as u16, 2, &canonical_int_frame),
        ),
        invalid(
            "oversized_declared_frame",
            "envelope",
            "int",
            "oversized_length",
            oversized_envelope,
        ),
        invalid(
            "truncated_envelope",
            "envelope",
            "int",
            "truncated_envelope",
            canonical_int_envelope[..6].to_vec(),
        ),
        invalid(
            "envelope_length_mismatch",
            "envelope",
            "int",
            "truncated_envelope",
            envelope_length_mismatch,
        ),
    ];

    let removable_scale_input = format!("1.{}", "0".repeat(29));
    let removable_scale_value = removable_scale_input
        .parse::<Numeric>()
        .expect("normalization precedes scale validation");
    let text_values = vec![object([
        ("id", Value::from("decimal_removable_scale_29")),
        ("kind", Value::from("decimal")),
        ("input", Value::from(removable_scale_input)),
        ("canonical", Value::from(removable_scale_value.to_string())),
    ])];
    let invalid_text_values = vec![
        invalid_text("int_json_number", "int", Value::from(1_u64), "invalid_text"),
        invalid_text("int_leading_zero", "int", Value::from("01"), "invalid_text"),
        invalid_text("int_plus_sign", "int", Value::from("+1"), "invalid_text"),
        invalid_text("int_negative_zero", "int", Value::from("-0"), "invalid_text"),
        invalid_text(
            "int_positive_overflow",
            "int",
            Value::from(positive_overflow_text.clone()),
            "mantissa_overflow",
        ),
        invalid_text(
            "int_negative_overflow",
            "int",
            Value::from(negative_overflow_text),
            "mantissa_overflow",
        ),
        invalid_text(
            "decimal_json_number",
            "decimal",
            Value::from(1_u64),
            "invalid_text",
        ),
        invalid_text(
            "decimal_exponent",
            "decimal",
            Value::from("1e0"),
            "invalid_text",
        ),
        invalid_text(
            "decimal_whitespace",
            "decimal",
            Value::from(" 1"),
            "invalid_text",
        ),
        invalid_text(
            "decimal_trailing_fractional_zero",
            "decimal",
            Value::from("1.0"),
            "invalid_text",
        ),
        invalid_text(
            "decimal_zero_fraction",
            "decimal",
            Value::from("0.0"),
            "invalid_text",
        ),
        invalid_text(
            "decimal_scale_29",
            "decimal",
            Value::from(scale_29_text.clone()),
            "invalid_scale",
        ),
        invalid_text(
            "decimal_mantissa_overflow",
            "decimal",
            Value::from(positive_overflow_text.clone()),
            "mantissa_overflow",
        ),
        invalid_text(
            "quantity_json_number",
            "quantity",
            Value::from(1_u64),
            "invalid_text",
        ),
        invalid_text(
            "quantity_leading_zero",
            "quantity",
            Value::from("01"),
            "invalid_text",
        ),
        invalid_text(
            "quantity_trailing_fractional_zero",
            "quantity",
            Value::from("1.0"),
            "invalid_text",
        ),
        invalid_text(
            "quantity_zero_fraction",
            "quantity",
            Value::from("0.0"),
            "invalid_text",
        ),
        invalid_text(
            "quantity_scale_29",
            "quantity",
            Value::from(scale_29_text),
            "invalid_scale",
        ),
        invalid_text(
            "quantity_mantissa_overflow",
            "quantity",
            Value::from(positive_overflow_text),
            "mantissa_overflow",
        ),
        invalid_text(
            "quantity_exponent",
            "quantity",
            Value::from("1e0"),
            "invalid_text",
        ),
        invalid_text(
            "quantity_not_a_number",
            "quantity",
            Value::from("NaN"),
            "invalid_text",
        ),
        invalid_text(
            "quantity_negative",
            "quantity",
            Value::from("-1"),
            "negative_quantity",
        ),
    ];

    let document = object([
        ("format", Value::from("iroha.numeric.v1")),
        (
            "generator",
            Value::from("ivm::numeric_tlv + iroha_primitives::numeric_abi"),
        ),
        (
            "signed_bits",
            Value::from(u64::try_from(MAX_MANTISSA_BITS).expect("numeric bit bound fits u64")),
        ),
        ("maximum_scale", Value::from(28_u64)),
        ("text", Value::Array(text_values)),
        ("invalid_text", Value::Array(invalid_text_values)),
        ("valid", Value::Array(valid_values)),
        ("invalid", Value::Array(invalid_values)),
    ]);
    let mut rendered = norito::json::to_json_pretty(&document).expect("serialize numeric fixture");
    rendered.push('\n');
    rendered
}

fn verify(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let expected = render_fixture();
    let actual = fs::read_to_string(path)?;
    if actual != expected {
        return Err(format!(
            "{} is stale; regenerate it with `cargo run -p ivm --example numeric_v1_fixture -- --write {}`",
            path.display(),
            path.display()
        )
        .into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    if let Some(flag) = arguments.next() {
        if flag == "--check" || flag == "--write" {
            let path = arguments
                .next()
                .ok_or("--check/--write requires a fixture path")?;
            if arguments.next().is_some() {
                return Err("unexpected arguments after fixture path".into());
            }
            let path = Path::new(&path);
            if flag == "--check" {
                verify(path)?;
            } else {
                fs::write(path, render_fixture())?;
            }
            return Ok(());
        }
        return Err("only `--check <path>` or `--write <path>` is supported".into());
    }
    print!("{}", render_fixture());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_decimal_increment_handles_carry_boundaries() {
        assert_eq!(increment_unsigned_decimal("0"), "1");
        assert_eq!(increment_unsigned_decimal("129"), "130");
        assert_eq!(increment_unsigned_decimal("999"), "1000");
    }
}
