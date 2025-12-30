use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::tempdir;

#[test]
fn sorafs_fetch_fixture_copies_and_verifies_local_files() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let fixtures_dir = workspace_root.join("fixtures").join("sorafs_chunker");
    let signatures_path = fixtures_dir.join("manifest_signatures.json");
    let manifest_path = fixtures_dir.join("manifest_blake3.json");

    let temp = tempdir().expect("tempdir");
    let output_dir = temp.path().join("fetched");
    let output_dir_str = output_dir.to_str().expect("output dir utf8").to_owned();

    let mut cmd = cargo_bin_cmd!("xtask");
    let result = cmd
        .args([
            "sorafs-fetch-fixture",
            "--signatures",
            signatures_path.to_str().expect("signatures path utf8"),
            "--out",
            &output_dir_str,
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run sorafs-fetch-fixture");

    assert!(
        result.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let fetched_signatures =
        fs::read(output_dir.join("manifest_signatures.json")).expect("read fetched signatures");
    let fetched_manifest =
        fs::read(output_dir.join("manifest_blake3.json")).expect("read fetched manifest");

    let original_signatures =
        fs::read(&signatures_path).expect("read original manifest_signatures.json");
    let original_manifest = fs::read(&manifest_path).expect("read original manifest_blake3.json");

    assert_eq!(
        fetched_signatures, original_signatures,
        "signature envelope mismatch"
    );
    assert_eq!(
        fetched_manifest, original_manifest,
        "manifest payload mismatch"
    );
}
