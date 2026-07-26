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
