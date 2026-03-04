//! Validate that compile-time `key_hash_const` matches TapeWalker::read_key_hash
//! when `crc-key-hash` is enabled.
#![cfg(all(feature = "json", feature = "crc-key-hash"))]

#[test]
fn key_hash_const_matches_runtime_crc32c() {
    let s = r#"{"id":1}"#;
    let mut w = norito::json::TapeWalker::new(s);
    w.expect_object_start().unwrap();
    let kh = w.read_key_hash().unwrap();
    assert_eq!(kh, norito::json::key_hash_const("id"));
}
