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
    asset::AssetDefinition,
    domain::Domain,
    isi::{
        Log, Mint, Register,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    metadata::Metadata,
    name::Name,
    prelude::*,
};
use iroha_primitives::json::Json;
use iroha_test_network::{
    NetworkBuilder, genesis_factory_with_post_topology, init_instruction_registry,
};
use iroha_test_samples::{ALICE_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use norito::json::{self, Value};
use tokio::time::sleep;

const EPOCH_LEN: u64 = 6;
const FINALITY_MARGIN: u64 = 2;
const MIN_SELF_BOND: u64 = 1_000;
const ELIGIBLE_STAKE: u64 = 2_000;
const INELIGIBLE_STAKE: u64 = 100;
const STAKE_DOMAIN_ID: &str = "ivm.universal";
const WAIT_HEIGHT: u64 = 16;
const COLLECTOR_RETRY: Duration = Duration::from_secs(60);
const COLLECTOR_POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy)]
enum StakeActivationProfile {
    MinStakeFilter,
    EntityCorrelationCap,
}

fn stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn stake_asset_id_literal() -> String {
    stake_asset_definition_id().to_string()
}

fn profile_for_index(index: usize, profile: StakeActivationProfile) -> (u64, Option<&'static str>) {
    match profile {
        StakeActivationProfile::MinStakeFilter => {
            if index == 0 {
                (ELIGIBLE_STAKE, None)
            } else {
                (INELIGIBLE_STAKE, None)
            }
        }
        StakeActivationProfile::EntityCorrelationCap => {
            if index < 2 {
                (ELIGIBLE_STAKE, Some("acme"))
            } else {
                (INELIGIBLE_STAKE, None)
            }
        }
    }
}

fn stake_genesis_post_topology_transactions(
    topology: &[PeerId],
    profile: StakeActivationProfile,
) -> Vec<Vec<InstructionBox>> {
    let stake_domain = DomainId::parse_fully_qualified(STAKE_DOMAIN_ID).expect("stake domain id");
    let nexus_domain = DomainId::try_new("nexus", "universal").expect("nexus domain id");
    let stake_asset_id = stake_asset_definition_id();
    let genesis_account_id = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());

    let definition = {
        let __asset_definition_id = stake_asset_id.clone();
        AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::default())
            .with_name(__asset_definition_id.name().to_string())
    }
    .with_metadata(Metadata::default());

    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(stake_domain.clone())).into(),
        Register::domain(Domain::new(nexus_domain.clone())).into(),
        Register::asset_definition(definition).into(),
    ];
    for (index, peer) in topology.iter().enumerate() {
        let validator_id = AccountId::new(peer.public_key().clone());
        let (stake, _) = profile_for_index(index, profile);
        if validator_id != genesis_account_id {
            bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        }
        bootstrap_tx.push(
            Mint::asset_numeric(stake, AssetId::new(stake_asset_id.clone(), validator_id)).into(),
        );
    }

    let mut validator_tx = Vec::new();
    for (index, peer) in topology.iter().enumerate() {
        let validator_id = AccountId::new(peer.public_key().clone());
        let (stake, entity) = profile_for_index(index, profile);
        let mut metadata = Metadata::default();
        if let Some(entity_name) = entity {
            metadata.insert(
                Name::from_str("entity").expect("entity key"),
                Json::new(entity_name),
            );
        }
        validator_tx.push(
            RegisterPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: validator_id.clone(),
                peer_id: PeerId::from(peer.public_key().clone()),
                stake_account: validator_id.clone(),
                initial_stake: Numeric::new(stake, 0),
                metadata,
            }
            .into(),
        );
        validator_tx.push(ActivatePublicLaneValidator::new(LaneId::SINGLE, validator_id).into());
    }

    vec![bootstrap_tx, validator_tx]
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
    let gas_account_str = ALICE_ID.to_string();

    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["nexus", "enabled"], true)
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id_literal(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str.clone(),
                )
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
        })
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology = stake_genesis_post_topology_transactions(
                topology.as_ref(),
                StakeActivationProfile::MinStakeFilter,
            );
            genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
            )
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

    let eligible_peer = &peers[0];

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
    let gas_account_str = ALICE_ID.to_string();

    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["nexus", "enabled"], true)
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id_literal(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str.clone(),
                )
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
        })
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology = stake_genesis_post_topology_transactions(
                topology.as_ref(),
                StakeActivationProfile::EntityCorrelationCap,
            );
            genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
            )
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

    let peer_a = &peers[0];
    let peer_b = &peers[1];

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
