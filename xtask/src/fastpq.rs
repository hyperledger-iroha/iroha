use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use blake3::hash as blake3_hash;
use eyre::{Context, Result, bail, ensure, eyre};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json as serde_json,
    json::{self, Value},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339, macros::format_description};

use crate::workspace_root;

#[derive(Debug, Clone)]
pub struct BenchInput {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct BenchManifestOptions {
    pub benches: Vec<BenchInput>,
    pub output: PathBuf,
    pub signing_key: Option<PathBuf>,
    pub require_rows: Option<u64>,
    pub max_operation_ms: BTreeMap<String, f64>,
    pub min_operation_speedup: BTreeMap<String, f64>,
    pub matrix_manifest: Option<PathBuf>,
    pub label_max_operation_ms: BTreeMap<String, BTreeMap<String, f64>>,
    pub label_min_operation_speedup: BTreeMap<String, BTreeMap<String, f64>>,
}

impl Default for BenchManifestOptions {
    fn default() -> Self {
        Self {
            benches: Vec::new(),
            output: default_manifest_path(),
            signing_key: None,
            require_rows: None,
            max_operation_ms: BTreeMap::new(),
            min_operation_speedup: BTreeMap::new(),
            matrix_manifest: None,
            label_max_operation_ms: BTreeMap::new(),
            label_min_operation_speedup: BTreeMap::new(),
        }
    }
}

pub fn default_manifest_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("fastpq_bench_manifest.json")
}

pub fn default_matrix_manifest_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("fastpq_benchmarks")
        .join("matrix")
        .join("matrix_manifest.json")
}

pub fn default_stage_profile_dir() -> PathBuf {
    let timestamp = stage_profile_timestamp();
    workspace_root()
        .join("artifacts")
        .join("fastpq_stage_profiles")
        .join(timestamp)
}

#[derive(Serialize, JsonSerialize)]
struct BenchHashes {
    blake3_hex: String,
    sha256_hex: String,
}

#[derive(Serialize, Default, JsonSerialize)]
struct BenchMetadata {
    generated_at: Option<String>,
    host: Option<String>,
    platform: Option<String>,
    machine: Option<String>,
    command: Option<String>,
    notes: Option<String>,
}

#[derive(Serialize, JsonSerialize)]
struct BenchEntry {
    label: String,
    path: String,
    rows: u64,
    padded_rows: Option<u64>,
    iterations: Option<u64>,
    warmups: Option<u64>,
    gpu_backend: Option<String>,
    gpu_available: Option<bool>,
    poseidon_microbench: Option<PoseidonMicrobenchSummary>,
    metadata: BenchMetadata,
    hashes: BenchHashes,
}

#[derive(Serialize, Default, JsonSerialize)]
struct PoseidonMicrobenchSample {
    mean_ms: Option<f64>,
    min_ms: Option<f64>,
    max_ms: Option<f64>,
    columns: Option<u64>,
    trace_log2: Option<u64>,
    states: Option<u64>,
    warmups: Option<u64>,
    iterations: Option<u64>,
    threadgroup_lanes: Option<u32>,
    states_per_lane: Option<u32>,
}

impl PoseidonMicrobenchSample {
    fn is_empty(&self) -> bool {
        self.mean_ms.is_none()
            && self.min_ms.is_none()
            && self.max_ms.is_none()
            && self.columns.is_none()
            && self.trace_log2.is_none()
            && self.states.is_none()
            && self.warmups.is_none()
            && self.iterations.is_none()
            && self.threadgroup_lanes.is_none()
            && self.states_per_lane.is_none()
    }
}

#[derive(Serialize, Default, JsonSerialize)]
struct PoseidonMicrobenchSummary {
    default: Option<PoseidonMicrobenchSample>,
    scalar_lane: Option<PoseidonMicrobenchSample>,
    speedup_vs_scalar: Option<f64>,
}

#[derive(Serialize, JsonSerialize)]
struct ConstraintSummary {
    require_rows: Option<u64>,
    max_operation_ms: BTreeMap<String, f64>,
    min_operation_speedup: BTreeMap<String, f64>,
}

#[derive(Serialize, JsonSerialize)]
struct BenchManifestPayload {
    version: u32,
    generated_unix_ms: u64,
    generator_commit: String,
    benches: Vec<BenchEntry>,
    constraints: ConstraintSummary,
}

#[derive(Serialize, JsonSerialize)]
struct SignatureEnvelope {
    algorithm: String,
    public_key_hex: String,
    signature_hex: String,
}

#[derive(Serialize, JsonSerialize)]
struct SignedBenchManifest {
    payload: BenchManifestPayload,
    signature: Option<SignatureEnvelope>,
}

#[derive(Clone, Copy, Debug)]
struct OperationStats {
    gpu_mean_ms: Option<f64>,
    speedup_ratio: Option<f64>,
}

impl OperationStats {
    fn from_value(value: &Value) -> OperationStats {
        let gpu_mean_ms = value.get("gpu_mean_ms").and_then(|v| v.as_f64());
        let speedup_ratio = value.get("speedup_ratio").and_then(|v| v.as_f64());
        OperationStats {
            gpu_mean_ms,
            speedup_ratio,
        }
    }
}

impl BenchManifestOptions {
    fn effective_max_operation_ms(&self, label: &str) -> BTreeMap<String, f64> {
        let mut map = self.max_operation_ms.clone();
        if let Some(per_label) = self.label_max_operation_ms.get(label) {
            if map.is_empty() {
                return per_label.clone();
            }
            for (operation, limit) in per_label {
                map.insert(operation.clone(), *limit);
            }
        }
        map
    }

    fn effective_min_operation_speedup(&self, label: &str) -> BTreeMap<String, f64> {
        let mut map = self.min_operation_speedup.clone();
        if let Some(per_label) = self.label_min_operation_speedup.get(label) {
            if map.is_empty() {
                return per_label.clone();
            }
            for (operation, limit) in per_label {
                map.insert(operation.clone(), *limit);
            }
        }
        map
    }
}

#[derive(Debug, Deserialize, JsonDeserialize)]
struct MatrixManifest {
    version: u32,
    #[serde(default)]
    #[norito(default)]
    require_rows: Option<u64>,
    #[serde(default)]
    #[norito(default)]
    max_operation_ms: BTreeMap<String, f64>,
    #[serde(default)]
    #[norito(default)]
    min_operation_speedup: BTreeMap<String, f64>,
    #[serde(default)]
    #[norito(default)]
    devices: Vec<MatrixDeviceEntry>,
}

#[derive(Debug, Deserialize, JsonDeserialize)]
struct MatrixDeviceEntry {
    label: String,
    #[serde(default)]
    #[norito(default)]
    max_operation_ms: BTreeMap<String, f64>,
    #[serde(default)]
    #[norito(default)]
    min_operation_speedup: BTreeMap<String, f64>,
}

fn apply_matrix_manifest(options: &mut BenchManifestOptions, manifest_path: &Path) -> Result<()> {
    let bytes = fs::read(manifest_path)
        .with_context(|| format!("read matrix manifest {}", manifest_path.display()))?;
    let manifest: MatrixManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse matrix manifest {}", manifest_path.display()))?;
    ensure!(
        manifest.version == 1,
        "matrix manifest {} has unsupported version {}",
        manifest_path.display(),
        manifest.version
    );
    if options.require_rows.is_none() {
        options.require_rows = manifest.require_rows;
    }
    if options.max_operation_ms.is_empty() && !manifest.max_operation_ms.is_empty() {
        options.max_operation_ms = manifest.max_operation_ms.clone();
    }
    if options.min_operation_speedup.is_empty() && !manifest.min_operation_speedup.is_empty() {
        options.min_operation_speedup = manifest.min_operation_speedup.clone();
    }
    for device in &manifest.devices {
        if !device.max_operation_ms.is_empty() {
            options
                .label_max_operation_ms
                .insert(device.label.clone(), device.max_operation_ms.clone());
        }
        if !device.min_operation_speedup.is_empty() {
            options
                .label_min_operation_speedup
                .insert(device.label.clone(), device.min_operation_speedup.clone());
        }
    }
    Ok(())
}

pub fn write_bench_manifest(mut options: BenchManifestOptions) -> Result<()> {
    if let Some(path) = options.matrix_manifest.clone() {
        apply_matrix_manifest(&mut options, &path)?;
    }
    if options.benches.is_empty() {
        bail!("fastpq-bench-manifest requires at least one --bench label=path argument");
    }

    let mut entries = Vec::with_capacity(options.benches.len());
    for bench in &options.benches {
        let entry = parse_bench_entry(bench, &options)?;
        entries.push(entry);
    }

    let payload = BenchManifestPayload {
        version: 1,
        generated_unix_ms: current_unix_ms(),
        generator_commit: current_commit().unwrap_or_else(|_| "unknown".to_string()),
        benches: entries,
        constraints: ConstraintSummary {
            require_rows: options.require_rows,
            max_operation_ms: options.max_operation_ms.clone(),
            min_operation_speedup: options.min_operation_speedup.clone(),
        },
    };

    let payload_bytes = serde_json::to_vec(&payload).context("serialize bench manifest payload")?;
    let signature = if let Some(key_path) = options.signing_key.as_ref() {
        Some(sign_manifest(&payload_bytes, key_path)?)
    } else {
        None
    };

    let signed = SignedBenchManifest { payload, signature };
    let json = serde_json::to_json_pretty(&signed).context("serialize signed manifest")?;

    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    fs::write(&options.output, json)
        .with_context(|| format!("write {}", options.output.display()))?;

    if let Ok(rel) = options.output.strip_prefix(workspace_root()) {
        println!("wrote {}", rel.display());
    } else {
        println!("wrote {}", options.output.display());
    }

    Ok(())
}

fn parse_bench_entry(bench: &BenchInput, constraints: &BenchManifestOptions) -> Result<BenchEntry> {
    let bytes = fs::read(&bench.path)
        .with_context(|| format!("read benchmark {}", bench.path.display()))?;
    let blake3_hex = blake3_hash(&bytes).to_hex().to_string();
    let sha256_hex = hex::encode(Sha256::digest(&bytes));

    let bundle: Value = json::from_slice(&bytes)
        .with_context(|| format!("decode JSON from {}", bench.path.display()))?;
    let metadata_value = bundle
        .get("metadata")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let benchmarks_value = bundle
        .get("benchmarks")
        .and_then(|v| v.as_object())
        .ok_or_else(|| eyre!("benchmarks block missing in {}", bench.path.display()))?;

    let rows = benchmarks_value
        .get("rows")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            eyre!(
                "benchmarks.rows missing or invalid in {}",
                bench.path.display()
            )
        })?;

    if let Some(required) = constraints.require_rows {
        ensure!(
            rows >= required,
            "bench `{}` rows {} below required threshold {}",
            bench.label,
            rows,
            required
        );
    }

    let operations = benchmarks_value
        .get("operations")
        .and_then(|v| v.as_array())
        .ok_or_else(|| eyre!("benchmarks.operations missing in {}", bench.path.display()))?;
    let operation_map = build_operation_map(operations);

    let effective_max = constraints.effective_max_operation_ms(&bench.label);
    let effective_min = constraints.effective_min_operation_speedup(&bench.label);

    enforce_operation_limits(
        &bench.label,
        &operation_map,
        &effective_max,
        |stats| stats.gpu_mean_ms,
        "gpu_mean_ms",
        "max-operation-ms",
    )?;
    enforce_operation_limits(
        &bench.label,
        &operation_map,
        &effective_min,
        |stats| stats.speedup_ratio,
        "speedup_ratio",
        "min-operation-speedup",
    )?;

    let metadata = BenchMetadata {
        generated_at: metadata_value
            .get("generated_at")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        host: metadata_value
            .get("host")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        platform: metadata_value
            .get("platform")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        machine: metadata_value
            .get("machine")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        command: metadata_value
            .get("command")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        notes: metadata_value
            .get("notes")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
    };

    let benchmark_path = display_path(&bench.path);
    let gpu_backend = benchmarks_value
        .get("gpu_backend")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned);
    let poseidon_microbench =
        parse_poseidon_microbench_summary(benchmarks_value.get("poseidon_microbench"));
    if gpu_backend.as_deref() == Some("metal") && poseidon_microbench.is_none() {
        bail!(
            "bench `{}` (label {}) missing `benchmarks.poseidon_microbench`; rerun scripts/fastpq/wrap_benchmark.py so Poseidon microbench evidence is captured",
            bench.path.display(),
            bench.label
        );
    }

    Ok(BenchEntry {
        label: bench.label.clone(),
        path: benchmark_path,
        rows,
        padded_rows: benchmarks_value.get("padded_rows").and_then(|v| v.as_u64()),
        iterations: benchmarks_value.get("iterations").and_then(|v| v.as_u64()),
        warmups: benchmarks_value.get("warmups").and_then(|v| v.as_u64()),
        gpu_backend,
        gpu_available: benchmarks_value
            .get("gpu_available")
            .and_then(|v| v.as_bool()),
        poseidon_microbench,
        metadata,
        hashes: BenchHashes {
            blake3_hex,
            sha256_hex,
        },
    })
}

fn parse_poseidon_microbench_summary(value: Option<&Value>) -> Option<PoseidonMicrobenchSummary> {
    let obj = value?.as_object()?;
    let default_sample = obj
        .get("default")
        .and_then(parse_poseidon_microbench_sample);
    let scalar_sample = obj
        .get("scalar_lane")
        .and_then(parse_poseidon_microbench_sample);
    let speedup = obj.get("speedup_vs_scalar").and_then(|v| v.as_f64());
    if default_sample.is_none() && scalar_sample.is_none() && speedup.is_none() {
        return None;
    }
    Some(PoseidonMicrobenchSummary {
        default: default_sample,
        scalar_lane: scalar_sample,
        speedup_vs_scalar: speedup,
    })
}

fn parse_poseidon_microbench_sample(value: &Value) -> Option<PoseidonMicrobenchSample> {
    let obj = value.as_object()?;
    let sample = PoseidonMicrobenchSample {
        mean_ms: obj.get("mean_ms").and_then(|v| v.as_f64()),
        min_ms: obj.get("min_ms").and_then(|v| v.as_f64()),
        max_ms: obj.get("max_ms").and_then(|v| v.as_f64()),
        columns: obj.get("columns").and_then(|v| v.as_u64()),
        trace_log2: obj.get("trace_log2").and_then(|v| v.as_u64()),
        states: obj.get("states").and_then(|v| v.as_u64()),
        warmups: obj.get("warmups").and_then(|v| v.as_u64()),
        iterations: obj.get("iterations").and_then(|v| v.as_u64()),
        threadgroup_lanes: obj
            .get("tuning")
            .and_then(|t| t.get("threadgroup_lanes"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok()),
        states_per_lane: obj
            .get("tuning")
            .and_then(|t| t.get("states_per_lane"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok()),
    };
    if sample.is_empty() {
        None
    } else {
        Some(sample)
    }
}

fn build_operation_map(entries: &[Value]) -> BTreeMap<String, OperationStats> {
    let mut map = BTreeMap::new();
    for entry in entries {
        if let Some(name) = entry.get("operation").and_then(|v| v.as_str()) {
            map.insert(name.to_string(), OperationStats::from_value(entry));
        }
    }
    map
}

fn enforce_operation_limits<F>(
    label: &str,
    operations: &BTreeMap<String, OperationStats>,
    limits: &BTreeMap<String, f64>,
    extractor: F,
    field_name: &str,
    flag_name: &str,
) -> Result<()>
where
    F: Fn(&OperationStats) -> Option<f64>,
{
    for (operation, limit) in limits {
        let stats = operations.get(operation).ok_or_else(|| {
            eyre!("bench `{label}` missing `{operation}` operation required by --{flag_name}")
        })?;
        let value = extractor(stats)
            .ok_or_else(|| eyre!("bench `{label}` operation `{operation}` missing {field_name}"))?;
        if flag_name == "max-operation-ms" {
            ensure!(
                value <= *limit,
                "bench `{label}` operation `{operation}` exceeded max {field_name} (value={value:.3} ms, limit={limit})"
            );
        } else {
            ensure!(
                value >= *limit,
                "bench `{label}` operation `{operation}` fell below min {field_name} (value={value:.3}, limit={limit})"
            );
        }
    }
    Ok(())
}

fn sign_manifest(payload: &[u8], key_path: &Path) -> Result<SignatureEnvelope> {
    let key_hex = fs::read_to_string(key_path)
        .with_context(|| format!("read signing key {}", key_path.display()))?;
    let cleaned: String = key_hex
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let private_key =
        PrivateKey::from_hex(Algorithm::Ed25519, &cleaned).context("parse signing key")?;
    let key_pair: KeyPair = private_key.clone().into();
    let signature = Signature::new(key_pair.private_key(), payload);
    let (algorithm, public_bytes) = key_pair.public_key().to_bytes();
    ensure!(
        algorithm == Algorithm::Ed25519,
        "only Ed25519 signing keys are supported"
    );
    Ok(SignatureEnvelope {
        algorithm: "ed25519".to_string(),
        public_key_hex: hex::encode(public_bytes),
        signature_hex: hex::encode(signature.payload()),
    })
}

fn display_path(path: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(workspace_root()) {
        rel.display().to_string()
    } else {
        path.display().to_string()
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn current_commit() -> Result<String> {
    let output = Command::new("git")
        .current_dir(workspace_root())
        .args(["rev-parse", "HEAD"])
        .output()
        .context("git rev-parse HEAD failed")?;
    if !output.status.success() {
        bail!("git rev-parse HEAD returned {}", output.status);
    }
    let text = String::from_utf8(output.stdout).context("rev-parse output")?;
    Ok(text.trim().to_string())
}

#[derive(Debug)]
pub struct StageProfileOptions {
    pub rows: usize,
    pub warmups: usize,
    pub iterations: usize,
    pub output_dir: PathBuf,
    pub release: bool,
    pub capture_trace: bool,
    pub trace_dir: Option<PathBuf>,
    pub trace_template: Option<String>,
    pub trace_seconds: Option<u32>,
    pub gpu_probe: bool,
    pub stages: Vec<StageKind>,
}

impl Default for StageProfileOptions {
    fn default() -> Self {
        Self {
            rows: 20_000,
            warmups: 1,
            iterations: 5,
            output_dir: default_stage_profile_dir(),
            release: true,
            capture_trace: false,
            trace_dir: None,
            trace_template: None,
            trace_seconds: None,
            gpu_probe: true,
            stages: vec![StageKind::Fft, StageKind::Lde, StageKind::Poseidon],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StageKind {
    Fft,
    Ifft,
    Lde,
    Poseidon,
}

impl StageKind {
    pub fn from_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "fft" => Some(Self::Fft),
            "ifft" => Some(Self::Ifft),
            "lde" => Some(Self::Lde),
            "poseidon" | "poseidon_hash_columns" | "poseidon-hash" => Some(Self::Poseidon),
            _ => None,
        }
    }

    fn spec(self) -> StageSpec {
        match self {
            StageKind::Fft => StageSpec {
                label: "fft",
                operation: "fft",
                dir: "fft",
            },
            StageKind::Ifft => StageSpec {
                label: "ifft",
                operation: "ifft",
                dir: "ifft",
            },
            StageKind::Lde => StageSpec {
                label: "lde",
                operation: "lde",
                dir: "lde",
            },
            StageKind::Poseidon => StageSpec {
                label: "poseidon",
                operation: "poseidon_hash_columns",
                dir: "poseidon",
            },
        }
    }
}

struct StageSpec {
    label: &'static str,
    operation: &'static str,
    dir: &'static str,
}

#[derive(JsonSerialize)]
struct StageProfileSummary {
    generated_at: String,
    rows: usize,
    warmups: usize,
    iterations: usize,
    release_build: bool,
    capture_trace: bool,
    stages: Vec<StageSummary>,
}

#[derive(JsonSerialize)]
struct StageSummary {
    stage: String,
    operation: String,
    benchmark_json: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    trace_artifact: Option<String>,
    stats: StageStats,
    #[norito(skip_serializing_if = "Option::is_none")]
    metal_dispatch_queue: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    column_staging: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    kernel_profiles: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    post_tile_dispatches: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    twiddle_cache: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    device_profile: Option<Value>,
    #[norito(skip_serializing_if = "Option::is_none")]
    metal_heuristics: Option<Value>,
}

#[derive(JsonSerialize)]
struct StageStats {
    cpu: StageStat,
    #[norito(skip_serializing_if = "Option::is_none")]
    gpu: Option<StageStat>,
    #[norito(skip_serializing_if = "Option::is_none")]
    speedup_ratio: Option<f64>,
    #[norito(skip_serializing_if = "Option::is_none")]
    speedup_delta_ms: Option<f64>,
    #[norito(skip_serializing_if = "Option::is_none")]
    columns: Option<u64>,
    #[norito(skip_serializing_if = "Option::is_none")]
    input_len: Option<u64>,
}

#[derive(JsonSerialize)]
struct StageStat {
    mean_ms: f64,
    min_ms: f64,
    max_ms: f64,
}

pub fn run_stage_profile(options: &StageProfileOptions) -> Result<PathBuf> {
    if options.stages.is_empty() {
        bail!("fastpq-stage-profile requires at least one --stage");
    }
    fs::create_dir_all(&options.output_dir)
        .with_context(|| format!("failed to create {}", options.output_dir.display()))?;
    let mut stages = Vec::new();
    for kind in dedup_stages(&options.stages) {
        let spec = kind.spec();
        let summary = run_stage(spec, options)?;
        stages.push(summary);
    }
    let generated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".into());
    let summary = StageProfileSummary {
        generated_at,
        rows: options.rows,
        warmups: options.warmups,
        iterations: options.iterations,
        release_build: options.release,
        capture_trace: options.capture_trace,
        stages,
    };
    let summary_path = options.output_dir.join("stage_profile_summary.json");
    let encoded =
        norito::json::to_vec_pretty(&summary).context("failed to encode stage profile summary")?;
    fs::write(&summary_path, encoded)
        .with_context(|| format!("failed to write {}", summary_path.display()))?;
    eprintln!(
        "fastpq-stage-profile: wrote {}",
        display_path(&summary_path)
    );
    Ok(summary_path)
}

fn run_stage(spec: StageSpec, options: &StageProfileOptions) -> Result<StageSummary> {
    eprintln!("fastpq-stage-profile: running {}", spec.label);
    let stage_dir = options.output_dir.join(spec.dir);
    fs::create_dir_all(&stage_dir)
        .with_context(|| format!("failed to create {}", stage_dir.display()))?;
    let bench_path = stage_dir.join("fastpq_metal_bench.json");
    let mut command = Command::new("cargo");
    command.current_dir(workspace_root());
    command.arg("run");
    if options.release {
        command.arg("--release");
    }
    command.args(["-p", "fastpq_prover", "--bin", "fastpq_metal_bench", "--"]);
    command.arg("--rows").arg(options.rows.to_string());
    command.arg("--warmups").arg(options.warmups.to_string());
    command
        .arg("--iterations")
        .arg(options.iterations.to_string());
    if options.gpu_probe {
        command.arg("--gpu-probe");
    }
    if options.capture_trace {
        let trace_root = options.trace_dir.as_ref().unwrap_or(&stage_dir);
        fs::create_dir_all(trace_root).with_context(|| {
            format!(
                "failed to create Metal trace directory {}",
                trace_root.display()
            )
        })?;
        command.arg("--trace-auto");
        command.arg("--trace-dir").arg(trace_root);
        if let Some(template) = &options.trace_template {
            command.arg("--trace-template").arg(template);
        }
        if let Some(seconds) = options.trace_seconds {
            command.arg("--trace-seconds").arg(seconds.to_string());
        }
    }
    command.arg("--operation").arg(spec.operation);
    command.arg("--output").arg(&bench_path);
    let status = command
        .status()
        .context("failed to run fastpq_metal_bench")?;
    ensure!(
        status.success(),
        "fastpq_metal_bench exited with {status} for stage {}",
        spec.label
    );
    let data = fs::read_to_string(&bench_path)
        .with_context(|| format!("failed to read {}", bench_path.display()))?;
    let payload: Value = json::from_str(&data)
        .with_context(|| format!("failed to parse {}", bench_path.display()))?;
    build_stage_summary(spec, &payload, &bench_path)
}

fn build_stage_summary(
    spec: StageSpec,
    payload: &Value,
    bench_path: &Path,
) -> Result<StageSummary> {
    let report = payload
        .get("report")
        .and_then(|value| value.as_object())
        .ok_or_else(|| eyre!("bench {} missing report block", bench_path.display()))?;
    let operations = report
        .get("operations")
        .and_then(|value| value.as_array())
        .ok_or_else(|| eyre!("bench {} missing report.operations", bench_path.display()))?;
    let entry = operations
        .iter()
        .find(|value| value.get("operation").and_then(|op| op.as_str()) == Some(spec.operation));
    let op_value =
        entry.ok_or_else(|| eyre!("bench {} missing {}", bench_path.display(), spec.operation))?;
    let stats = parse_stage_stats(op_value)
        .ok_or_else(|| eyre!("operation {} missing stats", spec.operation))?;
    let trace_artifact = report
        .get("metal_trace_output")
        .and_then(|value| value.as_str())
        .map(PathBuf::from)
        .map(|path| display_path(&path));
    Ok(StageSummary {
        stage: spec.label.to_string(),
        operation: spec.operation.to_string(),
        benchmark_json: display_path(bench_path),
        trace_artifact,
        stats,
        metal_dispatch_queue: report.get("metal_dispatch_queue").cloned(),
        column_staging: report.get("column_staging").cloned(),
        kernel_profiles: report.get("kernel_profiles").cloned(),
        post_tile_dispatches: report.get("post_tile_dispatches").cloned(),
        twiddle_cache: report.get("twiddle_cache").cloned(),
        device_profile: report.get("device_profile").cloned(),
        metal_heuristics: report.get("metal_heuristics").cloned(),
    })
}

fn parse_stage_stats(value: &Value) -> Option<StageStats> {
    let cpu = value.get("cpu")?.as_object()?;
    let cpu_stats = StageStat {
        mean_ms: cpu.get("mean_ms")?.as_f64()?,
        min_ms: cpu.get("min_ms")?.as_f64()?,
        max_ms: cpu.get("max_ms")?.as_f64()?,
    };
    let gpu_stats = value.get("gpu").and_then(|gpu| {
        let obj = gpu.as_object()?;
        Some(StageStat {
            mean_ms: obj.get("mean_ms")?.as_f64()?,
            min_ms: obj.get("min_ms")?.as_f64()?,
            max_ms: obj.get("max_ms")?.as_f64()?,
        })
    });
    let speedup_ratio = value
        .get("speedup")
        .and_then(|speedup| speedup.get("ratio"))
        .and_then(|ratio| ratio.as_f64());
    let speedup_delta_ms = value
        .get("speedup")
        .and_then(|speedup| speedup.get("delta_ms"))
        .and_then(|delta| delta.as_f64());
    Some(StageStats {
        cpu: cpu_stats,
        gpu: gpu_stats,
        speedup_ratio,
        speedup_delta_ms,
        columns: value.get("columns").and_then(|col| col.as_u64()),
        input_len: value.get("input_len").and_then(|len| len.as_u64()),
    })
}

fn dedup_stages(stages: &[StageKind]) -> Vec<StageKind> {
    let mut set = BTreeSet::new();
    for stage in stages {
        set.insert(*stage);
    }
    set.into_iter().collect()
}

fn stage_profile_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&format_description!(
            "[year][month][day]T[hour][minute][second]Z"
        ))
        .unwrap_or_else(|_| "unknown".into())
}

#[derive(Debug, Clone)]
pub struct CudaSuiteOptions {
    pub rows: usize,
    pub warmups: usize,
    pub iterations: usize,
    pub column_count: usize,
    pub require_gpu: bool,
    pub output: PathBuf,
    pub raw_output: PathBuf,
    pub wrap_output: bool,
    pub wrapper: PathBuf,
    pub require_lde_mean_ms: f64,
    pub require_poseidon_mean_ms: f64,
    pub labels: BTreeMap<String, String>,
    pub row_usage: Option<PathBuf>,
    pub device: Option<String>,
    pub notes: Option<String>,
    pub accel_instance: Option<String>,
    pub accel_state_json: Option<PathBuf>,
    pub accel_state_prom: Option<PathBuf>,
    pub sign_output: bool,
    pub gpg_key: Option<String>,
    pub dry_run: bool,
}

impl Default for CudaSuiteOptions {
    fn default() -> Self {
        let output = default_cuda_bench_output_path();
        let raw_output = default_cuda_raw_output(&output);
        Self {
            rows: 20_000,
            warmups: 1,
            iterations: 5,
            column_count: 16,
            require_gpu: false,
            output,
            raw_output,
            wrap_output: true,
            wrapper: default_wrapper_path(),
            require_lde_mean_ms: 950.0,
            require_poseidon_mean_ms: 1_000.0,
            labels: BTreeMap::new(),
            row_usage: None,
            device: None,
            notes: None,
            accel_instance: None,
            accel_state_json: None,
            accel_state_prom: None,
            sign_output: false,
            gpg_key: None,
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CudaSuiteResult {
    pub raw_output: PathBuf,
    pub wrapped_output: Option<PathBuf>,
    pub summary: PathBuf,
    pub dry_run: bool,
}

#[derive(Clone, JsonSerialize)]
struct RecordedCommand {
    program: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: String,
}

#[derive(JsonSerialize)]
struct CudaSuiteSummary {
    raw_output: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    wrapped_output: Option<String>,
    dry_run: bool,
    rows: usize,
    warmups: usize,
    iterations: usize,
    columns: usize,
    require_gpu: bool,
    require_lde_mean_ms: f64,
    require_poseidon_mean_ms: f64,
    #[norito(skip_serializing_if = "Option::is_none")]
    row_usage: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    device: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    #[norito(skip_serializing_if = "BTreeMap::is_empty")]
    labels: BTreeMap<String, String>,
    commands: Vec<RecordedCommand>,
}

struct CommandPlan {
    record: RecordedCommand,
    command: Command,
}

pub fn run_cuda_suite(options: &CudaSuiteOptions) -> Result<CudaSuiteResult> {
    fs::create_dir_all(
        options
            .output
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(workspace_root),
    )
    .with_context(|| format!("failed to create {}", options.output.display()))?;
    fs::create_dir_all(
        options
            .raw_output
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(workspace_root),
    )
    .with_context(|| format!("failed to create {}", options.raw_output.display()))?;

    let mut bench = build_cuda_bench_command(options)?;
    let bench_record = bench.record.clone();
    let mut wrap = options
        .wrap_output
        .then(|| build_wrap_command(options))
        .transpose()?;
    let wrap_record = wrap.as_ref().map(|plan| plan.record.clone());

    let mut commands = Vec::new();
    if options.dry_run {
        commands.push(bench_record);
        if let Some(plan) = wrap_record {
            commands.push(plan);
        }
        let summary = write_cuda_suite_summary(options, &commands)?;
        eprintln!(
            "fastpq-cuda-suite: dry run wrote plan {}",
            display_path(&summary)
        );
        return Ok(CudaSuiteResult {
            raw_output: options.raw_output.clone(),
            wrapped_output: wrap.as_ref().map(|_| options.output.clone()),
            summary,
            dry_run: true,
        });
    }

    run_command(&mut bench)?;
    commands.push(bench_record);

    if let Some(plan) = wrap.as_mut() {
        run_command(plan)?;
        if let Some(record) = wrap_record {
            commands.push(record);
        }
    }

    let summary = write_cuda_suite_summary(options, &commands)?;
    eprintln!("fastpq-cuda-suite: wrote plan {}", display_path(&summary));
    Ok(CudaSuiteResult {
        raw_output: options.raw_output.clone(),
        wrapped_output: options.wrap_output.then(|| options.output.clone()),
        summary,
        dry_run: false,
    })
}

fn write_cuda_suite_summary(
    options: &CudaSuiteOptions,
    commands: &[RecordedCommand],
) -> Result<PathBuf> {
    let summary_path = plan_summary_path(&options.output);
    let summary_dir = summary_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(workspace_root);
    fs::create_dir_all(&summary_dir)
        .with_context(|| format!("failed to create {}", summary_dir.display()))?;
    let summary = CudaSuiteSummary {
        raw_output: display_path(&options.raw_output),
        wrapped_output: options.wrap_output.then(|| display_path(&options.output)),
        dry_run: options.dry_run,
        rows: options.rows,
        warmups: options.warmups,
        iterations: options.iterations,
        columns: options.column_count,
        require_gpu: options.require_gpu,
        require_lde_mean_ms: options.require_lde_mean_ms,
        require_poseidon_mean_ms: options.require_poseidon_mean_ms,
        row_usage: options.row_usage.as_deref().map(display_path),
        device: options.device.clone(),
        notes: options.notes.clone(),
        labels: options.labels.clone(),
        commands: commands.to_vec(),
    };
    let encoded =
        norito::json::to_vec_pretty(&summary).context("failed to encode CUDA suite summary")?;
    fs::write(&summary_path, encoded)
        .with_context(|| format!("failed to write {}", summary_path.display()))?;
    Ok(summary_path)
}

fn run_command(plan: &mut CommandPlan) -> Result<()> {
    let status = plan.command.status().context("failed to execute command")?;
    ensure!(
        status.success(),
        "command exited with {status} ({})",
        plan.record.program
    );
    Ok(())
}

fn build_cuda_bench_command(options: &CudaSuiteOptions) -> Result<CommandPlan> {
    let mut args = vec![
        "run".to_owned(),
        "--release".to_owned(),
        "-p".to_owned(),
        "fastpq_prover".to_owned(),
        "--bin".to_owned(),
        "fastpq_cuda_bench".to_owned(),
        "--".to_owned(),
        "--rows".to_owned(),
        options.rows.to_string(),
        "--iterations".to_owned(),
        options.iterations.to_string(),
        "--warmups".to_owned(),
        options.warmups.to_string(),
        "--column-count".to_owned(),
        options.column_count.to_string(),
        "--output".to_owned(),
        display_path(&options.raw_output),
    ];
    if let Some(row_usage) = &options.row_usage {
        args.push("--row-usage".to_owned());
        args.push(display_path(row_usage));
    }
    if let Some(device) = &options.device {
        args.push("--device".to_owned());
        args.push(device.clone());
    }
    if let Some(notes) = &options.notes {
        args.push("--notes".to_owned());
        args.push(notes.clone());
    }
    if options.require_gpu {
        args.push("--require-gpu".to_owned());
    }

    let mut env = BTreeMap::new();
    env.insert("FASTPQ_GPU".into(), "gpu".into());

    let workspace = workspace_root();
    let mut command = Command::new("cargo");
    command.current_dir(&workspace);
    command.args(args.iter());
    for (key, value) in &env {
        command.env(key, value);
    }

    Ok(CommandPlan {
        record: RecordedCommand {
            program: "cargo".into(),
            args,
            env,
            cwd: display_path(&workspace),
        },
        command,
    })
}

fn build_wrap_command(options: &CudaSuiteOptions) -> Result<CommandPlan> {
    let mut args = vec![
        display_path(&options.wrapper),
        "--require-lde-mean-ms".to_owned(),
        options.require_lde_mean_ms.to_string(),
        "--require-poseidon-mean-ms".to_owned(),
        options.require_poseidon_mean_ms.to_string(),
    ];
    if let Some(row_usage) = &options.row_usage {
        args.push("--row-usage".to_owned());
        args.push(display_path(row_usage));
    }
    if let Some(instance) = &options.accel_instance {
        args.push("--accel-instance".to_owned());
        args.push(instance.clone());
    }
    if let Some(path) = &options.accel_state_json {
        args.push("--accel-state-json".to_owned());
        args.push(display_path(path));
    }
    if let Some(path) = &options.accel_state_prom {
        args.push("--accel-state-prom".to_owned());
        args.push(display_path(path));
    }
    for (key, value) in &options.labels {
        args.push("--label".to_owned());
        args.push(format!("{key}={value}"));
    }
    if options.sign_output {
        args.push("--sign-output".to_owned());
    }
    if let Some(key) = &options.gpg_key {
        args.push("--gpg-key".to_owned());
        args.push(key.clone());
    }
    args.push(display_path(&options.raw_output));
    args.push(display_path(&options.output));

    let workspace = workspace_root();
    let mut command = Command::new("python3");
    command.current_dir(&workspace);
    command.args(args.iter());

    Ok(CommandPlan {
        record: RecordedCommand {
            program: "python3".into(),
            args,
            env: BTreeMap::new(),
            cwd: display_path(&workspace),
        },
        command,
    })
}

fn plan_summary_path(output: &Path) -> PathBuf {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(workspace_root);
    let stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("fastpq_cuda_bench");
    parent.join(format!("{stem}_plan.json"))
}

pub fn default_cuda_bench_output_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("fastpq_benchmarks")
        .join(format!(
            "fastpq_cuda_bench_{}.json",
            stage_profile_timestamp()
        ))
}

pub fn default_cuda_raw_output(output: &Path) -> PathBuf {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(workspace_root);
    let stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("fastpq_cuda_bench");
    parent.join(format!("{stem}_raw.json"))
}

fn default_wrapper_path() -> PathBuf {
    workspace_root()
        .join("scripts")
        .join("fastpq")
        .join("wrap_benchmark.py")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;

    fn sample_bundle(rows: u64) -> Value {
        norito::json!({
            "metadata": {
                "generated_at": "2026-02-01T12:00:00Z",
                "host": "ci-host",
                "platform": "macOS-15-arm64",
                "machine": "arm64",
                "command": "FASTPQ_METAL_LIB=... fastpq_metal_bench --rows 20000",
                "notes": "Metal backend active"
            },
            "benchmarks": {
                "rows": rows,
                "padded_rows": 32768,
                "iterations": 3,
                "warmups": 1,
                "gpu_backend": "metal",
                "gpu_available": true,
                "poseidon_microbench": {
                    "default": {
                        "mean_ms": 1.0,
                        "iterations": 1,
                        "warmups": 0,
                        "columns": 1,
                        "trace_log2": 1,
                        "states": 1
                    }
                },
                "operations": [
                    {
                        "operation": "fft",
                        "gpu_mean_ms": 420.0,
                        "speedup_ratio": 1.2
                    },
                    {
                        "operation": "lde",
                        "gpu_mean_ms": 800.0,
                        "speedup_ratio": 1.05
                    }
                ]
            },
            "report": {}
        })
    }

    fn write_bundle(temp: &TempDir, name: &str, rows: u64) -> PathBuf {
        let path = temp.path().join(name);
        let json = sample_bundle(rows);
        fs::write(&path, norito::json::to_vec_pretty(&json).unwrap()).unwrap();
        path
    }

    #[test]
    fn build_stage_summary_extracts_stats() {
        let payload = norito::json!({
            "report": {
                "operations": [{
                    "operation": "fft",
                    "columns": 16,
                    "input_len": 32768,
                    "cpu": { "mean_ms": 10.0, "min_ms": 9.5, "max_ms": 10.5 },
                    "gpu": { "mean_ms": 8.0, "min_ms": 7.5, "max_ms": 8.5 },
                    "speedup": { "ratio": 1.25, "delta_ms": 2.0 }
                }],
                "metal_dispatch_queue": { "limit": 4, "dispatch_count": 32 },
                "column_staging": { "idle_ms": 1.0 },
                "kernel_profiles": [{ "name": "fft", "mean_ms": 10.0 }]
            }
        });
        let spec = StageSpec {
            label: "fft",
            operation: "fft",
            dir: "fft",
        };
        let summary = build_stage_summary(spec, &payload, Path::new("fft.json")).expect("summary");
        assert_eq!(summary.stage, "fft");
        assert_eq!(summary.operation, "fft");
        assert_eq!(summary.stats.columns, Some(16));
        assert_eq!(summary.stats.input_len, Some(32_768));
        assert!(summary.metal_dispatch_queue.is_some());
        assert!(summary.column_staging.is_some());
        assert!(summary.kernel_profiles.is_some());
        assert!(summary.stats.gpu.is_some());
    }

    #[test]
    fn dedup_stages_orders_unique() {
        let list = vec![
            StageKind::Fft,
            StageKind::Poseidon,
            StageKind::Fft,
            StageKind::Lde,
            StageKind::Poseidon,
        ];
        let deduped = dedup_stages(&list);
        assert_eq!(
            deduped,
            vec![StageKind::Fft, StageKind::Lde, StageKind::Poseidon]
        );
    }

    #[test]
    fn manifest_writes_with_constraints() {
        let temp = TempDir::new().expect("tempdir");
        let bench_path = write_bundle(&temp, "metal.json", 20_000);
        let mut options = BenchManifestOptions {
            benches: vec![BenchInput {
                label: "metal".into(),
                path: bench_path,
            }],
            output: temp.path().join("manifest.json"),
            signing_key: None,
            require_rows: Some(19_000),
            max_operation_ms: [("lde".into(), 900.0)].into(),
            min_operation_speedup: [("fft".into(), 1.1)].into(),
            matrix_manifest: None,
            label_max_operation_ms: BTreeMap::new(),
            label_min_operation_speedup: BTreeMap::new(),
        };
        options.max_operation_ms.extend([("fft".into(), 500.0)]);

        write_bench_manifest(options).expect("manifest succeeds");

        let manifest_text =
            fs::read_to_string(temp.path().join("manifest.json")).expect("read manifest");
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest_text).expect("manifest json");
        let benches = manifest
            .get("payload")
            .and_then(|p| p.get("benches"))
            .and_then(|b| b.as_array())
            .expect("benches array");
        assert_eq!(benches.len(), 1, "expected single bench entry");
        assert_eq!(
            benches[0]
                .get("label")
                .and_then(|v| v.as_str())
                .expect("bench label"),
            "metal"
        );
        assert_eq!(
            manifest
                .get("payload")
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_u64())
                .expect("manifest version"),
            1
        );
    }

    #[test]
    fn manifest_rejects_bad_threshold() {
        let temp = TempDir::new().expect("tempdir");
        let bench_path = write_bundle(&temp, "metal.json", 18_000);
        let options = BenchManifestOptions {
            benches: vec![BenchInput {
                label: "metal".into(),
                path: bench_path,
            }],
            output: temp.path().join("manifest.json"),
            signing_key: None,
            require_rows: Some(20_000),
            max_operation_ms: BTreeMap::new(),
            min_operation_speedup: BTreeMap::new(),
            matrix_manifest: None,
            label_max_operation_ms: BTreeMap::new(),
            label_min_operation_speedup: BTreeMap::new(),
        };
        let err = write_bench_manifest(options).expect_err("rows threshold fails");
        assert!(
            err.to_string()
                .contains("rows 18000 below required threshold")
        );
    }

    #[test]
    fn matrix_constraints_enforced() {
        let temp = TempDir::new().expect("tempdir");
        let bench_path = write_bundle(&temp, "metal.json", 20_000);
        let manifest_path = temp.path().join("matrix_manifest.json");
        let manifest_value = norito::json!({
            "version": 1,
            "require_rows": 20000,
            "devices": [{
                "label": "metal",
                "max_operation_ms": { "lde": 1600.0 },
                "min_operation_speedup": { "fft": 1.05 }
            }]
        });
        fs::write(
            &manifest_path,
            norito::json::to_vec_pretty(&manifest_value).unwrap(),
        )
        .unwrap();

        let options = BenchManifestOptions {
            benches: vec![BenchInput {
                label: "metal".into(),
                path: bench_path,
            }],
            output: temp.path().join("manifest.json"),
            signing_key: None,
            require_rows: None,
            max_operation_ms: BTreeMap::new(),
            min_operation_speedup: BTreeMap::new(),
            matrix_manifest: Some(manifest_path.clone()),
            label_max_operation_ms: BTreeMap::new(),
            label_min_operation_speedup: BTreeMap::new(),
        };
        write_bench_manifest(options).expect("matrix constraints pass");

        let mut slow_bundle = sample_bundle(20_000);
        if let Some(entries) = slow_bundle
            .get_mut("benchmarks")
            .and_then(|b| b.get_mut("operations"))
            .and_then(|ops| ops.as_array_mut())
        {
            for entry in entries {
                if entry
                    .get("operation")
                    .and_then(|v| v.as_str())
                    .is_some_and(|name| name == "lde")
                    && let Some(map) = entry.as_object_mut()
                {
                    map.insert("gpu_mean_ms".into(), norito::json!(2_500.0));
                }
            }
        }
        let slow_path = temp.path().join("metal_slow.json");
        fs::write(
            &slow_path,
            norito::json::to_vec_pretty(&slow_bundle).unwrap(),
        )
        .unwrap();
        let failing = BenchManifestOptions {
            benches: vec![BenchInput {
                label: "metal".into(),
                path: slow_path,
            }],
            output: temp.path().join("manifest_slow.json"),
            signing_key: None,
            require_rows: None,
            max_operation_ms: BTreeMap::new(),
            min_operation_speedup: BTreeMap::new(),
            matrix_manifest: Some(manifest_path),
            label_max_operation_ms: BTreeMap::new(),
            label_min_operation_speedup: BTreeMap::new(),
        };
        let err = write_bench_manifest(failing).expect_err("matrix threshold should fail");
        assert!(
            err.to_string().contains("exceeded max gpu_mean_ms"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn cuda_suite_defaults_land_in_artifacts_dir() {
        let defaults = CudaSuiteOptions::default();
        let output = display_path(&defaults.output);
        assert!(
            output.contains("artifacts/fastpq_benchmarks/fastpq_cuda_bench_"),
            "unexpected default output path: {output}"
        );
        assert_eq!(
            defaults
                .raw_output
                .parent()
                .map(display_path)
                .unwrap_or_default(),
            defaults
                .output
                .parent()
                .map(display_path)
                .unwrap_or_default(),
            "raw output should share parent directory"
        );
    }

    #[test]
    fn cuda_bench_command_builds_expected_args() {
        let temp = TempDir::new().expect("tempdir");
        let options = CudaSuiteOptions {
            rows: 128,
            warmups: 0,
            iterations: 2,
            column_count: 4,
            require_gpu: true,
            output: temp.path().join("wrapped.json"),
            raw_output: temp.path().join("raw.json"),
            wrap_output: false,
            wrapper: default_wrapper_path(),
            require_lde_mean_ms: 950.0,
            require_poseidon_mean_ms: 1_000.0,
            labels: BTreeMap::new(),
            row_usage: Some(temp.path().join("row_usage.json")),
            device: Some("gpu0".into()),
            notes: Some("test run".into()),
            accel_instance: None,
            accel_state_json: None,
            accel_state_prom: None,
            sign_output: false,
            gpg_key: None,
            dry_run: false,
        };
        fs::write(
            options.row_usage.as_ref().unwrap(),
            norito::json::to_vec_pretty(&norito::json!({ "batches": [] })).unwrap(),
        )
        .expect("write row usage");

        let plan = build_cuda_bench_command(&options).expect("build bench");
        let args: Vec<_> = plan.record.args.clone();
        assert!(args.contains(&"--rows".to_string()));
        assert!(args.contains(&"128".to_string()));
        assert!(args.contains(&"--require-gpu".to_string()));
        assert!(
            plan.record
                .env
                .get("FASTPQ_GPU")
                .is_some_and(|value| value == "gpu"),
            "FASTPQ_GPU env should be set"
        );
    }

    #[test]
    fn cuda_wrap_command_applies_labels_and_thresholds() {
        let temp = TempDir::new().expect("tempdir");
        let mut options = CudaSuiteOptions {
            output: temp.path().join("wrapped.json"),
            raw_output: temp.path().join("raw.json"),
            accel_instance: Some("xeon-rtx".into()),
            require_lde_mean_ms: 777.0,
            require_poseidon_mean_ms: 888.0,
            row_usage: Some(temp.path().join("row_usage.json")),
            ..CudaSuiteOptions::default()
        };
        options
            .labels
            .insert("device_class".into(), "xeon-rtx".into());

        let plan = build_wrap_command(&options).expect("wrap plan");
        let args = plan.record.args.join(" ");
        assert!(args.contains("--require-lde-mean-ms 777"));
        assert!(args.contains("--require-poseidon-mean-ms 888"));
        assert!(args.contains("device_class=xeon-rtx"));
        assert!(args.contains("--accel-instance xeon-rtx"));
    }

    #[test]
    fn cuda_suite_dry_run_writes_summary() {
        let temp = TempDir::new().expect("tempdir");
        let options = CudaSuiteOptions {
            output: temp.path().join("wrapped.json"),
            raw_output: temp.path().join("raw.json"),
            wrapper: default_wrapper_path(),
            wrap_output: false,
            rows: 16,
            iterations: 1,
            warmups: 0,
            dry_run: true,
            ..CudaSuiteOptions::default()
        };

        let result = run_cuda_suite(&options).expect("run dry suite");
        assert!(result.summary.exists());
        let summary_text = fs::read_to_string(&result.summary).expect("read summary");
        let value: Value = norito::json::from_str(&summary_text).expect("summary json");
        assert_eq!(value["dry_run"], norito::json!(true));
        assert_eq!(
            value["raw_output"].as_str(),
            Some(display_path(&options.raw_output).as_str())
        );
        assert_eq!(
            value["commands"].as_array().map(|array| array.len()),
            Some(1)
        );
    }
}
