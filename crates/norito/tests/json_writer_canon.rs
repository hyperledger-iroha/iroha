//! JSON writer canonicalizes floats and encodes non-finite values as null.

use norito::json::{self, Value, native::Number};

#[test]
fn json_float_formatting_is_canonical() {
    let value = Value::Number(Number::F64(1.0));
    assert_eq!(json::to_string(&value).expect("json"), "1.0");

    let value = Value::Number(Number::F64(1.25));
    assert_eq!(json::to_string(&value).expect("json"), "1.25");
}

#[test]
fn json_float_non_finite_serializes_as_null() {
    let value = Value::Number(Number::F64(f64::NAN));
    assert_eq!(json::to_string(&value).expect("json"), "null");

    let value = Value::Number(Number::F64(f64::INFINITY));
    assert_eq!(json::to_string(&value).expect("json"), "null");

    let value = Value::Number(Number::F64(f64::NEG_INFINITY));
    assert_eq!(json::to_string(&value).expect("json"), "null");
}

#[test]
fn json_float_roundtrip_examples() {
    let cases = [-0.0, 1.0, core::f64::consts::PI, 5e-324];
    for value in cases {
        let value = Value::Number(Number::F64(value));
        let rendered = json::to_string(&value).expect("json");
        let parsed = json::parse_value(&rendered).expect("parse");
        let got = parsed.as_f64().expect("float");
        assert_eq!(got.to_bits(), value.as_f64().unwrap().to_bits());
    }
}
