//! CLI fetch, integrity, streaming-output and provider-metadata regression tests.

use super::*;
use assert_cmd::Command as AssertCommand;
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signer, SigningKey};
use norito::to_bytes;
use sorafs_car::{
    CarWriter, compute_chunk_plan_digest_sha3, compute_por_root,
    fetch_plan::chunk_fetch_plan_to_string,
    multi_fetch::{ChunkReceipt, FetchOutcome, FetchProvider, ProviderId, ProviderReport},
};
use sorafs_manifest::{
    AdvertEndpoint, AdvertSignature, CapabilityTlv, DagCodecId, EndpointKind, EndpointMetadata,
    EndpointMetadataKey, GovernanceProofs, ManifestBuilder, PROVIDER_ADVERT_VERSION_V1,
    PathDiversityPolicy, PinPolicy, ProviderAdvertBodyV1, ProviderCapabilityRangeV1, QosHints,
    RendezvousTopic, SignatureAlgorithm, StakePointer, StorageClass, StreamBudgetV1,
    TransportHintV1, TransportProtocol, deal::XorQuantity, hybrid_envelope::HybridKemBundleV1,
    provider_advert::ProviderCapabilitySoranetPqV1,
};
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::{NamedTempFile, TempDir, tempdir};
#[test]
fn formats_all_policy_denied_providers() {
    let error = MultiSourceError::NoPolicyEligibleProviders {
        chunk_index: 7,
        providers: vec![ProviderId::new("alpha"), ProviderId::new("beta")],
    };

    let rendered = format_multi_source_error(error).expect("operator-visible error");

    assert_eq!(
        rendered,
        "score policy rejected every available provider for chunk 7: alpha, beta"
    );
}
fn canonical_tempdir() -> (TempDir, PathBuf) {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().canonicalize().expect("canonical tempdir");
    (temp, path)
}
fn write_canonical_plan(path: &Path, plan: &CarBuildPlan) {
    fs::write(
        path,
        chunk_fetch_plan_to_string(plan).expect("render canonical plan"),
    )
    .expect("write canonical plan");
}
fn xor_micro(value: u128) -> XorQuantity {
    XorQuantity::try_from_micro(value).expect("test micro-XOR amount is representable")
}
fn plan_chunks(payload: &[u8], plan: &CarBuildPlan) -> Vec<Vec<u8>> {
    plan.chunks
        .iter()
        .map(|chunk| {
            let start = chunk.offset as usize;
            let end = start + chunk.length as usize;
            payload[start..end].to_vec()
        })
        .collect()
}
#[test]
fn write_binary_creates_parent_and_writes_all_bytes() {
    let (_temp, temp_path) = canonical_tempdir();
    let output_path = temp_path.join("nested").join("payload.bin");
    write_binary(&output_path, b"sorafs-fetch-output").expect("write binary output");
    assert_eq!(
        fs::read(&output_path).expect("read output"),
        b"sorafs-fetch-output"
    );
}
#[cfg(unix)]
#[test]
fn write_text_rejects_symlink_output() {
    let (_temp, temp_path) = canonical_tempdir();
    let target_path = temp_path.join("target.json");
    fs::write(&target_path, b"unchanged\n").expect("write target");
    let output_path = temp_path.join("report.json");
    std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");
    let err = write_text(&output_path, "changed\n").expect_err("reject symlink output");
    assert!(
        err.contains("must not be a symlink"),
        "unexpected error: {err}"
    );
    assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
}
#[cfg(unix)]
#[test]
fn streaming_writer_rejects_symlink_parent() {
    let (_temp, temp_path) = canonical_tempdir();
    let real_dir = temp_path.join("real");
    fs::create_dir(&real_dir).expect("create real dir");
    let linked_dir = temp_path.join("linked");
    std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
    let output_path = linked_dir.join("assembled.bin");
    let err = match StreamingWriter::create(&output_path) {
        Ok(_) => panic!("symlink parent should be rejected"),
        Err(err) => err,
    };
    assert!(
        err.contains("parent") && err.contains("must not be a symlink"),
        "unexpected error: {err}"
    );
    assert!(
        !real_dir.join("assembled.bin").exists(),
        "symlink parent should not receive output"
    );
}
#[cfg(unix)]
#[test]
fn write_car_archive_rejects_symlink_output() {
    let (_temp, temp_path) = canonical_tempdir();
    let payload = b"sorafs-fetch-car-output".to_vec();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let chunks = plan_chunks(&payload, &plan);
    let target_path = temp_path.join("target.car");
    fs::write(&target_path, b"unchanged").expect("write target");
    let car_path = temp_path.join("payload.car");
    std::os::unix::fs::symlink(&target_path, &car_path).expect("create symlink");
    let err = match write_car_archive(&plan, &chunks, &car_path) {
        Ok(_) => panic!("symlink output should be rejected"),
        Err(err) => err,
    };
    assert!(
        err.contains("must not be a symlink"),
        "unexpected error: {err}"
    );
    assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged");
}
fn cargo_bin_path(bin_name: &str) -> PathBuf {
    let env_var = format!("CARGO_BIN_EXE_{bin_name}");
    if let Some(path) = env::var_os(env_var) {
        return PathBuf::from(path);
    }
    let mut path = env::current_exe().expect("current exe path should be available");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(format!("{bin_name}{}", env::consts::EXE_SUFFIX))
}
fn sorafs_fetch_cmd() -> AssertCommand {
    AssertCommand::new(cargo_bin_path("sorafs_fetch"))
}
fn range_capability_payload() -> Vec<u8> {
    let profile = ChunkProfile::DEFAULT;
    ProviderCapabilityRangeV1 {
        max_chunk_span: profile.max_size as u32,
        min_granularity: profile.min_size as u32,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    }
    .to_bytes()
    .expect("encode range capability")
}
fn sample_stream_budget() -> StreamBudgetV1 {
    StreamBudgetV1 {
        max_in_flight: 4,
        max_bytes_per_sec: 5_000_000,
        burst_bytes: Some(2_500_000),
    }
}
fn signed_provider_advert(
    body: ProviderAdvertBodyV1,
    signing_key: &SigningKey,
    issued_at: u64,
    expires_at: u64,
    allow_unknown_capabilities: bool,
) -> ProviderAdvertV1 {
    let mut advert = ProviderAdvertV1 {
        version: PROVIDER_ADVERT_VERSION_V1,
        issued_at,
        expires_at,
        body,
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: vec![0; 64],
        },
        signature_strict: true,
        allow_unknown_capabilities,
    };
    let payload = advert
        .signature_payload_bytes()
        .expect("serialize advert signature envelope");
    advert.signature.signature = signing_key.sign(&payload).to_bytes().to_vec();
    advert
}
fn default_profile_aliases() -> Vec<String> {
    chunker_registry::default_descriptor()
        .aliases
        .iter()
        .map(|alias| alias.to_string())
        .collect()
}
fn sample_endpoint() -> AdvertEndpoint {
    AdvertEndpoint {
        kind: EndpointKind::Torii,
        host_pattern: "torii.example.org".into(),
        metadata: vec![EndpointMetadata {
            key: EndpointMetadataKey::Region,
            value: b"global".to_vec(),
        }],
    }
}
fn sample_rendezvous_topics(label: &str) -> Vec<RendezvousTopic> {
    vec![RendezvousTopic {
        topic: format!("sorafs.{label}.primary"),
        region: "global".into(),
    }]
}
fn sample_transport_hints() -> Vec<TransportHintV1> {
    vec![TransportHintV1 {
        protocol: TransportProtocol::ToriiHttpRange,
        priority: 0,
    }]
}
fn sample_gateway_manifest_envelope() -> HybridPayloadEnvelopeV1 {
    HybridPayloadEnvelopeV1 {
        version: HYBRID_PAYLOAD_ENVELOPE_VERSION_V1,
        suite: HybridSuite::X25519MlKem768ChaCha20Poly1305.to_string(),
        kem: HybridKemBundleV1 {
            ephemeral_public: vec![7, 8, 9],
            kyber_ciphertext: vec![10, 11, 12],
        },
        nonce: [1_u8; 12],
        ciphertext: vec![42; 16],
    }
}
fn encode_gateway_manifest_envelope(envelope: &HybridPayloadEnvelopeV1) -> String {
    let bytes = to_bytes(envelope).expect("encode envelope");
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
#[test]
fn provider_advert_reader_accepts_boundary_and_rejects_one_over() {
    let (_directory, root) = canonical_tempdir();
    let path = root.join("provider-advert.to");
    fs::write(&path, vec![0xA5; PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1])
        .expect("write exact-boundary provider advert");
    assert_eq!(
        read_provider_advert_bytes(&path)
            .expect("read exact-boundary provider advert")
            .len(),
        PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1
    );
    fs::write(
        &path,
        vec![0xA5; PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1 + 1],
    )
    .expect("write one-over provider advert");
    assert!(read_provider_advert_bytes(&path).is_err());
}
#[test]
fn verify_provider_advert_signature_rejects_all_zero_signature_material() {
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0x11; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(vec![profile_handle]),
        stake: StakePointer {
            pool_id: [0x22; 32],
            stake_amount: xor_micro(1_000_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 500,
            max_concurrent_streams: 5,
        },
        capabilities: vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        endpoints: vec![sample_endpoint()],
        rendezvous_topics: sample_rendezvous_topics("zero-signature"),
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: None,
        transport_hints: None,
    };
    let signing_key = SigningKey::from_bytes(&[0xAB; 32]);
    let mut advert = signed_provider_advert(
        advert_body,
        &signing_key,
        1_700_000_000,
        1_700_003_600,
        false,
    );
    advert.signature.signature.fill(0);
    let err = verify_provider_advert_signature(&advert)
        .expect_err("all-zero signature material must be rejected");
    assert!(err.contains("all zero"), "unexpected error: {err}");
}
fn sample_gateway_manifest_envelope_b64() -> String {
    encode_gateway_manifest_envelope(&sample_gateway_manifest_envelope())
}
#[test]
fn runtime_provider_counts_reports_direct_and_gateway_lengths() {
    let mut registry = HashMap::new();
    let provider_file = NamedTempFile::new().expect("temp provider payload");
    registry.insert(
        "alpha".to_string(),
        ProviderRuntime::Local(
            ProviderSource::new("alpha", provider_file.path())
                .expect("provider source should be constructed"),
        ),
    );
    registry.insert("gw-beta".to_string(), ProviderRuntime::Gateway);
    let (provider_count, gateway_count) = runtime_provider_counts(&registry);
    assert_eq!(provider_count, 1);
    assert_eq!(gateway_count, 1);
}
#[test]
fn runtime_provider_counts_handles_gateway_only_runs() {
    let mut registry = HashMap::new();
    registry.insert("gw-alpha".to_string(), ProviderRuntime::Gateway);
    let (provider_count, gateway_count) = runtime_provider_counts(&registry);
    assert_eq!(provider_count, 0);
    assert_eq!(gateway_count, 1);
}
#[test]
fn provider_mix_labels_gateway_only_runs() {
    assert_eq!(provider_mix_label(0, 2), "gateway-only");
}
#[test]
fn provider_mix_labels_mixed_runs() {
    assert_eq!(provider_mix_label(2, 2), "mixed");
}
#[test]
fn report_records_manifest_identifiers_when_present() {
    let provider = Arc::new(FetchProvider::new("did:sora:test"));
    let outcome = FetchOutcome {
        chunks: vec![vec![1, 2, 3, 4]],
        chunk_receipts: vec![ChunkReceipt {
            chunk_index: 0,
            provider: ProviderId::new("did:sora:test"),
            attempts: 1,
            latency_ms: 9.5,
            bytes: 4,
        }],
        provider_reports: vec![ProviderReport {
            provider: Arc::clone(&provider),
            successes: 1,
            failures: 0,
            disabled: false,
        }],
    };
    let digest = [0_u8; 32];
    let transport_labels = transport_policy_labels(
        Some(TransportPolicy::SoranetPreferred),
        Some(TransportPolicy::DirectOnly),
    );
    let report = build_report(ReportContext {
        outcome: &outcome,
        payload_len: 4,
        digest: &digest,
        car_stats: None,
        car_verification: None,
        provider_count: 0,
        gateway_provider_count: 1,
        provider_mix: "gateway-only",
        manifest_id: Some("feedface"),
        manifest_cid: Some("c0ffee"),
        gateway_manifest_provided: true,
        telemetry_label: None,
        telemetry_region: None,
        transport_labels,
    });
    let map = report.as_object().expect("report object");
    assert_eq!(
        map.get("manifest_id").and_then(Value::as_str),
        Some("feedface")
    );
    assert_eq!(
        map.get("manifest_cid").and_then(Value::as_str),
        Some("c0ffee")
    );
    assert_eq!(
        map.get("gateway_manifest_provided")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        map.get("transport_policy").and_then(Value::as_str),
        Some("direct-only")
    );
    assert_eq!(
        map.get("transport_policy_override")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        map.get("transport_policy_override_label")
            .and_then(Value::as_str),
        Some("direct-only")
    );
}
#[test]
fn report_omits_manifest_identifiers_when_absent() {
    let provider = Arc::new(FetchProvider::new("did:sora:test"));
    let outcome = FetchOutcome {
        chunks: vec![vec![1, 2, 3, 4]],
        chunk_receipts: vec![ChunkReceipt {
            chunk_index: 0,
            provider: ProviderId::new("did:sora:test"),
            attempts: 1,
            latency_ms: 9.5,
            bytes: 4,
        }],
        provider_reports: vec![ProviderReport {
            provider: Arc::clone(&provider),
            successes: 1,
            failures: 0,
            disabled: false,
        }],
    };
    let digest = [0_u8; 32];
    let transport_labels = transport_policy_labels(Some(TransportPolicy::SoranetPreferred), None);
    let report = build_report(ReportContext {
        outcome: &outcome,
        payload_len: 4,
        digest: &digest,
        car_stats: None,
        car_verification: None,
        provider_count: 1,
        gateway_provider_count: 0,
        provider_mix: "direct-only",
        manifest_id: None,
        manifest_cid: None,
        gateway_manifest_provided: false,
        telemetry_label: None,
        telemetry_region: None,
        transport_labels,
    });
    let map = report.as_object().expect("report object");
    assert!(map.get("manifest_id").is_none());
    assert!(map.get("manifest_cid").is_none());
    assert_eq!(
        map.get("gateway_manifest_provided")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        map.get("transport_policy").and_then(Value::as_str),
        Some("soranet-first")
    );
    assert_eq!(
        map.get("transport_policy_override")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        map.get("transport_policy_override_label")
            .is_none_or(Value::is_null)
    );
}
#[test]
fn report_records_telemetry_label_when_present() {
    let provider = Arc::new(FetchProvider::new("did:sora:test"));
    let outcome = FetchOutcome {
        chunks: vec![vec![1, 2, 3, 4]],
        chunk_receipts: vec![ChunkReceipt {
            chunk_index: 0,
            provider: ProviderId::new("did:sora:test"),
            attempts: 1,
            latency_ms: 2.0,
            bytes: 4,
        }],
        provider_reports: vec![ProviderReport {
            provider: Arc::clone(&provider),
            successes: 1,
            failures: 0,
            disabled: false,
        }],
    };
    let digest = [0_u8; 32];
    let transport_labels = transport_policy_labels(Some(TransportPolicy::SoranetPreferred), None);
    let report = build_report(ReportContext {
        outcome: &outcome,
        payload_len: 4,
        digest: &digest,
        car_stats: None,
        car_verification: None,
        provider_count: 1,
        gateway_provider_count: 0,
        provider_mix: "direct-only",
        manifest_id: None,
        manifest_cid: None,
        gateway_manifest_provided: false,
        telemetry_label: Some("otel::ci"),
        telemetry_region: None,
        transport_labels,
    });
    let map = report.as_object().expect("report object");
    assert_eq!(
        map.get("telemetry_source").and_then(Value::as_str),
        Some("otel::ci")
    );
}
#[test]
fn report_records_telemetry_region_when_present() {
    let provider = Arc::new(FetchProvider::new("did:sora:test"));
    let outcome = FetchOutcome {
        chunks: vec![vec![1, 2, 3, 4]],
        chunk_receipts: vec![ChunkReceipt {
            chunk_index: 0,
            provider: ProviderId::new("did:sora:test"),
            attempts: 1,
            latency_ms: 2.0,
            bytes: 4,
        }],
        provider_reports: vec![ProviderReport {
            provider: Arc::clone(&provider),
            successes: 1,
            failures: 0,
            disabled: false,
        }],
    };
    let digest = [0_u8; 32];
    let transport_labels = transport_policy_labels(Some(TransportPolicy::SoranetPreferred), None);
    let report = build_report(ReportContext {
        outcome: &outcome,
        payload_len: 4,
        digest: &digest,
        car_stats: None,
        car_verification: None,
        provider_count: 1,
        gateway_provider_count: 0,
        provider_mix: "direct-only",
        manifest_id: None,
        manifest_cid: None,
        gateway_manifest_provided: false,
        telemetry_label: None,
        telemetry_region: Some("regulated-eu"),
        transport_labels,
    });
    let map = report.as_object().expect("report object");
    assert_eq!(
        map.get("telemetry_region").and_then(Value::as_str),
        Some("regulated-eu")
    );
}
#[test]
fn scoreboard_metadata_records_manifest_envelope_presence() {
    let provider_count = 0;
    let gateway_count = 1;
    let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
        scoreboard_mode: true,
        allow_implicit_metadata: false,
        provider_count,
        gateway_provider_count: gateway_count,
        provider_mix: provider_mix_label(provider_count, gateway_count),
        max_parallel: None,
        max_peers: None,
        retry_budget: None,
        failure_threshold: None,
        assume_now: Some(1_700_000_000),
        telemetry_label: Some("test-source"),
        telemetry_region: Some("iad-prod"),
        gateway_manifest_id: Some("feedface"),
        gateway_manifest_cid: Some("c0ffee"),
        gateway_manifest_envelope_present: true,
        transport_policy: None,
        transport_policy_override: None,
        anonymity_policy: None,
        anonymity_policy_override: None,
    });
    let map = metadata.as_object().expect("metadata object");
    assert_eq!(
        map.get("gateway_manifest_provided")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        map.get("telemetry_source").and_then(Value::as_str),
        Some("test-source")
    );
    assert_eq!(
        map.get("telemetry_region").and_then(Value::as_str),
        Some("iad-prod")
    );
    assert_eq!(
        map.get("gateway_manifest_id").and_then(Value::as_str),
        Some("feedface")
    );
    assert_eq!(
        map.get("gateway_manifest_cid").and_then(Value::as_str),
        Some("c0ffee")
    );
}
#[test]
fn scoreboard_metadata_marks_missing_manifest_envelope() {
    let provider_count = 0;
    let gateway_count = 0;
    let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
        scoreboard_mode: true,
        allow_implicit_metadata: false,
        provider_count,
        gateway_provider_count: gateway_count,
        provider_mix: provider_mix_label(provider_count, gateway_count),
        max_parallel: None,
        max_peers: None,
        retry_budget: None,
        failure_threshold: None,
        assume_now: None,
        telemetry_label: None,
        telemetry_region: None,
        gateway_manifest_id: None,
        gateway_manifest_cid: None,
        gateway_manifest_envelope_present: false,
        transport_policy: None,
        transport_policy_override: None,
        anonymity_policy: None,
        anonymity_policy_override: None,
    });
    let map = metadata.as_object().expect("metadata object");
    assert_eq!(
        map.get("gateway_manifest_provided")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(map.get("gateway_manifest_id").is_none_or(Value::is_null));
    assert!(map.get("gateway_manifest_cid").is_none_or(Value::is_null));
}
#[test]
fn scoreboard_metadata_records_transport_policy_labels() {
    let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
        scoreboard_mode: true,
        allow_implicit_metadata: false,
        provider_count: 1,
        gateway_provider_count: 1,
        provider_mix: provider_mix_label(1, 1),
        max_parallel: None,
        max_peers: None,
        retry_budget: None,
        failure_threshold: None,
        assume_now: None,
        telemetry_label: None,
        telemetry_region: None,
        gateway_manifest_id: None,
        gateway_manifest_cid: None,
        gateway_manifest_envelope_present: false,
        transport_policy: Some(TransportPolicy::SoranetStrict),
        transport_policy_override: None,
        anonymity_policy: None,
        anonymity_policy_override: None,
    });
    let map = metadata.as_object().expect("metadata object");
    assert_eq!(
        map.get("transport_policy").and_then(Value::as_str),
        Some("soranet-strict")
    );
    assert_eq!(
        map.get("transport_policy_override")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        map.get("transport_policy_override_label")
            .is_none_or(Value::is_null)
    );
    let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
        scoreboard_mode: true,
        allow_implicit_metadata: false,
        provider_count: 1,
        gateway_provider_count: 1,
        provider_mix: provider_mix_label(1, 1),
        max_parallel: None,
        max_peers: None,
        retry_budget: None,
        failure_threshold: None,
        assume_now: None,
        telemetry_label: None,
        telemetry_region: None,
        gateway_manifest_id: None,
        gateway_manifest_cid: None,
        gateway_manifest_envelope_present: false,
        transport_policy: Some(TransportPolicy::SoranetPreferred),
        transport_policy_override: Some(TransportPolicy::DirectOnly),
        anonymity_policy: None,
        anonymity_policy_override: None,
    });
    let map = metadata.as_object().expect("metadata object");
    assert_eq!(
        map.get("transport_policy").and_then(Value::as_str),
        Some("direct-only")
    );
    assert_eq!(
        map.get("transport_policy_override")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        map.get("transport_policy_override_label")
            .and_then(Value::as_str),
        Some("direct-only")
    );
}
#[test]
fn scoreboard_metadata_records_anonymity_policy_labels() {
    let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
        scoreboard_mode: true,
        allow_implicit_metadata: false,
        provider_count: 1,
        gateway_provider_count: 1,
        provider_mix: provider_mix_label(1, 1),
        max_parallel: None,
        max_peers: None,
        retry_budget: None,
        failure_threshold: None,
        assume_now: None,
        telemetry_label: None,
        telemetry_region: None,
        gateway_manifest_id: None,
        gateway_manifest_cid: None,
        gateway_manifest_envelope_present: false,
        transport_policy: None,
        transport_policy_override: None,
        anonymity_policy: Some(AnonymityPolicy::GuardPq),
        anonymity_policy_override: None,
    });
    let map = metadata.as_object().expect("metadata object");
    assert_eq!(
        map.get("anonymity_policy").and_then(Value::as_str),
        Some("anon-guard-pq")
    );
    assert_eq!(
        map.get("anonymity_policy_override")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        map.get("anonymity_policy_override_label")
            .is_none_or(Value::is_null)
    );
    let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
        scoreboard_mode: true,
        allow_implicit_metadata: false,
        provider_count: 1,
        gateway_provider_count: 1,
        provider_mix: provider_mix_label(1, 1),
        max_parallel: None,
        max_peers: None,
        retry_budget: None,
        failure_threshold: None,
        assume_now: None,
        telemetry_label: None,
        telemetry_region: None,
        gateway_manifest_id: None,
        gateway_manifest_cid: None,
        gateway_manifest_envelope_present: false,
        transport_policy: None,
        transport_policy_override: None,
        anonymity_policy: Some(AnonymityPolicy::MajorityPq),
        anonymity_policy_override: Some(AnonymityPolicy::StrictPq),
    });
    let map = metadata.as_object().expect("metadata object");
    assert_eq!(
        map.get("anonymity_policy").and_then(Value::as_str),
        Some("anon-strict-pq")
    );
    assert_eq!(
        map.get("anonymity_policy_override")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        map.get("anonymity_policy_override_label")
            .and_then(Value::as_str),
        Some("anon-strict-pq")
    );
}
#[test]
fn gateway_manifest_flag_requires_gateway_providers() {
    let envelope = sample_gateway_manifest_envelope_b64();
    assert!(
        !gateway_manifest_present(&Some(envelope.clone()), false),
        "presence flag should be false when no gateway providers are active"
    );
    assert!(gateway_manifest_present(&Some(envelope), true));
}
#[test]
fn gateway_manifest_flag_rejects_unknown_suite() {
    let mut envelope = sample_gateway_manifest_envelope();
    envelope.suite = "unknown-suite".to_string();
    let encoded = encode_gateway_manifest_envelope(&envelope);
    assert!(!gateway_manifest_present(&Some(encoded), true));
}
#[test]
fn gateway_manifest_flag_ignores_blank_values() {
    assert!(!gateway_manifest_present(&Some("   ".to_string()), true));
    assert!(!gateway_manifest_present(&None, true));
}
#[test]
fn gateway_manifest_flag_rejects_non_norito_payloads() {
    let bogus = base64::engine::general_purpose::STANDARD.encode(b"not-an-envelope");
    assert!(!gateway_manifest_present(&Some(bogus), true));
}
#[test]
fn gateway_manifest_flag_rejects_invalid_base64() {
    assert!(!gateway_manifest_present(
        &Some("not-base64!?".to_string()),
        true
    ));
}
#[test]
fn classify_scoreboard_aliases_maps_provider_ids() {
    let eligible_entry = scoreboard::ScoreboardEntry {
        normalised_weight: 0.0,
        raw_score: 0.0,
        provider: FetchProvider::new("did:sora:alpha"),
        eligibility: scoreboard::Eligibility::Eligible,
    };
    let ineligible_entry = scoreboard::ScoreboardEntry {
        normalised_weight: 0.0,
        raw_score: 0.0,
        provider: FetchProvider::new("did:sora:beta"),
        eligibility: scoreboard::Eligibility::Ineligible(
            scoreboard::IneligibilityReason::TelemetryPenalty,
        ),
    };
    let aliases = vec!["alpha".to_string(), "beta".to_string()];
    let (eligible, lookup, ineligible) =
        classify_scoreboard_aliases(&[eligible_entry, ineligible_entry], &aliases);
    assert!(eligible.contains("alpha"));
    assert!(!eligible.contains("beta"));
    assert_eq!(lookup.get("alpha").map(String::as_str), Some("alpha"));
    assert_eq!(
        lookup.get("did:sora:alpha").map(String::as_str),
        Some("alpha")
    );
    assert!(!lookup.contains_key("beta"));
    assert_eq!(ineligible.len(), 1);
    assert_eq!(ineligible[0].0, "beta");
    assert_eq!(
        ineligible[0].1,
        scoreboard::IneligibilityReason::TelemetryPenalty
    );
}
fn write_payload(path: &Path, size: usize) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    for (idx, byte) in buf.iter_mut().enumerate() {
        *byte = (idx as u8).wrapping_mul(31).wrapping_add(7);
    }
    fs::write(path, &buf).expect("write payload");
    buf
}
#[test]
fn parse_provider_with_concurrency_and_weight() {
    let spec = parse_provider_spec("alpha=/tmp/payload#4@3").expect("parse");
    assert_eq!(spec.name, "alpha");
    assert_eq!(spec.path, PathBuf::from("/tmp/payload"));
    assert_eq!(spec.max_concurrent.get(), 4);
    assert_eq!(spec.weight.unwrap().get(), 3);
    assert!(spec.concurrency_explicit);
    assert!(spec.weight_explicit);
}
#[test]
fn parse_provider_with_weight_only() {
    let spec = parse_provider_spec("beta=/data/payload@5").expect("parse");
    assert_eq!(spec.max_concurrent.get(), 2);
    assert_eq!(spec.weight.unwrap().get(), 5);
    assert!(!spec.concurrency_explicit);
    assert!(spec.weight_explicit);
}
#[test]
fn parse_provider_with_concurrency_only() {
    let spec = parse_provider_spec("gamma=/srv/payload#6").expect("parse");
    assert_eq!(spec.max_concurrent.get(), 6);
    assert_eq!(spec.weight.unwrap().get(), 1);
    assert!(spec.concurrency_explicit);
    assert!(!spec.weight_explicit);
}
#[test]
fn parse_provider_defaults_when_flags_omitted() {
    let spec = parse_provider_spec("delta=/srv/payload").expect("parse");
    assert_eq!(spec.max_concurrent.get(), 2);
    assert_eq!(spec.weight.unwrap().get(), 1);
    assert!(!spec.concurrency_explicit);
    assert!(!spec.weight_explicit);
}
#[test]
fn parse_provider_rejects_noncanonical_concurrency() {
    for value in [
        "alpha=/tmp/payload#0",
        "alpha=/tmp/payload#04",
        "alpha=/tmp/payload#+4",
        "alpha=/tmp/payload# 4",
        "alpha=/tmp/payload#4 ",
        "alpha=/tmp/payload#1844674407370955161618446744073709551616",
    ] {
        let err = parse_provider_spec(value).expect_err("invalid concurrency must fail");
        assert!(
            err.contains("provider concurrency"),
            "unexpected error for {value}: {err}"
        );
    }
}
#[test]
fn parse_provider_rejects_noncanonical_weight() {
    for value in [
        "alpha=/tmp/payload@0",
        "alpha=/tmp/payload@03",
        "alpha=/tmp/payload@+3",
        "alpha=/tmp/payload@ 3",
        "alpha=/tmp/payload@3 ",
        "alpha=/tmp/payload@4294967296",
    ] {
        let err = parse_provider_spec(value).expect_err("invalid weight must fail");
        assert!(
            err.contains("provider weight"),
            "unexpected error for {value}: {err}"
        );
    }
}
#[test]
fn parse_usize_rejects_noncanonical_limit_tokens() {
    assert_eq!(parse_usize("1", "--max-peers").expect("canonical one"), 1);
    assert_eq!(
        parse_usize("42", "--max-parallel").expect("canonical value"),
        42
    );
    for value in [
        "0",
        "00",
        "01",
        "+1",
        "1 ",
        " 1",
        "1844674407370955161618446744073709551616",
    ] {
        let err = parse_usize(value, "--retry-budget").expect_err("invalid limit must fail");
        assert!(
            err.contains("--retry-budget"),
            "unexpected error for {value}: {err}"
        );
    }
}
#[test]
fn parse_u64_value_rejects_noncanonical_unsigned_tokens() {
    assert_eq!(
        parse_u64_value("0", "--assume-now").expect("canonical zero"),
        0
    );
    assert_eq!(
        parse_u64_value("42", "--expect-payload-len").expect("canonical length"),
        42
    );
    for value in ["", "00", "01", "+1", "1 ", " 1", "18446744073709551616"] {
        let err = parse_u64_value(value, "--assume-now").expect_err("invalid u64 token must fail");
        assert!(
            err.contains("--assume-now"),
            "unexpected error for {value:?}: {err}"
        );
    }
}
#[test]
fn parse_boost_provider_rejects_noncanonical_delta() {
    assert_eq!(
        parse_boost_provider("alpha:0").expect("canonical zero"),
        ("alpha".to_string(), 0)
    );
    assert_eq!(
        parse_boost_provider("alpha:-3").expect("canonical negative"),
        ("alpha".to_string(), -3)
    );
    assert_eq!(
        parse_boost_provider("alpha:7").expect("canonical positive"),
        ("alpha".to_string(), 7)
    );
    for value in [
        "alpha",
        ":1",
        "alpha:",
        "alpha:+1",
        "alpha:-0",
        "alpha:-01",
        "alpha:01",
        "alpha:1 ",
        "alpha: 1",
        "alpha:9223372036854775808",
        "alpha:-9223372036854775809",
    ] {
        parse_boost_provider(value).expect_err("invalid boost provider must fail");
    }
}
#[test]
fn telemetry_json_rejects_noncanonical_unsigned_string_fields() {
    let mut last_updated = Map::new();
    last_updated.insert("provider_id".into(), Value::String("provider-a".into()));
    last_updated.insert("last_updated_unix".into(), Value::String("01".into()));
    let err = telemetry_from_value(Value::Array(vec![Value::Object(last_updated)]))
        .expect_err("noncanonical timestamp string should fail");
    assert!(
        err.contains("last_updated_unix") && err.contains("canonical unsigned"),
        "unexpected timestamp error: {err}"
    );
    let mut reputation = Map::new();
    reputation.insert("provider_id".into(), Value::String("provider-a".into()));
    reputation.insert("reputation_score_bps".into(), Value::String("+9200".into()));
    let err = telemetry_from_value(Value::Array(vec![Value::Object(reputation)]))
        .expect_err("noncanonical reputation string should fail");
    assert!(
        err.contains("reputation_score_bps") && err.contains("canonical unsigned"),
        "unexpected reputation error: {err}"
    );
}
#[test]
fn telemetry_json_rejects_nonfinite_metric_strings() {
    for field in [
        "qos_score",
        "latency_p95_ms",
        "failure_rate_ewma",
        "token_health",
        "staking_weight",
    ] {
        for value in ["NaN", "inf", "-inf", "Infinity"] {
            let mut telemetry_entry = Map::new();
            telemetry_entry.insert("provider_id".into(), Value::String("provider-a".into()));
            telemetry_entry.insert(field.into(), Value::String(value.into()));
            let err = telemetry_from_value(Value::Array(vec![Value::Object(telemetry_entry)]))
                .expect_err("non-finite telemetry string should fail");
            assert!(
                err.contains(field) && err.contains("finite"),
                "unexpected telemetry error for {field}={value}: {err}"
            );
        }
    }
}
#[test]
fn fetch_cli_applies_provider_advert() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 8 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let advert_path = temp_path.join("provider.advert");
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let now = unix_time_now().unwrap_or(1_700_000_000);
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0x11; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
        stake: StakePointer {
            pool_id: [0x22; 32],
            stake_amount: xor_micro(1_000_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 500,
            max_concurrent_streams: 5,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
        ],
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "localhost".into(),
            metadata: vec![EndpointMetadata {
                key: EndpointMetadataKey::Alpn,
                value: b"h2".to_vec(),
            }],
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: Some("test provider".into()),
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let signing_key = SigningKey::from_bytes(&[0xAB; 32]);
    let advert = signed_provider_advert(advert_body, &signing_key, now, now + 3_600, false);
    let advert_bytes = to_bytes(&advert).expect("serialize advert");
    fs::write(&advert_path, advert_bytes).expect("write advert");
    let output_path = temp_path.join("assembled.bin");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");
    let provider_reports = report
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider reports array");
    let provider = provider_reports.first().expect("provider entry");
    let metadata = provider
        .get("metadata")
        .and_then(Value::as_object)
        .expect("metadata present");
    assert_eq!(
        metadata
            .get("availability")
            .and_then(Value::as_str)
            .expect("availability"),
        "hot"
    );
    assert_eq!(
        metadata
            .get("max_streams")
            .and_then(Value::as_u64)
            .expect("max_streams") as u16,
        5
    );
    assert_eq!(
        metadata
            .get("stake_amount")
            .and_then(Value::as_str)
            .expect("stake_amount"),
        "1000000"
    );
    assert!(
        metadata
            .get("rendezvous_topics")
            .and_then(Value::as_array)
            .is_some_and(|topics| !topics.is_empty())
    );
    assert_eq!(
        metadata
            .get("allow_unknown_capabilities")
            .and_then(Value::as_bool),
        Some(false)
    );
    let capabilities = metadata
        .get("capabilities")
        .and_then(Value::as_array)
        .expect("capabilities present");
    let capability_names: Vec<&str> = capabilities.iter().filter_map(Value::as_str).collect();
    assert!(capability_names.contains(&"chunk_range_fetch"));
    let aliases = metadata
        .get("profile_aliases")
        .and_then(Value::as_array)
        .expect("profile_aliases present");
    let alias_strings: Vec<&str> = aliases.iter().filter_map(Value::as_str).collect();
    assert!(alias_strings.contains(&profile_handle.as_str()));
    assert_eq!(
        metadata.get("refresh_deadline").and_then(Value::as_u64),
        Some(now + 1_800)
    );
    let range = metadata
        .get("range_capability")
        .and_then(Value::as_object)
        .expect("range capability present");
    let profile = ChunkProfile::DEFAULT;
    assert_eq!(
        range.get("max_chunk_span").and_then(Value::as_u64),
        Some(profile.max_size as u64)
    );
    assert_eq!(
        range.get("min_granularity").and_then(Value::as_u64),
        Some(profile.min_size as u64)
    );
    let stream_budget = metadata
        .get("stream_budget")
        .and_then(Value::as_object)
        .expect("stream budget present");
    assert_eq!(
        stream_budget.get("max_in_flight").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        stream_budget
            .get("max_bytes_per_sec")
            .and_then(Value::as_u64),
        Some(5_000_000)
    );
    let transport_hints = metadata
        .get("transport_hints")
        .and_then(Value::as_array)
        .expect("transport hints present");
    assert_eq!(transport_hints.len(), 1);
    let hint = transport_hints[0]
        .as_object()
        .expect("transport hint object");
    assert_eq!(
        hint.get("protocol").and_then(Value::as_str),
        Some("torii_http_range")
    );
    assert_eq!(hint.get("priority").and_then(Value::as_u64), Some(0));
    let assembled = fs::read(&output_path).expect("read payload");
    assert_eq!(assembled, payload);
}
#[test]
fn fetch_cli_persists_scoreboard() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 8 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let advert_path = temp_path.join("provider.advert");
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let profile_aliases = default_profile_aliases();
    let now = unix_time_now().unwrap_or(1_700_000_000);
    let provider_id = [0x44; 32];
    let advert_body = ProviderAdvertBodyV1 {
        provider_id,
        profile_id: profile_handle.clone(),
        profile_aliases: Some(profile_aliases.clone()),
        stake: StakePointer {
            pool_id: [0x55; 32],
            stake_amount: xor_micro(750_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 400,
            max_concurrent_streams: 4,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
        ],
        endpoints: vec![sample_endpoint()],
        rendezvous_topics: sample_rendezvous_topics("alpha"),
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: Some("scoreboard integration".into()),
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let signing_key = SigningKey::from_bytes(&[0x9F; 32]);
    let advert = signed_provider_advert(advert_body, &signing_key, now, now + 3_600, false);
    let advert_bytes = to_bytes(&advert).expect("serialize advert");
    fs::write(&advert_path, advert_bytes).expect("write advert");
    let telemetry_path = temp_path.join("telemetry.json");
    let mut telemetry_entry = Map::new();
    telemetry_entry.insert("provider_id".into(), Value::String(to_hex(&provider_id)));
    telemetry_entry.insert("qos_score".into(), Value::from(92.0));
    telemetry_entry.insert("latency_p95_ms".into(), Value::from(180.0));
    telemetry_entry.insert("failure_rate_ewma".into(), Value::from(0.03));
    telemetry_entry.insert("token_health".into(), Value::from(0.96));
    telemetry_entry.insert("staking_weight".into(), Value::from(1.05));
    telemetry_entry.insert("reputation_score_bps".into(), Value::from(9_200_u64));
    telemetry_entry.insert("last_updated_unix".into(), Value::from(now));
    let telemetry = Value::Array(vec![Value::Object(telemetry_entry)]);
    fs::write(
        &telemetry_path,
        (norito::json::to_string_pretty(&telemetry).expect("telemetry json") + "\n").as_bytes(),
    )
    .expect("write telemetry");
    let scoreboard_path = temp_path.join("scoreboard.json");
    let output_path = temp_path.join("assembled.bin");
    sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--telemetry-json={}", telemetry_path.display()))
        .arg(format!("--scoreboard-out={}", scoreboard_path.display()))
        .arg("--use-scoreboard")
        .assert()
        .success();
    let scoreboard_contents = fs::read_to_string(&scoreboard_path).expect("read scoreboard");
    let scoreboard_value: Value =
        norito::json::from_str(&scoreboard_contents).expect("parse scoreboard");
    let entries = scoreboard_value
        .get("entries")
        .and_then(Value::as_array)
        .expect("entries array");
    assert_eq!(entries.len(), 1);
    let entry = entries[0].as_object().expect("scoreboard entry object");
    assert_eq!(
        entry
            .get("provider_id")
            .and_then(Value::as_str)
            .expect("provider id"),
        "alpha"
    );
    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload);
}
#[test]
fn telemetry_json_rejects_out_of_range_reputation_score() {
    let mut telemetry_entry = Map::new();
    telemetry_entry.insert("provider_id".into(), Value::String("provider-a".into()));
    telemetry_entry.insert("reputation_score_bps".into(), Value::from(10_001_u64));
    let err = telemetry_from_value(Value::Array(vec![Value::Object(telemetry_entry)]))
        .expect_err("out-of-range reputation score should fail");
    assert!(
        err.contains("reputation_score_bps"),
        "error should name the rejected field: {err}"
    );
}
#[test]
fn fetch_cli_score_policy_filters_providers() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path_alpha = temp_path.join("alpha.bin");
    let payload = write_payload(&payload_path_alpha, 8 * 1024);
    let payload_path_beta = temp_path.join("beta.bin");
    fs::write(&payload_path_beta, &payload).expect("write beta payload");
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let profile_aliases = default_profile_aliases();
    let now = unix_time_now().unwrap_or(1_700_000_000);
    let advert_alpha = ProviderAdvertBodyV1 {
        provider_id: [0x66; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(profile_aliases.clone()),
        stake: StakePointer {
            pool_id: [0x01; 32],
            stake_amount: xor_micro(600_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 400,
            max_concurrent_streams: 3,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
        ],
        endpoints: vec![sample_endpoint()],
        rendezvous_topics: sample_rendezvous_topics("alpha"),
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: Some("alpha provider".into()),
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let advert_beta = ProviderAdvertBodyV1 {
        provider_id: [0x77; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(profile_aliases.clone()),
        stake: StakePointer {
            pool_id: [0x02; 32],
            stake_amount: xor_micro(900_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 350,
            max_concurrent_streams: 4,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
        ],
        endpoints: vec![sample_endpoint()],
        rendezvous_topics: sample_rendezvous_topics("beta"),
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: Some("beta provider".into()),
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let advert_path_alpha = temp_path.join("alpha.advert");
    let advert_path_beta = temp_path.join("beta.advert");
    for (body, path, key_byte) in [
        (advert_alpha, &advert_path_alpha, 0xA1u8),
        (advert_beta, &advert_path_beta, 0xB2u8),
    ] {
        let signing_key = SigningKey::from_bytes(&[key_byte; 32]);
        let advert = signed_provider_advert(body, &signing_key, now, now + 3_600, false);
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        fs::write(path, advert_bytes).expect("write advert");
    }
    let metrics_path = temp_path.join("providers.json");
    let output_path = temp_path.join("assembled.bin");
    sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path_alpha.display()))
        .arg(format!("--provider=beta={}", payload_path_beta.display()))
        .arg(format!(
            "--provider-advert=alpha={}",
            advert_path_alpha.display()
        ))
        .arg(format!(
            "--provider-advert=beta={}",
            advert_path_beta.display()
        ))
        .arg(format!("--provider-metrics-out={}", metrics_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .arg("--deny-provider=alpha")
        .assert()
        .success();
    let metrics = fs::read_to_string(&metrics_path).expect("read provider metrics");
    let metrics_value: Value = norito::json::from_str(&metrics).expect("parse metrics");
    let entries = metrics_value.as_array().expect("provider metrics array");
    let alpha_entry = entries
        .iter()
        .find(|entry| {
            entry
                .get("provider")
                .and_then(Value::as_str)
                .map(|id| id == "alpha")
                .unwrap_or(false)
        })
        .expect("alpha entry present");
    assert_eq!(
        alpha_entry.get("successes").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(alpha_entry.get("failures").and_then(Value::as_u64), Some(0));
    assert_eq!(
        alpha_entry.get("disabled").and_then(Value::as_bool),
        Some(false)
    );
    let beta_entry = entries
        .iter()
        .find(|entry| {
            entry
                .get("provider")
                .and_then(Value::as_str)
                .map(|id| id == "beta")
                .unwrap_or(false)
        })
        .expect("beta entry present");
    let beta_successes = beta_entry
        .get("successes")
        .and_then(Value::as_u64)
        .expect("beta successes");
    assert!(beta_successes > 0);
    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload);
}
#[test]
fn fetch_cli_rejects_provider_without_range_capability() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let now = unix_time_now().unwrap_or(1_700_000_000);
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0xAA; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
        stake: StakePointer {
            pool_id: [0xBB; 32],
            stake_amount: xor_micro(1_500_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 400,
            max_concurrent_streams: 4,
        },
        capabilities: vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "localhost".into(),
            metadata: Vec::new(),
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: None,
        transport_hints: None,
    };
    let signing_key = SigningKey::from_bytes(&[0x01; 32]);
    let advert = signed_provider_advert(advert_body, &signing_key, now, now + 3_600, false);
    let advert_bytes = to_bytes(&advert).expect("serialize advert");
    let advert_path = temp_path.join("provider.advert");
    fs::write(&advert_path, advert_bytes).expect("write advert");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(stderr.contains("chunk_range_fetch capability"));
}
#[test]
fn fetch_cli_verifies_car_when_manifest_available() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 8 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let fetch_specs = plan.try_chunk_fetch_specs().expect("valid CAR plan");
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let mut car_digest = [0u8; 32];
    car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 1,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_hex = to_hex(&manifest_bytes);
    let mut manifest_obj = Map::new();
    manifest_obj.insert("version".into(), Value::from(1_u64));
    manifest_obj.insert("manifest_hex".into(), Value::from(manifest_hex));
    manifest_obj.insert(
        "car_digest_hex".into(),
        Value::from(to_hex(stats.car_archive_digest.as_bytes())),
    );
    manifest_obj.insert("car_size".into(), Value::from(stats.car_size));
    let mut report_obj = Map::new();
    report_obj.insert(
        "schema".into(),
        Value::from(MANIFEST_BUILDER_REPORT_SCHEMA_V1),
    );
    report_obj.insert("chunk_fetch_specs".into(), Value::Array(fetch_array));
    report_obj.insert(
        "payload_digest_hex".into(),
        Value::from(blake3::hash(&payload).to_hex().to_string()),
    );
    report_obj.insert("payload_len".into(), Value::from(payload.len() as u64));
    report_obj.insert("manifest".into(), Value::Object(manifest_obj));
    let manifest_path = temp_path.join("report.json");
    fs::write(
        &manifest_path,
        (to_string_pretty(&Value::Object(report_obj)).expect("json") + "\n").as_bytes(),
    )
    .expect("write manifest report");
    let car_path = temp_path.join("payload.car");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--manifest-report={}", manifest_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg("--allow-implicit-provider-metadata")
        .arg(format!("--car-out={}", car_path.display()))
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");
    let car_archive = report
        .get("car_archive")
        .and_then(Value::as_object)
        .expect("car_archive present");
    assert_eq!(
        car_archive.get("verified").and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        car_archive
            .get("por_leaf_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            > 0
    );
}
#[test]
fn fetch_cli_rejects_corrupted_payload_when_manifest_provided() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let fetch_specs = plan.try_chunk_fetch_specs().expect("valid CAR plan");
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let mut car_digest = [0u8; 32];
    car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 1,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let mut manifest_obj = Map::new();
    manifest_obj.insert("version".into(), Value::from(1_u64));
    manifest_obj.insert("manifest_hex".into(), Value::from(to_hex(&manifest_bytes)));
    manifest_obj.insert(
        "car_digest_hex".into(),
        Value::from(to_hex(stats.car_archive_digest.as_bytes())),
    );
    manifest_obj.insert("car_size".into(), Value::from(stats.car_size));
    let mut report_obj = Map::new();
    report_obj.insert(
        "schema".into(),
        Value::from(MANIFEST_BUILDER_REPORT_SCHEMA_V1),
    );
    report_obj.insert("chunk_fetch_specs".into(), Value::Array(fetch_array));
    report_obj.insert(
        "payload_digest_hex".into(),
        Value::from(blake3::hash(&payload).to_hex().to_string()),
    );
    report_obj.insert("payload_len".into(), Value::from(payload.len() as u64));
    report_obj.insert("manifest".into(), Value::Object(manifest_obj));
    let manifest_path = temp_path.join("report.json");
    fs::write(
        &manifest_path,
        (to_string_pretty(&Value::Object(report_obj)).expect("json") + "\n").as_bytes(),
    )
    .expect("write manifest report");
    // Corrupt the provider payload after the plan/manifest have been generated.
    let mut corrupted = fs::read(&payload_path).expect("read payload");
    corrupted[0] ^= 0xFF;
    fs::write(&payload_path, &corrupted).expect("rewrite payload");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--manifest-report={}", manifest_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg("--allow-implicit-provider-metadata")
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    let verification_failed = stderr.contains("CAR verification failed")
        || stderr.contains("chunk digest mismatch")
        || stderr.contains("payload length does not match")
        || stderr.contains("retry budget exhausted");
    assert!(
        verification_failed,
        "stderr did not include expected verification failure, got: {stderr}"
    );
}
#[test]
fn provider_advert_concurrency_respects_stream_budget() {
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0x42; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
        stake: StakePointer {
            pool_id: [0x24; 32],
            stake_amount: xor_micro(2_000_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Warm,
            max_retrieval_latency_ms: 800,
            max_concurrent_streams: 6,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
        ],
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "storage".into(),
            metadata: vec![],
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 5,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let advert = ProviderAdvertV1 {
        version: PROVIDER_ADVERT_VERSION_V1,
        issued_at: 0,
        expires_at: 3_600,
        body: advert_body,
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![0u8; PUBLIC_KEY_LENGTH],
            signature: vec![0u8; SIGNATURE_LENGTH],
        },
        signature_strict: true,
        allow_unknown_capabilities: false,
    };
    let metadata = provider_advert_to_metadata(advert).expect("metadata");
    assert!(metadata.supports_chunk_range);
    let concurrency = metadata.concurrency.expect("concurrency").get();
    assert_eq!(concurrency, sample_stream_budget().max_in_flight as usize);
    let budget = metadata
        .provider_metadata
        .stream_budget
        .expect("stream budget metadata");
    assert_eq!(budget.max_in_flight, sample_stream_budget().max_in_flight);
    assert_eq!(metadata.provider_metadata.transport_hints.len(), 1);
}
#[test]
fn fetch_cli_rejects_unknown_capabilities_without_allow_flag() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let now = unix_time_now().unwrap_or(1_700_000_000);
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0x21; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
        stake: StakePointer {
            pool_id: [0x31; 32],
            stake_amount: xor_micro(2_000_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 350,
            max_concurrent_streams: 6,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::VendorReserved,
                payload: vec![0xFF],
            },
        ],
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "localhost".into(),
            metadata: Vec::new(),
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let signing_key = SigningKey::from_bytes(&[0x55; 32]);
    let advert = signed_provider_advert(advert_body, &signing_key, now, now + 6_000, false);
    let advert_bytes = to_bytes(&advert).expect("serialize advert");
    let advert_path = temp_path.join("provider.advert");
    fs::write(&advert_path, advert_bytes).expect("write advert");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(stderr.contains("unsupported capabilities"));
}
#[test]
fn fetch_cli_ignores_unknown_capabilities_when_allowed() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let now = unix_time_now().unwrap_or(1_700_000_000);
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0x41; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
        stake: StakePointer {
            pool_id: [0x51; 32],
            stake_amount: xor_micro(2_500_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Warm,
            max_retrieval_latency_ms: 800,
            max_concurrent_streams: 3,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::VendorReserved,
                payload: vec![0xAA, 0xBB],
            },
        ],
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "localhost".into(),
            metadata: Vec::new(),
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 8,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let signing_key = SigningKey::from_bytes(&[0x77; 32]);
    let advert = signed_provider_advert(advert_body, &signing_key, now, now + 7_200, true);
    let advert_bytes = to_bytes(&advert).expect("serialize advert");
    let advert_path = temp_path.join("provider.advert");
    fs::write(&advert_path, advert_bytes).expect("write advert");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .assert()
        .success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(stderr.contains("advertised unknown capabilities"));
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");
    let provider_reports = report
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider reports array");
    let provider = provider_reports.first().expect("provider entry");
    let metadata = provider
        .get("metadata")
        .and_then(Value::as_object)
        .expect("metadata present");
    let capabilities = metadata
        .get("capabilities")
        .and_then(Value::as_array)
        .expect("capabilities present");
    let capability_names: Vec<&str> = capabilities.iter().filter_map(Value::as_str).collect();
    assert!(capability_names.contains(&"chunk_range_fetch"));
    assert!(capability_names.contains(&"torii_gateway"));
    assert!(!capability_names.contains(&"vendor_reserved"));
}
#[test]
fn fetch_cli_exposes_soranet_pq_labels() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let profile_aliases = default_profile_aliases();
    let now = unix_time_now().unwrap_or(1_700_000_000);
    let pq_payload = ProviderCapabilitySoranetPqV1 {
        supports_guard: true,
        supports_majority: true,
        supports_strict: false,
    }
    .to_bytes()
    .expect("encode soranet_pq");
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0x31; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(profile_aliases.clone()),
        stake: StakePointer {
            pool_id: [0x41; 32],
            stake_amount: xor_micro(1_500_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 600,
            max_concurrent_streams: 5,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::SoraNetHybridPq,
                payload: pq_payload,
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
        ],
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "relay.example.com".into(),
            metadata: Vec::new(),
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let signing_key = SigningKey::from_bytes(&[0x66; 32]);
    let advert = signed_provider_advert(advert_body, &signing_key, now, now + 7_200, false);
    let advert_bytes = to_bytes(&advert).expect("serialize advert");
    let advert_path = temp_path.join("provider.advert");
    fs::write(&advert_path, advert_bytes).expect("write advert");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");
    let provider_reports = report
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider reports array");
    let provider = provider_reports.first().expect("provider entry");
    let metadata = provider
        .get("metadata")
        .and_then(Value::as_object)
        .expect("metadata present");
    let capabilities = metadata
        .get("capabilities")
        .and_then(Value::as_array)
        .expect("capabilities present");
    let mut labels: Vec<&str> = capabilities.iter().filter_map(Value::as_str).collect();
    labels.sort();
    assert!(
        labels.contains(&"soranet_pq"),
        "expected base soranet_pq label, got {labels:?}"
    );
    assert!(
        labels.contains(&"soranet_pq_guard"),
        "expected guard label, got {labels:?}"
    );
    assert!(
        labels.contains(&"soranet_pq_majority"),
        "expected majority label, got {labels:?}"
    );
    assert!(
        !labels.contains(&"soranet_pq_strict"),
        "strict label should be absent, got {labels:?}"
    );
}
#[test]
fn fetch_cli_rejects_stale_provider_advert() {
    let (_tempdir, temp_path) = canonical_tempdir();
    let payload_path = temp_path.join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_path = temp_path.join("plan.json");
    write_canonical_plan(&plan_path, &plan);
    let descriptor = chunker_registry::default_descriptor();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let base_now = unix_time_now().unwrap_or(1_700_000_000);
    let issued_at = base_now.saturating_sub(13 * 3_600);
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: [0x61; 32],
        profile_id: profile_handle.clone(),
        profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
        stake: StakePointer {
            pool_id: [0x71; 32],
            stake_amount: xor_micro(1_000_000),
        },
        qos: QosHints {
            availability: AvailabilityTier::Warm,
            max_retrieval_latency_ms: 900,
            max_concurrent_streams: 3,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_capability_payload(),
            },
        ],
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "localhost".into(),
            metadata: Vec::new(),
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: Some(sample_stream_budget()),
        transport_hints: Some(sample_transport_hints()),
    };
    let signing_key = SigningKey::from_bytes(&[0x91; 32]);
    let advert = signed_provider_advert(
        advert_body,
        &signing_key,
        issued_at,
        issued_at + 24 * 3_600,
        false,
    );
    let advert_bytes = to_bytes(&advert).expect("serialize advert");
    let advert_path = temp_path.join("provider.advert");
    fs::write(&advert_path, advert_bytes).expect("write advert");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(stderr.contains("is stale"));
}
#[test]
fn ensure_range_capability_detects_max_span_violation() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(16 * 1024).collect();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let chunk_len = plan
        .chunks
        .first()
        .map(|chunk| chunk.length)
        .expect("chunk length present");
    assert!(chunk_len > 1);
    let mut metadata = ProviderMetadata::new();
    metadata.range_capability = Some(RangeCapability {
        max_chunk_span: chunk_len - 1,
        min_granularity: 1,
        supports_sparse_offsets: false,
        requires_alignment: false,
        supports_merkle_proof: false,
    });
    let err =
        ensure_range_capability_satisfies_plan(&plan, "alpha", &metadata).expect_err("should fail");
    assert!(
        err.contains("max_chunk_span"),
        "error should mention max_chunk_span, got {err}"
    );
}
#[test]
fn ensure_range_capability_detects_alignment_violation() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(32 * 1024).collect();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let chunk_len = plan
        .chunks
        .first()
        .map(|chunk| chunk.length)
        .expect("chunk length present");
    let mut metadata = ProviderMetadata::new();
    metadata.range_capability = Some(RangeCapability {
        max_chunk_span: chunk_len.saturating_mul(4),
        min_granularity: chunk_len.saturating_mul(2),
        supports_sparse_offsets: false,
        requires_alignment: true,
        supports_merkle_proof: false,
    });
    let err =
        ensure_range_capability_satisfies_plan(&plan, "beta", &metadata).expect_err("should fail");
    assert!(
        err.contains("alignment"),
        "error should mention alignment, got {err}"
    );
}
