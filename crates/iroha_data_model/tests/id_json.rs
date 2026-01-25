//! Norito JSON regression tests for identifier types.
#![cfg(feature = "json")]

use iroha_data_model::account::AccountId;

const SIGNATORY: &str = "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
const DOMAIN: &str = "wonderland";

fn account_id_str() -> String {
    format!("{SIGNATORY}@{DOMAIN}")
}

#[test]
fn account_id_json_roundtrip() {
    let raw = account_id_str();
    let parsed = AccountId::parse(&raw).expect("parse account id");
    let (account_id, canonical, _) = parsed.into_parts();

    let json = norito::json::to_json(&account_id).expect("serialize account id");
    assert_eq!(json, format!("\"{canonical}@{}\"", account_id.domain()));

    let decoded: AccountId = norito::json::from_json(&json).expect("deserialize account id");
    assert_eq!(decoded, account_id);
}

#[test]
fn account_id_rejects_object() {
    let json = "{\"AccountId\":\"value\"}";
    let err = norito::json::from_json::<AccountId>(json).expect_err("object should not parse");
    let msg = err.to_string();
    assert!(
        msg.contains("unexpected character"),
        "unexpected error: {msg}"
    );
}
