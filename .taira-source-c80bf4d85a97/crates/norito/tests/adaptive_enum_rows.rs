//! Tests for adaptive enum-shaped rows AoS/columnar Norito encoding.

use norito::columnar::{
    ADAPTIVE_ENUM_TAG_AOS, ADAPTIVE_ENUM_TAG_NCB, EnumBorrow, RowEnumOwned,
    decode_rows_u64_enum_bool_adaptive, encode_rows_u64_enum_bool_adaptive, should_use_columnar,
};

#[test]
fn adaptive_enum_small_roundtrip() {
    // Build a small dataset mixing Name and Code variants
    let mut rows_borrowed: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut rows_owned: Vec<(u64, RowEnumOwned, bool)> = Vec::new();
    // Prebuild unique short names for even rows to avoid lifetime/borrow issues
    let names: Vec<String> = (0..10u64).step_by(2).map(|i| format!("n{i:02}")).collect();
    let mut code_idx: u32 = 0;
    for i in 0..10u64 {
        // Make NCB heuristics less favorable at small n:
        // - IDs: huge deltas (i<<60) disable id-delta savings (varint >= 9B/delta)
        // - Names: short and unique to avoid dict; offsets overhead dominates
        // - Codes: alternating 0 / 0x8000_0000 to force large deltas (varint ~5B)
        let id = i << 60;
        if i % 2 == 0 {
            let idx = (i / 2) as usize;
            let s_ref: &str = names[idx].as_str();
            rows_borrowed.push((id, EnumBorrow::Name(s_ref), i % 3 == 0));
            rows_owned.push((id, RowEnumOwned::Name(names[idx].clone()), i % 3 == 0));
        } else {
            let code = if code_idx.is_multiple_of(2) {
                0u32
            } else {
                0x8000_0000
            };
            code_idx += 1;
            rows_borrowed.push((id, EnumBorrow::Code(code), i % 3 == 0));
            rows_owned.push((id, RowEnumOwned::Code(code), i % 3 == 0));
        }
    }
    let bytes = encode_rows_u64_enum_bool_adaptive(&rows_borrowed);
    assert!(
        matches!(
            bytes.first(),
            Some(&tag) if tag == ADAPTIVE_ENUM_TAG_AOS || tag == ADAPTIVE_ENUM_TAG_NCB
        ),
        "adaptive encoder tag should be AoS or NCB"
    );
    let decoded = decode_rows_u64_enum_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}

#[test]
fn adaptive_enum_large_prefers_columnar() {
    let mut rows_borrowed: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut rows_owned: Vec<(u64, RowEnumOwned, bool)> = Vec::new();
    for i in 0..128u64 {
        if i % 4 == 0 {
            rows_borrowed.push((i, EnumBorrow::Name("user"), i % 5 == 0));
            rows_owned.push((i, RowEnumOwned::Name("user".to_string()), i % 5 == 0));
        } else {
            let c = (1000 + (i % 16)) as u32;
            rows_borrowed.push((i, EnumBorrow::Code(c), i % 5 == 0));
            rows_owned.push((i, RowEnumOwned::Code(c), i % 5 == 0));
        }
    }
    let bytes = encode_rows_u64_enum_bool_adaptive(&rows_borrowed);
    let expected_tag = if should_use_columnar(rows_borrowed.len()) {
        ADAPTIVE_ENUM_TAG_NCB
    } else {
        ADAPTIVE_ENUM_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_enum_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}
