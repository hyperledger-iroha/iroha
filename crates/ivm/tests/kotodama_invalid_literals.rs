//! Tests for invalid Kotodama pointer literals.

use ivm::kotodama::{compiler::Compiler, i18n::Language};

fn english_compiler() -> Compiler {
    Compiler::new_with_language(Language::English)
}

#[test]
fn invalid_account_id_literal_reports_error() {
    let src = r#"
        fn main() {
            register_account(account_id("invalid-account"));
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
        fn main() {
            unregister_asset(asset_definition("invalid"));
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
        fn main() {
            set_account_detail(
                account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"),
                name("cursor"),
                json("{\"unterminated\":}")
            );
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
        fn main() {
            let cipher = encode_int(1);
            let nullifier = encode_int(2);
            let proof = encode_int(3);
            let vk = encode_int(4);
            build_submit_ballot_inline("election", cipher, nullifier, "ipa", proof, vk);
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
        fn main() {
            let amt = 1 + 1;
            let inputs = blob("0x01020304");
            build_unshield_inline(
                asset_definition("rose#wonderland"),
                account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"),
                amt,
                inputs,
                "ipa",
                blob("0x0a0b0c"),
                blob("0x0d0e0f")
            );
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("build_unshield_inline requires literal amount"),
        "expected literal amount error, got: {err}"
    );
}

#[test]
fn build_submit_ballot_inline_rejects_wrong_nullifier_length() {
    let src = r#"
        fn main() {
            build_submit_ballot_inline(
                "election",
                blob("ciphertext"),
                blob("short"),
                "ipa",
                blob("proof"),
                blob("vk")
            );
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
        fn main() {
            build_unshield_inline(
                asset_definition("rose#wonderland"),
                account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"),
                1,
                blob("short"),
                "ipa",
                blob("proof"),
                blob("vk")
            );
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("build_unshield_inline inputs must be 32 bytes"),
        "expected inputs length error, got: {err}"
    );
}

#[test]
fn build_unshield_inline_rejects_negative_amount() {
    let src = r#"
        fn main() {
            build_unshield_inline(
                asset_definition("rose#wonderland"),
                account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"),
                -1,
                blob("0123456789abcdef0123456789abcdef"),
                "ipa",
                blob("proof"),
                blob("vk")
            );
        }
    "#;

    let err = english_compiler().compile_source(src).unwrap_err();
    assert!(
        err.contains("build_unshield_inline requires non-negative amount"),
        "expected non-negative amount error, got: {err}"
    );
}
