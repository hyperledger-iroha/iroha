#![cfg(feature = "json")]
use norito::json::{
    self, CoerceKey, JsonDeserialize, MapVisitor, Number, RawValue, SeqVisitor, Value,
};
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
#[derive(Debug, PartialEq, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
enum DerivedTagged {
    Unit,
    Number(u64),
}
#[test]
fn derived_tagged_json_rejects_duplicate_envelope_fields() {
    assert_eq!(
        json::to_json(&DerivedTagged::Unit).expect("serialize renamed unit variant"),
        r#"{"kind":"unit","value":null}"#
    );
    for input in [
        r#"{"kind":"unit","kind":"number","value":null}"#,
        r#"{"kind":"unit","value":null,"value":null}"#,
    ] {
        let error = json::from_json::<DerivedTagged>(input)
            .expect_err("duplicate tag or content must fail");
        assert!(error.to_string().contains("duplicate field"));
    }
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
                        fn visit_u128(self, _: u128) -> Result<Self::Value, json::Error> {
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
#[test]
fn map_visitor_rejects_trailing_object_commas() {
    for input in [r#"{"threshold":10,}"#, "{\"threshold\":10, \n }"] {
        let error = json::from_json::<ManualConfig>(input)
            .expect_err("typed map visitor must reject a trailing object comma");
        assert!(error.to_string().contains("trailing comma"));
    }
    let mut parser = json::Parser::new(r#"{"threshold":10,}"#);
    let mut map = MapVisitor::new(&mut parser).expect("map visitor");
    let key = map.next_key().expect("first key").expect("threshold key");
    assert_eq!(key.as_str(), "threshold");
    assert_eq!(map.parse_value::<u64>().expect("threshold value"), 10);
    let error = map
        .finish()
        .expect_err("finish must not bypass trailing-comma validation");
    assert!(error.to_string().contains("trailing comma"));
}
#[test]
fn parser_skip_string_bounded_counts_exact_decoded_utf8_bytes() {
    for (input, expected) in [
        (r#""""#, 0),
        (r#""abc""#, 3),
        (r#""¢""#, 2),
        (r#""€""#, 3),
        (r#""😀""#, 4),
        (r#""\"\\\/\b\f\n\r\t""#, 8),
        (r#""\u007F""#, 1),
        (r#""\u0080""#, 2),
        (r#""\u07FF""#, 2),
        (r#""\u0800""#, 3),
        (r#""\uD7FF""#, 3),
        (r#""\uE000""#, 3),
        (r#""\uFFFF""#, 3),
        (r#""\uD83D\uDE00""#, 4),
        (r#""\u0061é""#, 3),
    ] {
        let mut bounded = json::Parser::new(input);
        assert_eq!(
            bounded
                .skip_string_bounded(expected)
                .expect("bounded string must parse"),
            expected
        );
        assert_eq!(bounded.position(), input.len());
        let mut owned = json::Parser::new(input);
        let decoded = owned.parse_string().expect("ordinary string parser");
        assert_eq!(decoded.len(), expected);
        assert_eq!(owned.position(), bounded.position());
        if expected > 0 {
            let mut one_under = json::Parser::new(input);
            let error = one_under
                .skip_string_bounded(expected - 1)
                .expect_err("one byte below the exact decoded length must reject");
            assert!(
                error
                    .to_string()
                    .contains(&format!("{}-byte limit", expected - 1))
            );
            let _remaining = one_under.input_from_pos();
        }
    }
}
#[test]
fn parser_skip_string_bounded_matches_malformed_string_rejection() {
    for input in [
        "\"\\",
        r#""\u12""#,
        r#""\u00G0""#,
        r#""\uDC00""#,
        r#""\uD800""#,
        r#""\uD800\u0041""#,
        "\"a\nb\"",
    ] {
        let mut bounded = json::Parser::new(input);
        assert!(
            bounded.skip_string_bounded(usize::MAX).is_err(),
            "bounded parser must reject malformed input {input:?}"
        );
        let mut owned = json::Parser::new(input);
        assert!(
            owned.parse_string().is_err(),
            "ordinary parser must reject malformed input {input:?}"
        );
    }
}
#[test]
fn parser_bounded_string_error_preserves_utf8_boundary() {
    for (input, maximum) in [(r#""€""#, 0), (r#""\uD83D\uDE00""#, 3)] {
        let mut parser = json::Parser::new(input);
        parser
            .skip_string_bounded(maximum)
            .expect_err("the decoded scalar exceeds the byte limit");
        assert_eq!(parser.input_from_pos(), r#"""#);
    }
}
#[test]
#[should_panic(expected = "JSON parser start must be a UTF-8 character boundary")]
fn parser_new_at_rejects_mid_scalar_offsets() {
    let _ = json::Parser::new_at("€", 1);
}
#[test]
#[should_panic(expected = "JSON parser start must be a UTF-8 character boundary")]
fn parser_new_at_rejects_out_of_range_offsets() {
    let _ = json::Parser::new_at("x", 2);
}
#[test]
fn parser_bump_at_eof_is_stable() {
    let mut parser = json::Parser::new("");
    assert_eq!(parser.bump(), None);
    assert_eq!(parser.bump(), None);
    assert_eq!(parser.position(), 0);
    assert_eq!(parser.input_from_pos(), "");
}
#[test]
fn generic_json_value_rejects_duplicate_object_fields() {
    let err = json::from_json::<json::Value>(r#"{"encrypted_input":"a","encrypted_input":"b"}"#)
        .expect_err("generic Value parsing must reject duplicate object fields");
    assert!(err.to_string().contains("encrypted_input"));
}
#[test]
fn generic_json_value_rejects_nested_duplicate_object_fields() {
    let err = json::from_json::<json::Value>(
        r#"{"output_opening":{"payload":{"program_id":"p","program_id":"q"}}}"#,
    )
    .expect_err("generic Value parsing must reject nested duplicate object fields");
    assert!(err.to_string().contains("program_id"));
}
#[test]
fn generic_json_value_rejects_duplicate_object_fields_inside_arrays() {
    let err =
        json::from_json::<json::Value>(r#"[{"policy_id":"policy-a","policy_id":"policy-b"}]"#)
            .expect_err("generic Value parsing must reject duplicate object fields inside arrays");
    assert!(err.to_string().contains("policy_id"));
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
fn seq_visitor_rejects_trailing_array_commas() {
    for input in ["[1,]", "[1, \n ]"] {
        let error = sum_array(input).expect_err("typed sequence must reject a trailing comma");
        assert!(error.to_string().contains("trailing comma"));
    }
    let mut parser = json::Parser::new("[1, \n ]");
    let mut sequence = SeqVisitor::new(&mut parser).expect("sequence visitor");
    assert_eq!(
        sequence.next_element::<u64>().expect("first element"),
        Some(1)
    );
    let error = sequence
        .finish()
        .expect_err("finish must not bypass trailing-comma validation");
    assert!(error.to_string().contains("trailing comma"));
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
    fn visit_u128(self, _: u128) -> Result<Self::Value, json::Error> {
        Ok("u128")
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
    assert!(matches!(
        json::parse_value("42").expect("owned unsigned integer"),
        Value::Number(Number::U64(42))
    ));
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

struct ExactUnsignedVisitor;

impl<'a> json::Visitor<'a> for ExactUnsignedVisitor {
    type Value = u128;

    fn visit_null(self) -> Result<Self::Value, json::Error> {
        Err(json::Error::Message("expected unsigned integer".into()))
    }

    fn visit_bool(self, _: bool) -> Result<Self::Value, json::Error> {
        Err(json::Error::Message("expected unsigned integer".into()))
    }

    fn visit_i64(self, value: i64) -> Result<Self::Value, json::Error> {
        u128::try_from(value).map_err(|_| json::Error::Message("expected unsigned integer".into()))
    }

    fn visit_u64(self, value: u64) -> Result<Self::Value, json::Error> {
        Ok(u128::from(value))
    }

    fn visit_u128(self, value: u128) -> Result<Self::Value, json::Error> {
        Ok(value)
    }

    fn visit_f64(self, _: f64) -> Result<Self::Value, json::Error> {
        Err(json::Error::Message("expected unsigned integer".into()))
    }

    fn visit_string(self, _: String) -> Result<Self::Value, json::Error> {
        Err(json::Error::Message("expected unsigned integer".into()))
    }

    fn visit_map(self, _: MapVisitor<'a, '_>) -> Result<Self::Value, json::Error> {
        Err(json::Error::Message("expected unsigned integer".into()))
    }

    fn visit_seq(self, _: SeqVisitor<'a, '_>) -> Result<Self::Value, json::Error> {
        Err(json::Error::Message("expected unsigned integer".into()))
    }
}

fn assert_exact_u128_value(value: &Value, expected: u128) {
    assert_eq!(value, &Value::Number(Number::U128(expected)));
    assert_eq!(value.as_u128(), Some(expected));
    assert_eq!(value.as_u64(), u64::try_from(expected).ok());

    let Value::Number(number) = value else {
        panic!("u128 JSON value must remain a number");
    };
    assert_eq!(number.as_u128(), Some(expected));
}

#[test]
fn u128_owned_value_paths_preserve_exact_integer_text() {
    for expected in [u128::from(u64::MAX) + 1, u128::MAX] {
        let expected_json = expected.to_string();

        let direct = Value::from(expected);
        assert_exact_u128_value(&direct, expected);

        let macro_value = norito::json!(expected);
        assert_exact_u128_value(&macro_value, expected);

        let converted = json::to_value(&expected).expect("serialize u128 into owned value");
        assert_exact_u128_value(&converted, expected);

        let parsed = json::parse_value(&expected_json).expect("parse exact u128 JSON value");
        assert_exact_u128_value(&parsed, expected);
        assert_eq!(
            json::to_json(&parsed).expect("render exact u128 JSON value"),
            expected_json
        );
        assert_eq!(
            json::value::from_value::<u128>(parsed).expect("decode owned u128 JSON value"),
            expected
        );
    }
}

#[test]
fn u128_streaming_visitor_preserves_values_above_u64() {
    for expected in [u128::from(u64::MAX) + 1, u128::MAX] {
        let input = expected.to_string();
        let mut parser = json::Parser::new(&input);
        assert_eq!(
            json::visit_value(&mut parser, ExactUnsignedVisitor)
                .expect("stream exact u128 JSON value"),
            expected
        );
        assert_eq!(parser.position(), input.len());

        let mut kind_parser = json::Parser::new(&input);
        assert_eq!(
            json::visit_value(&mut kind_parser, KindVisitor).expect("classify exact u128"),
            "u128"
        );
    }
}

#[test]
fn integer_larger_than_u128_is_rejected_without_float_fallback() {
    const OVERFLOW: &str = "340282366920938463463374607431768211456";
    const NEGATIVE_OVERFLOW: &str = "-9223372036854775809";

    let typed_error =
        json::from_json::<u128>(OVERFLOW).expect_err("typed u128 overflow must reject");
    assert!(typed_error.to_string().contains("u128 overflow"));

    let dom_error = json::parse_value(OVERFLOW)
        .expect_err("owned integer overflow must not fall back to an inexact float");
    assert!(dom_error.to_string().contains("integer out of range"));

    let mut parser = json::Parser::new(OVERFLOW);
    let visitor_error = json::visit_value(&mut parser, KindVisitor)
        .expect_err("streaming integer overflow must reject before visiting an f64");
    assert!(visitor_error.to_string().contains("integer out of range"));

    assert!(json::parse_value(NEGATIVE_OVERFLOW).is_err());
    let mut parser = json::Parser::new(NEGATIVE_OVERFLOW);
    assert!(json::visit_value(&mut parser, KindVisitor).is_err());
}

#[test]
fn u128_and_f64_equality_requires_the_same_exact_integer() {
    const FIRST_INEXACT_F64_INTEGER: u128 = (1_u128 << 53) + 1;
    const NEXT_EXACT_F64_INTEGER: u128 = (1_u128 << 53) + 2;

    assert_eq!(
        Number::U128(1_u128 << 53),
        Number::F64((1_u128 << 53) as f64)
    );
    assert_ne!(
        Number::U128(FIRST_INEXACT_F64_INTEGER),
        Number::F64(FIRST_INEXACT_F64_INTEGER as f64)
    );
    assert_ne!(
        Number::U64(FIRST_INEXACT_F64_INTEGER as u64),
        Number::F64(FIRST_INEXACT_F64_INTEGER as f64)
    );
    assert_ne!(
        Number::I64(FIRST_INEXACT_F64_INTEGER as i64),
        Number::F64(FIRST_INEXACT_F64_INTEGER as f64)
    );
    assert_eq!(
        Number::U128(NEXT_EXACT_F64_INTEGER),
        Number::F64(NEXT_EXACT_F64_INTEGER as f64)
    );
    assert_ne!(Number::U128(u128::MAX), Number::F64(u128::MAX as f64));

    assert_eq!(Number::U128(i64::MAX as u128).as_i64(), Some(i64::MAX));
    assert_eq!(Number::U128(i64::MAX as u128 + 1).as_i64(), None);
}
