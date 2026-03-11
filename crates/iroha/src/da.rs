//! Data-availability helpers shared across SDKs.

use std::{
    collections::HashSet,
    convert::TryFrom,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD as Base64Standard};
use blake3::Hasher;
use eyre::{Result, WrapErr, eyre};
use iroha_crypto::Signature;
use iroha_data_model::{
    da::{
        commitment::{DaCommitmentProof, DaCommitmentWithLocation, DaProofPolicyBundle},
        ingest::{DaIngestReceipt, DaIngestRequest},
        manifest::{ChunkRole, DaManifestV1},
        types::{
            BlobClass, BlobCodec, BlobDigest, Compression, DaRentLedgerProjection, ErasureProfile,
            ExtraMetadata, GovernanceTag, RetentionPolicy, StorageTicketId,
        },
    },
    nexus::LaneId,
    query::parameters::Pagination,
    sorafs::pin_registry::{ManifestDigest, StorageClass},
};
use iroha_primitives::numeric::Numeric;
use norito::{
    decode_from_bytes,
    derive::{JsonDeserialize, JsonSerialize},
    json::{self, Map, Value},
};
#[cfg(test)]
use sorafs_car::sorafs_chunker::ChunkProfile;
use sorafs_manifest::{deal::XorAmount, pdp::PdpCommitmentV1};
use sorafs_orchestrator::prelude::{CarBuildPlan, ChunkStore, InMemoryPayload, PorProof};

use crate::{
    crypto::KeyPair,
    data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        isi::{InstructionBox, Transfer},
    },
};

/// Canonical HTTP header carrying the base64-encoded PDP commitment bytes.
pub const PDP_COMMITMENT_HEADER: &str = "sora-pdp-commitment";

/// Decode the `sora-pdp-commitment` header into a typed PDP commitment.
///
/// # Errors
///
/// Returns an error if the header is not valid base64 or the decoded bytes fail
/// Norito deserialization.
pub fn decode_pdp_commitment_header(value: &str) -> Result<PdpCommitmentV1> {
    let bytes = Base64Standard
        .decode(value.as_bytes())
        .map_err(|err| eyre!("invalid {PDP_COMMITMENT_HEADER} header: {err}"))?;
    decode_pdp_commitment_bytes(&bytes)
}

/// Decode Norito-encoded PDP commitment bytes into a typed structure.
///
/// # Errors
///
/// Returns an error if the bytes cannot be decoded via Norito.
pub fn decode_pdp_commitment_bytes(bytes: &[u8]) -> Result<PdpCommitmentV1> {
    decode_from_bytes(bytes).map_err(|err| eyre!("failed to decode PDP commitment: {err}"))
}

/// Decode the optional PDP commitment embedded in a DA receipt.
///
/// # Errors
///
/// Returns an error if the commitment bytes contained in the receipt fail Norito decoding.
pub fn receipt_pdp_commitment(receipt: &DaIngestReceipt) -> Result<Option<PdpCommitmentV1>> {
    receipt.pdp_commitment.as_deref().map_or_else(
        || Ok(None),
        |bytes| decode_pdp_commitment_bytes(bytes).map(Some),
    )
}

/// Chunk selected for DA sampling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaSampledChunk {
    /// Manifest chunk index selected for sampling.
    pub index: u32,
    /// Role of the sampled chunk within the erasure layout.
    pub role: ChunkRole,
    /// Group identifier (stripe/column) for the sampled chunk.
    pub group: u32,
}

/// Deterministic sampling plan derived from a manifest and block hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaSamplingPlan {
    /// Assignment hash covering the manifest and block seed.
    pub assignment_hash: BlobDigest,
    /// Requested sampling window used to draw the chunk indices.
    pub sample_window: u16,
    /// Optional seed used to derive the deterministic sample set.
    pub sample_seed: Option<[u8; 32]>,
    /// Sampled chunks (deduplicated, order preserved after sorting).
    pub samples: Vec<DaSampledChunk>,
}

/// Canonical manifest + chunk-plan artefacts returned by Torii.
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
    /// Length of the Norito manifest payload (bytes).
    pub manifest_len: u64,
    /// Raw Norito manifest bytes.
    pub manifest_bytes: Vec<u8>,
    /// Rendered manifest JSON, when provided by Torii.
    pub manifest_json: Value,
    /// Chunk plan JSON emitted by Torii.
    pub chunk_plan: Value,
    /// Optional deterministic sampling plan bound to the manifest.
    pub sampling_plan: Option<DaSamplingPlan>,
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

/// Request payload for `/v1/da/commitments` and `/v1/da/commitments/prove`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentProofRequest {
    /// Optional manifest digest used as the primary lookup key.
    pub manifest_hash: Option<ManifestDigest>,
    /// Optional lane id used with `epoch` and `sequence` fallback lookup.
    pub lane_id: Option<u32>,
    /// Optional epoch used with `lane_id` and `sequence` fallback lookup.
    pub epoch: Option<u64>,
    /// Optional sequence used with `lane_id` and `epoch` fallback lookup.
    pub sequence: Option<u64>,
    /// Optional pagination metadata for list responses.
    pub pagination: Option<Pagination>,
}

/// Response payload for `/v1/da/commitments`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentListResponse {
    /// Active proof-policy bundle for DA commitments.
    pub policies: DaProofPolicyBundle,
    /// Matching commitment records with on-chain location metadata.
    pub commitments: Vec<DaCommitmentWithLocation>,
}

/// Response payload for `/v1/da/commitments/prove`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentProofResponse {
    /// Active proof-policy bundle for DA commitments.
    pub policies: DaProofPolicyBundle,
    /// Commitment proof bound to the requested record.
    pub proof: DaCommitmentProof,
}

/// Response payload for `/v1/da/commitments/verify`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentVerifyResponse {
    /// Indicates whether the supplied proof verified against the current store.
    pub valid: bool,
    /// Optional verification failure detail when `valid` is false.
    pub error: Option<String>,
}

/// Request payload for `/v1/da/pin_intents` and `/v1/da/pin_intents/prove`.
#[derive(Debug, Default, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaPinIntentQueryRequest {
    /// Optional manifest digest used as a lookup key.
    pub manifest_hash: Option<ManifestDigest>,
    /// Optional storage ticket used as a lookup key.
    pub storage_ticket: Option<StorageTicketId>,
    /// Optional human-readable alias used as a lookup key.
    pub alias: Option<String>,
    /// Optional lane id used with `epoch` and `sequence` fallback lookup.
    pub lane_id: Option<u32>,
    /// Optional epoch used with `lane_id` and `sequence` fallback lookup.
    pub epoch: Option<u64>,
    /// Optional sequence used with `lane_id` and `epoch` fallback lookup.
    pub sequence: Option<u64>,
    /// Optional pagination metadata for list responses.
    pub pagination: Option<Pagination>,
}

/// Response payload for `/v1/da/pin_intents/verify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaPinIntentVerifyResponse {
    /// Indicates whether the supplied pin intent verified against the current store.
    pub valid: bool,
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
        let sampling_plan = object
            .get("sampling_plan")
            .map(parse_sampling_plan)
            .transpose()?;
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
            sampling_plan,
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

    /// Persist the manifest artefacts to the provided directory, mirroring
    /// the `iroha da get-blob` layout (`manifest_<ticket>.norito/json`,
    /// `chunk_plan_<ticket>.json`).
    ///
    /// # Errors
    ///
    /// Returns an error when the output directory cannot be created, the ticket label is invalid,
    /// or any artefact fails to write.
    pub fn persist_to_dir(
        &self,
        output_dir: impl AsRef<Path>,
        ticket_label: impl AsRef<str>,
    ) -> Result<DaManifestPersistedPaths> {
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
/// Returns an error when manifest or payload ingestion fails or if any proof
/// fails verification.
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

/// Canonical ingest parameters shared by CLI and SDK clients.
#[derive(Debug, Clone)]
pub struct DaIngestParams {
    /// Lane identifier recorded in the DA request.
    pub lane_id: LaneId,
    /// Epoch identifier recorded in the DA request.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)`.
    pub sequence: u64,
    /// Semantic blob classification (e.g., `nexus_lane_sidecar`).
    pub blob_class: BlobClass,
    /// Codec label associated with the payload (e.g., `custom.binary`).
    pub blob_codec: BlobCodec,
    /// Requested erasure profile for chunking.
    pub erasure_profile: ErasureProfile,
    /// Retention policy to enforce.
    pub retention_policy: RetentionPolicy,
    /// Chunk size in bytes.
    pub chunk_size: u32,
    /// Optional caller-supplied blob digest override. Defaults to BLAKE3(payload).
    pub client_blob_id: Option<BlobDigest>,
}

impl DaIngestParams {
    /// Override the client-provided blob digest.
    #[must_use]
    pub fn with_client_blob_id(mut self, digest: BlobDigest) -> Self {
        self.client_blob_id = Some(digest);
        self
    }
}

impl Default for DaIngestParams {
    fn default() -> Self {
        Self {
            lane_id: LaneId::new(0),
            epoch: 0,
            sequence: 0,
            blob_class: BlobClass::NexusLaneSidecar,
            blob_codec: BlobCodec::new("custom.binary"),
            erasure_profile: ErasureProfile::default(),
            retention_policy: default_retention_policy(),
            chunk_size: 262_144,
            client_blob_id: None,
        }
    }
}

fn default_retention_policy() -> RetentionPolicy {
    RetentionPolicy {
        storage_class: StorageClass::Warm,
        governance_tag: GovernanceTag::new("da.generic"),
        ..RetentionPolicy::default()
    }
}

/// Build and sign a canonical `DaIngestRequest` using the supplied key pair.
///
/// # Errors
///
/// Returns an error if signature generation fails (should be infallible under
/// normal conditions).
pub fn build_da_request(
    payload_bytes: Vec<u8>,
    params: &DaIngestParams,
    metadata: ExtraMetadata,
    key_pair: &KeyPair,
    manifest_bytes: Option<Vec<u8>>,
) -> DaIngestRequest {
    let client_blob_id = params.client_blob_id.unwrap_or_else(|| {
        let mut hasher = Hasher::new();
        hasher.update(&payload_bytes);
        BlobDigest::from_hash(hasher.finalize())
    });
    let submitter = key_pair.public_key().clone();
    let signature = Signature::new(key_pair.private_key(), &payload_bytes);
    DaIngestRequest {
        client_blob_id,
        lane_id: params.lane_id,
        epoch: params.epoch,
        sequence: params.sequence,
        blob_class: params.blob_class,
        codec: params.blob_codec.clone(),
        erasure_profile: params.erasure_profile,
        retention_policy: params.retention_policy.clone(),
        chunk_size: params.chunk_size,
        total_size: payload_bytes.len() as u64,
        compression: Compression::Identity,
        norito_manifest: manifest_bytes,
        payload: payload_bytes,
        metadata,
        submitter,
        signature,
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
    map.insert(
        "chunk_roots".into(),
        Value::Array(
            report
                .proof
                .chunk_roots
                .iter()
                .map(|digest| Value::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert("verified".into(), Value::from(report.verified));
    Value::Object(map)
}

/// Planned rent ledger movements derived from a [`DaRentLedgerProjection`].
#[derive(Debug, Clone)]
pub struct DaRentLedgerPlan {
    /// Total rent owed for the retention period.
    pub rent_due: XorAmount,
    /// Protocol reserve allocation sourced from the rent.
    pub protocol_reserve_due: XorAmount,
    /// Provider payout sourced from the rent (excludes bonuses).
    pub provider_reward_due: XorAmount,
    /// PDP bonus pool earmarked per evaluation cycle.
    pub pdp_bonus_pool: XorAmount,
    /// `PoTR` bonus pool earmarked per evaluation cycle.
    pub potr_bonus_pool: XorAmount,
    /// Credit per GiB to reimburse fetch egress.
    pub egress_credit_per_gib: XorAmount,
    /// Transfer instructions required to enact the plan.
    pub instructions: Vec<InstructionBox>,
}

/// Accounts participating in the rent ledger settlement plan.
#[derive(Debug, Clone, Copy)]
pub struct DaRentLedgerAccounts<'a> {
    /// Account paying the rent bill.
    pub payer: &'a AccountId,
    /// Treasury account receiving the rent payment.
    pub treasury: &'a AccountId,
    /// Account that accrues the protocol reserve portion.
    pub protocol_reserve: &'a AccountId,
    /// Account receiving the provider reward payout.
    pub provider: &'a AccountId,
    /// Account credited with the PDP bonus portion.
    pub pdp_bonus: &'a AccountId,
    /// Account credited with the `PoTR` bonus portion.
    pub potr_bonus: &'a AccountId,
}

/// Build the transfer plan used to settle a rent ledger projection.
///
/// This mirrors the rent-ledger workflow exposed through `iroha da rent-ledger`,
/// allowing host automation to derive the same instructions without shelling
/// out to the CLI.
///
/// # Errors
///
/// Returns an error if any transfer instruction fails validation (e.g., due to invalid amounts).
pub fn build_da_rent_ledger_plan(
    projection: &DaRentLedgerProjection,
    accounts: &DaRentLedgerAccounts<'_>,
    asset_definition: &AssetDefinitionId,
) -> Result<DaRentLedgerPlan> {
    let mut instructions = Vec::new();
    push_rent_instruction(
        &mut instructions,
        accounts.payer,
        accounts.treasury,
        projection.rent_due,
        asset_definition,
    )?;
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.protocol_reserve,
        projection.protocol_reserve_due,
        asset_definition,
    )?;
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.provider,
        projection.provider_reward_due,
        asset_definition,
    )?;
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.pdp_bonus,
        projection.pdp_bonus_pool,
        asset_definition,
    )?;
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.potr_bonus,
        projection.potr_bonus_pool,
        asset_definition,
    )?;

    Ok(DaRentLedgerPlan {
        rent_due: projection.rent_due,
        protocol_reserve_due: projection.protocol_reserve_due,
        provider_reward_due: projection.provider_reward_due,
        pdp_bonus_pool: projection.pdp_bonus_pool,
        potr_bonus_pool: projection.potr_bonus_pool,
        egress_credit_per_gib: projection.egress_credit_per_gib,
        instructions,
    })
}

fn push_rent_instruction(
    instructions: &mut Vec<InstructionBox>,
    source_account: &AccountId,
    destination_account: &AccountId,
    amount: XorAmount,
    asset_definition: &AssetDefinitionId,
) -> Result<()> {
    if amount.is_zero() {
        return Ok(());
    }
    let numeric_amount = xor_amount_to_numeric(amount)?;
    let asset_id = AssetId::new(asset_definition.clone(), source_account.clone());
    let transfer = Transfer::asset_numeric(asset_id, numeric_amount, destination_account.clone());
    instructions.push(InstructionBox::from(transfer));
    Ok(())
}

fn xor_amount_to_numeric(amount: XorAmount) -> Result<Numeric> {
    Numeric::try_new(amount.as_micro(), 6)
        .map_err(|err| eyre!("failed to convert XOR amount to Numeric: {err}"))
}

fn value_from_usize(value: usize) -> Value {
    Value::from(u64::try_from(value).unwrap_or(u64::MAX))
}

fn value_from_u32(value: u32) -> Value {
    Value::from(u64::from(value))
}

fn parse_hex_digest(label: &str, hex_string: &str) -> Result<BlobDigest> {
    let trimmed = hex_string.trim();
    let bytes = hex::decode(trimmed).map_err(|err| eyre!("invalid {label} hex encoding: {err}"))?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| eyre!("{label} must decode to 32 bytes"))?;
    Ok(BlobDigest::new(array))
}

fn parse_sample_seed(label: &str, hex_string: &str) -> Result<[u8; 32]> {
    let digest = parse_hex_digest(label, hex_string)?;
    Ok(*digest.as_bytes())
}

fn parse_chunk_role(label: &str, value: &str) -> Result<ChunkRole> {
    match value {
        "data" => Ok(ChunkRole::Data),
        "local_parity" => Ok(ChunkRole::LocalParity),
        "global_parity" => Ok(ChunkRole::GlobalParity),
        "stripe_parity" => Ok(ChunkRole::StripeParity),
        other => Err(eyre!("{label} has unsupported role `{other}`")),
    }
}

fn parse_sampling_plan(value: &Value) -> Result<DaSamplingPlan> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("sampling_plan must be a JSON object"))?;
    let assignment_hex = object
        .get("assignment_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("sampling_plan.assignment_hash missing"))?;
    let assignment_hash = parse_hex_digest("sampling_plan.assignment_hash", assignment_hex)?;
    let sample_window = object
        .get("sample_window")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("sampling_plan.sample_window missing or invalid"))?;
    let sample_seed = match object.get("sample_seed").and_then(Value::as_str) {
        Some(seed_hex) => Some(parse_sample_seed("sampling_plan.sample_seed", seed_hex)?),
        None => None,
    };
    let samples_array = object
        .get("samples")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("sampling_plan.samples must be an array"))?;

    let mut samples = Vec::with_capacity(samples_array.len());
    for (idx, sample) in samples_array.iter().enumerate() {
        let sample_obj = sample
            .as_object()
            .ok_or_else(|| eyre!("sampling_plan.samples[{idx}] must be an object"))?;
        let index = sample_obj
            .get("index")
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("sampling_plan.samples[{idx}].index missing"))?;
        let role_label = sample_obj
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("sampling_plan.samples[{idx}].role missing"))?;
        let group = sample_obj.get("group").and_then(Value::as_u64).unwrap_or(0);

        samples.push(DaSampledChunk {
            index: u32::try_from(index)
                .map_err(|err| eyre!("sampling_plan.samples[{idx}].index out of range: {err}"))?,
            role: parse_chunk_role("sampling_plan.samples role", role_label)?,
            group: u32::try_from(group)
                .map_err(|err| eyre!("sampling_plan.samples[{idx}].group out of range: {err}"))?,
        });
    }

    Ok(DaSamplingPlan {
        assignment_hash,
        sample_window: u16::try_from(sample_window)
            .map_err(|err| eyre!("sampling_plan.sample_window out of range for u16: {err}"))?,
        sample_seed,
        samples,
    })
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

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use base64::engine::general_purpose::STANDARD as BASE64;
    use blake3::hash as blake3_hash;
    use iroha_crypto::Algorithm;
    use iroha_data_model::{
        asset::{AssetDefinitionId, AssetId},
        da::{
            ingest::DaStripeLayout,
            manifest::{ChunkCommitment, ChunkRole},
            types::{
                BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile,
                ExtraMetadata, FecScheme, GovernanceTag, MetadataEntry, MetadataVisibility,
                RetentionPolicy, StorageTicketId,
            },
        },
        nexus::LaneId,
        prelude::{AccountId, DomainId},
        sorafs::pin_registry::StorageClass,
    };
    use iroha_primitives::numeric::Numeric;
    use sorafs_manifest::{ChunkingProfileV1, ProfileId};
    use sorafs_orchestrator::prelude::ChunkStore;
    use tempfile::tempdir;

    use super::*;
    use crate::crypto::KeyPair;

    #[test]
    fn chunk_profile_from_chunk_size_rejects_zero() {
        let err = chunk_profile_from_chunk_size(0).expect_err("expected failure");
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
    fn build_da_request_hashes_payload_when_digest_absent() {
        let key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let params = sample_ingest_params(None);
        let payload = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let request = build_da_request(
            payload.clone(),
            &params,
            ExtraMetadata { items: Vec::new() },
            &key_pair,
            None,
        );
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let expected = BlobDigest::from_hash(hasher.finalize());
        assert_eq!(request.client_blob_id, expected);
        assert_eq!(request.payload, payload);
    }

    #[test]
    fn build_da_request_respects_digest_override() {
        let key_pair = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
        let override_digest = BlobDigest::new([0xAB; 32]);
        let params = sample_ingest_params(Some(override_digest));
        let request = build_da_request(
            vec![0x01, 0x02],
            &params,
            ExtraMetadata { items: Vec::new() },
            &key_pair,
            None,
        );
        assert_eq!(request.client_blob_id, override_digest);
        assert_eq!(request.chunk_size, params.chunk_size);
    }

    #[test]
    fn header_decodes_commitment() {
        let commitment = sample_commitment();
        let bytes = norito::to_bytes(&commitment).expect("encode commitment");
        let header_value = BASE64.encode(&bytes);
        let decoded = decode_pdp_commitment_header(&header_value).expect("decode header");
        assert_eq!(decoded, commitment);
    }

    #[test]
    fn receipt_helper_respects_absent_commitment() {
        let mut receipt = sample_receipt();
        receipt.pdp_commitment = None;
        assert!(
            receipt_pdp_commitment(&receipt)
                .expect("decode receipt commitment")
                .is_none()
        );
    }

    #[test]
    fn receipt_helper_decodes_bytes() {
        let commitment = sample_commitment();
        let bytes = norito::to_bytes(&commitment).expect("encode commitment");
        let mut receipt = sample_receipt();
        receipt.pdp_commitment = Some(bytes);
        let decoded = receipt_pdp_commitment(&receipt)
            .expect("decode commitment")
            .expect("commitment present");
        assert_eq!(decoded, commitment);
    }

    #[test]
    fn invalid_header_surfaces_error() {
        let err = decode_pdp_commitment_header("###").expect_err("expected failure");
        assert!(
            err.to_string().contains("invalid"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn da_commitment_proof_request_roundtrips_json() {
        let request = DaCommitmentProofRequest {
            manifest_hash: Some(ManifestDigest::new([0x11; 32])),
            lane_id: Some(7),
            epoch: Some(9),
            sequence: Some(12),
            pagination: Some(Pagination::new(std::num::NonZeroU64::new(2), 1)),
        };

        let bytes = norito::json::to_vec(&request).expect("encode request");
        let decoded: DaCommitmentProofRequest =
            norito::json::from_slice(&bytes).expect("decode request");

        assert_eq!(decoded.manifest_hash, request.manifest_hash);
        assert_eq!(decoded.lane_id, request.lane_id);
        assert_eq!(decoded.epoch, request.epoch);
        assert_eq!(decoded.sequence, request.sequence);
        assert_eq!(
            decoded
                .pagination
                .as_ref()
                .and_then(|pagination| pagination.limit.map(std::num::NonZeroU64::get)),
            Some(2)
        );
        assert_eq!(
            decoded
                .pagination
                .as_ref()
                .map(|pagination| pagination.offset),
            Some(1)
        );
    }

    #[test]
    fn da_pin_intent_query_request_roundtrips_json() {
        let request = DaPinIntentQueryRequest {
            manifest_hash: Some(ManifestDigest::new([0x22; 32])),
            storage_ticket: Some(StorageTicketId::new([0x33; 32])),
            alias: Some("news/latest".to_string()),
            lane_id: Some(4),
            epoch: Some(8),
            sequence: Some(16),
            pagination: Some(Pagination::new(std::num::NonZeroU64::new(5), 3)),
        };

        let bytes = norito::json::to_vec(&request).expect("encode request");
        let decoded: DaPinIntentQueryRequest =
            norito::json::from_slice(&bytes).expect("decode request");

        assert_eq!(decoded.manifest_hash, request.manifest_hash);
        assert_eq!(decoded.storage_ticket, request.storage_ticket);
        assert_eq!(decoded.alias, request.alias);
        assert_eq!(decoded.lane_id, request.lane_id);
        assert_eq!(decoded.epoch, request.epoch);
        assert_eq!(decoded.sequence, request.sequence);
        assert_eq!(
            decoded
                .pagination
                .as_ref()
                .and_then(|pagination| pagination.limit.map(std::num::NonZeroU64::get)),
            Some(5)
        );
        assert_eq!(
            decoded
                .pagination
                .as_ref()
                .map(|pagination| pagination.offset),
            Some(3)
        );
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
            Value::from(BASE64.encode([0u8; 4])),
        );
        object.insert(
            "manifest".into(),
            Value::Object(Map::from_iter([("dummy".into(), Value::from(1))])),
        );
        object.insert(
            "chunk_plan".into(),
            Value::Object(Map::from_iter([(
                "chunk_fetch_specs".into(),
                Value::Array(Vec::new()),
            )])),
        );
        object.insert("manifest_hash".into(), Value::from("55".repeat(32)));
        let json = Value::Object(object);
        let bundle = DaManifestBundle::from_json(&json).expect("bundle");
        assert_eq!(bundle.storage_ticket_hex, "11".repeat(32));
        assert_eq!(bundle.client_blob_id_hex, "22".repeat(32));
        assert_eq!(bundle.blob_hash_hex, "33".repeat(32));
        assert_eq!(bundle.chunk_root_hex, "44".repeat(32));
        assert_eq!(bundle.manifest_hash_hex, "55".repeat(32));
        assert_eq!(bundle.lane_id, 0);
        assert_eq!(bundle.epoch, 1);
        assert!(bundle.sampling_plan.is_none());
    }

    #[test]
    fn manifest_bundle_parses_sampling_plan() {
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
            Value::from(BASE64.encode([0u8; 4])),
        );
        object.insert(
            "chunk_plan".into(),
            Value::Object(Map::from_iter([(
                "chunk_fetch_specs".into(),
                Value::Array(Vec::new()),
            )])),
        );
        object.insert("manifest_hash".into(), Value::from("66".repeat(32)));
        object.insert(
            "sampling_plan".into(),
            Value::Object(Map::from_iter([
                ("assignment_hash".into(), Value::from("aa".repeat(32))),
                ("sample_window".into(), Value::from(4)),
                ("sample_seed".into(), Value::from("bb".repeat(32))),
                (
                    "samples".into(),
                    Value::Array(vec![Value::Object(Map::from_iter([
                        ("index".into(), Value::from(1)),
                        ("role".into(), Value::from("data")),
                        ("group".into(), Value::from(0)),
                    ]))]),
                ),
            ])),
        );

        let json = Value::Object(object);
        let bundle = DaManifestBundle::from_json(&json).expect("bundle");
        let plan = bundle.sampling_plan.expect("sampling plan");
        assert_eq!(plan.sample_window, 4);
        assert_eq!(plan.assignment_hash, BlobDigest::new([0xaa; 32]));
        assert_eq!(plan.sample_seed, Some([0xbb; 32]));
        assert_eq!(plan.samples.len(), 1);
        assert_eq!(plan.samples[0].index, 1);
        assert_eq!(plan.samples[0].role, ChunkRole::Data);
        assert_eq!(plan.samples[0].group, 0);
    }

    #[test]
    fn car_plan_matches_manifest_metadata() {
        let (manifest, _) = sample_manifest_and_payload();
        let plan = build_car_plan_from_manifest(&manifest).expect("plan");
        assert_eq!(plan.chunks.len(), manifest.chunks.len());
        assert_eq!(plan.content_length, manifest.total_size);
        assert_eq!(plan.payload_digest.as_bytes(), manifest.blob_hash.as_ref());
    }

    #[test]
    fn proof_summary_contains_expected_fields() {
        let (manifest, payload) = sample_manifest_and_payload();
        let summary = generate_da_proof_summary(&manifest, &payload, &DaProofConfig::default())
            .expect("summary");
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
                .is_some_and(|array| !array.is_empty()),
            "proof array missing"
        );
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
                .expect("artifact");
        let map = artifact.as_object().expect("artifact object");
        assert_eq!(
            map.get("manifest_path").and_then(Value::as_str),
            Some("/tmp/manifests/sample_manifest.norito")
        );
        assert_eq!(
            map.get("payload_path").and_then(Value::as_str),
            Some("/tmp/payloads/sample_payload.car")
        );
        assert!(map.contains_key("por_root"), "missing por_root");
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
            |proof, por_root| {
                assert!(proof.verify(por_root));
                (Duration::from_millis(5), true)
            },
        )
        .expect("benchmark");
        assert_eq!(bench.proof_count, 1);
        assert_eq!(bench.requested_sample_count, 1);
        assert_eq!(bench.explicit_index_count, 0);
        assert!(bench.within_budget);
        assert_eq!(bench.total_duration_ms, 5);
        assert_eq!(bench.max_duration_ms, 5);
        assert_eq!(bench.average_duration_ms, 5);
        assert_eq!(bench.payload_bytes, manifest.total_size);
        assert_eq!(bench.chunk_size, manifest.chunk_size);
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
            |proof, por_root| {
                assert!(proof.verify(por_root));
                (Duration::from_millis(10), true)
            },
        )
        .expect("benchmark");
        assert_eq!(bench.proof_count, 1);
        assert!(!bench.within_budget);
        assert_eq!(bench.total_duration_ms, 10);
        assert_eq!(bench.max_duration_ms, 10);
        assert_eq!(bench.budget_ms, 1);
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
            |_proof, _por_root| (Duration::ZERO, false),
        )
        .expect_err("expected verification failure");
        assert!(
            err.to_string().contains("failed verification"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn manifest_bundle_persist_to_dir_writes_outputs() {
        let (manifest, _) = sample_manifest_and_payload();
        let manifest_bytes = norito::to_bytes(&manifest).expect("encode manifest");
        let bundle = DaManifestBundle {
            storage_ticket_hex: "11".repeat(32),
            client_blob_id_hex: "22".repeat(32),
            blob_hash_hex: hex::encode(manifest.blob_hash.as_ref()),
            chunk_root_hex: hex::encode(manifest.chunk_root.as_ref()),
            manifest_hash_hex: hex::encode(blake3_hash(&manifest_bytes).as_bytes()),
            lane_id: u64::from(manifest.lane_id.as_u32()),
            epoch: manifest.epoch,
            manifest_len: manifest_bytes.len() as u64,
            manifest_bytes: manifest_bytes.clone(),
            manifest_json: norito::json::value::to_value(&manifest).expect("manifest json"),
            chunk_plan: Value::Object(Map::from_iter([(
                "chunk_fetch_specs".into(),
                Value::Array(Vec::new()),
            )])),
            sampling_plan: None,
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
            norito::json::from_slice(chunk_plan_json.as_bytes()).expect("chunk plan json parses");
        assert!(chunk_plan_value.is_object());
    }

    #[test]
    fn rent_ledger_plan_emits_expected_transfers() {
        let projection = DaRentLedgerProjection {
            rent_due: XorAmount::from_micro(1_500_000),
            protocol_reserve_due: XorAmount::from_micro(250_000),
            provider_reward_due: XorAmount::from_micro(1_250_000),
            pdp_bonus_pool: XorAmount::from_micro(50_000),
            potr_bonus_pool: XorAmount::from_micro(25_000),
            egress_credit_per_gib: XorAmount::from_micro(12_000),
        };
        let _wonderland_domain: DomainId = "wonderland".parse().expect("domain");
        let _sora_domain: DomainId = "sora".parse().expect("domain");

        let payer_key = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
        let payer = AccountId::new(payer_key.public_key().clone());

        let treasury_key = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
        let treasury = AccountId::new(treasury_key.public_key().clone());

        let reserve_key = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        let protocol_reserve = AccountId::new(reserve_key.public_key().clone());

        let provider_key = KeyPair::from_seed(vec![4; 32], Algorithm::Ed25519);
        let provider = AccountId::new(provider_key.public_key().clone());

        let pdp_key = KeyPair::from_seed(vec![5; 32], Algorithm::Ed25519);
        let pdp_bonus = AccountId::new(pdp_key.public_key().clone());

        let potr_key = KeyPair::from_seed(vec![6; 32], Algorithm::Ed25519);
        let potr_bonus = AccountId::new(potr_key.public_key().clone());
        let asset_definition: AssetDefinitionId = "xor#wonderland".parse().expect("xor asset");

        let accounts = DaRentLedgerAccounts {
            payer: &payer,
            treasury: &treasury,
            protocol_reserve: &protocol_reserve,
            provider: &provider,
            pdp_bonus: &pdp_bonus,
            potr_bonus: &potr_bonus,
        };
        let plan = build_da_rent_ledger_plan(&projection, &accounts, &asset_definition)
            .expect("ledger plan");

        assert_eq!(plan.rent_due, projection.rent_due);
        assert_eq!(plan.protocol_reserve_due, projection.protocol_reserve_due);
        assert_eq!(plan.provider_reward_due, projection.provider_reward_due);
        assert_eq!(plan.egress_credit_per_gib, projection.egress_credit_per_gib);

        let rent_amount = Numeric::try_new(projection.rent_due.as_micro(), 6).expect("numeric");
        let reserve_amount =
            Numeric::try_new(projection.protocol_reserve_due.as_micro(), 6).expect("numeric");
        let provider_amount =
            Numeric::try_new(projection.provider_reward_due.as_micro(), 6).expect("numeric");
        let pdp_amount =
            Numeric::try_new(projection.pdp_bonus_pool.as_micro(), 6).expect("numeric");
        let potr_amount =
            Numeric::try_new(projection.potr_bonus_pool.as_micro(), 6).expect("numeric");

        let expected = vec![
            InstructionBox::from(Transfer::asset_numeric(
                AssetId::new(asset_definition.clone(), payer.clone()),
                rent_amount,
                treasury.clone(),
            )),
            InstructionBox::from(Transfer::asset_numeric(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                reserve_amount,
                protocol_reserve.clone(),
            )),
            InstructionBox::from(Transfer::asset_numeric(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                provider_amount,
                provider.clone(),
            )),
            InstructionBox::from(Transfer::asset_numeric(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                pdp_amount,
                pdp_bonus.clone(),
            )),
            InstructionBox::from(Transfer::asset_numeric(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                potr_amount,
                potr_bonus.clone(),
            )),
        ];
        assert_eq!(plan.instructions, expected);
    }

    fn sample_commitment() -> PdpCommitmentV1 {
        PdpCommitmentV1 {
            version: sorafs_manifest::pdp::PDP_COMMITMENT_VERSION_V1,
            manifest_digest: [0x11; 32],
            chunk_profile: ChunkingProfileV1 {
                profile_id: ProfileId(7),
                namespace: "inline".to_string(),
                name: "inline".to_string(),
                semver: "1.0.0".to_string(),
                min_size: 64 * 1024,
                target_size: 64 * 1024,
                max_size: 64 * 1024,
                break_mask: 1,
                multihash_code: sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
                aliases: vec!["inline.inline@1.0.0".to_string()],
            },
            commitment_root_hot: [0x22; 32],
            commitment_root_segment: [0x33; 32],
            hash_algorithm: sorafs_manifest::pdp::HashAlgorithmV1::Blake3_256,
            hot_tree_height: 5,
            segment_tree_height: 4,
            sample_window: 16,
            sealed_at: 1_701_800_000,
        }
    }

    fn sample_receipt() -> DaIngestReceipt {
        DaIngestReceipt {
            client_blob_id: iroha_data_model::da::types::BlobDigest::new([0xAA; 32]),
            lane_id: LaneId::new(7),
            epoch: 9,
            blob_hash: iroha_data_model::da::types::BlobDigest::new([0xBB; 32]),
            chunk_root: iroha_data_model::da::types::BlobDigest::new([0xCC; 32]),
            manifest_hash: iroha_data_model::da::types::BlobDigest::new([0xDD; 32]),
            storage_ticket: iroha_data_model::da::types::StorageTicketId::new([0u8; 32]),
            pdp_commitment: None,
            stripe_layout: DaStripeLayout {
                total_stripes: 1,
                shards_per_stripe: 1,
                row_parity_stripes: 0,
            },
            queued_at_unix: 1_701_900_000,
            rent_quote: DaRentQuote::default(),
            operator_signature: iroha_crypto::Signature::from_bytes(&[0u8; 64]),
        }
    }

    fn sample_manifest_and_payload() -> (DaManifestV1, Vec<u8>) {
        let payload = vec![0xAB; 8];
        let mut store = ChunkStore::new();
        store.ingest_bytes(&payload);
        let chunk_commitments = store
            .chunks()
            .iter()
            .enumerate()
            .map(|(idx, chunk)| {
                let idx = u32::try_from(idx).expect("chunk index fits in u32");
                ChunkCommitment::new_with_role(
                    idx,
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
        let chunk_size = chunk_commitments.first().map_or_else(
            || u32::try_from(payload.len()).expect("payload length fits in u32"),
            |commitment| commitment.length,
        );
        let total_stripes = u32::try_from(
            chunk_commitments
                .len()
                .div_ceil(usize::from(ErasureProfile::default().data_shards)),
        )
        .expect("stripe count fits in u32");
        let shards_per_stripe = u32::from(
            ErasureProfile::default()
                .data_shards
                .saturating_add(ErasureProfile::default().parity_shards),
        );
        let metadata = ExtraMetadata {
            items: vec![
                MetadataEntry::new(
                    "taikai.event_id",
                    b"event".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.stream_id",
                    b"stream".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.rendition_id",
                    b"main".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.segment.sequence",
                    b"0".to_vec(),
                    MetadataVisibility::Public,
                ),
            ],
        };
        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(0),
            epoch: 1,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new("custom.binary".to_owned()),
            blob_hash,
            chunk_root,
            storage_ticket: StorageTicketId::new([0u8; 32]),
            total_size: payload.len() as u64,
            chunk_size,
            total_stripes,
            shards_per_stripe,
            erasure_profile: ErasureProfile {
                data_shards: 1,
                parity_shards: 0,
                row_parity_stripes: 0,
                chunk_alignment: 1,
                fec_scheme: iroha_data_model::da::types::FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 0,
                cold_retention_secs: 0,
                required_replicas: 1,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
                governance_tag: GovernanceTag::new("da.test".to_owned()),
            },
            rent_quote: DaRentQuote::default(),
            chunks: chunk_commitments,
            ipa_commitment: chunk_root,
            metadata,
            issued_at_unix: 0,
        };
        (manifest, payload)
    }

    fn sample_ingest_params(override_digest: Option<BlobDigest>) -> DaIngestParams {
        DaIngestParams {
            lane_id: LaneId::new(5),
            epoch: 9,
            sequence: 3,
            blob_class: BlobClass::NexusLaneSidecar,
            blob_codec: BlobCodec::new("custom.binary"),
            erasure_profile: ErasureProfile {
                data_shards: 10,
                parity_shards: 4,
                row_parity_stripes: 0,
                chunk_alignment: 8,
                fec_scheme: FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 600,
                cold_retention_secs: 3_600,
                required_replicas: 3,
                storage_class: StorageClass::Warm,
                governance_tag: GovernanceTag::new("da.tests"),
            },
            chunk_size: 262_144,
            client_blob_id: override_digest,
        }
    }
}
