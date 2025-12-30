//! Deterministic parity suite for the Rust SoraFS orchestrator client.
//!
//! The suite executes a multi-provider fetch twice and asserts that the
//! resulting assignments, receipts, and provider reports remain identical.
//! Instrumentation captures concurrency limits so the resulting metrics can be
//! recorded in the GA parity report shared with other SDKs.

use std::{
    collections::{HashMap, HashSet},
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use blake3::hash as blake3_hash;
use norito::json::{self, Value};
use sorafs_car::{
    fixtures::MultiPeerFixture,
    multi_fetch::{ChunkResponse, FetchRequest},
};
use sorafs_orchestrator::{Orchestrator, OrchestratorConfig, PolicyStatus};

/// Maximum parallel chunk fetches enforced during the parity suite.
const MAX_PARALLEL_FETCHES: usize = 3;

#[test]
fn shared_fixture_matches_multi_peer_template() {
    let fixture = MultiPeerFixture::with_providers(4);
    let fixture_dir = workspace_root().join("fixtures/sorafs_orchestrator/multi_peer_parity_v1");
    assert!(
        fixture_dir.is_dir(),
        "fixture directory missing: {}",
        fixture_dir.display()
    );

    validate_plan(&fixture, &fixture_dir.join("plan.json"));
    validate_providers(&fixture, &fixture_dir.join("providers.json"));
    validate_telemetry(&fixture, &fixture_dir.join("telemetry.json"));
    validate_options(&fixture_dir.join("options.json"));
    validate_metadata(&fixture, &fixture_dir.join("metadata.json"));
}

#[tokio::test(flavor = "multi_thread")]
async fn rust_orchestrator_fetch_suite_is_deterministic() {
    let (first_report, first_metrics) = execute_parity_run().await;
    let (second_report, second_metrics) = execute_parity_run().await;

    assert_eq!(
        first_report, second_report,
        "fetch outcomes must remain deterministic across runs"
    );

    assert_eq!(
        first_metrics.total_bytes, second_metrics.total_bytes,
        "payload bytes fetched must match across runs"
    );
    assert_eq!(
        first_metrics.max_inflight, second_metrics.max_inflight,
        "peak in-flight fetch count should remain stable"
    );

    assert!(
        first_metrics.max_inflight <= MAX_PARALLEL_FETCHES,
        "max inflight ({}) must not exceed configured parallel limit ({MAX_PARALLEL_FETCHES})",
        first_metrics.max_inflight
    );
    assert!(
        second_metrics.max_inflight <= MAX_PARALLEL_FETCHES,
        "max inflight ({}) must not exceed configured parallel limit ({MAX_PARALLEL_FETCHES})",
        second_metrics.max_inflight
    );

    assert!(
        first_metrics.duration_ms <= 2_000,
        "parity run exceeded 2 s ({} ms)",
        first_metrics.duration_ms
    );
    assert!(
        second_metrics.duration_ms <= 2_000,
        "parity run exceeded 2 s ({} ms)",
        second_metrics.duration_ms
    );

    println!(
        "Rust orchestrator parity: duration_ms={} total_bytes={} max_inflight={} peak_reserved_bytes={}",
        first_metrics.duration_ms,
        first_metrics.total_bytes,
        first_metrics.max_inflight,
        first_metrics.peak_reserved_bytes
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParityReport {
    chunk_assignments: Vec<ParityChunk>,
    provider_reports: Vec<ParityProvider>,
    payload_digest: [u8; 32],
    chunk_digests: Vec<[u8; 32]>,
    total_chunks: usize,
    total_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParityChunk {
    chunk_index: usize,
    provider_id: String,
    attempts: u32,
    bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParityProvider {
    provider_id: String,
    successes: usize,
    failures: usize,
    disabled: bool,
}

impl ParityReport {
    fn from_outcome(
        outcome: &sorafs_car::multi_fetch::FetchOutcome,
        plan: &sorafs_car::CarBuildPlan,
    ) -> Self {
        let mut chunk_digests = Vec::with_capacity(plan.chunks.len());
        for chunk in &plan.chunks {
            chunk_digests.push(chunk.digest);
        }

        let chunk_assignments = outcome
            .chunk_receipts
            .iter()
            .map(|receipt| ParityChunk {
                chunk_index: receipt.chunk_index,
                provider_id: receipt.provider.as_str().to_owned(),
                attempts: receipt.attempts as u32,
                bytes: receipt.bytes,
            })
            .collect::<Vec<_>>();

        let mut provider_reports = outcome
            .provider_reports
            .iter()
            .map(|report| ParityProvider {
                provider_id: report.provider.id().as_str().to_owned(),
                successes: report.successes,
                failures: report.failures,
                disabled: report.disabled,
            })
            .collect::<Vec<_>>();
        provider_reports.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));

        let assembled_bytes = outcome.assemble_payload();
        let mut payload_digest = [0_u8; 32];
        payload_digest.copy_from_slice(blake3_hash(&assembled_bytes).as_bytes());

        Self {
            chunk_assignments,
            provider_reports,
            payload_digest,
            chunk_digests,
            total_chunks: plan.chunks.len(),
            total_bytes: assembled_bytes.len(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RunMetrics {
    duration_ms: u128,
    total_bytes: usize,
    max_inflight: usize,
    peak_reserved_bytes: usize,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_json_array(path: &Path) -> Vec<Value> {
    let data = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    json::from_str(&data).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", path.display());
    })
}

fn read_json_value(path: &Path) -> Value {
    let data = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    json::from_str(&data).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", path.display());
    })
}

fn validate_plan(fixture: &MultiPeerFixture, path: &Path) {
    let plan_json = read_json_array(path);
    assert_eq!(
        plan_json.len(),
        fixture.plan().chunks.len(),
        "plan chunk count mismatch"
    );
    for (index, (chunk, value)) in fixture
        .plan()
        .chunks
        .iter()
        .zip(plan_json.iter())
        .enumerate()
    {
        assert_eq!(
            Some(index as u64),
            value.get("chunk_index").and_then(Value::as_u64)
        );
        assert_eq!(
            Some(chunk.offset),
            value.get("offset").and_then(Value::as_u64)
        );
        assert_eq!(
            Some(chunk.length as u64),
            value.get("length").and_then(Value::as_u64)
        );
        assert_eq!(
            Some(hex::encode(chunk.digest)),
            value
                .get("digest_blake3")
                .and_then(Value::as_str)
                .map(str::to_owned)
        );
    }
}

fn validate_providers(fixture: &MultiPeerFixture, path: &Path) {
    let providers_json = read_json_array(path);
    assert_eq!(
        providers_json.len(),
        fixture.providers().len(),
        "provider count mismatch"
    );
    for (metadata, json) in fixture.providers().iter().zip(providers_json.iter()) {
        let provider_id = metadata
            .provider_id
            .as_deref()
            .expect("fixture provider id");
        assert_eq!(
            Some(provider_id),
            json.get("provider_id").and_then(Value::as_str)
        );
        let aliases_json = json
            .get("profile_aliases")
            .and_then(Value::as_array)
            .expect("profile aliases array");
        let alias_values = aliases_json
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(metadata.profile_aliases, alias_values);
        assert_eq!(
            metadata.availability.as_deref(),
            json.get("availability").and_then(Value::as_str)
        );
        assert_eq!(
            metadata.capability_names,
            json.get("capability_names")
                .and_then(Value::as_array)
                .map(|array| array
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<_>>())
                .expect("capability list")
        );
        let range = metadata
            .range_capability
            .as_ref()
            .expect("range capability");
        let range_json = json
            .get("range_capability")
            .and_then(Value::as_object)
            .expect("range capability json");
        assert_eq!(
            Some(range.max_chunk_span as u64),
            range_json.get("max_chunk_span").and_then(Value::as_u64)
        );
        assert_eq!(
            Some(range.min_granularity as u64),
            range_json.get("min_granularity").and_then(Value::as_u64)
        );
        assert_eq!(
            Some(range.supports_sparse_offsets),
            range_json
                .get("supports_sparse_offsets")
                .and_then(Value::as_bool)
        );
        assert_eq!(
            Some(range.requires_alignment),
            range_json
                .get("requires_alignment")
                .and_then(Value::as_bool)
        );
        assert_eq!(
            Some(range.supports_merkle_proof),
            range_json
                .get("supports_merkle_proof")
                .and_then(Value::as_bool)
        );
        let stream = metadata.stream_budget.as_ref().expect("stream budget");
        let stream_json = json
            .get("stream_budget")
            .and_then(Value::as_object)
            .expect("stream budget json");
        assert_eq!(
            Some(stream.max_in_flight as u64),
            stream_json.get("max_in_flight").and_then(Value::as_u64)
        );
        assert_eq!(
            Some(stream.max_bytes_per_sec),
            stream_json.get("max_bytes_per_sec").and_then(Value::as_u64)
        );
        assert_eq!(
            stream.burst_bytes,
            stream_json.get("burst_bytes").and_then(Value::as_u64)
        );
        assert_eq!(
            metadata.refresh_deadline,
            json.get("refresh_deadline").and_then(Value::as_u64)
        );
        assert_eq!(
            metadata.expires_at,
            json.get("expires_at").and_then(Value::as_u64)
        );
        assert_eq!(
            metadata.ttl_secs,
            json.get("ttl_secs").and_then(Value::as_u64)
        );
    }
}

fn validate_telemetry(fixture: &MultiPeerFixture, path: &Path) {
    let telemetry_json = read_json_array(path);
    #[derive(Debug, Clone)]
    struct TelemetryRow {
        qos_score: f64,
        latency_p95_ms: f64,
        failure_rate_ewma: f64,
        token_health: f64,
        staking_weight: f64,
        penalty: bool,
        last_updated: u64,
    }
    let mut expected = telemetry_json
        .into_iter()
        .map(|entry| {
            let provider_id = entry
                .get("provider_id")
                .and_then(Value::as_str)
                .expect("provider_id");
            let row = TelemetryRow {
                qos_score: entry
                    .get("qos_score")
                    .and_then(Value::as_f64)
                    .expect("qos_score"),
                latency_p95_ms: entry
                    .get("latency_p95_ms")
                    .and_then(Value::as_f64)
                    .expect("latency_p95_ms"),
                failure_rate_ewma: entry
                    .get("failure_rate_ewma")
                    .and_then(Value::as_f64)
                    .expect("failure_rate_ewma"),
                token_health: entry
                    .get("token_health")
                    .and_then(Value::as_f64)
                    .expect("token_health"),
                staking_weight: entry
                    .get("staking_weight")
                    .and_then(Value::as_f64)
                    .expect("staking_weight"),
                penalty: entry
                    .get("penalty")
                    .and_then(Value::as_bool)
                    .expect("penalty flag"),
                last_updated: entry
                    .get("last_updated_unix")
                    .and_then(Value::as_u64)
                    .expect("last_updated"),
            };
            (provider_id.to_string(), row)
        })
        .collect::<std::collections::HashMap<_, TelemetryRow>>();
    for record in fixture.telemetry().iter() {
        let expected_row = expected
            .remove(&record.provider_id)
            .expect("telemetry provider present");
        assert_eq!(
            record.qos_score.expect("fixture qos"),
            expected_row.qos_score
        );
        assert_eq!(
            record.latency_p95_ms.expect("fixture latency"),
            expected_row.latency_p95_ms
        );
        assert_eq!(
            record.failure_rate_ewma.expect("fixture failure_rate_ewma"),
            expected_row.failure_rate_ewma
        );
        assert_eq!(
            record.token_health.expect("fixture token_health"),
            expected_row.token_health
        );
        assert_eq!(
            record.staking_weight.expect("fixture staking_weight"),
            expected_row.staking_weight
        );
        assert_eq!(record.penalty, expected_row.penalty);
        assert_eq!(
            record
                .last_updated_unix
                .expect("fixture telemetry last_updated"),
            expected_row.last_updated
        );
    }
    assert!(
        expected.is_empty(),
        "telemetry JSON contains extra providers: {:?}",
        expected.keys().collect::<Vec<_>>()
    );
}

fn validate_options(path: &Path) {
    let options = read_json_value(path);
    assert_eq!(Some(3), options.get("max_parallel").and_then(Value::as_u64));
    assert_eq!(Some(3), options.get("max_peers").and_then(Value::as_u64));
    assert_eq!(Some(5), options.get("retry_budget").and_then(Value::as_u64));
    assert_eq!(
        Some(2),
        options
            .get("provider_failure_threshold")
            .and_then(Value::as_u64)
    );
    assert_eq!(
        Some(true),
        options.get("verify_digests").and_then(Value::as_bool)
    );
    assert_eq!(
        Some(true),
        options.get("verify_lengths").and_then(Value::as_bool)
    );
    assert_eq!(
        Some("fixture"),
        options.get("telemetry_region").and_then(Value::as_str)
    );
    let scoreboard = options
        .get("scoreboard")
        .and_then(Value::as_object)
        .expect("scoreboard block");
    assert_eq!(
        Some(1_725_000_000),
        scoreboard.get("now_unix_secs").and_then(Value::as_u64)
    );
}

fn validate_metadata(fixture: &MultiPeerFixture, path: &Path) {
    let metadata = read_json_value(path);
    assert_eq!(
        Some(1_725_000_000),
        metadata.get("now_unix_secs").and_then(Value::as_u64)
    );
    assert_eq!(
        Some(fixture.plan().chunks.len() as u64),
        metadata.get("chunk_count").and_then(Value::as_u64)
    );
    assert_eq!(
        Some(fixture.providers().len() as u64),
        metadata.get("provider_count").and_then(Value::as_u64)
    );
    assert_eq!(
        Some(fixture.payload().len() as u64),
        metadata.get("payload_bytes").and_then(Value::as_u64)
    );
}

async fn execute_parity_run() -> (ParityReport, RunMetrics) {
    let fixture = MultiPeerFixture::with_providers(4);

    let mut config = OrchestratorConfig::default();
    config.scoreboard.now_unix_secs = fixture.now_unix_secs();
    config.scoreboard.telemetry_grace_period = Duration::from_hours(1);
    config.scoreboard.latency_cap_ms = 2_000;
    config.fetch.per_chunk_retry_limit = Some(5);
    config.fetch.provider_failure_threshold = 2;
    config.fetch.global_parallel_limit = Some(MAX_PARALLEL_FETCHES);
    config.max_providers = NonZeroUsize::new(3);
    config.telemetry_region = Some("fixture".into());

    let orchestrator = Orchestrator::new(config);
    let scoreboard = orchestrator
        .build_scoreboard(fixture.plan(), fixture.providers(), fixture.telemetry())
        .expect("scoreboard build");

    let provider_payloads = fixture
        .providers()
        .iter()
        .zip(fixture.provider_payloads())
        .filter_map(|(metadata, payload)| metadata.provider_id.as_ref().map(|id| (id, payload)))
        .map(|(id, payload)| (id.clone(), Arc::new(payload.clone())))
        .collect::<HashMap<String, Arc<Vec<u8>>>>();
    let provider_payloads = Arc::new(provider_payloads);

    let transient_failures = Arc::new(Mutex::new(HashSet::new()));
    let corrupt_once = Arc::new(Mutex::new(HashSet::new()));
    let tracker = Arc::new(InflightTracker::new());
    let total_bytes = Arc::new(AtomicUsize::new(0));

    let fetcher = {
        let provider_payloads = Arc::clone(&provider_payloads);
        let transient_failures = Arc::clone(&transient_failures);
        let corrupt_once = Arc::clone(&corrupt_once);
        let tracker = Arc::clone(&tracker);
        let total_bytes = Arc::clone(&total_bytes);
        move |request: FetchRequest| {
            let provider_payloads = Arc::clone(&provider_payloads);
            let transient_failures = Arc::clone(&transient_failures);
            let corrupt_once = Arc::clone(&corrupt_once);
            let tracker = Arc::clone(&tracker);
            let total_bytes = Arc::clone(&total_bytes);
            async move {
                let _guard = tracker.enter();
                let provider_id = request.provider.id().as_str().to_owned();
                let chunk_index = request.spec.chunk_index;

                if provider_id.ends_with('1') {
                    let mut guard = transient_failures.lock().expect("lock transient failures");
                    if guard.insert((provider_id.clone(), chunk_index)) {
                        return Err(FetchError("simulated transport error"));
                    }
                }

                let payload = provider_payloads
                    .get(&provider_id)
                    .expect("fixture provider present");
                let start = request.spec.offset as usize;
                let end = start + request.spec.length as usize;
                let mut bytes = payload[start..end].to_vec();

                if provider_id.ends_with('2') {
                    let mut guard = corrupt_once.lock().expect("lock corruption flag");
                    if guard.insert((provider_id.clone(), chunk_index)) {
                        bytes[0] ^= 0xFF;
                    }
                }

                total_bytes.fetch_add(bytes.len(), Ordering::Relaxed);
                Ok::<ChunkResponse, FetchError>(ChunkResponse::new(bytes))
            }
        }
    };

    let started = Instant::now();
    let session = orchestrator
        .fetch_with_scoreboard(fixture.plan(), &scoreboard, fetcher)
        .await
        .expect("fetch outcome");
    let duration = started.elapsed();

    assert_eq!(
        session.policy_report.status,
        PolicyStatus::NotApplicable,
        "fixture does not enforce a PQ policy"
    );

    let run_metrics = RunMetrics {
        duration_ms: duration.as_millis(),
        total_bytes: total_bytes.load(Ordering::Relaxed),
        max_inflight: tracker.max(),
        peak_reserved_bytes: tracker.max()
            * usize::try_from(fixture.max_chunk_length()).expect("chunk lengths fit usize"),
    };
    let report = ParityReport::from_outcome(&session.outcome, fixture.plan());

    assert_eq!(
        report.total_bytes,
        fixture.payload().len(),
        "assembled payload must match fixture payload size"
    );

    (report, run_metrics)
}

#[derive(Debug)]
struct FetchError(&'static str);

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for FetchError {}

#[derive(Debug, Default)]
struct InflightTracker {
    current: AtomicUsize,
    max: AtomicUsize,
}

impl InflightTracker {
    fn new() -> Self {
        Self {
            current: AtomicUsize::new(0),
            max: AtomicUsize::new(0),
        }
    }

    fn enter(self: &Arc<Self>) -> InflightGuard {
        let active = self.current.fetch_add(1, Ordering::AcqRel) + 1;
        self.update_max(active);
        InflightGuard {
            tracker: Arc::clone(self),
        }
    }

    fn update_max(&self, value: usize) {
        let mut observed = self.max.load(Ordering::Relaxed);
        while value > observed {
            match self
                .max
                .compare_exchange(observed, value, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(next) => observed = next,
            }
        }
    }

    fn leave(&self) {
        self.current.fetch_sub(1, Ordering::AcqRel);
    }

    fn max(&self) -> usize {
        self.max.load(Ordering::Relaxed)
    }
}

struct InflightGuard {
    tracker: Arc<InflightTracker>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.tracker.leave();
    }
}
