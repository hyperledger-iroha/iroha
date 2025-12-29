//! UTF-8 validation gating tests: small vs large strings roundtrip via Norito.
use norito::{decode_from_bytes, to_bytes};

#[test]
fn small_string_roundtrip() {
    let s = String::from("hello, norito");
    let bytes = to_bytes(&s).expect("encode");
    let out: String = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(s, out);
}

#[test]
fn large_string_roundtrip() {
    // Build a large valid UTF-8 string to exercise SIMD-gated path when enabled
    let s = "abc123XYZ_".repeat(8192 / 10);
    let bytes = to_bytes(&s).expect("encode");
    let out: String = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(s, out);
}
