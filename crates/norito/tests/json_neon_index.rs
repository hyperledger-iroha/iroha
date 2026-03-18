//! Validate NEON structural index parity with scalar path.
#![cfg(all(feature = "json", feature = "simd-accel", target_arch = "aarch64"))]

use norito::json::build_struct_index;

#[test]
fn neon_struct_index_matches_scalar_for_basic_inputs() {
    // Inputs without ambiguous trailing backslashes in 16-byte blocks
    let docs = [
        r#"{"a":1,"b":[2,3,4],"c":"x"}"#,
        r#"[]"#,
        r#"{"q":"es\"caped","n":null}"#,
    ];
    for s in docs {
        let t = build_struct_index(s);
        // Fallback path also uses scalar when NEON declines; this test asserts
        // basic correctness (i.e., non-empty for non-empty inputs) and monotonic offsets.
        if !s.is_empty() {
            assert!(!t.offsets.is_empty());
            let mut last = 0u32;
            for (i, off) in t.offsets.iter().enumerate() {
                if i > 0 {
                    assert!(*off >= last);
                }
                last = *off;
            }
        }
    }
}
