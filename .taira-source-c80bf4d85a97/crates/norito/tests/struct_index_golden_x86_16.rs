//! Golden parity tests for 16-byte lane edges on x86_64 (AVX2 path).
#![cfg(all(feature = "json", target_arch = "x86_64"))]

use norito::json::{build_struct_index, build_struct_index_scalar_test};

// Helper: left-pad with spaces to align a target substring index to a 16-byte boundary.
fn pad_to_align(doc: &str, target_sub: &str, lane: usize) -> String {
    let bytes = doc.as_bytes();
    let sub = target_sub.as_bytes();
    let pos = bytes
        .windows(sub.len())
        .position(|w| w == sub)
        .expect("substring present");
    let cur = pos % 16;
    let want = if lane < 16 { lane } else { lane % 16 };
    let pad = (16 + want - cur) % 16;
    let mut s = String::with_capacity(pad + doc.len());
    for _ in 0..pad {
        s.push(' ');
    }
    s.push_str(doc);
    s
}

#[test]
fn avx2_16byte_lane_parity_or_skip() {
    if !std::is_x86_feature_detected!("avx2") {
        // Skip when AVX2 is unavailable
        return;
    }
    let base_docs = vec![
        r#"{}"#.to_string(),
        r#"{"a":1}"#.to_string(),
        r#"{"arr":[1,2,3],"o":{}}"#.to_string(),
        r#"{"s":"x"}"#.to_string(),
        r#"{"s":"a\"b"}"#.to_string(),   // escaped quote in string
        r#"{"s":"c\\\\d"}"#.to_string(), // literal backslashes in string
        r#"{"s":"\u0041"}"#.to_string(), // unicode escape
        r#"{"nested":{"x":[{"y":"z"}]}}"#.to_string(),
    ];
    for doc in base_docs {
        let scalar = build_struct_index_scalar_test(&doc);
        let avx2 = build_struct_index(&doc);
        assert_eq!(scalar.offsets, avx2.offsets, "doc: {}", doc);
    }

    // Crafted lane-edge cases (16-byte)
    let crafted = vec![
        pad_to_align(r#"{"k":"a\\\"b"}"#, r#"\""#, 15), // '\\' at 15, '"' at 16
        pad_to_align(r#"{"k":"c\\\"d"}"#, r#"\""#, 0),  // aligns quote at lane start
        pad_to_align(r#"{"k":"e\\\\\\\"f"}"#, r#"\""#, 15),
    ];
    for doc in crafted {
        let scalar = build_struct_index_scalar_test(&doc);
        let avx2 = build_struct_index(&doc);
        assert_eq!(scalar.offsets, avx2.offsets, "crafted doc: {}", doc);
    }
}
