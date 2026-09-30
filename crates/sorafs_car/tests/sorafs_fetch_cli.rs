//! Same-source subprocess tests for the actual SoraFS fetch CLI.
use assert_cmd::Command as AssertCommand;
use ed25519_dalek::SigningKey;
use norito::{
    json::{Map, Value, to_string_pretty},
    to_bytes,
};
use sorafs_car::{
    CarBuildPlan, CarWriter, chunker_registry, compute_chunk_plan_digest_sha3, compute_por_root,
    fetch_plan::{MANIFEST_BUILDER_REPORT_SCHEMA_V1, chunk_fetch_plan_to_string},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    AdvertEndpoint, AvailabilityTier, CapabilityTlv, CapabilityType, DagCodecId, EndpointKind,
    EndpointMetadata, EndpointMetadataKey, GovernanceProofs, ManifestBuilder, PathDiversityPolicy,
    PinPolicy, ProviderAdvertBodyV1, QosHints, RendezvousTopic, StakePointer, StorageClass,
    provider_advert::ProviderCapabilitySoranetPqV1,
};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
#[path = "support/sorafs_fetch.rs"]
mod support;
use support::*;
const TEST_NETWORK_ID_ARG: &str =
    "--network-id=a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
fn sorafs_fetch_cmd() -> AssertCommand {
    AssertCommand::new(env!("CARGO_BIN_EXE_sorafs_fetch"))
}
fn unix_time_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}
fn write_canonical_plan(path: &Path, plan: &CarBuildPlan) {
    fs::write(
        path,
        chunk_fetch_plan_to_string(plan).expect("render canonical plan"),
    )
    .expect("write canonical plan");
}
fn default_profile_aliases() -> Vec<String> {
    chunker_registry::default_descriptor()
        .aliases
        .iter()
        .map(|alias| alias.to_string())
        .collect()
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
        .arg(TEST_NETWORK_ID_ARG)
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
        "1"
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
    telemetry_entry.insert(
        "provider_id".into(),
        Value::String(hex::encode(provider_id)),
    );
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
        .arg(TEST_NETWORK_ID_ARG)
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
        .arg(TEST_NETWORK_ID_ARG)
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
        .arg(TEST_NETWORK_ID_ARG)
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
            obj.insert(
                "digest_blake3".into(),
                Value::from(hex::encode(spec.digest)),
            );
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
    let manifest_hex = hex::encode(&manifest_bytes);
    let mut manifest_obj = Map::new();
    manifest_obj.insert("version".into(), Value::from(1_u64));
    manifest_obj.insert("manifest_hex".into(), Value::from(manifest_hex));
    manifest_obj.insert(
        "car_digest_hex".into(),
        Value::from(hex::encode(stats.car_archive_digest.as_bytes())),
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
            obj.insert(
                "digest_blake3".into(),
                Value::from(hex::encode(spec.digest)),
            );
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
    manifest_obj.insert(
        "manifest_hex".into(),
        Value::from(hex::encode(&manifest_bytes)),
    );
    manifest_obj.insert(
        "car_digest_hex".into(),
        Value::from(hex::encode(stats.car_archive_digest.as_bytes())),
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
        .arg(TEST_NETWORK_ID_ARG)
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
        .arg(TEST_NETWORK_ID_ARG)
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
        .arg(TEST_NETWORK_ID_ARG)
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
        .arg(TEST_NETWORK_ID_ARG)
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(stderr.contains("is stale"));
}
