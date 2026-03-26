//! Norito JSON regression tests for identifier types.
#![cfg(feature = "json")]

use iroha_data_model::account::AccountId;
use iroha_data_model::asset::{AssetDefinitionId, AssetId};

const SIGNATORY: &str = "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
const DOMAIN: &str = "wonderland";

fn account_id() -> AccountId {
    AccountId::new(SIGNATORY.parse().expect("public key"))
}

fn asset_id() -> AssetId {
    let domain = DOMAIN.parse().expect("asset domain");
    let account_id = AccountId::new(SIGNATORY.parse().expect("public key"));
    let definition = AssetDefinitionId::new(domain, "xor".parse().expect("asset name"));
    AssetId::new(definition, account_id)
}

#[test]
fn account_id_json_roundtrip() {
    let account_id = account_id();
    let canonical = account_id
        .canonical_i105()
        .expect("canonical Katakana i105");

    let json = norito::json::to_json(&account_id).expect("serialize account id");
    let expected = format!("\"{canonical}\"");
    assert_eq!(json, expected);

    let decoded: AccountId = norito::json::from_json(&json).expect("deserialize account id");
    assert_eq!(decoded.controller(), account_id.controller());
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

#[test]
fn asset_id_json_roundtrip_uses_fully_qualified_definition() {
    let asset_id = asset_id();

    let json = norito::json::to_json(&asset_id).expect("serialize asset id");
    assert!(
        !json.contains('@'),
        "asset JSON must not use legacy @domain account literals: {json}"
    );
    assert!(
        !json.contains("##"),
        "asset JSON must use fully-qualified definition form: {json}"
    );

    let decoded: AssetId = norito::json::from_json(&json).expect("deserialize asset id");
    assert_eq!(decoded, asset_id);
}
