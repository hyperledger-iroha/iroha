#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Runtime Nexus lane-registration benchmark with isolated timing metrics.

use std::{
    collections::BTreeMap,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::nexus::{DataSpaceId, LaneConfig, LaneId, LaneLifecyclePlan, LaneVisibility},
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

fn duration_min_avg_max_secs(samples: &[Duration]) -> Option<(f64, f64, f64)> {
    let mut iter = samples.iter();
    let first = iter.next()?;
    let mut min = first.as_secs_f64();
    let mut max = min;
    let mut total = min;
    let mut count = 1usize;
    for sample in iter {
        let secs = sample.as_secs_f64();
        min = min.min(secs);
        max = max.max(secs);
        total += secs;
        count += 1;
    }
    Some((min, total / count as f64, max))
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
    submit_samples: Vec<Duration>,
    apply_samples: Vec<Duration>,
    visibility_latency: Duration,
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

    let started = Instant::now();
    let mut submit_samples = Vec::with_capacity(network.peers().len());
    let mut apply_samples = Vec::with_capacity(network.peers().len());

    for (peer_index, peer) in network.peers().iter().enumerate() {
        let client = peer.client();

        let submit_started = Instant::now();
        rt.block_on(post_lane_lifecycle_plan(http, &client, &plan))
            .wrap_err_with(|| format!("submit lifecycle plan to peer {peer_index}"))?;
        submit_samples.push(submit_started.elapsed());

        let apply_started = Instant::now();
        let _lane_snapshot =
            wait_for_lane_visibility(&client, lane_id, "wait for submitter lane visibility")
                .wrap_err_with(|| {
                    format!(
                        "wait for lane {} visibility on submitter peer {}",
                        lane_id.as_u32(),
                        peer_index
                    )
                })?;
        apply_samples.push(apply_started.elapsed());
    }

    for (peer_index, peer) in network.peers().iter().enumerate() {
        let client = peer.client();
        let _lane_snapshot =
            wait_for_lane_visibility(&client, lane_id, "wait for all-peer lane visibility")
                .wrap_err_with(|| {
                    format!(
                        "wait for lane {} visibility on peer {}",
                        lane_id.as_u32(),
                        peer_index
                    )
                })?;
    }

    let total_latency = started.elapsed();
    Ok(RegistrationIterationMetrics {
        lane_id: lane_raw,
        submit_samples,
        apply_samples,
        visibility_latency: total_latency,
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

    let mut submit_samples = Vec::new();
    let mut apply_samples = Vec::new();
    let mut visibility_samples = Vec::new();
    let mut total_samples = Vec::new();
    let mut passes = 0usize;
    let mut failure: Option<eyre::Report> = None;

    for iteration in 0..BENCH_ITERATIONS {
        match run_registration_iteration(&rt, &http, &network, iteration) {
            Ok(metrics) => {
                passes += 1;
                submit_samples.extend(metrics.submit_samples.iter().copied());
                apply_samples.extend(metrics.apply_samples.iter().copied());
                visibility_samples.push(metrics.visibility_latency);
                total_samples.push(metrics.total_latency);

                let submit_stats = duration_min_avg_max_secs(&metrics.submit_samples)
                    .expect("submit samples present");
                let apply_stats = duration_min_avg_max_secs(&metrics.apply_samples)
                    .expect("apply samples present");
                eprintln!(
                    "[registration-perf] iter={}/{} lane={} submit(min/avg/max)={:.3}/{:.3}/{:.3}s apply(min/avg/max)={:.3}/{:.3}/{:.3}s visibility={:.3}s total={:.3}s",
                    iteration + 1,
                    BENCH_ITERATIONS,
                    metrics.lane_id,
                    submit_stats.0,
                    submit_stats.1,
                    submit_stats.2,
                    apply_stats.0,
                    apply_stats.1,
                    apply_stats.2,
                    metrics.visibility_latency.as_secs_f64(),
                    metrics.total_latency.as_secs_f64(),
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
    if let Some((min, avg, max)) = duration_min_avg_max_secs(&submit_samples) {
        eprintln!(
            "[registration-perf] submit latency min/avg/max = {:.3}/{:.3}/{:.3}s",
            min, avg, max
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max_secs(&apply_samples) {
        eprintln!(
            "[registration-perf] commit/apply latency min/avg/max = {:.3}/{:.3}/{:.3}s",
            min, avg, max
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max_secs(&visibility_samples) {
        eprintln!(
            "[registration-perf] all-peer visibility latency min/avg/max = {:.3}/{:.3}/{:.3}s",
            min, avg, max
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max_secs(&total_samples) {
        eprintln!(
            "[registration-perf] total workflow latency min/avg/max = {:.3}/{:.3}/{:.3}s",
            min, avg, max
        );
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
