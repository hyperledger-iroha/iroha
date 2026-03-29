#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Verify that all peers in a seven-peer network maintain consistent asset balances with DA enabled.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, eyre};
use futures_util::future::join_all;
use integration_tests::{sandbox, sync::get_status_with_retry_or_storage};
use iroha::{
    client::Client,
    data_model::{
        ValidationFail,
        parameter::{BlockParameter, SumeragiParameter},
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
use iroha_core::sumeragi::{network_topology::commit_quorum_from_len, rbc_status};
use iroha_test_network::*;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;
use norito::json::Value;

#[test]
#[allow(clippy::too_many_lines)]
fn seven_peer_cross_peer_consistency_basic() -> Result<()> {
    // Given: a 7-peer network and a simple state change
    let builder = NetworkBuilder::new()
        .with_peers(7)
        .with_pipeline_time(std::time::Duration::from_secs(2))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
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
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(8_000),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(2_000),
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
    let domain_id = DomainId::new(domain_name.clone());
    let create_domain = Register::domain(Domain::new(domain_id.clone()));
    let (account_id, _kp) = gen_account_in(&domain_name);
    let create_account = Register::account(Account::new_in_domain(
        account_id.clone(),
        domain_id.clone(),
    ));
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
    let setup_result = submitter_client.submit_all_blocking::<InstructionBox>([
        create_domain.into(),
        create_account.into(),
        create_asset_def.into(),
    ]);
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
    if let Err(err) = submitter_client.submit_blocking(Mint::asset_numeric(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    )) {
        eprintln!("seven_peer_consistency mint did not confirm; continuing. err={err:?}");
    }

    let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
    let submitter_deadline = Instant::now() + sync_timeout;
    loop {
        let err_detail = match submitter_client.query_single(FindAssetById::new(asset_id.clone())) {
            Ok(asset) => {
                if asset.value() == &quantity {
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

    let rbc_timeout = sync_timeout;
    let rbc_results = rt.block_on(async {
        let tasks = peers.iter().map(|peer| {
            let client = peer.client();
            let store_dir = peer.kura_store_dir().join("rbc_sessions");
            let peer_id = peer.id().clone();
            async move {
                let result = wait_for_rbc_delivery_inner(
                    client,
                    store_dir,
                    expected_min_height,
                    rbc_timeout,
                )
                .await;
                (peer_id, result)
            }
        });
        join_all(tasks).await
    });

    let mut delivered = Vec::new();
    let mut failures = Vec::new();
    for (peer_id, result) in rbc_results {
        match result {
            Ok(session) => delivered.push((peer_id, session)),
            Err(err) => failures.push((peer_id, format!("{err:?}"))),
        }
    }

    let required_height = expected_min_height.max(3);
    wait_for_blocks_at_least(&rt, peers, required_height, sync_timeout)
        .wrap_err("seven_peer_consistency blocks did not advance")?;

    let mut committed_without_session = Vec::new();
    let mut unresolved_failures = Vec::new();
    for (peer_id, err) in failures {
        let committed = peers
            .iter()
            .find(|peer| peer.id() == peer_id)
            .and_then(NetworkPeer::best_effort_block_height)
            .is_some_and(|height| height.total >= expected_min_height);
        if committed {
            committed_without_session.push(peer_id.to_string());
        } else {
            unresolved_failures.push(format!("{peer_id}: {err}"));
        }
    }

    eyre::ensure!(
        !delivered.is_empty(),
        "seven_peer_consistency no delivered RBC session observed at or above height {expected_min_height}"
    );

    let required = commit_quorum_from_len(peers.len());
    eyre::ensure!(
        delivery_or_commit_satisfied(delivered.len(), committed_without_session.len(), required),
        "seven_peer_consistency RBC/commit evidence below quorum (delivered={}, committed_without_session={}, required={}, failures={})",
        delivered.len(),
        committed_without_session.len(),
        required,
        unresolved_failures.join("; ")
    );

    for (peer_id, session) in delivered {
        eyre::ensure!(
            get_bool(&session, "delivered") == Some(true),
            "peer {peer_id} missing RBC delivery at or above height {expected_min_height}"
        );
        let total_chunks = get_u64(&session, "total_chunks")
            .ok_or_else(|| eyre!("peer {peer_id} missing total_chunks"))?;
        let received_chunks = get_u64(&session, "received_chunks")
            .ok_or_else(|| eyre!("peer {peer_id} missing received_chunks"))?;
        eyre::ensure!(
            total_chunks > 0,
            "peer {peer_id} reported zero total_chunks at or above height {expected_min_height}"
        );
        eyre::ensure!(
            received_chunks > 0,
            "peer {peer_id} recorded zero RBC chunks at or above height {expected_min_height}"
        );
        eyre::ensure!(
            get_bool(&session, "invalid") == Some(false),
            "peer {peer_id} flagged RBC session invalid"
        );
    }

    // Then: verify each peer reports the same state (cross-peer consistency).
    let deadline = Instant::now() + network.sync_timeout();
    loop {
        let mut pending = Vec::new();
        for peer in peers {
            let client = peer.client();
            match client.query_single(FindAssetById::new(asset_id.clone())) {
                Ok(asset) => {
                    if asset.value() != &quantity {
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

async fn wait_for_rbc_delivery_inner(
    client: Client,
    store_dir: PathBuf,
    min_height: u64,
    timeout: Duration,
) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    let mut last_err: Option<eyre::Report> = None;
    loop {
        if let Some(summary) = rbc_status::read_persisted_snapshot(&store_dir)
            .into_iter()
            .find(|summary| {
                summary.height >= min_height
                    && summary.delivered
                    && !summary.invalid
                    && summary.total_chunks > 0
                    && summary.received_chunks > 0
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
                "timed out waiting for RBC delivery at or above height {min_height}; last_err={last_err:?}"
            ));
        }

        match tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_sumeragi_rbc_sessions_json()
        })
        .await
        {
            Ok(Ok(snapshot)) => {
                if let Some(session) = delivered_session_for_height(&snapshot, min_height) {
                    return Ok(session);
                }
            }
            Ok(Err(err)) => {
                last_err = Some(err.wrap_err("fetch RBC sessions"));
            }
            Err(err) => {
                last_err = Some(eyre!("failed to join RBC sessions fetch: {err}"));
            }
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

fn delivered_session_for_height(value: &Value, min_height: u64) -> Option<Value> {
    let items = value.as_object()?.get("items")?.as_array()?;
    for item in items {
        let obj = item.as_object()?;
        let height = obj.get("height")?.as_u64()?;
        let delivered = obj.get("delivered")?.as_bool()?;
        let total_chunks = obj.get("total_chunks")?.as_u64()?;
        let received_chunks = obj.get("received_chunks")?.as_u64()?;
        let invalid = obj.get("invalid")?.as_bool().unwrap_or(false);
        if height >= min_height && delivered && !invalid && total_chunks > 0 && received_chunks > 0
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

fn delivery_or_commit_satisfied(
    delivered: usize,
    committed_without_session: usize,
    required: usize,
) -> bool {
    delivered.saturating_add(committed_without_session) >= required
}

#[test]
fn delivered_session_for_height_respects_min_height() {
    let payload = norito::json!({
        "items": [
            {
                "height": 2,
                "delivered": true,
                "total_chunks": 1,
                "received_chunks": 1,
                "invalid": false
            },
            {
                "height": 4,
                "delivered": true,
                "total_chunks": 2,
                "received_chunks": 2,
                "invalid": false
            }
        ]
    });

    let session = delivered_session_for_height(&payload, 3).expect("expected session");
    assert_eq!(get_u64(&session, "height"), Some(4));
}

#[test]
fn delivered_session_for_height_allows_partial_chunks() {
    let payload = norito::json!({
        "items": [
            {
                "height": 5,
                "delivered": true,
                "total_chunks": 4,
                "received_chunks": 1,
                "invalid": false
            }
        ]
    });

    let session = delivered_session_for_height(&payload, 5).expect("expected session");
    assert_eq!(get_u64(&session, "received_chunks"), Some(1));
}

#[test]
fn delivery_or_commit_satisfied_allows_committed_fallback() {
    assert!(delivery_or_commit_satisfied(4, 1, 5));
}

#[test]
fn delivery_or_commit_satisfied_requires_quorum() {
    assert!(!delivery_or_commit_satisfied(3, 1, 5));
}
