//! CLI regressions for the SoraNet trustless verifier.
use assert_cmd::cargo::cargo_bin_cmd;
use norito::decode_from_bytes;
use norito::json::Value;
use sorafs_car::{TrustlessVerificationError, TrustlessVerifier, TrustlessVerifierConfig};
use sorafs_manifest::{ManifestV1, SORAFS_GATEWAY_MANIFEST_DIGEST_HEX};
use std::{env, fs, path::PathBuf};
use tempfile::{Builder, TempDir};
fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(relative)
}
fn canonical_temp_base() -> PathBuf {
    env::temp_dir()
        .canonicalize()
        .expect("canonical system temp dir")
}
fn tempdir() -> Result<TempDir, std::io::Error> {
    Builder::new()
        .prefix("soranet-trustless-verifier-")
        .tempdir_in(canonical_temp_base())
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
fn trustless_verifier_rejects_manifest_chunk_plan_and_por_root_substitution() {
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
    let verifier = TrustlessVerifier::new(config);
    let mut substituted_plan = manifest.clone();
    substituted_plan.chunk_digest_sha3_256[0] ^= 0x80;
    assert!(matches!(
        verifier.verify_full(&substituted_plan, &car_bytes),
        Err(TrustlessVerificationError::ManifestChunkPlanMismatch { .. })
    ));
    let mut substituted_root = manifest;
    substituted_root.por_root[31] ^= 0x01;
    assert!(matches!(
        verifier.verify_full(&substituted_root, &car_bytes),
        Err(TrustlessVerificationError::ManifestPorRootMismatch { .. })
    ));
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
fn trustless_verifier_writes_validation_outcome_json_out() {
    let temp = tempdir().expect("tempdir");
    let outcome_path = temp.path().join("validation_outcome.json");
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
            "--json-out",
            outcome_path.to_str().expect("outcome path is utf-8"),
        ])
        .output()
        .expect("run trustless verifier");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout should be suppressed when --json-out is used"
    );
    let outcome_bytes = fs::read(&outcome_path).expect("read validation outcome");
    let outcome: Value = norito::json::from_slice(&outcome_bytes).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}
#[test]
fn retired_local_pin_record_flag_is_rejected() {
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
            "--finalized-pin-record",
            "finalized-pin-record.to",
        ])
        .output()
        .expect("run trustless verifier");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--finalized-pin-record'"),
        "stderr: {stderr}"
    );
}
#[test]
fn validation_outcome_rejects_noncanonical_generated_at() {
    let manifest = workspace_path("fixtures/sorafs_gateway/1.0.0/manifest_v1.to");
    let car = workspace_path("fixtures/sorafs_gateway/1.0.0/gateway.car");
    let config = workspace_path("configs/soranet/gateway_m0/gateway_trustless_verifier.toml");
    for (value, expected) in [
        ("0", "greater than zero"),
        ("0123", "leading zeros"),
        ("+123", "unsigned decimal"),
        ("123 ", "whitespace"),
        ("18446744073709551616", "invalid --generated-at"),
    ] {
        let temp = tempdir().expect("tempdir");
        let outcome_path = temp.path().join("validation_outcome.json");
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
                value,
                "--json-out",
                outcome_path.to_str().expect("outcome path is utf-8"),
            ])
            .output()
            .expect("run trustless verifier");
        assert!(
            !output.status.success(),
            "generated_at={value:?} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "generated_at={value:?} stderr did not contain {expected:?}: {stderr}"
        );
        assert!(
            !outcome_path.exists(),
            "generated_at={value:?} must fail before writing validation outcome"
        );
    }
}
#[test]
fn generated_at_requires_validation_outcome_mode() {
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
            "--generated-at",
            "123",
        ])
        .output()
        .expect("run trustless verifier");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--generated-at only applies with --validation-outcome"),
        "stderr: {stderr}"
    );
}
