//! Fixed-width length header tests.
use norito::core::{read_len_from_slice, write_len_to_vec};

#[test]
fn fixed_width_roundtrip() {
    norito::core::reset_decode_state();
    let values = [
        0u64,
        1,
        0x7F,
        0x80,
        0x123,
        0x3FFF,
        0x4000,
        0x1234_5678,
        0xFFFF_FFFF,
    ];
    for v in values {
        let mut buf = Vec::new();
        write_len_to_vec(&mut buf, v);
        let (out, used) = read_len_from_slice(&buf).expect("decode");
        assert_eq!(buf.len(), 8, "length header must be 8 bytes");
        assert_eq!(out as u64, v);
        assert_eq!(used, 8, "decoder must always consume 8 bytes");
    }
    norito::core::reset_decode_state();
}
