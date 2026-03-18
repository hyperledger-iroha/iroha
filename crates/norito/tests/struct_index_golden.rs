//! Golden parity tests for NEON Stage-1 structural index vs scalar reference.
#![cfg(all(feature = "json", feature = "simd-accel", target_arch = "aarch64"))]

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
fn neon_matches_scalar_on_corpus() {
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

    // Variants crafted to put a backslash immediately before a quote at a block edge
    let crafted = vec![
        pad_to_align(r#"{"k":"a\\\"b"}"#, r#"\""#, 15), // '\\' at 15, '"' at 16
        pad_to_align(r#"{"k":"c\\\"d"}"#, r#"\""#, 0), // '\\' before '"' crosses boundary differently
        pad_to_align(r#"{"k":"e\\\\\\\"f"}"#, r#"\""#, 15), // 3 backslashes then escaped quote
    ];

    let mut docs = base_docs;
    docs.extend(crafted);

    for doc in docs {
        let scalar = build_struct_index_scalar_test(&doc);
        let neon = build_struct_index(&doc);
        assert_eq!(scalar.offsets, neon.offsets, "doc: {}", doc);
    }
}
