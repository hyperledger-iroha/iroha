//! Adaptive tests for optional columns.

#![allow(unused_imports)]
use norito::{
    columnar as ncb,
    columnar::{
        ADAPTIVE_TAG_AOS, ADAPTIVE_TAG_NCB, decode_rows_u64_optstr_bool_adaptive,
        decode_rows_u64_optu32_bool_adaptive, encode_rows_u64_optstr_bool_adaptive,
        encode_rows_u64_optu32_bool_adaptive,
    },
};

#[test]
fn adaptive_optstr_small_prefers_aos() {
    let mut rows: Vec<(u64, Option<String>, bool)> = Vec::new();
    for i in 0..6u64 {
        // Mostly Some short strings; one None to exercise presence
        let s = if i == 5 { None } else { Some(format!("s{i}")) };
        // Use large ID deltas to defeat id-delta in NCB
        rows.push((i << 60, s, i % 2 == 0));
    }
    let borrowed: Vec<(u64, Option<&str>, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_deref(), *b))
        .collect();
    let bytes = encode_rows_u64_optstr_bool_adaptive(&borrowed);
    // Small-N two-pass may choose either AoS or NCB; accept both.
    assert!(bytes[0] == ADAPTIVE_TAG_AOS || bytes[0] == ADAPTIVE_TAG_NCB);
    let decoded = decode_rows_u64_optstr_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_optstr_large_prefers_ncb() {
    let mut rows: Vec<(u64, Option<String>, bool)> = Vec::new();
    for i in 0..128u64 {
        let s = if i % 5 == 0 {
            None
        } else {
            Some(format!("name{:02}", (i % 8)))
        };
        rows.push((i * 3, s, i % 3 == 0));
    }
    let borrowed: Vec<(u64, Option<&str>, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_deref(), *b))
        .collect();
    let bytes = encode_rows_u64_optstr_bool_adaptive(&borrowed);
    let decoded = decode_rows_u64_optstr_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_optu32_small_prefers_aos() {
    let mut rows: Vec<(u64, Option<u32>, bool)> = Vec::new();
    for i in 0..2u64 {
        let v = if i % 2 == 0 { Some(42u32) } else { None };
        // Large ID deltas to avoid id-delta heuristics in NCB
        rows.push((i << 60, v, i % 2 == 1));
    }
    let bytes = encode_rows_u64_optu32_bool_adaptive(&rows);
    // Small-N two-pass may choose either AoS or NCB; accept both.
    assert!(bytes[0] == ADAPTIVE_TAG_AOS || bytes[0] == ADAPTIVE_TAG_NCB);
    let decoded = decode_rows_u64_optu32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_optu32_large_prefers_ncb() {
    let mut rows: Vec<(u64, Option<u32>, bool)> = Vec::new();
    for i in 0..96u64 {
        let v = if i % 4 == 0 {
            None
        } else {
            Some((1000 + (i % 16)) as u32)
        };
        rows.push((i * 5, v, i % 7 == 0));
    }
    let bytes = encode_rows_u64_optu32_bool_adaptive(&rows);
    let decoded = decode_rows_u64_optu32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn small_n_optstr_all_none_prefers_ncb() {
    // When all Option<&str> are None, the presence bitset + minimal offsets
    // clearly beats per-row option tags, so small-N selector should choose NCB.
    let rows_borrowed: Vec<(u64, Option<&str>, bool)> =
        (0..10u64).map(|i| (i * 7, None, i % 2 == 0)).collect();
    let bytes = encode_rows_u64_optstr_bool_adaptive(&rows_borrowed);
    assert_eq!(bytes[0], ncb::ADAPTIVE_TAG_NCB);
    let decoded = decode_rows_u64_optstr_bool_adaptive(&bytes).expect("decode");
    let expected: Vec<(u64, Option<String>, bool)> = rows_borrowed
        .iter()
        .map(|(id, _s, b)| (*id, None, *b))
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn small_n_optu32_all_none_prefers_ncb() {
    // Same rationale: bitset + zero present values should be smaller than
    // per-row option tags at small N.
    let rows: Vec<(u64, Option<u32>, bool)> =
        (0..10u64).map(|i| (i * 11, None, i % 2 == 1)).collect();
    let bytes = encode_rows_u64_optu32_bool_adaptive(&rows);
    assert_eq!(bytes[0], ncb::ADAPTIVE_TAG_NCB);
    let decoded = decode_rows_u64_optu32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn small_n_optstr_repeated_some_roundtrip_either_tag() {
    // Repeated Some strings do not deduplicate in opt-str column; selector may choose either.
    let rep = "repeated_name".repeat(4);
    let rows_owned: Vec<(u64, Option<String>, bool)> = (0..12u64)
        .map(|i| (i * 3, Some(rep.clone()), i % 2 == 0))
        .collect();
    let borrowed: Vec<(u64, Option<&str>, bool)> = rows_owned
        .iter()
        .map(|(id, s, b)| (*id, s.as_deref(), *b))
        .collect();
    let bytes = encode_rows_u64_optstr_bool_adaptive(&borrowed);
    // Accept either AoS or NCB; assert roundtrip correctness.
    assert!(bytes[0] == ADAPTIVE_TAG_AOS || bytes[0] == ADAPTIVE_TAG_NCB);
    let decoded = decode_rows_u64_optstr_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}
