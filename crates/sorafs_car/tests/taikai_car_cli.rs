#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::{Command, cargo::cargo_bin_cmd};
use iroha_data_model::taikai::TaikaiSegmentEnvelopeV1;
use tempfile::tempdir;

#[test]
fn taikai_car_cli_generates_bundle() {
    let dir = tempdir().expect("tempdir");
    let payload_path = dir.path().join("segment.m4s");
    fs::write(&payload_path, b"taikai-payload").expect("write payload");
    let car_path = dir.path().join("segment.car");
    let envelope_path = dir.path().join("segment.to");
    let indexes_path = dir.path().join("segment.indexes.json");
    let ingest_path = dir.path().join("segment.ingest.json");

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
