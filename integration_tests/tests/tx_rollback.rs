//! Ensures invalid instructions are rolled back without side effects.
use eyre::Result;
use integration_tests::sandbox;
use iroha::collections::{Collection, ListQuery, field};
use iroha::data_model::{
    prelude::*,
    query::error::{FindError, QueryExecutionFail},
};
use iroha::query::QueryError;
use iroha_model_base::domain::DomainId;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
#[test]
fn client_sends_transaction_with_invalid_instruction_should_not_see_any_changes() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(client_sends_transaction_with_invalid_instruction_should_not_see_any_changes),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    //When
    let account_id = ALICE_ID.clone();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "xor".parse()?,
    );
    let wrong_asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "ksor".parse()?,
    );
    let create_asset = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal")?),
        )
    });
    let mint_asset = Mint::asset_quantity(
        200u32,
        AssetId::new(wrong_asset_definition_id.clone(), account_id.clone()),
    );
    let error = client
        .submit_all::<InstructionBox>(
            [create_asset.into(), mint_asset.into()],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("invalid second instruction must reject the transaction");
    // The second instruction must fail after the valid registration is attempted.
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains(&format!(
                "Failed to find asset definition: `{wrong_asset_definition_id}`"
            ))),
        "unexpected rollback rejection: {error:?}"
    );
    let missing_asset = AssetId::new(wrong_asset_definition_id.clone(), account_id);
    let asset_error = client
        .client()
        .query_single(FindAssetById::new(missing_asset.clone()))
        .unwrap_err();
    assert!(matches!(asset_error,
        QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::Asset(id)))) if *id == missing_asset
    ));
    // Both the invalid mint target and the definition registered by the first
    // instruction must be absent after the atomic transaction rolls back.
    for id in [wrong_asset_definition_id, asset_definition_id] {
        // Definition point-query failures do not expose a typed absence identity.
        // A complete successful exact-ID collection proves rollback without
        // confusing an unrelated HTTP failure with a missing definition.
        let page = client.list_page(
            &Collection::AssetDefinitions,
            &ListQuery::new()
                .filter(field("id").eq(id.to_string()))
                .limit(2),
        )?;
        assert!(
            page.next_cursor.is_none(),
            "incomplete exact definition lookup"
        );
        assert!(page.items.is_empty(), "definition {id} survived rollback");
    }
    Ok(())
}
