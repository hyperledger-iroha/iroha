//! Data-availability archive construction and proof workflows.

use base64::{Engine, engine::general_purpose::STANDARD as Base64Standard};
use eyre::{Result, WrapErr, eyre};
use iroha_data_model::da::manifest::DaManifestV1;
use norito::{
    decode_from_bytes,
    derive::JsonSerialize,
    json::{self, Map, Value},
};
use sorafs_car::fetch_plan::chunk_fetch_plan_from_json;
#[cfg(test)]
use sorafs_car::sorafs_chunker::ChunkProfile;
use sorafs_orchestrator::prelude::{CarBuildPlan, ChunkStore, InMemoryPayload, PorProof};
use std::{
    collections::HashSet,
    convert::TryFrom,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Canonical manifest and chunk-plan artefacts returned by Torii.
#[derive(Debug, Clone)]
pub struct DaManifestBundle {
    /// Hex-encoded storage ticket returned by Torii.
    pub storage_ticket_hex: String,
    /// Hex-encoded client blob id bound to the manifest.
    pub client_blob_id_hex: String,
    /// Hex-encoded BLAKE3 digest of the payload.
    pub blob_hash_hex: String,
    /// Hex-encoded chunk root recorded in the manifest.
    pub chunk_root_hex: String,
    /// Hex-encoded manifest hash recorded in the receipt/manifest fetch.
    pub manifest_hash_hex: String,
    /// Lane identifier associated with the blob.
    pub lane_id: u64,
    /// Epoch recorded by the manifest.
    pub epoch: u64,
    /// Length of the Norito manifest payload in bytes.
    pub manifest_len: u64,
    /// Raw Norito manifest bytes.
    pub manifest_bytes: Vec<u8>,
    /// Rendered manifest JSON, when provided by Torii.
    pub manifest_json: Value,
    /// Chunk plan JSON emitted by Torii.
    pub chunk_plan: Value,
}

/// Paths produced when a manifest bundle is written to disk.
#[derive(Debug, Clone)]
pub struct DaManifestPersistedPaths {
    /// Path to the raw Norito manifest bytes.
    pub manifest_raw: PathBuf,
    /// Path to the pretty-rendered JSON copy of the manifest.
    pub manifest_json: PathBuf,
    /// Path to the pretty-rendered chunk plan JSON payload.
    pub chunk_plan: PathBuf,
}

impl DaManifestBundle {
    /// Parse a Torii `/v1/da/manifests/{ticket}` JSON payload into a bundle.
    ///
    /// # Errors
    ///
    /// Returns an error when required fields are missing, malformed, or fail to decode.
    pub fn from_json(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| eyre!("DA manifest response must be a JSON object"))?;
        let storage_ticket_hex = require_hex_field(object, &["storage_ticket", "storageTicket"])?;
        let client_blob_id_hex = require_hex_field(object, &["client_blob_id", "clientBlobId"])?;
        let blob_hash_hex = require_hex_field(object, &["blob_hash", "blobHash"])?;
        let chunk_root_hex = require_hex_field(object, &["chunk_root", "chunkRoot"])?;
        let manifest_hash_hex = require_hex_field(object, &["manifest_hash", "manifestHash"])?;
        let lane_id = require_u64_field(object, &["lane_id", "laneId"])?;
        let epoch = require_u64_field(object, &["epoch"])?;
        let manifest_len =
            optional_u64_field(object, &["manifest_len", "manifestLen"])?.unwrap_or(0);
        let manifest_b64 = object
            .get("manifest_norito")
            .or_else(|| object.get("manifestNorito"))
            .or_else(|| object.get("manifest_b64"))
            .or_else(|| object.get("manifestB64"))
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("DA manifest response missing `manifest_norito` field"))?;
        let manifest_bytes = Base64Standard
            .decode(manifest_b64.as_bytes())
            .map_err(|err| eyre!("failed to decode manifest_norito: {err}"))?;
        let manifest_json = object
            .get("manifest")
            .or_else(|| object.get("manifest_json"))
            .or_else(|| object.get("manifestJson"))
            .cloned()
            .unwrap_or(Value::Null);
        let chunk_plan = object
            .get("chunk_plan")
            .or_else(|| object.get("chunkPlan"))
            .cloned()
            .ok_or_else(|| eyre!("DA manifest response missing `chunk_plan` field"))?;
        let parsed_chunk_plan = chunk_fetch_plan_from_json(&chunk_plan)
            .map_err(|err| eyre!("DA manifest response contained invalid chunk_plan: {err}"))?;
        if hex::encode(parsed_chunk_plan.payload_digest) != blob_hash_hex {
            return Err(eyre!(
                "DA manifest response contained invalid chunk_plan: payload digest does not match blob_hash"
            ));
        }
        Ok(Self {
            storage_ticket_hex,
            client_blob_id_hex,
            blob_hash_hex,
            chunk_root_hex,
            manifest_hash_hex,
            lane_id,
            epoch,
            manifest_len,
            manifest_bytes,
            manifest_json,
            chunk_plan,
        })
    }

    /// Decode the embedded Norito manifest payload.
    ///
    /// # Errors
    ///
    /// Returns an error if manifest deserialization fails.
    pub fn decode_manifest(&self) -> Result<DaManifestV1> {
        decode_from_bytes(&self.manifest_bytes)
            .map_err(|err| eyre!("failed to decode DaManifestV1: {err}"))
    }

    /// Persist the manifest artefacts to the provided directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the output directory cannot be created, the ticket label is invalid,
    /// or an artefact fails to write.
    pub fn persist_to_dir(
        &self,
        output_dir: impl AsRef<Path>,
        ticket_label: impl AsRef<str>,
    ) -> Result<DaManifestPersistedPaths> {
        let parsed_chunk_plan = chunk_fetch_plan_from_json(&self.chunk_plan)
            .map_err(|err| eyre!("refusing to persist invalid DA chunk plan: {err}"))?;
        if hex::encode(parsed_chunk_plan.payload_digest) != self.blob_hash_hex {
            return Err(eyre!(
                "refusing to persist DA chunk plan whose payload digest does not match blob_hash"
            ));
        }
        let root = output_dir.as_ref();
        if root.as_os_str().is_empty() {
            return Err(eyre!("manifest output directory must not be empty"));
        }
        fs::create_dir_all(root).wrap_err_with(|| {
            format!(
                "failed to create manifest output directory `{}`",
                root.display()
            )
        })?;
        let label = sanitize_manifest_label(ticket_label.as_ref())?;
        let manifest_path = root.join(format!("manifest_{label}.norito"));
        let manifest_json_path = root.join(format!("manifest_{label}.json"));
        let chunk_plan_path = root.join(format!("chunk_plan_{label}.json"));
        fs::write(&manifest_path, &self.manifest_bytes)
            .wrap_err_with(|| format!("failed to write `{}`", manifest_path.display()))?;
        let manifest_json = json::to_json_pretty(&self.manifest_json)
            .map_err(|err| eyre!("failed to render manifest JSON: {err}"))?;
        fs::write(&manifest_json_path, manifest_json)
            .wrap_err_with(|| format!("failed to write `{}`", manifest_json_path.display()))?;
        let chunk_plan_json = json::to_json_pretty(&self.chunk_plan)
            .map_err(|err| eyre!("failed to render chunk plan JSON: {err}"))?;
        fs::write(&chunk_plan_path, chunk_plan_json)
            .wrap_err_with(|| format!("failed to write `{}`", chunk_plan_path.display()))?;
        Ok(DaManifestPersistedPaths {
            manifest_raw: manifest_path,
            manifest_json: manifest_json_path,
            chunk_plan: chunk_plan_path,
        })
    }
}

fn sanitize_manifest_label(label: &str) -> Result<String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err(eyre!("ticket label must not be empty"));
    }
    let mut sanitized = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_') {
            sanitized.push(ch);
        } else {
            return Err(eyre!(
                "ticket label `{trimmed}` contains unsupported character `{ch}`"
            ));
        }
    }
    Ok(sanitized)
}

fn require_hex_field(object: &Map, keys: &[&str]) -> Result<String> {
    for key in keys {
        if let Some(Value::String(value)) = object.get(*key) {
            let trimmed = value.trim();
            if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(trimmed.to_ascii_lowercase());
            }
            return Err(eyre!("field `{key}` must be a 32-byte hex string"));
        }
    }
    Err(eyre!("response missing `{}` field", keys[0]))
}

fn require_u64_field(object: &Map, keys: &[&str]) -> Result<u64> {
    optional_u64_field(object, keys)?
        .map_or_else(|| Err(eyre!("response missing `{}` field", keys[0])), Ok)
}

fn optional_u64_field(object: &Map, keys: &[&str]) -> Result<Option<u64>> {
    for key in keys {
        if let Some(value) = object.get(*key) {
            return parse_u64_value(value, key).map(Some);
        }
    }
    Ok(None)
}

fn parse_u64_value(value: &Value, label: &str) -> Result<u64> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| eyre!("field `{label}` must be a positive integer")),
        Value::String(raw) => raw
            .trim()
            .parse::<u64>()
            .map_err(|err| eyre!("invalid integer value for `{label}`: {err}")),
        _ => Err(eyre!("field `{label}` must be an integer")),
    }
}

/// Sampling and verification controls for `PoR` proof generation.
#[derive(Debug, Clone)]
pub struct DaProofConfig {
    /// Number of `PoR` leaves to sample randomly.
    pub sample_count: usize,
    /// Deterministic seed used for `PoR` sampling.
    pub sample_seed: u64,
    /// Explicit `PoR` leaf indexes to verify in addition to sampled entries.
    pub leaf_indexes: Vec<usize>,
}
impl Default for DaProofConfig {
    fn default() -> Self {
        Self {
            sample_count: 8,
            sample_seed: 0,
            leaf_indexes: Vec::new(),
        }
    }
}
/// CLI-compatible metadata used when emitting `PoR` artefacts.
#[derive(Debug, Clone)]
pub struct DaProofArtifactMetadata {
    /// Path (or label) pointing to the manifest used for proof generation.
    pub manifest_path: String,
    /// Path (or label) pointing to the payload used for proof generation.
    pub payload_path: String,
}
impl DaProofArtifactMetadata {
    /// Construct metadata from displayable manifest/payload paths.
    #[must_use]
    pub fn new(manifest_path: impl Into<String>, payload_path: impl Into<String>) -> Self {
        Self {
            manifest_path: manifest_path.into(),
            payload_path: payload_path.into(),
        }
    }
}
/// Benchmark summary for data-availability proof verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize)]
pub struct DaProofBenchmark {
    /// Number of proofs requested via sampling.
    pub requested_sample_count: usize,
    /// Number of explicit proof indexes requested.
    pub explicit_index_count: usize,
    /// Total number of proofs generated and verified.
    pub proof_count: usize,
    /// Total leaves in the `PoR` tree.
    pub leaf_count: usize,
    /// Payload size in bytes.
    pub payload_bytes: u64,
    /// Chunk size derived from the manifest.
    pub chunk_size: u32,
    /// Sampling seed used for `PoR` leaf selection.
    pub sample_seed: u64,
    /// Aggregate verification time in milliseconds.
    pub total_duration_ms: u64,
    /// Mean verification time per proof in milliseconds.
    pub average_duration_ms: u64,
    /// Slowest verification time in milliseconds.
    pub max_duration_ms: u64,
    /// Soft budget used for evaluation.
    pub budget_ms: u64,
    /// Whether the aggregate verification fits within the budget.
    pub within_budget: bool,
}
#[cfg(test)]
fn chunk_profile_from_chunk_size(chunk_size: u32) -> Result<ChunkProfile> {
    if chunk_size == 0 {
        return Err(eyre!("manifest chunk_size must be non-zero"));
    }
    let size = usize::try_from(chunk_size).map_err(|_| eyre!("chunk_size exceeds host limits"))?;
    Ok(ChunkProfile {
        min_size: size,
        target_size: size,
        max_size: size,
        break_mask: 1,
    })
}
fn validate_manifest_consistency(manifest: &DaManifestV1, store: &ChunkStore) -> Result<()> {
    let blob_hash_bytes = manifest.blob_hash.as_ref();
    if store.payload_digest().as_bytes() != blob_hash_bytes {
        return Err(eyre!(
            "payload hash mismatch: manifest={} computed={}",
            hex::encode(blob_hash_bytes),
            hex::encode(store.payload_digest().as_bytes())
        ));
    }
    let chunk_root_bytes = manifest.chunk_root.as_ref();
    if store.por_tree().root() != chunk_root_bytes {
        return Err(eyre!(
            "chunk root mismatch: manifest={} computed={}",
            hex::encode(chunk_root_bytes),
            hex::encode(store.por_tree().root())
        ));
    }
    Ok(())
}
fn collect_proofs(
    chunk_store: &ChunkStore,
    proof_source: &mut InMemoryPayload<'_>,
    por_root: &[u8; 32],
    config: &DaProofConfig,
) -> Result<Vec<ProofReport>> {
    let mut proofs = Vec::new();
    let mut seen = HashSet::new();
    proofs.extend(sampled_proofs(
        chunk_store,
        proof_source,
        por_root,
        config,
        &mut seen,
    )?);
    proofs.extend(explicit_proofs(
        chunk_store,
        proof_source,
        por_root,
        config,
        &mut seen,
    )?);
    if proofs.is_empty() {
        return Err(eyre!(
            "no proofs were generated; provide a sample count or explicit leaf indexes"
        ));
    }
    Ok(proofs)
}
fn sampled_proofs(
    chunk_store: &ChunkStore,
    proof_source: &mut InMemoryPayload<'_>,
    por_root: &[u8; 32],
    config: &DaProofConfig,
    seen: &mut HashSet<usize>,
) -> Result<Vec<ProofReport>> {
    if config.sample_count == 0 {
        return Ok(Vec::new());
    }
    let sampled = chunk_store
        .sample_leaves_with(config.sample_count, config.sample_seed, proof_source)
        .wrap_err("failed to sample PoR leaves")?;
    let mut proofs = Vec::with_capacity(sampled.len());
    for (leaf_index, proof) in sampled {
        if !seen.insert(leaf_index) {
            continue;
        }
        let verified = proof.verify(por_root);
        proofs.push(ProofReport {
            origin: ProofOrigin::Sampled,
            leaf_index,
            proof,
            verified,
        });
    }
    Ok(proofs)
}
fn explicit_proofs(
    chunk_store: &ChunkStore,
    proof_source: &mut InMemoryPayload<'_>,
    por_root: &[u8; 32],
    config: &DaProofConfig,
    seen: &mut HashSet<usize>,
) -> Result<Vec<ProofReport>> {
    if config.leaf_indexes.is_empty() {
        return Ok(Vec::new());
    }
    let tree = chunk_store.por_tree();
    let leaf_count = tree.leaf_count();
    let mut proofs = Vec::new();
    for &leaf_index in &config.leaf_indexes {
        if leaf_index >= leaf_count {
            return Err(eyre!(
                "leaf-index {} out of range (tree tracks {leaf_count} leaves)",
                leaf_index
            ));
        }
        if !seen.insert(leaf_index) {
            continue;
        }
        let (chunk_idx, segment_idx, inner_idx) = tree
            .leaf_path(leaf_index)
            .ok_or_else(|| eyre!("missing leaf-path for {leaf_index}"))?;
        let proof = tree
            .prove_leaf_with(chunk_idx, segment_idx, inner_idx, proof_source)
            .wrap_err_with(|| format!("failed to build PoR proof for leaf-index {leaf_index}"))?
            .ok_or_else(|| eyre!("missing PoR proof for leaf-index {leaf_index}"))?;
        let verified = proof.verify(por_root);
        proofs.push(ProofReport {
            origin: ProofOrigin::Explicit,
            leaf_index,
            proof,
            verified,
        });
    }
    Ok(proofs)
}
#[derive(Debug)]
enum ProofOrigin {
    Sampled,
    Explicit,
}
impl ProofOrigin {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Sampled => "sampled",
            Self::Explicit => "explicit",
        }
    }
}
struct ProofReport {
    origin: ProofOrigin,
    leaf_index: usize,
    proof: PorProof,
    verified: bool,
}
struct ProofSummaryInputs<'a> {
    manifest: &'a DaManifestV1,
    por_root_hex: String,
    leaf_total: usize,
    segment_total: usize,
    chunk_total: usize,
    sample_count: usize,
    sample_seed: u64,
    artifact_metadata: Option<&'a DaProofArtifactMetadata>,
}
fn build_proof_summary(inputs: ProofSummaryInputs<'_>, proofs: &[ProofReport]) -> Value {
    let mut map = Map::new();
    if let Some(metadata) = inputs.artifact_metadata {
        map.insert(
            "manifest_path".into(),
            Value::from(metadata.manifest_path.clone()),
        );
        map.insert(
            "payload_path".into(),
            Value::from(metadata.payload_path.clone()),
        );
    }
    map.insert(
        "blob_hash".into(),
        Value::from(hex::encode(inputs.manifest.blob_hash.as_ref())),
    );
    map.insert(
        "chunk_root".into(),
        Value::from(hex::encode(inputs.manifest.chunk_root.as_ref())),
    );
    map.insert("por_root".into(), Value::from(inputs.por_root_hex));
    map.insert("leaf_count".into(), value_from_usize(inputs.leaf_total));
    map.insert(
        "segment_count".into(),
        value_from_usize(inputs.segment_total),
    );
    map.insert("chunk_count".into(), value_from_usize(inputs.chunk_total));
    map.insert("sample_count".into(), value_from_usize(inputs.sample_count));
    map.insert("sample_seed".into(), Value::from(inputs.sample_seed));
    map.insert("proof_count".into(), value_from_usize(proofs.len()));
    let proof_values = proofs.iter().map(proof_to_json).collect::<Vec<_>>();
    map.insert("proofs".into(), Value::Array(proof_values));
    Value::Object(map)
}
fn proof_to_json(report: &ProofReport) -> Value {
    let mut map = Map::new();
    map.insert("origin".into(), Value::from(report.origin.as_str()));
    map.insert("leaf_index".into(), value_from_usize(report.leaf_index));
    map.insert(
        "chunk_index".into(),
        value_from_usize(report.proof.chunk_index),
    );
    map.insert(
        "segment_index".into(),
        value_from_usize(report.proof.segment_index),
    );
    map.insert("leaf_offset".into(), Value::from(report.proof.leaf_offset));
    map.insert(
        "leaf_length".into(),
        value_from_u32(report.proof.leaf_length),
    );
    map.insert(
        "segment_offset".into(),
        Value::from(report.proof.segment_offset),
    );
    map.insert(
        "segment_length".into(),
        value_from_u32(report.proof.segment_length),
    );
    map.insert(
        "chunk_offset".into(),
        Value::from(report.proof.chunk_offset),
    );
    map.insert(
        "chunk_length".into(),
        value_from_u32(report.proof.chunk_length),
    );
    map.insert("payload_len".into(), Value::from(report.proof.payload_len));
    map.insert(
        "chunk_digest".into(),
        Value::from(hex::encode(report.proof.chunk_digest)),
    );
    map.insert(
        "chunk_root".into(),
        Value::from(hex::encode(report.proof.chunk_root)),
    );
    map.insert(
        "segment_digest".into(),
        Value::from(hex::encode(report.proof.segment_digest)),
    );
    map.insert(
        "leaf_digest".into(),
        Value::from(hex::encode(report.proof.leaf_digest)),
    );
    map.insert(
        "leaf_bytes_b64".into(),
        Value::from(Base64Standard.encode(&report.proof.leaf_bytes)),
    );
    map.insert(
        "segment_leaves".into(),
        Value::Array(
            report
                .proof
                .segment_leaves
                .iter()
                .map(|digest| Value::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert(
        "chunk_segments".into(),
        Value::Array(
            report
                .proof
                .chunk_segments
                .iter()
                .map(|digest| Value::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert("chunk_count".into(), Value::from(report.proof.chunk_count));
    map.insert(
        "chunk_merkle_path".into(),
        Value::Array(
            report
                .proof
                .chunk_merkle_path
                .iter()
                .map(|digest| Value::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert("verified".into(), Value::from(report.verified));
    Value::Object(map)
}

/// Generate a CAR build plan directly from a DA manifest.
///
/// # Errors
///
/// Returns an error when chunk metadata cannot be converted to a plan.
pub fn build_car_plan_from_manifest(manifest: &DaManifestV1) -> Result<CarBuildPlan> {
    sorafs_car::build_plan_from_da_manifest(manifest).map_err(|err| eyre!(err))
}
/// Compute a `PoR` summary from an in-memory payload and manifest metadata.
///
/// The returned JSON mirrors `iroha da prove --json-out` so downstream tooling can
/// consume identical artefacts across the CLI and SDK surfaces.
///
/// # Errors
///
/// Returns an error when manifest/payload validation fails or `PoR` proofs cannot be sampled.
pub fn generate_da_proof_summary(
    manifest: &DaManifestV1,
    payload: &[u8],
    config: &DaProofConfig,
) -> Result<Value> {
    generate_da_proof_summary_inner(manifest, payload, config, None)
}
/// Generate a CLI-compatible `PoR` artefact that includes manifest/payload paths.
///
/// This helper mirrors `iroha da prove --json-out` so SDKs can emit the same Norito
/// artefact without shelling out to the CLI. Provide canonical paths (or descriptive
/// labels) so downstream governance tooling records the provenance of the proof bundle.
///
/// # Errors
///
/// Returns an error when manifest/payload validation fails or `PoR` proofs cannot be sampled.
pub fn generate_da_proof_artifact(
    manifest: &DaManifestV1,
    payload: &[u8],
    config: &DaProofConfig,
    metadata: &DaProofArtifactMetadata,
) -> Result<Value> {
    generate_da_proof_summary_inner(manifest, payload, config, Some(metadata))
}
fn generate_da_proof_summary_inner(
    manifest: &DaManifestV1,
    payload: &[u8],
    config: &DaProofConfig,
    metadata: Option<&DaProofArtifactMetadata>,
) -> Result<Value> {
    let plan = build_car_plan_from_manifest(manifest)?;
    let mut chunk_store = ChunkStore::with_profile(plan.chunk_profile);
    let mut ingest_source = InMemoryPayload::new(payload);
    chunk_store
        .ingest_plan_source(&plan, &mut ingest_source)
        .wrap_err("failed to ingest payload for PoR generation")?;
    validate_manifest_consistency(manifest, &chunk_store)?;
    let mut proof_source = InMemoryPayload::new(payload);
    let por_root = *chunk_store.por_tree().root();
    let leaf_total = chunk_store.por_tree().leaf_count();
    let segment_total = chunk_store.por_tree().segment_count();
    let chunk_total = chunk_store.por_tree().chunks().len();
    let proofs = collect_proofs(&chunk_store, &mut proof_source, &por_root, config)?;
    let summary_inputs = ProofSummaryInputs {
        manifest,
        por_root_hex: hex::encode(por_root),
        leaf_total,
        segment_total,
        chunk_total,
        sample_count: config.sample_count,
        sample_seed: config.sample_seed,
        artifact_metadata: metadata,
    };
    Ok(build_proof_summary(summary_inputs, &proofs))
}
fn duration_to_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
/// Benchmark `PoR` verification against a soft budget.
///
/// The benchmark rebuilds the chunk store from the manifest/payload pair,
/// generates `PoR` proofs using the provided sampling configuration, re-verifies
/// each proof while timing the verification step, and returns a summary that
/// can be compared against the configured `zk_halo2_verifier_budget_ms`.
///
/// # Errors
///
/// Returns an error when manifest or payload ingestion fails or if any proof fails verification.
pub fn benchmark_da_proof_verification(
    manifest: &DaManifestV1,
    payload: &[u8],
    config: &DaProofConfig,
    budget_ms: u64,
) -> Result<DaProofBenchmark> {
    benchmark_da_proof_verification_with_measure(
        manifest,
        payload,
        config,
        budget_ms,
        |proof, por_root| {
            let started = Instant::now();
            let verified = proof.verify(por_root);
            (started.elapsed(), verified)
        },
    )
}
fn benchmark_da_proof_verification_with_measure<M>(
    manifest: &DaManifestV1,
    payload: &[u8],
    config: &DaProofConfig,
    budget_ms: u64,
    mut measure: M,
) -> Result<DaProofBenchmark>
where
    M: FnMut(&PorProof, &[u8; 32]) -> (Duration, bool),
{
    let plan = build_car_plan_from_manifest(manifest)?;
    let mut chunk_store = ChunkStore::with_profile(plan.chunk_profile);
    let mut ingest_source = InMemoryPayload::new(payload);
    chunk_store
        .ingest_plan_source(&plan, &mut ingest_source)
        .wrap_err("failed to ingest payload for verification benchmark")?;
    validate_manifest_consistency(manifest, &chunk_store)?;
    let por_root = *chunk_store.por_tree().root();
    let leaf_count = chunk_store.por_tree().leaf_count();
    let mut proof_source = InMemoryPayload::new(payload);
    let proofs = collect_proofs(&chunk_store, &mut proof_source, &por_root, config)?;
    let mut total_duration = Duration::ZERO;
    let mut max_duration = Duration::ZERO;
    for proof in &proofs {
        if !proof.verified {
            return Err(eyre!(
                "proof for leaf {} failed verification before benchmarking",
                proof.leaf_index
            ));
        }
        let (elapsed, verified) = measure(&proof.proof, &por_root);
        if !verified {
            return Err(eyre!(
                "proof for leaf {} failed verification during benchmarking",
                proof.leaf_index
            ));
        }
        total_duration = total_duration.saturating_add(elapsed);
        if elapsed > max_duration {
            max_duration = elapsed;
        }
    }
    let proof_count = proofs.len();
    let average_duration = if proof_count == 0 {
        Duration::ZERO
    } else {
        total_duration / u32::try_from(proof_count).expect("proof count fits in u32 for averaging")
    };
    let budget_duration = Duration::from_millis(budget_ms);
    Ok(DaProofBenchmark {
        requested_sample_count: config.sample_count,
        explicit_index_count: config.leaf_indexes.len(),
        proof_count,
        leaf_count,
        payload_bytes: manifest.total_size,
        chunk_size: manifest.chunk_size,
        sample_seed: config.sample_seed,
        total_duration_ms: duration_to_millis(total_duration),
        average_duration_ms: duration_to_millis(average_duration),
        max_duration_ms: duration_to_millis(max_duration),
        budget_ms,
        within_budget: total_duration <= budget_duration,
    })
}

fn value_from_usize(value: usize) -> Value {
    Value::from(u64::try_from(value).unwrap_or(u64::MAX))
}

fn value_from_u32(value: u32) -> Value {
    Value::from(u64::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        da::{
            manifest::{ChunkCommitment, ChunkRole},
            types::{
                BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile,
                ExtraMetadata, FecScheme, RetentionPolicy, StorageTicketId,
            },
        },
        nexus::LaneId,
    };
    use std::fs;
    use tempfile::tempdir;

    fn empty_chunk_fetch_plan(payload_digest_byte: u8) -> Value {
        Value::Object(Map::from_iter([
            (
                "schema".into(),
                Value::from(sorafs_car::fetch_plan::CHUNK_FETCH_PLAN_SCHEMA_V1),
            ),
            (
                "payload_digest_blake3_hex".into(),
                Value::from(hex::encode([payload_digest_byte; 32])),
            ),
            ("chunk_fetch_specs".into(), Value::Array(Vec::new())),
        ]))
    }

    #[test]
    fn manifest_bundle_parses_required_fields() {
        let mut object = Map::new();
        object.insert("storage_ticket".into(), Value::from("11".repeat(32)));
        object.insert("client_blob_id".into(), Value::from("22".repeat(32)));
        object.insert("blob_hash".into(), Value::from("33".repeat(32)));
        object.insert("chunk_root".into(), Value::from("44".repeat(32)));
        object.insert("lane_id".into(), Value::from(0));
        object.insert("epoch".into(), Value::from(1));
        object.insert("manifest_len".into(), Value::from(16));
        object.insert(
            "manifest_norito".into(),
            Value::from(Base64Standard.encode([0_u8; 4])),
        );
        object.insert(
            "manifest".into(),
            Value::Object(Map::from_iter([("dummy".into(), Value::from(1))])),
        );
        object.insert("chunk_plan".into(), empty_chunk_fetch_plan(0x33));
        object.insert("manifest_hash".into(), Value::from("55".repeat(32)));
        let bundle = DaManifestBundle::from_json(&Value::Object(object)).expect("bundle");
        assert_eq!(bundle.storage_ticket_hex, "11".repeat(32));
        assert_eq!(bundle.client_blob_id_hex, "22".repeat(32));
        assert_eq!(bundle.blob_hash_hex, "33".repeat(32));
        assert_eq!(bundle.chunk_root_hex, "44".repeat(32));
        assert_eq!(bundle.manifest_hash_hex, "55".repeat(32));
        assert_eq!(bundle.lane_id, 0);
        assert_eq!(bundle.epoch, 1);
    }

    #[test]
    fn manifest_bundle_rejects_retired_or_unbound_chunk_plans() {
        let base = Map::from_iter([
            ("storage_ticket".into(), Value::from("11".repeat(32))),
            ("client_blob_id".into(), Value::from("22".repeat(32))),
            ("blob_hash".into(), Value::from("33".repeat(32))),
            ("chunk_root".into(), Value::from("44".repeat(32))),
            ("manifest_hash".into(), Value::from("55".repeat(32))),
            ("lane_id".into(), Value::from(0)),
            ("epoch".into(), Value::from(1)),
            ("manifest_len".into(), Value::from(4)),
            (
                "manifest_norito".into(),
                Value::from(Base64Standard.encode([0_u8; 4])),
            ),
        ]);
        let invalid_plans = [
            Value::Array(Vec::new()),
            Value::Object(Map::from_iter([
                (
                    "schema".into(),
                    Value::from(sorafs_car::fetch_plan::CHUNK_FETCH_PLAN_SCHEMA_V1),
                ),
                ("chunk_fetch_specs".into(), Value::Array(Vec::new())),
            ])),
            empty_chunk_fetch_plan(0),
            empty_chunk_fetch_plan(0x77),
        ];
        for plan in invalid_plans {
            let mut object = base.clone();
            object.insert("chunk_plan".into(), plan);
            let error = DaManifestBundle::from_json(&Value::Object(object))
                .expect_err("retired or unbound plan must be rejected");
            assert!(
                error.to_string().contains("invalid chunk_plan"),
                "unexpected error: {error:?}"
            );
        }
    }

    #[test]
    fn manifest_bundle_persist_to_dir_writes_outputs() {
        let (manifest, payload) = sample_manifest_and_payload();
        let manifest_bytes = norito::to_bytes(&manifest).expect("encode manifest");
        let bundle = DaManifestBundle {
            storage_ticket_hex: "11".repeat(32),
            client_blob_id_hex: "22".repeat(32),
            blob_hash_hex: hex::encode(manifest.blob_hash.as_ref()),
            chunk_root_hex: hex::encode(manifest.chunk_root.as_ref()),
            manifest_hash_hex: hex::encode(blake3::hash(&manifest_bytes).as_bytes()),
            lane_id: u64::from(manifest.lane_id.as_u32()),
            epoch: manifest.epoch,
            manifest_len: manifest_bytes.len() as u64,
            manifest_bytes: manifest_bytes.clone(),
            manifest_json: norito::json::value::to_value(&manifest).expect("manifest json"),
            chunk_plan: sorafs_car::fetch_plan::try_chunk_fetch_plan_to_json(
                &CarBuildPlan::single_file(&payload).expect("build fixture CAR plan"),
            )
            .expect("render canonical chunk fetch plan"),
        };
        let dir = tempdir().expect("tempdir");
        let paths = bundle
            .persist_to_dir(dir.path(), "AA11")
            .expect("persist bundle");
        assert!(paths.manifest_raw.exists());
        assert!(paths.manifest_json.exists());
        assert!(paths.chunk_plan.exists());
        assert_eq!(
            fs::read(&paths.manifest_raw).expect("read manifest"),
            manifest_bytes
        );
        let manifest_json = fs::read_to_string(&paths.manifest_json).expect("read manifest json");
        let manifest_value: Value =
            norito::json::from_slice(manifest_json.as_bytes()).expect("manifest json parses");
        assert!(manifest_value.is_object());
        let chunk_plan_json = fs::read_to_string(&paths.chunk_plan).expect("read chunk plan");
        let chunk_plan_value: Value =
            norito::json::from_slice(chunk_plan_json.as_bytes()).expect("chunk plan JSON parses");
        assert!(chunk_plan_value.is_object());
    }

    #[test]
    fn chunk_profile_from_chunk_size_rejects_zero() {
        let err = chunk_profile_from_chunk_size(0).expect_err("zero chunk size must fail");
        assert!(err.to_string().contains("non-zero"));
    }

    #[test]
    fn chunk_profile_from_chunk_size_sets_equal_bounds() {
        let profile = chunk_profile_from_chunk_size(512).expect("fixed chunk profile");
        assert_eq!(profile.min_size, 512);
        assert_eq!(profile.target_size, 512);
        assert_eq!(profile.max_size, 512);
        assert_eq!(profile.break_mask, 1);
    }

    #[test]
    fn car_plan_matches_manifest_metadata() {
        let (manifest, _) = sample_manifest_and_payload();
        let plan = build_car_plan_from_manifest(&manifest).expect("CAR plan");
        assert_eq!(plan.chunks.len(), manifest.chunks.len());
        assert_eq!(plan.content_length, manifest.total_size);
        assert_eq!(plan.payload_digest.as_bytes(), manifest.blob_hash.as_ref());
    }

    #[test]
    fn car_plan_rejects_foreign_stripe_geometry() {
        let (mut manifest, _) = sample_manifest_and_payload();
        manifest.shards_per_stripe = 14;
        let err =
            build_car_plan_from_manifest(&manifest).expect_err("foreign stripe geometry must fail");
        assert!(
            err.to_string()
                .contains("manifest shards_per_stripe is 14; canonical value is 1"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn proof_summary_contains_expected_fields() {
        let (manifest, payload) = sample_manifest_and_payload();
        let summary = generate_da_proof_summary(&manifest, &payload, &DaProofConfig::default())
            .expect("proof summary");
        let map = summary.as_object().expect("summary object");
        let expected_blob_hash = hex::encode(manifest.blob_hash.as_ref());
        assert_eq!(
            map.get("blob_hash").and_then(Value::as_str),
            Some(expected_blob_hash.as_str())
        );
        assert_eq!(
            map.get("chunk_count").and_then(Value::as_u64),
            Some(manifest.chunks.len() as u64)
        );
        assert!(
            map.get("proofs")
                .and_then(Value::as_array)
                .is_some_and(|proofs| !proofs.is_empty())
        );
    }

    #[test]
    fn proof_summary_rejects_mismatched_payload() {
        let (manifest, mut payload) = sample_manifest_and_payload();
        payload[0] ^= 0xff;
        let err = generate_da_proof_summary(&manifest, &payload, &DaProofConfig::default())
            .expect_err("mismatched payload must fail");
        assert!(err.to_string().contains("failed to ingest payload"));
    }

    #[test]
    fn proof_artifact_includes_paths() {
        let (manifest, payload) = sample_manifest_and_payload();
        let metadata = DaProofArtifactMetadata::new(
            "/tmp/manifests/sample_manifest.norito",
            "/tmp/payloads/sample_payload.car",
        );
        let artifact =
            generate_da_proof_artifact(&manifest, &payload, &DaProofConfig::default(), &metadata)
                .expect("proof artifact");
        let map = artifact.as_object().expect("artifact object");
        assert_eq!(
            map.get("manifest_path").and_then(Value::as_str),
            Some("/tmp/manifests/sample_manifest.norito")
        );
        assert_eq!(
            map.get("payload_path").and_then(Value::as_str),
            Some("/tmp/payloads/sample_payload.car")
        );
        assert!(map.contains_key("por_root"));
    }

    #[test]
    fn benchmark_reports_under_budget() {
        let (manifest, payload) = sample_manifest_and_payload();
        let config = DaProofConfig {
            sample_count: 1,
            sample_seed: 42,
            leaf_indexes: Vec::new(),
        };
        let bench = benchmark_da_proof_verification_with_measure(
            &manifest,
            &payload,
            &config,
            50,
            |proof, root| {
                assert!(proof.verify(root));
                (Duration::from_millis(5), true)
            },
        )
        .expect("benchmark");
        assert_eq!(bench.proof_count, 1);
        assert_eq!(bench.total_duration_ms, 5);
        assert_eq!(bench.average_duration_ms, 5);
        assert!(bench.within_budget);
    }

    #[test]
    fn benchmark_flags_over_budget_runs() {
        let (manifest, payload) = sample_manifest_and_payload();
        let config = DaProofConfig {
            sample_count: 1,
            sample_seed: 7,
            leaf_indexes: Vec::new(),
        };
        let bench = benchmark_da_proof_verification_with_measure(
            &manifest,
            &payload,
            &config,
            1,
            |proof, root| {
                assert!(proof.verify(root));
                (Duration::from_millis(10), true)
            },
        )
        .expect("benchmark");
        assert_eq!(bench.total_duration_ms, 10);
        assert!(!bench.within_budget);
    }

    #[test]
    fn benchmark_rejects_failed_verification() {
        let (manifest, payload) = sample_manifest_and_payload();
        let config = DaProofConfig {
            sample_count: 1,
            sample_seed: 0,
            leaf_indexes: Vec::new(),
        };
        let err = benchmark_da_proof_verification_with_measure(
            &manifest,
            &payload,
            &config,
            10,
            |_proof, _root| (Duration::ZERO, false),
        )
        .expect_err("failed verification must fail");
        assert!(err.to_string().contains("failed verification"));
    }

    fn sample_manifest_and_payload() -> (DaManifestV1, Vec<u8>) {
        let payload = vec![0xab; 8];
        let mut store = ChunkStore::new();
        store.ingest_bytes(&payload).expect("ingest payload");
        let chunks = store
            .chunks()
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                ChunkCommitment::new_with_role(
                    u32::try_from(index).expect("chunk index fits in u32"),
                    chunk.offset,
                    chunk.length,
                    ChunkDigest::new(chunk.blake3),
                    ChunkRole::Data,
                    0,
                )
            })
            .collect::<Vec<_>>();
        let blob_hash = BlobDigest::new(*store.payload_digest().as_bytes());
        let chunk_root = BlobDigest::new(*store.por_tree().root());
        let erasure_profile = ErasureProfile {
            data_shards: 1,
            parity_shards: 0,
            row_parity_stripes: 0,
            chunk_alignment: 1,
            fec_scheme: FecScheme::Rs12_10,
        };
        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(0),
            epoch: 1,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new("custom.binary"),
            blob_hash,
            chunk_root,
            storage_ticket: StorageTicketId::new([0; 32]),
            total_size: payload.len() as u64,
            chunk_size: chunks
                .first()
                .map_or(payload.len() as u32, |chunk| chunk.length),
            total_stripes: u32::try_from(chunks.len()).expect("stripe count fits in u32"),
            shards_per_stripe: 1,
            erasure_profile,
            retention_policy: RetentionPolicy::default(),
            rent_quote: DaRentQuote::default(),
            chunks,
            ipa_commitment: chunk_root,
            metadata: ExtraMetadata::default(),
            issued_at_unix: 0,
        };
        (manifest, payload)
    }
}
