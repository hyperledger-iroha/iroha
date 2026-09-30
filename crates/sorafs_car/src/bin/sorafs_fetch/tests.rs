//! CLI fetch, integrity, streaming-output and provider-metadata regression tests.

use super::*;
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, SigningKey};
use norito::to_bytes;
use sorafs_car::multi_fetch::{
    ChunkReceipt, FetchOutcome, FetchProvider, ProviderId, ProviderReport,
};
use sorafs_manifest::{
    AdvertEndpoint, AdvertSignature, CapabilityTlv, EndpointKind, PROVIDER_ADVERT_VERSION_V1,
    PathDiversityPolicy, ProviderAdvertBodyV1, QosHints, RendezvousTopic, SignatureAlgorithm,
    StakePointer, hybrid_envelope::HybridKemBundleV1,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tempfile::NamedTempFile;
#[path = "../../../tests/support/sorafs_fetch.rs"]
mod support;
use support::*;
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

#[test]
fn output_spool_creates_parent_and_publishes_all_bytes() {
    let (_temp, temp_path) = canonical_tempdir();
    let output_path = temp_path.join("nested").join("payload.bin");
    let mut spool = private_output_spool(&output_path).expect("private output");
    spool.write_all(b"sorafs-fetch-output").unwrap();
    assert!(!output_path.exists());
    publish_output_spool(spool, &output_path).expect("publish verified output");
    assert_eq!(
        fs::read(&output_path).expect("read output"),
        b"sorafs-fetch-output"
    );
}

#[test]
fn interrupted_streaming_writer_discards_prefix_without_replacing_output() {
    let (_temp, directory) = canonical_tempdir();
    let output = directory.join("payload.bin");
    fs::write(&output, b"previous verified payload").unwrap();
    let mut writer = StreamingWriter::create(&output).unwrap();
    let temporary_path = writer.writer.get_ref().path().to_owned();
    writer.write_chunk(0, b"unverified prefix").unwrap();
    writer.flush().unwrap();
    let mut replay = Vec::new();
    writer.reader().unwrap().read_to_end(&mut replay).unwrap();
    assert_eq!(replay, b"unverified prefix");
    assert_eq!(writer.total_written(), replay.len() as u64);
    assert_eq!(writer.current_digest(), *blake3::hash(&replay).as_bytes());
    drop(writer);
    assert!(!temporary_path.exists());
    assert_eq!(fs::read(output).unwrap(), b"previous verified payload");
}

#[test]
fn streaming_writer_publishes_only_complete_verified_bytes() {
    let (_temp, directory) = canonical_tempdir();
    let output = directory.join("payload.bin");
    let mut writer = StreamingWriter::create(&output).unwrap();
    writer.write_chunk(0, b"verified bytes").unwrap();
    writer.publish(&output).unwrap();
    assert_eq!(fs::read(output).unwrap(), b"verified bytes");
}

#[test]
fn failed_output_publication_cleans_up_temporary_bytes() {
    let (_temp, directory) = canonical_tempdir();
    let output = directory.join("payload.bin");
    let spool = private_output_spool(&output).unwrap();
    let temporary_path = spool.path().to_owned();
    fs::create_dir(&output).unwrap();
    assert!(publish_output_spool(spool, &output).is_err());
    assert!(output.is_dir());
    assert!(!temporary_path.exists());
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
fn output_spool_rejects_symlink_output() {
    let (_temp, temp_path) = canonical_tempdir();
    let target_path = temp_path.join("target.car");
    fs::write(&target_path, b"unchanged").expect("write target");
    let car_path = temp_path.join("payload.car");
    std::os::unix::fs::symlink(&target_path, &car_path).expect("create symlink");
    let err = match private_output_spool(&car_path) {
        Ok(_) => panic!("symlink output should be rejected"),
        Err(err) => err,
    };
    assert!(
        err.contains("must not be a symlink"),
        "unexpected error: {err}"
    );
    assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged");
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
        network_id: [0xA1; 32],
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
