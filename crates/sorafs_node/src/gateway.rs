//! SoraFS gateway for the trustless delivery profile (SF-5).
//!
//! The gateway serves CAR/proof responses derived from the local SoraFS storage
//! backend instead of fixture bundles. It is pinned to a single manifest digest
//! at startup and uses live payload/chunk data for every request.

use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    ops::RangeInclusive,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Path as AxumPath, State},
    http::{
        HeaderMap, StatusCode,
        header::{
            ACCEPT, ACCEPT_ENCODING, ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE,
            CONTENT_TYPE, HOST, HeaderName, HeaderValue, RANGE, RETRY_AFTER,
        },
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine as _;
use eyre::WrapErr;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature};
use iroha_logger::{info, warn};
use iroha_telemetry::metrics::{SorafsGatewayOtel, global_sorafs_gateway_otel};
use norito::json::{self, Value};
use sorafs_car::{CarBuildPlan, CarChunk, CarWriter, FilePlan, PorMerkleTree};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    ManifestV1,
    gateway_fixture::SORAFS_GATEWAY_PROFILE_VERSION,
    por::{POR_PROOF_VERSION_V1, PorProofSampleV1, PorProofV1},
    provider_advert::{AdvertSignature, SignatureAlgorithm},
};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{NodeHandle, config::StorageConfig};

const HEADER_VERSION: &str = "x-sorafs-version";
const HEADER_NONCE: &str = "x-sorafs-nonce";
const HEADER_MANIFEST_ENVELOPE: &str = "x-sorafs-manifest-envelope";
const HEADER_CHUNKER: &str = "x-sorafs-chunker";
const HEADER_PROOF_DIGEST: &str = "x-sorafs-proof-digest";
const HEADER_POR_ROOT: &str = "x-sorafs-por-root";
const HEADER_CHUNK_RANGE: &str = "x-sora-chunk-range";
const HEADER_STREAM_TOKEN: &str = "x-sorafs-stream-token";
const HEADER_CLIENT_ID: &str = "x-sorafs-client";
const HEADER_CLIENT_QUOTA_REMAINING: &str = "x-sorafs-client-quota-remaining";
const HEADER_ALIAS: &str = "sora-name";
const HEADER_SORA_CONTENT_CID: &str = "sora-content-cid";
const HEADER_SORA_PROOF: &str = "sora-proof";
const HEADER_SORA_PROOF_STATUS: &str = "sora-proof-status";
const HEADER_SORA_ROUTE_BINDING: &str = "sora-route-binding";
const HEADER_PERMISSIONS_POLICY: &str = "permissions-policy";
const DAG_SCOPE_HEADER_LABEL: &str = "Sora-Dag-Scope";
const SUPPORTED_CAPABILITIES: &[&str] = &["sorafs.chunk-range.block"];
const DEFAULT_PROOF_STATUS: &str = "ok";
const DEFAULT_CSP_TEMPLATE: &str = "default-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'";
const DEFAULT_PERMISSIONS_TEMPLATE: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), camera=(), clipboard-read=(self), clipboard-write=(self), encrypted-media=(), fullscreen=(self), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), speaker-selection=(), usb=(), xr-spatial-tracking=()";
const DEFAULT_HSTS_TEMPLATE: &str = "max-age=63072000; includeSubDomains; preload";

/// Shared gateway dataset loaded from the local storage backend.
#[derive(Debug)]
pub struct GatewayDataset {
    manifest: ManifestV1,
    manifest_id_hex: String,
    content_cid: String,
    chunker_alias: String,
    provider_id: [u8; 32],
    car_bytes: Arc<Vec<u8>>,
    payload_bytes: Arc<Vec<u8>>,
    plan: CarBuildPlan,
    por_tree: PorMerkleTree,
    proof: PorProofV1,
    proof_digest_hex: String,
    por_root_hex: String,
    profile_version: String,
    route_generated_at: String,
    proof_verified: AtomicBool,
}

impl GatewayDataset {
    /// Load a gateway dataset from the local storage backend.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest digest is unknown, storage is disabled,
    /// or the stored payload does not match its manifest metadata.
    pub fn load_from_storage(
        node: &NodeHandle,
        manifest_digest_hex: &str,
    ) -> Result<Self, eyre::Report> {
        let provider_id = node.capacity_usage().provider_id.ok_or_else(|| {
            eyre::eyre!("gateway provider_id missing; record a capacity declaration")
        })?;
        Self::load_from_storage_with_provider(node, manifest_digest_hex, provider_id)
    }

    /// Load a gateway dataset using an explicit provider identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest digest is unknown, storage is disabled,
    /// the signing key is unavailable, or the stored payload does not match its manifest metadata.
    pub fn load_from_storage_with_provider(
        node: &NodeHandle,
        manifest_digest_hex: &str,
        provider_id: [u8; 32],
    ) -> Result<Self, eyre::Report> {
        let digest_bytes =
            hex::decode(manifest_digest_hex).wrap_err("manifest digest must be hex")?;
        let digest: [u8; 32] = digest_bytes
            .as_slice()
            .try_into()
            .map_err(|_| eyre::eyre!("manifest digest must be 32 bytes"))?;

        let stored = node
            .manifest_metadata_by_digest(&digest)
            .map_err(|err| eyre::eyre!("{err}"))?;

        let manifest = stored
            .load_manifest()
            .wrap_err("failed to decode stored manifest")?;
        let manifest_digest = manifest
            .digest()
            .wrap_err("failed to hash stored manifest")?;
        let manifest_id_hex = hex::encode(manifest_digest.as_bytes());
        if !equal_hex(&manifest_id_hex, manifest_digest_hex) {
            return Err(eyre::eyre!(
                "stored manifest digest does not match requested digest"
            ));
        }

        let chunker_alias = canonical_chunker_alias(&manifest);
        let content_cid = manifest_content_cid(&manifest);
        let profile = chunk_profile_for_manifest(&manifest)?;
        let plan = stored.to_car_plan(profile);

        let content_len = usize::try_from(manifest.content_length)
            .map_err(|_| eyre::eyre!("manifest content length exceeds supported size"))?;
        let payload_raw = node
            .read_payload_range(stored.manifest_id(), 0, content_len)
            .map_err(|err| eyre::eyre!("{err}"))?;
        if payload_raw.len() != content_len {
            return Err(eyre::eyre!("payload length mismatch"));
        }

        let mut car_bytes = Vec::new();
        CarWriter::new(&plan, &payload_raw)
            .map_err(|err| eyre::eyre!(err.to_string()))?
            .write_to(&mut car_bytes)
            .map_err(|err| eyre::eyre!(err.to_string()))?;

        let por_tree = stored.por_tree();
        let signing_key = load_gateway_signing_key(node.config())?;
        let proof = build_por_proof(&por_tree, &payload_raw, digest, provider_id, &signing_key)?;
        let proof_digest_hex = hex::encode(proof.proof_digest());
        let por_root_hex = hex::encode(por_tree.root());

        let route_generated_at = format_generated_at(unix_now()?)?;

        let dataset = Self {
            manifest,
            manifest_id_hex,
            content_cid,
            chunker_alias,
            provider_id,
            car_bytes: Arc::new(car_bytes),
            payload_bytes: Arc::new(payload_raw),
            plan,
            por_tree,
            proof,
            proof_digest_hex,
            por_root_hex,
            profile_version: SORAFS_GATEWAY_PROFILE_VERSION.to_string(),
            route_generated_at,
            proof_verified: AtomicBool::new(false),
        };

        dataset
            .verify_proof()
            .map_err(|err| eyre::eyre!(err.to_string()))?;
        Ok(dataset)
    }

    /// Hex-encoded manifest digest routed by the gateway.
    #[must_use]
    pub fn manifest_id_hex(&self) -> &str {
        &self.manifest_id_hex
    }

    /// Canonical chunker alias advertised in responses.
    #[must_use]
    pub fn chunker_alias(&self) -> &str {
        &self.chunker_alias
    }

    /// Canonical manifest identifier encoded as Sora-Content-CID (base32 multibase).
    #[must_use]
    pub fn content_cid(&self) -> &str {
        &self.content_cid
    }

    /// Length in bytes of the served CAR archive.
    #[must_use]
    pub fn car_len(&self) -> usize {
        self.car_bytes.len()
    }

    /// Reference to the canonical manifest used by the gateway.
    #[must_use]
    pub fn manifest(&self) -> &ManifestV1 {
        &self.manifest
    }

    /// Hex-encoded proof digest associated with the dataset.
    #[must_use]
    pub fn proof_digest_hex(&self) -> &str {
        &self.proof_digest_hex
    }

    /// Hex-encoded PoR tree root derived from the payload.
    #[must_use]
    pub fn por_root_hex(&self) -> &str {
        &self.por_root_hex
    }

    /// Profile version expected in `X-SoraFS-Version`.
    #[must_use]
    pub fn profile_version(&self) -> &str {
        &self.profile_version
    }

    /// RFC3339 timestamp used when emitting `Sora-Route-Binding` headers.
    #[must_use]
    pub fn route_generated_at(&self) -> &str {
        &self.route_generated_at
    }

    /// Chunk plan derived from the canonical payload.
    #[must_use]
    pub fn plan(&self) -> &CarBuildPlan {
        &self.plan
    }

    /// Provider identifier for the dataset encoded as lowercase hex.
    #[must_use]
    pub fn provider_id_hex(&self) -> String {
        hex::encode(self.provider_id)
    }

    fn proof(&self) -> &PorProofV1 {
        &self.proof
    }

    fn validate_proof(&self) -> Result<(), GatewayResponseError> {
        let refusal = |reason: String, details: Option<Value>| {
            GatewayResponseError::capability_refusal_with_details(
                StatusCode::UNPROCESSABLE_ENTITY,
                "proof_mismatch",
                reason,
                details,
            )
        };

        if let Err(err) = self.proof.validate() {
            let mut details = json::Map::new();
            details.insert("error".into(), Value::from(err.to_string()));
            return Err(refusal(
                "proof payload failed validation".to_string(),
                Some(Value::Object(details)),
            ));
        }

        if self.proof.provider_id != self.provider_id {
            let mut details = json::Map::new();
            details.insert(
                "provider_id".into(),
                Value::from(hex::encode(self.proof.provider_id)),
            );
            return Err(refusal(
                "proof provider id does not match gateway dataset".to_string(),
                Some(Value::Object(details)),
            ));
        }

        let proof_manifest_hex = hex::encode(self.proof.manifest_digest);
        if !equal_hex(&proof_manifest_hex, self.manifest_id_hex()) {
            let mut details = json::Map::new();
            details.insert(
                "manifest_digest".into(),
                Value::from(proof_manifest_hex.to_string()),
            );
            return Err(refusal(
                "proof manifest digest does not match gateway dataset".to_string(),
                Some(Value::Object(details)),
            ));
        }

        if self.por_tree.is_empty() {
            return Err(refusal("gateway PoR tree is empty".to_string(), None));
        }

        let expected_roots: Vec<[u8; 32]> = self
            .por_tree
            .chunks()
            .iter()
            .map(|chunk| chunk.root)
            .collect();
        if self.proof.auth_path != expected_roots {
            let mut details = json::Map::new();
            details.insert(
                "auth_path_len".into(),
                Value::from(self.proof.auth_path.len() as u64),
            );
            details.insert(
                "expected_len".into(),
                Value::from(expected_roots.len() as u64),
            );
            return Err(refusal(
                "proof authentication path does not match gateway tree".to_string(),
                Some(Value::Object(details)),
            ));
        }

        for (index, sample) in self.proof.samples.iter().enumerate() {
            let sample_index = usize::try_from(sample.sample_index).map_err(|_| {
                refusal(
                    "proof sample index exceeds supported range".to_string(),
                    Some(Value::from(index as u64)),
                )
            })?;

            let (chunk_idx, segment_idx, leaf_idx) =
                self.por_tree.leaf_path(sample_index).ok_or_else(|| {
                    refusal(
                        "proof sample index does not map to a PoR leaf".to_string(),
                        Some(Value::from(sample.sample_index)),
                    )
                })?;

            let chunk = &self.por_tree.chunks()[chunk_idx];
            let segment = &chunk.segments[segment_idx];
            let leaf = &segment.leaves[leaf_idx];

            if sample.chunk_offset != chunk.offset
                || sample.chunk_size != chunk.length
                || sample.chunk_digest != chunk.chunk_digest
                || sample.leaf_digest != leaf.digest
            {
                let mut details = json::Map::new();
                details.insert(
                    "section".into(),
                    Value::from(format!("proof.chunks[{index}]")),
                );
                details.insert("sample_index".into(), Value::from(sample.sample_index));
                details.insert("chunk_offset".into(), Value::from(sample.chunk_offset));
                details.insert("chunk_size".into(), Value::from(sample.chunk_size));
                return Err(refusal(
                    "proof sample does not match gateway tree".to_string(),
                    Some(Value::Object(details)),
                ));
            }
        }

        verify_proof_signature(self.proof())?;
        Ok(())
    }

    fn verify_proof(&self) -> Result<(), GatewayResponseError> {
        if self.proof_verified.load(Ordering::Acquire) {
            return Ok(());
        }

        self.validate_proof()?;
        self.proof_verified.store(true, Ordering::Release);
        Ok(())
    }

    #[doc(hidden)]
    pub fn proof_mut_for_testing(&mut self) -> &mut PorProofV1 {
        self.proof_verified.store(false, Ordering::Release);
        &mut self.proof
    }

    #[doc(hidden)]
    pub fn verify_proof_for_testing(&self) -> Result<(), GatewayResponseError> {
        self.verify_proof()
    }

    fn payload_slice(&self, start: u64, len: u64) -> Option<&[u8]> {
        let start_usize = usize::try_from(start).ok()?;
        let len_usize = usize::try_from(len).ok()?;
        let end = start_usize.checked_add(len_usize)?;
        self.payload_bytes.get(start_usize..end)
    }

    fn chunk_range(
        &self,
        range: RangeInclusive<u64>,
    ) -> Result<ChunkRangeInfo, GatewayResponseError> {
        let start = *range.start();
        let end = *range.end();
        if start > end {
            return Err(GatewayResponseError::invalid_range("start must be <= end"));
        }
        let content_length = self.manifest.content_length;
        if end >= content_length {
            return Err(GatewayResponseError::invalid_range(
                "range exceeds content length",
            ));
        }

        let mut chunk_indices = Vec::new();
        let mut expected_start = start;
        for (idx, chunk) in self.plan.chunks.iter().enumerate() {
            let chunk_start = chunk.offset;
            let chunk_end = chunk
                .offset
                .checked_add(u64::from(chunk.length))
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| GatewayResponseError::invalid_range("chunk offset overflow"))?;
            if chunk_end < start {
                continue;
            }
            if chunk_start > end {
                break;
            }
            if chunk_start != expected_start {
                return Err(GatewayResponseError::invalid_range(
                    "range must align with chunk boundaries",
                ));
            }
            chunk_indices.push(idx);
            expected_start = chunk_end
                .checked_add(1)
                .ok_or_else(|| GatewayResponseError::invalid_range("range overflow"))?;
            if chunk_end == end {
                break;
            }
        }

        if chunk_indices.is_empty() {
            return Err(GatewayResponseError::invalid_range(
                "no chunks match requested range",
            ));
        }

        if expected_start != end.checked_add(1).unwrap_or(0) {
            return Err(GatewayResponseError::invalid_range(
                "range must cover contiguous chunk span",
            ));
        }

        let total_payload_len = end
            .checked_sub(start)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| GatewayResponseError::invalid_range("range overflow"))?;

        Ok(ChunkRangeInfo {
            start,
            end,
            chunk_indices,
            payload_len: total_payload_len,
        })
    }

    fn build_block_car(&self, range: &ChunkRangeInfo) -> Result<Vec<u8>, GatewayResponseError> {
        let mut range_chunks = Vec::with_capacity(range.chunk_indices.len());
        let mut relative_offset = 0u64;
        for &idx in &range.chunk_indices {
            let chunk =
                self.plan.chunks.get(idx).ok_or_else(|| {
                    GatewayResponseError::invalid_range("chunk index out of range")
                })?;
            range_chunks.push(CarChunk {
                offset: relative_offset,
                length: chunk.length,
                digest: chunk.digest,
                taikai_segment_hint: chunk.taikai_segment_hint.clone(),
            });
            relative_offset = relative_offset
                .checked_add(u64::from(chunk.length))
                .ok_or_else(|| GatewayResponseError::invalid_range("chunk length overflow"))?;
        }

        let payload = self
            .payload_slice(range.start, range.payload_len)
            .ok_or_else(|| {
                GatewayResponseError::internal(eyre::eyre!("range payload slice out of bounds"))
            })?;
        let range_payload = payload.to_vec();
        let payload_digest = blake3::hash(&range_payload);

        let sub_plan = CarBuildPlan {
            chunk_profile: self.plan.chunk_profile,
            payload_digest,
            content_length: range.payload_len,
            chunks: range_chunks,
            files: vec![FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: range.chunk_indices.len(),
                size: range.payload_len,
            }],
        };

        let mut car_bytes = Vec::new();
        let writer = CarWriter::new(&sub_plan, &range_payload)
            .map_err(|err| GatewayResponseError::internal(err.into()))?;
        writer
            .write_to(&mut car_bytes)
            .map_err(|err| GatewayResponseError::internal(err.into()))?;

        Ok(car_bytes)
    }
}

fn manifest_content_cid(manifest: &ManifestV1) -> String {
    let encoded = encode_base32_lower(&manifest.root_cid);
    format!("b{encoded}")
}

fn encode_base32_lower(data: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if data.is_empty() {
        return String::new();
    }
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity((data.len() * 8).div_ceil(5));
    for byte in data {
        acc = (acc << 8) | (*byte as u32);
        bits += 8;
        while bits >= 5 {
            let index = ((acc >> (bits - 5)) & 0x1f) as usize;
            out.push(ALPHABET[index]);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[index]);
    }
    String::from_utf8(out).expect("base32 alphabet valid")
}

fn format_generated_at(unix: u64) -> Result<String, eyre::Report> {
    let timestamp =
        i64::try_from(unix).map_err(|err| eyre::eyre!("generated_at does not fit i64: {err}"))?;
    let datetime = OffsetDateTime::from_unix_timestamp(timestamp)
        .map_err(|err| eyre::eyre!("invalid generated_at timestamp: {err}"))?;
    datetime
        .format(&Rfc3339)
        .map_err(|err| eyre::eyre!(err.to_string()))
}

fn unix_now() -> Result<u64, eyre::Report> {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| eyre::eyre!("system clock before UNIX_EPOCH: {err}"))
}

fn chunk_profile_for_manifest(manifest: &ManifestV1) -> Result<ChunkProfile, eyre::Report> {
    if let Some(descriptor) =
        sorafs_manifest::chunker_registry::lookup(manifest.chunking.profile_id)
    {
        if descriptor.multihash_code != manifest.chunking.multihash_code {
            return Err(eyre::eyre!(
                "manifest multihash code {} does not match registry descriptor {}",
                manifest.chunking.multihash_code,
                descriptor.multihash_code
            ));
        }
        Ok(descriptor.profile)
    } else {
        let min_size = usize::try_from(manifest.chunking.min_size)
            .map_err(|_| eyre::eyre!("manifest min_size exceeds supported range"))?;
        let target_size = usize::try_from(manifest.chunking.target_size)
            .map_err(|_| eyre::eyre!("manifest target_size exceeds supported range"))?;
        let max_size = usize::try_from(manifest.chunking.max_size)
            .map_err(|_| eyre::eyre!("manifest max_size exceeds supported range"))?;
        let profile = ChunkProfile {
            min_size,
            target_size,
            max_size,
            break_mask: u64::from(manifest.chunking.break_mask),
        };
        if profile.min_size == 0
            || profile.target_size == 0
            || profile.max_size == 0
            || profile.break_mask == 0
        {
            return Err(eyre::eyre!(
                "manifest chunking profile parameters must be non-zero"
            ));
        }
        if profile.min_size > profile.target_size || profile.target_size > profile.max_size {
            return Err(eyre::eyre!(
                "manifest chunking profile sizes must satisfy min <= target <= max"
            ));
        }
        Ok(profile)
    }
}

fn build_por_proof(
    por_tree: &PorMerkleTree,
    payload: &[u8],
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    signing_key: &PrivateKey,
) -> Result<PorProofV1, eyre::Report> {
    let (chunk_idx, segment_idx, leaf_idx) = por_tree
        .leaf_path(0)
        .ok_or_else(|| eyre::eyre!("PoR tree has no leaves"))?;
    let proof = por_tree
        .prove_leaf(chunk_idx, segment_idx, leaf_idx, payload)
        .ok_or_else(|| eyre::eyre!("failed to build PoR proof from payload"))?;
    let sample = PorProofSampleV1 {
        sample_index: 0,
        chunk_offset: proof.chunk_offset,
        chunk_size: proof.chunk_length,
        chunk_digest: proof.chunk_digest,
        leaf_digest: proof.leaf_digest,
    };
    let auth_path = proof.chunk_roots.clone();
    let mut por_proof = PorProofV1 {
        version: POR_PROOF_VERSION_V1,
        challenge_id: manifest_digest,
        manifest_digest,
        provider_id,
        samples: vec![sample],
        auth_path,
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        },
        submitted_at: unix_now()?,
    };

    let proof_digest = por_proof.proof_digest();
    let keypair = KeyPair::from_private_key(signing_key.clone())
        .wrap_err("failed to derive gateway signing keypair")?;
    let signature = Signature::new(signing_key, proof_digest.as_ref());
    let (_, public_key) = keypair.public_key().to_bytes();
    por_proof.signature = AdvertSignature {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: public_key.to_vec(),
        signature: signature.payload().to_vec(),
    };

    Ok(por_proof)
}

fn load_gateway_signing_key(config: &StorageConfig) -> Result<PrivateKey, eyre::Report> {
    let path = config
        .stream_token_signing_key_path()
        .ok_or_else(|| eyre::eyre!("gateway signing key path not configured"))?;
    let raw = fs::read(path)
        .wrap_err_with(|| format!("failed to read gateway signing key from {}", path.display()))?;

    let trimmed = String::from_utf8_lossy(&raw).trim().to_owned();
    let key_bytes = if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(trimmed).wrap_err("failed to decode hex signing key")?
    } else {
        raw
    };

    if key_bytes.len() != 32 {
        return Err(eyre::eyre!(
            "gateway signing key at {} must be 32 bytes, found {}",
            path.display(),
            key_bytes.len()
        ));
    }

    let mut array = [0u8; 32];
    array.copy_from_slice(&key_bytes);
    PrivateKey::from_bytes(Algorithm::Ed25519, &array)
        .wrap_err("failed to parse gateway signing key")
}

fn verify_proof_signature(proof: &PorProofV1) -> Result<(), GatewayResponseError> {
    let refusal = |reason: String, details: Option<Value>| {
        GatewayResponseError::capability_refusal_with_details(
            StatusCode::UNPROCESSABLE_ENTITY,
            "proof_mismatch",
            reason,
            details,
        )
    };

    if proof.signature.algorithm != SignatureAlgorithm::Ed25519 {
        let mut details = json::Map::new();
        details.insert(
            "algorithm".into(),
            Value::from(format!("{:?}", proof.signature.algorithm)),
        );
        return Err(refusal(
            "proof signature algorithm must be Ed25519".to_string(),
            Some(Value::Object(details)),
        ));
    }

    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &proof.signature.public_key)
        .map_err(|_| {
            let mut details = json::Map::new();
            details.insert(
                "public_key_len".into(),
                Value::from(proof.signature.public_key.len() as u64),
            );
            refusal(
                "proof signature public key is invalid".to_string(),
                Some(Value::Object(details)),
            )
        })?;
    let signature = Signature::from_bytes(&proof.signature.signature);
    let digest = proof.proof_digest();
    signature.verify(&public_key, digest.as_ref()).map_err(|_| {
        let mut details = json::Map::new();
        details.insert(
            "signature_len".into(),
            Value::from(proof.signature.signature.len() as u64),
        );
        refusal(
            "proof signature does not verify".to_string(),
            Some(Value::Object(details)),
        )
    })
}

struct ChunkRangeInfo {
    start: u64,
    end: u64,
    chunk_indices: Vec<usize>,
    payload_len: u64,
}

impl ChunkRangeInfo {
    fn chunk_count(&self) -> usize {
        self.chunk_indices.len()
    }
}

#[derive(Debug, Clone)]
struct StreamTokenPayload {
    token_id: String,
    manifest_cid_hex: String,
    provider_id_hex: String,
    profile_handle: String,
    max_streams: u16,
    ttl_epoch: u64,
    rate_limit_bytes: u64,
    issued_at: u64,
    requests_per_minute: u32,
    client_id: String,
}

#[derive(Debug)]
struct TokenRecord {
    payload: StreamTokenPayload,
    expires_at: SystemTime,
    active_streams: u32,
}

const TOKEN_QUOTA_WINDOW: Duration = Duration::from_secs(60);

/// Token issuance defaults exposed by the gateway.
#[derive(Debug, Clone)]
pub struct TokenPolicy {
    /// Default lifetime applied to issued tokens (seconds).
    pub ttl_secs: u64,
    /// Maximum concurrent range requests allowed per token (`0` => unlimited).
    pub max_streams: u16,
    /// Maximum payload length permitted per request (`0` => unlimited).
    pub rate_limit_bytes: u64,
    /// Maximum number of token issuances per minute (`None` or `Some(0)` => unlimited).
    pub requests_per_minute: Option<u32>,
}

impl Default for TokenPolicy {
    fn default() -> Self {
        Self {
            ttl_secs: 900,
            max_streams: 4,
            rate_limit_bytes: 8 * 1024 * 1024,
            requests_per_minute: Some(120),
        }
    }
}

impl TokenPolicy {
    fn issuance_limit(&self) -> Option<u32> {
        self.requests_per_minute
            .and_then(|limit| if limit == 0 { None } else { Some(limit) })
    }
}

struct TokenIssue {
    payload: StreamTokenPayload,
    encoded: String,
    ttl_epoch: u64,
    remaining_quota: Option<u32>,
}

enum TokenIssueError {
    ClientQuotaExceeded { limit: u32, retry_after_secs: u64 },
    Internal(eyre::Report),
}

impl From<eyre::Report> for TokenIssueError {
    fn from(err: eyre::Report) -> Self {
        Self::Internal(err)
    }
}

#[derive(Debug, Clone)]
struct ClientQuota {
    window_start: Instant,
    limit: u32,
    used: u32,
}

struct TokenAcquisition {
    token_id: String,
    payload: StreamTokenPayload,
}

#[derive(Debug)]
struct TokenRegistry {
    counter: AtomicU64,
    tokens: Mutex<HashMap<String, TokenRecord>>,
    client_quotas: Mutex<HashMap<String, ClientQuota>>,
    policy: TokenPolicy,
}

impl TokenRegistry {
    fn new(policy: TokenPolicy) -> Self {
        Self {
            counter: AtomicU64::new(1),
            tokens: Mutex::new(HashMap::new()),
            client_quotas: Mutex::new(HashMap::new()),
            policy,
        }
    }

    fn purge_expired_entries(&self, now_system: SystemTime, now_instant: Instant) {
        if let Ok(mut tokens) = self.tokens.lock() {
            tokens.retain(|_, record| record.expires_at > now_system);
        }
        if let Ok(mut quotas) = self.client_quotas.lock() {
            quotas.retain(|_, quota| {
                now_instant
                    .checked_duration_since(quota.window_start)
                    .is_none_or(|elapsed| elapsed < TOKEN_QUOTA_WINDOW)
            });
        }
    }

    fn issue_token(
        &self,
        dataset: &GatewayDataset,
        client_id: &str,
    ) -> Result<TokenIssue, TokenIssueError> {
        let wall_clock_now = SystemTime::now();
        self.purge_expired_entries(wall_clock_now, Instant::now());

        let token_id = format!("tok-{:016x}", self.counter.fetch_add(1, Ordering::Relaxed));
        let issued_at = wall_clock_now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| TokenIssueError::Internal(err.into()))?
            .as_secs();
        let ttl_epoch = issued_at
            .checked_add(self.policy.ttl_secs)
            .ok_or_else(|| TokenIssueError::Internal(eyre::eyre!("token TTL overflow")))?;

        let limit = self.policy.issuance_limit();
        let remaining_quota = if let Some(limit) = limit {
            let mut quotas = self
                .client_quotas
                .lock()
                .map_err(|err| TokenIssueError::Internal(eyre::eyre!(err.to_string())))?;
            let now = Instant::now();
            match quotas.entry(client_id.to_owned()) {
                Entry::Occupied(mut entry) => {
                    let quota = entry.get_mut();
                    let elapsed = now.duration_since(quota.window_start);
                    if elapsed >= TOKEN_QUOTA_WINDOW || quota.limit != limit {
                        quota.window_start = now;
                        quota.limit = limit;
                        quota.used = 0;
                    }
                    if quota.used >= quota.limit {
                        let retry_after_secs = TOKEN_QUOTA_WINDOW
                            .saturating_sub(elapsed.min(TOKEN_QUOTA_WINDOW))
                            .as_secs()
                            .max(1);
                        return Err(TokenIssueError::ClientQuotaExceeded {
                            limit,
                            retry_after_secs,
                        });
                    }
                    quota.used += 1;
                    Some(quota.limit.saturating_sub(quota.used))
                }
                Entry::Vacant(entry) => {
                    entry.insert(ClientQuota {
                        window_start: now,
                        limit,
                        used: 1,
                    });
                    Some(limit.saturating_sub(1))
                }
            }
        } else {
            // Unlimited quota.
            self.client_quotas
                .lock()
                .map_err(|err| TokenIssueError::Internal(eyre::eyre!(err.to_string())))?
                .remove(client_id);
            None
        };

        let payload = StreamTokenPayload {
            token_id: token_id.clone(),
            manifest_cid_hex: dataset.manifest_id_hex().to_string(),
            provider_id_hex: dataset.provider_id_hex(),
            profile_handle: dataset.chunker_alias().to_string(),
            max_streams: self.policy.max_streams,
            ttl_epoch,
            rate_limit_bytes: self.policy.rate_limit_bytes,
            issued_at,
            requests_per_minute: limit.unwrap_or(0),
            client_id: client_id.to_string(),
        };

        let encoded = encode_stream_token(&payload)
            .map_err(|err| TokenIssueError::Internal(eyre::eyre!(err.to_string())))?;
        let record = TokenRecord {
            payload: payload.clone(),
            expires_at: SystemTime::UNIX_EPOCH + Duration::from_secs(ttl_epoch),
            active_streams: 0,
        };
        let mut guard = self
            .tokens
            .lock()
            .map_err(|err| TokenIssueError::Internal(eyre::eyre!(err.to_string())))?;
        guard.insert(token_id, record);
        Ok(TokenIssue {
            payload,
            encoded,
            ttl_epoch,
            remaining_quota,
        })
    }

    fn acquire(
        &self,
        header: &HeaderValue,
        dataset: &GatewayDataset,
        request_client_id: Option<&str>,
    ) -> Result<TokenAcquisition, GatewayResponseError> {
        self.purge_expired_entries(SystemTime::now(), Instant::now());

        let payload = decode_stream_token_header(header)?;
        if !equal_hex(&payload.manifest_cid_hex, dataset.manifest_id_hex()) {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "admission_mismatch",
                "stream token manifest does not match gateway dataset",
            ));
        }
        if !payload
            .profile_handle
            .eq_ignore_ascii_case(dataset.chunker_alias())
        {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "unsupported_chunker",
                "stream token profile does not match gateway dataset",
            ));
        }
        let mut guard = self
            .tokens
            .lock()
            .map_err(|err| GatewayResponseError::internal(eyre::eyre!(err.to_string())))?;
        let record = guard.get_mut(&payload.token_id).ok_or_else(|| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "admission_mismatch",
                "stream token is not recognised by this gateway",
            )
        })?;
        if SystemTime::now() >= record.expires_at {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "stream_token_expired",
                "stream token has expired",
            ));
        }
        match request_client_id {
            Some(id) if id == record.payload.client_id => {}
            Some(_) => {
                return Err(GatewayResponseError::capability_refusal(
                    StatusCode::PRECONDITION_FAILED,
                    "client_mismatch",
                    "stream token was issued to a different client",
                ));
            }
            None => {
                return Err(GatewayResponseError::capability_refusal(
                    StatusCode::PRECONDITION_REQUIRED,
                    "missing_header",
                    "range requests must include X-SoraFS-Client header",
                ));
            }
        }
        if record.payload.max_streams != 0
            && record.active_streams >= u32::from(record.payload.max_streams)
        {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::TOO_MANY_REQUESTS,
                "stream_token_exhausted",
                "stream token has exhausted its concurrency budget",
            ));
        }
        record.active_streams += 1;
        Ok(TokenAcquisition {
            token_id: payload.token_id,
            payload: record.payload.clone(),
        })
    }

    fn release(&self, token_id: &str) {
        self.purge_expired_entries(SystemTime::now(), Instant::now());
        if let Ok(mut guard) = self.tokens.lock()
            && let Some(record) = guard.get_mut(token_id)
        {
            record.active_streams = record.active_streams.saturating_sub(1);
        }
    }
}

#[derive(Clone)]
struct StreamTokenLease {
    token_id: String,
    payload: StreamTokenPayload,
    registry: Arc<TokenRegistry>,
}

impl Drop for StreamTokenLease {
    fn drop(&mut self) {
        self.registry.release(&self.token_id);
    }
}

impl StreamTokenLease {
    fn payload(&self) -> &StreamTokenPayload {
        &self.payload
    }
}

fn encode_stream_token(payload: &StreamTokenPayload) -> Result<String, GatewayResponseError> {
    let value = payload_to_value(payload);
    let bytes =
        norito::json::to_vec(&value).map_err(|err| GatewayResponseError::internal(err.into()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn decode_stream_token_header(
    header: &HeaderValue,
) -> Result<StreamTokenPayload, GatewayResponseError> {
    let raw = header
        .to_str()
        .map_err(|_| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "unsupported_encoding",
                "stream token must be valid ASCII",
            )
        })?
        .trim();
    if raw.is_empty() {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "missing_header",
            "stream token header must not be empty",
        ));
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw.as_bytes())
        .map_err(|_| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "unsupported_encoding",
                "stream token header is not valid base64",
            )
        })?;

    let value: Value = norito::json::from_slice(&decoded).map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_encoding",
            "stream token payload is not valid Norito JSON",
        )
    })?;
    parse_stream_token_value(value)
}

fn payload_to_value(payload: &StreamTokenPayload) -> Value {
    let mut map = json::Map::new();
    map.insert("token_id".into(), Value::from(payload.token_id.clone()));
    map.insert(
        "manifest_cid_hex".into(),
        Value::from(payload.manifest_cid_hex.clone()),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(payload.provider_id_hex.clone()),
    );
    map.insert(
        "profile_handle".into(),
        Value::from(payload.profile_handle.clone()),
    );
    map.insert(
        "max_streams".into(),
        Value::from(u64::from(payload.max_streams)),
    );
    map.insert("ttl_epoch".into(), Value::from(payload.ttl_epoch));
    map.insert(
        "rate_limit_bytes".into(),
        Value::from(payload.rate_limit_bytes),
    );
    map.insert("issued_at".into(), Value::from(payload.issued_at));
    map.insert(
        "requests_per_minute".into(),
        Value::from(payload.requests_per_minute),
    );
    map.insert("client_id".into(), Value::from(payload.client_id.clone()));
    Value::Object(map)
}

fn parse_stream_token_value(value: Value) -> Result<StreamTokenPayload, GatewayResponseError> {
    let map = value.as_object().ok_or_else(|| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_encoding",
            "stream token payload must be an object",
        )
    })?;

    let token_id = parse_required_string(map, "token_id")?;
    let manifest_cid_hex = parse_required_string(map, "manifest_cid_hex")?;
    let provider_id_hex = parse_required_string(map, "provider_id_hex")?;
    let profile_handle = parse_required_string(map, "profile_handle")?;

    let max_streams_raw = parse_required_u64(map, "max_streams")?;
    let max_streams = u16::try_from(max_streams_raw).map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "admission_mismatch",
            "stream token max_streams exceeds supported range",
        )
    })?;
    let ttl_epoch = parse_required_u64(map, "ttl_epoch")?;
    let rate_limit_bytes = parse_required_u64(map, "rate_limit_bytes")?;
    let issued_at = parse_required_u64(map, "issued_at")?;
    let requests_per_minute_raw = parse_required_u64(map, "requests_per_minute")?;
    let requests_per_minute = u32::try_from(requests_per_minute_raw).map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "admission_mismatch",
            "stream token requests_per_minute exceeds supported range",
        )
    })?;
    let client_id = parse_required_string(map, "client_id")?;

    Ok(StreamTokenPayload {
        token_id,
        manifest_cid_hex,
        provider_id_hex,
        profile_handle,
        max_streams,
        ttl_epoch,
        rate_limit_bytes,
        issued_at,
        requests_per_minute,
        client_id,
    })
}

#[derive(Copy, Clone)]
enum GatewayEndpoint {
    Car,
    Proof,
    Token,
}

impl GatewayEndpoint {
    fn as_label(self) -> &'static str {
        match self {
            Self::Car => "car",
            Self::Proof => "proof",
            Self::Token => "token",
        }
    }
}

#[derive(Clone)]
struct GatewayTelemetry {
    otel: Arc<SorafsGatewayOtel>,
}

impl GatewayTelemetry {
    fn request_context<'a>(
        &'a self,
        endpoint: GatewayEndpoint,
        method: &'static str,
        chunker: Option<&'a str>,
        profile: Option<&'a str>,
    ) -> GatewayRequestContext<'a> {
        GatewayRequestContext::new(&self.otel, endpoint.as_label(), method, chunker, profile)
    }

    fn record_proof_result(
        &self,
        profile_version: &str,
        success: bool,
        error_code: Option<&str>,
        duration_ms: f64,
    ) {
        self.otel.record_proof_verification(
            profile_version,
            if success { "success" } else { "failure" },
            error_code,
            duration_ms,
        );
    }
}

impl Default for GatewayTelemetry {
    fn default() -> Self {
        Self {
            otel: global_sorafs_gateway_otel(),
        }
    }
}

struct GatewayRequestContext<'a> {
    otel: &'a SorafsGatewayOtel,
    endpoint: &'static str,
    method: &'static str,
    chunker: Option<&'a str>,
    profile: Option<&'a str>,
    variant: Option<&'static str>,
    start: Instant,
    finished: bool,
}

impl<'a> GatewayRequestContext<'a> {
    fn new(
        otel: &'a SorafsGatewayOtel,
        endpoint: &'static str,
        method: &'static str,
        chunker: Option<&'a str>,
        profile: Option<&'a str>,
    ) -> Self {
        otel.request_started_detailed(endpoint, method, None, chunker, profile);
        Self {
            otel,
            endpoint,
            method,
            chunker,
            profile,
            variant: None,
            start: Instant::now(),
            finished: false,
        }
    }

    fn mark_variant(&mut self, variant: &'static str) {
        self.variant = Some(variant);
    }

    fn finish_success(&mut self, status: StatusCode) {
        self.finish_with("success", status, None);
    }

    fn finish_error(&mut self, status: StatusCode, error_code: &str) {
        self.finish_with("error", status, Some(error_code));
    }

    fn finish_with(&mut self, outcome: &'static str, status: StatusCode, reason: Option<&str>) {
        if self.finished {
            return;
        }
        let status_u16 = status.as_u16();
        let latency_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        self.otel.request_completed_detailed(
            self.endpoint,
            self.method,
            self.variant,
            self.chunker,
            self.profile,
            None,
            None,
            outcome,
            status_u16,
            reason,
        );
        self.otel.record_ttfb_detailed(
            self.endpoint,
            self.method,
            self.variant,
            self.chunker,
            self.profile,
            None,
            None,
            outcome,
            status_u16,
            reason,
            latency_ms,
        );
        match outcome {
            "success" => {
                info!(
                    target: "telemetry::sorafs.gateway.request",
                    endpoint = self.endpoint,
                    method = self.method,
                    variant = self.variant.unwrap_or("unspecified"),
                    status = status_u16,
                    duration_ms = latency_ms,
                    result = outcome,
                );
            }
            _ => {
                let error_code = reason.unwrap_or("unknown");
                warn!(
                    target: "telemetry::sorafs.gateway.request",
                    endpoint = self.endpoint,
                    method = self.method,
                    variant = self.variant.unwrap_or("unspecified"),
                    status = status_u16,
                    duration_ms = latency_ms,
                    result = outcome,
                    error_code = error_code,
                );
            }
        }
        self.finished = true;
    }
}

impl<'a> Drop for GatewayRequestContext<'a> {
    fn drop(&mut self) {
        if !self.finished {
            self.finish_with(
                "dropped",
                StatusCode::INTERNAL_SERVER_ERROR,
                Some("dropped"),
            );
        }
    }
}

fn parse_required_string(map: &json::Map, key: &str) -> Result<String, GatewayResponseError> {
    map.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "admission_mismatch",
                format!("stream token missing `{key}`"),
            )
        })
}

fn parse_required_u64(map: &json::Map, key: &str) -> Result<u64, GatewayResponseError> {
    map.get(key).and_then(Value::as_u64).ok_or_else(|| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "admission_mismatch",
            format!("stream token missing `{key}`"),
        )
    })
}

/// Convenience wrapper exposed via axum state.
#[derive(Clone)]
pub struct GatewayState {
    dataset: Arc<GatewayDataset>,
    tokens: Arc<TokenRegistry>,
    telemetry: GatewayTelemetry,
}

impl GatewayState {
    /// Construct a shared state handle.
    #[must_use]
    pub fn new(dataset: GatewayDataset) -> Self {
        Self::with_token_policy(dataset, TokenPolicy::default())
    }

    /// Construct a shared state handle with the provided token policy.
    #[must_use]
    pub fn with_token_policy(dataset: GatewayDataset, policy: TokenPolicy) -> Self {
        Self {
            tokens: Arc::new(TokenRegistry::new(policy)),
            dataset: Arc::new(dataset),
            telemetry: GatewayTelemetry::default(),
        }
    }

    fn dataset(&self) -> &GatewayDataset {
        &self.dataset
    }

    fn tokens(&self) -> &Arc<TokenRegistry> {
        &self.tokens
    }

    fn telemetry(&self) -> &GatewayTelemetry {
        &self.telemetry
    }

    fn acquire_stream_token(
        &self,
        header: &HeaderValue,
        client_id: Option<&str>,
    ) -> Result<StreamTokenLease, GatewayResponseError> {
        let acquisition = self.tokens.acquire(header, self.dataset(), client_id)?;
        Ok(StreamTokenLease {
            token_id: acquisition.token_id,
            payload: acquisition.payload,
            registry: Arc::clone(&self.tokens),
        })
    }
}

/// Build an axum router that serves the trustless profile endpoints.
pub fn router(state: GatewayState) -> Router {
    Router::new()
        .route("/car/{manifest_id}", get(get_car).head(head_car))
        .route("/proof/{manifest_id}", get(get_proof))
        .route("/token", post(post_token))
        .with_state(state)
}

async fn get_car(
    State(state): State<GatewayState>,
    AxumPath(manifest_id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Response, GatewayResponseError> {
    let (chunker_label, profile_label) = {
        let dataset = state.dataset();
        (
            dataset.chunker_alias().to_owned(),
            dataset.profile_version().to_owned(),
        )
    };
    let telemetry = state.telemetry().clone();
    let mut request_ctx = telemetry.request_context(
        GatewayEndpoint::Car,
        "GET",
        Some(chunker_label.as_str()),
        Some(profile_label.as_str()),
    );
    let result = get_car_inner(&state, &manifest_id, &headers, &mut request_ctx);
    match &result {
        Ok(response) => request_ctx.finish_success(response.status()),
        Err(err) => request_ctx.finish_error(err.status_code(), err.error_code()),
    }
    result
}

async fn head_car(
    State(state): State<GatewayState>,
    AxumPath(manifest_id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Response, GatewayResponseError> {
    let (chunker_label, profile_label) = {
        let dataset = state.dataset();
        (
            dataset.chunker_alias().to_owned(),
            dataset.profile_version().to_owned(),
        )
    };
    let telemetry = state.telemetry().clone();
    let mut request_ctx = telemetry.request_context(
        GatewayEndpoint::Car,
        "HEAD",
        Some(chunker_label.as_str()),
        Some(profile_label.as_str()),
    );
    request_ctx.mark_variant("head");
    let result = head_car_inner(&state, &manifest_id, &headers);
    match &result {
        Ok(response) => request_ctx.finish_success(response.status()),
        Err(err) => request_ctx.finish_error(err.status_code(), err.error_code()),
    }
    result
}

async fn get_proof(
    State(state): State<GatewayState>,
    AxumPath(manifest_id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Response, GatewayResponseError> {
    let (chunker_label, profile_label) = {
        let dataset = state.dataset();
        (
            dataset.chunker_alias().to_owned(),
            dataset.profile_version().to_owned(),
        )
    };
    let telemetry = state.telemetry().clone();
    let mut request_ctx = telemetry.request_context(
        GatewayEndpoint::Proof,
        "GET",
        Some(chunker_label.as_str()),
        Some(profile_label.as_str()),
    );
    request_ctx.mark_variant("proof");
    let result = get_proof_inner(&state, &manifest_id, &headers, &telemetry);
    match &result {
        Ok(response) => request_ctx.finish_success(response.status()),
        Err(err) => request_ctx.finish_error(err.status_code(), err.error_code()),
    }
    result
}

async fn post_token(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, GatewayResponseError> {
    let (chunker_label, profile_label) = {
        let dataset = state.dataset();
        (
            dataset.chunker_alias().to_owned(),
            dataset.profile_version().to_owned(),
        )
    };
    let telemetry = state.telemetry().clone();
    let mut request_ctx = telemetry.request_context(
        GatewayEndpoint::Token,
        "POST",
        Some(chunker_label.as_str()),
        Some(profile_label.as_str()),
    );
    request_ctx.mark_variant("issue");
    let result = post_token_inner(&state, &headers, &body);
    match &result {
        Ok(response) => request_ctx.finish_success(response.status()),
        Err(err) => request_ctx.finish_error(err.status_code(), err.error_code()),
    }
    result
}

fn get_car_inner(
    state: &GatewayState,
    manifest_id: &str,
    headers: &HeaderMap,
    telemetry: &mut GatewayRequestContext<'_>,
) -> Result<Response, GatewayResponseError> {
    let dataset = state.dataset();
    ensure_manifest_id(dataset, manifest_id)?;
    let request = ParsedRequest::from_headers(dataset, headers)?;

    if let Some(range_header) = headers.get(RANGE) {
        telemetry.mark_variant("range");
        ensure_accepts_block(headers)?;
        ensure_chunker_header(dataset, headers)?;
        if let Some(value) = headers.get(ACCEPT_ENCODING)
            && let Ok(raw) = value.to_str()
            && raw
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("gzip"))
        {
            let mut details = json::Map::new();
            details.insert("encoding".into(), Value::from("gzip"));
            return Err(GatewayResponseError::capability_refusal_with_details(
                StatusCode::NOT_ACCEPTABLE,
                "unsupported_encoding",
                format!(
                    "gzip compression is not allowed for {}",
                    dataset.chunker_alias()
                ),
                Some(Value::Object(details)),
            ));
        }
        let client_id = request.client_id().ok_or_else(|| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_REQUIRED,
                "missing_header",
                "range requests require X-SoraFS-Client header",
            )
        })?;
        let stream_token_header = headers.get(HEADER_STREAM_TOKEN).ok_or_else(|| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_REQUIRED,
                "missing_header",
                "range requests require X-SoraFS-Stream-Token header",
            )
        })?;
        let token_guard = state.acquire_stream_token(stream_token_header, Some(client_id))?;
        let range = parse_payload_range(range_header, dataset.manifest().content_length)?;
        let chunk_range = dataset.chunk_range(range.clone())?;
        if let Some(alias) = request.alias() {
            let alias_matches = dataset
                .manifest()
                .chunking
                .aliases
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(alias));
            if !alias_matches {
                drop(token_guard);
                let mut details = json::Map::new();
                details.insert("alias".into(), Value::from(alias.to_string()));
                return Err(GatewayResponseError::capability_refusal_with_details(
                    StatusCode::PRECONDITION_FAILED,
                    "manifest_variant_missing",
                    "requested manifest alias is not bound in the governance envelope",
                    Some(Value::Object(details)),
                ));
            }
        }
        let token_rate_limit = token_guard.payload().rate_limit_bytes;
        if token_rate_limit != 0 && chunk_range.payload_len > token_rate_limit {
            drop(token_guard);
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::TOO_MANY_REQUESTS,
                "stream_token_rate_limit",
                "requested range exceeds stream token byte budget",
            ));
        }
        let car_bytes = dataset.build_block_car(&chunk_range)?;
        let mut response = Response::new(Body::from(car_bytes.clone()));
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
        let headers_mut = response.headers_mut();
        headers_mut.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/vnd.ipld.car"),
        );
        headers_mut.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&car_bytes.len().to_string())
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
        headers_mut.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        let total_length = dataset.manifest().content_length;
        let content_range_value =
            format!("bytes {}-{}/{}", range.start(), range.end(), total_length);
        headers_mut.insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&content_range_value)
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
        let chunk_range_value = format!(
            "start={};end={};chunks={}",
            chunk_range.start,
            chunk_range.end,
            chunk_range.chunk_count(),
        );
        headers_mut.insert(
            HeaderName::from_static(HEADER_CHUNK_RANGE),
            HeaderValue::from_str(&chunk_range_value)
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
        headers_mut.insert(
            HeaderName::from_static(HEADER_CLIENT_ID),
            HeaderValue::from_str(client_id)
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
        populate_success_headers(&mut response, dataset, &request)?;
        response.extensions_mut().insert(token_guard);
        return Ok(response);
    }

    telemetry.mark_variant("full");
    let mut response = Response::new(Body::from((*dataset.car_bytes).clone()));
    let headers_mut = response.headers_mut();
    headers_mut.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.ipld.car"),
    );
    headers_mut.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&dataset.car_bytes.len().to_string())
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers_mut.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    populate_success_headers(&mut response, dataset, &request)?;
    Ok(response)
}

fn head_car_inner(
    state: &GatewayState,
    manifest_id: &str,
    headers: &HeaderMap,
) -> Result<Response, GatewayResponseError> {
    let dataset = state.dataset();
    ensure_manifest_id(dataset, manifest_id)?;
    let request = ParsedRequest::from_headers(dataset, headers)?;
    let mut response = Response::new(Body::empty());
    let headers_mut = response.headers_mut();
    headers_mut.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.ipld.car"),
    );
    headers_mut.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&dataset.car_bytes.len().to_string())
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers_mut.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    populate_success_headers(&mut response, dataset, &request)?;
    Ok(response)
}

fn get_proof_inner(
    state: &GatewayState,
    manifest_id: &str,
    headers: &HeaderMap,
    telemetry: &GatewayTelemetry,
) -> Result<Response, GatewayResponseError> {
    let dataset = state.dataset();
    ensure_manifest_id(dataset, manifest_id)?;
    let request = ParsedRequest::from_headers(dataset, headers)?;
    let verification_start = Instant::now();
    if let Err(err) = dataset.verify_proof() {
        let duration_ms = verification_start.elapsed().as_secs_f64() * 1000.0;
        telemetry.record_proof_result(
            dataset.profile_version(),
            false,
            Some(err.error_code()),
            duration_ms,
        );
        return Err(err);
    }
    let duration_ms = verification_start.elapsed().as_secs_f64() * 1000.0;
    telemetry.record_proof_result(dataset.profile_version(), true, None, duration_ms);
    let proof_json = json::to_vec(dataset.proof())
        .wrap_err("failed to serialize proof payload")
        .map_err(GatewayResponseError::internal)?;
    let proof_len = proof_json.len();
    let mut response = Response::new(Body::from(proof_json));
    let headers_mut = response.headers_mut();
    headers_mut.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers_mut.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&proof_len.to_string())
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    populate_success_headers(&mut response, dataset, &request)?;
    Ok(response)
}

fn post_token_inner(
    state: &GatewayState,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<Response, GatewayResponseError> {
    let dataset = state.dataset();
    let client_header = headers.get(HEADER_CLIENT_ID).ok_or_else(|| {
        GatewayResponseError::capability_refusal(
            StatusCode::BAD_REQUEST,
            "missing_client_id",
            "token requests require X-SoraFS-Client header",
        )
    })?;
    let client_id = client_header
        .to_str()
        .map_err(|_| {
            GatewayResponseError::capability_refusal(
                StatusCode::BAD_REQUEST,
                "invalid_client_id",
                "X-SoraFS-Client header must contain valid ASCII",
            )
        })?
        .trim();
    if client_id.is_empty() {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::BAD_REQUEST,
            "invalid_client_id",
            "X-SoraFS-Client header must not be empty",
        ));
    }

    let payload: Value = norito::json::from_slice(body).map_err(|_| {
        GatewayResponseError::ManifestEnvelope(
            "token requests must provide a valid JSON payload".to_string(),
        )
    })?;
    let manifest_envelope = payload
        .get("manifest_envelope")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GatewayResponseError::ManifestEnvelope(
                "token requests must supply `manifest_envelope` field".to_string(),
            )
        })?;
    let envelope_header = HeaderValue::from_str(manifest_envelope).map_err(|_| {
        GatewayResponseError::ManifestEnvelope("manifest envelope must be valid ASCII".into())
    })?;
    validate_manifest_envelope(dataset, &envelope_header)?;

    let capabilities: Vec<String> = payload
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(Value::as_str)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .collect()
        })
        .unwrap_or_default();
    for capability in &capabilities {
        let supported = SUPPORTED_CAPABILITIES
            .iter()
            .any(|allowed| capability.eq_ignore_ascii_case(allowed));
        if !supported {
            let mut details = json::Map::new();
            details.insert("capability".into(), Value::from(capability.clone()));
            return Err(GatewayResponseError::capability_refusal_with_details(
                StatusCode::PRECONDITION_REQUIRED,
                "unsupported_capability",
                format!("capability tlv {capability} is not permitted for this profile"),
                Some(Value::Object(details)),
            ));
        }
    }

    let issue = match state.tokens().issue_token(dataset, client_id) {
        Ok(issue) => issue,
        Err(TokenIssueError::ClientQuotaExceeded {
            limit,
            retry_after_secs,
        }) => {
            let message =
                format!("stream token issuance quota exceeded (limit {limit} requests per minute)");
            let mut response =
                json_error_response(StatusCode::TOO_MANY_REQUESTS, "quota_exceeded", message);
            let headers_mut = response.headers_mut();
            headers_mut.insert(
                RETRY_AFTER,
                HeaderValue::from_str(&retry_after_secs.to_string())
                    .map_err(|err| GatewayResponseError::internal(err.into()))?,
            );
            headers_mut.insert(
                HeaderName::from_static(HEADER_CLIENT_ID),
                HeaderValue::from_str(client_id)
                    .map_err(|err| GatewayResponseError::internal(err.into()))?,
            );
            headers_mut.insert(
                HeaderName::from_static(HEADER_CLIENT_QUOTA_REMAINING),
                HeaderValue::from_static("0"),
            );
            headers_mut.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
            return Ok(response);
        }
        Err(TokenIssueError::Internal(err)) => {
            return Err(GatewayResponseError::internal(err));
        }
    };

    let mut response_map = json::Map::new();
    response_map.insert("token".into(), payload_to_value(&issue.payload));
    response_map.insert("expires_at".into(), Value::from(issue.ttl_epoch));
    response_map.insert("signature".into(), Value::from("00".repeat(64)));

    let mut response = Response::new(Body::from(
        norito::json::to_vec(&Value::Object(response_map))
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    ));
    let headers_mut = response.headers_mut();
    headers_mut.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers_mut.insert(
        HeaderName::from_static("x-sorafs-token-id"),
        HeaderValue::from_str(&issue.payload.token_id)
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers_mut.insert(
        HeaderName::from_static(HEADER_STREAM_TOKEN),
        HeaderValue::from_str(&issue.encoded)
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers_mut.insert(
        HeaderName::from_static(HEADER_CLIENT_ID),
        HeaderValue::from_str(client_id)
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    let quota_header = issue
        .remaining_quota
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unlimited".to_string());
    headers_mut.insert(
        HeaderName::from_static(HEADER_CLIENT_QUOTA_REMAINING),
        HeaderValue::from_str(&quota_header)
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers_mut.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}

fn ensure_manifest_id(
    dataset: &GatewayDataset,
    manifest_id: &str,
) -> Result<(), GatewayResponseError> {
    if dataset.manifest_id_hex.eq_ignore_ascii_case(manifest_id) {
        Ok(())
    } else {
        Err(GatewayResponseError::not_found())
    }
}

fn parse_payload_range(
    header: &HeaderValue,
    total_length: u64,
) -> Result<RangeInclusive<u64>, GatewayResponseError> {
    let value = header
        .to_str()
        .map_err(|_| GatewayResponseError::invalid_range("range header must be valid ASCII"))?;
    let value = value.trim();
    let prefix = "bytes=";
    if !value.starts_with(prefix) {
        return Err(GatewayResponseError::invalid_range(
            "range header must start with `bytes=`",
        ));
    }
    let range_values = &value[prefix.len()..];
    let mut parts = range_values.split('-');
    let start_str = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| GatewayResponseError::invalid_range("range start missing"))?;
    let end_str = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| GatewayResponseError::invalid_range("range end missing"))?;
    if parts.next().is_some() {
        return Err(GatewayResponseError::invalid_range(
            "range header must contain a single start-end pair",
        ));
    }

    let start = start_str
        .parse::<u64>()
        .map_err(|_| GatewayResponseError::invalid_range("range start must be an integer"))?;
    let end = end_str
        .parse::<u64>()
        .map_err(|_| GatewayResponseError::invalid_range("range end must be an integer"))?;

    if start > end {
        return Err(GatewayResponseError::invalid_range(
            "range start must be <= range end",
        ));
    }
    if end >= total_length {
        return Err(GatewayResponseError::invalid_range(
            "range end exceeds manifest content length",
        ));
    }

    Ok(start..=end)
}

fn ensure_accepts_block(headers: &HeaderMap) -> Result<(), GatewayResponseError> {
    let missing_details = || {
        let mut details = json::Map::new();
        details.insert("header".into(), Value::from(DAG_SCOPE_HEADER_LABEL));
        GatewayResponseError::capability_refusal_with_details(
            StatusCode::PRECONDITION_REQUIRED,
            "missing_header",
            "dag-scope header is required for trustless range requests",
            Some(Value::Object(details)),
        )
    };

    let accept = headers.get(ACCEPT).ok_or_else(missing_details)?;
    let raw_value = accept.to_str().map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_capability",
            "Accept header must be valid ASCII",
        )
    })?;
    let value = raw_value.to_ascii_lowercase();
    if value.contains("dag-scope=block") {
        Ok(())
    } else {
        let mut details = json::Map::new();
        details.insert("header".into(), Value::from(DAG_SCOPE_HEADER_LABEL));
        Err(GatewayResponseError::capability_refusal_with_details(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_capability",
            "Accept header must include dag-scope=block",
            Some(Value::Object(details)),
        ))
    }
}

fn ensure_chunker_header(
    dataset: &GatewayDataset,
    headers: &HeaderMap,
) -> Result<(), GatewayResponseError> {
    let expected = dataset.chunker_alias();
    let header = require_header(headers, HEADER_CHUNKER)?;
    let value = header.to_str().map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_capability",
            "X-SoraFS-Chunker header must be valid ASCII",
        )
    })?;
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        let mut details = json::Map::new();
        details.insert("profile".into(), Value::from(expected.to_string()));
        details.insert("requested_profile".into(), Value::from(trimmed.to_string()));
        Err(GatewayResponseError::capability_refusal_with_details(
            StatusCode::NOT_ACCEPTABLE,
            "unsupported_chunker",
            format!("chunk profile {expected} is not enabled on this gateway"),
            Some(Value::Object(details)),
        ))
    }
}

struct ParsedRequest {
    nonce: HeaderValue,
    client_id: Option<String>,
    alias: Option<String>,
    host: Option<String>,
}

impl ParsedRequest {
    fn from_headers(
        dataset: &GatewayDataset,
        headers: &HeaderMap,
    ) -> Result<Self, GatewayResponseError> {
        let version = require_header(headers, HEADER_VERSION)?;
        if !version
            .to_str()
            .map(|value| value.eq_ignore_ascii_case(&dataset.profile_version))
            .unwrap_or(false)
        {
            return Err(GatewayResponseError::header_mismatch(
                HEADER_VERSION,
                dataset.profile_version.clone(),
            ));
        }

        let nonce = require_header(headers, HEADER_NONCE)?.clone();
        let envelope = require_header(headers, HEADER_MANIFEST_ENVELOPE)?;
        validate_manifest_envelope(dataset, envelope)?;

        let client_id = headers
            .get(HEADER_CLIENT_ID)
            .map(|value| {
                value.to_str().map(|raw| {
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
            })
            .transpose()
            .map_err(|_| {
                GatewayResponseError::capability_refusal(
                    StatusCode::PRECONDITION_FAILED,
                    "invalid_client_header",
                    "X-SoraFS-Client header must contain valid ASCII",
                )
            })?
            .flatten();

        let alias = headers
            .get(HEADER_ALIAS)
            .map(|value| {
                value.to_str().map(|raw| {
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
            })
            .transpose()
            .map_err(|_| {
                GatewayResponseError::capability_refusal(
                    StatusCode::PRECONDITION_FAILED,
                    "invalid_alias_header",
                    "Sora-Name header must contain valid ASCII",
                )
            })?
            .flatten();

        let host = headers
            .get(HOST)
            .map(|value| {
                value
                    .to_str()
                    .map(|raw| {
                        let trimmed = raw.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    })
                    .map_err(|_| {
                        GatewayResponseError::capability_refusal(
                            StatusCode::PRECONDITION_FAILED,
                            "invalid_host_header",
                            "Host header must contain valid ASCII",
                        )
                    })
            })
            .transpose()?
            .flatten();

        Ok(Self {
            nonce,
            client_id,
            alias,
            host,
        })
    }

    fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }
}

fn populate_success_headers(
    response: &mut Response,
    dataset: &GatewayDataset,
    request: &ParsedRequest,
) -> Result<(), GatewayResponseError> {
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static(HEADER_VERSION),
        HeaderValue::from_str(dataset.profile_version())
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers.insert(HeaderName::from_static(HEADER_NONCE), request.nonce.clone());
    headers.insert(
        HeaderName::from_static(HEADER_CHUNKER),
        HeaderValue::from_str(&dataset.chunker_alias)
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers.insert(
        HeaderName::from_static(HEADER_PROOF_DIGEST),
        HeaderValue::from_str(&dataset.proof_digest_hex)
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers.insert(
        HeaderName::from_static(HEADER_POR_ROOT),
        HeaderValue::from_str(&dataset.por_root_hex)
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    headers.insert(
        HeaderName::from_static(HEADER_SORA_CONTENT_CID),
        HeaderValue::from_str(dataset.content_cid())
            .map_err(|err| GatewayResponseError::internal(err.into()))?,
    );
    if let Some(client) = request.client_id() {
        headers.insert(
            HeaderName::from_static(HEADER_CLIENT_ID),
            HeaderValue::from_str(client)
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
    }
    if let Some(alias) = request.alias() {
        headers.insert(
            HeaderName::from_static(HEADER_ALIAS),
            HeaderValue::from_str(alias)
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
        let mut proof_value = json::Map::new();
        proof_value.insert("alias".into(), Value::from(alias.to_string()));
        proof_value.insert(
            "manifest".into(),
            Value::from(dataset.content_cid().to_string()),
        );
        let proof_bytes = json::to_vec(&Value::Object(proof_value))
            .map_err(|err| GatewayResponseError::internal(err.into()))?;
        let proof_header = base64::engine::general_purpose::STANDARD.encode(proof_bytes);
        headers.insert(
            HeaderName::from_static(HEADER_SORA_PROOF),
            HeaderValue::from_str(&proof_header)
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_SORA_PROOF_STATUS),
            HeaderValue::from_static(DEFAULT_PROOF_STATUS),
        );
    }
    if let Some(host) = request.host() {
        let mut parts = vec![
            format!("host={host}"),
            format!("cid={}", dataset.content_cid()),
            format!("generated_at={}", dataset.route_generated_at()),
        ];
        if !dataset.chunker_alias().is_empty() {
            parts.push(format!("label={}", dataset.chunker_alias()));
        }
        let binding = parts.join(";");
        headers.insert(
            HeaderName::from_static(HEADER_SORA_ROUTE_BINDING),
            HeaderValue::from_str(&binding)
                .map_err(|err| GatewayResponseError::internal(err.into()))?,
        );
    }
    headers.insert(
        HeaderName::from_static(HEADER_PERMISSIONS_POLICY),
        HeaderValue::from_static(DEFAULT_PERMISSIONS_TEMPLATE),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(DEFAULT_CSP_TEMPLATE),
    );
    headers.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static(DEFAULT_HSTS_TEMPLATE),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, path::PathBuf};

    use tempfile::{NamedTempFile, TempDir};

    use super::*;
    use crate::config::StorageConfig;

    #[test]
    fn capability_refusal_status_and_code_exposed() {
        let err = GatewayResponseError::capability_refusal(
            StatusCode::BAD_REQUEST,
            "custom_code",
            "failure",
        );
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(err.error_code(), "custom_code");
    }

    #[test]
    fn missing_header_uses_predefined_code() {
        let err = GatewayResponseError::missing_header("x-test-header");
        assert_eq!(err.status_code(), StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(err.error_code(), "required_headers_missing");
    }

    fn write_signing_key_hex() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp signing key");
        let key_bytes = [0x11u8; 32];
        let hex_key = hex::encode(key_bytes);
        file.write_all(hex_key.as_bytes())
            .expect("write signing key");
        file
    }

    fn fixture_dataset() -> GatewayDataset {
        let fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");
        let manifest_bytes =
            fs::read(fixtures.join("manifest_v1.to")).expect("read manifest fixture");
        let manifest: ManifestV1 =
            norito::decode_from_bytes(&manifest_bytes).expect("decode manifest fixture");
        let payload = fs::read(fixtures.join("payload.bin")).expect("read payload fixture");
        let profile = chunk_profile_for_manifest(&manifest).expect("chunk profile");
        let plan = CarBuildPlan::single_file_with_profile(&payload, profile).expect("build plan");

        let temp_dir = TempDir::new().expect("temp storage");
        let signing_key = write_signing_key_hex();
        let config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .stream_token_signing_key_path(Some(signing_key.path().to_path_buf()))
            .build();
        let node = NodeHandle::new(config);
        let mut reader = payload.as_slice();
        node.ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest = manifest.digest().expect("manifest digest");
        let manifest_digest_hex = hex::encode(manifest_digest.as_bytes());
        let provider_id = [0xAB; 32];
        GatewayDataset::load_from_storage_with_provider(&node, &manifest_digest_hex, provider_id)
            .expect("load storage-backed dataset")
    }

    #[test]
    fn sora_headers_populated_from_dataset() {
        let dataset = fixture_dataset();
        let mut response = Response::new(Body::empty());
        let request = ParsedRequest {
            nonce: HeaderValue::from_static("nonce"),
            client_id: Some("client-alpha".to_string()),
            alias: Some("docs.sora".to_string()),
            host: Some("docs.sora.link".to_string()),
        };
        populate_success_headers(&mut response, &dataset, &request).expect("headers populated");
        let headers = response.headers();

        let cid = headers
            .get(HEADER_SORA_CONTENT_CID)
            .expect("sora content cid present")
            .to_str()
            .expect("cid ascii");
        assert_eq!(cid, dataset.content_cid());

        let proof_header = headers
            .get(HEADER_SORA_PROOF)
            .expect("proof header present")
            .to_str()
            .unwrap();
        let proof_bytes = base64::engine::general_purpose::STANDARD
            .decode(proof_header.as_bytes())
            .expect("decode proof header");
        let proof_json: Value = json::from_slice(&proof_bytes).expect("parse proof payload");
        assert_eq!(proof_json["alias"].as_str(), Some("docs.sora"));
        assert_eq!(proof_json["manifest"].as_str(), Some(dataset.content_cid()));
        assert_eq!(
            headers
                .get(HEADER_SORA_PROOF_STATUS)
                .expect("status header present")
                .to_str()
                .unwrap(),
            DEFAULT_PROOF_STATUS
        );

        let binding = headers
            .get(HEADER_SORA_ROUTE_BINDING)
            .expect("route binding header present")
            .to_str()
            .unwrap();
        assert!(binding.contains("host=docs.sora.link"));
        assert!(binding.contains(&format!("cid={}", dataset.content_cid())));
        assert!(binding.contains("generated_at="));
        assert!(binding.contains("label="));

        assert_eq!(
            headers
                .get("content-security-policy")
                .expect("csp header present")
                .to_str()
                .unwrap(),
            DEFAULT_CSP_TEMPLATE
        );
        assert_eq!(
            headers
                .get("strict-transport-security")
                .expect("hsts header present")
                .to_str()
                .unwrap(),
            DEFAULT_HSTS_TEMPLATE
        );
        assert_eq!(
            headers
                .get(HEADER_PERMISSIONS_POLICY)
                .expect("permissions policy header present")
                .to_str()
                .unwrap(),
            DEFAULT_PERMISSIONS_TEMPLATE
        );
    }

    #[test]
    fn signing_key_loader_accepts_hex() {
        let signing_key = write_signing_key_hex();
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(signing_key.path().to_path_buf()))
            .build();
        let key = load_gateway_signing_key(&config).expect("load signing key");
        let keypair = KeyPair::from_private_key(key).expect("derive keypair");
        let signature = Signature::new(keypair.private_key(), b"sorafs-proof");
        signature
            .verify(keypair.public_key(), b"sorafs-proof")
            .expect("signature should verify");
    }

    #[test]
    fn por_proof_signature_rejects_tampering() {
        let fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");
        let manifest_bytes =
            fs::read(fixtures.join("manifest_v1.to")).expect("read manifest fixture");
        let manifest: ManifestV1 =
            norito::decode_from_bytes(&manifest_bytes).expect("decode manifest fixture");
        let payload = fs::read(fixtures.join("payload.bin")).expect("read payload fixture");
        let profile = chunk_profile_for_manifest(&manifest).expect("chunk profile");
        let plan = CarBuildPlan::single_file_with_profile(&payload, profile).expect("build plan");
        let stored_chunks = plan
            .chunks
            .iter()
            .map(|chunk| sorafs_car::StoredChunk {
                offset: chunk.offset,
                length: chunk.length,
                blake3: chunk.digest,
            })
            .collect::<Vec<_>>();
        let por_tree = PorMerkleTree::from_payload(&payload, &stored_chunks);
        let manifest_digest = manifest.digest().expect("manifest digest");
        let provider_id = [0xCD; 32];
        let signing_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x22; 32]).expect("private key");

        let mut proof = build_por_proof(
            &por_tree,
            &payload,
            *manifest_digest.as_bytes(),
            provider_id,
            &signing_key,
        )
        .expect("build proof");
        verify_proof_signature(&proof).expect("proof signature valid");
        proof.signature.signature[0] ^= 0xFF;
        let err = verify_proof_signature(&proof).expect_err("tampered proof should fail");
        assert_eq!(err.error_code(), "proof_mismatch");
    }

    #[test]
    fn chunk_profile_for_manifest_rejects_out_of_order_sizes() {
        let fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");
        let manifest_bytes =
            fs::read(fixtures.join("manifest_v1.to")).expect("read manifest fixture");
        let mut manifest: ManifestV1 =
            norito::decode_from_bytes(&manifest_bytes).expect("decode manifest fixture");
        manifest.chunking.profile_id = sorafs_manifest::ProfileId(u32::MAX);
        manifest.chunking.min_size = 1024;
        manifest.chunking.target_size = 512;
        manifest.chunking.max_size = 2048;
        manifest.chunking.break_mask = 1;
        let err = chunk_profile_for_manifest(&manifest).expect_err("should reject profile");
        assert!(
            err.to_string().contains("min <= target <= max"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn load_from_storage_requires_provider_id() {
        let fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");
        let manifest_bytes =
            fs::read(fixtures.join("manifest_v1.to")).expect("read manifest fixture");
        let manifest: ManifestV1 =
            norito::decode_from_bytes(&manifest_bytes).expect("decode manifest fixture");
        let payload = fs::read(fixtures.join("payload.bin")).expect("read payload fixture");
        let profile = chunk_profile_for_manifest(&manifest).expect("chunk profile");
        let plan = CarBuildPlan::single_file_with_profile(&payload, profile).expect("build plan");

        let temp_dir = TempDir::new().expect("temp storage");
        let signing_key = write_signing_key_hex();
        let config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .stream_token_signing_key_path(Some(signing_key.path().to_path_buf()))
            .build();
        let node = NodeHandle::new(config);
        let mut reader = payload.as_slice();
        node.ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest = manifest.digest().expect("manifest digest");
        let manifest_digest_hex = hex::encode(manifest_digest.as_bytes());

        let err = GatewayDataset::load_from_storage(&node, &manifest_digest_hex)
            .expect_err("provider id should be required");
        assert!(
            err.to_string().contains("provider_id missing"),
            "unexpected error: {err}"
        );
    }
}

fn require_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<&'a HeaderValue, GatewayResponseError> {
    headers
        .get(name)
        .ok_or_else(|| GatewayResponseError::missing_header(name))
}

fn canonical_chunker_alias(manifest: &ManifestV1) -> String {
    manifest
        .chunking
        .aliases
        .first()
        .cloned()
        .unwrap_or_else(|| {
            format!(
                "{}.{}@{}",
                manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
            )
        })
}

fn validate_manifest_envelope(
    dataset: &GatewayDataset,
    header: &HeaderValue,
) -> Result<(), GatewayResponseError> {
    let raw = header
        .to_str()
        .map_err(|_| GatewayResponseError::manifest_envelope("header must be valid UTF-8"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(GatewayResponseError::manifest_envelope(
            "header must not be empty",
        ));
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(trimmed.as_bytes())
        .map_err(|err| {
            GatewayResponseError::manifest_envelope(format!("invalid base64 payload: {err}"))
        })?;
    if decoded.is_empty() {
        return Err(GatewayResponseError::manifest_envelope(
            "decoded payload must not be empty",
        ));
    }
    let envelope: Value = norito::json::from_slice(&decoded).map_err(|err| {
        GatewayResponseError::manifest_envelope(format!("payload is not valid JSON: {err}"))
    })?;
    let envelope_map = envelope
        .as_object()
        .ok_or_else(|| GatewayResponseError::manifest_envelope("payload must be a JSON object"))?;

    let manifest_digest_hex = require_envelope_string(envelope_map, "manifest_digest_hex")?;
    if !equal_hex(manifest_digest_hex, dataset.manifest_id_hex()) {
        let mut details = json::Map::new();
        details.insert(
            "manifest_digest".into(),
            Value::from(dataset.manifest_id_hex().to_string()),
        );
        return Err(GatewayResponseError::capability_refusal_with_details(
            StatusCode::PRECONDITION_FAILED,
            "admission_mismatch",
            "manifest digest is not covered by the admission envelope",
            Some(Value::Object(details)),
        ));
    }

    let chunking_profile = require_envelope_string(envelope_map, "chunking_profile")?;
    if !chunking_profile.eq_ignore_ascii_case(dataset.chunker_alias()) {
        return Err(GatewayResponseError::manifest_envelope(
            "chunking_profile does not match manifest profile",
        ));
    }

    let provider_id_hex = require_envelope_string(envelope_map, "provider_id_hex")?;
    let expected_provider_hex = dataset.provider_id_hex();
    if !equal_hex(provider_id_hex, &expected_provider_hex) {
        return Err(GatewayResponseError::manifest_envelope(
            "provider_id_hex does not match gateway provider",
        ));
    }

    let gar_map = require_envelope_object(envelope_map, "gar")?;
    let gar_manifest_hex = require_envelope_string(gar_map, "manifest_id_hex")?;
    if !equal_hex(gar_manifest_hex, dataset.manifest_id_hex()) {
        return Err(GatewayResponseError::manifest_envelope(
            "gar.manifest_id_hex does not match manifest digest",
        ));
    }
    let gar_chunker = require_envelope_string(gar_map, "chunking_profile")?;
    if !gar_chunker.eq_ignore_ascii_case(dataset.chunker_alias()) {
        return Err(GatewayResponseError::manifest_envelope(
            "gar.chunking_profile does not match manifest profile",
        ));
    }
    let host_patterns_value = gar_map
        .get("host_patterns")
        .ok_or_else(|| GatewayResponseError::manifest_envelope("gar.host_patterns missing"))?;
    let host_patterns = host_patterns_value.as_array().ok_or_else(|| {
        GatewayResponseError::manifest_envelope("gar.host_patterns must be an array")
    })?;
    if host_patterns.is_empty()
        || !host_patterns
            .iter()
            .all(|value| value.as_str().is_some_and(|entry| !entry.trim().is_empty()))
    {
        return Err(GatewayResponseError::manifest_envelope(
            "gar.host_patterns must contain non-empty strings",
        ));
    }

    let admission_map = require_envelope_object(envelope_map, "admission")?;
    let admission_manifest_hex = require_envelope_string(admission_map, "manifest_digest_hex")?;
    if !equal_hex(admission_manifest_hex, dataset.manifest_id_hex()) {
        return Err(GatewayResponseError::manifest_envelope(
            "admission.manifest_digest_hex does not match manifest digest",
        ));
    }
    let admission_provider_hex = require_envelope_string(admission_map, "provider_id_hex")?;
    if !equal_hex(admission_provider_hex, provider_id_hex) {
        return Err(GatewayResponseError::manifest_envelope(
            "admission.provider_id_hex does not match provider",
        ));
    }
    require_envelope_string(admission_map, "signature")?;

    Ok(())
}

fn equal_hex(expected: &str, actual: &str) -> bool {
    expected.trim().eq_ignore_ascii_case(actual.trim())
}

fn require_envelope_string<'a>(
    map: &'a json::Map,
    key: &str,
) -> Result<&'a str, GatewayResponseError> {
    if let Some(Value::String(raw)) = map.get(key) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    Err(GatewayResponseError::manifest_envelope(format!(
        "field `{key}` must be a non-empty string"
    )))
}

fn require_envelope_object<'a>(
    map: &'a json::Map,
    key: &str,
) -> Result<&'a json::Map, GatewayResponseError> {
    map.get(key).and_then(Value::as_object).ok_or_else(|| {
        GatewayResponseError::manifest_envelope(format!("field `{key}` must be a JSON object"))
    })
}

/// Errors returned by the HTTP handlers.
#[derive(Debug, Error)]
pub enum GatewayResponseError {
    /// Required HTTP header missing from the request.
    #[error("missing required header {0}")]
    MissingHeader(&'static str),
    /// Request header provided but failed validation.
    #[error("header {0} did not match expected value")]
    HeaderMismatch(&'static str),
    /// Manifest envelope payload rejected by the gateway.
    #[error("manifest envelope invalid: {0}")]
    ManifestEnvelope(String),
    /// Byte range supplied by the client was invalid.
    #[error("invalid byte range: {0}")]
    InvalidRange(String),
    /// Capability failure surfaced while processing the request.
    #[error("capability refusal {error}: {reason}")]
    CapabilityRefusal {
        /// HTTP status code returned to the caller.
        status: StatusCode,
        /// Machine-readable capability error code.
        error: String,
        /// Human-readable reason describing the violation.
        reason: String,
        /// Optional detail map echoing structured metadata for telemetry/SDKs.
        details: Option<Value>,
    },
    /// Manifest identifier not recognised by this gateway.
    #[error("requested manifest not found")]
    NotFound,
    /// Internal gateway failure.
    #[error("gateway internal error")]
    Internal(#[source] eyre::Report),
}

impl GatewayResponseError {
    fn missing_header(name: &'static str) -> Self {
        Self::MissingHeader(name)
    }

    fn header_mismatch(name: &'static str, _expected: String) -> Self {
        Self::HeaderMismatch(name)
    }

    fn manifest_envelope(reason: impl Into<String>) -> Self {
        Self::ManifestEnvelope(reason.into())
    }

    fn invalid_range(reason: impl Into<String>) -> Self {
        Self::InvalidRange(reason.into())
    }

    fn capability_refusal(
        status: StatusCode,
        error: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::capability_refusal_with_details(status, error, reason, None)
    }

    fn capability_refusal_with_details(
        status: StatusCode,
        error: impl Into<String>,
        reason: impl Into<String>,
        details: Option<Value>,
    ) -> Self {
        Self::CapabilityRefusal {
            status,
            error: error.into(),
            reason: reason.into(),
            details,
        }
    }

    fn not_found() -> Self {
        Self::NotFound
    }

    fn internal(err: eyre::Report) -> Self {
        Self::Internal(err)
    }

    fn status_code(&self) -> StatusCode {
        match self {
            GatewayResponseError::MissingHeader(_) => StatusCode::PRECONDITION_REQUIRED,
            GatewayResponseError::HeaderMismatch(_) => StatusCode::PRECONDITION_FAILED,
            GatewayResponseError::ManifestEnvelope(_) => StatusCode::PRECONDITION_FAILED,
            GatewayResponseError::InvalidRange(_) => StatusCode::RANGE_NOT_SATISFIABLE,
            GatewayResponseError::CapabilityRefusal { status, .. } => *status,
            GatewayResponseError::NotFound => StatusCode::NOT_FOUND,
            GatewayResponseError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_code(&self) -> &str {
        match self {
            GatewayResponseError::MissingHeader(_) => "required_headers_missing",
            GatewayResponseError::HeaderMismatch(_) => "header_mismatch",
            GatewayResponseError::ManifestEnvelope(_) => "manifest_envelope_invalid",
            GatewayResponseError::InvalidRange(_) => "range_invalid",
            GatewayResponseError::CapabilityRefusal { error, .. } => error.as_str(),
            GatewayResponseError::NotFound => "manifest_not_found",
            GatewayResponseError::Internal(_) => "internal",
        }
    }
}

impl IntoResponse for GatewayResponseError {
    fn into_response(self) -> Response {
        match self {
            GatewayResponseError::MissingHeader(name) => json_error_response(
                StatusCode::PRECONDITION_REQUIRED,
                "required_headers_missing",
                format!("header `{name}` is required"),
            ),
            GatewayResponseError::HeaderMismatch(name) => json_error_response(
                StatusCode::PRECONDITION_FAILED,
                "header_mismatch",
                format!("header `{name}` did not match the expected value"),
            ),
            GatewayResponseError::ManifestEnvelope(reason) => json_error_response(
                StatusCode::PRECONDITION_FAILED,
                "manifest_envelope_invalid",
                reason,
            ),
            GatewayResponseError::InvalidRange(reason) => {
                json_error_response(StatusCode::RANGE_NOT_SATISFIABLE, "range_invalid", reason)
            }
            GatewayResponseError::CapabilityRefusal {
                status,
                error,
                reason,
                details,
            } => json_error_response_with_details(status, &error, reason, details),
            GatewayResponseError::NotFound => json_error_response(
                StatusCode::NOT_FOUND,
                "manifest_not_found",
                "manifest is not cached by this gateway".to_string(),
            ),
            GatewayResponseError::Internal(err) => json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                err.to_string(),
            ),
        }
    }
}

fn json_error_response(status: StatusCode, code: &str, message: String) -> Response {
    json_error_response_with_details(status, code, message, None)
}

fn json_error_response_with_details(
    status: StatusCode,
    code: &str,
    message: String,
    details: Option<Value>,
) -> Response {
    let mut map = json::Map::from_iter([
        ("error".to_string(), Value::from(code.to_string())),
        ("message".to_string(), Value::from(message)),
    ]);
    if let Some(value) = details {
        map.insert("details".to_string(), value);
    }
    let value = Value::Object(map);
    let body_bytes = json::to_vec(&value).unwrap_or_else(|_| b"{\"error\":\"internal\"}".to_vec());
    let mut response = Response::new(Body::from(body_bytes));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}
