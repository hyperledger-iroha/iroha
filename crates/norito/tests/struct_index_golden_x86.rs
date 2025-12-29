//! Golden parity tests for AVX2 Stage-1 structural index vs scalar reference (x86_64).
#![cfg(all(feature = "json", target_arch = "x86_64"))]

use norito::json::build_struct_index;

// Helper: left-pad with spaces to align a target substring index to a 32-byte boundary.
fn pad_to_align(doc: &str, target_sub: &str, lane: usize) -> String {
    let bytes = doc.as_bytes();
    let sub = target_sub.as_bytes();
    let pos = bytes
        .windows(sub.len())
        .position(|w| w == sub)
        .expect("substring present");
    let cur = pos % 32;
    let want = if lane < 32 { lane } else { lane % 32 };
    let pad = (32 + want - cur) % 32;
    let mut s = String::with_capacity(pad + doc.len());
    for _ in 0..pad {
        s.push(' ');
    }
    s.push_str(doc);
    s
}

#[cfg(feature = "json")]
fn build_struct_index_scalar(doc: &str) -> norito::json::StructIndex {
    norito::json::build_struct_index_scalar_test(doc)
}

#[test]
fn avx2_matches_scalar_on_examples_or_skips() {
    if !std::is_x86_feature_detected!("avx2") {
        // Skip on machines without AVX2 support
        return;
    }
    let docs = vec![
        r#"{}"#,
        r#"{"a":1}"#,
        r#"{"arr":[1,2,3],"o":{}}"#,
        r#"{"s":"x"}"#,
        r#"{"s":"a\"b"}"#,
        r#"{"s":"c\\\\d"}"#,
        r#"{"s":"\u0041"}"#,
        r#"{"nested":{"x":[{"y":"z"}]}}"#,
    ];
    for d in docs {
        let scalar = build_struct_index_scalar(d);
        let avx2 = build_struct_index(d);
        assert_eq!(scalar.offsets, avx2.offsets, "doc: {}", d);
    }

    // Crafted cases to test 32-byte lane edges and escaped quotes
    let crafted = vec![
        pad_to_align(r#"{"k":"a\\\"b"}"#, r#"\""#, 31), // '\\' at 31, '"' at 32
        pad_to_align(r#"{"k":"c\\\"d"}"#, r#"\""#, 0),  // aligns quote at lane start
        pad_to_align(r#"{"k":"e\\\\\\\"f"}"#, r#"\""#, 31),
    ];
    for doc in crafted {
        let scalar = build_struct_index_scalar(&doc);
        let avx2 = build_struct_index(&doc);
        assert_eq!(scalar.offsets, avx2.offsets, "crafted doc: {}", doc);
    }
}
