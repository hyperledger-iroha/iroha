#![cfg(feature = "cli-orchestrator")]
#![cfg_attr(feature = "cli-orchestrator", allow(unexpected_cfgs))]
use assert_cmd::{Command as AssertCommand, cargo::cargo_bin_cmd};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use blake3::hash as blake3_hash;
use ed25519_dalek::{Signer as _, SigningKey};
use hex::{decode as hex_decode, encode as hex_encode};
use httpmock::{Mock, prelude::*};
#[cfg(feature = "local-quic-proxy")]
use iroha_config::parameters::defaults::streaming::soranet::PROVISION_SPOOL_DIR;
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
use iroha_data_model::account::AccountId;
use iroha_data_model::taikai::TaikaiSegmentEnvelopeV1;
use norito::{
    codec::Encode as _,
    decode_from_bytes,
    derive::NoritoSerialize,
    json::{Map, Value, from_slice, to_vec},
    to_bytes,
};
use sha3::{Digest, Sha3_256};
use sorafs_car::{
    CarBuildPlan, CarWriter, chunker_registry, compute_chunk_plan_digest_sha3, compute_por_root,
    fetch_plan::{chunk_fetch_plan_from_json, chunk_fetch_plan_to_string},
};
use sorafs_manifest::por::{
    POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1, POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1,
    POR_STATUS_CURSOR_VERSION_V1, PorStatusCursorV1,
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, CouncilSignature, DagCodecId, GOVERNANCE_LOG_VERSION_V1,
    GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceDagSubmissionOriginV1,
    GovernanceDagSubmissionProvenanceV1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
    GovernanceLogSignatureV1, GovernanceProofs, GovernanceSignatureAlgorithm, ManifestBuilder,
    ManifestV1, POR_CHALLENGE_STATUS_VERSION_V1, POR_WEEKLY_REPORT_VERSION_V1, PinPolicy,
    PorChallengeOutcome, PorChallengeStatusV1, PorProviderSummaryV1, PorReportIsoWeek,
    PorSlashingEventV1, PorWeeklyReportV1, REPUTATION_PROVIDER_INPUT_VERSION_V1,
    REPUTATION_PROVIDER_METRICS_VERSION_V1, ReputationProviderInputV1, ReputationProviderMetricsV1,
    ReputationReserveStageV1, ReputationSnapshotV1, ReputationWeightsV1,
    SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1, SoraFsAppealFinanceAccountFlowV1,
    SoraFsAppealFinanceJurorPayoutV1, SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
    StorageClass, StreamTokenBodyV1, StreamTokenV1, XorQuantity, build_reputation_snapshot,
    governance_dag_submission_account_digest_v1, validate_governance_dag_head_against_chain_v1,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
const TEST_NETWORK_ID_LITERAL: &str =
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
fn sorafs_cli_cmd() -> AssertCommand {
    cargo_bin_cmd!("sorafs_cli")
}
fn assert_insecure_gateway_rejected(assert: assert_cmd::assert::Assert, output_paths: &[&Path]) {
    let assert = assert.failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("URL must use HTTPS") || stderr.contains("globally routable"),
        "expected fail-closed gateway URL error, got: {stderr}"
    );
    for path in output_paths {
        assert!(
            !path.exists(),
            "gateway URL validation must fail before writing {}",
            path.display()
        );
    }
}
struct CanonicalTempDir {
    _inner: TempDir,
    path: PathBuf,
}
impl CanonicalTempDir {
    fn path(&self) -> &Path {
        &self.path
    }
}
#[derive(NoritoSerialize)]
struct TestPorStatusPageV1 {
    version: u8,
    snapshot_generation: u64,
    record_limit: u32,
    canonical_byte_limit: u64,
    canonical_bytes: u64,
    inspected_candidates: u32,
    has_more: bool,
    #[norito(default)]
    next_cursor: Option<String>,
    statuses: Vec<PorChallengeStatusV1>,
}
#[derive(NoritoSerialize)]
struct TestPorStatusExportPageV1 {
    version: u8,
    #[norito(default)]
    start_epoch: Option<u64>,
    #[norito(default)]
    end_epoch: Option<u64>,
    page: TestPorStatusPageV1,
}
fn test_por_status_page(
    statuses: Vec<PorChallengeStatusV1>,
    next_cursor: Option<String>,
) -> TestPorStatusPageV1 {
    let canonical_bytes = statuses
        .iter()
        .map(|status| to_bytes(status).expect("encode PoR status fixture").len())
        .sum::<usize>();
    TestPorStatusPageV1 {
        version: 1,
        snapshot_generation: u64::try_from(statuses.len())
            .expect("fixture status count fits u64")
            .checked_add(1)
            .expect("fixture generation does not overflow"),
        record_limit: u32::try_from(POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1)
            .expect("PoR page limit fits u32"),
        canonical_byte_limit: u64::try_from(POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1)
            .expect("PoR byte limit fits u64"),
        canonical_bytes: u64::try_from(canonical_bytes).expect("fixture byte total fits u64"),
        inspected_candidates: u32::try_from(statuses.len()).expect("fixture status count fits u32"),
        has_more: next_cursor.is_some(),
        next_cursor,
        statuses,
    }
}
fn test_por_cursor(
    snapshot_generation: u64,
    epoch_id: u64,
    issued_at: u64,
    challenge_id: [u8; 32],
) -> String {
    PorStatusCursorV1 {
        version: POR_STATUS_CURSOR_VERSION_V1,
        snapshot_generation,
        selection_digest: [0xA5; 32],
        last_epoch_id: epoch_id,
        last_issued_at: issued_at,
        last_challenge_id: challenge_id,
    }
    .encode_opaque()
    .expect("encode canonical PoR cursor fixture")
}
fn large_por_status_page() -> TestPorStatusPageV1 {
    let statuses = (0..512)
        .map(|index| {
            let ordinal = u64::try_from(index + 1).expect("fixture ordinal fits u64");
            let mut challenge_id = [0x11; 32];
            challenge_id[..8].copy_from_slice(&ordinal.to_be_bytes());
            PorChallengeStatusV1 {
                version: POR_CHALLENGE_STATUS_VERSION_V1,
                challenge_id,
                manifest_digest: [0x22; 32],
                provider_id: [0x33; 32],
                epoch_id: 42,
                drand_round: 100 + ordinal,
                status: PorChallengeOutcome::AwaitingProof,
                sample_count: 64,
                forced: false,
                issued_at: 1_700_000_000 + ordinal,
                responded_at: None,
                proof_digest: None,
                repair_task_id: None,
                failure_reason: None,
                verifier_latency_ms: None,
            }
        })
        .collect();
    test_por_status_page(statuses, None)
}
fn tempdir() -> std::io::Result<CanonicalTempDir> {
    let inner = tempfile::tempdir()?;
    let path = inner.path().canonicalize()?;
    Ok(CanonicalTempDir {
        _inner: inner,
        path,
    })
}
fn deterministic_ed25519_authority_and_private_key() -> (String, String) {
    let keypair = KeyPair::try_from_seed(
        b"sorafs-cli-manifest-submit-authority".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture SoraFS CLI manifest authority key");
    let authority = AccountId::new(keypair.public_key().clone()).to_string();
    let private_key = ExposedPrivateKey(keypair.private_key().clone()).to_string();
    (authority, private_key)
}
fn write_deploy_client_config(dir: &Path, torii_url: &str) -> (PathBuf, String) {
    let keypair = KeyPair::try_from_seed(
        b"sorafs-cli-deploy-authority-seed".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture SoraFS CLI deploy authority key");
    let public_key = keypair.public_key().to_string();
    let private_key = ExposedPrivateKey(keypair.private_key().clone()).to_string();
    let path = dir.join("client.toml");
    fs::write(
        &path,
        format!(
            r#"
torii_url = "{torii_url}"
network_id = "{TEST_NETWORK_ID_LITERAL}"

[account]
public_key = "{public_key}"
private_key = "{private_key}"
chain_discriminant = 369
"#
        ),
    )
    .expect("write deploy client config");
    (path, private_key)
}
fn write_deploy_client_config_with_chain(
    dir: &Path,
    torii_url: &str,
    chain: &str,
) -> (PathBuf, String) {
    let keypair = KeyPair::try_from_seed(
        b"sorafs-cli-deploy-authority-seed".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture SoraFS CLI deploy authority key");
    let public_key = keypair.public_key().to_string();
    let private_key = ExposedPrivateKey(keypair.private_key().clone()).to_string();
    let path = dir.join("client-known-chain.toml");
    fs::write(
        &path,
        format!(
            r#"
chain = "{chain}"
torii_url = "{torii_url}"
network_id = "{TEST_NETWORK_ID_LITERAL}"

[account]
public_key = "{public_key}"
private_key = "{private_key}"
"#
        ),
    )
    .expect("write deploy client config with chain");
    (path, private_key)
}
fn make_stream_token_b64(
    manifest_id_hex: &str,
    provider_id_hex: &str,
    profile: &str,
    max_streams: u16,
) -> (String, String) {
    let mut provider_id = [0u8; 32];
    provider_id.copy_from_slice(&hex_decode(provider_id_hex).expect("decode provider identifier"));
    let signing_key = SigningKey::from_bytes(&[0x42; 32]);
    let token = StreamTokenV1::sign(
        StreamTokenBodyV1 {
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
        &signing_key,
    )
    .expect("sign stream token fixture");
    let bytes = to_bytes(&token).expect("encode stream token");
    (
        BASE64_STANDARD.encode(bytes),
        hex_encode(signing_key.verifying_key().to_bytes()),
    )
}
fn council_signed_governance_proofs() -> GovernanceProofs {
    GovernanceProofs {
        council_signatures: vec![CouncilSignature {
            signer: [0x42; 32],
            signature: vec![0x24; 64],
        }],
    }
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
    let canonical_plan =
        chunk_fetch_plan_from_json(&plan_json).expect("plan should be a canonical V1 envelope");
    assert_eq!(
        canonical_plan.payload_digest,
        *blake3_hash(&payload).as_bytes(),
        "plan must bind the complete payload"
    );
    let plan_array = canonical_plan.chunk_fetch_specs;
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
        status: PorChallengeOutcome::AwaitingProof,
        sample_count: 64,
        forced: false,
        issued_at: 1_700_000_000,
        responded_at: None,
        proof_digest: None,
        repair_task_id: None,
        failure_reason: None,
        verifier_latency_ms: None,
    };
    let body = to_bytes(&test_por_status_page(vec![status], None)).expect("encode status page");
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
        stdout.contains("awaiting_proof"),
        "expected status output to mention awaiting-proof status:\n{stdout}"
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
    let body = to_bytes(&test_por_status_page(vec![status], None)).expect("encode status page");
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
fn por_status_accepts_empty_sparse_page_with_advancing_cursor() {
    let server = MockServer::start();
    let cursor = test_por_cursor(1, 42, 1_700_000_000, [0x11; 32]);
    let mut page = test_por_status_page(Vec::new(), Some(cursor.clone()));
    page.inspected_candidates = 512;
    let body = to_bytes(&page).expect("encode empty sparse status page");
    server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/status");
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(body);
    });
    let stderr = sorafs_cli_cmd()
        .arg("por")
        .arg("status")
        .arg(format!("--torii-url={}", server.base_url()))
        .assert()
        .success()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(stderr).expect("stderr utf8");
    assert!(
        stderr.contains(&format!("next_cursor={cursor}")),
        "empty sparse page must expose its advancing cursor:\n{stderr}"
    );
}
#[test]
fn por_status_and_export_accept_fields_above_legacy_64k_limit() {
    let server = MockServer::start();
    let page = large_por_status_page();
    let status_body = to_bytes(&page).expect("encode large bounded status page");
    assert!(
        status_body.len() > 64 * 1024,
        "fixture must exceed the retired decoder field ceiling"
    );
    assert!(
        status_body.len() <= POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1 + 64 * 1024,
        "fixture must remain within the Torii response envelope"
    );
    server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/status");
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(status_body);
    });
    sorafs_cli_cmd()
        .arg("por")
        .arg("status")
        .arg(format!("--torii-url={}", server.base_url()))
        .assert()
        .success();
    let export_body = to_bytes(&TestPorStatusExportPageV1 {
        version: 1,
        start_epoch: None,
        end_epoch: None,
        page,
    })
    .expect("encode large bounded status export");
    assert!(
        export_body.len() > 64 * 1024,
        "nested export page must exceed the retired field ceiling"
    );
    assert!(
        export_body.len() <= POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1 + 64 * 1024,
        "export fixture must remain within the Torii response envelope"
    );
    server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/export");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(export_body);
    });
    let tempdir = tempdir().expect("tempdir");
    let out_path = tempdir.path().join("large-por-export.norito");
    sorafs_cli_cmd()
        .arg("por")
        .arg("export")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--out={}", out_path.display()))
        .assert()
        .success();
    assert!(out_path.exists());
}
#[test]
fn por_status_and_export_reject_record_byte_limit_above_torii_contract() {
    let server = MockServer::start();
    let above = POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1 + 1;
    let expected =
        format!("`--max-bytes` must be in 1..={POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1}");
    let status_stderr = sorafs_cli_cmd()
        .arg("por")
        .arg("status")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--max-bytes={above}"))
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8(status_stderr)
            .expect("status stderr utf8")
            .contains(&expected)
    );
    let tempdir = tempdir().expect("tempdir");
    let export_stderr = sorafs_cli_cmd()
        .arg("por")
        .arg("export")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!(
            "--out={}",
            tempdir.path().join("unused.to").display()
        ))
        .arg(format!("--max-bytes={above}"))
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8(export_stderr)
            .expect("export stderr utf8")
            .contains(&expected)
    );
}
#[test]
fn por_status_rejects_record_outside_requested_filters_before_output() {
    let server = MockServer::start();
    let status = PorChallengeStatusV1 {
        version: POR_CHALLENGE_STATUS_VERSION_V1,
        challenge_id: [0x31; 32],
        manifest_digest: [0x99; 32],
        provider_id: [0x33; 32],
        epoch_id: 42,
        drand_round: 100,
        status: PorChallengeOutcome::AwaitingProof,
        sample_count: 64,
        forced: false,
        issued_at: 1_700_000_000,
        responded_at: None,
        proof_digest: None,
        repair_task_id: None,
        failure_reason: None,
        verifier_latency_ms: None,
    };
    let body = to_bytes(&test_por_status_page(vec![status], None))
        .expect("encode substituted-filter status page");
    let manifest_hex = hex_encode([0x22; 32]);
    let provider_hex = hex_encode([0x33; 32]);
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/por/status")
            .query_param("manifest", manifest_hex.as_str())
            .query_param("provider", provider_hex.as_str())
            .query_param("epoch", "42")
            .query_param("status", "awaiting_proof");
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(body);
    });
    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("status")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--manifest={manifest_hex}"))
        .arg(format!("--provider={provider_hex}"))
        .arg("--epoch=42")
        .arg("--status=awaiting_proof")
        .output()
        .expect("command executes");
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "untrusted status must not be output"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(
        stderr.contains("does not match the requested manifest filter"),
        "unexpected stderr: {stderr}"
    );
}
#[test]
fn retired_por_trigger_command_is_absent_and_never_sends_a_request() {
    let server = MockServer::start();
    let trigger_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/por/trigger")
            .header("content-type", "application/x-norito");
        then.status(500);
    });
    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("trigger")
        .arg(format!("--torii-url={}", server.base_url()))
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).expect("CLI stderr is UTF-8");
    assert!(
        stderr.contains("sorafs_cli por status"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !stderr.contains("sorafs_cli por trigger"),
        "retired command leaked into usage: {stderr}"
    );
    trigger_mock.assert_calls(0);
}
#[test]
fn por_export_writes_file() {
    let server = MockServer::start();
    let status = PorChallengeStatusV1 {
        version: POR_CHALLENGE_STATUS_VERSION_V1,
        challenge_id: [0x41; 32],
        manifest_digest: [0x42; 32],
        provider_id: [0x43; 32],
        epoch_id: 10,
        drand_round: 101,
        status: PorChallengeOutcome::AwaitingProof,
        sample_count: 32,
        forced: false,
        issued_at: 1_700_000_200,
        responded_at: None,
        proof_digest: None,
        repair_task_id: None,
        failure_reason: None,
        verifier_latency_ms: None,
    };
    let next_cursor = test_por_cursor(2, status.epoch_id, status.issued_at, status.challenge_id);
    let payload = to_bytes(&TestPorStatusExportPageV1 {
        version: 1,
        start_epoch: Some(10),
        end_epoch: Some(10),
        page: test_por_status_page(vec![status], Some(next_cursor.clone())),
    })
    .expect("encode bounded PoR export page");
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/por/export")
            .query_param("start_epoch", "10")
            .query_param("end_epoch", "10");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(payload.clone());
    });
    let tempdir = tempdir().expect("tempdir");
    let out_path = tempdir.path().join("por-export.norito");
    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("export")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--start-epoch=10")
        .arg("--end-epoch=10")
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
    assert!(stdout.contains(&format!("next_cursor={next_cursor}")));
    let written = fs::read(&out_path).expect("read export file");
    assert_eq!(written, payload);
}
#[test]
fn por_export_rejects_noncanonical_response_cursor_without_writing() {
    let server = MockServer::start();
    let status = PorChallengeStatusV1 {
        version: POR_CHALLENGE_STATUS_VERSION_V1,
        challenge_id: [0x51; 32],
        manifest_digest: [0x52; 32],
        provider_id: [0x53; 32],
        epoch_id: 10,
        drand_round: 102,
        status: PorChallengeOutcome::AwaitingProof,
        sample_count: 32,
        forced: false,
        issued_at: 1_700_000_300,
        responded_at: None,
        proof_digest: None,
        repair_task_id: None,
        failure_reason: None,
        verifier_latency_ms: None,
    };
    let payload = to_bytes(&TestPorStatusExportPageV1 {
        version: 1,
        start_epoch: Some(10),
        end_epoch: Some(10),
        page: test_por_status_page(
            vec![status],
            Some("AB".to_owned()), // Decodes like `AA`, but has non-zero trailing bits.
        ),
    })
    .expect("encode malformed-cursor export page");
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/por/export")
            .query_param("start_epoch", "10")
            .query_param("end_epoch", "10");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(payload);
    });
    let tempdir = tempdir().expect("tempdir");
    let out_path = tempdir.path().join("malformed-export.norito");
    let stderr = sorafs_cli_cmd()
        .arg("por")
        .arg("export")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--start-epoch=10")
        .arg("--end-epoch=10")
        .arg(format!("--out={}", out_path.display()))
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8(stderr)
            .expect("stderr utf8")
            .contains("bounded canonical PoR cursor")
    );
    assert!(!out_path.exists());
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
        success_rate_bps: 9_791,
        first_failure_at: Some(1_700_000_300),
        last_success_latency_ms_p95: Some(1_850),
        repair_dispatched: true,
        pending_repairs: 1,
        ticket_id: Some("REP-123".to_string()),
    };
    let slashing_event = PorSlashingEventV1 {
        provider_id: [0x90; 32],
        manifest_digest: [0x91; 32],
        penalty_xor: XorQuantity::try_from_micro(250_000_000)
            .expect("legacy micro-XOR value is representable"),
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
        mean_latency_ms: Some(820),
        p95_latency_ms: Some(1_980),
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
fn proof_stream_cli_rejects_http_and_argv_secrets_before_network() {
    let tempdir = tempdir().expect("tempdir");
    let manifest_path = write_proof_stream_manifest(tempdir.path(), "manifest.to");
    let provider_id_hex = "22".repeat(32);
    let insecure = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--torii-url=http://127.0.0.1:9")
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .assert()
        .failure();
    assert!(String::from_utf8_lossy(&insecure.get_output().stderr).contains("must use HTTPS"));
    for unsafe_url in [
        "https://user:secret@torii.sora.example",
        "https://torii.sora.example?token=secret",
        "https://torii.sora.example#secret",
    ] {
        let rejected = sorafs_cli_cmd()
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg(format!("--torii-url={unsafe_url}"))
            .arg(format!("--provider-id-hex={provider_id_hex}"))
            .assert()
            .failure();
        let stderr = String::from_utf8_lossy(&rejected.get_output().stderr);
        assert!(
            stderr.contains("must not include"),
            "unsafe URL rejection was not explicit: {stderr}"
        );
        assert!(
            !stderr.contains("secret"),
            "unsafe URL credentials leaked into stderr: {stderr}"
        );
    }
    let missing_secret = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--torii-url=https://torii.sora.example")
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .assert()
        .failure();
    assert!(
        String::from_utf8_lossy(&missing_secret.get_output().stderr)
            .contains("missing required `--bearer-token-env=VAR`")
    );
    for (samples, expected) in [
        ("501", "`--samples` must not exceed 500"),
        ("0500", "value must be a canonical unsigned decimal integer"),
    ] {
        let rejected = sorafs_cli_cmd()
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg("--torii-url=https://torii.sora.example")
            .arg(format!("--provider-id-hex={provider_id_hex}"))
            .arg(format!("--samples={samples}"))
            .assert()
            .failure();
        assert!(
            String::from_utf8_lossy(&rejected.get_output().stderr).contains(expected),
            "bounded canonical sample validation must fail before network access"
        );
    }
    for retired in ["--stream-token=secret", "--max-failures=1"] {
        let rejected = sorafs_cli_cmd()
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg("--torii-url=https://torii.sora.example")
            .arg(format!("--provider-id-hex={provider_id_hex}"))
            .arg(retired)
            .assert()
            .failure();
        let option = retired.split('=').next().expect("retired option name");
        assert!(
            String::from_utf8_lossy(&rejected.get_output().stderr)
                .contains(&format!("unrecognised option `{option}`"))
        );
    }
}
#[test]
fn norito_build_compiles_contract() {
    let tempdir = tempdir().expect("tempdir");
    let source_path = PathBuf::from("../kotodama_lang/src/samples/kotodama_swap.ko");
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
    assert_eq!(
        summary_stdout.get("abi_version").and_then(Value::as_u64),
        Some(1),
        "the first-release compiler owns and reports ABI v1"
    );
}
#[test]
fn norito_build_rejects_removed_abi_selection() {
    let tempdir = tempdir().expect("tempdir");
    let source_path = PathBuf::from("../kotodama_lang/src/samples/kotodama_swap.ko");
    let bytecode_path = tempdir.path().join("contract.to");
    let assert = sorafs_cli_cmd()
        .arg("norito")
        .arg("build")
        .arg(format!("--source={}", source_path.display()))
        .arg(format!("--bytecode-out={}", bytecode_path.display()))
        .arg("--abi-version=1")
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("stderr utf8");
    assert!(
        stderr.contains("unrecognised option `--abi-version`"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !bytecode_path.exists(),
        "rejected ABI selection must not publish an artifact"
    );
}
#[test]
fn retired_manifest_authentication_commands_fail_closed_without_io_or_network() {
    let tempdir = tempdir().expect("tempdir");
    let missing_manifest = tempdir.path().join("missing-manifest.to");
    let missing_bundle = tempdir.path().join("missing-bundle.json");
    let bundle_out = tempdir.path().join("retired-bundle.json");
    let signature_out = tempdir.path().join("retired-signature.hex");
    let server = MockServer::start();
    let oidc_probe = server.mock(|when, then| {
        when.method(GET).path("/oidc/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"value":"header.payload.signature"}"#);
    });
    let sign_assert = sorafs_cli_cmd()
        .current_dir(tempdir.path())
        .env("ACTIONS_ID_TOKEN_REQUEST_URL", server.url("/oidc/token"))
        .env("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "must-not-be-used")
        .arg("manifest")
        .arg("sign")
        .arg(format!("--manifest={}", missing_manifest.display()))
        .arg(format!("--bundle-out={}", bundle_out.display()))
        .arg(format!("--signature-out={}", signature_out.display()))
        .arg("--identity-token-provider=github-actions")
        .arg("--identity-token-audience=release.example")
        .assert()
        .failure();
    assert!(
        sign_assert.get_output().stdout.is_empty(),
        "retired manifest sign command must not emit stdout"
    );
    let sign_stderr =
        String::from_utf8(sign_assert.get_output().stderr.clone()).expect("sign stderr utf8");
    assert!(
        sign_stderr.contains("Usage:"),
        "retired manifest sign command must fail as an absent command: {sign_stderr}"
    );
    assert!(
        !sign_stderr.contains("sorafs_cli manifest sign --"),
        "usage must not advertise the retired manifest sign command: {sign_stderr}"
    );
    let verify_assert = sorafs_cli_cmd()
        .current_dir(tempdir.path())
        .arg("manifest")
        .arg("verify-signature")
        .arg(format!("--manifest={}", missing_manifest.display()))
        .arg(format!("--bundle={}", missing_bundle.display()))
        .assert()
        .failure();
    assert!(
        verify_assert.get_output().stdout.is_empty(),
        "retired manifest verify-signature command must not emit stdout"
    );
    let verify_stderr =
        String::from_utf8(verify_assert.get_output().stderr.clone()).expect("verify stderr utf8");
    assert!(
        verify_stderr.contains("Usage:"),
        "retired manifest verify-signature command must fail as an absent command: {verify_stderr}"
    );
    assert!(
        !verify_stderr.contains("sorafs_cli manifest verify-signature --"),
        "usage must not advertise the retired verification command: {verify_stderr}"
    );
    oidc_probe.assert_calls(0);
    for output in [&bundle_out, &signature_out] {
        assert!(
            !output.exists(),
            "retired manifest authentication command must not write {}",
            output.display()
        );
    }
}
#[test]
fn manifest_submit_posts_payload() {
    let tempdir = tempdir().expect("tempdir");
    let (authority, private_key) = deterministic_ed25519_authority_and_private_key();
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
    let specs = chunk_fetch_plan_from_json(&plan_value)
        .expect("canonical plan")
        .chunk_fetch_specs;
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
        when.method(POST)
            .path("/v1/sorafs/pin/register")
            .header("content-type", "application/x-norito");
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
        .arg(format!("--network-id={TEST_NETWORK_ID_LITERAL}"))
        .arg(format!("--authority={authority}"))
        .arg(format!("--private-key={private_key}"))
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
    assert!(
        summary_stdout.get("submitted_epoch").is_none(),
        "client-supplied event epochs must not re-enter the signed pin request"
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
fn retired_storage_pin_subcommand_does_not_send_http() {
    let output = sorafs_cli_cmd()
        .arg("storage")
        .arg("pin")
        .output()
        .expect("command executes");
    assert!(!output.status.success());
}
#[test]
fn storage_prepare_writes_canonical_payload_and_files_manifest() {
    let tempdir = tempdir().expect("tempdir");
    let (manifest_path, _plan_path) = prepare_manifest_artifacts(tempdir.path());
    let payload_dir = tempdir.path().join("site");
    fs::create_dir_all(payload_dir.join("assets")).expect("create payload dir");
    fs::write(payload_dir.join("index.html"), "<html>hayahi</html>").expect("write index");
    fs::write(
        payload_dir.join("assets").join("app.js"),
        "console.log('hayahi');",
    )
    .expect("write script");
    let payload_out = tempdir.path().join("storage.payload.bin");
    let files_out = tempdir.path().join("storage.files.json");
    let summary_out = tempdir.path().join("storage.prepare.summary.json");
    let assert = sorafs_cli_cmd()
        .arg("storage")
        .arg("prepare")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_dir.display()))
        .arg(format!("--payload-out={}", payload_out.display()))
        .arg(format!("--files-out={}", files_out.display()))
        .arg(format!("--summary-out={}", summary_out.display()))
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value =
        norito::json::from_str(stdout.trim()).expect("storage prepare summary");
    let summary_file: Value =
        from_slice(&fs::read(&summary_out).expect("read summary")).expect("summary json");
    assert_eq!(summary_stdout, summary_file);
    assert_eq!(
        summary_stdout.get("payload_kind").and_then(Value::as_str),
        Some("directory")
    );
    assert_eq!(
        summary_stdout
            .get("payload_file_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let payload_bytes = fs::read(&payload_out).expect("read payload bytes");
    assert!(
        !payload_bytes.is_empty(),
        "prepared payload should not be empty"
    );
    let files_value: Value =
        from_slice(&fs::read(&files_out).expect("read files json")).expect("files json");
    let files = files_value
        .as_array()
        .expect("directory payload files should be an array");
    assert_eq!(files.len(), 2);
}
#[test]
fn deploy_registers_canonical_manifest_for_provider_outbox_ingest() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("site.bin");
    let payload = b"sorafs deploy payload".to_vec();
    fs::write(&payload_path, &payload).expect("write payload");
    let primary = MockServer::start();
    let (client_config, private_key) =
        write_deploy_client_config(tempdir.path(), &primary.base_url());
    let status = primary.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"current_epoch":7}"#);
    });
    let discovery = primary.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/storage/peers");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(format!(
                r#"{{"gateway_base_url":"{}"}}"#,
                primary.base_url()
            ));
    });
    let register = primary.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/pin/register")
            .header("content-type", "application/x-norito");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(
                r#"{"manifest_digest_hex":"abc","pin_fee_nano":"123","pin_fee_asset_id":"xor#universal","pin_fee_treasury_account_id":"treasury@test"}"#,
            );
    });
    let gateway = primary.mock(|when, then| {
        when.method(GET).path_matches(r"^/sorafs/cid/[^/]+$");
        then.status(200).body(payload.clone());
    });
    let summary_path = tempdir.path().join("deploy.summary.json");
    let out_dir = tempdir.path().join("deploy-out");
    let assert = sorafs_cli_cmd()
        .arg("deploy")
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--client-config={}", client_config.display()))
        .arg(format!("--out-dir={}", out_dir.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();
    status.assert_calls(1);
    discovery.assert_calls(1);
    register.assert_calls(1);
    gateway.assert_calls(1);
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(
        !stdout.contains("private_key") && !stdout.contains(&private_key),
        "deploy summary must not leak private key"
    );
    let summary: Value = norito::json::from_str(stdout.trim()).expect("deploy summary json");
    let summary_file = fs::read_to_string(&summary_path).expect("read deploy summary");
    assert!(
        !summary_file.contains("private_key") && !summary_file.contains(&private_key),
        "deploy summary file must not leak private key"
    );
    assert_eq!(summary.get("success").and_then(Value::as_bool), Some(true));
    assert_eq!(
        summary.get("payload_bytes").and_then(Value::as_u64),
        Some(payload.len() as u64)
    );
    assert!(
        summary
            .get("cid_base32_url")
            .and_then(Value::as_str)
            .is_some()
    );
    assert_eq!(
        summary
            .get("paid_pin_fee")
            .and_then(Value::as_object)
            .and_then(|fee| fee.get("pin_fee_nano"))
            .and_then(Value::as_str),
        Some("123")
    );
    assert_eq!(
        summary
            .get("provider_ingest")
            .and_then(Value::as_object)
            .and_then(|ingest| ingest.get("state"))
            .and_then(Value::as_str),
        Some("awaiting_finalized_provider_assignment")
    );
    assert_eq!(
        summary
            .get("provider_ingest")
            .and_then(Value::as_object)
            .and_then(|ingest| ingest.get("queued"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        summary
            .get("provider_ingest")
            .and_then(Value::as_object)
            .and_then(|ingest| ingest.get("direct_http_ingest"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(out_dir.join("site.bin.car").exists());
    assert!(out_dir.join("site.bin.plan.json").exists());
    assert!(out_dir.join("site.bin.manifest.to").exists());
    let checks = summary
        .get("gateway_verification")
        .and_then(|value| value.get("checks"))
        .and_then(Value::as_array)
        .expect("gateway checks");
    assert_eq!(
        checks
            .first()
            .and_then(|value| value.get("hash_ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
}
#[test]
fn deploy_accepts_known_chain_client_config_without_account_chain_discriminant() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("known-chain.bin");
    let payload = b"sorafs deploy known chain payload".to_vec();
    fs::write(&payload_path, &payload).expect("write payload");
    let primary = MockServer::start();
    let (client_config, _private_key) = write_deploy_client_config_with_chain(
        tempdir.path(),
        &primary.base_url(),
        "fc56984b-2be7-431d-840e-21514d1883f0",
    );
    let register = primary.mock(|when, then| {
        when.method(POST)
            .path("/v1/sorafs/pin/register")
            .header("content-type", "application/x-norito");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"manifest_digest_hex":"abc","pin_fee_nano":"1"}"#);
    });
    let gateway = primary.mock(|when, then| {
        when.method(GET).path_matches(r"^/sorafs/cid/[^/]+$");
        then.status(200).body(payload.clone());
    });
    let assert = sorafs_cli_cmd()
        .arg("deploy")
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--client-config={}", client_config.display()))
        .arg("--no-peer-discovery")
        .arg(format!(
            "--out-dir={}",
            tempdir.path().join("known-chain-out").display()
        ))
        .assert()
        .success();
    register.assert_calls(1);
    gateway.assert_calls(1);
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("deploy summary json");
    assert_eq!(summary.get("success").and_then(Value::as_bool), Some(true));
}
#[test]
fn deploy_falls_back_to_primary_when_peer_discovery_404() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = b"fallback deploy".to_vec();
    fs::write(&payload_path, &payload).expect("write payload");
    let primary = MockServer::start();
    let (client_config, _private_key) =
        write_deploy_client_config(tempdir.path(), &primary.base_url());
    let status = primary.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"current_epoch":8}"#);
    });
    let discovery = primary.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/storage/peers");
        then.status(404).body("not found");
    });
    let register = primary.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pin/register");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"pin_fee_nano":"1"}"#);
    });
    primary.mock(|when, then| {
        when.method(GET).path_matches(r"^/sorafs/cid/[^/]+$");
        then.status(200).body(payload.clone());
    });
    let assert = sorafs_cli_cmd()
        .arg("deploy")
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--client-config={}", client_config.display()))
        .arg(format!(
            "--out-dir={}",
            tempdir.path().join("out").display()
        ))
        .assert()
        .success();
    status.assert_calls(0);
    discovery.assert_calls(1);
    register.assert_calls(1);
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("deploy summary json");
    assert_eq!(summary.get("success").and_then(Value::as_bool), Some(true));
    assert!(
        summary
            .get("peer_discovery")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("warning"))
            .and_then(Value::as_str)
            .is_some()
    );
}
#[test]
fn deploy_does_not_fallback_or_replay_when_pin_register_route_is_unavailable() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = b"fallback transaction deploy".to_vec();
    fs::write(&payload_path, &payload).expect("write payload");
    let primary = MockServer::start();
    let (client_config, _private_key) =
        write_deploy_client_config(tempdir.path(), &primary.base_url());
    let register = primary.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pin/register");
        then.status(405).body("method not allowed");
    });
    let registry = primary.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/pin");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"attestation":{"chain_id":"fc56984b-2be7-431d-840e-21514d1883f0"}}"#);
    });
    let transaction = primary.mock(|when, then| {
        when.method(POST).path("/transaction");
        then.status(202).body("");
    });
    let pipeline = primary.mock(|when, then| {
        when.method(GET).path("/v1/pipeline/transactions/status");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"content":{"status":{"kind":"Committed","block_height":16}}}"#);
    });
    primary.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/storage/peers");
        then.status(404).body("not found");
    });
    primary.mock(|when, then| {
        when.method(GET).path_matches(r"^/sorafs/cid/[^/]+$");
        then.status(200).body(payload.clone());
    });
    let output = sorafs_cli_cmd()
        .arg("deploy")
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--client-config={}", client_config.display()))
        .arg(format!(
            "--out-dir={}",
            tempdir.path().join("fallback-out").display()
        ))
        .output()
        .expect("command executes");
    assert!(!output.status.success());
    register.assert_calls(1);
    registry.assert_calls(0);
    transaction.assert_calls(0);
    pipeline.assert_calls(0);
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("deploy summary json");
    assert_eq!(summary.get("success").and_then(Value::as_bool), Some(false));
    assert_eq!(
        summary
            .get("registration")
            .and_then(Value::as_object)
            .and_then(|registration| registration.get("success"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        summary
            .get("registration")
            .and_then(Value::as_object)
            .and_then(|registration| registration.get("error"))
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("generic transaction fallback is not supported"))
    );
}
#[test]
fn deploy_gateway_hash_mismatch_fails_even_when_length_matches() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    fs::write(&payload_path, b"abc").expect("write payload");
    let primary = MockServer::start();
    let (client_config, _private_key) =
        write_deploy_client_config(tempdir.path(), &primary.base_url());
    primary.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pin/register");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"pin_fee_nano":"1"}"#);
    });
    primary.mock(|when, then| {
        when.method(GET).path_matches(r"^/sorafs/cid/[^/]+$");
        then.status(200).body("xyz");
    });
    let output = sorafs_cli_cmd()
        .arg("deploy")
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--client-config={}", client_config.display()))
        .arg("--no-peer-discovery")
        .arg(format!(
            "--out-dir={}",
            tempdir.path().join("hash-out").display()
        ))
        .output()
        .expect("command executes");
    assert!(
        !output.status.success(),
        "deploy should fail when gateway bytes have the right length but wrong hash"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let summary: Value = norito::json::from_str(stdout.trim()).expect("deploy summary json");
    assert_eq!(summary.get("success").and_then(Value::as_bool), Some(false));
    let check = summary
        .get("gateway_verification")
        .and_then(|value| value.get("checks"))
        .and_then(Value::as_array)
        .and_then(|checks| checks.first())
        .expect("gateway check");
    assert_eq!(check.get("length_ok").and_then(Value::as_bool), Some(true));
    assert_eq!(check.get("hash_ok").and_then(Value::as_bool), Some(false));
}
#[test]
fn manifest_submit_rejects_chunk_digest_mismatch() {
    let tempdir = tempdir().expect("tempdir");
    let (authority, private_key) = deterministic_ed25519_authority_and_private_key();
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
        .arg(format!("--network-id={TEST_NETWORK_ID_LITERAL}"))
        .arg(format!("--authority={authority}"))
        .arg(format!("--private-key={private_key}"))
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
fn manifest_submit_rejects_retired_client_epoch_flags() {
    for retired in ["--submitted-epoch=7", "--resolve-submitted-epoch=true"] {
        let output = sorafs_cli_cmd()
            .arg("manifest")
            .arg("submit")
            .arg(retired)
            .output()
            .expect("command executes");
        assert!(
            !output.status.success(),
            "retired client epoch flag unexpectedly succeeded: {retired}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unrecognised option"),
            "retired client epoch flag produced an unexpected error: {stderr}"
        );
    }
    for args in [
        ["deploy", "--submitted-epoch=7", ""],
        ["manifest", "proposal", "--submitted-epoch=7"],
    ] {
        let output = sorafs_cli_cmd()
            .args(args.into_iter().filter(|arg| !arg.is_empty()))
            .output()
            .expect("command executes");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("option"),
            "retired client epoch flag produced an unexpected error: {stderr}"
        );
    }
}
#[test]
fn manifest_submit_does_not_fallback_or_replay_the_signed_body() {
    let tempdir = tempdir().expect("tempdir");
    let (authority, private_key) = deterministic_ed25519_authority_and_private_key();
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());
    let server = MockServer::start();
    let register_mock = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pin/register");
        then.status(405).body("method not allowed");
    });
    let registry_mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/pin");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"attestation":{"chain_id":"fc56984b-2be7-431d-840e-21514d1883f0"}}"#);
    });
    let tx_mock = server.mock(|when, then| {
        when.method(POST).path("/transaction");
        then.status(202).body("");
    });
    let status_mock = server.mock(|when, then| {
        when.method(GET).path("/v1/pipeline/transactions/status");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"content":{"status":{"kind":"Committed","block_height":16}}}"#);
    });
    let output = sorafs_cli_cmd()
        .arg("manifest")
        .arg("submit")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--network-id={TEST_NETWORK_ID_LITERAL}"))
        .arg(format!("--authority={authority}"))
        .arg(format!("--private-key={private_key}"))
        .output()
        .expect("command executes");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("generic transaction fallback is not supported"),
        "signed one-shot rejection produced an unexpected error: {stderr}"
    );
    register_mock.assert_calls(1);
    registry_mock.assert_calls(0);
    tx_mock.assert_calls(0);
    status_mock.assert_calls(0);
}
#[test]
fn manifest_submit_does_not_follow_307_or_308_with_the_signed_body() {
    let tempdir = tempdir().expect("tempdir");
    let (authority, private_key) = deterministic_ed25519_authority_and_private_key();
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());
    for status in [307_u16, 308_u16] {
        let server = MockServer::start();
        let register_mock = server.mock(|when, then| {
            when.method(POST).path("/v1/sorafs/pin/register");
            then.status(status)
                .header("Location", "/replayed-signed-pin")
                .body("redirect forbidden");
        });
        let replay_mock = server.mock(|when, then| {
            when.method(POST).path("/replayed-signed-pin");
            then.status(200).body("unexpected replay");
        });
        let output = sorafs_cli_cmd()
            .arg("manifest")
            .arg("submit")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg(format!("--chunk-plan={}", plan_path.display()))
            .arg(format!("--torii-url={}", server.base_url()))
            .arg(format!("--network-id={TEST_NETWORK_ID_LITERAL}"))
            .arg(format!("--authority={authority}"))
            .arg(format!("--private-key={private_key}"))
            .output()
            .expect("command executes");
        assert!(!output.status.success(), "HTTP {status} must be terminal");
        register_mock.assert_calls(1);
        replay_mock.assert_calls(0);
    }
}
#[test]
fn fetch_command_rejects_insecure_local_gateway_without_output() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..2048)
        .map(|idx| (idx as u8).wrapping_mul(19) ^ 0xA5)
        .collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json string");
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json).expect("write plan json");
    let provider_id_bytes = [0x17u8; 32];
    let provider_id_hex = hex_encode(provider_id_bytes);
    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");
    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .governance(council_signed_governance_proofs())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
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
    let chunk_specs = plan.try_chunk_fetch_specs().expect("valid CAR plan");
    let server = MockServer::start();
    let manifest_path = format!("/v1/sorafs/storage/manifest/{manifest_id_hex}");
    server.mock(|when, then| {
        when.method(GET).path(manifest_path.as_str());
        then.status(200).body(manifest_response.clone());
    });
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
        manifest_cid: hex_decode(&manifest_id_hex).expect("decode manifest id"),
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
    let gateway_public_key_hex = hex_encode(signing.verifying_key().to_bytes());
    let base_url = server.base_url();
    let provider_arg = format!(
        "name=alpha,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={},stream-token={stream_token_b64}",
        base_url
    );
    let output_path = tempdir.path().join("payload.out");
    let json_path = tempdir.path().join("fetch_summary.json");
    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg("--chunker-handle=sorafs.sf1@1.0.0")
        .arg("--telemetry-region=test-region")
        .arg("--profile=cold")
        .arg("--max-peers=1")
        .arg("--retry-budget=2")
        .arg(format!("--provider={provider_arg}"))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", json_path.display()))
        .assert();
    if base_url.starts_with("http://") {
        assert_insecure_gateway_rejected(assert, &[&output_path, &json_path]);
        return;
    }
    let assert = assert.success();
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
    assert_eq!(
        stdout_json.get("cache_state").and_then(Value::as_str),
        Some("cold")
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
    assert_eq!(
        summary_json.get("cache_profile").and_then(Value::as_str),
        Some("cold")
    );
    assert_eq!(
        summary_json.get("cache_state").and_then(Value::as_str),
        Some("cold")
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
    let specs = chunk_fetch_plan_from_json(&plan_value)
        .expect("canonical plan")
        .chunk_fetch_specs;
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
fn proof_verify_rejects_missing_or_substituted_plan_payload_digest() {
    let tempdir = tempdir().expect("tempdir");
    let input_path = tempdir.path().join("payload.bin");
    fs::write(&input_path, b"payload digest bound fetch plan").expect("write payload");
    let car_path = tempdir.path().join("payload.car");
    let plan_path = tempdir.path().join("plan.json");
    let pack_summary_path = tempdir.path().join("pack-summary.json");
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
    let canonical: Value =
        from_slice(&fs::read(&plan_path).expect("read plan")).expect("parse canonical plan");
    let mut missing = canonical.clone();
    missing
        .as_object_mut()
        .expect("plan object")
        .remove("payload_digest_blake3_hex");
    let missing_path = tempdir.path().join("missing-digest-plan.json");
    fs::write(
        &missing_path,
        to_vec(&missing).expect("encode missing plan"),
    )
    .expect("write missing plan");
    let missing_assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("verify")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--chunk-plan={}", missing_path.display()))
        .assert()
        .failure();
    let missing_stderr = String::from_utf8_lossy(&missing_assert.get_output().stderr);
    assert!(
        missing_stderr.contains("missing required `payload_digest_blake3_hex`"),
        "unexpected missing-digest failure: {missing_stderr}"
    );
    let mut substituted = canonical;
    substituted.as_object_mut().expect("plan object").insert(
        "payload_digest_blake3_hex".into(),
        Value::from("42".repeat(32)),
    );
    let substituted_path = tempdir.path().join("substituted-digest-plan.json");
    fs::write(
        &substituted_path,
        to_vec(&substituted).expect("encode substituted plan"),
    )
    .expect("write substituted plan");
    let substituted_assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("verify")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--chunk-plan={}", substituted_path.display()))
        .assert()
        .failure();
    let substituted_stderr = String::from_utf8_lossy(&substituted_assert.get_output().stderr);
    assert!(
        substituted_stderr.contains("payload digest does not match plan payload digest"),
        "unexpected substituted-digest failure: {substituted_stderr}"
    );
}
fn snapshot_id_fixture() -> [u8; 16] {
    [0x42; 16]
}
fn reputation_snapshot_fixture() -> ReputationSnapshotV1 {
    let metrics = ReputationProviderMetricsV1 {
        version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
        por_success_bps: 9_800,
        pdp_success_bps: 9_700,
        potr_success_bps: 9_600,
        latency_health_bps: 9_000,
        dispute_rate_bps: 100,
        token_violation_rate_bps: 50,
        repair_breach_rate_bps: 0,
    };
    let provider_input = |provider_id: &str| ReputationProviderInputV1 {
        version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
        provider_id: provider_id.to_string(),
        metrics,
        reserve_stage: ReputationReserveStageV1::Active,
        previous_score_bps: None,
        active_dispute: false,
        slashing_event: false,
    };
    build_reputation_snapshot(
        snapshot_id_fixture(),
        1_800_000_000,
        ReputationWeightsV1::default(),
        &[provider_input("provider-b"), provider_input("provider-a")],
        None,
    )
    .expect("reputation snapshot")
}
fn reputation_auth_args(directory: &CanonicalTempDir) -> [String; 3] {
    let keypair = KeyPair::try_from_seed(
        b"sorafs-cli-reputation-read-auth".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive reputation read fixture key");
    let account = AccountId::new(keypair.public_key().clone())
        .to_i105_for_discriminant(369)
        .expect("encode reputation read fixture account");
    let key_path = directory.path().join("reputation-read.key");
    let exposed = ExposedPrivateKey(keypair.private_key().clone()).to_string();
    fs::write(&key_path, format!("{exposed}\n")).expect("write reputation read fixture key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
            .expect("secure reputation read fixture key");
    }
    [
        format!("--network-id={TEST_NETWORK_ID_LITERAL}"),
        format!("--auth-account={account}"),
        format!("--auth-private-key-file={}", key_path.display()),
    ]
}
fn reputation_snapshot_summary_value(snapshot: &ReputationSnapshotV1) -> Value {
    let mut root = Map::new();
    root.insert(
        "snapshot_id_hex".into(),
        Value::from(hex_encode(snapshot.snapshot_id)),
    );
    root.insert(
        "generated_at_unix".into(),
        Value::from(snapshot.generated_at_unix),
    );
    root.insert(
        "provider_count".into(),
        Value::from(snapshot.providers.len() as u64),
    );
    root.insert(
        "merkle_root_hex".into(),
        Value::from(hex_encode(snapshot.merkle_root)),
    );
    root.insert("status".into(), Value::from("accepted"));
    Value::Object(root)
}
fn reputation_provider_response_value(snapshot: &ReputationSnapshotV1, provider_id: &str) -> Value {
    let provider = snapshot
        .providers
        .iter()
        .find(|entry| entry.provider_id == provider_id)
        .expect("provider should be present");
    let proof = snapshot
        .merkle_proof(provider_id)
        .expect("provider proof should build");
    let mut provider_map = Map::new();
    provider_map.insert(
        "provider_id".into(),
        Value::from(provider.provider_id.clone()),
    );
    provider_map.insert(
        "score_bps".into(),
        Value::from(u64::from(provider.score_bps)),
    );
    let mut proof_map = Map::new();
    proof_map.insert("provider_id".into(), Value::from(proof.provider_id));
    proof_map.insert(
        "leaf_index".into(),
        Value::from(u64::from(proof.leaf_index)),
    );
    proof_map.insert(
        "leaf_count".into(),
        Value::from(u64::from(proof.leaf_count)),
    );
    proof_map.insert(
        "siblings_hex".into(),
        Value::Array(
            proof
                .siblings
                .iter()
                .map(|sibling| Value::from(hex_encode(sibling)))
                .collect(),
        ),
    );
    let mut root = Map::new();
    root.insert(
        "snapshot_id_hex".into(),
        Value::from(hex_encode(snapshot.snapshot_id)),
    );
    root.insert(
        "merkle_root_hex".into(),
        Value::from(hex_encode(snapshot.merkle_root)),
    );
    root.insert("provider".into(), Value::Object(provider_map));
    root.insert("proof".into(), Value::Object(proof_map));
    Value::Object(root)
}
fn reputation_events_response_value(snapshot: &ReputationSnapshotV1) -> Value {
    let mut event = Map::new();
    event.insert("version".into(), Value::from(1_u64));
    event.insert("sequence".into(), Value::from(1_u64));
    event.insert(
        "snapshot_id_hex".into(),
        Value::from(hex_encode(snapshot.snapshot_id)),
    );
    event.insert(
        "generated_at_unix".into(),
        Value::from(snapshot.generated_at_unix),
    );
    event.insert(
        "merkle_root_hex".into(),
        Value::from(hex_encode(snapshot.merkle_root)),
    );
    event.insert(
        "provider_count".into(),
        Value::from(snapshot.providers.len() as u64),
    );
    event.insert("previous_snapshot_id_hex".into(), Value::Null);
    let mut root = Map::new();
    root.insert("since".into(), Value::from(0_u64));
    root.insert("limit".into(), Value::from(10_u64));
    root.insert("count".into(), Value::from(1_u64));
    root.insert("next_since".into(), Value::from(1_u64));
    root.insert("events".into(), Value::Array(vec![Value::Object(event)]));
    Value::Object(root)
}
#[test]
fn reputation_verify_validates_snapshot_and_merkle_proof() {
    let tempdir = tempdir().expect("tempdir");
    let snapshot_id = snapshot_id_fixture();
    let snapshot = reputation_snapshot_fixture();
    let proof = snapshot.merkle_proof("provider-a").expect("provider proof");
    let provider = snapshot
        .providers
        .iter()
        .find(|entry| entry.provider_id == "provider-a")
        .expect("provider-a should be present")
        .clone();
    let snapshot_path = tempdir.path().join("reputation-snapshot.to");
    let proof_path = tempdir.path().join("provider-a-proof.to");
    let summary_path = tempdir.path().join("reputation-summary.json");
    fs::write(
        &snapshot_path,
        to_bytes(&snapshot).expect("encode reputation snapshot"),
    )
    .expect("write reputation snapshot");
    fs::write(&proof_path, to_bytes(&proof).expect("encode proof")).expect("write proof");
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("verify")
        .arg(format!("--snapshot={}", snapshot_path.display()))
        .arg("--provider-id=provider-a")
        .arg(format!("--proof={}", proof_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value =
        norito::json::from_str(stdout.trim()).expect("reputation summary json");
    let summary_file_bytes = fs::read(&summary_path).expect("read reputation summary file");
    let summary_file: Value =
        from_slice(&summary_file_bytes).expect("reputation summary file json");
    assert_eq!(
        summary_stdout, summary_file,
        "stdout summary should match file"
    );
    assert_eq!(
        summary_stdout
            .get("snapshot_id_hex")
            .and_then(Value::as_str),
        Some(hex_encode(snapshot_id).as_str())
    );
    assert_eq!(
        summary_stdout.get("provider_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        summary_stdout.get("provider_id").and_then(Value::as_str),
        Some("provider-a")
    );
    assert_eq!(
        summary_stdout
            .get("provider_score_bps")
            .and_then(Value::as_u64),
        Some(u64::from(provider.score_bps))
    );
    assert_eq!(
        summary_stdout.get("valid").cloned(),
        Some(Value::from(true))
    );
    assert_eq!(
        summary_stdout.get("proof_verified").cloned(),
        Some(Value::from(true))
    );
}
#[test]
fn reputation_verify_missing_provider_diagnostic_is_payload_free() {
    let tempdir = tempdir().expect("tempdir");
    let snapshot = reputation_snapshot_fixture();
    let snapshot_path = tempdir.path().join("reputation-snapshot.to");
    fs::write(
        &snapshot_path,
        to_bytes(&snapshot).expect("encode reputation snapshot"),
    )
    .expect("write reputation snapshot");
    let provider_id = "provider-private-key-missing";
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("verify")
        .arg(format!("--snapshot={}", snapshot_path.display()))
        .arg(format!("--provider-id={provider_id}"))
        .arg("--proof=/runtime/missing-proof.to")
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("requested provider was not found"));
    assert!(!stderr.contains(provider_id));
    assert!(!stderr.contains("private-key"));
}
#[test]
fn reputation_verify_invalid_snapshot_diagnostic_is_payload_free() {
    let tempdir = tempdir().expect("tempdir");
    let mut snapshot = reputation_snapshot_fixture();
    let provider_id = "provider-private-key-corrupt";
    snapshot.providers[0].provider_id = provider_id.to_owned();
    let snapshot_path = tempdir.path().join("invalid-reputation-snapshot.to");
    fs::write(
        &snapshot_path,
        to_bytes(&snapshot).expect("encode invalid reputation snapshot"),
    )
    .expect("write invalid reputation snapshot");
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("verify")
        .arg(format!("--snapshot={}", snapshot_path.display()))
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("invalid reputation snapshot"));
    assert!(!stderr.contains(provider_id));
    assert!(!stderr.contains("private-key"));
}
#[test]
fn reputation_publish_command_is_retired() {
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("publish")
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("sorafs_cli reputation snapshot"));
    assert!(!stderr.contains("sorafs_cli reputation publish"));
}
#[test]
fn reputation_snapshot_fetches_latest_and_writes_output() {
    let tempdir = tempdir().expect("tempdir");
    let snapshot = reputation_snapshot_fixture();
    let output_path = tempdir.path().join("reputation-latest.json");
    let response_value = reputation_snapshot_summary_value(&snapshot);
    let response_body = to_vec(&response_value).expect("encode response");
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/reputation/latest");
        then.status(200)
            .header("content-type", "application/json")
            .body(response_body.clone());
    });
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("snapshot")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg(format!("--output={}", output_path.display()))
        .args(reputation_auth_args(&tempdir))
        .assert()
        .success();
    mock.assert();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let stdout_value: Value = norito::json::from_str(stdout.trim()).expect("stdout JSON");
    let output_value: Value =
        from_slice(&fs::read(&output_path).expect("read output")).expect("output JSON");
    for value in [&stdout_value, &output_value] {
        assert_eq!(
            value.get("provider_count").and_then(Value::as_u64),
            Some(snapshot.providers.len() as u64)
        );
    }
}
#[test]
fn reputation_fetch_outputs_provider_table_and_writes_summary() {
    let tempdir = tempdir().expect("tempdir");
    let snapshot = reputation_snapshot_fixture();
    let summary_path = tempdir.path().join("provider-summary.json");
    let response_value = reputation_provider_response_value(&snapshot, "provider-a");
    let response_body = to_vec(&response_value).expect("encode response");
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/reputation/providers/provider-a");
        then.status(200)
            .header("content-type", "application/json")
            .body(response_body.clone());
    });
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("fetch")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--provider-id=provider-a")
        .arg(format!("--summary-out={}", summary_path.display()))
        .args(reputation_auth_args(&tempdir))
        .assert()
        .success();
    mock.assert();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(stdout.contains("provider_id\tscore_bps"));
    assert!(stdout.contains("provider-a"));
    let summary_value: Value =
        from_slice(&fs::read(&summary_path).expect("read summary")).expect("summary JSON");
    assert_eq!(
        summary_value
            .get("provider")
            .and_then(Value::as_object)
            .and_then(|provider| provider.get("provider_id"))
            .and_then(Value::as_str),
        Some("provider-a")
    );
}
#[test]
fn reputation_fetch_outputs_provider_json() {
    let tempdir = tempdir().expect("tempdir");
    let snapshot = reputation_snapshot_fixture();
    let response_value = reputation_provider_response_value(&snapshot, "provider-a");
    let response_body = to_vec(&response_value).expect("encode response");
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/reputation/providers/provider-a");
        then.status(200)
            .header("content-type", "application/json")
            .body(response_body.clone());
    });
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("fetch")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--provider-id=provider-a")
        .arg("--format=json")
        .args(reputation_auth_args(&tempdir))
        .assert()
        .success();
    mock.assert();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let value: Value = norito::json::from_str(stdout.trim()).expect("stdout JSON");
    assert_eq!(
        value
            .get("provider")
            .and_then(Value::as_object)
            .and_then(|provider| provider.get("provider_id"))
            .and_then(Value::as_str),
        Some("provider-a")
    );
}
#[test]
fn reputation_watch_fetches_events_with_cursor_and_writes_summary() {
    let tempdir = tempdir().expect("tempdir");
    let snapshot = reputation_snapshot_fixture();
    let summary_path = tempdir.path().join("reputation-events.json");
    let response_value = reputation_events_response_value(&snapshot);
    let response_body = to_vec(&response_value).expect("encode response");
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sorafs/reputation/events")
            .query_param("since", "0")
            .query_param("limit", "10");
        then.status(200)
            .header("content-type", "application/json")
            .body(response_body.clone());
    });
    let assert = sorafs_cli_cmd()
        .arg("reputation")
        .arg("watch")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--since=0")
        .arg("--limit=10")
        .arg("--max-polls=1")
        .arg(format!("--summary-out={}", summary_path.display()))
        .args(reputation_auth_args(&tempdir))
        .assert()
        .success();
    mock.assert();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let value: Value = norito::json::from_str(stdout.trim()).expect("stdout JSON");
    assert_eq!(value.get("next_since").and_then(Value::as_u64), Some(1));
    let summary_value: Value =
        from_slice(&fs::read(&summary_path).expect("read summary")).expect("summary JSON");
    assert_eq!(summary_value.get("count").and_then(Value::as_u64), Some(1));
}
#[test]
fn proof_verify_accepts_chunk_plan_for_directory_payloads() {
    let tempdir = tempdir().expect("tempdir");
    let site_dir = tempdir.path().join("site");
    let assets_dir = site_dir.join("assets");
    fs::create_dir_all(&assets_dir).expect("create assets dir");
    fs::write(
        site_dir.join("index.html"),
        "<!doctype html><html><body>ok</body></html>",
    )
    .expect("write index");
    fs::write(site_dir.join("env.json"), "{\"NETWORK\":\"taira\"}\n").expect("write env");
    fs::write(assets_dir.join("app.js"), "console.log('sorafs');\n").expect("write app");
    let car_path = tempdir.path().join("site.car");
    let plan_path = tempdir.path().join("site.plan.json");
    let pack_summary_path = tempdir.path().join("site.pack.summary.json");
    sorafs_cli_cmd()
        .arg("car")
        .arg("pack")
        .arg(format!("--input={}", site_dir.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--plan-out={}", plan_path.display()))
        .arg(format!("--summary-out={}", pack_summary_path.display()))
        .assert()
        .success();
    let manifest_path = tempdir.path().join("site.manifest.to");
    sorafs_cli_cmd()
        .arg("manifest")
        .arg("build")
        .arg(format!("--summary={}", pack_summary_path.display()))
        .arg(format!("--manifest-out={}", manifest_path.display()))
        .assert()
        .success();
    let verify_summary_path = tempdir.path().join("site.verify.summary.json");
    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("verify")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg(format!("--summary-out={}", verify_summary_path.display()))
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let summary_stdout: Value = norito::json::from_str(stdout.trim()).expect("verify summary json");
    let plan_value: Value =
        from_slice(&fs::read(&plan_path).expect("read plan")).expect("plan json");
    let specs = chunk_fetch_plan_from_json(&plan_value)
        .expect("canonical plan")
        .chunk_fetch_specs;
    assert_eq!(
        summary_stdout
            .get("chunk_plan_source")
            .and_then(Value::as_str),
        Some(plan_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        summary_stdout
            .get("chunk_plan_chunk_count")
            .and_then(Value::as_u64),
        Some(specs.len() as u64)
    );
}
fn governance_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/sorafs_manifest/governance")
}
fn governance_fixture_node_cid_display() -> String {
    let bytes = fs::read(governance_fixture_root().join("node_v1.to"))
        .expect("read governance node fixture");
    let node: GovernanceLogNodeV1 =
        decode_from_bytes(&bytes).expect("decode governance node fixture");
    match std::str::from_utf8(&node.node_cid) {
        Ok(value)
            if !value.is_empty() && value.chars().all(|character| !character.is_control()) =>
        {
            value.to_owned()
        }
        _ => format!("hex:{}", hex_encode(node.node_cid)),
    }
}
fn parse_cli_json_stdout(output: &[u8]) -> Value {
    from_slice(output).expect("CLI stdout should be JSON")
}
fn governance_dag_build_key_hex() -> String {
    "cd".repeat(32)
}
fn write_governance_dag_provenance_node(root: &Path) -> (PathBuf, String) {
    let account_keypair = KeyPair::try_from_seed(
        b"sorafs-cli-governance-provenance-account".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture governance provenance account key");
    let publisher_account = AccountId::new(account_keypair.public_key().clone());
    let publisher_account_digest_hex = hex_encode(governance_dag_submission_account_digest_v1(
        &publisher_account.encode(),
    ));
    let report = SoraFsAppealFinanceReportV1 {
        version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        report_id: [0x42; 16],
        case_id: "case-42".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_700_000_031_000,
        appeal_finance_config_version: "baseline-v1".to_string(),
        evidence_bundle_digest: Some([0xA7; 32]),
        outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
        deposit_xor: "420".parse().expect("canonical XOR quantity"),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "refund-account".to_string(),
            amount_xor: "420".parse().expect("canonical XOR quantity"),
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "treasury-account".to_string(),
            amount_xor: "50".parse().expect("canonical XOR quantity"),
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "escrow-account".to_string(),
            amount_xor: "0".parse().expect("canonical XOR quantity"),
        },
        panel_size: 3,
        panel_reward_total_xor: "85".parse().expect("canonical XOR quantity"),
        rewards_paid_total_xor: "60".parse().expect("canonical XOR quantity"),
        rewards_forfeited_treasury_xor: "25".parse().expect("canonical XOR quantity"),
        juror_payouts: vec![
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-a".to_string(),
                stipend_xor: "25".parse().expect("canonical XOR quantity"),
                bonus_xor: "5".parse().expect("canonical XOR quantity"),
                total_xor: "30".parse().expect("canonical XOR quantity"),
            },
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-b".to_string(),
                stipend_xor: "25".parse().expect("canonical XOR quantity"),
                bonus_xor: "5".parse().expect("canonical XOR quantity"),
                total_xor: "30".parse().expect("canonical XOR quantity"),
            },
        ],
        no_show_juror_ids: vec!["juror-c".to_string()],
    };
    let signer = SigningKey::from_bytes(&[0xA5; 32]);
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: Vec::new(),
        prev_cid: None,
        timestamp: 1_700_000_031,
        publisher_peer_id: b"12D3KooWGovernanceProvenance".to_vec(),
        submission_provenance: Some(GovernanceDagSubmissionProvenanceV1 {
            publisher_account_digest: governance_dag_submission_account_digest_v1(
                &publisher_account.encode(),
            ),
            origin: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
        }),
        payload: GovernanceLogPayloadV1::AppealFinanceReport(report),
        publisher_signature: GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        },
    };
    node.node_cid = node
        .recompute_node_cid()
        .expect("derive governance node CID");
    let signature = signer.sign(
        &node
            .signature_payload_bytes()
            .expect("encode governance node signature payload"),
    );
    node.publisher_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signer.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    node.validate().expect("validate provenance-bearing node");
    node.verify_publisher_signature()
        .expect("verify provenance-bearing node signature");
    let path = root.join("finance-report.to");
    fs::write(
        &path,
        to_bytes(&node).expect("encode provenance-bearing node"),
    )
    .expect("write provenance-bearing node");
    (path, publisher_account_digest_hex)
}
fn assert_governance_submission_summary(value: &Value, publisher_account_digest_hex: &str) {
    assert_eq!(
        value
            .get("submission_publisher_account_digest_hex")
            .and_then(Value::as_str),
        Some(publisher_account_digest_hex),
        "signed submission account digest should be preserved in {value:?}"
    );
    assert_eq!(
        value.get("submission_origin").and_then(Value::as_str),
        Some("appeal_finance_report"),
        "signed submission origin should be preserved in {value:?}"
    );
}
fn build_governance_dag_fixture_archive(build_dir: &Path, summary_path: Option<&Path>) -> Value {
    let root = governance_fixture_root();
    let key_hex = governance_dag_build_key_hex();
    let mut command = sorafs_cli_cmd();
    command
        .arg("governance")
        .arg("dag")
        .arg("build")
        .arg(format!("--root={}", root.display()))
        .arg(format!("--out={}", build_dir.display()))
        .arg("--publisher-peer-id=12D3KooWGovernanceDagBuilder")
        .arg(format!("--key-hex={key_hex}"))
        .arg("--generated-at=1700000999");
    if let Some(path) = summary_path {
        command.arg(format!("--summary-out={}", path.display()));
    }
    let build_assert = command.assert().success();
    parse_cli_json_stdout(&build_assert.get_output().stdout)
}
#[test]
fn governance_dag_list_and_show_validate_fixture() {
    let root = governance_fixture_root();
    let node = root.join("node_v1.to");
    let node_cid = governance_fixture_node_cid_display();
    let list_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("list")
        .arg(format!("--root={}", root.display()))
        .arg("--format=json")
        .assert()
        .success();
    let list_json = parse_cli_json_stdout(&list_assert.get_output().stdout);
    assert_eq!(
        list_json.get("artifact_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(list_json.get("node_count").and_then(Value::as_u64), Some(1));
    assert_eq!(
        list_json.get("valid_node_count").and_then(Value::as_u64),
        Some(1)
    );
    let artifacts = list_json
        .get("artifacts")
        .and_then(Value::as_array)
        .expect("artifacts");
    assert_eq!(
        artifacts[0]
            .get("validation_status")
            .and_then(Value::as_str),
        Some("Ok")
    );
    assert_eq!(
        artifacts[0]
            .get("node")
            .and_then(|node| node.get("payload_kind"))
            .and_then(Value::as_str),
        Some("por_proof")
    );
    let listed_node = artifacts[0].get("node").expect("listed node summary");
    assert!(
        listed_node
            .get("submission_publisher_account_digest_hex")
            .is_some_and(Value::is_null),
        "internally produced node must expose an explicit null submission account digest: {listed_node:?}"
    );
    assert!(
        listed_node
            .get("submission_origin")
            .is_some_and(Value::is_null),
        "internally produced node must expose an explicit null submission origin: {listed_node:?}"
    );
    let show_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("show")
        .arg(format!("--node={}", node.display()))
        .arg("--format=json")
        .assert()
        .success();
    let show_json = parse_cli_json_stdout(&show_assert.get_output().stdout);
    assert_eq!(
        show_json.get("validation_code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        show_json
            .get("node")
            .and_then(|node| node.get("node_cid"))
            .and_then(Value::as_str),
        Some(node_cid.as_str())
    );
    let shown_node = show_json.get("node").expect("shown node summary");
    assert!(
        shown_node
            .get("submission_publisher_account_digest_hex")
            .is_some_and(Value::is_null),
        "internally produced node must expose an explicit null submission account digest: {shown_node:?}"
    );
    assert!(
        shown_node
            .get("submission_origin")
            .is_some_and(Value::is_null),
        "internally produced node must expose an explicit null submission origin: {shown_node:?}"
    );
}
#[test]
fn governance_dag_cli_preserves_signed_submission_provenance() {
    let tempdir = tempdir().expect("tempdir");
    let source_root = tempdir.path().join("source");
    fs::create_dir(&source_root).expect("create source root");
    let (node_path, publisher_account_digest_hex) =
        write_governance_dag_provenance_node(&source_root);
    let list_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("list")
        .arg(format!("--root={}", source_root.display()))
        .arg("--format=json")
        .assert()
        .success();
    let list_json = parse_cli_json_stdout(&list_assert.get_output().stdout);
    let listed_node = list_json
        .get("artifacts")
        .and_then(Value::as_array)
        .and_then(|artifacts| artifacts.first())
        .and_then(|artifact| artifact.get("node"))
        .expect("listed provenance-bearing node");
    assert_governance_submission_summary(listed_node, &publisher_account_digest_hex);
    let show_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("show")
        .arg(format!("--node={}", node_path.display()))
        .arg("--format=json")
        .assert()
        .success();
    let show_json = parse_cli_json_stdout(&show_assert.get_output().stdout);
    let shown_node = show_json
        .get("node")
        .expect("shown provenance-bearing node");
    assert_governance_submission_summary(shown_node, &publisher_account_digest_hex);
    let build_root = tempdir.path().join("build");
    let build_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("build")
        .arg(format!("--root={}", source_root.display()))
        .arg(format!("--out={}", build_root.display()))
        .arg("--publisher-peer-id=12D3KooWGovernanceDagBuilder")
        .arg(format!("--key-hex={}", governance_dag_build_key_hex()))
        .arg("--generated-at=1700000999")
        .assert()
        .success();
    let build_json = parse_cli_json_stdout(&build_assert.get_output().stdout);
    let built_block = build_json
        .get("blocks")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .expect("built provenance-bearing block summary");
    assert_governance_submission_summary(built_block, &publisher_account_digest_hex);
    let verify_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("verify-build")
        .arg(format!("--root={}", build_root.display()))
        .arg("--require-sidecars")
        .assert()
        .success();
    let verify_json = parse_cli_json_stdout(&verify_assert.get_output().stdout);
    let verified_block = verify_json
        .get("blocks")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .expect("verified provenance-bearing block summary");
    assert_governance_submission_summary(verified_block, &publisher_account_digest_hex);
}
#[test]
fn governance_dag_verify_rejects_unexpected_head() {
    let root = governance_fixture_root();
    let assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("verify")
        .arg(format!("--root={}", root.display()))
        .arg("--require-chain")
        .arg("--head-cid=wrong-head")
        .assert()
        .failure();
    let json = parse_cli_json_stdout(&assert.get_output().stdout);
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    let errors = json
        .get("errors")
        .and_then(Value::as_array)
        .expect("errors");
    assert!(
        errors
            .iter()
            .any(|error| error.get("kind").and_then(Value::as_str) == Some("head_cid")),
        "expected head_cid failure in {errors:?}"
    );
}
#[test]
fn governance_dag_verify_and_export_fixture_archive() {
    let root = governance_fixture_root();
    let head_cid = governance_fixture_node_cid_display();
    let verify_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("verify")
        .arg(format!("--root={}", root.display()))
        .arg(format!("--head-cid={head_cid}"))
        .assert()
        .success();
    let verify_json = parse_cli_json_stdout(&verify_assert.get_output().stdout);
    assert_eq!(verify_json.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        verify_json
            .get("head_cids")
            .and_then(Value::as_array)
            .and_then(|heads| heads.first())
            .and_then(Value::as_str),
        Some(head_cid.as_str())
    );
    let tempdir = tempdir().expect("tempdir");
    let export_dir = tempdir.path().join("export");
    let export_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("export")
        .arg(format!("--root={}", root.display()))
        .arg(format!("--out={}", export_dir.display()))
        .arg(format!("--head-cid={head_cid}"))
        .assert()
        .success();
    let export_json = parse_cli_json_stdout(&export_assert.get_output().stdout);
    assert_eq!(
        export_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.export.v1")
    );
    let exported_node = export_dir.join("nodes/node_v1.to");
    let exported_sidecar = export_dir.join("nodes/node_v1.to.blake3");
    assert!(exported_node.exists(), "exported node missing");
    assert!(exported_sidecar.exists(), "exported sidecar missing");
    let sidecar = fs::read_to_string(&exported_sidecar).expect("read sidecar");
    let exported_bytes = fs::read(&exported_node).expect("read exported node");
    assert_eq!(
        sidecar.trim(),
        hex_encode(blake3_hash(&exported_bytes).as_bytes())
    );
    assert!(
        export_dir.join("manifest.json").exists(),
        "manifest missing"
    );
}
#[test]
fn governance_dag_build_fixture_archive_writes_signed_blocks_and_head() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let summary_path = tempdir.path().join("build-summary.json");
    let key_hex = governance_dag_build_key_hex();
    let build_json = build_governance_dag_fixture_archive(&build_dir, Some(&summary_path));
    assert_eq!(
        build_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.build.v1")
    );
    assert_eq!(
        build_json.get("block_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        build_json.get("generated_at").and_then(Value::as_u64),
        Some(1_700_000_999)
    );
    let head_path = build_dir.join("head.to");
    let head_sidecar = build_dir.join("head.to.blake3");
    assert!(head_path.exists(), "head.to missing");
    assert!(head_sidecar.exists(), "head sidecar missing");
    let head_bytes = fs::read(&head_path).expect("read generated head");
    assert_eq!(
        fs::read_to_string(&head_sidecar)
            .expect("read head sidecar")
            .trim(),
        hex_encode(blake3_hash(&head_bytes).as_bytes())
    );
    let head: GovernanceDagHeadV1 =
        decode_from_bytes(&head_bytes).expect("decode generated governance DAG head");
    assert_eq!(
        head.head_signature.algorithm,
        GovernanceSignatureAlgorithm::Ed25519
    );
    assert_eq!(head.block_count, 1);
    let blocks = build_json
        .get("blocks")
        .and_then(Value::as_array)
        .expect("blocks");
    let block_path = build_dir.join(
        blocks[0]
            .get("path")
            .and_then(Value::as_str)
            .expect("block path"),
    );
    let block_bytes = fs::read(&block_path).expect("read generated block");
    let block: GovernanceDagBlockV1 =
        decode_from_bytes(&block_bytes).expect("decode generated governance DAG block");
    assert_eq!(
        block.block_signature.algorithm,
        GovernanceSignatureAlgorithm::Ed25519
    );
    validate_governance_dag_head_against_chain_v1(&head, &[block])
        .expect("generated governance DAG block/head chain validates");
    let manifest = fs::read_to_string(build_dir.join("manifest.json")).expect("read manifest");
    assert!(
        !manifest.contains(&key_hex),
        "runtime signing seed must not be persisted"
    );
    let summary = fs::read_to_string(&summary_path).expect("read summary");
    assert_eq!(manifest, summary);
}
#[test]
fn governance_dag_build_fixture_archive_writes_car_segment() {
    let root = governance_fixture_root();
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let car_path = tempdir.path().join("governance-dag.car");
    let car_plan_path = tempdir.path().join("governance-dag-plan.json");
    let key_hex = governance_dag_build_key_hex();
    let build_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("build")
        .arg(format!("--root={}", root.display()))
        .arg(format!("--out={}", build_dir.display()))
        .arg("--publisher-peer-id=12D3KooWGovernanceDagBuilder")
        .arg(format!("--key-hex={key_hex}"))
        .arg("--generated-at=1700000999")
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--car-plan-out={}", car_plan_path.display()))
        .assert()
        .success();
    let build_json = parse_cli_json_stdout(&build_assert.get_output().stdout);
    let car_summary = build_json
        .get("car_archive")
        .and_then(Value::as_object)
        .expect("car archive summary");
    assert_eq!(
        car_summary.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.car.v1")
    );
    assert_eq!(
        car_summary.get("output_car").and_then(Value::as_str),
        Some(car_path.to_str().expect("utf8 car path"))
    );
    assert_eq!(
        car_summary.get("chunk_plan_path").and_then(Value::as_str),
        Some(car_plan_path.to_str().expect("utf8 car plan path"))
    );
    assert_eq!(
        car_summary.get("file_count").and_then(Value::as_u64),
        Some(4)
    );
    let car_bytes = fs::read(&car_path).expect("read governance DAG CAR");
    assert_eq!(
        car_summary.get("car_size").and_then(Value::as_u64),
        Some(car_bytes.len() as u64)
    );
    let car_digest_hex = hex_encode(blake3_hash(&car_bytes).as_bytes());
    assert_eq!(
        car_summary.get("car_digest_hex").and_then(Value::as_str),
        Some(car_digest_hex.as_str())
    );
    let plan_bytes = fs::read(&car_plan_path).expect("read governance DAG CAR plan");
    let plan_json: Value = from_slice(&plan_bytes).expect("chunk plan json");
    let canonical_plan =
        chunk_fetch_plan_from_json(&plan_json).expect("canonical governance DAG CAR plan");
    assert!(
        !canonical_plan.chunk_fetch_specs.is_empty(),
        "CAR chunk plan should contain at least one chunk: {plan_json:?}"
    );
    let files = car_summary
        .get("files")
        .and_then(Value::as_array)
        .expect("car files");
    let file_paths = files
        .iter()
        .map(|file| {
            file.get("path")
                .and_then(Value::as_str)
                .expect("file path")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(file_paths.iter().any(|path| path == "head.to"));
    assert!(file_paths.iter().any(|path| path == "head.to.blake3"));
    assert!(
        file_paths
            .iter()
            .any(|path| path.starts_with("blocks/") && path.ends_with(".to"))
    );
    assert!(
        file_paths
            .iter()
            .any(|path| path.starts_with("blocks/") && path.ends_with(".to.blake3"))
    );
}
#[test]
fn governance_dag_verify_build_accepts_generated_blocks_and_head() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let summary_path = tempdir.path().join("verify-build-summary.json");
    let build_json = build_governance_dag_fixture_archive(&build_dir, None);
    let head_cid_hex = build_json
        .get("head_block_cid_hex")
        .and_then(Value::as_str)
        .expect("head cid hex");
    let verify_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("verify-build")
        .arg(format!("--root={}", build_dir.display()))
        .arg("--require-sidecars")
        .arg(format!("--head-cid=hex:{head_cid_hex}"))
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();
    let verify_json = parse_cli_json_stdout(&verify_assert.get_output().stdout);
    assert_eq!(
        verify_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.build.verify.v1")
    );
    assert_eq!(verify_json.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        verify_json.get("block_file_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        verify_json.get("block_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        verify_json
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str),
        Some(head_cid_hex)
    );
    assert!(
        verify_json
            .get("warnings")
            .and_then(Value::as_array)
            .is_some_and(|warnings| warnings.is_empty()),
        "generated snapshot should have no warnings: {verify_json:?}"
    );
    let summary = fs::read_to_string(&summary_path).expect("read verify-build summary");
    let summary_json: Value = from_slice(summary.as_bytes()).expect("summary json");
    assert_eq!(summary_json, verify_json);
}
#[test]
fn governance_dag_verify_build_rejects_tampered_block_snapshot() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let build_json = build_governance_dag_fixture_archive(&build_dir, None);
    let blocks = build_json
        .get("blocks")
        .and_then(Value::as_array)
        .expect("blocks");
    let block_path = build_dir.join(
        blocks[0]
            .get("path")
            .and_then(Value::as_str)
            .expect("block path"),
    );
    let block_bytes = fs::read(&block_path).expect("read generated block");
    let mut block: GovernanceDagBlockV1 =
        decode_from_bytes(&block_bytes).expect("decode generated governance DAG block");
    block.block_cid[0] ^= 0x55;
    let tampered_bytes = to_bytes(&block).expect("encode tampered block");
    fs::write(&block_path, &tampered_bytes).expect("write tampered block");
    fs::write(
        block_path.with_extension("to.blake3"),
        format!("{}\n", hex_encode(blake3_hash(&tampered_bytes).as_bytes())),
    )
    .expect("write tampered sidecar");
    let verify_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("verify-build")
        .arg(format!("--root={}", build_dir.display()))
        .arg("--require-sidecars")
        .assert()
        .failure();
    let verify_json = parse_cli_json_stdout(&verify_assert.get_output().stdout);
    assert_eq!(verify_json.get("ok").and_then(Value::as_bool), Some(false));
    let errors = verify_json
        .get("errors")
        .and_then(Value::as_array)
        .expect("errors");
    assert!(
        errors
            .iter()
            .any(|error| error.get("kind").and_then(Value::as_str) == Some("head_chain")),
        "expected head_chain failure in {errors:?}"
    );
}
#[test]
fn governance_dag_rebuild_head_recreates_signed_head_from_blocks() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let rebuilt_head_path = tempdir.path().join("rebuilt-head.to");
    let summary_path = tempdir.path().join("rebuild-head-summary.json");
    let build_json = build_governance_dag_fixture_archive(&build_dir, None);
    let key_hex = governance_dag_build_key_hex();
    let rebuild_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("rebuild-head")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--head-out={}", rebuilt_head_path.display()))
        .arg("--publisher-peer-id=12D3KooWGovernanceDagBuilder")
        .arg(format!("--key-hex={key_hex}"))
        .arg("--generated-at=1700000999")
        .arg("--require-sidecars")
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();
    let rebuild_json = parse_cli_json_stdout(&rebuild_assert.get_output().stdout);
    assert_eq!(
        rebuild_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.head.rebuild.v1")
    );
    assert_eq!(
        rebuild_json.get("block_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rebuild_json
            .get("head_block_cid_hex")
            .and_then(Value::as_str),
        build_json.get("head_block_cid_hex").and_then(Value::as_str)
    );
    let original_head = fs::read(build_dir.join("head.to")).expect("read original head");
    let rebuilt_head = fs::read(&rebuilt_head_path).expect("read rebuilt head");
    assert_eq!(
        rebuilt_head, original_head,
        "same blocks, signer, and timestamp should rebuild identical head bytes"
    );
    assert_eq!(
        fs::read_to_string(rebuilt_head_path.with_extension("to.blake3"))
            .expect("read rebuilt head sidecar")
            .trim(),
        hex_encode(blake3_hash(&rebuilt_head).as_bytes())
    );
    let summary = fs::read_to_string(&summary_path).expect("read rebuild summary");
    let summary_json: Value = from_slice(summary.as_bytes()).expect("summary json");
    assert_eq!(summary_json, rebuild_json);
}
#[test]
fn governance_dag_rebuild_head_rejects_tampered_block_snapshot() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let rebuilt_head_path = tempdir.path().join("rebuilt-head.to");
    let build_json = build_governance_dag_fixture_archive(&build_dir, None);
    let blocks = build_json
        .get("blocks")
        .and_then(Value::as_array)
        .expect("blocks");
    let block_path = build_dir.join(
        blocks[0]
            .get("path")
            .and_then(Value::as_str)
            .expect("block path"),
    );
    let block_bytes = fs::read(&block_path).expect("read generated block");
    let mut block: GovernanceDagBlockV1 =
        decode_from_bytes(&block_bytes).expect("decode generated governance DAG block");
    block.block_cid[0] ^= 0x33;
    let tampered_bytes = to_bytes(&block).expect("encode tampered block");
    fs::write(&block_path, &tampered_bytes).expect("write tampered block");
    fs::write(
        block_path.with_extension("to.blake3"),
        format!("{}\n", hex_encode(blake3_hash(&tampered_bytes).as_bytes())),
    )
    .expect("write tampered sidecar");
    sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("rebuild-head")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--head-out={}", rebuilt_head_path.display()))
        .arg("--publisher-peer-id=12D3KooWGovernanceDagBuilder")
        .arg(format!("--key-hex={}", governance_dag_build_key_hex()))
        .arg("--generated-at=1700000999")
        .arg("--require-sidecars")
        .assert()
        .failure();
    assert!(
        !rebuilt_head_path.exists(),
        "rebuild-head must not write a head for invalid blocks"
    );
}
#[test]
fn governance_dag_mirror_build_and_query_generated_snapshot() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let index_path = tempdir.path().join("mirror-index.json");
    let build_json = build_governance_dag_fixture_archive(&build_dir, None);
    let head_cid_hex = build_json
        .get("head_block_cid_hex")
        .and_then(Value::as_str)
        .expect("head cid hex");
    let mirror_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-build")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", index_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--head-cid=hex:{head_cid_hex}"))
        .assert()
        .success();
    let mirror_json = parse_cli_json_stdout(&mirror_assert.get_output().stdout);
    assert_eq!(
        mirror_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.mirror.v1")
    );
    assert_eq!(
        mirror_json.get("block_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        mirror_json
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str),
        Some(head_cid_hex)
    );
    let disk_index = fs::read_to_string(&index_path).expect("read mirror index");
    let disk_index_json: Value = from_slice(disk_index.as_bytes()).expect("disk mirror index json");
    assert_eq!(disk_index_json, mirror_json);
    let block = mirror_json
        .get("blocks")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .expect("indexed block");
    let block_cid_hex = block
        .get("block_cid_hex")
        .and_then(Value::as_str)
        .expect("block cid hex");
    let node_cid_hex = block
        .get("node_cid_hex")
        .and_then(Value::as_str)
        .expect("node cid hex");
    let head_query = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-query")
        .arg(format!("--index={}", index_path.display()))
        .arg("--head")
        .arg("--format=json")
        .assert()
        .success();
    let head_query_json = parse_cli_json_stdout(&head_query.get_output().stdout);
    assert_eq!(
        head_query_json.get("found").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        head_query_json
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str),
        Some(head_cid_hex)
    );
    let block_query = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-query")
        .arg(format!("--index={}", index_path.display()))
        .arg(format!("--block-cid=hex:{block_cid_hex}"))
        .arg("--format=json")
        .assert()
        .success();
    let block_query_json = parse_cli_json_stdout(&block_query.get_output().stdout);
    assert_eq!(
        block_query_json.get("found").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        block_query_json
            .get("block")
            .and_then(|block| block.get("block_cid_hex"))
            .and_then(Value::as_str),
        Some(block_cid_hex)
    );
    let node_query = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-query")
        .arg(format!("--index={}", index_path.display()))
        .arg(format!("--node-cid=hex:{node_cid_hex}"))
        .arg("--format=json")
        .assert()
        .success();
    let node_query_json = parse_cli_json_stdout(&node_query.get_output().stdout);
    assert_eq!(
        node_query_json.get("found").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        node_query_json
            .get("block")
            .and_then(|block| block.get("node_cid_hex"))
            .and_then(Value::as_str),
        Some(node_cid_hex)
    );
}
#[test]
fn governance_dag_mirror_query_rejects_unknown_block_cid() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let index_path = tempdir.path().join("mirror-index.json");
    build_governance_dag_fixture_archive(&build_dir, None);
    sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-build")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", index_path.display()))
        .assert()
        .success();
    let missing_query = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-query")
        .arg(format!("--index={}", index_path.display()))
        .arg(format!("--block-cid=hex:{}", "00".repeat(32)))
        .arg("--format=json")
        .assert()
        .failure();
    let missing_json = parse_cli_json_stdout(&missing_query.get_output().stdout);
    assert_eq!(
        missing_json.get("found").and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        matches!(
            missing_json.get("block"),
            Some(value) if matches!(value, Value::Null)
        ),
        "missing block query should return a null block: {missing_json:?}"
    );
}
#[test]
fn governance_dag_checkpoint_packages_verified_snapshot_artifacts() {
    let root = governance_fixture_root();
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let car_path = tempdir.path().join("governance-dag.car");
    let car_plan_path = tempdir.path().join("governance-dag-plan.json");
    let index_path = tempdir.path().join("mirror-index.json");
    let checkpoint_path = tempdir.path().join("checkpoint.json");
    let key_hex = governance_dag_build_key_hex();
    let build_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("build")
        .arg(format!("--root={}", root.display()))
        .arg(format!("--out={}", build_dir.display()))
        .arg("--publisher-peer-id=12D3KooWGovernanceDagBuilder")
        .arg(format!("--key-hex={key_hex}"))
        .arg("--generated-at=1700000999")
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--car-plan-out={}", car_plan_path.display()))
        .assert()
        .success();
    let build_json = parse_cli_json_stdout(&build_assert.get_output().stdout);
    let head_cid_hex = build_json
        .get("head_block_cid_hex")
        .and_then(Value::as_str)
        .expect("head cid hex");
    sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-build")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", index_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--head-cid=hex:{head_cid_hex}"))
        .assert()
        .success();
    let checkpoint_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("checkpoint")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", checkpoint_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--head-cid=hex:{head_cid_hex}"))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--mirror-index={}", index_path.display()))
        .arg("--generated-at=1700001999")
        .assert()
        .success();
    let checkpoint_json = parse_cli_json_stdout(&checkpoint_assert.get_output().stdout);
    assert_eq!(
        checkpoint_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.checkpoint.v1")
    );
    assert_eq!(
        checkpoint_json.get("generated_at").and_then(Value::as_u64),
        Some(1_700_001_999)
    );
    assert_eq!(
        checkpoint_json
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str),
        Some(head_cid_hex)
    );
    assert_eq!(
        checkpoint_json
            .get("verification")
            .and_then(|verification| verification.get("ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
    let car_bytes = fs::read(&car_path).expect("read checkpoint CAR");
    let car_digest_hex = hex_encode(blake3_hash(&car_bytes).as_bytes());
    assert_eq!(
        checkpoint_json
            .get("car_archive")
            .and_then(|car| car.get("car_size"))
            .and_then(Value::as_u64),
        Some(car_bytes.len() as u64)
    );
    assert_eq!(
        checkpoint_json
            .get("car_archive")
            .and_then(|car| car.get("blake3"))
            .and_then(Value::as_str),
        Some(car_digest_hex.as_str())
    );
    let index_bytes = fs::read(&index_path).expect("read checkpoint mirror index");
    let index_digest_hex = hex_encode(blake3_hash(&index_bytes).as_bytes());
    assert_eq!(
        checkpoint_json
            .get("mirror_index")
            .and_then(|index| index.get("schema"))
            .and_then(Value::as_str),
        Some("sorafs.governance_dag.mirror.v1")
    );
    assert_eq!(
        checkpoint_json
            .get("mirror_index")
            .and_then(|index| index.get("blake3"))
            .and_then(Value::as_str),
        Some(index_digest_hex.as_str())
    );
    let disk_checkpoint = fs::read_to_string(&checkpoint_path).expect("read checkpoint file");
    let disk_checkpoint_json: Value =
        from_slice(disk_checkpoint.as_bytes()).expect("checkpoint json");
    assert_eq!(disk_checkpoint_json, checkpoint_json);
}
#[test]
fn governance_dag_checkpoint_rejects_bad_mirror_index_schema() {
    let tempdir = tempdir().expect("tempdir");
    let build_dir = tempdir.path().join("build");
    let index_path = tempdir.path().join("mirror-index.json");
    let checkpoint_path = tempdir.path().join("checkpoint.json");
    build_governance_dag_fixture_archive(&build_dir, None);
    fs::write(&index_path, r#"{"schema":"not.sorafs"}"#).expect("write bad mirror index");
    sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("checkpoint")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", checkpoint_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--mirror-index={}", index_path.display()))
        .assert()
        .failure();
    assert!(
        !checkpoint_path.exists(),
        "checkpoint must not be written when mirror index schema is invalid"
    );
}
fn build_governance_dag_checkpoint_fixture(
    base: &Path,
) -> (PathBuf, PathBuf, PathBuf, PathBuf, String) {
    let root = governance_fixture_root();
    let build_dir = base.join("build");
    let car_path = base.join("governance-dag.car");
    let car_plan_path = base.join("governance-dag-plan.json");
    let index_path = base.join("mirror-index.json");
    let checkpoint_path = base.join("checkpoint.json");
    let key_hex = governance_dag_build_key_hex();
    let build_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("build")
        .arg(format!("--root={}", root.display()))
        .arg(format!("--out={}", build_dir.display()))
        .arg("--publisher-peer-id=12D3KooWGovernanceDagBuilder")
        .arg(format!("--key-hex={key_hex}"))
        .arg("--generated-at=1700000999")
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--car-plan-out={}", car_plan_path.display()))
        .assert()
        .success();
    let build_json = parse_cli_json_stdout(&build_assert.get_output().stdout);
    let head_cid_hex = build_json
        .get("head_block_cid_hex")
        .and_then(Value::as_str)
        .expect("head cid hex")
        .to_string();
    sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("mirror-build")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", index_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--head-cid=hex:{head_cid_hex}"))
        .assert()
        .success();
    sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("checkpoint")
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", checkpoint_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--head-cid=hex:{head_cid_hex}"))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--mirror-index={}", index_path.display()))
        .arg("--generated-at=1700001999")
        .assert()
        .success();
    (
        build_dir,
        car_path,
        index_path,
        checkpoint_path,
        head_cid_hex,
    )
}
#[test]
fn governance_dag_checkpoint_verify_accepts_recorded_artifacts() {
    let tempdir = tempdir().expect("tempdir");
    let (build_dir, car_path, index_path, checkpoint_path, head_cid_hex) =
        build_governance_dag_checkpoint_fixture(tempdir.path());
    let summary_path = tempdir.path().join("checkpoint-verify-summary.json");
    let verify_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("checkpoint-verify")
        .arg(format!("--checkpoint={}", checkpoint_path.display()))
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--mirror-index={}", index_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();
    let verify_json = parse_cli_json_stdout(&verify_assert.get_output().stdout);
    assert_eq!(
        verify_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.checkpoint.verify.v1")
    );
    assert_eq!(verify_json.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        verify_json
            .get("expected_head_cid_hex")
            .and_then(Value::as_str),
        Some(head_cid_hex.as_str())
    );
    assert_eq!(
        verify_json
            .get("root")
            .and_then(|root| root.get("ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        verify_json
            .get("head")
            .and_then(|head| head.get("digest_ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        verify_json
            .get("car_archive")
            .and_then(|car| car.get("ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        verify_json
            .get("mirror_index")
            .and_then(|index| index.get("ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
    let summary = fs::read_to_string(&summary_path).expect("read checkpoint verify summary");
    let summary_json: Value = from_slice(summary.as_bytes()).expect("checkpoint verify json");
    assert_eq!(summary_json, verify_json);
}
#[test]
fn governance_dag_checkpoint_verify_rejects_tampered_car() {
    let tempdir = tempdir().expect("tempdir");
    let (build_dir, car_path, index_path, checkpoint_path, _) =
        build_governance_dag_checkpoint_fixture(tempdir.path());
    fs::write(&car_path, b"tampered governance dag car").expect("tamper CAR");
    let verify_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("checkpoint-verify")
        .arg(format!("--checkpoint={}", checkpoint_path.display()))
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg(format!("--mirror-index={}", index_path.display()))
        .arg("--require-sidecars")
        .assert()
        .failure();
    let verify_json = parse_cli_json_stdout(&verify_assert.get_output().stdout);
    assert_eq!(verify_json.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        verify_json
            .get("car_archive")
            .and_then(|car| car.get("digest_ok"))
            .and_then(Value::as_bool),
        Some(false)
    );
    let errors = verify_json
        .get("errors")
        .and_then(Value::as_array)
        .expect("errors");
    assert!(
        errors
            .iter()
            .any(|error| error.get("kind").and_then(Value::as_str) == Some("car_archive_digest")),
        "expected CAR digest failure in {errors:?}"
    );
}
#[test]
fn governance_dag_checkpoint_recover_rebuilds_mirror_index() {
    let tempdir = tempdir().expect("tempdir");
    let (build_dir, car_path, index_path, checkpoint_path, head_cid_hex) =
        build_governance_dag_checkpoint_fixture(tempdir.path());
    fs::remove_file(&index_path).expect("remove original mirror index");
    let recovered_index_path = tempdir.path().join("recovered-mirror-index.json");
    let summary_path = tempdir.path().join("checkpoint-recover-summary.json");
    let recover_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("checkpoint-recover")
        .arg(format!("--checkpoint={}", checkpoint_path.display()))
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", recovered_index_path.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg("--require-sidecars")
        .arg(format!("--summary-out={}", summary_path.display()))
        .assert()
        .success();
    let recover_json = parse_cli_json_stdout(&recover_assert.get_output().stdout);
    assert_eq!(
        recover_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.checkpoint.recover.v1")
    );
    assert_eq!(recover_json.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        recover_json
            .get("recovered_mirror_index")
            .and_then(|index| index.get("head_block_cid_hex"))
            .and_then(Value::as_str),
        Some(head_cid_hex.as_str())
    );
    let recovered = fs::read_to_string(&recovered_index_path).expect("read recovered index");
    let recovered_json: Value = from_slice(recovered.as_bytes()).expect("recovered index json");
    assert_eq!(
        recovered_json.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.mirror.v1")
    );
    assert_eq!(
        recovered_json
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str),
        Some(head_cid_hex.as_str())
    );
    let summary = fs::read_to_string(&summary_path).expect("read recovery summary");
    let summary_json: Value = from_slice(summary.as_bytes()).expect("recovery summary json");
    assert_eq!(summary_json, recover_json);
}
#[test]
fn governance_dag_checkpoint_recover_rejects_tampered_car_without_writing_index() {
    let tempdir = tempdir().expect("tempdir");
    let (build_dir, car_path, _index_path, checkpoint_path, _) =
        build_governance_dag_checkpoint_fixture(tempdir.path());
    fs::write(&car_path, b"tampered governance dag car").expect("tamper CAR");
    let recovered_index_path = tempdir.path().join("recovered-mirror-index.json");
    let recover_assert = sorafs_cli_cmd()
        .arg("governance")
        .arg("dag")
        .arg("checkpoint-recover")
        .arg(format!("--checkpoint={}", checkpoint_path.display()))
        .arg(format!("--root={}", build_dir.display()))
        .arg(format!("--out={}", recovered_index_path.display()))
        .arg(format!("--car={}", car_path.display()))
        .arg("--require-sidecars")
        .assert()
        .failure();
    let recover_json = parse_cli_json_stdout(&recover_assert.get_output().stdout);
    assert_eq!(recover_json.get("ok").and_then(Value::as_bool), Some(false));
    assert!(
        !recovered_index_path.exists(),
        "checkpoint recovery must not write a mirror index for invalid inputs"
    );
    let errors = recover_json
        .get("errors")
        .and_then(Value::as_array)
        .expect("errors");
    assert!(
        errors
            .iter()
            .any(|error| error.get("kind").and_then(Value::as_str) == Some("car_archive_digest")),
        "expected CAR digest failure in {errors:?}"
    );
}
#[test]
fn ci_sample_fixtures_are_consistent() {
    let base_rel = PathBuf::from("fixtures/sorafs_manifest/ci_sample");
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(&base_rel);
    assert!(
        base.is_dir(),
        "expected fixture directory `{}` to exist",
        base.display()
    );
    let payload_path_rel = base_rel.join("payload.txt");
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
    let parsed_chunk_plan = chunk_fetch_plan_from_json(&chunk_plan_value)
        .expect("chunk plan should parse as the canonical envelope");
    assert_eq!(
        parsed_chunk_plan.payload_digest,
        *payload_digest.as_bytes(),
        "standalone plan must bind the complete payload digest"
    );
    let chunk_array = &parsed_chunk_plan.chunk_fetch_specs;
    assert_eq!(
        chunk_array.len(),
        1,
        "ci sample currently targets a single chunk plan"
    );
    let chunk_entry = &chunk_array[0];
    let chunk_offset = chunk_entry.offset;
    let chunk_length = u64::from(chunk_entry.length);
    assert_eq!(chunk_offset, 0, "chunk should start at offset zero");
    assert_eq!(
        chunk_length,
        payload.len() as u64,
        "chunk length should cover entire payload"
    );
    let chunk_digest_bytes = chunk_entry.digest;
    let chunk_digest_hex = hex_encode(chunk_digest_bytes);
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
    chunk_digest_sha3.update(chunk_digest_bytes);
    let chunk_digest_sha3_hex = hex_encode(chunk_digest_sha3.finalize());
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
    let car_payload_digest_hex = car_summary
        .get("car_payload_digest_hex")
        .and_then(Value::as_str)
        .expect("car payload digest present")
        .to_string();
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
        payload_path_rel.display().to_string(),
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
        manifest.pin_policy.retention_epoch, 86_400,
        "retention epoch should cover the documented one-day default"
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
    for retired_artifact in [
        "manifest.bundle.json",
        "manifest.sig",
        "manifest.sign.summary.json",
        "manifest.verify.summary.json",
    ] {
        let retired_path = base.join(retired_artifact);
        assert!(
            !retired_path.exists(),
            "retired CLI authentication artifact must remain absent: {}",
            retired_path.display()
        );
    }
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
        Some(payload_digest_hex.as_str()),
        "proof summary should embed payload BLAKE3 digest"
    );
    assert_eq!(
        proof_value.get("car_digest_hex").and_then(Value::as_str),
        Some(car_digest_hex.as_str()),
        "proof summary should embed CAR digest"
    );
    assert_eq!(
        proof_value
            .get("car_payload_digest_hex")
            .and_then(Value::as_str),
        Some(car_payload_digest_hex.as_str()),
        "proof summary should embed CAR payload digest"
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
    let mut specs = chunk_fetch_plan_from_json(&value)
        .expect("canonical plan")
        .chunk_fetch_specs;
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
    let payload = canonical_por_payload();
    let plan = CarBuildPlan::single_file(&payload).expect("proof-stream manifest plan");
    let car_stats = CarWriter::new(&plan, &payload)
        .expect("proof-stream manifest CAR writer")
        .write_to(std::io::sink())
        .expect("derive proof-stream manifest CAR archive stats");
    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(
            sorafs_chunker::ChunkProfile::DEFAULT,
            BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .build()
        .expect("manifest build");
    let path = dir.join(file_name);
    let bytes = to_bytes(&manifest).expect("encode manifest");
    fs::write(&path, bytes).expect("write manifest");
    path
}
fn canonical_por_payload() -> Vec<u8> {
    (0..1024)
        .map(|value| (value as u8).wrapping_mul(29))
        .collect()
}
#[test]
fn proof_stream_pdp_rejects_missing_challenge_and_client_sampling()
-> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path =
        write_proof_stream_manifest(tempdir.path(), "stream_pdp_invalid_manifest.to");
    let provider_id_hex = hex_encode([0x13u8; 32]);
    let missing = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--torii-url=https://example.invalid")
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=pdp")
        .assert()
        .failure();
    assert!(
        String::from_utf8_lossy(&missing.get_output().stderr)
            .contains("`--challenge-id-hex=HEX32` is required")
    );
    let sampled = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--torii-url=https://example.invalid")
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=pdp")
        .arg(format!("--challenge-id-hex={}", hex_encode([0x14u8; 32])))
        .arg("--samples=1")
        .assert()
        .failure();
    assert!(
        String::from_utf8_lossy(&sampled.get_output().stderr)
            .contains("sampling is fixed by the governed challenge")
    );
    let legacy_provider = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--torii-url=https://example.invalid")
        .arg("--provider-id=legacy-provider")
        .arg("--proof-kind=pdp")
        .arg(format!("--challenge-id-hex={}", hex_encode([0x15u8; 32])))
        .assert()
        .failure();
    assert!(
        String::from_utf8_lossy(&legacy_provider.get_output().stderr)
            .contains("unrecognised option `--provider-id`")
    );
    let legacy_endpoint = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--endpoint=https://example.invalid/v1/sorafs/proof/stream")
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=pdp")
        .arg(format!("--challenge-id-hex={}", hex_encode([0x15u8; 32])))
        .assert()
        .failure();
    assert!(
        String::from_utf8_lossy(&legacy_endpoint.get_output().stderr)
            .contains("unrecognised option `--endpoint`")
    );
    for invalid_provider_id in [
        "13".repeat(31),
        "ab".repeat(32).to_ascii_uppercase(),
        format!(" {}", "13".repeat(32)),
        format!("{} ", "13".repeat(32)),
    ] {
        let rejected = sorafs_cli_cmd()
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg("--torii-url=https://example.invalid")
            .arg(format!("--provider-id-hex={invalid_provider_id}"))
            .arg("--proof-kind=pdp")
            .arg(format!("--challenge-id-hex={}", hex_encode([0x15u8; 32])))
            .assert()
            .failure();
        assert!(
            String::from_utf8_lossy(&rejected.get_output().stderr)
                .contains("invalid `--provider-id-hex`")
                || String::from_utf8_lossy(&rejected.get_output().stderr)
                    .contains("must be exact 64-character lowercase hexadecimal")
        );
    }
    for (invalid_challenge_id, expected_error) in [
        ("00".repeat(32), "must be non-zero"),
        (
            "cd".repeat(32).to_ascii_uppercase(),
            "invalid `--challenge-id-hex`",
        ),
        (
            format!(" {}", "15".repeat(32)),
            "invalid `--challenge-id-hex`",
        ),
        (
            format!("{} ", "15".repeat(32)),
            "invalid `--challenge-id-hex`",
        ),
    ] {
        let rejected = sorafs_cli_cmd()
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg("--torii-url=https://example.invalid")
            .arg(format!("--provider-id-hex={provider_id_hex}"))
            .arg("--proof-kind=pdp")
            .arg(format!("--challenge-id-hex={invalid_challenge_id}"))
            .assert()
            .failure();
        assert!(
            String::from_utf8_lossy(&rejected.get_output().stderr).contains(expected_error),
            "expected `{expected_error}` for challenge `{invalid_challenge_id}`, got: {}",
            String::from_utf8_lossy(&rejected.get_output().stderr)
        );
    }
    for (option, value) in [("--proof-kind", " pdp"), ("--tier", " hot")] {
        let mut command = sorafs_cli_cmd();
        command
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg("--torii-url=https://example.invalid")
            .arg(format!("--provider-id-hex={provider_id_hex}"));
        if option != "--proof-kind" {
            command.arg("--proof-kind=pdp");
        }
        let rejected = command
            .arg(format!("--challenge-id-hex={}", hex_encode([0x15u8; 32])))
            .arg(format!("{option}={value}"))
            .assert()
            .failure();
        assert!(
            String::from_utf8_lossy(&rejected.get_output().stderr).contains("unsupported proof"),
            "expected canonical-label rejection for `{option}={value}`"
        );
    }
    Ok(())
}
#[test]
fn proof_stream_rejects_retired_root_and_verification_budget_flags()
-> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let manifest_path = write_proof_stream_manifest(tempdir.path(), "stream_retired_flags.to");
    let provider_id_hex = hex_encode([0x66u8; 32]);
    for retired in [
        format!("--por-root-hex={}", hex_encode([0xBBu8; 32])),
        "--max-verification-failures=1".to_string(),
        "--max-failures=1".to_string(),
        "--stream-token=argv-secret".to_string(),
    ] {
        let assert = sorafs_cli_cmd()
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg("--gateway-url=https://example.invalid/v1/sorafs/proof/stream")
            .arg(format!("--provider-id-hex={provider_id_hex}"))
            .arg(&retired)
            .assert()
            .failure();
        let stderr = String::from_utf8(assert.get_output().stderr.clone())?;
        let option = retired.split('=').next().expect("retired option");
        assert!(
            stderr.contains(&format!("unrecognised option `{option}`")),
            "retired proof-stream option must not parse: {stderr}"
        );
        assert!(!stderr.contains("argv-secret"));
    }
    let help = sorafs_cli_cmd().arg("--help").assert().failure();
    let stderr = String::from_utf8(help.get_output().stderr.clone())?;
    let proof_stream_help = stderr
        .lines()
        .find(|line| line.contains("sorafs_cli proof stream"))
        .expect("global usage must include the proof-stream command");
    for retired in [
        "--por-root-hex",
        "--max-verification-failures",
        "--max-failures",
        "--stream-token",
    ] {
        assert!(
            !proof_stream_help.contains(retired),
            "proof-stream usage retained `{retired}`: {proof_stream_help}"
        );
    }
    Ok(())
}
#[test]
fn proof_stream_rejects_zero_root_and_noncanonical_manifest_before_output()
-> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let canonical_path = write_proof_stream_manifest(tempdir.path(), "canonical_manifest.to");
    let canonical_bytes = fs::read(&canonical_path)?;
    let mut zero_root_manifest: ManifestV1 = decode_from_bytes(&canonical_bytes)?;
    zero_root_manifest.por_root = [0; 32];
    let zero_root_path = tempdir.path().join("zero_root_manifest.to");
    fs::write(&zero_root_path, to_bytes(&zero_root_manifest)?)?;
    let mut noncanonical_bytes = canonical_bytes;
    noncanonical_bytes.push(0);
    let noncanonical_path = tempdir.path().join("noncanonical_manifest.to");
    fs::write(&noncanonical_path, noncanonical_bytes)?;
    let provider_id_hex = hex_encode([0x67u8; 32]);
    for (manifest_path, expected_error, output_tag) in [
        (
            zero_root_path.as_path(),
            "requires a non-zero `por_root`",
            "zero-root",
        ),
        (
            noncanonical_path.as_path(),
            "exact canonical manifest",
            "noncanonical",
        ),
    ] {
        let summary_path = tempdir.path().join(format!("{output_tag}-summary.json"));
        let evidence_dir = tempdir.path().join(format!("{output_tag}-evidence"));
        let assert = sorafs_cli_cmd()
            .arg("proof")
            .arg("stream")
            .arg(format!("--manifest={}", manifest_path.display()))
            .arg("--gateway-url=https://example.invalid/v1/sorafs/proof/stream")
            .arg(format!("--provider-id-hex={provider_id_hex}"))
            .arg("--emit-events=true")
            .arg(format!("--summary-out={}", summary_path.display()))
            .arg(format!(
                "--governance-evidence-dir={}",
                evidence_dir.display()
            ))
            .assert()
            .failure();
        let stderr = String::from_utf8(assert.get_output().stderr.clone())?;
        assert!(
            stderr.contains(expected_error),
            "unexpected manifest rejection for {}: {stderr}",
            manifest_path.display()
        );
        assert!(assert.get_output().stdout.is_empty());
        assert!(!summary_path.exists());
        assert!(!evidence_dir.exists());
    }
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
        .arg("--torii-url=https://torii.sora.example")
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
fn proof_stream_potr_without_request_scope_job_id_errors() -> Result<(), Box<dyn std::error::Error>>
{
    let tempdir = tempdir()?;
    let manifest_path =
        write_proof_stream_manifest(tempdir.path(), "stream_potr_missing_job_id.to");
    let provider_id_hex = hex_encode([0x33u8; 32]);
    let assert = sorafs_cli_cmd()
        .arg("proof")
        .arg("stream")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg("--torii-url=https://torii.sora.example")
        .arg(format!("--provider-id-hex={provider_id_hex}"))
        .arg("--proof-kind=potr")
        .arg("--deadline-ms=90000")
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone())?;
    assert!(
        stderr.contains("`--orchestrator-job-id-hex=HEX16` is required"),
        "stderr should mention missing request-scope job id, got: {stderr}"
    );
    Ok(())
}
#[test]
fn fetch_command_gateway_path_rejects_insecure_local_url() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..4096).map(|idx| (idx % 251) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan");
    let server = MockServer::start();
    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");
    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .governance(council_signed_governance_proofs())
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
    let manifest_path = format!("/v1/sorafs/storage/manifest/{manifest_id_hex}");
    server.mock(|when, then| {
        when.method(GET).path(manifest_path.as_str());
        then.status(200).body(manifest_response.clone());
    });
    for spec in plan.try_chunk_fetch_specs().expect("valid CAR plan") {
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
    let (stream_token_b64, gateway_public_key_hex) =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 4);
    let output_path = tempdir.path().join("assembled.bin");
    let summary_path = tempdir.path().join("fetch_summary.json");
    let base_url = server.url("/");
    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=alpha,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={base_url},stream-token={stream_token_b64}"
        ))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg("--max-peers=1")
        .arg("--retry-budget=4")
        .assert();
    if base_url.starts_with("http://") {
        assert_insecure_gateway_rejected(assert, &[&output_path, &summary_path]);
        return;
    }
    let assert = assert.success();
    let stdout =
        String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8 summary");
    let stdout_summary: Value =
        norito::json::from_str(stdout.trim()).expect("stdout must be json summary");
    assert_eq!(
        stdout_summary.get("chunk_count").and_then(Value::as_u64),
        Some(plan.try_chunk_fetch_specs().expect("valid CAR plan").len() as u64)
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
    assert_eq!(
        receipts.len(),
        plan.try_chunk_fetch_specs().expect("valid CAR plan").len()
    );
}
#[test]
fn fetch_command_direct_policy_does_not_bypass_gateway_url_security() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..2048).map(|idx| (idx % 199) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");
    let server = MockServer::start();
    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");
    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .governance(council_signed_governance_proofs())
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
    let manifest_path = format!("/v1/sorafs/storage/manifest/{manifest_id_hex}");
    server.mock(|when, then| {
        when.method(GET).path(manifest_path.as_str());
        then.status(200).body(manifest_response.clone());
    });
    for spec in plan.try_chunk_fetch_specs().expect("valid CAR plan") {
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
    let (stream_token_b64, gateway_public_key_hex) =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 2);
    let summary_path = tempdir.path().join("direct_fetch_summary.json");
    let output_path = tempdir.path().join("direct_payload.bin");
    let base_url = server.url("/");
    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=gw-direct,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={base_url},stream-token={stream_token_b64}",
        ))
        .arg("--transport-policy=direct-only")
        .arg("--max-peers=1")
        .arg("--retry-budget=3")
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .assert();
    if base_url.starts_with("http://") {
        assert_insecure_gateway_rejected(assert, &[&output_path, &summary_path]);
        return;
    }
    let assert = assert.success();
    let stdout_summary: Value = norito::json::from_slice(assert.get_output().stdout.as_slice())
        .expect("stdout summary json");
    assert_eq!(
        stdout_summary.get("chunk_count").and_then(Value::as_u64),
        Some(plan.try_chunk_fetch_specs().expect("valid CAR plan").len() as u64)
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
fn fetch_command_policy_override_does_not_bypass_gateway_url_security() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..1024).map(|idx| (idx % 157) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json") + "\n";
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
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(payload.len() as u64)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .governance(council_signed_governance_proofs())
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
    let manifest_path = format!("/v1/sorafs/storage/manifest/{manifest_id_hex}");
    server.mock(|when, then| {
        when.method(GET).path(manifest_path.as_str());
        then.status(200).body(manifest_response.clone());
    });
    for spec in plan.try_chunk_fetch_specs().expect("valid CAR plan") {
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
    let (stream_token_b64, gateway_public_key_hex) =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 2);
    let output_path = tempdir.path().join("override.bin");
    let summary_path = tempdir.path().join("override_summary.json");
    let base_url = server.url("/");
    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=alpha,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={base_url},stream-token={stream_token_b64}"
        ))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg("--anonymity-policy-override=anon-guard-pq")
        .assert();
    if base_url.starts_with("http://") {
        assert_insecure_gateway_rejected(assert, &[&output_path, &summary_path]);
        return;
    }
    assert.success();
    let summary_bytes = fs::read(&summary_path).expect("read override summary");
    let summary: Value = from_slice(&summary_bytes).expect("parse override summary");
    assert_eq!(
        summary.get("anonymity_policy").and_then(Value::as_str),
        Some("anon-guard-pq")
    );
}
include!("sorafs_cli/pdp.rs");
include!("sorafs_cli/fetch_and_taikai_security.rs");
include!("sorafs_cli/por_report.rs");
