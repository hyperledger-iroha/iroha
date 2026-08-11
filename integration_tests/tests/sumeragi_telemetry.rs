#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Sumeragi telemetry soak exercising long-running epochs with deterministic chunk loss.
//!
//! This integration test spins up an `NPoS` network, cross-checks
//! `/v1/sumeragi/telemetry` snapshots against the Prometheus metrics surface
//! over multiple block heights, then injects a large RBC payload with
//! deterministic chunk loss and validates the backlog telemetry.

use std::time::{Duration, Instant};

use eyre::{Context as _, Result, ensure};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::{
    client::{Client, Status},
    data_model::{
        Level,
        isi::{Log, SetParameter},
        parameter::{Parameter, system::SumeragiNposParameters},
    },
};
use iroha_core::sumeragi::network_topology::commit_quorum_from_len;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::json::Value;
use reqwest::Client as HttpClient;
use tokio::time::sleep;

const EPOCH_LENGTH_BLOCKS: u64 = 6;
const DROP_EVERY_NTH_CHUNK: i64 = 3;
const RBC_STORE_SOFT_SESSIONS: i64 = 1;
const RBC_STORE_MAX_SESSIONS: i64 = 4;
const RBC_STORE_SOFT_BYTES: i64 = 64 * 1024;
const RBC_STORE_MAX_BYTES: i64 = 128 * 1024;
const RBC_CHUNK_MAX_BYTES: i64 = 16 * 1024;
const RBC_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;
const RBC_WARMUP_MESSAGES: u64 = 1;
const SAMPLE_ITERATIONS: usize = 3;
const PROGRESS_BLOCKS_PER_ITERATION: u64 = 1;
const TELEMETRY_RETRY_ATTEMPTS: usize = 40;

fn u64_to_f64(value: u64) -> f64 {
    const MAX_SAFE: u64 = 1 << f64::MANTISSA_DIGITS;
    debug_assert!(
        value < MAX_SAFE,
        "telemetry count {value} exceeds f64 mantissa precision"
    );
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

const TELEMETRY_RETRY_INTERVAL: Duration = Duration::from_millis(200);
const METRICS_RETRY_ATTEMPTS: usize = 20;
const METRICS_RETRY_INTERVAL: Duration = Duration::from_millis(200);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_telemetry_soak_matches_metrics_under_chunk_loss() -> Result<()> {
    init_instruction_registry();

    let npos_params = SumeragiNposParameters {
        epoch_length_blocks: std::num::NonZeroU64::new(EPOCH_LENGTH_BLOCKS)
            .expect("test epoch must be non-zero"),
        ..SumeragiNposParameters::default()
    };

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_millis(900))
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    RBC_CHUNK_MAX_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_sessions"],
                    RBC_STORE_SOFT_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_sessions"],
                    RBC_STORE_MAX_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_bytes"],
                    RBC_STORE_SOFT_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_bytes"],
                    RBC_STORE_MAX_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    900_000i64,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "drop_every_nth_chunk"],
                    DROP_EVERY_NTH_CHUNK,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos_params.into_custom_parameter(),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_telemetry_soak_matches_metrics_under_chunk_loss),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..2 {
        client.submit_blocking(
            Log::new(Level::INFO, format!("telemetry seed {idx}")),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    wait_for_total_height_quorum_with_bounded_lag(&network, 2).await?;

    let http = HttpClient::new();
    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;

    let mut previous_total_votes = 0_u64;
    let mut previous_vrf_updated = 0_u64;
    for iteration in 0..SAMPLE_ITERATIONS {
        let target_non_empty = client
            .get_status()
            .wrap_err("fetch status before telemetry iteration")?
            .blocks_non_empty
            .saturating_add(PROGRESS_BLOCKS_PER_ITERATION);
        submit_progress_logs(
            &network,
            PROGRESS_BLOCKS_PER_ITERATION,
            format!("telemetry iteration {iteration}"),
        )?;
        wait_for_non_empty_height_quorum_with_bounded_lag(&network, target_non_empty).await?;

        let telemetry = wait_for_telemetry(&client, |snapshot| {
            snapshot
                .get("availability")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("total_votes_ingested"))
                .and_then(Value::as_u64)
                .is_some()
        })
        .await?;
        let metrics = wait_for_metrics(&http, &metrics_url).await?;

        let total_votes = availability_total_votes(&telemetry)?;
        ensure!(
            total_votes >= previous_total_votes,
            "availability vote counter regressed (before={previous_total_votes}, after={total_votes})"
        );

        compare_availability(&telemetry, &metrics)?;
        compare_qc_latency(&telemetry, &metrics)?;
        compare_rbc_backlog(&telemetry, &metrics)?;

        if let Some(vrf) = telemetry.get("vrf").and_then(Value::as_object)
            && let Some(updated) = vrf.get("updated_at_height").and_then(Value::as_u64)
        {
            ensure!(
                updated >= previous_vrf_updated,
                "vrf updated_at_height regressed (prev={previous_vrf_updated}, now={updated})"
            );
            previous_vrf_updated = updated;
        }

        previous_total_votes = total_votes;
    }

    inject_large_rbc_payloads(&network, "telemetry adversarial backlog".to_owned()).await?;
    let telemetry = wait_for_telemetry(&client, |snapshot| {
        snapshot
            .get("rbc_backlog")
            .and_then(Value::as_object)
            .is_some()
    })
    .await?;
    let metrics = wait_for_metrics(&http, &metrics_url).await?;
    compare_rbc_backlog(&telemetry, &metrics)?;

    network.shutdown().await;
    Ok(())
}

fn availability_total_votes(snapshot: &Value) -> Result<u64> {
    snapshot
        .get("availability")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("total_votes_ingested"))
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre::eyre!("telemetry payload missing availability total"))
}

fn compare_availability(snapshot: &Value, metrics: &MetricsReader) -> Result<()> {
    let availability = snapshot
        .get("availability")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre::eyre!("telemetry payload missing availability section"))?;
    let total_votes = availability
        .get("total_votes_ingested")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let metric_total = metrics.get("sumeragi_da_votes_ingested_total");
    ensure!(
        (metric_total - u64_to_f64(total_votes)).abs() < f64::EPSILON,
        "metric/telemetry total votes mismatch ({metric_total} vs {total_votes})"
    );

    Ok(())
}

fn compare_qc_latency(snapshot: &Value, metrics: &MetricsReader) -> Result<()> {
    let entries = snapshot
        .get("qc_latency_ms")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre::eyre!("telemetry payload missing qc_latency_ms section"))?;
    for entry in entries {
        if let Some(map) = entry.as_object() {
            let kind = map
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| eyre::eyre!("qc latency entry missing kind"))?;
            let last = map.get("last_ms").and_then(Value::as_u64).unwrap_or(0);
            let metric_key = format!("sumeragi_qc_last_latency_ms{{kind=\"{kind}\"}}");
            let metric_value = metrics.get_optional(&metric_key).unwrap_or(0.0);
            if last == 0 {
                ensure!(
                    metric_value == 0.0,
                    "qc latency metric expected to be zero ({metric_value})"
                );
            } else {
                ensure!(
                    (metric_value - u64_to_f64(last)).abs() < f64::EPSILON,
                    "qc latency metric mismatch for {kind} ({metric_value} vs {last})"
                );
            }
        }
    }
    Ok(())
}

fn compare_rbc_backlog(snapshot: &Value, metrics: &MetricsReader) -> Result<()> {
    let backlog = snapshot
        .get("rbc_backlog")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre::eyre!("telemetry payload missing rbc_backlog section"))?;
    let pending = backlog
        .get("pending_sessions")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_missing = backlog
        .get("total_missing_chunks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let max_missing = backlog
        .get("max_missing_chunks")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let metric_pending = metrics
        .get_optional("sumeragi_rbc_backlog_sessions_pending")
        .unwrap_or(0.0);
    let metric_total_missing = metrics
        .get_optional("sumeragi_rbc_backlog_chunks_total")
        .unwrap_or(0.0);
    let metric_max_missing = metrics
        .get_optional("sumeragi_rbc_backlog_chunks_max")
        .unwrap_or(0.0);

    ensure!(
        (metric_pending - u64_to_f64(pending)).abs() < f64::EPSILON,
        "pending sessions metric mismatch ({metric_pending} vs {pending})"
    );
    ensure!(
        (metric_total_missing - u64_to_f64(total_missing)).abs() < f64::EPSILON,
        "total missing chunks metric mismatch ({metric_total_missing} vs {total_missing})"
    );
    ensure!(
        (metric_max_missing - u64_to_f64(max_missing)).abs() < f64::EPSILON,
        "max missing chunks metric mismatch ({metric_max_missing} vs {max_missing})"
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum TelemetryHeightKind {
    Total,
    NonEmpty,
}

impl TelemetryHeightKind {
    fn label(self) -> &'static str {
        match self {
            Self::Total => "total",
            Self::NonEmpty => "non_empty",
        }
    }

    fn value(self, status: &Status) -> u64 {
        match self {
            Self::Total => status.blocks,
            Self::NonEmpty => status.blocks_non_empty,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TelemetryHeightSnapshot {
    blocks: u64,
    blocks_non_empty: u64,
    queue_size: u64,
}

impl TelemetryHeightSnapshot {
    fn from_status(status: &Status) -> Self {
        Self {
            blocks: status.blocks,
            blocks_non_empty: status.blocks_non_empty,
            queue_size: status.queue_size,
        }
    }
}

async fn wait_for_total_height_quorum_with_bounded_lag(
    network: &iroha_test_network::Network,
    target_height: u64,
) -> Result<()> {
    wait_for_height_quorum_with_bounded_lag(network, TelemetryHeightKind::Total, target_height)
        .await
}

async fn wait_for_non_empty_height_quorum_with_bounded_lag(
    network: &iroha_test_network::Network,
    target_height: u64,
) -> Result<()> {
    wait_for_height_quorum_with_bounded_lag(network, TelemetryHeightKind::NonEmpty, target_height)
        .await
}

async fn wait_for_height_quorum_with_bounded_lag(
    network: &iroha_test_network::Network,
    kind: TelemetryHeightKind,
    target_height: u64,
) -> Result<()> {
    let deadline = Instant::now() + network.sync_timeout();
    let peer_count = network.peers().len();
    let quorum = commit_quorum_from_len(peer_count).max(1);

    loop {
        let mut snapshots = Vec::new();
        let mut heights = Vec::new();
        let mut client_height = None;
        let mut errors = Vec::new();

        for (idx, peer) in network.peers().iter().enumerate() {
            if !peer.is_running() {
                continue;
            }
            match peer.status().await {
                Ok(status) => {
                    let height = kind.value(&status);
                    if idx == 0 {
                        client_height = Some(height);
                    }
                    heights.push(height);
                    snapshots.push(TelemetryHeightSnapshot::from_status(&status));
                }
                Err(err) => errors.push(format!("peer#{idx}: {err:#}")),
            }
        }

        if errors.is_empty()
            && client_and_quorum_height_with_bounded_lag(
                client_height,
                &heights,
                target_height,
                quorum,
            )
        {
            return Ok(());
        }

        if Instant::now() >= deadline {
            let label = kind.label();
            eyre::bail!(
                "telemetry {label} height quorum did not reach {target_height} within {:?}: quorum={quorum}, client_height={client_height:?}, heights={heights:?}, snapshot={snapshots:?}, errors={errors:?}",
                network.sync_timeout()
            );
        }

        sleep(TELEMETRY_RETRY_INTERVAL).await;
    }
}

fn client_and_quorum_height_with_bounded_lag(
    client_height: Option<u64>,
    heights: &[u64],
    target_height: u64,
    quorum: usize,
) -> bool {
    client_height.is_some_and(|height| height >= target_height)
        && height_quorum_with_bounded_lag(heights, target_height, quorum)
}

fn height_quorum_with_bounded_lag(heights: &[u64], target_height: u64, quorum: usize) -> bool {
    if heights
        .iter()
        .filter(|height| **height >= target_height)
        .count()
        < quorum
    {
        return false;
    }

    let Some(min_height) = heights.iter().min().copied() else {
        return false;
    };
    let Some(max_height) = heights.iter().max().copied() else {
        return false;
    };

    max_height >= target_height && max_height.saturating_sub(min_height) <= 1
}

async fn wait_for_telemetry<F>(client: &Client, predicate: F) -> Result<Value>
where
    F: Fn(&Value) -> bool,
{
    for attempt in 0..TELEMETRY_RETRY_ATTEMPTS {
        let request_client = client.clone();
        let value =
            tokio::task::spawn_blocking(move || request_client.get_sumeragi_telemetry_json())
                .await
                .wrap_err("join operator-signed telemetry request")?
                .wrap_err("fetch operator-signed telemetry snapshot")?;
        if predicate(&value) {
            return Ok(value);
        }
        if attempt + 1 == TELEMETRY_RETRY_ATTEMPTS {
            break;
        }
        sleep(TELEMETRY_RETRY_INTERVAL).await;
    }
    eyre::bail!("telemetry endpoint did not satisfy predicate within retries")
}

async fn wait_for_metrics(http: &HttpClient, url: &reqwest::Url) -> Result<MetricsReader> {
    for attempt in 0..METRICS_RETRY_ATTEMPTS {
        let response = http
            .get(url.clone())
            .header("accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        if !response.status().is_success() {
            if attempt + 1 == METRICS_RETRY_ATTEMPTS {
                eyre::bail!("metrics endpoint returned {}", response.status());
            }
            sleep(METRICS_RETRY_INTERVAL).await;
            continue;
        }
        let body = response.text().await.wrap_err("read metrics body")?;
        return Ok(MetricsReader::new(&body));
    }
    eyre::bail!("metrics endpoint did not return success within retries")
}

async fn inject_large_rbc_payloads(
    network: &iroha_test_network::Network,
    batch_prefix: String,
) -> Result<()> {
    let submit_client = network.client();
    for idx in 0..RBC_WARMUP_MESSAGES {
        let prefix = format!("{batch_prefix}-{idx:02}-");
        let filler = "L".repeat(RBC_PAYLOAD_BYTES.saturating_sub(prefix.len()));
        let message = format!("{prefix}{filler}");
        submit_client.submit(
            Log::new(Level::INFO, message),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    Ok(())
}

fn submit_progress_logs(
    network: &iroha_test_network::Network,
    blocks: u64,
    prefix: String,
) -> Result<()> {
    let client = network.client();
    for idx in 0..blocks {
        client.submit(
            Log::new(Level::INFO, format!("{prefix} block {idx}")),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        availability_total_votes, client_and_quorum_height_with_bounded_lag, compare_availability,
    };

    #[test]
    fn compare_availability_handles_zero_counters() {
        use integration_tests::metrics::MetricsReader;

        let payload = norito::json!({
            "availability": {
                "total_votes_ingested": 0
            },
            "qc_latency_ms": [],
            "rbc_backlog": {"pending_sessions": 0, "total_missing_chunks": 0, "max_missing_chunks": 0},
        });
        let metrics = MetricsReader::new(
            "sumeragi_da_votes_ingested_total 0\n\
             sumeragi_qc_last_latency_ms{kind=\"availability\"} 0\n\
             sumeragi_rbc_backlog_sessions_pending 0\n\
             sumeragi_rbc_backlog_chunks_total 0\n\
             sumeragi_rbc_backlog_chunks_max 0\n",
        );
        compare_availability(&payload, &metrics).expect("availability comparison succeeds");
        assert_eq!(availability_total_votes(&payload).expect("total votes"), 0);
    }

    #[test]
    fn client_and_quorum_height_accepts_one_block_tail_lag() {
        assert!(client_and_quorum_height_with_bounded_lag(
            Some(4),
            &[4, 4, 4, 3],
            4,
            3
        ));
    }

    #[test]
    fn client_and_quorum_height_rejects_unsafe_progress_splits() {
        assert!(!client_and_quorum_height_with_bounded_lag(
            Some(3),
            &[3, 4, 4, 4],
            4,
            3
        ));
        assert!(!client_and_quorum_height_with_bounded_lag(
            Some(4),
            &[4, 4, 3, 3],
            4,
            3
        ));
        assert!(!client_and_quorum_height_with_bounded_lag(
            Some(5),
            &[5, 5, 5, 3],
            5,
            3
        ));
    }
}
