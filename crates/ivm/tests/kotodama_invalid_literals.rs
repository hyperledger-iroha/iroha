//! Tests for invalid Kotodama pointer literals.

use ivm::kotodama::{compiler::Compiler, i18n::Language};

fn english_compiler() -> Compiler {
    Compiler::new_with_language(Language::English)
}

#[test]
fn invalid_account_id_literal_reports_error() {
    let src = r#"
        seiyaku InvalidAccount {
          kotoage fn main() authorize("RegisterAccount") {
            ledger::account::register(AccountId::parse("invalid-account"));
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("invalid AccountId literal"),
        "expected invalid AccountId error, got: {err}"
    );
}

#[test]
fn invalid_asset_definition_literal_reports_error() {
    let src = r#"
        seiyaku InvalidAssetDefinition {
          kotoage fn main() authorize("UnregisterAsset") {
            ledger::asset::unregister(AssetDefinitionId::parse("invalid"));
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("invalid AssetDefinitionId literal"),
        "expected invalid AssetDefinitionId error, got: {err}"
    );
}

#[test]
fn invalid_json_literal_reports_error() {
    let src = r#"
        seiyaku InvalidJson {
          kotoage fn main() authorize("SetAccountDetail") {
            ledger::account::set_detail(account: AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), key: Name::parse("cursor"), value: Json::parse("{\"unterminated\":}"));
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("invalid JSON literal"),
        "expected invalid JSON error, got: {err}"
    );
}

#[test]
fn build_submit_ballot_inline_rejects_runtime_bytes() {
    let src = r#"
        seiyaku RuntimeBallotBytes {
          kotoage fn main(cipher: bytes, nullifier: bytes, proof: bytes, vk: bytes)
            authorize("SubmitBallot") {
            ledger::governance::build_submit_ballot(election_id: "election", ciphertext: cipher, nullifier: nullifier, backend: "ipa", proof: proof, verification_key: vk);
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("build_submit_ballot_inline requires literal ciphertext"),
        "expected literal ciphertext error, got: {err}"
    );
}

#[test]
fn build_unshield_inline_rejects_non_literal_amount() {
    let src = r#"
        seiyaku NonLiteralAmount {
          kotoage fn main(amt: i64) authorize("Unshield") {
            let inputs = b"0123456789abcdef0123456789abcdef";
            crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), amount: amt, inputs: inputs, backend: "ipa", proof: b"\x0a\x0b\x0c", verification_key: b"\x0d\x0e\x0f");
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("error[K3099]")
            && err.contains("build_unshield_inline requires literal amount"),
        "expected literal amount error, got: {err}"
    );
}

#[test]
fn build_submit_ballot_inline_rejects_wrong_nullifier_length() {
    let src = r#"
        seiyaku ShortNullifier {
          kotoage fn main() authorize("SubmitBallot") {
            ledger::governance::build_submit_ballot(election_id: "election", ciphertext: b"ciphertext", nullifier: b"short", backend: "ipa", proof: b"proof", verification_key: b"vk");
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("build_submit_ballot_inline nullifier must be 32 bytes"),
        "expected nullifier length error, got: {err}"
    );
}

#[test]
fn build_unshield_inline_rejects_wrong_inputs_length() {
    let src = r#"
        seiyaku ShortUnshieldInputs {
          kotoage fn main() authorize("Unshield") {
            crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), amount: 1, inputs: b"short", backend: "ipa", proof: b"proof", verification_key: b"vk");
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("error[K3099]")
            && err.contains("build_unshield_inline inputs must be a multiple of 32 bytes"),
        "expected inputs length error, got: {err}"
    );
}

#[test]
fn build_unshield_inline_rejects_negative_amount() {
    let src = r#"
        seiyaku NegativeUnshieldAmount {
          kotoage fn main() authorize("Unshield") {
            crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), amount: -1, inputs: b"0123456789abcdef0123456789abcdef", backend: "ipa", proof: b"proof", verification_key: b"vk");
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("build_unshield_inline requires non-negative amount"),
        "expected non-negative amount error, got: {err}"
    );
}
