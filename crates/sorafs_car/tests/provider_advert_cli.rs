//! CLI regression tests for the SoraFS provider-advert helper.
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
        .prefix("sorafs-provider-advert-cli-")
        .tempdir_in(canonical_temp_base())
}

fn repeated_hex(byte: &str, count: usize) -> String {
    byte.repeat(count)
}

fn base_emit_args(temp: &TempDir) -> Vec<String> {
    vec![
        "--emit".to_string(),
        "--chunker-profile=sorafs.sf1@1.0.0".to_string(),
        format!("--provider-id={}", repeated_hex("11", 32)),
        format!("--stake-pool-id={}", repeated_hex("22", 32)),
        "--stake-amount=5000000".to_string(),
        "--availability=hot".to_string(),
        "--max-latency-ms=1500".to_string(),
        "--max-streams=4".to_string(),
        "--capability=torii".to_string(),
        "--endpoint=torii:storage.example.com".to_string(),
        "--topic=sorafs.sf1.primary:global".to_string(),
        format!("--signing-key={}", repeated_hex("33", 32)),
        format!(
            "--advert-out={}",
            temp.path().join("provider.advert").display()
        ),
        format!(
            "--json-out={}",
            temp.path().join("provider.report.json").display()
        ),
        format!(
            "--public-key-out={}",
            temp.path().join("provider.pub").display()
        ),
        format!(
            "--signature-out={}",
            temp.path().join("provider.sig").display()
        ),
    ]
}

fn output_paths(temp: &TempDir) -> [PathBuf; 4] {
    [
        temp.path().join("provider.advert"),
        temp.path().join("provider.report.json"),
        temp.path().join("provider.pub"),
        temp.path().join("provider.sig"),
    ]
}

#[test]
fn provider_advert_cli_rejects_noncanonical_hex_before_outputs() {
    for (bad_arg, expected) in [
        (
            format!("--provider-id={}", repeated_hex("11", 31)),
            "expected exactly 32 hex bytes",
        ),
        (
            format!("--stake-pool-id={}", repeated_hex("22", 33)),
            "expected exactly 32 hex bytes",
        ),
        (
            format!("--provider-id={}", repeated_hex("AA", 32)),
            "lowercase even-length hex",
        ),
        (
            format!("--signing-key={}", repeated_hex("33", 31)),
            "signing key must be 32-byte seed",
        ),
        (
            format!("--signing-key={}", repeated_hex("CC", 32)),
            "lowercase even-length hex",
        ),
        (
            "--capability=vendor:ABCDEF".to_string(),
            "lowercase even-length hex",
        ),
        (
            "--capability=torii:a".to_string(),
            "lowercase even-length hex",
        ),
        (
            "--endpoint-meta=tls:ABCDEF".to_string(),
            "lowercase even-length hex",
        ),
    ] {
        let temp = tempdir().expect("tempdir");
        let mut args = base_emit_args(&temp);
        args.push(bad_arg.clone());

        let output = cargo_bin_cmd!("sorafs_provider_advert_stub")
            .args(args)
            .output()
            .expect("run provider advert helper");

        assert!(
            !output.status.success(),
            "provider advert unexpectedly succeeded for {bad_arg}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {bad_arg}, got: {stderr}"
        );
        for path in output_paths(&temp) {
            assert!(
                !path.exists(),
                "provider advert must fail before writing {}",
                path.display()
            );
        }
    }
}

#[test]
fn provider_advert_cli_accepts_exact_lowercase_hex_material() {
    let temp = tempdir().expect("tempdir");
    let mut args = base_emit_args(&temp);
    args.push("--capability=vendor:abcdef".to_string());
    args.push("--endpoint-meta=tls:abcdef".to_string());

    let output = cargo_bin_cmd!("sorafs_provider_advert_stub")
        .args(args)
        .output()
        .expect("run provider advert helper");

    assert!(
        output.status.success(),
        "provider advert should accept canonical hex: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for path in output_paths(&temp) {
        assert!(path.exists(), "expected output {}", path.display());
        assert!(
            fs::metadata(&path).expect("output metadata").len() > 0,
            "expected non-empty output {}",
            path.display()
        );
    }
}
