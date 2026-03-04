//! Ensure pretty JSON formatting and position-aware errors work.

use norito::json::{Parser, to_json_pretty};

#[test]
fn pretty_vec_numbers() {
    let v = vec![1u64, 2, 3, 10, 20];
    // Vec<T: JsonSerialize> implements JsonSerialize
    let s = to_json_pretty(&v).expect("pretty json");
    // Expect newlines and two-space indentation, and trailing bracket on its own line
    let expected = "[
  1,
  2,
  3,
  10,
  20
]";
    assert_eq!(s, expected);
}

#[test]
fn pretty_preserves_escapes() {
    let s = String::from("line1\nline2\t\"q\"");
    // Use the string directly (JsonSerialize for String)
    let out = to_json_pretty(&s).expect("pretty json");
    // The entire value is a JSON string; pretty has nothing to indent here.
    assert_eq!(out, "\"line1\\nline2\\t\\\"q\\\"\"");
}

#[test]
fn parser_error_positions_basic() {
    // parse_u64 on non-digit should error with a position near the first non-whitespace
    let mut p = Parser::new("  x");
    let err = p.parse_u64().unwrap_err();
    let msg = err.to_string();
    // Accept both previous and current wording
    assert!(
        msg.contains("JSON error: expected number") || msg.contains("JSON error: expected digits"),
        "{msg}"
    );
    assert!(msg.contains("byte"), "{msg}");
    assert!(msg.contains("line 1, col"), "{msg}");
}

#[test]
fn parser_string_controls_error_position() {
    // Newline inside a string should report a control error with a byte offset
    let mut p = Parser::new("\"a\n");
    let err = p.parse_string().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("JSON error: control in string"), "{msg}");
    assert!(msg.contains("byte"), "{msg}");
}
