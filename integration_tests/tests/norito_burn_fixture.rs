#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! JS Norito fixture parity with Rust canonical encoding.

use std::{fs, path::PathBuf, str::FromStr};

use iroha_data_model::prelude::{
    AccountId, AssetDefinitionId, AssetId, Burn, InstructionBox, Mint, Numeric, TriggerId,
};
use norito::codec::{Decode, Encode};

const FIXTURE_PUBLIC_KEY: &str =
    "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";

fn fixture_asset_id() -> AssetId {
    let public_key: iroha_data_model::PublicKey = FIXTURE_PUBLIC_KEY
        .parse()
        .expect("valid fixture public key");
    let account = AccountId::new(public_key);
    let definition: AssetDefinitionId = "rose#wonderland".parse().expect("valid asset definition");
    AssetId::new(definition, account)
}

#[derive(Debug)]
struct InstructionFixture {
    id: String,
    instruction: norito::json::Value,
    encoded_hex: String,
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("norito_instructions")
        .join(name)
}

fn load_fixture(name: &str) -> InstructionFixture {
    let raw = fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|err| panic!("read {name} fixture: {err}"));
    let value: norito::json::Value =
        norito::json::from_str(&raw).expect("parse Norito fixture as JSON");
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{name} JSON must be an object"));

    let fixture_id = object
        .get("fixture_id")
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("{name} fixture_id must be a string"))
        .to_owned();
    let encoded_hex = object
        .get("encoded_hex")
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("{name} encoded_hex must be a string"))
        .to_owned();
    let instruction = object
        .get("instruction")
        .cloned()
        .unwrap_or_else(|| panic!("{name} must include instruction JSON"));

    InstructionFixture {
        id: fixture_id,
        instruction,
        encoded_hex,
    }
}

fn assert_fixture_matches(name: &str, expected_id: &str, expected_instruction: &InstructionBox) {
    let fixture = load_fixture(name);
    assert_eq!(
        fixture.id, expected_id,
        "fixture {name} id mismatch (expected {expected_id})"
    );

    let expected_instruction_json = norito::json::to_value(expected_instruction)
        .expect("serialize canonical Norito instruction JSON");
    assert_eq!(
        fixture.instruction, expected_instruction_json,
        "fixture {name} instruction payload should match canonical Norito JSON"
    );

    let mut canonical_bytes = Vec::new();
    expected_instruction.encode_to(&mut canonical_bytes);
    let canonical_hex = hex::encode(&canonical_bytes);
    assert_eq!(
        fixture.encoded_hex, canonical_hex,
        "fixture {name} encoded_hex mismatch; canonical hex is {canonical_hex}"
    );

    let decoded =
        InstructionBox::decode(&mut canonical_bytes.as_slice()).expect("decode bytes via Norito");
    assert_eq!(
        decoded,
        expected_instruction.clone(),
        "decoded instruction should match expected canonical Norito instruction"
    );

    let mut roundtrip = Vec::new();
    decoded.encode_to(&mut roundtrip);
    assert_eq!(
        roundtrip, canonical_bytes,
        "Rust Norito encoding must match fixture bytes"
    );
}

#[test]
fn burn_asset_fixture_matches_rust_encoding() {
    let expected_asset_id = fixture_asset_id();
    let expected_quantity = Numeric::from_str("4").expect("valid numeric quantity");
    let expected_instruction: InstructionBox =
        Burn::asset_numeric(expected_quantity, expected_asset_id).into();
    assert_fixture_matches(
        "burn_asset_numeric.json",
        "burn-asset-numeric-v1",
        &expected_instruction,
    );
}

#[test]
fn burn_asset_fractional_fixture_matches_rust_encoding() {
    let expected_asset_id = fixture_asset_id();
    let expected_quantity = Numeric::from_str("3.1415").expect("valid numeric quantity");
    let expected_instruction: InstructionBox =
        Burn::asset_numeric(expected_quantity, expected_asset_id).into();
    assert_fixture_matches(
        "burn_asset_fractional.json",
        "burn-asset-fractional-v1",
        &expected_instruction,
    );
}

#[test]
fn mint_asset_fixture_matches_rust_encoding() {
    let expected_asset_id = fixture_asset_id();
    let expected_quantity = Numeric::from_str("4").expect("valid numeric quantity");
    let expected_instruction: InstructionBox =
        Mint::asset_numeric(expected_quantity, expected_asset_id).into();
    assert_fixture_matches(
        "mint_asset_numeric.json",
        "mint-asset-numeric-v1",
        &expected_instruction,
    );
}

#[test]
fn burn_trigger_repetitions_fixture_matches_rust_encoding() {
    let trigger_id = TriggerId::from_str("reconciliation_guard").expect("valid trigger id");
    let expected_instruction: InstructionBox = Burn::trigger_repetitions(7, trigger_id).into();
    assert_fixture_matches(
        "burn_trigger_repetitions.json",
        "burn-trigger-repetitions-v1",
        &expected_instruction,
    );
}
