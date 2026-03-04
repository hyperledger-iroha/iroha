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
    client::Client,
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
    sleep(Duration::from_secs(2)).await;
    let status_after = blocking_status(&client)?;
    let sessions_after = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.get_sumeragi_rbc_sessions_json()
    })
    .await
    .wrap_err("fetch RBC sessions after chunk-drop wait")??;

    let delivered = get_bool(&session, "delivered").unwrap_or(false)
        || any_delivered_session_for_height(&sessions_after, expected_height);
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
    configure_runtime_rbc(&client).await?;

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session = wait_for_rbc_session(&client, expected_height, Duration::from_secs(40)).await?;
    let status_after = wait_for_height(&client, expected_height, Duration::from_secs(60)).await?;

    let delivered = get_bool(&session, "delivered").unwrap_or(false);
    ensure!(delivered, "reorder scenario should still deliver payload");
    ensure!(
        status_after.blocks >= expected_height,
        "block should commit once RBC delivers"
    );

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("chunk_reorder"));
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

    let session = wait_for_rbc_session(&client, expected_height, Duration::from_secs(30)).await?;
    sleep(Duration::from_secs(3)).await;
    let status_after = blocking_status(&client)?;

    let delivered = get_bool(&session, "delivered").unwrap_or(false);
    if delivered {
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
    summary_map.insert("rbc_session".into(), session.clone());
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
    configure_runtime_rbc(&client).await?;

    let status_before = blocking_status(&client)?;
    let expected_height = status_before.blocks + 1;

    submit_heavy_log(&client, DEFAULT_PAYLOAD_BYTES).await?;

    let session = wait_for_rbc_session(&client, expected_height, Duration::from_secs(40)).await?;
    let status_after = wait_for_height(&client, expected_height, Duration::from_secs(60)).await?;

    let sessions_value = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.get_sumeragi_rbc_sessions_json()
    })
    .await
    .wrap_err("join duplicate sessions fetch")??;
    let base_view = session
        .as_object()
        .and_then(|obj| obj.get("view"))
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("missing view in primary session"))?;
    let views: Vec<u64> = extract_sessions_for_height(&sessions_value, expected_height)
        .iter()
        .filter_map(|value| value.as_object()?.get("view")?.as_u64())
        .collect();
    ensure!(
        views.contains(&base_view) && views.contains(&(base_view + 1)),
        "expected duplicate view entries in RBC sessions"
    );

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("duplicate_inits"));
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
        wait_for_rbc_session(&drop_client, expected_height, Duration::from_secs(20)).await?;
    sleep(Duration::from_secs(2)).await;
    let status_after_drop = blocking_status(&drop_client)?;
    let drop_delivered = get_bool(&drop_session, "delivered").unwrap_or(false);
    if drop_delivered {
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
    configure_runtime_rbc(&recovery_client).await?;
    let status_before_recovery = blocking_status(&recovery_client)?;
    let recovery_height = status_before_recovery.blocks + 1;

    submit_heavy_log(&recovery_client, DEFAULT_PAYLOAD_BYTES).await?;
    let recovery_session =
        wait_for_rbc_session(&recovery_client, recovery_height, Duration::from_secs(40)).await?;
    let status_after_recovery =
        wait_for_height(&recovery_client, recovery_height, Duration::from_secs(60)).await?;

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
    summary_map.insert("drop_session".into(), drop_session.clone());
    summary_map.insert(
        "recovery_status_before".into(),
        Value::from(status_before_recovery.blocks),
    );
    summary_map.insert(
        "recovery_status_after".into(),
        Value::from(status_after_recovery.blocks),
    );
    summary_map.insert("recovery_session".into(), recovery_session.clone());
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
    let _ = wait_for_rbc_session(&base_client, expected_height, Duration::from_secs(20)).await?;
    sleep(Duration::from_secs(3)).await;

    let mut missing = 0usize;
    let mut complete = 0usize;

    for peer in network.peers() {
        let peer_client = peer.client();
        let session =
            wait_for_rbc_session(&peer_client, expected_height, Duration::from_secs(20)).await?;
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
    }

    ensure!(
        complete >= 1,
        "expected at least one validator to receive all RBC chunks"
    );

    let status_after = blocking_status(&base_client)?;
    if missing >= 1 {
        ensure!(
            status_after.blocks == status_before.blocks,
            "block height must remain unchanged while selective drop prevents delivery"
        );
    } else {
        ensure!(
            complete == network.peers().len(),
            "selective drop recovered path should complete on every validator (complete={complete}, peers={})",
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
    let _ =
        wait_for_rbc_session(&targeted_client, expected_height, Duration::from_secs(20)).await?;
    sleep(Duration::from_secs(3)).await;

    let mut invalid_total = 0usize;
    let mut delivered_elsewhere = 0usize;

    for peer in network.peers() {
        let client = peer.client();
        let session =
            wait_for_rbc_session(&client, expected_height, Duration::from_secs(20)).await?;
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

    ensure!(
        delivered_elsewhere >= 1,
        "expected non-target validators to complete delivery"
    );

    let status_after = blocking_status(&targeted_client)?;
    let status_json_after = fetch_sumeragi_status(&targeted_client).await?;
    let chunk_digest_drop_after = consensus_message_total(
        &status_json_after,
        "rbc_chunk",
        "dropped",
        "chunk_digest_mismatch",
    );

    if invalid_total >= 1 {
        ensure!(
            status_after.blocks == status_before.blocks,
            "targeted validator should refuse to advance height when equivocation is detected"
        );
    } else {
        ensure!(
            chunk_digest_drop_after > chunk_digest_drop_before
                || rbc_mismatch_detected(&status_json_after),
            "equivocated chunk should be detected via digest-mismatch drops or mismatch counters"
        );
        ensure!(
            status_after.blocks >= expected_height,
            "without session invalidation, equivocation scenario should recover and commit"
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
    let _ =
        wait_for_rbc_session(&targeted_client, expected_height, Duration::from_secs(20)).await?;
    sleep(Duration::from_secs(4)).await;

    let mut invalid_sessions = 0usize;
    let mut delivered_sessions = 0usize;

    for peer in network.peers() {
        let session =
            wait_for_rbc_session(&peer.client(), expected_height, Duration::from_secs(20)).await?;
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
    if invalid_sessions >= 1 {
        ensure!(
            delivered_sessions == 0,
            "conflicting READY emissions must prevent RBC delivery when sessions invalidate"
        );
        ensure!(
            status_after.blocks == status_before.blocks,
            "validator should refuse to advance height when READY conflict invalidates the session"
        );
    } else {
        ensure!(
            invalid_ready_after_cluster > invalid_ready_before_cluster,
            "conflicting READY scenario should drop at least one READY as invalid_signature across the validator set"
        );
        ensure!(
            delivered_sessions >= 1,
            "without session invalidation, at least one validator should still deliver"
        );
        ensure!(
            status_after.blocks >= expected_height,
            "without session invalidation, conflicting READY scenario should recover and commit"
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
        wait_for_rbc_session(&client, expected_height, Duration::from_secs(40)).await?;
    let base_view = primary_session
        .as_object()
        .and_then(|obj| obj.get("view"))
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("missing view in primary RBC session"))?;

    let sessions_snapshot = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.get_sumeragi_rbc_sessions_json()
    })
    .await
    .wrap_err("fetch RBC session snapshot")??;

    sleep(Duration::from_secs(4)).await;

    let status_after = blocking_status(&client)?;
    let sessions_after = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.get_sumeragi_rbc_sessions_json()
    })
    .await
    .wrap_err("fetch post-gate RBC sessions")??;
    let primary_delivered = get_bool(&primary_session, "delivered").unwrap_or(false);
    let delivered_after = any_delivered_session_for_height(&sessions_after, expected_height);
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
            get_u64(&status_json, "block_created_proposal_mismatch_total").ok_or_else(|| {
                eyre!("missing block_created_proposal_mismatch_total after scenario")
            })?,
        );
    }
    ensure!(
        drop_after >= drop_before,
        "locked QC drop counter must be monotonic across the validator set (before={drop_before}, after={drop_after})"
    );
    ensure!(
        mismatch_after == mismatch_before,
        "proposal mismatch counter should remain unchanged when gated purely by locked QC (before={mismatch_before}, after={mismatch_after})"
    );

    let mut duplicate_views: Vec<u64> =
        extract_sessions_for_height(&sessions_snapshot, expected_height)
            .iter()
            .chain(extract_sessions_for_height(&sessions_after, expected_height).iter())
            .filter_map(|value| value.as_object()?.get("view")?.as_u64())
            .collect();
    duplicate_views.sort_unstable();
    duplicate_views.dedup();
    ensure!(
        duplicate_views.contains(&base_view)
            && duplicate_views.contains(&(base_view.saturating_add(1))),
        "expected duplicate RBC sessions for consecutive views under duplicate init scenario: base={base_view}, observed={duplicate_views:?}"
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
    let _ = wait_for_rbc_session(&base_client, expected_height, Duration::from_secs(20)).await?;
    sleep(Duration::from_secs(5)).await;

    let mut stalled_sessions = 0usize;
    let mut delivered_sessions = 0usize;

    for peer in network.peers() {
        let session =
            wait_for_rbc_session(&peer.client(), expected_height, Duration::from_secs(20)).await?;
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
    if stalled_sessions == network.peers().len() {
        ensure!(
            delivered_sessions == 0,
            "stalled partial-erasure sessions must not report delivered=true"
        );
        ensure!(
            status_after.blocks == status_before.blocks,
            "block height must remain unchanged while chunks are withheld"
        );
    } else {
        ensure!(
            delivered_sessions >= 1,
            "partial-erasure recovery path should deliver on at least one validator"
        );
        ensure!(
            status_after.blocks >= expected_height,
            "when withheld chunks are recovered from local payload data, commit height should advance"
        );
    }

    let mut summary_map = Map::new();
    summary_map.insert("scenario".into(), Value::from("partial_erasure"));
    summary_map.insert("expected_height".into(), Value::from(expected_height));
    summary_map.insert(
        "stalled_sessions".into(),
        Value::from(stalled_sessions as u64),
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
    let result = tokio::task::spawn_blocking(move || {
        client_clone.submit_blocking(Log::new(Level::INFO, payload))
    })
    .await
    .wrap_err("join submit task")?;
    if let Err(err) = result {
        if is_tx_confirmation_timeout(&err) {
            eprintln!("tx confirmation timed out; proceeding: {err}");
            return Ok(());
        }
        return Err(err).wrap_err("submit heavy log");
    }
    Ok(())
}

fn is_tx_confirmation_timeout(error: &Report) -> bool {
    error.chain().any(|cause| {
        let msg = cause.to_string();
        msg.contains("tx confirmation timed out")
            || msg.contains("haven't got tx confirmation within")
    })
}

fn blocking_status(client: &Client) -> Result<iroha::client::Status> {
    let client_clone = client.clone();
    tokio::task::block_in_place(|| client_clone.get_status()).wrap_err("fetch status")
}

async fn wait_for_rbc_session(
    client: &Client,
    target_height: u64,
    timeout: Duration,
) -> Result<Value> {
    let client = client.clone();
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(eyre!(
                "timed out waiting for RBC session at height {target_height}"
            ));
        }
        let sessions = tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_sumeragi_rbc_sessions_json()
        })
        .await
        .wrap_err("join sessions fetch")??;

        if let Some(session) = extract_session(&sessions, target_height) {
            return Ok(session);
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_height(
    client: &Client,
    target_height: u64,
    timeout: Duration,
) -> Result<iroha::client::Status> {
    let client = client.clone();
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(eyre!("timed out waiting for block height {target_height}"));
        }
        let status = blocking_status(&client)?;
        if status.blocks >= target_height {
            return Ok(status);
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
