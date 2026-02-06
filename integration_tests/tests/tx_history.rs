#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests covering Torii transaction history pagination and filters.

use std::time::Duration;

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{prelude::*, query::parameters::Pagination};
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn client_has_rejected_and_accepted_txs_should_return_tx_history() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(client_has_rejected_and_accepted_txs_should_return_tx_history),
    )?
    else {
        return Ok(());
    };
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
    let mint_existed_asset = Mint::asset_numeric(quantity.clone(), asset_id);
    let mint_not_existed_asset = Mint::asset_numeric(
        quantity,
        AssetId::new("foo#wonderland".parse()?, account_id.clone()),
    );

    let transactions_count = 10;

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
        .with_pagination(Pagination::new(Some(nonzero!(5_u64)), 1))
        .execute_all()?
        .into_iter()
        .filter(|tx| tx.entrypoint().authority() == &account_id)
        .collect::<Vec<_>>();
    assert_eq!(transactions.len(), 5);

    transactions
        .iter()
        .fold(Duration::MAX, |prev_timestamp, tx| {
            assert_eq!(tx.entrypoint().authority(), &account_id);
            match tx.entrypoint() {
                TransactionEntrypoint::External(entrypoint) => {
                    let curr_timestamp = entrypoint.creation_time();
                    // FindTransactions returns transactions in descending order.
                    assert!(prev_timestamp > curr_timestamp);
                    curr_timestamp
                }
                TransactionEntrypoint::Time(_) => {
                    panic!("unexpected time-triggered entrypoint");
                }
            }
        });

    Ok(())
}
