#![cfg(feature = "cli")]
#![cfg_attr(feature = "cli", allow(unexpected_cfgs))]

use std::{
    convert::TryInto,
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use assert_cmd::Command as AssertCommand;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use blake3::hash as blake3_hash;
use ed25519_dalek::{Signature, SigningKey, Verifier, VerifyingKey};
use hex::{decode as hex_decode, encode as hex_encode};
use httpmock::prelude::*;
use iroha_config::parameters::defaults::streaming::soranet::PROVISION_SPOOL_DIR;
use iroha_data_model::taikai::TaikaiSegmentEnvelopeV1;
use norito::{
    decode_from_bytes,
    derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize},
    json::{Map, Value, from_slice, to_vec},
    to_bytes,
};
use sha3::{Digest, Sha3_256};
use sorafs_car::{
    CarBuildPlan, CarWriter, ChunkStore, chunker_registry,
    fetch_plan::{chunk_fetch_specs_from_json, chunk_fetch_specs_to_string},
    por_json::{proof_to_value, sample_to_map},
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1,
    ManualPorChallengeV1, POR_CHALLENGE_STATUS_VERSION_V1, POR_WEEKLY_REPORT_VERSION_V1, PinPolicy,
    PorChallengeOutcome, PorChallengeStatusV1, PorProviderSummaryV1, PorReportIsoWeek,
    PorSlashingEventV1, PorWeeklyReportV1, StorageClass, StreamTokenBodyV1, StreamTokenV1,
    XorAmount,
};
use tempfile::tempdir;

fn sorafs_cli_cmd() -> AssertCommand {
    let path = env::var("CARGO_BIN_EXE_sorafs_cli")
        .expect("CARGO_BIN_EXE_sorafs_cli must be set by Cargo");
    AssertCommand::new(path)
}

fn taikai_car_cmd() -> AssertCommand {
    let path = env::var("CARGO_BIN_EXE_taikai_car")
        .expect("CARGO_BIN_EXE_taikai_car must be set by Cargo");
    AssertCommand::new(path)
}

fn make_stream_token_b64(
    manifest_id_hex: &str,
    provider_id_hex: &str,
    profile: &str,
    max_streams: u16,
) -> String {
    let mut provider_id = [0u8; 32];
    provider_id.copy_from_slice(&hex_decode(provider_id_hex).expect("decode provider identifier"));
    let token = StreamTokenV1 {
        body: StreamTokenBodyV1 {
            token_id: "01J9TK3GR0XM6YQF7WQXA9Z2SF".to_string(),
            manifest_cid: hex_decode(manifest_id_hex).expect("decode manifest id"),
            provider_id,
            profile_handle: profile.to_string(),
            max_streams,
            ttl_epoch: 9_999_999_999,
            rate_limit_bytes: 8 * 1024 * 1024,
            issued_at: 1_735_000_000,
            requests_per_minute: 120,
            token_pk_version: 1,
        },
        signature: vec![0; 64],
    };
    let bytes = to_bytes(&token).expect("encode stream token");
    BASE64_STANDARD.encode(bytes)
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
struct TestChallengeAuthScopeV1 {
    manifest_digest: [u8; 32],
    #[norito(default)]
    providers: Vec<[u8; 32]>,
    #[norito(default)]
    max_requests: u16,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
struct TestChallengeAuthTokenV1 {
    version: u8,
    token_id: String,
    operator_account: String,
    expires_at: u64,
    #[norito(default)]
    scopes: Vec<TestChallengeAuthScopeV1>,
    #[norito(default)]
    justification: Option<String>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
struct TestManualPorTriggerRequestV1 {
    version: u8,
    challenge: ManualPorChallengeV1,
    auth_token: TestChallengeAuthTokenV1,
}

#[test]
fn car_pack_emits_car_plan_and_summary() {
    let tempdir = tempdir().expect("tempdir");
    let input_path = tempdir.path().join("payload.bin");
    let mut payload = Vec::with_capacity(1024);
    for idx in 0..1024 {
        payload.push((idx as u8).wrapping_mul(17).wrapping_add(3));
    }
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.path().join("payload.car");
    let plan_path = tempdir.path().join("plan.json");
    let summary_path = tempdir.path().join("summary.json");

    let assert = sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value = norito::json::from_str(stdout.trim()).expect("stdout summary json");
    let summary_file_bytes = fs::read(&summary_path).expect("read summary file");
    let summary_file: Value =
        from_slice(&summary_file_bytes).expect("summary file json must parse");
    assert_eq!(
        summary_stdout, summary_file,
        "stdout summary should match file"
    );

    let payload_bytes = summary_stdout
        .get("payload_bytes")
        .and_then(Value::as_u64)
        .expect("payload_bytes");
    assert_eq!(payload_bytes, payload.len() as u64);
    assert_eq!(
        summary_stdout.get("chunker_handle").and_then(Value::as_str),
        Some("sorafs.sf1@1.0.0")
    );
    assert_eq!(
        summary_stdout.get("input_kind").and_then(Value::as_str),
        Some("file")
    );

    assert!(
        car_path.exists(),
        "expected CAR archive `{}` to be created",
        car_path.display()
    );

    let plan_bytes = fs::read(&plan_path).expect("read plan file");
    let plan_json: Value = from_slice(&plan_bytes).expect("parse plan json");
    let plan_array = plan_json.as_array().expect("plan should be json array");
    assert!(
        !plan_array.is_empty(),
        "expected plan array to contain chunk entries"
    );

    let chunk_count = summary_stdout
        .get("chunk_count")
        .and_then(Value::as_u64)
        .expect("chunk_count");
    assert_eq!(chunk_count, plan_array.len() as u64);
}

#[test]
fn manifest_build_consumes_summary_and_outputs_manifest() {
    let tempdir = tempdir().expect("tempdir");
    let input_path = tempdir.path().join("payload.bin");
    let payload: Vec<u8> = (0..2048).map(|i| (i as u8).wrapping_mul(13)).collect();
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.path().join("payload.car");
    let summary_path = tempdir.path().join("summary.json");

    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    let manifest_path = tempdir.path().join("manifest.to");
    let manifest_json_path = tempdir.path().join("manifest.json");

    let assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .arg(format!(
            "--manifest-json-out={}",
            manifest_json_path.display()
        ))
        .arg("--pin-min-replicas=3")
        .arg("--pin-storage-class=warm")
        .arg("--pin-retention-epoch=42")
        .arg("--metadata=env=dev")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("manifest summary json");
    assert_eq!(
        summary
            .get("manifest_path")
            .and_then(Value::as_str)
            .expect("manifest_path"),
        manifest_path.display().to_string()
    );

    let manifest_bytes = fs::read(&manifest_path).expect("read manifest bytes");
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("decode manifest");
    assert_eq!(manifest.content_length, payload.len() as u64);
    assert_eq!(manifest.pin_policy.min_replicas, 3);
    assert_eq!(manifest.pin_policy.storage_class, StorageClass::Warm);
    assert_eq!(manifest.pin_policy.retention_epoch, 42);
    assert!(
        manifest
            .metadata
            .iter()
            .any(|entry| entry.key == "env" && entry.value == "dev")
    );

    let manifest_json = fs::read(&manifest_json_path).expect("read manifest json");
    let manifest_value: Value = from_slice(&manifest_json).expect("parse manifest json");
    assert_eq!(
        manifest_value
            .get("pin_policy")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("min_replicas"))
            .and_then(Value::as_u64),
        Some(3)
    );
}

#[test]
fn por_status_outputs_table() {
    let server = MockServer::start();
    let status = PorChallengeStatusV1 {
        version: POR_CHALLENGE_STATUS_VERSION_V1,
        challenge_id: [0x11; 32],
        manifest_digest: [0x22; 32],
        provider_id: [0x33; 32],
        epoch_id: 42,
        drand_round: 100,
        status: PorChallengeOutcome::Pending,
        sample_count: 64,
        forced: false,
        issued_at: 1_700_000_000,
        responded_at: None,
        proof_digest: None,
        repair_task_id: None,
        failure_reason: None,
        verifier_latency_ms: Some(950),
    };
    let body = to_bytes(&vec![status]).expect("encode status list");
    let manifest_hex = hex_encode([0x22; 32]);
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/por/status")
            .query_param("manifest", manifest_hex.as_str());
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(body.clone());
    });

    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("status")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--manifest={manifest_hex}"))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(
        stdout.contains("pending"),
        "expected status output to mention pending status:\n{stdout}"
    );
}

#[test]
fn por_status_outputs_json() {
    let server = MockServer::start();
    let status = PorChallengeStatusV1 {
        version: POR_CHALLENGE_STATUS_VERSION_V1,
        challenge_id: [0xAA; 32],
        manifest_digest: [0xBB; 32],
        provider_id: [0xCC; 32],
        epoch_id: 7,
        drand_round: 55,
        status: PorChallengeOutcome::Verified,
        sample_count: 32,
        forced: true,
        issued_at: 1_700_000_100,
        responded_at: Some(1_700_000_120),
        proof_digest: Some([0xDD; 32]),
        repair_task_id: None,
        failure_reason: None,
        verifier_latency_ms: Some(1200),
    };
    let body = to_bytes(&vec![status]).expect("encode status list");
    server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/status");
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(body.clone());
    });

    let std_output = sorafs_cli_cmd()
        .arg("por")
        .arg("status")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--format=json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(std_output).expect("stdout utf8");
    assert!(
        stdout.contains("\"forced\": true"),
        "expected JSON output to include forced=true flag:\n{stdout}"
    );
}

#[test]
fn por_trigger_posts_manual_challenge() {
    let server = MockServer::start();
    let manifest_digest = [0x41; 32];
    let provider_id = [0x52; 32];
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock ok")
        .as_secs()
        + 3_600;
    let token = TestChallengeAuthTokenV1 {
        version: 1,
        token_id: "token-1".to_string(),
        operator_account: "council@sora".to_string(),
        expires_at,
        scopes: vec![TestChallengeAuthScopeV1 {
            manifest_digest,
            providers: vec![provider_id],
            max_requests: 1,
        }],
        justification: Some("incident probe".to_string()),
    };
    let token_bytes = to_bytes(&token).expect("encode token");
    let tempdir = tempdir().expect("tempdir");
    let token_path = tempdir.path().join("auth_token.to");
    fs::write(&token_path, &token_bytes).expect("write token");

    let trigger_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/por/trigger")
            .body_includes("\"reason\":\"incident_probe\"")
            .body_includes("\"requested_samples\":48");
        then.status(202)
            .header("content-type", "application/json")
            .body(r#"{"challenge_id":"cafebabe"}"#);
    });

    let manifest_hex = hex_encode(manifest_digest);
    let provider_hex = hex_encode(provider_id);
    sorafs_cli_cmd()
        .arg("por")
        .arg("trigger")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--manifest={manifest_hex}"))
        .arg(format!("--provider={provider_hex}"))
        .arg("--reason=incident_probe")
        .arg(format!("--auth-token={}", token_path.display()))
        .arg("--samples=48")
        .arg("--deadline-secs=900")
        .assert()
        .success();

    trigger_mock.assert();
}

#[test]
fn por_export_writes_file() {
    let server = MockServer::start();
    let payload = b"PARQUET".to_vec();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/por/export")
            .query_param("start_epoch", "10");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(payload.clone());
    });

    let tempdir = tempdir().expect("tempdir");
    let out_path = tempdir.path().join("report.parquet");
    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("export")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--start-epoch=10")
        .arg(format!("--out={}", out_path.display()))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(
        stdout.contains("exported"),
        "expected export command to report success:\n{stdout}"
    );
    let written = fs::read(&out_path).expect("read export file");
    assert_eq!(written, payload);
}

#[test]
fn por_report_outputs_markdown() {
    let server = MockServer::start();
    let provider_summary = PorProviderSummaryV1 {
        provider_id: [0x88; 32],
        manifest_count: 12,
        challenges: 96,
        successes: 94,
        failures: 2,
        forced: 0,
        success_rate: 0.979,
        first_failure_at: Some(1_700_000_300),
        last_success_latency_ms_p95: Some(1_850),
        repair_dispatched: true,
        pending_repairs: 1,
        ticket_id: Some("REP-123".to_string()),
    };
    let slashing_event = PorSlashingEventV1 {
        provider_id: [0x90; 32],
        manifest_digest: [0x91; 32],
        penalty_xor: XorAmount::from_micro(250_000_000),
        verdict_cid: "ipfs://verdict".to_string(),
        decided_at: 1_700_000_200,
    };
    let report = PorWeeklyReportV1 {
        version: POR_WEEKLY_REPORT_VERSION_V1,
        cycle: PorReportIsoWeek {
            year: 2025,
            week: 12,
        },
        generated_at: 1_700_000_400,
        challenges_total: 128,
        challenges_verified: 120,
        challenges_failed: 8,
        forced_challenges: 2,
        repairs_enqueued: 4,
        repairs_completed: 3,
        mean_latency_ms: Some(820.0),
        p95_latency_ms: Some(1_980.0),
        slashing_events: vec![slashing_event],
        providers_missing_vrf: vec![[0x77; 32]],
        top_offenders: vec![provider_summary],
        notes: Some("All forced challenges recovered within SLA.".to_string()),
    };
    report.validate().expect("report validates");
    let body = to_bytes(&report).expect("encode report");
    server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/report/2025-W12");
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(body.clone());
    });

    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("report")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--week=2025-W12")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(
        stdout.contains("PoR Weekly Health — 2025-W12"),
        "expected markdown output to include report heading:\n{stdout}"
    );
    assert!(
        stdout.contains("REP-123"),
        "expected markdown output to include ticket identifier:\n{stdout}"
    );
}

#[test]
fn proof_stream_command_consumes_ndjson() {
    let tempdir = tempdir().expect("tempdir");

    let payload: Vec<u8> = (0..512).map(|i| i as u8).collect();
    let mut chunk_store = ChunkStore::new();
    chunk_store.ingest_bytes(&payload);
    let por_root_hex = hex_encode(chunk_store.por_tree().root());

    let manifest = ManifestBuilder::new()
        .root_cid(vec![0xAA; 16])
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(
            sorafs_chunker::ChunkProfile::DEFAULT,
            BLAKE3_256_MULTIHASH_CODE,
        )
        .content_length(payload.len() as u64)
        .car_digest(blake3::hash(&payload).into())
        .car_size(payload.len() as u64)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("encode manifest");
    let manifest_path = tempdir.path().join("manifest.to");
    fs::write(&manifest_path, &manifest_bytes).expect("write manifest");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("digest").as_bytes());

    let samples = chunk_store.sample_leaves(1, 7, &payload);
    assert_eq!(samples.len(), 1);
    let (flat_index, por_proof) = samples.into_iter().next().expect("sample");

    let mut item_map = sample_to_map(flat_index, &por_proof);
    item_map.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_digest_hex.clone()),
    );
    let provider_id_hex = "22".repeat(32);
    item_map.insert(
        "provider_id_hex".into(),
        Value::from(provider_id_hex.clone()),
    );
    item_map.insert("proof_kind".into(), Value::from("por"));
    item_map.insert("result".into(), Value::from("success"));
    item_map.insert("latency_ms".into(), Value::from(42u64));
    item_map.insert("proof".into(), proof_to_value(&por_proof));
    let ndjson_line = norito::json::to_string(&Value::Object(item_map)).expect("serialize ndjson");

    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method("POST").path("/v1/sorafs/proof/stream");
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body(format!("{ndjson_line}\n"));
    });

    let summary_path = tempdir.path().join("summary.json");

    sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.url("")))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--samples=1")
        .arg("--sample-seed=7")
        .arg("--emit-events=false")
        .arg(format!("--por-root-hex={por_root_hex}"))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    let summary_bytes = fs::read(&summary_path).expect("read summary");
    let summary_value: Value = from_slice(&summary_bytes).expect("parse summary json");
    let metrics = summary_value
        .get("metrics")
        .and_then(Value::as_object)
        .expect("metrics object");
    assert_eq!(
        metrics
            .get("item_total")
            .and_then(Value::as_u64)
            .expect("item_total"),
        1
    );
    assert_eq!(
        summary_value
            .get("verification_failures")
            .and_then(Value::as_u64)
            .expect("verification_failures"),
        0
    );
}

#[test]
fn norito_build_compiles_contract() {
    let tempdir = tempdir().expect("tempdir");
    let source_path = PathBuf::from("../kotodama_lang/src/samples/kotodama_jp.ko");
    assert!(
        source_path.exists(),
        "expected Kotodama sample `{}` to exist",
        source_path.display()
    );

    let bytecode_path = tempdir.path().join("contract.to");
    let summary_path = tempdir.path().join("bytecode.json");

    let assert = sorafs_cli_cmd()
        .arg("norito")
        .arg("build")
        .arg(format!("--source={}", source_path.display()))
        .arg(format!("--bytecode-out={}", bytecode_path.display()))
        .arg("--abi-version=1")
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value = norito::json::from_str(stdout.trim()).expect("stdout summary json");
    let summary_file_bytes = fs::read(&summary_path).expect("read summary file");
    let summary_file: Value =
        from_slice(&summary_file_bytes).expect("summary file json must parse");
    assert_eq!(
        summary_stdout, summary_file,
        "stdout summary should match file"
    );

    assert!(
        bytecode_path.exists(),
        "expected bytecode to be written to `{}`",
        bytecode_path.display()
    );
    let bytecode = fs::read(&bytecode_path).expect("read bytecode");
    assert!(
        !bytecode.is_empty(),
        "compiled Kotodama bytecode should not be empty"
    );

    assert_eq!(
        summary_stdout
            .get("bytecode_path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .as_ref(),
        Some(&bytecode_path),
        "summary should report bytecode path"
    );
    assert_eq!(
        summary_stdout.get("source_kind").and_then(Value::as_str),
        Some("file")
    );
}

#[test]
fn manifest_sign_emits_bundle_and_signature() {
    let tempdir = tempdir().expect("tempdir");
    let input_path = tempdir.path().join("payload.bin");
    let payload: Vec<u8> = (0..512).map(|i| (i as u8).wrapping_mul(5)).collect();
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.path().join("payload.car");
    let plan_path = tempdir.path().join("plan.json");

    let summary_path = tempdir.path().join("plan_summary.json");

    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    let manifest_path = tempdir.path().join("manifest.to");
    let manifest_json_path = tempdir.path().join("manifest.json");

    sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .arg(format!(
            "--manifest-json-out={}",
            manifest_json_path.display()
        ))
        .arg("--pin-min-replicas=1")
        .arg("--pin-storage-class=hot")
        .arg("--pin-retention-epoch=1")
        .assert()
        .success();

    let bundle_path = tempdir.path().join("manifest.bundle.json");
    let signature_path = tempdir.path().join("manifest.sig");

    let token_header = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9"; // {"alg":"RS256","typ":"JWT"}
    let token_claims = "eyJzdWIiOiJ0ZXN0ZXIifQ"; // {"sub":"tester"}
    let identity_token = format!("{token_header}.{token_claims}.sig");

    let assert = sorafs_cli_cmd()
        .env("SIGSTORE_ID_TOKEN", &identity_token)
        .current_dir(tempdir.path())
        .arg("manifest")
        .arg("sign")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle-out={}", bundle_path.display()))
        .arg(format!("--signature-out={}", signature_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    assert_eq!(
        summary.get("identity_token_source").and_then(Value::as_str),
        Some("env:SIGSTORE_ID_TOKEN")
    );
    assert!(
        summary
            .get("signature_hex")
            .and_then(Value::as_str)
            .is_some()
    );

    let bundle_bytes = fs::read(&bundle_path).expect("read bundle bytes");
    let bundle_value: Value = from_slice(&bundle_bytes).expect("bundle json");
    assert_eq!(
        bundle_value
            .get("identity")
            .and_then(Value::as_object)
            .and_then(|identity| identity.get("token_source"))
            .and_then(Value::as_str),
        Some("env:SIGSTORE_ID_TOKEN")
    );

    let signature_contents = fs::read_to_string(&signature_path).expect("read signature");
    assert!(
        !signature_contents.trim().is_empty(),
        "signature should not be empty"
    );

    let token_hash = summary
        .get("identity_token_hash_blake3_hex")
        .and_then(Value::as_str)
        .expect("token hash in summary");

    let verify_bundle_assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("verify-signature")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle={}", bundle_path.display()))
        .arg(format!("--summary={}", summary_path.display()))
        .arg(format!("--expect-token-hash={token_hash}"))
        .assert()
        .success();
    let verify_bundle_stdout = String::from_utf8(verify_bundle_assert.get_output().stdout.clone())
        .expect("verify bundle stdout");
    let verify_bundle_summary: Value =
        norito::json::from_str(verify_bundle_stdout.trim()).expect("verify bundle summary");
    assert_eq!(
        verify_bundle_summary
            .get("verification_status")
            .and_then(Value::as_str),
        Some("ok")
    );

    let public_key_hex = summary
        .get("public_key_hex")
        .and_then(Value::as_str)
        .expect("public key hex in summary");

    let verify_signature_assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("verify-signature")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--signature={}", signature_path.display()))
        .arg(format!("--public-key-hex={public_key_hex}"))
        .arg(format!("--summary={}", summary_path.display()))
        .assert()
        .success();
    let verify_signature_stdout =
        String::from_utf8(verify_signature_assert.get_output().stdout.clone())
            .expect("verify signature stdout");
    let verify_signature_summary: Value =
        norito::json::from_str(verify_signature_stdout.trim()).expect("verify signature summary");
    assert_eq!(
        verify_signature_summary
            .get("verification_status")
            .and_then(Value::as_str),
        Some("ok")
    );
}

#[test]
fn manifest_submit_posts_payload() {
    let tempdir = tempdir().expect("tempdir");

    let input_path = tempdir.path().join("payload.bin");
    let payload: Vec<u8> = (0..4096).map(|idx| (idx as u8).wrapping_mul(31)).collect();
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.path().join("payload.car");
    let plan_path = tempdir.path().join("plan.json");
    let pack_summary_path = tempdir.path().join("pack_summary.json");

    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", pack_summary_path.display()))
        .assert()
        .success();

    let manifest_path = tempdir.path().join("manifest.to");
    sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", pack_summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .assert()
        .success();

    let verify_summary_path = tempdir.path().join("verify_summary.json");
    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("verify")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--summary-out={}", verify_summary_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value = norito::json::from_str(stdout.trim()).expect("verify summary json");
    let summary_file_bytes = fs::read(&verify_summary_path).expect("read verify summary file");
    let summary_file: Value =
        from_slice(&summary_file_bytes).expect("verify summary file json must parse");
    assert_eq!(
        summary_stdout, summary_file,
        "stdout summary should match file"
    );

    let expected_digest = compute_chunk_digest_hex(&plan_path);
    assert_eq!(
        summary_stdout
            .get("chunk_digest_sha3_hex")
            .and_then(Value::as_str),
        Some(expected_digest.as_str())
    );

    let plan_value: Value =
        from_slice(&fs::read(&plan_path).expect("read plan")).expect("plan json");
    let specs = chunk_fetch_specs_from_json(&plan_value).expect("plan specs");
    assert_eq!(
        summary_stdout.get("chunk_count").and_then(Value::as_u64),
        Some(specs.len() as u64)
    );

    let root_cids = summary_stdout
        .get("root_cids_hex")
        .and_then(Value::as_array)
        .expect("root cids array");
    assert!(
        !root_cids.is_empty(),
        "verify summary should report root CIDs"
    );

    assert_eq!(
        summary_stdout.get("chunker_handle").and_then(Value::as_str),
        Some("sorafs.sf1@1.0.0")
    );
    assert_eq!(
        summary_stdout
            .get("car_payload_bytes")
            .and_then(Value::as_u64),
        Some(payload.len() as u64)
    );
    assert_eq!(
        summary_stdout.get("payload_bytes").and_then(Value::as_u64),
        Some(payload.len() as u64)
    );

    let manifest_bytes = fs::read(&manifest_path).expect("read manifest");
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("decode manifest");
    let manifest_digest = manifest.digest().expect("manifest digest");
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    assert_eq!(
        summary_stdout
            .get("manifest_digest_hex")
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str())
    );
    let manifest_car_digest_hex = hex_encode(manifest.car_digest);
    assert_eq!(
        summary_stdout
            .get("manifest_car_digest_hex")
            .and_then(Value::as_str),
        Some(manifest_car_digest_hex.as_str())
    );

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pin/register");
        then.status(200)
            .header("Content-Type", "application/json")
            .body("{\"status\":\"ok\"}");
    });

    let submit_summary_path = tempdir.path().join("submit_summary.json");
    let response_path = tempdir.path().join("torii_response.bin");

    let assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("submit")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--submitted-epoch=7")
        .arg("--authority=alice@wonderland")
        .arg("--private-key=ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C")
        .arg(format!("--summary-out={}", submit_summary_path.display()))
        .arg(format!("--response-out={}", response_path.display()))
        .assert()
        .success();

    mock.assert_calls(1);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value = norito::json::from_str(stdout.trim()).expect("submit summary json");
    let summary_file_bytes = fs::read(&submit_summary_path).expect("read submit summary file");
    let summary_file: Value =
        from_slice(&summary_file_bytes).expect("submit summary file json must parse");
    assert_eq!(
        summary_stdout, summary_file,
        "stdout summary should match file"
    );

    let expected_endpoint = format!(
        "{}/v1/sorafs/pin/register",
        server.base_url().trim_end_matches('/')
    );
    assert_eq!(
        summary_stdout.get("torii_endpoint").and_then(Value::as_str),
        Some(expected_endpoint.as_str())
    );

    let expected_digest = compute_chunk_digest_hex(&plan_path);
    assert_eq!(
        summary_stdout
            .get("chunk_digest_sha3_hex")
            .and_then(Value::as_str),
        Some(expected_digest.as_str())
    );

    let response_bytes = fs::read(&response_path).expect("read response body");
    assert_eq!(response_bytes, br#"{"status":"ok"}"#);
}

#[test]
fn manifest_submit_rejects_chunk_digest_mismatch() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pin/register");
        then.status(200).body("{\"status\":\"ok\"}");
    });

    let wrong_digest = hex_encode([0xAB; 32]);
    let output = sorafs_cli_cmd()
        .arg("manifest")
        .arg("submit")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg(format!("--chunk-digest-sha3={wrong_digest}"))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--submitted-epoch=7")
        .arg("--authority=alice@wonderland")
        .arg("--private-key=ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C")
        .output()
        .expect("command executes");

    assert!(
        !output.status.success(),
        "CLI must fail when chunk digest mismatches manifest"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not match manifest CAR digest"),
        "stderr should mention digest mismatch, got: {stderr}"
    );
    mock.assert_calls(0);
}

#[test]
fn fetch_command_streams_payload_via_gateway() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..2048)
        .map(|idx| (idx as u8).wrapping_mul(19) ^ 0xA5)
        .collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json =
        chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json string");
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json).expect("write plan json");

    let manifest_id_hex = hex_encode([0x42u8; 32]);
    let provider_id_bytes = [0x17u8; 32];
    let provider_id_hex = hex_encode(provider_id_bytes);

    let chunk_specs = plan.chunk_fetch_specs();
    let server = MockServer::start();
    let mut mocks = Vec::with_capacity(chunk_specs.len());
    for spec in &chunk_specs {
        let digest_hex = hex_encode(spec.digest);
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let chunk_bytes = payload[start..end].to_vec();
        let manifest_for_path = manifest_id_hex.clone();
        let mock = server.mock(move |when, then| {
            when.method(GET).path(format!(
                "/v1/sorafs/storage/chunk/{}/{}",
                manifest_for_path, digest_hex
            ));
            then.status(200).body(chunk_bytes.clone());
        });
        mocks.push(mock);
    }

    let signing = SigningKey::from_bytes(&[0xAB; 32]);
    let token_body = StreamTokenBodyV1 {
        token_id: "tok-cli-integration".to_string(),
        manifest_cid: vec![0x01, 0x55, 0x01],
        provider_id: provider_id_bytes,
        profile_handle: "sorafs.sf1@1.0.0".to_string(),
        max_streams: 4,
        ttl_epoch: 1_800_000_000,
        rate_limit_bytes: 25 * 1024 * 1024,
        issued_at: 1_700_000_000,
        requests_per_minute: 120,
        token_pk_version: 1,
    };
    let stream_token = StreamTokenV1::sign(token_body, &signing).expect("sign stream token");
    let stream_token_bytes = to_bytes(&stream_token).expect("stream token bytes");
    let stream_token_b64 = BASE64_STANDARD.encode(stream_token_bytes);

    let provider_arg = format!(
        "name=alpha,provider-id={provider_id_hex},base-url={},stream-token={stream_token_b64}",
        server.base_url()
    );

    let output_path = tempdir.path().join("payload.out");
    let json_path = tempdir.path().join("fetch_summary.json");

    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg("--chunker-handle=sorafs.sf1@1.0.0")
        .arg("--telemetry-region=test-region")
        .arg("--max-peers=1")
        .arg("--retry-budget=2")
        .arg(format!("--provider={provider_arg}"))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", json_path.display()))
        .assert()
        .success();

    for mock in mocks {
        mock.assert();
    }

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let stdout_json: Value = norito::json::from_str(stdout.trim()).expect("stdout summary json");
    assert_eq!(
        stdout_json
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("stdout manifest id"),
        manifest_id_hex
    );
    assert_eq!(
        stdout_json.get("telemetry_region").and_then(Value::as_str),
        Some("test-region")
    );

    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload);

    let summary_bytes = fs::read(&json_path).expect("read fetch summary");
    let summary_json: Value = from_slice(&summary_bytes).expect("summary json");
    assert_eq!(
        summary_json
            .get("chunk_count")
            .and_then(Value::as_u64)
            .expect("chunk count"),
        chunk_specs.len() as u64
    );
    assert_eq!(
        summary_json
            .get("assembled_bytes")
            .and_then(Value::as_u64)
            .expect("assembled bytes"),
        payload.len() as u64
    );
    assert_eq!(
        summary_json.get("telemetry_region").and_then(Value::as_str),
        Some("test-region")
    );
    let reports = summary_json
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider reports");
    assert_eq!(reports.len(), 1);
    let report = reports[0].as_object().expect("report object");
    assert_eq!(
        report.get("provider").and_then(Value::as_str),
        Some("alpha")
    );
    assert_eq!(
        report.get("successes").and_then(Value::as_u64),
        Some(chunk_specs.len() as u64)
    );
    assert_eq!(
        summary_json
            .get("ineligible_providers")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default(),
        0
    );
}

#[test]
fn proof_verify_reports_chunk_digest() {
    let tempdir = tempdir().expect("tempdir");

    let input_path = tempdir.path().join("payload.bin");
    let payload: Vec<u8> = (0..2048).map(|i| (i as u8).wrapping_mul(7)).collect();
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.path().join("payload.car");
    let plan_path = tempdir.path().join("plan.json");
    let pack_summary_path = tempdir.path().join("pack_summary.json");
    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", pack_summary_path.display()))
        .assert()
        .success();

    let manifest_path = tempdir.path().join("manifest.to");
    sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", pack_summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .assert()
        .success();

    let verify_summary_path = tempdir.path().join("verify_summary.json");
    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("verify")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--summary-out={}", verify_summary_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value = norito::json::from_str(stdout.trim()).expect("verify summary json");
    let summary_file_bytes = fs::read(&verify_summary_path).expect("read verify summary file");
    let summary_file: Value =
        from_slice(&summary_file_bytes).expect("verify summary file json must parse");
    assert_eq!(
        summary_stdout, summary_file,
        "stdout summary should match file"
    );

    let expected_digest = compute_chunk_digest_hex(&plan_path);
    assert_eq!(
        summary_stdout
            .get("chunk_digest_sha3_hex")
            .and_then(Value::as_str),
        Some(expected_digest.as_str())
    );

    let plan_value: Value =
        from_slice(&fs::read(&plan_path).expect("read plan")).expect("plan json");
    let specs = chunk_fetch_specs_from_json(&plan_value).expect("plan specs");
    assert_eq!(
        summary_stdout.get("chunk_count").and_then(Value::as_u64),
        Some(specs.len() as u64)
    );

    let root_cids = summary_stdout
        .get("root_cids_hex")
        .and_then(Value::as_array)
        .expect("root cids array");
    assert!(
        !root_cids.is_empty(),
        "verify summary should report root CIDs"
    );

    assert_eq!(
        summary_stdout.get("chunker_handle").and_then(Value::as_str),
        Some("sorafs.sf1@1.0.0")
    );
    assert_eq!(
        summary_stdout
            .get("car_payload_bytes")
            .and_then(Value::as_u64),
        Some(payload.len() as u64)
    );
    assert_eq!(
        summary_stdout.get("payload_bytes").and_then(Value::as_u64),
        Some(payload.len() as u64)
    );

    let manifest_bytes = fs::read(&manifest_path).expect("read manifest");
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("decode manifest");
    let manifest_digest = manifest.digest().expect("manifest digest");
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    assert_eq!(
        summary_stdout
            .get("manifest_digest_hex")
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str())
    );
}

#[test]
fn manifest_sign_produces_bundle_and_signature() {
    let tempdir = tempdir().expect("tempdir");
    let input_path = tempdir.path().join("payload.bin");
    let payload: Vec<u8> = (0..1024).map(|i| (i as u8).wrapping_mul(9)).collect();
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.path().join("payload.car");
    let plan_path = tempdir.path().join("plan.json");
    let pack_summary_path = tempdir.path().join("pack_summary.json");

    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", pack_summary_path.display()))
        .assert()
        .success();

    let manifest_path = tempdir.path().join("manifest.to");
    sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", pack_summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .assert()
        .success();

    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let payload_claims = URL_SAFE_NO_PAD.encode(
        r#"{"iss":"https://issuer.example","sub":"unit-test","aud":"sorafs","exp":1234567890,"email":"unit@example.com"}"#,
    );
    let signature = URL_SAFE_NO_PAD.encode("sig");
    let token = format!("{header}.{payload_claims}.{signature}");

    let bundle_path = tempdir.path().join("manifest.bundle.json");
    let signature_path = tempdir.path().join("manifest.sig");
    let assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle-out={}", bundle_path.display()))
        .arg(format!("--signature-out={}", signature_path.display()))
        .arg(format!("--identity-token={}", token))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");

    let bundle_summary_path = summary
        .get("bundle_path")
        .and_then(Value::as_str)
        .expect("bundle_path");
    assert_eq!(bundle_summary_path, bundle_path.display().to_string());

    let signature_summary_path = summary
        .get("signature_path")
        .and_then(Value::as_str)
        .expect("signature_path");
    assert_eq!(signature_summary_path, signature_path.display().to_string());

    let chunk_plan_count = summary
        .get("chunk_plan_chunk_count")
        .and_then(Value::as_u64)
        .expect("chunk plan count");
    let plan_specs = {
        let plan_bytes = fs::read(&plan_path).expect("read plan");
        let plan_value: Value = from_slice(&plan_bytes).expect("plan json");
        chunk_fetch_specs_from_json(&plan_value).expect("plan specs")
    };
    assert_eq!(chunk_plan_count, plan_specs.len() as u64);

    let expected_chunk_digest = compute_chunk_digest_hex(&plan_path);
    assert_eq!(
        summary
            .get("chunk_digest_sha3_256_hex")
            .or_else(|| summary.get("chunk_digest_sha3_hex"))
            .and_then(Value::as_str),
        Some(expected_chunk_digest.as_str())
    );

    let manifest_bytes = fs::read(&manifest_path).expect("read manifest");
    let manifest: ManifestV1 =
        decode_from_bytes(&manifest_bytes).expect("decode manifest for summary");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    assert_eq!(
        summary
            .get("manifest_blake3_hex")
            .or_else(|| summary.get("manifest_digest_hex"))
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str())
    );

    let public_key_hex = summary
        .get("public_key_hex")
        .and_then(Value::as_str)
        .expect("public key hex");
    assert_eq!(
        public_key_hex.len(),
        64,
        "ed25519 public key should be 32 bytes"
    );

    let signature_hex = summary
        .get("signature_hex")
        .and_then(Value::as_str)
        .expect("signature hex");
    assert_eq!(
        signature_hex.len(),
        128,
        "ed25519 signature should be 64 bytes"
    );

    let expected_token_hash = hex_encode(blake3_hash(token.as_bytes()).as_bytes());
    assert_eq!(
        summary
            .get("identity_token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_token_hash.as_str())
    );
    assert_eq!(
        summary.get("identity_token_source").and_then(Value::as_str),
        Some("inline")
    );
    assert_eq!(
        summary.get("token_included").and_then(Value::as_bool),
        Some(false)
    );

    let signature_contents = fs::read_to_string(&signature_path).expect("read signature file");
    assert!(
        signature_contents.ends_with('\n'),
        "signature file should end with newline"
    );
    assert_eq!(
        signature_contents.trim().len(),
        128,
        "signature file should contain hex-encoded signature"
    );

    let bundle_bytes = fs::read(&bundle_path).expect("read bundle");
    let bundle: Value = from_slice(&bundle_bytes).expect("bundle json");
    assert_eq!(
        bundle.get("schema_version").and_then(Value::as_str),
        Some("sorafs-cli-manifest-sign/v1")
    );
    let inline_identity = bundle
        .get("identity")
        .and_then(Value::as_object)
        .expect("identity object");
    assert_eq!(
        inline_identity.get("token_source").and_then(Value::as_str),
        Some("inline")
    );
    assert_eq!(
        inline_identity
            .get("token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_token_hash.as_str())
    );
    assert_eq!(
        inline_identity
            .get("token_included")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        inline_identity.get("token").is_none(),
        "inline signing should not persist raw tokens"
    );
    let claims = inline_identity
        .get("jwt_claims")
        .and_then(Value::as_object)
        .expect("claims object");
    assert_eq!(claims.get("sub").and_then(Value::as_str), Some("unit-test"));
    assert_eq!(claims.get("aud").and_then(Value::as_str), Some("sorafs"));

    let signature_info = bundle
        .get("signature")
        .and_then(Value::as_object)
        .expect("signature object");
    assert_eq!(
        signature_info.get("public_key_hex").and_then(Value::as_str),
        Some(public_key_hex)
    );
    assert_eq!(
        signature_info.get("signature_hex").and_then(Value::as_str),
        Some(signature_hex)
    );

    let manifest_info = bundle
        .get("manifest")
        .and_then(Value::as_object)
        .expect("manifest info");
    assert_eq!(
        manifest_info
            .get("manifest_blake3_hex")
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str())
    );
    assert_eq!(
        manifest_info
            .get("chunk_digest_sha3_256_hex")
            .and_then(Value::as_str),
        Some(expected_chunk_digest.as_str())
    );

    // Repeat with env-sourced token and include-token flag
    let env_bundle_path = tempdir.path().join("manifest.env.bundle.json");
    let env_signature_path = tempdir.path().join("manifest.env.sig");
    let env_var = "TEST_SORAFS_OIDC_TOKEN";

    let assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .env(env_var, token.clone())
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle-out={}", env_bundle_path.display()))
        .arg(format!("--signature-out={}", env_signature_path.display()))
        .arg(format!("--identity-token-env={env_var}"))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg("--include-token=true")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let env_summary: Value = norito::json::from_str(stdout.trim()).expect("env summary json");
    let expected_env_source = format!("env:{env_var}");
    assert_eq!(
        env_summary
            .get("identity_token_source")
            .and_then(Value::as_str),
        Some(expected_env_source.as_str())
    );
    assert_eq!(
        env_summary.get("token_included").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        env_summary
            .get("identity_token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_token_hash.as_str())
    );

    let env_bundle_bytes = fs::read(&env_bundle_path).expect("read env bundle");
    let env_bundle: Value = from_slice(&env_bundle_bytes).expect("env bundle json");
    let env_identity = env_bundle
        .get("identity")
        .and_then(Value::as_object)
        .expect("identity object");
    assert_eq!(
        env_identity.get("token_included").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        env_identity.get("token").and_then(Value::as_str),
        Some(token.as_str())
    );
    assert_eq!(
        env_identity.get("token_source").and_then(Value::as_str),
        Some(expected_env_source.as_str())
    );
}

#[test]
fn manifest_sign_defaults_to_sigstore_id_token_env() {
    let tempdir = tempdir().expect("tempdir");
    let input_path = tempdir.path().join("payload.bin");
    let payload: Vec<u8> = (0..1536).map(|i| (i as u8).wrapping_mul(5)).collect();
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.path().join("payload.car");
    let plan_path = tempdir.path().join("plan.json");
    let summary_path = tempdir.path().join("summary.json");

    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    let manifest_path = tempdir.path().join("manifest.to");
    sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .assert()
        .success();

    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD.encode(
        r#"{"iss":"https://issuer.example","sub":"sigstore-test","aud":"sigstore","exp":1234567890,"email":"unit@example.com"}"#,
    );
    let signature = URL_SAFE_NO_PAD.encode("sig");
    let token = format!("{header}.{claims}.{signature}");

    let bundle_path = tempdir.path().join("sigstore.bundle.json");
    let signature_path = tempdir.path().join("sigstore.sig");
    let assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .env("SIGSTORE_ID_TOKEN", token.clone())
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle-out={}", bundle_path.display()))
        .arg(format!("--signature-out={}", signature_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    assert_eq!(
        summary.get("identity_token_source").and_then(Value::as_str),
        Some("env:SIGSTORE_ID_TOKEN")
    );
    assert_eq!(
        summary.get("token_included").and_then(Value::as_bool),
        Some(false)
    );
    let expected_hash = hex_encode(blake3_hash(token.as_bytes()).as_bytes());
    assert_eq!(
        summary
            .get("identity_token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_hash.as_str())
    );

    let bundle_bytes = fs::read(&bundle_path).expect("read bundle");
    let bundle: Value = from_slice(&bundle_bytes).expect("bundle json");
    let identity = bundle
        .get("identity")
        .and_then(Value::as_object)
        .expect("identity object");
    assert_eq!(
        identity.get("token_source").and_then(Value::as_str),
        Some("env:SIGSTORE_ID_TOKEN")
    );
    assert_eq!(
        identity
            .get("token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_hash.as_str())
    );
    assert_eq!(
        identity.get("token_included").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn manifest_sign_fetches_github_actions_oidc_token() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());

    let server = MockServer::start();
    let gha_request_token = "gha-request-token";
    let expected_audience = "sigstore-ci";
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD.encode(
        r#"{"iss":"https://gha.example","sub":"repo","aud":"sigstore-ci","exp":1735689600}"#,
    );
    let signature = URL_SAFE_NO_PAD.encode("signature");
    let expected_token = format!("{header}.{claims}.{signature}");
    let expected_body = format!(r#"{{"value":"{}"}}"#, expected_token);
    let auth_header = format!("Bearer {gha_request_token}");

    let mock = server.mock(move |when, then| {
        when.method(GET)
            .path("/oidc/token")
            .header("authorization", auth_header.clone())
            .query_param("audience", expected_audience);
        then.status(200)
            .header("content-type", "application/json")
            .body(expected_body.clone());
    });

    let bundle_path = tempdir.path().join("gha.bundle.json");
    let signature_path = tempdir.path().join("gha.sig");
    let assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .env_remove("SIGSTORE_ID_TOKEN")
        .env("ACTIONS_ID_TOKEN_REQUEST_URL", server.url("/oidc/token"))
        .env("ACTIONS_ID_TOKEN_REQUEST_TOKEN", gha_request_token)
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle-out={}", bundle_path.display()))
        .arg(format!("--signature-out={}", signature_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg("--identity-token-provider=github-actions")
        .arg(format!("--identity-token-audience={expected_audience}"))
        .assert()
        .success();

    mock.assert_calls(1);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    let expected_source = format!("oidc:github-actions({expected_audience})");
    assert_eq!(
        summary.get("identity_token_source").and_then(Value::as_str),
        Some(expected_source.as_str())
    );
    assert_eq!(
        summary.get("token_included").and_then(Value::as_bool),
        Some(false)
    );
    let expected_hash = hex_encode(blake3_hash(expected_token.as_bytes()).as_bytes());
    assert_eq!(
        summary
            .get("identity_token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_hash.as_str())
    );

    let bundle_bytes = fs::read(&bundle_path).expect("read bundle");
    let bundle: Value = from_slice(&bundle_bytes).expect("bundle json");
    let identity = bundle
        .get("identity")
        .and_then(Value::as_object)
        .expect("identity object");
    assert_eq!(
        identity.get("token_source").and_then(Value::as_str),
        Some(expected_source.as_str())
    );
    assert_eq!(
        identity.get("token_included").and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        identity.get("token").is_none(),
        "token should not be persisted in bundles by default"
    );
}

#[test]
fn manifest_sign_fetches_github_actions_oidc_token_with_explicit_audience() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());

    let server = MockServer::start();
    let gha_request_token = "gha-default-token";
    let expected_audience = "sigstore";
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD
        .encode(r#"{"iss":"https://gha.example","sub":"repo","aud":"sigstore","exp":1735689600}"#);
    let signature = URL_SAFE_NO_PAD.encode("signature");
    let expected_token = format!("{header}.{claims}.{signature}");
    let expected_body = format!(r#"{{"value":"{}"}}"#, expected_token);
    let auth_header = format!("Bearer {gha_request_token}");

    let mock = server.mock(move |when, then| {
        when.method(GET)
            .path("/oidc/token")
            .header("authorization", auth_header.clone())
            .query_param("audience", expected_audience);
        then.status(200)
            .header("content-type", "application/json")
            .body(expected_body.clone());
    });
    let bundle_path = tempdir.path().join("gha-default.bundle.json");
    let signature_path = tempdir.path().join("gha-default.sig");
    let assert = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .env_remove("SIGSTORE_ID_TOKEN")
        .env("ACTIONS_ID_TOKEN_REQUEST_URL", server.url("/oidc/token"))
        .env("ACTIONS_ID_TOKEN_REQUEST_TOKEN", gha_request_token)
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle-out={}", bundle_path.display()))
        .arg(format!("--signature-out={}", signature_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg("--identity-token-provider=github-actions")
        .arg(format!("--identity-token-audience={expected_audience}"))
        .assert()
        .success();

    mock.assert();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    assert_eq!(
        summary.get("identity_token_source").and_then(Value::as_str),
        Some("oidc:github-actions(sigstore)")
    );
    assert_eq!(
        summary.get("token_included").and_then(Value::as_bool),
        Some(false)
    );
    let expected_hash = hex_encode(blake3_hash(expected_token.as_bytes()).as_bytes());
    assert_eq!(
        summary
            .get("identity_token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_hash.as_str())
    );

    let bundle_bytes = fs::read(&bundle_path).expect("read bundle");
    let bundle: Value = from_slice(&bundle_bytes).expect("bundle json");
    let identity = bundle
        .get("identity")
        .and_then(Value::as_object)
        .expect("identity object");
    assert_eq!(
        identity.get("token_source").and_then(Value::as_str),
        Some("oidc:github-actions(sigstore)")
    );
    assert_eq!(
        identity
            .get("token_hash_blake3_hex")
            .and_then(Value::as_str),
        Some(expected_hash.as_str())
    );
}

#[test]
fn manifest_sign_missing_audience_fails() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());

    let server = MockServer::start();
    let gha_request_token = "gha-default-token";

    let _mock = server.mock(|when, then| {
        when.method(GET).path("/oidc/token");
        then.status(200)
            .header("content-type", "application/json")
            .body("{\"value\":\"token\"}");
    });

    let output = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .env_remove("SIGSTORE_ID_TOKEN")
        .env("ACTIONS_ID_TOKEN_REQUEST_URL", server.url("/oidc/token"))
        .env("ACTIONS_ID_TOKEN_REQUEST_TOKEN", gha_request_token)
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!(
            "--bundle-out={}",
            tempdir.path().join("bundle.json").display()
        ))
        .arg(format!(
            "--signature-out={}",
            tempdir.path().join("bundle.sig").display()
        ))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg("--identity-token-provider=github-actions")
        .output()
        .expect("command executes");

    assert!(
        !output.status.success(),
        "command must fail when audience is omitted"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires --identity-token-audience"),
        "stderr should mention missing audience, got: {stderr}"
    );
}

#[test]
fn manifest_sign_rejects_provider_with_explicit_token() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());

    let bundle_path = tempdir.path().join("reject.bundle.json");
    let signature_path = tempdir.path().join("reject.sig");
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256"}"#);
    let claims = URL_SAFE_NO_PAD.encode(r#"{"iss":"https://issuer","aud":"sigstore"}"#);
    let signature = URL_SAFE_NO_PAD.encode("sig");
    let token = format!("{header}.{claims}.{signature}");

    let output = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--bundle-out={}", bundle_path.display()))
        .arg(format!("--signature-out={}", signature_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg(format!("--identity-token={token}"))
        .arg("--identity-token-provider=github-actions")
        .output()
        .expect("command executes");

    assert!(
        !output.status.success(),
        "command should fail when provider and explicit token are combined"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("identity token provider flags cannot be combined"),
        "stderr should mention provider/token exclusivity"
    );
}

#[test]
fn proof_stream_consumes_ndjson_and_reports_metrics() {
    use httpmock::Method::POST;

    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, _) = prepare_manifest_artifacts(tempdir.path());

    let manifest_bytes = fs::read(&manifest_path).expect("read manifest");
    let manifest: ManifestV1 =
        decode_from_bytes(&manifest_bytes).expect("decode manifest for digest");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());

    let success_item = norito::json!({
        "manifest_digest_hex": manifest_digest_hex,
        "proof_kind": "por",
        "verification_status": "success",
        "latency_ms": 42,
        "sample_index": 0,
        "chunk_index": 0
    });
    let failure_item = norito::json!({
        "manifest_digest_hex": manifest_digest_hex,
        "proof_kind": "por",
        "verification_status": "failure",
        "latency_ms": 75,
        "sample_index": 1,
        "chunk_index": 1,
        "failure_reason": "invalid_proof"
    });
    let success_line = String::from_utf8(to_vec(&success_item).expect("success json encode"))
        .expect("success json utf8");
    let failure_line = String::from_utf8(to_vec(&failure_item).expect("failure json encode"))
        .expect("failure json utf8");
    let response_body = format!("{success_line}\n{failure_line}\n");

    let server = MockServer::start();
    let mock = server.mock(move |when, then| {
        when.method(POST)
            .path("/v1/proof/stream")
            .header("content-type", "application/json")
            .header("accept", "application/x-ndjson");
        then.status(200)
            .header("content-type", "application/x-ndjson")
            .body(response_body.clone());
    });

    let summary_path = tempdir.path().join("stream_summary.json");
    sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--gateway-url={}", server.url("/v1/proof/stream")))
        .arg("--provider-id=provider-1")
        .arg("--samples=2")
        .arg("--emit-events=false")
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    mock.assert();

    let summary_bytes = fs::read(&summary_path).expect("read summary json");
    let summary: Value = from_slice(&summary_bytes).expect("summary json should parse");
    let metrics = summary
        .get("metrics")
        .and_then(Value::as_object)
        .expect("metrics object");
    assert_eq!(metrics.get("item_total").and_then(Value::as_u64), Some(2));
    assert_eq!(
        metrics.get("success_total").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        metrics.get("failure_total").and_then(Value::as_u64),
        Some(1)
    );
    let reason_counts = metrics
        .get("failure_by_reason")
        .and_then(Value::as_object)
        .expect("failure map");
    assert_eq!(
        reason_counts.get("invalid_proof").and_then(Value::as_u64),
        Some(1)
    );

    let samples = summary
        .get("failure_samples")
        .and_then(Value::as_array)
        .expect("failure samples array");
    assert_eq!(samples.len(), 1);
    assert_eq!(
        samples[0].get("failure_reason").and_then(Value::as_str),
        Some("invalid_proof")
    );
}

#[test]
fn manifest_sign_rejects_audience_without_provider() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());

    let output = sorafs_cli_cmd()
        .arg("manifest")
        .arg("sign")
        .env("SIGSTORE_ID_TOKEN", "stub.token.value")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!(
            "--bundle-out={}",
            tempdir.path().join("bundle.json").display()
        ))
        .arg(format!(
            "--signature-out={}",
            tempdir.path().join("signature.txt").display()
        ))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg("--identity-token-audience=custom")
        .output()
        .expect("command executes");

    assert!(
        !output.status.success(),
        "command should fail when audience is supplied without a provider"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires `--identity-token-provider`"),
        "stderr should mention provider requirement"
    );
}

#[test]
fn proof_stream_consumes_ndjson_and_summarises_output() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, _plan_path) = prepare_manifest_artifacts(tempdir.path());

    let ndjson = r#"{"verification_status":"success","latency_ms":42}
{"verification_status":"failure","failure_reason":"invalid_proof","latency_ms":105}
"#;

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/proof/stream");
        then.status(200)
            .header("content-type", "application/x-ndjson")
            .body(ndjson);
    });

    let endpoint = server.url("/proof/stream");
    let summary_path = tempdir.path().join("proof_summary.json");
    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--endpoint={endpoint}"))
        .arg("--provider-id=provider-1")
        .arg("--summary-out=".to_string() + summary_path.to_str().expect("utf8 path"))
        .assert()
        .success();

    mock.assert();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    assert_eq!(
        summary.get("proof_kind").and_then(Value::as_str),
        Some("por")
    );
    assert_eq!(
        summary.get("provider_id").and_then(Value::as_str),
        Some("provider-1")
    );
    assert_eq!(summary.get("total_items").and_then(Value::as_u64), Some(2));
    assert_eq!(
        summary.get("success_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        summary.get("failure_count").and_then(Value::as_u64),
        Some(1)
    );
    let failure_breakdown = summary
        .get("failure_breakdown")
        .and_then(Value::as_object)
        .expect("failure breakdown object");
    assert_eq!(
        failure_breakdown
            .get("invalid_proof")
            .and_then(Value::as_u64),
        Some(1)
    );
    if let Some(avg) = summary.get("average_latency_ms").and_then(Value::as_f64) {
        assert!(
            (avg - 73.5).abs() < f64::EPSILON,
            "average latency mismatch"
        );
    } else {
        panic!("missing average latency");
    }

    let summary_disk = fs::read_to_string(&summary_path).expect("summary file");
    let summary_disk_json: Value =
        norito::json::from_str(summary_disk.trim()).expect("summary file json");
    assert_eq!(
        summary_disk_json.get("total_items").and_then(Value::as_u64),
        Some(2)
    );
}

#[test]
fn ci_sample_fixtures_are_consistent() {
    let base = Path::new("fixtures/sorafs_manifest/ci_sample");
    assert!(
        base.is_dir(),
        "expected fixture directory `{}` to exist",
        base.display()
    );

    let payload_path = base.join("payload.txt");
    let payload = fs::read(&payload_path).expect("read payload fixture");
    assert_eq!(
        payload.len(),
        201,
        "payload fixture should contain deterministic length"
    );
    let payload_digest = blake3_hash(&payload);
    let payload_digest_hex = hex_encode(payload_digest.as_bytes());

    let chunk_plan_path = base.join("chunk_plan.json");
    let chunk_plan_bytes = fs::read(&chunk_plan_path).expect("read chunk plan");
    let chunk_plan_value: Value =
        from_slice(&chunk_plan_bytes).expect("chunk plan json should parse");
    let chunk_array = chunk_plan_value
        .as_array()
        .expect("chunk plan should be an array");
    assert_eq!(
        chunk_array.len(),
        1,
        "ci sample currently targets a single chunk plan"
    );
    let chunk_entry = &chunk_array[0];
    let chunk_offset = chunk_entry
        .get("offset")
        .and_then(Value::as_u64)
        .expect("chunk offset present");
    let chunk_length = chunk_entry
        .get("length")
        .and_then(Value::as_u64)
        .expect("chunk length present");
    assert_eq!(chunk_offset, 0, "chunk should start at offset zero");
    assert_eq!(
        chunk_length,
        payload.len() as u64,
        "chunk length should cover entire payload"
    );
    let chunk_digest_hex = chunk_entry
        .get("digest_blake3")
        .and_then(Value::as_str)
        .expect("chunk digest hex present");
    let chunk_digest_bytes =
        hex_decode(chunk_digest_hex).expect("chunk digest hex should decode to bytes");
    assert_eq!(
        chunk_digest_bytes.len(),
        32,
        "chunk digest must be 32-byte BLAKE3 hash"
    );
    let computed_chunk_digest = blake3_hash(&payload);
    assert_eq!(
        chunk_digest_hex,
        hex_encode(computed_chunk_digest.as_bytes()),
        "chunk digest should match payload hash"
    );

    let mut chunk_digest_sha3 = Sha3_256::new();
    chunk_digest_sha3.update(chunk_offset.to_le_bytes());
    chunk_digest_sha3.update(chunk_length.to_le_bytes());
    chunk_digest_sha3.update(&chunk_digest_bytes);
    let chunk_digest_sha3_hex = hex_encode(chunk_digest_sha3.finalize());

    // Ensure the chunk plan parses through the helper used by the CLI.
    chunk_fetch_specs_from_json(&chunk_plan_value)
        .expect("chunk plan should parse into fetch specs");

    let car_path = base.join("payload.car");
    let car_bytes = fs::read(&car_path).expect("read CAR archive");
    let car_digest_hex = hex_encode(blake3_hash(&car_bytes).as_bytes());

    let car_summary_bytes = fs::read(base.join("car_summary.json")).expect("read car summary");
    let car_summary: Value = from_slice(&car_summary_bytes).expect("car summary json should parse");
    assert_eq!(
        car_summary
            .get("car_digest_hex")
            .and_then(Value::as_str)
            .expect("car digest present"),
        car_digest_hex,
        "car digest must match CAR archive hash"
    );
    assert_eq!(
        car_summary
            .get("car_payload_digest_hex")
            .and_then(Value::as_str)
            .expect("car payload digest present"),
        payload_digest_hex,
        "payload digest must match summary"
    );
    assert_eq!(
        car_summary
            .get("chunk_count")
            .and_then(Value::as_u64)
            .expect("chunk count present"),
        chunk_array.len() as u64,
        "chunk count should match plan entries"
    );
    assert_eq!(
        car_summary
            .get("input_path")
            .and_then(Value::as_str)
            .expect("input path present"),
        payload_path.display().to_string(),
        "input path should be recorded using workspace-relative path"
    );

    let manifest_path = base.join("manifest.to");
    let manifest_bytes = fs::read(&manifest_path).expect("read manifest bytes");
    let manifest: ManifestV1 =
        decode_from_bytes(&manifest_bytes).expect("manifest should decode via Norito");
    assert_eq!(
        manifest.content_length,
        payload.len() as u64,
        "manifest content length must match payload"
    );
    assert_eq!(
        hex_encode(manifest.car_digest),
        car_digest_hex,
        "manifest car digest should match CAR hash"
    );
    assert_eq!(
        manifest.pin_policy.min_replicas, 1,
        "sample manifest should request a single replica"
    );
    assert_eq!(
        manifest.pin_policy.storage_class,
        StorageClass::Hot,
        "storage class must match documented default"
    );
    assert_eq!(
        manifest.pin_policy.retention_epoch, 0,
        "retention epoch should be set to immediate availability"
    );
    let manifest_digest = manifest
        .digest()
        .expect("manifest digest computation should succeed");
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());

    let manifest_json_bytes = fs::read(base.join("manifest.json")).expect("read manifest json");
    let manifest_json: Value =
        from_slice(&manifest_json_bytes).expect("manifest json should parse");
    assert_eq!(
        manifest_json
            .get("pin_policy")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("storage_class"))
            .and_then(Value::as_str),
        Some("hot"),
        "manifest.json pin policy should mirror Norito manifest"
    );

    let bundle_bytes = fs::read(base.join("manifest.bundle.json")).expect("read bundle");
    let bundle_value: Value = from_slice(&bundle_bytes).expect("manifest bundle json should parse");
    let bundle_manifest = bundle_value
        .get("manifest")
        .and_then(Value::as_object)
        .expect("bundle manifest object");
    assert_eq!(
        bundle_manifest
            .get("manifest_blake3_hex")
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str()),
        "bundle should embed manifest digest"
    );
    assert_eq!(
        bundle_manifest
            .get("chunk_digest_sha3_256_hex")
            .and_then(Value::as_str),
        Some(chunk_digest_sha3_hex.as_str()),
        "bundle chunk digest should match computed SHA3 digest"
    );
    assert_eq!(
        bundle_manifest
            .get("chunk_plan_source")
            .and_then(Value::as_str)
            .expect("bundle chunk plan path"),
        chunk_plan_path.display().to_string(),
        "bundle should reference chunk plan path"
    );

    let bundle_signature = bundle_value
        .get("signature")
        .and_then(Value::as_object)
        .expect("bundle signature object");
    let signature_hex = bundle_signature
        .get("signature_hex")
        .and_then(Value::as_str)
        .expect("signature hex present");
    let signature_bytes_vec =
        hex_decode(signature_hex).expect("signature hex should decode into bytes");
    let signature_bytes: [u8; 64] = signature_bytes_vec
        .as_slice()
        .try_into()
        .expect("signature must be 64 bytes");
    let signature = Signature::from_bytes(&signature_bytes);

    let public_key_hex = bundle_signature
        .get("public_key_hex")
        .and_then(Value::as_str)
        .expect("public key hex present");
    let public_key_bytes_vec =
        hex_decode(public_key_hex).expect("public key hex should decode into bytes");
    let public_key_bytes: [u8; 32] = public_key_bytes_vec
        .as_slice()
        .try_into()
        .expect("public key must be 32 bytes");
    let verifying_key =
        VerifyingKey::from_bytes(&public_key_bytes).expect("verifying key should parse");
    verifying_key
        .verify(manifest_digest.as_bytes(), &signature)
        .expect("fixture signature should verify against manifest digest");

    let signature_file =
        fs::read_to_string(base.join("manifest.sig")).expect("read detached signature");
    assert_eq!(
        signature_file.trim(),
        signature_hex,
        "detached signature must match bundle signature"
    );

    let sign_summary_bytes =
        fs::read(base.join("manifest.sign.summary.json")).expect("read sign summary");
    let sign_summary: Value =
        from_slice(&sign_summary_bytes).expect("sign summary json should parse");
    assert_eq!(
        sign_summary
            .get("chunk_digest_sha3_256_hex")
            .and_then(Value::as_str),
        Some(chunk_digest_sha3_hex.as_str()),
        "sign summary should carry chunk digest"
    );
    assert_eq!(
        sign_summary
            .get("identity_token_hash_blake3_hex")
            .and_then(Value::as_str),
        bundle_value
            .get("identity")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("token_hash_blake3_hex"))
            .and_then(Value::as_str),
        "sign summary and bundle should agree on token hash"
    );
    assert_eq!(
        sign_summary
            .get("manifest_blake3_hex")
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str()),
        "sign summary should echo manifest digest"
    );
    assert_eq!(
        sign_summary
            .get("bundle_path")
            .and_then(Value::as_str)
            .expect("bundle path present"),
        base.join("manifest.bundle.json").display().to_string(),
        "sign summary should point to bundle path"
    );

    let verify_summary_bytes =
        fs::read(base.join("manifest.verify.summary.json")).expect("read verify summary");
    let verify_summary: Value =
        from_slice(&verify_summary_bytes).expect("verify summary json should parse");
    assert_eq!(
        verify_summary
            .get("verification_status")
            .and_then(Value::as_str),
        Some("ok"),
        "verify summary should report success"
    );
    assert_eq!(
        verify_summary
            .get("manifest_blake3_hex")
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str()),
        "verify summary should echo manifest digest"
    );
    assert_eq!(
        verify_summary
            .get("chunk_digest_sha3_256_hex")
            .and_then(Value::as_str),
        Some(chunk_digest_sha3_hex.as_str()),
        "verify summary should report chunk digest"
    );
    assert_eq!(
        verify_summary
            .get("bundle_path")
            .and_then(Value::as_str)
            .expect("verify bundle path present"),
        base.join("manifest.bundle.json").display().to_string(),
        "verify summary should point to bundle"
    );

    let proof_bytes = fs::read(base.join("proof.json")).expect("read proof summary");
    let proof_value: Value = from_slice(&proof_bytes).expect("proof summary json should parse");
    assert_eq!(
        proof_value
            .get("manifest_digest_hex")
            .and_then(Value::as_str),
        Some(manifest_digest_hex.as_str()),
        "proof summary should reference manifest digest"
    );
    assert_eq!(
        proof_value
            .get("chunk_digest_sha3_hex")
            .and_then(Value::as_str),
        Some(chunk_digest_sha3_hex.as_str()),
        "proof summary should include chunk digest"
    );
    assert_eq!(
        proof_value
            .get("payload_digest_hex")
            .and_then(Value::as_str),
        Some(chunk_digest_hex),
        "proof summary should embed chunk BLAKE3 digest"
    );
    assert_eq!(
        proof_value.get("car_digest_hex").and_then(Value::as_str),
        Some(car_digest_hex.as_str()),
        "proof summary should embed CAR digest"
    );
}

fn prepare_manifest_artifacts(tempdir: &Path) -> (PathBuf, PathBuf) {
    let input_path = tempdir.join("gha_payload.bin");
    let payload: Vec<u8> = (0..1024).map(|i| (i as u8).wrapping_mul(29)).collect();
    fs::write(&input_path, &payload).expect("write payload");

    let car_path = tempdir.join("gha_payload.car");
    let plan_path = tempdir.join("gha_plan.json");
    let summary_path = tempdir.join("gha_summary.json");

    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();

    let manifest_path = tempdir.join("gha_manifest.to");
    sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .assert()
        .success();

    (manifest_path, plan_path)
}

fn compute_chunk_digest_hex(plan_path: &Path) -> String {
    let plan_bytes = fs::read(plan_path).expect("read plan");
    let value: Value = from_slice(&plan_bytes).expect("plan json");
    let mut specs = chunk_fetch_specs_from_json(&value).expect("plan specs");
    specs.sort_by_key(|spec| spec.chunk_index);
    let mut hasher = Sha3_256::new();
    for spec in specs {
        hasher.update(spec.offset.to_le_bytes());
        hasher.update(u64::from(spec.length).to_le_bytes());
        hasher.update(spec.digest);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    hex_encode(digest)
}

fn write_proof_stream_manifest(dir: &Path, file_name: &str) -> PathBuf {
    let payload = b"proof-stream-fixture";
    let digest = blake3_hash(payload);
    let manifest = ManifestBuilder::new()
        .root_cid(digest.as_bytes().to_vec())
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(
            sorafs_chunker::ChunkProfile::DEFAULT,
            BLAKE3_256_MULTIHASH_CODE,
        )
        .content_length(payload.len() as u64)
        .car_digest(digest.into())
        .car_size(payload.len() as u64)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest build");
    let path = dir.join(file_name);
    let bytes = to_bytes(&manifest).expect("encode manifest");
    fs::write(&path, bytes).expect("write manifest");
    path
}

fn manifest_digest_hex(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let manifest: ManifestV1 = decode_from_bytes(&bytes)?;
    let digest = manifest
        .digest()
        .map_err(|err| format!("manifest digest error: {err}"))?;
    Ok(hex_encode(digest.as_bytes()))
}

#[test]
fn proof_stream_pdp_requests_use_samples() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path = write_proof_stream_manifest(tempdir.path(), "stream_pdp_manifest.to");
    let provider_id_hex = hex_encode([0x11u8; 32]);

    let server = MockServer::start();
    let expected_samples: u32 = 16;
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/proof/stream")
            .header("Content-Type", "application/json");
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body("{\"proof_kind\":\"pdp\",\"result\":\"success\"}\n");
    });

    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=pdp")
        .arg(format!("--samples={expected_samples}"))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())?;
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    let metrics = summary
        .get("metrics")
        .and_then(Value::as_object)
        .expect("metrics object");
    assert_eq!(
        metrics.get("success_total").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        metrics.get("failure_total").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        summary
            .get("requested_sample_count")
            .and_then(Value::as_u64),
        Some(expected_samples as u64)
    );
    assert!(
        summary
            .get("requested_deadline_ms")
            .and_then(Value::as_u64)
            .is_none()
    );
    mock.assert();
    Ok(())
}

#[test]
fn proof_stream_potr_requests_require_deadline() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path = write_proof_stream_manifest(tempdir.path(), "stream_potr_manifest.to");
    let provider_id_hex = hex_encode([0x22u8; 32]);
    let deadline_ms: u32 = 45_000;

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/proof/stream")
            .header("Content-Type", "application/json")
            .body_includes("\"proof_kind\":\"potr\"")
            .body_includes(format!("\"deadline_ms\":{}", deadline_ms));
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body("{\"proof_kind\":\"potr\",\"result\":\"success\",\"latency_ms\":120}\n");
    });

    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=potr")
        .arg(format!("--deadline-ms={deadline_ms}"))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())?;
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    let metrics = summary
        .get("metrics")
        .and_then(Value::as_object)
        .expect("metrics object");
    assert_eq!(
        metrics.get("success_total").and_then(Value::as_u64),
        Some(1)
    );
    let latency = metrics
        .get("latency_ms")
        .and_then(Value::as_object)
        .expect("latency object");
    assert_eq!(latency.get("min_ms").and_then(Value::as_u64), Some(120));
    assert_eq!(latency.get("max_ms").and_then(Value::as_u64), Some(120));
    assert!(
        summary
            .get("requested_sample_count")
            .and_then(Value::as_u64)
            .is_none()
    );
    assert_eq!(
        summary.get("requested_deadline_ms").and_then(Value::as_u64),
        Some(deadline_ms as u64)
    );
    mock.assert();
    Ok(())
}

#[test]
fn proof_stream_fails_when_gateway_reports_failure() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path = write_proof_stream_manifest(tempdir.path(), "stream_failure_manifest.to");
    let provider_id_hex = hex_encode([0x33u8; 32]);

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/proof/stream")
            .header("Content-Type", "application/json");
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body(
                "{\"proof_kind\":\"pdp\",\"result\":\"failure\",\"failure_reason\":\"timeout\"}\n",
            );
    });

    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=pdp")
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone())?;
    assert!(
        stderr.contains("gateway failures"),
        "stderr should mention gateway failure budget: {stderr}"
    );
    mock.assert();
    Ok(())
}

#[test]
fn proof_stream_respects_max_failures_override() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path = write_proof_stream_manifest(tempdir.path(), "stream_failure_budget.to");
    let provider_id_hex = hex_encode([0x44u8; 32]);

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/proof/stream")
            .header("Content-Type", "application/json");
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body(
                "{\"proof_kind\":\"pdp\",\"result\":\"failure\",\"failure_reason\":\"timeout\"}\n",
            );
    });

    sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=pdp")
        .arg("--max-failures=1")
        .assert()
        .success();
    mock.assert();
    Ok(())
}

#[test]
fn proof_stream_verification_failures_trigger_exit() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path =
        write_proof_stream_manifest(tempdir.path(), "stream_verification_manifest.to");
    let provider_id_hex = hex_encode([0x55u8; 32]);
    let root_hex = hex_encode([0xAAu8; 32]);

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/proof/stream")
            .header("Content-Type", "application/json");
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body("{\"proof_kind\":\"por\",\"result\":\"success\",\"latency_ms\":42}\n");
    });

    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg(format!("--por-root-hex={root_hex}"))
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone())?;
    assert!(
        stderr.contains("local verification failures"),
        "stderr should mention verification failures: {stderr}"
    );
    mock.assert();
    Ok(())
}

#[test]
fn proof_stream_verification_budget_allows_overrides() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path =
        write_proof_stream_manifest(tempdir.path(), "stream_verification_budget.to");
    let provider_id_hex = hex_encode([0x66u8; 32]);
    let root_hex = hex_encode([0xBBu8; 32]);

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/proof/stream")
            .header("Content-Type", "application/json");
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body("{\"proof_kind\":\"por\",\"result\":\"success\",\"latency_ms\":37}\n");
    });

    sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg(format!("--por-root-hex={root_hex}"))
        .arg("--max-verification-failures=1")
        .assert()
        .success();
    mock.assert();
    Ok(())
}

#[test]
fn proof_stream_potr_stream_summary_includes_failure_reason()
-> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path =
        write_proof_stream_manifest(tempdir.path(), "stream_potr_summary_manifest.to");
    let provider_id_hex = hex_encode([0x22u8; 32]);
    let manifest_hex = manifest_digest_hex(&manifest_path)?;

    let success_line = format!(
        "{{\"manifest_digest_hex\":\"{manifest_hex}\",\"provider_id_hex\":\"{provider_id_hex}\",\"proof_kind\":\"potr\",\"result\":\"success\",\"latency_ms\":45000,\"deadline_ms\":90000,\"tier\":\"hot\",\"recorded_at_ms\":1700000000000,\"trace_id\":\"{trace_hex}\"}}",
        trace_hex = hex_encode([0x44u8; 16])
    );
    let failure_line = format!(
        "{{\"manifest_digest_hex\":\"{manifest_hex}\",\"provider_id_hex\":\"{provider_id_hex}\",\"proof_kind\":\"potr\",\"result\":\"failure\",\"failure_reason\":\"missed_deadline\",\"latency_ms\":120000,\"deadline_ms\":90000,\"tier\":\"hot\",\"recorded_at_ms\":1700000500000}}"
    );

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/proof/stream")
            .header("Content-Type", "application/json")
            .body_includes("\"proof_kind\":\"potr\"");
        then.status(200)
            .header("Content-Type", "application/x-ndjson")
            .body(format!("{success_line}\n{failure_line}\n"));
    });

    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=potr")
        .arg("--deadline-ms=90000")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())?;
    let summary: Value = norito::json::from_str(stdout.trim()).expect("summary json");
    let metrics = summary
        .get("metrics")
        .and_then(Value::as_object)
        .expect("metrics object");
    assert_eq!(
        metrics.get("success_total").and_then(Value::as_u64),
        Some(1),
        "success count"
    );
    assert_eq!(
        metrics.get("failure_total").and_then(Value::as_u64),
        Some(1),
        "failure count"
    );
    let reasons = metrics
        .get("failure_by_reason")
        .and_then(Value::as_object)
        .expect("failure reason map");
    assert_eq!(
        reasons.get("missed_deadline").and_then(Value::as_u64),
        Some(1),
        "missed deadline tallied"
    );
    assert_eq!(
        summary.get("requested_deadline_ms").and_then(Value::as_u64),
        Some(90_000),
        "requested deadline recorded"
    );
    assert!(
        summary
            .get("failure_samples")
            .and_then(Value::as_array)
            .is_some(),
        "failure samples captured"
    );
    mock.assert();
    Ok(())
}

#[test]
fn proof_stream_potr_without_deadline_errors() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path =
        write_proof_stream_manifest(tempdir.path(), "stream_potr_missing_deadline.to");
    let provider_id_hex = hex_encode([0x33u8; 32]);

    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--torii-url=http://example.com/")
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=potr")
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone())?;
    assert!(
        stderr.contains("`--deadline-ms` is required"),
        "stderr should mention missing deadline, got: {stderr}"
    );
    Ok(())
}

#[test]
fn fetch_command_streams_gateway_payload() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..4096).map(|idx| (idx % 251) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json =
        chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan");

    let server = MockServer::start();
    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let payload_digest_hex = hex_encode(plan.payload_digest.as_bytes());
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        manifest_id_hex,
        BASE64_STANDARD.encode(&manifest_bytes),
        manifest_digest_hex,
        payload_digest_hex,
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );

    let manifest_report_path = tempdir.path().join("gateway_manifest_report.json");
    fs::write(
        &manifest_report_path,
        format!("{}\n", manifest_response).as_bytes(),
    )
    .expect("write manifest report");

    for spec in plan.chunk_fetch_specs() {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }

    let provider_id_hex = "ab".repeat(32);
    let stream_token_b64 =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 4);
    let output_path = tempdir.path().join("assembled.bin");
    let summary_path = tempdir.path().join("fetch_summary.json");
    let base_url = server.url("/");

    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=alpha,provider-id={provider_id_hex},base-url={base_url},stream-token={stream_token_b64}"
        ))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg("--max-peers=1")
        .arg("--retry-budget=4")
        .assert()
        .success();

    let stdout =
        String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8 summary");
    let stdout_summary: Value =
        norito::json::from_str(stdout.trim()).expect("stdout must be json summary");
    assert_eq!(
        stdout_summary.get("chunk_count").and_then(Value::as_u64),
        Some(plan.chunk_fetch_specs().len() as u64)
    );
    assert_eq!(
        stdout_summary.get("chunker_handle").and_then(Value::as_str),
        Some("sorafs.sf1@1.0.0")
    );

    let assembled = fs::read(&output_path).expect("assembled payload");
    assert_eq!(assembled, payload);

    let summary_bytes = fs::read(&summary_path).expect("read summary");
    let summary_file: Value = from_slice(&summary_bytes).expect("parse summary json");
    assert_eq!(
        summary_file.get("manifest_id_hex").and_then(Value::as_str),
        Some(manifest_id_hex.as_str())
    );
    let provider_reports = summary_file
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider reports array");
    assert_eq!(provider_reports.len(), 1);
    assert_eq!(
        provider_reports[0]
            .as_object()
            .and_then(|obj| obj.get("provider"))
            .and_then(Value::as_str),
        Some("alpha")
    );
    let receipts = summary_file
        .get("chunk_receipts")
        .and_then(Value::as_array)
        .expect("chunk receipts array");
    assert_eq!(receipts.len(), plan.chunk_fetch_specs().len());
}

#[test]
fn fetch_command_respects_direct_transports() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..2048).map(|idx| (idx % 199) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json =
        chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let server = MockServer::start();
    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let payload_digest_hex = hex_encode(plan.payload_digest.as_bytes());
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        manifest_id_hex,
        BASE64_STANDARD.encode(&manifest_bytes),
        manifest_digest_hex,
        payload_digest_hex,
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );

    let manifest_report_path = tempdir.path().join("proxy_manifest_report.json");
    fs::write(
        &manifest_report_path,
        format!("{}\n", manifest_response).as_bytes(),
    )
    .expect("write manifest report");

    for spec in plan.chunk_fetch_specs() {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }

    let provider_id_hex = "12".repeat(32);
    let stream_token_b64 =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 2);
    let summary_path = tempdir.path().join("direct_fetch_summary.json");
    let output_path = tempdir.path().join("direct_payload.bin");

    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=gw-direct,provider-id={provider_id_hex},base-url={},stream-token={stream_token_b64}",
            server.url("/")
        ))
        .arg("--transport-policy=direct-only")
        .arg("--max-peers=1")
        .arg("--retry-budget=3")
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .assert()
        .success();

    let stdout_summary: Value = norito::json::from_slice(assert.get_output().stdout.as_slice())
        .expect("stdout summary json");
    assert_eq!(
        stdout_summary.get("chunk_count").and_then(Value::as_u64),
        Some(plan.chunk_fetch_specs().len() as u64)
    );
    assert_eq!(
        stdout_summary
            .get("manifest_id_hex")
            .and_then(Value::as_str),
        Some(manifest_id_hex.as_str())
    );

    let file_summary_bytes = fs::read(&summary_path).expect("read summary file");
    let file_summary: Value =
        norito::json::from_slice(&file_summary_bytes).expect("parse summary file");
    let provider_reports = file_summary
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider reports array");
    assert_eq!(
        provider_reports.len(),
        1,
        "only direct providers should be scheduled"
    );
    assert_eq!(
        provider_reports[0]
            .as_object()
            .and_then(|obj| obj.get("provider"))
            .and_then(Value::as_str),
        Some("gw-direct")
    );

    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload);
}

#[test]
fn fetch_command_applies_policy_override() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..1024).map(|idx| (idx % 157) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json =
        chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let server = MockServer::start();
    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(
            sorafs_chunker::ChunkProfile::DEFAULT,
            BLAKE3_256_MULTIHASH_CODE,
        )
        .content_length(payload.len() as u64)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();

    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        manifest_id_hex,
        BASE64_STANDARD.encode(&manifest_bytes),
        manifest_digest_hex,
        hex_encode(plan.payload_digest.as_bytes()),
        plan.content_length,
        plan.chunks.len(),
        "sorafs.sf1@1.0.0"
    );
    let manifest_report_path = tempdir.path().join("gateway_manifest_override.json");
    fs::write(&manifest_report_path, format!("{}\n", manifest_response)).expect("write report");

    for spec in plan.chunk_fetch_specs() {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }

    let provider_id_hex = "ca".repeat(32);
    let stream_token_b64 =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 2);
    let output_path = tempdir.path().join("override.bin");
    let summary_path = tempdir.path().join("override_summary.json");
    let base_url = server.url("/");

    sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=alpha,provider-id={provider_id_hex},base-url={base_url},stream-token={stream_token_b64}"
        ))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg("--anonymity-policy-override=anon-guard-pq")
        .assert()
        .success();

    let summary_bytes = fs::read(&summary_path).expect("read override summary");
    let summary: Value = from_slice(&summary_bytes).expect("parse override summary");
    assert_eq!(
        summary.get("anonymity_policy").and_then(Value::as_str),
        Some("anon-guard-pq")
    );
}

#[test]
fn fetch_command_uses_orchestrator_config_json() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..1024).map(|idx| (idx % 151) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json =
        chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let payload_digest_hex = hex_encode(plan.payload_digest.as_bytes());
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_report_path = tempdir.path().join("direct_manifest_report.json");
    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        manifest_id_hex,
        BASE64_STANDARD.encode(&manifest_bytes),
        manifest_digest_hex,
        payload_digest_hex,
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );
    fs::write(
        &manifest_report_path,
        format!("{}\n", manifest_response).as_bytes(),
    )
    .expect("write manifest report");

    let server = MockServer::start();
    for spec in plan.chunk_fetch_specs() {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }
    let provider_id_hex = "34".repeat(32);
    let stream_token_b64 =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 3);
    let summary_path = tempdir.path().join("policy_fetch_summary.json");
    let output_path = tempdir.path().join("policy_payload.bin");
    let scoreboard_path = tempdir
        .path()
        .join("scoreboards/direct_policy_scoreboard.json");
    let policy_path = tempdir.path().join("direct_policy.json");

    let mut scoreboard = Map::new();
    scoreboard.insert("latency_cap_ms".into(), Value::from(3500u64));
    scoreboard.insert("weight_scale".into(), Value::from(200u64));
    scoreboard.insert("telemetry_grace_secs".into(), Value::from(45u64));
    scoreboard.insert(
        "persist_path".into(),
        Value::from(scoreboard_path.display().to_string()),
    );
    scoreboard.insert("now_unix_secs".into(), Value::from(1_700_000_000u64));

    let mut fetch = Map::new();
    fetch.insert("retry_budget".into(), Value::from(4u64));
    fetch.insert("provider_failure_threshold".into(), Value::from(2u64));
    fetch.insert("global_parallel_limit".into(), Value::from(1u64));
    fetch.insert("verify_lengths".into(), Value::from(true));
    fetch.insert("verify_digests".into(), Value::from(true));

    let mut root = Map::new();
    root.insert("scoreboard".into(), Value::Object(scoreboard));
    root.insert("fetch".into(), Value::Object(fetch));
    root.insert("telemetry_region".into(), Value::from("regulated-eu"));
    root.insert("max_providers".into(), Value::from(1u64));
    root.insert("transport_policy".into(), Value::from("direct-only"));

    let rendered = norito::json::to_string_pretty(&Value::Object(root))
        .expect("render orchestrator config json");
    fs::write(&policy_path, rendered.as_bytes()).expect("write orchestrator config json");

    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!("--manifest-report={}", manifest_report_path.display()))
        .arg(format!(
            "--provider=name=policy-gw,provider-id={provider_id_hex},base-url={},stream-token={stream_token_b64}",
            server.url("/")
        ))
        .arg(format!("--orchestrator-config={}", policy_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .assert()
        .success();

    let summary_value: Value =
        norito::json::from_slice(assert.get_output().stdout.as_slice()).expect("stdout summary");
    assert_eq!(
        summary_value
            .get("telemetry_region")
            .and_then(Value::as_str),
        Some("regulated-eu")
    );

    let summary_bytes = fs::read(&summary_path).expect("read summary file");
    let summary_file: Value =
        norito::json::from_slice(&summary_bytes).expect("parse summary file json");
    assert_eq!(
        summary_file.get("telemetry_region").and_then(Value::as_str),
        Some("regulated-eu")
    );

    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload);

    let scoreboard_bytes = fs::read(&scoreboard_path).expect("persisted scoreboard json");
    let persisted_scoreboard: Value =
        norito::json::from_slice(&scoreboard_bytes).expect("parse scoreboard json");
    let providers = persisted_scoreboard
        .get("entries")
        .and_then(Value::as_array)
        .expect("persisted scoreboard entries array");
    assert_eq!(providers.len(), 1);
    assert_eq!(
        providers[0]
            .as_object()
            .and_then(|obj| obj.get("provider_id"))
            .and_then(Value::as_str),
        Some("policy-gw")
    );
}

#[test]
fn fetch_command_persists_scoreboard_via_flag() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..2048).map(|idx| (idx % 179) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json =
        chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_report_path = tempdir.path().join("flag_manifest_report.json");
    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{manifest_id_hex}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{manifest_digest_hex}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        BASE64_STANDARD.encode(&manifest_bytes),
        hex_encode(plan.payload_digest.as_bytes()),
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );
    fs::write(
        &manifest_report_path,
        format!("{manifest_response}\n").as_bytes(),
    )
    .expect("write manifest report");

    let server = MockServer::start();
    for spec in plan.chunk_fetch_specs() {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }
    let provider_id_hex = "56".repeat(32);
    let stream_token_b64 =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, chunk_profile_handle, 2);

    let summary_path = tempdir.path().join("flag_summary.json");
    let output_path = tempdir.path().join("flag_payload.bin");
    let scoreboard_path = tempdir
        .path()
        .join("scoreboards/flag_fetch_scoreboard.json");

    sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=flag-gw,provider-id={provider_id_hex},base-url={},stream-token={stream_token_b64}",
            server.url("/")
        ))
        .arg(format!(
            "--manifest-report={}",
            manifest_report_path.display()
        ))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--scoreboard-out={}", scoreboard_path.display()))
        .arg("--scoreboard-now=1700000000")
        .assert()
        .success();

    let scoreboard_bytes = fs::read(&scoreboard_path).expect("read scoreboard file");
    let scoreboard_value: Value =
        norito::json::from_slice(&scoreboard_bytes).expect("parse scoreboard json");
    let entries = scoreboard_value
        .get("entries")
        .and_then(Value::as_array)
        .expect("entries array");
    assert!(
        !entries.is_empty(),
        "scoreboard entries should not be empty"
    );
    let first = entries[0]
        .get("provider_id")
        .and_then(Value::as_str)
        .expect("provider id");
    assert!(
        !first.is_empty(),
        "provider id should be recorded in scoreboard"
    );
}

#[test]
fn fetch_command_writes_local_proxy_manifest() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..512).map(|idx| (idx % 97) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json =
        chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let payload_digest_hex = hex_encode(plan.payload_digest.as_bytes());
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_report_path = tempdir.path().join("proxy_manifest_report.json");
    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        manifest_id_hex,
        BASE64_STANDARD.encode(&manifest_bytes),
        manifest_digest_hex,
        payload_digest_hex,
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );
    fs::write(
        &manifest_report_path,
        format!("{}\n", manifest_response).as_bytes(),
    )
    .expect("write manifest report");

    let server = MockServer::start();
    for spec in plan.chunk_fetch_specs() {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }

    let provider_id_hex = "ab".repeat(32);
    let stream_token_b64 =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 2);
    let summary_path = tempdir.path().join("proxy_fetch_summary.json");
    let manifest_out_path = tempdir.path().join("proxy_manifest.json");
    let policy_path = tempdir.path().join("proxy_config.json");

    let mut scoreboard = Map::new();
    scoreboard.insert("latency_cap_ms".into(), Value::from(3000u64));
    scoreboard.insert("weight_scale".into(), Value::from(100u64));
    scoreboard.insert("telemetry_grace_secs".into(), Value::from(30u64));
    scoreboard.insert("now_unix_secs".into(), Value::from(1_701_000_000u64));

    let mut fetch = Map::new();
    fetch.insert("retry_budget".into(), Value::from(3u64));
    fetch.insert("provider_failure_threshold".into(), Value::from(2u64));
    fetch.insert("global_parallel_limit".into(), Value::from(2u64));
    fetch.insert("verify_lengths".into(), Value::from(true));
    fetch.insert("verify_digests".into(), Value::from(true));

    let mut local_proxy = Map::new();
    local_proxy.insert("bind_addr".into(), Value::from("127.0.0.1:0"));
    local_proxy.insert("telemetry_label".into(), Value::from("test-proxy"));
    local_proxy.insert("proxy_mode".into(), Value::from("bridge"));
    local_proxy.insert("emit_browser_manifest".into(), Value::from(true));

    let mut root = Map::new();
    root.insert("scoreboard".into(), Value::Object(scoreboard));
    root.insert("fetch".into(), Value::Object(fetch));
    root.insert("local_proxy".into(), Value::Object(local_proxy));

    let rendered =
        norito::json::to_string_pretty(&Value::Object(root)).expect("render orchestrator config");
    fs::write(&policy_path, rendered.as_bytes()).expect("write orchestrator config");

    sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!("--manifest-report={}", manifest_report_path.display()))
        .arg(format!(
            "--provider=name=proxy-gw,provider-id={provider_id_hex},base-url={},stream-token={stream_token_b64}",
            server.url("/")
        ))
        .arg(format!("--orchestrator-config={}", policy_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!(
            "--local-proxy-manifest-out={}",
            manifest_out_path.display()
        ))
        .assert()
        .success();

    let summary_bytes = fs::read(&summary_path).expect("read summary json");
    let summary_value: Value =
        from_slice(&summary_bytes).expect("summary json must parse into Value");
    let manifest_from_summary = summary_value
        .get("local_proxy_manifest")
        .expect("summary should include proxy manifest")
        .clone();
    let summary_mode = summary_value
        .get("local_proxy_mode")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_mode");
    assert_eq!(summary_mode, "bridge");
    let summary_spool = summary_value
        .get("local_proxy_norito_spool")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_norito_spool");
    assert_eq!(summary_spool, PROVISION_SPOOL_DIR);
    let summary_kaigi_spool = summary_value
        .get("local_proxy_kaigi_spool")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_kaigi_spool");
    assert_eq!(summary_kaigi_spool, PROVISION_SPOOL_DIR);
    let summary_kaigi_policy = summary_value
        .get("local_proxy_kaigi_policy")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_kaigi_policy");
    assert_eq!(summary_kaigi_policy, "public");

    let manifest_bytes = fs::read(&manifest_out_path).expect("read manifest json");
    let manifest_value: Value =
        from_slice(&manifest_bytes).expect("manifest json should parse into Value");
    assert_eq!(
        manifest_value, manifest_from_summary,
        "manifest exported to disk should match summary"
    );

    let authority = manifest_value
        .get("authority")
        .and_then(Value::as_str)
        .expect("manifest authority");
    assert!(
        authority.starts_with("127.0.0.1:"),
        "authority `{authority}` should bind to loopback"
    );
    let proxy_mode = manifest_value
        .get("proxy_mode")
        .and_then(Value::as_str)
        .expect("proxy_mode");
    assert_eq!(proxy_mode, "bridge");
    let cert_pem = manifest_value
        .get("certificate_pem")
        .and_then(Value::as_str)
        .expect("certificate_pem");
    assert!(
        cert_pem.contains("BEGIN CERTIFICATE"),
        "manifest should contain embedded PEM certificate"
    );
    let salt_hex = manifest_value
        .get("cache_tagging")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("salt_hex"))
        .and_then(Value::as_str)
        .expect("cache_tagging.salt_hex");
    assert_eq!(salt_hex.len(), 32, "salt_hex must be 16 bytes encoded");

    let summary_override_path = tempdir.path().join("proxy_fetch_summary_override.json");
    let manifest_override_path = tempdir.path().join("proxy_manifest_override.json");
    sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=proxy-gw,provider-id={provider_id_hex},base-url={},stream-token={stream_token_b64}",
            server.url("/")
        ))
        .arg(format!("--orchestrator-config={}", policy_path.display()))
        .arg(format!("--json-out={}", summary_override_path.display()))
        .arg(format!(
            "--local-proxy-manifest-out={}",
            manifest_override_path.display()
        ))
        .arg("--local-proxy-mode=metadata-only")
        .assert()
        .success();

    let summary_override_bytes =
        fs::read(&summary_override_path).expect("read override summary json");
    let summary_override: Value =
        from_slice(&summary_override_bytes).expect("override summary json must parse");
    assert_eq!(
        summary_override
            .get("local_proxy_mode")
            .and_then(Value::as_str),
        Some("metadata-only")
    );
    assert!(
        summary_override.get("local_proxy_norito_spool").is_none(),
        "metadata-only overrides should not advertise a spool directory"
    );
    let manifest_override_bytes =
        fs::read(&manifest_override_path).expect("read override manifest json");
    let manifest_override: Value =
        from_slice(&manifest_override_bytes).expect("override manifest json should parse");
    assert_eq!(
        manifest_override.get("proxy_mode").and_then(Value::as_str),
        Some("metadata-only")
    );
}

#[test]
fn taikai_car_cli_generates_bundle() {
    let dir = tempdir().expect("tempdir");
    let payload_path = dir.path().join("segment.m4s");
    fs::write(&payload_path, b"taikai-payload").expect("write payload");
    let car_path = dir.path().join("segment.car");
    let envelope_path = dir.path().join("segment.to");
    let indexes_path = dir.path().join("segment.indexes.json");
    let ingest_path = dir.path().join("segment.ingest.json");

    let mut cmd = taikai_car_cmd();
    cmd.arg("--payload")
        .arg(&payload_path)
        .arg("--car-out")
        .arg(&car_path)
        .arg("--envelope-out")
        .arg(&envelope_path)
        .arg("--indexes-out")
        .arg(&indexes_path)
        .arg("--ingest-metadata-out")
        .arg(&ingest_path)
        .args([
            "--event-id",
            "demo-event",
            "--stream-id",
            "stage-a",
            "--rendition-id",
            "1080p",
            "--track-kind",
            "video",
            "--codec",
            "av1-main",
            "--bitrate-kbps",
            "8000",
            "--resolution",
            "1920x1080",
            "--segment-sequence",
            "42",
            "--segment-start-pts",
            "3600000",
            "--segment-duration",
            "2000000",
            "--wallclock-unix-ms",
            "1702560000000",
            "--manifest-hash",
            &"11".repeat(32),
            "--storage-ticket",
            &"22".repeat(32),
        ]);
    cmd.assert().success();

    let car_bytes = fs::read(&car_path).expect("read car");
    assert!(!car_bytes.is_empty(), "car archive must contain payload");

    let envelope_bytes = fs::read(&envelope_path).expect("read envelope");
    let envelope: TaikaiSegmentEnvelopeV1 =
        norito::decode_from_bytes(&envelope_bytes).expect("decode envelope");
    assert_eq!(envelope.segment_sequence, 42);

    assert!(indexes_path.exists(), "indexes JSON should exist");
    assert!(ingest_path.exists(), "ingest metadata JSON should exist");
}

#[test]
fn sorafs_cli_taikai_bundle_generates_artifacts() {
    let dir = tempdir().expect("tempdir");
    let payload_path = dir.path().join("segment_bundle.bin");
    fs::write(&payload_path, b"bundle-me").expect("write payload");
    let car_path = dir.path().join("segment_bundle.car");
    let envelope_path = dir.path().join("segment_bundle.to");
    let indexes_path = dir.path().join("segment_bundle.index.json");
    let ingest_path = dir.path().join("segment_bundle.ingest.json");
    let summary_path = dir.path().join("segment_bundle.summary.json");

    let mut cmd = sorafs_cli_cmd();
    cmd.arg("taikai")
        .arg("bundle")
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--envelope-out={}", envelope_path.display()))
        .arg(format!("--indexes-out={}", indexes_path.display()))
        .arg(format!("--ingest-metadata-out={}", ingest_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .arg("--event-id=demo-event")
        .arg("--stream-id=stage-b")
        .arg("--rendition-id=720p")
        .arg("--track-kind=video")
        .arg("--codec=avc-high")
        .arg("--bitrate-kbps=4500")
        .arg("--resolution=1280x720")
        .arg("--segment-sequence=7")
        .arg("--segment-start-pts=700000")
        .arg("--segment-duration=1000000")
        .arg("--wallclock-unix-ms=1702561000000")
        .arg(format!("--manifest-hash={}", "33".repeat(32)))
        .arg(format!("--storage-ticket={}", "44".repeat(32)))
        .arg("--ingest-latency-ms=42")
        .arg("--live-edge-drift-ms=-17")
        .arg("--ingest-node-id=node-a");
    cmd.assert().success();

    assert!(car_path.exists(), "car output should exist");
    assert!(envelope_path.exists(), "envelope output should exist");
    assert!(indexes_path.exists(), "indexes output should exist");
    assert!(ingest_path.exists(), "ingest metadata output should exist");
    assert!(summary_path.exists(), "summary output should exist");

    let envelope_bytes = fs::read(&envelope_path).expect("read envelope");
    let envelope: TaikaiSegmentEnvelopeV1 =
        norito::decode_from_bytes(&envelope_bytes).expect("decode envelope");
    assert_eq!(envelope.segment_sequence, 7);
    assert_eq!(
        envelope.instrumentation.encoder_to_ingest_latency_ms,
        Some(42)
    );
    assert_eq!(envelope.instrumentation.live_edge_drift_ms, Some(-17));

    let summary_bytes = fs::read(&summary_path).expect("read summary");
    let summary_json: Value = from_slice(&summary_bytes).expect("summary json");
    assert_eq!(
        summary_json
            .get("ingest")
            .and_then(|ingest| ingest.get("event_id"))
            .and_then(Value::as_str),
        Some("demo-event")
    );
    assert_eq!(
        summary_json
            .get("car")
            .and_then(|car| car.get("cid_multibase"))
            .and_then(Value::as_str)
            .map(|s| s.starts_with('b')),
        Some(true)
    );
}
