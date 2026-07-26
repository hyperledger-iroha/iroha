//! CLI regressions for the SoraFS Taikai CAR bundler.

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::{Command, cargo::cargo_bin_cmd};
use iroha_data_model::taikai::TaikaiSegmentEnvelopeV1;
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
