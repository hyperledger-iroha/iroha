#![cfg(feature = "json")]
//! UI-like checks for norito::json::Error Display strings (stable messages).

use norito::json::{self, Parser};

#[test]
fn expected_digits_message() {
    let mut p = Parser::new("x");
    let err = p.parse_u64().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("JSON error: expected digits"), "{msg}");
    assert!(msg.contains("byte 0 (line 1, col 1)"), "{msg}");
}

#[test]
fn expected_null_message() {
    let mut p = Parser::new("x");
    let err = p.parse_null().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("JSON error: expected null"), "{msg}");
    assert!(msg.contains("byte 0 (line 1, col 1)"), "{msg}");
}

#[test]
fn expected_bool_message() {
    let mut p = Parser::new("x");
    let err = p.parse_bool().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("JSON error: expected bool"), "{msg}");
    assert!(msg.contains("byte 0 (line 1, col 1)"), "{msg}");
}

#[test]
fn trailing_characters_message() {
    // Parse a number then leave an extra character
    let s = "1x";
    let res: Result<u64, _> = json::from_json(s);
    let err = res.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("JSON error: trailing characters"), "{msg}");
    // Trailing at byte position 1 (index of 'x')
    assert!(msg.contains("byte 1 (line 1, col 2)"), "{msg}");
}

#[test]
fn unexpected_character_message() {
    let mut p = Parser::new("x");
    let err = p.expect(b'{').unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("JSON error: unexpected character"), "{msg}");
    assert!(msg.contains("byte 0 (line 1, col 1)"), "{msg}");
}

#[test]
fn u64_overflow_message() {
    // One above u64::MAX
    let s = "18446744073709551616";
    let mut p = Parser::new(s);
    let err = p.parse_u64().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("u64 overflow"), "{msg}");
    assert!(msg.contains("byte 0 (line 1, col 1)"), "{msg}");
}

#[test]
fn invalid_hex_in_unescape_message() {
    let s = "\\u00G0"; // invalid hex digit 'G'
    let err = json::unescape_json_string(s).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid hex"), "{msg}");
}

#[test]
fn parse_value_expected_colon_message() {
    // Missing colon in object
    let s = "{\"k\" 1}";
    let err = json::parse_value(s).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected ':'"), "{msg}");
}

#[test]
fn unterminated_string_message() {
    let mut p = Parser::new("\"abc");
    let err = p.parse_string().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unterminated string"), "{msg}");
}

#[test]
fn parser_string_ref_eof_escape_has_position() {
    // Parser::parse_string_ref should report eof escape with position
    let mut p = Parser::new("\"abc\\"); // trailing backslash
    let mut arena = json::Arena::new();
    let res = p.parse_string_ref(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("eof escape"), "{msg}");
    assert!(msg.contains("byte"), "{msg}");
}

#[test]
fn tapewalker_u128_negative_has_position() {
    // TapeWalker::parse_u128_inline should reject negatives with a positioned error
    let s = "-1";
    let mut w = norito::json::TapeWalker::new(s);
    let err = w.parse_u128_inline().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("negative not allowed"), "{msg}");
    assert!(msg.contains("byte 0"), "{msg}");
}
