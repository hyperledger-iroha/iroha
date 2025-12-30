//! Regression: decoding must reject non-zero minor versions. V1 fixes the minor
//! byte to `VERSION_MINOR = 0x00`; layout selection lives in the header flags
//! instead of the minor version.

use norito::{core::header_flags, from_bytes, to_bytes};

#[test]
fn decode_rejects_nonzero_minor_version() {
    let value = vec![1u32, 2, 3];
    let bytes = to_bytes(&value).expect("serialize test payload");

    // Sanity checks: ensure header structure matches expectations.
    assert_eq!(bytes[0..4], *b"NRT0");
    assert_eq!(bytes[4], norito::core::VERSION_MAJOR);
    assert_eq!(bytes[5], norito::core::VERSION_MINOR);

    // Simulate a non-zero minor byte. V1 rejects mismatched minor versions
    // regardless of the header flags.
    let mut subset_minor = bytes.clone();
    subset_minor[5] = header_flags::PACKED_SEQ | header_flags::PACKED_STRUCT;

    // This returns `Err(UnsupportedMinorVersion(_))` because v1 minors must
    // match exactly.
    match from_bytes::<Vec<u32>>(&subset_minor) {
        Err(norito::Error::UnsupportedMinorVersion { .. }) => {}
        Err(err) => panic!("unexpected error kind: {err}"),
        Ok(_) => panic!("subset minor version should be rejected in canonical builds"),
    }
}
