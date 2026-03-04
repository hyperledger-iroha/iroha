#![cfg(feature = "json")]
//! Property-like tests for Norito JSON string parsing and skipping.

use norito::json::{Arena, Parser, TapeWalker, from_json_fast, write_json_string};
use proptest::prelude::*;

fn json_quote(s: &str) -> String {
    let mut out = String::new();
    write_json_string(s, &mut out);
    out
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, max_shrink_time: 1000, .. ProptestConfig::default() })]
    #[test]
    fn parse_string_matches_input(ref s in ".{0,64}") {
        let quoted = json_quote(s);
        let mut p = Parser::new(&quoted);
        let out = p.parse_string().expect("parse string");
        prop_assert_eq!(out, s.as_str());
    }
}

#[test]
fn parse_string_ref_inline_matches_input() {
    // Deterministic cases covering escapes, unicode, and edge conditions.
    let cases = vec![
        "",
        "a",
        "abc",
        "\"", // quote in content
        "\\", // backslash
        "quote\"inside",
        "back\\slash",
        "line\nbreak",
        "tab\tchar",
        "carriage\rreturn",
        "π",
        "😀",
        "mix 😀 π ascii",
        "multi\\\\slashes",
        "braces{}not structural",
    ];
    for s in cases {
        let quoted = json_quote(s);
        let mut w = TapeWalker::new(&quoted);
        let mut arena = Arena::new();
        let out = w
            .parse_string_ref_inline(&mut arena)
            .expect("inline string");
        let got = match out {
            norito::json::StrRef::Borrowed(b) => b.to_string(),
            norito::json::StrRef::Owned(o) => o.to_string(),
        };
        assert_eq!(got, s);
        // Ensure we advanced to end (or only whitespace remains)
        w.skip_ws();
        assert_eq!(w.raw_pos(), quoted.len());
    }
}

#[test]
fn skip_value_advances_to_next() {
    let a = json_quote("hello");
    let b = json_quote("world");
    let s = format!("[{a},{b}]");
    let mut w = TapeWalker::new(&s);
    // Move past '['
    if let Some((off, ch)) = w.peek_struct() {
        assert_eq!(ch, b'[');
        let _ = w.next_struct();
        w.sync_to_raw(off + 1);
    }
    // Skip first value
    w.skip_value().expect("skip first");
    // Consume comma
    let _ = w.consume_comma_if_present().expect("comma");
    // Parse second value
    let mut arena = Arena::new();
    let sr = w.parse_string_ref_inline(&mut arena).expect("second");
    let s2 = match sr {
        norito::json::StrRef::Borrowed(b) => b.to_string(),
        norito::json::StrRef::Owned(o) => o.to_string(),
    };
    assert_eq!(s2, "world");
}

#[test]
fn from_json_fast_equivalence_simple_struct() {
    #[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
    struct S {
        id: u64,
        name: String,
        ok: bool,
    }

    impl norito::json::JsonDeserialize for S {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
            p.skip_ws();
            p.consume_char(b'{')?;
            let mut id = None;
            let mut name = None;
            let mut ok = None;
            loop {
                p.skip_ws();
                if p.try_consume_char(b'}')? {
                    break;
                }
                let k = p.parse_key()?;
                match k.as_ref() {
                    "id" => id = Some(p.parse_u64()?),
                    "name" => name = Some(p.parse_string()?),
                    "ok" => ok = Some(p.parse_bool()?),
                    _ => return Err(norito::json::Error::Message("unexpected key".into())),
                }
                let _ = p.consume_comma_if_present()?;
            }
            Ok(S {
                id: id.unwrap(),
                name: name.unwrap(),
                ok: ok.unwrap(),
            })
        }
    }

    let s = r#"{"id":123,"name":"abc","ok":true}"#;
    let a: S = norito::json::from_json(s).expect("generic");
    let b: S = from_json_fast(s).expect("fast");
    assert_eq!(a, b);
}
