#![cfg(feature = "json")]
//! Verify that `Parser::parse_key` consumes the trailing colon.

use norito::json::Parser;

#[test]
fn parse_key_consumes_colon() {
    let s = "{\"id\":123}";
    let mut p = Parser::new(s);
    // Enter object
    p.consume_char(b'{').unwrap();
    let k = p.parse_key().expect("key");
    assert_eq!(k.as_ref(), "id");
    // Should be positioned at the start of the value now
    let n = p.parse_u64().expect("u64");
    assert_eq!(n, 123);
}
