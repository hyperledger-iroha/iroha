use std::thread;

use eyre::Result;
use iroha_client::client;
use iroha_core::{config::Configuration, prelude::*};
use iroha_data_model::prelude::*;
use test_network::*;

#[test]
fn network_stable_after_add_and_after_remove_peer() -> Result<()> {
    // Given a network
    let (rt, network, mut iroha_client_network, pipeline_time, account_id, asset_definition_id) =
        init()?;
    // When assets are minted
    mint(
        &asset_definition_id,
        &account_id,
        &mut iroha_client_network,
        pipeline_time,
        100,
    )?;
    // Then assets are propagated to the registered peer.
    let (peer, mut iroha_client_peer) = rt.block_on(network.add_peer());
    check_assets(
        &mut iroha_client_network,
        &account_id,
        &asset_definition_id,
        100,
    );
    // Also, when a peer is unregistered
    thread::sleep(pipeline_time * 2);
    iroha_client_network.submit(UnregisterBox::new(IdentifiableBox::Peer(peer.into())))?;
    thread::sleep(pipeline_time * 2);
    // We can mint without error.
    mint(
        &asset_definition_id,
        &account_id,
        &mut iroha_client_network,
        pipeline_time,
        200,
    )?;
    // Assets are increased on the main network.
    check_assets(
        &mut iroha_client_network,
        &account_id,
        &asset_definition_id,
        300,
    );
    // But not on the unregistered peer's  network.
    check_assets(
        &mut iroha_client_peer,
        &account_id,
        &asset_definition_id,
        100,
    );
    Ok(())
}

fn check_assets(
    iroha_client: &mut client::Client,
    account_id: &AccountId,
    asset_definition_id: &AssetDefinitionId,
    quantity: u32,
) {
    iroha_client.poll_request_with_period(
        client::asset::by_account_id(account_id.clone()),
        Configuration::block_sync_gossip_time(),
        15,
        |result| {
            result.iter().any(|asset| {
                asset.id.definition_id == *asset_definition_id
                    && asset.value == AssetValue::Quantity(quantity)
            })
        },
    );
}

fn mint(
    asset_definition_id: &AssetDefinitionId,
    account_id: &AccountId,
    iroha_client: &mut client::Client,
    pipeline_time: std::time::Duration,
    quantity: u32,
) -> Result<u32, color_eyre::Report> {
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
    Ok(quantity)
}

fn init() -> Result<(
    tokio::runtime::Runtime,
    test_network::Network,
    iroha_client::client::Client,
    std::time::Duration,
    AccountId,
    AssetDefinitionId,
)> {
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
    Ok((
        rt,
        network,
        iroha_client,
        pipeline_time,
        account_id,
        asset_definition_id,
    ))
}
