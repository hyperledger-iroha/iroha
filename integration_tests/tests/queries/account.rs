//! Account query coverage for Torii.
use eyre::Result;
use integration_tests::sandbox;
use iroha::collections::{Collection, ListQuery};
use iroha::data_model::prelude::*;
use iroha_model_base::domain::DomainId;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use std::collections::HashSet;
#[derive(norito::JsonDeserialize)]
struct AssetHolder {
    id: AssetId,
    account_id: AccountId,
    asset: AssetDefinitionId,
    quantity: Quantity,
}
#[test]
fn find_accounts_with_asset() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        super::query_network_builder(),
        stringify!(find_accounts_with_asset),
    )
    .unwrap() else {
        return Ok(());
    };
    let test_client = super::query_client(&network);
    let result: Result<()> = (|| {
        // Registering new asset definition
        let definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("Valid"),
            "test_coin".parse().expect("Valid"),
        );
        let asset_definition = {
            let __asset_definition_id = definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "test_coin".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        };
        test_client.submit(
            Register::asset_definition(asset_definition.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        // Checking results before all
        let received_asset_definition = test_client
            .client()
            .query_single(FindAssetDefinitionById::new(definition_id.clone()))
            .expect("asset definition should exist");
        assert_eq!(received_asset_definition.id(), asset_definition.id());
        let accounts: [AccountId; 5] = [
            ALICE_ID.clone(),
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
        ];
        // Registering accounts
        let register_accounts = accounts
            .iter()
            .skip(1) // Alice has already been registered in genesis
            .map(|account_id| Register::account(Account::new(account_id.clone())))
            .collect::<Vec<_>>();
        test_client.submit_all(
            register_accounts,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let mint_asset = accounts
            .iter()
            .cloned()
            .map(|account_id| AssetId::new(definition_id.clone(), account_id))
            .map(|asset_id| Mint::asset_quantity(1u32, asset_id))
            .collect::<Vec<_>>();
        test_client.submit_all(
            mint_asset,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let accounts = HashSet::from(accounts);
        // Checking results
        let received_asset_definition = test_client
            .client()
            .query_single(FindAssetDefinitionById::new(definition_id.clone()))
            .expect("asset definition should exist");
        assert_eq!(received_asset_definition.id(), asset_definition.id());
        assert_eq!(received_asset_definition.spec(), NumericSpec::default());
        // The native holder collection discovers the full holder set, including
        // unexpected accounts, within one complete bounded page.
        let page = test_client.list_page(
            &Collection::AssetHolders(definition_id.clone()),
            &ListQuery::new().limit(u32::try_from(accounts.len() + 1)?),
        )?;
        assert!(
            page.next_cursor.is_none(),
            "all fixture holders fit one page"
        );
        assert_eq!(page.items.len(), accounts.len());
        let mut found_ids = HashSet::new();
        for value in page.items {
            let holder: AssetHolder = norito::json::from_value(value)?;
            assert_eq!(holder.asset, definition_id);
            assert_eq!(
                holder.id,
                AssetId::new(definition_id.clone(), holder.account_id.clone())
            );
            assert_eq!(holder.quantity, Quantity::one());
            assert!(
                found_ids.insert(holder.account_id),
                "holder repeated in one page"
            );
        }
        assert_eq!(found_ids, accounts);
        Ok(())
    })();
    if sandbox::handle_result(result, stringify!(find_accounts_with_asset))?.is_none() {
        return Ok(());
    }
    Ok(())
}
