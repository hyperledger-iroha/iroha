//! Bounded-latency localnet smoke test for permissioned Sumeragi with DA enabled.

use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::future::try_join_all;
use integration_tests::sandbox;
use iroha::data_model::{
    Level,
    isi::{InstructionBox, Log, SetParameter},
    parameter::{BlockParameter, Parameter},
};
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use nonzero_ext::nonzero;
use tokio::time::sleep;

static LOCALNET_SMOKE_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
const SMOKE_PIPELINE_TIME: Duration = Duration::from_secs(2);
const STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_produces_blocks_within_bound() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("localnet smoke guard");

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(["sumeragi", "npos", "timeouts", "propose_ms"], 200_i64)
                .write(["sumeragi", "npos", "timeouts", "prevote_ms"], 400_i64)
                .write(["sumeragi", "npos", "timeouts", "precommit_ms"], 600_i64)
                .write(["sumeragi", "npos", "timeouts", "commit_ms"], 800_i64)
                .write(["sumeragi", "npos", "timeouts", "da_ms"], 400_i64)
                .write(["sumeragi", "pacemaker_max_backoff_ms"], 2_000_i64)
                .write(["sumeragi", "pacemaker_rtt_floor_multiplier"], 1_i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let baseline_view_changes: Vec<u64> = baseline_statuses
            .iter()
            .map(|status| status.view_changes.into())
            .collect();
        let peer_count = network.peers().len();
        let fault_tolerance = peer_count.saturating_sub(1) / 3;
        let max_extra_view_changes = u64::try_from(fault_tolerance.saturating_add(2))
            .unwrap_or(u64::MAX);

        let submit_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = submit_peer.client();
        client
            .submit::<InstructionBox>(
                Log::new(Level::INFO, "localnet bounded block".to_string()).into(),
            )
            .wrap_err("failed to submit log instruction")?;

        let target_height = baseline_height.saturating_add(1);
        let start = Instant::now();
        wait_for_converged_height(&network, target_height, Duration::from_secs(15)).await?;
        let elapsed = start.elapsed();
        ensure!(
            elapsed <= Duration::from_secs(15),
            "block production exceeded bound: elapsed={:?}",
            elapsed
        );

        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks >= target_height),
            "not all peers reached target height {target_height}: {after_statuses:?}"
        );
        for (idx, status) in after_statuses.iter().enumerate() {
            let before = baseline_view_changes.get(idx).copied().unwrap_or_default();
            ensure!(
                u64::from(status.view_changes) <= before.saturating_add(max_extra_view_changes),
                "peer {idx} experienced repeated view changes: before={before}, after={}, max_extra={max_extra_view_changes}",
                status.view_changes,
            );
        }
        let min_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .min()
            .unwrap_or_default();
        let max_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .max()
            .unwrap_or_default();
        ensure!(
            max_view_changes.saturating_sub(min_view_changes) <= max_extra_view_changes,
            "view_change counters diverged across peers: {after_statuses:?}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(
        result,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )?
    .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_reaches_100_blocks() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("localnet smoke guard");

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(["sumeragi", "npos", "timeouts", "propose_ms"], 200_i64)
                .write(["sumeragi", "npos", "timeouts", "prevote_ms"], 400_i64)
                .write(["sumeragi", "npos", "timeouts", "precommit_ms"], 600_i64)
                .write(["sumeragi", "npos", "timeouts", "commit_ms"], 800_i64)
                .write(["sumeragi", "npos", "timeouts", "da_ms"], 400_i64)
                .write(["sumeragi", "pacemaker_max_backoff_ms"], 2_000_i64)
                .write(["sumeragi", "pacemaker_rtt_floor_multiplier"], 1_i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_localnet_reaches_100_blocks),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();

        let target_blocks = 100_u64;
        let target_height = baseline_height.saturating_add(target_blocks);
        let submit_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = submit_peer.client();
        for idx in 0..target_blocks {
            client
                .submit::<InstructionBox>(
                    Log::new(Level::INFO, format!("localnet block {idx}")).into(),
                )
                .wrap_err_with(|| format!("failed to submit log instruction {idx}"))?;
        }

        let timeout = scale_duration(network.pipeline_time(), target_blocks)
            .saturating_add(Duration::from_secs(30));
        let start = Instant::now();
        wait_for_converged_height(&network, target_height, timeout).await?;
        let elapsed = start.elapsed();
        ensure!(
            elapsed <= timeout,
            "block production exceeded bound: elapsed={:?}",
            elapsed
        );

        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks >= target_height),
            "not all peers reached target height {target_height}: {after_statuses:?}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(result, stringify!(permissioned_localnet_reaches_100_blocks))?
        .is_none()
    {
        return Ok(());
    }
    Ok(())
}

async fn collect_statuses(
    network: &Network,
    status_timeout: Duration,
) -> Result<Vec<iroha::client::Status>> {
    try_join_all(network.peers().iter().map(|peer| async move {
        match tokio::time::timeout(status_timeout, peer.status()).await {
            Ok(result) => result
                .wrap_err_with(|| format!("status request failed for peer {}", peer.mnemonic())),
            Err(_) => Err(eyre!(
                "status request timed out after {:?} for peer {}",
                status_timeout,
                peer.mnemonic()
            )),
        }
    }))
    .await
}

async fn wait_for_status_responses(network: &Network, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match collect_statuses(network, STATUS_POLL_TIMEOUT).await {
            Ok(_) => return Ok(()),
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "status responses did not converge within {:?}; last_error={err:?}",
                        timeout,
                    ));
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_converged_height(
    network: &Network,
    target_height: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
    loop {
        match collect_statuses(network, STATUS_POLL_TIMEOUT).await {
            Ok(statuses) => {
                last_snapshot.clear();
                last_snapshot.extend(statuses.iter().map(StatusSnapshot::from_status));
                if statuses.iter().all(|status| status.blocks >= target_height) {
                    let first_height = statuses.first().map(|s| s.blocks);
                    if statuses
                        .iter()
                        .all(|status| Some(status.blocks) == first_height)
                    {
                        return Ok(());
                    }
                }
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "heights failed to converge to {target_height} within {:?}: last_snapshot={last_snapshot:?}",
                        timeout
                    ));
                }
            }
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "heights failed to converge to {target_height} within {:?}: last_snapshot={last_snapshot:?}, last_error={err:?}",
                        timeout
                    ));
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

fn scale_duration(duration: Duration, factor: u64) -> Duration {
    let total_ms = duration.as_millis().saturating_mul(u128::from(factor));
    Duration::from_millis(u64::try_from(total_ms).unwrap_or(u64::MAX))
}

#[derive(Debug)]
#[allow(dead_code)]
struct StatusSnapshot {
    blocks: u64,
    queue_size: u64,
    txs_approved: u64,
    txs_rejected: u64,
    view_changes: u32,
    leader_index: Option<u64>,
    highest_qc_height: Option<u64>,
    locked_qc_height: Option<u64>,
    tx_queue_depth: Option<u64>,
    tx_queue_saturated: Option<bool>,
    block_created_dropped_by_lock_total: Option<u64>,
    block_created_hint_mismatch_total: Option<u64>,
    block_created_proposal_mismatch_total: Option<u64>,
    commit_signatures_present: Option<u64>,
    commit_signatures_required: Option<u64>,
}

impl StatusSnapshot {
    fn from_status(status: &iroha::client::Status) -> Self {
        let sumeragi = status.sumeragi.as_ref();
        Self {
            blocks: status.blocks,
            queue_size: status.queue_size,
            txs_approved: status.txs_approved,
            txs_rejected: status.txs_rejected,
            view_changes: status.view_changes,
            leader_index: sumeragi.map(|s| s.leader_index),
            highest_qc_height: sumeragi.map(|s| s.highest_qc_height),
            locked_qc_height: sumeragi.map(|s| s.locked_qc_height),
            tx_queue_depth: sumeragi.map(|s| s.tx_queue_depth),
            tx_queue_saturated: sumeragi.map(|s| s.tx_queue_saturated),
            block_created_dropped_by_lock_total: sumeragi
                .map(|s| s.block_created_dropped_by_lock_total),
            block_created_hint_mismatch_total: sumeragi
                .map(|s| s.block_created_hint_mismatch_total),
            block_created_proposal_mismatch_total: sumeragi
                .map(|s| s.block_created_proposal_mismatch_total),
            commit_signatures_present: sumeragi.map(|s| s.commit_signatures_present),
            commit_signatures_required: sumeragi.map(|s| s.commit_signatures_required),
        }
    }
}
