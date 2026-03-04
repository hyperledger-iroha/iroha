#![cfg(feature = "cli")]

use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::{Command, cargo::cargo_bin_cmd};
use blake3::hash as blake3_hash;
use ed25519_dalek::{Signer, SigningKey};
use norito::{
    decode_from_bytes,
    json::{Map, Value, to_string_pretty},
    to_bytes,
};
use sorafs_car::{CarBuildPlan, fetch_plan::chunk_fetch_specs_to_string};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    CapabilityTlv, CapabilityType, CouncilSignature, ProviderAdvertV1, ProviderCapabilityRangeV1,
    SignatureAlgorithm, TransportHintV1, TransportProtocol, compute_advert_body_digest,
    compute_proposal_digest,
    provider_admission::{
        AdmissionRecord, ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1,
    },
};
use tempfile::{TempDir, tempdir};

fn write_payload(path: &PathBuf, size: usize) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    for (idx, byte) in buf.iter_mut().enumerate() {
        *byte = (idx as u8).wrapping_mul(31).wrapping_add(7);
    }
    fs::write(path, &buf).expect("write payload");
    buf
}

fn to_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}

const PROVIDER_ADMISSION_FIXTURES: &str = "fixtures/sorafs_manifest/provider_admission";
const PROVIDER_SIGNING_KEY_BYTES: [u8; 32] = [0x21; 32];

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PROVIDER_ADMISSION_FIXTURES)
}

fn load_fixture_advert(name: &str) -> ProviderAdvertV1 {
    let path = fixture_dir().join(name);
    let bytes = fs::read(&path).expect("read provider advert fixture");
    decode_from_bytes(&bytes).expect("decode provider advert fixture")
}

fn copy_fixture_advert(tempdir: &TempDir, name: &str) -> PathBuf {
    let src = fixture_dir().join(name);
    let dst = tempdir.path().join(name);
    fs::copy(&src, &dst).expect("copy provider advert fixture");
    dst
}

fn write_plan_for_payload_with_profile(
    tempdir: &TempDir,
    payload: &[u8],
    profile: ChunkProfile,
) -> PathBuf {
    let plan = CarBuildPlan::single_file_with_profile(payload, profile).expect("plan");
    let plan_json = chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
        .expect("serialize chunk specs")
        + "\n";
    let path = tempdir.path().join("plan.json");
    fs::write(&path, plan_json.as_bytes()).expect("write plan file");
    path
}

fn write_plan_for_payload(tempdir: &TempDir, payload: &[u8]) -> PathBuf {
    write_plan_for_payload_with_profile(tempdir, payload, ChunkProfile::DEFAULT)
}

fn chunk_profile_from_advert(advert: &ProviderAdvertV1) -> ChunkProfile {
    let range_cap = advert
        .body
        .capabilities
        .iter()
        .find(|cap| cap.cap_type == CapabilityType::ChunkRangeFetch)
        .and_then(|cap| ProviderCapabilityRangeV1::from_bytes(&cap.payload).ok())
        .expect("range capability present in advert");
    ChunkProfile {
        min_size: range_cap.min_granularity as usize,
        target_size: range_cap.min_granularity as usize,
        max_size: range_cap.max_chunk_span as usize,
        break_mask: ChunkProfile::DEFAULT.break_mask,
    }
}

fn resign_advert(advert: &mut ProviderAdvertV1) {
    resign_advert_unvalidated(advert);
    advert
        .validate_with_body(advert.issued_at)
        .expect("mutated advert must remain valid");
}

fn resign_advert_unvalidated(advert: &mut ProviderAdvertV1) {
    let signing_key = SigningKey::from_bytes(&PROVIDER_SIGNING_KEY_BYTES);
    let body_bytes = to_bytes(&advert.body).expect("encode advert body");
    let signature = signing_key.sign(&body_bytes);
    advert.signature.algorithm = SignatureAlgorithm::Ed25519;
    advert.signature.public_key = signing_key.verifying_key().as_bytes().to_vec();
    advert.signature.signature = signature.to_bytes().to_vec();
}

fn write_advert(tempdir: &TempDir, file_name: &str, advert: &ProviderAdvertV1) -> PathBuf {
    let path = tempdir.path().join(file_name);
    let bytes = to_bytes(advert).expect("encode provider advert");
    fs::write(&path, &bytes).expect("write provider advert");
    path
}

fn read_scoreboard_metadata(path: &Path) -> Map {
    let bytes = fs::read(path).expect("read scoreboard file");
    let value: Value = norito::json::from_slice(&bytes).expect("parse scoreboard json into Value");
    value
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .expect("scoreboard metadata object")
}

fn sorafs_fetch_cmd() -> Command {
    let mut cmd = cargo_bin_cmd!("sorafs_fetch");
    cmd.arg("--allow-implicit-provider-metadata");
    cmd
}

#[test]
fn fetch_cli_recovers_payload_from_multiple_providers() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 32 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = norito::json::Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let plan_json = to_string_pretty(&Value::Array(fetch_array)).expect("serialise plan") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan");

    let provider_a_path = tempdir.path().join("provider_a.bin");
    let provider_b_path = tempdir.path().join("provider_b.bin");
    fs::write(&provider_a_path, &payload).expect("write provider a");
    fs::write(&provider_b_path, &payload).expect("write provider b");

    let output_path = tempdir.path().join("assembled.bin");
    let json_out_path = tempdir.path().join("report.json");
    let provider_metrics_path = tempdir.path().join("providers.json");
    let chunk_receipts_path = tempdir.path().join("receipts.json");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", provider_a_path.display()))
        .arg(format!("--provider=beta={}#4", provider_b_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", json_out_path.display()))
        .arg(format!(
            "--provider-metrics-out={}",
            provider_metrics_path.display()
        ))
        .arg(format!(
            "--chunk-receipts-out={}",
            chunk_receipts_path.display()
        ))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");
    let chunk_count = report
        .get("chunk_count")
        .and_then(Value::as_u64)
        .expect("chunk_count");
    assert_eq!(chunk_count, plan.chunks.len() as u64);
    assert_eq!(
        report
            .get("chunk_retry_total")
            .and_then(Value::as_u64)
            .expect("chunk_retry_total"),
        0
    );
    assert_eq!(
        report
            .get("chunk_retry_rate")
            .and_then(Value::as_f64)
            .expect("chunk_retry_rate"),
        0.0
    );
    assert_eq!(
        report
            .get("chunk_attempt_average")
            .and_then(Value::as_f64)
            .expect("chunk_attempt_average"),
        1.0
    );
    assert_eq!(
        report
            .get("provider_failure_total")
            .and_then(Value::as_u64)
            .expect("provider_failure_total"),
        0
    );
    assert_eq!(
        report
            .get("provider_failure_rate")
            .and_then(Value::as_f64)
            .expect("provider_failure_rate"),
        0.0
    );
    assert_eq!(
        report
            .get("provider_success_total")
            .and_then(Value::as_u64)
            .expect("provider_success_total"),
        chunk_count
    );

    let assembled = fs::read(&output_path).expect("read output payload");
    assert_eq!(assembled, payload);

    let json_file = fs::read_to_string(&json_out_path).expect("read report json");
    let report_disk: Value =
        norito::json::from_str(&json_file).expect("parse json report from disk");
    assert_eq!(report, report_disk);

    let provider_metrics =
        fs::read_to_string(&provider_metrics_path).expect("read provider metrics json");
    let provider_value: Value =
        norito::json::from_str(&provider_metrics).expect("parse provider metrics json");
    assert_eq!(
        provider_value,
        report
            .get("provider_reports")
            .cloned()
            .expect("provider_reports in report")
    );

    let receipts = fs::read_to_string(&chunk_receipts_path).expect("read chunk receipts json");
    let receipts_value: Value =
        norito::json::from_str(&receipts).expect("parse chunk receipts json");
    assert_eq!(
        receipts_value,
        report
            .get("chunk_receipts")
            .cloned()
            .expect("chunk_receipts in report")
    );
}

#[test]
fn fetch_cli_handles_failures_across_three_providers() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let min_chunks = 4;
    let chunk_span = ChunkProfile::DEFAULT.max_size;
    let payload = write_payload(
        &payload_path,
        (chunk_span * min_chunks) + (chunk_span / 4).max(1),
    );

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let specs = plan.chunk_fetch_specs();
    assert!(
        specs.len() >= 4,
        "expected at least four chunks for multi-provider failure test, got {}",
        specs.len()
    );
    assert!(
        specs[1].length > 0,
        "chunk 1 length must be greater than zero to exercise failure path"
    );

    let fetch_array: Vec<Value> = specs
        .iter()
        .map(|spec| {
            let mut obj = norito::json::Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let plan_json = to_string_pretty(&Value::Array(fetch_array)).expect("serialise plan") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan");

    let provider_alpha_path = tempdir.path().join("provider_alpha.bin");
    fs::write(&provider_alpha_path, &payload).expect("write provider alpha");

    let provider_beta_path = tempdir.path().join("provider_beta.bin");
    let mut payload_beta = payload.clone();
    let beta_spec = &specs[1];
    let beta_start = beta_spec.offset as usize;
    let beta_len = beta_spec.length as usize;
    payload_beta[beta_start] ^= 0x5A;
    if beta_len > 1 {
        let mid = beta_start + beta_len / 2;
        payload_beta[mid] ^= 0xA5;
    }
    fs::write(&provider_beta_path, &payload_beta).expect("write provider beta");

    let provider_gamma_path = tempdir.path().join("provider_gamma.bin");
    fs::write(&provider_gamma_path, &payload).expect("write provider gamma");

    let output_path = tempdir.path().join("assembled.bin");
    let report_path = tempdir.path().join("report.json");
    let provider_metrics_path = tempdir.path().join("providers.json");
    let chunk_receipts_path = tempdir.path().join("receipts.json");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!(
            "--provider=alpha={}",
            provider_alpha_path.display()
        ))
        .arg(format!("--provider=beta={}", provider_beta_path.display()))
        .arg(format!(
            "--provider=gamma={}",
            provider_gamma_path.display()
        ))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--json-out={}", report_path.display()))
        .arg(format!(
            "--provider-metrics-out={}",
            provider_metrics_path.display()
        ))
        .arg(format!(
            "--chunk-receipts-out={}",
            chunk_receipts_path.display()
        ))
        .arg("--provider-failure-threshold=1")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");

    let chunk_count = report
        .get("chunk_count")
        .and_then(Value::as_u64)
        .expect("chunk_count");
    assert_eq!(chunk_count as usize, specs.len());

    let retries = report
        .get("chunk_retry_total")
        .and_then(Value::as_u64)
        .expect("chunk_retry_total");
    assert!(
        retries >= 1,
        "expected at least one chunk retry after induced failure"
    );

    let provider_failures = report
        .get("provider_failure_total")
        .and_then(Value::as_u64)
        .expect("provider_failure_total");
    assert!(
        provider_failures >= 1,
        "expected provider failures to be recorded"
    );

    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload, "assembled payload should match input");

    let report_disk = fs::read_to_string(&report_path).expect("read report json");
    let report_from_disk: Value =
        norito::json::from_str(&report_disk).expect("parse report json from disk");
    assert_eq!(report_from_disk, report);

    let provider_metrics =
        fs::read_to_string(&provider_metrics_path).expect("read provider metrics");
    let provider_metrics_value: Value =
        norito::json::from_str(&provider_metrics).expect("parse provider metrics json");
    assert_eq!(
        provider_metrics_value,
        report
            .get("provider_reports")
            .cloned()
            .expect("provider_reports in report")
    );

    let receipts_json = fs::read_to_string(&chunk_receipts_path).expect("read chunk receipts json");
    let receipts_value: Value =
        norito::json::from_str(&receipts_json).expect("parse chunk receipts json");
    assert_eq!(
        receipts_value,
        report
            .get("chunk_receipts")
            .cloned()
            .expect("chunk_receipts in report")
    );

    let receipts = report
        .get("chunk_receipts")
        .and_then(Value::as_array)
        .expect("chunk_receipts array");
    assert_eq!(receipts.len(), specs.len());
    for (index, entry) in receipts.iter().enumerate() {
        assert_eq!(
            entry
                .get("chunk_index")
                .and_then(Value::as_u64)
                .expect("chunk_index") as usize,
            index
        );
    }

    let served: std::collections::HashSet<String> = receipts
        .iter()
        .filter_map(|entry| entry.get("provider").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    for provider in ["alpha", "gamma"] {
        assert!(
            served.contains(provider),
            "expected chunk receipts to include provider {provider}"
        );
    }

    let chunk1_provider = receipts[1]
        .get("provider")
        .and_then(Value::as_str)
        .expect("chunk 1 provider");
    assert_eq!(
        chunk1_provider, "gamma",
        "chunk 1 should fall back to gamma after beta failure"
    );

    let chunk3_provider = receipts[3]
        .get("provider")
        .and_then(Value::as_str)
        .expect("chunk 3 provider");
    assert_ne!(
        chunk3_provider, "beta",
        "chunk 3 should be reassigned to a healthy provider after beta failure"
    );

    let provider_reports = report
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider_reports array");
    let mut report_map: std::collections::HashMap<String, (u64, u64, bool)> =
        std::collections::HashMap::new();
    for entry in provider_reports {
        let obj = entry.as_object().expect("provider report object");
        let name = obj
            .get("provider")
            .and_then(Value::as_str)
            .expect("provider name")
            .to_string();
        let successes = obj
            .get("successes")
            .and_then(Value::as_u64)
            .expect("successes");
        let failures = obj
            .get("failures")
            .and_then(Value::as_u64)
            .expect("failures");
        let disabled = obj
            .get("disabled")
            .and_then(Value::as_bool)
            .expect("disabled");
        report_map.insert(name, (successes, failures, disabled));
    }

    let beta_report = report_map.get("beta").expect("beta report");
    assert!(
        beta_report.1 >= 1,
        "beta should record failures after serving corrupted chunks"
    );
    let gamma_report = report_map.get("gamma").expect("gamma report");
    assert!(
        gamma_report.0 >= 1 && gamma_report.1 == 0 && !gamma_report.2,
        "gamma should succeed without failures"
    );
    let alpha_report = report_map.get("alpha").expect("alpha report");
    assert!(
        alpha_report.0 >= 1 && alpha_report.1 == 0 && !alpha_report.2,
        "alpha should remain healthy throughout the fetch"
    );
}

#[test]
fn fetch_cli_limits_max_peers() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 24 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = norito::json::Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let plan_json = to_string_pretty(&Value::Array(fetch_array)).expect("serialise plan") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan");

    let provider_a_path = tempdir.path().join("provider_alpha.bin");
    let provider_b_path = tempdir.path().join("provider_beta.bin");
    fs::write(&provider_a_path, &payload).expect("write provider alpha");
    fs::write(&provider_b_path, &payload).expect("write provider beta");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", provider_a_path.display()))
        .arg(format!("--provider=beta={}#4", provider_b_path.display()))
        .arg("--max-peers=1")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");

    let providers = report
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider_reports array");
    assert_eq!(
        providers.len(),
        1,
        "max-peers should restrict the active provider set"
    );
    let provider_name = providers
        .first()
        .and_then(|value| value.get("provider"))
        .and_then(Value::as_str)
        .expect("provider field");
    assert_eq!(provider_name, "alpha");

    let receipts = report
        .get("chunk_receipts")
        .and_then(Value::as_array)
        .expect("chunk_receipts array");
    assert!(
        receipts.iter().all(|entry| {
            entry
                .get("provider")
                .and_then(Value::as_str)
                .map(|value| value == "alpha")
                .unwrap_or(false)
        }),
        "chunk receipts should only reference the retained provider"
    );
}

#[test]
fn fetch_cli_respects_expected_digest_and_len() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 8 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = norito::json::Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let plan_path = tempdir.path().join("plan.json");
    fs::write(
        &plan_path,
        (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
    )
    .expect("write plan");

    let digest_hex = blake3_hash(&payload).to_hex().to_string();
    let payload_len = payload.len();

    sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--expect-payload-digest={digest_hex}"))
        .arg(format!("--expect-payload-len={payload_len}"))
        .assert()
        .success();
}

#[test]
fn fetch_cli_reads_plan_from_stdin() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 8 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let plan_json = chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json");

    let output_path = tempdir.path().join("assembled.bin");

    sorafs_fetch_cmd()
        .arg("--plan=-")
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .write_stdin(plan_json)
        .assert()
        .success();

    let assembled = fs::read(&output_path).expect("read output payload");
    assert_eq!(assembled, payload);
}

#[test]
fn fetch_cli_reads_manifest_report_from_stdin() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 6 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
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
    let mut manifest_map = Map::new();
    manifest_map.insert("chunk_fetch_specs".into(), Value::Array(fetch_array));
    manifest_map.insert("payload_len".into(), Value::from(payload.len() as u64));
    manifest_map.insert(
        "payload_digest_hex".into(),
        Value::from(blake3_hash(&payload).to_hex().to_string()),
    );
    let manifest_str = to_string_pretty(&Value::Object(manifest_map)).expect("json") + "\n";

    let output_path = tempdir.path().join("assembled.bin");

    sorafs_fetch_cmd()
        .arg("--manifest-report=-")
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .write_stdin(manifest_str)
        .assert()
        .success();

    let assembled = fs::read(&output_path).expect("read output payload");
    assert_eq!(assembled, payload);
}

#[test]
fn fetch_cli_reads_manifest_report_when_plan_omitted() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 6 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
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
    let digest_hex = blake3_hash(&payload).to_hex().to_string();

    let mut report_obj = Map::new();
    report_obj.insert("chunk_fetch_specs".into(), Value::Array(fetch_array));
    report_obj.insert("payload_digest_hex".into(), Value::from(digest_hex.clone()));
    report_obj.insert("payload_len".into(), Value::from(payload.len() as u64));
    let manifest_path = tempdir.path().join("report.json");
    fs::write(
        &manifest_path,
        (to_string_pretty(&Value::Object(report_obj)).expect("json") + "\n").as_bytes(),
    )
    .expect("write manifest report");

    let output_path = tempdir.path().join("assembled.bin");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--manifest-report={}", manifest_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");
    assert_eq!(
        report
            .get("chunk_retry_total")
            .and_then(Value::as_u64)
            .expect("chunk_retry_total"),
        0
    );
    assert_eq!(
        report
            .get("chunk_retry_rate")
            .and_then(Value::as_f64)
            .expect("chunk_retry_rate"),
        0.0
    );
    assert_eq!(
        report
            .get("chunk_attempt_average")
            .and_then(Value::as_f64)
            .expect("chunk_attempt_average"),
        1.0
    );
    assert_eq!(
        report
            .get("provider_failure_total")
            .and_then(Value::as_u64)
            .expect("provider_failure_total"),
        0
    );
    assert_eq!(
        report
            .get("provider_failure_rate")
            .and_then(Value::as_f64)
            .expect("provider_failure_rate"),
        0.0
    );

    let assembled = fs::read(&output_path).expect("read assembled");
    assert_eq!(assembled, payload);
}

#[test]
fn fetch_cli_rejects_digest_mismatch() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = norito::json::Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let plan_path = tempdir.path().join("plan.json");
    fs::write(
        &plan_path,
        (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
    )
    .expect("write plan");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg("--expect-payload-digest=0000000000000000000000000000000000000000000000000000000000000000")
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(assert.get_output().stderr.as_ref());
    assert!(
        stderr.contains("assembled payload digest"),
        "stderr should mention digest mismatch, got: {stderr}"
    );
}

#[test]
fn fetch_cli_streaming_outputs_fail_on_corruption() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 12 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = norito::json::Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let plan_path = tempdir.path().join("plan.json");
    fs::write(
        &plan_path,
        (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
    )
    .expect("write plan");

    // Corrupt the provider payload so chunk verification fails mid-stream.
    let mut corrupt = payload.clone();
    corrupt[0] ^= 0xff;
    let provider_path = tempdir.path().join("provider.bin");
    fs::write(&provider_path, &corrupt).expect("write corrupt payload");

    let output_path = tempdir.path().join("assembled.bin");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", provider_path.display()))
        .arg("--retry-budget=1")
        .arg(format!("--output={}", output_path.display()))
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(assert.get_output().stderr.as_ref());
    assert!(
        stderr.contains("retry budget exhausted"),
        "stderr should mention retry exhaustion, got: {stderr}"
    );

    let metadata = fs::metadata(&output_path).expect("output metadata");
    assert_eq!(
        metadata.len(),
        0,
        "streamed file should remain empty on failure"
    );
}

#[test]
fn fetch_cli_writes_car_archive() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 10 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
    let fetch_array: Vec<Value> = fetch_specs
        .iter()
        .map(|spec| {
            let mut obj = norito::json::Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
            Value::Object(obj)
        })
        .collect();
    let plan_path = tempdir.path().join("plan.json");
    fs::write(
        &plan_path,
        (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
    )
    .expect("write plan");

    let provider_path = tempdir.path().join("provider.bin");
    fs::write(&provider_path, &payload).expect("write provider");

    let output_path = tempdir.path().join("assembled.bin");
    let car_path = tempdir.path().join("payload.car");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", provider_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse report");
    let car_obj = report
        .get("car_archive")
        .and_then(Value::as_object)
        .expect("car_archive present");
    let car_size = car_obj
        .get("size")
        .and_then(Value::as_u64)
        .expect("car size present");
    assert!(car_size > payload.len() as u64);
    let archive_digest = car_obj
        .get("archive_digest_hex")
        .and_then(Value::as_str)
        .expect("archive digest present");
    assert_eq!(archive_digest.len(), 64);

    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload);

    let car_bytes = fs::read(&car_path).expect("read car output");
    assert!(
        car_bytes.len() > payload.len(),
        "car archive should be larger than raw payload"
    );
    assert!(car_bytes.len() >= 11, "car archive too small");
    assert_eq!(
        &car_bytes[..11],
        &[
            0x0a, 0xa1, 0x67, 0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x02
        ]
    );
}

#[test]
fn fetch_cli_writes_report_to_stdout_when_json_out_dash() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 8 * 1024);

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let fetch_specs = plan.chunk_fetch_specs();
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
    let plan_path = tempdir.path().join("plan.json");
    fs::write(
        &plan_path,
        (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
    )
    .expect("write plan");

    let output_path = tempdir.path().join("assembled.bin");
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg("--json-out=-")
        .arg(format!("--output={}", output_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse stdout json");
    assert_eq!(
        report
            .get("chunk_count")
            .and_then(Value::as_u64)
            .expect("chunk_count"),
        fetch_specs.len() as u64
    );
}

#[test]
fn fetch_cli_can_skip_digest_verification() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let original_payload = write_payload(&payload_path, 12 * 1024);

    let plan = CarBuildPlan::single_file_with_profile(&original_payload, ChunkProfile::DEFAULT)
        .expect("plan");
    let fetch_array: Vec<Value> = plan
        .chunk_fetch_specs()
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
    let plan_path = tempdir.path().join("plan.json");
    fs::write(
        &plan_path,
        (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
    )
    .expect("write plan");

    // Corrupt the provider payload so digest verification would normally fail.
    let mut corrupted = original_payload.clone();
    corrupted[0] ^= 0xFF;
    fs::write(&payload_path, &corrupted).expect("write corrupted payload");

    let output_path = tempdir.path().join("assembled.bin");
    sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg("--allow-insecure")
        .arg("--no-verify-digest")
        .arg(format!("--output={}", output_path.display()))
        .assert()
        .success();

    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, corrupted);
    assert_ne!(assembled, original_payload);
}

#[test]
fn fetch_cli_requires_allow_insecure_for_verification_bypass() {
    let assert = sorafs_fetch_cmd()
        .arg("--no-verify-digest")
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("allow-insecure"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn fetch_cli_accepts_fixture_advert_with_admission() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 6 * 1024);

    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/sorafs_manifest/provider_admission");
    let advert_path = fixture_dir.join("advert_v1.to");
    let proposal_path = fixture_dir.join("proposal_v1.to");
    let envelope_src = fixture_dir.join("envelope_v1.to");

    let fixture_advert = load_fixture_advert("advert_v1.to");
    let plan_path = write_plan_for_payload_with_profile(
        &tempdir,
        &payload,
        chunk_profile_from_advert(&fixture_advert),
    );
    let mut advert_key = [0u8; 32];
    advert_key.copy_from_slice(&fixture_advert.signature.public_key);
    let proposal_bytes = fs::read(&proposal_path).expect("read proposal fixture");
    let mut proposal: ProviderAdmissionProposalV1 =
        decode_from_bytes(&proposal_bytes).expect("decode proposal fixture");
    proposal.advert_key = advert_key;
    let envelope_template_bytes = fs::read(&envelope_src).expect("read envelope fixture");
    let envelope_template: ProviderAdmissionEnvelopeV1 =
        decode_from_bytes(&envelope_template_bytes).expect("decode envelope fixture");
    let council_key = SigningKey::from_bytes(&[0x45; 32]);
    let proposal_digest =
        compute_proposal_digest(&proposal).expect("compute proposal digest from fixture");
    let advert_body = fixture_advert.body.clone();
    let advert_body_digest =
        compute_advert_body_digest(&advert_body).expect("compute advert body digest");
    let council_signature = council_key.sign(&proposal_digest);
    let new_envelope = ProviderAdmissionEnvelopeV1 {
        version: envelope_template.version,
        proposal,
        proposal_digest,
        advert_body,
        advert_body_digest,
        issued_at: envelope_template.issued_at,
        retention_epoch: envelope_template.retention_epoch,
        council_signatures: vec![CouncilSignature {
            signer: *council_key.verifying_key().as_bytes(),
            signature: council_signature.to_bytes().to_vec(),
        }],
        notes: envelope_template.notes.clone(),
    };
    AdmissionRecord::new(new_envelope.clone()).expect("updated admission envelope must be valid");

    let admission_dir = tempdir.path().join("admission");
    fs::create_dir(&admission_dir).expect("create admission dir");
    let envelope_dst = admission_dir.join("envelope_v1.to");
    let new_envelope_bytes = to_bytes(&new_envelope).expect("encode updated envelope");
    fs::write(&envelope_dst, new_envelope_bytes).expect("write admission envelope");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--admission-dir={}", admission_dir.display()))
        .arg("--assume-now=300")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse stdout json");
    let provider_reports = report
        .get("provider_reports")
        .and_then(Value::as_array)
        .expect("provider reports array");
    assert_eq!(
        provider_reports.len(),
        1,
        "expected exactly one provider report entry"
    );
    let provider_report = provider_reports.first().expect("provider report entry");
    let metadata = provider_report
        .get("metadata")
        .and_then(Value::as_object)
        .expect("metadata object");

    let advert_bytes = fs::read(&advert_path).expect("read advert fixture");
    let advert: ProviderAdvertV1 = decode_from_bytes(&advert_bytes).expect("decode advert fixture");

    assert_eq!(
        metadata
            .get("provider_id")
            .and_then(Value::as_str)
            .expect("provider_id metadata field"),
        "alpha"
    );
    assert_eq!(
        metadata
            .get("profile_id")
            .and_then(Value::as_str)
            .expect("profile_id metadata field"),
        advert.body.profile_id
    );
    assert_eq!(
        metadata
            .get("max_streams")
            .and_then(Value::as_u64)
            .expect("max_streams metadata field"),
        advert.body.qos.max_concurrent_streams as u64
    );
    assert_eq!(
        report
            .get("payload_len")
            .and_then(Value::as_u64)
            .expect("payload_len field"),
        payload.len() as u64
    );
}

#[test]
fn fetch_cli_rejects_fixture_advert_without_admission() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 6 * 1024);

    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/sorafs_manifest/provider_admission");
    let advert_path = fixture_dir.join("advert_v1.to");
    let admission_dir = tempdir.path().join("admission");
    fs::create_dir(&admission_dir).expect("create admission dir");
    let fixture_advert = load_fixture_advert("advert_v1.to");
    let plan_path = write_plan_for_payload_with_profile(
        &tempdir,
        &payload,
        chunk_profile_from_advert(&fixture_advert),
    );

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--admission-dir={}", admission_dir.display()))
        .arg("--assume-now=300")
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("is not authorised by the supplied admission directory"),
        "stderr should mention missing admission authorisation, got: {stderr}"
    );
}

#[test]
fn fetch_cli_rejects_advert_after_refresh_deadline() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let advert = load_fixture_advert("advert_v1.to");
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    let refresh_deadline = advert.refresh_deadline();
    let advert_path = copy_fixture_advert(&tempdir, "advert_v1.to");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={refresh_deadline}"))
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("is stale"),
        "expected stale advert rejection, got: {stderr}"
    );
}

#[test]
fn fetch_cli_rejects_advert_missing_chunk_range_capability() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let mut advert = load_fixture_advert("advert_v1.to");
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    advert
        .body
        .capabilities
        .retain(|cap| cap.cap_type != CapabilityType::ChunkRangeFetch);
    advert.body.stream_budget = None;
    advert.body.transport_hints = None;
    assert!(
        advert
            .body
            .capabilities
            .iter()
            .any(|cap| cap.cap_type == CapabilityType::ToriiGateway),
        "mutated advert must retain a supported capability"
    );
    resign_advert(&mut advert);
    let advert_path = write_advert(&tempdir, "advert_without_chunk_range.to", &advert);

    let assume_now = advert.issued_at + 10;
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={assume_now}"))
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("chunk_range_fetch capability required"),
        "expected chunk_range_fetch rejection, got: {stderr}"
    );
}

#[test]
fn fetch_cli_rejects_advert_missing_stream_budget() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let mut advert = load_fixture_advert("advert_v1.to");
    advert.body.stream_budget = None;
    resign_advert(&mut advert);
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    let advert_path = write_advert(&tempdir, "advert_without_stream_budget.to", &advert);

    let assume_now = advert.issued_at + 10;
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={assume_now}"))
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("missing a stream_budget"),
        "expected stream_budget rejection, got: {stderr}"
    );
}

#[test]
fn fetch_cli_rejects_advert_missing_transport_hints() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let mut advert = load_fixture_advert("advert_v1.to");
    advert.body.transport_hints = None;
    resign_advert(&mut advert);
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    let advert_path = write_advert(&tempdir, "advert_without_transport_hints.to", &advert);

    let assume_now = advert.issued_at + 10;
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={assume_now}"))
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("missing transport_hints"),
        "expected transport hint rejection, got: {stderr}"
    );
}

#[test]
fn fetch_cli_rejects_soranet_transport_without_capability() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let mut advert = load_fixture_advert("advert_v1.to");
    advert.body.transport_hints = Some(vec![TransportHintV1 {
        protocol: TransportProtocol::SoraNetRelay,
        priority: 0,
    }]);
    // Fixture lacks SoraNet capability; validation should fail.
    resign_advert_unvalidated(&mut advert);
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    let advert_path = write_advert(&tempdir, "advert_with_soranet_hint.to", &advert);

    let assume_now = advert.issued_at + 10;
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={assume_now}"))
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("soranet transport hints require a soranet capability"),
        "expected soranet transport capability rejection, got: {stderr}"
    );
}

#[test]
fn fetch_cli_rejects_advert_with_invalid_stream_budget() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let mut advert = load_fixture_advert("advert_v1.to");
    if let Some(budget) = advert.body.stream_budget.as_mut() {
        budget.burst_bytes = Some(budget.max_bytes_per_sec + 1);
    }
    resign_advert_unvalidated(&mut advert);
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    let advert_path = write_advert(&tempdir, "advert_with_invalid_budget.to", &advert);

    let assume_now = advert.issued_at + 10;
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={assume_now}"))
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("stream budget invalid"),
        "expected invalid budget rejection, got: {stderr}"
    );
}

#[test]
fn fetch_cli_rejects_unknown_capability_without_opt_in() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let mut advert = load_fixture_advert("advert_v1.to");
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    advert.body.capabilities.push(CapabilityTlv {
        cap_type: CapabilityType::VendorReserved,
        payload: vec![0xAA, 0xBB],
    });
    advert.allow_unknown_capabilities = false;
    resign_advert(&mut advert);
    let advert_path = write_advert(&tempdir, "advert_with_unknown_capability.to", &advert);

    let assume_now = advert.issued_at + 10;
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={assume_now}"))
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("unsupported capabilities vendor_reserved"),
        "expected unknown capability rejection, got: {stderr}"
    );
}

#[test]
fn fetch_cli_allows_unknown_capability_with_opt_in() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);

    let mut advert = load_fixture_advert("advert_v1.to");
    let plan_path =
        write_plan_for_payload_with_profile(&tempdir, &payload, chunk_profile_from_advert(&advert));
    advert.body.capabilities.push(CapabilityTlv {
        cap_type: CapabilityType::VendorReserved,
        payload: vec![0x55, 0x44],
    });
    advert.allow_unknown_capabilities = true;
    resign_advert(&mut advert);
    let advert_path = write_advert(
        &tempdir,
        "advert_with_unknown_capability_opt_in.to",
        &advert,
    );

    let assume_now = advert.issued_at + 10;
    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--provider-advert=alpha={}", advert_path.display()))
        .arg(format!("--assume-now={assume_now}"))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse CLI report");
    let capability_names = report
        .get("provider_reports")
        .and_then(Value::as_array)
        .and_then(|reports| reports.first())
        .and_then(Value::as_object)
        .and_then(|report| report.get("metadata"))
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("capabilities"))
        .and_then(Value::as_array)
        .expect("capabilities metadata field");
    let known_capabilities: Vec<&str> = capability_names.iter().filter_map(Value::as_str).collect();
    assert!(
        known_capabilities
            .iter()
            .all(|name| *name != "vendor_reserved"),
        "unknown capability must be filtered out, got: {known_capabilities:?}"
    );

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        stderr.contains("warning: provider advert"),
        "expected warning about unknown capability, got: {stderr}"
    );
    assert!(
        stderr.contains("vendor_reserved"),
        "warning should mention vendor_reserved capability, got: {stderr}"
    );
}

#[test]
fn fetch_cli_persists_policy_metadata_in_scoreboard() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 4 * 1024);
    let plan_path = write_plan_for_payload(&tempdir, &payload);
    let scoreboard_default = tempdir.path().join("scoreboard_default.json");
    let scoreboard_override = tempdir.path().join("scoreboard_override.json");

    sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!("--scoreboard-out={}", scoreboard_default.display()))
        .assert()
        .success();

    let default_meta = read_scoreboard_metadata(&scoreboard_default);
    assert_eq!(
        default_meta.get("transport_policy").and_then(Value::as_str),
        Some("soranet-first")
    );
    assert_eq!(
        default_meta
            .get("transport_policy_override")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        default_meta.get("anonymity_policy").and_then(Value::as_str),
        Some("anon-guard-pq")
    );
    assert_eq!(
        default_meta
            .get("anonymity_policy_override")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        matches!(
            default_meta.get("transport_policy_override_label"),
            Some(Value::Null)
        ),
        "override label should be null when no override is supplied"
    );
    assert!(
        matches!(
            default_meta.get("anonymity_policy_override_label"),
            Some(Value::Null)
        ),
        "anonymity override label should be null when unset"
    );

    sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg(format!(
            "--scoreboard-out={}",
            scoreboard_override.display()
        ))
        .arg("--transport-policy=direct-only")
        .arg("--transport-policy-override=direct-only")
        .arg("--anonymity-policy=anon-majority-pq")
        .arg("--anonymity-policy-override=anon-strict-pq")
        .assert()
        .success();

    let override_meta = read_scoreboard_metadata(&scoreboard_override);
    assert_eq!(
        override_meta
            .get("transport_policy")
            .and_then(Value::as_str),
        Some("direct-only")
    );
    assert_eq!(
        override_meta
            .get("transport_policy_override")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        override_meta
            .get("transport_policy_override_label")
            .and_then(Value::as_str),
        Some("direct-only")
    );
    assert_eq!(
        override_meta
            .get("anonymity_policy")
            .and_then(Value::as_str),
        Some("anon-strict-pq")
    );
    assert_eq!(
        override_meta
            .get("anonymity_policy_override")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        override_meta
            .get("anonymity_policy_override_label")
            .and_then(Value::as_str),
        Some("anon-strict-pq")
    );
}

#[test]
fn fetch_cli_records_telemetry_region_in_outputs() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    let payload = write_payload(&payload_path, 2 * 1024);
    let plan_path = write_plan_for_payload(&tempdir, &payload);
    let report_path = tempdir.path().join("report.json");
    let scoreboard_path = tempdir.path().join("scoreboard.json");

    let assert = sorafs_fetch_cmd()
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--provider=alpha={}", payload_path.display()))
        .arg("--telemetry-region=regulated-eu")
        .arg(format!("--json-out={}", report_path.display()))
        .arg(format!("--scoreboard-out={}", scoreboard_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let stdout_json: Value = norito::json::from_str(stdout.trim()).expect("stdout json");
    assert_eq!(
        stdout_json.get("telemetry_region").and_then(Value::as_str),
        Some("regulated-eu")
    );

    let summary_bytes = fs::read(&report_path).expect("read report");
    let summary_json: Value = norito::json::from_slice(&summary_bytes).expect("summary json");
    assert_eq!(
        summary_json.get("telemetry_region").and_then(Value::as_str),
        Some("regulated-eu")
    );

    let scoreboard_meta = read_scoreboard_metadata(&scoreboard_path);
    assert_eq!(
        scoreboard_meta
            .get("telemetry_region")
            .and_then(Value::as_str),
        Some("regulated-eu")
    );
}
