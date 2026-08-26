// Principal binding shared by account-authenticated runtime/governance drafts.
#[cfg(feature = "app_api")]
fn require_runtime_governance_account(
    requested: &iroha_data_model::account::AccountId,
    authenticated: &iroha_data_model::account::AccountId,
    context: &'static str,
) -> Result<(), Error> {
    if requested == authenticated {
        return Ok(());
    }
    Err(Error::Query(
        iroha_data_model::ValidationFail::NotPermitted(format!(
            "authenticated account must match the {context} authority"
        )),
    ))
}
#[cfg(feature = "app_api")]
fn require_runtime_governance_canonical_account_literal(
    requested: &str,
    authenticated: &iroha_data_model::account::AccountId,
    context: &'static str,
) -> Result<(), Error> {
    let parsed = iroha_data_model::account::AccountId::parse_encoded(requested).map_err(|_| {
        crate::routing::conversion_error(format!(
            "{context} authority must use canonical I105 account id form"
        ))
    })?;
    if parsed.to_string() != requested {
        return Err(crate::routing::conversion_error(format!(
            "{context} authority must use canonical I105 account id form"
        )));
    }
    require_runtime_governance_account(&parsed, authenticated, context)
}
#[cfg(all(test, feature = "app_api"))]
mod runtime_governance_auth_tests {
    use super::{
        Error, require_runtime_governance_account,
        require_runtime_governance_canonical_account_literal,
    };
    use iroha_data_model::ValidationFail;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    #[test]
    fn exact_runtime_governance_authority_is_required() {
        require_runtime_governance_account(&ALICE_ID, &ALICE_ID, "citizen draft")
            .expect("the authenticated authority must be accepted");
        let error = require_runtime_governance_account(&BOB_ID, &ALICE_ID, "citizen draft")
            .expect_err("another body authority must be rejected");
        assert!(matches!(
            error,
            Error::Query(ValidationFail::NotPermitted(message))
                if message.contains("citizen draft authority")
        ));
    }
    #[test]
    fn canonical_literal_binding_rejects_mismatch_and_noncanonical_input() {
        let alice = ALICE_ID.to_string();
        require_runtime_governance_canonical_account_literal(
            &alice,
            &ALICE_ID,
            "ministry agenda draft",
        )
        .expect("the exact canonical authenticated authority must be accepted");
        let error = require_runtime_governance_canonical_account_literal(
            &BOB_ID.to_string(),
            &ALICE_ID,
            "ministry agenda draft",
        )
        .expect_err("another canonical body authority must be rejected");
        assert!(matches!(
            error,
            Error::Query(ValidationFail::NotPermitted(message))
                if message.contains("ministry agenda draft authority")
        ));
        require_runtime_governance_canonical_account_literal(
            &format!(" {alice}"),
            &ALICE_ID,
            "ministry agenda draft",
        )
        .expect_err("noncanonical whitespace must be rejected");
    }
}
