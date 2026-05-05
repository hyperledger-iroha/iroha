#![cfg(feature = "json")]
//! Deterministic tests for `Parser::parse_string`.

use norito::json::{Parser, write_json_string};

#[test]
fn parser_parse_string_matches_quoted_input() {
    let cases = [
        "",
        "plain",
        "\"quoted\"",
        "slash\\slash",
        "line\nbreak",
        "tab\tchar",
        "emoji 😀",
        "cuneiform 𒀭",
    ];

    for value in cases {
        let mut quoted = String::new();
        write_json_string(value, &mut quoted);
        let mut parser = Parser::new(&quoted);
        let got = parser.parse_string().expect("parse string");
        assert_eq!(got, value);
    }
}
