//! CLI regressions for the SoraFS DA reconstruction harness.
#![cfg(feature = "da_harness")]
use std::{env, fs, path::PathBuf};
use assert_cmd::cargo::cargo_bin_cmd;
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
        .prefix("da-reconstruct-cli-")
        .tempdir_in(canonical_temp_base())
}
#[test]
fn da_reconstruct_rejects_noncanonical_manifest_hex() {
    let cases = [
        ("prefixed.hex", "0x4e52", "0x prefix"),
        ("uppercase.hex", "4E52", "lowercase"),
        ("spaced.hex", "4e 52", "whitespace"),
        ("odd.hex", "4e5", "even number"),
    ];
    for (name, payload, expected) in cases {
        let temp = tempdir().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let manifest_path = temp_path.join(name);
        fs::write(&manifest_path, payload).expect("write manifest");
        let chunks_dir = temp_path.join("chunks");
        fs::create_dir(&chunks_dir).expect("create chunks dir");
        let output_path = temp_path.join("payload.bin");
        let output = cargo_bin_cmd!("da_reconstruct")
            .arg("--manifest")
            .arg(&manifest_path)
            .arg("--chunks-dir")
            .arg(&chunks_dir)
            .arg("--output")
            .arg(&output_path)
            .output()
            .expect("run da_reconstruct");
        assert!(
            !output.status.success(),
            "case {name} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "case {name} stderr did not contain {expected:?}: {stderr}"
        );
        assert!(
            !output_path.exists(),
            "case {name} must fail before writing output"
        );
    }
}
#[test]
fn da_reconstruct_rejects_unsafe_chunk_template_before_output_open() {
    let temp = tempdir().expect("tempdir");
    let temp_path = temp.path().canonicalize().expect("canonical tempdir");
    let fixture_root = workspace_path("fixtures/da/reconstruct/rs_parity_v1");
    let manifest = fixture_root.join("manifest.norito.hex");
    let chunks_dir = fixture_root.join("chunks");
    let output_path = temp_path.join("payload.bin");
    let output = cargo_bin_cmd!("da_reconstruct")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--chunks-dir")
        .arg(&chunks_dir)
        .arg("--output")
        .arg(&output_path)
        .arg("--chunk-template")
        .arg("../chunk_{index:05}.bin")
        .output()
        .expect("run da_reconstruct");
    assert!(
        !output.status.success(),
        "unsafe template unexpectedly succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("single path component") || stderr.contains("regular filename"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !output_path.exists(),
        "unsafe chunk template must fail before creating output"
    );
}
