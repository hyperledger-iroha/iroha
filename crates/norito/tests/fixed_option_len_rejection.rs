//! Mismatched compact flags should reject fixed-width payloads.

use norito::{NoritoSerialize, decode_from_bytes};

#[test]
fn option_fixed_u64_rejected_under_compact_flag() {
    use norito::core::header_flags;

    let value: Option<String> = Some("hi".to_string());
    let mut payload = Vec::new();
    value.serialize(&mut payload).expect("serialize option");

    // Frame a fixed-width payload but advertise compact lengths in the header.
    let bytes = norito::core::frame_bare_with_header_flags::<Option<String>>(
        &payload,
        header_flags::COMPACT_LEN,
    )
    .expect("frame payload");

    let err = decode_from_bytes::<Option<String>>(&bytes)
        .expect_err("compact flags must reject fixed-width payloads");
    assert!(matches!(err, norito::core::Error::LengthMismatch));
}
