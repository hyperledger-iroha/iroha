use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::TempDir;

#[test]
fn simulate_writes_frames_and_telemetry() {
    let temp = TempDir::new().expect("tempdir");
    let frames_dir = temp.path().join("frames");
    let telemetry_path = temp.path().join("telemetry.json");
    let json_path = temp.path().join("report.json");

    let mut cmd = cargo_bin_cmd!("soranet-handshake-harness");
    cmd.args([
        "simulate",
        "--client-hex",
        "0101000201010102000201010202000200047f100004deadbeef7f110004cafebabe",
        "--relay-hex",
        "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f02010001010202000200047f12000412345678",
        "--client-static-sk-hex",
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        "--relay-static-sk-hex",
        "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100",
        "--descriptor-commit-hex",
        "76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f",
        "--client-nonce-hex",
        "2c1f64028dbe42410d1921cd9a316bed4f8f5b52ffb62b4dcaf149048393ca8a",
        "--relay-nonce-hex",
        "d5f4f2f9c2b1a39e88bbd3c0a4f9e178d93e7bfacaf0c3e872b712f4a341c9de",
        "--kem-id",
        "1",
        "--sig-id",
        "1",
        "--json-out",
        json_path.to_str().expect("json path"),
        "--frames-out",
        frames_dir.to_str().expect("frames path"),
        "--telemetry-out",
        telemetry_path.to_str().expect("telemetry path"),
    ])
    .assert()
    .success();

    for (role, action) in [
        ("client", "clienthello"),
        ("relay", "relayhello"),
        ("client", "clientfinish"),
    ] {
        let filename = format!("{role}_{action}.bin");
        let path = frames_dir.join(&filename);
        let bytes = fs::read(&path).unwrap_or_else(|err| {
            panic!("expected frame {path:?} to exist: {err}");
        });
        assert!(!bytes.is_empty(), "frame {path:?} should not be empty");
        assert_eq!(
            bytes.len() % 1024,
            0,
            "frame {path:?} should be padded to 1024-byte blocks"
        );
    }

    let telemetry = fs::read_to_string(&telemetry_path).expect("telemetry file should be written");
    let telemetry_value: Value = json::from_str(&telemetry).expect("telemetry JSON should parse");
    let telemetry_obj = telemetry_value
        .as_object()
        .expect("telemetry JSON should be an object");
    assert_eq!(
        telemetry_obj
            .get("event")
            .and_then(Value::as_str)
            .expect("event field"),
        "soranet_handshake_simulation"
    );
    let signature = telemetry_obj
        .get("signature")
        .and_then(Value::as_str)
        .expect("signature field");
    assert!(
        signature.starts_with("dilithium3:"),
        "signature should carry dilithium3 prefix"
    );
    let witness = telemetry_obj
        .get("witness_signature")
        .and_then(Value::as_str)
        .expect("witness signature field");
    assert!(
        witness.starts_with("ed25519:"),
        "witness signature should carry ed25519 prefix"
    );
    assert!(
        telemetry.ends_with('\n'),
        "telemetry payload should end with newline"
    );

    let report = fs::read_to_string(&json_path).expect("json report content");
    assert!(report.contains("transcript_hash_hex"));
}
