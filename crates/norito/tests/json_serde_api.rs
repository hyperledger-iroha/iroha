#![cfg(feature = "json")]
//! Representative smoke/regression parity tests for the serde-style JSON API.

use std::collections::BTreeMap;

use norito::json;
use serde::Serialize;

#[derive(Debug, norito::JsonSerialize, Serialize)]
struct TypedChild {
    ok: bool,
    note: &'static str,
    ratio: f64,
}

#[derive(Debug, norito::JsonSerialize, Serialize)]
struct TypedPayload {
    id: u64,
    label: &'static str,
    tags: Vec<&'static str>,
    child: TypedChild,
    aliases: BTreeMap<&'static str, &'static str>,
}

#[derive(Debug, norito::JsonSerialize, Serialize)]
struct FloatEnvelope {
    finite: f64,
    tiny: f64,
    neg_zero: f64,
    nan: f64,
    pos_inf: f64,
    neg_inf: f64,
}

fn assert_matches_serde<T>(value: &T)
where
    T: norito::json::JsonSerialize + Serialize + ?Sized,
{
    assert_eq!(
        json::to_string(value).expect("norito compact json"),
        serde_json::to_string(value).expect("serde compact json")
    );
    assert_eq!(
        json::to_string_pretty(value).expect("norito pretty json"),
        serde_json::to_string_pretty(value).expect("serde pretty json")
    );
}

#[test]
fn typed_payload_smoke_matches_serde_json() {
    let aliases = BTreeMap::from([
        ("default", "demo@paynet"),
        ("fi", "ops@hbl.paynet"),
        ("merchant", "shop@ubl.paynet"),
    ]);
    let payload = TypedPayload {
        id: 7,
        label: "demo\u{2028}wallet \u{1f60a}",
        tags: vec!["alpha", "line\nbreak", "quote:\""],
        child: TypedChild {
            ok: true,
            note: "control:\u{0007}",
            ratio: 1e-6,
        },
        aliases,
    };

    assert_matches_serde(&payload);
}

#[test]
fn float_edge_case_regression_matches_serde_json() {
    let payload = FloatEnvelope {
        finite: 1.0,
        tiny: 1e-6,
        neg_zero: -0.0,
        nan: f64::NAN,
        pos_inf: f64::INFINITY,
        neg_inf: f64::NEG_INFINITY,
    };

    assert_matches_serde(&payload);
}

#[test]
fn deterministic_btreemap_regression_matches_serde_json() {
    let payload = BTreeMap::from([
        ("alias", "mint-signer1@paynet"),
        ("escaped", "tab\tnewline\nslash\\"),
        ("unicode", "emoji \u{1f389}"),
    ]);

    assert_matches_serde(&payload);
}
