#![allow(missing_docs)]

use eyre::Result;
use iroha::data_model::{prelude::*, query::parameters::Pagination};
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn client_has_rejected_and_accepted_txs_should_return_tx_history() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let client = network.client();

    // Given
    let account_id = ALICE_ID.clone();
    let asset_definition_id = "xor#wonderland".parse::<AssetDefinitionId>()?;
    let create_asset =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));
    client.submit_blocking(create_asset)?;

    //When
    let quantity = numeric!(200);
    let asset_id = AssetId::new(asset_definition_id, account_id.clone());
    let mint_existed_asset = Mint::asset_numeric(quantity, asset_id);
    let mint_not_existed_asset = Mint::asset_numeric(
        quantity,
        AssetId::new("foo#wonderland".parse()?, account_id.clone()),
    );

    let transactions_count = 100;

    for i in 0..transactions_count {
        let mint_asset = if i % 2 == 0 {
            &mint_existed_asset
        } else {
            &mint_not_existed_asset
        };
        let instructions: Vec<InstructionBox> = vec![mint_asset.clone().into()];
        let transaction = client.build_transaction(instructions, Metadata::default());
        let _ = client.submit_transaction_blocking(&transaction);
    }

    let transactions = client
        .query(FindTransactions::new())
        .filter_with(|tx| tx.value.authority.eq(account_id.clone()))
        .with_pagination(Pagination::new(Some(nonzero!(50_u64)), 1))
        .execute_all()?;
    assert_eq!(transactions.len(), 50);

    let mut prev_creation_time = None;
    transactions.iter().map(AsRef::as_ref).for_each(|tx| {
        assert_eq!(tx.authority(), &account_id);
        //check sorted descending
        if let Some(prev_creation_time) = prev_creation_time {
            assert!(tx.creation_time() <= prev_creation_time);
        }
        prev_creation_time = Some(tx.creation_time());
    });
    Ok(())
}
