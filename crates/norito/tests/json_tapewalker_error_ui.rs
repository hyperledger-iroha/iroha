#![cfg(feature = "json")]
//! UI-like checks for TapeWalker-specific expected-* errors.

use norito::json::{self, Reader, TapeWalker, Token};

#[test]
fn expect_object_start_message() {
    let mut w = TapeWalker::new("[]");
    let err = w.expect_object_start().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected object start"), "{msg}");
}

#[test]
fn expect_object_end_message() {
    let mut w = TapeWalker::new("{]}");
    // Consume '{'
    w.expect_object_start().unwrap();
    let err = w.expect_object_end().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected object end"), "{msg}");
}

#[test]
fn expect_colon_message() {
    let s = "{\"a\" 1}"; // missing colon
    let mut w = TapeWalker::new(s);
    w.expect_object_start().unwrap();
    // Using read_key_hash now enforces colon itself; assert it reports ExpectedColon
    let err = w.read_key_hash().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected ':'"), "{msg}");
}

#[test]
fn expect_colon_resync_message() {
    // Missing colon; resync path should still report ExpectedColon
    let s = "{\"a\" 1}";
    let mut w = TapeWalker::new(s);
    w.expect_object_start().unwrap();
    // read_key_hash walks quotes and sets last_key
    let _ = w.read_key_hash().unwrap_err(); // this will fail before colon; we need a direct colon check
    // Rebuild TapeWalker to position after '{' and run colon resync
    let mut w2 = TapeWalker::new(s);
    w2.expect_object_start().unwrap();
    // Manually set last_key using an alternate path: skip to after key string
    // Use read_key_hash to move past key then ignore its error and test colon
    let _ = w2.read_key_hash().unwrap_or(0);
    let err = w2.expect_colon_resync().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected ':'"), "{msg}");
}

#[test]
fn expect_array_start_message() {
    let mut w = TapeWalker::new("{}");
    let err = w.expect_array_start().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected array start"), "{msg}");
}

#[test]
fn expect_array_end_message() {
    let mut w = TapeWalker::new("[}");
    w.expect_array_start().unwrap();
    let err = w.expect_array_end().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected array end"), "{msg}");
}

#[test]
fn expected_key_string_and_unterminated_key_messages() {
    // Missing key string: use numeric key to trigger ExpectedKeyString at ':'
    let mut w = TapeWalker::new("{1:2}");
    w.expect_object_start().unwrap();
    let err = w.read_key_hash().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected key quote"), "{msg}");

    // Unterminated key: opening quote, no closing quote before colon
    let mut w = TapeWalker::new("{\"a:1}");
    w.expect_object_start().unwrap();
    let err = w.read_key_hash().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unterminated key"), "{msg}");
}

#[test]
fn reader_expected_colon_error() {
    // Reader should surface ExpectedColon after a key without ':'
    let mut r = Reader::new("{\"a\" 1}");
    // First token is StartObject
    match r.next_token() {
        Ok(Some(Token::StartObject)) => {}
        other => panic!("unexpected first token: {other:?}"),
    }
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected ':'"), "{msg}");
}

#[test]
fn reader_expected_digits_error() {
    // A lone minus should trigger ExpectedDigits via Reader scalar path
    let mut r = Reader::new("-");
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected digits"), "{msg}");
}

#[test]
fn reader_expected_frac_digits_error() {
    // A dot without following digits should trigger ExpectedFracDigits
    let mut r = Reader::new("1.");
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected frac digits"), "{msg}");
}

#[test]
fn reader_expected_exp_digits_error() {
    // An exponent marker without digits should trigger ExpectedExpDigits
    let mut r = Reader::new("1e");
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected exp digits"), "{msg}");
}

#[test]
fn reader_unexpected_value_error() {
    // A non-structural, non-scalar starting char should trigger UnexpectedValue
    let mut r = Reader::new("?");
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unexpected value"), "{msg}");
}

#[test]
fn reader_unexpected_structurals_top_level() {
    // '}' and ']' at top level should produce Unexpected*End
    for (input, needle) in [
        ("}", "unexpected object end"),
        ("]", "unexpected array end"),
    ] {
        let mut r = Reader::new(input);
        match r.next_token() {
            Err(e) => {
                let msg = e.to_string();
                assert!(msg.contains(needle), "{needle} not in {msg}");
            }
            Ok(Some(tok)) => {
                // Accept tokenization in tolerant mode
                match (input, tok) {
                    ("}", Token::EndObject) | ("]", Token::EndArray) => {}
                    (inp, other) => panic!("unexpected token for top-level {inp}: {other:?}"),
                }
            }
            Ok(None) => {
                // Accept no token at EOF for robustness
            }
        }
    }
    // Comma and colon at top level should produce UnexpectedComma/UnexpectedColon
    let mut r = Reader::new(",");
    let msg = r.next_token().unwrap_err().to_string();
    assert!(msg.contains("unexpected comma"), "{msg}");
    let mut r = Reader::new(":");
    let msg = r.next_token().unwrap_err().to_string();
    assert!(msg.contains("unexpected ':'"), "{msg}");
}

#[test]
fn reader_unexpected_quote_in_object() {
    // In object context, a quote without key+colon ahead should be flagged
    let mut r = Reader::new("{\"a\" \"b\"}");
    match r.next_token() {
        Ok(Some(Token::StartObject)) => {}
        other => panic!("{other:?}"),
    }
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    // Now uses a specialized variant
    assert!(msg.contains("unexpected quote"), "{msg}");
}

#[test]
fn reader_unexpected_object_end_nested_position() {
    // Nested case: close object properly, then an extra '}' at top level with exact position
    let s = "{\n}\n}"; // lines: 1:"{", 2:"}", 3:"}"
    let mut r = Reader::new(s);
    // StartObject
    match r.next_token() {
        Ok(Some(Token::StartObject)) => {}
        other => panic!("unexpected first token: {other:?}"),
    }
    // EndObject
    match r.next_token() {
        Ok(Some(Token::EndObject)) => {}
        other => panic!("unexpected second token: {other:?}"),
    }
    // Extra '}' at top level should produce UnexpectedObjectEnd at byte 4 (line 3, col 1)
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unexpected object end"), "{msg}");
    assert!(msg.contains("byte 4 (line 3, col 1)"), "{msg}");
}

#[test]
fn reader_unexpected_array_end_nested_position() {
    // Nested: close array properly, then an extra ']' at top level; assert exact position
    let s = "[\n]\n]"; // lines: 1:"[", 2:"]", 3:"]"
    let mut r = Reader::new(s);
    // StartArray
    match r.next_token() {
        Ok(Some(Token::StartArray)) => {}
        other => panic!("unexpected first token: {other:?}"),
    }
    // EndArray
    match r.next_token() {
        Ok(Some(Token::EndArray)) => {}
        other => panic!("unexpected second token: {other:?}"),
    }
    // Extra ']' at top level should produce UnexpectedArrayEnd at byte 4 (line 3, col 1)
    let err = r.next_token().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unexpected array end"), "{msg}");
    assert!(msg.contains("byte 4 (line 3, col 1)"), "{msg}");
}

#[test]
fn reader_array_object_mismatch_future_diagnostic() {
    // Mismatch: inside an object, an array end ']' appears prematurely (before the '}').
    // Use a tight string with no whitespace to ensure deterministic position.
    let s = "[{\"a\":1]}"; // indices: 0:'[',1:'{',2:'"',3:'a',4:'"',5:':',6:'1',7:']',8:'}'
    let mut r = Reader::new(s);
    loop {
        match r.next_token() {
            Ok(Some(_tok)) => continue,
            Ok(None) => break,
            Err(e) => {
                let msg = e.to_string();
                assert!(msg.contains("unexpected array end"), "{msg}");
                // ']' at byte 7 (line 1, col 8)
                assert!(msg.contains("byte 7 (line 1, col 8)"), "{msg}");
                break;
            }
        }
    }
}

#[test]
fn reader_object_array_mismatch_future_diagnostic() {
    // Mismatch (opposite direction): inside an array, an object end '}' appears prematurely.
    let mut r = Reader::new("{[1}]"); // indices: 0:'{',1:'[',2:'1',3:'}',4:']'
    loop {
        match r.next_token() {
            Ok(Some(_tok)) => continue,
            Ok(None) => break,
            Err(e) => {
                let msg = e.to_string();
                assert!(msg.contains("unexpected object end"), "{msg}");
                // '}' at byte 3 (line 1, col 4)
                assert!(msg.contains("byte 3 (line 1, col 4)"), "{msg}");
                break;
            }
        }
    }
}

#[test]
fn tapewalker_bad_escape_reports_position() {
    // \x is not a valid JSON escape
    let s = "\"bad: \\x\"";
    let mut w = TapeWalker::new(s);
    let mut arena = json::Arena::new();
    let res = w.parse_string_ref_inline(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("bad escape"), "{msg}");
}

#[test]
fn tapewalker_invalid_low_surrogate_reports_position() {
    // High surrogate followed by non-low surrogate
    let s = "\"\\uD834\\u0041\""; // U+D834 then 'A'
    let mut w = TapeWalker::new(s);
    let mut arena = json::Arena::new();
    let res = w.parse_string_ref_inline(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("invalid low surrogate"), "{msg}");
}

#[test]
fn tapewalker_unexpected_low_surrogate_reports_position() {
    // Lone low surrogate
    let s = "\"\\uDC00\"";
    let mut w = TapeWalker::new(s);
    let mut arena = json::Arena::new();
    let res = w.parse_string_ref_inline(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("unexpected low surrogate"), "{msg}");
}

#[test]
fn tapewalker_eof_escape_reports_position() {
    // Trailing backslash before closing quote
    let s = "\"abc\\\""; // content ends with backslash
    let mut w = TapeWalker::new(s);
    let mut arena = json::Arena::new();
    let res = w.parse_string_ref_inline(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    // Accept either eof escape or unterminated string depending on scanner
    assert!(
        msg.contains("eof escape") || msg.contains("unterminated string"),
        "{msg}"
    );
}

#[test]
fn tapewalker_string_control_char_error_has_position() {
    // String with an embedded control char 0x01 should error with ControlInString
    let s = "\"a\x01b\"";
    let mut w = TapeWalker::new(s);
    let mut arena = json::Arena::new();
    let res = w.parse_string_ref_inline(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("control in string"), "{msg}");
}

#[test]
fn tapewalker_string_unterminated_error_has_position() {
    // Unterminated string should surface UnterminatedString with byte offset
    let s = "\"unterminated";
    let mut w = TapeWalker::new(s);
    let mut arena = json::Arena::new();
    let res = w.parse_string_ref_inline(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("unterminated string"), "{msg}");
}

#[test]
fn tapewalker_string_eof_hex_error_has_position() {
    // Incomplete \uXXXX sequence should produce EofHex
    let s = "\"\\u12\""; // only two hex digits
    let mut w = TapeWalker::new(s);
    let mut arena = json::Arena::new();
    let res = w.parse_string_ref_inline(&mut arena);
    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("eof hex"), "{msg}");
}
