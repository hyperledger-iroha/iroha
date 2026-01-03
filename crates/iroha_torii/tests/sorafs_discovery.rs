#![cfg(feature = "app_api")]

//! Tests Torii's `SoraFS` discovery cache using the mesh harness from provider advert suites.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::TryInto,
    fs,
    net::SocketAddr,
    num::NonZeroU64,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

use axum::{
    Router,
    body::Body,
    extract::connect_info::ConnectInfo,
    http::{Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use hex::ToHex;
use http::header::{AGE, CACHE_CONTROL, RETRY_AFTER, WARNING};
use http_body_util::BodyExt;
use humantime::format_rfc3339;
use iroha_config::{
    base::util::Bytes,
    parameters::actual::{self as actual_cfg, SorafsAdmission},
};
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    prelude::World,
    query::store::LiveQueryStore,
    queue::Queue as CoreQueue,
    smartcontracts::Execute,
    state::{State, StateReadOnly, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{KeyPair, PrivateKey};
use iroha_data_model::{
    ChainId,
    block::BlockHeader,
    isi::sorafs::{ApprovePinManifest, RegisterPinManifest},
    name::Name,
    prelude as dm,
    sorafs::pin_registry::{
        ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
        ManifestDigest as RegistryManifestDigest, PinPolicy as RegistryPinPolicy,
        StorageClass as RegistryStorageClass,
    },
    transaction::{SignedTransaction, TransactionBuilder},
};
use iroha_futures::supervisor::Child;
use iroha_primitives::json::Json;
use iroha_torii::{
    MaybeTelemetry, OnlinePeersProvider, PinPolicyDto, PinPolicyStorageClassDto,
    RegisterPinManifestDto, Torii,
    sorafs::{
        AdmissionCheckError, AdmissionRegistry, AliasCachePolicyExt,
        api::StorageStateResponseDto,
        discovery::{
            AdvertError, AdvertIngest, AdvertIngestResult, AdvertWarning, ProviderAdvertCache,
        },
        unix_now_secs,
    },
    test_utils::{AuthorityCreds, drain_queue_and_apply_all, random_authority},
};
use mv::storage::StorageReadOnly;
use norito::{decode_from_bytes, json, to_bytes};
use sorafs_car::{ChunkStore, por_json::proof_from_value};
use sorafs_manifest::provider_advert::ProviderCapabilitySoranetPqV1;
use sorafs_manifest::{
    AdvertEndpoint, AdvertValidationError, AvailabilityTier, BLAKE3_256_MULTIHASH_CODE,
    CapabilityTlv, CapabilityType, CouncilSignature, DagCodecId, ENDPOINT_ATTESTATION_VERSION_V1,
    EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1, EndpointKind,
    EndpointMetadata, EndpointMetadataKey, GovernanceProofs, MANIFEST_DAG_CODEC, ManifestBuilder,
    PROVIDER_ADVERT_VERSION_V1, PathDiversityPolicy, PinPolicy, ProfileId,
    ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1, ProviderAdvertBodyV1,
    ProviderAdvertV1, ProviderCapabilityRangeV1, QosHints, RendezvousTopic, SignatureAlgorithm,
    StakePointer, StorageClass as ManifestStorageClass, StreamBudgetV1, TransportHintV1,
    TransportProtocol, compute_advert_body_digest, compute_proposal_digest,
    pin_registry::{
        AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
    },
};
use tempfile::tempdir;
use tower::ServiceExt as _;

const ISSUED_AT: u64 = 1_700_000_000;
const TTL_SECS: u64 = 3_600;
const STATUS_TIMESTAMP_KEY: &str = "sorafs_status_timestamp_unix";
const GOVERNANCE_REFS_KEY: &str = "sorafs_governance_refs";

fn ingest_tests_enabled() -> bool {
    std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")
}

fn range_capability_payload(span: u32, granularity: u32) -> Vec<u8> {
    ProviderCapabilityRangeV1 {
        max_chunk_span: span,
        min_granularity: granularity,
        supports_sparse_offsets: false,
        requires_alignment: false,
        supports_merkle_proof: false,
    }
    .to_bytes()
    .expect("construct range capability payload")
}

fn chunk_range_capability(span: u32, granularity: u32) -> CapabilityTlv {
    CapabilityTlv {
        cap_type: CapabilityType::ChunkRangeFetch,
        payload: range_capability_payload(span, granularity),
    }
}

fn default_range_capability() -> CapabilityTlv {
    chunk_range_capability(32, 1)
}

fn soranet_pq_capability() -> CapabilityTlv {
    let payload = ProviderCapabilitySoranetPqV1 {
        supports_guard: true,
        supports_majority: false,
        supports_strict: false,
    }
    .to_bytes()
    .expect("construct soranet pq capability payload");
    CapabilityTlv {
        cap_type: CapabilityType::SoraNetHybridPq,
        payload,
    }
}

#[derive(Clone)]
struct ProviderFixture {
    advert: ProviderAdvertV1,
    envelope: ProviderAdmissionEnvelopeV1,
}

fn find_alias_entry<'a>(
    aliases: &'a [json::Value],
    namespace: &str,
    name: &str,
) -> Option<&'a json::Value> {
    aliases.iter().find(|entry| {
        entry
            .get("alias")
            .and_then(json::Value::as_object)
            .is_some_and(|alias| {
                alias.get("namespace").and_then(json::Value::as_str) == Some(namespace)
                    && alias.get("name").and_then(json::Value::as_str) == Some(name)
            })
    })
}

fn manifest_entries(response: &json::Value) -> &[json::Value] {
    response
        .get("manifests")
        .and_then(json::Value::as_array)
        .unwrap_or_else(|| panic!("manifests array present; got {response:?}"))
}

#[test]
fn torii_mesh_propagates_valid_advert() {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0x11; 32],
        [0x21; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            default_range_capability(),
        ],
        false,
    );

    let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));

    let mut mesh = ToriiMesh::with_edges(
        vec![
            ToriiNode::new(
                "alpha",
                &[
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
                registry.clone(),
            ),
            ToriiNode::new(
                "beta",
                &[
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
                registry.clone(),
            ),
            ToriiNode::new(
                "gamma",
                &[
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
                registry.clone(),
            ),
        ],
        &[("alpha", "beta"), ("beta", "gamma")],
    );

    mesh.publish("alpha", &fixture.advert, ISSUED_AT + 30)
        .expect("propagation succeeds");

    for id in ["alpha", "beta", "gamma"] {
        let node = mesh.node(id);
        assert_eq!(node.stored_count(), 1, "{id} must retain the advert");
        assert_eq!(
            node.capabilities_for(&fixture.advert.body.provider_id),
            Some(vec![
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch
            ]),
            "{id} must keep both advertised capabilities"
        );
    }
}

#[test]
fn soranet_mesh_filters_capabilities_over_shared_dht() {
    let signing_key = SigningKey::from_bytes(&[0xA1; 32]);
    let mixed_fixture = make_signed_advert(
        &signing_key,
        [0x91; 32],
        [0xB1; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            chunk_range_capability(16, 1),
        ],
        true,
    );

    let registry = admission_registry_from_fixtures(std::slice::from_ref(&mixed_fixture));

    let mut mesh = ToriiMesh::with_edges(
        vec![
            ToriiNode::new(
                "hub",
                &[
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
                registry.clone(),
            ),
            ToriiNode::new("edge-a", &[CapabilityType::ToriiGateway], registry.clone()),
            ToriiNode::new("edge-b", &[CapabilityType::ToriiGateway], registry.clone()),
        ],
        &[("hub", "edge-a"), ("edge-a", "edge-b")],
    );

    mesh.publish("hub", &mixed_fixture.advert, ISSUED_AT + 48)
        .expect("advert propagates across shared SORA Nexus DHT mesh");

    let hub_caps = mesh
        .node("hub")
        .capabilities_for(&mixed_fixture.advert.body.provider_id)
        .expect("hub retains advert");
    assert_eq!(
        hub_caps,
        vec![
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch
        ],
        "hub keeps full capability set"
    );

    for edge in ["edge-a", "edge-b"] {
        let caps = mesh
            .node(edge)
            .capabilities_for(&mixed_fixture.advert.body.provider_id)
            .expect("edge node stores advert from shared mesh");
        assert_eq!(
            caps,
            vec![CapabilityType::ToriiGateway],
            "{edge} should retain only recognised capabilities"
        );
    }
}

#[test]
fn torii_mesh_enforces_grease_policy() {
    let signing_key = SigningKey::from_bytes(&[8u8; 32]);
    let grease_cap = CapabilityTlv {
        cap_type: CapabilityType::VendorReserved,
        payload: vec![0xAB, 0xCD],
    };

    let grease_fixture = make_signed_advert(
        &signing_key,
        [0x55; 32],
        [0x65; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            grease_cap.clone(),
        ],
        true,
    );

    let strict_fixture = make_signed_advert(
        &signing_key,
        [0x77; 32],
        [0x88; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            grease_cap,
        ],
        false,
    );

    let registry =
        admission_registry_from_fixtures(&[grease_fixture.clone(), strict_fixture.clone()]);

    let mut mesh = ToriiMesh::with_edges(
        vec![
            ToriiNode::new(
                "modern",
                &[CapabilityType::ToriiGateway, CapabilityType::VendorReserved],
                registry.clone(),
            ),
            ToriiNode::new(
                "relay",
                &[CapabilityType::ToriiGateway, CapabilityType::VendorReserved],
                registry.clone(),
            ),
            ToriiNode::new("strict", &[CapabilityType::ToriiGateway], registry.clone()),
        ],
        &[("modern", "relay"), ("relay", "strict")],
    );

    mesh.publish("modern", &grease_fixture.advert, ISSUED_AT + 45)
        .expect("GREASE-enabled advert must propagate");
    assert_eq!(
        mesh.node("strict")
            .capabilities_for(&grease_fixture.advert.body.provider_id),
        Some(vec![CapabilityType::ToriiGateway]),
        "strict node keeps only recognised capabilities when GREASE is allowed"
    );
    assert!(
        mesh.node("strict").rejection_reasons().is_empty(),
        "strict node should not reject GREASE advert with flag set"
    );

    mesh.publish("modern", &strict_fixture.advert, ISSUED_AT + 60)
        .expect("origin accepts advert");
    assert!(
        mesh.node("strict")
            .capabilities_for(&strict_fixture.advert.body.provider_id)
            .is_none(),
        "strict node must drop adverts with unknown capabilities when GREASE is disabled"
    );
    assert!(
        mesh.node("strict")
            .rejection_reasons()
            .iter()
            .any(|reason| reason.contains("unknown capabilities")),
        "strict node should record rejection for unknown capabilities"
    );
}

#[test]
fn provider_cache_warns_when_chunk_range_missing() {
    let signing_key = SigningKey::from_bytes(&[0x90; 32]);
    let missing_range_fixture = make_signed_advert(
        &signing_key,
        [0xAA; 32],
        [0xBB; 32],
        vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        false,
    );
    let registry = admission_registry_from_fixtures(std::slice::from_ref(&missing_range_fixture));
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
        ],
        registry,
    );

    let result = cache
        .ingest(missing_range_fixture.advert.clone(), ISSUED_AT + 15)
        .expect("advert should be accepted with warnings");
    assert!(
        matches!(result.outcome, AdvertIngest::Stored { .. }),
        "advert must be stored"
    );
    assert!(
        result
            .warnings
            .contains(&AdvertWarning::MissingChunkRangeCapability),
        "ingestion must emit missing_chunk_range warning"
    );

    let record = cache
        .record_by_provider(&missing_range_fixture.advert.body.provider_id)
        .expect("record stored");
    assert!(
        record
            .warnings()
            .contains(&AdvertWarning::MissingChunkRangeCapability),
        "cache record must retain warning metadata"
    );
}

#[test]
fn provider_cache_warns_when_signature_not_strict() {
    let signing_key = SigningKey::from_bytes(&[0x91; 32]);
    let modern_fixture = make_signed_advert(
        &signing_key,
        [0xAB; 32],
        [0xBC; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            chunk_range_capability(8, 1),
        ],
        false,
    );
    let registry = admission_registry_from_fixtures(std::slice::from_ref(&modern_fixture));
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
        ],
        registry,
    );

    let mut advert = modern_fixture.advert.clone();
    advert.signature_strict = false;
    let result = cache
        .ingest(advert, ISSUED_AT + 25)
        .expect("advert should be accepted with signature warning");
    assert!(
        result.warnings.contains(&AdvertWarning::SignatureNotStrict),
        "ingestion must warn about signature_not_strict"
    );
}

#[test]
fn provider_cache_rejects_soranet_transport_without_capability() {
    let signing_key = SigningKey::from_bytes(&[0x9A; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0xE1; 32],
        [0xE2; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            chunk_range_capability(8, 1),
        ],
        false,
    );
    let registry = AdmissionRegistry::from_envelopes([fixture.envelope.clone()])
        .expect("soranet admission envelope valid");
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
            CapabilityType::SoraNetHybridPq,
        ],
        Arc::new(registry),
    );

    let mut advert = fixture.advert.clone();
    advert.body.transport_hints = Some(vec![TransportHintV1 {
        protocol: TransportProtocol::SoraNetRelay,
        priority: 0,
    }]);
    let err = cache
        .ingest(advert, ISSUED_AT + 15)
        .expect_err("soranet transport without capability must be rejected");
    assert!(
        matches!(
            err,
            AdvertError::Validation(AdvertValidationError::SoranetTransportWithoutCapability)
        ),
        "validation error must surface soranet transport without capability"
    );
}

#[test]
fn provider_cache_accepts_soranet_transport_with_capability() {
    if !ingest_tests_enabled() {
        eprintln!("skipping soranet capability ingest test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let signing_key = SigningKey::from_bytes(&[0x9B; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0xE3; 32],
        [0xE4; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            soranet_pq_capability(),
            chunk_range_capability(8, 1),
        ],
        false,
    );
    let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
            CapabilityType::SoraNetHybridPq,
        ],
        registry,
    );

    let result = cache
        .ingest(fixture.advert.clone(), ISSUED_AT + 15)
        .expect("soranet transport with capability must ingest cleanly");
    assert!(
        matches!(result.outcome, AdvertIngest::Stored { .. }),
        "soranet advert should be stored"
    );
    let record = cache
        .record_by_provider(&fixture.advert.body.provider_id)
        .expect("record stored");
    assert!(
        record
            .known_capabilities()
            .contains(&CapabilityType::SoraNetHybridPq),
        "soranet capability must be retained on ingest"
    );
    assert!(
        record
            .advert()
            .body
            .transport_hints
            .as_ref()
            .is_some_and(|hints| hints
                .iter()
                .any(|hint| hint.protocol == TransportProtocol::SoraNetRelay)),
        "transport hints must include soranet relay"
    );
}

#[test]
fn provider_cache_rejects_invalid_signature_when_strict() {
    let signing_key = SigningKey::from_bytes(&[0x92; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0xCD; 32],
        [0xDE; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            chunk_range_capability(12, 1),
        ],
        false,
    );
    let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
        ],
        registry,
    );

    let mut advert = fixture.advert.clone();
    advert
        .signature
        .signature
        .iter_mut()
        .for_each(|byte| *byte ^= 0xFF);

    let err = cache
        .ingest(advert, ISSUED_AT + 30)
        .expect_err("strict advert must reject invalid signature");
    assert!(
        matches!(err, AdvertError::Signature(_)),
        "expected signature failure, got: {err:?}"
    );
}

#[test]
fn provider_cache_warns_about_invalid_signature_when_bypassed() {
    let signing_key = SigningKey::from_bytes(&[0x93; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0xEF; 32],
        [0xF1; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            chunk_range_capability(14, 1),
        ],
        false,
    );
    let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
        ],
        registry,
    );

    let mut advert = fixture.advert.clone();
    advert.signature_strict = false;
    if let Some(first) = advert.signature.signature.first_mut() {
        *first ^= 0xAB;
    }
    let result = cache
        .ingest(advert.clone(), ISSUED_AT + 32)
        .expect("non-strict advert should be accepted with warning");
    assert!(
        result.warnings.contains(&AdvertWarning::SignatureInvalid),
        "ingestion must warn about signature_invalid when verification fails"
    );
    let record = cache
        .record_by_provider(&advert.body.provider_id)
        .expect("record stored after bypass");
    assert!(
        record.warnings().contains(&AdvertWarning::SignatureInvalid),
        "cache record must retain signature_invalid warning"
    );
}

#[test]
fn provider_cache_prunes_expired_records() {
    let signing_key = SigningKey::from_bytes(&[0x94; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0xA1; 32],
        [0xB2; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            chunk_range_capability(10, 2),
        ],
        false,
    );
    let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
        ],
        registry,
    );

    let ingest_result = cache
        .ingest(fixture.advert.clone(), ISSUED_AT + 25)
        .expect("fresh advert ingested");
    assert!(
        matches!(ingest_result.outcome, AdvertIngest::Stored { .. }),
        "expected advert to be stored before pruning"
    );
    let removed = cache.prune_stale(fixture.advert.expires_at + 1);
    assert_eq!(removed, 1, "stale entries must be pruned");
    assert!(
        cache
            .record_by_provider(&fixture.advert.body.provider_id)
            .is_none(),
        "pruned advert must be removed from cache"
    );
}

#[test]
fn torii_mesh_rejects_stale_and_duplicate_adverts() {
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0x20; 32],
        [0x30; 32],
        vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        false,
    );

    let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));

    let mut mesh = ToriiMesh::with_edges(
        vec![ToriiNode::new(
            "alpha",
            &[
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            registry.clone(),
        )],
        &[],
    );

    let expired_at = fixture.advert.expires_at.saturating_add(1);
    let err = mesh
        .publish("alpha", &fixture.advert, expired_at)
        .expect_err("expired advert must be rejected");
    assert!(
        err.contains("advert expired"),
        "expected expiration error, got: {err}"
    );
    assert_eq!(mesh.node("alpha").stored_count(), 0);

    mesh.publish("alpha", &fixture.advert, ISSUED_AT + 5)
        .expect("fresh advert accepted");
    assert_eq!(mesh.node("alpha").stored_count(), 1);
    mesh.publish("alpha", &fixture.advert, ISSUED_AT + 6)
        .expect("duplicate advert treated as no-op");
    assert_eq!(
        mesh.node("alpha").stored_count(),
        1,
        "duplicate advert must not increment store count"
    );
}

#[test]
fn torii_mesh_rejects_invalid_path_policy() {
    let signing_key = SigningKey::from_bytes(&[0xBA; 32]);
    let fixture = make_signed_advert(
        &signing_key,
        [0x41; 32],
        [0x51; 32],
        vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        false,
    );
    let mut advert = fixture.advert.clone();
    advert.body.path_policy.min_guard_weight = 0;

    let registry = admission_registry_from_fixtures(&[fixture]);

    let mut mesh = ToriiMesh::with_edges(
        vec![ToriiNode::new(
            "alpha",
            &[CapabilityType::ToriiGateway],
            registry.clone(),
        )],
        &[],
    );
    let err = mesh
        .publish("alpha", &advert, ISSUED_AT + 10)
        .expect_err("invalid path policy must fail");
    assert!(
        err.contains("path diversity"),
        "expected path diversity failure, got: {err}"
    );
    assert_eq!(mesh.node("alpha").stored_count(), 0);
}

struct ToriiNode {
    id: &'static str,
    cache: ProviderAdvertCache,
    rejections: Vec<String>,
}

impl ToriiNode {
    fn new(id: &'static str, known: &[CapabilityType], admission: Arc<AdmissionRegistry>) -> Self {
        Self {
            id,
            cache: ProviderAdvertCache::new(known.iter().copied(), admission),
            rejections: Vec::new(),
        }
    }

    fn ingest(
        &mut self,
        advert: &ProviderAdvertV1,
        now: u64,
    ) -> Result<AdvertIngestResult, String> {
        match self.cache.ingest(advert.clone(), now) {
            Ok(result) => Ok(result),
            Err(err) => {
                let reason = err.to_string();
                self.rejections.push(reason.clone());
                Err(reason)
            }
        }
    }

    fn stored_count(&self) -> usize {
        self.cache.len()
    }

    fn capabilities_for(&self, provider_id: &[u8; 32]) -> Option<Vec<CapabilityType>> {
        self.cache
            .record_by_provider(provider_id)
            .map(|record| record.known_capabilities().to_vec())
    }

    #[allow(dead_code)]
    fn warnings_for(&self, provider_id: &[u8; 32]) -> Option<Vec<AdvertWarning>> {
        self.cache
            .record_by_provider(provider_id)
            .map(|record| record.warnings().to_vec())
    }

    fn rejection_reasons(&self) -> &[String] {
        &self.rejections
    }
}

struct ToriiMesh {
    nodes: HashMap<&'static str, ToriiNode>,
    adjacency: HashMap<&'static str, Vec<&'static str>>,
}

impl ToriiMesh {
    fn with_edges(nodes: Vec<ToriiNode>, edges: &[(&'static str, &'static str)]) -> Self {
        let mut adjacency: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
        for &(a, b) in edges {
            adjacency.entry(a).or_default().push(b);
            adjacency.entry(b).or_default().push(a);
        }
        let mut node_map = HashMap::new();
        for node in nodes {
            adjacency.entry(node.id).or_default();
            node_map.insert(node.id, node);
        }
        Self {
            nodes: node_map,
            adjacency,
        }
    }

    fn publish(
        &mut self,
        origin: &'static str,
        advert: &ProviderAdvertV1,
        now: u64,
    ) -> Result<(), String> {
        let node = self
            .nodes
            .get_mut(origin)
            .ok_or_else(|| format!("node {origin} not registered"))?;
        match node.ingest(advert, now) {
            Ok(result) => match result.outcome {
                AdvertIngest::Stored { .. } | AdvertIngest::Replaced { .. } => {}
                AdvertIngest::Duplicate { .. } => return Ok(()),
            },
            Err(err) => return Err(err),
        }

        let mut queue = VecDeque::new();
        queue.push_back((origin, advert.clone()));
        let mut visited: HashSet<&'static str> = HashSet::new();
        visited.insert(origin);

        while let Some((current, advert_payload)) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(current).cloned() {
                for neighbor in neighbors {
                    let result = self
                        .nodes
                        .get_mut(neighbor)
                        .ok_or_else(|| format!("node {neighbor} not registered"))?
                        .ingest(&advert_payload, now);
                    if let Ok(result) = result {
                        match result.outcome {
                            AdvertIngest::Stored { .. } | AdvertIngest::Replaced { .. } => {
                                if visited.insert(neighbor) {
                                    queue.push_back((neighbor, advert_payload.clone()));
                                }
                            }
                            AdvertIngest::Duplicate { .. } => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn node(&self, id: &'static str) -> &ToriiNode {
        self.nodes.get(id).expect("node registered in mesh")
    }
}

#[allow(clippy::too_many_lines)]
fn make_signed_advert(
    signing_key: &SigningKey,
    provider_id: [u8; 32],
    stake_pool_id: [u8; 32],
    capabilities: Vec<CapabilityTlv>,
    allow_unknown_capabilities: bool,
) -> ProviderFixture {
    let has_chunk_range = capabilities
        .iter()
        .any(|cap| cap.cap_type == CapabilityType::ChunkRangeFetch);
    let has_soranet = capabilities
        .iter()
        .any(|cap| cap.cap_type == CapabilityType::SoraNetHybridPq);
    let stream_budget = has_chunk_range.then_some(StreamBudgetV1 {
        max_in_flight: 8,
        max_bytes_per_sec: 9_000_000,
        burst_bytes: Some(4_500_000),
    });
    let transport_hints = {
        let mut hints = if has_chunk_range {
            vec![
                TransportHintV1 {
                    protocol: TransportProtocol::ToriiHttpRange,
                    priority: 0,
                },
                TransportHintV1 {
                    protocol: TransportProtocol::QuicStream,
                    priority: 1,
                },
            ]
        } else {
            Vec::new()
        };
        if has_soranet {
            let priority = u8::try_from(hints.len()).expect("transport hint priority fits in u8");
            hints.push(TransportHintV1 {
                protocol: TransportProtocol::SoraNetRelay,
                priority,
            });
        }
        if hints.is_empty() { None } else { Some(hints) }
    };
    let body = ProviderAdvertBodyV1 {
        provider_id,
        profile_id: "sorafs.sf1@1.0.0".to_owned(),
        profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
        stake: StakePointer {
            pool_id: stake_pool_id,
            stake_amount: 5_000_000,
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 1_200,
            max_concurrent_streams: 32,
        },
        capabilities,
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "storage.example.com".to_owned(),
            metadata: vec![EndpointMetadata {
                key: EndpointMetadataKey::Region,
                value: b"global".to_vec(),
            }],
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".to_owned(),
            region: "global".to_owned(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 5,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget,
        transport_hints: transport_hints.clone(),
    };
    body.validate().expect("test advert body must validate");
    let body_clone = body.clone();

    let body_bytes = norito::to_bytes(&body).expect("serialize body for signing");
    let signature = signing_key.sign(&body_bytes);
    let advert = ProviderAdvertV1 {
        version: PROVIDER_ADVERT_VERSION_V1,
        issued_at: ISSUED_AT,
        expires_at: ISSUED_AT
            .checked_add(TTL_SECS)
            .expect("ttl addition must not overflow"),
        body,
        signature: sorafs_manifest::AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        },
        signature_strict: true,
        allow_unknown_capabilities,
    };

    let attestation = EndpointAttestationV1 {
        version: ENDPOINT_ATTESTATION_VERSION_V1,
        kind: EndpointAttestationKind::Mtls,
        attested_at: ISSUED_AT.saturating_sub(300),
        expires_at: ISSUED_AT + TTL_SECS + 1_200,
        leaf_certificate: vec![0xAA, 0xBB, 0xCC, 0xDD],
        intermediate_certificates: Vec::new(),
        alpn_ids: vec!["h2".to_owned()],
        report: Vec::new(),
    };

    let proposal = ProviderAdmissionProposalV1 {
        version: sorafs_manifest::PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
        provider_id,
        profile_id: body_clone.profile_id.clone(),
        profile_aliases: body_clone.profile_aliases.clone(),
        stake: body_clone.stake,
        capabilities: body_clone.capabilities.clone(),
        endpoints: vec![EndpointAdmissionV1 {
            endpoint: body_clone
                .endpoints
                .first()
                .expect("fixture endpoint")
                .clone(),
            attestation,
        }],
        advert_key: signing_key.verifying_key().to_bytes(),
        jurisdiction_code: "US".to_owned(),
        contact_uri: Some("mailto:ops@example.com".to_owned()),
        stream_budget,
        transport_hints: transport_hints.clone(),
    };

    let proposal_digest =
        compute_proposal_digest(&proposal).expect("compute proposal digest for fixture");
    let advert_body_digest =
        compute_advert_body_digest(&body_clone).expect("compute advert body digest");

    let council_key = SigningKey::from_bytes(&[0x42; 32]);
    let council_signature = council_key.sign(&proposal_digest);
    let envelope = ProviderAdmissionEnvelopeV1 {
        version: sorafs_manifest::PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
        proposal,
        proposal_digest,
        advert_body: body_clone,
        advert_body_digest,
        issued_at: ISSUED_AT,
        retention_epoch: ISSUED_AT + TTL_SECS + 3_600,
        council_signatures: vec![CouncilSignature {
            signer: council_key.verifying_key().to_bytes(),
            signature: council_signature.to_bytes().to_vec(),
        }],
        notes: None,
    };

    ProviderFixture { advert, envelope }
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/sorafs_manifest/provider_admission")
}

fn fixture_from_disk(advert_name: &str, envelope_name: &str) -> ProviderFixture {
    let advert_path = fixtures_root().join(advert_name);
    let envelope_path = fixtures_root().join(envelope_name);
    let advert_bytes = fs::read(&advert_path).unwrap_or_else(|err| {
        panic!(
            "failed to read advert fixture {}: {err}",
            advert_path.display()
        )
    });
    let envelope_bytes = fs::read(&envelope_path).unwrap_or_else(|err| {
        panic!(
            "failed to read envelope fixture {}: {err}",
            envelope_path.display()
        )
    });
    let advert =
        decode_from_bytes(&advert_bytes).expect("decode provider advert fixture from disk");
    let envelope =
        decode_from_bytes(&envelope_bytes).expect("decode provider envelope fixture from disk");
    ProviderFixture { advert, envelope }
}

fn admission_registry_from_fixtures(fixtures: &[ProviderFixture]) -> Arc<AdmissionRegistry> {
    let envelopes = fixtures
        .iter()
        .map(|fixture| fixture.envelope.clone())
        .collect::<Vec<_>>();
    let registry = AdmissionRegistry::from_envelopes(envelopes)
        .expect("fixture admission registry must be valid");
    Arc::new(registry)
}

struct ToriiHarness {
    #[allow(dead_code)]
    torii: Torii,
    app: Router,
    #[allow(dead_code)]
    kiso_child: Child,
    state: Arc<State>,
    queue: Arc<CoreQueue>,
    chain_id: Arc<ChainId>,
    alias_policy: actual_cfg::SorafsAliasCachePolicy,
}

fn build_torii_harness(cfg: &actual_cfg::Root) -> ToriiHarness {
    let (kiso, kiso_child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query.clone(),
    ));
    if let Some(inner) = Arc::get_mut(&mut state) {
        inner.chain_id = cfg.common.chain.clone();
    }
    let queue_cfg = actual_cfg::Queue::default();
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(CoreQueue::from_config(queue_cfg, queue_events));

    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let chain_id = ChainId::from("test-chain");
    let chain_id_arc = Arc::new(chain_id.clone());
    let torii = Torii::new_with_handle(
        chain_id,
        kiso,
        cfg.torii.clone(),
        Arc::clone(&queue),
        tokio::sync::broadcast::channel(1).0,
        query,
        kura,
        Arc::clone(&state),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );
    let app = torii.api_router_for_tests();
    let alias_policy = cfg.torii.sorafs_alias_cache;

    ToriiHarness {
        torii,
        app,
        kiso_child,
        state,
        queue,
        chain_id: chain_id_arc,
        alias_policy,
    }
}

struct ManifestSetup {
    manifest_digest: RegistryManifestDigest,
    manifest_digest_hex: String,
    authority: AuthorityCreds,
    approved_epoch: u64,
    retention_epoch: u64,
    manifest_cid: Vec<u8>,
}

struct ManifestRequestFixture {
    request: RegisterPinManifestDto,
    manifest_digest_hex: String,
}

fn submit_transaction(harness: &ToriiHarness, tx: SignedTransaction, next_height: &mut u64) {
    let accepted_tx = AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
    harness
        .queue
        .push_with_lane(accepted_tx, harness.state.view())
        .expect("push transaction into queue");
    let applied = drain_queue_and_apply_all(
        &harness.state,
        &harness.queue,
        harness.chain_id.as_ref(),
        *next_height,
    );
    assert!(
        applied > 0,
        "expected transaction to apply at height {next_height}"
    );
    println!(
        "APPLIED {} transactions at height {}",
        applied, *next_height
    );
    *next_height += 1;
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
    let expires_at_unix = if expires_at_hint <= generated_at_unix {
        generated_at_unix.saturating_add(TTL_SECS)
    } else {
        expires_at_hint
    };
    let mut bundle = AliasProofBundleV1 {
        binding,
        registry_root: [0u8; 32],
        registry_height: 1,
        generated_at_unix,
        expires_at_unix,
        merkle_path: Vec::new(),
        council_signatures: Vec::new(),
    };
    let root =
        alias_merkle_root(&bundle.binding, &bundle.merkle_path).expect("compute alias proof root");
    bundle.registry_root = root;
    let digest = alias_proof_signature_digest(&bundle);
    let keypair = KeyPair::from_private_key(
        PrivateKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &[0x33; 32]).expect("seeded key"),
    )
    .expect("derive keypair");
    let signature = iroha_crypto::Signature::new(keypair.private_key(), digest.as_ref());
    let (_, signer_bytes) = keypair.public_key().to_bytes();
    let signer: [u8; 32] = signer_bytes
        .try_into()
        .expect("ed25519 public key must be 32 bytes");
    bundle.council_signatures.push(CouncilSignature {
        signer,
        signature: signature.payload().to_vec(),
    });
    to_bytes(&bundle).expect("encode alias proof bundle")
}

fn manifest_request_fixture<F>(submitted_epoch: u64, tweak: F) -> ManifestRequestFixture
where
    F: FnOnce(&mut RegisterPinManifestDto),
{
    let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
    let manifest_policy = PinPolicy {
        min_replicas: 3,
        storage_class: ManifestStorageClass::Hot,
        retention_epoch: 48,
    };
    let manifest = ManifestBuilder::new()
        .root_cid(vec![0x10, 0x20, 0x30])
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_from_registry(ProfileId(descriptor.id.0))
        .content_length(1_024)
        .car_digest([0xAA; 32])
        .car_size(4_096)
        .pin_policy(manifest_policy)
        .governance(GovernanceProofs {
            council_signatures: vec![CouncilSignature {
                signer: [0x01; 32],
                signature: vec![0x02; 64],
            }],
        })
        .build()
        .expect("manifest must build");
    let manifest_digest = manifest.digest().expect("manifest digest");
    let manifest_digest_hex = hex::encode(manifest_digest.as_bytes());
    let pin_policy_dto = PinPolicyDto {
        min_replicas: manifest_policy.min_replicas,
        storage_class: match manifest_policy.storage_class {
            ManifestStorageClass::Hot => PinPolicyStorageClassDto::Hot,
            ManifestStorageClass::Warm => PinPolicyStorageClassDto::Warm,
            ManifestStorageClass::Cold => PinPolicyStorageClassDto::Cold,
        },
        retention_epoch: manifest_policy.retention_epoch,
    };
    let key_pair = KeyPair::random();
    let authority = dm::AccountId::of(
        "nexus".parse().expect("valid domain"),
        key_pair.public_key().clone(),
    );
    let mut request = RegisterPinManifestDto {
        authority,
        private_key: dm::ExposedPrivateKey(key_pair.private_key().clone()),
        chunker_profile_id: descriptor.id.0,
        chunker_namespace: descriptor.namespace.to_string(),
        chunker_name: descriptor.name.to_string(),
        chunker_semver: descriptor.semver.to_string(),
        chunker_multihash_code: descriptor.multihash_code,
        pin_policy: pin_policy_dto,
        manifest_digest_hex: manifest_digest_hex.clone(),
        chunk_digest_sha3_256_hex: hex::encode([0xCD; 32]),
        submitted_epoch,
        alias: None,
        successor_of_hex: None,
    };
    tweak(&mut request);
    ManifestRequestFixture {
        request,
        manifest_digest_hex,
    }
}

fn create_manifest_setup(harness: &ToriiHarness, next_height: &mut u64) -> ManifestSetup {
    create_manifest_setup_with_seed(harness, next_height, 0xAB, None, None)
}

#[allow(clippy::too_many_lines)]
fn create_manifest_setup_with_seed(
    harness: &ToriiHarness,
    next_height: &mut u64,
    seed: u8,
    successor_of: Option<RegistryManifestDigest>,
    status_timestamp_unix: Option<u64>,
) -> ManifestSetup {
    let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
    let manifest_policy_registry = RegistryPinPolicy {
        min_replicas: 3,
        storage_class: RegistryStorageClass::Hot,
        retention_epoch: 48,
    };
    let manifest_policy_manifest = PinPolicy {
        min_replicas: manifest_policy_registry.min_replicas,
        storage_class: ManifestStorageClass::Hot,
        retention_epoch: manifest_policy_registry.retention_epoch,
    };
    let manifest_cid = vec![seed, seed ^ 0x5A, seed ^ 0xA5, seed.wrapping_add(1)];
    let manifest = ManifestBuilder::new()
        .root_cid(manifest_cid.clone())
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_from_registry(ProfileId(descriptor.id.0))
        .content_length(1_024)
        .car_digest([seed.wrapping_add(31); 32])
        .car_size(4_096)
        .pin_policy(manifest_policy_manifest)
        .governance(GovernanceProofs {
            council_signatures: vec![CouncilSignature {
                signer: [0x11; 32],
                signature: vec![0x22; 64],
            }],
        })
        .build()
        .expect("build manifest");
    let manifest_digest_value = manifest.digest().expect("manifest digest");
    let manifest_digest_bytes = *manifest_digest_value.as_bytes();
    let manifest_digest_hex = hex::encode(manifest_digest_bytes);
    let manifest_digest = RegistryManifestDigest::new(manifest_digest_bytes);
    let chunker_handle = ChunkerProfileHandle {
        profile_id: descriptor.id.0,
        namespace: descriptor.namespace.to_string(),
        name: descriptor.name.to_string(),
        semver: descriptor.semver.to_string(),
        multihash_code: descriptor.multihash_code,
    };
    let authority = random_authority();
    ensure_authority_registered(harness, &authority, next_height);
    let submitted_epoch = 12;
    let approved_epoch = 18;

    let register = RegisterPinManifest {
        digest: manifest_digest,
        chunker: chunker_handle,
        chunk_digest_sha3_256: [seed.wrapping_add(0x33); 32],
        policy: manifest_policy_registry,
        submitted_epoch,
        alias: None,
        successor_of,
    };
    let chain_id = harness.chain_id.as_ref().clone();
    let register_tx = TransactionBuilder::new(chain_id.clone(), authority.account.clone())
        .with_instructions([dm::InstructionBox::from(register)])
        .sign(&authority.private_key.0);
    submit_transaction(harness, register_tx, next_height);

    let approve = ApprovePinManifest {
        digest: manifest_digest,
        approved_epoch,
        council_envelope: None,
        council_envelope_digest: Some([0x99; 32]),
    };
    let approve_tx = TransactionBuilder::new(chain_id, authority.account.clone())
        .with_instructions([dm::InstructionBox::from(approve)])
        .sign(&authority.private_key.0);
    submit_transaction(harness, approve_tx, next_height);

    if let Some(timestamp) = status_timestamp_unix {
        let prev_hash = harness
            .state
            .view()
            .latest_block()
            .map(|block| block.hash());
        let header = BlockHeader::new(
            NonZeroU64::new(*next_height).expect("block height fits into NonZeroU64"),
            prev_hash,
            None,
            None,
            *next_height,
            0,
        );
        let mut block = harness.state.block(header);
        let mut tx = block.transaction();
        {
            let world = tx.world_mut_for_testing();
            let manifests = world.pin_manifests_mut_for_testing();
            let record = manifests
                .get_mut(&manifest_digest)
                .expect("manifest must exist before metadata update");
            let name = Name::from_str(STATUS_TIMESTAMP_KEY).expect("timestamp key");
            record.metadata.insert(
                name,
                Json::from(json::to_value(&timestamp).expect("timestamp serializes")),
            );
        }
        tx.apply();
        block
            .commit()
            .expect("commit manifest status timestamp block");
        *next_height += 1;
    }

    let view = harness.state.view();
    let manifests_store = view.world().pin_manifests();
    assert!(
        manifests_store.get(&manifest_digest).is_some(),
        "manifest must exist in registry after registration"
    );

    ManifestSetup {
        manifest_digest,
        manifest_digest_hex,
        authority,
        approved_epoch,
        retention_epoch: manifest_policy_registry.retention_epoch,
        manifest_cid,
    }
}

fn create_successor_manifest(
    harness: &ToriiHarness,
    predecessor: &ManifestSetup,
    next_height: &mut u64,
    seed: u8,
    status_timestamp_unix: u64,
) -> ManifestSetup {
    create_manifest_setup_with_seed(
        harness,
        next_height,
        seed,
        Some(predecessor.manifest_digest),
        Some(status_timestamp_unix),
    )
}

fn ensure_authority_registered(
    harness: &ToriiHarness,
    authority: &AuthorityCreds,
    next_height: &mut u64,
) {
    let view = harness.state.view();
    if view.world().accounts().get(&authority.account).is_some() {
        return;
    }
    let domain_id = authority.account.domain().clone();
    drop(view);

    let prev_hash = harness
        .state
        .view()
        .latest_block()
        .map(|block| block.hash());
    let header = BlockHeader::new(
        NonZeroU64::new(*next_height).expect("block height fits into NonZeroU64"),
        prev_hash,
        None,
        None,
        *next_height,
        0,
    );
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();

    if tx.world().domain(&domain_id).is_err() {
        let new_domain = dm::Domain::new(domain_id.clone());
        let register_domain = dm::Register::domain(new_domain);
        register_domain
            .execute(&authority.account, &mut tx)
            .expect("register domain for test authority");
    }

    let new_account = dm::Account::new(authority.account.clone());
    let register_account = dm::Register::account(new_account);
    register_account
        .execute(&authority.account, &mut tx)
        .expect("register account for test authority");

    tx.apply();
    block.commit().expect("commit authority registration block");
}

fn attach_governance_revocation(
    harness: &ToriiHarness,
    setup: &ManifestSetup,
    alias_label: &str,
    effective_at_unix: u64,
    next_height: &mut u64,
) {
    let prev_hash = harness
        .state
        .view()
        .latest_block()
        .map(|block| block.hash());
    let header = BlockHeader::new(
        NonZeroU64::new(*next_height).expect("block height fits into NonZeroU64"),
        prev_hash,
        None,
        None,
        *next_height,
        0,
    );
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    {
        let world = tx.world_mut_for_testing();
        let manifests = world.pin_manifests_mut_for_testing();
        let record = manifests
            .get_mut(&setup.manifest_digest)
            .expect("manifest must exist before governance update");
        let key = Name::from_str(GOVERNANCE_REFS_KEY).expect("governance key");
        let mut targets = json::Map::new();
        targets.insert("alias".into(), json::Value::String(alias_label.to_string()));
        targets.insert(
            "pin_digest_hex".into(),
            json::Value::String(setup.manifest_digest_hex.clone()),
        );

        let mut entry = json::Map::new();
        entry.insert(
            "kind".into(),
            json::Value::String("RevokeManifest".to_owned()),
        );
        entry.insert(
            "effective_at".into(),
            json::to_value(&effective_at_unix).expect("effective_at serializes"),
        );
        entry.insert("targets".into(), json::Value::Object(targets));
        entry.insert(
            "signers".into(),
            json::Value::Array(vec![json::Value::String("council-1".to_owned())]),
        );

        let value = json::Value::Array(vec![json::Value::Object(entry)]);
        record.metadata.insert(key, Json::from(value));
        let status_key = Name::from_str(STATUS_TIMESTAMP_KEY).expect("status timestamp key");
        record.metadata.insert(
            status_key,
            Json::from(
                json::to_value(&effective_at_unix).expect("status timestamp serializes to json"),
            ),
        );
    }
    tx.apply();
    block.commit().expect("commit governance metadata block");
    *next_height += 1;
}

#[allow(clippy::too_many_arguments)]
fn bind_alias_with_proof(
    harness: &ToriiHarness,
    setup: &ManifestSetup,
    alias_namespace: &str,
    alias_name: &str,
    generated_at_unix: u64,
    expires_at_unix: u64,
    bound_epoch: u64,
    expiry_epoch: u64,
    next_height: &mut u64,
) {
    let proof = encode_alias_proof_bytes(
        alias_namespace,
        alias_name,
        &setup.manifest_cid,
        bound_epoch,
        expiry_epoch,
        generated_at_unix,
        expires_at_unix,
    );
    let binding = ManifestAliasBinding {
        name: alias_name.to_owned(),
        namespace: alias_namespace.to_owned(),
        proof,
    };
    let alias_record = ManifestAliasRecord::new(
        binding.clone(),
        setup.manifest_digest,
        setup.authority.account.clone(),
        bound_epoch,
        expiry_epoch,
    );

    let prev_hash = harness
        .state
        .view()
        .latest_block()
        .map(|block| block.hash());
    let header = BlockHeader::new(
        NonZeroU64::new(*next_height).unwrap_or_else(|| NonZeroU64::new(1).unwrap()),
        prev_hash,
        None,
        None,
        *next_height,
        0,
    );
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();

    {
        let world = tx.world_mut_for_testing();
        {
            let manifests = world.pin_manifests_mut_for_testing();
            let record = manifests
                .get_mut(&setup.manifest_digest)
                .expect("manifest must exist before binding alias");
            record.alias = Some(binding.clone());
        }

        {
            let aliases = world.manifest_aliases_mut_for_testing();
            aliases.insert(ManifestAliasId::from(&binding), alias_record);
        }
    }

    tx.apply();
    block.commit().expect("commit alias binding block");
}

#[tokio::test]
async fn sorafs_routes_disabled_when_cache_off() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = false;
    cfg.torii.sorafs_discovery.admission = None;

    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/sorafs/providers")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sorafs_routes_enabled_with_admission_dir() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let temp = tempdir().expect("temp dir");
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: temp.path().to_path_buf(),
    });

    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sorafs/providers")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: json::Value = json::from_slice(&body).expect("valid JSON");
    assert_eq!(
        payload
            .get("count")
            .and_then(json::Value::as_u64)
            .expect("count present"),
        0
    );
    assert!(
        payload
            .get("providers")
            .and_then(json::Value::as_array)
            .expect("providers array")
            .is_empty()
    );
}

#[tokio::test]
async fn sorafs_capacity_route_disabled_when_storage_off() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_storage.enabled = false;

    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/sorafs/capacity/state")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sorafs_capacity_route_enabled_when_storage_on() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_storage.enabled = true;

    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/sorafs/capacity/state")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: json::Value = json::from_slice(&body).expect("valid JSON");
    assert_eq!(
        payload
            .get("declaration_count")
            .and_then(json::Value::as_u64)
            .expect("declaration_count present"),
        0
    );
    assert!(
        payload
            .get("declarations")
            .and_then(json::Value::as_array)
            .expect("declarations array")
            .is_empty()
    );
    assert_eq!(
        payload
            .get("ledger_count")
            .and_then(json::Value::as_u64)
            .expect("ledger_count present"),
        0
    );
    assert!(
        payload
            .get("fee_ledger")
            .and_then(json::Value::as_array)
            .expect("fee_ledger array")
            .is_empty()
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sorafs_storage_endpoints_round_trip() {
    if !ingest_tests_enabled() {
        eprintln!("skipping storage round-trip (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    cfg.torii.sorafs_gateway.enforce_capabilities = false;
    cfg.torii.sorafs_gateway.enforce_admission = false;
    cfg.torii.sorafs_gateway.require_manifest_envelope = false;
    let temp_dir = tempdir().expect("storage temp dir");
    cfg.torii.sorafs_storage.data_dir = temp_dir.path().join("storage");
    let expected_capacity = cfg.torii.sorafs_storage.max_capacity_bytes.0;

    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let payload = b"torii sorafs storage round-trip payload";
    let manifest = ManifestBuilder::new()
        .root_cid(vec![0xAA; 16])
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(
            sorafs_chunker::ChunkProfile::DEFAULT,
            BLAKE3_256_MULTIHASH_CODE,
        )
        .content_length(payload.len() as u64)
        .car_digest(blake3::hash(payload).into())
        .car_size(payload.len() as u64)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");

    let manifest_b64 = base64::engine::general_purpose::STANDARD
        .encode(to_bytes(&manifest).expect("encode manifest"));
    let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);

    let pin_body = {
        let mut map = json::Map::new();
        map.insert(
            "manifest_b64".to_owned(),
            json::Value::from(manifest_b64.clone()),
        );
        map.insert(
            "payload_b64".to_owned(),
            json::Value::from(payload_b64.clone()),
        );
        json::Value::Object(map)
    };
    let pin_request = Request::builder()
        .method("POST")
        .uri("/v1/sorafs/storage/pin")
        .header("content-type", "application/json")
        .body(Body::from(
            json::to_vec(&pin_body).expect("serialize pin request"),
        ))
        .expect("pin request");
    let pin_response = app
        .clone()
        .oneshot(pin_request)
        .await
        .expect("pin response");
    let pin_status = pin_response.status();
    let pin_bytes = BodyExt::collect(pin_response.into_body())
        .await
        .expect("collect pin body")
        .to_bytes();
    assert!(
        pin_status == StatusCode::OK,
        "pin request failed: {pin_status} body={}",
        String::from_utf8_lossy(&pin_bytes)
    );
    let pin_value: json::Value = json::from_slice(&pin_bytes).expect("decode pin response");
    let manifest_id = pin_value
        .get("manifest_id_hex")
        .and_then(json::Value::as_str)
        .expect("manifest id")
        .to_string();
    assert_eq!(
        pin_value
            .get("content_length")
            .and_then(json::Value::as_u64)
            .expect("content length"),
        payload.len() as u64
    );

    let fetch_body = {
        let mut map = json::Map::new();
        map.insert(
            "manifest_id_hex".to_owned(),
            json::Value::from(manifest_id.clone()),
        );
        map.insert("offset".to_owned(), json::Value::from(0u64));
        map.insert("length".to_owned(), json::Value::from(payload.len() as u64));
        json::Value::Object(map)
    };
    let fetch_request = Request::builder()
        .method("POST")
        .uri("/v1/sorafs/storage/fetch")
        .header("content-type", "application/json")
        .header("x-sorafs-manifest-envelope", "dummy-envelope")
        .body(Body::from(
            json::to_vec(&fetch_body).expect("serialize fetch request"),
        ))
        .expect("fetch request");
    let mut fetch_request = fetch_request;
    fetch_request
        .extensions_mut()
        .insert(ConnectInfo::<SocketAddr>(SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    let fetch_response = app
        .clone()
        .oneshot(fetch_request)
        .await
        .expect("fetch response");
    let fetch_status = fetch_response.status();
    let fetch_bytes = BodyExt::collect(fetch_response.into_body())
        .await
        .expect("collect fetch body")
        .to_bytes();
    assert!(
        fetch_status == StatusCode::OK,
        "fetch request failed: {fetch_status} body={}",
        String::from_utf8_lossy(&fetch_bytes)
    );
    let fetch_value: json::Value = json::from_slice(&fetch_bytes).expect("decode fetch response");
    let fetched_payload = fetch_value
        .get("data_b64")
        .and_then(json::Value::as_str)
        .map(|data| {
            base64::engine::general_purpose::STANDARD
                .decode(data.as_bytes())
                .expect("decode fetch payload")
        })
        .expect("fetch payload");
    assert_eq!(fetched_payload.as_slice(), payload);

    let por_body = {
        let mut map = json::Map::new();
        map.insert(
            "manifest_id_hex".to_owned(),
            json::Value::from(manifest_id.clone()),
        );
        map.insert("count".to_owned(), json::Value::from(2u64));
        map.insert("seed".to_owned(), json::Value::from(7u64));
        json::Value::Object(map)
    };
    let por_request = Request::builder()
        .method("POST")
        .uri("/v1/sorafs/storage/por-sample")
        .header("content-type", "application/json")
        .body(Body::from(
            json::to_vec(&por_body).expect("serialize por request"),
        ))
        .expect("por request");
    let por_response = app
        .clone()
        .oneshot(por_request)
        .await
        .expect("por response");
    let por_status = por_response.status();
    let por_bytes = BodyExt::collect(por_response.into_body())
        .await
        .expect("collect por body")
        .to_bytes();
    assert!(
        por_status == StatusCode::OK,
        "por sample failed: {por_status} body={}",
        String::from_utf8_lossy(&por_bytes)
    );
    let por_value: json::Value = json::from_slice(&por_bytes).expect("decode por response");
    let por_samples = por_value
        .get("samples")
        .and_then(json::Value::as_array)
        .expect("samples array");
    assert!(
        !por_samples.is_empty(),
        "por-sample endpoint should return at least one sample"
    );
    let por_sample_count = por_samples.len() as u64;

    let mut chunk_store = ChunkStore::new();
    chunk_store.ingest_bytes(payload);
    let expected_root = *chunk_store.por_tree().root();

    let proof_body = {
        let mut map = json::Map::new();
        map.insert(
            "manifest_id_hex".to_owned(),
            json::Value::from(manifest_id.clone()),
        );
        map.insert(
            "provider_id_hex".to_owned(),
            json::Value::from("22".repeat(32)),
        );
        map.insert("proof_kind".to_owned(), json::Value::from("por"));
        map.insert("sample_count".to_owned(), json::Value::from(2u32));
        map.insert("sample_seed".to_owned(), json::Value::from(7u64));
        map.insert(
            "nonce_b64".to_owned(),
            json::Value::from(BASE64_STANDARD.encode([0x44u8; 16])),
        );
        map.insert("tier".to_owned(), json::Value::from("hot"));
        json::Value::Object(map)
    };

    let proof_request = Request::builder()
        .method("POST")
        .uri("/v1/sorafs/proof/stream")
        .header("content-type", "application/json")
        .body(Body::from(
            json::to_vec(&proof_body).expect("serialize proof stream request"),
        ))
        .expect("proof request");
    let proof_response = app
        .clone()
        .oneshot(proof_request)
        .await
        .expect("proof response");
    assert_eq!(proof_response.status(), StatusCode::OK);
    let proof_bytes = BodyExt::collect(proof_response.into_body())
        .await
        .expect("collect proof stream body")
        .to_bytes();
    let proof_text =
        String::from_utf8(proof_bytes.to_vec()).expect("proof stream response is UTF-8");
    let mut proof_items = 0usize;
    for line in proof_text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        proof_items += 1;
        let value: json::Value = json::from_str(line).expect("parse proof stream item");
        assert_eq!(
            value.get("manifest_id_hex").and_then(json::Value::as_str),
            Some(manifest_id.as_str()),
            "manifest id echoed in stream item"
        );
        assert_eq!(
            value.get("proof_kind").and_then(json::Value::as_str),
            Some("por")
        );
        let proof_value = value
            .get("proof")
            .unwrap_or_else(|| panic!("stream item must include proof field: {value:?}"));
        let proof =
            proof_from_value(proof_value).expect("convert JSON proof into PorProof structure");
        assert!(
            proof.verify(&expected_root),
            "streamed proof should verify against computed PoR root"
        );
    }
    assert!(proof_items > 0, "proof stream should return items");

    let state_request = Request::builder()
        .method("GET")
        .uri("/v1/sorafs/storage/state")
        .body(Body::empty())
        .expect("state request");
    let state_response = app.oneshot(state_request).await.expect("state response");
    assert_eq!(state_response.status(), StatusCode::OK);
    let state_bytes = BodyExt::collect(state_response.into_body())
        .await
        .expect("collect state body")
        .to_bytes();
    let state: StorageStateResponseDto =
        json::from_slice(&state_bytes).expect("decode state response");

    assert_eq!(state.bytes_used, payload.len() as u64);
    assert_eq!(state.bytes_capacity, expected_capacity);
    assert_eq!(state.pin_queue_depth, 0);
    assert_eq!(state.fetch_inflight, 0);
    assert!(state.fetch_bytes_per_sec > 0);
    assert_eq!(state.por_inflight, 0);
    assert!(
        state.por_samples_success_total >= por_sample_count,
        "expected scheduler to record at least {por_sample_count} successful samples"
    );
    assert_eq!(state.por_samples_failed_total, 0);
    assert_eq!(state.fetch_utilisation_bps, 0);
    assert_eq!(state.pin_queue_utilisation_bps, 0);
    assert_eq!(state.por_utilisation_bps, 0);
}

#[tokio::test]
async fn sorafs_pin_register_route_accepts_manifest() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.transport.norito_rpc.enabled = true;
    cfg.torii.transport.norito_rpc.stage = actual_cfg::NoritoRpcStage::Ga;
    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let fixture = manifest_request_fixture(7, |_| {});

    let payload = norito::json::to_vec(&fixture.request).expect("serialize request");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sorafs/pin/register")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .expect("build request"),
        )
        .await
        .expect("router responds");
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = BodyExt::collect(response.into_body())
        .await
        .expect("collect response body")
        .to_bytes();
    let value: json::Value = json::from_slice(&bytes).expect("decode response");
    assert_eq!(
        value
            .get("manifest_digest_hex")
            .and_then(json::Value::as_str),
        Some(fixture.manifest_digest_hex.as_str()),
        "response should echo manifest digest"
    );
    assert_eq!(
        value.get("submitted_epoch").and_then(json::Value::as_u64),
        Some(7),
        "response should include submitted epoch"
    );
}

#[tokio::test]
async fn sorafs_pin_register_rejects_invalid_alias_proof() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let fixture = manifest_request_fixture(9, |req| {
        req.alias = Some(iroha_torii::PinAliasDto {
            namespace: "sora".into(),
            name: "bad-proof".into(),
            proof_base64: "%not_base64%".into(),
        });
    });

    let payload = norito::json::to_vec(&fixture.request).expect("serialize request");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sorafs/pin/register")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .expect("build request"),
        )
        .await
        .expect("router responds");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let message = String::from_utf8_lossy(&body);
    assert!(
        message.contains("invalid base64"),
        "error message should mention base64 decoding failure: {message}"
    );
}

#[tokio::test]
async fn sorafs_pin_register_accepts_norito_payload() {
    if !ingest_tests_enabled() {
        eprintln!("skipping pin register Norito payload (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.transport.norito_rpc.enabled = true;
    cfg.torii.transport.norito_rpc.stage = actual_cfg::NoritoRpcStage::Ga;
    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let fixture = manifest_request_fixture(8, |_| {});
    let norito_payload =
        norito::to_bytes(&fixture.request).expect("encode pin manifest request as Norito");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sorafs/pin/register")
                .header("content-type", "application/x-norito")
                .body(Body::from(norito_payload))
                .expect("build request"),
        )
        .await
        .expect("router responds");
    let status = response.status();
    let body = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    assert!(
        status == StatusCode::OK,
        "pin register norito payload failed: {status} body={}",
        String::from_utf8_lossy(&body)
    );
}

#[tokio::test]
async fn sorafs_pin_register_rejects_malformed_successor_hex() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let harness = build_torii_harness(&cfg);
    let app = harness.app.clone();

    let fixture = manifest_request_fixture(6, |req| {
        req.successor_of_hex = Some("zz-invalid-hex".into());
    });

    let payload = norito::json::to_vec(&fixture.request).expect("serialize request");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sorafs/pin/register")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .expect("build request"),
        )
        .await
        .expect("router responds");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let message = String::from_utf8_lossy(&body);
    assert!(
        message.contains("successor_of_hex"),
        "error message should mention successor digest parsing: {message}"
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sorafs_pin_manifest_returns_ok_with_fresh_alias_cache_headers() {
    if !ingest_tests_enabled() {
        eprintln!("skipping alias cache headers test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    let admission_dir = tempdir().expect("admission dir");
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: admission_dir.path().to_path_buf(),
    });
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    let storage_dir = tempdir().expect("storage dir");
    cfg.torii.sorafs_storage.data_dir = storage_dir.path().join("storage");

    let harness = build_torii_harness(&cfg);
    let mut next_height = 1;
    let setup = create_manifest_setup(&harness, &mut next_height);

    let positive_ttl = harness.alias_policy.positive_ttl_secs();
    let refresh_window = harness.alias_policy.refresh_window_secs();
    let freshness_margin = positive_ttl.saturating_sub(refresh_window);
    let fresh_age = freshness_margin
        .checked_div(2)
        .filter(|age| *age > 0)
        .unwrap_or_else(|| positive_ttl.saturating_sub(1).max(1));
    let now = unix_now_secs();
    let generated_at = now.saturating_sub(fresh_age);
    let expires_at = now.saturating_add(positive_ttl.saturating_mul(2));

    bind_alias_with_proof(
        &harness,
        &setup,
        "docs-fresh",
        "sora",
        generated_at,
        expires_at,
        setup.approved_epoch,
        setup.retention_epoch,
        &mut next_height,
    );

    let request = Request::builder()
        .method("GET")
        .uri(format!("/v1/sorafs/pin/{}", setup.manifest_digest_hex))
        .header("x-sorafs-manifest-envelope", "dummy-envelope")
        .body(Body::empty())
        .expect("build request");
    let response = harness
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("manifest response");
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let body_text = String::from_utf8_lossy(&body_bytes);

    assert_eq!(status, StatusCode::OK, "body={body_text}");

    let cache_control = headers
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok());
    let expected_cache = format!("max-age={positive_ttl}, stale-while-revalidate={refresh_window}");
    assert_eq!(cache_control, Some(expected_cache.as_str()));

    let proof_status = headers
        .get("sora-proof-status")
        .and_then(|value| value.to_str().ok());
    assert_eq!(proof_status, Some("fresh"));

    assert!(
        headers.get(WARNING).is_none(),
        "fresh responses must not emit warning headers"
    );

    let age = headers
        .get(AGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("age header present");
    assert!(
        age >= fresh_age.saturating_sub(2),
        "age should reflect recent proof ({age} vs {fresh_age})"
    );
    assert!(
        age <= fresh_age.saturating_add(5),
        "age should closely match configured fresh proof age ({age} vs {fresh_age})"
    );

    let alias_label = headers
        .get("sora-name")
        .and_then(|value| value.to_str().ok());
    assert_eq!(alias_label, Some("docs-fresh/sora"));
    assert!(
        headers
            .get("sora-proof")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| !value.is_empty()),
        "Sora-Proof header must be present for fresh responses"
    );

    let body: json::Value = json::from_slice(&body_bytes).expect("parse response body");
    let aliases = body
        .get("aliases")
        .and_then(json::Value::as_array)
        .expect("aliases array present");
    let alias_entry = aliases.first().expect("alias entry present");
    assert_eq!(
        alias_entry.get("cache_state").and_then(json::Value::as_str),
        Some("fresh")
    );
    assert_eq!(
        alias_entry
            .get("policy_positive_ttl_secs")
            .and_then(json::Value::as_u64),
        Some(positive_ttl)
    );
    let cache_eval = alias_entry
        .get("cache_evaluation")
        .expect("cache evaluation present");
    let ttl_unix = cache_eval
        .get("ttl_expires_at_unix")
        .and_then(json::Value::as_u64)
        .expect("ttl_expires_at_unix present");
    assert_eq!(ttl_unix, expires_at);
    let ttl_iso = cache_eval
        .get("ttl_expires_at")
        .and_then(json::Value::as_str)
        .expect("ttl_expires_at string present");
    let expected_ttl_iso = format_rfc3339(UNIX_EPOCH + Duration::from_secs(expires_at)).to_string();
    assert_eq!(ttl_iso, expected_ttl_iso);
    assert!(
        cache_eval
            .get("serve_until")
            .is_some_and(json::Value::is_null),
        "fresh alias without successor should not advertise serve_until"
    );
    let manifest_entry = body.get("manifest").expect("manifest object present");
    let lineage = manifest_entry
        .get("lineage")
        .expect("manifest lineage present");
    assert_eq!(
        lineage.get("head_hex").and_then(json::Value::as_str),
        Some(setup.manifest_digest_hex.as_str())
    );
    assert_eq!(
        lineage.get("is_head").and_then(json::Value::as_bool),
        Some(true)
    );
    assert!(
        lineage
            .get("superseded_by")
            .is_some_and(json::Value::is_null)
    );
    assert!(
        manifest_entry
            .get("governance_refs")
            .and_then(json::Value::as_array)
            .is_some_and(Vec::is_empty),
        "fresh manifest should not expose governance references"
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sorafs_pin_manifest_reports_refresh_window_alias_headers() {
    if !ingest_tests_enabled() {
        eprintln!("skipping alias refresh window test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    let admission_dir = tempdir().expect("admission dir");
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: admission_dir.path().to_path_buf(),
    });
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    let storage_dir = tempdir().expect("storage dir");
    cfg.torii.sorafs_storage.data_dir = storage_dir.path().join("storage");

    let harness = build_torii_harness(&cfg);
    let mut next_height = 1;
    let setup = create_manifest_setup(&harness, &mut next_height);

    let positive_ttl = harness.alias_policy.positive_ttl_secs();
    let refresh_window = harness.alias_policy.refresh_window_secs();
    let refresh_age = {
        let base = positive_ttl.saturating_sub(refresh_window);
        let adjustment = refresh_window
            .checked_div(2)
            .filter(|age| *age > 0)
            .unwrap_or(1);
        let candidate = base.saturating_add(adjustment);
        if candidate >= positive_ttl {
            positive_ttl.saturating_sub(1).max(1)
        } else {
            candidate.max(1)
        }
    };
    let now = unix_now_secs();
    let generated_at = now.saturating_sub(refresh_age);
    let expires_at = now.saturating_add(positive_ttl.saturating_mul(2));

    bind_alias_with_proof(
        &harness,
        &setup,
        "docs-refresh",
        "sora",
        generated_at,
        expires_at,
        setup.approved_epoch,
        setup.retention_epoch,
        &mut next_height,
    );

    let request = Request::builder()
        .method("GET")
        .uri(format!("/v1/sorafs/pin/{}", setup.manifest_digest_hex))
        .header("x-sorafs-manifest-envelope", "dummy-envelope")
        .body(Body::empty())
        .expect("build request");
    let response = harness
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("manifest response");
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let body_text = String::from_utf8_lossy(&body_bytes);

    assert_eq!(status, StatusCode::OK, "body={body_text}");

    let cache_control = headers
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok());
    let expected_cache = format!("max-age={positive_ttl}, stale-while-revalidate={refresh_window}");
    assert_eq!(cache_control, Some(expected_cache.as_str()));

    let proof_status = headers
        .get("sora-proof-status")
        .and_then(|value| value.to_str().ok());
    assert_eq!(proof_status, Some("refresh"));

    let warning = headers
        .get(WARNING)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        warning.contains("alias proof refresh in-flight"),
        "refresh-window responses must emit refresh warning (got {warning})"
    );

    let age = headers
        .get(AGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("age header present");
    let refresh_threshold = positive_ttl.saturating_sub(refresh_window);
    assert!(
        age >= refresh_threshold,
        "age should enter refresh window (threshold {refresh_threshold}, age {age})"
    );
    assert!(
        age <= refresh_age.saturating_add(5),
        "age should remain below positive TTL ({age} vs {refresh_age})"
    );

    let alias_label = headers
        .get("sora-name")
        .and_then(|value| value.to_str().ok());
    assert_eq!(alias_label, Some("docs-refresh/sora"));

    let body: json::Value = json::from_slice(&body_bytes).expect("parse response body");
    let aliases = body
        .get("aliases")
        .and_then(json::Value::as_array)
        .expect("aliases array present");
    let alias_entry = aliases.first().expect("alias entry present");
    assert_eq!(
        alias_entry.get("cache_state").and_then(json::Value::as_str),
        Some("refresh")
    );
    assert_eq!(
        alias_entry
            .get("policy_refresh_window_secs")
            .and_then(json::Value::as_u64),
        Some(refresh_window)
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sorafs_pin_manifest_returns_service_unavailable_for_stale_alias() {
    if !ingest_tests_enabled() {
        eprintln!("skipping stale alias test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    let admission_dir = tempdir().expect("admission dir");
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: admission_dir.path().to_path_buf(),
    });
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    let storage_dir = tempdir().expect("storage dir");
    cfg.torii.sorafs_storage.data_dir = storage_dir.path().join("storage");

    let harness = build_torii_harness(&cfg);
    let mut next_height = 1;
    let setup = create_manifest_setup(&harness, &mut next_height);

    let positive_ttl = harness.alias_policy.positive_ttl_secs();
    let refresh_window = harness.alias_policy.refresh_window_secs();
    let now = unix_now_secs();
    let generated_at = now.saturating_sub(positive_ttl + 45);
    let expires_at = now + 300;

    bind_alias_with_proof(
        &harness,
        &setup,
        "docs-stale",
        "sora",
        generated_at,
        expires_at,
        setup.approved_epoch,
        setup.retention_epoch,
        &mut next_height,
    );

    let list_request = Request::builder()
        .method("GET")
        .uri("/v1/sorafs/pin")
        .body(Body::empty())
        .expect("build list request");
    let list_response = harness
        .app
        .clone()
        .oneshot(list_request)
        .await
        .expect("list response");
    println!("LIST STATUS {:?}", list_response.status());
    let list_body = BodyExt::collect(list_response.into_body())
        .await
        .expect("collect list body")
        .to_bytes();
    println!("LIST BODY {}", String::from_utf8_lossy(&list_body));

    let request = Request::builder()
        .method("GET")
        .uri(format!("/v1/sorafs/pin/{}", setup.manifest_digest_hex))
        .header("x-sorafs-manifest-envelope", "dummy-envelope")
        .body(Body::empty())
        .expect("build request");
    let response = harness
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("manifest response");
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let body_text = String::from_utf8_lossy(&body_bytes);

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "body={body_text}");

    let proof_status = headers
        .get("sora-proof-status")
        .and_then(|value| value.to_str().ok());
    assert_eq!(proof_status, Some("expired"));

    let age = headers
        .get(AGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("age header present");
    assert!(
        age >= positive_ttl,
        "age header should reflect stale proof (age={age}, ttl={positive_ttl})"
    );

    let retry_after_expected = refresh_window.to_string();
    let retry_after = headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok());
    assert_eq!(retry_after, Some(retry_after_expected.as_str()));

    let warning = headers
        .get(WARNING)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        warning.contains("alias proof stale"),
        "warning header should mention stale proof: {warning}"
    );

    let cache_control = headers
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok());
    assert_eq!(cache_control, Some("no-store"));

    let body: json::Value = json::from_slice(&body_bytes).expect("parse response body");
    assert_eq!(
        body.get("cache_state").and_then(json::Value::as_str),
        Some("expired"),
        "manifest response should report expired alias cache_state"
    );
    assert_eq!(
        body.get("error").and_then(json::Value::as_str),
        Some("alias proof stale; refresh required")
    );
    assert!(
        body.get("proof_expires_in_seconds")
            .and_then(json::Value::as_u64)
            .is_some(),
        "stale response should report remaining expiry window"
    );
}

#[tokio::test]
async fn sorafs_pin_manifest_returns_precondition_failed_for_expired_alias() {
    if !ingest_tests_enabled() {
        eprintln!("skipping expired alias test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    let admission_dir = tempdir().expect("admission dir");
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: admission_dir.path().to_path_buf(),
    });
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    let storage_dir = tempdir().expect("storage dir");
    cfg.torii.sorafs_storage.data_dir = storage_dir.path().join("storage");

    let harness = build_torii_harness(&cfg);
    let mut next_height = 1;
    let setup = create_manifest_setup(&harness, &mut next_height);

    let hard_expiry = harness.alias_policy.hard_expiry_secs();
    let now = unix_now_secs();
    let generated_at = now.saturating_sub(hard_expiry + 90);
    let expires_at = now.saturating_sub(30);

    bind_alias_with_proof(
        &harness,
        &setup,
        "docs-expired",
        "sora",
        generated_at,
        expires_at,
        setup.approved_epoch,
        setup.retention_epoch,
        &mut next_height,
    );

    let request = Request::builder()
        .method("GET")
        .uri(format!("/v1/sorafs/pin/{}", setup.manifest_digest_hex))
        .header("x-sorafs-manifest-envelope", "dummy-envelope")
        .body(Body::empty())
        .expect("build request");
    let response = harness
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("manifest response");
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let body_text = String::from_utf8_lossy(&body_bytes);

    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "body={body_text}");
    assert!(
        headers.get(RETRY_AFTER).is_none(),
        "hard-expired responses must not include Retry-After"
    );

    let proof_status = headers
        .get("sora-proof-status")
        .and_then(|value| value.to_str().ok());
    assert_eq!(proof_status, Some("hard-expired"));

    let age = headers
        .get(AGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("age header present");
    assert!(
        age >= hard_expiry,
        "age header should reflect hard-expired proof (age={age}, hard_expiry={hard_expiry})"
    );

    let warning = headers
        .get(WARNING)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        warning.contains("alias proof expired"),
        "warning header should mention expired proof: {warning}"
    );

    let cache_control = headers
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok());
    assert_eq!(cache_control, Some("no-store"));

    let body: json::Value = json::from_slice(&body_bytes).expect("parse response body");
    assert_eq!(
        body.get("cache_state").and_then(json::Value::as_str),
        Some("hard-expired")
    );
    assert_eq!(
        body.get("error").and_then(json::Value::as_str),
        Some("alias proof expired; refresh required")
    );
    assert!(
        body.get("proof_expires_in_seconds").is_none(),
        "expired response must not contain remaining expiry interval"
    );
}

#[tokio::test]
async fn sorafs_pin_manifest_returns_gone_for_revoked_alias() {
    if !ingest_tests_enabled() {
        eprintln!("skipping revoked alias test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    let admission_dir = tempdir().expect("admission dir");
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: admission_dir.path().to_path_buf(),
    });
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    let storage_dir = tempdir().expect("storage dir");
    cfg.torii.sorafs_storage.data_dir = storage_dir.path().join("storage");
    cfg.torii.sorafs_alias_cache.successor_grace = Duration::from_secs(0);
    cfg.torii.sorafs_alias_cache.governance_grace = Duration::from_secs(0);

    let harness = build_torii_harness(&cfg);
    let mut next_height = 1;
    let setup = create_manifest_setup(&harness, &mut next_height);

    let now = unix_now_secs();
    bind_alias_with_proof(
        &harness,
        &setup,
        "docs-revoked",
        "gamma",
        now.saturating_sub(60),
        now + 900,
        setup.approved_epoch,
        setup.retention_epoch,
        &mut next_height,
    );
    next_height += 1;

    let alias_label = "docs-revoked/gamma";
    attach_governance_revocation(
        &harness,
        &setup,
        alias_label,
        now.saturating_sub(5),
        &mut next_height,
    );

    let request = Request::builder()
        .method("GET")
        .uri(format!("/v1/sorafs/pin/{}", setup.manifest_digest_hex))
        .header("x-sorafs-manifest-envelope", "dummy-envelope")
        .body(Body::empty())
        .expect("build request");
    let response = harness
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("manifest response");
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let body_text = String::from_utf8_lossy(&body_bytes);

    assert_eq!(status, StatusCode::GONE, "body={body_text}");

    let revocation_ttl = harness.alias_policy.revocation_ttl.as_secs();
    let expected_cache = format!("max-age={revocation_ttl}");
    assert_eq!(
        headers
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(expected_cache.as_str())
    );
    let expected_retry = revocation_ttl.to_string();
    assert_eq!(
        headers
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
        Some(expected_retry.as_str())
    );
    let proof_status = headers
        .get("sora-proof-status")
        .and_then(|value| value.to_str().ok());
    assert_eq!(proof_status, Some("governance-refused"));

    let body: json::Value = json::from_slice(&body_bytes).expect("parse response body");
    assert_eq!(
        body.get("cache_state").and_then(json::Value::as_str),
        Some("governance-refused")
    );
    assert_eq!(
        body.get("error").and_then(json::Value::as_str),
        Some("alias proof revoked by governance")
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sorafs_alias_listing_reports_successor_refusal() {
    if !ingest_tests_enabled() {
        eprintln!("skipping successor refusal test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    let admission_dir = tempdir().expect("admission dir");
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: admission_dir.path().to_path_buf(),
    });
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    let storage_dir = tempdir().expect("storage dir");
    cfg.torii.sorafs_storage.data_dir = storage_dir.path().join("storage");
    cfg.torii.sorafs_alias_cache.successor_grace = Duration::from_secs(0);
    cfg.torii.sorafs_alias_cache.governance_grace = Duration::from_secs(0);

    let harness = build_torii_harness(&cfg);
    let mut next_height = 1;

    let base = create_manifest_setup(&harness, &mut next_height);
    let successor_timestamp = unix_now_secs().saturating_sub(30);
    let successor =
        create_successor_manifest(&harness, &base, &mut next_height, 0xBC, successor_timestamp);

    let now = unix_now_secs();
    bind_alias_with_proof(
        &harness,
        &base,
        "docs-successor",
        "alpha",
        now.saturating_sub(60),
        now + 900,
        base.approved_epoch,
        base.retention_epoch,
        &mut next_height,
    );

    let list_request = Request::builder()
        .method("GET")
        .uri("/v1/sorafs/pin")
        .body(Body::empty())
        .expect("build alias list request");
    let list_response = harness
        .app
        .clone()
        .oneshot(list_request)
        .await
        .expect("alias list response");
    let status = list_response.status();
    let list_body = BodyExt::collect(list_response.into_body())
        .await
        .expect("collect alias list body")
        .to_bytes();
    assert_eq!(status, StatusCode::OK);
    let list_json: json::Value = json::from_slice(&list_body).expect("parse alias list response");
    let manifests = manifest_entries(&list_json);
    let alias_inventory: Vec<_> = harness
        .state
        .view()
        .world()
        .manifest_aliases()
        .iter()
        .map(|(id, record)| {
            (
                id.as_label(),
                record.manifest.as_bytes().encode_hex::<String>(),
            )
        })
        .collect();
    let alias_entry = find_alias_entry(manifests, "docs-successor", "alpha").unwrap_or_else(|| {
        panic!("docs-successor/alpha alias present; aliases={alias_inventory:?}")
    });

    assert!(
        alias_entry.get("cache_decision").is_none(),
        "alias listing no longer surfaces cache_decision",
    );

    let lineage = alias_entry
        .get("lineage")
        .and_then(json::Value::as_object)
        .expect("lineage metadata present");
    let superseded = lineage
        .get("superseded_by")
        .and_then(json::Value::as_object)
        .expect("superseded_by metadata present");
    assert_eq!(
        superseded.get("digest_hex").and_then(json::Value::as_str),
        Some(successor.manifest_digest_hex.as_str()),
        "superseded record should point at successor manifest"
    );
    assert_eq!(
        superseded
            .get("approved_epoch")
            .and_then(json::Value::as_u64),
        Some(successor.approved_epoch),
        "superseded record should carry successor epoch"
    );
    assert!(
        lineage
            .get("immediate_successor")
            .and_then(json::Value::as_object)
            .is_some(),
        "immediate_successor metadata should be populated"
    );
    let cache_eval = alias_entry
        .get("cache_evaluation")
        .and_then(json::Value::as_object)
        .unwrap_or_else(|| panic!("alias cache evaluation present: {alias_entry:?}"));
    let successor_eval = cache_eval
        .get("successor")
        .and_then(json::Value::as_object)
        .expect("successor evaluation present");
    assert_eq!(
        successor_eval
            .get("approved")
            .and_then(json::Value::as_bool),
        Some(true)
    );
    let expected_successor_iso =
        format_rfc3339(UNIX_EPOCH + Duration::from_secs(successor_timestamp)).to_string();
    assert_eq!(
        successor_eval
            .get("approved_at")
            .and_then(json::Value::as_str),
        Some(expected_successor_iso.as_str())
    );
}

async fn fetch_alias_entry(harness: &ToriiHarness, alias_label: &str) -> json::Value {
    let list_request = Request::builder()
        .method("GET")
        .uri("/v1/sorafs/pin")
        .body(Body::empty())
        .expect("build alias list request");
    let list_response = harness
        .app
        .clone()
        .oneshot(list_request)
        .await
        .expect("alias list response");
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = BodyExt::collect(list_response.into_body())
        .await
        .expect("collect alias list body")
        .to_bytes();
    let list_json: json::Value = json::from_slice(&list_body).expect("parse alias list response");
    let manifests = manifest_entries(&list_json);
    let (namespace, name) = alias_label
        .split_once('/')
        .expect("alias label contains namespace and name");
    find_alias_entry(manifests, namespace, name)
        .expect("alias entry present")
        .clone()
}

#[tokio::test]
async fn sorafs_alias_listing_reports_governance_revocation() {
    if !ingest_tests_enabled() {
        eprintln!("skipping governance revocation test (SORAFS_TORII_SKIP_INGEST_TESTS=1)");
        return;
    }
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_discovery.discovery_enabled = true;
    let admission_dir = tempdir().expect("admission dir");
    cfg.torii.sorafs_discovery.admission = Some(SorafsAdmission {
        envelopes_dir: admission_dir.path().to_path_buf(),
    });
    cfg.torii.sorafs_storage.enabled = true;
    cfg.torii.sorafs_storage.max_parallel_fetches = 1;
    cfg.torii.sorafs_storage.max_pins = 8;
    cfg.torii.sorafs_storage.max_capacity_bytes = Bytes(1_048_576);
    let storage_dir = tempdir().expect("storage dir");
    cfg.torii.sorafs_storage.data_dir = storage_dir.path().join("storage");
    cfg.torii.sorafs_alias_cache.successor_grace = Duration::from_secs(0);
    cfg.torii.sorafs_alias_cache.governance_grace = Duration::from_secs(0);

    let harness = build_torii_harness(&cfg);
    let mut next_height = 1;

    let manifest = create_manifest_setup_with_seed(&harness, &mut next_height, 0xC1, None, None);
    let now = unix_now_secs();
    bind_alias_with_proof(
        &harness,
        &manifest,
        "docs-governance",
        "beta",
        now.saturating_sub(60),
        now + 900,
        manifest.approved_epoch,
        manifest.retention_epoch,
        &mut next_height,
    );
    next_height += 1;

    let alias_label = "docs-governance/beta";
    let effective_at = now.saturating_sub(5);
    attach_governance_revocation(
        &harness,
        &manifest,
        alias_label,
        effective_at,
        &mut next_height,
    );

    let alias_entry = fetch_alias_entry(&harness, alias_label).await;

    assert!(
        alias_entry.get("cache_decision").is_none(),
        "alias list no longer reports cache_decision",
    );

    let metadata = alias_entry
        .get("metadata")
        .and_then(json::Value::as_object)
        .expect("metadata present");
    assert!(
        metadata.contains_key(STATUS_TIMESTAMP_KEY),
        "revocation metadata should include status timestamp: {metadata:?}"
    );
    let governance_eval = alias_entry
        .get("cache_evaluation")
        .and_then(|value| value.get("governance"))
        .unwrap_or_else(|| panic!("governance evaluation present: {alias_entry:?}"));
    assert_eq!(
        governance_eval
            .get("revoked")
            .and_then(json::Value::as_bool),
        Some(true),
        "governance_eval={governance_eval:?}"
    );
    let expected_effective =
        format_rfc3339(UNIX_EPOCH + Duration::from_secs(effective_at)).to_string();
    assert_eq!(
        governance_eval
            .get("effective_at")
            .and_then(json::Value::as_str),
        Some(expected_effective.as_str())
    );
    assert_eq!(
        governance_eval
            .get("effective_at_unix")
            .and_then(json::Value::as_u64),
        Some(effective_at)
    );
}

#[test]
fn disk_fixtures_detect_advert_key_mismatch() {
    let fixtures = [("advert_v1.to", "envelope_v1.to")];

    for (advert_path, envelope_path) in fixtures {
        let fixture = fixture_from_disk(advert_path, envelope_path);
        let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));
        let mut cache = ProviderAdvertCache::new(
            [
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            registry,
        );

        let now = fixture
            .advert
            .issued_at
            .saturating_add(30)
            .min(fixture.advert.expires_at.saturating_sub(1))
            .max(fixture.advert.issued_at);
        let err = cache
            .ingest(fixture.advert.clone(), now)
            .expect_err("fixture ingestion must fail due to advert key mismatch");
        match err {
            AdvertError::AdmissionFailed { error, .. } => {
                assert!(
                    matches!(error, AdmissionCheckError::AdvertKeyMismatch),
                    "expected advert key mismatch, got {error:?}"
                );
            }
            other => panic!("expected admission failure, got {other:?}"),
        }
    }
}
