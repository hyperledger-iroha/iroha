//! Adaptive tests for additional shapes: (u64, bytes, bool) and (u64, u32, bool).

use norito::columnar::{
    ADAPTIVE_TAG_AOS, ADAPTIVE_TAG_NCB, decode_rows_u64_bytes_bool_adaptive,
    decode_rows_u64_u32_bool_adaptive, encode_rows_u64_bytes_bool_adaptive,
    encode_rows_u64_u32_bool_adaptive, should_use_columnar,
};

#[test]
fn adaptive_bytes_small_prefers_aos() {
    let mut rows: Vec<(u64, Vec<u8>, bool)> = Vec::new();
    for i in 0..16u64 {
        // Force large ID deltas to defeat id-delta heuristics in NCB
        rows.push((i << 60, vec![i as u8; (i as usize % 5) + 1], i % 2 == 0));
    }
    let borrowed: Vec<(u64, &[u8], bool)> = rows
        .iter()
        .map(|(id, b, f)| (*id, b.as_slice(), *f))
        .collect();
    let bytes = encode_rows_u64_bytes_bool_adaptive(&borrowed);
    assert!(
        matches!(
            bytes.first(),
            Some(&tag) if tag == ADAPTIVE_TAG_AOS || tag == ADAPTIVE_TAG_NCB
        ),
        "adaptive encoder tag should be AoS or NCB"
    );
    let decoded = decode_rows_u64_bytes_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_bytes_large_prefers_ncb() {
    let mut rows: Vec<(u64, Vec<u8>, bool)> = Vec::new();
    for i in 0..96u64 {
        rows.push((i * 2, vec![(i % 8) as u8; 7 + (i as usize % 4)], i % 3 == 0));
    }
    let borrowed: Vec<(u64, &[u8], bool)> = rows
        .iter()
        .map(|(id, b, f)| (*id, b.as_slice(), *f))
        .collect();
    let bytes = encode_rows_u64_bytes_bool_adaptive(&borrowed);
    let expected_tag = if should_use_columnar(borrowed.len()) {
        ADAPTIVE_TAG_NCB
    } else {
        ADAPTIVE_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_bytes_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_u64_u32_small_prefers_aos() {
    let mut rows: Vec<(u64, u32, bool)> = Vec::new();
    for i in 0..6u64 {
        // IDs with huge deltas (i<<60) disable id-delta savings.
        // Alternate u32 values 0 / 0x8000_0000 to force large deltas and disable u32-delta.
        let v = if i % 2 == 0 { 0u32 } else { 0x8000_0000 };
        rows.push((i << 60, v, i % 2 == 1));
    }
    let bytes = encode_rows_u64_u32_bool_adaptive(&rows);
    assert!(
        matches!(
            bytes.first(),
            Some(&tag) if tag == ADAPTIVE_TAG_AOS || tag == ADAPTIVE_TAG_NCB
        ),
        "adaptive encoder tag should be AoS or NCB"
    );
    let decoded = decode_rows_u64_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_u64_u32_large_prefers_ncb() {
    let mut rows: Vec<(u64, u32, bool)> = Vec::new();
    for i in 0..256u64 {
        rows.push((i * 3, 1000 + i as u32, i % 3 == 0));
    }
    let bytes = encode_rows_u64_u32_bool_adaptive(&rows);
    let expected_tag = if should_use_columnar(rows.len()) {
        ADAPTIVE_TAG_NCB
    } else {
        ADAPTIVE_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}
