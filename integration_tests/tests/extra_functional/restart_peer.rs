//! Tests that a restarted peer restores its state.
use eyre::{Result, eyre};
use iroha::data_model::prelude::*;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use std::time::{Duration, Instant};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};

use integration_tests::sandbox;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn restarted_peer_should_restore_its_state() -> Result<()> {
    let asset_definition_id = "xor#wonderland".parse::<AssetDefinitionId>()?;
    let quantity = numeric!(200);

    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new().with_peers(4),
        stringify!(restarted_peer_should_restore_its_state),
    )
    .await?
    else {
        return Ok(());
    };
    let peers = network.peers();

    // create state on the first peer
    let peer_a = &peers[0];
    let client = peer_a.client();
    let client_for_submit = client.clone();
    let asset_definition_clone = asset_definition_id.clone();
    let mint_quantity = quantity.clone();
    spawn_blocking(move || {
        client_for_submit
            .submit_all::<InstructionBox>([
                Register::asset_definition(AssetDefinition::numeric(
                    asset_definition_clone.clone(),
                ))
                .into(),
                Mint::asset_numeric(
                    mint_quantity,
                    AssetId::new(asset_definition_clone, ALICE_ID.clone()),
                )
                .into(),
            ])
            .unwrap();
    })
    .await?;
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(restarted_peer_should_restore_its_state),
    )?
    .is_none()
    {
        return Ok(());
    }

    // Ensure the mint made it into the chain before shutting down peers.
    let mint_deadline = Instant::now() + network.sync_timeout();
    let minted = loop {
        let assets = sandbox::handle_result(
            spawn_blocking({
                let client = client.clone();
                move || client.query(FindAssets::new()).execute_all()
            })
            .await?
            .map_err(eyre::Report::new),
            stringify!(restarted_peer_should_restore_its_state),
        )?;
        let Some(assets) = assets else { return Ok(()) };

        if assets.iter().any(|asset| {
            *asset.id().account() == ALICE_ID.clone()
                && *asset.id().definition() == asset_definition_id
                && *asset.value() == quantity
        }) {
            break true;
        }

        if Instant::now() >= mint_deadline {
            break false;
        }
        sleep(Duration::from_millis(200)).await;
    };
    assert!(minted, "minted asset not observed before restart");

    // shutdown all
    network.shutdown().await;

    // restart another one, **without a genesis** even
    let peer_b = &peers[1];
    let config: Vec<_> = network.config_layers().collect();
    assert_ne!(peer_a, peer_b);
    let start_result = timeout(network.peer_startup_timeout(), async move {
        peer_b.start_checked(config.iter().cloned(), None).await?;
        peer_b.once_block(2).await;
        Ok::<(), eyre::Report>(())
    })
    .await;

    match start_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer_b: {reason}"
                )));
            }
            return Err(err);
        }
        Err(err) => {
            let err = eyre::Report::new(err);
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer_b: {reason}"
                )));
            }
            return Err(err);
        }
    }

    // ensure it has the state
    let client = peer_b.client();
    let deadline = Instant::now() + network.sync_timeout();
    let restored = loop {
        let assets = match sandbox::handle_result(
            spawn_blocking({
                let client = client.clone();
                move || client.query(FindAssets::new()).execute_all()
            })
            .await?
            .map_err(eyre::Report::new),
            stringify!(restarted_peer_should_restore_its_state),
        )? {
            Some(assets) => assets,
            None => return Ok(()),
        };
        if let Some(asset) = assets.into_iter().find(|asset| {
            *asset.id().account() == ALICE_ID.clone()
                && *asset.id().definition() == asset_definition_id
        }) {
            break Some(asset.value().clone());
        }
        if Instant::now() >= deadline {
            break None;
        }
        sleep(Duration::from_millis(200)).await;
    };
    let Some(restored_value) = restored else {
        return Err(eyre!("restarted peer did not restore asset before timeout"));
    };
    assert_eq!(quantity, restored_value);

    Ok(())
}
