//! Verify that all peers in a seven-peer network maintain consistent asset balances with DA enabled.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, eyre};
use iroha::{
    client::Client,
    data_model::{
        parameter::{BlockParameter, SumeragiParameter},
        prelude::*,
    },
};
use iroha_core::sumeragi::rbc_status;
use iroha_test_network::*;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;
use norito::json::Value;

use integration_tests::sandbox;

#[test]
fn seven_peer_cross_peer_consistency_basic() -> Result<()> {
    // Given: a 7-peer network and a simple state change
    let builder = NetworkBuilder::new()
        .with_peers(7)
        .with_default_pipeline_time()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da_enabled"], true);
        })
        // Keep blocks small to make block progression deterministic in tests
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )));
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(seven_peer_cross_peer_consistency_basic),
    )?
    else {
        return Ok(());
    };

    let peers = network.peers();
    let submitter = &peers[0];

    // Ensure the network is ready before submitting transactions.
    rt.block_on(async { network.ensure_blocks_with(|height| height.total >= 1).await })
        .wrap_err("seven_peer_consistency network did not start")?;
    wait_for_peer_connectivity(
        &rt,
        peers,
        peers.len().saturating_sub(1) as u64,
        network.sync_timeout(),
    )
    .wrap_err("seven_peer_consistency peers did not connect")?;

    // Create a fresh domain, account, and asset definition
    let domain_name: Name = "seven".parse()?;
    let create_domain = Register::domain(Domain::new(DomainId::new(domain_name.clone())));
    let (account_id, _kp) = gen_account_in(&domain_name);
    let create_account = Register::account(Account::new(account_id.clone()));
    let asset_definition_id: AssetDefinitionId = format!("xor#{domain_name}").parse()?;
    let create_asset_def =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));

    let submitter_client = submitter.client();
    submitter_client
        .submit_all_blocking::<InstructionBox>([
            create_domain.into(),
            create_account.into(),
            create_asset_def.into(),
        ])
        .wrap_err("seven_peer_consistency submit setup failed")?;

    let status_before_mint = submitter_client
        .get_status()
        .wrap_err("seven_peer_consistency status fetch failed")?;
    // Mint on one peer and wait until the network advances a few blocks
    let quantity = numeric!(500);
    submitter_client
        .submit_blocking(Mint::asset_numeric(
            quantity.clone(),
            AssetId::new(asset_definition_id.clone(), account_id.clone()),
        ))
        .wrap_err("seven_peer_consistency mint failed")?;

    let expected_height = status_before_mint.blocks + 1;

    for peer in peers {
        let client = peer.client();
        let store_dir = peer.kura_store_dir().join("rbc_sessions");
        let session = wait_for_rbc_delivery(
            &rt,
            &client,
            store_dir,
            expected_height,
            network.sync_timeout(),
        )
        .wrap_err("seven_peer_consistency RBC delivery timeout")?;
        let peer_id = peer.id();
        eyre::ensure!(
            get_bool(&session, "delivered") == Some(true),
            "peer {peer_id} missing RBC delivery at height {expected_height}"
        );
        let total_chunks = get_u64(&session, "total_chunks")
            .ok_or_else(|| eyre!("peer {peer_id} missing total_chunks"))?;
        let received_chunks = get_u64(&session, "received_chunks")
            .ok_or_else(|| eyre!("peer {peer_id} missing received_chunks"))?;
        eyre::ensure!(
            received_chunks == total_chunks,
            "peer {peer_id} missing RBC chunks at height {expected_height}"
        );
        eyre::ensure!(
            get_bool(&session, "invalid") == Some(false),
            "peer {peer_id} flagged RBC session invalid"
        );
    }

    // Ensure all running peers progressed sufficiently
    rt.block_on(async { network.ensure_blocks(3).await })
        .wrap_err("seven_peer_consistency blocks did not advance")?;

    // Then: verify each peer reports the same state (cross-peer consistency)
    for peer in peers {
        let assets = peer.client().query(FindAssets::new()).execute_all()?;
        let maybe_asset = assets
            .into_iter()
            .filter(|a| a.id().account() == &account_id)
            .find(|a| *a.id().definition() == asset_definition_id);

        let asset =
            maybe_asset.ok_or_else(|| eyre!("minted asset missing on peer {}", peer.id()))?;
        eyre::ensure!(
            *asset.value() == quantity,
            "mismatched balance on peer {} (got {}, expected {})",
            peer.id(),
            asset.value(),
            quantity
        );
    }

    Ok(())
}

fn wait_for_rbc_delivery(
    rt: &tokio::runtime::Runtime,
    client: &Client,
    store_dir: PathBuf,
    target_height: u64,
    timeout: Duration,
) -> Result<Value> {
    let client = client.clone();
    rt.block_on(wait_for_rbc_delivery_inner(
        client,
        store_dir,
        target_height,
        timeout,
    ))
}

async fn wait_for_rbc_delivery_inner(
    client: Client,
    store_dir: PathBuf,
    target_height: u64,
    timeout: Duration,
) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(summary) = rbc_status::read_persisted_snapshot(&store_dir)
            .into_iter()
            .find(|summary| {
                summary.height == target_height
                    && summary.delivered
                    && !summary.invalid
                    && summary.total_chunks > 0
                    && summary.received_chunks >= summary.total_chunks
            })
        {
            let mut obj = norito::json::Map::new();
            obj.insert("height".into(), Value::from(summary.height));
            obj.insert("delivered".into(), Value::from(summary.delivered));
            obj.insert("total_chunks".into(), Value::from(summary.total_chunks));
            obj.insert(
                "received_chunks".into(),
                Value::from(summary.received_chunks),
            );
            obj.insert("invalid".into(), Value::from(summary.invalid));
            return Ok(Value::Object(obj));
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for RBC delivery at height {target_height}"
            ));
        }

        let snapshot = tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_sumeragi_rbc_sessions_json()
        })
        .await
        .map_err(|err| eyre!("failed to fetch RBC sessions: {err}"))??;

        if let Some(session) = delivered_session_for_height(&snapshot, target_height) {
            return Ok(session);
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn wait_for_peer_connectivity(
    rt: &tokio::runtime::Runtime,
    peers: &[NetworkPeer],
    expected_peers: u64,
    timeout: Duration,
) -> Result<()> {
    rt.block_on(async {
        let deadline = Instant::now() + timeout;
        loop {
            let mut pending = Vec::new();
            for peer in peers {
                match peer.status().await {
                    Ok(status) if status.peers >= expected_peers => {}
                    Ok(status) => pending.push(format!("{}: peers={}", peer.id(), status.peers)),
                    Err(err) => {
                        if peer
                            .last_known_peers()
                            .is_some_and(|peers| peers >= expected_peers)
                        {
                            continue;
                        }
                        pending.push(format!("{}: status error: {err}", peer.id()));
                    }
                }
            }

            if pending.is_empty() {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for peer connectivity: {}",
                    pending.join("; ")
                ));
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
}

fn delivered_session_for_height(value: &Value, target_height: u64) -> Option<Value> {
    let items = value.as_object()?.get("items")?.as_array()?;
    for item in items {
        let obj = item.as_object()?;
        let height = obj.get("height")?.as_u64()?;
        let delivered = obj.get("delivered")?.as_bool()?;
        let total_chunks = obj.get("total_chunks")?.as_u64()?;
        let received_chunks = obj.get("received_chunks")?.as_u64()?;
        let invalid = obj.get("invalid")?.as_bool().unwrap_or(false);
        if height == target_height
            && delivered
            && !invalid
            && total_chunks > 0
            && received_chunks >= total_chunks
        {
            return Some(item.clone());
        }
    }
    None
}

fn get_bool(value: &Value, key: &str) -> Option<bool> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_bool)
}

fn get_u64(value: &Value, key: &str) -> Option<u64> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_u64)
}
