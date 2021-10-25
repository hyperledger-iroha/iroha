#![allow(clippy::restriction)]

use std::thread;

use eyre::Result;
use iroha_client::client;
use iroha_core::{config::Configuration, prelude::*};
use iroha_data_model::prelude::*;
use test_network::*;


#[test]
fn network_stable_after_add_and_after_remove_peer() -> Result<()> {
    // Given
    let (rt, network, mut iroha_client) = <Network>::start_test_with_runtime(4, 1);
    let pipeline_time = Configuration::pipeline_time();

    thread::sleep(pipeline_time * 2);
    iroha_logger::info!("Started");

    let create_domain = RegisterBox::new(IdentifiableBox::Domain(Domain::new("domain").into()));
    let account_id = AccountId::new("account", "domain");
    let create_account = RegisterBox::new(IdentifiableBox::NewAccount(
        NewAccount::with_signatory(account_id.clone(), KeyPair::generate()?.public_key).into(),
    ));
    let asset_definition_id = AssetDefinitionId::new("xor", "domain");
    let create_asset = RegisterBox::new(IdentifiableBox::AssetDefinition(
        AssetDefinition::new_quantity(asset_definition_id.clone()).into(),
    ));
    iroha_client.submit_all(vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ])?;
    thread::sleep(pipeline_time * 2);
    iroha_logger::info!("Init");

    // When
    let quantity: u32 = 200;
    let mint_asset = MintBox::new(
        Value::U32(quantity),
        IdBox::AssetId(AssetId::new(
            asset_definition_id.clone(),
            account_id.clone(),
        )),
    );
    iroha_client.submit(mint_asset)?;
    thread::sleep(pipeline_time * 5);
    iroha_logger::info!("Mint");

	// Then
	let (peer, mut iroha_client) = rt.block_on(network.add_peer());
    iroha_client.poll_request_with_period(
        client::asset::by_account_id(account_id.clone()),
        Configuration::block_sync_gossip_time(),
        15,
        |result| {
            result.iter().any(|asset| {
                asset.id.definition_id == asset_definition_id
                    && asset.value == AssetValue::Quantity(quantity)
            })
        },
    );

	// Also
	thread::sleep(pipeline_time * 2);
	let unregister_peer = UnregisterBox::new(IdentifiableBox::Peer(Box::new(peer.into())));
	iroha_client.submit(unregister_peer)?;
	iroha_logger::info!("Unregister");

    // Then
    iroha_client.poll_request_with_period(
        client::asset::by_account_id(account_id),
        Configuration::block_sync_gossip_time(),
        15,
        |result| {
            result.iter().any(|asset| {
                asset.id.definition_id == asset_definition_id
                    && asset.value == AssetValue::Quantity(quantity)
            })
        },
    );
    Ok(())
}
