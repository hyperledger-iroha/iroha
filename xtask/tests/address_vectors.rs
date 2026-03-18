use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn address_vectors_verify_defaults() {
    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.arg("address-vectors");
    cmd.arg("--verify");
    cmd.assert().success();
}

#[test]
fn address_vectors_write_custom_path() {
    let dir = TempDir::new().expect("temp dir");
    let output_path = dir.path().join("vectors.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args(["address-vectors", "--out", output_path.to_str().unwrap()]);
    cmd.assert().success();

    let raw = fs::read_to_string(&output_path).expect("vector output");
    assert!(raw.contains("\"format_version\""), "expected ADDR-2 format");
}
