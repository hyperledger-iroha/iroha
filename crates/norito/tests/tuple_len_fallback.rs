//! Mismatched compact flags should reject fixed-width tuple payloads.

use norito::{
    core::{NoritoSerialize, header_flags},
    decode_from_bytes,
};

#[test]
fn tuple_fixed_u64_rejected_under_compact_flag() {
    let value = (5u8, String::from("hi"));
    let mut payload = Vec::new();
    value.serialize(&mut payload).expect("serialize tuple");

    let bytes = norito::core::frame_bare_with_header_flags::<(u8, String)>(
        &payload,
        header_flags::COMPACT_LEN,
    )
    .expect("frame payload");

    let err = decode_from_bytes::<(u8, String)>(&bytes)
        .expect_err("compact flags must reject fixed-width payloads");
    assert!(matches!(err, norito::core::Error::LengthMismatch));
}
