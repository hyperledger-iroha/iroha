//! Adaptive bare codec tests: ensure encode_adaptive/decode_adaptive roundtrip.

use norito::codec::{decode_adaptive, encode_adaptive};

#[test]
fn adaptive_bare_small_vec_roundtrip() {
    let v: Vec<u32> = (0..256u32).collect();
    let bytes = encode_adaptive(&v);
    let out: Vec<u32> = decode_adaptive(&bytes).expect("decode");
    assert_eq!(out, v);
}

#[test]
fn adaptive_bare_large_vec_roundtrip() {
    let v: Vec<u32> = (0..200_000u32).collect();
    let bytes = encode_adaptive(&v);
    let out: Vec<u32> = decode_adaptive(&bytes).expect("decode");
    assert_eq!(out, v);
}

#[test]
fn adaptive_decode_handles_misaligned_payloads() {
    let value: Vec<Vec<u64>> = vec![vec![1, 2], vec![3, 4, 5], vec![]];
    let encoded = encode_adaptive(&value);

    let align = core::mem::align_of::<u64>();
    let mut buffer = vec![0u8; align + encoded.len()];
    let base = buffer.as_ptr() as usize;
    let offset = (1..align)
        .find(|candidate| !(base + candidate).is_multiple_of(align))
        .expect("non-zero offset for misalignment");
    buffer[offset..(offset + encoded.len())].copy_from_slice(&encoded);
    let misaligned = &buffer[offset..(offset + encoded.len())];
    assert_ne!(
        misaligned.as_ptr() as usize % align,
        0,
        "test requires misalignment"
    );

    let decoded: Vec<Vec<u64>> = decode_adaptive(misaligned).expect("decode");
    assert_eq!(decoded, value);
}
