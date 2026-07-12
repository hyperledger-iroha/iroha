//! SoraFS gateway for the trustless delivery profile (SF-5).
//!
//! The gateway serves CAR/proof responses derived from the local SoraFS storage
//! backend instead of fixture bundles. It is pinned to a single manifest digest
//! at startup and uses live payload/chunk data for every request.

use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    io::{self, Read, Write},
    ops::RangeInclusive,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> std::os::raw::c_uint;
}

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path as AxumPath, State},
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
use iroha_crypto::{
    Algorithm, KeyPair, PrivateKey, PublicKey, Signature, ed25519_parse_public_key,
    ed25519_parse_signature,
};
use iroha_logger::{info, warn};
use iroha_telemetry::metrics::{SorafsGatewayOtel, global_sorafs_gateway_otel};
use norito::json::{self, Value};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
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
const GATEWAY_SIGNING_KEY_FILE_MAX_BYTES: usize = 64;
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
const STREAM_TOKEN_VERSION: u8 = 1;
const STREAM_TOKEN_SIGNATURE_DOMAIN: &[u8] = b"sorafs-gateway-stream-token-v1\0";
const MAX_GATEWAY_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_GATEWAY_CAR_BYTES: usize = 96 * 1024 * 1024;
const MAX_GATEWAY_RANGE_CAR_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOKEN_REQUEST_BODY_BYTES: usize = 24 * 1024;
const MAX_MANIFEST_ENVELOPE_HEADER_BYTES: usize = 16 * 1024;
const MAX_MANIFEST_ENVELOPE_DECODED_BYTES: usize = 12 * 1024;
const MAX_STREAM_TOKEN_HEADER_BYTES: usize = 8 * 1024;
const MAX_STREAM_TOKEN_DECODED_BYTES: usize = 6 * 1024;
const MAX_CLIENT_ID_BYTES: usize = 64;
const MAX_CAPABILITIES: usize = 8;
const MAX_CAPABILITY_BYTES: usize = 96;
const MAX_ALIAS_BYTES: usize = 253;
const MAX_HOST_BYTES: usize = 253;
const MAX_TOKEN_TTL_SECS: u64 = 24 * 60 * 60;
const MAX_TOKEN_STREAMS: u16 = 64;
const MAX_TOKEN_RATE_LIMIT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOKEN_REQUESTS_PER_MINUTE: u32 = 10_000;
const MAX_TOKEN_REGISTRY_ENTRIES: usize = 65_536;
const MAX_CLIENT_QUOTA_ENTRIES: usize = 65_536;

/// Shared gateway dataset loaded from the local storage backend.
#[derive(Debug)]
pub struct GatewayDataset {
    manifest: ManifestV1,
    manifest_id_hex: String,
    content_cid: String,
    chunker_alias: String,
    provider_id: [u8; 32],
    car_bytes: Bytes,
    payload_bytes: Arc<Vec<u8>>,
    plan: CarBuildPlan,
    por_tree: PorMerkleTree,
    proof: PorProofV1,
    proof_digest_hex: String,
    por_root_hex: String,
    profile_version: String,
    route_generated_at: String,
    proof_verified: AtomicBool,
    signing_key: PrivateKey,
    signing_public_key: [u8; 32],
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
        if provider_id.iter().all(|byte| *byte == 0) {
            return Err(eyre::eyre!(
                "gateway provider_id must not be the all-zero identifier"
            ));
        }
        if manifest_digest_hex.len() != 64
            || !manifest_digest_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(eyre::eyre!(
                "manifest digest must be canonical 32-byte lowercase hex"
            ));
        }
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
        if manifest_id_hex != manifest_digest_hex {
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
        if content_len == 0 || content_len > MAX_GATEWAY_PAYLOAD_BYTES {
            return Err(eyre::eyre!(
                "gateway payload length {content_len} is outside the supported range 1..={MAX_GATEWAY_PAYLOAD_BYTES}"
            ));
        }
        let payload_raw = node
            .read_payload_range(stored.manifest_id(), 0, content_len)
            .map_err(|err| eyre::eyre!("{err}"))?;
        if payload_raw.len() != content_len {
            return Err(eyre::eyre!("payload length mismatch"));
        }

        let mut car_bytes = BoundedBuffer::new(MAX_GATEWAY_CAR_BYTES);
        CarWriter::new(&plan, &payload_raw)
            .map_err(|err| eyre::eyre!(err.to_string()))?
            .write_to(&mut car_bytes)
            .map_err(|err| eyre::eyre!("gateway CAR exceeds its memory limit: {err}"))?;
        let car_bytes = car_bytes.into_inner();
        validate_gateway_car_archive(&manifest, &car_bytes)?;

        let por_tree = stored.por_tree();
        let signing_key = load_gateway_signing_key(node.config())?;
        let signing_public_key = gateway_signing_public_key(&signing_key)?;
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
            car_bytes: Bytes::from(car_bytes),
            payload_bytes: Arc::new(payload_raw),
            plan,
            por_tree,
            proof,
            proof_digest_hex,
            por_root_hex,
            profile_version: SORAFS_GATEWAY_PROFILE_VERSION.to_string(),
            route_generated_at,
            proof_verified: AtomicBool::new(false),
            signing_key,
            signing_public_key,
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
        if proof_manifest_hex != self.manifest_id_hex() {
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

        let mut car_bytes = BoundedBuffer::new(MAX_GATEWAY_RANGE_CAR_BYTES);
        let writer = CarWriter::new(&sub_plan, &range_payload)
            .map_err(|err| GatewayResponseError::internal(err.into()))?;
        writer
            .write_to(&mut car_bytes)
            .map_err(|err| GatewayResponseError::internal(err.into()))?;

        Ok(car_bytes.into_inner())
    }
}

fn validate_gateway_car_archive(manifest: &ManifestV1, car_bytes: &[u8]) -> eyre::Result<()> {
    let actual_size = u64::try_from(car_bytes.len())
        .map_err(|_| eyre::eyre!("gateway CAR length exceeds the supported u64 range"))?;
    if actual_size != manifest.car_size {
        return Err(eyre::eyre!(
            "gateway CAR size {actual_size} does not match manifest car_size {}",
            manifest.car_size
        ));
    }
    let actual_digest = blake3::hash(car_bytes);
    if actual_digest.as_bytes() != &manifest.car_digest {
        return Err(eyre::eyre!(
            "gateway CAR digest does not match manifest car_digest"
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct BoundedBuffer {
    bytes: Vec<u8>,
    max_len: usize,
}

impl BoundedBuffer {
    fn new(max_len: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_len,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next_len = self
            .bytes
            .len()
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("bounded buffer length overflow"))?;
        if next_len > self.max_len {
            return Err(io::Error::other("bounded buffer capacity exceeded"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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
        Err(eyre::eyre!(
            "manifest chunker profile id {} is not registered",
            manifest.chunking.profile_id.0
        ))
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
        .try_prove_leaf(chunk_idx, segment_idx, leaf_idx, payload)
        .map_err(|err| eyre::eyre!("failed to build PoR proof from payload: {err}"))?
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
    let (algorithm, public_key) = keypair
        .public_key()
        .try_to_bytes()
        .wrap_err("failed to extract gateway signing public key")?;
    if algorithm != Algorithm::Ed25519 {
        return Err(eyre::eyre!(
            "gateway signing key must derive an Ed25519 public key, found {}",
            algorithm.as_static_str()
        ));
    }
    if public_key.len() != 32 {
        return Err(eyre::eyre!(
            "gateway signing public key must be 32 bytes, found {}",
            public_key.len()
        ));
    }
    let signature = Signature::try_new(signing_key, proof_digest.as_ref())
        .wrap_err("failed to sign gateway PoR proof")?;
    por_proof.signature = AdvertSignature {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: public_key.to_vec(),
        signature: signature.payload().to_vec(),
    };

    Ok(por_proof)
}

pub(crate) fn load_gateway_signing_key(config: &StorageConfig) -> Result<PrivateKey, eyre::Report> {
    let path = config
        .stream_token_signing_key_path()
        .ok_or_else(|| eyre::eyre!("gateway signing key path not configured"))?;
    let mut raw = read_gateway_signing_key_file(path)?;
    let parsed = if raw.len() == 64
        && raw
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        hex::decode(&raw).wrap_err("failed to decode lowercase hex gateway signing key")
    } else if raw.len() == 32 {
        Ok(raw.clone())
    } else {
        Err(eyre::eyre!(
            "gateway signing key at {} must be exactly 32 raw bytes or 64 lowercase hex bytes without whitespace",
            path.display()
        ))
    };
    raw.fill(0);
    let mut key_bytes = parsed?;
    if key_bytes.len() != 32 {
        key_bytes.fill(0);
        return Err(eyre::eyre!(
            "gateway signing key at {} must be 32 bytes, found {}",
            path.display(),
            key_bytes.len()
        ));
    }

    let mut array = [0u8; 32];
    array.copy_from_slice(&key_bytes);
    key_bytes.fill(0);
    if array.iter().all(|byte| *byte == 0) {
        array.fill(0);
        return Err(eyre::eyre!(
            "gateway signing key at {} must not be all zero",
            path.display()
        ));
    }
    let parsed = PrivateKey::from_bytes(Algorithm::Ed25519, &array);
    array.fill(0);
    parsed.wrap_err("failed to parse gateway signing key")
}

fn read_gateway_signing_key_file(path: &std::path::Path) -> Result<Vec<u8>, eyre::Report> {
    let before_open = fs::symlink_metadata(path).wrap_err_with(|| {
        format!(
            "failed to inspect gateway signing key at {}",
            path.display()
        )
    })?;
    validate_gateway_signing_key_metadata(path, &before_open)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_gateway_no_follow_flag(&mut options);
    let mut key_file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open gateway signing key at {}", path.display()))?;
    let opened_metadata = key_file.metadata().wrap_err_with(|| {
        format!(
            "failed to inspect opened gateway signing key at {}",
            path.display()
        )
    })?;
    validate_gateway_signing_key_metadata(path, &opened_metadata)?;
    if !gateway_metadata_identifies_same_file(&before_open, &opened_metadata) {
        return Err(eyre::eyre!(
            "gateway signing key at {} changed while being opened",
            path.display()
        ));
    }
    let max_bytes = u64::try_from(GATEWAY_SIGNING_KEY_FILE_MAX_BYTES)
        .expect("gateway signing-key limit fits u64");
    let mut raw = Vec::with_capacity(
        usize::try_from(opened_metadata.len()).unwrap_or(GATEWAY_SIGNING_KEY_FILE_MAX_BYTES),
    );
    let read_result = (&mut key_file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut raw);
    if let Err(err) = read_result {
        raw.fill(0);
        return Err(err).wrap_err_with(|| {
            format!("failed to read gateway signing key from {}", path.display())
        });
    }
    let validation = (|| -> Result<(), eyre::Report> {
        if raw.len() > GATEWAY_SIGNING_KEY_FILE_MAX_BYTES {
            return Err(eyre::eyre!(
                "gateway signing key at {} exceeds {} bytes",
                path.display(),
                GATEWAY_SIGNING_KEY_FILE_MAX_BYTES
            ));
        }
        let after_read_file = key_file.metadata().wrap_err_with(|| {
            format!(
                "failed to re-inspect opened gateway signing key at {}",
                path.display()
            )
        })?;
        if !gateway_metadata_stable_during_read(&opened_metadata, &after_read_file) {
            return Err(eyre::eyre!(
                "gateway signing key at {} changed while being read",
                path.display()
            ));
        }
        let after_read_path = fs::symlink_metadata(path).wrap_err_with(|| {
            format!(
                "failed to re-inspect gateway signing key path at {}",
                path.display()
            )
        })?;
        validate_gateway_signing_key_metadata(path, &after_read_path)?;
        if !gateway_metadata_identifies_same_file(&opened_metadata, &after_read_path) {
            return Err(eyre::eyre!(
                "gateway signing key path at {} changed while being read",
                path.display()
            ));
        }
        Ok(())
    })();
    if let Err(err) = validation {
        raw.fill(0);
        return Err(err);
    }
    Ok(raw)
}

fn validate_gateway_signing_key_metadata(
    path: &std::path::Path,
    metadata: &fs::Metadata,
) -> Result<(), eyre::Report> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(eyre::eyre!(
            "gateway signing key at {} must be a non-symlink regular file",
            path.display()
        ));
    }
    if metadata.len() > GATEWAY_SIGNING_KEY_FILE_MAX_BYTES as u64 {
        return Err(eyre::eyre!(
            "gateway signing key at {} exceeds {} bytes",
            path.display(),
            GATEWAY_SIGNING_KEY_FILE_MAX_BYTES
        ));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(eyre::eyre!(
                "gateway signing key at {} must have exactly one hard link",
                path.display()
            ));
        }
        if metadata.uid() != gateway_effective_user_id() {
            return Err(eyre::eyre!(
                "gateway signing key at {} must be owned by the effective user",
                path.display()
            ));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(eyre::eyre!(
                "gateway signing key at {} must not be accessible by group or other users",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn gateway_effective_user_id() -> u32 {
    // SAFETY: `geteuid` takes no arguments, owns no resources, and cannot fail.
    unsafe { geteuid() }
}

#[cfg(unix)]
fn gateway_metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn gateway_metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(unix)]
fn gateway_metadata_stable_during_read(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    gateway_metadata_identifies_same_file(left, right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn gateway_metadata_stable_during_read(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    gateway_metadata_identifies_same_file(left, right)
}

#[cfg(unix)]
fn set_gateway_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(gateway_no_follow_flag());
}

#[cfg(not(unix))]
fn set_gateway_no_follow_flag(_options: &mut fs::OpenOptions) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn gateway_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn gateway_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn gateway_no_follow_flag() -> i32 {
    0
}

fn gateway_signing_public_key(signing_key: &PrivateKey) -> Result<[u8; 32], eyre::Report> {
    let keypair = KeyPair::from_private_key(signing_key.clone())
        .wrap_err("failed to derive gateway signing keypair")?;
    let (algorithm, public_key) = keypair
        .public_key()
        .try_to_bytes()
        .wrap_err("failed to extract gateway signing public key")?;
    if algorithm != Algorithm::Ed25519 {
        return Err(eyre::eyre!(
            "gateway signing key must derive an Ed25519 public key, found {}",
            algorithm.as_static_str()
        ));
    }
    public_key
        .try_into()
        .map_err(|_| eyre::eyre!("gateway signing public key must be 32 bytes"))
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
    let signature = ed25519_parse_signature(&proof.signature.signature).map_err(|err| {
        let mut details = json::Map::new();
        details.insert(
            "signature_len".into(),
            Value::from(proof.signature.signature.len() as u64),
        );
        refusal(
            format!("proof signature material is invalid: {err}"),
            Some(Value::Object(details)),
        )
    })?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct StreamTokenPayload {
    version: u8,
    token_id: String,
    nonce_hex: String,
    manifest_digest_hex: String,
    provider_id_hex: String,
    profile_handle: String,
    max_streams: u16,
    ttl_epoch: u64,
    rate_limit_bytes: u64,
    issued_at: u64,
    requests_per_minute: u32,
    client_id: String,
    capabilities: Vec<String>,
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
    /// Maximum concurrent range requests allowed per token.
    pub max_streams: u16,
    /// Maximum payload length permitted per request.
    pub rate_limit_bytes: u64,
    /// Maximum number of token issuances per minute.
    pub requests_per_minute: Option<u32>,
    /// Maximum live tokens retained by this gateway process.
    pub max_issued_tokens: usize,
    /// Maximum distinct client quota records retained by this gateway process.
    pub max_quota_clients: usize,
}

impl Default for TokenPolicy {
    fn default() -> Self {
        Self {
            ttl_secs: 900,
            max_streams: 4,
            rate_limit_bytes: 8 * 1024 * 1024,
            requests_per_minute: Some(120),
            max_issued_tokens: 4_096,
            max_quota_clients: 1_024,
        }
    }
}

impl TokenPolicy {
    fn validate(&self) -> Result<(), GatewayStateError> {
        if self.ttl_secs == 0 || self.ttl_secs > MAX_TOKEN_TTL_SECS {
            return Err(GatewayStateError::InvalidTokenPolicy(format!(
                "ttl_secs must be in 1..={MAX_TOKEN_TTL_SECS}"
            )));
        }
        if self.max_streams == 0 || self.max_streams > MAX_TOKEN_STREAMS {
            return Err(GatewayStateError::InvalidTokenPolicy(format!(
                "max_streams must be in 1..={MAX_TOKEN_STREAMS}"
            )));
        }
        if self.rate_limit_bytes == 0 || self.rate_limit_bytes > MAX_TOKEN_RATE_LIMIT_BYTES {
            return Err(GatewayStateError::InvalidTokenPolicy(format!(
                "rate_limit_bytes must be in 1..={MAX_TOKEN_RATE_LIMIT_BYTES}"
            )));
        }
        let issuance_limit = self.requests_per_minute.ok_or_else(|| {
            GatewayStateError::InvalidTokenPolicy(
                "requests_per_minute must be configured".to_owned(),
            )
        })?;
        if issuance_limit == 0 || issuance_limit > MAX_TOKEN_REQUESTS_PER_MINUTE {
            return Err(GatewayStateError::InvalidTokenPolicy(format!(
                "requests_per_minute must be in 1..={MAX_TOKEN_REQUESTS_PER_MINUTE}"
            )));
        }
        if self.max_issued_tokens == 0 || self.max_issued_tokens > MAX_TOKEN_REGISTRY_ENTRIES {
            return Err(GatewayStateError::InvalidTokenPolicy(format!(
                "max_issued_tokens must be in 1..={MAX_TOKEN_REGISTRY_ENTRIES}"
            )));
        }
        if self.max_quota_clients == 0 || self.max_quota_clients > MAX_CLIENT_QUOTA_ENTRIES {
            return Err(GatewayStateError::InvalidTokenPolicy(format!(
                "max_quota_clients must be in 1..={MAX_CLIENT_QUOTA_ENTRIES}"
            )));
        }
        Ok(())
    }

    fn issuance_limit(&self) -> u32 {
        self.requests_per_minute
            .expect("validated token policy always has an issuance limit")
    }
}

#[derive(Debug)]
struct TokenIssue {
    payload: StreamTokenPayload,
    encoded: String,
    signature_hex: String,
    public_key_hex: String,
    ttl_epoch: u64,
    remaining_quota: u32,
}

#[derive(Debug)]
enum TokenIssueError {
    ClientQuotaExceeded { limit: u32, retry_after_secs: u64 },
    RegistryCapacity { registry: &'static str },
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

#[derive(Debug)]
struct TokenAcquisition {
    token_id: String,
    payload: StreamTokenPayload,
}

#[derive(Debug)]
struct TokenRegistry {
    tokens: Mutex<HashMap<String, TokenRecord>>,
    client_quotas: Mutex<HashMap<String, ClientQuota>>,
    policy: TokenPolicy,
    signing_key: PrivateKey,
    signing_public_key: [u8; 32],
}

impl TokenRegistry {
    fn new(policy: TokenPolicy, signing_key: PrivateKey, signing_public_key: [u8; 32]) -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            client_quotas: Mutex::new(HashMap::new()),
            policy,
            signing_key,
            signing_public_key,
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
        capabilities: &[String],
    ) -> Result<TokenIssue, TokenIssueError> {
        let wall_clock_now = SystemTime::now();
        self.purge_expired_entries(wall_clock_now, Instant::now());

        let issued_at = wall_clock_now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| TokenIssueError::Internal(err.into()))?
            .as_secs();
        let ttl_epoch = issued_at
            .checked_add(self.policy.ttl_secs)
            .ok_or_else(|| TokenIssueError::Internal(eyre::eyre!("token TTL overflow")))?;

        let limit = self.policy.issuance_limit();
        let remaining_quota = {
            let mut quotas = self
                .client_quotas
                .lock()
                .map_err(|err| TokenIssueError::Internal(eyre::eyre!(err.to_string())))?;
            let now = Instant::now();
            if !quotas.contains_key(client_id) && quotas.len() >= self.policy.max_quota_clients {
                return Err(TokenIssueError::RegistryCapacity {
                    registry: "client quota",
                });
            }
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
                    quota.used = quota.used.saturating_add(1);
                    quota.limit.saturating_sub(quota.used)
                }
                Entry::Vacant(entry) => {
                    entry.insert(ClientQuota {
                        window_start: now,
                        limit,
                        used: 1,
                    });
                    limit.saturating_sub(1)
                }
            }
        };

        let token_id = random_hex_32()?;
        let nonce_hex = random_hex_32()?;
        let payload = StreamTokenPayload {
            version: STREAM_TOKEN_VERSION,
            token_id: token_id.clone(),
            nonce_hex,
            manifest_digest_hex: dataset.manifest_id_hex().to_string(),
            provider_id_hex: dataset.provider_id_hex(),
            profile_handle: dataset.chunker_alias().to_string(),
            max_streams: self.policy.max_streams,
            ttl_epoch,
            rate_limit_bytes: self.policy.rate_limit_bytes,
            issued_at,
            requests_per_minute: limit,
            client_id: client_id.to_string(),
            capabilities: capabilities.to_vec(),
        };

        let signed = encode_stream_token(&payload, &self.signing_key, self.signing_public_key)
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
        if guard.len() >= self.policy.max_issued_tokens {
            return Err(TokenIssueError::RegistryCapacity {
                registry: "issued token",
            });
        }
        if guard.insert(token_id, record).is_some() {
            return Err(TokenIssueError::Internal(eyre::eyre!(
                "operating-system random token identifier collision"
            )));
        }
        Ok(TokenIssue {
            payload,
            encoded: signed.encoded,
            signature_hex: signed.signature_hex,
            public_key_hex: signed.public_key_hex,
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

        let payload = decode_stream_token_header(header, self.signing_public_key)?;
        if payload.manifest_digest_hex != dataset.manifest_id_hex() {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "admission_mismatch",
                "stream token manifest does not match gateway dataset",
            ));
        }
        if payload.provider_id_hex != dataset.provider_id_hex() {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "provider_mismatch",
                "stream token provider does not match gateway dataset",
            ));
        }
        if payload.profile_handle != dataset.chunker_alias() {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "unsupported_chunker",
                "stream token profile does not match gateway dataset",
            ));
        }
        if payload.capabilities.len() != SUPPORTED_CAPABILITIES.len()
            || payload
                .capabilities
                .iter()
                .zip(SUPPORTED_CAPABILITIES.iter().copied())
                .any(|(actual, expected)| actual != expected)
        {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "unsupported_capability",
                "stream token capabilities do not match the range endpoint",
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
        if payload != record.payload {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "stream_token_payload_mismatch",
                "stream token payload does not exactly match the issued record",
            ));
        }
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
        if record.active_streams >= u32::from(record.payload.max_streams) {
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

struct SignedStreamToken {
    encoded: String,
    signature_hex: String,
    public_key_hex: String,
}

fn encode_stream_token(
    payload: &StreamTokenPayload,
    signing_key: &PrivateKey,
    signing_public_key: [u8; 32],
) -> Result<SignedStreamToken, GatewayResponseError> {
    let payload_value = payload_to_value(payload);
    let payload_bytes = norito::json::to_vec(&payload_value)
        .map_err(|err| GatewayResponseError::internal(err.into()))?;
    let signing_message = stream_token_signing_message(&payload_bytes)?;
    let signature = Signature::try_new(signing_key, &signing_message)
        .map_err(|err| GatewayResponseError::internal(err.into()))?;
    let signature_hex = hex::encode(signature.payload());
    let public_key_hex = hex::encode(signing_public_key);

    let mut envelope = json::Map::new();
    envelope.insert("payload".into(), payload_value);
    envelope.insert("public_key_hex".into(), Value::from(public_key_hex.clone()));
    envelope.insert("signature_hex".into(), Value::from(signature_hex.clone()));
    let bytes = norito::json::to_vec(&Value::Object(envelope))
        .map_err(|err| GatewayResponseError::internal(err.into()))?;
    if bytes.len() > MAX_STREAM_TOKEN_DECODED_BYTES {
        return Err(GatewayResponseError::internal(eyre::eyre!(
            "signed stream token exceeds internal size limit"
        )));
    }
    Ok(SignedStreamToken {
        encoded: base64::engine::general_purpose::STANDARD.encode(bytes),
        signature_hex,
        public_key_hex,
    })
}

fn decode_stream_token_header(
    header: &HeaderValue,
    expected_public_key: [u8; 32],
) -> Result<StreamTokenPayload, GatewayResponseError> {
    let raw = header.to_str().map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_encoding",
            "stream token must be valid ASCII",
        )
    })?;
    if raw.is_empty() || raw.len() > MAX_STREAM_TOKEN_HEADER_BYTES || raw != raw.trim() {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_encoding",
            "stream token header has invalid canonical length or whitespace",
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
    if decoded.len() > MAX_STREAM_TOKEN_DECODED_BYTES
        || base64::engine::general_purpose::STANDARD.encode(&decoded) != raw
    {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_encoding",
            "stream token header is not canonical base64 or exceeds its size limit",
        ));
    }

    let value: Value = norito::json::from_slice(&decoded).map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_encoding",
            "stream token payload is not valid Norito JSON",
        )
    })?;
    let canonical =
        norito::json::to_vec(&value).map_err(|err| GatewayResponseError::internal(err.into()))?;
    if canonical != decoded {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_encoding",
            "stream token envelope must use canonical Norito JSON bytes",
        ));
    }
    let map = value.as_object().ok_or_else(stream_token_encoding_error)?;
    require_exact_keys(map, &["payload", "public_key_hex", "signature_hex"])?;

    let public_key_hex = parse_required_string(map, "public_key_hex")?;
    let public_key_bytes = decode_canonical_lower_hex(&public_key_hex, 32, "public_key_hex")?;
    let public_key_array: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| stream_token_admission_error("stream token public key has invalid length"))?;
    ed25519_parse_public_key(&public_key_array).map_err(|_| {
        stream_token_admission_error("stream token public key is malformed or weak")
    })?;
    if public_key_array != expected_public_key {
        return Err(stream_token_admission_error(
            "stream token was signed by a different gateway key",
        ));
    }
    let public_key =
        PublicKey::from_bytes(Algorithm::Ed25519, &public_key_array).map_err(|_| {
            stream_token_admission_error("stream token public key is malformed or weak")
        })?;

    let signature_hex = parse_required_string(map, "signature_hex")?;
    let signature_bytes = decode_canonical_lower_hex(&signature_hex, 64, "signature_hex")?;
    let signature = ed25519_parse_signature(&signature_bytes).map_err(|_| {
        stream_token_admission_error("stream token signature is malformed or non-canonical")
    })?;

    let payload_value = map
        .get("payload")
        .ok_or_else(stream_token_encoding_error)?
        .clone();
    let payload_bytes = norito::json::to_vec(&payload_value)
        .map_err(|err| GatewayResponseError::internal(err.into()))?;
    let signing_message = stream_token_signing_message(&payload_bytes)?;
    signature
        .verify(&public_key, &signing_message)
        .map_err(|_| stream_token_admission_error("stream token signature does not verify"))?;
    let payload = parse_stream_token_value(payload_value)?;
    validate_stream_token_payload(&payload)?;
    Ok(payload)
}

fn payload_to_value(payload: &StreamTokenPayload) -> Value {
    let mut map = json::Map::new();
    map.insert("version".into(), Value::from(payload.version));
    map.insert("token_id".into(), Value::from(payload.token_id.clone()));
    map.insert("nonce_hex".into(), Value::from(payload.nonce_hex.clone()));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(payload.manifest_digest_hex.clone()),
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
    map.insert(
        "capabilities".into(),
        Value::Array(
            payload
                .capabilities
                .iter()
                .cloned()
                .map(Value::from)
                .collect(),
        ),
    );
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
    require_exact_keys(
        map,
        &[
            "capabilities",
            "client_id",
            "issued_at",
            "manifest_digest_hex",
            "max_streams",
            "nonce_hex",
            "profile_handle",
            "provider_id_hex",
            "rate_limit_bytes",
            "requests_per_minute",
            "token_id",
            "ttl_epoch",
            "version",
        ],
    )?;

    let version_raw = parse_required_u64(map, "version")?;
    let version = u8::try_from(version_raw).map_err(|_| {
        stream_token_admission_error("stream token version exceeds supported range")
    })?;
    let token_id = parse_required_string(map, "token_id")?;
    let nonce_hex = parse_required_string(map, "nonce_hex")?;
    let manifest_digest_hex = parse_required_string(map, "manifest_digest_hex")?;
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
    let capabilities = map
        .get("capabilities")
        .and_then(Value::as_array)
        .ok_or_else(|| stream_token_admission_error("stream token capabilities must be an array"))?
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                stream_token_admission_error("stream token capabilities must contain only strings")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(StreamTokenPayload {
        version,
        token_id,
        nonce_hex,
        manifest_digest_hex,
        provider_id_hex,
        profile_handle,
        max_streams,
        ttl_epoch,
        rate_limit_bytes,
        issued_at,
        requests_per_minute,
        client_id,
        capabilities,
    })
}

fn stream_token_signing_message(payload_bytes: &[u8]) -> Result<Vec<u8>, GatewayResponseError> {
    let payload_len = u64::try_from(payload_bytes.len()).map_err(|_| {
        GatewayResponseError::internal(eyre::eyre!("stream token payload length overflow"))
    })?;
    let mut message = Vec::with_capacity(
        STREAM_TOKEN_SIGNATURE_DOMAIN.len() + std::mem::size_of::<u64>() + payload_bytes.len(),
    );
    message.extend_from_slice(STREAM_TOKEN_SIGNATURE_DOMAIN);
    message.extend_from_slice(&payload_len.to_be_bytes());
    message.extend_from_slice(payload_bytes);
    Ok(message)
}

fn random_hex_32() -> Result<String, TokenIssueError> {
    for _ in 0..4 {
        let mut bytes = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|err| TokenIssueError::Internal(eyre::eyre!("OS randomness failed: {err}")))?;
        if bytes.iter().any(|byte| *byte != 0) {
            return Ok(hex::encode(bytes));
        }
    }
    Err(TokenIssueError::Internal(eyre::eyre!(
        "OS randomness returned an all-zero token component repeatedly"
    )))
}

fn validate_stream_token_payload(payload: &StreamTokenPayload) -> Result<(), GatewayResponseError> {
    if payload.version != STREAM_TOKEN_VERSION {
        return Err(stream_token_admission_error(
            "stream token version is unsupported",
        ));
    }
    decode_canonical_lower_hex(&payload.token_id, 32, "token_id")?;
    decode_canonical_lower_hex(&payload.nonce_hex, 32, "nonce_hex")?;
    decode_canonical_lower_hex(&payload.manifest_digest_hex, 32, "manifest_digest_hex")?;
    decode_canonical_lower_hex(&payload.provider_id_hex, 32, "provider_id_hex")?;
    validate_client_id(&payload.client_id)?;
    validate_capabilities(&payload.capabilities)?;
    if payload.profile_handle.is_empty()
        || payload.profile_handle.len() > MAX_CAPABILITY_BYTES
        || payload.profile_handle != payload.profile_handle.trim()
    {
        return Err(stream_token_admission_error(
            "stream token profile handle is not canonical",
        ));
    }
    if payload.max_streams == 0 || payload.max_streams > MAX_TOKEN_STREAMS {
        return Err(stream_token_admission_error(
            "stream token max_streams is outside the supported range",
        ));
    }
    if payload.rate_limit_bytes == 0 || payload.rate_limit_bytes > MAX_TOKEN_RATE_LIMIT_BYTES {
        return Err(stream_token_admission_error(
            "stream token byte limit is outside the supported range",
        ));
    }
    if payload.requests_per_minute == 0
        || payload.requests_per_minute > MAX_TOKEN_REQUESTS_PER_MINUTE
    {
        return Err(stream_token_admission_error(
            "stream token issuance limit is outside the supported range",
        ));
    }
    let ttl = payload
        .ttl_epoch
        .checked_sub(payload.issued_at)
        .ok_or_else(|| stream_token_admission_error("stream token timestamps are inverted"))?;
    if ttl == 0 || ttl > MAX_TOKEN_TTL_SECS {
        return Err(stream_token_admission_error(
            "stream token TTL is outside the supported range",
        ));
    }
    let now = unix_now().map_err(GatewayResponseError::internal)?;
    if payload.issued_at > now {
        return Err(stream_token_admission_error(
            "stream token issue time is in the future",
        ));
    }
    if payload.ttl_epoch <= now {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "stream_token_expired",
            "stream token has expired",
        ));
    }
    Ok(())
}

fn validate_client_id(client_id: &str) -> Result<(), GatewayResponseError> {
    if client_id.is_empty()
        || client_id.len() > MAX_CLIENT_ID_BYTES
        || client_id != client_id.trim()
        || !client_id.is_ascii()
        || !client_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.:@".contains(&byte)
        })
        || !client_id
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !client_id
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::BAD_REQUEST,
            "invalid_client_id",
            "client id must be 1-64 canonical lowercase ASCII identifier bytes",
        ));
    }
    Ok(())
}

fn validate_capabilities(capabilities: &[String]) -> Result<(), GatewayResponseError> {
    if capabilities.is_empty() || capabilities.len() > MAX_CAPABILITIES {
        return Err(stream_token_admission_error(
            "capability list must be non-empty and bounded",
        ));
    }
    let mut previous: Option<&str> = None;
    for capability in capabilities {
        if capability.is_empty()
            || capability.len() > MAX_CAPABILITY_BYTES
            || capability != capability.trim()
            || !capability.is_ascii()
        {
            return Err(stream_token_admission_error(
                "stream token contains a non-canonical capability",
            ));
        }
        if previous.is_some_and(|entry| entry >= capability.as_str()) {
            return Err(stream_token_admission_error(
                "stream token capabilities must be strictly sorted and unique",
            ));
        }
        previous = Some(capability);
    }
    Ok(())
}

fn decode_canonical_lower_hex(
    value: &str,
    expected_bytes: usize,
    field: &str,
) -> Result<Vec<u8>, GatewayResponseError> {
    if value.len() != expected_bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(stream_token_admission_error(format!(
            "stream token `{field}` must be canonical lowercase hex"
        )));
    }
    let bytes = hex::decode(value).map_err(|_| {
        stream_token_admission_error(format!("stream token `{field}` is invalid hex"))
    })?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(stream_token_admission_error(format!(
            "stream token `{field}` must not be all zero"
        )));
    }
    Ok(bytes)
}

fn require_exact_keys(map: &json::Map, expected: &[&str]) -> Result<(), GatewayResponseError> {
    if map.len() != expected.len() || expected.iter().any(|key| !map.contains_key(*key)) {
        return Err(stream_token_admission_error(
            "stream token contains missing or unknown fields",
        ));
    }
    Ok(())
}

fn stream_token_encoding_error() -> GatewayResponseError {
    GatewayResponseError::capability_refusal(
        StatusCode::PRECONDITION_FAILED,
        "unsupported_encoding",
        "stream token envelope has an invalid shape",
    )
}

fn stream_token_admission_error(reason: impl Into<String>) -> GatewayResponseError {
    GatewayResponseError::capability_refusal(
        StatusCode::PRECONDITION_FAILED,
        "admission_mismatch",
        reason,
    )
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

/// Invalid standalone-gateway startup configuration.
#[derive(Debug, Error)]
pub enum GatewayStateError {
    /// The token limits would make issuance unbounded or unusable.
    #[error("invalid stream-token policy: {0}")]
    InvalidTokenPolicy(String),
    /// The explicit operator approval does not canonically bind this dataset.
    #[error("invalid operator-approved manifest envelope: {0}")]
    InvalidApprovedManifestEnvelope(String),
}

/// Convenience wrapper exposed via axum state.
#[derive(Clone)]
pub struct GatewayState {
    dataset: Arc<GatewayDataset>,
    tokens: Arc<TokenRegistry>,
    approved_manifest_envelope: Arc<str>,
    telemetry: GatewayTelemetry,
}

impl GatewayState {
    /// Construct a shared state handle pinned to one operator-approved envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when the approval envelope is not canonical or does not
    /// describe the loaded dataset, or when the default token policy is invalid.
    pub fn new(
        dataset: GatewayDataset,
        approved_manifest_envelope: impl Into<String>,
    ) -> Result<Self, GatewayStateError> {
        Self::with_token_policy(dataset, approved_manifest_envelope, TokenPolicy::default())
    }

    /// Construct a shared state handle with an approved envelope and token policy.
    ///
    /// # Errors
    ///
    /// Returns an error when the approval envelope or token policy is invalid.
    pub fn with_token_policy(
        dataset: GatewayDataset,
        approved_manifest_envelope: impl Into<String>,
        policy: TokenPolicy,
    ) -> Result<Self, GatewayStateError> {
        policy.validate()?;
        let approved_manifest_envelope = approved_manifest_envelope.into();
        let header = HeaderValue::from_str(&approved_manifest_envelope).map_err(|_| {
            GatewayStateError::InvalidApprovedManifestEnvelope(
                "approval must be an ASCII HTTP header value".to_owned(),
            )
        })?;
        validate_manifest_envelope_structure(&dataset, &header)
            .map_err(|err| GatewayStateError::InvalidApprovedManifestEnvelope(err.to_string()))?;
        let tokens = TokenRegistry::new(
            policy,
            dataset.signing_key.clone(),
            dataset.signing_public_key,
        );
        Ok(Self {
            tokens: Arc::new(tokens),
            dataset: Arc::new(dataset),
            approved_manifest_envelope: Arc::from(approved_manifest_envelope),
            telemetry: GatewayTelemetry::default(),
        })
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

    fn validate_manifest_envelope(&self, header: &HeaderValue) -> Result<(), GatewayResponseError> {
        let raw = header.to_str().map_err(|_| {
            GatewayResponseError::manifest_envelope("header must contain canonical ASCII")
        })?;
        if raw.len() > MAX_MANIFEST_ENVELOPE_HEADER_BYTES
            || raw != self.approved_manifest_envelope.as_ref()
        {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "admission_mismatch",
                "manifest envelope does not exactly match the operator-approved startup envelope",
            ));
        }
        Ok(())
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
        .layer(DefaultBodyLimit::max(MAX_TOKEN_REQUEST_BODY_BYTES))
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
        Ok(response) if response.status().is_success() => {
            request_ctx.finish_success(response.status());
        }
        Ok(response) => request_ctx.finish_error(response.status(), "refusal_response"),
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
    let request = ParsedRequest::from_headers(state, headers)?;

    if headers.get_all(RANGE).iter().count() > 1 {
        return Err(GatewayResponseError::invalid_range(
            "range header must appear exactly once",
        ));
    }
    if let Some(range_header) = headers.get(RANGE) {
        telemetry.mark_variant("range");
        ensure_accepts_block(headers)?;
        ensure_chunker_header(dataset, headers)?;
        if headers.get_all(ACCEPT_ENCODING).iter().count() > 1 {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::NOT_ACCEPTABLE,
                "unsupported_encoding",
                "Accept-Encoding header must not be repeated",
            ));
        }
        if let Some(value) = headers.get(ACCEPT_ENCODING) {
            let raw = value.to_str().map_err(|_| {
                GatewayResponseError::capability_refusal(
                    StatusCode::NOT_ACCEPTABLE,
                    "unsupported_encoding",
                    "Accept-Encoding header must contain canonical ASCII",
                )
            })?;
            if raw.is_empty() || raw.len() > 512 || raw != raw.trim() {
                return Err(GatewayResponseError::capability_refusal(
                    StatusCode::NOT_ACCEPTABLE,
                    "unsupported_encoding",
                    "Accept-Encoding header is empty, non-canonical, or too long",
                ));
            }
            if raw
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
        }
        let client_id = request.client_id().ok_or_else(|| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_REQUIRED,
                "missing_header",
                "range requests require X-SoraFS-Client header",
            )
        })?;
        if headers.get_all(HEADER_STREAM_TOKEN).iter().count() > 1 {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "unsupported_encoding",
                "range requests must include exactly one stream token header",
            ));
        }
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
                .any(|candidate| candidate == alias);
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
        let car_len = car_bytes.len();
        let mut response = Response::new(Body::from(car_bytes));
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
        let headers_mut = response.headers_mut();
        headers_mut.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/vnd.ipld.car"),
        );
        headers_mut.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&car_len.to_string())
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
    let mut response = Response::new(Body::from(dataset.car_bytes.clone()));
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
    let request = ParsedRequest::from_headers(state, headers)?;
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
    let request = ParsedRequest::from_headers(state, headers)?;
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
    if headers.get_all(HEADER_CLIENT_ID).iter().count() > 1 {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::BAD_REQUEST,
            "invalid_client_id",
            "token requests must not repeat X-SoraFS-Client",
        ));
    }
    let client_header = headers.get(HEADER_CLIENT_ID).ok_or_else(|| {
        GatewayResponseError::capability_refusal(
            StatusCode::BAD_REQUEST,
            "missing_client_id",
            "token requests require X-SoraFS-Client header",
        )
    })?;
    let client_id = client_header.to_str().map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::BAD_REQUEST,
            "invalid_client_id",
            "X-SoraFS-Client header must contain valid ASCII",
        )
    })?;
    validate_client_id(client_id)?;

    if body.len() > MAX_TOKEN_REQUEST_BODY_BYTES {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            "token request body exceeds the configured hard limit",
        ));
    }
    let payload: Value = norito::json::from_slice(body).map_err(|_| {
        GatewayResponseError::ManifestEnvelope(
            "token requests must provide a valid JSON payload".to_string(),
        )
    })?;
    let payload_map = payload.as_object().ok_or_else(|| {
        GatewayResponseError::ManifestEnvelope("token request body must be an object".to_owned())
    })?;
    if payload_map.len() != 2
        || !payload_map.contains_key("manifest_envelope")
        || !payload_map.contains_key("capabilities")
    {
        return Err(GatewayResponseError::ManifestEnvelope(
            "token request must contain exactly `manifest_envelope` and `capabilities`".to_owned(),
        ));
    }
    let manifest_envelope = payload_map
        .get("manifest_envelope")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GatewayResponseError::ManifestEnvelope(
                "token requests must supply `manifest_envelope` field".to_string(),
            )
        })?;
    if manifest_envelope.len() > MAX_MANIFEST_ENVELOPE_HEADER_BYTES
        || manifest_envelope != manifest_envelope.trim()
    {
        return Err(GatewayResponseError::ManifestEnvelope(
            "manifest envelope is not a canonical bounded header value".to_owned(),
        ));
    }
    let envelope_header = HeaderValue::from_str(manifest_envelope).map_err(|_| {
        GatewayResponseError::ManifestEnvelope("manifest envelope must be valid ASCII".into())
    })?;
    state.validate_manifest_envelope(&envelope_header)?;

    let capabilities: Vec<String> = payload_map
        .get("capabilities")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GatewayResponseError::capability_refusal(
                StatusCode::BAD_REQUEST,
                "unsupported_capability",
                "token request capabilities must be an array",
            )
        })?
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                GatewayResponseError::capability_refusal(
                    StatusCode::BAD_REQUEST,
                    "unsupported_capability",
                    "token request capabilities must contain only strings",
                )
            })
        })
        .collect::<Result<_, _>>()?;
    validate_capabilities(&capabilities)?;
    for capability in &capabilities {
        if !SUPPORTED_CAPABILITIES.contains(&capability.as_str()) {
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

    let issue = match state
        .tokens()
        .issue_token(dataset, client_id, &capabilities)
    {
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
        Err(TokenIssueError::RegistryCapacity { registry }) => {
            let mut response = json_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "registry_capacity_exceeded",
                format!("{registry} registry is at its configured capacity"),
            );
            response
                .headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from_static("60"));
            response
                .headers_mut()
                .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
            return Ok(response);
        }
        Err(TokenIssueError::Internal(err)) => {
            return Err(GatewayResponseError::internal(err));
        }
    };

    let mut response_map = json::Map::new();
    response_map.insert("token".into(), Value::from(issue.encoded.clone()));
    response_map.insert("token_payload".into(), payload_to_value(&issue.payload));
    response_map.insert("expires_at".into(), Value::from(issue.ttl_epoch));
    response_map.insert("signature".into(), Value::from(issue.signature_hex.clone()));
    response_map.insert(
        "public_key".into(),
        Value::from(issue.public_key_hex.clone()),
    );

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
    let quota_header = issue.remaining_quota.to_string();
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
    if dataset.manifest_id_hex == manifest_id {
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
    if value.is_empty() || value.len() > 128 || value != value.trim() {
        return Err(GatewayResponseError::invalid_range(
            "range header must be a bounded canonical ASCII value",
        ));
    }
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

    for (label, component) in [("start", start_str), ("end", end_str)] {
        if !component.bytes().all(|byte| byte.is_ascii_digit())
            || (component.len() > 1 && component.starts_with('0'))
        {
            return Err(GatewayResponseError::invalid_range(format!(
                "range {label} must be a canonical unsigned integer"
            )));
        }
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

    let mut accept_values = headers.get_all(ACCEPT).iter();
    let accept = accept_values.next().ok_or_else(missing_details)?;
    if accept_values.next().is_some() {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_capability",
            "Accept header must not be repeated",
        ));
    }
    let raw_value = accept.to_str().map_err(|_| {
        GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_capability",
            "Accept header must be valid ASCII",
        )
    })?;
    if raw_value.is_empty() || raw_value.len() > 512 || raw_value != raw_value.trim() {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "unsupported_capability",
            "Accept header is empty, non-canonical, or too long",
        ));
    }
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
    if trimmed == expected && trimmed == value {
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
        state: &GatewayState,
        headers: &HeaderMap,
    ) -> Result<Self, GatewayResponseError> {
        let dataset = state.dataset();
        let version = require_header(headers, HEADER_VERSION)?;
        if !version
            .to_str()
            .map(|value| value == dataset.profile_version)
            .unwrap_or(false)
        {
            return Err(GatewayResponseError::header_mismatch(
                HEADER_VERSION,
                dataset.profile_version.clone(),
            ));
        }

        let nonce = require_header(headers, HEADER_NONCE)?.clone();
        let nonce_raw = nonce.to_str().map_err(|_| {
            GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "invalid_nonce",
                "X-SoraFS-Nonce must be canonical lowercase hex",
            )
        })?;
        decode_canonical_lower_hex(nonce_raw, 32, "request_nonce")?;
        let envelope = require_header(headers, HEADER_MANIFEST_ENVELOPE)?;
        state.validate_manifest_envelope(envelope)?;

        let client_id = headers
            .get(HEADER_CLIENT_ID)
            .map(|value| value.to_str().map(|raw| Some(raw.to_string())))
            .transpose()
            .map_err(|_| {
                GatewayResponseError::capability_refusal(
                    StatusCode::PRECONDITION_FAILED,
                    "invalid_client_header",
                    "X-SoraFS-Client header must contain valid ASCII",
                )
            })?
            .flatten();
        if headers.get_all(HEADER_CLIENT_ID).iter().count() > 1 {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "invalid_client_header",
                "X-SoraFS-Client header must not be repeated",
            ));
        }
        if let Some(client_id) = &client_id {
            validate_client_id(client_id)?;
        }

        if headers.get_all(HEADER_ALIAS).iter().count() > 1 {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "invalid_alias_header",
                "Sora-Name header must not be repeated",
            ));
        }
        let alias = match headers.get(HEADER_ALIAS) {
            None => None,
            Some(value) => {
                let raw = value.to_str().map_err(|_| {
                    GatewayResponseError::capability_refusal(
                        StatusCode::PRECONDITION_FAILED,
                        "invalid_alias_header",
                        "Sora-Name header must contain valid ASCII",
                    )
                })?;
                if raw.is_empty()
                    || raw.len() > MAX_ALIAS_BYTES
                    || raw != raw.trim()
                    || !is_canonical_alias(raw)
                {
                    return Err(GatewayResponseError::capability_refusal(
                        StatusCode::PRECONDITION_FAILED,
                        "invalid_alias_header",
                        "Sora-Name header is empty, non-canonical, or too long",
                    ));
                }
                Some(raw.to_owned())
            }
        };

        if headers.get_all(HOST).iter().count() > 1 {
            return Err(GatewayResponseError::capability_refusal(
                StatusCode::PRECONDITION_FAILED,
                "invalid_host_header",
                "Host header must not be repeated",
            ));
        }
        let host = match headers.get(HOST) {
            None => None,
            Some(value) => {
                let raw = value.to_str().map_err(|_| {
                    GatewayResponseError::capability_refusal(
                        StatusCode::PRECONDITION_FAILED,
                        "invalid_host_header",
                        "Host header must contain valid ASCII",
                    )
                })?;
                if raw.is_empty()
                    || raw.len() > MAX_HOST_BYTES
                    || raw != raw.trim()
                    || !is_canonical_host(raw)
                {
                    return Err(GatewayResponseError::capability_refusal(
                        StatusCode::PRECONDITION_FAILED,
                        "invalid_host_header",
                        "Host header is empty, non-canonical, or too long",
                    ));
                }
                Some(raw.to_owned())
            }
        };

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

fn is_canonical_alias(value: &str) -> bool {
    value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b'_' | b'@')
        })
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn is_canonical_host(value: &str) -> bool {
    value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b':' | b'[' | b']')
        })
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'[')
        && value
            .as_bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b']')
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

    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

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
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let signing_key = write_signing_key_hex();
        let config = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
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

    fn sample_por_tree_payload() -> (Vec<u8>, PorMerkleTree) {
        let payload = b"sorafs gateway proof builder checked signing key".to_vec();
        let plan = CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT)
            .expect("build plan");
        let stored_chunks = plan
            .chunks
            .iter()
            .map(|chunk| sorafs_car::StoredChunk {
                offset: chunk.offset,
                length: chunk.length,
                blake3: chunk.digest,
            })
            .collect::<Vec<_>>();
        let por_tree = PorMerkleTree::try_from_payload(&payload, &stored_chunks)
            .expect("build fixture PoR tree");
        (payload, por_tree)
    }

    fn checked_test_keypair(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("checked SoraFS node gateway {algorithm:?} fixture key generation failed: {err}")
        })
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
        let signature = Signature::try_new(keypair.private_key(), b"sorafs-proof")
            .expect("checked gateway signing-key fixture signature");
        signature
            .verify(keypair.public_key(), b"sorafs-proof")
            .expect("signature should verify");
    }

    #[test]
    fn signing_key_loader_accepts_exact_raw_seed() {
        let mut signing_key = NamedTempFile::new().expect("raw signing key");
        signing_key
            .write_all(&[0x12; 32])
            .expect("write raw signing key");
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(signing_key.path().to_path_buf()))
            .build();
        load_gateway_signing_key(&config).expect("load exact raw seed");
    }

    #[test]
    fn signing_key_loader_rejects_zero_oversize_and_symlink_material() {
        let mut zero_key = NamedTempFile::new().expect("zero key");
        zero_key.write_all(&[0; 32]).expect("write zero key");
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(zero_key.path().to_path_buf()))
            .build();
        let err = load_gateway_signing_key(&config).expect_err("zero key must fail");
        assert!(err.to_string().contains("must not be all zero"));

        let mut oversized = NamedTempFile::new().expect("oversized key");
        oversized.write_all(&[0x55; 65]).expect("write key");
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(oversized.path().to_path_buf()))
            .build();
        let _ = load_gateway_signing_key(&config).expect_err("oversized key must fail");

        #[cfg(unix)]
        {
            let directory = TempDir::new().expect("symlink directory");
            let symlink = directory.path().join("gateway.key");
            std::os::unix::fs::symlink(zero_key.path(), &symlink).expect("create symlink");
            let config = StorageConfig::builder()
                .stream_token_signing_key_path(Some(symlink))
                .build();
            let _ = load_gateway_signing_key(&config).expect_err("symlink key must fail");
        }
    }

    #[test]
    fn signing_key_loader_rejects_uppercase_hex_and_whitespace() {
        let mut uppercase = NamedTempFile::new().expect("uppercase key");
        uppercase
            .write_all(hex::encode_upper([0xAB; 32]).as_bytes())
            .expect("write uppercase key");
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(uppercase.path().to_path_buf()))
            .build();
        let error = load_gateway_signing_key(&config).expect_err("uppercase hex must fail");
        assert!(error.to_string().contains("64 lowercase hex bytes"));

        let mut newline = NamedTempFile::new().expect("newline key");
        newline
            .write_all(format!("{}\n", hex::encode([0xAB; 32])).as_bytes())
            .expect("write newline key");
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(newline.path().to_path_buf()))
            .build();
        let error = load_gateway_signing_key(&config).expect_err("whitespace must fail");
        assert!(error.to_string().contains("exceeds 64 bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn signing_key_loader_rejects_hard_link_and_permissive_mode() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = TempDir::new().expect("key directory");
        let target = directory.path().join("target.key");
        let hard_link = directory.path().join("hard-link.key");
        fs::write(&target, [0x31; 32]).expect("write key target");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("set secure mode");
        fs::hard_link(&target, &hard_link).expect("create hard link");
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(hard_link))
            .build();
        let error = load_gateway_signing_key(&config).expect_err("hard link must fail");
        assert!(error.to_string().contains("exactly one hard link"));

        fs::remove_file(&target).expect("remove hard-link target");
        let permissive = directory.path().join("permissive.key");
        fs::write(&permissive, [0x31; 32]).expect("write permissive key");
        fs::set_permissions(&permissive, fs::Permissions::from_mode(0o644))
            .expect("set permissive mode");
        let config = StorageConfig::builder()
            .stream_token_signing_key_path(Some(permissive))
            .build();
        let error = load_gateway_signing_key(&config).expect_err("permissive mode must fail");
        assert!(error.to_string().contains("group or other users"));
    }

    #[test]
    fn por_proof_signature_rejects_tampering() {
        let (payload, por_tree) = sample_por_tree_payload();
        let manifest_digest = [0xA5; 32];
        let provider_id = [0xCD; 32];
        let signing_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x22; 32]).expect("private key");

        let mut proof = build_por_proof(
            &por_tree,
            &payload,
            manifest_digest,
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
    fn por_proof_signature_rejects_all_zero_signature_material() {
        let (payload, por_tree) = sample_por_tree_payload();
        let manifest_digest = [0xA5; 32];
        let provider_id = [0xCD; 32];
        let signing_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x22; 32]).expect("private key");

        let mut proof = build_por_proof(
            &por_tree,
            &payload,
            manifest_digest,
            provider_id,
            &signing_key,
        )
        .expect("build proof");
        proof.signature.signature.fill(0);

        let err = verify_proof_signature(&proof).expect_err("all-zero proof signature should fail");
        assert_eq!(err.error_code(), "proof_mismatch");
        match err {
            GatewayResponseError::CapabilityRefusal { reason, .. } => {
                assert!(reason.contains("all zero"));
            }
            other => panic!("expected proof mismatch refusal, got {other:?}"),
        }
    }

    #[test]
    fn por_proof_signature_rejects_malformed_ed25519_signature_r() {
        let (payload, por_tree) = sample_por_tree_payload();
        let manifest_digest = [0xA5; 32];
        let provider_id = [0xCD; 32];
        let signing_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x22; 32]).expect("private key");

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_R),
            ("noncanonical", NONCANONICAL_R),
        ] {
            let mut proof = build_por_proof(
                &por_tree,
                &payload,
                manifest_digest,
                provider_id,
                &signing_key,
            )
            .expect("build proof");
            proof.signature.signature[..32].copy_from_slice(&replacement_r);

            let err = verify_proof_signature(&proof)
                .expect_err("malformed proof signature R should fail");
            assert_eq!(err.error_code(), "proof_mismatch");
            match err {
                GatewayResponseError::CapabilityRefusal { reason, .. } => {
                    assert!(
                        reason.contains("proof signature material is invalid"),
                        "{label} signature R produced unexpected reason: {reason}"
                    );
                }
                other => panic!("expected proof mismatch refusal, got {other:?}"),
            }
        }
    }

    #[test]
    fn por_proof_non_ed25519_fixture_key_uses_checked_generation() {
        let secp_keypair = checked_test_keypair(Algorithm::Secp256k1);

        assert_eq!(
            secp_keypair
                .public_key()
                .try_algorithm()
                .expect("checked fixture public-key algorithm"),
            Algorithm::Secp256k1,
        );
    }

    #[test]
    fn por_proof_builder_rejects_non_ed25519_signing_key() {
        let (payload, por_tree) = sample_por_tree_payload();
        let manifest_digest = [0xA5; 32];
        let provider_id = [0xCD; 32];
        let secp_keypair = checked_test_keypair(Algorithm::Secp256k1);

        let err = build_por_proof(
            &por_tree,
            &payload,
            manifest_digest,
            provider_id,
            secp_keypair.private_key(),
        )
        .expect_err("gateway PoR proof signing must require Ed25519");

        assert!(
            err.to_string()
                .contains("gateway signing key must derive an Ed25519 public key"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn chunk_profile_for_manifest_rejects_unknown_profile_id() {
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
            err.to_string().contains("is not registered"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn gateway_car_archive_validation_rejects_size_and_digest_drift() {
        let fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");
        let manifest_bytes =
            fs::read(fixtures.join("manifest_v1.to")).expect("read manifest fixture");
        let manifest: ManifestV1 =
            norito::decode_from_bytes(&manifest_bytes).expect("decode manifest fixture");
        let payload = fs::read(fixtures.join("payload.bin")).expect("read payload fixture");
        let profile = chunk_profile_for_manifest(&manifest).expect("chunk profile");
        let plan = CarBuildPlan::single_file_with_profile(&payload, profile).expect("build plan");
        let mut car = Vec::new();
        CarWriter::new(&plan, &payload)
            .expect("CAR writer")
            .write_to(&mut car)
            .expect("write CAR");

        validate_gateway_car_archive(&manifest, &car).expect("fixture CAR must match manifest");

        let mut size_drift = manifest.clone();
        size_drift.car_size = size_drift
            .car_size
            .checked_add(1)
            .expect("fixture CAR size range");
        let error = validate_gateway_car_archive(&size_drift, &car)
            .expect_err("CAR size drift must fail closed");
        assert!(
            error
                .to_string()
                .contains("does not match manifest car_size")
        );

        let mut digest_drift = manifest;
        digest_drift.car_digest[0] ^= 0x80;
        let error = validate_gateway_car_archive(&digest_drift, &car)
            .expect_err("CAR digest drift must fail closed");
        assert!(
            error
                .to_string()
                .contains("does not match manifest car_digest")
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
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let signing_key = write_signing_key_hex();
        let config = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
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

    #[test]
    fn dataset_loader_rejects_zero_provider_and_noncanonical_digest_before_storage() {
        let node = NodeHandle::new(StorageConfig::builder().build());
        let err = GatewayDataset::load_from_storage_with_provider(
            &node,
            &hex::encode([0x11; 32]),
            [0; 32],
        )
        .expect_err("zero provider must fail");
        assert!(err.to_string().contains("must not be the all-zero"));

        let err = GatewayDataset::load_from_storage_with_provider(
            &node,
            &hex::encode_upper([0xAB; 32]),
            [0x22; 32],
        )
        .expect_err("uppercase digest must fail");
        assert!(err.to_string().contains("canonical 32-byte lowercase hex"));
    }

    fn sample_stream_token_payload() -> StreamTokenPayload {
        let issued_at = unix_now().expect("system time");
        StreamTokenPayload {
            version: STREAM_TOKEN_VERSION,
            token_id: hex::encode([0x41; 32]),
            nonce_hex: hex::encode([0x42; 32]),
            manifest_digest_hex: hex::encode([0x43; 32]),
            provider_id_hex: hex::encode([0x44; 32]),
            profile_handle: "sorafs.sf1@1.0.0".to_owned(),
            max_streams: 4,
            ttl_epoch: issued_at + 60,
            rate_limit_bytes: 1024,
            issued_at,
            requests_per_minute: 30,
            client_id: "client-alpha".to_owned(),
            capabilities: vec![SUPPORTED_CAPABILITIES[0].to_owned()],
        }
    }

    fn signing_fixture() -> (PrivateKey, [u8; 32]) {
        let private_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x22; 32]).expect("private key");
        let public_key = gateway_signing_public_key(&private_key).expect("public key");
        (private_key, public_key)
    }

    fn rewrite_signed_token(encoded: &str, mutate: impl FnOnce(&mut json::Map)) -> String {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("decode token");
        let mut value: Value = json::from_slice(&decoded).expect("parse token");
        mutate(value.as_object_mut().expect("token object"));
        base64::engine::general_purpose::STANDARD
            .encode(json::to_vec(&value).expect("encode token"))
    }

    #[test]
    fn signed_stream_token_roundtrip_verifies_domain_and_key() {
        let payload = sample_stream_token_payload();
        let (private_key, public_key) = signing_fixture();
        let signed = encode_stream_token(&payload, &private_key, public_key).expect("sign token");
        assert_ne!(signed.signature_hex, "00".repeat(64));
        assert_eq!(signed.public_key_hex, hex::encode(public_key));

        let header = HeaderValue::from_str(&signed.encoded).expect("token header");
        let decoded = decode_stream_token_header(&header, public_key).expect("verify token");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn signed_stream_token_rejects_tampering_wrong_key_and_unknown_fields() {
        let payload = sample_stream_token_payload();
        let (private_key, public_key) = signing_fixture();
        let signed = encode_stream_token(&payload, &private_key, public_key).expect("sign token");

        let tampered = rewrite_signed_token(&signed.encoded, |map| {
            map.get_mut("payload")
                .and_then(Value::as_object_mut)
                .expect("payload object")
                .insert("client_id".into(), Value::from("client-evil"));
        });
        let err = decode_stream_token_header(
            &HeaderValue::from_str(&tampered).expect("header"),
            public_key,
        )
        .expect_err("tampered token must fail");
        assert_eq!(err.error_code(), "admission_mismatch");

        let (_, wrong_public_key) = {
            let key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x33; 32])
                .expect("alternate private key");
            let public = gateway_signing_public_key(&key).expect("alternate public key");
            (key, public)
        };
        let err = decode_stream_token_header(
            &HeaderValue::from_str(&signed.encoded).expect("header"),
            wrong_public_key,
        )
        .expect_err("wrong gateway key must fail");
        assert_eq!(err.error_code(), "admission_mismatch");

        let unknown = rewrite_signed_token(&signed.encoded, |map| {
            map.insert("unknown".into(), Value::from(true));
        });
        let err = decode_stream_token_header(
            &HeaderValue::from_str(&unknown).expect("header"),
            public_key,
        )
        .expect_err("unknown fields must fail");
        assert_eq!(err.error_code(), "admission_mismatch");
    }

    #[test]
    fn signed_stream_token_rejects_zero_and_noncanonical_crypto_material() {
        let payload = sample_stream_token_payload();
        let (private_key, public_key) = signing_fixture();
        let signed = encode_stream_token(&payload, &private_key, public_key).expect("sign token");

        for (field, replacement) in [
            ("public_key_hex", "00".repeat(32)),
            ("public_key_hex", hex::encode(SMALL_ORDER_R)),
            ("public_key_hex", hex::encode(NONCANONICAL_R)),
            ("signature_hex", "00".repeat(64)),
            ("signature_hex", "AA".repeat(64)),
        ] {
            let malformed = rewrite_signed_token(&signed.encoded, |map| {
                map.insert(field.into(), Value::from(replacement));
            });
            decode_stream_token_header(
                &HeaderValue::from_str(&malformed).expect("header"),
                public_key,
            )
            .expect_err("malformed crypto material must fail");
        }

        for replacement_r in [SMALL_ORDER_R, NONCANONICAL_R] {
            let mut malformed_signature = hex::decode(&signed.signature_hex).expect("signature");
            malformed_signature[..32].copy_from_slice(&replacement_r);
            let malformed = rewrite_signed_token(&signed.encoded, |map| {
                map.insert(
                    "signature_hex".into(),
                    Value::from(hex::encode(malformed_signature)),
                );
            });
            decode_stream_token_header(
                &HeaderValue::from_str(&malformed).expect("header"),
                public_key,
            )
            .expect_err("noncanonical signature R must fail");
        }
    }

    #[test]
    fn signed_stream_token_rejects_noncanonical_base64_and_json() {
        let payload = sample_stream_token_payload();
        let (private_key, public_key) = signing_fixture();
        let signed = encode_stream_token(&payload, &private_key, public_key).expect("sign token");

        let spaced_header = HeaderValue::from_str(&format!(" {}", signed.encoded))
            .expect("ASCII whitespace header");
        decode_stream_token_header(&spaced_header, public_key)
            .expect_err("surrounding whitespace must fail");

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&signed.encoded)
            .expect("decode token");
        let mut noncanonical_json = b" \n".to_vec();
        noncanonical_json.extend_from_slice(&decoded);
        let encoded = base64::engine::general_purpose::STANDARD.encode(noncanonical_json);
        decode_stream_token_header(
            &HeaderValue::from_str(&encoded).expect("header"),
            public_key,
        )
        .expect_err("noncanonical JSON must fail");

        let parsed: Value = json::from_slice(&decoded).expect("parse token");
        let duplicate_signature = parsed["signature_hex"].as_str().expect("signature");
        let mut duplicate_json = String::from_utf8(decoded).expect("UTF-8 token");
        assert_eq!(duplicate_json.pop(), Some('}'));
        duplicate_json.push_str(&format!(",\"signature_hex\":\"{duplicate_signature}\"}}"));
        let encoded = base64::engine::general_purpose::STANDARD.encode(duplicate_json);
        decode_stream_token_header(
            &HeaderValue::from_str(&encoded).expect("header"),
            public_key,
        )
        .expect_err("duplicate JSON fields must fail");
    }

    #[test]
    fn signed_stream_token_rejects_expired_and_future_timestamps() {
        let (private_key, public_key) = signing_fixture();
        let now = unix_now().expect("system time");

        let mut expired = sample_stream_token_payload();
        expired.issued_at = now.saturating_sub(120);
        expired.ttl_epoch = now.saturating_sub(60);
        let signed = encode_stream_token(&expired, &private_key, public_key).expect("sign expired");
        let err = decode_stream_token_header(
            &HeaderValue::from_str(&signed.encoded).expect("header"),
            public_key,
        )
        .expect_err("expired token must fail");
        assert_eq!(err.error_code(), "stream_token_expired");

        let mut future = sample_stream_token_payload();
        future.issued_at = now + 60;
        future.ttl_epoch = now + 120;
        let signed = encode_stream_token(&future, &private_key, public_key).expect("sign future");
        let err = decode_stream_token_header(
            &HeaderValue::from_str(&signed.encoded).expect("header"),
            public_key,
        )
        .expect_err("future token must fail");
        assert_eq!(err.error_code(), "admission_mismatch");
    }

    #[test]
    fn token_policy_rejects_every_unbounded_form() {
        for policy in [
            TokenPolicy {
                ttl_secs: 0,
                ..TokenPolicy::default()
            },
            TokenPolicy {
                max_streams: 0,
                ..TokenPolicy::default()
            },
            TokenPolicy {
                rate_limit_bytes: 0,
                ..TokenPolicy::default()
            },
            TokenPolicy {
                requests_per_minute: None,
                ..TokenPolicy::default()
            },
            TokenPolicy {
                requests_per_minute: Some(0),
                ..TokenPolicy::default()
            },
            TokenPolicy {
                max_issued_tokens: 0,
                ..TokenPolicy::default()
            },
            TokenPolicy {
                max_quota_clients: 0,
                ..TokenPolicy::default()
            },
        ] {
            policy.validate().expect_err("unbounded policy must fail");
        }
    }

    #[test]
    fn client_ids_and_capability_lists_require_canonical_bounded_forms() {
        for invalid in [
            "",
            " client",
            "client ",
            "Client",
            "-client",
            "client-",
            "client/control",
            "client\ncontrol",
        ] {
            validate_client_id(invalid).expect_err("invalid client id must fail");
        }
        validate_client_id("client-01:edge@example.test").expect("canonical client id");
        assert!(is_canonical_alias("docs.sora"));
        assert!(!is_canonical_alias("docs.sora;cid=forged"));
        assert!(is_canonical_host("gateway.example:443"));
        assert!(!is_canonical_host("gateway.example;cid=forged"));
        assert!(is_canonical_host_pattern("*.gateway.example"));
        assert!(!is_canonical_host_pattern("*;cid=forged"));

        for invalid in [
            Vec::<String>::new(),
            vec![SUPPORTED_CAPABILITIES[0].to_owned(); 2],
            vec![format!(" {}", SUPPORTED_CAPABILITIES[0])],
            vec!["x".repeat(MAX_CAPABILITY_BYTES + 1)],
        ] {
            validate_capabilities(&invalid).expect_err("invalid capabilities must fail");
        }
    }

    #[test]
    fn token_registry_bounds_tokens_clients_and_exact_payloads() {
        let dataset = fixture_dataset();
        let policy = TokenPolicy {
            requests_per_minute: Some(20),
            max_issued_tokens: 1,
            max_quota_clients: 1,
            ..TokenPolicy::default()
        };
        policy.validate().expect("policy");
        let registry = TokenRegistry::new(
            policy.clone(),
            dataset.signing_key.clone(),
            dataset.signing_public_key,
        );
        let capabilities = vec![SUPPORTED_CAPABILITIES[0].to_owned()];
        let issued = registry
            .issue_token(&dataset, "client-alpha", &capabilities)
            .expect("first token");

        assert!(matches!(
            registry.issue_token(&dataset, "client-alpha", &capabilities),
            Err(TokenIssueError::RegistryCapacity {
                registry: "issued token"
            })
        ));
        assert!(matches!(
            registry.issue_token(&dataset, "client-beta", &capabilities),
            Err(TokenIssueError::RegistryCapacity {
                registry: "client quota"
            })
        ));

        let header = HeaderValue::from_str(&issued.encoded).expect("header");
        let fresh_registry = TokenRegistry::new(
            policy,
            dataset.signing_key.clone(),
            dataset.signing_public_key,
        );
        let err = fresh_registry
            .acquire(&header, &dataset, Some("client-alpha"))
            .expect_err("valid token absent from the local replay registry must fail");
        assert_eq!(err.error_code(), "admission_mismatch");

        let mut payload = decode_stream_token_header(&header, dataset.signing_public_key)
            .expect("decode issued token");

        for (mutated, expected_code) in {
            let mut wrong_manifest = payload.clone();
            wrong_manifest.manifest_digest_hex = hex::encode([0x71; 32]);
            let mut wrong_provider = payload.clone();
            wrong_provider.provider_id_hex = hex::encode([0x72; 32]);
            let mut wrong_profile = payload.clone();
            wrong_profile.profile_handle = "sorafs.sf1@9.9.9".to_owned();
            let mut wrong_capability = payload.clone();
            wrong_capability.capabilities = vec!["sorafs.unknown".to_owned()];
            [
                (wrong_manifest, "admission_mismatch"),
                (wrong_provider, "provider_mismatch"),
                (wrong_profile, "unsupported_chunker"),
                (wrong_capability, "unsupported_capability"),
            ]
        } {
            let forged =
                encode_stream_token(&mutated, &dataset.signing_key, dataset.signing_public_key)
                    .expect("sign mutated binding");
            let err = registry
                .acquire(
                    &HeaderValue::from_str(&forged.encoded).expect("header"),
                    &dataset,
                    Some("client-alpha"),
                )
                .expect_err("wrong token binding must fail");
            assert_eq!(err.error_code(), expected_code);
        }

        payload.rate_limit_bytes = payload.rate_limit_bytes.saturating_sub(1);
        let forged =
            encode_stream_token(&payload, &dataset.signing_key, dataset.signing_public_key)
                .expect("sign altered payload with fixture key");
        let err = registry
            .acquire(
                &HeaderValue::from_str(&forged.encoded).expect("header"),
                &dataset,
                Some("client-alpha"),
            )
            .expect_err("validly signed but non-issued payload must fail exact comparison");
        assert_eq!(err.error_code(), "stream_token_payload_mismatch");
    }

    #[test]
    fn concurrent_token_issuance_uses_unique_random_ids_and_stays_bounded() {
        let dataset = Arc::new(fixture_dataset());
        let policy = TokenPolicy {
            requests_per_minute: Some(32),
            max_issued_tokens: 16,
            max_quota_clients: 1,
            ..TokenPolicy::default()
        };
        let registry = Arc::new(TokenRegistry::new(
            policy,
            dataset.signing_key.clone(),
            dataset.signing_public_key,
        ));
        let mut workers = Vec::new();
        for _ in 0..16 {
            let dataset = Arc::clone(&dataset);
            let registry = Arc::clone(&registry);
            workers.push(std::thread::spawn(move || {
                registry
                    .issue_token(
                        &dataset,
                        "client-alpha",
                        &[SUPPORTED_CAPABILITIES[0].to_owned()],
                    )
                    .expect("concurrent issuance")
                    .payload
                    .token_id
            }));
        }
        let mut ids = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker"))
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 16, "random token identifiers must be unique");
        assert_eq!(registry.tokens.lock().expect("tokens").len(), 16);
    }

    #[test]
    fn bounded_buffer_refuses_growth_past_limit() {
        let mut buffer = BoundedBuffer::new(4);
        buffer.write_all(&[1, 2, 3, 4]).expect("within bound");
        buffer
            .write_all(&[5])
            .expect_err("growth past hard bound must fail");
        assert_eq!(buffer.into_inner(), vec![1, 2, 3, 4]);
    }
}

fn require_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<&'a HeaderValue, GatewayResponseError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .ok_or_else(|| GatewayResponseError::missing_header(name))?;
    if values.next().is_some() {
        return Err(GatewayResponseError::capability_refusal(
            StatusCode::PRECONDITION_FAILED,
            "duplicate_header",
            format!("header `{name}` must appear exactly once"),
        ));
    }
    Ok(value)
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

fn validate_manifest_envelope_structure(
    dataset: &GatewayDataset,
    header: &HeaderValue,
) -> Result<(), GatewayResponseError> {
    let raw = header
        .to_str()
        .map_err(|_| GatewayResponseError::manifest_envelope("header must be valid UTF-8"))?;
    if raw.is_empty() || raw.len() > MAX_MANIFEST_ENVELOPE_HEADER_BYTES || raw != raw.trim() {
        return Err(GatewayResponseError::manifest_envelope(
            "header must be non-empty canonical base64 within the size limit",
        ));
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw.as_bytes())
        .map_err(|err| {
            GatewayResponseError::manifest_envelope(format!("invalid base64 payload: {err}"))
        })?;
    if decoded.is_empty()
        || decoded.len() > MAX_MANIFEST_ENVELOPE_DECODED_BYTES
        || base64::engine::general_purpose::STANDARD.encode(&decoded) != raw
    {
        return Err(GatewayResponseError::manifest_envelope(
            "decoded payload must be non-empty canonical base64 within the size limit",
        ));
    }
    let envelope: Value = norito::json::from_slice(&decoded).map_err(|err| {
        GatewayResponseError::manifest_envelope(format!("payload is not valid JSON: {err}"))
    })?;
    let envelope_map = envelope
        .as_object()
        .ok_or_else(|| GatewayResponseError::manifest_envelope("payload must be a JSON object"))?;
    let canonical = norito::json::to_vec(&envelope)
        .map_err(|err| GatewayResponseError::manifest_envelope(err.to_string()))?;
    if canonical != decoded {
        return Err(GatewayResponseError::manifest_envelope(
            "payload must use canonical Norito JSON bytes",
        ));
    }
    require_envelope_exact_keys(
        envelope_map,
        &[
            "admission",
            "chunking_profile",
            "gar",
            "manifest_digest_hex",
            "provider_id_hex",
        ],
    )?;

    let manifest_digest_hex = require_envelope_string(envelope_map, "manifest_digest_hex")?;
    require_canonical_envelope_hex(manifest_digest_hex, 32, "manifest_digest_hex")?;
    if manifest_digest_hex != dataset.manifest_id_hex() {
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
    if chunking_profile != dataset.chunker_alias() {
        return Err(GatewayResponseError::manifest_envelope(
            "chunking_profile does not match manifest profile",
        ));
    }

    let provider_id_hex = require_envelope_string(envelope_map, "provider_id_hex")?;
    require_canonical_envelope_hex(provider_id_hex, 32, "provider_id_hex")?;
    let expected_provider_hex = dataset.provider_id_hex();
    if provider_id_hex != expected_provider_hex {
        return Err(GatewayResponseError::manifest_envelope(
            "provider_id_hex does not match gateway provider",
        ));
    }

    let gar_map = require_envelope_object(envelope_map, "gar")?;
    require_envelope_exact_keys(
        gar_map,
        &["chunking_profile", "host_patterns", "manifest_id_hex"],
    )?;
    let gar_manifest_hex = require_envelope_string(gar_map, "manifest_id_hex")?;
    require_canonical_envelope_hex(gar_manifest_hex, 32, "gar.manifest_id_hex")?;
    if gar_manifest_hex != dataset.manifest_id_hex() {
        return Err(GatewayResponseError::manifest_envelope(
            "gar.manifest_id_hex does not match manifest digest",
        ));
    }
    let gar_chunker = require_envelope_string(gar_map, "chunking_profile")?;
    if gar_chunker != dataset.chunker_alias() {
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
        || host_patterns.len() > 32
        || !host_patterns.iter().all(|value| {
            value.as_str().is_some_and(|entry| {
                !entry.is_empty()
                    && entry.len() <= MAX_HOST_BYTES
                    && entry == entry.trim()
                    && is_canonical_host_pattern(entry)
            })
        })
    {
        return Err(GatewayResponseError::manifest_envelope(
            "gar.host_patterns must contain non-empty strings",
        ));
    }

    let admission_map = require_envelope_object(envelope_map, "admission")?;
    require_envelope_exact_keys(
        admission_map,
        &[
            "manifest_digest_hex",
            "profile_version",
            "provider_id_hex",
            "signature",
        ],
    )?;
    let admission_manifest_hex = require_envelope_string(admission_map, "manifest_digest_hex")?;
    require_canonical_envelope_hex(admission_manifest_hex, 32, "admission.manifest_digest_hex")?;
    if admission_manifest_hex != dataset.manifest_id_hex() {
        return Err(GatewayResponseError::manifest_envelope(
            "admission.manifest_digest_hex does not match manifest digest",
        ));
    }
    let admission_provider_hex = require_envelope_string(admission_map, "provider_id_hex")?;
    require_canonical_envelope_hex(admission_provider_hex, 32, "admission.provider_id_hex")?;
    if admission_provider_hex != provider_id_hex {
        return Err(GatewayResponseError::manifest_envelope(
            "admission.provider_id_hex does not match provider",
        ));
    }
    let profile_version = require_envelope_string(admission_map, "profile_version")?;
    if profile_version != dataset.profile_version() {
        return Err(GatewayResponseError::manifest_envelope(
            "admission.profile_version does not match gateway profile",
        ));
    }
    let signature = require_envelope_string(admission_map, "signature")?;
    if signature.len() > 4_096 {
        return Err(GatewayResponseError::manifest_envelope(
            "admission.signature exceeds the bounded envelope field size",
        ));
    }

    Ok(())
}

fn require_envelope_string<'a>(
    map: &'a json::Map,
    key: &str,
) -> Result<&'a str, GatewayResponseError> {
    if let Some(Value::String(raw)) = map.get(key) {
        if !raw.is_empty() && raw == raw.trim() {
            return Ok(raw);
        }
    }
    Err(GatewayResponseError::manifest_envelope(format!(
        "field `{key}` must be a non-empty string"
    )))
}

fn require_envelope_exact_keys(
    map: &json::Map,
    expected: &[&str],
) -> Result<(), GatewayResponseError> {
    if map.len() != expected.len() || expected.iter().any(|key| !map.contains_key(*key)) {
        return Err(GatewayResponseError::manifest_envelope(
            "envelope contains missing or unknown fields",
        ));
    }
    Ok(())
}

fn require_canonical_envelope_hex(
    value: &str,
    expected_bytes: usize,
    field: &str,
) -> Result<(), GatewayResponseError> {
    if value.len() != expected_bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || hex::decode(value)
            .ok()
            .is_none_or(|bytes| bytes.iter().all(|byte| *byte == 0))
    {
        return Err(GatewayResponseError::manifest_envelope(format!(
            "field `{field}` must be canonical non-zero lowercase hex"
        )));
    }
    Ok(())
}

fn is_canonical_host_pattern(value: &str) -> bool {
    value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b':' | b'[' | b']' | b'*')
        })
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'[' | b'*'))
        && value
            .as_bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b']')
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
            GatewayResponseError::Internal(err) => {
                warn!(
                    target: "sorafs.gateway",
                    error = %err,
                    "gateway request failed internally"
                );
                json_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal gateway failure".to_owned(),
                )
            }
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
