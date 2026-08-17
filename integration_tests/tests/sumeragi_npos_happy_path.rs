#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Happy-path NPoS coverage for canonical v2 data availability and metrics.
use eyre::{WrapErr, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::data_model::{
    Level,
    isi::{Log, SetParameter},
    parameter::{Parameter, TransactionParameter},
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use std::{num::NonZeroU64, time::Duration};
use tokio::time::sleep;
const BLOCK_TARGET: u64 = 6;
const METRIC_ATTEMPTS: usize = 40;
const METRIC_INTERVAL: Duration = Duration::from_millis(250);
const PACEMAKER_EMA_BUDGET_MS: f64 = 8_000.0;
const BG_QUEUE_DEPTH_BUDGET: f64 = 16.0;
const LARGE_PAYLOAD_BYTES: usize = 1024 * 1024;
const COMMIT_WAIT_BUDGET: Duration = Duration::from_secs(480);
const NETWORK_FRAME_BUDGET_BYTES: i64 = 128 * 1024 * 1024;
const TORII_CONTENT_HEADROOM_BYTES: usize = 2 * 1024 * 1024;
fn torii_max_content_len_for_payload(payload_bytes: usize) -> i64 {
    let inflated = payload_bytes.saturating_mul(12);
    let with_headroom = payload_bytes.saturating_add(TORII_CONTENT_HEADROOM_BYTES);
    i64::try_from(inflated.max(with_headroom)).unwrap_or(i64::MAX)
}
fn tx_limit_for_payload(payload_bytes: usize) -> NonZeroU64 {
    NonZeroU64::new(
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX),
    )
    .expect("payload-driven transaction limit must be non-zero")
}
fn npos_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["network", "max_frame_bytes"], NETWORK_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    NETWORK_FRAME_BUDGET_BYTES,
                );
        })
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_happy_path_enforces_da_and_metrics_bounds() -> eyre::Result<()> {
    init_instruction_registry();
    let Some(network) = sandbox::start_network_async_or_skip(
        npos_builder(),
        stringify!(npos_happy_path_enforces_da_and_metrics_bounds),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..BLOCK_TARGET {
        client.submit_blocking(
            Log::new(Level::INFO, format!("npos happy seed {idx}")),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    network
        .ensure_blocks_with(|height| height.total >= BLOCK_TARGET)
        .await?;
    let status = client.get_status()?;
    ensure!(
        status.blocks >= BLOCK_TARGET,
        "expected at least {BLOCK_TARGET} blocks, observed {}",
        status.blocks
    );
    let v2 = client.get_sumeragi_status()?;
    v2.validate()
        .map_err(|err| eyre!("invalid canonical v2 status: {err}"))?;
    ensure!(
        v2.last_committed_height >= BLOCK_TARGET && v2.last_committed_subject.is_some(),
        "NPoS happy path must expose the committed v2 subject at or above height {BLOCK_TARGET}"
    );
    let diagnostics = client.get_sumeragi_diagnostics()?;
    ensure!(
        diagnostics.npos.is_some(),
        "NPoS happy path must expose NPoS diagnostics"
    );
    let http = integration_tests::http::client();
    let torii = client.torii_url.clone();
    let metrics_url = torii.join("metrics").wrap_err("compose metrics URL")?;
    ensure_metrics_within_bounds(
        &http,
        &metrics_url,
        PACEMAKER_EMA_BUDGET_MS,
        BG_QUEUE_DEPTH_BUDGET,
    )
    .await?;
    network.shutdown().await;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_large_da_payload_commits_with_consistent_v2_subject() -> eyre::Result<()> {
    init_instruction_registry();
    let tx_limit = tx_limit_for_payload(LARGE_PAYLOAD_BYTES);
    let builder = npos_builder()
        .with_config_layer(|layer| {
            layer.write(
                ["torii", "max_content_len"],
                torii_max_content_len_for_payload(LARGE_PAYLOAD_BYTES),
            );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit),
        )));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_large_da_payload_commits_with_consistent_v2_subject),
    )
    .await?
    else {
        return Ok(());
    };
    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;
    let client = network.client();
    let expected_height = client.get_status()?.blocks.saturating_add(1);
    let payload = "N".repeat(LARGE_PAYLOAD_BYTES);
    let submit_client = client.clone();
    tokio::task::spawn_blocking(move || {
        submit_client.submit(
            Log::new(Level::INFO, payload),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("join large NPoS DA submission")??;
    tokio::time::timeout(
        COMMIT_WAIT_BUDGET,
        network.ensure_blocks_with(|height| height.total >= expected_height),
    )
    .await
    .wrap_err("large NPoS DA payload did not commit within the budget")??;
    let required = network.peers().len().saturating_sub(1).max(1);
    let mut committed_subjects = Vec::new();
    for peer in network.peers() {
        let peer_client = peer.client();
        let status = peer_client.get_sumeragi_status()?;
        status
            .validate()
            .map_err(|err| eyre!("invalid canonical v2 status: {err}"))?;
        let diagnostics = peer_client.get_sumeragi_diagnostics()?;
        if diagnostics.npos.is_some()
            && status.last_committed_height >= expected_height
            && status.last_committed_subject.is_some()
        {
            committed_subjects.push(status.last_committed_subject);
        }
    }
    ensure!(
        committed_subjects.len() >= required,
        "expected a canonical v2 commit subject on {required} peers, observed {}",
        committed_subjects.len()
    );
    let expected_subject = committed_subjects[0];
    ensure!(
        committed_subjects
            .iter()
            .all(|subject| *subject == expected_subject),
        "quorum peers must agree on the NPoS DA subject: {committed_subjects:?}"
    );
    network.shutdown().await;
    Ok(())
}
async fn ensure_metrics_within_bounds(
    http: &reqwest::Client,
    url: &reqwest::Url,
    phase_budget_ms: f64,
    queue_budget: f64,
) -> eyre::Result<()> {
    let mut last_snapshot = String::new();
    let mut last_summary = String::new();
    for attempt in 0..METRIC_ATTEMPTS {
        let response = http
            .get(url.clone())
            .header("Accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        ensure!(
            response.status().is_success(),
            "metrics endpoint returned status {}",
            response.status()
        );
        let snapshot = response.text().await.wrap_err("read metrics body")?;
        let reader = MetricsReader::new(&snapshot);
        let queue_depth = reader.get("sumeragi_bg_post_queue_depth");
        let queue_depth_max = reader
            .max_with_prefix("sumeragi_bg_post_queue_depth_by_peer")
            .ok_or_else(|| eyre!("missing per-peer background queue depth metrics"))?;
        let phases = [
            "propose",
            "collect_da",
            "collect_prevote",
            "collect_precommit",
            "commit",
        ];
        let phase_values = phases.map(|phase| {
            let key = format!("sumeragi_phase_latency_ema_ms{{phase=\"{phase}\"}}");
            (phase, reader.get(&key))
        });
        let phases_ok = phase_values
            .iter()
            .all(|(_, value)| *value > 0.0 && *value <= phase_budget_ms);
        last_summary = format!(
            "queue_depth={queue_depth}, queue_depth_max={queue_depth_max}, phases={phase_values:?}"
        );
        last_snapshot = snapshot;
        if phases_ok && queue_depth <= queue_budget && queue_depth_max <= queue_budget {
            return Ok(());
        }
        if attempt + 1 < METRIC_ATTEMPTS {
            sleep(METRIC_INTERVAL).await;
        }
    }
    Err(eyre!(
        "telemetry thresholds not satisfied after {METRIC_ATTEMPTS} polls; last summary: {last_summary}\nmetrics snapshot:\n{last_snapshot}"
    ))
}
#[test]
fn payload_limits_include_transport_headroom() {
    let content_limit = torii_max_content_len_for_payload(LARGE_PAYLOAD_BYTES);
    assert!(content_limit > i64::try_from(LARGE_PAYLOAD_BYTES).expect("fixture fits i64"));
    assert_eq!(
        tx_limit_for_payload(LARGE_PAYLOAD_BYTES).get(),
        u64::try_from(content_limit).expect("positive content limit")
    );
}
