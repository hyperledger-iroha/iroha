#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Adversarial RBC scenarios exercising Sumeragi DA/RBC debug knobs.

use std::{
    cmp::Ordering,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use eyre::{Report, Result, WrapErr, ensure, eyre};
use iroha::{
    client::{Client, Status},
    data_model::{
        Level,
        block::consensus_v2::{PayloadEncoding, PayloadManifest},
        isi::{Log, SetParameter},
        parameter::{Parameter, SumeragiParameter},
    },
};
use iroha_core::sumeragi::network_topology::commit_quorum_from_len;
use iroha_crypto::{Hash, HashOf};
use iroha_test_network::NetworkBuilder;
use norito::codec::DecodeAll as _;
use norito::json::{self, Map, Value};
use tokio::time::sleep;
use toml::Table;

const DEFAULT_PAYLOAD_BYTES: usize = 512 * 1024; // 512 KiB
const CHUNK_DROP_QUORUM_WAIT: Duration = Duration::from_secs(60);
const CHUNK_REORDER_NETWORK_START_ATTEMPTS: usize = 3;
const CHUNK_REORDER_NETWORK_START_RETRY_DELAY: Duration = Duration::from_secs(1);
const DUPLICATE_INIT_QUORUM_TIMEOUT: Duration = Duration::from_secs(420);

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_chunk_drop() -> Result<()> {
    run_chunk_drop_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_chunk_reorder() -> Result<()> {
    run_chunk_reorder_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_witness_corruption() -> Result<()> {
    run_witness_corruption_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_duplicate_inits() -> Result<()> {
    run_duplicate_inits_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_chunk_drop_recovery() -> Result<()> {
    run_chunk_drop_recovery_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_validator_selective_drop() -> Result<()> {
    run_validator_selective_drop_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_chunk_equivocation_marks_invalid() -> Result<()> {
    run_chunk_equivocation_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_conflicting_ready_marks_invalid() -> Result<()> {
    run_conflicting_ready_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_locked_qc_gate_rejects_conflicting_proposal() -> Result<()> {
    run_locked_qc_gate_drop_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_partial_chunk_withholding_stalls_delivery() -> Result<()> {
    run_partial_erasure_scenario().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_adversarial_all_chunks_corrupted_abort() -> Result<()> {
    run_all_chunks_corrupted_scenario().await
}

use integration_tests::sandbox;

async fn run_chunk_drop_scenario() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(["sumeragi", "debug", "rbc", "drop_every_nth_chunk"], 2_i64);
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_chunk_drop_scenario)).await?
    else {
        return Ok(());
    };

    let client = network.client();
    let cluster_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session =
        wait_for_rbc_session(&network, &client, expected_height, Duration::from_secs(20)).await?;
    let session_height =
        session_height(&session).ok_or_else(|| eyre!("missing height in chunk-drop session"))?;
    sleep(Duration::from_secs(2)).await;
    let status_after = blocking_status(&client)?;
    let sessions_after = rbc_observation_snapshot(&network, &client, session_height).await?;

    let progress_quorum = commit_quorum_from_len(cluster_clients.len()).max(1);
    let status_after_all = match try_wait_for_cluster_height_quorum(
        &cluster_clients,
        expected_height,
        progress_quorum,
        CHUNK_DROP_QUORUM_WAIT,
    )
    .await?
    {
        Some(statuses) => statuses,
        None => collect_client_statuses_best_effort(&cluster_clients)?,
    };
    let min_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(status_after.blocks);
    let max_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(status_after.blocks);
    let delivered = require_bool(&session, "delivered")?
        || any_delivered_session_for_height(&sessions_after, session_height);
    let incomplete = session_has_missing_chunks(&session)
        || any_incomplete_session_for_height(&sessions_after, session_height);
    let progress_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_all, expected_height);
    let chunk_drop_progress = classify_chunk_drop_progress(
        status_before.blocks,
        expected_height,
        min_blocks,
        max_blocks,
        progress_quorum,
        progress_quorum_blocks,
    )
    .ok_or_else(|| {
        eyre!(
            "chunk drop should either reach commit quorum, remain pinned, or expose only bounded partial progress; before={}, expected={expected_height}, min={min_blocks}, max={max_blocks}, quorum={progress_quorum}, quorum_blocks={progress_quorum_blocks}, delivered={delivered}, incomplete={incomplete}",
            status_before.blocks
        )
    })?;
    if chunk_drop_progress.requires_loss_evidence() {
        ensure!(
            incomplete || !delivered,
            "chunk drop {} outcome requires loss evidence; delivered={delivered}, incomplete={incomplete}",
            chunk_drop_progress.as_str()
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("chunk_drop"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert("status_after_blocks".into(), Value::from(max_blocks));
    summary_map.insert("status_after_min_blocks".into(), Value::from(min_blocks));
    summary_map.insert("progress_quorum".into(), Value::from(progress_quorum));
    summary_map.insert(
        "progress_quorum_blocks".into(),
        Value::from(progress_quorum_blocks),
    );
    summary_map.insert("delivered".into(), Value::from(delivered));
    summary_map.insert("incomplete".into(), Value::from(incomplete));
    summary_map.insert(
        "progress_outcome".into(),
        Value::from(chunk_drop_progress.as_str()),
    );
    summary_map.insert("rbc_session".into(), session.clone());
    emit_summary("chunk_drop", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn run_chunk_reorder_scenario() -> Result<()> {
    let context = stringify!(run_chunk_reorder_scenario);
    let network = 'startup: loop {
        for attempt in 1..=CHUNK_REORDER_NETWORK_START_ATTEMPTS {
            let builder = NetworkBuilder::new()
                .with_peers(4)
                .with_auto_populated_trusted_peers()
                .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::DaEnabled(true),
                )))
                .with_config_layer(|layer| {
                    layer
                        .write("telemetry_enabled", true)
                        .write("telemetry_profile", "full")
                        .write(["sumeragi", "da", "enabled"], true)
                        .write(
                            ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                            16_i64 * 1024,
                        )
                        .write(["sumeragi", "debug", "rbc", "shuffle_chunks"], true);
                });
            match sandbox::start_network_async_or_skip(builder, context).await {
                Ok(Some(network)) => break 'startup network,
                Ok(None) => return Ok(()),
                Err(err)
                    if attempt < CHUNK_REORDER_NETWORK_START_ATTEMPTS
                        && err.chain().any(|cause| {
                            let text = cause.to_string();
                            text.contains("expected peers to start within timeout")
                                || text.contains("peer startup failed; startup snapshot:")
                        }) =>
                {
                    eprintln!(
                        "warning: {context} network rebuild attempt {attempt}/{CHUNK_REORDER_NETWORK_START_ATTEMPTS} failed after startup retries; retrying in {:?}: {err}",
                        CHUNK_REORDER_NETWORK_START_RETRY_DELAY
                    );
                    sleep(CHUNK_REORDER_NETWORK_START_RETRY_DELAY).await;
                }
                Err(err) => return Err(err),
            }
        }
        unreachable!("chunk reorder startup retry loop exits via break or return");
    };

    let client = network.client();
    let cluster_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session =
        try_wait_for_rbc_session(&network, &client, expected_height, Duration::from_secs(40))
            .await?;
    let session_height = session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    let progress_quorum = commit_quorum_from_len(cluster_clients.len()).max(1);
    let status_after_all = match try_wait_for_cluster_height_quorum(
        &cluster_clients,
        expected_height,
        progress_quorum,
        DUPLICATE_INIT_QUORUM_TIMEOUT,
    )
    .await?
    {
        Some(statuses) => statuses,
        None => collect_client_statuses(&cluster_clients)?,
    };
    let min_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(status_before.blocks);
    let max_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(status_before.blocks);
    let progress_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_all, expected_height);
    let sessions_after = rbc_observation_snapshot(&network, &client, session_height).await?;

    let delivered = optional_session_bool(session.as_ref(), "delivered")?
        || any_delivered_session_for_height(&sessions_after, session_height);
    let complete = any_complete_session_for_height(&sessions_after, session_height);
    if max_blocks >= expected_height {
        ensure!(
            progress_quorum_blocks >= progress_quorum,
            "reorder scenario should expose height {expected_height} on commit quorum {progress_quorum}; observed {progress_quorum_blocks} peers at/above target (min={min_blocks}, max={max_blocks})"
        );
        ensure!(
            max_blocks.saturating_sub(min_blocks) <= 1,
            "reorder scenario should not cause unbounded cluster divergence (min={min_blocks}, max={max_blocks})"
        );
    } else {
        ensure!(
            max_blocks == status_before.blocks,
            "reorder stall should keep the cluster at the prior height when delivery is not observed (before={}, max={max_blocks})",
            status_before.blocks
        );
    }
    ensure!(
        delivered
            || complete
            || extract_sessions_for_height(&sessions_after, session_height).is_empty(),
        "reorder scenario should either deliver or retain complete RBC chunk telemetry when session summaries remain present"
    );

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("chunk_reorder"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert("status_after_blocks".into(), Value::from(max_blocks));
    summary_map.insert("status_after_min_blocks".into(), Value::from(min_blocks));
    summary_map.insert("progress_quorum".into(), Value::from(progress_quorum));
    summary_map.insert(
        "progress_quorum_blocks".into(),
        Value::from(progress_quorum_blocks),
    );
    summary_map.insert("rbc_session".into(), session.unwrap_or(Value::Null));
    emit_summary("chunk_reorder", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn run_witness_corruption_scenario() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(["sumeragi", "debug", "rbc", "corrupt_witness_ack"], true);
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_witness_corruption_scenario))
            .await?
    else {
        return Ok(());
    };

    let client = network.client();

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session =
        try_wait_for_rbc_session(&network, &client, expected_height, Duration::from_secs(30))
            .await?;
    let session_height = session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    let observation_deadline = Instant::now() + Duration::from_secs(60);
    let (status_after, delivered, complete, retired) = loop {
        let status_after = blocking_status(&client)?;
        let sessions_after = rbc_observation_snapshot(&network, &client, session_height).await?;
        let delivered = optional_session_bool(session.as_ref(), "delivered")?
            || any_delivered_session_for_height(&sessions_after, session_height);
        let complete = any_complete_session_for_height(&sessions_after, session_height);
        let retired = extract_sessions_for_height(&sessions_after, session_height).is_empty();
        if status_after.blocks != status_before.blocks
            || delivered
            || complete
            || retired
            || Instant::now() >= observation_deadline
        {
            break (status_after, delivered, complete, retired);
        }
        sleep(Duration::from_millis(200)).await;
    };
    ensure!(
        status_after.blocks == status_before.blocks,
        "witness corruption should gate commit height even when the RBC session completes"
    );
    ensure!(
        delivered || complete || retired,
        "witness corruption should still complete RBC delivery telemetry or retire the session after completion"
    );
    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("witness_corruption"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert(
        "status_after_blocks".into(),
        Value::from(status_after.blocks),
    );
    summary_map.insert("rbc_session".into(), session.unwrap_or(Value::Null));
    emit_summary("witness_corruption", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn run_duplicate_inits_scenario() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(["sumeragi", "debug", "rbc", "duplicate_inits"], true);
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_duplicate_inits_scenario))
            .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let cluster_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session =
        try_wait_for_rbc_session(&network, &client, expected_height, Duration::from_secs(40))
            .await?;
    let session_height = session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    let mut views: Vec<u64> = Vec::new();
    for peer in network.peers() {
        let sessions_value =
            rbc_observation_snapshot(&network, &peer.client(), session_height).await?;
        views.extend(
            extract_sessions_for_height(&sessions_value, session_height)
                .iter()
                .filter_map(|value| value.as_object()?.get("view")?.as_u64()),
        );
    }
    let progress_quorum = commit_quorum_from_len(cluster_clients.len()).max(1);
    let status_after_all = match try_wait_for_cluster_height_quorum(
        &cluster_clients,
        expected_height,
        progress_quorum,
        DUPLICATE_INIT_QUORUM_TIMEOUT,
    )
    .await?
    {
        Some(statuses) => statuses,
        None => collect_client_statuses(&cluster_clients)?,
    };
    let min_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(status_before.blocks);
    let max_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(status_before.blocks);
    let progress_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_all, expected_height);

    let base_view = session
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|obj| obj.get("view"))
        .and_then(Value::as_u64);
    for peer in network.peers() {
        let sessions_value =
            rbc_observation_snapshot(&network, &peer.client(), session_height).await?;
        views.extend(
            extract_sessions_for_height(&sessions_value, session_height)
                .iter()
                .filter_map(|value| value.as_object()?.get("view")?.as_u64()),
        );
    }
    let mut saw_duplicate_session_evidence = false;
    if let Some(base_view) = base_view {
        let repeated_base_view_entries = views.iter().filter(|view| **view == base_view).count();
        let saw_consecutive_views =
            views.contains(&base_view) && views.contains(&(base_view.saturating_add(1)));
        saw_duplicate_session_evidence = repeated_base_view_entries >= 2 || saw_consecutive_views;
        ensure!(
            saw_duplicate_session_evidence || views.is_empty(),
            "expected duplicate-init evidence via repeated base-view sessions or consecutive views when RBC session telemetry is present (base={base_view}, repeated_base_view_entries={repeated_base_view_entries}, observed_views={views:?})"
        );
    }
    let duplicate_progress = classify_duplicate_init_progress(
        status_before.blocks,
        expected_height,
        min_blocks,
        max_blocks,
        progress_quorum,
        progress_quorum_blocks,
    )
    .ok_or_else(|| {
        eyre!(
            "duplicate-init scenario made unsafe or unclassified progress toward height {expected_height}: before={}, min={min_blocks}, max={max_blocks}, quorum={progress_quorum}, peers_at_target={progress_quorum_blocks}",
            status_before.blocks
        )
    })?;
    if duplicate_progress == DuplicateInitProgressOutcome::BoundedPartialProgress {
        ensure!(
            saw_duplicate_session_evidence || views.is_empty(),
            "bounded partial duplicate-init progress requires duplicate session evidence or absent session telemetry; observed_views={views:?}"
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("duplicate_inits"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert("status_after_blocks".into(), Value::from(max_blocks));
    summary_map.insert("status_after_min_blocks".into(), Value::from(min_blocks));
    summary_map.insert("progress_quorum".into(), Value::from(progress_quorum));
    summary_map.insert(
        "progress_quorum_blocks".into(),
        Value::from(progress_quorum_blocks),
    );
    summary_map.insert(
        "progress_outcome".into(),
        Value::from(duplicate_progress.as_str()),
    );
    summary_map.insert("rbc_session".into(), session.unwrap_or(Value::Null));
    emit_summary("duplicate_inits", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn run_chunk_drop_recovery_scenario() -> Result<()> {
    let drop_builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(["sumeragi", "debug", "rbc", "drop_every_nth_chunk"], 2_i64);
        });
    let Some(drop_network) = sandbox::start_network_async_or_skip(
        drop_builder,
        stringify!(run_chunk_drop_recovery_scenario_drop_phase),
    )
    .await?
    else {
        return Ok(());
    };

    let drop_client = drop_network.client();

    let status_before_drop = blocking_status(&drop_client)?;
    let expected_height = status_before_drop.blocks + 1;

    submit_heavy_log(&drop_client, DEFAULT_PAYLOAD_BYTES).await?;
    let drop_session = try_wait_for_rbc_session(
        &drop_network,
        &drop_client,
        expected_height,
        Duration::from_secs(40),
    )
    .await?;
    sleep(Duration::from_secs(2)).await;
    let status_after_drop = blocking_status(&drop_client)?;
    let drop_delivered = optional_session_bool(drop_session.as_ref(), "delivered")?;
    if drop_delivered || status_after_drop.blocks >= expected_height {
        ensure!(
            status_after_drop.blocks >= expected_height,
            "drop phase delivered via local payload recovery; expected commit height to advance"
        );
    } else {
        ensure!(
            status_after_drop.blocks == status_before_drop.blocks,
            "drop phase should keep commit height unchanged when delivery stalls"
        );
    }

    drop_network.shutdown_and_release().await;

    let recovery_builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true);
        });
    let Some(recovery_network) = sandbox::start_network_async_or_skip(
        recovery_builder,
        stringify!(run_chunk_drop_recovery_scenario_recovery_phase),
    )
    .await?
    else {
        return Ok(());
    };

    let recovery_client = recovery_network.client();
    let recovery_clients: Vec<Client> = recovery_network
        .peers()
        .iter()
        .map(|peer| peer.client())
        .collect();
    let recovery_quorum = commit_quorum_from_len(recovery_clients.len()).max(1);
    let status_before_recovery = blocking_status(&recovery_client)?;
    let recovery_height = status_before_recovery.blocks + 1;

    submit_heavy_log(&recovery_client, DEFAULT_PAYLOAD_BYTES).await?;
    let recovery_session = try_wait_for_rbc_session(
        &recovery_network,
        &recovery_client,
        recovery_height,
        Duration::from_secs(60),
    )
    .await?;
    let status_after_recovery_all = match try_wait_for_cluster_height_quorum(
        &recovery_clients,
        recovery_height,
        recovery_quorum,
        Duration::from_secs(180),
    )
    .await?
    {
        Some(statuses) => statuses,
        None => collect_client_statuses_best_effort(&recovery_clients)?,
    };
    let recovery_min_blocks = status_after_recovery_all
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(status_before_recovery.blocks);
    let recovery_max_blocks = status_after_recovery_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(status_before_recovery.blocks);
    let recovery_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_recovery_all, recovery_height);
    ensure!(
        recovery_max_blocks >= recovery_height,
        "recovery phase should make post-drop progress to height {recovery_height} (before={}, min={recovery_min_blocks}, max={recovery_max_blocks})",
        status_before_recovery.blocks
    );
    ensure!(
        recovery_quorum_blocks >= recovery_quorum,
        "recovery phase should expose height {recovery_height} on commit quorum {recovery_quorum}; observed {recovery_quorum_blocks} peers at/above target out of {} status responses (min={recovery_min_blocks}, max={recovery_max_blocks})",
        status_after_recovery_all.len()
    );
    ensure!(
        recovery_max_blocks.saturating_sub(recovery_min_blocks) <= 1,
        "recovery phase should keep the cluster within one block after progress resumes (min={recovery_min_blocks}, max={recovery_max_blocks})"
    );

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("chunk_drop_recovery"));
    summary_map.insert(
        "drop_status_before".into(),
        Value::from(status_before_drop.blocks),
    );
    summary_map.insert(
        "drop_status_after".into(),
        Value::from(status_after_drop.blocks),
    );
    summary_map.insert("drop_session".into(), drop_session.unwrap_or(Value::Null));
    summary_map.insert(
        "recovery_status_before".into(),
        Value::from(status_before_recovery.blocks),
    );
    summary_map.insert(
        "recovery_status_after".into(),
        Value::from(recovery_max_blocks),
    );
    summary_map.insert(
        "recovery_status_after_min".into(),
        Value::from(recovery_min_blocks),
    );
    summary_map.insert("recovery_quorum".into(), Value::from(recovery_quorum));
    summary_map.insert(
        "recovery_quorum_blocks".into(),
        Value::from(recovery_quorum_blocks),
    );
    summary_map.insert(
        "recovery_session".into(),
        recovery_session.unwrap_or(Value::Null),
    );
    summary_map.insert("drop_expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "recovery_expected_height".into(),
        Value::from(recovery_height),
    );
    emit_summary("chunk_drop_recovery", &Value::Object(summary_map))?;

    recovery_network.shutdown().await;
    Ok(())
}

async fn run_validator_selective_drop_scenario() -> Result<()> {
    const DROP_MASK: i64 = 0b0010;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "drop_validator_mask"],
                    DROP_MASK,
                );
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(run_validator_selective_drop_scenario),
    )
    .await?
    else {
        return Ok(());
    };

    let base_client = network.client();

    let status_before = blocking_status(&base_client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&base_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ = try_wait_for_rbc_session(
        &network,
        &base_client,
        expected_height,
        Duration::from_secs(20),
    )
    .await?;
    sleep(Duration::from_secs(3)).await;

    let mut missing = 0usize;
    let mut complete = 0usize;
    let mut delivered = 0usize;

    for peer in network.peers() {
        let peer_client = peer.client();
        let Some(session) = try_wait_for_rbc_session(
            &network,
            &peer_client,
            expected_height,
            Duration::from_secs(20),
        )
        .await?
        else {
            missing += 1;
            continue;
        };
        let (total, received) = require_session_chunk_counts(&session)?;
        if total == 0 {
            continue;
        }
        match received.cmp(&total) {
            Ordering::Less => missing += 1,
            Ordering::Equal => complete += 1,
            Ordering::Greater => {}
        }
        if require_bool(&session, "delivered")? {
            delivered += 1;
        }
    }

    let peer_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();
    let progress_quorum = commit_quorum_from_len(peer_clients.len()).max(1);
    let status_after_all = if delivered > 0 {
        match try_wait_for_cluster_height_quorum(
            &peer_clients,
            expected_height,
            progress_quorum,
            Duration::from_secs(20),
        )
        .await?
        {
            Some(statuses) => statuses,
            None => collect_client_statuses_best_effort(&peer_clients)
                .wrap_err("collect selective-drop statuses after quorum wait")?,
        }
    } else {
        collect_client_statuses_best_effort(&peer_clients)?
    };
    let max_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or_else(|| {
            blocking_status(&base_client)
                .map(|status| status.blocks)
                .unwrap_or(0)
        });
    let progress_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_all, expected_height);
    let status_after = blocking_status(&base_client)?;
    if max_blocks < expected_height {
        ensure!(
            status_after.blocks == status_before.blocks,
            "block height must remain unchanged while selective drop prevents delivery"
        );
        ensure!(
            missing >= 1
                || complete < network.peers().len()
                || complete.saturating_sub(delivered) >= 1
                || delivered >= network.peers().len(),
            "selective drop stall should leave missing/incomplete RBC telemetry or complete sessions that never reached delivery (missing={missing}, complete={complete}, delivered={delivered}, peers={})",
            network.peers().len()
        );
    } else {
        ensure!(
            delivered >= 1 || complete >= network.peers().len().saturating_sub(1) || missing >= 1,
            "selective drop recovery should leave delivery evidence, near-complete sessions, or at least bounded missing telemetry (complete={complete}, delivered={delivered}, missing={missing}, peers={})",
            network.peers().len()
        );
        ensure!(
            progress_quorum_blocks >= progress_quorum,
            "when selective drop is healed by local payload recovery, commit height should advance on commit quorum {progress_quorum}; observed {progress_quorum_blocks} peers at/above height {expected_height} (max={max_blocks})"
        );
    }

    network.shutdown().await;
    Ok(())
}

async fn run_chunk_equivocation_scenario() -> Result<()> {
    const TARGET_VALIDATOR_IDX: usize = 2;
    const EQUIVOCATE_MASK: i64 = 1 << TARGET_VALIDATOR_IDX;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(["sumeragi", "debug", "rbc", "equivocate_chunk_mask"], 1_i64)
                .write(
                    ["sumeragi", "debug", "rbc", "equivocate_validator_mask"],
                    EQUIVOCATE_MASK,
                );
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_chunk_equivocation_scenario))
            .await?
    else {
        return Ok(());
    };

    let targeted_peer = network
        .peers()
        .get(TARGET_VALIDATOR_IDX)
        .ok_or_else(|| eyre!("targeted validator index {TARGET_VALIDATOR_IDX} missing"))?;
    let targeted_client = targeted_peer.client();

    let status_before = blocking_status(&targeted_client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&targeted_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ = try_wait_for_rbc_session(
        &network,
        &targeted_client,
        expected_height,
        Duration::from_secs(20),
    )
    .await?;
    sleep(Duration::from_secs(3)).await;

    let mut missing_sessions = 0usize;
    let mut stalled_sessions = 0usize;
    let mut delivered_sessions = 0usize;

    for peer in network.peers() {
        let client = peer.client();
        let Some(session) =
            try_wait_for_rbc_session(&network, &client, expected_height, Duration::from_secs(20))
                .await?
        else {
            missing_sessions += 1;
            continue;
        };
        let delivered = require_bool(&session, "delivered")?;
        if delivered {
            delivered_sessions += 1;
        } else {
            stalled_sessions += 1;
        }
    }

    let status_after = blocking_status(&targeted_client)?;
    let mut status_after_all = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        status_after_all.push(blocking_status(&peer.client())?);
    }
    let progress_quorum = commit_quorum_from_len(status_after_all.len()).max(1);
    if count_statuses_at_or_above_height(&status_after_all, expected_height) < progress_quorum
        && status_after_all
            .iter()
            .any(|status| status.blocks >= expected_height)
    {
        let peer_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();
        if let Some(quorum_statuses) = try_wait_for_cluster_height_quorum(
            &peer_clients,
            expected_height,
            progress_quorum,
            Duration::from_secs(60),
        )
        .await?
        {
            status_after_all = quorum_statuses;
        }
    }
    let min_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(status_before.blocks);
    let max_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(status_before.blocks);
    let progress_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_all, expected_height);

    if max_blocks >= expected_height {
        // Grouped and exact runs can recover from isolated chunk equivocation. Once the
        // cluster commits, bounded convergence is the authoritative signal that the
        // honest validators rejected bad ingress and recovered canonical chunks.
        ensure!(
            progress_quorum_blocks >= progress_quorum,
            "equivocation recovery should expose height {expected_height} on commit quorum {progress_quorum}; observed {progress_quorum_blocks} peers at/above target (min={min_blocks}, max={max_blocks})"
        );
        ensure!(
            max_blocks.saturating_sub(min_blocks) <= 1,
            "equivocation should not cause unbounded height divergence (min={min_blocks}, max={max_blocks})"
        );
    } else {
        ensure!(
            status_after.blocks == status_before.blocks,
            "targeted validator should remain at the prior height when equivocation prevents recovery"
        );
        ensure!(
            missing_sessions >= 1 || stalled_sessions >= 1,
            "equivocation stall should leave at least one peer without a reconstructable canonical payload (missing_sessions={missing_sessions}, stalled_sessions={stalled_sessions}, delivered_sessions={delivered_sessions})"
        );
    }

    network.shutdown().await;
    Ok(())
}

async fn run_all_chunks_corrupted_scenario() -> Result<()> {
    const PEER_COUNT: usize = 4;
    const CHUNK_MASK: i64 = 1;
    const VALIDATOR_MASK: i64 = (1i64 << PEER_COUNT) - 1;

    let builder = NetworkBuilder::new()
        .with_peers(PEER_COUNT)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "equivocate_chunk_mask"],
                    CHUNK_MASK,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "equivocate_validator_mask"],
                    VALIDATOR_MASK,
                );
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(run_all_chunks_corrupted_scenario),
    )
    .await?
    else {
        return Ok(());
    };

    let mut status_before = Vec::with_capacity(PEER_COUNT);
    for peer in network.peers() {
        status_before.push(blocking_status(&peer.client())?);
    }
    let base_client = network.client();

    let expected_height = status_before.first().map_or(1, |status| status.blocks + 1);

    submit_heavy_log(&base_client, DEFAULT_PAYLOAD_BYTES).await?;

    let mut sessions = Vec::with_capacity(PEER_COUNT);
    for peer in network.peers() {
        let session = wait_for_rbc_session(
            &network,
            &peer.client(),
            expected_height,
            Duration::from_secs(20),
        )
        .await?;
        sessions.push(session);
    }

    sleep(Duration::from_secs(3)).await;

    let cluster_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();
    let mut delivered_total = 0usize;
    let mut stalled_total = 0usize;
    for session in &sessions {
        let delivered = require_bool(session, "delivered")?;
        if delivered {
            delivered_total += 1;
        } else {
            stalled_total += 1;
        }
    }

    let mut status_after = Vec::with_capacity(PEER_COUNT);
    for peer in network.peers() {
        let status = blocking_status(&peer.client())?;
        status_after.push(status);
    }

    let base_height = status_before
        .first()
        .map(|status| status.blocks)
        .unwrap_or(0);
    let progress_quorum = commit_quorum_from_len(cluster_clients.len()).max(1);
    if count_statuses_at_or_above_height(&status_after, expected_height) < progress_quorum
        && status_after
            .iter()
            .any(|status| status.blocks >= expected_height)
    {
        if let Some(quorum_statuses) = try_wait_for_cluster_height_quorum(
            &cluster_clients,
            expected_height,
            progress_quorum,
            Duration::from_secs(60),
        )
        .await?
        {
            status_after = quorum_statuses;
        }
    }
    let min_blocks = status_after
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(base_height);
    let max_blocks = status_after
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(base_height);
    let progress_quorum_blocks = count_statuses_at_or_above_height(&status_after, expected_height);

    if max_blocks >= expected_height {
        // A fully converged commit is the durable signal here. Exact/grouped runs can rotate or
        // clear the local `delivered=true` snapshot before the assertions inspect telemetry.
        ensure!(
            progress_quorum_blocks >= progress_quorum,
            "uniform corruption recovery should expose height {expected_height} on commit quorum {progress_quorum}; observed {progress_quorum_blocks} peers at/above target (min={min_blocks}, max={max_blocks})"
        );
        ensure!(
            max_blocks.saturating_sub(min_blocks) <= 1,
            "heights diverged under uniform corruption (min={min_blocks}, max={max_blocks})"
        );
    } else {
        ensure!(
            max_blocks == base_height,
            "unexpected partial height advance under corrupted shards (base={base_height}, max={max_blocks})"
        );
        for (idx, status_after) in status_after.iter().enumerate() {
            ensure!(
                status_after.blocks == status_before[idx].blocks,
                "peer {idx} must not advance height under corrupted shards"
            );
        }
        ensure!(
            delivered_total == 0 && stalled_total == PEER_COUNT,
            "rejected corrupted ingress must leave every persisted session below the canonical reconstruction threshold (delivered_total={delivered_total}, stalled_total={stalled_total})"
        );
    }
    let mut summary_map = Map::new();
    summary_map.insert(
        "scenario".into(),
        Value::from("all_chunks_corrupted".to_owned()),
    );
    summary_map.insert("peer_count".into(), Value::from(PEER_COUNT as u64));
    summary_map.insert(
        "delivered_sessions".into(),
        Value::from(delivered_total as u64),
    );
    summary_map.insert("stalled_sessions".into(), Value::from(stalled_total as u64));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert("base_height".into(), Value::from(base_height));
    summary_map.insert("min_blocks".into(), Value::from(min_blocks));
    summary_map.insert("max_blocks".into(), Value::from(max_blocks));
    summary_map.insert("progress_quorum".into(), Value::from(progress_quorum));
    summary_map.insert(
        "progress_quorum_blocks".into(),
        Value::from(progress_quorum_blocks),
    );
    emit_summary("all_chunks_corrupted", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn run_conflicting_ready_scenario() -> Result<()> {
    const TARGET_VALIDATOR_IDX: usize = 0;
    const FORK_MASK: i64 = 1 << TARGET_VALIDATOR_IDX;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                );
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_conflicting_ready_scenario))
            .await?
    else {
        return Ok(());
    };

    let targeted_peer = network
        .peers()
        .get(TARGET_VALIDATOR_IDX)
        .ok_or_else(|| eyre!("targeted validator index {TARGET_VALIDATOR_IDX} missing"))?;
    let targeted_client = targeted_peer.client();
    let cluster_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();

    let status_before = blocking_status(&targeted_client)?;
    let mut debug_layer = Table::new();
    let mut rbc = toml::map::Map::new();
    rbc.insert(
        "conflicting_ready_mask".into(),
        toml::Value::Integer(FORK_MASK),
    );
    let mut debug = toml::map::Map::new();
    debug.insert("rbc".into(), toml::Value::Table(rbc));
    let mut sumeragi = toml::map::Map::new();
    sumeragi.insert("debug".into(), toml::Value::Table(debug));
    debug_layer.insert("sumeragi".into(), toml::Value::Table(sumeragi));
    restart_network_with_extra_layer(
        &network,
        debug_layer,
        stringify!(run_conflicting_ready_scenario),
    )
    .await?;

    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&targeted_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ = try_wait_for_rbc_session(
        &network,
        &targeted_client,
        expected_height,
        Duration::from_secs(20),
    )
    .await?;
    sleep(Duration::from_secs(4)).await;

    let mut delivered_sessions = 0usize;
    let mut missing_sessions = 0usize;
    let mut retained_nondelivered_sessions = 0usize;

    for peer in network.peers() {
        let Some(session) = try_wait_for_rbc_session(
            &network,
            &peer.client(),
            expected_height,
            Duration::from_secs(20),
        )
        .await?
        else {
            missing_sessions += 1;
            continue;
        };
        if require_bool(&session, "delivered")? {
            delivered_sessions += 1;
        } else {
            retained_nondelivered_sessions += 1;
        }
    }

    let status_after = blocking_status(&targeted_client)?;
    let progress_quorum = commit_quorum_from_len(cluster_clients.len()).max(1);
    let status_after_all = if delivered_sessions > 0 {
        match try_wait_for_cluster_height_quorum(
            &cluster_clients,
            expected_height,
            progress_quorum,
            Duration::from_secs(60),
        )
        .await?
        {
            Some(statuses) => statuses,
            None => collect_client_statuses_best_effort(&cluster_clients)?,
        }
    } else {
        collect_client_statuses_best_effort(&cluster_clients)?
    };
    let min_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(status_before.blocks);
    let max_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(status_before.blocks);
    let progress_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_all, expected_height);

    if max_blocks >= expected_height {
        // Honest validators can recover and commit after rejecting conflicting ingress.
        ensure!(
            progress_quorum_blocks >= progress_quorum,
            "conflicting READY recovery should expose height {expected_height} on commit quorum {progress_quorum}; observed {progress_quorum_blocks} peers at/above target (min={min_blocks}, max={max_blocks})"
        );
        ensure!(
            max_blocks.saturating_sub(min_blocks) <= 1,
            "conflicting READY scenario should not cause unbounded height divergence (min={min_blocks}, max={max_blocks})"
        );
    } else {
        ensure!(
            status_after.blocks == status_before.blocks,
            "validator should remain at the prior height when conflicting READY prevents recovery"
        );
        ensure!(
            missing_sessions >= 1 || retained_nondelivered_sessions >= 1 || delivered_sessions >= 1,
            "conflicting READY stall should surface missing sessions, retained non-reconstructable sessions, or canonical payload recovery gated by consensus (missing={missing_sessions}, retained_nondelivered={retained_nondelivered_sessions}, delivered={delivered_sessions})"
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("conflicting_ready"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert("status_after_blocks".into(), Value::from(max_blocks));
    summary_map.insert("status_after_min_blocks".into(), Value::from(min_blocks));
    summary_map.insert("progress_quorum".into(), Value::from(progress_quorum));
    summary_map.insert(
        "progress_quorum_blocks".into(),
        Value::from(progress_quorum_blocks),
    );
    summary_map.insert(
        "delivered_sessions".into(),
        Value::from(delivered_sessions as u64),
    );
    summary_map.insert(
        "retained_nondelivered_sessions".into(),
        Value::from(retained_nondelivered_sessions as u64),
    );
    emit_summary("conflicting_ready", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn restart_network_with_extra_layer(
    network: &iroha_test_network::Network,
    extra_layer: Table,
    context: &str,
) -> Result<()> {
    network.shutdown().await;
    let base_layers: Vec<ConfigLayer> = network
        .config_layers()
        .map(|layer| ConfigLayer(layer.into_owned()))
        .collect();
    let extra_layer = ConfigLayer(extra_layer);
    for (index, peer) in network.peers().iter().enumerate() {
        let mut layers = base_layers.clone();
        layers.push(extra_layer.clone());
        peer.start_checked(layers.into_iter(), None)
            .await
            .wrap_err_with(|| format!("restart peer {index} for {context}"))?;
    }
    network
        .ensure_blocks(1)
        .await
        .wrap_err_with(|| format!("reach block 1 after {context} debug restart"))?;
    Ok(())
}

async fn run_locked_qc_gate_drop_scenario() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(["sumeragi", "debug", "rbc", "duplicate_inits"], true);
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_locked_qc_gate_drop_scenario))
            .await?
    else {
        return Ok(());
    };

    let client = network.client();

    let status_before = blocking_status(&client)?;
    let view_installs_before = sum_v2_view_change_installs(network.peers()).await?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let primary_session =
        try_wait_for_rbc_session(&network, &client, expected_height, Duration::from_secs(80))
            .await?;
    let primary_height = primary_session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    let base_view = primary_session
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|obj| obj.get("view"))
        .and_then(Value::as_u64);

    let primary_delivered = optional_session_bool(primary_session.as_ref(), "delivered")?;
    let observation_deadline = Instant::now() + Duration::from_secs(60);
    let (status_after, delivered_after, mut duplicate_views, view_installs_after) = loop {
        let status_after = blocking_status(&client)?;
        let mut delivered_after = false;
        let mut duplicate_views: Vec<u64> = Vec::new();
        for peer in network.peers() {
            let sessions_value =
                rbc_observation_snapshot(&network, &peer.client(), primary_height).await?;
            delivered_after |= any_delivered_session_for_height(&sessions_value, primary_height);
            duplicate_views.extend(
                extract_sessions_for_height(&sessions_value, primary_height)
                    .iter()
                    .filter_map(|value| value.as_object()?.get("view")?.as_u64()),
            );
        }

        let view_installs_after = sum_v2_view_change_installs(network.peers()).await?;

        let repeated_base_view_entries = base_view
            .map(|base_view| {
                duplicate_views
                    .iter()
                    .filter(|view| **view == base_view)
                    .count()
            })
            .unwrap_or(0);
        let duplicate_view_evidence = base_view.is_some_and(|base_view| {
            repeated_base_view_entries >= 2
                || (duplicate_views.contains(&base_view)
                    && duplicate_views.contains(&(base_view.saturating_add(1))))
        });
        let installed_new_view = view_installs_after > view_installs_before;
        if status_after.blocks >= expected_height
            || delivered_after
            || installed_new_view
            || duplicate_view_evidence
            || Instant::now() >= observation_deadline
        {
            break (
                status_after,
                delivered_after,
                duplicate_views,
                view_installs_after,
            );
        }
        sleep(Duration::from_millis(500)).await;
    };

    if primary_delivered || delivered_after || status_after.blocks >= expected_height {
        ensure!(
            status_after.blocks >= expected_height,
            "locked QC gate scenario recovered with delivered RBC session; expected commit height to advance"
        );
    } else {
        ensure!(
            status_after.blocks == status_before.blocks,
            "locked QC gate scenario must keep commit height unchanged while the primary session is gated"
        );
    }
    ensure!(
        view_installs_after >= view_installs_before,
        "authoritative v2 view-install counters must be monotonic across the validator set (before={view_installs_before}, after={view_installs_after})"
    );
    let repeated_base_view_entries = base_view
        .map(|base_view| {
            duplicate_views
                .iter()
                .filter(|view| **view == base_view)
                .count()
        })
        .unwrap_or(0);
    duplicate_views.sort_unstable();
    duplicate_views.dedup();
    let duplicate_view_evidence = base_view.is_some_and(|base_view| {
        repeated_base_view_entries >= 2
            || (duplicate_views.contains(&base_view)
                && duplicate_views.contains(&(base_view.saturating_add(1))))
    });
    ensure!(
        view_installs_after > view_installs_before || duplicate_view_evidence,
        "locked QC gate should install a durable v2 view change or expose exact duplicate-session evidence (view_installs_before={view_installs_before}, view_installs_after={view_installs_after}, repeated_base_view_entries={repeated_base_view_entries}, observed={duplicate_views:?})"
    );
    network.shutdown().await;
    Ok(())
}

async fn run_partial_erasure_scenario() -> Result<()> {
    const PARTIAL_MASK: i64 = 0b1; // withhold the first chunk deterministically
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "partial_chunk_mask"],
                    PARTIAL_MASK,
                );
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_partial_erasure_scenario))
            .await?
    else {
        return Ok(());
    };

    let base_client = network.client();
    let cluster_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();

    let status_before = blocking_status(&base_client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&base_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ = try_wait_for_rbc_session(
        &network,
        &base_client,
        expected_height,
        Duration::from_secs(20),
    )
    .await?;
    sleep(Duration::from_secs(5)).await;

    let mut stalled_sessions = 0usize;
    let mut delivered_sessions = 0usize;
    let mut missing_sessions = 0usize;
    let mut retained_nondelivered_sessions = 0usize;
    let peer_count = network.peers().len();
    let progress_quorum = commit_quorum_from_len(peer_count).max(1);

    for peer in network.peers() {
        let Some(session) = try_wait_for_rbc_session(
            &network,
            &peer.client(),
            expected_height,
            Duration::from_secs(20),
        )
        .await?
        else {
            missing_sessions += 1;
            continue;
        };
        let (total, received) = require_session_chunk_counts(&session)?;
        if total > received {
            stalled_sessions += 1;
        }
        if require_bool(&session, "delivered")? {
            delivered_sessions += 1;
        } else {
            retained_nondelivered_sessions += 1;
        }
    }

    let status_after_all = match try_wait_for_cluster_height_quorum(
        &cluster_clients,
        expected_height,
        progress_quorum,
        Duration::from_secs(180),
    )
    .await?
    {
        Some(statuses) => statuses,
        None => collect_client_statuses_best_effort(&cluster_clients)?,
    };
    let min_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .min()
        .unwrap_or(status_before.blocks);
    let max_blocks = status_after_all
        .iter()
        .map(|status| status.blocks)
        .max()
        .unwrap_or(status_before.blocks);
    let progress_quorum_blocks =
        count_statuses_at_or_above_height(&status_after_all, expected_height);
    if max_blocks < expected_height {
        ensure!(
            stalled_sessions >= peer_count.saturating_sub(1)
                || retained_nondelivered_sessions >= peer_count.saturating_sub(1)
                || delivered_sessions >= 1
                || missing_sessions >= 1,
            "partial-erasure should stall or retain every non-origin validator session, or leave missing/delivered RBC telemetry when cluster progress stays blocked (stalled={stalled_sessions}, retained_nondelivered={retained_nondelivered_sessions}, delivered={delivered_sessions}, missing={missing_sessions}, peers={peer_count})"
        );
        ensure!(
            max_blocks == status_before.blocks && min_blocks == status_before.blocks,
            "block height must remain unchanged while chunks are withheld (before={}, min={min_blocks}, max={max_blocks})",
            status_before.blocks
        );
    } else {
        ensure!(
            stalled_sessions >= 1
                || delivered_sessions >= 1
                || retained_nondelivered_sessions >= 1
                || missing_sessions >= 1,
            "partial-erasure recovery should still expose stalled, retained, delivered, or bounded missing telemetry (stalled={stalled_sessions}, retained_nondelivered={retained_nondelivered_sessions}, delivered={delivered_sessions}, missing={missing_sessions}, peers={peer_count})"
        );
        ensure!(
            progress_quorum_blocks >= progress_quorum,
            "partial-erasure recovery should expose height {expected_height} on commit quorum {progress_quorum}; observed {progress_quorum_blocks} peers at/above target (min={min_blocks}, max={max_blocks})"
        );
        ensure!(
            max_blocks.saturating_sub(min_blocks) <= 1,
            "when withheld chunks recover, the cluster should stay within one block of convergence (min={min_blocks}, max={max_blocks})"
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("partial_erasure"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert("peer_count".into(), Value::from(peer_count as u64));
    summary_map.insert("progress_quorum".into(), Value::from(progress_quorum));
    summary_map.insert(
        "progress_quorum_blocks".into(),
        Value::from(progress_quorum_blocks),
    );
    summary_map.insert(
        "stalled_sessions".into(),
        Value::from(stalled_sessions as u64),
    );
    summary_map.insert(
        "delivered_sessions".into(),
        Value::from(delivered_sessions as u64),
    );
    summary_map.insert(
        "retained_nondelivered_sessions".into(),
        Value::from(retained_nondelivered_sessions as u64),
    );
    summary_map.insert(
        "missing_sessions".into(),
        Value::from(missing_sessions as u64),
    );
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert("status_after_blocks".into(), Value::from(max_blocks));
    summary_map.insert("status_after_min_blocks".into(), Value::from(min_blocks));
    summary_map.insert("recovered_commit_height".into(), Value::from(max_blocks));
    emit_summary("partial_erasure", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn sum_v2_view_change_installs(peers: &[iroha_test_network::NetworkPeer]) -> Result<u64> {
    let mut total = 0_u64;
    for peer in peers {
        let client = peer.client();
        let status = tokio::task::spawn_blocking(move || client.get_sumeragi_v2_status())
            .await
            .wrap_err("join authoritative Sumeragi v2 status fetch")?
            .wrap_err("fetch authoritative Sumeragi v2 status")?;
        total = total.saturating_add(status.operator.view_change_install_total);
    }
    Ok(total)
}

async fn submit_heavy_log(client: &Client, bytes: usize) -> Result<()> {
    let payload = "X".repeat(bytes);
    let client_clone = client.clone();
    tokio::task::spawn_blocking(move || client_clone.submit(Log::new(Level::INFO, payload)))
        .await
        .wrap_err("join submit task")?
        .map(|_| ())
        .wrap_err("submit heavy log")
}

fn blocking_status(client: &Client) -> Result<Status> {
    let client_clone = client.clone();
    tokio::task::block_in_place(|| client_clone.get_status()).wrap_err("fetch status")
}

fn is_transient_status_fetch_error(err: &Report) -> bool {
    const NEEDLES: [&str; 5] = [
        "Failed to send http",
        "error sending request for url",
        "Connection refused",
        "connection closed",
        "operation timed out",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

fn is_transient_rbc_sessions_fetch_error(err: &Report) -> bool {
    is_transient_status_fetch_error(err)
}

#[derive(norito::Decode)]
struct StoredV2ChunkManifest {
    version: u16,
    manifest: PayloadManifest,
}

fn directory_entries(path: &std::path::Path) -> Result<Vec<fs::DirEntry>> {
    match fs::read_dir(path) {
        Ok(entries) => entries
            .collect::<std::io::Result<Vec<_>>>()
            .wrap_err_with(|| format!("read directory entries from {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).wrap_err_with(|| format!("read directory {}", path.display())),
    }
}

fn persisted_v2_session_items(
    store_root: &std::path::Path,
    target_height: u64,
) -> Result<Vec<Value>> {
    const V2_CHUNK_STORE_VERSION: u16 = 1;

    let chunk_root = store_root.join("sumeragi_v2").join("chunks");
    let mut items = Vec::new();
    for context_entry in directory_entries(&chunk_root)? {
        ensure!(
            context_entry.file_type()?.is_dir(),
            "unexpected non-directory entry in v2 chunk root: {}",
            context_entry.path().display()
        );
        for session_entry in directory_entries(&context_entry.path())? {
            ensure!(
                session_entry.file_type()?.is_dir(),
                "unexpected non-directory entry in v2 context directory: {}",
                session_entry.path().display()
            );
            let session_path = session_entry.path();
            let manifest_path = session_path.join("manifest.norito");
            let manifest_bytes = match fs::read(&manifest_path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error)
                        .wrap_err_with(|| format!("read {}", manifest_path.display()));
                }
            };
            let mut cursor = manifest_bytes.as_slice();
            let stored = StoredV2ChunkManifest::decode_all(&mut cursor)
                .map_err(|error| eyre!("decode {}: {error}", manifest_path.display()))?;
            ensure!(
                stored.version == V2_CHUNK_STORE_VERSION,
                "unsupported v2 chunk-store version {} in {}",
                stored.version,
                manifest_path.display()
            );
            stored.manifest.layout.validate().map_err(|error| {
                eyre!(
                    "invalid persisted v2 layout in {}: {error}",
                    manifest_path.display()
                )
            })?;
            let expected_context_name = hex::encode(stored.manifest.round.context_id.0.as_ref());
            ensure!(
                context_entry.file_name().to_string_lossy() == expected_context_name,
                "persisted v2 context directory does not match manifest context in {}",
                manifest_path.display()
            );
            let manifest_hash = HashOf::new(&stored.manifest);
            let expected_manifest_name = hex::encode(manifest_hash.as_ref());
            ensure!(
                session_entry.file_name().to_string_lossy() == expected_manifest_name,
                "persisted v2 session directory does not match manifest hash in {}",
                manifest_path.display()
            );
            if stored.manifest.round.height < target_height {
                continue;
            }

            let mut received = vec![false; stored.manifest.chunk_hashes.len()];
            for chunk_entry in directory_entries(&session_path)? {
                ensure!(
                    chunk_entry.file_type()?.is_file(),
                    "unexpected non-file entry in v2 session directory: {}",
                    chunk_entry.path().display()
                );
                let name = chunk_entry.file_name();
                let Some(name) = name.to_str() else {
                    return Err(eyre!(
                        "non-UTF-8 entry in persisted v2 session {}",
                        session_path.display()
                    ));
                };
                if name == "manifest.norito" || name.ends_with(".tmp") {
                    continue;
                }
                let Some(index) = name
                    .strip_suffix(".chunk")
                    .and_then(|index| index.parse::<usize>().ok())
                else {
                    return Err(eyre!(
                        "unexpected entry in persisted v2 session: {}",
                        chunk_entry.path().display()
                    ));
                };
                let expected_hash = stored.manifest.chunk_hashes.get(index).ok_or_else(|| {
                    eyre!(
                        "persisted chunk index {index} is outside manifest in {}",
                        session_path.display()
                    )
                })?;
                let chunk_path = chunk_entry.path();
                let chunk = fs::read(&chunk_path)
                    .wrap_err_with(|| format!("read persisted chunk {}", chunk_path.display()))?;
                ensure!(
                    Hash::new(&chunk) == *expected_hash,
                    "persisted authenticated v2 chunk failed its manifest hash in {}",
                    chunk_path.display()
                );
                let slot = received.get_mut(index).ok_or_else(|| {
                    eyre!(
                        "persisted chunk index {index} is outside manifest in {}",
                        session_path.display()
                    )
                })?;
                ensure!(
                    !std::mem::replace(slot, true),
                    "persisted v2 session contains duplicate chunk index {index} in {}",
                    session_path.display()
                );
            }

            let total_chunks = u64::try_from(stored.manifest.chunk_hashes.len())
                .wrap_err("v2 manifest chunk count does not fit u64")?;
            let received_chunks =
                u64::try_from(received.iter().filter(|present| **present).count())
                    .wrap_err("persisted v2 received chunk count does not fit u64")?;
            let reconstructable = persisted_chunks_reconstructable(
                stored.manifest.layout.encoding,
                stored.manifest.layout.data_shards,
                stored.manifest.layout.parity_shards,
                stored.manifest.chunk_hashes.len(),
                &received,
            )?;
            // The production worker reconstructs synchronously as soon as this exact
            // per-stripe threshold is persisted, so reconstructability is durable local
            // delivery evidence. Chain height alone cannot identify the winning manifest.
            items.push(norito::json!({
                "block_hash": (stored.manifest.subject.block_hash.to_string()),
                "manifest_hash": expected_manifest_name,
                "height": (stored.manifest.round.height),
                "view": (stored.manifest.round.view),
                "delivered": reconstructable,
                "total_chunks": total_chunks,
                "received_chunks": received_chunks
            }));
        }
    }
    Ok(items)
}

fn persisted_chunks_reconstructable(
    encoding: PayloadEncoding,
    data_shards: u16,
    parity_shards: u16,
    expected_chunks: usize,
    received: &[bool],
) -> Result<bool> {
    ensure!(
        received.len() == expected_chunks,
        "persisted v2 chunk presence does not match manifest geometry"
    );
    match encoding {
        PayloadEncoding::Plain => {
            Ok(!received.is_empty() && received.iter().all(|present| *present))
        }
        PayloadEncoding::ReedSolomon16 => {
            let data_shards = usize::from(data_shards);
            let stripe_width = data_shards
                .checked_add(usize::from(parity_shards))
                .ok_or_else(|| eyre!("persisted v2 RS16 stripe width overflow"))?;
            ensure!(
                data_shards > 0
                    && stripe_width > data_shards
                    && !received.is_empty()
                    && received.len().is_multiple_of(stripe_width),
                "persisted v2 RS16 chunk presence has invalid stripe geometry"
            );
            Ok(received
                .chunks_exact(stripe_width)
                .all(|stripe| stripe.iter().filter(|present| **present).count() >= data_shards))
        }
    }
}

/// Build a per-session observation from authenticated v2 manifests and chunks
/// persisted by the peer that serves `client`.
async fn rbc_observation_snapshot(
    network: &sandbox::SerializedNetwork,
    client: &Client,
    target_height: u64,
) -> Result<Value> {
    let store_root = network
        .peers()
        .iter()
        .find(|peer| peer.client().torii_url == client.torii_url)
        .map(|peer| peer.kura_store_dir())
        .ok_or_else(|| {
            eyre!(
                "client {} is not part of the test network",
                client.torii_url
            )
        })?;
    let items =
        tokio::task::spawn_blocking(move || persisted_v2_session_items(&store_root, target_height))
            .await
            .wrap_err("join Sumeragi status observation")??;
    Ok(norito::json!({ "items": items }))
}

async fn wait_for_rbc_session(
    network: &sandbox::SerializedNetwork,
    client: &Client,
    target_height: u64,
    timeout: Duration,
) -> Result<Value> {
    try_wait_for_rbc_session(network, client, target_height, timeout)
        .await?
        .ok_or_else(|| {
            eyre!("timed out waiting for RBC session at or after height {target_height}")
        })
}

async fn try_wait_for_rbc_session(
    network: &sandbox::SerializedNetwork,
    client: &Client,
    target_height: u64,
    timeout: Duration,
) -> Result<Option<Value>> {
    let client = client.clone();
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Ok(None);
        }
        let sessions = match rbc_observation_snapshot(network, &client, target_height).await {
            Ok(sessions) => sessions,
            Err(err) if is_transient_status_fetch_error(&err) => {
                sleep(Duration::from_millis(200)).await;
                continue;
            }
            Err(err) => return Err(err),
        };

        if let Some(session) = extract_session_at_or_after(&sessions, target_height) {
            return Ok(Some(session));
        }
        sleep(Duration::from_millis(200)).await;
    }
}

fn collect_client_statuses(clients: &[Client]) -> Result<Vec<Status>> {
    clients.iter().map(blocking_status).collect()
}

fn collect_client_statuses_best_effort(clients: &[Client]) -> Result<Vec<Status>> {
    let mut statuses = Vec::with_capacity(clients.len());
    let mut transient_err = None;

    for client in clients {
        match blocking_status(client) {
            Ok(status) => statuses.push(status),
            Err(err) if is_transient_status_fetch_error(&err) => transient_err = Some(err),
            Err(err) => return Err(err),
        }
    }

    if statuses.is_empty() {
        return Err(transient_err.unwrap_or_else(|| eyre!("no recovery client status available")));
    }

    Ok(statuses)
}

async fn try_wait_for_cluster_height_quorum(
    clients: &[Client],
    target_height: u64,
    quorum: usize,
    timeout: Duration,
) -> Result<Option<Vec<Status>>> {
    let deadline = Instant::now() + timeout;
    loop {
        let statuses = collect_client_statuses_best_effort(clients)?;
        if count_statuses_at_or_above_height(&statuses, target_height) >= quorum {
            return Ok(Some(statuses));
        }
        if Instant::now() > deadline {
            return Ok(None);
        }
        sleep(Duration::from_millis(200)).await;
    }
}

fn count_statuses_at_or_above_height(statuses: &[Status], target_height: u64) -> usize {
    count_heights_at_or_above_height(statuses.iter().map(|status| status.blocks), target_height)
}

fn count_heights_at_or_above_height<I>(heights: I, target_height: u64) -> usize
where
    I: IntoIterator<Item = u64>,
{
    heights
        .into_iter()
        .filter(|height| *height >= target_height)
        .count()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChunkDropProgressOutcome {
    CommitQuorumProgress,
    BoundedPartialProgress,
    PinnedStall,
}

impl ChunkDropProgressOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::CommitQuorumProgress => "commit_quorum_progress",
            Self::BoundedPartialProgress => "bounded_partial_progress",
            Self::PinnedStall => "pinned_stall",
        }
    }

    fn requires_loss_evidence(self) -> bool {
        matches!(self, Self::BoundedPartialProgress | Self::PinnedStall)
    }
}

fn classify_chunk_drop_progress(
    status_before_blocks: u64,
    expected_height: u64,
    min_blocks: u64,
    max_blocks: u64,
    progress_quorum: usize,
    progress_quorum_blocks: usize,
) -> Option<ChunkDropProgressOutcome> {
    if max_blocks >= expected_height {
        if max_blocks.saturating_sub(min_blocks) > 1 {
            return None;
        }
        if progress_quorum_blocks >= progress_quorum {
            return Some(ChunkDropProgressOutcome::CommitQuorumProgress);
        }
        if progress_quorum_blocks > 0 && min_blocks == status_before_blocks {
            return Some(ChunkDropProgressOutcome::BoundedPartialProgress);
        }
        return None;
    }

    (max_blocks == status_before_blocks && min_blocks == status_before_blocks)
        .then_some(ChunkDropProgressOutcome::PinnedStall)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DuplicateInitProgressOutcome {
    CommitQuorumProgress,
    BoundedPartialProgress,
    PinnedStall,
}

impl DuplicateInitProgressOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::CommitQuorumProgress => "commit_quorum_progress",
            Self::BoundedPartialProgress => "bounded_partial_progress",
            Self::PinnedStall => "pinned_stall",
        }
    }
}

fn classify_duplicate_init_progress(
    status_before_blocks: u64,
    expected_height: u64,
    min_blocks: u64,
    max_blocks: u64,
    progress_quorum: usize,
    progress_quorum_blocks: usize,
) -> Option<DuplicateInitProgressOutcome> {
    if max_blocks >= expected_height {
        if max_blocks.saturating_sub(min_blocks) > 1 {
            return None;
        }
        if progress_quorum_blocks >= progress_quorum {
            return Some(DuplicateInitProgressOutcome::CommitQuorumProgress);
        }
        if progress_quorum_blocks > 0 && min_blocks == status_before_blocks {
            return Some(DuplicateInitProgressOutcome::BoundedPartialProgress);
        }
        return None;
    }

    (max_blocks == status_before_blocks && min_blocks == status_before_blocks)
        .then_some(DuplicateInitProgressOutcome::PinnedStall)
}

fn extract_session(value: &Value, target_height: u64) -> Option<Value> {
    let items = value
        .as_object()
        .and_then(|obj| obj.get("items"))?
        .as_array()?;
    for item in items {
        let height = item
            .as_object()
            .and_then(|obj| obj.get("height"))
            .and_then(Value::as_u64)?;
        if height == target_height {
            return Some(item.clone());
        }
    }
    None
}

#[test]
fn transient_rbc_sessions_fetch_errors_include_connect_failures() {
    let connect_err = eyre!("Connection refused (os error 61)");
    let permanent_err = eyre!("permission denied");

    assert!(is_transient_rbc_sessions_fetch_error(&connect_err));
    assert!(!is_transient_rbc_sessions_fetch_error(&permanent_err));
}

#[test]
fn persisted_chunk_reconstruction_uses_manifest_geometry() {
    assert!(
        persisted_chunks_reconstructable(PayloadEncoding::Plain, 0, 0, 2, &[true, true]).unwrap()
    );
    assert!(
        !persisted_chunks_reconstructable(PayloadEncoding::Plain, 0, 0, 2, &[true, false]).unwrap()
    );

    assert!(
        persisted_chunks_reconstructable(
            PayloadEncoding::ReedSolomon16,
            4,
            2,
            6,
            &[true, true, true, true, false, false],
        )
        .unwrap()
    );
    assert!(
        !persisted_chunks_reconstructable(
            PayloadEncoding::ReedSolomon16,
            4,
            2,
            6,
            &[true, true, true, false, true, false],
        )
        .unwrap()
    );
    assert!(
        !persisted_chunks_reconstructable(
            PayloadEncoding::ReedSolomon16,
            4,
            2,
            12,
            &[
                true, true, true, true, false, false, true, true, true, false, false, false,
            ],
        )
        .unwrap()
    );
    assert!(
        persisted_chunks_reconstructable(PayloadEncoding::ReedSolomon16, 4, 2, 5, &[true; 5],)
            .is_err()
    );
}

#[test]
fn count_heights_at_or_above_height_counts_quorum_candidates() {
    assert_eq!(count_heights_at_or_above_height([], 4), 0);
    assert_eq!(count_heights_at_or_above_height([3, 4, 4, 5], 4), 3);
    assert_eq!(count_heights_at_or_above_height([5, 6, 7], 4), 3);
    assert_eq!(count_heights_at_or_above_height([1, 2, 3], 4), 0);
}

#[test]
fn chunk_drop_progress_classifier_accepts_safe_outcomes() {
    assert_eq!(
        classify_chunk_drop_progress(1, 2, 1, 2, 3, 3),
        Some(ChunkDropProgressOutcome::CommitQuorumProgress)
    );
    assert_eq!(
        classify_chunk_drop_progress(1, 2, 1, 2, 3, 2),
        Some(ChunkDropProgressOutcome::BoundedPartialProgress)
    );
    assert_eq!(
        classify_chunk_drop_progress(1, 2, 1, 1, 3, 0),
        Some(ChunkDropProgressOutcome::PinnedStall)
    );
}

#[test]
fn chunk_drop_progress_classifier_rejects_unsafe_splits() {
    assert_eq!(classify_chunk_drop_progress(1, 2, 1, 3, 3, 2), None);
    assert_eq!(classify_chunk_drop_progress(1, 2, 2, 2, 3, 0), None);
    assert_eq!(classify_chunk_drop_progress(1, 2, 1, 2, 3, 0), None);
}

#[test]
fn chunk_drop_loss_evidence_is_required_for_faulted_non_quorum_outcomes() {
    assert!(!ChunkDropProgressOutcome::CommitQuorumProgress.requires_loss_evidence());
    assert!(ChunkDropProgressOutcome::BoundedPartialProgress.requires_loss_evidence());
    assert!(ChunkDropProgressOutcome::PinnedStall.requires_loss_evidence());
}

#[test]
fn duplicate_init_progress_classifier_accepts_safe_outcomes() {
    assert_eq!(
        classify_duplicate_init_progress(1, 2, 1, 2, 3, 3),
        Some(DuplicateInitProgressOutcome::CommitQuorumProgress)
    );
    assert_eq!(
        classify_duplicate_init_progress(1, 2, 1, 2, 3, 2),
        Some(DuplicateInitProgressOutcome::BoundedPartialProgress)
    );
    assert_eq!(
        classify_duplicate_init_progress(1, 2, 1, 1, 3, 0),
        Some(DuplicateInitProgressOutcome::PinnedStall)
    );
}

#[test]
fn duplicate_init_progress_classifier_rejects_unsafe_splits() {
    assert_eq!(classify_duplicate_init_progress(1, 2, 1, 3, 3, 2), None);
    assert_eq!(classify_duplicate_init_progress(1, 2, 2, 2, 3, 0), None);
    assert_eq!(classify_duplicate_init_progress(1, 2, 1, 2, 3, 0), None);
}

fn extract_session_at_or_after(value: &Value, target_height: u64) -> Option<Value> {
    if let Some(session) = extract_session(value, target_height) {
        return Some(session);
    }
    let items = value
        .as_object()
        .and_then(|obj| obj.get("items"))?
        .as_array()?;
    let mut best: Option<(u64, Value)> = None;
    for item in items {
        let height = item
            .as_object()
            .and_then(|obj| obj.get("height"))
            .and_then(Value::as_u64)?;
        if height < target_height {
            continue;
        }
        match &best {
            Some((best_height, _)) if *best_height <= height => {}
            _ => best = Some((height, item.clone())),
        }
    }
    best.map(|(_, item)| item)
}

fn extract_sessions_for_height(value: &Value, target_height: u64) -> Vec<Value> {
    value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|vals| vals.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|item| {
                    item.as_object()
                        .and_then(|obj| obj.get("height"))
                        .and_then(Value::as_u64)
                        == Some(target_height)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn get_u64(value: &Value, key: &str) -> Option<u64> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_u64)
}

fn get_bool(value: &Value, key: &str) -> Option<bool> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_bool)
}

fn require_u64(value: &Value, key: &str) -> Result<u64> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("RBC session is not an object while reading `{key}`: {value:?}"))?;
    let raw = obj
        .get(key)
        .ok_or_else(|| eyre!("RBC session is missing required `{key}` field: {value:?}"))?;
    raw.as_u64()
        .ok_or_else(|| eyre!("RBC session field `{key}` is not a u64: {raw:?}"))
}

fn require_bool(value: &Value, key: &str) -> Result<bool> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("RBC session is not an object while reading `{key}`: {value:?}"))?;
    let raw = obj
        .get(key)
        .ok_or_else(|| eyre!("RBC session is missing required `{key}` field: {value:?}"))?;
    raw.as_bool()
        .ok_or_else(|| eyre!("RBC session field `{key}` is not a bool: {raw:?}"))
}

fn optional_session_bool(value: Option<&Value>, key: &str) -> Result<bool> {
    match value {
        Some(value) => require_bool(value, key),
        None => Ok(false),
    }
}

fn require_session_chunk_counts(value: &Value) -> Result<(u64, u64)> {
    let total = require_u64(value, "total_chunks")?;
    let received = require_u64(value, "received_chunks")?;
    ensure!(
        received <= total,
        "RBC session received_chunks must not exceed total_chunks: received={received}, total={total}, session={value:?}"
    );
    Ok((total, received))
}

fn session_chunk_counts(value: &Value) -> Option<(u64, u64)> {
    let total = get_u64(value, "total_chunks")?;
    let received = get_u64(value, "received_chunks")?;
    (received <= total).then_some((total, received))
}

fn session_height(value: &Value) -> Option<u64> {
    value
        .as_object()
        .and_then(|obj| obj.get("height"))
        .and_then(Value::as_u64)
}

fn any_delivered_session_for_height(value: &Value, target_height: u64) -> bool {
    extract_sessions_for_height(value, target_height)
        .iter()
        .any(|session| get_bool(session, "delivered") == Some(true))
}

fn any_complete_session_for_height(value: &Value, target_height: u64) -> bool {
    extract_sessions_for_height(value, target_height)
        .iter()
        .any(|session| {
            session_chunk_counts(session)
                .is_some_and(|(total, received)| total > 0 && received >= total)
        })
}

fn session_has_missing_chunks(value: &Value) -> bool {
    session_chunk_counts(value).is_some_and(|(total, received)| total > 0 && received < total)
}

fn any_incomplete_session_for_height(value: &Value, target_height: u64) -> bool {
    extract_sessions_for_height(value, target_height)
        .iter()
        .any(session_has_missing_chunks)
}

#[test]
fn delivered_height_check_scans_all_sessions_for_the_height() {
    let sessions = norito::json!({
        "items": [
            {"height": 3, "view": 0, "delivered": false},
            {"height": 3, "view": 1, "delivered": true},
            {"height": 4, "view": 0, "delivered": true}
        ]
    });

    assert!(any_delivered_session_for_height(&sessions, 3));
    assert!(any_delivered_session_for_height(&sessions, 4));
    assert!(!any_delivered_session_for_height(&sessions, 5));
}

#[test]
fn complete_height_check_accepts_full_chunk_telemetry_without_delivered_flag() {
    let sessions = norito::json!({
        "items": [
            {"height": 3, "view": 0, "delivered": false, "total_chunks": 8, "received_chunks": 8},
            {"height": 3, "view": 1, "delivered": false, "total_chunks": 8, "received_chunks": 6},
            {"height": 4, "view": 0, "delivered": true, "total_chunks": 2, "received_chunks": 2}
        ]
    });

    assert!(any_complete_session_for_height(&sessions, 3));
    assert!(any_complete_session_for_height(&sessions, 4));
    assert!(!any_complete_session_for_height(&sessions, 5));
}

#[test]
fn incomplete_height_check_accepts_delivered_sessions_with_missing_chunks() {
    let sessions = norito::json!({
        "items": [
            {"height": 3, "view": 0, "delivered": true, "total_chunks": 8, "received_chunks": 6},
            {"height": 4, "view": 0, "delivered": false, "total_chunks": 2, "received_chunks": 2}
        ]
    });

    assert!(any_incomplete_session_for_height(&sessions, 3));
    assert!(!any_incomplete_session_for_height(&sessions, 4));
    assert!(!any_incomplete_session_for_height(&sessions, 5));
}

#[test]
fn chunk_telemetry_checks_fail_closed_on_malformed_counts() {
    let sessions = norito::json!({
        "items": [
            {"height": 3, "view": 0, "delivered": false, "total_chunks": 8},
            {"height": 3, "view": 1, "delivered": false, "total_chunks": 8, "received_chunks": "1"},
            {"height": 3, "view": 2, "delivered": false, "total_chunks": 4, "received_chunks": 5},
            {"height": 4, "view": 0, "delivered": false, "total_chunks": 4, "received_chunks": 1}
        ]
    });

    assert!(!any_complete_session_for_height(&sessions, 3));
    assert!(!any_incomplete_session_for_height(&sessions, 3));
    assert!(any_incomplete_session_for_height(&sessions, 4));
}

#[test]
fn required_session_reads_reject_missing_or_malformed_evidence_fields() {
    let valid = norito::json!({
        "height": 3,
        "view": 0,
        "delivered": true,
        "total_chunks": 4,
        "received_chunks": 4
    });
    assert_eq!(require_session_chunk_counts(&valid).unwrap(), (4, 4));
    assert!(require_bool(&valid, "delivered").unwrap());

    let missing_received = norito::json!({
        "height": 3,
        "view": 0,
        "delivered": false,
        "total_chunks": 4
    });
    assert!(require_session_chunk_counts(&missing_received).is_err());

    let malformed_delivered = norito::json!({
        "height": 3,
        "view": 0,
        "delivered": "true",
        "total_chunks": 4,
        "received_chunks": 4
    });
    assert!(require_bool(&malformed_delivered, "delivered").is_err());

    let over_counted = norito::json!({
        "height": 3,
        "view": 0,
        "delivered": false,
        "total_chunks": 4,
        "received_chunks": 5
    });
    assert!(require_session_chunk_counts(&over_counted).is_err());
}

fn emit_summary(scenario: &str, summary: &Value) -> Result<()> {
    let pretty = json::to_json_pretty(summary).wrap_err("serialize summary")?;
    println!("sumeragi_adversarial::{scenario}::{pretty}");
    persist_summary_if_requested(scenario, &pretty)?;
    Ok(())
}

fn persist_summary_if_requested(scenario: &str, summary_pretty: &str) -> Result<()> {
    let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {
        return Ok(());
    };
    let root = PathBuf::from(dir);
    fs::create_dir_all(&root).wrap_err("create adversarial artifact dir")?;
    let path = root.join(format!("{scenario}.summary.json"));
    fs::write(path, format!("{summary_pretty}\n")).wrap_err("write adversarial summary")?;
    Ok(())
}
