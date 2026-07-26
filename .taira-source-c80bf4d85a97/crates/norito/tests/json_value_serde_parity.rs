#![cfg(feature = "json")]
//! Randomized `norito::json::Value` parity coverage against `serde_json`.

use std::collections::BTreeMap;

use norito::json::{self, Number, Value};
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde_json::{Map as SerdeMap, Number as SerdeNumber, Value as SerdeValue};

const RNG_SEED: u64 = 0x5EED_C0DE_D15E_A5E5;
const RANDOM_CASES: usize = 256;
const MAX_DEPTH: usize = 4;

fn norito_value_to_serde(value: &Value) -> SerdeValue {
    match value {
        Value::Null => SerdeValue::Null,
        Value::Bool(value) => SerdeValue::Bool(*value),
        Value::Number(Number::I64(value)) => SerdeValue::Number(SerdeNumber::from(*value)),
        Value::Number(Number::U64(value)) => SerdeValue::Number(SerdeNumber::from(*value)),
        Value::Number(Number::F64(value)) => {
            if value.is_finite() {
                SerdeValue::Number(SerdeNumber::from_f64(*value).expect("finite f64"))
            } else {
                SerdeValue::Null
            }
        }
        Value::String(value) => SerdeValue::String(value.clone()),
        Value::Array(values) => {
            SerdeValue::Array(values.iter().map(norito_value_to_serde).collect())
        }
        Value::Object(values) => {
            let mut object = SerdeMap::new();
            for (key, value) in values {
                object.insert(key.clone(), norito_value_to_serde(value));
            }
            SerdeValue::Object(object)
        }
    }
}

fn generate_string(rng: &mut StdRng) -> String {
    const FRAGMENTS: &[&str] = &[
        "",
        "alpha",
        "wallet",
        "line\nbreak",
        "tab\tstop",
        "quote:\"",
        "slash\\\\",
        "control:\u{0007}",
        "\u{2028}",
        "\u{2029}",
        "emoji 😀",
        "pi π",
    ];

    let mut rendered = String::new();
    for _ in 0..rng.random_range(0..=4) {
        rendered.push_str(FRAGMENTS[rng.random_range(0..FRAGMENTS.len())]);
    }
    rendered
}

fn generate_finite_f64(rng: &mut StdRng) -> f64 {
    const FIXED_CASES: &[f64] = &[
        -0.0,
        0.0,
        1.0,
        -1.0,
        1.25,
        -42.5,
        1e-6,
        -1e-6,
        core::f64::consts::PI,
        f64::MIN_POSITIVE,
        5e-324,
        9_007_199_254_740_992.0,
    ];

    if rng.random_bool(0.5) {
        FIXED_CASES[rng.random_range(0..FIXED_CASES.len())]
    } else {
        let sign = if rng.random_bool(0.5) { 1.0 } else { -1.0 };
        let magnitude = rng.random_range(0.0..=1_000_000.0);
        let exponent = rng.random_range(-12..=12);
        sign * magnitude * 10f64.powi(exponent)
    }
}

fn generate_scalar(rng: &mut StdRng) -> Value {
    match rng.random_range(0..=5) {
        0 => Value::Null,
        1 => Value::Bool(rng.random()),
        2 => Value::Number(Number::I64(rng.random())),
        3 => Value::Number(Number::U64(rng.random())),
        4 => Value::Number(Number::F64(generate_finite_f64(rng))),
        _ => Value::String(generate_string(rng)),
    }
}

fn generate_value(rng: &mut StdRng, depth: usize) -> Value {
    if depth >= MAX_DEPTH {
        return generate_scalar(rng);
    }

    match rng.random_range(0..=7) {
        0..=5 => generate_scalar(rng),
        6 => Value::Array(
            (0..rng.random_range(0..=4))
                .map(|_| generate_value(rng, depth + 1))
                .collect(),
        ),
        _ => {
            let mut object = BTreeMap::new();
            let target_len = rng.random_range(0..=4);
            while object.len() < target_len {
                let slot = object.len();
                let key = format!("k{depth}_{slot}_{}", rng.random::<u16>());
                object.insert(key, generate_value(rng, depth + 1));
            }
            Value::Object(object)
        }
    }
}

fn explicit_cases() -> Vec<Value> {
    vec![
        Value::Null,
        Value::Bool(true),
        Value::Number(Number::I64(-7)),
        Value::Number(Number::U64(7)),
        Value::Number(Number::F64(-0.0)),
        Value::Number(Number::F64(f64::NAN)),
        Value::Number(Number::F64(f64::INFINITY)),
        Value::Number(Number::F64(f64::NEG_INFINITY)),
        Value::String("seeded \u{2028}parity 😀".to_owned()),
        Value::Array(vec![
            Value::Null,
            Value::Bool(false),
            Value::Number(Number::I64(-9)),
            Value::Number(Number::U64(11)),
            Value::Number(Number::F64(1e-6)),
            Value::String("line\nbreak".to_owned()),
        ]),
        Value::Object(BTreeMap::from([
            (
                "array".to_owned(),
                Value::Array(vec![Value::from(1u64), Value::from("two")]),
            ),
            ("bool".to_owned(), Value::Bool(true)),
            (
                "float".to_owned(),
                Value::Number(Number::F64(core::f64::consts::PI)),
            ),
            (
                "nested".to_owned(),
                Value::Object(BTreeMap::from([
                    ("negative".to_owned(), Value::Number(Number::I64(-3))),
                    ("unicode".to_owned(), Value::String("pi π".to_owned())),
                ])),
            ),
        ])),
    ]
}

fn seeded_corpus() -> Vec<Value> {
    let mut corpus = explicit_cases();
    let mut rng = StdRng::seed_from_u64(RNG_SEED);
    for _ in 0..RANDOM_CASES {
        corpus.push(generate_value(&mut rng, 0));
    }
    corpus
}

#[test]
fn norito_value_corpus_matches_serde_json() {
    for (index, value) in seeded_corpus().iter().enumerate() {
        let serde_value = norito_value_to_serde(value);
        assert_eq!(
            json::to_string(value).expect("norito compact json"),
            serde_json::to_string(&serde_value).expect("serde compact json"),
            "compact case {index} with seed {RNG_SEED:#x}: {value:?}"
        );
        assert_eq!(
            json::to_string_pretty(value).expect("norito pretty json"),
            serde_json::to_string_pretty(&serde_value).expect("serde pretty json"),
            "pretty case {index} with seed {RNG_SEED:#x}: {value:?}"
        );
    }
}
