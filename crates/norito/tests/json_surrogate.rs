//! Surrogate pair handling tests for Norito JSON parser and helpers.
#![cfg(feature = "json")]

use norito::json::{Arena, Parser, TapeWalker, unescape_json_string};

#[test]
fn parse_string_surrogate_pair_ok() {
    let s = r#""\uD834\uDD1E""#; // U+1D11E MUSICAL SYMBOL G CLEF
    let mut p = Parser::new(s);
    let out = p.parse_string().expect("parse pair");
    assert_eq!(out, char::from_u32(0x1D11E).unwrap().to_string());
}

#[test]
fn parse_string_unexpected_low_surrogate_err() {
    let s = r#""\uDD1E""#;
    let mut p = Parser::new(s);
    assert!(p.parse_string().is_err());
}

#[test]
fn parse_string_missing_low_surrogate_err() {
    let s = r#""\uD834x""#;
    let mut p = Parser::new(s);
    assert!(p.parse_string().is_err());
}

#[test]
fn skip_value_handles_surrogate_pair() {
    // Parser skip
    let s = r#"{"k":"\uD834\uDD1E"}"#;
    let mut p = Parser::new(s);
    p.expect(b'{').unwrap();
    // skip key
    p.skip_string().unwrap();
    p.expect(b':').unwrap();
    // skip value string (with pair)
    p.skip_value().unwrap();

    // TapeWalker skip
    let mut w = TapeWalker::new(s);
    w.expect_object_start().unwrap();
    let _ = w.read_key_hash().unwrap();
    w.expect_colon().unwrap();
    w.skip_value().unwrap();
}

#[test]
fn unescape_json_string_surrogate_pair_ok() {
    let raw = r#"\uD834\uDD1E"#;
    let out = unescape_json_string(raw).expect("unescape");
    assert_eq!(out, char::from_u32(0x1D11E).unwrap().to_string());
}

#[test]
fn unescape_json_string_surrogate_errors() {
    // Unexpected low surrogate
    assert!(unescape_json_string(r#"\uDD1E"#).is_err());
    // Missing low surrogate after high
    assert!(unescape_json_string(r#"\uD834x"#).is_err());
}

#[test]
fn tape_parse_string_ref_surrogate_pair_ok() {
    let s = r#""\uD834\uDD1E""#; // U+1D11E
    let mut w = TapeWalker::new(s);
    let mut arena = Arena::new();
    let r = w
        .parse_string_ref_inline(&mut arena)
        .expect("parse string ref");
    let got = r.to_string();
    assert_eq!(got, char::from_u32(0x1D11E).unwrap().to_string());
}

#[test]
fn parser_parse_string_ref_surrogate_pair_ok() {
    let s = r#""\uD834\uDD1E""#; // U+1D11E
    let mut p = Parser::new(s);
    let mut arena = Arena::new();
    let r = p.parse_string_ref(&mut arena).expect("parse string ref");
    let got = r.to_string();
    assert_eq!(got, char::from_u32(0x1D11E).unwrap().to_string());
}
