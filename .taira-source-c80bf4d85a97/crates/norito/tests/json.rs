//! JSON helper tests for Norito (manual, no serde in lib).
#![cfg(feature = "json")]

use norito::{
    json as nj,
    json::{JsonDeserialize, JsonSerialize, Parser, from_json, write_json_string},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sample {
    id: u64,
    name: String,
    tags: Vec<String>,
    flag: bool,
    maybe: Option<u32>,
}

impl JsonSerialize for Sample {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        // id
        out.push_str("\"id\":");
        self.id.json_serialize(out);
        out.push(',');
        // name
        out.push_str("\"name\":");
        write_json_string(&self.name, out);
        out.push(',');
        // tags
        out.push_str("\"tags\":");
        self.tags.json_serialize(out);
        out.push(',');
        // flag
        out.push_str("\"flag\":");
        self.flag.json_serialize(out);
        out.push(',');
        // maybe
        out.push_str("\"maybe\":");
        self.maybe.json_serialize(out);
        out.push('}');
    }
}

impl JsonDeserialize for Sample {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.expect(b'{')?;
        let mut id: Option<u64> = None;
        let mut name: Option<String> = None;
        let mut tags: Option<Vec<String>> = None;
        let mut flag: Option<bool> = None;
        let mut maybe: Option<Option<u32>> = None;
        p.skip_ws();
        if matches!(p.peek(), Some(b'}')) {
            p.bump();
        } else {
            loop {
                let key = p.parse_string()?;
                p.skip_ws();
                p.expect(b':')?;
                match key.as_str() {
                    "id" => id = Some(u64::json_deserialize(p)?),
                    "name" => name = Some(String::json_deserialize(p)?),
                    "tags" => tags = Some(Vec::<String>::json_deserialize(p)?),
                    "flag" => flag = Some(bool::json_deserialize(p)?),
                    "maybe" => maybe = Some(Option::<u32>::json_deserialize(p)?),
                    _ => return Err(norito::json::Error::Message("unknown key".into())),
                }
                p.skip_ws();
                match p.bump() {
                    Some(b',') => {
                        p.skip_ws();
                        continue;
                    }
                    Some(b'}') => break,
                    _ => return Err(norito::json::Error::Message("expected , or }".into())),
                }
            }
        }
        Ok(Sample {
            id: id.ok_or_else(|| norito::json::Error::Message("missing id".into()))?,
            name: name.ok_or_else(|| norito::json::Error::Message("missing name".into()))?,
            tags: tags.unwrap_or_default(),
            flag: flag.unwrap_or(false),
            maybe: maybe.unwrap_or(None),
        })
    }
}

#[test]
fn json_roundtrip_sample() {
    let v = Sample {
        id: 42,
        name: "alice".to_string(),
        tags: vec!["a".into(), "b".into(), "c".into()],
        flag: true,
        maybe: Some(7),
    };
    let mut s = String::new();
    v.json_serialize(&mut s);
    let out: Sample = from_json(&s).expect("json decode");
    assert_eq!(v, out);
}

#[test]
fn typed_api_roundtrip() {
    let v = Sample {
        id: 7,
        name: "bob".to_string(),
        tags: vec!["x".into(), "y".into()],
        flag: false,
        maybe: None,
    };
    let json = nj::to_json(&v).expect("to_json");
    let back: Sample = nj::from_json(&json).expect("from_json");
    assert_eq!(v, back);

    let buf = nj::to_vec(&v).expect("to_vec");
    let back_slice: Sample = nj::from_slice(&buf).expect("from_slice");
    assert_eq!(v, back_slice);

    let value = nj::to_value(&v).expect("to_value");
    let back_value: Sample = nj::from_value(value).expect("from_value");
    assert_eq!(v, back_value);
}

#[test]
fn raw_value_api_compiles() {
    // Ensure RawValue/to_raw_value are available via norito::json::value
    let val = norito::json!({ "k": [1, 2, 3], "s": "t" });
    use norito::json::value;
    let raw: Box<value::RawValue> = value::to_raw_value(&val).expect("to_raw_value");
    // Parsing from RawValue back into a Value
    let parsed: nj::Value = nj::from_str(raw.get()).expect("from_str raw");
    assert_eq!(parsed, val);
}

#[test]
fn value_deserialize_roundtrip() {
    let src = r#"{"a":[1,true,{"b":null}],"c":"str"}"#;
    let parsed: nj::Value = nj::from_json(src).expect("parse value");
    // JSON renderer is canonical for our Value, so reparsing should yield the same structure
    let rendered = nj::to_string(&parsed).expect("render value");
    let reparsed: nj::Value = nj::from_json(&rendered).expect("reparse value");
    assert_eq!(parsed, reparsed);
}
