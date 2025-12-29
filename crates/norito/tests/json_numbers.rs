//! Number tokenization tests for the JSON Reader.
#![cfg(feature = "json")]

use norito::json::{Reader, Token};

#[test]
fn numbers_tokenization_variants() {
    let s = r#"{"a":-1,"b":0.0,"c":1.25,"d":-2.5e10,"e":3E+5,"f":6e-7}"#;
    let mut rdr = Reader::new(s);
    let mut toks = Vec::new();
    while let Some(tok) = rdr.next().expect("tokenize") {
        toks.push(tok);
    }
    use Token::*;
    let expect = vec![
        StartObject,
        KeyBorrowed("a"),
        Number("-1"),
        KeyBorrowed("b"),
        Number("0.0"),
        KeyBorrowed("c"),
        Number("1.25"),
        KeyBorrowed("d"),
        Number("-2.5e10"),
        KeyBorrowed("e"),
        Number("3E+5"),
        KeyBorrowed("f"),
        Number("6e-7"),
        EndObject,
    ];
    assert_eq!(toks.len(), expect.len(), "count mismatch: {toks:?}");
    for (i, (a, b)) in toks.iter().zip(expect.iter()).enumerate() {
        match (a, b) {
            (StartObject, StartObject) | (EndObject, EndObject) => {}
            (KeyBorrowed(x), KeyBorrowed(y)) => assert_eq!(x, y, "key mismatch at {i}"),
            (Number(x), Number(y)) => assert_eq!(x, y, "num mismatch at {i}"),
            other => panic!("kind mismatch at {i}: {other:?}"),
        }
    }
}

#[test]
fn tape_skip_value_numbers_array() {
    let s = r#"[1,-2,3.14,4e5]"#;
    let mut rdr = Reader::new(s);
    // First token should be StartArray, then skip two numbers and ensure we still progress
    assert!(matches!(rdr.next().unwrap().unwrap(), Token::StartArray));
    assert!(matches!(rdr.next().unwrap().unwrap(), Token::Number("1")));
    assert!(matches!(rdr.next().unwrap().unwrap(), Token::Number("-2")));
    assert!(matches!(
        rdr.next().unwrap().unwrap(),
        Token::Number("3.14")
    ));
    assert!(matches!(rdr.next().unwrap().unwrap(), Token::Number("4e5")));
    assert!(matches!(rdr.next().unwrap().unwrap(), Token::EndArray));
    assert!(rdr.next().unwrap().is_none());
}
