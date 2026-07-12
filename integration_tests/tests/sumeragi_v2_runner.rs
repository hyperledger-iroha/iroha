#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! End-to-end regressions for the authoritative Sumeragi v2 production runner.

use std::time::{Duration, Instant};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::{Algorithm, KeyPair},
    data_model::{
        Identifiable,
        account::{Account, AccountId},
        block::consensus_v2::PROTOCOL_VERSION,
        isi::Register,
        parameter::system::SumeragiNposParameters,
        prelude::FindAccountById,
        query::{
            account::prelude::FindAccounts, block::prelude::FindBlocks, prelude::QueryBuilderExt,
        },
    },
};
use iroha_test_network::{NetworkBuilder, NetworkPeer, init_instruction_registry};
use norito::json::Value;
use tokio::{task, time::sleep};

const VALIDATOR_COUNT: usize = 4;
const STATUS_TIMEOUT: Duration = Duration::from_secs(90);
const ACCOUNT_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(90);
const LEGACY_START_TIMEOUT: Duration = Duration::from_secs(45);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const FAST_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(25);
const TAIRA_PIPELINE_TIME: Duration = Duration::from_secs(3);
const TAIRA_ROUND_TIMEOUT_MS: i64 = 10_000;
const TAIRA_RECOVERY_BOUND: Duration = Duration::from_secs(50);

#[derive(Clone, Debug)]
struct V2StatusSnapshot {
    peer: String,
    protocol_version: u64,
    node_fingerprint: Value,
    build_fingerprint: Value,
    config_fingerprint: Value,
    height_context_id: Value,
    height: u64,
    view: u64,
    leader: u64,
    phase: Value,
    body_state: Value,
    last_timeout_view: Option<u64>,
    last_committed_height: u64,
}

/// A four-voter v2 network must finalize across one validator outage, recover
/// the restarted validator, and keep finalizing with the full roster restored.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authoritative_v2_finalizes_through_validator_restart() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_sync_timeout(Duration::from_secs(180));
    let context = stringify!(authoritative_v2_finalizes_through_validator_restart);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        ensure!(
            network.peers().len() == VALIDATOR_COUNT,
            "test requires exactly {VALIDATOR_COUNT} voting validators, got {}",
            network.peers().len()
        );
        ensure!(
            network.topology_entries().len() == VALIDATOR_COUNT
                && network
                    .peers()
                    .iter()
                    .all(|peer| peer.genesis_pop().is_some()),
            "all four validators must have BLS proof-of-possession entries in fresh genesis"
        );
        ensure!(
            network.peers().iter().all(NetworkPeer::is_running),
            "all four voting validators must be running after fresh genesis"
        );

        let all_peers = network.peers().to_vec();
        let initial_statuses = normal_statuses(&all_peers).await?;
        ensure!(
            initial_statuses.iter().all(|status| status.blocks >= 1),
            "fresh genesis must be committed by every validator: {initial_statuses:?}"
        );
        let initial_committed_floor = initial_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let initial_v2 =
            wait_for_v2_statuses(&all_peers, initial_committed_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&initial_v2, VALIDATOR_COUNT)?;

        let before_restart_account = fixture_account(0xA1)?;
        let during_outage_account = fixture_account(0xA2)?;
        let after_restart_account = fixture_account(0xA3)?;
        assert_accounts_absent(
            &all_peers,
            &[
                before_restart_account.clone(),
                during_outage_account.clone(),
                after_restart_account.clone(),
            ],
        )
        .await?;

        let first_target_non_empty = initial_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), before_restart_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= first_target_non_empty)
            .await
            .wrap_err("all four v2 validators did not finalize the pre-restart transaction")?;
        wait_for_accounts_visible(
            &all_peers,
            &[before_restart_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let pre_restart_statuses = normal_statuses(&all_peers).await?;
        let pre_restart_floor = pre_restart_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        ensure!(
            pre_restart_floor > initial_committed_floor,
            "the pre-restart transaction must advance committed height (initial={initial_committed_floor}, current={pre_restart_floor})"
        );
        let pre_restart_v2 =
            wait_for_v2_statuses(&all_peers, pre_restart_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&pre_restart_v2, VALIDATOR_COUNT)?;

        let config_layers = network
            .config_layers()
            .collect::<Vec<_>>();
        let restart_index = VALIDATOR_COUNT - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let restart_node_fingerprint = pre_restart_v2[restart_index].node_fingerprint.clone();
        restart_peer.shutdown().await;

        let remaining_peers = network
            .peers()
            .iter()
            .filter(|peer| peer.is_running())
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            remaining_peers.len() == VALIDATOR_COUNT - 1,
            "exactly three voting validators must remain after one-peer shutdown, got {}",
            remaining_peers.len()
        );

        let outage_baseline = normal_statuses(&remaining_peers).await?;
        let outage_target_non_empty = outage_baseline
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), during_outage_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= outage_target_non_empty)
            .await
            .wrap_err("the three-voter quorum did not finalize while one validator was offline")?;
        wait_for_accounts_visible(
            &remaining_peers,
            &[before_restart_account.clone(), during_outage_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let outage_statuses = normal_statuses(&remaining_peers).await?;
        let outage_floor = outage_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        ensure!(
            outage_floor > pre_restart_floor,
            "the online quorum must advance height during the outage (before={pre_restart_floor}, during={outage_floor})"
        );
        let outage_v2 =
            wait_for_v2_statuses(&remaining_peers, outage_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&outage_v2, VALIDATOR_COUNT)?;

        restart_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart v2 validator {}", restart_peer.mnemonic()))?;
        ensure!(restart_peer.is_running(), "restarted validator must be running");
        network
            .ensure_blocks_with(|height| {
                height.total >= outage_floor && height.non_empty >= outage_target_non_empty
            })
            .await
            .wrap_err("restarted v2 validator did not catch up to outage finality")?;
        wait_for_accounts_visible(
            &all_peers,
            &[before_restart_account.clone(), during_outage_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let recovered_statuses = normal_statuses(&all_peers).await?;
        let recovered_floor = recovered_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let recovered_v2 =
            wait_for_v2_statuses(&all_peers, recovered_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&recovered_v2, VALIDATOR_COUNT)?;
        ensure!(
            recovered_v2[restart_index].node_fingerprint == restart_node_fingerprint,
            "a restarted validator must retain its v2 node identity"
        );

        let post_restart_target_non_empty = recovered_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), after_restart_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= post_restart_target_non_empty)
            .await
            .wrap_err("the restored four-voter v2 network did not finalize a successor block")?;
        wait_for_accounts_visible(
            &all_peers,
            &[
                before_restart_account,
                during_outage_account,
                after_restart_account,
            ],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let final_statuses = normal_statuses(&all_peers).await?;
        let final_floor = final_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        ensure!(
            final_floor > recovered_floor,
            "finalization must continue after restart (recovered={recovered_floor}, final={final_floor})"
        );
        let final_v2 = wait_for_v2_statuses(&all_peers, final_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&final_v2, VALIDATOR_COUNT)?;
        for (before, after) in initial_v2.iter().zip(&final_v2) {
            ensure!(
                before.node_fingerprint == after.node_fingerprint,
                "validator {} changed v2 node fingerprint across the restart scenario",
                after.peer
            );
            ensure!(
                before.build_fingerprint == after.build_fingerprint,
                "validator {} changed build fingerprint across the restart scenario",
                after.peer
            );
            ensure!(
                before.config_fingerprint == after.config_fingerprint,
                "validator {} changed consensus-config fingerprint across the restart scenario",
                after.peer
            );
        }

        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

/// The production NPoS runner must replace an unavailable view leader through
/// a persisted timeout certificate and finalize within the Taira rollout bound.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn taira_npos_leader_timeout_commits_within_rotation_bound() -> Result<()> {
    init_instruction_registry();

    // `with_pipeline_time` assigns one third to the block cadence, so three
    // seconds reproduces Taira's one-second cadence. Global v2 uses the single
    // explicit ten-second round timeout below; legacy phase timeouts are not
    // involved in this scenario.
    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond())
        .with_pipeline_time(TAIRA_PIPELINE_TIME)
        .with_sync_timeout(Duration::from_secs(180))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "round_timeout_ms"], TAIRA_ROUND_TIMEOUT_MS);
        });
    let context = stringify!(taira_npos_leader_timeout_commits_within_rotation_bound);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        ensure!(
            network.peers().len() == VALIDATOR_COUNT
                && network.topology_entries().len() == VALIDATOR_COUNT,
            "Taira regression requires exactly four voting validators"
        );
        ensure!(
            network.peers().iter().all(NetworkPeer::is_running),
            "all Taira validators must be running after fresh NPoS genesis"
        );

        let all_peers = network.peers().to_vec();
        let seed_account = fixture_account(0xB0)?;
        let outage_account = fixture_account(0xB1)?;
        assert_accounts_absent(
            &all_peers,
            &[seed_account.clone(), outage_account.clone()],
        )
        .await?;

        // Commit a seed transaction so the next height has just opened. The
        // view-zero leader then has the full one-second cadence remaining,
        // which makes the leader outage deterministic rather than a race with
        // an already disseminated proposal.
        let initial = normal_statuses(&all_peers).await?;
        let seed_target_non_empty = initial
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), seed_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= seed_target_non_empty)
            .await
            .wrap_err("Taira NPoS seed transaction did not finalize")?;

        let seeded = normal_statuses(&all_peers).await?;
        let seeded_floor = seeded
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let outage_round = wait_for_common_awaiting_v2_round(
            &all_peers,
            seeded_floor,
            STATUS_TIMEOUT,
        )
        .await?;
        let target_height = outage_round[0].height;
        let initial_view = outage_round[0].view;
        let leader = outage_round[0].leader;
        let leader_peer_index = peer_index_for_validator(&all_peers, leader)?;
        let leader_peer = all_peers[leader_peer_index].clone();
        let leader_node_fingerprint = outage_round[leader_peer_index].node_fingerprint.clone();
        let config_layers = network
            .config_layers()
            .map(std::borrow::Cow::into_owned)
            .collect::<Vec<_>>();

        leader_peer.shutdown().await;
        ensure!(
            !leader_peer.is_running(),
            "the selected view leader must be offline before the proposal"
        );
        let remaining_peers = all_peers
            .iter()
            .filter(|peer| peer.is_running())
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            remaining_peers.len() == VALIDATOR_COUNT - 1,
            "exactly three NPoS voters must remain after the leader outage"
        );

        let outage_baseline = normal_statuses(&remaining_peers).await?;
        let outage_target_non_empty = outage_baseline
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(remaining_peers[0].client(), outage_account.clone()).await?;

        let recovery_started = Instant::now();
        tokio::time::timeout(
            TAIRA_RECOVERY_BOUND,
            network.ensure_blocks_with(|height| {
                height.total >= target_height && height.non_empty >= outage_target_non_empty
            }),
        )
        .await
        .wrap_err(
            "three-voter Taira quorum exceeded one leader rotation plus one successful round",
        )??;
        let recovery_elapsed = recovery_started.elapsed();
        ensure!(
            recovery_elapsed <= TAIRA_RECOVERY_BOUND,
            "Taira recovery exceeded {:?}: elapsed={recovery_elapsed:?}",
            TAIRA_RECOVERY_BOUND
        );

        wait_for_accounts_visible(
            &remaining_peers,
            &[seed_account.clone(), outage_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let mut committed_views = Vec::with_capacity(remaining_peers.len());
        for peer in &remaining_peers {
            committed_views.push(committed_view_at_height(peer, target_height).await?);
        }
        ensure!(
            committed_views.iter().all(|view| *view > initial_view),
            "the outage block must be proposed after a certified view advance: height={target_height}, initial_view={initial_view}, committed_views={committed_views:?}"
        );
        ensure!(
            committed_views
                .iter()
                .all(|view| *view <= initial_view.saturating_add(VALIDATOR_COUNT as u64)),
            "the outage block exceeded one complete leader rotation: initial_view={initial_view}, committed_views={committed_views:?}"
        );
        ensure!(
            committed_views.windows(2).all(|views| views[0] == views[1]),
            "validators disagreed on the committed block view at height {target_height}: {committed_views:?}"
        );

        leader_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart Taira leader {}", leader_peer.mnemonic()))?;
        network
            .ensure_blocks_with(|height| {
                height.total >= target_height && height.non_empty >= outage_target_non_empty
            })
            .await
            .wrap_err("restarted Taira leader did not catch up to later-view finality")?;
        wait_for_accounts_visible(
            &all_peers,
            &[seed_account, outage_account],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let recovered = wait_for_v2_statuses(&all_peers, target_height, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&recovered, VALIDATOR_COUNT)?;
        ensure!(
            recovered[leader_peer_index].node_fingerprint == leader_node_fingerprint,
            "the restarted Taira leader changed its v2 node fingerprint"
        );

        // TODO: add message-type filtering to the real-network harness so the
        // production test can retain distinct highest PrepareQCs across several
        // simultaneous live views. The authenticated embedded-QC adapter path
        // and exact captured divergence are covered by focused core tests.
        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

/// The executable must reject the retired v1 protocol before any consensus
/// process can advertise itself as a live validator.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_protocol_cannot_start_a_validator() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_config_layer(|layer| {
            layer.write(["sumeragi", "protocol_version"], 1_i64);
        });
    let context = stringify!(legacy_protocol_cannot_start_a_validator);
    let network = sandbox::build_network_or_skip(builder, context);
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        let genesis = network.genesis();
        let peer = network
            .peers()
            .first()
            .ok_or_else(|| eyre!("legacy-start fixture has no validator"))?;
        let start_result = tokio::time::timeout(
            LEGACY_START_TIMEOUT,
            peer.start_checked(network.config_layers(), Some(&genesis)),
        )
        .await
        .wrap_err("legacy-protocol validator did not fail closed within the timeout")?;
        let error = start_result.expect_err("protocol v1 must not start a live validator");
        let diagnostic = format!("{error:#}");
        ensure!(
            diagnostic.contains("sumeragi.protocol_version must be 2")
                || diagnostic.contains("unsupported Sumeragi protocol version 1")
                || diagnostic.contains("live consensus protocol 1 is unsupported"),
            "validator exited without the required v1 rejection diagnostic: {diagnostic}"
        );
        ensure!(
            network
                .peers()
                .iter()
                .skip(1)
                .all(|peer| !peer.is_running()),
            "the legacy-protocol negative fixture must not start any other validator"
        );
        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

async fn submit_account(client: Client, account_id: AccountId) -> Result<()> {
    task::spawn_blocking(move || {
        client.submit_blocking(Register::account(Account::new(account_id)))
    })
    .await
    .wrap_err("account-registration task panicked")??;
    Ok(())
}

fn fixture_account(seed_marker: u8) -> Result<AccountId> {
    let key_pair = KeyPair::try_from_seed(vec![seed_marker; 32], Algorithm::Ed25519)
        .wrap_err("derive deterministic v2-runner test account")?;
    Ok(AccountId::new(key_pair.public_key().clone()))
}

async fn normal_statuses(peers: &[NetworkPeer]) -> Result<Vec<iroha::client::Status>> {
    let mut statuses = Vec::with_capacity(peers.len());
    for peer in peers {
        statuses.push(
            peer.status()
                .await
                .wrap_err_with(|| format!("fetch /status from {}", peer.mnemonic()))?,
        );
    }
    Ok(statuses)
}

async fn wait_for_common_awaiting_v2_round(
    peers: &[NetworkPeer],
    min_committed_height: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut snapshots = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match fetch_v2_status(peer).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        let observation = format!(
            "rounds={:?}, errors={errors:?}",
            snapshots
                .iter()
                .map(|snapshot| (
                    snapshot.peer.clone(),
                    snapshot.height,
                    snapshot.view,
                    snapshot.leader,
                    snapshot.phase.clone(),
                    snapshot.last_committed_height,
                ))
                .collect::<Vec<_>>()
        );
        if snapshots.len() == peers.len() {
            validate_v2_status_set(&snapshots, VALIDATOR_COUNT)?;
            let first = &snapshots[0];
            let common_awaiting_round = snapshots.iter().all(|snapshot| {
                snapshot.height == first.height
                    && snapshot.view == first.view
                    && snapshot.leader == first.leader
                    && snapshot.height_context_id == first.height_context_id
                    && snapshot.last_committed_height >= min_committed_height
                    && snapshot.last_committed_height.checked_add(1) == Some(snapshot.height)
                    && status_is_awaiting_proposal(&snapshot.phase)
            });
            if common_awaiting_round {
                return Ok(snapshots);
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "validators did not expose a common awaiting-proposal v2 round within {timeout:?}: {observation}"
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

fn status_is_awaiting_proposal(phase: &Value) -> bool {
    phase
        .as_str()
        .or_else(|| {
            phase
                .as_object()
                .and_then(|object| object.get("phase"))
                .and_then(Value::as_str)
        })
        .is_some_and(|tag| {
            tag.eq_ignore_ascii_case("AwaitingProposal")
                || tag.eq_ignore_ascii_case("awaiting_proposal")
        })
}

fn peer_index_for_validator(peers: &[NetworkPeer], validator: u64) -> Result<usize> {
    let mut roster = peers
        .iter()
        .enumerate()
        .map(|(index, peer)| (peer.id(), index))
        .collect::<Vec<_>>();
    roster.sort_by(|left, right| left.0.cmp(&right.0));
    let validator = usize::try_from(validator).wrap_err("validator index does not fit usize")?;
    roster
        .get(validator)
        .map(|(_, network_index)| *network_index)
        .ok_or_else(|| {
            eyre!(
                "validator index {validator} is outside the {}-peer frozen roster",
                roster.len()
            )
        })
}

async fn committed_view_at_height(peer: &NetworkPeer, height: u64) -> Result<u64> {
    let client = peer.client();
    let peer_name = peer.mnemonic().to_owned();
    task::spawn_blocking(move || {
        let blocks = client
            .query(FindBlocks)
            .execute_all()
            .wrap_err_with(|| format!("query blocks from {peer_name}"))?;
        blocks
            .iter()
            .find(|block| block.header().height().get() == height)
            .map(|block| block.header().view_change_index())
            .ok_or_else(|| eyre!("{peer_name} has no committed block at height {height}"))
    })
    .await
    .wrap_err_with(|| format!("block-view query panicked for {}", peer.mnemonic()))?
}

async fn assert_accounts_absent(peers: &[NetworkPeer], accounts: &[AccountId]) -> Result<()> {
    for peer in peers {
        let client = peer.client();
        let peer_name = peer.mnemonic().to_owned();
        let stored = task::spawn_blocking(move || client.query(FindAccounts).execute_all())
            .await
            .wrap_err_with(|| format!("fresh-genesis account query panicked for {peer_name}"))?
            .wrap_err_with(|| format!("query fresh-genesis accounts from {peer_name}"))?;
        for account in accounts {
            let found = stored.iter().any(|stored| stored.id() == account);
            ensure!(
                !found,
                "fresh genesis unexpectedly contained test account {account} on {peer_name}"
            );
        }
    }
    Ok(())
}

async fn wait_for_accounts_visible(
    peers: &[NetworkPeer],
    accounts: &[AccountId],
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_missing = Vec::new();
    loop {
        last_missing.clear();
        for peer in peers {
            for account in accounts {
                let client = peer.client();
                let account = account.clone();
                let expected = account.clone();
                let expected_label = expected.to_string();
                let peer_name = peer.mnemonic().to_owned();
                let visible = task::spawn_blocking(move || {
                    client
                        .query_single(FindAccountById::new(account))
                        .is_ok_and(|stored| stored.id() == &expected)
                })
                .await
                .wrap_err_with(|| format!("account visibility query panicked for {peer_name}"))?;
                if !visible {
                    last_missing.push(format!("{expected_label} on {peer_name}"));
                }
            }
        }
        if last_missing.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "accounts did not become visible on every required validator within {timeout:?}: {last_missing:?}"
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn wait_for_v2_statuses(
    peers: &[NetworkPeer],
    min_committed_height: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut snapshots = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match fetch_v2_status(peer).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        let committed = snapshots
            .iter()
            .map(|snapshot| (snapshot.peer.clone(), snapshot.last_committed_height))
            .collect::<Vec<_>>();
        let observation = format!("committed={committed:?}, errors={errors:?}");
        if snapshots.len() == peers.len()
            && snapshots
                .iter()
                .all(|snapshot| snapshot.last_committed_height >= min_committed_height)
        {
            return Ok(snapshots);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "authoritative v2 status did not reach committed height {min_committed_height} on all validators within {timeout:?}: {observation}"
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn fetch_v2_status(peer: &NetworkPeer) -> Result<V2StatusSnapshot> {
    let client = peer.client();
    let peer_name = peer.mnemonic().to_owned();
    let value = task::spawn_blocking(move || client.get_sumeragi_status_json())
        .await
        .wrap_err_with(|| format!("v2 status task panicked for {peer_name}"))?
        .wrap_err_with(|| format!("fetch authoritative v2 status from {peer_name}"))?;
    parse_v2_status(peer_name, &value)
}

fn parse_v2_status(peer: String, value: &Value) -> Result<V2StatusSnapshot> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("v2 status for {peer} is not a JSON object"))?;
    let required_u64 = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("v2 status for {peer} lacks integer field `{field}`"))
    };
    let required_value = |field: &str| {
        let value = object
            .get(field)
            .filter(|value| !value.is_null())
            .cloned()
            .ok_or_else(|| eyre!("v2 status for {peer} lacks field `{field}`"))?;
        Ok::<_, eyre::Report>(value)
    };

    Ok(V2StatusSnapshot {
        peer: peer.clone(),
        protocol_version: required_u64("protocol_version")?,
        node_fingerprint: required_value("node_fingerprint")?,
        build_fingerprint: required_value("build_fingerprint")?,
        config_fingerprint: required_value("config_fingerprint")?,
        height_context_id: required_value("height_context_id")?,
        height: required_u64("height")?,
        view: required_u64("view")?,
        leader: required_u64("leader")?,
        phase: required_value("phase")?,
        body_state: required_value("body_state")?,
        last_timeout_view: optional_timeout_view(object, &peer)?,
        last_committed_height: required_u64("last_committed_height")?,
    })
}

fn optional_timeout_view(object: &norito::json::Map, peer: &str) -> Result<Option<u64>> {
    let Some(certificate) = object
        .get("last_timeout_certificate")
        .filter(|value| !value.is_null())
    else {
        return Ok(None);
    };
    let round = certificate
        .as_object()
        .and_then(|certificate| certificate.get("round"))
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("v2 status for {peer} has a malformed timeout-certificate round"))?;
    let view = round
        .get("view")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("v2 status for {peer} has a timeout certificate without a view"))?;
    Ok(Some(view))
}

fn validate_v2_status_set(
    snapshots: &[V2StatusSnapshot],
    frozen_validator_count: usize,
) -> Result<()> {
    ensure!(!snapshots.is_empty(), "v2 status set must not be empty");
    let expected_protocol = u64::from(PROTOCOL_VERSION);
    let first = &snapshots[0];
    for snapshot in snapshots {
        ensure!(
            snapshot.protocol_version == expected_protocol,
            "{} advertised protocol {}, expected authoritative v2 ({expected_protocol})",
            snapshot.peer,
            snapshot.protocol_version
        );
        ensure!(
            snapshot.height >= snapshot.last_committed_height
                && snapshot.height - snapshot.last_committed_height <= 1,
            "{} reported impossible v2 height relation: active={}, committed={}",
            snapshot.peer,
            snapshot.height,
            snapshot.last_committed_height
        );
        ensure!(
            snapshot.leader < frozen_validator_count as u64,
            "{} reported leader {} outside the frozen {frozen_validator_count}-validator roster",
            snapshot.peer,
            snapshot.leader
        );
        if let Some(timeout_view) = snapshot.last_timeout_view {
            ensure!(
                timeout_view.checked_add(1) == Some(snapshot.view),
                "{} reported current view {} after timeout certificate view {}",
                snapshot.peer,
                snapshot.view,
                timeout_view
            );
        }
        ensure!(
            snapshot.build_fingerprint == first.build_fingerprint,
            "{} disagrees on the v2 build fingerprint",
            snapshot.peer
        );
        ensure!(
            snapshot.config_fingerprint == first.config_fingerprint,
            "{} disagrees on the v2 consensus-config fingerprint",
            snapshot.peer
        );
        ensure!(
            !snapshot.phase.is_null() && !snapshot.body_state.is_null(),
            "{} returned an incomplete v2 reducer status",
            snapshot.peer
        );
    }

    for (index, left) in snapshots.iter().enumerate() {
        for right in &snapshots[index + 1..] {
            ensure!(
                left.node_fingerprint != right.node_fingerprint,
                "{} and {} unexpectedly share a v2 node fingerprint",
                left.peer,
                right.peer
            );
            if left.height == right.height {
                ensure!(
                    left.height_context_id == right.height_context_id,
                    "{} and {} disagree on the immutable context for height {}",
                    left.peer,
                    right.peer,
                    left.height
                );
                if left.view == right.view {
                    ensure!(
                        left.leader == right.leader,
                        "{} and {} disagree on the leader for height {} view {}",
                        left.peer,
                        right.peer,
                        left.height,
                        left.view
                    );
                }
            }
        }
    }
    Ok(())
}
