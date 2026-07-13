//! CLI regression tests for the SoraFS manifest-stub generator.
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
        .prefix("sorafs-manifest-stub-cli-")
        .tempdir_in(canonical_temp_base())
}

#[test]
fn manifest_stub_rejects_noncanonical_operator_inputs() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.bin");
    fs::write(&payload_path, b"manifest-stub parser boundary").expect("write payload");

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
        let output = cargo_bin_cmd!("sorafs_manifest_stub")
            .arg(&payload_path)
            .arg(arg)
            .output()
            .expect("run manifest stub");

        assert!(!output.status.success(), "{arg} should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {arg}, got {stderr}"
        );
    }
}

#[test]
fn manifest_stub_requires_one_positive_retention_epoch() {
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
        let output = cargo_bin_cmd!("sorafs_manifest_stub")
            .arg(&payload_path)
            .args(args)
            .output()
            .expect("run manifest stub");

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
        ("--jurisdiction-code=us", "uppercase"),
        ("--capability= range:64", "ASCII whitespace"),
        ("--capability=range:064", "canonical unsigned"),
        ("--capability=soranet:guard, strict", "ASCII whitespace"),
        (
            "--stream-budget=max_in_flight=02,max_bytes_per_sec=1024",
            "canonical unsigned",
        ),
        (
            "--stream-budget=max_in_flight=2, max_bytes_per_sec=1024",
            "ASCII whitespace",
        ),
        ("--transport-hint=torii:01", "canonical unsigned"),
        ("--transport-hint= torii:1", "ASCII whitespace"),
    ] {
        let output = cargo_bin_cmd!("sorafs_manifest_stub")
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
