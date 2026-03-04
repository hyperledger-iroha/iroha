#![cfg(feature = "json")]
//! Tests for `from_json_auto`: small vs large input selection and correctness.

use norito::json::{JsonDeserialize, from_json_auto};

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Item {
    id: u64,
    name: String,
    flag: bool,
}

// Provide the generic typed parser to satisfy the fallback branch
impl JsonDeserialize for Item {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut id = None;
        let mut name = None;
        let mut flag = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => id = Some(p.parse_u64()?),
                "name" => name = Some(p.parse_string()?),
                "flag" => flag = Some(p.parse_bool()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Item {
            id: id.unwrap(),
            name: name.unwrap(),
            flag: flag.unwrap(),
        })
    }
}

#[test]
fn auto_small_prefers_typed() {
    let s = "{\"id\":1,\"name\":\"x\",\"flag\":true}";
    let it: Item = from_json_auto(s).expect("parse");
    assert_eq!(
        it,
        Item {
            id: 1,
            name: "x".to_string(),
            flag: true
        }
    );
}

#[test]
fn auto_large_uses_fast() {
    let long_name = "a".repeat(600);
    let s = format!("{{\"id\":7,\"name\":\"{long_name}\",\"flag\":false}}");
    let it: Item = from_json_auto(&s).expect("parse");
    assert_eq!(
        it,
        Item {
            id: 7,
            name: long_name,
            flag: false
        }
    );
}

// ---- Nested types + floats/bools coverage ----

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Inner {
    value: f64,
    flags: Vec<bool>,
    tags: Vec<String>,
    opt: Option<String>,
}

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Outer {
    id: u64,
    inner: Inner,
    items: Vec<Inner>,
}

impl JsonDeserialize for Inner {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut value = None;
        let mut flags: Option<Vec<bool>> = None;
        let mut tags: Option<Vec<String>> = None;
        let mut opt: Option<Option<String>> = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "value" => value = Some(p.parse_f64()?),
                "flags" => flags = Some(p.parse_array::<bool>()?),
                "tags" => tags = Some(p.parse_array::<String>()?),
                "opt" => {
                    // Option<String>: null or string
                    p.skip_ws();
                    if p.try_consume_null()? {
                        opt = Some(None);
                    } else {
                        opt = Some(Some(p.parse_string()?));
                    }
                }
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Inner {
            value: value.unwrap(),
            flags: flags.unwrap(),
            tags: tags.unwrap(),
            opt: opt.unwrap(),
        })
    }
}

impl JsonDeserialize for Outer {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut id = None;
        let mut inner = None;
        let mut items: Option<Vec<Inner>> = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => id = Some(p.parse_u64()?),
                "inner" => inner = Some(Inner::json_deserialize(p)?),
                "items" => items = Some(p.parse_array::<Inner>()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Outer {
            id: id.unwrap(),
            inner: inner.unwrap(),
            items: items.unwrap(),
        })
    }
}

#[test]
#[allow(clippy::approx_constant)]
fn auto_nested_small() {
    let s = r#"{"id":1,"inner":{"value":3.14,"flags":[true,false,true],"tags":["a","b"],"opt":null},"items":[{"value":2.5,"flags":[false],"tags":["x"],"opt":"y"}]}"#;
    let out: Outer = from_json_auto(s).expect("parse");
    assert_eq!(out.id, 1);
    assert_eq!(out.inner.value, 3.14);
    assert_eq!(out.inner.flags, vec![true, false, true]);
    assert_eq!(out.inner.tags, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(out.inner.opt, None);
    assert_eq!(out.items.len(), 1);
    assert_eq!(out.items[0].opt.as_deref(), Some("y"));
}

#[test]
fn auto_nested_large_equivalence() {
    // Build a large JSON (>256 bytes) with multiple nested items and float/bool variety
    let mut items_json = String::from("[");
    for i in 0..20 {
        if i > 0 {
            items_json.push(',');
        }
        let tag = format!("tag{i}");
        let val = 1.0 + (i as f64) * 0.5;
        let opt = if i % 2 == 0 {
            format!("\"o{i}\"")
        } else {
            "null".to_string()
        };
        items_json.push_str(&format!(
            "{{\"value\":{val},\"flags\":[true,false],\"tags\":[\"{tag}\"],\"opt\":{opt}}}"
        ));
    }
    items_json.push(']');
    let big = format!(
        "{{\"id\":42,\"inner\":{{\"value\":-1.25e2,\"flags\":[false,true,false],\"tags\":[\"p\",\"q\"],\"opt\":\"z\"}},\"items\":{items_json}}}"
    );

    // All paths should agree
    let a: Outer = norito::json::from_json(&big).expect("generic parse");
    let b: Outer = norito::json::from_json_fast(&big).expect("fast parse");
    let c: Outer = from_json_auto(&big).expect("auto parse");
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(a.inner.value, -125.0);
}
