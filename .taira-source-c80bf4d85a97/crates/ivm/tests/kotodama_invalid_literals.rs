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
            ledger::account::set_detail(account: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), key: Name::parse("cursor"), value: Json::parse("{\"unterminated\":}"));
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
          kotoage fn main(bytes cipher, bytes nullifier, bytes proof, bytes vk)
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
fn build_unshield_inline_rejects_non_literal_quantity() {
    let src = r#"
        seiyaku NonLiteralAmount {
          kotoage fn main(quantity amount) authorize("Unshield") {
            let inputs = b"0123456789abcdef0123456789abcdef";
            crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), amount: amount, inputs: inputs, backend: "ipa", proof: b"\x0a\x0b\x0c", verification_key: b"\x0d\x0e\x0f");
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
fn build_unshield_inline_accepts_contextual_and_explicit_constant_quantities() {
    let src = r#"
        seiyaku LiteralAmounts {
          const quantity AMOUNT = 7;
          kotoage fn main() authorize("Unshield") {
            let inputs = b"0123456789abcdef0123456789abcdef";
            let _contextual = crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), amount: 5, inputs: inputs, backend: "ipa", proof: b"proof", verification_key: b"vk");
            let _constant = crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), amount: AMOUNT, inputs: inputs, backend: "ipa", proof: b"proof", verification_key: b"vk");
          }
        }
    "#;

    english_compiler()
        .compile_source(src)
        .expect("contextual and explicitly typed constant quantities must compile");
}

#[test]
fn build_unshield_inline_rejects_runtime_int_and_decimal_amounts() {
    for amount_type in ["int", "decimal"] {
        let src = format!(
            r#"
            seiyaku WrongNominalAmount {{
              kotoage fn main({amount_type} amount) authorize("Unshield") {{
                crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), amount: amount, inputs: b"0123456789abcdef0123456789abcdef", backend: "ipa", proof: b"proof", verification_key: b"vk");
              }}
            }}
            "#
        );

        let err = english_compiler().compile_source(&src).unwrap_err();
        assert!(
            err.contains("AssetDefinitionId, AccountId, quantity amount"),
            "type={amount_type}: expected nominal quantity diagnostic, got: {err}"
        );
    }
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
            crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), amount: 1, inputs: b"short", backend: "ipa", proof: b"proof", verification_key: b"vk");
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
            crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), amount: -1, inputs: b"0123456789abcdef0123456789abcdef", backend: "ipa", proof: b"proof", verification_key: b"vk");
          }
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("E_NEGATIVE_QUANTITY")
            && err.contains("contextual quantity literal cannot be negative"),
        "expected stable negative quantity error, got: {err}"
    );
}

#[test]
fn build_unshield_inline_rejects_fractional_and_overwide_quantities() {
    for (amount, expected) in [
        ("1.5", "requires a whole quantity with scale 0"),
        (
            "340282366920938463463374607431768211456",
            "quantity exceeds the u128 V1 proof-scalar range",
        ),
    ] {
        let src = format!(
            r#"
            seiyaku InvalidProofScalar {{
              kotoage fn main() authorize("Unshield") {{
                crypto::zk::build_unshield(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), amount: {amount}, inputs: b"0123456789abcdef0123456789abcdef", backend: "ipa", proof: b"proof", verification_key: b"vk");
              }}
            }}
            "#
        );

        let err = english_compiler().compile_source(&src).unwrap_err();
        assert!(
            err.contains("E_UNSHIELD_AMOUNT_RANGE") && err.contains(expected),
            "amount={amount}: expected `{expected}`, got: {err}"
        );
    }
}
