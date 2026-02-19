#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Happy-path `NPoS` consensus scenario with DA availability tracking and telemetry guardrails.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use eyre::{WrapErr, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::data_model::{
    Level,
    isi::{Log, SetParameter},
    parameter::{Parameter, SumeragiParameter},
};
use iroha_core::sumeragi::rbc_status;
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use norito::json::{self, Map, Value};
use tokio::time::{sleep, timeout};
use toml::Table;

const BLOCK_TARGET: u64 = 6;
const METRIC_ATTEMPTS: usize = 20;
const METRIC_INTERVAL: Duration = Duration::from_millis(250);
const PACEMAKER_EMA_BUDGET_MS: f64 = 5_000.0;
const BG_QUEUE_DEPTH_BUDGET: f64 = 16.0;
const RBC_DELIVER_MIN: f64 = 1.0;
const RBC_WAIT_BUDGET: Duration = Duration::from_secs(20);
const RBC_DELIVERY_BUDGET: Duration = Duration::from_secs(120);
const COMMIT_WAIT_BUDGET: Duration = Duration::from_secs(120);
const LARGE_PAYLOAD_BYTES: usize = 6 * 1024 * 1024;

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_happy_path_enforces_da_and_metrics_bounds() -> eyre::Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], 2_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64)
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    8_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(2),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(1),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_happy_path_enforces_da_and_metrics_bounds),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..BLOCK_TARGET {
        client.submit_blocking(Log::new(Level::INFO, format!("npos happy seed {idx}")))?;
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

    let http = reqwest::Client::new();
    let torii = client.torii_url.clone();
    let collectors_url = torii
        .join("v1/sumeragi/collectors")
        .wrap_err("compose collectors URL")?;
    let sessions_url = torii
        .join("v1/sumeragi/rbc/sessions")
        .wrap_err("compose RBC sessions URL")?;
    let metrics_url = torii.join("metrics").wrap_err("compose metrics URL")?;

    ensure_vrf_collectors(&http, &collectors_url).await?;
    wait_for_rbc_delivery(&http, &sessions_url, status.blocks, RBC_WAIT_BUDGET).await?;
    ensure_metrics_within_bounds(
        &http,
        &metrics_url,
        PACEMAKER_EMA_BUDGET_MS,
        BG_QUEUE_DEPTH_BUDGET,
        RBC_DELIVER_MIN,
    )
    .await?;

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_rbc_persists_payload_across_restart() -> eyre::Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], 2_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64)
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    8_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(2),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(1),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_rbc_persists_payload_across_restart),
    )
    .await?
    else {
        return Ok(());
    };

    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let peers = network.peers();
    let restart_peer = &peers[1];
    let config_layers: Vec<ConfigLayer> = network
        .config_layers()
        .map(|cow| ConfigLayer(cow.into_owned()))
        .collect();

    let client = network.client();
    let status = client.get_status()?;
    let expected_height = status.blocks + 1;

    let http = reqwest::Client::new();
    let start = Instant::now();

    let sessions_url_primary = client
        .torii_url
        .join("v1/sumeragi/rbc/sessions")
        .wrap_err("compose primary RBC sessions URL")?;
    let status_url_primary = client
        .torii_url
        .join("status")
        .wrap_err("compose primary status URL")?;
    let restart_sessions_url = reqwest::Url::parse(&format!(
        "{}/v1/sumeragi/rbc/sessions",
        restart_peer.torii_url()
    ))
    .wrap_err("compose restart peer sessions URL")?;

    let inflight_handle = tokio::spawn(wait_for_rbc_session_inflight(
        http.clone(),
        restart_sessions_url.clone(),
        expected_height,
        start,
        RBC_DELIVERY_BUDGET,
    ));

    let submit_payload = "R".repeat(LARGE_PAYLOAD_BYTES);
    let submit_client = client.clone();
    let submit_handle = tokio::task::spawn_blocking(move || {
        submit_client.submit(Log::new(Level::INFO, submit_payload))
    });

    let inflight_session = inflight_handle
        .await
        .wrap_err("join in-flight RBC detector")??;

    submit_handle.await.wrap_err("submit log instruction")??;

    restart_peer.shutdown().await;
    if sandbox::handle_result(
        restart_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await,
        "npos_rbc_persists_payload_across_restart_restart",
    )?
    .is_none()
    {
        return Ok(());
    }
    timeout(COMMIT_WAIT_BUDGET, restart_peer.once_block(expected_height))
        .await
        .map_err(|_| {
            eyre!(
                "restart peer failed to reach height {expected_height} within {:?}",
                COMMIT_WAIT_BUDGET
            )
        })?;

    wait_for_rbc_session_recovered(
        http.clone(),
        restart_sessions_url,
        expected_height,
        &inflight_session.block_hash,
        start,
        RBC_DELIVERY_BUDGET,
    )
    .await?;

    let _ = wait_for_rbc_session_delivered(
        &http,
        &sessions_url_primary,
        expected_height,
        start,
        RBC_DELIVERY_BUDGET,
    )
    .await?;

    let _ = wait_for_block_height(
        &http,
        &status_url_primary,
        expected_height,
        start,
        COMMIT_WAIT_BUDGET,
    )
    .await?;

    let restart_store_dir = restart_peer.kura_store_dir().join("rbc_sessions");
    ensure_rbc_sessions_persisted(
        &network,
        expected_height,
        start,
        COMMIT_WAIT_BUDGET + RBC_DELIVERY_BUDGET,
        Some(restart_store_dir.as_path()),
    )
    .await?;

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_rbc_large_payload_delivers_and_commits() -> eyre::Result<()> {
    init_instruction_registry();

    let builder = large_payload_npos_builder();

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_rbc_large_payload_delivers_and_commits),
    )
    .await?
    else {
        return Ok(());
    };

    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let client = network.client();
    let status = client.get_status()?;
    let expected_height = status.blocks + 1;

    let http = reqwest::Client::new();
    let sessions_url = client
        .torii_url
        .join("v1/sumeragi/rbc/sessions")
        .wrap_err("compose RBC sessions URL")?;
    let status_url = client
        .torii_url
        .join("status")
        .wrap_err("compose status URL")?;

    let payload = "N".repeat(LARGE_PAYLOAD_BYTES);
    let submit_client = client.clone();
    let start = Instant::now();

    let submit_handle =
        tokio::task::spawn_blocking(move || submit_client.submit(Log::new(Level::INFO, payload)));

    let (_delivered_at, session) = wait_for_rbc_session_delivered(
        &http,
        &sessions_url,
        expected_height,
        start,
        RBC_DELIVERY_BUDGET,
    )
    .await?;

    submit_handle.await.wrap_err("submit log instruction")??;

    let _commit_at = wait_for_block_height(
        &http,
        &status_url,
        expected_height,
        start,
        COMMIT_WAIT_BUDGET,
    )
    .await?;

    ensure_rbc_sessions_persisted(&network, expected_height, start, COMMIT_WAIT_BUDGET, None)
        .await?;

    ensure!(
        session.received_chunks == session.total_chunks,
        "delivered session should report all chunks received: {:?}",
        session
    );

    network.shutdown().await;
    Ok(())
}

fn large_payload_npos_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], 2_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64)
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    8_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(2),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(1),
        )))
}

async fn ensure_rbc_sessions_persisted(
    network: &Network,
    expected_height: u64,
    start: Instant,
    budget: Duration,
    skip_store_dir: Option<&Path>,
) -> eyre::Result<()> {
    let deadline = start + budget;
    loop {
        let mut missing = Vec::new();
        for peer in network.peers() {
            let store_dir = peer.kura_store_dir().join("rbc_sessions");
            if skip_store_dir.is_some_and(|skip| store_dir == skip) {
                continue;
            }
            let snapshot = rbc_status::read_persisted_snapshot(&store_dir);
            let found = snapshot.iter().any(|summary| {
                summary.height == expected_height
                    && summary.delivered
                    && summary.received_chunks == summary.total_chunks
                    && summary.total_chunks > 0
                    && !summary.invalid
            });
            if !found {
                missing.push(store_dir.display().to_string());
            }
        }

        if missing.is_empty() {
            return Ok(());
        }

        ensure!(
            Instant::now() <= deadline,
            "expected delivered session persisted in {}",
            missing.join(", ")
        );
        sleep(Duration::from_millis(200)).await;
    }
}

async fn ensure_vrf_collectors(http: &reqwest::Client, url: &reqwest::Url) -> eyre::Result<()> {
    let response = http
        .get(url.clone())
        .header("Accept", "application/json")
        .send()
        .await
        .wrap_err("fetch collectors snapshot")?;
    ensure!(
        response.status().is_success(),
        "collectors endpoint returned status {}",
        response.status()
    );
    let body = response.text().await.wrap_err("read collectors body")?;
    let value: Value = json::from_str(&body).wrap_err("parse collectors JSON")?;
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("collectors payload must be an object"))?;
    ensure!(
        matches!(
            root.get("consensus_mode").and_then(Value::as_str),
            Some("Npos")
        ),
        "collectors endpoint must report consensus_mode=\"Npos\""
    );
    let collectors = root
        .get("collectors")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("collectors payload missing collectors list"))?;
    ensure!(
        !collectors.is_empty(),
        "collectors list should not be empty in VRF mode"
    );
    let prf = root
        .get("prf")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("collectors payload missing prf context"))?;
    let seed_hex = prf
        .get("epoch_seed")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("collectors payload missing prf.epoch_seed"))?;
    let seed = hex::decode(seed_hex).wrap_err("decode epoch seed hex")?;
    ensure!(
        seed.len() == 32,
        "epoch seed must be 32 bytes, got {}",
        seed.len()
    );
    ensure!(
        seed.iter().any(|byte| *byte != 0),
        "epoch seed should be non-zero when VRF is active"
    );
    Ok(())
}

async fn wait_for_rbc_delivery(
    http: &reqwest::Client,
    url: &reqwest::Url,
    target_height: u64,
    budget: Duration,
) -> eyre::Result<()> {
    let deadline = Instant::now() + budget;
    loop {
        let response = http
            .get(url.clone())
            .header("accept", "application/json")
            .send()
            .await
            .wrap_err("fetch RBC sessions snapshot")?;
        if response.status().is_success() {
            let body = response.text().await.wrap_err("read RBC sessions body")?;
            let value: Value = json::from_str(&body).wrap_err("parse RBC sessions JSON")?;
            if has_delivered_session(&value, target_height)? {
                return Ok(());
            }
        }
        ensure!(
            Instant::now() <= deadline,
            "timed out waiting for delivered RBC session at height {target_height}"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

fn has_delivered_session(root: &Value, target_height: u64) -> eyre::Result<bool> {
    let Some(items) = root
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(Value::as_array)
    else {
        return Ok(false);
    };
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let height = obj
            .get("height")
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("RBC session missing height field"))?;
        if height < target_height {
            continue;
        }
        let delivered = obj
            .get("delivered")
            .and_then(Value::as_bool)
            .ok_or_else(|| eyre!("RBC session missing delivered field"))?;
        if delivered {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn ensure_metrics_within_bounds(
    http: &reqwest::Client,
    url: &reqwest::Url,
    phase_budget_ms: f64,
    queue_budget: f64,
    deliver_min: f64,
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

        let deliver_total = reader.get("sumeragi_rbc_deliver_broadcasts_total");
        let queue_depth = reader.get("sumeragi_bg_post_queue_depth");
        let queue_depth_max = reader
            .max_with_prefix("sumeragi_bg_post_queue_depth_by_peer")
            .ok_or_else(|| eyre!("missing per-peer bg queue depth metrics"))?;

        let phases = [
            "propose",
            "collect_da",
            "collect_prevote",
            "collect_precommit",
            "commit",
        ];

        let mut phases_ok = true;
        let mut phase_values = Vec::new();
        for phase in phases {
            let key = format!("sumeragi_phase_latency_ema_ms{{phase=\"{phase}\"}}");
            let value = reader.get(&key);
            phase_values.push((phase, value));
            if !(value > 0.0 && value <= phase_budget_ms) {
                phases_ok = false;
            }
        }

        last_snapshot = snapshot;
        last_summary = format!(
            "deliver_total={deliver_total}, queue_depth={queue_depth}, \
             queue_depth_max={queue_depth_max}, phases={phase_values:?}"
        );

        if phases_ok
            && deliver_total >= deliver_min
            && queue_depth <= queue_budget
            && queue_depth_max <= queue_budget
        {
            return Ok(());
        }

        if attempt + 1 == METRIC_ATTEMPTS {
            break;
        }
        sleep(METRIC_INTERVAL).await;
    }

    Err(eyre!(
        "telemetry thresholds not satisfied after {METRIC_ATTEMPTS} polls; \
         last summary: {last_summary}\nmetrics snapshot:\n{last_snapshot}"
    ))
}

async fn wait_for_rbc_session_delivered(
    http: &reqwest::Client,
    url: &reqwest::Url,
    target_height: u64,
    start: Instant,
    budget: Duration,
) -> eyre::Result<(Duration, RbcSessionView)> {
    let deadline = start + budget;
    loop {
        ensure!(
            Instant::now() <= deadline,
            "timed out waiting for delivered RBC session at height {target_height}"
        );
        let response = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch RBC sessions snapshot")?;
        if !response.status().is_success() {
            sleep(Duration::from_millis(200)).await;
            continue;
        }
        let body = response.text().await.wrap_err("read RBC sessions body")?;
        let value: Value = json::from_str(&body).wrap_err("parse RBC sessions JSON")?;
        let sessions = parse_rbc_sessions(&value)?;
        if let Some(session) = sessions
            .into_iter()
            .find(|session| session.height == target_height && session.delivered)
        {
            return Ok((start.elapsed(), session));
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_block_height(
    http: &reqwest::Client,
    url: &reqwest::Url,
    target_height: u64,
    start: Instant,
    budget: Duration,
) -> eyre::Result<Duration> {
    let deadline = start + budget;
    loop {
        ensure!(
            Instant::now() <= deadline,
            "timed out waiting for commit height {target_height}"
        );
        let response = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch /status snapshot")?;
        if response.status().is_success() {
            let body = response.text().await.wrap_err("read status body")?;
            let value: Value = json::from_str(&body).wrap_err("parse status JSON")?;
            if let Some(height) = value
                .as_object()
                .and_then(|obj| obj.get("blocks"))
                .and_then(Value::as_u64)
                && height >= target_height
            {
                return Ok(start.elapsed());
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_rbc_session_inflight(
    http: reqwest::Client,
    url: reqwest::Url,
    target_height: u64,
    start: Instant,
    budget: Duration,
) -> eyre::Result<RbcSessionView> {
    let deadline = start + budget;
    loop {
        ensure!(
            Instant::now() <= deadline,
            "timed out waiting for in-flight RBC session at height {target_height}"
        );
        let response = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch RBC sessions snapshot")?;
        if response.status().is_success() {
            let body = response.text().await.wrap_err("read RBC sessions body")?;
            let value = json::from_str(&body).wrap_err("parse RBC sessions JSON")?;
            let sessions = parse_rbc_sessions(&value)?;
            if let Some(session) = sessions.into_iter().find(|session| {
                session.height == target_height
                    && !session.delivered
                    && session.received_chunks > 0
                    && !session.invalid
            }) {
                return Ok(session);
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_rbc_session_recovered(
    http: reqwest::Client,
    url: reqwest::Url,
    target_height: u64,
    block_hash: &str,
    start: Instant,
    budget: Duration,
) -> eyre::Result<RbcSessionView> {
    let deadline = start + budget;
    loop {
        ensure!(
            Instant::now() <= deadline,
            "timed out waiting for recovered RBC session at height {target_height}"
        );
        let response = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch restart peer RBC sessions snapshot")?;
        if response.status().is_success() {
            let body = response.text().await.wrap_err("read RBC sessions body")?;
            let value = json::from_str(&body).wrap_err("parse RBC sessions JSON")?;
            let sessions = parse_rbc_sessions(&value)?;
            if let Some(session) = sessions.into_iter().find(|session| {
                session.height == target_height
                    && session.block_hash == block_hash
                    && session.recovered
                    && !session.invalid
            }) {
                return Ok(session);
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

#[derive(Clone, Debug)]
struct RbcSessionView {
    block_hash: String,
    height: u64,
    total_chunks: u32,
    received_chunks: u32,
    delivered: bool,
    recovered: bool,
    invalid: bool,
}

fn parse_rbc_sessions(root: &Value) -> eyre::Result<Vec<RbcSessionView>> {
    let Some(items) = root
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    let mut sessions = Vec::with_capacity(items.len());
    for item in items {
        let obj = item
            .as_object()
            .ok_or_else(|| eyre!("RBC session entry must be an object"))?;
        let block_hash = field_as_string(obj, "block_hash")?;
        let height = field_as_u64(obj, "height")?;
        let total_chunks = field_as_u32(obj, "total_chunks")?;
        let received_chunks = field_as_u32(obj, "received_chunks")?;
        let delivered = field_as_bool(obj, "delivered")?;
        let recovered = obj
            .get("recovered")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let invalid = obj.get("invalid").and_then(Value::as_bool).unwrap_or(false);
        sessions.push(RbcSessionView {
            block_hash,
            height,
            total_chunks,
            received_chunks,
            delivered,
            recovered,
            invalid,
        });
    }
    Ok(sessions)
}

fn field_as_string(obj: &Map, field: &str) -> eyre::Result<String> {
    obj.get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| eyre!("RBC session missing {field} field"))
}

fn field_as_u64(obj: &Map, field: &str) -> eyre::Result<u64> {
    obj.get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("RBC session missing {field} field"))
}

fn field_as_u32(obj: &Map, field: &str) -> eyre::Result<u32> {
    let value = field_as_u64(obj, field)?;
    u32::try_from(value)
        .map_err(|_| eyre!("RBC session field {field} does not fit in u32 (value={value})"))
}

fn field_as_bool(obj: &Map, field: &str) -> eyre::Result<bool> {
    obj.get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| eyre!("RBC session missing {field} field"))
}
