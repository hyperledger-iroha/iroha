//! Ensures the canonical confidential wallet fixtures stay in sync with the
//! expected signed transaction hashes.

use std::{fs, path::PathBuf};

use iroha_data_model::{
    instruction_registry, isi::set_instruction_registry, transaction::SignedTransaction,
};
use norito::{
    core::{DecodeFlagsGuard, NoritoDeserialize, from_bytes_view},
    json::{self, Value},
};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures/confidential/wallet_flows_v1.json")
}

#[test]
fn confidential_wallet_fixtures_are_stable() {
    set_instruction_registry(instruction_registry::default());

    let raw = fs::read_to_string(fixture_path()).expect("wallet fixture must load");
    let root: Value = json::from_str(&raw).expect("wallet fixture JSON must parse");
    assert_eq!(
        root.get("format_version")
            .and_then(Value::as_u64)
            .expect("format_version missing"),
        1,
        "unexpected wallet fixture version"
    );
    let cases = root
        .get("cases")
        .and_then(Value::as_array)
        .expect("cases array missing");
    assert!(
        !cases.is_empty(),
        "wallet fixture must contain at least one case"
    );

    for case in cases {
        let case_id = case
            .get("case_id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let signed_hex = case
            .get("signed_transaction_hex")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{case_id}: signed_transaction_hex missing"));
        let hash_hex = case
            .get("transaction_hash_hex")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{case_id}: transaction_hash_hex missing"));
        let signed_bytes = hex::decode(signed_hex)
            .unwrap_or_else(|err| panic!("{case_id}: failed to decode hex: {err}"));
        let tx = decode_signed_transaction(case_id, &signed_bytes)
            .unwrap_or_else(|err| panic!("{case_id}: failed to decode signed tx: {err}"));
        let actual_hash = hex::encode(tx.hash().as_ref());
        assert_eq!(
            actual_hash, hash_hex,
            "{case_id}: signed transaction hash mismatch"
        );
    }
}

#[test]
#[ignore = "utility for regenerating wallet fixture expected hashes"]
fn print_confidential_wallet_fixture_hashes() {
    set_instruction_registry(instruction_registry::default());

    let raw = fs::read_to_string(fixture_path()).expect("wallet fixture must load");
    let root: Value = json::from_str(&raw).expect("wallet fixture JSON must parse");
    let cases = root
        .get("cases")
        .and_then(Value::as_array)
        .expect("cases array missing");

    for case in cases {
        let case_id = case
            .get("case_id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let signed_hex = case
            .get("signed_transaction_hex")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{case_id}: signed_transaction_hex missing"));
        let signed_bytes = hex::decode(signed_hex)
            .unwrap_or_else(|err| panic!("{case_id}: failed to decode hex: {err}"));
        let tx = decode_signed_transaction(case_id, &signed_bytes)
            .unwrap_or_else(|err| panic!("{case_id}: failed to decode signed tx: {err}"));
        let actual_hash = hex::encode(tx.hash().as_ref());
        eprintln!("{case_id}: {actual_hash}");
    }
}

fn decode_signed_transaction(case_id: &str, bytes: &[u8]) -> Result<SignedTransaction, String> {
    // Primary path: framed decode with strict deserialization.
    let strict = std::panic::catch_unwind(|| {
        norito::from_bytes::<SignedTransaction>(bytes).and_then(SignedTransaction::try_deserialize)
    });
    match strict {
        Ok(Ok(tx)) => return Ok(tx),
        Ok(Err(err)) => eprintln!("{case_id}: strict decode failed: {err}"),
        Err(_) => eprintln!("{case_id}: strict decode panicked"),
    }

    // Lenient: use the framed view but decode via the bare codec with guessed flags.
    if let Ok(view) = from_bytes_view(bytes) {
        let flags = view.flags();
        let hint = view.flags_hint();
        let payload = view.as_bytes();
        // 1) Use header flags
        if let Ok(tx) = try_decode_adaptive_with_flags(payload, flags | hint) {
            return Ok(tx);
        }
        // 2) No flags
        if let Ok(tx) = try_decode_adaptive_with_flags(payload, 0) {
            return Ok(tx);
        }
        // 3) Packed-struct guess
        let packed = norito::core::header_flags::PACKED_STRUCT;
        if let Ok(tx) = try_decode_adaptive_with_flags(payload, packed) {
            return Ok(tx);
        }
    }

    // Last resort: headerless adaptive decode.
    try_decode_adaptive_with_flags(bytes, 0)
        .map_err(|err| format!("{case_id}: adaptive headerless decode failed: {err}"))
}

fn try_decode_adaptive_with_flags(payload: &[u8], flags: u8) -> Result<SignedTransaction, String> {
    let attempt = std::panic::catch_unwind(|| {
        let _guard = DecodeFlagsGuard::enter_with_hint(flags, flags);
        norito::codec::decode_adaptive::<SignedTransaction>(payload)
    });
    match attempt {
        Ok(Ok(tx)) => Ok(tx),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err("panic".to_owned()),
    }
}
