//! Startup and operation with subsets of peers offline.
use eyre::{Result, eyre};
use integration_tests::{
    sandbox,
    sync::{get_status_with_retry_async, run_on_dedicated_thread},
};
use iroha::{
    crypto::{Algorithm, KeyPair, bls_normal_pop_prove},
    data_model::{isi::register::RegisterPeerWithPop, prelude::*},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::peer::PeerId;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use std::time::{Duration, Instant};
use tokio::time::sleep;
#[tokio::test]
async fn genesis_block_is_committed_with_some_offline_peers() -> Result<()> {
    // Given
    let alice_id = ALICE_ID.clone();
    let roses = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "rose".parse()?,
    );
    let alice_has_roses = Quantity::from(13_u32);
    // When
    let Some(network) = sandbox::build_network_or_skip(
        NetworkBuilder::new().with_peers(4),
        stringify!(genesis_block_is_committed_with_some_offline_peers),
    ) else {
        return Ok(());
    };
    let genesis = network.genesis();
    for peer in network.peers().iter().take(2) {
        if let Err(err) = peer
            // Every starting node needs the same locally authenticated genesis;
            // the other two peers remain offline throughout this scenario.
            .start(network.config_layers(), Some(&genesis))
            .await
        {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                eprintln!(
                    "sandboxed network restriction detected while starting genesis_block_is_committed_with_some_offline_peers; skipping ({reason})"
                );
                return Ok(());
            }
            return Err(err);
        }
    }
    let sync_started = Instant::now();
    if sandbox::handle_result(
        network.ensure_blocks(1).await.map(|_| ()),
        stringify!(genesis_block_is_committed_with_some_offline_peers),
    )?
    .is_none()
    {
        return Ok(());
    }
    // Then
    let client = network
        .peers()
        .iter()
        .find(|x| x.is_running())
        .expect("there are two running peers")
        .client();
    // The storage observation above can precede Torii startup. Require an
    // authoritative applied-state response within the same sync budget before
    // exercising the signed asset query.
    let status = tokio::time::timeout(
        network
            .sync_timeout()
            .saturating_sub(sync_started.elapsed()),
        get_status_with_retry_async(&client),
    )
    .await
    .map_err(|_| eyre!("genesis peer Torii readiness exceeded the sync budget"))??;
    assert!(
        status.blocks >= 1,
        "genesis must be applied before the asset read"
    );
    let asset_id = AssetId::new(roses, alice_id);
    let asset = client
        .account_client()
        .query_single(FindAssetById::new(asset_id.clone()))
        .await?;
    assert_eq!(asset.id(), &asset_id);
    assert_eq!(&alice_has_roses, asset.value());
    Ok(())
}
#[tokio::test]
async fn register_offline_peer() -> Result<()> {
    const N_PEERS: usize = 4;
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_peers(N_PEERS)
            .with_max_validator_capacity(N_PEERS + 1),
        stringify!(register_offline_peer),
    )
    .await?
    else {
        return Ok(());
    };
    if sandbox::handle_result(
        check_status(&network, N_PEERS as u64 - 1).await,
        stringify!(register_offline_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let public_key = key_pair.public_key().clone();
    let peer_id = PeerId::new(public_key);
    let pop = bls_normal_pop_prove(key_pair.private_key()).expect("BLS PoP");
    let register_peer = RegisterPeerWithPop::new(peer_id, pop);
    // Wait for some time to allow peers to connect
    let client = network.client();
    run_on_dedicated_thread(move || {
        client.submit(
            register_peer,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await??;
    let ensure_result = network.ensure_blocks(2).await.map(|_| ());
    if sandbox::handle_result(ensure_result, stringify!(register_offline_peer))?.is_none() {
        return Ok(());
    }
    // Make sure peers count hasn't changed
    if sandbox::handle_result(
        check_status(&network, N_PEERS as u64 - 1).await,
        stringify!(register_offline_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    Ok(())
}
async fn check_status(network: &Network, expected_peers: u64) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut last_err: Option<eyre::Report> = None;
    loop {
        let mut all_ok = true;
        for peer in network.peers() {
            let status = match peer.status().await {
                Ok(status) => status,
                Err(err) => {
                    last_err = Some(err);
                    all_ok = false;
                    continue;
                }
            };
            if status.peers != expected_peers {
                last_err = Some(eyre!(
                    "unexpected peers for {}: expected {expected_peers}, got {}",
                    peer.id(),
                    status.peers
                ));
                all_ok = false;
            }
        }
        if all_ok {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(last_err.unwrap_or_else(|| {
                eyre!("peers count did not converge to {expected_peers} before timeout")
            }));
        }
        sleep(Duration::from_millis(200)).await;
    }
}
