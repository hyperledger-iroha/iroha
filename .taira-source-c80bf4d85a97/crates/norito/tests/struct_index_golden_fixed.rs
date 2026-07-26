//! Pinned golden outputs for structural index, decoupled from scalar.
#![cfg(feature = "json")]

use norito::json::build_struct_index;

#[test]
fn golden_small_docs() {
    // 1. {}
    let d1 = "{}";
    let t1 = build_struct_index(d1);
    assert_eq!(t1.offsets, vec![0, 1]);

    // 2. {"a":1}
    let d2 = "{\"a\":1}";
    let t2 = build_struct_index(d2);
    assert_eq!(t2.offsets, vec![0, 1, 3, 4, 6]);

    // 3. {"s":"x"}
    let d3 = "{\"s\":\"x\"}";
    let t3 = build_struct_index(d3);
    assert_eq!(t3.offsets, vec![0, 1, 3, 4, 5, 7, 8]);

    // 4. {"s":"a\"b"}
    let d4 = "{\"s\":\"a\\\"b\"}";
    let t4 = build_struct_index(d4);
    assert_eq!(t4.offsets, vec![0, 1, 3, 4, 5, 10, 11]);

    // 5. {"s":"\\\\"} (two backslashes)
    let d5 = "{\"s\":\"\\\\\\\\\"}"; // JSON: \\ \\ inside quotes
    let t5 = build_struct_index(d5);
    // Offsets: { (0), " (1), s (2), " (3), : (4), " (5), ... , closing " and }
    // Concrete indices: 0,1,3,4,5,10,11
    assert_eq!(t5.offsets, vec![0, 1, 3, 4, 5, 10, 11]);
}
