//! CLI regressions for the SoraFS Taikai CAR bundler.
#![cfg(feature = "cli")]
use assert_cmd::{Command, cargo::cargo_bin_cmd};
use iroha_data_model::taikai::TaikaiSegmentEnvelopeV1;
use norito::json::{self, Value};
use std::fs;
use tempfile::tempdir;
#[test]
fn taikai_car_cli_generates_bundle() {
    let dir = tempdir().expect("tempdir");
    let dir_path = dir.path().canonicalize().expect("canonical tempdir");
    let payload_path = dir_path.join("segment.m4s");
    fs::write(&payload_path, b"taikai-payload").expect("write payload");
    let car_path = dir_path.join("segment.car");
    let envelope_path = dir_path.join("segment.to");
    let indexes_path = dir_path.join("segment.indexes.json");
    let ingest_path = dir_path.join("segment.ingest.json");
    let mut cmd: Command = cargo_bin_cmd!("taikai_car");
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
fn taikai_car_cli_rejects_noncanonical_operator_inputs() {
    let cases = vec![
        ("--bitrate-kbps", "08000".to_string(), "leading zeros"),
        ("--bitrate-kbps", "0".to_string(), "greater than zero"),
        ("--segment-sequence", "+42".to_string(), "unsigned decimal"),
        (
            "--segment-start-pts",
            "03600000".to_string(),
            "leading zeros",
        ),
        ("--segment-duration", "0".to_string(), "greater than zero"),
        (
            "--wallclock-unix-ms",
            "1702560000000 ".to_string(),
            "whitespace",
        ),
        ("--ingest-latency-ms", "000".to_string(), "leading zeros"),
        ("--live-edge-drift-ms", "-00".to_string(), "leading zeros"),
        (
            "--codec",
            "aac-lc".to_string(),
            "not valid for a video track",
        ),
        (
            "--manifest-hash",
            format!("0x{}", "11".repeat(32)),
            "0x prefix",
        ),
        ("--manifest-hash", "AA".repeat(32), "lowercase"),
        ("--storage-ticket", "00".repeat(32), "all zeros"),
    ];
    for (flag, bad_value, expected) in cases {
        let dir = tempdir().expect("tempdir");
        let dir_path = dir.path().canonicalize().expect("canonical tempdir");
        let payload_path = dir_path.join("segment.m4s");
        fs::write(&payload_path, b"taikai-payload").expect("write payload");
        let car_path = dir_path.join("segment.car");
        let envelope_path = dir_path.join("segment.to");
        let indexes_path = dir_path.join("segment.indexes.json");
        let ingest_path = dir_path.join("segment.ingest.json");
        let mut cmd: Command = cargo_bin_cmd!("taikai_car");
        cmd.arg("--payload")
            .arg(&payload_path)
            .arg("--car-out")
            .arg(&car_path)
            .arg("--envelope-out")
            .arg(&envelope_path)
            .arg("--indexes-out")
            .arg(&indexes_path)
            .arg("--ingest-metadata-out")
            .arg(&ingest_path);
        push_metadata_args(&mut cmd, (flag, bad_value.as_str()));
        let output = cmd.output().expect("run taikai_car");
        assert!(
            !output.status.success(),
            "case {flag}={bad_value:?} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "case {flag}={bad_value:?} stderr did not contain {expected:?}: {stderr}"
        );
        assert!(
            !car_path.exists() && !envelope_path.exists(),
            "case {flag}={bad_value:?} must fail before writing bundle outputs"
        );
    }
}
#[test]
fn taikai_car_cli_rejects_summary_path_that_overwrites_an_artifact() {
    let dir = tempdir().expect("tempdir");
    let dir_path = dir.path().canonicalize().expect("canonical tempdir");
    let payload_path = dir_path.join("segment.m4s");
    fs::write(&payload_path, b"taikai-payload").expect("write payload");
    let car_path = dir_path.join("segment.car");
    let envelope_path = dir_path.join("segment.to");
    let mut cmd: Command = cargo_bin_cmd!("taikai_car");
    cmd.arg("--payload")
        .arg(&payload_path)
        .arg("--car-out")
        .arg(&car_path)
        .arg("--envelope-out")
        .arg(&envelope_path)
        .arg("--summary-out")
        .arg(&car_path);
    push_metadata_args(&mut cmd, ("", ""));

    let output = cmd.output().expect("run taikai_car");
    assert!(!output.status.success(), "path collision must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must use distinct paths"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !car_path.exists() && !envelope_path.exists(),
        "collision must fail before writing bundle artifacts"
    );
}
#[test]
fn taikai_car_cli_rejects_output_that_overwrites_summary_input() {
    let dir = tempdir().expect("tempdir");
    let dir_path = dir.path().canonicalize().expect("canonical tempdir");
    let payload_path = dir_path.join("segment.m4s");
    fs::write(&payload_path, b"taikai-payload").expect("write payload");
    let summary_path = dir_path.join("seed.json");
    let seed = format!(
        r#"{{
  "ingest": {{
    "event_id": "demo-event",
    "stream_id": "stage-a",
    "rendition_id": "1080p",
    "segment_sequence": 42,
    "segment_start_pts": 3600000,
    "segment_duration": 2000000,
    "wallclock_unix_ms": 1702560000000,
    "manifest_hash": "{}",
    "storage_ticket": "{}"
  }},
  "track": {{
    "kind": "video",
    "codec": "av1-main",
    "bitrate_kbps": 8000,
    "resolution": "1920x1080"
  }}
}}"#,
        "11".repeat(32),
        "22".repeat(32)
    );
    fs::write(&summary_path, &seed).expect("write summary seed");
    let envelope_path = dir_path.join("segment.to");
    let mut cmd: Command = cargo_bin_cmd!("taikai_car");
    cmd.arg("--payload")
        .arg(&payload_path)
        .arg("--summary-in")
        .arg(&summary_path)
        .arg("--car-out")
        .arg(&summary_path)
        .arg("--envelope-out")
        .arg(&envelope_path);

    let output = cmd.output().expect("run taikai_car");

    assert!(!output.status.success(), "input collision must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must use distinct paths"),
        "unexpected stderr: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(&summary_path).expect("read summary seed"),
        seed
    );
    assert!(!envelope_path.exists());
}
#[test]
fn taikai_car_cli_summary_uses_canonical_metadata() {
    let dir = tempdir().expect("tempdir");
    let dir_path = dir.path().canonicalize().expect("canonical tempdir");
    let payload_path = dir_path.join("segment.m4s");
    fs::write(&payload_path, b"taikai-payload").expect("write payload");
    let car_path = dir_path.join("segment.car");
    let envelope_path = dir_path.join("segment.to");
    let ingest_path = dir_path.join("segment.ingest.json");
    let summary_path = dir_path.join("segment.summary.json");
    let mut cmd: Command = cargo_bin_cmd!("taikai_car");
    cmd.arg("--payload")
        .arg(&payload_path)
        .arg("--car-out")
        .arg(&car_path)
        .arg("--envelope-out")
        .arg(&envelope_path)
        .arg("--ingest-metadata-out")
        .arg(&ingest_path)
        .arg("--summary-out")
        .arg(&summary_path)
        .arg("--audio-layout")
        .arg("stereo")
        .arg("--ingest-node-id")
        .arg("  node-a  ");
    push_metadata_args(&mut cmd, ("--codec", "  AV1-MAIN  "));

    cmd.assert().success();

    let summary: Value =
        json::from_str(&fs::read_to_string(&summary_path).expect("read bundle summary"))
            .expect("parse bundle summary");
    let root = summary.as_object().expect("summary object");
    let track = root
        .get("track")
        .and_then(Value::as_object)
        .expect("track object");
    assert_eq!(track.get("codec").and_then(Value::as_str), Some("av1-main"));
    assert_eq!(
        track.get("resolution").and_then(Value::as_str),
        Some("1920x1080")
    );
    assert!(
        !track.contains_key("audio_layout"),
        "video summary must not retain ignored audio metadata"
    );
    let ingest = root
        .get("ingest")
        .and_then(Value::as_object)
        .expect("ingest object");
    assert_eq!(
        ingest.get("ingest_node_id").and_then(Value::as_str),
        Some("node-a")
    );
    let envelope: TaikaiSegmentEnvelopeV1 =
        norito::decode_from_bytes(&fs::read(&envelope_path).expect("read envelope"))
            .expect("decode envelope");
    assert_eq!(
        envelope.instrumentation.ingest_node_id.as_deref(),
        Some("node-a")
    );
    let ingest_metadata: Value =
        json::from_str(&fs::read_to_string(ingest_path).expect("read ingest metadata"))
            .expect("parse ingest metadata");
    assert_eq!(
        ingest_metadata
            .as_object()
            .and_then(|metadata| metadata.get("taikai.instrumentation.ingest_node_id"))
            .and_then(Value::as_str),
        Some("node-a")
    );
}
#[test]
fn taikai_car_cli_readonly_summary_fails_before_bundle_outputs() {
    let dir = tempdir().expect("tempdir");
    let dir_path = dir.path().canonicalize().expect("canonical tempdir");
    let payload_path = dir_path.join("segment.m4s");
    fs::write(&payload_path, b"taikai-payload").expect("write payload");
    let car_path = dir_path.join("segment.car");
    let envelope_path = dir_path.join("segment.to");
    let summary_path = dir_path.join("segment.summary.json");
    fs::write(&summary_path, b"preserve").expect("write summary");
    let mut permissions = fs::metadata(&summary_path)
        .expect("summary metadata")
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&summary_path, permissions).expect("make summary read-only");
    let mut cmd: Command = cargo_bin_cmd!("taikai_car");
    cmd.arg("--payload")
        .arg(&payload_path)
        .arg("--car-out")
        .arg(&car_path)
        .arg("--envelope-out")
        .arg(&envelope_path)
        .arg("--summary-out")
        .arg(&summary_path);
    push_metadata_args(&mut cmd, ("", ""));

    let output = cmd.output().expect("run taikai_car");

    assert!(!output.status.success(), "read-only summary must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must be writable"),
        "unexpected stderr: {stderr}"
    );
    assert!(!car_path.exists() && !envelope_path.exists());
    assert_eq!(fs::read(summary_path).expect("read summary"), b"preserve");
}
fn push_metadata_args(cmd: &mut Command, override_pair: (&str, &str)) {
    push_pair(cmd, "--event-id", "demo-event", override_pair);
    push_pair(cmd, "--stream-id", "stage-a", override_pair);
    push_pair(cmd, "--rendition-id", "1080p", override_pair);
    push_pair(cmd, "--track-kind", "video", override_pair);
    push_pair(cmd, "--codec", "av1-main", override_pair);
    push_pair(cmd, "--bitrate-kbps", "8000", override_pair);
    push_pair(cmd, "--resolution", "1920x1080", override_pair);
    push_pair(cmd, "--segment-sequence", "42", override_pair);
    push_pair(cmd, "--segment-start-pts", "3600000", override_pair);
    push_pair(cmd, "--segment-duration", "2000000", override_pair);
    push_pair(cmd, "--wallclock-unix-ms", "1702560000000", override_pair);
    push_pair(cmd, "--manifest-hash", &"11".repeat(32), override_pair);
    push_pair(cmd, "--storage-ticket", &"22".repeat(32), override_pair);
    push_pair(cmd, "--ingest-latency-ms", "7", override_pair);
    push_pair(cmd, "--live-edge-drift-ms", "-3", override_pair);
}
fn push_pair(cmd: &mut Command, flag: &str, default: &str, override_pair: (&str, &str)) {
    cmd.arg(flag).arg(if override_pair.0 == flag {
        override_pair.1
    } else {
        default
    });
}
