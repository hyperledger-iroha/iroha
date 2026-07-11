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

/// Render the deterministic shared JSON fixture.
pub fn render_fixture() -> String {
    let (minimum, maximum) = signed_endpoints();
    let valid_values = vec![
        int_vector("int_zero", BigInt::zero()),
        int_vector("int_127", BigInt::from_i128(127)),
        int_vector("int_128", BigInt::from_i128(128)),
        int_vector("int_neg_128", BigInt::from_i128(-128)),
        int_vector("int_neg_129", BigInt::from_i128(-129)),
        int_vector("int_min", minimum),
        int_vector("int_max", maximum),
        decimal_vector("decimal_zero", Numeric::new(0, 0)),
        decimal_vector("decimal_neg_1_25", Numeric::new(-125, 2)),
        decimal_vector("decimal_scale_28", Numeric::new(BigInt::one(), 28)),
        quantity_vector("quantity_zero", "0".parse().expect("zero quantity")),
        quantity_vector("quantity_1_25", "1.25".parse().expect("quantity")),
    ];

    let redundant_zero_frame = frame(INT_SCHEMA_HASH_V1, &body(&[0], None));
    let redundant_positive_frame = frame(INT_SCHEMA_HASH_V1, &body(&[1, 0], None));
    let decimal_zero_scale_frame = frame(DECIMAL_SCHEMA_HASH_V1, &body(&[], Some(1)));
    let decimal_trailing_zero_frame = frame(DECIMAL_SCHEMA_HASH_V1, &body(&[10], Some(1)));
    let decimal_scale_29_frame = frame(DECIMAL_SCHEMA_HASH_V1, &body(&[1], Some(29)));
    let negative_quantity_frame = frame(QUANTITY_SCHEMA_HASH_V1, &body(&[0xff], Some(0)));
    let mut positive_overflow = vec![0_u8; MAX_MANTISSA_BYTES + 1];
    positive_overflow[MAX_MANTISSA_BYTES - 1] = 0x80;
    let positive_overflow_frame = frame(INT_SCHEMA_HASH_V1, &body(&positive_overflow, None));
    let canonical_int_frame = IntValueV1::try_new(BigInt::one())
        .expect("bounded attack integer")
        .encode_frame()
        .expect("canonical attack base");

    let mut wrong_schema_frame = canonical_int_frame.clone();
    wrong_schema_frame[6..22].copy_from_slice(&DECIMAL_SCHEMA_HASH_V1);

    let mut bad_crc_frame = canonical_int_frame.clone();
    let last = bad_crc_frame.len() - 1;
    bad_crc_frame[last] ^= 1;

    let canonical_int_envelope = numeric_tlv::encode_int(&BigInt::one()).expect("attack envelope");
    let mut bad_hash_envelope = canonical_int_envelope.clone();
    let last = bad_hash_envelope.len() - 1;
    bad_hash_envelope[last] ^= 1;

    let mut oversized_envelope = canonical_int_envelope.clone();
    oversized_envelope[3..7].copy_from_slice(
        &u32::try_from(MAX_INT_FRAME_BYTES_V1 + 1)
            .expect("numeric frame bound fits u32")
            .to_be_bytes(),
    );

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
            "positive_mantissa_overflow",
            "frame",
            "int",
            "frame_too_large",
            positive_overflow_frame,
        ),
        invalid(
            "wrong_frame_schema",
            "frame",
            "int",
            "schema_mismatch",
            wrong_schema_frame,
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
