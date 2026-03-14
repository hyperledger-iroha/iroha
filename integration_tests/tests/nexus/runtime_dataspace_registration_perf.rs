#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Runtime Nexus lane and dataspace-registration benchmark with isolated timing metrics.

use std::{
    collections::{BTreeMap, BTreeSet},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::{
    client::{Client, UaidManifestQuery, UaidManifestStatus, UaidManifestStatusFilter},
    crypto::Hash,
    data_model::{
        block::consensus::SumeragiStatusWire,
        events::{
            EventBox,
            pipeline::{PipelineEventBox, TransactionEventFilter, TransactionStatus},
        },
        isi::{
            Grant, InstructionBox,
            space_directory::{PublishSpaceDirectoryManifest, RevokeSpaceDirectoryManifest},
        },
        metadata::Metadata,
        nexus::{
            Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceId,
            LaneConfig, LaneId, LaneLifecyclePlan, LaneVisibility, ManifestEffect, ManifestEntry,
            ManifestVersion, UniversalAccountId,
        },
        prelude::Numeric,
        transaction::SignedTransaction,
    },
};
use iroha_executor_data_model::permission::nexus::CanPublishSpaceDirectoryManifest;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::ALICE_ID;
use norito::json::Value as JsonValue;
use rand::Rng;
use reqwest::StatusCode;
use sha2::{Digest as _, Sha256};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::{Table, Value as TomlValue};

const TOTAL_PEERS: usize = 4;
const BENCH_ITERATIONS: usize = 5;
const BENCH_ITERATIONS_ENV: &str = "IROHA_NEXUS_RUNTIME_BENCH_ITERATIONS";
const BASE_LANE_ID: u32 = 10;
const NEXUS_ALIAS: &str = "nexus";
const BENCH_MANIFEST_DATASPACE: DataSpaceId = DataSpaceId::new(4_096);
const BENCH_MANIFEST_ACTIVATION_EPOCH: u64 = 4_096;
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const COMMITTED_TX_OUTCOME_TIMEOUT: Duration = Duration::from_secs(30);
const TX_SSE_HANDSHAKE_DELAY: Duration = Duration::from_millis(100);
const HEADER_OPERATOR_PUBLIC_KEY: &str = "x-iroha-operator-public-key";
const HEADER_OPERATOR_TIMESTAMP_MS: &str = "x-iroha-operator-timestamp-ms";
const HEADER_OPERATOR_NONCE: &str = "x-iroha-operator-nonce";
const HEADER_OPERATOR_SIGNATURE: &str = "x-iroha-operator-signature";

fn parse_positive_usize_override(raw: Option<&str>, default: usize) -> usize {
    raw.and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn benchmark_iterations() -> usize {
    parse_positive_usize_override(
        std::env::var(BENCH_ITERATIONS_ENV).ok().as_deref(),
        BENCH_ITERATIONS,
    )
}

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

            let mut ds_benchmark = Table::new();
            ds_benchmark.insert(
                "alias".into(),
                TomlValue::String("runtime-benchmark".to_owned()),
            );
            ds_benchmark.insert(
                "id".into(),
                TomlValue::Integer(
                    i64::try_from(BENCH_MANIFEST_DATASPACE.as_u64())
                        .expect("benchmark dataspace id fits i64"),
                ),
            );
            ds_benchmark.insert(
                "description".into(),
                TomlValue::String(
                    "runtime benchmark dataspace for manifest publish/revoke metrics".to_owned(),
                ),
            );
            ds_benchmark.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut routing_policy = Table::new();
            let mut matcher_alice = Table::new();
            matcher_alice.insert("account".into(), TomlValue::String(ALICE_ID.to_string()));
            let mut rule_alice = Table::new();
            rule_alice.insert("lane".into(), TomlValue::Integer(0));
            rule_alice.insert(
                "dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            rule_alice.insert("matcher".into(), TomlValue::Table(matcher_alice));
            routing_policy.insert("default_lane".into(), TomlValue::Integer(0));
            routing_policy.insert(
                "default_dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            routing_policy.insert(
                "rules".into(),
                TomlValue::Array(vec![TomlValue::Table(rule_alice)]),
            );

            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 1_i64)
                .write(
                    ["nexus", "lane_catalog"],
                    TomlValue::Array(vec![TomlValue::Table(lane_nexus)]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(ds_nexus),
                        TomlValue::Table(ds_benchmark),
                    ]),
                )
                .write(
                    ["nexus", "routing_policy"],
                    TomlValue::Table(routing_policy),
                );
        })
}

fn wait_for_lane_visibility(client: &Client, lane_id: LaneId, context: &str) -> Result<JsonValue> {
    let started = Instant::now();
    let mut last_error = String::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.get_public_lane_validators(lane_id) {
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
            match client.get_public_lane_validators(lane_id) {
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

fn wait_for_lane_absence(client: &Client, lane_id: LaneId, context: &str) -> Result<()> {
    let started = Instant::now();
    let mut last_present = String::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.get_public_lane_validators(lane_id) {
            Ok(snapshot) => {
                if snapshot.get("total").and_then(JsonValue::as_u64) == Some(0) {
                    return Ok(());
                }
                last_present = format!("{snapshot:?}");
                thread::sleep(STATUS_POLL_INTERVAL);
            }
            Err(_) => return Ok(()),
        }
    }

    Err(eyre!(
        "{context}: timed out waiting for lane {lane_id} removal; last visible snapshot {last_present}"
    ))
}

fn wait_for_all_peers_lane_absence(
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
    let mut last_visible: BTreeMap<usize, String> = BTreeMap::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let mut resolved = Vec::new();
        for peer_index in pending.iter().copied() {
            let client = network.peers()[peer_index].client();
            match client.get_public_lane_validators(lane_id) {
                Ok(snapshot) => {
                    if snapshot.get("total").and_then(JsonValue::as_u64) == Some(0) {
                        resolved.push(peer_index);
                    } else {
                        last_visible.insert(peer_index, format!("{snapshot:?}"));
                    }
                }
                Err(_) => {
                    resolved.push(peer_index);
                }
            }
        }

        for peer_index in resolved {
            pending.remove(&peer_index);
            last_visible.remove(&peer_index);
        }
        if pending.is_empty() {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    Err(eyre!(
        "{context}: timed out waiting for lane {lane_id} removal on peers {pending:?}; last visible snapshots {last_visible:?}"
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
        match client.get_public_lane_validators(lane_id) {
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

fn wait_for_manifest_status(
    client: &Client,
    uaid_literal: &str,
    dataspace: DataSpaceId,
    expected_status: UaidManifestStatus,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_error = String::new();
    let mut last_statuses = Vec::<String>::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let query = UaidManifestQuery {
            dataspace_id: Some(dataspace.as_u64()),
            status: Some(UaidManifestStatusFilter::All),
            limit: Some(16),
            offset: Some(0),
        };
        match client.get_uaid_manifests(uaid_literal, Some(query)) {
            Ok(response) => {
                last_statuses = response
                    .manifests
                    .iter()
                    .map(|record| format!("{}:{:?}", record.dataspace_id, record.status))
                    .collect();
                if response.manifests.iter().any(|record| {
                    record.dataspace_id == dataspace.as_u64() && record.status == expected_status
                }) {
                    return Ok(());
                }
                last_error.clear();
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    if last_error.is_empty() {
        Err(eyre!(
            "{context}: timed out waiting for UAID {uaid_literal} dataspace {} status {:?}; last statuses {last_statuses:?}",
            dataspace.as_u64(),
            expected_status
        ))
    } else {
        Err(eyre!(
            "{context}: timed out waiting for UAID {uaid_literal} dataspace {} status {:?}; last statuses {last_statuses:?}; last error: {last_error}",
            dataspace.as_u64(),
            expected_status
        ))
    }
}

fn wait_for_all_peers_manifest_status(
    network: &sandbox::SerializedNetwork,
    uaid_literal: &str,
    dataspace: DataSpaceId,
    expected_status: UaidManifestStatus,
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
        let query = UaidManifestQuery {
            dataspace_id: Some(dataspace.as_u64()),
            status: Some(UaidManifestStatusFilter::All),
            limit: Some(16),
            offset: Some(0),
        };
        for peer_index in pending.iter().copied() {
            let client = network.peers()[peer_index].client();
            match client.get_uaid_manifests(uaid_literal, Some(query)) {
                Ok(response) => {
                    if response.manifests.iter().any(|record| {
                        record.dataspace_id == dataspace.as_u64()
                            && record.status == expected_status
                    }) {
                        resolved.push(peer_index);
                    } else {
                        let statuses = response
                            .manifests
                            .iter()
                            .map(|record| format!("{}:{:?}", record.dataspace_id, record.status))
                            .collect::<Vec<_>>();
                        last_errors.insert(
                            peer_index,
                            format!(
                                "{context}: expected {:?}, observed {statuses:?}",
                                expected_status
                            ),
                        );
                    }
                }
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
        "{context}: timed out waiting for UAID {uaid_literal} dataspace {} status {:?} on peers {pending:?}; last errors {last_errors:?}",
        dataspace.as_u64(),
        expected_status
    ))
}

fn ensure_publish_manifest_permission(client: &Client, dataspace: DataSpaceId) -> Result<()> {
    let grant_instruction = InstructionBox::from(Grant::account_permission(
        CanPublishSpaceDirectoryManifest { dataspace },
        client.account.clone(),
    ));
    let grant_tx = client.build_transaction([grant_instruction], Metadata::default());
    client
        .submit_transaction_blocking(&grant_tx)
        .map_err(|err| eyre!(err))
        .wrap_err("grant CanPublishSpaceDirectoryManifest permission")
        .map(|_| ())
}

async fn submit_and_wait_for_tx_approval(
    submitter: &Client,
    transaction: SignedTransaction,
    context: &str,
) -> Result<(Duration, Duration)> {
    let tx_hash = transaction.hash();
    let mut events = timeout(
        STATUS_WAIT_TIMEOUT,
        submitter.listen_for_events_async([TransactionEventFilter::default().for_hash(tx_hash)]),
    )
    .await
    .map_err(|_| eyre!("{context}: timed out opening transaction event stream"))??;

    // Give the event stream a brief head start to avoid missing early events.
    sleep(TX_SSE_HANDSHAKE_DELAY).await;

    let submit_started = Instant::now();
    let submitter_for_submit = submitter.clone();
    spawn_blocking(move || submitter_for_submit.submit_transaction(&transaction))
        .await
        .map_err(|err| eyre!("{context}: transaction submit task join error: {err}"))?
        .map_err(|err| eyre!("{context}: failed to submit transaction: {err}"))?;
    let submit_latency = submit_started.elapsed();

    let commit_apply_started = Instant::now();
    timeout(COMMITTED_TX_OUTCOME_TIMEOUT, async {
        loop {
            let Some(next) = events.next().await else {
                return Err(eyre!("{context}: transaction event stream closed"));
            };
            let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = next? else {
                continue;
            };
            match event.status() {
                TransactionStatus::Queued => continue,
                TransactionStatus::Approved => return Ok(()),
                TransactionStatus::Rejected(reason) => {
                    return Err(eyre!("{context}: transaction rejected: {reason}"));
                }
                TransactionStatus::Expired => {
                    return Err(eyre!("{context}: transaction expired"));
                }
            }
        }
    })
    .await
    .map_err(|_| eyre!("{context}: timed out waiting for transaction approval"))??;
    events.close().await;

    Ok((submit_latency, commit_apply_started.elapsed()))
}

fn benchmark_manifest(uaid: UniversalAccountId, issued_ms: u64) -> AssetPermissionManifest {
    AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace: BENCH_MANIFEST_DATASPACE,
        issued_ms,
        activation_epoch: BENCH_MANIFEST_ACTIVATION_EPOCH,
        expiry_epoch: None,
        entries: vec![ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(BENCH_MANIFEST_DATASPACE),
                program: None,
                method: None,
                asset: None,
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Numeric::from(1_u32)),
                window: AllowanceWindow::PerDay,
            }),
            notes: Some("runtime registration benchmark".to_owned()),
        }],
    }
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

fn emit_latency_stats(label: &str, samples: &[Duration]) {
    if let Some((min, avg, max)) = duration_min_avg_max(samples) {
        eprintln!(
            "[registration-perf] {label} min/avg/max = {}/{}/{}",
            format_duration(min),
            format_duration(avg),
            format_duration(max)
        );
    }
}

async fn post_lane_lifecycle_plan(
    http: &reqwest::Client,
    client: &Client,
    plan: &LaneLifecyclePlan,
) -> Result<()> {
    let url = client
        .torii_url
        .join("v2/nexus/lifecycle")
        .wrap_err("compose /v2/nexus/lifecycle URL")?;
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
        .wrap_err("send /v2/nexus/lifecycle request")?;
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
    manifest_uaid: String,
    submitter_peer_index: usize,
    submitter_height_delta: u64,
    cluster_height_delta: u64,
    lane_submit_latency: Duration,
    lane_commit_apply_latency: Duration,
    lane_all_peer_visibility_latency: Duration,
    lane_total_latency: Duration,
    manifest_publish_submit_latency: Duration,
    manifest_publish_commit_apply_latency: Duration,
    manifest_publish_all_peer_visibility_latency: Duration,
    manifest_publish_total_latency: Duration,
    manifest_revoke_submit_latency: Duration,
    manifest_revoke_commit_apply_latency: Duration,
    manifest_revoke_all_peer_visibility_latency: Duration,
    manifest_revoke_total_latency: Duration,
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
    let status_probe_client = network.peer().client();
    let submitter_peer_index = leader_or_highest_height_peer_index(network, &status_probe_client);
    let submitter = network.peers()[submitter_peer_index].client();
    let started = Instant::now();

    let uaid_seed = format!("runtime-registration-uaid-{lane_raw}");
    let manifest_uaid = UniversalAccountId::from_hash(Hash::new(uaid_seed.as_bytes()));
    let manifest_uaid_literal = manifest_uaid.to_string();
    let manifest = benchmark_manifest(
        manifest_uaid,
        u64::try_from(iteration).unwrap_or(0).saturating_add(
            u64::try_from(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
            )
            .unwrap_or(0),
        ),
    );

    let publish_started = Instant::now();
    let publish_tx = submitter.build_transaction(
        [InstructionBox::from(PublishSpaceDirectoryManifest {
            manifest: manifest.clone(),
        })],
        Metadata::default(),
    );
    let (manifest_publish_submit_latency, manifest_publish_commit_apply_latency) = rt
        .block_on(submit_and_wait_for_tx_approval(
            &submitter,
            publish_tx,
            "publish manifest transaction",
        ))
        .wrap_err("publish transaction did not reach Approved state")?;
    wait_for_manifest_status(
        &submitter,
        &manifest_uaid_literal,
        BENCH_MANIFEST_DATASPACE,
        UaidManifestStatus::Active,
        "wait for submitter manifest activation",
    )
    .wrap_err_with(|| {
        format!(
            "wait for UAID {} active manifest on submitter peer {}",
            manifest_uaid_literal, submitter_peer_index
        )
    })?;

    let publish_visibility_started = Instant::now();
    wait_for_all_peers_manifest_status(
        network,
        &manifest_uaid_literal,
        BENCH_MANIFEST_DATASPACE,
        UaidManifestStatus::Active,
        Some(submitter_peer_index),
        "wait for all-peer manifest activation",
    )
    .wrap_err_with(|| {
        format!(
            "wait for UAID {} active manifest visibility on all peers",
            manifest_uaid_literal
        )
    })?;
    let manifest_publish_all_peer_visibility_latency = publish_visibility_started.elapsed();
    let manifest_publish_total_latency = publish_started.elapsed();

    let revoke_started = Instant::now();
    let revoke_tx = submitter.build_transaction(
        [InstructionBox::from(RevokeSpaceDirectoryManifest {
            uaid: UniversalAccountId::from_hash(Hash::new(uaid_seed.as_bytes())),
            dataspace: BENCH_MANIFEST_DATASPACE,
            revoked_epoch: BENCH_MANIFEST_ACTIVATION_EPOCH.saturating_add(1),
            reason: Some("runtime registration benchmark revoke".to_owned()),
        })],
        Metadata::default(),
    );
    let (manifest_revoke_submit_latency, manifest_revoke_commit_apply_latency) = rt
        .block_on(submit_and_wait_for_tx_approval(
            &submitter,
            revoke_tx,
            "revoke manifest transaction",
        ))
        .wrap_err("revoke transaction did not reach Approved state")?;
    wait_for_manifest_status(
        &submitter,
        &manifest_uaid_literal,
        BENCH_MANIFEST_DATASPACE,
        UaidManifestStatus::Revoked,
        "wait for submitter manifest revocation",
    )
    .wrap_err_with(|| {
        format!(
            "wait for UAID {} revoked manifest on submitter peer {}",
            manifest_uaid_literal, submitter_peer_index
        )
    })?;

    let revoke_visibility_started = Instant::now();
    wait_for_all_peers_manifest_status(
        network,
        &manifest_uaid_literal,
        BENCH_MANIFEST_DATASPACE,
        UaidManifestStatus::Revoked,
        Some(submitter_peer_index),
        "wait for all-peer manifest revocation",
    )
    .wrap_err_with(|| {
        format!(
            "wait for UAID {} revoked manifest visibility on all peers",
            manifest_uaid_literal
        )
    })?;
    let manifest_revoke_all_peer_visibility_latency = revoke_visibility_started.elapsed();
    let manifest_revoke_total_latency = revoke_started.elapsed();

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

    let baseline_height = submitter
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))
        .wrap_err("fetch submitter baseline height")?
        .commit_qc
        .height;
    let baseline_cluster_height = network
        .peers()
        .iter()
        .map(|peer| {
            peer.client()
                .get_sumeragi_status_wire()
                .map(|status| status.commit_qc.height)
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);

    let lane_started = Instant::now();
    let lane_submit_started = Instant::now();
    rt.block_on(post_lane_lifecycle_plan(http, &submitter, &plan))
        .wrap_err_with(|| {
            format!("submit lifecycle plan to peer {submitter_peer_index} as leader target")
        })?;
    let lane_submit_latency = lane_submit_started.elapsed();

    let lane_apply_started = Instant::now();
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
    let lane_commit_apply_latency = lane_apply_started.elapsed();
    let submitter_height_delta = submitter_status
        .commit_qc
        .height
        .saturating_sub(baseline_height);

    let lane_visibility_started = Instant::now();
    wait_for_all_peers_lane_visibility(
        network,
        lane_id,
        Some(submitter_peer_index),
        "wait for all-peer lane visibility",
    )
    .wrap_err_with(|| format!("wait for lane {} visibility on all peers", lane_id.as_u32()))?;
    let lane_all_peer_visibility_latency = lane_visibility_started.elapsed();
    let cluster_height_delta = network
        .peers()
        .iter()
        .map(|peer| {
            peer.client()
                .get_sumeragi_status_wire()
                .map(|status| status.commit_qc.height)
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
        .saturating_sub(baseline_cluster_height);
    let lane_total_latency = lane_started.elapsed();

    let retire_plan = LaneLifecyclePlan {
        additions: Vec::new(),
        retire: vec![lane_id],
    };
    rt.block_on(post_lane_lifecycle_plan(http, &submitter, &retire_plan))
        .wrap_err_with(|| {
            format!(
                "submit lane retirement plan for lane {} to peer {submitter_peer_index}",
                lane_id.as_u32()
            )
        })?;
    wait_for_lane_absence(
        &submitter,
        lane_id,
        "wait for submitter lane retirement visibility",
    )
    .wrap_err_with(|| {
        format!(
            "wait for lane {} retirement on submitter peer {}",
            lane_id.as_u32(),
            submitter_peer_index
        )
    })?;
    wait_for_all_peers_lane_absence(
        network,
        lane_id,
        Some(submitter_peer_index),
        "wait for all-peer lane retirement visibility",
    )
    .wrap_err_with(|| format!("wait for lane {} retirement on all peers", lane_id.as_u32()))?;

    let total_latency = started.elapsed();
    Ok(RegistrationIterationMetrics {
        lane_id: lane_raw,
        manifest_uaid: manifest_uaid_literal,
        submitter_peer_index,
        submitter_height_delta,
        cluster_height_delta,
        lane_submit_latency,
        lane_commit_apply_latency,
        lane_all_peer_visibility_latency,
        lane_total_latency,
        manifest_publish_submit_latency,
        manifest_publish_commit_apply_latency,
        manifest_publish_all_peer_visibility_latency,
        manifest_publish_total_latency,
        manifest_revoke_submit_latency,
        manifest_revoke_commit_apply_latency,
        manifest_revoke_all_peer_visibility_latency,
        manifest_revoke_total_latency,
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
    let bench_iterations = benchmark_iterations();
    eprintln!("[registration-perf] report-only lane+dataspace metrics (no threshold gating)");

    let grant_probe_client = network.peer().client();
    let grant_peer_index = leader_or_highest_height_peer_index(&network, &grant_probe_client);
    let grant_client = network.peers()[grant_peer_index].client();
    ensure_publish_manifest_permission(&grant_client, BENCH_MANIFEST_DATASPACE).wrap_err_with(
        || {
            format!(
                "grant CanPublishSpaceDirectoryManifest on peer {grant_peer_index} for dataspace {}",
                BENCH_MANIFEST_DATASPACE.as_u64()
            )
        },
    )?;

    let mut lane_submit_samples = Vec::with_capacity(bench_iterations);
    let mut lane_commit_apply_samples = Vec::with_capacity(bench_iterations);
    let mut lane_visibility_samples = Vec::with_capacity(bench_iterations);
    let mut lane_total_samples = Vec::with_capacity(bench_iterations);
    let mut publish_submit_samples = Vec::with_capacity(bench_iterations);
    let mut publish_commit_apply_samples = Vec::with_capacity(bench_iterations);
    let mut publish_visibility_samples = Vec::with_capacity(bench_iterations);
    let mut publish_total_samples = Vec::with_capacity(bench_iterations);
    let mut revoke_submit_samples = Vec::with_capacity(bench_iterations);
    let mut revoke_commit_apply_samples = Vec::with_capacity(bench_iterations);
    let mut revoke_visibility_samples = Vec::with_capacity(bench_iterations);
    let mut revoke_total_samples = Vec::with_capacity(bench_iterations);
    let mut workflow_total_samples = Vec::with_capacity(bench_iterations);
    let mut submitter_height_delta_samples = Vec::with_capacity(bench_iterations);
    let mut cluster_height_delta_samples = Vec::with_capacity(bench_iterations);
    let mut passes = 0usize;
    let mut failure: Option<eyre::Report> = None;

    for iteration in 0..bench_iterations {
        match run_registration_iteration(&rt, &http, &network, iteration) {
            Ok(metrics) => {
                passes += 1;
                lane_submit_samples.push(metrics.lane_submit_latency);
                lane_commit_apply_samples.push(metrics.lane_commit_apply_latency);
                lane_visibility_samples.push(metrics.lane_all_peer_visibility_latency);
                lane_total_samples.push(metrics.lane_total_latency);
                publish_submit_samples.push(metrics.manifest_publish_submit_latency);
                publish_commit_apply_samples.push(metrics.manifest_publish_commit_apply_latency);
                publish_visibility_samples
                    .push(metrics.manifest_publish_all_peer_visibility_latency);
                publish_total_samples.push(metrics.manifest_publish_total_latency);
                revoke_submit_samples.push(metrics.manifest_revoke_submit_latency);
                revoke_commit_apply_samples.push(metrics.manifest_revoke_commit_apply_latency);
                revoke_visibility_samples.push(metrics.manifest_revoke_all_peer_visibility_latency);
                revoke_total_samples.push(metrics.manifest_revoke_total_latency);
                workflow_total_samples.push(metrics.total_latency);
                submitter_height_delta_samples.push(metrics.submitter_height_delta);
                cluster_height_delta_samples.push(metrics.cluster_height_delta);

                eprintln!(
                    "[registration-perf] iter={}/{} lane={} uaid={} submitter_peer={} submitter_height_delta={} cluster_height_delta={} lane_submit={} lane_commit/apply={} lane_visibility={} lane_total={} publish_submit={} publish_commit/apply={} publish_visibility={} publish_total={} revoke_submit={} revoke_commit/apply={} revoke_visibility={} revoke_total={} total={}",
                    iteration + 1,
                    bench_iterations,
                    metrics.lane_id,
                    metrics.manifest_uaid,
                    metrics.submitter_peer_index,
                    metrics.submitter_height_delta,
                    metrics.cluster_height_delta,
                    format_duration(metrics.lane_submit_latency),
                    format_duration(metrics.lane_commit_apply_latency),
                    format_duration(metrics.lane_all_peer_visibility_latency),
                    format_duration(metrics.lane_total_latency),
                    format_duration(metrics.manifest_publish_submit_latency),
                    format_duration(metrics.manifest_publish_commit_apply_latency),
                    format_duration(metrics.manifest_publish_all_peer_visibility_latency),
                    format_duration(metrics.manifest_publish_total_latency),
                    format_duration(metrics.manifest_revoke_submit_latency),
                    format_duration(metrics.manifest_revoke_commit_apply_latency),
                    format_duration(metrics.manifest_revoke_all_peer_visibility_latency),
                    format_duration(metrics.manifest_revoke_total_latency),
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

    let pass_rate = (passes as f64 / bench_iterations as f64) * 100.0;
    emit_latency_stats("lane submit latency", &lane_submit_samples);
    emit_latency_stats("lane commit/apply latency", &lane_commit_apply_samples);
    emit_latency_stats("lane all-peer visibility latency", &lane_visibility_samples);
    emit_latency_stats("lane workflow latency", &lane_total_samples);
    emit_latency_stats("manifest publish submit latency", &publish_submit_samples);
    emit_latency_stats(
        "manifest publish commit/apply latency",
        &publish_commit_apply_samples,
    );
    emit_latency_stats(
        "manifest publish all-peer visibility latency",
        &publish_visibility_samples,
    );
    emit_latency_stats("manifest publish workflow latency", &publish_total_samples);
    emit_latency_stats("manifest revoke submit latency", &revoke_submit_samples);
    emit_latency_stats(
        "manifest revoke commit/apply latency",
        &revoke_commit_apply_samples,
    );
    emit_latency_stats(
        "manifest revoke all-peer visibility latency",
        &revoke_visibility_samples,
    );
    emit_latency_stats("manifest revoke workflow latency", &revoke_total_samples);
    emit_latency_stats("total workflow latency", &workflow_total_samples);
    if !submitter_height_delta_samples.is_empty() {
        let min = *submitter_height_delta_samples.iter().min().unwrap_or(&0);
        let max = *submitter_height_delta_samples.iter().max().unwrap_or(&0);
        let avg = submitter_height_delta_samples.iter().sum::<u64>() as f64
            / submitter_height_delta_samples.len() as f64;
        eprintln!(
            "[registration-perf] submitter commit height delta min/avg/max = {min}/{avg:.2}/{max}"
        );
    }
    if !cluster_height_delta_samples.is_empty() {
        let min = *cluster_height_delta_samples.iter().min().unwrap_or(&0);
        let max = *cluster_height_delta_samples.iter().max().unwrap_or(&0);
        let avg = cluster_height_delta_samples.iter().sum::<u64>() as f64
            / cluster_height_delta_samples.len() as f64;
        eprintln!(
            "[registration-perf] cluster max commit height delta min/avg/max = {min}/{avg:.2}/{max}"
        );
        if max == 0 {
            eprintln!(
                "[registration-perf] note: lifecycle visibility completed without commit-height advancement on any peer (expected for control-plane /v2/nexus/lifecycle state mutation)"
            );
        }
    }
    eprintln!(
        "[registration-perf] iterations={} pass_rate={:.1}%",
        bench_iterations, pass_rate
    );

    if let Some(err) = failure {
        return Err(err);
    }
    ensure!(
        passes == bench_iterations,
        "registration benchmark did not complete all iterations"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{duration_min_avg_max, format_duration, parse_positive_usize_override};
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

    #[test]
    fn parse_positive_usize_override_uses_positive_input() {
        assert_eq!(parse_positive_usize_override(Some("8"), 5), 8);
        assert_eq!(parse_positive_usize_override(Some(" 3 "), 5), 3);
    }

    #[test]
    fn parse_positive_usize_override_falls_back_on_invalid_input() {
        assert_eq!(parse_positive_usize_override(None, 5), 5);
        assert_eq!(parse_positive_usize_override(Some("0"), 5), 5);
        assert_eq!(parse_positive_usize_override(Some("bad"), 5), 5);
        assert_eq!(parse_positive_usize_override(Some(""), 5), 5);
    }
}
