#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Runtime Nexus lane-registration benchmark with isolated timing metrics.

use std::{
    collections::{BTreeMap, BTreeSet},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        block::consensus::SumeragiStatusWire,
        nexus::{DataSpaceId, LaneConfig, LaneId, LaneLifecyclePlan, LaneVisibility},
    },
};
use iroha_test_network::NetworkBuilder;
use norito::json::Value as JsonValue;
use rand::Rng;
use reqwest::StatusCode;
use sha2::{Digest as _, Sha256};
use toml::{Table, Value as TomlValue};

const TOTAL_PEERS: usize = 4;
const BENCH_ITERATIONS: usize = 5;
const BASE_LANE_ID: u32 = 10;
const NEXUS_ALIAS: &str = "nexus";
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const HEADER_OPERATOR_PUBLIC_KEY: &str = "x-iroha-operator-public-key";
const HEADER_OPERATOR_TIMESTAMP_MS: &str = "x-iroha-operator-timestamp-ms";
const HEADER_OPERATOR_NONCE: &str = "x-iroha-operator-nonce";
const HEADER_OPERATOR_SIGNATURE: &str = "x-iroha-operator-signature";

fn runtime_registration_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        .with_config_layer(|layer| {
            let mut lane_nexus = Table::new();
            lane_nexus.insert("index".into(), TomlValue::Integer(0));
            lane_nexus.insert("alias".into(), TomlValue::String("lane-nexus".to_owned()));
            lane_nexus.insert(
                "dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            lane_nexus.insert("visibility".into(), TomlValue::String("public".to_owned()));
            lane_nexus.insert("metadata".into(), TomlValue::Table(Table::new()));

            let mut ds_nexus = Table::new();
            ds_nexus.insert("alias".into(), TomlValue::String(NEXUS_ALIAS.to_owned()));
            ds_nexus.insert("id".into(), TomlValue::Integer(0));
            ds_nexus.insert(
                "description".into(),
                TomlValue::String("runtime benchmark baseline dataspace".to_owned()),
            );
            ds_nexus.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut routing_policy = Table::new();
            routing_policy.insert("default_lane".into(), TomlValue::Integer(0));
            routing_policy.insert(
                "default_dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            routing_policy.insert("rules".into(), TomlValue::Array(Vec::new()));

            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 1_i64)
                .write(
                    ["nexus", "lane_catalog"],
                    TomlValue::Array(vec![TomlValue::Table(lane_nexus)]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    TomlValue::Array(vec![TomlValue::Table(ds_nexus)]),
                )
                .write(
                    ["nexus", "routing_policy"],
                    TomlValue::Table(routing_policy),
                );
        })
}

// TODO: Extend this benchmark with runtime dataspace-catalog mutations once a consensus-backed public API is available.

fn wait_for_lane_visibility(client: &Client, lane_id: LaneId, context: &str) -> Result<JsonValue> {
    let started = Instant::now();
    let mut last_error = String::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.get_public_lane_validators(lane_id, None) {
            Ok(snapshot) => return Ok(snapshot),
            Err(err) => {
                last_error = err.to_string();
                thread::sleep(STATUS_POLL_INTERVAL);
            }
        }
    }
    Err(eyre!(
        "{context}: timed out waiting for lane {lane_id} visibility; last error: {last_error}"
    ))
}

fn wait_for_all_peers_lane_visibility(
    network: &sandbox::SerializedNetwork,
    lane_id: LaneId,
    skip_peer_index: Option<usize>,
    context: &str,
) -> Result<()> {
    let mut pending: BTreeSet<usize> = (0..network.peers().len()).collect();
    if let Some(index) = skip_peer_index {
        pending.remove(&index);
    }
    if pending.is_empty() {
        return Ok(());
    }

    let started = Instant::now();
    let mut last_errors: BTreeMap<usize, String> = BTreeMap::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let mut resolved = Vec::new();
        for peer_index in pending.iter().copied() {
            let client = network.peers()[peer_index].client();
            match client.get_public_lane_validators(lane_id, None) {
                Ok(_snapshot) => resolved.push(peer_index),
                Err(err) => {
                    last_errors.insert(peer_index, err.to_string());
                }
            }
        }

        for peer_index in resolved {
            pending.remove(&peer_index);
            last_errors.remove(&peer_index);
        }
        if pending.is_empty() {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    Err(eyre!(
        "{context}: timed out waiting for lane {lane_id} visibility on peers {pending:?}; last errors {last_errors:?}"
    ))
}

fn wait_for_lane_visibility_with_status(
    client: &Client,
    lane_id: LaneId,
    context: &str,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0_u64;
    let mut last_error = String::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let status = client
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?;
        last_height = status.commit_qc.height;
        match client.get_public_lane_validators(lane_id, None) {
            Ok(_snapshot) => return Ok(status),
            Err(err) => {
                last_error = err.to_string();
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for lane {lane_id} visibility; last height {last_height}; last visibility error: {last_error}"
    ))
}

fn leader_or_highest_height_peer_index(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
) -> usize {
    let peers = network.peers();
    if peers.is_empty() {
        return 0;
    }

    if let Ok(status) = status_client.get_sumeragi_status_wire() {
        if let Ok(index) = usize::try_from(status.leader_index) {
            if index < peers.len() {
                let leader_height = peers[index]
                    .client()
                    .get_sumeragi_status_wire()
                    .map(|status| status.commit_qc.height)
                    .unwrap_or(0);
                if leader_height.saturating_add(1) >= status.commit_qc.height {
                    return index;
                }
            }
        }
    }

    peers
        .iter()
        .enumerate()
        .fold((0usize, 0u64), |best, (index, peer)| {
            let observed_height = peer
                .client()
                .get_sumeragi_status_wire()
                .map(|status| status.commit_qc.height)
                .unwrap_or(0);
            if observed_height >= best.1 {
                (index, observed_height)
            } else {
                best
            }
        })
        .0
}

fn duration_min_avg_max(samples: &[Duration]) -> Option<(Duration, Duration, Duration)> {
    let mut iter = samples.iter();
    let first = *iter.next()?;
    let mut min = first;
    let mut max = min;
    let mut total = min.as_secs_f64();
    let mut count = 1usize;
    for sample in iter {
        min = min.min(*sample);
        max = max.max(*sample);
        let secs = sample.as_secs_f64();
        total += secs;
        count += 1;
    }
    Some((min, Duration::from_secs_f64(total / count as f64), max))
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs >= 1.0 {
        format!("{secs:.3}s")
    } else if secs >= 0.001 {
        format!("{:.3}ms", secs * 1_000.0)
    } else {
        format!("{:.3}us", secs * 1_000_000.0)
    }
}

async fn post_lane_lifecycle_plan(
    http: &reqwest::Client,
    client: &Client,
    plan: &LaneLifecyclePlan,
) -> Result<()> {
    let url = client
        .torii_url
        .join("v1/nexus/lifecycle")
        .wrap_err("compose /v1/nexus/lifecycle URL")?;
    let body = norito::json::to_json(plan).wrap_err("serialize lane lifecycle plan")?;
    let operator_headers = operator_signature_headers(client, "POST", url.path(), body.as_bytes())?;

    let mut request = http
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json");
    for (name, value) in &client.headers {
        request = request.header(name, value);
    }
    for (name, value) in operator_headers {
        request = request.header(name, value);
    }

    let response = request
        .body(body)
        .send()
        .await
        .wrap_err("send /v1/nexus/lifecycle request")?;
    let status = response.status();
    let payload = response.text().await.unwrap_or_default();
    ensure!(
        status == StatusCode::ACCEPTED,
        "nexus lifecycle request failed with {status}: {payload}"
    );
    Ok(())
}

fn operator_signature_headers(
    client: &Client,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<Vec<(&'static str, String)>> {
    let Some(operator_key_pair) = client.operator_key_pair.as_ref() else {
        return Ok(Vec::new());
    };

    let timestamp_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let nonce_bytes: [u8; 12] = rand::rng().random();
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);

    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash_hex = hex::encode(hasher.finalize());
    let message = format!(
        "{}\n{}\n\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        path,
        body_hash_hex,
        timestamp_ms,
        nonce
    )
    .into_bytes();

    let signature = iroha_crypto::Signature::new(operator_key_pair.private_key(), &message);
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.payload());

    Ok(vec![
        (
            HEADER_OPERATOR_PUBLIC_KEY,
            operator_key_pair.public_key().to_string(),
        ),
        (HEADER_OPERATOR_TIMESTAMP_MS, timestamp_ms.to_string()),
        (HEADER_OPERATOR_NONCE, nonce),
        (HEADER_OPERATOR_SIGNATURE, signature_b64),
    ])
}

#[derive(Debug)]
struct RegistrationIterationMetrics {
    lane_id: u32,
    submitter_peer_index: usize,
    submitter_height_delta: u64,
    submit_latency: Duration,
    commit_apply_latency: Duration,
    all_peer_visibility_latency: Duration,
    total_latency: Duration,
}

fn run_registration_iteration(
    rt: &tokio::runtime::Runtime,
    http: &reqwest::Client,
    network: &sandbox::SerializedNetwork,
    iteration: usize,
) -> Result<RegistrationIterationMetrics> {
    let lane_raw = BASE_LANE_ID + u32::try_from(iteration).unwrap_or(u32::MAX);
    let lane_id = LaneId::new(lane_raw);
    let lane = LaneConfig {
        id: lane_id,
        dataspace_id: DataSpaceId::GLOBAL,
        alias: format!("bench-lane-{lane_raw}"),
        description: Some(format!("runtime registration benchmark lane {lane_raw}")),
        visibility: LaneVisibility::Public,
        metadata: BTreeMap::from([("benchmark".to_owned(), "runtime_registration".to_owned())]),
        ..LaneConfig::default()
    };
    let plan = LaneLifecyclePlan {
        additions: vec![lane],
        retire: Vec::new(),
    };

    let status_probe_client = network.peer().client();
    let submitter_peer_index = leader_or_highest_height_peer_index(network, &status_probe_client);
    let submitter = network.peers()[submitter_peer_index].client();
    let baseline_height = submitter
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))
        .wrap_err("fetch submitter baseline height")?
        .commit_qc
        .height;

    let started = Instant::now();
    let submit_started = Instant::now();
    rt.block_on(post_lane_lifecycle_plan(http, &submitter, &plan))
        .wrap_err_with(|| {
            format!("submit lifecycle plan to peer {submitter_peer_index} as leader target")
        })?;
    let submit_latency = submit_started.elapsed();

    let apply_started = Instant::now();
    let submitter_status = wait_for_lane_visibility_with_status(
        &submitter,
        lane_id,
        "wait for submitter lane visibility and commit/apply",
    )
    .wrap_err_with(|| {
        format!(
            "wait for lane {} commit/apply on submitter peer {}",
            lane_id.as_u32(),
            submitter_peer_index
        )
    })?;
    let commit_apply_latency = apply_started.elapsed();
    let submitter_height_delta = submitter_status
        .commit_qc
        .height
        .saturating_sub(baseline_height);

    let visibility_started = Instant::now();
    wait_for_all_peers_lane_visibility(
        network,
        lane_id,
        Some(submitter_peer_index),
        "wait for all-peer lane visibility",
    )
    .wrap_err_with(|| format!("wait for lane {} visibility on all peers", lane_id.as_u32()))?;

    let all_peer_visibility_latency = visibility_started.elapsed();
    let total_latency = started.elapsed();
    Ok(RegistrationIterationMetrics {
        lane_id: lane_raw,
        submitter_peer_index,
        submitter_height_delta,
        submit_latency,
        commit_apply_latency,
        all_peer_visibility_latency,
        total_latency,
    })
}

#[test]
fn runtime_nexus_registration_reports_lane_lifecycle_costs() -> Result<()> {
    let context = stringify!(runtime_nexus_registration_reports_lane_lifecycle_costs);
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(runtime_registration_builder(), context)?
    else {
        return Ok(());
    };

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers for runtime registration benchmark, got {}",
        network.peers().len()
    );
    for (peer_index, peer) in network.peers().iter().enumerate() {
        let client = peer.client();
        let _baseline_lane = wait_for_lane_visibility(&client, LaneId::new(0), "baseline lane")
            .wrap_err_with(|| format!("wait for baseline lane visibility on peer {peer_index}"))?;
    }

    let http = reqwest::Client::builder()
        .build()
        .wrap_err("build reqwest client for lifecycle benchmark")?;
    eprintln!("[registration-perf] report-only metrics (no threshold gating)");

    let mut submit_samples = Vec::with_capacity(BENCH_ITERATIONS);
    let mut commit_apply_samples = Vec::with_capacity(BENCH_ITERATIONS);
    let mut visibility_samples = Vec::with_capacity(BENCH_ITERATIONS);
    let mut total_samples = Vec::with_capacity(BENCH_ITERATIONS);
    let mut height_delta_samples = Vec::with_capacity(BENCH_ITERATIONS);
    let mut passes = 0usize;
    let mut failure: Option<eyre::Report> = None;

    for iteration in 0..BENCH_ITERATIONS {
        match run_registration_iteration(&rt, &http, &network, iteration) {
            Ok(metrics) => {
                passes += 1;
                submit_samples.push(metrics.submit_latency);
                commit_apply_samples.push(metrics.commit_apply_latency);
                visibility_samples.push(metrics.all_peer_visibility_latency);
                total_samples.push(metrics.total_latency);
                height_delta_samples.push(metrics.submitter_height_delta);

                eprintln!(
                    "[registration-perf] iter={}/{} lane={} submitter_peer={} height_delta={} submit={} commit/apply={} all-peer-visibility={} total={}",
                    iteration + 1,
                    BENCH_ITERATIONS,
                    metrics.lane_id,
                    metrics.submitter_peer_index,
                    metrics.submitter_height_delta,
                    format_duration(metrics.submit_latency),
                    format_duration(metrics.commit_apply_latency),
                    format_duration(metrics.all_peer_visibility_latency),
                    format_duration(metrics.total_latency),
                );
            }
            Err(err) => {
                failure =
                    Some(err.wrap_err(format!("registration iteration {} failed", iteration + 1)));
                break;
            }
        }
    }

    let pass_rate = (passes as f64 / BENCH_ITERATIONS as f64) * 100.0;
    if let Some((min, avg, max)) = duration_min_avg_max(&submit_samples) {
        eprintln!(
            "[registration-perf] submit latency min/avg/max = {}/{}/{}",
            format_duration(min),
            format_duration(avg),
            format_duration(max)
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max(&commit_apply_samples) {
        eprintln!(
            "[registration-perf] commit/apply latency min/avg/max = {}/{}/{}",
            format_duration(min),
            format_duration(avg),
            format_duration(max)
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max(&visibility_samples) {
        eprintln!(
            "[registration-perf] all-peer visibility latency min/avg/max = {}/{}/{}",
            format_duration(min),
            format_duration(avg),
            format_duration(max)
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max(&total_samples) {
        eprintln!(
            "[registration-perf] total workflow latency min/avg/max = {}/{}/{}",
            format_duration(min),
            format_duration(avg),
            format_duration(max)
        );
    }
    if !height_delta_samples.is_empty() {
        let min = *height_delta_samples.iter().min().unwrap_or(&0);
        let max = *height_delta_samples.iter().max().unwrap_or(&0);
        let avg =
            height_delta_samples.iter().sum::<u64>() as f64 / height_delta_samples.len() as f64;
        eprintln!(
            "[registration-perf] submitter commit height delta min/avg/max = {min}/{avg:.2}/{max}"
        );
        if max == 0 {
            eprintln!(
                "[registration-perf] note: lifecycle visibility completed without observed commit-height advancement on submitter"
            );
        }
    }
    eprintln!(
        "[registration-perf] iterations={} pass_rate={:.1}%",
        BENCH_ITERATIONS, pass_rate
    );

    if let Some(err) = failure {
        return Err(err);
    }
    ensure!(
        passes == BENCH_ITERATIONS,
        "registration benchmark did not complete all iterations"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{duration_min_avg_max, format_duration};
    use std::time::Duration;

    #[test]
    fn duration_min_avg_max_reports_expected_values() {
        let samples = [
            Duration::from_millis(10),
            Duration::from_millis(30),
            Duration::from_millis(20),
        ];
        let (min, avg, max) = duration_min_avg_max(&samples).expect("stats");
        assert_eq!(min, Duration::from_millis(10));
        assert_eq!(avg, Duration::from_millis(20));
        assert_eq!(max, Duration::from_millis(30));
    }

    #[test]
    fn format_duration_selects_unit_by_scale() {
        assert_eq!(format_duration(Duration::from_secs(2)), "2.000s");
        assert_eq!(format_duration(Duration::from_millis(10)), "10.000ms");
        assert_eq!(format_duration(Duration::from_nanos(500)), "0.500us");
    }
}
