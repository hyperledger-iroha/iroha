//! Regression test coverage for tuple deserialization edge cases.

use norito::{decode_from_bytes, to_bytes};

#[test]
fn tuple_roundtrip_handles_empty_string() {
    let value = (123u16, String::new());
    let bytes = to_bytes(&value).expect("serialize tuple with empty string");
    let decoded: (u16, String) = decode_from_bytes(&bytes).expect("decode tuple with empty string");
    assert_eq!(decoded, value);
}

#[test]
fn tuple_roundtrip_with_zst_prefix() {
    let value = ((), String::from("edge"));
    let bytes = to_bytes(&value).expect("serialize tuple with ZST prefix");
    let decoded: ((), String) = decode_from_bytes(&bytes).expect("decode tuple with ZST prefix");
    assert_eq!(decoded, value);
}

#[test]
fn tuple_roundtrip_mixed_large_elements() {
    let large_str = "x".repeat(4096);
    let nested: Vec<Vec<u8>> = vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8], Vec::new()];
    let value = ((), large_str, nested);
    let bytes = to_bytes(&value).expect("serialize tuple with mixed elements");
    let decoded: ((), String, Vec<Vec<u8>>) =
        decode_from_bytes(&bytes).expect("decode tuple with mixed elements");
    assert_eq!(decoded, value);
}
