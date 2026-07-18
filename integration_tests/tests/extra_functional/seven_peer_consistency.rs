#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Verify that all peers in a seven-peer network maintain consistent asset balances with DA enabled.

use std::time::{Duration, Instant};

use eyre::{Result, WrapErr, eyre};
use integration_tests::{sandbox, sync::get_status_with_retry_or_storage};
use iroha::{
    client::Client,
    data_model::{
        ValidationFail,
        parameter::BlockParameter,
        prelude::*,
        query::{
            account::prelude::FindAccounts,
            asset::prelude::FindAssetById,
            asset::prelude::FindAssetsDefinitions,
            domain::prelude::FindDomains,
            error::{FindError, QueryExecutionFail},
        },
    },
    query::QueryError,
};
use iroha_test_network::*;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;

#[test]
#[allow(clippy::too_many_lines)]
fn seven_peer_cross_peer_consistency_basic() -> Result<()> {
    // Given: a 7-peer network and a simple state change
    let builder = NetworkBuilder::new()
        .with_peers(7)
        .with_block_cadence(std::time::Duration::from_secs(2))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "advanced", "rbc", "chunk_fanout"], 7_i64)
                .write(
                    ["sumeragi", "advanced", "rbc", "payload_chunks_per_tick"],
                    64_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "rbc",
                        "rebroadcast_sessions_per_tick",
                    ],
                    32_i64,
                );
        })
        // Keep blocks small to make block progression deterministic in tests
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
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
    let sync_timeout = network.sync_timeout().saturating_mul(2);
    rt.block_on(async { network.ensure_blocks_with(|height| height.total >= 1).await })
        .wrap_err("seven_peer_consistency network did not start")?;
    wait_for_peer_connectivity(
        &rt,
        peers,
        peers.len().saturating_sub(1) as u64,
        sync_timeout,
    )
    .wrap_err("seven_peer_consistency peers did not connect")?;

    // Create a fresh domain, account, and asset definition
    let domain_name: Name = "seven".parse()?;
    let domain_id = DomainId::try_new(&domain_name, "universal")?;
    let create_domain = Register::domain(Domain::new(domain_id.clone()));
    let (account_id, _kp) = gen_account_in(&domain_name);
    let create_account = Register::account(Account::new(account_id.clone()));
    let asset_definition_id =
        iroha_data_model::asset::AssetDefinitionId::new(domain_id.clone(), "xor".parse()?);
    let create_asset_def = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(__asset_definition_id.clone())
            .with_name(__asset_definition_id.name().to_string())
    });

    let mut submitter_client = submitter.client();
    let tx_timeout = sync_timeout;
    submitter_client.transaction_status_timeout = tx_timeout;
    submitter_client.transaction_ttl = Some(tx_timeout + Duration::from_secs(5));
    ensure_domain_registration_lease_for_network(&network, &domain_id)?;
    let setup_result = submitter_client.submit_all_blocking::<InstructionBox>(
        [
            create_domain.into(),
            create_account.into(),
            create_asset_def.into(),
        ],
        iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    if let Err(err) = setup_result {
        eprintln!(
            "seven_peer_consistency setup submission did not confirm; waiting for state. err={err:?}"
        );
        wait_for_setup_state(
            &submitter_client,
            &domain_id,
            &account_id,
            &asset_definition_id,
            tx_timeout,
        )
        .wrap_err("seven_peer_consistency submit setup failed")?;
    }

    let status_before_mint = get_status_with_retry_or_storage(
        &network,
        &submitter_client,
        "seven_peer_consistency status fetch",
    )
    .wrap_err("seven_peer_consistency status fetch failed")?;
    // Mint on one peer and wait until the network advances a few blocks
    let quantity = numeric!(500);
    if let Err(err) = submitter_client.submit_blocking(
        Mint::asset_quantity(
            Quantity::try_from_numeric(quantity.clone())
                .expect("mint quantity must be non-negative"),
            AssetId::new(asset_definition_id.clone(), account_id.clone()),
        ),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    ) {
        eprintln!("seven_peer_consistency mint did not confirm; continuing. err={err:?}");
    }

    let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
    let submitter_deadline = Instant::now() + sync_timeout;
    loop {
        let err_detail = match submitter_client.query_single(FindAssetById::new(asset_id.clone())) {
            Ok(asset) => {
                if asset.value().as_numeric() == &quantity {
                    None
                } else {
                    Some(format!(
                        "mismatched balance (got {}, expected {})",
                        asset.value(),
                        quantity
                    ))
                }
            }
            Err(QueryError::Validation(ValidationFail::QueryFailed(
                QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
            ))) => Some("asset not found".to_owned()),
            Err(err) => Some(format!("query error: {err:?}")),
        };

        if err_detail.is_none() {
            break;
        }

        if Instant::now() >= submitter_deadline {
            return Err(eyre!(
                "minted asset did not appear on submitter {} before timeout; last_err={last_err:?}",
                submitter.id(),
                last_err = err_detail
            ));
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    let expected_min_height = status_before_mint.blocks.saturating_add(1);
    let required_height = expected_min_height.max(3);
    wait_for_blocks_at_least(&rt, peers, required_height, sync_timeout)
        .wrap_err("seven_peer_consistency peers did not all commit the post-mint height")?;

    // Then: verify each peer reports the same state (cross-peer consistency).
    let deadline = Instant::now() + network.sync_timeout();
    loop {
        let mut pending = Vec::new();
        for peer in peers {
            let client = peer.client();
            match client.query_single(FindAssetById::new(asset_id.clone())) {
                Ok(asset) => {
                    if asset.value().as_numeric() != &quantity {
                        pending.push(format!(
                            "{}: mismatched balance (got {}, expected {})",
                            peer.id(),
                            asset.value(),
                            quantity
                        ));
                    }
                }
                Err(QueryError::Validation(ValidationFail::QueryFailed(
                    QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
                ))) => {
                    pending.push(format!("{}: asset not found", peer.id()));
                }
                Err(err) => {
                    pending.push(format!("{}: query error: {err:?}", peer.id()));
                }
            }
        }

        if pending.is_empty() {
            break;
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "minted asset did not converge across peers before timeout: {}",
                pending.join("; ")
            ));
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    Ok(())
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

fn wait_for_blocks_at_least(
    rt: &tokio::runtime::Runtime,
    peers: &[NetworkPeer],
    height: u64,
    timeout: Duration,
) -> Result<()> {
    rt.block_on(async {
        tokio::time::timeout(
            timeout,
            iroha_test_network::once_blocks_sync(peers.iter(), &|h: BlockHeight| h.total >= height),
        )
        .await
        .map_err(|_| eyre!("timed out waiting for peers to reach height {height}"))?
        .map_err(|err| eyre!("block sync predicate failed: {err}"))?;
        Ok(())
    })
}

fn wait_for_setup_state(
    client: &Client,
    domain_id: &DomainId,
    account_id: &AccountId,
    asset_definition_id: &AssetDefinitionId,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_err = None;
    loop {
        let domains = client.query(FindDomains).execute_all();
        let accounts = client.query(FindAccounts).execute_all();
        let asset_defs = client.query(FindAssetsDefinitions).execute_all();

        match (domains, accounts, asset_defs) {
            (Ok(domains), Ok(accounts), Ok(asset_defs)) => {
                let domain_ok = domains.iter().any(|domain| domain.id() == domain_id);
                let account_ok = accounts.iter().any(|account| account.id() == account_id);
                let asset_ok = asset_defs
                    .iter()
                    .any(|asset_def| asset_def.id() == asset_definition_id);
                if domain_ok && account_ok && asset_ok {
                    return Ok(());
                }
            }
            (domain_err, account_err, asset_err) => {
                last_err = Some(format!(
                    "domain={:?}, account={:?}, asset_definition={:?}",
                    domain_err.err(),
                    account_err.err(),
                    asset_err.err()
                ));
            }
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for setup state; last_err={:?}",
                last_err
            ));
        }

        std::thread::sleep(Duration::from_millis(200));
    }
}
