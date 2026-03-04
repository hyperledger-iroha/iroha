#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests that a restarted peer restores its state.
use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::crypto::KeyPair;
use iroha::data_model::prelude::*;
use iroha_config_base::toml::WriteExt as _;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::Table;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn restarted_peer_should_restore_its_state() -> Result<()> {
    let asset_definition_id = "xor#wonderland".parse::<AssetDefinitionId>()?;
    let quantity = numeric!(200);

    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_peers(4)
            .with_config_layer(|layer| {
                layer
                    .write(["snapshot", "mode"], "read_write")
                    .write(["snapshot", "create_every_ms"], 200_i64);
            }),
        stringify!(restarted_peer_should_restore_its_state),
    )
    .await?
    else {
        return Ok(());
    };
    let peers = network.peers();

    // create state on the first peer
    let peer_a = &peers[0];
    let peer_b = &peers[1];
    let client = peer_a.client();
    let client_for_submit = client.clone();
    let asset_definition_clone = asset_definition_id.clone();
    let mint_quantity = quantity.clone();
    let submit_res: eyre::Result<()> = spawn_blocking(move || {
        client_for_submit
            .submit_all_blocking::<InstructionBox>([
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
            .map(|_| ())
    })
    .await
    .map_err(eyre::Report::from)?;
    if sandbox::handle_result(
        submit_res,
        stringify!(restarted_peer_should_restore_its_state),
    )?
    .is_none()
    {
        return Ok(());
    }
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

    // Ensure a post-mint snapshot persists before shutdown so restart can rebuild from disk.
    let snapshot_dir = peer_b.kura_store_dir().join("snapshot");
    let snapshot_deadline = Instant::now() + network.sync_timeout();
    let expected_snapshot_height = 2_u64;
    let snapshot_ready = loop {
        let data = snapshot_dir.join("snapshot.data");
        let digest = snapshot_dir.join("snapshot.sha256");
        let sig = snapshot_dir.join("snapshot.sig");
        let merkle = snapshot_dir.join("snapshot.merkle.json");
        let ready = data.exists() && digest.exists() && sig.exists() && merkle.exists();
        if ready {
            if let Ok(snapshot_bytes) = std::fs::read(&data) {
                if let Ok(value) = norito::json::from_slice::<norito::json::Value>(&snapshot_bytes)
                {
                    let height = value
                        .get("block_hashes")
                        .and_then(norito::json::Value::as_array)
                        .map(|entries| entries.len() as u64)
                        .unwrap_or(0);
                    if height >= expected_snapshot_height {
                        break true;
                    }
                }
            }
        }
        if Instant::now() >= snapshot_deadline {
            break false;
        }
        sleep(Duration::from_millis(200)).await;
    };
    if !snapshot_ready {
        return Err(eyre!("snapshot not created before shutdown"));
    }

    // shutdown all
    network.shutdown().await;

    // restart another one, **without a genesis** even
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

#[tokio::test]
async fn restarted_peer_with_mismatched_genesis_pubkey_uses_stored_genesis() -> Result<()> {
    let test_name = stringify!(restarted_peer_with_mismatched_genesis_pubkey_uses_stored_genesis);
    let Some(network) =
        sandbox::start_network_async_or_skip(NetworkBuilder::new().with_peers(4), test_name)
            .await?
    else {
        return Ok(());
    };
    let peer = &network.peers()[0];

    if sandbox::handle_result(network.ensure_blocks(1).await, test_name)?.is_none() {
        return Ok(());
    }

    let config_layers: Vec<_> = network.config_layers().collect();
    let wrong_genesis_pubkey = KeyPair::random().public_key().to_string();
    let override_layer = Table::new().write(["genesis", "public_key"], wrong_genesis_pubkey);
    let genesis = network.genesis();

    network.shutdown().await;

    let start_result = timeout(network.peer_startup_timeout(), async {
        peer.start_checked(
            config_layers
                .iter()
                .cloned()
                .chain(std::iter::once(Cow::Owned(override_layer))),
            Some(&genesis),
        )
        .await?;
        peer.once_block(1).await;
        Ok::<(), eyre::Report>(())
    })
    .await;

    match start_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer with mismatched genesis pubkey: {reason}"
                )));
            }
            return Err(err);
        }
        Err(err) => {
            let err = eyre::Report::new(err);
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer with mismatched genesis pubkey: {reason}"
                )));
            }
            return Err(err);
        }
    }

    network.shutdown().await;
    Ok(())
}
