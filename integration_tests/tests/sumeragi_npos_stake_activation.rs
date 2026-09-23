#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! `NPoS` election respects staking constraints and delays activation by finality margin.
#![allow(clippy::too_many_lines)]
use eyre::{Context as _, ensure};
use integration_tests::sandbox;
use iroha::blocking::Client;
use iroha::crypto::{Algorithm, KeyPair};
use iroha::data_model::{
    Level, NetworkId,
    account::Account,
    asset::AssetDefinition,
    block::consensus_v2::HeightContext,
    bridge::{BridgeFinalityProof, verify_bridge_finality_proof},
    domain::Domain,
    isi::{
        Log, Mint, Register,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    parameter::system::SumeragiNposParameters,
    prelude::*,
};
use iroha_config::parameters::defaults;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use iroha_primitives::json::Json;
use iroha_test_network::{
    NetworkBuilder, genesis_factory_with_post_topology, init_instruction_registry,
};
use iroha_test_samples::{ALICE_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use norito::json::{self, Value};
use std::{
    collections::BTreeSet,
    num::NonZeroU64,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use tokio::time::sleep;
const EPOCH_LEN: NonZeroU64 = NonZeroU64::new(6).expect("fixture epoch length must be non-zero");
const FINALITY_MARGIN: u64 = 2;
const MIN_SELF_BOND: u64 = 1_000;
const ELIGIBLE_STAKE: u64 = 2_000;
const INELIGIBLE_STAKE: u64 = 100;
const NEXUS_FEE_SEED_AMOUNT: u32 = 1_000_000;
const WAIT_HEIGHT: u64 = EPOCH_LEN.get() * 2 + 1;
const COLLECTOR_RETRY: Duration = Duration::from_secs(60);
const COLLECTOR_POLL: Duration = Duration::from_millis(100);
const HEIGHT_ADVANCE_RETRY: Duration = Duration::from_secs(600);
const HEIGHT_ADVANCE_POLL: Duration = Duration::from_millis(200);
const HEIGHT_ADVANCE_RESUBMIT_EVERY_ATTEMPTS: u64 = 4;
const TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
static NEXT_SUBMIT_PEER_INDEX: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, Copy)]
enum StakeActivationProfile {
    MinStakeFilter,
    EntityCorrelationCap,
}
fn min_connected_peers_for_submit(peer_count: usize) -> u64 {
    match peer_count {
        0..=2 => 0,
        _ => u64::try_from(peer_count.saturating_sub(2)).expect("peer count should fit into u64"),
    }
}
fn commit_quorum_size(peer_count: usize) -> usize {
    let tolerated_faults = peer_count.saturating_sub(1) / 3;
    peer_count.saturating_sub(tolerated_faults)
}
fn count_heights_at_or_above(heights: &[u64], target_height: u64) -> usize {
    heights
        .iter()
        .filter(|height| **height >= target_height)
        .count()
}
fn pick_fallback_submit_peer_index(block_totals: &[u64], seed: usize) -> usize {
    if block_totals.is_empty() {
        return 0;
    }
    let best_total = block_totals.iter().copied().max().unwrap_or(0);
    let best_indices = block_totals
        .iter()
        .enumerate()
        .filter_map(|(idx, total)| (*total == best_total).then_some(idx))
        .collect::<Vec<_>>();
    if best_indices.is_empty() {
        return 0;
    }
    let offset = seed % best_indices.len();
    best_indices[offset]
}
fn pick_submit_peer_index(
    leader_index: Option<usize>,
    leader_connected: bool,
    block_totals: &[u64],
    seed: usize,
) -> usize {
    let fallback = pick_fallback_submit_peer_index(block_totals, seed);
    if leader_connected {
        leader_index.unwrap_or(fallback)
    } else {
        fallback
    }
}
fn ordered_submit_peer_indices(
    leader_index: Option<usize>,
    leader_connected: bool,
    block_totals: &[u64],
    seed: usize,
) -> Vec<usize> {
    if block_totals.is_empty() {
        return Vec::new();
    }
    let fallback = pick_fallback_submit_peer_index(block_totals, seed);
    let mut ordered = Vec::with_capacity(block_totals.len());
    if leader_connected
        && let Some(leader_index) = leader_index
        && leader_index < block_totals.len()
    {
        ordered.push(leader_index);
    }
    for offset in 0..block_totals.len() {
        let idx = (fallback + offset) % block_totals.len();
        if !ordered.contains(&idx) {
            ordered.push(idx);
        }
    }
    ordered
}
async fn submit_peer_indices_for_network(
    network: &sandbox::SerializedNetwork,
    probe: &Client,
) -> Vec<usize> {
    let peer_count = network.peers().len();
    let mut status = None;
    for peer in network.peers() {
        status = peer.status().await.ok();
        if status.is_some() {
            break;
        }
    }
    if status.is_none() {
        status = probe.client().status().get().await.ok();
    }
    let sumeragi = network
        .peers()
        .iter()
        .find_map(|peer| peer.client().client().get_sumeragi_status().ok())
        .or_else(|| probe.client().get_sumeragi_status().ok());
    let leader_index = sumeragi
        .as_ref()
        .map(|status| status.leader)
        .and_then(|idx| usize::try_from(idx).ok())
        .filter(|&idx| idx < peer_count);
    let leader_is_connected = status
        .as_ref()
        .is_some_and(|status| status.peers >= min_connected_peers_for_submit(peer_count));
    let fallback_totals = network
        .peers()
        .iter()
        .map(|peer| {
            peer.best_effort_block_height()
                .map(|height| height.total)
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let fallback_seed = NEXT_SUBMIT_PEER_INDEX.fetch_add(1, Ordering::Relaxed);
    ordered_submit_peer_indices(
        leader_index,
        leader_is_connected,
        &fallback_totals,
        fallback_seed,
    )
}
fn validator_account_id_for_index(index: usize) -> AccountId {
    let key_pair = KeyPair::try_from_seed(
        format!("integration_tests::sumeragi_npos_stake_activation::{index}").into_bytes(),
        Algorithm::Ed25519,
    )
    .expect("fixture NPoS validator key");
    AccountId::new(key_pair.public_key().clone())
}
fn stake_asset_definition_id() -> AssetDefinitionId {
    defaults::nexus::staking::stake_asset_id()
        .parse()
        .expect("canonical network XOR asset")
}
fn stake_asset_id_literal() -> String {
    stake_asset_definition_id().to_string()
}
fn nexus_fee_asset_definition_id() -> AssetDefinitionId {
    defaults::nexus::fees::fee_asset_id()
        .parse()
        .expect("default nexus fee asset id")
}
fn nexus_fee_asset_id_literal() -> String {
    nexus_fee_asset_definition_id().to_string()
}
#[test]
fn validator_account_id_for_index_uses_checked_seed_derivation() {
    let expected_key_pair = KeyPair::try_from_seed(
        b"integration_tests::sumeragi_npos_stake_activation::3".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture NPoS validator key");
    assert_eq!(
        validator_account_id_for_index(3),
        AccountId::new(expected_key_pair.public_key().clone())
    );
}
#[test]
fn min_connected_peers_for_submit_keeps_quorum_margin() {
    assert_eq!(min_connected_peers_for_submit(0), 0);
    assert_eq!(min_connected_peers_for_submit(1), 0);
    assert_eq!(min_connected_peers_for_submit(2), 0);
    assert_eq!(min_connected_peers_for_submit(3), 1);
    assert_eq!(min_connected_peers_for_submit(4), 2);
}
#[test]
fn commit_quorum_size_matches_byzantine_fault_tolerance() {
    assert_eq!(commit_quorum_size(0), 0);
    assert_eq!(commit_quorum_size(1), 1);
    assert_eq!(commit_quorum_size(4), 3);
    assert_eq!(commit_quorum_size(7), 5);
}
#[test]
fn count_heights_at_or_above_counts_only_reached_peers() {
    assert_eq!(count_heights_at_or_above(&[2, 8, 9, 3], 8), 2);
    assert_eq!(count_heights_at_or_above(&[], 1), 0);
}
#[test]
fn should_submit_height_progress_tick_retries_on_new_height_or_interval() {
    assert!(should_submit_height_progress_tick(None, 0, 0, 4));
    assert!(!should_submit_height_progress_tick(Some(4), 4, 0, 4));
    assert!(!should_submit_height_progress_tick(Some(4), 4, 1, 4));
    assert!(should_submit_height_progress_tick(Some(3), 4, 1, 4));
    assert!(should_submit_height_progress_tick(Some(4), 4, 4, 4));
    assert!(!should_submit_height_progress_tick(Some(4), 4, 4, 0));
}
#[test]
fn pick_fallback_submit_peer_index_prefers_best_height_round_robin() {
    let totals = [7, 11, 11, 3];
    assert_eq!(pick_fallback_submit_peer_index(&totals, 0), 1);
    assert_eq!(pick_fallback_submit_peer_index(&totals, 1), 2);
    assert_eq!(pick_fallback_submit_peer_index(&totals, 2), 1);
    assert_eq!(pick_fallback_submit_peer_index(&[], 42), 0);
}
#[test]
fn pick_submit_peer_index_prefers_connected_leader() {
    let totals = [4, 9, 9, 1];
    assert_eq!(pick_submit_peer_index(Some(3), true, &totals, 0), 3);
    assert_eq!(pick_submit_peer_index(Some(3), false, &totals, 0), 1);
    assert_eq!(pick_submit_peer_index(None, true, &totals, 1), 2);
}
#[test]
fn ordered_submit_peer_indices_prioritize_leader_then_fallback_cycle() {
    let totals = [4, 9, 9, 1];
    assert_eq!(
        ordered_submit_peer_indices(Some(3), true, &totals, 0),
        vec![3, 1, 2, 0]
    );
    assert_eq!(
        ordered_submit_peer_indices(Some(3), false, &totals, 0),
        vec![1, 2, 3, 0]
    );
    assert_eq!(
        ordered_submit_peer_indices(None, true, &totals, 1),
        vec![2, 3, 0, 1]
    );
}
#[test]
fn staking_and_fees_use_the_same_real_network_xor() {
    let fee_asset_id = nexus_fee_asset_definition_id();
    assert_eq!(fee_asset_id, stake_asset_definition_id());
    assert_eq!(fee_asset_id.to_string(), TAIRA_XOR_ASSET_DEFINITION_ID);
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
    let xor_domain = DomainId::try_new("universal", "universal").expect("XOR domain id");
    let stake_asset_id = stake_asset_definition_id();
    let fee_asset_id = nexus_fee_asset_definition_id();
    assert_eq!(stake_asset_id, fee_asset_id);
    let genesis_account_id = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
    let definition = {
        AssetDefinition::new(
            stake_asset_id.clone(),
            "XOR".to_owned(),
            NumericSpec::fractional(9),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
    }
    .with_metadata(Metadata::default());
    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(xor_domain)).into(),
        Register::asset_definition(definition).into(),
        Mint::asset_quantity(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    if genesis_account_id != ALICE_ID.clone() {
        bootstrap_tx.push(
            Mint::asset_quantity(
                NEXUS_FEE_SEED_AMOUNT,
                AssetId::new(fee_asset_id.clone(), genesis_account_id.clone()),
            )
            .into(),
        );
    }
    for (index, _peer) in topology.iter().enumerate() {
        let validator_id = validator_account_id_for_index(index);
        let (stake, _) = profile_for_index(index, profile);
        if validator_id != genesis_account_id {
            bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        }
        bootstrap_tx.push(
            Mint::asset_quantity(stake, AssetId::new(stake_asset_id.clone(), validator_id)).into(),
        );
        bootstrap_tx.push(
            Mint::asset_quantity(
                NEXUS_FEE_SEED_AMOUNT,
                AssetId::new(fee_asset_id.clone(), validator_account_id_for_index(index)),
            )
            .into(),
        );
    }
    let mut validator_tx = Vec::new();
    for (index, peer) in topology.iter().enumerate() {
        let validator_id = validator_account_id_for_index(index);
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
                initial_stake: iroha_primitives::numeric::Quantity::from(stake),
                metadata,
                monetary_plan:
                    iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
                        AssetId::new(stake_asset_id.clone(), validator_id.clone()),
                        AssetId::new(stake_asset_id.clone(), ALICE_ID.clone()),
                        iroha_primitives::numeric::Quantity::from(stake),
                    ),
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
    let deadline = Instant::now() + HEIGHT_ADVANCE_RETRY;
    let mut tick = 0_u64;
    let mut last_height = 0;
    let mut last_submitted_height = None;
    let quorum = commit_quorum_size(network.peers().len()).max(1);
    let mut last_observed = Vec::new();
    wait_for_submit_connectivity(network, Duration::from_secs(30)).await?;
    while Instant::now() <= deadline {
        let heights = collect_network_heights(network, &mut last_observed).await;
        last_height = last_height.max(heights.iter().copied().max().unwrap_or_default());
        if count_heights_at_or_above(&heights, target_height) >= quorum {
            return Ok(());
        }
        if should_submit_height_progress_tick(
            last_submitted_height,
            last_height,
            tick,
            HEIGHT_ADVANCE_RESUBMIT_EVERY_ATTEMPTS,
        ) {
            submit_progress_log(network, client, format!("{log_prefix} {tick}")).await?;
            last_submitted_height = Some(last_height);
        }
        tick = tick.saturating_add(1);
        let remaining = deadline.saturating_duration_since(Instant::now());
        let probe_timeout = remaining.min(Duration::from_secs(15));
        let target_next_height = last_height.saturating_add(1).min(target_height);
        match wait_for_network_height_quorum(network, target_next_height, quorum, probe_timeout)
            .await
        {
            Ok(heights) => {
                last_height = last_height.max(heights.iter().copied().max().unwrap_or_default());
            }
            Err(_) => sleep(HEIGHT_ADVANCE_POLL).await,
        }
    }
    eyre::bail!(
        "network height did not reach quorum {quorum} at {target_height} for {log_prefix}; \
         last observed height={last_height}; last observed peers={last_observed:?}"
    );
}
async fn wait_for_network_height_quorum(
    network: &sandbox::SerializedNetwork,
    target_height: u64,
    quorum: usize,
    timeout: Duration,
) -> eyre::Result<Vec<u64>> {
    let deadline = Instant::now() + timeout;
    let mut last_observed = Vec::new();
    loop {
        let heights = collect_network_heights(network, &mut last_observed).await;
        if count_heights_at_or_above(&heights, target_height) >= quorum {
            return Ok(heights);
        }
        if Instant::now() >= deadline {
            eyre::bail!(
                "network height did not reach quorum {quorum} at {target_height} within {:?}; \
                 last observed peers={last_observed:?}",
                timeout
            );
        }
        sleep(HEIGHT_ADVANCE_POLL).await;
    }
}
async fn collect_network_heights(
    network: &sandbox::SerializedNetwork,
    last_observed: &mut Vec<String>,
) -> Vec<u64> {
    last_observed.clear();
    let mut heights = Vec::new();
    for peer in network.peers() {
        match peer.status().await {
            Ok(status) => {
                heights.push(status.blocks);
                last_observed.push(format!(
                    "{}:{} queue={} queued={} inflight={}",
                    peer.mnemonic(),
                    status.blocks,
                    status.queue_size,
                    status.queue_queued,
                    status.queue_inflight
                ));
            }
            Err(err) => last_observed.push(format!("{}:{err}", peer.mnemonic())),
        }
    }
    heights
}
async fn client_observing_height(
    network: &sandbox::SerializedNetwork,
    target_height: u64,
    fallback: &Client,
) -> Client {
    for peer in network.peers() {
        let storage_reached = peer
            .best_effort_block_height()
            .is_some_and(|height| height.total >= target_height);
        if storage_reached
            || peer
                .status()
                .await
                .is_ok_and(|status| status.blocks >= target_height)
        {
            return peer.client();
        }
    }
    fallback.clone()
}
async fn wait_for_submit_connectivity(
    network: &sandbox::SerializedNetwork,
    timeout: Duration,
) -> eyre::Result<()> {
    let deadline = Instant::now() + timeout;
    let expected = min_connected_peers_for_submit(network.peers().len());
    let mut last_snapshot = Vec::new();
    loop {
        let mut peer_counts = Vec::new();
        for peer in network.peers() {
            if let Ok(status) = peer.status().await {
                peer_counts.push(status.peers);
            }
        }
        if !peer_counts.is_empty() {
            last_snapshot.clone_from(&peer_counts);
            if peer_counts.iter().all(|count| *count >= expected) {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            eyre::bail!(
                "peer connectivity did not reach {expected} connected peers within {:?}; last_snapshot={last_snapshot:?}",
                timeout
            );
        }
        sleep(Duration::from_millis(250)).await;
    }
}
async fn submit_progress_log(
    network: &sandbox::SerializedNetwork,
    probe: &Client,
    message: String,
) -> eyre::Result<()> {
    let candidate_indices = submit_peer_indices_for_network(network, probe).await;
    let transaction = {
        let account = probe.account_client();
        account
            .prepare_transaction(iroha::client::AccountTransactionDraft::new(
                [Log::new(Level::INFO, message)],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))
            .and_then(|payload| account.sign_transaction(payload))
    }
    .expect("build integration-test transaction");
    let mut accepted = false;
    let mut errors = Vec::new();
    for idx in candidate_indices {
        let Some(peer) = network.peers().get(idx) else {
            continue;
        };
        let peer_name = peer.mnemonic().to_owned();
        let submit_client = peer.client();
        let transaction = transaction.clone();
        match tokio::task::spawn_blocking(move || submit_client.submit_transaction(&transaction))
            .await
        {
            Ok(Ok(_)) => accepted = true,
            Ok(Err(err)) => errors.push(format!("{peer_name}: {err:?}")),
            Err(err) => errors.push(format!("{peer_name}: submit join error: {err}")),
        }
    }
    ensure!(
        accepted,
        "progress log was not accepted by any candidate peer; errors={errors:?}"
    );
    Ok(())
}
fn should_submit_height_progress_tick(
    last_submitted_height: Option<u64>,
    current_height: u64,
    attempt: u64,
    resubmit_every_attempts: u64,
) -> bool {
    last_submitted_height != Some(current_height)
        || (resubmit_every_attempts > 0
            && attempt > 0
            && attempt.is_multiple_of(resubmit_every_attempts))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_incomplete_election_retains_the_exact_four_seat_committee() -> eyre::Result<()> {
    init_instruction_registry();
    let gas_account_str = ALICE_ID.to_string();
    let mut npos = SumeragiNposParameters::default();
    npos.epoch_length_blocks = EPOCH_LEN;
    npos.min_self_bond = MIN_SELF_BOND.into();
    npos.finality_margin_blocks = FINALITY_MARGIN;
    npos.evidence_horizon_blocks = 9;
    npos.slashing_delay_blocks = 3;
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )))
        .with_config_layer(|layer| {
            layer
                .write(
                    ["nexus", "fees", "fee_asset_id"],
                    nexus_fee_asset_id_literal(),
                )
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
        stringify!(npos_incomplete_election_retains_the_exact_four_seat_committee),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    let peers = network.peers();
    let expected = peers
        .iter()
        .map(|peer| peer.id().to_string())
        .collect::<BTreeSet<_>>();
    let pre_margin_height = EPOCH_LEN.get() - 1;
    advance_to_height(
        &network,
        &client,
        pre_margin_height,
        "stake activation seed",
    )
    .await?;
    let pre_client = client_observing_height(&network, pre_margin_height, &client).await;
    let collectors_url = client
        .client()
        .endpoint()
        .join("v1/sumeragi/validator-sets")
        .wrap_err("compose validator-set history URL")?;
    wait_for_exact_committee(&collectors_url, &expected).await?;
    let before = authenticated_committee_at_height(
        &pre_client,
        network.network_id(),
        pre_margin_height,
        &expected,
    )
    .await?;
    advance_to_height(&network, &client, WAIT_HEIGHT, "stake activation tick").await?;
    let activation_client = client_observing_height(&network, WAIT_HEIGHT, &client).await;
    let collectors_url = activation_client
        .client()
        .endpoint()
        .join("v1/sumeragi/validator-sets")
        .wrap_err("compose validator-set history URL")?;
    wait_for_exact_committee(&collectors_url, &expected).await?;
    let after = authenticated_committee_at_height(
        &activation_client,
        network.network_id(),
        WAIT_HEIGHT,
        &expected,
    )
    .await?;
    ensure!(
        before.kagemusha_mint_finality_authority == after.kagemusha_mint_finality_authority,
        "an incomplete elected pool must retain the certified authority generation"
    );
    network.shutdown().await;
    Ok(())
}
async fn fetch_collectors(http: &reqwest::Client, url: &reqwest::Url) -> eyre::Result<Value> {
    let response = http
        .get(url.clone())
        .header("accept", "application/json")
        .send()
        .await
        .wrap_err("fetch validator-set history")?;
    ensure!(
        response.status().is_success(),
        "validator-set history endpoint returned status {}",
        response.status()
    );
    let body = response
        .text()
        .await
        .wrap_err("validator-set history body")?;
    json::from_str(&body).wrap_err("parse validator-set history JSON")
}
async fn wait_for_exact_committee(
    collectors_url: &reqwest::Url,
    expected: &BTreeSet<String>,
) -> eyre::Result<()> {
    let http = integration_tests::http::client();
    let deadline = Instant::now() + COLLECTOR_RETRY;
    loop {
        let peers = collector_peer_ids(&http, collectors_url).await?;
        if peers.len() == expected.len()
            && peers.iter().cloned().collect::<BTreeSet<_>>() == *expected
        {
            return Ok(());
        }
        if Instant::now() > deadline {
            eyre::bail!(
                "committee never converged to the exact retained validator set; expected {expected:?}, got {peers:?}"
            );
        }
        sleep(COLLECTOR_POLL).await;
    }
}
async fn authenticated_committee_at_height(
    client: &Client,
    network_id: NetworkId,
    height: u64,
    expected: &BTreeSet<String>,
) -> eyre::Result<HeightContext> {
    let response = integration_tests::http::client()
        .get(
            client
                .client()
                .endpoint()
                .join(&format!("v1/bridge/finality/{height}"))?,
        )
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let proof: BridgeFinalityProof = norito::json::from_slice(&response)?;
    verify_bridge_finality_proof(&proof, &network_id)
        .wrap_err("retained committee needs authenticated finality")?;
    let context = proof.finality_artifact.height_context.clone();
    let actual = context
        .roster
        .iter()
        .map(|voter| voter.validator.to_string())
        .collect::<BTreeSet<_>>();
    ensure!(
        proof.finality_artifact.height == height
            && proof.block_header.height().get() == height
            && context.roster.len() == expected.len()
            && actual == *expected
            && context.roster.iter().all(|voter| voter.power == 1)
            && context.quorum.total_power == 4
            && context.quorum.min_signers == 3
            && proof.finality_artifact.commit_qc.signers.len() == 3,
        "retention must preserve four exact equal voters and a three-vote finality certificate"
    );
    Ok(context)
}
async fn collector_peer_ids(
    http: &reqwest::Client,
    collectors_url: &reqwest::Url,
) -> eyre::Result<Vec<String>> {
    let snapshot = fetch_collectors(http, collectors_url).await?;
    let collector_entries = snapshot
        .as_array()
        .and_then(|snapshots| snapshots.first())
        .and_then(|snapshot| snapshot.get("validator_set"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let peers: Vec<String> = collector_entries
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Ok(peers)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_entity_correlation_cannot_shrink_the_certified_committee() -> eyre::Result<()> {
    init_instruction_registry();
    let gas_account_str = ALICE_ID.to_string();
    let mut npos = SumeragiNposParameters::default();
    npos.epoch_length_blocks = EPOCH_LEN;
    npos.min_self_bond = MIN_SELF_BOND.into();
    npos.max_entity_correlation_pct = 50;
    npos.finality_margin_blocks = FINALITY_MARGIN;
    npos.evidence_horizon_blocks = 9;
    npos.slashing_delay_blocks = 3;
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )))
        .with_config_layer(|layer| {
            layer
                .write(
                    ["nexus", "fees", "fee_asset_id"],
                    nexus_fee_asset_id_literal(),
                )
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
        stringify!(npos_entity_correlation_cannot_shrink_the_certified_committee),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    let peers = network.peers();
    let expected = peers
        .iter()
        .map(|peer| peer.id().to_string())
        .collect::<BTreeSet<_>>();
    advance_to_height(
        &network,
        &client,
        WAIT_HEIGHT,
        "stake activation entity tick",
    )
    .await?;
    let activation_client = client_observing_height(&network, WAIT_HEIGHT, &client).await;
    let collectors_url = activation_client
        .client()
        .endpoint()
        .join("v1/sumeragi/validator-sets")
        .wrap_err("compose validator-set history URL")?;
    wait_for_exact_committee(&collectors_url, &expected).await?;
    authenticated_committee_at_height(
        &activation_client,
        network.network_id(),
        WAIT_HEIGHT,
        &expected,
    )
    .await?;
    network.shutdown().await;
    Ok(())
}

#[test]
fn genesis_staking_plans_bind_funded_validators_to_configured_custody() {
    iroha_test_network::init_instruction_registry();
    let topology = (0..4)
        .map(|index| {
            let key = iroha_crypto::KeyPair::try_from_seed(
                vec![index + 1; 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .expect("deterministic genesis staking validator");
            iroha_model_base::peer::PeerId::new(key.public_key().clone())
        })
        .collect::<Vec<_>>();
    let transactions =
        stake_genesis_post_topology_transactions(&topology, StakeActivationProfile::MinStakeFilter);
    let registrations = transactions
        .iter()
        .flatten()
        .filter_map(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
        })
        .collect::<Vec<_>>();
    assert_eq!(registrations.len(), 4);
    for registration in registrations {
        assert_eq!(
            registration.monetary_plan.network_scope,
            iroha_data_model::nexus::PublicLaneMonetaryScopeV1::Genesis
        );
        assert_eq!(registration.monetary_plan.valid_until_height, 1);
        assert_eq!(
            registration.monetary_plan.source_asset,
            iroha_data_model::asset::AssetId::new(
                stake_asset_definition_id(),
                registration.validator.clone()
            )
        );
        assert_eq!(
            registration.monetary_plan.destination_asset,
            iroha_data_model::asset::AssetId::new(stake_asset_definition_id(), ALICE_ID.clone())
        );
        assert_eq!(
            registration.monetary_plan.amount,
            registration.initial_stake
        );
        assert_eq!(
            registration.monetary_plan.precondition,
            iroha_data_model::nexus::PublicLaneMonetaryPreconditionV1::Registration(
                iroha_data_model::nexus::PublicLaneMonetaryRegistrationV1 {
                    activation_height: 1
                }
            )
        );
    }
}
