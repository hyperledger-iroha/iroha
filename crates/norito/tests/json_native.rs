#![cfg(feature = "json")]

use norito::json::{self, CoerceKey, JsonDeserialize, MapVisitor, RawValue, SeqVisitor};

#[derive(Debug, PartialEq, norito::JsonSerialize, norito::JsonDeserialize)]
struct DerivedExample {
    id: u64,
    #[norito(rename = "label")]
    name: String,
    #[norito(default)]
    flag: bool,
    optional: Option<String>,
}

fn default_port() -> u16 {
    8080
}

#[derive(Clone, Debug, PartialEq, norito::JsonSerialize, norito::JsonDeserialize)]
struct DerivedDefaults {
    #[norito(default = "default_port")]
    port: u16,
    #[norito(default)]
    tags: Vec<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[test]
fn derived_json_roundtrip() {
    let value = DerivedExample {
        id: 7,
        name: "demo".to_owned(),
        flag: true,
        optional: Some("x".to_owned()),
    };

    let rendered = json::to_json(&value).expect("render json");
    assert_eq!(
        rendered,
        "{\"id\":7,\"label\":\"demo\",\"flag\":true,\"optional\":\"x\"}"
    );

    let decoded: DerivedExample = json::from_json(&rendered).expect("from_json");
    assert_eq!(decoded, value);

    let missing = r#"{"id":1,"label":"mini"}"#;
    let fallback: DerivedExample = json::from_json(missing).expect("defaults");
    assert_eq!(fallback.id, 1);
    assert_eq!(fallback.name, "mini");
    assert!(!fallback.flag);
    assert!(fallback.optional.is_none());
}

#[test]
fn derived_json_default_fn_and_skip_serializing_if() {
    let value = DerivedDefaults {
        port: default_port(),
        tags: Vec::new(),
        note: None,
    };

    // When `note` is `None`, the field is omitted entirely.
    let rendered = json::to_json(&value).expect("serialize without note");
    assert_eq!(rendered, "{\"port\":8080,\"tags\":[]}");

    // Missing fields fall back to defaults (function + Default::default).
    let decoded: DerivedDefaults = json::from_json("{}").expect("defaults via derive");
    assert_eq!(decoded.port, 8080);
    assert!(decoded.tags.is_empty());
    assert!(decoded.note.is_none());

    // Populating the optional field reintroduces it in the output.
    let mut with_note = value.clone();
    with_note.note = Some("hi".to_owned());
    let rendered_note = json::to_json(&with_note).expect("serialize with note");
    assert_eq!(rendered_note, "{\"port\":8080,\"tags\":[],\"note\":\"hi\"}");
}

#[derive(Debug, PartialEq)]
struct ManualConfig {
    threshold: u64,
    enabled: bool,
}

impl JsonDeserialize for ManualConfig {
    fn json_deserialize(p: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut map = MapVisitor::new(p)?;
        let mut threshold: Option<u64> = None;
        let mut enabled: Option<bool> = None;
        while let Some(key) = map.next_key()? {
            match key.as_str() {
                "threshold" => {
                    if threshold.is_some() {
                        return Err(MapVisitor::duplicate_field("threshold"));
                    }
                    threshold = Some(map.parse_value::<u64>()?);
                }
                "enabled" => {
                    if enabled.is_some() {
                        return Err(MapVisitor::duplicate_field("enabled"));
                    }
                    // Exercise the visitor path
                    struct BoolVisitor;
                    impl<'a> json::Visitor<'a> for BoolVisitor {
                        type Value = bool;
                        fn visit_null(self) -> Result<Self::Value, json::Error> {
                            Err(json::Error::Message("expected bool".into()))
                        }
                        fn visit_bool(self, v: bool) -> Result<Self::Value, json::Error> {
                            Ok(v)
                        }
                        fn visit_i64(self, _: i64) -> Result<Self::Value, json::Error> {
                            Err(json::Error::Message("expected bool".into()))
                        }
                        fn visit_u64(self, _: u64) -> Result<Self::Value, json::Error> {
                            Err(json::Error::Message("expected bool".into()))
                        }
                        fn visit_f64(self, _: f64) -> Result<Self::Value, json::Error> {
                            Err(json::Error::Message("expected bool".into()))
                        }
                        fn visit_string(self, _: String) -> Result<Self::Value, json::Error> {
                            Err(json::Error::Message("expected bool".into()))
                        }
                        fn visit_map(
                            self,
                            mut map: MapVisitor<'a, '_>,
                        ) -> Result<Self::Value, json::Error> {
                            while map.next_key()?.is_some() {
                                map.skip_value()?;
                            }
                            map.finish()?;
                            Err(json::Error::Message("expected bool".into()))
                        }
                        fn visit_seq(
                            self,
                            mut seq: SeqVisitor<'a, '_>,
                        ) -> Result<Self::Value, json::Error> {
                            while seq.next_element::<RawValue>()?.is_some() {}
                            seq.finish()?;
                            Err(json::Error::Message("expected bool".into()))
                        }
                    }
                    enabled = Some(map.parse_value_with(BoolVisitor)?);
                }
                _ => map.skip_value()?,
            }
        }
        map.finish()?;
        Ok(Self {
            threshold: threshold.ok_or_else(|| MapVisitor::missing_field("threshold"))?,
            enabled: enabled.unwrap_or(false),
        })
    }
}

#[test]
fn map_visitor_manual_defaults() {
    let cfg: ManualConfig = json::from_json(r#"{"threshold":10}"#).expect("parse config");
    assert_eq!(
        cfg,
        ManualConfig {
            threshold: 10,
            enabled: false
        }
    );

    let dup = json::from_json::<ManualConfig>(r#"{"threshold":1,"threshold":2}"#);
    assert!(dup.unwrap_err().to_string().contains("duplicate field"));
}

fn parse_numeric_keys(input: &str) -> Result<Vec<(u64, bool)>, json::Error> {
    let mut parser = json::Parser::new(input);
    let mut map = MapVisitor::new(&mut parser)?;
    let mut pairs = Vec::new();
    while let Some(key) = map.next_key()? {
        let id = CoerceKey::from(key).parse::<u64>()?;
        let flag = map.parse_value::<bool>()?;
        pairs.push((id, flag));
    }
    map.finish()?;
    Ok(pairs)
}

#[test]
fn coerce_key_handles_numeric_object_keys() {
    let parsed = parse_numeric_keys(r#"{"1":true,"42":false}"#).expect("coerce keys");
    assert_eq!(parsed, vec![(1, true), (42, false)]);
}

fn sum_array(input: &str) -> Result<u64, json::Error> {
    let mut parser = json::Parser::new(input);
    let mut seq = SeqVisitor::new(&mut parser)?;
    let mut total = 0u64;
    while let Some(v) = seq.next_element::<u64>()? {
        total += v;
    }
    seq.finish()?;
    Ok(total)
}

#[test]
fn seq_visitor_accumulates() {
    let total = sum_array("[1,2,3,4]").expect("sum array");
    assert_eq!(total, 10);
}

#[test]
fn raw_value_captures_slice() {
    let mut parser = json::Parser::new(r#"{"payload":{"a":[1,2,3]}}"#);
    let mut map = MapVisitor::new(&mut parser).expect("map visitor");
    let mut captured: Option<Box<RawValue>> = None;
    while let Some(key) = map.next_key().expect("next key") {
        match key.as_str() {
            "payload" => {
                captured = Some(map.parse_value::<Box<RawValue>>().expect("raw"));
            }
            _ => map.skip_value().expect("skip"),
        }
    }
    map.finish().expect("finish map");
    let raw = captured.expect("captured raw");
    assert_eq!(raw.get(), "{\"a\":[1,2,3]}");
    let value = json::value::from_raw_value(&raw).expect("raw to value");
    assert_eq!(value, norito::json!({"a": [1, 2, 3]}));
}

struct KindVisitor;

impl<'a> json::Visitor<'a> for KindVisitor {
    type Value = &'static str;

    fn visit_null(self) -> Result<Self::Value, json::Error> {
        Ok("null")
    }

    fn visit_bool(self, _: bool) -> Result<Self::Value, json::Error> {
        Ok("bool")
    }

    fn visit_i64(self, _: i64) -> Result<Self::Value, json::Error> {
        Ok("i64")
    }

    fn visit_u64(self, _: u64) -> Result<Self::Value, json::Error> {
        Ok("u64")
    }

    fn visit_f64(self, _: f64) -> Result<Self::Value, json::Error> {
        Ok("f64")
    }

    fn visit_string(self, _: String) -> Result<Self::Value, json::Error> {
        Ok("string")
    }

    fn visit_map(self, mut map: MapVisitor<'a, '_>) -> Result<Self::Value, json::Error> {
        while map.next_key()?.is_some() {
            map.skip_value()?;
        }
        map.finish()?;
        Ok("map")
    }

    fn visit_seq(self, mut seq: SeqVisitor<'a, '_>) -> Result<Self::Value, json::Error> {
        while seq.next_element::<RawValue>()?.is_some() {}
        seq.finish()?;
        Ok("seq")
    }
}

#[test]
fn visit_value_classifies_scalars() {
    let mut num_parser = json::Parser::new("42");
    assert_eq!(
        json::visit_value(&mut num_parser, KindVisitor).expect("num"),
        "u64"
    );

    let mut bool_parser = json::Parser::new("true");
    assert_eq!(
        json::visit_value(&mut bool_parser, KindVisitor).expect("bool"),
        "bool"
    );

    let mut arr_parser = json::Parser::new("[null]");
    assert_eq!(
        json::visit_value(&mut arr_parser, KindVisitor).expect("seq"),
        "seq"
    );
}
