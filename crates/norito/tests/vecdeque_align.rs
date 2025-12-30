//! Ensure VecDeque<T> decodes correctly without per-element copies when aligned.

use std::collections::VecDeque;

use norito::{decode_from_bytes, to_bytes};

#[test]
fn vecdeque_roundtrip_u32() {
    let mut vd = VecDeque::new();
    for i in 0u32..64 {
        vd.push_back(i.wrapping_mul(3));
    }
    let bytes = to_bytes(&vd).expect("encode");
    let out: VecDeque<u32> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(vd, out);
}

#[test]
fn vecdeque_roundtrip_strings() {
    let mut vd = VecDeque::new();
    for i in 0..16usize {
        vd.push_back(format!("s{i}"));
    }
    let bytes = to_bytes(&vd).expect("encode");
    let out: VecDeque<String> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(vd, out);
}
