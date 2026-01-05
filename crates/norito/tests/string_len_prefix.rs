//! Regression tests for string length-prefix decoding.

use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self, DecodeFlagsGuard, header_flags, reset_decode_state},
};

#[test]
fn compact_len_string_preserves_leading_varint_byte() {
    let raw = vec![0x01, b'a'];
    let s = String::from_utf8(raw.clone()).expect("valid utf8");

    let mut payload = Vec::new();
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        core::write_len_to_vec(&mut payload, raw.len() as u64);
    }
    payload.extend_from_slice(&raw);

    let bytes = core::frame_bare_with_header_flags::<String>(&payload, header_flags::COMPACT_LEN)
        .expect("frame payload");

    reset_decode_state();
    let out: String = core::decode_from_bytes(&bytes).expect("decode string");
    assert_eq!(out.as_bytes(), s.as_bytes());
    reset_decode_state();
}

#[test]
fn compact_len_str_preserves_leading_varint_byte() {
    let raw = vec![0x01, b'a'];
    let s = String::from_utf8(raw.clone()).expect("valid utf8");

    let mut payload = Vec::new();
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        core::write_len_to_vec(&mut payload, raw.len() as u64);
    }
    payload.extend_from_slice(&raw);

    let bytes = core::frame_bare_with_header_flags::<String>(&payload, header_flags::COMPACT_LEN)
        .expect("frame payload");

    reset_decode_state();
    let out: &str = core::decode_from_bytes(&bytes).expect("decode str");
    assert_eq!(out.as_bytes(), s.as_bytes());
    reset_decode_state();
}

#[test]
fn string_deserialize_without_ctx_preserves_full_bytes() {
    reset_decode_state();
    let mut raw = Vec::new();
    raw.extend_from_slice(&8u64.to_le_bytes());
    raw.extend_from_slice(b"abcdefgh");
    let s = String::from_utf8(raw.clone()).expect("valid utf8");

    let mut payload = Vec::new();
    s.serialize(&mut payload).expect("serialize string");
    let archived = core::archived_from_slice_unchecked::<String>(&payload);
    let decoded = <String as NoritoDeserialize>::deserialize(archived.archived());
    assert_eq!(decoded.as_bytes(), s.as_bytes());
    reset_decode_state();
}
