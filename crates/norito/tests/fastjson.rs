//! Minimal FastJson/FastJsonWrite derive demo over TapeWalker.
#![cfg(feature = "json")]

use norito::json::{FastFromJson, FastJsonWrite, TapeWalker};

#[derive(Debug, Clone, PartialEq, Eq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Demo {
    id: u64,
    name: String,
    tags: Vec<String>,
    opt: Option<u64>,
}

#[test]
fn fastjson_roundtrip() {
    let d = Demo {
        id: 7,
        name: "alice".to_string(),
        tags: vec!["a".into(), "b".into()],
        opt: Some(9),
    };
    let mut out = String::new();
    d.write_json(&mut out);
    let mut w = TapeWalker::new(&out);
    let mut arena = norito::json::Arena::new();
    let got = <Demo as FastFromJson>::parse(&mut w, &mut arena).expect("parse");
    assert_eq!(d, got);
}

#[test]
fn fastjson_unknown_fields_use_strict_bounded_skip() {
    let over_limit = format!(
        "{}null{}",
        "[".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH),
        "]".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH)
    );
    let input = format!(r#"{{"id":7,"name":"alice","tags":[],"opt":null,"unknown":{over_limit}}}"#);
    let mut walker = TapeWalker::new(&input);
    let mut arena = norito::json::Arena::new();
    assert!(matches!(
        <Demo as FastFromJson>::parse(&mut walker, &mut arena),
        Err(norito::Error::Json(
            norito::json::Error::NestingDepthExceeded { .. }
        ))
    ));

    let malformed = r#"{"id":7,"name":"alice","tags":[],"opt":null,"unknown":[}}"#;
    let mut walker = TapeWalker::new(malformed);
    let mut arena = norito::json::Arena::new();
    assert!(<Demo as FastFromJson>::parse(&mut walker, &mut arena).is_err());
}
