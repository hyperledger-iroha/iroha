use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::decode_from_bytes;
use norito::json::Value;
use sorafs_car::{TrustlessVerifier, TrustlessVerifierConfig};
use sorafs_manifest::{ManifestV1, SORAFS_GATEWAY_MANIFEST_DIGEST_HEX};

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(relative)
}

#[test]
fn trustless_verifier_reports_gateway_fixture_digests() {
    let config_path = workspace_path("configs/soranet/gateway_m0/gateway_trustless_verifier.toml");
    let config =
        TrustlessVerifierConfig::from_file(&config_path).expect("gateway config parses cleanly");

    let manifest_bytes = fs::read(workspace_path(
        "fixtures/sorafs_gateway/1.0.0/manifest_v1.to",
    ))
    .expect("manifest bytes");
    let manifest: ManifestV1 =
        decode_from_bytes(&manifest_bytes).expect("manifest Norito decoding succeeds");

    let car_bytes = fs::read(workspace_path("fixtures/sorafs_gateway/1.0.0/gateway.car"))
        .expect("gateway CAR bytes");

    let outcome = TrustlessVerifier::new(config)
        .verify_full(&manifest, &car_bytes)
        .expect("trustless verification succeeds");

    assert_eq!(
        outcome.manifest_digest_hex(),
        SORAFS_GATEWAY_MANIFEST_DIGEST_HEX,
        "manifest digest should match published fixture metadata"
    );
    assert_eq!(
        outcome.car_digest_hex(),
        "ce50a9aadf84e57559208d39201621262fd1b1887ae490ca54470e2a00153f27",
        "CAR digest should match gateway helper file"
    );
    assert_eq!(
        outcome.payload_digest_hex(),
        "91275991d58858bdc7ce3eb4472b61c5289dec3ecc6cf43c6411db772c1888a8",
        "payload digest should match gateway helper file"
    );

    // Chunk plan digest and PoR root should be fully populated.
    assert_eq!(outcome.chunk_plan_digest_hex().len(), 64);
    assert_ne!(outcome.chunk_plan_digest_hex(), outcome.car_digest_hex());
    assert_eq!(outcome.por_root_hex().len(), 64);
    assert!(
        outcome.por_root_hex().chars().any(|ch| ch != '0'),
        "PoR root must not be all zeros"
    );

    assert_eq!(outcome.profile_handle(), "sorafs.sf1@1.0.0");
    assert_eq!(
        outcome.report.stats.payload_bytes, manifest.content_length,
        "payload length should come from the manifest"
    );
    assert!(
        !outcome.report.chunk_store.chunks().is_empty(),
        "chunk store should carry the rebuilt plan"
    );
}

#[test]
fn trustless_verifier_emits_reference_validation_outcome() {
    let manifest = workspace_path("fixtures/sorafs_gateway/1.0.0/manifest_v1.to");
    let car = workspace_path("fixtures/sorafs_gateway/1.0.0/gateway.car");
    let config = workspace_path("configs/soranet/gateway_m0/gateway_trustless_verifier.toml");

    let output = cargo_bin_cmd!("soranet_trustless_verifier")
        .args([
            "--manifest",
            manifest.to_str().expect("manifest path is utf-8"),
            "--car",
            car.to_str().expect("CAR path is utf-8"),
            "--config",
            config.to_str().expect("config path is utf-8"),
            "--validation-outcome",
            "--generated-at",
            "123",
        ])
        .output()
        .expect("run trustless verifier");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn validation_outcome_rejects_pin_record_flag_explicitly() {
    let manifest = workspace_path("fixtures/sorafs_gateway/1.0.0/manifest_v1.to");
    let car = workspace_path("fixtures/sorafs_gateway/1.0.0/gateway.car");
    let config = workspace_path("configs/soranet/gateway_m0/gateway_trustless_verifier.toml");

    let output = cargo_bin_cmd!("soranet_trustless_verifier")
        .args([
            "--manifest",
            manifest.to_str().expect("manifest path is utf-8"),
            "--car",
            car.to_str().expect("CAR path is utf-8"),
            "--config",
            config.to_str().expect("config path is utf-8"),
            "--validation-outcome",
            "--pin-record",
            "pin-record.to",
        ])
        .output()
        .expect("run trustless verifier");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--validation-outcome emits manifest/CAR replay outcomes"),
        "stderr: {stderr}"
    );
}
