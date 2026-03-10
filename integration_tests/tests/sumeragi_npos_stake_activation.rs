#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! `NPoS` election respects staking constraints and delays activation by finality margin.
#![allow(clippy::too_many_lines)]

use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use eyre::{Context as _, ensure};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha::data_model::{
    Level,
    account::Account,
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
const STAKE_ASSET_ID: &str = "xor#nexus";
const STAKE_DOMAIN_ID: &str = "ivm";
const WAIT_HEIGHT: u64 = 16;
const COLLECTOR_RETRY: Duration = Duration::from_secs(60);
const COLLECTOR_POLL: Duration = Duration::from_millis(100);

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

async fn advance_to_height(
    network: &sandbox::SerializedNetwork,
    client: &Client,
    target_height: u64,
    log_prefix: &str,
) -> eyre::Result<()> {
    loop {
        let status = client.get_status()?;
        if status.blocks >= target_height {
            return Ok(());
        }
        let next = status.blocks.saturating_add(1);
        client.submit(Log::new(Level::INFO, format!("{log_prefix} {next}")))?;
        network
            .ensure_blocks_with(move |height| height.total >= next)
            .await?;
    }
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
                .write(["nexus", "enabled"], true)
                .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true)
                .write(
                    ["sumeragi", "npos", "epoch_length_blocks"],
                    i64::try_from(EPOCH_LEN).unwrap(),
                )
                .write(
                    ["sumeragi", "npos", "vrf", "commit_deadline_offset_blocks"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "npos", "vrf", "reveal_deadline_offset_blocks"],
                    4_i64,
                )
                .write(["sumeragi", "collectors", "k"], 1_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64)
                .write(
                    ["sumeragi", "npos", "election", "min_self_bond"],
                    i64::try_from(MIN_SELF_BOND).unwrap(),
                )
                .write(
                    ["sumeragi", "npos", "election", "finality_margin_blocks"],
                    i64::try_from(FINALITY_MARGIN).unwrap(),
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
    let domain: DomainId = STAKE_DOMAIN_ID.parse().expect("domain id");
    let asset_def: AssetDefinitionId = STAKE_ASSET_ID.parse().expect("asset def");

    let eligible_peer = &peers[0];
    let ineligible_peer = &peers[1];
    let other_peer_a = &peers[2];
    let other_peer_b = &peers[3];
    let eligible_account = AccountId::new(eligible_peer.id().public_key().clone());
    let ineligible_account = AccountId::new(ineligible_peer.id().public_key().clone());
    let other_account_a = AccountId::new(other_peer_a.id().public_key().clone());
    let other_account_b = AccountId::new(other_peer_b.id().public_key().clone());

    // Register per-peer validators in an account domain sorted before `nexus`, so these records
    // drive candidate filtering instead of the default NPoS bootstrap records.
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
    instructions.extend(register_validator_instructions(
        &asset_def,
        &other_account_a,
        INELIGIBLE_STAKE,
        None,
    ));
    instructions.extend(register_validator_instructions(
        &asset_def,
        &other_account_b,
        INELIGIBLE_STAKE,
        None,
    ));
    client.submit_all_blocking(instructions)?;

    let pre_margin_height = (FINALITY_MARGIN / 2).max(1);
    advance_to_height(
        &network,
        &client,
        pre_margin_height,
        "stake activation seed",
    )
    .await?;
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

    advance_to_height(&network, &client, WAIT_HEIGHT, "stake activation tick").await?;

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
                .write(["nexus", "enabled"], true)
                .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true)
                .write(
                    ["sumeragi", "npos", "epoch_length_blocks"],
                    i64::try_from(EPOCH_LEN).unwrap(),
                )
                .write(
                    ["sumeragi", "npos", "vrf", "commit_deadline_offset_blocks"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "npos", "vrf", "reveal_deadline_offset_blocks"],
                    4_i64,
                )
                .write(["sumeragi", "collectors", "k"], 1_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64)
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
    let domain: DomainId = STAKE_DOMAIN_ID.parse().expect("domain id");
    let asset_def: AssetDefinitionId = STAKE_ASSET_ID.parse().expect("asset def");

    let peer_a = &peers[0];
    let peer_b = &peers[1];
    let peer_c = &peers[2];
    let peer_d = &peers[3];
    let account_a = AccountId::new(peer_a.id().public_key().clone());
    let account_b = AccountId::new(peer_b.id().public_key().clone());
    let account_c = AccountId::new(peer_c.id().public_key().clone());
    let account_d = AccountId::new(peer_d.id().public_key().clone());

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
    instructions.extend(register_validator_instructions(
        &asset_def,
        &account_c,
        INELIGIBLE_STAKE,
        None,
    ));
    instructions.extend(register_validator_instructions(
        &asset_def,
        &account_d,
        INELIGIBLE_STAKE,
        None,
    ));
    client.submit_all_blocking(instructions)?;

    advance_to_height(
        &network,
        &client,
        WAIT_HEIGHT,
        "stake activation entity tick",
    )
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
