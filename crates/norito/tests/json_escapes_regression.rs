//! Regression test for the tricky escaped-string sample used in JSON goldens.
//! Guards against accidental edits that break parsing (e.g., missing quotes).

#[test]
fn escaped_string_parses_and_roundtrips() {
    // This exact sample matches the fixed string in json_golden.rs.
    // It contains: newline, tab, backslash, and quotes inside the JSON string.
    let s = r#""escapes\n\t\"\"""#;

    // Parse via Norito and ensure stringify/parse roundtrips cleanly.
    let v: norito::json::Value = norito::json::parse_value(s).expect("parse norito");
    let out = norito::json::to_json(&v).expect("stringify");
    let v2: norito::json::Value = norito::json::parse_value(&out).expect("parse roundtrip");
    assert_eq!(v, v2, "roundtrip value mismatch");
}
