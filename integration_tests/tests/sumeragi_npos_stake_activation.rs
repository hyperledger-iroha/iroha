//! `NPoS` election respects staking constraints and delays activation by finality margin.

use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use eyre::{Context as _, ensure};
use integration_tests::sandbox;
use iroha::data_model::{
    Level,
    isi::{Log, Mint, Register, staking::RegisterPublicLaneValidator},
    metadata::Metadata,
    name::Name,
    prelude::*,
};
use iroha_primitives::json::Json;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::json::{self, Value};
use tokio::time::sleep;

const EPOCH_LEN: u64 = 6;
const FINALITY_MARGIN: u64 = 2;
const MIN_SELF_BOND: u64 = 1_000;
const ELIGIBLE_STAKE: u64 = 2_000;
const INELIGIBLE_STAKE: u64 = 100;
const WAIT_HEIGHT: u64 = 16;
const COLLECTOR_RETRY: Duration = Duration::from_secs(60);
const COLLECTOR_POLL: Duration = Duration::from_millis(500);

fn register_validator_instructions(
    asset_def: &AssetDefinitionId,
    validator: &AccountId,
    stake: u64,
    entity: Option<&str>,
) -> Vec<InstructionBox> {
    let mut metadata = Metadata::default();
    if let Some(entity_name) = entity {
        metadata.insert(
            Name::from_str("entity").expect("entity key"),
            Json::new(entity_name),
        );
    }
    vec![
        Register::account(Account::new(validator.clone())).into(),
        Mint::asset_numeric(stake, AssetId::new(asset_def.clone(), validator.clone())).into(),
        RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(stake, 0),
            metadata,
        }
        .into(),
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_election_filters_stake_and_applies_after_margin() -> eyre::Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "use_stake_snapshot_roster"], true)
                .write(
                    ["sumeragi", "epoch_length_blocks"],
                    i64::try_from(EPOCH_LEN).unwrap(),
                )
                .write(["sumeragi", "vrf_commit_deadline_offset"], 2_i64)
                .write(["sumeragi", "vrf_reveal_deadline_offset"], 4_i64)
                .write(["sumeragi", "collectors_k"], 1_i64)
                .write(["sumeragi", "collectors_redundant_send_r"], 1_i64)
                .write(
                    ["sumeragi", "npos", "election", "min_self_bond"],
                    i64::try_from(MIN_SELF_BOND).unwrap(),
                )
                .write(
                    ["sumeragi", "npos", "election", "finality_margin_blocks"],
                    i64::try_from(FINALITY_MARGIN).unwrap(),
                )
                // Use existing genesis asset/accounts for staking to avoid extra setup.
                .write(["nexus", "staking", "stake_asset_id"], "rose#wonderland")
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    "alice@wonderland",
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    "alice@wonderland",
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_election_filters_stake_and_applies_after_margin),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let peers = network.peers();
    let domain: DomainId = "wonderland".parse().expect("domain id");
    let asset_def: AssetDefinitionId = "rose#wonderland".parse().expect("asset def");

    let eligible_peer = &peers[0];
    let ineligible_peer = &peers[1];
    let eligible_account = AccountId::new(domain.clone(), eligible_peer.id().public_key().clone());
    let ineligible_account =
        AccountId::new(domain.clone(), ineligible_peer.id().public_key().clone());

    // Register accounts for peer signatories and fund them with stake asset.
    let mut instructions: Vec<InstructionBox> = Vec::new();
    instructions.extend(register_validator_instructions(
        &asset_def,
        &eligible_account,
        ELIGIBLE_STAKE,
        None,
    ));
    instructions.extend(register_validator_instructions(
        &asset_def,
        &ineligible_account,
        INELIGIBLE_STAKE,
        None,
    ));
    client.submit_all_blocking(instructions)?;

    let pre_margin_height = (FINALITY_MARGIN / 2).max(1);
    let status = client.get_status()?;
    for idx in status.blocks..pre_margin_height {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("stake activation seed {idx}"),
        ))?;
    }
    network.ensure_blocks(pre_margin_height).await?;
    let collectors_url = client
        .torii_url
        .join("v1/sumeragi/collectors")
        .wrap_err("compose collectors URL")?;
    assert_no_single_collector(
        &collectors_url,
        &eligible_peer.id().to_string(),
        &format!("collector roster should not have activated before height {pre_margin_height}"),
    )
    .await?;

    let status = client.get_status()?;
    for idx in status.blocks..WAIT_HEIGHT {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("stake activation tick {idx}"),
        ))?;
    }
    network.ensure_blocks(WAIT_HEIGHT).await?;

    let expected_peer = eligible_peer.id().to_string();
    wait_for_single_collector(&collectors_url, &expected_peer).await?;

    network.shutdown().await;
    Ok(())
}

async fn fetch_collectors(http: &reqwest::Client, url: &reqwest::Url) -> eyre::Result<Value> {
    let response = http
        .get(url.clone())
        .header("accept", "application/json")
        .send()
        .await
        .wrap_err("fetch collectors snapshot")?;
    ensure!(
        response.status().is_success(),
        "collectors endpoint returned status {}",
        response.status()
    );
    let body = response.text().await.wrap_err("collectors body")?;
    json::from_str(&body).wrap_err("parse collectors JSON")
}

async fn wait_for_single_collector(
    collectors_url: &reqwest::Url,
    expected_peer: &str,
) -> eyre::Result<()> {
    let http = reqwest::Client::new();
    let deadline = Instant::now() + COLLECTOR_RETRY;
    loop {
        let peers = collector_peer_ids(&http, collectors_url).await?;
        if peers.len() == 1 && peers[0] == expected_peer {
            return Ok(());
        }
        if Instant::now() > deadline {
            eyre::bail!(
                "collectors never converged to expected validator set; got {:?}",
                peers
            );
        }
        sleep(COLLECTOR_POLL).await;
    }
}

async fn assert_no_single_collector(
    collectors_url: &reqwest::Url,
    expected_peer: &str,
    context: &str,
) -> eyre::Result<()> {
    let http = reqwest::Client::new();
    let peers = collector_peer_ids(&http, collectors_url).await?;
    ensure!(
        peers.len() != 1 || peers[0] != expected_peer,
        "{context}; observed {:?}",
        peers
    );
    Ok(())
}

async fn collector_peer_ids(
    http: &reqwest::Client,
    collectors_url: &reqwest::Url,
) -> eyre::Result<Vec<String>> {
    let snapshot = fetch_collectors(http, collectors_url).await?;
    let collector_entries = snapshot
        .get("collectors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let peers: Vec<String> = collector_entries
        .iter()
        .filter_map(|entry| {
            entry
                .as_object()
                .and_then(|obj| obj.get("peer_id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    Ok(peers)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_entity_correlation_limits_validator_set() -> eyre::Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "use_stake_snapshot_roster"], true)
                .write(
                    ["sumeragi", "epoch_length_blocks"],
                    i64::try_from(EPOCH_LEN).unwrap(),
                )
                .write(["sumeragi", "vrf_commit_deadline_offset"], 2_i64)
                .write(["sumeragi", "vrf_reveal_deadline_offset"], 4_i64)
                .write(["sumeragi", "collectors_k"], 1_i64)
                .write(["sumeragi", "collectors_redundant_send_r"], 1_i64)
                .write(
                    ["sumeragi", "npos", "election", "min_self_bond"],
                    i64::try_from(MIN_SELF_BOND).unwrap(),
                )
                .write(
                    ["sumeragi", "npos", "election", "max_entity_correlation_pct"],
                    50_i64,
                )
                .write(
                    ["sumeragi", "npos", "election", "finality_margin_blocks"],
                    i64::try_from(FINALITY_MARGIN).unwrap(),
                )
                .write(["nexus", "staking", "stake_asset_id"], "rose#wonderland")
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    "alice@wonderland",
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    "alice@wonderland",
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_entity_correlation_limits_validator_set),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let peers = network.peers();
    let domain: DomainId = "wonderland".parse().expect("domain id");
    let asset_def: AssetDefinitionId = "rose#wonderland".parse().expect("asset def");

    let peer_a = &peers[0];
    let peer_b = &peers[1];
    let account_a = AccountId::new(domain.clone(), peer_a.id().public_key().clone());
    let account_b = AccountId::new(domain.clone(), peer_b.id().public_key().clone());

    let mut instructions = Vec::new();
    instructions.extend(register_validator_instructions(
        &asset_def,
        &account_a,
        ELIGIBLE_STAKE,
        Some("acme"),
    ));
    instructions.extend(register_validator_instructions(
        &asset_def,
        &account_b,
        ELIGIBLE_STAKE,
        Some("acme"),
    ));
    client.submit_all_blocking(instructions)?;

    let status = client.get_status()?;
    for idx in status.blocks..WAIT_HEIGHT {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("stake activation entity tick {idx}"),
        ))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= WAIT_HEIGHT)
        .await?;

    let collectors_url = client
        .torii_url
        .join("v1/sumeragi/collectors")
        .wrap_err("compose collectors URL")?;
    let http = reqwest::Client::new();
    let deadline = Instant::now() + COLLECTOR_RETRY;
    loop {
        let peers = collector_peer_ids(&http, &collectors_url).await?;
        if peers.len() == 1
            && (peers[0] == peer_a.id().to_string() || peers[0] == peer_b.id().to_string())
        {
            break;
        }
        if Instant::now() > deadline {
            eyre::bail!("collectors not limited by entity cap; observed {peers:?}");
        }
        sleep(COLLECTOR_POLL).await;
    }

    network.shutdown().await;
    Ok(())
}
