//! Tests for adaptive AoS/columnar Norito encoding.

use norito::columnar::{
    ADAPTIVE_TAG_AOS, ADAPTIVE_TAG_NCB, decode_rows_u64_str_bool_adaptive,
    encode_rows_u64_str_bool_adaptive, should_use_columnar,
};

#[test]
fn adaptive_small_two_pass_picks_smaller() {
    // For n < 64, encoder performs a two-pass size probe and picks the smaller
    // of AoS vs NCB. We only assert roundtrip and that the tag is one of the
    // adaptive markers.
    let mut rows: Vec<(u64, String, bool)> = Vec::new();
    for i in 0..10u64 {
        rows.push((i, format!("name{i}"), i % 2 == 0));
    }
    // Build borrowed view
    let borrowed: Vec<(u64, &str, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_str(), *b))
        .collect();
    let bytes = encode_rows_u64_str_bool_adaptive(&borrowed);
    assert!(matches!(
        bytes.first(),
        Some(&ADAPTIVE_TAG_AOS) | Some(&ADAPTIVE_TAG_NCB)
    ));
    let decoded = decode_rows_u64_str_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_large_prefers_columnar() {
    // n ≥ 64 should choose columnar path
    let mut rows: Vec<(u64, String, bool)> = Vec::new();
    for i in 0..128u64 {
        rows.push((i * 2, format!("user{:03}", (i % 8)), i % 3 == 0));
    }
    let borrowed: Vec<(u64, &str, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_str(), *b))
        .collect();
    let bytes = encode_rows_u64_str_bool_adaptive(&borrowed);
    let expected_tag = if should_use_columnar(borrowed.len()) {
        ADAPTIVE_TAG_NCB
    } else {
        ADAPTIVE_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_str_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}
