//! FNV-1a key hashing tests for Norito JSON.
#![cfg(feature = "json")]

use norito::json::{Parser, TapeWalker};

#[test]
fn crc32c_key_hash_parser_and_tapewalker_match() {
    let s = r#"{"simple":"v","es\"caped":1,"uhex":"\u0041"}"#;
    // simple
    let mut p = Parser::new(s);
    p.expect(b'{').unwrap();
    let h1 = p.read_key_hash().expect("hash");
    let mut w = TapeWalker::new(s);
    w.expect_object_start().unwrap();
    let hw = w.read_key_hash().expect("hash");
    assert_eq!(h1, hw);

    // step to next key
    p.expect(b':').unwrap();
    p.skip_value().unwrap();
    p.skip_ws();
    p.expect(b',').unwrap();
    let h2 = p.read_key_hash().expect("hash2");
    assert_ne!(h1, h2);
}

#[test]
fn crc32c_key_hash_surrogate_pair_matches() {
    // Key uses surrogate pair for U+1D11E (MUSICAL SYMBOL G CLEF)
    let s = r#"{"\uD834\uDD1E":1}"#;
    let mut p = Parser::new(s);
    p.expect(b'{').unwrap();
    let h1 = p.read_key_hash().expect("hash");
    let mut w = TapeWalker::new(s);
    w.expect_object_start().unwrap();
    let hw = w.read_key_hash().expect("hash");
    assert_eq!(h1, hw);
}

#[test]
fn key_hash_errors_on_missing_low_surrogate() {
    // Key with high surrogate only should error in both paths
    let s = r#"{"\uD834":1}"#;
    let mut p = Parser::new(s);
    p.expect(b'{').unwrap();
    assert!(p.read_key_hash().is_err());

    let mut w = TapeWalker::new(s);
    w.expect_object_start().unwrap();
    assert!(w.read_key_hash().is_err());
}

#[test]
fn key_hash_errors_on_invalid_hex() {
    // Invalid hex digit in \u sequence
    let s = r#"{"\u00G1":1}"#;
    let mut p = Parser::new(s);
    p.expect(b'{').unwrap();
    assert!(p.read_key_hash().is_err());

    let mut w = TapeWalker::new(s);
    w.expect_object_start().unwrap();
    assert!(w.read_key_hash().is_err());
}
