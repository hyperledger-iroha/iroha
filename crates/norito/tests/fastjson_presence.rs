#![cfg(feature = "json")]
//! Tests for FastJson presence bitset: duplicates, missing required, optional defaults.

use norito::json::{JsonDeserialize, Parser, from_json_fast};

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct DemoOpt {
    id: u64,
    name: String,
    opt: Option<u64>,
}

impl JsonDeserialize for DemoOpt {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut id = None;
        let mut name = None;
        let mut opt: Option<Option<u64>> = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => id = Some(p.parse_u64()?),
                "name" => name = Some(p.parse_string()?),
                "opt" => {
                    p.skip_ws();
                    // Accept null for None; else parse u64
                    if p.input_from_pos().as_bytes().starts_with(b"null") {
                        p.consume_char(b'n')?;
                        p.consume_char(b'u')?;
                        p.consume_char(b'l')?;
                        p.consume_char(b'l')?;
                        opt = Some(None);
                    } else {
                        opt = Some(Some(p.parse_u64()?));
                    }
                }
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(DemoOpt {
            id: id.unwrap(),
            name: name.unwrap(),
            opt: opt.unwrap_or(None),
        })
    }
}

#[test]
fn optional_field_defaults_to_none_when_absent() {
    let s = r#"{"id":1,"name":"x"}"#;
    let got: DemoOpt = from_json_fast(s).expect("parse");
    assert_eq!(
        got,
        DemoOpt {
            id: 1,
            name: "x".to_string(),
            opt: None,
        }
    );
}

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct DemoReq {
    id: u64,
    name: String,
}

impl JsonDeserialize for DemoReq {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut id = None;
        let mut name = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => id = Some(p.parse_u64()?),
                "name" => name = Some(p.parse_string()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(DemoReq {
            id: id.unwrap(),
            name: name.unwrap(),
        })
    }
}

#[test]
fn missing_required_field_errors() {
    let s = r#"{"id":1}"#;
    let err = from_json_fast::<DemoReq>(s).expect_err("should error");
    let msg = format!("{err}");
    assert!(msg.contains("missing field `name`"), "msg was: {msg}");
}

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct DemoDup {
    id: u64,
    name: String,
}

impl JsonDeserialize for DemoDup {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut id = None;
        let mut name = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => id = Some(p.parse_u64()?),
                "name" => name = Some(p.parse_string()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(DemoDup {
            id: id.unwrap(),
            name: name.unwrap(),
        })
    }
}

#[test]
fn duplicate_field_detection() {
    // Duplicate key "id"
    let s = r#"{"id":1,"id":2,"name":"a"}"#;
    let err = from_json_fast::<DemoDup>(s).expect_err("should detect duplicate");
    let msg = format!("{err}");
    assert!(msg.contains("duplicate field `id`"), "msg was: {msg}");
}
