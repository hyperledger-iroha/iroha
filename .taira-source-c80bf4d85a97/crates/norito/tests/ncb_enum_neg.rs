//! Negative tests for NCB enum view (invalid tags)

use norito::core::Error;

#[test]
fn ncb_enum_invalid_tag_detected() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool};
    // Build a simple valid payload first
    let rows = vec![
        (1u64, EnumBorrow::Name("a"), true),
        (2u64, EnumBorrow::Code(7), false),
    ];
    let mut body = encode_ncb_u64_enum_bool(&rows, false, false, false);
    // Compute offset to tags: [n:4][desc:1][align to 8][ids:8*n] then [tags:n]
    let n = 2usize;
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n; // ids
    // Flip first tag to an invalid value (2)
    assert!(off < body.len());
    body[off] = 2u8;
    let res = norito::columnar::view_ncb_u64_enum_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidTag { .. }));
    }
}
