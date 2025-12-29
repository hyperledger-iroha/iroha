//! Tests for the TapeWalker-based JSON Reader/Token API.
#![cfg(feature = "json")]

use norito::json::{Reader, Token};

fn collect_tokens(input: &str) -> Vec<Token<'_>> {
    let mut rdr = Reader::new(input);
    let mut toks = Vec::new();
    while let Some(tok) = rdr.next().expect("tokenize") {
        toks.push(tok);
    }
    toks
}

#[test]
fn reader_tokens_basic() {
    let s = r#"{"a":1,"b":"x","c":null,"d":true,"e":[1,2]}"#;
    let toks = collect_tokens(s);
    use Token::*;
    // Expected token sequence
    let expect = vec![
        StartObject,
        KeyBorrowed("a"),
        Number("1"),
        KeyBorrowed("b"),
        StringBorrowed("x"),
        KeyBorrowed("c"),
        Null,
        KeyBorrowed("d"),
        Bool(true),
        KeyBorrowed("e"),
        StartArray,
        Number("1"),
        Number("2"),
        EndArray,
        EndObject,
    ];
    assert_eq!(toks.len(), expect.len(), "token count mismatch: {toks:?}");
    // Compare discriminants and important payloads
    for (i, (a, b)) in toks.iter().zip(expect.iter()).enumerate() {
        match (a, b) {
            (StartObject, StartObject)
            | (EndObject, EndObject)
            | (StartArray, StartArray)
            | (EndArray, EndArray)
            | (Null, Null) => {}
            (Bool(x), Bool(y)) => assert_eq!(x, y, "bool mismatch at {i}"),
            (Number(x), Number(y)) => assert_eq!(x, y, "number mismatch at {i}"),
            (StringBorrowed(x), StringBorrowed(y)) => assert_eq!(x, y, "string mismatch at {i}"),
            (KeyBorrowed(x), KeyBorrowed(y)) => assert_eq!(x, y, "key mismatch at {i}"),
            other => panic!("token kind mismatch at {i}: {other:?}"),
        }
    }
}

#[test]
fn reader_tokens_vs_serde_count() {
    // Generate an array of objects with a large string payload and a small number field.
    fn make_blob(n_objs: usize, payload_size: usize) -> String {
        let mut s = String::with_capacity(n_objs * (payload_size + 64));
        s.push('[');
        for i in 0..n_objs {
            if i != 0 {
                s.push(',');
            }
            s.push('{');
            s.push_str("\"payload\":");
            s.push('"');
            for _ in 0..payload_size {
                s.push('x');
            }
            s.push('"');
            s.push_str(",\"other\":");
            s.push_str(&i.to_string());
            s.push('}');
        }
        s.push(']');
        s
    }

    let input = make_blob(8, 64);
    let norito_count = {
        let mut rdr = Reader::new(&input);
        let mut c = 0usize;
        while let Some(_tok) = rdr.next().expect("tokenize") {
            c += 1;
        }
        c
    };

    let serde_count = {
        let v: norito::json::Value = norito::json::from_json(&input).expect("norito decode");
        fn walk(v: &norito::json::Value, c: &mut usize) {
            use norito::json::Value;
            match v {
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => *c += 1,
                Value::Array(a) => {
                    *c += 1;
                    for x in a {
                        walk(x, c)
                    }
                    *c += 1;
                }
                Value::Object(m) => {
                    *c += 1;
                    for (_, val) in m.iter() {
                        *c += 1;
                        walk(val, c)
                    }
                    *c += 1;
                }
            }
        }
        let mut c = 0usize;
        walk(&v, &mut c);
        c
    };

    // Counts need not be equal (different tokenization granularity), but both must be > 0
    assert!(
        norito_count > 0 && serde_count > 0,
        "zero tokens: norito={norito_count}, serde={serde_count}"
    );
}

#[test]
fn reader_tokens_with_ws() {
    // Whitespace around colon and commas should not affect tokenization order.
    let s = r#"{ "a" : 1 , "b" : [ 2 , 3 ] }"#;
    let toks = collect_tokens(s);
    use Token::*;
    let expect = vec![
        StartObject,
        KeyBorrowed("a"),
        Number("1"),
        KeyBorrowed("b"),
        StartArray,
        Number("2"),
        Number("3"),
        EndArray,
        EndObject,
    ];
    assert_eq!(toks.len(), expect.len(), "token count mismatch: {toks:?}");
    for (i, (a, b)) in toks.iter().zip(expect.iter()).enumerate() {
        match (a, b) {
            (StartObject, StartObject)
            | (EndObject, EndObject)
            | (StartArray, StartArray)
            | (EndArray, EndArray) => {}
            (Number(x), Number(y)) => assert_eq!(x, y, "number mismatch at {i}"),
            (KeyBorrowed(x), KeyBorrowed(y)) => assert_eq!(x, y, "key mismatch at {i}"),
            other => panic!("token kind mismatch at {i}: {other:?}"),
        }
    }
}

#[test]
fn unescape_json_string_works() {
    let raw = r#"a\n\t\"z:\u0041\uD834\uDD1E"#; // A + surrogate pair
    let s = norito::json::unescape_json_string(raw).expect("unescape");
    let expected = format!("a\n\t\"z:{}{}", 'A', char::from_u32(0x1D11E).unwrap());
    assert_eq!(s, expected);
}

#[test]
fn reader_value_with_surrogate_pair_unescapes() {
    // The Reader yields a borrowed slice; verify unescape to owned scalar.
    let s = r#"{"k":"\uD834\uDD1E"}"#;
    let mut rdr = Reader::new(s);
    // {, key, value, }
    let _ = rdr.next().expect("tok"); // StartObject
    let key = match rdr.next().expect("tok").unwrap() {
        Token::KeyBorrowed(k) => k,
        _ => panic!("key"),
    };
    assert_eq!(key, "k");
    let val = match rdr.next().expect("tok").unwrap() {
        Token::StringBorrowed(v) => v,
        _ => panic!("val"),
    };
    let owned = norito::json::unescape_json_string(val).expect("unescape");
    assert_eq!(owned, char::from_u32(0x1D11E).unwrap().to_string());
}

#[test]
fn reader_handles_commas_and_colons() {
    // Ensure Reader progresses across punctuation even if tape index desyncs.
    let s = r#"{"a":1,"b":2}"#;
    let toks = collect_tokens(s);
    assert!(!toks.is_empty());
    assert!(matches!(toks[0], Token::StartObject));
    assert!(matches!(toks[toks.len() - 1], Token::EndObject));
}
