#![cfg(feature = "cli")]

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use assert_cmd::cargo::cargo_bin_cmd;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use norito::{
    decode_from_bytes,
    json::{self, Value},
};
use sorafs_manifest::capacity::{
    CapacityDeclarationV1, CapacityDisputeKind, CapacityDisputeV1, CapacityTelemetryV1,
    ReplicationOrderV1,
};
use tempfile::{Builder, TempDir};

fn canonical_temp_base() -> std::path::PathBuf {
    env::temp_dir()
        .canonicalize()
        .expect("canonical system temp dir")
}

fn tempdir() -> Result<TempDir, std::io::Error> {
    Builder::new()
        .prefix("sorafs-capacity-cli-")
        .tempdir_in(canonical_temp_base())
}

fn write_spec(temp: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = temp.path().join(name);
    fs::write(&path, contents.trim_start().as_bytes()).expect("write spec");
    path
}

fn run_capacity_command(args: impl IntoIterator<Item = String>) -> std::process::Output {
    let mut cmd = cargo_bin_cmd!("sorafs_manifest_builder");
    cmd.arg("capacity");
    cmd.args(args);
    cmd.output().expect("run capacity command")
}

fn assert_capacity_failure(output: std::process::Output, expected: &str, output_path: &Path) {
    assert!(
        !output.status.success(),
        "capacity command unexpectedly succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "stderr should contain {expected:?}, got: {stderr}"
    );
    assert!(
        !output_path.exists(),
        "capacity command must fail before writing {}",
        output_path.display()
    );
}

#[test]
fn capacity_declaration_cli_produces_canonical_outputs() {
    let temp = tempdir().expect("tempdir");
    let spec_path = temp.path().join("declaration_spec.json");
    fs::write(&spec_path, SPEC_JSON.trim_start().as_bytes()).expect("write spec");

    let json_out = temp.path().join("summary.json");
    let b64_out = temp.path().join("declaration.b64");
    let norito_out = temp.path().join("declaration.to");

    let mut cmd = cargo_bin_cmd!("sorafs_manifest_builder");
    cmd.arg("capacity")
        .arg("declaration")
        .arg(format!("--spec={}", spec_path.display()))
        .arg(format!("--json-out={}", json_out.display()))
        .arg(format!("--base64-out={}", b64_out.display()))
        .arg(format!("--norito-out={}", norito_out.display()))
        .arg("--quiet");
    cmd.assert().success();

    let base64_text = fs::read_to_string(&b64_out).expect("read base64");
    let base64_trimmed = base64_text.trim();
    let norito_bytes = fs::read(&norito_out).expect("read norito bytes");
    let decoded_bytes = BASE64_STD
        .decode(base64_trimmed.as_bytes())
        .expect("decode base64");
    assert_eq!(decoded_bytes, norito_bytes);

    let declaration: CapacityDeclarationV1 =
        decode_from_bytes(&norito_bytes).expect("decode capacity declaration");
    assert_eq!(declaration.provider_id, [0x11; 32]);
    assert_eq!(declaration.stake.stake_amount.to_string(), "5000");
    assert_eq!(declaration.committed_capacity_gib, 500);
    assert_eq!(declaration.chunker_commitments.len(), 1);
    assert_eq!(declaration.lane_commitments.len(), 1);
    assert_eq!(declaration.metadata.len(), 1);

    let summary_bytes = fs::read(&json_out).expect("read summary");
    let summary_value: Value = json::from_slice(&summary_bytes).expect("parse summary json");
    let summary_obj = summary_value
        .as_object()
        .expect("summary must be an object");
    assert_eq!(
        summary_obj
            .get("provider_id_hex")
            .and_then(Value::as_str)
            .unwrap(),
        "1111111111111111111111111111111111111111111111111111111111111111"
    );
    assert_eq!(
        summary_obj
            .get("committed_capacity_gib")
            .and_then(Value::as_u64)
            .unwrap(),
        500
    );
    assert_eq!(
        summary_obj
            .get("declaration_b64")
            .and_then(Value::as_str)
            .unwrap(),
        base64_trimmed
    );
}

#[test]
fn capacity_declaration_cli_rejects_retired_request_and_key_options() {
    let temp = tempdir().expect("tempdir");
    let spec_path = write_spec(&temp, "declaration_spec.json", SPEC_JSON);

    for option in [
        "--request-out=request.json",
        "--authority=authority@capacity",
        "--private-key=ed25519:retired",
        "--private-key-file=retired.key",
    ] {
        let output = run_capacity_command([
            "declaration".to_owned(),
            format!("--spec={}", spec_path.display()),
            option.to_owned(),
            "--quiet".to_owned(),
        ]);
        assert!(
            !output.status.success(),
            "retired option {option} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unknown capacity option"),
            "unexpected stderr for {option}: {stderr}"
        );
    }
}

#[test]
fn capacity_declaration_cli_rejects_noncanonical_epoch_overrides() {
    let temp = tempdir().expect("tempdir");
    let spec_path = write_spec(&temp, "declaration_spec.json", SPEC_JSON);

    for (flag, value, expected) in [
        ("--registered-epoch", "01700000000", "leading zeros"),
        ("--valid-from-epoch", "+1700000000", "canonical unsigned"),
        ("--valid-until-epoch", "1700086400 ", "whitespace"),
    ] {
        let json_out = temp.path().join(format!(
            "{}.json",
            flag.trim_start_matches("--").replace('-', "_")
        ));
        let output = run_capacity_command([
            "declaration".to_owned(),
            format!("--spec={}", spec_path.display()),
            format!("{flag}={value}"),
            format!("--json-out={}", json_out.display()),
            "--quiet".to_owned(),
        ]);
        assert_capacity_failure(output, expected, &json_out);
    }
}

#[test]
fn capacity_specs_reject_noncanonical_public_fields() {
    let cases = [
        (
            "declaration",
            "declaration_upper_hex.json",
            SPEC_JSON.replace(
                "1111111111111111111111111111111111111111111111111111111111111111",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ),
            "lowercase hex",
        ),
        (
            "declaration",
            "declaration_padded_stake.json",
            SPEC_JSON.replace("\"stake_amount\": \"5000\"", "\"stake_amount\": \"05000\""),
            "leading zeros",
        ),
        (
            "telemetry",
            "telemetry_padded_effective.json",
            TELEMETRY_JSON.replace(
                "\"effective_capacity_gib\": 380",
                "\"effective_capacity_gib\": \"0380\"",
            ),
            "leading zeros",
        ),
        (
            "replication-order",
            "replication_upper_cid.json",
            REPLICATION_JSON.replace(
                "\"manifest_cid_hex\": \"aabbccdd\"",
                "\"manifest_cid_hex\": \"AABBCCDD\"",
            ),
            "lowercase hex",
        ),
        (
            "replication-order",
            "replication_padded_target.json",
            REPLICATION_JSON.replace("\"target_replicas\": 2", "\"target_replicas\": \"02\""),
            "leading zeros",
        ),
        (
            "dispute",
            "dispute_upper_kind.json",
            DISPUTE_JSON.replace(
                "\"kind\": \"replication_shortfall\"",
                "\"kind\": \"Replication_Shortfall\"",
            ),
            "canonical lowercase",
        ),
        (
            "dispute",
            "dispute_padded_evidence_size.json",
            DISPUTE_JSON.replace("\"size_bytes\": 1024", "\"size_bytes\": \"01024\""),
            "leading zeros",
        ),
    ];

    for (subcommand, filename, spec, expected) in cases {
        let temp = tempdir().expect("tempdir");
        let spec_path = write_spec(&temp, filename, &spec);
        let json_out = temp.path().join("summary.json");
        let output = run_capacity_command([
            subcommand.to_owned(),
            format!("--spec={}", spec_path.display()),
            format!("--json-out={}", json_out.display()),
            "--quiet".to_owned(),
        ]);
        assert_capacity_failure(output, expected, &json_out);
    }
}

const SPEC_JSON: &str = r#"
{
  "provider_id_hex": "1111111111111111111111111111111111111111111111111111111111111111",
  "stake": {
    "pool_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "stake_amount": "5000"
  },
  "committed_capacity_gib": 500,
  "chunker_commitments": [
    {
      "profile_handle": "sorafs.sf1@1.0.0",
      "profile_aliases": ["sorafs.sf1@1.0.0", "sorafs-sf1"],
      "committed_gib": 500,
      "capability_refs": ["torii_gateway", "chunk_range_fetch"]
    }
  ],
  "lane_commitments": [
    { "lane_id": "global", "max_gib": 500 }
  ],
  "pricing": {
    "currency": "xor",
    "rate_per_gib_hour_milliu": 12,
    "min_commitment_hours": 24,
    "notes": "primary capacity tranche"
  },
  "valid_from": 1700000000,
  "valid_until": 1700086400,
  "metadata": {
    "region": "global"
  },
  "record_window": {
    "registered_epoch": 1700000000,
    "valid_from_epoch": 1700000000,
    "valid_until_epoch": 1700086400
  }
}
"#;

#[test]
fn capacity_telemetry_cli_produces_canonical_outputs() {
    let temp = tempdir().expect("tempdir");
    let spec_path = temp.path().join("telemetry_spec.json");
    fs::write(&spec_path, TELEMETRY_JSON.trim_start().as_bytes()).expect("write spec");

    let json_out = temp.path().join("telemetry_summary.json");
    let b64_out = temp.path().join("telemetry.b64");
    let norito_out = temp.path().join("telemetry.to");

    let mut cmd = cargo_bin_cmd!("sorafs_manifest_builder");
    cmd.arg("capacity")
        .arg("telemetry")
        .arg(format!("--spec={}", spec_path.display()))
        .arg(format!("--json-out={}", json_out.display()))
        .arg(format!("--base64-out={}", b64_out.display()))
        .arg(format!("--norito-out={}", norito_out.display()))
        .arg("--quiet");
    cmd.assert().success();

    let base64_text = fs::read_to_string(&b64_out).expect("read base64");
    let base64_trimmed = base64_text.trim();
    let norito_bytes = fs::read(&norito_out).expect("read norito bytes");
    let decoded_bytes = BASE64_STD
        .decode(base64_trimmed.as_bytes())
        .expect("decode base64");
    assert_eq!(decoded_bytes, norito_bytes);

    let telemetry: CapacityTelemetryV1 =
        decode_from_bytes(&decoded_bytes).expect("decode capacity telemetry");
    assert_eq!(telemetry.provider_id, [0x33; 32]);
    assert_eq!(telemetry.epoch_start, 1_700_000_000);
    assert_eq!(telemetry.epoch_end, 1_700_000_360);
    assert_eq!(telemetry.declared_capacity_gib, 400);
    assert_eq!(telemetry.utilised_capacity_gib, 360);
    assert_eq!(telemetry.successful_replications, 10);
    assert_eq!(telemetry.failed_replications, 1);
    assert_eq!(telemetry.uptime_percent_milli, 99_500);
    assert_eq!(telemetry.por_success_percent_milli, 99_000);
    assert_eq!(telemetry.notes.as_deref(), Some("weekly snapshot"));

    let summary_bytes = fs::read(&json_out).expect("read summary");
    let summary_value: Value = json::from_slice(&summary_bytes).expect("parse summary json");
    let summary_obj = summary_value.as_object().expect("summary must be object");
    assert_eq!(
        summary_obj
            .get("provider_id_hex")
            .and_then(Value::as_str)
            .unwrap(),
        "3333333333333333333333333333333333333333333333333333333333333333"
    );
    assert_eq!(
        summary_obj
            .get("effective_capacity_gib")
            .and_then(Value::as_u64)
            .unwrap(),
        380
    );
    assert_eq!(
        summary_obj
            .get("telemetry_b64")
            .and_then(Value::as_str)
            .unwrap(),
        base64_trimmed
    );
    assert_eq!(
        summary_obj.get("notes").and_then(Value::as_str).unwrap(),
        "weekly snapshot"
    );
}

#[test]
fn capacity_replication_order_cli_produces_canonical_outputs() {
    let temp = tempdir().expect("tempdir");
    let spec_path = temp.path().join("replication_spec.json");
    fs::write(&spec_path, REPLICATION_JSON.trim_start().as_bytes()).expect("write spec");

    let json_out = temp.path().join("replication_summary.json");
    let b64_out = temp.path().join("replication.b64");
    let norito_out = temp.path().join("replication.to");

    let mut cmd = cargo_bin_cmd!("sorafs_manifest_builder");
    cmd.arg("capacity")
        .arg("replication-order")
        .arg(format!("--spec={}", spec_path.display()))
        .arg(format!("--json-out={}", json_out.display()))
        .arg(format!("--base64-out={}", b64_out.display()))
        .arg(format!("--norito-out={}", norito_out.display()))
        .arg("--quiet");
    cmd.assert().success();

    let base64_text = fs::read_to_string(&b64_out).expect("read base64");
    let base64_trimmed = base64_text.trim();
    let norito_bytes = fs::read(&norito_out).expect("read norito bytes");
    let decoded_bytes = BASE64_STD
        .decode(base64_trimmed.as_bytes())
        .expect("decode base64");
    assert_eq!(decoded_bytes, norito_bytes);

    let order: ReplicationOrderV1 =
        decode_from_bytes(&decoded_bytes).expect("decode replication order");
    assert_eq!(order.order_id, [0x44; 32]);
    assert_eq!(order.chunking_profile, "sorafs.sf1@1.0.0");
    assert_eq!(order.target_replicas, 2);
    assert_eq!(order.assignments.len(), 2);
    assert_eq!(order.assignments[0].slice_gib, 100);
    assert_eq!(order.assignments[1].lane.as_deref(), Some("archive"));
    assert_eq!(order.metadata.len(), 1);
    assert_eq!(order.metadata[0].key, "priority");
    assert_eq!(order.metadata[0].value, "standard");

    let summary_bytes = fs::read(&json_out).expect("read summary");
    let summary_value: Value = json::from_slice(&summary_bytes).expect("parse summary json");
    let summary_obj = summary_value.as_object().expect("summary must be object");
    assert_eq!(
        summary_obj
            .get("order_id_hex")
            .and_then(Value::as_str)
            .unwrap(),
        "4444444444444444444444444444444444444444444444444444444444444444"
    );
    assert_eq!(
        summary_obj
            .get("replication_order_b64")
            .and_then(Value::as_str)
            .unwrap(),
        base64_trimmed
    );
    let assignments = summary_obj
        .get("assignments")
        .and_then(Value::as_array)
        .expect("assignments summary array");
    assert_eq!(assignments.len(), 2);
}

#[test]
fn capacity_cli_rejects_retired_request_output_and_complete_subcommand() {
    let temp = tempdir().expect("tempdir");
    let spec_path = write_spec(&temp, "replication_spec.json", REPLICATION_JSON);
    let output = run_capacity_command([
        "replication-order".to_owned(),
        format!("--spec={}", spec_path.display()),
        "--request-out=request.json".to_owned(),
        "--quiet".to_owned(),
    ]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown capacity option"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_capacity_command(["complete".to_owned()]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown capacity subcommand `complete`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn capacity_dispute_cli_produces_canonical_outputs() {
    let temp = tempdir().expect("tempdir");
    let spec_path = temp.path().join("dispute_spec.json");
    fs::write(&spec_path, DISPUTE_JSON.trim_start().as_bytes()).expect("write spec");

    let json_out = temp.path().join("dispute_summary.json");
    let b64_out = temp.path().join("dispute.b64");
    let norito_out = temp.path().join("dispute.to");

    let mut cmd = cargo_bin_cmd!("sorafs_manifest_builder");
    cmd.arg("capacity")
        .arg("dispute")
        .arg(format!("--spec={}", spec_path.display()))
        .arg(format!("--json-out={}", json_out.display()))
        .arg(format!("--base64-out={}", b64_out.display()))
        .arg(format!("--norito-out={}", norito_out.display()))
        .arg("--quiet");
    cmd.assert().success();

    let base64_text = fs::read_to_string(&b64_out).expect("read base64");
    let base64_trimmed = base64_text.trim();
    let norito_bytes = fs::read(&norito_out).expect("read norito bytes");
    let decoded_bytes = BASE64_STD
        .decode(base64_trimmed.as_bytes())
        .expect("decode base64");
    assert_eq!(decoded_bytes, norito_bytes);

    let dispute: CapacityDisputeV1 =
        decode_from_bytes(&decoded_bytes).expect("decode capacity dispute");
    assert_eq!(dispute.provider_id, [0x22; 32]);
    assert_eq!(dispute.complainant_id, [0xEE; 32]);
    assert_eq!(dispute.replication_order_id, Some([0xAB; 32]));
    assert_eq!(dispute.kind, CapacityDisputeKind::ReplicationShortfall);
    assert_eq!(dispute.evidence.evidence_digest, [0x55; 32]);
    assert_eq!(
        dispute.evidence.media_type.as_deref(),
        Some("application/zip")
    );
    assert_eq!(
        dispute.evidence.uri.as_deref(),
        Some("https://evidence.example.com/bundle.zip")
    );
    assert_eq!(dispute.evidence.size_bytes, Some(1_024));
    assert_eq!(dispute.submitted_epoch, 1_700_100_000);
    assert_eq!(
        dispute.description,
        "Provider failed to ingest replication order within SLA."
    );

    let summary_bytes = fs::read(&json_out).expect("read summary");
    let summary_value: Value = json::from_slice(&summary_bytes).expect("parse summary json");
    let summary_obj = summary_value
        .as_object()
        .expect("summary must be an object");
    assert_eq!(
        summary_obj
            .get("provider_id_hex")
            .and_then(Value::as_str)
            .unwrap(),
        "2222222222222222222222222222222222222222222222222222222222222222"
    );
    assert_eq!(
        summary_obj.get("kind").and_then(Value::as_str),
        Some("replication_shortfall")
    );
    assert_eq!(
        summary_obj
            .get("dispute_b64")
            .and_then(Value::as_str)
            .unwrap(),
        base64_trimmed
    );
}

const TELEMETRY_JSON: &str = r#"
{
  "provider_id_hex": "3333333333333333333333333333333333333333333333333333333333333333",
  "epoch_start": 1700000000,
  "epoch_end": 1700000360,
  "declared_capacity_gib": 400,
  "effective_capacity_gib": 380,
  "utilised_capacity_gib": 360,
  "successful_replications": 10,
  "failed_replications": 1,
  "uptime_percent_milli": 99500,
  "por_success_percent_milli": 99000,
  "notes": "weekly snapshot"
}
"#;

const REPLICATION_JSON: &str = r#"
{
  "order_id_hex": "4444444444444444444444444444444444444444444444444444444444444444",
  "manifest_cid_hex": "aabbccdd",
  "manifest_digest_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "chunking_profile": "sorafs.sf1@1.0.0",
  "target_replicas": 2,
  "assignments": [
    {
      "provider_id_hex": "5555555555555555555555555555555555555555555555555555555555555555",
      "slice_gib": 100,
      "lane": "global"
    },
    {
      "provider_id_hex": "6666666666666666666666666666666666666666666666666666666666666666",
      "slice_gib": 100,
      "lane": "archive"
    }
  ],
  "issued_at": 1700000500,
  "deadline_at": 1700000800,
  "sla": {
    "ingest_deadline_secs": 900,
    "min_availability_percent_milli": 99000,
    "min_por_success_percent_milli": 98000
  },
  "metadata": {
    "priority": "standard"
  }
}
"#;

const DISPUTE_JSON: &str = r#"
{
  "provider_id_hex": "2222222222222222222222222222222222222222222222222222222222222222",
  "complainant_id_hex": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
  "replication_order_id_hex": "abababababababababababababababababababababababababababababababab",
  "kind": "replication_shortfall",
  "submitted_epoch": 1700100000,
  "description": "Provider failed to ingest replication order within SLA.",
  "requested_remedy": "Slash 10% of committed stake",
  "evidence": {
    "digest_hex": "5555555555555555555555555555555555555555555555555555555555555555",
    "media_type": "application/zip",
    "uri": "https://evidence.example.com/bundle.zip",
    "size_bytes": 1024
  }
}
"#;
