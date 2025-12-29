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
