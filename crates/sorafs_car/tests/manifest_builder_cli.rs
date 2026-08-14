//! CLI regression tests for the SoraFS manifest builder.
#![cfg(feature = "cli")]
use std::{env, fs, path::PathBuf};
use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::{Builder, TempDir};
fn canonical_temp_base() -> PathBuf {
    env::temp_dir()
        .canonicalize()
        .expect("canonical system temp dir")
}
fn tempdir() -> Result<TempDir, std::io::Error> {
    Builder::new()
        .prefix("sorafs-manifest-builder-cli-")
        .tempdir_in(canonical_temp_base())
}
#[test]
fn manifest_builder_rejects_noncanonical_operator_inputs() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.bin");
    fs::write(&payload_path, b"manifest-builder parser boundary").expect("write payload");
    for (arg, expected) in [
        ("--dag-codec=0X71", "canonical unsigned"),
        ("--car-size=000", "canonical unsigned"),
        ("--chunker-profile-id=01", "canonical unsigned"),
        ("--chunker-profile= sorafs.sf1@1.0.0", "whitespace"),
        ("--min-replicas=03", "canonical unsigned"),
        ("--retention-epoch=01", "canonical unsigned"),
        ("--por-sample=0", "greater than zero"),
        ("--por-sample=03", "canonical unsigned"),
        ("--por-sample-seed=0x01", "canonical unsigned"),
        ("--por-proof=01:0:0", "canonical unsigned"),
    ] {
        let output = cargo_bin_cmd!("sorafs_manifest_builder")
            .arg(&payload_path)
            .arg(arg)
            .output()
            .expect("run manifest builder");
        assert!(!output.status.success(), "{arg} should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {arg}, got {stderr}"
        );
    }
}
#[test]
fn manifest_builder_requires_one_positive_retention_epoch() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.bin");
    fs::write(&payload_path, b"manifest retention policy").expect("write payload");
    for (args, expected) in [
        (Vec::new(), "missing required option --retention-epoch"),
        (
            vec!["--retention-epoch=0"],
            "--retention-epoch must be greater than zero",
        ),
        (
            vec!["--retention-epoch=1", "--retention-epoch=2"],
            "--retention-epoch may only be specified once",
        ),
    ] {
        let output = cargo_bin_cmd!("sorafs_manifest_builder")
            .arg(&payload_path)
            .args(args)
            .output()
            .expect("run manifest builder");
        assert!(
            !output.status.success(),
            "invalid retention policy should fail"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?}, got {stderr}"
        );
    }
}
#[test]
fn provider_admission_proposal_rejects_noncanonical_operator_inputs() {
    for (arg, expected) in [
        ("--chunker-profile= sorafs.sf1@1.0.0", "whitespace"),
        ("--chunker-profile=sorafs/sf1@1.0.0", "not canonical"),
        ("--jurisdiction=US", "unknown option"),
        ("--jurisdiction-code=us", "uppercase"),
        ("--capability= range:64", "ASCII whitespace"),
        ("--capability=range:064", "use --range-capability"),
        ("--capability=torii-gateway", "unknown capability"),
        ("--capability=soranet:guard", "use --soranet-pq"),
        ("--soranet-pq=stage-a", "expected exactly"),
        (
            "--range-capability=max_chunk_span=64,min_granularity=4",
            "unknown range-capability field",
        ),
        (
            "--stream-budget=max_in_flight=02,max_bytes_per_sec=1024",
            "canonical unsigned",
        ),
        (
            "--stream-budget=max_in_flight=2, max_bytes_per_sec=1024",
            "ASCII whitespace",
        ),
        (
            "--stream-budget=max-in-flight=2,max_bytes_per_sec=1024",
            "unknown stream-budget field",
        ),
        ("--transport-hint=torii:01", "canonical unsigned"),
        ("--transport-hint= torii:1", "ASCII whitespace"),
        (
            "--transport-hint=torii_http:1",
            "unknown transport protocol",
        ),
        (
            "--endpoint=noritorpc:storage.example",
            "unknown endpoint kind",
        ),
        ("--endpoint-kind=mtls", "unknown option"),
        ("--endpoint-attested-at=1", "unknown option"),
        ("--endpoint-expires-at=2", "unknown option"),
        ("--endpoint-leaf=leaf.der", "unknown option"),
        ("--endpoint-leaf-hex=11", "unknown option"),
        ("--endpoint-alpn=h2", "unknown option"),
        ("--endpoint-report=report.bin", "unknown option"),
        ("--endpoint-report-hex=11", "unknown option"),
        ("--endpoint-intermediate=chain.der", "unknown option"),
        ("--endpoint-intermediate-hex=11", "unknown option"),
    ] {
        let output = cargo_bin_cmd!("sorafs_manifest_builder")
            .arg("provider-admission")
            .arg("proposal")
            .arg(arg)
            .output()
            .expect("run provider-admission proposal");
        assert!(!output.status.success(), "{arg} should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {arg}, got {stderr}"
        );
    }
}
