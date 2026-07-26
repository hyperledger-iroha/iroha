//! AoS decoders reject trailing bytes in ad-hoc payloads.

use norito::{aos, columnar::EnumBorrow};

fn with_trailing(mut body: Vec<u8>) -> Vec<u8> {
    body.push(0xAA);
    body
}

#[test]
fn aos_decode_u64_u32_bool_rejects_trailing() {
    let body = aos::encode_rows_u64_u32_bool(&[(1u64, 2u32, true)]);
    let out = aos::decode_rows_u64_u32_bool(&with_trailing(body));
    assert!(out.is_err());
}

#[test]
fn aos_decode_u64_bytes_bool_rejects_trailing() {
    let payload = [1u8, 2u8, 3u8];
    let body = aos::encode_rows_u64_bytes_bool(&[(1u64, payload.as_slice(), false)]);
    let out = aos::decode_rows_u64_bytes_bool(&with_trailing(body));
    assert!(out.is_err());
}

#[test]
fn aos_decode_u64_bytes_u32_bool_rejects_trailing() {
    let payload = [4u8, 5u8];
    let body = aos::encode_rows_u64_bytes_u32_bool(&[(1u64, payload.as_slice(), 9u32, true)]);
    let out = aos::decode_rows_u64_bytes_u32_bool(&with_trailing(body));
    assert!(out.is_err());
}

#[test]
fn aos_decode_u64_optstr_bool_rejects_trailing() {
    let rows = [(1u64, Some("a"), true), (2u64, None, false)];
    let body = aos::encode_rows_u64_optstr_bool(&rows);
    let out = aos::decode_rows_u64_optstr_bool(&with_trailing(body));
    assert!(out.is_err());
}

#[test]
fn aos_decode_u64_optu32_bool_rejects_trailing() {
    let rows = [(1u64, Some(7u32), true), (2u64, None, false)];
    let body = aos::encode_rows_u64_optu32_bool(&rows);
    let out = aos::decode_rows_u64_optu32_bool(&with_trailing(body));
    assert!(out.is_err());
}

#[test]
fn aos_decode_u64_enum_bool_rejects_trailing() {
    let rows = [
        (1u64, EnumBorrow::Name("hi"), true),
        (2u64, EnumBorrow::Code(7), false),
    ];
    let body = aos::encode_rows_u64_enum_bool(&rows);
    let out = aos::decode_rows_u64_enum_bool(&with_trailing(body));
    assert!(out.is_err());
}
