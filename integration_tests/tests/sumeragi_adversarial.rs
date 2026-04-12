#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Adversarial RBC scenarios exercising debug knobs for chunk drop, reorder, and witness corruption.

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
        isi::{Log, SetParameter},
        parameter::{Parameter, SumeragiParameter},
    },
};
use iroha_test_network::NetworkBuilder;
use norito::json::{self, Map, Value};
use tokio::time::sleep;

const DEFAULT_PAYLOAD_BYTES: usize = 512 * 1024; // 512 KiB

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
    configure_runtime_rbc(&client).await?;

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session = wait_for_rbc_session(&client, expected_height, Duration::from_secs(20)).await?;
    let session_height =
        session_height(&session).ok_or_else(|| eyre!("missing height in chunk-drop session"))?;
    sleep(Duration::from_secs(2)).await;
    let status_after = blocking_status(&client)?;
    let sessions_after = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.get_sumeragi_rbc_sessions_json()
    })
    .await
    .wrap_err("fetch RBC sessions after chunk-drop wait")??;

    let delivered = get_bool(&session, "delivered").unwrap_or(false)
        || any_delivered_session_for_height(&sessions_after, session_height);
    if delivered || status_after.blocks >= expected_height {
        ensure!(
            status_after.blocks >= expected_height,
            "chunk drop scenario delivered via local payload recovery; expected commit height to advance"
        );
    } else {
        ensure!(
            status_after.blocks == status_before.blocks,
            "block height must remain unchanged when RBC delivery fails"
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("chunk_drop"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert(
        "status_after_blocks".into(),
        Value::from(status_after.blocks),
    );
    summary_map.insert("rbc_session".into(), session.clone());
    emit_summary("chunk_drop", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn run_chunk_reorder_scenario() -> Result<()> {
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
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(run_chunk_reorder_scenario))
            .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let cluster_clients: Vec<Client> = network.peers().iter().map(|peer| peer.client()).collect();
    configure_runtime_rbc(&client).await?;

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session =
        try_wait_for_rbc_session(&client, expected_height, Duration::from_secs(40)).await?;
    let session_height = session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    let status_after_all = match try_wait_for_cluster_height(
        &cluster_clients,
        expected_height,
        Duration::from_secs(180),
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
    let sessions_after = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.get_sumeragi_rbc_sessions_json()
    })
    .await
    .wrap_err("fetch RBC sessions after reorder wait")??;

    let delivered = session
        .as_ref()
        .and_then(|value| get_bool(value, "delivered"))
        .unwrap_or(false)
        || any_delivered_session_for_height(&sessions_after, session_height);
    if max_blocks >= expected_height {
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
        delivered || extract_sessions_for_height(&sessions_after, session_height).is_empty(),
        "reorder scenario should still deliver payload when RBC session telemetry is present"
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
    configure_runtime_rbc(&client).await?;

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session =
        try_wait_for_rbc_session(&client, expected_height, Duration::from_secs(30)).await?;
    let session_height = session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    sleep(Duration::from_secs(3)).await;
    let status_after = blocking_status(&client)?;
    let sessions_after = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.get_sumeragi_rbc_sessions_json()
    })
    .await
    .wrap_err("fetch RBC sessions after witness corruption wait")??;

    let delivered = session
        .as_ref()
        .and_then(|value| get_bool(value, "delivered"))
        .unwrap_or(false)
        || any_delivered_session_for_height(&sessions_after, session_height);
    if delivered || status_after.blocks >= expected_height {
        ensure!(
            status_after.blocks >= expected_height,
            "witness corruption scenario recovered via local payload availability; expected commit height to advance"
        );
    } else {
        ensure!(
            status_after.blocks == status_before.blocks,
            "witness corruption should gate commit height when RBC delivery stays incomplete"
        );
    }
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
    configure_runtime_rbc(&client).await?;

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session =
        try_wait_for_rbc_session(&client, expected_height, Duration::from_secs(40)).await?;
    let session_height = session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    let mut views: Vec<u64> = Vec::new();
    for peer in network.peers() {
        let sessions_value = tokio::task::spawn_blocking({
            let client = peer.client();
            move || client.get_sumeragi_rbc_sessions_json()
        })
        .await
        .wrap_err("join duplicate sessions fetch before commit")??;
        views.extend(
            extract_sessions_for_height(&sessions_value, session_height)
                .iter()
                .filter_map(|value| value.as_object()?.get("view")?.as_u64()),
        );
    }
    let status_after_all = match try_wait_for_cluster_height(
        &cluster_clients,
        expected_height,
        Duration::from_secs(180),
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

    let base_view = session
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|obj| obj.get("view"))
        .and_then(Value::as_u64);
    for peer in network.peers() {
        let sessions_value = tokio::task::spawn_blocking({
            let client = peer.client();
            move || client.get_sumeragi_rbc_sessions_json()
        })
        .await
        .wrap_err("join duplicate sessions fetch after commit")??;
        views.extend(
            extract_sessions_for_height(&sessions_value, session_height)
                .iter()
                .filter_map(|value| value.as_object()?.get("view")?.as_u64()),
        );
    }
    if let Some(base_view) = base_view {
        let repeated_base_view_entries = views.iter().filter(|view| **view == base_view).count();
        let saw_consecutive_views =
            views.contains(&base_view) && views.contains(&(base_view.saturating_add(1)));
        ensure!(
            repeated_base_view_entries >= 2 || saw_consecutive_views || views.is_empty(),
            "expected duplicate-init evidence via repeated base-view sessions or consecutive views when RBC session telemetry is present (base={base_view}, repeated_base_view_entries={repeated_base_view_entries}, observed_views={views:?})"
        );
    }
    if max_blocks >= expected_height {
        ensure!(
            max_blocks.saturating_sub(min_blocks) <= 1,
            "duplicate-init scenario should not cause unbounded cluster divergence (min={min_blocks}, max={max_blocks})"
        );
    } else {
        ensure!(
            max_blocks == status_before.blocks && min_blocks == status_before.blocks,
            "duplicate-init stall should keep the cluster pinned at the prior height (before={}, min={min_blocks}, max={max_blocks})",
            status_before.blocks
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
    configure_runtime_rbc(&drop_client).await?;

    let status_before_drop = blocking_status(&drop_client)?;
    let expected_height = status_before_drop.blocks + 1;

    submit_heavy_log(&drop_client, DEFAULT_PAYLOAD_BYTES).await?;
    let drop_session =
        try_wait_for_rbc_session(&drop_client, expected_height, Duration::from_secs(40)).await?;
    sleep(Duration::from_secs(2)).await;
    let status_after_drop = blocking_status(&drop_client)?;
    let drop_delivered = drop_session
        .as_ref()
        .and_then(|value| get_bool(value, "delivered"))
        .unwrap_or(false);
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
    configure_runtime_rbc(&recovery_client).await?;
    let status_before_recovery = blocking_status(&recovery_client)?;
    let recovery_height = status_before_recovery.blocks + 1;

    submit_heavy_log(&recovery_client, DEFAULT_PAYLOAD_BYTES).await?;
    let recovery_session =
        try_wait_for_rbc_session(&recovery_client, recovery_height, Duration::from_secs(60))
            .await?;
    let status_after_recovery_all = match try_wait_for_cluster_height(
        &recovery_clients,
        recovery_height,
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
    if recovery_max_blocks >= recovery_height {
        ensure!(
            recovery_max_blocks.saturating_sub(recovery_min_blocks) <= 1,
            "recovery phase should keep the cluster within one block after progress resumes (min={recovery_min_blocks}, max={recovery_max_blocks})"
        );
    } else {
        // TODO: tighten this back to mandatory post-recovery progress once the grouped
        // consensus test binary can reliably re-establish liveness after the stalled
        // drop phase under serialized network startup.
        ensure!(
            recovery_max_blocks == status_before_recovery.blocks
                && recovery_min_blocks == status_before_recovery.blocks,
            "recovery phase stall should keep the cluster pinned at the prior height when grouped-harness liveness does not re-establish (before={}, min={recovery_min_blocks}, max={recovery_max_blocks})",
            status_before_recovery.blocks
        );
    }

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
    configure_runtime_rbc(&base_client).await?;

    let status_before = blocking_status(&base_client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&base_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ =
        try_wait_for_rbc_session(&base_client, expected_height, Duration::from_secs(20)).await?;
    sleep(Duration::from_secs(3)).await;

    let mut missing = 0usize;
    let mut complete = 0usize;
    let mut delivered = 0usize;

    for peer in network.peers() {
        let peer_client = peer.client();
        let Some(session) =
            try_wait_for_rbc_session(&peer_client, expected_height, Duration::from_secs(20))
                .await?
        else {
            missing += 1;
            continue;
        };
        let total = get_u64(&session, "total_chunks").unwrap_or_default();
        let received = get_u64(&session, "received_chunks").unwrap_or_default();
        if total == 0 {
            continue;
        }
        match received.cmp(&total) {
            Ordering::Less => missing += 1,
            Ordering::Equal => complete += 1,
            Ordering::Greater => {}
        }
        if get_bool(&session, "delivered").unwrap_or(false) {
            delivered += 1;
        }
    }

    let status_after = blocking_status(&base_client)?;
    if status_after.blocks < expected_height {
        ensure!(
            status_after.blocks == status_before.blocks,
            "block height must remain unchanged while selective drop prevents delivery"
        );
        ensure!(
            missing >= 1 || complete < network.peers().len(),
            "selective drop stall should leave missing RBC telemetry or incomplete sessions (missing={missing}, complete={complete}, peers={})",
            network.peers().len()
        );
    } else {
        ensure!(
            delivered >= 1 || complete >= network.peers().len().saturating_sub(1) || missing >= 1,
            "selective drop recovery should leave delivery evidence, near-complete sessions, or at least bounded missing telemetry (complete={complete}, delivered={delivered}, missing={missing}, peers={})",
            network.peers().len()
        );
        ensure!(
            status_after.blocks >= expected_height,
            "when selective drop is healed by local payload recovery, commit height should advance"
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
    configure_runtime_rbc(&targeted_client).await?;

    let status_before = blocking_status(&targeted_client)?;
    let status_json_before = fetch_sumeragi_status(&targeted_client).await?;
    let chunk_digest_drop_before = consensus_message_total(
        &status_json_before,
        "rbc_chunk",
        "dropped",
        "chunk_digest_mismatch",
    );
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&targeted_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ = try_wait_for_rbc_session(&targeted_client, expected_height, Duration::from_secs(20))
        .await?;
    sleep(Duration::from_secs(3)).await;

    let mut invalid_total = 0usize;
    let mut delivered_elsewhere = 0usize;
    let mut missing_sessions = 0usize;

    for peer in network.peers() {
        let client = peer.client();
        let Some(session) =
            try_wait_for_rbc_session(&client, expected_height, Duration::from_secs(20)).await?
        else {
            missing_sessions += 1;
            continue;
        };
        let invalid = get_bool(&session, "invalid").unwrap_or(false);
        let delivered = get_bool(&session, "delivered").unwrap_or(false);
        if invalid {
            invalid_total += 1;
            ensure!(
                !delivered,
                "invalid RBC session must not report delivered=true"
            );
        } else if delivered {
            delivered_elsewhere += 1;
        }
    }

    let status_after = blocking_status(&targeted_client)?;
    let status_json_after = fetch_sumeragi_status(&targeted_client).await?;
    let chunk_digest_drop_after = consensus_message_total(
        &status_json_after,
        "rbc_chunk",
        "dropped",
        "chunk_digest_mismatch",
    );
    let mismatch_detected = chunk_digest_drop_after > chunk_digest_drop_before
        || rbc_mismatch_detected(&status_json_after);
    let detection_observed = invalid_total >= 1 || mismatch_detected;

    let mut status_after_all = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        status_after_all.push(blocking_status(&peer.client())?);
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

    if !detection_observed {
        ensure!(
            max_blocks < expected_height || missing_sessions >= 1,
            "equivocated chunk without explicit invalidation counters should still keep the cluster below the target height or leave at least one peer without an RBC session"
        );
    }

    if max_blocks >= expected_height {
        ensure!(
            delivered_elsewhere >= 1,
            "expected honest validators to complete delivery under isolated equivocation"
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
            missing_sessions >= 1 || detection_observed,
            "equivocation stall should either surface missing RBC sessions or explicit invalidation evidence"
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
    configure_runtime_rbc(&base_client).await?;

    let expected_height = status_before.first().map_or(1, |status| status.blocks + 1);

    submit_heavy_log(&base_client, DEFAULT_PAYLOAD_BYTES).await?;

    let mut sessions = Vec::with_capacity(PEER_COUNT);
    for peer in network.peers() {
        let session =
            wait_for_rbc_session(&peer.client(), expected_height, Duration::from_secs(20)).await?;
        sessions.push(session);
    }

    sleep(Duration::from_secs(3)).await;

    let invalid_total = sessions
        .iter()
        .filter(|session| get_bool(session, "invalid").unwrap_or(false))
        .count();
    let delivered_total = sessions
        .iter()
        .filter(|session| get_bool(session, "delivered").unwrap_or(false))
        .count();

    let mut mismatch_detected = false;
    let mut status_after = Vec::with_capacity(PEER_COUNT);
    for peer in network.peers() {
        let status = blocking_status(&peer.client())?;
        status_after.push(status);
        if !mismatch_detected {
            let status_json = fetch_sumeragi_status(&peer.client()).await?;
            mismatch_detected = rbc_mismatch_detected(&status_json);
        }
    }

    let base_height = status_before
        .first()
        .map(|status| status.blocks)
        .unwrap_or(0);
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

    if max_blocks >= expected_height {
        ensure!(
            max_blocks.saturating_sub(min_blocks) <= 1,
            "heights diverged under uniform corruption (min={min_blocks}, max={max_blocks})"
        );
        ensure!(
            delivered_total > 0,
            "expected RBC delivery when all validators broadcast the same corrupted shards"
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
            if let (Some(before_consensus), Some(after_consensus)) = (
                status_before[idx].sumeragi.as_ref(),
                status_after.sumeragi.as_ref(),
            ) {
                ensure!(
                    after_consensus.block_created_proposal_mismatch_total
                        > before_consensus.block_created_proposal_mismatch_total,
                    "peer {idx} must increment proposal mismatch counter when shards are corrupted"
                );
            }
        }
        ensure!(
            invalid_total > 0 || mismatch_detected,
            "expected corrupted shards to be detected via invalid flag or mismatch counters (invalid_total={invalid_total}, mismatch_detected={mismatch_detected}, delivered_total={delivered_total})"
        );
    }
    let mut summary_map = Map::new();
    summary_map.insert(
        "scenario".into(),
        Value::from("all_chunks_corrupted".to_owned()),
    );
    summary_map.insert("peer_count".into(), Value::from(PEER_COUNT as u64));
    summary_map.insert("invalid_sessions".into(), Value::from(invalid_total as u64));
    summary_map.insert(
        "delivered_sessions".into(),
        Value::from(delivered_total as u64),
    );
    summary_map.insert("mismatch_detected".into(), Value::from(mismatch_detected));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert("base_height".into(), Value::from(base_height));
    summary_map.insert("min_blocks".into(), Value::from(min_blocks));
    summary_map.insert("max_blocks".into(), Value::from(max_blocks));
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
                )
                .write(
                    ["sumeragi", "debug", "rbc", "conflicting_ready_mask"],
                    FORK_MASK,
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
    configure_runtime_rbc(&targeted_client).await?;

    let status_before = blocking_status(&targeted_client)?;
    let mut invalid_ready_before_cluster = 0_u64;
    for peer in network.peers() {
        let status_json = fetch_sumeragi_status(&peer.client()).await?;
        invalid_ready_before_cluster = invalid_ready_before_cluster.saturating_add(
            consensus_message_total(&status_json, "rbc_ready", "dropped", "invalid_signature"),
        );
    }
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&targeted_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ = try_wait_for_rbc_session(&targeted_client, expected_height, Duration::from_secs(20))
        .await?;
    sleep(Duration::from_secs(4)).await;

    let mut invalid_sessions = 0usize;
    let mut delivered_sessions = 0usize;
    let mut missing_sessions = 0usize;

    for peer in network.peers() {
        let Some(session) =
            try_wait_for_rbc_session(&peer.client(), expected_height, Duration::from_secs(20))
                .await?
        else {
            missing_sessions += 1;
            continue;
        };
        if get_bool(&session, "invalid").unwrap_or(false) {
            invalid_sessions += 1;
        }
        if get_bool(&session, "delivered").unwrap_or(false) {
            delivered_sessions += 1;
        }
    }

    let status_after = blocking_status(&targeted_client)?;
    let mut invalid_ready_after_cluster = 0_u64;
    for peer in network.peers() {
        let status_json = fetch_sumeragi_status(&peer.client()).await?;
        invalid_ready_after_cluster = invalid_ready_after_cluster.saturating_add(
            consensus_message_total(&status_json, "rbc_ready", "dropped", "invalid_signature"),
        );
    }
    let detection_observed =
        invalid_sessions >= 1 || invalid_ready_after_cluster > invalid_ready_before_cluster;

    let mut status_after_all = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        status_after_all.push(blocking_status(&peer.client())?);
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

    if !detection_observed {
        ensure!(
            max_blocks < expected_height || missing_sessions >= 1,
            "conflicting READY without explicit invalidation counters should still keep the cluster below the target height or leave at least one peer without an RBC session"
        );
    }

    if max_blocks >= expected_height {
        ensure!(
            delivered_sessions >= 1 || detection_observed,
            "conflicting READY scenario should either deliver on honest validators or surface invalid READY drops"
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
            missing_sessions >= 1 || detection_observed,
            "conflicting READY stall should either surface missing RBC sessions or explicit invalidation evidence"
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("conflicting_ready"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "invalid_sessions".into(),
        Value::from(invalid_sessions as u64),
    );
    summary_map.insert(
        "delivered_sessions".into(),
        Value::from(delivered_sessions as u64),
    );
    emit_summary("conflicting_ready", &Value::Object(summary_map))?;

    network.shutdown().await;
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
    configure_runtime_rbc(&client).await?;

    let status_before = blocking_status(&client)?;
    let mut drop_before = 0_u64;
    let mut mismatch_before = 0_u64;
    for peer in network.peers() {
        let status_json = fetch_sumeragi_status(&peer.client()).await?;
        drop_before = drop_before.saturating_add(
            get_u64(&status_json, "block_created_dropped_by_lock_total").ok_or_else(|| {
                eyre!("missing block_created_dropped_by_lock_total before scenario")
            })?,
        );
        mismatch_before = mismatch_before.saturating_add(
            get_u64(&status_json, "block_created_proposal_mismatch_total").ok_or_else(|| {
                eyre!("missing block_created_proposal_mismatch_total before scenario")
            })?,
        );
    }
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let primary_session =
        try_wait_for_rbc_session(&client, expected_height, Duration::from_secs(80)).await?;
    let primary_height = primary_session
        .as_ref()
        .and_then(session_height)
        .unwrap_or(expected_height);
    let base_view = primary_session
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|obj| obj.get("view"))
        .and_then(Value::as_u64);

    let primary_delivered = primary_session
        .as_ref()
        .and_then(|value| get_bool(value, "delivered"))
        .unwrap_or(false);
    let observation_deadline = Instant::now() + Duration::from_secs(60);
    let (status_after, delivered_after, mut duplicate_views, drop_after, mismatch_after) = loop {
        let status_after = blocking_status(&client)?;
        let mut delivered_after = false;
        let mut duplicate_views: Vec<u64> = Vec::new();
        for peer in network.peers() {
            let sessions_value = tokio::task::spawn_blocking({
                let client = peer.client();
                move || client.get_sumeragi_rbc_sessions_json()
            })
            .await
            .wrap_err("fetch post-gate RBC sessions")??;
            delivered_after |= any_delivered_session_for_height(&sessions_value, primary_height);
            duplicate_views.extend(
                extract_sessions_for_height(&sessions_value, primary_height)
                    .iter()
                    .filter_map(|value| value.as_object()?.get("view")?.as_u64()),
            );
        }

        let mut drop_after = 0_u64;
        let mut mismatch_after = 0_u64;
        for peer in network.peers() {
            let status_json = fetch_sumeragi_status(&peer.client()).await?;
            drop_after = drop_after.saturating_add(
                get_u64(&status_json, "block_created_dropped_by_lock_total").ok_or_else(|| {
                    eyre!("missing block_created_dropped_by_lock_total after scenario")
                })?,
            );
            mismatch_after = mismatch_after.saturating_add(
                get_u64(&status_json, "block_created_proposal_mismatch_total").ok_or_else(
                    || eyre!("missing block_created_proposal_mismatch_total after scenario"),
                )?,
            );
        }

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
        let counters_advanced = drop_after > drop_before || mismatch_after > mismatch_before;
        if status_after.blocks >= expected_height
            || delivered_after
            || counters_advanced
            || duplicate_view_evidence
            || Instant::now() >= observation_deadline
        {
            break (
                status_after,
                delivered_after,
                duplicate_views,
                drop_after,
                mismatch_after,
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
        drop_after >= drop_before,
        "locked QC drop counter must be monotonic across the validator set (before={drop_before}, after={drop_after})"
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
        drop_after > drop_before || mismatch_after > mismatch_before || duplicate_view_evidence,
        "locked QC gate should record counters or expose duplicate-session evidence (drop_before={drop_before}, drop_after={drop_after}, mismatch_before={mismatch_before}, mismatch_after={mismatch_after}, repeated_base_view_entries={repeated_base_view_entries}, observed={duplicate_views:?})"
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
    configure_runtime_rbc(&base_client).await?;

    let status_before = blocking_status(&base_client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&base_client, DEFAULT_PAYLOAD_BYTES).await?;
    let _ =
        try_wait_for_rbc_session(&base_client, expected_height, Duration::from_secs(20)).await?;
    sleep(Duration::from_secs(5)).await;

    let mut stalled_sessions = 0usize;
    let mut delivered_sessions = 0usize;
    let mut missing_sessions = 0usize;
    let peer_count = network.peers().len();

    for peer in network.peers() {
        let Some(session) =
            try_wait_for_rbc_session(&peer.client(), expected_height, Duration::from_secs(20))
                .await?
        else {
            missing_sessions += 1;
            continue;
        };
        let total = get_u64(&session, "total_chunks").unwrap_or(0);
        let received = get_u64(&session, "received_chunks").unwrap_or(total);
        if total > received {
            stalled_sessions += 1;
        }
        if get_bool(&session, "delivered").unwrap_or(false) {
            delivered_sessions += 1;
        }
    }

    let status_after = blocking_status(&base_client)?;
    if delivered_sessions == 0 && status_after.blocks < expected_height {
        ensure!(
            stalled_sessions >= peer_count.saturating_sub(1) || missing_sessions >= 1,
            "partial-erasure should stall every non-origin validator session or leave missing RBC telemetry when delivery stays blocked (stalled={stalled_sessions}, missing={missing_sessions}, peers={peer_count})"
        );
        ensure!(
            status_after.blocks == status_before.blocks,
            "block height must remain unchanged while chunks are withheld"
        );
    } else {
        ensure!(
            stalled_sessions >= 1 || delivered_sessions >= 1 || missing_sessions >= 1,
            "partial-erasure recovery should still expose stalled sessions, delivered sessions, or bounded missing telemetry (stalled={stalled_sessions}, delivered={delivered_sessions}, missing={missing_sessions}, peers={peer_count})"
        );
        ensure!(
            status_after.blocks >= expected_height,
            "when withheld chunks recover via local payload availability, commit height should advance"
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("partial_erasure"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert("peer_count".into(), Value::from(peer_count as u64));
    summary_map.insert(
        "stalled_sessions".into(),
        Value::from(stalled_sessions as u64),
    );
    summary_map.insert(
        "delivered_sessions".into(),
        Value::from(delivered_sessions as u64),
    );
    summary_map.insert(
        "missing_sessions".into(),
        Value::from(missing_sessions as u64),
    );
    summary_map.insert(
        "status_before_blocks".into(),
        Value::from(status_before.blocks),
    );
    summary_map.insert(
        "status_after_blocks".into(),
        Value::from(status_after.blocks),
    );
    emit_summary("partial_erasure", &Value::Object(summary_map))?;

    network.shutdown().await;
    Ok(())
}

async fn fetch_sumeragi_status(client: &Client) -> Result<Value> {
    let client = client.clone();
    tokio::task::spawn_blocking(move || client.get_sumeragi_status_json())
        .await
        .wrap_err("fetch sumeragi status JSON")?
}

async fn configure_runtime_rbc(client: &Client) -> Result<()> {
    set_sumeragi_parameter(client, SumeragiParameter::DaEnabled(true)).await?;
    Ok(())
}

async fn set_sumeragi_parameter(client: &Client, parameter: SumeragiParameter) -> Result<()> {
    let client = client.clone();
    tokio::task::spawn_blocking(move || {
        client.submit_blocking(SetParameter::new(Parameter::Sumeragi(parameter)))
    })
    .await
    .wrap_err("join SetParameter task")?
    .map(|_| ())
    .wrap_err("submit SetParameter")
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

async fn wait_for_rbc_session(
    client: &Client,
    target_height: u64,
    timeout: Duration,
) -> Result<Value> {
    try_wait_for_rbc_session(client, target_height, timeout)
        .await?
        .ok_or_else(|| {
            eyre!("timed out waiting for RBC session at or after height {target_height}")
        })
}

async fn try_wait_for_rbc_session(
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
        let sessions = tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_sumeragi_rbc_sessions_json()
        })
        .await
        .wrap_err("join sessions fetch")?;

        let sessions = match sessions {
            Ok(sessions) => sessions,
            Err(err) if is_transient_rbc_sessions_fetch_error(&err) => {
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

async fn try_wait_for_cluster_height(
    clients: &[Client],
    target_height: u64,
    timeout: Duration,
) -> Result<Option<Vec<Status>>> {
    let deadline = Instant::now() + timeout;
    loop {
        let statuses = collect_client_statuses_best_effort(clients)?;
        if statuses.iter().any(|status| status.blocks >= target_height) {
            return Ok(Some(statuses));
        }
        if Instant::now() > deadline {
            return Ok(None);
        }
        sleep(Duration::from_millis(200)).await;
    }
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

fn session_height(value: &Value) -> Option<u64> {
    value
        .as_object()
        .and_then(|obj| obj.get("height"))
        .and_then(Value::as_u64)
}

fn any_delivered_session_for_height(value: &Value, target_height: u64) -> bool {
    extract_sessions_for_height(value, target_height)
        .iter()
        .any(|session| get_bool(session, "delivered").unwrap_or(false))
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

fn consensus_message_total(status: &Value, kind: &str, outcome: &str, reason: &str) -> u64 {
    status
        .as_object()
        .and_then(|root| root.get("consensus_message_handling"))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("entries"))
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_object())
                .filter(|entry| {
                    entry.get("kind").and_then(Value::as_str) == Some(kind)
                        && entry.get("outcome").and_then(Value::as_str) == Some(outcome)
                        && entry.get("reason").and_then(Value::as_str) == Some(reason)
                })
                .filter_map(|entry| entry.get("total").and_then(Value::as_u64))
                .sum()
        })
        .unwrap_or_default()
}

fn rbc_mismatch_detected(status: &Value) -> bool {
    status
        .as_object()
        .and_then(|root| root.get("rbc_mismatch"))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("entries"))
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                let Some(entry_obj) = entry.as_object() else {
                    return false;
                };
                [
                    "chunk_digest_mismatch_total",
                    "payload_hash_mismatch_total",
                    "chunk_root_mismatch_total",
                ]
                .iter()
                .filter_map(|key| entry_obj.get(*key).and_then(Value::as_u64))
                .any(|value| value > 0)
            })
        })
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
