//! Conformance harness for the `SoraFS` gateway trustless profile (SF-5a).
//!
//! This module spins up fixture-backed HTTP adapters and runs the same manifest /
//! proof verification flow described in `docs/source/sorafs_gateway_profile.md`.
//! The adapters mimic the responses a real gateway would return so the harness
//! can validate behaviour deterministically during CI.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher;
use eyre::{Result as EyreResult, WrapErr, eyre};
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::account::AccountAddress;
use norito::{
    decode_from_bytes,
    json::{self, Value},
    to_bytes,
};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarWriteError, CarWriter, FilePlan, ingest_single_file,
    verifier::{CarVerifier, CarVerifyError},
};
use sorafs_manifest::{
    CouncilSignature, DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1,
    ManifestValidationError, PinPolicy, StorageClass, chunker_registry,
    por::{
        PorChallengeV1, PorChallengeValidationError, PorProofV1, PorProofValidationError,
        derive_challenge_id, derive_challenge_seed,
    },
    validation::{PinPolicyConstraints, validate_manifest},
};

use crate::sorafs_gateway_capability_refusal::{self, CapabilityRefusalScenario};

/// Describes a single replay scenario (positive or negative).
#[derive(Debug, Clone)]
struct ReplayScenario {
    id: &'static str,
    description: &'static str,
    expected_status: u16,
    expected_reason: ScenarioOutcome,
}

/// Result classification returned after replaying a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioOutcome {
    /// Scenario completed successfully and matched governance expectations.
    Success,
    /// Scenario was rejected with a deterministic refusal.
    Refusal,
    /// Scenario triggered an unexpected transport or runtime error.
    Error,
}

const MANIFEST_TO_FILE: &str = "manifest_v1.to";
const MANIFEST_JSON_FILE: &str = "manifest_v1.json";
const CHALLENGE_TO_FILE: &str = "challenge_v1.to";
const CHALLENGE_JSON_FILE: &str = "challenge_v1.json";
const PROOF_TO_FILE: &str = "proof_v1.to";
const PROOF_JSON_FILE: &str = "proof_v1.json";
const PAYLOAD_BIN_FILE: &str = "payload.bin";
const PAYLOAD_DIGEST_FILE: &str = "payload.blake3";
const CAR_BIN_FILE: &str = "gateway.car";
const CAR_DIGEST_FILE: &str = "gateway_car.blake3";
const SCENARIOS_JSON_FILE: &str = "scenarios.json";

/// Scenario mix used by the deterministic load harness.
const LOAD_TEST_SCENARIOS: &[&str] = &["A2", "A4", "A3", "B4", "B5", "B6"];

/// Canonical profile version covered by the conformance harness.
pub const PROFILE_VERSION: &str = "sf1";

/// Version identifier for the published `SoraFS` gateway fixture bundle.
pub const FIXTURE_VERSION: &str = "1.0.0";

/// Release timestamp (seconds since UNIX epoch) associated with `FIXTURE_VERSION`.
pub const FIXTURE_RELEASE_UNIX: u64 = 1_770_854_400;

/// Expected BLAKE3 digest (hex) for the canonical fixture bundle.
fn outcome_label(outcome: ScenarioOutcome) -> &'static str {
    match outcome {
        ScenarioOutcome::Success => "success",
        ScenarioOutcome::Refusal => "refusal",
        ScenarioOutcome::Error => "error",
    }
}

/// Aggregate view of a single scenario execution.
#[derive(Debug, Clone)]
pub struct ScenarioReport {
    /// Scenario identifier (matches the fixture directory).
    pub id: &'static str,
    /// Human-readable description summarising the scenario.
    pub description: &'static str,
    /// Expected HTTP status code returned by the gateway.
    pub expected_status: u16,
    /// Status code observed during the run.
    pub observed_status: u16,
    /// Expected semantic outcome (success/refusal/error).
    pub expected_outcome: ScenarioOutcome,
    /// Outcome classification reported by the harness.
    pub observed_outcome: ScenarioOutcome,
    /// Structured refusal payload when the scenario produced a deterministic refusal.
    pub refusal: Option<RefusalDetail>,
}

impl ScenarioReport {
    /// Returns `true` when the replay matched the status code and outcome.
    pub fn passed(&self) -> bool {
        self.expected_status == self.observed_status
            && self.expected_outcome == self.observed_outcome
    }

    fn to_json(&self) -> Value {
        let mut map = json::Map::new();
        map.insert("id".into(), Value::from(self.id));
        map.insert("description".into(), Value::from(self.description));
        map.insert(
            "expected_status".into(),
            Value::from(u64::from(self.expected_status)),
        );
        map.insert(
            "observed_status".into(),
            Value::from(u64::from(self.observed_status)),
        );
        map.insert(
            "expected_outcome".into(),
            Value::from(outcome_label(self.expected_outcome)),
        );
        map.insert(
            "observed_outcome".into(),
            Value::from(outcome_label(self.observed_outcome)),
        );
        map.insert("passed".into(), Value::from(self.passed()));
        if let Some(refusal) = &self.refusal {
            map.insert("refusal".into(), refusal.to_json());
        }
        Value::Object(map)
    }
}

/// Canonical refusal payload captured during replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalDetail {
    /// HTTP status code emitted for the refusal.
    pub status: u16,
    /// Machine-readable refusal code (`error` field).
    pub error: String,
    /// Human-readable explanation (`reason` field).
    pub reason: String,
    /// Structured metadata describing the refusal.
    pub details: Value,
    /// Canonical request payload associated with the refusal, when available.
    pub request: Option<Value>,
    /// Canonical response payload associated with the refusal, when available.
    pub response: Option<Value>,
    /// Canonical gateway mutation payload associated with the refusal, when available.
    pub gateway: Option<Value>,
}

impl RefusalDetail {
    fn to_json(&self) -> Value {
        let mut map = json::Map::new();
        map.insert("status".into(), Value::from(u64::from(self.status)));
        map.insert("error".into(), Value::from(self.error.clone()));
        map.insert("reason".into(), Value::from(self.reason.clone()));
        map.insert("details".into(), self.details.clone());
        if let Some(request) = &self.request {
            map.insert("request".into(), request.clone());
        }
        if let Some(response) = &self.response {
            map.insert("response".into(), response.clone());
        }
        if let Some(gateway) = &self.gateway {
            map.insert("gateway".into(), gateway.clone());
        }
        Value::Object(map)
    }
}

impl From<&CapabilityRefusalScenario> for RefusalDetail {
    fn from(scenario: &CapabilityRefusalScenario) -> Self {
        Self {
            status: scenario.status,
            error: scenario.error.clone(),
            reason: scenario.reason.clone(),
            details: scenario.details.clone(),
            request: scenario.request.clone(),
            response: scenario.response.clone(),
            gateway: scenario.gateway.clone(),
        }
    }
}

/// Summary produced by the deterministic load test.
#[derive(Debug, Clone)]
pub struct LoadTestReport {
    /// Total number of HTTP requests issued during the run.
    pub total_requests: u64,
    /// Total wall-clock time spent executing the load run.
    pub elapsed: Duration,
    /// Per-scenario statistics collected from the run.
    pub scenario_stats: BTreeMap<String, ScenarioStats>,
}

impl LoadTestReport {
    fn all_successful(&self) -> bool {
        self.scenario_stats.values().all(|stats| stats.error == 0)
    }

    fn to_json_value(&self) -> Value {
        let mut map = json::Map::new();
        map.insert("total_requests".into(), Value::from(self.total_requests));
        map.insert(
            "elapsed_seconds".into(),
            Value::from(self.elapsed.as_secs_f64()),
        );
        let scenarios = self
            .scenario_stats
            .iter()
            .map(|(id, stats)| {
                let mut entry = json::Map::new();
                entry.insert("id".into(), Value::from(id.clone()));
                entry.extend(stats.to_json_map());
                Value::Object(entry)
            })
            .collect();
        map.insert("scenarios".into(), Value::Array(scenarios));
        Value::Object(map)
    }
}

/// Aggregated statistics for a replay scenario gathered during load tests.
#[derive(Debug, Clone, Default)]
pub struct ScenarioStats {
    /// Total number of requests issued for the scenario.
    pub total: u64,
    /// Number of successful responses observed.
    pub success: u64,
    /// Number of deterministic refusals observed.
    pub refusal: u64,
    /// Number of error responses observed.
    pub error: u64,
    durations: Vec<Duration>,
}

impl ScenarioStats {
    fn record(&mut self, result: &GatewayResult, duration: Duration) {
        self.total += 1;
        match result.outcome {
            ScenarioOutcome::Success => self.success += 1,
            ScenarioOutcome::Refusal => self.refusal += 1,
            ScenarioOutcome::Error => self.error += 1,
        }
        self.durations.push(duration);
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn percentile_duration(&self, percentile: f64) -> Option<Duration> {
        if self.durations.is_empty() {
            return None;
        }
        let mut values: Vec<u128> = self.durations.iter().map(Duration::as_micros).collect();
        values.sort_unstable();
        let len = values.len();
        let position = if len == 1 {
            0
        } else {
            let rank = (percentile.clamp(0.0, 100.0) / 100.0) * ((len - 1) as f64);
            rank.round() as usize
        };
        let micros = values[position].min(u128::from(u64::MAX));
        let micros = u64::try_from(micros).expect("value bounded to u64::MAX prior to conversion");
        Some(Duration::from_micros(micros))
    }

    fn percentile_millis(&self, percentile: f64) -> Option<f64> {
        self.percentile_duration(percentile)
            .map(|duration| duration.as_secs_f64() * 1_000.0)
    }

    fn to_json_map(&self) -> json::Map {
        let mut map = json::Map::new();
        map.insert("total".into(), Value::from(self.total));
        map.insert("success".into(), Value::from(self.success));
        map.insert("refusal".into(), Value::from(self.refusal));
        map.insert("error".into(), Value::from(self.error));
        map.insert(
            "p50_ms".into(),
            self.percentile_millis(50.0)
                .map_or(Value::Null, Value::from),
        );
        map.insert(
            "p95_ms".into(),
            self.percentile_millis(95.0)
                .map_or(Value::Null, Value::from),
        );
        map.insert(
            "p99_ms".into(),
            self.percentile_millis(99.0)
                .map_or(Value::Null, Value::from),
        );
        map
    }
}

/// Final report materialised by the conformance harness.
#[derive(Debug, Clone)]
pub struct SuiteReport {
    /// Profile version identifier (e.g., `sf1`).
    pub profile_version: &'static str,
    /// Hex-encoded digest of the fixture bundle used for replay.
    pub fixtures_digest_hex: String,
    /// Load parameters applied to the deterministic stress test.
    pub load_profile: LoadProfile,
    /// Measured statistics captured during the load test.
    pub load_report: LoadTestReport,
    /// Optional target hostname used for the gateway under test.
    pub gateway_target: Option<String>,
    /// Scenario-level results in the order they were executed.
    pub scenarios: Vec<ScenarioReport>,
}

impl SuiteReport {
    /// Returns `true` when replay and load tests succeeded with no discrepancies.
    pub fn all_passed(&self) -> bool {
        self.scenarios.iter().all(ScenarioReport::passed) && self.load_report.all_successful()
    }

    /// Total number of replay scenarios included in the report.
    pub fn scenario_count(&self) -> usize {
        self.scenarios.len()
    }

    /// Look up a scenario report by identifier.
    pub fn scenario(&self, id: &str) -> Option<&ScenarioReport> {
        self.scenarios.iter().find(|scenario| scenario.id == id)
    }

    /// Render the report as a Norito JSON value used by the attestation bundle.
    pub fn to_json_value(&self) -> Value {
        let mut map = json::Map::new();
        map.insert("profile_version".into(), Value::from(self.profile_version));
        map.insert(
            "fixtures_digest_hex".into(),
            Value::from(self.fixtures_digest_hex.clone()),
        );
        let gateway_target = self
            .gateway_target
            .as_ref()
            .map_or(Value::Null, |target| Value::from(target.clone()));
        map.insert("gateway_target".into(), gateway_target);
        map.insert(
            "load_profile".into(),
            load_profile_to_value(self.load_profile),
        );
        map.insert("load_report".into(), self.load_report.to_json_value());
        let scenarios = self
            .scenarios
            .iter()
            .map(ScenarioReport::to_json)
            .collect::<Vec<_>>();
        map.insert("scenarios".into(), Value::Array(scenarios));
        Value::Object(map)
    }
}

/// Artifact generated after a successful conformance run.
#[derive(Debug, Clone)]
pub struct AttestationBundle {
    /// JSON serialization of the suite report (pretty formatted).
    pub report_json: Vec<u8>,
    /// Envelope bytes containing the signed manifest and bundle metadata.
    pub envelope_bytes: Vec<u8>,
    /// Human-readable summary emitted for operators.
    pub summary_text: String,
}

fn load_profile_to_value(profile: LoadProfile) -> Value {
    let mut map = json::Map::new();
    map.insert(
        "concurrent_streams".into(),
        Value::from(profile.concurrent_streams as u64),
    );
    map.insert(
        "max_duration_seconds".into(),
        Value::from(profile.max_duration.as_secs()),
    );
    Value::Object(map)
}

/// Parameters controlling load-test execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadProfile {
    /// Number of concurrent request streams generated during the load test.
    pub concurrent_streams: usize,
    /// Maximum duration of the deterministic load run.
    pub max_duration: Duration,
}

/// Represents a single wave in the deterministic load schedule.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
struct LoadWave {
    start_offset: Duration,
    end_offset: Duration,
    concurrency: usize,
}

/// Deterministic scheduler that converts a [`LoadProfile`] into concrete waves.
#[derive(Debug, Clone, Copy)]
struct DeterministicLoadGenerator {
    profile: LoadProfile,
    wave_span: Duration,
}

impl DeterministicLoadGenerator {
    /// Construct a generator that divides the load window into evenly spaced waves.
    fn from_profile(profile: LoadProfile) -> Self {
        let max_secs = profile.max_duration.as_secs();
        let wave_count = match max_secs {
            0 => 1,
            secs => secs.checked_div(30).map_or(1, |waves| waves.clamp(1, 16)),
        };
        let normalized_waves = wave_count.max(1);
        let wave_span_secs = if max_secs == 0 {
            1
        } else {
            max_secs.div_ceil(normalized_waves).max(1)
        };
        Self {
            profile,
            wave_span: Duration::from_secs(wave_span_secs),
        }
    }

    /// Produce the load schedule. The last wave may be shorter to respect the max duration.
    fn schedule(&self) -> Vec<LoadWave> {
        let mut waves = Vec::new();
        let mut start = Duration::ZERO;
        let max_duration = self.profile.max_duration;
        if max_duration.is_zero() {
            waves.push(LoadWave {
                start_offset: Duration::ZERO,
                end_offset: Duration::ZERO,
                concurrency: self.profile.concurrent_streams,
            });
            return waves;
        }

        while start < max_duration {
            let mut end = start + self.wave_span;
            if end > max_duration {
                end = max_duration;
            }
            waves.push(LoadWave {
                start_offset: start,
                end_offset: end,
                concurrency: self.profile.concurrent_streams,
            });
            if end == max_duration {
                break;
            }
            start = end;
        }
        waves
    }
}

/// Canonical gateway fixture bundle shared across tests.
#[derive(Debug, Clone)]
struct FixtureBundle {
    manifest: ManifestV1,
    challenge: PorChallengeV1,
    proof: PorProofV1,
    car_bytes: Vec<u8>,
    plan: CarBuildPlan,
    payload: Vec<u8>,
}

/// Metadata snapshot describing a generated gateway fixture bundle.
#[derive(Debug, Clone)]
pub struct FixtureMetadata {
    /// Semantic version string of the published bundle.
    pub version: String,
    /// Profile identifier validated by the bundle.
    pub profile_version: String,
    /// Release timestamp (seconds since UNIX epoch).
    pub released_at_unix: u64,
    /// Aggregate BLAKE3 digest across manifest, challenge, proof, CAR, and payload.
    pub fixtures_digest_blake3_hex: String,
    /// BLAKE3 digest of the canonical manifest bytes.
    pub manifest_blake3_hex: String,
    /// BLAKE3 digest of the deterministic payload (`payload.bin`).
    pub payload_blake3_hex: String,
    /// BLAKE3 digest of the generated CAR archive (`gateway.car`).
    pub car_blake3_hex: String,
}

fn generate_fixture_bundle() -> FixtureBundle {
    let payload = sample_payload_bytes();
    let summary = ingest_single_file(&payload).expect("fixture ingestion");
    let plan = summary.plan.clone();

    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .expect("fixture car writer")
        .write_to(&mut car_bytes)
        .expect("fixture car write");

    let mut car_digest = [0u8; 32];
    car_digest.copy_from_slice(blake3::hash(&car_bytes).as_bytes());
    let root_cid = stats
        .root_cids
        .first()
        .cloned()
        .unwrap_or_else(|| b"bafysorafsmanifest".to_vec());

    let mut manifest = ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 3,
            storage_class: StorageClass::Hot,
            retention_epoch: 86_400,
        })
        .governance(GovernanceProofs {
            council_signatures: vec![CouncilSignature {
                signer: [0x11; 32],
                signature: vec![0x22; 64],
            }],
        })
        .build()
        .expect("manifest construction");

    let descriptor = chunker_registry::default_descriptor();
    let alias_list: Vec<String> = descriptor
        .aliases
        .iter()
        .copied()
        .map(str::to_string)
        .collect();
    let canonical_alias = alias_list.first().cloned().unwrap_or_else(|| {
        format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )
    });
    manifest.chunking.aliases = alias_list;

    let mut challenge: PorChallengeV1 = decode_from_bytes(include_bytes!(
        "../../fixtures/sorafs_manifest/por/challenge_v1.to"
    ))
    .expect("decode PoR challenge");
    let mut proof: PorProofV1 = decode_from_bytes(include_bytes!(
        "../../fixtures/sorafs_manifest/por/proof_v1.to"
    ))
    .expect("decode PoR proof");
    challenge.chunking_profile.clone_from(&canonical_alias);

    let manifest_digest = manifest.digest().expect("manifest digest");
    let digest_bytes = manifest_digest.as_bytes();
    challenge.manifest_digest.copy_from_slice(digest_bytes);
    proof.manifest_digest.copy_from_slice(digest_bytes);
    let derived_seed = derive_challenge_seed(
        &challenge.drand_randomness,
        challenge.vrf_output.as_ref(),
        &challenge.manifest_digest,
        challenge.epoch_id,
    );
    challenge.seed = derived_seed;
    let derived_challenge_id = derive_challenge_id(
        &challenge.seed,
        &challenge.manifest_digest,
        &challenge.provider_id,
        challenge.epoch_id,
        challenge.drand_round,
    );
    challenge.challenge_id = derived_challenge_id;
    proof.challenge_id = derived_challenge_id;
    proof.provider_id = challenge.provider_id;

    FixtureBundle {
        manifest,
        challenge,
        proof,
        car_bytes,
        plan,
        payload,
    }
}

fn metadata_from_bundle(bundle: &FixtureBundle) -> FixtureMetadata {
    let manifest_bytes =
        to_bytes(&bundle.manifest).expect("serialize manifest fixture deterministically");
    let fixtures_digest = fixtures_digest(bundle).to_hex().to_string();
    let manifest_digest = blake3::hash(&manifest_bytes).to_hex().to_string();
    let payload_digest = blake3::hash(&bundle.payload).to_hex().to_string();
    let car_digest = blake3::hash(&bundle.car_bytes).to_hex().to_string();

    FixtureMetadata {
        version: sorafs_manifest::gateway_fixture::SORAFS_GATEWAY_FIXTURE_VERSION.to_string(),
        profile_version: sorafs_manifest::gateway_fixture::SORAFS_GATEWAY_PROFILE_VERSION
            .to_string(),
        released_at_unix: sorafs_manifest::gateway_fixture::SORAFS_GATEWAY_FIXTURE_RELEASE_UNIX,
        fixtures_digest_blake3_hex: fixtures_digest,
        manifest_blake3_hex: manifest_digest,
        payload_blake3_hex: payload_digest,
        car_blake3_hex: car_digest,
    }
}

fn fixtures_digest(bundle: &FixtureBundle) -> blake3::Hash {
    let mut hasher = Hasher::new();
    hasher.update(
        &to_bytes(&bundle.manifest).expect("serialize manifest fixture for digest computation"),
    );
    hasher.update(
        &to_bytes(&bundle.challenge).expect("serialize challenge fixture for digest computation"),
    );
    hasher
        .update(&to_bytes(&bundle.proof).expect("serialize proof fixture for digest computation"));
    hasher.update(&bundle.car_bytes);
    hasher.update(&bundle.payload);
    hasher.finalize()
}

fn sample_payload_bytes() -> Vec<u8> {
    const LEN: usize = 1_048_576;
    let mut payload = Vec::with_capacity(LEN);
    let mut state: u64 = 0x5A_EC_D4_BA;
    for _ in 0..LEN {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let byte = u8::try_from((state >> 24) & 0xFF).expect("masked to byte");
        payload.push(byte);
    }
    payload
}

fn canonical_fixture_bundle() -> &'static FixtureBundle {
    static BUNDLE: OnceLock<FixtureBundle> = OnceLock::new();
    BUNDLE.get_or_init(generate_fixture_bundle)
}

fn fixture_bundle() -> FixtureBundle {
    canonical_fixture_bundle().clone()
}

fn canonical_fixtures_digest_hex() -> String {
    fixtures_digest(canonical_fixture_bundle())
        .to_hex()
        .to_string()
}

/// Exclusive-end byte range used by range streaming scenarios.
#[derive(Debug, Clone, Copy)]
struct ByteRange {
    start: u64,
    end: u64,
}

impl ByteRange {
    fn new(start: u64, end: u64) -> Self {
        assert!(end >= start, "range end must be >= start");
        Self { start, end }
    }

    fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// Harness state backed by the canonical gateway fixture bundle.
#[derive(Debug, Clone)]
pub struct HarnessContext {
    scenarios: Vec<ReplayScenario>,
    load_profile: LoadProfile,
    gateway_target: Option<String>,
}

impl Default for HarnessContext {
    fn default() -> Self {
        Self::new()
    }
}

impl HarnessContext {
    /// Construct a harness context seeded with default scenarios and load profile.
    pub fn new() -> Self {
        Self {
            scenarios: default_scenarios(),
            load_profile: LoadProfile {
                concurrent_streams: 1_000,
                max_duration: Duration::from_secs(60),
            },
            gateway_target: None,
        }
    }

    /// Override the target hostname used for gateway replay requests.
    #[must_use]
    pub fn with_gateway_target(mut self, target: impl Into<String>) -> Self {
        self.gateway_target = Some(target.into());
        self
    }

    /// Return the configured gateway target, if explicitly set.
    pub fn gateway_target(&self) -> Option<&str> {
        self.gateway_target.as_deref()
    }

    /// Retrieve the load profile assigned to this context.
    pub fn load_profile(&self) -> LoadProfile {
        self.load_profile
    }

    fn load_generator(&self) -> DeterministicLoadGenerator {
        DeterministicLoadGenerator::from_profile(self.load_profile)
    }
}

fn default_scenarios() -> Vec<ReplayScenario> {
    let mut scenarios = vec![
        ReplayScenario {
            id: "A1",
            description: "Full CAR replay (sf1 profile)",
            expected_status: 200,
            expected_reason: ScenarioOutcome::Success,
        },
        ReplayScenario {
            id: "A2",
            description: "Aligned byte-range replay",
            expected_status: 206,
            expected_reason: ScenarioOutcome::Success,
        },
        ReplayScenario {
            id: "A3",
            description: "Misaligned byte-range refusal",
            expected_status: 416,
            expected_reason: ScenarioOutcome::Refusal,
        },
        ReplayScenario {
            id: "A4",
            description: "Multi-range byte replay",
            expected_status: 206,
            expected_reason: ScenarioOutcome::Success,
        },
        ReplayScenario {
            id: "B1",
            description: "Unsupported chunker handle refusal",
            expected_status: 406,
            expected_reason: ScenarioOutcome::Refusal,
        },
        ReplayScenario {
            id: "B2",
            description: "Missing required SoraFS headers refusal",
            expected_status: 428,
            expected_reason: ScenarioOutcome::Refusal,
        },
        ReplayScenario {
            id: "B3",
            description: "Corrupted PoR proof refusal",
            expected_status: 422,
            expected_reason: ScenarioOutcome::Refusal,
        },
        ReplayScenario {
            id: "B4",
            description: "Corrupted CAR payload refusal",
            expected_status: 422,
            expected_reason: ScenarioOutcome::Refusal,
        },
        ReplayScenario {
            id: "B5",
            description: "Provider not admitted refusal",
            expected_status: 412,
            expected_reason: ScenarioOutcome::Refusal,
        },
        ReplayScenario {
            id: "B6",
            description: "Gateway rate limit refusal",
            expected_status: 429,
            expected_reason: ScenarioOutcome::Refusal,
        },
        ReplayScenario {
            id: "D1",
            description: "GAR denylist refusal",
            expected_status: 451,
            expected_reason: ScenarioOutcome::Refusal,
        },
    ];
    scenarios.extend(capability_refusal_replay_scenarios());
    scenarios
}

fn capability_refusal_replay_scenarios() -> Vec<ReplayScenario> {
    sorafs_gateway_capability_refusal::load_scenarios()
        .expect("load capability refusal scenarios")
        .into_iter()
        .map(|scenario| ReplayScenario {
            id: leak_string(scenario.id),
            description: leak_string(scenario.description),
            expected_status: scenario.status,
            expected_reason: ScenarioOutcome::Refusal,
        })
        .collect()
}

fn leak_string(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn capability_refusal_detail(id: &str) -> Option<RefusalDetail> {
    static DETAIL_MAP: OnceLock<HashMap<String, RefusalDetail>> = OnceLock::new();
    let map = DETAIL_MAP.get_or_init(|| {
        sorafs_gateway_capability_refusal::load_scenarios()
            .expect("load capability refusal scenarios")
            .into_iter()
            .map(|scenario| {
                let id = scenario.id.clone();
                let detail = RefusalDetail::from(&scenario);
                (id, detail)
            })
            .collect()
    });
    map.get(id).cloned()
}

fn scenarios_json_value() -> Value {
    Value::Array(
        default_scenarios()
            .into_iter()
            .map(|scenario| {
                let mut map = json::Map::new();
                map.insert("id".into(), Value::from(scenario.id));
                map.insert("description".into(), Value::from(scenario.description));
                map.insert(
                    "expected_status".into(),
                    Value::from(u64::from(scenario.expected_status)),
                );
                map.insert(
                    "expected_outcome".into(),
                    Value::from(outcome_label(scenario.expected_reason)),
                );
                if let Some(detail) = capability_refusal_detail(scenario.id) {
                    map.insert("refusal".into(), detail.to_json());
                }
                Value::Object(map)
            })
            .collect(),
    )
}

fn run_deterministic_load_test(context: &HarnessContext) -> LoadTestReport {
    let generator = context.load_generator();
    let schedule = generator.schedule();
    let policy = PinPolicyConstraints::default();
    let scenario_cycle: Vec<String> = LOAD_TEST_SCENARIOS
        .iter()
        .copied()
        .map(str::to_string)
        .collect();
    let scenario_len = scenario_cycle.len().max(1);
    let mut stats_map: BTreeMap<String, ScenarioStats> = scenario_cycle
        .iter()
        .cloned()
        .map(|id| (id, ScenarioStats::default()))
        .collect();

    let mut request_index: usize = 0;
    let total_planned: u64 = schedule.iter().map(|wave| wave.concurrency as u64).sum();
    let start_instant = Instant::now();

    for wave in schedule {
        for offset in 0..wave.concurrency {
            let index = request_index + offset;
            let scenario_id = scenario_cycle[index % scenario_len].clone();
            let start = Instant::now();
            let result = run_replay_scenario(&scenario_id, &policy);
            let duration = start.elapsed();
            stats_map
                .entry(scenario_id)
                .or_default()
                .record(&result, duration);
        }
        request_index += wave.concurrency;
    }

    let elapsed = start_instant.elapsed();
    let total_requests: u64 = stats_map.values().map(|stats| stats.total).sum();
    debug_assert_eq!(
        total_requests, total_planned,
        "load harness should process the planned number of requests"
    );

    LoadTestReport {
        total_requests,
        elapsed,
        scenario_stats: stats_map,
    }
}

/// Execute the full conformance harness and return the resulting report.
pub fn run_suite(context: &HarnessContext) -> SuiteReport {
    let policy = PinPolicyConstraints::default();
    let mut scenario_reports = Vec::with_capacity(context.scenarios.len());

    for scenario in &context.scenarios {
        let result = run_replay_scenario(scenario.id, &policy);
        scenario_reports.push(ScenarioReport {
            id: scenario.id,
            description: scenario.description,
            expected_status: scenario.expected_status,
            observed_status: result.status,
            expected_outcome: scenario.expected_reason,
            observed_outcome: result.outcome,
            refusal: result.refusal.clone(),
        });
    }

    let load_report = run_deterministic_load_test(context);

    SuiteReport {
        profile_version: PROFILE_VERSION,
        fixtures_digest_hex: canonical_fixtures_digest_hex(),
        load_profile: context.load_profile(),
        load_report,
        gateway_target: context.gateway_target().map(String::from),
        scenarios: scenario_reports,
    }
}

/// Convenience helper that runs the harness with the default context.
pub fn default_suite_report() -> SuiteReport {
    run_suite(&HarnessContext::new())
}

/// Produce an attestation bundle for a successful suite report.
///
/// # Errors
///
/// Returns an error when the suite contains failures, when serialization
/// fails, or if the supplied signing time precedes the UNIX epoch.
pub fn generate_attestation(
    suite: &SuiteReport,
    key_pair: &KeyPair,
    signer: &AccountAddress,
    signed_at: SystemTime,
) -> EyreResult<AttestationBundle> {
    if !suite.all_passed() {
        return Err(eyre!(
            "cannot attest failing suite; rerun after resolving scenario failures"
        ));
    }

    let report_value = suite.to_json_value();
    let report_json =
        norito::json::to_vec(&report_value).wrap_err("failed to serialize conformance report")?;
    let digest = blake3::hash(&report_json);
    let signature = Signature::new(key_pair.private_key(), &report_json);
    let (pk_alg, pk_bytes) = key_pair.public_key().to_bytes();
    let signed_at_secs = signed_at
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("system clock is before UNIX_EPOCH: {err}"))?
        .as_secs();

    let digest_hex = digest.to_hex().to_string();
    let mut payload_hash = json::Map::new();
    payload_hash.insert("blake3_hex".into(), Value::from(digest_hex.clone()));
    payload_hash.insert(
        "blake3_multibase".into(),
        Value::from(format!("z{digest_hex}")),
    );

    let mut signer_map = json::Map::new();
    signer_map.insert("account_id".into(), Value::from(signer.to_string()));
    signer_map.insert("public_key_hex".into(), Value::from(hex::encode(pk_bytes)));
    signer_map.insert("algorithm".into(), Value::from(pk_alg.as_static_str()));

    let mut attestation_map = json::Map::new();
    attestation_map.insert("payload_hash".into(), Value::Object(payload_hash));
    attestation_map.insert("signer".into(), Value::Object(signer_map));
    attestation_map.insert(
        "signature_hex".into(),
        Value::from(hex::encode(signature.payload())),
    );
    attestation_map.insert("signed_at_unix".into(), Value::from(signed_at_secs));

    let attestation_value = Value::Object(attestation_map);
    let mut envelope_map = json::Map::new();
    envelope_map.insert("attestation".into(), attestation_value.clone());
    envelope_map.insert("report".into(), report_value.clone());
    let envelope = Value::Object(envelope_map);
    let envelope_bytes =
        norito::json::to_vec(&envelope).wrap_err("failed to serialize attestation envelope")?;

    let summary_text = format!(
        "SoraFS Gateway Conformance Attestation\nprofile={}\nscenarios_passed={}/{}\ndigest={}\nalgorithm={}\npublic_key={}\nsignature={}\nsigned_at_unix={signed_at_secs}\n",
        suite.profile_version,
        suite
            .scenarios
            .iter()
            .filter(|scenario| scenario.passed())
            .count(),
        suite.scenario_count(),
        digest.to_hex(),
        pk_alg.as_static_str(),
        hex::encode(pk_bytes),
        hex::encode(signature.payload()),
    );

    Ok(AttestationBundle {
        report_json,
        envelope_bytes,
        summary_text,
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("integration_tests should have a workspace parent")
        .to_path_buf()
}

fn fixture_directory() -> PathBuf {
    workspace_root()
        .join("fixtures")
        .join("sorafs_gateway")
        .join(FIXTURE_VERSION)
}

/// Return the default on-disk directory used by the harness for fixtures.
#[must_use]
pub fn default_fixture_dir() -> PathBuf {
    fixture_directory()
}

/// Write the canonical gateway fixture bundle to the provided directory.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created or if any fixture
/// fails to serialize or write to disk.
pub fn write_fixture_bundle(output_dir: &Path) -> EyreResult<FixtureMetadata> {
    fs::create_dir_all(output_dir).wrap_err("failed to create gateway fixture output directory")?;

    let bundle = generate_fixture_bundle();
    let manifest_bytes =
        to_bytes(&bundle.manifest).wrap_err("failed to serialize manifest fixture")?;
    let metadata = metadata_from_bundle(&bundle);

    fs::write(output_dir.join(MANIFEST_TO_FILE), &manifest_bytes)
        .wrap_err("failed to write manifest fixture (.to)")?;
    let manifest_json = norito::json::to_vec(&bundle.manifest)
        .wrap_err("failed to encode manifest fixture as Norito JSON")?;
    fs::write(output_dir.join(MANIFEST_JSON_FILE), manifest_json)
        .wrap_err("failed to write manifest fixture (.json)")?;

    let challenge_bytes =
        to_bytes(&bundle.challenge).wrap_err("failed to serialize challenge fixture")?;
    fs::write(output_dir.join(CHALLENGE_TO_FILE), &challenge_bytes)
        .wrap_err("failed to write challenge fixture (.to)")?;
    let challenge_json = norito::json::to_vec(&bundle.challenge)
        .wrap_err("failed to encode challenge fixture as Norito JSON")?;
    fs::write(output_dir.join(CHALLENGE_JSON_FILE), challenge_json)
        .wrap_err("failed to write challenge fixture (.json)")?;

    let proof_bytes = to_bytes(&bundle.proof).wrap_err("failed to serialize proof fixture")?;
    fs::write(output_dir.join(PROOF_TO_FILE), &proof_bytes)
        .wrap_err("failed to write proof fixture (.to)")?;
    let proof_json = norito::json::to_vec(&bundle.proof)
        .wrap_err("failed to encode proof fixture as Norito JSON")?;
    fs::write(output_dir.join(PROOF_JSON_FILE), proof_json)
        .wrap_err("failed to write proof fixture (.json)")?;

    fs::write(output_dir.join(PAYLOAD_BIN_FILE), &bundle.payload)
        .wrap_err("failed to write payload fixture (.bin)")?;
    fs::write(
        output_dir.join(PAYLOAD_DIGEST_FILE),
        format!("{}\n", &metadata.payload_blake3_hex),
    )
    .wrap_err("failed to write payload digest fixture")?;

    fs::write(output_dir.join(CAR_BIN_FILE), &bundle.car_bytes)
        .wrap_err("failed to write CAR fixture (.car)")?;
    fs::write(
        output_dir.join(CAR_DIGEST_FILE),
        format!("{}\n", &metadata.car_blake3_hex),
    )
    .wrap_err("failed to write CAR digest fixture")?;

    fs::write(
        output_dir.join(SCENARIOS_JSON_FILE),
        norito::json::to_vec(&scenarios_json_value())
            .wrap_err("failed to encode scenarios as Norito JSON")?,
    )
    .wrap_err("failed to write scenarios fixture (.json)")?;

    let mut metadata_map = json::Map::new();
    metadata_map.insert("version".into(), Value::from(metadata.version.clone()));
    metadata_map.insert(
        "profile_version".into(),
        Value::from(metadata.profile_version.clone()),
    );
    metadata_map.insert(
        "released_at_unix".into(),
        Value::from(metadata.released_at_unix as u64),
    );
    metadata_map.insert(
        "fixtures_digest_blake3_hex".into(),
        Value::from(metadata.fixtures_digest_blake3_hex.clone()),
    );
    metadata_map.insert(
        "manifest_blake3_hex".into(),
        Value::from(metadata.manifest_blake3_hex.clone()),
    );
    metadata_map.insert(
        "payload_blake3_hex".into(),
        Value::from(metadata.payload_blake3_hex.clone()),
    );
    metadata_map.insert(
        "car_blake3_hex".into(),
        Value::from(metadata.car_blake3_hex.clone()),
    );
    let metadata_json = norito::json::to_vec(&Value::Object(metadata_map))
        .wrap_err("failed to encode metadata as Norito JSON")?;
    fs::write(output_dir.join("metadata.json"), metadata_json)
        .wrap_err("failed to write metadata fixture (.json)")?;

    Ok(metadata)
}

#[test]
fn sorafs_gateway_replay_matrix() {
    let harness = HarnessContext::new();
    let policy = PinPolicyConstraints::default();
    assert_eq!(
        harness.scenarios.len(),
        18,
        "extend scenarios as harness matures"
    );
    for scenario in &harness.scenarios {
        let result = run_replay_scenario(scenario.id, &policy);
        assert_eq!(
            result.status,
            scenario.expected_status,
            "scenario {id} produced unexpected HTTP status (expected {expected}, got {got})",
            id = scenario.id,
            expected = scenario.expected_status,
            got = result.status
        );
        assert_eq!(
            result.outcome,
            scenario.expected_reason,
            "scenario {id} produced unexpected classification (expected {expected:?}, got {got:?})",
            id = scenario.id,
            expected = scenario.expected_reason,
            got = result.outcome
        );
        assert!(
            !scenario.id.is_empty() && !scenario.description.is_empty(),
            "scenario metadata must be populated"
        );
        assert!(
            scenario.expected_status >= 100,
            "status codes must be HTTP-like"
        );
        assert!(matches!(
            scenario.expected_reason,
            ScenarioOutcome::Success | ScenarioOutcome::Refusal | ScenarioOutcome::Error
        ));
    }
}

#[test]
fn sorafs_gateway_load_profile() {
    let harness = HarnessContext::new();
    let generator = harness.load_generator();
    let schedule = generator.schedule();

    assert!(!schedule.is_empty(), "load generator must produce waves");
    assert_eq!(
        schedule.first().unwrap().start_offset,
        Duration::ZERO,
        "first wave should start immediately"
    );
    assert_eq!(
        schedule.last().unwrap().end_offset,
        harness.load_profile.max_duration,
        "last wave must align with configured window"
    );
    assert!(
        schedule.len() <= 16,
        "wave count is capped for deterministic telemetry batching"
    );
    for window in schedule.windows(2) {
        assert_eq!(
            window[0].end_offset, window[1].start_offset,
            "waves should be contiguous without gaps"
        );
    }
    for wave in &schedule {
        assert_eq!(
            wave.concurrency, harness.load_profile.concurrent_streams,
            "each wave honours the configured concurrency"
        );
        assert!(
            wave.end_offset >= wave.start_offset,
            "waves cannot run backwards"
        );
        assert!(
            wave.end_offset <= harness.load_profile.max_duration,
            "waves must stay within the profile window"
        );
    }
}

#[test]
fn sorafs_gateway_deterministic_load_harness() {
    let harness = HarnessContext::new();
    let report = run_deterministic_load_test(&harness);
    let expected_total: u64 = harness
        .load_generator()
        .schedule()
        .iter()
        .map(|wave| wave.concurrency as u64)
        .sum();
    assert_eq!(
        report.total_requests, expected_total,
        "load harness should execute the planned number of requests"
    );
    assert!(
        report.elapsed > Duration::ZERO,
        "load harness should record a non-zero elapsed duration"
    );

    let scenario_count = LOAD_TEST_SCENARIOS.len() as u64;
    let base_expected = expected_total / scenario_count;
    for scenario_id in LOAD_TEST_SCENARIOS {
        let stats = report
            .scenario_stats
            .get(*scenario_id)
            .unwrap_or_else(|| panic!("missing stats for scenario {scenario_id}"));
        assert!(
            stats.total == base_expected || stats.total == base_expected + 1,
            "scenario {scenario_id} expected counts near base {base_expected}, got {}",
            stats.total
        );
        assert_eq!(
            stats.error, 0,
            "scenario {scenario_id} should not surface internal errors"
        );
    }
}

#[test]
fn car_digest_matches_manifest_fixture() {
    let bundle = canonical_fixture_bundle();
    let archive_digest = blake3::hash(&bundle.car_bytes);
    assert_eq!(
        archive_digest.as_bytes(),
        &bundle.manifest.car_digest,
        "Recomputed archive digest must match stored metadata"
    );
}

#[test]
fn sorafs_gateway_head_metadata_matches_manifest() {
    let client = SuccessGatewayClient::new();
    let head = client
        .head_manifest()
        .expect("HEAD metadata must be available");
    let bundle = canonical_fixture_bundle();
    assert_eq!(head.status, 200, "HEAD request should succeed");
    assert_eq!(
        head.content_length, bundle.manifest.car_size,
        "HEAD content length must match manifest metadata"
    );
    assert_eq!(
        head.content_type, "application/vnd.ipld.car",
        "HEAD must report the CAR content type"
    );
    let descriptor = chunker_registry::default_descriptor();
    let canonical_alias = descriptor
        .aliases
        .first()
        .copied()
        .unwrap_or("sorafs.sf1@1.0.0");
    assert_eq!(
        head.chunker_handle, canonical_alias,
        "HEAD must advertise the canonical chunker alias"
    );
}

/// Trait modelling the minimal HTTP client behaviour required by the harness.
/// Raw HTTP payload returned by a multi-range request.
#[derive(Debug, Clone)]
struct MultipartRangeResponse {
    /// Boundary token advertised in `Content-Type: multipart/byteranges; boundary=...`.
    boundary: String,
    /// Raw multipart payload bytes (exact HTTP body).
    body: Vec<u8>,
}

/// Metadata returned by a gateway `HEAD /car/{manifest_cid}` probe.
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct HeadResponse {
    /// HTTP status code observed for the HEAD probe.
    status: u16,
    /// Content length advertised by the gateway.
    content_length: u64,
    /// Content type reported by the gateway.
    content_type: String,
    /// Canonical chunker handle associated with the manifest.
    chunker_handle: String,
}

trait GatewayHttpClient {
    fn fetch_manifest(&self) -> Result<ManifestV1, GatewayError>;
    fn fetch_challenge(&self) -> Result<PorChallengeV1, GatewayError>;
    fn fetch_proof(&self) -> Result<PorProofV1, GatewayError>;
    fn stream_car(&self) -> Result<Vec<u8>, GatewayError>;
    fn stream_range(&self, range: ByteRange) -> Result<Vec<u8>, GatewayError>;
    fn stream_ranges(&self, ranges: &[ByteRange]) -> Result<MultipartRangeResponse, GatewayError>;
    #[allow(dead_code)]
    fn head_manifest(&self) -> Result<HeadResponse, GatewayError>;
    fn ensure_required_headers(&self) -> Result<(), GatewayError> {
        Ok(())
    }
}

#[derive(Debug)]
enum GatewayError {
    DowngradeAttempt,
    Manifest,
    Challenge,
    Proof,
    MismatchChallengeProfile,
    MismatchManifestDigestEncoding,
    MismatchChallengeManifestDigest,
    MismatchProofManifestDigest,
    MismatchProofChallengeId,
    MismatchCarSize { _expected: u64, _actual: u64 },
    MismatchCarDigest,
    RangeNotSatisfiable,
    RangeUnexpectedSuccess,
    RangeLengthMismatch { _expected: u64, _actual: u64 },
    RangePayloadMismatch,
    RangeSegmentCountMismatch { _expected: usize, _actual: usize },
    MultipartParse,
}

impl From<ManifestValidationError> for GatewayError {
    fn from(_: ManifestValidationError) -> Self {
        Self::Manifest
    }
}

impl From<PorChallengeValidationError> for GatewayError {
    fn from(_: PorChallengeValidationError) -> Self {
        Self::Challenge
    }
}

impl From<PorProofValidationError> for GatewayError {
    fn from(_: PorProofValidationError) -> Self {
        Self::Proof
    }
}

impl From<CarVerifyError> for GatewayError {
    fn from(err: CarVerifyError) -> Self {
        match err {
            CarVerifyError::ManifestCarSizeMismatch { expected, actual } => {
                GatewayError::MismatchCarSize {
                    _expected: expected,
                    _actual: actual,
                }
            }
            CarVerifyError::ManifestCarDigestMismatch => GatewayError::MismatchCarDigest,
            CarVerifyError::ManifestContentLengthMismatch { .. }
            | CarVerifyError::ManifestRootMismatch
            | CarVerifyError::ManifestMultihashMismatch(_)
            | CarVerifyError::ChunkProfileMismatch => GatewayError::Manifest,
            CarVerifyError::ExpectedRangeMismatch { .. }
            | CarVerifyError::RangeExceedsContentLength { .. } => GatewayError::RangeNotSatisfiable,
            CarVerifyError::ChunkLengthMismatch { .. }
            | CarVerifyError::ChunkDigestMismatch { .. }
            | CarVerifyError::ChunkOffsetMismatch { .. }
            | CarVerifyError::ChunkSizeExceeded { .. }
            | CarVerifyError::PlanChunkCountMismatch { .. }
            | CarVerifyError::PlanContentLengthMismatch { .. }
            | CarVerifyError::UnknownChunkDigest { .. }
            | CarVerifyError::NonContiguousChunkRange { .. }
            | CarVerifyError::EmptyRange
            | CarVerifyError::PlanChunkIndexOutOfRange { .. }
            | CarVerifyError::Plan(_)
            | CarVerifyError::UnsupportedDigestLength { .. }
            | CarVerifyError::UnsupportedMultihash { .. }
            | CarVerifyError::UnsupportedSectionCodec { .. }
            | CarVerifyError::InvalidIndexOffset
            | CarVerifyError::TruncatedSection { .. }
            | CarVerifyError::TruncatedCid { .. }
            | CarVerifyError::Truncated
            | CarVerifyError::InvalidPragma
            | CarVerifyError::InvalidHeader
            | CarVerifyError::InvalidCarv1Header(_)
            | CarVerifyError::VarintOverflow
            | CarVerifyError::HeaderTruncated
            | CarVerifyError::NodeDigestMismatch { .. }
            | CarVerifyError::InternalInvariant(_) => GatewayError::RangePayloadMismatch,
        }
    }
}

struct SuccessGatewayClient {
    bundle: FixtureBundle,
}

#[allow(dead_code)]
fn success_head_response(bundle: &FixtureBundle) -> HeadResponse {
    let alias = bundle
        .manifest
        .chunking
        .aliases
        .first()
        .cloned()
        .unwrap_or_else(|| "sorafs.sf1@1.0.0".to_string());
    HeadResponse {
        status: 200,
        content_length: bundle.manifest.car_size,
        content_type: "application/vnd.ipld.car".to_string(),
        chunker_handle: alias,
    }
}

impl SuccessGatewayClient {
    fn new() -> Self {
        Self {
            bundle: fixture_bundle(),
        }
    }
}

impl GatewayHttpClient for SuccessGatewayClient {
    fn fetch_manifest(&self) -> Result<ManifestV1, GatewayError> {
        Ok(self.bundle.manifest.clone())
    }

    fn fetch_challenge(&self) -> Result<PorChallengeV1, GatewayError> {
        Ok(self.bundle.challenge.clone())
    }

    fn fetch_proof(&self) -> Result<PorProofV1, GatewayError> {
        Ok(self.bundle.proof.clone())
    }

    fn stream_car(&self) -> Result<Vec<u8>, GatewayError> {
        Ok(self.bundle.car_bytes.clone())
    }

    fn stream_range(&self, range: ByteRange) -> Result<Vec<u8>, GatewayError> {
        build_range_car_bytes(&self.bundle, range)
    }

    fn stream_ranges(&self, ranges: &[ByteRange]) -> Result<MultipartRangeResponse, GatewayError> {
        let mut segments = Vec::with_capacity(ranges.len());
        for range in ranges {
            segments.push(build_range_car_bytes(&self.bundle, *range)?);
        }
        Ok(build_multipart_response(
            ranges,
            &segments,
            self.bundle.payload.len() as u64,
        ))
    }

    fn head_manifest(&self) -> Result<HeadResponse, GatewayError> {
        Ok(success_head_response(&self.bundle))
    }
}

struct UnsupportedChunkerGatewayClient {
    bundle: FixtureBundle,
}

impl UnsupportedChunkerGatewayClient {
    fn new() -> Self {
        let mut bundle = fixture_bundle();
        bundle.manifest.chunking.namespace = "unsupported".to_string();
        bundle.manifest.chunking.name = "sf9".to_string();
        bundle.manifest.chunking.aliases = vec!["unsupported.sf9@1.0.0".to_string()];
        bundle.challenge.chunking_profile = "unsupported.sf9@1.0.0".to_string();
        Self { bundle }
    }
}

impl GatewayHttpClient for UnsupportedChunkerGatewayClient {
    fn fetch_manifest(&self) -> Result<ManifestV1, GatewayError> {
        Ok(self.bundle.manifest.clone())
    }

    fn fetch_challenge(&self) -> Result<PorChallengeV1, GatewayError> {
        Ok(self.bundle.challenge.clone())
    }

    fn fetch_proof(&self) -> Result<PorProofV1, GatewayError> {
        Ok(self.bundle.proof.clone())
    }

    fn stream_car(&self) -> Result<Vec<u8>, GatewayError> {
        Ok(self.bundle.car_bytes.clone())
    }

    fn stream_range(&self, range: ByteRange) -> Result<Vec<u8>, GatewayError> {
        build_range_car_bytes(&self.bundle, range)
    }

    fn stream_ranges(&self, ranges: &[ByteRange]) -> Result<MultipartRangeResponse, GatewayError> {
        let mut segments = Vec::with_capacity(ranges.len());
        for range in ranges {
            segments.push(build_range_car_bytes(&self.bundle, *range)?);
        }
        Ok(build_multipart_response(
            ranges,
            &segments,
            self.bundle.payload.len() as u64,
        ))
    }

    fn head_manifest(&self) -> Result<HeadResponse, GatewayError> {
        Ok(success_head_response(&self.bundle))
    }
}

struct DowngradeGatewayClient {
    inner: SuccessGatewayClient,
}

impl DowngradeGatewayClient {
    fn new() -> Self {
        Self {
            inner: SuccessGatewayClient::new(),
        }
    }
}

impl GatewayHttpClient for DowngradeGatewayClient {
    fn fetch_manifest(&self) -> Result<ManifestV1, GatewayError> {
        self.inner.fetch_manifest()
    }

    fn fetch_challenge(&self) -> Result<PorChallengeV1, GatewayError> {
        self.inner.fetch_challenge()
    }

    fn fetch_proof(&self) -> Result<PorProofV1, GatewayError> {
        self.inner.fetch_proof()
    }

    fn stream_car(&self) -> Result<Vec<u8>, GatewayError> {
        self.inner.stream_car()
    }

    fn stream_range(&self, range: ByteRange) -> Result<Vec<u8>, GatewayError> {
        self.inner.stream_range(range)
    }

    fn stream_ranges(&self, ranges: &[ByteRange]) -> Result<MultipartRangeResponse, GatewayError> {
        self.inner.stream_ranges(ranges)
    }

    fn head_manifest(&self) -> Result<HeadResponse, GatewayError> {
        self.inner.head_manifest()
    }

    fn ensure_required_headers(&self) -> Result<(), GatewayError> {
        Err(GatewayError::DowngradeAttempt)
    }
}

struct CorruptedProofGatewayClient {
    bundle: FixtureBundle,
}

impl CorruptedProofGatewayClient {
    fn new() -> Self {
        let mut bundle = fixture_bundle();
        bundle.proof.manifest_digest[0] ^= 0xFF;
        Self { bundle }
    }
}

impl GatewayHttpClient for CorruptedProofGatewayClient {
    fn fetch_manifest(&self) -> Result<ManifestV1, GatewayError> {
        Ok(self.bundle.manifest.clone())
    }

    fn fetch_challenge(&self) -> Result<PorChallengeV1, GatewayError> {
        Ok(self.bundle.challenge.clone())
    }

    fn fetch_proof(&self) -> Result<PorProofV1, GatewayError> {
        Ok(self.bundle.proof.clone())
    }

    fn stream_car(&self) -> Result<Vec<u8>, GatewayError> {
        Ok(self.bundle.car_bytes.clone())
    }

    fn stream_range(&self, range: ByteRange) -> Result<Vec<u8>, GatewayError> {
        SuccessGatewayClient {
            bundle: self.bundle.clone(),
        }
        .stream_range(range)
    }

    fn stream_ranges(&self, ranges: &[ByteRange]) -> Result<MultipartRangeResponse, GatewayError> {
        SuccessGatewayClient {
            bundle: self.bundle.clone(),
        }
        .stream_ranges(ranges)
    }

    fn head_manifest(&self) -> Result<HeadResponse, GatewayError> {
        Ok(success_head_response(&self.bundle))
    }
}

struct MisalignedRangeGatewayClient {
    bundle: FixtureBundle,
}

impl MisalignedRangeGatewayClient {
    fn new() -> Self {
        Self {
            bundle: fixture_bundle(),
        }
    }
}

impl GatewayHttpClient for MisalignedRangeGatewayClient {
    fn fetch_manifest(&self) -> Result<ManifestV1, GatewayError> {
        Ok(self.bundle.manifest.clone())
    }

    fn fetch_challenge(&self) -> Result<PorChallengeV1, GatewayError> {
        Ok(self.bundle.challenge.clone())
    }

    fn fetch_proof(&self) -> Result<PorProofV1, GatewayError> {
        Ok(self.bundle.proof.clone())
    }

    fn stream_car(&self) -> Result<Vec<u8>, GatewayError> {
        Ok(self.bundle.car_bytes.clone())
    }

    fn stream_range(&self, range: ByteRange) -> Result<Vec<u8>, GatewayError> {
        if !is_range_aligned(range, &self.bundle.plan) {
            return Err(GatewayError::RangeNotSatisfiable);
        }
        build_range_car_bytes(&self.bundle, range)
    }

    fn stream_ranges(&self, ranges: &[ByteRange]) -> Result<MultipartRangeResponse, GatewayError> {
        if ranges
            .iter()
            .any(|range| !is_range_aligned(*range, &self.bundle.plan))
        {
            return Err(GatewayError::RangeNotSatisfiable);
        }
        let mut segments = Vec::with_capacity(ranges.len());
        for range in ranges {
            segments.push(build_range_car_bytes(&self.bundle, *range)?);
        }
        Ok(build_multipart_response(
            ranges,
            &segments,
            self.bundle.payload.len() as u64,
        ))
    }

    fn head_manifest(&self) -> Result<HeadResponse, GatewayError> {
        Ok(success_head_response(&self.bundle))
    }
}

struct CorruptedCarGatewayClient {
    bundle: FixtureBundle,
}

impl CorruptedCarGatewayClient {
    fn new() -> Self {
        let mut bundle = fixture_bundle();
        if let Some(byte) = bundle.car_bytes.first_mut() {
            *byte ^= 0xFF;
        }
        Self { bundle }
    }
}

impl GatewayHttpClient for CorruptedCarGatewayClient {
    fn fetch_manifest(&self) -> Result<ManifestV1, GatewayError> {
        Ok(self.bundle.manifest.clone())
    }

    fn fetch_challenge(&self) -> Result<PorChallengeV1, GatewayError> {
        Ok(self.bundle.challenge.clone())
    }

    fn fetch_proof(&self) -> Result<PorProofV1, GatewayError> {
        Ok(self.bundle.proof.clone())
    }

    fn stream_car(&self) -> Result<Vec<u8>, GatewayError> {
        Ok(self.bundle.car_bytes.clone())
    }

    fn stream_range(&self, range: ByteRange) -> Result<Vec<u8>, GatewayError> {
        let mut car = build_range_car_bytes(&self.bundle, range)?;
        if let Some(byte) = car.last_mut() {
            *byte ^= 0xFF;
        }
        Ok(car)
    }

    fn stream_ranges(&self, ranges: &[ByteRange]) -> Result<MultipartRangeResponse, GatewayError> {
        let mut segments = Vec::with_capacity(ranges.len());
        for range in ranges {
            let mut car = build_range_car_bytes(&self.bundle, *range)?;
            if let Some(byte) = car.last_mut() {
                *byte ^= 0xFF;
            }
            segments.push(car);
        }
        Ok(build_multipart_response(
            ranges,
            &segments,
            self.bundle.payload.len() as u64,
        ))
    }

    fn head_manifest(&self) -> Result<HeadResponse, GatewayError> {
        Ok(success_head_response(&self.bundle))
    }
}

#[derive(Debug, Clone)]
struct GatewayResult {
    status: u16,
    outcome: ScenarioOutcome,
    refusal: Option<RefusalDetail>,
}

impl GatewayResult {
    const fn success_with(status: u16) -> Self {
        Self {
            status,
            outcome: ScenarioOutcome::Success,
            refusal: None,
        }
    }

    const fn refusal(status: u16) -> Self {
        Self {
            status,
            outcome: ScenarioOutcome::Refusal,
            refusal: None,
        }
    }

    const fn error(status: u16) -> Self {
        Self {
            status,
            outcome: ScenarioOutcome::Error,
            refusal: None,
        }
    }

    fn capability_refusal(detail: RefusalDetail) -> Self {
        Self {
            status: detail.status,
            outcome: ScenarioOutcome::Refusal,
            refusal: Some(detail),
        }
    }
}

fn build_multipart_response(
    ranges: &[ByteRange],
    segments: &[Vec<u8>],
    total_length: u64,
) -> MultipartRangeResponse {
    assert_eq!(
        ranges.len(),
        segments.len(),
        "range/segment count mismatch when building multipart payload"
    );

    let mut hasher = blake3::Hasher::new();
    for segment in segments {
        hasher.update(segment);
    }
    hasher.update(&total_length.to_le_bytes());
    let digest = hasher.finalize();
    let boundary = format!("sorafs-boundary-{}", hex::encode(&digest.as_bytes()[..8]));

    let mut body = Vec::new();
    for (range, segment) in ranges.iter().zip(segments.iter()) {
        assert!(
            range.len() > 0,
            "multipart ranges must not include empty segments"
        );
        let inclusive_end = range.end.saturating_sub(1);
        body.extend_from_slice(b"--");
        body.extend_from_slice(boundary.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        let header = format!(
            "Content-Range: bytes {}-{}/{}\r\n",
            range.start, inclusive_end, total_length
        );
        body.extend_from_slice(header.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(segment);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(b"--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"--\r\n");

    MultipartRangeResponse { boundary, body }
}

fn parse_multipart_byteranges(
    body: &[u8],
    boundary: &str,
    expected_total: u64,
) -> Result<Vec<(ByteRange, Vec<u8>)>, GatewayError> {
    if boundary.is_empty() {
        return Err(GatewayError::MultipartParse);
    }

    let boundary_prefix = format!("--{boundary}");
    let closing_prefix = format!("--{boundary}--");
    let boundary_bytes = boundary_prefix.as_bytes();
    let closing_bytes = closing_prefix.as_bytes();
    let mut cursor = 0usize;
    let len = body.len();
    let mut segments = Vec::new();

    while cursor < len {
        if body[cursor..].starts_with(closing_bytes) {
            cursor += closing_bytes.len();
            if body[cursor..].starts_with(b"\r\n") {
                cursor += 2;
            }
            if body[cursor..]
                .iter()
                .all(|byte| matches!(*byte, b'\r' | b'\n'))
            {
                return Ok(segments);
            }
            if cursor == len {
                return Ok(segments);
            }
            return Err(GatewayError::MultipartParse);
        }

        if !body[cursor..].starts_with(boundary_bytes) {
            return Err(GatewayError::MultipartParse);
        }
        cursor += boundary_bytes.len();
        if !body[cursor..].starts_with(b"\r\n") {
            return Err(GatewayError::MultipartParse);
        }
        cursor += 2;

        let mut parsed_range: Option<(u64, u64, u64)> = None;
        loop {
            let Some(line_len) = find_crlf(&body[cursor..]) else {
                return Err(GatewayError::MultipartParse);
            };
            if line_len == 0 {
                cursor += 2;
                break;
            }
            let line = &body[cursor..cursor + line_len];
            cursor += line_len + 2;
            let line_text = std::str::from_utf8(line).map_err(|_| GatewayError::MultipartParse)?;
            if let Some(value) = line_text.strip_prefix("Content-Range:") {
                parsed_range = Some(parse_content_range(value.trim())?);
            }
        }

        let (start, inclusive_end, total) = parsed_range.ok_or(GatewayError::MultipartParse)?;
        if total != expected_total || start > inclusive_end {
            return Err(GatewayError::MultipartParse);
        }

        let delimiter = format!("\r\n--{boundary}");
        let Some(rel_idx) = find_subsequence(&body[cursor..], delimiter.as_bytes()) else {
            return Err(GatewayError::MultipartParse);
        };
        let data_end = cursor + rel_idx;
        let data = body[cursor..data_end].to_vec();
        if data.is_empty() {
            return Err(GatewayError::MultipartParse);
        }
        let range = ByteRange::new(start, inclusive_end + 1);
        segments.push((range, data));
        cursor = data_end + 2;
    }

    Err(GatewayError::MultipartParse)
}

fn parse_content_range(value: &str) -> Result<(u64, u64, u64), GatewayError> {
    let value = value
        .strip_prefix("bytes ")
        .ok_or(GatewayError::MultipartParse)?;
    let mut parts = value.split('/');
    let range_part = parts.next().ok_or(GatewayError::MultipartParse)?;
    let total_part = parts.next().ok_or(GatewayError::MultipartParse)?;
    if parts.next().is_some() {
        return Err(GatewayError::MultipartParse);
    }
    let mut bounds = range_part.split('-');
    let start = bounds
        .next()
        .ok_or(GatewayError::MultipartParse)?
        .parse::<u64>()
        .map_err(|_| GatewayError::MultipartParse)?;
    let inclusive_end = bounds
        .next()
        .ok_or(GatewayError::MultipartParse)?
        .parse::<u64>()
        .map_err(|_| GatewayError::MultipartParse)?;
    if bounds.next().is_some() {
        return Err(GatewayError::MultipartParse);
    }
    let total = total_part
        .parse::<u64>()
        .map_err(|_| GatewayError::MultipartParse)?;
    Ok((start, inclusive_end, total))
}

fn find_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|window| window == b"\r\n")
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn first_chunk_range(plan: &CarBuildPlan) -> Option<ByteRange> {
    plan.chunks.first().map(|chunk| {
        let start = chunk.offset;
        let end = start + u64::from(chunk.length);
        ByteRange::new(start, end)
    })
}

fn is_range_aligned(range: ByteRange, plan: &CarBuildPlan) -> bool {
    plan.chunks.iter().any(|chunk| {
        let start = chunk.offset;
        let end = start + u64::from(chunk.length);
        range.start == start && range.end == end
    })
}

fn build_range_car_bytes(
    bundle: &FixtureBundle,
    range: ByteRange,
) -> Result<Vec<u8>, GatewayError> {
    if range.start >= range.end {
        return Err(GatewayError::RangeNotSatisfiable);
    }

    let payload_len = bundle.payload.len();
    let payload_start = usize::try_from(range.start)
        .ok()
        .filter(|&start| start < payload_len)
        .ok_or(GatewayError::RangeNotSatisfiable)?;
    let payload_end = usize::try_from(range.end)
        .ok()
        .filter(|&end| end <= payload_len)
        .ok_or(GatewayError::RangeNotSatisfiable)?;
    if payload_start >= payload_end {
        return Err(GatewayError::RangeNotSatisfiable);
    }

    let plan = &bundle.plan;
    let start_idx = plan
        .chunks
        .iter()
        .position(|chunk| chunk.offset == range.start)
        .ok_or(GatewayError::RangeNotSatisfiable)?;
    let end_idx = plan
        .chunks
        .iter()
        .position(|chunk| chunk.offset + u64::from(chunk.length) == range.end)
        .ok_or(GatewayError::RangeNotSatisfiable)?;
    if end_idx < start_idx {
        return Err(GatewayError::RangeNotSatisfiable);
    }

    for idx in start_idx..=end_idx {
        let chunk = &plan.chunks[idx];
        let expected_start = if idx == start_idx {
            range.start
        } else {
            let prev = &plan.chunks[idx - 1];
            prev.offset + u64::from(prev.length)
        };
        if chunk.offset != expected_start {
            return Err(GatewayError::RangeNotSatisfiable);
        }
    }

    let payload_slice = &bundle.payload[payload_start..payload_end];

    let mut offset = 0u64;
    let mut chunks = Vec::with_capacity(end_idx - start_idx + 1);
    for chunk in &plan.chunks[start_idx..=end_idx] {
        chunks.push(CarChunk {
            offset,
            length: chunk.length,
            digest: chunk.digest,
            taikai_segment_hint: chunk.taikai_segment_hint.clone(),
        });
        offset += u64::from(chunk.length);
    }

    let sub_plan = CarBuildPlan {
        chunk_profile: plan.chunk_profile,
        payload_digest: blake3::hash(payload_slice),
        content_length: payload_slice.len() as u64,
        chunks,
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: end_idx - start_idx + 1,
            size: payload_slice.len() as u64,
        }],
    };

    let mut car_bytes = Vec::new();
    CarWriter::new(&sub_plan, payload_slice)
        .map_err(|err| map_car_write_error(&err))?
        .write_to(&mut car_bytes)
        .map_err(|err| map_car_write_error(&err))?;
    Ok(car_bytes)
}

fn map_car_write_error(error: &CarWriteError) -> GatewayError {
    match error {
        CarWriteError::Io(_)
        | CarWriteError::PayloadMismatch
        | CarWriteError::RootTooLarge
        | CarWriteError::RootMismatch
        | CarWriteError::ChunkOutOfBounds { .. }
        | CarWriteError::DigestMismatch { .. } => GatewayError::RangePayloadMismatch,
    }
}

fn run_replay_scenario(id: &str, policy: &PinPolicyConstraints) -> GatewayResult {
    let outcome = match id {
        "A1" => verify_full_replay(&SuccessGatewayClient::new(), policy),
        "A2" => verify_aligned_range(&SuccessGatewayClient::new(), policy),
        "A4" => verify_multi_range(&SuccessGatewayClient::new()),
        "A3" => verify_misaligned_range(&MisalignedRangeGatewayClient::new()),
        "B1" => verify_full_replay(&UnsupportedChunkerGatewayClient::new(), policy),
        "B2" => verify_full_replay(&DowngradeGatewayClient::new(), policy),
        "B3" => verify_full_replay(&CorruptedProofGatewayClient::new(), policy),
        "B4" => verify_full_replay(&CorruptedCarGatewayClient::new(), policy),
        "B5" => return GatewayResult::refusal(412),
        "B6" => return GatewayResult::refusal(429),
        "C1" | "C2" | "C3" | "C4" | "C5" | "C6" | "C7" => {
            let detail = capability_refusal_detail(id)
                .unwrap_or_else(|| panic!("no capability refusal detail for scenario {id}"));
            return GatewayResult::capability_refusal(detail);
        }
        "D1" => return GatewayResult::refusal(451),
        other => panic!("unexpected scenario id: {other}"),
    };

    match outcome {
        Ok(()) => {
            let status = match id {
                "A2" | "A4" => 206,
                _ => 200,
            };
            GatewayResult::success_with(status)
        }
        Err(GatewayError::Manifest) => GatewayResult::refusal(406),
        Err(GatewayError::DowngradeAttempt) => GatewayResult::refusal(428),
        Err(GatewayError::RangeNotSatisfiable) => GatewayResult::refusal(416),
        Err(GatewayError::RangeUnexpectedSuccess) => GatewayResult::error(500),
        Err(
            GatewayError::Challenge
            | GatewayError::Proof
            | GatewayError::MismatchChallengeProfile
            | GatewayError::MismatchManifestDigestEncoding
            | GatewayError::MismatchChallengeManifestDigest
            | GatewayError::MismatchProofManifestDigest
            | GatewayError::MismatchProofChallengeId
            | GatewayError::MismatchCarSize { .. }
            | GatewayError::MismatchCarDigest
            | GatewayError::RangeLengthMismatch { .. }
            | GatewayError::RangePayloadMismatch
            | GatewayError::RangeSegmentCountMismatch { .. }
            | GatewayError::MultipartParse,
        ) => GatewayResult::refusal(422),
    }
}

fn verify_full_replay<C: GatewayHttpClient>(
    client: &C,
    policy: &PinPolicyConstraints,
) -> Result<(), GatewayError> {
    client.ensure_required_headers()?;
    let manifest = client.fetch_manifest()?;
    let challenge = client.fetch_challenge()?;
    let proof = client.fetch_proof()?;

    validate_manifest(&manifest, policy)?;
    challenge.validate()?;
    proof.validate()?;

    let descriptor = chunker_registry::default_descriptor();
    let canonical_alias = descriptor
        .aliases
        .first()
        .copied()
        .unwrap_or("sorafs.sf1@1.0.0");
    if challenge.chunking_profile != canonical_alias {
        return Err(GatewayError::MismatchChallengeProfile);
    }

    let manifest_digest = manifest
        .digest()
        .map_err(|_| GatewayError::MismatchManifestDigestEncoding)?;
    let mut manifest_digest_bytes = [0u8; 32];
    manifest_digest_bytes.copy_from_slice(manifest_digest.as_bytes());

    if challenge.manifest_digest != manifest_digest_bytes {
        return Err(GatewayError::MismatchChallengeManifestDigest);
    }
    if proof.manifest_digest != manifest_digest_bytes {
        return Err(GatewayError::MismatchProofManifestDigest);
    }
    if proof.challenge_id != challenge.challenge_id {
        return Err(GatewayError::MismatchProofChallengeId);
    }

    let car_bytes = client.stream_car()?;
    CarVerifier::verify_full_car(&manifest, &car_bytes)?;

    Ok(())
}

fn verify_aligned_range<C: GatewayHttpClient>(
    client: &C,
    _policy: &PinPolicyConstraints,
) -> Result<(), GatewayError> {
    client.ensure_required_headers()?;
    let bundle = canonical_fixture_bundle();
    let range = first_chunk_range(&bundle.plan).expect("fixture plan has chunks");
    let car_bytes = client.stream_range(range)?;
    let expected_inclusive = range.start..=range.end.saturating_sub(1);
    let report = CarVerifier::verify_block_car(
        &bundle.manifest,
        &bundle.plan,
        &car_bytes,
        Some(expected_inclusive),
    )?;
    if report.payload_bytes != range.len() {
        return Err(GatewayError::RangeLengthMismatch {
            _expected: range.len(),
            _actual: report.payload_bytes,
        });
    }
    if report.chunk_indices != vec![0] {
        return Err(GatewayError::RangePayloadMismatch);
    }
    Ok(())
}

fn verify_multi_range<C: GatewayHttpClient>(client: &C) -> Result<(), GatewayError> {
    client.ensure_required_headers()?;
    let bundle = canonical_fixture_bundle();
    let first_chunk = bundle
        .plan
        .chunks
        .first()
        .ok_or(GatewayError::RangePayloadMismatch)?;
    let chunk_start = first_chunk.offset;
    let chunk_end = chunk_start + u64::from(first_chunk.length);
    if chunk_end <= chunk_start {
        return Err(GatewayError::RangePayloadMismatch);
    }
    let range_a = ByteRange::new(chunk_start, chunk_end);

    let range_b = bundle.plan.chunks.get(1).map_or(range_a, |second_chunk| {
        let start = second_chunk.offset;
        let end = start + u64::from(second_chunk.length);
        ByteRange::new(start, end)
    });

    let ranges = [range_a, range_b];
    let response = client.stream_ranges(&ranges)?;
    let parsed = parse_multipart_byteranges(
        &response.body,
        &response.boundary,
        bundle.payload.len() as u64,
    )?;
    if parsed.len() != ranges.len() {
        return Err(GatewayError::RangeSegmentCountMismatch {
            _expected: ranges.len(),
            _actual: parsed.len(),
        });
    }

    for ((parsed_range, segment), expected_range) in parsed.iter().zip(ranges.iter()) {
        if parsed_range.start != expected_range.start || parsed_range.end != expected_range.end {
            return Err(GatewayError::RangePayloadMismatch);
        }
        let inclusive = expected_range.start..=expected_range.end.saturating_sub(1);
        let report = CarVerifier::verify_block_car(
            &bundle.manifest,
            &bundle.plan,
            segment,
            Some(inclusive),
        )?;
        if report.payload_bytes != expected_range.len() {
            return Err(GatewayError::RangeLengthMismatch {
                _expected: expected_range.len(),
                _actual: report.payload_bytes,
            });
        }
    }

    Ok(())
}

fn verify_misaligned_range<C: GatewayHttpClient>(client: &C) -> Result<(), GatewayError> {
    client.ensure_required_headers()?;
    let bundle = canonical_fixture_bundle();
    let range = first_chunk_range(&bundle.plan).expect("fixture plan has chunks");
    let misaligned = ByteRange::new(range.start + 1, range.end);
    match client.stream_range(misaligned) {
        Err(GatewayError::RangeNotSatisfiable) => Err(GatewayError::RangeNotSatisfiable),
        Err(other) => Err(other),
        Ok(_) => Err(GatewayError::RangeUnexpectedSuccess),
    }
}

#[test]
fn multipart_roundtrip_parses_expected_segments() {
    let ranges = [ByteRange::new(0, 8), ByteRange::new(8, 16)];
    let segments = vec![b"ABCDEFGH".to_vec(), b"IJKLMNOP".to_vec()];
    let response = build_multipart_response(&ranges, &segments, 32);
    let parsed = parse_multipart_byteranges(&response.body, &response.boundary, 32).expect("parse");
    assert_eq!(parsed.len(), ranges.len());
    for (idx, (range, data)) in parsed.iter().enumerate() {
        assert_eq!(range.start, ranges[idx].start);
        assert_eq!(range.end, ranges[idx].end);
        assert_eq!(data, &segments[idx]);
    }
}

#[test]
fn multipart_parser_rejects_truncated_body() {
    let ranges = [ByteRange::new(0, 4)];
    let segments = vec![b"PING".to_vec()];
    let response = build_multipart_response(&ranges, &segments, 8);
    let truncated = &response.body[..response.body.len().saturating_sub(3)];
    let result = parse_multipart_byteranges(truncated, &response.boundary, 8);
    assert!(matches!(result, Err(GatewayError::MultipartParse)));
}
