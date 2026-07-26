#![cfg(feature = "json")]
//! Deterministic equivalence tests for Norito's JSON fast path.

use norito::json::{JsonDeserialize, Parser, from_json_fast};

#[derive(Clone, Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Inner {
    count: u32,
    title: String,
}

impl JsonDeserialize for Inner {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;
        let mut count = None;
        let mut title = None;
        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }
            let key = parser.parse_key()?;
            match key.as_ref() {
                "count" => count = Some(parser.parse_u64().map(|value| value as u32)?),
                "title" => title = Some(parser.parse_string()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = parser.consume_comma_if_present()?;
        }
        Ok(Inner {
            count: count.unwrap(),
            title: title.unwrap(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Outer {
    id: u64,
    name: String,
    active: bool,
    ratio: f64,
    tags: Vec<String>,
    nested: Option<Inner>,
    nums: Vec<u64>,
}

impl JsonDeserialize for Outer {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;
        let mut id = None;
        let mut name = None;
        let mut active = None;
        let mut ratio = None;
        let mut tags: Option<Vec<String>> = None;
        let mut nested: Option<Option<Inner>> = None;
        let mut nums: Option<Vec<u64>> = None;
        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }
            let key = parser.parse_key()?;
            match key.as_ref() {
                "id" => id = Some(parser.parse_u64()?),
                "name" => name = Some(parser.parse_string()?),
                "active" => active = Some(parser.parse_bool()?),
                "ratio" => ratio = Some(parser.parse_f64()?),
                "tags" => tags = Some(parser.parse_array::<String>()?),
                "nested" => {
                    parser.skip_ws();
                    if parser.try_consume_null()? {
                        nested = Some(None);
                    } else {
                        nested = Some(Some(Inner::json_deserialize(parser)?));
                    }
                }
                "nums" => nums = Some(parser.parse_array::<u64>()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = parser.consume_comma_if_present()?;
        }
        Ok(Outer {
            id: id.unwrap(),
            name: name.unwrap(),
            active: active.unwrap(),
            ratio: ratio.unwrap(),
            tags: tags.unwrap_or_default(),
            nested: nested.unwrap_or(None),
            nums: nums.unwrap_or_default(),
        })
    }
}

fn outer_cases() -> Vec<Outer> {
    vec![
        Outer {
            id: 0,
            name: String::new(),
            active: false,
            ratio: 0.0,
            tags: Vec::new(),
            nested: None,
            nums: Vec::new(),
        },
        Outer {
            id: 1,
            name: "iroha".to_owned(),
            active: true,
            ratio: 1.25,
            tags: vec!["ledger".to_owned(), "json".to_owned()],
            nested: Some(Inner {
                count: 7,
                title: "inner".to_owned(),
            }),
            nums: vec![0, 1, 2, u64::MAX],
        },
        Outer {
            id: u64::MAX,
            name: "line\nbreak".to_owned(),
            active: false,
            ratio: -42.5,
            tags: vec!["π".to_owned(), "😀".to_owned()],
            nested: Some(Inner {
                count: u32::MAX,
                title: "\"quoted\"".to_owned(),
            }),
            nums: vec![10, 20, 30],
        },
    ]
}

#[test]
fn from_json_fast_matches_generic_for_outer() {
    for value in outer_cases() {
        let json = norito::json::to_json(&value).expect("serialize json");
        let slow: Outer = norito::json::from_json(&json).expect("generic parse");
        let fast: Outer = from_json_fast(&json).expect("fast parse");
        assert_eq!(slow, fast);
    }
}

#[test]
fn tape_parse_string_ref_inline_matches_input() {
    let cases = ["", "plain", "line\nbreak", "\"quoted\"", "π", "😀"];

    for value in cases {
        let quoted = norito::json::to_json(&norito::json::Value::String(value.to_owned())).unwrap();
        let mut walker = norito::json::TapeWalker::new(&quoted);
        let mut arena = norito::json::Arena::new();
        let out = walker
            .parse_string_ref_inline(&mut arena)
            .expect("string ref");
        let got = match out {
            norito::json::StrRef::Borrowed(value) => value.to_string(),
            norito::json::StrRef::Owned(value) => value.to_string(),
        };
        assert_eq!(got, value);
    }
}

#[test]
fn tape_skip_value_advances_to_second_string() {
    let cases = [
        ("", "second"),
        ("first", ""),
        ("line\nbreak", "\"quoted\""),
        ("π", "😀"),
    ];

    for (first, second) in cases {
        let first_json =
            norito::json::to_json(&norito::json::Value::String(first.to_owned())).unwrap();
        let second_json =
            norito::json::to_json(&norito::json::Value::String(second.to_owned())).unwrap();
        let json = format!("[{first_json},{second_json}]");
        let mut walker = norito::json::TapeWalker::new(&json);
        if let Some((offset, ch)) = walker.peek_struct() {
            assert_eq!(ch, b'[');
            let _ = walker.next_struct();
            walker.sync_to_raw(offset + 1);
        }
        walker.skip_value().expect("skip first");
        let _ = walker.consume_comma_if_present().expect("comma");
        let mut arena = norito::json::Arena::new();
        let out = walker
            .parse_string_ref_inline(&mut arena)
            .expect("second string");
        let got = match out {
            norito::json::StrRef::Borrowed(value) => value.to_string(),
            norito::json::StrRef::Owned(value) => value.to_string(),
        };
        assert_eq!(got, second);
    }
}
