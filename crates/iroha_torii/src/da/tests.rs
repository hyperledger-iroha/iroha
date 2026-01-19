//! DA ingest and persistence tests.

use core::convert::TryInto;
use std::{
    cell::Cell,
    fs,
    io::Write,
    num::NonZeroU32,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, LazyLock},
    time::Duration,
};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use flate2::{Compression as FlateCompression, write::GzEncoder};
use iroha_config::parameters::actual::{
    DaTaikaiAnchor, LaneConfig as ConfigLaneConfig, TelemetryProfile,
};
use iroha_core::{da::LaneEpoch, telemetry::Telemetry};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, SignatureOf};
use iroha_data_model::{
    Encode,
    account::AccountId,
    da::{
        commitment::{DaCommitmentBundle, KzgCommitment},
        ingest::DaStripeLayout,
        types::{BlobDigest, DaRentQuote, StorageTicketId},
    },
    name::Name,
    nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestAliasBinding, ManifestDigest},
    },
    taikai::{
        GuardDirectoryId, SegmentTimestamp, TaikaiAliasBinding, TaikaiAvailabilityClass,
        TaikaiCarPointer, TaikaiCidIndexKey, TaikaiEnvelopeIndexes, TaikaiEventId,
        TaikaiGuardPolicy, TaikaiRenditionId, TaikaiRenditionRouteV1, TaikaiRoutingManifestV1,
        TaikaiSegmentEnvelopeV1, TaikaiSegmentSigningBodyV1, TaikaiSegmentSigningManifestV1,
        TaikaiSegmentWindow, TaikaiStreamId, TaikaiTimeIndexKey,
    },
};
use iroha_telemetry::metrics::Metrics;
use iroha_test_samples::ALICE_ID;
use norito::{
    codec::Decode,
    from_bytes,
    json::{self, Value},
    to_bytes,
};
use reqwest::Url;
use sorafs_car::{CarBuildPlan, PersistedChunkRecord};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, CouncilSignature, ProfileId,
    pdp::{HashAlgorithmV1, PDP_COMMITMENT_VERSION_V1, PdpCommitmentV1},
    pin_registry::{
        AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
    },
};
use tempfile::tempdir;
use tokio::{fs as async_fs, sync::Mutex as AsyncMutex};

use crate::da::taikai;
use crate::da::taikai::taikai_ingest;
use crate::da::taikai::taikai_ingest::{AnchorSender, collect_pending_uploads, process_batch};
use crate::da::taikai::{
    TAIKAI_ANCHOR_REQUEST_PREFIX, TAIKAI_ANCHOR_REQUEST_SUFFIX, TAIKAI_ANCHOR_SENTINEL_PREFIX,
    TAIKAI_ANCHOR_SENTINEL_SUFFIX, TAIKAI_SPOOL_SUBDIR,
};
use crate::da::{DaReceiptLog, ReplayCursorStore};

use super::*;

#[test]
fn replay_cursor_temp_path_keeps_suffixes() {
    let base = Path::new("/var/lib/iroha/replay_cursors.norito.json");
    let tmp = persistence::replay_cursor_temp_path(base);
    assert_eq!(
        tmp,
        Path::new("/var/lib/iroha/replay_cursors.norito.json.tmp")
    );
}

#[test]
fn parse_storage_ticket_hex_validates_variants() {
    let valid = format!("0x{}", "aa".repeat(32));
    let parsed = parse_storage_ticket_hex(&valid).expect("valid ticket");
    assert_eq!(parsed.len(), 32);
    assert!(parse_storage_ticket_hex("").is_err());
    assert!(parse_storage_ticket_hex("zz").is_err());
    assert!(parse_storage_ticket_hex("ab").is_err(), "too short");
}

#[test]
fn load_manifest_from_spool_locates_ticket() {
    let dir = tempdir().expect("dir");
    let ticket = StorageTicketId::new([0x77; 32]);
    let ticket_hex = hex::encode(ticket.as_bytes());
    let file =
        format!("manifest-00000001-0000000000000001-0000000000000002-{ticket_hex}-deadbeef.norito");
    let path = dir.path().join(file);
    fs::write(&path, b"manifest-bytes").expect("manifest file");

    let bytes = persistence::load_manifest_from_spool(dir.path(), &ticket).expect("manifest bytes");
    assert_eq!(bytes, b"manifest-bytes");

    let missing = StorageTicketId::new([0x55; 32]);
    let err =
        persistence::load_manifest_from_spool(dir.path(), &missing).expect_err("missing ticket");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn load_pdp_commitment_from_spool_locates_ticket() {
    let dir = tempdir().expect("dir");
    let ticket = StorageTicketId::new([0x99; 32]);
    let ticket_hex = hex::encode(ticket.as_bytes());
    let file = format!(
        "pdp-commitment-00000001-0000000000000001-0000000000000002-{ticket_hex}-feedface.norito"
    );
    let path = dir.path().join(file);
    let commitment = sample_pdp_commitment_for_tests();
    let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
    fs::write(&path, &bytes).expect("commitment file");

    let loaded =
        persistence::load_pdp_commitment_from_spool(dir.path(), &ticket).expect("commitment");
    assert_eq!(loaded, bytes);

    let missing = StorageTicketId::new([0x55; 32]);
    let err = persistence::load_pdp_commitment_from_spool(dir.path(), &missing)
        .expect_err("missing commitment");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn pdp_commitment_header_value_matches_base64_payload() {
    let commitment = sample_pdp_commitment_for_tests();
    let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
    let header_value = pdp_commitment_header_value(&bytes).expect("header value");
    let expected = BASE64.encode(bytes);
    assert_eq!(header_value.to_str().expect("utf8 header"), expected);
}

fn taikai_metadata() -> ExtraMetadata {
    ExtraMetadata {
        items: vec![
            MetadataEntry::new(
                taikai::META_TAIKAI_EVENT_ID,
                b"global-keynote".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_STREAM_ID,
                b"stage-a".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_RENDITION_ID,
                b"1080p".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_KIND,
                b"video".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_CODEC,
                b"av1-main".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_BITRATE,
                b"8000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_RESOLUTION,
                b"1920x1080".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_SEGMENT_SEQUENCE,
                b"42".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_SEGMENT_START,
                b"3600000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_SEGMENT_DURATION,
                b"2000000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_WALLCLOCK_MS,
                b"1702560000000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_INGEST_LATENCY_MS,
                b"120".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_INGEST_NODE_ID,
                b"ingest-node-1".to_vec(),
                MetadataVisibility::Public,
            ),
        ],
    }
}

#[test]
fn taikai_availability_defaults_without_trm() {
    let metadata = taikai_metadata();
    let availability = taikai::taikai_availability_from_metadata(&metadata, None).expect("derive");
    assert!(availability.is_none());
}

#[test]
fn taikai_availability_uses_trm_payload() {
    let metadata = taikai_metadata();
    let mut manifest = sample_trm_manifest();
    manifest.renditions[0].availability_class = TaikaiAvailabilityClass::Warm;
    let bytes = to_bytes(&manifest).expect("encode trm");
    let availability = taikai::taikai_availability_from_metadata(&metadata, Some(&bytes))
        .expect("derive")
        .expect("class");
    assert_eq!(availability, TaikaiAvailabilityClass::Warm);
}

#[test]
fn taikai_ingest_tags_include_availability_and_cache_hint() {
    let mut metadata = taikai_metadata();
    let retention = RetentionPolicy {
        hot_retention_secs: 3_600,
        cold_retention_secs: 12 * 60 * 60,
        required_replicas: 4,
        storage_class: StorageClass::Warm,
        governance_tag: GovernanceTag::new("da.taikai.test"),
    };
    let payload_digest = BlobDigest::from_hash(blake3_hash(b"taikai payload bytes"));
    taikai::apply_taikai_ingest_tags(
        &mut metadata,
        Some(TaikaiAvailabilityClass::Cold),
        &retention,
        payload_digest,
        1024,
    )
    .expect("tagging succeeds");

    fn value_for(metadata: &ExtraMetadata, key: &str) -> String {
        let entry = metadata
            .items
            .iter()
            .find(|entry| entry.key == key)
            .unwrap_or_else(|| panic!("missing metadata entry `{key}`"));
        String::from_utf8(entry.value.clone()).expect("utf8 value")
    }

    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_AVAILABILITY_CLASS),
        "cold"
    );
    assert_eq!(value_for(&metadata, taikai::META_DA_PROOF_TIER), "warm");
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_REPLICAS),
        "4"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_STORAGE),
        "warm"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_HOT_SECS),
        "3600"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_COLD_SECS),
        "43200"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_DA_PDP_SAMPLE_WINDOW),
        "32"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_DA_POTR_SAMPLE_WINDOW),
        "32"
    );

    let cache_hint_entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == taikai::META_TAIKAI_CACHE_HINT)
        .expect("cache hint entry");
    let cache_hint: Value = json::from_slice(&cache_hint_entry.value).expect("cache hint json");
    let hint = cache_hint.as_object().expect("cache hint object");
    assert_eq!(
        hint.get("event").and_then(Value::as_str).expect("event id"),
        "global-keynote"
    );
    assert_eq!(
        hint.get("stream")
            .and_then(Value::as_str)
            .expect("stream id"),
        "stage-a"
    );
    assert_eq!(
        hint.get("rendition")
            .and_then(Value::as_str)
            .expect("rendition id"),
        "1080p"
    );
    assert_eq!(
        hint.get("sequence")
            .and_then(Value::as_u64)
            .expect("sequence"),
        42
    );
    assert_eq!(
        hint.get("payload_len")
            .and_then(Value::as_u64)
            .expect("payload_len"),
        1024
    );
    assert_eq!(
        hint.get("payload_blake3_hex")
            .and_then(Value::as_str)
            .expect("digest"),
        hex::encode(payload_digest.as_ref())
    );
}

fn taikai_manifest_fixture() -> (DaIngestRequest, ManifestArtifacts) {
    let mut request = sample_request();
    request.metadata = taikai_metadata();

    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let payload_digest = BlobDigest::from_hash(blake3_hash(canonical.as_slice()));

    let mut metadata = request.metadata.clone();
    taikai::apply_taikai_ingest_tags(
        &mut metadata,
        Some(TaikaiAvailabilityClass::Hot),
        &request.retention_policy,
        payload_digest,
        request.total_size,
    )
    .expect("tagging succeeds");

    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        0,
        &rent_policy,
    )
    .expect("manifest");
    (request, manifest)
}

#[test]
fn verify_manifest_rejects_cache_hint_mismatch() {
    let (request, manifest) = taikai_manifest_fixture();
    let mut tampered = manifest.manifest.clone();

    // Replace the cache hint digest with a mismatched value.
    let hint_entry = tampered
        .metadata
        .items
        .iter_mut()
        .find(|entry| entry.key == taikai::META_TAIKAI_CACHE_HINT)
        .expect("cache hint entry");
    let mut hint: Value = json::from_slice(&hint_entry.value).expect("decode cache hint");
    if let Value::Object(map) = &mut hint {
        map.insert(
            "payload_blake3_hex".into(),
            Value::from(hex::encode([0xCD; 32])),
        );
    } else {
        panic!("cache hint must be a JSON object");
    }
    hint_entry.value = json::to_vec(&hint).expect("encode cache hint");

    let err = verify_manifest_against_request(
        &request,
        &tampered,
        &request.retention_policy,
        &tampered.metadata,
        &tampered.chunks,
        manifest.blob_hash,
        manifest.chunk_root,
        &manifest.manifest.rent_quote,
    )
    .expect_err("cache hint digest mismatch must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn verify_manifest_rejects_missing_proof_tier() {
    let (request, manifest) = taikai_manifest_fixture();
    let mut tampered = manifest.manifest.clone();
    tampered
        .metadata
        .items
        .retain(|entry| entry.key != taikai::META_DA_PROOF_TIER);

    let err = verify_manifest_against_request(
        &request,
        &tampered,
        &request.retention_policy,
        &tampered.metadata,
        &tampered.chunks,
        manifest.blob_hash,
        manifest.chunk_root,
        &manifest.manifest.rent_quote,
    )
    .expect_err("missing proof tier must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

fn sample_pdp_commitment_for_tests() -> PdpCommitmentV1 {
    PdpCommitmentV1 {
        version: PDP_COMMITMENT_VERSION_V1,
        manifest_digest: [0x11; 32],
        chunk_profile: ChunkingProfileV1 {
            profile_id: ProfileId(0xAB),
            namespace: "inline".to_owned(),
            name: "inline".to_owned(),
            semver: "1.0.0".to_owned(),
            min_size: 64 * 1024,
            target_size: 64 * 1024,
            max_size: 64 * 1024,
            break_mask: 1,
            multihash_code: BLAKE3_256_MULTIHASH_CODE,
            aliases: vec!["inline.inline@1.0.0".to_owned()],
        },
        commitment_root_hot: [0x22; 32],
        commitment_root_segment: [0x33; 32],
        hash_algorithm: HashAlgorithmV1::Blake3_256,
        hot_tree_height: 6,
        segment_tree_height: 4,
        sample_window: 32,
        sealed_at: 1_707_300_000,
    }
}

fn encode_alias_proof_bytes(
    alias_namespace: &str,
    alias_name: &str,
    manifest_cid: &[u8],
    bound_epoch: u64,
    expiry_epoch: u64,
    generated_at_unix: u64,
    expires_at_hint: u64,
) -> Vec<u8> {
    let binding = AliasBindingV1 {
        alias: format!("{alias_namespace}/{alias_name}"),
        manifest_cid: manifest_cid.to_vec(),
        bound_at: bound_epoch,
        expiry_epoch,
    };
    let mut bundle = AliasProofBundleV1 {
        binding,
        registry_root: [0u8; 32],
        registry_height: 1,
        generated_at_unix,
        expires_at_unix: expires_at_hint.max(generated_at_unix + 1),
        merkle_path: Vec::new(),
        council_signatures: Vec::new(),
    };
    bundle.registry_root =
        alias_merkle_root(&bundle.binding, &bundle.merkle_path).expect("compute alias proof root");
    let digest = alias_proof_signature_digest(&bundle);
    let council_key =
        PrivateKey::from_bytes(Algorithm::Ed25519, &[0x33; 32]).expect("seeded council key");
    let keypair = KeyPair::from_private_key(council_key).expect("derive council keypair");
    let signature = iroha_crypto::Signature::new(keypair.private_key(), digest.as_ref());
    let (_, signer_bytes) = keypair.public_key().to_bytes();
    let signer: [u8; 32] = signer_bytes.try_into().expect("ed25519 pk length");
    bundle.council_signatures.push(CouncilSignature {
        signer,
        signature: signature.payload().to_vec(),
    });
    to_bytes(&bundle).expect("encode alias proof")
}

fn build_ssm_bytes(
    manifest_hash: BlobDigest,
    car_digest: BlobDigest,
    envelope_hash: BlobDigest,
    segment_sequence: u64,
    generated_at_unix: u64,
    expires_at_hint: u64,
) -> Vec<u8> {
    let alias_proof = encode_alias_proof_bytes(
        "sora",
        "docs",
        b"cid-placeholder",
        1,
        32,
        generated_at_unix,
        expires_at_hint,
    );
    let alias_binding = ManifestAliasBinding {
        name: "docs".into(),
        namespace: "sora".into(),
        proof: alias_proof,
    };
    let publisher = KeyPair::random();
    let publisher_account = ALICE_ID.clone();
    let body = TaikaiSegmentSigningBodyV1::new(
        1,
        envelope_hash,
        manifest_hash,
        car_digest,
        segment_sequence,
        publisher_account,
        publisher.public_key().clone(),
        generated_at_unix * 1_000,
        alias_binding,
        ExtraMetadata::default(),
    );
    let signature = SignatureOf::new(publisher.private_key(), &body);
    let manifest = TaikaiSegmentSigningManifestV1::new(body, signature);
    to_bytes(&manifest).expect("encode signing manifest")
}

fn sample_trm_manifest() -> TaikaiRoutingManifestV1 {
    let event_id = TaikaiEventId::new(Name::from_str("global-keynote").unwrap());
    let stream_id = TaikaiStreamId::new(Name::from_str("stage-a").unwrap());
    let rendition_id = TaikaiRenditionId::new(Name::from_str("1080p").unwrap());
    let route = TaikaiRenditionRouteV1 {
        rendition_id: rendition_id.clone(),
        latest_manifest_hash: BlobDigest::from_hash(blake3_hash(b"manifest")),
        latest_car: TaikaiCarPointer::new(
            "zbafyqra",
            BlobDigest::from_hash(blake3_hash(b"car")),
            131_072,
        ),
        availability_class: TaikaiAvailabilityClass::Hot,
        replication_targets: vec![ProviderId::new([0x22; 32])],
        soranet_circuit: GuardDirectoryId::new("soranet/demo"),
        ssm_range: TaikaiSegmentWindow::new(40, 64),
    };
    TaikaiRoutingManifestV1 {
        version: TaikaiRoutingManifestV1::VERSION,
        event_id,
        stream_id,
        segment_window: TaikaiSegmentWindow::new(40, 64),
        renditions: vec![route],
        alias_binding: TaikaiAliasBinding {
            name: "docs".to_owned(),
            namespace: "sora".to_owned(),
            proof: vec![0xAB, 0xCD],
        },
        guard_policy: TaikaiGuardPolicy::new(
            GuardDirectoryId::new("soranet/demo"),
            1,
            3,
            vec!["lane-a".to_owned()],
        ),
        metadata: ExtraMetadata::default(),
    }
}

fn sample_trm_bytes() -> Vec<u8> {
    to_bytes(&sample_trm_manifest()).expect("encode trm")
}

fn taikai_envelope_fixture() -> taikai_ingest::EnvelopeArtifacts {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("envelope")
}

fn sampling_manifest(stripes: u32) -> DaManifestV1 {
    let mut request = sample_request();
    request.chunk_size = 512;
    request.erasure_profile = ErasureProfile {
        data_shards: 4,
        parity_shards: 2,
        row_parity_stripes: 0,
        chunk_alignment: 2,
        fec_scheme: FecScheme::Rs12_10,
    };
    request.total_size = u64::from(request.chunk_size)
        .saturating_mul(u64::from(request.erasure_profile.data_shards))
        .saturating_mul(u64::from(stripes));
    request.payload = vec![0xA5; request.total_size as usize];

    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_800_000,
        &rent_policy,
    )
    .expect("resolve manifest for sampling")
    .manifest
}

fn sample_request() -> DaIngestRequest {
    let keypair = KeyPair::random();
    let payload = b"example".to_vec();

    DaIngestRequest {
        client_blob_id: BlobDigest::from_hash(blake3::hash(b"blob-id")),
        lane_id: LaneId::new(1),
        epoch: 5,
        sequence: 7,
        blob_class: BlobClass::TaikaiSegment,
        codec: BlobCodec::new("cmaf"),
        erasure_profile: ErasureProfile {
            data_shards: 8,
            parity_shards: 4,
            row_parity_stripes: 0,
            chunk_alignment: 2,
            fec_scheme: FecScheme::Rs12_10,
        },
        retention_policy: RetentionPolicy {
            hot_retention_secs: 3600,
            cold_retention_secs: 10 * 3600,
            required_replicas: 3,
            storage_class: StorageClass::Hot,
            governance_tag: GovernanceTag::new("baseline"),
        },
        chunk_size: 1 << 10,
        total_size: payload.len() as u64,
        compression: Compression::Identity,
        norito_manifest: None,
        payload,
        metadata: ExtraMetadata {
            items: vec![MetadataEntry::new(
                "content-type",
                b"video/cmaf".to_vec(),
                MetadataVisibility::Public,
            )],
        },
        submitter: keypair.public_key().clone(),
        signature: Signature::from_bytes(&[0u8; 64]),
    }
}

fn lane_config_with_scheme(lane_id: LaneId, scheme: DaProofScheme) -> ConfigLaneConfig {
    let metadata = ModelLaneConfig {
        id: lane_id,
        dataspace_id: DataSpaceId::new(u64::from(lane_id.as_u32())),
        alias: format!("lane-{}", lane_id.as_u32()),
        proof_scheme: scheme,
        ..ModelLaneConfig::default()
    };

    let catalog = LaneCatalog::new(
        NonZeroU32::new(lane_id.as_u32().saturating_add(1)).expect("lane count"),
        vec![metadata],
    )
    .expect("lane catalog");
    ConfigLaneConfig::from_catalog(&catalog)
}

#[test]
fn validate_request_accepts_well_formed_payload() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    assert!(validate_request(&request, canonical.len()).is_ok());
}

#[test]
fn validate_request_rejects_non_power_two_chunks() {
    let mut request = sample_request();
    request.chunk_size = 1_500;
    let canonical = normalize_payload(&request).expect("normalize payload");
    let err = match validate_request(&request, canonical.len()) {
        Ok(_) => panic!("expected validation to reject non power-of-two chunk size"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

fn fingerprint_for_request(request: &DaIngestRequest) -> ReplayFingerprint {
    let canonical = normalize_payload(request).expect("normalize payload");
    let chunk_store = build_chunk_store(request, canonical.as_slice());
    let rent_policy = DaRentPolicyV1::default();
    resolve_manifest(
        request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        0,
        &rent_policy,
    )
    .expect("manifest")
    .fingerprint
}

#[test]
fn fingerprint_changes_with_client_blob_id() {
    let request = sample_request();
    let mut other = request.clone();
    other.client_blob_id = BlobDigest::from_hash(blake3::hash(b"different"));
    assert_ne!(
        fingerprint_for_request(&request),
        fingerprint_for_request(&other)
    );
}

#[test]
fn fingerprint_ignores_manifest_storage_ticket_and_timestamp() {
    let mut request = sample_request();
    request.blob_class = BlobClass::NexusLaneSidecar;
    let canonical = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let rent_policy = DaRentPolicyV1::default();
    let baseline_manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        7,
        &rent_policy,
    )
    .expect("manifest");
    request.norito_manifest =
        Some(to_bytes(&baseline_manifest.manifest).expect("encode baseline manifest"));

    let baseline = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        7,
        &rent_policy,
    )
    .expect("manifest with provided bytes");

    let mut tampered = baseline.manifest.clone();
    tampered.storage_ticket = StorageTicketId::new([0xAB; 32]);
    tampered.issued_at_unix = 123_456;
    request.norito_manifest = Some(to_bytes(&tampered).expect("encode manifest"));

    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        7,
        &rent_policy,
    )
    .expect("manifest with provided bytes");

    assert_eq!(baseline.fingerprint, manifest.fingerprint);
    assert_eq!(manifest.manifest.issued_at_unix, 7);
}

#[test]
fn lane_proof_scheme_accepts_kzg_policy() {
    let lane_id = LaneId::new(3);
    let config = lane_config_with_scheme(lane_id, DaProofScheme::KzgBls12_381);

    let scheme = lane_proof_scheme(&config, lane_id).expect("kzg lane should resolve");
    assert_eq!(scheme, DaProofScheme::KzgBls12_381);
}

#[test]
fn taikai_envelope_generation_requires_metadata() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata = request.metadata.clone();
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        0,
        &rent_policy,
    )
    .expect("manifest");

    let err = match taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    ) {
        Ok(_) => panic!("missing metadata must error"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn taikai_envelope_generation_computes_pointers() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata = request.metadata.clone();
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");

    let artifacts = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("taikai envelope");

    let mut cursor = std::io::Cursor::new(&artifacts.envelope_bytes);
    let envelope: TaikaiSegmentEnvelopeV1 = Decode::decode(&mut cursor).expect("decode envelope");
    assert_eq!(
        envelope.event_id.as_name(),
        &Name::from_str("global-keynote").unwrap()
    );
    assert_eq!(envelope.segment_sequence, 42);
    assert_eq!(
        envelope.ingest.chunk_count,
        chunk_store.chunks().len() as u32
    );
    assert!(envelope.ingest.car.cid_multibase.starts_with('b'));

    let indexes: TaikaiEnvelopeIndexes =
        norito::json::from_slice(&artifacts.indexes_json).expect("decode indexes");
    assert_eq!(indexes.time_key.event_id, envelope.event_id);
    assert_eq!(
        indexes.cid_key.cid_multibase,
        envelope.ingest.car.cid_multibase
    );
}

#[test]
fn taikai_envelope_calls_chunking_observer() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata = request.metadata.clone();
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");

    let called = Cell::new(0u32);
    let observer = |_: Duration| {
        called.set(called.get() + 1);
    };

    taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        Some(&observer),
    )
    .expect("envelope");

    assert_eq!(called.get(), 1);
}

#[test]
fn taikai_artifacts_persist_idempotent() {
    let dir = tempdir().expect("tempdir");
    let lane_id = LaneId::new(3);
    let epoch = 7;
    let sequence = 11;
    let storage_ticket = StorageTicketId::new([0x11; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3::hash(b"fingerprint"));

    let envelope_path = taikai_ingest::persist_envelope(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"envelope",
    )
    .expect("persist envelope")
    .expect("path");
    assert!(envelope_path.exists());

    let index_path = taikai_ingest::persist_indexes(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"indexes",
    )
    .expect("persist indexes")
    .expect("path");
    assert!(index_path.exists());

    let trm_path = taikai_ingest::persist_trm(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"trm",
    )
    .expect("persist trm")
    .expect("path");
    assert!(trm_path.exists());

    let envelope_second = taikai_ingest::persist_envelope(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"other",
    )
    .expect("persist envelope second")
    .expect("path");
    assert_eq!(envelope_path, envelope_second);

    let trm_second = taikai_ingest::persist_trm(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"other-trm",
    )
    .expect("persist trm second")
    .expect("path");
    assert_eq!(trm_path, trm_second);
}

#[derive(Default)]
struct MockAnchorSender {
    calls: AsyncMutex<Vec<(Url, String, Option<String>)>>,
}

#[async_trait]
impl AnchorSender for MockAnchorSender {
    async fn send(
        &self,
        endpoint: &Url,
        body: String,
        api_token: Option<&str>,
    ) -> Result<(), reqwest::Error> {
        self.calls
            .lock()
            .await
            .push((endpoint.clone(), body, api_token.map(str::to_owned)));
        Ok(())
    }
}

#[tokio::test]
async fn taikai_anchor_processing_generates_payload_and_sentinel() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    async_fs::create_dir_all(&spool_dir)
        .await
        .expect("create spool");

    let base_id = "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let envelope_path = spool_dir.join(format!("taikai-envelope-{base_id}.norito"));
    async_fs::write(&envelope_path, b"envelope-bytes")
        .await
        .expect("write envelope");

    let indexes = TaikaiEnvelopeIndexes {
        time_key: TaikaiTimeIndexKey {
            event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
            rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").unwrap()),
            segment_start_pts: SegmentTimestamp::new(3_600_000),
        },
        cid_key: TaikaiCidIndexKey {
            event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
            rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").unwrap()),
            cid_multibase: "zbafyqra".to_string(),
        },
    };
    let indexes_json = norito::json::to_json_pretty(&indexes).expect("indexes json");
    let indexes_path = spool_dir.join(format!("taikai-indexes-{base_id}.json"));
    async_fs::write(&indexes_path, indexes_json.as_bytes())
        .await
        .expect("write indexes");

    let ssm_path = spool_dir.join(format!("taikai-ssm-{base_id}.norito"));
    async_fs::write(&ssm_path, b"ssm-bytes")
        .await
        .expect("write ssm");
    let trm_bytes = sample_trm_bytes();
    let trm_path = spool_dir.join(format!("taikai-trm-{base_id}.norito"));
    async_fs::write(&trm_path, &trm_bytes)
        .await
        .expect("write trm");
    let mut lineage_hint = Map::new();
    lineage_hint.insert("version".into(), Value::from(1));
    lineage_hint.insert("alias_namespace".into(), Value::from("sora"));
    lineage_hint.insert("alias_name".into(), Value::from("docs"));
    lineage_hint.insert(
        "previous_manifest_digest_hex".into(),
        Value::from("cafebabe"),
    );
    lineage_hint.insert("previous_window_start_sequence".into(), Value::from(1));
    lineage_hint.insert("previous_window_end_sequence".into(), Value::from(120));
    lineage_hint.insert("previous_updated_unix".into(), Value::from(1_234_567));
    let lineage_value = Value::Object(lineage_hint.clone());
    let lineage_path = spool_dir.join(format!("taikai-lineage-{base_id}.json"));
    async_fs::write(
        &lineage_path,
        json::to_string(&lineage_value)
            .expect("lineage json")
            .as_bytes(),
    )
    .await
    .expect("write lineage hint");

    let anchor_cfg = DaTaikaiAnchor {
        endpoint: Url::parse("http://localhost/anchor").unwrap(),
        api_token: Some("secret-token".to_string()),
        poll_interval: Duration::from_secs(5),
    };

    let pending = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].base_id(), base_id);
    let payload: Value = norito::json::from_str(pending[0].body()).expect("payload json");
    assert_eq!(
        payload.get("envelope_base64").and_then(Value::as_str),
        Some(BASE64.encode(b"envelope-bytes")).as_deref()
    );
    assert_eq!(
        payload.get("ssm_base64").and_then(Value::as_str),
        Some(BASE64.encode(b"ssm-bytes")).as_deref()
    );
    assert_eq!(
        payload.get("trm_base64").and_then(Value::as_str),
        Some(BASE64.encode(&trm_bytes)).as_deref()
    );
    assert_eq!(payload.get("lineage_hint"), Some(&lineage_value));

    let sender = MockAnchorSender::default();
    process_batch(&spool_dir, &anchor_cfg, &sender)
        .await
        .expect("process batch");

    let calls = sender.calls.lock().await.clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, anchor_cfg.endpoint);
    assert_eq!(calls[0].2.as_deref(), anchor_cfg.api_token.as_deref());
    assert_eq!(calls[0].1, pending[0].body());

    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    assert!(async_fs::metadata(&sentinel).await.is_ok());
    let request_capture = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
    ));
    let capture_contents = async_fs::read_to_string(&request_capture)
        .await
        .expect("request capture");
    assert_eq!(capture_contents, pending[0].body());

    let pending_after = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect after upload");
    assert!(pending_after.is_empty());
}

#[test]
fn taikai_trm_lineage_guard_rejects_overlapping_windows() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 15);
    let alias = manifest.alias_binding.clone();
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&manifest, "deadbeef").expect("valid");
        guard
            .commit(manifest.segment_window, "deadbeef")
            .expect("commit");
    }

    let mut overlap = manifest.clone();
    overlap.segment_window = TaikaiSegmentWindow::new(10, 20);
    let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("guard")
        .expect("enabled");
    guard
        .validate(&overlap, "feedbead")
        .expect_err("must reject overlapping manifest windows");
}

#[test]
fn taikai_trm_lineage_hint_contains_previous_digest() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 8);
    let alias = manifest.alias_binding.clone();
    let lane_id = LaneId::new(7);
    let epoch = 42;
    let storage_ticket = StorageTicketId::new([0xAA; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3_hash(b"lineage-hint"));
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&manifest, "aaaa1111").expect("valid");
        guard
            .persist_lineage_hint(lane_id, epoch, 1, &storage_ticket, &fingerprint)
            .expect("persist hint");
        let base_id = format_base_id(lane_id, epoch, 1, &storage_ticket, &fingerprint);
        let hint_path = spool_dir
            .join(TAIKAI_SPOOL_SUBDIR)
            .join(format!("taikai-lineage-{base_id}.json"));
        let contents = fs::read_to_string(&hint_path).expect("lineage hint contents");
        let value: Value = json::from_str(&contents).expect("lineage value");
        assert!(
            value
                .get("previous_manifest_digest_hex")
                .is_some_and(Value::is_null)
        );
        guard
            .commit(manifest.segment_window, "aaaa1111")
            .expect("commit");
    }

    let mut next_manifest = manifest.clone();
    next_manifest.segment_window = TaikaiSegmentWindow::new(9, 16);
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&next_manifest, "bbbb2222").expect("valid");
        guard
            .persist_lineage_hint(lane_id, epoch, 2, &storage_ticket, &fingerprint)
            .expect("persist hint");
        let base_id = format_base_id(lane_id, epoch, 2, &storage_ticket, &fingerprint);
        let hint_path = spool_dir
            .join(TAIKAI_SPOOL_SUBDIR)
            .join(format!("taikai-lineage-{base_id}.json"));
        let contents = fs::read_to_string(&hint_path).expect("lineage hint contents");
        let value: Value = json::from_str(&contents).expect("lineage value");
        assert_eq!(
            value
                .get("previous_manifest_digest_hex")
                .and_then(Value::as_str),
            Some("aaaa1111")
        );
        guard
            .commit(next_manifest.segment_window, "bbbb2222")
            .expect("commit");
    }
}

#[test]
fn take_ssm_entry_returns_payload_and_strips_metadata() {
    let mut metadata = taikai_metadata();
    metadata.items.push(MetadataEntry::new(
        taikai::META_TAIKAI_SSM,
        vec![1, 2, 3],
        MetadataVisibility::Public,
    ));
    let payload = taikai_ingest::take_ssm_entry(&mut metadata)
        .expect("extract ssm")
        .expect("payload present");
    assert_eq!(payload, vec![1, 2, 3]);
    assert!(
        metadata
            .items
            .iter()
            .all(|entry| entry.key != taikai::META_TAIKAI_SSM)
    );
}

#[test]
fn take_trm_entry_returns_payload_and_strips_metadata() {
    let mut metadata = taikai_metadata();
    metadata.items.push(MetadataEntry::new(
        taikai::META_TAIKAI_TRM,
        vec![9, 8, 7],
        MetadataVisibility::Public,
    ));
    let payload = taikai_ingest::take_trm_entry(&mut metadata)
        .expect("extract trm")
        .expect("payload present");
    assert_eq!(payload, vec![9, 8, 7]);
    assert!(
        metadata
            .items
            .iter()
            .all(|entry| entry.key != taikai::META_TAIKAI_TRM)
    );
}

#[test]
fn validate_taikai_ssm_accepts_matching_payload() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let taikai = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("envelope");
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let alias_policy = crate::sorafs::AliasCachePolicy::new(
        Duration::from_secs(600),
        Duration::from_secs(60),
        Duration::from_secs(1_200),
        Duration::from_secs(60),
        Duration::from_secs(120),
        Duration::from_secs(10_000),
        Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let (_, telemetry) = telemetry_handle_for_tests();
    let outcome = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        &telemetry,
    )
    .expect("ssm valid");
    assert_eq!(outcome.alias_label, "sora/docs");
}

#[test]
fn validate_taikai_ssm_rejects_manifest_mismatch() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let taikai = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("envelope");
    let now_secs = crate::sorafs::unix_now_secs();
    let bad_ssm = build_ssm_bytes(
        BlobDigest::from_hash(blake3_hash(b"other-manifest")),
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let alias_policy = crate::sorafs::AliasCachePolicy::new(
        Duration::from_secs(600),
        Duration::from_secs(60),
        Duration::from_secs(1_200),
        Duration::from_secs(60),
        Duration::from_secs(120),
        Duration::from_secs(10_000),
        Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &bad_ssm,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        &telemetry,
    )
    .expect_err("manifest mismatch must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn validate_taikai_ssm_rejects_tampered_signature() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let taikai = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("envelope");
    let now_secs = crate::sorafs::unix_now_secs();
    let mut ssm_bytes = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    // Flip a byte in the signature payload to break verification.
    if let Some(last) = ssm_bytes.last_mut() {
        *last ^= 0xFF;
    }
    let alias_policy = crate::sorafs::AliasCachePolicy::new(
        Duration::from_secs(600),
        Duration::from_secs(60),
        Duration::from_secs(1_200),
        Duration::from_secs(60),
        Duration::from_secs(120),
        Duration::from_secs(10_000),
        Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        &telemetry,
    )
    .expect_err("tampered signature must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn validate_taikai_trm_accepts_matching_manifest() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let taikai = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("envelope");
    let routing_manifest =
        taikai::validate_taikai_trm(&sample_trm_bytes(), &taikai).expect("trm valid");
    assert_eq!(
        routing_manifest.alias_binding.name.as_str(),
        "docs",
        "alias binding should match the stream metadata"
    );
    assert_eq!(
        routing_manifest.segment_window.start_sequence, 40,
        "validated manifest should expose the expected window"
    );
}

#[test]
fn validate_taikai_trm_rejects_mismatched_event() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let taikai = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("envelope");
    let mut trm = sample_trm_manifest();
    trm.event_id = TaikaiEventId::new(Name::from_str("other-event").unwrap());
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai).expect_err("validation must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn validate_taikai_trm_rejects_invalid_version() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest();
    trm.version = TaikaiRoutingManifestV1::VERSION + 1;
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai).expect_err("validation must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("unsupported manifest version"),
        "unexpected error message: {}",
        err.1
    );
}

#[test]
fn validate_taikai_trm_rejects_invalid_window() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest();
    trm.segment_window = TaikaiSegmentWindow::new(50, 40);
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai).expect_err("validation must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("invalid routing manifest"),
        "unexpected error message: {}",
        err.1
    );
}

#[test]
fn normalize_payload_handles_gzip() {
    let mut request = sample_request();
    let canonical = request.payload.clone();
    let mut encoder = GzEncoder::new(Vec::new(), FlateCompression::default());
    encoder.write_all(&canonical).expect("write gzip payload");
    let compressed = encoder.finish().expect("finish gzip payload");
    request.payload = compressed;
    request.compression = Compression::Gzip;
    request.total_size = canonical.len() as u64;

    let normalized = normalize_payload(&request).expect("normalize gzip payload");
    assert_eq!(normalized.as_slice(), canonical.as_slice());
}

#[test]
fn normalize_payload_rejects_size_mismatch() {
    let mut request = sample_request();
    let canonical = request.payload.clone();
    let mut encoder = GzEncoder::new(Vec::new(), FlateCompression::default());
    encoder.write_all(&canonical).expect("write gzip payload");
    let compressed = encoder.finish().expect("finish gzip payload");
    request.payload = compressed;
    request.compression = Compression::Gzip;
    request.total_size = (canonical.len() as u64) + 1;

    let err = match normalize_payload(&request) {
        Ok(_) => panic!("expected normalization to reject mismatched size"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn build_receipt_includes_pdp_commitment() {
    let request = sample_request();
    let signer = KeyPair::random();
    let pdp_commitment = sample_pdp_commitment_for_tests();
    let encoded = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let rent_quote = DaRentQuote {
        base_rent: XorAmount::from_micro(111),
        protocol_reserve: XorAmount::from_micro(222),
        provider_reward: XorAmount::from_micro(333),
        pdp_bonus: XorAmount::from_micro(444),
        potr_bonus: XorAmount::from_micro(555),
        egress_credit_per_gib: XorAmount::from_micro(666),
    };
    let receipt = build_receipt(
        &signer,
        &request,
        123,
        BlobDigest::from_hash(blake3_hash(b"blob-hash")),
        BlobDigest::from_hash(blake3_hash(b"chunk-root")),
        BlobDigest::from_hash(blake3_hash(b"manifest-hash")),
        StorageTicketId::new([0x44; 32]),
        encoded.clone(),
        rent_quote,
        DaStripeLayout::default(),
    );
    assert_eq!(receipt.pdp_commitment, Some(encoded));
    assert_eq!(receipt.rent_quote, rent_quote);
}

#[test]
fn build_receipt_signs_with_operator_key() {
    let request = sample_request();
    let signer = KeyPair::random();
    let receipt = build_receipt(
        &signer,
        &request,
        999,
        BlobDigest::from_hash(blake3_hash(b"blob-hash")),
        BlobDigest::from_hash(blake3_hash(b"chunk-root")),
        BlobDigest::from_hash(blake3_hash(b"manifest-hash")),
        StorageTicketId::new([0xAA; 32]),
        Vec::new(),
        DaRentQuote::default(),
        DaStripeLayout::default(),
    );
    let mut unsigned = receipt.clone();
    unsigned.operator_signature = Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER);
    let unsigned_bytes = to_bytes(&unsigned).expect("encode unsigned receipt");
    receipt
        .operator_signature
        .verify(signer.public_key(), &unsigned_bytes)
        .expect("signature verifies");
}

#[test]
fn build_receipt_computes_chunk_root_from_payload() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_000,
        &rent_policy,
    )
    .expect("resolve manifest");
    let pdp_commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        1_701_000_000,
    )
    .expect("pdp commitment");
    let encoded_commitment =
        encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let signer = KeyPair::random();
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_000,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        encoded_commitment,
        manifest.manifest.rent_quote,
        stripe_layout,
    );
    assert_eq!(receipt.chunk_root, manifest.chunk_root);
    assert_eq!(
        manifest.chunk_root,
        BlobDigest::new(*chunk_store.por_tree().root())
    );
}

#[test]
fn build_receipt_prefers_chunk_root_from_manifest() {
    let mut request = sample_request();
    // Seed Taikai metadata so manifest validation passes gateway checks.
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let canonical_bytes = canonical.as_slice().to_vec();
    drop(canonical);
    let payload_hash = BlobDigest::from_hash(blake3_hash(&canonical_bytes));
    let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
    taikai::apply_taikai_ingest_tags(
        &mut request.metadata,
        None,
        &request.retention_policy,
        payload_hash.clone(),
        request.total_size,
    )
    .expect("apply taikai tags to metadata");
    let manifest_chunk_root = BlobDigest::new(*chunk_store.por_tree().root());
    let chunk_commitments =
        build_chunk_commitments(&request, &chunk_store, canonical_bytes.as_slice())
            .expect("expected chunk commitments");
    let ipa_commitment =
        ipa_commitment_from_chunks(&chunk_commitments).expect("ipa commitment from chunks");
    let total_stripes = (chunk_store.chunks().len() as u32)
        .div_ceil(u32::from(request.erasure_profile.data_shards));
    let shards_per_stripe = u32::from(request.erasure_profile.data_shards)
        .saturating_add(u32::from(request.erasure_profile.parity_shards));
    let total_stripes_full =
        total_stripes.saturating_add(u32::from(request.erasure_profile.row_parity_stripes));
    let rent_policy = DaRentPolicyV1::default();
    let (rent_gib, rent_months) =
        rent_usage_from_request(request.total_size, &request.retention_policy);
    let rent_quote = rent_policy
        .quote(rent_gib, rent_months)
        .expect("compute rent quote for manifest");
    let manifest = DaManifestV1 {
        version: DaManifestV1::VERSION,
        client_blob_id: request.client_blob_id.clone(),
        lane_id: request.lane_id,
        epoch: request.epoch,
        blob_class: request.blob_class,
        codec: request.codec.clone(),
        blob_hash: payload_hash,
        chunk_root: manifest_chunk_root.clone(),
        storage_ticket: StorageTicketId::new([0x55; 32]),
        total_size: request.total_size,
        chunk_size: request.chunk_size,
        total_stripes: total_stripes_full,
        shards_per_stripe,
        erasure_profile: request.erasure_profile,
        retention_policy: request.retention_policy.clone(),
        rent_quote,
        chunks: chunk_commitments,
        ipa_commitment,
        metadata: request.metadata.clone(),
        issued_at_unix: 42,
    };
    request.norito_manifest = Some(to_bytes(&manifest).expect("encode manifest"));

    let canonical = normalize_payload(&request).expect("normalize payload with manifest");
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_123,
        &rent_policy,
    )
    .expect("resolve provided manifest");
    let pdp_commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        1_701_000_123,
    )
    .expect("pdp commitment");
    let encoded_commitment =
        encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let signer = KeyPair::random();
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_123,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        encoded_commitment,
        manifest.manifest.rent_quote,
        stripe_layout,
    );
    assert_eq!(receipt.chunk_root, manifest_chunk_root);
}

#[test]
fn build_chunk_commitments_rejects_oversized_chunk_length() {
    let mut request = sample_request();
    request.chunk_size = 256;
    request.payload = vec![0xA5; 1024];
    request.total_size = request.payload.len() as u64;

    let canonical = normalize_payload(&request).expect("normalize payload");
    let oversized_profile = chunk_profile_for_request(1024);
    let mut chunk_store = ChunkStore::with_profile(oversized_profile);
    chunk_store.ingest_bytes(canonical.as_slice());

    let err = build_chunk_commitments(&request, &chunk_store, canonical.as_slice())
        .expect_err("oversized chunk length should be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("exceeds configured chunk_size"),
        "unexpected error message: {}",
        err.1
    );
}

#[test]
fn persist_manifest_for_sorafs_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_555,
        &rent_policy,
    )
    .expect("manifest");

    let first_path = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist manifest")
    .expect("spool path");
    let bytes = fs::read(&first_path).expect("read manifest file");
    assert_eq!(bytes, manifest.encoded);

    let second_path = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist manifest idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}

#[test]
fn persist_pdp_commitment_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_777,
        &rent_policy,
    )
    .expect("manifest");

    let commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        1_701_000_777,
    )
    .expect("commitment");

    let first_path = persistence::persist_pdp_commitment(
        manifest_dir,
        &commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist commitment")
    .expect("spool path");
    let bytes = fs::read(&first_path).expect("read commitment file");
    let archived = from_bytes::<PdpCommitmentV1>(&bytes).expect("decode commitment");
    let decoded = PdpCommitmentV1::deserialize(archived);
    assert_eq!(decoded, commitment);

    let second_path = persistence::persist_pdp_commitment(
        manifest_dir,
        &commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist commitment idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}

#[test]
fn build_da_commitment_record_reflects_artifacts() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_500_000,
        &rent_policy,
    )
    .expect("manifest");
    let pdp_commitment = sample_pdp_commitment_for_tests();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let receipt = build_receipt(
        &KeyPair::random(),
        &request,
        1_701_500_000,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote,
        stripe_layout,
    );
    let record = build_da_commitment_record(
        &request,
        &manifest,
        &request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::MerkleSha256,
    );
    assert_eq!(record.lane_id, request.lane_id);
    assert_eq!(record.epoch, request.epoch);
    assert_eq!(record.sequence, request.sequence);
    assert_eq!(record.client_blob_id, request.client_blob_id);
    assert_eq!(
        record.manifest_hash.as_bytes(),
        manifest.manifest_hash.as_bytes()
    );
    assert_eq!(record.retention_class, request.retention_policy);
    assert_eq!(record.storage_ticket, manifest.storage_ticket);
    assert!(record.proof_digest.is_some(), "expected proof digest");
    assert_eq!(record.proof_scheme, DaProofScheme::MerkleSha256);
    assert!(
        record.kzg_commitment.is_none(),
        "merkle lanes must not include KZG commitments"
    );
}

#[test]
fn build_da_commitment_record_sets_kzg_commitment_for_kzg_lane() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_500_000,
        &rent_policy,
    )
    .expect("manifest");
    let pdp_commitment = sample_pdp_commitment_for_tests();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let receipt = build_receipt(
        &KeyPair::random(),
        &request,
        1_701_500_000,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote,
        stripe_layout,
    );
    let record = build_da_commitment_record(
        &request,
        &manifest,
        &request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::KzgBls12_381,
    );
    let expected_kzg = derive_kzg_commitment(&manifest.chunk_root, &manifest.storage_ticket);
    assert_eq!(record.proof_scheme, DaProofScheme::KzgBls12_381);
    assert_eq!(record.kzg_commitment, Some(expected_kzg));
    assert!(record.proof_digest.is_some(), "expected proof digest");
}

#[test]
fn persist_da_commitment_record_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_600_000,
        &rent_policy,
    )
    .expect("manifest");
    let pdp_commitment = sample_pdp_commitment_for_tests();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let receipt = build_receipt(
        &KeyPair::random(),
        &request,
        1_701_600_000,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote,
        stripe_layout,
    );
    let record = build_da_commitment_record(
        &request,
        &manifest,
        &request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::MerkleSha256,
    );
    let first_path = persistence::persist_da_commitment_record(
        manifest_dir,
        &record,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist record")
    .expect("spool path");
    let bytes = fs::read(&first_path).expect("read record file");
    let archived = from_bytes::<DaCommitmentRecord>(&bytes).expect("decode record");
    let decoded = DaCommitmentRecord::deserialize(archived);
    assert_eq!(decoded, record);

    let second_path = persistence::persist_da_commitment_record(
        manifest_dir,
        &record,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist record idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}

#[test]
fn persist_da_commitment_schedule_entry_writes_bundle() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_600_000,
        &rent_policy,
    )
    .expect("manifest");
    let pdp_commitment = sample_pdp_commitment_for_tests();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let receipt = build_receipt(
        &KeyPair::random(),
        &request,
        1_701_600_000,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote,
        stripe_layout,
    );
    let record = build_da_commitment_record(
        &request,
        &manifest,
        &request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::MerkleSha256,
    );
    let schedule_path = persistence::persist_da_commitment_schedule_entry(
        manifest_dir,
        &record,
        &pdp_bytes,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist schedule entry")
    .expect("schedule path");
    let bytes = fs::read(&schedule_path).expect("read schedule entry");
    let archived = from_bytes::<persistence::DaCommitmentScheduleEntry>(&bytes)
        .expect("decode schedule entry");
    let decoded = persistence::DaCommitmentScheduleEntry::deserialize(archived);
    assert_eq!(decoded.record, record);
    assert_eq!(decoded.pdp_commitment, pdp_bytes);
}

#[test]
fn persist_da_pin_intent_writes_file() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let mut request = sample_request();
    request.sequence = 42;
    request.metadata.items.push(MetadataEntry::new(
        META_DA_REGISTRY_ALIAS,
        b"sora/docs".to_vec(),
        MetadataVisibility::Public,
    ));
    request.metadata.items.push(MetadataEntry::new(
        META_DA_REGISTRY_OWNER,
        ALICE_ID.to_string().into_bytes(),
        MetadataVisibility::Public,
    ));
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_700_123,
        &rent_policy,
    )
    .expect("manifest");
    let alias =
        registry_alias_from_metadata(&request.metadata).expect("alias metadata should parse");
    let owner =
        registry_owner_from_metadata(&request.metadata).expect("owner metadata should parse");
    let mut intent = DaPinIntent::new(
        request.lane_id,
        request.epoch,
        request.sequence,
        manifest.storage_ticket,
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
    );
    intent.alias = alias;
    intent.owner = owner;
    let path = persistence::persist_da_pin_intent(
        manifest_dir,
        &intent,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist pin")
    .expect("path");
    let bytes = fs::read(&path).expect("read pin intent");
    let archived = from_bytes::<DaPinIntent>(&bytes).expect("decode pin intent");
    let decoded: DaPinIntent =
        NoritoDeserialize::try_deserialize(archived).expect("deserialize pin intent");
    assert_eq!(decoded, intent);
    assert_eq!(decoded.alias, Some("sora/docs".to_owned()));
    assert_eq!(decoded.owner, Some(ALICE_ID.clone()));
}

fn test_receipt(signer: &KeyPair, lane_id: LaneId, epoch: u64, seed: u8) -> DaIngestReceipt {
    let mut receipt = DaIngestReceipt {
        client_blob_id: BlobDigest::new([seed; 32]),
        lane_id,
        epoch,
        blob_hash: BlobDigest::new([seed.wrapping_add(1); 32]),
        chunk_root: BlobDigest::new([seed.wrapping_add(2); 32]),
        manifest_hash: BlobDigest::new([seed.wrapping_add(3); 32]),
        storage_ticket: StorageTicketId::new([seed.wrapping_add(4); 32]),
        pdp_commitment: Some(vec![seed]),
        stripe_layout: DaStripeLayout::default(),
        queued_at_unix: 1234,
        rent_quote: DaRentQuote::default(),
        operator_signature: Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER),
    };
    let unsigned = persistence::unsigned_receipt_bytes(&receipt).expect("test receipt encodes");
    receipt.operator_signature = Signature::new(signer.private_key(), &unsigned);
    receipt
}

fn test_fingerprint(seed: u8) -> ReplayFingerprint {
    ReplayFingerprint::from_hash_bytes(&[seed; blake3::OUT_LEN])
}

#[test]
fn persist_da_receipt_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = KeyPair::random();
    let lane_id = LaneId::new(3);
    let receipt = test_receipt(&signer, lane_id, 5, 0xAA);
    let fingerprint = test_fingerprint(0xCC);

    let first_path = persistence::persist_da_receipt(manifest_dir, &receipt, 7, &fingerprint)
        .expect("persist receipt");
    let first_path = first_path.expect("receipt path");
    let bytes = fs::read(&first_path).expect("read receipt file");
    let decoded =
        decode_from_bytes::<persistence::StoredDaReceipt>(&bytes).expect("decode stored receipt");
    assert_eq!(decoded.version, persistence::STORED_RECEIPT_VERSION);
    assert_eq!(decoded.sequence, 7);
    assert_eq!(decoded.receipt.manifest_hash, receipt.manifest_hash);
    let loaded = persistence::load_da_receipts(manifest_dir).expect("load receipts");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].sequence, 7);
    assert_eq!(loaded[0].receipt.manifest_hash, receipt.manifest_hash);

    let second_path = persistence::persist_da_receipt(manifest_dir, &receipt, 7, &fingerprint)
        .expect("persist again");
    let second_path = second_path.expect("receipt path");
    assert_eq!(first_path, second_path);
}

#[test]
fn load_da_receipts_skips_unsupported_versions() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = KeyPair::random();
    let lane_id = LaneId::new(3);
    let receipt = test_receipt(&signer, lane_id, 5, 0xAB);
    let stored = persistence::StoredDaReceipt {
        version: persistence::STORED_RECEIPT_VERSION + 1,
        sequence: 7,
        receipt,
    };
    let bytes = to_bytes(&stored).expect("encode receipt");
    let path =
        manifest_dir.join("da-receipt-00000003-0000000000000005-0000000000000007-bad.norito");
    fs::write(&path, bytes).expect("write receipt");

    let loaded = persistence::load_da_receipts(manifest_dir).expect("load receipts");
    assert!(loaded.is_empty());
}

#[test]
fn da_receipt_log_enforces_ordering_and_dedupe() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 9);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = KeyPair::random();
    let log = DaReceiptLog::open(
        temp_dir.path().to_path_buf(),
        Arc::clone(&cursor_store),
        signer.public_key().clone(),
    )
    .unwrap();

    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1);
    assert!(matches!(
        log.append(lane_epoch, 1, receipt.clone(), test_fingerprint(1))
            .unwrap(),
        ReceiptInsertOutcome::Stored { .. }
    ));

    assert!(matches!(
        log.append(lane_epoch, 1, receipt.clone(), test_fingerprint(1))
            .unwrap(),
        ReceiptInsertOutcome::Duplicate { .. }
    ));

    let conflict = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 2);
    assert!(matches!(
        log.append(lane_epoch, 1, conflict, test_fingerprint(2))
            .unwrap(),
        ReceiptInsertOutcome::ManifestConflict { .. }
    ));

    let stale = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 3);
    assert!(matches!(
        log.append(lane_epoch, 0, stale, test_fingerprint(3))
            .unwrap(),
        ReceiptInsertOutcome::StaleSequence { highest: 1 }
    ));
}

#[test]
fn da_receipt_log_rejects_invalid_signature() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(5), 7);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = KeyPair::random();
    let log = DaReceiptLog::open(
        temp_dir.path().to_path_buf(),
        Arc::clone(&cursor_store),
        signer.public_key().clone(),
    )
    .expect("open log");

    let mut receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 4);
    let unsigned = persistence::unsigned_receipt_bytes(&receipt).expect("unsigned bytes");
    let wrong_signer = KeyPair::random();
    receipt.operator_signature = Signature::new(wrong_signer.private_key(), &unsigned);

    let outcome = log.append(lane_epoch, 1, receipt, test_fingerprint(4));
    assert!(
        outcome.is_err(),
        "receipt with mismatched signature must be rejected"
    );
}

#[test]
fn da_receipt_log_reloads_from_disk() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(5), 11);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = KeyPair::random();
    {
        let log = DaReceiptLog::open(
            temp_dir.path().to_path_buf(),
            Arc::clone(&cursor_store),
            signer.public_key().clone(),
        )
        .unwrap();
        let first = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 9);
        let second = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 10);
        log.append(lane_epoch, 1, first.clone(), test_fingerprint(9))
            .unwrap();
        log.append(lane_epoch, 2, second.clone(), test_fingerprint(10))
            .unwrap();
    }

    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let reopened = DaReceiptLog::open(
        temp_dir.path().to_path_buf(),
        Arc::clone(&cursor_store),
        signer.public_key().clone(),
    )
    .unwrap();
    let entries = reopened.receipts_for(lane_epoch);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].sequence, 1);
    assert_eq!(entries[1].sequence, 2);
    assert_eq!(
        entries[1].manifest_hash,
        BlobDigest::new([10u8.wrapping_add(3); 32])
    );
    assert!(
        cursor_store
            .highest_sequences()
            .iter()
            .any(|(key, seq)| *key == lane_epoch && *seq == 2),
        "cursor store should be seeded from disk"
    );
}

#[test]
fn da_receipt_log_skips_invalid_entries_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let bad_path = temp_dir
        .path()
        .join("da-receipt-00000001-0000000000000001-0000000000000001-bad.norito");
    fs::write(&bad_path, b"corrupt").expect("write corrupt receipt");

    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = KeyPair::random();
    let log = DaReceiptLog::open(
        temp_dir.path().to_path_buf(),
        Arc::clone(&cursor_store),
        signer.public_key().clone(),
    )
    .expect("open log");

    let lane_epoch = LaneEpoch::new(LaneId::new(1), 1);
    assert!(log.receipts_for(lane_epoch).is_empty());
}

#[test]
fn replay_cursor_store_persists_sequences() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().to_path_buf();
    let store = ReplayCursorStore::open(path.clone()).expect("open store");
    let lane_epoch = LaneEpoch::new(LaneId::new(2), 9);
    store.record(lane_epoch, 42).expect("record");
    drop(store);

    let reopened = ReplayCursorStore::open(path).expect("reopen store");
    let mut entries = reopened.highest_sequences();
    assert_eq!(entries.len(), 1);
    entries.sort_by_key(|(lane_epoch, _)| lane_epoch.lane_id.as_u32());
    assert_eq!(entries[0], (lane_epoch, 42));
}

#[test]
fn resolve_manifest_emits_parity_chunks() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_111,
        &rent_policy,
    )
    .expect("resolve manifest with parity");

    let expected = build_chunk_commitments(&request, &chunk_store, canonical.as_slice())
        .expect("expected chunk commitments");
    assert_eq!(artifacts.manifest.chunks, expected);

    let parity_chunks: Vec<_> = artifacts
        .manifest
        .chunks
        .iter()
        .filter(|chunk| chunk.parity)
        .collect();
    assert_eq!(
        parity_chunks.len(),
        usize::from(request.erasure_profile.parity_shards)
    );

    for (idx, chunk) in parity_chunks.into_iter().enumerate() {
        let expected_offset = request
            .total_size
            .checked_add(
                u64::try_from(idx)
                    .expect("parity index fits into u64")
                    .checked_mul(u64::from(request.chunk_size))
                    .expect("offset within test bounds"),
            )
            .expect("parity offset within test bounds");
        assert_eq!(chunk.offset, expected_offset);
        assert_eq!(chunk.length, request.chunk_size);
        assert!(chunk.parity);
    }
}

#[test]
fn resolve_manifest_uses_provided_rent_policy() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::from_components(750_000, 1_500, 250, 125, 2_000);
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_001_000,
        &rent_policy,
    )
    .expect("resolve manifest with custom rent policy");

    let (gib, months) = rent_usage_from_request(request.total_size, &request.retention_policy);
    let expected_quote = rent_policy
        .quote(gib, months)
        .expect("rent quote should compute for test inputs");
    assert_eq!(artifacts.manifest.rent_quote, expected_quote);
}

#[test]
fn resolve_manifest_applies_enforced_retention_policy() {
    let request = sample_request();
    let canonical_bytes = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let enforced = RetentionPolicy {
        hot_retention_secs: 99,
        cold_retention_secs: 199,
        required_replicas: 9,
        storage_class: StorageClass::Cold,
        governance_tag: GovernanceTag::new("da.test"),
    };

    let rent_policy = DaRentPolicyV1::default();
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical_bytes.as_slice(),
        &metadata,
        &enforced,
        1_701_000_555,
        &rent_policy,
    )
    .expect("resolve manifest with enforced retention");

    assert_eq!(artifacts.manifest.retention_policy, enforced);
}

#[test]
fn sampling_plan_clamps_to_sample_window_and_chunk_count() {
    use std::collections::HashSet;

    let manifest = sampling_manifest(20);
    let block_hash = Hash::new(b"block-assign-clamp");
    let plan = build_sampling_plan(&manifest, &block_hash);
    let expected_window = compute_sample_window(manifest.total_size);
    let expected_len = usize::min(manifest.chunks.len(), expected_window as usize);

    assert_eq!(plan.sample_window, expected_window);
    assert_eq!(plan.samples.len(), expected_len);
    let mut seen = HashSet::new();
    for sample in &plan.samples {
        assert!(
            seen.insert(sample.chunk_index),
            "duplicate sample {} detected",
            sample.chunk_index
        );
    }
}

#[test]
fn sampling_plan_is_deterministic_for_seed() {
    let manifest = sampling_manifest(6);
    let block_hash = Hash::new(b"block-deterministic");
    let first = build_sampling_plan(&manifest, &block_hash);
    let second = build_sampling_plan(&manifest, &block_hash);

    assert_eq!(first.assignment_hash, second.assignment_hash);
    assert_eq!(first.samples, second.samples);
}

#[test]
fn sampling_plan_changes_with_block_hash() {
    let manifest = sampling_manifest(6);
    let first = build_sampling_plan(&manifest, &Hash::new(b"block-a"));
    let second = build_sampling_plan(&manifest, &Hash::new(b"block-b"));

    assert_ne!(first.assignment_hash, second.assignment_hash);
    assert_ne!(first.samples, second.samples);
}

#[test]
fn provided_manifest_must_match_enforced_retention_policy() {
    let mut request = sample_request();
    let canonical_bytes = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical_bytes.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_600,
        &rent_policy,
    )
    .expect("resolve manifest");
    request.norito_manifest = Some(to_bytes(&artifacts.manifest).expect("encode manifest"));

    let strict_policy = RetentionPolicy {
        hot_retention_secs: request.retention_policy.hot_retention_secs + 1,
        cold_retention_secs: request.retention_policy.cold_retention_secs,
        required_replicas: request.retention_policy.required_replicas,
        storage_class: request.retention_policy.storage_class,
        governance_tag: GovernanceTag::new("da.strict"),
    };
    let err = resolve_manifest(
        &request,
        &chunk_store,
        canonical_bytes.as_slice(),
        &metadata,
        &strict_policy,
        1_701_000_601,
        &rent_policy,
    )
    .expect_err("mismatched retention policy must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn provided_manifest_with_wrong_parity_is_rejected() {
    let mut request = sample_request();
    let canonical_bytes = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical_bytes.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_222,
        &rent_policy,
    )
    .expect("resolve manifest");

    let mut tampered = artifacts.manifest.clone();
    let first_parity = tampered
        .chunks
        .iter_mut()
        .find(|chunk| chunk.parity)
        .expect("expected parity chunk to mutate");
    first_parity.parity = false;

    request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
    let err = match resolve_manifest(
        &request,
        &chunk_store,
        canonical_bytes.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_333,
        &rent_policy,
    ) {
        Ok(_) => panic!("manifest with mismatched parity flag must be rejected"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn provided_manifest_with_wrong_ipa_commitment_is_rejected() {
    let mut request = sample_request();
    let canonical_bytes = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical_bytes.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_920,
        &rent_policy,
    )
    .expect("resolve manifest");

    let mut tampered = artifacts.manifest.clone();
    tampered.ipa_commitment = BlobDigest::new([0xAB; 32]);
    request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
    let err = resolve_manifest(
        &request,
        &chunk_store,
        canonical_bytes.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_921,
        &rent_policy,
    )
    .expect_err("manifest with mismatched ipa commitment must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn governance_metadata_is_encrypted_with_configured_key() {
    let mut request = sample_request();
    let secret = b"confidential-notes".to_vec();
    request.metadata.items.push(MetadataEntry::new(
        "gov-notes",
        secret.clone(),
        MetadataVisibility::GovernanceOnly,
    ));
    let key = [0x11u8; 32];

    let encrypted = encrypt_governance_metadata(&request.metadata, Some(&key), Some("primary"))
        .expect("encryption");
    let entry = encrypted
        .items
        .iter()
        .find(|item| item.key == "gov-notes")
        .expect("entry present");
    assert_eq!(
        entry.encryption,
        MetadataEncryption::chacha20poly1305_with_label(Some("primary"))
    );
    assert_ne!(entry.value, secret);

    let decryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("decryptor");
    let plaintext = decryptor
        .decrypt_easy(entry.key.as_bytes(), &entry.value)
        .expect("decrypt");
    assert_eq!(plaintext, secret);
}

#[test]
fn governance_metadata_without_key_is_rejected() {
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::new(
            "gov-only",
            b"secret".to_vec(),
            MetadataVisibility::GovernanceOnly,
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, None, None) {
        Ok(_) => panic!("expected governance-only metadata to require encryption key"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn public_metadata_cannot_declare_encryption() {
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "public",
            b"plain".to_vec(),
            MetadataVisibility::Public,
            MetadataEncryption::chacha20poly1305_with_label(Some("public")),
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, Some(&[0u8; 32]), Some("primary")) {
        Ok(_) => panic!("expected public metadata to reject encryption hints"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn governance_metadata_rejects_label_mismatch() {
    let key = [0x22u8; 32];
    let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
    let ciphertext = encryptor
        .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
        .expect("encrypt payload");
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "gov-notes",
            ciphertext,
            MetadataVisibility::GovernanceOnly,
            MetadataEncryption::chacha20poly1305_with_label(Some("secondary")),
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, Some(&key), Some("primary")) {
        Ok(_) => panic!("expected label mismatch to be rejected"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn governance_metadata_requires_label_when_expected() {
    let key = [0x33u8; 32];
    let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
    let ciphertext = encryptor
        .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
        .expect("encrypt payload");
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "gov-notes",
            ciphertext,
            MetadataVisibility::GovernanceOnly,
            MetadataEncryption::chacha20poly1305_with_label(None::<String>),
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, Some(&key), Some("primary")) {
        Ok(_) => panic!("expected missing label to be rejected"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[test]
fn governance_metadata_accepts_matching_label_ciphertext() {
    let key = [0x44u8; 32];
    let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
    let ciphertext = encryptor
        .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
        .expect("encrypt payload");
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "gov-notes",
            ciphertext.clone(),
            MetadataVisibility::GovernanceOnly,
            MetadataEncryption::chacha20poly1305_with_label(Some("primary")),
        )],
    };
    let processed =
        encrypt_governance_metadata(&metadata, Some(&key), Some("primary")).expect("process");
    let entry = processed
        .items
        .iter()
        .find(|item| item.key == "gov-notes")
        .expect("entry");
    assert_eq!(entry.value, ciphertext);
    assert_eq!(
        entry.encryption,
        MetadataEncryption::chacha20poly1305_with_label(Some("primary"))
    );
}

#[test]
fn streaming_chunk_ingest_matches_fixture() {
    let (request, canonical_payload) = sample_request_with_payload();
    let chunk_profile = chunk_profile_for_request(request.chunk_size);
    let plan = CarBuildPlan::single_file_with_profile(&canonical_payload, chunk_profile)
        .expect("plan derivation succeeds");
    let mut streaming_store = ChunkStore::with_profile(chunk_profile);
    let chunk_dir = tempdir().expect("chunk dir");
    let mut payload_cursor: &[u8] = canonical_payload.as_slice();
    let stream_output = streaming_store
        .ingest_plan_stream_to_directory(&plan, &mut payload_cursor, chunk_dir.path())
        .expect("streaming ingest succeeds");
    assert_eq!(
        stream_output.total_bytes, request.total_size,
        "persisted byte count should match total_size"
    );

    let direct_store = build_chunk_store(&request, canonical_payload.as_slice());
    assert_eq!(
        streaming_store.profile(),
        direct_store.profile(),
        "chunk profiles must match"
    );
    assert_eq!(
        streaming_store.payload_digest(),
        direct_store.payload_digest(),
        "payload digests must match"
    );
    assert_eq!(
        streaming_store.payload_len(),
        direct_store.payload_len(),
        "payload lengths must match"
    );
    assert_eq!(
        streaming_store.chunks(),
        direct_store.chunks(),
        "chunk metadata mismatch between streaming/non-streaming ingestion"
    );

    let expected_records = load_chunk_record_fixture("sample_chunk_records.txt");
    assert_eq!(
        stream_output.records.len(),
        expected_records.len(),
        "chunk record count drifted; regenerate fixtures"
    );
    for (actual, expected) in stream_output.records.iter().zip(expected_records.iter()) {
        assert_eq!(actual.file_name, expected.file_name);
        assert_eq!(actual.offset, expected.offset);
        assert_eq!(actual.length, expected.length);
        assert_eq!(hex::encode(actual.digest), expected.digest_hex);
    }
}

#[test]
fn manifest_persistence_matches_fixture() {
    let context = sample_manifest_context_for(BlobClass::TaikaiSegment);
    let spool_dir = tempdir().expect("spool dir");
    let manifest_path = persistence::persist_manifest_for_sorafs(
        spool_dir.path(),
        &context.artifacts.encoded,
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &context.artifacts.storage_ticket,
        &context.artifacts.fingerprint,
    )
    .expect("persist manifest")
    .expect("spool path");
    let actual_bytes = fs::read(manifest_path).expect("read manifest");
    let expected_bytes = load_manifest_fixture("manifests/taikai_segment/manifest.norito.hex");
    assert_eq!(
        actual_bytes, expected_bytes,
        "DA manifest drifted; rerun regenerate_da_ingest_fixtures"
    );
}

#[test]
fn manifest_fixtures_cover_all_blob_classes() {
    for case in &MANIFEST_FIXTURE_CASES {
        let context = sample_manifest_context_for(case.blob_class);
        let expected_bytes =
            load_manifest_fixture(&format!("manifests/{}/manifest.norito.hex", case.slug));
        assert_eq!(
            context.artifacts.encoded, expected_bytes,
            "manifest fixture hex drifted for {}; rerun regenerate_da_ingest_fixtures",
            case.slug
        );

        let expected_json =
            load_manifest_json_fixture(&format!("manifests/{}/manifest.json", case.slug));
        let actual_json =
            json::to_value(&context.artifacts.manifest).expect("serialize manifest to JSON");
        assert_eq!(
            actual_json, expected_json,
            "manifest JSON fixture drifted for {}; rerun regenerate_da_ingest_fixtures",
            case.slug
        );
    }
}

#[test]
#[ignore = "regenerates DA ingest fixtures on disk"]
fn regenerate_da_ingest_fixtures() {
    for case in &MANIFEST_FIXTURE_CASES {
        let context = sample_manifest_context_for(case.blob_class);
        write_manifest_fixture_bundle(case, &context).expect("write manifest fixture bundle");
    }
    println!(
        "Regenerated manifest fixtures for {} blob classes under {}/manifests",
        MANIFEST_FIXTURE_CASES.len(),
        fixtures_dir().display()
    );

    let (request, canonical_payload) = sample_request_with_payload();
    let chunk_profile = chunk_profile_for_request(request.chunk_size);
    let plan = CarBuildPlan::single_file_with_profile(&canonical_payload, chunk_profile)
        .expect("plan derivation succeeds");
    let mut streaming_store = ChunkStore::with_profile(chunk_profile);
    let chunk_dir = tempdir().expect("chunk dir");
    let mut payload_cursor: &[u8] = canonical_payload.as_slice();
    let stream_output = streaming_store
        .ingest_plan_stream_to_directory(&plan, &mut payload_cursor, chunk_dir.path())
        .expect("streaming ingest succeeds");
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("encrypt metadata");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &streaming_store,
        canonical_payload.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_999,
        &rent_policy,
    )
    .expect("resolve manifest");
    let chunk_fixture_path = fixtures_dir().join("sample_chunk_records.txt");
    write_chunk_record_fixture(
        &chunk_fixture_path,
        &stream_output.records,
        stream_output.total_bytes,
    )
    .expect("write chunk fixture");
    println!(
        "Regenerated chunk fixtures at {} (total bytes = {})",
        chunk_fixture_path.display(),
        stream_output.total_bytes
    );
    println!(
        "Manifest hex for reference (taikai segment): {}",
        hex::encode(&manifest.encoded)
    );
}

fn sample_request_with_payload() -> (DaIngestRequest, Vec<u8>) {
    let request = sample_request();
    let canonical_vec = {
        let canonical = normalize_payload(&request).expect("normalize payload");
        canonical.into_vec()
    };
    (request, canonical_vec)
}

#[derive(Clone, Copy)]
struct ManifestFixtureCase {
    slug: &'static str,
    blob_class: BlobClass,
}

const MANIFEST_FIXTURE_CASES: [ManifestFixtureCase; 4] = [
    ManifestFixtureCase {
        slug: "taikai_segment",
        blob_class: BlobClass::TaikaiSegment,
    },
    ManifestFixtureCase {
        slug: "nexus_lane_sidecar",
        blob_class: BlobClass::NexusLaneSidecar,
    },
    ManifestFixtureCase {
        slug: "governance_artifact",
        blob_class: BlobClass::GovernanceArtifact,
    },
    ManifestFixtureCase {
        slug: "custom_0042",
        blob_class: BlobClass::Custom(0x0042),
    },
];

const fn manifest_fixture_variant_guard(class: BlobClass) {
    match class {
        BlobClass::TaikaiSegment
        | BlobClass::NexusLaneSidecar
        | BlobClass::GovernanceArtifact
        | BlobClass::Custom(_) => {}
    }
}

const _: fn(BlobClass) = manifest_fixture_variant_guard;

struct ManifestFixtureContext {
    request: DaIngestRequest,
    artifacts: ManifestArtifacts,
}

fn sample_manifest_context_for(blob_class: BlobClass) -> ManifestFixtureContext {
    let (mut request, canonical_payload) = sample_request_with_payload();
    request.blob_class = blob_class;
    let chunk_store = build_chunk_store(&request, canonical_payload.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical_payload.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_999,
        &rent_policy,
    )
    .expect("resolve manifest");

    ManifestFixtureContext { request, artifacts }
}
const METRIC_ASSERT_EPSILON: f64 = 1e-6;

#[test]
fn record_taikai_ingest_metrics_updates_histograms() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let sample = taikai_ingest::TaikaiTelemetrySample {
        event_id: "event".into(),
        stream_id: "stream-main".into(),
        rendition_id: "1080p".into(),
        segment_sequence: 5,
        wallclock_unix_ms: 1_702_560_000_000,
        ingest_latency_ms: Some(150),
        live_edge_drift_ms: Some(-37),
    };
    taikai::record_taikai_ingest_metrics(&telemetry, "cluster-a", &sample);

    let dump = metrics.try_to_string().expect("metrics text");
    let latency_line = find_metric_line(
        &dump,
        "taikai_ingest_segment_latency_ms_sum{cluster=\"cluster-a\"",
    );
    assert!(latency_line.contains(r#"stream="stream-main""#));
    let latency = parse_metric_value(latency_line);
    assert!(
        (latency - 150.0).abs() < METRIC_ASSERT_EPSILON,
        "expected ingest latency sum to equal 150.0, got {latency}"
    );

    let drift_line = find_metric_line(
        &dump,
        "taikai_ingest_live_edge_drift_ms_sum{cluster=\"cluster-a\"",
    );
    assert!(drift_line.contains(r#"stream="stream-main""#));
    let drift = parse_metric_value(drift_line);
    assert!(
        (drift - 37.0).abs() < METRIC_ASSERT_EPSILON,
        "expected live-edge drift sum to equal 37.0, got {drift}"
    );

    let signed_drift_line = find_metric_line(
        &dump,
        "taikai_ingest_live_edge_drift_signed_ms{cluster=\"cluster-a\"",
    );
    assert!(signed_drift_line.contains(r#"stream="stream-main""#));
    let signed_drift = parse_metric_value(signed_drift_line);
    assert!(
        (signed_drift + 37.0).abs() < METRIC_ASSERT_EPSILON,
        "expected signed live-edge drift gauge to equal -37.0, got {signed_drift}"
    );
}

#[test]
fn record_taikai_ingest_error_counts_by_status() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    taikai::record_taikai_ingest_error(
        &telemetry,
        "cluster-a",
        "stream-main",
        StatusCode::BAD_REQUEST,
    );

    let dump = metrics.try_to_string().expect("metrics text");
    let error_line = find_metric_line(&dump, "taikai_ingest_errors_total{cluster=\"cluster-a\"");
    assert!(error_line.contains(r#"stream="stream-main""#));
    assert!(error_line.contains(r#"reason="Bad Request""#));
    let errors = parse_metric_value(error_line);
    assert!(
        (errors - 1.0).abs() < METRIC_ASSERT_EPSILON,
        "expected error counter to equal 1.0, got {errors}"
    );
}

#[test]
fn record_taikai_alias_rotation_event_updates_metrics() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let manifest = sample_trm_manifest();
    taikai::record_taikai_alias_rotation_event(&telemetry, "cluster-a", &manifest, "deadbeef");

    let dump = metrics.try_to_string().expect("metrics text");
    let metric_line = find_metric_line(
        &dump,
        "taikai_trm_alias_rotations_total{alias_name=\"docs\",alias_namespace=\"sora\"",
    );
    assert!(
        metric_line.contains("cluster=\"cluster-a\"")
            && metric_line.contains("event=\"global-keynote\"")
            && metric_line.contains("stream=\"stage-a\""),
        "metric labels should reflect cluster/event/stream"
    );
    let value = parse_metric_value(metric_line);
    assert!(
        (value - 1.0).abs() < METRIC_ASSERT_EPSILON,
        "expected alias rotation counter to increment"
    );

    let snapshots = metrics.taikai_alias_rotation_status();
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.cluster, "cluster-a");
    assert_eq!(snapshot.event, "global-keynote");
    assert_eq!(snapshot.stream, "stage-a");
    assert_eq!(snapshot.alias_namespace, "sora");
    assert_eq!(snapshot.alias_name, "docs");
    assert_eq!(snapshot.window_start_sequence, 40);
    assert_eq!(snapshot.window_end_sequence, 64);
    assert_eq!(snapshot.manifest_digest_hex, "deadbeef");
    assert_eq!(snapshot.rotations_total, 1);
    assert!(snapshot.last_updated_unix > 0);
}

#[test]
fn record_da_rent_quote_metrics_accumulates_values() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let quote = DaRentQuote {
        base_rent: XorAmount::from_micro(1_000_000),
        protocol_reserve: XorAmount::from_micro(250_000),
        provider_reward: XorAmount::from_micro(750_000),
        pdp_bonus: XorAmount::from_micro(50_000),
        potr_bonus: XorAmount::from_micro(25_000),
        egress_credit_per_gib: XorAmount::from_micro(1_500),
    };

    record_da_rent_quote_metrics(&telemetry, "cluster-a", StorageClass::Warm, 4, 3, &quote);

    let dump = metrics.try_to_string().expect("metrics text");
    let gib_line = find_metric_line(
        &dump,
        "torii_da_rent_gib_months_total{cluster=\"cluster-a\"",
    );
    assert!(gib_line.contains(r#"storage_class="warm""#));
    let gib_months = parse_metric_value(gib_line);
    assert!(
        (gib_months - 12.0).abs() < METRIC_ASSERT_EPSILON,
        "expected 12 GiB-months recorded"
    );

    for (metric, expected) in [
        ("torii_da_rent_base_micro_total", 1_000_000.0),
        ("torii_da_protocol_reserve_micro_total", 250_000.0),
        ("torii_da_provider_reward_micro_total", 750_000.0),
        ("torii_da_pdp_bonus_micro_total", 50_000.0),
        ("torii_da_potr_bonus_micro_total", 25_000.0),
    ] {
        let line = find_metric_line(
            &dump,
            &format!("{metric}{{cluster=\"cluster-a\",storage_class=\"warm\""),
        );
        let value = parse_metric_value(line);
        assert!(
            (value - expected).abs() < METRIC_ASSERT_EPSILON,
            "metric {metric} expected {expected}, got {value}"
        );
    }
}

#[test]
fn record_da_chunking_metrics_observes_histogram() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    record_da_chunking_metrics(&telemetry, Duration::from_millis(150));
    let samples = metrics.torii_da_chunking_seconds.get_sample_count();
    assert_eq!(samples, 1);
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn da_rent_metrics_exposed_via_metrics_handler_snapshot() {
    let (metrics, telemetry) = telemetry_handle_for_tests_with_profile(TelemetryProfile::Extended);
    let quote = DaRentQuote {
        base_rent: XorAmount::from_micro(1_000_000),
        protocol_reserve: XorAmount::from_micro(250_000),
        provider_reward: XorAmount::from_micro(750_000),
        pdp_bonus: XorAmount::from_micro(50_000),
        potr_bonus: XorAmount::from_micro(25_000),
        egress_credit_per_gib: XorAmount::from_micro(1_500),
    };

    record_da_rent_quote_metrics(&telemetry, "cluster-a", StorageClass::Warm, 4, 3, &quote);

    let prometheus = crate::handle_metrics(&telemetry, true)
        .await
        .expect("prometheus snapshot");
    let snapshot = da_rent_metric_lines(&prometheus);
    assert_eq!(
        snapshot,
        vec![
            "# HELP torii_da_pdp_bonus_micro_total Aggregate PDP bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_potr_bonus_micro_total Aggregate PoTR bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_protocol_reserve_micro_total Aggregate protocol reserve (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_provider_reward_micro_total Aggregate provider rewards (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_rent_base_micro_total Aggregate base rent (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_rent_gib_months_total Aggregate GiB-month usage quoted by DA ingest grouped by cluster and storage class",
            "# TYPE torii_da_pdp_bonus_micro_total counter",
            "# TYPE torii_da_potr_bonus_micro_total counter",
            "# TYPE torii_da_protocol_reserve_micro_total counter",
            "# TYPE torii_da_provider_reward_micro_total counter",
            "# TYPE torii_da_rent_base_micro_total counter",
            "# TYPE torii_da_rent_gib_months_total counter",
            "torii_da_pdp_bonus_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 50000",
            "torii_da_potr_bonus_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 25000",
            "torii_da_protocol_reserve_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 250000",
            "torii_da_provider_reward_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 750000",
            "torii_da_rent_base_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 1000000",
            "torii_da_rent_gib_months_total{cluster=\"cluster-a\",storage_class=\"warm\"} 12"
        ],
        "DA rent Prometheus payload drifted"
    );

    let dump = metrics.try_to_string().expect("metrics text");
    for line in snapshot {
        assert!(
            dump.contains(&line),
            "metrics text missing `{line}`\n{dump}"
        );
    }
}

#[test]
fn record_da_receipt_metrics_tracks_outcomes_and_cursor() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let lane_epoch = LaneEpoch::new(LaneId::new(7), 3);

    record_da_receipt_metrics(
        &telemetry,
        lane_epoch,
        5,
        &ReceiptInsertOutcome::Stored {
            cursor_advanced: true,
        },
    );
    record_da_receipt_metrics(
        &telemetry,
        lane_epoch,
        5,
        &ReceiptInsertOutcome::Duplicate {
            path: std::path::PathBuf::new(),
        },
    );

    let stored = metrics
        .torii_da_receipts_total
        .with_label_values(&["stored", "7", "3"])
        .get();
    assert_eq!(stored, 1, "stored counter should increment");

    let duplicate = metrics
        .torii_da_receipts_total
        .with_label_values(&["duplicate", "7", "3"])
        .get();
    assert_eq!(duplicate, 1, "duplicate counter should increment");

    let cursor = metrics
        .torii_da_receipt_highest_sequence
        .with_label_values(&["7", "3"])
        .get();
    assert_eq!(cursor, 5, "cursor gauge should reflect stored sequence");
}

fn telemetry_handle_for_tests_with_profile(
    profile: TelemetryProfile,
) -> (Arc<Metrics>, MaybeTelemetry) {
    let metrics = test_metrics();
    let telemetry = Telemetry::new(metrics.clone(), true);
    let handle = MaybeTelemetry::from_profile(Some(telemetry), profile);
    (metrics, handle)
}

fn telemetry_handle_for_tests() -> (Arc<Metrics>, MaybeTelemetry) {
    telemetry_handle_for_tests_with_profile(TelemetryProfile::Operator)
}

fn test_metrics() -> Arc<Metrics> {
    enable_duplicate_metric_panic();
    Arc::new(Metrics::default())
}

fn enable_duplicate_metric_panic() {
    static INIT: LazyLock<()> = LazyLock::new(|| {
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("IROHA_METRICS_PANIC_ON_DUPLICATE", "1");
        }
    });
    LazyLock::force(&INIT);
}

fn find_metric_line<'a>(dump: &'a str, prefix: &str) -> &'a str {
    dump.lines()
        .find(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("metric `{prefix}` not found\n{dump}"))
}

fn da_rent_metric_lines(dump: &str) -> Vec<String> {
    let mut lines: Vec<String> = dump
        .lines()
        .filter(|line| {
            line.starts_with("# HELP torii_da_")
                || line.starts_with("# TYPE torii_da_")
                || line.starts_with("torii_da_")
        })
        .filter(|line| {
            line.contains("_rent_")
                || line.contains("protocol_reserve_micro_total")
                || line.contains("provider_reward_micro_total")
                || line.contains("_pdp_bonus_micro_total")
                || line.contains("_potr_bonus_micro_total")
        })
        .map(str::to_owned)
        .collect();
    lines.sort();
    lines
}

fn parse_metric_value(line: &str) -> f64 {
    line.split_whitespace()
        .last()
        .unwrap_or_default()
        .parse::<f64>()
        .expect("metric value")
}

struct ChunkRecordFixture {
    file_name: String,
    offset: u64,
    length: u32,
    digest_hex: String,
}

fn load_chunk_record_fixture(name: &str) -> Vec<ChunkRecordFixture> {
    let path = fixtures_dir().join(name);
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read chunk fixture {}: {err}", path.display());
    });
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let file_name = parts.next()?.to_string();
            let offset = parts
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(|| panic!("missing offset in fixture line `{line}`"));
            let length = parts
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(|| panic!("missing length in fixture line `{line}`"));
            let digest_hex = parts
                .next()
                .map(ToString::to_string)
                .unwrap_or_else(|| panic!("missing digest in fixture line `{line}`"));
            Some(ChunkRecordFixture {
                file_name,
                offset,
                length,
                digest_hex,
            })
        })
        .collect()
}

fn load_manifest_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read manifest fixture {}: {err}", path.display());
    });
    hex::decode(contents.trim()).expect("fixture must be valid hex")
}

fn load_manifest_json_fixture(name: &str) -> Value {
    let path = fixtures_dir().join(name);
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "failed to read manifest JSON fixture {}: {err}",
            path.display()
        );
    });
    json::from_str(&contents).expect("fixture must be valid Norito JSON")
}

fn write_manifest_fixture_bundle(
    case: &ManifestFixtureCase,
    context: &ManifestFixtureContext,
) -> std::io::Result<()> {
    let manifest_dir = fixtures_dir().join("manifests").join(case.slug);
    fs::create_dir_all(&manifest_dir)?;
    let hex_path = manifest_dir.join("manifest.norito.hex");
    let hex_text = format!("{}\n", hex::encode(&context.artifacts.encoded));
    fs::write(hex_path, hex_text)?;
    let manifest_value =
        json::to_value(&context.artifacts.manifest).expect("serialize manifest as JSON value");
    let json_text = json::to_string_pretty(&manifest_value).expect("render manifest JSON fixture");
    fs::write(manifest_dir.join("manifest.json"), format!("{json_text}\n"))?;
    Ok(())
}

fn write_chunk_record_fixture(
    path: &Path,
    records: &[PersistedChunkRecord],
    total_bytes: u64,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    writeln!(file, "# file_name offset length digest_hex")?;
    for record in records {
        writeln!(
            file,
            "{} {} {} {}",
            record.file_name,
            record.offset,
            record.length,
            hex::encode(record.digest)
        )?;
    }
    writeln!(file, "# total_bytes {total_bytes}")?;
    Ok(())
}

fn format_base_id(
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> String {
    let lane_hex = format!("{:08x}", lane_id.as_u32());
    let epoch_hex = format!("{:016x}", epoch);
    let sequence_hex = format!("{:016x}", sequence);
    let ticket_hex = hex::encode(ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    format!("{lane_hex}-{epoch_hex}-{sequence_hex}-{ticket_hex}-{fingerprint_hex}")
}

fn fixtures_dir() -> PathBuf {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");
    base.canonicalize()
        .expect("fixtures/da/ingest directory must exist")
}
