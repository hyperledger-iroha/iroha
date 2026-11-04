//! Ensures invalid instructions are rolled back without side effects.
use eyre::Result;
use integration_tests::{collections, sandbox};
use iroha::{
    collections::{Collection, ListQuery},
    data_model::prelude::*,
};
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
    let result = client.submit_all::<InstructionBox>(
        [create_asset.into(), mint_asset.into()],
        iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    let error = result.expect_err("invalid mint transaction must be rejected");
    assert!(
        error
            .downcast_ref::<iroha_data_model::transaction::error::TransactionRejectionReason>()
            .is_some(),
        "rollback requires an authenticated rejection: {error:?}"
    );
    //Then;
    for row in client.list(
        Collection::AccountAssets(account_id.clone()),
        ListQuery::new(),
    ) {
        let row = row?;
        let owner: AccountId = collections::field(&row, "account_id")?;
        let definition: AssetDefinitionId = collections::field(&row, "asset")?;
        assert_eq!(owner, account_id);
        assert_ne!(definition, wrong_asset_definition_id);
    }
    let definition_query_result = collections::definitions(&client)?;
    assert!(
        definition_query_result
            .iter()
            .all(|asset| *asset.id() != wrong_asset_definition_id)
    );
    assert!(
        definition_query_result
            .iter()
            .all(|asset| *asset.id() != asset_definition_id),
        "the earlier registration must roll back with the invalid mint"
    );
    Ok(())
}
